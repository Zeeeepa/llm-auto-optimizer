//! A/B testing engine with Thompson Sampling
//!
//! This module provides the main A/B testing engine that combines
//! Thompson Sampling, statistical significance testing, and experiment management.

use llm_optimizer_config::OptimizerConfig;
use llm_optimizer_types::{
    experiments::*,
    models::ModelConfig,
};
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
    errors::{DecisionError, Result},
    experiment_manager::{ExperimentManager, ExperimentStatistics},
    statistical::SampleSizeCalculator,
    variant_generator::{VariantGenerator, VariantStrategy},
};

/// A/B testing engine
pub struct ABTestEngine {
    /// Experiment manager
    manager: Arc<ExperimentManager>,
    
    /// Configuration
    min_sample_size: usize,
    significance_level: f64,
    max_duration_seconds: u64,
}

impl ABTestEngine {
    /// Create a new A/B testing engine
    pub fn new(config: &OptimizerConfig) -> Self {
        let ab_config = &config.strategies.ab_testing;
        
        Self {
            manager: Arc::new(ExperimentManager::new()),
            min_sample_size: ab_config.min_sample_size,
            significance_level: ab_config.significance_level,
            max_duration_seconds: ab_config.max_duration_seconds,
        }
    }

    /// Create a new experiment with generated variants
    pub fn create_experiment_from_strategy(
        &self,
        name: impl Into<String>,
        base_config: &ModelConfig,
        strategy: &VariantStrategy,
    ) -> Result<Uuid> {
        info!("Creating experiment with strategy: {:?}", strategy);
        
        // Generate variant configurations
        let configs = VariantGenerator::generate(base_config, strategy)?;
        
        if configs.len() < 2 {
            return Err(DecisionError::InvalidConfig(
                "Must have at least 2 variants".to_string()
            ));
        }
        
        // Validate all configurations
        for config in &configs {
            VariantGenerator::validate_config(config)?;
        }
        
        // Create variants with equal traffic allocation
        let allocation = 1.0 / configs.len() as f64;
        let variants: Vec<Variant> = configs.into_iter()
            .enumerate()
            .map(|(i, config)| {
                let name = if i == 0 {
                    "control".to_string()
                } else {
                    format!("variant_{}", i)
                };
                Variant::new(name, config, allocation)
            })
            .collect();
        
        // Create experiment
        let exp_id = self.manager.create_experiment(name, variants, vec![])?;
        
        info!("Created experiment {}", exp_id);
        
        Ok(exp_id)
    }

    /// Create a custom experiment with specific variants
    pub fn create_experiment(
        &self,
        name: impl Into<String>,
        variants: Vec<Variant>,
    ) -> Result<Uuid> {
        // Validate variants
        for variant in &variants {
            VariantGenerator::validate_config(&variant.config)?;
        }
        
        self.manager.create_experiment(name, variants, vec![])
    }

    /// Start an experiment
    pub fn start(&self, experiment_id: &Uuid) -> Result<()> {
        info!("Starting experiment {}", experiment_id);
        self.manager.start_experiment(experiment_id)
    }

    /// Pause an experiment
    pub fn pause(&self, experiment_id: &Uuid) -> Result<()> {
        info!("Pausing experiment {}", experiment_id);
        self.manager.pause_experiment(experiment_id)
    }

    /// Resume an experiment
    pub fn resume(&self, experiment_id: &Uuid) -> Result<()> {
        info!("Resuming experiment {}", experiment_id);
        self.manager.resume_experiment(experiment_id)
    }

    /// Assign a variant to a request
    pub fn assign_variant(&self, experiment_id: &Uuid) -> Result<(Uuid, ModelConfig)> {
        let variant_id = self.manager.select_variant(experiment_id)?;
        
        let experiment = self.manager.get_experiment(experiment_id)
            .ok_or_else(|| DecisionError::ExperimentNotFound(experiment_id.to_string()))?;
        
        let variant = experiment.variants.iter()
            .find(|v| v.id == variant_id)
            .ok_or_else(|| DecisionError::VariantNotFound(variant_id.to_string()))?;
        
        debug!("Assigned variant {} for experiment {}", variant.name, experiment_id);
        
        Ok((variant_id, variant.config.clone()))
    }

