//! Comprehensive Distributed State Backend Demo
//!
//! This example demonstrates all features of the Redis and PostgreSQL state backends
//! including basic operations, batch operations, transactions, distributed locking,
//! checkpointing, and high availability patterns.
//!
//! ## Prerequisites
//!
//! ```bash
//! # Start services
//! docker-compose -f examples/docker-compose.state.yml up -d
//!
//! # Run demo
//! cargo run --example distributed_state_demo
//! ```

use processor::state::{
    DistributedLock, PostgresConfig, PostgresStateBackend, RedisConfig, RedisDistributedLock,
    RedisStateBackend, StateBackend,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=".repeat(80));
    println!("Distributed State Backend Demo");
    println!("=".repeat(80));

    // Run all demos
    redis_basic_demo().await?;
    redis_advanced_demo().await?;
    redis_cluster_demo().await?;
    postgres_basic_demo().await?;
    postgres_advanced_demo().await?;
    distributed_lock_demo().await?;
    high_availability_demo().await?;
    production_patterns_demo().await?;

    println!("\n{}", "=".repeat(80));
    println!("All demos completed successfully!");
    println!("=".repeat(80));

    Ok(())
}

// ============================================================================
// REDIS BASIC OPERATIONS
// ============================================================================

async fn redis_basic_demo() -> anyhow::Result<()> {
    println!("\n{}", "-".repeat(80));
    println!("1. Redis Basic Operations");
    println!("{}", "-".repeat(80));

    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("demo:basic:")
        .default_ttl(Duration::from_secs(3600))
        .pool_size(5, 20)
        .build()?;

    let backend = RedisStateBackend::new(config).await?;
    backend.clear().await?;

    println!("✓ Connected to Redis");

    // PUT operation
    backend.put(b"user:123", b"alice").await?;
    println!("✓ PUT: user:123 = alice");

    // GET operation
    let value = backend.get(b"user:123").await?;
    println!("✓ GET: user:123 = {:?}", String::from_utf8_lossy(&value.unwrap()));

    // UPDATE operation
    backend.put(b"user:123", b"alice_updated").await?;
    let value = backend.get(b"user:123").await?;
    println!("✓ UPDATE: user:123 = {:?}", String::from_utf8_lossy(&value.unwrap()));

    // DELETE operation
    backend.delete(b"user:123").await?;
    let value = backend.get(b"user:123").await?;
    println!("✓ DELETE: user:123, exists = {}", value.is_some());

    // CONTAINS check
    backend.put(b"exists_key", b"value").await?;
    println!("✓ CONTAINS: exists_key = {}", backend.contains(b"exists_key").await?);

    // COUNT operation
    println!("✓ COUNT: {} keys", backend.count().await?);

    // LIST KEYS operation
    backend.put(b"window:1", b"data1").await?;
    backend.put(b"window:2", b"data2").await?;
    backend.put(b"meta:1", b"metadata").await?;

    let window_keys = backend.list_keys(b"window:").await?;
    println!("✓ LIST KEYS with prefix 'window:': {} keys", window_keys.len());

    Ok(())
}

// ============================================================================
// REDIS ADVANCED FEATURES
// ============================================================================

