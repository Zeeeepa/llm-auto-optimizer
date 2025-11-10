//! Telemetry demonstration example.
//!
//! This example demonstrates the complete telemetry integration including:
//! - Trace provider initialization
//! - Meter provider initialization
//! - Span creation and nesting
//! - Context propagation
//! - Metric recording
//! - Error handling
//!
//! To run this example:
//! 1. Start the OTLP collector: `docker-compose -f docker-compose.telemetry.yml up -d`
//! 2. Run the example: `cargo run --example telemetry_demo`
//! 3. View traces in Jaeger: http://localhost:16686
//! 4. View metrics in Prometheus: http://localhost:9090

use processor::telemetry::{
    context_with_span, init_telemetry, metric_attributes, metric_names, record_exception,
    MetricsRecorder, SpanBuilder, TelemetryConfig,
};
use processor::{otel_global, OtelContext, OtelKeyValue, SpanKind};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("Starting telemetry demo...");

    // Initialize telemetry
    let config = TelemetryConfig::new("telemetry-demo")
        .with_otlp_endpoint("http://localhost:4317")
        .with_service_version("1.0.0")
        .with_environment("development")
        .with_tracing_enabled(true)
        .with_metrics_enabled(true);

    println!("Initializing OpenTelemetry providers...");
    let providers = init_telemetry(config)?;

    // Create metrics recorder
    let metrics = MetricsRecorder::new()
        .with_attribute("service", "telemetry-demo")
        .with_attribute("environment", "development");

    // Run demo scenarios
    println!("\n=== Running Demo Scenarios ===\n");

    // Scenario 1: Basic span creation
    println!("1. Creating basic spans...");
    demo_basic_spans().await;

    // Scenario 2: Nested spans
    println!("2. Creating nested spans...");
    demo_nested_spans().await;

    // Scenario 3: Recording metrics
    println!("3. Recording metrics...");
    demo_metrics(&metrics).await;

    // Scenario 4: Error handling
    println!("4. Demonstrating error handling...");
    demo_error_handling().await;

    // Scenario 5: Async operations
    println!("5. Demonstrating async operations...");
    demo_async_operations().await;

    // Scenario 6: Simulated event processing
    println!("6. Simulating event processing...");
    demo_event_processing(&metrics).await;

    println!("\n=== Demo Complete ===\n");
    println!("View traces at: http://localhost:16686");
    println!("View metrics at: http://localhost:9090");

    // Wait a bit for final export
    println!("\nWaiting for final export...");
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Shutdown
    println!("Shutting down telemetry providers...");
    providers.shutdown().await?;

    println!("Demo finished!");
    Ok(())
}

/// Demonstrates basic span creation
async fn demo_basic_spans() {
    let tracer = otel_global::tracer("demo");

    for i in 0..3 {
        let mut span = tracer
            .span_builder(format!("basic_operation_{}", i))
            .with_kind(SpanKind::Internal)
            .with_attributes(vec![
                OtelKeyValue::new("operation.id", i.to_string()),
                OtelKeyValue::new("operation.type", "basic"),
            ])
            .start(&tracer);

        span.add_event("Operation started".to_string(), vec![]);

        // Simulate work
        tokio::time::sleep(Duration::from_millis(50)).await;

        span.add_event(
            "Operation completed".to_string(),
            vec![OtelKeyValue::new("status", "success")],
        );
    }

    println!("   ✓ Created 3 basic spans");
}

/// Demonstrates nested span relationships
async fn demo_nested_spans() {
    let tracer = otel_global::tracer("demo");

    // Parent span
    let parent_span = tracer
        .span_builder("parent_operation")
        .with_kind(SpanKind::Server)
        .with_attributes(vec![OtelKeyValue::new("level", "parent")])
        .start(&tracer);

    let parent_context = context_with_span(parent_span);
    let _parent_guard = parent_context.attach();

    // Child spans
    for i in 0..2 {
        let child_span = tracer
            .span_builder(format!("child_operation_{}", i))
            .with_kind(SpanKind::Internal)
            .with_attributes(vec![
                OtelKeyValue::new("level", "child"),
                OtelKeyValue::new("child.id", i.to_string()),
            ])
            .start(&tracer);

        let child_context = context_with_span(child_span);
        let _child_guard = child_context.attach();

        // Simulate work
        tokio::time::sleep(Duration::from_millis(30)).await;
    }

    println!("   ✓ Created parent span with 2 children");
}

