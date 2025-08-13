use crate::tools::file_access::FileAccessManager;
use crate::tools::r#trait::ToolExecutor;
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

    async fn execute(&self, arguments: &Value) -> Result<Value> {
        let file_path = arguments
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: file_path"))?;

        let content = arguments
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: content. Sometimes this can happen if you hit a token limit; try writing a smaller file"))?;

        // Use FileAccessManager for secure file writing
        self.file_manager.write_file(file_path, content).await?;

        Ok(json!({
            "success": true,
            "path": file_path,
            "bytes_written": content.len()
        }))
    }
}
