//! Error types for the stream processor
//!
//! This module provides comprehensive error handling for all processor operations
//! including windowing, aggregation, state management, and watermark generation.

use thiserror::Error;

/// Main processor error type
#[derive(Error, Debug)]
pub enum ProcessorError {
    /// Window-related errors
    #[error("window error: {0}")]
    Window(#[from] WindowError),

    /// Aggregation-related errors
    #[error("aggregation error: {0}")]
    Aggregation(#[from] AggregationError),

    /// State backend errors
    #[error("state error: {0}")]
    State(#[from] StateError),

    /// Watermark generation errors
    #[error("watermark error: {0}")]
    Watermark(#[from] WatermarkError),

    /// Configuration errors
    #[error("configuration error: {source}")]
    Configuration {
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Execution errors
    #[error("execution error: {source}")]
    Execution {
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Serialization/deserialization errors
    #[error("serialization error: {0}")]
    Serialization(String),

    /// I/O errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Kafka-related errors
    #[error("kafka error: {source}")]
    Kafka {
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Generic error for unexpected conditions
    #[error("unexpected error: {0}")]
    Unexpected(String),
}

/// Window assignment and management errors
#[derive(Error, Debug)]
pub enum WindowError {
    /// Event timestamp is out of valid range
    #[error("invalid event timestamp: {timestamp}, reason: {reason}")]
    InvalidTimestamp { timestamp: i64, reason: String },

    /// Window size is invalid
    #[error("invalid window size: {size}ms, must be greater than 0")]
    InvalidWindowSize { size: u64 },

    /// Slide size is invalid for sliding windows
    #[error("invalid slide size: {slide}ms, must be greater than 0 and less than or equal to window size {window}ms")]
    InvalidSlideSize { slide: u64, window: u64 },

    /// Gap size is invalid for session windows
    #[error("invalid gap size: {gap}ms, must be greater than 0")]
    InvalidGapSize { gap: u64 },

    /// Event arrived too late after watermark
    #[error("late event: event timestamp {event_time} is before watermark {watermark}, late by {late_by}ms")]
    LateEvent {
        event_time: i64,
        watermark: i64,
        late_by: i64,
    },

    /// Window has already been finalized
    #[error("window already closed: window_id={window_id}, close_time={close_time}")]
    WindowClosed { window_id: String, close_time: i64 },

    /// Cannot assign event to window
    #[error("window assignment failed: {reason}")]
    AssignmentFailed { reason: String },

    /// Window merge operation failed
    #[error("window merge failed: {reason}")]
    MergeFailed { reason: String },
}

/// Aggregation computation errors
#[derive(Error, Debug)]
pub enum AggregationError {
    /// Numeric overflow during aggregation
    #[error("numeric overflow in {operation}: {details}")]
    NumericOverflow { operation: String, details: String },

    /// Division by zero in average or similar calculations
    #[error("division by zero in {operation}")]
    DivisionByZero { operation: String },

    /// Invalid metric value (NaN, Inf, etc.)
    #[error("invalid metric value: {value}, reason: {reason}")]
    InvalidValue { value: f64, reason: String },

    /// Percentile calculation failed
    #[error("percentile calculation failed: percentile={percentile}, sample_count={sample_count}, reason: {reason}")]
    PercentileFailed {
        percentile: u8,
        sample_count: usize,
        reason: String,
    },

    /// Histogram bin configuration error
    #[error("invalid histogram configuration: {reason}")]
    HistogramConfig { reason: String },

    /// Insufficient data for aggregation
    #[error("insufficient data for {aggregation_type}: need at least {required} samples, got {actual}")]
    InsufficientData {
        aggregation_type: String,
        required: usize,
        actual: usize,
    },

    /// Aggregation state is corrupted
    #[error("corrupted aggregation state for {aggregation_type}: {details}")]
    CorruptedState {
        aggregation_type: String,
        details: String,
    },

    /// Type mismatch in aggregation
    #[error("type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },
}

/// State backend operation errors
#[derive(Error, Debug)]
pub enum StateError {
    /// State backend not initialized
    #[error("state backend not initialized: {backend_type}")]
    NotInitialized { backend_type: String },

    /// State key not found
    #[error("state key not found: {key}")]
    KeyNotFound { key: String },

    /// State serialization failed
    #[error("state serialization failed for key '{key}': {reason}")]
    SerializationFailed { key: String, reason: String },

    /// State deserialization failed
    #[error("state deserialization failed for key '{key}': {reason}")]
    DeserializationFailed { key: String, reason: String },

    /// State backend storage error
    #[error("storage error in {backend_type}: {details}")]
    StorageError {
        backend_type: String,
        details: String,
    },

    /// Checkpoint creation failed
    #[error("checkpoint failed at {checkpoint_id}: {reason}")]
    CheckpointFailed {
        checkpoint_id: String,
        reason: String,
    },

    /// Checkpoint restoration failed
    #[error("restore failed from checkpoint {checkpoint_id}: {reason}")]
    RestoreFailed {
        checkpoint_id: String,
        reason: String,
    },

    /// State TTL expired
    #[error("state expired for key '{key}': ttl={ttl}ms, age={age}ms")]
    StateExpired { key: String, ttl: u64, age: u64 },

    /// Transaction failed
    #[error("transaction failed: {operation}, reason: {reason}")]
    TransactionFailed { operation: String, reason: String },

    /// Concurrent modification detected
    #[error("concurrent modification detected for key '{key}'")]
    ConcurrentModification { key: String },

    /// State backend is full
    #[error("state backend full: used {used} bytes of {capacity} bytes")]
    BackendFull { used: u64, capacity: u64 },
}

/// Watermark generation and propagation errors
#[derive(Error, Debug)]
pub enum WatermarkError {
    /// Watermark went backwards
    #[error("watermark regression: new watermark {new_watermark} is before current {current_watermark}")]
    WatermarkRegression {
        current_watermark: i64,
        new_watermark: i64,
    },

    /// Source is idle for too long
    #[error("source idle timeout: source_id={source_id}, idle_duration={idle_duration}ms, timeout={timeout}ms")]
    SourceIdle {
        source_id: String,
        idle_duration: u64,
        timeout: u64,
    },

    /// Invalid watermark configuration
    #[error("invalid watermark configuration: {reason}")]
    InvalidConfig { reason: String },

    /// Watermark delay is negative
    #[error("invalid watermark delay: {delay}ms, must be non-negative")]
    InvalidDelay { delay: i64 },

    /// Missing timestamp in event
    #[error("event missing timestamp: event_id={event_id}")]
    MissingTimestamp { event_id: String },

    /// Clock skew detected
    #[error("clock skew detected: event time {event_time} is in the future, ahead by {ahead_by}ms")]
    ClockSkew { event_time: i64, ahead_by: i64 },

    /// Watermark alignment failed across partitions
    #[error("watermark alignment failed: {reason}")]
    AlignmentFailed { reason: String },
}

/// Result type alias for processor operations
pub type Result<T> = std::result::Result<T, ProcessorError>;

/// Result type alias for window operations
pub type WindowResult<T> = std::result::Result<T, WindowError>;

/// Result type alias for aggregation operations
pub type AggregationResult<T> = std::result::Result<T, AggregationError>;

/// Result type alias for state operations
pub type StateResult<T> = std::result::Result<T, StateError>;

/// Result type alias for watermark operations
pub type WatermarkResult<T> = std::result::Result<T, WatermarkError>;

impl From<bincode::Error> for ProcessorError {
    fn from(err: bincode::Error) -> Self {
        ProcessorError::Serialization(err.to_string())
    }
}

impl From<serde_json::Error> for ProcessorError {
    fn from(err: serde_json::Error) -> Self {
        ProcessorError::Serialization(err.to_string())
    }
}

impl From<anyhow::Error> for ProcessorError {
    fn from(err: anyhow::Error) -> Self {
        ProcessorError::Unexpected(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_error_display() {
        let err = WindowError::InvalidWindowSize { size: 0 };
        assert!(err.to_string().contains("invalid window size"));
    }

    #[test]
    fn test_aggregation_error_display() {
        let err = AggregationError::DivisionByZero {
            operation: "average".to_string(),
        };
        assert!(err.to_string().contains("division by zero"));
    }

    #[test]
    fn test_state_error_display() {
        let err = StateError::KeyNotFound {
            key: "test_key".to_string(),
        };
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_watermark_error_display() {
        let err = WatermarkError::WatermarkRegression {
            current_watermark: 1000,
            new_watermark: 900,
        };
        assert!(err.to_string().contains("regression"));
    }

    #[test]
    fn test_processor_error_from_window_error() {
        let window_err = WindowError::InvalidWindowSize { size: 0 };
        let processor_err: ProcessorError = window_err.into();
        assert!(matches!(processor_err, ProcessorError::Window(_)));
    }
}
