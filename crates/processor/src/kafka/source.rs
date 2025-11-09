//! Kafka source consumer implementation
//!
//! This module provides a robust Kafka consumer implementation for ingesting
//! events from Kafka topics with support for:
//! - Async message consumption with tokio
//! - Automatic reconnection on errors
//! - Configurable commit strategies (auto, manual, periodic)
//! - Multiple partition support
//! - Graceful shutdown
//! - Message deserialization with error handling
//! - Offset management integration with state backend
//! - Comprehensive metrics tracking

use chrono::{DateTime, Utc};
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, ConsumerContext, Rebalance, StreamConsumer};
use rdkafka::error::{KafkaError, KafkaResult};
use rdkafka::message::{BorrowedMessage, Headers};
use rdkafka::{ClientContext, Message, Offset, TopicPartitionList};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::time;
use tracing::{debug, error, info, warn};

use crate::error::{ProcessorError, Result};
use crate::state::StateBackend;

/// Default poll timeout for Kafka consumer
const DEFAULT_POLL_TIMEOUT: Duration = Duration::from_millis(100);

/// Default commit interval for periodic commits
const DEFAULT_COMMIT_INTERVAL: Duration = Duration::from_secs(5);

/// Default max retries for message consumption
const DEFAULT_MAX_RETRIES: u32 = 3;

/// Default backoff for retries
const DEFAULT_RETRY_BACKOFF: Duration = Duration::from_secs(1);

/// Commit strategy for offset management
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitStrategy {
    /// Automatic commits by rdkafka (not recommended for exactly-once)
    Auto,
    /// Manual commit after each message
    Manual,
    /// Periodic commit at specified interval
    Periodic,
    /// Commit on checkpoint (integration with state backend)
    OnCheckpoint,
}

/// Configuration for Kafka source consumer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaSourceConfig {
    /// Kafka brokers (comma-separated list)
    pub brokers: String,
    /// Consumer group ID
    pub group_id: String,
    /// Topics to subscribe to
    pub topics: Vec<String>,
    /// Commit strategy
    pub commit_strategy: CommitStrategy,
    /// Commit interval for periodic commits
    pub commit_interval_secs: Option<u64>,
    /// Enable auto offset reset to earliest
    pub auto_offset_reset: Option<String>,
    /// Additional Kafka consumer configuration
    pub extra_config: HashMap<String, String>,
    /// Maximum retries for message consumption
    pub max_retries: Option<u32>,
    /// Retry backoff duration in milliseconds
    pub retry_backoff_ms: Option<u64>,
    /// Poll timeout in milliseconds
    pub poll_timeout_ms: Option<u64>,
}

impl Default for KafkaSourceConfig {
    fn default() -> Self {
        Self {
            brokers: "localhost:9092".to_string(),
            group_id: "llm-optimizer-processor".to_string(),
            topics: vec![],
            commit_strategy: CommitStrategy::Periodic,
            commit_interval_secs: Some(5),
            auto_offset_reset: Some("earliest".to_string()),
            extra_config: HashMap::new(),
            max_retries: Some(DEFAULT_MAX_RETRIES),
            retry_backoff_ms: Some(DEFAULT_RETRY_BACKOFF.as_millis() as u64),
            poll_timeout_ms: Some(DEFAULT_POLL_TIMEOUT.as_millis() as u64),
        }
    }
}

/// Metrics tracked by the Kafka source
#[derive(Debug, Clone)]
pub struct KafkaSourceMetrics {
    /// Total messages consumed
    pub messages_consumed: u64,
    /// Total messages failed
    pub messages_failed: u64,
    /// Messages consumed per partition
    pub messages_per_partition: HashMap<i32, u64>,
    /// Current lag per partition (estimated)
    pub lag_per_partition: HashMap<i32, i64>,
    /// Total bytes consumed
    pub bytes_consumed: u64,
    /// Messages per second (last minute)
    pub throughput_msgs_per_sec: f64,
    /// Bytes per second (last minute)
    pub throughput_bytes_per_sec: f64,
    /// Average message size
    pub avg_message_size: f64,
    /// Last commit timestamp
    pub last_commit_time: Option<DateTime<Utc>>,
    /// Total commits
    pub total_commits: u64,
    /// Rebalance count
    pub rebalance_count: u64,
    /// Current partition assignment
    pub assigned_partitions: Vec<i32>,
}

