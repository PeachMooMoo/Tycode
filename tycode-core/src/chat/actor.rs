use crate::agents::{ActiveAgent, Agent, SoftwareEngineerAgent, ToolType};
use crate::ai::{
    bedrock::BedrockProvider,
    provider::AiProvider,
    types::{Content, ConversationRequest, Message, MessageContext, MessageRole},
};
use crate::ai::{ContentBlock, ModelSettings};
use crate::chat::{
    commands::CommandHandler,
    events::{ChatMessage, MessageSender, ModelInfo, ModelSource},
    state::SharedChatState,
};
use crate::settings::SettingsManager;
use crate::tools::context_utils::list_relevant_files;
use crate::tools::file_access::FileAccessManager;
use crate::tools::registry::ToolRegistry;
use anyhow::Result;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

pub enum ChatActorMessage {
    UserInput(String),
    ProcessCommand(String),
    ContinueConversation,
    Shutdown,
}

pub struct ChatActor {
    state: SharedChatState,
    provider: BedrockProvider,
    agent_stack: Vec<ActiveAgent>,
    rx: mpsc::UnboundedReceiver<ChatActorMessage>,
    command_handler: CommandHandler,
    settings: Option<Arc<SettingsManager>>,
}

impl ChatActor {
    pub fn new(
        state: SharedChatState,
        provider: BedrockProvider,
        rx: mpsc::UnboundedReceiver<ChatActorMessage>,
    ) -> Self {
        Self::with_settings(state, provider, rx, None)
    }

    pub fn with_settings(
        state: SharedChatState,
        provider: BedrockProvider,
        rx: mpsc::UnboundedReceiver<ChatActorMessage>,
        settings: Option<Arc<SettingsManager>>,
    ) -> Self {
        let command_handler = CommandHandler::new(state.clone());

        // Initialize with software engineer agent
        let mut agent_stack = Vec::new();
        agent_stack.push(ActiveAgent::new(Box::new(SoftwareEngineerAgent)));

        Self {
            state,
            provider,
            agent_stack,
            rx,
            command_handler,
            settings,
        }
    }

    fn current_agent(&self) -> &ActiveAgent {
        self.agent_stack.last().expect("No active agent")
    }

    fn current_agent_mut(&mut self) -> &mut ActiveAgent {
        self.agent_stack.last_mut().expect("No active agent")
    }

    pub fn push_agent(&mut self, agent: Box<dyn Agent>) {
        self.agent_stack.push(ActiveAgent::new(agent));
    }

    pub fn pop_agent(&mut self) -> Option<ActiveAgent> {
        if self.agent_stack.len() > 1 {
            self.agent_stack.pop()
        } else {
            None
        }
    }

    pub async fn run(mut self) {
        info!("ChatActor started");

        while let Some(message) = self.rx.recv().await {
            match message {
                ChatActorMessage::UserInput(input) => {
                    if let Err(e) = self.handle_user_input(input).await {
                        error!(?e, "Error handling user input");
                        self.add_error_message(format!("Error: {:?}", e));
                    }
                }
                ChatActorMessage::ProcessCommand(command) => {
                    self.handle_command(&command).await;
                }
                ChatActorMessage::ContinueConversation => {
                    if let Err(e) = self.send_ai_request().await {
                        error!(?e, "Error sending AI request");
                        self.add_error_message(format!("Error: {:?}", e));
                    }
                }
                ChatActorMessage::Shutdown => {
                    info!("ChatActor shutting down");
                    break;
                }
            }
        }
    }

    async fn handle_user_input(&mut self, input: String) -> Result<()> {
        if input.trim().is_empty() {
            return Ok(());
        }

        if let Some(command) = input.strip_prefix('/') {
            self.handle_command(command).await;
            return Ok(());
        }

        self.state.add_to_history(input.clone());

        self.add_message(ChatMessage {
            content: input.clone(),
            sender: MessageSender::User,
            timestamp: Instant::now(),
            reasoning: None,
            tool_calls: Vec::new(),
            model_info: None,
        });

        self.current_agent_mut().conversation.push(Message {
            role: MessageRole::User,
            content: Content::text_only(input),
        });

        self.send_ai_request().await
    }

    async fn handle_command(&mut self, command: &str) {
        let messages = self
            .command_handler
            .handle_command(command, Some(&self.provider))
            .await;

        for message in messages {
            if message.content == "Conversation cleared." {
                self.current_agent_mut().conversation.clear();
            }
            self.add_message(message);
        }
    }

