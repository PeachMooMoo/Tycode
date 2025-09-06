use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use crate::ai::model::Model;

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

#[derive(Debug, Clone)]
pub struct ConversationRequest {
    pub messages: Vec<Message>,
    pub model: ModelSettings,
    pub system_prompt: String,
    pub stop_sequences: Vec<String>,
    pub tools: Vec<ToolDefinition>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ReasoningBudget {
    Off,
    Low,
    High,
}

impl ReasoningBudget {
    pub fn get_max_tokens(&self) -> Option<u32> {
        match self {
            ReasoningBudget::Off => None,
            ReasoningBudget::Low => Some(4000),
            ReasoningBudget::High => Some(8000),
        }
    }

    pub fn from_u32(value: u32) -> Self {
        if value == 0 {
            ReasoningBudget::Off
        } else if value <= 4000 {
            ReasoningBudget::Low
        } else {
            ReasoningBudget::High
        }
    }
}

impl std::fmt::Display for ReasoningBudget {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ReasoningBudget::Off => write!(f, "off"),
            ReasoningBudget::Low => write!(f, "low"),
            ReasoningBudget::High => write!(f, "high"),
        }
    }
}

impl Default for ReasoningBudget {
    fn default() -> Self {
        ReasoningBudget::High
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]

pub struct ModelSettings {
    pub model: Model,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub reasoning_budget: ReasoningBudget,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: MessageRole,
    pub content: Content,
}

impl Message {
    pub fn new(role: MessageRole, content: Content) -> Self {
        Self { role, content }
    }

    pub fn user(content: impl Into<Content>) -> Self {
        Self::new(MessageRole::User, content.into())
    }

    pub fn assistant(content: impl Into<Content>) -> Self {
        Self::new(MessageRole::Assistant, content.into())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningData {
    pub text: String,
    pub signature: Option<String>,
    pub blob: Option<Vec<u8>>,
}

impl std::fmt::Display for ReasoningData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.text)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseData {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultData {
    pub tool_use_id: String,
    pub content: String,
    pub is_error: bool,
}

#[derive(Debug, Clone)]
pub enum ContentBlock {
    Text(String),
    ReasoningContent(ReasoningData),
    ToolUse(ToolUseData),
    ToolResult(ToolResultData),
}

#[derive(Debug, Clone)]
pub struct Content {
    blocks: Vec<ContentBlock>,
}

impl Content {
    pub fn new(blocks: Vec<ContentBlock>) -> Self {
        Self { blocks }
    }

    pub fn empty() -> Self {
        Self { blocks: Vec::new() }
    }

    pub fn text_only(text: String) -> Self {
        Self {
            blocks: vec![ContentBlock::Text(text.trim().to_string())],
        }
    }

    pub fn blocks(&self) -> &[ContentBlock] {
        &self.blocks
    }

    pub fn into_blocks(self) -> Vec<ContentBlock> {
        self.blocks
    }

    pub fn text(&self) -> String {
        self.blocks
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text(text) => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<String>>()
            .join("")
    }

    pub fn reasoning(&self) -> Vec<&ReasoningData> {
        self.blocks
            .iter()
            .filter_map(|block| match block {
                ContentBlock::ReasoningContent(reasoning) => Some(reasoning),
                _ => None,
            })
            .collect()
    }

    pub fn tool_uses(&self) -> Vec<&ToolUseData> {
        self.blocks
            .iter()
            .filter_map(|block| match block {
                ContentBlock::ToolUse(tool_use) => Some(tool_use),
                _ => None,
            })
            .collect()
    }

    pub fn tool_results(&self) -> Vec<&ToolResultData> {
        self.blocks
            .iter()
            .filter_map(|block| match block {
                ContentBlock::ToolResult(tool_result) => Some(tool_result),
                _ => None,
            })
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    pub fn push(&mut self, block: ContentBlock) {
        self.blocks.push(block);
    }

    pub fn extend(&mut self, blocks: Vec<ContentBlock>) {
        self.blocks.extend(blocks);
    }
}

impl From<Vec<ContentBlock>> for Content {
    fn from(blocks: Vec<ContentBlock>) -> Self {
        Self::new(blocks)
    }
}

impl From<ContentBlock> for Content {
    fn from(block: ContentBlock) -> Self {
        Self::new(vec![block])
    }
}

impl From<String> for Content {
    fn from(text: String) -> Self {
        Self::text_only(text)
    }
}

impl From<&str> for Content {
    fn from(text: &str) -> Self {
        Self::text_only(text.to_string())
    }
}

impl IntoIterator for Content {
    type Item = ContentBlock;
    type IntoIter = std::vec::IntoIter<ContentBlock>;

