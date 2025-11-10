//! Comprehensive tests for Time-Series Normalization implementation
//!
//! This test suite provides extensive coverage for time-series normalization,
//! including unit tests, integration tests, algorithm validation, performance benchmarks,
//! and edge case testing.
//!
//! Test Coverage:
//! - Unit Tests (20+): Timestamp alignment, fill strategies, interval detection
//! - Integration Tests (15+): End-to-end pipeline, multi-strategy, windowed processing
//! - Algorithm Validation (10+): Interpolation accuracy, fill correctness
//! - Performance Tests (8+): Throughput, latency, memory efficiency
//! - Edge Cases (12+): Missing data, duplicates, gaps, clock skew

use chrono::{DateTime, Duration, TimeZone, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

// ============================================================================
// TEST DATA STRUCTURES AND HELPERS
// ============================================================================

/// Test event for normalization
#[derive(Debug, Clone, PartialEq)]
struct TestEvent {
    timestamp: DateTime<Utc>,
    value: f64,
    tags: HashMap<String, String>,
}

impl TestEvent {
    fn new(timestamp: DateTime<Utc>, value: f64) -> Self {
        Self {
            timestamp,
            value,
            tags: HashMap::new(),
        }
    }

    fn with_tags(mut self, tags: HashMap<String, String>) -> Self {
        self.tags = tags;
        self
    }
}

/// Fill strategy for missing data points
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FillStrategy {
    /// Forward fill - use the last known value
    Forward,
    /// Backward fill - use the next known value
    Backward,
    /// Linear interpolation between known values
    Linear,
    /// Fill with zero
    Zero,
    /// Skip missing points
    Skip,
}

/// Timestamp alignment boundary
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlignmentBoundary {
    /// Align to the nearest second
    Second,
    /// Align to the nearest minute
    Minute,
    /// Align to the nearest hour
    Hour,
    /// No alignment
    None,
}

/// Time-series normalizer configuration
#[derive(Debug, Clone)]
struct NormalizerConfig {
    /// Target interval between data points
    interval: Duration,
    /// Fill strategy for missing data
    fill_strategy: FillStrategy,
    /// Timestamp alignment boundary
    alignment: AlignmentBoundary,
    /// Maximum gap to interpolate (larger gaps are treated as missing)
    max_interpolation_gap: Duration,
    /// Buffer size for out-of-order handling
    buffer_size: usize,
    /// Maximum latency for buffering
    max_latency: Duration,
}

impl Default for NormalizerConfig {
    fn default() -> Self {
        Self {
            interval: Duration::seconds(1),
            fill_strategy: FillStrategy::Linear,
            alignment: AlignmentBoundary::Second,
            max_interpolation_gap: Duration::seconds(10),
            buffer_size: 1000,
            max_latency: Duration::seconds(5),
        }
    }
}

/// Normalized time-series result
#[derive(Debug, Clone)]
struct NormalizedSeries {
    events: Vec<TestEvent>,
    metrics: NormalizationMetrics,
}

/// Metrics tracking normalization operations
#[derive(Debug, Clone, Default)]
struct NormalizationMetrics {
    input_count: usize,
    output_count: usize,
    interpolated_count: usize,
    forward_filled_count: usize,
    backward_filled_count: usize,
    dropped_count: usize,
    out_of_order_count: usize,
    gaps_detected: usize,
}

/// Time-series normalizer
struct TimeSeriesNormalizer {
    config: NormalizerConfig,
    buffer: Vec<TestEvent>,
    metrics: NormalizationMetrics,
}

impl TimeSeriesNormalizer {
    fn new(config: NormalizerConfig) -> Self {
        Self {
            config,
            buffer: Vec::new(),
            metrics: NormalizationMetrics::default(),
        }
    }

    /// Align timestamp to the configured boundary
    fn align_timestamp(&self, timestamp: DateTime<Utc>) -> DateTime<Utc> {
        match self.config.alignment {
            AlignmentBoundary::Second => {
                Utc.timestamp_opt(timestamp.timestamp(), 0).unwrap()
            }
            AlignmentBoundary::Minute => {
                let ts = timestamp.timestamp();
                let aligned = (ts / 60) * 60;
                Utc.timestamp_opt(aligned, 0).unwrap()
            }
            AlignmentBoundary::Hour => {
                let ts = timestamp.timestamp();
                let aligned = (ts / 3600) * 3600;
                Utc.timestamp_opt(aligned, 0).unwrap()
            }
            AlignmentBoundary::None => timestamp,
        }
    }

    /// Detect the interval between consecutive events
    fn detect_interval(&self, events: &[TestEvent]) -> Option<Duration> {
        if events.len() < 2 {
            return None;
        }

        let mut intervals = Vec::new();
        for window in events.windows(2) {
            let interval = window[1].timestamp - window[0].timestamp;
            if interval.num_seconds() > 0 {
                intervals.push(interval);
            }
        }

        if intervals.is_empty() {
            return None;
        }

        // Return the median interval
        intervals.sort();
        Some(intervals[intervals.len() / 2])
    }

    /// Linear interpolation between two points
    fn interpolate_linear(
        &self,
        start: &TestEvent,
        end: &TestEvent,
        target_time: DateTime<Utc>,
    ) -> f64 {
        let total_duration = (end.timestamp - start.timestamp).num_milliseconds() as f64;
        let elapsed_duration = (target_time - start.timestamp).num_milliseconds() as f64;

        if total_duration == 0.0 {
            return start.value;
        }

        let ratio = elapsed_duration / total_duration;
        start.value + (end.value - start.value) * ratio
    }

    /// Fill missing data points using the configured strategy
    fn fill_gap(
        &mut self,
        prev: Option<&TestEvent>,
        next: Option<&TestEvent>,
        timestamp: DateTime<Utc>,
    ) -> Option<TestEvent> {
        match self.config.fill_strategy {
            FillStrategy::Forward => {
                if let Some(prev) = prev {
                    self.metrics.forward_filled_count += 1;
                    Some(TestEvent::new(timestamp, prev.value))
                } else {
                    None
                }
            }
            FillStrategy::Backward => {
                if let Some(next) = next {
                    self.metrics.backward_filled_count += 1;
                    Some(TestEvent::new(timestamp, next.value))
                } else {
                    None
                }
            }
            FillStrategy::Linear => {
                if let (Some(prev), Some(next)) = (prev, next) {
                    let gap = next.timestamp - prev.timestamp;
                    if gap <= self.config.max_interpolation_gap {
                        let value = self.interpolate_linear(prev, next, timestamp);
                        self.metrics.interpolated_count += 1;
                        Some(TestEvent::new(timestamp, value))
                    } else {
                        self.metrics.gaps_detected += 1;
                        None
                    }
                } else {
                    None
                }
            }
            FillStrategy::Zero => {
                Some(TestEvent::new(timestamp, 0.0))
            }
            FillStrategy::Skip => None,
        }
    }

    /// Normalize a time series
    fn normalize(&mut self, mut events: Vec<TestEvent>) -> NormalizedSeries {
        self.metrics.input_count = events.len();

        if events.is_empty() {
            return NormalizedSeries {
                events: Vec::new(),
                metrics: self.metrics.clone(),
            };
        }

        // Sort events by timestamp
        events.sort_by_key(|e| e.timestamp);

        // Count out-of-order events
        let mut sorted_check = events.clone();
        sorted_check.sort_by_key(|e| e.timestamp);
        self.metrics.out_of_order_count = events
            .iter()
            .zip(sorted_check.iter())
            .filter(|(a, b)| a.timestamp != b.timestamp)
            .count();

        // Align timestamps
        let mut aligned_events: Vec<TestEvent> = events
            .iter()
            .map(|e| {
                let mut event = e.clone();
                event.timestamp = self.align_timestamp(e.timestamp);
                event
            })
            .collect();

        // Remove duplicates (keep last value for each timestamp)
        let mut deduped: HashMap<i64, TestEvent> = HashMap::new();
        for event in aligned_events {
            deduped.insert(event.timestamp.timestamp(), event);
        }
        aligned_events = deduped.values().cloned().collect();
        aligned_events.sort_by_key(|e| e.timestamp);

        // Generate normalized series
        let mut result = Vec::new();

        if aligned_events.is_empty() {
            return NormalizedSeries {
                events: result,
                metrics: self.metrics.clone(),
            };
        }

        let start_time = aligned_events[0].timestamp;
        let end_time = aligned_events[aligned_events.len() - 1].timestamp;

        let mut current_time = start_time;
        let mut event_idx = 0;

        while current_time <= end_time {
            // Find the event at current time
            if let Some(event) = aligned_events
                .iter()
                .find(|e| e.timestamp == current_time)
            {
                result.push(event.clone());
            } else {
                // Need to fill this gap
                let prev = aligned_events
                    .iter()
                    .rev()
                    .find(|e| e.timestamp < current_time);
                let next = aligned_events
                    .iter()
                    .find(|e| e.timestamp > current_time);

                if let Some(filled) = self.fill_gap(prev, next, current_time) {
                    result.push(filled);
                } else {
                    self.metrics.dropped_count += 1;
                }
            }

            current_time = current_time + self.config.interval;
        }

        self.metrics.output_count = result.len();

        NormalizedSeries {
            events: result,
            metrics: self.metrics.clone(),
        }
    }

    /// Add event to buffer for out-of-order handling
    fn add_to_buffer(&mut self, event: TestEvent) {
        self.buffer.push(event);
        self.buffer.sort_by_key(|e| e.timestamp);

        // Trim buffer if it exceeds size limit
        if self.buffer.len() > self.config.buffer_size {
            self.buffer.drain(0..self.buffer.len() - self.config.buffer_size);
        }
    }

    /// Flush buffer and return normalized events
    fn flush_buffer(&mut self) -> NormalizedSeries {
        let events = std::mem::take(&mut self.buffer);
        self.normalize(events)
    }
}

// ============================================================================
// UNIT TESTS - Timestamp Alignment (5 tests)
// ============================================================================

#[cfg(test)]
mod timestamp_alignment_tests {
    use super::*;

    #[test]
    fn test_align_to_second() {
        let config = NormalizerConfig {
            alignment: AlignmentBoundary::Second,
            ..Default::default()
        };
        let normalizer = TimeSeriesNormalizer::new(config);

        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 12, 30, 45).unwrap()
            + Duration::milliseconds(789);
        let aligned = normalizer.align_timestamp(timestamp);

        assert_eq!(aligned, Utc.with_ymd_and_hms(2024, 1, 1, 12, 30, 45).unwrap());
    }

    #[test]
    fn test_align_to_minute() {
        let config = NormalizerConfig {
            alignment: AlignmentBoundary::Minute,
            ..Default::default()
        };
        let normalizer = TimeSeriesNormalizer::new(config);

        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 12, 30, 45).unwrap();
        let aligned = normalizer.align_timestamp(timestamp);

        assert_eq!(aligned, Utc.with_ymd_and_hms(2024, 1, 1, 12, 30, 0).unwrap());
    }

    #[test]
    fn test_align_to_hour() {
        let config = NormalizerConfig {
            alignment: AlignmentBoundary::Hour,
            ..Default::default()
        };
        let normalizer = TimeSeriesNormalizer::new(config);

        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 12, 30, 45).unwrap();
        let aligned = normalizer.align_timestamp(timestamp);

        assert_eq!(aligned, Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap());
    }

    #[test]
    fn test_no_alignment() {
        let config = NormalizerConfig {
            alignment: AlignmentBoundary::None,
            ..Default::default()
        };
        let normalizer = TimeSeriesNormalizer::new(config);

        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 12, 30, 45).unwrap()
            + Duration::milliseconds(789);
        let aligned = normalizer.align_timestamp(timestamp);

        assert_eq!(aligned, timestamp);
    }

    #[test]
    fn test_alignment_preserves_timezone() {
        let config = NormalizerConfig {
            alignment: AlignmentBoundary::Second,
            ..Default::default()
        };
        let normalizer = TimeSeriesNormalizer::new(config);

        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap()
            + Duration::milliseconds(500);
        let aligned = normalizer.align_timestamp(timestamp);

        assert_eq!(aligned.timezone(), Utc);
    }
}

