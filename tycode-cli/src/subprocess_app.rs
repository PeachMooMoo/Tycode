use crate::base_app::BaseApp;
use crate::subprocess::SubprocessMessage;
use anyhow::Result;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use tycode_core::chat::events::{ChatEvent, MessageSender};

pub struct SubprocessApp {
    base: BaseApp,
}

impl SubprocessApp {
    pub async fn new(workspace_roots: Option<Vec<PathBuf>>, settings_path: Option<PathBuf>) -> Result<Self> {
        let base = BaseApp::new(workspace_roots, settings_path).await?;

        // Load initial settings through the actor
        let settings_json = base.get_settings().await?;
        let ready_msg = serde_json::to_string(&SubprocessMessage::Ready {
            settings: settings_json,
        })?;
        println!("{}", ready_msg);
        std::io::stdout().flush()?;

        Ok(Self { base })
    }

    pub async fn run(&mut self) -> Result<()> {
        use std::sync::mpsc;
        use std::thread;

        let (stdin_tx, stdin_rx) = mpsc::channel::<String>();

        thread::spawn(move || {
            let stdin = std::io::stdin();
            let reader = BufReader::new(stdin);
            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        if stdin_tx.send(line).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let mut event_rx = self.base.subscribe_to_events();
        tokio::task::spawn_local(async move {
            while let Ok(event) = event_rx.recv().await {
                if let Err(e) = Self::format_to_json(event) {
                    eprintln!("Error handling event: {:?}", e);
                }
            }
        });

        loop {
            match stdin_rx.try_recv() {
                Ok(line) => {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        self.handle_stdin_message(trimmed).await?;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => break,
            }

            tokio::task::yield_now().await;
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        Ok(())
    }

    async fn handle_stdin_message(&mut self, line: &str) -> Result<()> {
        match serde_json::from_str::<SubprocessMessage>(line) {
            Ok(SubprocessMessage::Chat { message }) => {
                self.base.send_message(message).await?;
            }
            Ok(SubprocessMessage::Cancel) => {
                self.base.cancel().await?;
            }
            Ok(SubprocessMessage::ChangeProvider { provider }) => {
                self.base.change_provider(provider).await?;
            }
            Ok(SubprocessMessage::LoadSettings) => {
                // Just get current settings without reloading
                // (reloading should be done explicitly via ReloadSettings message)
                let settings_json = self.base.get_settings().await?;
                let msg = serde_json::to_string(&SubprocessMessage::SettingsLoaded {
                    settings: settings_json,
                })?;
                println!("{}", msg);
                std::io::stdout().flush()?;
            }
            Ok(SubprocessMessage::SaveSettings { settings }) => {
                let result = self.base.save_settings(settings).await;
                let msg = match result {
                    Ok(()) => serde_json::to_string(&SubprocessMessage::SettingsSaved {
                        success: true,
                        error: None,
                    })?,
                    Err(e) => serde_json::to_string(&SubprocessMessage::SettingsSaved {
                        success: false,
                        error: Some(format!("{:?}", e)),
                    })?,
                };
                println!("{}", msg);
                std::io::stdout().flush()?;
            }
            Ok(SubprocessMessage::ReloadSettings) => {
                self.reload_settings().await?;
            }
            Ok(_) => {}
            Err(e) => {
                let error_msg = serde_json::to_string(&SubprocessMessage::Error {
                    error: format!("Invalid message format: {}", e),
                })?;
                println!("{}", error_msg);
                std::io::stdout().flush()?;
            }
        }
        Ok(())
    }

    async fn reload_settings(&mut self) -> Result<()> {
        // Send a reload message to the chat actor
        use tycode_core::chat::actor::ChatActorMessage;
        self.base.actor.tx.send(ChatActorMessage::ReloadSettings)?;
        
        Ok(())
    }

    fn format_to_json(event: ChatEvent) -> Result<()> {
        let message = match event {
            ChatEvent::MessageAdded(msg) => match msg.sender {
                MessageSender::Assistant => {
                    let tool_calls: Vec<crate::subprocess::ToolCall> = msg
                        .tool_calls
                        .iter()
                        .map(|tc| crate::subprocess::ToolCall {
                            name: tc.name.clone(),
                            arguments: tc.arguments.clone(),
                        })
                        .collect();

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
                        is_complete: msg.tool_calls.is_empty(),
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
            ChatEvent::ToolExecutionCompleted {
                tool_name,
                success,
                result,
                ui_data,
                error,
            } => {
                let combined_result = if let Some(ui) = ui_data {
                    if let Some(mut res) = result {
                        if let serde_json::Value::Object(ref mut res_map) = res {
                            if let serde_json::Value::Object(ui_map) = ui {
                                for (key, value) in ui_map {
                                    res_map.insert(key, value);
                                }
                            }
                        }
                        Some(res)
                    } else {
                        Some(ui)
                    }
                } else {
                    result
                };
                Some(SubprocessMessage::ToolResult {
                    tool_name,
                    success,
                    result: combined_result,
                    error,
                })
            }
            ChatEvent::TypingStatusChanged(_) => None,
            ChatEvent::OperationCancelled { message } => Some(SubprocessMessage::Event {
                event: "cancelled".to_string(),
                data: serde_json::json!({ "message": message }),
            }),
            ChatEvent::RetryAttempt {
                attempt,
                max_retries,
                error,
                backoff_ms,
            } => Some(SubprocessMessage::Event {
                event: "retry_attempt".to_string(),
                data: serde_json::json!({
                    "attempt": attempt,
                    "max_retries": max_retries,
                    "error": error,
                    "backoff_ms": backoff_ms,
                }),
            }),
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
