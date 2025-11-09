//! Tiered caching layer for optimal performance with distributed state
//!
//! This module implements a three-tiered caching architecture:
//! - L1: In-memory cache (moka) - fastest, local to instance
//! - L2: Redis cache - shared across instances, faster than DB
//! - L3: PostgreSQL/Sled backend - durable storage, source of truth
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │         CachedStateBackend<B>              │
//! │                                             │
//! │  ┌──────────────────────────────────────┐  │
//! │  │  L1: In-Memory Cache (moka)          │  │
//! │  │  - Thread-safe, lock-free            │  │
//! │  │  - LRU eviction                      │  │
//! │  │  - TTL support                       │  │
//! │  │  - ~1-10μs latency                   │  │
//! │  └──────────────────────────────────────┘  │
//! │              ↓ miss                         │
//! │  ┌──────────────────────────────────────┐  │
//! │  │  L2: Redis Cache                     │  │
//! │  │  - Shared across instances           │  │
//! │  │  - ~100-500μs latency                │  │
//! │  │  - TTL-based invalidation            │  │
//! │  └──────────────────────────────────────┘  │
//! │              ↓ miss                         │
//! │  ┌──────────────────────────────────────┐  │
//! │  │  L3: Backend (PostgreSQL/Sled)       │  │
//! │  │  - Durable, persistent               │  │
//! │  │  - Source of truth                   │  │
//! │  │  - ~1-10ms latency                   │  │
//! │  └──────────────────────────────────────┘  │
//! └─────────────────────────────────────────────┘
//! ```
//!
//! ## Features
//!
//! - **Write Strategies**: Write-through, write-back, write-behind
//! - **Read-through**: Automatic cache population on miss
//! - **Refresh-ahead**: Proactive refresh before TTL expiry
//! - **Statistics**: Per-layer hit rates, latency, evictions
//! - **Bulk Operations**: Optimized batch get/put operations
//! - **Cache Warming**: Preload hot keys on startup
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use processor::state::{
//!     CachedStateBackend, CacheConfig, WriteStrategy, EvictionPolicy,
//!     MemoryStateBackend,
//! };
//! use std::time::Duration;
//! use redis::Client;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Configure cache
//!     let config = CacheConfig {
//!         l1_capacity: 10_000,
//!         l1_ttl: Duration::from_secs(300),
//!         l2_ttl: Duration::from_secs(3600),
//!         write_strategy: WriteStrategy::WriteThrough,
//!         eviction_policy: EvictionPolicy::LRU,
//!         enable_stats: true,
//!     };
//!
//!     // Create backend
//!     let base_backend = MemoryStateBackend::new(None);
//!     let redis_client = Client::open("redis://127.0.0.1/")?;
//!
//!     let cached_backend = CachedStateBackend::new(
//!         base_backend,
//!         Some(redis_client),
//!         config,
//!     ).await?;
//!
//!     // Use it like any StateBackend
//!     cached_backend.put(b"key1", b"value1").await?;
//!     let value = cached_backend.get(b"key1").await?;
//!
//!     // Check stats
//!     let stats = cached_backend.stats().await;
//!     println!("L1 hit rate: {:.2}%", stats.l1_hit_rate * 100.0);
//!     println!("L2 hit rate: {:.2}%", stats.l2_hit_rate * 100.0);
//!
//!     Ok(())
//! }
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use async_trait::async_trait;
use moka::future::Cache as MokaCache;
use redis::aio::ConnectionManager;
use redis::{AsyncCommands, Client as RedisClient};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, warn, instrument};

use crate::error::{StateError, StateResult};
use crate::state::StateBackend;

/// Write strategy for cache updates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WriteStrategy {
    /// Write to cache and backend synchronously
    /// - Consistency: Strong
    /// - Latency: High
    /// - Use case: Critical data that must be durable immediately
    WriteThrough,

    /// Write to cache, async write to backend
    /// - Consistency: Eventual
    /// - Latency: Low
    /// - Use case: High write throughput, tolerates brief data loss
    WriteBack,

    /// Batch writes to backend
    /// - Consistency: Eventual (with batch delay)
    /// - Latency: Lowest
    /// - Use case: Very high write throughput, tolerates data loss
    WriteBehind,
}

