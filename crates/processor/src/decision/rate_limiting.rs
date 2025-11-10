//! Rate Limiting Strategy
//!
//! Implements adaptive rate limiting to control costs and prevent overload.
//! Uses a token bucket algorithm with adaptive adjustment based on error rates,
//! costs, and traffic patterns.

use async_trait::async_trait;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::config::RateLimitingConfig;
use super::error::{DecisionError, DecisionResult};
use super::traits::OptimizationStrategy;
use super::types::{
    ComparisonOperator, ConfigChange, ConfigType, Decision, DecisionCriteria, DecisionInput,
    DecisionOutcome, DecisionType, ExpectedImpact, RollbackCondition, RollbackPlan, SafetyCheck,
};

/// Rate limiting strategy that adjusts rate limits based on costs and error rates
pub struct RateLimitingStrategy {
    config: RateLimitingConfig,
    state: Arc<RwLock<RateLimitState>>,
}

/// Internal state for rate limiting strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RateLimitState {
    /// Current rate limit (requests per second)
    current_rps_limit: f64,

    /// Current token limit (tokens per minute)
    current_token_limit: u64,

    /// Current cost limit (USD per hour)
    current_cost_limit: f64,

    /// Token bucket state
    token_bucket: TokenBucket,

    /// Recent rate limit adjustments
    adjustment_history: VecDeque<RateLimitAdjustment>,

    /// Recent error rate samples
    error_rate_history: VecDeque<ErrorRateSample>,

    /// Recent cost rate samples
    cost_rate_history: VecDeque<CostRateSample>,

    /// Traffic pattern analysis
    traffic_pattern: TrafficPattern,

    /// Learning metrics
    learning_metrics: LearningMetrics,

    /// Last adjustment time
    last_adjustment: DateTime<Utc>,

    /// Number of decisions made
    decisions_made: u64,
}

/// Token bucket for rate limiting
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenBucket {
    /// Current token count
    tokens: f64,

    /// Maximum tokens (bucket capacity)
    capacity: f64,

    /// Token refill rate (tokens per second)
    refill_rate: f64,

    /// Last refill timestamp
    last_refill: DateTime<Utc>,

    /// Burst allowance multiplier
    burst_multiplier: f64,
}

impl TokenBucket {
    fn new(capacity: f64, refill_rate: f64, burst_multiplier: f64) -> Self {
        Self {
            tokens: capacity,
            capacity,
            refill_rate,
            last_refill: Utc::now(),
            burst_multiplier,
        }
    }

    /// Refill tokens based on elapsed time
    fn refill(&mut self) {
        let now = Utc::now();
        let elapsed = (now - self.last_refill).num_milliseconds() as f64 / 1000.0;

        let new_tokens = elapsed * self.refill_rate;
        let max_capacity = self.capacity * self.burst_multiplier;

        self.tokens = (self.tokens + new_tokens).min(max_capacity);
        self.last_refill = now;
    }

    /// Try to consume tokens
    fn try_consume(&mut self, tokens: f64) -> bool {
        self.refill();

        if self.tokens >= tokens {
            self.tokens -= tokens;
            true
        } else {
            false
        }
    }

    /// Update capacity and refill rate
    fn update(&mut self, new_capacity: f64, new_refill_rate: f64) {
        self.refill(); // Refill before changing rates
        self.capacity = new_capacity;
        self.refill_rate = new_refill_rate;

        // If capacity decreased, adjust current tokens
        if self.tokens > self.capacity * self.burst_multiplier {
            self.tokens = self.capacity * self.burst_multiplier;
        }
    }

    /// Get utilization (0-1)
    fn utilization(&self) -> f64 {
        1.0 - (self.tokens / self.capacity).clamp(0.0, 1.0)
    }
}

/// Record of a rate limit adjustment
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RateLimitAdjustment {
    timestamp: DateTime<Utc>,
    old_rps_limit: f64,
    new_rps_limit: f64,
    old_token_limit: u64,
    new_token_limit: u64,
    old_cost_limit: f64,
    new_cost_limit: f64,
    reason: String,
    effectiveness: Option<f64>,
}

/// Error rate sample
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ErrorRateSample {
    timestamp: DateTime<Utc>,
    error_rate_pct: f64,
    request_count: u64,
}

/// Cost rate sample
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CostRateSample {
    timestamp: DateTime<Utc>,
    cost_per_hour: f64,
    tokens_per_minute: f64,
}

/// Traffic pattern analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrafficPattern {
    /// Average requests per second
    avg_rps: f64,

    /// Peak requests per second
    peak_rps: f64,

    /// Average burst size
    avg_burst_size: f64,

    /// Burst frequency (bursts per hour)
    burst_frequency: f64,

    /// Traffic volatility (0-1)
    volatility: f64,

    /// Last update time
    last_update: DateTime<Utc>,
}

impl TrafficPattern {
    fn new() -> Self {
        Self {
            avg_rps: 0.0,
            peak_rps: 0.0,
            avg_burst_size: 0.0,
            burst_frequency: 0.0,
            volatility: 0.0,
            last_update: Utc::now(),
        }
    }

    fn update(&mut self, current_rps: f64, peak_rps: f64) {
        // Exponential moving average
        let alpha = 0.1;
        self.avg_rps = alpha * current_rps + (1.0 - alpha) * self.avg_rps;
        self.peak_rps = self.peak_rps.max(peak_rps);

        // Calculate volatility
        let deviation = (current_rps - self.avg_rps).abs();
        self.volatility = alpha * (deviation / self.avg_rps.max(1.0)) + (1.0 - alpha) * self.volatility;

        self.last_update = Utc::now();
    }
}

/// Learning metrics for adaptive adjustment
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LearningMetrics {
    /// Total adjustments made
    total_adjustments: u64,

    /// Successful adjustments (improved metrics)
    successful_adjustments: u64,

    /// Failed adjustments (degraded metrics)
    failed_adjustments: u64,

    /// Average effectiveness score
    avg_effectiveness: f64,

    /// Confidence in current limits (0-1)
    confidence: f64,

    /// Learning rate
    learning_rate: f64,
}

