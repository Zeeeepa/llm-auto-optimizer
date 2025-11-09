//! LLM Model Registry and Catalog
//!
//! This module provides a comprehensive registry of LLM models with their
//! pricing, performance characteristics, and capabilities.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::errors::Result;
use crate::pareto::{ModelCandidate, Objectives};

/// LLM Provider
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    OpenAI,
    Anthropic,
    Google,
    Meta,
    Mistral,
    XAI,
    DeepSeek,
    Alibaba,
}

impl Provider {
    /// Get all supported providers
    pub fn all() -> Vec<Provider> {
        vec![
            Provider::OpenAI,
            Provider::Anthropic,
            Provider::Google,
            Provider::Meta,
            Provider::Mistral,
            Provider::XAI,
            Provider::DeepSeek,
            Provider::Alibaba,
        ]
    }

    /// Get provider name
    pub fn name(&self) -> &'static str {
        match self {
            Provider::OpenAI => "OpenAI",
            Provider::Anthropic => "Anthropic",
            Provider::Google => "Google",
            Provider::Meta => "Meta",
            Provider::Mistral => "Mistral AI",
            Provider::XAI => "xAI",
            Provider::DeepSeek => "DeepSeek",
            Provider::Alibaba => "Alibaba Cloud",
        }
    }
}

/// Model tier for capability classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelTier {
    /// Flagship models (highest capability, highest cost)
    Flagship,
    /// Advanced models (high capability, moderate cost)
    Advanced,
    /// Standard models (good capability, low cost)
    Standard,
    /// Efficient models (basic capability, very low cost)
    Efficient,
}

/// Model pricing information
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ModelPricing {
    /// Input tokens price per 1,000 tokens (USD)
    pub input_price_per_1k: f64,
    /// Output tokens price per 1,000 tokens (USD)
    pub output_price_per_1k: f64,
    /// Cached input price per 1,000 tokens (USD), if supported
    pub cached_input_price_per_1k: Option<f64>,
}

impl ModelPricing {
    /// Create new pricing
    pub fn new(input: f64, output: f64) -> Self {
        Self {
            input_price_per_1k: input,
            output_price_per_1k: output,
            cached_input_price_per_1k: None,
        }
    }

    /// Create pricing with cache support
    pub fn with_cache(input: f64, output: f64, cached: f64) -> Self {
        Self {
            input_price_per_1k: input,
            output_price_per_1k: output,
            cached_input_price_per_1k: Some(cached),
        }
    }

    /// Calculate cost for given token counts
    pub fn calculate_cost(&self, input_tokens: usize, output_tokens: usize) -> f64 {
        let input_cost = (input_tokens as f64 / 1000.0) * self.input_price_per_1k;
        let output_cost = (output_tokens as f64 / 1000.0) * self.output_price_per_1k;
        input_cost + output_cost
    }
}

/// Model performance characteristics
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ModelPerformance {
    /// Average latency p50 in milliseconds
    pub latency_p50_ms: f64,
    /// P95 latency in milliseconds
    pub latency_p95_ms: f64,
    /// P99 latency in milliseconds
    pub latency_p99_ms: f64,
    /// Tokens per second throughput
    pub tokens_per_second: f64,
    /// Maximum context window (tokens)
    pub max_context_tokens: usize,
    /// Maximum output tokens
    pub max_output_tokens: usize,
}

impl ModelPerformance {
    /// Create new performance profile
    pub fn new(
        p50: f64,
        p95: f64,
        p99: f64,
        throughput: f64,
        context: usize,
        max_output: usize,
    ) -> Self {
        Self {
            latency_p50_ms: p50,
            latency_p95_ms: p95,
            latency_p99_ms: p99,
            tokens_per_second: throughput,
            max_context_tokens: context,
            max_output_tokens: max_output,
        }
    }
}

/// Model capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCapabilities {
    /// Supports function calling
    pub function_calling: bool,
    /// Supports vision/image input
    pub vision: bool,
    /// Supports streaming responses
    pub streaming: bool,
    /// Supports JSON mode
    pub json_mode: bool,
    /// Supports prompt caching
    pub prompt_caching: bool,
    /// Multimodal capabilities
    pub multimodal: bool,
    /// Supported languages (ISO codes)
    pub languages: Vec<String>,
}

