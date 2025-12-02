//! Benchmark Adapters Module
//!
//! This module provides the adapter system for exposing Auto-Optimizer
//! operations as benchmark targets. It defines the `BenchTarget` trait
//! and provides a registry of all available benchmark targets.

mod pareto_benchmark;
mod model_selection_benchmark;
mod cost_scoring_benchmark;

pub use pareto_benchmark::ParetoBenchmark;
pub use model_selection_benchmark::ModelSelectionBenchmark;
pub use cost_scoring_benchmark::CostScoringBenchmark;

/// Trait for benchmark targets.
///
/// Implement this trait to expose an operation as a benchmark target.
/// Each implementation should:
/// - Return a unique identifier via `id()`
/// - Execute the benchmark and return metrics via `run()`
///
/// # Example
///
/// ```rust,ignore
/// use llm_optimizer_benchmarks::adapters::BenchTarget;
/// use serde_json::{json, Value};
///
/// struct MyBenchmark;
///
/// impl BenchTarget for MyBenchmark {
///     fn id(&self) -> &str {
///         "my-benchmark"
///     }
///
///     fn run(&self) -> Value {
///         // Execute benchmark
///         json!({
///             "success": true,
///             "operations_count": 100,
///             "throughput": 1000.0
///         })
///     }
/// }
/// ```
pub trait BenchTarget: Send + Sync {
    /// Returns the unique identifier for this benchmark target.
    ///
    /// This ID is used in results and for selecting specific benchmarks.
    /// Convention: use kebab-case (e.g., "pareto-optimization").
    fn id(&self) -> &str;

    /// Execute the benchmark and return metrics.
    ///
    /// The returned JSON value should be an object containing:
    /// - `success`: bool (optional, defaults to true)
    /// - Any benchmark-specific metrics
    ///
    /// Note: `execution_time_ms` is automatically added by the runner.
    fn run(&self) -> serde_json::Value;

    /// Returns a description of what this benchmark tests.
    fn description(&self) -> &str {
        "No description provided"
    }

    /// Returns the category of this benchmark.
    fn category(&self) -> &str {
        "general"
    }
}

/// Registry of all available benchmark targets.
///
/// This function returns a vector of all benchmark target implementations.
/// Add new benchmark targets here to include them in `run_all_benchmarks()`.
///
/// # Returns
///
/// A vector of boxed `BenchTarget` trait objects.
pub fn all_targets() -> Vec<Box<dyn BenchTarget>> {
    vec![
        Box::new(ParetoBenchmark::new()),
        Box::new(ModelSelectionBenchmark::new()),
        Box::new(CostScoringBenchmark::new()),
    ]
}

/// Get a benchmark target by ID.
///
/// # Arguments
///
/// * `id` - The benchmark target identifier
///
/// # Returns
///
/// `Some(Box<dyn BenchTarget>)` if found, `None` otherwise.
pub fn get_target(id: &str) -> Option<Box<dyn BenchTarget>> {
    all_targets().into_iter().find(|t| t.id() == id)
}

/// List all available target IDs with descriptions.
///
/// # Returns
///
/// Vector of (id, description, category) tuples.
pub fn list_targets_info() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            ParetoBenchmark::ID,
            ParetoBenchmark::DESCRIPTION,
            ParetoBenchmark::CATEGORY,
        ),
        (
            ModelSelectionBenchmark::ID,
            ModelSelectionBenchmark::DESCRIPTION,
            ModelSelectionBenchmark::CATEGORY,
        ),
        (
            CostScoringBenchmark::ID,
            CostScoringBenchmark::DESCRIPTION,
            CostScoringBenchmark::CATEGORY,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_targets_not_empty() {
        let targets = all_targets();
        assert!(!targets.is_empty(), "Should have registered benchmark targets");
    }

    #[test]
    fn test_unique_ids() {
        let targets = all_targets();
        let mut ids: Vec<_> = targets.iter().map(|t| t.id()).collect();
        let original_len = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), original_len, "All target IDs should be unique");
    }

    #[test]
    fn test_all_targets_return_valid_json() {
        let targets = all_targets();
        for target in targets {
            let result = target.run();
            assert!(
                result.is_object(),
                "Target {} should return a JSON object",
                target.id()
            );
        }
    }

    #[test]
    fn test_get_target() {
        let target = get_target("pareto-optimization");
        assert!(target.is_some());
        assert_eq!(target.unwrap().id(), "pareto-optimization");
    }

    #[test]
    fn test_get_nonexistent_target() {
        let target = get_target("nonexistent-target");
        assert!(target.is_none());
    }

    #[test]
    fn test_list_targets_info() {
        let info = list_targets_info();
        assert!(!info.is_empty());

        for (id, description, category) in info {
            assert!(!id.is_empty());
            assert!(!description.is_empty());
            assert!(!category.is_empty());
        }
    }
}