impl LearningMetrics {
    fn new() -> Self {
        Self {
            total_adjustments: 0,
            successful_adjustments: 0,
            failed_adjustments: 0,
            avg_effectiveness: 0.5,
            confidence: 0.5,
            learning_rate: 0.1,
        }
    }

    fn record_outcome(&mut self, effectiveness: f64) {
        self.total_adjustments += 1;

        if effectiveness > 0.7 {
            self.successful_adjustments += 1;
        } else if effectiveness < 0.3 {
            self.failed_adjustments += 1;
        }

        // Update average effectiveness
        let alpha = self.learning_rate;
        self.avg_effectiveness = alpha * effectiveness + (1.0 - alpha) * self.avg_effectiveness;

        // Update confidence based on success rate
        let success_rate = self.successful_adjustments as f64 / self.total_adjustments.max(1) as f64;
        self.confidence = (0.5 * success_rate + 0.3 * self.avg_effectiveness + 0.2 * (1.0 - self.learning_rate)).clamp(0.0, 1.0);
    }
}

impl RateLimitState {
    fn new(config: &RateLimitingConfig) -> Self {
        let token_bucket = TokenBucket::new(
            config.max_requests_per_second * 10.0, // 10-second capacity
            config.max_requests_per_second,
            config.burst_multiplier,
        );

        Self {
            current_rps_limit: config.max_requests_per_second,
            current_token_limit: config.max_tokens_per_minute,
            current_cost_limit: config.max_cost_per_hour,
            token_bucket,
            adjustment_history: VecDeque::with_capacity(100),
            error_rate_history: VecDeque::with_capacity(100),
            cost_rate_history: VecDeque::with_capacity(100),
            traffic_pattern: TrafficPattern::new(),
            learning_metrics: LearningMetrics::new(),
            last_adjustment: Utc::now(),
            decisions_made: 0,
        }
    }

    /// Add error rate sample
    fn add_error_rate_sample(&mut self, error_rate_pct: f64, request_count: u64) {
        let sample = ErrorRateSample {
            timestamp: Utc::now(),
            error_rate_pct,
            request_count,
        };

        self.error_rate_history.push_back(sample);

        // Keep last 100 samples
        if self.error_rate_history.len() > 100 {
            self.error_rate_history.pop_front();
        }
    }

    /// Add cost rate sample
    fn add_cost_rate_sample(&mut self, cost_per_hour: f64, tokens_per_minute: f64) {
        let sample = CostRateSample {
            timestamp: Utc::now(),
            cost_per_hour,
            tokens_per_minute,
        };

        self.cost_rate_history.push_back(sample);

        // Keep last 100 samples
        if self.cost_rate_history.len() > 100 {
            self.cost_rate_history.pop_front();
        }
    }

    /// Get recent average error rate
    fn recent_avg_error_rate(&self) -> f64 {
        if self.error_rate_history.is_empty() {
            return 0.0;
        }

        let sum: f64 = self.error_rate_history.iter().map(|s| s.error_rate_pct).sum();
        sum / self.error_rate_history.len() as f64
    }

    /// Get recent average cost rate
    fn recent_avg_cost_rate(&self) -> f64 {
        if self.cost_rate_history.is_empty() {
            return 0.0;
        }

        let sum: f64 = self.cost_rate_history.iter().map(|s| s.cost_per_hour).sum();
        sum / self.cost_rate_history.len() as f64
    }

    /// Check if adjustment is needed
    fn needs_adjustment(&self, config: &RateLimitingConfig) -> bool {
        // Don't adjust too frequently
        let min_interval = ChronoDuration::minutes(5);
        if Utc::now() - self.last_adjustment < min_interval {
            return false;
        }

        // Check error rate
        let avg_error_rate = self.recent_avg_error_rate();
        if config.adaptive_enabled && avg_error_rate > config.target_error_rate_pct * 1.5 {
            return true;
        }

        // Check cost overrun
        let avg_cost = self.recent_avg_cost_rate();
        if avg_cost > config.max_cost_per_hour * 0.9 {
            return true;
        }

        // Check if we're significantly under limits (opportunity to increase)
        if avg_error_rate < config.target_error_rate_pct * 0.5
            && avg_cost < config.max_cost_per_hour * 0.5 {
            return true;
        }

        false
    }
}

impl RateLimitingStrategy {
    /// Create a new rate limiting strategy
    pub fn new(config: RateLimitingConfig) -> Self {
        let state = RateLimitState::new(&config);

        Self {
            config,
            state: Arc::new(RwLock::new(state)),
        }
    }

    /// Calculate optimal rate limits based on current metrics
    async fn calculate_optimal_limits(
        &self,
        input: &DecisionInput,
        criteria: &DecisionCriteria,
    ) -> (f64, u64, f64, f64) {
        let state = self.state.read().await;
        let metrics = &input.current_metrics;

        // Calculate current rates
        let current_error_rate = metrics.error_rate_pct;
        let current_cost_rate = metrics.daily_cost_usd / 24.0; // Convert to hourly
        let current_rps = metrics.throughput_rps;

        // Calculate adjustment factor based on error rate
        let error_adjustment = if self.config.adaptive_enabled {
            if current_error_rate > self.config.target_error_rate_pct * 2.0 {
                0.7 // Significant reduction
            } else if current_error_rate > self.config.target_error_rate_pct * 1.5 {
                0.85 // Moderate reduction
            } else if current_error_rate > self.config.target_error_rate_pct {
                0.95 // Small reduction
            } else if current_error_rate < self.config.target_error_rate_pct * 0.5 {
                1.1 // Small increase
            } else {
                1.0 // No change
            }
        } else {
            1.0
        };

        // Calculate adjustment factor based on cost
        let cost_adjustment = if current_cost_rate > self.config.max_cost_per_hour * 0.9 {
            0.8 // Reduce to stay within budget
        } else if current_cost_rate > self.config.max_cost_per_hour * 0.8 {
            0.9 // Small reduction
        } else if current_cost_rate < self.config.max_cost_per_hour * 0.5 {
            1.15 // Increase if we have budget headroom
        } else {
            1.0
        };

        // Calculate adjustment factor based on traffic pattern
        let traffic_adjustment = if state.traffic_pattern.volatility > 0.5 {
            1.05 // Slightly increase for volatile traffic
        } else {
            1.0
        };

        // Combine adjustment factors with learning
        let combined_adjustment = error_adjustment * cost_adjustment * traffic_adjustment;
        let learned_adjustment = state.learning_metrics.confidence * combined_adjustment
            + (1.0 - state.learning_metrics.confidence);

        // Apply gradual changes (max 20% per adjustment)
        let safe_adjustment = if learned_adjustment > 1.0 {
            learned_adjustment.min(1.2)
        } else {
            learned_adjustment.max(0.8)
        };

        // Calculate new limits
        let new_rps_limit = (state.current_rps_limit * safe_adjustment)
            .max(1.0) // Minimum 1 RPS
            .min(self.config.max_requests_per_second * 2.0); // Don't exceed 2x configured max

        let new_token_limit = (state.current_token_limit as f64 * safe_adjustment) as u64;
        let new_cost_limit = (state.current_cost_limit * safe_adjustment)
            .max(1.0)
            .min(self.config.max_cost_per_hour * 1.5);

        // Calculate confidence based on learning metrics and data quality
        let confidence = state.learning_metrics.confidence *
            (1.0 - state.traffic_pattern.volatility * 0.3);

        (new_rps_limit, new_token_limit, new_cost_limit, confidence)
    }

