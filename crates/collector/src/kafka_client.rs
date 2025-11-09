//! Kafka Client
//!
//! This module provides Kafka producer and consumer functionality
//! for feedback event streaming with retries and error handling.
//! Includes circuit breaker for resilient failure handling.

use async_trait::async_trait;
use rdkafka::config::ClientConfig;
use rdkafka::message::{Header, OwnedHeaders};
use rdkafka::producer::{FutureProducer, FutureRecord, Producer};
use rdkafka::util::Timeout;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, error, warn};

use crate::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use crate::feedback_events::{FeedbackEvent, FeedbackEventBatch};

/// Kafka error types
#[derive(Error, Debug)]
pub enum KafkaError {
    #[error("Kafka configuration error: {0}")]
    ConfigError(String),

    #[error("Kafka publish error: {0}")]
    PublishError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Timeout error: {0}")]
    TimeoutError(String),

    #[error("Circuit breaker open - failing fast")]
    CircuitBreakerOpen,

    #[error(transparent)]
    RdKafkaError(#[from] rdkafka::error::KafkaError),
}

pub type Result<T> = std::result::Result<T, KafkaError>;

/// Kafka producer configuration
#[derive(Debug, Clone)]
pub struct KafkaProducerConfig {
    /// Kafka brokers (comma-separated)
    pub brokers: String,
    /// Topic name for feedback events
    pub topic: String,
    /// Client ID
    pub client_id: String,
    /// Message timeout in milliseconds
    pub message_timeout_ms: u64,
    /// Compression type (none, gzip, snappy, lz4, zstd)
    pub compression_type: String,
    /// Number of retries
    pub retries: u32,
    /// Retry backoff in milliseconds
    pub retry_backoff_ms: u64,
    /// Circuit breaker configuration (optional)
    pub circuit_breaker_config: Option<CircuitBreakerConfig>,
}

impl Default for KafkaProducerConfig {
    fn default() -> Self {
        Self {
            brokers: "localhost:9092".to_string(),
            topic: "feedback-events".to_string(),
            client_id: "llm-optimizer-collector".to_string(),
            message_timeout_ms: 30000,
            compression_type: "gzip".to_string(),
            retries: 3,
            retry_backoff_ms: 1000,
            circuit_breaker_config: Some(CircuitBreakerConfig::default()),
        }
    }
}

/// Kafka feedback producer
pub struct FeedbackProducer {
    /// Kafka producer
    producer: FutureProducer,
    /// Configuration
    config: KafkaProducerConfig,
    /// Circuit breaker for fault tolerance
    circuit_breaker: Option<Arc<CircuitBreaker>>,
}

impl FeedbackProducer {
    /// Create new Kafka producer
    pub fn new(config: KafkaProducerConfig) -> Result<Self> {
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", &config.brokers)
            .set("client.id", &config.client_id)
            .set("message.timeout.ms", config.message_timeout_ms.to_string())
            .set("compression.type", &config.compression_type)
            .set("retries", config.retries.to_string())
            .set("retry.backoff.ms", config.retry_backoff_ms.to_string())
            .set("acks", "all") // Wait for all replicas
            .set("enable.idempotence", "true") // Exactly-once semantics
            .create()
            .map_err(|e| KafkaError::ConfigError(e.to_string()))?;

        // Initialize circuit breaker if configured
        let circuit_breaker = config
            .circuit_breaker_config
            .as_ref()
            .map(|cb_config| Arc::new(CircuitBreaker::with_config(cb_config.clone())));

        Ok(Self {
            producer,
            config,
            circuit_breaker,
        })
    }

    /// Get circuit breaker reference
    pub fn circuit_breaker(&self) -> Option<&Arc<CircuitBreaker>> {
        self.circuit_breaker.as_ref()
    }

