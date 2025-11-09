//! Kafka Sink for publishing processed results
//!
//! This module provides a high-performance, production-ready Kafka producer
//! with support for batching, compression, transactions, and exactly-once semantics.

use crate::error::{ProcessorError, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rdkafka::config::ClientConfig;
use rdkafka::error::KafkaError;
use rdkafka::message::{Header, OwnedHeaders};
use rdkafka::producer::{FutureProducer, FutureRecord, Producer};
use rdkafka::util::Timeout;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock, Semaphore};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// Maximum number of messages to batch together
const DEFAULT_BATCH_SIZE: usize = 100;

/// Default timeout for sending messages
const DEFAULT_SEND_TIMEOUT_MS: u64 = 30000;

/// Default number of retries for failed sends
const DEFAULT_MAX_RETRIES: u32 = 5;

/// Default base delay for exponential backoff (milliseconds)
const DEFAULT_BASE_BACKOFF_MS: u64 = 100;

/// Configuration for Kafka sink
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaSinkConfig {
    /// Kafka bootstrap servers
    pub brokers: String,

    /// Default topic for messages
    pub topic: String,

    /// Client ID for this producer
    pub client_id: String,

    /// Batch size for batching messages
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Timeout for sending messages (milliseconds)
    #[serde(default = "default_send_timeout")]
    pub send_timeout_ms: u64,

    /// Enable idempotent producer
    #[serde(default = "default_true")]
    pub enable_idempotence: bool,

    /// Enable transactions for exactly-once semantics
    #[serde(default)]
    pub enable_transactions: bool,

    /// Transaction ID (required if transactions enabled)
    pub transactional_id: Option<String>,

    /// Compression type (none, gzip, snappy, lz4, zstd)
    #[serde(default = "default_compression")]
    pub compression_type: String,

    /// Acknowledgment level (0, 1, all)
    #[serde(default = "default_acks")]
    pub acks: String,

    /// Maximum retries for failed sends
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Base delay for exponential backoff (milliseconds)
    #[serde(default = "default_base_backoff")]
    pub base_backoff_ms: u64,

    /// Maximum in-flight requests
    #[serde(default = "default_max_in_flight")]
    pub max_in_flight_requests: usize,

    /// Linger time for batching (milliseconds)
    #[serde(default)]
    pub linger_ms: u64,

    /// Buffer memory size (bytes)
    #[serde(default = "default_buffer_memory")]
    pub buffer_memory: usize,

    /// Enable circuit breaker
    #[serde(default = "default_true")]
    pub enable_circuit_breaker: bool,

    /// Circuit breaker failure threshold
    #[serde(default = "default_circuit_breaker_threshold")]
    pub circuit_breaker_threshold: u32,

    /// Circuit breaker timeout (seconds)
    #[serde(default = "default_circuit_breaker_timeout")]
    pub circuit_breaker_timeout_secs: u64,
}

fn default_batch_size() -> usize { DEFAULT_BATCH_SIZE }
fn default_send_timeout() -> u64 { DEFAULT_SEND_TIMEOUT_MS }
fn default_true() -> bool { true }
fn default_compression() -> String { "snappy".to_string() }
fn default_acks() -> String { "all".to_string() }
fn default_max_retries() -> u32 { DEFAULT_MAX_RETRIES }
fn default_base_backoff() -> u64 { DEFAULT_BASE_BACKOFF_MS }
fn default_max_in_flight() -> usize { 5 }
fn default_buffer_memory() -> usize { 33554432 } // 32MB
fn default_circuit_breaker_threshold() -> u32 { 5 }
fn default_circuit_breaker_timeout() -> u64 { 60 }

impl Default for KafkaSinkConfig {
    fn default() -> Self {
        Self {
            brokers: "localhost:9092".to_string(),
            topic: "llm-optimizer-results".to_string(),
            client_id: "llm-optimizer-producer".to_string(),
            batch_size: DEFAULT_BATCH_SIZE,
            send_timeout_ms: DEFAULT_SEND_TIMEOUT_MS,
            enable_idempotence: true,
            enable_transactions: false,
            transactional_id: None,
            compression_type: "snappy".to_string(),
            acks: "all".to_string(),
            max_retries: DEFAULT_MAX_RETRIES,
            base_backoff_ms: DEFAULT_BASE_BACKOFF_MS,
            max_in_flight_requests: 5,
            linger_ms: 10,
            buffer_memory: 33554432,
            enable_circuit_breaker: true,
            circuit_breaker_threshold: 5,
            circuit_breaker_timeout_secs: 60,
        }
    }
}

