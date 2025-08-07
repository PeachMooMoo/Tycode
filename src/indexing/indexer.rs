use crate::ai::provider::AiProvider;
use crate::indexing::index_structure::IndexMetadata;
use crate::indexing::{ChecksumManager, IndexAgent, IndexStructure};
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::SystemTime;
use tokio::fs;
use tracing::{debug, info, warn};

pub struct Indexer {
    workspace_root: PathBuf,
    checksum_manager: ChecksumManager,
    index_structure: IndexStructure,
    index_agent: IndexAgent,
}

impl Indexer {
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        let workspace_root = workspace_root.as_ref().to_path_buf();
        let checksum_manager = ChecksumManager::new(&workspace_root);
        let index_structure = IndexStructure::new(&workspace_root);
        let index_agent = IndexAgent::new();

        Self {
            workspace_root,
            checksum_manager,
            index_structure,
            index_agent,
        }
    }

    pub async fn rebuild_all_indexes(
        &mut self,
        provider: &dyn AiProvider,
    ) -> Result<IndexingStats> {
        self.rebuild_all_indexes_with_progress(provider, |_| {})
            .await
    }

    pub async fn rebuild_all_indexes_with_progress<F>(
        &mut self,
        provider: &dyn AiProvider,
        progress_callback: F,
    ) -> Result<IndexingStats>
    where
        F: Fn(&str) + Send + Sync,
    {
        info!("Starting full index rebuild");

        // Load existing checksum database
        self.checksum_manager.load().await?;

        let mut stats = IndexingStats::default();

        // Get all files in workspace (excluding .tycode directory)
        let files = self.collect_workspace_files().await?;

        info!("Found {} files to potentially index", files.len());

        // Process each file
        for (index, file_path) in files.iter().enumerate() {
            // Report progress
            let file_name = file_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            progress_callback(&format!(
                "{}/{} files - {}",
                index + 1,
                files.len(),
                file_name
            ));

            match self.process_file(provider, file_path, &mut stats).await {
                Ok(_) => {
                    debug!("Successfully processed: {:?}", file_path);
                }
                Err(e) => {
                    warn!("Failed to process file {:?}: {:?}", file_path, e);
                    stats.errors += 1;
                }
            }
        }

        // Process directories
        let directories = self.collect_workspace_directories().await?;
        for (index, dir_path) in directories.iter().enumerate() {
            // Report progress
            let dir_name = dir_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            progress_callback(&format!(
                "{}/{} directories - {}/",
                index + 1,
                directories.len(),
                dir_name
            ));

            match self.process_directory(provider, dir_path, &mut stats).await {
                Ok(_) => {
                    debug!("Successfully processed directory: {:?}", dir_path);
                }
                Err(e) => {
                    warn!("Failed to process directory {:?}: {:?}", dir_path, e);
                    stats.errors += 1;
                }
            }
        }

        // Save updated checksums
        self.checksum_manager.save().await?;

        info!("Index rebuild complete. Stats: {:?}", stats);
        Ok(stats)
    }

    async fn process_file(
        &mut self,
        provider: &dyn AiProvider,
        file_path: &Path,
        stats: &mut IndexingStats,
    ) -> Result<()> {
        // Skip if file hasn't changed
        if !self.checksum_manager.has_file_changed(file_path).await? {
            stats.unchanged += 1;
            return Ok(());
        }

        // Check if file is too large or binary
        if self.should_skip_file(file_path).await? {
            stats.skipped += 1;
            return Ok(());
        }

        // Read file content
        let content = match fs::read_to_string(file_path).await {
            Ok(content) => content,
            Err(_) => {
                // File might be binary, skip it
                stats.skipped += 1;
                return Ok(());
            }
        };

        // Generate outline (or skip if not source code)
        let outline = match self
            .index_agent
            .generate_file_outline(provider, file_path, &content)
            .await?
        {
            Some(outline) => outline,
            None => {
                // File was skipped (not source code)
                stats.skipped += 1;
                return Ok(());
            }
        };

        // Create metadata
        let checksum = self
            .checksum_manager
            .calculate_file_checksum(file_path)
            .await?;
        let file_type = self.determine_file_type(file_path);

        let metadata = IndexMetadata {
            source_file: file_path.to_path_buf(),
            checksum: checksum.clone(),
            generated_at: SystemTime::now(),
            file_type,
            is_directory: false,
        };

        // Save index file
        self.index_structure
            .create_index_file(file_path, &metadata, &outline)
            .await?;

        // Update checksum database
        self.checksum_manager
            .update_file_checksum(file_path)
            .await?;

        stats.updated += 1;
        Ok(())
    }

    async fn process_directory(
        &mut self,
        provider: &dyn AiProvider,
        dir_path: &Path,
        stats: &mut IndexingStats,
    ) -> Result<()> {
        // Get directory contents
        let mut entries = fs::read_dir(dir_path)
            .await
            .with_context(|| format!("Failed to read directory: {:?}", dir_path))?;

        let mut file_list = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let entry_path = entry.path();
            let name = entry_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            if entry_path.is_dir() {
                file_list.push(format!("{}/", name));
            } else {
                file_list.push(name);
            }
        }

        // Sort for consistent output
        file_list.sort();

        // Generate directory summary
        let summary = self
            .index_agent
            .generate_directory_summary(provider, dir_path, &file_list)
            .await?;

        // Create a fake checksum for directory based on its contents
        let dir_content = file_list.join("\n");
        let mut hasher = Sha256::new();
        hasher.update(dir_content.as_bytes());
        let checksum = format!("{:x}", hasher.finalize());

        let metadata = IndexMetadata {
            source_file: dir_path.to_path_buf(),
            checksum,
            generated_at: SystemTime::now(),
            file_type: "directory".to_string(),
            is_directory: true,
        };

        // Save directory index
        self.index_structure
            .create_index_file(dir_path, &metadata, &summary)
            .await?;

        stats.directories += 1;
        Ok(())
    }

    async fn collect_workspace_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        self.collect_files_recursive(&self.workspace_root, &mut files)
            .await?;
        Ok(files)
    }

    async fn collect_workspace_directories(&self) -> Result<Vec<PathBuf>> {
        let mut directories = Vec::new();
        self.collect_directories_recursive(&self.workspace_root, &mut directories)
            .await?;
        Ok(directories)
    }

    fn collect_files_recursive<'a>(
        &'a self,
        dir: &'a Path,
        files: &'a mut Vec<PathBuf>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let mut entries = fs::read_dir(dir)
                .await
                .with_context(|| format!("Failed to read directory: {:?}", dir))?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();

                // Skip .tycode directory and other common ignore patterns
                if self.should_skip_path(&path) {
                    continue;
                }

                if path.is_file() {
                    files.push(path);
                } else if path.is_dir() {
                    self.collect_files_recursive(&path, files).await?;
                }
            }

            Ok(())
        })
    }

    fn collect_directories_recursive<'a>(
        &'a self,
        dir: &'a Path,
        directories: &'a mut Vec<PathBuf>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let mut entries = fs::read_dir(dir)
                .await
                .with_context(|| format!("Failed to read directory: {:?}", dir))?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();

                // Skip .tycode directory and other common ignore patterns
                if self.should_skip_path(&path) {
                    continue;
                }

                if path.is_dir() {
                    directories.push(path.clone());
                    self.collect_directories_recursive(&path, directories)
                        .await?;
                }
            }

            Ok(())
        })
    }

    fn should_skip_path(&self, path: &Path) -> bool {
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Skip .tycode, .git, node_modules, target, etc.
        matches!(
            file_name,
            ".tycode"
                | ".git"
                | ".svn"
                | ".hg"
                | "node_modules"
                | "target"
                | "build"
                | "dist"
                | ".DS_Store"
                | "Thumbs.db"
        )
    }

    async fn should_skip_file(&self, file_path: &Path) -> Result<bool> {
        let metadata = fs::metadata(file_path).await?;

        // Skip very large files (>1MB)
        if metadata.len() > 1_000_000 {
            return Ok(true);
        }

        // Skip files with binary extensions
        if let Some(extension) = file_path.extension().and_then(|e| e.to_str()) {
            let binary_extensions = [
                "exe", "dll", "so", "dylib", "bin", "obj", "o", "jpg", "jpeg", "png", "gif", "bmp",
                "svg", "ico", "mp3", "mp4", "avi", "mov", "wav", "pdf", "zip", "tar", "gz", "rar",
                "7z", "wasm", "pdb",
            ];

            if binary_extensions.contains(&extension.to_lowercase().as_str()) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn determine_file_type(&self, file_path: &Path) -> String {
        file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("unknown")
            .to_string()
    }
}

#[derive(Debug, Default)]
pub struct IndexingStats {
    pub updated: usize,
    pub unchanged: usize,
    pub skipped: usize,
    pub directories: usize,
    pub errors: usize,
}

impl IndexingStats {
    pub fn total_processed(&self) -> usize {
        self.updated + self.unchanged + self.skipped
    }
}
