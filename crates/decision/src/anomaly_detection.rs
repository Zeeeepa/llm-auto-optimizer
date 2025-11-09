//! Anomaly Detection
//!
//! This module provides algorithms for detecting anomalies in metrics
//! using statistical methods and machine learning approaches.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::errors::{DecisionError, Result};

/// Anomaly detection result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnomalyResult {
    /// Is this an anomaly?
    pub is_anomaly: bool,
    /// Anomaly score (higher = more anomalous)
    pub score: f64,
    /// Severity level (0.0 - 1.0)
    pub severity: f64,
}

impl AnomalyResult {
    /// Create normal result
    pub fn normal(score: f64) -> Self {
        Self {
            is_anomaly: false,
            score,
            severity: 0.0,
        }
    }

    /// Create anomaly result
    pub fn anomaly(score: f64, severity: f64) -> Self {
        Self {
            is_anomaly: true,
            score,
            severity: severity.clamp(0.0, 1.0),
        }
    }
}

/// Z-score based anomaly detector
pub struct ZScoreDetector {
    /// Window of observations
    window: VecDeque<f64>,
    /// Window size
    window_size: usize,
    /// Z-score threshold
    threshold: f64,
    /// Sum for mean calculation
    sum: f64,
    /// Sum of squares for variance
    sum_squares: f64,
}

impl ZScoreDetector {
    /// Create new Z-score detector
    pub fn new(window_size: usize, threshold: f64) -> Result<Self> {
        if window_size < 2 {
            return Err(DecisionError::InvalidParameter(
                "Window size must be at least 2".to_string(),
            ));
        }

        if threshold <= 0.0 {
            return Err(DecisionError::InvalidParameter(
                "Threshold must be positive".to_string(),
            ));
        }

        Ok(Self {
            window: VecDeque::with_capacity(window_size),
            window_size,
            threshold,
            sum: 0.0,
            sum_squares: 0.0,
        })
    }

    /// Add observation and detect anomaly
    pub fn add(&mut self, value: f64) -> AnomalyResult {
        // Add to window
        if self.window.len() >= self.window_size {
            if let Some(old) = self.window.pop_front() {
                self.sum -= old;
                self.sum_squares -= old * old;
            }
        }

        self.window.push_back(value);
        self.sum += value;
        self.sum_squares += value * value;

        // Need enough data
        if self.window.len() < 3 {
            return AnomalyResult::normal(0.0);
        }

        // Calculate statistics
        let n = self.window.len() as f64;
        let mean = self.sum / n;
        let variance = (self.sum_squares / n) - (mean * mean);
        let std_dev = variance.max(0.0).sqrt();

        if std_dev == 0.0 {
            return AnomalyResult::normal(0.0);
        }

        // Calculate z-score
        let z_score = ((value - mean) / std_dev).abs();

        if z_score > self.threshold {
            let severity = (z_score / self.threshold - 1.0).min(1.0);
            AnomalyResult::anomaly(z_score, severity)
        } else {
            AnomalyResult::normal(z_score)
        }
    }

    /// Get current mean
    pub fn mean(&self) -> Option<f64> {
        if self.window.is_empty() {
            None
        } else {
            Some(self.sum / self.window.len() as f64)
        }
    }

    /// Get current standard deviation
    pub fn std_dev(&self) -> Option<f64> {
        if self.window.len() < 2 {
            None
        } else {
            let n = self.window.len() as f64;
            let mean = self.sum / n;
            let variance = (self.sum_squares / n) - (mean * mean);
            Some(variance.max(0.0).sqrt())
        }
    }

    /// Reset detector
    pub fn reset(&mut self) {
        self.window.clear();
        self.sum = 0.0;
        self.sum_squares = 0.0;
    }
}

/// IQR (Interquartile Range) based anomaly detector
pub struct IQRDetector {
    /// Window of observations
    window: VecDeque<f64>,
    /// Window size
    window_size: usize,
    /// IQR multiplier (typically 1.5 or 3.0)
    multiplier: f64,
}

impl IQRDetector {
    /// Create new IQR detector
    pub fn new(window_size: usize, multiplier: f64) -> Result<Self> {
        if window_size < 4 {
            return Err(DecisionError::InvalidParameter(
                "Window size must be at least 4".to_string(),
            ));
        }

        if multiplier <= 0.0 {
            return Err(DecisionError::InvalidParameter(
                "Multiplier must be positive".to_string(),
            ));
        }

        Ok(Self {
            window: VecDeque::with_capacity(window_size),
            window_size,
            multiplier,
        })
    }

