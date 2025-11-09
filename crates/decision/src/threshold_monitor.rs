//! Threshold-Based Monitoring
//!
//! This module provides threshold-based monitoring and alerting for metrics,
//! integrating drift detection and anomaly detection.

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    anomaly_detection::{AnomalyResult, MADDetector, ZScoreDetector},
    drift_detection::{DriftAlgorithm, DriftStatus, PageHinkley, ADWIN, CUSUM},
    errors::{DecisionError, Result},
};

/// Alert severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AlertSeverity {
    /// Informational
    Info,
    /// Warning
    Warning,
    /// Critical
    Critical,
}

/// Alert type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertType {
    /// Threshold violation
    ThresholdViolation,
    /// Drift detected
    Drift,
    /// Anomaly detected
    Anomaly,
    /// Performance degradation
    PerformanceDegradation,
}

/// Alert information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    /// Alert ID
    pub id: Uuid,
    /// Metric name
    pub metric_name: String,
    /// Alert type
    pub alert_type: AlertType,
    /// Severity
    pub severity: AlertSeverity,
    /// Message
    pub message: String,
    /// Metric value
    pub value: f64,
    /// Threshold
    pub threshold: Option<f64>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Additional context
    pub context: String,
}

impl Alert {
    /// Create new alert
    pub fn new(
        metric_name: impl Into<String>,
        alert_type: AlertType,
        severity: AlertSeverity,
        message: impl Into<String>,
        value: f64,
        threshold: Option<f64>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            metric_name: metric_name.into(),
            alert_type,
            severity,
            message: message.into(),
            value,
            threshold,
            timestamp: Utc::now(),
            context: String::new(),
        }
    }

    /// Set context
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = context.into();
        self
    }
}

/// Metric threshold configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdConfig {
    /// Minimum acceptable value
    pub min_value: Option<f64>,
    /// Maximum acceptable value
    pub max_value: Option<f64>,
    /// Warning threshold (lower)
    pub warning_min: Option<f64>,
    /// Warning threshold (upper)
    pub warning_max: Option<f64>,
    /// Enable drift detection
    pub enable_drift_detection: bool,
    /// Drift detection algorithm
    pub drift_algorithm: DriftAlgorithm,
    /// Enable anomaly detection
    pub enable_anomaly_detection: bool,
    /// Anomaly threshold (z-score)
    pub anomaly_threshold: f64,
}

impl Default for ThresholdConfig {
    fn default() -> Self {
        Self {
            min_value: None,
            max_value: None,
            warning_min: None,
            warning_max: None,
            enable_drift_detection: true,
            drift_algorithm: DriftAlgorithm::ADWIN,
            enable_anomaly_detection: true,
            anomaly_threshold: 3.0,
        }
    }
}

impl ThresholdConfig {
    /// Create config for quality metrics (0.0 - 1.0)
    pub fn quality() -> Self {
        Self {
            min_value: Some(0.5),
            max_value: Some(1.0),
            warning_min: Some(0.7),
            warning_max: None,
            enable_drift_detection: true,
            drift_algorithm: DriftAlgorithm::ADWIN,
            enable_anomaly_detection: true,
            anomaly_threshold: 2.5,
        }
    }

    /// Create config for cost metrics
    pub fn cost(max_cost: f64) -> Self {
        Self {
            min_value: Some(0.0),
            max_value: Some(max_cost),
            warning_min: None,
            warning_max: Some(max_cost * 0.8),
            enable_drift_detection: true,
            drift_algorithm: DriftAlgorithm::PageHinkley,
            enable_anomaly_detection: true,
            anomaly_threshold: 3.0,
        }
    }

    /// Create config for latency metrics (milliseconds)
    pub fn latency(max_latency: f64) -> Self {
        Self {
            min_value: Some(0.0),
            max_value: Some(max_latency),
            warning_min: None,
            warning_max: Some(max_latency * 0.8),
            enable_drift_detection: true,
            drift_algorithm: DriftAlgorithm::CUSUM,
            enable_anomaly_detection: true,
            anomaly_threshold: 2.5,
        }
    }
}

/// Metric monitor state
struct MetricMonitor {
    /// Configuration
    config: ThresholdConfig,
    /// Drift detector (ADWIN)
    adwin: Option<ADWIN>,
    /// Drift detector (Page-Hinkley)
    page_hinkley: Option<PageHinkley>,
    /// Drift detector (CUSUM)
    cusum: Option<CUSUM>,
    /// Anomaly detector (Z-score)
    zscore: Option<ZScoreDetector>,
    /// Anomaly detector (MAD)
    mad: Option<MADDetector>,
    /// Recent alerts
    recent_alerts: Vec<Alert>,
    /// Sample count
    sample_count: u64,
}

