//! Kafka offset management and checkpointing
//!
//! This module provides comprehensive offset tracking, persistence, and recovery
//! capabilities for Kafka consumers. It integrates with the state backend system
//! to ensure exactly-once processing semantics and fault tolerance.
//!
//! # Features
//!
//! - **Per-partition offset tracking**: Track offsets independently for each partition
//! - **Watermark-based commits**: Commit offsets only after watermark passes
//! - **Checkpoint integration**: Save offsets as part of state checkpoints
//! - **Offset lag monitoring**: Calculate and track consumer lag
//! - **Reset strategies**: Support for earliest, latest, and specific offset resets
//! - **Automatic commit strategies**: Periodic, watermark-based, or manual commits
//!
//! # Example
//!
//! ```rust,no_run
//! use processor::kafka::{OffsetManager, OffsetCommitStrategy, InMemoryOffsetStore};
//! use processor::state::MemoryStateBackend;
//! use std::sync::Arc;
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let backend = Arc::new(MemoryStateBackend::new(None));
//!     let store = Arc::new(InMemoryOffsetStore::new());
//!
//!     let mut manager = OffsetManager::new(
//!         "my-consumer-group".to_string(),
//!         backend,
//!         store,
//!         OffsetCommitStrategy::Periodic {
//!             interval: Duration::from_secs(5),
//!         },
//!     );
//!
//!     // Update offset after processing event
//!     manager.update_offset("my-topic", 0, 100).await?;
//!
//!     // Commit offsets
//!     manager.commit().await?;
//!
//!     // Check lag
//!     let lag = manager.lag("my-topic", 0, 200).await?;
//!     println!("Current lag: {}", lag);
//!
//!     Ok(())
//! }
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};

use crate::error::{StateError, StateResult};
use crate::state::StateBackend;
use crate::watermark::Watermark;

/// Represents a topic-partition pair
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct TopicPartition {
    /// Kafka topic name
    pub topic: String,
    /// Partition number
    pub partition: u32,
}

impl TopicPartition {
    /// Create a new TopicPartition
    pub fn new(topic: String, partition: u32) -> Self {
        Self { topic, partition }
    }

    /// Convert to string key for storage
    pub fn to_key(&self) -> String {
        format!("{}:{}", self.topic, self.partition)
    }
}

impl std::fmt::Display for TopicPartition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.topic, self.partition)
    }
}

/// Offset information for a partition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OffsetInfo {
    /// Current committed offset
    pub offset: i64,
    /// High watermark (latest available offset in partition)
    pub high_watermark: Option<i64>,
    /// Timestamp when offset was last updated
    pub last_updated: DateTime<Utc>,
    /// Metadata associated with this offset
    pub metadata: Option<String>,
}

impl OffsetInfo {
    /// Create new offset info
    pub fn new(offset: i64) -> Self {
        Self {
            offset,
            high_watermark: None,
            last_updated: Utc::now(),
            metadata: None,
        }
    }

    /// Create with high watermark
    pub fn with_high_watermark(offset: i64, high_watermark: i64) -> Self {
        Self {
            offset,
            high_watermark: Some(high_watermark),
            last_updated: Utc::now(),
            metadata: None,
        }
    }

    /// Calculate lag (difference between high watermark and current offset)
    pub fn lag(&self) -> Option<i64> {
        self.high_watermark.map(|hwm| hwm - self.offset)
    }
}

/// Strategy for committing offsets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OffsetCommitStrategy {
    /// Commit periodically at fixed intervals
    Periodic {
        /// Interval between commits
        interval: Duration,
    },
    /// Commit after processing N events
    EveryNEvents {
        /// Number of events between commits
        n: usize,
    },
    /// Commit when watermark advances
    OnWatermark,
    /// Manual commit only (application controls)
    Manual,
}

impl Default for OffsetCommitStrategy {
    fn default() -> Self {
        Self::Periodic {
            interval: Duration::from_secs(5),
        }
    }
}

/// Reset strategy for partition offsets
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OffsetResetStrategy {
    /// Reset to earliest available offset
    Earliest,
    /// Reset to latest available offset (end of partition)
    Latest,
    /// Reset to specific offset
    Specific(i64),
    /// Reset to specific timestamp
    Timestamp(i64),
}

/// Statistics about offset management
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OffsetStats {
    /// Total number of commits
    pub total_commits: u64,
    /// Number of successful commits
    pub successful_commits: u64,
    /// Number of failed commits
    pub failed_commits: u64,
    /// Total offsets committed
    pub total_offsets_committed: u64,
    /// Last commit timestamp
    pub last_commit_time: Option<DateTime<Utc>>,
    /// Average commit duration in milliseconds
    pub avg_commit_duration_ms: u64,
}

