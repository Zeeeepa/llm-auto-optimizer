//! Variant generation strategies for A/B testing
//!
//! This module provides strategies for generating prompt variants
//! for A/B testing experiments.

use llm_optimizer_types::models::ModelConfig;
use serde::{Deserialize, Serialize};

use crate::errors::{DecisionError, Result};

/// Strategy for generating prompt variants
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VariantStrategy {
    /// Vary temperature parameter
    Temperature(Vec<f64>),
    /// Vary top-p parameter
    TopP(Vec<f64>),
    /// Vary max tokens
    MaxTokens(Vec<u32>),
    /// Vary system prompt
    SystemPrompt(Vec<String>),
    /// Vary model
    Model(Vec<String>),
    /// Custom configuration variants
    Custom(Vec<ModelConfig>),
}

/// Variant generator
pub struct VariantGenerator;

impl VariantGenerator {
    /// Generate variants from a base configuration and strategy
    pub fn generate(base: &ModelConfig, strategy: &VariantStrategy) -> Result<Vec<ModelConfig>> {
        match strategy {
            VariantStrategy::Temperature(temps) => {
                Self::generate_temperature_variants(base, temps)
            }
            VariantStrategy::TopP(top_ps) => {
                Self::generate_top_p_variants(base, top_ps)
            }
            VariantStrategy::MaxTokens(max_tokens) => {
                Self::generate_max_tokens_variants(base, max_tokens)
            }
            VariantStrategy::SystemPrompt(prompts) => {
                Self::generate_system_prompt_variants(base, prompts)
            }
            VariantStrategy::Model(models) => {
                Self::generate_model_variants(base, models)
            }
            VariantStrategy::Custom(configs) => {
                Ok(configs.clone())
            }
        }
    }

    /// Generate variants with different temperature values
    fn generate_temperature_variants(base: &ModelConfig, temps: &[f64]) -> Result<Vec<ModelConfig>> {
        if temps.is_empty() {
            return Err(DecisionError::InvalidConfig(
                "Temperature variants list is empty".to_string()
            ));
        }

        let variants: Vec<ModelConfig> = temps.iter().map(|&temp| {
            let mut config = base.clone();
            config.temperature = temp.clamp(0.0, 1.0);
            config
        }).collect();

        Ok(variants)
    }

    /// Generate variants with different top-p values
    fn generate_top_p_variants(base: &ModelConfig, top_ps: &[f64]) -> Result<Vec<ModelConfig>> {
        if top_ps.is_empty() {
            return Err(DecisionError::InvalidConfig(
                "Top-p variants list is empty".to_string()
            ));
        }

        let variants: Vec<ModelConfig> = top_ps.iter().map(|&top_p| {
            let mut config = base.clone();
            config.top_p = top_p.clamp(0.0, 1.0);
            config
        }).collect();

        Ok(variants)
    }

    /// Generate variants with different max tokens
    fn generate_max_tokens_variants(base: &ModelConfig, max_tokens: &[u32]) -> Result<Vec<ModelConfig>> {
        if max_tokens.is_empty() {
            return Err(DecisionError::InvalidConfig(
                "Max tokens variants list is empty".to_string()
            ));
        }

        let variants: Vec<ModelConfig> = max_tokens.iter().map(|&max_tok| {
            let mut config = base.clone();
            config.max_tokens = max_tok;
            config
        }).collect();

        Ok(variants)
    }

    /// Generate variants with different system prompts
    fn generate_system_prompt_variants(base: &ModelConfig, prompts: &[String]) -> Result<Vec<ModelConfig>> {
        if prompts.is_empty() {
            return Err(DecisionError::InvalidConfig(
                "System prompt variants list is empty".to_string()
            ));
        }

        let variants: Vec<ModelConfig> = prompts.iter().map(|prompt| {
            let mut config = base.clone();
            config.system_prompt = Some(prompt.clone());
            config
        }).collect();

        Ok(variants)
    }

    /// Generate variants with different models
    fn generate_model_variants(base: &ModelConfig, models: &[String]) -> Result<Vec<ModelConfig>> {
        if models.is_empty() {
            return Err(DecisionError::InvalidConfig(
                "Model variants list is empty".to_string()
            ));
        }

        let variants: Vec<ModelConfig> = models.iter().map(|model| {
            let mut config = base.clone();
            config.model = model.clone();
            config
        }).collect();

        Ok(variants)
    }

    /// Generate standard set of temperature variants
    pub fn standard_temperature_variants(base: &ModelConfig) -> Result<Vec<ModelConfig>> {
        Self::generate_temperature_variants(base, &[0.0, 0.3, 0.7, 1.0])
    }

    /// Generate standard set of top-p variants
    pub fn standard_top_p_variants(base: &ModelConfig) -> Result<Vec<ModelConfig>> {
        Self::generate_top_p_variants(base, &[0.8, 0.9, 0.95, 1.0])
    }

