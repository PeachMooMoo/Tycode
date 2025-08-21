use anyhow::Result;
use std::path::PathBuf;
use tokio::sync::broadcast;
use tycode_core::chat::ChatActorMessage;
use tycode_core::chat::{
    actor::ChatActor, events::ChatEvent, state::SharedChatState,
};
use tycode_core::settings::SettingsManager;

pub struct BaseApp {
    pub actor: ChatActor,
    pub event_rx: broadcast::Receiver<ChatEvent>,
    chat_state: SharedChatState,
}

impl BaseApp {
    pub async fn new(
        workspace_roots: Option<Vec<PathBuf>>,
        settings_path: Option<PathBuf>,
    ) -> Result<Self> {
        let workspace_roots = workspace_roots.unwrap_or_else(|| vec![PathBuf::from(".")]);
        let chat_state = SharedChatState::new();

        // Create a new SettingsManager instance for the actor to own
        let actor_settings = if let Some(path) = settings_path {
            SettingsManager::from_path(path)?
        } else {
            SettingsManager::new()?
        };
        let actor = ChatActor::launch(chat_state.clone(), workspace_roots, actor_settings);

        let event_rx = chat_state.subscribe();

        Ok(Self {
            actor,
            event_rx,
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

    pub async fn change_provider(&self, provider: String) -> Result<()> {
        self.actor
            .tx
            .send(ChatActorMessage::ChangeProvider(provider))?;
        Ok(())
    }

    pub async fn get_settings(&self) -> Result<serde_json::Value> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.actor.tx.send(ChatActorMessage::GetSettings(tx))?;
        rx.await?
    }

    pub async fn save_settings(&self, settings: serde_json::Value) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.actor.tx.send(ChatActorMessage::SaveSettings {
            settings,
            response: tx,
        })?;
        rx.await?
    }
}
