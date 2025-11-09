# Distributed State Quick Start Guide

This guide will help you get started with distributed state backends quickly.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Configuration Examples](#configuration-examples)
- [Running Tests](#running-tests)
- [Running Examples](#running-examples)
- [Common Patterns](#common-patterns)
- [Troubleshooting](#troubleshooting)
- [Next Steps](#next-steps)

## Prerequisites

### Required Services

For distributed state backends, you need to run the required services:

```bash
# Start Redis and PostgreSQL using Docker Compose
docker-compose up -d redis postgres

# Verify services are running
docker-compose ps

# Check Redis
redis-cli ping
# Expected output: PONG

# Check PostgreSQL
psql -h localhost -U optimizer -d optimizer -c "SELECT 1;"
# Expected output: 1
```

### Optional Monitoring Services

```bash
# Start monitoring services
docker-compose up -d redis-commander pgadmin

# Access UIs
# Redis Commander: http://localhost:8081
# pgAdmin: http://localhost:5050
```

## Quick Start

### 1. Memory Backend (No External Dependencies)

```rust
use processor::state::{MemoryStateBackend, StateBackend};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create backend with 1-hour TTL
    let backend = MemoryStateBackend::new(Some(Duration::from_secs(3600)));

    // Store data
    backend.put(b"user:1001", b"Alice").await?;

    // Retrieve data
    if let Some(value) = backend.get(b"user:1001").await? {
        println!("Value: {}", String::from_utf8_lossy(&value));
    }

    Ok(())
}
```

**When to use**: Single instance, development, testing

### 2. Redis Backend (Distributed Cache)

```rust
use processor::state::{RedisConfig, RedisStateBackend, StateBackend};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Configure Redis
    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("myapp:")
        .default_ttl(Duration::from_secs(3600))
        .build()?;

    // Create backend
    let backend = RedisStateBackend::new(config).await?;

    // Health check
    assert!(backend.health_check().await?);

    // Store data
    backend.put(b"session:abc", b"session_data").await?;

    // Retrieve data
    if let Some(value) = backend.get(b"session:abc").await? {
        println!("Session: {}", String::from_utf8_lossy(&value));
    }

    // Get statistics
    let stats = backend.stats().await;
    println!("Hit rate: {:.2}%", stats.hit_rate() * 100.0);

    Ok(())
}
```

**When to use**: Multi-instance, fast distributed cache, session state

### 3. PostgreSQL Backend (Persistent Storage)

```rust
use processor::state::{PostgresStateBackend, StateBackend};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create backend
    let backend = PostgresStateBackend::new(
        "postgres://optimizer:password@localhost:5432/optimizer",
        "my_state_table"
    ).await?;

    // Health check
    assert!(backend.health_check().await?);

    // Store data
    backend.put(b"config:version", b"1.0.0").await?;

    // Retrieve data
    if let Some(value) = backend.get(b"config:version").await? {
        println!("Version: {}", String::from_utf8_lossy(&value));
    }

    // Get statistics
    let stats = backend.stats().await?;
    println!("Total keys: {}", stats.total_keys);

    Ok(())
}
```

**When to use**: Persistent storage, ACID transactions, audit logs

### 4. Tiered Cache (Memory + Redis + PostgreSQL)

```rust
use processor::state::{
    CachedStateBackend, MemoryStateBackend,
    RedisConfig, RedisStateBackend,
    PostgresStateBackend, StateBackend,
};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // L3: PostgreSQL (persistent)
    let postgres = Arc::new(
        PostgresStateBackend::new(
            "postgres://optimizer:password@localhost:5432/optimizer",
            "state"
        ).await?
    );

    // L2: Redis (distributed cache)
    let redis_config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("cache:")
        .default_ttl(Duration::from_secs(300))
        .build()?;
    let redis = Arc::new(RedisStateBackend::new(redis_config).await?);

    // L1: Memory cache on top of Redis
    let cached = CachedStateBackend::new(
        redis.clone(),
        1000,                          // 1000 items max
        Some(Duration::from_secs(60))  // 1-minute cache
    );

    // Use the cached backend
    cached.put(b"key", b"value").await?;
    let value = cached.get(b"key").await?;

    // Check cache performance
    let stats = cached.cache_stats().await;
    println!("Hit rate: {:.2}%", stats.hit_rate * 100.0);

    Ok(())
}
```

**When to use**: Production deployments, optimal performance

### 5. Distributed Lock

```rust
use processor::state::{RedisLock, DistributedLock};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let lock = RedisLock::new(
        "redis://localhost:6379",
        "my_resource_lock"
    ).await?;

    // Try to acquire lock
    if lock.try_lock(Duration::from_secs(30)).await? {
        println!("Lock acquired!");

        // Critical section
        // ... do work ...

        // Release lock
        lock.unlock().await?;
        println!("Lock released!");
    } else {
        println!("Could not acquire lock");
    }

    Ok(())
}
```

**When to use**: Leader election, distributed coordination, critical sections

## Configuration Examples

### Environment-Based Configuration

```rust
use std::env;

fn get_redis_config() -> anyhow::Result<RedisConfig> {
    let url = env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());

    let prefix = env::var("REDIS_KEY_PREFIX")
        .unwrap_or_else(|_| "app:".to_string());

    let ttl_secs = env::var("REDIS_TTL_SECONDS")
        .unwrap_or_else(|_| "3600".to_string())
        .parse::<u64>()?;

    RedisConfig::builder()
        .url(url)
        .key_prefix(prefix)
        .default_ttl(Duration::from_secs(ttl_secs))
        .build()
}
```

### YAML Configuration

```yaml
# config.yaml
state:
  backend: redis
  redis:
    url: redis://localhost:6379
    key_prefix: optimizer:
    default_ttl_seconds: 3600
    pool:
      min_connections: 5
      max_connections: 50
    timeouts:
      connect_ms: 5000
      read_ms: 3000
      write_ms: 3000
  postgres:
    url: postgres://optimizer:password@localhost:5432/optimizer
    table_name: processor_state
    pool:
      max_connections: 20
```

Load configuration:

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct Config {
    state: StateConfig,
}

#[derive(Deserialize)]
struct StateConfig {
    backend: String,
    redis: Option<RedisSettings>,
    postgres: Option<PostgresSettings>,
}

fn load_config() -> anyhow::Result<Config> {
    let config_str = std::fs::read_to_string("config.yaml")?;
    Ok(serde_yaml::from_str(&config_str)?)
}
```

### Production Configuration (Redis)

```rust
let config = RedisConfig::builder()
    .url("redis://redis.production.svc.cluster.local:6379")
    .key_prefix("llm-optimizer:")
    .default_ttl(Duration::from_secs(3600))
    .pool_size(10, 100)
    .timeouts(
        Duration::from_secs(5),   // connect
        Duration::from_secs(3),   // read
        Duration::from_secs(3),   // write
    )
    .retries(3, Duration::from_millis(100), Duration::from_secs(5))
    .cluster_mode(false)
    .tls(true)
    .build()?;
```

### Production Configuration (PostgreSQL)

```rust
let database_url = format!(
    "postgres://{}:{}@{}:{}/{}",
    env::var("POSTGRES_USER")?,
    env::var("POSTGRES_PASSWORD")?,
    env::var("POSTGRES_HOST")?,
    env::var("POSTGRES_PORT")?,
    env::var("POSTGRES_DB")?,
);

let backend = PostgresStateBackend::new(
    &database_url,
    "processor_state"
).await?;
```

## Running Tests

### Unit Tests

```bash
# Run all tests
cargo test --package processor

# Run specific test file
cargo test --package processor --test state_integration_tests
```

### Integration Tests (Require Services)

```bash
# Start services
docker-compose up -d redis postgres

# Run integration tests
cargo test --package processor --test distributed_state_tests -- --ignored

# Run specific test
cargo test --package processor --test distributed_state_tests test_redis_basic_operations -- --ignored

# Run with single thread (for lock tests)
cargo test --package processor --test distributed_state_tests -- --ignored --test-threads=1

# Run with verbose output
cargo test --package processor --test distributed_state_tests -- --ignored --nocapture
```

### Performance Benchmarks

```bash
# Run benchmark tests
cargo test --package processor --test distributed_state_tests bench_ -- --ignored --nocapture

# Example: Redis write throughput
cargo test --package processor --test distributed_state_tests bench_redis_write_throughput -- --ignored --nocapture
```

## Running Examples

### List Available Examples

```bash
ls crates/processor/examples/
```

### Run Examples

```bash
# Start services
docker-compose up -d redis postgres

# Redis state demo
cargo run --package processor --example redis_state_demo

# PostgreSQL state demo
cargo run --package processor --example postgres_state_demo

# Distributed lock demo
cargo run --package processor --example distributed_lock_demo

# Tiered cache demo
cargo run --package processor --example tiered_cache_demo

# Run multiple instances of lock demo (in separate terminals)
cargo run --package processor --example distributed_lock_demo
```

## Common Patterns

### Pattern 1: Cache-Aside

```rust
async fn get_user(
    cache: &impl StateBackend,
    db: &impl StateBackend,
    user_id: u64,
) -> anyhow::Result<Option<User>> {
    let key = format!("user:{}", user_id).into_bytes();

    // Try cache first
    if let Some(cached) = cache.get(&key).await? {
        return Ok(Some(serde_json::from_slice(&cached)?));
    }

    // Fetch from database
    if let Some(data) = db.get(&key).await? {
        // Update cache
        cache.put_with_ttl(&key, &data, Duration::from_secs(3600)).await?;
        return Ok(Some(serde_json::from_slice(&data)?));
    }

    Ok(None)
}
```

### Pattern 2: Write-Through Cache

```rust
async fn save_user(
    cache: &impl StateBackend,
    db: &impl StateBackend,
    user: &User,
) -> anyhow::Result<()> {
    let key = format!("user:{}", user.id).into_bytes();
    let data = serde_json::to_vec(user)?;

    // Write to database
    db.put(&key, &data).await?;

    // Update cache
    cache.put_with_ttl(&key, &data, Duration::from_secs(3600)).await?;

    Ok(())
}
```

### Pattern 3: Distributed Lock with Work

```rust
async fn process_with_lock(
    lock: &RedisLock,
    work: impl Future<Output = anyhow::Result<()>>,
) -> anyhow::Result<()> {
    lock.with_lock(Duration::from_secs(30), || async {
        work.await
    }).await
}
```

### Pattern 4: Leader Election

```rust
async fn leader_election_loop(
    lock: &RedisLock,
    leader_work: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    loop {
        if lock.try_lock(Duration::from_secs(10)).await? {
            println!("I am the leader!");

            // Do leader work
            leader_work()?;

            // Sleep before renewing
            tokio::time::sleep(Duration::from_secs(5)).await;

            // Renew lock
            lock.renew(Duration::from_secs(10)).await?;
        } else {
            println!("I am a follower");
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }
}
```

### Pattern 5: Batch Operations

```rust
async fn batch_save_users(
    backend: &RedisStateBackend,
    users: Vec<User>,
) -> anyhow::Result<()> {
    // Prepare batch
    let pairs: Vec<(Vec<u8>, Vec<u8>)> = users
        .iter()
        .map(|user| {
            let key = format!("user:{}", user.id).into_bytes();
            let value = serde_json::to_vec(user).unwrap();
            (key, value)
        })
        .collect();

    // Convert to references
    let pair_refs: Vec<(&[u8], &[u8])> = pairs
        .iter()
        .map(|(k, v)| (k.as_slice(), v.as_slice()))
        .collect();

    // Batch insert
    backend.batch_put(&pair_refs).await?;

    Ok(())
}
```

## Troubleshooting

### Issue: Cannot Connect to Redis

**Error**: `Connection refused` or `Failed to connect to Redis`

**Solution**:
```bash
# Check if Redis is running
docker-compose ps redis

# Start Redis
docker-compose up -d redis

# Check Redis logs
docker-compose logs redis

# Test connection
redis-cli ping

# If using custom port/host
redis-cli -h localhost -p 6379 ping
```

### Issue: Cannot Connect to PostgreSQL

**Error**: `Connection refused` or `Authentication failed`

**Solution**:
```bash
# Check if PostgreSQL is running
docker-compose ps postgres

# Start PostgreSQL
docker-compose up -d postgres

# Check PostgreSQL logs
docker-compose logs postgres

# Test connection
psql -h localhost -U optimizer -d optimizer

# Check credentials in docker-compose.yml
```

### Issue: Tests Failing

**Error**: `Some tests failed`

**Solution**:
```bash
# Ensure services are running
docker-compose up -d redis postgres

# Wait for services to be ready
sleep 5

# Clean up test data
redis-cli FLUSHDB
psql -h localhost -U optimizer -d optimizer -c "DROP TABLE IF EXISTS test_state;"

# Run tests again
cargo test --package processor --test distributed_state_tests -- --ignored
```

### Issue: Low Cache Hit Rate

**Symptoms**: Poor performance, many cache misses

**Diagnosis**:
```rust
let stats = backend.stats().await;
println!("Hit rate: {:.2}%", stats.hit_rate() * 100.0);
```

**Solution**:
- Increase cache size
- Increase TTL
- Warm up cache on startup
- Analyze access patterns

### Issue: Memory Pressure

**Symptoms**: OOM errors, high memory usage

**Solution**:
```rust
// Reduce cache size
let cached = CachedStateBackend::new(
    backend,
    500,  // Reduced from 1000
    Some(Duration::from_secs(30))  // Shorter TTL
);

// Or implement eviction
backend.clear().await?; // Periodic cleanup
```

### Issue: Connection Pool Exhausted

**Error**: `Too many connections`

**Solution**:
```rust
// Increase pool size
let config = RedisConfig::builder()
    .pool_size(20, 100)  // Increased from 10, 50
    .build()?;

// Or reduce connection lifetime
// Close connections promptly after use
```

## Next Steps

### Learn More

- Read the [Architecture Documentation](crates/processor/src/state/DISTRIBUTED_STATE.md)
- Explore the [examples directory](crates/processor/examples/)
- Review the [API documentation](https://docs.rs/processor)

### Production Deployment

1. **Enable Monitoring**:
   ```rust
   // Implement metrics collection
   let stats = backend.stats().await;
   metrics::gauge!("state.hit_rate", stats.hit_rate());
   metrics::histogram!("state.latency_us", stats.avg_latency_us as f64);
   ```

2. **Configure Alerts**:
   - Low hit rate (< 70%)
   - High latency (> 10ms)
   - Connection pool exhaustion
   - Memory pressure

3. **Setup High Availability**:
   - Use Redis Cluster or Redis Sentinel
   - Configure PostgreSQL replication
   - Implement health checks
   - Setup automatic failover

4. **Performance Testing**:
   ```bash
   # Load testing with k6 or similar
   cargo build --release
   ./target/release/benchmark --backend redis --duration 60s
   ```

5. **Backup and Recovery**:
   ```bash
   # Redis backup
   redis-cli BGSAVE

   # PostgreSQL backup
   pg_dump -h localhost -U optimizer optimizer > backup.sql

   # Restore
   psql -h localhost -U optimizer optimizer < backup.sql
   ```

### Get Help

- GitHub Issues: https://github.com/llm-devops/llm-auto-optimizer/issues
- Discussions: https://github.com/llm-devops/llm-auto-optimizer/discussions
- Documentation: https://docs.rs/processor

---

## Quick Reference

### Service URLs

| Service         | URL                                        |
|----------------|---------------------------------------------|
| Redis          | redis://localhost:6379                      |
| PostgreSQL     | postgres://optimizer:password@localhost:5432/optimizer |
| Redis Commander| http://localhost:8081                       |
| pgAdmin        | http://localhost:5050                       |

### Common Commands

```bash
# Start all services
docker-compose up -d

# Stop all services
docker-compose down

# View logs
docker-compose logs -f [service]

# Restart service
docker-compose restart [service]

# Run tests
cargo test --package processor --test distributed_state_tests -- --ignored

# Run example
cargo run --package processor --example [example_name]

# Clean up
docker-compose down -v  # Remove volumes too
```

### Backend Selection

| Use Case                  | Recommended Backend     |
|---------------------------|------------------------|
| Development               | Memory                 |
| Single Instance           | Sled                   |
| Multi-Instance Cache      | Redis                  |
| Persistent Storage        | PostgreSQL             |
| Optimal Performance       | Memory + Redis + PostgreSQL |
| Distributed Coordination  | Redis Lock             |
