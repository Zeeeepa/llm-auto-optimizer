//! Model performance and configuration types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Model performance profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPerformanceProfile {
    /// Unique model identifier
    pub model_id: String,
    /// Provider name (e.g., "anthropic", "openai")
    pub provider: String,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// 50th percentile latency
    pub p50_latency_ms: f64,
    /// 95th percentile latency
    pub p95_latency_ms: f64,
    /// 99th percentile latency
    pub p99_latency_ms: f64,
    /// Cost per 1k tokens
    pub cost_per_1k_tokens: f64,
    /// Composite quality score (0.0-1.0)
    pub quality_score: f64,
    /// Error rate (0.0-1.0)
    pub error_rate: f64,
    /// Throughput in queries per second
    pub throughput_qps: f64,
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl ModelPerformanceProfile {
    /// Create a new model performance profile
    pub fn new(model_id: impl Into<String>, provider: impl Into<String>) -> Self {
        Self {
            model_id: model_id.into(),
            provider: provider.into(),
            avg_latency_ms: 0.0,
            p50_latency_ms: 0.0,
            p95_latency_ms: 0.0,
            p99_latency_ms: 0.0,
            cost_per_1k_tokens: 0.0,
            quality_score: 0.0,
            error_rate: 0.0,
            throughput_qps: 0.0,
            last_updated: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// Calculate cost per request based on token count
    pub fn cost_per_request(&self, tokens: u32) -> f64 {
        (tokens as f64 / 1000.0) * self.cost_per_1k_tokens
    }

    /// Check if model meets latency SLA
    pub fn meets_latency_sla(&self, max_p95_ms: f64) -> bool {
        self.p95_latency_ms <= max_p95_ms
    }

    /// Check if model meets quality threshold
    pub fn meets_quality_threshold(&self, min_score: f64) -> bool {
        self.quality_score >= min_score
    }
}

/// Model configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Model identifier
    pub model: String,
    /// Temperature (0.0-1.0)
    pub temperature: f64,
    /// Top-p nucleus sampling (0.0-1.0)
    pub top_p: f64,
    /// Top-k sampling
    pub top_k: Option<u32>,
    /// Maximum output tokens
    pub max_tokens: u32,
    /// Presence penalty
    pub presence_penalty: Option<f64>,
    /// Frequency penalty
    pub frequency_penalty: Option<f64>,
    /// System prompt
    pub system_prompt: Option<String>,
    /// Additional provider-specific parameters
    pub extra_params: HashMap<String, serde_json::Value>,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            model: String::new(),
            temperature: 0.7,
            top_p: 0.9,
            top_k: None,
            max_tokens: 1024,
            presence_penalty: None,
            frequency_penalty: None,
            system_prompt: None,
            extra_params: HashMap::new(),
        }
    }
}

impl ModelConfig {
    /// Create configuration for classification tasks
    pub fn for_classification(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            temperature: 0.1,
            top_p: 0.9,
            max_tokens: 50,
            ..Default::default()
        }
    }

    /// Create configuration for creative generation
    pub fn for_creative_generation(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            temperature: 0.8,
            top_p: 0.95,
            max_tokens: 500,
            presence_penalty: Some(0.6),
            ..Default::default()
        }
    }

    /// Create configuration for data extraction
    pub fn for_data_extraction(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            temperature: 0.0,
            top_p: 0.9,
            max_tokens: 200,
            ..Default::default()
        }
    }
}

/// Task type classification
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    Classification,
    CreativeGeneration,
    DataExtraction,
    Summarization,
    QuestionAnswering,
    CodeGeneration,
    Translation,
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_performance_profile() {
        let mut profile = ModelPerformanceProfile::new("claude-3-opus", "anthropic");
        profile.p95_latency_ms = 1500.0;
        profile.quality_score = 0.92;
        profile.cost_per_1k_tokens = 15.0;

        assert!(profile.meets_latency_sla(2000.0));
        assert!(!profile.meets_latency_sla(1000.0));
        assert!(profile.meets_quality_threshold(0.9));
        assert!(!profile.meets_quality_threshold(0.95));
    }

    #[test]
    fn test_cost_calculation() {
        let profile = ModelPerformanceProfile {
            cost_per_1k_tokens: 15.0,
            ..ModelPerformanceProfile::new("test-model", "test-provider")
        };

        assert_eq!(profile.cost_per_request(1000), 15.0);
        assert_eq!(profile.cost_per_request(500), 7.5);
    }

    #[test]
    fn test_model_config_presets() {
        let classification = ModelConfig::for_classification("model-1");
        assert_eq!(classification.temperature, 0.1);
        assert_eq!(classification.max_tokens, 50);

        let creative = ModelConfig::for_creative_generation("model-2");
        assert_eq!(creative.temperature, 0.8);
        assert_eq!(creative.presence_penalty, Some(0.6));

        let extraction = ModelConfig::for_data_extraction("model-3");
        assert_eq!(extraction.temperature, 0.0);
    }
}
