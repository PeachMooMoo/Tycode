use anyhow::{anyhow, Context, Result};
use aws_config::SdkConfig;
use aws_sdk_cloudwatchlogs::{
    types::{LogGroup, QueryStatus},
    Client,
};
use aws_sdk_sts::Client as StsClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{debug, info, instrument, warn};

use super::cache::{CloudWatchDataCache, DataPoint, QueryMetadata, QueryStrategy};

#[derive(Clone)]
pub struct CloudWatchLogsClient {
    client: Client,
    account_id: Option<String>,
    // Cache for log groups - they practically never change, so cache forever
    log_groups_cache: Arc<Mutex<Option<Vec<LogGroup>>>>,
    query_cache: Arc<Mutex<CloudWatchDataCache>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub fields: HashMap<String, String>,
}

impl CloudWatchLogsClient {
    /// Create a new CloudWatch Logs client
    pub async fn new(config: &SdkConfig) -> Self {
        let client = Client::new(config);

        // Get account ID
        let account_id = Self::get_account_id(config).await;
        if let Some(id) = &account_id {
            info!(account_id = %id, "CloudWatch Logs client created for AWS account");
        } else {
            info!("CloudWatch Logs client created successfully (account ID unknown)");
        }

        Self {
            client,
            account_id,
            log_groups_cache: Arc::new(Mutex::new(None)),
            query_cache: Arc::new(Mutex::new(CloudWatchDataCache::default())),
        }
    }

    /// Get the AWS account ID using STS
    async fn get_account_id(config: &SdkConfig) -> Option<String> {
        // Create STS client
        let sts_client = StsClient::new(config);

        // Call get-caller-identity
        match sts_client.get_caller_identity().send().await {
            Ok(response) => {
                let account_id = response.account();
                account_id.map(|id| id.to_string())
            }
            Err(err) => {
                // Log the error but don't fail - account ID is optional
                tracing::warn!(error = ?err, "Failed to get AWS account ID");
                None
            }
        }
    }

    /// Get the current AWS account ID
    pub fn account_id(&self) -> Option<&str> {
        self.account_id.as_deref()
    }

    /// Run a CloudWatch Insights query with caching and timeout
    pub async fn run_insights_query(
        &self,
        log_groups: Vec<String>,
        query_string: String,
        start_time: i64,
        end_time: i64,
        timeout_seconds: u64,
    ) -> Result<Vec<QueryResult>> {
        // Log account information if available
        if let Some(id) = &self.account_id {
            tracing::info!(account_id = %id, "Running CloudWatch Insights query in AWS account");
        }

        // Generate cache key with all parameters for exact matching
        let cache_key = CloudWatchDataCache::generate_cache_key(
            &query_string,
            &log_groups,
            start_time,
            end_time,
        );

        // Check cache for exact match
        let strategy = {
            let cache_guard = self.query_cache.lock().unwrap();
            cache_guard.determine_query_strategy(&cache_key)
        };

        match strategy {
            QueryStrategy::UseCache => {
                info!("Cache hit found for exact query match");
                let cache_guard = self.query_cache.lock().unwrap();
                let cached_data = cache_guard.get_cached_data(&cache_key).unwrap_or_default();

                // Convert DataPoint back to QueryResult
                Ok(cached_data
                    .into_iter()
                    .map(|dp| QueryResult { fields: dp.fields })
                    .collect())
            }
            QueryStrategy::FullQuery => {
                info!("No cache hit, executing full query");
                self.execute_and_cache_query(
                    log_groups,
                    query_string,
                    start_time,
                    end_time,
                    timeout_seconds,
                    cache_key,
                )
                .await
            }
        }
    }

    /// Execute query and store results in cache
    async fn execute_and_cache_query(
        &self,
        log_groups: Vec<String>,
        query_string: String,
        start_time: i64,
        end_time: i64,
        timeout_seconds: u64,
        cache_key: String,
    ) -> Result<Vec<QueryResult>> {
        let results = self
            .execute_raw_query(
                log_groups.clone(),
                query_string.clone(),
                start_time,
                end_time,
                timeout_seconds,
            )
            .await?;

        // Convert to DataPoint and cache
        let data_points: Vec<DataPoint> = results
            .iter()
            .map(|qr| DataPoint {
                fields: qr.fields.clone(),
            })
            .collect();

        let query_metadata = QueryMetadata {
            query_string,
            log_groups,
            time_range: (start_time, end_time),
        };

        // Store in cache (will only cache if query end time is >5 minutes ago)
        {
            let mut cache_guard = self.query_cache.lock().unwrap();
            cache_guard.store_results(cache_key, query_metadata, data_points)?;
        }

        Ok(results)
    }

