//! Kafka integration for stream processing pipeline
//!
//! This module provides integration between Kafka and the stream processing pipeline,
//! enabling Kafka-to-Kafka streaming with automatic offset management, watermarking,
//! and error recovery.

use crate::core::{EventTimeExtractor, KeyExtractor, ProcessorEvent};
use crate::error::{ProcessorError, Result};
use crate::pipeline::StreamExecutor;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::producer::{FutureProducer, FutureRecord, Producer};
use rdkafka::message::{BorrowedMessage, OwnedHeaders};
use rdkafka::{Message, Offset, TopicPartitionList};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, trace, warn};

/// Statistics for Kafka stream processing
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KafkaStreamStats {
    /// Messages consumed from Kafka
    pub messages_consumed: u64,

    /// Messages produced to Kafka
    pub messages_produced: u64,

    /// Consumer lag (estimated)
    pub consumer_lag: i64,

    /// Number of consume errors
    pub consume_errors: u64,

    /// Number of produce errors
    pub produce_errors: u64,

    /// Number of offsets committed
    pub offsets_committed: u64,

    /// Number of commit errors
    pub commit_errors: u64,

    /// Last committed offset per partition
    pub committed_offsets: HashMap<i32, i64>,
}

impl KafkaStreamStats {
    /// Create new empty stats
    pub fn new() -> Self {
        Self::default()
    }

    /// Increment messages consumed
    pub fn inc_consumed(&mut self) {
        self.messages_consumed += 1;
    }

    /// Increment messages produced
    pub fn inc_produced(&mut self) {
        self.messages_produced += 1;
    }

    /// Increment consume errors
    pub fn inc_consume_errors(&mut self) {
        self.consume_errors += 1;
    }

    /// Increment produce errors
    pub fn inc_produce_errors(&mut self) {
        self.produce_errors += 1;
    }

    /// Increment offsets committed
    pub fn inc_offsets_committed(&mut self) {
        self.offsets_committed += 1;
    }

    /// Increment commit errors
    pub fn inc_commit_errors(&mut self) {
        self.commit_errors += 1;
    }

    /// Update committed offset for a partition
    pub fn update_committed_offset(&mut self, partition: i32, offset: i64) {
        self.committed_offsets.insert(partition, offset);
    }
}

/// Offset manager for tracking and committing Kafka offsets
pub struct OffsetManager {
    /// Pending offsets to commit (topic, partition, offset)
    pending_offsets: Arc<RwLock<HashMap<(String, i32), i64>>>,

    /// Consumer for committing offsets
    consumer: Arc<StreamConsumer>,

    /// Statistics
    stats: Arc<RwLock<KafkaStreamStats>>,

    /// Auto-commit interval
    auto_commit_interval: Duration,

    /// Running flag
    running: Arc<AtomicBool>,
}

