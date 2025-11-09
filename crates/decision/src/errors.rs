//! Error types for decision engine

use thiserror::Error;

pub type Result<T> = std::result::Result<T, DecisionError>;

#[derive(Error, Debug)]
pub enum DecisionError {
    #[error("Invalid experiment configuration: {0}")]
    InvalidConfig(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Insufficient data: {0}")]
    InsufficientData(String),

    #[error("Statistical error: {0}")]
    StatisticalError(String),

    #[error("Variant not found: {0}")]
    VariantNotFound(String),

    #[error("Experiment not found: {0}")]
    ExperimentNotFound(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Allocation error: {0}")]
    AllocationError(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
