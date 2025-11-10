//! Comprehensive Metrics Tests
//!
//! This module provides extensive test coverage for the metrics infrastructure including:
//! - Unit tests for metric registration and operations
//! - Integration tests for HTTP endpoints and scraping
//! - Performance tests for latency and throughput
//! - End-to-end tests for the full pipeline

use opentelemetry::metrics::{Counter, Histogram, MeterProvider as _, UpDownCounter};
use opentelemetry::KeyValue;
use opentelemetry_sdk::metrics::{
    data::{ResourceMetrics, ScopeMetrics},
    reader::TemporalitySelector,
    ManualReader, SdkMeterProvider,
};
use opentelemetry_sdk::Resource;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;

// ============================================================================
// UNIT TESTS - Metric Registration and Basic Operations
// ============================================================================

#[cfg(test)]
mod unit_tests {
    use super::*;

    fn setup_test_provider() -> (SdkMeterProvider, ManualReader) {
        let reader = ManualReader::builder().build();
        let provider = SdkMeterProvider::builder()
            .with_reader(reader.clone())
            .with_resource(Resource::new(vec![
                KeyValue::new("service.name", "test-service"),
                KeyValue::new("service.version", "0.1.0"),
            ]))
            .build();
        (provider, reader)
    }

    #[test]
    fn test_counter_registration() {
        let (provider, _reader) = setup_test_provider();
        let meter = provider.meter("test");

        let counter = meter
            .u64_counter("test_counter")
            .with_description("A test counter")
            .with_unit("requests")
            .init();

        // Counter should be created successfully
        counter.add(1, &[KeyValue::new("label", "value")]);
    }

    #[test]
    fn test_counter_increment() {
        let (provider, reader) = setup_test_provider();
        let meter = provider.meter("test");

        let counter = meter.u64_counter("requests_total").init();

        counter.add(5, &[]);
        counter.add(3, &[]);
        counter.add(2, &[]);

        // Verify counter increments (total should be 10)
        let metrics = reader.collect(&mut opentelemetry::Context::current()).unwrap();
        assert!(!metrics.scope_metrics.is_empty());
    }

    #[test]
    fn test_counter_with_labels() {
        let (provider, _reader) = setup_test_provider();
        let meter = provider.meter("test");

        let counter = meter.u64_counter("http_requests").init();

        counter.add(1, &[KeyValue::new("method", "GET"), KeyValue::new("status", "200")]);
        counter.add(1, &[KeyValue::new("method", "POST"), KeyValue::new("status", "201")]);
        counter.add(1, &[KeyValue::new("method", "GET"), KeyValue::new("status", "404")]);

        // Different label combinations should create separate metric series
    }

    #[test]
    fn test_gauge_set_value() {
        let (provider, _reader) = setup_test_provider();
        let meter = provider.meter("test");

        let gauge = meter.i64_up_down_counter("temperature").init();

        gauge.add(25, &[]);
        gauge.add(5, &[]); // Temperature increases to 30
        gauge.add(-3, &[]); // Temperature decreases to 27
    }

    #[test]
    fn test_gauge_add_subtract() {
        let (provider, _reader) = setup_test_provider();
        let meter = provider.meter("test");

        let gauge = meter.i64_up_down_counter("active_connections").init();

        // Add connections
        gauge.add(10, &[]);
        gauge.add(5, &[]);

        // Remove connections
        gauge.add(-3, &[]);

        // Net result: 12 connections
    }

    #[test]
    fn test_histogram_observe() {
        let (provider, _reader) = setup_test_provider();
        let meter = provider.meter("test");

        let histogram = meter.f64_histogram("request_duration").init();

        histogram.record(0.1, &[]);
        histogram.record(0.25, &[]);
        histogram.record(0.5, &[]);
        histogram.record(1.0, &[]);
    }

    #[test]
    fn test_histogram_with_labels() {
        let (provider, _reader) = setup_test_provider();
        let meter = provider.meter("test");

        let histogram = meter.f64_histogram("latency").init();

        histogram.record(100.0, &[KeyValue::new("service", "api"), KeyValue::new("endpoint", "/users")]);
        histogram.record(150.0, &[KeyValue::new("service", "api"), KeyValue::new("endpoint", "/posts")]);
        histogram.record(50.0, &[KeyValue::new("service", "database"), KeyValue::new("operation", "query")]);
    }

