//! Example demonstrating time-series normalization in a stream processing pipeline
//!
//! This example shows how to:
//! 1. Configure time-series normalization
//! 2. Build a pipeline with normalization
//! 3. Create and use a normalization operator
//! 4. Process events with normalization
//!
//! Run with: cargo run --example normalization_example

use processor::config::{
    AlignmentStrategy, DeduplicationConfig, FillStrategy, NormalizationConfig,
};
use processor::pipeline::{
    NormalizationOperator, OperatorContext, StreamOperator, StreamPipelineBuilder,
    TimestampedEvent,
};
use processor::watermark::Watermark;
use chrono::Utc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Time-Series Normalization Example ===\n");

    // Example 1: Basic Pipeline Configuration
    basic_pipeline_example()?;

    // Example 2: Advanced Configuration
    advanced_configuration_example()?;

    // Example 3: Using NormalizationConfig Builder
    config_builder_example()?;

    // Example 4: Manual Operator Usage
    manual_operator_example().await?;

    // Example 5: Complete Pipeline with Multiple Operators
    complete_pipeline_example()?;

    println!("\n=== All Examples Completed Successfully ===");
    Ok(())
}

/// Example 1: Basic pipeline configuration with normalization
fn basic_pipeline_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("Example 1: Basic Pipeline Configuration");
    println!("----------------------------------------");

    let pipeline = StreamPipelineBuilder::new()
        .with_name("basic-normalized-pipeline")
        .with_description("Simple pipeline with 5-second normalization")
        .with_normalization(true, Duration::from_secs(5))
        .with_fill_strategy(FillStrategy::LinearInterpolation)
        .build()?;

    println!("Created pipeline: {}", pipeline.name());
    println!("Normalization enabled: {}", pipeline.config().processor.normalization.enabled);
    println!("Interval: {:?}", pipeline.config().processor.normalization.interval);
    println!("Fill strategy: {:?}\n", pipeline.config().processor.normalization.fill_strategy);

    Ok(())
}

/// Example 2: Advanced configuration with all options
fn advanced_configuration_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("Example 2: Advanced Configuration");
    println!("----------------------------------");

    let pipeline = StreamPipelineBuilder::new()
        .with_name("advanced-normalized-pipeline")
        .with_normalization(true, Duration::from_secs(10))
        .with_fill_strategy(FillStrategy::LinearInterpolation)
        .with_alignment_strategy(AlignmentStrategy::Start)
        .with_normalization_buffer_size(2000)
        .with_interpolation_max_gap(Duration::from_secs(30))
        .with_normalization_outlier_detection(3.0)
        .with_parallelism(8)
        .with_buffer_size(50_000)
        .build()?;

    let norm_config = &pipeline.config().processor.normalization;
    println!("Pipeline: {}", pipeline.name());
    println!("Configuration:");
    println!("  - Interval: {:?}", norm_config.interval);
    println!("  - Fill strategy: {:?}", norm_config.fill_strategy);
    println!("  - Alignment: {:?}", norm_config.alignment);
    println!("  - Buffer size: {}", norm_config.buffer_size);
    println!("  - Max interpolation gap: {:?}", norm_config.interpolation_max_gap);
    println!("  - Outlier detection: {} (threshold: {})",
             norm_config.drop_outliers, norm_config.outlier_threshold);
    println!();

    Ok(())
}

/// Example 3: Using NormalizationConfig builder
fn config_builder_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("Example 3: NormalizationConfig Builder");
    println!("---------------------------------------");

    // Build custom normalization config
    let norm_config = NormalizationConfig::new()
        .enabled()
        .with_interval(Duration::from_secs(5))
        .with_fill_strategy(FillStrategy::ForwardFill)
        .with_alignment(AlignmentStrategy::End)
        .with_buffer_size(1500)
        .with_interpolation_max_gap(Duration::from_secs(20))
        .with_outlier_detection(2.5)
        .with_metrics(true);

    // Use it in pipeline
    let pipeline = StreamPipelineBuilder::new()
        .with_name("custom-config-pipeline")
        .with_normalization_config(norm_config.clone())
        .build()?;

    println!("Custom NormalizationConfig:");
    println!("  - Enabled: {}", norm_config.enabled);
    println!("  - Interval: {} ms", norm_config.interval_ms());
    println!("  - Fill strategy: {:?}", norm_config.fill_strategy);
    println!("  - Alignment: {:?}", norm_config.alignment);
    println!("  - Buffer size: {}", norm_config.buffer_size);
    println!("  - Outlier threshold: {}", norm_config.outlier_threshold);
    println!("  - Pipeline name: {}\n", pipeline.name());

    Ok(())
}

