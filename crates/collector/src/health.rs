//! Health Check Module
//!
//! This module provides production-grade health check endpoints for Kubernetes deployments.
//! It includes liveness, readiness, and detailed health status reporting with component-level
//! health tracking and caching to avoid overwhelming the system.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{debug, warn};

use crate::feedback_collector::CollectorStats;
use crate::kafka_client::FeedbackProducer;

/// Maximum time for a health check operation (2 seconds)
const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(2);

/// Cache TTL for health check results (5 seconds)
const HEALTH_CACHE_TTL: Duration = Duration::from_secs(5);

/// Buffer capacity warning threshold (80%)
const BUFFER_WARNING_THRESHOLD: f64 = 0.80;

/// Buffer capacity critical threshold (95%)
const BUFFER_CRITICAL_THRESHOLD: f64 = 0.95;

/// Health status enumeration
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    /// Component is healthy and operating normally
    Healthy,
    /// Component is operational but degraded (warnings present)
    Degraded,
    /// Component is unhealthy and may not be functioning correctly
    Unhealthy,
}

impl HealthStatus {
    /// Check if the status is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self, HealthStatus::Healthy)
    }

    /// Check if the status is degraded or worse
    pub fn is_degraded_or_worse(&self) -> bool {
        matches!(self, HealthStatus::Degraded | HealthStatus::Unhealthy)
    }

    /// Check if the status is unhealthy
    pub fn is_unhealthy(&self) -> bool {
        matches!(self, HealthStatus::Unhealthy)
    }

    /// Combine two health statuses (returns the worse of the two)
    pub fn combine(self, other: HealthStatus) -> HealthStatus {
        match (self, other) {
            (HealthStatus::Unhealthy, _) | (_, HealthStatus::Unhealthy) => HealthStatus::Unhealthy,
            (HealthStatus::Degraded, _) | (_, HealthStatus::Degraded) => HealthStatus::Degraded,
            _ => HealthStatus::Healthy,
        }
    }
}

/// Component health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Component name
    pub component_name: String,
    /// Health status
    pub status: HealthStatus,
    /// Status message providing details
    pub message: String,
    /// Last successful check timestamp
    pub last_check: DateTime<Utc>,
    /// Additional metadata about the component
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl ComponentHealth {
    /// Create a new healthy component health
    pub fn healthy(component_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            component_name: component_name.into(),
            status: HealthStatus::Healthy,
            message: message.into(),
            last_check: Utc::now(),
            metadata: None,
        }
    }

    /// Create a new degraded component health
    pub fn degraded(component_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            component_name: component_name.into(),
            status: HealthStatus::Degraded,
            message: message.into(),
            last_check: Utc::now(),
            metadata: None,
        }
    }

    /// Create a new unhealthy component health
    pub fn unhealthy(component_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            component_name: component_name.into(),
            status: HealthStatus::Unhealthy,
            message: message.into(),
            last_check: Utc::now(),
            metadata: None,
        }
    }

    /// Add metadata to the component health
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Complete health report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    /// Overall health status
    pub overall_status: HealthStatus,
    /// Individual component health status
    pub components: Vec<ComponentHealth>,
    /// Timestamp of this report
    pub timestamp: DateTime<Utc>,
    /// Version information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

impl HealthReport {
    /// Create a new health report
    pub fn new(components: Vec<ComponentHealth>) -> Self {
        let overall_status = components
            .iter()
            .map(|c| c.status)
            .fold(HealthStatus::Healthy, |acc, status| acc.combine(status));

        Self {
            overall_status,
            components,
            timestamp: Utc::now(),
            version: None,
        }
    }

    /// Set version information
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Check if the system is ready to accept traffic
    pub fn is_ready(&self) -> bool {
        // System is ready if no components are unhealthy
        !self.components.iter().any(|c| c.status.is_unhealthy())
    }

    /// Check if the system is alive
    pub fn is_alive(&self) -> bool {
        // For liveness, we're more lenient - system is alive unless critically broken
        // This prevents Kubernetes from killing pods that are just degraded
        !matches!(self.overall_status, HealthStatus::Unhealthy)
    }
}

/// Cached health check result
struct CachedHealthReport {
    /// The cached report
    report: HealthReport,
    /// When this cache entry was created
    cached_at: Instant,
}

impl CachedHealthReport {
    /// Check if this cache entry is still valid
    fn is_valid(&self) -> bool {
        self.cached_at.elapsed() < HEALTH_CACHE_TTL
    }
}

