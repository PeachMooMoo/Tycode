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
