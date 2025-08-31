use crate::agents::agent::Agent;
use crate::agents::tool_type::ToolType;
use crate::ai::model::Model;
use crate::ai::types::ModelSettings;

pub struct DesignAgent;

impl Agent for DesignAgent {
    fn name(&self) -> &str {
        "design"
    }

    fn system_prompt(&self) -> &str {
        "You are a software design agent. Your role is to analyze requirements, propose architecture, and create detailed design documents. You focus on system design, data models, API specifications, and overall software architecture. You can read existing code to understand current systems and search for patterns."
    }

    fn preferred_model(&self) -> ModelSettings {
        ModelSettings {
            model: Model::ClaudeSonnet4,
            max_tokens: Some(64000),
            temperature: Some(1.0),
            top_p: None,
            reasoning_budget: Some(8000),
        }
    }

    fn available_tools(&self) -> Vec<ToolType> {
        vec![
            ToolType::ReadFile,
            ToolType::ListFiles,
            ToolType::SearchFiles,
        ]
    }
}
