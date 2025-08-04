use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;
use tracing::{debug, info};

/// In-memory cache for CloudWatch Insights query results
/// Simple result-set caching with exact query parameter matching
pub struct CloudWatchDataCache {
    /// Cache entries keyed by cache key hash
    entries: HashMap<String, CacheEntry>,
    /// Maximum number of cache entries to prevent unbounded growth
    max_entries: usize,
}

/// Individual cache entry containing complete query results
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// Unique identifier for this cache entry
    pub cache_key: String,
    /// Complete query results
    pub results: Vec<DataPoint>,
    /// Time when this entry was cached
    pub cached_at: SystemTime,
    /// Metadata about the original query
    pub query_metadata: QueryMetadata,
}

/// Individual data point from CloudWatch query result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    /// All fields from the CloudWatch query result
    pub fields: HashMap<String, String>,
}

/// Metadata about the original query used to generate cached data
#[derive(Debug, Clone)]
pub struct QueryMetadata {
    /// Query string
    pub query_string: String,
    /// Log groups queried
    pub log_groups: Vec<String>,
    /// Query time range (start, end) as Unix timestamps
    pub time_range: (i64, i64),
}

/// Strategy for handling a cache lookup request
#[derive(Debug)]
pub enum QueryStrategy {
    /// Use cached data without any CloudWatch query
    UseCache,
    /// Execute full query (no cache hit or too recent to cache)
    FullQuery,
}

impl CloudWatchDataCache {
    /// Create a new cache with specified maximum entries
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_entries,
        }
    }

    /// Generate cache key from query parameters including time range
    pub fn generate_cache_key(
        query_string: &str,
        log_groups: &[String],
        start_time: i64,
        end_time: i64,
    ) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        query_string.hash(&mut hasher);
        start_time.hash(&mut hasher);
        end_time.hash(&mut hasher);

        // Sort log groups for consistent hashing
        let mut sorted_groups = log_groups.to_vec();
        sorted_groups.sort();
        for group in sorted_groups {
            group.hash(&mut hasher);
        }

        format!("{:x}", hasher.finish())
    }

    /// Determine if we should use cache or execute a full query
    pub fn determine_query_strategy(&self, cache_key: &str) -> QueryStrategy {
        match self.entries.get(cache_key) {
            Some(_) => {
                info!("Cache hit found for query");
                QueryStrategy::UseCache
            }
            None => {
                debug!("No cache entry found, using full query");
                QueryStrategy::FullQuery
            }
        }
    }

    /// Check if query end time is old enough to be cached (5+ minutes ago)
    pub fn should_cache_query(end_time: i64) -> bool {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let age_seconds = now - end_time;
        let min_age_for_caching = 5 * 60; // 5 minutes

        age_seconds >= min_age_for_caching
    }

    /// Store complete query results in cache
    pub fn store_results(
        &mut self,
        cache_key: String,
        query_metadata: QueryMetadata,
        raw_results: Vec<DataPoint>,
    ) -> Result<()> {
        // Don't cache if query end time is too recent (less than 5 minutes ago)
        if !Self::should_cache_query(query_metadata.time_range.1) {
            debug!("Skipping cache for recent query (end time within 5 minutes)");
            return Ok(());
        }

        // Create cache entry with complete results
        let entry = CacheEntry {
            cache_key: cache_key.clone(),
            results: raw_results,
            cached_at: SystemTime::now(),
            query_metadata,
        };

        // Evict old entries if we're at capacity
        if self.entries.len() >= self.max_entries && !self.entries.contains_key(&cache_key) {
            self.evict_oldest_entry();
        }

        info!(
            "Cached query results with {} data points",
            entry.results.len()
        );
        self.entries.insert(cache_key, entry);

        Ok(())
    }

    /// Get cached results for exact query match
    pub fn get_cached_data(&self, cache_key: &str) -> Option<Vec<DataPoint>> {
        let entry = self.entries.get(cache_key)?;
        Some(entry.results.clone())
    }

    /// Evict the oldest cache entry to make room for new ones
    fn evict_oldest_entry(&mut self) {
        let oldest_key = self
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.cached_at)
            .map(|(key, _)| key.clone());

        if let Some(key) = oldest_key {
            info!("Evicting cache entry: {}", key);
            self.entries.remove(&key);
        }
    }

    /// Get cache statistics for debugging
    pub fn get_stats(&self) -> CacheStats {
        let total_data_points: usize = self.entries.values().map(|entry| entry.results.len()).sum();

        CacheStats {
            entry_count: self.entries.len(),
            total_data_points,
            max_entries: self.max_entries,
        }
    }
}

/// Cache statistics for monitoring and debugging
#[derive(Debug)]
pub struct CacheStats {
    pub entry_count: usize,
    pub total_data_points: usize,
    pub max_entries: usize,
}

impl Default for CloudWatchDataCache {
    fn default() -> Self {
        Self::new(100) // Default to 100 cache entries max
    }
}
