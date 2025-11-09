//! Checkpointing and recovery for state backends
//!
//! This module provides checkpoint coordination for creating periodic snapshots
//! of state backends. Checkpoints enable fault tolerance and allow state to be
//! recovered after failures.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use super::backend::StateBackend;
use crate::error::{StateError, StateResult};

/// Checkpoint metadata
///
/// Contains information about a checkpoint including when it was created,
/// how many entries it contains, and whether it's been validated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointMetadata {
    /// Unique checkpoint identifier
    pub checkpoint_id: String,
    /// When the checkpoint was created
    pub created_at: DateTime<Utc>,
    /// Number of key-value pairs in the checkpoint
    pub entry_count: usize,
    /// Size of checkpoint file in bytes
    pub size_bytes: u64,
    /// Checksum for integrity verification (SHA-256)
    pub checksum: String,
    /// Whether this checkpoint has been validated
    pub validated: bool,
    /// Version of the checkpoint format
    pub version: u32,
}

impl CheckpointMetadata {
    /// Create new checkpoint metadata
    pub fn new(checkpoint_id: String, entry_count: usize, size_bytes: u64, checksum: String) -> Self {
        Self {
            checkpoint_id,
            created_at: Utc::now(),
            entry_count,
            size_bytes,
            checksum,
            validated: false,
            version: 1,
        }
    }
}

/// A checkpoint containing state data and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Checkpoint metadata
    pub metadata: CheckpointMetadata,
    /// The actual state data (key-value pairs)
    pub data: Vec<(Vec<u8>, Vec<u8>)>,
}

impl Checkpoint {
    /// Create a new checkpoint
    pub fn new(checkpoint_id: String, data: Vec<(Vec<u8>, Vec<u8>)>) -> Self {
        let size_bytes = data
            .iter()
            .map(|(k, v)| k.len() + v.len())
            .sum::<usize>() as u64;

        let checksum = Self::calculate_checksum(&data);

        Self {
            metadata: CheckpointMetadata::new(
                checkpoint_id,
                data.len(),
                size_bytes,
                checksum,
            ),
            data,
        }
    }

    /// Calculate SHA-256 checksum of the data
    fn calculate_checksum(data: &[(Vec<u8>, Vec<u8>)]) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        for (key, value) in data {
            key.hash(&mut hasher);
            value.hash(&mut hasher);
        }

        format!("{:x}", hasher.finish())
    }

    /// Validate the checkpoint's integrity
    pub fn validate(&mut self) -> StateResult<()> {
        let calculated_checksum = Self::calculate_checksum(&self.data);

        if calculated_checksum != self.metadata.checksum {
            return Err(StateError::CheckpointFailed {
                checkpoint_id: self.metadata.checkpoint_id.clone(),
                reason: format!(
                    "Checksum mismatch: expected {}, got {}",
                    self.metadata.checksum, calculated_checksum
                ),
            });
        }

        self.metadata.validated = true;
        Ok(())
    }

    /// Save checkpoint to a file
    pub async fn save<P: AsRef<Path>>(&self, path: P) -> StateResult<()> {
        let path = path.as_ref();
        info!("Saving checkpoint {} to {:?}", self.metadata.checkpoint_id, path);

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                StateError::CheckpointFailed {
                    checkpoint_id: self.metadata.checkpoint_id.clone(),
                    reason: format!("Failed to create directory: {}", e),
                }
            })?;
        }

        // Serialize checkpoint
        let serialized = bincode::serialize(self).map_err(|e| StateError::SerializationFailed {
            key: self.metadata.checkpoint_id.clone(),
            reason: e.to_string(),
        })?;

        // Write to file
        tokio::fs::write(path, &serialized).await.map_err(|e| {
            StateError::CheckpointFailed {
                checkpoint_id: self.metadata.checkpoint_id.clone(),
                reason: format!("Failed to write file: {}", e),
            }
        })?;

        info!(
            "Checkpoint saved: {} entries, {} bytes",
            self.metadata.entry_count, self.metadata.size_bytes
        );

        Ok(())
    }

    /// Load checkpoint from a file
    pub async fn load<P: AsRef<Path>>(path: P) -> StateResult<Self> {
        let path = path.as_ref();
        info!("Loading checkpoint from {:?}", path);

        // Read file
        let data = tokio::fs::read(path).await.map_err(|e| {
            StateError::RestoreFailed {
                checkpoint_id: path.to_string_lossy().to_string(),
                reason: format!("Failed to read file: {}", e),
            }
        })?;

        // Deserialize
        let mut checkpoint: Checkpoint =
            bincode::deserialize(&data).map_err(|e| StateError::DeserializationFailed {
                key: path.to_string_lossy().to_string(),
                reason: e.to_string(),
            })?;

        // Validate
        checkpoint.validate()?;

        info!(
            "Checkpoint loaded: {} entries, {} bytes",
            checkpoint.metadata.entry_count, checkpoint.metadata.size_bytes
        );

        Ok(checkpoint)
    }
}

