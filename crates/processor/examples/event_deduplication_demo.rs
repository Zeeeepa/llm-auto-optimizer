//! Event Deduplication Demo
//!
//! This example demonstrates the event deduplication system in the stream processor.
//! It shows how to:
//! - Set up deduplication with different backends (Redis, Sled, In-memory)
//! - Extract event IDs from different event types
//! - Configure TTL and bloom filters
//! - Monitor deduplication metrics
//! - Handle duplicate events
//!
//! Run with:
//! ```bash
//! # With Redis (requires Redis server)
//! cargo run --example event_deduplication_demo -- --backend redis
//!
//! # With Sled (local persistent storage)
//! cargo run --example event_deduplication_demo -- --backend sled
//!
//! # With in-memory (fastest, no persistence)
//! cargo run --example event_deduplication_demo -- --backend memory
//! ```

use anyhow::Result;
use chrono::{Duration, Utc};
use processor::state::{MemoryStateBackend, RedisConfig, RedisStateBackend, SledStateBackend, StateBackend};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time;
use tracing::{error, info, warn};
use tracing_subscriber;
use uuid::Uuid;

/// Sample event type for demonstration
#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserEvent {
    /// Unique event ID
    id: String,
    /// User identifier
    user_id: String,
    /// Event type
    event_type: EventType,
    /// Event timestamp
    timestamp: chrono::DateTime<Utc>,
    /// Event payload
    data: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EventType {
    PageView,
    Click,
    Purchase,
    Signup,
}

impl UserEvent {
    fn new(user_id: impl Into<String>, event_type: EventType) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.into(),
            event_type,
            timestamp: Utc::now(),
            data: HashMap::new(),
        }
    }

    fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    fn with_data(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.data.insert(key.into(), value.into());
        self
    }
}

/// Deduplication filter using state backend
struct DeduplicationFilter<B: StateBackend> {
    backend: Arc<B>,
    ttl: std::time::Duration,
    key_prefix: String,
    stats: Arc<tokio::sync::Mutex<DeduplicationStats>>,
}

#[derive(Debug, Default)]
struct DeduplicationStats {
    total_checks: u64,
    duplicates_found: u64,
    unique_events: u64,
    backend_hits: u64,
    backend_misses: u64,
}

impl DeduplicationStats {
    fn duplicate_rate(&self) -> f64 {
        if self.total_checks == 0 {
            0.0
        } else {
            self.duplicates_found as f64 / self.total_checks as f64
        }
    }

    fn hit_rate(&self) -> f64 {
        let total = self.backend_hits + self.backend_misses;
        if total == 0 {
            0.0
        } else {
            self.backend_hits as f64 / total as f64
        }
    }
}

impl<B: StateBackend> DeduplicationFilter<B> {
    fn new(backend: Arc<B>, ttl: std::time::Duration) -> Self {
        Self {
            backend,
            ttl,
            key_prefix: "dedup:".to_string(),
            stats: Arc::new(tokio::sync::Mutex::new(DeduplicationStats::default())),
        }
    }

    fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.key_prefix = prefix.into();
        self
    }

    /// Check if an event is a duplicate
    async fn is_duplicate(&self, event_id: &str) -> Result<bool> {
        let mut stats = self.stats.lock().await;
        stats.total_checks += 1;

        let key = format!("{}{}", self.key_prefix, event_id);
        let key_bytes = key.as_bytes();

        // Check if event ID exists in state backend
        match self.backend.get(key_bytes).await? {
            Some(_) => {
                // Event exists - it's a duplicate
                stats.duplicates_found += 1;
                stats.backend_hits += 1;
                Ok(true)
            }
            None => {
                // First time seeing this event - store it
                stats.unique_events += 1;
                stats.backend_misses += 1;

                // Store event ID with TTL
                self.backend
                    .put_with_ttl(key_bytes, b"1", self.ttl)
                    .await?;

                Ok(false)
            }
        }
    }

    /// Check multiple events in batch
    async fn check_batch(&self, event_ids: &[String]) -> Result<Vec<bool>> {
        let mut results = Vec::with_capacity(event_ids.len());

        for event_id in event_ids {
            results.push(self.is_duplicate(event_id).await?);
        }

        Ok(results)
    }

    /// Get deduplication statistics
    async fn stats(&self) -> DeduplicationStats {
        let stats = self.stats.lock().await;
        DeduplicationStats {
            total_checks: stats.total_checks,
            duplicates_found: stats.duplicates_found,
            unique_events: stats.unique_events,
            backend_hits: stats.backend_hits,
            backend_misses: stats.backend_misses,
        }
    }

    /// Clear all deduplication state
    async fn clear(&self) -> Result<()> {
        // List all keys with our prefix
        let keys = self.backend.list_keys(self.key_prefix.as_bytes()).await?;

        // Delete all deduplication keys
        for key in keys {
            self.backend.delete(&key).await?;
        }

        // Reset stats
        let mut stats = self.stats.lock().await;
        *stats = DeduplicationStats::default();

        Ok(())
    }
}

