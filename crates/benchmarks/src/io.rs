//! Benchmark I/O Operations
//!
//! This module provides functionality for reading and writing benchmark
//! results to the filesystem. It handles the canonical output directory
//! structure and file formats.

use crate::result::BenchmarkResult;
use crate::markdown::generate_report;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Default output directory path relative to project root.
pub const DEFAULT_OUTPUT_DIR: &str = "benchmarks/output";

/// Default raw results directory path relative to output dir.
pub const RAW_OUTPUT_SUBDIR: &str = "raw";

/// Default summary file name.
pub const SUMMARY_FILE: &str = "summary.md";

/// Get the canonical output directory path.
///
/// # Arguments
///
/// * `base_path` - Optional base path. If None, uses current directory.
///
/// # Returns
///
/// PathBuf to the output directory.
pub fn output_dir(base_path: Option<&Path>) -> PathBuf {
    match base_path {
        Some(base) => base.join(DEFAULT_OUTPUT_DIR),
        None => PathBuf::from(DEFAULT_OUTPUT_DIR),
    }
}

/// Get the raw results directory path.
///
/// # Arguments
///
/// * `base_path` - Optional base path. If None, uses current directory.
///
/// # Returns
///
/// PathBuf to the raw results directory.
pub fn raw_output_dir(base_path: Option<&Path>) -> PathBuf {
    output_dir(base_path).join(RAW_OUTPUT_SUBDIR)
}

/// Ensure output directories exist.
///
/// Creates the output directory structure if it doesn't exist:
/// - `benchmarks/output/`
/// - `benchmarks/output/raw/`
///
/// # Arguments
///
/// * `base_path` - Optional base path. If None, uses current directory.
///
/// # Returns
///
/// `Ok(())` if directories were created/exist, `Err` otherwise.
pub fn ensure_output_dirs(base_path: Option<&Path>) -> io::Result<()> {
    let output = output_dir(base_path);
    let raw = raw_output_dir(base_path);

    fs::create_dir_all(&output)?;
    fs::create_dir_all(&raw)?;

    Ok(())
}

/// Write benchmark results to JSON file.
///
/// # Arguments
///
/// * `results` - Slice of benchmark results to write
/// * `base_path` - Optional base path for output
///
/// # Returns
///
/// Path to the written file.
pub fn write_results_json(
    results: &[BenchmarkResult],
    base_path: Option<&Path>,
) -> io::Result<PathBuf> {
    ensure_output_dirs(base_path)?;

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("results_{}.json", timestamp);
    let filepath = raw_output_dir(base_path).join(&filename);

    let json = serde_json::to_string_pretty(results)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    fs::write(&filepath, json)?;

    // Also write a "latest.json" symlink/copy
    let latest_path = raw_output_dir(base_path).join("latest.json");
    fs::write(&latest_path, serde_json::to_string_pretty(results)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?)?;

    Ok(filepath)
}

/// Write markdown summary report.
///
/// # Arguments
///
/// * `results` - Slice of benchmark results
/// * `base_path` - Optional base path for output
///
/// # Returns
///
/// Path to the written summary file.
pub fn write_summary(
    results: &[BenchmarkResult],
    base_path: Option<&Path>,
) -> io::Result<PathBuf> {
    ensure_output_dirs(base_path)?;

    let report = generate_report(results);
    let filepath = output_dir(base_path).join(SUMMARY_FILE);

    fs::write(&filepath, report)?;

    Ok(filepath)
}

/// Write all benchmark outputs (JSON + summary).
///
/// This is the canonical way to persist benchmark results.
///
/// # Arguments
///
/// * `results` - Slice of benchmark results
/// * `base_path` - Optional base path for output
///
/// # Returns
///
/// Tuple of (json_path, summary_path).
pub fn write_all_outputs(
    results: &[BenchmarkResult],
    base_path: Option<&Path>,
) -> io::Result<(PathBuf, PathBuf)> {
    let json_path = write_results_json(results, base_path)?;
    let summary_path = write_summary(results, base_path)?;

    Ok((json_path, summary_path))
}

/// Read benchmark results from a JSON file.
///
/// # Arguments
///
/// * `filepath` - Path to the JSON file
///
/// # Returns
///
/// Vector of benchmark results.
pub fn read_results_json(filepath: &Path) -> io::Result<Vec<BenchmarkResult>> {
    let content = fs::read_to_string(filepath)?;
    let results: Vec<BenchmarkResult> = serde_json::from_str(&content)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    Ok(results)
}

