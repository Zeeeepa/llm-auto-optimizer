# Cached State Backend - Quick Reference Guide

## Quick Start

```rust
use processor::state::{
    CacheConfig, CachedStateBackend, MemoryStateBackend,
    WriteStrategy, EvictionPolicy, StateBackend
};

// Create with default config
let backend = MemoryStateBackend::new(None);
let config = CacheConfig::default();
let cached = CachedStateBackend::new(backend, None, config).await?;

// Basic operations
cached.put(b"key", b"value").await?;
let value = cached.get(b"key").await?;
cached.delete(b"key").await?;
```

## Configuration Options

### Write Strategies

```rust
// Strong consistency (default)
write_strategy: WriteStrategy::WriteThrough

// Low latency writes
write_strategy: WriteStrategy::WriteBack

// Batch writes
write_strategy: WriteStrategy::WriteBehind
```

### Eviction Policies

```rust
eviction_policy: EvictionPolicy::LRU   // Least Recently Used
eviction_policy: EvictionPolicy::LFU   // Least Frequently Used
eviction_policy: EvictionPolicy::FIFO  // First In First Out
```

### Complete Configuration

```rust
let config = CacheConfig {
    l1_capacity: 10_000,                      // 10K entries
    l1_ttl: Duration::from_secs(300),         // 5 minutes
    l2_ttl: Duration::from_secs(3600),        // 1 hour
    write_strategy: WriteStrategy::WriteThrough,
    eviction_policy: EvictionPolicy::LRU,
    enable_stats: true,
    refresh_ahead_threshold: Some(0.8),       // Refresh at 80% TTL
    write_behind_interval: Some(Duration::from_secs(10)),
    write_behind_batch_size: Some(1000),
};
```

## Common Operations

### Basic CRUD

```rust
// Put
cached.put(b"user:123", b"Alice").await?;

// Get
if let Some(value) = cached.get(b"user:123").await? {
    println!("Found: {:?}", value);
}

// Delete
cached.delete(b"user:123").await?;

// Check existence
if cached.contains(b"user:123").await? {
    println!("Key exists");
}
```

### Batch Operations

```rust
// Batch put
let pairs = vec![
    (b"k1".to_vec(), b"v1".to_vec()),
    (b"k2".to_vec(), b"v2".to_vec()),
    (b"k3".to_vec(), b"v3".to_vec()),
];
cached.put_batch(&pairs).await?;

// Batch get
let keys = vec![b"k1".to_vec(), b"k2".to_vec(), b"k3".to_vec()];
let values = cached.get_batch(&keys).await?;
```

### Cache Management

```rust
// Warm cache with hot keys
let hot_keys = vec![b"hot1".to_vec(), b"hot2".to_vec()];
cached.warm_cache(&hot_keys).await?;

// Invalidate specific key
cached.invalidate(b"user:123").await?;

// Clear all caches (keeps backend data)
cached.clear_cache().await?;

// Clear everything (including backend)
cached.clear().await?;
```

### Statistics

```rust
let stats = cached.stats().await;

// Hit rates
println!("L1 hit rate: {:.2}%", stats.l1_hit_rate * 100.0);
println!("L2 hit rate: {:.2}%", stats.l2_hit_rate * 100.0);
println!("Overall: {:.2}%", stats.overall_hit_rate * 100.0);

// Latency
println!("L1 latency: {:.2}μs", stats.l1_avg_latency_us);
println!("L2 latency: {:.2}μs", stats.l2_avg_latency_us);

// Size and evictions
println!("L1 size: {} / {}", stats.l1_size, stats.l1_capacity);
println!("Evictions: {}", stats.l1_evictions);
```

## With Redis (L2 Cache)

```rust
use redis::Client;

let backend = MemoryStateBackend::new(None);
let redis_client = Client::open("redis://127.0.0.1:6379/")?;
let config = CacheConfig::default();

let cached = CachedStateBackend::new(
    backend,
    Some(redis_client),  // Enable L2
    config,
).await?;
```

## Performance Tips

### For Read-Heavy Workloads

```rust
let config = CacheConfig {
    l1_capacity: 100_000,                     // Large L1
    l1_ttl: Duration::from_secs(3600),        // Long TTL
    write_strategy: WriteStrategy::WriteBack,  // Fast writes
    refresh_ahead_threshold: Some(0.9),       // Aggressive refresh
    ..Default::default()
};
```

