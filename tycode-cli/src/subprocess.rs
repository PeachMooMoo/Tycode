use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SubprocessMessage {
    // Incoming messages
    Chat {
        message: String,
    },
    Cancel,

    // Outgoing messages
    Response {
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        reasoning: Option<String>,
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        tool_calls: Vec<ToolCall>,
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        is_complete: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        context_info: Option<ContextInfo>,
        #[serde(skip_serializing_if = "Option::is_none")]
        token_usage: Option<TokenUsage>,
    },
    ToolResult {
        tool_name: String,
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    Event {
        event: String,
        data: serde_json::Value,
    },
    Ready,
    Error {
        error: String,
    },
}

#[derive(Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ContextInfo {
    pub directory_list_bytes: usize,
    pub files: Vec<FileInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FileInfo {
    pub path: String,
    pub bytes: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
}
