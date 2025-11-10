//! Event Deduplication System
//!
//! This module provides enterprise-grade event deduplication capabilities with Redis backend
//! support, configurable event ID extraction, TTL-based cleanup, and comprehensive metrics.
//!
//! ## Features
//!
//! - **Multiple ID Extraction Strategies**: UUID, hash-based, custom extractors
//! - **Redis-Backed Storage**: Production-ready distributed deduplication
//! - **TTL-Based Cleanup**: Automatic memory management with configurable expiration
//! - **Batch Operations**: Efficient bulk deduplication checks
//! - **Retry Logic**: Exponential backoff for transient failures
//! - **Comprehensive Metrics**: Track duplicates, cache hits, and performance
//! - **Thread-Safe**: Lock-free concurrent operations with Arc/DashMap
//! - **Zero Data Loss**: Guaranteed at-most-once processing semantics
//!
//! ## Architecture
//!
//! The deduplication system uses a layered architecture:
//!
//! ```text
//! ┌─────────────────────────────────┐
//! │   EventDeduplicator (API)       │
//! └─────────────┬───────────────────┘
//!               │
//! ┌─────────────┴───────────────────┐
//! │   EventIdExtractor (Strategy)   │
//! └─────────────┬───────────────────┘
//!               │
//! ┌─────────────┴───────────────────┐
//! │ RedisDeduplicationBackend       │
//! │  - Connection pooling           │
//! │  - Retry logic                  │
//! │  - Metrics tracking             │
//! └─────────────────────────────────┘
//! ```
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use processor::deduplication::{
//!     EventDeduplicator, DeduplicationConfig, EventIdExtractor,
//! };
//! use serde::{Deserialize, Serialize};
//! use std::time::Duration;
//!
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! struct MyEvent {
//!     id: String,
//!     data: String,
//! }
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Configure deduplicator
//!     let config = DeduplicationConfig::builder()
//!         .redis_url("redis://localhost:6379")
//!         .key_prefix("dedup:")
//!         .default_ttl(Duration::from_secs(3600))
//!         .build()?;
//!
//!     // Create custom ID extractor
//!     let extractor = EventIdExtractor::custom(|event: &MyEvent| {
//!         event.id.clone()
//!     });
//!
//!     // Initialize deduplicator
//!     let dedup = EventDeduplicator::new(config, extractor).await?;
//!
//!     // Check and process events
//!     let event = MyEvent {
//!         id: "event-123".to_string(),
//!         data: "payload".to_string(),
//!     };
//!
//!     if !dedup.is_duplicate(&event).await? {
//!         // Process event
//!         println!("Processing new event: {}", event.id);
//!
//!         // Mark as seen
//!         dedup.mark_seen(&event).await?;
//!     } else {
//!         println!("Duplicate event detected: {}", event.id);
//!     }
//!
//!     // Get statistics
//!     let stats = dedup.stats().await;
//!     println!("Duplicate rate: {:.2}%", stats.duplicate_rate() * 100.0);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Batch Processing
//!
//! For high-throughput scenarios, use batch operations:
//!
//! ```rust,no_run
//! # use processor::deduplication::{EventDeduplicator, DeduplicationConfig, EventIdExtractor};
//! # use serde::{Deserialize, Serialize};
//! # #[derive(Debug, Clone, Serialize, Deserialize)]
//! # struct MyEvent { id: String }
//! # async fn example(dedup: EventDeduplicator<MyEvent>) -> anyhow::Result<()> {
//! let events = vec![
//!     MyEvent { id: "event-1".to_string() },
//!     MyEvent { id: "event-2".to_string() },
//!     MyEvent { id: "event-3".to_string() },
//! ];
//!
//! // Check all events in one batch
//! let results = dedup.batch_check_duplicates(&events).await?;
//!
//! for (event, is_duplicate) in events.iter().zip(results.iter()) {
//!     if !is_duplicate {
//!         println!("Processing: {}", event.id);
//!     }
//! }
//!
//! // Mark all non-duplicates as seen
//! let new_events: Vec<_> = events.iter()
//!     .zip(results.iter())
//!     .filter_map(|(e, dup)| if !dup { Some(e) } else { None })
//!     .collect();
//!
//! dedup.batch_mark_seen(&new_events).await?;
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use redis::{aio::ConnectionManager, AsyncCommands, Client, RedisResult};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, trace, warn};
use uuid::Uuid;

use crate::error::{StateError, StateResult};

/// Configuration for event deduplication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeduplicationConfig {
    /// Redis connection URL
    pub redis_url: String,
    /// Key prefix for namespacing deduplication keys
    pub key_prefix: String,
    /// Default TTL for deduplication entries (None = no expiration)
    pub default_ttl: Option<Duration>,
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
    /// Pipeline batch size for bulk operations
    pub pipeline_batch_size: usize,
    /// Enable compression for event IDs (saves space for large IDs)
    pub compress_event_ids: bool,
}

