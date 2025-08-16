use crate::agents::{ActiveAgent, SoftwareEngineerAgent, ToolType};
use crate::ai::{
    bedrock::BedrockProvider,
    provider::AiProvider,
    types::{Content, ConversationRequest, Message, MessageContext, MessageRole},
};
use crate::ai::{ContentBlock, ModelSettings};
use crate::chat::{
    commands::CommandHandler,
    events::{ChatEvent, ChatMessage, ContextInfo, FileInfo, MessageSender, ModelInfo, ModelSource},
    state::SharedChatState,
};
use crate::settings::SettingsManager;
use crate::tools::context_utils::list_relevant_files;
use crate::tools::file_access::FileAccessManager;
use crate::tools::registry::ToolRegistry;
use anyhow::Result;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

pub enum ChatActorMessage {
    UserInput(String),
    ProcessCommand(String),
    ContinueConversation,
}

/// Handle to interact with the chat actor
pub struct ChatActor {
    pub tx: mpsc::UnboundedSender<ChatActorMessage>,
    pub cancel_tx: mpsc::UnboundedSender<()>,
}

/// Internal state for the actor implementation
struct ActorState {
    state: SharedChatState,
    provider: BedrockProvider,
    agent_stack: Vec<ActiveAgent>,
    command_handler: CommandHandler,
    settings: Option<Arc<SettingsManager>>,
    workspace_roots: Vec<PathBuf>,
}

impl ChatActor {
    /// Launch the chat actor and return a handle to it
    pub fn launch(
        state: SharedChatState,
        provider: BedrockProvider,
        workspace_roots: Vec<PathBuf>,
        settings: Option<Arc<SettingsManager>>,
    ) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let (cancel_tx, cancel_rx) = mpsc::unbounded_channel();

        let actor_state = ActorState {
            state: state.clone(),
            provider,
            agent_stack: vec![ActiveAgent::new(Box::new(SoftwareEngineerAgent))],
            command_handler: CommandHandler::new(state),
            settings,
            workspace_roots,
        };

        tokio::task::spawn_local(async move {
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
    }
}

fn handle_cancelled(state: &mut ActorState) {
    state.state.set_typing(false);
    
    // Send cancellation event
    let _ = state.state.event_tx.send(ChatEvent::OperationCancelled {
        message: "Operation cancelled by user".to_string(),
    });
    
    add_message(&state.state, ChatMessage {
        content: "Operation cancelled.".to_string(),
        sender: MessageSender::System,
        timestamp: Instant::now(),
        reasoning: None,
        tool_calls: Vec::new(),
        model_info: None,
        context_info: None,
        token_usage: None,
    });
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

    add_message(&state.state, ChatMessage {
        content: input.clone(),
        sender: MessageSender::User,
        timestamp: Instant::now(),
        reasoning: None,
        tool_calls: Vec::new(),
        model_info: None,
        context_info: None,
        token_usage: None,
    });

    current_agent_mut(state).conversation.push(Message {
        role: MessageRole::User,
        content: Content::text_only(input),
    });

    send_ai_request(state).await
}

async fn handle_command(state: &mut ActorState, command: &str) {
    let messages = state
        .command_handler
        .handle_command(command, Some(&state.provider))
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

        match state.provider.converse(request).await {
            Ok(response) => {
                let content = response.content.clone();

                // Log detailed response information if trace is enabled
                info!(?response, "AI response");

                let reasoning = content.reasoning().first().map(|r| (*r).clone());
                let tool_calls: Vec<_> =
                    content.tool_uses().iter().map(|t| (*t).clone()).collect();

                add_message(&state.state, ChatMessage {
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
                });

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
                        let result = tool_registry.execute_tool(tool_use).await;

                        info!(
                            tool_name = %tool_use.name,
                            ?result,
                            "Tool execution completed"
                        );

                        // Emit tool completion event
                        let parsed_result = if !result.is_error {
                            serde_json::from_str(&result.content).ok()
                        } else {
                            None
                        };
                        
                        info!(
                            "Emitting ToolExecutionCompleted event: tool={}, success={}, has_result={}, has_error={}",
                            tool_use.name,
                            !result.is_error,
                            parsed_result.is_some(),
                            result.is_error
                        );
                        
                        let event = ChatEvent::ToolExecutionCompleted {
                            tool_name: tool_use.name.clone(),
                            success: !result.is_error,
                            result: parsed_result,
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
                            info!("Successfully sent tool completion event for {}", tool_use.name);
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
            Err(e) => {
                error!(?e, "AI request failed");
                add_error_message(&state.state, format!("Error: {:?}", e));
                break;
            }
        }
    }

    state.state.set_typing(false);
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
    add_message(state, ChatMessage {
        content: error,
        sender: MessageSender::Error,
        timestamp: Instant::now(),
        reasoning: None,
        tool_calls: Vec::new(),
        model_info: None,
        context_info: None,
        token_usage: None,
    });
}

fn determine_model_settings_and_source(
    state: &ActorState,
    agent: &ActiveAgent,
) -> (ModelSettings, ModelSource) {
    let agent_name = agent.agent.name();

    if let Some(settings) = &state.settings {
        if let Some(agent_settings) = settings.settings().get_agent_settings(agent_name) {
            return (
                ModelSettings {
                    model: agent_settings.model,
                    max_tokens: agent_settings.max_tokens,
                    temperature: agent_settings.temperature,
                    top_p: agent_settings.top_p,
                    reasoning_budget: agent_settings.reasoning_budget,
                },
                ModelSource::UserConfigured,
            );
        }
    }

    (agent.agent.preferred_model(), ModelSource::AgentPreference)
}
