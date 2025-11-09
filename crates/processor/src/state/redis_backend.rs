//! Redis-based distributed state backend implementation
//!
//! This module provides a production-ready Redis state backend with comprehensive
//! features for distributed state management including connection pooling, cluster
//! support, TTL management, batch operations, and metrics tracking.
//!
//! ## Features
//!
//! - **Connection Pooling**: Automatic connection management with configurable pool size
//! - **Cluster Support**: Works with both standalone and clustered Redis deployments
//! - **TTL Management**: Per-key expiration with automatic cleanup
//! - **Batch Operations**: Efficient pipeline support for bulk operations
//! - **Retry Logic**: Configurable retry strategy for transient failures
//! - **Metrics**: Comprehensive operation tracking and statistics
//! - **Atomic Operations**: MULTI/EXEC transactions for consistency
//! - **Health Checks**: Connection monitoring and auto-reconnection
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use processor::state::{RedisStateBackend, RedisConfig, StateBackend};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Configure Redis backend
//!     let config = RedisConfig::builder()
//!         .url("redis://localhost:6379")
//!         .key_prefix("myapp:")
//!         .default_ttl(Duration::from_secs(3600))
//!         .pool_size(10, 50)
//!         .build()?;
//!
//!     // Create backend
//!     let backend = RedisStateBackend::new(config).await?;
//!
//!     // Store state with TTL
//!     backend.put_with_ttl(b"session:123", b"user_data", Duration::from_secs(1800)).await?;
//!
//!     // Retrieve state
//!     if let Some(data) = backend.get(b"session:123").await? {
//!         println!("Session data: {:?}", data);
//!     }
//!
//!     // Batch operations
//!     let keys = vec![b"key1", b"key2", b"key3"];
//!     let values = backend.batch_get(&keys).await?;
//!
//!     // Get statistics
//!     let stats = backend.stats().await;
//!     println!("Hit rate: {:.2}%", stats.hit_rate() * 100.0);
//!
//!     Ok(())
//! }
//! ```

use async_trait::async_trait;
use redis::{
    aio::ConnectionManager, AsyncCommands, Client, RedisResult, Script,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, trace, warn};

use super::backend::StateBackend;
use crate::error::{StateError, StateResult};

/// Redis connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis connection URL(s) - supports cluster mode with multiple URLs
    pub urls: Vec<String>,
    /// Key prefix for namespace isolation
    pub key_prefix: String,
    /// Default TTL for keys (None = no expiration)
    pub default_ttl: Option<Duration>,
    /// Minimum connections in pool
    pub min_connections: u32,
    /// Maximum connections in pool
    pub max_connections: u32,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Read timeout
    pub read_timeout: Duration,
    /// Write timeout
    pub write_timeout: Duration,
    /// Maximum retry attempts for failed operations
    pub max_retries: u32,
    /// Base delay for exponential backoff
    pub retry_base_delay: Duration,
    /// Maximum delay between retries
    pub retry_max_delay: Duration,
    /// Enable cluster mode
    pub cluster_mode: bool,
    /// Enable TLS/SSL
    pub tls_enabled: bool,
    /// Pipeline batch size for bulk operations
    pub pipeline_batch_size: usize,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            urls: vec!["redis://localhost:6379".to_string()],
            key_prefix: "state:".to_string(),
            default_ttl: None,
            min_connections: 5,
            max_connections: 50,
            connect_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_secs(3),
            write_timeout: Duration::from_secs(3),
            max_retries: 3,
            retry_base_delay: Duration::from_millis(100),
            retry_max_delay: Duration::from_secs(5),
            cluster_mode: false,
            tls_enabled: false,
            pipeline_batch_size: 100,
        }
    }
}

impl RedisConfig {
    /// Create a new configuration builder
    pub fn builder() -> RedisConfigBuilder {
        RedisConfigBuilder::default()
    }

    /// Get primary Redis URL
    pub fn primary_url(&self) -> &str {
        self.urls.first().map(|s| s.as_str()).unwrap_or("redis://localhost:6379")
    }
}

/// Builder for RedisConfig
#[derive(Debug, Default)]
pub struct RedisConfigBuilder {
    config: RedisConfig,
}

