# PostgreSQL State Backend

Production-ready PostgreSQL state backend for distributed state management in the LLM Auto-Optimizer stream processing system.

## Features

### Core Features

- **Connection Pooling**: Managed by sqlx PgPool for optimal performance
- **Async Operations**: Full async/await support throughout
- **ACID Transactions**: BEGIN/COMMIT/ROLLBACK support
- **JSONB Metadata**: Store additional context with state entries
- **Automatic Migrations**: Schema versioning and automatic upgrades
- **TTL Support**: Automatic expiration and cleanup of old entries

### Advanced Features

- **Checkpointing**: Point-in-time snapshots for backup and recovery
- **Batch Operations**: Efficient bulk inserts with prepared statements
- **Query Optimization**: Indexes, partial indexes, and statistics
- **Maintenance**: VACUUM, table size monitoring, and health checks
- **Production Ready**: SSL, retries, timeouts, and comprehensive error handling
- **Statistics**: Detailed operation metrics and performance tracking

## Architecture

### Database Schema

The backend uses two primary tables:

#### `state_entries` Table

```sql
CREATE TABLE state_entries (
    key BYTEA PRIMARY KEY,
    value BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    metadata JSONB
);
```

**Indexes:**
- Primary key on `key` (BTREE)
- Partial index on `expires_at` for TTL queries
- GIN index on `metadata` for JSONB queries
- Indexes on `created_at` and `updated_at` for time-based queries

#### `state_checkpoints` Table

```sql
CREATE TABLE state_checkpoints (
    checkpoint_id UUID PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    checkpoint_data JSONB NOT NULL,
    metadata JSONB,
    row_count BIGINT NOT NULL DEFAULT 0,
    data_size BIGINT NOT NULL DEFAULT 0
);
```

### Migration System

Migrations are automatically applied on backend initialization:

1. `001_create_state_table.sql` - Initial schema and tables
2. `002_add_indexes.sql` - Performance optimization indexes

Migrations are stored in `/crates/processor/migrations/` and are applied in order.

## Quick Start

### Prerequisites

Start PostgreSQL using Docker:

```bash
cd crates/processor/examples
docker-compose -f docker-compose.postgres.yml up -d
```

Or connect to an existing PostgreSQL instance.

### Basic Usage

```rust
use processor::state::{PostgresConfig, PostgresStateBackend, StateBackend};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Configure backend
    let config = PostgresConfig::new("postgresql://postgres:postgres@localhost/optimizer")
        .with_pool_size(10, 50)
        .with_ssl_mode("require")
        .with_timeout(Duration::from_secs(30));

    // Initialize (runs migrations automatically)
    let backend = PostgresStateBackend::new(config).await?;

    // Basic operations
    backend.put(b"window:123", b"aggregated_data").await?;

    if let Some(data) = backend.get(b"window:123").await? {
        println!("Retrieved: {:?}", data);
    }

    backend.delete(b"window:123").await?;

    Ok(())
}
```

## Configuration

### PostgresConfig Options

```rust
let config = PostgresConfig::new("postgresql://user:pass@host/db")
    // Connection pool settings
    .with_pool_size(min_connections, max_connections)

    // Table and schema names
    .with_table_name("custom_state_entries")
    .with_schema_name("my_schema")

    // Timeouts
    .with_timeout(Duration::from_secs(30))

    // SSL mode: disable, allow, prefer, require, verify-ca, verify-full
    .with_ssl_mode("require")

    // Retry configuration
    .with_retries(max_retries, backoff_duration)

    // Debug logging
    .with_statement_logging(true);
```

### Environment Variables

```bash
# Database connection
DATABASE_URL=postgresql://postgres:postgres@localhost/optimizer

# Pool settings
DB_MIN_CONNECTIONS=5
DB_MAX_CONNECTIONS=20

# SSL settings
DB_SSL_MODE=require
DB_SSL_CERT=/path/to/cert.pem
DB_SSL_KEY=/path/to/key.pem
```

## Advanced Usage

### Metadata Support

Store additional context with state entries using JSONB:

```rust
backend.put_with_metadata(
    b"event:12345",
    b"event_data",
    serde_json::json!({
        "source": "kafka",
        "partition": 5,
        "offset": 12345,
        "timestamp": "2024-01-15T10:30:00Z"
    })
).await?;
```

### TTL (Time-To-Live)

Automatically expire entries after a duration:

```rust
use std::time::Duration;

// Entry expires after 1 hour
backend.put_with_ttl(
    b"cache:key",
    b"temporary_value",
    Duration::from_secs(3600)
).await?;

// Cleanup expired entries
let deleted_count = backend.cleanup_expired().await?;
println!("Deleted {} expired entries", deleted_count);
```