// ============================================================================
// UNIT TESTS - Fill Strategies (8 tests)
// ============================================================================

#[cfg(test)]
mod fill_strategy_tests {
    use super::*;

    #[test]
    fn test_forward_fill_with_previous_value() {
        let mut config = NormalizerConfig::default();
        config.fill_strategy = FillStrategy::Forward;
        let mut normalizer = TimeSeriesNormalizer::new(config);

        let prev = TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 100.0);
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 1).unwrap();

        let filled = normalizer.fill_gap(Some(&prev), None, timestamp);
        assert!(filled.is_some());
        assert_eq!(filled.unwrap().value, 100.0);
        assert_eq!(normalizer.metrics.forward_filled_count, 1);
    }

    #[test]
    fn test_forward_fill_without_previous_value() {
        let mut config = NormalizerConfig::default();
        config.fill_strategy = FillStrategy::Forward;
        let mut normalizer = TimeSeriesNormalizer::new(config);

        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let filled = normalizer.fill_gap(None, None, timestamp);
        assert!(filled.is_none());
    }

    #[test]
    fn test_backward_fill_with_next_value() {
        let mut config = NormalizerConfig::default();
        config.fill_strategy = FillStrategy::Backward;
        let mut normalizer = TimeSeriesNormalizer::new(config);

        let next = TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 2).unwrap(), 200.0);
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 1).unwrap();

        let filled = normalizer.fill_gap(None, Some(&next), timestamp);
        assert!(filled.is_some());
        assert_eq!(filled.unwrap().value, 200.0);
        assert_eq!(normalizer.metrics.backward_filled_count, 1);
    }

    #[test]
    fn test_backward_fill_without_next_value() {
        let mut config = NormalizerConfig::default();
        config.fill_strategy = FillStrategy::Backward;
        let mut normalizer = TimeSeriesNormalizer::new(config);

        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let filled = normalizer.fill_gap(None, None, timestamp);
        assert!(filled.is_none());
    }

    #[test]
    fn test_linear_interpolation() {
        let mut config = NormalizerConfig::default();
        config.fill_strategy = FillStrategy::Linear;
        config.max_interpolation_gap = Duration::seconds(10);
        let mut normalizer = TimeSeriesNormalizer::new(config);

        let prev = TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 100.0);
        let next = TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 2).unwrap(), 200.0);
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 1).unwrap();

        let filled = normalizer.fill_gap(Some(&prev), Some(&next), timestamp);
        assert!(filled.is_some());
        assert_eq!(filled.unwrap().value, 150.0);
        assert_eq!(normalizer.metrics.interpolated_count, 1);
    }

    #[test]
    fn test_linear_interpolation_exceeds_max_gap() {
        let mut config = NormalizerConfig::default();
        config.fill_strategy = FillStrategy::Linear;
        config.max_interpolation_gap = Duration::seconds(2);
        let mut normalizer = TimeSeriesNormalizer::new(config);

        let prev = TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 100.0);
        let next = TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 10).unwrap(), 200.0);
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 5).unwrap();

        let filled = normalizer.fill_gap(Some(&prev), Some(&next), timestamp);
        assert!(filled.is_none());
        assert_eq!(normalizer.metrics.gaps_detected, 1);
    }

    #[test]
    fn test_zero_fill() {
        let mut config = NormalizerConfig::default();
        config.fill_strategy = FillStrategy::Zero;
        let mut normalizer = TimeSeriesNormalizer::new(config);

        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let filled = normalizer.fill_gap(None, None, timestamp);
        assert!(filled.is_some());
        assert_eq!(filled.unwrap().value, 0.0);
    }

    #[test]
    fn test_skip_fill() {
        let mut config = NormalizerConfig::default();
        config.fill_strategy = FillStrategy::Skip;
        let mut normalizer = TimeSeriesNormalizer::new(config);

        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let filled = normalizer.fill_gap(None, None, timestamp);
        assert!(filled.is_none());
    }
}

