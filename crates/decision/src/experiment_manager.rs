//! Experiment lifecycle management
//!
//! This module manages the full lifecycle of A/B experiments including
//! creation, execution, monitoring, and conclusion.

use chrono::Utc;
use dashmap::DashMap;
use llm_optimizer_types::experiments::*;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    errors::{DecisionError, Result},
    statistical::{StatisticalTest, ZTest},
    thompson_sampling::ThompsonSampling,
};

/// Experiment lifecycle manager
pub struct ExperimentManager {
    /// Active experiments
    experiments: Arc<DashMap<Uuid, Experiment>>,
    /// Thompson Sampling bandits per experiment
    bandits: Arc<DashMap<Uuid, ThompsonSampling>>,
}

impl ExperimentManager {
    /// Create a new experiment manager
    pub fn new() -> Self {
        Self {
            experiments: Arc::new(DashMap::new()),
            bandits: Arc::new(DashMap::new()),
        }
    }

    /// Create a new experiment
    pub fn create_experiment(
        &self,
        name: impl Into<String>,
        variants: Vec<Variant>,
        _metrics: Vec<MetricDefinition>,
    ) -> Result<Uuid> {
        if variants.len() < 2 {
            return Err(DecisionError::InvalidConfig(
                "Experiment must have at least 2 variants".to_string()
            ));
        }

        // Validate traffic allocation sums to 1.0
        let total_allocation: f64 = variants.iter()
            .map(|v| v.traffic_allocation)
            .sum();
        
        if (total_allocation - 1.0).abs() > 0.01 {
            return Err(DecisionError::InvalidConfig(
                format!("Traffic allocation must sum to 1.0, got {}", total_allocation)
            ));
        }

        let experiment = Experiment::new(name, variants.clone());
        let experiment_id = experiment.id;

        // Create Thompson Sampling bandit
        let mut bandit = ThompsonSampling::new();
        for variant in &variants {
            bandit.add_variant(variant.id);
        }

        self.experiments.insert(experiment_id, experiment);
        self.bandits.insert(experiment_id, bandit);

        Ok(experiment_id)
    }

    /// Start an experiment
    pub fn start_experiment(&self, experiment_id: &Uuid) -> Result<()> {
        let mut entry = self.experiments.get_mut(experiment_id)
            .ok_or_else(|| DecisionError::ExperimentNotFound(experiment_id.to_string()))?;
        
        entry.start();
        Ok(())
    }

    /// Pause an experiment
    pub fn pause_experiment(&self, experiment_id: &Uuid) -> Result<()> {
        let mut entry = self.experiments.get_mut(experiment_id)
            .ok_or_else(|| DecisionError::ExperimentNotFound(experiment_id.to_string()))?;
        
        if entry.status == ExperimentStatus::Running {
            entry.status = ExperimentStatus::Paused;
            Ok(())
        } else {
            Err(DecisionError::InvalidState(
                format!("Cannot pause experiment in state {:?}", entry.status)
            ))
        }
    }

    /// Resume an experiment
    pub fn resume_experiment(&self, experiment_id: &Uuid) -> Result<()> {
        let mut entry = self.experiments.get_mut(experiment_id)
            .ok_or_else(|| DecisionError::ExperimentNotFound(experiment_id.to_string()))?;
        
        if entry.status == ExperimentStatus::Paused {
            entry.status = ExperimentStatus::Running;
            Ok(())
        } else {
            Err(DecisionError::InvalidState(
                format!("Cannot resume experiment in state {:?}", entry.status)
            ))
        }
    }

    /// Select a variant for a request using Thompson Sampling
    pub fn select_variant(&self, experiment_id: &Uuid) -> Result<Uuid> {
        // Check experiment is running
        let experiment = self.experiments.get(experiment_id)
            .ok_or_else(|| DecisionError::ExperimentNotFound(experiment_id.to_string()))?;
        
        if experiment.status != ExperimentStatus::Running {
            return Err(DecisionError::InvalidState(
                format!("Experiment is not running: {:?}", experiment.status)
            ));
        }

        // Use Thompson Sampling to select variant
        let bandit = self.bandits.get(experiment_id)
            .ok_or_else(|| DecisionError::ExperimentNotFound(experiment_id.to_string()))?;
        
        bandit.select_variant()
    }