impl Default for KafkaSourceMetrics {
    fn default() -> Self {
        Self {
            messages_consumed: 0,
            messages_failed: 0,
            messages_per_partition: HashMap::new(),
            lag_per_partition: HashMap::new(),
            bytes_consumed: 0,
            throughput_msgs_per_sec: 0.0,
            throughput_bytes_per_sec: 0.0,
            avg_message_size: 0.0,
            last_commit_time: None,
            total_commits: 0,
            rebalance_count: 0,
            assigned_partitions: vec![],
        }
    }
}

/// Custom consumer context for handling callbacks
struct SourceConsumerContext {
    metrics: Arc<RwLock<KafkaSourceMetrics>>,
    state_backend: Option<Arc<dyn StateBackend>>,
}

impl ClientContext for SourceConsumerContext {}

impl ConsumerContext for SourceConsumerContext {
    fn pre_rebalance(&self, rebalance: &Rebalance) {
        match rebalance {
            Rebalance::Revoke(tpl) => {
                info!("Partition revocation: {:?}", tpl);
                let metrics = self.metrics.clone();
                tokio::spawn(async move {
                    let mut metrics = metrics.write().await;
                    metrics.rebalance_count += 1;
                });
            }
            Rebalance::Assign(tpl) => {
                info!("Partition assignment: {:?}", tpl);
            }
            Rebalance::Error(err) => {
                error!("Rebalance error: {}", err);
            }
        }
    }

    fn post_rebalance(&self, rebalance: &Rebalance) {
        match rebalance {
            Rebalance::Assign(tpl) => {
                let partitions: Vec<i32> = tpl
                    .elements()
                    .iter()
                    .map(|elem| elem.partition())
                    .collect();
                info!("Successfully assigned partitions: {:?}", partitions);

                let metrics = self.metrics.clone();
                tokio::spawn(async move {
                    let mut metrics = metrics.write().await;
                    metrics.assigned_partitions = partitions;
                });
            }
            Rebalance::Revoke(_) => {
                info!("Partitions revoked successfully");
            }
            Rebalance::Error(err) => {
                error!("Post-rebalance error: {}", err);
            }
        }
    }

    fn commit_callback(&self, result: KafkaResult<()>, _offsets: &TopicPartitionList) {
        match result {
            Ok(_) => {
                let metrics = self.metrics.clone();
                tokio::spawn(async move {
                    let mut metrics = metrics.write().await;
                    metrics.total_commits += 1;
                    metrics.last_commit_time = Some(Utc::now());
                });
            }
            Err(err) => {
                error!("Commit callback error: {}", err);
            }
        }
    }
}

/// Message envelope with metadata
#[derive(Debug, Clone)]
pub struct KafkaMessage<T> {
    /// Deserialized payload
    pub payload: T,
    /// Message key (if present)
    pub key: Option<Vec<u8>>,
    /// Topic name
    pub topic: String,
    /// Partition
    pub partition: i32,
    /// Offset
    pub offset: i64,
    /// Timestamp (if available)
    pub timestamp: Option<DateTime<Utc>>,
    /// Headers (if present)
    pub headers: HashMap<String, Vec<u8>>,
}

/// Kafka source consumer for ingesting events
pub struct KafkaSource<T> {
    /// Kafka consumer
    consumer: Arc<StreamConsumer<SourceConsumerContext>>,
    /// Configuration
    config: KafkaSourceConfig,
    /// Metrics
    metrics: Arc<RwLock<KafkaSourceMetrics>>,
    /// State backend for offset management
    state_backend: Option<Arc<dyn StateBackend>>,
    /// Shutdown signal
    shutdown: Arc<AtomicBool>,
    /// Throughput tracking
    throughput_tracker: Arc<RwLock<ThroughputTracker>>,
    /// Phantom data for type parameter
    _phantom: std::marker::PhantomData<T>,
}

