# Enhanced PostgreSQL State Backend

Enterprise-grade PostgreSQL state backend with advanced features for production deployments.

## Features

### 1. Connection Pooling
- **Advanced Pool Management**: Configurable min/max connections with health checks
- **Automatic Reconnection**: Handles connection failures gracefully
- **Idle Timeout**: Automatic cleanup of idle connections
- **Connection Lifetime**: Recycles connections after max lifetime
- **Test on Acquire**: Optional health check before returning connections

### 2. Replication Support
- **Read/Write Splitting**: Automatically routes read operations to replicas
- **Replica Lag Monitoring**: Tracks and monitors replication lag
- **Automatic Failover**: Falls back to primary when replicas are unhealthy
- **Health Checks**: Background monitoring of replica availability
- **Configurable Lag Threshold**: Set maximum acceptable replication lag

### 3. Transaction Management
- **Isolation Levels**: Support for Read Committed, Repeatable Read, and Serializable
- **Savepoints**: Nested transaction support with rollback points
- **Optimistic Locking**: Version-based concurrency control
- **Deadlock Detection**: Automatic retry on serialization failures

### 4. Advanced Queries
- **Batch Operations**: Efficient multi-row inserts with COPY-style performance
- **JSONB Support**: Query state entries by JSON metadata
- **Prepared Statements**: Statement caching for improved performance
- **Index Optimization**: Comprehensive indexing strategy

### 5. Distributed Coordination
- **Advisory Locks**: PostgreSQL native distributed locking
- **LISTEN/NOTIFY**: Event-driven coordination across instances
- **Leader Election**: Built-in leader election using advisory locks
- **Lock Guards**: RAII-style lock management with automatic cleanup

### 6. Data Management
- **Partitioning Support**: Hash, time-range, and list partitioning strategies
- **Retention Policies**: Automatic cleanup of old data
- **Archive Operations**: Move old data to archive tables
- **VACUUM Management**: Automated table maintenance

### 7. High Availability
- **Replication Status**: Monitor primary/replica status and lag
- **Manual Checkpoints**: Trigger database checkpoints on demand
- **Backup Integration**: Compatible with pg_dump and WAL archiving
- **Failover Automation**: Automatic failover to healthy replicas

### 8. Observability
- **Comprehensive Metrics**: Track operations, connections, transactions, and more
- **Slow Query Logging**: Identify performance bottlenecks
- **Table Statistics**: Monitor table size, bloat, and row counts
- **Pool Statistics**: Connection pool usage metrics
- **Query Performance**: Track query execution times

### 9. Security
- **TLS/SSL Support**: Full SSL/TLS encryption support
- **Certificate Validation**: Client and server certificate validation
- **Multiple SSL Modes**: disable, allow, prefer, require, verify-ca, verify-full
- **Secure Connections**: Certificate-based authentication

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
processor = { path = "crates/processor" }
```

## Quick Start

```rust
use processor::state::{EnhancedPostgresBackend, EnhancedPostgresConfig, StateBackend};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Basic configuration
    let config = EnhancedPostgresConfig::new("postgresql://user:pass@localhost/db");

    // Create backend
    let backend = EnhancedPostgresBackend::new(config).await?;

    // Use like any StateBackend
    backend.put(b"key", b"value").await?;
    let value = backend.get(b"key").await?;

    Ok(())
}
```

## Advanced Configuration

```rust
use processor::state::{
    EnhancedPostgresConfig, PostgresPoolConfig, PostgresSslConfig,
    PostgresReplicaConfig, RetentionPolicy, IsolationLevel,
};
use std::time::Duration;

