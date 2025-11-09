# PostgreSQL State Backend - Quick Start Guide

Get up and running with the PostgreSQL state backend in under 5 minutes.

## Step 1: Start PostgreSQL

Using Docker (recommended for testing):

```bash
cd /workspaces/llm-auto-optimizer/crates/processor/examples
docker-compose -f docker-compose.postgres.yml up -d
```

This starts:
- PostgreSQL 15 on port 5432
- pgAdmin on port 5050 (optional, for database management)

**Access pgAdmin**: http://localhost:5050
- Email: `admin@optimizer.local`
- Password: `admin`

## Step 2: Add to Your Code

```rust
use processor::state::{PostgresConfig, PostgresStateBackend, StateBackend};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Configure backend
    let config = PostgresConfig::new("postgresql://postgres:postgres@localhost/optimizer")
        .with_pool_size(10, 50)
        .with_timeout(Duration::from_secs(30));

    // Initialize (runs migrations automatically)
    let backend = PostgresStateBackend::new(config).await?;

    // Basic usage
    backend.put(b"window:123", b"aggregated_data").await?;

    if let Some(data) = backend.get(b"window:123").await? {
        println!("Retrieved {} bytes", data.len());
    }

    Ok(())
}
```

## Step 3: Run the Example

```bash
cargo run --example postgres_state_backend
```

This demonstrates:
- Basic CRUD operations
- Metadata storage (JSONB)
- TTL support
- Batch operations
- Checkpointing
- Statistics and monitoring

## Common Operations

### Store with Metadata

```rust
backend.put_with_metadata(
    b"event:12345",
    b"event_data",
    serde_json::json!({
        "source": "kafka",
        "partition": 5,
        "offset": 12345
    })
).await?;
```

### Set TTL (Time-To-Live)

```rust
// Expires after 1 hour
backend.put_with_ttl(
    b"cache:key",
    b"temporary_value",
    Duration::from_secs(3600)
).await?;

// Cleanup expired entries
let deleted = backend.cleanup_expired().await?;
```

### Batch Insert

```rust
let entries: Vec<(&[u8], &[u8])> = vec![
    (b"key1", b"value1"),
    (b"key2", b"value2"),
    (b"key3", b"value3"),
];

backend.batch_put(&entries).await?;
```

### Create Checkpoint

```rust
// Create snapshot
let checkpoint_id = backend.create_checkpoint().await?;

// Later: restore
backend.restore_checkpoint(checkpoint_id).await?;
```

## Configuration Options

```rust
let config = PostgresConfig::new("postgresql://user:pass@host/db")
    // Pool settings
    .with_pool_size(10, 50)

    // Timeouts
    .with_timeout(Duration::from_secs(30))

    // SSL (for production)
    .with_ssl_mode("require")

    // Retries
    .with_retries(3, Duration::from_millis(100))

    // Debug logging
    .with_statement_logging(true);
```

## Environment Variables

Instead of hardcoding credentials:

```bash
export DATABASE_URL="postgresql://postgres:postgres@localhost/optimizer"
```

```rust
let config = PostgresConfig::new(std::env::var("DATABASE_URL")?);
```

## Monitoring

```rust
// Get statistics
let stats = backend.stats().await;
println!("Operations: GET={}, PUT={}, DELETE={}",
    stats.get_count, stats.put_count, stats.delete_count);

// Table size
let (total_size, table_size) = backend.table_stats().await?;
println!("Size: {} MB", total_size / 1_048_576);

// Health check
let healthy = backend.health_check().await?;
```

## Running Tests

```bash
# Integration tests
cargo test --test postgres_integration_test

# Benchmarks
cargo bench --bench postgres_backend_bench
```

## Production Deployment

### 1. Update Configuration

```rust
let config = PostgresConfig::new(std::env::var("DATABASE_URL")?)
    .with_pool_size(20, 100)
    .with_ssl_mode("require")
    .with_timeout(Duration::from_secs(30))
    .with_statement_logging(false);
```

### 2. Set Environment Variables

```bash
DATABASE_URL=postgresql://user:password@host:5432/db
DB_SSL_MODE=require
DB_MAX_CONNECTIONS=100
```

### 3. Enable SSL

```rust
let config = PostgresConfig::new(url)
    .with_ssl_mode("verify-full");
```

### 4. Monitor Metrics

- Connection pool utilization
- Query latency (p95, p99)
- Error rate
- Table size growth

## Troubleshooting

### Cannot Connect

```bash
# Check if PostgreSQL is running
docker ps | grep postgres

# Check logs
docker logs optimizer-postgres

# Test connection
psql postgresql://postgres:postgres@localhost/optimizer
```

### Migrations Fail

```sql
-- Connect to database
psql postgresql://postgres:postgres@localhost/optimizer

-- Check tables
\dt

-- Manually run migration if needed
\i /path/to/migration.sql
```

### Performance Issues

```rust
// Increase pool size
let config = PostgresConfig::new(url)
    .with_pool_size(20, 100);

// Use batch operations
backend.batch_put(&entries).await?;

// Run VACUUM periodically
backend.vacuum().await?;
```

## Next Steps

- Read full documentation: `README_POSTGRES_BACKEND.md`
- Review implementation guide: `/docs/postgres_state_backend_guide.md`
- Explore advanced features in the example
- Run benchmarks to understand performance

## Support

- GitHub Issues: https://github.com/llm-devops/llm-auto-optimizer/issues
- Documentation: https://llmdevops.dev

## Quick Reference

```rust
// Basic operations
backend.put(key, value).await?;
backend.get(key).await?;
backend.delete(key).await?;
backend.list_keys(prefix).await?;

// Advanced operations
backend.put_with_metadata(key, value, metadata).await?;
backend.put_with_ttl(key, value, duration).await?;
backend.batch_put(&entries).await?;

// Checkpointing
backend.create_checkpoint().await?;
backend.restore_checkpoint(id).await?;

// Maintenance
backend.cleanup_expired().await?;
backend.vacuum().await?;
backend.health_check().await?;

// Statistics
backend.stats().await;
backend.table_stats().await?;
backend.count().await?;
```
