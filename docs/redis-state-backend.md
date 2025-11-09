# Redis State Backend

A production-ready Redis state backend for distributed state management in the LLM Auto-Optimizer stream processing system.

## Overview

The Redis State Backend provides a robust, distributed state storage solution with comprehensive features for high-availability, fault-tolerant stream processing applications. It implements the `StateBackend` trait and offers advanced capabilities beyond basic key-value storage.

## Features

### Core Capabilities

- **Connection Pooling**: Automatic connection management with configurable min/max pool sizes
- **Cluster Support**: Works seamlessly with both standalone and clustered Redis deployments
- **TTL Management**: Per-key expiration with automatic cleanup
- **Batch Operations**: Efficient pipeline support for bulk GET, PUT, and DELETE operations
- **Atomic Operations**: LUA script-based atomic operations for consistency
- **Retry Logic**: Exponential backoff retry strategy for transient failures
- **Health Checks**: Connection monitoring and auto-reconnection
- **Metrics Tracking**: Comprehensive operation statistics and performance monitoring

### Advanced Features

- **Distributed Locking**: Atomic lock acquisition with TTL for resource coordination
- **Checkpointing**: Full state snapshot and restoration for fault tolerance
- **Cursor-based Scanning**: Efficient key enumeration for large datasets
- **Memory Monitoring**: Real-time Redis memory usage tracking
- **Server Information**: Access to Redis server statistics and configuration
- **Namespace Isolation**: Key prefix support for multi-tenant deployments

## Installation

Add the Redis dependency to your `Cargo.toml`:

```toml
[dependencies]
redis = { version = "0.26", features = ["tokio-comp", "connection-manager", "streams"] }
```

The `processor` crate already includes Redis support through workspace dependencies.

## Basic Usage

### Creating a Backend

```rust
use processor::state::{RedisConfig, RedisStateBackend, StateBackend};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create configuration
    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("myapp:")
        .default_ttl(Duration::from_secs(3600))
        .pool_size(10, 50)
        .build()?;

    // Initialize backend
    let backend = RedisStateBackend::new(config).await?;

    // Verify connection
    let healthy = backend.health_check().await?;
    println!("Redis connection: {}", if healthy { "OK" } else { "FAILED" });

    Ok(())
}
```

### CRUD Operations

```rust
// Store data
backend.put(b"user:123", b"John Doe").await?;

// Retrieve data
if let Some(value) = backend.get(b"user:123").await? {
    println!("User: {}", String::from_utf8_lossy(&value));
}

// Check existence
let exists = backend.contains(b"user:123").await?;

// Delete data
backend.delete(b"user:123").await?;

// List keys with prefix
let user_keys = backend.list_keys(b"user:").await?;
println!("Found {} users", user_keys.len());

// Count total keys
let total = backend.count().await?;
```

### TTL Management

```rust
use std::time::Duration;

// Store with custom TTL (5 minutes)
backend.put_with_ttl(
    b"session:abc123",
    b"active",
    Duration::from_secs(300)
).await?;

// Check remaining TTL
if let Some(ttl) = backend.ttl(b"session:abc123").await? {
    println!("Session expires in: {:?}", ttl);
}

// Data automatically expires after TTL
```

### Batch Operations

```rust
// Batch PUT
let data = vec![
    (&b"product:1"[..], &b"Laptop"[..]),
    (&b"product:2"[..], &b"Mouse"[..]),
    (&b"product:3"[..], &b"Keyboard"[..]),
];
backend.batch_put(&data).await?;

// Batch GET
let keys = vec![b"product:1", b"product:2", b"product:3"];
let values = backend.batch_get(&keys).await?;

// Batch DELETE
let deleted = backend.batch_delete(&keys).await?;
println!("Deleted {} keys", deleted);
```

## Advanced Usage

### Atomic Operations

```rust
// Set only if key doesn't exist (distributed locking)
let acquired = backend.set_if_not_exists(
    b"lock:resource",
    b"node-1",
    Duration::from_secs(30)
).await?;

if acquired {
    // Critical section - lock acquired
    // ... perform work ...

    // Release lock
    backend.delete(b"lock:resource").await?;
}

// Atomic get-and-delete
let value = backend.get_and_delete(b"temp_data").await?;
```