    /// Record result for a variant
    pub fn record_result(
        &self,
        experiment_id: &Uuid,
        variant_id: &Uuid,
        success: bool,
        quality: f64,
        cost: f64,
        latency_ms: f64,
    ) -> Result<()> {
        // Update Thompson Sampling bandit
        if let Some(mut bandit) = self.bandits.get_mut(experiment_id) {
            bandit.update(variant_id, success)?;
        }

        // Update variant results
        let mut experiment = self.experiments.get_mut(experiment_id)
            .ok_or_else(|| DecisionError::ExperimentNotFound(experiment_id.to_string()))?;
        
        let variant = experiment.variants.iter_mut()
            .find(|v| v.id == *variant_id)
            .ok_or_else(|| DecisionError::VariantNotFound(variant_id.to_string()))?;

        // Initialize results if needed
        if variant.results.is_none() {
            variant.results = Some(VariantResults {
                total_requests: 0,
                conversions: 0,
                avg_quality: 0.0,
                avg_cost: 0.0,
                avg_latency_ms: 0.0,
                metrics: Default::default(),
            });
        }

        // Update results
        if let Some(results) = &mut variant.results {
            let n = results.total_requests as f64;
            
            // Update running averages
            results.avg_quality = (results.avg_quality * n + quality) / (n + 1.0);
            results.avg_cost = (results.avg_cost * n + cost) / (n + 1.0);
            results.avg_latency_ms = (results.avg_latency_ms * n + latency_ms) / (n + 1.0);
            
            results.total_requests += 1;
            if success {
                results.conversions += 1;
            }
        }

        Ok(())
    }

    /// Check if experiment should conclude
    pub fn should_conclude(&self, experiment_id: &Uuid, min_sample_size: usize, significance_level: f64) -> Result<bool> {
        let experiment = self.experiments.get(experiment_id)
            .ok_or_else(|| DecisionError::ExperimentNotFound(experiment_id.to_string()))?;
        
        if experiment.variants.len() != 2 {
            // For now, only support 2-variant experiments
            return Ok(false);
        }

        let variant1 = &experiment.variants[0];
        let variant2 = &experiment.variants[1];

        // Check minimum sample size
        let results1 = variant1.results.as_ref();
        let results2 = variant2.results.as_ref();

        if results1.is_none() || results2.is_none() {
            return Ok(false);
        }

        let r1 = results1.unwrap();
        let r2 = results2.unwrap();

        if r1.total_requests < min_sample_size as u64 || r2.total_requests < min_sample_size as u64 {
            return Ok(false);
        }

        // Perform statistical test
        let z_test = ZTest::new(
            r1.conversions,
            r1.total_requests,
            r2.conversions,
            r2.total_requests,
        );

        let is_significant = z_test.is_significant(significance_level)?;

        Ok(is_significant)
    }

    /// Conclude experiment and determine winner
    pub fn conclude_experiment(&self, experiment_id: &Uuid, significance_level: f64) -> Result<()> {
        let mut experiment = self.experiments.get_mut(experiment_id)
            .ok_or_else(|| DecisionError::ExperimentNotFound(experiment_id.to_string()))?;
        
        if experiment.variants.len() != 2 {
            return Err(DecisionError::InvalidConfig(
                "Can only conclude 2-variant experiments".to_string()
            ));
        }

        let variant1 = &experiment.variants[0];
        let variant2 = &experiment.variants[1];

        let results1 = variant1.results.as_ref()
            .ok_or_else(|| DecisionError::InsufficientData("Variant 1 has no results".to_string()))?;
        let results2 = variant2.results.as_ref()
            .ok_or_else(|| DecisionError::InsufficientData("Variant 2 has no results".to_string()))?;

        // Perform statistical analysis
        let z_test = ZTest::new(
            results1.conversions,
            results1.total_requests,
            results2.conversions,
            results2.total_requests,
        );

        let p_value = z_test.test()?;
        let is_significant = p_value < significance_level;
        let effect_size = z_test.effect_size();

        // Determine winner
        let winner_variant_id = if is_significant {
            if results1.conversions as f64 / results1.total_requests as f64 >
               results2.conversions as f64 / results2.total_requests as f64 {
                Some(variant1.id)
            } else {
                Some(variant2.id)
            }
        } else {
            None
        };

        // Create statistical analysis
        let analysis = StatisticalAnalysis {
            winner_variant_id,
            p_value,
            confidence_level: 1.0 - significance_level,
            effect_size,
            is_significant,
            method: "Two-proportion z-test".to_string(),
        };

        // Create experiment results
        let mut variant_details = std::collections::HashMap::new();
        variant_details.insert(variant1.id, results1.clone());
        variant_details.insert(variant2.id, results2.clone());

        let duration_seconds = (Utc::now() - experiment.start_time).num_seconds() as u64;
        let total_sample_size = results1.total_requests + results2.total_requests;

        let results = ExperimentResults {
            statistical_analysis: analysis,
            variant_details,
            total_sample_size,
            duration_seconds,
        };

        experiment.complete(results);

        Ok(())
    }

