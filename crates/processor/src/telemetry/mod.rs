//! OpenTelemetry instrumentation for the stream processor.
//!
//! This module provides comprehensive distributed tracing and metrics export
//! using OpenTelemetry. It includes:
//!
//! - **Tracing**: Distributed tracing with OTLP exporter, span instrumentation,
//!   and multiple sampling strategies.
//! - **Metrics**: OTLP metrics export with support for counters, histograms,
//!   and gauges.
//! - **Context Propagation**: W3C Trace Context and Baggage propagation for
//!   distributed tracing across service boundaries.
//! - **Resource Attributes**: Automatic detection and configuration of service
//!   metadata following OpenTelemetry semantic conventions.
//!
//! # Usage
//!
//! ## Initializing Tracing
//!
//! ```ignore
//! use processor::telemetry::{TraceConfig, init_tracer_provider};
//!
//! let config = TraceConfig {
//!     enabled: true,
//!     service_name: "my-processor".to_string(),
//!     otlp_endpoint: "http://localhost:4317".to_string(),
//!     ..Default::default()
//! };
//!
//! let provider = init_tracer_provider(config)?;
//! ```
//!
//! ## Creating Spans
//!
//! ```ignore
//! use processor::telemetry::SpanBuilder;
//! use opentelemetry::trace::SpanKind;
//!
//! let span = SpanBuilder::new("process_event")
//!     .with_kind(SpanKind::Internal)
//!     .with_attribute("event.type", "metric")
//!     .start();
//! ```
//!
//! Or using the macro:
//!
//! ```ignore
//! use processor::span;
//!
//! let span = span!("process_event", "event.type" => "metric");
//! ```
//!
//! ## Context Propagation
//!
//! ```ignore
//! use processor::telemetry::{inject_into_headers, extract_from_headers};
//! use opentelemetry::Context;
//!
//! // Inject context into outgoing request
//! let mut headers = HashMap::new();
//! inject_into_headers(&Context::current(), &mut headers);
//!
//! // Extract context from incoming request
//! let context = extract_from_headers(&headers);
//! ```
//!
//! ## Recording Metrics
//!
//! ```ignore
//! use processor::telemetry::{MetricsRecorder, metric_names};
//! use opentelemetry::KeyValue;
//!
//! let recorder = MetricsRecorder::new()
//!     .with_attribute("service", "processor");
//!
//! recorder.record_counter(
//!     "processor",
//!     metric_names::EVENTS_PROCESSED,
//!     1,
//!     vec![KeyValue::new("event.type", "metric")],
//! );
//! ```
//!
//! ## Integration with Existing Code
//!
//! The telemetry module integrates seamlessly with the existing `tracing` crate
//! used throughout the processor. Use `tracing-opentelemetry` to bridge the two:
//!
//! ```ignore
//! use tracing_subscriber::layer::SubscriberExt;
//! use tracing_subscriber::Registry;
//! use tracing_opentelemetry::OpenTelemetryLayer;
//!
//! let tracer = global::tracer("processor");
//! let telemetry = OpenTelemetryLayer::new(tracer);
//!
//! let subscriber = Registry::default().with(telemetry);
//! tracing::subscriber::set_global_default(subscriber)?;
//! ```

pub mod context;
pub mod metrics;
pub mod resource;
pub mod tracing;

// Re-export commonly used types
pub use context::{
    extract_context, extract_from_headers, inject_context, inject_into_headers,
    context_with_span, get_baggage, propagate_context, with_baggage, with_context,
    BaggageItem, HeaderCarrier,
};

pub use metrics::{
    init_meter_provider, shutdown_meter_provider, metric_attributes, metric_names,
    ExportConfig, MetricsConfig, MetricsRecorder, Temporality,
};

pub use resource::{create_processor_resource, ResourceBuilder};

pub use tracing::{
    init_tracer_provider, record_exception, shutdown_tracer_provider, BatchConfiguration,
    OtlpProtocol, SamplingStrategy, SpanBuilder, TraceConfig,
};

// Re-export OpenTelemetry types for convenience
pub use opentelemetry::{
    global,
    trace::{Span, SpanKind, Status, Tracer},
    Context, KeyValue,
};

/// Combined configuration for both tracing and metrics.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TelemetryConfig {
    /// Tracing configuration.
    pub tracing: TraceConfig,

    /// Metrics configuration.
    pub metrics: MetricsConfig,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            tracing: TraceConfig::default(),
            metrics: MetricsConfig::default(),
        }
    }
}

