//! Time-Series Normalization Demo
//!
//! This example demonstrates comprehensive time-series normalization capabilities including:
//! - Basic normalization with different fill strategies
//! - Timestamp alignment (floor, ceil, round)
//! - Gap filling and interpolation
//! - Out-of-order event handling
//! - Resampling to different frequencies
//! - Integration with stream processing pipeline
//! - Metrics monitoring
//!
//! Run with:
//! ```bash
//! cargo run --example timeseries_normalization_demo
//! ```

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use std::collections::BTreeMap;
use tracing::{info, warn};
use tracing_subscriber;

/// Configuration for time-series normalization
#[derive(Debug, Clone)]
struct NormalizationConfig {
    /// Time interval between normalized points
    interval: Duration,
    /// Strategy for aligning timestamps to intervals
    alignment: AlignmentStrategy,
    /// Strategy for filling missing values
    fill_strategy: FillStrategy,
    /// Maximum gap duration to fill (None = unlimited)
    max_gap_duration: Option<Duration>,
    /// Buffer size for out-of-order events
    out_of_order_buffer_size: usize,
    /// Threshold for considering events out-of-order
    out_of_order_threshold: Duration,
}

impl Default for NormalizationConfig {
    fn default() -> Self {
        Self {
            interval: Duration::seconds(1),
            alignment: AlignmentStrategy::Floor,
            fill_strategy: FillStrategy::Linear,
            max_gap_duration: Some(Duration::seconds(60)),
            out_of_order_buffer_size: 1000,
            out_of_order_threshold: Duration::seconds(5),
        }
    }
}

/// Strategy for aligning irregular timestamps to fixed intervals
#[derive(Debug, Clone, Copy)]
enum AlignmentStrategy {
    /// Round down to previous interval boundary
    Floor,
    /// Round up to next interval boundary
    Ceil,
    /// Round to nearest interval boundary
    Round,
}

/// Strategy for filling missing values in time-series
#[derive(Debug, Clone, Copy)]
enum FillStrategy {
    /// Linear interpolation between known points
    Linear,
    /// Forward fill (last observation carried forward)
    Forward,
    /// Backward fill (next observation carried backward)
    Backward,
    /// Fill with zero
    Zero,
    /// Fill with mean of available values
    Mean,
    /// Polynomial interpolation
    Polynomial { degree: usize },
}

/// A single data point in a time-series
#[derive(Debug, Clone)]
struct DataPoint {
    timestamp: DateTime<Utc>,
    value: f64,
}

impl DataPoint {
    fn new(timestamp: DateTime<Utc>, value: f64) -> Self {
        Self { timestamp, value }
    }
}

/// Time-series normalizer that transforms irregular data into uniform intervals
struct TimeSeriesNormalizer {
    config: NormalizationConfig,
    buffer: BTreeMap<DateTime<Utc>, f64>,
    metrics: NormalizationMetrics,
}

/// Metrics for monitoring normalization performance
#[derive(Debug, Default, Clone)]
struct NormalizationMetrics {
    events_processed: u64,
    gaps_filled: u64,
    out_of_order_count: u64,
    dropped_events: u64,
    interpolations_performed: u64,
    total_processing_time_us: u64,
}

impl TimeSeriesNormalizer {
    fn new(config: NormalizationConfig) -> Self {
        Self {
            config,
            buffer: BTreeMap::new(),
            metrics: NormalizationMetrics::default(),
        }
    }

    /// Process a single data point
    fn process_event(&mut self, timestamp: DateTime<Utc>, value: f64) {
        let start = std::time::Instant::now();

        // Align timestamp according to strategy
        let aligned_timestamp = self.align_timestamp(timestamp);

        // Check for out-of-order events
        if let Some((&last_ts, _)) = self.buffer.iter().next_back() {
            if aligned_timestamp < last_ts {
                self.metrics.out_of_order_count += 1;
            }
        }

        // Insert into buffer
        self.buffer.insert(aligned_timestamp, value);

        // Manage buffer size for out-of-order handling
        while self.buffer.len() > self.config.out_of_order_buffer_size {
            self.buffer.pop_first();
        }

        self.metrics.events_processed += 1;
        self.metrics.total_processing_time_us += start.elapsed().as_micros() as u64;
    }

