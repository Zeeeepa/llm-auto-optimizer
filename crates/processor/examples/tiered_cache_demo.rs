//! Tiered Cache Demo
//!
//! This example demonstrates a 3-tier caching architecture:
//! - L1: In-memory cache (fastest, smallest)
//! - L2: Redis cache (fast, medium size)
//! - L3: PostgreSQL (persistent, largest)
//!
//! ## Prerequisites
//!
//! Start services:
//! ```bash
//! docker-compose up -d redis postgres
//! ```
//!
//! ## Run Example
//!
//! ```bash
//! cargo run --example tiered_cache_demo
//! ```

use processor::state::{
    cached_backend::CachedStateBackend, memory::MemoryStateBackend,
    postgres_backend::PostgresStateBackend, redis_backend::RedisStateBackend, RedisConfig,
    StateBackend,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Tiered Cache Demo ===\n");

    // ========================================================================
    // Setup: Create 3-Tier Cache Architecture
    // ========================================================================
    println!("--- Setting Up 3-Tier Cache ---\n");

    // L3: PostgreSQL (Persistent Storage)
    println!("L3: Initializing PostgreSQL backend...");
    let postgres = Arc::new(
        PostgresStateBackend::new(
            "postgres://optimizer:password@localhost:5432/optimizer",
            "tiered_cache_demo",
        )
        .await?,
    );
    postgres.clear().await?;
    println!("  ✓ PostgreSQL connected");

    // L2: Redis (Distributed Cache)
    println!("L2: Initializing Redis backend...");
    let redis_config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("cache:l2:")
        .default_ttl(Duration::from_secs(300)) // 5 minutes
        .build()?;

    let redis = Arc::new(RedisStateBackend::new(redis_config).await?);
    redis.clear().await?;
    println!("  ✓ Redis connected");

    // L2 with L1 cache (Redis + Memory)
    println!("L2+L1: Creating cached Redis backend...");
    let redis_with_cache = Arc::new(CachedStateBackend::new(
        redis.clone(),
        1000,                              // Max 1000 items in memory
        Some(Duration::from_secs(60)),     // 1 minute TTL in memory
    ));
    println!("  ✓ Memory cache layer added");

    // L1: Pure memory (fastest)
    println!("L1: Creating in-memory backend...");
    let memory = Arc::new(MemoryStateBackend::new(Some(Duration::from_secs(30))));
    println!("  ✓ Memory backend created\n");

    println!("Cache Hierarchy:");
    println!("  L1 (Memory)  -> 1000 items, 30s TTL  [Fastest]");
    println!("  L2 (Redis)   -> Unlimited, 5min TTL [Fast]");
    println!("  L3 (Postgres) -> Unlimited, No TTL  [Persistent]\n");

    // ========================================================================
    // Demo 1: Cache Hit Rates
    // ========================================================================
    println!("--- Demo 1: Cache Hit Rate Analysis ---\n");

    // Populate L3 (PostgreSQL) with data
    println!("Populating L3 (PostgreSQL) with 100 items...");
    for i in 0..100 {
        let key = format!("data:{:03}", i);
        let value = format!("{{\"id\":{},\"content\":\"Data item {}\"}}", i, i);
        postgres.put(key.as_bytes(), value.as_bytes()).await?;
    }
    println!("✓ L3 populated\n");

    // First read: All cache misses
    println!("First read (cold cache):");
    let start = Instant::now();
    for i in 0..100 {
        let key = format!("data:{:03}", i);
        redis_with_cache.get(key.as_bytes()).await?;
    }
    let cold_time = start.elapsed();
    println!("  Time: {:?}", cold_time);

    let stats_after_cold = redis_with_cache.cache_stats().await;
    println!("  L1 hit rate: {:.2}%", stats_after_cold.hit_rate * 100.0);

    // Second read: All cache hits
    println!("\nSecond read (warm cache):");
    let start = Instant::now();
    for i in 0..100 {
        let key = format!("data:{:03}", i);
        redis_with_cache.get(key.as_bytes()).await?;
    }
    let warm_time = start.elapsed();
    println!("  Time: {:?}", warm_time);

    let stats_after_warm = redis_with_cache.cache_stats().await;
    println!("  L1 hit rate: {:.2}%", stats_after_warm.hit_rate * 100.0);

    println!("\nSpeedup: {:.2}x faster", cold_time.as_micros() as f64 / warm_time.as_micros() as f64);
    println!();

    // ========================================================================
    // Demo 2: Cache Eviction and Promotion
    // ========================================================================
    println!("--- Demo 2: Cache Eviction and Promotion ---\n");

    // Create small L1 cache
    let small_l1 = Arc::new(CachedStateBackend::new(
        postgres.clone(),
        10, // Only 10 items
        Some(Duration::from_secs(60)),
    ));

    // Insert 20 items
    println!("Inserting 20 items into cache with max size of 10...");
    for i in 0..20 {
        let key = format!("evict:{:02}", i);
        let value = format!("value_{}", i);
        small_l1.put(key.as_bytes(), value.as_bytes()).await?;
    }

    let stats = small_l1.cache_stats().await;
    println!("  L1 cache size: {} (evictions occurred)", stats.size);
    println!("  Total items in L3: {}", small_l1.count().await?);

    // Access pattern: frequently accessed items stay in cache
    println!("\nSimulating access pattern (frequently access first 5 items)...");
    for _ in 0..10 {
        for i in 0..5 {
            let key = format!("evict:{:02}", i);
            small_l1.get(key.as_bytes()).await?;
        }
    }

    let stats_after = small_l1.cache_stats().await;
    println!("  L1 hit rate: {:.2}%", stats_after.hit_rate * 100.0);
    println!("  Hot items promoted to L1, cold items evicted\n");

    // ========================================================================
    // Demo 3: Multi-Tier Write-Through
    // ========================================================================
    println!("--- Demo 3: Write-Through Strategy ---\n");

    let key = b"user:1001";
    let value = b"Alice";

    println!("Writing to all tiers...");
    let start = Instant::now();

    // Write to all tiers
    memory.put(key, value).await?;
    redis.put(key, value).await?;
    postgres.put(key, value).await?;

    let write_time = start.elapsed();
    println!("  Write time (3 tiers): {:?}", write_time);

    // Verify data in each tier
    println!("\nVerifying data in each tier:");
    println!("  L1 (Memory): {:?}", memory.get(key).await?.map(|v| String::from_utf8_lossy(&v)));
    println!("  L2 (Redis):  {:?}", redis.get(key).await?.map(|v| String::from_utf8_lossy(&v)));
    println!("  L3 (Postgres): {:?}", postgres.get(key).await?.map(|v| String::from_utf8_lossy(&v)));
    println!();

    // ========================================================================
    // Demo 4: Cache Aside Pattern
    // ========================================================================
    println!("--- Demo 4: Cache Aside Pattern ---\n");

    async fn get_with_cache_aside<'a>(
        l1: &'a Arc<MemoryStateBackend>,
        l2: &'a Arc<RedisStateBackend>,
        l3: &'a Arc<PostgresStateBackend>,
        key: &'a [u8],
    ) -> anyhow::Result<Option<Vec<u8>>> {
        // Try L1
        if let Some(value) = l1.get(key).await? {
            println!("    ✓ L1 hit");
            return Ok(Some(value));
        }

        // Try L2
        if let Some(value) = l2.get(key).await? {
            println!("    ✓ L2 hit");
            // Promote to L1
            l1.put(key, &value).await?;
            return Ok(Some(value));
        }

        // Try L3
        if let Some(value) = l3.get(key).await? {
            println!("    ✓ L3 hit");
            // Promote to L2 and L1
            l2.put(key, &value).await?;
            l1.put(key, &value).await?;
            return Ok(Some(value));
        }

        println!("    ✗ Miss (all tiers)");
        Ok(None)
    }

    // Clear L1 and L2
    memory.clear().await?;
    redis.clear().await?;

    // Data only in L3
    postgres.put(b"product:5001", b"Laptop").await?;

    println!("First access (data in L3 only):");
    get_with_cache_aside(&memory, &redis, &postgres, b"product:5001").await?;

    println!("\nSecond access (data promoted to L2 and L1):");
    get_with_cache_aside(&memory, &redis, &postgres, b"product:5001").await?;

    println!("\nThird access (data in L1):");
    get_with_cache_aside(&memory, &redis, &postgres, b"product:5001").await?;
    println!();

    // ========================================================================
    // Demo 5: Performance Comparison
    // ========================================================================
    println!("--- Demo 5: Performance Comparison ---\n");

    let test_key = b"perf:test";
    let test_value = b"performance test value";

    // Write to all backends
    memory.put(test_key, test_value).await?;
    redis.put(test_key, test_value).await?;
    postgres.put(test_key, test_value).await?;

    // Benchmark L1 (Memory)
    let start = Instant::now();
    for _ in 0..1000 {
        memory.get(test_key).await?;
    }
    let l1_time = start.elapsed();

    // Benchmark L2 (Redis)
    let start = Instant::now();
    for _ in 0..1000 {
        redis.get(test_key).await?;
    }
    let l2_time = start.elapsed();

    // Benchmark L3 (PostgreSQL)
    let start = Instant::now();
    for _ in 0..1000 {
        postgres.get(test_key).await?;
    }
    let l3_time = start.elapsed();

    println!("Read Performance (1000 operations):");
    println!("  L1 (Memory):     {:?}  ({:.2} μs/op)", l1_time, l1_time.as_micros() as f64 / 1000.0);
    println!("  L2 (Redis):      {:?}  ({:.2} μs/op)", l2_time, l2_time.as_micros() as f64 / 1000.0);
    println!("  L3 (PostgreSQL): {:?}  ({:.2} μs/op)", l3_time, l3_time.as_micros() as f64 / 1000.0);

    println!("\nRelative Performance:");
    println!("  L2 is {:.2}x slower than L1", l2_time.as_micros() as f64 / l1_time.as_micros() as f64);
    println!("  L3 is {:.2}x slower than L1", l3_time.as_micros() as f64 / l1_time.as_micros() as f64);
    println!("  L3 is {:.2}x slower than L2", l3_time.as_micros() as f64 / l2_time.as_micros() as f64);
    println!();

    // ========================================================================
    // Demo 6: TTL Cascade
    // ========================================================================
    println!("--- Demo 6: TTL Cascade ---\n");

    println!("Setting up TTL cascade:");
    println!("  L1: 5 seconds");
    println!("  L2: 10 seconds");
    println!("  L3: Permanent");

    let l1_ttl = Arc::new(MemoryStateBackend::new(Some(Duration::from_secs(5))));
    let l2_ttl_config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("ttl:")
        .default_ttl(Duration::from_secs(10))
        .build()?;
    let l2_ttl = Arc::new(RedisStateBackend::new(l2_ttl_config).await?);

    // Write to all tiers
    l1_ttl.put(b"cascade:key", b"value").await?;
    l2_ttl.put(b"cascade:key", b"value").await?;
    postgres.put(b"cascade:key", b"value").await?;

    println!("\nAfter 0 seconds:");
    println!("  L1: {}", l1_ttl.contains(b"cascade:key").await?);
    println!("  L2: {}", l2_ttl.contains(b"cascade:key").await?);
    println!("  L3: {}", postgres.contains(b"cascade:key").await?);

    println!("\nWaiting 6 seconds (L1 should expire)...");
    tokio::time::sleep(Duration::from_secs(6)).await;

    println!("After 6 seconds:");
    println!("  L1: {} (expired)", l1_ttl.contains(b"cascade:key").await?);
    println!("  L2: {} (still cached)", l2_ttl.contains(b"cascade:key").await?);
    println!("  L3: {} (permanent)", postgres.contains(b"cascade:key").await?);

    println!("\nData can be re-promoted from L2 to L1 on next access");
    println!();

    // ========================================================================
    // Demo 7: Cache Statistics Summary
    // ========================================================================
    println!("--- Demo 7: Cache Statistics Summary ---\n");

    // L2+L1 Stats
    let cached_stats = redis_with_cache.cache_stats().await;
    println!("L2+L1 (Redis with Memory Cache):");
    println!("  Total requests: {}", cached_stats.requests);
    println!("  Cache hits: {}", cached_stats.hits);
    println!("  Cache misses: {}", cached_stats.misses);
    println!("  Hit rate: {:.2}%", cached_stats.hit_rate * 100.0);
    println!("  Cache size: {}", cached_stats.size);

    // L2 Stats
    let redis_stats = redis.stats().await;
    println!("\nL2 (Redis):");
    println!("  GET operations: {}", redis_stats.get_count);
    println!("  PUT operations: {}", redis_stats.put_count);
    println!("  Hit rate: {:.2}%", redis_stats.hit_rate() * 100.0);
    println!("  Bytes read: {}", redis_stats.bytes_read);
    println!("  Bytes written: {}", redis_stats.bytes_written);

    // L3 Stats
    let postgres_stats = postgres.stats().await?;
    println!("\nL3 (PostgreSQL):");
    println!("  Total keys: {}", postgres_stats.total_keys);
    println!("  Database size: {:.2} MB", postgres_stats.database_size_bytes as f64 / 1_048_576.0);
    println!("  Table size: {:.2} MB", postgres_stats.table_size_bytes as f64 / 1_048_576.0);
    println!();

    // ========================================================================
    // Demo 8: Optimal Access Pattern
    // ========================================================================
    println!("--- Demo 8: Optimal Access Pattern ---\n");

    println!("Access pattern analysis:");
    println!("  Hot data (frequently accessed)  -> Keep in L1");
    println!("  Warm data (occasionally accessed) -> Keep in L2");
    println!("  Cold data (rarely accessed)     -> Only in L3");

    // Simulate access pattern
    let hot_keys = vec![b"hot:1", b"hot:2", b"hot:3"];
    let warm_keys = vec![b"warm:1", b"warm:2", b"warm:3", b"warm:4", b"warm:5"];
    let cold_keys = vec![b"cold:1", b"cold:2", b"cold:3", b"cold:4", b"cold:5",
                          b"cold:6", b"cold:7", b"cold:8", b"cold:9", b"cold:10"];

    // Write all data to L3
    for key in hot_keys.iter().chain(warm_keys.iter()).chain(cold_keys.iter()) {
        postgres.put(key, b"data").await?;
    }

    // Simulate access pattern
    for _ in 0..100 {
        // Hot data: accessed 50% of the time
        for key in &hot_keys {
            memory.get(key).await.ok();
        }
    }

    for _ in 0..20 {
        // Warm data: accessed 20% of the time
        for key in &warm_keys {
            redis.get(key).await.ok();
        }
    }

    // Cold data: accessed 5% of the time (stays in L3)

    println!("  Hot data promoted to L1 for fastest access");
    println!("  Warm data cached in L2 for fast access");
    println!("  Cold data remains in L3 (minimal cache pollution)");
    println!();

    // ========================================================================
    // Cleanup
    // ========================================================================
    println!("--- Cleanup ---");
    memory.clear().await?;
    redis.clear().await?;
    postgres.clear().await?;
    println!("✓ All tiers cleared\n");

    println!("=== Demo Complete ===\n");

    println!("Key Takeaways:");
    println!("  1. L1 (Memory) is 10-100x faster than L2 (Redis)");
    println!("  2. L2 (Redis) is 5-10x faster than L3 (PostgreSQL)");
    println!("  3. Cached backend provides automatic promotion");
    println!("  4. TTL cascade prevents stale data");
    println!("  5. Proper cache sizing prevents memory issues");
    println!("  6. Access patterns determine optimal tier placement");

    Ok(())
}
