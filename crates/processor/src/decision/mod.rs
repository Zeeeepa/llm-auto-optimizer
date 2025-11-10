//! Decision Engine Module
//!
//! This module provides the decision-making capabilities for the LLM Auto-Optimizer.
//! It includes various optimization strategies and a coordinator to manage them.

pub mod batching;
pub mod caching;
pub mod config;
pub mod coordinator;
pub mod error;
pub mod model_selection;
pub mod prompt_optimization;
pub mod rate_limiting;
pub mod traits;
pub mod types;

// Re-export commonly used types
pub use config::{
    BatchingConfig, CachingConfig, DecisionEngineConfig, ModelSelectionConfig, MonitoringConfig,
    PromptOptimizationConfig, RateLimitingConfig, SafetyConfig, StrategyConfigs,
};

pub use batching::BatchingStrategy;

pub use caching::CachingStrategy;

pub use coordinator::DecisionCoordinator;

pub use error::{DecisionError, DecisionResult, DecisionState};

pub use model_selection::ModelSelectionStrategy;

pub use prompt_optimization::PromptOptimizationStrategy;

pub use rate_limiting::RateLimitingStrategy;

pub use traits::{DecisionEngine, DecisionExecutor, DecisionValidator, OptimizationStrategy};

pub use types::{
    ActualImpact, ComparisonOperator, ConfigChange, ConfigType, Decision, DecisionCriteria,
    DecisionInput, DecisionOutcome, DecisionStats, DecisionType, ExpectedImpact, RollbackCondition,
    RollbackPlan, SafetyCheck, SystemMetrics,
};
