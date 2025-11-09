//! Feedback Collector
//!
//! This module provides the main feedback collection system with
//! batching, buffering, async processing, backpressure handling, and rate limiting.

use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use crate::backpressure::{BackpressureConfig, BackpressureError, BackpressureHandler};
use crate::feedback_events::{FeedbackEvent, FeedbackEventBatch};
use crate::health::HealthChecker;
use crate::kafka_client::{FeedbackProducer, KafkaError, KafkaProducerConfig};
use crate::rate_limiter::{RateLimiter, RateLimitConfig, RateLimitError};
use crate::telemetry::{TelemetryConfig, TelemetryMetrics, TelemetryProvider};

/// Submit error types
#[derive(Debug, thiserror::Error)]
pub enum SubmitError {
    /// Rate limit exceeded
    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(#[from] RateLimitError),

    /// Backpressure error
    #[error("Backpressure error: {0}")]
    Backpressure(#[from] BackpressureError),

    /// Channel send error
    #[error("Channel send error: {0}")]
    ChannelSend(String),
}

/// Try submit error types
#[derive(Debug, thiserror::Error)]
pub enum TrySubmitError {
    /// Backpressure error
    #[error("Backpressure error: {0}")]
    Backpressure(#[from] BackpressureError),

    /// Channel closed
    #[error("Channel closed")]
    ChannelClosed,
}

/// Feedback collector configuration
#[derive(Debug, Clone)]
pub struct FeedbackCollectorConfig {
    /// Buffer size for async channel
    pub buffer_size: usize,
    /// Maximum batch size
    pub max_batch_size: usize,
    /// Maximum batch age in seconds before forcing flush
    pub max_batch_age_secs: u64,
    /// Flush interval in milliseconds
    pub flush_interval_ms: u64,
    /// Number of worker threads
    pub num_workers: usize,
    /// Kafka configuration
    pub kafka_config: KafkaProducerConfig,
    /// Telemetry configuration
    pub telemetry_config: TelemetryConfig,
    /// Rate limit configuration (optional)
    pub rate_limit_config: Option<RateLimitConfig>,
    /// Backpressure configuration
    pub backpressure_config: BackpressureConfig,
    /// Enable automatic recovery from disk
    pub enable_recovery: bool,
    /// Shutdown timeout in seconds (default: 30)
    pub shutdown_timeout_secs: u64,
}

impl Default for FeedbackCollectorConfig {
    fn default() -> Self {
        Self {
            buffer_size: 10000,
            max_batch_size: 100,
            max_batch_age_secs: 10,
            flush_interval_ms: 5000,
            num_workers: 4,
            kafka_config: KafkaProducerConfig::default(),
            telemetry_config: TelemetryConfig::default(),
            rate_limit_config: Some(RateLimitConfig::default()),
            backpressure_config: BackpressureConfig::default(),
            enable_recovery: true,
            shutdown_timeout_secs: 30,
        }
    }
}

impl FeedbackCollectorConfig {
    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.buffer_size == 0 {
            return Err("buffer_size must be greater than 0".to_string());
        }
        if self.max_batch_size == 0 {
            return Err("max_batch_size must be greater than 0".to_string());
        }
        if self.max_batch_size > self.buffer_size {
            return Err("max_batch_size cannot exceed buffer_size".to_string());
        }
        if self.max_batch_age_secs == 0 {
            return Err("max_batch_age_secs must be greater than 0".to_string());
        }
        if self.flush_interval_ms == 0 {
            return Err("flush_interval_ms must be greater than 0".to_string());
        }
        if self.num_workers == 0 {
            return Err("num_workers must be greater than 0".to_string());
        }
        if self.shutdown_timeout_secs == 0 {
            return Err("shutdown_timeout_secs must be greater than 0".to_string());
        }
        Ok(())
    }
}

/// Feedback collector statistics
#[derive(Debug, Clone, Default)]
pub struct CollectorStats {
    /// Total events received
    pub events_received: u64,
    /// Total events published
    pub events_published: u64,
    /// Total events failed
    pub events_failed: u64,
    /// Total batches processed
    pub batches_processed: u64,
    /// Current buffer size
    pub current_buffer_size: usize,
    /// Total events rate limited
    pub events_rate_limited: u64,
    /// Events dropped due to overflow
    pub events_dropped: u64,
    /// Events persisted to disk
    pub events_persisted: u64,
    /// Events rejected (backpressure)
    pub events_rejected: u64,
    /// Buffer utilization percentage
    pub buffer_utilization_percent: f64,
}

/// Feedback collector
pub struct FeedbackCollector {
    /// Configuration
    config: FeedbackCollectorConfig,
    /// Event sender
    event_sender: mpsc::Sender<FeedbackEvent>,
    /// Worker handles
    worker_handles: Vec<JoinHandle<()>>,
    /// Statistics
    stats: Arc<DashMap<String, u64>>,
    /// Telemetry metrics
    metrics: Option<Arc<TelemetryMetrics>>,
    /// Kafka producer reference (for health checks)
    kafka_producer: Option<Arc<FeedbackProducer>>,
    /// Health checker
    health_checker: Option<Arc<HealthChecker>>,
    /// Rate limiter
    rate_limiter: Option<Arc<RateLimiter>>,
    /// Backpressure handler
    backpressure_handler: Arc<BackpressureHandler>,
}

impl FeedbackCollector {
    /// Create new feedback collector
    /// Returns (FeedbackCollector, receiver) - you must call start() with the receiver
    pub fn new(config: FeedbackCollectorConfig) -> (Self, mpsc::Receiver<FeedbackEvent>) {
        let (event_sender, event_receiver) = mpsc::channel(config.buffer_size);

        let stats = Arc::new(DashMap::new());
        stats.insert("events_received".to_string(), 0);
        stats.insert("events_published".to_string(), 0);
        stats.insert("events_failed".to_string(), 0);
        stats.insert("batches_processed".to_string(), 0);
        stats.insert("events_rate_limited".to_string(), 0);
        stats.insert("events_dropped".to_string(), 0);
        stats.insert("events_persisted".to_string(), 0);
        stats.insert("events_rejected".to_string(), 0);

        // Create rate limiter if configured
        let rate_limiter = config
            .rate_limit_config
            .as_ref()
            .and_then(|cfg| RateLimiter::new(cfg.clone()).ok())
            .map(Arc::new);

        let backpressure_handler = Arc::new(BackpressureHandler::new(
            config.backpressure_config.clone(),
        ));

        let collector = Self {
            config,
            event_sender,
            worker_handles: Vec::new(),
            stats,
            metrics: None,
            kafka_producer: None,
            health_checker: None,
            rate_limiter,
            backpressure_handler,
        };

        (collector, event_receiver)
    }

    /// Initialize with telemetry
    pub fn with_telemetry(mut self) -> Result<Self, Box<dyn std::error::Error>> {
        let telemetry_provider = TelemetryProvider::init(self.config.telemetry_config.clone())?;
        self.metrics = Some(telemetry_provider.metrics());
        Ok(self)
    }

    /// Start the collector
    pub async fn start(
        &mut self,
        event_receiver: mpsc::Receiver<FeedbackEvent>,
    ) -> Result<(), KafkaError> {
        // Validate configuration before starting
        if let Err(e) = self.config.validate() {
            return Err(KafkaError::ConfigError(format!(
                "Invalid configuration: {}",
                e
            )));
        }

        info!(
            "Starting feedback collector with {} workers",
            self.config.num_workers
        );

        // Initialize backpressure storage
        if let Err(e) = self.backpressure_handler.initialize_storage().await {
            warn!("Failed to initialize backpressure storage: {}", e);
        }

        // Recover events from disk if enabled
        if self.config.enable_recovery {
            match self.backpressure_handler.recover_from_disk().await {
                Ok(events) => {
                    if !events.is_empty() {
                        info!(
                            "Recovered {} events from disk, re-submitting...",
                            events.len()
                        );
                        for event in events {
                            // Try to re-submit recovered events
                            if let Err(e) = self.event_sender.try_send(event.clone()) {
                                warn!("Failed to re-submit recovered event: {}", e);
                                // Re-persist if can't submit
                                if let Err(pe) = self.backpressure_handler.handle_overflow(event).await
                                {
                                    error!("Failed to re-persist event: {}", pe);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to recover events from disk: {}", e);
                }
            }
        }

        // Create Kafka producer
        let producer = Arc::new(FeedbackProducer::new(self.config.kafka_config.clone())?);
        self.kafka_producer = Some(producer.clone());

        // Initialize health checker
        let stats = self.stats.clone();
        let buffer_size = self.config.buffer_size;
        let num_workers = self.config.num_workers;

        let health_checker = HealthChecker::new(buffer_size, num_workers)
            .with_kafka_producer(producer.clone())
            .with_collector_stats(move || CollectorStats {
                events_received: stats.get("events_received").map(|v| *v).unwrap_or(0),
                events_published: stats.get("events_published").map(|v| *v).unwrap_or(0),
                events_failed: stats.get("events_failed").map(|v| *v).unwrap_or(0),
                batches_processed: stats.get("batches_processed").map(|v| *v).unwrap_or(0),
                current_buffer_size: 0, // Would need channel-based tracking
                events_rate_limited: stats.get("events_rate_limited").map(|v| *v).unwrap_or(0),
                events_dropped: stats.get("events_dropped").map(|v| *v).unwrap_or(0),
                events_persisted: stats.get("events_persisted").map(|v| *v).unwrap_or(0),
                events_rejected: stats.get("events_rejected").map(|v| *v).unwrap_or(0),
                buffer_utilization_percent: 0.0,
            });

        self.health_checker = Some(Arc::new(health_checker));

        // Start worker
        let handle = self.spawn_worker(event_receiver, producer)?;
        self.worker_handles.push(handle);

        if let Some(metrics) = &self.metrics {
            metrics.increment_active_collectors();
        }

        Ok(())
    }

    /// Spawn a worker task
    fn spawn_worker(
        &self,
        mut event_receiver: mpsc::Receiver<FeedbackEvent>,
        producer: Arc<FeedbackProducer>,
    ) -> Result<JoinHandle<()>, KafkaError> {
        let config = self.config.clone();
        let stats = self.stats.clone();
        let metrics = self.metrics.clone();

        let handle = tokio::spawn(async move {
            let mut batch: Vec<FeedbackEvent> = Vec::with_capacity(config.max_batch_size);
            let mut batch_created_at = Instant::now();
            let mut flush_timer = interval(Duration::from_millis(config.flush_interval_ms));

            loop {
                tokio::select! {
                    // Receive event
                    Some(event) = event_receiver.recv() => {
                        // Record metrics
                        if let Some(ref m) = metrics {
                            m.record_event(&event);
                        }

                        // Increment stats
                        stats.entry("events_received".to_string())
                            .and_modify(|v| *v += 1)
                            .or_insert(1);

                        batch.push(event);

                        // Check if batch is full
                        if batch.len() >= config.max_batch_size {
                            Self::flush_batch(
                                &mut batch,
                                &mut batch_created_at,
                                &producer,
                                &stats,
                                &metrics,
                            )
                            .await;
                        }
                    }

                    // Flush timer tick
                    _ = flush_timer.tick() => {
                        if !batch.is_empty() {
                            let age = batch_created_at.elapsed();
                            if age.as_secs() >= config.max_batch_age_secs {
                                debug!("Flushing batch due to age: {:?}", age);
                                Self::flush_batch(
                                    &mut batch,
                                    &mut batch_created_at,
                                    &producer,
                                    &stats,
                                    &metrics,
                                )
                                .await;
                            }
                        }
                    }

                    // Channel closed
                    else => {
                        info!("Event channel closed, flushing remaining events");
                        if !batch.is_empty() {
                            Self::flush_batch(
                                &mut batch,
                                &mut batch_created_at,
                                &producer,
                                &stats,
                                &metrics,
                            )
                            .await;
                        }
                        break;
                    }
                }
            }

            if let Some(ref m) = metrics {
                m.decrement_active_collectors();
            }

            info!("Worker shutdown complete");
        });

        Ok(handle)
    }

    /// Flush batch to Kafka
    async fn flush_batch(
        batch: &mut Vec<FeedbackEvent>,
        batch_created_at: &mut Instant,
        producer: &FeedbackProducer,
        stats: &Arc<DashMap<String, u64>>,
        metrics: &Option<Arc<TelemetryMetrics>>,
    ) {
        if batch.is_empty() {
            return;
        }

        let start = Instant::now();
        let batch_size = batch.len();

        let event_batch = FeedbackEventBatch::new(batch.drain(..).collect());

        // Record batch size
        if let Some(ref m) = metrics {
            m.record_batch_size(batch_size as u64);
        }

        // Publish batch
        let (success_count, error_count) = producer.publish_batch_resilient(&event_batch).await;

        // Update stats
        stats
            .entry("events_published".to_string())
            .and_modify(|v| *v += success_count as u64)
            .or_insert(success_count as u64);

        stats
            .entry("events_failed".to_string())
            .and_modify(|v| *v += error_count as u64)
            .or_insert(error_count as u64);

        stats
            .entry("batches_processed".to_string())
            .and_modify(|v| *v += 1)
            .or_insert(1);

        // Record processing latency
        let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
        if let Some(ref m) = metrics {
            m.record_processing_latency(duration_ms, "batch");
            m.record_kafka_publish_latency(duration_ms, error_count == 0);
        }

        debug!(
            "Flushed batch: {} events, {} succeeded, {} failed, took {:.2}ms",
            batch_size, success_count, error_count, duration_ms
        );

        // Reset batch timer
        *batch_created_at = Instant::now();
    }

    /// Submit feedback event (with rate limiting and backpressure handling)
    pub async fn submit(&self, event: FeedbackEvent) -> Result<(), SubmitError> {
        // Check rate limit if enabled
        if let Some(ref limiter) = self.rate_limiter {
            if let Err(e) = limiter.check_limit(&event).await {
                self.stats
                    .entry("events_rate_limited".to_string())
                    .and_modify(|v| *v += 1)
                    .or_insert(1);

                // Record rate limit metric
                if let Some(ref m) = self.metrics {
                    m.record_rate_limit_check(false);
                }

                return Err(SubmitError::RateLimitExceeded(e));
            }

            // Record successful rate limit check
            if let Some(ref m) = self.metrics {
                m.record_rate_limit_check(true);
            }
        }

        // Update buffer size tracking
        let current_size = self.event_sender.max_capacity() - self.event_sender.capacity();
        self.backpressure_handler.update_buffer_size(current_size);

        // Try to send
        match self.event_sender.try_send(event.clone()) {
            Ok(_) => Ok(()),
            Err(mpsc::error::TrySendError::Full(event)) => {
                // Buffer full - apply overflow strategy
                warn!("Buffer full, applying backpressure strategy");
                self.backpressure_handler.handle_overflow(event).await?;

                // Update stats based on strategy
                let bp_stats = self.backpressure_handler.stats();
                self.stats.insert(
                    "events_dropped".to_string(),
                    bp_stats.total_dropped_oldest + bp_stats.total_dropped_newest,
                );
                self.stats
                    .insert("events_persisted".to_string(), bp_stats.total_persisted);
                self.stats
                    .insert("events_rejected".to_string(), bp_stats.total_rejected);

                Ok(())
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                error!("Event channel closed");
                Err(SubmitError::ChannelSend("Channel closed".to_string()))
            }
        }
    }

    /// Submit feedback event (non-blocking, returns error if buffer is full)
    pub fn try_submit(&self, event: FeedbackEvent) -> Result<(), TrySubmitError> {
        // Note: Rate limiting is async, so it cannot be done in try_submit
        // Users should use submit() for rate-limited submissions

        // Update buffer size tracking
        let current_size = self.event_sender.max_capacity() - self.event_sender.capacity();
        self.backpressure_handler.update_buffer_size(current_size);

        match self.event_sender.try_send(event) {
            Ok(_) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => {
                Err(TrySubmitError::Backpressure(BackpressureError::BufferFull))
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Err(TrySubmitError::ChannelClosed),
        }
    }

    /// Get current statistics
    pub fn stats(&self) -> CollectorStats {
        let bp_stats = self.backpressure_handler.stats();

        CollectorStats {
            events_received: self
                .stats
                .get("events_received")
                .map(|v| *v)
                .unwrap_or(0),
            events_published: self
                .stats
                .get("events_published")
                .map(|v| *v)
                .unwrap_or(0),
            events_failed: self.stats.get("events_failed").map(|v| *v).unwrap_or(0),
            batches_processed: self
                .stats
                .get("batches_processed")
                .map(|v| *v)
                .unwrap_or(0),
            current_buffer_size: bp_stats.current_buffer_size,
            events_rate_limited: self
                .stats
                .get("events_rate_limited")
                .map(|v| *v)
                .unwrap_or(0),
            events_dropped: bp_stats.total_dropped_oldest + bp_stats.total_dropped_newest,
            events_persisted: bp_stats.total_persisted,
            events_rejected: bp_stats.total_rejected,
            buffer_utilization_percent: bp_stats.utilization_percent,
        }
    }

    /// Get backpressure statistics
    pub fn backpressure_stats(&self) -> crate::backpressure::BackpressureStats {
        self.backpressure_handler.stats()
    }

    /// Get rate limiter (if enabled)
    pub fn rate_limiter(&self) -> Option<&Arc<RateLimiter>> {
        self.rate_limiter.as_ref()
    }

    /// Get health checker reference
    pub fn health_checker(&self) -> Option<Arc<HealthChecker>> {
        self.health_checker.clone()
    }

    /// Get Kafka producer reference (for circuit breaker access)
    pub fn kafka_producer(&self) -> Option<&Arc<FeedbackProducer>> {
        self.kafka_producer.as_ref()
    }

    /// Shutdown the collector
    pub async fn shutdown(self) -> Result<(), Box<dyn std::error::Error>> {
        use tokio::time::timeout;

        info!("Shutting down feedback collector");

        // Drop sender to close channel
        drop(self.event_sender);

        // Wait for all workers to finish with timeout
        let shutdown_timeout = Duration::from_secs(self.config.shutdown_timeout_secs);
        let shutdown_future = async {
            let mut errors = Vec::new();
            for (idx, handle) in self.worker_handles.into_iter().enumerate() {
                match handle.await {
                    Ok(_) => debug!("Worker {} shutdown successfully", idx),
                    Err(e) => {
                        error!("Worker {} failed during shutdown: {}", idx, e);
                        errors.push(e);
                    }
                }
            }
            errors
        };

        match timeout(shutdown_timeout, shutdown_future).await {
            Ok(errors) => {
                if !errors.is_empty() {
                    warn!(
                        "Collector shutdown completed with {} worker errors",
                        errors.len()
                    );
                } else {
                    info!("Feedback collector shutdown complete");
                }
                Ok(())
            }
            Err(_) => {
                error!(
                    "Collector shutdown timed out after {} seconds",
                    self.config.shutdown_timeout_secs
                );
                Err("Shutdown timeout exceeded".into())
            }
        }
    }
}

/// Feedback collector builder
pub struct FeedbackCollectorBuilder {
    config: FeedbackCollectorConfig,
    enable_telemetry: bool,
}

impl FeedbackCollectorBuilder {
    /// Create new builder
    pub fn new() -> Self {
        Self {
            config: FeedbackCollectorConfig::default(),
            enable_telemetry: false,
        }
    }

    /// Set buffer size
    pub fn buffer_size(mut self, size: usize) -> Self {
        self.config.buffer_size = size;
        self
    }

    /// Set max batch size
    pub fn max_batch_size(mut self, size: usize) -> Self {
        self.config.max_batch_size = size;
        self
    }

    /// Set Kafka brokers
    pub fn kafka_brokers(mut self, brokers: impl Into<String>) -> Self {
        self.config.kafka_config.brokers = brokers.into();
        self
    }

    /// Set Kafka topic
    pub fn kafka_topic(mut self, topic: impl Into<String>) -> Self {
        self.config.kafka_config.topic = topic.into();
        self
    }

    /// Enable telemetry
    pub fn with_telemetry(mut self, enable: bool) -> Self {
        self.enable_telemetry = enable;
        self
    }

    /// Set rate limit configuration
    pub fn rate_limit_config(mut self, config: RateLimitConfig) -> Self {
        self.config.rate_limit_config = Some(config);
        self
    }

    /// Disable rate limiting
    pub fn without_rate_limiting(mut self) -> Self {
        self.config.rate_limit_config = None;
        self
    }

    /// Set backpressure configuration
    pub fn backpressure_config(mut self, config: BackpressureConfig) -> Self {
        self.config.backpressure_config = config;
        self
    }

    /// Enable recovery from disk
    pub fn with_recovery(mut self, enable: bool) -> Self {
        self.config.enable_recovery = enable;
        self
    }

    /// Build the collector
    pub fn build(self) -> Result<(FeedbackCollector, mpsc::Receiver<FeedbackEvent>), Box<dyn std::error::Error>> {
        let (collector, receiver) = FeedbackCollector::new(self.config);

        if self.enable_telemetry {
            collector.with_telemetry().map(|c| (c, receiver))
        } else {
            Ok((collector, receiver))
        }
    }
}

impl Default for FeedbackCollectorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feedback_events::{PerformanceMetricsEvent, UserFeedbackEvent};
    use uuid::Uuid;

    #[test]
    fn test_collector_config_default() {
        let config = FeedbackCollectorConfig::default();
        assert_eq!(config.buffer_size, 10000);
        assert_eq!(config.max_batch_size, 100);
        assert_eq!(config.num_workers, 4);
        assert!(config.enable_recovery);
    }

    #[test]
    fn test_collector_creation() {
        let config = FeedbackCollectorConfig::default();
        let (collector, _rx) = FeedbackCollector::new(config);
        let stats = collector.stats();
        assert_eq!(stats.events_received, 0);
    }

    #[test]
    fn test_builder_pattern() {
        let result = FeedbackCollectorBuilder::new()
            .buffer_size(5000)
            .max_batch_size(50)
            .kafka_brokers("localhost:9092")
            .kafka_topic("test-events")
            .with_recovery(true)
            .build();

        assert!(result.is_ok());
        let (_collector, _rx) = result.unwrap();
    }

    #[tokio::test]
    async fn test_event_submission() {
        let (collector, _rx) = FeedbackCollectorBuilder::new()
            .buffer_size(100)
            .without_rate_limiting()
            .build()
            .unwrap();

        let request_id = Uuid::new_v4();
        let event = FeedbackEvent::UserFeedback(UserFeedbackEvent::new("session1", request_id));

        // Should succeed (channel has capacity)
        let result = collector.try_submit(event);
        if result.is_err() {
            eprintln!("Error submitting event: {:?}", result);
        }
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_stats_tracking() {
        let config = FeedbackCollectorConfig::default();
        let (collector, _rx) = FeedbackCollector::new(config);

        let initial_stats = collector.stats();
        assert_eq!(initial_stats.events_received, 0);
        assert_eq!(initial_stats.events_published, 0);
        assert_eq!(initial_stats.events_dropped, 0);
    }

    #[tokio::test]
    async fn test_backpressure_stats() {
        let config = FeedbackCollectorConfig::default();
        let (collector, _rx) = FeedbackCollector::new(config);

        let bp_stats = collector.backpressure_stats();
        assert_eq!(bp_stats.current_buffer_size, 0);
        assert_eq!(bp_stats.total_persisted, 0);
    }
}