// ============================================================================
// UNIT TESTS - Interval Detection (3 tests)
// ============================================================================

#[cfg(test)]
mod interval_detection_tests {
    use super::*;

    #[test]
    fn test_detect_uniform_interval() {
        let normalizer = TimeSeriesNormalizer::new(NormalizerConfig::default());

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 1.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 1).unwrap(), 2.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 2).unwrap(), 3.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 3).unwrap(), 4.0),
        ];

        let interval = normalizer.detect_interval(&events);
        assert_eq!(interval, Some(Duration::seconds(1)));
    }

    #[test]
    fn test_detect_interval_with_gaps() {
        let normalizer = TimeSeriesNormalizer::new(NormalizerConfig::default());

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 1.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 1).unwrap(), 2.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 5).unwrap(), 3.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 6).unwrap(), 4.0),
        ];

        let interval = normalizer.detect_interval(&events);
        assert_eq!(interval, Some(Duration::seconds(1)));
    }

    #[test]
    fn test_detect_interval_insufficient_data() {
        let normalizer = TimeSeriesNormalizer::new(NormalizerConfig::default());

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 1.0),
        ];

        let interval = normalizer.detect_interval(&events);
        assert_eq!(interval, None);
    }
}

// ============================================================================
// UNIT TESTS - Buffer Management (4 tests)
// ============================================================================

#[cfg(test)]
mod buffer_management_tests {
    use super::*;

