use anyhow::{Context, Result};
use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres, Row};
use uuid::Uuid;

/// Dashboard entry stored in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dashboard {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub content: DashboardContent,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

/// Contents of a dashboard stored as JSON in the content field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardContent {
    pub version: String,
    pub inputs: Vec<TextInput>,
    pub data_sources: Vec<CloudWatchDataSource>,
    pub graphs: Vec<Graph>,
    pub layout: Layout,
    // Can easily add new sections in the future
}

/// Text input for variable definitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextInput {
    pub id: String,
    pub variable_name: String,
    pub label: String,
    pub default_value: Option<String>,
    pub order: i32,
}

/// CloudWatch data source definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchDataSource {
    pub id: String,
    pub name: String,
    pub query: String,
    pub log_group_pattern: String,
}

/// Graph definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Graph {
    pub id: String,
    pub title: String,
    pub type_name: String, // Using type_name instead of type which is a reserved keyword
    pub data_source_refs: Vec<String>,
    pub config: serde_json::Value,
    #[serde(default = "default_width_blocks")]
    pub width_blocks: u8, // Number of blocks wide (1-12)
}

fn default_width_blocks() -> u8 {
    12 // Default to full width
}

/// Layout information for the dashboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layout {
    pub sections: Vec<Section>,
}

/// Section in the dashboard layout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub title: String,
    pub components: Vec<String>,
}

/// Client for handling dashboard operations
#[derive(Clone)]
pub struct DashboardClient {
    pool: Pool<Postgres>,
}

impl DashboardClient {
    /// Create a new dashboard client with a connection pool
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    /// Initialize the dashboard table if it doesn't exist
    pub async fn init_schema(&self) -> Result<()> {
        // Check if the table exists, but don't modify it
        let table_exists = sqlx::query(
            "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_name = 'dashboards')",
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to check if dashboards table exists")?
        .get::<bool, _>(0);

        if !table_exists {
            // Table doesn't exist, create it with the correct types
            // The table creation will match what's expected in the database
            tracing::info!("Creating dashboards table...");
            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS dashboards (
                    id UUID NOT NULL,
                    name VARCHAR(255) NOT NULL,
                    description TEXT,
                    content TEXT NOT NULL,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    PRIMARY KEY (id)
                )
                "#,
            )
            .execute(&self.pool)
            .await
            .context("Failed to create dashboards table")?;

            tracing::info!("Successfully created dashboards table");
        }

        // Regardless of existing schema, log the connection
        tracing::info!("Connected to dashboards table");

        Ok(())
    }

    /// Get a dashboard by ID
    pub async fn get_dashboard(&self, id: &Uuid) -> Result<Option<Dashboard>> {
        // Query for dashboard
        let row = sqlx::query(
            r#"
            SELECT id, name, description, content, created_at, updated_at
            FROM dashboards
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch dashboard")?;

        // Parse result
        if let Some(row) = row {
            let content: String = row.try_get("content")?;
            let content_parsed: DashboardContent =
                serde_json::from_str(&content).context("Failed to parse dashboard content")?;

            // Return dashboard with parsed content
            Ok(Some(Dashboard {
                id: row.try_get("id")?,
                name: row.try_get("name")?,
                description: row.try_get("description")?,
                content: content_parsed,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            }))
        } else {
            Ok(None)
        }
    }

    /// List all dashboards (without full content)
    pub async fn list_dashboards(&self) -> Result<Vec<DashboardSummary>> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, description, created_at, updated_at
            FROM dashboards
            ORDER BY name ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to list dashboards")?;

        let mut dashboards = Vec::with_capacity(rows.len());
        for row in rows {
            dashboards.push(DashboardSummary {
                id: row.try_get("id")?,
                name: row.try_get("name")?,
                description: row.try_get("description")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            });
        }

        Ok(dashboards)
    }

    /// Create a new dashboard
    pub async fn create_dashboard(&self, dashboard: &Dashboard) -> Result<()> {
        // Serialize content to JSON string
        let content_json = serde_json::to_string(&dashboard.content)
            .context("Failed to serialize dashboard content")?;

        sqlx::query(
            r#"
            INSERT INTO dashboards (id, name, description, content, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(dashboard.id)
        .bind(&dashboard.name)
        .bind(&dashboard.description)
        .bind(&content_json)
        .bind(dashboard.created_at)
        .bind(dashboard.updated_at)
        .execute(&self.pool)
        .await
        .context("Failed to create dashboard")?;

        Ok(())
    }

    /// Update an existing dashboard
    pub async fn update_dashboard(&self, dashboard: &Dashboard) -> Result<()> {
        // Serialize content to JSON string
        let content_json = serde_json::to_string(&dashboard.content)
            .context("Failed to serialize dashboard content")?;

        sqlx::query(
            r#"
            UPDATE dashboards
            SET name = $2, 
                description = $3, 
                content = $4, 
                updated_at = $5
            WHERE id = $1
            "#,
        )
        .bind(dashboard.id)
        .bind(&dashboard.name)
        .bind(&dashboard.description)
        .bind(&content_json)
        .bind(dashboard.updated_at)
        .execute(&self.pool)
        .await
        .context("Failed to update dashboard")?;

        Ok(())
    }

    /// Delete a dashboard
    pub async fn delete_dashboard(&self, id: &Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM dashboards
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .context("Failed to delete dashboard")?;

        Ok(result.rows_affected() > 0)
    }
}

/// Summary of a dashboard (without full content)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSummary {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

/// Helper to create a new dashboard with default values
pub fn new_dashboard(name: &str, description: Option<&str>) -> Dashboard {
    let now = Utc::now().naive_utc();

    Dashboard {
        id: Uuid::new_v4(),
        name: name.to_string(),
        description: description.map(|s| s.to_string()).unwrap_or_default(),
        content: DashboardContent {
            version: "1.0".to_string(),
            inputs: Vec::new(),
            data_sources: Vec::new(),
            graphs: Vec::new(),
            layout: Layout {
                sections: Vec::new(),
            },
        },
        created_at: now,
        updated_at: now,
    }
}

/// Process variables in a query
pub fn process_variables(query: &str, variables: &[(String, String)]) -> String {
    let mut result = query.to_string();

    for (name, value) in variables {
        let placeholder = format!("{{{}}}", name);
        result = result.replace(&placeholder, value);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variable_substitution() {
        let query =
            "filter @message like /{ClusterId}/ | stats sum(metric1) by bin(1m), {PartitionId}";
        let variables = vec![
            ("ClusterId".to_string(), "cluster-123".to_string()),
            ("PartitionId".to_string(), "partition-456".to_string()),
        ];

        let processed = process_variables(query, &variables);
        assert_eq!(
            processed,
            "filter @message like /cluster-123/ | stats sum(metric1) by bin(1m), partition-456"
        );
    }
}