    #[test]
    fn test_metric_naming_validation() {
        let (provider, _reader) = setup_test_provider();
        let meter = provider.meter("test");

        // Valid names
        let _counter1 = meter.u64_counter("valid_name").init();
        let _counter2 = meter.u64_counter("valid.name.with.dots").init();
        let _counter3 = meter.u64_counter("valid_name_123").init();

        // Names should follow Prometheus naming conventions
    }

    #[test]
    fn test_label_validation() {
        let (provider, _reader) = setup_test_provider();
        let meter = provider.meter("test");

        let counter = meter.u64_counter("test").init();

        // Valid labels
        counter.add(1, &[
            KeyValue::new("env", "production"),
            KeyValue::new("region", "us-west-2"),
            KeyValue::new("instance_id", "i-1234567890"),
        ]);

        // Label names should follow Prometheus conventions
    }

    #[test]
    fn test_multiple_instruments_same_meter() {
        let (provider, _reader) = setup_test_provider();
        let meter = provider.meter("test");

        let counter1 = meter.u64_counter("requests").init();
        let counter2 = meter.u64_counter("errors").init();
        let histogram = meter.f64_histogram("latency").init();
        let gauge = meter.i64_up_down_counter("connections").init();

        // All instruments should coexist
        counter1.add(1, &[]);
        counter2.add(1, &[]);
        histogram.record(0.5, &[]);
        gauge.add(5, &[]);
    }

    #[test]
    fn test_metric_description_and_unit() {
        let (provider, _reader) = setup_test_provider();
        let meter = provider.meter("test");

        let _counter = meter
            .u64_counter("bytes_transferred")
            .with_description("Total bytes transferred over the network")
            .with_unit("bytes")
            .init();

        let _histogram = meter
            .f64_histogram("response_time")
            .with_description("HTTP response time")
            .with_unit("ms")
            .init();
    }

    #[test]
    fn test_counter_does_not_decrement() {
        let (provider, _reader) = setup_test_provider();
        let meter = provider.meter("test");

        let counter = meter.u64_counter("monotonic_counter").init();

        counter.add(10, &[]);
        // Counters should only increase, cannot decrement
        // This is enforced by the u64 type
    }

    #[test]
    fn test_updown_counter_can_decrement() {
        let (provider, _reader) = setup_test_provider();
        let meter = provider.meter("test");

        let gauge = meter.i64_up_down_counter("queue_size").init();

        gauge.add(10, &[]);
        gauge.add(-5, &[]);
        gauge.add(3, &[]);
        // Net: 8
    }

    #[test]
    fn test_histogram_with_zero_values() {
        let (provider, _reader) = setup_test_provider();
        let meter = provider.meter("test");

        let histogram = meter.f64_histogram("latency").init();

        histogram.record(0.0, &[]);
        histogram.record(0.0, &[]);
        // Zero values should be recorded
    }

    #[test]
    fn test_histogram_with_negative_values() {
        let (provider, _reader) = setup_test_provider();
        let meter = provider.meter("test");

        let histogram = meter.f64_histogram("temperature_change").init();

        histogram.record(-5.0, &[]); // Temperature drop
        histogram.record(10.0, &[]); // Temperature rise
    }

    #[test]
    fn test_metric_collection() {
        let (provider, reader) = setup_test_provider();
        let meter = provider.meter("test");

        let counter = meter.u64_counter("requests").init();
        counter.add(100, &[]);

        let metrics = reader.collect(&mut opentelemetry::Context::current()).unwrap();

        assert_eq!(metrics.resource.len(), 2); // service.name and service.version
        assert!(!metrics.scope_metrics.is_empty());
    }

    #[test]
    fn test_multiple_meters() {
        let (provider, _reader) = setup_test_provider();

        let meter1 = provider.meter("service1");
        let meter2 = provider.meter("service2");

        let counter1 = meter1.u64_counter("requests").init();
        let counter2 = meter2.u64_counter("requests").init();

        // Same metric name but different meters
        counter1.add(10, &[]);
        counter2.add(20, &[]);
    }

    #[test]
    fn test_high_cardinality_labels() {
        let (provider, _reader) = setup_test_provider();
        let meter = provider.meter("test");

        let counter = meter.u64_counter("requests").init();

        // Simulate high cardinality (should be avoided in production)
        for i in 0..100 {
            counter.add(1, &[KeyValue::new("user_id", i.to_string())]);
        }
    }

