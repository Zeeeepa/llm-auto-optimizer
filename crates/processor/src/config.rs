//! Configuration types for the stream processor
//!
//! This module provides configuration structures for all processor components
//! including windows, watermarks, state backends, and aggregations.

use crate::error::{ProcessorError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Main processor configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorConfig {
    /// Window configuration
    pub window: WindowConfig,

    /// Watermark configuration
    pub watermark: WatermarkConfig,

    /// State backend configuration
    pub state: StateConfig,

    /// Aggregation configuration
    pub aggregation: AggregationConfig,

    /// Number of parallel processing workers
    #[serde(default = "default_parallelism")]
    pub parallelism: usize,

    /// Maximum events to buffer before backpressure
    #[serde(default = "default_buffer_size")]
    pub buffer_size: usize,

    /// Enable metrics collection
    #[serde(default = "default_true")]
    pub metrics_enabled: bool,
}

impl Default for ProcessorConfig {
    fn default() -> Self {
        Self {
            window: WindowConfig::default(),
            watermark: WatermarkConfig::default(),
            state: StateConfig::default(),
            aggregation: AggregationConfig::default(),
            parallelism: default_parallelism(),
            buffer_size: default_buffer_size(),
            metrics_enabled: true,
        }
    }
}

impl ProcessorConfig {
    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        self.window.validate()?;
        self.watermark.validate()?;
        self.state.validate()?;
        self.aggregation.validate()?;

        if self.parallelism == 0 {
            return Err(ProcessorError::Configuration {
                source: "parallelism must be greater than 0".into(),
            });
        }

        if self.buffer_size == 0 {
            return Err(ProcessorError::Configuration {
                source: "buffer_size must be greater than 0".into(),
            });
        }

        Ok(())
    }
}

/// Window configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    /// Type of window
    pub window_type: WindowType,

    /// Window size in milliseconds (for tumbling/sliding/hopping windows)
    pub size_ms: Option<u64>,

    /// Slide interval in milliseconds (for sliding/hopping windows)
    pub slide_ms: Option<u64>,

    /// Session gap in milliseconds (for session windows)
    pub gap_ms: Option<u64>,

    /// Allowed lateness for late events (milliseconds)
    #[serde(default = "default_allowed_lateness")]
    pub allowed_lateness_ms: u64,

    /// Whether to drop late events or process them
    #[serde(default)]
    pub drop_late_events: bool,
}

/// Type of windowing strategy
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WindowType {
    /// Fixed-size non-overlapping windows
    Tumbling,
    /// Fixed-size overlapping windows
    Sliding,
    /// Event-based windows with gaps
    Session,
    /// Count-based windows
    Count,
    /// Global window (all events in one window)
    Global,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            window_type: WindowType::Tumbling,
            size_ms: Some(60_000), // 1 minute
            slide_ms: None,
            gap_ms: None,
            allowed_lateness_ms: default_allowed_lateness(),
            drop_late_events: false,
        }
    }
}

impl WindowConfig {
    /// Create a tumbling window configuration
    pub fn tumbling(size_ms: u64) -> Self {
        Self {
            window_type: WindowType::Tumbling,
            size_ms: Some(size_ms),
            slide_ms: None,
            gap_ms: None,
            ..Default::default()
        }
    }

    /// Create a sliding window configuration
    pub fn sliding(size_ms: u64, slide_ms: u64) -> Self {
        Self {
            window_type: WindowType::Sliding,
            size_ms: Some(size_ms),
            slide_ms: Some(slide_ms),
            gap_ms: None,
            ..Default::default()
        }
    }

    /// Create a session window configuration
    pub fn session(gap_ms: u64) -> Self {
        Self {
            window_type: WindowType::Session,
            size_ms: None,
            slide_ms: None,
            gap_ms: Some(gap_ms),
            ..Default::default()
        }
    }

