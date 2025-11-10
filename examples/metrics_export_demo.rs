//! Comprehensive metrics export demonstration
//!
//! This example demonstrates the complete metrics and telemetry infrastructure:
//! - Prometheus metrics collection
//! - OpenTelemetry tracing
//! - HTTP metrics server
//! - Distributed context propagation
//!
//! # Running the Example
//!
//! ```bash
//! # Start the metrics server
//! cargo run --example metrics_export_demo
//!
//! # In another terminal, query metrics
//! curl http://localhost:9090/metrics
//! curl http://localhost:9090/health
//! curl http://localhost:9090/ready
//! ```
//!
//! # Prerequisites
//!
//! For full OpenTelemetry integration, you'll need:
//! - Prometheus (for scraping metrics)
//! - Jaeger or similar (for trace visualization)
//! - OpenTelemetry Collector (optional, for OTLP export)
//!
//! ## Quick Start with Docker
//!
//! ```bash
//! # Start observability stack
//! docker-compose -f monitoring/docker-compose.yml up -d
//!
//! # Access dashboards
//! open http://localhost:3000  # Grafana
//! open http://localhost:9090  # Prometheus
//! open http://localhost:16686 # Jaeger
//! ```

use processor::metrics::{
    labels::{BackendLabel, OperationLabel, ResultLabel, WindowTypeLabel},
    MetricsRegistry, MetricsServer, MetricsServerConfig, ProcessorMetrics,
};
use processor::telemetry::{
    context_with_span, init_telemetry, record_exception, SpanBuilder, TelemetryConfig,
};
use opentelemetry::trace::{Span, SpanKind, Status, Tracer};
use opentelemetry::{global, KeyValue};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing subscriber for logs
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    info!("🚀 Starting Metrics Export Demo");

    // 1. Initialize OpenTelemetry (optional - configure endpoint if you have collector)
    let telemetry_config = TelemetryConfig::new("llm-auto-optimizer-demo")
        .with_otlp_endpoint("http://localhost:4317")
        .with_service_version("1.0.0-demo")
        .with_environment("development")
        .with_tracing_enabled(true)
        .with_metrics_enabled(true);

    let _providers = match init_telemetry(telemetry_config) {
        Ok(providers) => {
            info!("✅ OpenTelemetry initialized successfully");
            Some(providers)
        }
        Err(e) => {
            warn!("⚠️  OpenTelemetry initialization failed ({}), continuing with Prometheus only", e);
            None
        }
    };

    // 2. Initialize Prometheus metrics
    let registry = MetricsRegistry::global();
    let mut reg = registry.registry().write();
    let metrics = Arc::new(ProcessorMetrics::new(&mut reg));
    drop(reg); // Release write lock

    info!("✅ Prometheus metrics registry initialized");

    // 3. Start metrics HTTP server
    let server_config = MetricsServerConfig::new("0.0.0.0", 9090);
    let metrics_server = MetricsServer::with_registry(
        server_config.clone(),
        registry.clone()
    );

    // Spawn server in background
    let server_handle = tokio::spawn(async move {
        info!("🌐 Starting metrics server on http://{}:{}",
            server_config.bind_address,
            server_config.port
        );
        if let Err(e) = metrics_server.start().await {
            error!("❌ Metrics server error: {}", e);
        }
    });

    // Give server time to start
    sleep(Duration::from_millis(500)).await;

    info!("📊 Metrics server ready:");
    info!("   - Metrics: http://localhost:9090/metrics");
    info!("   - Health:  http://localhost:9090/health");
    info!("   - Ready:   http://localhost:9090/ready");
    info!("");

    // 4. Simulate processing workload
    info!("🔄 Simulating stream processing workload...");
    simulate_stream_processing(metrics.clone()).await;

    info!("");
    info!("🔍 Simulating state backend operations...");
    simulate_state_operations(metrics.clone()).await;

    info!("");
    info!("🪟 Simulating window operations...");
    simulate_window_operations(metrics.clone()).await;

    info!("");
    info!("🔬 Demonstrating OpenTelemetry tracing...");
    demonstrate_tracing().await;

    info!("");
    info!("✅ Demo complete! Metrics are available at:");
    info!("   curl http://localhost:9090/metrics");
    info!("");
    info!("📈 Sample queries:");
    info!("   # Event throughput");
    info!("   rate(stream_processor_events_received_total[1m])");
    info!("");
    info!("   # Processing latency p95");
    info!("   histogram_quantile(0.95, rate(stream_processor_event_processing_duration_seconds_bucket[5m]))");
    info!("");
    info!("   # Error rate");
    info!("   rate(stream_processor_events_processed_total{{result=\"error\"}}[5m])");
    info!("");
    info!("Press Ctrl+C to exit...");

    // Keep server running
    server_handle.await?;

    Ok(())
}

