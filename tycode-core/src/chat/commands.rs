use crate::agents::catalog::AgentCatalog;
use crate::ai::model::Model;
use crate::ai::{ModelSettings, ReasoningBudget};
use crate::chat::events::EventSender;
use crate::chat::{
    actor::ActorState,
    ai::{self, current_agent},
    events::{ChatMessage, MessageSender},
    state::FileModificationApi,
};
use chrono::Utc;

#[derive(Clone, Debug)]
pub struct CommandInfo {
    pub name: String,
    pub description: String,
    pub usage: String,
}

/// Process a command and directly mutate the actor state
pub async fn process_command(state: &mut ActorState, command: &str) -> Vec<ChatMessage> {
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return vec![];
    }

    match parts[0] {
        "clear" => handle_clear_command(state).await,
        "context" => handle_context_command(state).await,
        "fileapi" => handle_fileapi_command(state, &parts).await,
        "model" => handle_model_command(state, &parts).await,
        "settings" => handle_settings_command(state).await,
        "security" => handle_security_command(&state.event_sender, &parts).await,
        "agentmodel" => handle_agentmodel_command(state, &parts).await,
        "agent" => handle_agent_command(state, &parts).await,
        "cost" => handle_cost_command(state).await,
        "help" => handle_help_command().await,
        "models" => handle_models_command(state).await,
        _ => vec![create_message(
            format!("Unknown command: /{}", parts[0]),
            MessageSender::Error,
        )],
    }
}

/// Get all available commands with their descriptions
pub fn get_available_commands() -> Vec<CommandInfo> {
    vec![
        CommandInfo {
            name: "clear".to_string(),
            description: r"Clear the conversation history".to_string(),
            usage: "/clear".to_string(),
        },
        CommandInfo {
            name: "context".to_string(),
            description: r"Show what files would be included in the AI context".to_string(),
            usage: "/context".to_string(),
        },
        CommandInfo {
            name: "fileapi".to_string(),
            description: r"Set the file modification API (patch or find-replace)".to_string(),
            usage: "/fileapi <patch|findreplace>".to_string(),
        },
        CommandInfo {
            name: r"model".to_string(),
            description: r"Set the AI model for all agents".to_string(),
            usage: r"/model <name> [temperature=0.7] [max_tokens=4096] [top_p=1.0] [reasoning_budget=...]".to_string(),
        },
        CommandInfo {
            name: "trace".to_string(),
            description: r"Enable/disable trace logging to .tycode/trace".to_string(),
            usage: "/trace <on|off>".to_string(),
        },
        CommandInfo {
            name: "settings".to_string(),
            description: "Display current settings and configuration".to_string(),
            usage: "/settings".to_string(),
        },
        CommandInfo {
            name: "security".to_string(),
            description: "Manage security mode and permissions".to_string(),
            usage: "/security [mode|whitelist|clear] [args...]".to_string(),
        },
        CommandInfo {
            name: "cost".to_string(),
            description: "Show session token usage and estimated cost".to_string(),
            usage: "/cost".to_string(),
        },
        CommandInfo {
            name: "help".to_string(),
            description: "Show this help message".to_string(),
            usage: "/help".to_string(),
        },
        CommandInfo {
            name: "models".to_string(),
            description: "List available AI models".to_string(),
            usage: "/models".to_string(),
        },
        CommandInfo {
            name: "agentmodel".to_string(),
            description: "Set the AI model for a specific agent with tunings".to_string(),
            usage: "/agentmodel <agent_name> <model_name> [temperature=0.7] [max_tokens=4096] [top_p=1.0] [reasoning_budget=...]".to_string(),
        },
        CommandInfo {
            name: "agent".to_string(),
            description: "Switch the current agent".to_string(),
            usage: "/agent <name>".to_string(),
        },
        CommandInfo {
            name: "quit".to_string(),
            description: "Exit the application".to_string(),
            usage: "/quit or /exit".to_string(),
        },
    ]
}

async fn handle_clear_command(state: &mut ActorState) -> Vec<ChatMessage> {
    state.event_sender.clear_conversation();
    ai::current_agent_mut(state).conversation.clear();
    vec![create_message(
        "Conversation cleared.".to_string(),
        MessageSender::System,
    )]
}

