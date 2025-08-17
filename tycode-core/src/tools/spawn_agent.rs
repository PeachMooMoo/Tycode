use crate::agents::AgentCatalog;
use crate::chat::actor::ChatActorMessage;
use crate::tools::r#trait::{ToolExecutor, ToolResult};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Serialize, Deserialize)]
struct SpawnAgentParams {
    /// Clear description of what the sub-agent should accomplish
    task: String,
    /// Relevant context, constraints, or guidance for the sub-agent
    context: Option<String>,
    /// Type of agent to spawn (optional - defaults to appropriate type based on task)
    agent_type: Option<String>,
}

pub struct SpawnAgent {
    actor_tx: UnboundedSender<ChatActorMessage>,
}

impl SpawnAgent {
    pub fn new(actor_tx: UnboundedSender<ChatActorMessage>) -> Self {
        Self { actor_tx }
    }
}

#[async_trait::async_trait(?Send)]
impl ToolExecutor for SpawnAgent {
    fn name(&self) -> &'static str {
        "spawn_agent"
    }

    fn description(&self) -> &'static str {
        "Spawn a sub-agent to handle a specific task. The sub-agent starts with fresh context and runs to completion. Use this to break complex tasks into focused subtasks."
    }

    fn input_schema(&self) -> Value {
        let agent_names = AgentCatalog::get_agent_names();
        let agent_descriptions = AgentCatalog::get_agent_descriptions();
        
        json!({
            "type": "object",
            "required": ["task"],
            "properties": {
                "task": {
                    "type": "string",
                    "description": "Clear, specific description of what the sub-agent should accomplish"
                },
                "context": {
                    "type": "string",
                    "description": "Any relevant context, constraints, or guidance for the sub-agent"
                },
                "agent_type": {
                    "type": "string",
                    "description": format!("Type of agent to spawn. Available agents: {}", agent_descriptions),
                    "enum": agent_names
                }
            }
        })
    }

    async fn execute(&self, arguments: &Value) -> Result<ToolResult> {
        let params: SpawnAgentParams = serde_json::from_value(arguments.clone())?;
        
        // Determine agent type, default to software_engineer
        let agent_type = params.agent_type.unwrap_or_else(|| "software_engineer".to_string());
        
        // Send message to chat actor to push the new agent
        self.actor_tx.send(ChatActorMessage::PushAgent {
            agent_type: agent_type.clone(),
            task: params.task.clone(),
            context: params.context.clone(),
        })?;
        
        let result = json!({
            "status": "spawned",
            "task": params.task,
            "agent_type": agent_type,
            "message": "Sub-agent spawned and is now handling the task"
        });
        
        // UI data for visualization
        let ui_data = json!({
            "type": "agent_spawned",
            "task": params.task,
            "agent_type": agent_type
        });
        
        Ok(ToolResult::with_ui(result, ui_data))
    }
}