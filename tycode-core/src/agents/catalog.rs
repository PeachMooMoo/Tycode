use crate::agents::{
    agent::Agent, code_review::CodeReviewAgent, coder::CoderAgent, coordinator::CoordinatorAgent,
    software_engineer_agent::SoftwareEngineerAgent,
};

/// Information about an available agent
#[derive(Clone, Debug)]
pub struct AgentInfo {
    pub name: String,
    pub description: String,
}

/// Registry of available agents
pub struct AgentCatalog;

impl AgentCatalog {
    /// Get all available agents with their descriptions
    pub fn list_agents() -> Vec<AgentInfo> {
        vec![
            AgentInfo {
                name: "coordinator".to_string(),
                description: "Coordinates task execution, breaking requests into steps and delegating to sub‑agents".to_string(),
            },
            AgentInfo {
                name: "software_engineer".to_string(),
                description: "Plans, implements, and reviews code following strict style mandates".to_string(),
            },
            AgentInfo {
                name: "coder".to_string(),
                description: "Executes assigned coding tasks, applying patches and managing files".to_string(),
            },
            AgentInfo {
                name: "code_reviewer".to_string(),
                description: "Approves or rejects proposed code changes to ensure compliance with style mandates".to_string(),
            },
        ]
    }

    /// Create an agent instance by name
    pub fn create_agent(name: &str) -> Option<Box<dyn Agent>> {
        match name {
            "coordinator" => Some(Box::new(CoordinatorAgent)),
            "coder" => Some(Box::new(CoderAgent)),
            "software_engineer" => Some(Box::new(SoftwareEngineerAgent)),
            "code_reviewer" => Some(Box::new(CodeReviewAgent)),
            _ => None,
        }
    }

    /// Get agent descriptions as a formatted string for tool schemas
    pub fn get_agent_descriptions() -> String {
        let agents = Self::list_agents();
        agents
            .iter()
            .map(|a| format!("'{}': {}", a.name, a.description))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Get valid agent names for enum schema
    pub fn get_agent_names() -> Vec<String> {
        Self::list_agents().iter().map(|a| a.name.clone()).collect()
    }
}
