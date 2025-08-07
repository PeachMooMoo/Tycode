//! TyCode - Operational tool for AWS
//!
//! This library provides core functionality for:
//! - CloudWatch Logs analysis
//! - Aurora DSQL database operations
//! - AWS configuration management

pub mod agents;
pub mod ai;
pub mod aws;
pub mod chat;
pub mod cli;
pub mod config;
pub mod db;
pub mod indexing;
pub mod terminal;
pub mod timing;
pub mod tools;

pub use ai::*;
pub use aws::*;
pub use db::*;

// Re-export commonly used types and traits
pub use anyhow::{Context, Result};
pub use serde::{Deserialize, Serialize};
pub use tokio;
pub use tracing::{debug, error, info, warn};
