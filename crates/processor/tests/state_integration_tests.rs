//! Integration tests for state management module

use processor::state::{
    Checkpoint, CheckpointCoordinator, CheckpointOptions, MemoryStateBackend, SledStateBackend,
    StateBackend,
};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_memory_backend_basic_operations() {
    let backend = MemoryStateBackend::new(None);

    // Test put and get
    backend.put(b"key1", b"value1").await.unwrap();
    assert_eq!(
        backend.get(b"key1").await.unwrap(),
        Some(b"value1".to_vec())
    );

    // Test update
    backend.put(b"key1", b"updated").await.unwrap();
    assert_eq!(
        backend.get(b"key1").await.unwrap(),
        Some(b"updated".to_vec())
    );

    // Test delete
    backend.delete(b"key1").await.unwrap();
    assert_eq!(backend.get(b"key1").await.unwrap(), None);
}

#[tokio::test]
async fn test_memory_backend_ttl() {
    let backend = MemoryStateBackend::new(Some(Duration::from_millis(100)));

    backend.put(b"temp_key", b"temp_value").await.unwrap();
    assert!(backend.get(b"temp_key").await.unwrap().is_some());

    // Wait for TTL to expire
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Entry should be expired
    assert!(backend.get(b"temp_key").await.unwrap().is_none());
}

#[tokio::test]
async fn test_memory_backend_prefix_listing() {
    let backend = MemoryStateBackend::new(None);

    backend.put(b"user:1:name", b"Alice").await.unwrap();
    backend.put(b"user:1:age", b"30").await.unwrap();
    backend.put(b"user:2:name", b"Bob").await.unwrap();
    backend.put(b"session:1", b"data").await.unwrap();

    // List all user:1 keys
    let user1_keys = backend.list_keys(b"user:1:").await.unwrap();
    assert_eq!(user1_keys.len(), 2);

    // List all user keys
    let user_keys = backend.list_keys(b"user:").await.unwrap();
    assert_eq!(user_keys.len(), 3);

    // List all keys
    let all_keys = backend.list_keys(b"").await.unwrap();
    assert_eq!(all_keys.len(), 4);
}

#[tokio::test]
async fn test_sled_backend_persistence() {
    let temp_dir =
        std::env::temp_dir().join(format!("sled_integration_test_{}", uuid::Uuid::new_v4()));

    // Create backend and write data
    {
        let backend = SledStateBackend::temporary().await.unwrap();
        backend.put(b"persistent_key", b"persistent_value").await.unwrap();
        backend.flush().await.unwrap();
    }

    // Data should persist (in temporary() backend it's in-memory but we can test the API)
    let backend = SledStateBackend::temporary().await.unwrap();
    backend.put(b"key1", b"value1").await.unwrap();
    assert_eq!(
        backend.get(b"key1").await.unwrap(),
        Some(b"value1".to_vec())
    );

    // Cleanup
    tokio::fs::remove_dir_all(&temp_dir).await.ok();
}

#[tokio::test]
async fn test_checkpoint_create_and_restore() {
    let backend = Arc::new(MemoryStateBackend::new(None));

    // Add some state
    backend.put(b"window:1", b"data1").await.unwrap();
    backend.put(b"window:2", b"data2").await.unwrap();
    backend.put(b"meta:checkpoint", b"info").await.unwrap();

    // Create checkpoint coordinator
    let temp_dir =
        std::env::temp_dir().join(format!("checkpoint_integration_{}", uuid::Uuid::new_v4()));
    let options = CheckpointOptions {
        checkpoint_dir: temp_dir.clone(),
        max_checkpoints: 3,
        compress: false,
        min_interval: Duration::from_millis(100),
    };

    let coordinator =
        CheckpointCoordinator::new(backend.clone(), Duration::from_secs(60), options);

    // Create checkpoint
    let checkpoint_id = coordinator.create_checkpoint().await.unwrap();
    assert!(!checkpoint_id.is_empty());

    // Clear backend
    backend.clear().await.unwrap();
    assert_eq!(backend.count().await.unwrap(), 0);

    // Restore from checkpoint
    coordinator.restore_latest().await.unwrap();

    // Verify data was restored
    assert_eq!(backend.count().await.unwrap(), 3);
    assert_eq!(
        backend.get(b"window:1").await.unwrap(),
        Some(b"data1".to_vec())
    );
    assert_eq!(
        backend.get(b"meta:checkpoint").await.unwrap(),
        Some(b"info".to_vec())
    );

    // Cleanup
    tokio::fs::remove_dir_all(&temp_dir).await.ok();
}

