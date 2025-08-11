use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::warn;

const MAX_FILES_PER_DIR: usize = 20;

pub struct RelevantFiles {
    pub files: Vec<PathBuf>,
    pub truncated: bool,
}

pub fn list_relevant_files(working_dir: &Path) -> RelevantFiles {
    let ignored = load_gitignore_patterns(working_dir);
    let mut files = Vec::new();
    let mut truncated = false;

    if let Err(e) = collect_files(
        working_dir,
        working_dir,
        &ignored,
        &mut files,
        &mut truncated,
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
) -> std::io::Result<()> {
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

    if dir_files.len() > MAX_FILES_PER_DIR {
        *truncated = true;
        files.extend(dir_files.into_iter().take(MAX_FILES_PER_DIR));
        files.push(PathBuf::from("..."));
    } else {
        files.extend(dir_files);
    }

    for subdir in subdirs {
        if let Err(e) = collect_files(base_dir, &subdir, ignored, files, truncated) {
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
        if pattern.ends_with('/') {
            if path.starts_with(&pattern[..pattern.len() - 1]) {
                return true;
            }
        } else if pattern.starts_with("*.") {
            let ext = &pattern[1..];
            if path.ends_with(ext) {
                return true;
            }
        } else if path.contains(pattern) || path.starts_with(pattern) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_is_ignored() {
        let mut patterns = HashSet::new();
        patterns.insert("target".to_string());
        patterns.insert("*.pyc".to_string());
        patterns.insert("node_modules/".to_string());

        assert!(is_ignored("target/debug/file", &patterns));
        assert!(is_ignored("test.pyc", &patterns));
        assert!(is_ignored("node_modules/package", &patterns));
        assert!(!is_ignored("src/main.rs", &patterns));
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

        assert!(!result.truncated);
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
}
