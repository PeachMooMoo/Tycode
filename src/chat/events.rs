use std::time::Instant;

#[derive(Debug, Clone)]
pub enum ChatEvent {
    MessageAdded(ChatMessage),
    TypingStatusChanged(bool),
    ConversationCleared,
    ModelChanged(crate::ai::types::Model),
    TuningsChanged(crate::ai::types::ModelSettings),
    ModelActuallyUsed {
        model: crate::ai::types::Model,
        source: ModelSource,
    },
    Error(String),
}

#[derive(Debug, Clone)]
pub enum ModelSource {
    UserConfigured,  // From settings file
    AgentPreference, // Agent's preferred model
    GlobalDefault,   // Global default fallback
    CommandLine,     // Specified via CLI args
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub content: String,
    pub sender: MessageSender,
    pub timestamp: Instant,
    pub reasoning: Option<crate::ai::types::ReasoningData>,
    pub tool_calls: Vec<crate::ai::types::ToolUseData>,
    pub model_info: Option<ModelInfo>,
}

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub model: crate::ai::types::Model,
    pub source: ModelSource,
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
