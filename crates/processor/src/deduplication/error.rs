//! Error types for event deduplication operations

use crate::error::StateError;
use thiserror::Error;

/// Errors that can occur during deduplication operations
#[derive(Error, Debug)]
pub enum DeduplicationError {
    /// Failed to extract event ID from event
    #[error("ID extraction failed: {reason}")]
    ExtractionFailed { reason: String },

    /// Redis storage operation failed
    #[error("storage error: {source}")]
    StorageError {
        #[from]
        source: StateError,
    },

    /// Redis is unavailable and strategy requires it
    #[error("storage unavailable: {details}")]
    StorageUnavailable { details: String },

    /// Invalid configuration provided
    #[error("invalid configuration: {reason}")]
    InvalidConfig { reason: String },

    /// Serialization failed
    #[error("serialization failed: {reason}")]
    SerializationFailed { reason: String },

    /// Circuit breaker is open, preventing operations
    #[error("circuit breaker open: too many consecutive failures")]
    CircuitBreakerOpen,

    /// Event ID field not found
    #[error("field '{field}' not found in event")]
    FieldNotFound { field: String },

    /// Invalid event ID format
    #[error("invalid event ID format: {reason}")]
    InvalidEventId { reason: String },

    /// Batch operation partially failed
    #[error("batch operation failed: {successful} succeeded, {failed} failed")]
    BatchPartialFailure { successful: usize, failed: usize },
}

/// Result type for deduplication operations
pub type DeduplicationResult<T> = std::result::Result<T, DeduplicationError>;

impl From<serde_json::Error> for DeduplicationError {
    fn from(err: serde_json::Error) -> Self {
        DeduplicationError::SerializationFailed {
            reason: err.to_string(),
        }
    }
}

impl From<bincode::Error> for DeduplicationError {
    fn from(err: bincode::Error) -> Self {
        DeduplicationError::SerializationFailed {
            reason: err.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = DeduplicationError::ExtractionFailed {
            reason: "field missing".to_string(),
        };
        assert!(err.to_string().contains("extraction failed"));

        let err = DeduplicationError::StorageUnavailable {
            details: "Redis down".to_string(),
        };
        assert!(err.to_string().contains("unavailable"));
    }

    #[test]
    fn test_from_serde_error() {
        let json_err = serde_json::from_str::<i32>("invalid").unwrap_err();
        let dedup_err: DeduplicationError = json_err.into();
        assert!(matches!(
            dedup_err,
            DeduplicationError::SerializationFailed { .. }
        ));
    }
}
