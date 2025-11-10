//! Request Batching Strategy
//!
//! This strategy analyzes request patterns and groups similar requests to improve
//! efficiency and reduce costs. It balances latency vs cost savings by:
//!
//! - Detecting similar requests that can be batched together
//! - Calculating optimal batch sizes and timeouts
//! - Analyzing latency-cost tradeoffs
//! - Managing priority queues for different request types
//! - Learning from batching effectiveness over time
//!
//! # Example
//!
//! ```rust,no_run
//! use processor::decision::batching::BatchingStrategy;
//! use processor::decision::config::BatchingConfig;
//!
//! let config = BatchingConfig::default();
//! let mut strategy = BatchingStrategy::new(config);
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::time::Duration;
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::config::BatchingConfig;
use super::error::{DecisionError, DecisionResult};
use super::traits::OptimizationStrategy;
use super::types::*;
use crate::analyzer::{Insight, InsightCategory, Recommendation, Severity};

/// Request Batching Strategy
///
/// Groups similar requests to improve efficiency and reduce costs while
/// balancing latency requirements.
pub struct BatchingStrategy {
    config: BatchingConfig,
    stats: BatchingStats,
    request_patterns: HashMap<String, RequestPattern>,
    batch_history: VecDeque<BatchExecution>,
    similarity_cache: HashMap<String, Vec<String>>,
    learning_data: LearningData,
}

