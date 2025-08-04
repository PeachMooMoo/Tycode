use std::time::Instant;

#[derive(Debug, Clone)]
pub enum ChatEvent {
    MessageAdded(ChatMessage),
    TypingStatusChanged(bool),
    ConversationCleared,
    ModelChanged(crate::ai::types::Model),
    TuningsChanged(crate::ai::types::ModelTunings),
    Error(String),
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub content: String,
    pub sender: MessageSender,
    pub timestamp: Instant,
    pub reasoning: Option<crate::ai::types::ReasoningData>,
    pub tool_calls: Vec<crate::ai::types::ToolUseData>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MessageSender {
    User,
    Assistant,
    System,
    Error,
}

impl MessageSender {
    pub fn prefix(&self) -> &str {
        match self {
            MessageSender::User => "You",
            MessageSender::Assistant => "AI",
            MessageSender::System => "System",
            MessageSender::Error => "Error",
        }
    }
}
