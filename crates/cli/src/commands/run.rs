//! Run command - Execute benchmarks and other operations
//!
//! This module provides the `run` subcommand for executing benchmarks
//! and other operations that don't require API connectivity.

use crate::{output::Formatter, CliError, CliResult};
use clap::Subcommand;
use colored::Colorize;

/// Run command variants
#[derive(Subcommand, Debug, Clone)]
pub enum RunCommand {
    /// Run benchmark suite
    #[command(name = "benchmarks", about = "Run the benchmark suite")]
    Benchmarks {
        /// Run only a specific benchmark target
        #[arg(long, short = 't', value_name = "TARGET_ID")]
        target: Option<String>,

        /// List available benchmark targets
        #[arg(long, short)]
        list: bool,

        /// Output directory for results (defaults to benchmarks/output)
        #[arg(long, short = 'o', value_name = "PATH")]
        output_dir: Option<std::path::PathBuf>,

        /// Skip writing output files
        #[arg(long)]
        no_output: bool,

        /// Output format for results (json, table)
        #[arg(long, default_value = "table")]
        format: String,
    },
}

impl RunCommand {
    /// Execute the run command
    pub async fn execute(&self, formatter: &dyn Formatter) -> CliResult<()> {
        match self {
            RunCommand::Benchmarks {
                target,
                list,
                output_dir,
                no_output,
                format,
            } => {
                self.run_benchmarks(target, *list, output_dir, *no_output, format, formatter)
                    .await
            }
        }
    }

    async fn run_benchmarks(
        &self,
        target: &Option<String>,
        list: bool,
        output_dir: &Option<std::path::PathBuf>,
        no_output: bool,
        format: &str,
        _formatter: &dyn Formatter,
    ) -> CliResult<()> {
        // List available targets
        if list {
            println!("{}", "Available Benchmark Targets:".bold().cyan());
            println!();

            let targets = get_benchmark_targets();
            for (id, description, category) in targets {
                println!(
                    "  {} [{}]",
                    id.green().bold(),
                    category.dimmed()
                );
                println!("    {}", description);
                println!();
            }
            return Ok(());
        }

        // Run benchmarks
        println!("{}", "LLM Auto-Optimizer Benchmark Suite".bold().cyan());
        println!("{}", "=".repeat(40));
        println!();

        let start_time = std::time::Instant::now();

        let results = if let Some(target_id) = target {
            // Run specific benchmark
            println!("Running benchmark: {}", target_id.yellow());
            match run_single_benchmark(target_id) {
                Some(result) => vec![result],
                None => {
                    return Err(CliError::NotFound(format!(
                        "Benchmark target '{}' not found. Use --list to see available targets.",
                        target_id
                    )));
                }
            }
        } else {
            // Run all benchmarks
            println!("Running all benchmarks...");
            println!();
            run_all_benchmarks()
        };

        let elapsed = start_time.elapsed();

        // Display results
        println!();
        println!("{}", "Results:".bold().cyan());
        println!("{}", "-".repeat(40));

        let successful = results.iter().filter(|r| r.success).count();
        let failed = results.len() - successful;

        for result in &results {
            let status = if result.success {
                "✓".green()
            } else {
                "✗".red()
            };

            let time_str = format!("{}ms", result.execution_time_ms);
            println!(
                "  {} {} ({})",
                status,
                result.target_id.bold(),
                time_str.dimmed()
            );
        }

        println!();
        println!("{}", "Summary:".bold().cyan());
        println!("  Total:      {}", results.len());
        println!(
            "  Successful: {}",
            successful.to_string().green()
        );
        if failed > 0 {
            println!("  Failed:     {}", failed.to_string().red());
        }
        println!("  Duration:   {:.2}s", elapsed.as_secs_f64());

        // Write output files
        if !no_output {
            let base_path = output_dir.as_ref().map(|p| p.as_path());

            match write_benchmark_outputs(&results, base_path) {
                Ok((json_path, summary_path)) => {
                    println!();
                    println!("{}", "Output files:".bold().cyan());
                    println!("  JSON:    {}", json_path.display());
                    println!("  Summary: {}", summary_path.display());
                }
                Err(e) => {
                    eprintln!(
                        "{} Failed to write output files: {}",
                        "Warning:".yellow(),
                        e
                    );
                }
            }
        }

        // JSON output format
        if format == "json" {
            let json_results: Vec<serde_json::Value> = results
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "target_id": r.target_id,
                        "success": r.success,
                        "execution_time_ms": r.execution_time_ms,
                        "metrics": r.metrics
                    })
                })
                .collect();

            println!();
            println!(
                "{}",
                serde_json::to_string_pretty(&json_results)
                    .unwrap_or_else(|_| "[]".to_string())
            );
        }

        if failed > 0 {
            Err(CliError::OperationFailed(format!(
                "{} benchmark(s) failed",
                failed
            )))
        } else {
            Ok(())
        }
    }
}

