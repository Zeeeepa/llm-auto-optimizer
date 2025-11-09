//! Metrics and measurement types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Aggregation type for metrics
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AggregationType {
    Sum,
    Average,
    Min,
    Max,
    Count,
    Histogram,
    Percentile,
}

/// Metric aggregation data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricAggregation {
    /// Type of aggregation
    pub aggregation_type: AggregationType,
    /// Number of samples
    pub count: u64,
    /// Sum of values
    pub sum: f64,
    /// Minimum value
    pub min: f64,
    /// Maximum value
    pub max: f64,
    /// Percentile values (p50, p95, p99, etc.)
    pub percentiles: Vec<(u8, f64)>,
}

impl MetricAggregation {
    /// Calculate average
    pub fn average(&self) -> f64 {
        if self.count > 0 {
            self.sum / self.count as f64
        } else {
            0.0
        }
    }

    /// Get specific percentile value
    pub fn get_percentile(&self, percentile: u8) -> Option<f64> {
        self.percentiles
            .iter()
            .find(|(p, _)| *p == percentile)
            .map(|(_, v)| *v)
    }
}

/// Time-series metric point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricPoint {
    /// Metric name
    pub name: String,
    /// Metric value
    pub value: f64,
    /// Unit of measurement
    pub unit: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Tags/labels
    pub tags: std::collections::HashMap<String, String>,
    /// Aggregation data (if applicable)
    pub aggregation: Option<MetricAggregation>,
}

impl MetricPoint {
    /// Create a new metric point
    pub fn new(name: impl Into<String>, value: f64, unit: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value,
            unit: unit.into(),
            timestamp: Utc::now(),
            tags: std::collections::HashMap::new(),
            aggregation: None,
        }
    }

    /// Add a tag
    pub fn with_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.insert(key.into(), value.into());
        self
    }

    /// Add aggregation data
    pub fn with_aggregation(mut self, aggregation: MetricAggregation) -> Self {
        self.aggregation = Some(aggregation);
        self
    }
}

/// Performance metrics summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// p50 latency
    pub p50_latency_ms: f64,
    /// p95 latency
    pub p95_latency_ms: f64,
    /// p99 latency
    pub p99_latency_ms: f64,
    /// Error rate (0.0-1.0)
    pub error_rate: f64,
    /// Throughput (requests per second)
    pub throughput_qps: f64,
    /// Total requests
    pub total_requests: u64,
    /// Time period
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
}

/// Cost metrics summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostMetrics {
    /// Total cost
    pub total_cost: f64,
    /// Average cost per request
    pub avg_cost_per_request: f64,
    /// Total tokens used
    pub total_tokens: u64,
    /// Input tokens
    pub input_tokens: u64,
    /// Output tokens
    pub output_tokens: u64,
    /// Time period
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
}

/// Quality metrics summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityMetrics {
    /// Overall quality score (0.0-1.0)
    pub overall_score: f64,
    /// Accuracy score
    pub accuracy: f64,
    /// Relevance score
    pub relevance: f64,
    /// Coherence score
    pub coherence: f64,
    /// User satisfaction (if available)
    pub user_satisfaction: Option<f64>,
    /// Time period
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_aggregation() {
        let agg = MetricAggregation {
            aggregation_type: AggregationType::Histogram,
            count: 1000,
            sum: 125000.0,
            min: 50.0,
            max: 500.0,
            percentiles: vec![(50, 100.0), (95, 250.0), (99, 400.0)],
        };

        assert_eq!(agg.average(), 125.0);
        assert_eq!(agg.get_percentile(95), Some(250.0));
        assert_eq!(agg.get_percentile(75), None);
    }

    #[test]
    fn test_metric_point_creation() {
        let point = MetricPoint::new("latency", 123.45, "ms")
            .with_tag("service", "test-service")
            .with_tag("model", "claude-3-opus");

        assert_eq!(point.name, "latency");
        assert_eq!(point.value, 123.45);
        assert_eq!(point.tags.len(), 2);
    }
}
