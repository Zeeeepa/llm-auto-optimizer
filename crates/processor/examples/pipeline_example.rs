//! Comprehensive example of using the stream processing pipeline
//!
//! This example demonstrates:
//! - Building a pipeline with fluent API
//! - Creating and using stream operators
//! - Event ingestion and processing
//! - Window assignment and aggregation
//! - Watermark handling
//!
//! Run with: cargo run --package processor --example pipeline_example

use processor::{
    EventKey, EventTimeExtractor, KeyExtractor,
    StreamPipelineBuilder, StreamExecutor,
    StateBackend,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Example event representing a metric measurement
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MetricEvent {
    /// Metric name
    name: String,
    /// Metric value
    value: f64,
    /// Event timestamp
    timestamp: DateTime<Utc>,
    /// Source/key for partitioning
    source: String,
    /// Additional tags
    tags: Vec<String>,
}

impl MetricEvent {
    fn new(name: impl Into<String>, value: f64, source: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value,
            timestamp: Utc::now(),
            source: source.into(),
            tags: Vec::new(),
        }
    }

    fn with_timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = timestamp;
        self
    }

    fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }
}

/// Implement EventTimeExtractor to extract event timestamps
impl EventTimeExtractor<MetricEvent> for MetricEvent {
    fn extract_event_time(&self, event: &MetricEvent) -> DateTime<Utc> {
        event.timestamp
    }
}