    /// Execute raw query without caching
    async fn execute_raw_query(
        &self,
        log_groups: Vec<String>,
        query_string: String,
        start_time: i64,
        end_time: i64,
        timeout_seconds: u64,
    ) -> Result<Vec<QueryResult>> {
        // Start the query
        let query_id = self
            .start_query(log_groups, query_string, start_time, end_time)
            .await?;

        // Poll for results with timeout
        let start_time = Instant::now();
        let timeout = Duration::from_secs(timeout_seconds);

        loop {
            if start_time.elapsed() > timeout {
                return Err(anyhow!("Query timed out after {} seconds", timeout_seconds));
            }

            // Check query status
            let (status, results) = self.get_query_results(&query_id).await?;

            match status {
                QueryStatus::Complete => return Ok(results),
                QueryStatus::Failed => return Err(anyhow!("Query failed")),
                QueryStatus::Scheduled | QueryStatus::Running => {
                    // Wait a bit before polling again
                    sleep(Duration::from_secs(1)).await;
                }
                _ => {
                    // For other statuses (Cancelled, Timeout, etc.), treat as failure
                    return Err(anyhow!("Query failed with status: {:?}", status));
                }
            }
        }
    }

    /// Start a CloudWatch Insights query
    async fn start_query(
        &self,
        log_groups: Vec<String>,
        query_string: String,
        start_time: i64,
        end_time: i64,
    ) -> Result<String> {
        // Create the request
        let request = self
            .client
            .start_query()
            .query_string(query_string)
            .start_time(start_time)
            .end_time(end_time)
            .set_log_group_identifiers(Some(log_groups));

        // Execute the request
        let response = request
            .send()
            .await
            .context("Failed to start CloudWatch Logs query")?;

        // Extract the query ID
        let query_id = response
            .query_id
            .ok_or_else(|| anyhow!("Query ID not returned from CloudWatch"))?;

        Ok(query_id)
    }

    /// Get CloudWatch Insights query results
    async fn get_query_results(&self, query_id: &str) -> Result<(QueryStatus, Vec<QueryResult>)> {
        // Create and execute the request
        let request = self.client.get_query_results().query_id(query_id);
        let response = request
            .send()
            .await
            .context("Failed to get CloudWatch query results")?;

        // Extract the status
        let status = match response.status {
            Some(s) => s,
            None => QueryStatus::Failed,
        };

        // Extract and convert results
        let results = match response.results {
            Some(rows) => {
                let mut query_results = Vec::new();

                for row in rows {
                    let mut fields = HashMap::new();

                    for field in row {
                        if let (Some(key), Some(value)) = (field.field, field.value) {
                            fields.insert(key, value);
                        }
                    }

                    query_results.push(QueryResult { fields });
                }

                query_results
            }
            None => Vec::new(),
        };

        Ok((status, results))
    }

    /// List all CloudWatch log groups in the account and linked accounts
    ///
    /// Returns a vector of LogGroup objects, which contain name, ARN, and other metadata.
    /// For large numbers of log groups, this method handles pagination automatically.
    /// Includes log groups from linked accounts in monitoring account scenarios.
    ///
    /// Log groups are cached forever since they practically never change.
    #[instrument(skip(self), name = "list_log_groups")]
    pub async fn list_log_groups(&self) -> Result<Vec<LogGroup>> {
        // Check cache first
        {
            let cache_guard = self.log_groups_cache.lock().unwrap();
            if let Some(cached_groups) = cache_guard.as_ref() {
                info!(count = cached_groups.len(), "Returning cached log groups");
                return Ok(cached_groups.clone());
            }
        }

        // Cache miss - fetch from AWS
        info!("Cache miss - fetching log groups from AWS CloudWatch");

        // Log account information if available
        if let Some(id) = &self.account_id {
            info!(account_id = %id, "Listing all CloudWatch log groups in AWS account and linked accounts");
        } else {
            info!("Listing all CloudWatch log groups and linked accounts");
        }

        // Create a vector to store all log groups
        let mut log_groups = Vec::new();

        // Get the first page of results
        let mut next_token = None;

        loop {
            // Prepare request with pagination token if available and include linked accounts
            let mut request = self
                .client
                .describe_log_groups()
                .include_linked_accounts(true);
            if let Some(ref token) = next_token {
                request = request.next_token(token);
            }

            // Send the request
            tracing::debug!(?request, "Sending request to list CloudWatch log groups");
            debug!("Request details: {:?}", request);

            let response = match request.send().await {
                Ok(resp) => {
                    tracing::debug!(?resp, "Successfully received response from list_log_groups");
                    resp
                }
                Err(e) => {
                    tracing::error!(error = ?e, "Failed to list CloudWatch log groups");
                    return Err(anyhow::anyhow!(e).context("Failed to list CloudWatch log groups"));
                }
            };

            // Process the response
            if let Some(groups) = &response.log_groups {
                tracing::debug!(count = groups.len(), "Received log groups from CloudWatch");
                log_groups.extend(groups.clone());
            } else {
                warn!("No log groups returned in response");
            }

            // Check if there are more pages
            next_token = response.next_token.clone();
            if next_token.is_some() {
                tracing::debug!("More pages of log groups available");
            }

            // If no more pages, break the loop
            if next_token.is_none() {
                break;
            }
        }

        // Sort log groups by name for consistent results
        log_groups.sort_by(|a, b| {
            let a_name = a.log_group_name.as_deref().unwrap_or("");
            let b_name = b.log_group_name.as_deref().unwrap_or("");
            a_name.cmp(b_name)
        });

        // Cache the results
        {
            let mut cache_guard = self.log_groups_cache.lock().unwrap();
            *cache_guard = Some(log_groups.clone());
        }

        info!(
            count = log_groups.len(),
            "Cached log groups for future requests"
        );
        Ok(log_groups)
    }

