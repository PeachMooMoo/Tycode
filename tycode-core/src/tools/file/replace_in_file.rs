use crate::tools::file_access::FileAccessManager;
use crate::tools::r#trait::ToolExecutor;
use anyhow::Result;
use serde_json::{json, Value};
use similar::{ChangeTag, TextDiff};
use std::io::{Read, Write};
use std::path::PathBuf;

#[derive(Clone)]
pub struct ReplaceInFileTool {
    file_access: FileAccessManager,
}

impl ReplaceInFileTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            file_access: FileAccessManager::new(workspace_root),
        }
    }

    fn apply_multiple_replacements(
        &self,
        original: &str,
        replacements: &[SearchReplaceBlock],
    ) -> Result<String> {
        let mut content = original.to_string();

        for (i, block) in replacements.iter().enumerate() {
            let search_normalized = block.search.replace("\r\n", "\n").replace('\r', "\n");
            let content_normalized = content.replace("\r\n", "\n").replace('\r', "\n");

            if !content_normalized.contains(&search_normalized) {
                anyhow::bail!(
                    "Search text not found in replacement block {} (1-indexed):\n{}",
                    i + 1,
                    block.search
                );
            }

            content = content_normalized.replacen(&search_normalized, &block.replace, 1);
        }

        Ok(content)
    }
}

#[derive(Debug, Clone)]
struct SearchReplaceBlock {
    search: String,
    replace: String,
}

impl SearchReplaceBlock {
    fn parse_from_diff(diff_text: &str) -> Result<Vec<Self>> {
        let mut blocks = Vec::new();
        let lines: Vec<&str> = diff_text.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            if lines[i].trim() == "------- SEARCH" {
                let search_start = i + 1;
                let mut search_end = search_start;

                while search_end < lines.len() && lines[search_end].trim() != "=======" {
                    search_end += 1;
                }

                if search_end >= lines.len() {
                    anyhow::bail!(
                        "Missing '=======' separator after SEARCH block starting at line {}",
                        i + 1
                    );
                }

                let replace_start = search_end + 1;
                let mut replace_end = replace_start;

                while replace_end < lines.len() && lines[replace_end].trim() != "+++++++ REPLACE" {
                    replace_end += 1;
                }

                if replace_end >= lines.len() {
                    anyhow::bail!("Missing '+++++++ REPLACE' separator after replacement content starting at line {}", replace_start + 1);
                }

                let search_lines = &lines[search_start..search_end];
                let replace_lines = &lines[replace_start..replace_end];

                let search = search_lines.join("\n");
                let replace = replace_lines.join("\n");

                blocks.push(SearchReplaceBlock { search, replace });

                i = replace_end + 1;
            } else {
                i += 1;
            }
        }

        if blocks.is_empty() {
            anyhow::bail!("No valid SEARCH/REPLACE blocks found in diff");
        }

        Ok(blocks)
    }
}

