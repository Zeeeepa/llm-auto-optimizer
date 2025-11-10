//! Decision Engine error types

use thiserror::Error;

/// Result type for Decision Engine operations
pub type DecisionResult<T> = Result<T, DecisionError>;

/// Error types for the Decision Engine
#[derive(Error, Debug, Clone, PartialEq)]
pub enum DecisionError {
    /// Strategy not found
    #[error("Strategy not found: {0}")]
    StrategyNotFound(String),

    /// Invalid decision criteria
    #[error("Invalid decision criteria: {0}")]
    InvalidCriteria(String),

    /// Decision validation failed
    #[error("Decision validation failed: {0}")]
    ValidationFailed(String),

    /// Insufficient data for decision
    #[error("Insufficient data: {0}")]
    InsufficientData(String),

    /// Constraint violation
    #[error("Constraint violation: {0}")]
    ConstraintViolation(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Execution error
    #[error("Execution error: {0}")]
    ExecutionError(String),

    /// Rollback failed
    #[error("Rollback failed: {0}")]
    RollbackFailed(String),

    /// Strategy conflict
    #[error("Strategy conflict: {0}")]
    StrategyConflict(String),

    /// Timeout
    #[error("Decision timeout after {0}ms")]
    Timeout(u64),

    /// Not running
    #[error("Decision engine is not running")]
    NotRunning,

    /// Invalid state transition
    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidStateTransition {
        from: DecisionState,
        to: DecisionState,
    },

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Decision Engine state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecisionState {
    /// Initialized but not started
    Initialized,
    /// Starting up
    Starting,
    /// Running and processing decisions
    Running,
    /// Draining before shutdown
    Draining,
    /// Stopped
    Stopped,
    /// Failed state
    Failed,
}

impl DecisionState {
    /// Check if the state allows processing decisions
    pub fn can_process(&self) -> bool {
        matches!(self, DecisionState::Running)
    }

    /// Check if transition to another state is valid
    pub fn can_transition_to(&self, to: DecisionState) -> bool {
        use DecisionState::*;
        matches!(
            (self, to),
            (Initialized, Starting)
                | (Starting, Running)
                | (Starting, Failed)
                | (Running, Draining)
                | (Running, Failed)
                | (Draining, Stopped)
                | (Draining, Failed)
                | (Failed, Initialized)
        )
    }
}