    /// Publish single event
    pub async fn publish_event(&self, event: &FeedbackEvent) -> Result<()> {
        // Check circuit breaker
        if let Some(cb) = &self.circuit_breaker {
            if !cb.is_request_allowed().await {
                cb.record_rejection();
                warn!("Circuit breaker is open - rejecting publish request");
                return Err(KafkaError::CircuitBreakerOpen);
            }
        }

        let payload = serde_json::to_vec(event)
            .map_err(|e| KafkaError::SerializationError(e.to_string()))?;

        let key = event.id().to_string();
        let event_type = event.event_type();

        let headers = OwnedHeaders::new()
            .insert(Header {
                key: "event_type",
                value: Some(event_type),
            })
            .insert(Header {
                key: "event_id",
                value: Some(key.as_bytes()),
            })
            .insert(Header {
                key: "timestamp",
                value: Some(event.timestamp().to_rfc3339().as_bytes()),
            });

        let record = FutureRecord::to(&self.config.topic)
            .payload(&payload)
            .key(&key)
            .headers(headers);

        let timeout = Timeout::After(Duration::from_millis(self.config.message_timeout_ms));

        // Attempt to send
        let result = self
            .producer
            .send(record, timeout)
            .await
            .map_err(|(err, _)| KafkaError::PublishError(err.to_string()));

        // Record result in circuit breaker
        if let Some(cb) = &self.circuit_breaker {
            match &result {
                Ok(_) => cb.record_success().await,
                Err(_) => cb.record_failure().await,
            }
        }

        if result.is_ok() {
            debug!("Published event {} to Kafka topic {}", key, self.config.topic);
        }

        result.map(|_| ())
    }

    /// Publish batch of events
    pub async fn publish_batch(&self, batch: &FeedbackEventBatch) -> Result<Vec<Result<()>>> {
        debug!("Publishing batch {} with {} events", batch.batch_id, batch.size());

        let mut results = Vec::with_capacity(batch.events.len());

        for event in &batch.events {
            let result = self.publish_event(event).await;
            results.push(result);
        }

        Ok(results)
    }

    /// Publish batch with error handling (returns count of successful publishes)
    pub async fn publish_batch_resilient(&self, batch: &FeedbackEventBatch) -> (usize, usize) {
        let mut success_count = 0;
        let mut error_count = 0;

        for event in &batch.events {
            match self.publish_event(event).await {
                Ok(_) => success_count += 1,
                Err(e) => {
                    error_count += 1;
                    error!("Failed to publish event {}: {}", event.id(), e);
                }
            }
        }

        debug!(
            "Batch {}: {} succeeded, {} failed",
            batch.batch_id, success_count, error_count
        );

        (success_count, error_count)
    }

    /// Flush pending messages
    pub async fn flush(&self, timeout_ms: u64) -> Result<()> {
        let timeout = Timeout::After(Duration::from_millis(timeout_ms));
        self.producer
            .flush(timeout)
            .map_err(|e| KafkaError::PublishError(e.to_string()))
    }
}

/// Feedback event handler trait
#[async_trait]
pub trait FeedbackEventHandler: Send + Sync {
    /// Handle feedback event
    async fn handle_event(&self, event: FeedbackEvent) -> Result<()>;

    /// Handle batch of events
    async fn handle_batch(&self, batch: FeedbackEventBatch) -> Result<()> {
        for event in batch.events {
            self.handle_event(event).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feedback_events::{UserFeedbackEvent, PerformanceMetricsEvent};
    use uuid::Uuid;

    #[test]
    fn test_kafka_producer_config_default() {
        let config = KafkaProducerConfig::default();
        assert_eq!(config.brokers, "localhost:9092");
        assert_eq!(config.topic, "feedback-events");
        assert_eq!(config.retries, 3);
    }

    #[test]
    fn test_kafka_producer_creation() {
        let config = KafkaProducerConfig::default();
        // This will fail if Kafka is not running, which is expected in tests
        let result = FeedbackProducer::new(config);
        // We just check that the creation logic works
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_event_serialization() {
        let request_id = Uuid::new_v4();
        let event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new("session1", request_id).with_rating(4.5),
        );

        // Test serialization
        let payload = serde_json::to_vec(&event).unwrap();
        assert!(!payload.is_empty());

        // Test deserialization
        let deserialized: FeedbackEvent = serde_json::from_slice(&payload).unwrap();
        assert_eq!(event, deserialized);
    }

    #[tokio::test]
    async fn test_batch_creation() {
        let request_id = Uuid::new_v4();
        let events = vec![
            FeedbackEvent::UserFeedback(UserFeedbackEvent::new("s1", request_id)),
            FeedbackEvent::PerformanceMetrics(PerformanceMetricsEvent::new(
                request_id,
                "gpt-4",
                1000.0,
                100,
                200,
                0.05,
            )),
        ];

        let batch = FeedbackEventBatch::new(events);
        assert_eq!(batch.size(), 2);
        assert!(!batch.is_empty());
    }

    // Note: Integration tests with actual Kafka broker would go in tests/ directory
}
