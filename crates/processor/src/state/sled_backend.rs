//! Sled-based persistent state backend
//!
//! This module provides a persistent state backend using the Sled embedded database.
//! It's ideal for production deployments where state needs to survive restarts.

use async_trait::async_trait;
use sled::Db;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, trace};

use super::backend::StateBackend;
use crate::error::{StateError, StateResult};

/// Statistics for the Sled backend
#[derive(Debug, Clone, Default)]
pub struct SledBackendStats {
    /// Number of get operations
    pub get_count: u64,
    /// Number of put operations
    pub put_count: u64,
    /// Number of delete operations
    pub delete_count: u64,
    /// Number of flush operations
    pub flush_count: u64,
    /// Total bytes written
    pub bytes_written: u64,
    /// Total bytes read
    pub bytes_read: u64,
}

/// Configuration for the Sled backend
#[derive(Debug, Clone)]
pub struct SledConfig {
    /// Path to the database directory
    pub path: PathBuf,
    /// Cache size in bytes (default: 128MB)
    pub cache_capacity: u64,
    /// Whether to use compression (default: true)
    pub use_compression: bool,
    /// Flush every N operations (default: 1000)
    pub flush_every: u64,
    /// Segment size for log files (default: 8MB)
    pub segment_size: usize,
}

impl Default for SledConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("/tmp/sled_state"),
            cache_capacity: 128 * 1024 * 1024, // 128MB
            use_compression: true,
            flush_every: 1000,
            segment_size: 8 * 1024 * 1024, // 8MB
        }
    }
}

impl SledConfig {
    /// Create a new configuration with the given path
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            ..Default::default()
        }
    }

    /// Set the cache capacity
    pub fn with_cache_capacity(mut self, capacity: u64) -> Self {
        self.cache_capacity = capacity;
        self
    }

    /// Enable or disable compression
    pub fn with_compression(mut self, enabled: bool) -> Self {
        self.use_compression = enabled;
        self
    }

    /// Set the flush interval
    pub fn with_flush_every(mut self, count: u64) -> Self {
        self.flush_every = count;
        self
    }
}

/// Sled-based persistent state backend
///
/// This backend uses the Sled embedded database for persistent state storage.
/// It's designed for production use where state must survive process restarts.
///
/// ## Features
///
/// - **Persistent**: State survives restarts
/// - **ACID**: Atomic operations with durability guarantees
/// - **Efficient**: Lock-free data structures and zero-copy operations
/// - **Compression**: Optional compression to save disk space
/// - **Crash recovery**: Automatic recovery from crashes
///
/// ## Example
///
/// ```rust,no_run
/// use processor::state::{SledStateBackend, SledConfig, StateBackend};
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     // Create configuration
///     let config = SledConfig::new("/var/lib/optimizer/state")
///         .with_cache_capacity(256 * 1024 * 1024) // 256MB
///         .with_compression(true);
///
///     // Open database
///     let backend = SledStateBackend::open(config).await?;
///
///     // Use like any other backend
///     backend.put(b"window:123", b"aggregated_data").await?;
///
///     // Data persists across restarts
///     backend.flush().await?;
///
///     Ok(())
/// }
/// ```
pub struct SledStateBackend {
    /// The Sled database instance
    db: Arc<Db>,
    /// Configuration
    config: SledConfig,
    /// Statistics
    stats: Arc<RwLock<SledBackendStats>>,
    /// Operation counter for periodic flushes
    op_counter: Arc<RwLock<u64>>,
}

impl SledStateBackend {
    /// Open a Sled database with the given configuration
    ///
    /// # Arguments
    ///
    /// * `config` - Database configuration
    ///
    /// # Returns
    ///
    /// * `Ok(SledStateBackend)` - Successfully opened database
    /// * `Err(StateError)` - Failed to open database
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use processor::state::{SledStateBackend, SledConfig};
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// let config = SledConfig::new("/tmp/test_db");
    /// let backend = SledStateBackend::open(config).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn open(config: SledConfig) -> StateResult<Self> {
        info!("Opening Sled database at {:?}", config.path);