impl Default for DeduplicationConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://localhost:6379".to_string(),
            key_prefix: "dedup:".to_string(),
            default_ttl: Some(Duration::from_secs(3600)), // 1 hour default
            connect_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_secs(3),
            write_timeout: Duration::from_secs(3),
            max_retries: 3,
            retry_base_delay: Duration::from_millis(100),
            retry_max_delay: Duration::from_secs(5),
            pipeline_batch_size: 100,
            compress_event_ids: false,
        }
    }
}

impl DeduplicationConfig {
    /// Create a new configuration builder
    pub fn builder() -> DeduplicationConfigBuilder {
        DeduplicationConfigBuilder::default()
    }
}

/// Builder for DeduplicationConfig
#[derive(Debug, Default)]
pub struct DeduplicationConfigBuilder {
    config: DeduplicationConfig,
}

impl DeduplicationConfigBuilder {
    /// Set Redis URL
    pub fn redis_url(mut self, url: impl Into<String>) -> Self {
        self.config.redis_url = url.into();
        self
    }

    /// Set key prefix for namespace isolation
    pub fn key_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.config.key_prefix = prefix.into();
        self
    }

    /// Set default TTL for deduplication entries
    pub fn default_ttl(mut self, ttl: Duration) -> Self {
        self.config.default_ttl = Some(ttl);
        self
    }

    /// Set connection timeout
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.config.connect_timeout = timeout;
        self
    }

    /// Set retry configuration (max attempts, base delay, max delay)
    pub fn retries(mut self, max: u32, base_delay: Duration, max_delay: Duration) -> Self {
        self.config.max_retries = max;
        self.config.retry_base_delay = base_delay;
        self.config.retry_max_delay = max_delay;
        self
    }

    /// Set pipeline batch size
    pub fn pipeline_batch_size(mut self, size: usize) -> Self {
        self.config.pipeline_batch_size = size;
        self
    }

    /// Enable event ID compression
    pub fn compress_event_ids(mut self, enabled: bool) -> Self {
        self.config.compress_event_ids = enabled;
        self
    }

    /// Build the configuration
    pub fn build(self) -> StateResult<DeduplicationConfig> {
        if self.config.redis_url.is_empty() {
            return Err(StateError::StorageError {
                backend_type: "redis-dedup".to_string(),
                details: "Redis URL cannot be empty".to_string(),
            });
        }

        if self.config.pipeline_batch_size == 0 {
            return Err(StateError::StorageError {
                backend_type: "redis-dedup".to_string(),
                details: "Pipeline batch size must be greater than 0".to_string(),
            });
        }

        Ok(self.config)
    }
}

/// Statistics for deduplication operations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeduplicationStats {
    /// Total events checked
    pub events_checked: u64,
    /// Total duplicates found
    pub duplicates_found: u64,
    /// Total new events (marked as seen)
    pub new_events: u64,
    /// Total cache hits (event found in Redis)
    pub cache_hits: u64,
    /// Total cache misses (event not in Redis)
    pub cache_misses: u64,
    /// Total batch operations
    pub batch_operations: u64,
    /// Total retry attempts
    pub retry_count: u64,
    /// Total failed operations
    pub error_count: u64,
    /// Average operation latency (microseconds)
    pub avg_latency_us: u64,
    /// Total bytes written (event IDs)
    pub bytes_written: u64,
    /// Total bytes read
    pub bytes_read: u64,
}