    /// Add observation and detect anomaly
    pub fn add(&mut self, value: f64) -> AnomalyResult {
        // Add to window
        if self.window.len() >= self.window_size {
            self.window.pop_front();
        }
        self.window.push_back(value);

        // Need enough data
        if self.window.len() < 4 {
            return AnomalyResult::normal(0.0);
        }

        // Calculate IQR
        let mut sorted: Vec<f64> = self.window.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let q1 = percentile(&sorted, 25.0);
        let q3 = percentile(&sorted, 75.0);
        let iqr = q3 - q1;

        let lower_bound = q1 - self.multiplier * iqr;
        let upper_bound = q3 + self.multiplier * iqr;

        // Check if value is an outlier
        if value < lower_bound || value > upper_bound {
            let distance = if value < lower_bound {
                lower_bound - value
            } else {
                value - upper_bound
            };

            let score = distance / iqr.max(0.001);
            let severity = (score / self.multiplier).min(1.0);

            AnomalyResult::anomaly(score, severity)
        } else {
            // Calculate score relative to bounds
            let mid = (q1 + q3) / 2.0;
            let score = ((value - mid).abs() / (iqr / 2.0).max(0.001)).min(1.0);
            AnomalyResult::normal(score)
        }
    }

    /// Reset detector
    pub fn reset(&mut self) {
        self.window.clear();
    }
}

/// MAD (Median Absolute Deviation) based anomaly detector
pub struct MADDetector {
    /// Window of observations
    window: VecDeque<f64>,
    /// Window size
    window_size: usize,
    /// MAD multiplier (threshold)
    threshold: f64,
}

impl MADDetector {
    /// Create new MAD detector
    pub fn new(window_size: usize, threshold: f64) -> Result<Self> {
        if window_size < 3 {
            return Err(DecisionError::InvalidParameter(
                "Window size must be at least 3".to_string(),
            ));
        }

        if threshold <= 0.0 {
            return Err(DecisionError::InvalidParameter(
                "Threshold must be positive".to_string(),
            ));
        }

        Ok(Self {
            window: VecDeque::with_capacity(window_size),
            window_size,
            threshold,
        })
    }

    /// Add observation and detect anomaly
    pub fn add(&mut self, value: f64) -> AnomalyResult {
        // Add to window
        if self.window.len() >= self.window_size {
            self.window.pop_front();
        }
        self.window.push_back(value);

        // Need enough data
        if self.window.len() < 3 {
            return AnomalyResult::normal(0.0);
        }

        // Calculate median
        let mut sorted: Vec<f64> = self.window.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = percentile(&sorted, 50.0);

        // Calculate MAD
        let deviations: Vec<f64> = sorted.iter().map(|x| (x - median).abs()).collect();
        let mut sorted_dev = deviations.clone();
        sorted_dev.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mad = percentile(&sorted_dev, 50.0);

        // Modified z-score
        let constant = 1.4826; // For normally distributed data
        let modified_z = if mad > 0.0 {
            constant * (value - median).abs() / mad
        } else {
            0.0
        };

        if modified_z > self.threshold {
            let severity = (modified_z / self.threshold - 1.0).min(1.0);
            AnomalyResult::anomaly(modified_z, severity)
        } else {
            AnomalyResult::normal(modified_z)
        }
    }

    /// Reset detector
    pub fn reset(&mut self) {
        self.window.clear();
    }
}

/// Multi-dimensional anomaly detector using Mahalanobis distance
pub struct MahalanobisDetector {
    /// Window of observation vectors
    window: VecDeque<Vec<f64>>,
    /// Window size
    window_size: usize,
    /// Number of dimensions
    dimensions: usize,
    /// Distance threshold
    threshold: f64,
}

impl MahalanobisDetector {
    /// Create new Mahalanobis detector
    pub fn new(window_size: usize, dimensions: usize, threshold: f64) -> Result<Self> {
        if window_size < dimensions + 1 {
            return Err(DecisionError::InvalidParameter(
                "Window size must be greater than dimensions".to_string(),
            ));
        }

        if dimensions == 0 {
            return Err(DecisionError::InvalidParameter(
                "Dimensions must be positive".to_string(),
            ));
        }

        Ok(Self {
            window: VecDeque::with_capacity(window_size),
            window_size,
            dimensions,
            threshold,
        })
    }