impl ModelCapabilities {
    /// Standard capabilities (text-only, basic features)
    pub fn standard() -> Self {
        Self {
            function_calling: true,
            vision: false,
            streaming: true,
            json_mode: true,
            prompt_caching: false,
            multimodal: false,
            languages: vec!["en".to_string()],
        }
    }

    /// Advanced capabilities (multimodal, all features)
    pub fn advanced() -> Self {
        Self {
            function_calling: true,
            vision: true,
            streaming: true,
            json_mode: true,
            prompt_caching: true,
            multimodal: true,
            languages: vec![
                "en", "es", "fr", "de", "it", "pt", "ja", "ko", "zh",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        }
    }
}

/// Complete model definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDefinition {
    /// Model identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Provider
    pub provider: Provider,
    /// Model tier
    pub tier: ModelTier,
    /// Pricing information
    pub pricing: ModelPricing,
    /// Performance characteristics
    pub performance: ModelPerformance,
    /// Capabilities
    pub capabilities: ModelCapabilities,
    /// Estimated quality score [0, 1]
    pub quality_score: f64,
    /// Release date (for versioning)
    pub release_date: Option<String>,
    /// Whether model is currently available
    pub available: bool,
}

impl ModelDefinition {
    /// Create a Pareto candidate from this model definition
    pub fn to_pareto_candidate(
        &self,
        input_tokens: usize,
        output_tokens: usize,
    ) -> Result<ModelCandidate> {
        let cost = self.pricing.calculate_cost(input_tokens, output_tokens);

        ModelCandidate::new(
            Uuid::new_v4(),
            &self.id,
            Objectives::new(self.quality_score, cost, self.performance.latency_p95_ms),
        )
    }
}

/// Model registry
pub struct ModelRegistry {
    models: HashMap<String, ModelDefinition>,
}

impl ModelRegistry {
    /// Create a new model registry with all supported models
    pub fn new() -> Self {
        let mut registry = Self {
            models: HashMap::new(),
        };

        // OpenAI Models
        registry.register_openai_models();

        // Anthropic Models
        registry.register_anthropic_models();

        // Google Models
        registry.register_google_models();

        // Meta Models
        registry.register_meta_models();

        // Mistral Models
        registry.register_mistral_models();

        // xAI Models
        registry.register_xai_models();

        // DeepSeek Models
        registry.register_deepseek_models();

        // Alibaba Models
        registry.register_alibaba_models();

        registry
    }

    /// Register a model
    fn register(&mut self, model: ModelDefinition) {
        self.models.insert(model.id.clone(), model);
    }

    /// Get model by ID
    pub fn get(&self, id: &str) -> Option<&ModelDefinition> {
        self.models.get(id)
    }

    /// Get all models
    pub fn all_models(&self) -> Vec<&ModelDefinition> {
        self.models.values().collect()
    }

    /// Get models by provider
    pub fn models_by_provider(&self, provider: Provider) -> Vec<&ModelDefinition> {
        self.models
            .values()
            .filter(|m| m.provider == provider)
            .collect()
    }

    /// Get models by tier
    pub fn models_by_tier(&self, tier: ModelTier) -> Vec<&ModelDefinition> {
        self.models.values().filter(|m| m.tier == tier).collect()
    }

    /// Get available models
    pub fn available_models(&self) -> Vec<&ModelDefinition> {
        self.models.values().filter(|m| m.available).collect()
    }

    /// Compare models and find Pareto optimal set
    pub fn find_pareto_optimal(
        &self,
        model_ids: &[String],
        input_tokens: usize,
        output_tokens: usize,
    ) -> Result<Vec<ModelCandidate>> {
        let candidates: Result<Vec<ModelCandidate>> = model_ids
            .iter()
            .filter_map(|id| self.get(id))
            .map(|model| model.to_pareto_candidate(input_tokens, output_tokens))
            .collect();

        let candidates = candidates?;
        Ok(crate::pareto::ParetoFrontier::find_pareto_optimal(
            &candidates,
        ))
    }

    // Provider-specific registration methods