    /// Get experiment by ID
    pub fn get_experiment(&self, experiment_id: &Uuid) -> Option<Experiment> {
        self.experiments.get(experiment_id).map(|e| e.clone())
    }

    /// Get all experiments
    pub fn list_experiments(&self) -> Vec<Experiment> {
        self.experiments.iter().map(|e| e.value().clone()).collect()
    }

    /// Get active experiments
    pub fn list_active_experiments(&self) -> Vec<Experiment> {
        self.experiments.iter()
            .filter(|e| e.status == ExperimentStatus::Running)
            .map(|e| e.value().clone())
            .collect()
    }

    /// Get experiment statistics
    pub fn get_statistics(&self, experiment_id: &Uuid) -> Result<ExperimentStatistics> {
        let experiment = self.experiments.get(experiment_id)
            .ok_or_else(|| DecisionError::ExperimentNotFound(experiment_id.to_string()))?;
        
        let bandit = self.bandits.get(experiment_id);

        let total_requests: u64 = experiment.variants.iter()
            .filter_map(|v| v.results.as_ref())
            .map(|r| r.total_requests)
            .sum();

        let conversion_rates = experiment.variants.iter()
            .map(|v| {
                let rate = v.conversion_rate().unwrap_or(0.0);
                (v.id, rate)
            })
            .collect();

        let bandit_regret = bandit.as_ref().map(|b| b.calculate_regret());

        Ok(ExperimentStatistics {
            experiment_id: *experiment_id,
            status: experiment.status,
            total_requests,
            conversion_rates,
            bandit_regret,
            duration_seconds: (Utc::now() - experiment.start_time).num_seconds() as u64,
        })
    }
}

impl Default for ExperimentManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Experiment statistics
#[derive(Debug, Clone)]
pub struct ExperimentStatistics {
    pub experiment_id: Uuid,
    pub status: ExperimentStatus,
    pub total_requests: u64,
    pub conversion_rates: std::collections::HashMap<Uuid, f64>,
    pub bandit_regret: Option<f64>,
    pub duration_seconds: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_optimizer_types::models::ModelConfig;

    fn create_test_variants() -> Vec<Variant> {
        vec![
            Variant::new("control", ModelConfig::default(), 0.5),
            Variant::new("variant_a", ModelConfig::default(), 0.5),
        ]
    }

    #[test]
    fn test_create_experiment() {
        let manager = ExperimentManager::new();
        let variants = create_test_variants();
        
        let exp_id = manager.create_experiment("Test Experiment", variants, vec![]).unwrap();
        
        let experiment = manager.get_experiment(&exp_id).unwrap();
        assert_eq!(experiment.name, "Test Experiment");
        assert_eq!(experiment.variants.len(), 2);
        assert_eq!(experiment.status, ExperimentStatus::Draft);
    }

    #[test]
    fn test_invalid_variant_count() {
        let manager = ExperimentManager::new();
        let variants = vec![Variant::new("control", ModelConfig::default(), 1.0)];
        
        assert!(manager.create_experiment("Test", variants, vec![]).is_err());
    }

    #[test]
    fn test_invalid_traffic_allocation() {
        let manager = ExperimentManager::new();
        let variants = vec![
            Variant::new("control", ModelConfig::default(), 0.3),
            Variant::new("variant_a", ModelConfig::default(), 0.5),
        ];
        
        // Total is 0.8, should error
        assert!(manager.create_experiment("Test", variants, vec![]).is_err());
    }