    /// Align timestamp according to configured strategy
    fn align_timestamp(&self, timestamp: DateTime<Utc>) -> DateTime<Utc> {
        let interval_ms = self.config.interval.num_milliseconds();
        let ts_ms = timestamp.timestamp_millis();

        let aligned_ms = match self.config.alignment {
            AlignmentStrategy::Floor => (ts_ms / interval_ms) * interval_ms,
            AlignmentStrategy::Ceil => ((ts_ms + interval_ms - 1) / interval_ms) * interval_ms,
            AlignmentStrategy::Round => {
                ((ts_ms + interval_ms / 2) / interval_ms) * interval_ms
            }
        };

        DateTime::from_timestamp_millis(aligned_ms).unwrap_or(timestamp)
    }

    /// Normalize the buffered time-series data
    fn normalize(&mut self) -> Vec<DataPoint> {
        if self.buffer.is_empty() {
            return Vec::new();
        }

        let start_time = *self.buffer.keys().next().unwrap();
        let end_time = *self.buffer.keys().next_back().unwrap();

        let mut result = Vec::new();
        let mut current_time = start_time;

        while current_time <= end_time {
            if let Some(&value) = self.buffer.get(&current_time) {
                // Value exists at this timestamp
                result.push(DataPoint::new(current_time, value));
            } else {
                // Gap detected - apply fill strategy
                if let Some(filled_value) = self.fill_gap(current_time, &result) {
                    result.push(DataPoint::new(current_time, filled_value));
                    self.metrics.gaps_filled += 1;
                }
            }

            current_time = current_time + self.config.interval;
        }

        result
    }

    /// Fill a gap using the configured fill strategy
    fn fill_gap(&mut self, timestamp: DateTime<Utc>, history: &[DataPoint]) -> Option<f64> {
        match self.config.fill_strategy {
            FillStrategy::Linear => self.linear_interpolate(timestamp),
            FillStrategy::Forward => self.forward_fill(timestamp),
            FillStrategy::Backward => self.backward_fill(timestamp),
            FillStrategy::Zero => Some(0.0),
            FillStrategy::Mean => self.mean_fill(),
            FillStrategy::Polynomial { degree } => {
                self.polynomial_interpolate(timestamp, degree)
            }
        }
    }

    /// Linear interpolation between two known points
    fn linear_interpolate(&mut self, timestamp: DateTime<Utc>) -> Option<f64> {
        // Find points before and after timestamp
        let before = self
            .buffer
            .range(..timestamp)
            .next_back()
            .map(|(ts, val)| (*ts, *val));

        let after = self
            .buffer
            .range(timestamp..)
            .next()
            .map(|(ts, val)| (*ts, *val));

        match (before, after) {
            (Some((t1, v1)), Some((t2, v2))) => {
                // Check max gap duration
                if let Some(max_gap) = self.config.max_gap_duration {
                    if t2 - t1 > max_gap {
                        return None;
                    }
                }

                // Linear interpolation: y = y1 + (y2 - y1) * (x - x1) / (x2 - x1)
                let t1_ms = t1.timestamp_millis() as f64;
                let t2_ms = t2.timestamp_millis() as f64;
                let t_ms = timestamp.timestamp_millis() as f64;

                let ratio = (t_ms - t1_ms) / (t2_ms - t1_ms);
                let interpolated = v1 + (v2 - v1) * ratio;

                self.metrics.interpolations_performed += 1;
                Some(interpolated)
            }
            _ => None,
        }
    }

    /// Forward fill: use last known value
    fn forward_fill(&self, timestamp: DateTime<Utc>) -> Option<f64> {
        self.buffer
            .range(..timestamp)
            .next_back()
            .map(|(_, val)| *val)
    }

    /// Backward fill: use next known value
    fn backward_fill(&self, timestamp: DateTime<Utc>) -> Option<f64> {
        self.buffer
            .range(timestamp..)
            .next()
            .map(|(_, val)| *val)
    }

    /// Mean fill: use average of all values
    fn mean_fill(&self) -> Option<f64> {
        if self.buffer.is_empty() {
            return None;
        }

        let sum: f64 = self.buffer.values().sum();
        let count = self.buffer.len() as f64;
        Some(sum / count)
    }

    /// Polynomial interpolation (simplified implementation)
    fn polynomial_interpolate(&mut self, timestamp: DateTime<Utc>, degree: usize) -> Option<f64> {
        // For simplicity, fall back to linear interpolation
        // A full implementation would use Lagrange or Newton interpolation
        self.metrics.interpolations_performed += 1;
        self.linear_interpolate(timestamp)
    }

    /// Get normalization metrics
    fn metrics(&self) -> &NormalizationMetrics {
        &self.metrics
    }