    fn register_openai_models(&mut self) {
        // GPT-5 (Future flagship)
        self.register(ModelDefinition {
            id: "gpt-5".to_string(),
            name: "GPT-5".to_string(),
            provider: Provider::OpenAI,
            tier: ModelTier::Flagship,
            pricing: ModelPricing::with_cache(0.010, 0.030, 0.005),
            performance: ModelPerformance::new(800.0, 1500.0, 2500.0, 120.0, 200000, 16384),
            capabilities: ModelCapabilities::advanced(),
            quality_score: 0.98,
            release_date: Some("2025-Q3".to_string()),
            available: false,
        });

        // GPT-4.5 (Future advanced)
        self.register(ModelDefinition {
            id: "gpt-4.5".to_string(),
            name: "GPT-4.5".to_string(),
            provider: Provider::OpenAI,
            tier: ModelTier::Advanced,
            pricing: ModelPricing::with_cache(0.005, 0.015, 0.0025),
            performance: ModelPerformance::new(600.0, 1200.0, 2000.0, 150.0, 128000, 8192),
            capabilities: ModelCapabilities::advanced(),
            quality_score: 0.95,
            release_date: Some("2025-Q2".to_string()),
            available: false,
        });

        // GPT-4.1 (Projected improvement)
        self.register(ModelDefinition {
            id: "gpt-4.1".to_string(),
            name: "GPT-4.1 Turbo".to_string(),
            provider: Provider::OpenAI,
            tier: ModelTier::Advanced,
            pricing: ModelPricing::with_cache(0.003, 0.012, 0.0015),
            performance: ModelPerformance::new(500.0, 1000.0, 1800.0, 180.0, 128000, 4096),
            capabilities: ModelCapabilities::advanced(),
            quality_score: 0.93,
            release_date: Some("2025-Q1".to_string()),
            available: false,
        });

        // Current GPT-4 Turbo
        self.register(ModelDefinition {
            id: "gpt-4-turbo".to_string(),
            name: "GPT-4 Turbo".to_string(),
            provider: Provider::OpenAI,
            tier: ModelTier::Advanced,
            pricing: ModelPricing::new(0.01, 0.03),
            performance: ModelPerformance::new(800.0, 1600.0, 2800.0, 100.0, 128000, 4096),
            capabilities: ModelCapabilities::advanced(),
            quality_score: 0.92,
            release_date: Some("2024-04".to_string()),
            available: true,
        });

        // GPT-3.5 Turbo
        self.register(ModelDefinition {
            id: "gpt-3.5-turbo".to_string(),
            name: "GPT-3.5 Turbo".to_string(),
            provider: Provider::OpenAI,
            tier: ModelTier::Efficient,
            pricing: ModelPricing::new(0.0005, 0.0015),
            performance: ModelPerformance::new(300.0, 600.0, 1000.0, 250.0, 16385, 4096),
            capabilities: ModelCapabilities::standard(),
            quality_score: 0.78,
            release_date: Some("2023-11".to_string()),
            available: true,
        });
    }

