use crate::agents::{ActiveAgent, SoftwareEngineerAgent, ToolType};
use crate::ai::{error::AiError, ContentBlock, ModelSettings};
use crate::ai::{
    provider::AiProvider,
    types::{
        Content, ConversationRequest, ConversationResponse, Message, MessageContext, MessageRole,
    },
};
use crate::chat::{
    commands::CommandHandler,
    events::{
        ChatEvent, ChatMessage, ContextInfo, FileInfo, MessageSender, ModelInfo, ModelSource,
    },
    state::SharedChatState,
};
use crate::settings::SettingsManager;
use crate::tools::context_utils::list_relevant_files;
use crate::tools::file_access::FileAccessManager;
use crate::tools::registry::ToolRegistry;
use anyhow::{bail, Result};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::{error, info, warn};

pub enum ChatActorMessage {
    UserInput(String),
    ProcessCommand(String),
    ContinueConversation,
    ChangeProvider(String),
    ReloadSettings,
    GetSettings(tokio::sync::oneshot::Sender<Result<serde_json::Value>>),
    SaveSettings {
        settings: serde_json::Value,
        response: tokio::sync::oneshot::Sender<Result<()>>,
    },
}

/// Handle to interact with the chat actor
pub struct ChatActor {
    pub tx: mpsc::UnboundedSender<ChatActorMessage>,
    pub cancel_tx: mpsc::UnboundedSender<()>,
}

/// Internal state for the actor implementation
struct ActorState {
    state: SharedChatState,
    provider: Box<dyn AiProvider>,
    agent_stack: Vec<ActiveAgent>,
    command_handler: CommandHandler,
    settings: SettingsManager,
    workspace_roots: Vec<PathBuf>,
}

impl ChatActor {
    /// Launch the chat actor and return a handle to it
    pub fn launch(
        state: SharedChatState,
        workspace_roots: Vec<PathBuf>,
        settings: SettingsManager,
    ) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let (cancel_tx, cancel_rx) = mpsc::unbounded_channel();

        tokio::task::spawn_local(async move {
            let provider = match create_provider_from_settings(&settings).await {
                Ok(p) => p,
                Err(e) => {
                    error!("Failed to initialize provider: {}", e);
                    // Don't just return - we need to keep the actor running
                    // This error will be handled when trying to send messages
                    return;
                }
            };

            let actor_state = ActorState {
                state: state.clone(),
                provider,
                agent_stack: vec![ActiveAgent::new(Box::new(SoftwareEngineerAgent))],
                command_handler: CommandHandler::new(state),
                settings,
                workspace_roots,
            };

            run_actor(actor_state, rx, cancel_rx).await;
        });

        ChatActor { tx, cancel_tx }
    }

    pub async fn send_message(&self, message: String) -> Result<()> {
        self.tx.send(ChatActorMessage::UserInput(message))?;
        Ok(())
    }

    pub async fn cancel(&self) -> Result<()> {
        self.cancel_tx.send(())?;
        Ok(())
    }

    pub async fn change_provider(&self, provider: String) -> Result<()> {
        self.tx.send(ChatActorMessage::ChangeProvider(provider))?;
        Ok(())
    }
}

async fn create_provider_from_settings(settings: &SettingsManager) -> Result<Box<dyn AiProvider>> {
    let config = settings.settings();

    if let Some(provider_config) = config.active_provider() {
        match provider_config {
            crate::settings::ProviderConfig::Bedrock { profile, region } => {
                use crate::ai::bedrock::BedrockProvider;
                use aws_config::retry::RetryConfig;
                use aws_config::Region;

                // Ensure region is not empty, use default if needed
                if region.is_empty() {
                    bail!("AWS region is empty")
                };

                let aws_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
                    .profile_name(profile)
                    .region(Region::new(region.to_string()))
                    .retry_config(RetryConfig::disabled())
                    .load()
                    .await;

                let client = aws_sdk_bedrockruntime::Client::new(&aws_config);
                Ok(Box::new(BedrockProvider::new(client)))
            }
        }
    } else {
        Err(anyhow::anyhow!("No active provider configured in settings"))
    }
}

