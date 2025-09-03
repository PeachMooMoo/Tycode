use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessage,
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
        ChatCompletionRequestToolMessage, ChatCompletionRequestUserMessage,
        ChatCompletionResponseMessage, ChatCompletionTool, ChatCompletionToolChoiceOption,
        ChatCompletionToolType, CreateChatCompletionRequestArgs, FinishReason, FunctionCall,
        FunctionObject, Role,
    },
    Client as OpenAIClient,
};
use serde_json::Value;

use crate::ai::model::Model;
use crate::ai::{error::AiError, provider::AiProvider, types::*};

#[derive(Clone)]
pub struct OpenRouterProvider {
    client: OpenAIClient<OpenAIConfig>,
}

impl OpenRouterProvider {
    pub fn new(api_key: String) -> Self {
        let config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base("https://openrouter.ai/api/v1");

        let client = OpenAIClient::with_config(config);

        Self { client }
    }

    fn get_openrouter_model_id(&self, model: &Model) -> Result<String, AiError> {
        let model_id = match model {
            Model::ClaudeOpus41 => "anthropic/claude-opus-4.1",
            Model::ClaudeOpus4 => "anthropic/claude-opus-4",
            Model::ClaudeSonnet4 => "anthropic/claude-sonnet-4",
            Model::ClaudeSonnet37 => "anthropic/claude-3.7-sonnet",
            Model::GptOss120b => "openai/gpt-oss-120b",
            Model::GrokCodeFast1 => "x-ai/grok-code-fast-1",
            _ => {
                return Err(AiError::Terminal(anyhow::anyhow!(
                    "Model {} is not supported in OpenRouter",
                    model.name()
                )))
            }
        };
        Ok(model_id.to_string())
    }

    fn convert_to_openai_messages(
        &self,
        messages: &[Message],
        system_prompt: &str,
    ) -> Result<Vec<ChatCompletionRequestMessage>, AiError> {
        let mut openai_messages = Vec::new();

        // Add system message first
        if !system_prompt.trim().is_empty() {
            openai_messages.push(ChatCompletionRequestMessage::System(
                ChatCompletionRequestSystemMessage {
                    content: system_prompt.to_string(),
                    role: Role::System,
                    name: None,
                },
            ));
        }

        for msg in messages.iter() {
            openai_messages.extend(message_to_openai(msg));
        }

        Ok(openai_messages)
    }
}

