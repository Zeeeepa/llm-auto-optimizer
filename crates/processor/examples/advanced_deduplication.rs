//! Advanced Event Deduplication Demo
//!
//! This example demonstrates advanced deduplication patterns including:
//! - Multi-key deduplication strategies
//! - Distributed deduplication across multiple services
//! - High-throughput scenarios with bloom filters
//! - TTL management and cleanup strategies
//! - Monitoring and alerting integration
//! - Performance optimization techniques
//!
//! Run with:
//! ```bash
//! # Full demo with Redis (requires Redis)
//! cargo run --example advanced_deduplication
//!
//! # Specific scenarios
//! cargo run --example advanced_deduplication -- --scenario bloom-filter
//! cargo run --example advanced_deduplication -- --scenario multi-key
//! cargo run --example advanced_deduplication -- --scenario distributed
//! ```

use anyhow::Result;
use chrono::{DateTime, Utc};
use processor::state::{
    MemoryStateBackend, RedisConfig, RedisStateBackend, SledStateBackend, StateBackend,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration as TokioDuration};
use tracing::{debug, error, info, warn};
use tracing_subscriber;

/// Advanced event with multiple deduplication strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AdvancedEvent {
    /// Primary event ID
    event_id: String,
    /// User/customer ID
    user_id: String,
    /// Transaction ID (for financial events)
    transaction_id: Option<String>,
    /// Idempotency key (for API requests)
    idempotency_key: Option<String>,
    /// Event type
    event_type: String,
    /// Timestamp
    timestamp: DateTime<Utc>,
    /// Payload hash for content-based deduplication
    content_hash: String,
    /// Custom metadata
    metadata: HashMap<String, String>,
}

impl AdvancedEvent {
    fn new(event_id: String, user_id: String, event_type: String) -> Self {
        let data = format!("{}-{}-{}", event_id, user_id, event_type);
        let content_hash = format!("{:x}", md5::compute(data.as_bytes()));

        Self {
            event_id,
            user_id,
            transaction_id: None,
            idempotency_key: None,
            event_type,
            timestamp: Utc::now(),
            content_hash,
            metadata: HashMap::new(),
        }
    }

    fn with_transaction(mut self, tx_id: String) -> Self {
        self.transaction_id = Some(tx_id);
        self
    }

    fn with_idempotency_key(mut self, key: String) -> Self {
        self.idempotency_key = Some(key);
        self
    }
}

/// Multi-strategy deduplication filter
struct MultiStrategyDedup<B: StateBackend> {
    backend: Arc<B>,
    strategies: Vec<DedupStrategy>,
    stats: Arc<RwLock<MultiStrategyStats>>,
}

#[derive(Debug, Clone)]
enum DedupStrategy {
    /// Deduplicate by event ID
    ByEventId { ttl: std::time::Duration },
    /// Deduplicate by user + event type + time window
    ByUserAndType { window: std::time::Duration },
    /// Deduplicate by transaction ID (financial events)
    ByTransaction { ttl: std::time::Duration },
    /// Deduplicate by idempotency key (API requests)
    ByIdempotencyKey { ttl: std::time::Duration },
    /// Deduplicate by content hash
    ByContentHash { ttl: std::time::Duration },
}

#[derive(Debug, Default)]
struct MultiStrategyStats {
    checks_by_strategy: HashMap<String, u64>,
    duplicates_by_strategy: HashMap<String, u64>,
    total_checks: u64,
    total_duplicates: u64,
}

impl<B: StateBackend> MultiStrategyDedup<B> {
    fn new(backend: Arc<B>, strategies: Vec<DedupStrategy>) -> Self {
        Self {
            backend,
            strategies,
            stats: Arc::new(RwLock::new(MultiStrategyStats::default())),
        }
    }

    async fn is_duplicate(&self, event: &AdvancedEvent) -> Result<bool> {
        let mut stats = self.stats.write().await;
        stats.total_checks += 1;

        // Check each strategy in order
        for strategy in &self.strategies {
            let (key, strategy_name) = self.generate_key(event, strategy);

            *stats.checks_by_strategy.entry(strategy_name.clone()).or_insert(0) += 1;

            // Check if key exists
            if self.backend.get(key.as_bytes()).await?.is_some() {
                *stats.duplicates_by_strategy.entry(strategy_name).or_insert(0) += 1;
                stats.total_duplicates += 1;
                return Ok(true);
            }

            // Store key with appropriate TTL
            let ttl = self.get_ttl(strategy);
            self.backend.put_with_ttl(key.as_bytes(), b"1", ttl).await?;
        }

        Ok(false)
    }