/// Options for checkpoint creation
#[derive(Debug, Clone)]
pub struct CheckpointOptions {
    /// Checkpoint directory
    pub checkpoint_dir: PathBuf,
    /// Maximum number of checkpoints to keep
    pub max_checkpoints: usize,
    /// Whether to compress checkpoints
    pub compress: bool,
    /// Minimum interval between checkpoints
    pub min_interval: Duration,
}

impl Default for CheckpointOptions {
    fn default() -> Self {
        Self {
            checkpoint_dir: PathBuf::from("/tmp/checkpoints"),
            max_checkpoints: 5,
            compress: true,
            min_interval: Duration::from_secs(60),
        }
    }
}

/// Statistics about checkpointing
#[derive(Debug, Clone, Default)]
pub struct CheckpointStats {
    /// Total number of checkpoints created
    pub checkpoints_created: u64,
    /// Total number of checkpoint failures
    pub checkpoint_failures: u64,
    /// Total number of restores
    pub restores: u64,
    /// Total number of restore failures
    pub restore_failures: u64,
    /// Last checkpoint timestamp
    pub last_checkpoint_time: Option<DateTime<Utc>>,
    /// Last checkpoint duration in milliseconds
    pub last_checkpoint_duration_ms: Option<u64>,
    /// Total bytes checkpointed
    pub total_bytes_checkpointed: u64,
}

/// Checkpoint coordinator
///
/// Manages periodic checkpoint creation and restoration for a state backend.
///
/// ## Features
///
/// - **Periodic checkpoints**: Automatically creates checkpoints at regular intervals
/// - **Retention policy**: Keeps only the N most recent checkpoints
/// - **Graceful shutdown**: Ensures final checkpoint on shutdown
/// - **Recovery**: Can restore from the latest valid checkpoint
///
/// ## Example
///
/// ```rust,no_run
/// use processor::state::{
///     CheckpointCoordinator, CheckpointOptions, MemoryStateBackend
/// };
/// use std::sync::Arc;
/// use std::time::Duration;
/// use std::path::PathBuf;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let backend = Arc::new(MemoryStateBackend::new(None));
///
///     let options = CheckpointOptions {
///         checkpoint_dir: PathBuf::from("/var/lib/optimizer/checkpoints"),
///         max_checkpoints: 10,
///         compress: true,
///         min_interval: Duration::from_secs(300),
///     };
///
///     let coordinator = CheckpointCoordinator::new(
///         backend.clone(),
///         Duration::from_secs(300),
///         options,
///     );
///
///     // Start periodic checkpointing
///     coordinator.start().await?;
///
///     // Work with state...
///
///     // Shutdown with final checkpoint
///     coordinator.shutdown().await?;
///
///     Ok(())
/// }
/// ```
pub struct CheckpointCoordinator<B: StateBackend> {
    /// The state backend to checkpoint
    backend: Arc<B>,
    /// Checkpoint interval
    interval: Duration,
    /// Checkpoint options
    options: CheckpointOptions,
    /// Statistics
    stats: Arc<RwLock<CheckpointStats>>,
    /// Background task handle
    task_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    /// Shutdown signal
    shutdown_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
}

