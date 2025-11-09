//! Kafka-specific error types and error handling.
//!
//! This module provides comprehensive error types for all Kafka operations,
//! including connection errors, serialization/deserialization errors,
//! consumption errors, and production errors.

use std::fmt;
use thiserror::Error;

/// Result type alias for Kafka operations.
pub type Result<T> = std::result::Result<T, KafkaError>;

/// Comprehensive error type for Kafka operations.
///
/// This enum covers all possible error scenarios when working with Kafka,
/// providing detailed context for debugging and error handling.
///
/// # Examples
///
/// ```rust
/// use processor::kafka::error::{KafkaError, Result};
///
/// fn connect_to_kafka() -> Result<()> {
///     // Simulate connection error
///     Err(KafkaError::Connection {
///         message: "Failed to connect to broker".to_string(),
///         broker: "localhost:9092".to_string(),
///     })
/// }
///
/// match connect_to_kafka() {
///     Ok(_) => println!("Connected successfully"),
///     Err(KafkaError::Connection { message, broker }) => {
///         eprintln!("Connection error to {}: {}", broker, message);
///     }
///     Err(e) => eprintln!("Other error: {}", e),
/// }
/// ```
#[derive(Error, Debug)]
pub enum KafkaError {
    /// Error establishing connection to Kafka broker.
    #[error("Failed to connect to Kafka broker {broker}: {message}")]
    Connection {
        /// Error message describing the connection failure.
        message: String,
        /// Broker address that failed to connect.
        broker: String,
    },

    /// Error during message consumption.
    #[error("Failed to consume message from topic {topic}: {message}")]
    Consumption {
        /// Error message describing the consumption failure.
        message: String,
        /// Topic from which consumption failed.
        topic: String,
        /// Partition number if available.
        partition: Option<i32>,
        /// Offset if available.
        offset: Option<i64>,
    },

    /// Error during message production.
    #[error("Failed to produce message to topic {topic}: {message}")]
    Production {
        /// Error message describing the production failure.
        message: String,
        /// Topic to which production failed.
        topic: String,
        /// Partition number if available.
        partition: Option<i32>,
    },

    /// Error during serialization of messages.
    #[error("Serialization error: {message}")]
    Serialization {
        /// Error message describing the serialization failure.
        message: String,
        /// Optional context about what was being serialized.
        context: Option<String>,
    },

    /// Error during deserialization of messages.
    #[error("Deserialization error: {message}")]
    Deserialization {
        /// Error message describing the deserialization failure.
        message: String,
        /// Optional context about what was being deserialized.
        context: Option<String>,
        /// Raw bytes that failed to deserialize (truncated for display).
        payload_sample: Option<Vec<u8>>,
    },

    /// Configuration validation error.
    #[error("Invalid configuration: {message}")]
    InvalidConfiguration {
        /// Error message describing the configuration issue.
        message: String,
        /// Field name that has invalid configuration.
        field: Option<String>,
    },

    /// Error committing offsets.
    #[error("Failed to commit offsets for topic {topic}: {message}")]
    OffsetCommit {
        /// Error message describing the commit failure.
        message: String,
        /// Topic for which offset commit failed.
        topic: String,
        /// Partition for which offset commit failed.
        partition: i32,
        /// Offset that failed to commit.
        offset: i64,
    },

    /// Error during consumer group coordination.
    #[error("Consumer group coordination error for group {group_id}: {message}")]
    GroupCoordination {
        /// Error message describing the coordination failure.
        message: String,
        /// Consumer group ID.
        group_id: String,
    },

    /// Error during partition assignment.
    #[error("Partition assignment error: {message}")]
    PartitionAssignment {
        /// Error message describing the assignment failure.
        message: String,
        /// Topics involved in the assignment.
        topics: Vec<String>,
    },

    /// Authentication or authorization error.
    #[error("Authentication/Authorization error: {message}")]
    AuthError {
        /// Error message describing the auth failure.
        message: String,
        /// Security protocol being used.
        protocol: String,
    },

    /// Network or I/O error.
    #[error("Network I/O error: {message}")]
    IoError {
        /// Error message describing the I/O failure.
        message: String,
        /// Optional underlying I/O error.
        #[source]
        source: Option<std::io::Error>,
    },

    /// Timeout error during Kafka operations.
    #[error("Operation timed out after {timeout_ms}ms: {operation}")]
    Timeout {
        /// Description of the operation that timed out.
        operation: String,
        /// Timeout duration in milliseconds.
        timeout_ms: u64,
    },

    /// Error related to topic metadata.
    #[error("Topic metadata error for topic {topic}: {message}")]
    TopicMetadata {
        /// Error message describing the metadata failure.
        message: String,
        /// Topic name.
        topic: String,
    },

    /// Error when a topic partition doesn't exist.
    #[error("Topic partition {topic}:{partition} does not exist")]
    PartitionNotFound {
        /// Topic name.
        topic: String,
        /// Partition number.
        partition: i32,
    },

