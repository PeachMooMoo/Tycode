use crate::agents::{ActiveAgent, SoftwareEngineerAgent};
use crate::ai::{
    provider::AiProvider,
    types::{Content, Message, MessageRole},
};
use crate::chat::{
    ai,
    events::{ChatEvent, ChatMessage, MessageSender},
    state::{ChatConfig, SharedChatState},
};
use crate::security::SecurityManager;
use crate::settings::{ProviderConfig, SettingsManager};
use anyhow::{bail, Result};
use aws_config::timeout::TimeoutConfig;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{error, info};

#[derive(Debug, Clone)]
pub struct AgentCompletionResult {
    pub success: bool,
    pub summary: String,
    pub artifacts: Option<serde_json::Value>,
}

pub enum ChatActorMessage {
    UserInput(String),
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
            let provider = match create_default_provider(&settings).await {
                Ok(p) => p,
                Err(e) => {
                    error!("Failed to initialize provider: {}", e);
                    return;
                }
            };

            let security_config = settings.settings().security.clone();
            let security_manager = SecurityManager::new(security_config);

            let actor_state = ActorState {
                state: state.clone(),
                provider,
                agent_stack: vec![ActiveAgent::new(Box::new(SoftwareEngineerAgent))],
                workspace_roots,
                security_manager,
                settings,
                config: ChatConfig::default(),
                tracked_files: HashSet::new(),
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
}

pub struct ActorState {
    pub state: SharedChatState,
    pub provider: Box<dyn AiProvider>,
    pub agent_stack: Vec<ActiveAgent>,
    pub workspace_roots: Vec<PathBuf>,
    pub security_manager: SecurityManager,
    pub settings: SettingsManager,
    pub config: ChatConfig,
    pub tracked_files: HashSet<PathBuf>,
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
            result = process_message(&mut rx, &mut state) => {
                if let Err(e) = result {
                    error!(?e, "Error processing message");
                    add_error_message(&state.state, format!("Error: {:?}", e));
                }
            }

            // Handle cancellation even when no message is being processed
            Some(_) = cancel_rx.recv() => {
                info!("Cancellation received while idle");
                handle_cancelled(&mut state);
            }
        }

        state.state.set_typing(false);
    }
}

async fn process_message(
    rx: &mut mpsc::UnboundedReceiver<ChatActorMessage>,
    state: &mut ActorState,
) -> Result<()> {
    let Some(message) = rx.recv().await else {
        bail!("request queue dropped")
    };
    state.state.set_typing(true);
    match message {
        ChatActorMessage::UserInput(input) => handle_user_input(state, input).await,
        ChatActorMessage::ChangeProvider(provider) => handle_provider_change(state, provider).await,
        ChatActorMessage::ReloadSettings => handle_settings_reload(state).await,
        ChatActorMessage::GetSettings(response) => {
            let settings = state.settings.settings();
            let settings_json = serde_json::to_value(settings)
                .map_err(|e| anyhow::anyhow!("Failed to serialize settings: {}", e));
            let _ = response.send(settings_json);
            Ok(())
        }
        ChatActorMessage::SaveSettings { settings, response } => {
            let result = (|| -> Result<()> {
                let new_settings: crate::settings::config::Settings =
                    serde_json::from_value(settings)
                        .map_err(|e| anyhow::anyhow!("Failed to deserialize settings: {}", e))?;
                state.settings.save_settings(new_settings)?;
                state.settings.reload()?;
                Ok(())
            })();
            let _ = response.send(result);
            Ok(())
        }
    }
}

fn handle_cancelled(state: &mut ActorState) {
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
        let messages = crate::chat::commands::process_command(state, command).await;

        for message in messages {
            add_message(&state.state, message);
        }
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

    ai::current_agent_mut(state).conversation.push(Message {
        role: MessageRole::User,
        content: Content::text_only(input),
    });

    ai::send_ai_request(state).await
}

async fn handle_settings_reload(state: &mut ActorState) -> Result<()> {
    info!("Reloading settings from disk");
    state.settings.reload()?;

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

async fn handle_provider_change(state: &mut ActorState, provider_name: String) -> Result<()> {
    info!("Changing provider to: {}", provider_name);
    state.provider = create_provider(&state.settings, &provider_name).await?;

    add_message(
        &state.state,
        ChatMessage {
            content: format!("Switched to provider: {}", provider_name),
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

/// Initializes the provider with the given name if it exists in settings, else
/// raises an error.
async fn create_provider(
    settings: &SettingsManager,
    provider: &str,
) -> Result<Box<dyn AiProvider>> {
    let config = settings.settings();
    let Some(provider_config) = config.providers.get(provider) else {
        bail!("No active provider configured in settings")
    };

    match provider_config {
        ProviderConfig::Bedrock { profile, region } => {
            use crate::ai::bedrock::BedrockProvider;
            use aws_config::retry::RetryConfig;
            use aws_config::Region;

            if region.is_empty() {
                bail!("AWS region is empty")
            };

            let aws_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
                .profile_name(profile)
                .region(Region::new(region.to_string()))
                .retry_config(RetryConfig::disabled())
                .timeout_config(
                    // Tuned for Alaska airline's Wifi
                    TimeoutConfig::builder()
                        .connect_timeout(Duration::from_secs(60))
                        .operation_attempt_timeout(Duration::from_secs(300))
                        .read_timeout(Duration::from_secs(300))
                        .build(),
                )
                .load()
                .await;

            let client = aws_sdk_bedrockruntime::Client::new(&aws_config);
            Ok(Box::new(BedrockProvider::new(client)))
        }
        ProviderConfig::Mock { behavior } => {
            use crate::ai::mock::{MockBehavior, MockProvider};

            let mock_behavior = match behavior {
                crate::settings::config::MockBehaviorConfig::Success => MockBehavior::Success,
                crate::settings::config::MockBehaviorConfig::RetryThenSuccess {
                    errors_before_success,
                } => MockBehavior::RetryableErrorThenSuccess {
                    remaining_errors: *errors_before_success,
                },
                crate::settings::config::MockBehaviorConfig::AlwaysRetryError => {
                    MockBehavior::AlwaysRetryableError
                }
                crate::settings::config::MockBehaviorConfig::AlwaysError => {
                    MockBehavior::AlwaysNonRetryableError
                }
                crate::settings::config::MockBehaviorConfig::ToolUse {
                    tool_name,
                    tool_arguments,
                } => MockBehavior::ToolUse {
                    tool_name: tool_name.clone(),
                    tool_arguments: tool_arguments.clone(),
                },
            };

            Ok(Box::new(MockProvider::new(mock_behavior)))
        }
    }
}

/// Creates the provider marked as default from the current settings. Note: the
/// "active" provider in the settings is just the default that is used if the
/// user hasn't selected an overriding provider (using the ChangeProvider event)
async fn create_default_provider(settings: &SettingsManager) -> Result<Box<dyn AiProvider>> {
    let default = &settings.settings().active_provider;
    create_provider(settings, &default).await
}
