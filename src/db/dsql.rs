use anyhow::{Context, Result};
use aws_config::{Region, SdkConfig};
use aws_sdk_dsql::auth_token::{AuthToken, AuthTokenGenerator, Config};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time;
use tracing::{debug, info};

use super::audit::{generate_session_id, AuditClient};
use super::dashboard::{Dashboard, DashboardClient, DashboardSummary};
use super::memory::{MemoryClient, MemoryNode};
use crate::config::LaunchConfig as AppConfig;

const SECONDS_PER_MINUTE: u64 = 60;
const TOKEN_EXPIRATION_MINUTES: u64 = 15;
const TOKEN_EXPIRATION_SECONDS: u64 = TOKEN_EXPIRATION_MINUTES * SECONDS_PER_MINUTE;
const TOKEN_REFRESH_MINUTES: u64 = TOKEN_EXPIRATION_MINUTES - 5;
const TOKEN_REFRESH_SECONDS: u64 = TOKEN_REFRESH_MINUTES * SECONDS_PER_MINUTE;

/// DSQL client for interacting with Aurora DSQL
#[derive(Clone)]
pub struct DsqlClient {
    sdk_config: SdkConfig,
    app_config: AppConfig,
    // Lazy-initialized actual client
    inner: Arc<Mutex<Option<ConnectedDsqlClient>>>,
}

/// Internal struct that holds the actual connected DSQL client
#[derive(Clone)]
struct ConnectedDsqlClient {
    memory_client: MemoryClient,
    audit_client: AuditClient,
    dashboard_client: DashboardClient,
}

impl DsqlClient {
    /// Create a new DSQL client (lazy-loaded, won't connect until first use)
    pub async fn new(sdk_config: SdkConfig, app_config: AppConfig) -> Result<Self> {
        info!(
            "Creating lazy DSQL client for cluster at {}",
            app_config.dsql_cluster_endpoint
        );

        Ok(Self {
            sdk_config,
            app_config,
            inner: Arc::new(Mutex::new(None)),
        })
    }

    /// Ensure the DSQL client is connected, initializing if necessary
    async fn ensure_connected(&self) -> Result<ConnectedDsqlClient> {
        // First, check if we already have a connection
        {
            let inner = self.inner.lock().unwrap();
            if let Some(ref client) = *inner {
                return Ok(client.clone());
            }
        }

        // Not connected yet - initialize now
        tracing::info!("Initializing DSQL connection...");

        let connected_client = self
            .create_connected_client()
            .await
            .context("Failed to create DSQL connection")?;

        // Store the connection
        {
            let mut inner = self.inner.lock().unwrap();
            *inner = Some(connected_client.clone());
        }

        Ok(connected_client)
    }

    /// Create the actual connected DSQL client
    async fn create_connected_client(&self) -> Result<ConnectedDsqlClient> {
        let cluster_user = "admin";
        let cluster_endpoint = self.app_config.dsql_cluster_endpoint.clone();
        let region = cluster_endpoint
            .split('.')
            .nth(2)
            .unwrap_or_default()
            .to_string();
        let database = "postgres";

        info!(
            "Connecting to DSQL cluster at {} as user {}",
            cluster_endpoint, cluster_user
        );

        // Load AWS config and create token generator
        let signer = AuthTokenGenerator::new(
            Config::builder()
                .hostname(&cluster_endpoint)
                .region(Region::new(region.clone()))
                .expires_in(TOKEN_EXPIRATION_SECONDS)
                .build()
                .map_err(|e| anyhow::anyhow!("Failed to build auth token config: {}", e))?,
        );

        // Generate initial auth token
        let password_token =
            Self::generate_password_token(cluster_user, &signer, &self.sdk_config).await?;

        // Create connection options
        let connection_options = PgConnectOptions::new()
            .host(&cluster_endpoint)
            .port(5432)
            .database(database)
            .username(cluster_user)
            .password(password_token.as_str())
            .ssl_mode(sqlx::postgres::PgSslMode::VerifyFull);

        // Create connection pool
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect_with(connection_options.clone())
            .await
            .context("Failed to create connection pool")?;

        // Set up automatic token refresh
        let pool_clone = pool.clone();
        let sdk_config_clone = self.sdk_config.clone();
        tokio::spawn(async move {
            loop {
                time::sleep(Duration::from_secs(TOKEN_REFRESH_SECONDS)).await;

                match Self::generate_password_token(cluster_user, &signer, &sdk_config_clone).await
                {
                    Ok(new_token) => {
                        let new_options = connection_options.clone().password(new_token.as_str());
                        pool_clone.set_connect_options(new_options);
                        debug!("Successfully refreshed DSQL auth token");
                    }
                    Err(e) => {
                        tracing::error!(?e, "Failed to refresh DSQL auth token");
                    }
                }
            }
        });

        // Generate a new session ID for this client instance
        let session_id = generate_session_id();
        info!("Generated new session ID: {}", session_id);

        // Create the clients
        let memory_client = MemoryClient::new(pool.clone());
        let audit_client = AuditClient::new(pool.clone(), session_id);
        let dashboard_client = DashboardClient::new(pool.clone());

        // Create the connected client instance
        let connected_client = ConnectedDsqlClient {
            memory_client,
            audit_client,
            dashboard_client,
        };

        // Initialize schemas for all tables
        connected_client.memory_client.init_schema().await?;
        connected_client.audit_client.init_schema().await?;
        connected_client.dashboard_client.init_schema().await?;

        Ok(connected_client)
    }