    /// Validate window configuration
    pub fn validate(&self) -> Result<()> {
        match self.window_type {
            WindowType::Tumbling => {
                if let Some(size) = self.size_ms {
                    if size == 0 {
                        return Err(ProcessorError::Configuration {
                            source: "tumbling window size must be greater than 0".into(),
                        });
                    }
                } else {
                    return Err(ProcessorError::Configuration {
                        source: "tumbling window requires size_ms".into(),
                    });
                }
            }
            WindowType::Sliding => {
                let size = self.size_ms.ok_or_else(|| ProcessorError::Configuration {
                    source: "sliding window requires size_ms".into(),
                })?;
                let slide = self.slide_ms.ok_or_else(|| ProcessorError::Configuration {
                    source: "sliding window requires slide_ms".into(),
                })?;

                if size == 0 {
                    return Err(ProcessorError::Configuration {
                        source: "sliding window size must be greater than 0".into(),
                    });
                }
                if slide == 0 {
                    return Err(ProcessorError::Configuration {
                        source: "sliding window slide must be greater than 0".into(),
                    });
                }
                if slide > size {
                    return Err(ProcessorError::Configuration {
                        source: "sliding window slide must be less than or equal to size".into(),
                    });
                }
            }
            WindowType::Session => {
                if let Some(gap) = self.gap_ms {
                    if gap == 0 {
                        return Err(ProcessorError::Configuration {
                            source: "session window gap must be greater than 0".into(),
                        });
                    }
                } else {
                    return Err(ProcessorError::Configuration {
                        source: "session window requires gap_ms".into(),
                    });
                }
            }
            WindowType::Count | WindowType::Global => {
                // No specific validation needed
            }
        }

        Ok(())
    }
}

/// Watermark configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatermarkConfig {
    /// Watermark generation strategy
    pub strategy: WatermarkStrategy,

    /// Maximum out-of-orderness delay (milliseconds)
    #[serde(default = "default_max_delay")]
    pub max_delay_ms: u64,

    /// Idle source timeout (milliseconds)
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_ms: u64,

    /// Watermark update interval (milliseconds)
    #[serde(default = "default_update_interval")]
    pub update_interval_ms: u64,

    /// Whether to allow clock skew
    #[serde(default = "default_true")]
    pub allow_clock_skew: bool,

    /// Maximum allowed clock skew (milliseconds)
    #[serde(default = "default_max_clock_skew")]
    pub max_clock_skew_ms: u64,
}

/// Watermark generation strategy
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WatermarkStrategy {
    /// Periodic watermark based on processing time
    Periodic,
    /// Bounded out-of-orderness based on event time
    BoundedOutOfOrderness,
    /// Ascending timestamps (no out-of-order events)
    Ascending,
    /// Manual watermark control
    Manual,
}

impl Default for WatermarkConfig {
    fn default() -> Self {
        Self {
            strategy: WatermarkStrategy::BoundedOutOfOrderness,
            max_delay_ms: default_max_delay(),
            idle_timeout_ms: default_idle_timeout(),
            update_interval_ms: default_update_interval(),
            allow_clock_skew: true,
            max_clock_skew_ms: default_max_clock_skew(),
        }
    }
}

impl WatermarkConfig {
    /// Validate watermark configuration
    pub fn validate(&self) -> Result<()> {
        if self.update_interval_ms == 0 {
            return Err(ProcessorError::Configuration {
                source: "watermark update_interval_ms must be greater than 0".into(),
            });
        }

        if self.idle_timeout_ms == 0 {
            return Err(ProcessorError::Configuration {
                source: "watermark idle_timeout_ms must be greater than 0".into(),
            });
        }

        Ok(())
    }

    /// Get max delay as Duration
    pub fn max_delay(&self) -> Duration {
        Duration::from_millis(self.max_delay_ms)
    }

    /// Get idle timeout as Duration
    pub fn idle_timeout(&self) -> Duration {
        Duration::from_millis(self.idle_timeout_ms)
    }

    /// Get update interval as Duration
    pub fn update_interval(&self) -> Duration {
        Duration::from_millis(self.update_interval_ms)
    }
}

/// State backend configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateConfig {
    /// State backend type
    pub backend: StateBackend,

    /// State storage path (for file-based backends)
    pub storage_path: Option<PathBuf>,

    /// Checkpoint interval (milliseconds)
    #[serde(default = "default_checkpoint_interval")]
    pub checkpoint_interval_ms: u64,

    /// Enable incremental checkpoints
    #[serde(default = "default_true")]
    pub incremental_checkpoints: bool,

    /// Number of checkpoints to retain
    #[serde(default = "default_checkpoint_retention")]
    pub checkpoint_retention: usize,

    /// State TTL (time-to-live) in milliseconds
    pub ttl_ms: Option<u64>,

    /// Maximum state size in bytes (for memory backends)
    pub max_size_bytes: Option<u64>,

    /// Enable state compression
    #[serde(default)]
    pub compression_enabled: bool,
}

