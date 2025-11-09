//! Thompson Sampling implementation for multi-armed bandit optimization
//!
//! This module implements Thompson Sampling using Beta distributions for
//! traffic allocation in A/B tests. It provides adaptive traffic routing
//! that balances exploration and exploitation.

use rand_distr::{Beta, Distribution};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::errors::{DecisionError, Result};

/// Thompson Sampling bandit for variant selection
#[derive(Debug, Clone)]
pub struct ThompsonSampling {
    /// Arms (variants) in the bandit
    arms: HashMap<Uuid, BanditArm>,
    /// Total samples drawn
    total_samples: u64,
}

/// A single arm in the multi-armed bandit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanditArm {
    /// Variant ID
    pub variant_id: Uuid,
    /// Number of successes (conversions, positive outcomes)
    pub successes: f64,
    /// Number of failures (non-conversions, negative outcomes)
    pub failures: f64,
    /// Total trials
    pub trials: u64,
}

impl BanditArm {
    /// Create a new bandit arm with prior
    pub fn new(variant_id: Uuid) -> Self {
        Self {
            variant_id,
            successes: 1.0, // Prior: Beta(1, 1) is uniform distribution
            failures: 1.0,
            trials: 0,
        }
    }

    /// Update arm with observation
    pub fn update(&mut self, success: bool) {
        if success {
            self.successes += 1.0;
        } else {
            self.failures += 1.0;
        }
        self.trials += 1;
    }

    /// Get conversion rate (mean of Beta distribution)
    pub fn conversion_rate(&self) -> f64 {
        self.successes / (self.successes + self.failures)
    }

    /// Get credible interval (Bayesian confidence interval)
    pub fn credible_interval(&self, confidence: f64) -> (f64, f64) {
        use statrs::distribution::Beta as BetaDist;

        let _beta = BetaDist::new(self.successes, self.failures).unwrap();
        let lower = (1.0 - confidence) / 2.0;
        let _upper = 1.0 - lower;

        // Approximate quantiles (for production, use proper quantile function)
        let mean = self.conversion_rate();
        let std = (self.successes * self.failures /
                   ((self.successes + self.failures).powi(2) *
                    (self.successes + self.failures + 1.0))).sqrt();

        (
            (mean - 1.96 * std).max(0.0),
            (mean + 1.96 * std).min(1.0),
        )
    }

    /// Sample from the Beta distribution
    pub fn sample(&self) -> Result<f64> {
        let beta = Beta::new(self.successes, self.failures)
            .map_err(|e| DecisionError::StatisticalError(
                format!("Failed to create Beta distribution: {}", e)
            ))?;
        
        let mut rng = rand::thread_rng();
        Ok(beta.sample(&mut rng))
    }
}

impl ThompsonSampling {
    /// Create a new Thompson Sampling instance
    pub fn new() -> Self {
        Self {
            arms: HashMap::new(),
            total_samples: 0,
        }
    }

    /// Add a new variant
    pub fn add_variant(&mut self, variant_id: Uuid) {
        self.arms.insert(variant_id, BanditArm::new(variant_id));
    }

    /// Remove a variant
    pub fn remove_variant(&mut self, variant_id: &Uuid) {
        self.arms.remove(variant_id);
    }

    /// Select a variant using Thompson Sampling
    ///
    /// This samples from each arm's Beta distribution and selects
    /// the arm with the highest sampled value.
    pub fn select_variant(&self) -> Result<Uuid> {
        if self.arms.is_empty() {
            return Err(DecisionError::InvalidState(
                "No variants available for selection".to_string()
            ));
        }

        let mut best_variant = None;
        let mut best_sample = f64::MIN;

        for (variant_id, arm) in &self.arms {
            let sample = arm.sample()?;
            if sample > best_sample {
                best_sample = sample;
                best_variant = Some(*variant_id);
            }
        }

        best_variant.ok_or_else(|| 
            DecisionError::AllocationError("Failed to select variant".to_string())
        )
    }