### For Write-Heavy Workloads

```rust
let config = CacheConfig {
    l1_capacity: 10_000,                      // Smaller L1
    write_strategy: WriteStrategy::WriteBehind,// Batch writes
    write_behind_interval: Some(Duration::from_millis(100)),
    write_behind_batch_size: Some(5000),
    ..Default::default()
};
```

### For Consistency

```rust
let config = CacheConfig {
    write_strategy: WriteStrategy::WriteThrough, // Sync writes
    l1_ttl: Duration::from_secs(60),            // Short TTL
    refresh_ahead_threshold: None,               // No refresh-ahead
    ..Default::default()
};
```

## Monitoring

### Access Patterns

```rust
let stats = cached.stats().await;

// Cache effectiveness
let cache_hit_rate = (stats.l1_hits + stats.l2_hits) as f64
    / (stats.l1_hits + stats.l2_hits + stats.l3_hits) as f64;

// Memory utilization
let l1_utilization = stats.l1_size as f64 / stats.l1_capacity as f64;

// Write efficiency (if using write-behind)
let batch_ratio = stats.writes_batched as f64 / stats.writes_total as f64;
```

### Performance Tracking

```rust
use std::time::Instant;

let start = Instant::now();
cached.get(b"key").await?;
let latency = start.elapsed();

println!("Get latency: {:?}", latency);
```

## Testing

### Unit Tests

```rust
#[tokio::test]
async fn test_cache_hit() {
    let backend = MemoryStateBackend::new(None);
    let config = CacheConfig {
        enable_stats: true,
        ..Default::default()
    };
    let cached = CachedStateBackend::new(backend, None, config).await.unwrap();

    cached.put(b"key", b"value").await.unwrap();
    cached.get(b"key").await.unwrap(); // L1 hit

    let stats = cached.stats().await;
    assert!(stats.l1_hits > 0);
}
```

### Benchmarking

```rust
use criterion::{black_box, criterion_group, Criterion};

fn bench_cache_get(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let cached = rt.block_on(async {
        let backend = MemoryStateBackend::new(None);
        let config = CacheConfig::default();
        let cached = CachedStateBackend::new(backend, None, config).await.unwrap();
        cached.put(b"key", b"value").await.unwrap();
        cached
    });

    c.bench_function("get", |b| {
        b.to_async(&rt).iter(|| async {
            black_box(cached.get(b"key").await.unwrap());
        });
    });
}

criterion_group!(benches, bench_cache_get);
```

## Troubleshooting

### High L3 Hit Rate (Cache Misses)

```rust
// Increase L1 capacity
config.l1_capacity = 100_000;

// Increase TTL
config.l1_ttl = Duration::from_secs(3600);

// Warm cache with hot keys
cached.warm_cache(&hot_keys).await?;
```

### High Eviction Rate

```rust
// Increase capacity
config.l1_capacity = 50_000;

// Change eviction policy
config.eviction_policy = EvictionPolicy::LFU;
```

### Write Latency Issues

```rust
// Use write-back
config.write_strategy = WriteStrategy::WriteBack;

// Or write-behind with tuning
config.write_strategy = WriteStrategy::WriteBehind;
config.write_behind_interval = Some(Duration::from_millis(50));
config.write_behind_batch_size = Some(10000);
```

## Best Practices

1. **Enable statistics** in production for monitoring
2. **Use write-through** for critical data
3. **Use write-back/behind** for high-throughput scenarios
4. **Warm cache** on startup with known hot keys
5. **Monitor hit rates** and adjust capacity
6. **Set appropriate TTLs** based on data staleness tolerance
7. **Use batch operations** for bulk loads
8. **Test eviction policies** for your workload

## Examples Location

- Full examples: `/workspaces/llm-auto-optimizer/crates/processor/examples/cached_state_demo.rs`
- Tests: `/workspaces/llm-auto-optimizer/crates/processor/tests/cached_backend_tests.rs`
- Benchmarks: `/workspaces/llm-auto-optimizer/crates/processor/benches/cached_backend_bench.rs`

## Run Examples

```bash
# Run demo
cargo run -p processor --example cached_state_demo

# Run tests
cargo test -p processor cached_backend

# Run benchmarks
cargo bench -p processor --bench cached_backend_bench
```
