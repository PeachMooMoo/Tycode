use crate::chat::state::SharedChatState;
use crate::tools::file_access::FileAccessManager;
use crate::tools::r#trait::ToolExecutor;
use anyhow::Result;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct SetTrackedFilesTool {
    file_manager: FileAccessManager,
    chat_state: Arc<SharedChatState>,
}

impl SetTrackedFilesTool {
    pub fn new(workspace_roots: Vec<PathBuf>, chat_state: Arc<SharedChatState>) -> Self {
        let file_manager = FileAccessManager::new(workspace_roots);
        Self {
            file_manager,
            chat_state,
        }
    }
}

#[async_trait::async_trait(?Send)]
impl ToolExecutor for SetTrackedFilesTool {
    fn name(&self) -> &'static str {
        "set_tracked_files"
    }

    fn description(&self) -> &'static str {
        "Set the complete list of files to track for inclusion in all future messages. This replaces any previously tracked files. Minimize tracked files to conserve context. Pass an empty array to clear all tracked files."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_paths": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "Array of file paths to track. Empty array clears all tracked files."
                }
            },
            "required": ["file_paths"]
        })
    }

    async fn execute(&self, arguments: &Value) -> Result<Value> {
        let file_paths = arguments
            .get("file_paths")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: file_paths"))?;

        let mut new_paths = Vec::new();
        let mut invalid_files = Vec::new();

        // Validate all files exist
        for path_value in file_paths {
            if let Some(path_str) = path_value.as_str() {
                if self.file_manager.file_exists(path_str).await? {
                    new_paths.push(PathBuf::from(path_str));
                } else {
                    invalid_files.push(path_str.to_string());
                }
            }
        }

        if !invalid_files.is_empty() {
            return Err(anyhow::anyhow!(
                "The following files do not exist: {:?}",
                invalid_files
            ));
        }

        // Clear existing and set new tracked files
        self.chat_state.set_tracked_files(new_paths.clone());

        // Calculate total context size
        let mut total_size = 0usize;
        for path in &new_paths {
            let path_str = path.to_string_lossy();
            if let Ok(content) = self.file_manager.read_file(&path_str).await {
                total_size += content.len();
            }
        }

        Ok(json!({
            "success": true,
            "tracked_files": new_paths.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>(),
            "tracked_files_count": new_paths.len(),
            "total_context_size_bytes": total_size,
            "message": if new_paths.is_empty() {
                "Cleared all tracked files".to_string()
            } else {
                format!("Now tracking {} file(s). Context size: {} bytes", new_paths.len(), total_size)
            }
        }))
    }
}