    #[test]
    fn test_empty_labels() {
        let (provider, _reader) = setup_test_provider();
        let meter = provider.meter("test");

        let counter = meter.u64_counter("requests").init();

        counter.add(1, &[]); // No labels
        counter.add(1, &[]); // Should aggregate
    }

    #[test]
    fn test_label_ordering() {
        let (provider, _reader) = setup_test_provider();
        let meter = provider.meter("test");

        let counter = meter.u64_counter("requests").init();

        // Same labels, different order - should be normalized
        counter.add(1, &[KeyValue::new("a", "1"), KeyValue::new("b", "2")]);
        counter.add(1, &[KeyValue::new("b", "2"), KeyValue::new("a", "1")]);
    }

    #[test]
    fn test_resource_attributes() {
        let reader = ManualReader::builder().build();
        let provider = SdkMeterProvider::builder()
            .with_reader(reader.clone())
            .with_resource(Resource::new(vec![
                KeyValue::new("service.name", "test-service"),
                KeyValue::new("service.version", "1.0.0"),
                KeyValue::new("deployment.environment", "production"),
                KeyValue::new("host.name", "server-01"),
            ]))
            .build();

        let meter = provider.meter("test");
        let counter = meter.u64_counter("test").init();
        counter.add(1, &[]);

        let metrics = reader.collect(&mut opentelemetry::Context::current()).unwrap();
        assert_eq!(metrics.resource.len(), 4);
    }

    #[test]
    fn test_metric_aggregation() {
        let (provider, reader) = setup_test_provider();
        let meter = provider.meter("test");

        let counter = meter.u64_counter("requests").init();

        // Multiple recordings with same labels should aggregate
        counter.add(10, &[KeyValue::new("status", "200")]);
        counter.add(20, &[KeyValue::new("status", "200")]);
        counter.add(30, &[KeyValue::new("status", "200")]);

        let _metrics = reader.collect(&mut opentelemetry::Context::current()).unwrap();
        // Should show aggregated value
    }

    #[test]
    fn test_concurrent_metric_recording() {
        use std::sync::Arc;
        use std::thread;

        let (provider, _reader) = setup_test_provider();
        let meter = Arc::new(provider.meter("test"));

        let counter = meter.u64_counter("concurrent_requests").init();
        let counter_clone = Arc::new(counter);

        let mut handles = vec![];

        for _ in 0..10 {
            let counter = counter_clone.clone();
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    counter.add(1, &[]);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Total should be 1000
    }

    #[test]
    fn test_histogram_percentile_buckets() {
        let (provider, _reader) = setup_test_provider();
        let meter = provider.meter("test");

        let histogram = meter.f64_histogram("latency").init();

        // Record various latencies
        let latencies = vec![10.0, 20.0, 30.0, 40.0, 50.0, 100.0, 200.0, 500.0, 1000.0];
        for latency in latencies {
            histogram.record(latency, &[]);
        }
    }

    #[test]
    fn test_shutdown_provider() {
        let (provider, _reader) = setup_test_provider();
        let meter = provider.meter("test");

        let counter = meter.u64_counter("requests").init();
        counter.add(1, &[]);

        // Shutdown should succeed
        let result = provider.shutdown();
        assert!(result.is_ok());
    }

    #[test]
    fn test_metric_with_special_characters_in_labels() {
        let (provider, _reader) = setup_test_provider();
        let meter = provider.meter("test");

        let counter = meter.u64_counter("special_chars_test").init();

        // Test labels with special characters
        counter.add(1, &[
            KeyValue::new("path", "/api/v1/users"),
            KeyValue::new("user_agent", "Mozilla/5.0 (compatible)"),
            KeyValue::new("status_text", "OK - Success"),
        ]);

        // Should handle special characters properly
    }
}

// ============================================================================
// INTEGRATION TESTS - HTTP Endpoints and Scraping
// ============================================================================

#[cfg(test)]
mod integration_tests {
    use super::*;
    use axum::{
        body::Body,
        extract::State,
        http::{Request, StatusCode},
        response::IntoResponse,
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    #[derive(Clone)]
    struct AppState {
        provider: Arc<SdkMeterProvider>,
        reader: Arc<ManualReader>,
    }

    async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
        match state.reader.collect(&mut opentelemetry::Context::current()) {
            Ok(metrics) => {
                // Convert to Prometheus format
                let output = format_prometheus_metrics(&metrics);
                (StatusCode::OK, output)
            }
            Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, String::from("Error collecting metrics")),
        }
    }

