use crate::security::types::RiskLevel;
use crate::tools::file_access::FileAccessManager;
use crate::tools::r#trait::{ToolExecutor, ToolRequest, ToolResult};
use anyhow::Result;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Clone)]
pub struct SearchFilesTool {
    workspace_roots: Vec<PathBuf>,
    file_manager: FileAccessManager,
}

impl SearchFilesTool {
    pub fn new(workspace_roots: Vec<PathBuf>) -> Self {
        let file_manager = FileAccessManager::new(workspace_roots.clone());
        Self {
            workspace_roots,
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
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results to return (default: 100)"
                },
                "include_context": {
                    "type": "boolean",
                    "description": "Include context lines before/after matches (default: false)"
                },
                "context_lines": {
                    "type": "integer",
                    "description": "Number of context lines to include when include_context is true (default: 2)"
                }
            },
            "required": ["directory_path", "pattern"]
        })
    }

    fn evaluate_risk(&self, _arguments: &Value) -> RiskLevel {
        RiskLevel::ReadOnly
    }

    async fn execute(&self, request: &ToolRequest) -> Result<ToolResult> {
        let directory_path = request.arguments
            .get("directory_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: directory_path"))?;

        let pattern = request.arguments
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: pattern"))?;

        let file_pattern = request.arguments.get("file_pattern").and_then(|v| v.as_str());
        
        let max_results = request.arguments
            .get("max_results")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        
        let include_context = request.arguments
            .get("include_context")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        
        let context_lines = request.arguments
            .get("context_lines")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);

        // Use FileAccessManager for secure file searching
        let (results, truncated) = self
            .file_manager
            .search_files(directory_path, pattern, file_pattern, max_results, include_context, context_lines)
            .await?;

        let mut json_results = Vec::new();
        for result in results {
            // Find which workspace root this path belongs to
            let relative_path = self.workspace_roots.iter()
                .find_map(|root| {
                    result.path
                        .strip_prefix(root)
                        .ok()
                        .map(|rel| rel.to_string_lossy().to_string())
                })
                .unwrap_or_else(|| result.path.to_string_lossy().to_string());

            let mut result_obj = json!({
                "path": relative_path,
                "line_number": result.line_number,
                "line": result.line_content,
            });
            
            // Only include context if present
            if let Some(context_before) = result.context_before {
                result_obj["context_before"] = json!(context_before);
            }
            if let Some(context_after) = result.context_after {
                result_obj["context_after"] = json!(context_after);
            }

            json_results.push(result_obj);
        }

        let mut response = json!({
            "results": json_results,
            "count": json_results.len(),
        });
        
        if truncated {
            response["truncated"] = json!(true);
            response["message"] = json!("Results truncated to limit");
        }

        Ok(ToolResult::context_only(response))
    }
}
