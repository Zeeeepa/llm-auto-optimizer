//! Reinforcement Feedback Engine
//!
//! This module provides the main API for reinforcement learning-based
//! optimization using contextual bandits with user feedback.

use std::sync::Arc;
use dashmap::DashMap;
use llm_optimizer_types::models::ModelConfig;
use uuid::Uuid;

use crate::{
    context::RequestContext,
    contextual_bandit::{ContextualThompson, LinUCB},
    errors::{DecisionError, Result},
    reward::{RewardCalculator, RewardWeights, ResponseMetrics, UserFeedback},
};

/// Algorithm type for contextual bandits
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BanditAlgorithm {
    /// Linear Upper Confidence Bound
    LinUCB,
    /// Contextual Thompson Sampling
    ContextualThompson,
}

/// Reinforcement feedback engine
pub struct ReinforcementEngine {
    /// Algorithm to use
    algorithm: BanditAlgorithm,
    /// LinUCB instance (if using LinUCB)
    linucb: Option<Arc<DashMap<String, LinUCB>>>,
    /// Contextual Thompson instance (if using ContextualThompson)
    contextual_thompson: Option<Arc<DashMap<String, ContextualThompson>>>,
    /// Reward calculator
    reward_calculator: RewardCalculator,
    /// Variant configurations
    variant_configs: Arc<DashMap<Uuid, ModelConfig>>,
    /// Feature dimension
    feature_dimension: usize,
    /// LinUCB exploration parameter (alpha)
    alpha: f64,
}

impl ReinforcementEngine {
    /// Create a new reinforcement engine with LinUCB
    pub fn with_linucb(alpha: f64, reward_weights: RewardWeights) -> Self {
        let feature_dim = RequestContext::feature_dimension();

        Self {
            algorithm: BanditAlgorithm::LinUCB,
            linucb: Some(Arc::new(DashMap::new())),
            contextual_thompson: None,
            reward_calculator: RewardCalculator::new(reward_weights, 1.0, 5000.0),
            variant_configs: Arc::new(DashMap::new()),
            feature_dimension: feature_dim,
            alpha,
        }
    }

    /// Create a new reinforcement engine with Contextual Thompson Sampling
    pub fn with_contextual_thompson(reward_weights: RewardWeights) -> Self {
        let feature_dim = RequestContext::feature_dimension();

        Self {
            algorithm: BanditAlgorithm::ContextualThompson,
            linucb: None,
            contextual_thompson: Some(Arc::new(DashMap::new())),
            reward_calculator: RewardCalculator::new(reward_weights, 1.0, 5000.0),
            variant_configs: Arc::new(DashMap::new()),
            feature_dimension: feature_dim,
            alpha: 0.0, // Not used for Thompson Sampling
        }
    }

    /// Create a new experiment/policy
    pub fn create_policy(
        &self,
        policy_name: impl Into<String>,
        variants: Vec<(Uuid, ModelConfig)>,
    ) -> Result<()> {
        let name = policy_name.into();

        // Store variant configurations
        for (variant_id, config) in &variants {
            self.variant_configs.insert(*variant_id, config.clone());
        }

        // Initialize bandit for this policy
        match self.algorithm {
            BanditAlgorithm::LinUCB => {
                let mut bandit = LinUCB::new(self.alpha, self.feature_dimension);
                for (variant_id, _) in variants {
                    bandit.add_arm(variant_id);
                }
                self.linucb
                    .as_ref()
                    .ok_or_else(|| DecisionError::InvalidState("LinUCB not initialized".to_string()))?
                    .insert(name, bandit);
            }
            BanditAlgorithm::ContextualThompson => {
                let mut bandit = ContextualThompson::new(self.feature_dimension);
                for (variant_id, _) in variants {
                    bandit.add_arm(variant_id);
                }
                self.contextual_thompson
                    .as_ref()
                    .ok_or_else(|| {
                        DecisionError::InvalidState("ContextualThompson not initialized".to_string())
                    })?
                    .insert(name, bandit);
            }
        }

        Ok(())
    }

    /// Select best variant for given context
    pub fn select_variant(
        &self,
        policy_name: &str,
        context: &RequestContext,
    ) -> Result<(Uuid, ModelConfig)> {
        let variant_id = match self.algorithm {
            BanditAlgorithm::LinUCB => {
                let linucb = self
                    .linucb
                    .as_ref()
                    .ok_or_else(|| DecisionError::InvalidState("LinUCB not initialized".to_string()))?;

                let bandit = linucb
                    .get(policy_name)
                    .ok_or_else(|| DecisionError::ExperimentNotFound(policy_name.to_string()))?;

                bandit.select_arm(context)?
            }
            BanditAlgorithm::ContextualThompson => {
                let ct = self.contextual_thompson.as_ref().ok_or_else(|| {
                    DecisionError::InvalidState("ContextualThompson not initialized".to_string())
                })?;

                let bandit = ct
                    .get(policy_name)
                    .ok_or_else(|| DecisionError::ExperimentNotFound(policy_name.to_string()))?;

                bandit.select_arm(context)?
            }
        };

        // Get variant configuration
        let config = self
            .variant_configs
            .get(&variant_id)
            .ok_or_else(|| DecisionError::VariantNotFound(variant_id.to_string()))?
            .clone();

        Ok((variant_id, config))
    }