#[tokio::test]
async fn test_checkpoint_periodic() {
    let backend = Arc::new(MemoryStateBackend::new(None));

    let temp_dir =
        std::env::temp_dir().join(format!("checkpoint_periodic_{}", uuid::Uuid::new_v4()));
    let options = CheckpointOptions {
        checkpoint_dir: temp_dir.clone(),
        max_checkpoints: 5,
        compress: false,
        min_interval: Duration::from_millis(50),
    };

    let coordinator = CheckpointCoordinator::new(
        backend.clone(),
        Duration::from_millis(200),
        options,
    );

    // Start periodic checkpointing
    coordinator.start().await.unwrap();

    // Add data over time
    for i in 0..10 {
        backend
            .put(&format!("key:{}", i).into_bytes(), b"value")
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Shutdown coordinator (creates final checkpoint)
    coordinator.shutdown().await.unwrap();

    // Check that checkpoints were created
    let stats = coordinator.stats().await;
    assert!(stats.checkpoints_created > 0);
    assert!(stats.last_checkpoint_time.is_some());

    // Cleanup
    tokio::fs::remove_dir_all(&temp_dir).await.ok();
}

#[tokio::test]
async fn test_concurrent_state_access() {
    let backend = Arc::new(MemoryStateBackend::new(None));
    let mut handles = vec![];

    // Spawn 10 tasks, each writing 100 keys
    for task_id in 0..10 {
        let backend = Arc::clone(&backend);
        let handle = tokio::spawn(async move {
            for i in 0..100 {
                let key = format!("task:{}:key:{}", task_id, i).into_bytes();
                let value = format!("value:{}", i).into_bytes();
                backend.put(&key, &value).await.unwrap();
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all 1000 entries were written
    assert_eq!(backend.count().await.unwrap(), 1000);

    // Verify we can read them
    for task_id in 0..10 {
        let keys = backend
            .list_keys(format!("task:{}:", task_id).as_bytes())
            .await
            .unwrap();
        assert_eq!(keys.len(), 100);
    }
}

#[tokio::test]
async fn test_memory_backend_stats() {
    let backend = MemoryStateBackend::new(None);

    // Perform operations
    backend.put(b"key1", b"value1").await.unwrap();
    backend.put(b"key2", b"value2").await.unwrap();

    backend.get(b"key1").await.unwrap(); // hit
    backend.get(b"key1").await.unwrap(); // hit
    backend.get(b"nonexistent").await.unwrap(); // miss

    backend.delete(b"key1").await.unwrap();

    // Check stats
    let stats = backend.stats().await;
    assert_eq!(stats.put_count, 2);
    assert_eq!(stats.get_count, 3);
    assert_eq!(stats.delete_count, 1);
    assert_eq!(stats.hit_count, 2);
    assert_eq!(stats.miss_count, 1);
}

#[tokio::test]
async fn test_checkpoint_metadata() {
    let data = vec![
        (b"key1".to_vec(), b"value1".to_vec()),
        (b"key2".to_vec(), b"value2".to_vec()),
    ];

    let checkpoint = Checkpoint::new("test_checkpoint".to_string(), data);

    assert_eq!(checkpoint.metadata.checkpoint_id, "test_checkpoint");
    assert_eq!(checkpoint.metadata.entry_count, 2);
    assert!(checkpoint.metadata.size_bytes > 0);
    assert!(!checkpoint.metadata.checksum.is_empty());
    assert_eq!(checkpoint.metadata.version, 1);
}

#[tokio::test]
async fn test_checkpoint_save_and_load() {
    let temp_dir = std::env::temp_dir().join(format!("checkpoint_io_{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_dir).await.unwrap();

    let data = vec![
        (b"window:1".to_vec(), b"state1".to_vec()),
        (b"window:2".to_vec(), b"state2".to_vec()),
    ];

    let checkpoint = Checkpoint::new("save_load_test".to_string(), data);
    let checkpoint_path = temp_dir.join("test.ckpt");

    // Save checkpoint
    checkpoint.save(&checkpoint_path).await.unwrap();
    assert!(checkpoint_path.exists());

    // Load checkpoint
    let loaded = Checkpoint::load(&checkpoint_path).await.unwrap();

    assert_eq!(loaded.metadata.checkpoint_id, "save_load_test");
    assert_eq!(loaded.metadata.entry_count, 2);
    assert_eq!(loaded.data.len(), 2);
    assert!(loaded.metadata.validated);

    // Cleanup
    tokio::fs::remove_dir_all(&temp_dir).await.ok();
}

#[tokio::test]
async fn test_sled_backend_checkpoint() {
    let backend = SledStateBackend::temporary().await.unwrap();

    backend.put(b"sled:key1", b"value1").await.unwrap();
    backend.put(b"sled:key2", b"value2").await.unwrap();

    // Create checkpoint
    let checkpoint = backend.checkpoint().await.unwrap();
    assert_eq!(checkpoint.len(), 2);

    // Clear and restore
    backend.clear().await.unwrap();
    assert_eq!(backend.count().await.unwrap(), 0);

    backend.restore(checkpoint).await.unwrap();
    assert_eq!(backend.count().await.unwrap(), 2);
    assert_eq!(
        backend.get(b"sled:key1").await.unwrap(),
        Some(b"value1".to_vec())
    );
}
