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

    /// Constructs the path to the index file for the given file path
    fn get_index_path(&self, file_path: &str) -> PathBuf {
        let index_base = self
            .file_access
            .workspace_root()
            .join(".tycode")
            .join("index");

        // Add .md extension to the file path for the index
        let index_file = format!("{}.md", file_path);
        index_base.join(index_file)
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
                },
                "summary": {
                    "type": "boolean",
                    "description": "If true, return a summary of the file rather than the full file content. Use summaries to understand project structure and interfaces without needing to read full source files."
                }
            },
            "required": ["file_path", "summary"]
        })
    }

    async fn execute(&self, arguments: &Value) -> Result<Value> {
        let file_path = arguments
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: file_path"))?;

        let summary = arguments
            .get("summary")
            .and_then(|v| v.as_bool())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: summary"))?;

        if summary {
            // Try to read from the index
            let index_path = self.get_index_path(file_path);

            if index_path.exists() && index_path.is_file() {
                // Read the summary from the index
                let mut index_file = fs::File::open(&index_path)?;
                let mut summary_content = String::new();
                index_file.read_to_string(&mut summary_content)?;

                let metadata = fs::metadata(&index_path)?;

                return Ok(json!({
                    "content": summary_content,
                    "size": metadata.len(),
                    "path": file_path,
                    "is_summary": true
                }));
            }

            // Fall back to full file if no index exists
        }

        // Check if the path is a directory (for non-summary requests)
        let file_path_resolved = self.file_access.workspace_root().join(file_path);
        if file_path_resolved.is_dir() {
            // For directories, we can't read them as files, but we might have a summary
            if !summary {
                return Err(anyhow::anyhow!("Path is a directory, not a file: {}. Use summary=true to get directory summary if available.", file_path));
            } else {
                // No summary was found for this directory
                return Err(anyhow::anyhow!(
                    "No summary found for directory: {}",
                    file_path
                ));
            }
        }

        // Read the full file
        let mut file = self.file_access.open_read(file_path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;

        let metadata = fs::metadata(&file_path_resolved)?;

        Ok(json!({
            "content": content,
            "size": metadata.len(),
            "path": file_path,
            "is_summary": false
        }))
    }
}
