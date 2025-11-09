//! Comprehensive integration tests for distributed state backends
//!
//! These tests require running services. Start services with:
//!
//! ```bash
//! docker-compose up -d redis postgres
//! ```
//!
//! Run tests with:
//!
//! ```bash
//! cargo test --test distributed_state_tests -- --ignored --test-threads=1
//! ```

use processor::state::{
    cached_backend::CachedStateBackend, postgres_backend::PostgresStateBackend,
    redis_backend::RedisStateBackend, redis_lock::RedisLock, DistributedLock, RedisConfig,
    StateBackend,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

// ============================================================================
// Redis Backend Tests
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_redis_basic_operations() {
    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("test_basic:")
        .build()
        .expect("Failed to build config");

    let backend = RedisStateBackend::new(config)
        .await
        .expect("Failed to connect to Redis");

    backend.clear().await.unwrap();

    // Test PUT
    backend.put(b"key1", b"value1").await.unwrap();

    // Test GET
    let value = backend.get(b"key1").await.unwrap();
    assert_eq!(value, Some(b"value1".to_vec()));

    // Test overwrite
    backend.put(b"key1", b"value2").await.unwrap();
    let value = backend.get(b"key1").await.unwrap();
    assert_eq!(value, Some(b"value2".to_vec()));

    // Test DELETE
    backend.delete(b"key1").await.unwrap();
    let value = backend.get(b"key1").await.unwrap();
    assert_eq!(value, None);

    // Test non-existent key
    let value = backend.get(b"nonexistent").await.unwrap();
    assert_eq!(value, None);

    backend.clear().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_redis_ttl_expiration() {
    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("test_ttl:")
        .default_ttl(Duration::from_secs(2))
        .build()
        .expect("Failed to build config");

    let backend = RedisStateBackend::new(config)
        .await
        .expect("Failed to connect to Redis");

    backend.clear().await.unwrap();

    // Put key with default TTL
    backend.put(b"expiring_key", b"value").await.unwrap();

    // Should exist immediately
    assert!(backend.contains(b"expiring_key").await.unwrap());

    // Check TTL
    let ttl = backend.ttl(b"expiring_key").await.unwrap();
    assert!(ttl.is_some());
    assert!(ttl.unwrap().as_secs() <= 2);

    // Wait for expiration
    sleep(Duration::from_secs(3)).await;

    // Should be expired
    assert!(!backend.contains(b"expiring_key").await.unwrap());

    backend.clear().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_redis_custom_ttl() {
    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("test_custom_ttl:")
        .build()
        .expect("Failed to build config");

    let backend = RedisStateBackend::new(config)
        .await
        .expect("Failed to connect to Redis");

    backend.clear().await.unwrap();

    // Put with custom TTL
    backend
        .put_with_ttl(b"short_lived", b"value", Duration::from_secs(1))
        .await
        .unwrap();

    assert!(backend.contains(b"short_lived").await.unwrap());

    sleep(Duration::from_secs(2)).await;

    assert!(!backend.contains(b"short_lived").await.unwrap());

    backend.clear().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_redis_batch_operations() {
    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("test_batch:")
        .build()
        .expect("Failed to build config");

    let backend = RedisStateBackend::new(config)
        .await
        .expect("Failed to connect to Redis");

    backend.clear().await.unwrap();

    // Batch PUT
    let pairs = vec![
        (&b"batch_key1"[..], &b"value1"[..]),
        (&b"batch_key2"[..], &b"value2"[..]),
        (&b"batch_key3"[..], &b"value3"[..]),
        (&b"batch_key4"[..], &b"value4"[..]),
        (&b"batch_key5"[..], &b"value5"[..]),
    ];

    backend.batch_put(&pairs).await.unwrap();

    // Batch GET
    let keys: Vec<&[u8]> = vec![
        b"batch_key1",
        b"batch_key2",
        b"batch_key3",
        b"batch_key4",
        b"batch_key5",
    ];

    let values = backend.batch_get(&keys).await.unwrap();

    assert_eq!(values.len(), 5);
    for (i, value) in values.iter().enumerate() {
        let expected = format!("value{}", i + 1);
        assert_eq!(value, &Some(expected.into_bytes()));
    }

    // Batch DELETE
    let deleted = backend.batch_delete(&keys).await.unwrap();
    assert_eq!(deleted, 5);

    // Verify deletion
    let values = backend.batch_get(&keys).await.unwrap();
    for value in values {
        assert_eq!(value, None);
    }

    backend.clear().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_redis_list_keys() {
    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("test_list:")
        .build()
        .expect("Failed to build config");

    let backend = RedisStateBackend::new(config)
        .await
        .expect("Failed to connect to Redis");

    backend.clear().await.unwrap();

    // Create keys with different prefixes
    backend.put(b"user:1", b"alice").await.unwrap();
    backend.put(b"user:2", b"bob").await.unwrap();
    backend.put(b"user:3", b"charlie").await.unwrap();
    backend.put(b"session:1", b"sess1").await.unwrap();
    backend.put(b"session:2", b"sess2").await.unwrap();

    // List all keys
    let all_keys = backend.list_keys(b"").await.unwrap();
    assert_eq!(all_keys.len(), 5);

    // List user keys
    let user_keys = backend.list_keys(b"user:").await.unwrap();
    assert_eq!(user_keys.len(), 3);

    // List session keys
    let session_keys = backend.list_keys(b"session:").await.unwrap();
    assert_eq!(session_keys.len(), 2);

    backend.clear().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_redis_atomic_operations() {
    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("test_atomic:")
        .build()
        .expect("Failed to build config");

    let backend = RedisStateBackend::new(config)
        .await
        .expect("Failed to connect to Redis");

    backend.clear().await.unwrap();

    // Test get_and_delete
    backend.put(b"atomic_key", b"atomic_value").await.unwrap();

    let value = backend.get_and_delete(b"atomic_key").await.unwrap();
    assert_eq!(value, Some(b"atomic_value".to_vec()));

    // Key should be deleted
    assert!(!backend.contains(b"atomic_key").await.unwrap());

    // Test set_if_not_exists
    let set1 = backend
        .set_if_not_exists(b"nx_key", b"first", Duration::from_secs(60))
        .await
        .unwrap();
    assert!(set1, "First set should succeed");

    let set2 = backend
        .set_if_not_exists(b"nx_key", b"second", Duration::from_secs(60))
        .await
        .unwrap();
    assert!(!set2, "Second set should fail");

    // Verify first value is preserved
    let value = backend.get(b"nx_key").await.unwrap();
    assert_eq!(value, Some(b"first".to_vec()));

    backend.clear().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_redis_checkpoint_restore() {
    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("test_checkpoint:")
        .build()
        .expect("Failed to build config");

    let backend = RedisStateBackend::new(config)
        .await
        .expect("Failed to connect to Redis");

    backend.clear().await.unwrap();

    // Create some state
    backend.put(b"key1", b"value1").await.unwrap();
    backend.put(b"key2", b"value2").await.unwrap();
    backend.put(b"key3", b"value3").await.unwrap();

    // Create checkpoint
    let checkpoint = backend.checkpoint().await.unwrap();
    assert_eq!(checkpoint.len(), 3);

    // Clear state
    backend.clear().await.unwrap();
    assert_eq!(backend.count().await.unwrap(), 0);

    // Restore from checkpoint
    backend.restore(checkpoint).await.unwrap();
    assert_eq!(backend.count().await.unwrap(), 3);

    // Verify data
    assert_eq!(backend.get(b"key1").await.unwrap(), Some(b"value1".to_vec()));
    assert_eq!(backend.get(b"key2").await.unwrap(), Some(b"value2".to_vec()));
    assert_eq!(backend.get(b"key3").await.unwrap(), Some(b"value3".to_vec()));

    backend.clear().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_redis_stats_tracking() {
    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("test_stats:")
        .build()
        .expect("Failed to build config");

    let backend = RedisStateBackend::new(config)
        .await
        .expect("Failed to connect to Redis");

    backend.clear().await.unwrap();

    // Perform various operations
    backend.put(b"key1", b"value1").await.unwrap();
    backend.put(b"key2", b"value2").await.unwrap();

    backend.get(b"key1").await.unwrap(); // Hit
    backend.get(b"key2").await.unwrap(); // Hit
    backend.get(b"nonexistent").await.unwrap(); // Miss

    backend.delete(b"key1").await.unwrap();

    // Check stats
    let stats = backend.stats().await;
    assert_eq!(stats.put_count, 2);
    assert_eq!(stats.get_count, 3);
    assert_eq!(stats.delete_count, 1);
    assert_eq!(stats.hit_count, 2);
    assert_eq!(stats.miss_count, 1);
    assert!(stats.hit_rate() > 0.6 && stats.hit_rate() < 0.7);
    assert_eq!(stats.total_operations(), 6);

    backend.clear().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_redis_health_check() {
    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("test_health:")
        .build()
        .expect("Failed to build config");

    let backend = RedisStateBackend::new(config)
        .await
        .expect("Failed to connect to Redis");

    let healthy = backend.health_check().await.unwrap();
    assert!(healthy);
}

// ============================================================================
// PostgreSQL Backend Tests
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_postgres_basic_operations() {
    let backend = PostgresStateBackend::new("postgres://optimizer:password@localhost:5432/optimizer", "test_basic")
        .await
        .expect("Failed to connect to PostgreSQL");

    backend.clear().await.unwrap();

    // Test PUT
    backend.put(b"key1", b"value1").await.unwrap();

    // Test GET
    let value = backend.get(b"key1").await.unwrap();
    assert_eq!(value, Some(b"value1".to_vec()));

    // Test overwrite
    backend.put(b"key1", b"value2").await.unwrap();
    let value = backend.get(b"key1").await.unwrap();
    assert_eq!(value, Some(b"value2".to_vec()));

    // Test DELETE
    backend.delete(b"key1").await.unwrap();
    let value = backend.get(b"key1").await.unwrap();
    assert_eq!(value, None);

    backend.clear().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_postgres_transactions() {
    let backend = PostgresStateBackend::new("postgres://optimizer:password@localhost:5432/optimizer", "test_txn")
        .await
        .expect("Failed to connect to PostgreSQL");

    backend.clear().await.unwrap();

    // Begin transaction
    let txn = backend.begin_transaction().await.unwrap();

    // Put within transaction
    backend
        .put_in_transaction(&txn, b"txn_key1", b"value1")
        .await
        .unwrap();

    backend
        .put_in_transaction(&txn, b"txn_key2", b"value2")
        .await
        .unwrap();

    // Data shouldn't be visible outside transaction yet
    // (This would require a separate connection to test properly)

    // Commit transaction
    backend.commit_transaction(txn).await.unwrap();

    // Now data should be visible
    assert_eq!(
        backend.get(b"txn_key1").await.unwrap(),
        Some(b"value1".to_vec())
    );
    assert_eq!(
        backend.get(b"txn_key2").await.unwrap(),
        Some(b"value2".to_vec())
    );

    backend.clear().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_postgres_rollback() {
    let backend = PostgresStateBackend::new("postgres://optimizer:password@localhost:5432/optimizer", "test_rollback")
        .await
        .expect("Failed to connect to PostgreSQL");

    backend.clear().await.unwrap();

    // Begin transaction
    let txn = backend.begin_transaction().await.unwrap();

    // Put within transaction
    backend
        .put_in_transaction(&txn, b"rollback_key", b"value")
        .await
        .unwrap();

    // Rollback transaction
    backend.rollback_transaction(txn).await.unwrap();

    // Data should not be persisted
    assert_eq!(backend.get(b"rollback_key").await.unwrap(), None);

    backend.clear().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_postgres_batch_operations() {
    let backend = PostgresStateBackend::new("postgres://optimizer:password@localhost:5432/optimizer", "test_batch")
        .await
        .expect("Failed to connect to PostgreSQL");

    backend.clear().await.unwrap();

    // Batch put
    let pairs = vec![
        (&b"batch1"[..], &b"value1"[..]),
        (&b"batch2"[..], &b"value2"[..]),
        (&b"batch3"[..], &b"value3"[..]),
    ];

    backend.batch_upsert(&pairs).await.unwrap();

    // Verify
    assert_eq!(backend.get(b"batch1").await.unwrap(), Some(b"value1".to_vec()));
    assert_eq!(backend.get(b"batch2").await.unwrap(), Some(b"value2".to_vec()));
    assert_eq!(backend.get(b"batch3").await.unwrap(), Some(b"value3".to_vec()));

    backend.clear().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_postgres_vacuum() {
    let backend = PostgresStateBackend::new("postgres://optimizer:password@localhost:5432/optimizer", "test_vacuum")
        .await
        .expect("Failed to connect to PostgreSQL");

    backend.clear().await.unwrap();

    // Create and delete data
    for i in 0..100 {
        let key = format!("key{}", i);
        backend.put(key.as_bytes(), b"value").await.unwrap();
    }

    for i in 0..50 {
        let key = format!("key{}", i);
        backend.delete(key.as_bytes()).await.unwrap();
    }

    // Run vacuum
    backend.vacuum().await.unwrap();

    // Should still have 50 keys
    assert_eq!(backend.count().await.unwrap(), 50);

    backend.clear().await.unwrap();
}

// ============================================================================
// Distributed Lock Tests
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_redis_lock_acquire_release() {
    let lock = RedisLock::new("redis://localhost:6379", "test_lock_basic")
        .await
        .expect("Failed to create Redis lock");

    // Acquire lock
    let acquired = lock.try_lock(Duration::from_secs(10)).await.unwrap();
    assert!(acquired, "Should acquire lock on first attempt");

    // Try to acquire again (should fail)
    let acquired2 = lock.try_lock(Duration::from_secs(10)).await.unwrap();
    assert!(!acquired2, "Should not acquire lock when already held");

    // Release lock
    lock.unlock().await.unwrap();

    // Should be able to acquire again
    let acquired3 = lock.try_lock(Duration::from_secs(10)).await.unwrap();
    assert!(acquired3, "Should acquire lock after release");

    lock.unlock().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_redis_lock_expiration() {
    let lock = RedisLock::new("redis://localhost:6379", "test_lock_expire")
        .await
        .expect("Failed to create Redis lock");

    // Acquire lock with short TTL
    let acquired = lock.try_lock(Duration::from_secs(1)).await.unwrap();
    assert!(acquired);

    // Wait for expiration
    sleep(Duration::from_secs(2)).await;

    // Create new lock instance (simulating different process)
    let lock2 = RedisLock::new("redis://localhost:6379", "test_lock_expire")
        .await
        .expect("Failed to create Redis lock");

    // Should be able to acquire expired lock
    let acquired2 = lock2.try_lock(Duration::from_secs(10)).await.unwrap();
    assert!(acquired2, "Should acquire expired lock");

    lock2.unlock().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_redis_lock_multi_instance() {
    let lock1 = RedisLock::new("redis://localhost:6379", "test_lock_multi")
        .await
        .expect("Failed to create Redis lock 1");

    let lock2 = RedisLock::new("redis://localhost:6379", "test_lock_multi")
        .await
        .expect("Failed to create Redis lock 2");

    // Lock 1 acquires
    let acquired1 = lock1.try_lock(Duration::from_secs(10)).await.unwrap();
    assert!(acquired1);

    // Lock 2 fails to acquire
    let acquired2 = lock2.try_lock(Duration::from_secs(10)).await.unwrap();
    assert!(!acquired2);

    // Lock 1 releases
    lock1.unlock().await.unwrap();

    // Lock 2 can now acquire
    let acquired3 = lock2.try_lock(Duration::from_secs(10)).await.unwrap();
    assert!(acquired3);

    lock2.unlock().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_redis_lock_with_block() {
    let lock = RedisLock::new("redis://localhost:6379", "test_lock_block")
        .await
        .expect("Failed to create Redis lock");

    // Use lock with closure
    let result = lock
        .with_lock(Duration::from_secs(10), || async {
            // Critical section
            sleep(Duration::from_millis(100)).await;
            Ok::<_, anyhow::Error>(42)
        })
        .await
        .unwrap();

    assert_eq!(result, 42);

    // Lock should be released automatically
    let lock2 = RedisLock::new("redis://localhost:6379", "test_lock_block")
        .await
        .expect("Failed to create Redis lock");

    let acquired = lock2.try_lock(Duration::from_secs(10)).await.unwrap();
    assert!(acquired, "Lock should be released after with_lock");

    lock2.unlock().await.unwrap();
}

// ============================================================================
// Cached Backend Tests
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_cached_backend_hit_rate() {
    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("test_cache:")
        .build()
        .expect("Failed to build config");

    let redis = Arc::new(
        RedisStateBackend::new(config)
            .await
            .expect("Failed to connect to Redis"),
    );

    let cached = CachedStateBackend::new(redis.clone(), 100, Some(Duration::from_secs(60)));

    cached.clear().await.unwrap();

    // First access (miss)
    cached.put(b"cached_key", b"value").await.unwrap();

    // Second access (hit from cache)
    let value1 = cached.get(b"cached_key").await.unwrap();
    assert_eq!(value1, Some(b"value".to_vec()));

    // Third access (hit from cache)
    let value2 = cached.get(b"cached_key").await.unwrap();
    assert_eq!(value2, Some(b"value".to_vec()));

    // Check stats
    let stats = cached.cache_stats().await;
    assert!(stats.hit_rate > 0.0);

    cached.clear().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_cached_backend_eviction() {
    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("test_evict:")
        .build()
        .expect("Failed to build config");

    let redis = Arc::new(
        RedisStateBackend::new(config)
            .await
            .expect("Failed to connect to Redis"),
    );

    // Small cache (10 entries)
    let cached = CachedStateBackend::new(redis.clone(), 10, None);

    cached.clear().await.unwrap();

    // Insert more than cache capacity
    for i in 0..20 {
        let key = format!("key{}", i);
        let value = format!("value{}", i);
        cached.put(key.as_bytes(), value.as_bytes()).await.unwrap();
    }

    // All should be in persistent backend
    assert_eq!(cached.count().await.unwrap(), 20);

    // But cache size should be limited
    let stats = cached.cache_stats().await;
    assert!(stats.size <= 10);

    cached.clear().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_cached_backend_ttl() {
    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("test_cache_ttl:")
        .build()
        .expect("Failed to build config");

    let redis = Arc::new(
        RedisStateBackend::new(config)
            .await
            .expect("Failed to connect to Redis"),
    );

    // Cache with 1-second TTL
    let cached = CachedStateBackend::new(redis.clone(), 100, Some(Duration::from_secs(1)));

    cached.clear().await.unwrap();

    cached.put(b"ttl_key", b"value").await.unwrap();

    // Should be in cache
    let value1 = cached.get(b"ttl_key").await.unwrap();
    assert_eq!(value1, Some(b"value".to_vec()));

    // Wait for cache expiration
    sleep(Duration::from_secs(2)).await;

    // Should fetch from backend (cache miss)
    let value2 = cached.get(b"ttl_key").await.unwrap();
    assert_eq!(value2, Some(b"value".to_vec()));

    cached.clear().await.unwrap();
}

// ============================================================================
// Failover Scenarios
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_redis_reconnection() {
    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("test_reconnect:")
        .max_retries(5)
        .build()
        .expect("Failed to build config");

    let backend = RedisStateBackend::new(config)
        .await
        .expect("Failed to connect to Redis");

    backend.clear().await.unwrap();

    // Put some data
    backend.put(b"key1", b"value1").await.unwrap();

    // Verify
    let value = backend.get(b"key1").await.unwrap();
    assert_eq!(value, Some(b"value1".to_vec()));

    // NOTE: To fully test reconnection, you would need to:
    // 1. Stop Redis: docker-compose stop redis
    // 2. Wait for connection to fail
    // 3. Restart Redis: docker-compose start redis
    // 4. Verify operations resume

    backend.clear().await.unwrap();
}

// ============================================================================
// Performance Benchmarks
// ============================================================================

#[tokio::test]
#[ignore]
async fn bench_redis_write_throughput() {
    use std::time::Instant;

    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("bench_write:")
        .build()
        .expect("Failed to build config");

    let backend = RedisStateBackend::new(config)
        .await
        .expect("Failed to connect to Redis");

    backend.clear().await.unwrap();

    let count = 1000;
    let start = Instant::now();

    for i in 0..count {
        let key = format!("key{}", i);
        backend.put(key.as_bytes(), b"value").await.unwrap();
    }

    let elapsed = start.elapsed();
    let ops_per_sec = count as f64 / elapsed.as_secs_f64();

    println!(
        "Redis write throughput: {:.2} ops/sec ({} ops in {:?})",
        ops_per_sec, count, elapsed
    );

    assert!(ops_per_sec > 100.0, "Write throughput too low");

    backend.clear().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn bench_redis_read_throughput() {
    use std::time::Instant;

    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("bench_read:")
        .build()
        .expect("Failed to build config");

    let backend = RedisStateBackend::new(config)
        .await
        .expect("Failed to connect to Redis");

    backend.clear().await.unwrap();

    // Prepare data
    let count = 1000;
    for i in 0..count {
        let key = format!("key{}", i);
        backend.put(key.as_bytes(), b"value").await.unwrap();
    }

    // Benchmark reads
    let start = Instant::now();

    for i in 0..count {
        let key = format!("key{}", i);
        backend.get(key.as_bytes()).await.unwrap();
    }

    let elapsed = start.elapsed();
    let ops_per_sec = count as f64 / elapsed.as_secs_f64();

    println!(
        "Redis read throughput: {:.2} ops/sec ({} ops in {:?})",
        ops_per_sec, count, elapsed
    );

    assert!(ops_per_sec > 100.0, "Read throughput too low");

    backend.clear().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn bench_postgres_write_throughput() {
    use std::time::Instant;

    let backend = PostgresStateBackend::new("postgres://optimizer:password@localhost:5432/optimizer", "bench_write")
        .await
        .expect("Failed to connect to PostgreSQL");

    backend.clear().await.unwrap();

    let count = 1000;
    let start = Instant::now();

    for i in 0..count {
        let key = format!("key{}", i);
        backend.put(key.as_bytes(), b"value").await.unwrap();
    }

    let elapsed = start.elapsed();
    let ops_per_sec = count as f64 / elapsed.as_secs_f64();

    println!(
        "PostgreSQL write throughput: {:.2} ops/sec ({} ops in {:?})",
        ops_per_sec, count, elapsed
    );

    assert!(ops_per_sec > 50.0, "Write throughput too low");

    backend.clear().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn bench_cached_vs_uncached() {
    use std::time::Instant;

    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("bench_cache:")
        .build()
        .expect("Failed to build config");

    let redis = Arc::new(
        RedisStateBackend::new(config)
            .await
            .expect("Failed to connect to Redis"),
    );

    let cached = CachedStateBackend::new(redis.clone(), 1000, None);

    cached.clear().await.unwrap();

    // Prepare data
    let count = 100;
    for i in 0..count {
        let key = format!("key{}", i);
        cached.put(key.as_bytes(), b"value").await.unwrap();
    }

    // Benchmark uncached
    let start = Instant::now();
    for i in 0..count {
        let key = format!("key{}", i);
        redis.get(key.as_bytes()).await.unwrap();
    }
    let uncached_time = start.elapsed();

    // Benchmark cached (warm cache)
    let start = Instant::now();
    for i in 0..count {
        let key = format!("key{}", i);
        cached.get(key.as_bytes()).await.unwrap();
    }
    let cached_time1 = start.elapsed();

    // Benchmark cached again
    let start = Instant::now();
    for i in 0..count {
        let key = format!("key{}", i);
        cached.get(key.as_bytes()).await.unwrap();
    }
    let cached_time2 = start.elapsed();

    println!("Uncached time: {:?}", uncached_time);
    println!("Cached time (first): {:?}", cached_time1);
    println!("Cached time (second): {:?}", cached_time2);

    // Cached should be faster
    assert!(
        cached_time2 < uncached_time,
        "Cached reads should be faster than uncached"
    );

    let stats = cached.cache_stats().await;
    println!("Cache hit rate: {:.2}%", stats.hit_rate * 100.0);

    cached.clear().await.unwrap();
}
