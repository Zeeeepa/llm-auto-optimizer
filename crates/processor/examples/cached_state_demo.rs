//! Demonstration of the tiered caching state backend
//!
//! This example shows how to use the CachedStateBackend with different
//! configurations and strategies.

use processor::state::{
    CacheConfig, CachedStateBackend, EvictionPolicy, MemoryStateBackend, StateBackend,
    WriteStrategy,
};
use std::time::Duration;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("=== Cached State Backend Demo ===\n");

    // Demo 1: Basic usage with write-through strategy
    demo_write_through().await?;

    // Demo 2: Write-back strategy for high throughput
    demo_write_back().await?;

    // Demo 3: Cache warming for frequently accessed keys
    demo_cache_warming().await?;

    // Demo 4: Batch operations
    demo_batch_operations().await?;

    // Demo 5: Cache statistics monitoring
    demo_statistics().await?;

    // Demo 6: Different eviction policies
    demo_eviction_policies().await?;

    // Demo 7: Cache invalidation
    demo_invalidation().await?;

    info!("\n=== Demo Complete ===");

    Ok(())
}

/// Demo 1: Write-through strategy
async fn demo_write_through() -> anyhow::Result<()> {
    info!("--- Demo 1: Write-Through Strategy ---");

    let backend = MemoryStateBackend::new(None);
    let config = CacheConfig {
        write_strategy: WriteStrategy::WriteThrough,
        l1_capacity: 1000,
        l1_ttl: Duration::from_secs(300),
        l2_ttl: Duration::from_secs(3600),
        enable_stats: true,
        ..Default::default()
    };

    let cached = CachedStateBackend::new(backend.clone(), None, config).await?;

    info!("Writing data with write-through strategy...");
    cached.put(b"user:123", b"Alice").await?;
    cached.put(b"user:456", b"Bob").await?;

    // Data is immediately in backend
    let backend_value = backend.get(b"user:123").await?;
    info!("Backend contains: {:?}", backend_value);

    // Subsequent reads are from L1 cache (fast)
    let value = cached.get(b"user:123").await?;
    info!("Retrieved from cache: {:?}", value);

    let stats = cached.stats().await;
    info!(
        "L1 hit rate: {:.2}%, Avg latency: {:.2}μs\n",
        stats.l1_hit_rate * 100.0,
        stats.l1_avg_latency_us
    );

    Ok(())
}

/// Demo 2: Write-back strategy
async fn demo_write_back() -> anyhow::Result<()> {
    info!("--- Demo 2: Write-Back Strategy ---");

    let backend = MemoryStateBackend::new(None);
    let config = CacheConfig {
        write_strategy: WriteStrategy::WriteBack,
        l1_capacity: 1000,
        enable_stats: true,
        ..Default::default()
    };

    let cached = CachedStateBackend::new(backend.clone(), None, config).await?;

    info!("Writing data with write-back strategy...");

    // Writes are fast (only to cache)
    let start = std::time::Instant::now();
    for i in 0..100 {
        let key = format!("session:{}", i);
        let value = format!("session_data_{}", i);
        cached.put(key.as_bytes(), value.as_bytes()).await?;
    }
    let elapsed = start.elapsed();

    info!("Wrote 100 entries in {:?}", elapsed);

    // Give time for async writes to complete
    tokio::time::sleep(Duration::from_millis(200)).await;

    let backend_count = backend.count().await?;
    info!("Backend now contains {} entries", backend_count);

    let stats = cached.stats().await;
    info!(
        "Write throughput: {:.0} ops/sec\n",
        100.0 / elapsed.as_secs_f64()
    );

    Ok(())
}