/// Simplified benchmark result for CLI display
#[derive(Clone)]
struct BenchmarkResultSimple {
    target_id: String,
    success: bool,
    execution_time_ms: u64,
    metrics: serde_json::Value,
}

/// Get available benchmark targets (inline implementation)
fn get_benchmark_targets() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "pareto-optimization",
            "Evaluates Pareto multi-objective optimization for quality/cost/latency trade-offs",
            "optimization",
        ),
        (
            "model-selection",
            "Evaluates model selection, registry lookups, and provider comparison operations",
            "selection",
        ),
        (
            "cost-performance-scoring",
            "Evaluates cost-performance scoring, weighted rankings, and optimization trade-offs",
            "scoring",
        ),
    ]
}

/// Run all benchmarks (inline implementation)
fn run_all_benchmarks() -> Vec<BenchmarkResultSimple> {
    let mut results = Vec::new();

    // Pareto optimization benchmark
    let start = std::time::Instant::now();
    let pareto_metrics = run_pareto_benchmark();
    let elapsed = start.elapsed();
    results.push(BenchmarkResultSimple {
        target_id: "pareto-optimization".to_string(),
        success: pareto_metrics.get("success").and_then(|v| v.as_bool()).unwrap_or(true),
        execution_time_ms: elapsed.as_millis() as u64,
        metrics: pareto_metrics,
    });

    // Model selection benchmark
    let start = std::time::Instant::now();
    let model_metrics = run_model_selection_benchmark();
    let elapsed = start.elapsed();
    results.push(BenchmarkResultSimple {
        target_id: "model-selection".to_string(),
        success: model_metrics.get("success").and_then(|v| v.as_bool()).unwrap_or(true),
        execution_time_ms: elapsed.as_millis() as u64,
        metrics: model_metrics,
    });

    // Cost scoring benchmark
    let start = std::time::Instant::now();
    let cost_metrics = run_cost_scoring_benchmark();
    let elapsed = start.elapsed();
    results.push(BenchmarkResultSimple {
        target_id: "cost-performance-scoring".to_string(),
        success: cost_metrics.get("success").and_then(|v| v.as_bool()).unwrap_or(true),
        execution_time_ms: elapsed.as_millis() as u64,
        metrics: cost_metrics,
    });

    results
}

/// Run a single benchmark by ID
fn run_single_benchmark(target_id: &str) -> Option<BenchmarkResultSimple> {
    let start = std::time::Instant::now();

    let metrics = match target_id {
        "pareto-optimization" => run_pareto_benchmark(),
        "model-selection" => run_model_selection_benchmark(),
        "cost-performance-scoring" => run_cost_scoring_benchmark(),
        _ => return None,
    };

    let elapsed = start.elapsed();

    Some(BenchmarkResultSimple {
        target_id: target_id.to_string(),
        success: metrics.get("success").and_then(|v| v.as_bool()).unwrap_or(true),
        execution_time_ms: elapsed.as_millis() as u64,
        metrics,
    })
}

