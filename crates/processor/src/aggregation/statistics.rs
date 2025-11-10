//! Enhanced statistical utilities for stream processing
//!
//! This module provides additional statistical functions and utilities
//! optimized for stream processing scenarios.

use crate::error::{AggregationError, AggregationResult};
use serde::{Deserialize, Serialize};
use statrs::distribution::{ContinuousCDF, Normal};
use statrs::statistics::{Data, OrderStatistics, Statistics};
use std::collections::VecDeque;

/// Online variance calculator using Welford's algorithm
///
/// Provides numerically stable variance calculation for streaming data
/// without storing all values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineVariance {
    count: u64,
    mean: f64,
    m2: f64,
}

impl OnlineVariance {
    /// Create a new online variance calculator
    pub fn new() -> Self {
        Self {
            count: 0,
            mean: 0.0,
            m2: 0.0,
        }
    }

    /// Update with a new value
    pub fn update(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }

    /// Get the current mean
    pub fn mean(&self) -> f64 {
        self.mean
    }

    /// Get the variance (sample variance)
    pub fn variance(&self) -> Option<f64> {
        if self.count < 2 {
            None
        } else {
            Some(self.m2 / (self.count - 1) as f64)
        }
    }

    /// Get the standard deviation
    pub fn std_dev(&self) -> Option<f64> {
        self.variance().map(|v| v.sqrt())
    }

    /// Get the population variance
    pub fn population_variance(&self) -> Option<f64> {
        if self.count == 0 {
            None
        } else {
            Some(self.m2 / self.count as f64)
        }
    }

    /// Get the count of values
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Merge with another online variance calculator
    pub fn merge(&mut self, other: &OnlineVariance) {
        if other.count == 0 {
            return;
        }

        let combined_count = self.count + other.count;
        let delta = other.mean - self.mean;
        let combined_mean = (self.count as f64 * self.mean + other.count as f64 * other.mean)
            / combined_count as f64;

        self.m2 = self.m2
            + other.m2
            + delta * delta * (self.count as f64 * other.count as f64) / combined_count as f64;

        self.count = combined_count;
        self.mean = combined_mean;
    }

    /// Reset to initial state
    pub fn reset(&mut self) {
        self.count = 0;
        self.mean = 0.0;
        self.m2 = 0.0;
    }
}

impl Default for OnlineVariance {
    fn default() -> Self {
        Self::new()
    }
}

/// Exponential moving average calculator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExponentialMovingAverage {
    alpha: f64,
    current: Option<f64>,
}

impl ExponentialMovingAverage {
    /// Create a new EMA with the given smoothing factor (0 < alpha <= 1)
    pub fn new(alpha: f64) -> AggregationResult<Self> {
        if alpha <= 0.0 || alpha > 1.0 {
            return Err(AggregationError::InvalidValue {
                value: alpha,
                reason: "Alpha must be between 0 and 1".to_string(),
            });
        }

        Ok(Self {
            alpha,
            current: None,
        })
    }

    /// Create an EMA with a span (number of periods)
    pub fn with_span(span: f64) -> AggregationResult<Self> {
        let alpha = 2.0 / (span + 1.0);
        Self::new(alpha)
    }

    /// Update with a new value
    pub fn update(&mut self, value: f64) {
        self.current = Some(match self.current {
            Some(current) => self.alpha * value + (1.0 - self.alpha) * current,
            None => value,
        });
    }

    /// Get the current EMA value
    pub fn value(&self) -> Option<f64> {
        self.current
    }

    /// Reset to initial state
    pub fn reset(&mut self) {
        self.current = None;
    }
}

/// Sliding window statistics
///
/// Maintains a fixed-size sliding window of recent values
/// and computes statistics over that window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlidingWindowStats {
    window_size: usize,
    values: VecDeque<f64>,
}

impl SlidingWindowStats {
    /// Create a new sliding window with the given size
    pub fn new(window_size: usize) -> AggregationResult<Self> {
        if window_size == 0 {
            return Err(AggregationError::InvalidValue {
                value: window_size as f64,
                reason: "Window size must be greater than 0".to_string(),
            });
        }

        Ok(Self {
            window_size,
            values: VecDeque::with_capacity(window_size),
        })
    }

    /// Add a new value to the window
    pub fn update(&mut self, value: f64) {
        if self.values.len() >= self.window_size {
            self.values.pop_front();
        }
        self.values.push_back(value);
    }

