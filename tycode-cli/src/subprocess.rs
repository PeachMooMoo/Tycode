use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SubprocessMessage {
    // Incoming messages
    Chat {
        message: String,
    },

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