#[async_trait::async_trait]
impl AiProvider for OpenRouterProvider {
    fn name(&self) -> &'static str {
        "OpenRouter"
    }

    fn supported_models(&self) -> Vec<Model> {
        vec![
            Model::ClaudeOpus41,
            Model::ClaudeSonnet4,
            Model::ClaudeOpus4,
            Model::ClaudeSonnet37,
            Model::GptOss120b,
            Model::GrokCodeFast1,
        ]
    }

    async fn converse(
        &self,
        request: ConversationRequest,
    ) -> Result<ConversationResponse, AiError> {
        request
            .model
            .validate()
            .map_err(|e| AiError::Terminal(anyhow::anyhow!(e)))?;

        let model_id = self.get_openrouter_model_id(&request.model.model)?;
        let messages =
            self.convert_to_openai_messages(&request.messages, &request.system_prompt)?;

        tracing::debug!(?model_id, "Using OpenRouter API");

        let mut request_builder = CreateChatCompletionRequestArgs::default();
        request_builder.model(&model_id);
        request_builder.messages(messages);

        if let Some(max_tokens) = request.model.max_tokens {
            request_builder.max_tokens(max_tokens as u16);
        }

        if let Some(temperature) = request.model.temperature {
            request_builder.temperature(temperature);
        }

        if let Some(top_p) = request.model.top_p {
            request_builder.top_p(top_p);
        }

        if !request.stop_sequences.is_empty() {
            request_builder.stop(request.stop_sequences);
        }

        if !request.tools.is_empty() {
            let openai_tools = convert_tools_to_openai(&request.tools);
            request_builder
                .tools(openai_tools)
                .tool_choice(ChatCompletionToolChoiceOption::Auto);
        }

        // TODO: OpenRouter may not support reasoning budget directly
        // May need to add reasoning instructions to system prompt instead
        if let Some(_reasoning_budget) = request.model.reasoning_budget {
            tracing::warn!("Reasoning budget not directly supported by OpenRouter, ignoring");
        }

        let chat_request = request_builder
            .build()
            .map_err(|e| AiError::Terminal(anyhow::anyhow!("Failed to build request: {}", e)))?;

        let response = self.client.chat().create(chat_request).await.map_err(|e| {
            tracing::warn!(?e, "OpenRouter API call failed");

            // TODO: Map specific OpenRouter/OpenAI errors to Retryable vs Terminal
            // For now, treat most errors as retryable
            if e.to_string().contains("rate limit")
                || e.to_string().contains("timeout")
                || e.to_string().contains("503")
                || e.to_string().contains("502")
                || e.to_string().contains("500")
            {
                AiError::Retryable(anyhow::anyhow!(e))
            } else {
                AiError::Terminal(anyhow::anyhow!(e))
            }
        })?;

        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| AiError::Terminal(anyhow::anyhow!("No choices in response")))?;

        let usage = if let Some(usage) = response.usage {
            TokenUsage::new(usage.prompt_tokens as u32, usage.completion_tokens as u32)
        } else {
            TokenUsage::empty()
        };

        let stop_reason = match choice.finish_reason {
            Some(FinishReason::Stop) => StopReason::EndTurn,
            Some(FinishReason::Length) => StopReason::MaxTokens,
            Some(FinishReason::ToolCalls) => StopReason::ToolUse,
            Some(FinishReason::ContentFilter) => StopReason::EndTurn,
            Some(FinishReason::FunctionCall) => StopReason::ToolUse,
            None => StopReason::EndTurn,
        };

        let content = extract_content_from_response(&choice.message);

        Ok(ConversationResponse {
            content,
            usage,
            stop_reason,
        })
    }

    fn get_cost(&self, model: &Model) -> Cost {
        match model {
            Model::ClaudeOpus41 => Cost::new(0.015, 0.075),
            Model::ClaudeOpus4 => Cost::new(0.015, 0.075),
            Model::ClaudeSonnet4 => Cost::new(0.003, 0.015),
            Model::ClaudeSonnet37 => Cost::new(0.003, 0.015),
            Model::GptOss120b => Cost::new(0.0001, 0.0005),
            Model::GrokCodeFast1 => Cost::new(0.0002, 0.0015),
            _ => Cost::new(0.0, 0.0),
        }
    }
}

fn message_to_openai(message: &Message) -> Vec<ChatCompletionRequestMessage> {
    let mut results = vec![];

    for tool_result in message.content.tool_results() {
        results.push(ChatCompletionRequestMessage::Tool(
            ChatCompletionRequestToolMessage {
                role: Role::Tool,
                content: tool_result.content.clone(),
                tool_call_id: tool_result.tool_use_id.clone(),
            },
        ));
    }

    match message.role {
        MessageRole::User => {
            let content = extract_text_content(&message.content);
            results.push(ChatCompletionRequestMessage::User(
                ChatCompletionRequestUserMessage {
                    content: content.into(),
                    role: Role::User,
                    name: None,
                },
            ));
        }
        MessageRole::Assistant => {
            let mut content = String::new();
            let mut tool_calls = Vec::new();

            for block in message.content.blocks() {
                match block {
                    ContentBlock::Text(text) => {
                        if !content.is_empty() {
                            content.push_str("\n");
                        }
                        content.push_str(text);
                    }
                    ContentBlock::ReasoningContent(reasoning) => {
                        // Include reasoning content as text for OpenRouter
                        if !content.is_empty() {
                            content.push_str("\n");
                        }
                        content.push_str(&format!("[Reasoning: {}]", reasoning.text));
                    }
                    ContentBlock::ToolUse(tool_use) => {
                        tool_calls.push(ChatCompletionMessageToolCall {
                            id: tool_use.id.clone(),
                            r#type: ChatCompletionToolType::Function,
                            function: FunctionCall {
                                name: tool_use.name.clone(),
                                arguments: serde_json::to_string(&tool_use.arguments)
                                    .expect("Failed to serialize json to string"),
                            },
                        });
                    }
                    ContentBlock::ToolResult(_) => {
                        continue;
                    }
                }
            }

            results.push(ChatCompletionRequestMessage::Assistant(
                ChatCompletionRequestAssistantMessage {
                    content: Some(content),
                    role: Role::Assistant,
                    tool_calls: Some(tool_calls),
                    ..Default::default()
                },
            ));
        }
    }

    results
}