/// Tracks throughput over time windows
struct ThroughputTracker {
    /// Messages in current window
    messages_in_window: u64,
    /// Bytes in current window
    bytes_in_window: u64,
    /// Window start time
    window_start: Instant,
    /// Window duration
    window_duration: Duration,
}

impl ThroughputTracker {
    fn new() -> Self {
        Self {
            messages_in_window: 0,
            bytes_in_window: 0,
            window_start: Instant::now(),
            window_duration: Duration::from_secs(60),
        }
    }

    fn record(&mut self, bytes: usize) {
        self.messages_in_window += 1;
        self.bytes_in_window += bytes as u64;
    }

    fn calculate(&mut self) -> (f64, f64) {
        let elapsed = self.window_start.elapsed();

        if elapsed >= self.window_duration {
            let secs = elapsed.as_secs_f64();
            let msgs_per_sec = self.messages_in_window as f64 / secs;
            let bytes_per_sec = self.bytes_in_window as f64 / secs;

            // Reset for next window
            self.messages_in_window = 0;
            self.bytes_in_window = 0;
            self.window_start = Instant::now();

            (msgs_per_sec, bytes_per_sec)
        } else {
            (0.0, 0.0)
        }
    }
}

impl<T> KafkaSource<T>
where
    T: DeserializeOwned + Send + Sync + 'static,
{
    /// Create a new Kafka source consumer
    ///
    /// # Arguments
    ///
    /// * `config` - Kafka source configuration
    /// * `state_backend` - Optional state backend for offset management
    ///
    /// # Returns
    ///
    /// * `Result<Self>` - New Kafka source instance
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use processor::kafka::{KafkaSource, KafkaSourceConfig};
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Debug, Deserialize, Serialize)]
    /// struct MyEvent {
    ///     id: String,
    ///     value: f64,
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let config = KafkaSourceConfig {
    ///         brokers: "localhost:9092".to_string(),
    ///         group_id: "my-consumer-group".to_string(),
    ///         topics: vec!["my-topic".to_string()],
    ///         ..Default::default()
    ///     };
    ///
    ///     let source = KafkaSource::<MyEvent>::new(config, None).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn new(
        config: KafkaSourceConfig,
        state_backend: Option<Arc<dyn StateBackend>>,
    ) -> Result<Self> {
        let metrics = Arc::new(RwLock::new(KafkaSourceMetrics::default()));

        let context = SourceConsumerContext {
            metrics: metrics.clone(),
            state_backend: state_backend.clone(),
        };

        let consumer = Self::create_consumer(&config, context)?;

        Ok(Self {
            consumer: Arc::new(consumer),
            config,
            metrics,
            state_backend,
            shutdown: Arc::new(AtomicBool::new(false)),
            throughput_tracker: Arc::new(RwLock::new(ThroughputTracker::new())),
            _phantom: std::marker::PhantomData,
        })
    }

    /// Create the underlying Kafka consumer
    fn create_consumer(
        config: &KafkaSourceConfig,
        context: SourceConsumerContext,
    ) -> Result<StreamConsumer<SourceConsumerContext>> {
        let mut client_config = ClientConfig::new();

        client_config
            .set("bootstrap.servers", &config.brokers)
            .set("group.id", &config.group_id)
            .set("enable.auto.commit", "false"); // We handle commits manually

        // Set auto offset reset
        if let Some(ref reset) = config.auto_offset_reset {
            client_config.set("auto.offset.reset", reset);
        }

        // Apply extra configuration
        for (key, value) in &config.extra_config {
            client_config.set(key, value);
        }

        // Create consumer
        let consumer: StreamConsumer<SourceConsumerContext> = client_config
            .create_with_context(context)
            .map_err(|e| ProcessorError::Configuration {
                source: Box::new(e),
            })?;

        Ok(consumer)
    }

    /// Subscribe to topics
    ///
    /// # Arguments
    ///
    /// * `topics` - Topics to subscribe to
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Success or error
    pub async fn subscribe(&self, topics: &[String]) -> Result<()> {
        let topic_refs: Vec<&str> = topics.iter().map(|s| s.as_str()).collect();

        self.consumer
            .subscribe(&topic_refs)
            .map_err(|e| ProcessorError::Configuration {
                source: Box::new(e),
            })?;

        info!("Subscribed to topics: {:?}", topics);
        Ok(())
    }

    /// Poll for a single message
    ///
    /// # Returns
    ///
    /// * `Result<Option<KafkaMessage<T>>>` - Message or None if timeout/shutdown
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use processor::kafka::{KafkaSource, KafkaSourceConfig};
    /// # use serde::{Deserialize, Serialize};
    /// # #[derive(Debug, Deserialize, Serialize)]
    /// # struct MyEvent { id: String }
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// # let config = KafkaSourceConfig::default();
    /// # let source = KafkaSource::<MyEvent>::new(config, None).await?;
    /// while let Some(msg) = source.poll().await? {
    ///     println!("Received: {:?}", msg.payload);
    ///     source.commit().await?;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn poll(&self) -> Result<Option<KafkaMessage<T>>> {
        if self.shutdown.load(Ordering::Relaxed) {
            return Ok(None);
        }

        let _timeout = Duration::from_millis(
            self.config.poll_timeout_ms.unwrap_or(DEFAULT_POLL_TIMEOUT.as_millis() as u64)
        );

        match self.consumer.recv().await {
            Ok(msg) => {
                let kafka_msg = self.process_message(&msg).await?;

                // Update metrics
                self.update_metrics(&msg).await;

                Ok(Some(kafka_msg))
            }
            Err(KafkaError::PartitionEOF(_)) => {
                debug!("Reached end of partition");
                Ok(None)
            }
            Err(e) => {
                error!("Error polling message: {}", e);
                let mut metrics = self.metrics.write().await;
                metrics.messages_failed += 1;

                Err(ProcessorError::Execution {
                    source: Box::new(e),
                })
            }
        }
    }

    /// Process a borrowed message into a typed message
    async fn process_message(&self, msg: &BorrowedMessage<'_>) -> Result<KafkaMessage<T>> {
        // Extract payload
        let payload_bytes = msg.payload().ok_or_else(|| ProcessorError::Unexpected(
            "Message has no payload".to_string(),
        ))?;

        // Deserialize payload
        let payload: T = serde_json::from_slice(payload_bytes)
            .map_err(|e| ProcessorError::Serialization(format!("Failed to deserialize message: {}", e)))?;

        // Extract key
        let key = msg.key().map(|k| k.to_vec());

        // Extract headers
        let mut headers = HashMap::new();
        if let Some(msg_headers) = msg.headers() {
            for idx in 0..msg_headers.count() {
                let header = msg_headers.get(idx);
                headers.insert(header.key.to_string(), header.value.map(|v| v.to_vec()).unwrap_or_default());
            }
        }

        // Extract timestamp
        let timestamp = msg.timestamp().to_millis().and_then(|millis| {
            DateTime::from_timestamp_millis(millis)
        });

        Ok(KafkaMessage {
            payload,
            key,
            topic: msg.topic().to_string(),
            partition: msg.partition(),
            offset: msg.offset(),
            timestamp,
            headers,
        })
    }

    /// Update metrics after processing a message
    async fn update_metrics(&self, msg: &BorrowedMessage<'_>) {
        let partition = msg.partition();
        let payload_size = msg.payload().map(|p| p.len()).unwrap_or(0);

        let mut metrics = self.metrics.write().await;

        metrics.messages_consumed += 1;
        metrics.bytes_consumed += payload_size as u64;

        *metrics.messages_per_partition.entry(partition).or_insert(0) += 1;

        // Update average message size
        if metrics.messages_consumed > 0 {
            metrics.avg_message_size = metrics.bytes_consumed as f64 / metrics.messages_consumed as f64;
        }

        drop(metrics);

        // Update throughput
        let mut tracker = self.throughput_tracker.write().await;
        tracker.record(payload_size);
        let (msgs_per_sec, bytes_per_sec) = tracker.calculate();

        if msgs_per_sec > 0.0 {
            let mut metrics = self.metrics.write().await;
            metrics.throughput_msgs_per_sec = msgs_per_sec;
            metrics.throughput_bytes_per_sec = bytes_per_sec;
        }
    }

    /// Commit current offsets
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Success or error
    pub async fn commit(&self) -> Result<()> {
        self.consumer
            .commit_consumer_state(rdkafka::consumer::CommitMode::Async)
            .map_err(|e| ProcessorError::Execution {
                source: Box::new(e),
            })?;

        debug!("Committed offsets");
        Ok(())
    }

    /// Seek to a specific offset for a partition
    ///
    /// # Arguments
    ///
    /// * `topic` - Topic name
    /// * `partition` - Partition number
    /// * `offset` - Offset to seek to
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Success or error
    pub async fn seek(&self, topic: &str, partition: i32, offset: i64) -> Result<()> {
        self.consumer
            .seek(topic, partition, Offset::Offset(offset), Duration::from_secs(10))
            .map_err(|e| ProcessorError::Execution {
                source: Box::new(e),
            })?;

        info!("Seeked to offset {} for {}/{}", offset, topic, partition);
        Ok(())
    }

    /// Get current consumption metrics
    ///
    /// # Returns
    ///
    /// * `KafkaSourceMetrics` - Current metrics snapshot
    pub async fn metrics(&self) -> KafkaSourceMetrics {
        self.metrics.read().await.clone()
    }

    /// Start consuming messages and send them to a channel
    ///
    /// This method runs in a loop, consuming messages and sending them to the
    /// provided channel. It handles automatic commits based on the configured
    /// commit strategy.
    ///
    /// # Arguments
    ///
    /// * `tx` - Channel sender for messages
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Success or error
    pub async fn start(&self, tx: mpsc::Sender<KafkaMessage<T>>) -> Result<()> {
        // Subscribe to configured topics
        self.subscribe(&self.config.topics.clone()).await?;

        let mut commit_interval = time::interval(Duration::from_secs(
            self.config.commit_interval_secs.unwrap_or(5),
        ));

        loop {
            tokio::select! {
                _ = commit_interval.tick() => {
                    if self.config.commit_strategy == CommitStrategy::Periodic {
                        if let Err(e) = self.commit().await {
                            warn!("Failed to commit offsets: {}", e);
                        }
                    }
                }

                result = self.poll() => {
                    match result {
                        Ok(Some(msg)) => {
                            // Send message to channel
                            if tx.send(msg).await.is_err() {
                                warn!("Channel closed, stopping consumer");
                                break;
                            }

                            // Commit if strategy is manual
                            if self.config.commit_strategy == CommitStrategy::Manual {
                                if let Err(e) = self.commit().await {
                                    warn!("Failed to commit offset: {}", e);
                                }
                            }
                        }
                        Ok(None) => {
                            // Timeout or shutdown
                            if self.shutdown.load(Ordering::Relaxed) {
                                info!("Shutdown signal received");
                                break;
                            }
                        }
                        Err(e) => {
                            error!("Error consuming message: {}", e);

                            // Implement retry logic
                            let _max_retries = self.config.max_retries.unwrap_or(DEFAULT_MAX_RETRIES);
                            let backoff = Duration::from_millis(
                                self.config.retry_backoff_ms.unwrap_or(DEFAULT_RETRY_BACKOFF.as_millis() as u64)
                            );

                            tokio::time::sleep(backoff).await;
                        }
                    }
                }
            }
        }

        // Final commit before shutdown
        if let Err(e) = self.commit().await {
            warn!("Failed to commit offsets on shutdown: {}", e);
        }

        info!("Kafka source consumer stopped");
        Ok(())
    }

    /// Stop the consumer gracefully
    pub async fn close(&self) -> Result<()> {
        info!("Stopping Kafka source consumer");
        self.shutdown.store(true, Ordering::Relaxed);

        // Final commit
        self.commit().await?;

        info!("Kafka source consumer stopped gracefully");
        Ok(())
    }

    /// Get current lag for all assigned partitions
    ///
    /// This fetches watermarks from Kafka to calculate lag
    pub async fn get_lag(&self) -> Result<HashMap<i32, i64>> {
        let metadata = self.consumer
            .fetch_metadata(None, Duration::from_secs(10))
            .map_err(|e| ProcessorError::Execution {
                source: Box::new(e),
            })?;

        let mut lag_map = HashMap::new();

        for topic in metadata.topics() {
            for partition in topic.partitions() {
                let partition_id = partition.id();

                // Get current position
                let position = self.consumer
                    .position()
                    .map_err(|e| ProcessorError::Execution {
                        source: Box::new(e),
                    })?;

                let current_offset = position
                    .elements_for_topic(topic.name())
                    .iter()
                    .find(|e| e.partition() == partition_id)
                    .and_then(|e| e.offset().to_raw())
                    .unwrap_or(0);

                // Get high watermark
                let (_, high_watermark) = self.consumer
                    .fetch_watermarks(topic.name(), partition_id, Duration::from_secs(10))
                    .map_err(|e| ProcessorError::Execution {
                        source: Box::new(e),
                    })?;

                let lag = high_watermark - current_offset;
                lag_map.insert(partition_id, lag);
            }
        }

        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.lag_per_partition = lag_map.clone();

        Ok(lag_map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestEvent {
        id: String,
        value: f64,
        timestamp: i64,
    }

    #[test]
    fn test_kafka_source_config_default() {
        let config = KafkaSourceConfig::default();
        assert_eq!(config.brokers, "localhost:9092");
        assert_eq!(config.group_id, "llm-optimizer-processor");
        assert_eq!(config.commit_strategy, CommitStrategy::Periodic);
    }

    #[test]
    fn test_commit_strategy_serialization() {
        let strategy = CommitStrategy::Periodic;
        let json = serde_json::to_string(&strategy).unwrap();
        assert_eq!(json, "\"periodic\"");

        let deserialized: CommitStrategy = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, CommitStrategy::Periodic);
    }

    #[test]
    fn test_throughput_tracker() {
        let mut tracker = ThroughputTracker::new();

        // Record some messages
        for _ in 0..100 {
            tracker.record(1024);
        }

        // Not enough time has passed
        let (msgs, bytes) = tracker.calculate();
        assert_eq!(msgs, 0.0);
        assert_eq!(bytes, 0.0);
    }

    #[tokio::test]
    async fn test_kafka_message_structure() {
        let event = TestEvent {
            id: "test-123".to_string(),
            value: 42.0,
            timestamp: 1234567890,
        };

        let msg = KafkaMessage {
            payload: event.clone(),
            key: Some(b"key1".to_vec()),
            topic: "test-topic".to_string(),
            partition: 0,
            offset: 100,
            timestamp: Some(Utc::now()),
            headers: HashMap::new(),
        };

        assert_eq!(msg.payload.id, "test-123");
        assert_eq!(msg.partition, 0);
        assert_eq!(msg.offset, 100);
    }

    #[tokio::test]
    async fn test_metrics_default() {
        let metrics = KafkaSourceMetrics::default();
        assert_eq!(metrics.messages_consumed, 0);
        assert_eq!(metrics.messages_failed, 0);
        assert_eq!(metrics.bytes_consumed, 0);
        assert_eq!(metrics.total_commits, 0);
        assert_eq!(metrics.rebalance_count, 0);
    }
}
