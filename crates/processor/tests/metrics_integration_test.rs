//! Integration tests for Prometheus metrics

use processor::metrics::{
    labels::{BackendLabel, CacheLayerLabel, OperationLabel, ResultLabel, StrategyLabel, WindowTypeLabel},
    MetricsRegistry, MetricsServer, MetricsServerConfig, ProcessorMetrics,
};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_metrics_basic_operations() {
    let registry = MetricsRegistry::new();
    let mut reg = registry.registry().write();
    let metrics = ProcessorMetrics::new(&mut reg);

    // Test event metrics
    metrics.record_event_received();
    metrics.record_event_received();
    metrics.record_event_processed(ResultLabel::Success);
    metrics.record_event_processed(ResultLabel::Error);

    assert_eq!(metrics.events_received_total.get(), 2);

    // Test event processing duration
    metrics.record_event_processing_duration(0.001, ResultLabel::Success);
    metrics.record_event_processing_duration(0.005, ResultLabel::Error);

    // Test pipeline metrics
    metrics.set_pipeline_lag(1.5);
    metrics.set_backpressure_events(100);

    assert_eq!(metrics.pipeline_lag_seconds.get(), 1.5);
    assert_eq!(metrics.backpressure_events.get(), 100);
}

#[tokio::test]
async fn test_window_metrics() {
    let registry = MetricsRegistry::new();
    let mut reg = registry.registry().write();
    let metrics = ProcessorMetrics::new(&mut reg);

    // Create windows
    metrics.record_window_created(WindowTypeLabel::Tumbling);
    metrics.record_window_created(WindowTypeLabel::Sliding);
    metrics.record_window_created(WindowTypeLabel::Session);

    // Trigger windows
    metrics.record_window_triggered(WindowTypeLabel::Tumbling);
    metrics.record_window_triggered(WindowTypeLabel::Tumbling);

    // Set active windows
    metrics.set_windows_active(WindowTypeLabel::Tumbling, 5);
    metrics.set_windows_active(WindowTypeLabel::Sliding, 3);

    // Record window sizes
    metrics.record_window_size(WindowTypeLabel::Tumbling, 100.0);
    metrics.record_window_size(WindowTypeLabel::Tumbling, 150.0);

    // Record late events
    metrics.record_late_event();
    metrics.record_late_event();

    assert_eq!(metrics.late_events_total.get(), 2);
}

#[tokio::test]
async fn test_state_backend_metrics() {
    let registry = MetricsRegistry::new();
    let mut reg = registry.registry().write();
    let metrics = ProcessorMetrics::new(&mut reg);

    // Record state operations
    metrics.record_state_operation(OperationLabel::Get, BackendLabel::Redis, 0.001);
    metrics.record_state_operation(OperationLabel::Put, BackendLabel::Redis, 0.002);
    metrics.record_state_operation(OperationLabel::Get, BackendLabel::InMemory, 0.0001);

    // Record cache hits and misses
    metrics.record_state_cache_hit(CacheLayerLabel::L1);
    metrics.record_state_cache_hit(CacheLayerLabel::L1);
    metrics.record_state_cache_miss(CacheLayerLabel::L1);
    metrics.record_state_cache_hit(CacheLayerLabel::L2);

    // Set state size
    metrics.set_state_size_bytes(BackendLabel::Redis, 1024 * 1024);
    metrics.set_state_size_bytes(BackendLabel::InMemory, 512 * 1024);

    // Verify state size
    let redis_labels = vec![("backend".to_string(), "redis".to_string())];
    assert_eq!(
        metrics.state_size_bytes.get_or_create(&redis_labels).get(),
        1024 * 1024
    );
}

#[tokio::test]
async fn test_deduplication_metrics() {
    let registry = MetricsRegistry::new();
    let mut reg = registry.registry().write();
    let metrics = ProcessorMetrics::new(&mut reg);

    // Check events for duplicates
    metrics.record_dedup_check(false); // Not a duplicate
    metrics.record_dedup_check(true);  // Duplicate
    metrics.record_dedup_check(false); // Not a duplicate
    metrics.record_dedup_check(true);  // Duplicate
    metrics.record_dedup_check(true);  // Duplicate

    assert_eq!(metrics.dedup_events_checked_total.get(), 5);
    assert_eq!(metrics.dedup_duplicates_found_total.get(), 3);

    // Set cache size
    metrics.set_dedup_cache_size(1000);
    assert_eq!(metrics.dedup_cache_size.get(), 1000);

    // Record cache evictions
    metrics.dedup_cache_evictions_total.inc();
    assert_eq!(metrics.dedup_cache_evictions_total.get(), 1);
}

#[tokio::test]
async fn test_normalization_metrics() {
    let registry = MetricsRegistry::new();
    let mut reg = registry.registry().write();
    let metrics = ProcessorMetrics::new(&mut reg);

    // Record normalization events
    metrics.normalization_events_total.inc();
    metrics.normalization_events_total.inc();

    // Record fill operations with different strategies
    metrics.record_normalization_fill(StrategyLabel::Forward);
    metrics.record_normalization_fill(StrategyLabel::Backward);
    metrics.record_normalization_fill(StrategyLabel::Linear);

    // Record gaps filled
    metrics.normalization_gaps_filled_total.inc();
    metrics.normalization_gaps_filled_total.inc();

    // Record outliers
    metrics.normalization_outliers_detected_total.inc();

    assert_eq!(metrics.normalization_events_total.get(), 2);
    assert_eq!(metrics.normalization_gaps_filled_total.get(), 2);
    assert_eq!(metrics.normalization_outliers_detected_total.get(), 1);
}

