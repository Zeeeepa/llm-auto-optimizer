//! Error types for the optimizer

use thiserror::Error;

/// Result type alias for optimizer operations
pub type Result<T> = std::result::Result<T, OptimizerError>;

/// Main error type for the optimizer
#[derive(Error, Debug)]
pub enum OptimizerError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Integration error: {0}")]
    Integration(String),

    #[error("Analysis error: {0}")]
    Analysis(String),

    #[error("Decision error: {0}")]
    Decision(String),

    #[error("Deployment error: {0}")]
    Deployment(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