/// Simulate stream processing events
async fn simulate_stream_processing(metrics: Arc<ProcessorMetrics>) {
    let event_types = ["metric", "feedback", "configuration"];

    for i in 0..100 {
        let start = Instant::now();

        // Record event received
        metrics.record_event_received();

        // Simulate processing with varying latency
        let processing_time = if i % 10 == 0 {
            // Occasionally slow event
            sleep(Duration::from_millis(50)).await;
            0.050
        } else {
            sleep(Duration::from_millis(5)).await;
            0.005
        };

        // Record processing result
        let result = if i % 20 == 0 {
            // Occasional error
            ResultLabel::Error
        } else {
            ResultLabel::Success
        };

        metrics.record_event_processed(result);
        metrics.record_event_processing_duration(
            start.elapsed().as_secs_f64(),
            result
        );

        // Update pipeline metrics
        metrics.set_pipeline_lag(i as f64 * 0.01);
        metrics.set_backpressure_events((100 - i) as i64);

        if i % 25 == 0 {
            info!("  📦 Processed {} events", i);
        }
    }

    info!("  ✅ Processed 100 events");
    info!("     - Success: {}", metrics.events_received_total.get());
    info!("     - Errors:  ~5 (simulated)");
}

/// Simulate state backend operations
async fn simulate_state_operations(metrics: Arc<ProcessorMetrics>) {
    let backends = [BackendLabel::Redis, BackendLabel::InMemory, BackendLabel::Postgres];
    let operations = [OperationLabel::Get, OperationLabel::Put, OperationLabel::Delete];

    for i in 0..50 {
        let backend = &backends[i % backends.len()];
        let operation = &operations[i % operations.len()];

        // Simulate operation latency (Redis > Postgres > InMemory)
        let latency = match backend {
            BackendLabel::InMemory => 0.0001, // 0.1ms
            BackendLabel::Redis => 0.002,     // 2ms
            BackendLabel::Postgres => 0.005,  // 5ms
            _ => 0.001,
        };

        sleep(Duration::from_millis((latency * 1000.0) as u64)).await;
        metrics.record_state_operation(*operation, *backend, latency);

        // Simulate cache hits/misses
        if i % 3 == 0 {
            use processor::metrics::labels::CacheLayerLabel;
            metrics.record_state_cache_hit(CacheLayerLabel::L1);
        } else {
            use processor::metrics::labels::CacheLayerLabel;
            metrics.record_state_cache_miss(CacheLayerLabel::L1);
        }

        // Update state size
        metrics.set_state_size_bytes(*backend, (i as i64 + 1) * 1024 * 100);
    }

    info!("  ✅ Completed 50 state operations");
    info!("     - Cache hit rate: ~33%");
}