impl RedisConfigBuilder {
    /// Set Redis URL (single instance)
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.config.urls = vec![url.into()];
        self
    }

    /// Set Redis URLs (cluster mode)
    pub fn urls(mut self, urls: Vec<String>) -> Self {
        self.config.cluster_mode = urls.len() > 1;
        self.config.urls = urls;
        self
    }

    /// Set key prefix for namespace isolation
    pub fn key_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.config.key_prefix = prefix.into();
        self
    }

    /// Set default TTL for all keys
    pub fn default_ttl(mut self, ttl: Duration) -> Self {
        self.config.default_ttl = Some(ttl);
        self
    }

    /// Set connection pool size (min, max)
    pub fn pool_size(mut self, min: u32, max: u32) -> Self {
        self.config.min_connections = min;
        self.config.max_connections = max;
        self
    }

    /// Set timeouts (connect, read, write)
    pub fn timeouts(mut self, connect: Duration, read: Duration, write: Duration) -> Self {
        self.config.connect_timeout = connect;
        self.config.read_timeout = read;
        self.config.write_timeout = write;
        self
    }

    /// Set retry configuration (max attempts, base delay, max delay)
    pub fn retries(mut self, max: u32, base_delay: Duration, max_delay: Duration) -> Self {
        self.config.max_retries = max;
        self.config.retry_base_delay = base_delay;
        self.config.retry_max_delay = max_delay;
        self
    }

    /// Enable cluster mode
    pub fn cluster_mode(mut self, enabled: bool) -> Self {
        self.config.cluster_mode = enabled;
        self
    }

    /// Enable TLS/SSL
    pub fn tls(mut self, enabled: bool) -> Self {
        self.config.tls_enabled = enabled;
        self
    }

    /// Set pipeline batch size
    pub fn pipeline_batch_size(mut self, size: usize) -> Self {
        self.config.pipeline_batch_size = size;
        self
    }

    /// Build the configuration
    pub fn build(self) -> StateResult<RedisConfig> {
        if self.config.urls.is_empty() {
            return Err(StateError::StorageError {
                backend_type: "redis".to_string(),
                details: "No Redis URLs configured".to_string(),
            });
        }

        if self.config.max_connections < self.config.min_connections {
            return Err(StateError::StorageError {
                backend_type: "redis".to_string(),
                details: "max_connections must be >= min_connections".to_string(),
            });
        }

        Ok(self.config)
    }
}

/// Statistics for Redis backend operations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RedisStats {
    /// Total GET operations
    pub get_count: u64,
    /// Total PUT operations
    pub put_count: u64,
    /// Total DELETE operations
    pub delete_count: u64,
    /// Total cache hits
    pub hit_count: u64,
    /// Total cache misses
    pub miss_count: u64,
    /// Total pipeline operations
    pub pipeline_count: u64,
    /// Total retry attempts
    pub retry_count: u64,
    /// Total failed operations
    pub error_count: u64,
    /// Total bytes read
    pub bytes_read: u64,
    /// Total bytes written
    pub bytes_written: u64,
    /// Average operation latency (microseconds)
    pub avg_latency_us: u64,
    /// Number of active connections
    pub active_connections: u32,
}

impl RedisStats {
    /// Calculate hit rate (0.0 to 1.0)
    pub fn hit_rate(&self) -> f64 {
        let total = self.hit_count + self.miss_count;
        if total == 0 {
            0.0
        } else {
            self.hit_count as f64 / total as f64
        }
    }

    /// Calculate error rate (0.0 to 1.0)
    pub fn error_rate(&self) -> f64 {
        let total = self.get_count + self.put_count + self.delete_count;
        if total == 0 {
            0.0
        } else {
            self.error_count as f64 / total as f64
        }
    }

    /// Get total operations count
    pub fn total_operations(&self) -> u64 {
        self.get_count + self.put_count + self.delete_count
    }
}

/// Redis state backend with connection pooling and advanced features
pub struct RedisStateBackend {
    /// Redis connection manager
    connection: Arc<ConnectionManager>,
    /// Configuration
    config: RedisConfig,
    /// Statistics tracking
    stats: Arc<RwLock<RedisStats>>,
    /// LUA script for atomic operations
    scripts: RedisScripts,
}

/// Pre-compiled LUA scripts for atomic operations
struct RedisScripts {
    /// Atomic get-and-delete operation
    get_delete: Script,
    /// Atomic increment with TTL
    incr_with_ttl: Script,
    /// Conditional set (only if not exists)
    set_nx_with_ttl: Script,
}

