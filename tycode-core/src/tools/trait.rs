use anyhow::Result;
use serde_json::Value;

/// Result from tool execution with separate context and UI data
#[derive(Debug)]
pub struct ToolResult {
    /// Data to include in conversation context
    pub context_data: Value,
    /// Optional UI-specific data (not added to context)
    pub ui_data: Option<Value>,
}

impl ToolResult {
    /// Create a result with only context data
    pub fn context_only(data: Value) -> Self {
        Self {
            context_data: data,
            ui_data: None,
        }
    }

    /// Create a result with both context and UI data
    pub fn with_ui(context_data: Value, ui_data: Value) -> Self {
        Self {
            context_data,
            ui_data: Some(ui_data),
        }
    }
}

#[async_trait::async_trait(?Send)]
pub trait ToolExecutor {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn input_schema(&self) -> Value;
    async fn execute(&self, arguments: &Value) -> Result<ToolResult>;
}
