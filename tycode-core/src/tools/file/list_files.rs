use crate::tools::file_access::FileAccessManager;
use crate::tools::r#trait::ToolExecutor;
use anyhow::Result;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Clone)]
pub struct ListFilesTool {
    workspace_root: PathBuf,
    file_manager: FileAccessManager,
}

impl ListFilesTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        let file_manager = FileAccessManager::new(workspace_root.clone());
        Self {
            workspace_root,
            file_manager,
        }
    }
}

#[async_trait::async_trait(?Send)]
impl ToolExecutor for ListFilesTool {
    fn name(&self) -> &'static str {
        "list_files"
    }

    fn description(&self) -> &'static str {
        "List files and directories in a directory"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "directory_path": {
                    "type": "string",
                    "description": "Path to directory to list. Defaults to workspace root if not specified."
                },
            },
            "required": []
        })
    }

    async fn execute(&self, arguments: &Value) -> Result<Value> {
        let directory_path = arguments.get("directory_path").and_then(|v| v.as_str());

        // Use FileAccessManager for secure directory listing
        let paths = self.file_manager.list_directory(directory_path).await?;

        let mut entries = Vec::new();
        for path in paths {
            let relative_path = path
                .strip_prefix(&self.workspace_root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();

            // Check if it's a directory by trying to list it
            let relative_str = path
                .strip_prefix(&self.workspace_root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            let is_dir = self
                .file_manager
                .list_directory(Some(&relative_str))
                .await
                .is_ok();

            entries.push(json!({
                "name": path.file_name().unwrap_or_default().to_string_lossy(),
                "path": relative_path,
                "type": if is_dir { "directory" } else { "file" },
            }));
        }

        entries.sort_by(|a, b| {
            let a_type = a["type"].as_str().unwrap_or("");
            let b_type = b["type"].as_str().unwrap_or("");
            let a_name = a["name"].as_str().unwrap_or("");
            let b_name = b["name"].as_str().unwrap_or("");

            match (a_type, b_type) {
                ("directory", "file") => std::cmp::Ordering::Less,
                ("file", "directory") => std::cmp::Ordering::Greater,
                _ => a_name.cmp(b_name),
            }
        });

        Ok(json!({
            "entries": entries,
            "path": directory_path.unwrap_or("."),
        }))
    }
}
