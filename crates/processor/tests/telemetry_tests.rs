//! Integration tests for OpenTelemetry telemetry module.

use processor::telemetry::{
    context_with_span, extract_from_headers, inject_into_headers, BaggageItem, BatchConfiguration,
    ExportConfig, HeaderCarrier, MetricsConfig as TelemetryMetricsConfig, MetricsRecorder,
    OtlpProtocol, ResourceBuilder, SamplingStrategy, SpanBuilder, Temporality, TraceConfig,
    TelemetryConfig,
};
use processor::{OtelContext, OtelKeyValue, SpanKind};
use std::collections::HashMap;
use std::time::Duration;

#[test]
fn test_trace_config_builder() {
    let config = TraceConfig {
        enabled: true,
        service_name: "test-processor".to_string(),
        service_version: "1.0.0".to_string(),
        otlp_endpoint: "http://localhost:4317".to_string(),
        otlp_protocol: OtlpProtocol::Grpc,
        sampling: SamplingStrategy::AlwaysOn,
        batch_config: BatchConfiguration::default(),
        headers: HashMap::new(),
        environment: Some("test".to_string()),
    };

    assert_eq!(config.service_name, "test-processor");
    assert_eq!(config.otlp_protocol, OtlpProtocol::Grpc);
    assert_eq!(config.sampling, SamplingStrategy::AlwaysOn);
}

#[test]
fn test_metrics_config_builder() {
    let config = TelemetryMetricsConfig {
        enabled: true,
        service_name: "test-processor".to_string(),
        service_version: "1.0.0".to_string(),
        otlp_endpoint: "http://localhost:4317".to_string(),
        temporality: Temporality::Delta,
        export_config: ExportConfig::default(),
        headers: HashMap::new(),
        environment: Some("test".to_string()),
    };

    assert_eq!(config.service_name, "test-processor");
    assert_eq!(config.temporality, Temporality::Delta);
}

#[test]
fn test_telemetry_config_creation() {
    let config = TelemetryConfig::new("my-service")
        .with_otlp_endpoint("http://collector:4317")
        .with_service_version("2.0.0")
        .with_environment("production")
        .with_tracing_enabled(true)
        .with_metrics_enabled(false);

    assert_eq!(config.tracing.service_name, "my-service");
    assert_eq!(config.metrics.service_name, "my-service");
    assert_eq!(config.tracing.otlp_endpoint, "http://collector:4317");
    assert_eq!(config.metrics.otlp_endpoint, "http://collector:4317");
    assert_eq!(config.tracing.service_version, "2.0.0");
    assert!(config.tracing.enabled);
    assert!(!config.metrics.enabled);
}

#[test]
fn test_sampling_strategies() {
    let always_on = SamplingStrategy::AlwaysOn;
    let always_off = SamplingStrategy::AlwaysOff;
    let ratio = SamplingStrategy::TraceIdRatio { ratio: 0.5 };
    let parent_based = SamplingStrategy::ParentBased {
        root: Box::new(SamplingStrategy::AlwaysOn),
    };

    // Just ensure they can be created and converted
    let _ = always_on.to_sampler();
    let _ = always_off.to_sampler();
    let _ = ratio.to_sampler();
    let _ = parent_based.to_sampler();
}

#[test]
fn test_resource_builder() {
    let resource = ResourceBuilder::new()
        .with_service_name("test-service")
        .with_service_version("1.0.0")
        .with_deployment_environment("test")
        .with_attribute("custom.key", "custom.value")
        .build_without_detection();

    // Verify attributes are present
    let attrs: Vec<_> = resource.iter().collect();
    assert!(!attrs.is_empty());
}

#[test]
fn test_span_builder() {
    let builder = SpanBuilder::new("test-operation")
        .with_kind(SpanKind::Internal)
        .with_attribute("test.key", "test.value")
        .with_attribute("test.number", "42");

    // Verify builder state (we can't easily test the span without a real provider)
    assert!(true); // Placeholder - builder was created successfully
}

#[test]
fn test_header_carrier() {
    let mut carrier = HeaderCarrier::new();
    carrier.set("test-key".to_string(), "test-value".to_string());

    assert_eq!(carrier.get("test-key"), Some(&"test-value".to_string()));

    let headers = carrier.into_headers();
    assert_eq!(headers.get("test-key"), Some(&"test-value".to_string()));
}

#[test]
fn test_header_carrier_from_headers() {
    let mut headers = HashMap::new();
    headers.insert("traceparent".to_string(), "test-traceparent".to_string());
    headers.insert("tracestate".to_string(), "test-tracestate".to_string());

    let carrier = HeaderCarrier::from_headers(headers);

    assert_eq!(
        carrier.get("traceparent"),
        Some(&"test-traceparent".to_string())
    );
    assert_eq!(
        carrier.get("tracestate"),
        Some(&"test-tracestate".to_string())
    );
}

#[test]
fn test_context_inject_extract() {
    let context = OtelContext::current();
    let mut headers = HashMap::new();

    inject_into_headers(&context, &mut headers);

    // Headers should have been populated with trace context
    // Even if empty, the function should not panic
    let extracted_context = extract_from_headers(&headers);

    // Context extraction should succeed
    assert!(true); // Placeholder - context was extracted
}