### Checkpointing and Fault Tolerance

```rust
// Create checkpoint (snapshot all state)
let checkpoint = backend.checkpoint().await?;
println!("Checkpoint created with {} entries", checkpoint.len());

// Save checkpoint to disk
let checkpoint_data = bincode::serialize(&checkpoint)?;
std::fs::write("checkpoint.bin", checkpoint_data)?;

// Later, restore from checkpoint
let checkpoint_data = std::fs::read("checkpoint.bin")?;
let checkpoint = bincode::deserialize(&checkpoint_data)?;
backend.restore(checkpoint).await?;
```

### Distributed State for Stream Processing

```rust
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct WindowState {
    window_id: String,
    count: u64,
    sum: f64,
}

// Store window state
let window = WindowState {
    window_id: "window-123".to_string(),
    count: 100,
    sum: 1500.0,
};

let key = format!("window:{}", window.window_id).into_bytes();
let data = bincode::serialize(&window)?;
backend.put(&key, &data).await?;

// Retrieve window state
if let Some(data) = backend.get(&key).await? {
    let window: WindowState = bincode::deserialize(&data)?;
    println!("Window: count={}, sum={}", window.count, window.sum);
}
```

### Statistics and Monitoring

```rust
let stats = backend.stats().await;

println!("Operations: {}", stats.total_operations());
println!("Hit rate: {:.2}%", stats.hit_rate() * 100.0);
println!("Error rate: {:.2}%", stats.error_rate() * 100.0);
println!("Avg latency: {}µs", stats.avg_latency_us);
println!("Bytes read: {}", stats.bytes_read);
println!("Bytes written: {}", stats.bytes_written);

// Memory usage
let memory = backend.memory_usage().await?;
println!("Redis memory: {:.2} MB", memory as f64 / 1024.0 / 1024.0);
```

## Configuration

### RedisConfig Options

```rust
let config = RedisConfig::builder()
    // Connection
    .url("redis://localhost:6379")              // Single instance
    // .urls(vec![...])                         // Multiple URLs for cluster

    // Namespace
    .key_prefix("myapp:")                       // Key prefix for isolation

    // TTL
    .default_ttl(Duration::from_secs(3600))     // Default expiration

    // Connection Pool
    .pool_size(10, 50)                          // Min/max connections

    // Timeouts
    .timeouts(
        Duration::from_secs(5),                 // Connect timeout
        Duration::from_secs(3),                 // Read timeout
        Duration::from_secs(3)                  // Write timeout
    )

    // Retry Strategy
    .retries(
        3,                                      // Max retry attempts
        Duration::from_millis(100),             // Base delay
        Duration::from_secs(5)                  // Max delay
    )

    // Cluster and TLS
    .cluster_mode(false)                        // Enable cluster support
    .tls(false)                                 // Enable TLS/SSL

    // Pipeline
    .pipeline_batch_size(100)                   // Batch size for pipelines

    .build()?;
```

### Default Configuration

```rust
RedisConfig {
    urls: vec!["redis://localhost:6379"],
    key_prefix: "state:",
    default_ttl: None,
    min_connections: 5,
    max_connections: 50,
    connect_timeout: Duration::from_secs(5),
    read_timeout: Duration::from_secs(3),
    write_timeout: Duration::from_secs(3),
    max_retries: 3,
    retry_base_delay: Duration::from_millis(100),
    retry_max_delay: Duration::from_secs(5),
    cluster_mode: false,
    tls_enabled: false,
    pipeline_batch_size: 100,
}
```

## Production Deployment

### Docker Compose Setup

```yaml
version: '3.8'

services:
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis-data:/data
    command: redis-server --appendonly yes --maxmemory 2gb --maxmemory-policy allkeys-lru
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 3s
      retries: 3

  redis-sentinel:
    image: redis:7-alpine
    command: redis-sentinel /etc/redis/sentinel.conf
    volumes:
      - ./sentinel.conf:/etc/redis/sentinel.conf

volumes:
  redis-data:
```

### Redis Cluster Setup