    fn into_iter(self) -> Self::IntoIter {
        self.blocks.into_iter()
    }
}

impl<'a> IntoIterator for &'a Content {
    type Item = &'a ContentBlock;
    type IntoIter = std::slice::Iter<'a, ContentBlock>;

    fn into_iter(self) -> Self::IntoIter {
        self.blocks.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_tree_compaction() {
        let mut context = MessageContext::new(vec![PathBuf::from(".")]);

        // Add files that would be repetitive in flat format
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

        // Verify the tree structure
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

        // Count characters to show compaction
        let tree_size = formatted.len();

        // Compare with what flat format would be
        let flat_format = format!(
            "Working Directory: .\n\nProject Files:\n  - Cargo.toml\n  - src/main.rs\n  - src/lib.rs\n  - src/module/file1.rs\n  - src/module/file2.rs\n  - src/module/submodule/file1.rs\n  - src/module/submodule/file2.rs\n  - src/module/submodule/file3.rs\n  - tests/test1.rs\n  - tests/test2.rs\n\n"
        );
        let flat_size = flat_format.len();

        println!("Tree format size: {} chars", tree_size);
        println!("Flat format size: {} chars", flat_size);
        println!(
            "Savings: {} chars ({:.1}% reduction)",
            flat_size - tree_size,
            ((flat_size - tree_size) as f64 / flat_size as f64) * 100.0
        );

        // Tree format should be more compact
        assert!(
            tree_size < flat_size,
            "Tree format should be more compact than flat format"
        );
    }

    #[test]
    fn test_file_tree_single_files() {
        let mut context = MessageContext::new(vec![PathBuf::from(".")]);

        // Test with just root-level files
        context.relevant_files = vec![
            PathBuf::from("README.md"),
            PathBuf::from("Cargo.toml"),
            PathBuf::from(".gitignore"),
        ];

        let formatted = context.to_formatted_string();

        assert!(formatted.contains("  README.md"));
        assert!(formatted.contains("  Cargo.toml"));
        assert!(formatted.contains("  .gitignore"));
        // Should not have any directory indicators
        assert!(!formatted.contains("/\n"));
    }

    #[test]
    fn test_file_tree_deep_nesting() {
        let mut context = MessageContext::new(vec![PathBuf::from(".")]);

        // Test with deeply nested structure
        context.relevant_files = vec![
            PathBuf::from("a/b/c/d/e/file.rs"),
            PathBuf::from("a/b/c/d/e/file2.rs"),
        ];

        let formatted = context.to_formatted_string();

        // Should create nested structure
        assert!(formatted.contains("  a/"));
        assert!(formatted.contains("    b/"));
        assert!(formatted.contains("      c/"));
        assert!(formatted.contains("        d/"));
        assert!(formatted.contains("          e/"));
        assert!(formatted.contains("            file.rs"));
        assert!(formatted.contains("            file2.rs"));
    }
}

#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub provider: String,
    pub model_id: String,
    pub region: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConversationResponse {
    pub content: Content,
    pub usage: TokenUsage,
    pub stop_reason: StopReason,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone)]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    StopSequence(String),
    ToolUse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

impl TokenUsage {
    pub fn new(input_tokens: u32, output_tokens: u32) -> Self {
        Self {
            input_tokens,
            output_tokens,
            total_tokens: input_tokens + output_tokens,
        }
    }

    pub fn empty() -> Self {
        Self::new(0, 0)
    }
}

#[derive(Debug, Clone)]
pub struct Cost {
    pub input_cost_per_1k_tokens: f64,
    pub output_cost_per_1k_tokens: f64,
}

impl Cost {
    pub fn new(input_cost_per_1k_tokens: f64, output_cost_per_1k_tokens: f64) -> Self {
        Self {
            input_cost_per_1k_tokens,
            output_cost_per_1k_tokens,
        }
    }

    pub fn calculate_cost(&self, usage: &TokenUsage) -> f64 {
        let input_cost = (usage.input_tokens as f64 / 1000.0) * self.input_cost_per_1k_tokens;
        let output_cost = (usage.output_tokens as f64 / 1000.0) * self.output_cost_per_1k_tokens;
        input_cost + output_cost
    }
}
