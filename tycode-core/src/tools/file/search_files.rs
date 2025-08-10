use crate::tools::file_access::FileAccessManager;
use crate::tools::r#trait::ToolExecutor;
use anyhow::Result;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Clone)]
pub struct SearchFilesTool {
    workspace_root: PathBuf,
    file_manager: FileAccessManager,
}

impl SearchFilesTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        let file_manager = FileAccessManager::new(workspace_root.clone());
        Self {
            workspace_root,
            file_manager,
        }
    }
}

#[async_trait::async_trait(?Send)]
impl ToolExecutor for SearchFilesTool {
    fn name(&self) -> &'static str {
        "search_files"
    }

    fn description(&self) -> &'static str {
        "Search for text patterns in files"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "directory_path": {
                    "type": "string",
                    "description": "Directory to search in"
                },
                "pattern": {
                    "type": "string",
                    "description": "Regular expression pattern to search for"
                },
                "file_pattern": {
                    "type": "string",
                    "description": "Optional file name pattern (e.g. '*.rs')"
                }
            },
            "required": ["directory_path", "pattern"]
        })
    }

    async fn execute(&self, arguments: &Value) -> Result<Value> {
        let directory_path = arguments
            .get("directory_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: directory_path"))?;

        let pattern = arguments
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: pattern"))?;

        let file_pattern = arguments.get("file_pattern").and_then(|v| v.as_str());

        // Use FileAccessManager for secure file searching
        let results = self
            .file_manager
            .search_files(directory_path, pattern, file_pattern)
            .await?;

        let mut json_results = Vec::new();
        for result in results {
            let relative_path = result
                .path
                .strip_prefix(&self.workspace_root)
                .unwrap_or(&result.path)
                .to_string_lossy()
                .to_string();

            json_results.push(json!({
                "path": relative_path,
                "line_number": result.line_number,
                "line": result.line_content,
                "context_before": result.context_before,
                "context_after": result.context_after,
            }));
        }

        Ok(json!({
            "results": json_results,
            "count": json_results.len(),
        }))
    }
}