    /// Perform safety checks for rate limit adjustment
    async fn perform_safety_checks(
        &self,
        new_rps: f64,
        new_tokens: u64,
        new_cost: f64,
        criteria: &DecisionCriteria,
    ) -> Vec<SafetyCheck> {
        let state = self.state.read().await;
        let mut checks = Vec::new();

        // Check 1: Minimum availability (don't reduce too much)
        let min_rps = self.config.max_requests_per_second * 0.1;
        let availability_check = SafetyCheck {
            name: "availability_check".to_string(),
            passed: new_rps >= min_rps,
            details: format!(
                "New RPS limit {:.2} vs minimum {:.2}",
                new_rps, min_rps
            ),
            timestamp: Utc::now(),
        };
        checks.push(availability_check);

        // Check 2: Gradual change (max 20% change)
        let change_pct = ((new_rps - state.current_rps_limit) / state.current_rps_limit).abs();
        let gradual_check = SafetyCheck {
            name: "gradual_change_check".to_string(),
            passed: change_pct <= 0.2,
            details: format!("Change {:.1}% (max 20%)", change_pct * 100.0),
            timestamp: Utc::now(),
        };
        checks.push(gradual_check);

        // Check 3: Cost constraint
        let cost_check = SafetyCheck {
            name: "cost_check".to_string(),
            passed: new_cost <= self.config.max_cost_per_hour * 1.5,
            details: format!(
                "New cost limit ${:.2}/hr vs max ${:.2}/hr",
                new_cost,
                self.config.max_cost_per_hour * 1.5
            ),
            timestamp: Utc::now(),
        };
        checks.push(cost_check);

        // Check 4: Token budget
        let token_check = SafetyCheck {
            name: "token_budget_check".to_string(),
            passed: new_tokens <= self.config.max_tokens_per_minute * 2,
            details: format!(
                "New token limit {} vs max {}",
                new_tokens,
                self.config.max_tokens_per_minute * 2
            ),
            timestamp: Utc::now(),
        };
        checks.push(token_check);

        // Check 5: Learning confidence
        let confidence_check = SafetyCheck {
            name: "learning_confidence_check".to_string(),
            passed: state.learning_metrics.confidence >= 0.3,
            details: format!(
                "Learning confidence {:.2}",
                state.learning_metrics.confidence
            ),
            timestamp: Utc::now(),
        };
        checks.push(confidence_check);

        checks
    }

    /// Create rollback plan
    async fn create_rollback_plan(&self) -> RollbackPlan {
        let state = self.state.read().await;

        let revert_changes = vec![
            ConfigChange {
                config_type: ConfigType::RateLimit,
                path: "rate_limit.requests_per_second".to_string(),
                old_value: None,
                new_value: serde_json::json!(state.current_rps_limit),
                description: "Revert RPS limit".to_string(),
            },
            ConfigChange {
                config_type: ConfigType::RateLimit,
                path: "rate_limit.tokens_per_minute".to_string(),
                old_value: None,
                new_value: serde_json::json!(state.current_token_limit),
                description: "Revert token limit".to_string(),
            },
            ConfigChange {
                config_type: ConfigType::RateLimit,
                path: "rate_limit.cost_per_hour".to_string(),
                old_value: None,
                new_value: serde_json::json!(state.current_cost_limit),
                description: "Revert cost limit".to_string(),
            },
        ];

        let trigger_conditions = vec![
            RollbackCondition {
                metric: "error_rate_pct".to_string(),
                operator: ComparisonOperator::GreaterThan,
                threshold: self.config.target_error_rate_pct * 3.0,
                description: "Error rate exceeds 3x target".to_string(),
            },
            RollbackCondition {
                metric: "cost_per_hour".to_string(),
                operator: ComparisonOperator::GreaterThan,
                threshold: self.config.max_cost_per_hour * 1.5,
                description: "Cost exceeds 1.5x budget".to_string(),
            },
            RollbackCondition {
                metric: "success_rate_pct".to_string(),
                operator: ComparisonOperator::LessThan,
                threshold: 90.0,
                description: "Success rate drops below 90%".to_string(),
            },
        ];

        RollbackPlan {
            description: "Revert rate limit changes if metrics degrade".to_string(),
            revert_changes,
            trigger_conditions,
            max_duration: std::time::Duration::from_secs(600), // 10 minutes
        }
    }

