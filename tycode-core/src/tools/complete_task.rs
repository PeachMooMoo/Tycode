use crate::chat::actor::ChatActorMessage;
use crate::tools::r#trait::{ToolExecutor, ToolResult};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Serialize, Deserialize)]
struct CompleteTaskParams {
    /// Summary of what was accomplished (or what failed)
    summary: String,
    /// Whether the task was successfully completed
    success: bool,
    /// Reason for failure (required if success is false)
    failure_reason: Option<String>,
    /// Optional key data/artifacts to pass back to parent agent
    artifacts: Option<Value>,
}

pub struct CompleteTask {
    actor_tx: UnboundedSender<ChatActorMessage>,
}

impl CompleteTask {
    pub fn new(actor_tx: UnboundedSender<ChatActorMessage>) -> Self {
        Self { actor_tx }
    }
}

#[async_trait::async_trait(?Send)]
impl ToolExecutor for CompleteTask {
    fn name(&self) -> &'static str {
        "complete_task"
    }

    fn description(&self) -> &'static str {
        "Signal task completion (success or failure) and return control to parent agent. \
         FAIL a task when: \
         • Required resources/files don't exist \
         • The task requirements are unclear or contradictory \
         • You encounter errors you cannot resolve \
         • The requested change would break existing functionality \
         • You lack necessary permissions or access \
         SUCCEED when: \
         • All requested changes are implemented \
         • The task objectives are met"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["summary", "success"],
            "properties": {
                "summary": {
                    "type": "string",
                    "description": "Clear summary of what was accomplished or what went wrong"
                },
                "success": {
                    "type": "boolean",
                    "description": "Whether the task completed successfully"
                },
                "failure_reason": {
                    "type": "string",
                    "description": "Specific reason for failure (required if success is false)"
                },
                "artifacts": {
                    "type": "object",
                    "description": "Optional data to pass back to parent agent (e.g., created file paths, important values)"
                }
            }
        })
    }

    async fn execute(&self, arguments: &Value) -> Result<ToolResult> {
        let params: CompleteTaskParams = serde_json::from_value(arguments.clone())?;

        // Validate failure reason is provided when failing
        if !params.success && params.failure_reason.is_none() {
            return Err(anyhow::anyhow!(
                "failure_reason is required when success is false"
            ));
        }

        // Create combined summary including failure reason if present
        let summary = if let Some(ref reason) = params.failure_reason {
            format!("{}\nReason: {}", params.summary, reason)
        } else {
            params.summary.clone()
        };

        // Send message to chat actor to pop the current agent
        self.actor_tx.send(ChatActorMessage::PopAgent {
            success: params.success,
            summary: summary.clone(),
            artifacts: params.artifacts.clone(),
        })?;

        let result = json!({
            "status": "completed",
            "success": params.success,
            "summary": summary,
            "message": if params.success {
                "Task completed successfully"
            } else {
                "Task failed"
            }
        });

        // UI data for visualization
        let ui_data = json!({
            "type": "task_completed",
            "success": params.success,
            "summary": params.summary,
            "failure_reason": params.failure_reason
        });

        Ok(ToolResult::with_ui(result, ui_data))
    }
}