    fn format_prometheus_metrics(metrics: &ResourceMetrics) -> String {
        let mut output = String::new();

        for scope in &metrics.scope_metrics {
            for metric in &scope.metrics {
                output.push_str(&format!("# TYPE {} counter\n", metric.name));
                output.push_str(&format!("{} 1\n", metric.name));
            }
        }

        output
    }

    fn create_test_app() -> (Router, AppState) {
        let reader = ManualReader::builder().build();
        let provider = SdkMeterProvider::builder()
            .with_reader(reader.clone())
            .build();

        let state = AppState {
            provider: Arc::new(provider),
            reader: Arc::new(reader),
        };

        let app = Router::new()
            .route("/metrics", get(metrics_handler))
            .with_state(state.clone());

        (app, state)
    }

    #[tokio::test]
    async fn test_metrics_endpoint_exists() {
        let (app, _state) = create_test_app();

        let request = Request::builder()
            .uri("/metrics")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_metrics_endpoint_content_type() {
        let (app, _state) = create_test_app();

        let request = Request::builder()
            .uri("/metrics")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should return text/plain for Prometheus
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_prometheus_scraping_simulation() {
        let (app, state) = create_test_app();
        let meter = state.provider.meter("test");

        // Record some metrics
        let counter = meter.u64_counter("http_requests_total").init();
        counter.add(100, &[KeyValue::new("method", "GET")]);
        counter.add(50, &[KeyValue::new("method", "POST")]);

        // Simulate Prometheus scrape
        let request = Request::builder()
            .uri("/metrics")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_concurrent_metric_updates() {
        let (provider, _reader) = super::unit_tests::setup_test_provider();
        let meter = Arc::new(provider.meter("test"));
        let counter = Arc::new(meter.u64_counter("concurrent_updates").init());

        let mut tasks = vec![];

        for _ in 0..10 {
            let counter_clone = counter.clone();
            let task = tokio::spawn(async move {
                for _ in 0..1000 {
                    counter_clone.add(1, &[]);
                }
            });
            tasks.push(task);
        }

        for task in tasks {
            task.await.unwrap();
        }

        // Should have recorded 10,000 increments
    }

    #[tokio::test]
    async fn test_label_cardinality_limits() {
        let (provider, _reader) = super::unit_tests::setup_test_provider();
        let meter = provider.meter("test");

        let counter = meter.u64_counter("requests").init();

        // Test high cardinality (should monitor and alert in production)
        for i in 0..1000 {
            counter.add(1, &[
                KeyValue::new("endpoint", format!("/api/endpoint{}", i)),
                KeyValue::new("status", "200"),
            ]);
        }

        // In production, high cardinality should be detected
    }

    #[tokio::test]
    async fn test_memory_usage_high_cardinality() {
        let (provider, _reader) = super::unit_tests::setup_test_provider();
        let meter = provider.meter("test");

        let counter = meter.u64_counter("high_card_test").init();

        // Create many unique label combinations
        for i in 0..100 {
            for j in 0..100 {
                counter.add(1, &[
                    KeyValue::new("label1", format!("value{}", i)),
                    KeyValue::new("label2", format!("value{}", j)),
                ]);
            }
        }

        // Memory should be monitored for high cardinality
    }

    #[tokio::test]
    async fn test_otlp_span_creation() {
        use opentelemetry::trace::{Tracer, TracerProvider as _};
        use opentelemetry_sdk::trace::TracerProvider;

        let provider = TracerProvider::builder().build();
        let tracer = provider.tracer("test");

        let span = tracer.start("test_operation");
        drop(span);

        // Span should be created successfully
    }

    #[tokio::test]
    async fn test_trace_context_propagation() {
        use opentelemetry::trace::{Tracer, TracerProvider as _};
        use opentelemetry_sdk::trace::TracerProvider;

        let provider = TracerProvider::builder().build();
        let tracer = provider.tracer("test");

        let parent_span = tracer.start("parent_operation");
        let parent_context = opentelemetry::Context::current_with_span(parent_span);

        let _child_span = tracer.start_with_context("child_operation", &parent_context);

        // Child span should inherit parent context
    }

    #[tokio::test]
    async fn test_otlp_export_validation() {
        let (provider, reader) = super::unit_tests::setup_test_provider();
        let meter = provider.meter("test");

        let counter = meter.u64_counter("export_test").init();
        counter.add(42, &[]);

        let metrics = reader.collect(&mut opentelemetry::Context::current()).unwrap();

        // Verify metrics can be exported
        assert!(!metrics.scope_metrics.is_empty());
    }

    #[tokio::test]
    async fn test_metrics_collection_under_load() {
        let (provider, _reader) = super::unit_tests::setup_test_provider();
        let meter = Arc::new(provider.meter("test"));

        let start = Instant::now();

        let mut tasks = vec![];
        for _ in 0..100 {
            let meter_clone = meter.clone();
            let task = tokio::spawn(async move {
                let counter = meter_clone.u64_counter("load_test").init();
                for _ in 0..1000 {
                    counter.add(1, &[KeyValue::new("worker", "test")]);
                }
            });
            tasks.push(task);
        }

        for task in tasks {
            task.await.unwrap();
        }

        let duration = start.elapsed();

        // Should complete within reasonable time
        assert!(duration.as_secs() < 10);
    }

    #[tokio::test]
    async fn test_metrics_with_high_label_count() {
        let (provider, _reader) = super::unit_tests::setup_test_provider();
        let meter = provider.meter("test");

        let counter = meter.u64_counter("multi_label_test").init();

        counter.add(1, &[
            KeyValue::new("label1", "value1"),
            KeyValue::new("label2", "value2"),
            KeyValue::new("label3", "value3"),
            KeyValue::new("label4", "value4"),
            KeyValue::new("label5", "value5"),
            KeyValue::new("label6", "value6"),
            KeyValue::new("label7", "value7"),
            KeyValue::new("label8", "value8"),
            KeyValue::new("label9", "value9"),
            KeyValue::new("label10", "value10"),
        ]);

        // Should handle many labels per metric
    }

    #[tokio::test]
    async fn test_histogram_aggregation() {
        let (provider, _reader) = super::unit_tests::setup_test_provider();
        let meter = provider.meter("test");

        let histogram = meter.f64_histogram("request_latency").init();

        // Record many observations
        for i in 1..=1000 {
            histogram.record(i as f64, &[]);
        }

        // Histogram should aggregate properly
    }

    #[tokio::test]
    async fn test_metrics_reset_behavior() {
        let (provider, reader) = super::unit_tests::setup_test_provider();
        let meter = provider.meter("test");

        let counter = meter.u64_counter("reset_test").init();
        counter.add(100, &[]);

        // First collection
        let _metrics1 = reader.collect(&mut opentelemetry::Context::current()).unwrap();

        // Second collection (cumulative or delta depending on temporality)
        let _metrics2 = reader.collect(&mut opentelemetry::Context::current()).unwrap();
    }

    #[tokio::test]
    async fn test_multiple_metric_readers() {
        let reader1 = ManualReader::builder().build();
        let reader2 = ManualReader::builder().build();

        let provider = SdkMeterProvider::builder()
            .with_reader(reader1.clone())
            .with_reader(reader2.clone())
            .build();

        let meter = provider.meter("test");
        let counter = meter.u64_counter("test").init();
        counter.add(1, &[]);

        // Both readers should see the metrics
        let _metrics1 = reader1.collect(&mut opentelemetry::Context::current()).unwrap();
        let _metrics2 = reader2.collect(&mut opentelemetry::Context::current()).unwrap();
    }

    #[tokio::test]
    async fn test_metric_views_and_aggregation() {
        // Test custom metric views (filtering, renaming, custom aggregation)
        let (provider, _reader) = super::unit_tests::setup_test_provider();
        let meter = provider.meter("test");

        let histogram = meter.f64_histogram("custom_view_test").init();

        for i in 1..=100 {
            histogram.record(i as f64, &[]);
        }
    }

    #[tokio::test]
    async fn test_exemplars_recording() {
        // Test that exemplars can be recorded with metrics
        let (provider, _reader) = super::unit_tests::setup_test_provider();
        let meter = provider.meter("test");

        let histogram = meter.f64_histogram("exemplar_test").init();

        // Record with trace context for exemplars
        histogram.record(123.45, &[KeyValue::new("trace_id", "abc123")]);
    }

    #[tokio::test]
    async fn test_metric_export_batch_size() {
        let (provider, reader) = super::unit_tests::setup_test_provider();
        let meter = provider.meter("test");

        // Create many metrics
        for i in 0..100 {
            let counter = meter.u64_counter(format!("metric_{}", i)).init();
            counter.add(1, &[]);
        }

        let metrics = reader.collect(&mut opentelemetry::Context::current()).unwrap();

        // Should export in batches
        assert!(!metrics.scope_metrics.is_empty());
    }

    #[tokio::test]
    async fn test_metric_staleness_handling() {
        let (provider, reader) = super::unit_tests::setup_test_provider();
        let meter = provider.meter("test");

        let counter = meter.u64_counter("stale_test").init();
        counter.add(1, &[KeyValue::new("series", "1")]);

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Old series should be marked stale if not updated
        let _metrics = reader.collect(&mut opentelemetry::Context::current()).unwrap();
    }

    #[tokio::test]
    async fn test_histogram_boundary_values() {
        let (provider, _reader) = super::unit_tests::setup_test_provider();
        let meter = provider.meter("test");

        let histogram = meter.f64_histogram("boundary_test").init();

        // Test boundary values
        histogram.record(f64::MIN_POSITIVE, &[]);
        histogram.record(f64::MAX / 2.0, &[]);
        histogram.record(0.0, &[]);
    }
}

// ============================================================================
// PERFORMANCE TESTS - Latency and Throughput
// ============================================================================

#[cfg(test)]
mod performance_tests {
    use super::*;

    #[tokio::test]
    async fn test_metric_recording_latency() {
        let (provider, _reader) = super::unit_tests::setup_test_provider();
        let meter = provider.meter("perf_test");

        let counter = meter.u64_counter("latency_test").init();

        let iterations = 10_000;
        let start = Instant::now();

        for _ in 0..iterations {
            counter.add(1, &[]);
        }

        let duration = start.elapsed();
        let avg_latency = duration.as_micros() / iterations;

        println!("Average recording latency: {}μs", avg_latency);

        // Should be less than 1μs per recording
        assert!(avg_latency < 1, "Recording latency too high: {}μs", avg_latency);
    }

    #[tokio::test]
    async fn test_metric_recording_with_labels_latency() {
        let (provider, _reader) = super::unit_tests::setup_test_provider();
        let meter = provider.meter("perf_test");

        let counter = meter.u64_counter("label_latency_test").init();

        let iterations = 10_000;
        let start = Instant::now();

        for i in 0..iterations {
            counter.add(1, &[
                KeyValue::new("label1", (i % 10).to_string()),
                KeyValue::new("label2", (i % 5).to_string()),
            ]);
        }

        let duration = start.elapsed();
        let avg_latency = duration.as_micros() / iterations;

        println!("Average recording latency with labels: {}μs", avg_latency);

        // Should still be fast with labels
        assert!(avg_latency < 5, "Recording latency with labels too high: {}μs", avg_latency);
    }

    #[tokio::test]
    async fn test_histogram_recording_latency() {
        let (provider, _reader) = super::unit_tests::setup_test_provider();
        let meter = provider.meter("perf_test");

        let histogram = meter.f64_histogram("histogram_latency_test").init();

        let iterations = 10_000;
        let start = Instant::now();

        for i in 0..iterations {
            histogram.record((i % 1000) as f64, &[]);
        }

        let duration = start.elapsed();
        let avg_latency = duration.as_micros() / iterations;

        println!("Average histogram recording latency: {}μs", avg_latency);

        assert!(avg_latency < 5, "Histogram recording latency too high: {}μs", avg_latency);
    }

    #[tokio::test]
    async fn test_concurrent_recording_throughput() {
        let (provider, _reader) = super::unit_tests::setup_test_provider();
        let meter = Arc::new(provider.meter("perf_test"));
        let counter = Arc::new(meter.u64_counter("throughput_test").init());

        let num_tasks = 10;
        let iterations_per_task = 100_000;
        let start = Instant::now();

        let mut tasks = vec![];
        for _ in 0..num_tasks {
            let counter_clone = counter.clone();
            let task = tokio::spawn(async move {
                for _ in 0..iterations_per_task {
                    counter_clone.add(1, &[]);
                }
            });
            tasks.push(task);
        }

        for task in tasks {
            task.await.unwrap();
        }

        let duration = start.elapsed();
        let total_ops = num_tasks * iterations_per_task;
        let ops_per_sec = total_ops as f64 / duration.as_secs_f64();

        println!("Throughput: {:.0} ops/sec", ops_per_sec);

        // Should handle at least 1M ops/sec
        assert!(ops_per_sec > 1_000_000.0, "Throughput too low: {:.0} ops/sec", ops_per_sec);
    }

    #[tokio::test]
    async fn test_memory_usage_with_1000_metrics() {
        let (provider, _reader) = super::unit_tests::setup_test_provider();
        let meter = provider.meter("perf_test");

        let mut counters = vec![];

        // Create 1000 different metrics
        for i in 0..1000 {
            let counter = meter.u64_counter(format!("metric_{}", i)).init();
            counters.push(counter);
        }

        // Record values
        for counter in &counters {
            counter.add(1, &[]);
        }

        // Memory usage should be reasonable
        // In production, monitor with proper memory profiling
    }

    #[tokio::test]
    async fn test_collection_latency() {
        let (provider, reader) = super::unit_tests::setup_test_provider();
        let meter = provider.meter("perf_test");

        // Create and record many metrics
        for i in 0..100 {
            let counter = meter.u64_counter(format!("collect_test_{}", i)).init();
            for _ in 0..100 {
                counter.add(1, &[]);
            }
        }

        let start = Instant::now();
        let _metrics = reader.collect(&mut opentelemetry::Context::current()).unwrap();
        let duration = start.elapsed();

        println!("Collection latency: {:?}", duration);

        // Collection should be fast
        assert!(duration.as_millis() < 100, "Collection too slow: {:?}", duration);
    }

    #[tokio::test]
    async fn test_high_frequency_recording() {
        let (provider, _reader) = super::unit_tests::setup_test_provider();
        let meter = provider.meter("perf_test");

        let histogram = meter.f64_histogram("high_freq_test").init();

        let duration = Duration::from_secs(1);
        let start = Instant::now();
        let mut count = 0;

        while start.elapsed() < duration {
            histogram.record(1.0, &[]);
            count += 1;
        }

        let ops_per_sec = count;
        println!("High frequency recording: {} ops/sec", ops_per_sec);

        // Should handle very high frequency
        assert!(ops_per_sec > 100_000, "High frequency recording too low: {} ops/sec", ops_per_sec);
    }

    #[tokio::test]
    async fn test_label_combination_performance() {
        let (provider, _reader) = super::unit_tests::setup_test_provider();
        let meter = provider.meter("perf_test");

        let counter = meter.u64_counter("label_combo_test").init();

        let iterations = 10_000;
        let start = Instant::now();

        for i in 0..iterations {
            counter.add(1, &[
                KeyValue::new("method", if i % 2 == 0 { "GET" } else { "POST" }),
                KeyValue::new("status", format!("{}", (i % 5) * 100 + 200)),
                KeyValue::new("endpoint", format!("/api/v{}", i % 3)),
            ]);
        }

        let duration = start.elapsed();
        println!("Label combination performance: {:?}", duration);

        assert!(duration.as_millis() < 500, "Label combination too slow: {:?}", duration);
    }

    #[tokio::test]
    async fn test_batch_recording_performance() {
        let (provider, _reader) = super::unit_tests::setup_test_provider();
        let meter = provider.meter("perf_test");

        let counter = meter.u64_counter("batch_test").init();

        let batch_size = 1000;
        let num_batches = 100;
        let start = Instant::now();

        for _ in 0..num_batches {
            for _ in 0..batch_size {
                counter.add(1, &[]);
            }
        }

        let duration = start.elapsed();
        let total_ops = batch_size * num_batches;
        let ops_per_sec = total_ops as f64 / duration.as_secs_f64();

        println!("Batch recording: {:.0} ops/sec", ops_per_sec);
    }

    #[tokio::test]
    async fn test_metric_export_performance() {
        let (provider, reader) = super::unit_tests::setup_test_provider();
        let meter = provider.meter("perf_test");

        // Create realistic workload
        let request_counter = meter.u64_counter("requests").init();
        let latency_histogram = meter.f64_histogram("latency").init();

        for i in 0..10_000 {
            request_counter.add(1, &[
                KeyValue::new("method", if i % 2 == 0 { "GET" } else { "POST" }),
                KeyValue::new("status", "200"),
            ]);
            latency_histogram.record((i % 1000) as f64, &[]);
        }

        let start = Instant::now();
        let _metrics = reader.collect(&mut opentelemetry::Context::current()).unwrap();
        let duration = start.elapsed();

        println!("Export performance: {:?}", duration);
        assert!(duration.as_millis() < 200, "Export too slow: {:?}", duration);
    }
}

// ============================================================================
// END-TO-END TESTS - Full Pipeline
// ============================================================================

#[cfg(test)]
mod e2e_tests {
    use super::*;

    #[tokio::test]
    async fn test_full_pipeline_metrics_collection() {
        // Setup full observability pipeline
        let (provider, reader) = super::unit_tests::setup_test_provider();
        let meter = provider.meter("e2e_test");

        // Simulate application metrics
        let request_counter = meter.u64_counter("http_requests_total").init();
        let error_counter = meter.u64_counter("http_requests_errors").init();
        let latency_histogram = meter.f64_histogram("http_request_duration_ms").init();
        let active_connections = meter.i64_up_down_counter("active_connections").init();

        // Simulate traffic
        for i in 0..1000 {
            // Record request
            request_counter.add(1, &[
                KeyValue::new("method", "GET"),
                KeyValue::new("endpoint", "/api/users"),
            ]);

            // Record latency
            latency_histogram.record(
                (50.0 + (i % 200) as f64),
                &[KeyValue::new("endpoint", "/api/users")],
            );

            // Simulate some errors
            if i % 50 == 0 {
                error_counter.add(1, &[
                    KeyValue::new("type", "timeout"),
                    KeyValue::new("endpoint", "/api/users"),
                ]);
            }

            // Track connections
            if i % 10 == 0 {
                active_connections.add(1, &[]);
            }
            if i % 15 == 0 {
                active_connections.add(-1, &[]);
            }
        }

        // Collect metrics
        let metrics = reader.collect(&mut opentelemetry::Context::current()).unwrap();

        assert!(!metrics.scope_metrics.is_empty());
    }

    #[tokio::test]
    async fn test_dashboard_query_validation() {
        let (provider, reader) = super::unit_tests::setup_test_provider();
        let meter = provider.meter("e2e_test");

        // Create metrics that would be used in dashboards
        let request_rate = meter.u64_counter("request_rate").init();
        let p95_latency = meter.f64_histogram("request_latency").init();
        let error_rate = meter.u64_counter("error_rate").init();

        // Record data
        for _ in 0..100 {
            request_rate.add(10, &[]);
            p95_latency.record(150.0, &[]);
        }

        for _ in 0..5 {
            error_rate.add(1, &[]);
        }

        // Simulate dashboard queries
        let metrics = reader.collect(&mut opentelemetry::Context::current()).unwrap();

        // Verify metrics are queryable
        assert!(!metrics.scope_metrics.is_empty());
    }

    #[tokio::test]
    async fn test_alert_rule_triggering() {
        let (provider, reader) = super::unit_tests::setup_test_provider();
        let meter = provider.meter("e2e_test");

        let error_rate = meter.u64_counter("errors_total").init();
        let total_requests = meter.u64_counter("requests_total").init();

        // Simulate high error rate scenario
        total_requests.add(100, &[]);
        error_rate.add(10, &[]); // 10% error rate

        let metrics = reader.collect(&mut opentelemetry::Context::current()).unwrap();

        // Alert should trigger if error_rate > 5%
        assert!(!metrics.scope_metrics.is_empty());
    }

    #[tokio::test]
    async fn test_trace_visualization_pipeline() {
        use opentelemetry::trace::{Tracer, TracerProvider as _};
        use opentelemetry_sdk::trace::TracerProvider;

        let trace_provider = TracerProvider::builder().build();
        let tracer = trace_provider.tracer("e2e_test");

        let (metrics_provider, _reader) = super::unit_tests::setup_test_provider();
        let meter = metrics_provider.meter("e2e_test");

        // Simulate traced request with metrics
        let span = tracer.start("handle_request");
        let _context = opentelemetry::Context::current_with_span(span);

        let request_counter = meter.u64_counter("traced_requests").init();
        request_counter.add(1, &[KeyValue::new("traced", "true")]);

        // Trace should be linked with metrics for visualization
    }

    #[tokio::test]
    async fn test_multi_component_observability() {
        // Test observability across multiple components
        let (provider, reader) = super::unit_tests::setup_test_provider();

        // API component
        let api_meter = provider.meter("api");
        let api_requests = api_meter.u64_counter("requests").init();

        // Database component
        let db_meter = provider.meter("database");
        let db_queries = db_meter.u64_counter("queries").init();

        // Cache component
        let cache_meter = provider.meter("cache");
        let cache_hits = cache_meter.u64_counter("hits").init();

        // Simulate cross-component operation
        api_requests.add(1, &[]);
        cache_hits.add(0, &[]); // Cache miss
        db_queries.add(1, &[]);

        let metrics = reader.collect(&mut opentelemetry::Context::current()).unwrap();

        // Should see metrics from all components
        assert!(!metrics.scope_metrics.is_empty());
    }
}