### Batch Operations

Efficiently insert multiple entries:

```rust
let entries: Vec<(&[u8], &[u8])> = vec![
    (b"key1", b"value1"),
    (b"key2", b"value2"),
    (b"key3", b"value3"),
];

backend.batch_put(&entries).await?;
```

### Checkpointing

Create point-in-time snapshots for backup and recovery:

```rust
// Create checkpoint
let checkpoint_id = backend.create_checkpoint().await?;
println!("Created checkpoint: {}", checkpoint_id);

// List all checkpoints
let checkpoints = backend.list_checkpoints().await?;
for checkpoint in checkpoints {
    println!("Checkpoint {} - {} rows, {} bytes",
        checkpoint.checkpoint_id,
        checkpoint.row_count,
        checkpoint.data_size);
}

// Restore from checkpoint
backend.restore_checkpoint(checkpoint_id).await?;
```

### Statistics and Monitoring

```rust
// Get backend statistics
let stats = backend.stats().await;
println!("Operations: GET={}, PUT={}, DELETE={}",
    stats.get_count, stats.put_count, stats.delete_count);
println!("Data: written={} bytes, read={} bytes",
    stats.bytes_written, stats.bytes_read);

// Get table size
let (total_size, table_size) = backend.table_stats().await?;
println!("Total size: {} MB", total_size / 1_048_576);

// Health check
let healthy = backend.health_check().await?;
assert!(healthy);
```

### Maintenance

```rust
// Run VACUUM to reclaim storage and update statistics
backend.vacuum().await?;

// Get table statistics for query optimization
let (total_size, table_size) = backend.table_stats().await?;

// Manual cleanup of expired entries
let deleted = backend.cleanup_expired().await?;
```

## Performance Optimization

### Index Usage

The backend automatically creates optimized indexes:

1. **Primary Key Index**: Fast key lookups
2. **Partial Expiration Index**: Efficient TTL queries
3. **JSONB GIN Index**: Fast metadata queries
4. **Timestamp Indexes**: Time-based queries

### Connection Pooling

Configure pool size based on workload:

```rust
// High-throughput workload
let config = PostgresConfig::new(url)
    .with_pool_size(20, 100);

// Low-latency workload
let config = PostgresConfig::new(url)
    .with_pool_size(5, 20);
```

### Batch Operations

Use batch operations for bulk inserts:

```rust
// Inefficient - multiple round trips
for (key, value) in entries {
    backend.put(key, value).await?;
}

// Efficient - single transaction
backend.batch_put(&entries).await?;
```

### Query Optimization

The backend uses prepared statements and query optimization:

- Prepared statements for repeated queries
- Partial indexes for common filters
- Statistics collection for query planning
- Connection reuse and pooling

## Production Deployment

### Database Setup

1. **Create Database**:
   ```sql
   CREATE DATABASE optimizer;
   CREATE USER optimizer_user WITH PASSWORD 'secure_password';
   GRANT ALL PRIVILEGES ON DATABASE optimizer TO optimizer_user;
   ```

2. **Configure SSL**:
   ```sql
   ALTER SYSTEM SET ssl = on;
   ALTER SYSTEM SET ssl_cert_file = '/path/to/server.crt';
   ALTER SYSTEM SET ssl_key_file = '/path/to/server.key';
   ```

3. **Performance Tuning**:
   ```sql
   -- Increase shared buffers for caching
   ALTER SYSTEM SET shared_buffers = '256MB';

   -- Increase work memory for sorting
   ALTER SYSTEM SET work_mem = '16MB';

   -- Enable parallel queries
   ALTER SYSTEM SET max_parallel_workers_per_gather = 4;
   ```

### Monitoring

Monitor key metrics:

- Connection pool utilization
- Query latency (p50, p95, p99)
- Table size growth
- Index usage statistics
- Checkpoint frequency and duration

### Backup and Recovery

1. **Regular Checkpoints**:
   ```rust
   // Create daily checkpoints
   let checkpoint_id = backend.create_checkpoint().await?;
   ```

2. **PostgreSQL Backups**:
   ```bash
   # WAL archiving for point-in-time recovery
   pg_basebackup -D /backup/location -Ft -z -P

   # Logical backup
   pg_dump optimizer > backup.sql
   ```

3. **Restore**:
   ```rust
   // Restore from application checkpoint
   backend.restore_checkpoint(checkpoint_id).await?;
   ```

## Example Application

Run the comprehensive example:

```bash
# Start PostgreSQL
cd crates/processor/examples
docker-compose -f docker-compose.postgres.yml up -d

# Run example
cargo run --example postgres_state_backend
```

