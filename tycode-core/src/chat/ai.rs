use crate::agents::agent::ActiveAgent;
use crate::agents::catalog::AgentCatalog;
use crate::agents::tool_type::ToolType;
use crate::ai::{
    error::AiError, provider::AiProvider, Content, ContentBlock, ConversationRequest,
    ConversationResponse, Message, MessageContext, MessageRole, ModelSettings, ToolResultData,
    ToolUseData,
};
use crate::chat::events::{ChatEvent, ChatMessage, ContextInfo, FileInfo, ModelInfo};
use crate::security::types::{RiskLevel, SecurityMode, ToolPermission};
use crate::tools::context_utils::list_relevant_files;
use crate::tools::file_access::FileAccessManager;
use crate::tools::r#trait::ToolResult;
use crate::tools::registry::ToolRegistry;
use anyhow::{bail, Result};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

use super::actor::ActorState;

// Helper functions for ActorState
pub fn current_agent(state: &ActorState) -> &ActiveAgent {
    state.agent_stack.last().expect("No active agent")
}

pub fn current_agent_mut(state: &mut ActorState) -> &mut ActiveAgent {
    state.agent_stack.last_mut().expect("No active agent")
}

pub async fn send_ai_request(state: &mut ActorState) -> Result<()> {
    loop {
        // Prepare the AI request with all necessary context
        let (request, context_info, model_settings) = prepare_ai_request(state).await?;

        // Send request and get response
        let response = match send_request_with_retry(state, request).await {
            Ok(response) => response,
            Err(e) => {
                state
                    .event_sender
                    .add_message(ChatMessage::error(format!("Error: {:?}", e)));
                return Ok(());
            }
        };

        // Process the response and update conversation
        let tool_calls = process_ai_response(state, response, model_settings, context_info);

        // If there are tool calls, execute them and continue the loop
        if !tool_calls.is_empty() {
            Box::pin(execute_tool_calls(state, tool_calls)).await?;
            continue;
        }

        // No more tool calls, exit the loop
        break;
    }

    Ok(())
}

async fn prepare_ai_request(
    state: &mut ActorState,
) -> Result<(ConversationRequest, ContextInfo, ModelSettings)> {
    let current = current_agent(state);

    // Prepare tools
    let allowed_tools: HashSet<ToolType> = current.agent.available_tools().into_iter().collect();
    let allowed_tool_types: Vec<ToolType> = allowed_tools.into_iter().collect();

    let file_modification_api = state.config.file_modification_api.clone();
    let tool_registry = ToolRegistry::new(state.workspace_roots.clone(), file_modification_api);
    let available_tools = tool_registry.get_tool_definitions_for_types(&allowed_tool_types);

    // Build message context
    let message_context = build_message_context(state).await;
    let context_info = create_context_info(&message_context);
    let context_string = message_context.to_formatted_string();
    let context_text = format!("Current Context:\n{}", context_string);

    let mut conversation = current_agent(state).conversation.clone();
    if conversation.is_empty() {
        bail!("No messages to send to AI. Conversation is empty!")
    }

    conversation
        .last_mut()
        .unwrap()
        .content
        .push(ContentBlock::Text(context_text));

    let default_model = current.agent.default_model();
    let agent_name = current.agent.name();
    let model_settings =
        if let Some(override_model) = state.settings.settings().get_agent_model(agent_name) {
            override_model.clone()
        } else {
            default_model
        };
    let system_prompt = current.agent.system_prompt().to_string();

    let request = ConversationRequest {
        messages: conversation,
        model: model_settings.clone(),
        system_prompt,
        stop_sequences: vec![],
        tools: available_tools,
    };

    info!(?request, "AI request");

    Ok((request, context_info, model_settings))
}