    /// Get the current window size (may be less than max if not full)
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Check if the window is empty
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Get the mean of values in the window
    pub fn mean(&self) -> Option<f64> {
        if self.values.is_empty() {
            None
        } else {
            Some(self.values.iter().sum::<f64>() / self.values.len() as f64)
        }
    }

    /// Get the min value in the window
    pub fn min(&self) -> Option<f64> {
        self.values.iter().copied().min_by(|a, b| {
            a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Get the max value in the window
    pub fn max(&self) -> Option<f64> {
        self.values.iter().copied().max_by(|a, b| {
            a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Get the median value in the window
    pub fn median(&self) -> Option<f64> {
        if self.values.is_empty() {
            return None;
        }

        let mut sorted: Vec<f64> = self.values.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let len = sorted.len();
        if len % 2 == 0 {
            Some((sorted[len / 2 - 1] + sorted[len / 2]) / 2.0)
        } else {
            Some(sorted[len / 2])
        }
    }

    /// Get a percentile value (0-100)
    pub fn percentile(&self, p: f64) -> AggregationResult<f64> {
        if self.values.is_empty() {
            return Err(AggregationError::InsufficientData {
                aggregation_type: "percentile".to_string(),
                required: 1,
                actual: 0,
            });
        }

        if p < 0.0 || p > 100.0 {
            return Err(AggregationError::InvalidValue {
                value: p,
                reason: "Percentile must be between 0 and 100".to_string(),
            });
        }

        let mut sorted: Vec<f64> = self.values.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let index = (p / 100.0 * (sorted.len() - 1) as f64).round() as usize;
        Ok(sorted[index])
    }

    /// Get standard deviation
    pub fn std_dev(&self) -> Option<f64> {
        if self.values.len() < 2 {
            return None;
        }

        let mean = self.mean()?;
        let variance = self
            .values
            .iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>()
            / (self.values.len() - 1) as f64;

        Some(variance.sqrt())
    }

    /// Reset the window
    pub fn reset(&mut self) {
        self.values.clear();
    }
}

/// Rate calculator for computing events per second
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateCalculator {
    event_count: u64,
    start_time: Option<chrono::DateTime<chrono::Utc>>,
    last_time: Option<chrono::DateTime<chrono::Utc>>,
}

impl RateCalculator {
    /// Create a new rate calculator
    pub fn new() -> Self {
        Self {
            event_count: 0,
            start_time: None,
            last_time: None,
        }
    }

    /// Record an event
    pub fn record_event(&mut self, timestamp: chrono::DateTime<chrono::Utc>) {
        self.event_count += 1;

        if self.start_time.is_none() {
            self.start_time = Some(timestamp);
        }

        self.last_time = Some(timestamp);
    }

    /// Get the overall rate (events per second)
    pub fn overall_rate(&self) -> Option<f64> {
        if self.event_count == 0 {
            return Some(0.0);
        }

        let start = self.start_time?;
        let end = self.last_time?;

        let duration = end.signed_duration_since(start);
        let seconds = duration.num_milliseconds() as f64 / 1000.0;

        if seconds > 0.0 {
            Some(self.event_count as f64 / seconds)
        } else {
            None
        }
    }

    /// Get event count
    pub fn count(&self) -> u64 {
        self.event_count
    }

    /// Reset the calculator
    pub fn reset(&mut self) {
        self.event_count = 0;
        self.start_time = None;
        self.last_time = None;
    }
}

impl Default for RateCalculator {
    fn default() -> Self {
        Self::new()
    }
}

/// Anomaly detector using z-score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZScoreAnomalyDetector {
    variance: OnlineVariance,
    threshold: f64,
}

impl ZScoreAnomalyDetector {
    /// Create a new anomaly detector with the given z-score threshold
    pub fn new(threshold: f64) -> Self {
        Self {
            variance: OnlineVariance::new(),
            threshold,
        }
    }

    /// Update with a new value and check if it's an anomaly
    pub fn check(&mut self, value: f64) -> bool {
        let is_anomaly = if let Some(std_dev) = self.variance.std_dev() {
            if std_dev > 0.0 {
                let z_score = (value - self.variance.mean()).abs() / std_dev;
                z_score > self.threshold
            } else {
                false
            }
        } else {
            false
        };

        self.variance.update(value);
        is_anomaly
    }

    /// Get the current mean
    pub fn mean(&self) -> f64 {
        self.variance.mean()
    }

    /// Get the current standard deviation
    pub fn std_dev(&self) -> Option<f64> {
        self.variance.std_dev()
    }

    /// Reset the detector
    pub fn reset(&mut self) {
        self.variance.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_online_variance() {
        let mut ov = OnlineVariance::new();

        let values = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        for &v in &values {
            ov.update(v);
        }

        assert_eq!(ov.count(), 5);
        assert_eq!(ov.mean(), 30.0);

        let std_dev = ov.std_dev().unwrap();
        assert!((std_dev - 15.811).abs() < 0.01);
    }

    #[test]
    fn test_online_variance_merge() {
        let mut ov1 = OnlineVariance::new();
        ov1.update(10.0);
        ov1.update(20.0);

        let mut ov2 = OnlineVariance::new();
        ov2.update(30.0);
        ov2.update(40.0);

        ov1.merge(&ov2);

        assert_eq!(ov1.count(), 4);
        assert_eq!(ov1.mean(), 25.0);
    }

    #[test]
    fn test_ema() {
        let mut ema = ExponentialMovingAverage::new(0.5).unwrap();

        ema.update(10.0);
        assert_eq!(ema.value(), Some(10.0));

        ema.update(20.0);
        assert_eq!(ema.value(), Some(15.0));

        ema.update(30.0);
        assert_eq!(ema.value(), Some(22.5));
    }

    #[test]
    fn test_ema_with_span() {
        let mut ema = ExponentialMovingAverage::with_span(9.0).unwrap();

        for i in 1..=10 {
            ema.update(i as f64);
        }

        assert!(ema.value().is_some());
        let value = ema.value().unwrap();
        assert!(value > 5.0 && value < 10.0);
    }

    #[test]
    fn test_sliding_window_stats() {
        let mut sw = SlidingWindowStats::new(3).unwrap();

        sw.update(10.0);
        sw.update(20.0);
        sw.update(30.0);

        assert_eq!(sw.len(), 3);
        assert_eq!(sw.mean(), Some(20.0));
        assert_eq!(sw.min(), Some(10.0));
        assert_eq!(sw.max(), Some(30.0));

        // Add another value (should evict 10.0)
        sw.update(40.0);
        assert_eq!(sw.len(), 3);
        assert_eq!(sw.mean(), Some(30.0));
        assert_eq!(sw.min(), Some(20.0));
    }

    #[test]
    fn test_sliding_window_median() {
        let mut sw = SlidingWindowStats::new(5).unwrap();

        sw.update(10.0);
        sw.update(20.0);
        sw.update(30.0);
        sw.update(40.0);
        sw.update(50.0);

        assert_eq!(sw.median(), Some(30.0));
    }

    #[test]
    fn test_sliding_window_percentile() {
        let mut sw = SlidingWindowStats::new(5).unwrap();

        for i in 1..=5 {
            sw.update(i as f64 * 10.0);
        }

        let p50 = sw.percentile(50.0).unwrap();
        assert_eq!(p50, 30.0);

        let p95 = sw.percentile(95.0).unwrap();
        assert_eq!(p95, 50.0);
    }

    #[test]
    fn test_rate_calculator() {
        let mut rate = RateCalculator::new();

        let base_time = chrono::Utc::now();

        rate.record_event(base_time);
        rate.record_event(base_time + chrono::Duration::milliseconds(500));
        rate.record_event(base_time + chrono::Duration::seconds(1));

        assert_eq!(rate.count(), 3);

        let overall_rate = rate.overall_rate().unwrap();
        assert!(overall_rate > 0.0);
        assert!(overall_rate <= 3.0); // Should be around 3 events/sec
    }

    #[test]
    fn test_zscore_anomaly_detector() {
        let mut detector = ZScoreAnomalyDetector::new(2.0);

        // Feed normal values
        for i in 1..=10 {
            let is_anomaly = detector.check(i as f64);
            // First few values might be anomalies until we have enough data
        }

        // Feed an anomaly (way outside normal range)
        let is_anomaly = detector.check(100.0);
        assert!(is_anomaly);
    }

    #[test]
    fn test_online_variance_reset() {
        let mut ov = OnlineVariance::new();
        ov.update(10.0);
        ov.update(20.0);

        assert_eq!(ov.count(), 2);

        ov.reset();
        assert_eq!(ov.count(), 0);
        assert_eq!(ov.mean(), 0.0);
    }

    #[test]
    fn test_sliding_window_empty() {
        let sw = SlidingWindowStats::new(5).unwrap();

        assert!(sw.is_empty());
        assert_eq!(sw.len(), 0);
        assert_eq!(sw.mean(), None);
        assert_eq!(sw.median(), None);
    }
}