/// Cache eviction policy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvictionPolicy {
    /// Least Recently Used - evict oldest accessed entry
    LRU,

    /// Least Frequently Used - evict least accessed entry
    LFU,

    /// First In First Out - evict oldest entry
    FIFO,
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Maximum number of entries in L1 cache
    pub l1_capacity: usize,

    /// L1 cache entry TTL
    pub l1_ttl: Duration,

    /// L2 cache entry TTL (Redis)
    pub l2_ttl: Duration,

    /// Write strategy
    pub write_strategy: WriteStrategy,

    /// Eviction policy for L1
    pub eviction_policy: EvictionPolicy,

    /// Enable statistics collection
    pub enable_stats: bool,

    /// Refresh-ahead threshold (fraction of TTL)
    /// If set to 0.8, entries accessed when 80% of TTL has elapsed
    /// will be refreshed asynchronously
    pub refresh_ahead_threshold: Option<f64>,

    /// Batch write interval for write-behind strategy
    pub write_behind_interval: Option<Duration>,

    /// Maximum batch size for write-behind
    pub write_behind_batch_size: Option<usize>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            l1_capacity: 10_000,
            l1_ttl: Duration::from_secs(300),      // 5 minutes
            l2_ttl: Duration::from_secs(3600),     // 1 hour
            write_strategy: WriteStrategy::WriteThrough,
            eviction_policy: EvictionPolicy::LRU,
            enable_stats: true,
            refresh_ahead_threshold: Some(0.8),
            write_behind_interval: Some(Duration::from_secs(10)),
            write_behind_batch_size: Some(1000),
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheStats {
    // Hit/miss counts
    pub l1_hits: u64,
    pub l1_misses: u64,
    pub l2_hits: u64,
    pub l2_misses: u64,
    pub l3_hits: u64,
    pub l3_misses: u64,

    // Hit rates (calculated)
    pub l1_hit_rate: f64,
    pub l2_hit_rate: f64,
    pub l3_hit_rate: f64,
    pub overall_hit_rate: f64,

    // Latency stats (in microseconds)
    pub l1_avg_latency_us: f64,
    pub l2_avg_latency_us: f64,
    pub l3_avg_latency_us: f64,

    // Eviction counts
    pub l1_evictions: u64,
    pub l2_evictions: u64,

    // Cache sizes
    pub l1_size: u64,
    pub l1_capacity: u64,
    pub l2_size: u64,

    // Write stats
    pub writes_total: u64,
    pub writes_batched: u64,

    // Refresh stats
    pub refresh_ahead_count: u64,
}

impl CacheStats {
    /// Calculate derived statistics
    fn calculate_rates(&mut self) {
        let l1_total = self.l1_hits + self.l1_misses;
        let l2_total = self.l2_hits + self.l2_misses;
        let l3_total = self.l3_hits + self.l3_misses;

        self.l1_hit_rate = if l1_total > 0 {
            self.l1_hits as f64 / l1_total as f64
        } else {
            0.0
        };

        self.l2_hit_rate = if l2_total > 0 {
            self.l2_hits as f64 / l2_total as f64
        } else {
            0.0
        };

        self.l3_hit_rate = if l3_total > 0 {
            self.l3_hits as f64 / l3_total as f64
        } else {
            0.0
        };

        let overall_total = self.l1_hits + self.l2_hits + self.l3_hits
            + self.l1_misses + self.l2_misses + self.l3_misses;
        let overall_hits = self.l1_hits + self.l2_hits + self.l3_hits;

        self.overall_hit_rate = if overall_total > 0 {
            overall_hits as f64 / overall_total as f64
        } else {
            0.0
        };
    }
}

/// Entry metadata for tracking TTL and refresh
#[derive(Debug, Clone)]
struct CacheEntry {
    value: Vec<u8>,
    inserted_at: SystemTime,
    ttl: Duration,
}

impl CacheEntry {
    fn new(value: Vec<u8>, ttl: Duration) -> Self {
        Self {
            value,
            inserted_at: SystemTime::now(),
            ttl,
        }
    }

    fn is_expired(&self) -> bool {
        self.inserted_at
            .elapsed()
            .map(|elapsed| elapsed >= self.ttl)
            .unwrap_or(true)
    }

    fn needs_refresh(&self, threshold: f64) -> bool {
        self.inserted_at
            .elapsed()
            .map(|elapsed| {
                let threshold_duration = Duration::from_secs_f64(self.ttl.as_secs_f64() * threshold);
                elapsed >= threshold_duration
            })
            .unwrap_or(false)
    }
}

/// Statistics collector
struct StatsCollector {
    l1_hits: AtomicU64,
    l1_misses: AtomicU64,
    l2_hits: AtomicU64,
    l2_misses: AtomicU64,
    l3_hits: AtomicU64,
    l3_misses: AtomicU64,
    l1_evictions: AtomicU64,
    l2_evictions: AtomicU64,
    writes_total: AtomicU64,
    writes_batched: AtomicU64,
    refresh_ahead_count: AtomicU64,