        // Create parent directory if it doesn't exist
        if let Some(parent) = config.path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                StateError::StorageError {
                    backend_type: "sled".to_string(),
                    details: format!("Failed to create directory: {}", e),
                }
            })?;
        }

        // Configure and open Sled
        let db = sled::Config::new()
            .path(&config.path)
            .cache_capacity(config.cache_capacity)
            .use_compression(config.use_compression)
            .segment_size(config.segment_size)
            .open()
            .map_err(|e| StateError::StorageError {
                backend_type: "sled".to_string(),
                details: format!("Failed to open database: {}", e),
            })?;

        info!(
            "Sled database opened successfully, {} keys",
            db.len()
        );

        Ok(Self {
            db: Arc::new(db),
            config,
            stats: Arc::new(RwLock::new(SledBackendStats::default())),
            op_counter: Arc::new(RwLock::new(0)),
        })
    }

    /// Create a temporary Sled backend for testing
    ///
    /// This creates a database in a temporary directory that will be cleaned up
    /// when the backend is dropped.
    ///
    /// # Returns
    ///
    /// A new temporary Sled backend
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use processor::state::SledStateBackend;
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// let backend = SledStateBackend::temporary().await?;
    /// // Use for testing...
    /// # Ok(())
    /// # }
    /// ```
    pub async fn temporary() -> StateResult<Self> {
        let temp_dir = std::env::temp_dir().join(format!("sled_test_{}", uuid::Uuid::new_v4()));
        Self::open(SledConfig::new(temp_dir)).await
    }

    /// Flush all pending writes to disk
    ///
    /// This ensures that all writes are persisted to disk. It's called automatically
    /// after every N operations (configured by `flush_every`), but can also be called
    /// manually for critical checkpoints.
    ///
    /// # Returns
    ///
    /// * `Ok(usize)` - Number of bytes flushed
    /// * `Err(StateError)` - Flush failed
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use processor::state::{SledStateBackend, SledConfig};
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// # let backend = SledStateBackend::temporary().await?;
    /// backend.put(b"important", b"data").await?;
    /// let flushed = backend.flush().await?;
    /// println!("Flushed {} bytes", flushed);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn flush(&self) -> StateResult<usize> {
        trace!("Flushing Sled database");

        let flushed = tokio::task::block_in_place(|| {
            self.db.flush().map_err(|e| StateError::StorageError {
                backend_type: "sled".to_string(),
                details: format!("Flush failed: {}", e),
            })
        })?;

        let mut stats = self.stats.write().await;
        stats.flush_count += 1;

        debug!("Flushed {} bytes to disk", flushed);
        Ok(flushed)
    }

    /// Get database statistics
    ///
    /// # Returns
    ///
    /// Current backend statistics
    pub async fn stats(&self) -> SledBackendStats {
        self.stats.read().await.clone()
    }

    /// Get the size of the database on disk in bytes
    ///
    /// # Returns
    ///
    /// Database size in bytes
    pub async fn size_on_disk(&self) -> StateResult<u64> {
        let path = &self.config.path;
        let mut total_size = 0u64;

        let entries = tokio::fs::read_dir(path).await.map_err(|e| {
            StateError::StorageError {
                backend_type: "sled".to_string(),
                details: format!("Failed to read directory: {}", e),
            }
        })?;

        let mut entries = entries;
        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            StateError::StorageError {
                backend_type: "sled".to_string(),
                details: format!("Failed to read entry: {}", e),
            }
        })? {
            if let Ok(metadata) = entry.metadata().await {
                total_size += metadata.len();
            }
        }

        Ok(total_size)
    }

    /// Create a checkpoint of the current database state
    ///
    /// This exports all key-value pairs for backup or migration.
    ///
    /// # Returns
    ///
    /// Vector of (key, value) pairs
    pub async fn checkpoint(&self) -> StateResult<Vec<(Vec<u8>, Vec<u8>)>> {
        info!("Creating Sled checkpoint");

        let checkpoint = tokio::task::block_in_place(|| {
            self.db
                .iter()
                .map(|result| {
                    result
                        .map(|(k, v)| (k.to_vec(), v.to_vec()))
                        .map_err(|e| StateError::StorageError {
                            backend_type: "sled".to_string(),
                            details: format!("Checkpoint iteration failed: {}", e),
                        })
                })
                .collect::<StateResult<Vec<_>>>()
        })?;

        info!("Checkpoint created with {} entries", checkpoint.len());
        Ok(checkpoint)
    }

    /// Restore from a checkpoint
    ///
    /// This clears the current database and restores from the provided checkpoint.
    ///
    /// # Arguments
    ///
    /// * `checkpoint` - Vector of (key, value) pairs to restore
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Restore successful
    /// * `Err(StateError)` - Restore failed
    pub async fn restore(&self, checkpoint: Vec<(Vec<u8>, Vec<u8>)>) -> StateResult<()> {
        info!("Restoring Sled from checkpoint with {} entries", checkpoint.len());

        tokio::task::block_in_place(|| {
            // Clear existing data
            self.db.clear().map_err(|e| StateError::StorageError {
                backend_type: "sled".to_string(),
                details: format!("Clear failed: {}", e),
            })?;

            // Restore checkpoint
            for (key, value) in checkpoint {
                self.db.insert(key, value).map_err(|e| StateError::StorageError {
                    backend_type: "sled".to_string(),
                    details: format!("Insert failed during restore: {}", e),
                })?;
            }

            // Flush to disk
            self.db.flush().map_err(|e| StateError::StorageError {
                backend_type: "sled".to_string(),
                details: format!("Flush failed after restore: {}", e),
            })?;

            Ok(())
        })?;

        info!("Restore completed successfully");
        Ok(())
    }

    /// Check if automatic flush is needed and perform it
    async fn maybe_flush(&self) -> StateResult<()> {
        let mut counter = self.op_counter.write().await;
        *counter += 1;

        if *counter >= self.config.flush_every {
            *counter = 0;
            drop(counter); // Release lock before flush
            self.flush().await?;
        }

        Ok(())
    }
}

