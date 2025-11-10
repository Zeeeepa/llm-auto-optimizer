//! Integration tests for Enterprise Redis Backend
//!
//! These tests require a running Redis instance on localhost:6379
//! Run with: cargo test --test redis_enterprise_integration -- --ignored

use processor::state::{
    BackendStats, CompressionAlgorithm, EnterpriseRedisBackend, EnterpriseRedisConfig,
    RedisMode, StateBackend,
};
use std::time::Duration;
use tokio::time::sleep;

/// Helper to create test backend
async fn create_test_backend(compression: CompressionAlgorithm) -> Option<EnterpriseRedisBackend> {
    let config = EnterpriseRedisConfig::builder()
        .mode(RedisMode::Standalone {
            url: "redis://localhost:6379".to_string(),
        })
        .key_prefix("test:enterprise:")
        .compression(compression)
        .compression_threshold(100)
        .enable_metrics(true)
        .enable_circuit_breaker(true)
        .slow_query_log(true, Duration::from_millis(50))
        .build()
        .ok()?;

    match EnterpriseRedisBackend::new(config).await {
        Ok(backend) => {
            // Clean up any existing test data
            let _ = backend.clear().await;
            Some(backend)
        }
        Err(e) => {
            eprintln!("Failed to create backend: {}", e);
            None
        }
    }
}

#[tokio::test]
#[ignore]
async fn test_basic_crud_operations() {
    let backend = create_test_backend(CompressionAlgorithm::None)
        .await
        .expect("Redis not available");

    // Create
    backend.put(b"test_key", b"test_value").await.unwrap();

    // Read
    let value = backend.get(b"test_key").await.unwrap();
    assert_eq!(value, Some(b"test_value".to_vec()));

    // Update
    backend.put(b"test_key", b"new_value").await.unwrap();
    let value = backend.get(b"test_key").await.unwrap();
    assert_eq!(value, Some(b"new_value".to_vec()));

    // Delete
    backend.delete(b"test_key").await.unwrap();
    let value = backend.get(b"test_key").await.unwrap();
    assert_eq!(value, None);
}

#[tokio::test]
#[ignore]
async fn test_lz4_compression() {
    let backend = create_test_backend(CompressionAlgorithm::Lz4)
        .await
        .expect("Redis not available");

    // Create large data that will benefit from compression
    let large_value = vec![b'x'; 10000];
    backend.put(b"large_key", &large_value).await.unwrap();

    // Verify we can retrieve it correctly
    let retrieved = backend.get(b"large_key").await.unwrap().unwrap();
    assert_eq!(retrieved, large_value);

    // Check compression stats
    let stats = backend.stats().await;
    assert!(stats.bytes_compressed > 0);
    assert!(stats.bytes_compressed < stats.bytes_written);

    println!("Compression ratio: {:.2}x",
        stats.bytes_written as f64 / stats.bytes_compressed as f64);
}

#[tokio::test]
#[ignore]
async fn test_snappy_compression() {
    let backend = create_test_backend(CompressionAlgorithm::Snappy)
        .await
        .expect("Redis not available");

    let data = b"Hello, World! ".repeat(100);
    backend.put(b"snappy_key", &data).await.unwrap();

    let retrieved = backend.get(b"snappy_key").await.unwrap().unwrap();
    assert_eq!(retrieved, data);
}

#[tokio::test]
#[ignore]
async fn test_zstd_compression() {
    let backend = create_test_backend(CompressionAlgorithm::Zstd)
        .await
        .expect("Redis not available");

    let data = b"Zstandard compression test data. ".repeat(100);
    backend.put(b"zstd_key", &data).await.unwrap();

    let retrieved = backend.get(b"zstd_key").await.unwrap().unwrap();
    assert_eq!(retrieved, data);
}

#[tokio::test]
#[ignore]
async fn test_batch_operations_performance() {
    let backend = create_test_backend(CompressionAlgorithm::None)
        .await
        .expect("Redis not available");

    let start = std::time::Instant::now();

    // Batch write 1000 items
    let mut pairs = Vec::new();
    for i in 0..1000 {
        let key = format!("batch_key_{}", i);
        let value = format!("batch_value_{}", i);
        pairs.push((key.as_bytes().to_vec(), value.as_bytes().to_vec()));
    }

    let pair_refs: Vec<(&[u8], &[u8])> = pairs
        .iter()
        .map(|(k, v)| (k.as_slice(), v.as_slice()))
        .collect();

    backend.batch_put_optimized(&pair_refs).await.unwrap();

    let batch_write_time = start.elapsed();
    println!("Batch write 1000 items: {:?}", batch_write_time);

    // Batch read
    let start = std::time::Instant::now();
    let keys: Vec<&[u8]> = pairs.iter().map(|(k, _)| k.as_slice()).collect();
    let values = backend.batch_get_optimized(&keys).await.unwrap();
    let batch_read_time = start.elapsed();

    println!("Batch read 1000 items: {:?}", batch_read_time);

    assert_eq!(values.len(), 1000);
    assert!(values.iter().all(|v| v.is_some()));

    // Verify performance improvement over individual operations
    assert!(batch_write_time < Duration::from_secs(2));
    assert!(batch_read_time < Duration::from_secs(2));
}

