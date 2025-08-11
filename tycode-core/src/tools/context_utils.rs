use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::warn;

const MAX_FILES_PER_DIR: usize = 20;
const MAX_OUTPUT_SIZE_BYTES: usize = 8192; // 8KB limit

pub struct RelevantFiles {
    pub files: Vec<PathBuf>,
    pub truncated: bool,
}

pub fn list_relevant_files(working_dir: &Path) -> RelevantFiles {
    let ignored = load_gitignore_patterns(working_dir);
    let mut files = Vec::new();
    let mut truncated = false;
    let mut total_size = 0usize;

    if let Err(e) = collect_files(
        working_dir,
        working_dir,
        &ignored,
        &mut files,
        &mut truncated,
        &mut total_size,
    ) {
        warn!(?e, "Error collecting files");
    }

    RelevantFiles { files, truncated }
}

fn collect_files(
    base_dir: &Path,
    current_dir: &Path,
    ignored: &HashSet<String>,
    files: &mut Vec<PathBuf>,
    truncated: &mut bool,
    total_size: &mut usize,
) -> std::io::Result<()> {
    // Check size limit
    if *total_size >= MAX_OUTPUT_SIZE_BYTES {
        *truncated = true;
        return Ok(());
    }

    let entries = fs::read_dir(current_dir)?;
    let mut dir_files = Vec::new();
    let mut subdirs = Vec::new();

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if let Some(rel_path) = path.strip_prefix(base_dir).ok() {
            let path_str = rel_path.to_string_lossy().replace('\\', "/");

            if is_ignored(&path_str, &ignored) {
                continue;
            }

            if path.is_dir() {
                subdirs.push(path);
            } else {
                dir_files.push(rel_path.to_path_buf());
            }
        }
    }

    // Add files with size tracking
    let files_to_add = if dir_files.len() > MAX_FILES_PER_DIR {
        *truncated = true;
        dir_files.into_iter().take(MAX_FILES_PER_DIR).collect::<Vec<_>>()
    } else {
        dir_files
    };

    for file in files_to_add {
        let file_str = file.to_string_lossy();
        *total_size += file_str.len() + 1; // +1 for newline
        
        if *total_size >= MAX_OUTPUT_SIZE_BYTES {
            *truncated = true;
            files.push(PathBuf::from("..."));
            return Ok(());
        }
        
        files.push(file);
    }

    // Process subdirectories
    for subdir in subdirs {
        if *total_size >= MAX_OUTPUT_SIZE_BYTES {
            *truncated = true;
            return Ok(());
        }
        
        if let Err(e) = collect_files(base_dir, &subdir, ignored, files, truncated, total_size) {
            warn!(?e, "Error collecting files from {:?}", subdir);
        }
    }

    Ok(())
}

fn load_gitignore_patterns(working_dir: &Path) -> HashSet<String> {
    let mut patterns = HashSet::new();

    patterns.insert(".git".to_string());
    patterns.insert("target".to_string());
    patterns.insert("node_modules".to_string());
    patterns.insert("*.pyc".to_string());
    patterns.insert("__pycache__".to_string());
    patterns.insert(".DS_Store".to_string());

    let gitignore_path = working_dir.join(".gitignore");
    if gitignore_path.exists() {
        if let Ok(content) = fs::read_to_string(&gitignore_path) {
            for line in content.lines() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with('#') {
                    patterns.insert(line.to_string());
                }
            }
        }
    }

    patterns
}

