//! Integration tests for PostgreSQL state backend
//!
//! These tests require a running PostgreSQL instance.
//! Run with: cargo test --test postgres_integration_test

use processor::state::{PostgresConfig, PostgresStateBackend, StateBackend};
use std::sync::Arc;
use std::time::Duration;

async fn create_test_backend() -> Option<PostgresStateBackend> {
    let config =
        PostgresConfig::new("postgresql://postgres:postgres@localhost/optimizer_test")
            .with_pool_size(2, 10)
            .with_timeout(Duration::from_secs(10))
            .with_retries(3, Duration::from_millis(50));

    match PostgresStateBackend::new(config).await {
        Ok(backend) => {
            backend.clear().await.ok();
            Some(backend)
        }
        Err(e) => {
            eprintln!("Skipping PostgreSQL tests - database not available: {}", e);
            None
        }
    }
}

#[tokio::test]
async fn test_postgres_full_lifecycle() {
    let Some(backend) = create_test_backend().await else {
        return;
    };

    // Test basic operations
    assert!(backend.put(b"key1", b"value1").await.is_ok());
    assert_eq!(backend.get(b"key1").await.unwrap(), Some(b"value1".to_vec()));

    // Test update
    assert!(backend.put(b"key1", b"value_updated").await.is_ok());
    assert_eq!(
        backend.get(b"key1").await.unwrap(),
        Some(b"value_updated".to_vec())
    );

    // Test delete
    assert!(backend.delete(b"key1").await.is_ok());
    assert_eq!(backend.get(b"key1").await.unwrap(), None);

    // Test multiple entries
    assert!(backend.put(b"user:1", b"alice").await.is_ok());
    assert!(backend.put(b"user:2", b"bob").await.is_ok());
    assert!(backend.put(b"user:3", b"charlie").await.is_ok());

    let count = backend.count().await.unwrap();
    assert_eq!(count, 3);

    // Test prefix listing
    let user_keys = backend.list_keys(b"user:").await.unwrap();
    assert_eq!(user_keys.len(), 3);

    // Cleanup
    backend.clear().await.unwrap();
    assert_eq!(backend.count().await.unwrap(), 0);
}

#[tokio::test]
async fn test_postgres_metadata() {
    let Some(backend) = create_test_backend().await else {
        return;
    };

    let metadata = serde_json::json!({
        "source": "kafka",
        "topic": "metrics",
        "partition": 5,
        "offset": 12345,
        "timestamp": "2024-01-15T10:30:00Z",
        "tags": ["important", "production"]
    });

    backend
        .put_with_metadata(b"event:123", b"event_data", metadata.clone())
        .await
        .unwrap();

    // Verify data is stored
    let value = backend.get(b"event:123").await.unwrap();
    assert_eq!(value, Some(b"event_data".to_vec()));

    backend.clear().await.unwrap();
}

#[tokio::test]
async fn test_postgres_ttl() {
    let Some(backend) = create_test_backend().await else {
        return;
    };

    // Create entry with 2 second TTL
    backend
        .put_with_ttl(b"temp:key", b"temp_value", Duration::from_secs(2))
        .await
        .unwrap();

    // Should exist immediately
    assert!(backend.contains(b"temp:key").await.unwrap());
    assert_eq!(
        backend.get(b"temp:key").await.unwrap(),
        Some(b"temp_value".to_vec())
    );

    // Wait for expiration
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Should be filtered out
    assert!(!backend.contains(b"temp:key").await.unwrap());
    assert_eq!(backend.get(b"temp:key").await.unwrap(), None);

    // Cleanup should remove it
    let deleted = backend.cleanup_expired().await.unwrap();
    assert_eq!(deleted, 1);

    backend.clear().await.unwrap();
}

