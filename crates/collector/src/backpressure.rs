//! Backpressure Handling
//!
//! This module implements backpressure mechanisms to prevent data loss when
//! the system is overwhelmed. It provides overflow strategies, buffer monitoring,
//! and disk-based fallback storage.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::fs::{self, File};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::feedback_events::FeedbackEvent;

/// Overflow strategy for when buffer is full
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverflowStrategy {
    /// Drop oldest events from buffer
    DropOldest,
    /// Drop newest events (reject new submissions)
    DropNewest,
    /// Reject new events and return error to caller
    RejectNew,
    /// Persist overflow events to disk
    PersistToDisk,
}

impl Default for OverflowStrategy {
    fn default() -> Self {
        Self::PersistToDisk
    }
}

impl OverflowStrategy {
    /// Get strategy name for metrics
    pub fn name(&self) -> &'static str {
        match self {
            Self::DropOldest => "drop_oldest",
            Self::DropNewest => "drop_newest",
            Self::RejectNew => "reject_new",
            Self::PersistToDisk => "persist_to_disk",
        }
    }
}

/// Backpressure handler configuration
#[derive(Debug, Clone)]
pub struct BackpressureConfig {
    /// Buffer capacity
    pub buffer_capacity: usize,
    /// Threshold for warning (percentage, 0-100)
    pub warning_threshold_percent: u8,
    /// Threshold for critical state (percentage, 0-100)
    pub critical_threshold_percent: u8,
    /// Overflow strategy
    pub overflow_strategy: OverflowStrategy,
    /// Fallback storage directory (for PersistToDisk strategy)
    pub fallback_storage_path: Option<PathBuf>,
    /// Maximum disk storage size in bytes
    pub max_disk_storage_bytes: u64,
    /// Batch size for disk writes
    pub disk_write_batch_size: usize,
}

impl Default for BackpressureConfig {
    fn default() -> Self {
        Self {
            buffer_capacity: 10000,
            warning_threshold_percent: 80,
            critical_threshold_percent: 95,
            overflow_strategy: OverflowStrategy::PersistToDisk,
            fallback_storage_path: Some(PathBuf::from("/tmp/llm-optimizer/fallback")),
            max_disk_storage_bytes: 1024 * 1024 * 1024, // 1GB
            disk_write_batch_size: 100,
        }
    }
}

/// Buffer utilization level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferLevel {
    /// Buffer usage is normal
    Normal,
    /// Buffer usage is high (warning threshold exceeded)
    Warning,
    /// Buffer usage is critical
    Critical,
    /// Buffer is full
    Full,
}

impl BufferLevel {
    /// Get level name for metrics
    pub fn name(&self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Warning => "warning",
            Self::Critical => "critical",
            Self::Full => "full",
        }
    }
}

/// Backpressure handler statistics
#[derive(Debug, Clone, Default)]
pub struct BackpressureStats {
    /// Current buffer size
    pub current_buffer_size: usize,
    /// Buffer capacity
    pub buffer_capacity: usize,
    /// Buffer utilization percentage
    pub utilization_percent: f64,
    /// Current buffer level
    pub buffer_level: String,
    /// Total events dropped (oldest)
    pub total_dropped_oldest: u64,
    /// Total events dropped (newest)
    pub total_dropped_newest: u64,
    /// Total events rejected
    pub total_rejected: u64,
    /// Total events persisted to disk
    pub total_persisted: u64,
    /// Total events recovered from disk
    pub total_recovered: u64,
    /// Current disk storage size
    pub disk_storage_bytes: u64,
}

/// Error type for backpressure operations
#[derive(Debug, thiserror::Error)]
pub enum BackpressureError {
    #[error("Buffer full - request rejected")]
    BufferFull,