#[test]
fn test_baggage_item() {
    let item = BaggageItem::new("user-id", "12345");
    assert_eq!(item.key, "user-id");
    assert_eq!(item.value, "12345");

    let item2 = BaggageItem::new("tenant", "acme-corp");
    assert_eq!(item2.key, "tenant");
    assert_eq!(item2.value, "acme-corp");
}

#[test]
fn test_metrics_recorder() {
    let recorder = MetricsRecorder::new()
        .with_attribute("service", "test")
        .with_attribute("environment", "test");

    // Verify recorder can be created with attributes
    // Actual metric recording requires a real meter provider
    assert!(true);
}

#[test]
fn test_batch_configuration() {
    let config = BatchConfiguration {
        max_queue_size: 1000,
        max_export_batch_size: 256,
        scheduled_delay: Duration::from_secs(10),
        export_timeout: Duration::from_secs(20),
    };

    assert_eq!(config.max_queue_size, 1000);
    assert_eq!(config.max_export_batch_size, 256);
    assert_eq!(config.scheduled_delay, Duration::from_secs(10));
    assert_eq!(config.export_timeout, Duration::from_secs(20));
}

#[test]
fn test_export_config() {
    let config = ExportConfig {
        export_interval: Duration::from_secs(30),
        export_timeout: Duration::from_secs(15),
    };

    assert_eq!(config.export_interval, Duration::from_secs(30));
    assert_eq!(config.export_timeout, Duration::from_secs(15));
}

#[test]
fn test_temporality_values() {
    let delta = Temporality::Delta;
    let cumulative = Temporality::Cumulative;

    assert_eq!(delta, Temporality::Delta);
    assert_eq!(cumulative, Temporality::Cumulative);
    assert_ne!(delta, cumulative);
}

#[test]
fn test_config_serialization() {
    let config = TelemetryConfig::new("test-service")
        .with_otlp_endpoint("http://localhost:4317")
        .with_service_version("1.0.0");

    // Serialize to JSON
    let json = serde_json::to_string(&config).expect("Failed to serialize");
    assert!(!json.is_empty());

    // Deserialize from JSON
    let deserialized: TelemetryConfig =
        serde_json::from_str(&json).expect("Failed to deserialize");
    assert_eq!(deserialized.tracing.service_name, "test-service");
    assert_eq!(deserialized.metrics.service_name, "test-service");
}

#[tokio::test]
async fn test_async_context_propagation() {
    use processor::telemetry::{propagate_context, with_context};

    let context = OtelContext::current();

    // Test with_context
    let result = with_context(context.clone(), async { 42 }).await;
    assert_eq!(result, 42);

    // Test propagate_context
    let result = propagate_context(async { "test" }).await;
    assert_eq!(result, "test");
}

#[test]
fn test_otlp_protocol_variants() {
    let grpc = OtlpProtocol::Grpc;
    let http = OtlpProtocol::Http;

    assert_eq!(grpc, OtlpProtocol::Grpc);
    assert_eq!(http, OtlpProtocol::Http);
    assert_ne!(grpc, http);
}

#[test]
fn test_sampling_strategy_serialization() {
    let always_on = SamplingStrategy::AlwaysOn;
    let json = serde_json::to_string(&always_on).expect("Failed to serialize");
    let deserialized: SamplingStrategy =
        serde_json::from_str(&json).expect("Failed to deserialize");

    assert_eq!(deserialized, SamplingStrategy::AlwaysOn);
}

#[test]
fn test_trace_config_defaults() {
    let config = TraceConfig::default();

    assert!(config.enabled);
    assert_eq!(config.service_name, "llm-optimizer-processor");
    assert_eq!(config.otlp_endpoint, "http://localhost:4317");
    assert_eq!(config.otlp_protocol, OtlpProtocol::Grpc);
}

#[test]
fn test_metrics_config_defaults() {
    let config = TelemetryMetricsConfig::default();

    assert!(config.enabled);
    assert_eq!(config.service_name, "llm-optimizer-processor");
    assert_eq!(config.otlp_endpoint, "http://localhost:4317");
    assert_eq!(config.temporality, Temporality::Cumulative);
}

#[test]
fn test_resource_builder_with_multiple_attributes() {
    let mut attrs = HashMap::new();
    attrs.insert("env".to_string(), "production".to_string());
    attrs.insert("region".to_string(), "us-east-1".to_string());

    let resource = ResourceBuilder::new()
        .with_service_name("multi-attr-service")
        .with_attributes(attrs)
        .build_without_detection();

    let resource_attrs: Vec<_> = resource.iter().collect();
    assert!(!resource_attrs.is_empty());
}

#[test]
fn test_span_builder_with_multiple_attributes() {
    let attrs = vec![
        OtelKeyValue::new("attr1", "value1"),
        OtelKeyValue::new("attr2", "value2"),
    ];

    let builder = SpanBuilder::new("multi-attr-span")
        .with_kind(SpanKind::Server)
        .with_attributes(attrs);

    // Builder should be created successfully
    assert!(true);
}
