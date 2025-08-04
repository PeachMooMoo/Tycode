use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct ConversationRequest {
    pub messages: Vec<Message>,
    pub model: Model,
    pub system_prompt: String,
    pub tunings: ModelTunings,
    pub stop_sequences: Vec<String>,
    pub tools: Vec<ToolDefinition>,
}

#[derive(Debug, Clone)]
pub struct ModelTunings {
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub reasoning_budget: Option<u32>,
}

impl ModelTunings {
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

impl Default for ModelTunings {
    fn default() -> Self {
        Self {
            max_tokens: None,
            temperature: None,
            top_p: None,
            reasoning_budget: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Model {
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
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: MessageRole,
    pub content: Content,
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
