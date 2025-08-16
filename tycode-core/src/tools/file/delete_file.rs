use crate::tools::file_access::FileAccessManager;
use crate::tools::r#trait::ToolExecutor;
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

    async fn execute(&self, arguments: &Value) -> Result<Value> {
        let file_path = arguments
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: file_path"))?;

        // Use FileAccessManager for secure file deletion
        self.file_manager.delete_file(file_path).await?;

        Ok(json!({
            "success": true,
            "path": file_path
        }))
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
        let result = tool
            .execute(&json!({
                "file_path": "test.txt"
            }))
            .await
            .unwrap();

        assert_eq!(result["success"], true);
        assert!(!test_file.exists());
    }

    #[tokio::test]
    async fn test_delete_empty_directory() {
        let temp_dir = tempdir().unwrap();
        let tool = DeleteFileTool::new(vec![temp_dir.path().to_path_buf()]);

        // Create an empty directory
        let test_dir = temp_dir.path().join("test_dir");
        fs::create_dir(&test_dir).unwrap();

        // Delete the directory
        let result = tool
            .execute(&json!({
                "file_path": "test_dir"
            }))
            .await
            .unwrap();

        assert_eq!(result["success"], true);
        assert!(!test_dir.exists());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_file() {
        let temp_dir = tempdir().unwrap();
        let tool = DeleteFileTool::new(vec![temp_dir.path().to_path_buf()]);

        // Try to delete a file that doesn't exist
        let result = tool
            .execute(&json!({
                "file_path": "nonexistent.txt"
            }))
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_missing_file_path() {
        let temp_dir = tempdir().unwrap();
        let tool = DeleteFileTool::new(vec![temp_dir.path().to_path_buf()]);

        let result = tool.execute(&json!({})).await;

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
        let result = tool
            .execute(&json!({
                "file_path": "subdir/test.txt"
            }))
            .await
            .unwrap();

        assert_eq!(result["success"], true);
        assert!(!test_file.exists());
        assert!(sub_dir.exists()); // Directory should still exist
    }
}