impl BatchingStrategy {
    /// Create a new batching strategy with the given configuration
    pub fn new(config: BatchingConfig) -> Self {
        Self {
            config,
            stats: BatchingStats::default(),
            request_patterns: HashMap::new(),
            batch_history: VecDeque::new(),
            similarity_cache: HashMap::new(),
            learning_data: LearningData::default(),
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(BatchingConfig::default())
    }

    /// Detect patterns in request data
    fn detect_request_patterns(&mut self, input: &DecisionInput) -> Vec<RequestPattern> {
        let mut patterns = Vec::new();

        // Analyze insights for batching opportunities
        for insight in &input.insights {
            if let Some(pattern) = self.extract_pattern_from_insight(insight) {
                patterns.push(pattern);
            }
        }

        // Analyze recommendations for batching suggestions
        for recommendation in &input.recommendations {
            if let Some(pattern) = self.extract_pattern_from_recommendation(recommendation) {
                patterns.push(pattern);
            }
        }

        // Analyze current metrics for request frequency
        if input.current_metrics.throughput_rps > 0.0 {
            let pattern = self.infer_pattern_from_metrics(&input.current_metrics);
            patterns.push(pattern);
        }

        patterns
    }

    /// Extract batching pattern from insight
    fn extract_pattern_from_insight(&self, insight: &Insight) -> Option<RequestPattern> {
        // Look for insights about similar requests or patterns
        if insight.title.to_lowercase().contains("similar")
            || insight.title.to_lowercase().contains("pattern")
            || insight.title.to_lowercase().contains("batch")
        {
            Some(RequestPattern {
                pattern_id: format!("pattern-{}", Uuid::new_v4()),
                request_type: extract_request_type(&insight.title),
                frequency_per_minute: 10.0, // Default estimate
                avg_processing_time_ms: 100.0,
                similarity_score: insight.confidence.value(),
                last_seen: Utc::now(),
            })
        } else {
            None
        }
    }

    /// Extract batching pattern from recommendation
    fn extract_pattern_from_recommendation(&self, rec: &Recommendation) -> Option<RequestPattern> {
        // Look for recommendations about batching
        if rec.action.action_type.to_lowercase().contains("batch")
            || rec.title.to_lowercase().contains("group")
            || rec.title.to_lowercase().contains("combine")
        {
            Some(RequestPattern {
                pattern_id: format!("pattern-{}", Uuid::new_v4()),
                request_type: extract_request_type(&rec.title),
                frequency_per_minute: 15.0, // Default estimate
                avg_processing_time_ms: 120.0,
                similarity_score: rec.confidence.value(),
                last_seen: Utc::now(),
            })
        } else {
            None
        }
    }

    /// Infer pattern from system metrics
    fn infer_pattern_from_metrics(&self, metrics: &SystemMetrics) -> RequestPattern {
        RequestPattern {
            pattern_id: format!("pattern-metrics-{}", Uuid::new_v4()),
            request_type: "general".to_string(),
            frequency_per_minute: metrics.throughput_rps * 60.0,
            avg_processing_time_ms: metrics.avg_latency_ms,
            similarity_score: 0.7, // Default moderate similarity
            last_seen: Utc::now(),
        }
    }

    /// Calculate optimal batch size for a pattern
    fn calculate_optimal_batch_size(
        &self,
        pattern: &RequestPattern,
        criteria: &DecisionCriteria,
    ) -> OptimalBatchSize {
        // Start with configured maximum
        let max_batch_size = self.config.max_batch_size;

        // Calculate based on frequency
        let frequency_based_size = (pattern.frequency_per_minute / 60.0
            * self.config.batch_timeout.as_millis() as f64
            / 1000.0)
            .ceil() as usize;

        // Constrain by maximum allowed latency increase
        let max_latency_ms = criteria
            .max_latency_increase_ms
            .unwrap_or(self.config.max_latency_increase_ms);

        let latency_constrained_size = if pattern.avg_processing_time_ms > 0.0 {
            (max_latency_ms / pattern.avg_processing_time_ms).floor() as usize
        } else {
            max_batch_size
        };

        // Take minimum of all constraints
        let optimal_size = frequency_based_size
            .min(latency_constrained_size)
            .min(max_batch_size)
            .max(self.config.min_requests_for_batching);

        // Calculate cost savings estimate
        let cost_savings_pct = self.estimate_cost_savings(optimal_size);

        // Calculate latency increase estimate
        let latency_increase_ms = self.estimate_latency_increase(optimal_size, pattern);

        OptimalBatchSize {
            batch_size: optimal_size,
            expected_cost_savings_pct: cost_savings_pct,
            expected_latency_increase_ms: latency_increase_ms,
            confidence: self.calculate_batch_confidence(pattern, optimal_size),
        }
    }

    /// Estimate cost savings for a batch size
    fn estimate_cost_savings(&self, batch_size: usize) -> f64 {
        // Cost savings increase with batch size but with diminishing returns
        // Formula: savings = base_rate * log(batch_size + 1) / log(max_size + 1)
        let base_savings = 30.0; // 30% max savings
        let normalized = (batch_size as f64 + 1.0).ln()
            / (self.config.max_batch_size as f64 + 1.0).ln();

        base_savings * normalized
    }

    /// Estimate latency increase for a batch size and pattern
    fn estimate_latency_increase(&self, batch_size: usize, pattern: &RequestPattern) -> f64 {
        // Latency increases linearly with wait time
        let max_wait_time_ms = self.config.batch_timeout.as_millis() as f64;

        // Average wait time is half the timeout for a full batch
        let fill_ratio = (pattern.frequency_per_minute / 60.0)
            * (max_wait_time_ms / 1000.0)
            / batch_size as f64;

        if fill_ratio >= 1.0 {
            // Batch fills quickly, minimal wait
            max_wait_time_ms * 0.3
        } else {
            // Batch takes longer to fill
            max_wait_time_ms * (1.0 - fill_ratio * 0.7)
        }
    }

    /// Calculate confidence in batch configuration
    fn calculate_batch_confidence(&self, pattern: &RequestPattern, batch_size: usize) -> f64 {
        let mut confidence = pattern.similarity_score;

        // Higher confidence if we have historical data
        if let Some(history) = self.learning_data.pattern_performance.get(&pattern.pattern_id) {
            confidence = confidence * 0.5 + history.success_rate * 0.5;
        }

        // Lower confidence for very large batches without proven history
        if batch_size > 5 && !self.learning_data.pattern_performance.contains_key(&pattern.pattern_id) {
            confidence *= 0.8;
        }

        // Higher confidence for frequently seen patterns
        if pattern.frequency_per_minute > 20.0 {
            confidence = (confidence * 1.1).min(1.0);
        }

        confidence
    }

    /// Analyze latency-cost tradeoffs
    fn analyze_tradeoffs(
        &self,
        optimal: &OptimalBatchSize,
        criteria: &DecisionCriteria,
    ) -> TradeoffAnalysis {
        let cost_benefit = optimal.expected_cost_savings_pct / 100.0;
        let latency_penalty = optimal.expected_latency_increase_ms
            / criteria
                .max_latency_increase_ms
                .unwrap_or(self.config.max_latency_increase_ms);

        // Calculate tradeoff score (higher is better)
        // Weighted by priority: cost savings vs latency impact
        let cost_weight = 0.6;
        let latency_weight = 0.4;

        let tradeoff_score = (cost_benefit * cost_weight) - (latency_penalty * latency_weight);

        let is_acceptable = tradeoff_score > 0.2 // Minimum threshold
            && optimal.expected_latency_increase_ms <= criteria
                .max_latency_increase_ms
                .unwrap_or(self.config.max_latency_increase_ms)
            && optimal.expected_cost_savings_pct >= self.config.min_cost_savings_pct;

        TradeoffAnalysis {
            tradeoff_score,
            cost_benefit,
            latency_penalty,
            is_acceptable,
            reasoning: self.generate_tradeoff_reasoning(
                optimal,
                tradeoff_score,
                is_acceptable,
            ),
        }
    }

    /// Generate human-readable reasoning for tradeoff
    fn generate_tradeoff_reasoning(
        &self,
        optimal: &OptimalBatchSize,
        score: f64,
        acceptable: bool,
    ) -> String {
        if acceptable {
            format!(
                "Batching is beneficial: {:.1}% cost savings with only {:.0}ms latency increase. \
                Tradeoff score: {:.2}",
                optimal.expected_cost_savings_pct, optimal.expected_latency_increase_ms, score
            )
        } else if optimal.expected_latency_increase_ms > self.config.max_latency_increase_ms {
            format!(
                "Latency increase ({:.0}ms) exceeds maximum allowed ({:.0}ms)",
                optimal.expected_latency_increase_ms, self.config.max_latency_increase_ms
            )
        } else if optimal.expected_cost_savings_pct < self.config.min_cost_savings_pct {
            format!(
                "Cost savings ({:.1}%) below minimum threshold ({:.1}%)",
                optimal.expected_cost_savings_pct, self.config.min_cost_savings_pct
            )
        } else {
            format!(
                "Tradeoff score ({:.2}) below acceptable threshold (0.2)",
                score
            )
        }
    }

    /// Perform safety checks for batching decision
    fn perform_safety_checks(
        &self,
        optimal: &OptimalBatchSize,
        criteria: &DecisionCriteria,
    ) -> Vec<SafetyCheck> {
        let mut checks = Vec::new();

        // Check 1: Latency increase within limits
        let latency_ok = optimal.expected_latency_increase_ms
            <= criteria
                .max_latency_increase_ms
                .unwrap_or(self.config.max_latency_increase_ms);

        checks.push(SafetyCheck {
            name: "latency_check".to_string(),
            passed: latency_ok,
            details: format!(
                "Expected latency increase: {:.0}ms (max: {:.0}ms)",
                optimal.expected_latency_increase_ms,
                criteria
                    .max_latency_increase_ms
                    .unwrap_or(self.config.max_latency_increase_ms)
            ),
            timestamp: Utc::now(),
        });

        // Check 2: Cost savings meet minimum threshold
        let cost_ok = optimal.expected_cost_savings_pct >= self.config.min_cost_savings_pct;

        checks.push(SafetyCheck {
            name: "cost_check".to_string(),
            passed: cost_ok,
            details: format!(
                "Expected cost savings: {:.1}% (min: {:.1}%)",
                optimal.expected_cost_savings_pct, self.config.min_cost_savings_pct
            ),
            timestamp: Utc::now(),
        });

        // Check 3: Confidence meets minimum threshold
        let confidence_ok = optimal.confidence >= criteria.min_confidence;

        checks.push(SafetyCheck {
            name: "confidence_check".to_string(),
            passed: confidence_ok,
            details: format!(
                "Configuration confidence: {:.2} (min: {:.2})",
                optimal.confidence, criteria.min_confidence
            ),
            timestamp: Utc::now(),
        });

        // Check 4: Quality should not be significantly impacted
        // Batching typically doesn't affect quality, but check history
        let quality_ok = if let Some(avg_history) = self.get_average_quality_impact() {
            avg_history > -0.1 // Allow up to 0.1 point quality drop
        } else {
            true // No history, assume OK
        };

        checks.push(SafetyCheck {
            name: "quality_check".to_string(),
            passed: quality_ok,
            details: if let Some(impact) = self.get_average_quality_impact() {
                format!("Historical quality impact: {:.2}", impact)
            } else {
                "No historical quality data".to_string()
            },
            timestamp: Utc::now(),
        });

        checks
    }

    /// Get average quality impact from historical data
    fn get_average_quality_impact(&self) -> Option<f64> {
        if self.batch_history.is_empty() {
            return None;
        }

        let mut total_impact = 0.0;
        let mut count = 0;

        for execution in &self.batch_history {
            if let Some(quality_impact) = execution.quality_impact {
                total_impact += quality_impact;
                count += 1;
            }
        }

        if count > 0 {
            Some(total_impact / count as f64)
        } else {
            None
        }
    }

    /// Generate batching configuration decision
    fn generate_decision(
        &self,
        pattern: &RequestPattern,
        optimal: &OptimalBatchSize,
        tradeoff: &TradeoffAnalysis,
        safety_checks: Vec<SafetyCheck>,
    ) -> Decision {
        let decision_id = Uuid::new_v4().to_string();

        let config_changes = vec![
            ConfigChange {
                config_type: ConfigType::Batching,
                path: "batching.enabled".to_string(),
                old_value: Some(serde_json::json!(false)),
                new_value: serde_json::json!(true),
                description: "Enable request batching".to_string(),
            },
            ConfigChange {
                config_type: ConfigType::Batching,
                path: "batching.batch_size".to_string(),
                old_value: None,
                new_value: serde_json::json!(optimal.batch_size),
                description: format!("Set batch size to {}", optimal.batch_size),
            },
            ConfigChange {
                config_type: ConfigType::Batching,
                path: "batching.timeout_ms".to_string(),
                old_value: None,
                new_value: serde_json::json!(self.config.batch_timeout.as_millis()),
                description: format!(
                    "Set batch timeout to {}ms",
                    self.config.batch_timeout.as_millis()
                ),
            },
            ConfigChange {
                config_type: ConfigType::Batching,
                path: "batching.request_type".to_string(),
                old_value: None,
                new_value: serde_json::json!(pattern.request_type),
                description: format!("Apply batching to '{}' requests", pattern.request_type),
            },
        ];

        let rollback_plan = RollbackPlan {
            description: "Disable batching if metrics degrade".to_string(),
            revert_changes: vec![ConfigChange {
                config_type: ConfigType::Batching,
                path: "batching.enabled".to_string(),
                old_value: Some(serde_json::json!(true)),
                new_value: serde_json::json!(false),
                description: "Disable batching".to_string(),
            }],
            trigger_conditions: vec![
                RollbackCondition {
                    metric: "avg_latency_ms".to_string(),
                    operator: ComparisonOperator::GreaterThan,
                    threshold: optimal.expected_latency_increase_ms * 1.5,
                    description: "Latency increased more than expected".to_string(),
                },
                RollbackCondition {
                    metric: "error_rate_pct".to_string(),
                    operator: ComparisonOperator::GreaterThan,
                    threshold: 2.0,
                    description: "Error rate increased significantly".to_string(),
                },
            ],
            max_duration: Duration::from_secs(1800), // 30 minutes
        };

        Decision {
            id: decision_id,
            timestamp: Utc::now(),
            strategy: "batching".to_string(),
            decision_type: DecisionType::EnableBatching,
            confidence: optimal.confidence,
            expected_impact: ExpectedImpact {
                cost_change_usd: Some(
                    -(optimal.expected_cost_savings_pct / 100.0) * 10.0, // Negative = savings
                ),
                latency_change_ms: Some(optimal.expected_latency_increase_ms),
                quality_change: None, // Batching typically doesn't affect quality
                throughput_change_rps: None, // May improve slightly but not primary benefit
                success_rate_change_pct: None,
                time_to_impact: Duration::from_secs(300), // 5 minutes
                confidence: optimal.confidence,
            },
            config_changes,
            justification: format!(
                "Enable batching for '{}' requests with batch size {} and {}ms timeout. {}",
                pattern.request_type, optimal.batch_size,
                self.config.batch_timeout.as_millis(),
                tradeoff.reasoning
            ),
            related_insights: vec![],
            related_recommendations: vec![],
            priority: self.calculate_decision_priority(optimal, tradeoff),
            requires_approval: !tradeoff.is_acceptable || optimal.expected_latency_increase_ms > 100.0,
            safety_checks,
            rollback_plan: Some(rollback_plan),
            metadata: {
                let mut meta = HashMap::new();
                meta.insert(
                    "pattern_id".to_string(),
                    serde_json::json!(pattern.pattern_id),
                );
                meta.insert(
                    "request_type".to_string(),
                    serde_json::json!(pattern.request_type),
                );
                meta.insert(
                    "batch_size".to_string(),
                    serde_json::json!(optimal.batch_size),
                );
                meta.insert(
                    "tradeoff_score".to_string(),
                    serde_json::json!(tradeoff.tradeoff_score),
                );
                meta
            },
        }
    }

    /// Calculate decision priority based on impact
    fn calculate_decision_priority(&self, optimal: &OptimalBatchSize, tradeoff: &TradeoffAnalysis) -> u32 {
        let mut priority = self.config.priority;

        // Boost priority for high cost savings
        if optimal.expected_cost_savings_pct > 25.0 {
            priority += 10;
        } else if optimal.expected_cost_savings_pct > 15.0 {
            priority += 5;
        }

        // Reduce priority for high latency impact
        if optimal.expected_latency_increase_ms > 150.0 {
            priority = priority.saturating_sub(10);
        } else if optimal.expected_latency_increase_ms > 100.0 {
            priority = priority.saturating_sub(5);
        }

        // Boost priority for high confidence
        if optimal.confidence > 0.85 {
            priority += 5;
        }

        // Boost priority for good tradeoff score
        if tradeoff.tradeoff_score > 0.5 {
            priority += 5;
        }

        priority
    }

    /// Update learning data from decision outcome
    fn update_learning(&mut self, decision: &Decision, outcome: &DecisionOutcome) {
        self.stats.total_batching_decisions += 1;

        if outcome.success {
            self.stats.successful_batching_decisions += 1;

            // Extract pattern info from metadata
            if let Some(pattern_id) = decision.metadata.get("pattern_id") {
                if let Some(pattern_id_str) = pattern_id.as_str() {
                    let performance = self
                        .learning_data
                        .pattern_performance
                        .entry(pattern_id_str.to_string())
                        .or_insert_with(PatternPerformance::default);

                    performance.total_executions += 1;
                    performance.successful_executions += 1;
                    performance.success_rate =
                        performance.successful_executions as f64 / performance.total_executions as f64;

                    // Update with actual impact
                    if let Some(actual_impact) = &outcome.actual_impact {
                        performance.total_cost_savings += actual_impact.cost_change_usd.abs();
                        performance.total_latency_increase += actual_impact.latency_change_ms;

                        // Update averages
                        performance.avg_cost_savings = performance.total_cost_savings
                            / performance.successful_executions as f64;
                        performance.avg_latency_increase = performance.total_latency_increase
                            / performance.successful_executions as f64;

                        // Track variance from expected
                        let variance = (actual_impact.cost_change_usd.abs()
                            - decision
                                .expected_impact
                                .cost_change_usd
                                .unwrap_or(0.0)
                                .abs())
                        .abs();
                        performance.prediction_accuracy =
                            1.0 - (variance / decision.expected_impact.cost_change_usd.unwrap_or(1.0).abs()).min(1.0);
                    }
                }
            }
        } else {
            self.stats.failed_batching_decisions += 1;

            // Update failure tracking for pattern
            if let Some(pattern_id) = decision.metadata.get("pattern_id") {
                if let Some(pattern_id_str) = pattern_id.as_str() {
                    let performance = self
                        .learning_data
                        .pattern_performance
                        .entry(pattern_id_str.to_string())
                        .or_insert_with(PatternPerformance::default);

                    performance.total_executions += 1;
                    performance.success_rate =
                        performance.successful_executions as f64 / performance.total_executions as f64;
                }
            }
        }

        // Record execution
        if let Some(actual_impact) = &outcome.actual_impact {
            self.batch_history.push_back(BatchExecution {
                timestamp: outcome.execution_time,
                batch_size: decision
                    .metadata
                    .get("batch_size")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1) as usize,
                cost_savings: actual_impact.cost_change_usd.abs(),
                latency_increase: actual_impact.latency_change_ms,
                quality_impact: Some(actual_impact.quality_change),
                success: outcome.success,
            });

            // Keep history bounded
            if self.batch_history.len() > 100 {
                self.batch_history.pop_front();
            }
        }

        info!(
            "Updated batching learning data: {} total decisions, {:.1}% success rate",
            self.stats.total_batching_decisions,
            (self.stats.successful_batching_decisions as f64 / self.stats.total_batching_decisions as f64) * 100.0
        );
    }
}