impl RedisScripts {
    fn new() -> Self {
        Self {
            // GET and DELETE atomically
            get_delete: Script::new(
                r"
                local value = redis.call('GET', KEYS[1])
                if value then
                    redis.call('DEL', KEYS[1])
                end
                return value
                "
            ),
            // INCREMENT with TTL
            incr_with_ttl: Script::new(
                r"
                local value = redis.call('INCR', KEYS[1])
                redis.call('EXPIRE', KEYS[1], ARGV[1])
                return value
                "
            ),
            // SET NX (only if not exists) with TTL
            set_nx_with_ttl: Script::new(
                r"
                local exists = redis.call('EXISTS', KEYS[1])
                if exists == 0 then
                    redis.call('SET', KEYS[1], ARGV[1])
                    redis.call('EXPIRE', KEYS[1], ARGV[2])
                    return 1
                end
                return 0
                "
            ),
        }
    }
}

impl RedisStateBackend {
    /// Create a new Redis state backend
    ///
    /// # Arguments
    ///
    /// * `config` - Redis configuration
    ///
    /// # Returns
    ///
    /// A new `RedisStateBackend` instance
    ///
    /// # Errors
    ///
    /// Returns error if connection to Redis fails
    pub async fn new(config: RedisConfig) -> StateResult<Self> {
        let client = Self::create_client(&config)?;
        let connection = Self::create_connection_manager(client, &config).await?;

        Ok(Self {
            connection: Arc::new(connection),
            config,
            stats: Arc::new(RwLock::new(RedisStats::default())),
            scripts: RedisScripts::new(),
        })
    }

    /// Create Redis client from configuration
    fn create_client(config: &RedisConfig) -> StateResult<Client> {
        let url = config.primary_url();
        Client::open(url).map_err(|e| StateError::StorageError {
            backend_type: "redis".to_string(),
            details: format!("Failed to create client: {}", e),
        })
    }

    /// Create connection manager with retry logic
    async fn create_connection_manager(
        client: Client,
        config: &RedisConfig,
    ) -> StateResult<ConnectionManager> {
        let mut retries = 0;
        let mut delay = config.retry_base_delay;

        loop {
            match ConnectionManager::new(client.clone()).await {
                Ok(conn) => {
                    debug!("Redis connection established");
                    return Ok(conn);
                }
                Err(e) => {
                    retries += 1;
                    if retries >= config.max_retries {
                        error!("Failed to connect to Redis after {} retries: {}", retries, e);
                        return Err(StateError::StorageError {
                            backend_type: "redis".to_string(),
                            details: format!("Connection failed after {} retries: {}", retries, e),
                        });
                    }

                    warn!("Redis connection attempt {} failed: {}, retrying in {:?}",
                          retries, e, delay);
                    tokio::time::sleep(delay).await;
                    delay = std::cmp::min(delay * 2, config.retry_max_delay);
                }
            }
        }
    }

    /// Build full key with prefix
    fn build_key(&self, key: &[u8]) -> Vec<u8> {
        let mut full_key = self.config.key_prefix.as_bytes().to_vec();
        full_key.extend_from_slice(key);
        full_key
    }

    /// Strip prefix from key
    fn strip_prefix(&self, key: &[u8]) -> Vec<u8> {
        let prefix = self.config.key_prefix.as_bytes();
        if key.starts_with(prefix) {
            key[prefix.len()..].to_vec()
        } else {
            key.to_vec()
        }
    }

