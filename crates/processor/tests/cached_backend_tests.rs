//! Integration tests for the cached state backend

use processor::state::{
    CacheConfig, CachedStateBackend, EvictionPolicy, MemoryStateBackend, StateBackend,
    WriteStrategy,
};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_cached_backend_write_through() {
    let backend = MemoryStateBackend::new(None);
    let config = CacheConfig {
        write_strategy: WriteStrategy::WriteThrough,
        enable_stats: true,
        ..Default::default()
    };

    let cached = CachedStateBackend::new(backend.clone(), None, config)
        .await
        .unwrap();

    // Write data
    cached.put(b"key1", b"value1").await.unwrap();

    // Should be immediately in backend
    let backend_value = backend.get(b"key1").await.unwrap();
    assert_eq!(backend_value, Some(b"value1".to_vec()));

    // Should also be in cache (L1 hit)
    let cached_value = cached.get(b"key1").await.unwrap();
    assert_eq!(cached_value, Some(b"value1".to_vec()));

    let stats = cached.stats().await;
    assert_eq!(stats.l1_hits, 1);
}

#[tokio::test]
async fn test_cached_backend_write_back() {
    let backend = MemoryStateBackend::new(None);
    let config = CacheConfig {
        write_strategy: WriteStrategy::WriteBack,
        enable_stats: true,
        ..Default::default()
    };

    let cached = CachedStateBackend::new(backend.clone(), None, config)
        .await
        .unwrap();

    // Write data
    cached.put(b"key1", b"value1").await.unwrap();

    // Should be in cache immediately
    let cached_value = cached.get(b"key1").await.unwrap();
    assert_eq!(cached_value, Some(b"value1".to_vec()));

    // Wait for async write to complete
    sleep(Duration::from_millis(100)).await;

    // Should now be in backend
    let backend_value = backend.get(b"key1").await.unwrap();
    assert_eq!(backend_value, Some(b"value1".to_vec()));
}

#[tokio::test]
async fn test_cached_backend_l1_hit() {
    let backend = MemoryStateBackend::new(None);
    let config = CacheConfig {
        enable_stats: true,
        ..Default::default()
    };

    let cached = CachedStateBackend::new(backend, None, config)
        .await
        .unwrap();

    cached.put(b"hot_key", b"hot_value").await.unwrap();

    // First get - populates cache
    cached.get(b"hot_key").await.unwrap();

    // Second get - should be L1 hit
    cached.get(b"hot_key").await.unwrap();

    let stats = cached.stats().await;
    assert!(stats.l1_hits >= 1);
    assert!(stats.l1_hit_rate > 0.0);
}

#[tokio::test]
async fn test_cached_backend_l3_fallback() {
    let backend = MemoryStateBackend::new(None);

    // Prepopulate backend before creating cache
    backend.put(b"existing", b"data").await.unwrap();

    let config = CacheConfig {
        enable_stats: true,
        ..Default::default()
    };

    let cached = CachedStateBackend::new(backend, None, config)
        .await
        .unwrap();

    // Get from L3 (cache miss)
    let value = cached.get(b"existing").await.unwrap();
    assert_eq!(value, Some(b"data".to_vec()));

    let stats = cached.stats().await;
    assert_eq!(stats.l3_hits, 1);

    // Second get should be L1 hit (read-through populated cache)
    let value = cached.get(b"existing").await.unwrap();
    assert_eq!(value, Some(b"data".to_vec()));

    let stats = cached.stats().await;
    assert!(stats.l1_hits >= 1);
}

#[tokio::test]
async fn test_cached_backend_invalidation() {
    let backend = MemoryStateBackend::new(None);
    let config = CacheConfig::default();

    let cached = CachedStateBackend::new(backend, None, config)
        .await
        .unwrap();

    cached.put(b"key1", b"value1").await.unwrap();

    // Verify it's cached
    let value = cached.get(b"key1").await.unwrap();
    assert_eq!(value, Some(b"value1".to_vec()));

    // Invalidate
    cached.invalidate(b"key1").await.unwrap();

    // Should still exist in backend
    let value = cached.get(b"key1").await.unwrap();
    assert_eq!(value, Some(b"value1".to_vec()));
}

#[tokio::test]
async fn test_cached_backend_delete() {
    let backend = MemoryStateBackend::new(None);
    let config = CacheConfig::default();

    let cached = CachedStateBackend::new(backend, None, config)
        .await
        .unwrap();

    cached.put(b"key1", b"value1").await.unwrap();

    // Delete
    cached.delete(b"key1").await.unwrap();

    // Should be gone
    let value = cached.get(b"key1").await.unwrap();
    assert_eq!(value, None);
}