async fn handle_context_command(state: &ActorState) -> Vec<ChatMessage> {
    use crate::tools::context_utils::list_relevant_files;
    use crate::tools::file_access::FileAccessManager;
    use std::path::PathBuf;

    let working_dir = PathBuf::from(".");

    // Get relevant files (directory listing)
    let relevant = list_relevant_files(&[working_dir.clone()]);

    // Get tracked files
    let file_manager = FileAccessManager::new(vec![working_dir.clone()]);

    let mut message = String::new();
    message.push_str(r"=== AI Context Debug Info ===\n\n");

    // Show directory listing info
    message.push_str(r"DIRECTORY LISTING (file paths only):\n");
    message.push_str(&format!(r"  Total files: {}\n", relevant.files.len()));
    if relevant.truncated {
        message.push_str(r"  ⚠️ WARNING: Directory listing was TRUNCATED (too many files)\n");
    }

    // Group files by directory for better readability
    let mut dirs: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for file in &relevant.files {
        let file_str = file.to_string_lossy().to_string();
        if file_str == "..." {
            continue; // Skip truncation marker
        }
        let dir = if let Some(parent) = file.parent() {
            parent.to_string_lossy().to_string()
        } else {
            ".".to_string()
        };
        dirs.entry(dir).or_default().push(
            file.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| file_str.clone()),
        );
    }

    let mut sorted_dirs: Vec<_> = dirs.into_iter().collect();
    sorted_dirs.sort_by(|a, b| a.0.cmp(&b.0));

    for (dir, mut files) in sorted_dirs {
        files.sort();
        let dir_display = if dir.is_empty() || dir == "." {
            "."
        } else {
            &dir
        };
        message.push_str(&format!("\n  {}/ ({} files)\n", dir_display, files.len()));
        for file in files.iter().take(5) {
            message.push_str(&format!("    - {}\n", file));
        }
        if files.len() > 5 {
            message.push_str(&format!("    ... and {} more\n", files.len() - 5));
        }
    }

    // Calculate directory listing size
    let dir_list_size: usize = relevant
        .files
        .iter()
        .map(|p| p.to_string_lossy().len() + 1) // +1 for newline
        .sum();
    message.push_str(&format!(
        "\n  Directory listing size: {} bytes\n",
        dir_list_size
    ));

    // Show tracked files info
    message.push_str("\n\nTRACKED FILES (full content sent):\n");
    message.push_str(&format!("  Total tracked: {}\n", state.tracked_files.len()));

    let mut total_tracked_size = 0;
    let mut tracked_info = Vec::new();

    for file_path in &state.tracked_files {
        let path_str = file_path.to_string_lossy();
        match file_manager.read_file(&path_str).await {
            Ok(content) => {
                let size = content.len();
                total_tracked_size += size;
                tracked_info.push((path_str.to_string(), size));
            }
            Err(e) => {
                tracked_info.push((format!("{} (ERROR: {:?})", path_str, e), 0));
            }
        }
    }

    // Sort by size descending
    tracked_info.sort_by(|a, b| b.1.cmp(&a.1));

    for (path, size) in tracked_info {
        message.push_str(&format!("    - {} ({} bytes)\n", path, size));
    }

    message.push_str(&format!(
        "\n  Total tracked files size: {} bytes\n",
        total_tracked_size
    ));

    // Show totals
    message.push_str("\n\nTOTAL CONTEXT SIZE:\n");
    let total_size = dir_list_size + total_tracked_size;
    message.push_str(&format!(
        "  {} bytes ({:.2} KB)\n",
        total_size,
        total_size as f64 / 1024.0
    ));

    if total_size > 100_000 {
        message.push_str("\n  ⚠️ WARNING: Context is very large (>100KB). Consider:\n");
        message.push_str("     - Clearing tracked files with /clear\n");
        message.push_str("     - Adding more patterns to .gitignore\n");
        message.push_str("     - Working in a subdirectory\n");
    }

    vec![create_message(message, MessageSender::System)]
}