impl OffsetManager {
    /// Create a new offset manager
    pub fn new(
        consumer: Arc<StreamConsumer>,
        stats: Arc<RwLock<KafkaStreamStats>>,
        auto_commit_interval: Duration,
    ) -> Self {
        Self {
            pending_offsets: Arc::new(RwLock::new(HashMap::new())),
            consumer,
            stats,
            auto_commit_interval,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Track an offset for a message that has been processed
    pub async fn track_offset(&self, topic: String, partition: i32, offset: i64) {
        let mut offsets = self.pending_offsets.write().await;
        offsets.insert((topic.clone(), partition), offset);
        trace!(
            topic = %topic,
            partition = partition,
            offset = offset,
            "Tracked offset for commit"
        );
    }

    /// Commit all pending offsets
    pub async fn commit_offsets(&self) -> Result<()> {
        let offsets = self.pending_offsets.read().await;

        if offsets.is_empty() {
            return Ok(());
        }

        // Build topic partition list
        let mut tpl = TopicPartitionList::new();
        for ((topic, partition), offset) in offsets.iter() {
            tpl.add_partition_offset(topic, *partition, Offset::from_raw(*offset + 1))
                .map_err(|e| ProcessorError::Kafka {
                    source: format!("Failed to add partition to commit list: {}", e).into(),
                })?;
        }

        debug!(
            partitions = offsets.len(),
            "Committing offsets"
        );

        // Commit synchronously
        self.consumer
            .commit(&tpl, rdkafka::consumer::CommitMode::Sync)
            .map_err(|e| ProcessorError::Kafka {
                source: format!("Failed to commit offsets: {}", e).into(),
            })?;

        // Update stats
        let mut stats = self.stats.write().await;
        stats.inc_offsets_committed();
        for ((_, partition), offset) in offsets.iter() {
            stats.update_committed_offset(*partition, *offset);
        }

        info!("Offsets committed successfully");
        Ok(())
    }

    /// Start auto-commit task
    pub fn start_auto_commit(&self) {
        self.running.store(true, Ordering::Relaxed);

        let pending_offsets = Arc::clone(&self.pending_offsets);
        let consumer = Arc::clone(&self.consumer);
        let stats = Arc::clone(&self.stats);
        let running = Arc::clone(&self.running);
        let interval = self.auto_commit_interval;

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);

            while running.load(Ordering::Relaxed) {
                ticker.tick().await;

                let offsets = pending_offsets.read().await;
                if offsets.is_empty() {
                    continue;
                }

                // Build topic partition list
                let mut tpl = TopicPartitionList::new();
                for ((topic, partition), offset) in offsets.iter() {
                    if let Err(e) = tpl.add_partition_offset(topic, *partition, Offset::from_raw(*offset + 1)) {
                        error!(error = %e, "Failed to add partition to commit list");
                        stats.write().await.inc_commit_errors();
                        continue;
                    }
                }
                drop(offsets);

                // Commit offsets
                if let Err(e) = consumer.commit(&tpl, rdkafka::consumer::CommitMode::Async) {
                    error!(error = %e, "Failed to commit offsets");
                    stats.write().await.inc_commit_errors();
                } else {
                    debug!("Auto-committed offsets");
                    stats.write().await.inc_offsets_committed();
                }
            }

            debug!("Auto-commit task stopped");
        });
    }

    /// Stop auto-commit task
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

/// Kafka stream source that integrates with StreamExecutor
///
/// This component polls messages from Kafka and feeds them into the stream processing pipeline.
/// It handles:
/// - Message deserialization
/// - Event time extraction
/// - Watermark generation
/// - Offset tracking
pub struct KafkaStreamSource<T>
where
    T: Clone + Send + Sync + 'static,
{
    /// Kafka consumer
    consumer: Arc<StreamConsumer>,

    /// Topics to consume from
    topics: Vec<String>,

    /// Stream executor
    executor: Arc<StreamExecutor<T>>,

    /// Offset manager
    offset_manager: Arc<OffsetManager>,

    /// Statistics
    stats: Arc<RwLock<KafkaStreamStats>>,

    /// Running flag
    running: Arc<AtomicBool>,

    /// Poll timeout
    poll_timeout: Duration,
}