#[async_trait]
impl OptimizationStrategy for BatchingStrategy {
    fn name(&self) -> &str {
        "batching"
    }

    fn priority(&self) -> u32 {
        self.config.priority
    }

    fn is_applicable(&self, input: &DecisionInput) -> bool {
        // Batching is applicable if:
        // 1. We have sufficient request volume
        let has_volume = input.current_metrics.throughput_rps > 1.0;

        // 2. We have insights or recommendations suggesting batching
        let has_batching_insights = input.insights.iter().any(|i| {
            i.title.to_lowercase().contains("batch")
                || i.title.to_lowercase().contains("similar")
                || i.title.to_lowercase().contains("group")
        });

        let has_batching_recommendations = input.recommendations.iter().any(|r| {
            r.action.action_type.to_lowercase().contains("batch")
                || r.title.to_lowercase().contains("batch")
        });

        // 3. Current metrics suggest opportunity for optimization
        let has_cost_pressure = input.current_metrics.total_cost_usd > 10.0
            || input.current_metrics.daily_cost_usd > 1.0;

        has_volume && (has_batching_insights || has_batching_recommendations || has_cost_pressure)
    }

    async fn evaluate(
        &self,
        input: &DecisionInput,
        criteria: &DecisionCriteria,
    ) -> DecisionResult<Vec<Decision>> {
        debug!("Evaluating batching strategy");

        // Detect request patterns
        let patterns = {
            let mut strategy = self.clone();
            strategy.detect_request_patterns(input)
        };

        if patterns.is_empty() {
            debug!("No batching patterns detected");
            return Ok(vec![]);
        }

        let mut decisions = Vec::new();

        for pattern in patterns {
            // Calculate optimal batch size
            let optimal = self.calculate_optimal_batch_size(&pattern, criteria);

            // Analyze tradeoffs
            let tradeoff = self.analyze_tradeoffs(&optimal, criteria);

            // Skip if tradeoff is not acceptable
            if !tradeoff.is_acceptable {
                debug!(
                    "Skipping pattern {} due to unacceptable tradeoff: {}",
                    pattern.pattern_id, tradeoff.reasoning
                );
                continue;
            }

            // Perform safety checks
            let safety_checks = self.perform_safety_checks(&optimal, criteria);

            // Verify all required safety checks passed
            let all_checks_passed = safety_checks.iter().all(|check| check.passed);

            if !all_checks_passed {
                debug!(
                    "Skipping pattern {} due to failed safety checks",
                    pattern.pattern_id
                );
                continue;
            }

            // Generate decision
            let decision = self.generate_decision(&pattern, &optimal, &tradeoff, safety_checks);

            info!(
                "Generated batching decision for pattern '{}': batch_size={}, cost_savings={:.1}%, latency_increase={:.0}ms",
                pattern.request_type,
                optimal.batch_size,
                optimal.expected_cost_savings_pct,
                optimal.expected_latency_increase_ms
            );

            decisions.push(decision);
        }

        Ok(decisions)
    }

