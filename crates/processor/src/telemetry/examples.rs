//! Usage examples for the telemetry module.
//!
//! This file contains example code demonstrating various telemetry features.

#![allow(dead_code)]

use crate::telemetry::{
    context_with_span, extract_from_headers, init_telemetry, inject_into_headers, metric_attributes,
    metric_names, MetricsRecorder, SamplingStrategy, SpanBuilder, TelemetryConfig,
};
use opentelemetry::trace::{Span, SpanKind, Tracer};
use opentelemetry::{global, Context, KeyValue};
use std::collections::HashMap;

/// Example: Initialize telemetry with default configuration
pub async fn example_init_default_telemetry() -> Result<(), Box<dyn std::error::Error>> {
    let config = TelemetryConfig::new("my-processor")
        .with_otlp_endpoint("http://localhost:4317")
        .with_service_version("1.0.0")
        .with_environment("production");

    let providers = init_telemetry(config)?;

    // Use telemetry...

    // Shutdown when done
    providers.shutdown().await?;

    Ok(())
}

/// Example: Create and configure a span manually
pub fn example_create_span() {
    let span = SpanBuilder::new("process_event")
        .with_kind(SpanKind::Internal)
        .with_attribute("event.type", "metric")
        .with_attribute("event.id", "12345")
        .start();

    // Use span...
    drop(span);
}

/// Example: Create nested spans to show parent-child relationships
pub fn example_nested_spans() {
    let tracer = global::tracer("processor");

    // Parent span
    let parent_span = tracer
        .span_builder("parent_operation")
        .with_kind(SpanKind::Server)
        .start(&tracer);

    // Attach parent context
    let parent_context = context_with_span(parent_span);
    let _guard = parent_context.attach();

    // Child span (automatically becomes a child of parent)
    let child_span = tracer
        .span_builder("child_operation")
        .with_kind(SpanKind::Internal)
        .start(&tracer);

    // Use child span...
    drop(child_span);
    drop(_guard);
}

/// Example: Propagate context across async boundaries
pub async fn example_async_context_propagation() {
    use crate::telemetry::propagate_context;

    let tracer = global::tracer("processor");

    let span = tracer
        .span_builder("async_operation")
        .with_kind(SpanKind::Internal)
        .start(&tracer);

    let context = context_with_span(span);
    let _guard = context.attach();

    // Spawn async task with context propagation
    let handle = tokio::spawn(propagate_context(async {
        // This task inherits the parent context
        process_async_work().await;
    }));

    handle.await.ok();
}

async fn process_async_work() {
    // Async work here...
}

/// Example: Inject and extract context for cross-service tracing
pub fn example_context_injection_extraction() {
    let tracer = global::tracer("processor");

    // Service A: Create span and inject context into headers
    let span_a = tracer
        .span_builder("outgoing_request")
        .with_kind(SpanKind::Client)
        .start(&tracer);

    let context_a = context_with_span(span_a);
    let mut headers = HashMap::new();
    inject_into_headers(&context_a, &mut headers);

    // Send request with headers...

    // Service B: Extract context from headers
    let incoming_headers = headers; // Received from request
    let context_b = extract_from_headers(&incoming_headers);
    let _guard = context_b.attach();

    let span_b = tracer
        .span_builder("incoming_request")
        .with_kind(SpanKind::Server)
        .start(&tracer);

    // Process request...
    drop(span_b);
}

/// Example: Record metrics
pub fn example_record_metrics() {
    let recorder = MetricsRecorder::new()
        .with_attribute("service", "processor")
        .with_attribute("environment", "production");

    // Record a counter
    recorder.record_counter(
        "processor",
        metric_names::EVENTS_PROCESSED,
        1,
        vec![KeyValue::new(metric_attributes::EVENT_TYPE, "metric")],
    );

    // Record a histogram
    recorder.record_histogram(
        "processor",
        metric_names::PROCESSING_DURATION,
        123.45,
        vec![KeyValue::new(metric_attributes::OPERATION_TYPE, "aggregation")],
    );

    // Record a gauge
    recorder.record_gauge(
        "processor",
        metric_names::WATERMARK_LAG,
        1000,
        vec![KeyValue::new(metric_attributes::PARTITION_ID, "0")],
    );
}

/// Example: Add events to spans
pub fn example_span_events() {
    let tracer = global::tracer("processor");

    let mut span = tracer
        .span_builder("complex_operation")
        .with_kind(SpanKind::Internal)
        .start(&tracer);

    span.add_event("Starting validation".to_string(), vec![]);

    // Do validation...

    span.add_event(
        "Validation complete".to_string(),
        vec![KeyValue::new("validation.result", "success")],
    );

    span.add_event("Starting processing".to_string(), vec![]);

    // Do processing...

    span.add_event("Processing complete".to_string(), vec![]);

    drop(span);
}