    fn generate_key(&self, event: &AdvancedEvent, strategy: &DedupStrategy) -> (String, String) {
        match strategy {
            DedupStrategy::ByEventId { .. } => {
                (format!("dedup:event_id:{}", event.event_id), "event_id".to_string())
            }
            DedupStrategy::ByUserAndType { window } => {
                let window_secs = window.as_secs();
                let window_key = event.timestamp.timestamp() / window_secs as i64;
                (
                    format!("dedup:user_type:{}:{}:{}", event.user_id, event.event_type, window_key),
                    "user_type".to_string(),
                )
            }
            DedupStrategy::ByTransaction { .. } => {
                let tx_id = event.transaction_id.as_ref().unwrap_or(&"none".to_string());
                (format!("dedup:tx:{}", tx_id), "transaction".to_string())
            }
            DedupStrategy::ByIdempotencyKey { .. } => {
                let key = event.idempotency_key.as_ref().unwrap_or(&"none".to_string());
                (format!("dedup:idem:{}", key), "idempotency".to_string())
            }
            DedupStrategy::ByContentHash { .. } => {
                (format!("dedup:hash:{}", event.content_hash), "content_hash".to_string())
            }
        }
    }

    fn get_ttl(&self, strategy: &DedupStrategy) -> std::time::Duration {
        match strategy {
            DedupStrategy::ByEventId { ttl }
            | DedupStrategy::ByTransaction { ttl }
            | DedupStrategy::ByIdempotencyKey { ttl }
            | DedupStrategy::ByContentHash { ttl } => *ttl,
            DedupStrategy::ByUserAndType { window } => *window * 2,
        }
    }

    async fn stats(&self) -> MultiStrategyStats {
        self.stats.read().await.clone()
    }
}

/// Bloom filter simulation for high-throughput scenarios
struct BloomFilterDedup {
    /// Simulated bloom filter (in production, use a real bloom filter crate)
    seen_hashes: Arc<RwLock<HashMap<u64, bool>>>,
    backend: Arc<dyn StateBackend>,
    capacity: usize,
    false_positive_rate: f64,
    stats: Arc<RwLock<BloomStats>>,
}

#[derive(Debug, Default)]
struct BloomStats {
    bloom_checks: u64,
    bloom_hits: u64,
    bloom_misses: u64,
    backend_checks: u64,
    true_duplicates: u64,
    false_positives: u64,
}

impl BloomFilterDedup {
    fn new(
        backend: Arc<dyn StateBackend>,
        capacity: usize,
        false_positive_rate: f64,
    ) -> Self {
        Self {
            seen_hashes: Arc::new(RwLock::new(HashMap::new())),
            backend,
            capacity,
            false_positive_rate,
            stats: Arc::new(RwLock::new(BloomStats::default())),
        }
    }

    async fn is_duplicate(&self, event_id: &str) -> Result<bool> {
        let mut stats = self.stats.write().await;
        stats.bloom_checks += 1;

        // Hash the event ID
        let hash = self.hash(event_id);

        // Check bloom filter first (fast path)
        {
            let bloom = self.seen_hashes.read().await;
            if !bloom.contains_key(&hash) {
                // Definitely not seen before
                stats.bloom_misses += 1;
                drop(bloom);

                // Add to bloom filter and backend
                let mut bloom = self.seen_hashes.write().await;
                bloom.insert(hash, true);

                // Manage bloom filter size
                if bloom.len() > self.capacity {
                    bloom.clear();
                    info!("Bloom filter cleared (reached capacity)");
                }

                let key = format!("bloom:{}", event_id);
                self.backend
                    .put_with_ttl(key.as_bytes(), b"1", std::time::Duration::from_secs(3600))
                    .await?;

                return Ok(false);
            }
        }

        // Bloom filter indicates possible duplicate - check backend for confirmation
        stats.bloom_hits += 1;
        stats.backend_checks += 1;

        let key = format!("bloom:{}", event_id);
        if self.backend.get(key.as_bytes()).await?.is_some() {
            stats.true_duplicates += 1;
            Ok(true)
        } else {
            stats.false_positives += 1;
            Ok(false)
        }
    }

