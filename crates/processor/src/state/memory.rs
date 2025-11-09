//! In-memory state backend implementation
//!
//! This module provides a high-performance in-memory state backend using DashMap
//! for concurrent access. It supports optional TTL (time-to-live) for automatic
//! state cleanup and provides checkpoint/restore capabilities.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, trace, warn};

use super::backend::StateBackend;
use crate::error::StateResult;

/// Entry in the memory state backend with TTL support
#[derive(Debug, Clone)]
struct StateEntry {
    /// The actual value stored
    value: Vec<u8>,
    /// When this entry was created
    created_at: DateTime<Utc>,
    /// When this entry was last accessed
    accessed_at: DateTime<Utc>,
    /// When this entry was last modified
    modified_at: DateTime<Utc>,
}

impl StateEntry {
    /// Create a new state entry
    fn new(value: Vec<u8>) -> Self {
        let now = Utc::now();
        Self {
            value,
            created_at: now,
            accessed_at: now,
            modified_at: now,
        }
    }

    /// Check if this entry has expired
    fn is_expired(&self, ttl: Duration) -> bool {
        let age = Utc::now().signed_duration_since(self.modified_at);
        age.num_milliseconds() as u64 > ttl.as_millis() as u64
    }

    /// Update the accessed timestamp
    fn touch(&mut self) {
        self.accessed_at = Utc::now();
    }

    /// Update the value and modified timestamp
    fn update(&mut self, new_value: Vec<u8>) {
        self.value = new_value;
        self.modified_at = Utc::now();
        self.accessed_at = Utc::now();
    }
}

/// Statistics about the memory state backend
#[derive(Debug, Clone, Default)]
pub struct MemoryBackendStats {
    /// Total number of entries
    pub total_entries: usize,
    /// Number of expired entries
    pub expired_entries: usize,
    /// Total bytes stored
    pub total_bytes: usize,
    /// Number of get operations
    pub get_count: u64,
    /// Number of put operations
    pub put_count: u64,
    /// Number of delete operations
    pub delete_count: u64,
    /// Number of cache hits
    pub hit_count: u64,
    /// Number of cache misses
    pub miss_count: u64,
}

/// In-memory state backend using DashMap
///
/// This backend stores all state in memory using a concurrent hash map (DashMap).
/// It's ideal for:
///
/// - **Testing**: Fast, no external dependencies
/// - **Stateless processing**: When state doesn't need to survive restarts
/// - **Small state**: When state fits comfortably in memory
/// - **High throughput**: Fastest possible state access
///
/// ## Features
///
/// - **Thread-safe**: Uses DashMap for lock-free concurrent access
/// - **TTL support**: Automatic expiration of old entries
/// - **Statistics**: Tracks access patterns and cache hit rates
/// - **Checkpoint/Restore**: Can snapshot state to disk
///
/// ## Example
///
/// ```rust,no_run
/// use processor::state::{MemoryStateBackend, StateBackend};
/// use std::time::Duration;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     // Create backend with 1-hour TTL
///     let backend = MemoryStateBackend::new(Some(Duration::from_secs(3600)));
///
///     // Store state
///     backend.put(b"session:123", b"user_data").await?;
///
///     // Retrieve state
///     if let Some(data) = backend.get(b"session:123").await? {
///         println!("Session data: {:?}", data);
///     }
///
///     // Get statistics
///     let stats = backend.stats().await;
///     println!("Hit rate: {:.2}%",
///         100.0 * stats.hit_count as f64 / (stats.hit_count + stats.miss_count) as f64);
///
///     Ok(())
/// }
/// ```
pub struct MemoryStateBackend {
    /// The actual state storage
    data: Arc<DashMap<Vec<u8>, StateEntry>>,
    /// Optional TTL for automatic expiration
    ttl: Option<Duration>,
    /// Statistics tracking
    stats: Arc<RwLock<MemoryBackendStats>>,
}