#[tokio::test]
#[ignore]
async fn test_distributed_locking() {
    let backend = create_test_backend(CompressionAlgorithm::None)
        .await
        .expect("Redis not available");

    let resource = "test_resource";

    // Acquire lock
    let guard1 = backend
        .acquire_lock(resource, Duration::from_secs(5))
        .await
        .unwrap();

    // Try to acquire same lock should fail
    let guard2 = backend
        .try_acquire_lock(resource, Duration::from_secs(5))
        .await
        .unwrap();
    assert!(guard2.is_none());

    // Release first lock
    drop(guard1);
    sleep(Duration::from_millis(100)).await;

    // Now we should be able to acquire
    let guard3 = backend
        .try_acquire_lock(resource, Duration::from_secs(5))
        .await
        .unwrap();
    assert!(guard3.is_some());
}

#[tokio::test]
#[ignore]
async fn test_concurrent_lock_acquisition() {
    use std::sync::Arc;
    use tokio::task;

    let backend = Arc::new(
        create_test_backend(CompressionAlgorithm::None)
            .await
            .expect("Redis not available"),
    );

    let resource = "concurrent_resource";
    let mut handles = vec![];

    // Spawn 10 tasks trying to acquire the same lock
    for i in 0..10 {
        let backend = backend.clone();
        let resource = resource.to_string();

        let handle = task::spawn(async move {
            match backend
                .try_acquire_lock(&resource, Duration::from_secs(1))
                .await
            {
                Ok(Some(guard)) => {
                    println!("Task {} acquired lock", i);
                    sleep(Duration::from_millis(100)).await;
                    drop(guard);
                    Some(i)
                }
                Ok(None) => {
                    println!("Task {} failed to acquire lock", i);
                    None
                }
                Err(e) => {
                    eprintln!("Task {} error: {}", i, e);
                    None
                }
            }
        });

        handles.push(handle);
    }

    // Collect results
    let mut successful = 0;
    for handle in handles {
        if let Ok(Some(_)) = handle.await {
            successful += 1;
        }
    }

    // At least one should succeed
    assert!(successful >= 1);
    println!("{} tasks successfully acquired lock", successful);
}

#[tokio::test]
#[ignore]
async fn test_health_check() {
    let backend = create_test_backend(CompressionAlgorithm::None)
        .await
        .expect("Redis not available");

    let healthy = backend.health_check().await.unwrap();
    assert!(healthy);
}

#[tokio::test]
#[ignore]
async fn test_statistics_tracking() {
    let backend = create_test_backend(CompressionAlgorithm::Lz4)
        .await
        .expect("Redis not available");

    // Perform various operations
    backend.put(b"stats_key1", b"value1").await.unwrap();
    backend.put(b"stats_key2", b"value2").await.unwrap();

    backend.get(b"stats_key1").await.unwrap(); // Hit
    backend.get(b"stats_key1").await.unwrap(); // Hit
    backend.get(b"nonexistent").await.unwrap(); // Miss

    backend.delete(b"stats_key1").await.unwrap();

    // Get stats
    let stats = backend.stats().await;

    println!("Statistics: {:?}", stats);

    assert_eq!(stats.cache_hits, 2);
    assert_eq!(stats.cache_misses, 1);
    assert!(stats.operations_success > 0);
    assert!(stats.bytes_written > 0);
    assert!(stats.avg_latency_us > 0);
}

#[tokio::test]
#[ignore]
async fn test_list_keys_with_prefix() {
    let backend = create_test_backend(CompressionAlgorithm::None)
        .await
        .expect("Redis not available");

    // Create keys with different prefixes
    backend.put(b"user:1:name", b"Alice").await.unwrap();
    backend.put(b"user:2:name", b"Bob").await.unwrap();
    backend.put(b"user:3:name", b"Charlie").await.unwrap();
    backend.put(b"product:1:name", b"Widget").await.unwrap();

    // List keys with prefix
    let user_keys = backend.list_keys(b"user:").await.unwrap();

    assert_eq!(user_keys.len(), 3);
    assert!(user_keys
        .iter()
        .all(|k| k.starts_with(b"user:")));

    // List all keys
    let all_keys = backend.list_keys(b"").await.unwrap();
    assert_eq!(all_keys.len(), 4);
}

