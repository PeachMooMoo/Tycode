use crate::chat::events::{ChatEvent, ChatMessage};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

/// ChatState holds UI-specific state for rendering the chat interface.
/// 
/// This state is owned by SharedChatState and is used to:
/// - Display chat message history to the user
/// - Show typing/thinking indicators
/// - Track command input history for UI features like arrow-key navigation
/// 
/// This state should NOT contain:
/// - Application configuration (belongs in ActorState)
/// - File tracking state (belongs in ActorState)
/// - Security settings (belongs in ActorState)
/// - AI provider configuration (belongs in ActorState)
/// - Any business logic state (belongs in ActorState)
/// 
/// The separation ensures that the UI layer only contains display-specific
/// state while all application logic remains in the actor.
pub struct ChatState {
    /// Messages to display in the chat UI
    pub messages: VecDeque<ChatMessage>,
    /// Command history for UI navigation (up/down arrows)
    pub input_history: Vec<String>,
    /// Whether the assistant is currently processing (shows loading indicator)
    pub is_typing: bool,
}

impl ChatState {
    pub fn new() -> Self {
        Self {
            messages: VecDeque::new(),
            input_history: Vec::new(),
            is_typing: false,
        }
    }
}

/// Configuration for chat behavior - moved to ActorState
#[derive(Debug, Clone)]
pub struct ChatConfig {
    pub file_modification_api: FileModificationApi,
    pub trace: bool,
}

impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            file_modification_api: FileModificationApi::FindReplace,
            trace: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FileModificationApi {
    Patch,
    FindReplace,
}

impl Default for FileModificationApi {
    fn default() -> Self {
        FileModificationApi::FindReplace
    }
}

/// SharedChatState wraps UI-specific state and provides event broadcasting.
/// 
/// This is a lightweight wrapper that:
/// - Maintains chat UI state (messages, typing status, input history)
/// - Broadcasts events to UI subscribers for reactive updates
/// - Provides thread-safe access to UI state
/// 
/// All application logic, configuration, and business state is managed
/// by the ChatActor and its ActorState.
#[derive(Clone)]
pub struct SharedChatState {
    inner: Arc<Mutex<ChatState>>,
    pub event_tx: broadcast::Sender<ChatEvent>,
}

impl SharedChatState {
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(100);
        Self {
            inner: Arc::new(Mutex::new(ChatState::new())),
            event_tx,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ChatEvent> {
        self.event_tx.subscribe()
    }

    pub fn add_message(&self, message: ChatMessage) {
        {
            let mut state = self.inner.lock().unwrap();
            state.messages.push_back(message.clone());
        }
        let _ = self.event_tx.send(ChatEvent::MessageAdded(message));
    }

    pub fn set_typing(&self, typing: bool) {
        {
            let mut state = self.inner.lock().unwrap();
            state.is_typing = typing;
        }
        let _ = self.event_tx.send(ChatEvent::TypingStatusChanged(typing));
    }

    pub fn add_to_history(&self, input: String) {
        let mut state = self.inner.lock().unwrap();
        if !input.trim().is_empty() {
            state.input_history.push(input);
        }
    }

    pub fn clear_conversation(&self) {
        {
            let mut state = self.inner.lock().unwrap();
            state.messages.clear();
        }
        let _ = self.event_tx.send(ChatEvent::ConversationCleared);
    }

    pub fn get_messages(&self) -> VecDeque<ChatMessage> {
        let state = self.inner.lock().unwrap();
        state.messages.clone()
    }

    pub fn is_typing(&self) -> bool {
        let state = self.inner.lock().unwrap();
        state.is_typing
    }

    pub fn get_input_history(&self) -> Vec<String> {
        let state = self.inner.lock().unwrap();
        state.input_history.clone()
    }
}