impl MemoryStateBackend {
    /// Create a new in-memory state backend
    ///
    /// # Arguments
    ///
    /// * `ttl` - Optional time-to-live for entries. If None, entries never expire.
    ///
    /// # Returns
    ///
    /// A new `MemoryStateBackend` instance
    ///
    /// # Example
    ///
    /// ```rust
    /// use processor::state::MemoryStateBackend;
    /// use std::time::Duration;
    ///
    /// // No TTL - entries never expire
    /// let backend1 = MemoryStateBackend::new(None);
    ///
    /// // 1 hour TTL
    /// let backend2 = MemoryStateBackend::new(Some(Duration::from_secs(3600)));
    /// ```
    pub fn new(ttl: Option<Duration>) -> Self {
        Self {
            data: Arc::new(DashMap::new()),
            ttl,
            stats: Arc::new(RwLock::new(MemoryBackendStats::default())),
        }
    }

    /// Get current statistics
    ///
    /// # Returns
    ///
    /// A snapshot of current backend statistics
    pub async fn stats(&self) -> MemoryBackendStats {
        self.stats.read().await.clone()
    }

    /// Clean up expired entries
    ///
    /// This method scans all entries and removes those that have exceeded the TTL.
    /// Returns the number of entries removed.
    ///
    /// # Returns
    ///
    /// Number of entries removed
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use processor::state::MemoryStateBackend;
    /// # use std::time::Duration;
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// let backend = MemoryStateBackend::new(Some(Duration::from_secs(60)));
    ///
    /// // Periodically clean up expired entries
    /// let removed = backend.cleanup_expired().await;
    /// println!("Removed {} expired entries", removed);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn cleanup_expired(&self) -> usize {
        let ttl = match self.ttl {
            Some(ttl) => ttl,
            None => return 0, // No TTL, nothing to clean up
        };

        let mut removed = 0;
        let keys_to_remove: Vec<Vec<u8>> = self
            .data
            .iter()
            .filter_map(|entry| {
                if entry.value().is_expired(ttl) {
                    Some(entry.key().clone())
                } else {
                    None
                }
            })
            .collect();

        for key in keys_to_remove {
            self.data.remove(&key);
            removed += 1;
        }

        if removed > 0 {
            debug!("Cleaned up {} expired entries", removed);
            let mut stats = self.stats.write().await;
            stats.expired_entries += removed;
        }

        removed
    }

    /// Start automatic cleanup background task
    ///
    /// Spawns a background task that periodically cleans up expired entries.
    ///
    /// # Arguments
    ///
    /// * `interval` - How often to run cleanup
    ///
    /// # Returns
    ///
    /// A handle that cancels the cleanup task when dropped
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use processor::state::MemoryStateBackend;
    /// # use std::time::Duration;
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// let backend = MemoryStateBackend::new(Some(Duration::from_secs(3600)));
    ///
    /// // Clean up every 5 minutes
    /// let _cleanup_handle = backend.start_cleanup_task(Duration::from_secs(300));
    ///
    /// // Cleanup runs in background...
    /// # Ok(())
    /// # }
    /// ```
    pub fn start_cleanup_task(&self, interval: Duration) -> tokio::task::JoinHandle<()> {
        let backend = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);
            loop {
                interval.tick().await;
                let removed = backend.cleanup_expired().await;
                if removed > 0 {
                    debug!("Background cleanup removed {} entries", removed);
                }
            }
        })
    }

    /// Create a checkpoint of current state
    ///
    /// Returns all key-value pairs as a vector for serialization.
    ///
    /// # Returns
    ///
    /// Vector of (key, value) pairs
    pub async fn checkpoint(&self) -> Vec<(Vec<u8>, Vec<u8>)> {
        self.data
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().value.clone()))
            .collect()
    }

    /// Restore state from a checkpoint
    ///
    /// Replaces all current state with the provided checkpoint data.
    ///
    /// # Arguments
    ///
    /// * `checkpoint` - Vector of (key, value) pairs to restore
    pub async fn restore(&self, checkpoint: Vec<(Vec<u8>, Vec<u8>)>) -> StateResult<()> {
        self.data.clear();

        for (key, value) in checkpoint {
            self.data.insert(key, StateEntry::new(value));
        }

        let mut stats = self.stats.write().await;
        stats.total_entries = self.data.len();

        Ok(())
    }

    /// Get total memory usage in bytes (approximate)
    ///
    /// # Returns
    ///
    /// Approximate memory usage in bytes
    pub async fn memory_usage(&self) -> usize {
        self.data
            .iter()
            .map(|entry| {
                entry.key().len() + entry.value().value.len() + std::mem::size_of::<StateEntry>()
            })
            .sum()
    }
}

