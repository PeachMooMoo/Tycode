use crate::chat::events::{ContextInfo, FileInfo};
use crate::file::access::FileAccessManager;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use tracing::warn;

#[derive(Debug, Clone)]
pub struct AllFiles {
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct MessageContext {
    pub working_directories: Vec<PathBuf>,
    pub relevant_files: Vec<PathBuf>,
    pub tracked_file_contents: HashMap<PathBuf, String>,
}

impl MessageContext {
    pub fn new(working_directories: Vec<PathBuf>) -> Self {
        Self {
            working_directories,
            relevant_files: Vec::new(),
            tracked_file_contents: HashMap::new(),
        }
    }

    pub fn add_tracked_file(&mut self, path: PathBuf, content: String) {
        self.tracked_file_contents.insert(path, content);
    }

    pub fn set_relevant_files(&mut self, files: Vec<PathBuf>) {
        self.relevant_files = files;
    }

    pub fn get_context_size(&self) -> usize {
        self.tracked_file_contents.values().map(|s| s.len()).sum()
    }

    pub fn to_formatted_string(&self) -> String {
        let mut result = String::new();

        if self.working_directories.len() == 1 {
            result.push_str(&format!(
                "Working Directory: {}\n\n",
                self.working_directories[0].display()
            ));
        } else {
            result.push_str("Working Directories:\n");
            for dir in &self.working_directories {
                result.push_str(&format!("  {}\n", dir.display()));
            }
            result.push('\n');
        }

        if !self.relevant_files.is_empty() {
            result.push_str("Project Files:\n");
            result.push_str(&self.build_file_tree());
            result.push_str("\n");
        }

        if !self.tracked_file_contents.is_empty() {
            result.push_str("Tracked Files:\n");
            for (path, content) in &self.tracked_file_contents {
                result.push_str(&format!("\n=== {} ===\n", path.display()));
                result.push_str(content);
                result.push_str("\n");
            }
        }

        result
    }

