//! Statistical significance testing for A/B experiments
//!
//! This module provides statistical tests to determine if differences
//! between variants are statistically significant.

use statrs::distribution::{ContinuousCDF, Normal};

use crate::errors::{DecisionError, Result};

/// Statistical test trait
pub trait StatisticalTest {
    /// Perform the test and return p-value
    fn test(&self) -> Result<f64>;
    
    /// Check if result is significant at given alpha level
    fn is_significant(&self, alpha: f64) -> Result<bool> {
        Ok(self.test()? < alpha)
    }
}

/// Two-proportion z-test for comparing conversion rates
///
/// Tests the null hypothesis that two proportions are equal.
/// Used to compare conversion rates between control and treatment groups.
#[derive(Debug, Clone)]
pub struct ZTest {
    /// Successes in group 1
    pub successes_1: u64,
    /// Total trials in group 1
    pub trials_1: u64,
    /// Successes in group 2
    pub successes_2: u64,
    /// Total trials in group 2
    pub trials_2: u64,
}

impl ZTest {
    /// Create a new z-test
    pub fn new(successes_1: u64, trials_1: u64, successes_2: u64, trials_2: u64) -> Self {
        Self {
            successes_1,
            trials_1,
            successes_2,
            trials_2,
        }
    }

    /// Calculate sample proportions
    pub fn proportions(&self) -> (f64, f64) {
        let p1 = if self.trials_1 > 0 {
            self.successes_1 as f64 / self.trials_1 as f64
        } else {
            0.0
        };
        
        let p2 = if self.trials_2 > 0 {
            self.successes_2 as f64 / self.trials_2 as f64
        } else {
            0.0
        };
        
        (p1, p2)
    }

    /// Calculate pooled proportion
    pub fn pooled_proportion(&self) -> f64 {
        let total_successes = self.successes_1 + self.successes_2;
        let total_trials = self.trials_1 + self.trials_2;
        
        if total_trials > 0 {
            total_successes as f64 / total_trials as f64
        } else {
            0.0
        }
    }

    /// Calculate z-statistic
    pub fn z_statistic(&self) -> Result<f64> {
        let (p1, p2) = self.proportions();
        let p_pool = self.pooled_proportion();
        
        let n1 = self.trials_1 as f64;
        let n2 = self.trials_2 as f64;
        
        if n1 == 0.0 || n2 == 0.0 {
            return Err(DecisionError::InsufficientData(
                "Cannot perform z-test with zero trials".to_string()
            ));
        }
        
        // Standard error: sqrt(p_pool * (1 - p_pool) * (1/n1 + 1/n2))
        let se = (p_pool * (1.0 - p_pool) * (1.0/n1 + 1.0/n2)).sqrt();
        
        if se == 0.0 {
            return Err(DecisionError::StatisticalError(
                "Standard error is zero, cannot compute z-statistic".to_string()
            ));
        }
        
        // Z = (p1 - p2) / SE
        Ok((p1 - p2) / se)
    }

    /// Calculate confidence interval for difference in proportions
    pub fn confidence_interval(&self, confidence: f64) -> Result<(f64, f64)> {
        let (p1, p2) = self.proportions();
        let diff = p1 - p2;
        
        let n1 = self.trials_1 as f64;
        let n2 = self.trials_2 as f64;
        
        if n1 == 0.0 || n2 == 0.0 {
            return Err(DecisionError::InsufficientData(
                "Cannot calculate confidence interval with zero trials".to_string()
            ));
        }
        
        // Standard error for difference: sqrt(p1(1-p1)/n1 + p2(1-p2)/n2)
        let se = ((p1 * (1.0 - p1) / n1) + (p2 * (1.0 - p2) / n2)).sqrt();
        
        // Z-score for confidence level
        let z = match confidence {
            c if (c - 0.90).abs() < 0.001 => 1.645,
            c if (c - 0.95).abs() < 0.001 => 1.96,
            c if (c - 0.99).abs() < 0.001 => 2.576,
            _ => {
                let normal = Normal::new(0.0, 1.0)
                    .map_err(|e| DecisionError::StatisticalError(e.to_string()))?;
                let alpha = 1.0 - confidence;
                normal.inverse_cdf(1.0 - alpha / 2.0)
            }
        };
        
        let margin = z * se;
        Ok((diff - margin, diff + margin))
    }

