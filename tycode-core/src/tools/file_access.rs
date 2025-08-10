//! Direct file system operations for tools
//! No abstraction needed since we're pure Rust now

use anyhow::{Context, Result};
use regex::Regex;
use std::path::{Path, PathBuf};
use tokio::fs;
use walkdir::WalkDir;

#[derive(Clone)]
pub struct FileAccessManager {
    workspace_root: PathBuf,
    max_file_size: usize,
}

impl FileAccessManager {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
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

    pub async fn list_directory(&self, directory_path: Option<&str>) -> Result<Vec<PathBuf>> {
        let dir_path = match directory_path {
            Some(path) => self.validate_path(path)?,
            None => self.workspace_root.clone(),
        };

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
    ) -> Result<Vec<SearchResult>> {
        let dir_path = self.validate_path(directory_path)?;

        let regex =
            Regex::new(pattern).with_context(|| format!("Invalid regex pattern: {}", pattern))?;

        let file_regex = if let Some(fp) = file_pattern {
            Some(Regex::new(fp).with_context(|| format!("Invalid file pattern: {}", fp))?)
        } else {
            None
        };

        let mut results = Vec::new();

        // Walk through all files in directory
        for entry in WalkDir::new(&dir_path) {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();

            // Check if file matches file pattern if provided
            if let Some(ref file_regex) = file_regex {
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !file_regex.is_match(file_name) {
                    continue;
                }
            }

            // Read file and search for pattern
            if let Ok(content) = fs::read_to_string(path).await {
                for (line_number, line) in content.lines().enumerate() {
                    if regex.is_match(line) {
                        // Get context (2 lines before and after)
                        let lines: Vec<&str> = content.lines().collect();
                        let start = line_number.saturating_sub(2);
                        let end = (line_number + 3).min(lines.len());

                        let context_before = lines[start..line_number]
                            .iter()
                            .map(|s| s.to_string())
                            .collect();

                        let context_after = lines[(line_number + 1)..end]
                            .iter()
                            .map(|s| s.to_string())
                            .collect();

                        results.push(SearchResult {
                            path: path.to_path_buf(),
                            line_number: line_number + 1, // 1-indexed
                            line_content: line.to_string(),
                            context_before,
                            context_after,
                        });
                    }
                }
            }
        }

        Ok(results)
    }

    pub fn workspace_root(&self) -> &PathBuf {
        &self.workspace_root
    }

    pub fn validate_path(&self, file_path: &str) -> Result<PathBuf> {
        let path = Path::new(file_path);

        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.workspace_root.join(path)
        };

        let canonical_workspace = self.workspace_root.canonicalize().with_context(|| {
            format!(
                "Failed to canonicalize workspace root: {:?}",
                self.workspace_root
            )
        })?;

        if let Ok(canonical_path) = full_path.canonicalize() {
            if !canonical_path.starts_with(&canonical_workspace) {
                anyhow::bail!("Path is outside workspace root: {}", file_path);
            }
        } else {
            let parent_path = full_path.parent().unwrap_or(&full_path);
            if let Ok(canonical_parent) = parent_path.canonicalize() {
                if !canonical_parent.starts_with(&canonical_workspace) {
                    anyhow::bail!("Path is outside workspace root: {}", file_path);
                }
            }
        }

        Ok(full_path)
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub path: PathBuf,
    pub line_number: usize,
    pub line_content: String,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
}
