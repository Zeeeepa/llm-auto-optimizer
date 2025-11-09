//! Pareto optimization for multi-objective decision making
//!
//! This module implements Pareto frontier calculation and multi-objective
//! optimization for balancing quality, cost, and latency trade-offs.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use uuid::Uuid;

use crate::errors::{DecisionError, Result};

/// Multi-objective optimization objectives
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Objectives {
    /// Quality score (higher is better) [0, 1]
    pub quality: f64,
    /// Cost per request in dollars (lower is better)
    pub cost: f64,
    /// Latency p95 in milliseconds (lower is better)
    pub latency_p95: f64,
}

impl Objectives {
    /// Create new objectives
    pub fn new(quality: f64, cost: f64, latency_p95: f64) -> Self {
        Self {
            quality,
            cost,
            latency_p95,
        }
    }

    /// Validate objectives
    pub fn validate(&self) -> Result<()> {
        if self.quality < 0.0 || self.quality > 1.0 {
            return Err(DecisionError::InvalidConfig(
                format!("Quality {} must be in [0, 1]", self.quality),
            ));
        }
        if self.cost < 0.0 {
            return Err(DecisionError::InvalidConfig(
                format!("Cost {} must be non-negative", self.cost),
            ));
        }
        if self.latency_p95 < 0.0 {
            return Err(DecisionError::InvalidConfig(
                format!("Latency {} must be non-negative", self.latency_p95),
            ));
        }
        Ok(())
    }
}

/// Model candidate for Pareto optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCandidate {
    /// Unique identifier (variant ID or model ID)
    pub id: Uuid,
    /// Human-readable name
    pub name: String,
    /// Objectives for this candidate
    pub objectives: Objectives,
    /// Additional metadata
    pub metadata: Option<serde_json::Value>,
}

impl ModelCandidate {
    /// Create a new model candidate
    pub fn new(id: Uuid, name: impl Into<String>, objectives: Objectives) -> Result<Self> {
        objectives.validate()?;
        Ok(Self {
            id,
            name: name.into(),
            objectives,
            metadata: None,
        })
    }

    /// Add metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Check if this candidate Pareto-dominates another
    ///
    /// Candidate A dominates B if:
    /// - A is at least as good as B in all objectives
    /// - A is strictly better than B in at least one objective
    pub fn dominates(&self, other: &ModelCandidate) -> bool {
        let better_or_equal_quality = self.objectives.quality >= other.objectives.quality;
        let better_or_equal_cost = self.objectives.cost <= other.objectives.cost;
        let better_or_equal_latency = self.objectives.latency_p95 <= other.objectives.latency_p95;

        let strictly_better_quality = self.objectives.quality > other.objectives.quality;
        let strictly_better_cost = self.objectives.cost < other.objectives.cost;
        let strictly_better_latency = self.objectives.latency_p95 < other.objectives.latency_p95;

        // At least as good in all objectives AND strictly better in at least one
        better_or_equal_quality
            && better_or_equal_cost
            && better_or_equal_latency
            && (strictly_better_quality || strictly_better_cost || strictly_better_latency)
    }

    /// Calculate composite score based on weighted objectives
    pub fn composite_score(&self, weights: &ObjectiveWeights) -> f64 {
        // Normalize cost and latency to [0, 1] range (inverted since lower is better)
        // Quality is already in [0, 1]

        let normalized_cost = if weights.max_cost > 0.0 {
            (1.0 - (self.objectives.cost / weights.max_cost).min(1.0)).max(0.0)
        } else {
            1.0
        };

        let normalized_latency = if weights.max_latency > 0.0 {
            (1.0 - (self.objectives.latency_p95 / weights.max_latency).min(1.0)).max(0.0)
        } else {
            1.0
        };

        weights.quality * self.objectives.quality
            + weights.cost * normalized_cost
            + weights.latency * normalized_latency
    }
}

/// Weights for objective scalarization
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ObjectiveWeights {
    /// Weight for quality (default: 0.5)
    pub quality: f64,
    /// Weight for cost (default: 0.3)
    pub cost: f64,
    /// Weight for latency (default: 0.2)
    pub latency: f64,
    /// Maximum acceptable cost for normalization
    pub max_cost: f64,
    /// Maximum acceptable latency for normalization
    pub max_latency: f64,
}

