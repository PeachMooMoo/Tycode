use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Model {
    ClaudeOpus41,
    ClaudeOpus4,
    ClaudeSonnet4,
    ClaudeSonnet37,
    GptOss120b,
    GrokCodeFast1,
}

impl Model {
    pub const fn name(self) -> &'static str {
        match self {
            Self::ClaudeOpus41 => "claude-opus-4-1",
            Self::ClaudeOpus4 => "claude-opus-4",
            Self::ClaudeSonnet4 => "claude-sonnet-4",
            Self::ClaudeSonnet37 => "claude-sonnet-3-7",
            Self::GptOss120b => "gpt-oss-120b",
            Self::GrokCodeFast1 => "x-ai/grok-code-fast-1",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "claude-opus-4-1" => Some(Self::ClaudeOpus41),
            "claude-opus-4" => Some(Self::ClaudeOpus4),
            "claude-sonnet-4" => Some(Self::ClaudeSonnet4),
            "claude-sonnet-3-7" => Some(Self::ClaudeSonnet37),
            "gpt-oss-120b" => Some(Self::GptOss120b),
            "x-ai/grok-code-fast-1" => Some(Self::GrokCodeFast1),
            _ => None,
        }
    }

    pub fn all_models() -> Vec<Self> {
        vec![
            Self::ClaudeOpus41,
            Self::ClaudeOpus4,
            Self::ClaudeSonnet4,
            Self::ClaudeSonnet37,
            Self::GptOss120b,
            Self::GrokCodeFast1,
        ]
    }
}

impl Default for Model {
    fn default() -> Self {
        Model::ClaudeSonnet4
    }
}
