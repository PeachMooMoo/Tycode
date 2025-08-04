use crate::tools::file_access::FileAccessManager;
use crate::tools::r#trait::ToolExecutor;
use anyhow::Result;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

#[derive(Clone)]
pub struct ListFilesTool {
    file_access: FileAccessManager,
}

impl ListFilesTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            file_access: FileAccessManager::new(workspace_root),
        }
    }

    fn collect_entries_recursive(
        &self,
        dir_path: &PathBuf,
        entries: &mut Vec<Value>,
    ) -> Result<()> {
        let paths = self
            .file_access
            .list_directory(Some(&dir_path.to_string_lossy()))?;

        for path in paths {
            let metadata = fs::metadata(&path)?;
            let relative_path = path
                .strip_prefix(self.file_access.workspace_root())
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();

            entries.push(json!({
                "name": path.file_name().unwrap_or_default().to_string_lossy(),
                "path": relative_path,
                "type": if metadata.is_dir() { "directory" } else { "file" },
                "size": if metadata.is_file() { Some(metadata.len()) } else { None::<u64> }
            }));

            if metadata.is_dir() {
                self.collect_entries_recursive(&path, entries)?;
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
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
                    "description": "Path to directory to list (relative to workspace root). Defaults to workspace root if not specified."
                },
                "recursive": {
                    "type": "boolean",
                    "description": "Whether to list files recursively. Defaults to false."
                }
            },
            "required": []
        })
    }

    async fn execute(&self, arguments: &Value) -> Result<Value> {
        let directory_path = arguments.get("directory_path").and_then(|v| v.as_str());
        let recursive = arguments
            .get("recursive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut entries = Vec::new();

        if recursive {
            let start_path = match directory_path {
                Some(path) => self.file_access.workspace_root().join(path),
                None => self.file_access.workspace_root().clone(),
            };
            self.collect_entries_recursive(&start_path, &mut entries)?;
        } else {
            let paths = self.file_access.list_directory(directory_path)?;

            for path in paths {
                let metadata = fs::metadata(&path)?;
                let relative_path = path
                    .strip_prefix(self.file_access.workspace_root())
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();

                entries.push(json!({
                    "name": path.file_name().unwrap_or_default().to_string_lossy(),
                    "path": relative_path,
                    "type": if metadata.is_dir() { "directory" } else { "file" },
                    "size": if metadata.is_file() { Some(metadata.len()) } else { None::<u64> }
                }));
            }
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
            "recursive": recursive
        }))
    }
}
