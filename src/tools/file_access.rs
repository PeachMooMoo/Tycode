use anyhow::{Context, Result};
use std::fs::File;
use std::path::{Path, PathBuf};

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

    pub fn open_read(&self, file_path: &str) -> Result<File> {
        let path = self.validate_path(file_path)?;

        if !path.exists() {
            anyhow::bail!("File not found: {}", file_path);
        }

        if !path.is_file() {
            anyhow::bail!("Path is not a file: {}", file_path);
        }

        let metadata = std::fs::metadata(&path)
            .with_context(|| format!("Failed to read metadata for: {}", file_path))?;

        if metadata.len() > self.max_file_size as u64 {
            anyhow::bail!(
                "File too large: {} bytes (max: {} bytes)",
                metadata.len(),
                self.max_file_size
            );
        }

        File::open(&path).with_context(|| format!("Failed to open file for reading: {}", file_path))
    }

    pub fn open_write(&self, file_path: &str) -> Result<File> {
        let path = self.validate_path(file_path)?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create parent directories for: {}", file_path)
            })?;
        }

        File::create(&path)
            .with_context(|| format!("Failed to create file for writing: {}", file_path))
    }

    pub fn list_directory(&self, directory_path: Option<&str>) -> Result<Vec<PathBuf>> {
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

        let read_dir = std::fs::read_dir(&dir_path)
            .with_context(|| format!("Failed to read directory: {}", dir_path.display()))?;

        let mut entries = Vec::new();
        for entry in read_dir {
            let entry = entry?;
            entries.push(entry.path());
        }

        Ok(entries)
    }

    pub fn workspace_root(&self) -> &PathBuf {
        &self.workspace_root
    }

    fn validate_path(&self, file_path: &str) -> Result<PathBuf> {
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
