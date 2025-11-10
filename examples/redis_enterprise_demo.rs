//! Enterprise Redis Backend Demo
//!
//! This example demonstrates the advanced features of the enterprise Redis backend.
//!
//! Run with: cargo run --example redis_enterprise_demo
//!
//! Requirements:
//! - Redis server running on localhost:6379

use processor::state::{
    BackendStats, CompressionAlgorithm, EnterpriseRedisBackend, EnterpriseRedisConfig,
    RedisMode, StateBackend,
};
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== Enterprise Redis Backend Demo ===\n");

    // Demo 1: Basic Operations with Compression
    demo_compression().await?;

    // Demo 2: Batch Operations Performance
    demo_batch_operations().await?;

    // Demo 3: Distributed Locking
    demo_distributed_locking().await?;

    // Demo 4: Statistics and Monitoring
    demo_statistics().await?;

    // Demo 5: Error Handling and Resilience
    demo_resilience().await?;

    println!("\n=== All Demos Completed ===");
    Ok(())
}

async fn demo_compression() -> anyhow::Result<()> {
    println!("=== Demo 1: Compression ===");

    // Create backend with LZ4 compression
    let config = EnterpriseRedisConfig::builder()
        .mode(RedisMode::Standalone {
            url: "redis://localhost:6379".to_string(),
        })
        .key_prefix("demo:compression:")
        .compression(CompressionAlgorithm::Lz4)
        .compression_threshold(100)
        .enable_metrics(true)
        .build()?;

    let backend = EnterpriseRedisBackend::new(config).await?;
    backend.clear().await?;

    // Test with small data (won't be compressed)
    let small_data = b"Small data";
    backend.put(b"small", small_data).await?;
    println!("Stored small data: {} bytes", small_data.len());

    // Test with large data (will be compressed)
    let large_data = vec![b'x'; 10000];
    backend.put(b"large", &large_data).await?;
    println!("Stored large data: {} bytes", large_data.len());

    // Retrieve and verify
    let retrieved = backend.get(b"large").await?;
    assert_eq!(retrieved, Some(large_data));
    println!("Retrieved and verified large data");

    // Show compression stats
    let stats = backend.stats().await;
    if stats.bytes_compressed > 0 {
        let ratio = stats.bytes_written as f64 / stats.bytes_compressed as f64;
        println!("Compression ratio: {:.2}x", ratio);
        println!("Saved: {} bytes\n", stats.bytes_written - stats.bytes_compressed);
    }

    Ok(())
}

async fn demo_batch_operations() -> anyhow::Result<()> {
    println!("=== Demo 2: Batch Operations Performance ===");

    let config = EnterpriseRedisConfig::builder()
        .mode(RedisMode::Standalone {
            url: "redis://localhost:6379".to_string(),
        })
        .key_prefix("demo:batch:")
        .compression(CompressionAlgorithm::Lz4)
        .pipeline_batch_size(100)
        .build()?;

    let backend = EnterpriseRedisBackend::new(config).await?;
    backend.clear().await?;

    let num_items = 1000;

    // Benchmark individual operations
    println!("Testing individual operations...");
    let start = Instant::now();
    for i in 0..num_items {
        let key = format!("individual_{}", i);
        let value = format!("value_{}", i);
        backend.put(key.as_bytes(), value.as_bytes()).await?;
    }
    let individual_time = start.elapsed();
    println!("Individual PUT {} items: {:?}", num_items, individual_time);

    backend.clear().await?;

    // Benchmark batch operations
    println!("Testing batch operations...");
    let mut pairs = Vec::new();
    for i in 0..num_items {
        let key = format!("batch_{}", i).into_bytes();
        let value = format!("value_{}", i).into_bytes();
        pairs.push((key, value));
    }

    let pair_refs: Vec<(&[u8], &[u8])> = pairs
        .iter()
        .map(|(k, v)| (k.as_slice(), v.as_slice()))
        .collect();

    let start = Instant::now();
    backend.batch_put_optimized(&pair_refs).await?;
    let batch_time = start.elapsed();
    println!("Batch PUT {} items: {:?}", num_items, batch_time);

    // Calculate speedup
    let speedup = individual_time.as_secs_f64() / batch_time.as_secs_f64();
    println!("Batch operations are {:.2}x faster\n", speedup);

    // Test batch get
    let keys: Vec<&[u8]> = pairs.iter().map(|(k, _)| k.as_slice()).collect();
    let start = Instant::now();
    let values = backend.batch_get_optimized(&keys).await?;
    let batch_get_time = start.elapsed();

    println!("Batch GET {} items: {:?}", num_items, batch_get_time);
    println!(
        "Retrieved {} values, all present: {}\n",
        values.len(),
        values.iter().all(|v| v.is_some())
    );

    Ok(())
}