#[tokio::test]
async fn test_postgres_batch_operations() {
    let Some(backend) = create_test_backend().await else {
        return;
    };

    // Create batch entries
    let entries: Vec<(&[u8], &[u8])> = vec![
        (b"batch:1", b"value1"),
        (b"batch:2", b"value2"),
        (b"batch:3", b"value3"),
        (b"batch:4", b"value4"),
        (b"batch:5", b"value5"),
    ];

    backend.batch_put(&entries).await.unwrap();

    // Verify all entries exist
    assert_eq!(backend.count().await.unwrap(), 5);

    for (key, expected_value) in entries.iter() {
        let value = backend.get(key).await.unwrap();
        assert_eq!(value, Some(expected_value.to_vec()));
    }

    // Test batch update
    let update_entries: Vec<(&[u8], &[u8])> = vec![
        (b"batch:1", b"updated1"),
        (b"batch:2", b"updated2"),
    ];

    backend.batch_put(&update_entries).await.unwrap();

    assert_eq!(
        backend.get(b"batch:1").await.unwrap(),
        Some(b"updated1".to_vec())
    );

    backend.clear().await.unwrap();
}

#[tokio::test]
async fn test_postgres_checkpoint_restore() {
    let Some(backend) = create_test_backend().await else {
        return;
    };

    // Create initial state
    backend.put(b"state:1", b"initial1").await.unwrap();
    backend.put(b"state:2", b"initial2").await.unwrap();
    backend.put(b"state:3", b"initial3").await.unwrap();

    let initial_count = backend.count().await.unwrap();
    assert_eq!(initial_count, 3);

    // Create checkpoint
    let checkpoint_id = backend.create_checkpoint().await.unwrap();
    println!("Created checkpoint: {}", checkpoint_id);

    // Verify checkpoint exists
    let checkpoints = backend.list_checkpoints().await.unwrap();
    assert!(!checkpoints.is_empty());

    let checkpoint = checkpoints
        .iter()
        .find(|c| c.checkpoint_id == checkpoint_id)
        .unwrap();
    assert_eq!(checkpoint.row_count, 3);
    assert!(checkpoint.data_size > 0);

    // Modify state
    backend.put(b"state:1", b"modified").await.unwrap();
    backend.delete(b"state:2").await.unwrap();
    backend.put(b"state:4", b"new_entry").await.unwrap();

    let modified_count = backend.count().await.unwrap();
    assert_eq!(modified_count, 3); // state:1, state:3, state:4

    // Restore checkpoint
    backend.restore_checkpoint(checkpoint_id).await.unwrap();

    // Verify state restored
    let restored_count = backend.count().await.unwrap();
    assert_eq!(restored_count, initial_count);

    assert_eq!(
        backend.get(b"state:1").await.unwrap(),
        Some(b"initial1".to_vec())
    );
    assert_eq!(
        backend.get(b"state:2").await.unwrap(),
        Some(b"initial2".to_vec())
    );
    assert_eq!(backend.get(b"state:4").await.unwrap(), None);

    backend.clear().await.unwrap();
}

