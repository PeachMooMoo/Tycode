use crate::base_app::BaseApp;
use crate::event_handler::EventFormatter;
use crate::formatter::Formatter;
use anyhow::Result;
use rustyline::DefaultEditor;
use std::sync::Arc;
use tokio::sync::broadcast;
use tycode_core::ai::bedrock::BedrockProvider;
use tycode_core::ai::types::ModelSettings;
use tycode_core::chat::events::{ChatEvent, MessageSender};
use tycode_core::settings::SettingsManager;

pub struct InteractiveApp {
    base: BaseApp,
    formatter: Formatter,
}

impl InteractiveApp {
    pub async fn new(
        provider: BedrockProvider,
        tunings: ModelSettings,
        workspace_roots: Option<Vec<std::path::PathBuf>>,
        settings: Option<Arc<SettingsManager>>,
    ) -> Result<Self> {
        let base = BaseApp::new(provider, tunings, workspace_roots, settings).await?;
        let formatter = Formatter::new();

        // Display welcome message
        let settings_info = if let Some(ref settings_mgr) = base.settings {
            format!("📁 Settings: {}\n", settings_mgr.path().display())
        } else {
            String::new()
        };

        let welcome_message = format!(
            "{}💡 Type /help for commands, /settings to view configuration, /quit to exit",
            settings_info
        );

        formatter.print_system(&welcome_message);

        Ok(Self { base, formatter })
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
            self.base.send_message(input.to_string()).await?;

            // Process AI response - wait for it to complete
            self.wait_for_response().await?;
        }

        println!("\nGoodbye!");
        Ok(())
    }

    async fn wait_for_response(&mut self) -> Result<()> {
        loop {
            match self.base.event_rx.recv().await {
                Ok(event) => {
                    // Check if this completes the response
                    let is_complete = match &event {
                        ChatEvent::MessageAdded(message) => match message.sender {
                            MessageSender::Assistant if message.tool_calls.is_empty() => true,
                            MessageSender::Error => true,
                            MessageSender::System => true,
                            _ => false,
                        },
                        _ => false,
                    };

                    // Format the event
                    self.format_event(event)?;

                    if is_complete {
                        break;
                    }
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

    fn print_help(&self) {
        self.formatter.print_divider();
        self.formatter.print_system("📚 Available Commands:");
        self.formatter.print_system("");

        // Get all available commands from the command handler
        let commands = self.base.command_handler.get_available_commands();

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

    fn print_settings(&self) {
        self.formatter.print_divider();
        self.formatter.print_system("📋 Current Settings:");
        self.formatter.print_system("");

        if let Some(ref settings_mgr) = self.base.settings {
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

impl EventFormatter for InteractiveApp {
    fn format_event(&mut self, event: ChatEvent) -> Result<()> {
        match event {
            ChatEvent::MessageAdded(message) => match message.sender {
                MessageSender::Assistant => {
                    // Display reasoning first if present
                    if let Some(ref reasoning) = message.reasoning {
                        self.formatter
                            .print_system(&format!("💭 Reasoning: {}", reasoning.text));
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
                MessageSender::User => {
                    // Skip printing user messages - they're already visible from input
                }
            },
            ChatEvent::TypingStatusChanged(_) => {
                // Ignore typing status - not useful
            }
            _ => {}
        }
        Ok(())
    }
}