async fn demo_distributed_locking() -> anyhow::Result<()> {
    println!("=== Demo 3: Distributed Locking ===");

    let config = EnterpriseRedisConfig::builder()
        .mode(RedisMode::Standalone {
            url: "redis://localhost:6379".to_string(),
        })
        .key_prefix("demo:lock:")
        .build()?;

    let backend = EnterpriseRedisBackend::new(config).await?;

    let resource = "critical_resource";

    // Acquire lock
    println!("Acquiring lock on '{}'...", resource);
    let guard = backend
        .acquire_lock(resource, Duration::from_secs(10))
        .await?;
    println!("✓ Lock acquired");

    // Try to acquire same lock (should fail)
    println!("Attempting to acquire same lock...");
    let result = backend
        .try_acquire_lock(resource, Duration::from_secs(10))
        .await?;

    if result.is_none() {
        println!("✓ Lock acquisition failed as expected (already held)");
    }

    // Simulate critical section work
    println!("Performing critical operation...");
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("✓ Critical operation completed");

    // Release lock
    println!("Releasing lock...");
    drop(guard);
    tokio::time::sleep(Duration::from_millis(100)).await; // Let async drop complete

    // Now we should be able to acquire
    println!("Attempting to acquire lock again...");
    let guard2 = backend
        .try_acquire_lock(resource, Duration::from_secs(10))
        .await?;

    if guard2.is_some() {
        println!("✓ Lock re-acquired successfully\n");
    }

    Ok(())
}

async fn demo_statistics() -> anyhow::Result<()> {
    println!("=== Demo 4: Statistics and Monitoring ===");

    let config = EnterpriseRedisConfig::builder()
        .mode(RedisMode::Standalone {
            url: "redis://localhost:6379".to_string(),
        })
        .key_prefix("demo:stats:")
        .compression(CompressionAlgorithm::Lz4)
        .enable_metrics(true)
        .enable_slow_query_log(true)
        .slow_query_threshold(Duration::from_millis(50))
        .build()?;

    let backend = EnterpriseRedisBackend::new(config).await?;
    backend.clear().await?;

    // Perform various operations
    println!("Performing operations for statistics...");
    for i in 0..100 {
        let key = format!("stat_key_{}", i);
        let value = format!("value_{}", i).repeat(10);
        backend.put(key.as_bytes(), value.as_bytes()).await?;
    }

    // Generate some cache hits and misses
    for i in 0..50 {
        let key = format!("stat_key_{}", i);
        backend.get(key.as_bytes()).await?; // Hit
    }

    for i in 100..110 {
        let key = format!("nonexistent_{}", i);
        backend.get(key.as_bytes()).await?; // Miss
    }

    // Get and display statistics
    let stats = backend.stats().await;

    println!("\n--- Backend Statistics ---");
    println!("Total operations: {}", stats.operations_total);
    println!("Successful: {}", stats.operations_success);
    println!("Failed: {}", stats.operations_failed);

    let success_rate = if stats.operations_total > 0 {
        stats.operations_success as f64 / stats.operations_total as f64 * 100.0
    } else {
        0.0
    };
    println!("Success rate: {:.2}%", success_rate);

    println!("\nCache Performance:");
    println!("  Hits: {}", stats.cache_hits);
    println!("  Misses: {}", stats.cache_misses);

    let hit_rate = if stats.cache_hits + stats.cache_misses > 0 {
        stats.cache_hits as f64 / (stats.cache_hits + stats.cache_misses) as f64 * 100.0
    } else {
        0.0
    };
    println!("  Hit rate: {:.2}%", hit_rate);

    println!("\nData Transfer:");
    println!("  Bytes written: {}", stats.bytes_written);
    println!("  Bytes read: {}", stats.bytes_read);
    println!("  Bytes compressed: {}", stats.bytes_compressed);

    if stats.bytes_compressed > 0 {
        let compression_ratio = stats.bytes_written as f64 / stats.bytes_compressed as f64;
        println!("  Compression ratio: {:.2}x", compression_ratio);
    }

    println!("\nPerformance:");
    println!("  Average latency: {}µs", stats.avg_latency_us);
    println!("  Slow queries: {}", stats.slow_queries);

    // Health check
    println!("\nHealth Check:");
    let healthy = backend.health_check().await?;
    println!("  Status: {}", if healthy { "✓ Healthy" } else { "✗ Unhealthy" });

    println!();
    Ok(())
}

async fn demo_resilience() -> anyhow::Result<()> {
    println!("=== Demo 5: Error Handling and Resilience ===");

    let config = EnterpriseRedisConfig::builder()
        .mode(RedisMode::Standalone {
            url: "redis://localhost:6379".to_string(),
        })
        .key_prefix("demo:resilience:")
        .retry_config(
            3,
            Duration::from_millis(100),
            Duration::from_secs(5),
            0.3, // 30% jitter
        )
        .enable_circuit_breaker(true)
        .circuit_breaker(true, 10, Duration::from_secs(60))
        .build()?;

    let backend = EnterpriseRedisBackend::new(config).await?;

    println!("Configuration:");
    println!("  Max retries: 3");
    println!("  Retry delay: 100ms - 5s with 30% jitter");
    println!("  Circuit breaker: Enabled (threshold: 10 failures)");

    // Normal operations
    println!("\nPerforming normal operations...");
    backend.put(b"test_key", b"test_value").await?;
    let value = backend.get(b"test_key").await?;
    println!("✓ Operations successful: {:?}", value);

    // Demonstrate retry logic
    println!("\nRetry logic is handled automatically for transient failures");
    println!("The backend will retry failed operations with exponential backoff");

    // Demonstrate error handling
    println!("\nError handling example:");
    match backend.get(b"some_key").await {
        Ok(Some(value)) => println!("  Found value: {:?}", value),
        Ok(None) => println!("  Key not found (this is not an error)"),
        Err(e) => println!("  Error occurred: {}", e),
    }

    println!("\n✓ Resilience features demonstrated\n");

    Ok(())
}
