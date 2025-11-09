//! PostgreSQL State Backend Example
//!
//! This example demonstrates the production-ready PostgreSQL state backend
//! for distributed state management.
//!
//! ## Prerequisites
//!
//! 1. Running PostgreSQL instance:
//!    ```bash
//!    docker run -d --name postgres \
//!      -e POSTGRES_PASSWORD=postgres \
//!      -e POSTGRES_DB=optimizer \
//!      -p 5432:5432 \
//!      postgres:15
//!    ```
//!
//! 2. Run this example:
//!    ```bash
//!    cargo run --example postgres_state_backend
//!    ```

use processor::state::{PostgresConfig, PostgresStateBackend, StateBackend};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== PostgreSQL State Backend Example ===\n");

    // 1. Configure PostgreSQL backend
    println!("1. Configuring PostgreSQL backend...");
    let config = PostgresConfig::new("postgresql://postgres:postgres@localhost/optimizer")
        .with_pool_size(5, 20)
        .with_ssl_mode("prefer")
        .with_timeout(Duration::from_secs(30))
        .with_retries(3, Duration::from_millis(100))
        .with_statement_logging(true);

    println!("   Database URL: postgresql://***@localhost/optimizer");
    println!("   Pool size: 5-20 connections");
    println!("   Timeout: 30s");
    println!();

    // 2. Initialize backend (runs migrations automatically)
    println!("2. Initializing backend (running migrations)...");
    let backend = PostgresStateBackend::new(config).await?;
    println!("   ✓ Backend initialized successfully");
    println!();

    // 3. Basic operations
    println!("3. Testing basic operations...");

    // Put some data
    backend.put(b"user:1:session", b"session_token_abc123").await?;
    backend.put(b"user:2:session", b"session_token_xyz789").await?;
    backend.put(b"window:123:state", b"aggregated_metrics_data").await?;
    println!("   ✓ Inserted 3 entries");

    // Get data
    if let Some(value) = backend.get(b"user:1:session").await? {
        println!("   ✓ Retrieved: {} bytes", value.len());
    }

    // Count entries
    let count = backend.count().await?;
    println!("   ✓ Total entries: {}", count);
    println!();

    // 4. Metadata support
    println!("4. Testing JSONB metadata...");
    backend
        .put_with_metadata(
            b"event:12345",
            b"event_payload_data",
            serde_json::json!({
                "source": "kafka",
                "topic": "metrics",
                "partition": 5,
                "offset": 12345,
                "timestamp": "2024-01-15T10:30:00Z"
            }),
        )
        .await?;
    println!("   ✓ Stored entry with metadata");
    println!();

    // 5. TTL support
    println!("5. Testing TTL (time-to-live)...");
    backend
        .put_with_ttl(b"temp:cache:key", b"temporary_value", Duration::from_secs(5))
        .await?;
    println!("   ✓ Stored entry with 5-second TTL");

    // Verify it exists
    assert!(backend.contains(b"temp:cache:key").await?);
    println!("   ✓ Entry exists immediately after creation");

    println!("   Waiting 6 seconds for expiration...");
    tokio::time::sleep(Duration::from_secs(6)).await;

    // Should be filtered out by query
    let exists = backend.contains(b"temp:cache:key").await?;
    println!("   Entry exists after TTL: {}", exists);
    println!();

    // 6. Cleanup expired entries
    println!("6. Cleaning up expired entries...");
    let deleted = backend.cleanup_expired().await?;
    println!("   ✓ Deleted {} expired entries", deleted);
    println!();

    // 7. Batch operations
    println!("7. Testing batch operations...");
    let batch_entries: Vec<(&[u8], &[u8])> = vec![
        (b"batch:1", b"value1"),
        (b"batch:2", b"value2"),
        (b"batch:3", b"value3"),
        (b"batch:4", b"value4"),
        (b"batch:5", b"value5"),
    ];

    backend.batch_put(&batch_entries).await?;
    println!("   ✓ Batch inserted {} entries", batch_entries.len());
    println!();

    // 8. List keys with prefix
    println!("8. Testing prefix queries...");
    let batch_keys = backend.list_keys(b"batch:").await?;
    println!("   ✓ Found {} entries with 'batch:' prefix", batch_keys.len());

    let user_keys = backend.list_keys(b"user:").await?;
    println!("   ✓ Found {} entries with 'user:' prefix", user_keys.len());
    println!();

    // 9. Checkpointing
    println!("9. Testing checkpointing...");
    let checkpoint_id = backend.create_checkpoint().await?;
    println!("   ✓ Created checkpoint: {}", checkpoint_id);

    // List all checkpoints
    let checkpoints = backend.list_checkpoints().await?;
    println!("   ✓ Total checkpoints: {}", checkpoints.len());

    if let Some(checkpoint) = checkpoints.first() {
        println!("   Latest checkpoint:");
        println!("     - ID: {}", checkpoint.checkpoint_id);
        println!("     - Created: {}", checkpoint.created_at);
        println!("     - Rows: {}", checkpoint.row_count);
        println!("     - Size: {} bytes", checkpoint.data_size);
    }
    println!();

    // 10. Modify state and restore
    println!("10. Testing checkpoint restore...");
    let original_count = backend.count().await?;
    println!("    Original entry count: {}", original_count);

    // Modify state
    backend.put(b"new:entry", b"new_value").await?;
    backend.delete(b"batch:1").await?;
    let modified_count = backend.count().await?;
    println!("    Modified entry count: {}", modified_count);

    // Restore checkpoint
    backend.restore_checkpoint(checkpoint_id).await?;
    let restored_count = backend.count().await?;
    println!("    Restored entry count: {}", restored_count);
    println!("    ✓ State restored successfully");
    println!();

    // 11. Statistics
    println!("11. Backend statistics...");
    let stats = backend.stats().await;
    println!("    Operations:");
    println!("      - GET: {}", stats.get_count);
    println!("      - PUT: {}", stats.put_count);
    println!("      - DELETE: {}", stats.delete_count);
    println!("      - Queries: {}", stats.query_count);
    println!("      - Transactions: {}", stats.transaction_count);
    println!("      - Retries: {}", stats.retry_count);
    println!("    Data:");
    println!("      - Bytes written: {}", stats.bytes_written);
    println!("      - Bytes read: {}", stats.bytes_read);
    println!();

    // 12. Table statistics
    println!("12. Table statistics...");
    let (total_size, table_size) = backend.table_stats().await?;
    println!("    Total size (with indexes): {} bytes ({:.2} MB)", total_size, total_size as f64 / 1_048_576.0);
    println!("    Table size: {} bytes ({:.2} MB)", table_size, table_size as f64 / 1_048_576.0);
    println!();

    // 13. Health check
    println!("13. Health check...");
    let healthy = backend.health_check().await?;
    println!("    Database status: {}", if healthy { "✓ HEALTHY" } else { "✗ UNHEALTHY" });
    println!();

    // 14. Vacuum (maintenance)
    println!("14. Running VACUUM for optimization...");
    backend.vacuum().await?;
    println!("    ✓ VACUUM completed");
    println!();

    // 15. Cleanup
    println!("15. Cleaning up test data...");
    backend.clear().await?;
    println!("    ✓ All entries cleared");
    println!();

    // Close connection pool
    backend.close().await;
    println!("=== Example completed successfully ===");

    Ok(())
}