/// State backend type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StateBackend {
    /// In-memory state (not fault-tolerant)
    Memory,
    /// File-based state using RocksDB/Sled
    FileSystem,
    /// Redis-backed state
    Redis,
    /// PostgreSQL-backed state
    PostgreSQL,
}

impl Default for StateConfig {
    fn default() -> Self {
        Self {
            backend: StateBackend::Memory,
            storage_path: None,
            checkpoint_interval_ms: default_checkpoint_interval(),
            incremental_checkpoints: true,
            checkpoint_retention: default_checkpoint_retention(),
            ttl_ms: None,
            max_size_bytes: None,
            compression_enabled: false,
        }
    }
}

impl StateConfig {
    /// Validate state configuration
    pub fn validate(&self) -> Result<()> {
        if self.checkpoint_interval_ms == 0 {
            return Err(ProcessorError::Configuration {
                source: "checkpoint_interval_ms must be greater than 0".into(),
            });
        }

        if self.checkpoint_retention == 0 {
            return Err(ProcessorError::Configuration {
                source: "checkpoint_retention must be greater than 0".into(),
            });
        }

        if matches!(self.backend, StateBackend::FileSystem) && self.storage_path.is_none() {
            return Err(ProcessorError::Configuration {
                source: "file-based state backend requires storage_path".into(),
            });
        }

        Ok(())
    }

    /// Get checkpoint interval as Duration
    pub fn checkpoint_interval(&self) -> Duration {
        Duration::from_millis(self.checkpoint_interval_ms)
    }

    /// Get TTL as Duration
    pub fn ttl(&self) -> Option<Duration> {
        self.ttl_ms.map(Duration::from_millis)
    }
}

/// Aggregation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregationConfig {
    /// Enabled aggregation types
    pub enabled_aggregations: Vec<AggregationType>,

    /// Percentiles to compute (e.g., [50, 95, 99])
    #[serde(default = "default_percentiles")]
    pub percentiles: Vec<u8>,

    /// Histogram bucket configuration
    pub histogram_buckets: Option<Vec<f64>>,

    /// Enable incremental aggregation
    #[serde(default = "default_true")]
    pub incremental: bool,

    /// Minimum samples required for aggregation
    #[serde(default = "default_min_samples")]
    pub min_samples: usize,

    /// Maximum samples to keep in memory
    #[serde(default = "default_max_samples")]
    pub max_samples: usize,

    /// Enable outlier detection and removal
    #[serde(default)]
    pub remove_outliers: bool,

    /// Outlier threshold (standard deviations)
    #[serde(default = "default_outlier_threshold")]
    pub outlier_threshold: f64,
}

/// Types of aggregations to compute
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AggregationType {
    /// Sum of all values
    Sum,
    /// Count of values
    Count,
    /// Arithmetic mean
    Average,
    /// Minimum value
    Min,
    /// Maximum value
    Max,
    /// Percentiles
    Percentile,
    /// Histogram
    Histogram,
    /// Standard deviation
    StdDev,
    /// Variance
    Variance,
}

impl Default for AggregationConfig {
    fn default() -> Self {
        Self {
            enabled_aggregations: vec![
                AggregationType::Sum,
                AggregationType::Count,
                AggregationType::Average,
                AggregationType::Min,
                AggregationType::Max,
                AggregationType::Percentile,
            ],
            percentiles: default_percentiles(),
            histogram_buckets: None,
            incremental: true,
            min_samples: default_min_samples(),
            max_samples: default_max_samples(),
            remove_outliers: false,
            outlier_threshold: default_outlier_threshold(),
        }
    }
}

impl AggregationConfig {
    /// Validate aggregation configuration
    pub fn validate(&self) -> Result<()> {
        if self.enabled_aggregations.is_empty() {
            return Err(ProcessorError::Configuration {
                source: "at least one aggregation type must be enabled".into(),
            });
        }

        for &p in &self.percentiles {
            if p > 100 {
                return Err(ProcessorError::Configuration {
                    source: format!("invalid percentile: {}, must be 0-100", p).into(),
                });
            }
        }

        if let Some(buckets) = &self.histogram_buckets {
            if buckets.is_empty() {
                return Err(ProcessorError::Configuration {
                    source: "histogram_buckets cannot be empty".into(),
                });
            }

            // Check if buckets are sorted
            for i in 1..buckets.len() {
                if buckets[i] <= buckets[i - 1] {
                    return Err(ProcessorError::Configuration {
                        source: "histogram_buckets must be in ascending order".into(),
                    });
                }
            }
        }

        if self.min_samples > self.max_samples {
            return Err(ProcessorError::Configuration {
                source: "min_samples cannot be greater than max_samples".into(),
            });
        }

        if self.outlier_threshold <= 0.0 {
            return Err(ProcessorError::Configuration {
                source: "outlier_threshold must be positive".into(),
            });
        }

        Ok(())
    }