    fn register_anthropic_models(&mut self) {
        // Claude 4 Opus (Future flagship)
        self.register(ModelDefinition {
            id: "claude-4-opus".to_string(),
            name: "Claude 4 Opus".to_string(),
            provider: Provider::Anthropic,
            tier: ModelTier::Flagship,
            pricing: ModelPricing::with_cache(0.012, 0.060, 0.006),
            performance: ModelPerformance::new(1000.0, 2000.0, 3500.0, 100.0, 200000, 8192),
            capabilities: ModelCapabilities::advanced(),
            quality_score: 0.97,
            release_date: Some("2025-Q4".to_string()),
            available: false,
        });

        // Claude Sonnet 4.5 (Future advanced) - Based on actual release
        self.register(ModelDefinition {
            id: "claude-sonnet-4.5".to_string(),
            name: "Claude Sonnet 4.5".to_string(),
            provider: Provider::Anthropic,
            tier: ModelTier::Advanced,
            pricing: ModelPricing::with_cache(0.003, 0.015, 0.0015),
            performance: ModelPerformance::new(600.0, 1200.0, 2200.0, 140.0, 200000, 8192),
            capabilities: ModelCapabilities::advanced(),
            quality_score: 0.94,
            release_date: Some("2024-10".to_string()),
            available: true,
        });

        // Current Claude 3.5 Sonnet
        self.register(ModelDefinition {
            id: "claude-3.5-sonnet".to_string(),
            name: "Claude 3.5 Sonnet".to_string(),
            provider: Provider::Anthropic,
            tier: ModelTier::Advanced,
            pricing: ModelPricing::with_cache(0.003, 0.015, 0.0015),
            performance: ModelPerformance::new(700.0, 1400.0, 2400.0, 120.0, 200000, 8192),
            capabilities: ModelCapabilities::advanced(),
            quality_score: 0.93,
            release_date: Some("2024-06".to_string()),
            available: true,
        });

        // Claude 3 Opus
        self.register(ModelDefinition {
            id: "claude-3-opus".to_string(),
            name: "Claude 3 Opus".to_string(),
            provider: Provider::Anthropic,
            tier: ModelTier::Flagship,
            pricing: ModelPricing::new(0.015, 0.075),
            performance: ModelPerformance::new(1200.0, 2400.0, 4000.0, 80.0, 200000, 4096),
            capabilities: ModelCapabilities::advanced(),
            quality_score: 0.95,
            release_date: Some("2024-03".to_string()),
            available: true,
        });

        // Claude 3 Haiku
        self.register(ModelDefinition {
            id: "claude-3-haiku".to_string(),
            name: "Claude 3 Haiku".to_string(),
            provider: Provider::Anthropic,
            tier: ModelTier::Efficient,
            pricing: ModelPricing::new(0.00025, 0.00125),
            performance: ModelPerformance::new(300.0, 600.0, 1000.0, 200.0, 200000, 4096),
            capabilities: ModelCapabilities::standard(),
            quality_score: 0.82,
            release_date: Some("2024-03".to_string()),
            available: true,
        });
    }

    fn register_google_models(&mut self) {
        // Gemini 2.5 Pro (Future)
        self.register(ModelDefinition {
            id: "gemini-2.5-pro".to_string(),
            name: "Gemini 2.5 Pro".to_string(),
            provider: Provider::Google,
            tier: ModelTier::Flagship,
            pricing: ModelPricing::new(0.00125, 0.005),
            performance: ModelPerformance::new(700.0, 1400.0, 2400.0, 130.0, 1000000, 8192),
            capabilities: ModelCapabilities::advanced(),
            quality_score: 0.96,
            release_date: Some("2025-Q2".to_string()),
            available: false,
        });

        // Gemini 2.0 Flash
        self.register(ModelDefinition {
            id: "gemini-2.0-flash".to_string(),
            name: "Gemini 2.0 Flash".to_string(),
            provider: Provider::Google,
            tier: ModelTier::Standard,
            pricing: ModelPricing::new(0.0, 0.0), // Free tier available
            performance: ModelPerformance::new(400.0, 800.0, 1400.0, 180.0, 1000000, 8192),
            capabilities: ModelCapabilities::advanced(),
            quality_score: 0.88,
            release_date: Some("2024-12".to_string()),
            available: true,
        });

        // Gemini 1.5 Pro
        self.register(ModelDefinition {
            id: "gemini-1.5-pro".to_string(),
            name: "Gemini 1.5 Pro".to_string(),
            provider: Provider::Google,
            tier: ModelTier::Advanced,
            pricing: ModelPricing::new(0.00125, 0.005),
            performance: ModelPerformance::new(800.0, 1600.0, 2800.0, 110.0, 2000000, 8192),
            capabilities: ModelCapabilities::advanced(),
            quality_score: 0.91,
            release_date: Some("2024-05".to_string()),
            available: true,
        });
    }

