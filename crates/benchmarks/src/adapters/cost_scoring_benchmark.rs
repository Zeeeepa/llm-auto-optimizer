//! Cost-Performance Scoring Benchmark
//!
//! This benchmark target evaluates the cost-performance scoring
//! capabilities of the LLM Auto-Optimizer, testing weighted scoring,
//! normalization, and ranking algorithms.

use super::BenchTarget;
use serde_json::{json, Value};
use uuid::Uuid;

/// Benchmark for cost-performance scoring operations.
///
/// This benchmark tests:
/// - Weighted composite scoring calculations
/// - Score normalization across different scales
/// - Ranking and sorting by various criteria
/// - Weight sensitivity analysis
pub struct CostScoringBenchmark {
    sample_sizes: Vec<usize>,
    weight_configurations: Vec<WeightConfig>,
}

impl CostScoringBenchmark {
    /// Benchmark identifier
    pub const ID: &'static str = "cost-performance-scoring";

    /// Benchmark description
    pub const DESCRIPTION: &'static str =
        "Evaluates cost-performance scoring, weighted rankings, and optimization trade-offs";

    /// Benchmark category
    pub const CATEGORY: &'static str = "scoring";

    /// Create a new cost scoring benchmark.
    pub fn new() -> Self {
        Self {
            sample_sizes: vec![100, 500, 1000],
            weight_configurations: vec![
                WeightConfig::balanced(),
                WeightConfig::quality_focused(),
                WeightConfig::cost_focused(),
                WeightConfig::latency_focused(),
            ],
        }
    }

    /// Create with custom configuration.
    pub fn with_config(sample_sizes: Vec<usize>, weight_configs: Vec<WeightConfig>) -> Self {
        Self {
            sample_sizes,
            weight_configurations: weight_configs,
        }
    }

    /// Generate sample performance data.
    fn generate_samples(count: usize) -> Vec<PerformanceSample> {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        (0..count)
            .map(|i| PerformanceSample {
                id: Uuid::new_v4(),
                name: format!("sample-{}", i),
                quality: rng.gen_range(0.4..1.0),
                cost_usd: rng.gen_range(0.001..0.100),
                latency_ms: rng.gen_range(50.0..3000.0),
                throughput: rng.gen_range(10.0..1000.0),
            })
            .collect()
    }

    /// Calculate composite score for a sample.
    fn calculate_score(sample: &PerformanceSample, weights: &WeightConfig) -> f64 {
        // Normalize values to [0, 1] range
        let norm_quality = sample.quality; // Already in [0, 1]
        let norm_cost = (1.0 - (sample.cost_usd / weights.max_cost).min(1.0)).max(0.0);
        let norm_latency = (1.0 - (sample.latency_ms / weights.max_latency).min(1.0)).max(0.0);
        let norm_throughput = (sample.throughput / weights.max_throughput).min(1.0);

        // Weighted sum
        weights.quality * norm_quality
            + weights.cost * norm_cost
            + weights.latency * norm_latency
            + weights.throughput * norm_throughput
    }

    /// Rank samples by score.
    fn rank_by_score(samples: &[PerformanceSample], weights: &WeightConfig) -> Vec<RankedSample> {
        let mut ranked: Vec<_> = samples
            .iter()
            .map(|s| RankedSample {
                id: s.id,
                name: s.name.clone(),
                score: Self::calculate_score(s, weights),
            })
            .collect();

        ranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        ranked
    }

    /// Analyze score distribution.
    fn analyze_distribution(scores: &[f64]) -> DistributionStats {
        if scores.is_empty() {
            return DistributionStats::default();
        }

        let mut sorted = scores.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let min = sorted[0];
        let max = sorted[sorted.len() - 1];
        let sum: f64 = sorted.iter().sum();
        let mean = sum / sorted.len() as f64;

        let p25_idx = sorted.len() / 4;
        let p50_idx = sorted.len() / 2;
        let p75_idx = (sorted.len() * 3) / 4;

        let variance: f64 = sorted.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / sorted.len() as f64;
        let std_dev = variance.sqrt();

        DistributionStats {
            min,
            max,
            mean,
            std_dev,
            p25: sorted[p25_idx],
            p50: sorted[p50_idx],
            p75: sorted[p75_idx],
        }
    }
}

impl Default for CostScoringBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchTarget for CostScoringBenchmark {
    fn id(&self) -> &str {
        Self::ID
    }