/// Partitioning strategy for messages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionStrategy {
    /// Use key hash for partitioning
    KeyHash,
    /// Round-robin partitioning
    RoundRobin,
    /// Custom partition specified per message
    Custom,
    /// All messages to partition 0
    Single,
}

/// Delivery guarantee level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryGuarantee {
    /// At-most-once (fire and forget)
    AtMostOnce,
    /// At-least-once (retry on failure)
    AtLeastOnce,
    /// Exactly-once (transactions required)
    ExactlyOnce,
}

/// Message to be sent to Kafka
#[derive(Debug, Clone)]
pub struct KafkaMessage<T> {
    /// Message payload
    pub payload: T,
    /// Message key (optional)
    pub key: Option<String>,
    /// Target topic (uses default if None)
    pub topic: Option<String>,
    /// Target partition (for custom partitioning)
    pub partition: Option<i32>,
    /// Message headers
    pub headers: HashMap<String, String>,
    /// Timestamp
    pub timestamp: Option<DateTime<Utc>>,
}

impl<T> KafkaMessage<T> {
    /// Create a new message with payload
    pub fn new(payload: T) -> Self {
        Self {
            payload,
            key: None,
            topic: None,
            partition: None,
            headers: HashMap::new(),
            timestamp: None,
        }
    }

    /// Set message key
    pub fn with_key(mut self, key: String) -> Self {
        self.key = Some(key);
        self
    }

    /// Set topic
    pub fn with_topic(mut self, topic: String) -> Self {
        self.topic = Some(topic);
        self
    }

    /// Set partition
    pub fn with_partition(mut self, partition: i32) -> Self {
        self.partition = Some(partition);
        self
    }

    /// Add header
    pub fn with_header(mut self, key: String, value: String) -> Self {
        self.headers.insert(key, value);
        self
    }

    /// Set timestamp
    pub fn with_timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = Some(timestamp);
        self
    }
}

/// Production metrics
#[derive(Debug, Default, Clone)]
pub struct SinkMetrics {
    /// Total messages sent successfully
    pub messages_sent: u64,
    /// Total messages failed
    pub messages_failed: u64,
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total send attempts
    pub send_attempts: u64,
    /// Total retries
    pub retries: u64,
    /// Average send latency (microseconds)
    pub avg_latency_us: u64,
    /// Maximum send latency (microseconds)
    pub max_latency_us: u64,
    /// Circuit breaker trips
    pub circuit_breaker_trips: u64,
    /// Last error timestamp
    pub last_error: Option<DateTime<Utc>>,
    /// Last error message
    pub last_error_msg: Option<String>,
}

/// Internal metrics tracking
struct MetricsTracker {
    messages_sent: AtomicU64,
    messages_failed: AtomicU64,
    bytes_sent: AtomicU64,
    send_attempts: AtomicU64,
    retries: AtomicU64,
    total_latency_us: AtomicU64,
    max_latency_us: AtomicU64,
    circuit_breaker_trips: AtomicU64,
    last_error: Mutex<Option<(DateTime<Utc>, String)>>,
}

impl MetricsTracker {
    fn new() -> Self {
        Self {
            messages_sent: AtomicU64::new(0),
            messages_failed: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            send_attempts: AtomicU64::new(0),
            retries: AtomicU64::new(0),
            total_latency_us: AtomicU64::new(0),
            max_latency_us: AtomicU64::new(0),
            circuit_breaker_trips: AtomicU64::new(0),
            last_error: Mutex::new(None),
        }
    }

    async fn record_success(&self, latency_us: u64, bytes: u64) {
        self.messages_sent.fetch_add(1, Ordering::Relaxed);
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
        self.total_latency_us.fetch_add(latency_us, Ordering::Relaxed);

        let mut max = self.max_latency_us.load(Ordering::Relaxed);
        while latency_us > max {
            match self.max_latency_us.compare_exchange_weak(
                max,
                latency_us,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => max = x,
            }
        }
    }

