//! Decision Engine core types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::analyzer::{AnalysisReport, Insight, Recommendation};

/// Input to the decision engine from analyzers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionInput {
    /// Timestamp of the input
    pub timestamp: DateTime<Utc>,

    /// Analysis reports from all analyzers
    pub analysis_reports: Vec<AnalysisReport>,

    /// Aggregated insights
    pub insights: Vec<Insight>,

    /// Aggregated recommendations
    pub recommendations: Vec<Recommendation>,

    /// Current system metrics
    pub current_metrics: SystemMetrics,

    /// Additional context
    pub context: HashMap<String, serde_json::Value>,
}

/// Current system metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// Average latency (ms)
    pub avg_latency_ms: f64,

    /// P95 latency (ms)
    pub p95_latency_ms: f64,

    /// P99 latency (ms)
    pub p99_latency_ms: f64,

    /// Success rate (0-100)
    pub success_rate_pct: f64,

    /// Error rate (0-100)
    pub error_rate_pct: f64,

    /// Throughput (requests/sec)
    pub throughput_rps: f64,

    /// Total cost (USD)
    pub total_cost_usd: f64,

    /// Daily cost (USD)
    pub daily_cost_usd: f64,

    /// Average rating (0-5)
    pub avg_rating: Option<f64>,

    /// Cache hit rate (0-100)
    pub cache_hit_rate_pct: Option<f64>,

    /// Current request count
    pub request_count: u64,

    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// A decision made by the engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    /// Unique decision ID
    pub id: String,

    /// Timestamp of the decision
    pub timestamp: DateTime<Utc>,

    /// Strategy that made this decision
    pub strategy: String,

    /// Type of decision
    pub decision_type: DecisionType,

    /// Confidence in the decision (0-1)
    pub confidence: f64,

    /// Expected impact of the decision
    pub expected_impact: ExpectedImpact,

    /// Configuration changes to apply
    pub config_changes: Vec<ConfigChange>,

    /// Justification for the decision
    pub justification: String,

    /// Related insights that led to this decision
    pub related_insights: Vec<String>,

    /// Related recommendations
    pub related_recommendations: Vec<String>,

    /// Priority (higher = more urgent)
    pub priority: u32,

    /// Whether this decision requires approval
    pub requires_approval: bool,

    /// Safety checks passed
    pub safety_checks: Vec<SafetyCheck>,

    /// Rollback plan
    pub rollback_plan: Option<RollbackPlan>,

    /// Metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Type of decision
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DecisionType {
    /// Switch to a different model
    ModelSwitch,

    /// Enable or update caching
    EnableCaching,

    /// Adjust rate limits
    AdjustRateLimits,

    /// Enable request batching
    EnableBatching,

    /// Optimize prompts
    OptimizePrompt,

    /// Scale resources
    ScaleResources,

    /// Rollback previous change
    Rollback,

    /// No action needed
    NoAction,
}

/// Expected impact of a decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedImpact {
    /// Expected cost change (USD/day)
    pub cost_change_usd: Option<f64>,

    /// Expected latency change (ms)
    pub latency_change_ms: Option<f64>,

    /// Expected quality score change (0-5)
    pub quality_change: Option<f64>,

    /// Expected throughput change (req/sec)
    pub throughput_change_rps: Option<f64>,

    /// Expected impact on success rate (percentage points)
    pub success_rate_change_pct: Option<f64>,

    /// Time to realize impact
    pub time_to_impact: std::time::Duration,

    /// Confidence in impact estimate (0-1)
    pub confidence: f64,
}

/// A configuration change to apply
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigChange {
    /// Type of configuration being changed
    pub config_type: ConfigType,

    /// Path to the configuration (e.g., "model.name", "cache.enabled")
    pub path: String,

    /// Old value (if any)
    pub old_value: Option<serde_json::Value>,

    /// New value
    pub new_value: serde_json::Value,

    /// Description of the change
    pub description: String,
}

/// Type of configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConfigType {
    /// Model configuration
    Model,

    /// Cache configuration
    Cache,

    /// Rate limit configuration
    RateLimit,

    /// Batching configuration
    Batching,

    /// Prompt configuration
    Prompt,

    /// Timeout configuration
    Timeout,

    /// Resource configuration
    Resource,

    /// Other configuration
    Other,
}

/// Safety check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyCheck {
    /// Name of the check
    pub name: String,

    /// Whether the check passed
    pub passed: bool,

    /// Details about the check
    pub details: String,

    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Rollback plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackPlan {
    /// Description of the rollback
    pub description: String,

    /// Configuration changes to revert
    pub revert_changes: Vec<ConfigChange>,

    /// Conditions that trigger automatic rollback
    pub trigger_conditions: Vec<RollbackCondition>,

    /// Maximum time before automatic rollback (if metrics don't improve)
    pub max_duration: std::time::Duration,
}