    /// Add observation vector and detect anomaly
    pub fn add(&mut self, values: &[f64]) -> Result<AnomalyResult> {
        if values.len() != self.dimensions {
            return Err(DecisionError::InvalidParameter(format!(
                "Expected {} dimensions, got {}",
                self.dimensions,
                values.len()
            )));
        }

        // Add to window
        if self.window.len() >= self.window_size {
            self.window.pop_front();
        }
        self.window.push_back(values.to_vec());

        // Need enough data
        if self.window.len() < self.dimensions + 1 {
            return Ok(AnomalyResult::normal(0.0));
        }

        // Calculate mean vector
        let mean = self.mean_vector();

        // Calculate covariance matrix (simplified - just diagonal for efficiency)
        let variances = self.variance_vector(&mean);

        // Calculate Mahalanobis distance (simplified)
        let mut distance_sq = 0.0;
        for i in 0..self.dimensions {
            let diff = values[i] - mean[i];
            let var = variances[i].max(0.0001); // Avoid division by zero
            distance_sq += (diff * diff) / var;
        }

        let distance = distance_sq.sqrt();

        if distance > self.threshold {
            let severity = (distance / self.threshold - 1.0).min(1.0);
            Ok(AnomalyResult::anomaly(distance, severity))
        } else {
            Ok(AnomalyResult::normal(distance))
        }
    }

    /// Calculate mean vector
    fn mean_vector(&self) -> Vec<f64> {
        let n = self.window.len() as f64;
        let mut means = vec![0.0; self.dimensions];

        for obs in &self.window {
            for i in 0..self.dimensions {
                means[i] += obs[i];
            }
        }

        for mean in &mut means {
            *mean /= n;
        }

        means
    }

    /// Calculate variance vector
    fn variance_vector(&self, mean: &[f64]) -> Vec<f64> {
        let n = self.window.len() as f64;
        let mut variances = vec![0.0; self.dimensions];

        for obs in &self.window {
            for i in 0..self.dimensions {
                let diff = obs[i] - mean[i];
                variances[i] += diff * diff;
            }
        }

        for var in &mut variances {
            *var /= n;
        }

        variances
    }

    /// Reset detector
    pub fn reset(&mut self) {
        self.window.clear();
    }
}