#[tokio::test]
async fn test_postgres_concurrent_access() {
    let Some(backend) = create_test_backend().await else {
        return;
    };

    let backend = Arc::new(backend);
    let mut handles = Vec::new();

    // Spawn multiple tasks writing concurrently
    for i in 0..20 {
        let backend = Arc::clone(&backend);
        let handle = tokio::spawn(async move {
            for j in 0..10 {
                let key = format!("concurrent:{}:{}", i, j).into_bytes();
                let value = format!("value:{}:{}", i, j).into_bytes();

                backend.put(&key, &value).await.unwrap();

                // Verify immediately
                let retrieved = backend.get(&key).await.unwrap();
                assert_eq!(retrieved, Some(value));
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all entries
    let count = backend.count().await.unwrap();
    assert_eq!(count, 200); // 20 tasks * 10 entries

    backend.clear().await.unwrap();
}

#[tokio::test]
async fn test_postgres_prefix_queries() {
    let Some(backend) = create_test_backend().await else {
        return;
    };

    // Create entries with different prefixes
    backend.put(b"user:alice:name", b"Alice").await.unwrap();
    backend.put(b"user:alice:age", b"30").await.unwrap();
    backend.put(b"user:bob:name", b"Bob").await.unwrap();
    backend.put(b"user:bob:age", b"25").await.unwrap();
    backend.put(b"session:123", b"token1").await.unwrap();
    backend.put(b"session:456", b"token2").await.unwrap();

    // Test different prefix queries
    let user_keys = backend.list_keys(b"user:").await.unwrap();
    assert_eq!(user_keys.len(), 4);

    let alice_keys = backend.list_keys(b"user:alice:").await.unwrap();
    assert_eq!(alice_keys.len(), 2);

    let session_keys = backend.list_keys(b"session:").await.unwrap();
    assert_eq!(session_keys.len(), 2);

    let all_keys = backend.list_keys(b"").await.unwrap();
    assert_eq!(all_keys.len(), 6);

    backend.clear().await.unwrap();
}

#[tokio::test]
async fn test_postgres_large_values() {
    let Some(backend) = create_test_backend().await else {
        return;
    };

    // Test various value sizes
    let sizes = vec![1024, 10240, 102400, 1048576]; // 1KB, 10KB, 100KB, 1MB

    for size in sizes {
        let key = format!("large:{}", size).into_bytes();
        let value = vec![0u8; size];

        backend.put(&key, &value).await.unwrap();

        let retrieved = backend.get(&key).await.unwrap();
        assert_eq!(retrieved, Some(value));

        backend.delete(&key).await.unwrap();
    }

    backend.clear().await.unwrap();
}

#[tokio::test]
async fn test_postgres_statistics() {
    let Some(backend) = create_test_backend().await else {
        return;
    };

    // Perform various operations
    backend.put(b"key1", b"value1").await.unwrap();
    backend.put(b"key2", b"value2").await.unwrap();
    backend.get(b"key1").await.unwrap();
    backend.get(b"key2").await.unwrap();
    backend.delete(b"key1").await.unwrap();

    // Check statistics
    let stats = backend.stats().await;
    assert!(stats.put_count >= 2);
    assert!(stats.get_count >= 2);
    assert!(stats.delete_count >= 1);
    assert!(stats.query_count >= 5);
    assert!(stats.bytes_written > 0);
    assert!(stats.bytes_read > 0);

    backend.clear().await.unwrap();
}

#[tokio::test]
async fn test_postgres_health_check() {
    let Some(backend) = create_test_backend().await else {
        return;
    };

    let healthy = backend.health_check().await.unwrap();
    assert!(healthy);
}

#[tokio::test]
async fn test_postgres_table_stats() {
    let Some(backend) = create_test_backend().await else {
        return;
    };

    // Add some data
    for i in 0..100 {
        let key = format!("stats:{}", i).into_bytes();
        let value = vec![0u8; 1000]; // 1KB per entry
        backend.put(&key, &value).await.unwrap();
    }

    // Get table statistics
    let (total_size, table_size) = backend.table_stats().await.unwrap();
    assert!(total_size > 0);
    assert!(table_size > 0);
    assert!(total_size >= table_size); // Total includes indexes

    println!("Total size: {} bytes", total_size);
    println!("Table size: {} bytes", table_size);

    backend.clear().await.unwrap();
}

#[tokio::test]
async fn test_postgres_vacuum() {
    let Some(backend) = create_test_backend().await else {
        return;
    };

    // Add and delete data to create dead tuples
    for i in 0..100 {
        let key = format!("vacuum:{}", i).into_bytes();
        backend.put(&key, b"value").await.unwrap();
        backend.delete(&key).await.unwrap();
    }

    // Run vacuum
    assert!(backend.vacuum().await.is_ok());

    backend.clear().await.unwrap();
}

#[tokio::test]
async fn test_postgres_error_handling() {
    let Some(backend) = create_test_backend().await else {
        return;
    };

    // Test non-existent checkpoint restore
    let fake_checkpoint = uuid::Uuid::new_v4();
    let result = backend.restore_checkpoint(fake_checkpoint).await;
    assert!(result.is_err());

    backend.clear().await.unwrap();
}

#[tokio::test]
async fn test_postgres_cleanup_expired_multiple() {
    let Some(backend) = create_test_backend().await else {
        return;
    };

    // Create multiple entries with TTL
    for i in 0..10 {
        let key = format!("expire:{}", i).into_bytes();
        backend
            .put_with_ttl(&key, b"temp", Duration::from_secs(1))
            .await
            .unwrap();
    }

    // Verify they exist
    assert_eq!(backend.count().await.unwrap(), 10);

    // Wait for expiration
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Cleanup all expired
    let deleted = backend.cleanup_expired().await.unwrap();
    assert_eq!(deleted, 10);

    assert_eq!(backend.count().await.unwrap(), 0);

    backend.clear().await.unwrap();
}
