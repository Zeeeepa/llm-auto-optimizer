//! Comprehensive tests for Enhanced PostgreSQL Backend
//!
//! These tests require a running PostgreSQL instance.
//! Run with: docker run -d -p 5432:5432 -e POSTGRES_PASSWORD=postgres postgres:15

#[cfg(test)]
mod tests {
    use super::super::postgres_backend_enhanced::*;
    use super::super::backend::StateBackend;
    use std::time::Duration;

    // Test helper to create backend
    async fn create_test_backend() -> Option<EnhancedPostgresBackend> {
        let config = EnhancedPostgresConfig::new(
            "postgresql://postgres:postgres@localhost/optimizer_test_enhanced"
        )
        .with_pool_config(PostgresPoolConfig {
            min_connections: 1,
            max_connections: 5,
            acquire_timeout: Duration::from_secs(10),
            idle_timeout: Some(Duration::from_secs(300)),
            max_lifetime: Some(Duration::from_secs(600)),
            health_check_interval: Duration::from_secs(30),
            test_on_acquire: true,
        });

        match EnhancedPostgresBackend::new(config).await {
            Ok(backend) => {
                let _ = backend.clear().await;
                Some(backend)
            }
            Err(e) => {
                eprintln!("Skipping PostgreSQL tests - database not available: {}", e);
                None
            }
        }
    }

    // ========================================================================
    // Basic StateBackend Tests
    // ========================================================================

    #[tokio::test]
    async fn test_basic_get_put_delete() {
        if let Some(backend) = create_test_backend().await {
            // Put
            backend.put(b"test_key", b"test_value").await.unwrap();

            // Get
            let value = backend.get(b"test_key").await.unwrap();
            assert_eq!(value, Some(b"test_value".to_vec()));

            // Delete
            backend.delete(b"test_key").await.unwrap();

            // Get after delete
            let value = backend.get(b"test_key").await.unwrap();
            assert_eq!(value, None);
        }
    }

    #[tokio::test]
    async fn test_contains() {
        if let Some(backend) = create_test_backend().await {
            backend.put(b"exists_key", b"value").await.unwrap();

            assert!(backend.contains(b"exists_key").await.unwrap());
            assert!(!backend.contains(b"nonexistent").await.unwrap());
        }
    }

    #[tokio::test]
    async fn test_count() {
        if let Some(backend) = create_test_backend().await {
            backend.clear().await.unwrap();

            assert_eq!(backend.count().await.unwrap(), 0);

            backend.put(b"key1", b"val1").await.unwrap();
            backend.put(b"key2", b"val2").await.unwrap();
            backend.put(b"key3", b"val3").await.unwrap();

            assert_eq!(backend.count().await.unwrap(), 3);
        }
    }

    #[tokio::test]
    async fn test_list_keys() {
        if let Some(backend) = create_test_backend().await {
            backend.clear().await.unwrap();

            backend.put(b"prefix:key1", b"val1").await.unwrap();
            backend.put(b"prefix:key2", b"val2").await.unwrap();
            backend.put(b"other:key3", b"val3").await.unwrap();

            let keys = backend.list_keys(b"prefix:").await.unwrap();
            assert_eq!(keys.len(), 2);

            let all_keys = backend.list_keys(b"").await.unwrap();
            assert_eq!(all_keys.len(), 3);
        }
    }

    // ========================================================================
    // Enhanced Features Tests
    // ========================================================================

    #[tokio::test]
    async fn test_put_with_metadata() {
        if let Some(backend) = create_test_backend().await {
            let metadata = serde_json::json!({
                "source": "kafka",
                "partition": 5,
                "offset": 12345
            });

            backend
                .put_with_metadata(b"meta_key", b"meta_value", metadata)
                .await
                .unwrap();

            let value = backend.get(b"meta_key").await.unwrap();
            assert_eq!(value, Some(b"meta_value".to_vec()));
        }
    }

