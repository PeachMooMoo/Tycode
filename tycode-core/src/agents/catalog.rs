use crate::agents::{
    agent::Agent, design_agent::DesignAgent, software_engineer_agent::SoftwareEngineerAgent,
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
                name: "design".to_string(),
                description: "Analyzes requirements, proposes architecture, creates design documents. Best for planning and high-level design work.".to_string(),
            },
            AgentInfo {
                name: "software_engineer".to_string(),
                description: "Implements code changes, refactors, writes tests. Best for hands-on coding tasks.".to_string(),
            },
        ]
    }

    /// Create an agent instance by name
    pub fn create_agent(name: &str) -> Option<Box<dyn Agent>> {
        match name {
            "design" => Some(Box::new(DesignAgent)),
            "software_engineer" => Some(Box::new(SoftwareEngineerAgent)),
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