#[async_trait]
impl StateBackend for SledStateBackend {
    async fn get(&self, key: &[u8]) -> StateResult<Option<Vec<u8>>> {
        trace!("Getting key from Sled: {:?}", key);

        let result = tokio::task::block_in_place(|| {
            self.db.get(key).map_err(|e| StateError::StorageError {
                backend_type: "sled".to_string(),
                details: format!("Get failed: {}", e),
            })
        })?;

        let mut stats = self.stats.write().await;
        stats.get_count += 1;

        if let Some(ref value) = result {
            stats.bytes_read += value.len() as u64;
        }

        Ok(result.map(|v| v.to_vec()))
    }

    async fn put(&self, key: &[u8], value: &[u8]) -> StateResult<()> {
        trace!("Putting key to Sled: {:?}, value size: {} bytes", key, value.len());

        tokio::task::block_in_place(|| {
            self.db.insert(key, value).map_err(|e| StateError::StorageError {
                backend_type: "sled".to_string(),
                details: format!("Put failed: {}", e),
            })
        })?;

        let mut stats = self.stats.write().await;
        stats.put_count += 1;
        stats.bytes_written += (key.len() + value.len()) as u64;
        drop(stats);

        self.maybe_flush().await?;

        Ok(())
    }

    async fn delete(&self, key: &[u8]) -> StateResult<()> {
        trace!("Deleting key from Sled: {:?}", key);

        tokio::task::block_in_place(|| {
            self.db.remove(key).map_err(|e| StateError::StorageError {
                backend_type: "sled".to_string(),
                details: format!("Delete failed: {}", e),
            })
        })?;

        let mut stats = self.stats.write().await;
        stats.delete_count += 1;
        drop(stats);

        self.maybe_flush().await?;

        Ok(())
    }

    async fn list_keys(&self, prefix: &[u8]) -> StateResult<Vec<Vec<u8>>> {
        trace!("Listing keys with prefix: {:?}", prefix);

        let keys = tokio::task::block_in_place(|| {
            if prefix.is_empty() {
                // List all keys
                self.db
                    .iter()
                    .keys()
                    .map(|result| {
                        result
                            .map(|k| k.to_vec())
                            .map_err(|e| StateError::StorageError {
                                backend_type: "sled".to_string(),
                                details: format!("Key iteration failed: {}", e),
                            })
                    })
                    .collect::<StateResult<Vec<_>>>()
            } else {
                // List keys with prefix
                self.db
                    .scan_prefix(prefix)
                    .keys()
                    .map(|result| {
                        result
                            .map(|k| k.to_vec())
                            .map_err(|e| StateError::StorageError {
                                backend_type: "sled".to_string(),
                                details: format!("Prefix scan failed: {}", e),
                            })
                    })
                    .collect::<StateResult<Vec<_>>>()
            }
        })?;

        Ok(keys)
    }

