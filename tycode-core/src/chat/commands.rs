use crate::ai::provider::AiProvider;
use crate::chat::{
    events::{ChatMessage, MessageSender},
    state::SharedChatState,
};
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct CommandInfo {
    pub name: String,
    pub description: String,
    pub usage: String,
}

pub struct CommandHandler {
    state: SharedChatState,
}

impl CommandHandler {
    pub fn new(state: SharedChatState) -> Self {
        Self { state }
    }

    /// Get all available commands with their descriptions
    pub fn get_available_commands(&self) -> Vec<CommandInfo> {
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

    pub async fn handle_command(
        &self,
        command: &str,
        _provider: Option<&dyn AiProvider>,
    ) -> Vec<ChatMessage> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return vec![];
        }

        match parts[0] {
            "clear" => self.handle_clear_command().await,
            "context" => self.handle_context_command().await,
            "fileapi" => self.handle_fileapi_command(&parts).await,
            "trace" => self.handle_trace_command(&parts).await,
            _ => vec![self.create_message(
                format!("Unknown command: /{}", command),
                MessageSender::System,
            )],
        }
    }

    async fn handle_clear_command(&self) -> Vec<ChatMessage> {
        self.state.clear_conversation();
        vec![self.create_message("Conversation cleared.".to_string(), MessageSender::System)]
    }

    async fn handle_context_command(&self) -> Vec<ChatMessage> {
        use crate::tools::context_utils::list_relevant_files;
        use crate::tools::file_access::FileAccessManager;
        use std::path::PathBuf;

        let working_dir = PathBuf::from(".");

        // Get relevant files (directory listing)
        let relevant = list_relevant_files(&[working_dir.clone()]);

        // Get tracked files
        let tracked_files = self.state.get_tracked_files();
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
        let mut dirs: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
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
        message.push_str(&format!("  Total tracked: {}\n", tracked_files.len()));

        let mut total_tracked_size = 0;
        let mut tracked_info = Vec::new();

        for file_path in &tracked_files {
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

        vec![self.create_message(message, MessageSender::System)]
    }

    async fn handle_fileapi_command(&self, parts: &[&str]) -> Vec<ChatMessage> {
        if let Some(api_name) = parts.get(1) {
            match api_name.to_lowercase().as_str() {
                "patch" => {
                    self.state
                        .set_file_modification_api(crate::chat::state::FileModificationApi::Patch);
                    vec![self.create_message(
                        "File modification API set to: patch".to_string(),
                        MessageSender::System,
                    )]
                }
                "findreplace" | "find-replace" => {
                    self.state.set_file_modification_api(
                        crate::chat::state::FileModificationApi::FindReplace,
                    );
                    vec![self.create_message(
                        "File modification API set to: find-replace".to_string(),
                        MessageSender::System,
                    )]
                }
                _ => vec![self.create_message(
                    "Unknown file API. Use: patch, findreplace".to_string(),
                    MessageSender::System,
                )],
            }
        } else {
            let current_api = match self.state.get_file_modification_api() {
                crate::chat::state::FileModificationApi::Patch => "patch",
                crate::chat::state::FileModificationApi::FindReplace => "find-replace",
            };
            vec![self.create_message(
                format!(
                    "Current file modification API: {}. Usage: /fileapi <patch|findreplace>",
                    current_api
                ),
                MessageSender::System,
            )]
        }
    }

    async fn handle_trace_command(&self, parts: &[&str]) -> Vec<ChatMessage> {
        if let Some(action) = parts.get(1) {
            match action.to_lowercase().as_str() {
                "on" | "enable" | "true" => {
                    if let Err(e) = self.setup_trace_logging().await {
                        vec![self.create_message(
                            format!("Failed to enable trace logging: {:?}", e),
                            MessageSender::System,
                        )]
                    } else {
                        self.state.set_trace(true);
                        vec![self.create_message(
                            "Trace logging enabled. Logs will be written to .tycode/trace"
                                .to_string(),
                            MessageSender::System,
                        )]
                    }
                }
                "off" | "disable" | "false" => {
                    self.state.set_trace(false);
                    vec![self.create_message(
                        "Trace logging disabled".to_string(),
                        MessageSender::System,
                    )]
                }
                _ => vec![self
                    .create_message("Usage: /trace <on|off>".to_string(), MessageSender::System)],
            }
        } else {
            let current_trace = self.state.get_trace();
            vec![self.create_message(
                format!(
                    "Trace logging: {}. Usage: /trace <on|off>",
                    if current_trace { "enabled" } else { "disabled" }
                ),
                MessageSender::System,
            )]
        }
    }

    async fn setup_trace_logging(&self) -> Result<(), Box<dyn std::error::Error>> {
        crate::chat::trace::setup_trace_logging()
    }

    fn create_message(&self, content: String, sender: MessageSender) -> ChatMessage {
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
}