/// Trait for offset storage backends
///
/// Implementations can store offsets in memory, state backends,
/// external systems, or Kafka itself.
#[async_trait]
pub trait OffsetStore: Send + Sync {
    /// Store offset for a topic-partition
    ///
    /// # Arguments
    /// * `consumer_group` - Consumer group ID
    /// * `topic_partition` - Topic and partition
    /// * `offset_info` - Offset information to store
    ///
    /// # Returns
    /// * `Ok(())` - Offset stored successfully
    /// * `Err(StateError)` - Storage failed
    async fn store_offset(
        &self,
        consumer_group: &str,
        topic_partition: &TopicPartition,
        offset_info: &OffsetInfo,
    ) -> StateResult<()>;

    /// Retrieve offset for a topic-partition
    ///
    /// # Arguments
    /// * `consumer_group` - Consumer group ID
    /// * `topic_partition` - Topic and partition
    ///
    /// # Returns
    /// * `Ok(Some(OffsetInfo))` - Offset found
    /// * `Ok(None)` - No offset stored
    /// * `Err(StateError)` - Retrieval failed
    async fn get_offset(
        &self,
        consumer_group: &str,
        topic_partition: &TopicPartition,
    ) -> StateResult<Option<OffsetInfo>>;

    /// Get all offsets for a consumer group
    ///
    /// # Arguments
    /// * `consumer_group` - Consumer group ID
    ///
    /// # Returns
    /// * `Ok(HashMap)` - Map of topic-partitions to offset info
    /// * `Err(StateError)` - Retrieval failed
    async fn get_all_offsets(
        &self,
        consumer_group: &str,
    ) -> StateResult<HashMap<TopicPartition, OffsetInfo>>;

    /// Delete offset for a topic-partition
    ///
    /// # Arguments
    /// * `consumer_group` - Consumer group ID
    /// * `topic_partition` - Topic and partition
    ///
    /// # Returns
    /// * `Ok(())` - Offset deleted
    /// * `Err(StateError)` - Deletion failed
    async fn delete_offset(
        &self,
        consumer_group: &str,
        topic_partition: &TopicPartition,
    ) -> StateResult<()>;

    /// Clear all offsets for a consumer group
    ///
    /// # Arguments
    /// * `consumer_group` - Consumer group ID
    ///
    /// # Returns
    /// * `Ok(())` - All offsets cleared
    /// * `Err(StateError)` - Clear failed
    async fn clear_offsets(&self, consumer_group: &str) -> StateResult<()>;
}

/// In-memory offset store implementation
///
/// Stores offsets in memory using a concurrent hash map.
/// Suitable for testing and development.
pub struct InMemoryOffsetStore {
    offsets: Arc<DashMap<String, HashMap<TopicPartition, OffsetInfo>>>,
}

impl InMemoryOffsetStore {
    /// Create a new in-memory offset store
    pub fn new() -> Self {
        Self {
            offsets: Arc::new(DashMap::new()),
        }
    }
}

impl Default for InMemoryOffsetStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl OffsetStore for InMemoryOffsetStore {
    async fn store_offset(
        &self,
        consumer_group: &str,
        topic_partition: &TopicPartition,
        offset_info: &OffsetInfo,
    ) -> StateResult<()> {
        self.offsets
            .entry(consumer_group.to_string())
            .or_insert_with(HashMap::new)
            .insert(topic_partition.clone(), offset_info.clone());
        Ok(())
    }

    async fn get_offset(
        &self,
        consumer_group: &str,
        topic_partition: &TopicPartition,
    ) -> StateResult<Option<OffsetInfo>> {
        Ok(self
            .offsets
            .get(consumer_group)
            .and_then(|offsets| offsets.get(topic_partition).cloned()))
    }

    async fn get_all_offsets(
        &self,
        consumer_group: &str,
    ) -> StateResult<HashMap<TopicPartition, OffsetInfo>> {
        Ok(self
            .offsets
            .get(consumer_group)
            .map(|offsets| offsets.clone())
            .unwrap_or_default())
    }

    async fn delete_offset(
        &self,
        consumer_group: &str,
        topic_partition: &TopicPartition,
    ) -> StateResult<()> {
        if let Some(mut offsets) = self.offsets.get_mut(consumer_group) {
            offsets.remove(topic_partition);
        }
        Ok(())
    }

