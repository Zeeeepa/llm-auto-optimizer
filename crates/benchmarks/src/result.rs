//! Benchmark Result Types
//!
//! This module defines the canonical `BenchmarkResult` struct that is used
//! across all benchmark-target repositories for standardized result reporting.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Canonical benchmark result structure.
///
/// This struct represents the standardized output format for all benchmark
/// executions across the benchmark-target repositories.
///
/// # Fields
///
/// * `target_id` - Unique identifier for the benchmark target (e.g., "pareto-optimization")
/// * `metrics` - JSON value containing benchmark-specific metrics and measurements
/// * `timestamp` - UTC timestamp when the benchmark was executed
///
/// # Example
///
/// ```rust
/// use llm_optimizer_benchmarks::BenchmarkResult;
/// use chrono::Utc;
/// use serde_json::json;
///
/// let result = BenchmarkResult {
///     target_id: "model-selection".to_string(),
///     metrics: json!({
///         "candidates_evaluated": 10,
///         "pareto_optimal_count": 3,
///         "execution_time_ms": 42
///     }),
///     timestamp: Utc::now(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// Unique identifier for the benchmark target
    pub target_id: String,

    /// JSON object containing benchmark metrics
    pub metrics: serde_json::Value,

    /// UTC timestamp of benchmark execution
    pub timestamp: DateTime<Utc>,
}

impl BenchmarkResult {
    /// Create a new benchmark result.
    ///
    /// # Arguments
    ///
    /// * `target_id` - Unique identifier for the benchmark target
    /// * `metrics` - JSON value containing benchmark metrics
    ///
    /// # Returns
    ///
    /// A new `BenchmarkResult` with the current UTC timestamp.
    pub fn new(target_id: impl Into<String>, metrics: serde_json::Value) -> Self {
        Self {
            target_id: target_id.into(),
            metrics,
            timestamp: Utc::now(),
        }
    }

    /// Create a benchmark result with a specific timestamp.
    ///
    /// # Arguments
    ///
    /// * `target_id` - Unique identifier for the benchmark target
    /// * `metrics` - JSON value containing benchmark metrics
    /// * `timestamp` - Specific UTC timestamp to use
    pub fn with_timestamp(
        target_id: impl Into<String>,
        metrics: serde_json::Value,
        timestamp: DateTime<Utc>,
    ) -> Self {
        Self {
            target_id: target_id.into(),
            metrics,
            timestamp,
        }
    }

    /// Get a metric value by key.
    ///
    /// # Arguments
    ///
    /// * `key` - The metric key to retrieve
    ///
    /// # Returns
    ///
    /// `Some(&Value)` if the metric exists, `None` otherwise.
    pub fn get_metric(&self, key: &str) -> Option<&serde_json::Value> {
        self.metrics.get(key)
    }

    /// Get a metric as a specific type.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The type to deserialize the metric into
    ///
    /// # Arguments
    ///
    /// * `key` - The metric key to retrieve
    ///
    /// # Returns
    ///
    /// `Some(T)` if the metric exists and can be deserialized, `None` otherwise.
    pub fn get_metric_as<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.metrics
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Check if the result indicates success.
    ///
    /// Returns `true` if there's a "success" metric set to `true`,
    /// or if there's no explicit failure indication.
    pub fn is_success(&self) -> bool {
        self.metrics
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    }

    /// Get execution time in milliseconds if available.
    pub fn execution_time_ms(&self) -> Option<u64> {
        self.get_metric_as::<u64>("execution_time_ms")
    }

    /// Convert to a summary string for logging.
    pub fn to_summary(&self) -> String {
        let exec_time = self
            .execution_time_ms()
            .map(|t| format!(" ({}ms)", t))
            .unwrap_or_default();

        let status = if self.is_success() { "✓" } else { "✗" };

        format!(
            "{} {}{} @ {}",
            status,
            self.target_id,
            exec_time,
            self.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
        )
    }
}

impl std::fmt::Display for BenchmarkResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_summary())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_benchmark_result_new() {
        let result = BenchmarkResult::new(
            "test-target",
            json!({
                "metric1": 42,
                "metric2": "value"
            }),
        );

        assert_eq!(result.target_id, "test-target");
        assert_eq!(result.get_metric("metric1"), Some(&json!(42)));
        assert_eq!(result.get_metric("metric2"), Some(&json!("value")));
    }

    #[test]
    fn test_get_metric_as() {
        let result = BenchmarkResult::new(
            "test",
            json!({
                "count": 10,
                "ratio": 0.75,
                "name": "test"
            }),
        );

        assert_eq!(result.get_metric_as::<i64>("count"), Some(10));
        assert_eq!(result.get_metric_as::<f64>("ratio"), Some(0.75));
        assert_eq!(result.get_metric_as::<String>("name"), Some("test".to_string()));
        assert_eq!(result.get_metric_as::<i64>("nonexistent"), None);
    }

    #[test]
    fn test_is_success() {
        let success = BenchmarkResult::new("test", json!({ "success": true }));
        assert!(success.is_success());

        let failure = BenchmarkResult::new("test", json!({ "success": false }));
        assert!(!failure.is_success());

        let no_indicator = BenchmarkResult::new("test", json!({ "other": "data" }));
        assert!(no_indicator.is_success()); // Default to success
    }

    #[test]
    fn test_execution_time() {
        let with_time = BenchmarkResult::new(
            "test",
            json!({ "execution_time_ms": 100 }),
        );
        assert_eq!(with_time.execution_time_ms(), Some(100));

        let without_time = BenchmarkResult::new("test", json!({}));
        assert_eq!(without_time.execution_time_ms(), None);
    }

    #[test]
    fn test_serialization() {
        let result = BenchmarkResult::new(
            "test-target",
            json!({ "value": 42 }),
        );

        let serialized = serde_json::to_string(&result).unwrap();
        let deserialized: BenchmarkResult = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.target_id, result.target_id);
        assert_eq!(deserialized.metrics, result.metrics);
    }

    #[test]
    fn test_display() {
        let result = BenchmarkResult::new(
            "test-target",
            json!({ "execution_time_ms": 50, "success": true }),
        );

        let display = format!("{}", result);
        assert!(display.contains("test-target"));
        assert!(display.contains("50ms"));
        assert!(display.contains("✓"));
    }
}
