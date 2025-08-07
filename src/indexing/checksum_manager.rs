use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChecksum {
    pub path: PathBuf,
    pub checksum: String,
    pub last_modified: SystemTime,
    pub size: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ChecksumDatabase {
    pub files: HashMap<PathBuf, FileChecksum>,
}

pub struct ChecksumManager {
    db_path: PathBuf,
    database: ChecksumDatabase,
}

impl ChecksumManager {
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        let db_path = workspace_root.as_ref().join(".tycode").join("checksums.json");
        Self {
            db_path,
            database: ChecksumDatabase::default(),
        }
    }

    pub async fn load(&mut self) -> Result<()> {
        if self.db_path.exists() {
            let content = fs::read_to_string(&self.db_path).await
                .context("Failed to read checksum database")?;
            self.database = serde_json::from_str(&content)
                .context("Failed to parse checksum database")?;
        }
        Ok(())
    }

    pub async fn save(&self) -> Result<()> {
        if let Some(parent) = self.db_path.parent() {
            fs::create_dir_all(parent).await
                .context("Failed to create .tycode directory")?;
        }

        let content = serde_json::to_string_pretty(&self.database)
            .context("Failed to serialize checksum database")?;
        
        fs::write(&self.db_path, content).await
            .context("Failed to write checksum database")?;
        
        Ok(())
    }

    pub async fn calculate_file_checksum(&self, file_path: impl AsRef<Path>) -> Result<String> {
        let content = fs::read(&file_path).await
            .with_context(|| format!("Failed to read file: {:?}", file_path.as_ref()))?;
        
        let mut hasher = Sha256::new();
        hasher.update(&content);
        Ok(format!("{:x}", hasher.finalize()))
    }

    pub async fn get_file_info(&self, file_path: impl AsRef<Path>) -> Result<(String, SystemTime, u64)> {
        let metadata = fs::metadata(&file_path).await
            .with_context(|| format!("Failed to get metadata for: {:?}", file_path.as_ref()))?;
        
        let checksum = self.calculate_file_checksum(&file_path).await?;
        let last_modified = metadata.modified()
            .context("Failed to get file modification time")?;
        let size = metadata.len();

        Ok((checksum, last_modified, size))
    }

    pub async fn has_file_changed(&self, file_path: impl AsRef<Path>) -> Result<bool> {
        let path = file_path.as_ref().to_path_buf();
        
        // If we don't have this file in our database, it's considered changed
        let stored_checksum = match self.database.files.get(&path) {
            Some(checksum) => checksum,
            None => return Ok(true),
        };

        // Check if file still exists
        if !path.exists() {
            return Ok(true);
        }

        // Calculate current checksum and compare
        let (current_checksum, current_modified, current_size) = self.get_file_info(&path).await?;
        
        Ok(current_checksum != stored_checksum.checksum 
           || current_modified != stored_checksum.last_modified
           || current_size != stored_checksum.size)
    }

    pub async fn update_file_checksum(&mut self, file_path: impl AsRef<Path>) -> Result<()> {
        let path = file_path.as_ref().to_path_buf();
        let (checksum, last_modified, size) = self.get_file_info(&path).await?;

        let file_checksum = FileChecksum {
            path: path.clone(),
            checksum,
            last_modified,
            size,
        };

        self.database.files.insert(path, file_checksum);
        Ok(())
    }

    pub fn get_stored_files(&self) -> Vec<&PathBuf> {
        self.database.files.keys().collect()
    }

    pub fn remove_file(&mut self, file_path: impl AsRef<Path>) {
        let path = file_path.as_ref().to_path_buf();
        self.database.files.remove(&path);
    }
}