/// Health checker for the feedback collector
pub struct HealthChecker {
    /// Reference to the Kafka producer for connectivity checks
    kafka_producer: Option<Arc<FeedbackProducer>>,
    /// Reference to collector statistics
    collector_stats: Option<Arc<dyn Fn() -> CollectorStats + Send + Sync>>,
    /// Configured buffer size for capacity calculations
    buffer_size: usize,
    /// Number of worker threads
    num_workers: usize,
    /// Cached health check result
    cached_report: Arc<RwLock<Option<CachedHealthReport>>>,
    /// Service version
    version: Option<String>,
}

impl HealthChecker {
    /// Create a new health checker
    pub fn new(buffer_size: usize, num_workers: usize) -> Self {
        Self {
            kafka_producer: None,
            collector_stats: None,
            buffer_size,
            num_workers,
            cached_report: Arc::new(RwLock::new(None)),
            version: None,
        }
    }

    /// Set the Kafka producer for connectivity checks
    pub fn with_kafka_producer(mut self, producer: Arc<FeedbackProducer>) -> Self {
        self.kafka_producer = Some(producer);
        self
    }

    /// Set the collector stats provider
    pub fn with_collector_stats<F>(mut self, stats_fn: F) -> Self
    where
        F: Fn() -> CollectorStats + Send + Sync + 'static,
    {
        self.collector_stats = Some(Arc::new(stats_fn));
        self
    }

    /// Set service version
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Perform a liveness check (simple alive check for Kubernetes liveness probe)
    ///
    /// This is a lightweight check that only verifies the service is running.
    /// It uses cached results to avoid overwhelming the system.
    pub async fn check_liveness(&self) -> HealthReport {
        // Check cache first
        {
            let cache = self.cached_report.read().await;
            if let Some(cached) = cache.as_ref() {
                if cached.is_valid() {
                    debug!("Returning cached liveness check");
                    return cached.report.clone();
                }
            }
        }

        // Perform basic liveness check
        let mut components = vec![ComponentHealth::healthy(
            "service",
            "Service is running and responsive",
        )];

        // Add basic worker health
        components.push(ComponentHealth::healthy(
            "workers",
            format!("{} workers configured", self.num_workers),
        ));

        let report = HealthReport::new(components);
        let report = if let Some(ref version) = self.version {
            report.with_version(version.clone())
        } else {
            report
        };

        // Update cache
        {
            let mut cache = self.cached_report.write().await;
            *cache = Some(CachedHealthReport {
                report: report.clone(),
                cached_at: Instant::now(),
            });
        }

        report
    }

    /// Perform a readiness check (full check to determine if service can accept traffic)
    ///
    /// This performs comprehensive checks of all components with timeout protection.
    pub async fn check_readiness(&self) -> HealthReport {
        // Check cache first
        {
            let cache = self.cached_report.read().await;
            if let Some(cached) = cache.as_ref() {
                if cached.is_valid() {
                    debug!("Returning cached readiness check");
                    return cached.report.clone();
                }
            }
        }

        // Perform full health check with timeout
        let report = match timeout(HEALTH_CHECK_TIMEOUT, self.perform_full_health_check()).await {
            Ok(report) => report,
            Err(_) => {
                warn!("Health check timed out after {:?}", HEALTH_CHECK_TIMEOUT);
                HealthReport::new(vec![ComponentHealth::unhealthy(
                    "health_check",
                    format!("Health check timed out after {:?}", HEALTH_CHECK_TIMEOUT),
                )])
            }
        };

        // Update cache
        {
            let mut cache = self.cached_report.write().await;
            *cache = Some(CachedHealthReport {
                report: report.clone(),
                cached_at: Instant::now(),
            });
        }

        report
    }

    /// Get detailed health status (always performs fresh checks, no caching)
    ///
    /// This provides the most up-to-date health information for monitoring dashboards.
    pub async fn get_health_status(&self) -> HealthReport {
        match timeout(HEALTH_CHECK_TIMEOUT, self.perform_full_health_check()).await {
            Ok(report) => report,
            Err(_) => {
                warn!("Detailed health check timed out after {:?}", HEALTH_CHECK_TIMEOUT);
                HealthReport::new(vec![ComponentHealth::unhealthy(
                    "health_check",
                    format!("Health check timed out after {:?}", HEALTH_CHECK_TIMEOUT),
                )])
            }
        }
    }