async fn redis_advanced_demo() -> anyhow::Result<()> {
    println!("\n{}", "-".repeat(80));
    println!("2. Redis Advanced Features");
    println!("{}", "-".repeat(80));

    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("demo:advanced:")
        .pipeline_batch_size(100)
        .build()?;

    let backend = RedisStateBackend::new(config).await?;
    backend.clear().await?;

    // TTL management
    backend.put_with_ttl(b"session:temp", b"data", Duration::from_secs(5)).await?;
    let ttl = backend.ttl(b"session:temp").await?;
    println!("✓ TTL: session:temp expires in {:?}", ttl);

    // Batch PUT
    let data: Vec<(Vec<u8>, Vec<u8>)> = (0..100)
        .map(|i| (format!("batch_key_{}", i).into_bytes(), format!("value_{}", i).into_bytes()))
        .collect();

    let refs: Vec<(&[u8], &[u8])> = data
        .iter()
        .map(|(k, v)| (k.as_slice(), v.as_slice()))
        .collect();

    backend.batch_put(&refs).await?;
    println!("✓ BATCH PUT: {} keys", data.len());

    // Batch GET
    let keys: Vec<&[u8]> = data.iter().take(10).map(|(k, _)| k.as_slice()).collect();
    let values = backend.batch_get(&keys).await?;
    println!("✓ BATCH GET: retrieved {} values", values.len());

    // Batch DELETE
    let deleted = backend.batch_delete(&keys).await?;
    println!("✓ BATCH DELETE: removed {} keys", deleted);

    // Atomic operations
    backend.put(b"atomic_key", b"original").await?;
    let value = backend.get_and_delete(b"atomic_key").await?;
    println!("✓ GET_AND_DELETE: retrieved and deleted atomically");

    let was_set = backend.set_if_not_exists(
        b"once_key",
        b"first_value",
        Duration::from_secs(60)
    ).await?;
    println!("✓ SET_IF_NOT_EXISTS: first attempt = {}", was_set);

    let was_set = backend.set_if_not_exists(
        b"once_key",
        b"second_value",
        Duration::from_secs(60)
    ).await?;
    println!("✓ SET_IF_NOT_EXISTS: second attempt = {}", was_set);

    // Checkpoint and restore
    backend.clear().await?;
    for i in 0..10 {
        let key = format!("checkpoint_key_{}", i);
        backend.put(key.as_bytes(), b"checkpoint_data").await?;
    }

    let checkpoint = backend.checkpoint().await?;
    println!("✓ CHECKPOINT: saved {} keys", checkpoint.len());

    backend.clear().await?;
    println!("✓ Cleared all keys");

    backend.restore(checkpoint).await?;
    println!("✓ RESTORE: restored {} keys", backend.count().await?);

    // Statistics
    let stats = backend.stats().await;
    println!("✓ STATS:");
    println!("  - Total ops: {}", stats.total_operations());
    println!("  - Hit rate: {:.2}%", stats.hit_rate() * 100.0);
    println!("  - Error rate: {:.2}%", stats.error_rate() * 100.0);
    println!("  - Avg latency: {}µs", stats.avg_latency_us);

    // Health check
    let healthy = backend.health_check().await?;
    println!("✓ HEALTH CHECK: {}", if healthy { "OK" } else { "FAILED" });

    Ok(())
}

// ============================================================================
// REDIS CLUSTER DEMO
// ============================================================================

async fn redis_cluster_demo() -> anyhow::Result<()> {
    println!("\n{}", "-".repeat(80));
    println!("3. Redis Cluster (Simulated)");
    println!("{}", "-".repeat(80));

    // For this demo, we'll use standalone Redis but show cluster configuration
    println!("✓ Cluster configuration (example):");
    println!("  - Node 1: redis://node1:7000");
    println!("  - Node 2: redis://node2:7001");
    println!("  - Node 3: redis://node3:7002");

    let config = RedisConfig::builder()
        .url("redis://localhost:6379")  // In production, use cluster URLs
        .key_prefix("demo:cluster:")
        .pool_size(20, 100)
        .build()?;

    let backend = RedisStateBackend::new(config).await?;

    // Demonstrate hash slot distribution (conceptual)
    println!("✓ Hash slot distribution:");
    for i in 0..5 {
        let key = format!("{{user_{}}}_data", i);  // Hash tag for same slot
        backend.put(key.as_bytes(), b"data").await?;
        println!("  - Key: {}", key);
    }

    println!("✓ Cluster operations completed");

    Ok(())
}

// ============================================================================
// POSTGRESQL BASIC OPERATIONS
// ============================================================================

async fn postgres_basic_demo() -> anyhow::Result<()> {
    println!("\n{}", "-".repeat(80));
    println!("4. PostgreSQL Basic Operations");
    println!("{}", "-".repeat(80));

    let config = PostgresConfig::new("postgresql://postgres:postgres@localhost:5432/optimizer")
        .with_pool_size(5, 20)
        .with_timeout(Duration::from_secs(30));

    let backend = PostgresStateBackend::new(config).await?;
    backend.clear().await?;

    println!("✓ Connected to PostgreSQL");

    // Basic CRUD operations
    backend.put(b"user:456", b"bob").await?;
    println!("✓ PUT: user:456 = bob");

    let value = backend.get(b"user:456").await?;
    println!("✓ GET: user:456 = {:?}", String::from_utf8_lossy(&value.unwrap()));

    backend.put(b"user:456", b"bob_updated").await?;
    println!("✓ UPDATE: user:456 = bob_updated");

    backend.delete(b"user:456").await?;
    println!("✓ DELETE: user:456");

    // Verify count
    println!("✓ COUNT: {} rows", backend.count().await?);

    Ok(())
}

// ============================================================================
// POSTGRESQL ADVANCED FEATURES
// ============================================================================