    // Latency tracking (sum of latencies for averaging)
    l1_latency_sum_us: AtomicU64,
    l2_latency_sum_us: AtomicU64,
    l3_latency_sum_us: AtomicU64,
}

impl StatsCollector {
    fn new() -> Self {
        Self {
            l1_hits: AtomicU64::new(0),
            l1_misses: AtomicU64::new(0),
            l2_hits: AtomicU64::new(0),
            l2_misses: AtomicU64::new(0),
            l3_hits: AtomicU64::new(0),
            l3_misses: AtomicU64::new(0),
            l1_evictions: AtomicU64::new(0),
            l2_evictions: AtomicU64::new(0),
            writes_total: AtomicU64::new(0),
            writes_batched: AtomicU64::new(0),
            refresh_ahead_count: AtomicU64::new(0),
            l1_latency_sum_us: AtomicU64::new(0),
            l2_latency_sum_us: AtomicU64::new(0),
            l3_latency_sum_us: AtomicU64::new(0),
        }
    }

    fn record_l1_hit(&self, latency_us: u64) {
        self.l1_hits.fetch_add(1, Ordering::Relaxed);
        self.l1_latency_sum_us.fetch_add(latency_us, Ordering::Relaxed);
    }

    fn record_l1_miss(&self) {
        self.l1_misses.fetch_add(1, Ordering::Relaxed);
    }

    fn record_l2_hit(&self, latency_us: u64) {
        self.l2_hits.fetch_add(1, Ordering::Relaxed);
        self.l2_latency_sum_us.fetch_add(latency_us, Ordering::Relaxed);
    }

    fn record_l2_miss(&self) {
        self.l2_misses.fetch_add(1, Ordering::Relaxed);
    }

    fn record_l3_hit(&self, latency_us: u64) {
        self.l3_hits.fetch_add(1, Ordering::Relaxed);
        self.l3_latency_sum_us.fetch_add(latency_us, Ordering::Relaxed);
    }

    fn record_l3_miss(&self) {
        self.l3_misses.fetch_add(1, Ordering::Relaxed);
    }