/// Write benchmark outputs
fn write_benchmark_outputs(
    results: &[BenchmarkResultSimple],
    base_path: Option<&std::path::Path>,
) -> std::io::Result<(std::path::PathBuf, std::path::PathBuf)> {
    use std::fs;
    use std::io::Write;

    let output_dir = match base_path {
        Some(p) => p.join("benchmarks/output"),
        None => std::path::PathBuf::from("benchmarks/output"),
    };
    let raw_dir = output_dir.join("raw");

    fs::create_dir_all(&raw_dir)?;

    // Write JSON results
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let json_filename = format!("results_{}.json", timestamp);
    let json_path = raw_dir.join(&json_filename);

    let json_results: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            serde_json::json!({
                "target_id": r.target_id,
                "metrics": r.metrics,
                "timestamp": chrono::Utc::now().to_rfc3339()
            })
        })
        .collect();

    let mut json_file = fs::File::create(&json_path)?;
    json_file.write_all(
        serde_json::to_string_pretty(&json_results)
            .unwrap_or_default()
            .as_bytes(),
    )?;

    // Write latest.json
    let latest_path = raw_dir.join("latest.json");
    fs::copy(&json_path, &latest_path)?;

    // Write summary.md
    let summary_path = output_dir.join("summary.md");
    let mut summary_file = fs::File::create(&summary_path)?;

    let successful = results.iter().filter(|r| r.success).count();
    let total_time: u64 = results.iter().map(|r| r.execution_time_ms).sum();

    writeln!(summary_file, "# LLM Auto-Optimizer Benchmark Results")?;
    writeln!(summary_file)?;
    writeln!(
        summary_file,
        "**Generated:** {}",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    )?;
    writeln!(summary_file)?;
    writeln!(summary_file, "## Summary")?;
    writeln!(summary_file)?;
    writeln!(summary_file, "| Metric | Value |")?;
    writeln!(summary_file, "|--------|-------|")?;
    writeln!(summary_file, "| Total Benchmarks | {} |", results.len())?;
    writeln!(summary_file, "| Successful | {} |", successful)?;
    writeln!(summary_file, "| Failed | {} |", results.len() - successful)?;
    writeln!(summary_file, "| Total Execution Time | {}ms |", total_time)?;
    writeln!(summary_file)?;
    writeln!(summary_file, "## Benchmark Results")?;
    writeln!(summary_file)?;
    writeln!(
        summary_file,
        "| Target ID | Status | Execution Time |"
    )?;
    writeln!(summary_file, "|-----------|--------|----------------|")?;

    for result in results {
        let status = if result.success { "✓ Pass" } else { "✗ Fail" };
        writeln!(
            summary_file,
            "| {} | {} | {}ms |",
            result.target_id, status, result.execution_time_ms
        )?;
    }

    writeln!(summary_file)?;
    writeln!(
        summary_file,
        "---\n\n*Generated by LLM Auto-Optimizer Benchmark Suite*"
    )?;

    Ok((json_path, summary_path))
}

// Inline benchmark implementations

fn run_pareto_benchmark() -> serde_json::Value {
    use rand::Rng;

    let mut rng = rand::thread_rng();
    let candidate_counts = vec![10, 50, 100];
    let mut results = Vec::new();

    for count in &candidate_counts {
        // Generate candidates
        let candidates: Vec<(f64, f64, f64)> = (0..*count)
            .map(|_| {
                (
                    rng.gen_range(0.3..1.0),    // quality
                    rng.gen_range(0.01..0.50),   // cost
                    rng.gen_range(100.0..5000.0), // latency
                )
            })
            .collect();

        // Find Pareto optimal (simple implementation)
        let start = std::time::Instant::now();
        let mut pareto_count = 0;
        for (i, a) in candidates.iter().enumerate() {
            let dominated = candidates.iter().enumerate().any(|(j, b)| {
                i != j
                    && b.0 >= a.0
                    && b.1 <= a.1
                    && b.2 <= a.2
                    && (b.0 > a.0 || b.1 < a.1 || b.2 < a.2)
            });
            if !dominated {
                pareto_count += 1;
            }
        }
        let elapsed = start.elapsed();

        results.push(serde_json::json!({
            "candidate_count": count,
            "pareto_optimal_count": pareto_count,
            "time_us": elapsed.as_micros()
        }));
    }

    serde_json::json!({
        "success": true,
        "benchmark": "pareto-optimization",
        "results": results,
        "total_candidates": candidate_counts.iter().sum::<usize>()
    })
}