    /// Update internal state with new metrics
    async fn update_state(&self, input: &DecisionInput) {
        let mut state = self.state.write().await;
        let metrics = &input.current_metrics;

        // Update error rate history
        state.add_error_rate_sample(metrics.error_rate_pct, metrics.request_count);

        // Update cost rate history
        let cost_per_hour = metrics.daily_cost_usd / 24.0;
        let tokens_per_minute = metrics.throughput_rps * 60.0; // Rough estimate
        state.add_cost_rate_sample(cost_per_hour, tokens_per_minute);

        // Update traffic pattern
        state.traffic_pattern.update(metrics.throughput_rps, metrics.throughput_rps);
    }
}

#[async_trait]
impl OptimizationStrategy for RateLimitingStrategy {
    fn name(&self) -> &str {
        "rate_limiting"
    }

    fn priority(&self) -> u32 {
        self.config.priority
    }

    fn is_applicable(&self, input: &DecisionInput) -> bool {
        if !self.config.enabled {
            return false;
        }

        let metrics = &input.current_metrics;

        // Applicable if:
        // 1. Error rate is high
        // 2. Cost is approaching or exceeding budget
        // 3. There's opportunity to increase limits safely

        let error_rate_high = metrics.error_rate_pct > self.config.target_error_rate_pct;
        let cost_high = metrics.daily_cost_usd / 24.0 > self.config.max_cost_per_hour * 0.8;
        let has_headroom = metrics.error_rate_pct < self.config.target_error_rate_pct * 0.5
            && metrics.daily_cost_usd / 24.0 < self.config.max_cost_per_hour * 0.6;

        error_rate_high || cost_high || has_headroom
    }

    async fn evaluate(
        &self,
        input: &DecisionInput,
        criteria: &DecisionCriteria,
    ) -> DecisionResult<Vec<Decision>> {
        // Update internal state
        self.update_state(input).await;

        // Check if adjustment is needed
        let state = self.state.read().await;
        if !state.needs_adjustment(&self.config) {
            debug!("Rate limit adjustment not needed at this time");
            return Ok(vec![]);
        }
        drop(state);

        // Calculate optimal limits
        let (new_rps, new_tokens, new_cost, confidence) =
            self.calculate_optimal_limits(input, criteria).await;

        // Check if limits actually changed
        let state = self.state.read().await;
        let rps_change = (new_rps - state.current_rps_limit).abs();
        let token_change = (new_tokens as i64 - state.current_token_limit as i64).abs();
        let cost_change = (new_cost - state.current_cost_limit).abs();
        drop(state);

        if rps_change < 0.1 && token_change < 100 && cost_change < 0.1 {
            debug!("Rate limits unchanged, no decision needed");
            return Ok(vec![]);
        }

        // Perform safety checks
        let safety_checks = self.perform_safety_checks(new_rps, new_tokens, new_cost, criteria).await;

        // Check if all critical safety checks passed
        let all_passed = safety_checks.iter().all(|check| check.passed);
        if !all_passed {
            warn!("Rate limit adjustment failed safety checks");
            return Ok(vec![]);
        }

        // Create configuration changes
        let state = self.state.read().await;
        let config_changes = vec![
            ConfigChange {
                config_type: ConfigType::RateLimit,
                path: "rate_limit.requests_per_second".to_string(),
                old_value: Some(serde_json::json!(state.current_rps_limit)),
                new_value: serde_json::json!(new_rps),
                description: format!(
                    "Adjust RPS limit from {:.2} to {:.2}",
                    state.current_rps_limit, new_rps
                ),
            },
            ConfigChange {
                config_type: ConfigType::RateLimit,
                path: "rate_limit.tokens_per_minute".to_string(),
                old_value: Some(serde_json::json!(state.current_token_limit)),
                new_value: serde_json::json!(new_tokens),
                description: format!(
                    "Adjust token limit from {} to {}",
                    state.current_token_limit, new_tokens
                ),
            },
            ConfigChange {
                config_type: ConfigType::RateLimit,
                path: "rate_limit.cost_per_hour".to_string(),
                old_value: Some(serde_json::json!(state.current_cost_limit)),
                new_value: serde_json::json!(new_cost),
                description: format!(
                    "Adjust cost limit from ${:.2} to ${:.2}",
                    state.current_cost_limit, new_cost
                ),
            },
        ];

        // Calculate expected impact
        let rps_change_pct = ((new_rps - state.current_rps_limit) / state.current_rps_limit) * 100.0;
        let expected_cost_change = state.current_cost_limit - new_cost;
        let expected_throughput_change = new_rps - state.current_rps_limit;

        let expected_impact = ExpectedImpact {
            cost_change_usd: Some(expected_cost_change),
            latency_change_ms: Some(if rps_change_pct < 0.0 { -10.0 } else { 5.0 }),
            quality_change: None,
            throughput_change_rps: Some(expected_throughput_change),
            success_rate_change_pct: Some(if rps_change_pct < 0.0 { 1.0 } else { -0.5 }),
            time_to_impact: std::time::Duration::from_secs(60),
            confidence,
        };

        // Generate justification
        let avg_error_rate = state.recent_avg_error_rate();
        let avg_cost_rate = state.recent_avg_cost_rate();

        let justification = if rps_change_pct < 0.0 {
            format!(
                "Reducing rate limits by {:.1}% due to: error rate {:.2}% (target {:.2}%), \
                 cost ${:.2}/hr (budget ${:.2}/hr). This will improve stability and control costs.",
                rps_change_pct.abs(), avg_error_rate, self.config.target_error_rate_pct,
                avg_cost_rate, self.config.max_cost_per_hour
            )
        } else {
            format!(
                "Increasing rate limits by {:.1}% due to: low error rate {:.2}% (target {:.2}%), \
                 cost headroom ${:.2}/hr available. This will improve throughput.",
                rps_change_pct, avg_error_rate, self.config.target_error_rate_pct,
                self.config.max_cost_per_hour - avg_cost_rate
            )
        };

        drop(state);

        // Create rollback plan
        let rollback_plan = self.create_rollback_plan().await;

        // Create decision
        let decision = Decision {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            strategy: self.name().to_string(),
            decision_type: DecisionType::AdjustRateLimits,
            confidence,
            expected_impact,
            config_changes,
            justification,
            related_insights: input
                .insights
                .iter()
                .filter(|i| {
                    i.title.to_lowercase().contains("rate")
                        || i.title.to_lowercase().contains("error")
                        || i.title.to_lowercase().contains("cost")
                })
                .map(|i| i.id.clone())
                .collect(),
            related_recommendations: input
                .recommendations
                .iter()
                .filter(|r| {
                    r.title.to_lowercase().contains("rate")
                        || r.title.to_lowercase().contains("limit")
                })
                .map(|r| r.id.clone())
                .collect(),
            priority: if avg_error_rate > self.config.target_error_rate_pct * 2.0 {
                90 // High priority if error rate is very high
            } else if avg_cost_rate > self.config.max_cost_per_hour {
                85 // High priority if over budget
            } else {
                self.priority()
            },
            requires_approval: rps_change_pct.abs() > 15.0, // Require approval for large changes
            safety_checks,
            rollback_plan: Some(rollback_plan),
            metadata: {
                let mut meta = std::collections::HashMap::new();
                meta.insert("strategy_version".to_string(), serde_json::json!("1.0"));
                meta.insert("adjustment_type".to_string(),
                    serde_json::json!(if rps_change_pct < 0.0 { "decrease" } else { "increase" }));
                meta.insert("rps_change_pct".to_string(), serde_json::json!(rps_change_pct));
                meta.insert("token_change".to_string(), serde_json::json!(token_change));
                meta.insert("cost_change".to_string(), serde_json::json!(cost_change));
                meta
            },
        };

        info!(
            "Generated rate limiting decision: RPS {:.2} -> {:.2} ({:+.1}%)",
            self.state.read().await.current_rps_limit, new_rps, rps_change_pct
        );

        Ok(vec![decision])
    }