    async fn clear_offsets(&self, consumer_group: &str) -> StateResult<()> {
        self.offsets.remove(consumer_group);
        Ok(())
    }
}

/// State backend offset store implementation
///
/// Stores offsets in a state backend for durability and recovery.
pub struct StateBackendOffsetStore<B: StateBackend> {
    backend: Arc<B>,
    prefix: String,
}

impl<B: StateBackend> StateBackendOffsetStore<B> {
    /// Create a new state backend offset store
    ///
    /// # Arguments
    /// * `backend` - State backend to use
    /// * `prefix` - Key prefix for offset storage (default: "kafka_offset:")
    pub fn new(backend: Arc<B>) -> Self {
        Self::with_prefix(backend, "kafka_offset:".to_string())
    }

    /// Create with custom prefix
    pub fn with_prefix(backend: Arc<B>, prefix: String) -> Self {
        Self { backend, prefix }
    }

    /// Build storage key
    fn build_key(&self, consumer_group: &str, topic_partition: &TopicPartition) -> Vec<u8> {
        format!(
            "{}{}:{}",
            self.prefix,
            consumer_group,
            topic_partition.to_key()
        )
        .into_bytes()
    }

    /// Build prefix for consumer group
    fn build_group_prefix(&self, consumer_group: &str) -> Vec<u8> {
        format!("{}{}:", self.prefix, consumer_group).into_bytes()
    }
}

#[async_trait]
impl<B: StateBackend> OffsetStore for StateBackendOffsetStore<B> {
    async fn store_offset(
        &self,
        consumer_group: &str,
        topic_partition: &TopicPartition,
        offset_info: &OffsetInfo,
    ) -> StateResult<()> {
        let key = self.build_key(consumer_group, topic_partition);
        let value = bincode::serialize(offset_info).map_err(|e| {
            StateError::SerializationFailed {
                key: String::from_utf8_lossy(&key).to_string(),
                reason: e.to_string(),
            }
        })?;

        self.backend.put(&key, &value).await
    }

    async fn get_offset(
        &self,
        consumer_group: &str,
        topic_partition: &TopicPartition,
    ) -> StateResult<Option<OffsetInfo>> {
        let key = self.build_key(consumer_group, topic_partition);

        match self.backend.get(&key).await? {
            Some(value) => {
                let offset_info = bincode::deserialize(&value).map_err(|e| {
                    StateError::DeserializationFailed {
                        key: String::from_utf8_lossy(&key).to_string(),
                        reason: e.to_string(),
                    }
                })?;
                Ok(Some(offset_info))
            }
            None => Ok(None),
        }
    }

    async fn get_all_offsets(
        &self,
        consumer_group: &str,
    ) -> StateResult<HashMap<TopicPartition, OffsetInfo>> {
        let prefix = self.build_group_prefix(consumer_group);
        let keys = self.backend.list_keys(&prefix).await?;

        let mut offsets = HashMap::new();

        for key in keys {
            if let Some(value) = self.backend.get(&key).await? {
                if let Ok(offset_info) = bincode::deserialize::<OffsetInfo>(&value) {
                    // Extract topic-partition from key
                    let key_str = String::from_utf8_lossy(&key);
                    if let Some(tp_str) = key_str.strip_prefix(&format!("{}{}:", self.prefix, consumer_group)) {
                        if let Some((topic, partition_str)) = tp_str.rsplit_once(':') {
                            if let Ok(partition) = partition_str.parse::<u32>() {
                                let tp = TopicPartition::new(topic.to_string(), partition);
                                offsets.insert(tp, offset_info);
                            }
                        }
                    }
                }
            }
        }

        Ok(offsets)
    }

    async fn delete_offset(
        &self,
        consumer_group: &str,
        topic_partition: &TopicPartition,
    ) -> StateResult<()> {
        let key = self.build_key(consumer_group, topic_partition);
        self.backend.delete(&key).await
    }

    async fn clear_offsets(&self, consumer_group: &str) -> StateResult<()> {
        let prefix = self.build_group_prefix(consumer_group);
        let keys = self.backend.list_keys(&prefix).await?;

        for key in keys {
            self.backend.delete(&key).await?;
        }

        Ok(())
    }
}