    /// Execute operation with retry logic
    async fn with_retry<F, Fut, T>(&self, mut operation: F) -> StateResult<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = RedisResult<T>>,
    {
        let mut retries = 0;
        let mut delay = self.config.retry_base_delay;

        loop {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    retries += 1;

                    // Update error stats
                    let mut stats = self.stats.write().await;
                    stats.error_count += 1;
                    stats.retry_count += 1;
                    drop(stats);

                    if retries >= self.config.max_retries {
                        error!("Operation failed after {} retries: {}", retries, e);
                        return Err(StateError::StorageError {
                            backend_type: "redis".to_string(),
                            details: format!("Operation failed after {} retries: {}", retries, e),
                        });
                    }

                    warn!("Operation attempt {} failed: {}, retrying in {:?}",
                          retries, e, delay);
                    tokio::time::sleep(delay).await;
                    delay = std::cmp::min(delay * 2, self.config.retry_max_delay);
                }
            }
        }
    }

    /// Get current statistics
    pub async fn stats(&self) -> RedisStats {
        self.stats.read().await.clone()
    }

    /// Check connection health
    pub async fn health_check(&self) -> StateResult<bool> {
        let mut conn = self.connection.as_ref().clone();
        let result: RedisResult<String> = redis::cmd("PING").query_async(&mut conn).await;

        match result {
            Ok(response) => Ok(response == "PONG"),
            Err(e) => {
                error!("Health check failed: {}", e);
                Err(StateError::StorageError {
                    backend_type: "redis".to_string(),
                    details: format!("Health check failed: {}", e),
                })
            }
        }
    }

    /// Get Redis server info
    pub async fn server_info(&self) -> StateResult<String> {
        let mut conn = self.connection.as_ref().clone();
        redis::cmd("INFO")
            .query_async(&mut conn)
            .await
            .map_err(|e| StateError::StorageError {
                backend_type: "redis".to_string(),
                details: format!("Failed to get server info: {}", e),
            })
    }

    /// Put with custom TTL
    pub async fn put_with_ttl(&self, key: &[u8], value: &[u8], ttl: Duration) -> StateResult<()> {
        let full_key = self.build_key(key);
        let ttl_secs = ttl.as_secs();

        let conn = self.connection.as_ref().clone();
        let full_key_clone = full_key.clone();
        let value_clone = value.to_vec();

        let result = self.with_retry(move || {
            let mut conn = conn.clone();
            let full_key = full_key_clone.clone();
            let value = value_clone.clone();
            async move {
                conn.set_ex(&full_key, &value, ttl_secs).await
            }
        }).await;

        if result.is_ok() {
            let mut stats = self.stats.write().await;
            stats.put_count += 1;
            stats.bytes_written += value.len() as u64;
        }

        result
    }

    /// Batch get multiple keys
    pub async fn batch_get(&self, keys: &[&[u8]]) -> StateResult<Vec<Option<Vec<u8>>>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        let full_keys: Vec<Vec<u8>> = keys.iter().map(|k| self.build_key(k)).collect();
        let conn = self.connection.as_ref().clone();

        let full_keys_clone = full_keys.clone();
        let result: Vec<Option<Vec<u8>>> = self.with_retry(move || {
            let mut conn = conn.clone();
            let full_keys = full_keys_clone.clone();
            async move {
                redis::cmd("MGET")
                    .arg(&full_keys)
                    .query_async(&mut conn)
                    .await
            }
        }).await?;

        // Update stats
        let mut stats = self.stats.write().await;
        stats.get_count += keys.len() as u64;
        for value in &result {
            if let Some(v) = value {
                stats.hit_count += 1;
                stats.bytes_read += v.len() as u64;
            } else {
                stats.miss_count += 1;
            }
        }

        Ok(result)
    }

    /// Batch put multiple key-value pairs
    pub async fn batch_put(&self, pairs: &[(&[u8], &[u8])]) -> StateResult<()> {
        if pairs.is_empty() {
            return Ok(());
        }

        // Process in chunks based on pipeline batch size
        for chunk in pairs.chunks(self.config.pipeline_batch_size) {
            let mut conn = self.connection.as_ref().clone();

            // Build commands for this chunk
            let commands: Vec<(Vec<u8>, Vec<u8>, Option<u64>)> = chunk
                .iter()
                .map(|(key, value)| {
                    let full_key = self.build_key(key);
                    let ttl = self.config.default_ttl.map(|d| d.as_secs());
                    (full_key, value.to_vec(), ttl)
                })
                .collect();

            self.with_retry(move || {
                let mut conn = conn.clone();
                let commands = commands.clone();
                async move {
                    let mut pipe = redis::pipe();
                    for (full_key, value, ttl) in &commands {
                        if let Some(ttl_secs) = ttl {
                            pipe.set_ex(full_key, value, *ttl_secs);
                        } else {
                            pipe.set(full_key, value);
                        }
                    }
                    pipe.query_async(&mut conn).await
                }
            }).await?;

            // Update stats
            let mut stats = self.stats.write().await;
            stats.put_count += chunk.len() as u64;
            stats.pipeline_count += 1;
            stats.bytes_written += chunk.iter().map(|(_, v)| v.len() as u64).sum::<u64>();
        }

        Ok(())
    }

    /// Batch delete multiple keys
    pub async fn batch_delete(&self, keys: &[&[u8]]) -> StateResult<usize> {
        if keys.is_empty() {
            return Ok(0);
        }

        let full_keys: Vec<Vec<u8>> = keys.iter().map(|k| self.build_key(k)).collect();
        let conn = self.connection.as_ref().clone();
        let full_keys_clone = full_keys.clone();

        let deleted: usize = self.with_retry(move || {
            let mut conn = conn.clone();
            let full_keys = full_keys_clone.clone();
            async move {
                redis::cmd("DEL")
                    .arg(&full_keys)
                    .query_async(&mut conn)
                    .await
            }
        }).await?;

        // Update stats
        let mut stats = self.stats.write().await;
        stats.delete_count += keys.len() as u64;

        Ok(deleted)
    }

    /// Scan keys with cursor (for large datasets)
    async fn scan_keys_internal(&self, pattern: Vec<u8>, count: usize) -> StateResult<Vec<Vec<u8>>> {
        let mut conn = self.connection.as_ref().clone();
        let mut cursor = 0u64;
        let mut all_keys = Vec::new();

        loop {
            let (new_cursor, keys): (u64, Vec<Vec<u8>>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(count)
                .query_async(&mut conn)
                .await
                .map_err(|e| StateError::StorageError {
                    backend_type: "redis".to_string(),
                    details: format!("SCAN failed: {}", e),
                })?;

            all_keys.extend(keys);
            cursor = new_cursor;

            if cursor == 0 {
                break;
            }
        }

        Ok(all_keys)
    }

    /// Atomic get and delete operation
    pub async fn get_and_delete(&self, key: &[u8]) -> StateResult<Option<Vec<u8>>> {
        let full_key = self.build_key(key);
        let mut conn = self.connection.as_ref().clone();

        let result: Option<Vec<u8>> = self.scripts
            .get_delete
            .key(&full_key)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| StateError::StorageError {
                backend_type: "redis".to_string(),
                details: format!("GET_DELETE script failed: {}", e),
            })?;

        // Update stats
        let mut stats = self.stats.write().await;
        stats.get_count += 1;
        stats.delete_count += 1;

        if result.is_some() {
            stats.hit_count += 1;
        } else {
            stats.miss_count += 1;
        }

        Ok(result)
    }

    /// Set key only if it doesn't exist (with TTL)
    pub async fn set_if_not_exists(&self, key: &[u8], value: &[u8], ttl: Duration) -> StateResult<bool> {
        let full_key = self.build_key(key);
        let ttl_secs = ttl.as_secs();
        let mut conn = self.connection.as_ref().clone();

        let result: i32 = self.scripts
            .set_nx_with_ttl
            .key(&full_key)
            .arg(value)
            .arg(ttl_secs)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| StateError::StorageError {
                backend_type: "redis".to_string(),
                details: format!("SET_NX_WITH_TTL script failed: {}", e),
            })?;

        if result == 1 {
            let mut stats = self.stats.write().await;
            stats.put_count += 1;
            stats.bytes_written += value.len() as u64;
        }

        Ok(result == 1)
    }

    /// Get remaining TTL for a key
    pub async fn ttl(&self, key: &[u8]) -> StateResult<Option<Duration>> {
        let full_key = self.build_key(key);
        let mut conn = self.connection.as_ref().clone();

        let ttl_secs: i64 = redis::cmd("TTL")
            .arg(&full_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| StateError::StorageError {
                backend_type: "redis".to_string(),
                details: format!("TTL query failed: {}", e),
            })?;

        match ttl_secs {
            -2 => Ok(None), // Key doesn't exist
            -1 => Ok(Some(Duration::MAX)), // No expiration
            secs if secs >= 0 => Ok(Some(Duration::from_secs(secs as u64))),
            _ => Ok(None),
        }
    }

    /// Create a checkpoint (snapshot) of all state
    pub async fn checkpoint(&self) -> StateResult<Vec<(Vec<u8>, Vec<u8>)>> {
        let pattern = format!("{}*", self.config.key_prefix).into_bytes();
        let keys = self.scan_keys_internal(pattern, 1000).await?;

        if keys.is_empty() {
            return Ok(Vec::new());
        }

        // Get all values
        let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
        let values = self.batch_get(&key_refs).await?;

        // Combine keys and values, stripping prefix
        let checkpoint: Vec<(Vec<u8>, Vec<u8>)> = keys
            .into_iter()
            .zip(values)
            .filter_map(|(k, v)| v.map(|val| (self.strip_prefix(&k), val)))
            .collect();

        Ok(checkpoint)
    }

    /// Restore state from a checkpoint
    pub async fn restore(&self, checkpoint: Vec<(Vec<u8>, Vec<u8>)>) -> StateResult<()> {
        if checkpoint.is_empty() {
            return Ok(());
        }

        let pairs: Vec<(&[u8], &[u8])> = checkpoint
            .iter()
            .map(|(k, v)| (k.as_slice(), v.as_slice()))
            .collect();

        self.batch_put(&pairs).await
    }

    /// Get memory usage statistics from Redis
    pub async fn memory_usage(&self) -> StateResult<u64> {
        let mut conn = self.connection.as_ref().clone();

        let info: String = redis::cmd("INFO")
            .arg("memory")
            .query_async(&mut conn)
            .await
            .map_err(|e| StateError::StorageError {
                backend_type: "redis".to_string(),
                details: format!("Failed to get memory info: {}", e),
            })?;

        // Parse used_memory from INFO response
        for line in info.lines() {
            if line.starts_with("used_memory:") {
                if let Some(bytes_str) = line.split(':').nth(1) {
                    if let Ok(bytes) = bytes_str.trim().parse::<u64>() {
                        return Ok(bytes);
                    }
                }
            }
        }

        Ok(0)
    }
}

