use crate::base_app::BaseApp;
use crate::subprocess::SubprocessMessage;
use anyhow::{bail, Result};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use tracing::{error, info};
use tycode_core::chat::events::{ChatEvent, MessageSender};
use tycode_core::chat::ChatActor;

pub struct SubprocessApp {
    base: BaseApp,
}

impl SubprocessApp {
    pub async fn new(
        workspace_roots: Option<Vec<PathBuf>>,
        settings_path: Option<PathBuf>,
    ) -> Result<Self> {
        info!("Starting SubprocessApp");
        info!("Workspace roots: {:?}", workspace_roots);
        info!("Settings path: {:?}", settings_path);

        let base = BaseApp::new(workspace_roots, settings_path).await?;
        base.actor.get_settings()?;

        Ok(Self { base })
    }

    pub async fn run(self) -> Result<()> {
        use std::sync::mpsc;
        use std::thread;

        info!("Starting subprocess run loop");
        let BaseApp {
            actor,
            mut event_rx,
        } = self.base;

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

        tokio::task::spawn_local(async move {
            // handle the initial ready message - we sent GetSettings on init
            // so the first message back out of the actor should always be settings
            let Some(ChatEvent::Settings(settings)) = event_rx.recv().await else {
                bail!("First message should be settings!");
            };
            let json = serde_json::to_string(&SubprocessMessage::Ready { settings })?;
            println!("{}", json);
            std::io::stdout().flush()?;

            while let Some(event) = event_rx.recv().await {
                if let Err(e) = format_to_json(event) {
                    error!("Error handling event: {:?}", e);
                }
            }

            Ok(())
        });

        loop {
            match stdin_rx.try_recv() {
                Ok(line) => {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        info!("Received stdin message: {}", trimmed);
                        handle_stdin_message(&actor, trimmed).await?;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    info!("Stdin disconnected, shutting down");
                    break;
                }
            }

            tokio::task::yield_now().await;
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        info!("Subprocess run loop completed");
        Ok(())
    }
}

async fn handle_stdin_message(actor: &ChatActor, line: &str) -> Result<()> {
    match serde_json::from_str::<SubprocessMessage>(line) {
        Ok(SubprocessMessage::Chat { message }) => {
            actor.send_message(message)?;
        }
        Ok(SubprocessMessage::Cancel) => {
            actor.cancel()?;
        }
        Ok(SubprocessMessage::ChangeProvider { provider }) => {
            actor.change_provider(provider)?;
        }
        Ok(SubprocessMessage::LoadSettings) => {
            actor.get_settings()?;
        }
        Ok(SubprocessMessage::SaveSettings { settings }) => {
            let result = actor.save_settings(settings);
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

fn format_to_json(event: ChatEvent) -> Result<()> {
    let message = match event {
        ChatEvent::MessageAdded(msg) => match msg.sender {
            MessageSender::Assistant { agent: _agent } => {
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
        ChatEvent::TypingStatusChanged(is_typing) => Some(SubprocessMessage::Event {
            event: "typing_status".to_string(),
            data: serde_json::json!({ "is_typing": is_typing }),
        }),
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
        ChatEvent::Settings(settings) => Some(SubprocessMessage::SettingsLoaded { settings }),
        _ => None,
    };

    if let Some(msg) = message {
        let json = serde_json::to_string(&msg)?;
        println!("{}", json);
        std::io::stdout().flush()?;
    }
    Ok(())
}