    #[test]
    fn test_start_experiment() {
        let manager = ExperimentManager::new();
        let variants = create_test_variants();
        
        let exp_id = manager.create_experiment("Test", variants, vec![]).unwrap();
        manager.start_experiment(&exp_id).unwrap();
        
        let experiment = manager.get_experiment(&exp_id).unwrap();
        assert_eq!(experiment.status, ExperimentStatus::Running);
    }

    #[test]
    fn test_select_variant() {
        let manager = ExperimentManager::new();
        let variants = create_test_variants();
        
        let exp_id = manager.create_experiment("Test", variants, vec![]).unwrap();
        manager.start_experiment(&exp_id).unwrap();
        
        let variant_id = manager.select_variant(&exp_id).unwrap();
        assert!(variant_id != Uuid::nil());
    }

    #[test]
    fn test_record_result() {
        let manager = ExperimentManager::new();
        let variants = create_test_variants();
        
        let exp_id = manager.create_experiment("Test", variants, vec![]).unwrap();
        manager.start_experiment(&exp_id).unwrap();
        
        let variant_id = manager.select_variant(&exp_id).unwrap();
        
        manager.record_result(&exp_id, &variant_id, true, 0.9, 0.05, 1200.0).unwrap();
        
        let experiment = manager.get_experiment(&exp_id).unwrap();
        let variant = experiment.variants.iter().find(|v| v.id == variant_id).unwrap();
        
        assert!(variant.results.is_some());
        let results = variant.results.as_ref().unwrap();
        assert_eq!(results.total_requests, 1);
        assert_eq!(results.conversions, 1);
    }

    #[test]
    fn test_pause_resume() {
        let manager = ExperimentManager::new();
        let variants = create_test_variants();
        
        let exp_id = manager.create_experiment("Test", variants, vec![]).unwrap();
        manager.start_experiment(&exp_id).unwrap();
        manager.pause_experiment(&exp_id).unwrap();
        
        let experiment = manager.get_experiment(&exp_id).unwrap();
        assert_eq!(experiment.status, ExperimentStatus::Paused);
        
        manager.resume_experiment(&exp_id).unwrap();
        let experiment = manager.get_experiment(&exp_id).unwrap();
        assert_eq!(experiment.status, ExperimentStatus::Running);
    }

    #[test]
    fn test_should_conclude() {
        let manager = ExperimentManager::new();
        let variants = create_test_variants();
        
        let exp_id = manager.create_experiment("Test", variants, vec![]).unwrap();
        manager.start_experiment(&exp_id).unwrap();
        
        // Not enough data yet
        assert!(!manager.should_conclude(&exp_id, 100, 0.05).unwrap());
        
        // Add data to first variant (high conversion)
        let var1_id = manager.get_experiment(&exp_id).unwrap().variants[0].id;
        for _ in 0..100 {
            manager.record_result(&exp_id, &var1_id, true, 0.9, 0.05, 1000.0).unwrap();
        }
        
        // Add data to second variant (low conversion)
        let var2_id = manager.get_experiment(&exp_id).unwrap().variants[1].id;
        for _ in 0..30 {
            manager.record_result(&exp_id, &var2_id, true, 0.7, 0.05, 1000.0).unwrap();
        }
        for _ in 0..70 {
            manager.record_result(&exp_id, &var2_id, false, 0.5, 0.05, 1000.0).unwrap();
        }
        
        // Should conclude (significant difference)
        assert!(manager.should_conclude(&exp_id, 100, 0.05).unwrap());
    }

    #[test]
    fn test_get_statistics() {
        let manager = ExperimentManager::new();
        let variants = create_test_variants();
        
        let exp_id = manager.create_experiment("Test", variants, vec![]).unwrap();
        manager.start_experiment(&exp_id).unwrap();
        
        let stats = manager.get_statistics(&exp_id).unwrap();
        assert_eq!(stats.experiment_id, exp_id);
        assert_eq!(stats.status, ExperimentStatus::Running);
        assert_eq!(stats.total_requests, 0);
    }
}