    fn register_meta_models(&mut self) {
        // Llama 4 Scout (Future - reasoning-focused)
        self.register(ModelDefinition {
            id: "llama-4-scout".to_string(),
            name: "Llama 4 Scout".to_string(),
            provider: Provider::Meta,
            tier: ModelTier::Advanced,
            pricing: ModelPricing::new(0.0, 0.0), // Open source, self-hosted
            performance: ModelPerformance::new(900.0, 1800.0, 3000.0, 90.0, 128000, 8192),
            capabilities: ModelCapabilities::advanced(),
            quality_score: 0.90,
            release_date: Some("2025-Q3".to_string()),
            available: false,
        });

        // Llama 4 Maverick (Future - efficient variant)
        self.register(ModelDefinition {
            id: "llama-4-maverick".to_string(),
            name: "Llama 4 Maverick".to_string(),
            provider: Provider::Meta,
            tier: ModelTier::Standard,
            pricing: ModelPricing::new(0.0, 0.0), // Open source
            performance: ModelPerformance::new(500.0, 1000.0, 1800.0, 150.0, 128000, 8192),
            capabilities: ModelCapabilities::standard(),
            quality_score: 0.85,
            release_date: Some("2025-Q3".to_string()),
            available: false,
        });

        // Current Llama 3.3 70B
        self.register(ModelDefinition {
            id: "llama-3.3-70b".to_string(),
            name: "Llama 3.3 70B".to_string(),
            provider: Provider::Meta,
            tier: ModelTier::Advanced,
            pricing: ModelPricing::new(0.0, 0.0), // Open source
            performance: ModelPerformance::new(800.0, 1600.0, 2800.0, 100.0, 128000, 8192),
            capabilities: ModelCapabilities::standard(),
            quality_score: 0.87,
            release_date: Some("2024-12".to_string()),
            available: true,
        });
    }

    fn register_mistral_models(&mut self) {
        // Mistral Large 2
        self.register(ModelDefinition {
            id: "mistral-large-2".to_string(),
            name: "Mistral Large 2".to_string(),
            provider: Provider::Mistral,
            tier: ModelTier::Advanced,
            pricing: ModelPricing::new(0.002, 0.006),
            performance: ModelPerformance::new(600.0, 1200.0, 2000.0, 130.0, 128000, 8192),
            capabilities: ModelCapabilities::advanced(),
            quality_score: 0.89,
            release_date: Some("2024-07".to_string()),
            available: true,
        });

        // Mixtral 8x22B
        self.register(ModelDefinition {
            id: "mixtral-8x22b".to_string(),
            name: "Mixtral 8x22B".to_string(),
            provider: Provider::Mistral,
            tier: ModelTier::Advanced,
            pricing: ModelPricing::new(0.0, 0.0), // Open source
            performance: ModelPerformance::new(700.0, 1400.0, 2400.0, 110.0, 65536, 8192),
            capabilities: ModelCapabilities::standard(),
            quality_score: 0.86,
            release_date: Some("2024-04".to_string()),
            available: true,
        });

        // Mistral Nemo
        self.register(ModelDefinition {
            id: "mistral-nemo".to_string(),
            name: "Mistral Nemo".to_string(),
            provider: Provider::Mistral,
            tier: ModelTier::Efficient,
            pricing: ModelPricing::new(0.0003, 0.0003),
            performance: ModelPerformance::new(300.0, 600.0, 1000.0, 200.0, 128000, 8192),
            capabilities: ModelCapabilities::standard(),
            quality_score: 0.80,
            release_date: Some("2024-07".to_string()),
            available: true,
        });
    }

    fn register_xai_models(&mut self) {
        // Grok 4 (Future)
        self.register(ModelDefinition {
            id: "grok-4".to_string(),
            name: "Grok 4".to_string(),
            provider: Provider::XAI,
            tier: ModelTier::Flagship,
            pricing: ModelPricing::new(0.005, 0.015),
            performance: ModelPerformance::new(800.0, 1600.0, 2800.0, 120.0, 128000, 8192),
            capabilities: ModelCapabilities::advanced(),
            quality_score: 0.92,
            release_date: Some("2025-Q4".to_string()),
            available: false,
        });

        // Grok 2 (Current)
        self.register(ModelDefinition {
            id: "grok-2".to_string(),
            name: "Grok 2".to_string(),
            provider: Provider::XAI,
            tier: ModelTier::Advanced,
            pricing: ModelPricing::new(0.002, 0.010),
            performance: ModelPerformance::new(900.0, 1800.0, 3200.0, 100.0, 128000, 8192),
            capabilities: ModelCapabilities::standard(),
            quality_score: 0.88,
            release_date: Some("2024-08".to_string()),
            available: true,
        });
    }

