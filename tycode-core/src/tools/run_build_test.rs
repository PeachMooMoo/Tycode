use crate::security::types::RiskLevel;
use crate::tools::r#trait::{ToolExecutor, ToolRequest, ToolResult};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

#[derive(Clone)]
pub struct RunBuildTestTool {
    workspace_roots: Vec<PathBuf>,
}

impl RunBuildTestTool {
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

        Err(anyhow!(
            "Working directory '{}' is outside allowed workspace roots",
            dir
        ))
    }
}

#[async_trait::async_trait(?Send)]
impl ToolExecutor for RunBuildTestTool {
    fn name(&self) -> &'static str {
        "run_build_test"
    }

    fn description(&self) -> &'static str {
        "Run build, test, or execution commands (cargo build, npm test, python main.py) - NOT for file operations (no cat/ls/grep/find) or shell features (no pipes/redirects); use dedicated file tools instead."
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
                },
                "timeout_seconds": {
                    "type": "integer",
                    "description": "Maximum seconds to wait for command completion",
                    "minimum": 1,
                    "maximum": 300
                }
            },
            "required": ["command", "timeout_seconds"]
        })
    }

    fn evaluate_risk(&self, _arguments: &Value) -> RiskLevel {
        RiskLevel::HighRisk
    }

    async fn execute(&self, request: &ToolRequest) -> Result<ToolResult> {
        let command_str = request
            .arguments
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'command' argument"))?;

        let timeout_seconds = request
            .arguments
            .get("timeout_seconds")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow!("Missing 'timeout_seconds' argument"))?;

        // Determine working directory
        let working_dir = if let Some(dir_str) = request
            .arguments
            .get("working_directory")
            .and_then(|v| v.as_str())
        {
            self.validate_working_directory(dir_str)?
        } else if self.workspace_roots.len() == 1 {
            self.workspace_roots[0].clone()
        } else {
            return Err(anyhow!(
                "Multiple workspace roots available. Please specify 'working_directory' parameter. Available roots: {:?}",
                self.workspace_roots.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()
            ));
        };

        let parts: Vec<&str> = command_str.split_whitespace().collect();
        if parts.is_empty() {
            return Err(anyhow!("Empty command"));
        }

        let (program, args) = (parts[0], &parts[1..]);

        // Spawn the command as a child process
        let child = Command::new(program)
            .args(args)
            .current_dir(&working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true) // Ensure process is killed when dropped
            .spawn()?;

        // Try to get output with timeout
        match timeout(Duration::from_secs(timeout_seconds), async {
            let output = child.wait_with_output().await?;
            Ok::<_, std::io::Error>(output)
        })
        .await
        {
            Ok(Ok(output)) => {
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
            Ok(Err(e)) => Err(anyhow!("Failed to execute command: {}", e)),
            Err(_) => {
                // Timeout - child will be killed when it goes out of scope due to kill_on_drop
                Ok(ToolResult::context_only(json!({
                    "success": false,
                    "exit_code": -1,
                    "stdout": "",
                    "stderr": format!("Command timed out after {} seconds", timeout_seconds),
                    "command": command_str,
                    "working_directory": working_dir.display().to_string(),
                    "timed_out": true
                })))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_cargo_version_command() {
        let temp_dir = tempdir().unwrap();
        let tool = RunBuildTestTool::new(vec![temp_dir.path().to_path_buf()]);

        let args = json!({
            "command": "cargo --version",
            "timeout_seconds": 10
        });

        let request = ToolRequest::new(args, "test_id".to_string());
        let result = tool.execute(&request).await.unwrap();

        match result {
            ToolResult::Success { context_data, .. } => {
                assert_eq!(context_data["success"], true);
                assert_eq!(context_data["exit_code"], 0);
                assert!(context_data["stdout"].as_str().unwrap().contains("cargo"));
            }
            _ => panic!("Expected Success variant"),
        }
    }

    #[tokio::test]
    async fn test_echo_command() {
        let temp_dir = tempdir().unwrap();
        let tool = RunBuildTestTool::new(vec![temp_dir.path().to_path_buf()]);

        let args = json!({
            "command": "echo hello",
            "timeout_seconds": 5
        });

        let request = ToolRequest::new(args, "test_id".to_string());
        let result = tool.execute(&request).await.unwrap();

        match result {
            ToolResult::Success { context_data, .. } => {
                assert_eq!(context_data["success"], true);
                assert_eq!(context_data["exit_code"], 0);
                assert!(context_data["stdout"].as_str().unwrap().contains("hello"));
            }
            _ => panic!("Expected Success variant"),
        }
    }

    #[tokio::test]
    async fn test_cargo_with_arguments() {
        let temp_dir = tempdir().unwrap();
        let tool = RunBuildTestTool::new(vec![temp_dir.path().to_path_buf()]);

        let args = json!({
            "command": "cargo help",
            "timeout_seconds": 10
        });

        let request = ToolRequest::new(args, "test_id".to_string());
        let result = tool.execute(&request).await.unwrap();

        match result {
            ToolResult::Success { context_data, .. } => {
                assert_eq!(context_data["success"], true);
                assert!(context_data["stdout"]
                    .as_str()
                    .unwrap()
                    .contains("Rust's package manager"));
            }
            _ => panic!("Expected Success variant"),
        }
    }

    #[tokio::test]
    async fn test_failed_command() {
        let temp_dir = tempdir().unwrap();
        let tool = RunBuildTestTool::new(vec![temp_dir.path().to_path_buf()]);

        let args = json!({
            "command": "false",  // Unix command that always fails
            "timeout_seconds": 5
        });

        let request = ToolRequest::new(args, "test_id".to_string());
        let result = tool.execute(&request).await.unwrap();

        match result {
            ToolResult::Success { context_data, .. } => {
                assert_eq!(context_data["success"], false);
                assert_ne!(context_data["exit_code"], 0);
            }
            _ => panic!("Expected Success variant"),
        }
    }

    #[tokio::test]
    async fn test_nonexistent_command() {
        let temp_dir = tempdir().unwrap();
        let tool = RunBuildTestTool::new(vec![temp_dir.path().to_path_buf()]);

        let args = json!({
            "command": "this_command_does_not_exist_12345",
            "timeout_seconds": 5
        });

        let request = ToolRequest::new(args, "test_id".to_string());
        let result = tool.execute(&request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_multiple_workspaces_requires_directory() {
        let temp_dir1 = tempdir().unwrap();
        let temp_dir2 = tempdir().unwrap();
        let tool = RunBuildTestTool::new(vec![
            temp_dir1.path().to_path_buf(),
            temp_dir2.path().to_path_buf(),
        ]);

        let args = json!({
            "command": "cargo --version",
            "timeout_seconds": 10
        });

        let request = ToolRequest::new(args, "test_id".to_string());
        let result = tool.execute(&request).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Multiple workspace roots available"));
    }

    #[tokio::test]
    async fn test_with_working_directory() {
        let temp_dir = tempdir().unwrap();
        let tool = RunBuildTestTool::new(vec![temp_dir.path().to_path_buf()]);

        let args = json!({
            "command": "cargo --version",
            "working_directory": temp_dir.path().to_str().unwrap(),
            "timeout_seconds": 10
        });

        let request = ToolRequest::new(args, "test_id".to_string());
        let result = tool.execute(&request).await.unwrap();

        match result {
            ToolResult::Success { context_data, .. } => {
                assert_eq!(context_data["success"], true);
                // Compare canonical paths to handle symlinks on macOS
                let result_dir = context_data["working_directory"].as_str().unwrap();
                let canonical_result = std::path::Path::new(result_dir).canonicalize().unwrap();
                let canonical_expected = temp_dir.path().canonicalize().unwrap();
                assert_eq!(canonical_result, canonical_expected);
            }
            _ => panic!("Expected Success variant"),
        }
    }

    #[tokio::test]
    async fn test_working_directory_outside_workspace_rejected() {
        let temp_dir = tempdir().unwrap();
        let other_dir = tempdir().unwrap();
        let tool = RunBuildTestTool::new(vec![temp_dir.path().to_path_buf()]);

        let args = json!({
            "command": "cargo --version",
            "working_directory": other_dir.path().to_str().unwrap(),
            "timeout_seconds": 10
        });

        let request = ToolRequest::new(args, "test_id".to_string());
        let result = tool.execute(&request).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("outside allowed workspace roots"));
    }

    #[tokio::test]
    async fn test_command_timeout() {
        let temp_dir = tempdir().unwrap();
        let tool = RunBuildTestTool::new(vec![temp_dir.path().to_path_buf()]);

        let args = json!({
            "command": "sleep 10",
            "timeout_seconds": 1
        });

        let request = ToolRequest::new(args, "test_id".to_string());
        let result = tool.execute(&request).await.unwrap();

        match result {
            ToolResult::Success { context_data, .. } => {
                assert_eq!(context_data["success"], false);
                assert_eq!(context_data["timed_out"], true);
                assert!(context_data["stderr"]
                    .as_str()
                    .unwrap()
                    .contains("timed out"));
            }
            _ => panic!("Expected Success variant"),
        }
    }

    #[tokio::test]
    async fn test_missing_timeout_parameter() {
        let temp_dir = tempdir().unwrap();
        let tool = RunBuildTestTool::new(vec![temp_dir.path().to_path_buf()]);

        let args = json!({
            "command": "echo hello"
        });

        let request = ToolRequest::new(args, "test_id".to_string());
        let result = tool.execute(&request).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing 'timeout_seconds' argument"));
    }
}