    #[error("Disk storage error: {0}")]
    DiskError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Backpressure handler
pub struct BackpressureHandler {
    /// Configuration
    config: BackpressureConfig,
    /// In-memory buffer (for overflow events)
    overflow_buffer: Arc<RwLock<VecDeque<FeedbackEvent>>>,
    /// Statistics
    dropped_oldest: AtomicU64,
    dropped_newest: AtomicU64,
    rejected: AtomicU64,
    persisted_to_disk: AtomicU64,
    recovered_from_disk: AtomicU64,
    /// Current buffer size (tracked separately for metrics)
    current_buffer_size: AtomicUsize,
    /// Disk storage size
    disk_storage_bytes: AtomicU64,
}

impl BackpressureHandler {
    /// Create new backpressure handler
    pub fn new(config: BackpressureConfig) -> Self {
        info!(
            "Creating backpressure handler: capacity={}, strategy={:?}, warning={}%, critical={}%",
            config.buffer_capacity,
            config.overflow_strategy,
            config.warning_threshold_percent,
            config.critical_threshold_percent
        );

        Self {
            config,
            overflow_buffer: Arc::new(RwLock::new(VecDeque::new())),
            dropped_oldest: AtomicU64::new(0),
            dropped_newest: AtomicU64::new(0),
            rejected: AtomicU64::new(0),
            persisted_to_disk: AtomicU64::new(0),
            recovered_from_disk: AtomicU64::new(0),
            current_buffer_size: AtomicUsize::new(0),
            disk_storage_bytes: AtomicU64::new(0),
        }
    }

    /// Check buffer level based on utilization
    pub fn check_buffer_level(&self, current_size: usize) -> BufferLevel {
        let utilization = (current_size as f64 / self.config.buffer_capacity as f64) * 100.0;

        if current_size >= self.config.buffer_capacity {
            BufferLevel::Full
        } else if utilization >= self.config.critical_threshold_percent as f64 {
            BufferLevel::Critical
        } else if utilization >= self.config.warning_threshold_percent as f64 {
            BufferLevel::Warning
        } else {
            BufferLevel::Normal
        }
    }

    /// Update current buffer size
    pub fn update_buffer_size(&self, size: usize) {
        let old_size = self.current_buffer_size.swap(size, Ordering::Relaxed);
        let old_level = self.check_buffer_level(old_size);
        let new_level = self.check_buffer_level(size);

        // Log level changes
        if old_level != new_level {
            match new_level {
                BufferLevel::Normal => {
                    info!("Buffer level: NORMAL ({}/{})", size, self.config.buffer_capacity);
                }
                BufferLevel::Warning => {
                    warn!(
                        "Buffer level: WARNING ({}/{})",
                        size, self.config.buffer_capacity
                    );
                }
                BufferLevel::Critical => {
                    error!(
                        "Buffer level: CRITICAL ({}/{})",
                        size, self.config.buffer_capacity
                    );
                }
                BufferLevel::Full => {
                    error!("Buffer level: FULL ({}/{})", size, self.config.buffer_capacity);
                }
            }
        }
    }

    /// Handle overflow event based on configured strategy
    pub async fn handle_overflow(
        &self,
        event: FeedbackEvent,
    ) -> Result<(), BackpressureError> {
        match self.config.overflow_strategy {
            OverflowStrategy::DropOldest => {
                self.drop_oldest_and_add(event).await;
                Ok(())
            }
            OverflowStrategy::DropNewest => {
                self.drop_newest(event).await;
                Ok(())
            }
            OverflowStrategy::RejectNew => {
                self.reject_event(event).await;
                Err(BackpressureError::BufferFull)
            }
            OverflowStrategy::PersistToDisk => {
                self.persist_to_disk(event).await?;
                Ok(())
            }
        }
    }

    /// Drop oldest event and add new one
    async fn drop_oldest_and_add(&self, event: FeedbackEvent) {
        let mut buffer = self.overflow_buffer.write().await;

        // Remove oldest if at capacity
        if buffer.len() >= self.config.buffer_capacity {
            if let Some(dropped) = buffer.pop_front() {
                self.dropped_oldest.fetch_add(1, Ordering::Relaxed);
                warn!("Dropped oldest event: {}", dropped.id());
            }
        }

        buffer.push_back(event);
        debug!("Added event to overflow buffer (drop oldest strategy)");
    }

