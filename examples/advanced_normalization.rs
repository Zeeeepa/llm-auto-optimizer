//! Advanced Time-Series Normalization Examples
//!
//! This example demonstrates advanced normalization scenarios:
//! - Financial time-series (OHLC data)
//! - Sensor data with irregular sampling
//! - Multi-metric normalization with different strategies
//! - Handling gaps and missing data
//! - Outlier detection and removal
//! - High-frequency data normalization with downsampling
//!
//! Run with:
//! ```bash
//! cargo run --example advanced_normalization
//! ```

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use std::collections::{BTreeMap, HashMap};
use tracing::{info, warn};
use tracing_subscriber;

/// OHLC (Open, High, Low, Close) data for financial time-series
#[derive(Debug, Clone, Copy)]
struct OhlcData {
    timestamp: DateTime<Utc>,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
}

impl OhlcData {
    fn new(timestamp: DateTime<Utc>, open: f64, high: f64, low: f64, close: f64, volume: f64) -> Self {
        Self {
            timestamp,
            open,
            high,
            low,
            close,
            volume,
        }
    }
}

/// Sensor measurement with quality indicator
#[derive(Debug, Clone)]
struct SensorReading {
    timestamp: DateTime<Utc>,
    value: f64,
    quality: DataQuality,
    sensor_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum DataQuality {
    Good,
    Uncertain,
    Bad,
}

impl SensorReading {
    fn new(timestamp: DateTime<Utc>, value: f64, quality: DataQuality, sensor_id: impl Into<String>) -> Self {
        Self {
            timestamp,
            value,
            quality,
            sensor_id: sensor_id.into(),
        }
    }
}

/// Multi-metric normalizer for handling different time-series simultaneously
struct MultiMetricNormalizer {
    metrics: HashMap<String, MetricNormalizer>,
    interval: Duration,
    alignment: AlignmentStrategy,
}

#[derive(Debug, Clone, Copy)]
enum AlignmentStrategy {
    Floor,
    Ceil,
    Round,
}

struct MetricNormalizer {
    name: String,
    fill_strategy: FillStrategy,
    buffer: BTreeMap<DateTime<Utc>, f64>,
    outlier_detector: Option<OutlierDetector>,
}

#[derive(Debug, Clone, Copy)]
enum FillStrategy {
    Linear,
    Forward,
    Backward,
    Zero,
    Mean,
}

/// Statistical outlier detection using Z-score
struct OutlierDetector {
    threshold: f64,
    values: Vec<f64>,
    mean: f64,
    std_dev: f64,
}

impl OutlierDetector {
    fn new(threshold: f64) -> Self {
        Self {
            threshold,
            values: Vec::new(),
            mean: 0.0,
            std_dev: 0.0,
        }
    }

    fn add_value(&mut self, value: f64) {
        self.values.push(value);
        self.update_statistics();
    }

    fn update_statistics(&mut self) {
        if self.values.is_empty() {
            return;
        }

        // Calculate mean
        let sum: f64 = self.values.iter().sum();
        self.mean = sum / self.values.len() as f64;

        // Calculate standard deviation
        let variance: f64 = self
            .values
            .iter()
            .map(|v| {
                let diff = v - self.mean;
                diff * diff
            })
            .sum::<f64>()
            / self.values.len() as f64;

        self.std_dev = variance.sqrt();
    }

    fn is_outlier(&self, value: f64) -> bool {
        if self.std_dev == 0.0 {
            return false;
        }

        let z_score = (value - self.mean).abs() / self.std_dev;
        z_score > self.threshold
    }

    fn statistics(&self) -> (f64, f64, usize) {
        (self.mean, self.std_dev, self.values.len())
    }
}

impl MultiMetricNormalizer {
    fn new(interval: Duration, alignment: AlignmentStrategy) -> Self {
        Self {
            metrics: HashMap::new(),
            interval,
            alignment,
        }
    }

    fn add_metric(&mut self, name: impl Into<String>, fill_strategy: FillStrategy, detect_outliers: bool) {
        let name = name.into();
        self.metrics.insert(
            name.clone(),
            MetricNormalizer {
                name,
                fill_strategy,
                buffer: BTreeMap::new(),
                outlier_detector: if detect_outliers {
                    Some(OutlierDetector::new(3.0))
                } else {
                    None
                },
            },
        );
    }

