# Distributed State Backend Guide

Complete guide to deploying and managing distributed state backends (Redis and PostgreSQL) for the LLM Auto Optimizer in production environments.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Redis Backend](#redis-backend)
3. [PostgreSQL Backend](#postgresql-backend)
4. [Distributed Locking](#distributed-locking)
5. [High Availability](#high-availability)
6. [Performance Tuning](#performance-tuning)
7. [Monitoring & Alerting](#monitoring--alerting)
8. [Troubleshooting](#troubleshooting)
9. [Security](#security)
10. [Production Checklist](#production-checklist)

---

## Architecture Overview

### Components

```
┌─────────────────────────────────────────────────────────┐
│                  Application Layer                       │
│  ┌──────────┐  ┌──────────┐  ┌───────────────┐         │
│  │StateBackend│  │  Lock    │  │ Checkpoint    │         │
│  │   Trait    │  │Interface │  │  Coordinator  │         │
│  └──────────┘  └──────────┘  └───────────────┘         │
└─────────────────┬──────────────────┬──────────────────────┘
                  │                  │
        ┌─────────┴────────┐  ┌──────┴────────┐
        │                  │  │               │
┌───────▼────────┐  ┌──────▼──────┐  ┌───────▼────────┐
│ Redis Backend  │  │   PostgreSQL │  │  Distributed   │
│                │  │   Backend    │  │     Locks      │
│ - Fast K/V     │  │              │  │                │
│ - TTL Support  │  │ - ACID       │  │ - Redis Lock   │
│ - Atomic Ops   │  │ - Metadata   │  │ - PG Advisory  │
└────────────────┘  └──────────────┘  └────────────────┘
        │                  │                 │
        │                  │                 │
┌───────▼────────┐  ┌──────▼──────┐  ┌───────▼────────┐
│ Redis Cluster  │  │ PostgreSQL  │  │  Lock Storage  │
│ (3+ nodes)     │  │ Primary +   │  │  (Redis/PG)    │
│                │  │ Replicas    │  │                │
└────────────────┘  └──────────────┘  └────────────────┘
```

### Backend Comparison

| Feature | Redis | PostgreSQL | Best For |
|---------|-------|------------|----------|
| **Throughput** | 100K+ ops/sec | 10K+ ops/sec | Redis for high frequency |
| **Latency** | <1ms p99 | 1-5ms p99 | Redis for low latency |
| **Durability** | AOF/RDB | WAL + Replication | PostgreSQL for critical data |
| **Transactions** | Limited | Full ACID | PostgreSQL for complex ops |
| **Query Flexibility** | Basic | SQL (indexes, joins) | PostgreSQL for analytics |
| **TTL** | Native | Manual cleanup | Redis for expiring data |
| **Memory Usage** | All in RAM | Disk + cache | PostgreSQL for large datasets |
| **Operational Complexity** | Moderate | Higher | Redis for simpler ops |

---

## Redis Backend

### Architecture Patterns

#### 1. Redis Sentinel (High Availability)

```
┌──────────────────────────────────────────┐
│           Client Applications             │
└─────────┬──────────────┬─────────────────┘
          │              │
     ┌────▼──────┐  ┌────▼──────┐
     │ Sentinel  │  │ Sentinel  │
     │  Node 1   │  │  Node 2   │
     └────┬──────┘  └────┬──────┘
          │              │
     ┌────▼──────────────▼──────┐
     │   Sentinel Node 3         │
     └────┬──────────────────────┘
          │
    ┌─────┴──────┬──────────┬─────────┐
    │            │          │         │
┌───▼───┐  ┌────▼───┐  ┌───▼───┐  ┌──▼──┐
│Primary│  │Replica │  │Replica│  │Replica│
│ (RW)  │  │  (RO)  │  │  (RO) │  │  (RO) │
└───────┘  └────────┘  └───────┘  └─────┘
```

**Benefits:**
- Automatic failover (<30 seconds)
- Read scaling via replicas
- No single point of failure
- Simple configuration

**Configuration:**

```rust
use processor::state::{RedisConfig, RedisStateBackend};

let config = RedisConfig::builder()
    .urls(vec![
        "redis://sentinel1:26379".to_string(),
        "redis://sentinel2:26379".to_string(),
        "redis://sentinel3:26379".to_string(),
    ])
    .key_prefix("optimizer:")
    .default_ttl(Duration::from_secs(3600))
    .pool_size(10, 50)
    .retries(5, Duration::from_millis(100), Duration::from_secs(5))
    .build()?;

let backend = RedisStateBackend::new(config).await?;
```

**Sentinel Configuration** (`sentinel.conf`):

```conf
sentinel monitor mymaster 10.0.1.10 6379 2
sentinel down-after-milliseconds mymaster 5000
sentinel parallel-syncs mymaster 1
sentinel failover-timeout mymaster 10000
```

#### 2. Redis Cluster (Sharding + HA)

```
┌──────────────────────────────────────────────┐
│            Client Applications               │
└──────┬───────────┬──────────┬────────────────┘
       │           │          │
   ┌───▼──┐    ┌───▼──┐   ┌───▼──┐
   │Shard │    │Shard │   │Shard │
   │  1   │    │  2   │   │  3   │
   │Master│    │Master│   │Master│
   └───┬──┘    └───┬──┘   └───┬──┘
       │           │          │
   ┌───▼──┐    ┌───▼──┐   ┌───▼──┐
   │Shard │    │Shard │   │Shard │
   │  1   │    │  2   │   │  3   │
   │Replica│   │Replica│  │Replica│
   └──────┘    └──────┘   └──────┘
```

**Benefits:**
- Horizontal scaling (1M+ ops/sec)
- Automatic sharding (16,384 hash slots)
- Built-in replication
- No proxy overhead

**Configuration:**

```rust
let config = RedisConfig::builder()
    .urls(vec![
        "redis://node1:7000".to_string(),
        "redis://node2:7001".to_string(),
        "redis://node3:7002".to_string(),
        "redis://node4:7003".to_string(),
        "redis://node5:7004".to_string(),
        "redis://node6:7005".to_string(),
    ])
    .cluster_mode(true)
    .key_prefix("optimizer:")
    .pool_size(20, 100)
    .build()?;

let backend = RedisStateBackend::new(config).await?;
```

**Cluster Setup:**

```bash
# Create cluster (3 masters + 3 replicas)
redis-cli --cluster create \
  node1:7000 node2:7001 node3:7002 \
  node4:7003 node5:7004 node6:7005 \
  --cluster-replicas 1
```

### Redis Operations

#### Basic Operations

```rust
use processor::state::{RedisStateBackend, StateBackend};

// Initialize
let backend = RedisStateBackend::new(config).await?;

// PUT with auto TTL
backend.put(b"session:123", b"user_data").await?;

// PUT with custom TTL
backend.put_with_ttl(
    b"cache:item:456",
    b"value",
    Duration::from_secs(300)
).await?;

// GET
let value = backend.get(b"session:123").await?;

// DELETE
backend.delete(b"session:123").await?;

// BATCH operations
let pairs = vec![
    (b"key1", b"value1"),
    (b"key2", b"value2"),
];
backend.batch_put(&pairs).await?;

let keys = vec![b"key1", b"key2"];
let values = backend.batch_get(&keys).await?;
```

#### Atomic Operations

```rust
// Get and delete atomically
let value = backend.get_and_delete(b"one_time_token").await?;

// Set if not exists (SETNX)
let was_set = backend.set_if_not_exists(
    b"lock:resource",
    b"owner_id",
    Duration::from_secs(30)
).await?;

// Check TTL
let remaining = backend.ttl(b"session:123").await?;
```

#### Checkpointing

```rust
// Create checkpoint
let checkpoint = backend.checkpoint().await?;
println!("Checkpointed {} keys", checkpoint.len());

// Restore from checkpoint
backend.restore(checkpoint).await?;
```

### Redis Performance Tuning

#### Connection Pooling

```rust
let config = RedisConfig::builder()
    .url("redis://localhost:6379")
    .pool_size(
        num_cpus::get() as u32,      // Min: # of CPU cores
        num_cpus::get() as u32 * 4   // Max: 4x CPU cores
    )
    .build()?;
```

#### Pipeline Batching

```rust
let config = RedisConfig::builder()
    .url("redis://localhost:6379")
    .pipeline_batch_size(100)  // Batch 100 ops per pipeline
    .build()?;

// Automatically uses pipelining for batch operations
backend.batch_put(&large_dataset).await?;
```

#### Memory Optimization

`redis.conf`:

```conf
# Set max memory (e.g., 4GB)
maxmemory 4gb

# Eviction policy
maxmemory-policy allkeys-lru

# Use LZ4 compression for values > 1KB
# (Application-level compression)

# Disable persistence for cache-only workloads
save ""
appendonly no
```

#### Persistence Configuration

For durability:

```conf
# AOF for maximum durability
appendonly yes
appendfsync everysec
auto-aof-rewrite-percentage 100
auto-aof-rewrite-min-size 64mb

# RDB for faster restarts
save 900 1
save 300 10
save 60 10000
```

---

## PostgreSQL Backend

### Architecture Patterns

#### 1. Primary-Replica Setup

```
┌────────────────────────────────┐
│    Application (Write Path)    │
└────────────┬───────────────────┘
             │
        ┌────▼─────┐
        │ Primary  │
        │  (RW)    │
        └────┬─────┘
             │
   ┌─────────┴─────────┐
   │ Streaming         │
   │ Replication       │
   │ (WAL)             │
   └─────┬──────┬──────┘
         │      │
    ┌────▼───┐ │ ┌────▼───┐
    │Replica │ │ │Replica │
    │  (RO)  │ │ │  (RO)  │
    └────────┘ │ └────────┘
               │
         ┌─────▼────┐
         │ Replica  │
         │  (RO)    │
         └──────────┘
```

**Configuration:**

```rust
let primary_config = PostgresConfig::new(
    "postgresql://user:pass@primary:5432/optimizer"
)
.with_pool_size(10, 50)
.with_ssl_mode("require");

let primary = PostgresStateBackend::new(primary_config).await?;

// For read replicas (read-only)
let replica_config = PostgresConfig::new(
    "postgresql://user:pass@replica:5432/optimizer"
)
.with_pool_size(20, 100)
.with_ssl_mode("require");

let replica = PostgresStateBackend::new(replica_config).await?;
```

#### 2. Connection Pooling with PgBouncer

```
┌────────────────────────────────┐
│      Application Instances     │
│   (100s of connections)        │
└──────────┬──┬──┬───────────────┘
           │  │  │
      ┌────▼──▼──▼────┐
      │   PgBouncer    │
      │   (pooling)    │
      │                │
      │  20 backend    │
      │  connections   │
      └────────┬───────┘
               │
        ┌──────▼───────┐
        │  PostgreSQL  │
        │   Primary    │
        └──────────────┘
```

**PgBouncer Configuration** (`pgbouncer.ini`):

```ini
[databases]
optimizer = host=postgres dbname=optimizer

[pgbouncer]
listen_addr = 0.0.0.0
listen_port = 6432
pool_mode = transaction
max_client_conn = 1000
default_pool_size = 20
reserve_pool_size = 5
reserve_pool_timeout = 3
```

### PostgreSQL Operations

#### Basic Operations

```rust
let config = PostgresConfig::new("postgresql://localhost/optimizer")
    .with_pool_size(10, 50);

let backend = PostgresStateBackend::new(config).await?;

// PUT
backend.put(b"key1", b"value1").await?;

// PUT with metadata (JSONB)
backend.put_with_metadata(
    b"event:123",
    b"event_data",
    serde_json::json!({
        "source": "kafka",
        "partition": 5,
        "timestamp": "2025-11-10T12:00:00Z"
    })
).await?;

// PUT with TTL
backend.put_with_ttl(
    b"temp:key",
    b"value",
    Duration::from_secs(600)
).await?;

// GET
let value = backend.get(b"key1").await?;

// BATCH operations
let entries = vec![(b"k1", b"v1"), (b"k2", b"v2")];
backend.batch_put(&entries).await?;
```

#### Advanced Features

```rust
// Checkpoint
let checkpoint_id = backend.create_checkpoint().await?;

// List checkpoints
let checkpoints = backend.list_checkpoints().await?;
for cp in checkpoints {
    println!("Checkpoint {}: {} rows, {} bytes",
        cp.checkpoint_id, cp.row_count, cp.data_size);
}

// Restore
backend.restore_checkpoint(checkpoint_id).await?;

// Cleanup expired entries
let deleted = backend.cleanup_expired().await?;

// VACUUM for maintenance
backend.vacuum().await?;

// Table statistics
let (total_size, table_size) = backend.table_stats().await?;
println!("Total: {}MB, Table: {}MB",
    total_size / 1_048_576,
    table_size / 1_048_576
);
```

### PostgreSQL Performance Tuning

#### Database Configuration

`postgresql.conf`:

```conf
# Memory settings (for 16GB RAM server)
shared_buffers = 4GB
effective_cache_size = 12GB
maintenance_work_mem = 1GB
work_mem = 64MB

# Connection settings
max_connections = 200

# WAL settings
wal_level = replica
max_wal_senders = 10
wal_keep_size = 1GB

# Checkpoints
checkpoint_timeout = 15min
checkpoint_completion_target = 0.9

# Autovacuum
autovacuum = on
autovacuum_max_workers = 4
autovacuum_vacuum_scale_factor = 0.1
autovacuum_analyze_scale_factor = 0.05

# Query planner
random_page_cost = 1.1  # For SSD
effective_io_concurrency = 200

# Logging
log_min_duration_statement = 1000  # Log slow queries (>1s)
log_line_prefix = '%t [%p]: [%l-1] user=%u,db=%d,app=%a,client=%h '
```

#### Schema Optimization

```sql
-- Optimized schema with indexes
CREATE TABLE state_entries (
    key BYTEA PRIMARY KEY,
    value BYTEA NOT NULL,
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ
);

-- Indexes
CREATE INDEX idx_state_expires ON state_entries(expires_at)
    WHERE expires_at IS NOT NULL;

CREATE INDEX idx_state_metadata ON state_entries
    USING GIN(metadata);

CREATE INDEX idx_state_updated ON state_entries(updated_at DESC);

-- Function for cleanup
CREATE OR REPLACE FUNCTION cleanup_expired_state_entries()
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    DELETE FROM state_entries
    WHERE expires_at IS NOT NULL AND expires_at < NOW();

    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- Automatic cleanup with pg_cron
SELECT cron.schedule('cleanup-expired', '*/5 * * * *',
    $$SELECT cleanup_expired_state_entries()$$);
```

---

## Distributed Locking

### Redis Locks

```rust
use processor::state::{RedisDistributedLock, DistributedLock};

let lock = RedisDistributedLock::new("redis://localhost:6379").await?;

// Non-blocking lock
if let Some(guard) = lock.try_acquire("resource", Duration::from_secs(30)).await? {
    // Critical section
    println!("Acquired lock");
    drop(guard);  // Auto-release
} else {
    println!("Lock held by another instance");
}

// Blocking lock with retry
let guard = lock.acquire("checkpoint", Duration::from_secs(60)).await?;
// Perform checkpoint
drop(guard);

// Lock extension for long operations
let mut guard = lock.acquire("long_task", Duration::from_secs(30)).await?;
for _ in 0..10 {
    // Do work
    tokio::time::sleep(Duration::from_secs(20)).await;

    // Extend lock before expiration
    lock.extend(&mut guard, Duration::from_secs(30)).await?;
}
```

### PostgreSQL Advisory Locks

```rust
use processor::state::PostgresDistributedLock;

let lock = PostgresDistributedLock::new("postgresql://localhost/optimizer").await?;

// Advisory locks (session-level)
let guard = lock.acquire("migration", Duration::from_secs(300)).await?;
// Run migration
drop(guard);
```

### Lock Patterns

#### Leader Election

```rust
async fn become_leader(lock: Arc<dyn DistributedLock>) -> Result<()> {
    loop {
        if let Some(guard) = lock.try_acquire(
            "leader",
            Duration::from_secs(30)
        ).await? {
            println!("Became leader");

            // Perform leader duties
            while !should_step_down() {
                do_leader_work().await?;

                // Extend leadership
                lock.extend(&mut guard, Duration::from_secs(30)).await?;

                tokio::time::sleep(Duration::from_secs(10)).await;
            }

            drop(guard);
        }

        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
```

---

## High Availability

### Deployment Patterns

#### Active-Active (Redis Cluster)

```
Region A              Region B              Region C
┌─────────┐          ┌─────────┐          ┌─────────┐
│ Master  │◄────────►│ Master  │◄────────►│ Master  │
│ Shard 1 │          │ Shard 2 │          │ Shard 3 │
└────┬────┘          └────┬────┘          └────┬────┘
     │                    │                    │
┌────▼────┐          ┌────▼────┐          ┌────▼────┐
│ Replica │          │ Replica │          │ Replica │
└─────────┘          └─────────┘          └─────────┘
```

#### Active-Passive (PostgreSQL)

```
Primary Site                  Standby Site
┌──────────┐                 ┌──────────┐
│ Primary  │────WAL Ship────►│ Standby  │
│ (Active) │                 │ (Passive)│
└──────────┘                 └──────────┘
     │                            │
     │ VIP Failover              │
     └────────────────────────────┘
```

### Failure Scenarios

| Scenario | Redis Sentinel | Redis Cluster | PostgreSQL |
|----------|----------------|---------------|------------|
| **Primary Failure** | Auto-failover (<30s) | Auto-failover (<5s) | Manual/Patroni |
| **Replica Failure** | Continues (degraded reads) | Slot migration | Continues |
| **Network Partition** | Quorum-based decisions | Split-brain possible | Requires fencing |
| **Data Center Loss** | Requires cross-DC setup | Requires cross-DC setup | Standby promotion |

---

## Monitoring & Alerting

### Key Metrics

#### Redis Metrics

```rust
let stats = backend.stats().await;

// Log metrics
info!("Redis Metrics:");
info!("  Operations: {}", stats.total_operations());
info!("  Hit Rate: {:.2}%", stats.hit_rate() * 100.0);
info!("  Error Rate: {:.2}%", stats.error_rate() * 100.0);
info!("  Avg Latency: {}µs", stats.avg_latency_us);
info!("  Bytes Read: {}", stats.bytes_read);
info!("  Bytes Written: {}", stats.bytes_written);

// Health check
if !backend.health_check().await? {
    alert!("Redis backend unhealthy");
}

// Memory usage
let memory = backend.memory_usage().await?;
if memory > 4_000_000_000 { // 4GB
    warn!("Redis memory usage high: {}MB", memory / 1_048_576);
}
```

#### PostgreSQL Metrics

```rust
let stats = backend.stats().await;

info!("PostgreSQL Metrics:");
info!("  Queries: {}", stats.query_count);
info!("  Transactions: {}", stats.transaction_count);
info!("  Retries: {}", stats.retry_count);

// Table stats
let (total_size, table_size) = backend.table_stats().await?;
info!("  Table Size: {}MB", table_size / 1_048_576);

// Health check
assert!(backend.health_check().await?);
```

### Prometheus Integration

```rust
use prometheus::{register_counter, register_histogram, Counter, Histogram};

lazy_static! {
    static ref STATE_OPS_TOTAL: Counter = register_counter!(
        "state_operations_total",
        "Total state operations"
    ).unwrap();

    static ref STATE_OP_DURATION: Histogram = register_histogram!(
        "state_operation_duration_seconds",
        "State operation duration"
    ).unwrap();
}

// Instrument operations
async fn instrumented_put(backend: &impl StateBackend, key: &[u8], value: &[u8]) -> Result<()> {
    let timer = STATE_OP_DURATION.start_timer();
    let result = backend.put(key, value).await;
    timer.observe_duration();
    STATE_OPS_TOTAL.inc();
    result
}
```

### Alert Rules

```yaml
# Prometheus alerts
groups:
  - name: state_backend
    rules:
      - alert: HighErrorRate
        expr: rate(state_errors_total[5m]) > 0.01
        for: 5m
        annotations:
          summary: "High error rate in state backend"

      - alert: LowHitRate
        expr: state_cache_hit_rate < 0.80
        for: 15m
        annotations:
          summary: "Cache hit rate below 80%"

      - alert: HighLatency
        expr: histogram_quantile(0.99, state_op_duration_seconds) > 0.1
        for: 10m
        annotations:
          summary: "P99 latency above 100ms"
```

---

## Troubleshooting

### Common Issues

#### Redis Connection Failures

**Symptoms:** `Failed to connect to Redis`

**Solutions:**
```bash
# Check Redis is running
redis-cli ping

# Check network connectivity
telnet redis-host 6379

# Check logs
tail -f /var/log/redis/redis-server.log

# Verify configuration
redis-cli CONFIG GET bind
redis-cli CONFIG GET protected-mode
```

#### High Memory Usage (Redis)

**Symptoms:** Redis evicting keys, OOM errors

**Solutions:**
```bash
# Check memory usage
redis-cli INFO memory

# Check key sizes
redis-cli --bigkeys

# Set max memory
redis-cli CONFIG SET maxmemory 4gb
redis-cli CONFIG SET maxmemory-policy allkeys-lru

# Enable persistence to disk
redis-cli CONFIG SET appendonly yes
```

#### Slow Queries (PostgreSQL)

**Symptoms:** High query latency

**Solutions:**
```sql
-- Find slow queries
SELECT query, mean_exec_time, calls
FROM pg_stat_statements
ORDER BY mean_exec_time DESC
LIMIT 10;

-- Check missing indexes
SELECT schemaname, tablename, attname
FROM pg_stats
WHERE correlation < 0.1
  AND schemaname NOT IN ('pg_catalog', 'information_schema');

-- Analyze tables
ANALYZE state_entries;

-- VACUUM bloated tables
VACUUM ANALYZE state_entries;
```

---

## Security

### Network Security

```yaml
# Redis ACL (redis 6.0+)
user optimizer on >strong_password ~optimizer:* +get +set +del

# PostgreSQL pg_hba.conf
hostssl optimizer optimizer 10.0.0.0/8 scram-sha-256
hostssl all all 0.0.0.0/0 reject
```

### Encryption

```rust
// Redis with TLS
let config = RedisConfig::builder()
    .url("rediss://localhost:6380")  // Note: rediss://
    .tls(true)
    .build()?;

// PostgreSQL with SSL
let config = PostgresConfig::new("postgresql://localhost/optimizer")
    .with_ssl_mode("verify-full");
```

---

## Production Checklist

### Pre-Deployment

- [ ] Load test with production-like traffic
- [ ] Configure monitoring and alerting
- [ ] Set up backup and restore procedures
- [ ] Document runbooks for common issues
- [ ] Configure connection pooling
- [ ] Enable TLS/SSL
- [ ] Set appropriate timeouts and retries
- [ ] Configure high availability
- [ ] Test failover procedures
- [ ] Set up log aggregation

### Post-Deployment

- [ ] Monitor error rates and latencies
- [ ] Verify backup procedures
- [ ] Test disaster recovery
- [ ] Optimize slow queries
- [ ] Review and adjust pool sizes
- [ ] Monitor resource usage
- [ ] Update documentation
- [ ] Conduct chaos engineering tests

---

This guide provides comprehensive coverage of distributed state backends for production deployment. Refer to specific sections as needed for your deployment scenario.