/// Simulate window operations
async fn simulate_window_operations(metrics: Arc<ProcessorMetrics>) {
    let window_types = [
        WindowTypeLabel::Tumbling,
        WindowTypeLabel::Sliding,
        WindowTypeLabel::Session,
    ];

    for i in 0..30 {
        let window_type = &window_types[i % window_types.len()];

        // Create window
        metrics.record_window_created(*window_type);
        metrics.set_windows_active(*window_type, (i % 5 + 1) as i64);

        // Simulate window filling
        sleep(Duration::from_millis(20)).await;

        // Record window size
        let size = 50.0 + (i as f64 * 5.0);
        metrics.record_window_size(*window_type, size);

        // Trigger window
        if i % 3 == 0 {
            metrics.record_window_triggered(*window_type);

            // Record aggregation
            use processor::metrics::labels::AggregationTypeLabel;
            metrics.record_aggregation_computed(
                AggregationTypeLabel::Sum,
                0.001
            );
        }

        // Occasional late event
        if i % 7 == 0 {
            metrics.record_late_event();
        }
    }

    info!("  ✅ Processed 30 window operations");
    info!("     - Windows created: 30");
    info!("     - Windows triggered: 10");
    info!("     - Late events: 4");
}

/// Demonstrate OpenTelemetry distributed tracing
async fn demonstrate_tracing() {
    let tracer = global::tracer("llm-auto-optimizer-demo");

    // Create parent span for the entire operation
    let parent_span = SpanBuilder::new("process_optimization_request")
        .with_kind(SpanKind::Server)
        .with_attribute("request.id", "demo-request-123")
        .with_attribute("user.id", "user-456")
        .start();

    let parent_context = context_with_span(parent_span);
    let _parent_guard = parent_context.attach();

    info!("  📍 Parent span: process_optimization_request");

    // Simulate multiple processing stages
    simulate_llm_call(&tracer).await;
    simulate_analysis(&tracer).await;

    // Simulate an error in one stage
    if let Err(e) = simulate_error_stage(&tracer).await {
        error!("  ⚠️  Error occurred: {}", e);
    }

    simulate_decision(&tracer).await;

    drop(_parent_guard);
    info!("  ✅ Trace complete - check Jaeger UI for visualization");
}

async fn simulate_llm_call(tracer: &impl Tracer) {
    let span = tracer
        .span_builder("call_llm_provider")
        .with_kind(SpanKind::Client)
        .with_attributes(vec![
            KeyValue::new("llm.provider", "openai"),
            KeyValue::new("llm.model", "gpt-4"),
            KeyValue::new("llm.tokens", 150),
        ])
        .start(tracer);

    let _guard = context_with_span(span).attach();

    // Simulate LLM API call
    sleep(Duration::from_millis(200)).await;
    info!("    └─ call_llm_provider (200ms)");
}

async fn simulate_analysis(tracer: &impl Tracer) {
    let span = tracer
        .span_builder("analyze_response")
        .with_kind(SpanKind::Internal)
        .with_attributes(vec![
            KeyValue::new("analysis.type", "performance"),
            KeyValue::new("metrics.count", 5),
        ])
        .start(tracer);

    let _guard = context_with_span(span).attach();

    sleep(Duration::from_millis(50)).await;
    info!("    └─ analyze_response (50ms)");
}

async fn simulate_error_stage(tracer: &impl Tracer) -> Result<(), Box<dyn std::error::Error>> {
    let mut span = tracer
        .span_builder("validate_constraints")
        .with_kind(SpanKind::Internal)
        .start(tracer);

    let _guard = context_with_span(span.clone()).attach();

    // Simulate error
    let error = std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "Validation constraint failed: cost threshold exceeded"
    );

    // Record the exception in the span
    record_exception(&mut span, &error);
    span.set_status(Status::error("Validation failed"));

    info!("    └─ validate_constraints (error)");
    Err(Box::new(error))
}

async fn simulate_decision(tracer: &impl Tracer) {
    let span = tracer
        .span_builder("make_optimization_decision")
        .with_kind(SpanKind::Internal)
        .with_attributes(vec![
            KeyValue::new("decision.strategy", "ml_based"),
            KeyValue::new("decision.confidence", 0.85),
        ])
        .start(tracer);

    let _guard = context_with_span(span).attach();

    sleep(Duration::from_millis(30)).await;
    info!("    └─ make_optimization_decision (30ms)");
}
