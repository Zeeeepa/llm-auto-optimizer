//! Decision Engine configuration

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for the Decision Engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionEngineConfig {
    /// Base configuration
    pub id: String,

    /// Whether the engine is enabled
    pub enabled: bool,

    /// Decision interval (how often to make decisions)
    pub decision_interval: Duration,

    /// Minimum time between decisions for the same issue
    pub min_decision_gap: Duration,

    /// Maximum concurrent decisions
    pub max_concurrent_decisions: usize,

    /// Decision timeout
    pub decision_timeout: Duration,

    /// Whether to enable automatic execution
    pub auto_execute: bool,

    /// Whether to require approval for decisions
    pub require_approval: bool,

    /// Strategy configurations
    pub strategies: StrategyConfigs,

    /// Safety configuration
    pub safety: SafetyConfig,

    /// Monitoring configuration
    pub monitoring: MonitoringConfig,
}

impl Default for DecisionEngineConfig {
    fn default() -> Self {
        Self {
            id: "decision-engine".to_string(),
            enabled: true,
            decision_interval: Duration::from_secs(60),
            min_decision_gap: Duration::from_secs(300),
            max_concurrent_decisions: 3,
            decision_timeout: Duration::from_secs(30),
            auto_execute: false,
            require_approval: true,
            strategies: StrategyConfigs::default(),
            safety: SafetyConfig::default(),
            monitoring: MonitoringConfig::default(),
        }
    }
}

/// Configuration for all strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfigs {
    /// Model selection strategy configuration
    pub model_selection: ModelSelectionConfig,

    /// Caching strategy configuration
    pub caching: CachingConfig,

    /// Rate limiting strategy configuration
    pub rate_limiting: RateLimitingConfig,

    /// Batching strategy configuration
    pub batching: BatchingConfig,

    /// Prompt optimization strategy configuration
    pub prompt_optimization: PromptOptimizationConfig,
}

impl Default for StrategyConfigs {
    fn default() -> Self {
        Self {
            model_selection: ModelSelectionConfig::default(),
            caching: CachingConfig::default(),
            rate_limiting: RateLimitingConfig::default(),
            batching: BatchingConfig::default(),
            prompt_optimization: PromptOptimizationConfig::default(),
        }
    }
}

/// Model selection strategy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSelectionConfig {
    /// Whether this strategy is enabled
    pub enabled: bool,

    /// Priority (higher = more important)
    pub priority: u32,

    /// Minimum confidence required
    pub min_confidence: f64,

    /// Cost weight in decision (0-1)
    pub cost_weight: f64,

    /// Performance weight in decision (0-1)
    pub performance_weight: f64,

    /// Quality weight in decision (0-1)
    pub quality_weight: f64,

    /// Available models to choose from
    pub available_models: Vec<String>,

    /// Minimum evaluation period before switching models
    pub min_evaluation_period: Duration,

    /// Maximum cost increase allowed for quality improvement (%)
    pub max_cost_increase_pct: f64,
}

impl Default for ModelSelectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            priority: 100,
            min_confidence: 0.7,
            cost_weight: 0.4,
            performance_weight: 0.3,
            quality_weight: 0.3,
            available_models: vec![
                "gpt-4".to_string(),
                "gpt-3.5-turbo".to_string(),
                "claude-3-opus".to_string(),
                "claude-3-sonnet".to_string(),
            ],
            min_evaluation_period: Duration::from_secs(3600), // 1 hour
            max_cost_increase_pct: 50.0,
        }
    }
}

/// Caching strategy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachingConfig {
    /// Whether this strategy is enabled
    pub enabled: bool,

    /// Priority
    pub priority: u32,

    /// Minimum confidence required
    pub min_confidence: f64,

    /// Minimum request frequency to enable caching (req/min)
    pub min_request_frequency: f64,

    /// Minimum similarity threshold for cache hits (0-1)
    pub min_similarity_threshold: f64,

    /// Cache TTL
    pub cache_ttl: Duration,

    /// Maximum cache size (number of entries)
    pub max_cache_size: usize,

    /// Minimum expected hit rate to enable caching (%)
    pub min_expected_hit_rate_pct: f64,
}

impl Default for CachingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            priority: 90,
            min_confidence: 0.7,
            min_request_frequency: 10.0,
            min_similarity_threshold: 0.85,
            cache_ttl: Duration::from_secs(3600),
            max_cache_size: 10000,
            min_expected_hit_rate_pct: 20.0,
        }
    }
}