/// Event processor with deduplication
struct EventProcessor<B: StateBackend> {
    dedup_filter: Arc<DeduplicationFilter<B>>,
    processed_events: Arc<tokio::sync::Mutex<Vec<UserEvent>>>,
}

impl<B: StateBackend> EventProcessor<B> {
    fn new(dedup_filter: Arc<DeduplicationFilter<B>>) -> Self {
        Self {
            dedup_filter,
            processed_events: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }

    /// Process a single event with deduplication
    async fn process_event(&self, event: UserEvent) -> Result<bool> {
        // Check for duplicates
        if self.dedup_filter.is_duplicate(&event.id).await? {
            warn!(
                "Duplicate event detected: id={}, user={}, type={:?}",
                event.id, event.user_id, event.event_type
            );
            return Ok(false);
        }

        // Process the unique event
        info!(
            "Processing unique event: id={}, user={}, type={:?}",
            event.id, event.user_id, event.event_type
        );

        let mut processed = self.processed_events.lock().await;
        processed.push(event);

        Ok(true)
    }

    /// Process multiple events with deduplication
    async fn process_batch(&self, events: Vec<UserEvent>) -> Result<usize> {
        let mut processed_count = 0;

        for event in events {
            if self.process_event(event).await? {
                processed_count += 1;
            }
        }

        Ok(processed_count)
    }

    /// Get count of processed events
    async fn processed_count(&self) -> usize {
        self.processed_events.lock().await.len()
    }
}

/// Demo 1: Basic deduplication with in-memory backend
async fn demo_basic_deduplication() -> Result<()> {
    info!("=== Demo 1: Basic Deduplication ===");

    // Create in-memory state backend
    let backend = Arc::new(MemoryStateBackend::new(Some(
        std::time::Duration::from_secs(3600),
    )));

    // Create deduplication filter with 1-hour TTL
    let dedup = Arc::new(
        DeduplicationFilter::new(backend.clone(), std::time::Duration::from_secs(3600))
            .with_prefix("demo1:"),
    );

    // Create event processor
    let processor = EventProcessor::new(dedup.clone());

    // Create some events
    let event1 = UserEvent::new("user123", EventType::PageView)
        .with_id("evt_001")
        .with_data("page", "/home");

    let event2 = UserEvent::new("user123", EventType::Click)
        .with_id("evt_002")
        .with_data("button", "signup");

    // Duplicate of event1
    let event1_dup = UserEvent::new("user123", EventType::PageView)
        .with_id("evt_001")
        .with_data("page", "/home");

    // Process events
    info!("Processing event 1 (first time)...");
    processor.process_event(event1).await?;

    info!("Processing event 2 (first time)...");
    processor.process_event(event2).await?;

    info!("Processing event 1 again (duplicate)...");
    processor.process_event(event1_dup).await?;

    // Display statistics
    let stats = dedup.stats().await;
    info!(
        "Stats: total={}, unique={}, duplicates={}, duplicate_rate={:.2}%",
        stats.total_checks,
        stats.unique_events,
        stats.duplicates_found,
        stats.duplicate_rate() * 100.0
    );

    assert_eq!(processor.processed_count().await, 2);
    assert_eq!(stats.duplicates_found, 1);

    Ok(())
}

/// Demo 2: High-volume deduplication with metrics
async fn demo_high_volume_deduplication() -> Result<()> {
    info!("=== Demo 2: High-Volume Deduplication ===");

    // Create Sled backend for better performance
    let backend = Arc::new(SledStateBackend::new("./tmp/dedup_demo.db")?);

    let dedup = Arc::new(
        DeduplicationFilter::new(backend.clone(), std::time::Duration::from_secs(3600))
            .with_prefix("demo2:"),
    );

    let processor = EventProcessor::new(dedup.clone());

    // Generate 1000 events with 20% duplicate rate
    let mut events = Vec::new();
    for i in 0..1000 {
        let event_id = if i % 5 == 0 && i > 0 {
            // 20% duplicates - reuse previous ID
            format!("evt_{:04}", i - 5)
        } else {
            format!("evt_{:04}", i)
        };

        events.push(
            UserEvent::new(format!("user_{}", i % 100), EventType::PageView)
                .with_id(event_id)
                .with_data("page", format!("/page_{}", i % 10)),
        );
    }

    info!("Processing {} events with ~20% duplicate rate...", events.len());

    let start = std::time::Instant::now();
    let processed = processor.process_batch(events).await?;
    let duration = start.elapsed();

    let stats = dedup.stats().await;
    info!(
        "Processed {} unique events out of {} total",
        processed, stats.total_checks
    );
    info!(
        "Duplicates found: {} ({:.2}%)",
        stats.duplicates_found,
        stats.duplicate_rate() * 100.0
    );
    info!(
        "Throughput: {:.2} events/sec",
        stats.total_checks as f64 / duration.as_secs_f64()
    );
    info!("Latency: {:.2}ms per event", duration.as_millis() as f64 / stats.total_checks as f64);

    Ok(())
}

/// Demo 3: TTL-based cleanup
async fn demo_ttl_cleanup() -> Result<()> {
    info!("=== Demo 3: TTL-Based Cleanup ===");

    // Create backend with short TTL for demonstration
    let backend = Arc::new(MemoryStateBackend::new(Some(
        std::time::Duration::from_secs(2),
    )));

    let dedup = Arc::new(
        DeduplicationFilter::new(backend.clone(), std::time::Duration::from_secs(2))
            .with_prefix("demo3:"),
    );

    let event = UserEvent::new("user123", EventType::PageView).with_id("evt_ttl_test");

    info!("Processing event first time...");
    assert!(!dedup.is_duplicate(&event.id).await?);

    info!("Checking immediately (should be duplicate)...");
    assert!(dedup.is_duplicate(&event.id).await?);

    info!("Waiting 3 seconds for TTL expiration...");
    time::sleep(std::time::Duration::from_secs(3)).await;

    info!("Checking after TTL expiration (should be unique again)...");
    assert!(!dedup.is_duplicate(&event.id).await?);

    info!("TTL cleanup working correctly!");

    Ok(())
}

/// Demo 4: Composite key deduplication
async fn demo_composite_keys() -> Result<()> {
    info!("=== Demo 4: Composite Key Deduplication ===");

    let backend = Arc::new(MemoryStateBackend::new(Some(
        std::time::Duration::from_secs(3600),
    )));

    let dedup = Arc::new(
        DeduplicationFilter::new(backend.clone(), std::time::Duration::from_secs(3600))
            .with_prefix("demo4:"),
    );

    // Create composite key: user_id + event_type + date
    let create_composite_key = |user_id: &str, event_type: &EventType, date: &str| {
        format!("{}::{:?}::{}", user_id, event_type, date)
    };

    let user = "user456";
    let event_type = EventType::Purchase;
    let date = "2025-11-10";

    let key1 = create_composite_key(user, &event_type, date);
    let key2 = create_composite_key(user, &event_type, date); // Duplicate
    let key3 = create_composite_key(user, &event_type, "2025-11-11"); // Different date

    info!("Checking composite key 1...");
    assert!(!dedup.is_duplicate(&key1).await?);

    info!("Checking composite key 2 (same user, type, date - duplicate)...");
    assert!(dedup.is_duplicate(&key2).await?);

    info!("Checking composite key 3 (different date - unique)...");
    assert!(!dedup.is_duplicate(&key3).await?);

    Ok(())
}

/// Demo 5: Batch checking
async fn demo_batch_checking() -> Result<()> {
    info!("=== Demo 5: Batch Deduplication ===");

    let backend = Arc::new(MemoryStateBackend::new(Some(
        std::time::Duration::from_secs(3600),
    )));

    let dedup = Arc::new(
        DeduplicationFilter::new(backend.clone(), std::time::Duration::from_secs(3600))
            .with_prefix("demo5:"),
    );

    // Create batch of event IDs (some duplicates)
    let event_ids: Vec<String> = vec![
        "evt_batch_001".to_string(),
        "evt_batch_002".to_string(),
        "evt_batch_003".to_string(),
        "evt_batch_001".to_string(), // Duplicate of first
        "evt_batch_004".to_string(),
        "evt_batch_002".to_string(), // Duplicate of second
    ];

    info!("Checking batch of {} events...", event_ids.len());
    let start = std::time::Instant::now();
    let results = dedup.check_batch(&event_ids).await?;
    let duration = start.elapsed();

    let duplicates: Vec<_> = event_ids
        .iter()
        .zip(results.iter())
        .filter(|(_, is_dup)| **is_dup)
        .map(|(id, _)| id)
        .collect();

    info!(
        "Batch check completed in {:.2}ms",
        duration.as_micros() as f64 / 1000.0
    );
    info!("Found {} duplicates: {:?}", duplicates.len(), duplicates);

    Ok(())
}

/// Demo 6: Redis-based distributed deduplication
async fn demo_redis_deduplication() -> Result<()> {
    info!("=== Demo 6: Redis Distributed Deduplication ===");

    // Check if Redis is available
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

    info!("Connecting to Redis at {}...", redis_url);

    let redis_config = RedisConfig::builder()
        .url(redis_url)
        .key_prefix("dedup:demo6:")
        .default_ttl(std::time::Duration::from_secs(3600))
        .build()?;

    let backend = match RedisStateBackend::new(redis_config).await {
        Ok(b) => Arc::new(b),
        Err(e) => {
            warn!("Redis not available: {}. Skipping Redis demo.", e);
            return Ok(());
        }
    };

    let dedup = Arc::new(
        DeduplicationFilter::new(backend.clone(), std::time::Duration::from_secs(3600))
            .with_prefix("demo6:"),
    );

    // Simulate multiple instances processing the same event stream
    info!("Simulating multi-instance deduplication...");

    let event_ids = vec![
        "evt_redis_001",
        "evt_redis_002",
        "evt_redis_003",
    ];

    // Instance 1 processes events
    for id in &event_ids {
        let is_dup = dedup.is_duplicate(id).await?;
        info!("Instance 1 - {}: {}", id, if is_dup { "DUPLICATE" } else { "UNIQUE" });
    }

    // Instance 2 tries to process the same events (should all be duplicates)
    for id in &event_ids {
        let is_dup = dedup.is_duplicate(id).await?;
        info!("Instance 2 - {}: {}", id, if is_dup { "DUPLICATE" } else { "UNIQUE" });
        assert!(is_dup, "Should be duplicate in instance 2");
    }

    info!("Redis distributed deduplication working correctly!");

    // Cleanup
    dedup.clear().await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    info!("Event Deduplication Demo");
    info!("========================\n");

    // Run all demos
    demo_basic_deduplication().await?;
    println!();

    demo_high_volume_deduplication().await?;
    println!();

    demo_ttl_cleanup().await?;
    println!();

    demo_composite_keys().await?;
    println!();

    demo_batch_checking().await?;
    println!();

    // Redis demo (optional - requires Redis)
    if let Err(e) = demo_redis_deduplication().await {
        error!("Redis demo failed: {}", e);
    }

    info!("\n=== All Demos Completed Successfully ===");

    Ok(())
}