    /// Calculate effect size (Cohen's h)
    pub fn effect_size(&self) -> f64 {
        let (p1, p2) = self.proportions();
        
        // Cohen's h = 2 * (arcsin(sqrt(p1)) - arcsin(sqrt(p2)))
        2.0 * (p1.sqrt().asin() - p2.sqrt().asin())
    }

    /// Calculate statistical power (approximate)
    pub fn power(&self, alpha: f64, effect_size: f64) -> Result<f64> {
        let n1 = self.trials_1 as f64;
        let n2 = self.trials_2 as f64;
        
        if n1 == 0.0 || n2 == 0.0 {
            return Ok(0.0);
        }
        
        // Simplified power calculation
        let n_harmonic = 2.0 / (1.0/n1 + 1.0/n2);
        let noncentrality = effect_size * (n_harmonic / 4.0).sqrt();
        
        let normal = Normal::new(0.0, 1.0)
            .map_err(|e| DecisionError::StatisticalError(e.to_string()))?;
        
        let z_alpha = normal.inverse_cdf(1.0 - alpha / 2.0);
        let power = 1.0 - normal.cdf(z_alpha - noncentrality);
        
        Ok(power)
    }
}

impl StatisticalTest for ZTest {
    /// Perform two-tailed z-test and return p-value
    fn test(&self) -> Result<f64> {
        let z = self.z_statistic()?;
        
        let normal = Normal::new(0.0, 1.0)
            .map_err(|e| DecisionError::StatisticalError(e.to_string()))?;
        
        // Two-tailed p-value
        let p_value = 2.0 * (1.0 - normal.cdf(z.abs()));
        
        Ok(p_value)
    }
}

/// Sample size calculator for A/B tests
pub struct SampleSizeCalculator {
    /// Baseline conversion rate
    pub baseline_rate: f64,
    /// Minimum detectable effect (relative improvement)
    pub min_effect: f64,
    /// Statistical power (1 - beta)
    pub power: f64,
    /// Significance level (alpha)
    pub alpha: f64,
}

impl SampleSizeCalculator {
    /// Create a new sample size calculator
    pub fn new(baseline_rate: f64, min_effect: f64, power: f64, alpha: f64) -> Result<Self> {
        if baseline_rate <= 0.0 || baseline_rate >= 1.0 {
            return Err(DecisionError::InvalidConfig(
                "Baseline rate must be between 0 and 1".to_string()
            ));
        }
        
        if power <= 0.0 || power >= 1.0 {
            return Err(DecisionError::InvalidConfig(
                "Power must be between 0 and 1".to_string()
            ));
        }
        
        if alpha <= 0.0 || alpha >= 1.0 {
            return Err(DecisionError::InvalidConfig(
                "Alpha must be between 0 and 1".to_string()
            ));
        }
        
        Ok(Self {
            baseline_rate,
            min_effect,
            power,
            alpha,
        })
    }

