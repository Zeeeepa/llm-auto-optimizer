//! Comprehensive demonstration of Enhanced PostgreSQL Backend features
//!
//! Run with: cargo run --package processor --example postgres_enhanced_demo
//!
//! Prerequisites:
//! - PostgreSQL running on localhost:5432
//! - Database 'optimizer_demo' created

use processor::state::{
    EnhancedPostgresBackend, EnhancedPostgresConfig, IsolationLevel, PostgresLockGuard,
    PostgresPoolConfig, PostgresReplicaConfig, PostgresSslConfig, RetentionPolicy, StateBackend,
};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== Enhanced PostgreSQL Backend Demo ===\n");

    // 1. Basic Configuration
    println!("1. Creating backend with basic configuration...");
    let config = EnhancedPostgresConfig::new("postgresql://postgres:postgres@localhost/optimizer_demo");
    let backend = EnhancedPostgresBackend::new(config).await?;
    println!("✓ Backend created\n");

    // 2. Basic Operations
    println!("2. Basic CRUD operations...");
    backend.put(b"user:1", b"John Doe").await?;
    backend.put(b"user:2", b"Jane Smith").await?;

    let value = backend.get(b"user:1").await?;
    println!("✓ Retrieved: {:?}", value);
    println!("✓ Count: {}\n", backend.count().await?);

    // 3. Metadata Operations
    println!("3. Operations with metadata...");
    backend
        .put_with_metadata(
            b"product:101",
            b"Laptop",
            serde_json::json!({
                "category": "electronics",
                "price": 999.99,
                "stock": 15
            }),
        )
        .await?;
    println!("✓ Stored product with metadata\n");

    // 4. TTL Support
    println!("4. TTL (Time To Live) support...");
    backend
        .put_with_ttl(b"session:abc123", b"session_data", Duration::from_secs(5))
        .await?;
    println!("✓ Created session with 5s TTL");
    println!("   Session exists: {}", backend.contains(b"session:abc123").await?);

    tokio::time::sleep(Duration::from_secs(6)).await;
    println!("   After 6s, exists: {}", backend.contains(b"session:abc123").await?);

    let cleaned = backend.cleanup_expired().await?;
    println!("✓ Cleaned up {} expired entries\n", cleaned);

    // 5. Batch Operations
    println!("5. Batch operations...");
    let entries: Vec<(&[u8], &[u8])> = vec![
        (b"batch:1", b"value1"),
        (b"batch:2", b"value2"),
        (b"batch:3", b"value3"),
        (b"batch:4", b"value4"),
        (b"batch:5", b"value5"),
    ];
    backend.batch_put(&entries).await?;
    println!("✓ Batch inserted 5 entries\n");

    // 6. High-Performance Bulk Insert
    println!("6. Bulk insert with COPY-style performance...");
    let mut bulk_entries = Vec::new();
    for i in 0..100 {
        bulk_entries.push((
            format!("bulk:{}", i).into_bytes(),
            format!("value_{}", i).into_bytes(),
        ));
    }
    let refs: Vec<(&[u8], &[u8])> = bulk_entries
        .iter()
        .map(|(k, v)| (k.as_slice(), v.as_slice()))
        .collect();

    backend.batch_copy(&refs).await?;
    println!("✓ Bulk inserted 100 entries\n");

    // 7. Transactions
    println!("7. Transaction with savepoints...");
    let mut tx = backend
        .begin_transaction(IsolationLevel::RepeatableRead)
        .await?;

    sqlx::query("INSERT INTO state_entries (key, value) VALUES ($1, $2)")
        .bind(b"tx:step1")
        .bind(b"committed")
        .execute(tx.executor())
        .await?;

    tx.savepoint("sp1").await?;

    sqlx::query("INSERT INTO state_entries (key, value) VALUES ($1, $2)")
        .bind(b"tx:step2")
        .bind(b"rolled_back")
        .execute(tx.executor())
        .await?;

    tx.rollback_to_savepoint("sp1").await?;
    tx.commit().await?;

    println!("✓ tx:step1 exists: {}", backend.contains(b"tx:step1").await?);
    println!("✓ tx:step2 exists: {}", backend.contains(b"tx:step2").await?);
    println!();

    // 8. Optimistic Locking
    println!("8. Optimistic locking with versioning...");
    let v1 = backend.put_with_version(b"versioned:key", b"v1", None).await?;
    println!("✓ Initial version: {}", v1);

    let v2 = backend
        .put_with_version(b"versioned:key", b"v2", Some(v1))
        .await?;
    println!("✓ Updated version: {}", v2);

    let result = backend
        .put_with_version(b"versioned:key", b"v3_stale", Some(v1))
        .await;
    println!("✓ Stale version update failed: {}", result.is_err());
    println!();

    // 9. Distributed Coordination - Advisory Locks
    println!("9. Advisory locks...");
    let lock_id = 12345i64;

    let acquired = backend.acquire_advisory_lock(lock_id).await?;
    println!("✓ Lock acquired: {}", acquired);

    let acquired_again = backend.acquire_advisory_lock(lock_id).await?;
    println!("✓ Try acquire again: {}", acquired_again);

    backend.release_advisory_lock(lock_id).await?;
    println!("✓ Lock released\n");

    // 10. Lock Guards
    println!("10. RAII-style lock guards...");
    {
        let guard = PostgresLockGuard::new(
            backend.clone_handle(),
            "demo_lock",
            Duration::from_secs(10),
        )
        .await?;

        if let Some(_guard) = guard {
            println!("✓ Lock guard acquired");
            // Lock held here
        } // Lock automatically released when guard drops
    }
    println!("✓ Lock guard released\n");

    // 11. Leader Election
    println!("11. Leader election...");
    let is_leader = backend
        .try_become_leader("cluster_leader", Duration::from_secs(30))
        .await?;
    println!("✓ This instance is leader: {}\n", is_leader);

    // 12. Query by Metadata
    println!("12. Query by JSONB metadata...");
    let products = backend
        .query_by_metadata(
            "category",
            &serde_json::json!({"category": "electronics"}),
        )
        .await?;
    println!("✓ Found {} products in electronics\n", products.len());

    // 13. Statistics and Monitoring
    println!("13. Backend statistics...");
    let stats = backend.get_stats();
    println!("   GET operations: {}", stats.get_count);
    println!("   PUT operations: {}", stats.put_count);
    println!("   DELETE operations: {}", stats.delete_count);
    println!("   Queries executed: {}", stats.query_count);
    println!("   Transactions: {}", stats.transaction_count);
    println!("   Avg query time: {}ms", stats.avg_query_duration_ms);
    println!("   Slow queries: {}", stats.slow_query_count);
    println!("   Bytes written: {}", stats.bytes_written);
    println!("   Bytes read: {}", stats.bytes_read);
    println!();

    // 14. Pool Statistics
    println!("14. Connection pool statistics...");
    let pool_stats = backend.get_pool_stats();
    println!("   Pool size: {}", pool_stats.size);
    println!("   Idle connections: {}", pool_stats.idle);
    println!("   Active connections: {}", pool_stats.active);
    println!("   Max size: {}", pool_stats.max_size);
    println!();

    // 15. Table Statistics
    println!("15. Table statistics...");
    let table_stats = backend.get_table_statistics().await?;
    println!("   Total rows: {}", table_stats.total_rows);
    println!("   Active rows: {}", table_stats.active_rows);
    println!("   Expired rows: {}", table_stats.expired_rows);
    println!("   Total size: {} bytes", table_stats.total_size);
    println!("   Table size: {} bytes", table_stats.table_size);
    println!("   Indexes size: {} bytes", table_stats.indexes_size);
    println!();

    // 16. Replication Status
    println!("16. Replication status...");
    let repl_status = backend.check_replication_status().await?;
    println!("   Is replica: {}", repl_status.is_replica);
    println!("   Replication lag: {}ms", repl_status.lag_ms);
    println!("   Replicas connected: {}", repl_status.replicas_connected);
    println!();

    // 17. Data Management
    println!("17. Data management operations...");
    backend.vacuum_analyze(false).await?;
    println!("✓ VACUUM ANALYZE completed");

    let deleted = backend.apply_retention_policy().await?;
    println!("✓ Retention policy: deleted {} entries", deleted);
    println!();

    // 18. Health Check
    println!("18. Health check...");
    let healthy = backend.health_check().await?;
    println!("✓ Backend healthy: {}\n", healthy);

    // 19. Notification
    println!("19. Send notification...");
    backend
        .notify("demo_channel", "demo_complete")
        .await?;
    println!("✓ Notification sent\n");

    // 20. Cleanup
    println!("20. Cleanup...");
    // Note: In production, don't clear the entire table!
    // backend.clear().await?;
    println!("✓ Demo complete (table not cleared)\n");

    // Close connections
    backend.close().await;
    println!("✓ Connections closed");

    println!("\n=== Demo Complete ===");
    println!("All enterprise features demonstrated successfully!");

    Ok(())
}

#[allow(dead_code)]
async fn advanced_configuration_example() -> anyhow::Result<()> {
    // Full configuration with all options
    let config = EnhancedPostgresConfig::new("postgresql://user:pass@primary:5432/db")
        .with_pool_config(PostgresPoolConfig {
            min_connections: 5,
            max_connections: 50,
            acquire_timeout: Duration::from_secs(30),
            idle_timeout: Some(Duration::from_secs(600)),
            max_lifetime: Some(Duration::from_secs(1800)),
            health_check_interval: Duration::from_secs(30),
            test_on_acquire: true,
        })
        .with_ssl_config(PostgresSslConfig {
            mode: "verify-full".to_string(),
            root_cert_path: Some("/path/to/ca-cert.pem".into()),
            client_cert_path: Some("/path/to/client-cert.pem".into()),
            client_key_path: Some("/path/to/client-key.pem".into()),
        })
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
        .with_isolation_level(IsolationLevel::RepeatableRead)
        .with_retention_policy(RetentionPolicy {
            enabled: true,
            max_age: Duration::from_secs(86400 * 30),
            cleanup_interval: Duration::from_secs(3600),
            batch_size: 1000,
        });

    let _backend = EnhancedPostgresBackend::new(config).await?;

    Ok(())
}
