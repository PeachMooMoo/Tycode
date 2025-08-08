use crate::ai::{
    bedrock::BedrockProvider,
    types::{Model, ModelSettings},
};
use crate::chat::{
    actor::{ChatActor, ChatActorMessage},
    events::{ChatEvent, ChatMessage, MessageSender},
};
use crate::terminal::{splash::SplashScreen, state::SharedState, ui::UI};
use anyhow::Result;
use arboard::Clipboard;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use std::{env, time::Instant};
use tokio::sync::mpsc;

pub struct App {
    state: SharedState,
    actor_tx: mpsc::UnboundedSender<ChatActorMessage>,
    ui: UI,
    splash: SplashScreen,
    _event_task: tokio::task::JoinHandle<()>,
}

impl App {
    pub async fn new(
        provider: BedrockProvider,
        model: Model,
        _system_prompt: String,
        _tunings: ModelSettings,
    ) -> Result<Self> {
        let state = SharedState::new();
        let (actor_tx, actor_rx) = mpsc::unbounded_channel();

        let workspace_root = env::current_dir()?;

        let chat_state = state.chat_state().clone();
        let actor = ChatActor::new(chat_state, provider, workspace_root, actor_rx);

        tokio::spawn(async move {
            actor.run().await;
        });

        // Set up event handling to sync chat events with terminal state
        let mut event_rx = state.chat_state().subscribe();
        let terminal_state = state.clone();
        let event_task = tokio::spawn(async move {
            while let Ok(event) = event_rx.recv().await {
                match event {
                    ChatEvent::MessageAdded(message) => {
                        terminal_state.update_messages(|messages| {
                            messages.push_back(message);
                        });
                    }
                    ChatEvent::TypingStatusChanged(typing) => {
                        terminal_state.set_typing(typing);
                    }
                    ChatEvent::ConversationCleared => {
                        terminal_state.clear_conversation();
                    }
                    _ => {}
                }
            }
        });

        let app = Self {
            state: state.clone(),
            actor_tx,
            ui: UI::new(),
            splash: SplashScreen::new(),
            _event_task: event_task,
        };

        state.update_messages(|messages| {
            messages.push_back(ChatMessage {
                content: format!(
                    "🎯 TyCode Chat\nModel: {}\nType /help for commands",
                    model.name()
                ),
                sender: MessageSender::System,
                timestamp: Instant::now(),
                reasoning: None,
                tool_calls: Vec::new(),
                model_info: None,
            });
        });

        Ok(app)
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        let mut state = self.state.lock();

        if state.show_help || state.show_settings {
            match key.code {
                KeyCode::Esc => {
                    state.show_help = false;
                    state.show_settings = false;
                }
                _ => {}
            }
            return;
        }

        // Check if AI is processing - prevent most input except help and settings
        if state.is_typing {
            match key.code {
                KeyCode::F(1) => {
                    state.show_help = !state.show_help;
                }
                KeyCode::F(2) => {
                    state.show_settings = !state.show_settings;
                }
                KeyCode::Esc => {
                    state.show_help = false;
                    state.show_settings = false;
                }
                // Allow scrolling while AI is processing
                KeyCode::Up => {
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        state.scroll_state.scroll_up();
                    }
                }
                KeyCode::Down => {
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        state.scroll_state.scroll_down();
                    }
                }
                KeyCode::PageUp => {
                    state.scroll_state.scroll_page_up();
                }
                KeyCode::PageDown => {
                    state.scroll_state.scroll_page_down();
                }
                KeyCode::Home => {
                    state.scroll_state.scroll_to_top();
                }
                KeyCode::End => {
                    state.scroll_state.scroll_to_bottom();
                }
                _ => {
                    // Ignore other input while AI is processing
                    return;
                }
            }
            return;
        }

        // Handle Ctrl+V (Windows/Linux) or Cmd+V (macOS) for paste
        if key.code == KeyCode::Char('v')
            && (key.modifiers.contains(KeyModifiers::CONTROL)
                || key.modifiers.contains(KeyModifiers::SUPER))
        {
            if let Ok(mut clipboard) = Clipboard::new() {
                if let Ok(clipboard_text) = clipboard.get_text() {
                    // Insert clipboard text at current cursor position
                    for line in clipboard_text.lines() {
                        state.input.insert_str(line);
                        // Add newline for all lines except the last one
                        if line != clipboard_text.lines().last().unwrap_or("") {
                            state.input.insert_newline();
                        }
                    }
                }
            }
            // Don't process this key further
            return;
        }

        if state.splash_active {
            state.splash_active = false;
            return;
        }

        match key.code {
            KeyCode::Esc => {
                state.show_help = false;
                state.show_settings = false;
            }
            KeyCode::Enter => {
                let input = state.input.lines().join("\n");

                if !input.trim().is_empty() {
                    // Clear the textarea
                    state.input = tui_textarea::TextArea::default();
                    state
                        .input
                        .set_cursor_line_style(ratatui::style::Style::default());
                    state.history_index = None;

                    // Add to history directly through StateHandle to avoid double-locking
                    state.input_history.push(input.clone());

                    let tx = self.actor_tx.clone();
                    tokio::spawn(async move {
                        let _ = tx.send(ChatActorMessage::UserInput(input));
                    });
                }
            }
            KeyCode::Up => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    state.scroll_state.scroll_up();
                } else {
                    self.navigate_history_up(&mut state);
                }
            }
            KeyCode::Down => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    state.scroll_state.scroll_down();
                } else {
                    self.navigate_history_down(&mut state);
                }
            }
            KeyCode::PageUp => {
                state.scroll_state.scroll_page_up();
            }
            KeyCode::PageDown => {
                state.scroll_state.scroll_page_down();
            }
            KeyCode::Home => {
                state.scroll_state.scroll_to_top();
            }
            KeyCode::End => {
                state.scroll_state.scroll_to_bottom();
            }
            KeyCode::F(1) => {
                state.show_help = !state.show_help;
            }
            KeyCode::F(2) => {
                state.show_settings = !state.show_settings;
            }
            _ => {
                state.input.input(Event::Key(key));
            }
        }
    }

    fn navigate_history_up(&self, state: &mut crate::terminal::state::StateHandle) {
        if state.input_history.is_empty() {
            return;
        }

        let new_index = match state.history_index {
            None => Some(state.input_history.len() - 1),
            Some(i) => {
                if i > 0 {
                    Some(i - 1)
                } else {
                    Some(0)
                }
            }
        };

        if let Some(index) = new_index {
            state.history_index = Some(index);
            // Create new TextArea with history content
            let mut textarea = tui_textarea::TextArea::default();
            textarea.set_cursor_line_style(ratatui::style::Style::default());
            textarea.insert_str(&state.input_history[index]);
            state.input = textarea;
        }
    }

    fn navigate_history_down(&self, state: &mut crate::terminal::state::StateHandle) {
        match state.history_index {
            None => {}
            Some(i) => {
                if i < state.input_history.len() - 1 {
                    state.history_index = Some(i + 1);
                    // Create new TextArea with history content
                    let mut textarea = tui_textarea::TextArea::default();
                    textarea.set_cursor_line_style(ratatui::style::Style::default());
                    textarea.insert_str(&state.input_history[i + 1]);
                    state.input = textarea;
                } else {
                    state.history_index = None;
                    // Clear the textarea
                    state.input = tui_textarea::TextArea::default();
                    state
                        .input
                        .set_cursor_line_style(ratatui::style::Style::default());
                }
            }
        }
    }

    pub fn check_splash_timeout(&mut self) {
        if self.splash.should_auto_dismiss() {
            self.state.dismiss_splash();
        }
    }

    pub fn render(&mut self, f: &mut Frame) {
        {
            let state = self.state.lock();
            if state.splash_active {
                self.ui.render_splash(f);
                return;
            }
        }

        // Get a new lock for rendering
        let mut state = self.state.lock();

        // Extract values to avoid borrowing conflicts
        let messages = state.messages.clone();
        let input = state.input.clone();
        let is_typing = state.is_typing;
        let show_help = state.show_help;
        let show_settings = state.show_settings;

        // Get chat-related values from chat state
        let file_modification_api = self.state.chat_state().get_file_modification_api();

        // Use default values for model and tunings since they're no longer managed by chat state
        let model = Model::default();
        let tunings = ModelSettings::default();
        let system_prompt = "System prompt managed by settings".to_string();

        self.ui.render_all(
            f,
            &messages,
            &input,
            is_typing,
            &model,
            &tunings,
            show_help,
            show_settings,
            &system_prompt,
            &file_modification_api,
            &mut state.scroll_state,
        );
    }

    pub async fn shutdown(&self) {
        let _ = self.actor_tx.send(ChatActorMessage::Shutdown);
    }
}
