use crate::tools::file_access::FileAccessManager;
use crate::tools::r#trait::ToolExecutor;
use anyhow::Result;
use serde_json::{json, Value};
use std::fs;
use std::io::Read;
use std::path::PathBuf;

#[derive(Clone)]
pub struct ReadFileTool {
    file_access: FileAccessManager,
}

impl ReadFileTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            file_access: FileAccessManager::new(workspace_root),
        }
    }
}

#[async_trait::async_trait]
impl ToolExecutor for ReadFileTool {
    fn name(&self) -> &'static str {
        "read_file"
    }

    fn description(&self) -> &'static str {
        "Read the contents of a file"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to read (relative to workspace root)"
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

        let mut file = self.file_access.open_read(file_path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;

        let file_path_resolved = self.file_access.workspace_root().join(file_path);
        let metadata = fs::metadata(&file_path_resolved)?;

        Ok(json!({
            "content": content,
            "size": metadata.len(),
            "path": file_path
        }))
    }
}