impl MetricMonitor {
    /// Create new metric monitor
    fn new(config: ThresholdConfig) -> Result<Self> {
        let adwin = if config.enable_drift_detection
            && config.drift_algorithm == DriftAlgorithm::ADWIN
        {
            Some(ADWIN::new(0.002, 100)?)
        } else {
            None
        };

        let page_hinkley = if config.enable_drift_detection
            && config.drift_algorithm == DriftAlgorithm::PageHinkley
        {
            Some(PageHinkley::new(10.0, 0.005)?)
        } else {
            None
        };

        let cusum = if config.enable_drift_detection
            && config.drift_algorithm == DriftAlgorithm::CUSUM
        {
            // Need to estimate target mean - use default for now
            Some(CUSUM::new(5.0, 0.5, 0.1)?)
        } else {
            None
        };

        let zscore = if config.enable_anomaly_detection {
            Some(ZScoreDetector::new(50, config.anomaly_threshold)?)
        } else {
            None
        };

        let mad = if config.enable_anomaly_detection {
            Some(MADDetector::new(50, config.anomaly_threshold)?)
        } else {
            None
        };

        Ok(Self {
            config,
            adwin,
            page_hinkley,
            cusum,
            zscore,
            mad,
            recent_alerts: Vec::new(),
            sample_count: 0,
        })
    }

    /// Check value and generate alerts
    fn check_value(&mut self, metric_name: &str, value: f64) -> Vec<Alert> {
        let mut alerts = Vec::new();
        self.sample_count += 1;

        // Check hard thresholds
        if let Some(min) = self.config.min_value {
            if value < min {
                alerts.push(
                    Alert::new(
                        metric_name,
                        AlertType::ThresholdViolation,
                        AlertSeverity::Critical,
                        format!("Value {} below minimum threshold {}", value, min),
                        value,
                        Some(min),
                    )
                    .with_context(format!("Sample count: {}", self.sample_count)),
                );
            }
        }

        if let Some(max) = self.config.max_value {
            if value > max {
                alerts.push(
                    Alert::new(
                        metric_name,
                        AlertType::ThresholdViolation,
                        AlertSeverity::Critical,
                        format!("Value {} exceeds maximum threshold {}", value, max),
                        value,
                        Some(max),
                    )
                    .with_context(format!("Sample count: {}", self.sample_count)),
                );
            }
        }

        // Check warning thresholds
        if let Some(warning_min) = self.config.warning_min {
            if value < warning_min && !alerts.iter().any(|a| matches!(a.alert_type, AlertType::ThresholdViolation)) {
                alerts.push(
                    Alert::new(
                        metric_name,
                        AlertType::ThresholdViolation,
                        AlertSeverity::Warning,
                        format!("Value {} below warning threshold {}", value, warning_min),
                        value,
                        Some(warning_min),
                    )
                    .with_context(format!("Sample count: {}", self.sample_count)),
                );
            }
        }

        if let Some(warning_max) = self.config.warning_max {
            if value > warning_max && !alerts.iter().any(|a| matches!(a.alert_type, AlertType::ThresholdViolation)) {
                alerts.push(
                    Alert::new(
                        metric_name,
                        AlertType::ThresholdViolation,
                        AlertSeverity::Warning,
                        format!("Value {} exceeds warning threshold {}", value, warning_max),
                        value,
                        Some(warning_max),
                    )
                    .with_context(format!("Sample count: {}", self.sample_count)),
                );
            }
        }

        // Check drift
        if self.config.enable_drift_detection {
            let drift_status = if let Some(adwin) = &mut self.adwin {
                adwin.add(value)
            } else if let Some(ph) = &mut self.page_hinkley {
                ph.add(value)
            } else if let Some(cusum) = &mut self.cusum {
                cusum.add(value)
            } else {
                DriftStatus::Stable
            };

            match drift_status {
                DriftStatus::Drift => {
                    alerts.push(
                        Alert::new(
                            metric_name,
                            AlertType::Drift,
                            AlertSeverity::Critical,
                            format!(
                                "Drift detected using {:?}",
                                self.config.drift_algorithm
                            ),
                            value,
                            None,
                        )
                        .with_context(format!("Sample count: {}", self.sample_count)),
                    );
                }
                DriftStatus::Warning => {
                    alerts.push(
                        Alert::new(
                            metric_name,
                            AlertType::Drift,
                            AlertSeverity::Warning,
                            "Possible drift detected".to_string(),
                            value,
                            None,
                        )
                        .with_context(format!("Sample count: {}", self.sample_count)),
                    );
                }
                DriftStatus::Stable => {}
            }
        }

        // Check anomalies
        if self.config.enable_anomaly_detection {
            let anomaly_result = if let Some(zscore) = &mut self.zscore {
                zscore.add(value)
            } else if let Some(mad) = &mut self.mad {
                mad.add(value)
            } else {
                AnomalyResult::normal(0.0)
            };

            if anomaly_result.is_anomaly {
                let severity = if anomaly_result.severity > 0.7 {
                    AlertSeverity::Critical
                } else {
                    AlertSeverity::Warning
                };

                alerts.push(
                    Alert::new(
                        metric_name,
                        AlertType::Anomaly,
                        severity,
                        format!(
                            "Anomaly detected (score: {:.2}, severity: {:.2})",
                            anomaly_result.score, anomaly_result.severity
                        ),
                        value,
                        Some(self.config.anomaly_threshold),
                    )
                    .with_context(format!("Sample count: {}", self.sample_count)),
                );
            }
        }

        // Store recent alerts
        for alert in &alerts {
            self.recent_alerts.push(alert.clone());
        }

        // Keep only last 100 alerts
        if self.recent_alerts.len() > 100 {
            self.recent_alerts.drain(0..self.recent_alerts.len() - 100);
        }

        alerts
    }
}