/// Demo 3: Cache warming
async fn demo_cache_warming() -> anyhow::Result<()> {
    info!("--- Demo 3: Cache Warming ---");

    let backend = MemoryStateBackend::new(None);

    // Prepopulate backend with data
    info!("Prepopulating backend with 1000 entries...");
    for i in 0..1000 {
        let key = format!("product:{}", i);
        let value = format!("product_data_{}", i);
        backend.put(key.as_bytes(), value.as_bytes()).await?;
    }

    let config = CacheConfig {
        l1_capacity: 100, // Small cache
        enable_stats: true,
        ..Default::default()
    };

    let cached = CachedStateBackend::new(backend, None, config).await?;

    // Identify hot keys (e.g., top 10 products)
    let hot_keys: Vec<Vec<u8>> = (0..10)
        .map(|i| format!("product:{}", i).into_bytes())
        .collect();

    info!("Warming cache with {} hot keys...", hot_keys.len());
    cached.warm_cache(&hot_keys).await?;

    // Access hot keys - should be L1 hits
    let start = std::time::Instant::now();
    for key in &hot_keys {
        cached.get(key).await?;
    }
    let hot_elapsed = start.elapsed();

    // Access cold keys - will be L3 hits
    let start = std::time::Instant::now();
    for i in 90..100 {
        let key = format!("product:{}", i);
        cached.get(key.as_bytes()).await?;
    }
    let cold_elapsed = start.elapsed();

    let stats = cached.stats().await;
    info!(
        "Hot keys (L1): {:?}, Cold keys (L3): {:?}",
        hot_elapsed, cold_elapsed
    );
    info!(
        "L1 hits: {}, L3 hits: {}\n",
        stats.l1_hits, stats.l3_hits
    );

    Ok(())
}

/// Demo 4: Batch operations
async fn demo_batch_operations() -> anyhow::Result<()> {
    info!("--- Demo 4: Batch Operations ---");

    let backend = MemoryStateBackend::new(None);
    let config = CacheConfig {
        l1_capacity: 10000,
        enable_stats: true,
        ..Default::default()
    };

    let cached = CachedStateBackend::new(backend, None, config).await?;

    // Batch put
    info!("Performing batch put...");
    let pairs: Vec<_> = (0..1000)
        .map(|i| {
            (
                format!("key:{}", i).into_bytes(),
                format!("value:{}", i).into_bytes(),
            )
        })
        .collect();

    let start = std::time::Instant::now();
    cached.put_batch(&pairs).await?;
    let put_elapsed = start.elapsed();

    // Batch get
    info!("Performing batch get...");
    let keys: Vec<_> = (0..1000)
        .map(|i| format!("key:{}", i).into_bytes())
        .collect();

    let start = std::time::Instant::now();
    let values = cached.get_batch(&keys).await?;
    let get_elapsed = start.elapsed();

    info!(
        "Batch put: {:?} ({:.0} ops/sec)",
        put_elapsed,
        1000.0 / put_elapsed.as_secs_f64()
    );
    info!(
        "Batch get: {:?} ({:.0} ops/sec)",
        get_elapsed,
        1000.0 / get_elapsed.as_secs_f64()
    );
    info!("Retrieved {} values\n", values.len());

    Ok(())
}

/// Demo 5: Statistics monitoring
async fn demo_statistics() -> anyhow::Result<()> {
    info!("--- Demo 5: Statistics Monitoring ---");

    let backend = MemoryStateBackend::new(None);

    // Prepopulate backend
    for i in 0..100 {
        let key = format!("metric:{}", i);
        let value = format!("data:{}", i);
        backend.put(key.as_bytes(), value.as_bytes()).await?;
    }

    let config = CacheConfig {
        l1_capacity: 50, // Half the data fits in cache
        enable_stats: true,
        ..Default::default()
    };

    let cached = CachedStateBackend::new(backend, None, config).await?;

    // Simulate workload: access keys in different patterns
    info!("Simulating workload...");

    // Hot keys (0-20) - accessed multiple times
    for _ in 0..5 {
        for i in 0..20 {
            let key = format!("metric:{}", i);
            cached.get(key.as_bytes()).await?;
        }
    }

    // Warm keys (20-50) - accessed once
    for i in 20..50 {
        let key = format!("metric:{}", i);
        cached.get(key.as_bytes()).await?;
    }

    // Cold keys (50-100) - never accessed
    // (no access)

    let stats = cached.stats().await;

    info!("\n=== Cache Statistics ===");
    info!("L1 Cache:");
    info!("  Hits: {}", stats.l1_hits);
    info!("  Misses: {}", stats.l1_misses);
    info!("  Hit Rate: {:.2}%", stats.l1_hit_rate * 100.0);
    info!("  Avg Latency: {:.2}μs", stats.l1_avg_latency_us);
    info!("  Size: {} / {}", stats.l1_size, stats.l1_capacity);

    info!("\nL3 Backend:");
    info!("  Hits: {}", stats.l3_hits);
    info!("  Misses: {}", stats.l3_misses);
    info!("  Hit Rate: {:.2}%", stats.l3_hit_rate * 100.0);
    info!("  Avg Latency: {:.2}μs", stats.l3_avg_latency_us);

    info!("\nOverall:");
    info!("  Hit Rate: {:.2}%", stats.overall_hit_rate * 100.0);
    info!("  Total Writes: {}", stats.writes_total);
    info!("");

    Ok(())
}