    async fn validate(
        &self,
        decision: &Decision,
        criteria: &DecisionCriteria,
    ) -> DecisionResult<()> {
        // Verify decision is for rate limiting
        if decision.decision_type != DecisionType::AdjustRateLimits {
            return Err(DecisionError::ValidationFailed(
                "Decision is not a rate limit adjustment".to_string(),
            ));
        }

        // Verify strategy matches
        if decision.strategy != self.name() {
            return Err(DecisionError::ValidationFailed(
                "Decision strategy does not match".to_string(),
            ));
        }

        // Verify confidence threshold
        if decision.confidence < criteria.min_confidence {
            return Err(DecisionError::ValidationFailed(format!(
                "Confidence {:.2} below minimum {:.2}",
                decision.confidence, criteria.min_confidence
            )));
        }

        // Verify all safety checks passed
        let all_passed = decision.safety_checks.iter().all(|check| check.passed);
        if !all_passed {
            return Err(DecisionError::ValidationFailed(
                "Not all safety checks passed".to_string(),
            ));
        }

        // Verify config changes are present
        if decision.config_changes.is_empty() {
            return Err(DecisionError::ValidationFailed(
                "No configuration changes specified".to_string(),
            ));
        }

        // Verify rollback plan exists
        if decision.rollback_plan.is_none() {
            return Err(DecisionError::ValidationFailed(
                "No rollback plan specified".to_string(),
            ));
        }

        Ok(())
    }

    async fn learn(
        &mut self,
        decision: &Decision,
        outcome: &DecisionOutcome,
    ) -> DecisionResult<()> {
        let mut state = self.state.write().await;

        // Calculate effectiveness
        let effectiveness = if outcome.success {
            if let Some(actual_impact) = &outcome.actual_impact {
                // Compare actual vs expected impact
                let expected_cost_change = decision
                    .expected_impact
                    .cost_change_usd
                    .unwrap_or(0.0);
                let actual_cost_change = actual_impact.cost_change_usd;

                // Calculate how close we were to expectations
                let cost_accuracy = if expected_cost_change.abs() > 0.01 {
                    1.0 - ((actual_cost_change - expected_cost_change).abs() / expected_cost_change.abs()).min(1.0)
                } else {
                    0.8
                };

                // Factor in whether outcome was positive
                let outcome_quality = if outcome.rolled_back {
                    0.2 // Poor outcome if we had to rollback
                } else if actual_cost_change < 0.0 {
                    0.9 // Good if we saved money
                } else {
                    0.6 // Neutral if cost increased but manageable
                };

                (cost_accuracy * 0.4 + outcome_quality * 0.6).clamp(0.0, 1.0)
            } else {
                0.7 // Success but no impact data yet
            }
        } else {
            0.1 // Failed execution
        };

        // Update learning metrics
        state.learning_metrics.record_outcome(effectiveness);

        // If this was a successful decision, update our limits
        if outcome.success && !outcome.rolled_back {
            if let Some(actual_impact) = &outcome.actual_impact {
                // Extract new limits from the decision
                for change in &decision.config_changes {
                    match change.path.as_str() {
                        "rate_limit.requests_per_second" => {
                            if let Some(new_rps) = change.new_value.as_f64() {
                                state.current_rps_limit = new_rps;
                                state.token_bucket.update(
                                    new_rps * 10.0,
                                    new_rps,
                                );
                            }
                        }
                        "rate_limit.tokens_per_minute" => {
                            if let Some(new_tokens) = change.new_value.as_u64() {
                                state.current_token_limit = new_tokens;
                            }
                        }
                        "rate_limit.cost_per_hour" => {
                            if let Some(new_cost) = change.new_value.as_f64() {
                                state.current_cost_limit = new_cost;
                            }
                        }
                        _ => {}
                    }
                }
            }

            // Record the adjustment
            let adjustment = RateLimitAdjustment {
                timestamp: outcome.execution_time,
                old_rps_limit: decision
                    .config_changes
                    .iter()
                    .find(|c| c.path == "rate_limit.requests_per_second")
                    .and_then(|c| c.old_value.as_ref())
                    .and_then(|v| v.as_f64())
                    .unwrap_or(state.current_rps_limit),
                new_rps_limit: state.current_rps_limit,
                old_token_limit: decision
                    .config_changes
                    .iter()
                    .find(|c| c.path == "rate_limit.tokens_per_minute")
                    .and_then(|c| c.old_value.as_ref())
                    .and_then(|v| v.as_u64())
                    .unwrap_or(state.current_token_limit),
                new_token_limit: state.current_token_limit,
                old_cost_limit: decision
                    .config_changes
                    .iter()
                    .find(|c| c.path == "rate_limit.cost_per_hour")
                    .and_then(|c| c.old_value.as_ref())
                    .and_then(|v| v.as_f64())
                    .unwrap_or(state.current_cost_limit),
                new_cost_limit: state.current_cost_limit,
                reason: decision.justification.clone(),
                effectiveness: Some(effectiveness),
            };

            state.adjustment_history.push_back(adjustment);

            // Keep last 100 adjustments
            if state.adjustment_history.len() > 100 {
                state.adjustment_history.pop_front();
            }
        }

        state.last_adjustment = Utc::now();
        state.decisions_made += 1;

        info!(
            "Learned from rate limiting decision: effectiveness={:.2}, confidence={:.2}",
            effectiveness, state.learning_metrics.confidence
        );

        Ok(())
    }