    async fn record_failure(&self, error: String) {
        self.messages_failed.fetch_add(1, Ordering::Relaxed);
        let mut last_error = self.last_error.lock().await;
        *last_error = Some((Utc::now(), error));
    }

    fn record_attempt(&self) {
        self.send_attempts.fetch_add(1, Ordering::Relaxed);
    }

    fn record_retry(&self) {
        self.retries.fetch_add(1, Ordering::Relaxed);
    }

    fn record_circuit_breaker_trip(&self) {
        self.circuit_breaker_trips.fetch_add(1, Ordering::Relaxed);
    }

    async fn snapshot(&self) -> SinkMetrics {
        let messages_sent = self.messages_sent.load(Ordering::Relaxed);
        let total_latency = self.total_latency_us.load(Ordering::Relaxed);
        let avg_latency = if messages_sent > 0 {
            total_latency / messages_sent
        } else {
            0
        };

        let last_error = self.last_error.lock().await;
        let (last_error_time, last_error_msg) = last_error.as_ref()
            .map(|(t, m)| (Some(*t), Some(m.clone())))
            .unwrap_or((None, None));

        SinkMetrics {
            messages_sent,
            messages_failed: self.messages_failed.load(Ordering::Relaxed),
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            send_attempts: self.send_attempts.load(Ordering::Relaxed),
            retries: self.retries.load(Ordering::Relaxed),
            avg_latency_us: avg_latency,
            max_latency_us: self.max_latency_us.load(Ordering::Relaxed),
            circuit_breaker_trips: self.circuit_breaker_trips.load(Ordering::Relaxed),
            last_error: last_error_time,
            last_error_msg,
        }
    }
}

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

/// Circuit breaker for fault tolerance
struct CircuitBreaker {
    state: RwLock<CircuitState>,
    failure_count: AtomicU64,
    threshold: u32,
    timeout: Duration,
    last_failure_time: RwLock<Option<Instant>>,
}

impl CircuitBreaker {
    fn new(threshold: u32, timeout: Duration) -> Self {
        Self {
            state: RwLock::new(CircuitState::Closed),
            failure_count: AtomicU64::new(0),
            threshold,
            timeout,
            last_failure_time: RwLock::new(None),
        }
    }

