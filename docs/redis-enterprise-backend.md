# Enterprise Redis Backend Documentation

## Overview

The Enterprise Redis Backend provides production-ready Redis state management with advanced features for distributed systems. It extends the basic Redis backend with enterprise-grade capabilities including high availability, advanced compression, distributed locking, and comprehensive observability.

## Table of Contents

- [Features](#features)
- [Installation](#installation)
- [Configuration](#configuration)
- [Deployment Modes](#deployment-modes)
- [Compression](#compression)
- [Distributed Locking](#distributed-locking)
- [Batch Operations](#batch-operations)
- [Observability](#observability)
- [Error Handling](#error-handling)
- [Best Practices](#best-practices)
- [Examples](#examples)

## Features

### Core Features

- **High Availability**: Support for Redis Sentinel and Cluster modes
- **Connection Pooling**: Advanced connection management with deadpool-redis
- **Distributed Locking**: Redlock algorithm implementation
- **Batch Operations**: Optimized pipeline and transaction support
- **Compression**: Multiple algorithms (LZ4, Snappy, Zstd)
- **Serialization**: Flexible format support (Bincode, MessagePack, JSON)
- **Observability**: Prometheus metrics and structured logging
- **Resilience**: Circuit breaker pattern, retry with exponential backoff and jitter
- **Performance**: Slow query logging and latency tracking

### Production Features

✅ Automatic failover handling
✅ Health monitoring and auto-reconnection
✅ TTL management per key
✅ Key namespacing and prefixing
✅ Comprehensive error handling
✅ Zero-copy optimizations where possible
✅ Thread-safe concurrent access

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
processor = { path = "../processor" }
tokio = { version = "1.40", features = ["full"] }
```

## Configuration

### Basic Configuration

```rust
use processor::state::{
    EnterpriseRedisBackend, EnterpriseRedisConfig, RedisMode,
    CompressionAlgorithm
};
use std::time::Duration;

let config = EnterpriseRedisConfig::builder()
    .mode(RedisMode::Standalone {
        url: "redis://localhost:6379".to_string(),
    })
    .key_prefix("myapp:")
    .default_ttl(Duration::from_secs(3600))
    .pool_config(10, 100, Duration::from_secs(30))
    .compression(CompressionAlgorithm::Lz4)
    .enable_metrics(true)
    .build()?;

let backend = EnterpriseRedisBackend::new(config).await?;
```

### Configuration Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `mode` | `RedisMode` | Standalone | Deployment mode (Standalone/Sentinel/Cluster) |
| `key_prefix` | `String` | "state:" | Namespace prefix for all keys |
| `default_ttl` | `Option<Duration>` | None | Default expiration time for keys |
| `pool_min` | `usize` | 10 | Minimum connections in pool |
| `pool_max` | `usize` | 100 | Maximum connections in pool |
| `pool_timeout` | `Duration` | 30s | Timeout for acquiring connection |
| `connect_timeout` | `Duration` | 5s | Connection establishment timeout |
| `command_timeout` | `Duration` | 3s | Individual command timeout |
| `max_retries` | `u32` | 3 | Maximum retry attempts |
| `retry_initial_delay` | `Duration` | 100ms | Initial retry delay |
| `retry_max_delay` | `Duration` | 5s | Maximum retry delay |
| `retry_jitter` | `f64` | 0.3 | Jitter factor (0.0-1.0) |
| `compression` | `CompressionAlgorithm` | None | Compression algorithm |
| `compression_threshold` | `usize` | 1024 | Min size in bytes to compress |
| `pipeline_batch_size` | `usize` | 100 | Items per pipeline batch |
| `enable_circuit_breaker` | `bool` | true | Enable circuit breaker |
| `circuit_breaker_threshold` | `u32` | 10 | Failures before opening |
| `circuit_breaker_timeout` | `Duration` | 60s | Time before retry |
| `enable_metrics` | `bool` | true | Enable Prometheus metrics |
| `enable_slow_query_log` | `bool` | true | Log slow queries |
| `slow_query_threshold` | `Duration` | 100ms | Slow query threshold |

## Deployment Modes

### Standalone Mode

Single Redis instance - simplest setup for development and small deployments.

```rust
let config = EnterpriseRedisConfig::builder()
    .mode(RedisMode::Standalone {
        url: "redis://localhost:6379".to_string(),
    })
    .build()?;
```

### Sentinel Mode

High availability with automatic failover.

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
    .build()?;
```

**Features:**
- Automatic master discovery
- Failover handling
- Health monitoring of sentinel nodes
- Automatic reconnection to new master

### Cluster Mode

Horizontal scaling with data sharding (Note: Requires additional setup).

```rust
let config = EnterpriseRedisConfig::builder()
    .mode(RedisMode::Cluster {
        nodes: vec![
            "redis://node1:6379".to_string(),
            "redis://node2:6379".to_string(),
            "redis://node3:6379".to_string(),
        ],
    })
    .build()?;
```

**Note:** Cluster mode requires the `redis-cluster` crate and is currently marked for future implementation.

## Compression

### Supported Algorithms

#### LZ4 (Recommended)
- **Speed**: Very fast compression and decompression
- **Ratio**: Moderate compression (2-3x)
- **Use case**: General purpose, low-latency requirements

```rust
.compression(CompressionAlgorithm::Lz4)
.compression_threshold(1024) // Only compress data > 1KB
```

#### Snappy
- **Speed**: Fast
- **Ratio**: Moderate compression (2-2.5x)
- **Use case**: Balanced speed and compression

```rust
.compression(CompressionAlgorithm::Snappy)
```

#### Zstd
- **Speed**: Slower but configurable
- **Ratio**: Best compression (3-5x)
- **Use case**: Large data, storage optimization

```rust
.compression(CompressionAlgorithm::Zstd)
```

#### None
- No compression overhead
- Use for small values or pre-compressed data

```rust
.compression(CompressionAlgorithm::None)
```

### Compression Threshold

Data smaller than the threshold is not compressed:

```rust
.compression_threshold(1024) // Don't compress data < 1KB
```

## Distributed Locking

### Redlock Algorithm

Enterprise Redis Backend implements the Redlock distributed locking algorithm for safe coordination across multiple instances.

### Basic Usage

```rust
// Acquire lock
let guard = backend.acquire_lock("resource_name", Duration::from_secs(30)).await?;

// Critical section - only one instance can execute this
perform_critical_operation().await?;

// Lock automatically released when guard is dropped
drop(guard);
```

### Non-blocking Acquisition

```rust
if let Some(guard) = backend.try_acquire_lock("resource", Duration::from_secs(30)).await? {
    // Lock acquired
    do_work().await?;
    drop(guard);
} else {
    // Lock already held by another instance
    println!("Resource is busy");
}
```

### Use Cases

1. **Checkpoint Coordination**: Ensure only one instance creates checkpoints
2. **Leader Election**: Single coordinator for distributed tasks
3. **Resource Management**: Prevent concurrent access to shared resources
4. **State Compaction**: Serialize compaction operations

### Lock Properties

- **Automatic Expiration**: Locks automatically expire after TTL
- **Safe Release**: Token-based verification prevents accidental release
- **Deadlock Prevention**: TTL ensures locks don't persist after crashes
- **RAII Pattern**: Automatic cleanup via Drop trait

## Batch Operations

### Batch Get

Efficiently retrieve multiple keys in a single round-trip:

```rust
let keys = vec![b"key1".as_ref(), b"key2".as_ref(), b"key3".as_ref()];
let values = backend.batch_get_optimized(&keys).await?;

for (key, value) in keys.iter().zip(values.iter()) {
    if let Some(v) = value {
        println!("{:?} = {:?}", key, v);
    }
}
```

### Batch Put

Write multiple key-value pairs efficiently:

```rust
let pairs = vec![
    (b"key1".as_ref(), b"value1".as_ref()),
    (b"key2".as_ref(), b"value2".as_ref()),
    (b"key3".as_ref(), b"value3".as_ref()),
];

backend.batch_put_optimized(&pairs).await?;
```

### Performance

Batch operations use Redis pipelines for optimal performance:
- Single network round-trip for multiple operations
- Automatic chunking based on `pipeline_batch_size`
- Atomic execution with MULTI/EXEC

## Observability

### Prometheus Metrics

The backend exports comprehensive metrics for monitoring:

```rust
// Get metrics registry
if let Some(metrics) = backend.metrics() {
    // Register with your Prometheus registry
    // metrics.register(&mut registry);
}
```

#### Available Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `redis_operations_total` | Counter | Total operations by type and status |
| `redis_operation_duration_seconds` | Histogram | Operation latency distribution |
| `redis_pool_connections_active` | Gauge | Active connections |
| `redis_pool_connections_idle` | Gauge | Idle connections |
| `redis_circuit_breaker_state` | Gauge | Circuit breaker state (0/1/2) |
| `redis_compression_ratio` | Gauge | Compression effectiveness |
| `redis_errors_total` | Counter | Total errors by type |

### Backend Statistics

Get runtime statistics:

```rust
let stats = backend.stats().await;

println!("Total operations: {}", stats.operations_total);
println!("Success rate: {:.2}%",
    stats.operations_success as f64 / stats.operations_total as f64 * 100.0);
println!("Avg latency: {}µs", stats.avg_latency_us);
println!("Cache hit rate: {:.2}%",
    stats.cache_hits as f64 / (stats.cache_hits + stats.cache_misses) as f64 * 100.0);
println!("Compression ratio: {:.2}x",
    stats.bytes_written as f64 / stats.bytes_compressed as f64);
```

### Slow Query Logging

Automatically log slow queries:

```rust
.enable_slow_query_log(true)
.slow_query_threshold(Duration::from_millis(100))
```

Slow queries will be logged with:
- Operation name
- Duration
- Full timing breakdown

### Health Checks

Monitor backend health:

```rust
if backend.health_check().await? {
    println!("Backend is healthy");
} else {
    println!("Backend is unhealthy");
}
```

## Error Handling

### Circuit Breaker Pattern

Prevents cascade failures by opening the circuit after repeated errors:

```rust
.enable_circuit_breaker(true)
.circuit_breaker_threshold(10)  // Open after 10 failures
.circuit_breaker_timeout(Duration::from_secs(60))  // Stay open for 60s
```

**States:**
- **Closed**: Normal operation, requests pass through
- **Open**: Too many failures, requests fail immediately
- **Half-Open**: Testing if backend recovered

### Retry Logic

Automatic retry with exponential backoff and jitter:

```rust
.retry_config(
    3,                              // max_retries
    Duration::from_millis(100),     // initial_delay
    Duration::from_secs(5),         // max_delay
    0.3                             // jitter (30%)
)
```

**Features:**
- Exponential backoff prevents thundering herd
- Jitter randomizes retry timing
- Configurable max delay cap
- Detailed logging of retry attempts

### Error Types

All operations return `StateResult<T>`:

```rust
match backend.get(b"key").await {
    Ok(Some(value)) => println!("Found: {:?}", value),
    Ok(None) => println!("Not found"),
    Err(StateError::StorageError { details, .. }) => {
        eprintln!("Storage error: {}", details);
    }
    Err(e) => eprintln!("Other error: {}", e),
}
```

## Best Practices

### Connection Pooling

```rust
// Production settings
.pool_config(
    20,                            // min: Keep 20 connections warm
    200,                           // max: Allow up to 200 connections
    Duration::from_secs(30)        // timeout: 30s to get connection
)
```

### Compression Strategy

```rust
// For mixed workload
.compression(CompressionAlgorithm::Lz4)
.compression_threshold(1024)  // Only compress > 1KB

// For large data (analytics, logs)
.compression(CompressionAlgorithm::Zstd)
.compression_threshold(512)

// For low-latency (caching, sessions)
.compression(CompressionAlgorithm::None)
```

### TTL Management

```rust
// Global default
.default_ttl(Duration::from_secs(3600))  // 1 hour

// Per-key TTL (use specialized methods in base redis_backend)
// backend.put_with_ttl(key, value, Duration::from_secs(300)).await?;
```

### Namespacing

Use clear prefixes to avoid key collisions:

```rust
.key_prefix("myapp:production:")  // Separate by environment
.key_prefix("sessions:")          // Separate by data type
```

### Batch Size

Optimize for your network and workload:

```rust
// For high-latency networks
.pipeline_batch_size(500)  // Larger batches

// For low-latency networks
.pipeline_batch_size(50)   // Smaller batches, lower memory
```

## Examples

### Complete Production Setup

```rust
use processor::state::{
    EnterpriseRedisBackend, EnterpriseRedisConfig, RedisMode,
    CompressionAlgorithm, StateBackend,
};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Configure for production
    let config = EnterpriseRedisConfig::builder()
        .mode(RedisMode::Sentinel {
            sentinels: vec![
                "redis://sentinel1:26379".to_string(),
                "redis://sentinel2:26379".to_string(),
                "redis://sentinel3:26379".to_string(),
            ],
            service_name: "mymaster".to_string(),
        })
        .key_prefix("myapp:prod:")
        .default_ttl(Duration::from_secs(3600))
        .pool_config(20, 200, Duration::from_secs(30))
        .timeouts(Duration::from_secs(5), Duration::from_secs(3))
        .retry_config(
            3,
            Duration::from_millis(100),
            Duration::from_secs(5),
            0.3,
        )
        .compression(CompressionAlgorithm::Lz4)
        .compression_threshold(1024)
        .circuit_breaker(true, 10, Duration::from_secs(60))
        .enable_metrics(true)
        .slow_query_log(true, Duration::from_millis(100))
        .build()?;

    let backend = EnterpriseRedisBackend::new(config).await?;

    // Use the backend
    backend.put(b"user:123:session", b"session_data").await?;

    if let Some(data) = backend.get(b"user:123:session").await? {
        println!("Session: {:?}", data);
    }

    // Get statistics
    let stats = backend.stats().await;
    println!("Operations: {}, Hit rate: {:.2}%",
        stats.operations_total,
        stats.cache_hits as f64 / (stats.cache_hits + stats.cache_misses) as f64 * 100.0
    );

    Ok(())
}
```

### Coordinated Checkpoint Example

```rust
use processor::state::{EnterpriseRedisBackend, StateBackend};
use std::time::Duration;

async fn coordinated_checkpoint(
    backend: &EnterpriseRedisBackend,
) -> anyhow::Result<()> {
    // Try to acquire checkpoint lock
    if let Some(guard) = backend
        .try_acquire_lock("checkpoint", Duration::from_secs(300))
        .await?
    {
        println!("Checkpoint lock acquired, creating checkpoint...");

        // Create checkpoint - only this instance will do this
        create_checkpoint(backend).await?;

        println!("Checkpoint created successfully");
        drop(guard);
    } else {
        println!("Another instance is creating checkpoint, skipping...");
    }

    Ok(())
}

async fn create_checkpoint(backend: &EnterpriseRedisBackend) -> anyhow::Result<()> {
    // Checkpoint logic here
    Ok(())
}
```

### High-Performance Batch Processing

```rust
use processor::state::{EnterpriseRedisBackend, StateBackend};

async fn process_batch(
    backend: &EnterpriseRedisBackend,
    items: Vec<(String, String)>,
) -> anyhow::Result<()> {
    // Convert to byte slices
    let pairs: Vec<(&[u8], &[u8])> = items
        .iter()
        .map(|(k, v)| (k.as_bytes(), v.as_bytes()))
        .collect();

    // Batch write
    backend.batch_put_optimized(&pairs).await?;

    // Batch read
    let keys: Vec<&[u8]> = items.iter().map(|(k, _)| k.as_bytes()).collect();
    let values = backend.batch_get_optimized(&keys).await?;

    for (i, value) in values.iter().enumerate() {
        if let Some(v) = value {
            println!("Item {}: {:?}", i, String::from_utf8_lossy(v));
        }
    }

    Ok(())
}
```

## Troubleshooting

### Connection Issues

```rust
// Increase timeouts for slow networks
.connect_timeout(Duration::from_secs(10))
.command_timeout(Duration::from_secs(5))

// Increase pool size for high concurrency
.pool_config(50, 500, Duration::from_secs(60))
```

### High Latency

```rust
// Enable slow query logging
.enable_slow_query_log(true)
.slow_query_threshold(Duration::from_millis(50))

// Check compression overhead
.compression(CompressionAlgorithm::None)  // Disable temporarily

// Use batch operations
backend.batch_get_optimized(&keys).await?;
```

### Memory Issues

```rust
// Reduce compression threshold
.compression_threshold(512)

// Set appropriate TTLs
.default_ttl(Duration::from_secs(1800))

// Use smaller batch sizes
.pipeline_batch_size(50)
```

### Circuit Breaker Tripping

```rust
// Increase threshold for noisy backends
.circuit_breaker_threshold(20)

// Increase timeout
.circuit_breaker_timeout(Duration::from_secs(120))

// Check health
if !backend.health_check().await? {
    println!("Backend unhealthy!");
}
```

## Performance Tuning

### Latency Optimization

1. Use batch operations for multiple keys
2. Enable LZ4 compression for large values
3. Set appropriate connection pool size
4. Use shorter TTLs to reduce memory pressure

### Throughput Optimization

1. Increase pipeline batch size
2. Use larger connection pools
3. Enable compression for bandwidth-limited networks
4. Use batch operations instead of individual calls

### Memory Optimization

1. Set appropriate TTLs
2. Use Zstd compression for better ratios
3. Set compression threshold based on your data
4. Regular cleanup of expired keys

## Migration Guide

### From Basic Redis Backend

The enterprise backend is fully compatible with the `StateBackend` trait:

```rust
// Before (basic backend)
use processor::state::{RedisStateBackend, RedisConfig};

// After (enterprise backend)
use processor::state::{EnterpriseRedisBackend, EnterpriseRedisConfig, RedisMode};

let config = EnterpriseRedisConfig::builder()
    .mode(RedisMode::Standalone { url: "redis://localhost:6379".to_string() })
    .key_prefix("myapp:")
    .build()?;

let backend = EnterpriseRedisBackend::new(config).await?;

// All StateBackend methods work the same
backend.put(b"key", b"value").await?;
```

### New Features

Take advantage of enterprise features:

```rust
// Use compression
.compression(CompressionAlgorithm::Lz4)

// Use batch operations
backend.batch_put_optimized(&pairs).await?;

// Use distributed locking
let guard = backend.acquire_lock("resource", Duration::from_secs(30)).await?;

// Monitor with metrics
let stats = backend.stats().await;
```

## Support

For issues, questions, or feature requests:
- GitHub Issues: https://github.com/llm-devops/llm-auto-optimizer/issues
- Documentation: https://llmdevops.dev/docs

## License

Apache-2.0