/// Example 4: Manual operator usage
async fn manual_operator_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("Example 4: Manual Operator Usage");
    println!("----------------------------------");

    // Create normalization config
    let config = NormalizationConfig::new()
        .enabled()
        .with_interval(Duration::from_secs(1))
        .with_fill_strategy(FillStrategy::LinearInterpolation)
        .with_alignment(AlignmentStrategy::Start);

    // Create operator
    let operator = NormalizationOperator::new("my-normalizer", config);
    println!("Created NormalizationOperator: {}", operator.name());

    // Create some test events
    let now = Utc::now();
    let events = vec![
        TimestampedEvent {
            timestamp: now,
            value: 10.0,
        },
        TimestampedEvent {
            timestamp: now + chrono::Duration::milliseconds(500),
            value: 15.0,
        },
        TimestampedEvent {
            timestamp: now + chrono::Duration::milliseconds(1500),
            value: 20.0,
        },
    ];

    // Process events
    let ctx = OperatorContext::new(Watermark::min(), 0);
    println!("\nProcessing {} events...", events.len());

    for (i, event) in events.iter().enumerate() {
        let input = bincode::serialize(event)?;
        let _output = operator.process(input, &ctx).await?;
        println!("  Event {}: timestamp={}, value={}",
                 i + 1, event.timestamp, event.value);
    }

    // Get statistics
    let stats = operator.stats();
    println!("\nStatistics:");
    println!("  - Events processed: {}", stats.events_processed);
    println!("  - Events aligned: {}", stats.events_aligned);
    println!("  - Intervals filled: {}", stats.intervals_filled);
    println!("  - Outliers dropped: {}", stats.outliers_dropped);
    println!();

    Ok(())
}

/// Example 5: Complete pipeline with deduplication and normalization
fn complete_pipeline_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("Example 5: Complete Pipeline");
    println!("-----------------------------");

    // Configure deduplication
    let dedup_config = DeduplicationConfig::new()
        .enabled()
        .with_ttl(Duration::from_secs(3600));

    // Configure normalization
    let norm_config = NormalizationConfig::new()
        .enabled()
        .with_interval(Duration::from_secs(5))
        .with_fill_strategy(FillStrategy::LinearInterpolation)
        .with_alignment(AlignmentStrategy::Start)
        .with_outlier_detection(3.0);

    // Build complete pipeline
    let pipeline = StreamPipelineBuilder::new()
        .with_name("production-pipeline")
        .with_description("Production pipeline with deduplication and normalization")
        .with_tag("production")
        .with_tag("metrics")
        .with_deduplication_config(dedup_config)
        .with_normalization_config(norm_config)
        .with_tumbling_window(60_000) // 1-minute windows
        .with_parallelism(8)
        .with_buffer_size(50_000)
        .with_metrics(true)
        .build()?;

    println!("Production Pipeline Configuration:");
    println!("  Name: {}", pipeline.name());
    println!("  Description: {}", pipeline.description().unwrap_or("N/A"));
    println!("  Tags: {:?}", pipeline.tags());
    println!("\nDeduplication:");
    println!("  - Enabled: {}", pipeline.config().processor.deduplication.enabled);
    println!("  - TTL: {:?}", pipeline.config().processor.deduplication.ttl);
    println!("\nNormalization:");
    println!("  - Enabled: {}", pipeline.config().processor.normalization.enabled);
    println!("  - Interval: {:?}", pipeline.config().processor.normalization.interval);
    println!("  - Fill strategy: {:?}", pipeline.config().processor.normalization.fill_strategy);
    println!("  - Alignment: {:?}", pipeline.config().processor.normalization.alignment);
    println!("\nWindow:");
    println!("  - Type: {:?}", pipeline.config().processor.window.window_type);
    println!("  - Size: {:?} ms", pipeline.config().processor.window.size_ms);
    println!("\nResources:");
    println!("  - Parallelism: {}", pipeline.config().processor.parallelism);
    println!("  - Buffer size: {}", pipeline.config().processor.buffer_size);
    println!();

    Ok(())
}