    /// Drop newest event
    async fn drop_newest(&self, event: FeedbackEvent) {
        self.dropped_newest.fetch_add(1, Ordering::Relaxed);
        warn!("Dropped newest event: {}", event.id());
    }

    /// Reject event
    async fn reject_event(&self, event: FeedbackEvent) {
        self.rejected.fetch_add(1, Ordering::Relaxed);
        warn!("Rejected event due to backpressure: {}", event.id());
    }

    /// Persist event to disk
    async fn persist_to_disk(&self, event: FeedbackEvent) -> Result<(), BackpressureError> {
        let storage_path = self
            .config
            .fallback_storage_path
            .as_ref()
            .ok_or_else(|| BackpressureError::DiskError("No storage path configured".into()))?;

        // Ensure directory exists
        fs::create_dir_all(storage_path).await?;

        // Check disk storage limit
        let current_disk_usage = self.disk_storage_bytes.load(Ordering::Relaxed);
        if current_disk_usage >= self.config.max_disk_storage_bytes {
            warn!("Disk storage limit reached, falling back to drop oldest");
            self.drop_oldest_and_add(event).await;
            return Ok(());
        }

        // Serialize event
        let event_json = serde_json::to_vec(&event).map_err(|e| {
            BackpressureError::SerializationError(e.to_string())
        })?;

        // Write to disk
        let file_path = storage_path.join(format!("{}.json", event.id()));
        let mut file = File::create(&file_path).await?;
        file.write_all(&event_json).await?;
        file.sync_all().await?;

        // Update disk usage
        let file_size = event_json.len() as u64;
        self.disk_storage_bytes
            .fetch_add(file_size, Ordering::Relaxed);
        self.persisted_to_disk.fetch_add(1, Ordering::Relaxed);

        debug!(
            "Persisted event {} to disk: {} bytes",
            event.id(),
            file_size
        );

        Ok(())
    }

    /// Recover events from disk
    pub async fn recover_from_disk(&self) -> Result<Vec<FeedbackEvent>, BackpressureError> {
        let storage_path = self
            .config
            .fallback_storage_path
            .as_ref()
            .ok_or_else(|| BackpressureError::DiskError("No storage path configured".into()))?;

        if !storage_path.exists() {
            return Ok(Vec::new());
        }

        let mut events = Vec::new();
        let mut dir = fs::read_dir(storage_path).await?;

        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                match self.recover_event_from_file(&path).await {
                    Ok(event) => {
                        events.push(event);
                        // Delete recovered file
                        if let Err(e) = fs::remove_file(&path).await {
                            error!("Failed to delete recovered file {:?}: {}", path, e);
                        }
                    }
                    Err(e) => {
                        error!("Failed to recover event from {:?}: {}", path, e);
                    }
                }
            }
        }

        let recovered_count = events.len();
        if recovered_count > 0 {
            self.recovered_from_disk
                .fetch_add(recovered_count as u64, Ordering::Relaxed);
            info!("Recovered {} events from disk", recovered_count);
        }