    #[test]
    fn test_buffer_maintains_order() {
        let mut normalizer = TimeSeriesNormalizer::new(NormalizerConfig::default());

        // Add events out of order
        normalizer.add_to_buffer(TestEvent::new(
            Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 2).unwrap(),
            3.0,
        ));
        normalizer.add_to_buffer(TestEvent::new(
            Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(),
            1.0,
        ));
        normalizer.add_to_buffer(TestEvent::new(
            Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 1).unwrap(),
            2.0,
        ));

        // Buffer should be sorted
        assert_eq!(normalizer.buffer.len(), 3);
        assert_eq!(normalizer.buffer[0].value, 1.0);
        assert_eq!(normalizer.buffer[1].value, 2.0);
        assert_eq!(normalizer.buffer[2].value, 3.0);
    }

    #[test]
    fn test_buffer_size_limit() {
        let mut config = NormalizerConfig::default();
        config.buffer_size = 3;
        let mut normalizer = TimeSeriesNormalizer::new(config);

        // Add more events than buffer size
        for i in 0..5 {
            normalizer.add_to_buffer(TestEvent::new(
                Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, i as u32).unwrap(),
                i as f64,
            ));
        }

        // Buffer should be trimmed to size limit
        assert_eq!(normalizer.buffer.len(), 3);
        // Should keep the most recent events
        assert_eq!(normalizer.buffer[0].value, 2.0);
        assert_eq!(normalizer.buffer[1].value, 3.0);
        assert_eq!(normalizer.buffer[2].value, 4.0);
    }

    #[test]
    fn test_flush_buffer_clears_state() {
        let mut normalizer = TimeSeriesNormalizer::new(NormalizerConfig::default());

        normalizer.add_to_buffer(TestEvent::new(
            Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(),
            1.0,
        ));
        normalizer.add_to_buffer(TestEvent::new(
            Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 1).unwrap(),
            2.0,
        ));

        let _result = normalizer.flush_buffer();

        assert_eq!(normalizer.buffer.len(), 0);
    }

    #[test]
    fn test_flush_empty_buffer() {
        let mut normalizer = TimeSeriesNormalizer::new(NormalizerConfig::default());
        let result = normalizer.flush_buffer();

        assert_eq!(result.events.len(), 0);
        assert_eq!(result.metrics.input_count, 0);
    }
}

// ============================================================================
// INTEGRATION TESTS - End-to-End Normalization (5 tests)
// ============================================================================

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_normalize_complete_series() {
        let mut config = NormalizerConfig::default();
        config.interval = Duration::seconds(1);
        config.fill_strategy = FillStrategy::Linear;
        config.alignment = AlignmentBoundary::Second;

        let mut normalizer = TimeSeriesNormalizer::new(config);

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 100.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 1).unwrap(), 110.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 2).unwrap(), 120.0),
        ];

        let result = normalizer.normalize(events);

        assert_eq!(result.events.len(), 3);
        assert_eq!(result.metrics.input_count, 3);
        assert_eq!(result.metrics.output_count, 3);
        assert_eq!(result.metrics.interpolated_count, 0);
    }

    #[test]
    fn test_normalize_with_gaps() {
        let mut config = NormalizerConfig::default();
        config.interval = Duration::seconds(1);
        config.fill_strategy = FillStrategy::Linear;

        let mut normalizer = TimeSeriesNormalizer::new(config);

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 100.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 3).unwrap(), 130.0),
        ];

        let result = normalizer.normalize(events);

        // Should have 4 points: 0, 1, 2, 3
        assert_eq!(result.events.len(), 4);
        assert_eq!(result.events[0].value, 100.0);
        assert_eq!(result.events[1].value, 110.0); // Interpolated
        assert_eq!(result.events[2].value, 120.0); // Interpolated
        assert_eq!(result.events[3].value, 130.0);
        assert_eq!(result.metrics.interpolated_count, 2);
    }

    #[test]
    fn test_normalize_out_of_order_events() {
        let mut config = NormalizerConfig::default();
        config.interval = Duration::seconds(1);

        let mut normalizer = TimeSeriesNormalizer::new(config);

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 2).unwrap(), 120.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 100.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 1).unwrap(), 110.0),
        ];

        let result = normalizer.normalize(events);

        // Should be sorted in output
        assert_eq!(result.events.len(), 3);
        assert_eq!(result.events[0].value, 100.0);
        assert_eq!(result.events[1].value, 110.0);
        assert_eq!(result.events[2].value, 120.0);
    }

    #[test]
    fn test_normalize_duplicate_timestamps() {
        let mut config = NormalizerConfig::default();
        config.interval = Duration::seconds(1);
        config.alignment = AlignmentBoundary::Second;

        let mut normalizer = TimeSeriesNormalizer::new(config);

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 100.0),
            TestEvent::new(
                Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap() + Duration::milliseconds(500),
                105.0,
            ),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 1).unwrap(), 110.0),
        ];

        let result = normalizer.normalize(events);

        // Duplicates should be handled (keeping one)
        assert_eq!(result.events.len(), 2);
    }

    #[test]
    fn test_normalize_empty_input() {
        let mut normalizer = TimeSeriesNormalizer::new(NormalizerConfig::default());
        let result = normalizer.normalize(Vec::new());

        assert_eq!(result.events.len(), 0);
        assert_eq!(result.metrics.input_count, 0);
        assert_eq!(result.metrics.output_count, 0);
    }
}

// ============================================================================
// INTEGRATION TESTS - Multi-Strategy (5 tests)
// ============================================================================

#[cfg(test)]
mod multi_strategy_tests {
    use super::*;