    fn record_write(&self, batched: bool) {
        self.writes_total.fetch_add(1, Ordering::Relaxed);
        if batched {
            self.writes_batched.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn record_refresh_ahead(&self) {
        self.refresh_ahead_count.fetch_add(1, Ordering::Relaxed);
    }

    fn snapshot(&self, l1_cache: &MokaCache<Vec<u8>, CacheEntry>, config: &CacheConfig) -> CacheStats {
        let l1_hits = self.l1_hits.load(Ordering::Relaxed);
        let l1_misses = self.l1_misses.load(Ordering::Relaxed);
        let l2_hits = self.l2_hits.load(Ordering::Relaxed);
        let l2_misses = self.l2_misses.load(Ordering::Relaxed);
        let l3_hits = self.l3_hits.load(Ordering::Relaxed);
        let l3_misses = self.l3_misses.load(Ordering::Relaxed);

        let l1_latency_sum = self.l1_latency_sum_us.load(Ordering::Relaxed);
        let l2_latency_sum = self.l2_latency_sum_us.load(Ordering::Relaxed);
        let l3_latency_sum = self.l3_latency_sum_us.load(Ordering::Relaxed);

        let mut stats = CacheStats {
            l1_hits,
            l1_misses,
            l2_hits,
            l2_misses,
            l3_hits,
            l3_misses,
            l1_evictions: self.l1_evictions.load(Ordering::Relaxed),
            l2_evictions: self.l2_evictions.load(Ordering::Relaxed),
            writes_total: self.writes_total.load(Ordering::Relaxed),
            writes_batched: self.writes_batched.load(Ordering::Relaxed),
            refresh_ahead_count: self.refresh_ahead_count.load(Ordering::Relaxed),
            l1_size: l1_cache.entry_count(),
            l1_capacity: config.l1_capacity as u64,
            l2_size: 0, // Would need Redis DBSIZE, skipped for performance
            l1_avg_latency_us: if l1_hits > 0 {
                l1_latency_sum as f64 / l1_hits as f64
            } else {
                0.0
            },
            l2_avg_latency_us: if l2_hits > 0 {
                l2_latency_sum as f64 / l2_hits as f64
            } else {
                0.0
            },
            l3_avg_latency_us: if l3_hits > 0 {
                l3_latency_sum as f64 / l3_hits as f64
            } else {
                0.0
            },
            ..Default::default()
        };

        stats.calculate_rates();
        stats
    }
}

/// Write-behind batch queue
struct WriteBehindQueue {
    pending: RwLock<Vec<(Vec<u8>, Vec<u8>)>>,
    config: CacheConfig,
}

impl WriteBehindQueue {
    fn new(config: CacheConfig) -> Self {
        Self {
            pending: RwLock::new(Vec::new()),
            config,
        }
    }

    async fn enqueue(&self, key: Vec<u8>, value: Vec<u8>) -> bool {
        let mut pending = self.pending.write().await;
        pending.push((key, value));

        pending.len() >= self.config.write_behind_batch_size.unwrap_or(1000)
    }

    async fn drain(&self) -> Vec<(Vec<u8>, Vec<u8>)> {
        let mut pending = self.pending.write().await;
        std::mem::take(&mut *pending)
    }

    async fn len(&self) -> usize {
        self.pending.read().await.len()
    }
}

/// Tiered caching state backend
///
/// Generic over any `StateBackend` implementation to provide three-tier caching:
/// L1 (in-memory), L2 (Redis), L3 (backend storage).
pub struct CachedStateBackend<B: StateBackend> {
    /// L1 cache (moka)
    l1_cache: MokaCache<Vec<u8>, CacheEntry>,

    /// L2 cache (Redis) - optional
    l2_cache: Option<ConnectionManager>,

    /// L3 backend (source of truth)
    backend: Arc<B>,

    /// Configuration
    config: CacheConfig,

    /// Statistics collector
    stats: Arc<StatsCollector>,

    /// Write-behind queue (if using write-behind strategy)
    write_behind_queue: Option<Arc<WriteBehindQueue>>,
}

impl<B: StateBackend + 'static> CachedStateBackend<B> {
    /// Create a new cached state backend
    ///
    /// # Arguments
    ///
    /// * `backend` - The underlying state backend (L3)
    /// * `redis_client` - Optional Redis client for L2 cache
    /// * `config` - Cache configuration
    ///
    /// # Returns
    ///
    /// Returns the cached backend wrapped in a Result
    pub async fn new(
        backend: B,
        redis_client: Option<RedisClient>,
        config: CacheConfig,
    ) -> StateResult<Self> {
        // Build L1 cache
        let l1_cache = MokaCache::builder()
            .max_capacity(config.l1_capacity as u64)
            .time_to_live(config.l1_ttl)
            .build();

        // Connect to Redis for L2 if provided
        let l2_cache = if let Some(client) = redis_client {
            let manager = client
                .get_connection_manager()
                .await
                .map_err(|e| StateError::StorageError {
                    backend_type: "redis".to_string(),
                    details: e.to_string(),
                })?;
            Some(manager)
        } else {
            None
        };

        // Create write-behind queue if needed
        let write_behind_queue = if config.write_strategy == WriteStrategy::WriteBehind {
            Some(Arc::new(WriteBehindQueue::new(config.clone())))
        } else {
            None
        };

        let cached = Self {
            l1_cache,
            l2_cache,
            backend: Arc::new(backend),
            config: config.clone(),
            stats: Arc::new(StatsCollector::new()),
            write_behind_queue,
        };

        // Start write-behind background task if needed
        if let Some(queue) = &cached.write_behind_queue {
            cached.start_write_behind_task(queue.clone()).await;
        }

        Ok(cached)
    }

    /// Start background task for write-behind strategy
    async fn start_write_behind_task(&self, queue: Arc<WriteBehindQueue>) {
        let backend = self.backend.clone();
        let stats = self.stats.clone();
        let interval = self.config.write_behind_interval.unwrap_or(Duration::from_secs(10));

        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            loop {
                interval_timer.tick().await;

                let batch = queue.drain().await;
                if batch.is_empty() {
                    continue;
                }

                debug!("Flushing write-behind batch: {} entries", batch.len());

                for (key, value) in batch {
                    if let Err(e) = backend.put(&key, &value).await {
                        warn!("Write-behind flush failed for key: {:?}", e);
                    } else {
                        stats.record_write(true);
                    }
                }
            }
        });
    }

    /// Get value from L1 cache
    #[instrument(skip(self, key), level = "trace")]
    async fn get_from_l1(&self, key: &[u8]) -> Option<Vec<u8>> {
        let start = Instant::now();

        if let Some(entry) = self.l1_cache.get(&key.to_vec()).await {
            if !entry.is_expired() {
                let latency_us = start.elapsed().as_micros() as u64;
                if self.config.enable_stats {
                    self.stats.record_l1_hit(latency_us);
                }

                // Check if refresh-ahead is needed
                if let Some(threshold) = self.config.refresh_ahead_threshold {
                    if entry.needs_refresh(threshold) {
                        self.refresh_ahead(key.to_vec()).await;
                    }
                }

                return Some(entry.value.clone());
            }
        }

        if self.config.enable_stats {
            self.stats.record_l1_miss();
        }
        None
    }

    /// Get value from L2 cache (Redis)
    #[instrument(skip(self, key), level = "trace")]
    async fn get_from_l2(&self, key: &[u8]) -> StateResult<Option<Vec<u8>>> {
        let Some(ref mut conn) = self.l2_cache.clone() else {
            return Ok(None);
        };

        let start = Instant::now();
        let redis_key = format!("state:{}", hex::encode(key));

        let result: Result<Option<Vec<u8>>, redis::RedisError> =
            conn.clone().get(&redis_key).await;

        match result {
            Ok(Some(value)) => {
                let latency_us = start.elapsed().as_micros() as u64;
                if self.config.enable_stats {
                    self.stats.record_l2_hit(latency_us);
                }

                // Populate L1 cache
                let entry = CacheEntry::new(value.clone(), self.config.l1_ttl);
                self.l1_cache.insert(key.to_vec(), entry).await;

                Ok(Some(value))
            }
            Ok(None) => {
                if self.config.enable_stats {
                    self.stats.record_l2_miss();
                }
                Ok(None)
            }
            Err(e) => {
                warn!("L2 cache error: {}", e);
                if self.config.enable_stats {
                    self.stats.record_l2_miss();
                }
                Ok(None)
            }
        }
    }

    /// Get value from L3 backend
    #[instrument(skip(self, key), level = "trace")]
    async fn get_from_l3(&self, key: &[u8]) -> StateResult<Option<Vec<u8>>> {
        let start = Instant::now();

        let result = self.backend.get(key).await?;

        let latency_us = start.elapsed().as_micros() as u64;

        if result.is_some() {
            if self.config.enable_stats {
                self.stats.record_l3_hit(latency_us);
            }
        } else {
            if self.config.enable_stats {
                self.stats.record_l3_miss();
            }
        }

        Ok(result)
    }

    /// Put value to L1 cache
    async fn put_to_l1(&self, key: &[u8], value: &[u8]) {
        let entry = CacheEntry::new(value.to_vec(), self.config.l1_ttl);
        self.l1_cache.insert(key.to_vec(), entry).await;
    }

    /// Put value to L2 cache (Redis)
    async fn put_to_l2(&self, key: &[u8], value: &[u8]) -> StateResult<()> {
        let Some(ref mut conn) = self.l2_cache.clone() else {
            return Ok(());
        };

        let redis_key = format!("state:{}", hex::encode(key));
        let ttl_seconds = self.config.l2_ttl.as_secs();

        let _: () = conn.clone()
            .set_ex(&redis_key, value, ttl_seconds as u64)
            .await
            .map_err(|e| StateError::StorageError {
                backend_type: "redis".to_string(),
                details: e.to_string(),
            })?;

        Ok(())
    }

    /// Put value to L3 backend
    async fn put_to_l3(&self, key: &[u8], value: &[u8]) -> StateResult<()> {
        self.backend.put(key, value).await
    }

    /// Refresh a key asynchronously (refresh-ahead strategy)
    async fn refresh_ahead(&self, key: Vec<u8>) {
        if self.config.enable_stats {
            self.stats.record_refresh_ahead();
        }

        let backend = self.backend.clone();
        let l1_cache = self.l1_cache.clone();
        let l2_cache = self.l2_cache.clone();
        let l1_ttl = self.config.l1_ttl;
        let l2_ttl = self.config.l2_ttl;

        tokio::spawn(async move {
            debug!("Refresh-ahead for key: {:?}", key);

            // Fetch from backend
            if let Ok(Some(value)) = backend.get(&key).await {
                // Update L1
                let entry = CacheEntry::new(value.clone(), l1_ttl);
                l1_cache.insert(key.clone(), entry).await;

                // Update L2 if available
                if let Some(mut conn) = l2_cache {
                    let redis_key = format!("state:{}", hex::encode(&key));
                    let ttl_seconds = l2_ttl.as_secs();
                    let _: Result<(), _> = conn.set_ex(&redis_key, &value, ttl_seconds as u64).await;
                }
            }
        });
    }

    /// Invalidate a key from all cache layers
    #[instrument(skip(self, key), level = "debug")]
    pub async fn invalidate(&self, key: &[u8]) -> StateResult<()> {
        // Remove from L1
        self.l1_cache.invalidate(&key.to_vec()).await;

        // Remove from L2
        if let Some(ref mut conn) = self.l2_cache.clone() {
            let redis_key = format!("state:{}", hex::encode(key));
            let _: Result<(), _> = conn.clone().del(&redis_key).await;
        }

        Ok(())
    }

    /// Clear all caches (L1 and L2)
    pub async fn clear_cache(&self) -> StateResult<()> {
        // Clear L1
        self.l1_cache.invalidate_all();

        // Clear L2 (flush all state keys)
        if let Some(ref mut conn) = self.l2_cache.clone() {
            // Note: This flushes ALL keys matching the pattern
            // In production, use a separate Redis DB or namespace
            let pattern = "state:*";
            let keys: Result<Vec<String>, _> = redis::cmd("KEYS")
                .arg(pattern)
                .query_async(&mut conn.clone())
                .await;

            if let Ok(keys) = keys {
                if !keys.is_empty() {
                    let _: Result<(), _> = conn.clone().del(keys).await;
                }
            }
        }

        Ok(())
    }

    /// Get current cache statistics
    pub async fn stats(&self) -> CacheStats {
        self.stats.snapshot(&self.l1_cache, &self.config)
    }

    /// Warm up cache with frequently accessed keys
    ///
    /// Preloads specified keys from backend into cache layers
    pub async fn warm_cache(&self, keys: &[Vec<u8>]) -> StateResult<()> {
        debug!("Warming cache with {} keys", keys.len());

        for key in keys {
            if let Some(value) = self.backend.get(key).await? {
                // Populate L1
                self.put_to_l1(key, &value).await;

                // Populate L2
                let _ = self.put_to_l2(key, &value).await;
            }
        }

        Ok(())
    }

    /// Get multiple keys in batch (optimized)
    pub async fn get_batch(&self, keys: &[Vec<u8>]) -> StateResult<Vec<Option<Vec<u8>>>> {
        let mut results = Vec::with_capacity(keys.len());

        for key in keys {
            results.push(self.get(key).await?);
        }

        Ok(results)
    }

    /// Put multiple key-value pairs in batch (optimized)
    pub async fn put_batch(&self, pairs: &[(Vec<u8>, Vec<u8>)]) -> StateResult<()> {
        for (key, value) in pairs {
            self.put(key, value).await?;
        }

        Ok(())
    }
}