    /// Calculate required sample size per variant
    pub fn calculate(&self) -> Result<usize> {
        let p1 = self.baseline_rate;
        let p2 = self.baseline_rate * (1.0 + self.min_effect);
        
        if p2 >= 1.0 {
            return Err(DecisionError::InvalidConfig(
                "Effect size too large, treatment rate exceeds 1.0".to_string()
            ));
        }
        
        let normal = Normal::new(0.0, 1.0)
            .map_err(|e| DecisionError::StatisticalError(e.to_string()))?;
        
        let z_alpha = normal.inverse_cdf(1.0 - self.alpha / 2.0);
        let z_beta = normal.inverse_cdf(self.power);
        
        let p_avg = (p1 + p2) / 2.0;
        let delta = (p2 - p1).abs();
        
        // Sample size formula
        let n = ((z_alpha + z_beta).powi(2) * 2.0 * p_avg * (1.0 - p_avg)) / delta.powi(2);
        
        Ok(n.ceil() as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_z_test_proportions() {
        let test = ZTest::new(50, 100, 60, 100);
        let (p1, p2) = test.proportions();
        
        assert_eq!(p1, 0.5);
        assert_eq!(p2, 0.6);
    }

    #[test]
    fn test_pooled_proportion() {
        let test = ZTest::new(50, 100, 60, 100);
        let p_pool = test.pooled_proportion();
        
        // (50 + 60) / (100 + 100) = 110/200 = 0.55
        assert_eq!(p_pool, 0.55);
    }

    #[test]
    fn test_z_statistic() {
        let test = ZTest::new(50, 100, 60, 100);
        let z = test.z_statistic().unwrap();
        
        // Should be negative (p1 < p2)
        assert!(z < 0.0);
        
        // Approximate value
        assert!(z.abs() > 1.0 && z.abs() < 2.0);
    }

    #[test]
    fn test_z_test_significant_difference() {
        // Large difference should be significant
        let test = ZTest::new(30, 100, 70, 100);
        let p_value = test.test().unwrap();
        
        // Should be highly significant (p < 0.05)
        assert!(p_value < 0.05);
    }

    #[test]
    fn test_z_test_no_difference() {
        // Same proportions should not be significant
        let test = ZTest::new(50, 100, 50, 100);
        let p_value = test.test().unwrap();
        
        // Should not be significant
        assert!(p_value > 0.05);
    }

    #[test]
    fn test_is_significant() {
        let test = ZTest::new(30, 100, 70, 100);
        
        // Should be significant at 0.05 level
        assert!(test.is_significant(0.05).unwrap());
        
        // Might not be at 0.001 level
        // (depends on exact z-value)
    }

    #[test]
    fn test_confidence_interval() {
        let test = ZTest::new(50, 100, 60, 100);
        let (lower, upper) = test.confidence_interval(0.95).unwrap();
        
        // Difference is -0.1 (0.5 - 0.6)
        let diff = -0.1;
        
        // Interval should contain the difference
        assert!(lower < diff && diff < upper);
        
        // Interval should be reasonable
        assert!(upper - lower < 0.3);
    }

    #[test]
    fn test_effect_size() {
        let test = ZTest::new(30, 100, 70, 100);
        let h = test.effect_size();
        
        // Cohen's h for 0.3 vs 0.7 should be substantial
        assert!(h.abs() > 0.5);
    }

    #[test]
    fn test_insufficient_data_error() {
        let test = ZTest::new(5, 10, 0, 0);
        
        // Should error with zero trials in group 2
        assert!(test.z_statistic().is_err());
    }

    #[test]
    fn test_sample_size_calculator() {
        let calc = SampleSizeCalculator::new(
            0.1,   // 10% baseline
            0.2,   // 20% relative improvement (10% -> 12%)
            0.8,   // 80% power
            0.05,  // 5% significance
        ).unwrap();
        
        let n = calc.calculate().unwrap();
        
        // Should require substantial sample size
        assert!(n > 100);
        assert!(n < 100000); // Sanity check
    }

    #[test]
    fn test_sample_size_larger_effect() {
        let small_effect = SampleSizeCalculator::new(0.1, 0.1, 0.8, 0.05)
            .unwrap()
            .calculate()
            .unwrap();
        
        let large_effect = SampleSizeCalculator::new(0.1, 0.5, 0.8, 0.05)
            .unwrap()
            .calculate()
            .unwrap();
        
        // Larger effect requires smaller sample
        assert!(large_effect < small_effect);
    }

    #[test]
    fn test_power_calculation() {
        let test = ZTest::new(500, 1000, 550, 1000);
        let power = test.power(0.05, 0.1).unwrap();
        
        // Power should be between 0 and 1
        assert!(power > 0.0 && power < 1.0);
    }

    #[test]
    fn test_real_world_scenario() {
        // Realistic A/B test: 10% vs 15% conversion
        // with 1000 samples per variant (larger effect for reliable significance)
        let test = ZTest::new(100, 1000, 150, 1000);

        let (p1, p2) = test.proportions();
        assert_relative_eq!(p1, 0.1, epsilon = 0.001);
        assert_relative_eq!(p2, 0.15, epsilon = 0.001);

        let p_value = test.test().unwrap();

        // With 1000 samples and 5% absolute difference, should be significant
        assert!(p_value < 0.05, "p-value {} should be < 0.05", p_value);

        let (lower, upper) = test.confidence_interval(0.95).unwrap();

        // CI should not include 0 (since it's significant)
        assert!(lower < 0.0 && upper < 0.0 || lower > 0.0 && upper > 0.0);
    }
}
