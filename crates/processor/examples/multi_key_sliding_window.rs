//! Multi-key sliding window example
//!
//! This example demonstrates:
//! - Sliding windows for smoothed metrics
//! - Multiple aggregation keys (different services)
//! - Real-time metric visualization
//!
//! Run with: cargo run --example multi_key_sliding_window

use processor::{
    StreamProcessorBuilder, StreamProcessorConfig,
    aggregation::CompositeAggregator,
};
use chrono::{Duration, Utc};
use tokio_stream::StreamExt;
use tracing::{info, Level};
use tracing_subscriber;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("Starting multi-key sliding window demo");

    // Configure the processor
    let config = StreamProcessorConfig {
        allow_late_events: true,
        late_event_threshold: Duration::seconds(10),
        watermark_interval: Duration::seconds(1),
        window_retention: Duration::minutes(15),
        max_windows_per_key: 200,
        result_buffer_size: 2000,
    };

    // Build the stream processor with sliding windows
    // Window size: 60 seconds, Slide: 10 seconds
    // This means we get updated metrics every 10 seconds, computed over the last 60 seconds
    let mut processor = StreamProcessorBuilder::new()
        .with_config(config)
        .build_sliding(
            Duration::seconds(60),  // 1-minute window
            Duration::seconds(10),  // Slide every 10 seconds
            |key| {
                info!("Creating aggregator for service: {}", key);
                let mut agg = CompositeAggregator::new();
                agg.add_count("requests")
                   .add_avg("avg_latency")
                   .add_p95("p95_latency")
                   .add_max("max_latency");
                agg
            },
        );

    // Get the results stream
    let mut results = processor.results().unwrap();

    // Track metrics per service
    let metrics = std::sync::Arc::new(tokio::sync::RwLock::new(
        HashMap::<String, Vec<(chrono::DateTime<chrono::Utc>, f64)>>::new()
    ));
    let metrics_clone = metrics.clone();

    // Spawn a task to collect results
    let results_task = tokio::spawn(async move {
        while let Some(result) = results.next().await {
            let key = result.key.clone();
            let timestamp = result.window.bounds.end;

            if let Some(avg_latency) = result.results.get_value("avg_latency") {
                let mut m = metrics_clone.write().await;
                m.entry(key.clone())
                    .or_insert_with(Vec::new)
                    .push((timestamp, avg_latency));

                info!(
                    "📊 {} | Window: {} | Requests: {} | Avg: {:.1}ms | P95: {:.1}ms | Max: {:.1}ms",
                    key,
                    result.window.bounds,
                    result.results.get_count("requests").unwrap_or(0),
                    avg_latency,
                    result.results.get_value("p95_latency").unwrap_or(0.0),
                    result.results.get_value("max_latency").unwrap_or(0.0),
                );
            }
        }
    });

    // Define multiple services to track
    let services = vec![
        ("gpt-4-service", 80.0, 20.0),      // base latency, variance
        ("claude-3-service", 120.0, 30.0),  // base latency, variance
        ("llama-2-service", 50.0, 15.0),    // base latency, variance
    ];

    info!("Simulating traffic for {} services...", services.len());

    let base_time = Utc::now();
    let mut event_time = base_time;

    // Simulate 2 minutes of traffic (120 seconds)
    for iteration in 0..240 {
        // Process events for each service
        for (service, base_latency, variance) in &services {
            // Generate latency with some random variation
            let latency = base_latency + (rand::random::<f64>() - 0.5) * 2.0 * variance;

            // Add occasional spikes for realism
            let latency = if rand::random::<f64>() < 0.05 {
                latency * 2.0 // 5% spike probability
            } else {
                latency
            };

            processor.process_event(
                service.to_string(),
                event_time,
                latency,
            ).await?;
        }

        // Advance time by 500ms
        event_time = event_time + Duration::milliseconds(500);

        // Show progress every 10 iterations (5 seconds)
        if iteration % 10 == 0 {
            let elapsed = iteration / 2; // seconds
            info!("⏱️  Processed {} seconds of data...", elapsed);
        }

        // Small delay to simulate real-time
        tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
    }

    info!("✅ All events processed");

    // Wait for final results
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Display final statistics
    let stats = processor.stats().await;
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("Final Processor Statistics:");
    info!("  Events Processed: {}", stats.events_processed);
    info!("  Events Dropped: {}", stats.events_dropped);
    info!("  Windows Fired: {}", stats.windows_fired);
    info!("  Active Windows: {}", stats.active_windows);
    info!("  Processing Latency P50: {:.2}ms", stats.latency_p50_ms);
    info!("  Processing Latency P95: {:.2}ms", stats.latency_p95_ms);
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Display service summaries
    let m = metrics.read().await;
    info!("Service Metric Summaries:");
    for (service, data) in m.iter() {
        if !data.is_empty() {
            let avg_of_avgs = data.iter().map(|(_, v)| v).sum::<f64>() / data.len() as f64;
            let min_latency = data.iter().map(|(_, v)| v).fold(f64::INFINITY, |a, &b| a.min(b));
            let max_latency = data.iter().map(|(_, v)| v).fold(f64::NEG_INFINITY, |a, &b| a.max(b));

            info!("  {} ({} windows):", service, data.len());
            info!("    Overall Avg Latency: {:.1}ms", avg_of_avgs);
            info!("    Min Avg Latency: {:.1}ms", min_latency);
            info!("    Max Avg Latency: {:.1}ms", max_latency);
        }
    }

    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("Demo completed successfully!");

    Ok(())
}