    /// Error when reaching resource limits.
    #[error("Resource limit exceeded: {message}")]
    ResourceExhausted {
        /// Error message describing the resource exhaustion.
        message: String,
        /// Resource type (e.g., "buffer", "connections").
        resource: String,
    },

    /// Error from the underlying rdkafka library.
    #[error("Kafka client error: {message} (error code: {code:?})")]
    KafkaClient {
        /// Error message from rdkafka.
        message: String,
        /// rdkafka error code.
        code: Option<i32>,
    },

    /// Generic error for cases not covered by specific variants.
    #[error("Kafka error: {0}")]
    Other(String),
}

impl KafkaError {
    /// Create a connection error.
    pub fn connection(message: impl Into<String>, broker: impl Into<String>) -> Self {
        Self::Connection {
            message: message.into(),
            broker: broker.into(),
        }
    }

    /// Create a consumption error.
    pub fn consumption(
        message: impl Into<String>,
        topic: impl Into<String>,
        partition: Option<i32>,
        offset: Option<i64>,
    ) -> Self {
        Self::Consumption {
            message: message.into(),
            topic: topic.into(),
            partition,
            offset,
        }
    }

    /// Create a production error.
    pub fn production(
        message: impl Into<String>,
        topic: impl Into<String>,
        partition: Option<i32>,
    ) -> Self {
        Self::Production {
            message: message.into(),
            topic: topic.into(),
            partition,
        }
    }

    /// Create a serialization error.
    pub fn serialization(message: impl Into<String>, context: Option<String>) -> Self {
        Self::Serialization {
            message: message.into(),
            context,
        }
    }

    /// Create a deserialization error.
    pub fn deserialization(
        message: impl Into<String>,
        context: Option<String>,
        payload_sample: Option<Vec<u8>>,
    ) -> Self {
        Self::Deserialization {
            message: message.into(),
            context,
            payload_sample,
        }
    }

    /// Create an invalid configuration error.
    pub fn invalid_config(message: impl Into<String>, field: Option<String>) -> Self {
        Self::InvalidConfiguration {
            message: message.into(),
            field,
        }
    }

    /// Create an offset commit error.
    pub fn offset_commit(
        message: impl Into<String>,
        topic: impl Into<String>,
        partition: i32,
        offset: i64,
    ) -> Self {
        Self::OffsetCommit {
            message: message.into(),
            topic: topic.into(),
            partition,
            offset,
        }
    }

    /// Create a group coordination error.
    pub fn group_coordination(message: impl Into<String>, group_id: impl Into<String>) -> Self {
        Self::GroupCoordination {
            message: message.into(),
            group_id: group_id.into(),
        }
    }

    /// Create a partition assignment error.
    pub fn partition_assignment(message: impl Into<String>, topics: Vec<String>) -> Self {
        Self::PartitionAssignment {
            message: message.into(),
            topics,
        }
    }

    /// Create an authentication/authorization error.
    pub fn auth_error(message: impl Into<String>, protocol: impl Into<String>) -> Self {
        Self::AuthError {
            message: message.into(),
            protocol: protocol.into(),
        }
    }

    /// Create a timeout error.
    pub fn timeout(operation: impl Into<String>, timeout_ms: u64) -> Self {
        Self::Timeout {
            operation: operation.into(),
            timeout_ms,
        }
    }

    /// Create a topic metadata error.
    pub fn topic_metadata(message: impl Into<String>, topic: impl Into<String>) -> Self {
        Self::TopicMetadata {
            message: message.into(),
            topic: topic.into(),
        }
    }

    /// Create a partition not found error.
    pub fn partition_not_found(topic: impl Into<String>, partition: i32) -> Self {
        Self::PartitionNotFound {
            topic: topic.into(),
            partition,
        }
    }

    /// Create a resource exhausted error.
    pub fn resource_exhausted(message: impl Into<String>, resource: impl Into<String>) -> Self {
        Self::ResourceExhausted {
            message: message.into(),
            resource: resource.into(),
        }
    }

    /// Create a Kafka client error.
    pub fn kafka_client(message: impl Into<String>, code: Option<i32>) -> Self {
        Self::KafkaClient {
            message: message.into(),
            code,
        }
    }

    /// Check if the error is retryable.
    ///
    /// Returns `true` if the operation that caused this error can be retried.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            KafkaError::Timeout { .. }
                | KafkaError::IoError { .. }
                | KafkaError::Connection { .. }
                | KafkaError::GroupCoordination { .. }
                | KafkaError::ResourceExhausted { .. }
        )
    }

    /// Check if the error is fatal and requires system shutdown.
    ///
    /// Returns `true` if the error indicates an unrecoverable state.
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            KafkaError::AuthError { .. } | KafkaError::InvalidConfiguration { .. }
        )
    }

    /// Get error severity level.
    ///
    /// Returns a severity level suitable for logging and monitoring.
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            KafkaError::InvalidConfiguration { .. } | KafkaError::AuthError { .. } => {
                ErrorSeverity::Critical
            }
            KafkaError::Connection { .. }
            | KafkaError::GroupCoordination { .. }
            | KafkaError::Production { .. } => ErrorSeverity::Error,
            KafkaError::Timeout { .. }
            | KafkaError::ResourceExhausted { .. }
            | KafkaError::Consumption { .. } => ErrorSeverity::Warning,
            _ => ErrorSeverity::Info,
        }
    }
}

