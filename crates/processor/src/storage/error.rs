//! Storage error types

use thiserror::Error;

/// Result type for storage operations
pub type StorageResult<T> = Result<T, StorageError>;

/// Errors that can occur during storage operations
#[derive(Debug, Error)]
pub enum StorageError {
    /// Backend not available
    #[error("Backend not available: {backend}")]
    BackendNotAvailable { backend: String },

    /// Connection error
    #[error("Connection error: {source}")]
    ConnectionError {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Query error
    #[error("Query error: {message}")]
    QueryError { message: String },

    /// Serialization error
    #[error("Serialization error: {source}")]
    SerializationError {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Deserialization error
    #[error("Deserialization error: {source}")]
    DeserializationError {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Transaction error
    #[error("Transaction error: {message}")]
    TransactionError { message: String },

    /// Constraint violation
    #[error("Constraint violation: {message}")]
    ConstraintViolation { message: String },

    /// Not found
    #[error("Not found: {entity} with key {key}")]
    NotFound { entity: String, key: String },

    /// Already exists
    #[error("Already exists: {entity} with key {key}")]
    AlreadyExists { entity: String, key: String },

    /// Invalid configuration
    #[error("Invalid configuration: {message}")]
    InvalidConfiguration { message: String },

    /// Timeout
    #[error("Operation timed out after {duration_ms}ms")]
    Timeout { duration_ms: u64 },

    /// Invalid state
    #[error("Invalid state: {message}")]
    InvalidState { message: String },

    /// Lock error
    #[error("Lock error: {message}")]
    LockError { message: String },

    /// Migration error
    #[error("Migration error: {message}")]
    MigrationError { message: String },

    /// Unsupported operation
    #[error("Unsupported operation: {operation} not supported by {backend}")]
    UnsupportedOperation { operation: String, backend: String },

    /// Internal error
    #[error("Internal error: {message}")]
    InternalError { message: String },

    /// Capacity error
    #[error("Capacity exceeded: {message}")]
    CapacityError { message: String },

    /// I/O error
    #[error("I/O error: {source}")]
    IoError {
        #[from]
        source: std::io::Error,
    },
}

impl StorageError {
    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            StorageError::ConnectionError { .. }
                | StorageError::Timeout { .. }
                | StorageError::LockError { .. }
        )
    }

    /// Check if error is transient
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            StorageError::ConnectionError { .. }
                | StorageError::Timeout { .. }
                | StorageError::LockError { .. }
                | StorageError::BackendNotAvailable { .. }
        )
    }

    /// Get error category
    pub fn category(&self) -> ErrorCategory {
        match self {
            StorageError::BackendNotAvailable { .. } => ErrorCategory::Availability,
            StorageError::ConnectionError { .. } => ErrorCategory::Connection,
            StorageError::QueryError { .. } => ErrorCategory::Query,
            StorageError::SerializationError { .. } | StorageError::DeserializationError { .. } => {
                ErrorCategory::Serialization
            }
            StorageError::TransactionError { .. } => ErrorCategory::Transaction,
            StorageError::ConstraintViolation { .. } => ErrorCategory::Constraint,
            StorageError::NotFound { .. } => ErrorCategory::NotFound,
            StorageError::AlreadyExists { .. } => ErrorCategory::Conflict,
            StorageError::InvalidConfiguration { .. } => ErrorCategory::Configuration,
            StorageError::Timeout { .. } => ErrorCategory::Timeout,
            StorageError::InvalidState { .. } => ErrorCategory::State,
            StorageError::LockError { .. } => ErrorCategory::Concurrency,
            StorageError::MigrationError { .. } => ErrorCategory::Migration,
            StorageError::UnsupportedOperation { .. } => ErrorCategory::Unsupported,
            StorageError::InternalError { .. } => ErrorCategory::Internal,
            StorageError::CapacityError { .. } => ErrorCategory::Capacity,
            StorageError::IoError { .. } => ErrorCategory::Io,
        }
    }
}

/// Error category for classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    Availability,
    Connection,
    Query,
    Serialization,
    Transaction,
    Constraint,
    NotFound,
    Conflict,
    Configuration,
    Timeout,
    State,
    Concurrency,
    Migration,
    Unsupported,
    Internal,
    Capacity,
    Io,
}

impl ErrorCategory {
    /// Check if category represents a client error (4xx)
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            ErrorCategory::Query
                | ErrorCategory::Constraint
                | ErrorCategory::NotFound
                | ErrorCategory::Conflict
                | ErrorCategory::Configuration
                | ErrorCategory::State
                | ErrorCategory::Unsupported
        )
    }

    /// Check if category represents a server error (5xx)
    pub fn is_server_error(&self) -> bool {
        matches!(
            self,
            ErrorCategory::Availability
                | ErrorCategory::Connection
                | ErrorCategory::Transaction
                | ErrorCategory::Timeout
                | ErrorCategory::Concurrency
                | ErrorCategory::Migration
                | ErrorCategory::Internal
                | ErrorCategory::Capacity
                | ErrorCategory::Io
        )
    }
}