impl<T> KafkaStreamSource<T>
where
    T: Clone + Send + Sync + 'static + Serialize + for<'de> Deserialize<'de>,
{
    /// Create a new Kafka stream source
    pub fn new(
        consumer: Arc<StreamConsumer>,
        topics: Vec<String>,
        executor: Arc<StreamExecutor<T>>,
        offset_manager: Arc<OffsetManager>,
        stats: Arc<RwLock<KafkaStreamStats>>,
        poll_timeout: Duration,
    ) -> Self {
        Self {
            consumer,
            topics,
            executor,
            offset_manager,
            stats,
            running: Arc::new(AtomicBool::new(false)),
            poll_timeout,
        }
    }

    /// Start consuming messages and feeding them to the executor
    pub async fn run(&self) -> Result<()>
    where
        T: EventTimeExtractor<T> + KeyExtractor<T>,
    {
        info!(
            topics = ?self.topics,
            "Starting Kafka stream source"
        );

        // Subscribe to topics
        let topic_refs: Vec<&str> = self.topics.iter().map(|s| s.as_str()).collect();
        self.consumer
            .subscribe(&topic_refs)
            .map_err(|e| ProcessorError::Kafka {
                source: format!("Failed to subscribe to topics: {}", e).into(),
            })?;

        self.running.store(true, Ordering::Relaxed);

        // Start consuming
        while self.running.load(Ordering::Relaxed) {
            match self.consumer.recv().await {
                Ok(message) => {
                    if let Err(e) = self.process_message(&message).await {
                        error!(error = %e, "Failed to process message");
                        self.stats.write().await.inc_consume_errors();
                    }
                }
                Err(e) => {
                    error!(error = %e, "Error receiving message from Kafka");
                    self.stats.write().await.inc_consume_errors();
                }
            }
        }

        info!("Kafka stream source stopped");
        Ok(())
    }

    /// Process a single Kafka message
    async fn process_message(&self, message: &BorrowedMessage<'_>) -> Result<()>
    where
        T: EventTimeExtractor<T> + KeyExtractor<T>,
    {
        // Deserialize message
        let payload = message.payload().ok_or_else(|| ProcessorError::Kafka {
            source: "Message has no payload".into(),
        })?;

        let event: T = serde_json::from_slice(payload).map_err(|e| ProcessorError::Kafka {
            source: format!("Failed to deserialize message: {}", e).into(),
        })?;

        trace!(
            topic = message.topic(),
            partition = message.partition(),
            offset = message.offset(),
            "Received message"
        );

        // Feed to executor
        self.executor.ingest(event).await?;

        // Track offset for commit
        self.offset_manager
            .track_offset(
                message.topic().to_string(),
                message.partition(),
                message.offset(),
            )
            .await;

        // Update stats
        self.stats.write().await.inc_consumed();

        Ok(())
    }

    /// Stop the source
    pub fn stop(&self) {
        info!("Stopping Kafka stream source");
        self.running.store(false, Ordering::Relaxed);
    }

    /// Check if running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Get statistics
    pub async fn stats(&self) -> KafkaStreamStats {
        self.stats.read().await.clone()
    }
}

/// Kafka stream sink that receives results from the pipeline
///
/// This component receives aggregated results from the stream processing pipeline
/// and produces them to a Kafka topic. It handles:
/// - Message serialization
/// - Delivery guarantees
/// - Retries on failure
/// - Error handling
pub struct KafkaStreamSink<R>
where
    R: Clone + Send + Sync + 'static,
{
    /// Kafka producer
    producer: Arc<FutureProducer>,

    /// Output topic
    output_topic: String,

    /// Result receiver channel
    result_rx: Arc<RwLock<mpsc::Receiver<ProcessorEvent<R>>>>,

    /// Statistics
    stats: Arc<RwLock<KafkaStreamStats>>,

    /// Running flag
    running: Arc<AtomicBool>,

    /// Delivery timeout
    delivery_timeout: Duration,

    /// Max retries
    max_retries: u32,
}

impl<R> KafkaStreamSink<R>
where
    R: Clone + Send + Sync + 'static + Serialize + for<'de> Deserialize<'de>,
{
    /// Create a new Kafka stream sink
    pub fn new(
        producer: Arc<FutureProducer>,
        output_topic: String,
        result_rx: mpsc::Receiver<ProcessorEvent<R>>,
        stats: Arc<RwLock<KafkaStreamStats>>,
        delivery_timeout: Duration,
        max_retries: u32,
    ) -> Self {
        Self {
            producer,
            output_topic,
            result_rx: Arc::new(RwLock::new(result_rx)),
            stats,
            running: Arc::new(AtomicBool::new(false)),
            delivery_timeout,
            max_retries,
        }
    }

    /// Start consuming results and producing to Kafka
    pub async fn run(&self) -> Result<()> {
        info!(
            topic = %self.output_topic,
            "Starting Kafka stream sink"
        );

        self.running.store(true, Ordering::Relaxed);

        let mut rx = self.result_rx.write().await;

        while self.running.load(Ordering::Relaxed) {
            match tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
                Ok(Some(result)) => {
                    if let Err(e) = self.produce_result(result).await {
                        error!(error = %e, "Failed to produce result to Kafka");
                        self.stats.write().await.inc_produce_errors();
                    }
                }
                Ok(None) => {
                    debug!("Result channel closed");
                    break;
                }
                Err(_) => {
                    // Timeout - continue
                    continue;
                }
            }
        }

        info!("Kafka stream sink stopped");
        Ok(())
    }

    /// Produce a result to Kafka with retries
    async fn produce_result(&self, result: ProcessorEvent<R>) -> Result<()> {
        // Serialize result
        let payload = serde_json::to_vec(&result).map_err(|e| ProcessorError::Kafka {
            source: format!("Failed to serialize result: {}", e).into(),
        })?;

        let key = result.key.as_bytes();

        // Build headers
        let headers = OwnedHeaders::new()
            .insert(rdkafka::message::Header {
                key: "event_time",
                value: Some(result.event_time.to_rfc3339().as_bytes()),
            })
            .insert(rdkafka::message::Header {
                key: "processing_time",
                value: Some(result.processing_time.to_rfc3339().as_bytes()),
            });

        // Retry logic
        let mut retries = 0;
        loop {
            let record = FutureRecord::to(&self.output_topic)
                .payload(&payload)
                .key(key)
                .headers(headers.clone());

            match self.producer.send(record, self.delivery_timeout).await {
                Ok((partition, offset)) => {
                    trace!(
                        topic = %self.output_topic,
                        partition = partition,
                        offset = offset,
                        "Produced result to Kafka"
                    );

                    self.stats.write().await.inc_produced();
                    return Ok(());
                }
                Err((e, _)) => {
                    retries += 1;
                    if retries >= self.max_retries {
                        return Err(ProcessorError::Kafka {
                            source: format!(
                                "Failed to produce after {} retries: {}",
                                retries, e
                            )
                            .into(),
                        });
                    }

                    warn!(
                        error = %e,
                        retry = retries,
                        max_retries = self.max_retries,
                        "Failed to produce, retrying"
                    );

                    // Exponential backoff
                    let backoff = Duration::from_millis(100 * 2u64.pow(retries - 1));
                    tokio::time::sleep(backoff).await;
                }
            }
        }
    }

    /// Stop the sink
    pub fn stop(&self) {
        info!("Stopping Kafka stream sink");
        self.running.store(false, Ordering::Relaxed);
    }

    /// Check if running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Get statistics
    pub async fn stats(&self) -> KafkaStreamStats {
        self.stats.read().await.clone()
    }
}