fn is_ignored(path: &str, patterns: &HashSet<String>) -> bool {
    for pattern in patterns {
        if pattern.starts_with('/') {
            // Root-only pattern - must match from the beginning of path
            let root_pattern = &pattern[1..]; // Remove leading slash
            
            if root_pattern.ends_with('/') {
                // Root directory pattern
                let dir_name = &root_pattern[..root_pattern.len() - 1];
                if path == dir_name || path.starts_with(&format!("{}/", dir_name)) {
                    return true;
                }
            } else {
                // Root file or directory pattern
                if path == root_pattern || path.starts_with(&format!("{}/", root_pattern)) {
                    return true;
                }
            }
        } else if pattern.ends_with('/') {
            // Directory pattern - matches directory and all its contents anywhere
            let dir_name = &pattern[..pattern.len() - 1];
            
            // Check if path is the directory itself or inside it
            if path == dir_name || path.starts_with(&format!("{}/", dir_name)) {
                return true;
            }
            
            // Check if directory appears as a component anywhere in the path
            let components: Vec<&str> = path.split('/').collect();
            if components.contains(&dir_name) {
                return true;
            }
        } else if pattern.starts_with("*.") {
            // Extension pattern - matches files with this extension
            let ext = &pattern[1..]; // includes the dot
            if path.ends_with(ext) {
                return true;
            }
        } else if pattern.contains('/') {
            // Path pattern with slash - must match from root
            if path == pattern || path.starts_with(&format!("{}/", pattern)) {
                return true;
            }
        } else {
            // Simple filename/directory pattern - matches as a complete component
            let components: Vec<&str> = path.split('/').collect();
            
            // Check if pattern matches any complete path component
            if components.contains(&pattern.as_str()) {
                return true;
            }
            
            // Also check if it matches the complete path (for single-level files)
            if path == pattern {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_is_ignored_basic_patterns() {
        let mut patterns = HashSet::new();
        patterns.insert("target".to_string());
        patterns.insert("*.pyc".to_string());
        patterns.insert("node_modules/".to_string());

        // Directory without trailing slash should match directory and contents
        assert!(is_ignored("target", &patterns));
        assert!(is_ignored("target/debug/file", &patterns));
        assert!(is_ignored("src/target/file", &patterns)); // target as a subdirectory
        
        // Extension patterns
        assert!(is_ignored("test.pyc", &patterns));
        assert!(is_ignored("src/test.pyc", &patterns));
        assert!(!is_ignored("test.py", &patterns));
        
        // Directory with trailing slash
        assert!(is_ignored("node_modules", &patterns));
        assert!(is_ignored("node_modules/package", &patterns));
        assert!(is_ignored("src/node_modules/package", &patterns));
        
        // Should not match
        assert!(!is_ignored("src/main.rs", &patterns));
        assert!(!is_ignored("targeted", &patterns)); // not a complete component
    }

    #[test]
    fn test_is_ignored_gitignore_patterns() {
        let mut patterns = HashSet::new();
        patterns.insert(".git".to_string());
        patterns.insert("target/".to_string());
        patterns.insert("Cargo.lock".to_string());
        patterns.insert(".DS_Store".to_string());
        patterns.insert("*.pyc".to_string());
        patterns.insert("__pycache__".to_string());
        patterns.insert(".tycode/".to_string());

        // .git directory
        assert!(is_ignored(".git", &patterns));
        assert!(is_ignored(".git/config", &patterns));
        
        // target directory with trailing slash
        assert!(is_ignored("target", &patterns));
        assert!(is_ignored("target/debug", &patterns));
        assert!(is_ignored("target/debug/deps/file", &patterns));
        
        // Cargo.lock file
        assert!(is_ignored("Cargo.lock", &patterns));
        assert!(is_ignored("subdir/Cargo.lock", &patterns));
        
        // .DS_Store file
        assert!(is_ignored(".DS_Store", &patterns));
        assert!(is_ignored("src/.DS_Store", &patterns));
        
        // Python files
        assert!(is_ignored("test.pyc", &patterns));
        assert!(is_ignored("__pycache__", &patterns));
        assert!(is_ignored("__pycache__/file.pyc", &patterns));
        assert!(is_ignored("src/__pycache__/file.pyc", &patterns));
        
        // .tycode directory
        assert!(is_ignored(".tycode", &patterns));
        assert!(is_ignored(".tycode/index", &patterns));
        
        // Should not match
        assert!(!is_ignored("src/main.rs", &patterns));
        assert!(!is_ignored("Cargo.toml", &patterns));
    }

    #[test]
    fn test_is_ignored_edge_cases() {
        let mut patterns = HashSet::new();
        patterns.insert("build/".to_string());
        patterns.insert("test".to_string());
        
        // Directory with trailing slash
        assert!(is_ignored("build", &patterns));
        assert!(is_ignored("build/output", &patterns));
        
        // File or directory without trailing slash
        assert!(is_ignored("test", &patterns));
        assert!(is_ignored("test/file", &patterns));
        assert!(is_ignored("src/test", &patterns));
        assert!(is_ignored("src/test/file", &patterns));
        
        // Should not match partial names
        assert!(!is_ignored("testing", &patterns));
        assert!(!is_ignored("build.rs", &patterns));
        assert!(!is_ignored("my_build", &patterns));
    }

    #[test]
    fn test_is_ignored_root_only_patterns() {
        let mut patterns = HashSet::new();
        patterns.insert("/build".to_string());
        patterns.insert("/build/".to_string());
        patterns.insert("/test.txt".to_string());
        patterns.insert("/.env".to_string());
        
        // /build without trailing slash - matches root build directory and its contents
        assert!(is_ignored("build", &patterns));
        assert!(is_ignored("build/output", &patterns));
        assert!(is_ignored("build/debug/file", &patterns));
        
        // Should NOT match build in subdirectories
        assert!(!is_ignored("src/build", &patterns));
        assert!(!is_ignored("src/build/output", &patterns));
        assert!(!is_ignored("nested/path/build", &patterns));
        
        // /test.txt - only matches file at root
        assert!(is_ignored("test.txt", &patterns));
        assert!(!is_ignored("src/test.txt", &patterns));
        assert!(!is_ignored("nested/test.txt", &patterns));
        
        // /.env - only matches file at root
        assert!(is_ignored(".env", &patterns));
        assert!(!is_ignored("src/.env", &patterns));
        assert!(!is_ignored("config/.env", &patterns));
        
        // Should not match partial names even at root
        assert!(!is_ignored("building", &patterns));
        assert!(!is_ignored("build.rs", &patterns));
        assert!(!is_ignored("my_build", &patterns));
    }

    #[test]
    fn test_list_relevant_files() {
        let temp_dir = tempdir().unwrap();
        let dir_path = temp_dir.path();

        fs::create_dir(dir_path.join("src")).unwrap();
        fs::write(dir_path.join("src/main.rs"), "content").unwrap();
        fs::write(dir_path.join("Cargo.toml"), "content").unwrap();

        fs::create_dir(dir_path.join("target")).unwrap();
        fs::write(dir_path.join("target/debug"), "content").unwrap();

        fs::write(dir_path.join(".gitignore"), "target\n*.tmp").unwrap();
        fs::write(dir_path.join("test.tmp"), "content").unwrap();

        let result = list_relevant_files(dir_path);

        assert!(result
            .files
            .iter()
            .any(|p| p.to_string_lossy().contains("main.rs")));
        assert!(result
            .files
            .iter()
            .any(|p| p.to_string_lossy().contains("Cargo.toml")));
        assert!(!result
            .files
            .iter()
            .any(|p| p.to_string_lossy().contains("target")));
        assert!(!result
            .files
            .iter()
            .any(|p| p.to_string_lossy().contains("test.tmp")));
    }

    #[test]
    fn test_list_relevant_files_with_size_limit() {
        let temp_dir = tempdir().unwrap();
        let dir_path = temp_dir.path();

        // Create many files to test size limit
        for i in 0..1000 {
            let filename = format!("file_with_very_long_name_to_test_size_limit_{:04}.txt", i);
            fs::write(dir_path.join(&filename), "content").unwrap();
        }

        let result = list_relevant_files(dir_path);

        // Should be truncated due to size limit
        assert!(result.truncated);
        
        // Calculate approximate size
        let total_size: usize = result
            .files
            .iter()
            .map(|p| p.to_string_lossy().len() + 1)
            .sum();
        
        // Should be close to but not exceed MAX_OUTPUT_SIZE_BYTES
        assert!(total_size <= MAX_OUTPUT_SIZE_BYTES + 1000); // Some buffer for the last file
    }
}