/// Main offset manager
///
/// Coordinates offset tracking, committing, and recovery for Kafka consumers.
///
/// # Features
///
/// - Per-partition offset tracking
/// - Watermark-based commit strategies
/// - Checkpoint integration
/// - Offset lag monitoring
/// - Automatic and manual commit modes
pub struct OffsetManager<B: StateBackend, S: OffsetStore> {
    /// Consumer group ID
    consumer_group: String,
    /// State backend for checkpointing
    backend: Arc<B>,
    /// Offset storage
    store: Arc<S>,
    /// Commit strategy
    strategy: OffsetCommitStrategy,
    /// Pending offsets (not yet committed)
    pending_offsets: Arc<DashMap<TopicPartition, OffsetInfo>>,
    /// Committed offsets
    committed_offsets: Arc<DashMap<TopicPartition, OffsetInfo>>,
    /// Statistics
    stats: Arc<RwLock<OffsetStats>>,
    /// Last commit time
    last_commit: Arc<Mutex<DateTime<Utc>>>,
    /// Event counter for EveryNEvents strategy
    event_counter: Arc<Mutex<usize>>,
    /// Watermark tracker for OnWatermark strategy
    last_watermark: Arc<Mutex<Option<Watermark>>>,
}

impl<B: StateBackend, S: OffsetStore> OffsetManager<B, S> {
    /// Create a new offset manager
    ///
    /// # Arguments
    /// * `consumer_group` - Consumer group ID
    /// * `backend` - State backend for checkpointing
    /// * `store` - Offset storage implementation
    /// * `strategy` - Commit strategy
    pub fn new(
        consumer_group: String,
        backend: Arc<B>,
        store: Arc<S>,
        strategy: OffsetCommitStrategy,
    ) -> Self {
        Self {
            consumer_group,
            backend,
            store,
            strategy,
            pending_offsets: Arc::new(DashMap::new()),
            committed_offsets: Arc::new(DashMap::new()),
            stats: Arc::new(RwLock::new(OffsetStats::default())),
            last_commit: Arc::new(Mutex::new(Utc::now())),
            event_counter: Arc::new(Mutex::new(0)),
            last_watermark: Arc::new(Mutex::new(None)),
        }
    }

    /// Update offset for a partition
    ///
    /// # Arguments
    /// * `topic` - Kafka topic
    /// * `partition` - Partition number
    /// * `offset` - New offset value
    ///
    /// # Returns
    /// * `Ok(())` - Offset updated
    /// * `Err(StateError)` - Update failed
    pub async fn update_offset(
        &mut self,
        topic: &str,
        partition: u32,
        offset: i64,
    ) -> StateResult<()> {
        let tp = TopicPartition::new(topic.to_string(), partition);
        let offset_info = OffsetInfo::new(offset);

        self.pending_offsets.insert(tp.clone(), offset_info);

        debug!(
            consumer_group = %self.consumer_group,
            topic = topic,
            partition = partition,
            offset = offset,
            "Updated offset"
        );

        // Check if we should auto-commit
        if self.should_auto_commit().await {
            self.commit().await?;
        }

        Ok(())
    }

    /// Update offset with high watermark
    pub async fn update_offset_with_watermark(
        &mut self,
        topic: &str,
        partition: u32,
        offset: i64,
        high_watermark: i64,
    ) -> StateResult<()> {
        let tp = TopicPartition::new(topic.to_string(), partition);
        let offset_info = OffsetInfo::with_high_watermark(offset, high_watermark);

        self.pending_offsets.insert(tp.clone(), offset_info);

        debug!(
            consumer_group = %self.consumer_group,
            topic = topic,
            partition = partition,
            offset = offset,
            high_watermark = high_watermark,
            "Updated offset with high watermark"
        );

        // Check if we should auto-commit
        if self.should_auto_commit().await {
            self.commit().await?;
        }

        Ok(())
    }

    /// Get current offset for a partition
    ///
    /// Returns the committed offset, or None if no offset has been committed.
    pub async fn get_offset(&self, topic: &str, partition: u32) -> StateResult<Option<i64>> {
        let tp = TopicPartition::new(topic.to_string(), partition);

        // Check committed offsets first
        if let Some(info) = self.committed_offsets.get(&tp) {
            return Ok(Some(info.offset));
        }

        // Fall back to stored offsets
        if let Some(info) = self.store.get_offset(&self.consumer_group, &tp).await? {
            return Ok(Some(info.offset));
        }

        Ok(None)
    }

    /// Get offset info (includes metadata and watermarks)
    pub async fn get_offset_info(
        &self,
        topic: &str,
        partition: u32,
    ) -> StateResult<Option<OffsetInfo>> {
        let tp = TopicPartition::new(topic.to_string(), partition);

        // Check committed offsets first
        if let Some(info) = self.committed_offsets.get(&tp) {
            return Ok(Some(info.clone()));
        }

        // Fall back to stored offsets
        self.store.get_offset(&self.consumer_group, &tp).await
    }