let config = EnhancedPostgresConfig::new("postgresql://user:pass@primary:5432/db")
    // Connection pool
    .with_pool_config(PostgresPoolConfig {
        min_connections: 5,
        max_connections: 50,
        acquire_timeout: Duration::from_secs(30),
        idle_timeout: Some(Duration::from_secs(600)),
        max_lifetime: Some(Duration::from_secs(1800)),
        health_check_interval: Duration::from_secs(30),
        test_on_acquire: true,
    })
    // SSL/TLS
    .with_ssl_config(PostgresSslConfig {
        mode: "verify-full".to_string(),
        root_cert_path: Some("/path/to/ca-cert.pem".into()),
        client_cert_path: Some("/path/to/client-cert.pem".into()),
        client_key_path: Some("/path/to/client-key.pem".into()),
    })
    // Read replicas
    .with_replicas(PostgresReplicaConfig {
        replica_urls: vec![
            "postgresql://user:pass@replica1:5432/db".to_string(),
            "postgresql://user:pass@replica2:5432/db".to_string(),
        ],
        enable_read_splitting: true,
        max_replication_lag_ms: 1000,
        health_check_interval: Duration::from_secs(10),
        enable_auto_failover: true,
    })
    // Transaction isolation
    .with_isolation_level(IsolationLevel::RepeatableRead)
    // Data retention
    .with_retention_policy(RetentionPolicy {
        enabled: true,
        max_age: Duration::from_secs(86400 * 30), // 30 days
        cleanup_interval: Duration::from_secs(3600),
        batch_size: 1000,
    });

let backend = EnhancedPostgresBackend::new(config).await?;
```

## Usage Examples

### Basic Operations

```rust
// Put with metadata
backend.put_with_metadata(
    b"user:123",
    b"user_data",
    serde_json::json!({
        "type": "user",
        "created_at": "2025-01-01",
        "source": "registration"
    })
).await?;

// Put with TTL
backend.put_with_ttl(
    b"session:456",
    b"session_data",
    Duration::from_secs(3600) // 1 hour
).await?;

// Batch operations
let entries: Vec<(&[u8], &[u8])> = vec![
    (b"key1", b"value1"),
    (b"key2", b"value2"),
    (b"key3", b"value3"),
];
backend.batch_put(&entries).await?;

// High-performance bulk insert
backend.batch_copy(&entries).await?;
```

### Transactions

```rust
use processor::state::IsolationLevel;

// Begin transaction
let mut tx = backend.begin_transaction(IsolationLevel::Serializable).await?;

// Use savepoints
tx.savepoint("sp1").await?;

// ... perform operations ...

// Rollback to savepoint if needed
tx.rollback_to_savepoint("sp1").await?;

// Commit transaction
tx.commit().await?;
```

### Optimistic Locking

```rust
// Insert with version 1
let version = backend.put_with_version(b"key", b"value1", None).await?;
// version == 1

// Update with version check
let new_version = backend.put_with_version(b"key", b"value2", Some(version)).await?;
// new_version == 2

// Concurrent update with stale version fails
let result = backend.put_with_version(b"key", b"value3", Some(version)).await;
// result is Err(StateError::ConcurrentModification)
```

### Distributed Coordination

```rust
use processor::state::PostgresLockGuard;

// Advisory lock
let acquired = backend.acquire_advisory_lock(12345).await?;
if acquired {
    // Critical section
    backend.release_advisory_lock(12345).await?;
}

// Lock guard (RAII)
let guard = PostgresLockGuard::new(
    backend.clone_handle(),
    "my_lock",
    Duration::from_secs(30)
).await?;

if let Some(guard) = guard {
    // Lock acquired, do work
    // Lock automatically released when guard drops
}

// Leader election
let is_leader = backend.try_become_leader(
    "cluster_leader",
    Duration::from_secs(60)
).await?;

if is_leader {
    // This instance is the leader
}
```

### LISTEN/NOTIFY

```rust
use processor::state::PostgresListener;

// Notify
backend.notify("state_changes", "new_data_available").await?;

// Listen (in a separate task/connection)
let mut listener = PostgresListener::new(
    "postgresql://user:pass@localhost/db",
    "state_changes"
).await?;

while let Ok(message) = listener.recv().await {
    println!("Received: {}", message);
}
```

### Query by Metadata

```rust
// Find all users
let users = backend.query_by_metadata(
    "type",
    &serde_json::json!({"type": "user"})
).await?;

// Find specific user
let user = backend.query_by_metadata(
    "id",
    &serde_json::json!({"id": 123})
).await?;
```

### Data Management

```rust
// Vacuum and analyze
backend.vacuum_analyze(false).await?; // Regular vacuum
backend.vacuum_analyze(true).await?;  // Full vacuum

// Apply retention policy
let deleted = backend.apply_retention_policy().await?;
println!("Deleted {} expired entries", deleted);