impl TelemetryConfig {
    /// Creates a new telemetry configuration.
    pub fn new(service_name: impl Into<String>) -> Self {
        let service_name = service_name.into();
        Self {
            tracing: TraceConfig {
                service_name: service_name.clone(),
                ..Default::default()
            },
            metrics: MetricsConfig {
                service_name,
                ..Default::default()
            },
        }
    }

    /// Sets the OTLP endpoint for both tracing and metrics.
    pub fn with_otlp_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        let endpoint = endpoint.into();
        self.tracing.otlp_endpoint = endpoint.clone();
        self.metrics.otlp_endpoint = endpoint;
        self
    }

    /// Sets the service version for both tracing and metrics.
    pub fn with_service_version(mut self, version: impl Into<String>) -> Self {
        let version = version.into();
        self.tracing.service_version = version.clone();
        self.metrics.service_version = version;
        self
    }

    /// Sets the deployment environment for both tracing and metrics.
    pub fn with_environment(mut self, env: impl Into<String>) -> Self {
        let env = Some(env.into());
        self.tracing.environment = env.clone();
        self.metrics.environment = env;
        self
    }

    /// Enables or disables tracing.
    pub fn with_tracing_enabled(mut self, enabled: bool) -> Self {
        self.tracing.enabled = enabled;
        self
    }

    /// Enables or disables metrics.
    pub fn with_metrics_enabled(mut self, enabled: bool) -> Self {
        self.metrics.enabled = enabled;
        self
    }
}

/// Initializes both tracing and metrics providers.
///
/// # Errors
///
/// Returns an error if either provider fails to initialize.
pub fn init_telemetry(
    config: TelemetryConfig,
) -> Result<TelemetryProviders, Box<dyn std::error::Error>> {
    let tracer_provider = if config.tracing.enabled {
        Some(init_tracer_provider(config.tracing)?)
    } else {
        None
    };

    let meter_provider = if config.metrics.enabled {
        Some(init_meter_provider(config.metrics)?)
    } else {
        None
    };

    Ok(TelemetryProviders {
        tracer_provider,
        meter_provider,
    })
}

/// Container for initialized telemetry providers.
pub struct TelemetryProviders {
    pub tracer_provider: Option<opentelemetry_sdk::trace::TracerProvider>,
    pub meter_provider: Option<opentelemetry_sdk::metrics::SdkMeterProvider>,
}

impl TelemetryProviders {
    /// Shuts down both providers and flushes pending data.
    pub async fn shutdown(self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(_provider) = self.tracer_provider {
            shutdown_tracer_provider().await;
        }

        if let Some(_provider) = self.meter_provider {
            shutdown_meter_provider()?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_config_default() {
        let config = TelemetryConfig::default();
        assert!(config.tracing.enabled);
        assert!(config.metrics.enabled);
    }

    #[test]
    fn test_telemetry_config_new() {
        let config = TelemetryConfig::new("test-service");
        assert_eq!(config.tracing.service_name, "test-service");
        assert_eq!(config.metrics.service_name, "test-service");
    }

    #[test]
    fn test_telemetry_config_builder() {
        let config = TelemetryConfig::new("test-service")
            .with_otlp_endpoint("http://collector:4317")
            .with_service_version("1.0.0")
            .with_environment("production")
            .with_tracing_enabled(true)
            .with_metrics_enabled(false);

        assert_eq!(config.tracing.otlp_endpoint, "http://collector:4317");
        assert_eq!(config.metrics.otlp_endpoint, "http://collector:4317");
        assert_eq!(config.tracing.service_version, "1.0.0");
        assert_eq!(config.metrics.service_version, "1.0.0");
        assert_eq!(
            config.tracing.environment,
            Some("production".to_string())
        );
        assert_eq!(
            config.metrics.environment,
            Some("production".to_string())
        );
        assert!(config.tracing.enabled);
        assert!(!config.metrics.enabled);
    }

    #[test]
    fn test_telemetry_config_serialization() {
        let config = TelemetryConfig::new("test");
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: TelemetryConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.tracing.service_name, deserialized.tracing.service_name);
        assert_eq!(config.metrics.service_name, deserialized.metrics.service_name);
    }
}