/// Demo 6: Eviction policies
async fn demo_eviction_policies() -> anyhow::Result<()> {
    info!("--- Demo 6: Eviction Policies ---");

    for policy in [EvictionPolicy::LRU, EvictionPolicy::LFU, EvictionPolicy::FIFO].iter() {
        info!("\nTesting {:?} policy:", policy);

        let backend = MemoryStateBackend::new(None);
        let config = CacheConfig {
            l1_capacity: 10,
            eviction_policy: *policy,
            enable_stats: true,
            ..Default::default()
        };

        let cached = CachedStateBackend::new(backend, None, config).await?;

        // Fill cache to capacity
        for i in 0..10 {
            let key = format!("item:{}", i);
            let value = format!("data:{}", i);
            cached.put(key.as_bytes(), value.as_bytes()).await?;
        }

        // Access a few items to affect eviction
        cached.get(b"item:0").await?;
        cached.get(b"item:1").await?;
        cached.get(b"item:0").await?; // item:0 accessed twice

        // Insert new item (will trigger eviction)
        cached.put(b"item:new", b"new_data").await?;

        let stats = cached.stats().await;
        info!("  Cache size: {}", stats.l1_size);
        info!("  Evictions: {}", stats.l1_evictions);
    }

    info!("");
    Ok(())
}

/// Demo 7: Cache invalidation
async fn demo_invalidation() -> anyhow::Result<()> {
    info!("--- Demo 7: Cache Invalidation ---");

    let backend = MemoryStateBackend::new(None);
    let config = CacheConfig {
        l1_capacity: 100,
        enable_stats: true,
        ..Default::default()
    };

    let cached = CachedStateBackend::new(backend.clone(), None, config).await?;

    // Add data
    info!("Adding data to cache...");
    for i in 0..10 {
        let key = format!("config:{}", i);
        let value = format!("value:{}", i);
        cached.put(key.as_bytes(), value.as_bytes()).await?;
    }

    // Verify cached
    cached.get(b"config:5").await?;
    let stats = cached.stats().await;
    info!("L1 hits before invalidation: {}", stats.l1_hits);

    // Invalidate specific key (e.g., config changed)
    info!("Invalidating config:5...");
    cached.invalidate(b"config:5").await?;

    // Next access will be L3 hit
    cached.get(b"config:5").await?;
    let stats = cached.stats().await;
    info!("L3 hits after invalidation: {}", stats.l3_hits);

    // Clear entire cache
    info!("Clearing entire cache...");
    cached.clear_cache().await?;

    // All accesses will be L3 hits
    for i in 0..10 {
        let key = format!("config:{}", i);
        cached.get(key.as_bytes()).await?;
    }

    let stats = cached.stats().await;
    info!("L3 hits after cache clear: {}", stats.l3_hits);
    info!("Cache still contains data in backend: {}", backend.count().await?);
    info!("");

    Ok(())
}
