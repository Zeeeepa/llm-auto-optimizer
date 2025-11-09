//! Kafka integration for stream processing
//!
//! This module provides Kafka source and sink implementations for ingesting
//! and producing events in the stream processing pipeline.
//!
//! # Features
//!
//! ## KafkaSource (Consumer)
//! - Robust Kafka consumer with metrics and offset management
//! - Automatic reconnection on errors
//! - Configurable commit strategies (auto, manual, periodic, on-checkpoint)
//! - Multiple partition support with rebalancing
//! - Graceful shutdown handling
//! - Message deserialization with error handling
//! - Comprehensive metrics tracking (throughput, lag, etc.)
//!
//! ## KafkaSink (Producer)
//! - High-performance Kafka producer with batching and compression
//! - Idempotent producer support
//! - Transactional support for exactly-once semantics
//! - Circuit breaker for fault tolerance
//! - Retry logic with exponential backoff
//! - Configurable partitioning strategies
//! - Comprehensive metrics tracking (latency, throughput, errors)
//!
//! ## Configuration
//! - Comprehensive configuration types for source and sink
//! - Support for all rdkafka settings
//! - Security configuration (SASL, SSL)
//! - Performance tuning options
//!
//! ## Error Handling
//! - Kafka-specific error types with proper context
//! - Error severity levels for monitoring
//! - Retryable vs fatal error classification
//!
//! # Example: Source Usage
//!
//! ```rust,no_run
//! use processor::kafka::{KafkaSource, KafkaSourceConfig, CommitStrategy};
//! use serde::{Deserialize, Serialize};
//! use tokio::sync::mpsc;
//!
//! #[derive(Clone, Debug, Serialize, Deserialize)]
//! struct MyEvent {
//!     value: f64,
//!     timestamp: i64,
//! }
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Configure Kafka source
//! let config = KafkaSourceConfig {
//!     brokers: "localhost:9092".to_string(),
//!     group_id: "my-consumer-group".to_string(),
//!     topics: vec!["my-topic".to_string()],
//!     commit_strategy: CommitStrategy::Periodic,
//!     commit_interval_secs: Some(5),
//!     ..Default::default()
//! };
//!
//! // Create Kafka source
//! let source = KafkaSource::<MyEvent>::new(config, None).await?;
//!
//! // Create channel for messages
//! let (tx, mut rx) = mpsc::channel(1000);
//!
//! // Start consuming in background
//! let source_handle = tokio::spawn(async move {
//!     source.start(tx).await
//! });
//!
//! // Process messages
//! while let Some(msg) = rx.recv().await {
//!     println!("Received: {:?}", msg.payload);
//! }
//!
//! # Ok(())
//! # }
//! ```
//!
//! # Example: Sink Usage
//!
//! ```rust,no_run
//! use processor::kafka::{KafkaSink, KafkaSinkConfig, KafkaMessage};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Clone, Debug, Serialize, Deserialize)]
//! struct MetricResult {
//!     avg_latency: f64,
//!     count: u64,
//! }
//!
//! # async fn example() -> anyhow::Result<()> {
//! let config = KafkaSinkConfig {
//!     brokers: "localhost:9092".to_string(),
//!     topic: "results".to_string(),
//!     enable_idempotence: true,
//!     ..Default::default()
//! };
//!
//! let sink = KafkaSink::<MetricResult>::new(config).await?;
//!
//! let message = KafkaMessage::new(MetricResult {
//!     avg_latency: 123.45,
//!     count: 1000,
//! }).with_key("service-a".to_string());
//!
//! sink.send(message).await?;
//! sink.close().await?;
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod error;
pub mod integration;
pub mod offset_manager;
pub mod sink;
pub mod source;

#[cfg(test)]
mod tests;

// Re-export config types
pub use config::{
    CompressionType, KafkaSinkConfig as DetailedKafkaSinkConfig,
    KafkaSourceConfig as DetailedKafkaSourceConfig, SaslMechanism, SecurityProtocol, SslConfig,
};

// Re-export error types
pub use error::{ErrorSeverity, KafkaError, Result as KafkaResult};

// Re-export offset manager types
pub use offset_manager::{
    InMemoryOffsetStore, OffsetCommitStrategy, OffsetInfo, OffsetManager, OffsetResetStrategy,
    OffsetStats, OffsetStore, StateBackendOffsetStore, TopicPartition,
};

// Re-export source types
pub use source::{
    CommitStrategy, KafkaMessage as SourceMessage, KafkaSource, KafkaSourceConfig,
    KafkaSourceMetrics,
};

// Re-export sink types
pub use sink::{
    BincodeSerializer, DeliveryGuarantee, JsonSerializer, KafkaMessage as SinkMessage,
    KafkaSink, KafkaSinkConfig, MessageSerializer, PartitionStrategy, SinkMetrics,
};

// Re-export integration types
pub use integration::{
    KafkaStreamConfig, KafkaStreamPipeline, KafkaStreamSink, KafkaStreamSource,
    KafkaStreamStats, OffsetManager as StreamOffsetManager,
};