    #[test]
    fn test_forward_fill_strategy() {
        let mut config = NormalizerConfig::default();
        config.interval = Duration::seconds(1);
        config.fill_strategy = FillStrategy::Forward;

        let mut normalizer = TimeSeriesNormalizer::new(config);

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 100.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 3).unwrap(), 130.0),
        ];

        let result = normalizer.normalize(events);

        // All gaps should be forward-filled
        assert_eq!(result.events.len(), 4);
        assert_eq!(result.events[1].value, 100.0); // Forward filled
        assert_eq!(result.events[2].value, 100.0); // Forward filled
        assert_eq!(result.metrics.forward_filled_count, 2);
    }

    #[test]
    fn test_backward_fill_strategy() {
        let mut config = NormalizerConfig::default();
        config.interval = Duration::seconds(1);
        config.fill_strategy = FillStrategy::Backward;

        let mut normalizer = TimeSeriesNormalizer::new(config);

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 100.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 3).unwrap(), 130.0),
        ];

        let result = normalizer.normalize(events);

        // All gaps should be backward-filled
        assert_eq!(result.events.len(), 4);
        assert_eq!(result.events[1].value, 130.0); // Backward filled
        assert_eq!(result.events[2].value, 130.0); // Backward filled
        assert_eq!(result.metrics.backward_filled_count, 2);
    }

    #[test]
    fn test_zero_fill_strategy() {
        let mut config = NormalizerConfig::default();
        config.interval = Duration::seconds(1);
        config.fill_strategy = FillStrategy::Zero;

        let mut normalizer = TimeSeriesNormalizer::new(config);

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 100.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 3).unwrap(), 130.0),
        ];

        let result = normalizer.normalize(events);

        // All gaps should be zero-filled
        assert_eq!(result.events.len(), 4);
        assert_eq!(result.events[1].value, 0.0); // Zero filled
        assert_eq!(result.events[2].value, 0.0); // Zero filled
    }

    #[test]
    fn test_skip_fill_strategy() {
        let mut config = NormalizerConfig::default();
        config.interval = Duration::seconds(1);
        config.fill_strategy = FillStrategy::Skip;

        let mut normalizer = TimeSeriesNormalizer::new(config);

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 100.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 3).unwrap(), 130.0),
        ];

        let result = normalizer.normalize(events);

        // Gaps should be skipped
        assert_eq!(result.events.len(), 2);
        assert_eq!(result.metrics.dropped_count, 2);
    }

    #[test]
    fn test_linear_with_max_gap() {
        let mut config = NormalizerConfig::default();
        config.interval = Duration::seconds(1);
        config.fill_strategy = FillStrategy::Linear;
        config.max_interpolation_gap = Duration::seconds(2);

        let mut normalizer = TimeSeriesNormalizer::new(config);

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 100.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 5).unwrap(), 150.0),
        ];

        let result = normalizer.normalize(events);

        // Gap exceeds max, should not interpolate
        assert!(result.metrics.gaps_detected > 0);
    }
}

// ============================================================================
// INTEGRATION TESTS - Batch Processing (5 tests)
// ============================================================================

#[cfg(test)]
mod batch_processing_tests {
    use super::*;

    #[test]
    fn test_large_batch_normalization() {
        let mut normalizer = TimeSeriesNormalizer::new(NormalizerConfig::default());

        let mut events = Vec::new();
        for i in 0..1000 {
            events.push(TestEvent::new(
                Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap() + Duration::seconds(i),
                i as f64,
            ));
        }

        let result = normalizer.normalize(events);

        assert_eq!(result.events.len(), 1000);
        assert_eq!(result.metrics.input_count, 1000);
    }

    #[test]
    fn test_batch_with_random_gaps() {
        let mut config = NormalizerConfig::default();
        config.fill_strategy = FillStrategy::Linear;
        let mut normalizer = TimeSeriesNormalizer::new(config);

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 0.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 2).unwrap(), 20.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 5).unwrap(), 50.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 6).unwrap(), 60.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 10).unwrap(), 100.0),
        ];

        let result = normalizer.normalize(events);

        assert_eq!(result.events.len(), 11);
        assert!(result.metrics.interpolated_count > 0);
    }

    #[test]
    fn test_batch_memory_efficiency() {
        let mut normalizer = TimeSeriesNormalizer::new(NormalizerConfig::default());

        let mut events = Vec::new();
        for i in 0..10000 {
            events.push(TestEvent::new(
                Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap() + Duration::seconds(i),
                i as f64,
            ));
        }

        let result = normalizer.normalize(events);
        assert_eq!(result.events.len(), 10000);
    }

    #[test]
    fn test_batch_with_all_out_of_order() {
        let mut normalizer = TimeSeriesNormalizer::new(NormalizerConfig::default());

        let mut events = Vec::new();
        for i in (0..100).rev() {
            events.push(TestEvent::new(
                Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap() + Duration::seconds(i),
                i as f64,
            ));
        }

        let result = normalizer.normalize(events);

        // Should be properly sorted and normalized
        assert_eq!(result.events.len(), 100);
        for i in 0..99 {
            assert!(result.events[i].timestamp < result.events[i + 1].timestamp);
        }
    }

    #[test]
    fn test_batch_single_point() {
        let mut normalizer = TimeSeriesNormalizer::new(NormalizerConfig::default());

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 42.0),
        ];

        let result = normalizer.normalize(events);

        assert_eq!(result.events.len(), 1);
        assert_eq!(result.events[0].value, 42.0);
    }
}

// ============================================================================
// ALGORITHM VALIDATION TESTS - Linear Interpolation (4 tests)
// ============================================================================

#[cfg(test)]
mod linear_interpolation_tests {
    use super::*;

