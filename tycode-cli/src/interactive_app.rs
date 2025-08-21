use crate::base_app::BaseApp;
use crate::event_handler::EventFormatter;
use crate::formatter::Formatter;
use anyhow::Result;
use rustyline::DefaultEditor;
use std::path::PathBuf;
use tokio::sync::broadcast;
use tycode_core::chat::events::{ChatEvent, MessageSender};

pub struct InteractiveApp {
    base: BaseApp,
    formatter: Formatter,
}

impl InteractiveApp {
    pub async fn new(
        workspace_roots: Option<Vec<PathBuf>>,
        settings_path: Option<PathBuf>,
    ) -> Result<Self> {
        let base = BaseApp::new(workspace_roots, settings_path).await?;
        let formatter = Formatter::new();

        let welcome_message =
            "💡 Type /help for commands, /settings to view configuration, /quit to exit";

        formatter.print_system(welcome_message);

        Ok(Self { base, formatter })
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut rl = DefaultEditor::new()?;

        loop {
            let prompt = self.formatter.print_prompt();
            let line = match rl.readline(&prompt) {
                Ok(line) => line,
                Err(_) => break,
            };

            let input = line.trim();
            if input.is_empty() {
                continue;
            }

            if input == "/quit" || input == "/exit" {
                break;
            }

            if input == "/help" {
                self.print_help();
                continue;
            }

            if input == "/settings" {
                self.print_settings().await;
                continue;
            }

            rl.add_history_entry(&line)?;
            self.base.send_message(input.to_string()).await?;
            self.wait_for_response().await?;
        }

        println!("\nGoodbye!");
        Ok(())
    }

    async fn wait_for_response(&mut self) -> Result<()> {
        loop {
            match self.base.event_rx.recv().await {
                Ok(event) => {
                    let is_complete = match &event {
                        ChatEvent::MessageAdded(message) => match message.sender {
                            MessageSender::Assistant if message.tool_calls.is_empty() => true,
                            MessageSender::Error => true,
                            MessageSender::System => true,
                            _ => false,
                        },
                        _ => false,
                    };

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

        let commands = tycode_core::chat::commands::get_available_commands();
        let max_name_len = commands.iter().map(|cmd| cmd.name.len()).max().unwrap_or(0);

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

    async fn print_settings(&mut self) {
        self.formatter.print_divider();
        self.formatter.print_system("📋 Current Settings:");
        self.formatter.print_system("");

        // Get settings from the actor
        let settings_json = match self.base.get_settings().await {
            Ok(s) => s,
            Err(e) => {
                self.formatter
                    .print_error(&format!("Failed to load settings: {}", e));
                return;
            }
        };

        let settings: tycode_core::settings::Settings = match serde_json::from_value(settings_json)
        {
            Ok(s) => s,
            Err(e) => {
                self.formatter
                    .print_error(&format!("Failed to parse settings: {}", e));
                return;
            }
        };

        self.formatter
            .print_system(&format!("  Active Provider: {}", settings.active_provider));
        self.formatter.print_system("");

        if !settings.providers.is_empty() {
            self.formatter.print_system("  Configured Providers:");
            for (name, config) in &settings.providers {
                let is_active = name == &settings.active_provider;
                let marker = if is_active { " (active)" } else { "" };

                self.formatter
                    .print_system(&format!("    {}{}:", name, marker));

                match config {
                    tycode_core::settings::ProviderConfig::Bedrock { profile, region } => {
                        self.formatter.print_system("      Type: AWS Bedrock");
                        self.formatter
                            .print_system(&format!("      Profile: {}", profile));
                        self.formatter
                            .print_system(&format!("      Region: {}", region));
                    }
                    tycode_core::settings::ProviderConfig::Mock { behavior } => {
                        self.formatter.print_system("      Type: Mock (Testing)");
                        self.formatter
                            .print_system(&format!("      Behavior: {:?}", behavior));
                    }
                }
            }
        } else {
            self.formatter
                .print_system("  No providers configured (using defaults)");
        }

        self.formatter.print_system("");
        self.formatter
            .print_system("  Use the VSCode extension to edit settings");

        self.formatter.print_divider();
    }
}

impl EventFormatter for InteractiveApp {
    fn format_event(&mut self, event: ChatEvent) -> Result<()> {
        match event {
            ChatEvent::MessageAdded(message) => match message.sender {
                MessageSender::Assistant => {
                    if let Some(ref reasoning) = message.reasoning {
                        self.formatter
                            .print_system(&format!("💭 Reasoning: {}", reasoning.text));
                    }

                    if let Some(ref model_info) = message.model_info {
                        self.formatter
                            .print_ai_with_model(&message.content, model_info);
                    } else {
                        self.formatter.print_ai(&message.content);
                    }

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
                MessageSender::User => {}
            },
            ChatEvent::TypingStatusChanged(_) => {}
            _ => {}
        }
        Ok(())
    }
}
