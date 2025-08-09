//! Platform abstraction layer for file system operations
//!
//! This module provides a trait-based abstraction over platform-specific
//! file operations, allowing the same code to work on desktop, WASM, and
//! other platforms.

use anyhow::Result;
use async_trait::async_trait;
use std::path::{Path, PathBuf};

/// Platform trait defining file system operations needed by tools
#[async_trait]
pub trait Platform: Send + Sync {
    /// Read the contents of a file as a string
    async fn read_file(&self, path: &Path) -> Result<String>;

    /// Write content to a file, creating parent directories if needed
    async fn write_file(&self, path: &Path, content: &str) -> Result<()>;

    /// Delete a file
    async fn delete_file(&self, path: &Path) -> Result<()>;

    /// List files in a directory
    /// If recursive is true, lists all files recursively
    async fn list_files(&self, path: &Path, recursive: bool) -> Result<Vec<PathBuf>>;

    /// Check if a file exists
    async fn file_exists(&self, path: &Path) -> Result<bool>;

    /// Create a directory and all parent directories
    async fn create_dir_all(&self, path: &Path) -> Result<()>;

    /// Search for patterns in files
    async fn search_files(
        &self,
        directory: &Path,
        pattern: &str,
        file_pattern: Option<&str>,
    ) -> Result<Vec<SearchResult>>;
}

/// Result from searching files
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub path: PathBuf,
    pub line_number: usize,
    pub line_content: String,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
}

/// No-op implementation that returns errors for all operations
/// Used as default when no platform features are enabled
pub struct NoOpPlatform;

#[async_trait]
impl Platform for NoOpPlatform {
    async fn read_file(&self, _path: &Path) -> Result<String> {
        Err(anyhow::anyhow!(
            "File operations not available on this platform"
        ))
    }

    async fn write_file(&self, _path: &Path, _content: &str) -> Result<()> {
        Err(anyhow::anyhow!(
            "File operations not available on this platform"
        ))
    }

    async fn delete_file(&self, _path: &Path) -> Result<()> {
        Err(anyhow::anyhow!(
            "File operations not available on this platform"
        ))
    }

    async fn list_files(&self, _path: &Path, _recursive: bool) -> Result<Vec<PathBuf>> {
        Err(anyhow::anyhow!(
            "File operations not available on this platform"
        ))
    }

    async fn file_exists(&self, _path: &Path) -> Result<bool> {
        Ok(false)
    }

    async fn create_dir_all(&self, _path: &Path) -> Result<()> {
        Err(anyhow::anyhow!(
            "File operations not available on this platform"
        ))
    }

    async fn search_files(
        &self,
        _directory: &Path,
        _pattern: &str,
        _file_pattern: Option<&str>,
    ) -> Result<Vec<SearchResult>> {
        Err(anyhow::anyhow!(
            "File operations not available on this platform"
        ))
    }
}

// Desktop platform implementation
#[cfg(feature = "desktop")]
pub mod desktop;
#[cfg(feature = "desktop")]
pub use desktop::DesktopPlatform;

// Export a default platform based on features
pub fn default_platform() -> Box<dyn Platform> {
    #[cfg(feature = "desktop")]
    {
        Box::new(DesktopPlatform::new())
    }
    #[cfg(not(feature = "desktop"))]
    {
        Box::new(NoOpPlatform)
    }
}