// Clone implementation for use in background tasks
impl Clone for MemoryStateBackend {
    fn clone(&self) -> Self {
        Self {
            data: Arc::clone(&self.data),
            ttl: self.ttl,
            stats: Arc::clone(&self.stats),
        }
    }
}

#[async_trait]
impl StateBackend for MemoryStateBackend {
    async fn get(&self, key: &[u8]) -> StateResult<Option<Vec<u8>>> {
        trace!("Getting key: {:?}", key);

        let mut stats = self.stats.write().await;
        stats.get_count += 1;

        // Check if entry exists
        let result = if let Some(mut entry) = self.data.get_mut(key) {
            // Check TTL
            if let Some(ttl) = self.ttl {
                if entry.is_expired(ttl) {
                    warn!("Key expired: {:?}", key);
                    drop(entry); // Release the lock before removing
                    self.data.remove(key);
                    stats.miss_count += 1;
                    stats.expired_entries += 1;
                    return Ok(None);
                }
            }

            // Update access time
            entry.touch();
            stats.hit_count += 1;
            Some(entry.value.clone())
        } else {
            stats.miss_count += 1;
            None
        };

        Ok(result)
    }

    async fn put(&self, key: &[u8], value: &[u8]) -> StateResult<()> {
        trace!("Putting key: {:?}, value size: {} bytes", key, value.len());

        let mut stats = self.stats.write().await;
        stats.put_count += 1;

        if let Some(mut entry) = self.data.get_mut(key) {
            entry.update(value.to_vec());
        } else {
            self.data.insert(key.to_vec(), StateEntry::new(value.to_vec()));
            stats.total_entries += 1;
        }

        Ok(())
    }

    async fn delete(&self, key: &[u8]) -> StateResult<()> {
        trace!("Deleting key: {:?}", key);

        let mut stats = self.stats.write().await;
        stats.delete_count += 1;

        if self.data.remove(key).is_some() {
            stats.total_entries = stats.total_entries.saturating_sub(1);
        }

        Ok(())
    }

    async fn list_keys(&self, prefix: &[u8]) -> StateResult<Vec<Vec<u8>>> {
        trace!("Listing keys with prefix: {:?}", prefix);

        let keys: Vec<Vec<u8>> = if prefix.is_empty() {
            self.data.iter().map(|entry| entry.key().clone()).collect()
        } else {
            self.data
                .iter()
                .filter(|entry| entry.key().starts_with(prefix))
                .map(|entry| entry.key().clone())
                .collect()
        };

        Ok(keys)
    }

    async fn clear(&self) -> StateResult<()> {
        debug!("Clearing all state");
        self.data.clear();

        let mut stats = self.stats.write().await;
        stats.total_entries = 0;

        Ok(())
    }

    async fn count(&self) -> StateResult<usize> {
        Ok(self.data.len())
    }

