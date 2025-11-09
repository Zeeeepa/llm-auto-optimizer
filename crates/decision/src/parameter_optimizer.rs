//! Parameter Optimizer
//!
//! High-level API for adaptive parameter optimization integrating
//! contextual bandits, search strategies, and performance tracking.

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    adaptive_params::{AdaptiveParameterTuner, ParameterConfig, ParameterRange, ParameterStats},
    context::RequestContext,
    contextual_bandit::LinUCB,
    errors::{DecisionError, Result},
    parameter_search::{GridSearchConfig, ParameterSearchManager, SearchStrategy},
    reward::{RewardCalculator, RewardWeights, ResponseMetrics, UserFeedback},
};

/// Optimization mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptimizationMode {
    /// Explore parameter space (search phase)
    Explore,
    /// Exploit best known parameters (production phase)
    Exploit,
    /// Balanced exploration and exploitation
    Balanced,
}

/// Parameter optimization policy
#[derive(Debug, Clone)]
pub struct OptimizationPolicy {
    /// Policy name
    pub name: String,
    /// Parameter range
    pub range: ParameterRange,
    /// Optimization mode
    pub mode: OptimizationMode,
    /// Exploration rate (for balanced mode)
    pub exploration_rate: f64,
}

impl OptimizationPolicy {
    /// Create new optimization policy
    pub fn new(name: impl Into<String>, range: ParameterRange, mode: OptimizationMode) -> Self {
        Self {
            name: name.into(),
            range,
            mode,
            exploration_rate: 0.2,
        }
    }

    /// Set exploration rate
    pub fn with_exploration_rate(mut self, rate: f64) -> Self {
        self.exploration_rate = rate.clamp(0.0, 1.0);
        self
    }
}

/// Parameter optimizer engine
pub struct ParameterOptimizer {
    /// Adaptive tuners per policy
    tuners: Arc<DashMap<String, AdaptiveParameterTuner>>,
    /// Contextual bandits for parameter selection
    bandits: Arc<DashMap<String, LinUCB>>,
    /// Optimization policies
    policies: Arc<DashMap<String, OptimizationPolicy>>,
    /// Reward calculator
    reward_calculator: RewardCalculator,
    /// Feature dimension for contextual bandits
    feature_dimension: usize,
    /// LinUCB exploration parameter
    alpha: f64,
}

