use crate::ai::types::{Model, ModelTunings};
use crate::chat::{events::ChatMessage, state::SharedChatState};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};
use tui_scrollview::ScrollViewState;
use tui_textarea::TextArea;

pub struct State {
    pub messages: VecDeque<ChatMessage>,
    pub input: TextArea<'static>,
    pub input_history: Vec<String>,
    pub history_index: Option<usize>,
    pub is_typing: bool,
    pub show_help: bool,
    pub show_settings: bool,
    pub splash_active: bool,
    pub scroll_state: ScrollViewState,
}

impl State {
    pub fn new() -> Self {
        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(ratatui::style::Style::default());

        Self {
            messages: VecDeque::new(),
            input: textarea,
            input_history: Vec::new(),
            history_index: None,
            is_typing: false,
            show_help: false,
            show_settings: false,
            splash_active: true,
            scroll_state: ScrollViewState::default(),
        }
    }
}

#[derive(Clone)]
pub struct SharedState {
    inner: Arc<Mutex<State>>,
    chat_state: SharedChatState,
}

impl SharedState {
    pub fn new(model: Model, system_prompt: String, tunings: ModelTunings) -> Self {
        Self {
            inner: Arc::new(Mutex::new(State::new())),
            chat_state: SharedChatState::new(model, system_prompt, tunings),
        }
    }

    pub fn chat_state(&self) -> &SharedChatState {
        &self.chat_state
    }

    pub fn lock(&self) -> StateHandle {
        StateHandle {
            guard: self.inner.lock().unwrap(),
        }
    }

    pub fn update_messages<F>(&self, f: F)
    where
        F: FnOnce(&mut VecDeque<ChatMessage>),
    {
        let mut state = self.inner.lock().unwrap();
        f(&mut state.messages);
        state.scroll_state.scroll_to_bottom();
    }

    pub fn set_typing(&self, typing: bool) {
        let mut state = self.inner.lock().unwrap();
        state.is_typing = typing;
    }

    pub fn add_to_history(&self, input: String) {
        let mut state = self.inner.lock().unwrap();
        if !input.trim().is_empty() {
            state.input_history.push(input);
        }
    }

    pub fn clear_conversation(&self) {
        let mut state = self.inner.lock().unwrap();
        state.messages.clear();
        state.scroll_state = ScrollViewState::default();
    }

    pub fn dismiss_splash(&self) {
        let mut state = self.inner.lock().unwrap();
        state.splash_active = false;
    }

    pub fn toggle_help(&self) {
        let mut state = self.inner.lock().unwrap();
        state.show_help = !state.show_help;
    }

    pub fn toggle_settings(&self) {
        let mut state = self.inner.lock().unwrap();
        state.show_settings = !state.show_settings;
    }

    pub fn reset_dialogs(&self) {
        let mut state = self.inner.lock().unwrap();
        state.show_help = false;
        state.show_settings = false;
    }
}

pub struct StateHandle<'a> {
    guard: MutexGuard<'a, State>,
}

impl<'a> std::ops::Deref for StateHandle<'a> {
    type Target = State;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl<'a> std::ops::DerefMut for StateHandle<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}