/// Calculate percentile of sorted data
fn percentile(sorted_data: &[f64], p: f64) -> f64 {
    if sorted_data.is_empty() {
        return 0.0;
    }

    let n = sorted_data.len();
    if n == 1 {
        return sorted_data[0];
    }

    let index = (p / 100.0) * (n - 1) as f64;
    let lower = index.floor() as usize;
    let upper = index.ceil() as usize;

    if lower == upper {
        sorted_data[lower]
    } else {
        let fraction = index - lower as f64;
        sorted_data[lower] * (1.0 - fraction) + sorted_data[upper] * fraction
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zscore_creation() {
        let detector = ZScoreDetector::new(30, 3.0).unwrap();
        assert!(detector.mean().is_none());
    }

    #[test]
    fn test_zscore_invalid_params() {
        assert!(ZScoreDetector::new(1, 3.0).is_err());
        assert!(ZScoreDetector::new(30, 0.0).is_err());
        assert!(ZScoreDetector::new(30, -1.0).is_err());
    }

    #[test]
    fn test_zscore_normal_data() {
        let mut detector = ZScoreDetector::new(20, 3.0).unwrap();

        for i in 0..30 {
            let result = detector.add(1.0 + ((i % 5) as f64) * 0.01);
            if i > 5 {
                assert!(!result.is_anomaly);
            }
        }
    }

    #[test]
    fn test_zscore_anomaly_detection() {
        let mut detector = ZScoreDetector::new(20, 3.0).unwrap();

        // Add normal data
        for _ in 0..20 {
            detector.add(1.0);
        }

        // Add anomaly
        let result = detector.add(10.0);
        assert!(result.is_anomaly);
        assert!(result.score > 3.0);
        assert!(result.severity > 0.0);
    }

    #[test]
    fn test_zscore_statistics() {
        let mut detector = ZScoreDetector::new(10, 3.0).unwrap();

        for i in 1..=10 {
            detector.add(i as f64);
        }

        let mean = detector.mean().unwrap();
        assert!((mean - 5.5).abs() < 0.1);

        let std_dev = detector.std_dev().unwrap();
        assert!(std_dev > 0.0);
    }

    #[test]
    fn test_iqr_creation() {
        let detector = IQRDetector::new(30, 1.5).unwrap();
        assert_eq!(detector.window_size, 30);
    }

    #[test]
    fn test_iqr_invalid_params() {
        assert!(IQRDetector::new(3, 1.5).is_err());
        assert!(IQRDetector::new(30, 0.0).is_err());
    }

    #[test]
    fn test_iqr_normal_data() {
        let mut detector = IQRDetector::new(20, 1.5).unwrap();

        for i in 1..=30 {
            let result = detector.add(i as f64);
            if i > 10 {
                assert!(!result.is_anomaly);
            }
        }
    }

    #[test]
    fn test_iqr_anomaly_detection() {
        let mut detector = IQRDetector::new(20, 1.5).unwrap();

        // Add normal data (1-20)
        for i in 1..=20 {
            detector.add(i as f64);
        }

        // Add outlier
        let result = detector.add(100.0);
        assert!(result.is_anomaly);
        assert!(result.severity > 0.0);
    }

    #[test]
    fn test_mad_creation() {
        let detector = MADDetector::new(30, 3.5).unwrap();
        assert_eq!(detector.window_size, 30);
    }

    #[test]
    fn test_mad_invalid_params() {
        assert!(MADDetector::new(2, 3.5).is_err());
        assert!(MADDetector::new(30, -1.0).is_err());
    }

    #[test]
    fn test_mad_normal_data() {
        let mut detector = MADDetector::new(20, 3.5).unwrap();

        for _ in 0..30 {
            let result = detector.add(1.0);
            assert!(!result.is_anomaly);
        }
    }

    #[test]
    fn test_mad_anomaly_detection() {
        let mut detector = MADDetector::new(20, 3.5).unwrap();

        // Add normal data
        for i in 1..=20 {
            detector.add(i as f64);
        }

        // Add anomaly
        let result = detector.add(100.0);
        assert!(result.is_anomaly);
        assert!(result.score > 3.5);
    }

    #[test]
    fn test_mahalanobis_creation() {
        let detector = MahalanobisDetector::new(30, 3, 5.0).unwrap();
        assert_eq!(detector.dimensions, 3);
    }

    #[test]
    fn test_mahalanobis_invalid_params() {
        assert!(MahalanobisDetector::new(3, 5, 5.0).is_err());
        assert!(MahalanobisDetector::new(30, 0, 5.0).is_err());
    }

    #[test]
    fn test_mahalanobis_normal_data() {
        let mut detector = MahalanobisDetector::new(20, 3, 5.0).unwrap();

        for _ in 0..25 {
            let result = detector.add(&[1.0, 2.0, 3.0]).unwrap();
            if detector.window.len() > 10 {
                assert!(!result.is_anomaly);
            }
        }
    }

    #[test]
    fn test_mahalanobis_anomaly_detection() {
        let mut detector = MahalanobisDetector::new(20, 3, 5.0).unwrap();

        // Add normal data
        for _ in 0..20 {
            detector.add(&[1.0, 2.0, 3.0]).unwrap();
        }

        // Add anomaly
        let result = detector.add(&[100.0, 200.0, 300.0]).unwrap();
        assert!(result.is_anomaly);
        assert!(result.severity > 0.0);
    }

    #[test]
    fn test_mahalanobis_dimension_mismatch() {
        let mut detector = MahalanobisDetector::new(20, 3, 5.0).unwrap();
        assert!(detector.add(&[1.0, 2.0]).is_err());
        assert!(detector.add(&[1.0, 2.0, 3.0, 4.0]).is_err());
    }

    #[test]
    fn test_percentile() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(percentile(&data, 0.0), 1.0);
        assert_eq!(percentile(&data, 50.0), 3.0);
        assert_eq!(percentile(&data, 100.0), 5.0);
    }

    #[test]
    fn test_percentile_interpolation() {
        let data = vec![1.0, 2.0, 3.0, 4.0];
        let p25 = percentile(&data, 25.0);
        assert!(p25 > 1.0 && p25 < 2.0);
    }

    #[test]
    fn test_anomaly_result_creation() {
        let normal = AnomalyResult::normal(0.5);
        assert!(!normal.is_anomaly);
        assert_eq!(normal.score, 0.5);
        assert_eq!(normal.severity, 0.0);

        let anomaly = AnomalyResult::anomaly(5.0, 0.8);
        assert!(anomaly.is_anomaly);
        assert_eq!(anomaly.score, 5.0);
        assert_eq!(anomaly.severity, 0.8);
    }

    #[test]
    fn test_detector_reset() {
        let mut zscore = ZScoreDetector::new(20, 3.0).unwrap();
        for _ in 0..10 {
            zscore.add(1.0);
        }
        assert!(zscore.mean().is_some());
        zscore.reset();
        assert!(zscore.mean().is_none());

        let mut iqr = IQRDetector::new(20, 1.5).unwrap();
        for i in 1..=10 {
            iqr.add(i as f64);
        }
        iqr.reset();
        assert_eq!(iqr.window.len(), 0);

        let mut mad = MADDetector::new(20, 3.5).unwrap();
        for _ in 0..10 {
            mad.add(1.0);
        }
        mad.reset();
        assert_eq!(mad.window.len(), 0);
    }
}
