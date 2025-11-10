//! Statistical utilities for analyzers
//!
//! This module provides common statistical functions used by analyzers,
//! including percentile calculation, moving averages, anomaly detection, etc.

use std::collections::VecDeque;

/// Circular buffer for maintaining a sliding window of values
#[derive(Debug, Clone)]
pub struct CircularBuffer<T> {
    buffer: VecDeque<T>,
    capacity: usize,
}

impl<T> CircularBuffer<T> {
    /// Create a new circular buffer with the given capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Add a value to the buffer
    pub fn push(&mut self, value: T) {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(value);
    }

    /// Get the current length
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Get a reference to the buffer
    pub fn as_slice(&self) -> &VecDeque<T> {
        &self.buffer
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Get the buffer capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Check if the buffer is at capacity
    pub fn is_full(&self) -> bool {
        self.buffer.len() >= self.capacity
    }
}

impl<T: Clone> CircularBuffer<T> {
    /// Get all values as a vector
    pub fn to_vec(&self) -> Vec<T> {
        self.buffer.iter().cloned().collect()
    }
}

/// Exponential Moving Average calculator
#[derive(Debug, Clone)]
pub struct ExponentialMovingAverage {
    alpha: f64,
    current: Option<f64>,
}

impl ExponentialMovingAverage {
    /// Create a new EMA calculator with the given smoothing factor (0.0-1.0)
    ///
    /// Higher alpha means more weight on recent values.
    /// Typical values: 0.1 (slow), 0.3 (medium), 0.5 (fast)
    pub fn new(alpha: f64) -> Self {
        Self {
            alpha: alpha.clamp(0.0, 1.0),
            current: None,
        }
    }

    /// Update the EMA with a new value
    pub fn update(&mut self, value: f64) {
        self.current = Some(match self.current {
            None => value,
            Some(prev) => self.alpha * value + (1.0 - self.alpha) * prev,
        });
    }

    /// Get the current EMA value
    pub fn value(&self) -> Option<f64> {
        self.current
    }

    /// Reset the EMA
    pub fn reset(&mut self) {
        self.current = None;
    }
}

/// Simple Moving Average calculator
#[derive(Debug, Clone)]
pub struct SimpleMovingAverage {
    window: CircularBuffer<f64>,
    sum: f64,
}

impl SimpleMovingAverage {
    /// Create a new SMA calculator with the given window size
    pub fn new(window_size: usize) -> Self {
        Self {
            window: CircularBuffer::new(window_size),
            sum: 0.0,
        }
    }

    /// Add a new value
    pub fn update(&mut self, value: f64) {
        if self.window.is_full() {
            // Remove oldest value from sum
            if let Some(&oldest) = self.window.as_slice().front() {
                self.sum -= oldest;
            }
        }

        self.window.push(value);
        self.sum += value;
    }

    /// Get the current average
    pub fn value(&self) -> Option<f64> {
        if self.window.is_empty() {
            None
        } else {
            Some(self.sum / self.window.len() as f64)
        }
    }

    /// Get the count of values
    pub fn count(&self) -> usize {
        self.window.len()
    }

    /// Reset the SMA
    pub fn reset(&mut self) {
        self.window.clear();
        self.sum = 0.0;
    }
}

/// Calculate percentiles from a dataset
pub fn calculate_percentiles(mut values: Vec<f64>, percentiles: &[f64]) -> Vec<(f64, f64)> {
    if values.is_empty() {
        return percentiles.iter().map(|&p| (p, 0.0)).collect();
    }

    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    percentiles
        .iter()
        .map(|&p| {
            let idx = ((p / 100.0) * (values.len() - 1) as f64).round() as usize;
            let value = values[idx.min(values.len() - 1)];
            (p, value)
        })
        .collect()
}

/// Calculate mean of a dataset
pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

/// Calculate standard deviation of a dataset
pub fn std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }

    let m = mean(values);
    let variance = values.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
    variance.sqrt()
}