    /// Commit all pending offsets
    ///
    /// # Returns
    /// * `Ok(committed_count)` - Number of offsets committed
    /// * `Err(StateError)` - Commit failed
    pub async fn commit(&mut self) -> StateResult<usize> {
        let start_time = Utc::now();
        let pending: Vec<_> = self
            .pending_offsets
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();

        if pending.is_empty() {
            debug!(consumer_group = %self.consumer_group, "No pending offsets to commit");
            return Ok(0);
        }

        info!(
            consumer_group = %self.consumer_group,
            count = pending.len(),
            "Committing offsets"
        );

        let mut stats = self.stats.write().await;
        stats.total_commits += 1;

        let mut committed_count = 0;
        let mut failed = false;

        for (tp, offset_info) in pending {
            match self
                .store
                .store_offset(&self.consumer_group, &tp, &offset_info)
                .await
            {
                Ok(()) => {
                    self.committed_offsets.insert(tp.clone(), offset_info.clone());
                    self.pending_offsets.remove(&tp);
                    committed_count += 1;
                    debug!(topic_partition = %tp, offset = offset_info.offset, "Committed offset");
                }
                Err(e) => {
                    error!(
                        topic_partition = %tp,
                        error = %e,
                        "Failed to commit offset"
                    );
                    failed = true;
                }
            }
        }

        // Update statistics
        if failed {
            stats.failed_commits += 1;
        } else {
            stats.successful_commits += 1;
        }
        stats.total_offsets_committed += committed_count as u64;
        stats.last_commit_time = Some(start_time);

        let duration = Utc::now().signed_duration_since(start_time);
        let duration_ms = duration.num_milliseconds() as u64;

        // Update rolling average
        if stats.avg_commit_duration_ms == 0 {
            stats.avg_commit_duration_ms = duration_ms;
        } else {
            stats.avg_commit_duration_ms =
                (stats.avg_commit_duration_ms * 9 + duration_ms) / 10;
        }

        drop(stats);

        *self.last_commit.lock().await = Utc::now();

        info!(
            consumer_group = %self.consumer_group,
            committed = committed_count,
            duration_ms = duration_ms,
            "Offset commit completed"
        );

        if failed {
            Err(StateError::TransactionFailed {
                operation: "offset_commit".to_string(),
                reason: format!("Failed to commit some offsets, {} succeeded", committed_count),
            })
        } else {
            Ok(committed_count)
        }
    }

    /// Calculate offset lag for a partition
    ///
    /// # Arguments
    /// * `topic` - Kafka topic
    /// * `partition` - Partition number
    /// * `high_watermark` - Current high watermark (latest offset in partition)
    ///
    /// # Returns
    /// * `Ok(lag)` - Offset lag (high_watermark - current_offset)
    /// * `Err(StateError)` - Calculation failed
    pub async fn lag(
        &self,
        topic: &str,
        partition: u32,
        high_watermark: i64,
    ) -> StateResult<i64> {
        let current_offset = self.get_offset(topic, partition).await?.unwrap_or(0);
        Ok(high_watermark - current_offset)
    }

    /// Get total lag across all partitions
    pub async fn total_lag(&self) -> StateResult<i64> {
        let mut total = 0i64;

        for entry in self.committed_offsets.iter() {
            if let Some(lag) = entry.value().lag() {
                total += lag;
            }
        }

        Ok(total)
    }

    /// Restore offsets from storage
    ///
    /// Loads all committed offsets for the consumer group from the offset store.
    ///
    /// # Returns
    /// * `Ok(restored_count)` - Number of offsets restored
    /// * `Err(StateError)` - Restore failed
    pub async fn restore(&mut self) -> StateResult<usize> {
        info!(consumer_group = %self.consumer_group, "Restoring offsets");

        let offsets = self.store.get_all_offsets(&self.consumer_group).await?;
        let count = offsets.len();

        for (tp, offset_info) in offsets {
            self.committed_offsets.insert(tp.clone(), offset_info);
            debug!(topic_partition = %tp, "Restored offset");
        }

        info!(
            consumer_group = %self.consumer_group,
            count = count,
            "Offset restore completed"
        );

        Ok(count)
    }