    async fn validate(
        &self,
        decision: &Decision,
        criteria: &DecisionCriteria,
    ) -> DecisionResult<()> {
        // Verify this is a batching decision
        if decision.decision_type != DecisionType::EnableBatching {
            return Err(DecisionError::ValidationFailed(
                "Not a batching decision".to_string(),
            ));
        }

        // Verify confidence meets threshold
        if decision.confidence < criteria.min_confidence {
            return Err(DecisionError::ValidationFailed(format!(
                "Confidence {:.2} below minimum {:.2}",
                decision.confidence, criteria.min_confidence
            )));
        }

        // Verify latency increase is acceptable
        if let Some(latency_change) = decision.expected_impact.latency_change_ms {
            let max_latency = criteria
                .max_latency_increase_ms
                .unwrap_or(self.config.max_latency_increase_ms);

            if latency_change > max_latency {
                return Err(DecisionError::ConstraintViolation(format!(
                    "Latency increase {:.0}ms exceeds maximum {:.0}ms",
                    latency_change, max_latency
                )));
            }
        }

        // Verify all required safety checks passed
        for required_check in &criteria.required_safety_checks {
            let check_passed = decision
                .safety_checks
                .iter()
                .any(|check| check.name == *required_check && check.passed);

            if !check_passed {
                return Err(DecisionError::ValidationFailed(format!(
                    "Required safety check '{}' did not pass",
                    required_check
                )));
            }
        }

        Ok(())
    }

