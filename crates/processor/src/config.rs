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

    /// Deduplication configuration
    #[serde(default)]
    pub deduplication: DeduplicationConfig,

    /// Time-series normalization configuration
    #[serde(default)]
    pub normalization: NormalizationConfig,

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
            deduplication: DeduplicationConfig::default(),
            normalization: NormalizationConfig::default(),
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
        self.deduplication.validate()?;
        self.normalization.validate()?;

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

/// Time-series normalization configuration
///
/// Provides configuration for normalizing time-series data by aligning events to fixed
/// intervals, filling gaps, and handling out-of-order events.
///
/// Normalization is applied before windowing to ensure consistent event time alignment
/// and reduce skew in aggregations.
///
/// # Example
///
/// ```rust
/// use processor::config::{NormalizationConfig, FillStrategy, AlignmentStrategy};
/// use std::time::Duration;
///
/// let config = NormalizationConfig::new()
///     .enabled()
///     .with_interval(Duration::from_secs(5))
///     .with_fill_strategy(FillStrategy::LinearInterpolation)
///     .with_alignment(AlignmentStrategy::Start)
///     .with_buffer_size(1000);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizationConfig {
    /// Enable time-series normalization
    #[serde(default)]
    pub enabled: bool,

    /// Fixed interval for time alignment (e.g., 1s, 5s, 1min)
    #[serde(
        default = "default_normalization_interval",
        serialize_with = "serialize_duration",
        deserialize_with = "deserialize_duration"
    )]
    pub interval: Duration,

    /// Strategy for filling missing data points
    #[serde(default)]
    pub fill_strategy: FillStrategy,

    /// Alignment strategy for timestamps (start, end, or center of interval)
    #[serde(default)]
    pub alignment: AlignmentStrategy,

    /// Buffer size for handling out-of-order events
    #[serde(default = "default_normalization_buffer_size")]
    pub buffer_size: usize,

    /// Maximum gap to interpolate (larger gaps use fill strategy)
    #[serde(
        default = "default_interpolation_max_gap",
        serialize_with = "serialize_duration",
        deserialize_with = "deserialize_duration"
    )]
    pub interpolation_max_gap: Duration,

    /// Enable outlier detection and removal
    #[serde(default)]
    pub drop_outliers: bool,

    /// Outlier threshold in standard deviations (z-score)
    #[serde(default = "default_normalization_outlier_threshold")]
    pub outlier_threshold: f64,

    /// Enable metrics emission for normalization operations
    #[serde(default = "default_true")]
    pub emit_metrics: bool,
}

impl Default for NormalizationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval: default_normalization_interval(),
            fill_strategy: FillStrategy::default(),
            alignment: AlignmentStrategy::default(),
            buffer_size: default_normalization_buffer_size(),
            interpolation_max_gap: default_interpolation_max_gap(),
            drop_outliers: false,
            outlier_threshold: default_normalization_outlier_threshold(),
            emit_metrics: true,
        }
    }
}

impl NormalizationConfig {
    /// Create a new normalization configuration with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable normalization
    pub fn enabled(mut self) -> Self {
        self.enabled = true;
        self
    }

    /// Set the normalization interval
    ///
    /// # Example
    ///
    /// ```rust
    /// use processor::config::NormalizationConfig;
    /// use std::time::Duration;
    ///
    /// let config = NormalizationConfig::new()
    ///     .with_interval(Duration::from_secs(5)); // Normalize to 5-second intervals
    /// ```
    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    /// Set the fill strategy
    pub fn with_fill_strategy(mut self, strategy: FillStrategy) -> Self {
        self.fill_strategy = strategy;
        self
    }

    /// Set the alignment strategy
    pub fn with_alignment(mut self, alignment: AlignmentStrategy) -> Self {
        self.alignment = alignment;
        self
    }

    /// Set the buffer size for out-of-order events
    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    /// Set the maximum gap for interpolation
    pub fn with_interpolation_max_gap(mut self, gap: Duration) -> Self {
        self.interpolation_max_gap = gap;
        self
    }

    /// Enable outlier detection and removal
    pub fn with_outlier_detection(mut self, threshold: f64) -> Self {
        self.drop_outliers = true;
        self.outlier_threshold = threshold;
        self
    }

    /// Enable or disable metrics emission
    pub fn with_metrics(mut self, enabled: bool) -> Self {
        self.emit_metrics = enabled;
        self
    }