/// Implement KeyExtractor for partitioning
impl KeyExtractor<MetricEvent> for MetricEvent {
    fn extract_key(&self, event: &MetricEvent) -> EventKey {
        EventKey::Session(event.source.clone())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Stream Processing Pipeline Example ===\n");

    // Example 1: Basic Pipeline with Operators
    println!("Example 1: Basic Pipeline with Operators");
    basic_pipeline_example().await?;

    // Example 2: Windowed Aggregation
    println!("\nExample 2: Windowed Aggregation");
    windowed_aggregation_example().await?;

    // Example 3: Complex Pipeline with Multiple Operators
    println!("\nExample 3: Complex Pipeline");
    complex_pipeline_example().await?;

    Ok(())
}

/// Example 1: Basic pipeline with map and filter operators
async fn basic_pipeline_example() -> Result<(), Box<dyn std::error::Error>> {
    // Build a simple pipeline
    let pipeline = StreamPipelineBuilder::new()
        .with_name("basic-pipeline")
        .with_description("A basic example pipeline")
        .with_parallelism(2)
        .with_buffer_size(1000)
        .build()?;

    println!("Created pipeline: {}", pipeline.name());
    println!("Description: {}", pipeline.description().unwrap_or("None"));

    // Create executor
    let executor: StreamExecutor<MetricEvent> = pipeline.create_executor();

    // Ingest some events
    let events = vec![
        MetricEvent::new("latency", 100.0, "service-a"),
        MetricEvent::new("latency", 150.0, "service-b"),
        MetricEvent::new("latency", 75.0, "service-a"),
    ];

    for event in events {
        executor.ingest(event).await?;
    }

    // Check stats
    let stats = executor.stats().await;
    println!("Events processed: {}", stats.events_processed);
    println!("Events dropped: {}", stats.events_dropped);

    Ok(())
}

/// Example 2: Windowed aggregation pipeline
async fn windowed_aggregation_example() -> Result<(), Box<dyn std::error::Error>> {
    // Build a pipeline with tumbling windows
    let pipeline = StreamPipelineBuilder::new()
        .with_name("windowed-aggregator")
        .with_description("Aggregates metrics in 1-minute windows")
        .with_tumbling_window(60_000) // 1 minute windows
        .with_watermark_delay(Duration::from_secs(5))
        .with_state_backend(StateBackend::Memory)
        .with_allowed_lateness(10_000) // 10 seconds
        .build()?;

    println!("Created windowed pipeline: {}", pipeline.name());
    println!("Window type: Tumbling (60s)");

    let executor: StreamExecutor<MetricEvent> = pipeline.create_executor();

    // Simulate a stream of events
    let base_time = Utc::now();
    let events = vec![
        MetricEvent::new("requests", 100.0, "api-server")
            .with_timestamp(base_time)
            .with_tag("http"),
        MetricEvent::new("requests", 150.0, "api-server")
            .with_timestamp(base_time + chrono::Duration::seconds(10))
            .with_tag("http"),
        MetricEvent::new("requests", 120.0, "api-server")
            .with_timestamp(base_time + chrono::Duration::seconds(20))
            .with_tag("http"),
    ];

    for event in events {
        println!("Ingesting event: {} @ {}", event.name, event.timestamp);
        executor.ingest(event).await?;
    }

    // Get statistics
    let stats = executor.stats().await;
    println!("\nPipeline Statistics:");
    println!("  Events processed: {}", stats.events_processed);
    println!("  Windows created: {}", stats.windows_created);
    println!("  Active windows: {}", stats.active_windows);
    println!("  Current watermark: {}", stats.current_watermark);

    Ok(())
}

/// Example 3: Complex pipeline with multiple operators
async fn complex_pipeline_example() -> Result<(), Box<dyn std::error::Error>> {
    // Build a sophisticated pipeline
    let pipeline = StreamPipelineBuilder::new()
        .with_name("complex-pipeline")
        .with_description("Multi-stage processing with operators")
        .with_parallelism(4)
        .with_buffer_size(10_000)
        .with_sliding_window(300_000, 60_000) // 5 min window, 1 min slide
        .with_watermark_delay(Duration::from_secs(10))
        .with_state_backend(StateBackend::Memory)
        .with_max_state_size(100_000_000) // 100MB
        .with_percentiles(vec![50, 90, 95, 99])
        .with_outlier_removal(3.0) // Remove outliers beyond 3 std devs
        .with_tag("production")
        .with_tag("metrics")
        .build()?;

    println!("Created complex pipeline: {}", pipeline.name());
    println!("Tags: {:?}", pipeline.tags());
    println!("Parallelism: {}", pipeline.config().processor.parallelism);
    println!("Buffer size: {}", pipeline.config().processor.buffer_size);

    let executor: StreamExecutor<MetricEvent> = pipeline.create_executor();

    // Simulate realistic metric stream
    let base_time = Utc::now();
    let mut events = Vec::new();

    for i in 0..10 {
        events.push(
            MetricEvent::new("cpu_usage", 50.0 + (i as f64 * 2.0), "server-1")
                .with_timestamp(base_time + chrono::Duration::seconds(i * 10))
                .with_tag("cpu")
                .with_tag("monitoring"),
        );

        events.push(
            MetricEvent::new("memory_usage", 75.0 + (i as f64), "server-1")
                .with_timestamp(base_time + chrono::Duration::seconds(i * 10))
                .with_tag("memory")
                .with_tag("monitoring"),
        );
    }

    println!("\nIngesting {} events...", events.len());
    executor.ingest_batch(events).await?;

    // Display final statistics
    let stats = executor.stats().await;
    println!("\nFinal Statistics:");
    println!("  Total events: {}", stats.events_processed);
    println!("  Dropped events: {}", stats.events_dropped);
    println!("  Windows created: {}", stats.windows_created);
    println!("  Windows triggered: {}", stats.windows_triggered);
    println!("  Active windows: {}", stats.active_windows);
    println!("  Aggregations computed: {}", stats.aggregations_computed);
    println!("  Bytes processed: {} bytes", stats.bytes_processed);
    println!("  Errors: {}", stats.errors);

    // Calculate throughput
    let elapsed = 1.0; // In a real scenario, track actual elapsed time
    println!("  Throughput: {:.2} events/sec", stats.events_per_second(elapsed));
    println!("  Throughput: {:.2} MB/s", stats.throughput_mbps(elapsed));

    Ok(())
}