    #[tokio::test]
    async fn test_put_with_ttl() {
        if let Some(backend) = create_test_backend().await {
            // Put with 1 second TTL
            backend
                .put_with_ttl(b"ttl_key", b"ttl_value", Duration::from_secs(1))
                .await
                .unwrap();

            // Should exist immediately
            assert!(backend.contains(b"ttl_key").await.unwrap());

            // Wait for expiration
            tokio::time::sleep(Duration::from_secs(2)).await;

            // Should be filtered out
            assert!(!backend.contains(b"ttl_key").await.unwrap());

            // Cleanup
            let deleted = backend.cleanup_expired().await.unwrap();
            assert_eq!(deleted, 1);
        }
    }

    #[tokio::test]
    async fn test_batch_put() {
        if let Some(backend) = create_test_backend().await {
            let entries: Vec<(&[u8], &[u8])> = vec![
                (b"batch1", b"value1"),
                (b"batch2", b"value2"),
                (b"batch3", b"value3"),
                (b"batch4", b"value4"),
                (b"batch5", b"value5"),
            ];

            backend.batch_put(&entries).await.unwrap();

            assert_eq!(backend.count().await.unwrap(), 5);

            for (key, expected_value) in &entries {
                let value = backend.get(key).await.unwrap();
                assert_eq!(value, Some(expected_value.to_vec()));
            }
        }
    }

    #[tokio::test]
    async fn test_batch_copy() {
        if let Some(backend) = create_test_backend().await {
            backend.clear().await.unwrap();

            let mut entries = Vec::new();
            for i in 0..100 {
                entries.push((
                    format!("copy_key_{}", i).into_bytes(),
                    format!("copy_value_{}", i).into_bytes(),
                ));
            }

            let refs: Vec<(&[u8], &[u8])> = entries
                .iter()
                .map(|(k, v)| (k.as_slice(), v.as_slice()))
                .collect();

            backend.batch_copy(&refs).await.unwrap();

            assert_eq!(backend.count().await.unwrap(), 100);
        }
    }

    // ========================================================================
    // Transaction Tests
    // ========================================================================

    #[tokio::test]
    async fn test_transaction_commit() {
        if let Some(backend) = create_test_backend().await {
            backend.clear().await.unwrap();

            let mut tx = backend
                .begin_transaction(IsolationLevel::ReadCommitted)
                .await
                .unwrap();

            sqlx::query("INSERT INTO state_entries (key, value) VALUES ($1, $2)")
                .bind(b"tx_key")
                .bind(b"tx_value")
                .execute(tx.executor())
                .await
                .unwrap();

            tx.commit().await.unwrap();

            let value = backend.get(b"tx_key").await.unwrap();
            assert_eq!(value, Some(b"tx_value".to_vec()));
        }
    }

    #[tokio::test]
    async fn test_transaction_rollback() {
        if let Some(backend) = create_test_backend().await {
            backend.clear().await.unwrap();

            let mut tx = backend
                .begin_transaction(IsolationLevel::ReadCommitted)
                .await
                .unwrap();

            sqlx::query("INSERT INTO state_entries (key, value) VALUES ($1, $2)")
                .bind(b"rollback_key")
                .bind(b"rollback_value")
                .execute(tx.executor())
                .await
                .unwrap();

            tx.rollback().await.unwrap();

            let value = backend.get(b"rollback_key").await.unwrap();
            assert_eq!(value, None);
        }
    }

    #[tokio::test]
    async fn test_transaction_savepoint() {
        if let Some(backend) = create_test_backend().await {
            backend.clear().await.unwrap();

            let mut tx = backend
                .begin_transaction(IsolationLevel::ReadCommitted)
                .await
                .unwrap();

            // Insert first value
            sqlx::query("INSERT INTO state_entries (key, value) VALUES ($1, $2)")
                .bind(b"sp_key1")
                .bind(b"sp_value1")
                .execute(tx.executor())
                .await
                .unwrap();

            // Create savepoint
            tx.savepoint("sp1").await.unwrap();

            // Insert second value
            sqlx::query("INSERT INTO state_entries (key, value) VALUES ($1, $2)")
                .bind(b"sp_key2")
                .bind(b"sp_value2")
                .execute(tx.executor())
                .await
                .unwrap();

            // Rollback to savepoint
            tx.rollback_to_savepoint("sp1").await.unwrap();

            tx.commit().await.unwrap();

            // First key should exist
            assert!(backend.contains(b"sp_key1").await.unwrap());
            // Second key should not exist
            assert!(!backend.contains(b"sp_key2").await.unwrap());
        }
    }

