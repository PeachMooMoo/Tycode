use crate::ai::provider::AiProvider;
use crate::chat::{
    events::{ChatMessage, MessageSender},
    state::SharedChatState,
};
use crate::indexing::Indexer;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

pub struct RebuildIndexCommand {
    state: SharedChatState,
}

impl RebuildIndexCommand {
    pub fn new(state: SharedChatState) -> Self {
        Self { state }
    }

    pub async fn execute(&self, provider: &dyn AiProvider) -> Vec<ChatMessage> {
        let start_time = Instant::now();

        // Set AI processing state to true to show user something is happening
        self.state.set_typing(true);

        // Show initial message
        let mut messages = vec![self.create_message(
            "🔄 Starting index rebuild... This may take a while depending on your codebase size."
                .to_string(),
            MessageSender::System,
        )];

        // Get workspace root (assuming current directory)
        let workspace_root =
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

        // Create indexer with progress callback that reports every 10th file
        let state_clone = self.state.clone();
        let progress_counter = Arc::new(AtomicUsize::new(0));
        let progress_callback = move |current_file: &str| {
            let count = progress_counter.fetch_add(1, Ordering::Relaxed) + 1;

            // Only send a progress message every 10 files to avoid spam
            if count % 10 == 0 || current_file.contains("directories") {
                state_clone.add_message(ChatMessage {
                    content: format!("📝 Processing: {}", current_file),
                    sender: MessageSender::System,
                    timestamp: Instant::now(),
                    reasoning: None,
                    tool_calls: Vec::new(),
                });
            }
        };

        let mut indexer = Indexer::new(&workspace_root);

        // Run indexing with progress updates
        let indexing_result = indexer
            .rebuild_all_indexes_with_progress(provider, progress_callback)
            .await;

        // Clear AI processing state
        self.state.set_typing(false);

        match indexing_result {
            Ok(stats) => {
                let duration = start_time.elapsed();
                let summary = format!(
                    "✅ Index rebuild completed in {:.2}s\n\n📊 **Statistics:**\n- Updated: {} files\n- Unchanged: {} files\n- Skipped: {} files\n- Directories: {} directories\n- Errors: {} errors\n- Total processed: {} items",
                    duration.as_secs_f64(),
                    stats.updated,
                    stats.unchanged,
                    stats.skipped,
                    stats.directories,
                    stats.errors,
                    stats.total_processed()
                );

                messages.push(self.create_message(summary, MessageSender::System));

                // Add helpful message about usage
                if stats.updated > 0 || stats.directories > 0 {
                    messages.push(self.create_message(
                        "💡 Index files are now available in `.tycode/index/` - these provide lightweight summaries of your codebase for faster context loading.".to_string(),
                        MessageSender::System,
                    ));
                }
            }
            Err(e) => {
                messages.push(
                    self.create_message(
                        format!("❌ Indexing failed: {}", e),
                        MessageSender::System,
                    ),
                );
            }
        }

        messages
    }

    fn create_message(&self, content: String, sender: MessageSender) -> ChatMessage {
        ChatMessage {
            content,
            sender,
            timestamp: Instant::now(),
            reasoning: None,
            tool_calls: Vec::new(),
        }
    }
}