#[tokio::test]
async fn test_cached_backend_cache_warming() {
    let backend = MemoryStateBackend::new(None);

    // Prepopulate backend
    backend.put(b"hot1", b"data1").await.unwrap();
    backend.put(b"hot2", b"data2").await.unwrap();
    backend.put(b"hot3", b"data3").await.unwrap();
    backend.put(b"cold", b"cold_data").await.unwrap();

    let config = CacheConfig {
        enable_stats: true,
        ..Default::default()
    };

    let cached = CachedStateBackend::new(backend, None, config)
        .await
        .unwrap();

    // Warm cache with hot keys
    let hot_keys = vec![b"hot1".to_vec(), b"hot2".to_vec(), b"hot3".to_vec()];
    cached.warm_cache(&hot_keys).await.unwrap();

    // All hot key accesses should be L1 hits
    cached.get(b"hot1").await.unwrap();
    cached.get(b"hot2").await.unwrap();
    cached.get(b"hot3").await.unwrap();

    let stats = cached.stats().await;
    assert_eq!(stats.l1_hits, 3);

    // Cold key should be L3 hit
    cached.get(b"cold").await.unwrap();
    let stats = cached.stats().await;
    assert_eq!(stats.l3_hits, 1);
}

#[tokio::test]
async fn test_cached_backend_batch_operations() {
    let backend = MemoryStateBackend::new(None);
    let config = CacheConfig::default();

    let cached = CachedStateBackend::new(backend, None, config)
        .await
        .unwrap();

    // Batch put
    let pairs = vec![
        (b"k1".to_vec(), b"v1".to_vec()),
        (b"k2".to_vec(), b"v2".to_vec()),
        (b"k3".to_vec(), b"v3".to_vec()),
        (b"k4".to_vec(), b"v4".to_vec()),
        (b"k5".to_vec(), b"v5".to_vec()),
    ];

    cached.put_batch(&pairs).await.unwrap();

    // Batch get
    let keys = vec![
        b"k1".to_vec(),
        b"k2".to_vec(),
        b"k3".to_vec(),
        b"k4".to_vec(),
        b"k5".to_vec(),
    ];

    let values = cached.get_batch(&keys).await.unwrap();

    assert_eq!(values.len(), 5);
    for i in 0..5 {
        assert_eq!(values[i], Some(format!("v{}", i + 1).into_bytes()));
    }
}

#[tokio::test]
async fn test_cached_backend_list_keys() {
    let backend = MemoryStateBackend::new(None);
    let config = CacheConfig::default();

    let cached = CachedStateBackend::new(backend, None, config)
        .await
        .unwrap();

    // Insert with different prefixes
    cached.put(b"user:1", b"alice").await.unwrap();
    cached.put(b"user:2", b"bob").await.unwrap();
    cached.put(b"session:1", b"sess1").await.unwrap();
    cached.put(b"session:2", b"sess2").await.unwrap();

    // List user keys
    let user_keys = cached.list_keys(b"user:").await.unwrap();
    assert_eq!(user_keys.len(), 2);

    // List session keys
    let session_keys = cached.list_keys(b"session:").await.unwrap();
    assert_eq!(session_keys.len(), 2);

    // List all keys
    let all_keys = cached.list_keys(b"").await.unwrap();
    assert_eq!(all_keys.len(), 4);
}

#[tokio::test]
async fn test_cached_backend_clear_cache() {
    let backend = MemoryStateBackend::new(None);
    let config = CacheConfig {
        enable_stats: true,
        ..Default::default()
    };

    let cached = CachedStateBackend::new(backend, None, config)
        .await
        .unwrap();

    // Add data
    cached.put(b"k1", b"v1").await.unwrap();
    cached.put(b"k2", b"v2").await.unwrap();

    // Verify L1 hits
    cached.get(b"k1").await.unwrap();
    cached.get(b"k2").await.unwrap();

    let stats = cached.stats().await;
    assert_eq!(stats.l1_hits, 2);

    // Clear cache
    cached.clear_cache().await.unwrap();

    // Next gets should be L3 hits
    cached.get(b"k1").await.unwrap();
    cached.get(b"k2").await.unwrap();

    let stats = cached.stats().await;
    assert_eq!(stats.l3_hits, 2);
}

#[tokio::test]
async fn test_cached_backend_clear_all() {
    let backend = MemoryStateBackend::new(None);
    let config = CacheConfig::default();

    let cached = CachedStateBackend::new(backend.clone(), None, config)
        .await
        .unwrap();

    cached.put(b"k1", b"v1").await.unwrap();
    cached.put(b"k2", b"v2").await.unwrap();

    assert_eq!(cached.count().await.unwrap(), 2);

    // Clear everything
    cached.clear().await.unwrap();

    assert_eq!(cached.count().await.unwrap(), 0);
    assert_eq!(backend.count().await.unwrap(), 0);
}