    /// Update with observed reward from metrics only
    pub fn update_from_metrics(
        &self,
        policy_name: &str,
        variant_id: &Uuid,
        context: &RequestContext,
        metrics: &ResponseMetrics,
    ) -> Result<()> {
        let reward = self.reward_calculator.calculate_reward_metrics_only(metrics);
        self.update_reward(policy_name, variant_id, context, reward)
    }

    /// Update with observed reward from metrics and user feedback
    pub fn update_from_feedback(
        &self,
        policy_name: &str,
        variant_id: &Uuid,
        context: &RequestContext,
        metrics: &ResponseMetrics,
        feedback: &UserFeedback,
    ) -> Result<()> {
        let reward = self.reward_calculator.calculate_reward(metrics, feedback);
        self.update_reward(policy_name, variant_id, context, reward)
    }

    /// Internal: update with computed reward
    fn update_reward(
        &self,
        policy_name: &str,
        variant_id: &Uuid,
        context: &RequestContext,
        reward: f64,
    ) -> Result<()> {
        match self.algorithm {
            BanditAlgorithm::LinUCB => {
                let linucb = self
                    .linucb
                    .as_ref()
                    .ok_or_else(|| DecisionError::InvalidState("LinUCB not initialized".to_string()))?;

                let mut bandit = linucb
                    .get_mut(policy_name)
                    .ok_or_else(|| DecisionError::ExperimentNotFound(policy_name.to_string()))?;

                bandit.update(variant_id, context, reward)?;
            }
            BanditAlgorithm::ContextualThompson => {
                let ct = self.contextual_thompson.as_ref().ok_or_else(|| {
                    DecisionError::InvalidState("ContextualThompson not initialized".to_string())
                })?;

                let mut bandit = ct
                    .get_mut(policy_name)
                    .ok_or_else(|| DecisionError::ExperimentNotFound(policy_name.to_string()))?;

                bandit.update(variant_id, context, reward)?;
            }
        }

        Ok(())
    }

    /// Get current performance statistics for all variants in a policy
    pub fn get_policy_stats(&self, policy_name: &str) -> Result<Vec<VariantStats>> {
        match self.algorithm {
            BanditAlgorithm::LinUCB => {
                let linucb = self
                    .linucb
                    .as_ref()
                    .ok_or_else(|| DecisionError::InvalidState("LinUCB not initialized".to_string()))?;

                let bandit = linucb
                    .get(policy_name)
                    .ok_or_else(|| DecisionError::ExperimentNotFound(policy_name.to_string()))?;

                let rewards = bandit.get_average_rewards();

                Ok(rewards
                    .iter()
                    .map(|(id, reward)| {
                        let arm = bandit.get_arm(id).unwrap();
                        VariantStats {
                            variant_id: *id,
                            average_reward: *reward,
                            num_selections: arm.num_selections,
                            total_reward: arm.total_reward,
                        }
                    })
                    .collect())
            }
            BanditAlgorithm::ContextualThompson => {
                let ct = self.contextual_thompson.as_ref().ok_or_else(|| {
                    DecisionError::InvalidState("ContextualThompson not initialized".to_string())
                })?;

                let bandit = ct
                    .get(policy_name)
                    .ok_or_else(|| DecisionError::ExperimentNotFound(policy_name.to_string()))?;

                let rewards = bandit.get_average_rewards();

                Ok(rewards
                    .iter()
                    .map(|(id, reward)| {
                        let arm = bandit.get_arm(id).unwrap();
                        VariantStats {
                            variant_id: *id,
                            average_reward: *reward,
                            num_selections: arm.num_selections,
                            total_reward: arm.total_reward,
                        }
                    })
                    .collect())
            }
        }
    }

    /// Get algorithm type
    pub fn algorithm(&self) -> BanditAlgorithm {
        self.algorithm
    }

    /// Update reward calculator weights
    pub fn set_reward_weights(&mut self, weights: RewardWeights) {
        self.reward_calculator.set_weights(weights);
    }
}

/// Variant statistics
#[derive(Debug, Clone)]
pub struct VariantStats {
    pub variant_id: Uuid,
    pub average_reward: f64,
    pub num_selections: u64,
    pub total_reward: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::OutputLengthCategory;

    fn test_variants() -> Vec<(Uuid, ModelConfig)> {
        vec![
            (Uuid::new_v4(), ModelConfig::default()),
            (Uuid::new_v4(), ModelConfig::default()),
        ]
    }

    #[test]
    fn test_create_engine_linucb() {
        let engine = ReinforcementEngine::with_linucb(1.0, RewardWeights::default_weights());
        assert_eq!(engine.algorithm(), BanditAlgorithm::LinUCB);
    }