    fn register_deepseek_models(&mut self) {
        // Gemma 3 (Future - Google/DeepMind)
        self.register(ModelDefinition {
            id: "gemma-3".to_string(),
            name: "Gemma 3".to_string(),
            provider: Provider::Google, // Gemma is from Google
            tier: ModelTier::Standard,
            pricing: ModelPricing::new(0.0, 0.0), // Open source
            performance: ModelPerformance::new(400.0, 800.0, 1400.0, 170.0, 128000, 8192),
            capabilities: ModelCapabilities::standard(),
            quality_score: 0.83,
            release_date: Some("2025-Q2".to_string()),
            available: false,
        });

        // DeepSeek V3
        self.register(ModelDefinition {
            id: "deepseek-v3".to_string(),
            name: "DeepSeek V3".to_string(),
            provider: Provider::DeepSeek,
            tier: ModelTier::Advanced,
            pricing: ModelPricing::new(0.00027, 0.0011),
            performance: ModelPerformance::new(600.0, 1200.0, 2000.0, 140.0, 128000, 8192),
            capabilities: ModelCapabilities::standard(),
            quality_score: 0.89,
            release_date: Some("2024-12".to_string()),
            available: true,
        });
    }

    fn register_alibaba_models(&mut self) {
        // Qwen 3 (Future)
        self.register(ModelDefinition {
            id: "qwen-3".to_string(),
            name: "Qwen 3".to_string(),
            provider: Provider::Alibaba,
            tier: ModelTier::Advanced,
            pricing: ModelPricing::new(0.0, 0.0), // Open source
            performance: ModelPerformance::new(500.0, 1000.0, 1800.0, 150.0, 128000, 8192),
            capabilities: ModelCapabilities::advanced(),
            quality_score: 0.87,
            release_date: Some("2025-Q2".to_string()),
            available: false,
        });

        // Qwen 2.5 72B (Current)
        self.register(ModelDefinition {
            id: "qwen-2.5-72b".to_string(),
            name: "Qwen 2.5 72B".to_string(),
            provider: Provider::Alibaba,
            tier: ModelTier::Advanced,
            pricing: ModelPricing::new(0.0, 0.0), // Open source
            performance: ModelPerformance::new(700.0, 1400.0, 2400.0, 120.0, 128000, 8192),
            capabilities: ModelCapabilities::standard(),
            quality_score: 0.85,
            release_date: Some("2024-09".to_string()),
            available: true,
        });
    }
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_registry_creation() {
        let registry = ModelRegistry::new();
        assert!(!registry.all_models().is_empty());
    }

    #[test]
    fn test_all_specified_models_registered() {
        let registry = ModelRegistry::new();

        // Test that all requested models are registered
        let required_models = vec![
            "gpt-5",
            "gpt-4.5",
            "gpt-4.1",
            "gemini-2.5-pro",
            "claude-4-opus",
            "claude-sonnet-4.5",
            "llama-4-scout",
            "llama-4-maverick",
            "gemma-3",
            "mistral-large-2",
            "mixtral-8x22b",
            "qwen-3",
            "grok-4",
        ];

        for model_id in required_models {
            assert!(
                registry.get(model_id).is_some(),
                "Model {} not found in registry",
                model_id
            );
        }
    }

    #[test]
    fn test_provider_filtering() {
        let registry = ModelRegistry::new();

        let openai_models = registry.models_by_provider(Provider::OpenAI);
        assert!(!openai_models.is_empty());

        let anthropic_models = registry.models_by_provider(Provider::Anthropic);
        assert!(!anthropic_models.is_empty());
    }

    #[test]
    fn test_tier_filtering() {
        let registry = ModelRegistry::new();

        let flagship = registry.models_by_tier(ModelTier::Flagship);
        assert!(!flagship.is_empty());

        let efficient = registry.models_by_tier(ModelTier::Efficient);
        assert!(!efficient.is_empty());
    }

    #[test]
    fn test_available_models() {
        let registry = ModelRegistry::new();

        let available = registry.available_models();
        assert!(!available.is_empty());

        // Check that some future models are not available
        assert!(registry.get("gpt-5").unwrap().available == false);
        assert!(registry.get("claude-4-opus").unwrap().available == false);
    }

