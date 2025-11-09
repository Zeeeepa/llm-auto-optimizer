# Distributed State Architecture

This document provides a comprehensive overview of the distributed state management system for the LLM Auto-Optimizer stream processor.

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [State Backend Implementations](#state-backend-implementations)
- [Backend Comparison](#backend-comparison)
- [Deployment Patterns](#deployment-patterns)
- [Scaling Strategies](#scaling-strategies)
- [Best Practices](#best-practices)
- [Performance Tuning](#performance-tuning)
- [Troubleshooting](#troubleshooting)

## Architecture Overview

### Core Components

The distributed state system consists of several key components:

```
┌─────────────────────────────────────────────────────────────┐
│                    Application Layer                         │
├─────────────────────────────────────────────────────────────┤
│                   StateBackend Trait                         │
├──────────┬──────────┬──────────┬──────────┬────────────────┤
│  Memory  │   Sled   │  Redis   │PostgreSQL│ CachedBackend  │
└──────────┴──────────┴──────────┴──────────┴────────────────┘
     ↓          ↓          ↓          ↓             ↓
   Local     Local    Distributed  Distributed   Tiered
   Cache     Disk      Cache       Database       Cache
```

### StateBackend Trait

All state backends implement the `StateBackend` trait, which provides:

- **get(key)** - Retrieve value by key
- **put(key, value)** - Store key-value pair
- **delete(key)** - Remove key
- **list_keys(prefix)** - List keys with prefix
- **clear()** - Remove all state
- **count()** - Count total keys
- **contains(key)** - Check if key exists

### Design Principles

1. **Abstraction** - Uniform interface across all backends
2. **Async-First** - All operations are async for non-blocking I/O
3. **Type Safety** - Generic serialization/deserialization
4. **Fault Tolerance** - Automatic retries and error handling
5. **Observability** - Built-in metrics and logging

## State Backend Implementations

### 1. Memory Backend

**Purpose**: In-process caching with TTL support

**Features**:
- Fastest access (no I/O)
- Optional TTL-based expiration
- Automatic cleanup of expired entries
- Thread-safe via `DashMap`

**Use Cases**:
- L1 cache in tiered architecture
- Session data for single instance
- Temporary computation results
- Development and testing

**Limitations**:
- Lost on process restart
- Not shared across instances
- Memory constrained
- No persistence

**Configuration**:
```rust
use processor::state::MemoryStateBackend;
use std::time::Duration;

let backend = MemoryStateBackend::new(Some(Duration::from_secs(3600)));
```

### 2. Sled Backend

**Purpose**: Embedded persistent storage

**Features**:
- Persistent across restarts
- ACID transactions
- Fast local disk I/O
- Compression support
- Crash-safe

**Use Cases**:
- Single-instance deployments
- Development environments
- Edge deployments
- Checkpoint storage

**Limitations**:
- Not distributed
- File system dependent
- No network sharing
- Limited scalability

**Configuration**:
```rust
use processor::state::SledStateBackend;

let backend = SledStateBackend::new("/var/lib/optimizer/state")?;
```

### 3. Redis Backend

**Purpose**: Distributed in-memory cache

**Features**:
- High-performance distributed cache
- Automatic TTL expiration
- Connection pooling
- Cluster support
- Atomic operations (LUA scripts)
- Pub/sub capabilities
- Batch operations (pipelining)

**Use Cases**:
- Multi-instance deployments
- Distributed caching
- Session management
- Rate limiting
- Distributed locks

**Advanced Features**:
- **Batch Operations**: `batch_get`, `batch_put`, `batch_delete`
- **Atomic Operations**: `get_and_delete`, `set_if_not_exists`
- **TTL Management**: `put_with_ttl`, `ttl`
- **Checkpointing**: `checkpoint`, `restore`
- **Statistics**: Hit rate, latency, throughput
- **Health Checks**: Connection monitoring

**Configuration**:
```rust
use processor::state::{RedisConfig, RedisStateBackend};
use std::time::Duration;

let config = RedisConfig::builder()
    .url("redis://localhost:6379")
    .key_prefix("optimizer:")
    .default_ttl(Duration::from_secs(3600))
    .pool_size(10, 50)
    .max_retries(3)
    .build()?;

let backend = RedisStateBackend::new(config).await?;
```

### 4. PostgreSQL Backend

**Purpose**: Persistent relational database storage

**Features**:
- Full ACID compliance
- Complex transactions
- SQL querying
- Automatic schema migration
- Connection pooling
- Vacuum and optimization

**Use Cases**:
- Persistent state storage
- Audit trails
- Long-term retention
- Complex queries
- Regulatory compliance

**Advanced Features**:
- **Transactions**: `begin_transaction`, `commit_transaction`, `rollback_transaction`
- **Batch Operations**: `batch_upsert`
- **Maintenance**: `vacuum`, `analyze`
- **Statistics**: Database size, table size, dead tuples
- **Health Checks**: Connection pool monitoring

**Configuration**:
```rust
use processor::state::PostgresStateBackend;

let backend = PostgresStateBackend::new(
    "postgres://user:pass@localhost:5432/db",
    "state_table"
).await?;
```

### 5. Cached Backend

**Purpose**: Multi-tier caching with automatic promotion

**Features**:
- Transparent L1 cache over any backend
- Automatic cache promotion/demotion
- Configurable size and TTL
- LRU eviction
- Hit rate tracking

**Use Cases**:
- Optimizing slow backends
- Reducing database load
- Improving read latency
- Hybrid architectures

**Configuration**:
```rust
use processor::state::{CachedStateBackend, RedisStateBackend};
use std::sync::Arc;
use std::time::Duration;

let redis = Arc::new(RedisStateBackend::new(config).await?);
let cached = CachedStateBackend::new(
    redis,
    1000,                          // Max 1000 items in cache
    Some(Duration::from_secs(60))  // 1-minute cache TTL
);
```

### 6. Distributed Lock (Redis)

**Purpose**: Distributed coordination and mutual exclusion

**Features**:
- Distributed locks with TTL
- Leader election
- Automatic lock renewal
- Deadlock prevention
- RAII pattern support

**Use Cases**:
- Coordinating multiple instances
- Leader election
- Critical sections
- Resource allocation
- Task distribution

**Configuration**:
```rust
use processor::state::{RedisLock, DistributedLock};
use std::time::Duration;

let lock = RedisLock::new("redis://localhost:6379", "my_lock").await?;

// Try to acquire lock
if lock.try_lock(Duration::from_secs(30)).await? {
    // Critical section
    lock.unlock().await?;
}

// Or use RAII pattern
let result = lock.with_lock(Duration::from_secs(30), || async {
    // Critical section
    Ok::<_, anyhow::Error>(42)
}).await?;
```

## Backend Comparison

### Performance Characteristics

| Backend    | Read Latency | Write Latency | Throughput | Persistence | Distribution |
|------------|-------------|---------------|------------|-------------|--------------|
| Memory     | 1-10μs      | 1-10μs        | >1M ops/s  | No          | No           |
| Sled       | 10-100μs    | 50-500μs      | >100K ops/s| Yes         | No           |
| Redis      | 100-500μs   | 100-500μs     | >50K ops/s | Optional    | Yes          |
| PostgreSQL | 1-5ms       | 2-10ms        | >10K ops/s | Yes         | Yes          |
| Cached     | 1-500μs     | 100-500μs     | Variable   | Via Backend | Via Backend  |

*Note: Latencies are approximate and depend on hardware, network, and load*

### Feature Matrix

| Feature                  | Memory | Sled | Redis | PostgreSQL |
|-------------------------|--------|------|-------|------------|
| TTL Support             | ✓      | ✗    | ✓     | ✗          |
| Transactions            | ✗      | ✓    | ✓*    | ✓          |
| Batch Operations        | ✓      | ✓    | ✓     | ✓          |
| Atomic Operations       | ✓      | ✓    | ✓     | ✓          |
| Distributed Locks       | ✗      | ✗    | ✓     | ✓          |
| Persistence             | ✗      | ✓    | ✓*    | ✓          |
| Cross-Instance Sharing  | ✗      | ✗    | ✓     | ✓          |
| Compression             | ✗      | ✓    | ✗     | ✗          |
| SQL Querying            | ✗      | ✗    | ✗     | ✓          |
| Pub/Sub                 | ✗      | ✗    | ✓     | ✓          |
| Clustering              | ✗      | ✗    | ✓     | ✓          |

*✓* = Supported, *✗* = Not supported, *✓** = Optional/Limited

### Selection Guide

Choose backend based on requirements:

**Memory**:
- Need: Fastest possible access
- Can tolerate: Data loss on restart
- Scale: Single instance
- Example: Request deduplication, rate limiting

**Sled**:
- Need: Local persistence
- Can tolerate: No distribution
- Scale: Single instance
- Example: Edge deployments, development

**Redis**:
- Need: Fast distributed cache
- Can tolerate: Potential data loss (if persistence disabled)
- Scale: Multiple instances
- Example: Session state, distributed locks, caching

**PostgreSQL**:
- Need: ACID compliance, persistence
- Can tolerate: Higher latency
- Scale: Multiple instances
- Example: Audit logs, checkpoints, long-term storage

**Cached**:
- Need: Optimize slow backend
- Can tolerate: Eventual consistency
- Scale: Same as underlying backend
- Example: PostgreSQL with memory cache

## Deployment Patterns

### 1. Single Instance (Development)

```
┌────────────────┐
│   Processor    │
│   ┌─────────┐  │
│   │ Memory  │  │
│   └─────────┘  │
└────────────────┘
```

**Backends**: Memory or Sled
**Pros**: Simple, fast
**Cons**: No fault tolerance, no scaling

### 2. Multi-Instance with Shared Redis

```
┌────────────┐     ┌────────────┐     ┌────────────┐
│Processor 1 │     │Processor 2 │     │Processor 3 │
└─────┬──────┘     └─────┬──────┘     └─────┬──────┘
      │                  │                  │
      └──────────────────┼──────────────────┘
                         │
                    ┌────▼────┐
                    │  Redis  │
                    └─────────┘
```

**Backends**: Redis
**Pros**: Horizontal scaling, state sharing
**Cons**: Single point of failure (use Redis Cluster for HA)

### 3. Three-Tier Architecture

```
┌───────────────────────────────────────────────────┐
│                  Processor Instance                │
│                                                   │
│  ┌──────────┐                                     │
│  │ L1: Mem  │ ← In-process cache (fastest)        │
│  └────┬─────┘                                     │
│       │                                           │
│  ┌────▼──────┐                                    │
│  │ L2: Redis │ ← Distributed cache (fast)         │
│  └────┬──────┘                                    │
│       │                                           │
│  ┌────▼──────────┐                                │
│  │ L3: PostgreSQL│ ← Persistent storage (durable) │
│  └───────────────┘                                │
└───────────────────────────────────────────────────┘
```

**Backends**: Memory + Redis + PostgreSQL
**Pros**: Optimal performance, durability, scalability
**Cons**: Complex, higher operational cost

### 4. Read Replicas Pattern

```
┌──────────┐         ┌──────────┐
│Processor │ WRITE   │PostgreSQL│
│ (Leader) │────────▶│  Primary │
└──────────┘         └────┬─────┘
                          │
      ┌───────────────────┼───────────────────┐
      │                   │                   │
┌─────▼─────┐      ┌──────▼──────┐    ┌──────▼──────┐
│PostgreSQL │      │ PostgreSQL  │    │ PostgreSQL  │
│ Replica 1 │      │  Replica 2  │    │  Replica 3  │
└─────▲─────┘      └──────▲──────┘    └──────▲──────┘
      │                   │                   │
┌─────┴─────┐      ┌──────┴──────┐    ┌──────┴──────┐
│Processor  │ READ │  Processor  │READ│  Processor  │
│(Follower) │      │  (Follower) │    │  (Follower) │
└───────────┘      └─────────────┘    └─────────────┘
```

**Backends**: PostgreSQL with replication
**Pros**: Read scalability, high availability
**Cons**: Replication lag, write bottleneck

### 5. Hybrid Pattern (Recommended for Production)

```
┌────────────────────────────────────────────────────┐
│                 Processor Instances                 │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐         │
│  │  Memory  │  │  Memory  │  │  Memory  │  L1     │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘         │
└───────┼─────────────┼─────────────┼────────────────┘
        │             │             │
        └─────────────┼─────────────┘
                      │
           ┌──────────▼───────────┐
           │   Redis Cluster      │  L2
           │  (High Availability) │
           └──────────┬───────────┘
                      │
           ┌──────────▼───────────┐
           │   PostgreSQL         │  L3
           │  (Primary + Replicas)│
           └──────────────────────┘
```

**Backends**: Memory + Redis Cluster + PostgreSQL
**Pros**: Performance, scalability, fault tolerance
**Cons**: Operational complexity

## Scaling Strategies

### Vertical Scaling

**Memory Backend**:
- Increase process memory limit
- Use larger instance types
- Optimize data structures

**Redis Backend**:
- Increase maxmemory
- Use larger instance
- Enable compression

**PostgreSQL Backend**:
- Increase connection pool size
- Tune shared_buffers
- Add more RAM/CPU

### Horizontal Scaling

**Partitioning (Sharding)**:
```rust
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn select_shard(key: &[u8], num_shards: usize) -> usize {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    (hasher.finish() as usize) % num_shards
}

// Use different backends for different shards
let shards = vec![
    redis_backend_1,
    redis_backend_2,
    redis_backend_3,
];

let shard = select_shard(key, shards.len());
shards[shard].get(key).await?;
```

**Read Replicas**:
- Use read replicas for GET operations
- Route writes to primary
- Implement connection pooling

**Caching Layers**:
- Add memory cache (L1)
- Use Redis for distributed cache (L2)
- PostgreSQL for persistence (L3)

### Load Balancing

**Client-Side**:
```rust
use rand::seq::SliceRandom;

let backends = vec![redis1, redis2, redis3];
let backend = backends.choose(&mut rand::thread_rng()).unwrap();
backend.get(key).await?;
```

**Server-Side**:
- Use Redis Cluster (automatic sharding)
- PostgreSQL connection pooler (PgBouncer)
- HAProxy/Nginx for load balancing

## Best Practices

### 1. Key Design

**Use Prefixes for Namespacing**:
```rust
// Good
backend.put(b"user:1001:profile", data).await?;
backend.put(b"session:abc123", data).await?;

// Bad (no namespace)
backend.put(b"1001", data).await?;
```

**Keep Keys Short**:
```rust
// Good
b"u:1001"

// Bad (wastes memory)
b"user_profile_data_for_user_id_1001"
```

**Use Consistent Encoding**:
```rust
// Good (consistent)
let key = format!("user:{:08}", user_id);

// Bad (inconsistent padding)
let key = format!("user:{}", user_id);
```

### 2. Error Handling

**Implement Retries**:
```rust
use tokio::time::{sleep, Duration};

async fn get_with_retry(backend: &impl StateBackend, key: &[u8], retries: u32) -> StateResult<Option<Vec<u8>>> {
    for attempt in 0..retries {
        match backend.get(key).await {
            Ok(value) => return Ok(value),
            Err(e) if attempt < retries - 1 => {
                tracing::warn!("Attempt {} failed: {}, retrying...", attempt + 1, e);
                sleep(Duration::from_millis(100 * 2u64.pow(attempt))).await;
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}
```

**Graceful Degradation**:
```rust
// Fallback to slower backend on cache miss
match memory_cache.get(key).await {
    Ok(Some(value)) => Ok(value),
    _ => postgres.get(key).await,
}
```

### 3. TTL Management

**Set Appropriate TTLs**:
```rust
// Session data: short TTL
backend.put_with_ttl(session_key, data, Duration::from_secs(1800)).await?;

// User profiles: longer TTL
backend.put_with_ttl(profile_key, data, Duration::from_secs(86400)).await?;

// Permanent data: no TTL
backend.put(config_key, data).await?;
```

**Cascading TTLs**:
```rust
// L1: 1 minute
l1.put_with_ttl(key, value, Duration::from_secs(60)).await?;

// L2: 10 minutes
l2.put_with_ttl(key, value, Duration::from_secs(600)).await?;

// L3: Permanent
l3.put(key, value).await?;
```

### 4. Batch Operations

**Use Batching for Efficiency**:
```rust
// Good (single round trip)
backend.batch_put(&pairs).await?;

// Bad (multiple round trips)
for (key, value) in pairs {
    backend.put(key, value).await?;
}
```

### 5. Connection Management

**Use Connection Pooling**:
```rust
let config = RedisConfig::builder()
    .pool_size(10, 50)  // Min 10, max 50 connections
    .build()?;
```

**Monitor Pool Exhaustion**:
```rust
let pool_stats = backend.pool_stats();
if pool_stats.active_connections >= pool_stats.max_connections {
    tracing::warn!("Connection pool exhausted!");
}
```

### 6. Monitoring and Observability

**Track Metrics**:
```rust
let stats = backend.stats().await;
tracing::info!(
    "Hit rate: {:.2}%, Latency: {}μs",
    stats.hit_rate() * 100.0,
    stats.avg_latency_us
);
```

**Health Checks**:
```rust
if !backend.health_check().await? {
    tracing::error!("Backend unhealthy!");
    // Switch to fallback backend
}
```

## Performance Tuning

### Redis Optimization

**1. Enable Pipelining**:
```rust
let config = RedisConfig::builder()
    .pipeline_batch_size(100)
    .build()?;
```

**2. Configure Eviction**:
```
maxmemory 2gb
maxmemory-policy allkeys-lru
```

**3. Persistence Trade-offs**:
```
# No persistence (fastest)
appendonly no

# Balanced
appendonly yes
appendfsync everysec

# Durable (slowest)
appendonly yes
appendfsync always
```

### PostgreSQL Optimization

**1. Tune Connection Pool**:
```rust
// Recommended: connections = (CPU cores * 2) + effective_spindle_count
let pool_size = num_cpus::get() * 2 + 1;
```

**2. Create Indexes**:
```sql
CREATE INDEX idx_state_key ON state_table(key);
CREATE INDEX idx_state_prefix ON state_table(key text_pattern_ops);
```

**3. Vacuum Regularly**:
```rust
// Scheduled vacuum
tokio::spawn(async move {
    loop {
        sleep(Duration::from_secs(3600)).await;
        backend.vacuum().await.ok();
    }
});
```

**4. Tune PostgreSQL Settings**:
```sql
-- Increase shared buffers (25% of RAM)
shared_buffers = 8GB

-- Increase work memory
work_mem = 64MB

-- Enable parallel queries
max_parallel_workers_per_gather = 4
```

### Memory Backend Optimization

**1. Set Appropriate Capacity**:
```rust
// Pre-allocate capacity
let backend = MemoryStateBackend::with_capacity(10000);
```

**2. Configure Cleanup Interval**:
```rust
// More frequent cleanup for short TTLs
let backend = MemoryStateBackend::builder()
    .ttl(Duration::from_secs(60))
    .cleanup_interval(Duration::from_secs(10))
    .build();
```

## Troubleshooting

### Common Issues

**1. High Latency**

*Symptoms*: Slow operations, timeouts

*Diagnosis*:
```rust
let stats = backend.stats().await;
println!("Avg latency: {}μs", stats.avg_latency_us);
```

*Solutions*:
- Add caching layer
- Increase connection pool
- Check network latency
- Optimize queries

**2. Connection Pool Exhaustion**

*Symptoms*: "Too many connections" errors

*Diagnosis*:
```rust
let pool_stats = backend.pool_stats();
println!("Active: {}/{}", pool_stats.active_connections, pool_stats.max_connections);
```

*Solutions*:
- Increase pool size
- Reduce operation duration
- Close connections promptly
- Use connection multiplexing

**3. Memory Pressure**

*Symptoms*: OOM errors, evictions

*Diagnosis*:
```rust
let memory = backend.memory_usage().await?;
println!("Memory usage: {} MB", memory / 1_048_576);
```

*Solutions*:
- Reduce cache size
- Implement TTLs
- Enable eviction policies
- Add more memory

**4. Cache Misses**

*Symptoms*: Low hit rate, poor performance

*Diagnosis*:
```rust
let stats = backend.stats().await;
println!("Hit rate: {:.2}%", stats.hit_rate() * 100.0);
```

*Solutions*:
- Increase cache size
- Adjust TTL
- Warm up cache
- Analyze access patterns

**5. Replication Lag**

*Symptoms*: Stale reads, inconsistency

*Diagnosis*:
```sql
SELECT pg_wal_lsn_diff(pg_current_wal_lsn(), replay_lsn) AS lag_bytes
FROM pg_stat_replication;
```

*Solutions*:
- Increase network bandwidth
- Reduce write load
- Add more replicas
- Use synchronous replication

### Debug Mode

Enable debug logging:
```rust
tracing_subscriber::fmt()
    .with_max_level(tracing::Level::DEBUG)
    .init();
```

Enable Redis command logging:
```
CONFIG SET slowlog-log-slower-than 10000
SLOWLOG GET 10
```

Enable PostgreSQL query logging:
```sql
SET log_statement = 'all';
SET log_min_duration_statement = 100;
```

## Conclusion

The distributed state system provides flexible, scalable, and fault-tolerant state management for the LLM Auto-Optimizer. By understanding the trade-offs between different backends and following best practices, you can build a robust and performant system.

For getting started, see the [Quick Start Guide](../DISTRIBUTED_STATE_GUIDE.md).

For examples, see the `examples/` directory.