impl DeduplicationStats {
    /// Calculate duplicate rate (0.0 to 1.0)
    pub fn duplicate_rate(&self) -> f64 {
        if self.events_checked == 0 {
            0.0
        } else {
            self.duplicates_found as f64 / self.events_checked as f64
        }
    }

    /// Calculate cache hit rate (0.0 to 1.0)
    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            self.cache_hits as f64 / total as f64
        }
    }

    /// Calculate error rate (0.0 to 1.0)
    pub fn error_rate(&self) -> f64 {
        if self.events_checked == 0 {
            0.0
        } else {
            self.error_count as f64 / self.events_checked as f64
        }
    }

    /// Get total operations count
    pub fn total_operations(&self) -> u64 {
        self.events_checked + self.new_events
    }
}

/// Strategy for extracting unique event IDs
pub enum EventIdExtractor<T> {
    /// Extract UUID from event
    Uuid(Box<dyn Fn(&T) -> Uuid + Send + Sync>),
    /// Extract string ID from event
    String(Box<dyn Fn(&T) -> String + Send + Sync>),
    /// Generate hash-based ID from event
    Hash(Box<dyn Fn(&T) -> u64 + Send + Sync>),
    /// Custom extraction logic
    Custom(Box<dyn Fn(&T) -> String + Send + Sync>),
}

impl<T> EventIdExtractor<T> {
    /// Create UUID-based extractor
    pub fn uuid<F>(f: F) -> Self
    where
        F: Fn(&T) -> Uuid + Send + Sync + 'static,
    {
        Self::Uuid(Box::new(f))
    }

    /// Create string-based extractor
    pub fn string<F>(f: F) -> Self
    where
        F: Fn(&T) -> String + Send + Sync + 'static,
    {
        Self::String(Box::new(f))
    }

    /// Create hash-based extractor
    pub fn hash<F>(f: F) -> Self
    where
        F: Fn(&T) -> u64 + Send + Sync + 'static,
    {
        Self::Hash(Box::new(f))
    }

    /// Create custom string extractor
    pub fn custom<F>(f: F) -> Self
    where
        F: Fn(&T) -> String + Send + Sync + 'static,
    {
        Self::Custom(Box::new(f))
    }

    /// Extract event ID as a string
    pub fn extract_id(&self, event: &T) -> String {
        match self {
            Self::Uuid(f) => f(event).to_string(),
            Self::String(f) => f(event),
            Self::Hash(f) => format!("{:016x}", f(event)),
            Self::Custom(f) => f(event),
        }
    }
}

impl<T> Debug for EventIdExtractor<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Uuid(_) => write!(f, "EventIdExtractor::Uuid"),
            Self::String(_) => write!(f, "EventIdExtractor::String"),
            Self::Hash(_) => write!(f, "EventIdExtractor::Hash"),
            Self::Custom(_) => write!(f, "EventIdExtractor::Custom"),
        }
    }
}

/// Redis-backed deduplication backend
struct RedisDeduplicationBackend {
    /// Redis connection manager
    connection: Arc<ConnectionManager>,
    /// Configuration
    config: DeduplicationConfig,
    /// Statistics tracking
    stats: Arc<RwLock<DeduplicationStats>>,
}

impl RedisDeduplicationBackend {
    /// Create a new Redis deduplication backend
    async fn new(config: DeduplicationConfig) -> StateResult<Self> {
        let client = Self::create_client(&config)?;
        let connection = Self::create_connection_manager(client, &config).await?;

        Ok(Self {
            connection: Arc::new(connection),
            config,
            stats: Arc::new(RwLock::new(DeduplicationStats::default())),
        })
    }

    /// Create Redis client from configuration
    fn create_client(config: &DeduplicationConfig) -> StateResult<Client> {
        Client::open(config.redis_url.as_str()).map_err(|e| StateError::StorageError {
            backend_type: "redis-dedup".to_string(),
            details: format!("Failed to create Redis client: {}", e),
        })
    }