    fn hash(&self, s: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish()
    }

    async fn stats(&self) -> BloomStats {
        self.stats.read().await.clone()
    }

    async fn efficiency(&self) -> f64 {
        let stats = self.stats.read().await;
        if stats.bloom_checks == 0 {
            return 0.0;
        }
        stats.bloom_hits as f64 / stats.bloom_checks as f64
    }
}

/// Distributed deduplication coordinator
struct DistributedDedup {
    instance_id: String,
    redis_backend: Arc<RedisStateBackend>,
    local_cache: Arc<MemoryStateBackend>,
    processed_count: AtomicU64,
    duplicate_count: AtomicU64,
}

impl DistributedDedup {
    async fn new(instance_id: String, redis_url: String) -> Result<Self> {
        let redis_config = RedisConfig::builder()
            .url(redis_url)
            .key_prefix(format!("distributed:{}:", instance_id))
            .default_ttl(std::time::Duration::from_secs(3600))
            .build()?;

        let redis_backend = Arc::new(RedisStateBackend::new(redis_config).await?);
        let local_cache = Arc::new(MemoryStateBackend::new(Some(std::time::Duration::from_secs(300))));

        Ok(Self {
            instance_id,
            redis_backend,
            local_cache,
            processed_count: AtomicU64::new(0),
            duplicate_count: AtomicU64::new(0),
        })
    }

    async fn process_event(&self, event_id: &str) -> Result<bool> {
        // Check local cache first (fast path)
        let cache_key = format!("cache:{}", event_id);
        if self.local_cache.get(cache_key.as_bytes()).await?.is_some() {
            self.duplicate_count.fetch_add(1, Ordering::Relaxed);
            return Ok(false);
        }

        // Check Redis (authoritative)
        let redis_key = format!("global:{}", event_id);
        if self.redis_backend.get(redis_key.as_bytes()).await?.is_some() {
            // Update local cache
            self.local_cache
                .put_with_ttl(cache_key.as_bytes(), b"1", std::time::Duration::from_secs(300))
                .await?;

            self.duplicate_count.fetch_add(1, Ordering::Relaxed);
            return Ok(false);
        }

        // New event - store in both Redis and local cache
        self.redis_backend
            .put_with_ttl(redis_key.as_bytes(), b"1", std::time::Duration::from_secs(3600))
            .await?;

        self.local_cache
            .put_with_ttl(cache_key.as_bytes(), b"1", std::time::Duration::from_secs(300))
            .await?;

        self.processed_count.fetch_add(1, Ordering::Relaxed);
        Ok(true)
    }

    fn stats(&self) -> (u64, u64) {
        (
            self.processed_count.load(Ordering::Relaxed),
            self.duplicate_count.load(Ordering::Relaxed),
        )
    }
}

/// Scenario 1: Multi-key deduplication
async fn scenario_multi_key_deduplication() -> Result<()> {
    info!("=== Scenario 1: Multi-Key Deduplication ===");

    let backend = Arc::new(MemoryStateBackend::new(Some(std::time::Duration::from_secs(3600))));

    let strategies = vec![
        DedupStrategy::ByEventId {
            ttl: std::time::Duration::from_secs(3600),
        },
        DedupStrategy::ByTransaction {
            ttl: std::time::Duration::from_secs(7200),
        },
        DedupStrategy::ByIdempotencyKey {
            ttl: std::time::Duration::from_secs(1800),
        },
    ];

    let dedup = MultiStrategyDedup::new(backend, strategies);

    // Create events with different deduplication needs
    let events = vec![
        AdvancedEvent::new("evt_001".to_string(), "user_001".to_string(), "purchase".to_string())
            .with_transaction("tx_12345".to_string())
            .with_idempotency_key("idem_abc".to_string()),
        // Duplicate by event ID
        AdvancedEvent::new("evt_001".to_string(), "user_001".to_string(), "purchase".to_string())
            .with_transaction("tx_67890".to_string()),
        // Duplicate by transaction ID
        AdvancedEvent::new("evt_002".to_string(), "user_001".to_string(), "purchase".to_string())
            .with_transaction("tx_12345".to_string()),
        // Unique event
        AdvancedEvent::new("evt_003".to_string(), "user_002".to_string(), "purchase".to_string())
            .with_transaction("tx_99999".to_string()),
    ];

    for (i, event) in events.iter().enumerate() {
        let is_dup = dedup.is_duplicate(event).await?;
        info!(
            "Event {}: {} (event_id={}, tx={:?})",
            i + 1,
            if is_dup { "DUPLICATE" } else { "UNIQUE" },
            event.event_id,
            event.transaction_id
        );
    }

    let stats = dedup.stats().await;
    info!("\nMulti-Strategy Stats:");
    info!("  Total checks: {}", stats.total_checks);
    info!("  Total duplicates: {}", stats.total_duplicates);
    for (strategy, count) in stats.duplicates_by_strategy {
        info!("  {} duplicates: {}", strategy, count);
    }

    Ok(())
}

