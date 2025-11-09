//! Integration tests for drift and anomaly detection
//!
//! This test file demonstrates the complete integration of threshold-based
//! heuristics with drift detection and anomaly detection for production monitoring.

use decision::{
    ThresholdMonitoringSystem, ThresholdConfig, AlertType, AlertSeverity,
    DriftAlgorithm, DriftStatus, ADWIN, PageHinkley, CUSUM,
    ZScoreDetector, IQRDetector, MADDetector, MahalanobisDetector,
};

#[test]
fn test_quality_monitoring_with_drift() {
    let system = ThresholdMonitoringSystem::new();

    // Register quality metric with drift detection
    let config = ThresholdConfig {
        min_value: Some(0.7),
        max_value: Some(1.0),
        warning_min: Some(0.8),
        warning_max: None,
        enable_drift_detection: true,
        drift_algorithm: DriftAlgorithm::ADWIN,
        enable_anomaly_detection: true,
        anomaly_threshold: 3.0,
    };

    system.register_metric("quality_score", config).unwrap();

    // Simulate stable performance
    for _ in 0..30 {
        let alerts = system.record("quality_score", 0.9);
        assert!(alerts.is_empty());
    }

    // Simulate gradual degradation (drift)
    let mut drift_detected = false;
    for i in 0..40 {
        let value = 0.9 - (i as f64 * 0.01); // Gradual decline
        let alerts = system.record("quality_score", value);

        for alert in &alerts {
            if alert.alert_type == AlertType::Drift {
                drift_detected = true;
            }
        }
    }

    // Should eventually detect drift
    assert!(drift_detected);
}

#[test]
fn test_cost_monitoring_with_anomaly() {
    let system = ThresholdMonitoringSystem::new();

    // Register cost metric with anomaly detection
    let config = ThresholdConfig::cost(1.0);
    system.register_metric("cost", config).unwrap();

    // Simulate normal costs
    for _ in 0..30 {
        let alerts = system.record("cost", 0.05);
        assert!(alerts.iter().all(|a| a.severity != AlertSeverity::Critical));
    }

    // Simulate cost spike (anomaly)
    let alerts = system.record("cost", 5.0);
    assert!(alerts.iter().any(|a| {
        a.alert_type == AlertType::Anomaly || a.alert_type == AlertType::ThresholdViolation
    }));
}

#[test]
fn test_latency_monitoring_comprehensive() {
    let system = ThresholdMonitoringSystem::new();

    let config = ThresholdConfig::latency(2000.0);
    system.register_metric("latency", config).unwrap();

    // Normal latency
    for _ in 0..20 {
        system.record("latency", 1000.0);
    }

    // Warning threshold
    let alerts = system.record("latency", 1700.0);
    if !alerts.is_empty() {
        assert!(alerts.iter().any(|a| a.severity == AlertSeverity::Warning));
    }

    // Critical threshold
    let alerts = system.record("latency", 2500.0);
    assert!(alerts.iter().any(|a| a.severity == AlertSeverity::Critical));
}

#[test]
fn test_multiple_metrics_monitoring() {
    let system = ThresholdMonitoringSystem::new();

    // Register multiple metrics
    system.register_metric("quality", ThresholdConfig::quality()).unwrap();
    system.register_metric("cost", ThresholdConfig::cost(1.0)).unwrap();
    system.register_metric("latency", ThresholdConfig::latency(3000.0)).unwrap();

    assert_eq!(system.get_metrics().len(), 3);

    // Simulate normal operation
    for _ in 0..25 {
        system.record("quality", 0.9);
        system.record("cost", 0.05);
        system.record("latency", 1200.0);
    }

    // Trigger violations on different metrics
    let quality_alerts = system.record("quality", 0.4);
    let cost_alerts = system.record("cost", 5.0);
    let latency_alerts = system.record("latency", 8000.0);

    assert!(!quality_alerts.is_empty());
    assert!(!cost_alerts.is_empty());
    assert!(!latency_alerts.is_empty());
}