/// Configuration for Kafka stream pipeline
#[derive(Debug, Clone)]
pub struct KafkaStreamConfig {
    /// Kafka brokers
    pub brokers: String,

    /// Consumer group ID
    pub group_id: String,

    /// Input topics
    pub input_topics: Vec<String>,

    /// Output topic
    pub output_topic: String,

    /// Auto-commit interval
    pub auto_commit_interval_ms: u64,

    /// Poll timeout
    pub poll_timeout_ms: u64,

    /// Delivery timeout
    pub delivery_timeout_ms: u64,

    /// Max retries for producing
    pub max_produce_retries: u32,

    /// Additional consumer config
    pub consumer_config: HashMap<String, String>,

    /// Additional producer config
    pub producer_config: HashMap<String, String>,
}

impl Default for KafkaStreamConfig {
    fn default() -> Self {
        Self {
            brokers: "localhost:9092".to_string(),
            group_id: "stream-processor".to_string(),
            input_topics: vec!["input".to_string()],
            output_topic: "output".to_string(),
            auto_commit_interval_ms: 5000,
            poll_timeout_ms: 100,
            delivery_timeout_ms: 5000,
            max_produce_retries: 3,
            consumer_config: HashMap::new(),
            producer_config: HashMap::new(),
        }
    }
}

