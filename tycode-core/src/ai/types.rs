use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct MessageContext {
    pub working_directory: PathBuf,
    pub relevant_files: Vec<PathBuf>,
    pub tracked_file_contents: HashMap<PathBuf, String>,
}

impl MessageContext {
    pub fn new(working_directory: PathBuf) -> Self {
        Self {
            working_directory,
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

        result.push_str(&format!(
            "Working Directory: {}\n\n",
            self.working_directory.display()
        ));

        if !self.relevant_files.is_empty() {
            result.push_str("Project Files:\n");
            for file in &self.relevant_files {
                result.push_str(&format!("  - {}\n", file.display()));
            }
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
}

#[derive(Debug, Clone)]
pub struct ConversationRequest {
    pub messages: Vec<Message>,
    pub model: ModelSettings,
    pub system_prompt: String,
    pub stop_sequences: Vec<String>,
    pub tools: Vec<ToolDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ModelSettings {
    pub model: Model,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub reasoning_budget: Option<u32>,
}

impl ModelSettings {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(reasoning_budget) = self.reasoning_budget {
            if let Some(max_tokens) = self.max_tokens {
                if max_tokens <= reasoning_budget {
                    return Err(format!(
                        "max_tokens ({}) must be greater than reasoning_budget ({})",
                        max_tokens, reasoning_budget
                    ));
                }
            } else {
                return Err("max_tokens is required when using reasoning_budget".to_string());
            }

            if let Some(temperature) = self.temperature {
                if temperature != 1.0 {
                    return Err(format!(
                        "temperature must be 1.0 when using reasoning_budget, got {}",
                        temperature
                    ));
                }
            } else {
                return Err("temperature must be 1.0 when using reasoning_budget".to_string());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Model {
    ClaudeOpus41,
    ClaudeOpus4,
    ClaudeSonnet4,
    ClaudeSonnet37,
    ClaudeHaiku35,
    ClaudeSonnet35V2,
    ClaudeSonnet35,
    ClaudeHaiku3,
}

impl Model {
    pub const fn name(self) -> &'static str {
        match self {
            Self::ClaudeOpus41 => "claude-opus-4-1",
            Self::ClaudeOpus4 => "claude-opus-4",
            Self::ClaudeSonnet4 => "claude-sonnet-4",
            Self::ClaudeSonnet37 => "claude-sonnet-3-7",
            Self::ClaudeHaiku35 => "claude-haiku-3-5",
            Self::ClaudeSonnet35V2 => "claude-sonnet-3-5-v2",
            Self::ClaudeSonnet35 => "claude-sonnet-3-5",
            Self::ClaudeHaiku3 => "claude-haiku-3",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "claude-opus-4-1" => Some(Self::ClaudeOpus41),
            "claude-opus-4" => Some(Self::ClaudeOpus4),
            "claude-sonnet-4" => Some(Self::ClaudeSonnet4),
            "claude-sonnet-3-7" => Some(Self::ClaudeSonnet37),
            "claude-haiku-3-5" => Some(Self::ClaudeHaiku35),
            "claude-sonnet-3-5-v2" => Some(Self::ClaudeSonnet35V2),
            "claude-sonnet-3-5" => Some(Self::ClaudeSonnet35),
            "claude-haiku-3" => Some(Self::ClaudeHaiku3),
            _ => None,
        }
    }

    pub fn all_models() -> Vec<Self> {
        vec![
            Self::ClaudeOpus41,
            Self::ClaudeOpus4,
            Self::ClaudeSonnet4,
            Self::ClaudeSonnet37,
            Self::ClaudeHaiku35,
            Self::ClaudeSonnet35V2,
            Self::ClaudeSonnet35,
            Self::ClaudeHaiku3,
        ]
    }
}

impl Default for Model {
    fn default() -> Self {
        Model::ClaudeSonnet4
    }
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

    pub fn system(content: impl Into<Content>) -> Self {
        Self::new(MessageRole::System, content.into())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct ToolUseData {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone)]
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
            blocks: vec![ContentBlock::Text(text)],
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

#[derive(Debug, Clone)]
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
