//! Desktop platform implementation using standard file system operations

use super::{Platform, SearchResult};
use anyhow::{Context, Result};
use async_trait::async_trait;
use regex::Regex;
use std::path::{Path, PathBuf};
use tokio::fs;
use walkdir::WalkDir;

/// Desktop platform implementation using tokio::fs and std::fs
pub struct DesktopPlatform;

impl DesktopPlatform {
    pub fn new() -> Self {
        DesktopPlatform
    }
}

#[async_trait]
impl Platform for DesktopPlatform {
    async fn read_file(&self, path: &Path) -> Result<String> {
        fs::read_to_string(path)
            .await
            .with_context(|| format!("Failed to read file: {}", path.display()))
    }

    async fn write_file(&self, path: &Path, content: &str) -> Result<()> {
        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.with_context(|| {
                format!(
                    "Failed to create parent directories for: {}",
                    path.display()
                )
            })?;
        }

        fs::write(path, content)
            .await
            .with_context(|| format!("Failed to write file: {}", path.display()))
    }

    async fn delete_file(&self, path: &Path) -> Result<()> {
        fs::remove_file(path)
            .await
            .with_context(|| format!("Failed to delete file: {}", path.display()))
    }

    async fn list_files(&self, path: &Path, recursive: bool) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        if recursive {
            // Use walkdir for recursive listing
            for entry in WalkDir::new(path) {
                let entry = entry?;
                if entry.file_type().is_file() {
                    files.push(entry.path().to_path_buf());
                }
            }
        } else {
            // Use tokio::fs for non-recursive listing
            let mut entries = fs::read_dir(path)
                .await
                .with_context(|| format!("Failed to read directory: {}", path.display()))?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                let metadata = entry.metadata().await?;
                if metadata.is_file() {
                    files.push(path);
                }
            }
        }

        Ok(files)
    }

    async fn file_exists(&self, path: &Path) -> Result<bool> {
        Ok(path.exists())
    }

    async fn create_dir_all(&self, path: &Path) -> Result<()> {
        fs::create_dir_all(path)
            .await
            .with_context(|| format!("Failed to create directories: {}", path.display()))
    }

    async fn search_files(
        &self,
        directory: &Path,
        pattern: &str,
        file_pattern: Option<&str>,
    ) -> Result<Vec<SearchResult>> {
        let regex =
            Regex::new(pattern).with_context(|| format!("Invalid regex pattern: {}", pattern))?;

        let file_regex = if let Some(fp) = file_pattern {
            Some(Regex::new(fp).with_context(|| format!("Invalid file pattern: {}", fp))?)
        } else {
            None
        };

        let mut results = Vec::new();

        // Walk through all files in directory
        for entry in WalkDir::new(directory) {
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
}
