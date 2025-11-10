//! Comprehensive demonstration of the stream processor
//!
//! This example shows how to:
//! - Create a stream processor with tumbling windows
//! - Process streaming events with aggregations
//! - Handle watermarks and late events
//! - Collect and display results
//!
//! Run with: cargo run --example stream_processor_demo

use processor::{
    StreamProcessorBuilder, StreamProcessorConfig,
    aggregation::CompositeAggregator,
};
use chrono::{Duration, Utc};
use tokio_stream::StreamExt;
use tracing::{info, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("Starting stream processor demo");

    // Configure the processor
    let config = StreamProcessorConfig {
        allow_late_events: true,
        late_event_threshold: Duration::seconds(5),
        watermark_interval: Duration::milliseconds(500),
        window_retention: Duration::minutes(10),
        max_windows_per_key: 100,
        result_buffer_size: 1000,
    };

    // Build the stream processor with 10-second tumbling windows
    let mut processor = StreamProcessorBuilder::new()
        .with_config(config)
        .build_tumbling(
            Duration::seconds(10),
            |key| {
                info!("Creating aggregator for key: {}", key);
                let mut agg = CompositeAggregator::new();
                agg.add_count("request_count")
                   .add_avg("avg_latency")
                   .add_min("min_latency")
                   .add_max("max_latency")
                   .add_p50("p50_latency")
                   .add_p95("p95_latency")
                   .add_p99("p99_latency")
                   .add_stddev("stddev_latency");
                agg
            },
        );

    // Get the results stream
    let mut results = processor.results().unwrap();

    // Spawn a task to process results
    let results_task = tokio::spawn(async move {
        info!("Waiting for results...");
        while let Some(result) = results.next().await {
            info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            info!("Window Result:");
            info!("  Window: {}", result.window);
            info!("  Key: {}", result.key);
            info!("  Events: {}", result.event_count);
            info!("  Processed at: {}", result.processed_at);

            if let Some(count) = result.results.get_count("request_count") {
                info!("  Request Count: {}", count);
            }

            if let Some(avg) = result.results.get_value("avg_latency") {
                info!("  Average Latency: {:.2}ms", avg);
            }

            if let Some(min) = result.results.get_value("min_latency") {
                info!("  Min Latency: {:.2}ms", min);
            }

            if let Some(max) = result.results.get_value("max_latency") {
                info!("  Max Latency: {:.2}ms", max);
            }

            if let Some(p50) = result.results.get_value("p50_latency") {
                info!("  P50 Latency: {:.2}ms", p50);
            }

            if let Some(p95) = result.results.get_value("p95_latency") {
                info!("  P95 Latency: {:.2}ms", p95);
            }

            if let Some(p99) = result.results.get_value("p99_latency") {
                info!("  P99 Latency: {:.2}ms", p99);
            }

            if let Some(stddev) = result.results.get_value("stddev_latency") {
                info!("  StdDev Latency: {:.2}ms", stddev);
            }

            info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        }
        info!("Results stream ended");
    });

    // Simulate streaming events
    info!("Simulating streaming latency events...");

    let base_time = Utc::now();
    let mut event_time = base_time;

    // Process events for 3 windows (30 seconds of data)
    for window_idx in 0..3 {
        info!("Processing window {} events...", window_idx + 1);

        // Generate events for this window (10 seconds worth)
        for i in 0..20 {
            // Simulate latency values with some variance
            let latency = 100.0 + (i as f64 * 2.0) + (rand::random::<f64>() * 50.0);

            // Process event
            processor.process_event(
                "llm-service".to_string(),
                event_time,
                latency,
            ).await?;

            // Advance time by 500ms
            event_time = event_time + Duration::milliseconds(500);

            // Small delay to simulate real-time processing
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        info!("Completed window {} with 20 events", window_idx + 1);
    }

    info!("All events processed");

    // Display statistics
    let stats = processor.stats().await;
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("Processor Statistics:");
    info!("  Events Processed: {}", stats.events_processed);
    info!("  Events Dropped: {}", stats.events_dropped);
    info!("  Windows Created: {}", stats.windows_created);
    info!("  Windows Fired: {}", stats.windows_fired);
    info!("  Active Windows: {}", stats.active_windows);
    info!("  Processing Latency P50: {:.2}ms", stats.latency_p50_ms);
    info!("  Processing Latency P95: {:.2}ms", stats.latency_p95_ms);
    info!("  Processing Latency P99: {:.2}ms", stats.latency_p99_ms);
    if let Some(wm) = stats.current_watermark {
        info!("  Current Watermark: {}", wm);
    }
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Wait for results to be processed
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    info!("Demo completed successfully!");

    Ok(())
}