/// Threshold monitoring system
pub struct ThresholdMonitoringSystem {
    /// Monitors per metric
    monitors: Arc<DashMap<String, MetricMonitor>>,
    /// Alert handlers
    alert_handlers: Arc<DashMap<String, Box<dyn Fn(&Alert) + Send + Sync>>>,
}

impl ThresholdMonitoringSystem {
    /// Create new monitoring system
    pub fn new() -> Self {
        Self {
            monitors: Arc::new(DashMap::new()),
            alert_handlers: Arc::new(DashMap::new()),
        }
    }

    /// Register metric with configuration
    pub fn register_metric(&self, name: impl Into<String>, config: ThresholdConfig) -> Result<()> {
        let name = name.into();
        let monitor = MetricMonitor::new(config)?;
        self.monitors.insert(name, monitor);
        Ok(())
    }

    /// Record metric value and check for alerts
    pub fn record(&self, metric_name: &str, value: f64) -> Vec<Alert> {
        if let Some(mut monitor) = self.monitors.get_mut(metric_name) {
            let alerts = monitor.check_value(metric_name, value);

            // Trigger alert handlers
            for alert in &alerts {
                if let Some(handler) = self.alert_handlers.get(metric_name) {
                    handler(alert);
                }
            }

            alerts
        } else {
            Vec::new()
        }
    }

    /// Get recent alerts for a metric
    pub fn get_recent_alerts(&self, metric_name: &str) -> Vec<Alert> {
        self.monitors
            .get(metric_name)
            .map(|m| m.recent_alerts.clone())
            .unwrap_or_default()
    }

    /// Clear alerts for a metric
    pub fn clear_alerts(&self, metric_name: &str) {
        if let Some(mut monitor) = self.monitors.get_mut(metric_name) {
            monitor.recent_alerts.clear();
        }
    }

    /// Reset monitoring for a metric
    pub fn reset_metric(&self, metric_name: &str) -> Result<()> {
        if let Some(mut entry) = self.monitors.get_mut(metric_name) {
            let config = entry.config.clone();
            let new_monitor = MetricMonitor::new(config)?;
            *entry = new_monitor;
            Ok(())
        } else {
            Err(DecisionError::InvalidParameter(format!(
                "Metric {} not found",
                metric_name
            )))
        }
    }

    /// Get all monitored metrics
    pub fn get_metrics(&self) -> Vec<String> {
        self.monitors.iter().map(|e| e.key().clone()).collect()
    }

    /// Check if metric is registered
    pub fn has_metric(&self, metric_name: &str) -> bool {
        self.monitors.contains_key(metric_name)
    }
}

impl Default for ThresholdMonitoringSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_creation() {
        let alert = Alert::new(
            "quality",
            AlertType::ThresholdViolation,
            AlertSeverity::Warning,
            "Low quality",
            0.6,
            Some(0.7),
        );