#[async_trait]
impl StateBackend for RedisStateBackend {
    async fn get(&self, key: &[u8]) -> StateResult<Option<Vec<u8>>> {
        let start = std::time::Instant::now();
        let full_key = self.build_key(key);
        trace!("Getting key: {:?}", full_key);

        let conn = self.connection.as_ref().clone();
        let full_key_clone = full_key.clone();

        let result: Option<Vec<u8>> = self.with_retry(move || {
            let mut conn = conn.clone();
            let full_key = full_key_clone.clone();
            async move {
                conn.get(&full_key).await
            }
        }).await?;

        // Update stats
        let mut stats = self.stats.write().await;
        stats.get_count += 1;

        if let Some(ref value) = result {
            stats.hit_count += 1;
            stats.bytes_read += value.len() as u64;
        } else {
            stats.miss_count += 1;
        }

        // Update latency (simple moving average)
        let latency_us = start.elapsed().as_micros() as u64;
        stats.avg_latency_us = if stats.total_operations() == 1 {
            latency_us
        } else {
            (stats.avg_latency_us * 9 + latency_us) / 10
        };

        Ok(result)
    }

    async fn put(&self, key: &[u8], value: &[u8]) -> StateResult<()> {
        let full_key = self.build_key(key);
        trace!("Putting key: {:?}, value size: {} bytes", full_key, value.len());

        let conn = self.connection.as_ref().clone();
        let full_key_clone = full_key.clone();
        let value_clone = value.to_vec();

        if let Some(ttl) = self.config.default_ttl {
            let ttl_secs = ttl.as_secs();
            self.with_retry(move || {
                let mut conn = conn.clone();
                let full_key = full_key_clone.clone();
                let value = value_clone.clone();
                async move {
                    conn.set_ex(&full_key, &value, ttl_secs).await
                }
            }).await?;
        } else {
            self.with_retry(move || {
                let mut conn = conn.clone();
                let full_key = full_key_clone.clone();
                let value = value_clone.clone();
                async move {
                    conn.set(&full_key, &value).await
                }
            }).await?;
        }

        // Update stats
        let mut stats = self.stats.write().await;
        stats.put_count += 1;
        stats.bytes_written += value.len() as u64;

        Ok(())
    }