async fn handle_fileapi_command(state: &mut ActorState, parts: &[&str]) -> Vec<ChatMessage> {
    if let Some(api_name) = parts.get(1) {
        match api_name.to_lowercase().as_str() {
            "patch" => {
                state.config.file_modification_api = FileModificationApi::Patch;
                vec![create_message(
                    "File modification API set to: patch".to_string(),
                    MessageSender::Error,
                )]
            }
            "findreplace" | "find-replace" => {
                state.config.file_modification_api = FileModificationApi::FindReplace;
                vec![create_message(
                    "File modification API set to: find-replace".to_string(),
                    MessageSender::Error,
                )]
            }
            _ => vec![create_message(
                "Unknown file API. Use: patch, findreplace".to_string(),
                MessageSender::Error,
            )],
        }
    } else {
        let current_api = match state.config.file_modification_api {
            FileModificationApi::Patch => "patch",
            FileModificationApi::FindReplace => "find-replace",
        };
        vec![create_message(
            format!(
                "Current file modification API: {}. Usage: /fileapi <patch|findreplace>",
                current_api
            ),
            MessageSender::System,
        )]
    }
}

async fn handle_settings_command(state: &ActorState) -> Vec<ChatMessage> {
    let mut message = String::new();
    message.push_str("=== Current Settings ===\n\n");

    // Config settings
    let current_api = match state.config.file_modification_api {
        FileModificationApi::Patch => "patch",
        FileModificationApi::FindReplace => "find-replace",
    };
    message.push_str(&format!("FILE API: {}\n", current_api));
    message.push_str(&format!(
        "TRACE LOGGING: {}\n",
        if state.config.trace {
            "enabled"
        } else {
            "disabled"
        }
    ));

    // Add provider and security info from ActorState
    let settings = state.settings.settings();
    message.push_str(&format!(
        "\nACTIVE PROVIDER: {}\n",
        settings.active_provider
    ));
    message.push_str(&format!("SECURITY MODE: {:?}\n", settings.security.mode));

    vec![create_message(message, MessageSender::System)]
}

async fn handle_security_command(_state: &EventSender, parts: &[&str]) -> Vec<ChatMessage> {
    if parts.len() < 2 {
        return vec![create_message(
            "Security commands:\n\
              /security mode [all|auto|readonly] - Set security mode\n\
              /security status - Show current security settings"
                .to_string(),
            MessageSender::System,
        )];
    }

    match parts[1] {
        "mode" => {
            if let Some(mode_str) = parts.get(2) {
                vec![create_message(
                    format!(
                        "Security mode changes must be made through the settings file.\n\
                         Requested mode: {}",
                        mode_str
                    ),
                    MessageSender::Error,
                )]
            } else {
                vec![create_message(
                    "Security mode information is available via /settings command".to_string(),
                    MessageSender::System,
                )]
            }
        }

        "status" => {
            vec![create_message(
                "Security status information is available via /settings command".to_string(),
                MessageSender::System,
            )]
        }
        _ => vec![create_message(
            format!("Unknown security subcommand: {}", parts[1]),
            MessageSender::System,
        )],
    }
}

async fn handle_cost_command(state: &ActorState) -> Vec<ChatMessage> {
    let usage = &state.session_token_usage;
    let current_model = current_agent(state).agent.default_model().model;

    let mut message = String::new();
    message.push_str("=== Session Cost Summary ===\n\n");
    message.push_str(&format!("Current Model: {:?}\n", current_model));
    message.push_str(&format!("Provider: {}\n\n", state.provider.name()));

    message.push_str("Token Usage:\n");
    message.push_str(&format!("  Input tokens:  {:>8}\n", usage.input_tokens));
    message.push_str(&format!("  Output tokens: {:>8}\n", usage.output_tokens));
    message.push_str(&format!("  Total tokens:  {:>8}\n\n", usage.total_tokens));

    message.push_str("Accumulated Cost:\n");
    message.push_str(&format!("  Total cost: ${:.6}\n", state.session_cost));

    if usage.total_tokens > 0 {
        let avg_cost_per_1k = (state.session_cost / usage.total_tokens as f64) * 1000.0;
        message.push_str(&format!(
            "  Average per 1K tokens: ${:.6}\n",
            avg_cost_per_1k
        ));
    }

    vec![create_message(message, MessageSender::System)]
}

async fn handle_help_command() -> Vec<ChatMessage> {
    let commands = get_available_commands();
    let mut message = String::from("Available commands:\n\n");

    for cmd in commands {
        message.push_str(&format!("/{} - {}\n", cmd.name, cmd.description));
        message.push_str(&format!("  Usage: {}\n\n", cmd.usage));
    }
    vec![create_message(message, MessageSender::System)]
}

