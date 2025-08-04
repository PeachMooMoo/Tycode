use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres, Row};
use tracing::{debug, info};
use uuid::Uuid;

const MAX_CONTENT_SIZE_BYTES: usize = 1_900_000; // ~1.9MB to stay safely under 2MB limit
const TRUNCATION_MARKER: &str = "... [TRUNCATED]";

/// Represents different types of events that can be audited
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EventType {
    HumanInput,
    ToolExecution,
    AiResponse,
}

impl EventType {
    /// Convert event type to a string representation for storage
    fn as_str(&self) -> &'static str {
        match self {
            EventType::HumanInput => "human_input",
            EventType::ToolExecution => "tool_execution",
            EventType::AiResponse => "ai_response",
        }
    }
}

/// Entry in the audit log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub session_id: String,
    pub timestamp: String,
    pub event_type: String,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
}

/// Client for handling audit logging operations
#[derive(Clone)]
pub struct AuditClient {
    pool: Pool<Postgres>,
    session_id: String,
}

impl AuditClient {
    /// Create a new audit client with a connection pool and session ID
    pub fn new(pool: Pool<Postgres>, session_id: String) -> Self {
        Self { pool, session_id }
    }

    /// Initialize the audit_logs table if it doesn't exist
    pub async fn init_schema(&self) -> Result<()> {
        info!("Initializing DSQL schema for audit logs");

        // Create the audit_logs table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS audit_logs (
                id UUID NOT NULL DEFAULT gen_random_uuid(),
                session_id UUID NOT NULL,
                timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                event_type VARCHAR(20) NOT NULL,
                content TEXT NOT NULL,
                metadata TEXT,
                PRIMARY KEY (id)
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .context("Failed to create audit_logs table")?;

        // Create indexes (Aurora DSQL requires ASYNC for index creation)
        sqlx::query(
            r#"
            CREATE INDEX ASYNC IF NOT EXISTS idx_audit_logs_session_id ON audit_logs(session_id);
            "#,
        )
        .execute(&self.pool)
        .await
        .context("Failed to create session_id index")?;

        info!("Audit logs schema initialization complete");
        Ok(())
    }

    /// Get the current session ID
    pub fn get_session_id(&self) -> String {
        self.session_id.to_string()
    }

    /// Record a human input event
    pub async fn record_human_input(&self, content: &str) -> Result<()> {
        self.record_event(EventType::HumanInput, content, None)
            .await
    }

    /// Record a tool execution event
    pub async fn record_tool_execution(
        &self,
        content: &str,
        metadata: Option<serde_json::Value>,
    ) -> Result<()> {
        self.record_event(EventType::ToolExecution, content, metadata)
            .await
    }

    /// Record an AI response event
    pub async fn record_ai_response(&self, content: &str) -> Result<()> {
        self.record_event(EventType::AiResponse, content, None)
            .await
    }

    /// Record an audit event with the given type and content
    pub async fn record_event(
        &self,
        event_type: EventType,
        content: &str,
        metadata: Option<serde_json::Value>,
    ) -> Result<()> {
        let safe_content = truncate_if_needed(content);
        let event_type_str = event_type.as_str();

        // Convert metadata JSON to string if present
        let metadata_str = metadata
            .map(|m| serde_json::to_string(&m))
            .transpose()
            .context("Failed to serialize metadata to JSON string")?;

        debug!(
            "Recording audit event: type={}, session_id={}, content_length={}",
            event_type_str,
            self.session_id,
            safe_content.len()
        );

        // Parse the session ID string into a UUID for the database
        let session_uuid =
            Uuid::parse_str(&self.session_id).context("Failed to parse session_id as UUID")?;

        sqlx::query(
            r#"
            INSERT INTO audit_logs (session_id, event_type, content, metadata)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(session_uuid)
        .bind(event_type_str)
        .bind(&safe_content)
        .bind(&metadata_str)
        .execute(&self.pool)
        .await
        .context("Failed to insert audit log entry")?;

        Ok(())
    }

    /// Get audit entries for a specific session
    pub async fn get_entries_by_session(&self, session_id: &str) -> Result<Vec<AuditEntry>> {
        // Parse the session ID string into a UUID for the database
        let session_uuid =
            Uuid::parse_str(session_id).context("Failed to parse session_id as UUID")?;

        let rows = sqlx::query(
            r#"
            SELECT id, session_id, timestamp, event_type, content, metadata
            FROM audit_logs
            WHERE session_id = $1
            ORDER BY timestamp ASC
            "#,
        )
        .bind(session_uuid)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch audit entries")?;

        let mut entries = Vec::with_capacity(rows.len());
        for row in rows {
            // Parse metadata string back to JSON if present
            let metadata_str: Option<String> = row.try_get("metadata")?;
            let metadata = if let Some(str_value) = metadata_str {
                if str_value.is_empty() {
                    None
                } else {
                    match serde_json::from_str(&str_value) {
                        Ok(json) => Some(json),
                        Err(_) => {
                            // If parsing fails, store as a simple string value
                            Some(serde_json::Value::String(str_value))
                        }
                    }
                }
            } else {
                None
            };

            entries.push(AuditEntry {
                id: row.try_get::<Uuid, _>("id")?.to_string(),
                session_id: row.try_get::<Uuid, _>("session_id")?.to_string(),
                timestamp: row
                    .try_get::<chrono::NaiveDateTime, _>("timestamp")?
                    .to_string(),
                event_type: row.try_get("event_type")?,
                content: row.try_get("content")?,
                metadata,
            });
        }

        Ok(entries)
    }
}

/// Truncate content if it exceeds the maximum allowed size
fn truncate_if_needed(content: &str) -> String {
    // Check if content is within size limits
    if content.len() <= MAX_CONTENT_SIZE_BYTES {
        return content.to_string();
    }

    // Calculate safe truncation point (account for the marker)
    let max_content = MAX_CONTENT_SIZE_BYTES - TRUNCATION_MARKER.len();

    // Find a valid UTF-8 boundary for truncation
    // Walk back from max_content until we find a valid char boundary
    let mut truncate_at = max_content;
    while truncate_at > 0 && !content.is_char_boundary(truncate_at) {
        truncate_at -= 1;
    }

    // Create truncated content
    let mut truncated = content[..truncate_at].to_string();
    truncated.push_str(TRUNCATION_MARKER);

    truncated
}

/// Generate a new random UUID for session identification
pub fn generate_session_id() -> String {
    Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_content() {
        // Normal case - no truncation needed
        let small_content = "This is a small piece of content";
        assert_eq!(truncate_if_needed(small_content), small_content);

        // Create a large string exceeding the limit
        let large_content = "x".repeat(MAX_CONTENT_SIZE_BYTES + 1000);
        let truncated = truncate_if_needed(&large_content);

        // Verify truncation
        assert!(truncated.len() <= MAX_CONTENT_SIZE_BYTES);
        assert!(truncated.ends_with(TRUNCATION_MARKER));
    }

    #[test]
    fn test_event_type_as_str() {
        assert_eq!(EventType::HumanInput.as_str(), "human_input");
        assert_eq!(EventType::ToolExecution.as_str(), "tool_execution");
        assert_eq!(EventType::AiResponse.as_str(), "ai_response");
    }
}