    /// Generate an authentication token for the given user
    async fn generate_password_token(
        cluster_user: &str,
        signer: &AuthTokenGenerator,
        sdk_config: &SdkConfig,
    ) -> Result<AuthToken> {
        if cluster_user == "admin" {
            signer
                .db_connect_admin_auth_token(sdk_config)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to generate admin auth token: {}", e))
        } else {
            signer
                .db_connect_auth_token(sdk_config)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to generate auth token: {}", e))
        }
    }

    // Memory-related methods that delegate to the memory client

    /// Initialize the database schema for memory, audit, and dashboard tables
    pub async fn init_schema(&self) -> Result<()> {
        let client = self.ensure_connected().await?;
        // Initialize all schemas
        client.memory_client.init_schema().await?;
        client.audit_client.init_schema().await?;
        client.dashboard_client.init_schema().await?;
        Ok(())
    }

    /// Get a memory node and its children
    pub async fn get_memory_node(&self, path: &str) -> Result<Option<MemoryNode>> {
        let client = self.ensure_connected().await?;
        client.memory_client.get_memory_node(path).await
    }

    /// Create a new memory node
    pub async fn create_memory(&self, path: &str, content: &str) -> Result<MemoryNode> {
        let client = self.ensure_connected().await?;
        client.memory_client.create_memory(path, content).await
    }

    /// Update an existing memory node
    pub async fn update_memory(&self, path: &str, content: &str) -> Result<Option<MemoryNode>> {
        let client = self.ensure_connected().await?;
        client.memory_client.update_memory(path, content).await
    }

    /// Delete a memory node
    pub async fn delete_memory(&self, path: &str) -> Result<bool> {
        let client = self.ensure_connected().await?;
        client.memory_client.delete_memory(path).await
    }

    // Audit-related methods

    /// Get the current session ID
    pub fn get_session_id(&self) -> String {
        // For session ID, we can return a default if not connected yet
        // The session ID is generated when connection is established
        if let Ok(client) = self.inner.lock().unwrap().as_ref().ok_or("Not connected") {
            client.audit_client.get_session_id()
        } else {
            "not_connected".to_string()
        }
    }

    /// Record a human input event in the audit log
    pub async fn record_human_input(&self, content: &str) -> Result<()> {
        let client = self.ensure_connected().await?;
        client.audit_client.record_human_input(content).await
    }

    /// Record a tool execution event in the audit log
    pub async fn record_tool_execution(
        &self,
        content: &str,
        metadata: Option<serde_json::Value>,
    ) -> Result<()> {
        let client = self.ensure_connected().await?;
        client
            .audit_client
            .record_tool_execution(content, metadata)
            .await
    }

    /// Record an AI response event in the audit log
    pub async fn record_ai_response(&self, content: &str) -> Result<()> {
        let client = self.ensure_connected().await?;
        client.audit_client.record_ai_response(content).await
    }

    /// Get all audit entries for the current session
    pub async fn get_audit_entries(&self) -> Result<Vec<super::audit::AuditEntry>> {
        let client = self.ensure_connected().await?;
        let session_id = client.audit_client.get_session_id();
        client
            .audit_client
            .get_entries_by_session(&session_id)
            .await
    }

    // Dashboard-related methods

    /// Get a dashboard by ID
    pub async fn get_dashboard(&self, id: &uuid::Uuid) -> Result<Option<Dashboard>> {
        let client = self.ensure_connected().await?;
        client.dashboard_client.get_dashboard(id).await
    }

    /// List all dashboards (without full content)
    pub async fn list_dashboards(&self) -> Result<Vec<DashboardSummary>> {
        let client = self.ensure_connected().await?;
        client.dashboard_client.list_dashboards().await
    }

    /// Create a new dashboard
    pub async fn create_dashboard(&self, dashboard: &Dashboard) -> Result<()> {
        let client = self.ensure_connected().await?;
        client.dashboard_client.create_dashboard(dashboard).await
    }

    /// Update an existing dashboard
    pub async fn update_dashboard(&self, dashboard: &Dashboard) -> Result<()> {
        let client = self.ensure_connected().await?;
        client.dashboard_client.update_dashboard(dashboard).await
    }

    /// Delete a dashboard
    pub async fn delete_dashboard(&self, id: &uuid::Uuid) -> Result<bool> {
        let client = self.ensure_connected().await?;
        client.dashboard_client.delete_dashboard(id).await
    }
}