async fn handle_models_command(state: &ActorState) -> Vec<ChatMessage> {
    let models = state.provider.supported_models();
    let model_names: Vec<String> = if models.is_empty() {
        vec![Model::GrokCodeFast1.name().to_string()]
    } else {
        models.iter().map(|m| m.name().to_string()).collect()
    };
    let response = model_names.join(", ");
    vec![create_message(response, MessageSender::System)]
}

async fn handle_model_command(state: &mut ActorState, parts: &[&str]) -> Vec<ChatMessage> {
    if parts.len() < 2 {
        return vec![create_message(
            "Usage: /model <name> [key=value...]\nValid keys: temperature, max_tokens, top_p, reasoning_budget\nUse /models to list available models.".to_string(),
            MessageSender::System,
        )];
    }

    let model_name = parts[1];
    let model = match Model::from_name(model_name) {
        Some(m) => m,
        None => {
            return vec![create_message(
                format!(
                    "Unknown model: {}. Use /models to list available models.",
                    model_name
                ),
                MessageSender::Error,
            )];
        }
    };

    let settings = match parse_model_settings_overrides(&model, &parts[2..]) {
        Ok(s) => s,
        Err(e) => return vec![create_message(e, MessageSender::Error)],
    };

    // Set for all agents
    let agent_names: Vec<String> = AgentCatalog::get_agent_names();
    for agent_name in agent_names {
        state
            .settings
            .settings_mut()
            .set_agent_model(agent_name, settings.clone());
    }

    // Save
    if let Err(e) = state.settings.save() {
        return vec![create_message(
            format!("Failed to save settings: {}", e),
            MessageSender::System,
        )];
    }

    // Success message
    let mut overrides = Vec::new();
    if settings.temperature.is_some() {
        overrides.push(format!("temperature={}", settings.temperature.unwrap()));
    }
    if settings.max_tokens.is_some() {
        overrides.push(format!("max_tokens={}", settings.max_tokens.unwrap()));
    }
    if settings.top_p.is_some() {
        overrides.push(format!("top_p={}", settings.top_p.unwrap()));
    }
    overrides.push(format!("reasoning_budget={}", settings.reasoning_budget));

    let overrides_str = if overrides.is_empty() {
        "".to_string()
    } else {
        format!(" (with {})", overrides.join(", "))
    };

    vec![create_message(
        format!(
            "Model successfully set to {} for all agents{}.",
            model.name(),
            overrides_str
        ),
        MessageSender::System,
    )]
}

async fn handle_agentmodel_command(state: &mut ActorState, parts: &[&str]) -> Vec<ChatMessage> {
    if parts.len() < 3 {
        return vec![create_message(format!("Usage: /agentmodel <agent_name> <model_name> [temperature=0.7] [max_tokens=4096] [top_p=1.0] [reasoning_budget=...]\nValid agents: {}", AgentCatalog::get_agent_names().join(", ")), MessageSender::System)];
    }
    let agent_name = parts[1];
    if !AgentCatalog::get_agent_names().contains(&agent_name.to_string()) {
        return vec![create_message(
            format!(
                "Unknown agent: {}. Valid agents: {}",
                agent_name,
                AgentCatalog::get_agent_names().join(", ")
            ),
            MessageSender::Error,
        )];
    }
    let model_name = parts[2];
    let model = match Model::from_name(model_name) {
        Some(m) => m,
        None => {
            return vec![create_message(
                format!(
                    "Unknown model: {}. Use /models to list available models.",
                    model_name
                ),
                MessageSender::Error,
            )]
        }
    };
    let settings = match parse_model_settings_overrides(&model, &parts[3..]) {
        Ok(s) => s,
        Err(e) => return vec![create_message(e, MessageSender::Error)],
    };
    state
        .settings
        .settings_mut()
        .set_agent_model(agent_name.to_string(), settings.clone());
    if let Err(e) = state.settings.save() {
        return vec![create_message(
            format!("Failed to save settings: {}", e),
            MessageSender::System,
        )];
    }
    // Collect overrides for message
    let mut overrides = Vec::new();
    if let Some(v) = settings.temperature {
        overrides.push(format!("temperature={}", v));
    }
    if let Some(v) = settings.max_tokens {
        overrides.push(format!("max_tokens={}", v));
    }
    if let Some(v) = settings.top_p {
        overrides.push(format!("top_p={}", v));
    }
    overrides.push(format!("reasoning_budget={}", settings.reasoning_budget));

    let overrides_str = if overrides.is_empty() {
        "".to_string()
    } else {
        format!(" (with {})", overrides.join(", "))
    };
    vec![create_message(
        format!(
            "Model successfully set to {} for agent {}{}.",
            model.name(),
            agent_name,
            overrides_str
        ),
        MessageSender::System,
    )]
}