    async fn contains(&self, key: &[u8]) -> StateResult<bool> {
        Ok(self.data.contains_key(key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::backend::tests::*;

    #[tokio::test]
    async fn test_memory_backend_basic() {
        let backend = MemoryStateBackend::new(None);
        test_backend_basic_ops(backend).await;
    }

    #[tokio::test]
    async fn test_memory_backend_list_keys() {
        let backend = MemoryStateBackend::new(None);
        test_backend_list_keys(backend).await;
    }

    #[tokio::test]
    async fn test_memory_backend_clear() {
        let backend = MemoryStateBackend::new(None);
        test_backend_clear(backend).await;
    }

    #[tokio::test]
    async fn test_memory_backend_contains() {
        let backend = MemoryStateBackend::new(None);
        test_backend_contains(backend).await;
    }

    #[tokio::test]
    async fn test_ttl_expiration() {
        let backend = MemoryStateBackend::new(Some(Duration::from_millis(100)));

        backend.put(b"key1", b"value1").await.unwrap();
        assert!(backend.get(b"key1").await.unwrap().is_some());

        // Wait for TTL to expire
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Entry should be expired
        assert!(backend.get(b"key1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let backend = MemoryStateBackend::new(Some(Duration::from_millis(100)));

        backend.put(b"key1", b"value1").await.unwrap();
        backend.put(b"key2", b"value2").await.unwrap();
        backend.put(b"key3", b"value3").await.unwrap();

        assert_eq!(backend.count().await.unwrap(), 3);

        // Wait for entries to expire
        tokio::time::sleep(Duration::from_millis(150)).await;

        let removed = backend.cleanup_expired().await;
        assert_eq!(removed, 3);
        assert_eq!(backend.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_checkpoint_restore() {
        let backend = MemoryStateBackend::new(None);

        backend.put(b"key1", b"value1").await.unwrap();
        backend.put(b"key2", b"value2").await.unwrap();

        // Create checkpoint
        let checkpoint = backend.checkpoint().await;
        assert_eq!(checkpoint.len(), 2);

        // Clear and restore
        backend.clear().await.unwrap();
        assert_eq!(backend.count().await.unwrap(), 0);

        backend.restore(checkpoint).await.unwrap();
        assert_eq!(backend.count().await.unwrap(), 2);

        assert_eq!(
            backend.get(b"key1").await.unwrap(),
            Some(b"value1".to_vec())
        );
    }

    #[tokio::test]
    async fn test_stats() {
        let backend = MemoryStateBackend::new(None);

        backend.put(b"key1", b"value1").await.unwrap();
        backend.put(b"key2", b"value2").await.unwrap();

        backend.get(b"key1").await.unwrap(); // Hit
        backend.get(b"key1").await.unwrap(); // Hit
        backend.get(b"nonexistent").await.unwrap(); // Miss

        backend.delete(b"key1").await.unwrap();

        let stats = backend.stats().await;
        assert_eq!(stats.put_count, 2);
        assert_eq!(stats.get_count, 3);
        assert_eq!(stats.delete_count, 1);
        assert_eq!(stats.hit_count, 2);
        assert_eq!(stats.miss_count, 1);
    }

    #[tokio::test]
    async fn test_memory_usage() {
        let backend = MemoryStateBackend::new(None);

        let initial_usage = backend.memory_usage().await;
        assert_eq!(initial_usage, 0);

        backend.put(b"key1", b"value1").await.unwrap();
        let usage_after_put = backend.memory_usage().await;
        assert!(usage_after_put > initial_usage);

        backend.delete(b"key1").await.unwrap();
        let usage_after_delete = backend.memory_usage().await;
        assert_eq!(usage_after_delete, 0);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        use std::sync::Arc;

        let backend = Arc::new(MemoryStateBackend::new(None));
        let mut handles = vec![];

        // Spawn multiple tasks writing concurrently
        for i in 0..10 {
            let backend = Arc::clone(&backend);
            let handle = tokio::spawn(async move {
                for j in 0..10 {
                    let key = format!("key:{}:{}", i, j).into_bytes();
                    let value = format!("value:{}:{}", i, j).into_bytes();
                    backend.put(&key, &value).await.unwrap();
                }
            });
            handles.push(handle);
        }

        // Wait for all tasks
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify all entries were written
        assert_eq!(backend.count().await.unwrap(), 100);
    }
}
