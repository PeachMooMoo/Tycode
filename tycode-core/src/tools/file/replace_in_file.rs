use crate::tools::file_access::FileAccessManager;
use crate::tools::r#trait::ToolExecutor;
use anyhow::Result;
use serde_json::{json, Value};
use std::path::PathBuf;

/// Tool for replacing sections of content in files
#[derive(Clone)]
pub struct ReplaceInFileTool {
    file_manager: FileAccessManager,
}

impl ReplaceInFileTool {
    pub fn new(workspace_roots: Vec<PathBuf>) -> Self {
        let file_manager = FileAccessManager::new(workspace_roots);
        Self { file_manager }
    }

    /// Parse the diff format to extract replacements
    fn parse_diff(&self, diff: &str) -> Result<Vec<(String, String)>> {
        let mut replacements = Vec::new();
        let blocks: Vec<&str> = diff.split("------- SEARCH").collect();

        for block in blocks.iter().skip(1) {
            // Skip the first empty block
            let parts: Vec<&str> = block.split("=======").collect();
            if parts.len() != 2 {
                return Err(anyhow::anyhow!(
                    "Invalid diff format: Each SEARCH block must have exactly one ======= separator"
                ));
            }

            let search_part = parts[0].trim();
            let replace_parts: Vec<&str> = parts[1].split("+++++++ REPLACE").collect();

            if replace_parts.len() != 2 {
                return Err(anyhow::anyhow!(
                    "Invalid diff format: Missing or invalid REPLACE marker"
                ));
            }

            let replace_part = replace_parts[0].trim();

            replacements.push((search_part.to_string(), replace_part.to_string()));
        }

        if replacements.is_empty() {
            return Err(anyhow::anyhow!("No SEARCH/REPLACE blocks found in diff"));
        }

        Ok(replacements)
    }

    /// Apply replacements to content
    fn apply_replacements(
        &self,
        content: &str,
        replacements: Vec<(String, String)>,
    ) -> Result<String> {
        let mut result = content.to_string();

        for (search, replace) in replacements {
            if !result.contains(&search) {
                return Err(anyhow::anyhow!(
                    "Search pattern not found in file:\n{}",
                    search
                ));
            }
            // Replace only the first occurrence as specified
            result = result.replacen(&search, &replace, 1);
        }

        Ok(result)
    }
}

#[async_trait::async_trait(?Send)]
impl ToolExecutor for ReplaceInFileTool {
    fn name(&self) -> &'static str {
        "replace_in_file"
    }

    fn description(&self) -> &'static str {
        "Replace sections of content in an existing file"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to modify"
                },
                "diff": {
                    "type": "string",
                    "description": "One or more SEARCH/REPLACE blocks following this exact format:\n------- SEARCH\n[exact content to find]\n=======\n[new content to replace with]\n+++++++ REPLACE\n\nCritical: Use exactly 7 dashes before SEARCH, 7 equals for separator, 7 pluses before REPLACE. Multiple blocks can be included in one diff. Example:\n------- SEARCH\nold line 1\nold line 2\n=======\nnew line 1\nnew line 2\n+++++++ REPLACE"
                }
            },
            "required": ["file_path", "diff"]
        })
    }

    async fn execute(&self, arguments: &Value) -> Result<Value> {
        let file_path = arguments
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: file_path"))?;

        let diff = arguments
            .get("diff")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: diff"))?;

        // Read the current content using FileAccessManager
        let original_content = self.file_manager.read_file(file_path).await?;

        // Parse and apply the diff
        let replacements = self.parse_diff(diff)?;
        let new_content = self.apply_replacements(&original_content, replacements)?;

        // Write the modified content back using FileAccessManager
        self.file_manager
            .write_file(file_path, &new_content)
            .await?;

        Ok(json!({
            "success": true,
            "path": file_path,
            "changes_applied": true,
            "original_content": original_content,
            "new_content": new_content
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_replace_in_file() {
        let temp_dir = TempDir::new().unwrap();
        let tool = ReplaceInFileTool::new(temp_dir.path().to_path_buf());

        // Create a test file using FileAccessManager
        let file_manager = FileAccessManager::new(temp_dir.path().to_path_buf());
        file_manager
            .write_file("test.txt", "Hello World\nThis is a test\nGoodbye")
            .await
            .unwrap();

        // Test replacement
        let diff = r#"------- SEARCH
Hello World
=======
Hello Universe
+++++++ REPLACE

------- SEARCH
Goodbye
=======
See you later
+++++++ REPLACE"#;

        let result = tool
            .execute(&json!({
                "file_path": "test.txt",
                "diff": diff
            }))
            .await
            .unwrap();

        assert_eq!(result["success"], true);

        // Verify the content was changed
        let new_content = file_manager.read_file("test.txt").await.unwrap();
        assert!(new_content.contains("Hello Universe"));
        assert!(new_content.contains("See you later"));
        assert!(!new_content.contains("Hello World"));
        assert!(!new_content.contains("Goodbye"));
    }
}
