use crate::ai::{bedrock::BedrockProvider, types::ModelSettings};
use crate::chat::{
    actor::{ChatActor, ChatActorMessage},
    commands::CommandHandler,
    events::{ChatEvent, MessageSender},
    state::SharedChatState,
};
use crate::cli::formatter::Formatter;
use crate::settings::SettingsManager;
use crate::terminal::splash::TYCODE_ASCII;
use anyhow::Result;
use rustyline::DefaultEditor;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

pub struct CliApp {
    actor_tx: mpsc::UnboundedSender<ChatActorMessage>,
    formatter: Formatter,
    event_rx: broadcast::Receiver<ChatEvent>,
    command_handler: CommandHandler,
    settings: Option<Arc<SettingsManager>>,
}

impl CliApp {
    pub async fn new(provider: BedrockProvider, tunings: ModelSettings) -> Result<Self> {
        Self::with_settings(provider, tunings, None).await
    }

    pub async fn with_settings(
        provider: BedrockProvider,
        _tunings: ModelSettings,
        settings: Option<Arc<SettingsManager>>,
    ) -> Result<Self> {
        // Create shared chat state
        let chat_state = SharedChatState::new();

        // Create actor communication channel
        let (actor_tx, actor_rx) = mpsc::unbounded_channel();

        // Get workspace root
        let workspace_root = std::env::current_dir()?;

        // Create and spawn the chat actor with settings
        let actor = ChatActor::with_settings(
            chat_state.clone(),
            provider,
            workspace_root,
            actor_rx,
            settings.clone(),
        );
        tokio::spawn(async move {
            actor.run().await;
        });

        // Subscribe to events but don't spawn a separate task
        let event_rx = chat_state.subscribe();
        let formatter = Formatter::new();

        // Create command handler for getting command info
        let command_handler = CommandHandler::new(chat_state.clone());

        // Display welcome message with proper ASCII art from splash.rs
        println!();

        // Display TYCODE ASCII art in yellow
        formatter.print_splash_art(TYCODE_ASCII, "\x1b[33m");

        println!();
        formatter.print_divider();
        println!();

        // Create the welcome message with settings info
        let settings_info = if let Some(ref settings_mgr) = settings {
            format!("📁 Settings: {}\n", settings_mgr.path().display())
        } else {
            String::new()
        };

        let welcome_message = format!(
            "{}💡 Type /help for commands, /settings to view configuration, /quit to exit",
            settings_info
        );

        // Display the welcome message immediately (only once)
        formatter.print_system(&welcome_message);

        Ok(Self {
            actor_tx,
            formatter,
            event_rx,
            command_handler,
            settings,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut rl = DefaultEditor::new()?;

        loop {
            // Wait for user input
            let prompt = self.formatter.print_prompt();
            let line = match rl.readline(&prompt) {
                Ok(line) => line,
                Err(_) => break, // Ctrl-D or error
            };

            let input = line.trim();
            if input.is_empty() {
                continue;
            }

            // Handle commands
            if input == "/quit" || input == "/exit" {
                break;
            }

            if input == "/help" {
                self.print_help();
                continue;
            }

            if input == "/settings" {
                self.print_settings();
                continue;
            }

            // Add to history
            rl.add_history_entry(&line)?;

            // Send input to chat actor
            let _ = self
                .actor_tx
                .send(ChatActorMessage::UserInput(input.to_string()));

            // Process AI response - wait for it to complete
            self.wait_for_response().await?;
        }

        // Shutdown the actor
        let _ = self.actor_tx.send(ChatActorMessage::Shutdown);
        println!("\nGoodbye!");
        Ok(())
    }

    /// Wait for the AI to finish responding
    async fn wait_for_response(&mut self) -> Result<()> {
        let mut ai_started = false;
        let mut ai_finished = false;

        while !ai_finished {
            match self.event_rx.recv().await {
                Ok(event) => {
                    match &event {
                        ChatEvent::TypingStatusChanged(typing) => {
                            if *typing {
                                ai_started = true;
                            } else if ai_started {
                                ai_finished = true;
                            }
                        }
                        ChatEvent::MessageAdded(message) => match message.sender {
                            MessageSender::Assistant => {
                                ai_started = true;
                                if message.tool_calls.is_empty() {
                                    ai_finished = true;
                                }
                            }
                            MessageSender::Error => {
                                ai_finished = true;
                            }
                            MessageSender::System => {
                                if !ai_started {
                                    ai_finished = true;
                                }
                            }
                            _ => {}
                        },
                        _ => {}
                    }
                    self.handle_event(event).await?;
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle a chat event
    async fn handle_event(&mut self, event: ChatEvent) -> Result<()> {
        match event {
            ChatEvent::MessageAdded(message) => {
                // Display the message using the formatter
                match message.sender {
                    MessageSender::User => {
                        self.formatter.print_user(&message.content);
                    }
                    MessageSender::Assistant => {
                        // Display reasoning first if present
                        if let Some(reasoning) = &message.reasoning {
                            self.formatter
                                .print_system(&format!("💭 Reasoning: {}", reasoning));
                        }

                        // Display the response with model info if available
                        if let Some(ref model_info) = message.model_info {
                            self.formatter
                                .print_ai_with_model(&message.content, model_info);
                        } else {
                            self.formatter.print_ai(&message.content);
                        }

                        // Display tool calls if present
                        for tool_call in &message.tool_calls {
                            self.formatter
                                .print_tool_call(&tool_call.name, &tool_call.arguments);
                        }
                    }
                    MessageSender::System => {
                        self.formatter.print_system(&message.content);
                    }
                    MessageSender::Error => {
                        self.formatter.print_error(&message.content);
                    }
                }
            }
            ChatEvent::TypingStatusChanged(typing) => {
                if typing {
                    self.formatter.print_thinking();
                }
            }
            _ => {
                // Handle other events if needed
            }
        }
        Ok(())
    }

    /// Print help information
    fn print_help(&self) {
        self.formatter.print_divider();
        self.formatter.print_system("📚 Available Commands:");
        self.formatter.print_system("");

        // Get all available commands from the command handler
        let commands = self.command_handler.get_available_commands();

        // Find the maximum command name length for alignment
        let max_name_len = commands.iter().map(|cmd| cmd.name.len()).max().unwrap_or(0);

        // Display each command with aligned formatting
        for command in commands {
            let padding = " ".repeat(max_name_len - command.name.len());
            self.formatter.print_system(&format!(
                "  /{}{} - {}",
                command.name, padding, command.description
            ));
            if !command.usage.is_empty() && command.usage != format!("/{}", command.name) {
                self.formatter.print_system(&format!(
                    "    {}  Usage: {}",
                    " ".repeat(max_name_len),
                    command.usage
                ));
            }
        }

        self.formatter.print_system("");
        self.formatter
            .print_system("💬 Just type your message and press Enter to chat with the AI.");
        self.formatter.print_divider();
    }

    /// Print current settings information
    fn print_settings(&self) {
        self.formatter.print_divider();
        self.formatter.print_system("📋 Current Settings:");
        self.formatter.print_system("");

        if let Some(ref settings_mgr) = self.settings {
            let settings = settings_mgr.settings();

            // Global settings
            self.formatter.print_system("  Global:");
            self.formatter.print_system(&format!(
                "    File API: {:?}",
                settings.global.file_modification_api
            ));
            self.formatter
                .print_system(&format!("    Trace: {}", settings.global.trace));
            self.formatter.print_system("");

            // Provider settings
            self.formatter.print_system("  Providers:");
            self.formatter.print_system("    Bedrock:");
            if let Some(ref profile) = settings.providers.bedrock.profile {
                self.formatter
                    .print_system(&format!("      Profile: {}", profile));
            } else {
                self.formatter.print_system("      Profile: <default>");
            }
            self.formatter.print_system("");

            // Agent settings
            if !settings.agents.is_empty() {
                self.formatter.print_system("  Agent Overrides:");
                for (agent_name, agent_settings) in &settings.agents {
                    self.formatter.print_system(&format!("    {}:", agent_name));

                    self.formatter
                        .print_system(&format!("      Model: {}", agent_settings.model.name()));

                    if let Some(temp) = agent_settings.temperature {
                        self.formatter
                            .print_system(&format!("      Temperature: {}", temp));
                    }
                    if let Some(max_tokens) = agent_settings.max_tokens {
                        self.formatter
                            .print_system(&format!("      Max Tokens: {}", max_tokens));
                    }
                    if let Some(reasoning_budget) = agent_settings.reasoning_budget {
                        self.formatter
                            .print_system(&format!("      Reasoning Budget: {}", reasoning_budget));
                    }
                }
            } else {
                self.formatter
                    .print_system("  No agent-specific overrides configured");
            }

            self.formatter.print_system("");
            self.formatter.print_system(&format!(
                "  Settings file: {}",
                settings_mgr.path().display()
            ));
            self.formatter
                .print_system("  Edit the file directly to modify settings");
        } else {
            self.formatter.print_system("  No settings file loaded");
            self.formatter
                .print_system("  Using command-line arguments and defaults");
        }

        self.formatter.print_divider();
    }
}