    #[test]
    fn test_pricing_calculation() {
        let pricing = ModelPricing::new(0.01, 0.03);
        let cost = pricing.calculate_cost(1000, 500);

        // 1000 input tokens = 1.0 * 0.01 = 0.01
        // 500 output tokens = 0.5 * 0.03 = 0.015
        // Total = 0.025
        assert!((cost - 0.025).abs() < 0.0001);
    }

    #[test]
    fn test_pareto_candidate_creation() {
        let registry = ModelRegistry::new();
        let model = registry.get("gpt-4-turbo").unwrap();

        let candidate = model.to_pareto_candidate(1000, 500).unwrap();
        assert_eq!(candidate.name, "gpt-4-turbo");
        assert!(candidate.objectives.quality > 0.9);
        assert!(candidate.objectives.cost > 0.0);
    }

    #[test]
    fn test_find_pareto_optimal_across_providers() {
        let registry = ModelRegistry::new();

        let model_ids = vec![
            "gpt-4-turbo".to_string(),
            "claude-3.5-sonnet".to_string(),
            "gemini-1.5-pro".to_string(),
            "mistral-large-2".to_string(),
            "grok-2".to_string(),
        ];

        let pareto_set = registry
            .find_pareto_optimal(&model_ids, 1000, 500)
            .unwrap();

        assert!(!pareto_set.is_empty());
        assert!(pareto_set.len() <= model_ids.len());
    }

    #[test]
    fn test_future_models_pareto_analysis() {
        let registry = ModelRegistry::new();

        // Test with future models
        let model_ids = vec![
            "gpt-5".to_string(),
            "claude-4-opus".to_string(),
            "gemini-2.5-pro".to_string(),
            "llama-4-scout".to_string(),
            "grok-4".to_string(),
        ];

        let pareto_set = registry
            .find_pareto_optimal(&model_ids, 1000, 500)
            .unwrap();

        assert!(!pareto_set.is_empty());
    }

    #[test]
    fn test_all_providers_represented() {
        let registry = ModelRegistry::new();

        for provider in Provider::all() {
            let models = registry.models_by_provider(provider);
            assert!(
                !models.is_empty(),
                "No models for provider: {:?}",
                provider
            );
        }
    }

    #[test]
    fn test_model_capabilities() {
        let registry = ModelRegistry::new();

        // Flagship models should have advanced capabilities
        let flagship = registry.get("claude-3-opus").unwrap();
        assert!(flagship.capabilities.vision);
        assert!(flagship.capabilities.function_calling);

        // Efficient models may have standard capabilities
        let efficient = registry.get("gpt-3.5-turbo").unwrap();
        assert!(efficient.tier == ModelTier::Efficient);
    }

    #[test]
    fn test_performance_characteristics() {
        let registry = ModelRegistry::new();

        for model in registry.all_models() {
            // Validate performance metrics
            assert!(model.performance.latency_p50_ms > 0.0);
            assert!(model.performance.latency_p95_ms >= model.performance.latency_p50_ms);
            assert!(model.performance.latency_p99_ms >= model.performance.latency_p95_ms);
            assert!(model.performance.tokens_per_second > 0.0);
            assert!(model.performance.max_context_tokens > 0);
            assert!(model.performance.max_output_tokens > 0);
        }
    }

    #[test]
    fn test_quality_scores_valid_range() {
        let registry = ModelRegistry::new();

        for model in registry.all_models() {
            assert!(
                model.quality_score >= 0.0 && model.quality_score <= 1.0,
                "Invalid quality score for {}: {}",
                model.name,
                model.quality_score
            );
        }
    }

    #[test]
    fn test_cost_comparison() {
        let registry = ModelRegistry::new();

        let gpt5 = registry.get("gpt-5").unwrap();
        let claude_haiku = registry.get("claude-3-haiku").unwrap();

        let gpt5_cost = gpt5.pricing.calculate_cost(1000, 1000);
        let haiku_cost = claude_haiku.pricing.calculate_cost(1000, 1000);

        // Haiku should be much cheaper
        assert!(haiku_cost < gpt5_cost);
    }
}