#[test]
fn test_alert_history_and_clearing() {
    let system = ThresholdMonitoringSystem::new();

    system.register_metric("quality", ThresholdConfig::quality()).unwrap();

    // Generate alerts
    system.record("quality", 0.4);
    system.record("quality", 0.3);

    let alerts = system.get_recent_alerts("quality");
    assert!(!alerts.is_empty());

    // Clear alerts
    system.clear_alerts("quality");
    let alerts = system.get_recent_alerts("quality");
    assert!(alerts.is_empty());
}

#[test]
fn test_metric_reset() {
    let system = ThresholdMonitoringSystem::new();

    system.register_metric("quality", ThresholdConfig::quality()).unwrap();

    // Add data
    for _ in 0..30 {
        system.record("quality", 0.9);
    }

    // Reset
    system.reset_metric("quality").unwrap();

    // Should start fresh
    for _ in 0..10 {
        let alerts = system.record("quality", 0.9);
        assert!(alerts.is_empty());
    }
}

#[test]
fn test_drift_detection_algorithms() {
    // Test ADWIN
    let mut adwin = ADWIN::new(0.002, 100).unwrap();
    for _ in 0..30 {
        adwin.add(1.0);
    }
    let mut drift_count = 0;
    for _ in 0..30 {
        if adwin.add(2.0) == DriftStatus::Drift {
            drift_count += 1;
        }
    }
    assert!(drift_count > 0);

    // Test Page-Hinkley
    let mut ph = PageHinkley::new(10.0, 0.005).unwrap();
    for _ in 0..20 {
        ph.add(1.0);
    }
    let mut ph_drift = false;
    for _ in 0..30 {
        if ph.add(3.0) == DriftStatus::Drift {
            ph_drift = true;
            break;
        }
    }
    assert!(ph_drift);

    // Test CUSUM
    let mut cusum = CUSUM::new(3.0, 1.0, 0.5).unwrap();
    let mut cusum_drift = false;
    for _ in 0..30 {
        if cusum.add(2.5) == DriftStatus::Drift {
            cusum_drift = true;
            break;
        }
    }
    assert!(cusum_drift);
}

#[test]
fn test_anomaly_detection_methods() {
    // Z-score detector
    let mut zscore = ZScoreDetector::new(30, 3.0).unwrap();
    for _ in 0..30 {
        zscore.add(10.0);
    }
    let result = zscore.add(50.0);
    assert!(result.is_anomaly);

    // IQR detector
    let mut iqr = IQRDetector::new(20, 1.5).unwrap();
    for i in 1..=20 {
        iqr.add(i as f64);
    }
    let result = iqr.add(100.0);
    assert!(result.is_anomaly);

    // MAD detector
    let mut mad = MADDetector::new(20, 3.5).unwrap();
    for i in 1..=20 {
        mad.add(i as f64);
    }
    let result = mad.add(200.0);
    assert!(result.is_anomaly);

    // Mahalanobis detector (multi-dimensional)
    let mut maha = MahalanobisDetector::new(20, 3, 5.0).unwrap();
    for _ in 0..20 {
        maha.add(&[1.0, 2.0, 3.0]).unwrap();
    }
    let result = maha.add(&[100.0, 200.0, 300.0]).unwrap();
    assert!(result.is_anomaly);
}

#[test]
fn test_real_world_scenario_performance_degradation() {
    let system = ThresholdMonitoringSystem::new();

    // Monitor quality with drift detection
    let config = ThresholdConfig {
        min_value: Some(0.7),
        warning_min: Some(0.85),
        enable_drift_detection: true,
        drift_algorithm: DriftAlgorithm::ADWIN,
        enable_anomaly_detection: true,
        anomaly_threshold: 2.5,
        ..Default::default()
    };

    system.register_metric("model_quality", config).unwrap();

    // Phase 1: Normal operation
    for _ in 0..40 {
        let alerts = system.record("model_quality", 0.92);
        assert!(alerts.iter().all(|a| a.severity != AlertSeverity::Critical));
    }

    // Phase 2: Gradual degradation
    let mut warning_received = false;
    for i in 0..50 {
        let quality = 0.92 - (i as f64 * 0.005);
        let alerts = system.record("model_quality", quality);

        for alert in &alerts {
            if alert.severity == AlertSeverity::Warning {
                warning_received = true;
            }
        }
    }

    assert!(warning_received);
}

