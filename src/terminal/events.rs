#[derive(Debug, Clone)]
pub enum AppMessage {
    UserInput(String),
    AiResponse(String),
    AiResponseWithContent(crate::ai::types::Content),
    SystemMessage(String),
    Error(String),
    Typing,
    ToolResults(Vec<crate::ai::types::ToolResultData>),
}

#[derive(Debug)]
pub enum AppEvent {
    Tick,
    Key(crossterm::event::KeyEvent),
    AiMessage(AppMessage),
}

pub use crate::chat::events::ChatMessage;

impl crate::chat::events::MessageSender {
    pub fn color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            crate::chat::events::MessageSender::User => Color::Cyan,
            crate::chat::events::MessageSender::Assistant => Color::Green,
            crate::chat::events::MessageSender::System => Color::Yellow,
            crate::chat::events::MessageSender::Error => Color::Red,
        }
    }
}