    /// Helper method to filter log groups by pattern (case-insensitive)
    /// This centralizes the filtering logic to avoid duplication
    async fn filter_log_groups_internal(&self, pattern: &str) -> Result<Vec<LogGroup>> {
        // Get all log groups using the cached list
        let all_groups = self.list_log_groups().await?;

        // Convert pattern to lowercase for case-insensitive matching
        let pattern_lower = pattern.to_lowercase();

        // Filter groups that contain the pattern (case insensitive)
        let matching_groups: Vec<LogGroup> = all_groups
            .into_iter()
            .filter(|lg| {
                lg.log_group_name()
                    .map(|n| n.to_lowercase().contains(&pattern_lower))
                    .unwrap_or(false)
            })
            .collect();

        if matching_groups.is_empty() {
            return Err(anyhow!("No log groups found matching pattern: {}", pattern));
        }

        Ok(matching_groups)
    }

    /// Helper method to process ARN (remove ":*" suffix if present)
    fn process_arn(arn: &str) -> String {
        if arn.ends_with(":*") {
            // Strip the ":*" suffix
            arn[0..arn.len() - 2].to_string()
        } else {
            arn.to_string()
        }
    }

    /// Find log groups matching a pattern (case-insensitive) and return their processed ARNs
    ///
    /// This method handles:
    /// - Case-insensitive pattern matching against log group names
    /// - ARN processing (removing ":*" suffixes)
    /// - Filtering out groups without ARNs
    ///
    /// Returns a vector of processed log group ARNs suitable for CloudWatch Insights queries.
    #[instrument(skip(self), name = "find_log_groups")]
    pub async fn find_log_groups(&self, pattern: &str) -> Result<Vec<String>> {
        let matching_groups = self.filter_log_groups_internal(pattern).await?;

        // Extract and process ARNs
        let processed_arns: Vec<String> = matching_groups
            .into_iter()
            .filter_map(|lg| lg.arn)
            .map(|arn| Self::process_arn(&arn))
            .collect();

        info!(
            pattern = %pattern,
            found_groups = ?processed_arns,
            count = processed_arns.len(),
            "Found log groups matching pattern"
        );

        Ok(processed_arns)
    }

    /// Filter log groups by pattern (case-insensitive) and return the filtered LogGroup objects
    ///
    /// This method also processes ARNs (removes ":*" suffixes) in the returned LogGroup objects.
    /// This is useful when you need the full LogGroup metadata with clean ARNs.
    /// For CloudWatch Insights queries, use `find_log_groups()` instead.
    #[instrument(skip(self), name = "filter_log_groups")]
    pub async fn filter_log_groups(&self, pattern: &str) -> Result<Vec<LogGroup>> {
        let mut matching_groups = self.filter_log_groups_internal(pattern).await?;

        // Process ARNs in the LogGroup objects to remove ":*" suffixes
        for group in &mut matching_groups {
            if let Some(arn) = &group.arn {
                group.arn = Some(Self::process_arn(arn));
            }
        }

        info!(
            pattern = %pattern,
            count = matching_groups.len(),
            "Filtered log groups by pattern with processed ARNs"
        );

        Ok(matching_groups)
    }
}