/// Error severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrorSeverity {
    /// Informational messages.
    Info,
    /// Warning messages for potential issues.
    Warning,
    /// Error messages for failures.
    Error,
    /// Critical errors requiring immediate attention.
    Critical,
}

impl fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorSeverity::Info => write!(f, "INFO"),
            ErrorSeverity::Warning => write!(f, "WARNING"),
            ErrorSeverity::Error => write!(f, "ERROR"),
            ErrorSeverity::Critical => write!(f, "CRITICAL"),
        }
    }
}

// Conversions from common error types

impl From<std::io::Error> for KafkaError {
    fn from(err: std::io::Error) -> Self {
        KafkaError::IoError {
            message: err.to_string(),
            source: Some(err),
        }
    }
}

impl From<serde_json::Error> for KafkaError {
    fn from(err: serde_json::Error) -> Self {
        KafkaError::Serialization {
            message: err.to_string(),
            context: Some("JSON serialization".to_string()),
        }
    }
}

impl From<bincode::Error> for KafkaError {
    fn from(err: bincode::Error) -> Self {
        KafkaError::Serialization {
            message: err.to_string(),
            context: Some("Bincode serialization".to_string()),
        }
    }
}

// Conversion from rdkafka errors
impl From<rdkafka::error::KafkaError> for KafkaError {
    fn from(err: rdkafka::error::KafkaError) -> Self {
        use rdkafka::error::KafkaError as RdKafkaError;

        match err {
            // Connection-related errors
            RdKafkaError::ClientCreation(msg) => KafkaError::connection(msg, "unknown"),

            // Metadata errors
            RdKafkaError::MetadataFetch(err) => {
                KafkaError::topic_metadata(err.to_string(), "unknown")
            }

            // Subscription errors
            RdKafkaError::Subscription(msg) => {
                KafkaError::partition_assignment(msg, Vec::new())
            }

            // Group coordination errors
            RdKafkaError::GroupListFetch(err) => {
                KafkaError::group_coordination(err.to_string(), "unknown")
            }

            // Message production errors
            RdKafkaError::MessageProduction(code) => KafkaError::production(
                format!("Message production failed with code: {:?}", code),
                "unknown",
                None,
            ),

            // Message consumption errors
            RdKafkaError::MessageConsumption(code) => KafkaError::consumption(
                format!("Message consumption failed with code: {:?}", code),
                "unknown",
                None,
                None,
            ),

            // Offset store/commit errors
            RdKafkaError::StoreOffset(err) | RdKafkaError::OffsetFetch(err) => {
                KafkaError::offset_commit(err.to_string(), "unknown", -1, -1)
            }

            // Partition EOF (not really an error, but can be handled)
            RdKafkaError::PartitionEOF(partition) => KafkaError::consumption(
                format!("Reached end of partition {}", partition),
                "unknown",
                Some(partition),
                None,
            ),

            // Generic error
            _ => KafkaError::kafka_client(err.to_string(), None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = KafkaError::connection("Connection refused", "localhost:9092");
        assert!(matches!(err, KafkaError::Connection { .. }));
    }

    #[test]
    fn test_error_is_retryable() {
        let timeout_err = KafkaError::timeout("poll", 1000);
        assert!(timeout_err.is_retryable());

        let auth_err = KafkaError::auth_error("Invalid credentials", "SASL_SSL");
        assert!(!auth_err.is_retryable());
    }

    #[test]
    fn test_error_is_fatal() {
        let auth_err = KafkaError::auth_error("Invalid credentials", "SASL_SSL");
        assert!(auth_err.is_fatal());

        let timeout_err = KafkaError::timeout("poll", 1000);
        assert!(!timeout_err.is_fatal());
    }

    #[test]
    fn test_error_severity() {
        let critical = KafkaError::invalid_config("Missing bootstrap servers", None);
        assert_eq!(critical.severity(), ErrorSeverity::Critical);

        let error = KafkaError::connection("Connection refused", "localhost:9092");
        assert_eq!(error.severity(), ErrorSeverity::Error);

        let warning = KafkaError::timeout("poll", 1000);
        assert_eq!(warning.severity(), ErrorSeverity::Warning);
    }

    #[test]
    fn test_error_display() {
        let err = KafkaError::connection("Connection refused", "localhost:9092");
        let display = format!("{}", err);
        assert!(display.contains("localhost:9092"));
        assert!(display.contains("Connection refused"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "test error");
        let kafka_err: KafkaError = io_err.into();
        assert!(matches!(kafka_err, KafkaError::IoError { .. }));
    }

    #[test]
    fn test_serde_error_conversion() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let kafka_err: KafkaError = json_err.into();
        assert!(matches!(kafka_err, KafkaError::Serialization { .. }));
    }
}