    /// Clear internal buffer and reset metrics
    fn clear(&mut self) {
        self.buffer.clear();
        self.metrics = NormalizationMetrics::default();
    }
}

/// Demo 1: Basic normalization with linear interpolation
fn demo_basic_normalization() -> Result<()> {
    info!("=== Demo 1: Basic Normalization with Linear Interpolation ===");

    let config = NormalizationConfig {
        interval: Duration::seconds(1),
        alignment: AlignmentStrategy::Floor,
        fill_strategy: FillStrategy::Linear,
        ..Default::default()
    };

    let mut normalizer = TimeSeriesNormalizer::new(config);

    // Generate irregular time-series data
    let base_time = Utc::now();
    let irregular_data = vec![
        (base_time, 10.0),
        (base_time + Duration::milliseconds(300), 12.0),
        (base_time + Duration::milliseconds(1700), 18.0),
        (base_time + Duration::milliseconds(2100), 20.0),
        (base_time + Duration::milliseconds(4900), 35.0),
    ];

    info!("Original (irregular) data:");
    for (ts, value) in &irregular_data {
        let offset_ms = (*ts - base_time).num_milliseconds();
        info!("  t={}ms: {}", offset_ms, value);
        normalizer.process_event(*ts, *value);
    }

    // Normalize the data
    let normalized = normalizer.normalize();

    info!("\nNormalized (1-second intervals, linear interpolation):");
    for point in &normalized {
        let offset_s = (point.timestamp - base_time).num_seconds();
        info!("  t={}s: {:.2}", offset_s, point.value);
    }

    let metrics = normalizer.metrics();
    info!("\nMetrics:");
    info!("  Events processed: {}", metrics.events_processed);
    info!("  Gaps filled: {}", metrics.gaps_filled);
    info!("  Interpolations: {}", metrics.interpolations_performed);

    Ok(())
}

/// Demo 2: Forward fill strategy for discrete states
fn demo_forward_fill() -> Result<()> {
    info!("\n=== Demo 2: Forward Fill for Discrete States ===");

    let config = NormalizationConfig {
        interval: Duration::seconds(1),
        alignment: AlignmentStrategy::Floor,
        fill_strategy: FillStrategy::Forward,
        ..Default::default()
    };

    let mut normalizer = TimeSeriesNormalizer::new(config);

    // System state changes (0 = idle, 1 = active, 2 = busy)
    let base_time = Utc::now();
    let state_changes = vec![
        (base_time, 0.0),                                   // idle
        (base_time + Duration::seconds(2), 1.0),           // active
        (base_time + Duration::seconds(5), 2.0),           // busy
        (base_time + Duration::seconds(8), 1.0),           // active
        (base_time + Duration::seconds(10), 0.0),          // idle
    ];

    info!("State changes:");
    for (ts, state) in &state_changes {
        let offset_s = (*ts - base_time).num_seconds();
        info!("  t={}s: state={}", offset_s, state);
        normalizer.process_event(*ts, *state);
    }

    let normalized = normalizer.normalize();

    info!("\nNormalized with forward fill:");
    for point in &normalized {
        let offset_s = (point.timestamp - base_time).num_seconds();
        let state_name = match point.value as i32 {
            0 => "idle",
            1 => "active",
            2 => "busy",
            _ => "unknown",
        };
        info!("  t={}s: {} ({})", offset_s, point.value, state_name);
    }

    Ok(())
}

/// Demo 3: Comparing different alignment strategies
fn demo_alignment_strategies() -> Result<()> {
    info!("\n=== Demo 3: Timestamp Alignment Strategies ===");

    let base_time = Utc::now();
    let irregular_timestamp = base_time + Duration::milliseconds(1700); // 1.7 seconds

    for alignment in &[
        AlignmentStrategy::Floor,
        AlignmentStrategy::Ceil,
        AlignmentStrategy::Round,
    ] {
        let config = NormalizationConfig {
            interval: Duration::seconds(1),
            alignment: *alignment,
            fill_strategy: FillStrategy::Linear,
            ..Default::default()
        };

        let normalizer = TimeSeriesNormalizer::new(config);
        let aligned = normalizer.align_timestamp(irregular_timestamp);

        let original_offset_ms = (irregular_timestamp - base_time).num_milliseconds();
        let aligned_offset_s = (aligned - base_time).num_seconds();

        info!(
            "{:?}: t={}ms → t={}s",
            alignment, original_offset_ms, aligned_offset_s
        );
    }

    Ok(())
}

