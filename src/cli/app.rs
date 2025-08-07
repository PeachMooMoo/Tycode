use crate::ai::{
    bedrock::BedrockProvider,
    types::{Model, ModelTunings},
};
use crate::chat::{
    actor::{ChatActor, ChatActorMessage},
    events::{ChatEvent, ChatMessage, MessageSender},
    state::SharedChatState,
};
use crate::cli::formatter::Formatter;
use crate::terminal::splash::TYCODE_ASCII;
use anyhow::Result;
use rustyline::DefaultEditor;
use tokio::sync::{broadcast, mpsc};

pub struct CliApp {
    chat_state: SharedChatState,
    actor_tx: mpsc::UnboundedSender<ChatActorMessage>,
    formatter: Formatter,
    event_rx: broadcast::Receiver<ChatEvent>,
}

impl CliApp {
    pub async fn new(
        provider: BedrockProvider,
        model: Model,
        system_prompt: String,
        tunings: ModelTunings,
    ) -> Result<Self> {
        // Create shared chat state
        let chat_state = SharedChatState::new(model, system_prompt.clone(), tunings);

        // Create actor communication channel
        let (actor_tx, actor_rx) = mpsc::unbounded_channel();

        // Get workspace root
        let workspace_root = std::env::current_dir()?;

        // Create and spawn the chat actor
        let actor = ChatActor::new(chat_state.clone(), provider, workspace_root, actor_rx);
        tokio::spawn(async move {
            actor.run().await;
        });

        // Subscribe to events but don't spawn a separate task
        let event_rx = chat_state.subscribe();
        let formatter = Formatter::new();

        // Display welcome message with proper ASCII art from splash.rs
        println!();

        // Display TYCODE ASCII art in yellow
        formatter.print_splash_art(TYCODE_ASCII, "\x1b[33m");

        println!();
        formatter.print_divider();
        println!();

        // Create the welcome message
        let welcome_message = format!(
            "📦 Model: {}\n💡 Type /help for commands, /quit to exit",
            model.name()
        );
        
        // Display the welcome message immediately (only once)
        formatter.print_system(&welcome_message);

        Ok(Self {
            chat_state,
            actor_tx,
            formatter,
            event_rx,
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
                        self.formatter.print_ai(&message.content);

                        // Display reasoning if present
                        if let Some(reasoning) = &message.reasoning {
                            self.formatter
                                .print_system(&format!("💭 Reasoning: {}", reasoning));
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
        self.formatter.print_system("Available commands:");
        self.formatter
            .print_system("  /help    - Show this help message");
        self.formatter
            .print_system("  /quit    - Exit the application");
        self.formatter
            .print_system("  /exit    - Exit the application");
        self.formatter.print_system("");
        self.formatter
            .print_system("Just type your message and press Enter to chat with the AI.");
        self.formatter.print_divider();
    }
}
