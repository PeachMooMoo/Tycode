use crate::tools::file_access::FileAccessManager;
use crate::tools::r#trait::ToolExecutor;
use anyhow::Result;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Clone)]
pub struct ReadFileTool {
    workspace_root: PathBuf,
    file_manager: FileAccessManager,
}

impl ReadFileTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        let file_manager = FileAccessManager::new(workspace_root.clone());
        Self {
            workspace_root,
            file_manager,
        }
    }

    /// Constructs the path to the index file for the given file path
    fn get_index_path(&self, file_path: &str) -> PathBuf {
        let index_base = self.workspace_root.join(".tycode").join("index");

        // Add .md extension to the file path for the index
        let index_file = format!("{}.md", file_path);
        index_base.join(index_file)
    }
}

#[async_trait::async_trait(?Send)]
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
                    "description": "Path to the file to read"
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
            let index_path_str = index_path
                .strip_prefix(&self.workspace_root)
                .unwrap_or(&index_path)
                .to_string_lossy()
                .to_string();

            if self
                .file_manager
                .file_exists(&index_path_str)
                .await
                .unwrap_or(false)
            {
                // Read the summary from the index
                let summary_content = self.file_manager.read_file(&index_path_str).await?;

                return Ok(json!({
                    "content": summary_content,
                    "size": summary_content.len(),
                    "path": file_path,
                    "is_summary": true
                }));
            }

            // Fall back to full file if no index exists
        }

        // Check if the path is a directory (for non-summary requests)
        // Try to list directory to check if it's a directory
        if self
            .file_manager
            .list_directory(Some(file_path))
            .await
            .is_ok()
        {
            // It's a directory
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
        let content = self.file_manager.read_file(file_path).await?;

        Ok(json!({
            "content": content,
            "size": content.len(),
            "path": file_path,
            "is_summary": false
        }))
    }
}
