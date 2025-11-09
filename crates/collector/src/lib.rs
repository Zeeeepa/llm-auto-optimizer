//! Feedback Collector
//!
//! This crate provides comprehensive feedback collection using OpenTelemetry
//! for observability and Kafka for event streaming. It supports async collection,
//! batching, buffering, resilient event processing, circuit breaker pattern,
//! and backpressure handling for production reliability.

pub mod backpressure;
pub mod circuit_breaker;
pub mod dead_letter_queue;
pub mod feedback_collector;
pub mod feedback_events;
pub mod health;
pub mod kafka_client;
pub mod kafka_producer_with_dlq;
pub mod rate_limiter;
pub mod telemetry;

pub use backpressure::{
    BackpressureConfig, BackpressureError, BackpressureHandler, BackpressureStats, BufferLevel,
    OverflowStrategy,
};

pub use circuit_breaker::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerStats, CircuitState,
};

pub use dead_letter_queue::{
    DLQConfig, DLQEntry, DLQError, DLQManager, DLQStats, DLQType, DeadLetterQueue, FileDLQ,
    HybridDLQ, KafkaDLQ,
};

pub use feedback_collector::{
    CollectorStats, FeedbackCollector, FeedbackCollectorBuilder, FeedbackCollectorConfig,
    SubmitError, TrySubmitError,
};

pub use feedback_events::{
    ExperimentResultEvent, FeedbackEvent, FeedbackEventBatch, HealthStatus, ModelResponseEvent,
    PerformanceMetricsEvent, SystemHealthEvent, UserFeedbackEvent,
};

pub use health::{ComponentHealth, HealthChecker, HealthReport, HealthStatus as HealthCheckStatus};

pub use kafka_client::{FeedbackEventHandler, FeedbackProducer, KafkaError, KafkaProducerConfig};

pub use kafka_producer_with_dlq::{KafkaProducerWithDLQ, ProducerWithDLQError};

pub use rate_limiter::{
    RateLimitConfig, RateLimitError, RateLimitStats, RateLimiter, SourceIdentifier,
    SourceIdentifierStrategy,
};

pub use telemetry::{TelemetryConfig, TelemetryError, TelemetryMetrics, TelemetryProvider};
