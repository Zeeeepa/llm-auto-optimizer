//! Optimization-related request/response models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

use llm_optimizer_types::decisions::{OptimizationStrategy, DecisionStatus};

/// Request to create an optimization
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateOptimizationRequest {
    /// Target services to optimize
    #[validate(length(min = 1))]
    pub target_services: Vec<String>,

    /// Optimization strategy to use
    pub strategy: OptimizationStrategy,

    /// Configuration parameters
    #[serde(default)]
    pub config: serde_json::Value,

    /// Constraints for optimization
    #[serde(default)]
    pub constraints: Vec<ConstraintInput>,

    /// Dry run mode (don't actually deploy)
    #[serde(default)]
    pub dry_run: bool,
}

/// Constraint input
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct ConstraintInput {
    /// Constraint type
    #[validate(length(min = 1))]
    pub constraint_type: String,

    /// Constraint value
    pub value: serde_json::Value,

    /// Whether this is a hard constraint
    #[serde(default)]
    pub hard: bool,
}

/// Optimization response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OptimizationResponse {
    /// Optimization ID
    pub id: Uuid,

    /// Target services
    pub target_services: Vec<String>,

    /// Strategy used
    pub strategy: OptimizationStrategy,

    /// Current status
    pub status: DecisionStatus,

    /// Configuration changes
    pub changes: Vec<ConfigurationChangeResponse>,

    /// Expected impact
    pub expected_impact: ExpectedImpactResponse,

    /// Actual impact (if completed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_impact: Option<ActualImpactResponse>,

    /// Rationale
    pub rationale: String,

    /// Created at
    pub created_at: DateTime<Utc>,

    /// Deployed at (if deployed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deployed_at: Option<DateTime<Utc>>,
}

/// Configuration change response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConfigurationChangeResponse {
    /// Parameter name
    pub parameter: String,

    /// Old value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_value: Option<serde_json::Value>,

    /// New value
    pub new_value: serde_json::Value,

    /// Change type
    pub change_type: String,
}

/// Expected impact response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ExpectedImpactResponse {
    /// Expected cost reduction percentage
    pub cost_reduction_pct: f64,

    /// Expected quality change percentage
    pub quality_delta_pct: f64,

    /// Expected latency change percentage
    pub latency_delta_pct: f64,

    /// Confidence level (0.0-1.0)
    pub confidence: f64,
}

/// Actual impact response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ActualImpactResponse {
    /// Actual cost reduction percentage
    pub cost_reduction_pct: f64,

    /// Actual quality change percentage
    pub quality_delta_pct: f64,

    /// Actual latency change percentage
    pub latency_delta_pct: f64,

    /// Number of requests affected
    pub requests_affected: u64,

    /// Measurement period
    pub measured_from: DateTime<Utc>,
    pub measured_until: DateTime<Utc>,
}

/// Request to deploy an optimization
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct DeployOptimizationRequest {
    /// Whether to perform gradual rollout
    #[serde(default = "default_gradual")]
    pub gradual: bool,

    /// Rollout percentage (if gradual)
    #[serde(default = "default_rollout_pct")]
    #[validate(range(min = 0.0, max = 100.0))]
    pub rollout_percentage: f64,
}

fn default_gradual() -> bool {
    true
}

fn default_rollout_pct() -> f64 {
    10.0
}

/// Request to rollback an optimization
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct RollbackOptimizationRequest {
    /// Reason for rollback
    #[validate(length(min = 1))]
    pub reason: String,
}

/// List optimizations query parameters
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, IntoParams)]
pub struct ListOptimizationsQuery {
    /// Filter by status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<DecisionStatus>,

    /// Filter by strategy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy: Option<OptimizationStrategy>,

    /// Filter by service
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,

    /// Date range
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<DateTime<Utc>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_optimization_request() {
        let req = CreateOptimizationRequest {
            target_services: vec!["service-1".to_string()],
            strategy: OptimizationStrategy::CostPerformanceScoring,
            config: serde_json::json!({}),
            constraints: vec![],
            dry_run: false,
        };

        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_deploy_optimization_request_defaults() {
        let req = DeployOptimizationRequest {
            gradual: default_gradual(),
            rollout_percentage: default_rollout_pct(),
        };

        assert!(req.gradual);
        assert_eq!(req.rollout_percentage, 10.0);
    }
}