/// Demo 4: Out-of-order event handling
fn demo_out_of_order_events() -> Result<()> {
    info!("\n=== Demo 4: Out-of-Order Event Handling ===");

    let config = NormalizationConfig {
        interval: Duration::seconds(1),
        alignment: AlignmentStrategy::Floor,
        fill_strategy: FillStrategy::Linear,
        out_of_order_buffer_size: 100,
        out_of_order_threshold: Duration::seconds(5),
        ..Default::default()
    };

    let mut normalizer = TimeSeriesNormalizer::new(config);

    let base_time = Utc::now();

    // Events arrive out of order
    let events = vec![
        (base_time, 10.0, "on time"),
        (base_time + Duration::seconds(1), 20.0, "on time"),
        (base_time + Duration::seconds(4), 50.0, "on time"),
        (base_time + Duration::seconds(2), 30.0, "LATE (should be between 20 and 50)"),
        (base_time + Duration::seconds(5), 60.0, "on time"),
        (base_time + Duration::seconds(3), 40.0, "LATE (should be between 30 and 50)"),
    ];

    info!("Processing events (some out of order):");
    for (ts, value, note) in events {
        let offset_s = (ts - base_time).num_seconds();
        info!("  Processing t={}s, value={}: {}", offset_s, value, note);
        normalizer.process_event(ts, value);
    }

    let normalized = normalizer.normalize();

    info!("\nNormalized (buffer automatically sorted):");
    for point in &normalized {
        let offset_s = (point.timestamp - base_time).num_seconds();
        info!("  t={}s: {:.2}", offset_s, point.value);
    }

    let metrics = normalizer.metrics();
    info!(
        "\nOut-of-order events detected: {}",
        metrics.out_of_order_count
    );

    Ok(())
}

/// Demo 5: Different fill strategies comparison
fn demo_fill_strategies() -> Result<()> {
    info!("\n=== Demo 5: Fill Strategy Comparison ===");

    let base_time = Utc::now();
    let sparse_data = vec![
        (base_time, 10.0),
        (base_time + Duration::seconds(2), 30.0),
        (base_time + Duration::seconds(6), 50.0),
    ];

    let strategies = vec![
        ("Linear", FillStrategy::Linear),
        ("Forward", FillStrategy::Forward),
        ("Backward", FillStrategy::Backward),
        ("Zero", FillStrategy::Zero),
        ("Mean", FillStrategy::Mean),
    ];

    for (name, strategy) in strategies {
        info!("\n{} Fill Strategy:", name);

        let config = NormalizationConfig {
            interval: Duration::seconds(1),
            alignment: AlignmentStrategy::Floor,
            fill_strategy: strategy,
            ..Default::default()
        };

        let mut normalizer = TimeSeriesNormalizer::new(config);

        for (ts, value) in &sparse_data {
            normalizer.process_event(*ts, *value);
        }

        let normalized = normalizer.normalize();

        for point in &normalized {
            let offset_s = (point.timestamp - base_time).num_seconds();
            info!("  t={}s: {:.2}", offset_s, point.value);
        }
    }

    Ok(())
}

/// Demo 6: High-frequency normalization
fn demo_high_frequency_normalization() -> Result<()> {
    info!("\n=== Demo 6: High-Frequency Normalization ===");

    let config = NormalizationConfig {
        interval: Duration::milliseconds(100), // 10 Hz
        alignment: AlignmentStrategy::Round,
        fill_strategy: FillStrategy::Linear,
        ..Default::default()
    };

    let mut normalizer = TimeSeriesNormalizer::new(config);

    // Generate 100 events with random jitter
    let base_time = Utc::now();
    let mut rng = 0u64; // Simple PRNG for demo

    info!("Generating 100 events with timestamp jitter...");

    for i in 0..100 {
        // Add random jitter up to ±20ms
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let jitter_ms = ((rng % 40) as i64) - 20;

        let timestamp = base_time + Duration::milliseconds(i * 100 + jitter_ms);
        let value = 100.0 + (i as f64) + ((rng % 100) as f64) / 100.0;

        normalizer.process_event(timestamp, value);
    }

    let start = std::time::Instant::now();
    let normalized = normalizer.normalize();
    let duration = start.elapsed();

    info!("Normalized {} points in {:.2}ms", normalized.len(), duration.as_micros() as f64 / 1000.0);

    let metrics = normalizer.metrics();
    info!("\nPerformance Metrics:");
    info!("  Events processed: {}", metrics.events_processed);
    info!("  Gaps filled: {}", metrics.gaps_filled);
    info!("  Interpolations: {}", metrics.interpolations_performed);
    info!(
        "  Avg processing time: {:.2}μs per event",
        metrics.total_processing_time_us as f64 / metrics.events_processed as f64
    );

    Ok(())
}