/// Calculate z-score for a value
pub fn z_score(value: f64, mean: f64, std_dev: f64) -> f64 {
    if std_dev == 0.0 {
        return 0.0;
    }
    (value - mean) / std_dev
}

/// Check if a value is an outlier using z-score method
///
/// A value is considered an outlier if |z-score| > threshold
/// Common threshold: 3.0 (99.7% of data within ±3σ)
pub fn is_outlier_zscore(value: f64, mean: f64, std_dev: f64, threshold: f64) -> bool {
    z_score(value, mean, std_dev).abs() > threshold
}

/// Calculate interquartile range (IQR)
pub fn calculate_iqr(mut values: Vec<f64>) -> (f64, f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0, 0.0);
    }

    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let q1_idx = (values.len() / 4).max(0);
    let q3_idx = (3 * values.len() / 4).min(values.len() - 1);

    let q1 = values[q1_idx];
    let q3 = values[q3_idx];
    let iqr = q3 - q1;

    (q1, q3, iqr)
}

/// Check if a value is an outlier using IQR method
///
/// A value is an outlier if it's below Q1 - multiplier*IQR or above Q3 + multiplier*IQR
/// Common multiplier: 1.5 (standard outlier), 3.0 (extreme outlier)
pub fn is_outlier_iqr(value: f64, values: &[f64], multiplier: f64) -> bool {
    if values.len() < 4 {
        return false;
    }

    let (q1, q3, iqr) = calculate_iqr(values.to_vec());
    let lower_bound = q1 - multiplier * iqr;
    let upper_bound = q3 + multiplier * iqr;

    value < lower_bound || value > upper_bound
}

/// Calculate rate (events per time unit)
pub fn calculate_rate(count: u64, duration_secs: f64) -> f64 {
    if duration_secs == 0.0 {
        return 0.0;
    }
    count as f64 / duration_secs
}

/// Calculate percentage change
pub fn percentage_change(old: f64, new: f64) -> f64 {
    if old == 0.0 {
        if new == 0.0 {
            return 0.0;
        }
        return 100.0; // or f64::INFINITY
    }
    ((new - old) / old) * 100.0
}

/// Linear regression to find trend
#[derive(Debug, Clone)]
pub struct LinearTrend {
    /// Slope of the trend line
    pub slope: f64,
    /// Intercept of the trend line
    pub intercept: f64,
    /// R-squared (coefficient of determination)
    pub r_squared: f64,
}

