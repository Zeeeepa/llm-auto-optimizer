//! Model Selection Benchmark
//!
//! This benchmark target evaluates the model selection and registry
//! operations of the LLM Auto-Optimizer, testing performance of
//! model lookups, filtering, and provider comparisons.

use super::BenchTarget;
use serde_json::{json, Value};

/// Benchmark for model selection evaluation operations.
///
/// This benchmark tests:
/// - Model registry lookups by provider
/// - Model filtering by tier and capabilities
/// - Cost-based model comparisons
/// - Provider diversity analysis
pub struct ModelSelectionBenchmark {
    iteration_count: usize,
}

impl ModelSelectionBenchmark {
    /// Benchmark identifier
    pub const ID: &'static str = "model-selection";

    /// Benchmark description
    pub const DESCRIPTION: &'static str =
        "Evaluates model selection, registry lookups, and provider comparison operations";

    /// Benchmark category
    pub const CATEGORY: &'static str = "selection";

    /// Create a new model selection benchmark.
    pub fn new() -> Self {
        Self {
            iteration_count: 1000,
        }
    }

    /// Create with custom iteration count.
    pub fn with_iterations(iteration_count: usize) -> Self {
        Self { iteration_count }
    }

    /// Simulate model definitions for benchmarking.
    fn generate_model_registry() -> Vec<ModelDef> {
        vec![
            // OpenAI Models
            ModelDef::new("gpt-4o", Provider::OpenAI, Tier::Flagship, 0.005, 0.015),
            ModelDef::new("gpt-4o-mini", Provider::OpenAI, Tier::Efficient, 0.00015, 0.0006),
            ModelDef::new("gpt-4-turbo", Provider::OpenAI, Tier::Advanced, 0.01, 0.03),
            // Anthropic Models
            ModelDef::new("claude-3-5-sonnet", Provider::Anthropic, Tier::Advanced, 0.003, 0.015),
            ModelDef::new("claude-3-5-haiku", Provider::Anthropic, Tier::Efficient, 0.001, 0.005),
            ModelDef::new("claude-3-opus", Provider::Anthropic, Tier::Flagship, 0.015, 0.075),
            // Google Models
            ModelDef::new("gemini-1.5-pro", Provider::Google, Tier::Advanced, 0.00125, 0.005),
            ModelDef::new("gemini-1.5-flash", Provider::Google, Tier::Efficient, 0.000075, 0.0003),
            // Mistral Models
            ModelDef::new("mistral-large", Provider::Mistral, Tier::Advanced, 0.002, 0.006),
            ModelDef::new("mistral-small", Provider::Mistral, Tier::Efficient, 0.0002, 0.0006),
            // Meta Models
            ModelDef::new("llama-3.1-405b", Provider::Meta, Tier::Flagship, 0.003, 0.003),
            ModelDef::new("llama-3.1-70b", Provider::Meta, Tier::Advanced, 0.00059, 0.00079),
        ]
    }

    /// Filter models by provider.
    fn filter_by_provider(models: &[ModelDef], provider: Provider) -> Vec<&ModelDef> {
        models.iter().filter(|m| m.provider == provider).collect()
    }

    /// Filter models by tier.
    fn filter_by_tier(models: &[ModelDef], tier: Tier) -> Vec<&ModelDef> {
        models.iter().filter(|m| m.tier == tier).collect()
    }

    /// Find cheapest model for a given token count.
    fn find_cheapest(models: &[ModelDef], input_tokens: usize, output_tokens: usize) -> Option<&ModelDef> {
        models.iter().min_by(|a, b| {
            let cost_a = a.calculate_cost(input_tokens, output_tokens);
            let cost_b = b.calculate_cost(input_tokens, output_tokens);
            cost_a.partial_cmp(&cost_b).unwrap()
        })
    }

    /// Calculate cost statistics across all models.
    fn cost_statistics(models: &[ModelDef], input_tokens: usize, output_tokens: usize) -> CostStats {
        let costs: Vec<f64> = models
            .iter()
            .map(|m| m.calculate_cost(input_tokens, output_tokens))
            .collect();

        let min = costs.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = costs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let sum: f64 = costs.iter().sum();
        let avg = sum / costs.len() as f64;

        CostStats { min, max, avg, sum }
    }
}

impl Default for ModelSelectionBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchTarget for ModelSelectionBenchmark {
    fn id(&self) -> &str {
        Self::ID
    }

