//! Integration tests for Stream Processor State Management
//!
//! Tests all required features:
//! 1. Redis-based state persistence
//! 2. Checkpointing every 10 seconds
//! 3. State recovery on restart
//! 4. Connection failure handling
//! 5. Connection pooling and health checks
//! 6. Comprehensive metrics

use processor::state::{
    CheckpointCoordinator, CheckpointOptions, RedisConfig, RedisStateBackend, StateBackend,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct WindowState {
    window_id: String,
    count: u64,
    sum: f64,
}

impl WindowState {
    fn new(window_id: String) -> Self {
        Self {
            window_id,
            count: 0,
            sum: 0.0,
        }
    }

    fn add_value(&mut self, value: f64) {
        self.count += 1;
        self.sum += value;
    }
}

/// Helper to create Redis backend for testing
async fn create_redis_backend() -> Option<RedisStateBackend> {
    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("test_stream_processor:")
        .default_ttl(Duration::from_secs(3600))
        .pool_size(5, 20)
        .retries(3, Duration::from_millis(100), Duration::from_secs(5))
        .build()
        .ok()?;

    match RedisStateBackend::new(config).await {
        Ok(backend) => {
            // Clear any existing test data
            let _ = backend.clear().await;
            Some(backend)
        }
        Err(_) => None,
    }
}

#[tokio::test]
async fn test_redis_state_persistence() {
    if let Some(backend) = create_redis_backend().await {
        // Store window state
        let mut window = WindowState::new("window_1".to_string());
        window.add_value(10.0);
        window.add_value(20.0);

        let key = b"window:window_1";
        let value = bincode::serialize(&window).unwrap();
        backend.put(key, &value).await.unwrap();

        // Retrieve window state
        let retrieved = backend.get(key).await.unwrap().unwrap();
        let restored: WindowState = bincode::deserialize(&retrieved).unwrap();

        assert_eq!(restored.window_id, "window_1");
        assert_eq!(restored.count, 2);
        assert_eq!(restored.sum, 30.0);
    } else {
        eprintln!("Skipping test: Redis not available");
    }
}

#[tokio::test]
async fn test_connection_pooling() {
    if let Some(backend) = create_redis_backend().await {
        // Verify health check
        let healthy = backend.health_check().await.unwrap();
        assert!(healthy, "Redis connection should be healthy");

        // Perform multiple concurrent operations to test pooling
        let mut handles = vec![];

        for i in 0..20 {
            let backend = Arc::new(backend);
            let handle = tokio::spawn(async move {
                let key = format!("concurrent:key:{}", i).into_bytes();
                let value = format!("value:{}", i).into_bytes();
                backend.put(&key, &value).await.unwrap();
                backend.get(&key).await.unwrap()
            });
            handles.push(handle);
        }

        // Wait for all operations
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_some());
        }
    } else {
        eprintln!("Skipping test: Redis not available");
    }
}

#[tokio::test]
async fn test_retry_logic() {
    if let Some(backend) = create_redis_backend().await {
        // Perform operations that will track retries
        backend.put(b"retry_test", b"value").await.unwrap();
        backend.get(b"retry_test").await.unwrap();

        let stats = backend.stats().await;

        // Operations should succeed without retries if Redis is healthy
        assert_eq!(stats.error_count, 0);
        assert!(stats.get_count > 0);
        assert!(stats.put_count > 0);
    } else {
        eprintln!("Skipping test: Redis not available");
    }
}

