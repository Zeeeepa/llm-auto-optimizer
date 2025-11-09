//! Redis State Backend Demo
//!
//! This example demonstrates the Redis state backend features including:
//! - Basic CRUD operations
//! - TTL and expiration
//! - Batch operations
//! - Connection pooling
//! - Stats tracking
//!
//! ## Prerequisites
//!
//! Start Redis:
//! ```bash
//! docker-compose up -d redis
//! ```
//!
//! ## Run Example
//!
//! ```bash
//! cargo run --example redis_state_demo
//! ```

use processor::state::{RedisConfig, RedisStateBackend, StateBackend};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Redis State Backend Demo ===\n");

    // Create Redis configuration
    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("demo:")
        .default_ttl(Duration::from_secs(3600)) // 1 hour default TTL
        .pool_size(5, 20)
        .build()?;

    println!("Connecting to Redis at {}", config.primary_url());

    let backend = RedisStateBackend::new(config).await?;

    // Health check
    println!("Performing health check...");
    if backend.health_check().await? {
        println!("✓ Redis connection healthy\n");
    } else {
        println!("✗ Redis connection unhealthy\n");
        return Ok(());
    }

    // Clear any existing demo data
    backend.clear().await?;

    // ========================================================================
    // Demo 1: Basic CRUD Operations
    // ========================================================================
    println!("--- Demo 1: Basic CRUD Operations ---");

    // PUT
    println!("Storing user:1001 = Alice");
    backend.put(b"user:1001", b"Alice").await?;

    // GET
    if let Some(value) = backend.get(b"user:1001").await? {
        println!("Retrieved: {}", String::from_utf8_lossy(&value));
    }

    // UPDATE (overwrite)
    println!("Updating user:1001 = Alice Smith");
    backend.put(b"user:1001", b"Alice Smith").await?;

    if let Some(value) = backend.get(b"user:1001").await? {
        println!("Updated value: {}", String::from_utf8_lossy(&value));
    }

    // DELETE
    println!("Deleting user:1001");
    backend.delete(b"user:1001").await?;

    if backend.get(b"user:1001").await?.is_none() {
        println!("✓ User deleted successfully\n");
    }

    // ========================================================================
    // Demo 2: TTL and Expiration
    // ========================================================================
    println!("--- Demo 2: TTL and Expiration ---");

    // Store with custom TTL (5 seconds)
    println!("Storing session:abc with 5-second TTL");
    backend
        .put_with_ttl(b"session:abc", b"user_session_data", Duration::from_secs(5))
        .await?;

    // Check TTL
    if let Some(ttl) = backend.ttl(b"session:abc").await? {
        println!("Remaining TTL: {:?}", ttl);
    }

    println!("Waiting 2 seconds...");
    tokio::time::sleep(Duration::from_secs(2)).await;

    if let Some(ttl) = backend.ttl(b"session:abc").await? {
        println!("Remaining TTL: {:?}", ttl);
    }

    // Store permanent key
    println!("Storing config:version with no expiration");
    backend.put(b"config:version", b"1.0.0").await?;

    if let Some(ttl) = backend.ttl(b"config:version").await? {
        if ttl == Duration::MAX {
            println!("✓ No expiration set\n");
        }
    }

    // ========================================================================
    // Demo 3: Batch Operations
    // ========================================================================
    println!("--- Demo 3: Batch Operations ---");

    // Batch PUT
    println!("Batch inserting 5 users...");
    let users = vec![
        (&b"user:2001"[..], &b"Bob"[..]),
        (&b"user:2002"[..], &b"Carol"[..]),
        (&b"user:2003"[..], &b"Dave"[..]),
        (&b"user:2004"[..], &b"Eve"[..]),
        (&b"user:2005"[..], &b"Frank"[..]),
    ];

    backend.batch_put(&users).await?;
    println!("✓ Batch insert complete");

    // Batch GET
    println!("Batch retrieving users...");
    let keys: Vec<&[u8]> = vec![
        b"user:2001",
        b"user:2002",
        b"user:2003",
        b"user:2004",
        b"user:2005",
    ];

    let values = backend.batch_get(&keys).await?;
    for (i, value) in values.iter().enumerate() {
        if let Some(v) = value {
            println!("  user:200{} = {}", i + 1, String::from_utf8_lossy(v));
        }
    }

    // List keys with prefix
    println!("\nListing all user keys:");
    let user_keys = backend.list_keys(b"user:").await?;
    println!("Found {} user keys", user_keys.len());

    // Batch DELETE
    println!("\nBatch deleting users...");
    let deleted_count = backend.batch_delete(&keys).await?;
    println!("✓ Deleted {} keys\n", deleted_count);

    // ========================================================================
    // Demo 4: Atomic Operations
    // ========================================================================
    println!("--- Demo 4: Atomic Operations ---");

    // Set if not exists
    println!("Setting lock:resource if not exists...");
    let set1 = backend
        .set_if_not_exists(b"lock:resource", b"locked", Duration::from_secs(60))
        .await?;
    println!("First attempt: {}", if set1 { "SUCCESS" } else { "FAILED" });

    let set2 = backend
        .set_if_not_exists(b"lock:resource", b"locked", Duration::from_secs(60))
        .await?;
    println!("Second attempt: {}", if set2 { "SUCCESS" } else { "FAILED (as expected)" });

    // Get and delete atomically
    println!("\nGet and delete atomically...");
    if let Some(value) = backend.get_and_delete(b"lock:resource").await? {
        println!("Retrieved and deleted: {}", String::from_utf8_lossy(&value));
    }

    if backend.contains(b"lock:resource").await? {
        println!("✗ Key still exists (unexpected)");
    } else {
        println!("✓ Key deleted atomically\n");
    }

    // ========================================================================
    // Demo 5: Checkpoint and Restore
    // ========================================================================
    println!("--- Demo 5: Checkpoint and Restore ---");

    // Create some state
    println!("Creating application state...");
    backend.put(b"app:counter", b"42").await?;
    backend.put(b"app:status", b"running").await?;
    backend.put(b"app:version", b"2.0.0").await?;

    // Create checkpoint
    println!("Creating checkpoint...");
    let checkpoint = backend.checkpoint().await?;
    println!("✓ Checkpoint created with {} entries", checkpoint.len());

    // Simulate crash - clear all data
    println!("\nSimulating system crash...");
    backend.clear().await?;
    println!("All data cleared");

    // Restore from checkpoint
    println!("\nRestoring from checkpoint...");
    backend.restore(checkpoint).await?;
    println!("✓ Restore complete");

    // Verify restoration
    println!("\nVerifying restored data:");
    if let Some(value) = backend.get(b"app:counter").await? {
        println!("  app:counter = {}", String::from_utf8_lossy(&value));
    }
    if let Some(value) = backend.get(b"app:status").await? {
        println!("  app:status = {}", String::from_utf8_lossy(&value));
    }
    if let Some(value) = backend.get(b"app:version").await? {
        println!("  app:version = {}\n", String::from_utf8_lossy(&value));
    }

    // ========================================================================
    // Demo 6: Statistics and Monitoring
    // ========================================================================
    println!("--- Demo 6: Statistics and Monitoring ---");

    // Perform various operations
    backend.put(b"metric:cpu", b"75").await?;
    backend.put(b"metric:memory", b"8192").await?;
    backend.put(b"metric:disk", b"500").await?;

    backend.get(b"metric:cpu").await?; // Hit
    backend.get(b"metric:memory").await?; // Hit
    backend.get(b"metric:nonexistent").await?; // Miss

    // Get stats
    let stats = backend.stats().await;
    println!("Operation Statistics:");
    println!("  Total operations: {}", stats.total_operations());
    println!("  PUT operations: {}", stats.put_count);
    println!("  GET operations: {}", stats.get_count);
    println!("  DELETE operations: {}", stats.delete_count);
    println!("  Cache hits: {}", stats.hit_count);
    println!("  Cache misses: {}", stats.miss_count);
    println!("  Hit rate: {:.2}%", stats.hit_rate() * 100.0);
    println!("  Error rate: {:.2}%", stats.error_rate() * 100.0);
    println!("  Bytes read: {}", stats.bytes_read);
    println!("  Bytes written: {}", stats.bytes_written);
    println!("  Avg latency: {}μs\n", stats.avg_latency_us);

    // ========================================================================
    // Demo 7: Memory Usage
    // ========================================================================
    println!("--- Demo 7: Memory Usage ---");

    let memory = backend.memory_usage().await?;
    println!("Redis memory usage: {} bytes ({:.2} MB)\n", memory, memory as f64 / 1_048_576.0);

    // ========================================================================
    // Demo 8: Large Dataset Handling
    // ========================================================================
    println!("--- Demo 8: Large Dataset Handling ---");

    println!("Inserting 1000 entries...");
    for i in 0..1000 {
        let key = format!("large:key:{}", i);
        let value = format!("value_{}", i);
        backend.put(key.as_bytes(), value.as_bytes()).await?;
    }
    println!("✓ Insert complete");

    // Count keys
    let count = backend.count().await?;
    println!("Total keys in backend: {}", count);

    // List with prefix
    let large_keys = backend.list_keys(b"large:").await?;
    println!("Keys with 'large:' prefix: {}", large_keys.len());

    // Clean up large dataset
    println!("\nCleaning up large dataset...");
    let delete_keys: Vec<&[u8]> = large_keys.iter().map(|k| k.as_slice()).collect();
    for chunk in delete_keys.chunks(100) {
        backend.batch_delete(chunk).await?;
    }
    println!("✓ Cleanup complete\n");

    // ========================================================================
    // Demo 9: Server Info
    // ========================================================================
    println!("--- Demo 9: Server Info ---");

    let info = backend.server_info().await?;
    println!("Redis Server Info (excerpt):");
    for line in info.lines().take(10) {
        if !line.is_empty() && !line.starts_with('#') {
            println!("  {}", line);
        }
    }
    println!();

    // ========================================================================
    // Cleanup
    // ========================================================================
    println!("--- Cleanup ---");
    backend.clear().await?;
    println!("✓ All demo data cleared");

    // Final stats
    let final_stats = backend.stats().await;
    println!("\nFinal Statistics:");
    println!("  Total operations: {}", final_stats.total_operations());
    println!("  Hit rate: {:.2}%", final_stats.hit_rate() * 100.0);

    println!("\n=== Demo Complete ===");

    Ok(())
}