    async fn learn(&mut self, decision: &Decision, outcome: &DecisionOutcome) -> DecisionResult<()> {
        debug!(
            "Learning from batching decision outcome: success={}",
            outcome.success
        );

        self.update_learning(decision, outcome);

        Ok(())
    }

    fn get_stats(&self) -> serde_json::Value {
        serde_json::json!({
            "total_decisions": self.stats.total_batching_decisions,
            "successful_decisions": self.stats.successful_batching_decisions,
            "failed_decisions": self.stats.failed_batching_decisions,
            "success_rate": if self.stats.total_batching_decisions > 0 {
                (self.stats.successful_batching_decisions as f64 / self.stats.total_batching_decisions as f64) * 100.0
            } else {
                0.0
            },
            "patterns_tracked": self.request_patterns.len(),
            "batch_history_size": self.batch_history.len(),
            "pattern_performance": self.learning_data.pattern_performance.iter().map(|(k, v)| {
                (k.clone(), serde_json::json!({
                    "executions": v.total_executions,
                    "success_rate": v.success_rate,
                    "avg_cost_savings": v.avg_cost_savings,
                    "avg_latency_increase": v.avg_latency_increase,
                    "prediction_accuracy": v.prediction_accuracy,
                }))
            }).collect::<HashMap<_, _>>(),
        })
    }
}