    /// Validate a configuration
    pub fn validate_config(config: &ModelConfig) -> Result<()> {
        if config.temperature < 0.0 || config.temperature > 1.0 {
            return Err(DecisionError::InvalidConfig(
                format!("Temperature {} is out of range [0, 1]", config.temperature)
            ));
        }

        if config.top_p < 0.0 || config.top_p > 1.0 {
            return Err(DecisionError::InvalidConfig(
                format!("Top-p {} is out of range [0, 1]", config.top_p)
            ));
        }

        if config.max_tokens == 0 {
            return Err(DecisionError::InvalidConfig(
                "Max tokens must be greater than 0".to_string()
            ));
        }

        if let Some(presence) = config.presence_penalty {
            if presence < -2.0 || presence > 2.0 {
                return Err(DecisionError::InvalidConfig(
                    format!("Presence penalty {} is out of range [-2, 2]", presence)
                ));
            }
        }

        if let Some(frequency) = config.frequency_penalty {
            if frequency < -2.0 || frequency > 2.0 {
                return Err(DecisionError::InvalidConfig(
                    format!("Frequency penalty {} is out of range [-2, 2]", frequency)
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_config() -> ModelConfig {
        ModelConfig {
            model: "test-model".to_string(),
            temperature: 0.7,
            top_p: 0.9,
            top_k: None,
            max_tokens: 1024,
            presence_penalty: None,
            frequency_penalty: None,
            system_prompt: None,
            extra_params: Default::default(),
        }
    }

    #[test]
    fn test_generate_temperature_variants() {
        let base = base_config();
        let strategy = VariantStrategy::Temperature(vec![0.0, 0.5, 1.0]);
        
        let variants = VariantGenerator::generate(&base, &strategy).unwrap();
        
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0].temperature, 0.0);
        assert_eq!(variants[1].temperature, 0.5);
        assert_eq!(variants[2].temperature, 1.0);
    }

    #[test]
    fn test_generate_top_p_variants() {
        let base = base_config();
        let strategy = VariantStrategy::TopP(vec![0.8, 0.9, 1.0]);
        
        let variants = VariantGenerator::generate(&base, &strategy).unwrap();
        
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0].top_p, 0.8);
        assert_eq!(variants[1].top_p, 0.9);
        assert_eq!(variants[2].top_p, 1.0);
    }

    #[test]
    fn test_generate_max_tokens_variants() {
        let base = base_config();
        let strategy = VariantStrategy::MaxTokens(vec![512, 1024, 2048]);
        
        let variants = VariantGenerator::generate(&base, &strategy).unwrap();
        
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0].max_tokens, 512);
        assert_eq!(variants[1].max_tokens, 1024);
        assert_eq!(variants[2].max_tokens, 2048);
    }

    #[test]
    fn test_generate_system_prompt_variants() {
        let base = base_config();
        let prompts = vec![
            "You are a helpful assistant.".to_string(),
            "You are a coding expert.".to_string(),
        ];
        let strategy = VariantStrategy::SystemPrompt(prompts.clone());
        
        let variants = VariantGenerator::generate(&base, &strategy).unwrap();
        
        assert_eq!(variants.len(), 2);
        assert_eq!(variants[0].system_prompt, Some(prompts[0].clone()));
        assert_eq!(variants[1].system_prompt, Some(prompts[1].clone()));
    }

    #[test]
    fn test_generate_model_variants() {
        let base = base_config();
        let models = vec!["model-1".to_string(), "model-2".to_string()];
        let strategy = VariantStrategy::Model(models.clone());
        
        let variants = VariantGenerator::generate(&base, &strategy).unwrap();
        
        assert_eq!(variants.len(), 2);
        assert_eq!(variants[0].model, models[0]);
        assert_eq!(variants[1].model, models[1]);
    }

    #[test]
    fn test_empty_variants_error() {
        let base = base_config();
        let strategy = VariantStrategy::Temperature(vec![]);
        
        assert!(VariantGenerator::generate(&base, &strategy).is_err());
    }

    #[test]
    fn test_temperature_clamping() {
        let base = base_config();
        let strategy = VariantStrategy::Temperature(vec![-0.5, 0.5, 1.5]);
        
        let variants = VariantGenerator::generate(&base, &strategy).unwrap();
        
        // Should clamp to [0, 1]
        assert_eq!(variants[0].temperature, 0.0);
        assert_eq!(variants[1].temperature, 0.5);
        assert_eq!(variants[2].temperature, 1.0);
    }

    #[test]
    fn test_standard_temperature_variants() {
        let base = base_config();
        let variants = VariantGenerator::standard_temperature_variants(&base).unwrap();
        
        assert_eq!(variants.len(), 4);
        assert_eq!(variants[0].temperature, 0.0);
        assert_eq!(variants[3].temperature, 1.0);
    }

    #[test]
    fn test_standard_top_p_variants() {
        let base = base_config();
        let variants = VariantGenerator::standard_top_p_variants(&base).unwrap();
        
        assert_eq!(variants.len(), 4);
        assert_eq!(variants[0].top_p, 0.8);
        assert_eq!(variants[3].top_p, 1.0);
    }

    #[test]
    fn test_validate_config_valid() {
        let config = base_config();
        assert!(VariantGenerator::validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_config_invalid_temperature() {
        let mut config = base_config();
        config.temperature = 1.5;
        
        assert!(VariantGenerator::validate_config(&config).is_err());
    }

    #[test]
    fn test_validate_config_invalid_top_p() {
        let mut config = base_config();
        config.top_p = -0.1;
        
        assert!(VariantGenerator::validate_config(&config).is_err());
    }

    #[test]
    fn test_validate_config_zero_max_tokens() {
        let mut config = base_config();
        config.max_tokens = 0;
        
        assert!(VariantGenerator::validate_config(&config).is_err());
    }

    #[test]
    fn test_validate_config_invalid_penalties() {
        let mut config = base_config();
        config.presence_penalty = Some(3.0);
        
        assert!(VariantGenerator::validate_config(&config).is_err());
        
        config.presence_penalty = None;
        config.frequency_penalty = Some(-3.0);
        
        assert!(VariantGenerator::validate_config(&config).is_err());
    }
}