    fn run(&self) -> Value {
        let models = Self::generate_model_registry();

        // Benchmark provider filtering
        let start = std::time::Instant::now();
        for _ in 0..self.iteration_count {
            let _ = Self::filter_by_provider(&models, Provider::OpenAI);
            let _ = Self::filter_by_provider(&models, Provider::Anthropic);
            let _ = Self::filter_by_provider(&models, Provider::Google);
        }
        let provider_filter_time = start.elapsed();

        // Benchmark tier filtering
        let start = std::time::Instant::now();
        for _ in 0..self.iteration_count {
            let _ = Self::filter_by_tier(&models, Tier::Flagship);
            let _ = Self::filter_by_tier(&models, Tier::Advanced);
            let _ = Self::filter_by_tier(&models, Tier::Efficient);
        }
        let tier_filter_time = start.elapsed();

        // Benchmark cheapest model lookup
        let start = std::time::Instant::now();
        for _ in 0..self.iteration_count {
            let _ = Self::find_cheapest(&models, 1000, 500);
            let _ = Self::find_cheapest(&models, 5000, 2000);
        }
        let cheapest_lookup_time = start.elapsed();

        // Benchmark cost statistics calculation
        let start = std::time::Instant::now();
        for _ in 0..self.iteration_count {
            let _ = Self::cost_statistics(&models, 1000, 500);
        }
        let cost_stats_time = start.elapsed();

        // Calculate final statistics
        let cost_stats = Self::cost_statistics(&models, 1000, 500);
        let openai_models = Self::filter_by_provider(&models, Provider::OpenAI).len();
        let anthropic_models = Self::filter_by_provider(&models, Provider::Anthropic).len();
        let flagship_models = Self::filter_by_tier(&models, Tier::Flagship).len();
        let cheapest = Self::find_cheapest(&models, 1000, 500);

        json!({
            "success": true,
            "benchmark": Self::ID,
            "iterations": self.iteration_count,
            "model_count": models.len(),
            "timing": {
                "provider_filter_us": provider_filter_time.as_micros() as u64,
                "tier_filter_us": tier_filter_time.as_micros() as u64,
                "cheapest_lookup_us": cheapest_lookup_time.as_micros() as u64,
                "cost_stats_us": cost_stats_time.as_micros() as u64,
                "total_us": (provider_filter_time + tier_filter_time + cheapest_lookup_time + cost_stats_time).as_micros() as u64
            },
            "throughput": {
                "provider_filters_per_sec": (self.iteration_count * 3) as f64 / provider_filter_time.as_secs_f64(),
                "tier_filters_per_sec": (self.iteration_count * 3) as f64 / tier_filter_time.as_secs_f64(),
                "cheapest_lookups_per_sec": (self.iteration_count * 2) as f64 / cheapest_lookup_time.as_secs_f64()
            },
            "registry_stats": {
                "openai_models": openai_models,
                "anthropic_models": anthropic_models,
                "flagship_models": flagship_models,
                "providers_count": 5,
                "cheapest_model": cheapest.map(|m| m.name.clone())
            },
            "cost_analysis": {
                "min_cost_usd": cost_stats.min,
                "max_cost_usd": cost_stats.max,
                "avg_cost_usd": cost_stats.avg,
                "cost_ratio": cost_stats.max / cost_stats.min
            }
        })
    }

    fn description(&self) -> &str {
        Self::DESCRIPTION
    }

    fn category(&self) -> &str {
        Self::CATEGORY
    }
}

/// Model definition for benchmarking.
#[derive(Clone)]
struct ModelDef {
    name: String,
    provider: Provider,
    tier: Tier,
    input_price_per_1k: f64,
    output_price_per_1k: f64,
}

impl ModelDef {
    fn new(
        name: &str,
        provider: Provider,
        tier: Tier,
        input_price: f64,
        output_price: f64,
    ) -> Self {
        Self {
            name: name.to_string(),
            provider,
            tier,
            input_price_per_1k: input_price,
            output_price_per_1k: output_price,
        }
    }

    fn calculate_cost(&self, input_tokens: usize, output_tokens: usize) -> f64 {
        let input_cost = (input_tokens as f64 / 1000.0) * self.input_price_per_1k;
        let output_cost = (output_tokens as f64 / 1000.0) * self.output_price_per_1k;
        input_cost + output_cost
    }
}

/// LLM Provider enumeration.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Provider {
    OpenAI,
    Anthropic,
    Google,
    Mistral,
    Meta,
}

/// Model tier enumeration.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tier {
    Flagship,
    Advanced,
    Efficient,
}

/// Cost statistics structure.
struct CostStats {
    min: f64,
    max: f64,
    avg: f64,
    sum: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_selection_benchmark_runs() {
        let bench = ModelSelectionBenchmark::with_iterations(100);
        let result = bench.run();

        assert!(result.get("success").unwrap().as_bool().unwrap());
        assert!(result.get("model_count").unwrap().as_u64().unwrap() > 0);
    }

    #[test]
    fn test_cost_calculation() {
        let model = ModelDef::new("test", Provider::OpenAI, Tier::Advanced, 0.01, 0.03);
        let cost = model.calculate_cost(1000, 500);
        let expected = (1000.0 / 1000.0) * 0.01 + (500.0 / 1000.0) * 0.03;
        assert!((cost - expected).abs() < 0.0001);
    }
}