    #[tokio::test]
    async fn test_optimistic_locking() {
        if let Some(backend) = create_test_backend().await {
            backend.clear().await.unwrap();

            // Initial put
            let v1 = backend.put_with_version(b"lock_key", b"value1", None).await.unwrap();
            assert_eq!(v1, 1);

            // Update with correct version
            let v2 = backend
                .put_with_version(b"lock_key", b"value2", Some(v1))
                .await
                .unwrap();
            assert_eq!(v2, 2);

            // Update with stale version should fail
            let result = backend
                .put_with_version(b"lock_key", b"value3", Some(v1))
                .await;
            assert!(result.is_err());

            // Update with current version should succeed
            let v3 = backend
                .put_with_version(b"lock_key", b"value3", Some(v2))
                .await
                .unwrap();
            assert_eq!(v3, 3);
        }
    }

    // ========================================================================
    // Distributed Coordination Tests
    // ========================================================================

    #[tokio::test]
    async fn test_advisory_lock() {
        if let Some(backend) = create_test_backend().await {
            let lock_id = 12345i64;

            // Acquire lock
            assert!(backend.acquire_advisory_lock(lock_id).await.unwrap());

            // Try to acquire again (should fail)
            assert!(!backend.acquire_advisory_lock(lock_id).await.unwrap());

            // Release lock
            backend.release_advisory_lock(lock_id).await.unwrap();

            // Should be able to acquire again
            assert!(backend.acquire_advisory_lock(lock_id).await.unwrap());

            // Cleanup
            backend.release_advisory_lock(lock_id).await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_lock_guard() {
        if let Some(backend) = create_test_backend().await {
            let guard = PostgresLockGuard::new(
                backend.clone_handle(),
                "test_lock",
                Duration::from_secs(5),
            )
            .await
            .unwrap();

            assert!(guard.is_some());

            // Try to acquire same lock (should timeout quickly)
            let guard2 = PostgresLockGuard::new(
                backend.clone_handle(),
                "test_lock",
                Duration::from_millis(100),
            )
            .await
            .unwrap();

            assert!(guard2.is_none());

            // Release first guard
            guard.unwrap().release().await.unwrap();

            // Now should be able to acquire
            let guard3 = PostgresLockGuard::new(
                backend.clone_handle(),
                "test_lock",
                Duration::from_secs(5),
            )
            .await
            .unwrap();

            assert!(guard3.is_some());
        }
    }

    #[tokio::test]
    async fn test_notify() {
        if let Some(backend) = create_test_backend().await {
            backend.notify("test_channel", "test_message").await.unwrap();
            // Note: To fully test this, we'd need a listener in a separate connection
        }
    }

    #[tokio::test]
    async fn test_leader_election() {
        if let Some(backend) = create_test_backend().await {
            // First instance becomes leader
            let is_leader1 = backend
                .try_become_leader("election_test", Duration::from_secs(10))
                .await
                .unwrap();

            assert!(is_leader1);

            // Second instance should not become leader
            let is_leader2 = backend
                .try_become_leader("election_test", Duration::from_secs(10))
                .await
                .unwrap();

            assert!(!is_leader2);
        }
    }

    // ========================================================================
    // Data Management Tests
    // ========================================================================

    #[tokio::test]
    async fn test_vacuum() {
        if let Some(backend) = create_test_backend().await {
            // Regular vacuum
            backend.vacuum_analyze(false).await.unwrap();

            // Full vacuum (may be slow)
            // backend.vacuum_analyze(true).await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_retention_policy() {
        if let Some(backend) = create_test_backend().await {
            backend.clear().await.unwrap();

            // Insert some old data (would need to manipulate created_at in real scenario)
            backend.put(b"old_key", b"old_value").await.unwrap();

            // Apply retention policy (nothing should be deleted with default config)
            let deleted = backend.apply_retention_policy().await.unwrap();
            assert_eq!(deleted, 0);
        }
    }

    #[tokio::test]
    async fn test_table_statistics() {
        if let Some(backend) = create_test_backend().await {
            backend.clear().await.unwrap();

            // Insert some data
            for i in 0..10 {
                backend
                    .put(format!("stat_key_{}", i).as_bytes(), b"stat_value")
                    .await
                    .unwrap();
            }

            let stats = backend.get_table_statistics().await.unwrap();

            assert!(stats.total_rows >= 10);
            assert!(stats.total_size > 0);
            assert!(stats.table_size > 0);
        }
    }

    // ========================================================================
    // High Availability Tests
    // ========================================================================

    #[tokio::test]
    async fn test_replication_status() {
        if let Some(backend) = create_test_backend().await {
            let status = backend.check_replication_status().await.unwrap();

            // On a primary instance
            assert!(!status.is_replica);
            assert_eq!(status.lag_ms, 0);
        }
    }

    #[tokio::test]
    async fn test_checkpoint() {
        if let Some(backend) = create_test_backend().await {
            backend.checkpoint().await.unwrap();
        }
    }

    // ========================================================================
    // Observability Tests
    // ========================================================================

    #[tokio::test]
    async fn test_statistics_tracking() {
        if let Some(backend) = create_test_backend().await {
            backend.clear().await.unwrap();

            // Perform operations
            backend.put(b"stat1", b"value1").await.unwrap();
            backend.get(b"stat1").await.unwrap();
            backend.delete(b"stat1").await.unwrap();

            let stats = backend.get_stats();

            assert!(stats.put_count >= 1);
            assert!(stats.get_count >= 1);
            assert!(stats.delete_count >= 1);
            assert!(stats.bytes_written > 0);
            assert!(stats.bytes_read > 0);
        }
    }

    #[tokio::test]
    async fn test_pool_statistics() {
        if let Some(backend) = create_test_backend().await {
            let pool_stats = backend.get_pool_stats();

            assert!(pool_stats.size > 0);
            assert!(pool_stats.max_size > 0);
            assert!(pool_stats.idle <= pool_stats.size);
            assert!(pool_stats.active <= pool_stats.size);
        }
    }

    #[tokio::test]
    async fn test_health_check() {
        if let Some(backend) = create_test_backend().await {
            assert!(backend.health_check().await.unwrap());
        }
    }

    // ========================================================================
    // Metadata Query Tests
    // ========================================================================

    #[tokio::test]
    async fn test_query_by_metadata() {
        if let Some(backend) = create_test_backend().await {
            backend.clear().await.unwrap();

            // Insert entries with metadata
            backend
                .put_with_metadata(
                    b"meta1",
                    b"value1",
                    serde_json::json!({"type": "user", "id": 1}),
                )
                .await
                .unwrap();

            backend
                .put_with_metadata(
                    b"meta2",
                    b"value2",
                    serde_json::json!({"type": "user", "id": 2}),
                )
                .await
                .unwrap();

            backend
                .put_with_metadata(
                    b"meta3",
                    b"value3",
                    serde_json::json!({"type": "system", "id": 3}),
                )
                .await
                .unwrap();

            // Query by metadata
            let results = backend
                .query_by_metadata("type", &serde_json::json!({"type": "user"}))
                .await
                .unwrap();

            assert_eq!(results.len(), 2);
        }
    }

    // ========================================================================
    // Stress Tests
    // ========================================================================

    #[tokio::test]
    async fn test_concurrent_operations() {
        if let Some(backend) = create_test_backend().await {
            backend.clear().await.unwrap();

            let backend = std::sync::Arc::new(backend);
            let mut handles = vec![];

            // Spawn multiple concurrent operations
            for i in 0..10 {
                let backend_clone = backend.clone();
                let handle = tokio::spawn(async move {
                    for j in 0..10 {
                        let key = format!("concurrent_{}_{}", i, j);
                        let value = format!("value_{}_{}", i, j);
                        backend_clone
                            .put(key.as_bytes(), value.as_bytes())
                            .await
                            .unwrap();
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.await.unwrap();
            }

            assert_eq!(backend.count().await.unwrap(), 100);
        }
    }
}