fn run_model_selection_benchmark() -> serde_json::Value {
    let iterations = 1000;

    // Model registry simulation
    let models = vec![
        ("gpt-4o", "openai", 0.005, 0.015),
        ("gpt-4o-mini", "openai", 0.00015, 0.0006),
        ("claude-3-5-sonnet", "anthropic", 0.003, 0.015),
        ("claude-3-5-haiku", "anthropic", 0.001, 0.005),
        ("gemini-1.5-pro", "google", 0.00125, 0.005),
        ("gemini-1.5-flash", "google", 0.000075, 0.0003),
    ];

    // Benchmark provider filtering
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let _: Vec<_> = models.iter().filter(|m| m.1 == "openai").collect();
        let _: Vec<_> = models.iter().filter(|m| m.1 == "anthropic").collect();
    }
    let filter_time = start.elapsed();

    // Benchmark cost calculation
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        for model in &models {
            let _cost = (1000.0 / 1000.0) * model.2 + (500.0 / 1000.0) * model.3;
        }
    }
    let cost_time = start.elapsed();

    // Find cheapest
    let cheapest = models
        .iter()
        .min_by(|a, b| {
            let cost_a = a.2 + a.3;
            let cost_b = b.2 + b.3;
            cost_a.partial_cmp(&cost_b).unwrap()
        })
        .map(|m| m.0);

    serde_json::json!({
        "success": true,
        "benchmark": "model-selection",
        "iterations": iterations,
        "model_count": models.len(),
        "filter_time_us": filter_time.as_micros(),
        "cost_calc_time_us": cost_time.as_micros(),
        "cheapest_model": cheapest,
        "throughput_filters_per_sec": (iterations * 2) as f64 / filter_time.as_secs_f64()
    })
}

fn run_cost_scoring_benchmark() -> serde_json::Value {
    use rand::Rng;

    let mut rng = rand::thread_rng();
    let sample_size = 500;

    // Generate samples
    let samples: Vec<(f64, f64, f64, f64)> = (0..sample_size)
        .map(|_| {
            (
                rng.gen_range(0.4..1.0),      // quality
                rng.gen_range(0.001..0.100),   // cost
                rng.gen_range(50.0..3000.0),   // latency
                rng.gen_range(10.0..1000.0),   // throughput
            )
        })
        .collect();

    // Benchmark scoring
    let start = std::time::Instant::now();
    let scores: Vec<f64> = samples
        .iter()
        .map(|(q, c, l, t)| {
            let norm_cost = (1.0 - (c / 0.1).min(1.0)).max(0.0);
            let norm_latency = (1.0 - (l / 3000.0).min(1.0)).max(0.0);
            let norm_throughput = (t / 1000.0).min(1.0);
            0.30 * q + 0.25 * norm_cost + 0.25 * norm_latency + 0.20 * norm_throughput
        })
        .collect();
    let scoring_time = start.elapsed();

    // Benchmark ranking
    let start = std::time::Instant::now();
    let mut ranked: Vec<_> = scores.iter().enumerate().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
    let ranking_time = start.elapsed();

    // Stats
    let min_score = scores.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_score = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let avg_score: f64 = scores.iter().sum::<f64>() / scores.len() as f64;

    serde_json::json!({
        "success": true,
        "benchmark": "cost-performance-scoring",
        "sample_size": sample_size,
        "scoring_time_us": scoring_time.as_micros(),
        "ranking_time_us": ranking_time.as_micros(),
        "score_stats": {
            "min": min_score,
            "max": max_score,
            "avg": avg_score
        },
        "top_score": ranked.first().map(|(_, s)| **s)
    })
}
