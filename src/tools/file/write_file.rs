use crate::tools::file_access::FileAccessManager;
use crate::tools::r#trait::ToolExecutor;
use anyhow::Result;
use serde_json::{json, Value};
use std::io::Write;
use std::path::PathBuf;

#[derive(Clone)]
pub struct WriteFileTool {
    file_access: FileAccessManager,
}

impl WriteFileTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            file_access: FileAccessManager::new(workspace_root),
        }
    }
}

#[async_trait::async_trait]
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
                    "description": "Path where the file should be created (relative to workspace root)"
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
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: content"))?;

        let mut file = self.file_access.open_write(file_path)?;
        file.write_all(content.as_bytes())?;

        Ok(json!({
            "success": true,
            "path": file_path,
            "bytes_written": content.len()
        }))
    }
}