    #[test]
    fn test_create_engine_contextual_thompson() {
        let engine =
            ReinforcementEngine::with_contextual_thompson(RewardWeights::default_weights());
        assert_eq!(engine.algorithm(), BanditAlgorithm::ContextualThompson);
    }

    #[test]
    fn test_create_policy_linucb() {
        let engine = ReinforcementEngine::with_linucb(1.0, RewardWeights::default_weights());
        let variants = test_variants();

        engine.create_policy("test_policy", variants).unwrap();
    }

    #[test]
    fn test_create_policy_contextual_thompson() {
        let engine =
            ReinforcementEngine::with_contextual_thompson(RewardWeights::default_weights());
        let variants = test_variants();

        engine.create_policy("test_policy", variants).unwrap();
    }

    #[test]
    fn test_select_variant_linucb() {
        let engine = ReinforcementEngine::with_linucb(1.0, RewardWeights::default_weights());
        let variants = test_variants();

        engine.create_policy("test_policy", variants).unwrap();

        let context = RequestContext::new(100)
            .with_task_type("generation")
            .with_output_length(OutputLengthCategory::Medium);

        let (variant_id, _config) = engine.select_variant("test_policy", &context).unwrap();
        assert!(variant_id != Uuid::nil());
    }

    #[test]
    fn test_select_variant_contextual_thompson() {
        let engine =
            ReinforcementEngine::with_contextual_thompson(RewardWeights::default_weights());
        let variants = test_variants();

        engine.create_policy("test_policy", variants).unwrap();

        let context = RequestContext::new(100);
        let (variant_id, _config) = engine.select_variant("test_policy", &context).unwrap();
        assert!(variant_id != Uuid::nil());
    }

    #[test]
    fn test_update_from_metrics_linucb() {
        let engine = ReinforcementEngine::with_linucb(1.0, RewardWeights::default_weights());
        let variants = test_variants();
        let variant_id = variants[0].0;

        engine
            .create_policy("test_policy", variants.clone())
            .unwrap();

        let context = RequestContext::new(100);
        let metrics = ResponseMetrics {
            quality_score: 0.9,
            cost: 0.1,
            latency_ms: 1000.0,
            token_count: 500,
        };

        engine
            .update_from_metrics("test_policy", &variant_id, &context, &metrics)
            .unwrap();
    }

    #[test]
    fn test_update_from_feedback() {
        let engine =
            ReinforcementEngine::with_contextual_thompson(RewardWeights::default_weights());
        let variants = test_variants();
        let variant_id = variants[0].0;

        engine.create_policy("test_policy", variants).unwrap();

        let context = RequestContext::new(100);
        let metrics = ResponseMetrics {
            quality_score: 0.8,
            cost: 0.2,
            latency_ms: 1500.0,
            token_count: 600,
        };

        let mut feedback = UserFeedback::new();
        feedback.task_completed = true;
        feedback.explicit_rating = Some(4.0);

        engine
            .update_from_feedback("test_policy", &variant_id, &context, &metrics, &feedback)
            .unwrap();
    }

    #[test]
    fn test_get_policy_stats() {
        let engine = ReinforcementEngine::with_linucb(1.0, RewardWeights::default_weights());
        let variants = test_variants();

        engine.create_policy("test_policy", variants).unwrap();

        let stats = engine.get_policy_stats("test_policy").unwrap();
        assert_eq!(stats.len(), 2);
    }

    #[test]
    fn test_learning_convergence() {
        let engine = ReinforcementEngine::with_linucb(0.5, RewardWeights::default_weights());
        let variants = test_variants();
        let good_variant = variants[0].0;
        let bad_variant = variants[1].0;

        engine
            .create_policy("test_policy", variants.clone())
            .unwrap();

        // Simulate: good variant gets high rewards, bad variant gets low rewards
        for _ in 0..50 {
            let context = RequestContext::new(100);

            let good_metrics = ResponseMetrics {
                quality_score: 0.9,
                cost: 0.1,
                latency_ms: 1000.0,
                token_count: 500,
            };

            let bad_metrics = ResponseMetrics {
                quality_score: 0.3,
                cost: 0.5,
                latency_ms: 3000.0,
                token_count: 800,
            };

            engine
                .update_from_metrics("test_policy", &good_variant, &context, &good_metrics)
                .unwrap();
            engine
                .update_from_metrics("test_policy", &bad_variant, &context, &bad_metrics)
                .unwrap();
        }

        let stats = engine.get_policy_stats("test_policy").unwrap();
        let good_stats = stats.iter().find(|s| s.variant_id == good_variant).unwrap();
        let bad_stats = stats.iter().find(|s| s.variant_id == bad_variant).unwrap();

        // Good variant should have higher average reward
        assert!(good_stats.average_reward > bad_stats.average_reward);
    }

    #[test]
    fn test_set_reward_weights() {
        let mut engine = ReinforcementEngine::with_linucb(1.0, RewardWeights::default_weights());

        let new_weights = RewardWeights::cost_focused();
        engine.set_reward_weights(new_weights);
    }
}