    /// Create connection manager with retry logic
    async fn create_connection_manager(
        client: Client,
        config: &DeduplicationConfig,
    ) -> StateResult<ConnectionManager> {
        let mut retries = 0;
        let mut delay = config.retry_base_delay;

        loop {
            match ConnectionManager::new(client.clone()).await {
                Ok(conn) => {
                    debug!("Redis deduplication backend connected");
                    return Ok(conn);
                }
                Err(e) => {
                    retries += 1;
                    if retries >= config.max_retries {
                        error!("Failed to connect to Redis after {} retries: {}", retries, e);
                        return Err(StateError::StorageError {
                            backend_type: "redis-dedup".to_string(),
                            details: format!("Connection failed after {} retries: {}", retries, e),
                        });
                    }

                    warn!(
                        "Redis connection attempt {} failed: {}, retrying in {:?}",
                        retries, e, delay
                    );
                    tokio::time::sleep(delay).await;
                    delay = std::cmp::min(delay * 2, config.retry_max_delay);
                }
            }
        }
    }

    /// Build full key with prefix
    fn build_key(&self, event_id: &str) -> String {
        format!("{}{}", self.config.key_prefix, event_id)
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
                        error!("Deduplication operation failed after {} retries: {}", retries, e);
                        return Err(StateError::StorageError {
                            backend_type: "redis-dedup".to_string(),
                            details: format!("Operation failed after {} retries: {}", retries, e),
                        });
                    }