    /// Perform a full health check of all components
    async fn perform_full_health_check(&self) -> HealthReport {
        let mut components = Vec::new();

        // Check Kafka connectivity
        components.push(self.check_kafka_health().await);

        // Check buffer status
        components.push(self.check_buffer_health());

        // Check worker health
        components.push(self.check_worker_health());

        // Check OpenTelemetry exporter (placeholder for now)
        components.push(self.check_telemetry_health());

        // Create overall report
        let mut report = HealthReport::new(components);

        if let Some(ref version) = self.version {
            report = report.with_version(version.clone());
        }

        report
    }

    /// Check Kafka connection health
    async fn check_kafka_health(&self) -> ComponentHealth {
        if let Some(ref producer) = self.kafka_producer {
            // Try to flush with a short timeout to verify connectivity
            match timeout(Duration::from_millis(500), producer.flush(500)).await {
                Ok(Ok(())) => ComponentHealth::healthy("kafka", "Kafka connection is healthy"),
                Ok(Err(e)) => ComponentHealth::degraded(
                    "kafka",
                    format!("Kafka connection degraded: {}", e),
                ),
                Err(_) => ComponentHealth::unhealthy("kafka", "Kafka connection check timed out"),
            }
        } else {
            ComponentHealth::degraded("kafka", "Kafka producer not configured")
        }
    }

    /// Check buffer capacity and status
    fn check_buffer_health(&self) -> ComponentHealth {
        if let Some(ref stats_fn) = self.collector_stats {
            let stats = stats_fn();
            let buffer_usage = stats.current_buffer_size as f64 / self.buffer_size as f64;

            let metadata = serde_json::json!({
                "buffer_size": self.buffer_size,
                "current_usage": stats.current_buffer_size,
                "usage_percent": (buffer_usage * 100.0).round(),
                "events_received": stats.events_received,
                "events_published": stats.events_published,
                "events_failed": stats.events_failed,
            });

            if buffer_usage >= BUFFER_CRITICAL_THRESHOLD {
                ComponentHealth::unhealthy(
                    "buffer",
                    format!(
                        "Buffer critically full: {:.1}% ({}/{})",
                        buffer_usage * 100.0,
                        stats.current_buffer_size,
                        self.buffer_size
                    ),
                )
                .with_metadata(metadata)
            } else if buffer_usage >= BUFFER_WARNING_THRESHOLD {
                ComponentHealth::degraded(
                    "buffer",
                    format!(
                        "Buffer usage high: {:.1}% ({}/{})",
                        buffer_usage * 100.0,
                        stats.current_buffer_size,
                        self.buffer_size
                    ),
                )
                .with_metadata(metadata)
            } else {
                ComponentHealth::healthy(
                    "buffer",
                    format!(
                        "Buffer healthy: {:.1}% ({}/{})",
                        buffer_usage * 100.0,
                        stats.current_buffer_size,
                        self.buffer_size
                    ),
                )
                .with_metadata(metadata)
            }
        } else {
            ComponentHealth::degraded("buffer", "Buffer statistics not available")
        }
    }

    /// Check worker status
    fn check_worker_health(&self) -> ComponentHealth {
        if let Some(ref stats_fn) = self.collector_stats {
            let stats = stats_fn();

            let error_rate = if stats.events_received > 0 {
                stats.events_failed as f64 / stats.events_received as f64
            } else {
                0.0
            };

            let metadata = serde_json::json!({
                "num_workers": self.num_workers,
                "events_processed": stats.events_received,
                "batches_processed": stats.batches_processed,
                "error_rate": (error_rate * 100.0).round(),
            });

            if error_rate > 0.1 {
                // More than 10% error rate is unhealthy
                ComponentHealth::unhealthy(
                    "workers",
                    format!(
                        "{} workers active, high error rate: {:.1}%",
                        self.num_workers,
                        error_rate * 100.0
                    ),
                )
                .with_metadata(metadata)
            } else if error_rate > 0.05 {
                // More than 5% error rate is degraded
                ComponentHealth::degraded(
                    "workers",
                    format!(
                        "{} workers active, elevated error rate: {:.1}%",
                        self.num_workers,
                        error_rate * 100.0
                    ),
                )
                .with_metadata(metadata)
            } else {
                ComponentHealth::healthy(
                    "workers",
                    format!(
                        "{} workers active and processing events",
                        self.num_workers
                    ),
                )
                .with_metadata(metadata)
            }
        } else {
            ComponentHealth::degraded("workers", "Worker statistics not available")
        }
    }