// Archive old data
let archived = backend.archive_old_data(
    "state_entries_archive",
    Duration::from_secs(86400 * 90) // 90 days
).await?;

// Get table statistics
let stats = backend.get_table_statistics().await?;
println!("Total rows: {}", stats.total_rows);
println!("Table size: {} bytes", stats.table_size);
println!("Index size: {} bytes", stats.indexes_size);
```

### Monitoring

```rust
// Get backend statistics
let stats = backend.get_stats();
println!("Operations: {} gets, {} puts", stats.get_count, stats.put_count);
println!("Average query time: {}ms", stats.avg_query_duration_ms);
println!("Slow queries: {}", stats.slow_query_count);

// Pool statistics
let pool_stats = backend.get_pool_stats();
println!("Pool: {}/{} connections", pool_stats.active, pool_stats.max_size);

// Replication status
let repl_status = backend.check_replication_status().await?;
println!("Is replica: {}", repl_status.is_replica);
println!("Replication lag: {}ms", repl_status.lag_ms);

// Health check
let healthy = backend.health_check().await?;
```

## Database Schema

The enhanced backend uses the following tables:

### state_entries
Main state storage table with JSONB metadata, TTL support, and versioning.

### state_checkpoints
Stores point-in-time snapshots for backup and recovery.

### state_advisory_locks
Tracks advisory lock ownership for debugging.

### state_entries_archive
Historical archive for old state entries.

## Migrations

Migrations are automatically applied on backend initialization. The migration files are located in `crates/processor/migrations/`:

- `001_create_state_table.sql` - Base schema
- `002_add_indexes.sql` - Performance indexes
- `003_enhanced_features.sql` - Enhanced features (versioning, functions, views)

## Performance Considerations

1. **Connection Pooling**: Tune `min_connections` and `max_connections` based on workload
2. **Read Replicas**: Use replicas for read-heavy workloads
3. **Batch Operations**: Use `batch_copy()` for bulk inserts (up to 10x faster)
4. **Indexes**: The migration scripts create optimal indexes
5. **Vacuum**: Run `vacuum_analyze()` periodically
6. **Partitioning**: Consider partitioning for large tables (>100M rows)

## Best Practices

1. **Always use connection pooling** - Don't create multiple backend instances
2. **Monitor replica lag** - Set appropriate `max_replication_lag_ms`
3. **Use optimistic locking** - For concurrent updates to the same keys
4. **Enable SSL in production** - Use `verify-full` mode with certificates
5. **Set retention policies** - Prevent unbounded growth
6. **Monitor metrics** - Track slow queries and pool statistics
7. **Use transactions** - For multi-step operations
8. **Archive old data** - Move historical data to archive tables

## Troubleshooting

### Connection Pool Exhausted
- Increase `max_connections`
- Check for connection leaks (transactions not committed/rolled back)
- Monitor active connections with `get_pool_stats()`

### Slow Queries
- Check `slow_query_count` in statistics
- Run `vacuum_analyze()` to update statistics
- Consider adding indexes for specific query patterns
- Review `pg_stat_statements` output

### Replication Lag
- Check replica health with `check_replication_status()`
- Verify network connectivity to replicas
- Consider increasing `max_replication_lag_ms` threshold
- Check replica server load

### Lock Contention
- Monitor `advisory_lock_wait_ms` in statistics
- Reduce lock hold time
- Consider using savepoints for long transactions

## Testing

Run tests with a PostgreSQL instance:

```bash
# Start PostgreSQL
docker run -d -p 5432:5432 -e POSTGRES_PASSWORD=postgres postgres:15

# Run tests
cargo test --package processor postgres_backend_enhanced
```

## Migration from Basic Backend

```rust
// Old
use processor::state::{PostgresStateBackend, PostgresConfig};
let backend = PostgresStateBackend::new(PostgresConfig::new(url)).await?;

// New
use processor::state::{EnhancedPostgresBackend, EnhancedPostgresConfig};
let backend = EnhancedPostgresBackend::new(EnhancedPostgresConfig::new(url)).await?;
```

Both implement the `StateBackend` trait, so the basic API is compatible.

## License

Apache-2.0
