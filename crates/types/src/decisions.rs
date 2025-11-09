//! Optimization decision types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Optimization strategy type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationStrategy {
    /// A/B testing with variants
    ABTesting,
    /// Reinforcement learning bandit
    ReinforcementFeedback,
    /// Cost-performance Pareto optimization
    CostPerformanceScoring,
    /// Adaptive parameter tuning
    AdaptiveParameterTuning,
    /// Threshold-based heuristics
    ThresholdBased,
    /// Hybrid approach
    Hybrid,
}

/// Type of configuration change
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    Replace,
    Add,
    Remove,
    Update,
}

/// A single configuration change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigurationChange {
    /// Parameter name (e.g., "model", "temperature")
    pub parameter: String,
    /// Old value (if any)
    pub old_value: Option<serde_json::Value>,
    /// New value
    pub new_value: serde_json::Value,
    /// Type of change
    pub change_type: ChangeType,
}

impl ConfigurationChange {
    /// Create a new configuration change
    pub fn new(
        parameter: impl Into<String>,
        new_value: serde_json::Value,
        change_type: ChangeType,
    ) -> Self {
        Self {
            parameter: parameter.into(),
            old_value: None,
            new_value,
            change_type,
        }
    }

    /// Add the old value
    pub fn with_old_value(mut self, old_value: serde_json::Value) -> Self {
        self.old_value = Some(old_value);
        self
    }
}

/// Expected impact of an optimization decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedImpact {
    /// Expected cost reduction percentage
    pub cost_reduction_pct: f64,
    /// Expected quality change (can be negative)
    pub quality_delta_pct: f64,
    /// Expected latency change percentage (negative is improvement)
    pub latency_delta_pct: f64,
    /// Confidence level (0.0-1.0)
    pub confidence: f64,
}

impl ExpectedImpact {
    /// Create a new expected impact
    pub fn new(cost_reduction: f64, quality_delta: f64, latency_delta: f64) -> Self {
        Self {
            cost_reduction_pct: cost_reduction,
            quality_delta_pct: quality_delta,
            latency_delta_pct: latency_delta,
            confidence: 0.8, // Default 80% confidence
        }
    }

    /// Set confidence level
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }
}

/// Constraint on optimization decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    /// Constraint type (e.g., "min_quality", "max_cost")
    pub constraint_type: String,
    /// Constraint value
    pub value: serde_json::Value,
    /// Whether this is a hard constraint
    pub hard: bool,
}

/// Status of an optimization decision
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStatus {
    /// Decision created but not yet deployed
    Pending,
    /// Currently being validated
    Validating,
    /// Validation failed
    ValidationFailed,
    /// Currently being deployed
    Deploying,
    /// Successfully deployed
    Deployed,
    /// Deployment failed
    DeploymentFailed,
    /// Currently being monitored
    Monitoring,
    /// Rolled back due to issues
    RolledBack,
    /// Completed successfully
    Completed,
    /// Cancelled
    Cancelled,
}

/// Actual impact after deployment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActualImpact {
    /// Actual cost reduction percentage
    pub cost_reduction_pct: f64,
    /// Actual quality change percentage
    pub quality_delta_pct: f64,
    /// Actual latency change percentage
    pub latency_delta_pct: f64,
    /// Number of requests affected
    pub requests_affected: u64,
    /// Measurement period start
    pub measured_from: DateTime<Utc>,
    /// Measurement period end
    pub measured_until: DateTime<Utc>,
}

/// Core optimization decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationDecision {
    /// Unique decision identifier
    pub id: Uuid,
    /// When the decision was created
    pub created_at: DateTime<Utc>,
    /// Strategy used
    pub strategy: OptimizationStrategy,
    /// Target services affected
    pub target_services: Vec<String>,
    /// Configuration changes to apply
    pub changes: Vec<ConfigurationChange>,
    /// Rationale for the decision
    pub rationale: String,
    /// Expected impact
    pub expected_impact: ExpectedImpact,
    /// Constraints that must be satisfied
    pub constraints: Vec<Constraint>,
    /// Current status
    pub status: DecisionStatus,
    /// When deployed (if applicable)
    pub deployed_at: Option<DateTime<Utc>>,
    /// When rolled back (if applicable)
    pub rolled_back_at: Option<DateTime<Utc>>,
    /// Actual impact (after deployment)
    pub actual_impact: Option<ActualImpact>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl OptimizationDecision {
    /// Create a new optimization decision
    pub fn new(
        strategy: OptimizationStrategy,
        target_services: Vec<String>,
        changes: Vec<ConfigurationChange>,
        rationale: impl Into<String>,
        expected_impact: ExpectedImpact,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            created_at: Utc::now(),
            strategy,
            target_services,
            changes,
            rationale: rationale.into(),
            expected_impact,
            constraints: Vec::new(),
            status: DecisionStatus::Pending,
            deployed_at: None,
            rolled_back_at: None,
            actual_impact: None,
            metadata: HashMap::new(),
        }
    }

    /// Add a constraint to the decision
    pub fn with_constraint(mut self, constraint: Constraint) -> Self {
        self.constraints.push(constraint);
        self
    }

    /// Mark as deployed
    pub fn mark_deployed(&mut self) {
        self.status = DecisionStatus::Deployed;
        self.deployed_at = Some(Utc::now());
    }

    /// Mark as rolled back
    pub fn mark_rolled_back(&mut self) {
        self.status = DecisionStatus::RolledBack;
        self.rolled_back_at = Some(Utc::now());
    }

    /// Record actual impact
    pub fn record_impact(&mut self, impact: ActualImpact) {
        self.actual_impact = Some(impact);
        self.status = DecisionStatus::Completed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimization_decision_creation() {
        let changes = vec![ConfigurationChange::new(
            "model",
            serde_json::json!("claude-3-haiku"),
            ChangeType::Replace,
        )];

        let expected_impact = ExpectedImpact::new(30.0, -2.0, -10.0);

        let decision = OptimizationDecision::new(
            OptimizationStrategy::CostPerformanceScoring,
            vec!["service-1".to_string()],
            changes,
            "Switching to more cost-effective model",
            expected_impact,
        );

        assert_eq!(decision.strategy, OptimizationStrategy::CostPerformanceScoring);
        assert_eq!(decision.status, DecisionStatus::Pending);
        assert_eq!(decision.target_services.len(), 1);
    }

    #[test]
    fn test_decision_lifecycle() {
        let mut decision = OptimizationDecision::new(
            OptimizationStrategy::ThresholdBased,
            vec!["service-1".to_string()],
            vec![],
            "Test decision",
            ExpectedImpact::new(10.0, 0.0, 0.0),
        );

        assert_eq!(decision.status, DecisionStatus::Pending);

        decision.mark_deployed();
        assert_eq!(decision.status, DecisionStatus::Deployed);
        assert!(decision.deployed_at.is_some());

        decision.mark_rolled_back();
        assert_eq!(decision.status, DecisionStatus::RolledBack);
        assert!(decision.rolled_back_at.is_some());
    }
}