/// Condition that triggers rollback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackCondition {
    /// Metric to monitor
    pub metric: String,

    /// Comparison operator
    pub operator: ComparisonOperator,

    /// Threshold value
    pub threshold: f64,

    /// Description
    pub description: String,
}

/// Comparison operator for rollback conditions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComparisonOperator {
    /// Greater than
    GreaterThan,

    /// Less than
    LessThan,

    /// Greater than or equal
    GreaterThanOrEqual,

    /// Less than or equal
    LessThanOrEqual,

    /// Equal
    Equal,

    /// Not equal
    NotEqual,
}

/// Outcome of a decision after execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionOutcome {
    /// Decision ID
    pub decision_id: String,

    /// Timestamp when decision was executed
    pub execution_time: DateTime<Utc>,

    /// Whether execution succeeded
    pub success: bool,

    /// Error message if failed
    pub error: Option<String>,

    /// Actual impact observed (after sufficient time)
    pub actual_impact: Option<ActualImpact>,

    /// Whether rollback was triggered
    pub rolled_back: bool,

    /// Rollback reason
    pub rollback_reason: Option<String>,

    /// Metrics before the decision
    pub metrics_before: SystemMetrics,

    /// Metrics after the decision (if available)
    pub metrics_after: Option<SystemMetrics>,

    /// Duration the decision was active
    pub duration: std::time::Duration,
}

/// Actual impact observed after a decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActualImpact {
    /// Actual cost change (USD/day)
    pub cost_change_usd: f64,

    /// Actual latency change (ms)
    pub latency_change_ms: f64,

    /// Actual quality change (0-5)
    pub quality_change: f64,

    /// Actual throughput change (req/sec)
    pub throughput_change_rps: f64,

    /// Actual success rate change (percentage points)
    pub success_rate_change_pct: f64,

    /// Time it took to realize impact
    pub time_to_impact: std::time::Duration,

    /// Comparison with expected impact
    pub variance_from_expected: f64,
}

/// Criteria for making decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionCriteria {
    /// Minimum confidence required (0-1)
    pub min_confidence: f64,

    /// Maximum cost increase allowed (USD/day)
    pub max_cost_increase_usd: Option<f64>,

    /// Maximum latency increase allowed (ms)
    pub max_latency_increase_ms: Option<f64>,

    /// Minimum quality score (0-5)
    pub min_quality_score: Option<f64>,

    /// Required safety checks
    pub required_safety_checks: Vec<String>,

    /// Whether to allow breaking changes
    pub allow_breaking_changes: bool,

    /// Maximum number of concurrent decisions
    pub max_concurrent_decisions: usize,

    /// Enabled strategies
    pub enabled_strategies: Vec<String>,

    /// Strategy priorities (higher = more important)
    pub strategy_priorities: HashMap<String, u32>,
}

impl Default for DecisionCriteria {
    fn default() -> Self {
        Self {
            min_confidence: 0.7,
            max_cost_increase_usd: Some(100.0),
            max_latency_increase_ms: Some(500.0),
            min_quality_score: Some(3.5),
            required_safety_checks: vec![
                "cost_check".to_string(),
                "latency_check".to_string(),
                "quality_check".to_string(),
            ],
            allow_breaking_changes: false,
            max_concurrent_decisions: 3,
            enabled_strategies: vec![
                "model_selection".to_string(),
                "caching".to_string(),
                "rate_limiting".to_string(),
                "batching".to_string(),
                "prompt_optimization".to_string(),
            ],
            strategy_priorities: {
                let mut priorities = HashMap::new();
                priorities.insert("model_selection".to_string(), 100);
                priorities.insert("caching".to_string(), 90);
                priorities.insert("rate_limiting".to_string(), 80);
                priorities.insert("batching".to_string(), 70);
                priorities.insert("prompt_optimization".to_string(), 85);
                priorities
            },
        }
    }
}

/// Statistics about decision engine performance
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DecisionStats {
    /// Total decisions made
    pub total_decisions: u64,

    /// Decisions by type
    pub decisions_by_type: HashMap<String, u64>,

    /// Decisions by strategy
    pub decisions_by_strategy: HashMap<String, u64>,

    /// Successful decisions
    pub successful_decisions: u64,

    /// Failed decisions
    pub failed_decisions: u64,

    /// Rolled back decisions
    pub rolled_back_decisions: u64,

    /// Average decision latency (ms)
    pub avg_decision_latency_ms: f64,

    /// Average confidence
    pub avg_confidence: f64,

    /// Total cost savings (USD)
    pub total_cost_savings_usd: f64,

    /// Total latency improvement (ms)
    pub total_latency_improvement_ms: f64,

    /// Last decision time
    pub last_decision_time: Option<DateTime<Utc>>,
}