/// Example: Record exceptions in spans
pub fn example_span_with_error() {
    use crate::telemetry::record_exception;

    let tracer = global::tracer("processor");

    let mut span = tracer
        .span_builder("operation_with_error")
        .with_kind(SpanKind::Internal)
        .start(&tracer);

    // Simulate an error
    let result: Result<(), std::io::Error> =
        Err(std::io::Error::new(std::io::ErrorKind::Other, "Something went wrong"));

    if let Err(e) = result {
        record_exception(&mut span, &e);
    }

    drop(span);
}

/// Example: Configure sampling strategies
pub fn example_sampling_strategies() -> Result<(), Box<dyn std::error::Error>> {
    use crate::telemetry::{init_tracer_provider, TraceConfig};

    // Always sample (100%)
    let config = TraceConfig {
        sampling: SamplingStrategy::AlwaysOn,
        ..Default::default()
    };
    let _provider = init_tracer_provider(config)?;

    // Sample 10% of traces
    let config = TraceConfig {
        sampling: SamplingStrategy::TraceIdRatio { ratio: 0.1 },
        ..Default::default()
    };
    let _provider = init_tracer_provider(config)?;

    // Parent-based sampling (follow parent's decision)
    let config = TraceConfig {
        sampling: SamplingStrategy::ParentBased {
            root: Box::new(SamplingStrategy::TraceIdRatio { ratio: 0.5 }),
        },
        ..Default::default()
    };
    let _provider = init_tracer_provider(config)?;

    Ok(())
}

/// Example: Use with baggage for cross-cutting concerns
pub fn example_baggage() {
    use crate::telemetry::{get_baggage, with_baggage, BaggageItem};

    let context = Context::current();

    // Add baggage items
    let items = vec![
        BaggageItem::new("user-id", "12345"),
        BaggageItem::new("tenant-id", "acme-corp"),
        BaggageItem::new("request-id", "abc-123"),
    ];

    let context = with_baggage(&context, items);
    let _guard = context.attach();

    // Retrieve baggage values
    if let Some(user_id) = get_baggage(&context, "user-id") {
        println!("User ID: {}", user_id);
    }

    if let Some(tenant_id) = get_baggage(&context, "tenant-id") {
        println!("Tenant ID: {}", tenant_id);
    }
}

/// Example: Integrate with existing tracing crate
pub fn example_tracing_integration() -> Result<(), Box<dyn std::error::Error>> {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::Registry;

    let tracer = global::tracer("processor");
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    let subscriber = Registry::default().with(telemetry);
    tracing::subscriber::set_global_default(subscriber)?;

    // Now you can use tracing macros with OpenTelemetry
    tracing::info!("This will be exported to OpenTelemetry");
    tracing::error!("Errors are captured too");

    Ok(())
}

/// Example: Instrument a function with automatic span creation
pub async fn example_instrumented_function() {
    // Using tracing crate macros (requires tracing-opentelemetry integration)
    tracing::info_span!("process_batch", batch_size = 100).in_scope(|| {
        // Process batch...
    });
}

/// Example: Complete processor workflow with telemetry
pub async fn example_complete_workflow() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize telemetry
    let config = TelemetryConfig::new("stream-processor")
        .with_otlp_endpoint("http://localhost:4317")
        .with_service_version("1.0.0")
        .with_environment("production");

    let providers = init_telemetry(config)?;

    let tracer = global::tracer("processor");
    let recorder = MetricsRecorder::new().with_attribute("service", "processor");

    // Process events
    for i in 0..10 {
        let mut span = tracer
            .span_builder(format!("process_event_{}", i))
            .with_kind(SpanKind::Internal)
            .with_attributes(vec![
                KeyValue::new("event.id", i.to_string()),
                KeyValue::new("event.type", "metric"),
            ])
            .start(&tracer);

        let context = context_with_span(span.clone());
        let _guard = context.attach();

        span.add_event("Processing started".to_string(), vec![]);

        // Simulate processing
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Record metrics
        recorder.record_counter(
            "processor",
            metric_names::EVENTS_PROCESSED,
            1,
            vec![KeyValue::new(metric_attributes::EVENT_TYPE, "metric")],
        );

        span.add_event("Processing completed".to_string(), vec![]);
        drop(_guard);
        drop(span);
    }

    // Shutdown
    providers.shutdown().await?;

    Ok(())
}