    /// Validate normalization configuration
    pub fn validate(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        if self.interval.as_millis() == 0 {
            return Err(ProcessorError::Configuration {
                source: "normalization interval must be greater than 0".into(),
            });
        }

        if self.buffer_size == 0 {
            return Err(ProcessorError::Configuration {
                source: "normalization buffer_size must be greater than 0".into(),
            });
        }

        if self.interpolation_max_gap.as_millis() == 0 {
            return Err(ProcessorError::Configuration {
                source: "interpolation_max_gap must be greater than 0".into(),
            });
        }

        if self.outlier_threshold <= 0.0 {
            return Err(ProcessorError::Configuration {
                source: "outlier_threshold must be positive".into(),
            });
        }

        // Validate that interpolation gap is reasonable relative to interval
        if self.interpolation_max_gap < self.interval {
            return Err(ProcessorError::Configuration {
                source: "interpolation_max_gap should be >= interval".into(),
            });
        }

        Ok(())
    }

    /// Get interval in milliseconds
    pub fn interval_ms(&self) -> u64 {
        self.interval.as_millis() as u64
    }

    /// Get interpolation max gap in milliseconds
    pub fn interpolation_max_gap_ms(&self) -> u64 {
        self.interpolation_max_gap.as_millis() as u64
    }
}

/// Strategy for filling missing data points in time-series normalization
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FillStrategy {
    /// Fill with zero value
    Zero,

    /// Forward fill - use last known value
    ForwardFill,

    /// Backward fill - use next known value
    BackwardFill,

    /// Linear interpolation between known values
    LinearInterpolation,

    /// Drop missing intervals (no fill)
    Drop,

    /// Fill with a specific constant value
    Constant,

    /// Use mean of surrounding values
    Mean,
}

impl Default for FillStrategy {
    fn default() -> Self {
        Self::ForwardFill
    }
}

/// Strategy for aligning event timestamps to intervals
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlignmentStrategy {
    /// Align to the start of the interval
    /// Example: 10:23:47 with 5s interval -> 10:23:45
    Start,

    /// Align to the end of the interval
    /// Example: 10:23:47 with 5s interval -> 10:23:50
    End,

    /// Align to the center of the interval
    /// Example: 10:23:47 with 5s interval -> 10:23:47.5
    Center,

    /// Round to nearest interval boundary
    /// Example: 10:23:47 with 5s interval -> 10:23:45 (closer to 45 than 50)
    Nearest,
}

impl Default for AlignmentStrategy {
    fn default() -> Self {
        Self::Start
    }
}

/// Event deduplication configuration
///
/// Provides configuration for detecting and filtering duplicate events in the stream.
/// When enabled, events are deduplicated based on their content hash using a Redis-backed
/// bloom filter or exact matching store.
///
/// # Example
///
/// ```rust
/// use processor::config::{DeduplicationConfig, DeduplicationStrategy};
/// use std::time::Duration;
///
/// let config = DeduplicationConfig {
///     enabled: true,
///     ttl: Duration::from_secs(3600),
///     strategy: DeduplicationStrategy::ContentHash,
///     redis_key_prefix: "dedup".to_string(),
///     max_retries: 3,
///     cleanup_interval: Duration::from_secs(300),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeduplicationConfig {
    /// Enable event deduplication
    #[serde(default)]
    pub enabled: bool,

    /// Time-to-live for deduplication records
    /// Events with the same signature within this window are considered duplicates
    #[serde(
        default = "default_deduplication_ttl",
        serialize_with = "serialize_duration",
        deserialize_with = "deserialize_duration"
    )]
    pub ttl: Duration,

    /// Deduplication strategy to use
    #[serde(default)]
    pub strategy: DeduplicationStrategy,

    /// Redis key prefix for deduplication storage
    #[serde(default = "default_redis_key_prefix")]
    pub redis_key_prefix: String,

    /// Maximum number of retries for Redis operations
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Interval for cleanup of expired deduplication records
    #[serde(
        default = "default_cleanup_interval",
        serialize_with = "serialize_duration",
        deserialize_with = "deserialize_duration"
    )]
    pub cleanup_interval: Duration,
}

impl Default for DeduplicationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            ttl: default_deduplication_ttl(),
            strategy: DeduplicationStrategy::default(),
            redis_key_prefix: default_redis_key_prefix(),
            max_retries: default_max_retries(),
            cleanup_interval: default_cleanup_interval(),
        }
    }
}

impl DeduplicationConfig {
    /// Create a new deduplication configuration with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable deduplication
    pub fn enabled(mut self) -> Self {
        self.enabled = true;
        self
    }