/// Scenario 2: High-throughput with bloom filter
async fn scenario_bloom_filter_high_throughput() -> Result<()> {
    info!("=== Scenario 2: Bloom Filter High-Throughput ===");

    let backend: Arc<dyn StateBackend> = Arc::new(MemoryStateBackend::new(Some(
        std::time::Duration::from_secs(3600),
    )));

    let bloom_dedup = BloomFilterDedup::new(backend, 100_000, 0.01);

    // Generate 10,000 events with 30% duplicate rate
    let mut event_ids = Vec::new();
    for i in 0..10_000 {
        let id = if i % 3 == 0 && i > 0 {
            format!("evt_{:05}", i - 1) // 30% duplicates
        } else {
            format!("evt_{:05}", i)
        };
        event_ids.push(id);
    }

    info!("Processing {} events with bloom filter...", event_ids.len());
    let start = std::time::Instant::now();

    let mut unique_count = 0;
    for event_id in &event_ids {
        if !bloom_dedup.is_duplicate(event_id).await? {
            unique_count += 1;
        }
    }

    let duration = start.elapsed();
    let throughput = event_ids.len() as f64 / duration.as_secs_f64();

    let stats = bloom_dedup.stats().await;
    info!("\nBloom Filter Performance:");
    info!("  Processed: {} unique / {} total", unique_count, event_ids.len());
    info!("  Throughput: {:.0} events/sec", throughput);
    info!("  Latency: {:.2}μs per event", duration.as_micros() as f64 / event_ids.len() as f64);
    info!("  Bloom hits: {} ({:.1}%)", stats.bloom_hits, stats.bloom_hits as f64 / stats.bloom_checks as f64 * 100.0);
    info!("  Backend checks: {} ({:.1}%)", stats.backend_checks, stats.backend_checks as f64 / stats.bloom_checks as f64 * 100.0);
    info!("  False positives: {}", stats.false_positives);
    info!("  Bloom efficiency: {:.1}%", bloom_dedup.efficiency().await * 100.0);

    Ok(())
}