impl ParameterOptimizer {
    /// Create new parameter optimizer
    pub fn new(reward_weights: RewardWeights, alpha: f64) -> Self {
        let feature_dim = RequestContext::feature_dimension();

        Self {
            tuners: Arc::new(DashMap::new()),
            bandits: Arc::new(DashMap::new()),
            policies: Arc::new(DashMap::new()),
            reward_calculator: RewardCalculator::new(reward_weights, 1.0, 5000.0),
            feature_dimension: feature_dim,
            alpha,
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(RewardWeights::default_weights(), 1.0)
    }

    /// Create new optimization policy
    pub fn create_policy(&self, policy: OptimizationPolicy) -> Result<()> {
        let policy_name = policy.name.clone();

        // Create adaptive tuner for this policy
        let tuner = AdaptiveParameterTuner::new(policy.range.clone());
        self.tuners.insert(policy_name.clone(), tuner);

        // Create contextual bandit for parameter selection
        let bandit = LinUCB::new(self.alpha, self.feature_dimension);
        self.bandits.insert(policy_name.clone(), bandit);

        // Store policy
        self.policies.insert(policy_name, policy);

        Ok(())
    }

    /// Initialize policy with search strategy
    pub fn initialize_with_search(
        &self,
        policy_name: &str,
        strategy: SearchStrategy,
        num_configs: usize,
    ) -> Result<Vec<Uuid>> {
        let policy = self
            .policies
            .get(policy_name)
            .ok_or_else(|| DecisionError::ExperimentNotFound(policy_name.to_string()))?;

        let mut tuner = self
            .tuners
            .get_mut(policy_name)
            .ok_or_else(|| DecisionError::ExperimentNotFound(policy_name.to_string()))?;

        let mut bandit = self
            .bandits
            .get_mut(policy_name)
            .ok_or_else(|| DecisionError::ExperimentNotFound(policy_name.to_string()))?;

        // Generate configurations based on strategy
        let configs = match strategy {
            SearchStrategy::Grid => {
                let grid_config = GridSearchConfig {
                    temp_steps: (num_configs as f64).cbrt().ceil() as usize,
                    top_p_steps: (num_configs as f64).cbrt().ceil() as usize,
                    max_tokens_steps: (num_configs as f64).cbrt().ceil() as usize,
                };
                let search = ParameterSearchManager::with_grid_search(policy.range.clone(), grid_config);
                search.grid_search.map(|s| s.all_configs()).unwrap_or_default()
            }
            SearchStrategy::Random => {
                let mut search = ParameterSearchManager::with_random_search(policy.range.clone(), num_configs);
                let mut configs = Vec::new();
                while let Some(config) = search.next() {
                    configs.push(config);
                }
                configs
            }
            SearchStrategy::LatinHypercube => {
                let search = ParameterSearchManager::with_lhs(policy.range.clone(), num_configs);
                search.lhs_search.map(|s| s.all_configs()).unwrap_or_default()
            }
            SearchStrategy::Sobol => {
                return Err(DecisionError::InvalidParameter(
                    "Sobol sequence not yet implemented".to_string(),
                ));
            }
        };

        // Register all configurations
        let mut config_ids = Vec::new();
        for config in configs {
            let config_id = tuner.register_config(config)?;
            bandit.add_arm(config_id);
            config_ids.push(config_id);
        }

        Ok(config_ids)
    }

    /// Select parameter configuration for request
    pub fn select_parameters(
        &self,
        policy_name: &str,
        context: &RequestContext,
    ) -> Result<(Uuid, ParameterConfig)> {
        let policy = self
            .policies
            .get(policy_name)
            .ok_or_else(|| DecisionError::ExperimentNotFound(policy_name.to_string()))?;

        match policy.mode {
            OptimizationMode::Explore => self.select_explore(policy_name, context),
            OptimizationMode::Exploit => self.select_exploit(policy_name, context),
            OptimizationMode::Balanced => {
                // Randomly choose between explore and exploit based on rate
                if rand::random::<f64>() < policy.exploration_rate {
                    self.select_explore(policy_name, context)
                } else {
                    self.select_exploit(policy_name, context)
                }
            }
        }
    }

    /// Select using exploration (contextual bandit)
    fn select_explore(
        &self,
        policy_name: &str,
        context: &RequestContext,
    ) -> Result<(Uuid, ParameterConfig)> {
        let bandit = self
            .bandits
            .get(policy_name)
            .ok_or_else(|| DecisionError::ExperimentNotFound(policy_name.to_string()))?;

        let tuner = self
            .tuners
            .get(policy_name)
            .ok_or_else(|| DecisionError::ExperimentNotFound(policy_name.to_string()))?;

        let config_id = bandit.select_arm(context)?;
        let stats = tuner
            .get_stats(&config_id)
            .ok_or_else(|| DecisionError::VariantNotFound(config_id.to_string()))?;

        Ok((config_id, stats.config.clone()))
    }

    /// Select using exploitation (best known)
    fn select_exploit(
        &self,
        policy_name: &str,
        context: &RequestContext,
    ) -> Result<(Uuid, ParameterConfig)> {
        let tuner = self
            .tuners
            .get(policy_name)
            .ok_or_else(|| DecisionError::ExperimentNotFound(policy_name.to_string()))?;

        // Use task-specific best if available
        if let Some(task_type) = &context.task_type {
            if let Some((config_id, config)) = tuner.get_best_for_task(task_type) {
                return Ok((config_id, config));
            }
        }

        // Fall back to overall best
        let all_stats = tuner.get_all_stats();
        let best = all_stats
            .iter()
            .max_by(|a, b| {
                a.average_reward
                    .partial_cmp(&b.average_reward)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .ok_or_else(|| DecisionError::InvalidState("No configurations available".to_string()))?;

        Ok((best.config_id, best.config.clone()))
    }

    /// Update with performance feedback
    pub fn update_performance(
        &self,
        policy_name: &str,
        config_id: &Uuid,
        context: &RequestContext,
        metrics: &ResponseMetrics,
        feedback: Option<&UserFeedback>,
    ) -> Result<()> {
        // Calculate reward
        let reward = if let Some(fb) = feedback {
            self.reward_calculator.calculate_reward(metrics, fb)
        } else {
            self.reward_calculator.calculate_reward_metrics_only(metrics)
        };

        // Update adaptive tuner
        let mut tuner = self
            .tuners
            .get_mut(policy_name)
            .ok_or_else(|| DecisionError::ExperimentNotFound(policy_name.to_string()))?;

        tuner.update_config(config_id, reward, metrics, feedback)?;

        // Update contextual bandit
        let mut bandit = self
            .bandits
            .get_mut(policy_name)
            .ok_or_else(|| DecisionError::ExperimentNotFound(policy_name.to_string()))?;

        bandit.update(config_id, context, reward)?;

        Ok(())
    }

    /// Get performance statistics
    pub fn get_performance_stats(&self, policy_name: &str) -> Result<Vec<ParameterStats>> {
        let tuner = self
            .tuners
            .get(policy_name)
            .ok_or_else(|| DecisionError::ExperimentNotFound(policy_name.to_string()))?;

        Ok(tuner.get_all_stats())
    }

    /// Get best configuration for task type
    pub fn get_best_for_task(
        &self,
        policy_name: &str,
        task_type: &str,
    ) -> Result<Option<(Uuid, ParameterConfig)>> {
        let tuner = self
            .tuners
            .get(policy_name)
            .ok_or_else(|| DecisionError::ExperimentNotFound(policy_name.to_string()))?;

        Ok(tuner.get_best_for_task(task_type))
    }

    /// Update task-specific best configurations
    pub fn update_task_bests(&self, policy_name: &str, task_types: &[String]) -> Result<()> {
        let mut tuner = self
            .tuners
            .get_mut(policy_name)
            .ok_or_else(|| DecisionError::ExperimentNotFound(policy_name.to_string()))?;

        for task_type in task_types {
            tuner.update_task_best(task_type.clone());
        }

        Ok(())
    }

    /// Change optimization mode
    pub fn set_mode(&self, policy_name: &str, mode: OptimizationMode) -> Result<()> {
        let mut policy = self
            .policies
            .get_mut(policy_name)
            .ok_or_else(|| DecisionError::ExperimentNotFound(policy_name.to_string()))?;

        policy.mode = mode;
        Ok(())
    }

    /// Get current optimization mode
    pub fn get_mode(&self, policy_name: &str) -> Result<OptimizationMode> {
        let policy = self
            .policies
            .get(policy_name)
            .ok_or_else(|| DecisionError::ExperimentNotFound(policy_name.to_string()))?;

        Ok(policy.mode)
    }

    /// Update reward weights
    pub fn set_reward_weights(&mut self, weights: RewardWeights) {
        self.reward_calculator.set_weights(weights);
    }

    /// Get number of policies
    pub fn num_policies(&self) -> usize {
        self.policies.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimizer_creation() {
        let optimizer = ParameterOptimizer::with_defaults();
        assert_eq!(optimizer.num_policies(), 0);
    }

    #[test]
    fn test_create_policy() {
        let optimizer = ParameterOptimizer::with_defaults();
        let policy = OptimizationPolicy::new(
            "test_policy",
            ParameterRange::default(),
            OptimizationMode::Balanced,
        );

        optimizer.create_policy(policy).unwrap();
        assert_eq!(optimizer.num_policies(), 1);
    }

    #[test]
    fn test_initialize_with_grid_search() {
        let optimizer = ParameterOptimizer::with_defaults();
        let policy = OptimizationPolicy::new(
            "test_policy",
            ParameterRange::default(),
            OptimizationMode::Explore,
        );

        optimizer.create_policy(policy).unwrap();
        let config_ids = optimizer
            .initialize_with_search("test_policy", SearchStrategy::Grid, 8)
            .unwrap();

        assert!(!config_ids.is_empty());
    }

    #[test]
    fn test_initialize_with_random_search() {
        let optimizer = ParameterOptimizer::with_defaults();
        let policy = OptimizationPolicy::new(
            "test_policy",
            ParameterRange::default(),
            OptimizationMode::Explore,
        );

        optimizer.create_policy(policy).unwrap();
        let config_ids = optimizer
            .initialize_with_search("test_policy", SearchStrategy::Random, 10)
            .unwrap();

        assert_eq!(config_ids.len(), 10);
    }

    #[test]
    fn test_initialize_with_lhs() {
        let optimizer = ParameterOptimizer::with_defaults();
        let policy = OptimizationPolicy::new(
            "test_policy",
            ParameterRange::default(),
            OptimizationMode::Explore,
        );

        optimizer.create_policy(policy).unwrap();
        let config_ids = optimizer
            .initialize_with_search("test_policy", SearchStrategy::LatinHypercube, 15)
            .unwrap();

        assert!(!config_ids.is_empty());
    }

    #[test]
    fn test_select_parameters_explore() {
        let optimizer = ParameterOptimizer::with_defaults();
        let policy = OptimizationPolicy::new(
            "test_policy",
            ParameterRange::default(),
            OptimizationMode::Explore,
        );

        optimizer.create_policy(policy).unwrap();
        optimizer
            .initialize_with_search("test_policy", SearchStrategy::Random, 5)
            .unwrap();

        let context = RequestContext::new(100);
        let (config_id, config) = optimizer.select_parameters("test_policy", &context).unwrap();

        assert!(config_id != Uuid::nil());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_select_parameters_balanced() {
        let optimizer = ParameterOptimizer::with_defaults();
        let policy = OptimizationPolicy::new(
            "test_policy",
            ParameterRange::default(),
            OptimizationMode::Balanced,
        );

        optimizer.create_policy(policy).unwrap();
        optimizer
            .initialize_with_search("test_policy", SearchStrategy::Random, 5)
            .unwrap();

        let context = RequestContext::new(100);
        let (_, config) = optimizer.select_parameters("test_policy", &context).unwrap();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_update_performance() {
        let optimizer = ParameterOptimizer::with_defaults();
        let policy = OptimizationPolicy::new(
            "test_policy",
            ParameterRange::default(),
            OptimizationMode::Explore,
        );

        optimizer.create_policy(policy).unwrap();
        let config_ids = optimizer
            .initialize_with_search("test_policy", SearchStrategy::Random, 3)
            .unwrap();

        let context = RequestContext::new(100);
        let metrics = ResponseMetrics {
            quality_score: 0.9,
            cost: 0.1,
            latency_ms: 1000.0,
            token_count: 500,
        };

        optimizer
            .update_performance("test_policy", &config_ids[0], &context, &metrics, None)
            .unwrap();

        let stats = optimizer.get_performance_stats("test_policy").unwrap();
        let updated = stats.iter().find(|s| s.config_id == config_ids[0]).unwrap();
        assert_eq!(updated.num_uses, 1);
    }

    #[test]
    fn test_optimizer_learning() {
        let optimizer = ParameterOptimizer::with_defaults();
        let policy = OptimizationPolicy::new(
            "test_policy",
            ParameterRange::default(),
            OptimizationMode::Explore,
        );

        optimizer.create_policy(policy).unwrap();
        let config_ids = optimizer
            .initialize_with_search("test_policy", SearchStrategy::Random, 3)
            .unwrap();

        let good_id = config_ids[0];
        let bad_id = config_ids[1];

        let context = RequestContext::new(100);
        let good_metrics = ResponseMetrics {
            quality_score: 0.95,
            cost: 0.05,
            latency_ms: 800.0,
            token_count: 400,
        };

        let bad_metrics = ResponseMetrics {
            quality_score: 0.4,
            cost: 0.3,
            latency_ms: 2000.0,
            token_count: 800,
        };

        // Train with clear difference
        for _ in 0..20 {
            optimizer
                .update_performance("test_policy", &good_id, &context, &good_metrics, None)
                .unwrap();
            optimizer
                .update_performance("test_policy", &bad_id, &context, &bad_metrics, None)
                .unwrap();
        }

        let stats = optimizer.get_performance_stats("test_policy").unwrap();
        let good_stats = stats.iter().find(|s| s.config_id == good_id).unwrap();
        let bad_stats = stats.iter().find(|s| s.config_id == bad_id).unwrap();

        assert!(good_stats.average_reward > bad_stats.average_reward);
    }

    #[test]
    fn test_get_best_for_task() {
        let optimizer = ParameterOptimizer::with_defaults();
        let range = ParameterRange::for_task_type("code");
        let policy = OptimizationPolicy::new("code_policy", range, OptimizationMode::Explore);

        optimizer.create_policy(policy).unwrap();
        optimizer
            .initialize_with_search("code_policy", SearchStrategy::Random, 5)
            .unwrap();

        let context = RequestContext::new(100).with_task_type("code");
        let (config_id, _) = optimizer.select_parameters("code_policy", &context).unwrap();

        let metrics = ResponseMetrics {
            quality_score: 0.95,
            cost: 0.1,
            latency_ms: 1000.0,
            token_count: 500,
        };

        // Need enough samples for best selection
        for _ in 0..15 {
            optimizer
                .update_performance("code_policy", &config_id, &context, &metrics, None)
                .unwrap();
        }

        optimizer
            .update_task_bests("code_policy", &["code".to_string()])
            .unwrap();

        let best = optimizer.get_best_for_task("code_policy", "code").unwrap();
        assert!(best.is_some());
    }

    #[test]
    fn test_set_mode() {
        let optimizer = ParameterOptimizer::with_defaults();
        let policy = OptimizationPolicy::new(
            "test_policy",
            ParameterRange::default(),
            OptimizationMode::Explore,
        );

        optimizer.create_policy(policy).unwrap();
        assert_eq!(
            optimizer.get_mode("test_policy").unwrap(),
            OptimizationMode::Explore
        );

        optimizer
            .set_mode("test_policy", OptimizationMode::Exploit)
            .unwrap();
        assert_eq!(
            optimizer.get_mode("test_policy").unwrap(),
            OptimizationMode::Exploit
        );
    }

    #[test]
    fn test_policy_with_exploration_rate() {
        let policy = OptimizationPolicy::new(
            "test",
            ParameterRange::default(),
            OptimizationMode::Balanced,
        )
        .with_exploration_rate(0.3);

        assert_eq!(policy.exploration_rate, 0.3);
    }

    #[test]
    fn test_exploration_rate_clamping() {
        let policy = OptimizationPolicy::new(
            "test",
            ParameterRange::default(),
            OptimizationMode::Balanced,
        )
        .with_exploration_rate(1.5);

        assert_eq!(policy.exploration_rate, 1.0);
    }
}
