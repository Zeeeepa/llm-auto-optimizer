//! OpenTelemetry Integration
//!
//! This module provides OpenTelemetry metrics and tracing integration
//! for comprehensive observability.

use opentelemetry::metrics::{Counter, Histogram, Meter, MeterProvider as _, UpDownCounter};
use opentelemetry::KeyValue;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::Resource;
use std::sync::Arc;
use thiserror::Error;

use crate::feedback_events::FeedbackEvent;

/// Telemetry error types
#[derive(Error, Debug)]
pub enum TelemetryError {
    #[error("Telemetry initialization error: {0}")]
    InitializationError(String),

    #[error("Metrics export error: {0}")]
    ExportError(String),

    #[error(transparent)]
    OpenTelemetryError(#[from] opentelemetry::metrics::MetricsError),
}

pub type Result<T> = std::result::Result<T, TelemetryError>;

/// Telemetry metrics collector
pub struct TelemetryMetrics {
    /// Meter for creating instruments
    meter: Meter,
    /// Event counter
    event_counter: Counter<u64>,
    /// Event processing latency
    processing_latency: Histogram<f64>,
    /// Active collectors
    active_collectors: UpDownCounter<i64>,
    /// Event batch size
    batch_size: Histogram<u64>,
    /// Error counter
    error_counter: Counter<u64>,
    /// Kafka publish latency
    kafka_publish_latency: Histogram<f64>,
    /// Kafka publish errors
    kafka_errors: Counter<u64>,
    /// Rate limit checks counter
    rate_limit_checks: Counter<u64>,
    /// Rate limit current usage gauge
    rate_limit_usage: UpDownCounter<i64>,
    /// Active sources being tracked
    rate_limit_active_sources: UpDownCounter<i64>,
}

impl TelemetryMetrics {
    /// Create new telemetry metrics
    pub fn new(meter: Meter) -> Result<Self> {
        let event_counter = meter
            .u64_counter("feedback_events_total")
            .with_description("Total number of feedback events received")
            .with_unit("events")
            .init();

        let processing_latency = meter
            .f64_histogram("feedback_processing_duration_ms")
            .with_description("Time taken to process feedback events")
            .with_unit("ms")
            .init();

        let active_collectors = meter
            .i64_up_down_counter("active_feedback_collectors")
            .with_description("Number of active feedback collectors")
            .init();

        let batch_size = meter
            .u64_histogram("feedback_batch_size")
            .with_description("Number of events in each batch")
            .with_unit("events")
            .init();

        let error_counter = meter
            .u64_counter("feedback_errors_total")
            .with_description("Total number of feedback processing errors")
            .with_unit("errors")
            .init();

        let kafka_publish_latency = meter
            .f64_histogram("kafka_publish_duration_ms")
            .with_description("Time taken to publish to Kafka")
            .with_unit("ms")
            .init();

        let kafka_errors = meter
            .u64_counter("kafka_errors_total")
            .with_description("Total number of Kafka publish errors")
            .with_unit("errors")
            .init();

        let rate_limit_checks = meter
            .u64_counter("rate_limit_checks_total")
            .with_description("Total number of rate limit checks")
            .with_unit("checks")
            .init();

        let rate_limit_usage = meter
            .i64_up_down_counter("rate_limit_current_usage")
            .with_description("Current rate limit usage")
            .init();

        let rate_limit_active_sources = meter
            .i64_up_down_counter("rate_limit_active_sources")
            .with_description("Number of active sources being tracked")
            .init();

        Ok(Self {
            meter,
            event_counter,
            processing_latency,
            active_collectors,
            batch_size,
            error_counter,
            kafka_publish_latency,
            kafka_errors,
            rate_limit_checks,
            rate_limit_usage,
            rate_limit_active_sources,
        })
    }

    /// Record event received
    pub fn record_event(&self, event: &FeedbackEvent) {
        let event_type = event.event_type();
        self.event_counter.add(1, &[KeyValue::new("event_type", event_type)]);
    }

    /// Record processing latency
    pub fn record_processing_latency(&self, duration_ms: f64, event_type: &str) {
        self.processing_latency.record(
            duration_ms,
            &[KeyValue::new("event_type", event_type.to_string())],
        );
    }

    /// Increment active collectors
    pub fn increment_active_collectors(&self) {
        self.active_collectors.add(1, &[]);
    }

    /// Decrement active collectors
    pub fn decrement_active_collectors(&self) {
        self.active_collectors.add(-1, &[]);
    }

    /// Record batch size
    pub fn record_batch_size(&self, size: u64) {
        self.batch_size.record(size, &[]);
    }

    /// Record error
    pub fn record_error(&self, error_type: &str) {
        self.error_counter.add(1, &[KeyValue::new("error_type", error_type.to_string())]);
    }

    /// Record Kafka publish latency
    pub fn record_kafka_publish_latency(&self, duration_ms: f64, success: bool) {
        self.kafka_publish_latency.record(
            duration_ms,
            &[KeyValue::new("success", success.to_string())],
        );
    }