/// Scenario 3: Distributed deduplication simulation
async fn scenario_distributed_deduplication() -> Result<()> {
    info!("=== Scenario 3: Distributed Deduplication ===");

    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

    // Create multiple instances
    let instance1 = match DistributedDedup::new("instance_1".to_string(), redis_url.clone()).await {
        Ok(i) => Arc::new(i),
        Err(e) => {
            warn!("Redis not available: {}. Skipping distributed demo.", e);
            return Ok(());
        }
    };

    let instance2 = Arc::new(
        DistributedDedup::new("instance_2".to_string(), redis_url.clone()).await?,
    );

    let instance3 = Arc::new(
        DistributedDedup::new("instance_3".to_string(), redis_url.clone()).await?,
    );

    // Generate shared event stream
    let event_ids: Vec<String> = (0..1000).map(|i| format!("shared_evt_{:04}", i)).collect();

    info!("Simulating 3 instances processing {} events concurrently...", event_ids.len());

    // Process events concurrently across instances
    let start = std::time::Instant::now();

    let mut handles = vec![];

    // Instance 1
    let inst1 = instance1.clone();
    let events1 = event_ids.clone();
    handles.push(tokio::spawn(async move {
        for event_id in events1 {
            let _ = inst1.process_event(&event_id).await;
        }
    }));

    // Instance 2 (slight delay)
    let inst2 = instance2.clone();
    let events2 = event_ids.clone();
    handles.push(tokio::spawn(async move {
        sleep(TokioDuration::from_millis(10)).await;
        for event_id in events2 {
            let _ = inst2.process_event(&event_id).await;
        }
    }));

    // Instance 3 (more delay)
    let inst3 = instance3.clone();
    let events3 = event_ids.clone();
    handles.push(tokio::spawn(async move {
        sleep(TokioDuration::from_millis(20)).await;
        for event_id in events3 {
            let _ = inst3.process_event(&event_id).await;
        }
    }));

    // Wait for all instances
    for handle in handles {
        handle.await?;
    }

    let duration = start.elapsed();

    // Collect stats
    let (processed1, dup1) = instance1.stats();
    let (processed2, dup2) = instance2.stats();
    let (processed3, dup3) = instance3.stats();

    info!("\nDistributed Deduplication Results:");
    info!("  Total time: {:.2}s", duration.as_secs_f64());
    info!("  Instance 1: {} unique, {} duplicates", processed1, dup1);
    info!("  Instance 2: {} unique, {} duplicates", processed2, dup2);
    info!("  Instance 3: {} unique, {} duplicates", processed3, dup3);
    info!("  Total unique processed: {}", processed1 + processed2 + processed3);
    info!("  Expected: {} (should be close)", event_ids.len());
    info!(
        "  Deduplication effectiveness: {:.1}%",
        (1.0 - (processed1 + processed2 + processed3) as f64 / (event_ids.len() * 3) as f64) * 100.0
    );

    Ok(())
}

/// Scenario 4: TTL management strategies
async fn scenario_ttl_management() -> Result<()> {
    info!("=== Scenario 4: TTL Management Strategies ===");

    let backend = Arc::new(MemoryStateBackend::new(Some(std::time::Duration::from_secs(5))));

    // Different TTL strategies for different event types
    info!("Testing different TTL strategies...");

    // Short TTL for high-frequency events
    let key_short = b"ttl:short:event";
    backend
        .put_with_ttl(key_short, b"data", std::time::Duration::from_secs(2))
        .await?;

    // Medium TTL for standard events
    let key_medium = b"ttl:medium:event";
    backend
        .put_with_ttl(key_medium, b"data", std::time::Duration::from_secs(5))
        .await?;

    // Long TTL for critical events
    let key_long = b"ttl:long:event";
    backend
        .put_with_ttl(key_long, b"data", std::time::Duration::from_secs(10))
        .await?;

    info!("All keys stored. Waiting for TTL expiration...");

    // Check at 3 seconds
    sleep(TokioDuration::from_secs(3)).await;
    info!("\nAfter 3 seconds:");
    info!("  Short TTL (2s): {}", if backend.get(key_short).await?.is_some() { "EXISTS" } else { "EXPIRED" });
    info!("  Medium TTL (5s): {}", if backend.get(key_medium).await?.is_some() { "EXISTS" } else { "EXPIRED" });
    info!("  Long TTL (10s): {}", if backend.get(key_long).await?.is_some() { "EXISTS" } else { "EXPIRED" });

    // Check at 6 seconds
    sleep(TokioDuration::from_secs(3)).await;
    info!("\nAfter 6 seconds:");
    info!("  Short TTL (2s): {}", if backend.get(key_short).await?.is_some() { "EXISTS" } else { "EXPIRED" });
    info!("  Medium TTL (5s): {}", if backend.get(key_medium).await?.is_some() { "EXISTS" } else { "EXPIRED" });
    info!("  Long TTL (10s): {}", if backend.get(key_long).await?.is_some() { "EXISTS" } else { "EXPIRED" });

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    info!("Advanced Event Deduplication Demo");
    info!("==================================\n");

    // Run all scenarios
    scenario_multi_key_deduplication().await?;
    println!();

    scenario_bloom_filter_high_throughput().await?;
    println!();

    if let Err(e) = scenario_distributed_deduplication().await {
        error!("Distributed scenario failed: {}", e);
    }
    println!();

    scenario_ttl_management().await?;
    println!();

    info!("=== All Advanced Scenarios Completed ===");

    Ok(())
}