/// Rate limiting strategy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitingConfig {
    /// Whether this strategy is enabled
    pub enabled: bool,

    /// Priority
    pub priority: u32,

    /// Minimum confidence required
    pub min_confidence: f64,

    /// Maximum requests per second
    pub max_requests_per_second: f64,

    /// Maximum tokens per minute
    pub max_tokens_per_minute: u64,

    /// Maximum cost per hour (USD)
    pub max_cost_per_hour: f64,

    /// Burst multiplier (allows temporary bursts)
    pub burst_multiplier: f64,

    /// Adaptive rate limiting enabled
    pub adaptive_enabled: bool,

    /// Target error rate for adaptive limiting (%)
    pub target_error_rate_pct: f64,
}

impl Default for RateLimitingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            priority: 80,
            min_confidence: 0.7,
            max_requests_per_second: 100.0,
            max_tokens_per_minute: 100000,
            max_cost_per_hour: 10.0,
            burst_multiplier: 1.5,
            adaptive_enabled: true,
            target_error_rate_pct: 1.0,
        }
    }
}

/// Batching strategy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchingConfig {
    /// Whether this strategy is enabled
    pub enabled: bool,

    /// Priority
    pub priority: u32,

    /// Minimum confidence required
    pub min_confidence: f64,

    /// Maximum batch size
    pub max_batch_size: usize,

    /// Batch timeout (max wait time)
    pub batch_timeout: Duration,

    /// Minimum requests to enable batching
    pub min_requests_for_batching: usize,

    /// Maximum latency increase allowed (ms)
    pub max_latency_increase_ms: f64,

    /// Minimum cost savings to enable batching (%)
    pub min_cost_savings_pct: f64,
}

impl Default for BatchingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            priority: 70,
            min_confidence: 0.7,
            max_batch_size: 10,
            batch_timeout: Duration::from_millis(100),
            min_requests_for_batching: 5,
            max_latency_increase_ms: 200.0,
            min_cost_savings_pct: 15.0,
        }
    }
}

/// Prompt optimization strategy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptOptimizationConfig {
    /// Whether this strategy is enabled
    pub enabled: bool,

    /// Priority
    pub priority: u32,

    /// Minimum confidence required
    pub min_confidence: f64,

    /// Minimum token reduction to apply optimization (%)
    pub min_token_reduction_pct: f64,

    /// A/B test duration
    pub ab_test_duration: Duration,

    /// A/B test traffic split (0-1)
    pub ab_test_traffic_split: f64,

    /// Minimum quality score for optimized prompts
    pub min_quality_score: f64,

    /// Maximum quality degradation allowed (%)
    pub max_quality_degradation_pct: f64,
}

impl Default for PromptOptimizationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            priority: 85,
            min_confidence: 0.7,
            min_token_reduction_pct: 10.0,
            ab_test_duration: Duration::from_secs(3600),
            ab_test_traffic_split: 0.1,
            min_quality_score: 4.0,
            max_quality_degradation_pct: 5.0,
        }
    }
}

/// Safety configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyConfig {
    /// Maximum cost increase allowed (USD/day)
    pub max_cost_increase_usd: f64,

    /// Maximum latency increase allowed (ms)
    pub max_latency_increase_ms: f64,

    /// Minimum quality score (0-5)
    pub min_quality_score: f64,

    /// Minimum success rate (%)
    pub min_success_rate_pct: f64,

    /// Enable circuit breaker
    pub circuit_breaker_enabled: bool,

    /// Circuit breaker error threshold (%)
    pub circuit_breaker_error_threshold_pct: f64,

    /// Circuit breaker timeout
    pub circuit_breaker_timeout: Duration,

    /// Enable automatic rollback
    pub auto_rollback_enabled: bool,

    /// Rollback timeout (max time before auto-rollback if metrics don't improve)
    pub rollback_timeout: Duration,
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            max_cost_increase_usd: 100.0,
            max_latency_increase_ms: 500.0,
            min_quality_score: 3.5,
            min_success_rate_pct: 95.0,
            circuit_breaker_enabled: true,
            circuit_breaker_error_threshold_pct: 10.0,
            circuit_breaker_timeout: Duration::from_secs(300),
            auto_rollback_enabled: true,
            rollback_timeout: Duration::from_secs(600),
        }
    }
}

/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable detailed logging
    pub enable_logging: bool,

    /// Enable metrics collection
    pub enable_metrics: bool,

    /// Enable tracing
    pub enable_tracing: bool,

    /// Metrics export interval
    pub metrics_interval: Duration,

    /// Decision history size (number of decisions to keep)
    pub decision_history_size: usize,

    /// Outcome history size (number of outcomes to keep)
    pub outcome_history_size: usize,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enable_logging: true,
            enable_metrics: true,
            enable_tracing: true,
            metrics_interval: Duration::from_secs(60),
            decision_history_size: 1000,
            outcome_history_size: 1000,
        }
    }
}
