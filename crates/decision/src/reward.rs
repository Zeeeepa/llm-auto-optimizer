//! Reward signal calculation for reinforcement learning
//!
//! This module calculates reward signals from multiple sources including
//! user feedback, quality metrics, cost, and latency.

use serde::{Deserialize, Serialize};

/// User feedback signal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserFeedback {
    /// Explicit rating (1-5 stars, or thumbs up/down)
    pub explicit_rating: Option<f64>,
    /// Task was completed successfully
    pub task_completed: bool,
    /// User retried the request
    pub retry_occurred: bool,
    /// User edited the response
    pub response_edited: bool,
    /// Time spent viewing response (seconds)
    pub dwell_time_seconds: Option<f64>,
    /// Response was copied/saved
    pub response_saved: bool,
    /// Response was shared
    pub response_shared: bool,
}

impl UserFeedback {
    /// Create new user feedback with defaults
    pub fn new() -> Self {
        Self {
            explicit_rating: None,
            task_completed: false,
            retry_occurred: false,
            response_edited: false,
            dwell_time_seconds: None,
            response_saved: false,
            response_shared: false,
        }
    }

    /// Infer implicit quality score from user behavior
    pub fn infer_quality(&self) -> f64 {
        let mut score: f64 = 0.5; // Start neutral

        // Task completion is strong positive signal
        if self.task_completed {
            score += 0.3;
        }

        // Retry is strong negative signal
        if self.retry_occurred {
            score -= 0.4;
        }

        // Edit suggests partial success
        if self.response_edited {
            score -= 0.1;
        }

        // Dwell time (longer = better, up to a point)
        if let Some(dwell) = self.dwell_time_seconds {
            if dwell > 10.0 && dwell < 120.0 {
                score += 0.2;
            } else if dwell < 3.0 {
                score -= 0.2;
            }
        }

        // Saving/sharing is positive
        if self.response_saved || self.response_shared {
            score += 0.2;
        }

        score.clamp(0.0, 1.0)
    }
}

impl Default for UserFeedback {
    fn default() -> Self {
        Self::new()
    }
}

/// LLM response metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMetrics {
    /// Quality score (0-1)
    pub quality_score: f64,
    /// Cost in dollars
    pub cost: f64,
    /// Latency in milliseconds
    pub latency_ms: f64,
    /// Token count
    pub token_count: usize,
}

/// Reward weights for different components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardWeights {
    /// Weight for quality (default: 0.6)
    pub quality: f64,
    /// Weight for cost (default: 0.3)
    pub cost: f64,
    /// Weight for latency (default: 0.1)
    pub latency: f64,
}

impl RewardWeights {
    /// Create new reward weights
    pub fn new(quality: f64, cost: f64, latency: f64) -> Self {
        let total = quality + cost + latency;
        Self {
            quality: quality / total,
            cost: cost / total,
            latency: latency / total,
        }
    }

    /// Create default weights (60% quality, 30% cost, 10% latency)
    pub fn default_weights() -> Self {
        Self {
            quality: 0.6,
            cost: 0.3,
            latency: 0.1,
        }
    }

    /// Create quality-focused weights (80% quality, 15% cost, 5% latency)
    pub fn quality_focused() -> Self {
        Self {
            quality: 0.8,
            cost: 0.15,
            latency: 0.05,
        }
    }

    /// Create cost-focused weights (40% quality, 50% cost, 10% latency)
    pub fn cost_focused() -> Self {
        Self {
            quality: 0.4,
            cost: 0.5,
            latency: 0.1,
        }
    }

    /// Create latency-focused weights (40% quality, 30% cost, 30% latency)
    pub fn latency_focused() -> Self {
        Self {
            quality: 0.4,
            cost: 0.3,
            latency: 0.3,
        }
    }
}

impl Default for RewardWeights {
    fn default() -> Self {
        Self::default_weights()
    }
}

/// Reward calculator
pub struct RewardCalculator {
    weights: RewardWeights,
    max_acceptable_cost: f64,
    max_acceptable_latency: f64,
}

impl RewardCalculator {
    /// Create a new reward calculator
    pub fn new(weights: RewardWeights, max_cost: f64, max_latency: f64) -> Self {
        Self {
            weights,
            max_acceptable_cost: max_cost,
            max_acceptable_latency: max_latency,
        }
    }

    /// Create with default parameters
    pub fn default() -> Self {
        Self::new(
            RewardWeights::default_weights(),
            1.0,      // $1 max cost
            5000.0,   // 5 seconds max latency
        )
    }

    /// Calculate reward from response metrics and user feedback
    pub fn calculate_reward(
        &self,
        metrics: &ResponseMetrics,
        feedback: &UserFeedback,
    ) -> f64 {
        // Quality component
        let quality = match feedback.explicit_rating {
            Some(rating) => rating / 5.0, // Normalize 1-5 to 0-1
            None => {
                // Combine implicit feedback with quality score
                let implicit = feedback.infer_quality();
                let explicit_quality = metrics.quality_score;
                (implicit + explicit_quality) / 2.0
            }
        };

        // Cost component (lower cost = higher reward)
        let cost_reward = (1.0 - (metrics.cost / self.max_acceptable_cost)).clamp(0.0, 1.0);

        // Latency component (lower latency = higher reward)
        let latency_reward = (1.0 - (metrics.latency_ms / self.max_acceptable_latency)).clamp(0.0, 1.0);

        // Weighted combination
        let reward = self.weights.quality * quality
            + self.weights.cost * cost_reward
            + self.weights.latency * latency_reward;

        // Apply penalty for very poor performance
        let penalty = if quality < 0.3 || feedback.retry_occurred {
            0.8 // 20% penalty
        } else {
            1.0
        };

        (reward * penalty).clamp(0.0, 1.0)
    }