    fn run(&self) -> Value {
        let mut results = Vec::new();

        for &sample_size in &self.sample_sizes {
            let samples = Self::generate_samples(sample_size);

            let mut config_results = Vec::new();

            for weights in &self.weight_configurations {
                // Benchmark scoring
                let start = std::time::Instant::now();
                let scores: Vec<f64> = samples
                    .iter()
                    .map(|s| Self::calculate_score(s, weights))
                    .collect();
                let scoring_time = start.elapsed();

                // Benchmark ranking
                let start = std::time::Instant::now();
                let ranked = Self::rank_by_score(&samples, weights);
                let ranking_time = start.elapsed();

                // Analyze distribution
                let distribution = Self::analyze_distribution(&scores);

                config_results.push(json!({
                    "weight_config": weights.name,
                    "scoring_time_us": scoring_time.as_micros() as u64,
                    "ranking_time_us": ranking_time.as_micros() as u64,
                    "top_score": ranked.first().map(|r| r.score).unwrap_or(0.0),
                    "bottom_score": ranked.last().map(|r| r.score).unwrap_or(0.0),
                    "distribution": {
                        "min": distribution.min,
                        "max": distribution.max,
                        "mean": distribution.mean,
                        "std_dev": distribution.std_dev,
                        "p25": distribution.p25,
                        "p50": distribution.p50,
                        "p75": distribution.p75
                    }
                }));
            }

            results.push(json!({
                "sample_size": sample_size,
                "configurations": config_results
            }));
        }

        // Calculate aggregate statistics
        let total_samples: usize = self.sample_sizes.iter().sum();
        let total_configurations = self.weight_configurations.len();

        json!({
            "success": true,
            "benchmark": Self::ID,
            "sample_sizes": self.sample_sizes,
            "weight_configs_count": total_configurations,
            "results": results,
            "summary": {
                "total_samples_processed": total_samples,
                "total_scoring_operations": total_samples * total_configurations,
                "weight_configurations": self.weight_configurations.iter().map(|w| json!({
                    "name": w.name,
                    "quality": w.quality,
                    "cost": w.cost,
                    "latency": w.latency,
                    "throughput": w.throughput
                })).collect::<Vec<_>>()
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

/// Performance sample for scoring.
struct PerformanceSample {
    id: Uuid,
    name: String,
    quality: f64,
    cost_usd: f64,
    latency_ms: f64,
    throughput: f64,
}

/// Ranked sample result.
struct RankedSample {
    id: Uuid,
    name: String,
    score: f64,
}

/// Weight configuration for scoring.
#[derive(Clone)]
pub struct WeightConfig {
    pub name: &'static str,
    pub quality: f64,
    pub cost: f64,
    pub latency: f64,
    pub throughput: f64,
    pub max_cost: f64,
    pub max_latency: f64,
    pub max_throughput: f64,
}

impl WeightConfig {
    /// Balanced weights.
    pub fn balanced() -> Self {
        Self {
            name: "balanced",
            quality: 0.30,
            cost: 0.25,
            latency: 0.25,
            throughput: 0.20,
            max_cost: 0.1,
            max_latency: 3000.0,
            max_throughput: 1000.0,
        }
    }

    /// Quality-focused weights.
    pub fn quality_focused() -> Self {
        Self {
            name: "quality-focused",
            quality: 0.50,
            cost: 0.15,
            latency: 0.20,
            throughput: 0.15,
            max_cost: 0.1,
            max_latency: 3000.0,
            max_throughput: 1000.0,
        }
    }

    /// Cost-focused weights.
    pub fn cost_focused() -> Self {
        Self {
            name: "cost-focused",
            quality: 0.20,
            cost: 0.50,
            latency: 0.15,
            throughput: 0.15,
            max_cost: 0.1,
            max_latency: 3000.0,
            max_throughput: 1000.0,
        }
    }

    /// Latency-focused weights.
    pub fn latency_focused() -> Self {
        Self {
            name: "latency-focused",
            quality: 0.25,
            cost: 0.20,
            latency: 0.40,
            throughput: 0.15,
            max_cost: 0.1,
            max_latency: 3000.0,
            max_throughput: 1000.0,
        }
    }
}

/// Score distribution statistics.
#[derive(Default)]
struct DistributionStats {
    min: f64,
    max: f64,
    mean: f64,
    std_dev: f64,
    p25: f64,
    p50: f64,
    p75: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_scoring_benchmark_runs() {
        let bench = CostScoringBenchmark::with_config(
            vec![50, 100],
            vec![WeightConfig::balanced()],
        );
        let result = bench.run();

        assert!(result.get("success").unwrap().as_bool().unwrap());
    }

    #[test]
    fn test_score_calculation() {
        let sample = PerformanceSample {
            id: Uuid::new_v4(),
            name: "test".to_string(),
            quality: 0.9,
            cost_usd: 0.01,
            latency_ms: 100.0,
            throughput: 500.0,
        };

        let weights = WeightConfig::balanced();
        let score = CostScoringBenchmark::calculate_score(&sample, &weights);

        assert!(score > 0.0 && score <= 1.0);
    }

    #[test]
    fn test_ranking() {
        let samples = CostScoringBenchmark::generate_samples(10);
        let weights = WeightConfig::balanced();
        let ranked = CostScoringBenchmark::rank_by_score(&samples, &weights);

        assert_eq!(ranked.len(), 10);
        // Should be sorted descending
        for i in 1..ranked.len() {
            assert!(ranked[i - 1].score >= ranked[i].score);
        }
    }
}