    async fn build_message_context(&self) -> MessageContext {
        let working_dir = PathBuf::from(".");
        let mut context = MessageContext::new(working_dir.clone());

        // Get relevant files from the directory
        let relevant_files = list_relevant_files(&working_dir);
        context.set_relevant_files(relevant_files.files);

        // Load content of tracked files
        let tracked_files = self.state.get_tracked_files();
        let file_manager = FileAccessManager::new(working_dir);

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

    async fn send_ai_request(&mut self) -> Result<()> {
        self.state.set_typing(true);

        loop {
            let current = self.current_agent();
            let allowed_tools: HashSet<ToolType> =
                current.agent.available_tools().into_iter().collect();

            let file_modification_api = self.state.get_file_modification_api();
            let tool_registry = ToolRegistry::with_chat_state(
                ".".into(),
                file_modification_api,
                Some(Arc::new(self.state.clone())),
            );

            // Get tool definitions for the agent's allowed tool types
            let allowed_tool_types: Vec<ToolType> = allowed_tools.into_iter().collect();
            let available_tools = tool_registry.get_tool_definitions_for_types(&allowed_tool_types);

            // Build message context with tracked files
            let message_context = self.build_message_context().await;

            // Build messages for the request WITHOUT modifying the stored conversation
            let mut messages_for_request = Vec::new();

            // Only add context if we have tracked files or it's the first message
            let tracked_files = self.state.get_tracked_files();
            if !tracked_files.is_empty() || self.current_agent().conversation.is_empty() {
                let context_string = message_context.to_formatted_string();

                // Add context as the first message (not stored in conversation history)
                let context_message = Message {
                    role: MessageRole::User,
                    content: Content::text_only(format!("Current Context:\n{}", context_string)),
                };
                messages_for_request.push(context_message);
            }

            // Add the actual conversation history
            messages_for_request.extend(self.current_agent().conversation.clone());

            // Determine which model settings to use and track the source
            let (model_settings, model_source) = self.determine_model_settings_and_source(current);

            let system_prompt = current.agent.system_prompt().to_string();

            let request = ConversationRequest {
                messages: messages_for_request,
                model: model_settings.clone(),
                system_prompt,
                stop_sequences: vec![],
                tools: available_tools,
            };

            info!(?request, "AI request");

            match self.provider.converse(request).await {
                Ok(response) => {
                    let content = response.content.clone();

                    // Log detailed response information if trace is enabled
                    info!(?response, "AI response");

                    let reasoning = content.reasoning().first().map(|r| (*r).clone());
                    let tool_calls: Vec<_> =
                        content.tool_uses().iter().map(|t| (*t).clone()).collect();

                    self.add_message(ChatMessage {
                        content: content.text(),
                        sender: MessageSender::Assistant,
                        timestamp: Instant::now(),
                        reasoning,
                        tool_calls: tool_calls.clone(),
                        model_info: Some(ModelInfo {
                            model: model_settings.model,
                            source: model_source.clone(),
                        }),
                    });

                    self.current_agent_mut().conversation.push(Message {
                        role: MessageRole::Assistant,
                        content: content,
                    });

                    if !tool_calls.is_empty() {
                        info!(
                            tool_count = tool_calls.len(),
                            tools = ?tool_calls.iter().map(|t| &t.name).collect::<Vec<_>>(),
                            "Executing tool calls"
                        );

                        for tool_use in &tool_calls {
                            let result = tool_registry.execute_tool(tool_use).await;

                            // Log tool results if trace is enabled
                            info!(
                                tool_name = %tool_use.name,
                                ?result,
                                "Tool execution completed"
                            );

                            self.current_agent_mut().conversation.push(Message {
                                role: MessageRole::User,
                                content: vec![
                                    ContentBlock::ToolResult(result),
                                    ContentBlock::Text("Here is the tool result:".to_string()),
                                ]
                                .into(),
                            });
                        }
                        continue;
                    } else {
                        break;
                    }
                }
                Err(e) => {
                    error!(?e, "AI request failed");
                    self.add_error_message(format!("Error: {:?}", e));
                    break;
                }
            }
        }

        self.state.set_typing(false);
        Ok(())
    }

    fn add_message(&self, message: ChatMessage) {
        self.state.add_message(message);
    }

    fn add_error_message(&self, error: String) {
        self.add_message(ChatMessage {
            content: error,
            sender: MessageSender::Error,
            timestamp: Instant::now(),
            reasoning: None,
            tool_calls: Vec::new(),
            model_info: None,
        });
    }

    fn determine_model_settings_and_source(
        &self,
        agent: &ActiveAgent,
    ) -> (ModelSettings, ModelSource) {
        let agent_name = agent.agent.name();

        // First check if user has configured settings for this agent
        if let Some(settings) = &self.settings {
            if let Some(agent_settings) = settings.settings().get_agent_settings(agent_name) {
                return (
                    ModelSettings {
                        model: agent_settings.model,
                        max_tokens: agent_settings.max_tokens,
                        temperature: agent_settings.temperature,
                        top_p: agent_settings.top_p,
                        reasoning_budget: agent_settings.reasoning_budget,
                    },
                    ModelSource::UserConfigured,
                );
            }
        }

        // Otherwise use agent's preferred model settings
        (agent.agent.preferred_model(), ModelSource::AgentPreference)
    }
}
