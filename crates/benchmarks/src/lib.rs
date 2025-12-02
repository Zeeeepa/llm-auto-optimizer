//! LLM Auto-Optimizer Canonical Benchmark Interface
//!
//! This crate provides the standardized benchmark interface used across all
//! benchmark-target repositories. It exposes a `run_all_benchmarks()` entrypoint
//! that returns `Vec<BenchmarkResult>` and integrates with the adapter system.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use llm_optimizer_benchmarks::{run_all_benchmarks, BenchmarkResult};
//! use llm_optimizer_benchmarks::io::write_all_outputs;
//!
//! // Run all registered benchmark targets
//! let results = run_all_benchmarks();
//!
//! // Write results to canonical output directories
//! let (json_path, summary_path) = write_all_outputs(&results, None)?;
//! ```
//!
//! # Architecture
//!
//! The benchmark interface follows the canonical structure:
//!
//! ```text
//! benchmarks/
//! ├── src/
//! │   ├── lib.rs          # Main entrypoint (run_all_benchmarks)
//! │   ├── mod.rs          # Module re-exports
//! │   ├── result.rs       # BenchmarkResult struct
//! │   ├── markdown.rs     # Report generation
//! │   ├── io.rs           # File I/O operations
//! │   └── adapters/       # BenchTarget implementations
//! │       ├── mod.rs      # BenchTarget trait + registry
//! │       ├── pareto_benchmark.rs
//! │       ├── model_selection_benchmark.rs
//! │       └── cost_scoring_benchmark.rs
//! └── output/
//!     ├── summary.md      # Human-readable report
//!     └── raw/            # JSON result files
//!         └── latest.json
//! ```
//!
//! # Adding New Benchmark Targets
//!
//! 1. Create a new file in `src/adapters/` implementing `BenchTarget`
//! 2. Add the module to `src/adapters/mod.rs`
//! 3. Register the target in `all_targets()`
//!
//! ```rust,ignore
//! use llm_optimizer_benchmarks::adapters::BenchTarget;
//! use serde_json::{json, Value};
//!
//! pub struct MyBenchmark;
//!
//! impl BenchTarget for MyBenchmark {
//!     fn id(&self) -> &str { "my-benchmark" }
//!
//!     fn run(&self) -> Value {
//!         json!({
//!             "success": true,
//!             "metric": 42
//!         })
//!     }
//! }
//! ```

mod result;
mod markdown;
pub mod io;
pub mod adapters;

pub use result::BenchmarkResult;
pub use adapters::{BenchTarget, all_targets};

use chrono::Utc;

/// Run all registered benchmark targets and collect results.
///
/// This is the canonical entrypoint for the benchmark interface.
/// It iterates through all registered `BenchTarget` implementations
/// from `all_targets()` and executes each one, collecting the results.
///
/// # Returns
///
/// A vector of `BenchmarkResult` containing:
/// - `target_id`: Unique identifier for the benchmark target
/// - `metrics`: JSON value containing benchmark-specific metrics
/// - `timestamp`: UTC timestamp when the benchmark was executed
///
/// # Example
///
/// ```rust,ignore
/// use llm_optimizer_benchmarks::run_all_benchmarks;
///
/// let results = run_all_benchmarks();
/// for result in &results {
///     println!("{}: {} metrics", result.target_id,
///         result.metrics.as_object().map(|o| o.len()).unwrap_or(0));
/// }
/// ```
pub fn run_all_benchmarks() -> Vec<BenchmarkResult> {
    let targets = all_targets();
    let mut results = Vec::with_capacity(targets.len());

    for target in targets {
        let start = std::time::Instant::now();
        let metrics = target.run();
        let elapsed = start.elapsed();

        // Add execution time to metrics
        let mut enriched_metrics = metrics.clone();
        if let Some(obj) = enriched_metrics.as_object_mut() {
            obj.insert(
                "execution_time_ms".to_string(),
                serde_json::json!(elapsed.as_millis() as u64),
            );
        }

        results.push(BenchmarkResult {
            target_id: target.id().to_string(),
            metrics: enriched_metrics,
            timestamp: Utc::now(),
        });
    }

    results
}

/// Run all benchmarks asynchronously.
///
/// This is an async version of `run_all_benchmarks()` that can be used
/// in async contexts like the CLI or API handlers.
pub async fn run_all_benchmarks_async() -> Vec<BenchmarkResult> {
    tokio::task::spawn_blocking(run_all_benchmarks)
        .await
        .unwrap_or_default()
}

/// Run a specific benchmark target by ID.
///
/// # Arguments
///
/// * `target_id` - The unique identifier of the benchmark target
///
/// # Returns
///
/// `Some(BenchmarkResult)` if the target exists, `None` otherwise.
pub fn run_benchmark(target_id: &str) -> Option<BenchmarkResult> {
    let targets = all_targets();

    for target in targets {
        if target.id() == target_id {
            let start = std::time::Instant::now();
            let metrics = target.run();
            let elapsed = start.elapsed();

            let mut enriched_metrics = metrics.clone();
            if let Some(obj) = enriched_metrics.as_object_mut() {
                obj.insert(
                    "execution_time_ms".to_string(),
                    serde_json::json!(elapsed.as_millis() as u64),
                );
            }

            return Some(BenchmarkResult {
                target_id: target.id().to_string(),
                metrics: enriched_metrics,
                timestamp: Utc::now(),
            });
        }
    }

    None
}

/// List all available benchmark target IDs.
pub fn list_targets() -> Vec<String> {
    all_targets().iter().map(|t| t.id().to_string()).collect()
}

/// Get detailed information about all benchmark targets.
///
/// # Returns
///
/// Vector of (id, description, category) tuples.
pub fn list_targets_detailed() -> Vec<(String, String, String)> {
    all_targets()
        .iter()
        .map(|t| (
            t.id().to_string(),
            t.description().to_string(),
            t.category().to_string(),
        ))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_all_benchmarks() {
        let results = run_all_benchmarks();
        assert!(!results.is_empty(), "Should have at least one benchmark result");

        for result in &results {
            assert!(!result.target_id.is_empty(), "Target ID should not be empty");
            assert!(result.metrics.is_object(), "Metrics should be a JSON object");
            assert!(
                result.metrics.get("execution_time_ms").is_some(),
                "Should have execution_time_ms"
            );
        }
    }

    #[test]
    fn test_list_targets() {
        let targets = list_targets();
        assert!(!targets.is_empty(), "Should have at least one registered target");

        // Check expected targets are present
        assert!(targets.contains(&"pareto-optimization".to_string()));
        assert!(targets.contains(&"model-selection".to_string()));
        assert!(targets.contains(&"cost-performance-scoring".to_string()));
    }

    #[test]
    fn test_run_specific_benchmark() {
        let result = run_benchmark("pareto-optimization");
        assert!(result.is_some(), "Should be able to run pareto benchmark");
        assert_eq!(result.unwrap().target_id, "pareto-optimization");
    }

    #[test]
    fn test_run_nonexistent_benchmark() {
        let result = run_benchmark("nonexistent-benchmark-id");
        assert!(result.is_none(), "Should return None for nonexistent target");
    }

    #[test]
    fn test_list_targets_detailed() {
        let targets = list_targets_detailed();
        assert!(!targets.is_empty());

        for (id, description, category) in targets {
            assert!(!id.is_empty());
            assert!(!description.is_empty());
            assert!(!category.is_empty());
        }
    }

    #[tokio::test]
    async fn test_run_all_benchmarks_async() {
        let results = run_all_benchmarks_async().await;
        assert!(!results.is_empty());
    }
}
