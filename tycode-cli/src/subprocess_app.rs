use crate::base_app::BaseApp;
use crate::subprocess::SubprocessMessage;
use anyhow::Result;
use std::io::{BufRead, BufReader, Write};
use std::sync::Arc;
use tycode_core::ai::bedrock::BedrockProvider;
use tycode_core::ai::types::ModelSettings;
use tycode_core::chat::events::{ChatEvent, MessageSender};
use tycode_core::settings::SettingsManager;

pub struct SubprocessApp {
    base: BaseApp,
}

impl SubprocessApp {
    pub async fn new(
        provider: BedrockProvider,
        tunings: ModelSettings,
        workspace_roots: Option<Vec<std::path::PathBuf>>,
        settings: Option<Arc<SettingsManager>>,
    ) -> Result<Self> {
        let base = BaseApp::new(provider, tunings, workspace_roots, settings).await?;

        // Send ready signal
        let ready_msg = serde_json::to_string(&SubprocessMessage::Ready)?;
        println!("{}", ready_msg);
        std::io::stdout().flush()?;

        Ok(Self { base })
    }

    pub async fn run(&mut self) -> Result<()> {
        use std::sync::mpsc;
        use std::thread;

        // Create a channel for stdin messages
        let (stdin_tx, stdin_rx) = mpsc::channel::<String>();

        // Spawn a blocking thread to read stdin
        thread::spawn(move || {
            let stdin = std::io::stdin();
            let reader = BufReader::new(stdin);
            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        if stdin_tx.send(line).is_err() {
                            break; // Receiver dropped
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // Spawn task to handle events and send them to stdout
        let mut event_rx = self.base.subscribe_to_events();
        tokio::task::spawn_local(async move {
            while let Ok(event) = event_rx.recv().await {
                if let Err(e) = Self::format_to_json(event) {
                    eprintln!("Error handling event: {:?}", e);
                }
            }
        });

        // Process messages from stdin
        loop {
            // Check for stdin messages (non-blocking)
            match stdin_rx.try_recv() {
                Ok(line) => {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        self.handle_stdin_message(trimmed).await?;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No message available, continue
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    // Stdin reader thread ended
                    break;
                }
            }

            // Yield to allow event processing
            tokio::task::yield_now().await;

            // Small delay to avoid busy-waiting
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        // Cleanup
        self.base.shutdown().await?;

        Ok(())
    }

    async fn handle_stdin_message(&mut self, line: &str) -> Result<()> {
        match serde_json::from_str::<SubprocessMessage>(line) {
            Ok(SubprocessMessage::Chat { message }) => {
                // Send message to chat actor
                self.base.send_message(message).await?;
            }
            Ok(_) => {
                // Ignore other message types (they're outgoing only)
            }
            Err(e) => {
                // Send error response
                let error_msg = serde_json::to_string(&SubprocessMessage::Error {
                    error: format!("Invalid message format: {}", e),
                })?;
                println!("{}", error_msg);
                std::io::stdout().flush()?;
            }
        }
        Ok(())
    }

    fn format_to_json(event: ChatEvent) -> Result<()> {
        let message = match event {
            ChatEvent::MessageAdded(msg) => match msg.sender {
                MessageSender::Assistant => {
                    // Convert tool calls to our format
                    let tool_calls: Vec<crate::subprocess::ToolCall> = msg
                        .tool_calls
                        .iter()
                        .map(|tc| crate::subprocess::ToolCall {
                            name: tc.name.clone(),
                            arguments: tc.arguments.clone(),
                        })
                        .collect();

                    // Convert context info if present
                    let context_info =
                        msg.context_info
                            .as_ref()
                            .map(|ci| crate::subprocess::ContextInfo {
                                directory_list_bytes: ci.directory_list_bytes,
                                files: ci
                                    .files
                                    .iter()
                                    .map(|f| crate::subprocess::FileInfo {
                                        path: f.path.clone(),
                                        bytes: f.bytes,
                                    })
                                    .collect(),
                            });

                    // Convert token usage if present
                    let token_usage =
                        msg.token_usage
                            .as_ref()
                            .map(|tu| crate::subprocess::TokenUsage {
                                input_tokens: tu.input_tokens,
                                output_tokens: tu.output_tokens,
                                total_tokens: tu.total_tokens,
                            });

                    Some(SubprocessMessage::Response {
                        content: msg.content,
                        reasoning: msg.reasoning.as_ref().map(|r| r.text.clone()),
                        tool_calls,
                        model: msg.model_info.as_ref().map(|m| m.model.name().to_string()),
                        is_complete: msg.tool_calls.is_empty(), // Complete if no tool calls
                        context_info,
                        token_usage,
                    })
                }
                MessageSender::System => Some(SubprocessMessage::Event {
                    event: "system".to_string(),
                    data: serde_json::json!({ "content": msg.content }),
                }),
                MessageSender::Error => Some(SubprocessMessage::Error { error: msg.content }),
                MessageSender::User => None,
            },
            ChatEvent::TypingStatusChanged(_) => None, // Ignore typing status - not useful
            _ => None,
        };

        if let Some(msg) = message {
            let json = serde_json::to_string(&msg)?;
            println!("{}", json);
            std::io::stdout().flush()?;
        }
        Ok(())
    }
}
