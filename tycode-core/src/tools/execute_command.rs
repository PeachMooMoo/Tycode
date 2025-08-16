use crate::tools::r#trait::{ToolExecutor, ToolResult};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

#[derive(Clone)]
pub struct ExecuteCommandTool {
    workspace_roots: Vec<PathBuf>,
}

impl ExecuteCommandTool {
    pub fn new(workspace_roots: Vec<PathBuf>) -> Self {
        Self { workspace_roots }
    }

    fn validate_working_directory(&self, dir: &str) -> Result<PathBuf> {
        let path = Path::new(dir);
        let canonical = if path.is_absolute() {
            path.canonicalize()?
        } else {
            // Try to resolve relative to each workspace root
            let mut found = None;
            for root in &self.workspace_roots {
                let candidate = root.join(path);
                if candidate.exists() {
                    found = Some(candidate.canonicalize()?);
                    break;
                }
            }
            found.ok_or_else(|| anyhow!("Directory '{}' not found in any workspace", dir))?
        };

        // Verify it's within one of our workspace roots
        for root in &self.workspace_roots {
            let canonical_root = root.canonicalize()?;
            if canonical.starts_with(&canonical_root) {
                return Ok(canonical);
            }
        }

        Err(anyhow!("Working directory '{}' is outside allowed workspace roots", dir))
    }
}

#[async_trait::async_trait(?Send)]
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
                },
                "working_directory": {
                    "type": "string",
                    "description": "The directory to run the command in (must be within a workspace root)"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, arguments: &Value) -> Result<ToolResult> {
        let command_str = arguments
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'command' argument"))?;

        // Determine working directory
        let working_dir = if let Some(dir_str) = arguments.get("working_directory").and_then(|v| v.as_str()) {
            self.validate_working_directory(dir_str)?
        } else if self.workspace_roots.len() == 1 {
            self.workspace_roots[0].clone()
        } else {
            return Err(anyhow!(
                "Multiple workspace roots available. Please specify 'working_directory' parameter. Available roots: {:?}",
                self.workspace_roots.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()
            ));
        };

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
            .current_dir(&working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);
        let success = output.status.success();

        Ok(ToolResult::context_only(json!({
            "success": success,
            "exit_code": exit_code,
            "stdout": stdout,
            "stderr": stderr,
            "command": command_str,
            "working_directory": working_dir.display().to_string(),
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_cargo_version_command() {
        let temp_dir = tempdir().unwrap();
        let tool = ExecuteCommandTool::new(vec![temp_dir.path().to_path_buf()]);

        let args = json!({
            "command": "cargo --version"
        });

        let result = tool.execute(&args).await.unwrap();
        assert_eq!(result.context_data["success"], true);
        assert_eq!(result.context_data["exit_code"], 0);
        assert!(result.context_data["stdout"].as_str().unwrap().contains("cargo"));
    }

    #[tokio::test]
    async fn test_non_cargo_command_rejected() {
        let temp_dir = tempdir().unwrap();
        let tool = ExecuteCommandTool::new(vec![temp_dir.path().to_path_buf()]);

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
        let tool = ExecuteCommandTool::new(vec![temp_dir.path().to_path_buf()]);

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
        let tool = ExecuteCommandTool::new(vec![temp_dir.path().to_path_buf()]);

        let args = json!({
            "command": "cargo help"
        });

        let result = tool.execute(&args).await.unwrap();
        assert_eq!(result.context_data["success"], true);
        assert!(result.context_data["stdout"]
            .as_str()
            .unwrap()
            .contains("Rust's package manager"));
    }

    #[tokio::test]
    async fn test_multiple_workspaces_requires_directory() {
        let temp_dir1 = tempdir().unwrap();
        let temp_dir2 = tempdir().unwrap();
        let tool = ExecuteCommandTool::new(vec![
            temp_dir1.path().to_path_buf(),
            temp_dir2.path().to_path_buf(),
        ]);

        let args = json!({
            "command": "cargo --version"
        });

        let result = tool.execute(&args).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Multiple workspace roots available"));
    }

    #[tokio::test]
    async fn test_with_working_directory() {
        let temp_dir = tempdir().unwrap();
        let tool = ExecuteCommandTool::new(vec![temp_dir.path().to_path_buf()]);

        let args = json!({
            "command": "cargo --version",
            "working_directory": temp_dir.path().to_str().unwrap()
        });

        let result = tool.execute(&args).await.unwrap();
        assert_eq!(result.context_data["success"], true);
        assert_eq!(result.context_data["working_directory"], temp_dir.path().to_str().unwrap());
    }

    #[tokio::test]
    async fn test_working_directory_outside_workspace_rejected() {
        let temp_dir = tempdir().unwrap();
        let other_dir = tempdir().unwrap();
        let tool = ExecuteCommandTool::new(vec![temp_dir.path().to_path_buf()]);

        let args = json!({
            "command": "cargo --version",
            "working_directory": other_dir.path().to_str().unwrap()
        });

        let result = tool.execute(&args).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("outside allowed workspace roots"));
    }
}
