use crate::chat::{
    actor::ActorState,
    ai::{self, current_agent},
    events::{ChatMessage, MessageSender},
    state::{FileModificationApi, SharedChatState},
};
use std::time::Instant;

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
        "settings" => handle_settings_command(state).await,
        "security" => handle_security_command(&state.state, &parts).await,
        "cost" => handle_cost_command(state).await,
        "help" => handle_help_command().await,
        _ => vec![create_message(
            format!("Unknown command: /{}", parts[0]),
            MessageSender::System,
        )],
    }
}

/// Get all available commands with their descriptions
pub fn get_available_commands() -> Vec<CommandInfo> {
    vec![
        CommandInfo {
            name: "clear".to_string(),
            description: "Clear the conversation history".to_string(),
            usage: "/clear".to_string(),
        },
        CommandInfo {
            name: "context".to_string(),
            description: "Show what files would be included in the AI context".to_string(),
            usage: "/context".to_string(),
        },
        CommandInfo {
            name: "fileapi".to_string(),
            description: "Set the file modification API (patch or find-replace)".to_string(),
            usage: "/fileapi <patch|findreplace>".to_string(),
        },
        CommandInfo {
            name: "trace".to_string(),
            description: "Enable/disable trace logging to .tycode/trace".to_string(),
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
            name: "quit".to_string(),
            description: "Exit the application".to_string(),
            usage: "/quit or /exit".to_string(),
        },
    ]
}

async fn handle_clear_command(state: &mut ActorState) -> Vec<ChatMessage> {
    state.state.clear_conversation();
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
    message.push_str("=== AI Context Debug Info ===\n\n");

    // Show directory listing info
    message.push_str("DIRECTORY LISTING (file paths only):\n");
    message.push_str(&format!("  Total files: {}\n", relevant.files.len()));
    if relevant.truncated {
        message.push_str("  ⚠️ WARNING: Directory listing was TRUNCATED (too many files)\n");
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
                    MessageSender::System,
                )]
            }
            "findreplace" | "find-replace" => {
                state.config.file_modification_api = FileModificationApi::FindReplace;
                vec![create_message(
                    "File modification API set to: find-replace".to_string(),
                    MessageSender::System,
                )]
            }
            _ => vec![create_message(
                "Unknown file API. Use: patch, findreplace".to_string(),
                MessageSender::System,
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

async fn handle_security_command(_state: &SharedChatState, parts: &[&str]) -> Vec<ChatMessage> {
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
                    MessageSender::System,
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
    let current_model = current_agent(state).agent.preferred_model().model;

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

fn create_message(content: String, sender: MessageSender) -> ChatMessage {
    ChatMessage {
        content,
        sender,
        timestamp: Instant::now(),
        reasoning: None,
        tool_calls: Vec::new(),
        model_info: None,
        context_info: None,
        token_usage: None,
    }
}