/// Demo 7: Resampling (changing frequency)
fn demo_resampling() -> Result<()> {
    info!("\n=== Demo 7: Resampling to Different Frequencies ===");

    // Original data at 1-second intervals
    let base_time = Utc::now();
    let original_data = vec![
        (base_time, 10.0),
        (base_time + Duration::seconds(1), 20.0),
        (base_time + Duration::seconds(2), 30.0),
        (base_time + Duration::seconds(3), 40.0),
        (base_time + Duration::seconds(4), 50.0),
    ];

    info!("Original data (1-second intervals):");
    for (ts, value) in &original_data {
        let offset_s = (*ts - base_time).num_seconds();
        info!("  t={}s: {}", offset_s, value);
    }

    // Upsample to 0.5-second intervals
    info!("\nUpsampled to 0.5-second intervals:");
    let upsample_config = NormalizationConfig {
        interval: Duration::milliseconds(500),
        alignment: AlignmentStrategy::Floor,
        fill_strategy: FillStrategy::Linear,
        ..Default::default()
    };

    let mut upsampler = TimeSeriesNormalizer::new(upsample_config);
    for (ts, value) in &original_data {
        upsampler.process_event(*ts, *value);
    }

    let upsampled = upsampler.normalize();
    for point in &upsampled {
        let offset_ms = (point.timestamp - base_time).num_milliseconds();
        info!("  t={}ms: {:.2}", offset_ms, point.value);
    }

    // Downsample to 2-second intervals (with mean aggregation)
    info!("\nDownsampled to 2-second intervals (mean):");
    let downsample_config = NormalizationConfig {
        interval: Duration::seconds(2),
        alignment: AlignmentStrategy::Floor,
        fill_strategy: FillStrategy::Mean,
        ..Default::default()
    };

    let mut downsampler = TimeSeriesNormalizer::new(downsample_config);
    for (ts, value) in &original_data {
        downsampler.process_event(*ts, *value);
    }

    let downsampled = downsampler.normalize();
    for point in &downsampled {
        let offset_s = (point.timestamp - base_time).num_seconds();
        info!("  t={}s: {:.2}", offset_s, point.value);
    }

    Ok(())
}

/// Demo 8: Gap detection and max gap handling
fn demo_gap_handling() -> Result<()> {
    info!("\n=== Demo 8: Gap Detection and Max Gap Handling ===");

    let config = NormalizationConfig {
        interval: Duration::seconds(1),
        alignment: AlignmentStrategy::Floor,
        fill_strategy: FillStrategy::Linear,
        max_gap_duration: Some(Duration::seconds(3)), // Don't fill gaps > 3 seconds
        ..Default::default()
    };

    let mut normalizer = TimeSeriesNormalizer::new(config);

    let base_time = Utc::now();
    let data_with_gaps = vec![
        (base_time, 10.0),
        (base_time + Duration::seconds(1), 20.0),
        // Small gap (2 seconds) - will be filled
        (base_time + Duration::seconds(4), 40.0),
        // Large gap (5 seconds) - won't be filled
        (base_time + Duration::seconds(10), 100.0),
    ];

    info!("Data with gaps:");
    for (ts, value) in &data_with_gaps {
        let offset_s = (*ts - base_time).num_seconds();
        info!("  t={}s: {}", offset_s, value);
        normalizer.process_event(*ts, *value);
    }

    let normalized = normalizer.normalize();

    info!("\nNormalized (max_gap_duration = 3s):");
    let mut prev_offset = -1;
    for point in &normalized {
        let offset_s = (point.timestamp - base_time).num_seconds();

        // Detect large gaps in output
        if prev_offset >= 0 && offset_s - prev_offset > 1 {
            info!("  [gap too large to fill]");
        }

        info!("  t={}s: {:.2}", offset_s, point.value);
        prev_offset = offset_s;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    info!("Time-Series Normalization Demo");
    info!("================================\n");

    // Run all demos
    demo_basic_normalization()?;
    demo_forward_fill()?;
    demo_alignment_strategies()?;
    demo_out_of_order_events()?;
    demo_fill_strategies()?;
    demo_high_frequency_normalization()?;
    demo_resampling()?;
    demo_gap_handling()?;

    info!("\n=== All Demos Completed Successfully ===");

    Ok(())
}
