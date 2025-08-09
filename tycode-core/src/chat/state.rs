use crate::chat::events::{ChatEvent, ChatMessage};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

pub struct ChatState {
    pub messages: VecDeque<ChatMessage>,
    pub input_history: Vec<String>,
    pub is_typing: bool,
    pub config: ChatConfig,
}

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

impl ChatState {
    pub fn new() -> Self {
        Self {
            messages: VecDeque::new(),
            input_history: Vec::new(),
            is_typing: false,
            config: ChatConfig::default(),
        }
    }
}

#[derive(Clone)]
pub struct SharedChatState {
    inner: Arc<Mutex<ChatState>>,
    event_tx: broadcast::Sender<ChatEvent>,
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

    pub fn get_config(&self) -> ChatConfig {
        let state = self.inner.lock().unwrap();
        state.config.clone()
    }

    pub fn set_config(&self, config: ChatConfig) {
        let mut state = self.inner.lock().unwrap();
        state.config = config;
    }

    pub fn set_file_modification_api(&self, api: FileModificationApi) {
        let mut state = self.inner.lock().unwrap();
        state.config.file_modification_api = api;
    }

    pub fn get_file_modification_api(&self) -> FileModificationApi {
        let state = self.inner.lock().unwrap();
        state.config.file_modification_api.clone()
    }

    pub fn set_trace(&self, trace: bool) {
        let mut state = self.inner.lock().unwrap();
        state.config.trace = trace;
    }

    pub fn get_trace(&self) -> bool {
        let state = self.inner.lock().unwrap();
        state.config.trace
    }
}
