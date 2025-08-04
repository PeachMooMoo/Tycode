use anyhow::Result;
use serde_json::Value;

#[async_trait::async_trait]
pub trait ToolExecutor: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn input_schema(&self) -> Value;
    async fn execute(&self, arguments: &Value) -> Result<Value>;
}
