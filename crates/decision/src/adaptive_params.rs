//! Adaptive Parameter Tuning for LLM configurations
//!
//! This module provides adaptive tuning for temperature, top-p, and max tokens
//! based on task context, historical performance, and user feedback.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    context::RequestContext,
    errors::{DecisionError, Result},
    reward::{ResponseMetrics, UserFeedback},
};

/// Parameter configuration for LLM generation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParameterConfig {
    /// Sampling temperature (0.0 - 2.0)
    pub temperature: f64,
    /// Nucleus sampling threshold (0.0 - 1.0)
    pub top_p: f64,
    /// Maximum output tokens
    pub max_tokens: usize,
}

impl Default for ParameterConfig {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            top_p: 0.9,
            max_tokens: 2048,
        }
    }
}

impl ParameterConfig {
    /// Create new parameter configuration
    pub fn new(temperature: f64, top_p: f64, max_tokens: usize) -> Result<Self> {
        let config = Self {
            temperature,
            top_p,
            max_tokens,
        };
        config.validate()?;
        Ok(config)
    }

    /// Validate parameter ranges
    pub fn validate(&self) -> Result<()> {
        if self.temperature < 0.0 || self.temperature > 2.0 {
            return Err(DecisionError::InvalidParameter(format!(
                "Temperature {} out of range [0.0, 2.0]",
                self.temperature
            )));
        }

        if self.top_p < 0.0 || self.top_p > 1.0 {
            return Err(DecisionError::InvalidParameter(format!(
                "Top-p {} out of range [0.0, 1.0]",
                self.top_p
            )));
        }

        if self.max_tokens == 0 {
            return Err(DecisionError::InvalidParameter(
                "Max tokens must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }

    /// Create configuration for creative tasks
    pub fn creative() -> Self {
        Self {
            temperature: 1.2,
            top_p: 0.95,
            max_tokens: 2048,
        }
    }

    /// Create configuration for analytical tasks
    pub fn analytical() -> Self {
        Self {
            temperature: 0.3,
            top_p: 0.85,
            max_tokens: 1024,
        }
    }

    /// Create configuration for code generation
    pub fn code_generation() -> Self {
        Self {
            temperature: 0.2,
            top_p: 0.9,
            max_tokens: 2048,
        }
    }

    /// Create configuration for balanced general use
    pub fn balanced() -> Self {
        Self::default()
    }
}

/// Parameter range constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterRange {
    /// Minimum temperature
    pub temp_min: f64,
    /// Maximum temperature
    pub temp_max: f64,
    /// Minimum top-p
    pub top_p_min: f64,
    /// Maximum top-p
    pub top_p_max: f64,
    /// Minimum max tokens
    pub max_tokens_min: usize,
    /// Maximum max tokens
    pub max_tokens_max: usize,
}

impl Default for ParameterRange {
    fn default() -> Self {
        Self {
            temp_min: 0.0,
            temp_max: 2.0,
            top_p_min: 0.7,
            top_p_max: 1.0,
            max_tokens_min: 256,
            max_tokens_max: 8192,
        }
    }
}

impl ParameterRange {
    /// Create new parameter range
    pub fn new(
        temp_min: f64,
        temp_max: f64,
        top_p_min: f64,
        top_p_max: f64,
        max_tokens_min: usize,
        max_tokens_max: usize,
    ) -> Result<Self> {
        if temp_min >= temp_max {
            return Err(DecisionError::InvalidParameter(
                "Temperature min must be less than max".to_string(),
            ));
        }

        if top_p_min >= top_p_max {
            return Err(DecisionError::InvalidParameter(
                "Top-p min must be less than max".to_string(),
            ));
        }

        if max_tokens_min >= max_tokens_max {
            return Err(DecisionError::InvalidParameter(
                "Max tokens min must be less than max".to_string(),
            ));
        }

        Ok(Self {
            temp_min,
            temp_max,
            top_p_min,
            top_p_max,
            max_tokens_min,
            max_tokens_max,
        })
    }

    /// Check if configuration is within range
    pub fn contains(&self, config: &ParameterConfig) -> bool {
        config.temperature >= self.temp_min
            && config.temperature <= self.temp_max
            && config.top_p >= self.top_p_min
            && config.top_p <= self.top_p_max
            && config.max_tokens >= self.max_tokens_min
            && config.max_tokens <= self.max_tokens_max
    }

    /// Clamp configuration to range
    pub fn clamp(&self, config: &ParameterConfig) -> ParameterConfig {
        ParameterConfig {
            temperature: config.temperature.clamp(self.temp_min, self.temp_max),
            top_p: config.top_p.clamp(self.top_p_min, self.top_p_max),
            max_tokens: config
                .max_tokens
                .clamp(self.max_tokens_min, self.max_tokens_max),
        }
    }

    /// Create restricted range for specific task type
    pub fn for_task_type(task_type: &str) -> Self {
        match task_type {
            "creative" | "storytelling" | "brainstorming" => Self {
                temp_min: 0.8,
                temp_max: 1.5,
                top_p_min: 0.9,
                top_p_max: 0.98,
                max_tokens_min: 512,
                max_tokens_max: 4096,
            },
            "code" | "programming" | "technical" => Self {
                temp_min: 0.0,
                temp_max: 0.5,
                top_p_min: 0.85,
                top_p_max: 0.95,
                max_tokens_min: 256,
                max_tokens_max: 4096,
            },
            "analytical" | "reasoning" | "math" => Self {
                temp_min: 0.0,
                temp_max: 0.4,
                top_p_min: 0.8,
                top_p_max: 0.9,
                max_tokens_min: 512,
                max_tokens_max: 2048,
            },
            _ => Self::default(),
        }
    }
}

/// Performance statistics for a parameter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterStats {
    /// Configuration ID
    pub config_id: Uuid,
    /// Parameter configuration
    pub config: ParameterConfig,
    /// Number of times used
    pub num_uses: u64,
    /// Total reward accumulated
    pub total_reward: f64,
    /// Average reward
    pub average_reward: f64,
    /// Average quality score
    pub avg_quality: f64,
    /// Average cost
    pub avg_cost: f64,
    /// Average latency
    pub avg_latency: f64,
    /// Success rate (task completion)
    pub success_rate: f64,
}

impl ParameterStats {
    /// Create new parameter statistics
    pub fn new(config_id: Uuid, config: ParameterConfig) -> Self {
        Self {
            config_id,
            config,
            num_uses: 0,
            total_reward: 0.0,
            average_reward: 0.0,
            avg_quality: 0.0,
            avg_cost: 0.0,
            avg_latency: 0.0,
            success_rate: 0.0,
        }
    }

    /// Update statistics with new observation
    pub fn update(&mut self, reward: f64, metrics: &ResponseMetrics, success: bool) {
        let n = self.num_uses as f64;
        let n_plus_1 = (self.num_uses + 1) as f64;

        // Running average updates
        self.total_reward += reward;
        self.average_reward = (self.average_reward * n + reward) / n_plus_1;
        self.avg_quality = (self.avg_quality * n + metrics.quality_score) / n_plus_1;
        self.avg_cost = (self.avg_cost * n + metrics.cost) / n_plus_1;
        self.avg_latency = (self.avg_latency * n + metrics.latency_ms) / n_plus_1;

        let success_count = (self.success_rate * n) + if success { 1.0 } else { 0.0 };
        self.success_rate = success_count / n_plus_1;

        self.num_uses += 1;
    }

    /// Get confidence interval width (exploration bonus)
    pub fn confidence_width(&self, exploration_factor: f64) -> f64 {
        if self.num_uses == 0 {
            return f64::INFINITY;
        }
        exploration_factor * (2.0 * (self.num_uses as f64).ln()).sqrt() / (self.num_uses as f64)
    }

    /// Upper confidence bound for this configuration
    pub fn ucb(&self, exploration_factor: f64) -> f64 {
        self.average_reward + self.confidence_width(exploration_factor)
    }
}

/// Adaptive parameter tuning engine
pub struct AdaptiveParameterTuner {
    /// Parameter range constraints
    range: ParameterRange,
    /// Statistics for each configuration
    config_stats: HashMap<Uuid, ParameterStats>,
    /// Task-specific best configurations
    task_best_configs: HashMap<String, Uuid>,
    /// Exploration factor for UCB
    exploration_factor: f64,
    /// Learning rate for gradient-based updates
    learning_rate: f64,
    /// Minimum uses before considering a configuration stable
    min_uses_for_stability: u64,
}

impl AdaptiveParameterTuner {
    /// Create new adaptive parameter tuner
    pub fn new(range: ParameterRange) -> Self {
        Self {
            range,
            config_stats: HashMap::new(),
            task_best_configs: HashMap::new(),
            exploration_factor: 2.0,
            learning_rate: 0.1,
            min_uses_for_stability: 10,
        }
    }

    /// Create with default range
    pub fn with_defaults() -> Self {
        Self::new(ParameterRange::default())
    }

    /// Set exploration factor
    pub fn with_exploration_factor(mut self, factor: f64) -> Self {
        self.exploration_factor = factor;
        self
    }

    /// Set learning rate
    pub fn with_learning_rate(mut self, rate: f64) -> Self {
        self.learning_rate = rate;
        self
    }

    /// Register a new parameter configuration
    pub fn register_config(&mut self, config: ParameterConfig) -> Result<Uuid> {
        config.validate()?;
        if !self.range.contains(&config) {
            return Err(DecisionError::InvalidParameter(
                "Configuration outside allowed range".to_string(),
            ));
        }

        let config_id = Uuid::new_v4();
        self.config_stats
            .insert(config_id, ParameterStats::new(config_id, config));
        Ok(config_id)
    }

    /// Select best configuration for given context using UCB
    pub fn select_config(&self, context: &RequestContext) -> Result<(Uuid, ParameterConfig)> {
        if self.config_stats.is_empty() {
            return Err(DecisionError::InvalidState(
                "No configurations registered".to_string(),
            ));
        }

        // Check for task-specific best configuration
        if let Some(task_type) = &context.task_type {
            if let Some(config_id) = self.task_best_configs.get(task_type) {
                if let Some(stats) = self.config_stats.get(config_id) {
                    if stats.num_uses >= self.min_uses_for_stability {
                        // Exploit with 80% probability, explore with 20%
                        if rand::random::<f64>() < 0.8 {
                            return Ok((*config_id, stats.config.clone()));
                        }
                    }
                }
            }
        }

        // Use UCB for selection
        let (best_id, best_stats) = self
            .config_stats
            .iter()
            .max_by(|(_, a), (_, b)| {
                let ucb_a = a.ucb(self.exploration_factor);
                let ucb_b = b.ucb(self.exploration_factor);
                ucb_a.partial_cmp(&ucb_b).unwrap_or(std::cmp::Ordering::Equal)
            })
            .ok_or_else(|| DecisionError::InvalidState("No configurations available".to_string()))?;

        Ok((*best_id, best_stats.config.clone()))
    }

    /// Update configuration performance
    pub fn update_config(
        &mut self,
        config_id: &Uuid,
        reward: f64,
        metrics: &ResponseMetrics,
        feedback: Option<&UserFeedback>,
    ) -> Result<()> {
        let stats = self.config_stats.get_mut(config_id).ok_or_else(|| {
            DecisionError::InvalidParameter(format!("Configuration {} not found", config_id))
        })?;

        let success = feedback.map(|f| f.task_completed).unwrap_or(true);
        stats.update(reward, metrics, success);

        Ok(())
    }

    /// Get best configuration for a task type
    pub fn get_best_for_task(&self, task_type: &str) -> Option<(Uuid, ParameterConfig)> {
        // Filter configurations by task suitability and find best
        let task_range = ParameterRange::for_task_type(task_type);

        self.config_stats
            .iter()
            .filter(|(_, stats)| {
                stats.num_uses >= self.min_uses_for_stability
                    && task_range.contains(&stats.config)
            })
            .max_by(|(_, a), (_, b)| {
                a.average_reward
                    .partial_cmp(&b.average_reward)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(id, stats)| (*id, stats.config.clone()))
    }

    /// Update task-specific best configuration
    pub fn update_task_best(&mut self, task_type: String) {
        if let Some((config_id, _)) = self.get_best_for_task(&task_type) {
            self.task_best_configs.insert(task_type, config_id);
        }
    }

    /// Suggest new configuration based on gradient
    pub fn suggest_improvement(&self, config_id: &Uuid) -> Result<ParameterConfig> {
        let stats = self.config_stats.get(config_id).ok_or_else(|| {
            DecisionError::InvalidParameter(format!("Configuration {} not found", config_id))
        })?;

        if stats.num_uses < self.min_uses_for_stability {
            return Err(DecisionError::InvalidState(
                "Not enough data for improvement suggestion".to_string(),
            ));
        }

        // Simple gradient-based suggestion
        // If quality is low, decrease temperature (more focused)
        // If quality is high but diversity might help, try increasing slightly
        let mut new_config = stats.config.clone();

        if stats.avg_quality < 0.7 {
            // Low quality - make more deterministic
            new_config.temperature *= 1.0 - self.learning_rate;
            new_config.top_p *= 1.0 - self.learning_rate * 0.5;
        } else if stats.avg_quality > 0.9 && stats.success_rate > 0.8 {
            // High quality - try slight exploration
            new_config.temperature *= 1.0 + self.learning_rate * 0.5;
            new_config.top_p = (new_config.top_p + 0.05).min(1.0);
        }

        // Clamp to range
        new_config = self.range.clamp(&new_config);
        new_config.validate()?;

        Ok(new_config)
    }

    /// Get statistics for all configurations
    pub fn get_all_stats(&self) -> Vec<ParameterStats> {
        self.config_stats.values().cloned().collect()
    }

    /// Get statistics for specific configuration
    pub fn get_stats(&self, config_id: &Uuid) -> Option<&ParameterStats> {
        self.config_stats.get(config_id)
    }

    /// Get number of registered configurations
    pub fn num_configs(&self) -> usize {
        self.config_stats.len()
    }

    /// Clear all statistics (for reset)
    pub fn reset(&mut self) {
        self.config_stats.clear();
        self.task_best_configs.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameter_config_creation() {
        let config = ParameterConfig::new(0.7, 0.9, 1024).unwrap();
        assert_eq!(config.temperature, 0.7);
        assert_eq!(config.top_p, 0.9);
        assert_eq!(config.max_tokens, 1024);
    }

    #[test]
    fn test_parameter_config_validation() {
        assert!(ParameterConfig::new(-0.1, 0.9, 1024).is_err());
        assert!(ParameterConfig::new(2.5, 0.9, 1024).is_err());
        assert!(ParameterConfig::new(0.7, 1.5, 1024).is_err());
        assert!(ParameterConfig::new(0.7, 0.9, 0).is_err());
    }

    #[test]
    fn test_preset_configs() {
        let creative = ParameterConfig::creative();
        assert!(creative.temperature > 1.0);
        assert!(creative.validate().is_ok());

        let analytical = ParameterConfig::analytical();
        assert!(analytical.temperature < 0.5);
        assert!(analytical.validate().is_ok());

        let code = ParameterConfig::code_generation();
        assert!(code.temperature < 0.3);
        assert!(code.validate().is_ok());
    }

    #[test]
    fn test_parameter_range_contains() {
        let range = ParameterRange::default();
        let config = ParameterConfig::default();
        assert!(range.contains(&config));

        let out_of_range = ParameterConfig {
            temperature: 3.0,
            top_p: 0.9,
            max_tokens: 1024,
        };
        assert!(!range.contains(&out_of_range));
    }

    #[test]
    fn test_parameter_range_clamp() {
        let range = ParameterRange::default();
        let config = ParameterConfig {
            temperature: 3.0,
            top_p: 0.5,
            max_tokens: 10000,
        };

        let clamped = range.clamp(&config);
        assert_eq!(clamped.temperature, range.temp_max);
        assert_eq!(clamped.top_p, 0.7); // Clamped to min
        assert_eq!(clamped.max_tokens, range.max_tokens_max);
    }

    #[test]
    fn test_task_specific_ranges() {
        let creative_range = ParameterRange::for_task_type("creative");
        assert!(creative_range.temp_min >= 0.8);

        let code_range = ParameterRange::for_task_type("code");
        assert!(code_range.temp_max <= 0.5);

        let analytical_range = ParameterRange::for_task_type("analytical");
        assert!(analytical_range.temp_max <= 0.4);
    }

    #[test]
    fn test_parameter_stats_creation() {
        let config_id = Uuid::new_v4();
        let config = ParameterConfig::default();
        let stats = ParameterStats::new(config_id, config.clone());

        assert_eq!(stats.config_id, config_id);
        assert_eq!(stats.num_uses, 0);
        assert_eq!(stats.average_reward, 0.0);
    }

    #[test]
    fn test_parameter_stats_update() {
        let config_id = Uuid::new_v4();
        let config = ParameterConfig::default();
        let mut stats = ParameterStats::new(config_id, config);

        let metrics = ResponseMetrics {
            quality_score: 0.9,
            cost: 0.1,
            latency_ms: 1000.0,
            token_count: 500,
        };

        stats.update(0.8, &metrics, true);

        assert_eq!(stats.num_uses, 1);
        assert_eq!(stats.average_reward, 0.8);
        assert_eq!(stats.avg_quality, 0.9);
        assert_eq!(stats.success_rate, 1.0);
    }

    #[test]
    fn test_parameter_stats_running_average() {
        let config_id = Uuid::new_v4();
        let config = ParameterConfig::default();
        let mut stats = ParameterStats::new(config_id, config);

        let metrics1 = ResponseMetrics {
            quality_score: 0.8,
            cost: 0.1,
            latency_ms: 1000.0,
            token_count: 500,
        };

        let metrics2 = ResponseMetrics {
            quality_score: 1.0,
            cost: 0.2,
            latency_ms: 1500.0,
            token_count: 600,
        };

        stats.update(0.7, &metrics1, true);
        stats.update(0.9, &metrics2, true);

        assert_eq!(stats.num_uses, 2);
        assert_eq!(stats.average_reward, 0.8);
        assert_eq!(stats.avg_quality, 0.9);
        assert_eq!(stats.success_rate, 1.0);
    }

    #[test]
    fn test_ucb_calculation() {
        let config_id = Uuid::new_v4();
        let config = ParameterConfig::default();
        let mut stats = ParameterStats::new(config_id, config);

        let metrics = ResponseMetrics {
            quality_score: 0.9,
            cost: 0.1,
            latency_ms: 1000.0,
            token_count: 500,
        };

        // Update multiple times to get meaningful UCB
        for _ in 0..5 {
            stats.update(0.8, &metrics, true);
        }

        let ucb = stats.ucb(2.0);
        assert!(ucb >= stats.average_reward);
        assert!(stats.num_uses == 5);
    }

    #[test]
    fn test_adaptive_tuner_creation() {
        let tuner = AdaptiveParameterTuner::with_defaults();
        assert_eq!(tuner.num_configs(), 0);
    }

    #[test]
    fn test_register_config() {
        let mut tuner = AdaptiveParameterTuner::with_defaults();
        let config = ParameterConfig::default();

        let config_id = tuner.register_config(config).unwrap();
        assert_eq!(tuner.num_configs(), 1);
        assert!(tuner.get_stats(&config_id).is_some());
    }

    #[test]
    fn test_register_invalid_config() {
        let mut tuner = AdaptiveParameterTuner::with_defaults();
        let config = ParameterConfig {
            temperature: 3.0,
            top_p: 0.9,
            max_tokens: 1024,
        };

        assert!(tuner.register_config(config).is_err());
    }

    #[test]
    fn test_select_config() {
        let mut tuner = AdaptiveParameterTuner::with_defaults();
        let config1 = ParameterConfig::default();
        let config2 = ParameterConfig::creative();

        tuner.register_config(config1).unwrap();
        tuner.register_config(config2).unwrap();

        let context = RequestContext::new(100);
        let (config_id, _) = tuner.select_config(&context).unwrap();
        assert!(tuner.get_stats(&config_id).is_some());
    }

    #[test]
    fn test_update_config() {
        let mut tuner = AdaptiveParameterTuner::with_defaults();
        let config = ParameterConfig::default();
        let config_id = tuner.register_config(config).unwrap();

        let metrics = ResponseMetrics {
            quality_score: 0.9,
            cost: 0.1,
            latency_ms: 1000.0,
            token_count: 500,
        };

        tuner.update_config(&config_id, 0.8, &metrics, None).unwrap();

        let stats = tuner.get_stats(&config_id).unwrap();
        assert_eq!(stats.num_uses, 1);
        assert_eq!(stats.average_reward, 0.8);
    }

    #[test]
    fn test_tuner_learning() {
        let mut tuner = AdaptiveParameterTuner::with_defaults();
        let config1 = ParameterConfig::default();
        let config2 = ParameterConfig::creative();

        let id1 = tuner.register_config(config1).unwrap();
        let id2 = tuner.register_config(config2).unwrap();

        let good_metrics = ResponseMetrics {
            quality_score: 0.95,
            cost: 0.1,
            latency_ms: 1000.0,
            token_count: 500,
        };

        let bad_metrics = ResponseMetrics {
            quality_score: 0.5,
            cost: 0.2,
            latency_ms: 2000.0,
            token_count: 600,
        };

        // Update config1 with good performance
        for _ in 0..20 {
            tuner.update_config(&id1, 0.9, &good_metrics, None).unwrap();
        }

        // Update config2 with poor performance
        for _ in 0..20 {
            tuner.update_config(&id2, 0.3, &bad_metrics, None).unwrap();
        }

        let stats1 = tuner.get_stats(&id1).unwrap();
        let stats2 = tuner.get_stats(&id2).unwrap();

        assert!(stats1.average_reward > stats2.average_reward);
    }

    #[test]
    fn test_get_best_for_task() {
        let mut tuner = AdaptiveParameterTuner::with_defaults();
        let code_config = ParameterConfig::code_generation();
        let config_id = tuner.register_config(code_config).unwrap();

        let good_metrics = ResponseMetrics {
            quality_score: 0.95,
            cost: 0.1,
            latency_ms: 1000.0,
            token_count: 500,
        };

        // Need enough samples for stability
        for _ in 0..15 {
            tuner.update_config(&config_id, 0.9, &good_metrics, None).unwrap();
        }

        tuner.update_task_best("code".to_string());
        let best = tuner.get_best_for_task("code");
        assert!(best.is_some());
    }

    #[test]
    fn test_suggest_improvement() {
        let mut tuner = AdaptiveParameterTuner::with_defaults();
        let config = ParameterConfig::default();
        let config_id = tuner.register_config(config).unwrap();

        let metrics = ResponseMetrics {
            quality_score: 0.5,
            cost: 0.1,
            latency_ms: 1000.0,
            token_count: 500,
        };

        // Need enough samples for suggestion
        for _ in 0..15 {
            tuner.update_config(&config_id, 0.6, &metrics, None).unwrap();
        }

        let improved = tuner.suggest_improvement(&config_id).unwrap();
        let original = tuner.get_stats(&config_id).unwrap();

        // Low quality should suggest lower temperature
        assert!(improved.temperature <= original.config.temperature);
    }

    #[test]
    fn test_reset() {
        let mut tuner = AdaptiveParameterTuner::with_defaults();
        tuner.register_config(ParameterConfig::default()).unwrap();

        assert_eq!(tuner.num_configs(), 1);
        tuner.reset();
        assert_eq!(tuner.num_configs(), 0);
    }
}
