use crate::tools::file_access::FileAccessManager;
use crate::tools::r#trait::ToolExecutor;
use anyhow::Result;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

#[derive(Clone)]
pub struct SearchFilesTool {
    file_access: FileAccessManager,
}

impl SearchFilesTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            file_access: FileAccessManager::new(workspace_root),
        }
    }

    fn search_in_directory(
        &self,
        dir_path: &PathBuf,
        pattern: &str,
        file_extensions: &Option<Vec<String>>,
        matches: &mut Vec<Value>,
    ) -> Result<()> {
        let paths = self
            .file_access
            .list_directory(Some(&dir_path.to_string_lossy()))?;

        for path in paths {
            let metadata = fs::metadata(&path)?;

            if metadata.is_dir() {
                self.search_in_directory(&path, pattern, file_extensions, matches)?;
            } else if metadata.is_file() {
                if let Some(extensions) = file_extensions {
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        if !extensions.iter().any(|allowed| allowed == ext) {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }

                if let Ok(content) = fs::read_to_string(&path) {
                    let relative_path = path
                        .strip_prefix(self.file_access.workspace_root())
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string();

                    for (line_num, line) in content.lines().enumerate() {
                        if line.contains(pattern) {
                            matches.push(json!({
                                "file": relative_path,
                                "line_number": line_num + 1,
                                "line_content": line,
                                "match_positions": find_match_positions(line, pattern)
                            }));
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

fn find_match_positions(line: &str, pattern: &str) -> Vec<usize> {
    let mut positions = Vec::new();
    let mut start = 0;

    while let Some(pos) = line[start..].find(pattern) {
        positions.push(start + pos);
        start += pos + 1;
    }

    positions
}

#[async_trait::async_trait]
impl ToolExecutor for SearchFilesTool {
    fn name(&self) -> &'static str {
        "search_files"
    }

    fn description(&self) -> &'static str {
        "Search for text patterns across files in a directory"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "directory_path": {
                    "type": "string",
                    "description": "Directory to search in. Defaults to workspace root if not specified."
                },
                "pattern": {
                    "type": "string",
                    "description": "Text pattern to search for"
                },
                "file_extensions": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "Optional array of file extensions to limit search to (e.g., ['rs', 'py', 'js'])"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, arguments: &Value) -> Result<Value> {
        let directory_path = arguments.get("directory_path").and_then(|v| v.as_str());

        let pattern = arguments
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: pattern"))?;

        let file_extensions = arguments
            .get("file_extensions")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<String>>()
            });

        let mut matches = Vec::new();
        let search_path = match directory_path {
            Some(path) => self.file_access.workspace_root().join(path),
            None => self.file_access.workspace_root().clone(),
        };

        self.search_in_directory(&search_path, pattern, &file_extensions, &mut matches)?;

        Ok(json!({
            "matches": matches,
            "pattern": pattern,
            "directory": directory_path.unwrap_or("."),
            "total_matches": matches.len()
        }))
    }
}
