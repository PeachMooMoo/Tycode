use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tycode_core::ai::bedrock::BedrockProvider;
use tycode_core::ai::types::ModelSettings;
use tycode_core::chat::{
    actor::{ChatActor, ChatActorMessage},
    commands::CommandHandler,
    events::ChatEvent,
    state::SharedChatState,
};
use tycode_core::settings::SettingsManager;

pub struct BaseApp {
    pub actor_tx: mpsc::UnboundedSender<ChatActorMessage>,
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

        let (actor_tx, actor_rx) = mpsc::unbounded_channel();

        let actor = ChatActor::new(
            chat_state.clone(),
            provider,
            actor_rx,
            workspace_roots,
            settings.clone(),
        );

        // spawn_local for single-threaded execution required by tokio LocalSet
        tokio::task::spawn_local(async move {
            actor.run().await;
        });

        let event_rx = chat_state.subscribe();

        let command_handler = CommandHandler::new(chat_state.clone());

        Ok(Self {
            actor_tx,
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
        self.actor_tx.send(ChatActorMessage::UserInput(message))?;
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<()> {
        self.actor_tx.send(ChatActorMessage::Shutdown)?;
        Ok(())
    }
}
