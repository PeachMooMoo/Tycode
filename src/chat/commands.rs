use crate::ai::provider::AiProvider;
use crate::chat::{
    events::{ChatMessage, MessageSender},
    state::SharedChatState,
    rebuild_index_command::RebuildIndexCommand,
};
use std::time::Instant;

pub struct CommandHandler {
    state: SharedChatState,
}

impl CommandHandler {
    pub fn new(state: SharedChatState) -> Self {
        Self { state }
    }

    pub async fn handle_command(&self, command: &str, provider: Option<&dyn AiProvider>) -> Vec<ChatMessage> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return vec![];
        }

        match parts[0] {
            "clear" => self.handle_clear_command().await,
            "model" => self.handle_model_command(&parts).await,
            "reasoning" => self.handle_reasoning_command(&parts).await,
            "fileapi" => self.handle_fileapi_command(&parts).await,
            "trace" => self.handle_trace_command(&parts).await,
            "rebuild-index" => self.handle_rebuild_index_command(provider).await,
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

    async fn handle_model_command(&self, parts: &[&str]) -> Vec<ChatMessage> {
        if let Some(model_name) = parts.get(1) {
            if let Some(new_model) = crate::ai::types::Model::from_name(model_name) {
                self.state.set_model(new_model);
                vec![self.create_message(
                    format!("Switched to model: {}", model_name),
                    MessageSender::System,
                )]
            } else {
                vec![self.create_message(
                    format!("Unknown model: {}", model_name),
                    MessageSender::System,
                )]
            }
        } else {
            vec![self.create_message("Usage: /model <name>".to_string(), MessageSender::System)]
        }
    }

    async fn handle_reasoning_command(&self, parts: &[&str]) -> Vec<ChatMessage> {
        if let Some(budget_str) = parts.get(1) {
            if let Ok(budget) = budget_str.parse::<u32>() {
                let mut tunings = self.state.get_tunings();
                tunings.reasoning_budget = Some(budget);
                self.state.set_tunings(tunings);

                vec![self.create_message(
                    format!("Reasoning enabled with budget: {} tokens", budget),
                    MessageSender::System,
                )]
            } else {
                vec![self.create_message(
                    "Invalid reasoning budget".to_string(),
                    MessageSender::System,
                )]
            }
        } else {
            let mut tunings = self.state.get_tunings();
            tunings.reasoning_budget = None;
            self.state.set_tunings(tunings);

            vec![self.create_message("Reasoning disabled".to_string(), MessageSender::System)]
        }
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

    async fn handle_rebuild_index_command(&self, provider: Option<&dyn AiProvider>) -> Vec<ChatMessage> {
        if let Some(provider) = provider {
            let rebuild_command = RebuildIndexCommand::new(self.state.clone());
            rebuild_command.execute(provider).await
        } else {
            vec![self.create_message(
                "❌ AI provider not available for indexing".to_string(),
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
        }
    }
}
