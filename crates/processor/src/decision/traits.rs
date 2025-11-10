//! Decision Engine core traits

use async_trait::async_trait;

use super::error::{DecisionError, DecisionResult, DecisionState};
use super::types::{Decision, DecisionCriteria, DecisionInput, DecisionOutcome, DecisionStats};

/// Core trait for the Decision Engine
#[async_trait]
pub trait DecisionEngine: Send + Sync {
    /// Get the name of this decision engine
    fn name(&self) -> &str;

    /// Get the current state
    fn state(&self) -> DecisionState;

    /// Start the decision engine
    async fn start(&mut self) -> DecisionResult<()>;

    /// Stop the decision engine
    async fn stop(&mut self) -> DecisionResult<()>;

    /// Make decisions based on analyzer input
    ///
    /// # Arguments
    /// * `input` - Input from the analyzer engine
    /// * `criteria` - Decision criteria to use
    ///
    /// # Returns
    /// Vector of decisions to execute
    async fn make_decisions(
        &mut self,
        input: DecisionInput,
        criteria: &DecisionCriteria,
    ) -> DecisionResult<Vec<Decision>>;

    /// Record the outcome of a decision
    async fn record_outcome(&mut self, outcome: DecisionOutcome) -> DecisionResult<()>;

    /// Get decision statistics
    fn get_stats(&self) -> DecisionStats;

    /// Health check
    async fn health_check(&self) -> DecisionResult<()>;

    /// Reset the decision engine state
    async fn reset(&mut self) -> DecisionResult<()>;
}

/// Trait for optimization strategies
#[async_trait]
pub trait OptimizationStrategy: Send + Sync {
    /// Get the name of this strategy
    fn name(&self) -> &str;

    /// Get the priority of this strategy (higher = more important)
    fn priority(&self) -> u32;

    /// Check if this strategy is applicable to the given input
    fn is_applicable(&self, input: &DecisionInput) -> bool;

    /// Evaluate and generate decisions
    ///
    /// # Arguments
    /// * `input` - Input from the analyzer engine
    /// * `criteria` - Decision criteria to use
    ///
    /// # Returns
    /// Vector of decisions this strategy recommends
    async fn evaluate(
        &self,
        input: &DecisionInput,
        criteria: &DecisionCriteria,
    ) -> DecisionResult<Vec<Decision>>;

    /// Validate a decision before execution
    ///
    /// # Arguments
    /// * `decision` - The decision to validate
    /// * `criteria` - Decision criteria to validate against
    ///
    /// # Returns
    /// Ok if valid, Err with reason if invalid
    async fn validate(
        &self,
        decision: &Decision,
        criteria: &DecisionCriteria,
    ) -> DecisionResult<()>;

    /// Learn from a decision outcome
    ///
    /// # Arguments
    /// * `decision` - The original decision
    /// * `outcome` - The outcome that was observed
    async fn learn(&mut self, decision: &Decision, outcome: &DecisionOutcome)
        -> DecisionResult<()>;

    /// Get strategy-specific statistics
    fn get_stats(&self) -> serde_json::Value;
}

/// Trait for decision validators
#[async_trait]
pub trait DecisionValidator: Send + Sync {
    /// Validate a decision
    async fn validate(
        &self,
        decision: &Decision,
        criteria: &DecisionCriteria,
    ) -> DecisionResult<()>;
}

/// Trait for decision executors (actuators)
#[async_trait]
pub trait DecisionExecutor: Send + Sync {
    /// Execute a decision
    async fn execute(&self, decision: &Decision) -> DecisionResult<DecisionOutcome>;

    /// Rollback a decision
    async fn rollback(&self, decision: &Decision) -> DecisionResult<()>;
}