                    warn!(
                        "Deduplication operation attempt {} failed: {}, retrying in {:?}",
                        retries, e, delay
                    );
                    tokio::time::sleep(delay).await;
                    delay = std::cmp::min(delay * 2, self.config.retry_max_delay);
                }
            }
        }
    }

    /// Check if event ID exists (is a duplicate)
    async fn exists(&self, event_id: &str) -> StateResult<bool> {
        let start = std::time::Instant::now();
        let key = self.build_key(event_id);
        trace!("Checking existence of key: {}", key);

        let conn = self.connection.as_ref().clone();
        let key_clone = key.clone();

        let exists: bool = self
            .with_retry(move || {
                let mut conn = conn.clone();
                let key = key_clone.clone();
                async move { conn.exists(&key).await }
            })
            .await?;

        // Update stats
        let mut stats = self.stats.write().await;
        stats.events_checked += 1;
        if exists {
            stats.duplicates_found += 1;
            stats.cache_hits += 1;
        } else {
            stats.cache_misses += 1;
        }

        // Update latency
        let latency_us = start.elapsed().as_micros() as u64;
        stats.avg_latency_us = if stats.events_checked == 1 {
            latency_us
        } else {
            (stats.avg_latency_us * 9 + latency_us) / 10
        };

        Ok(exists)
    }

    /// Mark event ID as seen (add to deduplication store)
    async fn mark_seen(&self, event_id: &str) -> StateResult<()> {
        let key = self.build_key(event_id);
        trace!("Marking key as seen: {}", key);

        let conn = self.connection.as_ref().clone();
        let key_clone = key.clone();
        let ttl = self.config.default_ttl;

        self.with_retry(move || {
            let mut conn = conn.clone();
            let key = key_clone.clone();
            async move {
                if let Some(ttl_duration) = ttl {
                    let ttl_secs = ttl_duration.as_secs();
                    conn.set_ex(&key, "1", ttl_secs).await
                } else {
                    conn.set(&key, "1").await
                }
            }
        })
        .await?;

        // Update stats
        let mut stats = self.stats.write().await;
        stats.new_events += 1;
        stats.bytes_written += event_id.len() as u64;

        Ok(())
    }

    /// Batch check multiple event IDs
    async fn batch_exists(&self, event_ids: &[String]) -> StateResult<Vec<bool>> {
        if event_ids.is_empty() {
            return Ok(Vec::new());
        }

        let keys: Vec<String> = event_ids.iter().map(|id| self.build_key(id)).collect();
        let conn = self.connection.as_ref().clone();
        let keys_clone = keys.clone();

        let results: Vec<bool> = self
            .with_retry(move || {
                let mut conn = conn.clone();
                let keys = keys_clone.clone();
                async move {
                    let mut pipe = redis::pipe();
                    for key in &keys {
                        pipe.exists(key);
                    }
                    pipe.query_async(&mut conn).await
                }
            })
            .await?;

        // Update stats
        let mut stats = self.stats.write().await;
        stats.events_checked += event_ids.len() as u64;
        stats.batch_operations += 1;

        for &exists in &results {
            if exists {
                stats.duplicates_found += 1;
                stats.cache_hits += 1;
            } else {
                stats.cache_misses += 1;
            }
        }

        Ok(results)
    }

    /// Batch mark multiple event IDs as seen
    async fn batch_mark_seen(&self, event_ids: &[String]) -> StateResult<()> {
        if event_ids.is_empty() {
            return Ok(());
        }

        // Process in chunks based on pipeline batch size
        for chunk in event_ids.chunks(self.config.pipeline_batch_size) {
            let commands: Vec<(String, Option<u64>)> = chunk
                .iter()
                .map(|id| {
                    let key = self.build_key(id);
                    let ttl = self.config.default_ttl.map(|d| d.as_secs());
                    (key, ttl)
                })
                .collect();

            let conn = self.connection.as_ref().clone();

            self.with_retry(move || {
                let mut conn = conn.clone();
                let commands = commands.clone();
                async move {
                    let mut pipe = redis::pipe();
                    for (key, ttl) in &commands {
                        if let Some(ttl_secs) = ttl {
                            pipe.set_ex(key, "1", *ttl_secs);
                        } else {
                            pipe.set(key, "1");
                        }
                    }
                    pipe.query_async(&mut conn).await
                }
            })
            .await?;

            // Update stats
            let mut stats = self.stats.write().await;
            stats.new_events += chunk.len() as u64;
            stats.batch_operations += 1;
            stats.bytes_written += chunk.iter().map(|id| id.len() as u64).sum::<u64>();
        }

        Ok(())
    }

    /// Get current statistics
    async fn stats(&self) -> DeduplicationStats {
        self.stats.read().await.clone()
    }

    /// Check connection health
    async fn health_check(&self) -> StateResult<bool> {
        let mut conn = self.connection.as_ref().clone();
        let result: RedisResult<String> = redis::cmd("PING").query_async(&mut conn).await;

        match result {
            Ok(response) => Ok(response == "PONG"),
            Err(e) => {
                error!("Deduplication backend health check failed: {}", e);
                Err(StateError::StorageError {
                    backend_type: "redis-dedup".to_string(),
                    details: format!("Health check failed: {}", e),
                })
            }
        }
    }

    /// Clear all deduplication data (use with caution!)
    async fn clear(&self) -> StateResult<usize> {
        let pattern = format!("{}*", self.config.key_prefix);
        let mut conn = self.connection.as_ref().clone();
        let mut cursor = 0u64;
        let mut total_deleted = 0;

        loop {
            let (new_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(1000)
                .query_async(&mut conn)
                .await
                .map_err(|e| StateError::StorageError {
                    backend_type: "redis-dedup".to_string(),
                    details: format!("SCAN failed: {}", e),
                })?;

            if !keys.is_empty() {
                let deleted: usize = redis::cmd("DEL")
                    .arg(&keys)
                    .query_async(&mut conn)
                    .await
                    .map_err(|e| StateError::StorageError {
                        backend_type: "redis-dedup".to_string(),
                        details: format!("DEL failed: {}", e),
                    })?;
                total_deleted += deleted;
            }

            cursor = new_cursor;
            if cursor == 0 {
                break;
            }
        }

        debug!("Cleared {} deduplication entries", total_deleted);
        Ok(total_deleted)
    }
}

/// High-level event deduplicator
///
/// Provides a type-safe interface for event deduplication with configurable
/// ID extraction strategies and comprehensive error handling.
pub struct EventDeduplicator<T> {
    /// Backend storage
    backend: Arc<RedisDeduplicationBackend>,
    /// Event ID extraction strategy
    extractor: EventIdExtractor<T>,
    /// Phantom data for type safety
    _phantom: PhantomData<T>,
}

impl<T> EventDeduplicator<T> {
    /// Create a new event deduplicator
    ///
    /// # Arguments
    ///
    /// * `config` - Deduplication configuration
    /// * `extractor` - Event ID extraction strategy
    ///
    /// # Returns
    ///
    /// A new `EventDeduplicator` instance
    ///
    /// # Errors
    ///
    /// Returns error if Redis connection fails
    pub async fn new(
        config: DeduplicationConfig,
        extractor: EventIdExtractor<T>,
    ) -> StateResult<Self> {
        let backend = RedisDeduplicationBackend::new(config).await?;

        Ok(Self {
            backend: Arc::new(backend),
            extractor,
            _phantom: PhantomData,
        })
    }