/// Read the latest benchmark results.
///
/// # Arguments
///
/// * `base_path` - Optional base path
///
/// # Returns
///
/// Vector of the most recent benchmark results.
pub fn read_latest_results(base_path: Option<&Path>) -> io::Result<Vec<BenchmarkResult>> {
    let latest_path = raw_output_dir(base_path).join("latest.json");
    read_results_json(&latest_path)
}

/// List all available result files.
///
/// # Arguments
///
/// * `base_path` - Optional base path
///
/// # Returns
///
/// Vector of paths to result JSON files, sorted by name (newest first).
pub fn list_result_files(base_path: Option<&Path>) -> io::Result<Vec<PathBuf>> {
    let raw_dir = raw_output_dir(base_path);

    if !raw_dir.exists() {
        return Ok(Vec::new());
    }

    let mut files: Vec<PathBuf> = fs::read_dir(&raw_dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .map(|ext| ext == "json")
                .unwrap_or(false)
                && path.file_name()
                    .map(|name| name != "latest.json")
                    .unwrap_or(false)
        })
        .collect();

    files.sort_by(|a, b| b.cmp(a)); // Reverse sort (newest first)

    Ok(files)
}

/// Clean old result files, keeping only the N most recent.
///
/// # Arguments
///
/// * `base_path` - Optional base path
/// * `keep_count` - Number of recent files to keep
///
/// # Returns
///
/// Number of files deleted.
pub fn cleanup_old_results(base_path: Option<&Path>, keep_count: usize) -> io::Result<usize> {
    let files = list_result_files(base_path)?;
    let mut deleted = 0;

    for file in files.iter().skip(keep_count) {
        fs::remove_file(file)?;
        deleted += 1;
    }

    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn sample_results() -> Vec<BenchmarkResult> {
        vec![
            BenchmarkResult::new("test1", json!({ "value": 42 })),
            BenchmarkResult::new("test2", json!({ "value": 100 })),
        ]
    }

    #[test]
    fn test_ensure_output_dirs() {
        let temp = TempDir::new().unwrap();
        ensure_output_dirs(Some(temp.path())).unwrap();

        assert!(output_dir(Some(temp.path())).exists());
        assert!(raw_output_dir(Some(temp.path())).exists());
    }

    #[test]
    fn test_write_and_read_results() {
        let temp = TempDir::new().unwrap();
        let results = sample_results();

        let json_path = write_results_json(&results, Some(temp.path())).unwrap();
        assert!(json_path.exists());

        let read_results = read_results_json(&json_path).unwrap();
        assert_eq!(read_results.len(), 2);
        assert_eq!(read_results[0].target_id, "test1");
    }

    #[test]
    fn test_write_summary() {
        let temp = TempDir::new().unwrap();
        let results = sample_results();

        let summary_path = write_summary(&results, Some(temp.path())).unwrap();
        assert!(summary_path.exists());

        let content = fs::read_to_string(&summary_path).unwrap();
        assert!(content.contains("# LLM Auto-Optimizer Benchmark Results"));
        assert!(content.contains("test1"));
        assert!(content.contains("test2"));
    }

    #[test]
    fn test_write_all_outputs() {
        let temp = TempDir::new().unwrap();
        let results = sample_results();

        let (json_path, summary_path) = write_all_outputs(&results, Some(temp.path())).unwrap();

        assert!(json_path.exists());
        assert!(summary_path.exists());
    }

    #[test]
    fn test_read_latest_results() {
        let temp = TempDir::new().unwrap();
        let results = sample_results();

        write_results_json(&results, Some(temp.path())).unwrap();

        let latest = read_latest_results(Some(temp.path())).unwrap();
        assert_eq!(latest.len(), 2);
    }

    #[test]
    fn test_list_result_files() {
        let temp = TempDir::new().unwrap();
        let results = sample_results();

        // Write multiple result files
        write_results_json(&results, Some(temp.path())).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        write_results_json(&results, Some(temp.path())).unwrap();

        let files = list_result_files(Some(temp.path())).unwrap();
        assert_eq!(files.len(), 2);
    }
}
