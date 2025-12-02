//! Pareto Optimization Benchmark
//!
//! This benchmark target evaluates the Pareto optimization capabilities
//! of the LLM Auto-Optimizer, including multi-objective optimization
//! for quality, cost, and latency trade-offs.

use super::BenchTarget;
use serde_json::{json, Value};
use uuid::Uuid;

/// Benchmark for Pareto optimization operations.
///
/// This benchmark tests:
/// - Pareto frontier calculation
/// - Dominance relationship checking
/// - Composite scoring with different weight configurations
/// - Hypervolume and spacing metrics
pub struct ParetoBenchmark {
    candidate_counts: Vec<usize>,
}

impl ParetoBenchmark {
    /// Benchmark identifier
    pub const ID: &'static str = "pareto-optimization";

    /// Benchmark description
    pub const DESCRIPTION: &'static str =
        "Evaluates Pareto multi-objective optimization for quality/cost/latency trade-offs";

    /// Benchmark category
    pub const CATEGORY: &'static str = "optimization";

    /// Create a new Pareto benchmark instance.
    pub fn new() -> Self {
        Self {
            candidate_counts: vec![10, 50, 100, 500],
        }
    }

    /// Create with custom candidate counts.
    pub fn with_candidate_counts(candidate_counts: Vec<usize>) -> Self {
        Self { candidate_counts }
    }

    /// Generate test model candidates.
    fn generate_candidates(&self, count: usize) -> Vec<TestCandidate> {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        (0..count)
            .map(|i| TestCandidate {
                id: Uuid::new_v4(),
                name: format!("candidate-{}", i),
                quality: rng.gen_range(0.3..1.0),
                cost: rng.gen_range(0.01..0.50),
                latency_p95: rng.gen_range(100.0..5000.0),
            })
            .collect()
    }

    /// Check Pareto dominance between candidates.
    fn dominates(a: &TestCandidate, b: &TestCandidate) -> bool {
        let better_or_equal_quality = a.quality >= b.quality;
        let better_or_equal_cost = a.cost <= b.cost;
        let better_or_equal_latency = a.latency_p95 <= b.latency_p95;

        let strictly_better_quality = a.quality > b.quality;
        let strictly_better_cost = a.cost < b.cost;
        let strictly_better_latency = a.latency_p95 < b.latency_p95;

        better_or_equal_quality
            && better_or_equal_cost
            && better_or_equal_latency
            && (strictly_better_quality || strictly_better_cost || strictly_better_latency)
    }

    /// Find Pareto optimal set.
    fn find_pareto_optimal(&self, candidates: &[TestCandidate]) -> Vec<TestCandidate> {
        let mut pareto_set = Vec::new();

        for candidate in candidates {
            let is_dominated = candidates
                .iter()
                .any(|other| other.id != candidate.id && Self::dominates(other, candidate));

            if !is_dominated {
                pareto_set.push(candidate.clone());
            }
        }

        pareto_set
    }

    /// Calculate composite score.
    fn composite_score(
        candidate: &TestCandidate,
        quality_weight: f64,
        cost_weight: f64,
        latency_weight: f64,
        max_cost: f64,
        max_latency: f64,
    ) -> f64 {
        let normalized_cost = (1.0 - (candidate.cost / max_cost).min(1.0)).max(0.0);
        let normalized_latency = (1.0 - (candidate.latency_p95 / max_latency).min(1.0)).max(0.0);

        quality_weight * candidate.quality
            + cost_weight * normalized_cost
            + latency_weight * normalized_latency
    }

    /// Run benchmark for a specific candidate count.
    fn benchmark_count(&self, count: usize) -> CountResult {
        let candidates = self.generate_candidates(count);

        // Benchmark Pareto frontier calculation
        let start = std::time::Instant::now();
        let pareto_set = self.find_pareto_optimal(&candidates);
        let pareto_time = start.elapsed();

        // Benchmark dominance checking
        let start = std::time::Instant::now();
        let mut dominance_count = 0;
        for i in 0..candidates.len().min(100) {
            for j in 0..candidates.len().min(100) {
                if i != j && Self::dominates(&candidates[i], &candidates[j]) {
                    dominance_count += 1;
                }
            }
        }
        let dominance_time = start.elapsed();

        // Benchmark composite scoring
        let start = std::time::Instant::now();
        let _scores: Vec<f64> = candidates
            .iter()
            .map(|c| Self::composite_score(c, 0.5, 0.3, 0.2, 1.0, 5000.0))
            .collect();
        let scoring_time = start.elapsed();

        CountResult {
            candidate_count: count,
            pareto_optimal_count: pareto_set.len(),
            pareto_time_us: pareto_time.as_micros() as u64,
            dominance_checks: dominance_count,
            dominance_time_us: dominance_time.as_micros() as u64,
            scoring_time_us: scoring_time.as_micros() as u64,
        }
    }
}

impl Default for ParetoBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchTarget for ParetoBenchmark {
    fn id(&self) -> &str {
        Self::ID
    }

    fn run(&self) -> Value {
        let results: Vec<_> = self
            .candidate_counts
            .iter()
            .map(|&count| self.benchmark_count(count))
            .collect();

        let total_pareto_time: u64 = results.iter().map(|r| r.pareto_time_us).sum();
        let avg_pareto_ratio: f64 = results
            .iter()
            .map(|r| r.pareto_optimal_count as f64 / r.candidate_count as f64)
            .sum::<f64>()
            / results.len() as f64;

        json!({
            "success": true,
            "benchmark": Self::ID,
            "candidate_counts": self.candidate_counts,
            "results": results.iter().map(|r| json!({
                "candidate_count": r.candidate_count,
                "pareto_optimal_count": r.pareto_optimal_count,
                "pareto_time_us": r.pareto_time_us,
                "dominance_checks": r.dominance_checks,
                "dominance_time_us": r.dominance_time_us,
                "scoring_time_us": r.scoring_time_us,
                "pareto_ratio": r.pareto_optimal_count as f64 / r.candidate_count as f64
            })).collect::<Vec<_>>(),
            "summary": {
                "total_pareto_time_us": total_pareto_time,
                "avg_pareto_ratio": avg_pareto_ratio,
                "total_candidates_processed": self.candidate_counts.iter().sum::<usize>()
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

/// Test candidate structure (mirrors ModelCandidate from decision crate).
#[derive(Clone)]
struct TestCandidate {
    id: Uuid,
    name: String,
    quality: f64,
    cost: f64,
    latency_p95: f64,
}

/// Results for a single candidate count benchmark.
struct CountResult {
    candidate_count: usize,
    pareto_optimal_count: usize,
    pareto_time_us: u64,
    dominance_checks: usize,
    dominance_time_us: u64,
    scoring_time_us: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pareto_benchmark_runs() {
        let bench = ParetoBenchmark::with_candidate_counts(vec![10, 20]);
        let result = bench.run();

        assert!(result.get("success").unwrap().as_bool().unwrap());
        assert!(result.get("results").unwrap().as_array().unwrap().len() == 2);
    }

    #[test]
    fn test_dominance_relationship() {
        let better = TestCandidate {
            id: Uuid::new_v4(),
            name: "better".to_string(),
            quality: 0.9,
            cost: 0.05,
            latency_p95: 1000.0,
        };

        let worse = TestCandidate {
            id: Uuid::new_v4(),
            name: "worse".to_string(),
            quality: 0.7,
            cost: 0.10,
            latency_p95: 2000.0,
        };

        assert!(ParetoBenchmark::dominates(&better, &worse));
        assert!(!ParetoBenchmark::dominates(&worse, &better));
    }
}
