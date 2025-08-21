use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityMode {
    ReadOnly,
    Auto,
    All,
}

impl Default for SecurityMode {
    fn default() -> Self {
        SecurityMode::Auto
    }
}

/// Permission result for a tool operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolPermission {
    Allowed,
    Denied,
}

/// Risk level for operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    /// No risk - read-only operations
    ReadOnly,
    /// Low risk - safe modifications within workspace
    LowRisk,
    /// High risk - command execution, sensitive paths, or outside workspace
    HighRisk,
}

/// Configuration for security policies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Current security mode
    #[serde(default)]
    pub mode: SecurityMode,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            mode: SecurityMode::default(),  // Uses SecurityMode::Auto
        }
    }
}
