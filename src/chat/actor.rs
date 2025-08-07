use crate::agents::{ActiveAgent, Agent, SoftwareEngineerAgent, ToolType};
use crate::ai::{
    bedrock::BedrockProvider,
    provider::AiProvider,
    types::{Content, ContentBlock, ConversationRequest, Message, MessageRole},
};
use crate::chat::{
    commands::CommandHandler,
    events::{ChatMessage, MessageSender},
    state::SharedChatState,
};
use crate::tools::registry::ToolRegistry;
use anyhow::Result;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{error, info};

pub enum ChatActorMessage {
    UserInput(String),
    ProcessCommand(String),
    ContinueConversation,
    Shutdown,
}

pub struct ChatActor {
    state: SharedChatState,
    provider: BedrockProvider,
    workspace_root: PathBuf,
    agent_stack: Vec<ActiveAgent>,
    rx: mpsc::UnboundedReceiver<ChatActorMessage>,
    command_handler: CommandHandler,
}

impl ChatActor {
    pub fn new(
        state: SharedChatState,
        provider: BedrockProvider,
        workspace_root: PathBuf,
        rx: mpsc::UnboundedReceiver<ChatActorMessage>,
    ) -> Self {
        let command_handler = CommandHandler::new(state.clone());

        // Initialize with software engineer agent
        let mut agent_stack = Vec::new();
        agent_stack.push(ActiveAgent::new(Box::new(SoftwareEngineerAgent)));

        Self {
            state,
            provider,
            workspace_root,
            agent_stack,
            rx,
            command_handler,
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
        });

        self.current_agent_mut().conversation.push(Message {
            role: MessageRole::User,
            content: Content::text_only(input),
        });

        self.send_ai_request().await
    }

    async fn handle_command(&mut self, command: &str) {
        let messages = self.command_handler.handle_command(command, Some(&self.provider)).await;

        for message in messages {
            if message.content == "Conversation cleared." {
                self.current_agent_mut().conversation.clear();
            }
            self.add_message(message);
        }
    }

    fn get_tool_registry(&self) -> ToolRegistry {
        let file_modification_api = self.state.get_file_modification_api();
        ToolRegistry::new(self.workspace_root.clone(), file_modification_api)
    }

    async fn send_ai_request(&mut self) -> Result<()> {
        self.state.set_typing(true);

        loop {
            let current = self.current_agent();
            let allowed_tools: HashSet<ToolType> =
                current.agent.available_tools().into_iter().collect();

            let tool_registry = self.get_tool_registry();

            // Get tool definitions for the agent's allowed tool types
            let allowed_tool_types: Vec<ToolType> = allowed_tools.into_iter().collect();
            let available_tools = tool_registry.get_tool_definitions_for_types(&allowed_tool_types);

            let conversation = self.current_agent().conversation.clone();
            let model = current
                .agent
                .preferred_model()
                .unwrap_or(self.state.get_model());
            let system_prompt = current.agent.system_prompt().to_string();
            let tunings = self.state.get_tunings();

            let request = ConversationRequest {
                messages: conversation,
                model,
                system_prompt,
                tunings: tunings.clone(),
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
                                content: vec![ContentBlock::ToolResult(result)].into(),
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
        });
    }
}