    async fn can_attempt(&self) -> bool {
        let state = *self.state.read().await;

        match state {
            CircuitState::Closed => true,
            CircuitState::HalfOpen => true,
            CircuitState::Open => {
                // Check if timeout has elapsed
                if let Some(last_failure) = *self.last_failure_time.read().await {
                    if last_failure.elapsed() >= self.timeout {
                        // Transition to half-open
                        *self.state.write().await = CircuitState::HalfOpen;
                        self.failure_count.store(0, Ordering::Relaxed);
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
        }
    }

    async fn record_success(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::Closed;
        self.failure_count.store(0, Ordering::Relaxed);
    }

    async fn record_failure(&self) {
        let failures = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        *self.last_failure_time.write().await = Some(Instant::now());

        if failures >= self.threshold as u64 {
            let mut state = self.state.write().await;
            *state = CircuitState::Open;
        }
    }

    async fn is_open(&self) -> bool {
        *self.state.read().await == CircuitState::Open
    }
}

/// Serializer trait for converting messages to bytes
#[async_trait]
pub trait MessageSerializer<T>: Send + Sync {
    /// Serialize message to bytes
    async fn serialize(&self, message: &T) -> Result<Vec<u8>>;

    /// Get content type identifier
    fn content_type(&self) -> &str;
}

/// JSON serializer
pub struct JsonSerializer;

#[async_trait]
impl<T: Serialize + Send + Sync> MessageSerializer<T> for JsonSerializer {
    async fn serialize(&self, message: &T) -> Result<Vec<u8>> {
        serde_json::to_vec(message).map_err(|e| ProcessorError::Serialization(e.to_string()))
    }

    fn content_type(&self) -> &str {
        "application/json"
    }
}

/// Bincode serializer
pub struct BincodeSerializer;

#[async_trait]
impl<T: Serialize + Send + Sync> MessageSerializer<T> for BincodeSerializer {
    async fn serialize(&self, message: &T) -> Result<Vec<u8>> {
        bincode::serialize(message).map_err(|e| ProcessorError::Serialization(e.to_string()))
    }

    fn content_type(&self) -> &str {
        "application/octet-stream"
    }
}

/// Kafka sink for publishing messages
pub struct KafkaSink<T> {
    /// Kafka producer
    producer: Arc<FutureProducer>,

    /// Configuration
    config: KafkaSinkConfig,

    /// Message serializer
    serializer: Arc<dyn MessageSerializer<T>>,

    /// Partitioning strategy
    partition_strategy: PartitionStrategy,

    /// Delivery guarantee
    delivery_guarantee: DeliveryGuarantee,

    /// Metrics tracker
    metrics: Arc<MetricsTracker>,

    /// Circuit breaker
    circuit_breaker: Option<Arc<CircuitBreaker>>,

    /// Semaphore for controlling in-flight requests
    in_flight_semaphore: Arc<Semaphore>,

    /// Whether transactions are active
    in_transaction: AtomicBool,

    /// Shutdown flag
    shutdown: AtomicBool,
}

impl<T> KafkaSink<T>
where
    T: Serialize + Send + Sync + 'static,
{
    /// Create a new Kafka sink with JSON serialization
    pub async fn new(config: KafkaSinkConfig) -> Result<Self> {
        Self::with_serializer(config, Arc::new(JsonSerializer)).await
    }

    /// Create a new Kafka sink with custom serializer
    pub async fn with_serializer(
        config: KafkaSinkConfig,
        serializer: Arc<dyn MessageSerializer<T>>,
    ) -> Result<Self> {
        // Validate configuration
        if config.enable_transactions && config.transactional_id.is_none() {
            return Err(ProcessorError::Configuration {
                source: "transactional_id required when transactions are enabled".into(),
            });
        }

        // Build Kafka producer config
        let mut client_config = ClientConfig::new();
        client_config
            .set("bootstrap.servers", &config.brokers)
            .set("client.id", &config.client_id)
            .set("compression.type", &config.compression_type)
            .set("acks", &config.acks)
            .set("linger.ms", config.linger_ms.to_string())
            .set("batch.size", config.batch_size.to_string())
            .set("buffer.memory", config.buffer_memory.to_string())
            .set("max.in.flight.requests.per.connection", config.max_in_flight_requests.to_string())
            .set("retries", config.max_retries.to_string());

        if config.enable_idempotence {
            client_config.set("enable.idempotence", "true");
        }

        if config.enable_transactions {
            if let Some(ref txn_id) = config.transactional_id {
                client_config.set("transactional.id", txn_id);
            }
        }

        // Create producer
        let producer: FutureProducer = client_config
            .create()
            .map_err(|e| ProcessorError::Configuration {
                source: Box::new(e),
            })?;

        // Initialize transactions if enabled
        if config.enable_transactions {
            producer.init_transactions(Timeout::After(Duration::from_secs(30)))
                .map_err(|e| ProcessorError::Configuration {
                    source: Box::new(e),
                })?;
        }

        let circuit_breaker = if config.enable_circuit_breaker {
            Some(Arc::new(CircuitBreaker::new(
                config.circuit_breaker_threshold,
                Duration::from_secs(config.circuit_breaker_timeout_secs),
            )))
        } else {
            None
        };

        Ok(Self {
            producer: Arc::new(producer),
            config,
            serializer,
            partition_strategy: PartitionStrategy::KeyHash,
            delivery_guarantee: DeliveryGuarantee::AtLeastOnce,
            metrics: Arc::new(MetricsTracker::new()),
            circuit_breaker,
            in_flight_semaphore: Arc::new(Semaphore::new(5)),
            in_transaction: AtomicBool::new(false),
            shutdown: AtomicBool::new(false),
        })
    }

    /// Set partitioning strategy
    pub fn with_partition_strategy(mut self, strategy: PartitionStrategy) -> Self {
        self.partition_strategy = strategy;
        self
    }

    /// Set delivery guarantee
    pub fn with_delivery_guarantee(mut self, guarantee: DeliveryGuarantee) -> Self {
        self.delivery_guarantee = guarantee;
        self
    }

    /// Send a single message
    pub async fn send(&self, message: KafkaMessage<T>) -> Result<()> {
        if self.shutdown.load(Ordering::Relaxed) {
            return Err(ProcessorError::Unexpected("sink is shut down".to_string()));
        }

        // Check circuit breaker
        if let Some(ref cb) = self.circuit_breaker {
            if !cb.can_attempt().await {
                self.metrics.record_circuit_breaker_trip();
                return Err(ProcessorError::Unexpected(
                    "circuit breaker is open".to_string()
                ));
            }
        }

        // Serialize message
        let payload_bytes = self.serializer.serialize(&message.payload).await?;
        let payload_size = payload_bytes.len() as u64;

        // Send with retry logic
        let result = self.send_with_retry(message, payload_bytes).await;

        // Update circuit breaker
        if let Some(ref cb) = self.circuit_breaker {
            match result {
                Ok(_) => cb.record_success().await,
                Err(_) => cb.record_failure().await,
            }
        }

        // Update metrics
        match result {
            Ok(latency_us) => {
                self.metrics.record_success(latency_us, payload_size).await;
                Ok(())
            }
            Err(e) => {
                self.metrics.record_failure(e.to_string()).await;
                Err(e)
            }
        }
    }

    /// Send a batch of messages
    pub async fn send_batch(&self, messages: Vec<KafkaMessage<T>>) -> Result<Vec<Result<()>>> {
        if self.shutdown.load(Ordering::Relaxed) {
            return Err(ProcessorError::Unexpected("sink is shut down".to_string()));
        }

        let mut results = Vec::with_capacity(messages.len());

        // Process messages in parallel with concurrency control
        let mut handles = Vec::new();

        for message in messages {
            let permit = self.in_flight_semaphore.clone().acquire_owned().await
                .map_err(|e| ProcessorError::Unexpected(e.to_string()))?;

            let sink = self.clone_for_send();
            let handle = tokio::spawn(async move {
                let result = sink.send(message).await;
                drop(permit);
                result
            });

            handles.push(handle);
        }

        // Collect results
        for handle in handles {
            let result = handle.await
                .map_err(|e| ProcessorError::Unexpected(e.to_string()))?;
            results.push(result);
        }

        Ok(results)
    }

    /// Begin a transaction
    pub async fn begin_transaction(&self) -> Result<()> {
        if !self.config.enable_transactions {
            return Err(ProcessorError::Configuration {
                source: "transactions not enabled".into(),
            });
        }

        if self.in_transaction.swap(true, Ordering::SeqCst) {
            return Err(ProcessorError::Unexpected(
                "transaction already in progress".to_string()
            ));
        }

        self.producer
            .begin_transaction()
            .map_err(|e| ProcessorError::Execution {
                source: Box::new(e),
            })?;

        debug!("Transaction begun");
        Ok(())
    }

    /// Commit current transaction
    pub async fn commit_transaction(&self) -> Result<()> {
        if !self.in_transaction.load(Ordering::SeqCst) {
            return Err(ProcessorError::Unexpected(
                "no transaction in progress".to_string()
            ));
        }

        self.producer
            .commit_transaction(Timeout::After(Duration::from_secs(30)))
            .map_err(|e| ProcessorError::Execution {
                source: Box::new(e),
            })?;

        self.in_transaction.store(false, Ordering::SeqCst);
        debug!("Transaction committed");
        Ok(())
    }

    /// Abort current transaction
    pub async fn abort_transaction(&self) -> Result<()> {
        if !self.in_transaction.load(Ordering::SeqCst) {
            return Err(ProcessorError::Unexpected(
                "no transaction in progress".to_string()
            ));
        }

        self.producer
            .abort_transaction(Timeout::After(Duration::from_secs(30)))
            .map_err(|e| ProcessorError::Execution {
                source: Box::new(e),
            })?;

        self.in_transaction.store(false, Ordering::SeqCst);
        warn!("Transaction aborted");
        Ok(())
    }

    /// Flush pending messages
    pub async fn flush(&self) -> Result<()> {
        info!("Flushing pending messages...");

        self.producer
            .flush(Timeout::After(Duration::from_millis(self.config.send_timeout_ms)))
            .map_err(|e| ProcessorError::Execution {
                source: Box::new(e),
            })?;

        info!("Flush completed");
        Ok(())
    }

    /// Get current metrics
    pub async fn metrics(&self) -> SinkMetrics {
        self.metrics.snapshot().await
    }

    /// Graceful shutdown
    pub async fn close(&self) -> Result<()> {
        info!("Shutting down Kafka sink...");

        self.shutdown.store(true, Ordering::Relaxed);

        // Abort any active transaction
        if self.in_transaction.load(Ordering::SeqCst) {
            if let Err(e) = self.abort_transaction().await {
                error!("Failed to abort transaction during shutdown: {}", e);
            }
        }

        // Flush pending messages
        if let Err(e) = self.flush().await {
            error!("Failed to flush during shutdown: {}", e);
        }

        let metrics = self.metrics().await;
        info!(
            "Kafka sink shut down. Sent: {}, Failed: {}, Avg latency: {}us",
            metrics.messages_sent,
            metrics.messages_failed,
            metrics.avg_latency_us
        );

        Ok(())
    }

    /// Internal: Send message with retry logic
    async fn send_with_retry(
        &self,
        message: KafkaMessage<T>,
        payload_bytes: Vec<u8>,
    ) -> Result<u64> {
        let mut retries = 0;
        let max_retries = self.config.max_retries;

        loop {
            self.metrics.record_attempt();
            let start = Instant::now();

            let topic = message.topic.as_ref()
                .unwrap_or(&self.config.topic);

            // Build Kafka record
            let mut record = FutureRecord::to(topic)
                .payload(&payload_bytes);

            if let Some(ref key) = message.key {
                record = record.key(key);
            }

            if let Some(partition) = message.partition {
                record = record.partition(partition);
            }

            if let Some(timestamp) = message.timestamp {
                record = record.timestamp(timestamp.timestamp_millis());
            }

            // Add headers
            if !message.headers.is_empty() {
                let mut headers = OwnedHeaders::new();
                for (k, v) in &message.headers {
                    headers = headers.insert(Header {
                        key: k,
                        value: Some(v.as_bytes()),
                    });
                }
                // Add content type
                headers = headers.insert(Header {
                    key: "content-type",
                    value: Some(self.serializer.content_type().as_bytes()),
                });
                record = record.headers(headers);
            }

            // Send message
            let result = self.producer
                .send(
                    record,
                    Timeout::After(Duration::from_millis(self.config.send_timeout_ms)),
                )
                .await;

            let latency_us = start.elapsed().as_micros() as u64;

            match result {
                Ok(_) => {
                    if retries > 0 {
                        debug!("Message sent successfully after {} retries", retries);
                    }
                    return Ok(latency_us);
                }
                Err((err, _)) => {
                    if !self.should_retry(&err, retries, max_retries) {
                        error!("Failed to send message after {} retries: {}", retries, err);
                        return Err(ProcessorError::Execution {
                            source: Box::new(err),
                        });
                    }

                    retries += 1;
                    self.metrics.record_retry();

                    let backoff = self.calculate_backoff(retries);
                    warn!(
                        "Send failed (attempt {}/{}), retrying in {:?}: {}",
                        retries, max_retries + 1, backoff, err
                    );
                    sleep(backoff).await;
                }
            }
        }
    }

    /// Check if error is retriable
    fn should_retry(&self, error: &KafkaError, retries: u32, max_retries: u32) -> bool {
        if retries >= max_retries {
            return false;
        }

        // Check if error is retriable
        match error {
            KafkaError::MessageProduction(rdkafka::types::RDKafkaErrorCode::QueueFull) => true,
            KafkaError::MessageProduction(rdkafka::types::RDKafkaErrorCode::NetworkException) => true,
            KafkaError::MessageProduction(rdkafka::types::RDKafkaErrorCode::RequestTimedOut) => true,
            KafkaError::MessageProduction(rdkafka::types::RDKafkaErrorCode::NotLeaderForPartition) => true,
            _ => false,
        }
    }

    /// Calculate exponential backoff delay
    fn calculate_backoff(&self, retry_count: u32) -> Duration {
        let base_ms = self.config.base_backoff_ms;
        let backoff_ms = base_ms * 2u64.pow(retry_count.min(10));
        let max_backoff_ms = 60_000; // 1 minute max
        Duration::from_millis(backoff_ms.min(max_backoff_ms))
    }

    /// Clone for parallel sends (shares producer and metrics)
    fn clone_for_send(&self) -> Self {
        Self {
            producer: Arc::clone(&self.producer),
            config: self.config.clone(),
            serializer: Arc::clone(&self.serializer),
            partition_strategy: self.partition_strategy,
            delivery_guarantee: self.delivery_guarantee,
            metrics: Arc::clone(&self.metrics),
            circuit_breaker: self.circuit_breaker.as_ref().map(Arc::clone),
            in_flight_semaphore: Arc::clone(&self.in_flight_semaphore),
            in_transaction: AtomicBool::new(false),
            shutdown: AtomicBool::new(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestMessage {
        id: String,
        value: i32,
    }

    #[tokio::test]
    async fn test_kafka_message_builder() {
        let msg = KafkaMessage::new(TestMessage {
            id: "test".to_string(),
            value: 42,
        })
        .with_key("key1".to_string())
        .with_topic("test-topic".to_string())
        .with_header("trace-id".to_string(), "12345".to_string());

        assert_eq!(msg.key, Some("key1".to_string()));
        assert_eq!(msg.topic, Some("test-topic".to_string()));
        assert_eq!(msg.headers.get("trace-id"), Some(&"12345".to_string()));
    }

    #[tokio::test]
    async fn test_json_serializer() {
        let serializer = JsonSerializer;
        let msg = TestMessage {
            id: "test".to_string(),
            value: 42,
        };

        let bytes = MessageSerializer::serialize(&serializer, &msg).await.unwrap();
        let deserialized: TestMessage = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(msg, deserialized);
        assert_eq!(MessageSerializer::<TestMessage>::content_type(&serializer), "application/json");
    }

    #[tokio::test]
    async fn test_bincode_serializer() {
        let serializer = BincodeSerializer;
        let msg = TestMessage {
            id: "test".to_string(),
            value: 42,
        };

        let bytes = MessageSerializer::serialize(&serializer, &msg).await.unwrap();
        let deserialized: TestMessage = bincode::deserialize(&bytes).unwrap();

        assert_eq!(msg, deserialized);
        assert_eq!(MessageSerializer::<TestMessage>::content_type(&serializer), "application/octet-stream");
    }

    #[tokio::test]
    async fn test_metrics_tracker() {
        let tracker = MetricsTracker::new();

        tracker.record_success(1000, 100).await;
        tracker.record_success(2000, 200).await;
        tracker.record_failure("test error".to_string()).await;
        tracker.record_attempt();
        tracker.record_retry();

        let metrics = tracker.snapshot().await;

        assert_eq!(metrics.messages_sent, 2);
        assert_eq!(metrics.messages_failed, 1);
        assert_eq!(metrics.bytes_sent, 300);
        assert_eq!(metrics.send_attempts, 1);
        assert_eq!(metrics.retries, 1);
        assert_eq!(metrics.avg_latency_us, 1500);
        assert_eq!(metrics.max_latency_us, 2000);
    }

    #[tokio::test]
    async fn test_circuit_breaker() {
        let cb = CircuitBreaker::new(3, Duration::from_millis(100));

        // Initially closed
        assert!(cb.can_attempt().await);
        assert!(!cb.is_open().await);

        // Record failures
        cb.record_failure().await;
        cb.record_failure().await;
        assert!(!cb.is_open().await);

        cb.record_failure().await;
        assert!(cb.is_open().await);
        assert!(!cb.can_attempt().await);

        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should transition to half-open
        assert!(cb.can_attempt().await);

        // Success closes circuit
        cb.record_success().await;
        assert!(!cb.is_open().await);
    }

    #[test]
    fn test_config_defaults() {
        let config = KafkaSinkConfig::default();

        assert_eq!(config.brokers, "localhost:9092");
        assert_eq!(config.batch_size, DEFAULT_BATCH_SIZE);
        assert!(config.enable_idempotence);
        assert!(!config.enable_transactions);
        assert_eq!(config.compression_type, "snappy");
        assert_eq!(config.acks, "all");
    }

    #[test]
    fn test_partition_strategy() {
        assert_eq!(PartitionStrategy::KeyHash, PartitionStrategy::KeyHash);
        assert_ne!(PartitionStrategy::KeyHash, PartitionStrategy::RoundRobin);
    }

    #[test]
    fn test_delivery_guarantee() {
        assert_eq!(DeliveryGuarantee::AtLeastOnce, DeliveryGuarantee::AtLeastOnce);
        assert_ne!(DeliveryGuarantee::AtLeastOnce, DeliveryGuarantee::ExactlyOnce);
    }
}
