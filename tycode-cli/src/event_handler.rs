use anyhow::Result;
use tycode_core::chat::events::ChatEvent;

pub trait EventFormatter {
    fn format_event(&mut self, event: ChatEvent) -> Result<()>;
}
