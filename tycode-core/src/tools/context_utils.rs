use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::warn;

const MAX_FILES_PER_DIR: usize = 20;
const MAX_OUTPUT_SIZE_BYTES: usize = 8192; // 8KB limit

pub struct RelevantFiles {
    pub files: Vec<PathBuf>,
    pub truncated: bool,
    pub workspace_roots: Vec<PathBuf>,
}

pub fn list_relevant_files(working_dirs: &[PathBuf]) -> RelevantFiles {
    let mut all_files = Vec::new();
    let mut truncated = false;
    let mut total_size = 0usize;
    let use_workspace_prefix = working_dirs.len() > 1;

    for working_dir in working_dirs {
        if total_size >= MAX_OUTPUT_SIZE_BYTES {
            truncated = true;
            break;
        }

        let ignored = load_gitignore_patterns(working_dir);
        
        let workspace_name = if use_workspace_prefix {
            Some(working_dir.file_name()
                .unwrap_or(working_dir.as_os_str())
                .to_string_lossy()
                .to_string())
        } else {
            None
        };

        let mut workspace_files = Vec::new();
        
        if let Err(e) = collect_files(
            working_dir,
            working_dir,
            &ignored,
            &mut workspace_files,
            &mut truncated,
            &mut total_size,
        ) {
            warn!(?e, "Error collecting files from {:?}", working_dir);
        }

        // Prefix files with workspace name if we have multiple workspaces
        if let Some(ws_name) = workspace_name {
            for file in workspace_files {
                let prefixed = PathBuf::from(format!("[{}]/{}", ws_name, file.display()));
                all_files.push(prefixed);
            }
        } else {
            all_files.extend(workspace_files);
        }
    }

    RelevantFiles { 
        files: all_files, 
        truncated,
        workspace_roots: working_dirs.to_vec(),
    }
}



fn collect_files(
    base_dir: &Path,
    current_dir: &Path,
    ignored: &HashSet<String>,
    files: &mut Vec<PathBuf>,
    truncated: &mut bool,
    total_size: &mut usize,
) -> std::io::Result<()> {
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

    let files_to_add = if dir_files.len() > MAX_FILES_PER_DIR {
        *truncated = true;
        dir_files.into_iter().take(MAX_FILES_PER_DIR).collect::<Vec<_>>()
    } else {
        dir_files
    };

    for file in files_to_add {
        let file_str = file.to_string_lossy();
        *total_size += file_str.len() + 1;
        
        if *total_size >= MAX_OUTPUT_SIZE_BYTES {
            *truncated = true;
            files.push(PathBuf::from("..."));
            return Ok(());
        }
        
        files.push(file);
    }

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
            let root_pattern = &pattern[1..];
            
            if root_pattern.ends_with('/') {
                let dir_name = &root_pattern[..root_pattern.len() - 1];
                if path == dir_name || path.starts_with(&format!("{}/", dir_name)) {
                    return true;
                }
            } else {
                if path == root_pattern || path.starts_with(&format!("{}/", root_pattern)) {
                    return true;
                }
            }
        } else if pattern.ends_with('/') {
            let dir_name = &pattern[..pattern.len() - 1];
            
            if path == dir_name || path.starts_with(&format!("{}/", dir_name)) {
                return true;
            }
            
            let components: Vec<&str> = path.split('/').collect();
            if components.contains(&dir_name) {
                return true;
            }
        } else if pattern.starts_with("*.") {
            let ext = &pattern[1..];
            if path.ends_with(ext) {
                return true;
            }
        } else if pattern.contains('/') {
            if path == pattern || path.starts_with(&format!("{}/", pattern)) {
                return true;
            }
        } else {
            let components: Vec<&str> = path.split('/').collect();
            
            if components.contains(&pattern.as_str()) {
                return true;
            }
            
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

        let result = list_relevant_files(&[dir_path.to_path_buf()]);

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

        let result = list_relevant_files(&[dir_path.to_path_buf()]);

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

    #[test]
    fn test_list_relevant_files_multiple_roots() {
        let temp_dir1 = tempdir().unwrap();
        let dir_path1 = temp_dir1.path();
        
        let temp_dir2 = tempdir().unwrap();
        let dir_path2 = temp_dir2.path();

        // Create files in first workspace
        fs::create_dir(dir_path1.join("src")).unwrap();
        fs::write(dir_path1.join("src/main.rs"), "content").unwrap();
        fs::write(dir_path1.join("Cargo.toml"), "content").unwrap();

        // Create files in second workspace
        fs::create_dir(dir_path2.join("lib")).unwrap();
        fs::write(dir_path2.join("lib/utils.rs"), "content").unwrap();
        fs::write(dir_path2.join("package.json"), "content").unwrap();

        let result = list_relevant_files(&[
            dir_path1.to_path_buf(),
            dir_path2.to_path_buf(),
        ]);

        // Files should be prefixed with workspace names
        let files_str: Vec<String> = result.files.iter().map(|p| p.to_string_lossy().to_string()).collect();
        
        // Should contain files from both workspaces with workspace prefixes
        assert!(files_str.iter().any(|p| p.contains("]") && p.contains("main.rs")));
        assert!(files_str.iter().any(|p| p.contains("]") && p.contains("Cargo.toml")));
        assert!(files_str.iter().any(|p| p.contains("]") && p.contains("utils.rs")));
        assert!(files_str.iter().any(|p| p.contains("]") && p.contains("package.json")));
        
        // Should have both workspace roots tracked
        assert_eq!(result.workspace_roots.len(), 2);
    }
}
