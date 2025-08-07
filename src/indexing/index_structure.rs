use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::SystemTime;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMetadata {
    pub source_file: PathBuf,
    pub checksum: String,
    pub generated_at: SystemTime,
    pub file_type: String,
    pub is_directory: bool,
}

pub struct IndexStructure {
    workspace_root: PathBuf,
    index_root: PathBuf,
}

impl IndexStructure {
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        let workspace_root = workspace_root.as_ref().to_path_buf();
        let index_root = workspace_root.join(".tycode").join("index");

        Self {
            workspace_root,
            index_root,
        }
    }

    pub fn get_index_path(&self, source_path: impl AsRef<Path>) -> PathBuf {
        let source_path = source_path.as_ref();

        // Make path relative to workspace root if it's absolute
        let relative_path = if source_path.is_absolute() {
            source_path
                .strip_prefix(&self.workspace_root)
                .unwrap_or(source_path)
        } else {
            source_path
        };

        // Create the corresponding index path with .md extension
        let mut index_path = self.index_root.join(relative_path);

        if !index_path.extension().map_or(false, |ext| ext == "md") {
            // Add .md extension if it doesn't have one
            if let Some(file_name) = index_path.file_name() {
                let new_name = format!("{}.md", file_name.to_string_lossy());
                index_path.set_file_name(new_name);
            }
        }

        index_path
    }

    pub async fn create_index_file(
        &self,
        source_path: impl AsRef<Path>,
        metadata: &IndexMetadata,
        content: &str,
    ) -> Result<()> {
        let index_path = self.get_index_path(&source_path);

        // Ensure parent directory exists
        if let Some(parent) = index_path.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create index directory structure")?;
        }

        // Create markdown content with frontmatter
        let frontmatter =
            serde_yaml::to_string(metadata).context("Failed to serialize metadata")?;

        let full_content = format!("---\n{}---\n\n{}", frontmatter, content);

        fs::write(&index_path, full_content)
            .await
            .with_context(|| format!("Failed to write index file: {:?}", index_path))?;

        Ok(())
    }

    pub async fn read_index_file(
        &self,
        source_path: impl AsRef<Path>,
    ) -> Result<Option<(IndexMetadata, String)>> {
        let index_path = self.get_index_path(&source_path);

        if !index_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&index_path)
            .await
            .with_context(|| format!("Failed to read index file: {:?}", index_path))?;

        self.parse_index_content(&content)
    }

    pub fn parse_index_content(&self, content: &str) -> Result<Option<(IndexMetadata, String)>> {
        if !content.starts_with("---\n") {
            // No frontmatter, treat as legacy file
            return Ok(None);
        }

        let parts: Vec<&str> = content.splitn(3, "---\n").collect();
        if parts.len() < 3 {
            return Ok(None);
        }

        let frontmatter = parts[1];
        let body = parts[2];

        let metadata: IndexMetadata =
            serde_yaml::from_str(frontmatter).context("Failed to parse index metadata")?;

        Ok(Some((metadata, body.to_string())))
    }

    pub async fn index_exists(&self, source_path: impl AsRef<Path>) -> bool {
        self.get_index_path(source_path).exists()
    }

    pub async fn delete_index(&self, source_path: impl AsRef<Path>) -> Result<()> {
        let index_path = self.get_index_path(source_path);
        if index_path.exists() {
            fs::remove_file(&index_path)
                .await
                .with_context(|| format!("Failed to delete index file: {:?}", index_path))?;
        }
        Ok(())
    }

    pub async fn list_all_indexes(&self) -> Result<Vec<PathBuf>> {
        let mut indexes = Vec::new();

        if !self.index_root.exists() {
            return Ok(indexes);
        }

        self.collect_index_files(&self.index_root, &mut indexes)
            .await?;
        Ok(indexes)
    }

    fn collect_index_files<'a>(
        &'a self,
        dir: &'a Path,
        indexes: &'a mut Vec<PathBuf>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let mut entries = fs::read_dir(dir)
                .await
                .with_context(|| format!("Failed to read directory: {:?}", dir))?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();

                if path.is_dir() {
                    self.collect_index_files(&path, indexes).await?;
                } else if path.extension().map_or(false, |ext| ext == "md") {
                    indexes.push(path);
                }
            }

            Ok(())
        })
    }

    pub fn get_source_path_from_index(&self, index_path: impl AsRef<Path>) -> Result<PathBuf> {
        let index_path = index_path.as_ref();

        // Get relative path from index root
        let relative_index = index_path
            .strip_prefix(&self.index_root)
            .context("Index path is not within index root")?;

        // Remove .md extension to get source path
        let mut source_relative = relative_index.to_path_buf();
        if source_relative.extension().map_or(false, |ext| ext == "md") {
            source_relative.set_extension("");
        }

        Ok(self.workspace_root.join(source_relative))
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn index_root(&self) -> &Path {
        &self.index_root
    }
}