#[tokio::test]
async fn test_cached_backend_contains() {
    let backend = MemoryStateBackend::new(None);
    let config = CacheConfig::default();

    let cached = CachedStateBackend::new(backend, None, config)
        .await
        .unwrap();

    assert!(!cached.contains(b"nonexistent").await.unwrap());

    cached.put(b"exists", b"yes").await.unwrap();

    assert!(cached.contains(b"exists").await.unwrap());
}

#[tokio::test]
async fn test_cache_stats_calculation() {
    let backend = MemoryStateBackend::new(None);

    // Prepopulate backend
    for i in 0..100 {
        let key = format!("key{}", i);
        let value = format!("value{}", i);
        backend.put(key.as_bytes(), value.as_bytes()).await.unwrap();
    }

    let config = CacheConfig {
        l1_capacity: 50, // Only cache 50 items
        enable_stats: true,
        ..Default::default()
    };

    let cached = CachedStateBackend::new(backend, None, config)
        .await
        .unwrap();

    // Access first 50 keys twice (should be L1 hits on second access)
    for i in 0..50 {
        let key = format!("key{}", i);
        cached.get(key.as_bytes()).await.unwrap();
        cached.get(key.as_bytes()).await.unwrap();
    }

    // Access next 50 keys once (should be L3 hits)
    for i in 50..100 {
        let key = format!("key{}", i);
        cached.get(key.as_bytes()).await.unwrap();
    }

    let stats = cached.stats().await;

    // Should have 50 L1 hits (second access of first 50 keys)
    assert_eq!(stats.l1_hits, 50);

    // Should have 100 L3 hits (first access of all keys)
    assert_eq!(stats.l3_hits, 100);

    // L1 hit rate should be 50/(50+100) = 0.333...
    assert!((stats.l1_hit_rate - 0.333).abs() < 0.01);

    // Overall hit rate should be (50+100)/(50+100+0) = 1.0
    assert_eq!(stats.overall_hit_rate, 1.0);
}

#[tokio::test]
async fn test_eviction_policy_configuration() {
    let backend = MemoryStateBackend::new(None);

    let lru_config = CacheConfig {
        eviction_policy: EvictionPolicy::LRU,
        ..Default::default()
    };

    let cached_lru = CachedStateBackend::new(backend.clone(), None, lru_config)
        .await
        .unwrap();

    cached_lru.put(b"test", b"value").await.unwrap();
    assert!(cached_lru.contains(b"test").await.unwrap());

    let lfu_config = CacheConfig {
        eviction_policy: EvictionPolicy::LFU,
        ..Default::default()
    };

    let backend2 = MemoryStateBackend::new(None);
    let cached_lfu = CachedStateBackend::new(backend2, None, lfu_config)
        .await
        .unwrap();

    cached_lfu.put(b"test", b"value").await.unwrap();
    assert!(cached_lfu.contains(b"test").await.unwrap());
}

#[tokio::test]
async fn test_concurrent_access() {
    use tokio::task::JoinSet;

    let backend = MemoryStateBackend::new(None);
    let config = CacheConfig {
        enable_stats: true,
        ..Default::default()
    };

    let cached = CachedStateBackend::new(backend, None, config)
        .await
        .unwrap();

    let cached = std::sync::Arc::new(cached);

    // Spawn multiple tasks doing concurrent reads and writes
    let mut set = JoinSet::new();

    for i in 0..10 {
        let cached_clone = cached.clone();
        set.spawn(async move {
            for j in 0..100 {
                let key = format!("key{}_{}", i, j);
                let value = format!("value{}_{}", i, j);

                cached_clone
                    .put(key.as_bytes(), value.as_bytes())
                    .await
                    .unwrap();

                let retrieved = cached_clone.get(key.as_bytes()).await.unwrap();
                assert_eq!(retrieved, Some(value.as_bytes().to_vec()));
            }
        });
    }

    while let Some(result) = set.join_next().await {
        result.unwrap();
    }

    // Verify final count
    assert_eq!(cached.count().await.unwrap(), 1000);

    let stats = cached.stats().await;
    println!("Concurrent access stats: {:#?}", stats);
    assert!(stats.l1_hits > 0);
}

#[tokio::test]
async fn test_large_value_handling() {
    let backend = MemoryStateBackend::new(None);
    let config = CacheConfig::default();

    let cached = CachedStateBackend::new(backend, None, config)
        .await
        .unwrap();

    // 1MB value
    let large_value = vec![0u8; 1024 * 1024];

    cached.put(b"large_key", &large_value).await.unwrap();

    let retrieved = cached.get(b"large_key").await.unwrap();
    assert_eq!(retrieved, Some(large_value));
}