// Actor implementation as free functions
async fn run_actor(
    mut state: ActorState,
    mut rx: mpsc::UnboundedReceiver<ChatActorMessage>,
    mut cancel_rx: mpsc::UnboundedReceiver<()>,
) {
    info!("ChatActor started");

    loop {
        tokio::select! {
            // Handle incoming messages
            Some(message) = rx.recv() => {
                // Now process the message with cancellation support
                tokio::select! {
                    Some(_) = cancel_rx.recv() => {
                        info!("Operation cancelled");
                        handle_cancelled(&mut state);
                        // The process_message future is dropped here
                    }
                    result = process_message(&mut state, message) => {
                        if let Err(e) = result {
                            error!(?e, "Error processing message");
                            add_error_message(&state.state, format!("Error: {:?}", e));
                        }
                    }
                }
            }

            // Handle cancellation even when no message is being processed
            Some(_) = cancel_rx.recv() => {
                info!("Cancellation received while idle");
                // Just consume the cancellation - nothing to cancel when idle
            }

            // Both channels closed, exit
            else => {
                info!("ChatActor shutting down");
                break;
            }
        }
    }
}

async fn process_message(state: &mut ActorState, message: ChatActorMessage) -> Result<()> {
    match message {
        ChatActorMessage::UserInput(input) => handle_user_input(state, input).await,
        ChatActorMessage::ProcessCommand(command) => {
            handle_command(state, &command).await;
            Ok(())
        }
        ChatActorMessage::ContinueConversation => send_ai_request(state).await,
        ChatActorMessage::ChangeProvider(provider) => handle_provider_change(state, provider).await,
        ChatActorMessage::ReloadSettings => handle_settings_reload(state).await,
        ChatActorMessage::GetSettings(response) => {
            let settings = state.settings.settings();
            let settings_json = serde_json::to_value(settings)?;
            let _ = response.send(Ok(settings_json));
            Ok(())
        }
        ChatActorMessage::SaveSettings { settings, response } => {
            use crate::settings::Settings;
            let result: Result<()> = (|| {
                let settings_obj: Settings = serde_json::from_value(settings)?;
                state.settings.save_settings(settings_obj)?;
                Ok(())
            })();
            let _ = response.send(result);
            Ok(())
        }
    }
}

fn handle_cancelled(state: &mut ActorState) {
    state.state.set_typing(false);

    // Send cancellation event
    let _ = state.state.event_tx.send(ChatEvent::OperationCancelled {
        message: "Operation cancelled by user".to_string(),
    });

    add_message(
        &state.state,
        ChatMessage {
            content: "Operation cancelled.".to_string(),
            sender: MessageSender::System,
            timestamp: Instant::now(),
            reasoning: None,
            tool_calls: Vec::new(),
            model_info: None,
            context_info: None,
            token_usage: None,
        },
    );
}

async fn handle_user_input(state: &mut ActorState, input: String) -> Result<()> {
    if input.trim().is_empty() {
        return Ok(());
    }

    if let Some(command) = input.strip_prefix('/') {
        handle_command(state, command).await;
        return Ok(());
    }

    state.state.add_to_history(input.clone());

    add_message(
        &state.state,
        ChatMessage {
            content: input.clone(),
            sender: MessageSender::User,
            timestamp: Instant::now(),
            reasoning: None,
            tool_calls: Vec::new(),
            model_info: None,
            context_info: None,
            token_usage: None,
        },
    );

    current_agent_mut(state).conversation.push(Message {
        role: MessageRole::User,
        content: Content::text_only(input),
    });

    send_ai_request(state).await
}

async fn handle_command(state: &mut ActorState, command: &str) {
    let messages = state
        .command_handler
        .handle_command(command, Some(state.provider.as_ref()))
        .await;

    for message in messages {
        if message.content == "Conversation cleared." {
            current_agent_mut(state).conversation.clear();
        }
        add_message(&state.state, message);
    }
}