    /// Reset offset for a partition
    ///
    /// # Arguments
    /// * `topic` - Kafka topic
    /// * `partition` - Partition number
    /// * `strategy` - Reset strategy (earliest, latest, specific offset, etc.)
    /// * `partition_high_watermark` - Current high watermark for the partition
    ///
    /// # Returns
    /// * `Ok(new_offset)` - The new offset value
    /// * `Err(StateError)` - Reset failed
    pub async fn reset_offset(
        &mut self,
        topic: &str,
        partition: u32,
        strategy: OffsetResetStrategy,
        partition_high_watermark: Option<i64>,
    ) -> StateResult<i64> {
        let tp = TopicPartition::new(topic.to_string(), partition);

        let new_offset = match strategy {
            OffsetResetStrategy::Earliest => 0,
            OffsetResetStrategy::Latest => partition_high_watermark.unwrap_or(0),
            OffsetResetStrategy::Specific(offset) => offset,
            OffsetResetStrategy::Timestamp(ts) => {
                // In a real implementation, this would query Kafka for offset by timestamp
                // For now, we'll just use the timestamp value as offset
                warn!(
                    topic = topic,
                    partition = partition,
                    timestamp = ts,
                    "Timestamp-based reset not fully implemented, using timestamp as offset"
                );
                ts
            }
        };

        let offset_info = OffsetInfo::new(new_offset);
        self.store
            .store_offset(&self.consumer_group, &tp, &offset_info)
            .await?;
        self.committed_offsets.insert(tp.clone(), offset_info);
        self.pending_offsets.remove(&tp);

        info!(
            consumer_group = %self.consumer_group,
            topic = topic,
            partition = partition,
            new_offset = new_offset,
            strategy = ?strategy,
            "Reset offset"
        );

        Ok(new_offset)
    }

    /// Check if automatic commit should occur based on strategy
    async fn should_auto_commit(&mut self) -> bool {
        match &self.strategy {
            OffsetCommitStrategy::Periodic { interval } => {
                let last_commit = *self.last_commit.lock().await;
                let elapsed = Utc::now().signed_duration_since(last_commit);
                elapsed.num_milliseconds() as u64 >= interval.as_millis() as u64
            }
            OffsetCommitStrategy::EveryNEvents { n } => {
                let mut counter = self.event_counter.lock().await;
                *counter += 1;
                if *counter >= *n {
                    *counter = 0;
                    true
                } else {
                    false
                }
            }
            OffsetCommitStrategy::OnWatermark => false, // Handled externally via notify_watermark
            OffsetCommitStrategy::Manual => false,
        }
    }

