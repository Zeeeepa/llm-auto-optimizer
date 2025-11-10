# Storage Layer User Guide

**Version:** 0.1.0
**Last Updated:** 2025-11-10
**Status:** Production Ready

## Table of Contents

1. [Quick Start Guide](#quick-start-guide)
2. [Overview of Storage Layer](#overview-of-storage-layer)
3. [PostgreSQL Backend Guide](#postgresql-backend-guide)
4. [Redis Backend Guide](#redis-backend-guide)
5. [Sled Backend Guide](#sled-backend-guide)
6. [Multi-Backend Usage](#multi-backend-usage)
7. [Migration Guide](#migration-guide)
8. [Backup and Recovery](#backup-and-recovery)
9. [Monitoring and Metrics](#monitoring-and-metrics)
10. [Troubleshooting](#troubleshooting)
11. [Production Operations Guide](#production-operations-guide)

---

## Quick Start Guide

### Installation

Add the required dependencies to your `Cargo.toml`:

```toml
[dependencies]
llm-optimizer-processor = { path = "crates/processor" }
tokio = { version = "1.40", features = ["full"] }
anyhow = "1.0"
```

### Basic Usage - In-Memory Backend

```rust
use llm_optimizer_processor::state::{MemoryStateBackend, StateBackend};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create in-memory backend with TTL
    let backend = MemoryStateBackend::new(Some(Duration::from_secs(3600)));

    // Store state
    backend.put(b"window:123", b"aggregated_data").await?;

    // Retrieve state
    if let Some(data) = backend.get(b"window:123").await? {
        println!("Retrieved: {:?}", String::from_utf8_lossy(&data));
    }

    // List keys with prefix
    let keys = backend.list_keys(b"window:").await?;
    println!("Found {} windows", keys.len());

    Ok(())
}
```

### Quick Start - PostgreSQL

```rust
use llm_optimizer_processor::state::{PostgresStateBackend, PostgresConfig, StateBackend};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = PostgresConfig::new("postgresql://user:pass@localhost/optimizer")
        .with_pool_size(5, 20)
        .with_ssl_mode("require");

    let backend = PostgresStateBackend::new(config).await?;
    backend.put(b"key", b"value").await?;

    Ok(())
}
```

### Quick Start - Redis

```rust
use llm_optimizer_processor::state::{RedisStateBackend, RedisConfig, StateBackend};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("optimizer:")
        .default_ttl(Duration::from_secs(3600))
        .build()?;

    let backend = RedisStateBackend::new(config).await?;
    backend.put(b"key", b"value").await?;

    Ok(())
}
```

### Quick Start - Sled (Embedded Database)

```rust
use llm_optimizer_processor::state::{SledStateBackend, SledConfig, StateBackend};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = SledConfig::new("/var/lib/optimizer/state")
        .with_cache_capacity(256 * 1024 * 1024) // 256MB
        .with_best_compression();

    let backend = SledStateBackend::open(config).await?;
    backend.put(b"key", b"value").await?;
    backend.flush().await?; // Persist to disk

    Ok(())
}
```

---

## Overview of Storage Layer

### Architecture

The LLM Auto-Optimizer storage layer provides a unified interface (`StateBackend` trait) with multiple backend implementations optimized for different use cases:

```
┌─────────────────────────────────────────┐
│         StateBackend Trait              │
│  (Unified Interface for All Backends)   │
└─────────────────────────────────────────┘
                    │
        ┌───────────┼───────────┐
        │           │           │
┌───────▼────┐ ┌───▼─────┐ ┌──▼──────┐
│ PostgreSQL │ │  Redis  │ │  Sled   │
│  Backend   │ │ Backend │ │ Backend │
└────────────┘ └─────────┘ └─────────┘
     │             │            │
┌────▼────┐  ┌────▼────┐  ┌───▼────┐
│ Cluster │  │Sentinel │  │ Local  │
│ Support │  │Cluster  │  │ Disk   │
└─────────┘  └─────────┘  └────────┘
```

### StateBackend Trait

All backends implement the `StateBackend` trait, providing consistent operations:

```rust
#[async_trait]
pub trait StateBackend: Send + Sync {
    // Core operations
    async fn get(&self, key: &[u8]) -> StateResult<Option<Vec<u8>>>;
    async fn put(&self, key: &[u8], value: &[u8]) -> StateResult<()>;
    async fn delete(&self, key: &[u8]) -> StateResult<()>;

    // Bulk operations
    async fn list_keys(&self, prefix: &[u8]) -> StateResult<Vec<Vec<u8>>>;
    async fn clear(&self) -> StateResult<()>;

    // Metadata operations
    async fn count(&self) -> StateResult<usize>;
    async fn contains(&self, key: &[u8]) -> StateResult<bool>;
}
```

### Backend Comparison Matrix

| Feature | PostgreSQL | Redis | Sled |
|---------|-----------|-------|------|
| **Persistence** | Durable (ACID) | Optional (AOF/RDB) | Durable (Log-structured) |
| **Performance** | Good (100-1k ops/s) | Excellent (10k-100k ops/s) | Very Good (1k-10k ops/s) |
| **Scalability** | Vertical + Horizontal | Horizontal (Cluster) | Vertical Only |
| **Memory Usage** | Low | High (in-memory) | Medium (cache + disk) |
| **Query Capability** | SQL, JSONB, Full-text | Key-value, Pub/Sub | Key-value, Prefix scan |
| **Replication** | Streaming, Logical | Sentinel, Cluster | None (single node) |
| **Transactions** | Full ACID | Limited (MULTI/EXEC) | Single-key atomic |
| **Use Case** | Primary data store | Caching, sessions | Embedded, single-node |
| **Ops Complexity** | High | Medium | Low |
| **Cost** | Medium | High (RAM) | Low |

### When to Use Each Backend

**PostgreSQL** - Choose for:
- Primary system of record
- Complex queries and analytics
- Strong consistency requirements
- Multi-datacenter replication
- Regulatory compliance (audit trails)
- Long-term data retention

**Redis** - Choose for:
- High-performance caching
- Session storage
- Real-time analytics
- Rate limiting and counters
- Pub/Sub messaging
- Distributed locks

**Sled** - Choose for:
- Embedded applications
- Single-node deployments
- Development and testing
- Edge computing
- Cost-sensitive deployments
- Simple key-value storage

---

## PostgreSQL Backend Guide

### Setup and Configuration

#### Basic Configuration

```rust
use llm_optimizer_processor::state::{PostgresConfig, PostgresStateBackend};

let config = PostgresConfig::new("postgresql://user:password@localhost:5432/optimizer")
    .with_pool_size(10, 50)           // min=10, max=50 connections
    .with_ssl_mode("require")          // Enforce SSL
    .with_timeout(Duration::from_secs(30))
    .with_retries(3, Duration::from_millis(100))
    .with_statement_logging(true);

let backend = PostgresStateBackend::new(config).await?;
```

#### Environment-Based Configuration

```bash
# .env file
DATABASE_URL=postgresql://optimizer:secret@postgres.example.com:5432/optimizer?sslmode=require
PG_MIN_CONNECTIONS=5
PG_MAX_CONNECTIONS=20
PG_CONNECT_TIMEOUT=30
PG_QUERY_TIMEOUT=60
```

```rust
use std::env;

let config = PostgresConfig::new(env::var("DATABASE_URL")?)
    .with_pool_size(
        env::var("PG_MIN_CONNECTIONS")?.parse()?,
        env::var("PG_MAX_CONNECTIONS")?.parse()?
    );
```

### Connection Management

#### Connection Pool Tuning

```rust
// For high-throughput applications
let config = PostgresConfig::new(database_url)
    .with_pool_size(20, 100)  // More connections
    .with_timeout(Duration::from_secs(10));

// For low-latency applications
let config = PostgresConfig::new(database_url)
    .with_pool_size(5, 20)    // Fewer connections
    .with_timeout(Duration::from_secs(1));
```

**Pool Sizing Guidelines:**
- Start with: `max_connections = (CPU cores * 2) + disk spindles`
- For read-heavy: Increase max_connections
- For write-heavy: Keep connections moderate, tune batch sizes
- Monitor connection usage and adjust accordingly

#### Connection Health Checks

```rust
use sqlx::postgres::PgPoolOptions;

let pool = PgPoolOptions::new()
    .min_connections(5)
    .max_connections(20)
    .acquire_timeout(Duration::from_secs(30))
    .idle_timeout(Duration::from_secs(600))      // Close idle after 10min
    .max_lifetime(Duration::from_secs(1800))     // Recycle after 30min
    .test_before_acquire(true)                   // Test connection health
    .connect(&database_url)
    .await?;
```

### Schema Management

#### Automatic Migrations

The PostgreSQL backend automatically runs migrations on initialization:

```sql
-- Migration V1: Initial schema
CREATE TABLE IF NOT EXISTS state_entries (
    key BYTEA PRIMARY KEY,
    value BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    metadata JSONB
);

CREATE INDEX idx_state_entries_expires_at ON state_entries(expires_at)
    WHERE expires_at IS NOT NULL;

CREATE INDEX idx_state_entries_metadata ON state_entries USING GIN(metadata)
    WHERE metadata IS NOT NULL;
```

#### Manual Schema Creation

```sql
-- Create database
CREATE DATABASE optimizer;

-- Create dedicated user
CREATE USER optimizer_user WITH PASSWORD 'secure_password';
GRANT ALL PRIVILEGES ON DATABASE optimizer TO optimizer_user;

-- Connect to database
\c optimizer

-- Create schema (if not using public)
CREATE SCHEMA state;
GRANT USAGE ON SCHEMA state TO optimizer_user;
GRANT CREATE ON SCHEMA state TO optimizer_user;

-- Set search path
ALTER USER optimizer_user SET search_path TO state, public;
```

### Advanced Features

#### JSONB Metadata Storage

```rust
use serde_json::json;

// Store state with metadata
backend.put_with_metadata(
    b"window:123",
    b"aggregated_data",
    json!({
        "source": "kafka",
        "partition": 5,
        "offset": 12345,
        "timestamp": "2025-11-10T10:00:00Z"
    })
).await?;

// Query by metadata
let results = sqlx::query!(
    "SELECT key, value FROM state_entries
     WHERE metadata->>'source' = $1
     AND (metadata->>'partition')::int = $2",
    "kafka",
    5
)
.fetch_all(&pool)
.await?;
```

#### TTL and Expiration

```rust
// Set TTL during put
backend.put_with_ttl(
    b"session:abc",
    b"user_data",
    Duration::from_secs(3600)  // Expire after 1 hour
).await?;

// Cleanup expired entries (run periodically)
let deleted = backend.cleanup_expired().await?;
println!("Cleaned up {} expired entries", deleted);
```

#### Checkpointing

```rust
// Create checkpoint
let checkpoint_id = backend.create_checkpoint().await?;
println!("Checkpoint created: {}", checkpoint_id);

// List checkpoints
let checkpoints = backend.list_checkpoints().await?;
for cp in checkpoints {
    println!("Checkpoint {} at {}: {} rows, {} bytes",
        cp.checkpoint_id, cp.created_at, cp.row_count, cp.data_size);
}

// Restore from checkpoint
backend.restore_checkpoint(checkpoint_id).await?;
```

### Performance Tuning

#### PostgreSQL Server Configuration

```ini
# postgresql.conf

# Memory settings
shared_buffers = 4GB              # 25% of system RAM
effective_cache_size = 12GB       # 75% of system RAM
work_mem = 64MB                   # Per operation memory
maintenance_work_mem = 1GB        # For VACUUM, CREATE INDEX

# Connection settings
max_connections = 200
superuser_reserved_connections = 3

# Checkpoint settings
checkpoint_timeout = 15min
checkpoint_completion_target = 0.9
max_wal_size = 4GB
min_wal_size = 1GB

# Query planning
random_page_cost = 1.1           # For SSD
effective_io_concurrency = 200   # For SSD

# Logging
log_min_duration_statement = 1000  # Log slow queries (>1s)
log_line_prefix = '%t [%p]: [%l-1] user=%u,db=%d,app=%a,client=%h '
log_checkpoints = on
log_connections = on
log_disconnections = on
log_lock_waits = on
```

#### Index Optimization

```sql
-- Create covering index for common queries
CREATE INDEX idx_state_entries_key_value ON state_entries(key)
    INCLUDE (value) WHERE expires_at IS NULL OR expires_at > NOW();

-- Prefix index for range queries
CREATE INDEX idx_state_entries_key_prefix ON state_entries(key bytea_pattern_ops);

-- Partial index for active sessions
CREATE INDEX idx_active_sessions ON state_entries(key)
    WHERE key LIKE 'session:%' AND expires_at > NOW();

-- Analyze index usage
SELECT schemaname, tablename, indexname, idx_scan, idx_tup_read, idx_tup_fetch
FROM pg_stat_user_indexes
WHERE schemaname = 'public'
ORDER BY idx_scan ASC;
```

#### Query Performance Analysis

```sql
-- Enable query timing
\timing on

-- Analyze query plan
EXPLAIN (ANALYZE, BUFFERS)
SELECT value FROM state_entries WHERE key = '\x...'::bytea;

-- Check slow queries
SELECT
    query,
    calls,
    total_exec_time,
    mean_exec_time,
    max_exec_time
FROM pg_stat_statements
WHERE mean_exec_time > 1000  -- > 1 second
ORDER BY total_exec_time DESC
LIMIT 20;
```

#### Vacuum and Maintenance

```sql
-- Auto-vacuum configuration (postgresql.conf)
autovacuum = on
autovacuum_max_workers = 4
autovacuum_naptime = 30s
autovacuum_vacuum_threshold = 50
autovacuum_analyze_threshold = 50
autovacuum_vacuum_scale_factor = 0.1
autovacuum_analyze_scale_factor = 0.05

-- Manual vacuum operations
VACUUM ANALYZE state_entries;

-- Full vacuum (requires table lock)
VACUUM FULL state_entries;

-- Reindex if needed
REINDEX TABLE CONCURRENTLY state_entries;
```

### Replication and High Availability

#### Streaming Replication Setup

**Primary Server:**

```ini
# postgresql.conf
wal_level = replica
max_wal_senders = 10
max_replication_slots = 10
synchronous_commit = on
```

```sql
-- Create replication user
CREATE USER replicator WITH REPLICATION ENCRYPTED PASSWORD 'rep_password';

-- Create replication slot
SELECT * FROM pg_create_physical_replication_slot('replica_slot');
```

**Replica Server:**

```bash
# Stop replica
systemctl stop postgresql

# Backup from primary
pg_basebackup -h primary.example.com -D /var/lib/postgresql/data \
    -U replicator -P -v -R -X stream -C -S replica_slot

# Start replica
systemctl start postgresql
```

#### Application-Level Failover

```rust
use llm_optimizer_processor::state::{
    EnhancedPostgresBackend,
    EnhancedPostgresConfig,
    PostgresReplicaConfig
};

let config = EnhancedPostgresConfig::new("postgresql://primary:5432/optimizer")
    .with_replicas(PostgresReplicaConfig {
        replica_urls: vec![
            "postgresql://replica1:5432/optimizer".to_string(),
            "postgresql://replica2:5432/optimizer".to_string(),
        ],
        enable_read_splitting: true,
        max_replication_lag_ms: 1000,
        enable_auto_failover: true,
        ..Default::default()
    });

let backend = EnhancedPostgresBackend::new(config).await?;

// Reads automatically distributed across replicas
let value = backend.get(b"key").await?;

// Writes always go to primary
backend.put(b"key", b"value").await?;
```

### Monitoring Queries

```sql
-- Active connections
SELECT
    datname,
    usename,
    application_name,
    client_addr,
    state,
    query,
    query_start
FROM pg_stat_activity
WHERE datname = 'optimizer';

-- Database size
SELECT
    pg_size_pretty(pg_database_size('optimizer')) as db_size,
    pg_size_pretty(pg_total_relation_size('state_entries')) as table_size;

-- Lock contention
SELECT
    l.locktype,
    l.relation::regclass,
    l.mode,
    l.granted,
    a.query
FROM pg_locks l
JOIN pg_stat_activity a ON l.pid = a.pid
WHERE NOT l.granted;

-- Replication lag
SELECT
    application_name,
    client_addr,
    state,
    sync_state,
    pg_wal_lsn_diff(pg_current_wal_lsn(), replay_lsn) AS lag_bytes,
    pg_wal_lsn_diff(pg_current_wal_lsn(), replay_lsn) / 1024 / 1024 AS lag_mb
FROM pg_stat_replication;
```

---

## Redis Backend Guide

### Setup and Configuration

#### Standalone Configuration

```rust
use llm_optimizer_processor::state::{RedisConfig, RedisStateBackend};

let config = RedisConfig::builder()
    .url("redis://localhost:6379")
    .key_prefix("optimizer:")
    .default_ttl(Duration::from_secs(3600))
    .pool_size(10, 50)
    .timeouts(
        Duration::from_secs(5),   // connect
        Duration::from_secs(3),   // read
        Duration::from_secs(3)    // write
    )
    .retries(3, Duration::from_millis(100), Duration::from_secs(5))
    .build()?;

let backend = RedisStateBackend::new(config).await?;
```

#### Redis Cluster Configuration

```rust
use llm_optimizer_processor::state::redis_backend_enterprise::{
    EnterpriseRedisBackend,
    EnterpriseRedisConfig,
    RedisMode
};

let config = EnterpriseRedisConfig::builder()
    .mode(RedisMode::Cluster {
        nodes: vec![
            "redis://node1:6379".to_string(),
            "redis://node2:6379".to_string(),
            "redis://node3:6379".to_string(),
        ]
    })
    .key_prefix("optimizer:")
    .pool_config(20, 100, Duration::from_secs(30))
    .compression(CompressionAlgorithm::Lz4)
    .build()?;

let backend = EnterpriseRedisBackend::new(config).await?;
```

#### Redis Sentinel Configuration

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
    .pool_config(10, 50, Duration::from_secs(30))
    .enable_metrics(true)
    .build()?;

let backend = EnterpriseRedisBackend::new(config).await?;
```

### Caching Strategies

#### Write-Through Cache

```rust
async fn write_through_cache(
    cache: &RedisStateBackend,
    db: &PostgresStateBackend,
    key: &[u8],
    value: &[u8]
) -> StateResult<()> {
    // Write to database first
    db.put(key, value).await?;

    // Then update cache
    cache.put_with_ttl(key, value, Duration::from_secs(3600)).await?;

    Ok(())
}
```

#### Write-Behind Cache (Async)

```rust
use tokio::sync::mpsc;

async fn write_behind_cache(
    cache: &RedisStateBackend,
    db_tx: &mpsc::Sender<(Vec<u8>, Vec<u8>)>,
    key: &[u8],
    value: &[u8]
) -> StateResult<()> {
    // Update cache immediately
    cache.put_with_ttl(key, value, Duration::from_secs(3600)).await?;

    // Queue database write asynchronously
    db_tx.send((key.to_vec(), value.to_vec())).await
        .map_err(|_| StateError::StorageError {
            backend_type: "redis".to_string(),
            details: "Failed to queue database write".to_string(),
        })?;

    Ok(())
}

// Background worker
async fn db_writer_worker(
    mut rx: mpsc::Receiver<(Vec<u8>, Vec<u8>)>,
    db: Arc<PostgresStateBackend>
) {
    while let Some((key, value)) = rx.recv().await {
        if let Err(e) = db.put(&key, &value).await {
            error!("Database write failed: {}", e);
            // Implement retry logic or dead letter queue
        }
    }
}
```

#### Cache-Aside Pattern

```rust
async fn cache_aside_get(
    cache: &RedisStateBackend,
    db: &PostgresStateBackend,
    key: &[u8]
) -> StateResult<Option<Vec<u8>>> {
    // Try cache first
    if let Some(value) = cache.get(key).await? {
        return Ok(Some(value));
    }

    // Cache miss - fetch from database
    if let Some(value) = db.get(key).await? {
        // Populate cache for next time
        cache.put_with_ttl(key, &value, Duration::from_secs(3600)).await?;
        return Ok(Some(value));
    }

    Ok(None)
}
```

### TTL Management

#### Dynamic TTL Based on Access Patterns

```rust
async fn adaptive_ttl_put(
    backend: &RedisStateBackend,
    key: &[u8],
    value: &[u8],
    access_frequency: u32
) -> StateResult<()> {
    let ttl = match access_frequency {
        0..=10 => Duration::from_secs(300),      // 5 minutes (rarely accessed)
        11..=100 => Duration::from_secs(1800),   // 30 minutes (moderate)
        101..=1000 => Duration::from_secs(3600), // 1 hour (frequent)
        _ => Duration::from_secs(7200),          // 2 hours (very frequent)
    };

    backend.put_with_ttl(key, value, ttl).await
}
```

#### TTL Refresh on Access

```rust
async fn get_and_refresh_ttl(
    backend: &RedisStateBackend,
    key: &[u8],
    ttl: Duration
) -> StateResult<Option<Vec<u8>>> {
    // Get value
    let value = backend.get(key).await?;

    // Refresh TTL if exists
    if value.is_some() {
        backend.expire(key, ttl).await?;
    }

    Ok(value)
}
```

### Pub/Sub Patterns

#### Event Broadcasting

```rust
use redis::aio::PubSub;

// Publisher
async fn publish_state_change(
    backend: &RedisStateBackend,
    channel: &str,
    event: &StateChangeEvent
) -> StateResult<()> {
    let message = serde_json::to_string(event)?;
    backend.publish(channel, message.as_bytes()).await?;
    Ok(())
}

// Subscriber
async fn subscribe_state_changes(
    backend: &RedisStateBackend,
    channel: &str
) -> StateResult<()> {
    let mut pubsub = backend.get_pubsub().await?;
    pubsub.subscribe(channel).await?;

    let mut stream = pubsub.on_message();
    while let Some(msg) = stream.next().await {
        let payload: StateChangeEvent = serde_json::from_slice(msg.get_payload_bytes())?;
        handle_state_change(payload).await?;
    }

    Ok(())
}
```

#### Cache Invalidation

```rust
// Invalidation publisher
async fn invalidate_cache_key(
    backend: &RedisStateBackend,
    key: &[u8]
) -> StateResult<()> {
    // Delete from cache
    backend.delete(key).await?;

    // Notify other instances
    backend.publish(
        "cache:invalidation",
        key
    ).await?;

    Ok(())
}

// Invalidation subscriber (runs on all instances)
async fn listen_cache_invalidations(
    backend: Arc<RedisStateBackend>
) {
    let mut pubsub = backend.get_pubsub().await.unwrap();
    pubsub.subscribe("cache:invalidation").await.unwrap();

    let mut stream = pubsub.on_message();
    while let Some(msg) = stream.next().await {
        let key = msg.get_payload_bytes();
        let _ = backend.delete(key).await;
    }
}
```

### Batch Operations

#### Pipeline for Bulk Writes

```rust
async fn batch_put_pipeline(
    backend: &RedisStateBackend,
    entries: Vec<(&[u8], &[u8])>
) -> StateResult<()> {
    // Process in batches
    const BATCH_SIZE: usize = 100;

    for chunk in entries.chunks(BATCH_SIZE) {
        backend.batch_put(chunk).await?;
    }

    Ok(())
}
```

#### Atomic Multi-Key Operations

```rust
use redis::AsyncCommands;

async fn atomic_swap(
    backend: &RedisStateBackend,
    key1: &[u8],
    key2: &[u8]
) -> StateResult<()> {
    // Using MULTI/EXEC transaction
    let mut conn = backend.get_connection().await?;

    let _: () = redis::pipe()
        .atomic()
        .get(key1)
        .get(key2)
        .del(key1)
        .del(key2)
        .set(key2, key1)
        .set(key1, key2)
        .query_async(&mut conn)
        .await?;

    Ok(())
}
```

### Distributed Locking with Redlock

```rust
use llm_optimizer_processor::state::redis_backend_enterprise::RedlockGuard;

async fn with_distributed_lock<F, T>(
    backend: &EnterpriseRedisBackend,
    resource: &str,
    ttl: Duration,
    f: F
) -> StateResult<T>
where
    F: FnOnce() -> T + Send
{
    // Acquire lock using Redlock algorithm
    let lock = backend.acquire_lock(resource, ttl).await?;

    // Execute critical section
    let result = f();

    // Lock automatically released when guard drops
    drop(lock);

    Ok(result)
}

// Usage
let result = with_distributed_lock(
    &backend,
    "checkpoint:coordinator",
    Duration::from_secs(30),
    || {
        // Only one instance executes this
        perform_checkpoint()
    }
).await?;
```

### Redis Server Configuration

#### Memory Management

```conf
# redis.conf

# Memory limit (adjust based on available RAM)
maxmemory 4gb

# Eviction policy
maxmemory-policy allkeys-lru

# Eviction sample size (higher = more accurate)
maxmemory-samples 10

# Memory allocator
mem-allocator jemalloc
```

#### Persistence Options

```conf
# RDB Snapshots (point-in-time backups)
save 900 1        # After 900s if at least 1 key changed
save 300 10       # After 300s if at least 10 keys changed
save 60 10000     # After 60s if at least 10000 keys changed
stop-writes-on-bgsave-error yes
rdbcompression yes
rdbchecksum yes
dbfilename dump.rdb

# AOF (Append-Only File) - more durable
appendonly yes
appendfilename "appendonly.aof"
appendfsync everysec    # Options: always, everysec, no
no-appendfsync-on-rewrite no
auto-aof-rewrite-percentage 100
auto-aof-rewrite-min-size 64mb
```

#### Performance Tuning

```conf
# Networking
tcp-backlog 511
timeout 300
tcp-keepalive 300
maxclients 10000

# Threading (Redis 6+)
io-threads 4
io-threads-do-reads yes

# Slow log
slowlog-log-slower-than 10000    # Log queries slower than 10ms
slowlog-max-len 128

# Client output buffer limits
client-output-buffer-limit normal 0 0 0
client-output-buffer-limit replica 256mb 64mb 60
client-output-buffer-limit pubsub 32mb 8mb 60
```

### Monitoring Commands

```bash
# Connection info
redis-cli INFO clients

# Memory usage
redis-cli INFO memory
redis-cli MEMORY STATS
redis-cli MEMORY DOCTOR

# Performance stats
redis-cli INFO stats
redis-cli LATENCY DOCTOR

# Slow queries
redis-cli SLOWLOG GET 10

# Key space info
redis-cli INFO keyspace
redis-cli DBSIZE

# Monitor real-time commands
redis-cli MONITOR

# Check replication
redis-cli INFO replication
```

---

## Sled Backend Guide

### Setup and Configuration

#### Basic Setup

```rust
use llm_optimizer_processor::state::{SledConfig, SledStateBackend, StateBackend};

let config = SledConfig::new("/var/lib/optimizer/sled")
    .with_cache_capacity(256 * 1024 * 1024)  // 256MB cache
    .with_balanced_compression()              // Snappy compression
    .with_flush_every(1000);                  // Flush every 1000 ops

let backend = SledStateBackend::open(config).await?;
```

#### Compression Options

```rust
// No compression (fastest)
let config = SledConfig::new(path).without_compression();

// Fast compression (LZ4) - 4x throughput, 2x compression
let config = SledConfig::new(path).with_fast_compression();

// Balanced (Snappy) - 3x throughput, 3x compression
let config = SledConfig::new(path).with_balanced_compression();

// Best compression (Zstd) - 2x throughput, 5x compression
let config = SledConfig::new(path).with_best_compression();

// Custom compression
use llm_optimizer_processor::state::compression::{CompressionAlgorithm, CompressionConfig};

let compression = CompressionConfig {
    algorithm: CompressionAlgorithm::Zstd,
    level: 15,                    // Max compression
    threshold: 512,                // Only compress > 512 bytes
    enable_stats: true,
};

let config = SledConfig::new(path).with_compression(compression);
```

### Key Design Best Practices

#### Hierarchical Key Namespaces

```rust
// Good: Use structured prefixes for efficient scanning
let keys = vec![
    b"window:tumbling:300s:bucket1",
    b"window:tumbling:300s:bucket2",
    b"window:sliding:60s:bucket1",
    b"checkpoint:2025-11-10T10:00:00",
    b"metric:latency:p99",
];

// List all tumbling windows
let tumbling_windows = backend.list_keys(b"window:tumbling:").await?;

// List all checkpoints
let checkpoints = backend.list_keys(b"checkpoint:").await?;
```

#### Key Size Considerations

```rust
// Prefer fixed-width keys for better performance
fn make_key(window_type: &str, duration_secs: u32, bucket_id: u64) -> Vec<u8> {
    format!("{}:{:08}:{:016x}", window_type, duration_secs, bucket_id).into_bytes()
}

// Good: Fixed-width, sortable keys
// "tumbling:00000300:000000000000001a"
// "tumbling:00000300:000000000000001b"

// Bad: Variable-width keys
// "tumbling:300s:bucket_26"
// "tumbling:300s:bucket_27"
```

### Compaction

Sled automatically compacts the log-structured storage, but you can tune the process:

#### Automatic Compaction

```rust
use sled::Config;

let db = Config::new()
    .path(path)
    .cache_capacity(cache_size)
    .segment_size(8 * 1024 * 1024)  // 8MB segments
    .use_compression(false)          // We handle compression
    .flush_every_ms(Some(1000))      // Flush every 1s
    .open()?;
```

#### Manual Compaction Trigger

```rust
// Compact entire database
async fn compact_database(backend: &SledStateBackend) -> StateResult<()> {
    info!("Starting database compaction");
    let start = Instant::now();

    // Create checkpoint (snapshot current state)
    let checkpoint = backend.checkpoint().await?;

    // Clear database
    backend.clear().await?;

    // Restore from checkpoint (writes compacted)
    backend.restore(checkpoint).await?;

    // Flush to disk
    backend.flush().await?;

    info!("Compaction completed in {:?}", start.elapsed());
    Ok(())
}
```

#### Size Monitoring

```rust
use humansize::{FileSize, file_size_opts};

async fn check_db_size(backend: &SledStateBackend) -> StateResult<()> {
    let size = backend.size_on_disk().await?;
    let count = backend.count().await?;

    println!("Database size: {}", size.file_size(file_size_opts::BINARY).unwrap());
    println!("Entry count: {}", count);
    println!("Average entry size: {} bytes", size / count as u64);

    let ratio = backend.compression_ratio().await;
    if let Some(ratio) = ratio {
        println!("Compression ratio: {:.2}%", ratio);
    }

    let saved = backend.space_saved().await;
    println!("Space saved: {}", saved.file_size(file_size_opts::BINARY).unwrap());

    Ok(())
}
```

### Backup Strategies

#### Hot Backup (No Downtime)

```rust
use std::path::Path;
use tokio::fs;

async fn hot_backup(
    backend: &SledStateBackend,
    backup_path: &Path
) -> StateResult<()> {
    info!("Creating hot backup to {:?}", backup_path);

    // Create checkpoint (consistent snapshot)
    let checkpoint = backend.checkpoint().await?;

    // Create backup directory
    fs::create_dir_all(backup_path).await?;

    // Write checkpoint to backup file
    let backup_file = backup_path.join("checkpoint.bincode");
    let encoded = bincode::serialize(&checkpoint)?;
    fs::write(&backup_file, encoded).await?;

    info!("Backup completed: {} entries", checkpoint.len());
    Ok(())
}
```

#### Incremental Backup

```rust
use chrono::Utc;

async fn incremental_backup(
    backend: &SledStateBackend,
    backup_path: &Path,
    last_backup_time: DateTime<Utc>
) -> StateResult<()> {
    // Get all keys modified since last backup
    let all_keys = backend.list_keys(b"").await?;
    let mut changed = Vec::new();

    for key in all_keys {
        // Get metadata to check modification time
        if let Some(value) = backend.get(&key).await? {
            changed.push((key, value));
        }
    }

    // Write incremental backup
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let backup_file = backup_path.join(format!("incremental_{}.bincode", timestamp));
    let encoded = bincode::serialize(&changed)?;
    fs::write(&backup_file, encoded).await?;

    info!("Incremental backup: {} changed entries", changed.len());
    Ok(())
}
```

#### Restore from Backup

```rust
async fn restore_from_backup(
    backend: &SledStateBackend,
    backup_file: &Path
) -> StateResult<()> {
    info!("Restoring from backup: {:?}", backup_file);

    // Read backup file
    let encoded = fs::read(backup_file).await?;
    let checkpoint: Vec<(Vec<u8>, Vec<u8>)> = bincode::deserialize(&encoded)?;

    // Restore to backend
    backend.restore(checkpoint).await?;

    info!("Restore completed");
    Ok(())
}
```

### Directory Structure

```
/var/lib/optimizer/sled/
├── conf                 # Configuration
├── db/                  # Main database directory
│   ├── snap.00000001   # Snapshot files
│   ├── snap.00000002
│   └── log.segments/   # Log segments
│       ├── 0000000000000000
│       ├── 0000000000000001
│       └── ...
└── backups/            # Backup directory
    ├── checkpoint.bincode
    └── incremental_*.bincode
```

### Performance Optimization

#### Cache Tuning

```rust
// For read-heavy workloads
let config = SledConfig::new(path)
    .with_cache_capacity(1024 * 1024 * 1024);  // 1GB cache

// For write-heavy workloads
let config = SledConfig::new(path)
    .with_cache_capacity(256 * 1024 * 1024)    // Smaller cache
    .with_flush_every(100);                     // Flush more frequently
```

#### Batch Operations

```rust
async fn batch_insert(
    backend: &SledStateBackend,
    entries: Vec<(Vec<u8>, Vec<u8>)>
) -> StateResult<()> {
    // Batch writes (no intermediate flushes)
    for (key, value) in entries {
        backend.put(&key, &value).await?;
    }

    // Single flush at the end
    backend.flush().await?;

    Ok(())
}
```

### Statistics and Monitoring

```rust
use llm_optimizer_processor::state::SledBackendStats;

async fn print_stats(backend: &SledStateBackend) {
    let stats = backend.stats().await;

    println!("Sled Backend Statistics:");
    println!("  Operations:");
    println!("    GET:    {}", stats.get_count);
    println!("    PUT:    {}", stats.put_count);
    println!("    DELETE: {}", stats.delete_count);
    println!("    FLUSH:  {}", stats.flush_count);

    println!("  Data Transfer:");
    println!("    Bytes read:    {}", humansize(stats.bytes_read));
    println!("    Bytes written: {}", humansize(stats.bytes_written));

    println!("  Compression:");
    println!("    Before: {}", humansize(stats.bytes_before_compression));
    println!("    After:  {}", humansize(stats.bytes_after_compression));

    if let Some(ratio) = backend.compression_ratio().await {
        println!("    Ratio:  {:.2}%", ratio);
    }

    let saved = backend.space_saved().await;
    println!("    Saved:  {}", humansize(saved));
}

fn humansize(bytes: u64) -> String {
    use humansize::{FileSize, file_size_opts};
    bytes.file_size(file_size_opts::BINARY).unwrap()
}
```

---

## Multi-Backend Usage

### Hybrid Architecture Patterns

#### Pattern 1: Redis Cache + PostgreSQL Database

```rust
use std::sync::Arc;

pub struct HybridBackend {
    cache: Arc<RedisStateBackend>,
    database: Arc<PostgresStateBackend>,
}

impl HybridBackend {
    pub async fn new(
        redis_config: RedisConfig,
        postgres_config: PostgresConfig
    ) -> StateResult<Self> {
        Ok(Self {
            cache: Arc::new(RedisStateBackend::new(redis_config).await?),
            database: Arc::new(PostgresStateBackend::new(postgres_config).await?),
        })
    }

    // Read with cache-aside pattern
    pub async fn get(&self, key: &[u8]) -> StateResult<Option<Vec<u8>>> {
        // Try cache first
        if let Some(value) = self.cache.get(key).await? {
            return Ok(Some(value));
        }

        // Cache miss - fetch from database
        if let Some(value) = self.database.get(key).await? {
            // Populate cache asynchronously
            let cache = self.cache.clone();
            let key = key.to_vec();
            let value_clone = value.clone();
            tokio::spawn(async move {
                let _ = cache.put_with_ttl(&key, &value_clone, Duration::from_secs(3600)).await;
            });

            return Ok(Some(value));
        }

        Ok(None)
    }

    // Write-through to both
    pub async fn put(&self, key: &[u8], value: &[u8]) -> StateResult<()> {
        // Write to database first (source of truth)
        self.database.put(key, value).await?;

        // Then update cache
        self.cache.put_with_ttl(key, value, Duration::from_secs(3600)).await?;

        Ok(())
    }

    // Invalidate cache on delete
    pub async fn delete(&self, key: &[u8]) -> StateResult<()> {
        // Delete from cache first (fail-safe)
        let _ = self.cache.delete(key).await;

        // Then delete from database
        self.database.delete(key).await
    }
}
```

#### Pattern 2: Sled Local + PostgreSQL Shared

```rust
pub struct LocalCachedBackend {
    local: Arc<SledStateBackend>,
    shared: Arc<PostgresStateBackend>,
}

impl LocalCachedBackend {
    // Read from local first, fallback to shared
    pub async fn get(&self, key: &[u8]) -> StateResult<Option<Vec<u8>>> {
        // Check local cache
        if let Some(value) = self.local.get(key).await? {
            return Ok(Some(value));
        }

        // Fetch from shared database
        if let Some(value) = self.shared.get(key).await? {
            // Cache locally
            self.local.put(key, &value).await?;
            return Ok(Some(value));
        }

        Ok(None)
    }

    // Write to both
    pub async fn put(&self, key: &[u8], value: &[u8]) -> StateResult<()> {
        // Write to shared first
        self.shared.put(key, value).await?;

        // Update local cache
        self.local.put(key, value).await?;

        Ok(())
    }
}
```

### Failover Strategies

#### Active-Passive Failover

```rust
pub struct FailoverBackend {
    primary: Arc<PostgresStateBackend>,
    secondary: Arc<SledStateBackend>,
    health_check_interval: Duration,
    primary_healthy: Arc<AtomicBool>,
}

impl FailoverBackend {
    pub async fn new(
        primary: PostgresStateBackend,
        secondary: SledStateBackend
    ) -> Self {
        let primary_healthy = Arc::new(AtomicBool::new(true));
        let backend = Self {
            primary: Arc::new(primary),
            secondary: Arc::new(secondary),
            health_check_interval: Duration::from_secs(10),
            primary_healthy: primary_healthy.clone(),
        };

        // Start health check task
        let primary = backend.primary.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(10)).await;
                let healthy = Self::check_health(&primary).await;
                primary_healthy.store(healthy, Ordering::Relaxed);
            }
        });

        backend
    }

    async fn check_health(backend: &PostgresStateBackend) -> bool {
        // Simple health check
        backend.get(b"_health_check").await.is_ok()
    }

    pub async fn get(&self, key: &[u8]) -> StateResult<Option<Vec<u8>>> {
        if self.primary_healthy.load(Ordering::Relaxed) {
            // Try primary
            match self.primary.get(key).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    warn!("Primary backend failed, falling back to secondary: {}", e);
                    self.primary_healthy.store(false, Ordering::Relaxed);
                }
            }
        }

        // Fallback to secondary
        self.secondary.get(key).await
    }
}
```

#### Retry with Exponential Backoff

```rust
use tokio::time::{sleep, Duration};

pub async fn resilient_get(
    backends: Vec<Arc<dyn StateBackend>>,
    key: &[u8],
    max_retries: u32
) -> StateResult<Option<Vec<u8>>> {
    for backend in backends {
        let mut retries = 0;
        let mut delay = Duration::from_millis(100);

        while retries < max_retries {
            match backend.get(key).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    warn!("Backend operation failed (attempt {}): {}", retries + 1, e);
                    retries += 1;

                    if retries < max_retries {
                        sleep(delay).await;
                        delay *= 2; // Exponential backoff
                    }
                }
            }
        }
    }

    Err(StateError::StorageError {
        backend_type: "multi".to_string(),
        details: "All backends failed after retries".to_string(),
    })
}
```

### Consistency Considerations

#### Eventual Consistency

```rust
// Async replication with eventual consistency
pub async fn eventually_consistent_put(
    primary: &PostgresStateBackend,
    replicas: &[SledStateBackend],
    key: &[u8],
    value: &[u8]
) -> StateResult<()> {
    // Write to primary (synchronous)
    primary.put(key, value).await?;

    // Replicate to secondaries (asynchronous, best-effort)
    for replica in replicas {
        let replica = replica.clone();
        let key = key.to_vec();
        let value = value.to_vec();
        tokio::spawn(async move {
            if let Err(e) = replica.put(&key, &value).await {
                error!("Replication failed: {}", e);
            }
        });
    }

    Ok(())
}
```

#### Strong Consistency

```rust
// Synchronous replication with quorum
pub async fn strong_consistent_put(
    backends: &[Arc<dyn StateBackend>],
    key: &[u8],
    value: &[u8],
    quorum: usize
) -> StateResult<()> {
    let mut handles = Vec::new();

    // Write to all backends in parallel
    for backend in backends {
        let backend = backend.clone();
        let key = key.to_vec();
        let value = value.to_vec();

        let handle = tokio::spawn(async move {
            backend.put(&key, &value).await
        });
        handles.push(handle);
    }

    // Wait for quorum successes
    let mut successes = 0;
    for handle in handles {
        if handle.await.is_ok() {
            successes += 1;
        }
    }

    if successes >= quorum {
        Ok(())
    } else {
        Err(StateError::StorageError {
            backend_type: "multi".to_string(),
            details: format!("Quorum not reached: {}/{}", successes, quorum),
        })
    }
}
```

---

## Migration Guide

### PostgreSQL to Sled Migration

```rust
async fn migrate_postgres_to_sled(
    source: &PostgresStateBackend,
    target: &SledStateBackend,
    batch_size: usize
) -> StateResult<()> {
    info!("Starting PostgreSQL to Sled migration");

    let mut offset = 0;
    let mut total_migrated = 0;

    loop {
        // Fetch batch from PostgreSQL
        let entries = source.get_range(offset, batch_size).await?;
        if entries.is_empty() {
            break;
        }

        // Write batch to Sled
        for (key, value) in entries {
            target.put(&key, &value).await?;
            total_migrated += 1;
        }

        // Flush periodically
        if total_migrated % 10000 == 0 {
            target.flush().await?;
            info!("Migrated {} entries", total_migrated);
        }

        offset += batch_size;
    }

    // Final flush
    target.flush().await?;

    info!("Migration complete: {} entries", total_migrated);
    Ok(())
}
```

### Redis to PostgreSQL Migration

```rust
async fn migrate_redis_to_postgres(
    source: &RedisStateBackend,
    target: &PostgresStateBackend
) -> StateResult<()> {
    info!("Starting Redis to PostgreSQL migration");

    // Get all keys from Redis
    let keys = source.list_keys(b"").await?;
    let total = keys.len();
    let mut migrated = 0;

    // Process in batches
    const BATCH_SIZE: usize = 1000;
    for chunk in keys.chunks(BATCH_SIZE) {
        // Fetch values
        let mut entries = Vec::new();
        for key in chunk {
            if let Some(value) = source.get(key).await? {
                entries.push((key.clone(), value));
            }
        }

        // Batch insert to PostgreSQL
        target.batch_put(&entries).await?;

        migrated += entries.len();
        info!("Migration progress: {}/{} ({:.1}%)",
            migrated, total, (migrated as f64 / total as f64) * 100.0);
    }

    info!("Migration complete: {} entries", migrated);
    Ok(())
}
```

### Zero-Downtime Migration

```rust
pub struct DualWriteBackend {
    old: Arc<dyn StateBackend>,
    new: Arc<dyn StateBackend>,
    read_from_new: AtomicBool,
}

impl DualWriteBackend {
    pub fn new(old: Arc<dyn StateBackend>, new: Arc<dyn StateBackend>) -> Self {
        Self {
            old,
            new,
            read_from_new: AtomicBool::new(false),
        }
    }

    // Dual-write phase
    pub async fn put(&self, key: &[u8], value: &[u8]) -> StateResult<()> {
        // Write to both backends
        let old_result = self.old.put(key, value).await;
        let new_result = self.new.put(key, value).await;

        // Require old backend to succeed, new is best-effort
        old_result?;

        if let Err(e) = new_result {
            warn!("New backend write failed: {}", e);
        }

        Ok(())
    }

    // Read from appropriate backend
    pub async fn get(&self, key: &[u8]) -> StateResult<Option<Vec<u8>>> {
        if self.read_from_new.load(Ordering::Relaxed) {
            // Read from new, fallback to old if missing
            if let Ok(Some(value)) = self.new.get(key).await {
                return Ok(Some(value));
            }
            self.old.get(key).await
        } else {
            // Still reading from old
            self.old.get(key).await
        }
    }

    // Switch reads to new backend
    pub fn switch_reads_to_new(&self) {
        self.read_from_new.store(true, Ordering::Relaxed);
        info!("Switched reads to new backend");
    }
}

// Migration process
async fn zero_downtime_migration(
    old_backend: Arc<dyn StateBackend>,
    new_backend: Arc<dyn StateBackend>
) -> StateResult<()> {
    // Phase 1: Dual write (writes go to both, reads from old)
    let dual_backend = Arc::new(DualWriteBackend::new(
        old_backend.clone(),
        new_backend.clone()
    ));
    info!("Phase 1: Dual write enabled");

    // Phase 2: Backfill data from old to new
    info!("Phase 2: Backfilling data");
    backfill_data(&old_backend, &new_backend).await?;

    // Phase 3: Switch reads to new backend
    info!("Phase 3: Switching reads to new backend");
    dual_backend.switch_reads_to_new();

    // Phase 4: Monitor for issues
    tokio::time::sleep(Duration::from_secs(300)).await;

    // Phase 5: Stop dual writes (writes only to new)
    info!("Phase 5: Migration complete, old backend can be decommissioned");

    Ok(())
}

async fn backfill_data(
    old: &Arc<dyn StateBackend>,
    new: &Arc<dyn StateBackend>
) -> StateResult<()> {
    let keys = old.list_keys(b"").await?;

    for key in keys {
        // Check if already exists in new
        if !new.contains(&key).await? {
            // Backfill missing entry
            if let Some(value) = old.get(&key).await? {
                new.put(&key, &value).await?;
            }
        }
    }

    Ok(())
}
```

---

## Backup and Recovery

### Automated Backup System

```rust
use tokio::time::{interval, Duration};
use chrono::{Utc, DateTime};

pub struct BackupScheduler {
    backend: Arc<dyn StateBackend>,
    backup_dir: PathBuf,
    interval: Duration,
    retention_days: i64,
}

impl BackupScheduler {
    pub fn new(
        backend: Arc<dyn StateBackend>,
        backup_dir: PathBuf,
        interval: Duration,
        retention_days: i64
    ) -> Self {
        Self {
            backend,
            backup_dir,
            interval,
            retention_days,
        }
    }

    pub async fn start(self: Arc<Self>) {
        let mut ticker = interval(self.interval);

        loop {
            ticker.tick().await;

            if let Err(e) = self.perform_backup().await {
                error!("Backup failed: {}", e);
            }

            if let Err(e) = self.cleanup_old_backups().await {
                error!("Backup cleanup failed: {}", e);
            }
        }
    }

    async fn perform_backup(&self) -> StateResult<()> {
        let timestamp = Utc::now();
        let backup_name = format!("backup_{}.bincode", timestamp.format("%Y%m%d_%H%M%S"));
        let backup_path = self.backup_dir.join(&backup_name);

        info!("Starting backup to {:?}", backup_path);

        // Create checkpoint
        let checkpoint = self.create_checkpoint().await?;

        // Write to file
        let encoded = bincode::serialize(&checkpoint)?;
        tokio::fs::write(&backup_path, encoded).await?;

        // Compress backup
        let compressed_path = self.backup_dir.join(format!("{}.zst", backup_name));
        self.compress_backup(&backup_path, &compressed_path).await?;

        // Remove uncompressed backup
        tokio::fs::remove_file(&backup_path).await?;

        info!("Backup completed: {:?}", compressed_path);

        Ok(())
    }

    async fn create_checkpoint(&self) -> StateResult<Vec<(Vec<u8>, Vec<u8>)>> {
        let keys = self.backend.list_keys(b"").await?;
        let mut checkpoint = Vec::with_capacity(keys.len());

        for key in keys {
            if let Some(value) = self.backend.get(&key).await? {
                checkpoint.push((key, value));
            }
        }

        Ok(checkpoint)
    }

    async fn compress_backup(&self, src: &Path, dst: &Path) -> StateResult<()> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use async_compression::tokio::write::ZstdEncoder;

        let input = tokio::fs::read(src).await?;
        let mut encoder = ZstdEncoder::new(tokio::fs::File::create(dst).await?);
        encoder.write_all(&input).await?;
        encoder.shutdown().await?;

        Ok(())
    }

    async fn cleanup_old_backups(&self) -> StateResult<()> {
        let cutoff = Utc::now() - chrono::Duration::days(self.retention_days);

        let mut entries = tokio::fs::read_dir(&self.backup_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let metadata = entry.metadata().await?;
            if let Ok(modified) = metadata.modified() {
                let modified: DateTime<Utc> = modified.into();

                if modified < cutoff {
                    info!("Removing old backup: {:?}", entry.path());
                    tokio::fs::remove_file(entry.path()).await?;
                }
            }
        }

        Ok(())
    }
}

// Usage
let scheduler = Arc::new(BackupScheduler::new(
    backend.clone(),
    PathBuf::from("/var/backups/optimizer"),
    Duration::from_secs(3600),  // Hourly backups
    7                            // Keep 7 days
));

tokio::spawn(async move {
    scheduler.start().await;
});
```

### Point-in-Time Recovery

```rust
pub struct PointInTimeRecover {
    backup_dir: PathBuf,
}

impl PointInTimeRecover {
    pub async fn list_backups(&self) -> StateResult<Vec<BackupInfo>> {
        let mut backups = Vec::new();

        let mut entries = tokio::fs::read_dir(&self.backup_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.extension() == Some(OsStr::new("zst")) {
                let metadata = entry.metadata().await?;
                let modified: DateTime<Utc> = metadata.modified()?.into();
                let size = metadata.len();

                backups.push(BackupInfo {
                    path,
                    timestamp: modified,
                    size,
                });
            }
        }

        backups.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(backups)
    }

    pub async fn restore_to_time(
        &self,
        backend: &dyn StateBackend,
        target_time: DateTime<Utc>
    ) -> StateResult<()> {
        // Find the latest backup before target time
        let backups = self.list_backups().await?;
        let backup = backups.iter()
            .find(|b| b.timestamp <= target_time)
            .ok_or_else(|| StateError::StorageError {
                backend_type: "backup".to_string(),
                details: "No backup found before target time".to_string(),
            })?;

        info!("Restoring from backup at {}", backup.timestamp);

        // Decompress and restore
        self.restore_from_backup(backend, &backup.path).await?;

        Ok(())
    }

    async fn restore_from_backup(
        &self,
        backend: &dyn StateBackend,
        backup_path: &Path
    ) -> StateResult<()> {
        use async_compression::tokio::read::ZstdDecoder;
        use tokio::io::AsyncReadExt;

        // Decompress
        let file = tokio::fs::File::open(backup_path).await?;
        let mut decoder = ZstdDecoder::new(file);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).await?;

        // Deserialize
        let checkpoint: Vec<(Vec<u8>, Vec<u8>)> = bincode::deserialize(&decompressed)?;

        // Clear current state
        backend.clear().await?;

        // Restore entries
        for (key, value) in checkpoint {
            backend.put(&key, &value).await?;
        }

        info!("Restored {} entries", checkpoint.len());
        Ok(())
    }
}

#[derive(Debug)]
pub struct BackupInfo {
    pub path: PathBuf,
    pub timestamp: DateTime<Utc>,
    pub size: u64,
}
```

---

## Monitoring and Metrics

### Prometheus Metrics Export

```rust
use prometheus_client::registry::Registry;
use prometheus_client::metrics::{counter::Counter, gauge::Gauge, histogram::Histogram};
use prometheus_client::encoding::text::encode;

pub struct StorageMetrics {
    // Operation counters
    gets: Counter,
    puts: Counter,
    deletes: Counter,
    errors: Counter,

    // Performance metrics
    operation_duration: Histogram,

    // Resource metrics
    active_connections: Gauge,
    cache_size: Gauge,
    disk_usage: Gauge,
}

impl StorageMetrics {
    pub fn new() -> (Self, Registry) {
        let mut registry = Registry::default();

        let metrics = Self {
            gets: Counter::default(),
            puts: Counter::default(),
            deletes: Counter::default(),
            errors: Counter::default(),
            operation_duration: Histogram::new(
                exponential_buckets(0.001, 2.0, 10)  // 1ms to ~1s
            ),
            active_connections: Gauge::default(),
            cache_size: Gauge::default(),
            disk_usage: Gauge::default(),
        };

        // Register metrics
        registry.register("storage_gets_total", "Total GET operations", metrics.gets.clone());
        registry.register("storage_puts_total", "Total PUT operations", metrics.puts.clone());
        registry.register("storage_deletes_total", "Total DELETE operations", metrics.deletes.clone());
        registry.register("storage_errors_total", "Total errors", metrics.errors.clone());
        registry.register("storage_operation_duration_seconds", "Operation duration",
            metrics.operation_duration.clone());
        registry.register("storage_active_connections", "Active connections",
            metrics.active_connections.clone());
        registry.register("storage_cache_size_bytes", "Cache size",
            metrics.cache_size.clone());
        registry.register("storage_disk_usage_bytes", "Disk usage",
            metrics.disk_usage.clone());

        (metrics, registry)
    }

    pub fn record_get(&self, duration: Duration) {
        self.gets.inc();
        self.operation_duration.observe(duration.as_secs_f64());
    }

    pub fn record_error(&self) {
        self.errors.inc();
    }
}

// HTTP endpoint for Prometheus scraping
use axum::{routing::get, Router};

async fn metrics_handler(registry: Arc<Registry>) -> String {
    let mut buffer = String::new();
    encode(&mut buffer, &registry).unwrap();
    buffer
}

pub fn metrics_router(registry: Arc<Registry>) -> Router {
    Router::new()
        .route("/metrics", get(move || metrics_handler(registry.clone())))
}
```

### Health Check Endpoints

```rust
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct HealthStatus {
    status: String,
    backends: Vec<BackendHealth>,
}

#[derive(Serialize)]
pub struct BackendHealth {
    name: String,
    status: String,
    latency_ms: u64,
    error: Option<String>,
}

pub async fn health_check(
    backends: Vec<(String, Arc<dyn StateBackend>)>
) -> (StatusCode, Json<HealthStatus>) {
    let mut backend_health = Vec::new();
    let mut all_healthy = true;

    for (name, backend) in backends {
        let start = Instant::now();
        let result = backend.get(b"_health").await;
        let latency_ms = start.elapsed().as_millis() as u64;

        let (status, error) = match result {
            Ok(_) => ("healthy".to_string(), None),
            Err(e) => {
                all_healthy = false;
                ("unhealthy".to_string(), Some(e.to_string()))
            }
        };

        backend_health.push(BackendHealth {
            name,
            status,
            latency_ms,
            error,
        });
    }

    let status_code = if all_healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let response = HealthStatus {
        status: if all_healthy { "healthy".to_string() } else { "degraded".to_string() },
        backends: backend_health,
    };

    (status_code, Json(response))
}
```

### Logging Best Practices

```rust
use tracing::{info, warn, error, debug, instrument};

#[instrument(skip(backend, value))]
pub async fn instrumented_put(
    backend: &dyn StateBackend,
    key: &[u8],
    value: &[u8]
) -> StateResult<()> {
    let key_str = String::from_utf8_lossy(key);
    debug!("PUT operation starting for key: {}", key_str);

    let start = Instant::now();
    let result = backend.put(key, value).await;
    let duration = start.elapsed();

    match &result {
        Ok(_) => {
            info!("PUT success: key={}, size={}, duration={:?}",
                key_str, value.len(), duration);
        }
        Err(e) => {
            error!("PUT failed: key={}, error={}, duration={:?}",
                key_str, e, duration);
        }
    }

    result
}
```

---

## Troubleshooting

### Common Issues and Solutions

#### Issue 1: PostgreSQL Connection Pool Exhaustion

**Symptoms:**
- "connection pool timeout" errors
- Slow query performance
- Application hangs

**Diagnosis:**
```sql
-- Check active connections
SELECT count(*) as connection_count, state
FROM pg_stat_activity
WHERE datname = 'optimizer'
GROUP BY state;

-- Find long-running queries
SELECT pid, now() - query_start as duration, query
FROM pg_stat_activity
WHERE state = 'active'
ORDER BY duration DESC;
```

**Solutions:**
```rust
// Increase pool size
let config = PostgresConfig::new(database_url)
    .with_pool_size(20, 100);  // Increase max connections

// Set connection timeouts
let config = config
    .with_timeout(Duration::from_secs(10));

// Kill long-running queries
// SELECT pg_terminate_backend(pid) WHERE pid = <pid>;
```

#### Issue 2: Redis Memory Pressure

**Symptoms:**
- OOM errors
- Eviction of hot keys
- High latency

**Diagnosis:**
```bash
redis-cli INFO memory
redis-cli MEMORY STATS
redis-cli MEMORY DOCTOR
```

**Solutions:**
```conf
# Increase maxmemory
maxmemory 8gb

# Tune eviction policy
maxmemory-policy allkeys-lru

# Enable key eviction notifications
notify-keyspace-events Ex
```

```rust
// Implement TTL for all keys
let config = RedisConfig::builder()
    .default_ttl(Duration::from_secs(3600))
    .build()?;

// Monitor memory usage
let info: HashMap<String, String> = redis::cmd("INFO")
    .arg("memory")
    .query_async(&mut conn)
    .await?;
```

#### Issue 3: Sled Corruption After Crash

**Symptoms:**
- "database corrupted" errors
- Missing data after restart
- IO errors

**Diagnosis:**
```rust
// Check database integrity
let config = SledConfig::new(path);
match SledStateBackend::open(config).await {
    Ok(backend) => {
        let count = backend.count().await?;
        println!("Database opened successfully: {} entries", count);
    }
    Err(e) => {
        eprintln!("Database corrupted: {}", e);
    }
}
```

**Solutions:**
```bash
# Restore from latest backup
cp -r /var/backups/optimizer/latest/* /var/lib/optimizer/sled/

# If backup unavailable, try recovery mode
# (Implementation-specific recovery procedures)
```

```rust
// Implement crash recovery
async fn recover_sled(path: &Path, backup_path: &Path) -> StateResult<SledStateBackend> {
    // Try normal open
    match SledStateBackend::open(SledConfig::new(path)).await {
        Ok(backend) => Ok(backend),
        Err(_) => {
            // Restore from backup
            warn!("Database corrupted, restoring from backup");
            tokio::fs::remove_dir_all(path).await?;
            tokio::fs::create_dir_all(path).await?;

            // Copy backup
            copy_dir_all(backup_path, path).await?;

            // Retry open
            SledStateBackend::open(SledConfig::new(path)).await
        }
    }
}
```

### Debug Mode Operations

```rust
// Enable detailed logging
std::env::set_var("RUST_LOG", "llm_optimizer_processor::state=trace");
tracing_subscriber::fmt()
    .with_max_level(tracing::Level::TRACE)
    .init();

// Query performance profiling
use std::time::Instant;

async fn profile_operation<F, T>(name: &str, f: F) -> T
where
    F: Future<Output = T>
{
    let start = Instant::now();
    let result = f.await;
    let duration = start.elapsed();

    eprintln!("{}: {:?}", name, duration);

    result
}

// Usage
let value = profile_operation("get_key", backend.get(b"key")).await?;
```

---

## Production Operations Guide

### Deployment Checklist

#### Pre-Deployment

- [ ] Review configuration settings
- [ ] Set up monitoring and alerting
- [ ] Configure backups
- [ ] Test failover procedures
- [ ] Document runbooks
- [ ] Set up log aggregation
- [ ] Configure resource limits

#### PostgreSQL Production Setup

```bash
# 1. Install PostgreSQL 15+
apt-get install postgresql-15

# 2. Configure postgresql.conf
vim /etc/postgresql/15/main/postgresql.conf

# 3. Configure pg_hba.conf for authentication
vim /etc/postgresql/15/main/pg_hba.conf

# 4. Create database and user
sudo -u postgres psql
CREATE DATABASE optimizer;
CREATE USER optimizer_user WITH ENCRYPTED PASSWORD 'secure_password';
GRANT ALL PRIVILEGES ON DATABASE optimizer TO optimizer_user;

# 5. Enable SSL
cp /path/to/server.crt /etc/postgresql/15/main/
cp /path/to/server.key /etc/postgresql/15/main/
chown postgres:postgres /etc/postgresql/15/main/server.*
chmod 600 /etc/postgresql/15/main/server.key

# Update postgresql.conf
ssl = on
ssl_cert_file = '/etc/postgresql/15/main/server.crt'
ssl_key_file = '/etc/postgresql/15/main/server.key'

# 6. Restart PostgreSQL
systemctl restart postgresql
```

#### Redis Production Setup

```bash
# 1. Install Redis 7+
apt-get install redis-server

# 2. Configure redis.conf
vim /etc/redis/redis.conf

# Key settings:
bind 0.0.0.0
protected-mode yes
requirepass your_strong_password
maxmemory 4gb
maxmemory-policy allkeys-lru
appendonly yes
appendfsync everysec

# 3. Enable systemd service
systemctl enable redis-server
systemctl start redis-server

# 4. Test connection
redis-cli -a your_strong_password PING
```

#### Sled Production Setup

```bash
# 1. Create data directory
mkdir -p /var/lib/optimizer/sled
chown optimizer:optimizer /var/lib/optimizer/sled
chmod 700 /var/lib/optimizer/sled

# 2. Create backup directory
mkdir -p /var/backups/optimizer
chown optimizer:optimizer /var/backups/optimizer

# 3. Set up logrotate
cat > /etc/logrotate.d/optimizer <<EOF
/var/log/optimizer/*.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
}
EOF
```

### Capacity Planning

#### PostgreSQL Sizing

```
Storage Requirements:
- Average entry size: ~500 bytes
- Expected entries: 10,000,000
- Raw storage: 10M * 500B = 5GB
- With indexes: 5GB * 1.5 = 7.5GB
- Growth buffer (6 months): 15GB
- Total disk: 20GB minimum

Memory Requirements:
- shared_buffers: 25% of RAM = 8GB (for 32GB server)
- effective_cache_size: 75% of RAM = 24GB
- work_mem: 64MB per connection
- Total RAM: 32GB minimum

Connection Pool:
- Max connections: CPU cores * 4 = 64 (for 16-core server)
```

#### Redis Sizing

```
Memory Requirements:
- Average entry size: ~1KB
- Expected entries: 1,000,000
- Raw memory: 1M * 1KB = 1GB
- Redis overhead: ~2x = 2GB
- Operating buffer: 50% = 1GB
- Total RAM: 4GB minimum

Replication:
- Each replica needs same memory
- 3-node cluster: 12GB total
```

### Runbooks

#### Runbook 1: Handle PostgreSQL Failover

```bash
#!/bin/bash
# postgres-failover.sh

set -e

PRIMARY_HOST="postgres-primary"
REPLICA_HOST="postgres-replica"
VIP="10.0.1.100"

# 1. Verify primary is down
if pg_isready -h $PRIMARY_HOST; then
    echo "Primary is still up, aborting"
    exit 1
fi

# 2. Promote replica
ssh $REPLICA_HOST "sudo -u postgres pg_ctl promote -D /var/lib/postgresql/data"

# 3. Wait for promotion
sleep 5

# 4. Update VIP
ip addr add $VIP/24 dev eth0

# 5. Update application configuration
kubectl set env deployment/optimizer DATABASE_URL="postgresql://optimizer@${REPLICA_HOST}:5432/optimizer"

# 6. Verify connectivity
psql -h $REPLICA_HOST -U optimizer -d optimizer -c "SELECT 1"

echo "Failover complete"
```

#### Runbook 2: Redis Cluster Rebalancing

```bash
#!/bin/bash
# redis-rebalance.sh

CLUSTER_NODES="node1:6379 node2:6379 node3:6379"

# 1. Check cluster health
for node in $CLUSTER_NODES; do
    redis-cli -h ${node%:*} -p ${node#*:} CLUSTER INFO
done

# 2. Rebalance slots
redis-cli --cluster rebalance ${CLUSTER_NODES%% *} \
    --cluster-use-empty-masters \
    --cluster-threshold 5

# 3. Verify slot distribution
redis-cli --cluster check ${CLUSTER_NODES%% *}

echo "Rebalancing complete"
```

#### Runbook 3: Emergency Disk Space Recovery

```bash
#!/bin/bash
# disk-cleanup.sh

set -e

DATA_DIR="/var/lib/optimizer/sled"
BACKUP_DIR="/var/backups/optimizer"
THRESHOLD_GB=10

# Check available space
available=$(df -BG $DATA_DIR | tail -1 | awk '{print $4}' | sed 's/G//')

if [ $available -lt $THRESHOLD_GB ]; then
    echo "Low disk space: ${available}GB available"

    # 1. Remove old backups (keep last 3)
    ls -t $BACKUP_DIR/backup_*.zst | tail -n +4 | xargs rm -f

    # 2. Compact Sled database
    systemctl stop optimizer
    /usr/local/bin/sled-compact $DATA_DIR
    systemctl start optimizer

    # 3. Verify space recovered
    available=$(df -BG $DATA_DIR | tail -1 | awk '{print $4}' | sed 's/G//')
    echo "Space recovered: ${available}GB available"
fi
```

### Alert Definitions

```yaml
# prometheus-alerts.yml

groups:
  - name: storage_alerts
    interval: 30s
    rules:
      - alert: HighErrorRate
        expr: rate(storage_errors_total[5m]) > 0.05
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High error rate in storage layer"
          description: "Error rate is {{ $value }} errors/sec"

      - alert: DatabaseConnectionPoolExhausted
        expr: storage_active_connections >= storage_max_connections * 0.9
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: "Database connection pool near capacity"
          description: "Using {{ $value }} of max connections"

      - alert: HighLatency
        expr: histogram_quantile(0.99, rate(storage_operation_duration_seconds_bucket[5m])) > 1.0
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High storage latency"
          description: "P99 latency is {{ $value }}s"

      - alert: DiskSpaceLow
        expr: storage_disk_usage_bytes / storage_disk_total_bytes > 0.85
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Low disk space"
          description: "Disk usage is {{ $value }}%"
```

---

## Conclusion

This user guide covers the complete lifecycle of the LLM Auto-Optimizer storage layer, from initial setup through production operations. Key takeaways:

1. **Choose the right backend** for your use case:
   - PostgreSQL for persistent, queryable storage
   - Redis for high-performance caching
   - Sled for embedded, single-node deployments

2. **Plan for scale** from the beginning:
   - Size your infrastructure appropriately
   - Implement monitoring early
   - Test failover procedures

3. **Automate operations**:
   - Backups and recovery
   - Health checks and alerting
   - Capacity management

4. **Monitor continuously**:
   - Track performance metrics
   - Set up alerting thresholds
   - Review logs regularly

For additional support:
- GitHub Issues: https://github.com/llm-devops/llm-auto-optimizer/issues
- Documentation: https://llmdevops.dev/docs
- Community: https://discord.gg/llm-devops

**Version History:**
- v0.1.0 (2025-11-10): Initial release

