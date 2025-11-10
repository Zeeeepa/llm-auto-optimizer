# Enterprise Redis Backend - Quick Start Guide

## 🚀 Quick Start

Get up and running with the Enterprise Redis Backend in 5 minutes.

## Prerequisites

- Rust 1.75 or later
- Redis 6.0+ running locally or remotely
- (Optional) Redis Sentinel for HA setup

## Installation

The enterprise backend is already integrated into the processor crate. No additional installation needed.

## Basic Usage

### 1. Create Configuration

```rust
use processor::state::{
    EnterpriseRedisBackend, EnterpriseRedisConfig,
    RedisMode, CompressionAlgorithm, StateBackend,
};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Configure with sensible defaults
    let config = EnterpriseRedisConfig::builder()
        .mode(RedisMode::Standalone {
            url: "redis://localhost:6379".to_string(),
        })
        .key_prefix("myapp:")
        .compression(CompressionAlgorithm::Lz4)
        .enable_metrics(true)
        .build()?;

    let backend = EnterpriseRedisBackend::new(config).await?;

    // Ready to use!
    Ok(())
}
```

### 2. Basic Operations

```rust
// Store data
backend.put(b"user:123", b"user_data").await?;

// Retrieve data
if let Some(data) = backend.get(b"user:123").await? {
    println!("Found: {:?}", data);
}

// Delete data
backend.delete(b"user:123").await?;

// Check existence
if backend.contains(b"user:123").await? {
    println!("Key exists");
}
```

### 3. Batch Operations (Recommended)

```rust
// Batch write - 10-20x faster than individual operations
let pairs = vec![
    (b"key1".as_ref(), b"value1".as_ref()),
    (b"key2".as_ref(), b"value2".as_ref()),
    (b"key3".as_ref(), b"value3".as_ref()),
];
backend.batch_put_optimized(&pairs).await?;

// Batch read
let keys = vec![b"key1".as_ref(), b"key2".as_ref()];
let values = backend.batch_get_optimized(&keys).await?;
```

### 4. Distributed Locking

```rust
// Coordinate across multiple instances
let guard = backend.acquire_lock(
    "checkpoint",
    Duration::from_secs(30)
).await?;

// Critical section - only one instance can execute
perform_critical_operation().await?;

// Lock automatically released when guard drops
drop(guard);
```

### 5. Monitoring

```rust
// Get statistics
let stats = backend.stats().await;
println!("Operations: {}", stats.operations_total);
println!("Hit rate: {:.2}%",
    stats.cache_hits as f64 / (stats.cache_hits + stats.cache_misses) as f64 * 100.0);

// Health check
if backend.health_check().await? {
    println!("✓ Backend healthy");
}
```

## Configuration Presets

### Development (Single Instance)

```rust
let config = EnterpriseRedisConfig::builder()
    .mode(RedisMode::Standalone {
        url: "redis://localhost:6379".to_string(),
    })
    .key_prefix("dev:")
    .compression(CompressionAlgorithm::None)
    .enable_metrics(false)
    .build()?;
```

### Production (High Availability)

```rust
let config = EnterpriseRedisConfig::builder()
    .mode(RedisMode::Sentinel {
        sentinels: vec![
            "redis://sentinel1:26379".to_string(),
            "redis://sentinel2:26379".to_string(),
            "redis://sentinel3:26379".to_string(),
        ],
        service_name: "mymaster".to_string(),
    })
    .key_prefix("prod:")
    .default_ttl(Duration::from_secs(3600))
    .pool_config(20, 200, Duration::from_secs(30))
    .compression(CompressionAlgorithm::Lz4)
    .enable_metrics(true)
    .build()?;
```

### High Performance (Optimized)

```rust
let config = EnterpriseRedisConfig::builder()
    .mode(RedisMode::Standalone {
        url: "redis://localhost:6379".to_string(),
    })
    .pool_config(50, 500, Duration::from_secs(30))
    .compression(CompressionAlgorithm::Lz4)
    .compression_threshold(512)
    .pipeline_batch_size(500)
    .enable_circuit_breaker(true)
    .build()?;
```

## Common Patterns

### Pattern 1: Session Storage

```rust
// Store session with TTL
backend.put_with_ttl(
    b"session:abc123",
    b"session_data",
    Duration::from_secs(1800)  // 30 minutes
).await?;
```

### Pattern 2: Coordinated Checkpoint

```rust
async fn checkpoint(backend: &EnterpriseRedisBackend) -> anyhow::Result<()> {
    if let Some(guard) = backend.try_acquire_lock(
        "checkpoint",
        Duration::from_secs(300)
    ).await? {
        // Only this instance creates checkpoint
        create_checkpoint(backend).await?;
        drop(guard);
    }
    Ok(())
}
```

### Pattern 3: Bulk Data Processing

```rust
async fn process_batch(backend: &EnterpriseRedisBackend, items: Vec<Item>) -> anyhow::Result<()> {
    // Convert to pairs
    let pairs: Vec<(&[u8], &[u8])> = items
        .iter()
        .map(|item| (item.key.as_bytes(), item.value.as_bytes()))
        .collect();

    // Write in one go
    backend.batch_put_optimized(&pairs).await?;
    Ok(())
}
```

### Pattern 4: Cache with TTL

```rust
async fn get_or_compute(
    backend: &EnterpriseRedisBackend,
    key: &[u8],
) -> anyhow::Result<Vec<u8>> {
    // Try cache first
    if let Some(cached) = backend.get(key).await? {
        return Ok(cached);
    }

    // Compute value
    let value = expensive_computation().await?;

    // Cache for 1 hour
    backend.put_with_ttl(key, &value, Duration::from_secs(3600)).await?;

    Ok(value)
}
```

## Troubleshooting

### Connection Failed

```
Error: Failed to connect to Redis
```

**Solution:**
```rust
// Increase timeouts
.timeouts(
    Duration::from_secs(10),  // connect
    Duration::from_secs(5)    // command
)
```

### High Latency

```
Warning: Slow query detected: get took 150ms
```

**Solution:**
```rust
// Use batch operations
backend.batch_get_optimized(&keys).await?;

// Enable compression for large values
.compression(CompressionAlgorithm::Lz4)
```

### Memory Issues

```
Warning: Compression ratio low
```

**Solution:**
```rust
// Set appropriate TTLs
.default_ttl(Duration::from_secs(1800))

// Adjust compression threshold
.compression_threshold(512)
```

## Next Steps

1. **Read Full Documentation**
   - [Complete Guide](./redis-enterprise-backend.md)
   - [Implementation Details](./REDIS_ENTERPRISE_IMPLEMENTATION.md)

2. **Run Examples**
   ```bash
   cargo run --example redis_enterprise_demo
   ```

3. **Run Tests**
   ```bash
   cargo test --test redis_enterprise_integration -- --ignored
   ```

4. **Set Up Monitoring**
   - Configure Prometheus metrics export
   - Set up alerting for error rates
   - Monitor compression ratios

## Performance Tips

1. **Use Batch Operations** - 10-20x faster for multiple keys
2. **Enable Compression** - Save bandwidth and storage
3. **Set Appropriate TTLs** - Prevent memory bloat
4. **Tune Connection Pool** - Match your concurrency needs
5. **Use Distributed Locks** - Coordinate safely across instances

## Support

- **Documentation**: [Full Guide](./redis-enterprise-backend.md)
- **Examples**: `/examples/redis_enterprise_demo.rs`
- **Tests**: `/tests/redis_enterprise_integration.rs`
- **Issues**: GitHub Issues

## License

Apache-2.0

---

**Ready to get started?** Copy the basic usage example above and you're good to go! 🚀
