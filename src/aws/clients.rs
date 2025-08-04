use anyhow::{Context, Result};
use std::sync::{Arc, Mutex};

use crate::ai::BedrockProvider;
use crate::aws::credentials::Credentials;
use crate::config::LaunchConfig;
use crate::db::DsqlClient;

use super::cloudwatch_logs::CloudWatchLogsClient;

/// A central structure that holds all AWS client instances.
///
/// This struct is designed to be initialized once and shared throughout the application
/// using Arc, avoiding the need to create multiple copies of the same clients.
#[derive(Clone)]
pub struct AwsClients {
    inner: Arc<Mutex<Inner>>,
}

impl AwsClients {
    /// Create a new AwsClients instance with the provided credentials
    pub async fn new(credentials: Credentials, app_config: LaunchConfig) -> Result<Self> {
        let inner = Inner::new(credentials, app_config).await?;
        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
        })
    }

    /// Get a CloudWatch Logs client
    pub fn cloudwatch_logs(&self) -> CloudWatchLogsClient {
        self.inner.lock().unwrap().cloudwatch_logs.clone()
    }

    /// Get a DSQL client
    pub fn dsql(&self) -> DsqlClient {
        self.inner.lock().unwrap().dsql.clone()
    }

    /// Get a Bedrock AI provider
    pub fn bedrock(&self) -> BedrockProvider {
        self.inner.lock().unwrap().bedrock.clone()
    }

    /// Get the current credentials (cloned)
    pub fn credentials(&self) -> Credentials {
        self.inner.lock().unwrap().credentials.clone()
    }

    /// Update all clients with new credentials
    pub async fn update_credentials(
        &self,
        credentials: Credentials,
        app_config: LaunchConfig,
    ) -> Result<()> {
        tracing::info!("Updating AWS clients with new credentials");

        let new_inner = Inner::new(credentials, app_config).await?;

        *self.inner.lock().unwrap() = new_inner;

        tracing::info!("AWS clients updated successfully");
        Ok(())
    }
}

/// Internal structure that holds all AWS client instances without Arc wrappers
struct Inner {
    cloudwatch_logs: CloudWatchLogsClient,
    dsql: DsqlClient,
    bedrock: BedrockProvider,
    credentials: Credentials,
}

impl Inner {
    /// Create a new Inner instance with the provided credentials
    async fn new(credentials: Credentials, app_config: LaunchConfig) -> Result<Self> {
        // Initialize CloudWatch Logs client with aggressive retry policy for throttling
        let cloudwatch_config = credentials
            .cloudwatch
            .sdk_config()
            .await
            .context("Failed to create CloudWatch SDK config")?
            .into_builder()
            .retry_config(
                aws_config::retry::RetryConfig::adaptive()
                    .with_max_attempts(15)
                    .with_initial_backoff(std::time::Duration::from_millis(100))
                    .with_max_backoff(std::time::Duration::from_secs(1)),
            )
            .build();
        let cloudwatch_logs = CloudWatchLogsClient::new(&cloudwatch_config).await;

        // Initialize DSQL AWS client
        let dsql_config = credentials
            .dsql
            .sdk_config()
            .await
            .context("Failed to create DSQL SDK config")?;

        // Initialize DSQL client (lazy-loaded, won't fail on creation)
        let dsql_client = DsqlClient::new(dsql_config, app_config.clone())
            .await
            .context("Failed to create DSQL client")?;

        let bedrock_config = credentials
            .dsql
            .sdk_config()
            .await
            .context("Failed to create Bedrock SDK config")?
            .into_builder()
            .retry_config(
                aws_config::retry::RetryConfig::adaptive()
                    .with_max_attempts(15)
                    .with_initial_backoff(std::time::Duration::from_millis(100))
                    .with_max_backoff(std::time::Duration::from_secs(1)),
            )
            .build();
        let bedrock_client = aws_sdk_bedrockruntime::Client::new(&bedrock_config);
        let bedrock = BedrockProvider::new(bedrock_client);

        Ok(Self {
            cloudwatch_logs,
            dsql: dsql_client,
            bedrock,
            credentials,
        })
    }
}
