//! Example demonstrating Prometheus metrics usage
//!
//! This example shows how to:
//! 1. Initialize the metrics system
//! 2. Record various metrics
//! 3. Start the metrics HTTP server
//! 4. Query metrics via HTTP
//!
//! Run with: cargo run --example metrics_example
//! Then visit: http://localhost:9090/metrics

use processor::metrics::{
    labels::{BackendLabel, CacheLayerLabel, OperationLabel, ResultLabel, StrategyLabel, WindowTypeLabel},
    MetricsRegistry, MetricsServer, MetricsServerConfig, ProcessorMetrics,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, sleep};
use tracing::{info, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("Starting Prometheus metrics example");

    // Initialize metrics
    let registry = MetricsRegistry::global();
    let mut reg = registry.registry().write();
    let metrics = Arc::new(ProcessorMetrics::new(&mut reg));
    drop(reg);

    info!("Metrics initialized");

    // Start metrics server
    let config = MetricsServerConfig::new("0.0.0.0", 9090);
    let server = MetricsServer::with_registry(config, registry.clone());

    info!("Starting metrics server on http://0.0.0.0:9090");
    info!("Metrics available at: http://localhost:9090/metrics");
    info!("Health check at: http://localhost:9090/health");
    info!("Readiness probe at: http://localhost:9090/ready");

    // Start server in background
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server.start().await {
            eprintln!("Metrics server error: {}", e);
        }
    });

    // Simulate workload
    let metrics_clone = Arc::clone(&metrics);
    let workload_handle = tokio::spawn(async move {
        simulate_workload(metrics_clone).await;
    });

    info!("Simulating workload... Press Ctrl+C to stop");

    // Wait for Ctrl+C
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down...");
        }
        _ = workload_handle => {
            info!("Workload completed");
        }
    }

    // Graceful shutdown
    server_handle.abort();
    info!("Server stopped");

    Ok(())
}

async fn simulate_workload(metrics: Arc<ProcessorMetrics>) {
    let mut tick = interval(Duration::from_millis(100));

    // Counters for simulation
    let mut event_count = 0u64;
    let mut window_count = 0i64;

    loop {
        tick.tick().await;

        // Simulate event processing
        for _ in 0..10 {
            event_count += 1;

            // Record event received
            metrics.record_event_received();

            // Simulate processing with some failures
            let result = if event_count % 10 == 0 {
                ResultLabel::Error
            } else if event_count % 20 == 0 {
                ResultLabel::Dropped
            } else {
                ResultLabel::Success
            };

            // Record processing duration (between 1ms and 10ms)
            let duration = 0.001 + (event_count % 10) as f64 * 0.001;
            metrics.record_event_processing_duration(duration, result);
            metrics.record_event_processed(result);
        }

        // Simulate window operations every 5 ticks
        if event_count % 50 == 0 {
            metrics.record_window_created(WindowTypeLabel::Tumbling);
            window_count += 1;

            if window_count > 10 {
                metrics.record_window_triggered(WindowTypeLabel::Tumbling);
                window_count -= 1;
            }

            metrics.set_windows_active(WindowTypeLabel::Tumbling, window_count);
            metrics.record_window_size(WindowTypeLabel::Tumbling, 100.0);
        }

        // Simulate state operations
        if event_count % 5 == 0 {
            metrics.record_state_operation(
                OperationLabel::Get,
                BackendLabel::Redis,
                0.001,
            );

            // Simulate cache hits/misses (80% hit rate)
            if event_count % 5 != 0 {
                metrics.record_state_cache_hit(CacheLayerLabel::L1);
            } else {
                metrics.record_state_cache_miss(CacheLayerLabel::L1);
                metrics.record_state_cache_hit(CacheLayerLabel::L2);
            }
        }

        // Simulate deduplication
        if event_count % 3 == 0 {
            let is_duplicate = event_count % 15 == 0;
            metrics.record_dedup_check(is_duplicate);
        }

        // Update deduplication cache size
        metrics.set_dedup_cache_size((event_count / 10) as i64);

        // Simulate normalization
        if event_count % 20 == 0 {
            metrics.normalization_events_total.inc();
            metrics.record_normalization_fill(StrategyLabel::Forward);
            metrics.normalization_gaps_filled_total.inc();
        }

        // Update watermark metrics
        let watermark_lag = (event_count % 100) as f64 * 0.01;
        metrics.set_watermark_lag(watermark_lag);

        if event_count % 50 == 0 {
            metrics.record_watermark_update();
        }

        // Simulate out-of-order events
        if event_count % 25 == 0 {
            metrics.record_out_of_order_event();
        }

        // Simulate late events
        if event_count % 30 == 0 {
            metrics.record_late_event();
        }

        // Update pipeline metrics
        let pipeline_lag = (event_count % 100) as f64 * 0.02;
        metrics.set_pipeline_lag(pipeline_lag);

        let backpressure = ((event_count % 200) as i64) * 5;
        metrics.set_backpressure_events(backpressure);

        // Update state size
        let state_size = (event_count * 1024) as i64;
        metrics.set_state_size_bytes(BackendLabel::Redis, state_size);
        metrics.set_state_size_bytes(BackendLabel::InMemory, state_size / 2);

        // Log progress every 1000 events
        if event_count % 1000 == 0 {
            info!(
                "Processed {} events, {} windows active",
                event_count, window_count
            );
        }

        // Run for a limited time in example
        if event_count >= 10000 {
            info!("Workload simulation complete (10000 events processed)");
            break;
        }
    }

    // Keep server running even after workload completes
    sleep(Duration::from_secs(60)).await;
}