async fn build_message_context(state: &ActorState) -> MessageContext {
    let mut context = MessageContext::new(state.workspace_roots.clone());

    let relevant_files = list_relevant_files(&state.workspace_roots);
    context.set_relevant_files(relevant_files.files);

    let tracked_files = state.state.get_tracked_files();
    let file_manager = FileAccessManager::new(state.workspace_roots.clone());

    for file_path in tracked_files {
        let path_str = file_path.to_string_lossy();
        match file_manager.read_file(&path_str).await {
            Ok(content) => {
                context.add_tracked_file(file_path, content);
            }
            Err(e) => {
                warn!(?e, "Failed to read tracked file: {:?}", file_path);
            }
        }
    }

    context
}

async fn send_ai_request(state: &mut ActorState) -> Result<()> {
    state.state.set_typing(true);

    loop {
        let current = current_agent(state);
        let allowed_tools: HashSet<ToolType> =
            current.agent.available_tools().into_iter().collect();

        let file_modification_api = state.state.get_file_modification_api();
        let tool_registry = ToolRegistry::new(
            state.workspace_roots.clone(),
            file_modification_api,
            Some(Arc::new(state.state.clone())),
        );

        let allowed_tool_types: Vec<ToolType> = allowed_tools.into_iter().collect();
        let available_tools = tool_registry.get_tool_definitions_for_types(&allowed_tool_types);

        let message_context = build_message_context(state).await;

        let mut messages_for_request = Vec::new();

        let context_string = message_context.to_formatted_string();

        let dir_list_size = message_context
            .relevant_files
            .iter()
            .map(|p| p.to_string_lossy().len() + 1)
            .sum::<usize>();

        let files: Vec<FileInfo> = message_context
            .tracked_file_contents
            .iter()
            .map(|(path, content)| FileInfo {
                path: path.to_string_lossy().to_string(),
                bytes: content.len(),
            })
            .collect();

        let context_info = Some(ContextInfo {
            directory_list_bytes: dir_list_size,
            files,
        });

        let context_message = Message {
            role: MessageRole::User,
            content: Content::text_only(format!("Current Context:\n{}", context_string)),
        };
        messages_for_request.push(context_message);

        messages_for_request.extend(current_agent(state).conversation.clone());

        let (model_settings, model_source) = determine_model_settings_and_source(state, current);

        let system_prompt = current.agent.system_prompt().to_string();

        let request = ConversationRequest {
            messages: messages_for_request,
            model: model_settings.clone(),
            system_prompt,
            stop_sequences: vec![],
            tools: available_tools,
        };

        info!(?request, "AI request");

        let response = match send_request_with_retry(state, request).await {
            Ok(response) => response,
            Err(e) => {
                state.state.set_typing(false);
                add_error_message(&state.state, format!("Error: {:?}", e));
                return Ok(());
            }
        };

        let content = response.content.clone();

        // Log detailed response information if trace is enabled
        info!(?response, "AI response");

        let reasoning = content.reasoning().first().map(|r| (*r).clone());
        let tool_calls: Vec<_> = content.tool_uses().iter().map(|t| (*t).clone()).collect();

        add_message(
            &state.state,
            ChatMessage {
                content: content.text(),
                sender: MessageSender::Assistant,
                timestamp: Instant::now(),
                reasoning,
                tool_calls: tool_calls.clone(),
                model_info: Some(ModelInfo {
                    model: model_settings.model,
                    source: model_source.clone(),
                }),
                context_info: context_info.clone(),
                token_usage: Some(response.usage.clone()),
            },
        );

        current_agent_mut(state).conversation.push(Message {
            role: MessageRole::Assistant,
            content: content,
        });

        if !tool_calls.is_empty() {
            info!(
                tool_count = tool_calls.len(),
                tools = ?tool_calls.iter().map(|t| &t.name).collect::<Vec<_>>(),
                "Executing tool calls"
            );

            for tool_use in &tool_calls {
                let (result, ui_data) = tool_registry.execute_tool(tool_use).await;

                info!(
                    tool_name = %tool_use.name,
                    ?result,
                    ?ui_data,
                    "Tool execution completed"
                );

                // Emit tool completion event
                let parsed_result = if !result.is_error {
                    serde_json::from_str(&result.content).ok()
                } else {
                    None
                };

                info!(
                    "Emitting ToolExecutionCompleted event: tool={}, success={}, has_result={}, has_ui_data={}, has_error={}",
                    tool_use.name,
                    !result.is_error,
                    parsed_result.is_some(),
                    ui_data.is_some(),
                    result.is_error
                );

                let event = ChatEvent::ToolExecutionCompleted {
                    tool_name: tool_use.name.clone(),
                    success: !result.is_error,
                    result: parsed_result,
                    ui_data,
                    error: if result.is_error {
                        Some(result.content.clone())
                    } else {
                        None
                    },
                };

                // Send the event through the broadcast channel
                if let Err(e) = state.state.event_tx.send(event.clone()) {
                    error!("Failed to send tool completion event: {:?}", e);
                } else {
                    info!(
                        "Successfully sent tool completion event for {}",
                        tool_use.name
                    );
                }

                current_agent_mut(state).conversation.push(Message {
                    role: MessageRole::User,
                    content: vec![
                        ContentBlock::ToolResult(result),
                        ContentBlock::Text("Here is the tool result:".to_string()),
                    ]
                    .into(),
                });
            }
            continue;
        } else {
            break;
        }
    }

    state.state.set_typing(false);
    Ok(())
}

