use crate::security::types::RiskLevel;
use crate::tools::file_access::FileAccessManager;
use crate::tools::r#trait::{ToolExecutor, ToolRequest, ToolResult};
use anyhow::Result;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Clone)]
pub struct DeleteFileTool {
    file_manager: FileAccessManager,
}

impl DeleteFileTool {
    pub fn new(workspace_roots: Vec<PathBuf>) -> Self {
        let file_manager = FileAccessManager::new(workspace_roots);
        Self { file_manager }
    }
}

#[async_trait::async_trait(?Send)]
impl ToolExecutor for DeleteFileTool {
    fn name(&self) -> &'static str {
        "delete_file"
    }

    fn description(&self) -> &'static str {
        "Delete a file or empty directory"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file or directory to delete"
                }
            },
            "required": ["file_path"]
        })
    }

    fn evaluate_risk(&self, arguments: &Value) -> RiskLevel {
        if let Some(file_path) = arguments.get("file_path").and_then(|v| v.as_str()) {
            self.file_manager.evaluate_path_risk(file_path)
        } else {
            RiskLevel::HighRisk
        }
    }

    async fn execute(&self, request: &ToolRequest) -> Result<ToolResult> {
        let file_path = request.arguments
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: file_path"))?;

        // Use FileAccessManager for secure file deletion
        self.file_manager.delete_file(file_path).await?;

        Ok(ToolResult::context_only(json!({
            "success": true,
            "path": file_path
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_delete_file() {
        let temp_dir = tempdir().unwrap();
        let tool = DeleteFileTool::new(vec![temp_dir.path().to_path_buf()]);

        // Create a test file
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "Test content").unwrap();

        // Delete the file
        let request = ToolRequest::new(json!({
            "file_path": "test.txt"
        }), "test_id".to_string());
        let result = tool
            .execute(&request)
            .await
            .unwrap();

        match result {
            ToolResult::Success { context_data, .. } => {
                assert_eq!(context_data["success"], true);
                assert!(!test_file.exists());
            }
            _ => panic!("Expected Success variant"),
        }
    }

    #[tokio::test]
    async fn test_delete_empty_directory() {
        let temp_dir = tempdir().unwrap();
        let tool = DeleteFileTool::new(vec![temp_dir.path().to_path_buf()]);

        // Create an empty directory
        let test_dir = temp_dir.path().join("test_dir");
        fs::create_dir(&test_dir).unwrap();

        // Delete the directory
        let request = ToolRequest::new(json!({
            "file_path": "test_dir"
        }), "test_id".to_string());
        let result = tool
            .execute(&request)
            .await
            .unwrap();

        match result {
            ToolResult::Success { context_data, .. } => {
                assert_eq!(context_data["success"], true);
                assert!(!test_dir.exists());
            }
            _ => panic!("Expected Success variant"),
        }
    }

    #[tokio::test]
    async fn test_delete_nonexistent_file() {
        let temp_dir = tempdir().unwrap();
        let tool = DeleteFileTool::new(vec![temp_dir.path().to_path_buf()]);

        // Try to delete a file that doesn't exist
        let request = ToolRequest::new(json!({
            "file_path": "nonexistent.txt"
        }), "test_id".to_string());
        let result = tool
            .execute(&request)
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_missing_file_path() {
        let temp_dir = tempdir().unwrap();
        let tool = DeleteFileTool::new(vec![temp_dir.path().to_path_buf()]);

        let request = ToolRequest::new(json!({}), "test_id".to_string());
        let result = tool.execute(&request).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing required parameter: file_path"));
    }

    #[tokio::test]
    async fn test_delete_file_with_subdirectory() {
        let temp_dir = tempdir().unwrap();
        let tool = DeleteFileTool::new(vec![temp_dir.path().to_path_buf()]);

        // Create a subdirectory with a file
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();
        let test_file = sub_dir.join("test.txt");
        fs::write(&test_file, "Test content").unwrap();

        // Delete the file in the subdirectory
        let request = ToolRequest::new(json!({
            "file_path": "subdir/test.txt"
        }), "test_id".to_string());
        let result = tool
            .execute(&request)
            .await
            .unwrap();

        match result {
            ToolResult::Success { context_data, .. } => {
                assert_eq!(context_data["success"], true);
                assert!(!test_file.exists());
                assert!(sub_dir.exists()); // Directory should still exist
            }
            _ => panic!("Expected Success variant"),
        }
    }
}
