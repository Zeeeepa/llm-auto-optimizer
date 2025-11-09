//! A/B testing and experiment types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::models::ModelConfig;

/// Status of an experiment
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentStatus {
    Draft,
    Running,
    Paused,
    Completed,
    Cancelled,
}

/// Metric definition for experiment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDefinition {
    /// Metric name
    pub name: String,
    /// Metric type
    pub metric_type: MetricType,
    /// Whether higher is better
    pub higher_is_better: bool,
    /// Minimum sample size required
    pub min_sample_size: usize,
}

/// Type of metric
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MetricType {
    ConversionRate,
    AverageValue,
    Count,
    Percentile,
}

/// A single variant in an A/B test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variant {
    /// Unique variant identifier
    pub id: Uuid,
    /// Variant name (e.g., "control", "variant_a")
    pub name: String,
    /// Configuration for this variant
    pub config: ModelConfig,
    /// Traffic allocation (0.0-1.0)
    pub traffic_allocation: f64,
    /// Results for this variant
    pub results: Option<VariantResults>,
}

impl Variant {
    /// Create a new variant
    pub fn new(name: impl Into<String>, config: ModelConfig, traffic_allocation: f64) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            config,
            traffic_allocation: traffic_allocation.clamp(0.0, 1.0),
            results: None,
        }
    }

    /// Get conversion rate if available
    pub fn conversion_rate(&self) -> Option<f64> {
        self.results.as_ref().map(|r| {
            if r.total_requests > 0 {
                r.conversions as f64 / r.total_requests as f64
            } else {
                0.0
            }
        })
    }
}

/// Results for a single variant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantResults {
    /// Total requests served
    pub total_requests: u64,
    /// Number of conversions
    pub conversions: u64,
    /// Average quality score
    pub avg_quality: f64,
    /// Average cost per request
    pub avg_cost: f64,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// Per-metric results
    pub metrics: HashMap<String, f64>,
}

/// Statistical analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticalAnalysis {
    /// Winning variant ID (if statistically significant)
    pub winner_variant_id: Option<Uuid>,
    /// P-value from statistical test
    pub p_value: f64,
    /// Confidence level (e.g., 0.95 for 95%)
    pub confidence_level: f64,
    /// Effect size
    pub effect_size: f64,
    /// Whether result is statistically significant
    pub is_significant: bool,
    /// Analysis method used
    pub method: String,
}

/// Complete experiment results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentResults {
    /// Statistical analysis
    pub statistical_analysis: StatisticalAnalysis,
    /// Per-variant details
    pub variant_details: HashMap<Uuid, VariantResults>,
    /// Overall sample size
    pub total_sample_size: u64,
    /// Experiment duration in seconds
    pub duration_seconds: u64,
}

/// A/B test experiment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experiment {
    /// Unique experiment identifier
    pub id: Uuid,
    /// Experiment name
    pub name: String,
    /// Current status
    pub status: ExperimentStatus,
    /// Variants being tested
    pub variants: Vec<Variant>,
    /// Traffic allocation per variant
    pub traffic_allocation: HashMap<String, f64>,
    /// Metrics being tracked
    pub metrics: Vec<MetricDefinition>,
    /// Start time
    pub start_time: DateTime<Utc>,
    /// End time (if completed)
    pub end_time: Option<DateTime<Utc>>,
    /// Results (if completed)
    pub results: Option<ExperimentResults>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl Experiment {
    /// Create a new experiment
    pub fn new(name: impl Into<String>, variants: Vec<Variant>) -> Self {
        let traffic_allocation = variants
            .iter()
            .map(|v| (v.name.clone(), v.traffic_allocation))
            .collect();

        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            status: ExperimentStatus::Draft,
            variants,
            traffic_allocation,
            metrics: Vec::new(),
            start_time: Utc::now(),
            end_time: None,
            results: None,
            metadata: HashMap::new(),
        }
    }

    /// Add a metric to track
    pub fn with_metric(mut self, metric: MetricDefinition) -> Self {
        self.metrics.push(metric);
        self
    }

    /// Start the experiment
    pub fn start(&mut self) {
        self.status = ExperimentStatus::Running;
        self.start_time = Utc::now();
    }

    /// Complete the experiment
    pub fn complete(&mut self, results: ExperimentResults) {
        self.status = ExperimentStatus::Completed;
        self.end_time = Some(Utc::now());
        self.results = Some(results);
    }

    /// Get the winning variant if available
    pub fn get_winner(&self) -> Option<&Variant> {
        self.results.as_ref().and_then(|results| {
            results
                .statistical_analysis
                .winner_variant_id
                .and_then(|id| self.variants.iter().find(|v| v.id == id))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_experiment_creation() {
        let control = Variant::new(
            "control",
            ModelConfig::default(),
            0.5,
        );
        let variant_a = Variant::new(
            "variant_a",
            ModelConfig::default(),
            0.5,
        );

        let experiment = Experiment::new("Test Experiment", vec![control, variant_a]);

        assert_eq!(experiment.status, ExperimentStatus::Draft);
        assert_eq!(experiment.variants.len(), 2);
    }

    #[test]
    fn test_variant_conversion_rate() {
        let mut variant = Variant::new("test", ModelConfig::default(), 0.5);

        variant.results = Some(VariantResults {
            total_requests: 1000,
            conversions: 750,
            avg_quality: 0.9,
            avg_cost: 0.05,
            avg_latency_ms: 1200.0,
            metrics: HashMap::new(),
        });

        assert_eq!(variant.conversion_rate(), Some(0.75));
    }
}
