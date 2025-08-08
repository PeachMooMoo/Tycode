use crate::tools::r#trait::ToolExecutor;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

#[derive(Clone)]
pub struct ExecuteCommandTool {
    workspace_root: PathBuf,
}

impl ExecuteCommandTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

#[async_trait::async_trait]
impl ToolExecutor for ExecuteCommandTool {
    fn name(&self) -> &'static str {
        "execute_command"
    }

    fn description(&self) -> &'static str {
        "Execute commands in the workspace (only cargo commands are allowed for security)"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The command to execute"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, arguments: &Value) -> Result<Value> {
        let command_str = arguments
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'command' argument"))?;

        let trimmed_command = command_str.trim();
        if !trimmed_command.starts_with("cargo ") && trimmed_command != "cargo" {
            return Err(anyhow!(
                "Security restriction: Only 'cargo' commands are allowed. Got: '{}'",
                command_str
            ));
        }

        let parts: Vec<&str> = trimmed_command.split_whitespace().collect();
        if parts.is_empty() {
            return Err(anyhow!("Empty command"));
        }

        let args = &parts[1..];

        let output = Command::new("cargo")
            .args(args)
            .current_dir(&self.workspace_root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);
        let success = output.status.success();

        Ok(json!({
            "success": success,
            "exit_code": exit_code,
            "stdout": stdout,
            "stderr": stderr,
            "command": command_str,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_cargo_version_command() {
        let temp_dir = tempdir().unwrap();
        let tool = ExecuteCommandTool::new(temp_dir.path().to_path_buf());

        let args = json!({
            "command": "cargo --version"
        });

        let result = tool.execute(&args).await.unwrap();
        assert_eq!(result["success"], true);
        assert_eq!(result["exit_code"], 0);
        assert!(result["stdout"].as_str().unwrap().contains("cargo"));
    }

    #[tokio::test]
    async fn test_non_cargo_command_rejected() {
        let temp_dir = tempdir().unwrap();
        let tool = ExecuteCommandTool::new(temp_dir.path().to_path_buf());

        let args = json!({
            "command": "ls -la"
        });

        let result = tool.execute(&args).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Security restriction"));
    }

    #[tokio::test]
    async fn test_disguised_command_rejected() {
        let temp_dir = tempdir().unwrap();
        let tool = ExecuteCommandTool::new(temp_dir.path().to_path_buf());

        let args = json!({
            "command": "echo cargo"
        });

        let result = tool.execute(&args).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Security restriction"));
    }

    #[tokio::test]
    async fn test_cargo_with_arguments() {
        let temp_dir = tempdir().unwrap();
        let tool = ExecuteCommandTool::new(temp_dir.path().to_path_buf());

        let args = json!({
            "command": "cargo help"
        });

        let result = tool.execute(&args).await.unwrap();
        assert_eq!(result["success"], true);
        assert!(result["stdout"]
            .as_str()
            .unwrap()
            .contains("Rust's package manager"));
    }
}