    /// Notify manager of watermark advancement (for OnWatermark strategy)
    ///
    /// # Arguments
    /// * `watermark` - New watermark value
    ///
    /// # Returns
    /// * `Ok(committed)` - True if commit occurred
    /// * `Err(StateError)` - Notification failed
    pub async fn notify_watermark(&mut self, watermark: Watermark) -> StateResult<bool> {
        let mut last_wm = self.last_watermark.lock().await;

        if matches!(self.strategy, OffsetCommitStrategy::OnWatermark) {
            if last_wm.is_none() || watermark > last_wm.unwrap() {
                *last_wm = Some(watermark);
                drop(last_wm);

                self.commit().await?;
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Get statistics
    pub async fn stats(&self) -> OffsetStats {
        self.stats.read().await.clone()
    }

    /// Get all committed offsets
    pub async fn get_all_committed(&self) -> HashMap<TopicPartition, OffsetInfo> {
        self.committed_offsets
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    /// Get all pending offsets (not yet committed)
    pub async fn get_all_pending(&self) -> HashMap<TopicPartition, OffsetInfo> {
        self.pending_offsets
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    /// Clear all offsets for this consumer group
    pub async fn clear(&mut self) -> StateResult<()> {
        info!(consumer_group = %self.consumer_group, "Clearing all offsets");

        self.store.clear_offsets(&self.consumer_group).await?;
        self.committed_offsets.clear();
        self.pending_offsets.clear();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::MemoryStateBackend;

    #[test]
    fn test_topic_partition() {
        let tp = TopicPartition::new("test-topic".to_string(), 5);
        assert_eq!(tp.topic, "test-topic");
        assert_eq!(tp.partition, 5);
        assert_eq!(tp.to_key(), "test-topic:5");
        assert_eq!(tp.to_string(), "test-topic:5");
    }

    #[test]
    fn test_offset_info_lag() {
        let info = OffsetInfo::with_high_watermark(100, 200);
        assert_eq!(info.lag(), Some(100));

        let info_no_hwm = OffsetInfo::new(100);
        assert_eq!(info_no_hwm.lag(), None);
    }

    #[tokio::test]
    async fn test_in_memory_offset_store() {
        let store = InMemoryOffsetStore::new();
        let tp = TopicPartition::new("test".to_string(), 0);
        let offset_info = OffsetInfo::new(100);

        // Store offset
        store
            .store_offset("group1", &tp, &offset_info)
            .await
            .unwrap();

        // Retrieve offset
        let retrieved = store.get_offset("group1", &tp).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().offset, 100);

        // Get all offsets
        let all = store.get_all_offsets("group1").await.unwrap();
        assert_eq!(all.len(), 1);

        // Delete offset
        store.delete_offset("group1", &tp).await.unwrap();
        let retrieved = store.get_offset("group1", &tp).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_state_backend_offset_store() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let store = StateBackendOffsetStore::new(backend.clone());
        let tp = TopicPartition::new("test-topic".to_string(), 3);
        let offset_info = OffsetInfo::new(500);

        // Store offset
        store
            .store_offset("group1", &tp, &offset_info)
            .await
            .unwrap();

        // Retrieve offset
        let retrieved = store.get_offset("group1", &tp).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().offset, 500);

        // Get all offsets
        let all = store.get_all_offsets("group1").await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all.get(&tp).unwrap().offset, 500);

        // Clear offsets
        store.clear_offsets("group1").await.unwrap();
        let all = store.get_all_offsets("group1").await.unwrap();
        assert_eq!(all.len(), 0);
    }

    #[tokio::test]
    async fn test_offset_manager_update_and_commit() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let store = Arc::new(InMemoryOffsetStore::new());
        let mut manager = OffsetManager::new(
            "test-group".to_string(),
            backend,
            store.clone(),
            OffsetCommitStrategy::Manual,
        );

        // Update offset
        manager.update_offset("topic1", 0, 100).await.unwrap();

        // Offset should be pending
        let pending = manager.get_all_pending().await;
        assert_eq!(pending.len(), 1);

        // Commit
        let committed = manager.commit().await.unwrap();
        assert_eq!(committed, 1);

        // Should be in committed offsets now
        let offset = manager.get_offset("topic1", 0).await.unwrap();
        assert_eq!(offset, Some(100));

        // Pending should be empty
        let pending = manager.get_all_pending().await;
        assert_eq!(pending.len(), 0);
    }

    #[tokio::test]
    async fn test_offset_manager_lag() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let store = Arc::new(InMemoryOffsetStore::new());
        let mut manager = OffsetManager::new(
            "test-group".to_string(),
            backend,
            store,
            OffsetCommitStrategy::Manual,
        );

        manager.update_offset("topic1", 0, 100).await.unwrap();
        manager.commit().await.unwrap();

        let lag = manager.lag("topic1", 0, 200).await.unwrap();
        assert_eq!(lag, 100);
    }

    #[tokio::test]
    async fn test_offset_manager_restore() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let store = Arc::new(InMemoryOffsetStore::new());

        // Create first manager and commit some offsets
        let mut manager1 = OffsetManager::new(
            "test-group".to_string(),
            backend.clone(),
            store.clone(),
            OffsetCommitStrategy::Manual,
        );

        manager1.update_offset("topic1", 0, 100).await.unwrap();
        manager1.update_offset("topic1", 1, 200).await.unwrap();
        manager1.commit().await.unwrap();

        // Create second manager and restore
        let mut manager2 = OffsetManager::new(
            "test-group".to_string(),
            backend,
            store,
            OffsetCommitStrategy::Manual,
        );

        let restored = manager2.restore().await.unwrap();
        assert_eq!(restored, 2);

        assert_eq!(manager2.get_offset("topic1", 0).await.unwrap(), Some(100));
        assert_eq!(manager2.get_offset("topic1", 1).await.unwrap(), Some(200));
    }

    #[tokio::test]
    async fn test_offset_manager_reset() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let store = Arc::new(InMemoryOffsetStore::new());
        let mut manager = OffsetManager::new(
            "test-group".to_string(),
            backend,
            store,
            OffsetCommitStrategy::Manual,
        );

        manager.update_offset("topic1", 0, 100).await.unwrap();
        manager.commit().await.unwrap();

        // Reset to earliest
        let new_offset = manager
            .reset_offset("topic1", 0, OffsetResetStrategy::Earliest, None)
            .await
            .unwrap();
        assert_eq!(new_offset, 0);
        assert_eq!(manager.get_offset("topic1", 0).await.unwrap(), Some(0));