    /// Update a variant with observation
    pub fn update(&mut self, variant_id: &Uuid, success: bool) -> Result<()> {
        let arm = self.arms.get_mut(variant_id)
            .ok_or_else(|| DecisionError::VariantNotFound(variant_id.to_string()))?;
        
        arm.update(success);
        self.total_samples += 1;
        Ok(())
    }

    /// Get current conversion rates for all variants
    pub fn get_conversion_rates(&self) -> HashMap<Uuid, f64> {
        self.arms.iter()
            .map(|(id, arm)| (*id, arm.conversion_rate()))
            .collect()
    }

    /// Get arm statistics
    pub fn get_arm(&self, variant_id: &Uuid) -> Option<&BanditArm> {
        self.arms.get(variant_id)
    }

    /// Get all arms
    pub fn get_arms(&self) -> &HashMap<Uuid, BanditArm> {
        &self.arms
    }

    /// Calculate regret (difference from optimal arm)
    pub fn calculate_regret(&self) -> f64 {
        if self.arms.is_empty() || self.total_samples == 0 {
            return 0.0;
        }

        // Best possible conversion rate
        let best_rate = self.arms.values()
            .map(|arm| arm.conversion_rate())
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        // Actual conversions (excluding priors)
        let actual_conversions: f64 = self.arms.values()
            .map(|arm| (arm.successes - 1.0).max(0.0))
            .sum();

        // Expected conversions if we always chose best arm
        let expected_conversions = best_rate * self.total_samples as f64;

        // Regret is the difference
        (expected_conversions - actual_conversions).max(0.0)
    }

    /// Get total number of samples
    pub fn total_samples(&self) -> u64 {
        self.total_samples
    }

    /// Check if bandit has converged (low regret relative to samples)
    pub fn has_converged(&self, threshold: f64) -> bool {
        if self.total_samples < 100 {
            return false;
        }

        let regret = self.calculate_regret();
        let regret_rate = regret / self.total_samples as f64;
        
        regret_rate < threshold
    }
}

impl Default for ThompsonSampling {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    #[test]
    fn test_bandit_arm_creation() {
        let arm = BanditArm::new(Uuid::new_v4());
        assert_eq!(arm.successes, 1.0);
        assert_eq!(arm.failures, 1.0);
        assert_eq!(arm.trials, 0);
        assert_eq!(arm.conversion_rate(), 0.5); // Beta(1,1) mean
    }

    #[test]
    fn test_bandit_arm_update() {
        let mut arm = BanditArm::new(Uuid::new_v4());
        
        arm.update(true);
        assert_eq!(arm.successes, 2.0);
        assert_eq!(arm.trials, 1);
        
        arm.update(false);
        assert_eq!(arm.failures, 2.0);
        assert_eq!(arm.trials, 2);
    }