#[async_trait::async_trait]
impl ToolExecutor for ReplaceInFileTool {
    fn name(&self) -> &'static str {
        "replace_in_file"
    }

    fn description(&self) -> &'static str {
        "Replace specific text sections in a file using search and replace blocks. More reliable than patches as it doesn't depend on line numbers."
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
                    "description": "SEARCH/REPLACE blocks in the format:\n------- SEARCH\n[exact text to find]\n=======\n[replacement text]\n+++++++ REPLACE\n\nCan contain multiple such blocks for multiple replacements."
                },
                "dry_run": {
                    "type": "boolean",
                    "description": "If true, preview the changes without applying them. Defaults to false."
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
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: diff. Sometimes this can happen if you hit a token limit; try writing a smaller diff"))?;

        let dry_run = arguments
            .get("dry_run")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut file = self.file_access.open_read(file_path)?;
        let mut original_content = String::new();
        file.read_to_string(&mut original_content)?;

        let blocks = SearchReplaceBlock::parse_from_diff(diff)?;
        let new_content = self.apply_multiple_replacements(&original_content, &blocks)?;

        if dry_run {
            let diff = TextDiff::from_lines(&original_content, &new_content);
            let mut preview = Vec::new();

            for change in diff.iter_all_changes() {
                let sign = match change.tag() {
                    ChangeTag::Delete => "-",
                    ChangeTag::Insert => "+",
                    ChangeTag::Equal => " ",
                };
                preview.push(format!("{}{}", sign, change));
            }

            Ok(json!({
                "dry_run": true,
                "preview": preview.join(""),
                "changes_count": diff.iter_all_changes().filter(|c| c.tag() != ChangeTag::Equal).count()
            }))
        } else {
            let mut write_file = self.file_access.open_write(file_path)?;
            write_file.write_all(new_content.as_bytes())?;

            let diff = TextDiff::from_lines(&original_content, &new_content);
            let changes_count = diff
                .iter_all_changes()
                .filter(|c| c.tag() != ChangeTag::Equal)
                .count();

            Ok(json!({
                "success": true,
                "path": file_path,
                "changes_applied": changes_count,
                "original_lines": original_content.lines().count(),
                "new_lines": new_content.lines().count()
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_tool() -> (ReplaceInFileTool, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let tool = ReplaceInFileTool::new(temp_dir.path().to_path_buf());
        (tool, temp_dir)
    }

    #[test]
    fn test_parse_single_search_replace_block() {
        let diff = r#"------- SEARCH
print("Hello")
=======
print("Hello World!")
+++++++ REPLACE"#;

        let blocks = SearchReplaceBlock::parse_from_diff(diff).unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].search, "print(\"Hello\")");
        assert_eq!(blocks[0].replace, "print(\"Hello World!\")");
    }

    #[test]
    fn test_parse_multiple_search_replace_blocks() {
        let diff = r#"------- SEARCH
print("Hello")
=======
print("Hello World!")
+++++++ REPLACE

------- SEARCH
return True
=======
return False
+++++++ REPLACE"#;

        let blocks = SearchReplaceBlock::parse_from_diff(diff).unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].search, "print(\"Hello\")");
        assert_eq!(blocks[0].replace, "print(\"Hello World!\")");
        assert_eq!(blocks[1].search, "return True");
        assert_eq!(blocks[1].replace, "return False");
    }

    #[test]
    fn test_parse_multiline_search_replace() {
        let diff = r#"------- SEARCH
def hello():
    print("Hello")
=======
def hello():
    print("Hello World!")
    print("This is a test")
+++++++ REPLACE"#;

        let blocks = SearchReplaceBlock::parse_from_diff(diff).unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].search, "def hello():\n    print(\"Hello\")");
        assert_eq!(
            blocks[0].replace,
            "def hello():\n    print(\"Hello World!\")\n    print(\"This is a test\")"
        );
    }

    #[test]
    fn test_apply_multiple_replacements() {
        let (tool, _temp_dir) = create_test_tool();
        let original = "def hello():\n    print(\"Hello\")\n    return True";
        let blocks = vec![
            SearchReplaceBlock {
                search: "print(\"Hello\")".to_string(),
                replace: "print(\"Hello World!\")".to_string(),
            },
            SearchReplaceBlock {
                search: "return True".to_string(),
                replace: "return False".to_string(),
            },
        ];

        let result = tool.apply_multiple_replacements(original, &blocks).unwrap();
        let expected = "def hello():\n    print(\"Hello World!\")\n    return False";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_invalid_diff_missing_separator() {
        let diff = r#"------- SEARCH
print("Hello")
print("Hello World!")
+++++++ REPLACE"#;

        let result = SearchReplaceBlock::parse_from_diff(diff);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing '=======' separator"));
    }

    #[test]
    fn test_parse_invalid_diff_missing_replace() {
        let diff = r#"------- SEARCH
print("Hello")
=======
print("Hello World!")"#;

        let result = SearchReplaceBlock::parse_from_diff(diff);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing '+++++++ REPLACE' separator"));
    }

    #[test]
    fn test_deletion_with_empty_replace() {
        let (tool, _temp_dir) = create_test_tool();
        let original =
            "def hello():\n    print(\"Hello\")\n    print(\"Extra line\")\n    return True";
        let blocks = vec![SearchReplaceBlock {
            search: "    print(\"Extra line\")\n".to_string(),
            replace: "".to_string(),
        }];

        let result = tool.apply_multiple_replacements(original, &blocks).unwrap();
        let expected = "def hello():\n    print(\"Hello\")\n    return True";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_replacement_fails_when_search_text_not_found() {
        let (tool, _temp_dir) = create_test_tool();
        let original = "def hello():\n    print(\"Hello\")\n    return True";
        let blocks = vec![SearchReplaceBlock {
            search: "print(\"Goodbye\")".to_string(),
            replace: "print(\"Hello World!\")".to_string(),
        }];

        let result = tool.apply_multiple_replacements(original, &blocks);
        assert!(
            result.is_err(),
            "Replacement should fail when search text is not found"
        );
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Search text not found"));
    }

    #[test]
    fn test_replacement_fails_with_mismatched_whitespace() {
        let (tool, _temp_dir) = create_test_tool();
        let original = "def hello():\n    print(\"Hello\")\n    return True";
        let blocks = vec![SearchReplaceBlock {
            search: "def hello():\n  print(\"Hello\")".to_string(),
            replace: "def hello():\n    print(\"Hello World!\")".to_string(),
        }];

        let result = tool.apply_multiple_replacements(original, &blocks);
        assert!(
            result.is_err(),
            "Replacement should fail when whitespace doesn't match exactly"
        );
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Search text not found"));
    }

    #[test]
    fn test_replacement_fails_with_partial_match() {
        let (tool, _temp_dir) = create_test_tool();
        let original = "def hello():\n    print(\"Hello World\")\n    return True";
        let blocks = vec![SearchReplaceBlock {
            search: "print(\"Hello\")".to_string(),
            replace: "print(\"Goodbye\")".to_string(),
        }];

        let result = tool.apply_multiple_replacements(original, &blocks);
        assert!(
            result.is_err(),
            "Replacement should fail when search text only partially matches"
        );
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Search text not found"));
    }

    #[test]
    fn test_replacement_fails_with_wrong_line_endings() {
        let (tool, _temp_dir) = create_test_tool();
        let original = "def hello():\n    print(\"Hello\")\n    return True";
        let blocks = vec![SearchReplaceBlock {
            search: "def hello():\r\n    print(\"Hello\")".to_string(),
            replace: "def hello():\n    print(\"Hello World!\")".to_string(),
        }];

        let result = tool.apply_multiple_replacements(original, &blocks);
        assert!(
            result.is_ok(),
            "Line ending normalization should handle this case"
        );
    }

    #[test]
    fn test_replacement_fails_with_case_mismatch() {
        let (tool, _temp_dir) = create_test_tool();
        let original = "def hello():\n    print(\"Hello\")\n    return True";
        let blocks = vec![SearchReplaceBlock {
            search: "print(\"hello\")".to_string(),
            replace: "print(\"Hello World!\")".to_string(),
        }];

        let result = tool.apply_multiple_replacements(original, &blocks);
        assert!(
            result.is_err(),
            "Replacement should fail when case doesn't match exactly"
        );
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Search text not found"));
    }

    #[test]
    fn test_replacement_fails_with_multiline_mismatch() {
        let (tool, _temp_dir) = create_test_tool();
        let original = "def hello():\n    print(\"Hello\")\n    return True";
        let blocks = vec![SearchReplaceBlock {
            search: "def hello():\n    print(\"Goodbye\")\n    return True".to_string(),
            replace: "def hello():\n    print(\"Hello World!\")\n    return False".to_string(),
        }];

        let result = tool.apply_multiple_replacements(original, &blocks);
        assert!(
            result.is_err(),
            "Replacement should fail when multiline search doesn't match exactly"
        );
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Search text not found"));
    }

    #[test]
    fn test_multiple_replacements_fail_on_first_mismatch() {
        let (tool, _temp_dir) = create_test_tool();
        let original = "def hello():\n    print(\"Hello\")\n    return True";
        let blocks = vec![
            SearchReplaceBlock {
                search: "print(\"Hello\")".to_string(),
                replace: "print(\"Hello World!\")".to_string(),
            },
            SearchReplaceBlock {
                search: "return False".to_string(),
                replace: "return True".to_string(),
            },
        ];

        let result = tool.apply_multiple_replacements(original, &blocks);
        assert!(
            result.is_err(),
            "Multiple replacements should fail on first mismatch"
        );
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Search text not found in replacement block 2"));
    }
}