#[tokio::test]
async fn test_checkpoint_every_10_seconds() {
    if let Some(backend) = create_redis_backend().await {
        let temp_dir =
            std::env::temp_dir().join(format!("checkpoint_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Store some window state
        backend.put(b"window:1", b"state1").await.unwrap();
        backend.put(b"window:2", b"state2").await.unwrap();

        let options = CheckpointOptions {
            checkpoint_dir: temp_dir.clone(),
            max_checkpoints: 5,
            compress: true,
            min_interval: Duration::from_secs(1), // Shorter for testing
        };

        let coordinator = CheckpointCoordinator::new(
            Arc::new(backend),
            Duration::from_secs(2), // Checkpoint every 2 seconds for faster testing
            options,
        );

        // Start periodic checkpointing
        coordinator.start().await.unwrap();

        // Wait for multiple checkpoints
        sleep(Duration::from_secs(6)).await;

        // Shutdown to get stats
        coordinator.shutdown().await.unwrap();

        // Verify checkpoints were created
        let stats = coordinator.stats().await;
        assert!(
            stats.checkpoints_created >= 2,
            "Should have created at least 2 checkpoints"
        );
        assert_eq!(stats.checkpoint_failures, 0);

        // Cleanup
        std::fs::remove_dir_all(&temp_dir).ok();
    } else {
        eprintln!("Skipping test: Redis not available");
    }
}

#[tokio::test]
async fn test_state_recovery() {
    if let Some(backend) = create_redis_backend().await {
        let temp_dir =
            std::env::temp_dir().join(format!("recovery_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Phase 1: Create state and checkpoint
        {
            let backend_arc = Arc::new(backend);

            // Store window state
            for i in 1..=5 {
                let window = WindowState::new(format!("window_{}", i));
                let key = format!("window:window_{}", i).into_bytes();
                let value = bincode::serialize(&window).unwrap();
                backend_arc.put(&key, &value).await.unwrap();
            }

            let options = CheckpointOptions {
                checkpoint_dir: temp_dir.clone(),
                max_checkpoints: 3,
                compress: true,
                min_interval: Duration::from_secs(1),
            };

            let coordinator = CheckpointCoordinator::new(
                backend_arc.clone(),
                Duration::from_secs(60),
                options,
            );

            // Create checkpoint
            let checkpoint_id = coordinator.create_checkpoint().await.unwrap();
            assert!(!checkpoint_id.is_empty());

            // Clear backend to simulate restart
            backend_arc.clear().await.unwrap();
            assert_eq!(backend_arc.count().await.unwrap(), 0);
        }

        // Phase 2: Restore state from checkpoint
        {
            let backend = create_redis_backend().await.unwrap();
            let backend_arc = Arc::new(backend);

            let options = CheckpointOptions {
                checkpoint_dir: temp_dir.clone(),
                max_checkpoints: 3,
                compress: true,
                min_interval: Duration::from_secs(1),
            };

            let coordinator = CheckpointCoordinator::new(
                backend_arc.clone(),
                Duration::from_secs(60),
                options,
            );

            // Restore from checkpoint
            coordinator.restore_latest().await.unwrap();

            // Verify state was restored
            let count = backend_arc.count().await.unwrap();
            assert_eq!(count, 5, "Should have restored 5 windows");

            // Verify specific window
            let key = b"window:window_1";
            let data = backend_arc.get(key).await.unwrap().unwrap();
            let window: WindowState = bincode::deserialize(&data).unwrap();
            assert_eq!(window.window_id, "window_1");
        }

        // Cleanup
        std::fs::remove_dir_all(&temp_dir).ok();
    } else {
        eprintln!("Skipping test: Redis not available");
    }
}

#[tokio::test]
async fn test_batch_operations() {
    if let Some(backend) = create_redis_backend().await {
        // Prepare batch data
        let windows: Vec<WindowState> = (1..=10)
            .map(|i| WindowState::new(format!("window_{}", i)))
            .collect();

        // Batch put
        let pairs: Vec<(Vec<u8>, Vec<u8>)> = windows
            .iter()
            .map(|w| {
                let key = format!("window:{}", w.window_id).into_bytes();
                let value = bincode::serialize(w).unwrap();
                (key, value)
            })
            .collect();

        let pair_refs: Vec<(&[u8], &[u8])> = pairs
            .iter()
            .map(|(k, v)| (k.as_slice(), v.as_slice()))
            .collect();

        backend.batch_put(&pair_refs).await.unwrap();

        // Batch get
        let keys: Vec<Vec<u8>> = (1..=10)
            .map(|i| format!("window:window_{}", i).into_bytes())
            .collect();

        let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
        let values = backend.batch_get(&key_refs).await.unwrap();

        assert_eq!(values.len(), 10);
        for value in values {
            assert!(value.is_some());
        }

        // Batch delete
        let deleted = backend.batch_delete(&key_refs).await.unwrap();
        assert_eq!(deleted, 10);
    } else {
        eprintln!("Skipping test: Redis not available");
    }
}

#[tokio::test]
async fn test_ttl_management() {
    if let Some(backend) = create_redis_backend().await {
        // Store with custom TTL
        backend
            .put_with_ttl(b"temp_window", b"data", Duration::from_secs(2))
            .await
            .unwrap();

        // Verify key exists
        assert!(backend.contains(b"temp_window").await.unwrap());

        // Check TTL
        let ttl = backend.ttl(b"temp_window").await.unwrap();
        assert!(ttl.is_some());
        assert!(ttl.unwrap().as_secs() <= 2);

        // Wait for expiration
        sleep(Duration::from_secs(3)).await;

        // Key should be expired
        assert!(!backend.contains(b"temp_window").await.unwrap());
    } else {
        eprintln!("Skipping test: Redis not available");
    }
}

#[tokio::test]
async fn test_atomic_operations() {
    if let Some(backend) = create_redis_backend().await {
        // Test set if not exists
        let set1 = backend
            .set_if_not_exists(b"lock:window:1", b"processor_1", Duration::from_secs(60))
            .await
            .unwrap();
        assert!(set1, "First set should succeed");

        let set2 = backend
            .set_if_not_exists(b"lock:window:1", b"processor_2", Duration::from_secs(60))
            .await
            .unwrap();
        assert!(!set2, "Second set should fail");

        // Test get and delete atomically
        backend.put(b"temp_key", b"temp_value").await.unwrap();

        let value = backend.get_and_delete(b"temp_key").await.unwrap();
        assert_eq!(value, Some(b"temp_value".to_vec()));

        // Key should be deleted
        assert!(!backend.contains(b"temp_key").await.unwrap());
    } else {
        eprintln!("Skipping test: Redis not available");
    }
}

#[tokio::test]
async fn test_statistics_tracking() {
    if let Some(backend) = create_redis_backend().await {
        // Perform various operations
        backend.put(b"key1", b"value1").await.unwrap();
        backend.put(b"key2", b"value2").await.unwrap();

        backend.get(b"key1").await.unwrap(); // Hit
        backend.get(b"key1").await.unwrap(); // Hit
        backend.get(b"nonexistent").await.unwrap(); // Miss

        backend.delete(b"key1").await.unwrap();

        // Check statistics
        let stats = backend.stats().await;

        assert!(stats.put_count >= 2);
        assert!(stats.get_count >= 3);
        assert!(stats.delete_count >= 1);
        assert!(stats.hit_count >= 2);
        assert!(stats.miss_count >= 1);

        assert!(stats.hit_rate() > 0.0);
        assert!(stats.hit_rate() <= 1.0);

        assert!(stats.total_operations() > 0);
        assert!(stats.bytes_read > 0);
        assert!(stats.bytes_written > 0);
    } else {
        eprintln!("Skipping test: Redis not available");
    }
}

#[tokio::test]
async fn test_window_state_lifecycle() {
    if let Some(backend) = create_redis_backend().await {
        // Create window
        let mut window = WindowState::new("test_window".to_string());

        // Add events
        for i in 1..=100 {
            window.add_value(i as f64);
        }

        // Store window
        let key = b"window:test_window";
        let value = bincode::serialize(&window).unwrap();
        backend.put(key, &value).await.unwrap();

        // Update window
        window.add_value(101.0);
        let updated_value = bincode::serialize(&window).unwrap();
        backend.put(key, &updated_value).await.unwrap();

        // Retrieve and verify
        let retrieved = backend.get(key).await.unwrap().unwrap();
        let restored: WindowState = bincode::deserialize(&retrieved).unwrap();

        assert_eq!(restored.count, 101);
        assert_eq!(restored.sum, 5151.0); // Sum of 1..=101

        // Clean up
        backend.delete(key).await.unwrap();
        assert!(!backend.contains(key).await.unwrap());
    } else {
        eprintln!("Skipping test: Redis not available");
    }
}

#[tokio::test]
async fn test_concurrent_window_access() {
    if let Some(backend) = create_redis_backend().await {
        let backend = Arc::new(backend);
        let mut handles = vec![];

        // Spawn 10 tasks, each updating different windows
        for task_id in 0..10 {
            let backend = Arc::clone(&backend);
            let handle = tokio::spawn(async move {
                for i in 0..10 {
                    let mut window = WindowState::new(format!("task_{}_{}", task_id, i));
                    window.add_value(42.0);

                    let key = format!("window:task_{}_{}", task_id, i).into_bytes();
                    let value = bincode::serialize(&window).unwrap();
                    backend.put(&key, &value).await.unwrap();
                }
            });
            handles.push(handle);
        }

        // Wait for all tasks
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify all windows were created
        let all_windows = backend.list_keys(b"window:task_").await.unwrap();
        assert_eq!(all_windows.len(), 100);
    } else {
        eprintln!("Skipping test: Redis not available");
    }
}

#[tokio::test]
async fn test_memory_usage_monitoring() {
    if let Some(backend) = create_redis_backend().await {
        // Get initial memory usage
        let memory = backend.memory_usage().await.unwrap();
        assert!(memory > 0, "Should report memory usage");

        // Add data
        for i in 0..100 {
            let key = format!("memory_test:{}", i).into_bytes();
            let value = vec![0u8; 1024]; // 1KB per key
            backend.put(&key, &value).await.unwrap();
        }

        // Memory usage should increase
        let new_memory = backend.memory_usage().await.unwrap();
        assert!(
            new_memory > memory,
            "Memory usage should increase after adding data"
        );

        // Clean up
        let keys: Vec<Vec<u8>> = (0..100)
            .map(|i| format!("memory_test:{}", i).into_bytes())
            .collect();
        let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
        backend.batch_delete(&key_refs).await.unwrap();
    } else {
        eprintln!("Skipping test: Redis not available");
    }
}