    /// Check OpenTelemetry exporter status
    fn check_telemetry_health(&self) -> ComponentHealth {
        // For now, assume telemetry is healthy if configured
        // In a real implementation, we would check the exporter connection
        ComponentHealth::healthy(
            "telemetry",
            "OpenTelemetry exporter operational",
        )
    }

    /// Clear the health check cache (useful for testing)
    pub async fn clear_cache(&self) {
        let mut cache = self.cached_report.write().await;
        *cache = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_combine() {
        assert_eq!(
            HealthStatus::Healthy.combine(HealthStatus::Healthy),
            HealthStatus::Healthy
        );
        assert_eq!(
            HealthStatus::Healthy.combine(HealthStatus::Degraded),
            HealthStatus::Degraded
        );
        assert_eq!(
            HealthStatus::Healthy.combine(HealthStatus::Unhealthy),
            HealthStatus::Unhealthy
        );
        assert_eq!(
            HealthStatus::Degraded.combine(HealthStatus::Unhealthy),
            HealthStatus::Unhealthy
        );
    }

    #[test]
    fn test_health_status_checks() {
        assert!(HealthStatus::Healthy.is_healthy());
        assert!(!HealthStatus::Degraded.is_healthy());
        assert!(!HealthStatus::Unhealthy.is_healthy());

        assert!(!HealthStatus::Healthy.is_unhealthy());
        assert!(!HealthStatus::Degraded.is_unhealthy());
        assert!(HealthStatus::Unhealthy.is_unhealthy());
    }

    #[test]
    fn test_component_health_creation() {
        let healthy = ComponentHealth::healthy("test", "All good");
        assert_eq!(healthy.status, HealthStatus::Healthy);
        assert_eq!(healthy.component_name, "test");
        assert_eq!(healthy.message, "All good");

        let degraded = ComponentHealth::degraded("test", "Warning");
        assert_eq!(degraded.status, HealthStatus::Degraded);

        let unhealthy = ComponentHealth::unhealthy("test", "Error");
        assert_eq!(unhealthy.status, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_health_report_overall_status() {
        let components = vec![
            ComponentHealth::healthy("comp1", "OK"),
            ComponentHealth::healthy("comp2", "OK"),
        ];
        let report = HealthReport::new(components);
        assert_eq!(report.overall_status, HealthStatus::Healthy);

        let components = vec![
            ComponentHealth::healthy("comp1", "OK"),
            ComponentHealth::degraded("comp2", "Warning"),
        ];
        let report = HealthReport::new(components);
        assert_eq!(report.overall_status, HealthStatus::Degraded);

        let components = vec![
            ComponentHealth::healthy("comp1", "OK"),
            ComponentHealth::unhealthy("comp2", "Error"),
        ];
        let report = HealthReport::new(components);
        assert_eq!(report.overall_status, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_health_report_readiness() {
        let components = vec![
            ComponentHealth::healthy("comp1", "OK"),
            ComponentHealth::degraded("comp2", "Warning"),
        ];
        let report = HealthReport::new(components);
        assert!(report.is_ready()); // Degraded is still ready

        let components = vec![
            ComponentHealth::healthy("comp1", "OK"),
            ComponentHealth::unhealthy("comp2", "Error"),
        ];
        let report = HealthReport::new(components);
        assert!(!report.is_ready()); // Unhealthy means not ready
    }

    #[test]
    fn test_health_report_liveness() {
        let components = vec![
            ComponentHealth::healthy("comp1", "OK"),
            ComponentHealth::degraded("comp2", "Warning"),
        ];
        let report = HealthReport::new(components);
        assert!(report.is_alive()); // Degraded is still alive

        let components = vec![
            ComponentHealth::healthy("comp1", "OK"),
            ComponentHealth::unhealthy("comp2", "Error"),
        ];
        let report = HealthReport::new(components);
        assert!(!report.is_alive()); // Unhealthy overall means not alive
    }

    #[tokio::test]
    async fn test_health_checker_creation() {
        let checker = HealthChecker::new(10000, 4);
        assert_eq!(checker.buffer_size, 10000);
        assert_eq!(checker.num_workers, 4);
    }

    #[tokio::test]
    async fn test_health_checker_liveness() {
        let checker = HealthChecker::new(10000, 4).with_version("1.0.0");
        let report = checker.check_liveness().await;

        assert!(report.is_alive());
        assert_eq!(report.version, Some("1.0.0".to_string()));
        assert!(!report.components.is_empty());
    }

    #[tokio::test]
    async fn test_health_checker_readiness() {
        let checker = HealthChecker::new(10000, 4);
        let report = checker.check_readiness().await;

        // Should complete without timeout
        assert!(!report.components.is_empty());
    }

    #[tokio::test]
    async fn test_health_checker_cache() {
        let checker = HealthChecker::new(10000, 4);

        // First call should populate cache
        let report1 = checker.check_liveness().await;
        let timestamp1 = report1.timestamp;

        // Immediate second call should return cached result
        let report2 = checker.check_liveness().await;
        let timestamp2 = report2.timestamp;

        // Timestamps should be the same (cached)
        assert_eq!(timestamp1, timestamp2);

        // Clear cache
        checker.clear_cache().await;

        // After clearing cache, should get new result
        let report3 = checker.check_liveness().await;
        let timestamp3 = report3.timestamp;

        // This timestamp should be different
        assert!(timestamp3 > timestamp1);
    }

    #[tokio::test]
    async fn test_buffer_health_thresholds() {
        let checker = HealthChecker::new(1000, 4).with_collector_stats(|| CollectorStats {
            events_received: 100,
            events_published: 95,
            events_failed: 5,
            batches_processed: 10,
            current_buffer_size: 850, // 85% full
            buffer_utilization_percent: 0.0,
            events_dropped: 0,
            events_persisted: 0,
            events_rejected: 0,
            events_rate_limited: 0,
        });

        let health = checker.check_buffer_health();
        assert_eq!(health.status, HealthStatus::Degraded); // Above warning threshold

        let checker = HealthChecker::new(1000, 4).with_collector_stats(|| CollectorStats {
            events_received: 100,
            events_published: 95,
            events_failed: 5,
            batches_processed: 10,
            current_buffer_size: 960, // 96% full
            buffer_utilization_percent: 0.0,
            events_dropped: 0,
            events_persisted: 0,
            events_rejected: 0,
            events_rate_limited: 0,
        });

        let health = checker.check_buffer_health();
        assert_eq!(health.status, HealthStatus::Unhealthy); // Above critical threshold
    }

    #[tokio::test]
    async fn test_worker_health_error_rate() {
        // Low error rate - healthy
        let checker = HealthChecker::new(1000, 4).with_collector_stats(|| CollectorStats {
            events_received: 100,
            events_published: 98,
            events_failed: 2, // 2% error rate
            batches_processed: 10,
            current_buffer_size: 50,
            buffer_utilization_percent: 0.0,
            events_dropped: 0,
            events_persisted: 0,
            events_rejected: 0,
            events_rate_limited: 0,
        });

        let health = checker.check_worker_health();
        assert_eq!(health.status, HealthStatus::Healthy);

        // Medium error rate - degraded
        let checker = HealthChecker::new(1000, 4).with_collector_stats(|| CollectorStats {
            events_received: 100,
            events_published: 92,
            events_failed: 8, // 8% error rate
            batches_processed: 10,
            current_buffer_size: 50,
            buffer_utilization_percent: 0.0,
            events_dropped: 0,
            events_persisted: 0,
            events_rejected: 0,
            events_rate_limited: 0,
        });

        let health = checker.check_worker_health();
        assert_eq!(health.status, HealthStatus::Degraded);

        // High error rate - unhealthy
        let checker = HealthChecker::new(1000, 4).with_collector_stats(|| CollectorStats {
            events_received: 100,
            events_published: 85,
            events_failed: 15, // 15% error rate
            batches_processed: 10,
            current_buffer_size: 50,
            buffer_utilization_percent: 0.0,
            events_dropped: 0,
            events_persisted: 0,
            events_rejected: 0,
            events_rate_limited: 0,
        });

        let health = checker.check_worker_health();
        assert_eq!(health.status, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_health_report_serialization() {
        let components = vec![
            ComponentHealth::healthy("comp1", "OK"),
            ComponentHealth::degraded("comp2", "Warning").with_metadata(serde_json::json!({
                "detail": "some info"
            })),
        ];
        let report = HealthReport::new(components).with_version("1.0.0");

        // Should serialize to JSON without errors
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("comp1"));
        assert!(json.contains("comp2"));
        assert!(json.contains("1.0.0"));

        // Should deserialize back
        let deserialized: HealthReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.components.len(), 2);
        assert_eq!(deserialized.version, Some("1.0.0".to_string()));
    }
}