    /// Check if a specific aggregation type is enabled
    pub fn is_enabled(&self, agg_type: AggregationType) -> bool {
        self.enabled_aggregations.contains(&agg_type)
    }
}

// Default value functions
fn default_parallelism() -> usize {
    num_cpus::get()
}

fn default_buffer_size() -> usize {
    10_000
}

fn default_allowed_lateness() -> u64 {
    5_000 // 5 seconds
}

fn default_max_delay() -> u64 {
    10_000 // 10 seconds
}

fn default_idle_timeout() -> u64 {
    60_000 // 1 minute
}

fn default_update_interval() -> u64 {
    1_000 // 1 second
}

fn default_max_clock_skew() -> u64 {
    30_000 // 30 seconds
}

fn default_checkpoint_interval() -> u64 {
    60_000 // 1 minute
}

fn default_checkpoint_retention() -> usize {
    3
}

fn default_percentiles() -> Vec<u8> {
    vec![50, 95, 99]
}

fn default_min_samples() -> usize {
    1
}

fn default_max_samples() -> usize {
    100_000
}

fn default_outlier_threshold() -> f64 {
    3.0
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_processor_config() {
        let config = ProcessorConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_tumbling_window_config() {
        let config = WindowConfig::tumbling(60_000);
        assert_eq!(config.window_type, WindowType::Tumbling);
        assert_eq!(config.size_ms, Some(60_000));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_sliding_window_config() {
        let config = WindowConfig::sliding(60_000, 30_000);
        assert_eq!(config.window_type, WindowType::Sliding);
        assert_eq!(config.size_ms, Some(60_000));
        assert_eq!(config.slide_ms, Some(30_000));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_sliding_window() {
        let mut config = WindowConfig::sliding(30_000, 60_000);
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_session_window_config() {
        let config = WindowConfig::session(5_000);
        assert_eq!(config.window_type, WindowType::Session);
        assert_eq!(config.gap_ms, Some(5_000));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_watermark_config_validation() {
        let config = WatermarkConfig::default();
        assert!(config.validate().is_ok());

        let mut invalid_config = config.clone();
        invalid_config.update_interval_ms = 0;
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_state_config_validation() {
        let config = StateConfig::default();
        assert!(config.validate().is_ok());

        let mut fs_config = config.clone();
        fs_config.backend = StateBackend::FileSystem;
        assert!(fs_config.validate().is_err()); // Missing storage_path

        fs_config.storage_path = Some(PathBuf::from("/tmp/state"));
        assert!(fs_config.validate().is_ok());
    }

    #[test]
    fn test_aggregation_config_validation() {
        let config = AggregationConfig::default();
        assert!(config.validate().is_ok());

        let mut invalid_config = config.clone();
        invalid_config.percentiles.push(101);
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_aggregation_is_enabled() {
        let config = AggregationConfig::default();
        assert!(config.is_enabled(AggregationType::Sum));
        assert!(config.is_enabled(AggregationType::Average));
        assert!(!config.is_enabled(AggregationType::StdDev));
    }

    #[test]
    fn test_histogram_bucket_validation() {
        let mut config = AggregationConfig::default();
        config.histogram_buckets = Some(vec![1.0, 5.0, 10.0, 50.0, 100.0]);
        assert!(config.validate().is_ok());

        // Unsorted buckets
        config.histogram_buckets = Some(vec![1.0, 10.0, 5.0]);
        assert!(config.validate().is_err());

        // Empty buckets
        config.histogram_buckets = Some(vec![]);
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_duration_conversions() {
        let watermark_config = WatermarkConfig::default();
        assert_eq!(
            watermark_config.max_delay(),
            Duration::from_millis(default_max_delay())
        );

        let state_config = StateConfig::default();
        assert_eq!(
            state_config.checkpoint_interval(),
            Duration::from_millis(default_checkpoint_interval())
        );
    }
}