    fn build_file_tree(&self) -> String {
        #[derive(Debug)]
        struct TreeNode {
            children: BTreeMap<String, TreeNode>,
            is_file: bool,
        }

        impl TreeNode {
            fn new() -> Self {
                Self {
                    children: BTreeMap::new(),
                    is_file: false,
                }
            }

            fn insert(&mut self, path: &[&str]) {
                if path.is_empty() {
                    return;
                }

                if path.len() == 1 {
                    self.children.entry(path[0].to_string()).or_insert_with(|| {
                        let mut node = TreeNode::new();
                        node.is_file = true;
                        node
                    });
                } else {
                    let child = self
                        .children
                        .entry(path[0].to_string())
                        .or_insert_with(TreeNode::new);
                    child.insert(&path[1..]);
                }
            }

            fn format(&self, indent: usize) -> String {
                let mut result = String::new();
                let indent_str = " ".repeat(indent);

                for (name, node) in &self.children {
                    if node.is_file || node.children.is_empty() {
                        result.push_str(&format!("{}{}\n", indent_str, name));
                    } else {
                        result.push_str(&format!("{}{}/\n", indent_str, name));
                        result.push_str(&node.format(indent + 2));
                    }
                }

                result
            }
        }

        let mut root = TreeNode::new();

        for file in &self.relevant_files {
            let path_str = file.to_string_lossy();
            let parts: Vec<&str> = path_str.split('/').collect();
            root.insert(&parts);
        }

        root.format(2)
    }
}

pub async fn build_message_context(
    workspace_roots: &[PathBuf],
    tracked_files: &[PathBuf],
) -> MessageContext {
    let mut context = MessageContext::new(workspace_roots.to_vec());

    let file_manager = FileAccessManager::new(workspace_roots.to_vec());
    let all_files = list_all_files(&file_manager).await;
    context.set_relevant_files(all_files.files);

    let file_manager = FileAccessManager::new(workspace_roots.to_vec());

    for file_path in tracked_files {
        let path_str = file_path.to_string_lossy();
        match file_manager.read_file(&path_str).await {
            Ok(content) => {
                context.add_tracked_file(file_path.clone(), content);
            }
            Err(e) => {
                warn!(?e, "Failed to read tracked file: {:?}", file_path);
            }
        }
    }

    context
}

async fn list_all_files(file_manager: &FileAccessManager) -> AllFiles {
    let mut all_files = Vec::new();

    for root in &file_manager.roots {
        if let Ok(files) = collect_files_recursively(file_manager, root).await {
            all_files.extend(files);
        }
    }

    AllFiles { files: all_files }
}

async fn collect_files_recursively(
    file_manager: &FileAccessManager,
    directory_path: &str,
) -> Result<Vec<PathBuf>, anyhow::Error> {
    let mut files = Vec::new();

    let entries = file_manager.list_directory(directory_path).await?;

    for entry in entries {
        let entry_str = entry.to_string_lossy();

        // Check if this entry exists and get metadata
        if file_manager.file_exists(&entry_str).await.unwrap_or(false) {
            // Try to list it as a directory - if this succeeds, it's a directory
            if let Ok(_) = file_manager.list_directory(&entry_str).await {
                // It's a directory, recurse into it
                if let Ok(subfiles) =
                    Box::pin(collect_files_recursively(file_manager, &entry_str)).await
                {
                    files.extend(subfiles);
                }
            } else {
                // It's a file, add it to our list
                files.push(entry);
            }
        }
    }

    Ok(files)
}

pub fn create_context_info(message_context: &MessageContext) -> ContextInfo {
    let dir_list_size = message_context
        .relevant_files
        .iter()
        .map(|p| p.to_string_lossy().len() + 1)
        .sum::<usize>();

    let files: Vec<FileInfo> = message_context
        .tracked_file_contents
        .iter()
        .map(|(path, content)| FileInfo {
            path: path.to_string_lossy().to_string(),
            bytes: content.len(),
        })
        .collect();

    ContextInfo {
        directory_list_bytes: dir_list_size,
        files,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_tree_compaction() {
        let mut context = MessageContext::new(vec![PathBuf::from(".")]);

        context.relevant_files = vec![
            PathBuf::from("Cargo.toml"),
            PathBuf::from("src/main.rs"),
            PathBuf::from("src/lib.rs"),
            PathBuf::from("src/module/file1.rs"),
            PathBuf::from("src/module/file2.rs"),
            PathBuf::from("src/module/submodule/file1.rs"),
            PathBuf::from("src/module/submodule/file2.rs"),
            PathBuf::from("src/module/submodule/file3.rs"),
            PathBuf::from("tests/test1.rs"),
            PathBuf::from("tests/test2.rs"),
        ];

        let formatted = context.to_formatted_string();

        assert!(formatted.contains("Project Files:"));
        assert!(formatted.contains("  Cargo.toml"));
        assert!(formatted.contains("  src/"));
        assert!(formatted.contains("    main.rs"));
        assert!(formatted.contains("    lib.rs"));
        assert!(formatted.contains("    module/"));
        assert!(formatted.contains("      file1.rs"));
        assert!(formatted.contains("      file2.rs"));
        assert!(formatted.contains("      submodule/"));
        assert!(formatted.contains("        file1.rs"));
        assert!(formatted.contains("        file2.rs"));
        assert!(formatted.contains("        file3.rs"));
        assert!(formatted.contains("  tests/"));
        assert!(formatted.contains("    test1.rs"));
        assert!(formatted.contains("    test2.rs"));

        let tree_size = formatted.len();

        let flat_format = format!(
            "Working Directory: .\n\nProject Files:\n  - Cargo.toml\n  - src/main.rs\n  - src/lib.rs\n  - src/module/file1.rs\n  - src/module/file2.rs\n  - src/module/submodule/file1.rs\n  - src/module/submodule/file2.rs\n  - src/module/submodule/file3.rs\n  - tests/test1.rs\n  - tests/test2.rs\n\n"
        );
        let flat_size = flat_format.len();

        assert!(
            tree_size < flat_size,
            "Tree format should be more compact than flat format"
        );
    }

    #[test]
    fn test_file_tree_single_files() {
        let mut context = MessageContext::new(vec![PathBuf::from(".")]);

        context.relevant_files = vec![
            PathBuf::from("README.md"),
            PathBuf::from("Cargo.toml"),
            PathBuf::from(".gitignore"),
        ];

        let formatted = context.to_formatted_string();

        assert!(formatted.contains("  README.md"));
        assert!(formatted.contains("  Cargo.toml"));
        assert!(formatted.contains("  .gitignore"));
        assert!(!formatted.contains("/\n"));
    }

    #[test]
    fn test_file_tree_deep_nesting() {
        let mut context = MessageContext::new(vec![PathBuf::from(".")]);

        context.relevant_files = vec![
            PathBuf::from("a/b/c/d/e/file.rs"),
            PathBuf::from("a/b/c/d/e/file2.rs"),
        ];

        let formatted = context.to_formatted_string();

        assert!(formatted.contains("  a/"));
        assert!(formatted.contains("    b/"));
        assert!(formatted.contains("      c/"));
        assert!(formatted.contains("        d/"));
        assert!(formatted.contains("          e/"));
        assert!(formatted.contains("            file.rs"));
        assert!(formatted.contains("            file2.rs"));
    }
}