    /// Set the TTL for deduplication records
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    /// Set the deduplication strategy
    pub fn with_strategy(mut self, strategy: DeduplicationStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Set the Redis key prefix
    pub fn with_redis_key_prefix<S: Into<String>>(mut self, prefix: S) -> Self {
        self.redis_key_prefix = prefix.into();
        self
    }

    /// Set maximum retries for Redis operations
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Set cleanup interval
    pub fn with_cleanup_interval(mut self, interval: Duration) -> Self {
        self.cleanup_interval = interval;
        self
    }

    /// Validate deduplication configuration
    pub fn validate(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        if self.ttl.as_secs() == 0 {
            return Err(ProcessorError::Configuration {
                source: "deduplication ttl must be greater than 0".into(),
            });
        }

        if self.redis_key_prefix.is_empty() {
            return Err(ProcessorError::Configuration {
                source: "deduplication redis_key_prefix cannot be empty".into(),
            });
        }

        if self.cleanup_interval.as_secs() == 0 {
            return Err(ProcessorError::Configuration {
                source: "deduplication cleanup_interval must be greater than 0".into(),
            });
        }

        Ok(())
    }

    /// Get TTL in milliseconds
    pub fn ttl_ms(&self) -> u64 {
        self.ttl.as_millis() as u64
    }

    /// Get cleanup interval in milliseconds
    pub fn cleanup_interval_ms(&self) -> u64 {
        self.cleanup_interval.as_millis() as u64
    }
}

/// Strategy for event deduplication
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeduplicationStrategy {
    /// Use content-based hashing (SHA-256) for deduplication
    /// Deduplicates based on the full event payload
    ContentHash,

    /// Use event ID for exact matching
    /// Requires events to have an ID field
    EventId,

    /// Use composite key (source + type + timestamp)
    /// Useful for structured events with known fields
    CompositeKey,
}

impl Default for DeduplicationStrategy {
    fn default() -> Self {
        Self::ContentHash
    }
}

// Helper functions for Duration serialization/deserialization
fn serialize_duration<S>(duration: &Duration, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_u64(duration.as_millis() as u64)
}

fn deserialize_duration<'de, D>(deserializer: D) -> std::result::Result<Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let millis = u64::deserialize(deserializer)?;
    Ok(Duration::from_millis(millis))
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

fn default_deduplication_ttl() -> Duration {
    Duration::from_secs(3600) // 1 hour
}

fn default_redis_key_prefix() -> String {
    "dedup".to_string()
}

fn default_max_retries() -> u32 {
    3
}

fn default_cleanup_interval() -> Duration {
    Duration::from_secs(300) // 5 minutes
}

fn default_normalization_interval() -> Duration {
    Duration::from_secs(1) // 1 second
}

fn default_normalization_buffer_size() -> usize {
    1000
}

fn default_interpolation_max_gap() -> Duration {
    Duration::from_secs(5) // 5 seconds
}

fn default_normalization_outlier_threshold() -> f64 {
    3.0 // 3 standard deviations
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

    #[test]
    fn test_default_deduplication_config() {
        let config = DeduplicationConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.ttl, Duration::from_secs(3600));
        assert_eq!(config.strategy, DeduplicationStrategy::ContentHash);
        assert_eq!(config.redis_key_prefix, "dedup");
        assert_eq!(config.max_retries, 3);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_deduplication_config_builder() {
        let config = DeduplicationConfig::new()
            .enabled()
            .with_ttl(Duration::from_secs(7200))
            .with_strategy(DeduplicationStrategy::EventId)
            .with_redis_key_prefix("custom_dedup")
            .with_max_retries(5)
            .with_cleanup_interval(Duration::from_secs(600));

        assert!(config.enabled);
        assert_eq!(config.ttl, Duration::from_secs(7200));
        assert_eq!(config.strategy, DeduplicationStrategy::EventId);
        assert_eq!(config.redis_key_prefix, "custom_dedup");
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.cleanup_interval, Duration::from_secs(600));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_deduplication_config_validation() {
        // Valid disabled config
        let config = DeduplicationConfig::default();
        assert!(config.validate().is_ok());

        // Invalid TTL
        let mut invalid_config = DeduplicationConfig::new().enabled();
        invalid_config.ttl = Duration::from_secs(0);
        assert!(invalid_config.validate().is_err());

        // Invalid Redis key prefix
        let mut invalid_config = DeduplicationConfig::new().enabled();
        invalid_config.redis_key_prefix = String::new();
        assert!(invalid_config.validate().is_err());

        // Invalid cleanup interval
        let mut invalid_config = DeduplicationConfig::new().enabled();
        invalid_config.cleanup_interval = Duration::from_secs(0);
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_deduplication_strategy_default() {
        let strategy = DeduplicationStrategy::default();
        assert_eq!(strategy, DeduplicationStrategy::ContentHash);
    }

    #[test]
    fn test_deduplication_config_ttl_ms() {
        let config = DeduplicationConfig::new()
            .with_ttl(Duration::from_secs(60));
        assert_eq!(config.ttl_ms(), 60_000);
    }

    #[test]
    fn test_deduplication_config_cleanup_interval_ms() {
        let config = DeduplicationConfig::new()
            .with_cleanup_interval(Duration::from_secs(300));
        assert_eq!(config.cleanup_interval_ms(), 300_000);
    }

    #[test]
    fn test_processor_config_with_deduplication() {
        let config = ProcessorConfig::default();
        assert!(!config.deduplication.enabled);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_default_normalization_config() {
        let config = NormalizationConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.interval, Duration::from_secs(1));
        assert_eq!(config.fill_strategy, FillStrategy::ForwardFill);
        assert_eq!(config.alignment, AlignmentStrategy::Start);
        assert_eq!(config.buffer_size, 1000);
        assert!(!config.drop_outliers);
        assert_eq!(config.outlier_threshold, 3.0);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_normalization_config_builder() {
        let config = NormalizationConfig::new()
            .enabled()
            .with_interval(Duration::from_secs(5))
            .with_fill_strategy(FillStrategy::LinearInterpolation)
            .with_alignment(AlignmentStrategy::End)
            .with_buffer_size(2000)
            .with_interpolation_max_gap(Duration::from_secs(15))
            .with_outlier_detection(2.5)
            .with_metrics(false);

        assert!(config.enabled);
        assert_eq!(config.interval, Duration::from_secs(5));
        assert_eq!(config.fill_strategy, FillStrategy::LinearInterpolation);
        assert_eq!(config.alignment, AlignmentStrategy::End);
        assert_eq!(config.buffer_size, 2000);
        assert_eq!(config.interpolation_max_gap, Duration::from_secs(15));
        assert!(config.drop_outliers);
        assert_eq!(config.outlier_threshold, 2.5);
        assert!(!config.emit_metrics);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_normalization_config_validation() {
        // Valid disabled config
        let config = NormalizationConfig::default();
        assert!(config.validate().is_ok());

        // Invalid zero interval
        let mut invalid_config = NormalizationConfig::new().enabled();
        invalid_config.interval = Duration::from_secs(0);
        assert!(invalid_config.validate().is_err());

        // Invalid zero buffer size
        let mut invalid_config = NormalizationConfig::new().enabled();
        invalid_config.buffer_size = 0;
        assert!(invalid_config.validate().is_err());

        // Invalid zero interpolation gap
        let mut invalid_config = NormalizationConfig::new().enabled();
        invalid_config.interpolation_max_gap = Duration::from_secs(0);
        assert!(invalid_config.validate().is_err());

        // Invalid negative outlier threshold
        let mut invalid_config = NormalizationConfig::new().enabled();
        invalid_config.outlier_threshold = -1.0;
        assert!(invalid_config.validate().is_err());

        // Invalid interpolation gap < interval
        let mut invalid_config = NormalizationConfig::new()
            .enabled()
            .with_interval(Duration::from_secs(10))
            .with_interpolation_max_gap(Duration::from_secs(5));
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_fill_strategy_default() {
        let strategy = FillStrategy::default();
        assert_eq!(strategy, FillStrategy::ForwardFill);
    }

    #[test]
    fn test_alignment_strategy_default() {
        let strategy = AlignmentStrategy::default();
        assert_eq!(strategy, AlignmentStrategy::Start);
    }

    #[test]
    fn test_normalization_config_interval_ms() {
        let config = NormalizationConfig::new()
            .with_interval(Duration::from_secs(5));
        assert_eq!(config.interval_ms(), 5_000);
    }

    #[test]
    fn test_normalization_config_interpolation_max_gap_ms() {
        let config = NormalizationConfig::new()
            .with_interpolation_max_gap(Duration::from_secs(10));
        assert_eq!(config.interpolation_max_gap_ms(), 10_000);
    }

    #[test]
    fn test_processor_config_with_normalization() {
        let config = ProcessorConfig::default();
        assert!(!config.normalization.enabled);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_fill_strategies() {
        assert_eq!(FillStrategy::Zero, FillStrategy::Zero);
        assert_ne!(FillStrategy::Zero, FillStrategy::ForwardFill);

        let strategies = vec![
            FillStrategy::Zero,
            FillStrategy::ForwardFill,
            FillStrategy::BackwardFill,
            FillStrategy::LinearInterpolation,
            FillStrategy::Drop,
            FillStrategy::Constant,
            FillStrategy::Mean,
        ];

        for strategy in strategies {
            let serialized = serde_json::to_string(&strategy).unwrap();
            let deserialized: FillStrategy = serde_json::from_str(&serialized).unwrap();
            assert_eq!(strategy, deserialized);
        }
    }

    #[test]
    fn test_alignment_strategies() {
        let strategies = vec![
            AlignmentStrategy::Start,
            AlignmentStrategy::End,
            AlignmentStrategy::Center,
            AlignmentStrategy::Nearest,
        ];

        for strategy in strategies {
            let serialized = serde_json::to_string(&strategy).unwrap();
            let deserialized: AlignmentStrategy = serde_json::from_str(&serialized).unwrap();
            assert_eq!(strategy, deserialized);
        }
    }
}
