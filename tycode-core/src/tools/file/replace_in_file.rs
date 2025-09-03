use crate::security::types::RiskLevel;
use crate::tools::file_access::FileAccessManager;
use crate::tools::r#trait::{ToolExecutor, ToolRequest, ToolResult};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;

/// Tool for replacing sections of content in files
#[derive(Clone, Deserialize)]
pub struct SearchReplaceBlock {
    pub search: String,
    pub replace: String,
}

#[derive(Clone)]
pub struct ReplaceInFileTool {
    file_manager: FileAccessManager,
}

impl ReplaceInFileTool {
    pub fn new(workspace_roots: Vec<PathBuf>) -> Self {
        let file_manager = FileAccessManager::new(workspace_roots);
        Self { file_manager }
    }

    /// Apply replacements to content
    fn apply_replacements(
        &self,
        content: &str,
        replacements: Vec<SearchReplaceBlock>,
    ) -> Result<String> {
        let mut result = content.to_string();

        for block in replacements {
            if !result.contains(&block.search) {
                return Err(anyhow::anyhow!(
                    "Search pattern not found in file:\n{}",
                    block.search
                ));
            }
            // Replace only the first occurrence as specified
            result = result.replacen(&block.search, &block.replace, 1);
        }

        Ok(result)
    }
}

#[async_trait::async_trait(?Send)]
impl ToolExecutor for ReplaceInFileTool {
    fn name(&self) -> &'static str {
        "replace_in_file"
    }

    fn description(&self) -> &'static str {
        "Replace sections of content in an existing file"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to modify. Note: File to modify must be tracked using the track_file tool before being modified. Search block in diff must exactly match the current file context from the context."
                },
                "diff": {
                    "type": "array",
                    "description": "Array of search and replace blocks",
                    "items": {
                        "type": "object",
                        "properties": {
                            "search": {
                                "type": "string",
                                "description": "Exact content to find"
                            },
                            "replace": {
                                "type": "string",
                                "description": "New content to replace with"
                            }
                        },
                        "required": ["search", "replace"]
                    }
                }
            },
            "required": ["file_path", "diff"]
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
        let file_path = request
            .arguments
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: file_path"))?;

        let diff_value = request
            .arguments
            .get("diff")
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: diff"))?;

        // Read the current content using FileAccessManager
        let original_content = self.file_manager.read_file(file_path).await?;

        // Parse and apply the diff
        let replacements: Vec<SearchReplaceBlock> = serde_json::from_value(diff_value.clone())?;
        let replacement_count = replacements.len();
        let new_content = self.apply_replacements(&original_content, replacements)?;

        // Write the modified content back using FileAccessManager
        self.file_manager
            .write_file(file_path, &new_content)
            .await?;

        // Context data: minimal information for the conversation
        let context_data = json!({
            "success": true,
            "path": file_path,
            "changes_applied": true,
            "replacements_made": replacement_count
        });

        // UI data: full content for diff display in VSCode
        let ui_data = json!({
            "path": file_path,
            "original_content": original_content,
            "new_content": new_content
        });

        Ok(ToolResult::with_ui(context_data, ui_data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_replace_in_file() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ReplaceInFileTool::new(vec![temp_dir.path().to_path_buf()]);

        // Create a test file using FileAccessManager
        let file_manager = FileAccessManager::new(vec![temp_dir.path().to_path_buf()]);
        file_manager
            .write_file("test.txt", "Hello World\nThis is a test\nGoodbye")
            .await
            .unwrap();

        // Test replacement
        let diff = json!([
            {"search": "Hello World", "replace": "Hello Universe"},
            {"search": "Goodbye", "replace": "See you later"}
        ]);

        let request = ToolRequest::new(
            json!({
                "file_path": "test.txt",
                "diff": diff
            }),
            "test_id".to_string(),
        );
        let result = tool.execute(&request).await.unwrap();

        match result {
            ToolResult::Success { context_data, .. } => {
                assert_eq!(context_data["success"], true);
            }
            _ => panic!("Expected Success variant"),
        }

        // Verify the content was changed
        let new_content = file_manager.read_file("test.txt").await.unwrap();
        assert!(new_content.contains("Hello Universe"));
        assert!(new_content.contains("See you later"));
        assert!(!new_content.contains("Hello World"));
        assert!(!new_content.contains("Goodbye"));
    }
}