    fn get_stats(&self) -> serde_json::Value {
        // Use try_read to avoid blocking
        if let Ok(state) = self.state.try_read() {
            serde_json::json!({
                "current_rps_limit": state.current_rps_limit,
                "current_token_limit": state.current_token_limit,
                "current_cost_limit": state.current_cost_limit,
                "token_bucket_utilization": state.token_bucket.utilization(),
                "total_adjustments": state.adjustment_history.len(),
                "learning_metrics": {
                    "total_adjustments": state.learning_metrics.total_adjustments,
                    "successful_adjustments": state.learning_metrics.successful_adjustments,
                    "failed_adjustments": state.learning_metrics.failed_adjustments,
                    "avg_effectiveness": state.learning_metrics.avg_effectiveness,
                    "confidence": state.learning_metrics.confidence,
                },
                "traffic_pattern": {
                    "avg_rps": state.traffic_pattern.avg_rps,
                    "peak_rps": state.traffic_pattern.peak_rps,
                    "volatility": state.traffic_pattern.volatility,
                },
                "recent_error_rate": state.recent_avg_error_rate(),
                "recent_cost_rate": state.recent_avg_cost_rate(),
                "decisions_made": state.decisions_made,
            })
        } else {
            serde_json::json!({
                "error": "Unable to acquire state lock"
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::{AnalysisReport, Insight, InsightCategory, Recommendation, Severity, Confidence};
    use std::collections::HashMap;

    fn create_test_config() -> RateLimitingConfig {
        RateLimitingConfig {
            enabled: true,
            priority: 80,
            min_confidence: 0.6,
            max_requests_per_second: 100.0,
            max_tokens_per_minute: 100000,
            max_cost_per_hour: 10.0,
            burst_multiplier: 1.5,
            adaptive_enabled: true,
            target_error_rate_pct: 1.0,
        }
    }

    fn create_test_input(error_rate: f64, cost_per_day: f64, throughput: f64) -> DecisionInput {
        DecisionInput {
            timestamp: Utc::now(),
            analysis_reports: vec![],
            insights: vec![],
            recommendations: vec![],
            current_metrics: super::super::types::SystemMetrics {
                avg_latency_ms: 100.0,
                p95_latency_ms: 200.0,
                p99_latency_ms: 300.0,
                success_rate_pct: 99.0,
                error_rate_pct: error_rate,
                throughput_rps: throughput,
                total_cost_usd: cost_per_day * 7.0,
                daily_cost_usd: cost_per_day,
                avg_rating: Some(4.5),
                cache_hit_rate_pct: Some(30.0),
                request_count: 10000,
                timestamp: Utc::now(),
            },
            context: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_strategy_creation() {
        let config = create_test_config();
        let strategy = RateLimitingStrategy::new(config);

        assert_eq!(strategy.name(), "rate_limiting");
        assert_eq!(strategy.priority(), 80);
    }

    #[tokio::test]
    async fn test_token_bucket_basic() {
        let mut bucket = TokenBucket::new(100.0, 10.0, 1.5);

        // Should allow consuming tokens up to capacity
        assert!(bucket.try_consume(50.0));
        assert!(bucket.try_consume(50.0));
        assert!(!bucket.try_consume(1.0)); // Exceeds capacity
    }

    #[tokio::test]
    async fn test_token_bucket_refill() {
        let mut bucket = TokenBucket::new(100.0, 100.0, 1.5); // 100 tokens/sec

        // Consume all tokens
        assert!(bucket.try_consume(100.0));
        assert!(!bucket.try_consume(1.0));

        // Wait and refill
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        bucket.refill();

        // Should have ~50 tokens after 0.5 seconds at 100/sec
        assert!(bucket.try_consume(40.0));
    }

    #[tokio::test]
    async fn test_token_bucket_burst() {
        let mut bucket = TokenBucket::new(100.0, 10.0, 2.0); // 2x burst

        // Can burst up to 200 tokens
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        bucket.refill();

        assert!(bucket.try_consume(150.0)); // Within burst
    }

    #[tokio::test]
    async fn test_is_applicable_high_error_rate() {
        let config = create_test_config();
        let strategy = RateLimitingStrategy::new(config);

        // High error rate should trigger applicability
        let input = create_test_input(3.0, 100.0, 50.0);
        assert!(strategy.is_applicable(&input));
    }

    #[tokio::test]
    async fn test_is_applicable_high_cost() {
        let config = create_test_config();
        let strategy = RateLimitingStrategy::new(config);

        // High cost should trigger applicability (>80% of budget)
        let input = create_test_input(0.5, 200.0, 50.0); // $200/day = $8.33/hr
        assert!(strategy.is_applicable(&input));
    }

    #[tokio::test]
    async fn test_is_applicable_headroom() {
        let config = create_test_config();
        let strategy = RateLimitingStrategy::new(config);

        // Low error and cost should trigger increase opportunity
        let input = create_test_input(0.2, 50.0, 50.0);
        assert!(strategy.is_applicable(&input));
    }

    #[tokio::test]
    async fn test_not_applicable_disabled() {
        let mut config = create_test_config();
        config.enabled = false;
        let strategy = RateLimitingStrategy::new(config);

        let input = create_test_input(3.0, 200.0, 50.0);
        assert!(!strategy.is_applicable(&input));
    }

    #[tokio::test]
    async fn test_evaluate_reduce_on_high_error_rate() {
        let config = create_test_config();
        let mut strategy = RateLimitingStrategy::new(config);

        // Create input with high error rate
        let input = create_test_input(5.0, 100.0, 80.0);

        // Add some samples to trigger adjustment
        {
            let mut state = strategy.state.write().await;
            state.last_adjustment = Utc::now() - ChronoDuration::minutes(10);
        }

        let criteria = DecisionCriteria::default();
        let decisions = strategy.evaluate(&input, &criteria).await.unwrap();

        assert!(!decisions.is_empty());
        let decision = &decisions[0];
        assert_eq!(decision.decision_type, DecisionType::AdjustRateLimits);

        // Should reduce limits
        let rps_change = decision.config_changes
            .iter()
            .find(|c| c.path == "rate_limit.requests_per_second")
            .unwrap();
        let new_rps: f64 = rps_change.new_value.as_f64().unwrap();
        let old_rps: f64 = rps_change.old_value.as_ref().unwrap().as_f64().unwrap();
        assert!(new_rps < old_rps, "RPS should decrease");
    }

    #[tokio::test]
    async fn test_evaluate_increase_with_headroom() {
        let config = create_test_config();
        let mut strategy = RateLimitingStrategy::new(config);

        // Create input with low error rate and cost
        let input = create_test_input(0.2, 50.0, 30.0);

        // Add some samples and set last adjustment time
        {
            let mut state = strategy.state.write().await;
            state.last_adjustment = Utc::now() - ChronoDuration::minutes(10);
            state.add_error_rate_sample(0.2, 1000);
            state.add_cost_rate_sample(2.0, 5000.0);
        }

        let criteria = DecisionCriteria::default();
        let decisions = strategy.evaluate(&input, &criteria).await.unwrap();

        if !decisions.is_empty() {
            let decision = &decisions[0];

            // Should increase limits
            let rps_change = decision.config_changes
                .iter()
                .find(|c| c.path == "rate_limit.requests_per_second")
                .unwrap();
            let new_rps: f64 = rps_change.new_value.as_f64().unwrap();
            let old_rps: f64 = rps_change.old_value.as_ref().unwrap().as_f64().unwrap();
            assert!(new_rps >= old_rps, "RPS should increase or stay same");
        }
    }

    #[tokio::test]
    async fn test_safety_checks() {
        let config = create_test_config();
        let strategy = RateLimitingStrategy::new(config);

        let criteria = DecisionCriteria::default();
        let checks = strategy.perform_safety_checks(90.0, 90000, 9.0, &criteria).await;

        assert!(!checks.is_empty());
        assert!(checks.iter().any(|c| c.name == "availability_check"));
        assert!(checks.iter().any(|c| c.name == "gradual_change_check"));
        assert!(checks.iter().any(|c| c.name == "cost_check"));
    }

    #[tokio::test]
    async fn test_safety_check_minimum_availability() {
        let config = create_test_config();
        let strategy = RateLimitingStrategy::new(config);

        let criteria = DecisionCriteria::default();
        let checks = strategy.perform_safety_checks(5.0, 5000, 1.0, &criteria).await;

        // Should fail availability check (too low)
        let availability = checks.iter().find(|c| c.name == "availability_check").unwrap();
        assert!(!availability.passed);
    }

    #[tokio::test]
    async fn test_rollback_plan_creation() {
        let config = create_test_config();
        let strategy = RateLimitingStrategy::new(config);

        let plan = strategy.create_rollback_plan().await;

        assert!(!plan.revert_changes.is_empty());
        assert!(!plan.trigger_conditions.is_empty());
        assert!(plan.trigger_conditions.iter().any(|c| c.metric == "error_rate_pct"));
        assert!(plan.trigger_conditions.iter().any(|c| c.metric == "cost_per_hour"));
    }

    #[tokio::test]
    async fn test_validate_decision() {
        let config = create_test_config();
        let strategy = RateLimitingStrategy::new(config);

        let decision = Decision {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            strategy: "rate_limiting".to_string(),
            decision_type: DecisionType::AdjustRateLimits,
            confidence: 0.8,
            expected_impact: ExpectedImpact {
                cost_change_usd: Some(-1.0),
                latency_change_ms: Some(-5.0),
                quality_change: None,
                throughput_change_rps: Some(-10.0),
                success_rate_change_pct: Some(0.5),
                time_to_impact: std::time::Duration::from_secs(60),
                confidence: 0.8,
            },
            config_changes: vec![ConfigChange {
                config_type: ConfigType::RateLimit,
                path: "rate_limit.requests_per_second".to_string(),
                old_value: Some(serde_json::json!(100.0)),
                new_value: serde_json::json!(90.0),
                description: "Reduce RPS".to_string(),
            }],
            justification: "Test".to_string(),
            related_insights: vec![],
            related_recommendations: vec![],
            priority: 80,
            requires_approval: false,
            safety_checks: vec![SafetyCheck {
                name: "test_check".to_string(),
                passed: true,
                details: "OK".to_string(),
                timestamp: Utc::now(),
            }],
            rollback_plan: Some(RollbackPlan {
                description: "Test rollback".to_string(),
                revert_changes: vec![],
                trigger_conditions: vec![],
                max_duration: std::time::Duration::from_secs(300),
            }),
            metadata: HashMap::new(),
        };

        let criteria = DecisionCriteria::default();
        assert!(strategy.validate(&decision, &criteria).await.is_ok());
    }

    #[tokio::test]
    async fn test_validate_wrong_decision_type() {
        let config = create_test_config();
        let strategy = RateLimitingStrategy::new(config);

        let decision = Decision {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            strategy: "rate_limiting".to_string(),
            decision_type: DecisionType::ModelSwitch, // Wrong type
            confidence: 0.8,
            expected_impact: ExpectedImpact {
                cost_change_usd: None,
                latency_change_ms: None,
                quality_change: None,
                throughput_change_rps: None,
                success_rate_change_pct: None,
                time_to_impact: std::time::Duration::from_secs(60),
                confidence: 0.8,
            },
            config_changes: vec![],
            justification: "Test".to_string(),
            related_insights: vec![],
            related_recommendations: vec![],
            priority: 80,
            requires_approval: false,
            safety_checks: vec![],
            rollback_plan: None,
            metadata: HashMap::new(),
        };

        let criteria = DecisionCriteria::default();
        assert!(strategy.validate(&decision, &criteria).await.is_err());
    }

    #[tokio::test]
    async fn test_learn_successful_outcome() {
        let config = create_test_config();
        let mut strategy = RateLimitingStrategy::new(config);

        let decision = Decision {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            strategy: "rate_limiting".to_string(),
            decision_type: DecisionType::AdjustRateLimits,
            confidence: 0.8,
            expected_impact: ExpectedImpact {
                cost_change_usd: Some(-1.0),
                latency_change_ms: Some(-5.0),
                quality_change: None,
                throughput_change_rps: Some(-10.0),
                success_rate_change_pct: Some(0.5),
                time_to_impact: std::time::Duration::from_secs(60),
                confidence: 0.8,
            },
            config_changes: vec![
                ConfigChange {
                    config_type: ConfigType::RateLimit,
                    path: "rate_limit.requests_per_second".to_string(),
                    old_value: Some(serde_json::json!(100.0)),
                    new_value: serde_json::json!(90.0),
                    description: "Reduce RPS".to_string(),
                },
                ConfigChange {
                    config_type: ConfigType::RateLimit,
                    path: "rate_limit.tokens_per_minute".to_string(),
                    old_value: Some(serde_json::json!(100000)),
                    new_value: serde_json::json!(90000),
                    description: "Reduce tokens".to_string(),
                },
            ],
            justification: "Test".to_string(),
            related_insights: vec![],
            related_recommendations: vec![],
            priority: 80,
            requires_approval: false,
            safety_checks: vec![],
            rollback_plan: None,
            metadata: HashMap::new(),
        };

        let outcome = DecisionOutcome {
            decision_id: decision.id.clone(),
            execution_time: Utc::now(),
            success: true,
            error: None,
            actual_impact: Some(super::super::types::ActualImpact {
                cost_change_usd: -1.2,
                latency_change_ms: -4.0,
                quality_change: 0.0,
                throughput_change_rps: -10.0,
                success_rate_change_pct: 0.5,
                time_to_impact: std::time::Duration::from_secs(60),
                variance_from_expected: 0.2,
            }),
            rolled_back: false,
            rollback_reason: None,
            metrics_before: create_test_input(2.0, 100.0, 100.0).current_metrics,
            metrics_after: Some(create_test_input(1.5, 95.0, 90.0).current_metrics),
            duration: std::time::Duration::from_secs(300),
        };

        let result = strategy.learn(&decision, &outcome).await;
        assert!(result.is_ok());

        // Check that learning metrics were updated
        let state = strategy.state.read().await;
        assert_eq!(state.learning_metrics.total_adjustments, 1);
        assert!(state.learning_metrics.avg_effectiveness > 0.5);
        assert_eq!(state.current_rps_limit, 90.0);
    }

    #[tokio::test]
    async fn test_learn_failed_outcome() {
        let config = create_test_config();
        let mut strategy = RateLimitingStrategy::new(config);

        let decision = Decision {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            strategy: "rate_limiting".to_string(),
            decision_type: DecisionType::AdjustRateLimits,
            confidence: 0.8,
            expected_impact: ExpectedImpact {
                cost_change_usd: Some(-1.0),
                latency_change_ms: None,
                quality_change: None,
                throughput_change_rps: None,
                success_rate_change_pct: None,
                time_to_impact: std::time::Duration::from_secs(60),
                confidence: 0.8,
            },
            config_changes: vec![],
            justification: "Test".to_string(),
            related_insights: vec![],
            related_recommendations: vec![],
            priority: 80,
            requires_approval: false,
            safety_checks: vec![],
            rollback_plan: None,
            metadata: HashMap::new(),
        };

        let outcome = DecisionOutcome {
            decision_id: decision.id.clone(),
            execution_time: Utc::now(),
            success: false,
            error: Some("Execution failed".to_string()),
            actual_impact: None,
            rolled_back: false,
            rollback_reason: None,
            metrics_before: create_test_input(2.0, 100.0, 100.0).current_metrics,
            metrics_after: None,
            duration: std::time::Duration::from_secs(10),
        };

        let result = strategy.learn(&decision, &outcome).await;
        assert!(result.is_ok());

        // Check that learning metrics were updated with low effectiveness
        let state = strategy.state.read().await;
        assert_eq!(state.learning_metrics.total_adjustments, 1);
        assert!(state.learning_metrics.avg_effectiveness < 0.5);
    }

    #[tokio::test]
    async fn test_get_stats() {
        let config = create_test_config();
        let strategy = RateLimitingStrategy::new(config);

        let stats = strategy.get_stats();
        assert!(stats.get("current_rps_limit").is_some());
        assert!(stats.get("current_token_limit").is_some());
        assert!(stats.get("learning_metrics").is_some());
    }

    #[tokio::test]
    async fn test_gradual_adjustment() {
        let config = create_test_config();
        let strategy = RateLimitingStrategy::new(config);

        // Simulate needing a large adjustment
        {
            let mut state = strategy.state.write().await;
            state.current_rps_limit = 100.0;
            // Set very high error rate to trigger large reduction
            state.add_error_rate_sample(10.0, 1000);
            state.add_error_rate_sample(10.0, 1000);
        }

        let input = create_test_input(10.0, 100.0, 100.0);
        let criteria = DecisionCriteria::default();

        let (new_rps, _, _, _) = strategy.calculate_optimal_limits(&input, &criteria).await;

        // Should not reduce by more than 20%
        let state = strategy.state.read().await;
        let change_pct = (state.current_rps_limit - new_rps) / state.current_rps_limit;
        assert!(change_pct <= 0.2, "Change should be gradual (<= 20%)");
    }
}