async fn postgres_advanced_demo() -> anyhow::Result<()> {
    println!("\n{}", "-".repeat(80));
    println!("5. PostgreSQL Advanced Features");
    println!("{}", "-".repeat(80));

    let config = PostgresConfig::new("postgresql://postgres:postgres@localhost:5432/optimizer");
    let backend = PostgresStateBackend::new(config).await?;
    backend.clear().await?;

    // PUT with metadata (JSONB)
    backend.put_with_metadata(
        b"event:123",
        b"event_data",
        serde_json::json!({
            "source": "kafka",
            "partition": 5,
            "offset": 12345,
            "timestamp": "2025-11-10T12:00:00Z"
        })
    ).await?;
    println!("✓ PUT with JSONB metadata");

    // PUT with TTL
    backend.put_with_ttl(
        b"temp:key",
        b"temporary_data",
        Duration::from_secs(300)
    ).await?;
    println!("✓ PUT with TTL (5 minutes)");

    // Batch operations
    let data: Vec<(Vec<u8>, Vec<u8>)> = (0..50)
        .map(|i| (format!("pg_key_{}", i).into_bytes(), format!("pg_value_{}", i).into_bytes()))
        .collect();

    let refs: Vec<(&[u8], &[u8])> = data
        .iter()
        .map(|(k, v)| (k.as_slice(), v.as_slice()))
        .collect();

    backend.batch_put(&refs).await?;
    println!("✓ BATCH PUT: {} rows", data.len());

    // Checkpoint operations
    let checkpoint_id = backend.create_checkpoint().await?;
    println!("✓ CREATE CHECKPOINT: {}", checkpoint_id);

    let checkpoints = backend.list_checkpoints().await?;
    println!("✓ LIST CHECKPOINTS: {} checkpoints", checkpoints.len());

    for cp in &checkpoints {
        println!("  - Checkpoint {}: {} rows, {} bytes",
            cp.checkpoint_id, cp.row_count, cp.data_size);
    }

    // Modify state
    backend.put(b"pg_key_0", b"modified").await?;

    // Restore checkpoint
    backend.restore_checkpoint(checkpoint_id).await?;
    println!("✓ RESTORE CHECKPOINT: state restored");

    // Verify restoration
    let value = backend.get(b"pg_key_0").await?;
    println!("  - Restored value: {:?}", String::from_utf8_lossy(&value.unwrap()));

    // Cleanup expired entries
    backend.put_with_ttl(b"expired1", b"data", Duration::from_secs(1)).await?;
    sleep(Duration::from_secs(2)).await;

    let deleted = backend.cleanup_expired().await?;
    println!("✓ CLEANUP EXPIRED: {} entries removed", deleted);

    // Vacuum
    backend.vacuum().await?;
    println!("✓ VACUUM: completed");

    // Table statistics
    let (total_size, table_size) = backend.table_stats().await?;
    println!("✓ TABLE STATS:");
    println!("  - Total size: {} KB", total_size / 1024);
    println!("  - Table size: {} KB", table_size / 1024);

    // Backend statistics
    let stats = backend.stats().await;
    println!("✓ BACKEND STATS:");
    println!("  - Queries: {}", stats.query_count);
    println!("  - Transactions: {}", stats.transaction_count);
    println!("  - Retries: {}", stats.retry_count);

    Ok(())
}

// ============================================================================
// DISTRIBUTED LOCKING DEMO
// ============================================================================

