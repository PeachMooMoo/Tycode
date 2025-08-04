use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres, Row};
use tracing::debug;

/// Memory client for handling memory storage operations
#[derive(Clone)]
pub struct MemoryClient {
    pool: Pool<Postgres>,
}

impl MemoryClient {
    /// Create a new memory client with a connection pool
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    /// Initialize the memories table if it doesn't exist
    pub async fn init_schema(&self) -> Result<()> {
        tracing::info!("Initializing DSQL schema for memories");

        // Create the memories table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS memories (
                id UUID NOT NULL DEFAULT gen_random_uuid(),
                path VARCHAR(1024) NOT NULL UNIQUE,
                content TEXT NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (id)
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .context("Failed to create memories table")?;

        // Create indexes
        sqlx::query(
            r#"
            CREATE INDEX ASYNC IF NOT EXISTS idx_memories_path ON memories(path);
            "#,
        )
        .execute(&self.pool)
        .await
        .context("Failed to create path index")?;

        // Insert root node if it doesn't exist
        let root_content = r#"You are working with Amazon Aurora DSQL (codename Xanadu), a distributed SQL database."#;

        sqlx::query(
            r#"
            INSERT INTO memories (path, content) 
            VALUES ($1, $2)
            ON CONFLICT (path) DO NOTHING
            "#,
        )
        .bind("/")
        .bind(root_content)
        .execute(&self.pool)
        .await
        .context("Failed to insert root node")?;

        tracing::info!("DSQL schema initialization complete");
        Ok(())
    }

    /// Get a memory node and its children
    pub async fn get_memory_node(&self, path: &str) -> Result<Option<MemoryNode>> {
        debug!("Getting memory node at path: {}", path);

        // First, get the node itself
        let node_result = sqlx::query(
            r#"
            SELECT path, content, updated_at
            FROM memories
            WHERE path = $1
            "#,
        )
        .bind(path)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch memory node")?;

        match node_result {
            Some(node) => {
                let path: String = node.try_get("path")?;
                let content: String = node.try_get("content")?;
                let updated_at: chrono::NaiveDateTime = node.try_get("updated_at")?;

                // Get children paths
                let children_rows = sqlx::query(
                    r#"
                    SELECT path
                    FROM memories
                    WHERE 
                        CASE 
                            WHEN $1 = '/' THEN 
                                path LIKE '/%' AND path NOT LIKE '/%/%' AND path != '/'
                            ELSE 
                                path LIKE $1 || '/%' AND path NOT LIKE $1 || '/%/%'
                        END
                    ORDER BY path
                    "#,
                )
                .bind(path.as_str())
                .fetch_all(&self.pool)
                .await
                .context("Failed to fetch children")?;

                let children: Vec<String> = children_rows
                    .into_iter()
                    .map(|row| row.try_get::<String, _>("path"))
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(Some(MemoryNode {
                    path,
                    content,
                    updated_at: updated_at.to_string(),
                    children,
                }))
            }
            None => Ok(None),
        }
    }

    /// Create a new memory node
    pub async fn create_memory(&self, path: &str, content: &str) -> Result<MemoryNode> {
        debug!("Creating memory node at path: {}", path);

        let row = sqlx::query(
            r#"
            INSERT INTO memories (path, content)
            VALUES ($1, $2)
            RETURNING path, content, updated_at
            "#,
        )
        .bind(path)
        .bind(content)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create memory")?;

        let path: String = row.try_get("path")?;
        let content: String = row.try_get("content")?;
        let updated_at: chrono::NaiveDateTime = row.try_get("updated_at")?;

        Ok(MemoryNode {
            path,
            content,
            updated_at: updated_at.to_string(),
            children: vec![],
        })
    }

    /// Update an existing memory node
    pub async fn update_memory(&self, path: &str, content: &str) -> Result<Option<MemoryNode>> {
        debug!("Updating memory node at path: {}", path);

        let row = sqlx::query(
            r#"
            UPDATE memories
            SET content = $2, updated_at = CURRENT_TIMESTAMP
            WHERE path = $1
            RETURNING path, content, updated_at
            "#,
        )
        .bind(path)
        .bind(content)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to update memory")?;

        match row {
            Some(r) => {
                let path: String = r.try_get("path")?;
                let content: String = r.try_get("content")?;
                let updated_at: chrono::NaiveDateTime = r.try_get("updated_at")?;

                Ok(Some(MemoryNode {
                    path,
                    content,
                    updated_at: updated_at.to_string(),
                    children: vec![],
                }))
            }
            None => Ok(None),
        }
    }

    /// Delete a memory node
    pub async fn delete_memory(&self, path: &str) -> Result<bool> {
        debug!("Deleting memory node at path: {}", path);

        let result = sqlx::query(
            r#"
            DELETE FROM memories
            WHERE path = $1
            "#,
        )
        .bind(path)
        .execute(&self.pool)
        .await
        .context("Failed to delete memory")?;

        // Return true if a row was deleted, false otherwise
        Ok(result.rows_affected() > 0)
    }
}

/// Memory node structure returned from queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryNode {
    pub path: String,
    pub content: String,
    pub updated_at: String,
    pub children: Vec<String>,
}