impl ObjectiveWeights {
    /// Create new weights with normalization bounds
    pub fn new(
        quality: f64,
        cost: f64,
        latency: f64,
        max_cost: f64,
        max_latency: f64,
    ) -> Self {
        let total = quality + cost + latency;
        Self {
            quality: quality / total,
            cost: cost / total,
            latency: latency / total,
            max_cost,
            max_latency,
        }
    }

    /// Balanced weights (50% quality, 30% cost, 20% latency)
    pub fn balanced(max_cost: f64, max_latency: f64) -> Self {
        Self::new(0.5, 0.3, 0.2, max_cost, max_latency)
    }

    /// Quality-focused weights (70% quality, 20% cost, 10% latency)
    pub fn quality_focused(max_cost: f64, max_latency: f64) -> Self {
        Self::new(0.7, 0.2, 0.1, max_cost, max_latency)
    }

    /// Cost-focused weights (30% quality, 60% cost, 10% latency)
    pub fn cost_focused(max_cost: f64, max_latency: f64) -> Self {
        Self::new(0.3, 0.6, 0.1, max_cost, max_latency)
    }

    /// Latency-focused weights (40% quality, 30% cost, 30% latency)
    pub fn latency_focused(max_cost: f64, max_latency: f64) -> Self {
        Self::new(0.4, 0.3, 0.3, max_cost, max_latency)
    }
}

impl Default for ObjectiveWeights {
    fn default() -> Self {
        Self::balanced(1.0, 5000.0)
    }
}

/// Pareto frontier calculator
pub struct ParetoFrontier;

impl ParetoFrontier {
    /// Find Pareto optimal set from candidates
    ///
    /// Returns all non-dominated candidates
    pub fn find_pareto_optimal(candidates: &[ModelCandidate]) -> Vec<ModelCandidate> {
        if candidates.is_empty() {
            return Vec::new();
        }

        let mut pareto_set = Vec::new();

        for candidate in candidates {
            let mut is_dominated = false;

            // Check if this candidate is dominated by any other
            for other in candidates {
                if candidate.id != other.id && other.dominates(candidate) {
                    is_dominated = true;
                    break;
                }
            }

            if !is_dominated {
                pareto_set.push(candidate.clone());
            }
        }

        pareto_set
    }

    /// Find Pareto optimal set (efficient version using sorting)
    pub fn find_pareto_optimal_efficient(candidates: &[ModelCandidate]) -> Vec<ModelCandidate> {
        if candidates.is_empty() {
            return Vec::new();
        }

        // Sort by quality (descending) for more efficient domination checking
        let mut sorted: Vec<_> = candidates.to_vec();
        sorted.sort_by(|a, b| {
            b.objectives
                .quality
                .partial_cmp(&a.objectives.quality)
                .unwrap_or(Ordering::Equal)
        });

        let mut pareto_set: Vec<ModelCandidate> = Vec::new();

        for candidate in sorted {
            let mut is_dominated = false;

            // Only need to check against current Pareto set
            for pareto_member in &pareto_set {
                if pareto_member.dominates(&candidate) {
                    is_dominated = true;
                    break;
                }
            }

            if !is_dominated {
                pareto_set.push(candidate);
            }
        }

        pareto_set
    }

    /// Select best candidate from Pareto set based on weights
    pub fn select_optimal(
        pareto_set: &[ModelCandidate],
        weights: &ObjectiveWeights,
    ) -> Option<ModelCandidate> {
        pareto_set
            .iter()
            .max_by(|a, b| {
                let score_a = a.composite_score(weights);
                let score_b = b.composite_score(weights);
                score_a.partial_cmp(&score_b).unwrap_or(Ordering::Equal)
            })
            .cloned()
    }

    /// Calculate hypervolume indicator (quality metric for Pareto set)
    ///
    /// Reference point should be worst acceptable values for all objectives
    pub fn hypervolume(pareto_set: &[ModelCandidate], reference: &Objectives) -> f64 {
        if pareto_set.is_empty() {
            return 0.0;
        }

        // Simplified 2D hypervolume for quality-cost space
        // Sort by quality descending
        let mut sorted: Vec<_> = pareto_set.to_vec();
        sorted.sort_by(|a, b| {
            b.objectives
                .quality
                .partial_cmp(&a.objectives.quality)
                .unwrap_or(Ordering::Equal)
        });

        let mut volume = 0.0;
        let mut prev_cost = 0.0;

        for candidate in sorted {
            let width = (reference.cost - prev_cost).max(0.0);
            let height = (candidate.objectives.quality - 0.0).max(0.0); // 0.0 is min quality
            volume += width * height;
            prev_cost = candidate.objectives.cost.max(prev_cost);
        }

        volume
    }

