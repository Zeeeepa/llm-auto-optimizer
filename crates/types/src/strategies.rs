//! Optimization strategy configuration and parameters

use serde::{Deserialize, Serialize};

/// Threshold configuration for heuristic-based optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thresholds {
    /// Performance thresholds
    pub latency_p95_ms: f64,
    pub latency_increase_pct: f64,
    pub error_rate_pct: f64,

    /// Cost thresholds
    pub cost_per_request: f64,
    pub daily_budget: f64,

    /// Quality thresholds
    pub quality_score_min: f64,
    pub quality_drop_pct: f64,

    /// Drift thresholds
    pub psi_threshold: f64,
    pub ks_test_p_value: f64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            latency_p95_ms: 5000.0,
            latency_increase_pct: 20.0,
            error_rate_pct: 5.0,
            cost_per_request: 0.10,
            daily_budget: 1000.0,
            quality_score_min: 0.7,
            quality_drop_pct: 10.0,
            psi_threshold: 0.1,
            ks_test_p_value: 0.05,
        }
    }
}

/// A/B testing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABTestConfig {
    /// Minimum sample size per variant
    pub min_sample_size: usize,
    /// Statistical significance level (e.g., 0.05 for p < 0.05)
    pub significance_level: f64,
    /// Maximum test duration in seconds
    pub max_duration_seconds: u64,
    /// Traffic allocation strategy
    pub allocation_strategy: TrafficAllocationStrategy,
}

impl Default for ABTestConfig {
    fn default() -> Self {
        Self {
            min_sample_size: 1000,
            significance_level: 0.05,
            max_duration_seconds: 7 * 24 * 3600, // 7 days
            allocation_strategy: TrafficAllocationStrategy::ThompsonSampling,
        }
    }
}

/// Traffic allocation strategy for A/B tests
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrafficAllocationStrategy {
    /// Fixed allocation percentages
    Fixed,
    /// Thompson Sampling (adaptive)
    ThompsonSampling,
    /// Epsilon-greedy
    EpsilonGreedy,
}

/// Reinforcement learning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RLConfig {
    /// Learning rate
    pub learning_rate: f64,
    /// Exploration parameter (epsilon for epsilon-greedy)
    pub exploration_rate: f64,
    /// Discount factor (gamma)
    pub discount_factor: f64,
    /// Reward signal weights
    pub reward_weights: RewardWeights,
}

impl Default for RLConfig {
    fn default() -> Self {
        Self {
            learning_rate: 0.1,
            exploration_rate: 0.1,
            discount_factor: 0.95,
            reward_weights: RewardWeights::default(),
        }
    }
}

/// Weights for different components of the reward signal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardWeights {
    /// Weight for quality component
    pub quality: f64,
    /// Weight for cost component
    pub cost: f64,
    /// Weight for latency component
    pub latency: f64,
    /// Weight for user feedback
    pub feedback: f64,
}

impl Default for RewardWeights {
    fn default() -> Self {
        Self {
            quality: 0.6,
            cost: 0.3,
            latency: 0.1,
            feedback: 0.0, // Optional
        }
    }
}

/// Cost-performance optimization mode
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationMode {
    /// Prioritize cost reduction
    CostOptimized,
    /// Balance cost and quality
    Balanced,
    /// Prioritize quality
    QualityOptimized,
    /// Custom weights
    Custom,
}

/// Cost-performance optimization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostPerformanceConfig {
    /// Optimization mode
    pub mode: OptimizationMode,
    /// Quality weight (if mode is Custom)
    pub quality_weight: f64,
    /// Cost weight (if mode is Custom)
    pub cost_weight: f64,
    /// Latency weight (if mode is Custom)
    pub latency_weight: f64,
}

impl Default for CostPerformanceConfig {
    fn default() -> Self {
        Self {
            mode: OptimizationMode::Balanced,
            quality_weight: 0.5,
            cost_weight: 0.3,
            latency_weight: 0.2,
        }
    }
}

impl CostPerformanceConfig {
    /// Get effective weights based on mode
    pub fn get_weights(&self) -> (f64, f64, f64) {
        match self.mode {
            OptimizationMode::CostOptimized => (0.2, 0.7, 0.1),
            OptimizationMode::Balanced => (0.5, 0.3, 0.2),
            OptimizationMode::QualityOptimized => (0.8, 0.1, 0.1),
            OptimizationMode::Custom => (self.quality_weight, self.cost_weight, self.latency_weight),
        }
    }
}

/// Complete strategy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    /// Threshold-based heuristics configuration
    pub thresholds: Thresholds,
    /// A/B testing configuration
    pub ab_testing: ABTestConfig,
    /// Reinforcement learning configuration
    pub rl: RLConfig,
    /// Cost-performance optimization configuration
    pub cost_performance: CostPerformanceConfig,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            thresholds: Thresholds::default(),
            ab_testing: ABTestConfig::default(),
            rl: RLConfig::default(),
            cost_performance: CostPerformanceConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_thresholds() {
        let thresholds = Thresholds::default();
        assert_eq!(thresholds.latency_p95_ms, 5000.0);
        assert_eq!(thresholds.quality_score_min, 0.7);
    }

    #[test]
    fn test_cost_performance_weights() {
        let config = CostPerformanceConfig {
            mode: OptimizationMode::CostOptimized,
            ..Default::default()
        };
        let (quality, cost, latency) = config.get_weights();
        assert_eq!(quality, 0.2);
        assert_eq!(cost, 0.7);
        assert_eq!(latency, 0.1);

        let custom = CostPerformanceConfig {
            mode: OptimizationMode::Custom,
            quality_weight: 0.6,
            cost_weight: 0.3,
            latency_weight: 0.1,
        };
        let (q, c, l) = custom.get_weights();
        assert_eq!(q, 0.6);
        assert_eq!(c, 0.3);
        assert_eq!(l, 0.1);
    }
}