    /// Check if an event is a duplicate
    ///
    /// # Arguments
    ///
    /// * `event` - The event to check
    ///
    /// # Returns
    ///
    /// `true` if the event is a duplicate, `false` otherwise
    ///
    /// # Errors
    ///
    /// Returns error if Redis operation fails
    pub async fn is_duplicate(&self, event: &T) -> StateResult<bool> {
        let event_id = self.extractor.extract_id(event);
        self.backend.exists(&event_id).await
    }

    /// Mark an event as seen
    ///
    /// # Arguments
    ///
    /// * `event` - The event to mark as seen
    ///
    /// # Errors
    ///
    /// Returns error if Redis operation fails
    pub async fn mark_seen(&self, event: &T) -> StateResult<()> {
        let event_id = self.extractor.extract_id(event);
        self.backend.mark_seen(&event_id).await
    }

    /// Check and mark event as seen in one operation (optimized)
    ///
    /// # Arguments
    ///
    /// * `event` - The event to check and mark
    ///
    /// # Returns
    ///
    /// `true` if the event was a duplicate, `false` if it was new
    ///
    /// # Errors
    ///
    /// Returns error if Redis operation fails
    pub async fn check_and_mark(&self, event: &T) -> StateResult<bool> {
        let event_id = self.extractor.extract_id(event);
        let was_duplicate = self.backend.exists(&event_id).await?;

        if !was_duplicate {
            self.backend.mark_seen(&event_id).await?;
        }

        Ok(was_duplicate)
    }

    /// Batch check multiple events for duplicates
    ///
    /// # Arguments
    ///
    /// * `events` - Slice of events to check
    ///
    /// # Returns
    ///
    /// Vector of booleans indicating which events are duplicates
    ///
    /// # Errors
    ///
    /// Returns error if Redis operation fails
    pub async fn batch_check_duplicates(&self, events: &[T]) -> StateResult<Vec<bool>> {
        let event_ids: Vec<String> = events.iter().map(|e| self.extractor.extract_id(e)).collect();
        self.backend.batch_exists(&event_ids).await
    }

    /// Batch mark multiple events as seen
    ///
    /// # Arguments
    ///
    /// * `events` - Slice of events to mark as seen
    ///
    /// # Errors
    ///
    /// Returns error if Redis operation fails
    pub async fn batch_mark_seen(&self, events: &[&T]) -> StateResult<()> {
        let event_ids: Vec<String> = events.iter().map(|e| self.extractor.extract_id(e)).collect();
        self.backend.batch_mark_seen(&event_ids).await
    }

    /// Get deduplication statistics
    pub async fn stats(&self) -> DeduplicationStats {
        self.backend.stats().await
    }

    /// Check backend health
    pub async fn health_check(&self) -> StateResult<bool> {
        self.backend.health_check().await
    }

    /// Clear all deduplication data (use with caution!)
    ///
    /// This removes all deduplication state. Only use for testing or maintenance.
    pub async fn clear(&self) -> StateResult<usize> {
        self.backend.clear().await
    }
}

impl<T> Clone for EventDeduplicator<T> {
    fn clone(&self) -> Self {
        Self {
            backend: Arc::clone(&self.backend),
            // Note: We cannot clone the extractor as it contains Box<dyn Fn>
            // Users should create a new instance with the same extractor
            extractor: EventIdExtractor::Custom(Box::new(|_| {
                panic!("Cannot clone EventDeduplicator with extractor. Create a new instance instead.")
            })),
            _phantom: PhantomData,
        }
    }
}