fn extract_text_content(content: &Content) -> String {
    let mut text_parts = Vec::new();

    for block in content.blocks() {
        match block {
            ContentBlock::Text(text) => {
                text_parts.push(text.clone());
            }
            ContentBlock::ReasoningContent(reasoning) => {
                // Include reasoning as text for user messages
                text_parts.push(format!("[Reasoning: {}]", reasoning.text));
            }
            ContentBlock::ToolUse(_) | ContentBlock::ToolResult(_) => {
                continue;
            }
        }
    }

    text_parts.join("\n")
}

fn convert_tools_to_openai(tools: &[ToolDefinition]) -> Vec<ChatCompletionTool> {
    tools
        .iter()
        .map(|tool| ChatCompletionTool {
            r#type: ChatCompletionToolType::Function,
            function: FunctionObject {
                name: tool.name.clone(),
                description: Some(tool.description.clone()),
                parameters: Some(tool.input_schema.clone()),
            },
        })
        .collect()
}

fn extract_content_from_response(message: &ChatCompletionResponseMessage) -> Content {
    let mut content_blocks = Vec::new();

    if let Some(content) = &message.content {
        if !content.trim().is_empty() {
            content_blocks.push(ContentBlock::Text(content.clone()));
        }
    }

    if let Some(tool_calls) = &message.tool_calls {
        for tool_call in tool_calls {
            if let Ok(arguments) = serde_json::from_str::<Value>(&tool_call.function.arguments) {
                let tool_use_data = ToolUseData {
                    id: tool_call.id.clone(),
                    name: tool_call.function.name.clone(),
                    arguments,
                };
                content_blocks.push(ContentBlock::ToolUse(tool_use_data));
            }
        }
    }

    Content::from(content_blocks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::tests::{
        test_hello_world, test_reasoning_conversation, test_reasoning_with_tools, test_tool_usage,
    };

    async fn create_openrouter_provider() -> anyhow::Result<OpenRouterProvider> {
        let api_key = "";
        Ok(OpenRouterProvider::new(api_key.to_string()))
    }

    #[tokio::test]
    #[ignore = "requires OpenRouter API key"]
    async fn test_openrouter_hello_world() {
        let provider = match create_openrouter_provider().await {
            Ok(provider) => provider,
            Err(e) => {
                tracing::error!(?e, "Failed to create OpenRouter provider");
                panic!("Failed to create OpenRouter provider: {:?}", e);
            }
        };

        if let Err(e) = test_hello_world(provider).await {
            tracing::error!(?e, "OpenRouter hello world test failed");
            panic!("OpenRouter hello world test failed: {:?}", e);
        }
    }

    #[tokio::test]
    #[ignore = "requires OpenRouter API key"]
    async fn test_openrouter_reasoning_conversation() {
        let provider = match create_openrouter_provider().await {
            Ok(provider) => provider,
            Err(e) => {
                tracing::error!(?e, "Failed to create OpenRouter provider");
                panic!("Failed to create OpenRouter provider: {:?}", e);
            }
        };

        if let Err(e) = test_reasoning_conversation(provider).await {
            tracing::error!(?e, "OpenRouter reasoning conversation test failed");
            panic!("OpenRouter reasoning conversation test failed: {:?}", e);
        }
    }

    #[tokio::test]
    #[ignore = "requires OpenRouter API key"]
    async fn test_openrouter_tool_usage() {
        let provider = match create_openrouter_provider().await {
            Ok(provider) => provider,
            Err(e) => {
                tracing::error!(?e, "Failed to create OpenRouter provider");
                panic!("Failed to create OpenRouter provider: {:?}", e);
            }
        };

        if let Err(e) = test_tool_usage(provider).await {
            tracing::error!(?e, "OpenRouter tool usage test failed");
            panic!("OpenRouter tool usage test failed: {:?}", e);
        }
    }

    #[tokio::test]
    #[ignore = "requires OpenRouter API key"]
    async fn test_openrouter_reasoning_with_tools() {
        let provider = match create_openrouter_provider().await {
            Ok(provider) => provider,
            Err(e) => {
                tracing::error!(?e, "Failed to create OpenRouter provider");
                panic!("Failed to create OpenRouter provider: {:?}", e);
            }
        };

        if let Err(e) = test_reasoning_with_tools(provider).await {
            tracing::error!(?e, "OpenRouter reasoning with tools test failed");
            panic!("OpenRouter reasoning with tools test failed: {:?}", e);
        }
    }
}
