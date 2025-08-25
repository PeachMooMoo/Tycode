//! Direct file system operations for tools
//! No abstraction needed since we're pure Rust now

use crate::security::types::RiskLevel;
use anyhow::{Context, Result};
use regex::Regex;
use std::path::{Path, PathBuf};
use tokio::fs;
use walkdir::WalkDir;

#[derive(Clone)]
pub struct FileAccessManager {
    workspace_roots: Vec<PathBuf>,
    max_file_size: usize,
}

impl FileAccessManager {
    pub fn new(workspace_roots: Vec<PathBuf>) -> Self {
        Self {
            workspace_roots,
            max_file_size: 10 * 1024 * 1024, // 10MB
        }
    }

    pub async fn read_file(&self, file_path: &str) -> Result<String> {
        let path = self.validate_path(file_path)?;

        if !path.exists() {
            anyhow::bail!("File not found: {}", file_path);
        }

        if !path.is_file() {
            anyhow::bail!("Path is not a file: {}", file_path);
        }

        let metadata = fs::metadata(&path)
            .await
            .with_context(|| format!("Failed to read metadata for: {}", file_path))?;

        if metadata.len() > self.max_file_size as u64 {
            anyhow::bail!(
                "File too large: {} bytes (max: {} bytes)",
                metadata.len(),
                self.max_file_size
            );
        }

        fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read file: {}", file_path))
    }

    pub async fn write_file(&self, file_path: &str, content: &str) -> Result<()> {
        let path = self.validate_path(file_path)?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.with_context(|| {
                format!("Failed to create parent directories for: {}", file_path)
            })?;
        }