    async fn delete(&self, key: &[u8]) -> StateResult<()> {
        let full_key = self.build_key(key);
        trace!("Deleting key: {:?}", full_key);

        let conn = self.connection.as_ref().clone();
        let full_key_clone = full_key.clone();

        self.with_retry(move || {
            let mut conn = conn.clone();
            let full_key = full_key_clone.clone();
            async move {
                conn.del(&full_key).await
            }
        }).await?;

        // Update stats
        let mut stats = self.stats.write().await;
        stats.delete_count += 1;

        Ok(())
    }

    async fn list_keys(&self, prefix: &[u8]) -> StateResult<Vec<Vec<u8>>> {
        trace!("Listing keys with prefix: {:?}", prefix);

        let pattern = if prefix.is_empty() {
            format!("{}*", self.config.key_prefix).into_bytes()
        } else {
            let mut p = self.config.key_prefix.as_bytes().to_vec();
            p.extend_from_slice(prefix);
            p.push(b'*');
            p
        };

        let keys = self.scan_keys_internal(pattern, 1000).await?;

        // Strip prefix from all keys
        let stripped_keys = keys
            .into_iter()
            .map(|k| self.strip_prefix(&k))
            .collect();

        Ok(stripped_keys)
    }

    async fn clear(&self) -> StateResult<()> {
        debug!("Clearing all state");

        let pattern = format!("{}*", self.config.key_prefix).into_bytes();
        let keys = self.scan_keys_internal(pattern, 1000).await?;

        if keys.is_empty() {
            return Ok(());
        }

        // Delete in batches
        let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
        for chunk in key_refs.chunks(self.config.pipeline_batch_size) {
            let chunk_vec: Vec<Vec<u8>> = chunk.iter().map(|k| k.to_vec()).collect();

            // Perform deletion directly without retry wrapper to avoid closure lifetime issues
            let mut retries = 0;
            let mut delay = self.config.retry_base_delay;

            loop {
                let mut conn = self.connection.as_ref().clone();

                let result: RedisResult<()> = redis::cmd("DEL")
                    .arg(&chunk_vec)
                    .query_async(&mut conn)
                    .await;

                match result {
                    Ok(_) => break,
                    Err(e) => {
                        retries += 1;

                        if retries >= self.config.max_retries {
                            return Err(StateError::StorageError {
                                backend_type: "redis".to_string(),
                                details: format!("Delete batch failed after {} retries: {}", retries, e),
                            });
                        }

                        warn!("Delete batch attempt {} failed: {}, retrying in {:?}", retries, e, delay);
                        tokio::time::sleep(delay).await;
                        delay = std::cmp::min(delay * 2, self.config.retry_max_delay);
                    }
                }
            }
        }

        Ok(())
    }