fn create_context_info(message_context: &MessageContext) -> ContextInfo {
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

fn process_ai_response(
    state: &mut ActorState,
    response: ConversationResponse,
    model_settings: ModelSettings,
    context_info: ContextInfo,
) -> Vec<ToolUseData> {
    let content = response.content.clone();

    info!(?response, "AI response");

    // Accumulate token usage for session tracking
    state.session_token_usage.input_tokens += response.usage.input_tokens;
    state.session_token_usage.output_tokens += response.usage.output_tokens;
    state.session_token_usage.total_tokens += response.usage.total_tokens;

    // Calculate and accumulate cost using the actual model used for this response
    let cost = state.provider.get_cost(&model_settings.model);
    let response_cost = cost.calculate_cost(&response.usage);
    state.session_cost += response_cost;

    let reasoning = content.reasoning().first().map(|r| (*r).clone());
    let tool_calls: Vec<_> = content.tool_uses().iter().map(|t| (*t).clone()).collect();

    // Add assistant message to UI
    state.event_sender.add_message(ChatMessage::assistant(
        current_agent(state).agent.name().to_string(),
        content.text(),
        tool_calls.clone(),
        ModelInfo {
            model: model_settings.model,
        },
        response.usage,
        context_info,
        reasoning,
    ));

    // Add to conversation history
    current_agent_mut(state).conversation.push(Message {
        role: MessageRole::Assistant,
        content,
    });

    tool_calls
}

async fn execute_tool_calls(state: &mut ActorState, tool_calls: Vec<ToolUseData>) -> Result<()> {
    info!(
        tool_count = tool_calls.len(),
        tools = ?tool_calls.iter().map(|t| &t.name).collect::<Vec<_>>(),
        "Executing tool calls"
    );

    // Get allowed tools for security checks
    let current = current_agent(state);
    let allowed_tools: HashSet<ToolType> = current.agent.available_tools().into_iter().collect();
    let allowed_tool_types: Vec<ToolType> = allowed_tools.into_iter().collect();

    let file_modification_api = state.config.file_modification_api.clone();
    let tool_registry = ToolRegistry::new(state.workspace_roots.clone(), file_modification_api);

    for tool_use in &tool_calls {
        let tool_result =
            execute_tool_with_security(state, &tool_registry, tool_use, &allowed_tool_types).await;

        handle_tool_result(state, tool_result, tool_use).await?;
    }
    Ok(())
}

async fn handle_tool_result(
    state: &mut ActorState,
    tool_result: crate::tools::r#trait::ToolResult,
    tool_use: &ToolUseData,
) -> Result<()> {
    match tool_result {
        ToolResult::Success {
            context_data,
            ui_data,
        } => {
            handle_tool_success(state, tool_use, context_data, ui_data);
        }
        ToolResult::Error(error) => {
            handle_tool_error(state, tool_use, error);
        }
        ToolResult::PushAgent {
            agent_type,
            task,
            context,
        } => {
            handle_tool_push_agent(state, agent_type, task, context, tool_use.id.clone()).await?;
        }
        ToolResult::PopAgent {
            success,
            summary,
            artifacts,
        } => {
            handle_tool_pop_agent(state, success, summary, artifacts, tool_use.id.clone()).await?;
        }
    }
    Ok(())
}

fn handle_tool_success(
    state: &mut ActorState,
    tool_use: &ToolUseData,
    context_data: serde_json::Value,
    ui_data: Option<serde_json::Value>,
) {
    // Check if this is a set_tracked_files tool and update state accordingly
    if tool_use.name == "set_tracked_files" {
        if let Some(action) = context_data.get("action") {
            if action.as_str() == Some("set_tracked_files") {
                if let Some(tracked_files) = context_data.get("tracked_files") {
                    if let Some(files_array) = tracked_files.as_array() {
                        // Clear and update tracked files in actor state
                        state.tracked_files.clear();
                        for file_value in files_array {
                            if let Some(file_str) = file_value.as_str() {
                                state.tracked_files.insert(PathBuf::from(file_str));
                            }
                        }
                        info!("Updated tracked files: {:?}", state.tracked_files);
                    }
                }
            }
        }
    }

    let result = ToolResultData {
        tool_use_id: tool_use.id.clone(),
        content: context_data.to_string(),
        is_error: false,
    };

    info!(
        tool_name = %tool_use.name,
        ?result,
        ?ui_data,
        "Tool execution completed"
    );

    // Emit tool completion event
    let parsed_result = serde_json::from_str(&result.content).ok();
    let event = ChatEvent::ToolExecutionCompleted {
        tool_name: tool_use.name.clone(),
        success: true,
        result: parsed_result,
        ui_data,
        error: None,
    };

    if let Err(e) = state.event_sender.event_tx.send(event) {
        error!("Failed to send tool completion event: {:?}", e);
    }

    // Add to conversation - just the tool result, context will be added later
    current_agent_mut(state).conversation.push(Message {
        role: MessageRole::User,
        content: Content::from(vec![ContentBlock::ToolResult(result)]),
    });
}

fn handle_tool_error(state: &mut ActorState, tool_use: &ToolUseData, error: String) {
    let result = ToolResultData {
        tool_use_id: tool_use.id.clone(),
        content: error.clone(),
        is_error: true,
    };

    info!(
        tool_name = %tool_use.name,
        ?result,
        "Tool execution failed"
    );

    let event = ChatEvent::ToolExecutionCompleted {
        tool_name: tool_use.name.clone(),
        success: false,
        result: None,
        ui_data: None,
        error: Some(error),
    };

    if let Err(e) = state.event_sender.event_tx.send(event) {
        error!("Failed to send tool completion event: {:?}", e);
    }

    // Add to conversation - just the tool result, context will be added later
    current_agent_mut(state).conversation.push(Message {
        role: MessageRole::User,
        content: Content::from(vec![ContentBlock::ToolResult(result)]),
    });
}

async fn handle_tool_push_agent(
    state: &mut ActorState,
    agent_type: String,
    task: String,
    context: Option<String>,
    tool_use_id: String,
) -> Result<()> {
    info!(
        "Tool requesting agent push: type={}, task={}",
        agent_type, task
    );

    // Store the tool_use_id in the current agent before pushing
    current_agent_mut(state).spawn_tool_use_id = Some(tool_use_id.clone());
    info!("Pushing new agent: type={}, task={}", agent_type, task);

    let Some(agent) = AgentCatalog::create_agent(&agent_type) else {
        // On error, add a tool result to continue the conversation
        let error_msg = format!("Unknown agent type: {}", agent_type);
        let result = ToolResultData {
            tool_use_id,
            content: format!("Unknown agent: {:?}", agent_type),
            is_error: true,
        };

        current_agent_mut(state).conversation.push(Message {
            role: MessageRole::User,
            content: Content::from(vec![ContentBlock::ToolResult(result)]),
        });
        state
            .event_sender
            .add_message(ChatMessage::error(error_msg));
        return Ok(());
    };

    // Create initial message for the new agent
    let mut initial_message = task.clone();
    if let Some(ctx) = context {
        initial_message.push_str(&format!("\n\nContext from parent agent:\n{}", ctx));
    }

    // Push the new agent onto the stack
    let mut new_agent = ActiveAgent::new(agent);
    new_agent.conversation.push(Message {
        role: MessageRole::User,
        content: Content::text_only(initial_message.clone()),
    });

    state.agent_stack.push(new_agent);

    // Notify user
    state.event_sender.add_message(ChatMessage::system(format!(
        "🔄 Spawning {} agent for task: {}",
        agent_type, task
    )));

    Ok(())
}

async fn handle_tool_pop_agent(
    state: &mut ActorState,
    success: bool,
    summary: String,
    artifacts: Option<serde_json::Value>,
    tool_use_id: String,
) -> Result<()> {
    info!("Popping agent: success={}, summary={}", success, summary);

    // Don't pop if we're at the root agent
    if state.agent_stack.len() <= 1 {
        current_agent_mut(state).conversation.push(Message {
            role: MessageRole::User,
            content: ContentBlock::ToolResult(ToolResultData {
                tool_use_id,
                content: "Cannot complete task; you are the root agent".to_string(),
                is_error: true,
            })
            .into(),
        });

        state.event_sender.add_message(ChatMessage::error(
            "Cannot complete task - this is the root agent".to_string(),
        ));
        return Ok(());
    }

    state.agent_stack.pop();

    // Create result content
    let result_content = if let Some(ref artifacts_data) = artifacts {
        serde_json::json!({
            "success": success,
            "summary": summary,
            "artifacts": artifacts_data
        })
    } else {
        serde_json::json!({
            "success": success,
            "summary": summary
        })
    };

    // If we have a tool_use_id, add the tool result to complete the spawn_agent call
    let Some(tool_id) = current_agent_mut(state).spawn_tool_use_id.take() else {
        bail!("BUG: no tool_use_id set on parent agent")
    };

    let tool_result = ToolResultData {
        tool_use_id: tool_id,
        content: result_content.to_string(),
        is_error: false,
    };

    current_agent_mut(state).conversation.push(Message {
        role: MessageRole::User,
        content: Content::from(vec![ContentBlock::ToolResult(tool_result)]),
    });

    // Add a user-friendly summary message
    let result_message = if success {
        format!("✅ Sub-agent completed successfully:\n{}", summary)
    } else {
        format!("❌ Sub-agent failed:\n{}", summary)
    };

    // Notify user
    state
        .event_sender
        .add_message(ChatMessage::system(result_message));

    Ok(())
}

async fn build_message_context(state: &ActorState) -> MessageContext {
    let mut context = MessageContext::new(state.workspace_roots.clone());

    let relevant_files = list_relevant_files(&state.workspace_roots);
    context.set_relevant_files(relevant_files.files);

    let tracked_files: Vec<PathBuf> = state.tracked_files.iter().cloned().collect();
    let file_manager = FileAccessManager::new(state.workspace_roots.clone());

    for file_path in tracked_files {
        let path_str = file_path.to_string_lossy();
        match file_manager.read_file(&path_str).await {
            Ok(content) => {
                context.add_tracked_file(file_path, content);
            }
            Err(e) => {
                warn!(?e, "Failed to read tracked file: {:?}", file_path);
            }
        }
    }

    context
}

async fn send_request_with_retry(
    state: &mut ActorState,
    request: ConversationRequest,
) -> Result<ConversationResponse> {
    const MAX_RETRIES: u32 = 1000;
    const INITIAL_BACKOFF_MS: u64 = 100;
    const MAX_BACKOFF_MS: u64 = 1000;
    const BACKOFF_MULTIPLIER: f64 = 2.0;

    let mut attempt = 0;

    loop {
        match try_send_request(&state.provider, &request).await {
            Ok(response) => {
                if attempt > 0 {
                    info!("Request succeeded after {} retries", attempt);
                }
                return Ok(response);
            }
            Err(error) => {
                if !should_retry(&error, attempt, MAX_RETRIES) {
                    warn!(
                        attempt,
                        max_retries = MAX_RETRIES,
                        "Request failed after {} retries: {}",
                        attempt,
                        error
                    );
                    return Err(error.into());
                }

                let backoff_ms = calculate_backoff(
                    attempt,
                    INITIAL_BACKOFF_MS,
                    MAX_BACKOFF_MS,
                    BACKOFF_MULTIPLIER,
                );

                emit_retry_event(state, attempt + 1, MAX_RETRIES, &error, backoff_ms);

                warn!(
                    attempt = attempt + 1,
                    max_retries = MAX_RETRIES,
                    backoff_ms,
                    error = %error,
                    "Request failed, retrying after backoff"
                );

                sleep(Duration::from_millis(backoff_ms)).await;
                attempt += 1;
            }
        }
    }
}

async fn try_send_request(
    provider: &Box<dyn AiProvider>,
    request: &ConversationRequest,
) -> Result<ConversationResponse, AiError> {
    provider.converse(request.clone()).await
}

fn should_retry(error: &AiError, attempt: u32, max_retries: u32) -> bool {
    matches!(error, AiError::Retryable(_)) && attempt < max_retries
}

fn calculate_backoff(attempt: u32, initial_ms: u64, max_ms: u64, multiplier: f64) -> u64 {
    let base_backoff = initial_ms as f64 * multiplier.powi(attempt as i32);
    base_backoff.min(max_ms as f64) as u64
}

fn emit_retry_event(
    state: &ActorState,
    attempt: u32,
    max_retries: u32,
    error: &AiError,
    backoff_ms: u64,
) {
    let retry_event = ChatEvent::RetryAttempt {
        attempt,
        max_retries,
        error: error.to_string(),
        backoff_ms,
    };

    if let Err(e) = state.event_sender.event_tx.send(retry_event) {
        error!("Failed to send retry event: {:?}", e);
    }
}

async fn execute_tool_with_security(
    state: &ActorState,
    tool_registry: &ToolRegistry,
    tool_use: &ToolUseData,
    allowed_tool_types: &[ToolType],
) -> crate::tools::r#trait::ToolResult {
    // First evaluate the risk level of the tool
    let risk_level = match tool_registry.evaluate_tool_risk(&tool_use.name, &tool_use.arguments) {
        Ok(risk) => risk,
        Err(e) => {
            error!(
                tool_name = %tool_use.name,
                error = ?e,
                "Failed to evaluate tool risk"
            );
            return crate::tools::r#trait::ToolResult::Error(format!(
                "Failed to evaluate tool risk: {}",
                e
            ));
        }
    };

    // Check permission with security manager
    let permission = state.security_manager.check_permission(risk_level);
    let current_mode = state.security_manager.get_mode();

    info!(
        tool_name = %tool_use.name,
        ?risk_level,
        ?permission,
        ?current_mode,
        "Security check performed"
    );

    // If permission denied, return error result
    if permission == ToolPermission::Denied {
        let mode_hint = get_mode_change_hint(risk_level, current_mode);
        let denial_message = format!(
            "🔒 Security: Blocked execution of '{}'\nRisk Level: {:?}\nCurrent Mode: {:?}\n{}",
            tool_use.name, risk_level, current_mode, mode_hint
        );

        warn!(
            tool_name = %tool_use.name,
            ?risk_level,
            ?current_mode,
            "Tool execution denied by security policy"
        );

        return crate::tools::r#trait::ToolResult::Error(denial_message);
    }

    // Permission granted - execute the tool
    info!(
        tool_name = %tool_use.name,
        "Tool execution allowed, proceeding with execution"
    );

    tool_registry
        .execute_tool(tool_use, allowed_tool_types)
        .await
}

fn get_mode_change_hint(risk_level: RiskLevel, current_mode: SecurityMode) -> String {
    match (risk_level, current_mode) {
        (RiskLevel::ReadOnly, _) => {
            // ReadOnly operations should never be blocked
            String::new()
        }
        (RiskLevel::LowRisk, SecurityMode::ReadOnly) => {
            "To allow this operation, change to 'auto' or 'all' mode with /security auto"
                .to_string()
        }
        (RiskLevel::HighRisk, SecurityMode::ReadOnly) => {
            "To allow this operation, change to 'all' mode with /security all".to_string()
        }
        (RiskLevel::HighRisk, SecurityMode::Auto) => {
            "To allow this operation, change to 'all' mode with /security all".to_string()
        }
        _ => String::new(),
    }
}