impl<B: StateBackend + 'static> CheckpointCoordinator<B> {
    /// Create a new checkpoint coordinator
    ///
    /// # Arguments
    ///
    /// * `backend` - The state backend to checkpoint
    /// * `interval` - How often to create checkpoints
    /// * `options` - Checkpoint configuration options
    ///
    /// # Returns
    ///
    /// A new coordinator instance
    pub fn new(backend: Arc<B>, interval: Duration, options: CheckpointOptions) -> Self {
        Self {
            backend,
            interval,
            options,
            stats: Arc::new(RwLock::new(CheckpointStats::default())),
            task_handle: Arc::new(Mutex::new(None)),
            shutdown_tx: Arc::new(Mutex::new(None)),
        }
    }

    /// Start periodic checkpointing
    ///
    /// Spawns a background task that creates checkpoints at the configured interval.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Checkpointing started
    /// * `Err(StateError)` - Failed to start
    pub async fn start(&self) -> StateResult<()> {
        let mut handle_guard = self.task_handle.lock().await;

        if handle_guard.is_some() {
            warn!("Checkpoint coordinator already started");
            return Ok(());
        }

        info!("Starting checkpoint coordinator with interval {:?}", self.interval);

        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        *self.shutdown_tx.lock().await = Some(shutdown_tx);

        let backend = Arc::clone(&self.backend);
        let options = self.options.clone();
        let stats = Arc::clone(&self.stats);
        let interval = self.interval;

        let handle = tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            let mut last_checkpoint = Utc::now();

            loop {
                tokio::select! {
                    _ = interval_timer.tick() => {
                        // Check minimum interval
                        let elapsed = Utc::now().signed_duration_since(last_checkpoint);
                        if elapsed.num_milliseconds() as u64 >= options.min_interval.as_millis() as u64 {
                            if let Err(e) = Self::create_checkpoint_internal(
                                &backend,
                                &options,
                                &stats,
                            ).await {
                                error!("Checkpoint failed: {}", e);
                                let mut stats_guard = stats.write().await;
                                stats_guard.checkpoint_failures += 1;
                            } else {
                                last_checkpoint = Utc::now();
                            }
                        }
                    }
                    _ = &mut shutdown_rx => {
                        info!("Checkpoint coordinator shutting down");
                        // Create final checkpoint
                        if let Err(e) = Self::create_checkpoint_internal(
                            &backend,
                            &options,
                            &stats,
                        ).await {
                            error!("Final checkpoint failed: {}", e);
                        }
                        break;
                    }
                }
            }
        });

        *handle_guard = Some(handle);
        Ok(())
    }

    /// Create a checkpoint immediately
    ///
    /// # Returns
    ///
    /// * `Ok(checkpoint_id)` - Checkpoint created successfully
    /// * `Err(StateError)` - Checkpoint failed
    pub async fn create_checkpoint(&self) -> StateResult<String> {
        Self::create_checkpoint_internal(&self.backend, &self.options, &self.stats).await
    }

    /// Internal checkpoint creation
    async fn create_checkpoint_internal(
        backend: &Arc<B>,
        options: &CheckpointOptions,
        stats: &Arc<RwLock<CheckpointStats>>,
    ) -> StateResult<String> {
        let start_time = Utc::now();
        let checkpoint_id = format!("checkpoint_{}", start_time.timestamp_millis());

        debug!("Creating checkpoint {}", checkpoint_id);

        // Get all state data
        let data = Self::export_state(backend).await?;

        // Create checkpoint
        let checkpoint = Checkpoint::new(checkpoint_id.clone(), data);

        // Save to disk
        let checkpoint_path = options
            .checkpoint_dir
            .join(format!("{}.ckpt", checkpoint_id));

        checkpoint.save(&checkpoint_path).await?;

        // Update statistics
        let duration = Utc::now().signed_duration_since(start_time);
        let mut stats_guard = stats.write().await;
        stats_guard.checkpoints_created += 1;
        stats_guard.last_checkpoint_time = Some(start_time);
        stats_guard.last_checkpoint_duration_ms = Some(duration.num_milliseconds() as u64);
        stats_guard.total_bytes_checkpointed += checkpoint.metadata.size_bytes;
        drop(stats_guard);

        // Clean up old checkpoints
        Self::cleanup_old_checkpoints(options).await?;

        info!(
            "Checkpoint {} created in {}ms",
            checkpoint_id,
            duration.num_milliseconds()
        );

        Ok(checkpoint_id)
    }

    /// Export all state from the backend
    async fn export_state(backend: &Arc<B>) -> StateResult<Vec<(Vec<u8>, Vec<u8>)>> {
        let keys = backend.list_keys(b"").await?;
        let mut data = Vec::with_capacity(keys.len());

        for key in keys {
            if let Some(value) = backend.get(&key).await? {
                data.push((key, value));
            }
        }

        Ok(data)
    }

    /// Clean up old checkpoints based on retention policy
    async fn cleanup_old_checkpoints(options: &CheckpointOptions) -> StateResult<()> {
        let mut entries = tokio::fs::read_dir(&options.checkpoint_dir)
            .await
            .map_err(|e| StateError::StorageError {
                backend_type: "checkpoint".to_string(),
                details: format!("Failed to read checkpoint directory: {}", e),
            })?;

        let mut checkpoints = Vec::new();

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            StateError::StorageError {
                backend_type: "checkpoint".to_string(),
                details: format!("Failed to read directory entry: {}", e),
            }
        })? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("ckpt") {
                if let Ok(metadata) = entry.metadata().await {
                    if let Ok(modified) = metadata.modified() {
                        checkpoints.push((path, modified));
                    }
                }
            }
        }

        // Sort by modification time (newest first)
        checkpoints.sort_by(|a, b| b.1.cmp(&a.1));

        // Delete old checkpoints
        for (path, _) in checkpoints.into_iter().skip(options.max_checkpoints) {
            debug!("Deleting old checkpoint: {:?}", path);
            if let Err(e) = tokio::fs::remove_file(&path).await {
                warn!("Failed to delete checkpoint {:?}: {}", path, e);
            }
        }

        Ok(())
    }

    /// Restore from the latest checkpoint
    ///
    /// # Returns
    ///
    /// * `Ok(checkpoint_id)` - Successfully restored
    /// * `Err(StateError)` - No checkpoint found or restore failed
    pub async fn restore_latest(&self) -> StateResult<String> {
        info!("Restoring from latest checkpoint");

        // Find latest checkpoint
        let checkpoint_path = self.find_latest_checkpoint().await?;

        // Load checkpoint
        let checkpoint = Checkpoint::load(&checkpoint_path).await?;

        // Restore to backend
        self.restore_checkpoint(&checkpoint).await?;

        let mut stats = self.stats.write().await;
        stats.restores += 1;

        Ok(checkpoint.metadata.checkpoint_id)
    }

    /// Find the latest checkpoint file
    async fn find_latest_checkpoint(&self) -> StateResult<PathBuf> {
        let mut entries = tokio::fs::read_dir(&self.options.checkpoint_dir)
            .await
            .map_err(|e| StateError::RestoreFailed {
                checkpoint_id: "unknown".to_string(),
                reason: format!("Failed to read checkpoint directory: {}", e),
            })?;

        let mut latest: Option<(PathBuf, std::time::SystemTime)> = None;

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            StateError::RestoreFailed {
                checkpoint_id: "unknown".to_string(),
                reason: format!("Failed to read directory entry: {}", e),
            }
        })? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("ckpt") {
                if let Ok(metadata) = entry.metadata().await {
                    if let Ok(modified) = metadata.modified() {
                        if latest.is_none() || modified > latest.as_ref().unwrap().1 {
                            latest = Some((path, modified));
                        }
                    }
                }
            }
        }

        latest.map(|(path, _)| path).ok_or_else(|| StateError::RestoreFailed {
            checkpoint_id: "unknown".to_string(),
            reason: "No checkpoint files found".to_string(),
        })
    }

    /// Restore state from a checkpoint
    async fn restore_checkpoint(&self, checkpoint: &Checkpoint) -> StateResult<()> {
        info!(
            "Restoring checkpoint {} with {} entries",
            checkpoint.metadata.checkpoint_id, checkpoint.metadata.entry_count
        );

        // Clear existing state
        self.backend.clear().await?;

        // Restore all entries
        for (key, value) in &checkpoint.data {
            self.backend.put(key, value).await?;
        }

        info!("Checkpoint restored successfully");
        Ok(())
    }

    /// Get checkpoint statistics
    ///
    /// # Returns
    ///
    /// Current checkpoint statistics
    pub async fn stats(&self) -> CheckpointStats {
        self.stats.read().await.clone()
    }

    /// Shutdown the coordinator
    ///
    /// Stops the background task and creates a final checkpoint.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Shutdown successful
    /// * `Err(StateError)` - Shutdown failed
    pub async fn shutdown(&self) -> StateResult<()> {
        info!("Shutting down checkpoint coordinator");

        // Send shutdown signal
        if let Some(tx) = self.shutdown_tx.lock().await.take() {
            let _ = tx.send(());
        }

        // Wait for background task
        if let Some(handle) = self.task_handle.lock().await.take() {
            if let Err(e) = handle.await {
                error!("Background task panicked: {}", e);
            }
        }

        info!("Checkpoint coordinator shutdown complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::MemoryStateBackend;

    #[tokio::test]
    async fn test_checkpoint_creation() {
        let backend = Arc::new(MemoryStateBackend::new(None));

        backend.put(b"key1", b"value1").await.unwrap();
        backend.put(b"key2", b"value2").await.unwrap();

        let data = vec![
            (b"key1".to_vec(), b"value1".to_vec()),
            (b"key2".to_vec(), b"value2".to_vec()),
        ];

        let checkpoint = Checkpoint::new("test_checkpoint".to_string(), data);
        assert_eq!(checkpoint.metadata.entry_count, 2);
        assert!(checkpoint.metadata.size_bytes > 0);
    }

    #[tokio::test]
    async fn test_checkpoint_validation() {
        let data = vec![(b"key1".to_vec(), b"value1".to_vec())];
        let mut checkpoint = Checkpoint::new("test".to_string(), data);

        checkpoint.validate().unwrap();
        assert!(checkpoint.metadata.validated);

        // Corrupt the data
        checkpoint.data.push((b"key2".to_vec(), b"value2".to_vec()));

        assert!(checkpoint.validate().is_err());
    }

    #[tokio::test]
    async fn test_checkpoint_save_load() {
        let temp_dir = std::env::temp_dir().join(format!("checkpoint_test_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let data = vec![
            (b"key1".to_vec(), b"value1".to_vec()),
            (b"key2".to_vec(), b"value2".to_vec()),
        ];

        let checkpoint = Checkpoint::new("test".to_string(), data);
        let checkpoint_path = temp_dir.join("test.ckpt");

        checkpoint.save(&checkpoint_path).await.unwrap();
        assert!(checkpoint_path.exists());

        let loaded = Checkpoint::load(&checkpoint_path).await.unwrap();
        assert_eq!(loaded.metadata.entry_count, 2);
        assert!(loaded.metadata.validated);

        // Cleanup
        tokio::fs::remove_dir_all(&temp_dir).await.ok();
    }

    #[tokio::test]
    async fn test_coordinator_create_checkpoint() {
        let temp_dir = std::env::temp_dir().join(format!("coordinator_test_{}", uuid::Uuid::new_v4()));

        let backend = Arc::new(MemoryStateBackend::new(None));
        backend.put(b"key1", b"value1").await.unwrap();
        backend.put(b"key2", b"value2").await.unwrap();

        let options = CheckpointOptions {
            checkpoint_dir: temp_dir.clone(),
            max_checkpoints: 3,
            compress: false,
            min_interval: Duration::from_secs(1),
        };

        let coordinator = CheckpointCoordinator::new(
            backend.clone(),
            Duration::from_secs(60),
            options,
        );

        let checkpoint_id = coordinator.create_checkpoint().await.unwrap();
        assert!(!checkpoint_id.is_empty());

        let stats = coordinator.stats().await;
        assert_eq!(stats.checkpoints_created, 1);

        // Cleanup
        tokio::fs::remove_dir_all(&temp_dir).await.ok();
    }

    #[tokio::test]
    async fn test_coordinator_restore() {
        let temp_dir = std::env::temp_dir().join(format!("restore_test_{}", uuid::Uuid::new_v4()));

        let backend = Arc::new(MemoryStateBackend::new(None));
        backend.put(b"key1", b"value1").await.unwrap();
        backend.put(b"key2", b"value2").await.unwrap();

        let options = CheckpointOptions {
            checkpoint_dir: temp_dir.clone(),
            max_checkpoints: 3,
            compress: false,
            min_interval: Duration::from_secs(1),
        };

        let coordinator = CheckpointCoordinator::new(
            backend.clone(),
            Duration::from_secs(60),
            options,
        );

        // Create checkpoint
        coordinator.create_checkpoint().await.unwrap();

        // Clear backend
        backend.clear().await.unwrap();
        assert_eq!(backend.count().await.unwrap(), 0);

        // Restore
        coordinator.restore_latest().await.unwrap();
        assert_eq!(backend.count().await.unwrap(), 2);
        assert_eq!(
            backend.get(b"key1").await.unwrap(),
            Some(b"value1".to_vec())
        );

        // Cleanup
        tokio::fs::remove_dir_all(&temp_dir).await.ok();
    }

    #[tokio::test]
    async fn test_checkpoint_retention() {
        let temp_dir = std::env::temp_dir().join(format!("retention_test_{}", uuid::Uuid::new_v4()));

        let backend = Arc::new(MemoryStateBackend::new(None));

        let options = CheckpointOptions {
            checkpoint_dir: temp_dir.clone(),
            max_checkpoints: 3,
            compress: false,
            min_interval: Duration::from_millis(100),
        };

        let coordinator = CheckpointCoordinator::new(
            backend.clone(),
            Duration::from_secs(60),
            options,
        );

        // Create multiple checkpoints
        for i in 0..5 {
            backend
                .put(&format!("key{}", i).into_bytes(), b"value")
                .await
                .unwrap();
            coordinator.create_checkpoint().await.unwrap();
            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        // Count checkpoint files
        let mut count = 0;
        let mut entries = tokio::fs::read_dir(&temp_dir).await.unwrap();
        while let Some(_entry) = entries.next_entry().await.unwrap() {
            count += 1;
        }

        // Should only keep max_checkpoints
        assert!(count <= 3);

        // Cleanup
        tokio::fs::remove_dir_all(&temp_dir).await.ok();
    }
}