        assert_eq!(alert.metric_name, "quality");
        assert_eq!(alert.alert_type, AlertType::ThresholdViolation);
        assert_eq!(alert.severity, AlertSeverity::Warning);
        assert_eq!(alert.value, 0.6);
    }

    #[test]
    fn test_threshold_config_quality() {
        let config = ThresholdConfig::quality();
        assert_eq!(config.min_value, Some(0.5));
        assert!(config.enable_drift_detection);
        assert!(config.enable_anomaly_detection);
    }

    #[test]
    fn test_threshold_config_cost() {
        let config = ThresholdConfig::cost(1.0);
        assert_eq!(config.max_value, Some(1.0));
        assert_eq!(config.warning_max, Some(0.8));
    }

    #[test]
    fn test_threshold_config_latency() {
        let config = ThresholdConfig::latency(5000.0);
        assert_eq!(config.max_value, Some(5000.0));
        assert_eq!(config.warning_max, Some(4000.0));
        assert_eq!(config.drift_algorithm, DriftAlgorithm::CUSUM);
    }

    #[test]
    fn test_monitoring_system_creation() {
        let system = ThresholdMonitoringSystem::new();
        assert_eq!(system.get_metrics().len(), 0);
    }

    #[test]
    fn test_register_metric() {
        let system = ThresholdMonitoringSystem::new();
        let config = ThresholdConfig::quality();

        system.register_metric("quality", config).unwrap();
        assert!(system.has_metric("quality"));
        assert_eq!(system.get_metrics().len(), 1);
    }

    #[test]
    fn test_threshold_violation() {
        let system = ThresholdMonitoringSystem::new();
        let config = ThresholdConfig {
            min_value: Some(0.7),
            max_value: Some(1.0),
            ..Default::default()
        };

        system.register_metric("quality", config).unwrap();

        // Below minimum
        let alerts = system.record("quality", 0.5);
        assert!(!alerts.is_empty());
        assert!(alerts.iter().any(|a| a.severity == AlertSeverity::Critical));

        // Clear and test above maximum
        system.clear_alerts("quality");
        let alerts = system.record("quality", 1.5);
        assert!(!alerts.is_empty());
    }

    #[test]
    fn test_warning_threshold() {
        let system = ThresholdMonitoringSystem::new();
        let config = ThresholdConfig {
            min_value: Some(0.5),
            warning_min: Some(0.7),
            ..Default::default()
        };

        system.register_metric("quality", config).unwrap();

        // In warning zone
        let alerts = system.record("quality", 0.6);
        assert!(!alerts.is_empty());
        assert!(alerts.iter().any(|a| a.severity == AlertSeverity::Warning));
    }

    #[test]
    fn test_drift_detection() {
        let system = ThresholdMonitoringSystem::new();
        let config = ThresholdConfig {
            enable_drift_detection: true,
            drift_algorithm: DriftAlgorithm::ADWIN,
            enable_anomaly_detection: false,
            ..Default::default()
        };

        system.register_metric("quality", config).unwrap();

        // Add stable data
        for _ in 0..30 {
            system.record("quality", 0.9);
        }

        // Add drifted data
        let mut drift_detected = false;
        for _ in 0..30 {
            let alerts = system.record("quality", 0.5);
            if alerts.iter().any(|a| a.alert_type == AlertType::Drift) {
                drift_detected = true;
                break;
            }
        }

        assert!(drift_detected);
    }

    #[test]
    fn test_anomaly_detection() {
        let system = ThresholdMonitoringSystem::new();
        let config = ThresholdConfig {
            enable_drift_detection: false,
            enable_anomaly_detection: true,
            anomaly_threshold: 3.0,
            ..Default::default()
        };

        system.register_metric("latency", config).unwrap();

        // Add normal data
        for _ in 0..30 {
            system.record("latency", 1000.0);
        }

        // Add anomaly
        let alerts = system.record("latency", 5000.0);
        assert!(alerts.iter().any(|a| a.alert_type == AlertType::Anomaly));
    }

    #[test]
    fn test_recent_alerts() {
        let system = ThresholdMonitoringSystem::new();
        let config = ThresholdConfig {
            min_value: Some(0.7),
            ..Default::default()
        };

        system.register_metric("quality", config).unwrap();

        // Generate alerts
        system.record("quality", 0.5);
        system.record("quality", 0.4);

        let recent = system.get_recent_alerts("quality");
        assert!(!recent.is_empty());
    }

    #[test]
    fn test_clear_alerts() {
        let system = ThresholdMonitoringSystem::new();
        let config = ThresholdConfig {
            min_value: Some(0.7),
            ..Default::default()
        };

        system.register_metric("quality", config).unwrap();

        system.record("quality", 0.5);
        assert!(!system.get_recent_alerts("quality").is_empty());

        system.clear_alerts("quality");
        assert!(system.get_recent_alerts("quality").is_empty());
    }

    #[test]
    fn test_reset_metric() {
        let system = ThresholdMonitoringSystem::new();
        let config = ThresholdConfig::quality();

        system.register_metric("quality", config).unwrap();

        // Add data
        for _ in 0..20 {
            system.record("quality", 0.9);
        }

        // Reset
        system.reset_metric("quality").unwrap();

        // Alerts should be cleared
        assert!(system.get_recent_alerts("quality").is_empty());
    }

    #[test]
    fn test_unregistered_metric() {
        let system = ThresholdMonitoringSystem::new();
        let alerts = system.record("unknown", 1.0);
        assert!(alerts.is_empty());
    }

    #[test]
    fn test_alert_severity_ordering() {
        assert!(AlertSeverity::Critical > AlertSeverity::Warning);
        assert!(AlertSeverity::Warning > AlertSeverity::Info);
    }
}
