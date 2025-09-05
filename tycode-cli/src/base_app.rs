use anyhow::Result;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tycode_core::chat::{actor::ChatActor, events::ChatEvent};
use tycode_core::settings::SettingsManager;

pub struct BaseApp {
    pub actor: ChatActor,
    pub event_rx: mpsc::UnboundedReceiver<ChatEvent>,
}

impl BaseApp {
    pub async fn new(
        workspace_roots: Option<Vec<PathBuf>>,
        settings_path: Option<PathBuf>,
    ) -> Result<Self> {
        let workspace_roots = workspace_roots.unwrap_or_else(|| vec![PathBuf::from(".")]);

        // Create a new SettingsManager instance for the actor to own
        let actor_settings = if let Some(path) = settings_path {
            SettingsManager::from_path(path)?
        } else {
            SettingsManager::new()?
        };
        let (actor, event_rx) = ChatActor::launch(workspace_roots, actor_settings);

        Ok(Self { actor, event_rx })
    }

    pub fn send_message(&self, message: String) -> Result<()> {
        self.actor.send_message(message)
    }

    pub fn cancel(&self) -> Result<()> {
        self.actor.cancel()
    }
}
