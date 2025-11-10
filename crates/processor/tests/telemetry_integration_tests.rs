//! Integration tests for OpenTelemetry with OTLP collector.
//!
//! These tests require a running OTLP collector. They are marked with #[ignore]
//! by default and can be run with: `cargo test --test telemetry_integration_tests -- --ignored`

use processor::telemetry::{
    context_with_span, extract_from_headers, inject_into_headers, init_meter_provider,
    init_telemetry, init_tracer_provider, metric_attributes, metric_names, shutdown_meter_provider,
    shutdown_tracer_provider, BatchConfiguration, ExportConfig, MetricsConfig as TelemetryMetricsConfig,
    MetricsRecorder, OtlpProtocol, SamplingStrategy, SpanBuilder, Temporality, TraceConfig,
    TelemetryConfig,
};
use processor::{otel_global, OtelContext, OtelKeyValue, SpanKind};
use std::collections::HashMap;
use std::time::Duration;

/// Helper to check if OTLP collector is available
fn is_otlp_available() -> bool {
    std::env::var("OTLP_ENDPOINT").is_ok()
        || std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok()
}

fn get_otlp_endpoint() -> String {
    std::env::var("OTLP_ENDPOINT")
        .or_else(|_| std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT"))
        .unwrap_or_else(|_| "http://localhost:4317".to_string())
}

#[tokio::test]
#[ignore] // Run with --ignored flag when OTLP collector is available
async fn test_trace_export_to_otlp() {
    if !is_otlp_available() {
        eprintln!("Skipping test: OTLP collector not available");
        return;
    }

    let config = TraceConfig {
        enabled: true,
        service_name: "test-processor".to_string(),
        service_version: "1.0.0".to_string(),
        otlp_endpoint: get_otlp_endpoint(),
        otlp_protocol: OtlpProtocol::Grpc,
        sampling: SamplingStrategy::AlwaysOn,
        batch_config: BatchConfiguration {
            max_queue_size: 512,
            max_export_batch_size: 128,
            scheduled_delay: Duration::from_secs(1),
            export_timeout: Duration::from_secs(5),
        },
        headers: HashMap::new(),
        environment: Some("test".to_string()),
    };

    let provider = init_tracer_provider(config).expect("Failed to init tracer provider");

    // Create and record some spans
    let tracer = otel_global::tracer("test-integration");

    let mut span = tracer
        .span_builder("test-operation")
        .with_kind(SpanKind::Internal)
        .with_attributes(vec![
            OtelKeyValue::new("test.id", "12345"),
            OtelKeyValue::new("test.type", "integration"),
        ])
        .start(&tracer);

    // Add event to span
    span.add_event("Processing started".to_string(), vec![]);

    // Simulate some work
    tokio::time::sleep(Duration::from_millis(100)).await;

    span.add_event("Processing completed".to_string(), vec![]);

    // End span
    drop(span);

    // Wait for batch export
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Shutdown provider
    shutdown_tracer_provider().await;
}

#[tokio::test]
#[ignore] // Run with --ignored flag when OTLP collector is available
async fn test_nested_spans() {
    if !is_otlp_available() {
        eprintln!("Skipping test: OTLP collector not available");
        return;
    }

    let config = TraceConfig {
        enabled: true,
        service_name: "test-nested-spans".to_string(),
        service_version: "1.0.0".to_string(),
        otlp_endpoint: get_otlp_endpoint(),
        otlp_protocol: OtlpProtocol::Grpc,
        sampling: SamplingStrategy::AlwaysOn,
        batch_config: BatchConfiguration::default(),
        headers: HashMap::new(),
        environment: Some("test".to_string()),
    };

    let _provider = init_tracer_provider(config).expect("Failed to init tracer provider");

    let tracer = otel_global::tracer("test-nested");

    // Parent span
    let parent_span = tracer
        .span_builder("parent-operation")
        .with_kind(SpanKind::Server)
        .with_attributes(vec![OtelKeyValue::new("span.level", "parent")])
        .start(&tracer);

    let parent_context = context_with_span(parent_span);
    let _guard = parent_context.attach();

    // Child span
    let child_span = tracer
        .span_builder("child-operation")
        .with_kind(SpanKind::Internal)
        .with_attributes(vec![OtelKeyValue::new("span.level", "child")])
        .start(&tracer);

    let child_context = context_with_span(child_span);
    let _child_guard = child_context.attach();

    // Simulate work
    tokio::time::sleep(Duration::from_millis(50)).await;

    drop(_child_guard);
    drop(_guard);

    // Wait for export
    tokio::time::sleep(Duration::from_secs(2)).await;

    shutdown_tracer_provider().await;
}

#[tokio::test]
#[ignore] // Run with --ignored flag when OTLP collector is available
async fn test_context_propagation_across_services() {
    if !is_otlp_available() {
        eprintln!("Skipping test: OTLP collector not available");
        return;
    }

    let config = TraceConfig {
        enabled: true,
        service_name: "test-context-propagation".to_string(),
        service_version: "1.0.0".to_string(),
        otlp_endpoint: get_otlp_endpoint(),
        otlp_protocol: OtlpProtocol::Grpc,
        sampling: SamplingStrategy::AlwaysOn,
        batch_config: BatchConfiguration::default(),
        headers: HashMap::new(),
        environment: Some("test".to_string()),
    };

    let _provider = init_tracer_provider(config).expect("Failed to init tracer provider");

    let tracer = otel_global::tracer("test-propagation");

    // Service A: Create span and inject context
    let span_a = tracer
        .span_builder("service-a-operation")
        .with_kind(SpanKind::Client)
        .start(&tracer);

    let context_a = context_with_span(span_a);
    let mut headers = HashMap::new();
    inject_into_headers(&context_a, &mut headers);

    // Verify headers were set
    assert!(
        headers.contains_key("traceparent"),
        "traceparent header should be set"
    );

    // Service B: Extract context and create child span
    let context_b = extract_from_headers(&headers);
    let _guard = context_b.attach();

    let span_b = tracer
        .span_builder("service-b-operation")
        .with_kind(SpanKind::Server)
        .start(&tracer);

    let _context_b = context_with_span(span_b);

    // Simulate work
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Wait for export
    tokio::time::sleep(Duration::from_secs(2)).await;

    shutdown_tracer_provider().await;
}

#[tokio::test]
#[ignore] // Run with --ignored flag when OTLP collector is available
async fn test_metrics_export_to_otlp() {
    if !is_otlp_available() {
        eprintln!("Skipping test: OTLP collector not available");
        return;
    }

    let config = TelemetryMetricsConfig {
        enabled: true,
        service_name: "test-metrics".to_string(),
        service_version: "1.0.0".to_string(),
        otlp_endpoint: get_otlp_endpoint(),
        temporality: Temporality::Cumulative,
        export_config: ExportConfig {
            export_interval: Duration::from_secs(1),
            export_timeout: Duration::from_secs(5),
        },
        headers: HashMap::new(),
        environment: Some("test".to_string()),
    };

    let _provider = init_meter_provider(config).expect("Failed to init meter provider");

    let recorder = MetricsRecorder::new()
        .with_attribute("service", "test-metrics")
        .with_attribute("environment", "test");

    // Record some metrics
    for i in 0..10 {
        recorder.record_counter(
            "test-meter",
            metric_names::EVENTS_PROCESSED,
            1,
            vec![OtelKeyValue::new(metric_attributes::EVENT_TYPE, "test")],
        );

        recorder.record_histogram(
            "test-meter",
            metric_names::PROCESSING_DURATION,
            (i as f64) * 10.0,
            vec![OtelKeyValue::new(metric_attributes::OPERATION_TYPE, "test")],
        );

        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Wait for metric export
    tokio::time::sleep(Duration::from_secs(2)).await;

    shutdown_meter_provider().expect("Failed to shutdown meter provider");
}

#[tokio::test]
#[ignore] // Run with --ignored flag when OTLP collector is available
async fn test_combined_telemetry() {
    if !is_otlp_available() {
        eprintln!("Skipping test: OTLP collector not available");
        return;
    }

    let config = TelemetryConfig::new("test-combined")
        .with_otlp_endpoint(get_otlp_endpoint())
        .with_service_version("1.0.0")
        .with_environment("test")
        .with_tracing_enabled(true)
        .with_metrics_enabled(true);

    let providers = init_telemetry(config).expect("Failed to init telemetry");

    // Create spans
    let tracer = otel_global::tracer("test-combined");
    let span = tracer
        .span_builder("combined-operation")
        .with_kind(SpanKind::Internal)
        .start(&tracer);

    let context = context_with_span(span);
    let _guard = context.attach();

    // Record metrics
    let recorder = MetricsRecorder::new();
    recorder.record_counter(
        "combined-meter",
        metric_names::EVENTS_PROCESSED,
        5,
        vec![],
    );

    // Simulate work
    tokio::time::sleep(Duration::from_millis(100)).await;

    drop(_guard);

    // Wait for export
    tokio::time::sleep(Duration::from_secs(2)).await;

    providers
        .shutdown()
        .await
        .expect("Failed to shutdown providers");
}

#[tokio::test]
#[ignore] // Run with --ignored flag when OTLP collector is available
async fn test_span_with_error() {
    if !is_otlp_available() {
        eprintln!("Skipping test: OTLP collector not available");
        return;
    }

    let config = TraceConfig {
        enabled: true,
        service_name: "test-error-spans".to_string(),
        service_version: "1.0.0".to_string(),
        otlp_endpoint: get_otlp_endpoint(),
        otlp_protocol: OtlpProtocol::Grpc,
        sampling: SamplingStrategy::AlwaysOn,
        batch_config: BatchConfiguration::default(),
        headers: HashMap::new(),
        environment: Some("test".to_string()),
    };

    let _provider = init_tracer_provider(config).expect("Failed to init tracer provider");

    let tracer = otel_global::tracer("test-errors");

    let mut span = tracer
        .span_builder("error-operation")
        .with_kind(SpanKind::Internal)
        .start(&tracer);

    // Simulate an error
    let error = std::io::Error::new(std::io::ErrorKind::Other, "Test error");
    processor::telemetry::record_exception(&mut span, &error);

    drop(span);

    // Wait for export
    tokio::time::sleep(Duration::from_secs(2)).await;

    shutdown_tracer_provider().await;
}

#[tokio::test]
#[ignore] // Run with --ignored flag when OTLP collector is available
async fn test_http_protocol() {
    if !is_otlp_available() {
        eprintln!("Skipping test: OTLP collector not available");
        return;
    }

    let http_endpoint = get_otlp_endpoint().replace("4317", "4318"); // HTTP port

    let config = TraceConfig {
        enabled: true,
        service_name: "test-http-protocol".to_string(),
        service_version: "1.0.0".to_string(),
        otlp_endpoint: http_endpoint,
        otlp_protocol: OtlpProtocol::Http,
        sampling: SamplingStrategy::AlwaysOn,
        batch_config: BatchConfiguration::default(),
        headers: HashMap::new(),
        environment: Some("test".to_string()),
    };

    // This may fail if HTTP endpoint is not configured, which is expected
    if let Ok(_provider) = init_tracer_provider(config) {
        let tracer = otel_global::tracer("test-http");
        let span = tracer
            .span_builder("http-protocol-test")
            .with_kind(SpanKind::Internal)
            .start(&tracer);

        drop(span);

        tokio::time::sleep(Duration::from_secs(2)).await;
        shutdown_tracer_provider().await;
    }
}