    /// Calculate reward with only metrics (no user feedback)
    pub fn calculate_reward_metrics_only(&self, metrics: &ResponseMetrics) -> f64 {
        let quality = metrics.quality_score;
        let cost_reward = (1.0 - (metrics.cost / self.max_acceptable_cost)).clamp(0.0, 1.0);
        let latency_reward = (1.0 - (metrics.latency_ms / self.max_acceptable_latency)).clamp(0.0, 1.0);

        let reward = self.weights.quality * quality
            + self.weights.cost * cost_reward
            + self.weights.latency * latency_reward;

        reward.clamp(0.0, 1.0)
    }

    /// Set new weights
    pub fn set_weights(&mut self, weights: RewardWeights) {
        self.weights = weights;
    }

    /// Get current weights
    pub fn weights(&self) -> &RewardWeights {
        &self.weights
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_feedback_infer_quality() {
        let mut feedback = UserFeedback::new();

        // Positive signals
        feedback.task_completed = true;
        feedback.response_saved = true;
        let quality = feedback.infer_quality();
        assert!(quality > 0.7);

        // Negative signals
        let mut feedback = UserFeedback::new();
        feedback.retry_occurred = true;
        let quality = feedback.infer_quality();
        assert!(quality < 0.3);
    }

    #[test]
    fn test_reward_weights() {
        let weights = RewardWeights::new(6.0, 3.0, 1.0);
        assert!((weights.quality - 0.6).abs() < 0.01);
        assert!((weights.cost - 0.3).abs() < 0.01);
        assert!((weights.latency - 0.1).abs() < 0.01);
    }

    #[test]
    fn test_reward_calculation_with_explicit_feedback() {
        let calculator = RewardCalculator::default();

        let metrics = ResponseMetrics {
            quality_score: 0.9,
            cost: 0.1,
            latency_ms: 1000.0,
            token_count: 500,
        };

        let mut feedback = UserFeedback::new();
        feedback.explicit_rating = Some(5.0);
        feedback.task_completed = true;

        let reward = calculator.calculate_reward(&metrics, &feedback);
        assert!(reward > 0.8, "Expected high reward, got {}", reward);
    }

    #[test]
    fn test_reward_calculation_with_implicit_feedback() {
        let calculator = RewardCalculator::default();

        let metrics = ResponseMetrics {
            quality_score: 0.8,
            cost: 0.2,
            latency_ms: 2000.0,
            token_count: 500,
        };

        let mut feedback = UserFeedback::new();
        feedback.task_completed = true;
        feedback.response_saved = true;

        let reward = calculator.calculate_reward(&metrics, &feedback);
        assert!(reward > 0.5 && reward < 1.0);
    }

    #[test]
    fn test_reward_with_retry_penalty() {
        let calculator = RewardCalculator::default();

        let metrics = ResponseMetrics {
            quality_score: 0.7,
            cost: 0.2,
            latency_ms: 1500.0,
            token_count: 400,
        };

        let mut feedback = UserFeedback::new();
        feedback.retry_occurred = true;

        let reward = calculator.calculate_reward(&metrics, &feedback);
        assert!(reward < 0.5, "Expected penalty for retry, got {}", reward);
    }

    #[test]
    fn test_reward_metrics_only() {
        let calculator = RewardCalculator::default();

        let metrics = ResponseMetrics {
            quality_score: 0.9,
            cost: 0.1,
            latency_ms: 1000.0,
            token_count: 500,
        };

        let reward = calculator.calculate_reward_metrics_only(&metrics);
        assert!(reward > 0.7);
    }

    #[test]
    fn test_cost_focused_weights() {
        let weights = RewardWeights::cost_focused();
        assert!(weights.cost > weights.quality);
        assert!(weights.cost > weights.latency);
    }

    #[test]
    fn test_reward_bounds() {
        let calculator = RewardCalculator::default();

        // Test minimum reward
        let bad_metrics = ResponseMetrics {
            quality_score: 0.0,
            cost: 10.0,
            latency_ms: 10000.0,
            token_count: 1000,
        };
        let mut bad_feedback = UserFeedback::new();
        bad_feedback.retry_occurred = true;

        let reward = calculator.calculate_reward(&bad_metrics, &bad_feedback);
        assert!(reward >= 0.0 && reward <= 1.0);

        // Test maximum reward
        let good_metrics = ResponseMetrics {
            quality_score: 1.0,
            cost: 0.01,
            latency_ms: 100.0,
            token_count: 100,
        };
        let mut good_feedback = UserFeedback::new();
        good_feedback.explicit_rating = Some(5.0);
        good_feedback.task_completed = true;

        let reward = calculator.calculate_reward(&good_metrics, &good_feedback);
        assert!(reward >= 0.0 && reward <= 1.0);
        assert!(reward > 0.9);
    }
}
