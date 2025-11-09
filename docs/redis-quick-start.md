# Redis State Backend - Quick Start Guide

Get up and running with the Redis state backend in 5 minutes.

## Prerequisites

### 1. Start Redis

Using Docker:
```bash
docker run -d -p 6379:6379 --name redis redis:7-alpine
```

Using Docker Compose:
```yaml
version: '3.8'
services:
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis-data:/data
    command: redis-server --appendonly yes

volumes:
  redis-data:
```

### 2. Verify Connection

```bash
docker exec redis redis-cli PING
# Expected output: PONG
```

## Basic Usage

### 1. Add to Your Project

The Redis backend is already included in the `processor` crate:

```rust
use processor::state::{RedisConfig, RedisStateBackend, StateBackend};
use std::time::Duration;
```

### 2. Create Backend

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Simple configuration
    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("myapp:")
        .build()?;

    let backend = RedisStateBackend::new(config).await?;

    // Verify connection
    backend.health_check().await?;
    println!("Connected to Redis!");

    Ok(())
}
```

### 3. Store and Retrieve Data

```rust
// Store data
backend.put(b"user:123", b"John Doe").await?;

// Retrieve data
if let Some(value) = backend.get(b"user:123").await? {
    println!("User: {}", String::from_utf8_lossy(&value));
}

// Check if exists
if backend.contains(b"user:123").await? {
    println!("User exists");
}

// Delete data
backend.delete(b"user:123").await?;
```

### 4. Use TTL for Sessions

```rust
// Store with 30-minute expiration
backend.put_with_ttl(
    b"session:abc123",
    b"session_data",
    Duration::from_secs(1800)
).await?;

// Check remaining time
if let Some(ttl) = backend.ttl(b"session:abc123").await? {
    println!("Session expires in: {:?}", ttl);
}
```

## Common Patterns

### Pattern 1: Window State Management

```rust
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct WindowState {
    count: u64,
    sum: f64,
}

// Store window state
let state = WindowState { count: 100, sum: 1500.0 };
let key = b"window:123";
let data = bincode::serialize(&state)?;
backend.put(&key, &data).await?;

// Retrieve window state
if let Some(data) = backend.get(key).await? {
    let state: WindowState = bincode::deserialize(&data)?;
    println!("Window: count={}, sum={}", state.count, state.sum);
}
```

### Pattern 2: Distributed Locking

```rust
// Try to acquire lock
let lock_acquired = backend.set_if_not_exists(
    b"lock:resource",
    b"worker-1",
    Duration::from_secs(30)
).await?;

if lock_acquired {
    println!("Lock acquired! Performing work...");

    // Do work here...

    // Release lock
    backend.delete(b"lock:resource").await?;
} else {
    println!("Lock already held by another worker");
}
```

### Pattern 3: Bulk Operations

```rust
// Prepare data
let data = vec![
    (&b"metric:cpu"[..], &b"45.2"[..]),
    (&b"metric:memory"[..], &b"78.5"[..]),
    (&b"metric:disk"[..], &b"62.1"[..]),
];

// Batch insert (much faster than individual puts)
backend.batch_put(&data).await?;

// Batch retrieve
let keys = vec![b"metric:cpu", b"metric:memory", b"metric:disk"];
let values = backend.batch_get(&keys).await?;

for (key, value) in keys.iter().zip(values.iter()) {
    if let Some(val) = value {
        println!("{}: {}",
                 String::from_utf8_lossy(key),
                 String::from_utf8_lossy(val));
    }
}
```

### Pattern 4: Checkpointing

```rust
// Create checkpoint before risky operation
let checkpoint = backend.checkpoint().await?;
println!("Saved {} keys", checkpoint.len());

// Do risky operation...
backend.put(b"new_key", b"new_value").await?;

// If something goes wrong, restore
if error_occurred {
    backend.restore(checkpoint).await?;
    println!("State restored");
}
```

## Configuration Examples

### Development (Local)

```rust
let config = RedisConfig::builder()
    .url("redis://localhost:6379")
    .key_prefix("dev:")
    .build()?;
```

### Production (With TTL)

```rust
let config = RedisConfig::builder()
    .url("redis://redis-prod:6379")
    .key_prefix("prod:")
    .default_ttl(Duration::from_secs(3600))  // 1 hour
    .pool_size(20, 100)
    .retries(5, Duration::from_millis(100), Duration::from_secs(5))
    .build()?;
```

### Production (Cluster)

```rust
let config = RedisConfig::builder()
    .urls(vec![
        "redis://node1:6379".to_string(),
        "redis://node2:6379".to_string(),
        "redis://node3:6379".to_string(),
    ])
    .cluster_mode(true)
    .key_prefix("cluster:")
    .pool_size(50, 200)
    .build()?;
```

## Monitoring

### Basic Health Check

```rust
tokio::spawn({
    let backend = backend.clone();
    async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;

            if let Ok(healthy) = backend.health_check().await {
                if !healthy {
                    eprintln!("WARNING: Redis health check failed!");
                }
            }
        }
    }
});
```

### Statistics Dashboard

```rust
let stats = backend.stats().await;
println!("=== Redis Statistics ===");
println!("Total operations: {}", stats.total_operations());
println!("Hit rate: {:.2}%", stats.hit_rate() * 100.0);
println!("Error rate: {:.2}%", stats.error_rate() * 100.0);
println!("Avg latency: {}µs", stats.avg_latency_us);

let memory = backend.memory_usage().await?;
println!("Memory: {:.2} MB", memory as f64 / 1024.0 / 1024.0);
```

## Running Examples

### Example 1: Basic Features

```bash
# Start Redis
docker run -d -p 6379:6379 redis:7-alpine

# Run example
cargo run --example redis_state_backend
```

### Example 2: Distributed State

```bash
cargo run --example redis_distributed_state
```

## Troubleshooting

### Connection Failed

```
Error: Storage error in redis: Connection failed after 3 retries
```

**Solution**: Check Redis is running and accessible:
```bash
docker ps | grep redis
redis-cli PING
```

### Slow Operations

**Solution**: Check Redis performance:
```bash
redis-cli --latency
redis-cli --latency-history
```

Increase pool size:
```rust
let config = RedisConfig::builder()
    .pool_size(50, 200)  // Increase from default
    .build()?;
```

### Memory Issues

**Solution**: Set TTL and monitor usage:
```rust
let config = RedisConfig::builder()
    .default_ttl(Duration::from_secs(3600))
    .build()?;

// Monitor
let memory = backend.memory_usage().await?;
println!("Memory: {} bytes", memory);
```

## Next Steps

1. Read the [complete documentation](redis-state-backend.md)
2. Review the [implementation summary](redis-state-backend-summary.md)
3. Run the examples to see all features
4. Integrate into your stream processor

## Resources

- [Redis Documentation](https://redis.io/documentation)
- [redis-rs Crate](https://docs.rs/redis/)
- [State Backend Trait](../crates/processor/src/state/backend.rs)

## Support

For issues or questions:
- Check the inline documentation
- Review the examples
- Consult the full documentation

Happy caching! 🚀