        fs::write(&path, content)
            .await
            .with_context(|| format!("Failed to write file: {}", file_path))
    }

    pub async fn delete_file(&self, file_path: &str) -> Result<()> {
        let path = self.validate_path(file_path)?;

        let metadata = fs::metadata(&path)
            .await
            .with_context(|| format!("Failed to get metadata for: {}", file_path))?;

        if metadata.is_dir() {
            fs::remove_dir(&path)
                .await
                .with_context(|| format!("Failed to delete directory: {}", file_path))?;
        } else {
            fs::remove_file(&path)
                .await
                .with_context(|| format!("Failed to delete file: {}", file_path))?;
        }

        Ok(())
    }

    pub async fn list_directory(&self, directory_path: &str) -> Result<Vec<PathBuf>> {
        let dir_path = self.validate_path(directory_path)?;

        if !dir_path.exists() {
            anyhow::bail!("Directory not found: {}", dir_path.display());
        }

        if !dir_path.is_dir() {
            anyhow::bail!("Path is not a directory: {}", dir_path.display());
        }

        let mut entries = fs::read_dir(&dir_path)
            .await
            .with_context(|| format!("Failed to read directory: {}", dir_path.display()))?;

        let mut paths = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            paths.push(entry.path());
        }

        Ok(paths)
    }

    pub async fn file_exists(&self, file_path: &str) -> Result<bool> {
        let path = self.validate_path(file_path)?;
        Ok(path.exists())
    }

    pub async fn search_files(
        &self,
        directory_path: &str,
        pattern: &str,
        file_pattern: Option<&str>,
        max_results: usize,
        include_context: bool,
        context_lines: usize,
    ) -> Result<(Vec<SearchResult>, bool)> {
        let dir_path = self.validate_path(directory_path)?;

        let regex =
            Regex::new(pattern).with_context(|| format!("Invalid regex pattern: {}", pattern))?;

        let file_regex = if let Some(fp) = file_pattern {
            Some(Regex::new(fp).with_context(|| format!("Invalid file pattern: {}", fp))?)
        } else {
            None
        };

        let mut results = Vec::new();
        let mut truncated = false;

        // Walk through all files in directory
        for entry in WalkDir::new(&dir_path) {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();

            // Skip if file doesn't match pattern
            if let Some(ref file_regex) = file_regex {
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !file_regex.is_match(file_name) {
                    continue;
                }
            }

            // Process matches in this file
            let found_truncated = self.search_in_file(
                path,
                &regex,
                include_context,
                context_lines,
                max_results,
                &mut results,
            ).await?;
            
            if found_truncated {
                truncated = true;
                break;
            }
        }

        Ok((results, truncated))
    }

    async fn search_in_file(
        &self,
        path: &Path,
        regex: &Regex,
        include_context: bool,
        context_lines: usize,
        max_results: usize,
        results: &mut Vec<SearchResult>,
    ) -> Result<bool> {
        // Read file content
        let content = match fs::read_to_string(path).await {
            Ok(c) => c,
            Err(_) => return Ok(false),
        };

        let lines: Vec<&str> = if include_context {
            content.lines().collect()
        } else {
            vec![]
        };

        for (line_number, line) in content.lines().enumerate() {
            if !regex.is_match(line) {
                continue;
            }

            // Check if we've hit the result limit
            if results.len() >= max_results {
                return Ok(true);
            }

            let (context_before, context_after) = if !include_context {
                (None, None)
            } else {
                let start = line_number.saturating_sub(context_lines);
                let end = (line_number + context_lines + 1).min(lines.len());

                let before = lines[start..line_number]
                    .iter()
                    .map(|s| s.to_string())
                    .collect();

                let after = lines[(line_number + 1)..end]
                    .iter()
                    .map(|s| s.to_string())
                    .collect();

                (Some(before), Some(after))
            };

            results.push(SearchResult {
                path: path.to_path_buf(),
                line_number: line_number + 1, // 1-indexed
                line_content: line.to_string(),
                context_before,
                context_after,
            });
        }

        Ok(false)
    }

    pub fn workspace_roots(&self) -> &[PathBuf] {
        &self.workspace_roots
    }
    
    /// Evaluate risk level for a file path operation
    pub fn evaluate_path_risk(&self, file_path: &str) -> RiskLevel {
        // Restricted directories that should never be modified
        const RESTRICTED_DIRS: &[&str] = &[".git", ".svn", ".hg", ".bzr"];
        
        let path = Path::new(file_path);
        
        // Check if path contains restricted directories
        for component in path.components() {
            if let Some(name) = component.as_os_str().to_str() {
                if RESTRICTED_DIRS.contains(&name) {
                    return RiskLevel::HighRisk;
                }
            }
        }
        
        // Check if path is absolute
        if path.is_absolute() {
            // Check if within any workspace root
            for workspace_root in &self.workspace_roots {
                if let Ok(canonical_workspace) = workspace_root.canonicalize() {
                    if let Ok(canonical_path) = path.canonicalize() {
                        if canonical_path.starts_with(&canonical_workspace) {
                            return RiskLevel::LowRisk;
                        }
                    } else {
                        // Path doesn't exist yet, check parent
                        if let Some(parent) = path.parent() {
                            if let Ok(canonical_parent) = parent.canonicalize() {
                                if canonical_parent.starts_with(&canonical_workspace) {
                                    return RiskLevel::LowRisk;
                                }
                            }
                        }
                    }
                }
            }
            // Absolute path outside workspace
            return RiskLevel::HighRisk;
        }
        
        // Relative path - will be resolved within workspace
        RiskLevel::LowRisk
    }

    pub fn validate_path(&self, file_path: &str) -> Result<PathBuf> {
        let path = Path::new(file_path);

        // If absolute path, check if it's within any workspace root
        if path.is_absolute() {
            for workspace_root in &self.workspace_roots {
                let canonical_workspace = workspace_root.canonicalize().with_context(|| {
                    format!(
                        "Failed to canonicalize workspace root: {:?}",
                        workspace_root
                    )
                })?;

                // First check if the file exists and is within workspace
                if path.exists() {
                    if let Ok(canonical_path) = path.canonicalize() {
                        if canonical_path.starts_with(&canonical_workspace) {
                            return Ok(path.to_path_buf());
                        }
                    }
                } else {
                    // For new files, check different strategies
                    // 1. Check if parent exists and is within workspace
                    if let Some(parent) = path.parent() {
                        if parent.exists() {
                            if let Ok(canonical_parent) = parent.canonicalize() {
                                if canonical_parent.starts_with(&canonical_workspace) {
                                    return Ok(path.to_path_buf());
                                }
                            }
                        } else {
                            // 2. Parent doesn't exist, check if path starts with workspace
                            // This handles new directories like /workspace/new_dir/new_file.txt
                            if path.starts_with(&canonical_workspace) {
                                return Ok(path.to_path_buf());
                            }
                        }
                    } else {
                        // No parent (root path?), check direct match
                        if path.starts_with(&canonical_workspace) {
                            return Ok(path.to_path_buf());
                        }
                    }
                }
            }
            anyhow::bail!("Path is outside all workspace roots: {}", file_path);
        }

        // For relative paths, try each workspace root
        for workspace_root in &self.workspace_roots {
            let full_path = workspace_root.join(path);
            
            // Check if file exists in this root
            if full_path.exists() {
                let canonical_workspace = workspace_root.canonicalize().with_context(|| {
                    format!(
                        "Failed to canonicalize workspace root: {:?}",
                        workspace_root
                    )
                })?;

                if let Ok(canonical_path) = full_path.canonicalize() {
                    if canonical_path.starts_with(&canonical_workspace) {
                        return Ok(full_path);
                    }
                }
            }
        }

        // File doesn't exist in any root, use first root for new files
        if let Some(first_root) = self.workspace_roots.first() {
            let full_path = first_root.join(path);
            let canonical_workspace = first_root.canonicalize().with_context(|| {
                format!("Failed to canonicalize workspace root: {:?}", first_root)
            })?;

            // Check parent directory for new files
            if let Some(parent) = full_path.parent() {
                if parent.exists() {
                    if let Ok(canonical_parent) = parent.canonicalize() {
                        if !canonical_parent.starts_with(&canonical_workspace) {
                            anyhow::bail!("Path is outside workspace root: {}", file_path);
                        }
                    }
                } else {
                    // Parent doesn't exist, just ensure the path doesn't escape
                    let mut check_path = first_root.clone();
                    for component in path.components() {
                        match component {
                            std::path::Component::ParentDir => {
                                check_path.pop();
                                // Make sure we haven't escaped the workspace
                                if !check_path.starts_with(first_root) {
                                    anyhow::bail!("Path traversal detected: {}", file_path);
                                }
                            }
                            std::path::Component::Normal(name) => {
                                check_path.push(name);
                            }
                            _ => {}
                        }
                    }
                }
            }

            Ok(full_path)
        } else {
            anyhow::bail!("No workspace roots configured");
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub path: PathBuf,
    pub line_number: usize,
    pub line_content: String,
    pub context_before: Option<Vec<String>>,
    pub context_after: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup_test_dir() -> (TempDir, FileAccessManager) {
        let temp_dir = TempDir::new().unwrap();
        let manager = FileAccessManager::new(vec![temp_dir.path().to_path_buf()]);
        (temp_dir, manager)
    }

    #[tokio::test]
    async fn test_search_files_default_no_context() {
        let (temp_dir, manager) = setup_test_dir().await;
        
        // Create test file
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "line 1\nmatch line\nline 3\nanother match\nline 5")
            .await
            .unwrap();

        let (results, truncated) = manager
            .search_files(
                temp_dir.path().to_str().unwrap(),
                "match",
                None,
                100,
                false,
                2,
            )
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert!(!truncated);
        
        // Verify no context included
        assert!(results[0].context_before.is_none());
        assert!(results[0].context_after.is_none());
        assert!(results[1].context_before.is_none());
        assert!(results[1].context_after.is_none());
    }

    #[tokio::test]
    async fn test_search_files_with_context() {
        let (temp_dir, manager) = setup_test_dir().await;
        
        // Create test file
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "line 1\nline 2\nmatch line\nline 4\nline 5")
            .await
            .unwrap();

        let (results, _) = manager
            .search_files(
                temp_dir.path().to_str().unwrap(),
                "match",
                None,
                100,
                true,
                2,
            )
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        
        // Verify context included
        let context_before = results[0].context_before.as_ref().unwrap();
        assert_eq!(context_before.len(), 2);
        assert_eq!(context_before[0], "line 1");
        assert_eq!(context_before[1], "line 2");
        
        let context_after = results[0].context_after.as_ref().unwrap();
        assert_eq!(context_after.len(), 2);
        assert_eq!(context_after[0], "line 4");
        assert_eq!(context_after[1], "line 5");
    }

    #[tokio::test]
    async fn test_search_files_max_results_limit() {
        let (temp_dir, manager) = setup_test_dir().await;
        
        // Create test file with many matches
        let mut content = String::new();
        for i in 0..200 {
            content.push_str(&format!("match line {}\n", i));
        }
        
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, content).await.unwrap();

        let (results, truncated) = manager
            .search_files(
                temp_dir.path().to_str().unwrap(),
                "match",
                None,
                50,
                false,
                2,
            )
            .await
            .unwrap();

        assert_eq!(results.len(), 50);
        assert!(truncated);
    }

    #[tokio::test]
    async fn test_search_files_default_limit() {
        let (temp_dir, manager) = setup_test_dir().await;
        
        // Create test file with many matches
        let mut content = String::new();
        for i in 0..150 {
            content.push_str(&format!("match line {}\n", i));
        }
        
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, content).await.unwrap();

        let (results, truncated) = manager
            .search_files(
                temp_dir.path().to_str().unwrap(),
                "match",
                None,
                100,
                false,
                2,
            )
            .await
            .unwrap();

        assert_eq!(results.len(), 100);
        assert!(truncated);
    }

    #[tokio::test]
    async fn test_search_files_file_pattern() {
        let (temp_dir, manager) = setup_test_dir().await;
        
        // Create test files with different extensions
        fs::write(temp_dir.path().join("test.rs"), "match in rust").await.unwrap();
        fs::write(temp_dir.path().join("test.txt"), "match in text").await.unwrap();
        fs::write(temp_dir.path().join("test.md"), "match in markdown").await.unwrap();

        let (results, _) = manager
            .search_files(
                temp_dir.path().to_str().unwrap(),
                "match",
                Some(r".*\.rs$"),
                100,
                false,
                2,
            )
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].path.to_str().unwrap().ends_with("test.rs"));
    }

    #[tokio::test]
    async fn test_search_files_custom_context_lines() {
        let (temp_dir, manager) = setup_test_dir().await;
        
        // Create test file
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "1\n2\n3\n4\nmatch\n6\n7\n8\n9")
            .await
            .unwrap();

        let (results, _) = manager
            .search_files(
                temp_dir.path().to_str().unwrap(),
                "match",
                None,
                100,
                true,
                3,
            )
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        
        let context_before = results[0].context_before.as_ref().unwrap();
        assert_eq!(context_before.len(), 3);
        assert_eq!(context_before[0], "2");
        assert_eq!(context_before[1], "3");
        assert_eq!(context_before[2], "4");
        
        let context_after = results[0].context_after.as_ref().unwrap();
        assert_eq!(context_after.len(), 3);
        assert_eq!(context_after[0], "6");
        assert_eq!(context_after[1], "7");
        assert_eq!(context_after[2], "8");
    }
}