    /// Record the outcome of a request
    pub fn record_outcome(
        &self,
        experiment_id: &Uuid,
        variant_id: &Uuid,
        success: bool,
        quality: f64,
        cost: f64,
        latency_ms: f64,
    ) -> Result<()> {
        self.manager.record_result(
            experiment_id,
            variant_id,
            success,
            quality,
            cost,
            latency_ms,
        )?;
        
        debug!(
            "Recorded result for variant {} in experiment {}: success={}, quality={:.2}, cost={:.4}",
            variant_id, experiment_id, success, quality, cost
        );
        
        // Check if experiment should conclude
        self.check_experiment_conclusion(experiment_id)?;
        
        Ok(())
    }

    /// Check if an experiment should conclude
    fn check_experiment_conclusion(&self, experiment_id: &Uuid) -> Result<()> {
        let should_conclude = self.manager.should_conclude(
            experiment_id,
            self.min_sample_size,
            self.significance_level,
        )?;
        
        if should_conclude {
            info!("Experiment {} has reached statistical significance", experiment_id);
            self.conclude(experiment_id)?;
        } else {
            // Check if max duration exceeded
            let stats = self.manager.get_statistics(experiment_id)?;
            if stats.duration_seconds >= self.max_duration_seconds {
                warn!(
                    "Experiment {} exceeded max duration ({} seconds), concluding without significance",
                    experiment_id, self.max_duration_seconds
                );
                self.conclude(experiment_id)?;
            }
        }
        
        Ok(())
    }

    /// Manually conclude an experiment
    pub fn conclude(&self, experiment_id: &Uuid) -> Result<Experiment> {
        info!("Concluding experiment {}", experiment_id);
        
        self.manager.conclude_experiment(experiment_id, self.significance_level)?;
        
        let experiment = self.manager.get_experiment(experiment_id)
            .ok_or_else(|| DecisionError::ExperimentNotFound(experiment_id.to_string()))?;
        
        if let Some(results) = &experiment.results {
            if let Some(winner_id) = results.statistical_analysis.winner_variant_id {
                let winner = experiment.variants.iter()
                    .find(|v| v.id == winner_id)
                    .map(|v| &v.name);
                
                info!(
                    "Experiment {} concluded. Winner: {:?}, p-value: {:.4}, effect size: {:.4}",
                    experiment_id,
                    winner,
                    results.statistical_analysis.p_value,
                    results.statistical_analysis.effect_size
                );
            } else {
                info!(
                    "Experiment {} concluded with no significant winner (p-value: {:.4})",
                    experiment_id,
                    results.statistical_analysis.p_value
                );
            }
        }
        
        Ok(experiment)
    }

    /// Get experiment statistics
    pub fn get_statistics(&self, experiment_id: &Uuid) -> Result<ExperimentStatistics> {
        self.manager.get_statistics(experiment_id)
    }

    /// Get experiment details
    pub fn get_experiment(&self, experiment_id: &Uuid) -> Option<Experiment> {
        self.manager.get_experiment(experiment_id)
    }

    /// List all experiments
    pub fn list_experiments(&self) -> Vec<Experiment> {
        self.manager.list_experiments()
    }

    /// List active experiments
    pub fn list_active_experiments(&self) -> Vec<Experiment> {
        self.manager.list_active_experiments()
    }

    /// Calculate required sample size for an experiment
    pub fn calculate_sample_size(
        &self,
        baseline_rate: f64,
        min_effect: f64,
        power: f64,
    ) -> Result<usize> {
        let calculator = SampleSizeCalculator::new(
            baseline_rate,
            min_effect,
            power,
            self.significance_level,
        )?;
        
        calculator.calculate()
    }