/// Calculate linear trend from time series data
pub fn calculate_linear_trend(values: &[(f64, f64)]) -> Option<LinearTrend> {
    if values.len() < 2 {
        return None;
    }

    let n = values.len() as f64;
    let sum_x: f64 = values.iter().map(|(x, _)| x).sum();
    let sum_y: f64 = values.iter().map(|(_, y)| y).sum();
    let sum_xy: f64 = values.iter().map(|(x, y)| x * y).sum();
    let sum_x2: f64 = values.iter().map(|(x, _)| x * x).sum();
    let sum_y2: f64 = values.iter().map(|(_, y)| y * y).sum();

    let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x * sum_x);
    let intercept = (sum_y - slope * sum_x) / n;

    // Calculate R-squared
    let mean_y = sum_y / n;
    let ss_tot: f64 = values.iter().map(|(_, y)| (y - mean_y).powi(2)).sum();
    let ss_res: f64 = values
        .iter()
        .map(|(x, y)| {
            let predicted = slope * x + intercept;
            (y - predicted).powi(2)
        })
        .sum();

    let r_squared = if ss_tot == 0.0 {
        1.0
    } else {
        1.0 - (ss_res / ss_tot)
    };

    Some(LinearTrend {
        slope,
        intercept,
        r_squared,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circular_buffer() {
        let mut buffer = CircularBuffer::new(3);
        assert!(buffer.is_empty());

        buffer.push(1);
        buffer.push(2);
        buffer.push(3);
        assert!(buffer.is_full());
        assert_eq!(buffer.len(), 3);

        buffer.push(4);
        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.to_vec(), vec![2, 3, 4]);
    }

    #[test]
    fn test_exponential_moving_average() {
        let mut ema = ExponentialMovingAverage::new(0.5);
        assert_eq!(ema.value(), None);

        ema.update(10.0);
        assert_eq!(ema.value(), Some(10.0));

        ema.update(20.0);
        assert_eq!(ema.value(), Some(15.0)); // 0.5 * 20 + 0.5 * 10

        ema.reset();
        assert_eq!(ema.value(), None);
    }

    #[test]
    fn test_simple_moving_average() {
        let mut sma = SimpleMovingAverage::new(3);
        assert_eq!(sma.value(), None);

        sma.update(10.0);
        assert_eq!(sma.value(), Some(10.0));

        sma.update(20.0);
        assert_eq!(sma.value(), Some(15.0));

        sma.update(30.0);
        assert_eq!(sma.value(), Some(20.0));

        sma.update(40.0); // Should drop 10.0
        assert_eq!(sma.value(), Some(30.0)); // (20 + 30 + 40) / 3
    }

    #[test]
    fn test_calculate_percentiles() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let percentiles = calculate_percentiles(values, &[50.0, 95.0, 99.0]);

        assert_eq!(percentiles.len(), 3);
        assert!((percentiles[0].1 - 5.5).abs() < 0.1); // p50 ≈ 5.5
        assert!((percentiles[1].1 - 10.0).abs() < 0.1); // p95 ≈ 10.0
    }

    #[test]
    fn test_mean_and_std_dev() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(mean(&values), 3.0);

        let std = std_dev(&values);
        assert!((std - 1.58).abs() < 0.01); // std dev ≈ 1.58
    }

    #[test]
    fn test_z_score() {
        let score = z_score(100.0, 50.0, 10.0);
        assert_eq!(score, 5.0);

        let score = z_score(40.0, 50.0, 10.0);
        assert_eq!(score, -1.0);
    }

    #[test]
    fn test_is_outlier_zscore() {
        assert!(is_outlier_zscore(100.0, 50.0, 10.0, 3.0)); // z=5.0 > 3.0
        assert!(!is_outlier_zscore(65.0, 50.0, 10.0, 3.0)); // z=1.5 < 3.0
    }

    #[test]
    fn test_calculate_iqr() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let (q1, q3, iqr) = calculate_iqr(values);

        assert_eq!(q1, 2.0);
        assert_eq!(q3, 6.0);
        assert_eq!(iqr, 4.0);
    }

    #[test]
    fn test_is_outlier_iqr() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

        // Q1=2, Q3=6, IQR=4, bounds: [2-1.5*4, 6+1.5*4] = [-4, 12]
        assert!(!is_outlier_iqr(10.0, &values, 1.5));
        assert!(is_outlier_iqr(20.0, &values, 1.5));
    }

    #[test]
    fn test_calculate_rate() {
        assert_eq!(calculate_rate(100, 10.0), 10.0);
        assert_eq!(calculate_rate(0, 10.0), 0.0);
        assert_eq!(calculate_rate(100, 0.0), 0.0);
    }

    #[test]
    fn test_percentage_change() {
        assert_eq!(percentage_change(100.0, 150.0), 50.0);
        assert_eq!(percentage_change(100.0, 50.0), -50.0);
        assert_eq!(percentage_change(0.0, 100.0), 100.0);
    }

    #[test]
    fn test_calculate_linear_trend() {
        let data = vec![(1.0, 2.0), (2.0, 4.0), (3.0, 6.0), (4.0, 8.0)];
        let trend = calculate_linear_trend(&data).unwrap();

        assert!((trend.slope - 2.0).abs() < 0.01);
        assert!((trend.intercept - 0.0).abs() < 0.01);
        assert!((trend.r_squared - 1.0).abs() < 0.01); // Perfect linear fit
    }
}
