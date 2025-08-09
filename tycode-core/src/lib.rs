//! TyCode Core - Minimal chat actor and core functionality
//!
//! This crate provides the core functionality that works in both WASM and native:
//! - Chat actor for AI interactions
//! - Agent system
//! - AI types and interfaces
//! - Configuration and settings

pub mod agents;
pub mod ai;
pub mod chat;
pub mod platform;
pub mod settings;
pub mod timing;
pub mod tools;

// Re-export commonly used types and traits
pub use anyhow::{Context, Result};
pub use serde::{Deserialize, Serialize};
pub use tracing::{debug, error, info, warn};

// Re-export main modules for convenience
pub use ai::*;
pub use chat::actor::ChatActor;
pub use chat::events::{ChatEvent, ChatMessage, MessageSender};
pub use chat::state::ChatState;
pub use platform::{Platform, SearchResult};