/// Example 6: Different fill strategies comparison
#[allow(dead_code)]
fn fill_strategies_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("Example 6: Fill Strategies Comparison");
    println!("--------------------------------------");

    let strategies = vec![
        ("Zero Fill", FillStrategy::Zero),
        ("Forward Fill", FillStrategy::ForwardFill),
        ("Backward Fill", FillStrategy::BackwardFill),
        ("Linear Interpolation", FillStrategy::LinearInterpolation),
        ("Drop", FillStrategy::Drop),
        ("Mean", FillStrategy::Mean),
    ];

    for (name, strategy) in strategies {
        let config = NormalizationConfig::new()
            .enabled()
            .with_interval(Duration::from_secs(5))
            .with_fill_strategy(strategy);

        println!("{}: {:?}", name, config.fill_strategy);
    }

    println!();
    Ok(())
}

/// Example 7: Different alignment strategies comparison
#[allow(dead_code)]
fn alignment_strategies_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("Example 7: Alignment Strategies Comparison");
    println!("-------------------------------------------");

    let strategies = vec![
        ("Start Alignment", AlignmentStrategy::Start),
        ("End Alignment", AlignmentStrategy::End),
        ("Center Alignment", AlignmentStrategy::Center),
        ("Nearest Alignment", AlignmentStrategy::Nearest),
    ];

    for (name, strategy) in strategies {
        let config = NormalizationConfig::new()
            .enabled()
            .with_interval(Duration::from_secs(5))
            .with_alignment(strategy);

        println!("{}: {:?}", name, config.alignment);
    }

    println!();
    Ok(())
}

/// Example 8: High-frequency metrics use case
#[allow(dead_code)]
fn high_frequency_metrics_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("Example 8: High-Frequency Metrics");
    println!("-----------------------------------");

    let pipeline = StreamPipelineBuilder::new()
        .with_name("high-freq-metrics")
        .with_description("High-frequency metrics with 1-second normalization")
        .with_normalization(true, Duration::from_secs(1))
        .with_fill_strategy(FillStrategy::LinearInterpolation)
        .with_alignment_strategy(AlignmentStrategy::Start)
        .with_normalization_buffer_size(5000) // Large buffer for high frequency
        .with_interpolation_max_gap(Duration::from_secs(5))
        .with_normalization_outlier_detection(3.0)
        .with_tumbling_window(60_000) // Aggregate over 1-minute windows
        .with_parallelism(16) // High parallelism for throughput
        .build()?;

    println!("High-Frequency Pipeline:");
    println!("  - Normalization interval: 1 second");
    println!("  - Buffer size: 5000 events");
    println!("  - Window size: 1 minute");
    println!("  - Parallelism: 16 workers");
    println!("  - Use case: Real-time metrics with outlier detection\n");

    Ok(())
}

/// Example 9: Sparse data use case
#[allow(dead_code)]
fn sparse_data_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("Example 9: Sparse Data Handling");
    println!("--------------------------------");

    let pipeline = StreamPipelineBuilder::new()
        .with_name("sparse-data")
        .with_description("Sparse data with forward fill")
        .with_normalization(true, Duration::from_secs(60))
        .with_fill_strategy(FillStrategy::ForwardFill) // Best for sparse data
        .with_alignment_strategy(AlignmentStrategy::Start)
        .with_interpolation_max_gap(Duration::from_secs(300)) // 5 minutes
        .with_tumbling_window(3600_000) // 1-hour windows
        .build()?;

    println!("Sparse Data Pipeline:");
    println!("  - Normalization interval: 60 seconds");
    println!("  - Fill strategy: Forward Fill");
    println!("  - Max gap: 5 minutes");
    println!("  - Window size: 1 hour");
    println!("  - Use case: IoT sensors with infrequent updates\n");

    Ok(())
}
