use crate::base_app::BaseApp;
use crate::event_handler::EventFormatter;
use crate::formatter::Formatter;
use anyhow::Result;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::path::PathBuf;
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
                Err(err) => match err {
                    ReadlineError::Interrupted => {
                        continue;
                    }
                    _ => break,
                },
            };

            let input = line.trim();
            if input.is_empty() {
                continue;
            }

            if input == "/quit" || input == "/exit" {
                break;
            }

            rl.add_history_entry(&line)?;
            self.base.send_message(input.to_string())?;
            self.wait_for_response().await?;
        }

        println!("\nGoodbye!");
        Ok(())
    }

    async fn wait_for_response(&mut self) -> Result<()> {
        use tokio::signal;
        loop {
            tokio::select! {
                recv = self.base.event_rx.recv() => {
                    match recv {
                        Some(event) => {
                            let is_complete = match &event {
                                ChatEvent::TypingStatusChanged(typing) => !*typing,
                                _ => false,
                            };
                            self.format_event(event)?;
                            if is_complete {
                                break;
                            }
                        }
                        None => {
                            break;
                        }
                    }
                }
                _ = signal::ctrl_c() => {
                    self.base.cancel()?;
                    continue;
                }
            }
        }

        Ok(())
    }
}

impl EventFormatter for InteractiveApp {
    fn format_event(&mut self, event: ChatEvent) -> Result<()> {
        match event {
            ChatEvent::MessageAdded(message) => match message.sender {
                MessageSender::Assistant { agent } => {
                    if let Some(ref reasoning) = message.reasoning {
                        self.formatter
                            .print_system(&format!("💭 Reasoning: {}", reasoning.text));
                    }

                    self.formatter.print_ai(
                        &message.content,
                        &agent,
                        &message.model_info,
                        &message.token_usage,
                    );

                    for tool_call in &message.tool_calls {
                        self.formatter
                            .print_formatted_tool_call(&tool_call.name, &tool_call.arguments);
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
            ChatEvent::Error(e) => self.formatter.print_error(&e),
            ChatEvent::ToolExecutionCompleted {
                tool_name,
                success,
                result,
                ui_data,
                error,
            } => {
                self.formatter.print_tool_result(
                    &tool_name,
                    success,
                    result.as_ref(),
                    ui_data.as_ref(),
                    error.as_deref(),
                );
            }
            _ => {}
        }
        Ok(())
    }
}
