use crate::security::types::RiskLevel;
use crate::tools::file_access::FileAccessManager;
use crate::tools::r#trait::{ToolExecutor, ToolRequest, ToolResult};
use anyhow::Result;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Clone)]
pub struct WriteFileTool {
    file_manager: FileAccessManager,
}

impl WriteFileTool {
    pub fn new(workspace_roots: Vec<PathBuf>) -> Self {
        let file_manager = FileAccessManager::new(workspace_roots);
        Self { file_manager }
    }
}

#[async_trait::async_trait(?Send)]
impl ToolExecutor for WriteFileTool {
    fn name(&self) -> &'static str {
        "write_file"
    }

    fn description(&self) -> &'static str {
        "Create a new file or completely overwrite an existing file"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path where the file should be created"
                },
                "content": {
                    "type": "string",
                    "description": "Complete content to write to the file"
                }
            },
            "required": ["file_path", "content"]
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

        let content = request.arguments
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: content. Sometimes this can happen if you hit a token limit; try writing a smaller file"))?;

        // Try to read original content if file exists
        let original_content = self.file_manager.read_file(file_path).await.unwrap_or_default();
        let file_exists = !original_content.is_empty();

        // Use FileAccessManager for secure file writing
        self.file_manager.write_file(file_path, content).await?;

        // Context data: minimal information for the conversation
        let context_data = json!({
            "success": true,
            "path": file_path,
            "bytes_written": content.len(),
            "created": !file_exists,
            "updated": file_exists
        });

        // UI data: full content for diff display in VSCode
        let ui_data = json!({
            "path": file_path,
            "original_content": original_content,
            "new_content": content
        });

        Ok(ToolResult::with_ui(context_data, ui_data))
    }
}
