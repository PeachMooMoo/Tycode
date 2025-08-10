use anyhow::Result;
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
        settings: Option<Arc<SettingsManager>>,
    ) -> Result<Self> {
        // Create shared chat state
        let chat_state = SharedChatState::new();

        // Create actor communication channel
        let (actor_tx, actor_rx) = mpsc::unbounded_channel();

        // Create and spawn the chat actor with settings
        let actor =
            ChatActor::with_settings(chat_state.clone(), provider, actor_rx, settings.clone());

        // Use spawn_local for single-threaded execution
        tokio::task::spawn_local(async move {
            actor.run().await;
        });

        // Subscribe to events
        let event_rx = chat_state.subscribe();

        // Create command handler for getting command info
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