    /// Record Kafka error
    pub fn record_kafka_error(&self, error_type: &str) {
        self.kafka_errors.add(1, &[KeyValue::new("error_type", error_type.to_string())]);
    }

    /// Record rate limit check
    pub fn record_rate_limit_check(&self, allowed: bool) {
        let status = if allowed { "allowed" } else { "denied" };
        self.rate_limit_checks.add(1, &[KeyValue::new("status", status.to_string())]);
    }

    /// Update rate limit usage
    pub fn update_rate_limit_usage(&self, delta: i64) {
        self.rate_limit_usage.add(delta, &[]);
    }

    /// Update active sources count
    pub fn update_rate_limit_sources(&self, count: i64) {
        self.rate_limit_active_sources.add(count, &[]);
    }
}

/// Telemetry provider configuration
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// Service name
    pub service_name: String,
    /// Service version
    pub service_version: String,
    /// Export interval in seconds
    pub export_interval_secs: u64,
    /// OTLP endpoint (optional)
    pub otlp_endpoint: Option<String>,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            service_name: "llm-optimizer-collector".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            export_interval_secs: 60,
            otlp_endpoint: None,
        }
    }
}

/// Telemetry provider
pub struct TelemetryProvider {
    /// Meter provider
    meter_provider: SdkMeterProvider,
    /// Telemetry metrics
    metrics: Arc<TelemetryMetrics>,
}

impl TelemetryProvider {
    /// Initialize telemetry provider
    pub fn init(config: TelemetryConfig) -> Result<Self> {
        // Create resource
        let resource = Resource::new(vec![
            KeyValue::new("service.name", config.service_name.clone()),
            KeyValue::new("service.version", config.service_version.clone()),
        ]);

        // Create meter provider
        // For development without OTLP, create a meter provider without periodic export
        // Metrics will still be collected but not exported
        // TODO: Add OTLP exporter support when endpoint is configured
        let meter_provider = SdkMeterProvider::builder()
            .with_resource(resource)
            .build();

        // Create meter
        let meter = meter_provider.meter("feedback_collector");

        // Create metrics
        let metrics = Arc::new(TelemetryMetrics::new(meter)?);

        Ok(Self {
            meter_provider,
            metrics,
        })
    }

    /// Get telemetry metrics
    pub fn metrics(&self) -> Arc<TelemetryMetrics> {
        self.metrics.clone()
    }

    /// Shutdown telemetry
    pub fn shutdown(self) -> Result<()> {
        self.meter_provider
            .shutdown()
            .map_err(|e| TelemetryError::ExportError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feedback_events::{UserFeedbackEvent, PerformanceMetricsEvent};
    use uuid::Uuid;

    #[test]
    fn test_telemetry_config_default() {
        let config = TelemetryConfig::default();
        assert_eq!(config.service_name, "llm-optimizer-collector");
        assert_eq!(config.export_interval_secs, 60);
    }

    #[test]
    fn test_telemetry_provider_init() {
        let config = TelemetryConfig::default();
        let provider = TelemetryProvider::init(config).unwrap();
        let metrics = provider.metrics();

        // Metrics should be created
        assert!(Arc::strong_count(&metrics) >= 1);

        // Shutdown
        provider.shutdown().unwrap();
    }

    #[test]
    fn test_record_event() {
        let config = TelemetryConfig::default();
        let provider = TelemetryProvider::init(config).unwrap();
        let metrics = provider.metrics();

        let request_id = Uuid::new_v4();
        let event = FeedbackEvent::UserFeedback(UserFeedbackEvent::new("session1", request_id));

        // Should not panic
        metrics.record_event(&event);

        provider.shutdown().unwrap();
    }

    #[test]
    fn test_record_metrics() {
        let config = TelemetryConfig::default();
        let provider = TelemetryProvider::init(config).unwrap();
        let metrics = provider.metrics();

        // Record various metrics
        metrics.record_processing_latency(100.0, "user_feedback");
        metrics.increment_active_collectors();
        metrics.record_batch_size(50);
        metrics.record_error("validation_error");
        metrics.record_kafka_publish_latency(25.0, true);
        metrics.decrement_active_collectors();

        // Should not panic
        provider.shutdown().unwrap();
    }

    #[test]
    fn test_active_collectors_tracking() {
        let config = TelemetryConfig::default();
        let provider = TelemetryProvider::init(config).unwrap();
        let metrics = provider.metrics();

        metrics.increment_active_collectors();
        metrics.increment_active_collectors();
        metrics.decrement_active_collectors();

        // Net: +1 collector
        provider.shutdown().unwrap();
    }

    #[test]
    fn test_kafka_metrics() {
        let config = TelemetryConfig::default();
        let provider = TelemetryProvider::init(config).unwrap();
        let metrics = provider.metrics();

        metrics.record_kafka_publish_latency(50.0, true);
        metrics.record_kafka_publish_latency(150.0, false);
        metrics.record_kafka_error("connection_error");

        provider.shutdown().unwrap();
    }
}
