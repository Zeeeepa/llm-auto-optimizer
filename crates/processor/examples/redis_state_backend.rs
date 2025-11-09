//! Redis State Backend Examples
//!
//! This example demonstrates various features of the Redis state backend including:
//! - Basic CRUD operations
//! - Batch operations
//! - TTL management
//! - Atomic operations
//! - Checkpointing and restoration
//! - Statistics and monitoring
//!
//! ## Prerequisites
//!
//! Redis must be running on localhost:6379. You can start it with Docker:
//!
//! ```bash
//! docker run -d -p 6379:6379 redis:7-alpine
//! ```
//!
//! ## Running
//!
//! ```bash
//! cargo run --example redis_state_backend
//! ```

use processor::state::{RedisConfig, RedisStateBackend, StateBackend};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("info,redis_state_backend=debug")
        .init();

    println!("=== Redis State Backend Example ===\n");

    // Example 1: Basic Configuration and Connection
    println!("1. Creating Redis backend...");
    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("example:")
        .default_ttl(Duration::from_secs(3600))
        .pool_size(5, 20)
        .retries(3, Duration::from_millis(100), Duration::from_secs(5))
        .build()?;

    let backend = RedisStateBackend::new(config).await?;
    println!("✓ Connected to Redis\n");

    // Example 2: Health Check
    println!("2. Performing health check...");
    let healthy = backend.health_check().await?;
    println!("✓ Health check: {}\n", if healthy { "HEALTHY" } else { "UNHEALTHY" });

    // Example 3: Basic CRUD Operations
    println!("3. Basic CRUD operations...");

    // Put
    backend.put(b"user:1", b"John Doe").await?;
    backend.put(b"user:2", b"Jane Smith").await?;
    println!("✓ Stored 2 users");

    // Get
    if let Some(value) = backend.get(b"user:1").await? {
        println!("✓ Retrieved user:1 = {:?}", String::from_utf8_lossy(&value));
    }

    // Contains
    let exists = backend.contains(b"user:1").await?;
    println!("✓ user:1 exists: {}", exists);

    // Count
    let count = backend.count().await?;
    println!("✓ Total keys: {}\n", count);

    // Example 4: Batch Operations
    println!("4. Batch operations...");

    // Batch put
    let batch_data = vec![
        (&b"product:1"[..], &b"Laptop"[..]),
        (&b"product:2"[..], &b"Mouse"[..]),
        (&b"product:3"[..], &b"Keyboard"[..]),
        (&b"product:4"[..], &b"Monitor"[..]),
        (&b"product:5"[..], &b"Headphones"[..]),
    ];
    backend.batch_put(&batch_data).await?;
    println!("✓ Batch stored {} products", batch_data.len());

    // Batch get
    let product_keys = vec![b"product:1", b"product:2", b"product:3"];
    let products = backend.batch_get(&product_keys).await?;
    println!("✓ Batch retrieved {} products:", products.len());
    for (i, product) in products.iter().enumerate() {
        if let Some(p) = product {
            println!("  - product:{} = {:?}", i + 1, String::from_utf8_lossy(p));
        }
    }
    println!();

    // Example 5: TTL Management
    println!("5. TTL management...");

    // Put with custom TTL (5 seconds)
    backend.put_with_ttl(b"session:123", b"active", Duration::from_secs(5)).await?;
    println!("✓ Created session with 5 second TTL");

    // Check TTL
    if let Some(ttl) = backend.ttl(b"session:123").await? {
        println!("✓ Session TTL: {:?}", ttl);
    }

    // Verify it exists
    assert!(backend.contains(b"session:123").await?);
    println!("✓ Session exists");

    // Wait for expiration
    println!("  Waiting for expiration...");
    tokio::time::sleep(Duration::from_secs(6)).await;

    // Verify it's gone
    assert!(!backend.contains(b"session:123").await?);
    println!("✓ Session expired\n");

    // Example 6: Atomic Operations
    println!("6. Atomic operations...");

    // Set if not exists
    backend.put(b"counter", b"10").await?;

    let set1 = backend.set_if_not_exists(b"lock:resource", b"locked", Duration::from_secs(60)).await?;
    println!("✓ First lock attempt: {}", if set1 { "SUCCESS" } else { "FAILED" });

    let set2 = backend.set_if_not_exists(b"lock:resource", b"locked", Duration::from_secs(60)).await?;
    println!("✓ Second lock attempt: {}", if set2 { "SUCCESS" } else { "FAILED" });

    // Get and delete atomically
    let value = backend.get_and_delete(b"counter").await?;
    println!("✓ Get and delete counter: {:?}", value.map(|v| String::from_utf8_lossy(&v).to_string()));
    assert!(!backend.contains(b"counter").await?);
    println!("✓ Counter deleted\n");

    // Example 7: List Keys with Prefix
    println!("7. List keys with prefix...");

    let product_keys = backend.list_keys(b"product:").await?;
    println!("✓ Found {} product keys:", product_keys.len());
    for key in &product_keys[..3.min(product_keys.len())] {
        println!("  - {:?}", String::from_utf8_lossy(key));
    }
    println!();

    // Example 8: Checkpointing
    println!("8. Checkpointing and restoration...");

    // Create checkpoint
    let checkpoint = backend.checkpoint().await?;
    println!("✓ Created checkpoint with {} entries", checkpoint.len());

    // Modify some data
    backend.put(b"product:6", b"Webcam").await?;
    println!("✓ Added new product");

    let count_before = backend.count().await?;
    println!("✓ Keys before restore: {}", count_before);

    // Clear and restore
    backend.clear().await?;
    println!("✓ Cleared all data");

    backend.restore(checkpoint).await?;
    let count_after = backend.count().await?;
    println!("✓ Restored from checkpoint");
    println!("✓ Keys after restore: {}\n", count_after);

    // Example 9: Statistics
    println!("9. Statistics...");

    let stats = backend.stats().await;
    println!("✓ Operations statistics:");
    println!("  - Total operations: {}", stats.total_operations());
    println!("  - GET operations: {}", stats.get_count);
    println!("  - PUT operations: {}", stats.put_count);
    println!("  - DELETE operations: {}", stats.delete_count);
    println!("  - Cache hits: {}", stats.hit_count);
    println!("  - Cache misses: {}", stats.miss_count);
    println!("  - Hit rate: {:.2}%", stats.hit_rate() * 100.0);
    println!("  - Pipeline operations: {}", stats.pipeline_count);
    println!("  - Retry attempts: {}", stats.retry_count);
    println!("  - Error count: {}", stats.error_count);
    println!("  - Bytes read: {}", stats.bytes_read);
    println!("  - Bytes written: {}", stats.bytes_written);
    println!("  - Avg latency: {}µs\n", stats.avg_latency_us);

    // Example 10: Memory Usage
    println!("10. Memory usage...");
    let memory = backend.memory_usage().await?;
    println!("✓ Redis memory usage: {} bytes ({:.2} MB)\n", memory, memory as f64 / 1024.0 / 1024.0);

    // Example 11: Server Info
    println!("11. Server information...");
    let info = backend.server_info().await?;
    println!("✓ Redis server info (first 200 chars):");
    println!("{}\n", &info[..200.min(info.len())]);

    // Cleanup
    println!("Cleaning up...");
    backend.clear().await?;
    println!("✓ All data cleared");

    println!("\n=== Example Complete ===");

    Ok(())
}