async fn distributed_lock_demo() -> anyhow::Result<()> {
    println!("\n{}", "-".repeat(80));
    println!("6. Distributed Locking");
    println!("{}", "-".repeat(80));

    let redis_lock = Arc::new(RedisDistributedLock::new("redis://localhost:6379").await?);

    // Try acquire (non-blocking)
    println!("Demo: Non-blocking lock acquisition");
    if let Some(guard) = redis_lock.try_acquire("resource1", Duration::from_secs(10)).await? {
        println!("✓ Acquired lock on resource1");

        // Try to acquire again (should fail)
        if let Some(_) = redis_lock.try_acquire("resource1", Duration::from_secs(10)).await? {
            println!("✗ Should not acquire lock twice!");
        } else {
            println!("✓ Second acquisition failed (as expected)");
        }

        drop(guard);
        println!("✓ Released lock");
    }

    // Blocking acquire
    println!("\nDemo: Blocking lock acquisition");
    let guard = redis_lock.acquire("resource2", Duration::from_secs(30)).await?;
    println!("✓ Acquired lock with blocking call");

    // Check lock status
    let is_locked = redis_lock.is_locked("resource2").await?;
    println!("✓ Lock status: {}", if is_locked { "LOCKED" } else { "FREE" });

    // Lock extension
    println!("\nDemo: Lock extension");
    let mut guard_ext = redis_lock.acquire("resource3", Duration::from_secs(5)).await?;
    println!("✓ Acquired lock with 5s TTL");

    sleep(Duration::from_secs(3)).await;
    redis_lock.extend(&mut guard_ext, Duration::from_secs(10)).await?;
    println!("✓ Extended lock to 10s");

    // Leader election pattern
    println!("\nDemo: Leader election pattern");
    let lock_clone1 = redis_lock.clone();
    let lock_clone2 = redis_lock.clone();

    let handle1 = tokio::spawn(async move {
        if let Some(guard) = lock_clone1.try_acquire("leader", Duration::from_secs(5)).await.ok().flatten() {
            println!("  Instance 1: Became leader");
            sleep(Duration::from_secs(2)).await;
            drop(guard);
            println!("  Instance 1: Stepped down");
        } else {
            println!("  Instance 1: Failed to become leader");
        }
    });

    let handle2 = tokio::spawn(async move {
        sleep(Duration::from_millis(100)).await;  // Slight delay

        if let Some(guard) = lock_clone2.try_acquire("leader", Duration::from_secs(5)).await.ok().flatten() {
            println!("  Instance 2: Became leader");
            sleep(Duration::from_secs(2)).await;
            drop(guard);
            println!("  Instance 2: Stepped down");
        } else {
            println!("  Instance 2: Failed to become leader");
        }
    });

    handle1.await?;
    handle2.await?;

    println!("✓ Leader election completed");

    Ok(())
}

// ============================================================================
// HIGH AVAILABILITY DEMO
// ============================================================================

async fn high_availability_demo() -> anyhow::Result<()> {
    println!("\n{}", "-".repeat(80));
    println!("7. High Availability Patterns");
    println!("{}", "-".repeat(80));

    // Simulated HA configuration
    println!("✓ Redis Sentinel configuration:");
    println!("  - Sentinel 1: localhost:26379");
    println!("  - Sentinel 2: localhost:26380");
    println!("  - Sentinel 3: localhost:26381");

    println!("\n✓ PostgreSQL replication:");
    println!("  - Primary: localhost:5432");
    println!("  - Replica 1: localhost:5433");
    println!("  - Replica 2: localhost:5434");

    // Connection retry demonstration
    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("demo:ha:")
        .retries(5, Duration::from_millis(100), Duration::from_secs(5))
        .build()?;

    let backend = RedisStateBackend::new(config).await?;

    println!("\n✓ Retry configuration:");
    println!("  - Max retries: 5");
    println!("  - Base delay: 100ms");
    println!("  - Max delay: 5s");

    // Test normal operation
    backend.put(b"ha_key", b"ha_value").await?;
    println!("✓ Normal operation successful");

    Ok(())
}

// ============================================================================
// PRODUCTION PATTERNS
// ============================================================================

async fn production_patterns_demo() -> anyhow::Result<()> {
    println!("\n{}", "-".repeat(80));
    println!("8. Production Patterns");
    println!("{}", "-".repeat(80));

    // Circuit breaker pattern
    println!("Pattern 1: Circuit Breaker");
    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("demo:prod:")
        .retries(3, Duration::from_millis(100), Duration::from_secs(1))
        .build()?;

    let backend = RedisStateBackend::new(config).await?;

    let stats = backend.stats().await;
    if stats.error_rate() > 0.1 {
        println!("⚠ Circuit breaker would trip (error rate > 10%)");
    } else {
        println!("✓ Circuit breaker closed (error rate OK)");
    }

    // Graceful degradation
    println!("\nPattern 2: Graceful Degradation");
    println!("✓ If backend fails, fall back to:");
    println!("  1. In-memory cache");
    println!("  2. Read replicas");
    println!("  3. Stale data with warning");

    // Health check integration
    println!("\nPattern 3: Health Checks");
    let healthy = backend.health_check().await?;
    println!("✓ Backend health: {}", if healthy { "HEALTHY" } else { "UNHEALTHY" });

    // Monitoring integration
    println!("\nPattern 4: Metrics Collection");
    let stats = backend.stats().await;
    println!("✓ Metrics exported:");
    println!("  - state_operations_total: {}", stats.total_operations());
    println!("  - state_hit_rate: {:.2}", stats.hit_rate());
    println!("  - state_error_rate: {:.2}", stats.error_rate());
    println!("  - state_latency_avg_us: {}", stats.avg_latency_us);

    // Rate limiting
    println!("\nPattern 5: Rate Limiting");
    println!("✓ Connection pool limits:");
    println!("  - Min connections: 10");
    println!("  - Max connections: 50");
    println!("  - Acquire timeout: 5s");

    Ok(())
}