        Ok(events)
    }

    /// Recover single event from file
    async fn recover_event_from_file(
        &self,
        path: &PathBuf,
    ) -> Result<FeedbackEvent, BackpressureError> {
        let mut file = File::open(path).await?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).await?;

        let file_size = contents.len() as u64;
        self.disk_storage_bytes
            .fetch_sub(file_size, Ordering::Relaxed);

        let event: FeedbackEvent = serde_json::from_slice(&contents)
            .map_err(|e| BackpressureError::SerializationError(e.to_string()))?;

        Ok(event)
    }

    /// Get statistics
    pub fn stats(&self) -> BackpressureStats {
        let current_size = self.current_buffer_size.load(Ordering::Relaxed);
        let level = self.check_buffer_level(current_size);
        let utilization = (current_size as f64 / self.config.buffer_capacity as f64) * 100.0;

        BackpressureStats {
            current_buffer_size: current_size,
            buffer_capacity: self.config.buffer_capacity,
            utilization_percent: utilization,
            buffer_level: level.name().to_string(),
            total_dropped_oldest: self.dropped_oldest.load(Ordering::Relaxed),
            total_dropped_newest: self.dropped_newest.load(Ordering::Relaxed),
            total_rejected: self.rejected.load(Ordering::Relaxed),
            total_persisted: self.persisted_to_disk.load(Ordering::Relaxed),
            total_recovered: self.recovered_from_disk.load(Ordering::Relaxed),
            disk_storage_bytes: self.disk_storage_bytes.load(Ordering::Relaxed),
        }
    }

    /// Initialize fallback storage (create directory if needed)
    pub async fn initialize_storage(&self) -> Result<(), BackpressureError> {
        if let Some(path) = &self.config.fallback_storage_path {
            fs::create_dir_all(path).await?;
            info!("Initialized fallback storage at {:?}", path);
        }
        Ok(())
    }

    /// Clean up old fallback files (manual maintenance)
    pub async fn cleanup_storage(&self) -> Result<u64, BackpressureError> {
        let storage_path = self
            .config
            .fallback_storage_path
            .as_ref()
            .ok_or_else(|| BackpressureError::DiskError("No storage path configured".into()))?;

        if !storage_path.exists() {
            return Ok(0);
        }

        let mut cleaned_bytes = 0u64;
        let mut dir = fs::read_dir(storage_path).await?;

        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(metadata) = fs::metadata(&path).await {
                    cleaned_bytes += metadata.len();
                }
                fs::remove_file(&path).await?;
            }
        }

        self.disk_storage_bytes.store(0, Ordering::Relaxed);
        info!("Cleaned up {} bytes from fallback storage", cleaned_bytes);

        Ok(cleaned_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feedback_events::UserFeedbackEvent;
    use tempfile::TempDir;
    use uuid::Uuid;

    #[test]
    fn test_overflow_strategy_names() {
        assert_eq!(OverflowStrategy::DropOldest.name(), "drop_oldest");
        assert_eq!(OverflowStrategy::DropNewest.name(), "drop_newest");
        assert_eq!(OverflowStrategy::RejectNew.name(), "reject_new");
        assert_eq!(OverflowStrategy::PersistToDisk.name(), "persist_to_disk");
    }

    #[test]
    fn test_buffer_level_calculation() {
        let config = BackpressureConfig {
            buffer_capacity: 100,
            warning_threshold_percent: 80,
            critical_threshold_percent: 95,
            ..Default::default()
        };

        let handler = BackpressureHandler::new(config);

        assert_eq!(handler.check_buffer_level(50), BufferLevel::Normal);
        assert_eq!(handler.check_buffer_level(85), BufferLevel::Warning);
        assert_eq!(handler.check_buffer_level(96), BufferLevel::Critical);
        assert_eq!(handler.check_buffer_level(100), BufferLevel::Full);
    }

    #[tokio::test]
    async fn test_drop_oldest_strategy() {
        let config = BackpressureConfig {
            buffer_capacity: 2,
            overflow_strategy: OverflowStrategy::DropOldest,
            ..Default::default()
        };

        let handler = BackpressureHandler::new(config);

        // Add 3 events (should drop oldest)
        for i in 0..3 {
            let event = FeedbackEvent::UserFeedback(UserFeedbackEvent::new(
                &format!("session{}", i),
                Uuid::new_v4(),
            ));
            handler.drop_oldest_and_add(event).await;
        }

        let stats = handler.stats();
        assert_eq!(stats.total_dropped_oldest, 1);
    }

    #[tokio::test]
    async fn test_drop_newest_strategy() {
        let config = BackpressureConfig {
            overflow_strategy: OverflowStrategy::DropNewest,
            ..Default::default()
        };

        let handler = BackpressureHandler::new(config);

        let event = FeedbackEvent::UserFeedback(UserFeedbackEvent::new("session1", Uuid::new_v4()));
        handler.drop_newest(event).await;

        let stats = handler.stats();
        assert_eq!(stats.total_dropped_newest, 1);
    }

    #[tokio::test]
    async fn test_reject_strategy() {
        let config = BackpressureConfig {
            overflow_strategy: OverflowStrategy::RejectNew,
            ..Default::default()
        };

        let handler = BackpressureHandler::new(config);

        let event = FeedbackEvent::UserFeedback(UserFeedbackEvent::new("session1", Uuid::new_v4()));
        let result = handler.handle_overflow(event).await;

        assert!(result.is_err());
        let stats = handler.stats();
        assert_eq!(stats.total_rejected, 1);
    }

    #[tokio::test]
    async fn test_persist_to_disk() {
        let temp_dir = TempDir::new().unwrap();
        let config = BackpressureConfig {
            overflow_strategy: OverflowStrategy::PersistToDisk,
            fallback_storage_path: Some(temp_dir.path().to_path_buf()),
            ..Default::default()
        };

        let handler = BackpressureHandler::new(config);
        handler.initialize_storage().await.unwrap();

        let event = FeedbackEvent::UserFeedback(UserFeedbackEvent::new("session1", Uuid::new_v4()));
        let event_id = event.id();

        handler.persist_to_disk(event.clone()).await.unwrap();

        let stats = handler.stats();
        assert_eq!(stats.total_persisted, 1);
        assert!(stats.disk_storage_bytes > 0);

        // Check file exists
        let file_path = temp_dir.path().join(format!("{}.json", event_id));
        assert!(file_path.exists());
    }

    #[tokio::test]
    async fn test_recover_from_disk() {
        let temp_dir = TempDir::new().unwrap();
        let config = BackpressureConfig {
            overflow_strategy: OverflowStrategy::PersistToDisk,
            fallback_storage_path: Some(temp_dir.path().to_path_buf()),
            ..Default::default()
        };

        let handler = BackpressureHandler::new(config);
        handler.initialize_storage().await.unwrap();

        // Persist multiple events
        let events: Vec<_> = (0..3)
            .map(|i| {
                FeedbackEvent::UserFeedback(UserFeedbackEvent::new(
                    &format!("session{}", i),
                    Uuid::new_v4(),
                ))
            })
            .collect();

        for event in &events {
            handler.persist_to_disk(event.clone()).await.unwrap();
        }

        // Recover events
        let recovered = handler.recover_from_disk().await.unwrap();
        assert_eq!(recovered.len(), 3);

        let stats = handler.stats();
        assert_eq!(stats.total_recovered, 3);
        assert_eq!(stats.disk_storage_bytes, 0); // Should be cleared after recovery
    }

    #[tokio::test]
    async fn test_buffer_size_tracking() {
        let config = BackpressureConfig {
            buffer_capacity: 100,
            warning_threshold_percent: 80,
            ..Default::default()
        };

        let handler = BackpressureHandler::new(config);

        handler.update_buffer_size(50);
        let stats = handler.stats();
        assert_eq!(stats.current_buffer_size, 50);
        assert_eq!(stats.buffer_level, "normal");

        handler.update_buffer_size(85);
        let stats = handler.stats();
        assert_eq!(stats.buffer_level, "warning");
    }

    #[tokio::test]
    async fn test_cleanup_storage() {
        let temp_dir = TempDir::new().unwrap();
        let config = BackpressureConfig {
            overflow_strategy: OverflowStrategy::PersistToDisk,
            fallback_storage_path: Some(temp_dir.path().to_path_buf()),
            ..Default::default()
        };

        let handler = BackpressureHandler::new(config);
        handler.initialize_storage().await.unwrap();

        // Persist event
        let event = FeedbackEvent::UserFeedback(UserFeedbackEvent::new("session1", Uuid::new_v4()));
        handler.persist_to_disk(event).await.unwrap();

        // Cleanup
        let cleaned = handler.cleanup_storage().await.unwrap();
        assert!(cleaned > 0);

        let stats = handler.stats();
        assert_eq!(stats.disk_storage_bytes, 0);
    }
}