    /// Get winning variant configuration (if experiment concluded)
    pub fn get_winner_config(&self, experiment_id: &Uuid) -> Result<Option<ModelConfig>> {
        let experiment = self.manager.get_experiment(experiment_id)
            .ok_or_else(|| DecisionError::ExperimentNotFound(experiment_id.to_string()))?;
        
        if experiment.status != ExperimentStatus::Completed {
            return Ok(None);
        }
        
        let winner = experiment.get_winner();
        Ok(winner.map(|v| v.config.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_optimizer_config::{ServiceConfig, DatabaseConfig, IntegrationConfig, ObservabilityConfig};
    use llm_optimizer_types::strategies::StrategyConfig;

    fn test_config() -> OptimizerConfig {
        OptimizerConfig {
            service: ServiceConfig::default(),
            database: DatabaseConfig::default(),
            integrations: IntegrationConfig::default(),
            strategies: StrategyConfig::default(),
            observability: ObservabilityConfig::default(),
        }
    }

    fn base_model_config() -> ModelConfig {
        ModelConfig::default()
    }

    #[test]
    fn test_create_engine() {
        let config = test_config();
        let engine = ABTestEngine::new(&config);
        
        assert_eq!(engine.min_sample_size, 1000);
        assert_eq!(engine.significance_level, 0.05);
    }

    #[test]
    fn test_create_experiment_from_strategy() {
        let config = test_config();
        let engine = ABTestEngine::new(&config);
        
        let base = base_model_config();
        let strategy = VariantStrategy::Temperature(vec![0.0, 0.7, 1.0]);
        
        let exp_id = engine.create_experiment_from_strategy(
            "Temperature Test",
            &base,
            &strategy,
        ).unwrap();
        
        let experiment = engine.get_experiment(&exp_id).unwrap();
        assert_eq!(experiment.variants.len(), 3);
        assert_eq!(experiment.name, "Temperature Test");
    }

    #[test]
    fn test_full_experiment_lifecycle() {
        let config = test_config();
        let engine = ABTestEngine::new(&config);
        
        // Create experiment
        let base = base_model_config();
        let strategy = VariantStrategy::Temperature(vec![0.3, 0.7]);
        
        let exp_id = engine.create_experiment_from_strategy(
            "Test",
            &base,
            &strategy,
        ).unwrap();
        
        // Start experiment
        engine.start(&exp_id).unwrap();
        
        // Assign variants and record results
        for i in 0..100 {
            let (variant_id, _config) = engine.assign_variant(&exp_id).unwrap();
            
            // Simulate: first variant has 80% success, second has 60%
            let variant_idx = engine.get_experiment(&exp_id).unwrap()
                .variants.iter()
                .position(|v| v.id == variant_id)
                .unwrap();
            
            let success = if variant_idx == 0 {
                i % 10 < 8
            } else {
                i % 10 < 6
            };
            
            engine.record_outcome(&exp_id, &variant_id, success, 0.9, 0.05, 1000.0).unwrap();
        }
        
        // Get statistics
        let stats = engine.get_statistics(&exp_id).unwrap();
        assert!(stats.total_requests > 0);
    }

    #[test]
    fn test_pause_resume() {
        let config = test_config();
        let engine = ABTestEngine::new(&config);
        
        let base = base_model_config();
        let strategy = VariantStrategy::Temperature(vec![0.3, 0.7]);
        
        let exp_id = engine.create_experiment_from_strategy("Test", &base, &strategy).unwrap();
        
        engine.start(&exp_id).unwrap();
        engine.pause(&exp_id).unwrap();
        
        let exp = engine.get_experiment(&exp_id).unwrap();
        assert_eq!(exp.status, ExperimentStatus::Paused);
        
        // Can't assign variant when paused
        assert!(engine.assign_variant(&exp_id).is_err());
        
        engine.resume(&exp_id).unwrap();
        
        // Can assign after resume
        assert!(engine.assign_variant(&exp_id).is_ok());
    }

    #[test]
    fn test_sample_size_calculation() {
        let config = test_config();
        let engine = ABTestEngine::new(&config);
        
        let sample_size = engine.calculate_sample_size(
            0.1,   // 10% baseline
            0.2,   // 20% relative improvement
            0.8,   // 80% power
        ).unwrap();
        
        assert!(sample_size > 100);
        assert!(sample_size < 100000);
    }

    #[test]
    fn test_list_experiments() {
        let config = test_config();
        let engine = ABTestEngine::new(&config);
        
        let base = base_model_config();
        let strategy = VariantStrategy::Temperature(vec![0.3, 0.7]);
        
        engine.create_experiment_from_strategy("Test 1", &base, &strategy).unwrap();
        engine.create_experiment_from_strategy("Test 2", &base, &strategy).unwrap();
        
        let experiments = engine.list_experiments();
        assert_eq!(experiments.len(), 2);
    }

    #[test]
    fn test_list_active_experiments() {
        let config = test_config();
        let engine = ABTestEngine::new(&config);
        
        let base = base_model_config();
        let strategy = VariantStrategy::Temperature(vec![0.3, 0.7]);
        
        let exp_id = engine.create_experiment_from_strategy("Test", &base, &strategy).unwrap();
        engine.start(&exp_id).unwrap();
        
        let active = engine.list_active_experiments();
        assert_eq!(active.len(), 1);
    }
}