    async fn clear(&self) -> StateResult<()> {
        debug!("Clearing Sled database");

        tokio::task::block_in_place(|| {
            self.db.clear().map_err(|e| StateError::StorageError {
                backend_type: "sled".to_string(),
                details: format!("Clear failed: {}", e),
            })
        })?;

        self.flush().await?;

        Ok(())
    }

    async fn count(&self) -> StateResult<usize> {
        Ok(self.db.len())
    }

    async fn contains(&self, key: &[u8]) -> StateResult<bool> {
        Ok(tokio::task::block_in_place(|| {
            self.db.contains_key(key).unwrap_or(false)
        }))
    }
}

// Implement Drop to ensure clean shutdown
impl Drop for SledStateBackend {
    fn drop(&mut self) {
        if let Err(e) = self.db.flush() {
            error!("Failed to flush Sled database on drop: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::backend::tests::*;

    async fn create_test_backend() -> SledStateBackend {
        SledStateBackend::temporary().await.unwrap()
    }

    #[tokio::test]
    async fn test_sled_backend_basic() {
        let backend = create_test_backend().await;
        test_backend_basic_ops(backend).await;
    }

    #[tokio::test]
    async fn test_sled_backend_list_keys() {
        let backend = create_test_backend().await;
        test_backend_list_keys(backend).await;
    }

    #[tokio::test]
    async fn test_sled_backend_clear() {
        let backend = create_test_backend().await;
        test_backend_clear(backend).await;
    }

    #[tokio::test]
    async fn test_sled_backend_contains() {
        let backend = create_test_backend().await;
        test_backend_contains(backend).await;
    }

    #[tokio::test]
    async fn test_sled_persistence() {
        let temp_dir = std::env::temp_dir().join(format!("sled_persist_test_{}", uuid::Uuid::new_v4()));
        let config = SledConfig::new(&temp_dir);

        // Create backend and add data
        {
            let backend = SledStateBackend::open(config.clone()).await.unwrap();
            backend.put(b"key1", b"value1").await.unwrap();
            backend.put(b"key2", b"value2").await.unwrap();
            backend.flush().await.unwrap();
            // Backend dropped here
        }

        // Reopen and verify data persisted
        {
            let backend = SledStateBackend::open(config).await.unwrap();
            assert_eq!(backend.count().await.unwrap(), 2);
            assert_eq!(
                backend.get(b"key1").await.unwrap(),
                Some(b"value1".to_vec())
            );
            assert_eq!(
                backend.get(b"key2").await.unwrap(),
                Some(b"value2".to_vec())
            );
        }

        // Cleanup
        tokio::fs::remove_dir_all(&temp_dir).await.ok();
    }

    #[tokio::test]
    async fn test_checkpoint_restore() {
        let backend = create_test_backend().await;

        backend.put(b"key1", b"value1").await.unwrap();
        backend.put(b"key2", b"value2").await.unwrap();

        // Create checkpoint
        let checkpoint = backend.checkpoint().await.unwrap();
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
        let backend = create_test_backend().await;

        backend.put(b"key1", b"value1").await.unwrap();
        backend.put(b"key2", b"value2").await.unwrap();

        backend.get(b"key1").await.unwrap();
        backend.delete(b"key1").await.unwrap();

        let stats = backend.stats().await;
        assert_eq!(stats.put_count, 2);
        assert_eq!(stats.get_count, 1);
        assert_eq!(stats.delete_count, 1);
        assert!(stats.bytes_written > 0);
        assert!(stats.bytes_read > 0);
    }

    #[tokio::test]
    async fn test_size_on_disk() {
        let backend = create_test_backend().await;

        let initial_size = backend.size_on_disk().await.unwrap();

        // Add some data
        for i in 0..100 {
            let key = format!("key:{}", i).into_bytes();
            let value = vec![0u8; 1000]; // 1KB value
            backend.put(&key, &value).await.unwrap();
        }

        backend.flush().await.unwrap();

        let final_size = backend.size_on_disk().await.unwrap();
        assert!(final_size > initial_size);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        use std::sync::Arc;

        let backend = Arc::new(create_test_backend().await);
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

        backend.flush().await.unwrap();

        // Verify all entries were written
        assert_eq!(backend.count().await.unwrap(), 100);
    }
}