    /// Calculate spacing metric (uniformity of distribution along Pareto front)
    pub fn spacing(pareto_set: &[ModelCandidate]) -> f64 {
        if pareto_set.len() < 2 {
            return 0.0;
        }

        let distances: Vec<f64> = pareto_set
            .iter()
            .map(|candidate| {
                pareto_set
                    .iter()
                    .filter(|other| other.id != candidate.id)
                    .map(|other| {
                        // Euclidean distance in normalized objective space
                        let dq = candidate.objectives.quality - other.objectives.quality;
                        let dc = (candidate.objectives.cost - other.objectives.cost) / 1.0; // Normalize
                        let dl = (candidate.objectives.latency_p95 - other.objectives.latency_p95)
                            / 5000.0; // Normalize
                        (dq * dq + dc * dc + dl * dl).sqrt()
                    })
                    .min_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal))
                    .unwrap_or(0.0)
            })
            .collect();

        let mean: f64 = distances.iter().sum::<f64>() / distances.len() as f64;
        let variance: f64 = distances
            .iter()
            .map(|d| (d - mean).powi(2))
            .sum::<f64>()
            / distances.len() as f64;

        variance.sqrt()
    }
}

/// Cost calculator for different models
pub struct CostCalculator;

impl CostCalculator {
    /// Calculate token-based cost
    pub fn token_cost(
        input_tokens: usize,
        output_tokens: usize,
        input_price_per_1k: f64,
        output_price_per_1k: f64,
    ) -> f64 {
        let input_cost = (input_tokens as f64 / 1000.0) * input_price_per_1k;
        let output_cost = (output_tokens as f64 / 1000.0) * output_price_per_1k;
        input_cost + output_cost
    }

    /// Calculate compute cost
    pub fn compute_cost(inference_time_seconds: f64, gpu_cost_per_second: f64) -> f64 {
        inference_time_seconds * gpu_cost_per_second
    }

    /// Calculate latency penalty cost
    pub fn latency_penalty(latency_ms: f64, sla_threshold_ms: f64, penalty_per_ms: f64) -> f64 {
        if latency_ms > sla_threshold_ms {
            (latency_ms - sla_threshold_ms) * penalty_per_ms
        } else {
            0.0
        }
    }

    /// Calculate total cost with all components
    pub fn total_cost(
        input_tokens: usize,
        output_tokens: usize,
        input_price_per_1k: f64,
        output_price_per_1k: f64,
        inference_time_seconds: f64,
        gpu_cost_per_second: f64,
        latency_ms: f64,
        sla_threshold_ms: f64,
        penalty_per_ms: f64,
    ) -> f64 {
        let token_cost = Self::token_cost(
            input_tokens,
            output_tokens,
            input_price_per_1k,
            output_price_per_1k,
        );
        let compute = Self::compute_cost(inference_time_seconds, gpu_cost_per_second);
        let penalty = Self::latency_penalty(latency_ms, sla_threshold_ms, penalty_per_ms);

        token_cost + compute + penalty
    }
}

/// Quality metric aggregator
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct QualityMetrics {
    /// Task-specific accuracy [0, 1]
    pub accuracy: f64,
    /// Semantic relevance [0, 1]
    pub relevance: f64,
    /// Logical coherence [0, 1]
    pub coherence: f64,
    /// Information completeness [0, 1]
    pub completeness: f64,
}

impl QualityMetrics {
    /// Create new quality metrics
    pub fn new(accuracy: f64, relevance: f64, coherence: f64, completeness: f64) -> Self {
        Self {
            accuracy,
            relevance,
            coherence,
            completeness,
        }
    }

    /// Calculate aggregate quality score
    pub fn aggregate(&self, weights: Option<[f64; 4]>) -> f64 {
        let w = weights.unwrap_or([0.4, 0.3, 0.2, 0.1]); // Default weights
        let total: f64 = w.iter().sum();

        (w[0] * self.accuracy
            + w[1] * self.relevance
            + w[2] * self.coherence
            + w[3] * self.completeness)
            / total
    }