/// End-to-end Kafka stream processing pipeline
///
/// This component orchestrates the complete Kafka-to-Kafka stream processing flow:
/// 1. Consume events from input topics
/// 2. Process through StreamExecutor (windowing, aggregation, etc.)
/// 3. Produce results to output topic
/// 4. Manage offsets automatically
/// 5. Handle errors and recovery
pub struct KafkaStreamPipeline<T, R>
where
    T: Clone + Send + Sync + 'static,
    R: Clone + Send + Sync + 'static,
{
    /// Configuration
    config: KafkaStreamConfig,

    /// Stream executor (public for accessing outputs)
    pub executor: Arc<StreamExecutor<T>>,

    /// Kafka consumer
    consumer: Arc<StreamConsumer>,

    /// Kafka producer
    producer: Arc<FutureProducer>,

    /// Offset manager
    offset_manager: Arc<OffsetManager>,

    /// Stream source
    source: Option<KafkaStreamSource<T>>,

    /// Stream sink
    sink: Option<KafkaStreamSink<R>>,

    /// Statistics (public for monitoring)
    pub stats: Arc<RwLock<KafkaStreamStats>>,

    /// Running flag
    running: Arc<AtomicBool>,
}

impl<T, R> KafkaStreamPipeline<T, R>
where
    T: Clone + Send + Sync + 'static + Serialize + for<'de> Deserialize<'de>,
    R: Clone + Send + Sync + 'static + Serialize + for<'de> Deserialize<'de>,
{
    /// Create a new Kafka stream pipeline
    pub fn new(
        config: KafkaStreamConfig,
        executor: StreamExecutor<T>,
    ) -> Result<Self> {
        // Create consumer
        let mut consumer_config = ClientConfig::new();
        consumer_config
            .set("bootstrap.servers", &config.brokers)
            .set("group.id", &config.group_id)
            .set("enable.auto.commit", "false")
            .set("auto.offset.reset", "earliest");

        // Apply additional consumer config
        for (key, value) in &config.consumer_config {
            consumer_config.set(key, value);
        }

        let consumer: StreamConsumer = consumer_config.create().map_err(|e| {
            ProcessorError::Kafka {
                source: format!("Failed to create consumer: {}", e).into(),
            }
        })?;

        // Create producer
        let mut producer_config = ClientConfig::new();
        producer_config
            .set("bootstrap.servers", &config.brokers)
            .set("message.timeout.ms", "5000");

        // Apply additional producer config
        for (key, value) in &config.producer_config {
            producer_config.set(key, value);
        }

        let producer: FutureProducer = producer_config.create().map_err(|e| {
            ProcessorError::Kafka {
                source: format!("Failed to create producer: {}", e).into(),
            }
        })?;

        let consumer = Arc::new(consumer);
        let producer = Arc::new(producer);
        let executor = Arc::new(executor);
        let stats = Arc::new(RwLock::new(KafkaStreamStats::new()));

        // Create offset manager
        let offset_manager = Arc::new(OffsetManager::new(
            Arc::clone(&consumer),
            Arc::clone(&stats),
            Duration::from_millis(config.auto_commit_interval_ms),
        ));

        Ok(Self {
            config,
            executor,
            consumer,
            producer,
            offset_manager,
            source: None,
            sink: None,
            stats,
            running: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Run the complete pipeline
    pub async fn run(&mut self, result_rx: mpsc::Receiver<ProcessorEvent<R>>) -> Result<()>
    where
        T: EventTimeExtractor<T> + KeyExtractor<T>,
    {
        info!("Starting Kafka stream pipeline");

        self.running.store(true, Ordering::Relaxed);

        // Create source
        let source = KafkaStreamSource::new(
            Arc::clone(&self.consumer),
            self.config.input_topics.clone(),
            Arc::clone(&self.executor),
            Arc::clone(&self.offset_manager),
            Arc::clone(&self.stats),
            Duration::from_millis(self.config.poll_timeout_ms),
        );

        // Create sink
        let sink = KafkaStreamSink::new(
            Arc::clone(&self.producer),
            self.config.output_topic.clone(),
            result_rx,
            Arc::clone(&self.stats),
            Duration::from_millis(self.config.delivery_timeout_ms),
            self.config.max_produce_retries,
        );

        // Start offset auto-commit
        self.offset_manager.start_auto_commit();

        // Start executor
        let executor_handle = {
            let executor = Arc::clone(&self.executor);
            tokio::spawn(async move {
                if let Err(e) = executor.run().await {
                    error!(error = %e, "Executor error");
                }
            })
        };

        // Start source
        let source_handle = {
            let source_clone = source.clone();
            tokio::spawn(async move {
                if let Err(e) = source_clone.run().await {
                    error!(error = %e, "Source error");
                }
            })
        };

        // Clone sink for storage
        let sink_clone = KafkaStreamSink {
            producer: Arc::clone(&sink.producer),
            output_topic: sink.output_topic.clone(),
            result_rx: Arc::clone(&sink.result_rx),
            stats: Arc::clone(&sink.stats),
            running: Arc::clone(&sink.running),
            delivery_timeout: sink.delivery_timeout,
            max_retries: sink.max_retries,
        };

        // Start sink
        let sink_handle = tokio::spawn(async move {
            if let Err(e) = sink.run().await {
                error!(error = %e, "Sink error");
            }
        });

        self.source = Some(source);
        self.sink = Some(sink_clone);

        // Wait for all tasks
        let _ = tokio::try_join!(executor_handle, source_handle, sink_handle);

        info!("Kafka stream pipeline stopped");
        Ok(())
    }

    /// Gracefully shutdown the pipeline
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down Kafka stream pipeline");

        self.running.store(false, Ordering::Relaxed);

        // Stop components
        if let Some(source) = &self.source {
            source.stop();
        }

        if let Some(sink) = &self.sink {
            sink.stop();
        }

        self.executor.stop();

        // Flush producer
        info!("Flushing producer");
        self.producer.flush(Duration::from_secs(30)).map_err(|e| {
            ProcessorError::Kafka {
                source: format!("Failed to flush producer: {:?}", e).into(),
            }
        })?;

        // Commit final offsets
        info!("Committing final offsets");
        self.offset_manager.commit_offsets().await?;

        // Stop offset manager
        self.offset_manager.stop();

        info!("Shutdown complete");
        Ok(())
    }

    /// Get current statistics
    pub async fn stats(&self) -> KafkaStreamStats {
        self.stats.read().await.clone()
    }

    /// Check if pipeline is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Perform health check
    pub async fn health_check(&self) -> bool {
        // Check if executor is running
        if !self.executor.is_running() {
            warn!("Executor is not running");
            return false;
        }

        // Check if source is running
        if let Some(source) = &self.source {
            if !source.is_running() {
                warn!("Source is not running");
                return false;
            }
        }

        // Check if sink is running
        if let Some(sink) = &self.sink {
            if !sink.is_running() {
                warn!("Sink is not running");
                return false;
            }
        }

        true
    }
}

// Make KafkaStreamSource cloneable by cloning the Arc references
impl<T> Clone for KafkaStreamSource<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            consumer: Arc::clone(&self.consumer),
            topics: self.topics.clone(),
            executor: Arc::clone(&self.executor),
            offset_manager: Arc::clone(&self.offset_manager),
            stats: Arc::clone(&self.stats),
            running: Arc::clone(&self.running),
            poll_timeout: self.poll_timeout,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kafka_stream_stats() {
        let mut stats = KafkaStreamStats::new();

        stats.inc_consumed();
        assert_eq!(stats.messages_consumed, 1);

        stats.inc_produced();
        assert_eq!(stats.messages_produced, 1);

        stats.inc_consume_errors();
        assert_eq!(stats.consume_errors, 1);

        stats.inc_produce_errors();
        assert_eq!(stats.produce_errors, 1);

        stats.inc_offsets_committed();
        assert_eq!(stats.offsets_committed, 1);

        stats.inc_commit_errors();
        assert_eq!(stats.commit_errors, 1);

        stats.update_committed_offset(0, 100);
        assert_eq!(stats.committed_offsets.get(&0), Some(&100));
    }

    #[test]
    fn test_kafka_stream_config_default() {
        let config = KafkaStreamConfig::default();

        assert_eq!(config.brokers, "localhost:9092");
        assert_eq!(config.group_id, "stream-processor");
        assert_eq!(config.input_topics, vec!["input"]);
        assert_eq!(config.output_topic, "output");
        assert_eq!(config.auto_commit_interval_ms, 5000);
        assert_eq!(config.max_produce_retries, 3);
    }
}
