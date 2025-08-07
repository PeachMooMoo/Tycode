use crate::tools::file_access::FileAccessManager;
use crate::tools::r#trait::ToolExecutor;
use anyhow::{Context, Result};
use serde_json::{json, Value};
use similar::{ChangeTag, TextDiff};
use std::io::{Read, Write};
use std::path::PathBuf;

#[derive(Clone)]
pub struct ApplyPatchTool {
    file_access: FileAccessManager,
}

impl ApplyPatchTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            file_access: FileAccessManager::new(workspace_root),
        }
    }

    fn apply_unified_diff(&self, original: &str, patch: &str) -> Result<String> {
        let mut result_lines: Vec<String> = original.lines().map(|s| s.to_string()).collect();
        let mut patch_lines = patch.lines().peekable();
        let mut offset = 0i32;

        while let Some(line) = patch_lines.next() {
            if line.starts_with("@@") {
                let (start_line, _line_count) = self.parse_hunk_header(line)?;
                let mut current_line = if start_line == 0 { 0 } else { start_line - 1 };
                let mut result_pos = (current_line + offset).max(0) as usize;

                while let Some(&next_line) = patch_lines.peek() {
                    if next_line.starts_with("@@")
                        || (!next_line.starts_with(' ')
                            && !next_line.starts_with('-')
                            && !next_line.starts_with('+'))
                    {
                        break;
                    }

                    let line = patch_lines.next().unwrap();
                    match line.chars().next() {
                        Some(' ') => {
                            let expected_content = &line[1..];
                            if result_pos >= result_lines.len() || result_lines[result_pos] != expected_content {
                                anyhow::bail!("Context line mismatch at position {}: expected '{}', found '{}'", 
                                    result_pos, expected_content, 
                                    result_lines.get(result_pos).unwrap_or(&"<end of file>".to_string()));
                            }
                            current_line += 1;
                            result_pos += 1;
                        }
                        Some('-') => {
                            let expected_content = &line[1..];
                            if result_pos >= result_lines.len() || result_lines[result_pos] != expected_content {
                                anyhow::bail!("Remove line mismatch at position {}: expected '{}', found '{}'", 
                                    result_pos, expected_content, 
                                    result_lines.get(result_pos).unwrap_or(&"<end of file>".to_string()));
                            }
                            result_lines.remove(result_pos);
                            offset -= 1;
                            current_line += 1;
                        }
                        Some('+') => {
                            let content = &line[1..];
                            if result_pos <= result_lines.len() {
                                result_lines.insert(result_pos, content.to_string());
                                offset += 1;
                                result_pos += 1;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(result_lines.join("\n"))
    }

    fn parse_hunk_header(&self, header: &str) -> Result<(i32, i32)> {
        let parts: Vec<&str> = header.split_whitespace().collect();
        if parts.len() < 2 {
            anyhow::bail!("Invalid hunk header: {}", header);
        }

        let old_range = parts[1];
        if !old_range.starts_with('-') {
            anyhow::bail!("Invalid old range in hunk header: {}", header);
        }

        let range_part = &old_range[1..];
        let (start, count) = if let Some(comma_pos) = range_part.find(',') {
            let start: i32 = range_part[..comma_pos].parse().with_context(|| {
                format!("Invalid start line number: {}", &range_part[..comma_pos])
            })?;
            let count: i32 = range_part[comma_pos + 1..]
                .parse()
                .with_context(|| format!("Invalid line count: {}", &range_part[comma_pos + 1..]))?;
            (start, count)
        } else {
            let start: i32 = range_part
                .parse()
                .with_context(|| format!("Invalid line number: {}", range_part))?;
            (start, 1)
        };

        Ok((start, count))
    }
}

#[async_trait::async_trait]
impl ToolExecutor for ApplyPatchTool {
    fn name(&self) -> &'static str {
        "apply_patch"
    }

    fn description(&self) -> &'static str {
        "Apply a unified diff patch to modify a file incrementally"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to modify"
                },
                "patch": {
                    "type": "string",
                    "description": "Unified diff format patch to apply"
                },
                "dry_run": {
                    "type": "boolean",
                    "description": "If true, preview the changes without applying them. Defaults to false."
                }
            },
            "required": ["file_path", "patch"]
        })
    }

    async fn execute(&self, arguments: &Value) -> Result<Value> {
        let file_path = arguments
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: file_path"))?;

        let patch = arguments
            .get("patch")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: patch"))?;

        let dry_run = arguments
            .get("dry_run")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut file = self.file_access.open_read(file_path)?;
        let mut original_content = String::new();
        file.read_to_string(&mut original_content)?;

        let patched_content = self.apply_unified_diff(&original_content, patch)?;

        if dry_run {
            let diff = TextDiff::from_lines(&original_content, &patched_content);
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
            write_file.write_all(patched_content.as_bytes())?;

            let diff = TextDiff::from_lines(&original_content, &patched_content);
            let changes_count = diff
                .iter_all_changes()
                .filter(|c| c.tag() != ChangeTag::Equal)
                .count();

            Ok(json!({
                "success": true,
                "path": file_path,
                "changes_applied": changes_count,
                "original_lines": original_content.lines().count(),
                "new_lines": patched_content.lines().count()
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_tool() -> (ApplyPatchTool, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let tool = ApplyPatchTool::new(temp_dir.path().to_path_buf());
        (tool, temp_dir)
    }

    #[test]
    fn test_simple_line_replacement() {
        let (tool, _temp_dir) = create_test_tool();
        let original = "def hello():\n    print(\"Hello\")\n    return True";
        let patch = r#"--- test.py
+++ test.py
@@ -1,3 +1,3 @@
 def hello():
-    print("Hello")
+    print("Hello World!")
     return True"#;

        let result = tool.apply_unified_diff(original, patch).unwrap();
        let expected = "def hello():\n    print(\"Hello World!\")\n    return True";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_line_insertion() {
        let (tool, _temp_dir) = create_test_tool();
        let original = "def hello():\n    print(\"Hello\")\n    return True";
        let patch = r#"--- test.py
+++ test.py
@@ -1,3 +1,4 @@
 def hello():
     print("Hello")
+    print("This is a test")
     return True"#;

        let result = tool.apply_unified_diff(original, patch).unwrap();
        let expected =
            "def hello():\n    print(\"Hello\")\n    print(\"This is a test\")\n    return True";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_line_deletion() {
        let (tool, _temp_dir) = create_test_tool();
        let original =
            "def hello():\n    print(\"Hello\")\n    print(\"Extra line\")\n    return True";
        let patch = r#"--- test.py
+++ test.py
@@ -1,4 +1,3 @@
 def hello():
     print("Hello")
-    print("Extra line")
     return True"#;

        let result = tool.apply_unified_diff(original, patch).unwrap();
        let expected = "def hello():\n    print(\"Hello\")\n    return True";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_multiple_changes_in_one_hunk() {
        let (tool, _temp_dir) = create_test_tool();
        let original = "def hello():\n    print(\"Hello\")\n    return True\n\ndef goodbye():\n    print(\"Goodbye\")\n    return False";
        let patch = r#"--- test.py
+++ test.py
@@ -1,7 +1,8 @@
 def hello():
-    print("Hello")
+    print("Hello World!")
+    print("This is a test")
     return True

 def goodbye():
     print("Goodbye")
     return False"#;

        let result = tool.apply_unified_diff(original, patch).unwrap();
        let expected = "def hello():\n    print(\"Hello World!\")\n    print(\"This is a test\")\n    return True\n\ndef goodbye():\n    print(\"Goodbye\")\n    return False";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_multiple_hunks() {
        let (tool, _temp_dir) = create_test_tool();
        let original = "line1\nline2\nline3\nline4\nline5\nline6";
        let patch = r#"--- test.txt
+++ test.txt
@@ -1,3 +1,3 @@
 line1
-line2
+modified line2
 line3
@@ -4,3 +4,3 @@
 line4
-line5
+modified line5
 line6"#;

        let result = tool.apply_unified_diff(original, patch).unwrap();
        let expected = "line1\nmodified line2\nline3\nline4\nmodified line5\nline6";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_hunk_header_with_count() {
        let (tool, _temp_dir) = create_test_tool();
        let result = tool.parse_hunk_header("@@ -1,7 +1,8 @@").unwrap();
        assert_eq!(result, (1, 7));
    }

    #[test]
    fn test_parse_hunk_header_without_count() {
        let (tool, _temp_dir) = create_test_tool();
        let result = tool.parse_hunk_header("@@ -5 +5,2 @@").unwrap();
        assert_eq!(result, (5, 1));
    }

    #[test]
    fn test_parse_hunk_header_invalid() {
        let (tool, _temp_dir) = create_test_tool();
        let result = tool.parse_hunk_header("invalid header");
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_patch() {
        let (tool, _temp_dir) = create_test_tool();
        let original = "line1\nline2\nline3";
        let patch = "";
        let result = tool.apply_unified_diff(original, patch).unwrap();
        assert_eq!(result, original);
    }

    #[test]
    fn test_context_only_patch() {
        let (tool, _temp_dir) = create_test_tool();
        let original = "line1\nline2\nline3";
        let patch = r#"--- test.txt
+++ test.txt
@@ -1,3 +1,3 @@
 line1
 line2
 line3"#;
        let result = tool.apply_unified_diff(original, patch).unwrap();
        assert_eq!(result, original);
    }

    #[test]
    fn test_insertion_at_beginning() {
        let (tool, _temp_dir) = create_test_tool();
        let original = "line2\nline3";
        let patch = r#"--- test.txt
+++ test.txt
@@ -1,2 +1,3 @@
+line1
 line2
 line3"#;
        let result = tool.apply_unified_diff(original, patch).unwrap();
        let expected = "line1\nline2\nline3";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_insertion_at_end() {
        let (tool, _temp_dir) = create_test_tool();
        let original = "line1\nline2";
        let patch = r#"--- test.txt
+++ test.txt
@@ -1,2 +1,3 @@
 line1
 line2
+line3"#;
        let result = tool.apply_unified_diff(original, patch).unwrap();
        let expected = "line1\nline2\nline3";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_empty_file_patch() {
        let (tool, _temp_dir) = create_test_tool();
        let original = "";
        let patch = r#"--- test.txt
+++ test.txt
@@ -0,0 +1,3 @@
+line1
+line2
+line3"#;
        let result = tool.apply_unified_diff(original, patch).unwrap();
        let expected = "line1\nline2\nline3";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_invalid_patch_format() {
        let (tool, _temp_dir) = create_test_tool();
        let original = "line1\nline2\nline3";
        let patch = "this is not a valid patch format";
        let result = tool.apply_unified_diff(original, patch).unwrap();
        assert_eq!(result, original);
    }

    #[test]
    fn test_patch_fails_when_remove_line_doesnt_match() {
        let (tool, _temp_dir) = create_test_tool();
        let original = "def hello():\n    print(\"Hello\")\n    return True";
        let patch = r#"--- test.py
+++ test.py
@@ -1,3 +1,3 @@
 def hello():
-    print("Goodbye")
+    print("Hello World!")
     return True"#;

        let result = tool.apply_unified_diff(original, patch);
        assert!(
            result.is_err(),
            "Patch should fail when remove line doesn't match actual content"
        );
    }

    #[test]
    fn test_patch_fails_when_context_line_doesnt_match() {
        let (tool, _temp_dir) = create_test_tool();
        let original = "def hello():\n    print(\"Hello\")\n    return True";
        let patch = r#"--- test.py
+++ test.py
@@ -1,3 +1,3 @@
 def goodbye():
-    print("Hello")
+    print("Hello World!")
     return True"#;

        let result = tool.apply_unified_diff(original, patch);
        assert!(
            result.is_err(),
            "Patch should fail when context line doesn't match actual content"
        );
    }

    #[test]
    fn test_patch_fails_when_removing_nonexistent_line() {
        let (tool, _temp_dir) = create_test_tool();
        let original = "line1\nline2";
        let patch = r#"--- test.txt
+++ test.txt
@@ -1,3 +1,2 @@
 line1
 line2
-line3"#;

        let result = tool.apply_unified_diff(original, patch);
        assert!(
            result.is_err(),
            "Patch should fail when trying to remove line that doesn't exist"
        );
    }

    #[test]
    fn test_patch_fails_with_wrong_line_position() {
        let (tool, _temp_dir) = create_test_tool();
        let original = "line1\nline2\nline3\nline4";
        let patch = r#"--- test.txt
+++ test.txt
@@ -2,2 +2,2 @@
-line3
+modified line3
 line4"#;

        let result = tool.apply_unified_diff(original, patch);
        assert!(
            result.is_err(),
            "Patch should fail when line position doesn't match expected content"
        );
    }

    #[test]
    fn test_patch_fails_with_mismatched_whitespace() {
        let (tool, _temp_dir) = create_test_tool();
        let original = "def hello():\n    print(\"Hello\")\n    return True";
        let patch = r#"--- test.py
+++ test.py
@@ -1,3 +1,3 @@
 def hello():
-  print("Hello")
+    print("Hello World!")
     return True"#;

        let result = tool.apply_unified_diff(original, patch);
        assert!(
            result.is_err(),
            "Patch should fail when whitespace doesn't match exactly"
        );
    }
}