async fn handle_settings_reload(state: &mut ActorState) -> Result<()> {
    info!("Reloading settings from disk");
    
    // Reload settings from disk
    state.settings.reload()?;
    
    // Check if the active provider has changed and update if needed
    let settings = state.settings.settings();
    let active_provider = &settings.active_provider;
    
    // Only recreate provider if it's different from current
    // This is a bit tricky since we can't easily compare providers
    // For now, we'll recreate the provider to ensure it's up to date
    if let Some(provider_config) = settings.providers.get(active_provider) {
        match provider_config {
            crate::settings::ProviderConfig::Bedrock { profile, region } => {
                use crate::ai::bedrock::BedrockProvider;
                use aws_config::retry::RetryConfig;

                // Ensure region is not empty, use default if needed
                let region_str = if region.is_empty() {
                    "us-west-2"
                } else {
                    region.as_str()
                };

                let aws_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
                    .profile_name(profile)
                    .region(aws_config::Region::new(region_str.to_string()))
                    .retry_config(RetryConfig::disabled())
                    .load()
                    .await;

                let client = aws_sdk_bedrockruntime::Client::new(&aws_config);
                state.provider = Box::new(BedrockProvider::new(client));
                
                info!("Reloaded provider configuration for: {}", active_provider);
            }
        }
    }
    
    add_message(
        &state.state,
        ChatMessage {
            content: "Settings reloaded successfully.".to_string(),
            sender: MessageSender::System,
            timestamp: Instant::now(),
            reasoning: None,
            tool_calls: Vec::new(),
            model_info: None,
            context_info: None,
            token_usage: None,
        },
    );
    
    Ok(())
}

// Helper functions
fn current_agent(state: &ActorState) -> &ActiveAgent {
    state.agent_stack.last().expect("No active agent")
}

fn current_agent_mut(state: &mut ActorState) -> &mut ActiveAgent {
    state.agent_stack.last_mut().expect("No active agent")
}

fn add_message(state: &SharedChatState, message: ChatMessage) {
    state.add_message(message);
}

fn add_error_message(state: &SharedChatState, error: String) {
    add_message(
        state,
        ChatMessage {
            content: error,
            sender: MessageSender::Error,
            timestamp: Instant::now(),
            reasoning: None,
            tool_calls: Vec::new(),
            model_info: None,
            context_info: None,
            token_usage: None,
        },
    );
}

fn determine_model_settings_and_source(
    _state: &ActorState,
    agent: &ActiveAgent,
) -> (ModelSettings, ModelSource) {
    // For now, we just use the agent's preferred model
    // In the future, we could allow model configuration per provider
    (agent.agent.preferred_model(), ModelSource::AgentPreference)
}