#[tokio::test]
async fn test_watermark_metrics() {
    let registry = MetricsRegistry::new();
    let mut reg = registry.registry().write();
    let metrics = ProcessorMetrics::new(&mut reg);

    // Set watermark lag
    metrics.set_watermark_lag(2.5);
    assert_eq!(metrics.watermark_lag_seconds.get(), 2.5);

    // Update watermark
    metrics.record_watermark_update();
    metrics.record_watermark_update();
    assert_eq!(metrics.watermark_updates_total.get(), 2);

    // Record out-of-order events
    metrics.record_out_of_order_event();
    metrics.record_out_of_order_event();
    metrics.record_out_of_order_event();
    assert_eq!(metrics.out_of_order_events_total.get(), 3);
}

#[tokio::test]
async fn test_metrics_encoding() {
    let registry = MetricsRegistry::new();
    let mut reg = registry.registry().write();
    let metrics = ProcessorMetrics::new(&mut reg);
    drop(reg); // Release write lock

    // Add some metrics
    metrics.record_event_received();
    metrics.record_event_processed(ResultLabel::Success);
    metrics.set_pipeline_lag(1.0);

    // Encode metrics
    let encoded = registry.encode().expect("Failed to encode metrics");

    // Verify output contains expected metric names
    assert!(encoded.contains("processor_events_received"));
    assert!(encoded.contains("processor_events_processed"));
    assert!(encoded.contains("processor_pipeline_lag_seconds"));
}

#[tokio::test]
async fn test_metrics_server_config() {
    let config = MetricsServerConfig::new("127.0.0.1", 19090);
    assert_eq!(config.bind_address, "127.0.0.1");
    assert_eq!(config.port, 19090);

    let addr = config.socket_addr().expect("Invalid socket address");
    assert_eq!(addr.port(), 19090);
}

#[tokio::test]
async fn test_metrics_snapshot() {
    use processor::metrics::MetricsSnapshot;

    let registry = MetricsRegistry::new();
    let mut reg = registry.registry().write();
    let metrics = ProcessorMetrics::new(&mut reg);
    drop(reg);

    // Record some metrics
    metrics.record_event_received();
    metrics.record_event_received();
    metrics.record_event_processed(ResultLabel::Success);
    metrics.record_dedup_check(true);

    // Capture snapshot
    let snapshot = MetricsSnapshot::capture(&metrics);
    assert_eq!(snapshot.events_received, 2);
    assert_eq!(snapshot.events_processed_success, 1);
    assert_eq!(snapshot.duplicates_found, 1);
}

#[tokio::test]
async fn test_concurrent_metric_updates() {
    let registry = MetricsRegistry::new();
    let mut reg = registry.registry().write();
    let metrics = Arc::new(ProcessorMetrics::new(&mut reg));
    drop(reg);

    // Spawn multiple tasks that update metrics concurrently
    let mut handles = vec![];

    for _ in 0..10 {
        let metrics_clone = Arc::clone(&metrics);
        let handle = tokio::spawn(async move {
            for _ in 0..100 {
                metrics_clone.record_event_received();
                metrics_clone.record_event_processed(ResultLabel::Success);
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.expect("Task panicked");
    }

    // Verify total count (10 tasks * 100 increments each)
    assert_eq!(metrics.events_received_total.get(), 1000);
}

#[tokio::test]
async fn test_histogram_observations() {
    let registry = MetricsRegistry::new();
    let mut reg = registry.registry().write();
    let metrics = ProcessorMetrics::new(&mut reg);

    // Record various durations to test histogram bucketing
    let durations = vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0];

    for duration in durations {
        metrics.record_event_processing_duration(duration, ResultLabel::Success);
    }

    // The histogram should have observed all values
    // (Specific bucket verification would require direct access to histogram internals)
}

#[tokio::test]
async fn test_label_families() {
    let registry = MetricsRegistry::new();
    let mut reg = registry.registry().write();
    let metrics = ProcessorMetrics::new(&mut reg);

    // Test that different label combinations create separate metric series
    metrics.record_event_processed(ResultLabel::Success);
    metrics.record_event_processed(ResultLabel::Error);
    metrics.record_event_processed(ResultLabel::Dropped);

    // Each result label should have its own counter
    let success_labels = vec![("result".to_string(), "success".to_string())];
    let error_labels = vec![("result".to_string(), "error".to_string())];
    let dropped_labels = vec![("result".to_string(), "dropped".to_string())];

    assert_eq!(
        metrics.events_processed_total.get_or_create(&success_labels).get(),
        1
    );
    assert_eq!(
        metrics.events_processed_total.get_or_create(&error_labels).get(),
        1
    );
    assert_eq!(
        metrics.events_processed_total.get_or_create(&dropped_labels).get(),
        1
    );
}

#[tokio::test]
#[ignore] // This test actually starts a server, run manually
async fn test_metrics_server_startup() {
    let config = MetricsServerConfig::new("127.0.0.1", 19091);
    let server = MetricsServer::new(config);

    // Start server in background
    let handle = tokio::spawn(async move {
        server.start().await
    });

    // Give server time to start
    sleep(Duration::from_millis(500)).await;

    // Test endpoints (would need reqwest for actual HTTP calls)
    // This is a smoke test to ensure the server starts without panicking

    // Abort the server
    handle.abort();
}