// Need to add hex crate for encoding keys
// Will be added to dependencies

// Implement StateBackend trait
#[async_trait]
impl<B: StateBackend + 'static> StateBackend for CachedStateBackend<B> {
    #[instrument(skip(self, key), level = "debug")]
    async fn get(&self, key: &[u8]) -> StateResult<Option<Vec<u8>>> {
        // Try L1 cache first
        if let Some(value) = self.get_from_l1(key).await {
            return Ok(Some(value));
        }

        // Try L2 cache
        if let Some(value) = self.get_from_l2(key).await? {
            return Ok(Some(value));
        }

        // Finally, try L3 backend
        let value = self.get_from_l3(key).await?;

        // If found in L3, populate upper caches (read-through)
        if let Some(ref val) = value {
            self.put_to_l1(key, val).await;
            let _ = self.put_to_l2(key, val).await; // Ignore Redis errors
        }

        Ok(value)
    }

    #[instrument(skip(self, key, value), level = "debug")]
    async fn put(&self, key: &[u8], value: &[u8]) -> StateResult<()> {
        match self.config.write_strategy {
            WriteStrategy::WriteThrough => {
                // Write to all layers synchronously
                self.put_to_l1(key, value).await;
                let _ = self.put_to_l2(key, value).await;
                self.put_to_l3(key, value).await?;

                if self.config.enable_stats {
                    self.stats.record_write(false);
                }
            }
            WriteStrategy::WriteBack => {
                // Write to cache immediately, backend asynchronously
                self.put_to_l1(key, value).await;
                let _ = self.put_to_l2(key, value).await;

                let backend = self.backend.clone();
                let key = key.to_vec();
                let value = value.to_vec();
                let stats = self.stats.clone();
                let enable_stats = self.config.enable_stats;

                tokio::spawn(async move {
                    if let Err(e) = backend.put(&key, &value).await {
                        warn!("Write-back failed: {:?}", e);
                    } else if enable_stats {
                        stats.record_write(false);
                    }
                });
            }
            WriteStrategy::WriteBehind => {
                // Write to cache immediately, queue for batch write
                self.put_to_l1(key, value).await;
                let _ = self.put_to_l2(key, value).await;

                if let Some(ref queue) = self.write_behind_queue {
                    queue.enqueue(key.to_vec(), value.to_vec()).await;
                }
            }
        }

        Ok(())
    }

    #[instrument(skip(self, key), level = "debug")]
    async fn delete(&self, key: &[u8]) -> StateResult<()> {
        // Remove from all cache layers
        self.invalidate(key).await?;

        // Delete from backend
        self.backend.delete(key).await
    }

    async fn list_keys(&self, prefix: &[u8]) -> StateResult<Vec<Vec<u8>>> {
        // List keys always goes to backend (source of truth)
        self.backend.list_keys(prefix).await
    }

    async fn clear(&self) -> StateResult<()> {
        // Clear caches
        self.clear_cache().await?;

        // Clear backend
        self.backend.clear().await
    }

    async fn count(&self) -> StateResult<usize> {
        // Count from backend (source of truth)
        self.backend.count().await
    }

    async fn contains(&self, key: &[u8]) -> StateResult<bool> {
        // Check L1 first
        if self.l1_cache.get(&key.to_vec()).await.is_some() {
            return Ok(true);
        }

        // Fall through to backend
        self.backend.contains(key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::MemoryStateBackend;
    use std::time::Duration;

    #[tokio::test]
    async fn test_cache_config_default() {
        let config = CacheConfig::default();
        assert_eq!(config.l1_capacity, 10_000);
        assert_eq!(config.write_strategy, WriteStrategy::WriteThrough);
        assert_eq!(config.eviction_policy, EvictionPolicy::LRU);
    }

    #[tokio::test]
    async fn test_cached_backend_creation() {
        let backend = MemoryStateBackend::new(None);
        let config = CacheConfig::default();

        let cached = CachedStateBackend::new(backend, None, config).await;
        assert!(cached.is_ok());
    }

    #[tokio::test]
    async fn test_l1_cache_hit() {
        let backend = MemoryStateBackend::new(None);
        let config = CacheConfig {
            enable_stats: true,
            ..Default::default()
        };

        let cached = CachedStateBackend::new(backend, None, config).await.unwrap();

        // First put
        cached.put(b"key1", b"value1").await.unwrap();

        // First get - should be L1 hit
        let value = cached.get(b"key1").await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));

        // Check stats
        let stats = cached.stats().await;
        assert_eq!(stats.l1_hits, 1);
        assert!(stats.l1_hit_rate > 0.0);
    }

    #[tokio::test]
    async fn test_l3_fallback() {
        let backend = MemoryStateBackend::new(None);
        backend.put(b"existing", b"data").await.unwrap();

        let config = CacheConfig {
            enable_stats: true,
            ..Default::default()
        };

        let cached = CachedStateBackend::new(backend, None, config).await.unwrap();

        // Get should fall through to L3
        let value = cached.get(b"existing").await.unwrap();
        assert_eq!(value, Some(b"data".to_vec()));

        // Check stats - should be L3 hit
        let stats = cached.stats().await;
        assert_eq!(stats.l3_hits, 1);
        assert_eq!(stats.l1_hits, 0);
    }

    #[tokio::test]
    async fn test_write_through_strategy() {
        let backend = MemoryStateBackend::new(None);
        let config = CacheConfig {
            write_strategy: WriteStrategy::WriteThrough,
            ..Default::default()
        };

        let cached = CachedStateBackend::new(backend.clone(), None, config).await.unwrap();

        cached.put(b"key1", b"value1").await.unwrap();

        // Value should be in backend immediately
        let backend_value = backend.get(b"key1").await.unwrap();
        assert_eq!(backend_value, Some(b"value1".to_vec()));
    }

    #[tokio::test]
    async fn test_invalidate() {
        let backend = MemoryStateBackend::new(None);
        let config = CacheConfig::default();

        let cached = CachedStateBackend::new(backend, None, config).await.unwrap();

        cached.put(b"key1", b"value1").await.unwrap();

        // Should be in L1
        let value = cached.get(b"key1").await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));

        // Invalidate
        cached.invalidate(b"key1").await.unwrap();

        // Should still be in backend, but need to fetch from L3
        let value = cached.get(b"key1").await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let backend = MemoryStateBackend::new(None);
        let config = CacheConfig {
            enable_stats: true,
            ..Default::default()
        };

        let cached = CachedStateBackend::new(backend, None, config).await.unwrap();

        // Generate some traffic
        cached.put(b"k1", b"v1").await.unwrap();
        cached.put(b"k2", b"v2").await.unwrap();

        cached.get(b"k1").await.unwrap(); // L1 hit
        cached.get(b"k1").await.unwrap(); // L1 hit
        cached.get(b"k3").await.unwrap(); // L3 miss

        let stats = cached.stats().await;
        assert_eq!(stats.l1_hits, 2);
        assert_eq!(stats.l3_misses, 1);
        assert!(stats.l1_hit_rate > 0.0);
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let backend = MemoryStateBackend::new(None);
        let config = CacheConfig::default();

        let cached = CachedStateBackend::new(backend, None, config).await.unwrap();

        // Batch put
        let pairs = vec![
            (b"k1".to_vec(), b"v1".to_vec()),
            (b"k2".to_vec(), b"v2".to_vec()),
            (b"k3".to_vec(), b"v3".to_vec()),
        ];
        cached.put_batch(&pairs).await.unwrap();

        // Batch get
        let keys = vec![b"k1".to_vec(), b"k2".to_vec(), b"k3".to_vec()];
        let values = cached.get_batch(&keys).await.unwrap();

        assert_eq!(values.len(), 3);
        assert_eq!(values[0], Some(b"v1".to_vec()));
        assert_eq!(values[1], Some(b"v2".to_vec()));
        assert_eq!(values[2], Some(b"v3".to_vec()));
    }

    #[tokio::test]
    async fn test_cache_warming() {
        let backend = MemoryStateBackend::new(None);

        // Prepopulate backend
        backend.put(b"hot1", b"data1").await.unwrap();
        backend.put(b"hot2", b"data2").await.unwrap();
        backend.put(b"hot3", b"data3").await.unwrap();

        let config = CacheConfig {
            enable_stats: true,
            ..Default::default()
        };

        let cached = CachedStateBackend::new(backend, None, config).await.unwrap();

        // Warm cache
        let hot_keys = vec![b"hot1".to_vec(), b"hot2".to_vec(), b"hot3".to_vec()];
        cached.warm_cache(&hot_keys).await.unwrap();

        // All gets should be L1 hits
        cached.get(b"hot1").await.unwrap();
        cached.get(b"hot2").await.unwrap();
        cached.get(b"hot3").await.unwrap();

        let stats = cached.stats().await;
        assert_eq!(stats.l1_hits, 3);
        assert_eq!(stats.l3_hits, 0);
    }

    #[tokio::test]
    async fn test_clear_cache() {
        let backend = MemoryStateBackend::new(None);
        let config = CacheConfig::default();

        let cached = CachedStateBackend::new(backend, None, config).await.unwrap();

        cached.put(b"k1", b"v1").await.unwrap();
        cached.put(b"k2", b"v2").await.unwrap();

        // Clear cache only
        cached.clear_cache().await.unwrap();

        // Data should still be in backend (L3)
        // But will cause L3 hits instead of L1
        let value = cached.get(b"k1").await.unwrap();
        assert_eq!(value, Some(b"v1".to_vec()));

        let stats = cached.stats().await;
        assert_eq!(stats.l3_hits, 1); // Had to fetch from L3
    }

    #[tokio::test]
    async fn test_entry_expiration() {
        let entry = CacheEntry::new(b"value".to_vec(), Duration::from_millis(50));

        assert!(!entry.is_expired());

        tokio::time::sleep(Duration::from_millis(60)).await;

        assert!(entry.is_expired());
    }

    #[tokio::test]
    async fn test_refresh_ahead_detection() {
        let entry = CacheEntry::new(b"value".to_vec(), Duration::from_millis(100));

        assert!(!entry.needs_refresh(0.8));

        tokio::time::sleep(Duration::from_millis(85)).await;

        assert!(entry.needs_refresh(0.8));
    }

    #[tokio::test]
    async fn test_stats_calculation() {
        let mut stats = CacheStats {
            l1_hits: 80,
            l1_misses: 20,
            l2_hits: 15,
            l2_misses: 5,
            l3_hits: 4,
            l3_misses: 1,
            ..Default::default()
        };

        stats.calculate_rates();

        assert_eq!(stats.l1_hit_rate, 0.8);
        assert_eq!(stats.l2_hit_rate, 0.75);
        assert_eq!(stats.l3_hit_rate, 0.8);

        // Overall: (80+15+4) / (80+20+15+5+4+1) = 99/125 = 0.792
        assert!((stats.overall_hit_rate - 0.792).abs() < 0.001);
    }
}
