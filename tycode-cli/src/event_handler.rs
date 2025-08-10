use anyhow::Result;
use tycode_core::chat::events::ChatEvent;

/// Trait for formatting chat events for output
pub trait EventFormatter {
    fn format_event(&mut self, event: ChatEvent) -> Result<()>;
}