    fn process(&mut self, metric: &str, timestamp: DateTime<Utc>, value: f64) -> Result<()> {
        if let Some(normalizer) = self.metrics.get_mut(metric) {
            // Check for outliers
            if let Some(detector) = &mut normalizer.outlier_detector {
                if !detector.values.is_empty() && detector.is_outlier(value) {
                    warn!(
                        "Outlier detected in {}: value={:.2} (mean={:.2}, stddev={:.2})",
                        metric, value, detector.mean, detector.std_dev
                    );
                    // Skip outliers
                    return Ok(());
                }
                detector.add_value(value);
            }

            let aligned = self.align_timestamp(timestamp);
            normalizer.buffer.insert(aligned, value);
        }

        Ok(())
    }

    fn align_timestamp(&self, timestamp: DateTime<Utc>) -> DateTime<Utc> {
        let interval_ms = self.interval.num_milliseconds();
        let ts_ms = timestamp.timestamp_millis();

        let aligned_ms = match self.alignment {
            AlignmentStrategy::Floor => (ts_ms / interval_ms) * interval_ms,
            AlignmentStrategy::Ceil => ((ts_ms + interval_ms - 1) / interval_ms) * interval_ms,
            AlignmentStrategy::Round => ((ts_ms + interval_ms / 2) / interval_ms) * interval_ms,
        };

        DateTime::from_timestamp_millis(aligned_ms).unwrap_or(timestamp)
    }

    fn normalize_all(&self) -> HashMap<String, Vec<(DateTime<Utc>, f64)>> {
        let mut results = HashMap::new();

        for (name, normalizer) in &self.metrics {
            if normalizer.buffer.is_empty() {
                continue;
            }

            let start_time = *normalizer.buffer.keys().next().unwrap();
            let end_time = *normalizer.buffer.keys().next_back().unwrap();

            let mut normalized = Vec::new();
            let mut current_time = start_time;

            while current_time <= end_time {
                if let Some(&value) = normalizer.buffer.get(&current_time) {
                    normalized.push((current_time, value));
                } else if let Some(filled_value) = self.fill_gap(normalizer, current_time) {
                    normalized.push((current_time, filled_value));
                }

                current_time = current_time + self.interval;
            }

            results.insert(name.clone(), normalized);
        }

        results
    }

    fn fill_gap(&self, normalizer: &MetricNormalizer, timestamp: DateTime<Utc>) -> Option<f64> {
        match normalizer.fill_strategy {
            FillStrategy::Linear => {
                let before = normalizer
                    .buffer
                    .range(..timestamp)
                    .next_back()
                    .map(|(ts, val)| (*ts, *val));

                let after = normalizer
                    .buffer
                    .range(timestamp..)
                    .next()
                    .map(|(ts, val)| (*ts, *val));

                match (before, after) {
                    (Some((t1, v1)), Some((t2, v2))) => {
                        let t1_ms = t1.timestamp_millis() as f64;
                        let t2_ms = t2.timestamp_millis() as f64;
                        let t_ms = timestamp.timestamp_millis() as f64;

                        let ratio = (t_ms - t1_ms) / (t2_ms - t1_ms);
                        Some(v1 + (v2 - v1) * ratio)
                    }
                    _ => None,
                }
            }
            FillStrategy::Forward => normalizer
                .buffer
                .range(..timestamp)
                .next_back()
                .map(|(_, val)| *val),
            FillStrategy::Backward => normalizer
                .buffer
                .range(timestamp..)
                .next()
                .map(|(_, val)| *val),
            FillStrategy::Zero => Some(0.0),
            FillStrategy::Mean => {
                if normalizer.buffer.is_empty() {
                    None
                } else {
                    let sum: f64 = normalizer.buffer.values().sum();
                    Some(sum / normalizer.buffer.len() as f64)
                }
            }
        }
    }