// Make BatchingStrategy cloneable for internal use
impl Clone for BatchingStrategy {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            stats: self.stats.clone(),
            request_patterns: self.request_patterns.clone(),
            batch_history: self.batch_history.clone(),
            similarity_cache: self.similarity_cache.clone(),
            learning_data: self.learning_data.clone(),
        }
    }
}

/// Request pattern for batching
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RequestPattern {
    pattern_id: String,
    request_type: String,
    frequency_per_minute: f64,
    avg_processing_time_ms: f64,
    similarity_score: f64,
    last_seen: DateTime<Utc>,
}

/// Optimal batch size calculation result
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OptimalBatchSize {
    batch_size: usize,
    expected_cost_savings_pct: f64,
    expected_latency_increase_ms: f64,
    confidence: f64,
}

/// Tradeoff analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TradeoffAnalysis {
    tradeoff_score: f64,
    cost_benefit: f64,
    latency_penalty: f64,
    is_acceptable: bool,
    reasoning: String,
}

/// Statistics for batching strategy
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct BatchingStats {
    total_batching_decisions: u64,
    successful_batching_decisions: u64,
    failed_batching_decisions: u64,
}

/// Historical batch execution
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BatchExecution {
    timestamp: DateTime<Utc>,
    batch_size: usize,
    cost_savings: f64,
    latency_increase: f64,
    quality_impact: Option<f64>,
    success: bool,
}

/// Learning data for pattern optimization
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct LearningData {
    pattern_performance: HashMap<String, PatternPerformance>,
}

/// Performance tracking for a specific pattern
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PatternPerformance {
    total_executions: u64,
    successful_executions: u64,
    success_rate: f64,
    total_cost_savings: f64,
    total_latency_increase: f64,
    avg_cost_savings: f64,
    avg_latency_increase: f64,
    prediction_accuracy: f64,
}

