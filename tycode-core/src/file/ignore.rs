use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub struct Ignored {
    patterns: HashSet<String>,
}

impl Ignored {
    pub fn new(root: &Path) -> anyhow::Result<Self> {
        // Default patterns we respect even if there is no .gitignore
        let mut patterns = HashSet::new();
        patterns.insert(".git".to_string());
        patterns.insert("node_modules".to_string());
        patterns.insert("*.pyc".to_string());
        patterns.insert("__pycache__".to_string());
        patterns.insert(".DS_Store".to_string());

        let gitignore_path = root.join(".gitignore");
        if gitignore_path.exists() {
            let content = fs::read_to_string(&gitignore_path)?;
            for line in content.lines() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with('#') {
                    patterns.insert(line.to_string());
                }
            }
        }

        Ok(Self { patterns })
    }

    pub fn is_ignored(&self, path: &str) -> bool {
        // all '.' directories are always ignored - this is critical to avoid
        // the AI modifying .git directories so we hard code it to reduce the
        // chance of bugs!
        let components: Vec<&str> = path.split('/').collect();
        if components.iter().any(|c| c.starts_with('.')) {
            return true;
        }

        for pattern in &self.patterns {
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
}