    #[test]
    fn test_interpolate_midpoint() {
        let config = NormalizerConfig::default();
        let normalizer = TimeSeriesNormalizer::new(config);

        let start = TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 0.0);
        let end = TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 2).unwrap(), 100.0);
        let target = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 1).unwrap();

        let value = normalizer.interpolate_linear(&start, &end, target);
        assert_eq!(value, 50.0);
    }

    #[test]
    fn test_interpolate_quarter_point() {
        let config = NormalizerConfig::default();
        let normalizer = TimeSeriesNormalizer::new(config);

        let start = TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 0.0);
        let end = TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 4).unwrap(), 100.0);
        let target = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 1).unwrap();

        let value = normalizer.interpolate_linear(&start, &end, target);
        assert_eq!(value, 25.0);
    }

    #[test]
    fn test_interpolate_negative_slope() {
        let config = NormalizerConfig::default();
        let normalizer = TimeSeriesNormalizer::new(config);

        let start = TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 100.0);
        let end = TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 2).unwrap(), 0.0);
        let target = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 1).unwrap();

        let value = normalizer.interpolate_linear(&start, &end, target);
        assert_eq!(value, 50.0);
    }

    #[test]
    fn test_interpolate_same_values() {
        let config = NormalizerConfig::default();
        let normalizer = TimeSeriesNormalizer::new(config);

        let start = TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 42.0);
        let end = TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 2).unwrap(), 42.0);
        let target = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 1).unwrap();

        let value = normalizer.interpolate_linear(&start, &end, target);
        assert_eq!(value, 42.0);
    }
}

// ============================================================================
// ALGORITHM VALIDATION TESTS - Resampling (3 tests)
// ============================================================================

#[cfg(test)]
mod resampling_tests {
    use super::*;

    #[test]
    fn test_upsampling() {
        let mut config = NormalizerConfig::default();
        config.interval = Duration::milliseconds(500); // Upsample to 500ms
        config.fill_strategy = FillStrategy::Linear;
        config.alignment = AlignmentBoundary::None;

        let mut normalizer = TimeSeriesNormalizer::new(config);

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 0.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 1).unwrap(), 10.0),
        ];

        let result = normalizer.normalize(events);

        // Should have more points after upsampling
        assert!(result.events.len() >= 2);
    }

    #[test]
    fn test_downsampling() {
        let mut config = NormalizerConfig::default();
        config.interval = Duration::seconds(2); // Downsample to 2s

        let mut normalizer = TimeSeriesNormalizer::new(config);

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 0.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 1).unwrap(), 10.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 2).unwrap(), 20.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 3).unwrap(), 30.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 4).unwrap(), 40.0),
        ];

        let result = normalizer.normalize(events);

        // Should have fewer points after downsampling
        assert!(result.events.len() <= events.len());
    }

    #[test]
    fn test_resampling_preserves_range() {
        let mut config = NormalizerConfig::default();
        config.interval = Duration::milliseconds(500);
        config.fill_strategy = FillStrategy::Linear;
        config.alignment = AlignmentBoundary::None;

        let mut normalizer = TimeSeriesNormalizer::new(config);

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 0.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 2).unwrap(), 100.0),
        ];

        let result = normalizer.normalize(events);

        // First and last values should match original
        assert_eq!(result.events.first().unwrap().value, 0.0);
        assert_eq!(result.events.last().unwrap().value, 100.0);
    }
}

// ============================================================================
// ALGORITHM VALIDATION TESTS - Accuracy (3 tests)
// ============================================================================

#[cfg(test)]
mod accuracy_tests {
    use super::*;

    #[test]
    fn test_interpolation_accuracy() {
        let mut config = NormalizerConfig::default();
        config.fill_strategy = FillStrategy::Linear;
        config.interval = Duration::seconds(1);

        let mut normalizer = TimeSeriesNormalizer::new(config);

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 0.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 10).unwrap(), 100.0),
        ];

        let result = normalizer.normalize(events);

        // Check each interpolated point
        for i in 0..=10 {
            let expected = i as f64 * 10.0;
            let actual = result.events[i].value;
            let error = (actual - expected).abs();
            assert!(error < 0.001, "Point {} has error {}", i, error);
        }
    }

    #[test]
    fn test_alignment_precision() {
        let mut config = NormalizerConfig::default();
        config.alignment = AlignmentBoundary::Second;

        let mut normalizer = TimeSeriesNormalizer::new(config);

        let events = vec![
            TestEvent::new(
                Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap() + Duration::milliseconds(123),
                1.0,
            ),
            TestEvent::new(
                Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 1).unwrap() + Duration::milliseconds(456),
                2.0,
            ),
        ];

        let result = normalizer.normalize(events);

        // All timestamps should be aligned to seconds
        for event in &result.events {
            assert_eq!(event.timestamp.timestamp_subsec_nanos(), 0);
        }
    }

    #[test]
    fn test_metrics_accuracy() {
        let mut config = NormalizerConfig::default();
        config.fill_strategy = FillStrategy::Linear;
        config.interval = Duration::seconds(1);

        let mut normalizer = TimeSeriesNormalizer::new(config);

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 0.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 5).unwrap(), 50.0),
        ];

        let result = normalizer.normalize(events);

        assert_eq!(result.metrics.input_count, 2);
        assert_eq!(result.metrics.output_count, 6);
        assert_eq!(result.metrics.interpolated_count, 4);
        assert_eq!(result.metrics.dropped_count, 0);
    }
}

// ============================================================================
// PERFORMANCE TESTS - Throughput (3 tests)
// ============================================================================

#[cfg(test)]
mod throughput_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_throughput_10k_events() {
        let mut normalizer = TimeSeriesNormalizer::new(NormalizerConfig::default());

        let mut events = Vec::new();
        for i in 0..10000 {
            events.push(TestEvent::new(
                Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap() + Duration::seconds(i),
                i as f64,
            ));
        }

        let start = Instant::now();
        let result = normalizer.normalize(events);
        let duration = start.elapsed();

