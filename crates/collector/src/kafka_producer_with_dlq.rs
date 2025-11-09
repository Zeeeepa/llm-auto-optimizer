//! Kafka Producer with DLQ Integration
//!
//! This module provides an integrated Kafka producer that automatically
//! routes failed events to the dead letter queue.

use std::sync::Arc;
use tracing::{error, warn};

use crate::feedback_events::{FeedbackEvent, FeedbackEventBatch};
use crate::kafka_client::{FeedbackProducer, KafkaError};
use crate::dead_letter_queue::{DLQManager, DLQError};

/// Result type for producer with DLQ
pub type Result<T> = std::result::Result<T, ProducerWithDLQError>;

/// Error types for producer with DLQ
#[derive(Debug, thiserror::Error)]
pub enum ProducerWithDLQError {
    #[error("Kafka error: {0}")]
    KafkaError(#[from] KafkaError),

    #[error("DLQ error: {0}")]
    DLQError(#[from] DLQError),
}

/// Kafka producer integrated with DLQ
pub struct KafkaProducerWithDLQ {
    producer: Arc<FeedbackProducer>,
    dlq_manager: Option<Arc<DLQManager>>,
}

impl KafkaProducerWithDLQ {
    /// Create new producer with DLQ
    pub fn new(
        producer: Arc<FeedbackProducer>,
        dlq_manager: Option<Arc<DLQManager>>,
    ) -> Self {
        Self {
            producer,
            dlq_manager,
        }
    }

    /// Publish single event with DLQ fallback
    pub async fn publish_event(&self, event: &FeedbackEvent) -> Result<()> {
        match self.producer.publish_event(event).await {
            Ok(_) => Ok(()),
            Err(e) => {
                // Determine if error is transient or permanent
                if self.is_permanent_failure(&e) {
                    warn!("Permanent Kafka failure for event {}: {}", event.id(), e);
                    self.send_to_dlq(event.clone(), e.to_string()).await?;
                } else {
                    error!("Transient Kafka failure for event {}: {}", event.id(), e);
                    // For transient failures, we might want to retry here
                    // or let the caller handle it
                    return Err(ProducerWithDLQError::KafkaError(e));
                }
                Ok(())
            }
        }
    }

    /// Publish batch with DLQ fallback
    pub async fn publish_batch(&self, batch: &FeedbackEventBatch) -> Result<Vec<Result<()>>> {
        let results = self.producer.publish_batch(batch).await?;

        let mut output_results = Vec::with_capacity(results.len());

        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(_) => output_results.push(Ok(())),
                Err(e) => {
                    if self.is_permanent_failure(&e) {
                        let event = &batch.events[i];
                        warn!("Permanent Kafka failure for event {}: {}", event.id(), e);
                        self.send_to_dlq(event.clone(), e.to_string()).await?;
                        output_results.push(Ok(())); // DLQ handled it
                    } else {
                        output_results.push(Err(ProducerWithDLQError::KafkaError(e)));
                    }
                }
            }
        }

        Ok(output_results)
    }

    /// Publish batch with resilient error handling
    pub async fn publish_batch_resilient(&self, batch: &FeedbackEventBatch) -> (usize, usize, usize) {
        let mut success_count = 0;
        let mut dlq_count = 0;
        let mut error_count = 0;

        for event in &batch.events {
            match self.producer.publish_event(event).await {
                Ok(_) => success_count += 1,
                Err(e) => {
                    if self.is_permanent_failure(&e) {
                        match self.send_to_dlq(event.clone(), e.to_string()).await {
                            Ok(_) => dlq_count += 1,
                            Err(dlq_err) => {
                                error!("Failed to send to DLQ: {}", dlq_err);
                                error_count += 1;
                            }
                        }
                    } else {
                        error_count += 1;
                    }
                }
            }
        }

        (success_count, dlq_count, error_count)
    }

    /// Send event to DLQ
    async fn send_to_dlq(&self, event: FeedbackEvent, reason: String) -> Result<()> {
        if let Some(dlq) = &self.dlq_manager {
            dlq.add_failed_event(event, reason).await?;
        } else {
            warn!("DLQ not configured, dropping failed event");
        }
        Ok(())
    }

    /// Determine if a Kafka error is a permanent failure
    fn is_permanent_failure(&self, error: &KafkaError) -> bool {
        match error {
            // Timeout errors might be transient
            KafkaError::TimeoutError(_) => false,

            // Configuration errors are permanent
            KafkaError::ConfigError(_) => true,

            // Serialization errors are permanent (bad data)
            KafkaError::SerializationError(_) => true,

            // Circuit breaker is a transient failure - retry later
            KafkaError::CircuitBreakerOpen => false,

            // Publish errors might be transient (network issues)
            KafkaError::PublishError(msg) => {
                // Check error message for specific conditions
                if msg.contains("timeout") || msg.contains("connection") {
                    false // Transient
                } else if msg.contains("invalid") || msg.contains("malformed") {
                    true // Permanent
                } else {
                    // Default to transient for safety
                    false
                }
            }

            // RdKafka errors - analyze based on error code
            KafkaError::RdKafkaError(e) => {
                use rdkafka::error::RDKafkaErrorCode;
                match e {
                    rdkafka::error::KafkaError::MessageProduction(code) => {
                        matches!(
                            code,
                            RDKafkaErrorCode::InvalidMessage
                                | RDKafkaErrorCode::MessageSizeTooLarge
                                | RDKafkaErrorCode::InvalidMessageSize
                        )
                    }
                    _ => false, // Default to transient
                }
            }
        }
    }

    /// Get DLQ statistics if available
    pub async fn dlq_stats(&self) -> Option<crate::dead_letter_queue::DLQStats> {
        if let Some(dlq) = &self.dlq_manager {
            dlq.stats().await.ok()
        } else {
            None
        }
    }

    /// Flush pending Kafka messages
    pub async fn flush(&self, timeout_ms: u64) -> Result<()> {
        self.producer.flush(timeout_ms).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kafka_client::KafkaProducerConfig;

    #[test]
    fn test_is_permanent_failure() {
        let config = KafkaProducerConfig::default();
        let producer = Arc::new(FeedbackProducer::new(config).unwrap());
        let producer_with_dlq = KafkaProducerWithDLQ::new(producer, None);

        // Test serialization error (permanent)
        let error = KafkaError::SerializationError("Invalid JSON".to_string());
        assert!(producer_with_dlq.is_permanent_failure(&error));

        // Test timeout error (transient)
        let error = KafkaError::TimeoutError("Request timeout".to_string());
        assert!(!producer_with_dlq.is_permanent_failure(&error));

        // Test config error (permanent)
        let error = KafkaError::ConfigError("Invalid broker".to_string());
        assert!(producer_with_dlq.is_permanent_failure(&error));
    }
}