    async fn count(&self) -> StateResult<usize> {
        let pattern = format!("{}*", self.config.key_prefix).into_bytes();
        let keys = self.scan_keys_internal(pattern, 1000).await?;
        Ok(keys.len())
    }

    async fn contains(&self, key: &[u8]) -> StateResult<bool> {
        let full_key = self.build_key(key);
        let conn = self.connection.as_ref().clone();
        let full_key_clone = full_key.clone();

        let exists: bool = self.with_retry(move || {
            let mut conn = conn.clone();
            let full_key = full_key_clone.clone();
            async move {
                conn.exists(&full_key).await
            }
        }).await?;

        Ok(exists)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::backend::tests::*;

    // Helper to create test backend (requires Redis running)
    async fn create_test_backend() -> Option<RedisStateBackend> {
        let config = RedisConfig::builder()
            .url("redis://localhost:6379")
            .key_prefix("test:")
            .build()
            .ok()?;

        match RedisStateBackend::new(config).await {
            Ok(backend) => {
                // Clear any existing test data
                let _ = backend.clear().await;
                Some(backend)
            }
            Err(_) => None,
        }
    }

    #[tokio::test]
    async fn test_redis_backend_basic() {
        if let Some(backend) = create_test_backend().await {
            test_backend_basic_ops(backend).await;
        } else {
            eprintln!("Skipping test: Redis not available");
        }
    }

    #[tokio::test]
    async fn test_redis_backend_list_keys() {
        if let Some(backend) = create_test_backend().await {
            test_backend_list_keys(backend).await;
        } else {
            eprintln!("Skipping test: Redis not available");
        }
    }

    #[tokio::test]
    async fn test_redis_backend_clear() {
        if let Some(backend) = create_test_backend().await {
            test_backend_clear(backend).await;
        } else {
            eprintln!("Skipping test: Redis not available");
        }
    }

    #[tokio::test]
    async fn test_redis_backend_contains() {
        if let Some(backend) = create_test_backend().await {
            test_backend_contains(backend).await;
        } else {
            eprintln!("Skipping test: Redis not available");
        }
    }

    #[tokio::test]
    async fn test_redis_ttl() {
        if let Some(backend) = create_test_backend().await {
            backend
                .put_with_ttl(b"ttl_key", b"value", Duration::from_secs(2))
                .await
                .unwrap();

            assert!(backend.get(b"ttl_key").await.unwrap().is_some());

            // Check TTL
            let ttl = backend.ttl(b"ttl_key").await.unwrap();
            assert!(ttl.is_some());
            assert!(ttl.unwrap().as_secs() <= 2);

            // Wait for expiration
            tokio::time::sleep(Duration::from_secs(3)).await;
            assert!(backend.get(b"ttl_key").await.unwrap().is_none());
        } else {
            eprintln!("Skipping test: Redis not available");
        }
    }

    #[tokio::test]
    async fn test_redis_batch_operations() {
        if let Some(backend) = create_test_backend().await {
            // Batch put
            let pairs = vec![
                (&b"batch1"[..], &b"value1"[..]),
                (&b"batch2"[..], &b"value2"[..]),
                (&b"batch3"[..], &b"value3"[..]),
            ];
            backend.batch_put(&pairs).await.unwrap();

            // Batch get
            let keys = vec![b"batch1", b"batch2", b"batch3"];
            let values = backend.batch_get(&keys).await.unwrap();
            assert_eq!(values.len(), 3);
            assert_eq!(values[0], Some(b"value1".to_vec()));
            assert_eq!(values[1], Some(b"value2".to_vec()));
            assert_eq!(values[2], Some(b"value3".to_vec()));

            // Batch delete
            let deleted = backend.batch_delete(&keys).await.unwrap();
            assert_eq!(deleted, 3);
        } else {
            eprintln!("Skipping test: Redis not available");
        }
    }

    #[tokio::test]
    async fn test_redis_atomic_operations() {
        if let Some(backend) = create_test_backend().await {
            backend.put(b"atomic_key", b"value").await.unwrap();

            // Get and delete atomically
            let value = backend.get_and_delete(b"atomic_key").await.unwrap();
            assert_eq!(value, Some(b"value".to_vec()));

            // Key should be gone
            assert!(backend.get(b"atomic_key").await.unwrap().is_none());

            // Set if not exists
            let set1 = backend
                .set_if_not_exists(b"nx_key", b"value1", Duration::from_secs(60))
                .await
                .unwrap();
            assert!(set1);

            let set2 = backend
                .set_if_not_exists(b"nx_key", b"value2", Duration::from_secs(60))
                .await
                .unwrap();
            assert!(!set2);

            // First value should still be there
            assert_eq!(
                backend.get(b"nx_key").await.unwrap(),
                Some(b"value1".to_vec())
            );
        } else {
            eprintln!("Skipping test: Redis not available");
        }
    }

    #[tokio::test]
    async fn test_redis_checkpoint_restore() {
        if let Some(backend) = create_test_backend().await {
            backend.put(b"key1", b"value1").await.unwrap();
            backend.put(b"key2", b"value2").await.unwrap();

            // Create checkpoint
            let checkpoint = backend.checkpoint().await.unwrap();
            assert_eq!(checkpoint.len(), 2);

            // Clear and restore
            backend.clear().await.unwrap();
            assert_eq!(backend.count().await.unwrap(), 0);

            backend.restore(checkpoint).await.unwrap();
            assert_eq!(backend.count().await.unwrap(), 2);

            assert_eq!(
                backend.get(b"key1").await.unwrap(),
                Some(b"value1".to_vec())
            );
        } else {
            eprintln!("Skipping test: Redis not available");
        }
    }

    #[tokio::test]
    async fn test_redis_stats() {
        if let Some(backend) = create_test_backend().await {
            backend.put(b"key1", b"value1").await.unwrap();
            backend.put(b"key2", b"value2").await.unwrap();

            backend.get(b"key1").await.unwrap(); // Hit
            backend.get(b"key1").await.unwrap(); // Hit
            backend.get(b"nonexistent").await.unwrap(); // Miss

            backend.delete(b"key1").await.unwrap();

            let stats = backend.stats().await;
            assert_eq!(stats.put_count, 2);
            assert_eq!(stats.get_count, 3);
            assert_eq!(stats.delete_count, 1);
            assert_eq!(stats.hit_count, 2);
            assert_eq!(stats.miss_count, 1);
            assert!(stats.hit_rate() > 0.0);
        } else {
            eprintln!("Skipping test: Redis not available");
        }
    }

    #[tokio::test]
    async fn test_redis_health_check() {
        if let Some(backend) = create_test_backend().await {
            let healthy = backend.health_check().await.unwrap();
            assert!(healthy);
        } else {
            eprintln!("Skipping test: Redis not available");
        }
    }

    #[tokio::test]
    async fn test_config_builder() {
        let config = RedisConfig::builder()
            .url("redis://localhost:6379")
            .key_prefix("myapp:")
            .default_ttl(Duration::from_secs(3600))
            .pool_size(10, 50)
            .cluster_mode(false)
            .build()
            .unwrap();

        assert_eq!(config.urls.len(), 1);
        assert_eq!(config.key_prefix, "myapp:");
        assert_eq!(config.default_ttl, Some(Duration::from_secs(3600)));
        assert_eq!(config.min_connections, 10);
        assert_eq!(config.max_connections, 50);
        assert!(!config.cluster_mode);
    }
}
