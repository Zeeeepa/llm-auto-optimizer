//! PostgreSQL State Backend Demo
//!
//! This example demonstrates the PostgreSQL state backend features including:
//! - Basic CRUD operations
//! - Transactions (ACID)
//! - Batch operations
//! - Query optimization
//! - Migrations
//!
//! ## Prerequisites
//!
//! Start PostgreSQL:
//! ```bash
//! docker-compose up -d postgres
//! ```
//!
//! ## Run Example
//!
//! ```bash
//! cargo run --example postgres_state_demo
//! ```

use processor::state::{PostgresStateBackend, StateBackend};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== PostgreSQL State Backend Demo ===\n");

    let database_url = "postgres://optimizer:password@localhost:5432/optimizer";
    let table_name = "demo_state";

    println!("Connecting to PostgreSQL at localhost:5432");

    let backend = PostgresStateBackend::new(database_url, table_name).await?;

    println!("✓ Connected and table '{}' ensured\n", table_name);

    // Clear any existing demo data
    backend.clear().await?;

    // ========================================================================
    // Demo 1: Basic CRUD Operations
    // ========================================================================
    println!("--- Demo 1: Basic CRUD Operations ---");

    // CREATE (PUT)
    println!("Storing customer:1001 = John Doe");
    backend.put(b"customer:1001", b"John Doe").await?;

    // READ (GET)
    if let Some(value) = backend.get(b"customer:1001").await? {
        println!("Retrieved: {}", String::from_utf8_lossy(&value));
    }

    // UPDATE
    println!("Updating customer:1001 = John A. Doe");
    backend.put(b"customer:1001", b"John A. Doe").await?;

    if let Some(value) = backend.get(b"customer:1001").await? {
        println!("Updated value: {}", String::from_utf8_lossy(&value));
    }

    // DELETE
    println!("Deleting customer:1001");
    backend.delete(b"customer:1001").await?;

    if backend.get(b"customer:1001").await?.is_none() {
        println!("✓ Customer deleted successfully\n");
    }

    // ========================================================================
    // Demo 2: Transactions (ACID)
    // ========================================================================
    println!("--- Demo 2: Transactions (ACID) ---");

    println!("Beginning transaction...");
    let txn = backend.begin_transaction().await?;

    // Insert within transaction
    backend
        .put_in_transaction(&txn, b"account:1001:balance", b"1000.00")
        .await?;
    backend
        .put_in_transaction(&txn, b"account:1002:balance", b"2000.00")
        .await?;

    println!("  account:1001:balance = 1000.00 (in transaction)");
    println!("  account:1002:balance = 2000.00 (in transaction)");

    // Commit transaction
    println!("Committing transaction...");
    backend.commit_transaction(txn).await?;
    println!("✓ Transaction committed\n");

    // Verify data persistence
    println!("Verifying committed data:");
    if let Some(value) = backend.get(b"account:1001:balance").await? {
        println!("  account:1001:balance = {}", String::from_utf8_lossy(&value));
    }
    if let Some(value) = backend.get(b"account:1002:balance").await? {
        println!("  account:1002:balance = {}\n", String::from_utf8_lossy(&value));
    }

    // ========================================================================
    // Demo 3: Transaction Rollback
    // ========================================================================
    println!("--- Demo 3: Transaction Rollback ---");

    println!("Beginning transaction...");
    let txn2 = backend.begin_transaction().await?;

    // Attempt to modify
    backend
        .put_in_transaction(&txn2, b"account:1001:balance", b"500.00")
        .await?;

    println!("  Modified account:1001:balance = 500.00 (in transaction)");
    println!("  Simulating error condition...");

    // Rollback
    println!("Rolling back transaction...");
    backend.rollback_transaction(txn2).await?;
    println!("✓ Transaction rolled back\n");

    // Verify original value preserved
    println!("Verifying rollback:");
    if let Some(value) = backend.get(b"account:1001:balance").await? {
        println!("  account:1001:balance = {} (unchanged)", String::from_utf8_lossy(&value));
    }
    println!();

    // ========================================================================
    // Demo 4: Batch Operations (UPSERT)
    // ========================================================================
    println!("--- Demo 4: Batch Operations (UPSERT) ---");

    println!("Batch inserting 5 products...");
    let products = vec![
        (&b"product:P001"[..], &b"{\"name\":\"Laptop\",\"price\":999.99}"[..]),
        (&b"product:P002"[..], &b"{\"name\":\"Mouse\",\"price\":29.99}"[..]),
        (&b"product:P003"[..], &b"{\"name\":\"Keyboard\",\"price\":79.99}"[..]),
        (&b"product:P004"[..], &b"{\"name\":\"Monitor\",\"price\":299.99}"[..]),
        (&b"product:P005"[..], &b"{\"name\":\"Webcam\",\"price\":89.99}"[..]),
    ];

    backend.batch_upsert(&products).await?;
    println!("✓ Batch insert complete");

    // List products
    println!("\nListing all products:");
    let product_keys = backend.list_keys(b"product:").await?;
    println!("Found {} products:", product_keys.len());

    for key in &product_keys {
        if let Some(value) = backend.get(key).await? {
            println!("  {} = {}", String::from_utf8_lossy(key), String::from_utf8_lossy(&value));
        }
    }
    println!();

    // ========================================================================
    // Demo 5: Complex Queries and Filters
    // ========================================================================
    println!("--- Demo 5: Complex Queries ---");

    // Count total keys
    let total = backend.count().await?;
    println!("Total keys in database: {}", total);

    // Count with prefix
    let product_count = product_keys.len();
    println!("Product keys: {}", product_count);

    // Check existence
    println!("\nChecking existence:");
    let exists = backend.contains(b"product:P001").await?;
    println!("  product:P001 exists: {}", exists);

    let not_exists = backend.contains(b"product:P999").await?;
    println!("  product:P999 exists: {}\n", not_exists);

    // ========================================================================
    // Demo 6: Large Dataset Handling
    // ========================================================================
    println!("--- Demo 6: Large Dataset Handling ---");

    println!("Inserting 1000 log entries...");
    let start = std::time::Instant::now();

    // Use batch upsert for better performance
    let mut batch = Vec::new();
    for i in 0..1000 {
        let key = format!("log:entry:{:04}", i);
        let value = format!("{{\"timestamp\":{},\"message\":\"Log entry {}\"}}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs(),
            i
        );
        batch.push((key.into_bytes(), value.into_bytes()));
    }

    // Convert to references for batch_upsert
    let batch_refs: Vec<(&[u8], &[u8])> = batch
        .iter()
        .map(|(k, v)| (k.as_slice(), v.as_slice()))
        .collect();

    backend.batch_upsert(&batch_refs).await?;

    let elapsed = start.elapsed();
    println!("✓ Inserted 1000 entries in {:?}", elapsed);
    println!("  Throughput: {:.2} ops/sec\n", 1000.0 / elapsed.as_secs_f64());

    // ========================================================================
    // Demo 7: Database Statistics
    // ========================================================================
    println!("--- Demo 7: Database Statistics ---");

    let stats = backend.stats().await?;
    println!("PostgreSQL Statistics:");
    println!("  Total keys: {}", stats.total_keys);
    println!("  Database size: {} bytes ({:.2} MB)",
        stats.database_size_bytes,
        stats.database_size_bytes as f64 / 1_048_576.0
    );
    println!("  Table size: {} bytes ({:.2} MB)",
        stats.table_size_bytes,
        stats.table_size_bytes as f64 / 1_048_576.0
    );
    println!("  Index size: {} bytes ({:.2} MB)",
        stats.index_size_bytes,
        stats.index_size_bytes as f64 / 1_048_576.0
    );
    println!("  Dead tuples: {}\n", stats.dead_tuples);

    // ========================================================================
    // Demo 8: Vacuum and Optimization
    // ========================================================================
    println!("--- Demo 8: Vacuum and Optimization ---");

    // Delete some data to create dead tuples
    println!("Deleting half of log entries...");
    for i in 0..500 {
        let key = format!("log:entry:{:04}", i);
        backend.delete(key.as_bytes()).await?;
    }
    println!("✓ Deleted 500 entries");

    // Check stats before vacuum
    let stats_before = backend.stats().await?;
    println!("\nBefore VACUUM:");
    println!("  Total keys: {}", stats_before.total_keys);
    println!("  Dead tuples: {}", stats_before.dead_tuples);

    // Run vacuum
    println!("\nRunning VACUUM...");
    backend.vacuum().await?;
    println!("✓ VACUUM complete");

    // Check stats after vacuum
    let stats_after = backend.stats().await?;
    println!("\nAfter VACUUM:");
    println!("  Total keys: {}", stats_after.total_keys);
    println!("  Dead tuples: {}", stats_after.dead_tuples);
    println!("  Reclaimed {} dead tuples\n",
        stats_before.dead_tuples.saturating_sub(stats_after.dead_tuples)
    );

    // ========================================================================
    // Demo 9: Connection Pool Information
    // ========================================================================
    println!("--- Demo 9: Connection Pool ---");

    let pool_stats = backend.pool_stats();
    println!("Connection Pool Statistics:");
    println!("  Active connections: {}", pool_stats.active_connections);
    println!("  Idle connections: {}", pool_stats.idle_connections);
    println!("  Total connections: {}", pool_stats.total_connections);
    println!();

    // ========================================================================
    // Demo 10: Health Check
    // ========================================================================
    println!("--- Demo 10: Health Check ---");

    if backend.health_check().await? {
        println!("✓ Database connection healthy");
    } else {
        println!("✗ Database connection unhealthy");
    }
    println!();

    // ========================================================================
    // Demo 11: Concurrent Operations
    // ========================================================================
    println!("--- Demo 11: Concurrent Operations ---");

    println!("Spawning 10 concurrent tasks...");
    let backend_clone = std::sync::Arc::new(backend);
    let mut handles = vec![];

    for i in 0..10 {
        let backend = backend_clone.clone();
        let handle = tokio::spawn(async move {
            for j in 0..10 {
                let key = format!("concurrent:task{}:item{}", i, j);
                let value = format!("value_{}_{}", i, j);
                backend.put(key.as_bytes(), value.as_bytes()).await?;
            }
            Ok::<_, anyhow::Error>(())
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await??;
    }

    println!("✓ 10 tasks completed (100 operations total)");

    // Verify concurrent inserts
    let concurrent_keys = backend_clone.list_keys(b"concurrent:").await?;
    println!("  Concurrent keys created: {}\n", concurrent_keys.len());

    // ========================================================================
    // Cleanup
    // ========================================================================
    println!("--- Cleanup ---");
    backend_clone.clear().await?;
    println!("✓ All demo data cleared");

    // Final stats
    let final_stats = backend_clone.stats().await?;
    println!("\nFinal Statistics:");
    println!("  Total keys: {}", final_stats.total_keys);
    println!("  Database size: {:.2} MB", final_stats.database_size_bytes as f64 / 1_048_576.0);

    println!("\n=== Demo Complete ===");

    Ok(())
}
