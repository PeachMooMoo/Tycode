use crate::agents::ToolType;
use crate::ai::types::{Message, ModelSettings};

pub trait Agent: Send + Sync {
    fn name(&self) -> &str;
    fn system_prompt(&self) -> &str;
    fn preferred_model(&self) -> ModelSettings;
    fn available_tools(&self) -> Vec<ToolType>;
}

pub struct ActiveAgent {
    pub agent: Box<dyn Agent>,
    pub conversation: Vec<Message>,
}

impl ActiveAgent {
    pub fn new(agent: Box<dyn Agent>) -> Self {
        Self {
            agent,
            conversation: Vec::new(),
        }
    }
}