    /// Validate all metrics are in [0, 1]
    pub fn validate(&self) -> Result<()> {
        for (name, value) in [
            ("accuracy", self.accuracy),
            ("relevance", self.relevance),
            ("coherence", self.coherence),
            ("completeness", self.completeness),
        ] {
            if value < 0.0 || value > 1.0 {
                return Err(DecisionError::InvalidConfig(format!(
                    "{} {} must be in [0, 1]",
                    name, value
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_candidate(
        name: &str,
        quality: f64,
        cost: f64,
        latency: f64,
    ) -> ModelCandidate {
        ModelCandidate::new(
            Uuid::new_v4(),
            name,
            Objectives::new(quality, cost, latency),
        )
        .unwrap()
    }

    #[test]
    fn test_objectives_creation() {
        let obj = Objectives::new(0.9, 0.05, 1000.0);
        assert_eq!(obj.quality, 0.9);
        assert_eq!(obj.cost, 0.05);
        assert_eq!(obj.latency_p95, 1000.0);
    }

    #[test]
    fn test_objectives_validation() {
        let valid = Objectives::new(0.9, 0.05, 1000.0);
        assert!(valid.validate().is_ok());

        let invalid_quality = Objectives::new(1.5, 0.05, 1000.0);
        assert!(invalid_quality.validate().is_err());

        let invalid_cost = Objectives::new(0.9, -0.05, 1000.0);
        assert!(invalid_cost.validate().is_err());
    }

    #[test]
    fn test_dominance_relationship() {
        let better = create_test_candidate("better", 0.9, 0.05, 1000.0);
        let worse = create_test_candidate("worse", 0.7, 0.10, 2000.0);

        assert!(better.dominates(&worse));
        assert!(!worse.dominates(&better));
    }

    #[test]
    fn test_no_dominance_different_objectives() {
        let a = create_test_candidate("a", 0.9, 0.10, 1000.0);
        let b = create_test_candidate("b", 0.7, 0.05, 2000.0);

        // Neither dominates (a has better quality, b has better cost)
        assert!(!a.dominates(&b));
        assert!(!b.dominates(&a));
    }

    #[test]
    fn test_pareto_frontier_simple() {
        let candidates = vec![
            create_test_candidate("best", 0.9, 0.05, 1000.0),
            create_test_candidate("dominated", 0.7, 0.10, 2000.0),
            create_test_candidate("good_cheap", 0.8, 0.03, 1500.0),
        ];

        let pareto = ParetoFrontier::find_pareto_optimal(&candidates);

        // "dominated" should not be in Pareto set
        assert_eq!(pareto.len(), 2);
        assert!(pareto.iter().any(|c| c.name == "best"));
        assert!(pareto.iter().any(|c| c.name == "good_cheap"));
        assert!(!pareto.iter().any(|c| c.name == "dominated"));
    }

    #[test]
    fn test_pareto_frontier_all_optimal() {
        let candidates = vec![
            create_test_candidate("high_quality", 0.95, 0.10, 1500.0),
            create_test_candidate("low_cost", 0.70, 0.03, 2000.0),
            create_test_candidate("low_latency", 0.80, 0.08, 500.0),
        ];

        let pareto = ParetoFrontier::find_pareto_optimal(&candidates);

        // All should be in Pareto set (no candidate dominates another)
        assert_eq!(pareto.len(), 3);
    }

    #[test]
    fn test_composite_score() {
        let candidate = create_test_candidate("test", 0.9, 0.05, 1000.0);
        let weights = ObjectiveWeights::balanced(1.0, 5000.0);

        let score = candidate.composite_score(&weights);

        // Score should be in [0, 1]
        assert!(score >= 0.0 && score <= 1.0);
        // High quality, low cost, low latency should give high score
        assert!(score > 0.7);
    }

    #[test]
    fn test_select_optimal_from_pareto() {
        let pareto_set = vec![
            create_test_candidate("high_quality", 0.95, 0.10, 1500.0),
            create_test_candidate("balanced", 0.85, 0.05, 1000.0),
            create_test_candidate("low_cost", 0.75, 0.02, 1200.0),
        ];

        // Quality-focused weights should prefer high quality
        let weights_quality = ObjectiveWeights::quality_focused(1.0, 5000.0);
        let best = ParetoFrontier::select_optimal(&pareto_set, &weights_quality).unwrap();
        assert_eq!(best.name, "high_quality");

        // Cost-focused should still pick balanced because it's much better on quality
        // and latency, compensating for slightly higher cost (0.05 vs 0.02)
        let weights_cost = ObjectiveWeights::cost_focused(1.0, 5000.0);
        let best = ParetoFrontier::select_optimal(&pareto_set, &weights_cost).unwrap();
        assert_eq!(best.name, "balanced");

        // Test with extreme cost preference (90% cost, 5% quality, 5% latency)
        let weights_extreme_cost = ObjectiveWeights::new(0.05, 0.90, 0.05, 1.0, 5000.0);
        let best = ParetoFrontier::select_optimal(&pareto_set, &weights_extreme_cost).unwrap();
        assert_eq!(best.name, "low_cost");
    }

    #[test]
    fn test_objective_weights() {
        let weights = ObjectiveWeights::new(5.0, 3.0, 2.0, 1.0, 5000.0);

        // Should be normalized
        let total = weights.quality + weights.cost + weights.latency;
        assert!((total - 1.0).abs() < 0.001);

        // Check proportions
        assert!((weights.quality - 0.5).abs() < 0.001);
        assert!((weights.cost - 0.3).abs() < 0.001);
        assert!((weights.latency - 0.2).abs() < 0.001);
    }

    #[test]
    fn test_token_cost_calculation() {
        let cost = CostCalculator::token_cost(
            1000,  // input tokens
            500,   // output tokens
            0.003, // $0.003 per 1k input
            0.015, // $0.015 per 1k output
        );

        let expected = (1000.0 / 1000.0) * 0.003 + (500.0 / 1000.0) * 0.015;
        assert!((cost - expected).abs() < 0.0001);
    }

    #[test]
    fn test_latency_penalty() {
        // No penalty under SLA
        let penalty1 = CostCalculator::latency_penalty(1000.0, 2000.0, 0.0001);
        assert_eq!(penalty1, 0.0);

        // Penalty over SLA
        let penalty2 = CostCalculator::latency_penalty(3000.0, 2000.0, 0.0001);
        assert_eq!(penalty2, (3000.0 - 2000.0) * 0.0001);
    }

    #[test]
    fn test_quality_metrics() {
        let metrics = QualityMetrics::new(0.9, 0.8, 0.85, 0.75);
        assert!(metrics.validate().is_ok());

        let aggregate = metrics.aggregate(None);
        assert!(aggregate >= 0.0 && aggregate <= 1.0);
        assert!(aggregate > 0.7); // All metrics are > 0.7
    }

    #[test]
    fn test_quality_metrics_custom_weights() {
        let metrics = QualityMetrics::new(1.0, 0.5, 0.5, 0.5);

        // Default weights (accuracy weighted most)
        let score1 = metrics.aggregate(None);

        // Equal weights
        let score2 = metrics.aggregate(Some([0.25, 0.25, 0.25, 0.25]));

        // score1 should be higher because accuracy is 1.0 and weighted more
        assert!(score1 > score2);
    }

    #[test]
    fn test_hypervolume() {
        let pareto_set = vec![
            create_test_candidate("a", 0.9, 0.05, 1000.0),
            create_test_candidate("b", 0.8, 0.03, 1500.0),
        ];

        let reference = Objectives::new(0.0, 1.0, 5000.0);
        let hv = ParetoFrontier::hypervolume(&pareto_set, &reference);

        assert!(hv > 0.0);
    }

    #[test]
    fn test_efficient_pareto_frontier() {
        let candidates = vec![
            create_test_candidate("best", 0.9, 0.05, 1000.0),
            create_test_candidate("dominated", 0.7, 0.10, 2000.0),
            create_test_candidate("good_cheap", 0.8, 0.03, 1500.0),
        ];

        let pareto1 = ParetoFrontier::find_pareto_optimal(&candidates);
        let pareto2 = ParetoFrontier::find_pareto_optimal_efficient(&candidates);

        // Both methods should give same results
        assert_eq!(pareto1.len(), pareto2.len());
    }

    #[test]
    fn test_spacing_metric() {
        let well_spaced = vec![
            create_test_candidate("a", 0.9, 0.05, 1000.0),
            create_test_candidate("b", 0.7, 0.08, 2000.0),
            create_test_candidate("c", 0.5, 0.11, 3000.0),
        ];

        let spacing = ParetoFrontier::spacing(&well_spaced);
        assert!(spacing >= 0.0);
    }
}