    fn print_outlier_statistics(&self) {
        info!("\nOutlier Detection Statistics:");
        for (name, normalizer) in &self.metrics {
            if let Some(detector) = &normalizer.outlier_detector {
                let (mean, std_dev, count) = detector.statistics();
                info!(
                    "  {}: mean={:.2}, stddev={:.2}, samples={}",
                    name, mean, std_dev, count
                );
            }
        }
    }
}

/// Demo 1: Financial time-series (OHLC data) normalization
fn demo_financial_timeseries() -> Result<()> {
    info!("=== Demo 1: Financial Time-Series (OHLC Data) ===");

    // Simulate irregular trading data with gaps (weekends, holidays)
    let base_time = Utc::now();

    let ohlc_data = vec![
        // Monday
        OhlcData::new(base_time, 100.0, 105.0, 99.0, 103.0, 1000000.0),
        OhlcData::new(base_time + Duration::hours(1), 103.0, 107.0, 102.0, 106.0, 1200000.0),
        OhlcData::new(base_time + Duration::hours(2), 106.0, 108.0, 104.0, 105.0, 900000.0),
        // Gap (market closed)
        // Tuesday
        OhlcData::new(base_time + Duration::hours(20), 105.0, 110.0, 105.0, 109.0, 1500000.0),
        OhlcData::new(base_time + Duration::hours(21), 109.0, 112.0, 108.0, 111.0, 1300000.0),
    ];

    info!("Raw OHLC data:");
    for data in &ohlc_data {
        let offset_h = (data.timestamp - base_time).num_hours();
        info!(
            "  t={}h: O={:.2} H={:.2} L={:.2} C={:.2} V={:.0}",
            offset_h, data.open, data.high, data.low, data.close, data.volume
        );
    }

    // Normalize each component separately
    let mut normalizer = MultiMetricNormalizer::new(Duration::hours(1), AlignmentStrategy::Floor);

    normalizer.add_metric("open", FillStrategy::Forward, false);
    normalizer.add_metric("high", FillStrategy::Forward, false);
    normalizer.add_metric("low", FillStrategy::Forward, false);
    normalizer.add_metric("close", FillStrategy::Forward, false);
    normalizer.add_metric("volume", FillStrategy::Zero, false); // No volume = 0

    for data in &ohlc_data {
        normalizer.process("open", data.timestamp, data.open)?;
        normalizer.process("high", data.timestamp, data.high)?;
        normalizer.process("low", data.timestamp, data.low)?;
        normalizer.process("close", data.timestamp, data.close)?;
        normalizer.process("volume", data.timestamp, data.volume)?;
    }

    let normalized = normalizer.normalize_all();

    info!("\nNormalized OHLC data (1-hour intervals, forward fill for prices, zero for volume):");
    if let (Some(opens), Some(highs), Some(lows), Some(closes), Some(volumes)) = (
        normalized.get("open"),
        normalized.get("high"),
        normalized.get("low"),
        normalized.get("close"),
        normalized.get("volume"),
    ) {
        for i in 0..opens.len().min(highs.len()).min(lows.len()).min(closes.len()).min(volumes.len()) {
            let offset_h = (opens[i].0 - base_time).num_hours();
            info!(
                "  t={}h: O={:.2} H={:.2} L={:.2} C={:.2} V={:.0}",
                offset_h, opens[i].1, highs[i].1, lows[i].1, closes[i].1, volumes[i].1
            );
        }
    }

    Ok(())
}

/// Demo 2: Sensor data with quality indicators
fn demo_sensor_data() -> Result<()> {
    info!("\n=== Demo 2: Sensor Data with Quality Indicators ===");

    let base_time = Utc::now();

    let sensor_readings = vec![
        SensorReading::new(base_time, 23.5, DataQuality::Good, "temp_01"),
        SensorReading::new(base_time + Duration::seconds(10), 23.7, DataQuality::Good, "temp_01"),
        SensorReading::new(base_time + Duration::seconds(20), 99.9, DataQuality::Bad, "temp_01"), // Bad reading
        SensorReading::new(base_time + Duration::seconds(30), 23.9, DataQuality::Good, "temp_01"),
        SensorReading::new(base_time + Duration::seconds(40), 24.1, DataQuality::Uncertain, "temp_01"),
        SensorReading::new(base_time + Duration::seconds(50), 24.3, DataQuality::Good, "temp_01"),
    ];

    info!("Raw sensor readings:");
    for reading in &sensor_readings {
        let offset_s = (reading.timestamp - base_time).num_seconds();
        info!(
            "  t={}s: {:.2}°C [quality={:?}]",
            offset_s, reading.value, reading.quality
        );
    }

    // Process only good and uncertain readings, skip bad ones
    let mut normalizer = MultiMetricNormalizer::new(Duration::seconds(10), AlignmentStrategy::Floor);
    normalizer.add_metric("temperature", FillStrategy::Linear, false);

    for reading in &sensor_readings {
        if reading.quality != DataQuality::Bad {
            normalizer.process("temperature", reading.timestamp, reading.value)?;
        } else {
            warn!(
                "Skipping bad reading at t={}s: {:.2}°C",
                (reading.timestamp - base_time).num_seconds(),
                reading.value
            );
        }
    }

    let normalized = normalizer.normalize_all();

    info!("\nNormalized sensor data (bad readings excluded, linear interpolation):");
    if let Some(temps) = normalized.get("temperature") {
        for (ts, value) in temps {
            let offset_s = (*ts - base_time).num_seconds();
            info!("  t={}s: {:.2}°C", offset_s, value);
        }
    }

    Ok(())
}

/// Demo 3: Multi-metric system monitoring
fn demo_multi_metric_monitoring() -> Result<()> {
    info!("\n=== Demo 3: Multi-Metric System Monitoring ===");

    let base_time = Utc::now();

    // Simulate system metrics with different characteristics
    let mut normalizer = MultiMetricNormalizer::new(Duration::seconds(5), AlignmentStrategy::Round);

    normalizer.add_metric("cpu_percent", FillStrategy::Linear, false);
    normalizer.add_metric("memory_percent", FillStrategy::Linear, false);
    normalizer.add_metric("requests_per_sec", FillStrategy::Zero, false); // Count metric
    normalizer.add_metric("error_count", FillStrategy::Zero, false);     // Count metric

    // Generate irregular metrics
    let metrics_data = vec![
        (base_time, vec![("cpu_percent", 45.2), ("memory_percent", 62.1), ("requests_per_sec", 150.0)]),
        (
            base_time + Duration::seconds(7),
            vec![("cpu_percent", 52.3), ("memory_percent", 63.5), ("requests_per_sec", 175.0), ("error_count", 2.0)],
        ),
        (
            base_time + Duration::seconds(13),
            vec![("cpu_percent", 48.1), ("memory_percent", 64.0), ("requests_per_sec", 160.0)],
        ),
        (
            base_time + Duration::seconds(22),
            vec![("cpu_percent", 55.7), ("memory_percent", 65.2), ("requests_per_sec", 200.0), ("error_count", 1.0)],
        ),
    ];

    info!("Raw system metrics:");
    for (ts, metrics) in &metrics_data {
        let offset_s = (*ts - base_time).num_seconds();
        info!("  t={}s:", offset_s);
        for (name, value) in metrics {
            info!("    {}: {:.1}", name, value);
        }
    }

    // Process all metrics
    for (ts, metrics) in &metrics_data {
        for (name, value) in metrics {
            normalizer.process(name, *ts, *value)?;
        }
    }

    let normalized = normalizer.normalize_all();

    info!("\nNormalized system metrics (5-second intervals):");

    // Find time range
    let all_timestamps: Vec<DateTime<Utc>> = normalized
        .values()
        .flat_map(|v| v.iter().map(|(ts, _)| *ts))
        .collect();

    if let Some(&start) = all_timestamps.iter().min() {
        if let Some(&end) = all_timestamps.iter().max() {
            let mut current = start;
            while current <= end {
                let offset_s = (current - base_time).num_seconds();
                info!("  t={}s:", offset_s);

                for metric_name in &["cpu_percent", "memory_percent", "requests_per_sec", "error_count"] {
                    if let Some(data) = normalized.get(*metric_name) {
                        if let Some((_, value)) = data.iter().find(|(ts, _)| *ts == current) {
                            info!("    {}: {:.1}", metric_name, value);
                        }
                    }
                }

                current = current + Duration::seconds(5);
            }
        }
    }

    Ok(())
}

/// Demo 4: Outlier detection and removal
fn demo_outlier_detection() -> Result<()> {
    info!("\n=== Demo 4: Outlier Detection and Removal ===");

    let base_time = Utc::now();

    // Generate data with outliers
    let mut data = vec![
        (base_time, 100.0),
        (base_time + Duration::seconds(1), 102.0),
        (base_time + Duration::seconds(2), 101.0),
        (base_time + Duration::seconds(3), 103.0),
        (base_time + Duration::seconds(4), 500.0), // OUTLIER
        (base_time + Duration::seconds(5), 104.0),
        (base_time + Duration::seconds(6), 102.0),
        (base_time + Duration::seconds(7), 105.0),
        (base_time + Duration::seconds(8), -50.0), // OUTLIER
        (base_time + Duration::seconds(9), 103.0),
        (base_time + Duration::seconds(10), 106.0),
    ];

    info!("Data with outliers:");
    for (ts, value) in &data {
        let offset_s = (*ts - base_time).num_seconds();
        info!("  t={}s: {:.2}", offset_s, value);
    }

    // Process with outlier detection enabled
    let mut normalizer = MultiMetricNormalizer::new(Duration::seconds(1), AlignmentStrategy::Floor);
    normalizer.add_metric("sensor", FillStrategy::Linear, true); // Enable outlier detection

    for (ts, value) in &data {
        normalizer.process("sensor", *ts, *value)?;
    }

    normalizer.print_outlier_statistics();

    let normalized = normalizer.normalize_all();

    info!("\nNormalized data (outliers removed, gaps filled with linear interpolation):");
    if let Some(values) = normalized.get("sensor") {
        for (ts, value) in values {
            let offset_s = (*ts - base_time).num_seconds();
            info!("  t={}s: {:.2}", offset_s, value);
        }
    }

    Ok(())
}

/// Demo 5: High-frequency data with downsampling
fn demo_high_frequency_downsampling() -> Result<()> {
    info!("\n=== Demo 5: High-Frequency Data with Downsampling ===");

    let base_time = Utc::now();

    // Generate high-frequency data (100 Hz)
    info!("Generating high-frequency data (100 Hz, 1000 samples)...");

    let mut high_freq_data = Vec::new();
    for i in 0..1000 {
        let timestamp = base_time + Duration::milliseconds(i * 10);
        // Simulate noisy sensor with sine wave + noise
        let value = 100.0 + 50.0 * (i as f64 * 0.1).sin() + ((i * 7) % 20) as f64 - 10.0;
        high_freq_data.push((timestamp, value));
    }

    info!("First 10 samples:");
    for (ts, value) in high_freq_data.iter().take(10) {
        let offset_ms = (*ts - base_time).num_milliseconds();
        info!("  t={}ms: {:.2}", offset_ms, value);
    }

    // Downsample to 1 Hz with mean aggregation
    let mut normalizer = MultiMetricNormalizer::new(Duration::seconds(1), AlignmentStrategy::Floor);
    normalizer.add_metric("sensor", FillStrategy::Mean, false);

    for (ts, value) in &high_freq_data {
        normalizer.process("sensor", *ts, *value)?;
    }

    let normalized = normalizer.normalize_all();

    info!("\nDownsampled to 1 Hz (mean aggregation):");
    if let Some(values) = normalized.get("sensor") {
        for (ts, value) in values {
            let offset_s = (*ts - base_time).num_seconds();
            info!("  t={}s: {:.2}", offset_s, value);
        }
        info!("Reduced from {} to {} points", high_freq_data.len(), values.len());
    }

    Ok(())
}

/// Demo 6: Handling large gaps in time-series
fn demo_large_gaps() -> Result<()> {
    info!("\n=== Demo 6: Handling Large Gaps in Time-Series ===");

    let base_time = Utc::now();

    let data_with_large_gaps = vec![
        (base_time, 100.0),
        (base_time + Duration::seconds(10), 110.0),
        // 5-minute gap
        (base_time + Duration::seconds(310), 200.0),
        (base_time + Duration::seconds(320), 210.0),
        // Another large gap
        (base_time + Duration::seconds(620), 300.0),
    ];

    info!("Data with large gaps:");
    for (ts, value) in &data_with_large_gaps {
        let offset_s = (*ts - base_time).num_seconds();
        info!("  t={}s: {:.2}", offset_s, value);
    }

    let mut normalizer = MultiMetricNormalizer::new(Duration::seconds(10), AlignmentStrategy::Floor);
    normalizer.add_metric("metric", FillStrategy::Linear, false);

    for (ts, value) in &data_with_large_gaps {
        normalizer.process("metric", *ts, *value)?;
    }

    let normalized = normalizer.normalize_all();

    info!("\nNormalized (10-second intervals, linear interpolation across gaps):");
    info!("Note: Large gaps are filled with interpolated values");

    if let Some(values) = normalized.get("metric") {
        let mut prev_offset = -10;
        for (ts, value) in values {
            let offset_s = (*ts - base_time).num_seconds();

            // Highlight large gaps
            if prev_offset >= 0 && offset_s - prev_offset > 20 {
                info!("  [Large gap filled with {} interpolated points]", (offset_s - prev_offset) / 10 - 1);
            }

            info!("  t={}s: {:.2}", offset_s, value);
            prev_offset = offset_s;
        }

        info!("\nTotal points: {} (from {} original)", values.len(), data_with_large_gaps.len());
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

    info!("Advanced Time-Series Normalization Examples");
    info!("==========================================\n");

    // Run all demos
    demo_financial_timeseries()?;
    demo_sensor_data()?;
    demo_multi_metric_monitoring()?;
    demo_outlier_detection()?;
    demo_high_frequency_downsampling()?;
    demo_large_gaps()?;

    info!("\n=== All Advanced Demos Completed Successfully ===");

    Ok(())
}