async fn send_request_with_retry(
    state: &mut ActorState,
    request: ConversationRequest,
) -> Result<ConversationResponse> {
    const MAX_RETRIES: u32 = 1000;
    const INITIAL_BACKOFF_MS: u64 = 100;
    const MAX_BACKOFF_MS: u64 = 1000;
    const BACKOFF_MULTIPLIER: f64 = 2.0;

    let mut attempt = 0;

    loop {
        match try_send_request(&state.provider, &request).await {
            Ok(response) => {
                if attempt > 0 {
                    info!("Request succeeded after {} retries", attempt);
                }
                return Ok(response);
            }
            Err(error) => {
                if !should_retry(&error, attempt, MAX_RETRIES) {
                    warn!(
                        attempt,
                        max_retries = MAX_RETRIES,
                        "Request failed after {} retries: {}",
                        attempt,
                        error
                    );
                    return Err(error.into());
                }

                let backoff_ms = calculate_backoff(
                    attempt,
                    INITIAL_BACKOFF_MS,
                    MAX_BACKOFF_MS,
                    BACKOFF_MULTIPLIER,
                );

                emit_retry_event(state, attempt + 1, MAX_RETRIES, &error, backoff_ms);

                warn!(
                    attempt = attempt + 1,
                    max_retries = MAX_RETRIES,
                    backoff_ms,
                    error = %error,
                    "Request failed, retrying after backoff"
                );

                sleep(Duration::from_millis(backoff_ms)).await;
                attempt += 1;
            }
        }
    }
}

async fn try_send_request(
    provider: &Box<dyn AiProvider>,
    request: &ConversationRequest,
) -> Result<ConversationResponse, AiError> {
    provider.converse(request.clone()).await
}

fn should_retry(error: &AiError, attempt: u32, max_retries: u32) -> bool {
    matches!(error, AiError::Retryable(_)) && attempt < max_retries
}

fn calculate_backoff(attempt: u32, initial_ms: u64, max_ms: u64, multiplier: f64) -> u64 {
    let base_backoff = initial_ms as f64 * multiplier.powi(attempt as i32);
    base_backoff.min(max_ms as f64) as u64
}

fn emit_retry_event(
    state: &ActorState,
    attempt: u32,
    max_retries: u32,
    error: &AiError,
    backoff_ms: u64,
) {
    let retry_event = ChatEvent::RetryAttempt {
        attempt,
        max_retries,
        error: error.to_string(),
        backoff_ms,
    };

    if let Err(e) = state.state.event_tx.send(retry_event) {
        error!("Failed to send retry event: {:?}", e);
    }
}

async fn handle_provider_change(state: &mut ActorState, provider_name: String) -> Result<()> {
    info!("Changing provider to: {}", provider_name);

    // Reload settings from disk to get latest configuration
    state.settings.reload()?;

    let settings = state.settings.settings();

    let Some(provider_config) = settings.providers.get(&provider_name) else {
        bail!("Provider name: {provider_name} not found in settings");
    };

    match provider_config {
        crate::settings::ProviderConfig::Bedrock { profile, region } => {
            use crate::ai::bedrock::BedrockProvider;
            use aws_config::retry::RetryConfig;
            use aws_sdk_bedrockruntime::Client as BedrockClient;

            // Ensure region is not empty, use default if needed
            let region_str = if region.is_empty() {
                "us-west-2"
            } else {
                region.as_str()
            };

            let aws_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
                .profile_name(profile)
                .region(aws_config::Region::new(region_str.to_string()))
                .retry_config(RetryConfig::disabled())
                .load()
                .await;

            let client = BedrockClient::new(&aws_config);
            let provider = BedrockProvider::new(client);
            state.provider = Box::new(provider);

            add_message(
                &state.state,
                ChatMessage {
                    content: format!(
                        "Switched to provider: {} (AWS profile: {})",
                        provider_name, profile
                    ),
                    sender: MessageSender::System,
                    timestamp: Instant::now(),
                    reasoning: None,
                    tool_calls: Vec::new(),
                    model_info: None,
                    context_info: None,
                    token_usage: None,
                },
            );
        }
    }

    Ok(())
}