fn parse_model_settings_overrides(
    model: &Model,
    overrides: &[&str],
) -> Result<ModelSettings, String> {
    let mut settings = model.default_settings();
    for &arg in overrides {
        let eq_pos = arg
            .find('=')
            .ok_or(format!("Invalid argument: {}. Expected key=value", arg))?;
        let key = &arg[..eq_pos];
        let value_str = &arg[eq_pos + 1..];
        match key {
            "temperature" => {
                let v: f32 = value_str.parse().map_err(|_| format!("Invalid temperature value: {}. Expected a float (e.g., 0.7).", value_str))?;
                settings.temperature = Some(v);
            }
            "max_tokens" => {
                let v: u32 = value_str.parse().map_err(|_| format!("Invalid max_tokens value: {}. Expected a positive integer (e.g., 4096).", value_str))?;
                settings.max_tokens = Some(v);
            }
            "top_p" => {
                let v: f32 = value_str.parse().map_err(|_| format!("Invalid top_p value: {}. Expected a float (e.g., 1.0).", value_str))?;
                settings.top_p = Some(v);
            }
            "reasoning_budget" => {
                let reasoning_budget = match value_str {
                    "High" | "high" => ReasoningBudget::High,
                    "Low" | "low" => ReasoningBudget::Low,
                    "Off" | "off" => ReasoningBudget::Off,
                    _ => return Err("Unsupported reasoning budget - must be one of high low or off".to_string())
                };
                settings.reasoning_budget = reasoning_budget;
            }
            _ => return Err(format!("Unknown parameter: {}. Valid parameters: temperature, max_tokens, top_p, reasoning_budget", key)),
        }
    }
    Ok(settings)
}

fn create_message(content: String, sender: MessageSender) -> ChatMessage {
    ChatMessage {
        content,
        sender,
        timestamp: Utc::now().timestamp_millis() as u64,
        reasoning: None,
        tool_calls: Vec::new(),
        model_info: None,
        context_info: None,
        token_usage: None,
    }
}

async fn handle_agent_command(state: &mut ActorState, parts: &[&str]) -> Vec<ChatMessage> {
    if parts.len() < 2 {
        return vec![create_message(
            format!(
                "Usage: /agent <name>. Valid agents: {}",
                AgentCatalog::get_agent_names().join(", ")
            ),
            MessageSender::System,
        )];
    }

    let agent_name = parts[1];

    if !AgentCatalog::get_agent_names().contains(&agent_name.to_string()) {
        return vec![create_message(
            format!(
                "Unknown agent: {}. Valid agents: {}",
                agent_name,
                AgentCatalog::get_agent_names().join(", ")
            ),
            MessageSender::System,
        )];
    }

    // Check for sub-agents: block switch if sub-agents are active
    if state.agent_stack.len() > 1 {
        return vec![create_message(
            "Cannot switch agent while sub-agents are active.".to_string(),
            MessageSender::System,
        )];
    }

    // Check if already on the agent to avoid unnecessary switching
    if ai::current_agent(state).agent.name() == agent_name {
        return vec![create_message(
            format!("Already switched to agent: {}", agent_name),
            MessageSender::System,
        )];
    }

    // Preserve conversation from current agent before switching
    let old_conversation = ai::current_agent(state).conversation.clone();

    // Create new root agent and replace the current one
    let new_agent_dyn = AgentCatalog::create_agent(agent_name).unwrap();
    let mut new_root_agent = crate::agents::agent::ActiveAgent::new(new_agent_dyn);
    new_root_agent.conversation = old_conversation;
    new_root_agent.spawn_tool_use_id = None;
    state.agent_stack[0] = new_root_agent;

    vec![create_message(
        format!("Switched to agent: {}", agent_name),
        MessageSender::System,
    )]
}