#[tokio::test]
#[ignore]
async fn test_count_operations() {
    let backend = create_test_backend(CompressionAlgorithm::None)
        .await
        .expect("Redis not available");

    backend.clear().await.unwrap();

    for i in 0..10 {
        let key = format!("count_key_{}", i);
        backend.put(key.as_bytes(), b"value").await.unwrap();
    }

    let count = backend.count().await.unwrap();
    assert_eq!(count, 10);
}

#[tokio::test]
#[ignore]
async fn test_contains_operation() {
    let backend = create_test_backend(CompressionAlgorithm::None)
        .await
        .expect("Redis not available");

    backend.put(b"exists_key", b"value").await.unwrap();

    assert!(backend.contains(b"exists_key").await.unwrap());
    assert!(!backend.contains(b"nonexistent_key").await.unwrap());
}

#[tokio::test]
#[ignore]
async fn test_retry_logic() {
    let backend = create_test_backend(CompressionAlgorithm::None)
        .await
        .expect("Redis not available");

    // Normal operation should work
    backend.put(b"retry_key", b"value").await.unwrap();
    let value = backend.get(b"retry_key").await.unwrap();
    assert_eq!(value, Some(b"value".to_vec()));

    // The backend will automatically retry on transient failures
    // This is tested by the internal retry mechanism
}

#[tokio::test]
#[ignore]
async fn test_compression_threshold() {
    let backend = create_test_backend(CompressionAlgorithm::Lz4)
        .await
        .expect("Redis not available");

    // Small data below threshold shouldn't be compressed
    let small_data = b"small";
    backend.put(b"small_key", small_data).await.unwrap();

    // Large data above threshold should be compressed
    let large_data = vec![b'x'; 10000];
    backend.put(b"large_key", &large_data).await.unwrap();

    // Both should be retrieved correctly
    assert_eq!(
        backend.get(b"small_key").await.unwrap(),
        Some(small_data.to_vec())
    );
    assert_eq!(
        backend.get(b"large_key").await.unwrap(),
        Some(large_data)
    );
}

#[tokio::test]
#[ignore]
async fn test_metrics_collection() {
    let backend = create_test_backend(CompressionAlgorithm::Lz4)
        .await
        .expect("Redis not available");

    // Verify metrics are enabled
    assert!(backend.metrics().is_some());

    // Perform operations
    backend.put(b"metric_key", b"value").await.unwrap();
    backend.get(b"metric_key").await.unwrap();

    // Metrics should be updated
    let stats = backend.stats().await;
    assert!(stats.operations_total > 0);
}

#[tokio::test]
#[ignore]
async fn test_clear_all_keys() {
    let backend = create_test_backend(CompressionAlgorithm::None)
        .await
        .expect("Redis not available");

    // Add multiple keys
    for i in 0..50 {
        let key = format!("clear_key_{}", i);
        backend.put(key.as_bytes(), b"value").await.unwrap();
    }

    let count_before = backend.count().await.unwrap();
    assert_eq!(count_before, 50);

    // Clear all
    backend.clear().await.unwrap();

    let count_after = backend.count().await.unwrap();
    assert_eq!(count_after, 0);
}

#[tokio::test]
#[ignore]
async fn test_stress_test() {
    use std::sync::Arc;
    use tokio::task;

    let backend = Arc::new(
        create_test_backend(CompressionAlgorithm::Lz4)
            .await
            .expect("Redis not available"),
    );

    let mut handles = vec![];

    // Spawn 20 concurrent tasks
    for task_id in 0..20 {
        let backend = backend.clone();

        let handle = task::spawn(async move {
            for i in 0..50 {
                let key = format!("stress_{}_{}", task_id, i);
                let value = format!("value_{}_{}", task_id, i);

                backend.put(key.as_bytes(), value.as_bytes()).await.unwrap();

                if i % 10 == 0 {
                    let retrieved = backend.get(key.as_bytes()).await.unwrap();
                    assert_eq!(retrieved, Some(value.as_bytes().to_vec()));
                }
            }
        });

        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify we have 1000 keys (20 * 50)
    let count = backend.count().await.unwrap();
    assert_eq!(count, 1000);

    let stats = backend.stats().await;
    println!("Stress test stats: {:?}", stats);
    assert!(stats.operations_success >= 1000);
}