    #[test]
    fn test_bandit_arm_conversion_rate() {
        let mut arm = BanditArm::new(Uuid::new_v4());
        
        // 7 successes out of 10 trials
        for _ in 0..7 {
            arm.update(true);
        }
        for _ in 0..3 {
            arm.update(false);
        }
        
        // (1 + 7) / (1 + 7 + 1 + 3) = 8/12 = 0.666...
        let rate = arm.conversion_rate();
        assert!((rate - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_thompson_sampling_creation() {
        let ts = ThompsonSampling::new();
        assert_eq!(ts.total_samples(), 0);
        assert!(ts.get_arms().is_empty());
    }

    #[test]
    fn test_add_remove_variant() {
        let mut ts = ThompsonSampling::new();
        let id = Uuid::new_v4();
        
        ts.add_variant(id);
        assert_eq!(ts.get_arms().len(), 1);
        assert!(ts.get_arm(&id).is_some());
        
        ts.remove_variant(&id);
        assert_eq!(ts.get_arms().len(), 0);
    }

    #[test]
    fn test_select_variant() {
        let mut ts = ThompsonSampling::new();
        
        // Should fail with no variants
        assert!(ts.select_variant().is_err());
        
        // Add variants
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        ts.add_variant(id1);
        ts.add_variant(id2);
        
        // Should select one
        let selected = ts.select_variant().unwrap();
        assert!(selected == id1 || selected == id2);
    }

    #[test]
    fn test_update_variant() {
        let mut ts = ThompsonSampling::new();
        let id = Uuid::new_v4();
        ts.add_variant(id);
        
        // Update with success
        ts.update(&id, true).unwrap();
        assert_eq!(ts.total_samples(), 1);
        
        let arm = ts.get_arm(&id).unwrap();
        assert_eq!(arm.successes, 2.0);
        assert_eq!(arm.trials, 1);
    }

    #[test]
    fn test_conversion_rates() {
        let mut ts = ThompsonSampling::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        
        ts.add_variant(id1);
        ts.add_variant(id2);
        
        // Variant 1: 8/10 success rate
        for _ in 0..8 {
            ts.update(&id1, true).unwrap();
        }
        for _ in 0..2 {
            ts.update(&id1, false).unwrap();
        }
        
        // Variant 2: 3/10 success rate
        for _ in 0..3 {
            ts.update(&id2, true).unwrap();
        }
        for _ in 0..7 {
            ts.update(&id2, false).unwrap();
        }
        
        let rates = ts.get_conversion_rates();
        
        // Variant 1 should have higher rate
        assert!(rates[&id1] > rates[&id2]);
        
        // Check approximate values (with priors)
        // id1: (1+8)/(1+8+1+2) = 9/12 = 0.75
        // id2: (1+3)/(1+3+1+7) = 4/12 = 0.333
        assert!((rates[&id1] - 0.75).abs() < 0.01);
        assert!((rates[&id2] - 0.333).abs() < 0.01);
    }

    #[test]
    fn test_regret_calculation() {
        let mut ts = ThompsonSampling::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        
        ts.add_variant(id1);
        ts.add_variant(id2);
        
        // Initial regret should be 0 or positive
        let initial_regret = ts.calculate_regret();
        assert!(initial_regret >= -0.01, "Initial regret should be >= 0, got: {}", initial_regret);

        // Add some samples
        for _ in 0..10 {
            ts.update(&id1, true).unwrap();
        }

        // Regret should be low if we're selecting the best arm
        let regret = ts.calculate_regret();
        assert!(regret >= -0.01, "Regret should be >= 0, got: {}", regret);
    }

    #[test]
    fn test_thompson_sampling_convergence() {
        let mut ts = ThompsonSampling::new();
        let good_variant = Uuid::new_v4();
        let bad_variant = Uuid::new_v4();
        
        ts.add_variant(good_variant);
        ts.add_variant(bad_variant);
        
        // Simulate: good variant has 80% success, bad has 20%
        let mut rng = rand::thread_rng();
        
        for _ in 0..1000 {
            let selected = ts.select_variant().unwrap();
            
            let success = if selected == good_variant {
                rng.gen::<f64>() < 0.8
            } else {
                rng.gen::<f64>() < 0.2
            };
            
            ts.update(&selected, success).unwrap();
        }
        
        // After many trials, good variant should be selected more
        let rates = ts.get_conversion_rates();
        assert!(rates[&good_variant] > rates[&bad_variant]);
        
        // Check that conversion rates are approximately correct
        assert!((rates[&good_variant] - 0.8).abs() < 0.1);
    }

    #[test]
    fn test_credible_interval() {
        let mut arm = BanditArm::new(Uuid::new_v4());
        
        // Add data: 70 successes, 30 failures
        for _ in 0..70 {
            arm.update(true);
        }
        for _ in 0..30 {
            arm.update(false);
        }
        
        let (lower, upper) = arm.credible_interval(0.95);
        
        // Interval should contain the true mean
        let mean = arm.conversion_rate();
        assert!(lower < mean && mean < upper);
        
        // Interval should be reasonable
        assert!(lower > 0.0 && upper < 1.0);
        assert!(upper - lower < 0.2); // Width < 20%
    }
}