/// Demonstrates metric recording
async fn demo_metrics(recorder: &MetricsRecorder) {
    // Record counters
    for i in 0..5 {
        recorder.record_counter(
            "demo",
            metric_names::EVENTS_PROCESSED,
            1,
            vec![OtelKeyValue::new(metric_attributes::EVENT_TYPE, "demo")],
        );

        // Record histograms
        recorder.record_histogram(
            "demo",
            metric_names::PROCESSING_DURATION,
            (i as f64) * 10.5,
            vec![OtelKeyValue::new(
                metric_attributes::OPERATION_TYPE,
                "demo",
            )],
        );

        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    // Record gauge
    recorder.record_gauge(
        "demo",
        metric_names::WATERMARK_LAG,
        100,
        vec![OtelKeyValue::new(metric_attributes::PARTITION_ID, "0")],
    );

    println!("   ✓ Recorded 5 counters, 5 histograms, and 1 gauge");
}

/// Demonstrates error handling in spans
async fn demo_error_handling() {
    let tracer = otel_global::tracer("demo");

    let mut span = tracer
        .span_builder("operation_with_error")
        .with_kind(SpanKind::Internal)
        .start(&tracer);

    // Simulate an error
    let error = std::io::Error::new(std::io::ErrorKind::Other, "Simulated error for demo");

    record_exception(&mut span, &error);

    println!("   ✓ Recorded exception in span");
}

/// Demonstrates async operation tracing
async fn demo_async_operations() {
    use processor::telemetry::propagate_context;

    let tracer = otel_global::tracer("demo");

    let span = tracer
        .span_builder("async_parent")
        .with_kind(SpanKind::Internal)
        .start(&tracer);

    let context = context_with_span(span);
    let _guard = context.attach();

    // Spawn multiple async tasks
    let mut handles = vec![];

    for i in 0..3 {
        let handle = tokio::spawn(propagate_context(async move {
            let tracer = otel_global::tracer("demo");
            let _span = tracer
                .span_builder(format!("async_task_{}", i))
                .with_kind(SpanKind::Internal)
                .start(&tracer);

            tokio::time::sleep(Duration::from_millis(20)).await;
        }));

        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.ok();
    }

    println!("   ✓ Spawned 3 async tasks with context propagation");
}

/// Demonstrates simulated event processing workflow
async fn demo_event_processing(recorder: &MetricsRecorder) {
    let tracer = otel_global::tracer("demo");

    for event_id in 0..5 {
        // Main processing span
        let mut process_span = tracer
            .span_builder("process_event")
            .with_kind(SpanKind::Internal)
            .with_attributes(vec![
                OtelKeyValue::new("event.id", event_id.to_string()),
                OtelKeyValue::new("event.type", "metric"),
            ])
            .start(&tracer);

        let process_context = context_with_span(process_span.clone());
        let _process_guard = process_context.attach();

        process_span.add_event("Event received".to_string(), vec![]);

        // Validation span
        {
            let mut validate_span = tracer
                .span_builder("validate_event")
                .with_kind(SpanKind::Internal)
                .start(&tracer);

            tokio::time::sleep(Duration::from_millis(10)).await;

            validate_span.add_event("Validation passed".to_string(), vec![]);
        }

        // Processing span
        {
            let mut process_span = tracer
                .span_builder("process_data")
                .with_kind(SpanKind::Internal)
                .start(&tracer);

            tokio::time::sleep(Duration::from_millis(20)).await;

            process_span.add_event("Processing complete".to_string(), vec![]);
        }

        // State operation span
        {
            let mut state_span = tracer
                .span_builder("update_state")
                .with_kind(SpanKind::Internal)
                .with_attributes(vec![OtelKeyValue::new(
                    metric_attributes::STATE_BACKEND,
                    "memory",
                )])
                .start(&tracer);

            tokio::time::sleep(Duration::from_millis(5)).await;

            state_span.add_event("State updated".to_string(), vec![]);

            // Record state operation metric
            recorder.record_histogram(
                "demo",
                metric_names::STATE_OPERATION_DURATION,
                5.0,
                vec![
                    OtelKeyValue::new(metric_attributes::STATE_BACKEND, "memory"),
                    OtelKeyValue::new(metric_attributes::OPERATION_TYPE, "update"),
                ],
            );
        }

        process_span.add_event("Event processing complete".to_string(), vec![]);

        // Record processing metrics
        recorder.record_counter(
            "demo",
            metric_names::EVENTS_PROCESSED,
            1,
            vec![OtelKeyValue::new(metric_attributes::EVENT_TYPE, "metric")],
        );

        recorder.record_histogram(
            "demo",
            metric_names::PROCESSING_DURATION,
            35.0,
            vec![OtelKeyValue::new(
                metric_attributes::OPERATION_TYPE,
                "process",
            )],
        );
    }

    println!("   ✓ Processed 5 events with full telemetry");
}