/// Extract request type from text
fn extract_request_type(text: &str) -> String {
    let text_lower = text.to_lowercase();

    if text_lower.contains("query") || text_lower.contains("search") {
        "query".to_string()
    } else if text_lower.contains("completion") || text_lower.contains("generate") {
        "completion".to_string()
    } else if text_lower.contains("summary") || text_lower.contains("summarize") {
        "summary".to_string()
    } else if text_lower.contains("translation") || text_lower.contains("translate") {
        "translation".to_string()
    } else if text_lower.contains("classification") || text_lower.contains("classify") {
        "classification".to_string()
    } else {
        "general".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::{Confidence, Insight, InsightCategory, Recommendation, Severity};
    use crate::decision::types::*;

    fn create_test_input() -> DecisionInput {
        DecisionInput {
            timestamp: Utc::now(),
            analysis_reports: vec![],
            insights: vec![create_test_insight()],
            recommendations: vec![create_test_recommendation()],
            current_metrics: create_test_metrics(),
            context: HashMap::new(),
        }
    }

    fn create_test_insight() -> Insight {
        Insight {
            id: "insight-1".to_string(),
            analyzer: "pattern".to_string(),
            timestamp: Utc::now(),
            severity: Severity::Info,
            confidence: Confidence::new(0.85),
            title: "Similar query requests detected".to_string(),
            description: "Multiple similar queries could be batched".to_string(),
            category: InsightCategory::Performance,
            impact: None,
            evidence: vec![],
            metadata: HashMap::new(),
        }
    }

    fn create_test_recommendation() -> Recommendation {
        Recommendation {
            id: "rec-1".to_string(),
            analyzer: "cost".to_string(),
            timestamp: Utc::now(),
            priority: crate::analyzer::Priority::High,
            confidence: Confidence::new(0.8),
            title: "Enable request batching".to_string(),
            description: "Batch similar requests to reduce costs".to_string(),
            action: crate::analyzer::Action {
                action_type: "enable_batching".to_string(),
                parameters: HashMap::new(),
                estimated_effort: None,
            },
            expected_impact: None,
            risk_level: crate::analyzer::RiskLevel::Low,
            metadata: HashMap::new(),
        }
    }

    fn create_test_metrics() -> SystemMetrics {
        SystemMetrics {
            avg_latency_ms: 150.0,
            p95_latency_ms: 300.0,
            p99_latency_ms: 500.0,
            success_rate_pct: 99.0,
            error_rate_pct: 1.0,
            throughput_rps: 10.0,
            total_cost_usd: 50.0,
            daily_cost_usd: 5.0,
            avg_rating: Some(4.5),
            cache_hit_rate_pct: Some(30.0),
            request_count: 1000,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_strategy_creation() {
        let config = BatchingConfig::default();
        let strategy = BatchingStrategy::new(config);

        assert_eq!(strategy.name(), "batching");
        assert_eq!(strategy.priority(), 70);
    }

    #[test]
    fn test_is_applicable_with_volume() {
        let strategy = BatchingStrategy::with_defaults();
        let input = create_test_input();

        assert!(strategy.is_applicable(&input));
    }

    #[test]
    fn test_is_not_applicable_without_volume() {
        let strategy = BatchingStrategy::with_defaults();
        let mut input = create_test_input();
        input.current_metrics.throughput_rps = 0.1; // Very low
        input.insights.clear(); // No insights
        input.recommendations.clear(); // No recommendations
        input.current_metrics.total_cost_usd = 0.5; // Low cost

        assert!(!strategy.is_applicable(&input));
    }

    #[test]
    fn test_detect_request_patterns() {
        let mut strategy = BatchingStrategy::with_defaults();
        let input = create_test_input();

        let patterns = strategy.detect_request_patterns(&input);

        assert!(!patterns.is_empty());
        assert!(patterns.iter().any(|p| p.request_type == "query"));
    }

    #[test]
    fn test_calculate_optimal_batch_size() {
        let strategy = BatchingStrategy::with_defaults();
        let pattern = RequestPattern {
            pattern_id: "test-pattern".to_string(),
            request_type: "query".to_string(),
            frequency_per_minute: 60.0,
            avg_processing_time_ms: 100.0,
            similarity_score: 0.85,
            last_seen: Utc::now(),
        };
        let criteria = DecisionCriteria::default();

        let optimal = strategy.calculate_optimal_batch_size(&pattern, &criteria);

        assert!(optimal.batch_size >= 1);
        assert!(optimal.batch_size <= strategy.config.max_batch_size);
        assert!(optimal.expected_cost_savings_pct > 0.0);
        assert!(optimal.confidence > 0.0 && optimal.confidence <= 1.0);
    }

    #[test]
    fn test_estimate_cost_savings() {
        let strategy = BatchingStrategy::with_defaults();

        let savings_2 = strategy.estimate_cost_savings(2);
        let savings_5 = strategy.estimate_cost_savings(5);
        let savings_10 = strategy.estimate_cost_savings(10);

        // Savings should increase with batch size
        assert!(savings_2 < savings_5);
        assert!(savings_5 < savings_10);

        // But with diminishing returns
        let increase_2_to_5 = savings_5 - savings_2;
        let increase_5_to_10 = savings_10 - savings_5;
        assert!(increase_5_to_10 < increase_2_to_5);
    }

    #[test]
    fn test_estimate_latency_increase() {
        let strategy = BatchingStrategy::with_defaults();
        let pattern = RequestPattern {
            pattern_id: "test".to_string(),
            request_type: "query".to_string(),
            frequency_per_minute: 60.0,
            avg_processing_time_ms: 100.0,
            similarity_score: 0.85,
            last_seen: Utc::now(),
        };

        let latency = strategy.estimate_latency_increase(5, &pattern);

        assert!(latency >= 0.0);
        assert!(latency <= strategy.config.batch_timeout.as_millis() as f64);
    }

    #[test]
    fn test_analyze_tradeoffs_acceptable() {
        let strategy = BatchingStrategy::with_defaults();
        let optimal = OptimalBatchSize {
            batch_size: 5,
            expected_cost_savings_pct: 25.0,
            expected_latency_increase_ms: 50.0,
            confidence: 0.85,
        };
        let criteria = DecisionCriteria::default();

        let tradeoff = strategy.analyze_tradeoffs(&optimal, &criteria);

        assert!(tradeoff.is_acceptable);
        assert!(tradeoff.tradeoff_score > 0.0);
    }

    #[test]
    fn test_analyze_tradeoffs_unacceptable_latency() {
        let strategy = BatchingStrategy::with_defaults();
        let optimal = OptimalBatchSize {
            batch_size: 10,
            expected_cost_savings_pct: 20.0,
            expected_latency_increase_ms: 600.0, // Too high
            confidence: 0.85,
        };
        let criteria = DecisionCriteria::default();

        let tradeoff = strategy.analyze_tradeoffs(&optimal, &criteria);

        assert!(!tradeoff.is_acceptable);
    }

    #[test]
    fn test_perform_safety_checks() {
        let strategy = BatchingStrategy::with_defaults();
        let optimal = OptimalBatchSize {
            batch_size: 5,
            expected_cost_savings_pct: 20.0,
            expected_latency_increase_ms: 100.0,
            confidence: 0.85,
        };
        let criteria = DecisionCriteria::default();

        let checks = strategy.perform_safety_checks(&optimal, &criteria);

        assert!(!checks.is_empty());
        assert!(checks.iter().any(|c| c.name == "latency_check"));
        assert!(checks.iter().any(|c| c.name == "cost_check"));
        assert!(checks.iter().any(|c| c.name == "confidence_check"));
    }

    #[tokio::test]
    async fn test_evaluate_generates_decisions() {
        let strategy = BatchingStrategy::with_defaults();
        let input = create_test_input();
        let criteria = DecisionCriteria::default();

        let decisions = strategy.evaluate(&input, &criteria).await.unwrap();

        // Should generate at least one decision given our test input
        assert!(!decisions.is_empty());

        let decision = &decisions[0];
        assert_eq!(decision.decision_type, DecisionType::EnableBatching);
        assert_eq!(decision.strategy, "batching");
        assert!(!decision.config_changes.is_empty());
    }

    #[tokio::test]
    async fn test_validate_valid_decision() {
        let strategy = BatchingStrategy::with_defaults();
        let criteria = DecisionCriteria::default();

        let decision = Decision {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            strategy: "batching".to_string(),
            decision_type: DecisionType::EnableBatching,
            confidence: 0.85,
            expected_impact: ExpectedImpact {
                cost_change_usd: Some(-5.0),
                latency_change_ms: Some(100.0),
                quality_change: None,
                throughput_change_rps: None,
                success_rate_change_pct: None,
                time_to_impact: Duration::from_secs(300),
                confidence: 0.85,
            },
            config_changes: vec![],
            justification: "Test".to_string(),
            related_insights: vec![],
            related_recommendations: vec![],
            priority: 70,
            requires_approval: false,
            safety_checks: vec![
                SafetyCheck {
                    name: "cost_check".to_string(),
                    passed: true,
                    details: "OK".to_string(),
                    timestamp: Utc::now(),
                },
                SafetyCheck {
                    name: "latency_check".to_string(),
                    passed: true,
                    details: "OK".to_string(),
                    timestamp: Utc::now(),
                },
                SafetyCheck {
                    name: "quality_check".to_string(),
                    passed: true,
                    details: "OK".to_string(),
                    timestamp: Utc::now(),
                },
            ],
            rollback_plan: None,
            metadata: HashMap::new(),
        };

        assert!(strategy.validate(&decision, &criteria).await.is_ok());
    }

    #[tokio::test]
    async fn test_validate_wrong_decision_type() {
        let strategy = BatchingStrategy::with_defaults();
        let criteria = DecisionCriteria::default();

        let decision = Decision {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            strategy: "batching".to_string(),
            decision_type: DecisionType::ModelSwitch, // Wrong type
            confidence: 0.85,
            expected_impact: ExpectedImpact {
                cost_change_usd: Some(-5.0),
                latency_change_ms: Some(100.0),
                quality_change: None,
                throughput_change_rps: None,
                success_rate_change_pct: None,
                time_to_impact: Duration::from_secs(300),
                confidence: 0.85,
            },
            config_changes: vec![],
            justification: "Test".to_string(),
            related_insights: vec![],
            related_recommendations: vec![],
            priority: 70,
            requires_approval: false,
            safety_checks: vec![],
            rollback_plan: None,
            metadata: HashMap::new(),
        };

        assert!(strategy.validate(&decision, &criteria).await.is_err());
    }

    #[tokio::test]
    async fn test_learn_from_outcome() {
        let mut strategy = BatchingStrategy::with_defaults();

        let decision = Decision {
            id: "test-decision".to_string(),
            timestamp: Utc::now(),
            strategy: "batching".to_string(),
            decision_type: DecisionType::EnableBatching,
            confidence: 0.85,
            expected_impact: ExpectedImpact {
                cost_change_usd: Some(-5.0),
                latency_change_ms: Some(100.0),
                quality_change: None,
                throughput_change_rps: None,
                success_rate_change_pct: None,
                time_to_impact: Duration::from_secs(300),
                confidence: 0.85,
            },
            config_changes: vec![],
            justification: "Test".to_string(),
            related_insights: vec![],
            related_recommendations: vec![],
            priority: 70,
            requires_approval: false,
            safety_checks: vec![],
            rollback_plan: None,
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("pattern_id".to_string(), serde_json::json!("test-pattern"));
                meta.insert("batch_size".to_string(), serde_json::json!(5));
                meta
            },
        };

        let outcome = DecisionOutcome {
            decision_id: "test-decision".to_string(),
            execution_time: Utc::now(),
            success: true,
            error: None,
            actual_impact: Some(ActualImpact {
                cost_change_usd: -6.0,
                latency_change_ms: 95.0,
                quality_change: 0.0,
                throughput_change_rps: 0.0,
                success_rate_change_pct: 0.0,
                time_to_impact: Duration::from_secs(300),
                variance_from_expected: 0.2,
            }),
            rolled_back: false,
            rollback_reason: None,
            metrics_before: create_test_metrics(),
            metrics_after: Some(create_test_metrics()),
            duration: Duration::from_secs(600),
        };

        strategy.learn(&decision, &outcome).await.unwrap();

        assert_eq!(strategy.stats.total_batching_decisions, 1);
        assert_eq!(strategy.stats.successful_batching_decisions, 1);
        assert!(!strategy.batch_history.is_empty());
    }

    #[test]
    fn test_extract_request_type() {
        assert_eq!(extract_request_type("query requests"), "query");
        assert_eq!(extract_request_type("completion tasks"), "completion");
        assert_eq!(extract_request_type("summary generation"), "summary");
        assert_eq!(extract_request_type("translation requests"), "translation");
        assert_eq!(extract_request_type("classification jobs"), "classification");
        assert_eq!(extract_request_type("other requests"), "general");
    }

    #[test]
    fn test_get_stats() {
        let strategy = BatchingStrategy::with_defaults();
        let stats = strategy.get_stats();

        assert!(stats.is_object());
        assert!(stats.get("total_decisions").is_some());
        assert!(stats.get("successful_decisions").is_some());
        assert!(stats.get("failed_decisions").is_some());
        assert!(stats.get("success_rate").is_some());
    }

    #[test]
    fn test_calculate_decision_priority() {
        let strategy = BatchingStrategy::with_defaults();

        let optimal_high_savings = OptimalBatchSize {
            batch_size: 5,
            expected_cost_savings_pct: 30.0,
            expected_latency_increase_ms: 50.0,
            confidence: 0.9,
        };

        let tradeoff = TradeoffAnalysis {
            tradeoff_score: 0.6,
            cost_benefit: 0.3,
            latency_penalty: 0.1,
            is_acceptable: true,
            reasoning: "Good tradeoff".to_string(),
        };

        let priority = strategy.calculate_decision_priority(&optimal_high_savings, &tradeoff);

        // Should be higher than base priority due to high savings and confidence
        assert!(priority > strategy.config.priority);
    }
}