        // Reset to latest
        let new_offset = manager
            .reset_offset("topic1", 0, OffsetResetStrategy::Latest, Some(500))
            .await
            .unwrap();
        assert_eq!(new_offset, 500);
        assert_eq!(manager.get_offset("topic1", 0).await.unwrap(), Some(500));

        // Reset to specific
        let new_offset = manager
            .reset_offset("topic1", 0, OffsetResetStrategy::Specific(250), None)
            .await
            .unwrap();
        assert_eq!(new_offset, 250);
        assert_eq!(manager.get_offset("topic1", 0).await.unwrap(), Some(250));
    }

    #[tokio::test]
    async fn test_offset_manager_periodic_strategy() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let store = Arc::new(InMemoryOffsetStore::new());
        let mut manager = OffsetManager::new(
            "test-group".to_string(),
            backend,
            store,
            OffsetCommitStrategy::Periodic {
                interval: Duration::from_millis(100),
            },
        );

        // First update - should not auto-commit yet
        manager.update_offset("topic1", 0, 100).await.unwrap();
        assert_eq!(manager.get_all_pending().await.len(), 1);

        // Wait for interval
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Second update - should auto-commit
        manager.update_offset("topic1", 0, 200).await.unwrap();

        // Previous offset should be committed
        assert_eq!(manager.get_offset("topic1", 0).await.unwrap(), Some(200));
    }

    #[tokio::test]
    async fn test_offset_manager_watermark_strategy() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let store = Arc::new(InMemoryOffsetStore::new());
        let mut manager = OffsetManager::new(
            "test-group".to_string(),
            backend,
            store,
            OffsetCommitStrategy::OnWatermark,
        );

        manager.update_offset("topic1", 0, 100).await.unwrap();
        assert_eq!(manager.get_all_pending().await.len(), 1);

        // Notify watermark advancement
        let watermark = Watermark::new(1000);
        let committed = manager.notify_watermark(watermark).await.unwrap();
        assert!(committed);

        // Should be committed now
        assert_eq!(manager.get_offset("topic1", 0).await.unwrap(), Some(100));
        assert_eq!(manager.get_all_pending().await.len(), 0);
    }

    #[tokio::test]
    async fn test_offset_manager_stats() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let store = Arc::new(InMemoryOffsetStore::new());
        let mut manager = OffsetManager::new(
            "test-group".to_string(),
            backend,
            store,
            OffsetCommitStrategy::Manual,
        );

        let stats = manager.stats().await;
        assert_eq!(stats.total_commits, 0);

        manager.update_offset("topic1", 0, 100).await.unwrap();
        manager.commit().await.unwrap();

        let stats = manager.stats().await;
        assert_eq!(stats.total_commits, 1);
        assert_eq!(stats.successful_commits, 1);
        assert_eq!(stats.total_offsets_committed, 1);
        assert!(stats.last_commit_time.is_some());
    }

    #[tokio::test]
    async fn test_offset_manager_multiple_partitions() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let store = Arc::new(InMemoryOffsetStore::new());
        let mut manager = OffsetManager::new(
            "test-group".to_string(),
            backend,
            store,
            OffsetCommitStrategy::Manual,
        );

        // Update multiple partitions
        manager.update_offset("topic1", 0, 100).await.unwrap();
        manager.update_offset("topic1", 1, 200).await.unwrap();
        manager.update_offset("topic2", 0, 300).await.unwrap();

        let pending = manager.get_all_pending().await;
        assert_eq!(pending.len(), 3);

        // Commit all
        let committed = manager.commit().await.unwrap();
        assert_eq!(committed, 3);

        // Verify all committed
        assert_eq!(manager.get_offset("topic1", 0).await.unwrap(), Some(100));
        assert_eq!(manager.get_offset("topic1", 1).await.unwrap(), Some(200));
        assert_eq!(manager.get_offset("topic2", 0).await.unwrap(), Some(300));
    }

    #[tokio::test]
    async fn test_offset_manager_with_high_watermark() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let store = Arc::new(InMemoryOffsetStore::new());
        let mut manager = OffsetManager::new(
            "test-group".to_string(),
            backend,
            store,
            OffsetCommitStrategy::Manual,
        );

        manager
            .update_offset_with_watermark("topic1", 0, 100, 200)
            .await
            .unwrap();
        manager.commit().await.unwrap();

        let info = manager.get_offset_info("topic1", 0).await.unwrap();
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.offset, 100);
        assert_eq!(info.high_watermark, Some(200));
        assert_eq!(info.lag(), Some(100));

        let total_lag = manager.total_lag().await.unwrap();
        assert_eq!(total_lag, 100);
    }
}