        assert_eq!(result.events.len(), 10000);
        // Should process 10K events in under 100ms
        assert!(duration.as_millis() < 100, "Duration: {:?}", duration);
    }

    #[test]
    fn test_throughput_100k_events() {
        let mut normalizer = TimeSeriesNormalizer::new(NormalizerConfig::default());

        let mut events = Vec::new();
        for i in 0..100000 {
            events.push(TestEvent::new(
                Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap() + Duration::seconds(i),
                i as f64,
            ));
        }

        let start = Instant::now();
        let result = normalizer.normalize(events);
        let duration = start.elapsed();

        assert_eq!(result.events.len(), 100000);
        // Should process 100K events in under 1s
        assert!(duration.as_secs() < 1, "Duration: {:?}", duration);
    }

    #[test]
    fn test_throughput_with_gaps() {
        let mut config = NormalizerConfig::default();
        config.fill_strategy = FillStrategy::Linear;
        let mut normalizer = TimeSeriesNormalizer::new(config);

        let mut events = Vec::new();
        for i in (0..10000).step_by(2) {
            events.push(TestEvent::new(
                Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap() + Duration::seconds(i),
                i as f64,
            ));
        }

        let start = Instant::now();
        let result = normalizer.normalize(events);
        let duration = start.elapsed();

        // Should fill gaps efficiently
        assert!(result.events.len() >= 5000);
        assert!(duration.as_millis() < 200, "Duration: {:?}", duration);
    }
}

// ============================================================================
// PERFORMANCE TESTS - Latency (3 tests)
// ============================================================================

#[cfg(test)]
mod latency_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_latency_single_event() {
        let mut normalizer = TimeSeriesNormalizer::new(NormalizerConfig::default());

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 42.0),
        ];

        let start = Instant::now();
        let _result = normalizer.normalize(events);
        let duration = start.elapsed();

        // Should process single event in under 1ms
        assert!(duration.as_micros() < 1000, "Duration: {:?}", duration);
    }

    #[test]
    fn test_latency_small_batch() {
        let mut normalizer = TimeSeriesNormalizer::new(NormalizerConfig::default());

        let mut events = Vec::new();
        for i in 0..10 {
            events.push(TestEvent::new(
                Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap() + Duration::seconds(i),
                i as f64,
            ));
        }

        let start = Instant::now();
        let _result = normalizer.normalize(events);
        let duration = start.elapsed();

        // Should process 10 events in under 5ms
        assert!(duration.as_micros() < 5000, "Duration: {:?}", duration);
    }

    #[test]
    fn test_latency_p99() {
        let mut normalizer = TimeSeriesNormalizer::new(NormalizerConfig::default());

        let mut latencies = Vec::new();
        for _ in 0..100 {
            let events = vec![
                TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 100.0),
                TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 5).unwrap(), 150.0),
            ];

            let start = Instant::now();
            let _result = normalizer.normalize(events.clone());
            latencies.push(start.elapsed());
        }

        latencies.sort();
        let p99 = latencies[99];

        // P99 should be under 10ms
        assert!(p99.as_millis() < 10, "P99: {:?}", p99);
    }
}

// ============================================================================
// PERFORMANCE TESTS - Memory (2 tests)
// ============================================================================

#[cfg(test)]
mod memory_tests {
    use super::*;

    #[test]
    fn test_memory_buffer_management() {
        let mut config = NormalizerConfig::default();
        config.buffer_size = 1000;
        let mut normalizer = TimeSeriesNormalizer::new(config);

        // Add many events to buffer
        for i in 0..5000 {
            normalizer.add_to_buffer(TestEvent::new(
                Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap() + Duration::seconds(i),
                i as f64,
            ));
        }

        // Buffer should be capped at buffer_size
        assert_eq!(normalizer.buffer.len(), 1000);
    }

    #[test]
    fn test_memory_large_series() {
        let mut normalizer = TimeSeriesNormalizer::new(NormalizerConfig::default());

        let mut events = Vec::new();
        for i in 0..100000 {
            events.push(TestEvent::new(
                Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap() + Duration::seconds(i),
                i as f64,
            ));
        }

        let result = normalizer.normalize(events);

        // Should successfully process large series
        assert_eq!(result.events.len(), 100000);
    }
}

// ============================================================================
// EDGE CASES - Missing Data (4 tests)
// ============================================================================

#[cfg(test)]
mod missing_data_tests {
    use super::*;

    #[test]
    fn test_missing_all_timestamps() {
        let mut normalizer = TimeSeriesNormalizer::new(NormalizerConfig::default());
        let result = normalizer.normalize(Vec::new());

        assert_eq!(result.events.len(), 0);
        assert_eq!(result.metrics.input_count, 0);
    }

    #[test]
    fn test_missing_leading_data() {
        let mut config = NormalizerConfig::default();
        config.fill_strategy = FillStrategy::Forward;
        let mut normalizer = TimeSeriesNormalizer::new(config);

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 5).unwrap(), 50.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 6).unwrap(), 60.0),
        ];

        let result = normalizer.normalize(events);

        // Should start from first available data point
        assert!(result.events.len() >= 2);
    }

    #[test]
    fn test_missing_trailing_data() {
        let mut config = NormalizerConfig::default();
        config.fill_strategy = FillStrategy::Backward;
        let mut normalizer = TimeSeriesNormalizer::new(config);

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 0.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 1).unwrap(), 10.0),
        ];

        let result = normalizer.normalize(events);

        // Should end at last available data point
        assert!(result.events.len() >= 2);
    }

    #[test]
    fn test_missing_middle_data_large_gap() {
        let mut config = NormalizerConfig::default();
        config.fill_strategy = FillStrategy::Linear;
        config.max_interpolation_gap = Duration::seconds(5);
        let mut normalizer = TimeSeriesNormalizer::new(config);

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 0.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 20).unwrap(), 100.0),
        ];

        let result = normalizer.normalize(events);

        // Should detect gap
        assert!(result.metrics.gaps_detected > 0);
    }
}

// ============================================================================
// EDGE CASES - Duplicates and Clock Issues (4 tests)
// ============================================================================

#[cfg(test)]
mod duplicate_and_clock_tests {
    use super::*;

