use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;
use tycode_core::ai::bedrock::BedrockProvider;
use tycode_core::ai::types::ModelSettings;
use tycode_core::chat::{
    actor::ChatActor,
    commands::CommandHandler,
    events::ChatEvent,
    state::SharedChatState,
};
use tycode_core::settings::SettingsManager;

pub struct BaseApp {
    pub actor: ChatActor,
    pub event_rx: broadcast::Receiver<ChatEvent>,
    pub command_handler: CommandHandler,
    pub settings: Option<Arc<SettingsManager>>,
    chat_state: SharedChatState,
}

impl BaseApp {
    pub async fn new(
        provider: BedrockProvider,
        _tunings: ModelSettings,
        workspace_roots: Option<Vec<PathBuf>>,
        settings: Option<Arc<SettingsManager>>,
    ) -> Result<Self> {
        let workspace_roots = workspace_roots.unwrap_or_else(|| vec![PathBuf::from(".")]);
        let chat_state = SharedChatState::new();

        let actor = ChatActor::launch(
            chat_state.clone(),
            provider,
            workspace_roots,
            settings.clone(),
        );

        let event_rx = chat_state.subscribe();
        let command_handler = CommandHandler::new(chat_state.clone());

        Ok(Self {
            actor,
            event_rx,
            command_handler,
            settings,
            chat_state,
        })
    }

    pub fn subscribe_to_events(&self) -> broadcast::Receiver<ChatEvent> {
        self.chat_state.subscribe()
    }

    pub async fn send_message(&self, message: String) -> Result<()> {
        self.actor.send_message(message).await
    }

    pub async fn cancel(&self) -> Result<()> {
        self.actor.cancel().await
    }
}