```rust
let config = RedisConfig::builder()
    .urls(vec![
        "redis://node1:6379".to_string(),
        "redis://node2:6379".to_string(),
        "redis://node3:6379".to_string(),
    ])
    .cluster_mode(true)
    .key_prefix("processor:")
    .build()?;

let backend = RedisStateBackend::new(config).await?;
```

### Monitoring and Alerting

```rust
use std::time::Duration;
use tokio::time::interval;

// Background monitoring task
tokio::spawn({
    let backend = backend.clone();
    async move {
        let mut ticker = interval(Duration::from_secs(60));
        loop {
            ticker.tick().await;

            // Check health
            if let Ok(healthy) = backend.health_check().await {
                if !healthy {
                    eprintln!("ALERT: Redis health check failed!");
                }
            }

            // Check stats
            let stats = backend.stats().await;
            if stats.error_rate() > 0.05 {
                eprintln!("ALERT: High error rate: {:.2}%", stats.error_rate() * 100.0);
            }

            // Check memory
            if let Ok(memory) = backend.memory_usage().await {
                let mb = memory as f64 / 1024.0 / 1024.0;
                if mb > 1800.0 {
                    eprintln!("ALERT: High memory usage: {:.2} MB", mb);
                }
            }
        }
    }
});
```

## Performance Considerations

### Connection Pooling

- **Min connections**: Set based on baseline load (5-10 for most applications)
- **Max connections**: Set based on peak load and Redis server capacity
- Monitor active connections and adjust pool size accordingly

### Batch Operations

- Use `batch_get`, `batch_put`, and `batch_delete` for bulk operations
- Batch size of 100-1000 keys typically provides best performance
- Adjust `pipeline_batch_size` based on network latency and Redis capacity

### TTL Strategy

- Set appropriate TTLs to prevent memory exhaustion
- Use longer TTLs for infrequently changing data
- Use shorter TTLs for session data and temporary state
- Monitor expired keys and eviction policy

### Key Design

- Use consistent key naming: `prefix:type:id`
- Keep keys short to reduce memory usage
- Use prefixes for efficient `list_keys` operations
- Avoid very long keys (>1KB)

## Error Handling

```rust
use processor::error::StateError;

match backend.get(b"key").await {
    Ok(Some(value)) => println!("Found: {:?}", value),
    Ok(None) => println!("Not found"),
    Err(StateError::StorageError { backend_type, details }) => {
        eprintln!("Storage error in {}: {}", backend_type, details);
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

## Troubleshooting

### Connection Issues

```rust
// Increase timeouts for slow networks
let config = RedisConfig::builder()
    .url("redis://remote-server:6379")
    .timeouts(
        Duration::from_secs(10),  // Connect
        Duration::from_secs(5),   // Read
        Duration::from_secs(5)    // Write
    )
    .retries(5, Duration::from_millis(200), Duration::from_secs(10))
    .build()?;
```

### Memory Issues

```bash
# Check Redis memory usage
redis-cli INFO memory

# Configure maxmemory and eviction policy
redis-cli CONFIG SET maxmemory 2gb
redis-cli CONFIG SET maxmemory-policy allkeys-lru
```

### Performance Issues

```rust
// Increase pipeline batch size for bulk operations
let config = RedisConfig::builder()
    .pipeline_batch_size(500)
    .pool_size(20, 100)
    .build()?;
```

## Testing

### Unit Tests

```bash
# Start Redis
docker run -d -p 6379:6379 redis:7-alpine

# Run tests
cargo test --package processor redis_backend
```

### Integration Tests

```rust
#[tokio::test]
async fn test_distributed_scenario() {
    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("test:")
        .build()
        .unwrap();

    let backend = RedisStateBackend::new(config).await.unwrap();
    backend.clear().await.unwrap();

    // Test scenario...
}
```

## Examples

Run the included examples:

```bash
# Basic features
cargo run --example redis_state_backend

# Distributed state management
cargo run --example redis_distributed_state
```

## References

- [Redis Documentation](https://redis.io/documentation)
- [redis-rs Crate](https://docs.rs/redis/)
- [Stream Processing with Redis](https://redis.io/docs/manual/data-types/streams/)
- [Redis Cluster Tutorial](https://redis.io/docs/management/scaling/)

## License

Apache-2.0