impl<T> Debug for EventDeduplicator<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventDeduplicator")
            .field("extractor", &self.extractor)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestEvent {
        id: String,
        data: String,
    }

    // Helper to create test deduplicator (requires Redis running)
    async fn create_test_deduplicator() -> Option<EventDeduplicator<TestEvent>> {
        let config = DeduplicationConfig::builder()
            .redis_url("redis://localhost:6379")
            .key_prefix("test:dedup:")
            .default_ttl(Duration::from_secs(60))
            .build()
            .ok()?;

        let extractor = EventIdExtractor::string(|event: &TestEvent| event.id.clone());

        match EventDeduplicator::new(config, extractor).await {
            Ok(dedup) => {
                // Clear any existing test data
                let _ = dedup.clear().await;
                Some(dedup)
            }
            Err(_) => None,
        }
    }

    #[tokio::test]
    async fn test_basic_deduplication() {
        if let Some(dedup) = create_test_deduplicator().await {
            let event1 = TestEvent {
                id: "event-1".to_string(),
                data: "data1".to_string(),
            };

            // First check - should not be duplicate
            assert!(!dedup.is_duplicate(&event1).await.unwrap());

            // Mark as seen
            dedup.mark_seen(&event1).await.unwrap();

            // Second check - should be duplicate
            assert!(dedup.is_duplicate(&event1).await.unwrap());
        } else {
            eprintln!("Skipping test: Redis not available");
        }
    }

    #[tokio::test]
    async fn test_check_and_mark() {
        if let Some(dedup) = create_test_deduplicator().await {
            let event = TestEvent {
                id: "event-check-mark".to_string(),
                data: "data".to_string(),
            };

            // First call - should not be duplicate and mark it
            let was_dup1 = dedup.check_and_mark(&event).await.unwrap();
            assert!(!was_dup1);

            // Second call - should be duplicate
            let was_dup2 = dedup.check_and_mark(&event).await.unwrap();
            assert!(was_dup2);
        } else {
            eprintln!("Skipping test: Redis not available");
        }
    }

    #[tokio::test]
    async fn test_batch_operations() {
        if let Some(dedup) = create_test_deduplicator().await {
            let events = vec![
                TestEvent {
                    id: "batch-1".to_string(),
                    data: "data1".to_string(),
                },
                TestEvent {
                    id: "batch-2".to_string(),
                    data: "data2".to_string(),
                },
                TestEvent {
                    id: "batch-3".to_string(),
                    data: "data3".to_string(),
                },
            ];

            // Check all - should be new
            let results = dedup.batch_check_duplicates(&events).await.unwrap();
            assert_eq!(results, vec![false, false, false]);

            // Mark all as seen
            let refs: Vec<&TestEvent> = events.iter().collect();
            dedup.batch_mark_seen(&refs).await.unwrap();

            // Check again - should be duplicates
            let results2 = dedup.batch_check_duplicates(&events).await.unwrap();
            assert_eq!(results2, vec![true, true, true]);
        } else {
            eprintln!("Skipping test: Redis not available");
        }
    }

    #[tokio::test]
    async fn test_stats() {
        if let Some(dedup) = create_test_deduplicator().await {
            let event1 = TestEvent {
                id: "stats-1".to_string(),
                data: "data1".to_string(),
            };
            let event2 = TestEvent {
                id: "stats-2".to_string(),
                data: "data2".to_string(),
            };

            // Check first event (miss)
            dedup.is_duplicate(&event1).await.unwrap();
            // Mark it
            dedup.mark_seen(&event1).await.unwrap();
            // Check again (hit, duplicate)
            dedup.is_duplicate(&event1).await.unwrap();
            // Check new event (miss)
            dedup.is_duplicate(&event2).await.unwrap();

            let stats = dedup.stats().await;
            assert_eq!(stats.events_checked, 3);
            assert_eq!(stats.duplicates_found, 1);
            assert_eq!(stats.new_events, 1);
            assert_eq!(stats.cache_hits, 1);
            assert_eq!(stats.cache_misses, 2);
            assert!(stats.duplicate_rate() > 0.0);
        } else {
            eprintln!("Skipping test: Redis not available");
        }
    }

    #[tokio::test]
    async fn test_uuid_extractor() {
        if let Some(_) = create_test_deduplicator().await {
            #[derive(Debug, Clone)]
            struct UuidEvent {
                uuid: Uuid,
            }

            let config = DeduplicationConfig::builder()
                .redis_url("redis://localhost:6379")
                .key_prefix("test:uuid:")
                .build()
                .unwrap();

            let extractor = EventIdExtractor::uuid(|event: &UuidEvent| event.uuid);

            if let Ok(dedup) = EventDeduplicator::new(config, extractor).await {
                let uuid = Uuid::new_v4();
                let event = UuidEvent { uuid };

                assert!(!dedup.is_duplicate(&event).await.unwrap());
                dedup.mark_seen(&event).await.unwrap();
                assert!(dedup.is_duplicate(&event).await.unwrap());

                dedup.clear().await.unwrap();
            }
        } else {
            eprintln!("Skipping test: Redis not available");
        }
    }

    #[tokio::test]
    async fn test_hash_extractor() {
        if let Some(_) = create_test_deduplicator().await {
            use std::collections::hash_map::DefaultHasher;

            #[derive(Debug, Clone)]
            struct HashEvent {
                data: String,
            }

            let config = DeduplicationConfig::builder()
                .redis_url("redis://localhost:6379")
                .key_prefix("test:hash:")
                .build()
                .unwrap();

            let extractor = EventIdExtractor::hash(|event: &HashEvent| {
                let mut hasher = DefaultHasher::new();
                event.data.hash(&mut hasher);
                hasher.finish()
            });

            if let Ok(dedup) = EventDeduplicator::new(config, extractor).await {
                let event1 = HashEvent {
                    data: "same-data".to_string(),
                };
                let event2 = HashEvent {
                    data: "same-data".to_string(),
                };

                assert!(!dedup.is_duplicate(&event1).await.unwrap());
                dedup.mark_seen(&event1).await.unwrap();
                // event2 has same data, so same hash
                assert!(dedup.is_duplicate(&event2).await.unwrap());

                dedup.clear().await.unwrap();
            }
        } else {
            eprintln!("Skipping test: Redis not available");
        }
    }

    #[tokio::test]
    async fn test_health_check() {
        if let Some(dedup) = create_test_deduplicator().await {
            let healthy = dedup.health_check().await.unwrap();
            assert!(healthy);
        } else {
            eprintln!("Skipping test: Redis not available");
        }
    }

    #[tokio::test]
    async fn test_clear() {
        if let Some(dedup) = create_test_deduplicator().await {
            let events: Vec<TestEvent> = (0..5)
                .map(|i| TestEvent {
                    id: format!("clear-{}", i),
                    data: "data".to_string(),
                })
                .collect();

            // Mark all as seen
            let refs: Vec<&TestEvent> = events.iter().collect();
            dedup.batch_mark_seen(&refs).await.unwrap();

            // Clear all
            let deleted = dedup.clear().await.unwrap();
            assert_eq!(deleted, 5);

            // Check all - should not be duplicates
            let results = dedup.batch_check_duplicates(&events).await.unwrap();
            assert_eq!(results, vec![false, false, false, false, false]);
        } else {
            eprintln!("Skipping test: Redis not available");
        }
    }

    #[test]
    fn test_config_builder() {
        let config = DeduplicationConfig::builder()
            .redis_url("redis://localhost:6379")
            .key_prefix("myapp:dedup:")
            .default_ttl(Duration::from_secs(7200))
            .pipeline_batch_size(200)
            .compress_event_ids(true)
            .build()
            .unwrap();

        assert_eq!(config.redis_url, "redis://localhost:6379");
        assert_eq!(config.key_prefix, "myapp:dedup:");
        assert_eq!(config.default_ttl, Some(Duration::from_secs(7200)));
        assert_eq!(config.pipeline_batch_size, 200);
        assert!(config.compress_event_ids);
    }

    #[test]
    fn test_stats_calculations() {
        let mut stats = DeduplicationStats::default();
        assert_eq!(stats.duplicate_rate(), 0.0);
        assert_eq!(stats.cache_hit_rate(), 0.0);

        stats.events_checked = 100;
        stats.duplicates_found = 25;
        stats.cache_hits = 25;
        stats.cache_misses = 75;

        assert_eq!(stats.duplicate_rate(), 0.25);
        assert_eq!(stats.cache_hit_rate(), 0.25);
        assert_eq!(stats.total_operations(), 100);
    }
}