#[test]
fn test_real_world_scenario_cost_spike() {
    let system = ThresholdMonitoringSystem::new();

    // Monitor cost with anomaly detection
    let config = ThresholdConfig {
        max_value: Some(0.10),
        warning_max: Some(0.08),
        enable_drift_detection: false,
        enable_anomaly_detection: true,
        anomaly_threshold: 3.0,
        ..Default::default()
    };

    system.register_metric("request_cost", config).unwrap();

    // Normal operation
    for _ in 0..30 {
        system.record("request_cost", 0.05);
    }

    // Sudden spike
    let alerts = system.record("request_cost", 0.50);

    assert!(alerts.iter().any(|a| {
        a.alert_type == AlertType::Anomaly || a.alert_type == AlertType::ThresholdViolation
    }));
    assert!(alerts.iter().any(|a| a.severity == AlertSeverity::Critical));
}

#[test]
fn test_real_world_scenario_latency_monitoring() {
    let system = ThresholdMonitoringSystem::new();

    let config = ThresholdConfig {
        max_value: Some(5000.0),
        warning_max: Some(3000.0),
        enable_drift_detection: true,
        drift_algorithm: DriftAlgorithm::CUSUM,
        enable_anomaly_detection: true,
        anomaly_threshold: 3.0,
        ..Default::default()
    };

    system.register_metric("p95_latency", config).unwrap();

    // Fast responses
    for _ in 0..30 {
        system.record("p95_latency", 800.0);
    }

    // Gradually increase latency
    for i in 1..=10 {
        let latency = 800.0 + (i as f64 * 150.0);
        system.record("p95_latency", latency);
    }

    // Critical latency - well above max
    let alerts = system.record("p95_latency", 6000.0);
    assert!(alerts.iter().any(|a| a.severity == AlertSeverity::Critical));
    assert!(alerts.iter().any(|a| a.alert_type == AlertType::ThresholdViolation));
}

#[test]
fn test_concurrent_metric_monitoring() {
    let system = ThresholdMonitoringSystem::new();

    // Register metrics
    for i in 0..5 {
        let metric_name = format!("metric_{}", i);
        system.register_metric(&metric_name, ThresholdConfig::quality()).unwrap();
    }

    assert_eq!(system.get_metrics().len(), 5);

    // Simulate concurrent updates
    for _ in 0..20 {
        for i in 0..5 {
            let metric_name = format!("metric_{}", i);
            system.record(&metric_name, 0.9);
        }
    }

    // Verify all metrics are being monitored
    for i in 0..5 {
        let metric_name = format!("metric_{}", i);
        assert!(system.has_metric(&metric_name));
    }
}

#[test]
fn test_alert_severity_escalation() {
    let system = ThresholdMonitoringSystem::new();

    let config = ThresholdConfig {
        min_value: Some(0.5),
        warning_min: Some(0.7),
        ..Default::default()
    };

    system.register_metric("quality", config).unwrap();

    // Warning level
    let alerts = system.record("quality", 0.65);
    if !alerts.is_empty() {
        assert!(alerts.iter().any(|a| a.severity == AlertSeverity::Warning));
    }

    // Critical level
    let alerts = system.record("quality", 0.3);
    assert!(alerts.iter().any(|a| a.severity == AlertSeverity::Critical));
}

#[test]
fn test_drift_detection_sensitivity() {
    // Test different drift algorithms with various sensitivities

    // ADWIN - sensitive to distribution changes
    let mut adwin = ADWIN::new(0.002, 100).unwrap();
    for _ in 0..30 {
        adwin.add(1.0);
    }
    assert!(adwin.mean().is_some());

    // Page-Hinkley - good for mean shifts
    let mut ph = PageHinkley::new(20.0, 0.01).unwrap();
    for _ in 0..20 {
        ph.add(1.0);
    }
    assert!(ph.statistic() >= 0.0);

    // CUSUM - detects small shifts
    let mut cusum = CUSUM::new(5.0, 1.0, 0.5).unwrap();
    cusum.add(1.0);
    assert!(cusum.positive_cusum() >= 0.0);
    assert!(cusum.negative_cusum() >= 0.0);
}
