use crate::tools::file_access::FileAccessManager;
use crate::tools::r#trait::ToolExecutor;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

#[derive(Clone)]
pub struct DeleteFileTool {
    file_access: FileAccessManager,
}

impl DeleteFileTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            file_access: FileAccessManager::new(workspace_root),
        }
    }
}

#[async_trait::async_trait]
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
                    "description": "Path to the file or empty directory to delete"
                }
            },
            "required": ["file_path"]
        })
    }

    async fn execute(&self, arguments: &Value) -> Result<Value> {
        let file_path = arguments
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'file_path' argument"))?;

        let full_path = self.file_access.validate_path(file_path)?;

        let relative_path = full_path
            .strip_prefix(self.file_access.workspace_root())
            .unwrap_or(&full_path)
            .to_string_lossy()
            .to_string();

        if !full_path.exists() {
            return Err(anyhow!(
                "File or directory does not exist: {}",
                relative_path
            ));
        }

        let metadata = fs::metadata(&full_path)?;

        if metadata.is_dir() {
            if fs::read_dir(&full_path)?.next().is_some() {
                return Err(anyhow!(
                    "Cannot delete non-empty directory: {}",
                    relative_path
                ));
            }
            fs::remove_dir(&full_path)?;
        } else {
            fs::remove_file(&full_path)?;
        }

        Ok(json!({
            "success": true,
            "deleted_path": relative_path,
            "was_directory": metadata.is_dir()
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
        let tool = DeleteFileTool::new(temp_dir.path().to_path_buf());

        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "test content").unwrap();

        let args = json!({
            "file_path": "test.txt"
        });

        let result = tool.execute(&args).await.unwrap();
        assert_eq!(result["success"], true);
        assert_eq!(result["deleted_path"], "test.txt");
        assert_eq!(result["was_directory"], false);
        assert!(!test_file.exists());
    }

    #[tokio::test]
    async fn test_delete_empty_directory() {
        let temp_dir = tempdir().unwrap();
        let tool = DeleteFileTool::new(temp_dir.path().to_path_buf());

        let test_dir = temp_dir.path().join("empty_dir");
        fs::create_dir(&test_dir).unwrap();

        let args = json!({
            "file_path": "empty_dir"
        });

        let result = tool.execute(&args).await.unwrap();
        assert_eq!(result["success"], true);
        assert_eq!(result["deleted_path"], "empty_dir");
        assert_eq!(result["was_directory"], true);
        assert!(!test_dir.exists());
    }

    #[tokio::test]
    async fn test_delete_non_empty_directory_fails() {
        let temp_dir = tempdir().unwrap();
        let tool = DeleteFileTool::new(temp_dir.path().to_path_buf());

        let test_dir = temp_dir.path().join("non_empty_dir");
        fs::create_dir(&test_dir).unwrap();
        fs::write(test_dir.join("file.txt"), "content").unwrap();

        let args = json!({
            "file_path": "non_empty_dir"
        });

        let result = tool.execute(&args).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("non-empty directory"));
        assert!(test_dir.exists());
    }

    #[tokio::test]
    async fn test_delete_non_existent_file() {
        let temp_dir = tempdir().unwrap();
        let tool = DeleteFileTool::new(temp_dir.path().to_path_buf());

        let args = json!({
            "file_path": "non_existent.txt"
        });

        let result = tool.execute(&args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[tokio::test]
    async fn test_delete_outside_workspace() {
        let temp_dir = tempdir().unwrap();
        let tool = DeleteFileTool::new(temp_dir.path().to_path_buf());

        let args = json!({
            "file_path": "../outside.txt"
        });

        let result = tool.execute(&args).await;
        assert!(result.is_err());
    }
}