The example demonstrates:
- Basic CRUD operations
- Metadata support
- TTL and expiration
- Batch operations
- Checkpointing and restore
- Statistics and monitoring
- Maintenance operations

## Troubleshooting

### Connection Issues

**Problem**: Cannot connect to database

**Solution**:
```rust
// Check connection string
let config = PostgresConfig::new("postgresql://user:pass@host:5432/db");

// Verify PostgreSQL is running
docker ps | grep postgres

// Check logs
docker logs optimizer-postgres
```

### Migration Failures

**Problem**: Migrations fail to apply

**Solution**:
```sql
-- Check existing tables
\dt

-- Manually run migrations
\i /path/to/migration.sql

-- Reset if needed (WARNING: deletes all data)
DROP TABLE state_entries CASCADE;
DROP TABLE state_checkpoints CASCADE;
```

### Performance Issues

**Problem**: Slow queries

**Solution**:
```sql
-- Analyze query performance
EXPLAIN ANALYZE SELECT * FROM state_entries WHERE key = $1;

-- Update statistics
ANALYZE state_entries;

-- Rebuild indexes
REINDEX TABLE state_entries;
```

### Pool Exhaustion

**Problem**: Too many connections

**Solution**:
```rust
// Increase pool size
let config = PostgresConfig::new(url)
    .with_pool_size(10, 50);

// Or increase timeout
let config = PostgresConfig::new(url)
    .with_timeout(Duration::from_secs(60));
```

## Testing

### Unit Tests

```bash
# Run tests (requires PostgreSQL)
cargo test postgres

# Run with logging
RUST_LOG=debug cargo test postgres -- --nocapture
```

### Integration Tests

The backend includes comprehensive tests:
- Basic CRUD operations
- Metadata storage
- TTL and expiration
- Batch operations
- Checkpointing
- Statistics tracking
- Health checks

### Test Database

Tests use a separate database:

```sql
CREATE DATABASE optimizer_test;
```

Set the test database URL:

```rust
let config = PostgresConfig::new(
    "postgresql://postgres:postgres@localhost/optimizer_test"
);
```

## Security

### SSL/TLS

Enable SSL for production:

```rust
let config = PostgresConfig::new(url)
    .with_ssl_mode("require");
```

SSL Modes:
- `disable`: No SSL
- `allow`: Try SSL, fallback to non-SSL
- `prefer`: Prefer SSL, fallback to non-SSL (default)
- `require`: Require SSL
- `verify-ca`: Require SSL and verify CA
- `verify-full`: Require SSL and verify hostname

### Connection Security

```rust
// Use environment variables for credentials
let database_url = std::env::var("DATABASE_URL")?;
let config = PostgresConfig::new(database_url);

// Never hardcode credentials
// BAD: PostgresConfig::new("postgresql://user:pass@host/db")
// GOOD: PostgresConfig::new(std::env::var("DATABASE_URL")?)
```

### SQL Injection

The backend uses prepared statements to prevent SQL injection:

```rust
// Safe - uses parameterized queries
backend.put(b"key", b"value").await?;

// Queries are always parameterized
sqlx::query("SELECT value FROM state_entries WHERE key = $1")
    .bind(key)
    .fetch_one(&pool)
    .await?;
```

## Comparison with Other Backends

| Feature | PostgreSQL | Sled | Redis | Memory |
|---------|-----------|------|-------|--------|
| Persistence | ✓ | ✓ | ✓ | ✗ |
| Distributed | ✓ | ✗ | ✓ | ✗ |
| ACID Transactions | ✓ | ✓ | ✗ | ✗ |
| JSONB Metadata | ✓ | ✗ | ✗ | ✗ |
| TTL Support | ✓ | ✗ | ✓ | ✓ |
| Checkpointing | ✓ | ✓ | ✓ | ✗ |
| Connection Pool | ✓ | ✗ | ✓ | ✗ |
| Query Optimization | ✓ | ✓ | ✗ | ✗ |

### When to Use PostgreSQL Backend

**Use PostgreSQL when:**
- You need ACID transactions
- You require JSONB metadata support
- You need SQL query capabilities
- You want distributed state across multiple nodes
- You need point-in-time recovery
- You require mature backup/restore tools

**Use Sled when:**
- You need embedded database
- You have single-node deployment
- You want zero-copy operations
- You need crash recovery

**Use Redis when:**
- You need extremely low latency
- You want TTL with automatic eviction
- You need pub/sub capabilities
- You have high read throughput

**Use Memory when:**
- You need maximum performance
- State can be rebuilt from source
- You have sufficient RAM

## License

Apache-2.0

## Support

For issues and questions:
- GitHub Issues: https://github.com/llm-devops/llm-auto-optimizer/issues
- Documentation: https://llmdevops.dev