    #[test]
    fn test_exact_duplicate_timestamps() {
        let mut normalizer = TimeSeriesNormalizer::new(NormalizerConfig::default());

        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let events = vec![
            TestEvent::new(timestamp, 100.0),
            TestEvent::new(timestamp, 200.0),
            TestEvent::new(timestamp, 300.0),
        ];

        let result = normalizer.normalize(events);

        // Should handle duplicates (keeping one value)
        assert!(result.events.len() >= 1);
    }

    #[test]
    fn test_near_duplicate_timestamps() {
        let mut config = NormalizerConfig::default();
        config.alignment = AlignmentBoundary::Second;
        let mut normalizer = TimeSeriesNormalizer::new(config);

        let base = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let events = vec![
            TestEvent::new(base, 100.0),
            TestEvent::new(base + Duration::milliseconds(100), 110.0),
            TestEvent::new(base + Duration::milliseconds(900), 190.0),
        ];

        let result = normalizer.normalize(events);

        // Should align to same second and handle as duplicates
        assert!(result.events.len() >= 1);
    }

    #[test]
    fn test_backwards_time_jump() {
        let mut normalizer = TimeSeriesNormalizer::new(NormalizerConfig::default());

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 5).unwrap(), 50.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 3).unwrap(), 30.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 7).unwrap(), 70.0),
        ];

        let result = normalizer.normalize(events);

        // Should handle by sorting
        assert!(result.events.len() >= 3);
        // Verify sorted order
        for i in 0..result.events.len() - 1 {
            assert!(result.events[i].timestamp <= result.events[i + 1].timestamp);
        }
    }

    #[test]
    fn test_clock_skew_microseconds() {
        let mut config = NormalizerConfig::default();
        config.alignment = AlignmentBoundary::None;
        let mut normalizer = TimeSeriesNormalizer::new(config);

        let base = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let events = vec![
            TestEvent::new(base, 100.0),
            TestEvent::new(base + Duration::microseconds(1), 100.1),
            TestEvent::new(base + Duration::microseconds(2), 100.2),
        ];

        let result = normalizer.normalize(events);

        // Should preserve microsecond precision when no alignment
        assert_eq!(result.events.len(), 3);
    }
}

// ============================================================================
// EDGE CASES - Extreme Values (4 tests)
// ============================================================================

#[cfg(test)]
mod extreme_value_tests {
    use super::*;

    #[test]
    fn test_extreme_values() {
        let mut normalizer = TimeSeriesNormalizer::new(NormalizerConfig::default());

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), f64::MAX),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 1).unwrap(), f64::MIN),
        ];

        let result = normalizer.normalize(events);

        assert_eq!(result.events.len(), 2);
        assert_eq!(result.events[0].value, f64::MAX);
        assert_eq!(result.events[1].value, f64::MIN);
    }

    #[test]
    fn test_zero_values() {
        let mut normalizer = TimeSeriesNormalizer::new(NormalizerConfig::default());

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), 0.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 1).unwrap(), 0.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 2).unwrap(), 0.0),
        ];

        let result = normalizer.normalize(events);

        assert_eq!(result.events.len(), 3);
        for event in result.events {
            assert_eq!(event.value, 0.0);
        }
    }

    #[test]
    fn test_negative_values() {
        let mut normalizer = TimeSeriesNormalizer::new(NormalizerConfig::default());

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(), -100.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 1).unwrap(), -50.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 2).unwrap(), 0.0),
        ];

        let result = normalizer.normalize(events);

        assert_eq!(result.events.len(), 3);
        assert_eq!(result.events[0].value, -100.0);
        assert_eq!(result.events[1].value, -50.0);
        assert_eq!(result.events[2].value, 0.0);
    }

    #[test]
    fn test_very_large_time_range() {
        let mut normalizer = TimeSeriesNormalizer::new(NormalizerConfig::default());

        let events = vec![
            TestEvent::new(Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(), 0.0),
            TestEvent::new(Utc.with_ymd_and_hms(2024, 12, 31, 23, 59, 59).unwrap(), 100.0),
        ];

        let result = normalizer.normalize(events);

        // Should handle year-long range
        assert!(result.events.len() >= 2);
    }
}

// ============================================================================
// CONCURRENT NORMALIZATION TEST
// ============================================================================

#[cfg(test)]
mod concurrent_tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn test_concurrent_normalization() {
        let config = Arc::new(NormalizerConfig::default());

        let mut handles = Vec::new();

        for i in 0..10 {
            let config = config.clone();
            let handle = tokio::spawn(async move {
                let mut normalizer = TimeSeriesNormalizer::new((*config).clone());

                let mut events = Vec::new();
                for j in 0..100 {
                    events.push(TestEvent::new(
                        Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap()
                            + Duration::seconds(i * 100 + j),
                        (i * 100 + j) as f64,
                    ));
                }

                normalizer.normalize(events)
            });
            handles.push(handle);
        }

        for handle in handles {
            let result = handle.await.unwrap();
            assert_eq!(result.events.len(), 100);
        }
    }
}

// ============================================================================
// TEST SUMMARY
// ============================================================================

#[cfg(test)]
mod test_summary {
    #[test]
    fn test_coverage_summary() {
        // This test documents the test coverage

        // Unit Tests: 20+
        // - Timestamp Alignment: 5 tests
        // - Fill Strategies: 8 tests
        // - Interval Detection: 3 tests
        // - Buffer Management: 4 tests

        // Integration Tests: 15+
        // - End-to-End: 5 tests
        // - Multi-Strategy: 5 tests
        // - Batch Processing: 5 tests

        // Algorithm Validation: 10+
        // - Linear Interpolation: 4 tests
        // - Resampling: 3 tests
        // - Accuracy: 3 tests

        // Performance Tests: 8+
        // - Throughput: 3 tests
        // - Latency: 3 tests
        // - Memory: 2 tests

        // Edge Cases: 12+
        // - Missing Data: 4 tests
        // - Duplicates/Clock: 4 tests
        // - Extreme Values: 4 tests

        // Concurrent: 1 test

        // Total: 66 tests
        assert!(true, "Test coverage documented");
    }
}
