//! Dead Letter Queue Integration Example
//!
//! This example demonstrates how to integrate the DLQ system with
//! the feedback collector and Kafka producer.

use std::sync::Arc;
use std::path::PathBuf;
use collector::{
    FeedbackEvent, UserFeedbackEvent, PerformanceMetricsEvent,
    FeedbackProducer, KafkaProducerConfig,
    DLQManager, DLQConfig, DLQType,
    KafkaProducerWithDLQ,
};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== Dead Letter Queue Integration Example ===\n");

    // Example 1: File-based DLQ
    println!("Example 1: File-based DLQ");
    file_based_dlq_example().await?;

    // Example 2: Hybrid DLQ
    println!("\nExample 2: Hybrid DLQ");
    hybrid_dlq_example().await?;

    // Example 3: Manual DLQ management
    println!("\nExample 3: Manual DLQ Management");
    manual_dlq_management().await?;

    // Example 4: DLQ with producer integration
    println!("\nExample 4: Producer with DLQ Integration");
    producer_with_dlq_example().await?;

    Ok(())
}

/// Example 1: File-based DLQ
async fn file_based_dlq_example() -> Result<(), Box<dyn std::error::Error>> {
    // Configure file-based DLQ
    let dlq_config = DLQConfig {
        enabled: true,
        dlq_type: DLQType::File,
        kafka_topic: "feedback-events-dlq".to_string(),
        kafka_config: None, // Not needed for file-only DLQ
        file_path: PathBuf::from("/tmp/dlq_example"),
        max_file_size_mb: 10,
        max_files: 5,
        retry_interval_seconds: 60,
        max_retry_attempts: 3,
        retention_days: 7,
        enable_recovery: false, // Disabled for this example
    };

    // Create Kafka producer (for recovery)
    let kafka_config = KafkaProducerConfig::default();
    let producer = Arc::new(FeedbackProducer::new(kafka_config)?);

    // Create DLQ manager
    let dlq_manager = DLQManager::new(dlq_config, producer)?;

    // Simulate failed event
    let request_id = Uuid::new_v4();
    let event = FeedbackEvent::UserFeedback(
        UserFeedbackEvent::new("session123", request_id)
            .with_rating(4.5)
            .with_liked(true)
    );

    // Add to DLQ
    dlq_manager.add_failed_event(
        event,
        "Kafka broker unavailable".to_string()
    ).await?;

    println!("  ✓ Added failed event to file-based DLQ");

    // Get statistics
    let stats = dlq_manager.stats().await?;
    println!("  ✓ DLQ Stats:");
    println!("    - Total entries: {}", stats.total_entries);
    println!("    - Current depth: {}", stats.current_depth);
    println!("    - File size: {} bytes", stats.file_size_bytes);

    // List entries
    let entries = dlq_manager.list_entries(0, 10).await?;
    println!("  ✓ DLQ contains {} entries", entries.len());

    Ok(())
}

/// Example 2: Hybrid DLQ
async fn hybrid_dlq_example() -> Result<(), Box<dyn std::error::Error>> {
    // Configure hybrid DLQ (Kafka + File fallback)
    let mut kafka_config = KafkaProducerConfig::default();
    kafka_config.topic = "feedback-events-dlq".to_string();

    let dlq_config = DLQConfig {
        enabled: true,
        dlq_type: DLQType::Hybrid,
        kafka_topic: "feedback-events-dlq".to_string(),
        kafka_config: Some(kafka_config.clone()),
        file_path: PathBuf::from("/tmp/dlq_hybrid"),
        max_file_size_mb: 50,
        max_files: 10,
        retry_interval_seconds: 300,
        max_retry_attempts: 3,
        retention_days: 7,
        enable_recovery: true,
    };

    // Create Kafka producer
    let producer = Arc::new(FeedbackProducer::new(kafka_config)?);

    // Create DLQ manager with recovery
    let mut dlq_manager = DLQManager::new(dlq_config, producer)?;
    dlq_manager.start_recovery_task();

    println!("  ✓ Created hybrid DLQ with automatic recovery");

    // Simulate multiple failed events
    for i in 0..5 {
        let request_id = Uuid::new_v4();
        let event = FeedbackEvent::PerformanceMetrics(
            PerformanceMetricsEvent::new(
                request_id,
                "gpt-4",
                1500.0 + (i as f64 * 100.0),
                100,
                200,
                0.05
            )
        );

        dlq_manager.add_failed_event(
            event,
            format!("Timeout error #{}", i)
        ).await?;
    }

    println!("  ✓ Added 5 events to hybrid DLQ");

    // Get statistics
    let stats = dlq_manager.stats().await?;
    println!("  ✓ DLQ Stats:");
    println!("    - Total entries: {}", stats.total_entries);
    println!("    - Current depth: {}", stats.current_depth);

    // Let recovery task run briefly
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Shutdown
    dlq_manager.shutdown().await?;
    println!("  ✓ DLQ manager shut down gracefully");

    Ok(())
}

/// Example 3: Manual DLQ management
async fn manual_dlq_management() -> Result<(), Box<dyn std::error::Error>> {
    let dlq_config = DLQConfig {
        enabled: true,
        dlq_type: DLQType::File,
        kafka_topic: "feedback-events-dlq".to_string(),
        kafka_config: None,
        file_path: PathBuf::from("/tmp/dlq_manual"),
        max_file_size_mb: 10,
        max_files: 5,
        retry_interval_seconds: 60,
        max_retry_attempts: 3,
        retention_days: 7,
        enable_recovery: false,
    };

    let kafka_config = KafkaProducerConfig::default();
    let producer = Arc::new(FeedbackProducer::new(kafka_config)?);
    let dlq_manager = DLQManager::new(dlq_config, producer)?;

    // Add some events
    let request_id = Uuid::new_v4();
    let event = FeedbackEvent::UserFeedback(
        UserFeedbackEvent::new("session456", request_id)
            .with_task_completed(true)
    );

    dlq_manager.add_failed_event(
        event,
        "Network unreachable".to_string()
    ).await?;

    println!("  ✓ Added event to DLQ");

    // List all entries
    let entries = dlq_manager.list_entries(0, 100).await?;
    println!("  ✓ Found {} entries in DLQ", entries.len());

    // Display entry details
    for entry in &entries {
        println!("    - Entry {}", entry.dlq_id);
        println!("      Event ID: {}", entry.original_event.id());
        println!("      Failure: {}", entry.failure_reason);
        println!("      Retry count: {}", entry.retry_count);
        println!("      Failed at: {}", entry.failure_timestamp);
    }

    // Manually replay an entry
    if let Some(first_entry) = entries.first() {
        println!("  ✓ Attempting manual replay of entry {}", first_entry.dlq_id);
        let success = dlq_manager.replay_entry(&first_entry.dlq_id).await?;
        if success {
            println!("    ✓ Successfully replayed entry");
        } else {
            println!("    ✗ Failed to replay entry");
        }
    }

    // Purge old entries (older than 1 day)
    let purged = dlq_manager.purge_old_entries(1).await?;
    println!("  ✓ Purged {} old entries", purged);

    Ok(())
}

/// Example 4: Producer with DLQ integration
async fn producer_with_dlq_example() -> Result<(), Box<dyn std::error::Error>> {
    // Setup Kafka producer
    let kafka_config = KafkaProducerConfig::default();
    let producer = Arc::new(FeedbackProducer::new(kafka_config.clone())?);

    // Setup DLQ
    let dlq_config = DLQConfig {
        enabled: true,
        dlq_type: DLQType::File,
        kafka_topic: "feedback-events-dlq".to_string(),
        kafka_config: None,
        file_path: PathBuf::from("/tmp/dlq_producer"),
        max_file_size_mb: 10,
        max_files: 5,
        retry_interval_seconds: 60,
        max_retry_attempts: 3,
        retention_days: 7,
        enable_recovery: true,
    };

    let dlq_manager = Arc::new(DLQManager::new(dlq_config, producer.clone())?);

    // Create integrated producer
    let producer_with_dlq = KafkaProducerWithDLQ::new(
        producer.clone(),
        Some(dlq_manager.clone())
    );

    println!("  ✓ Created Kafka producer with DLQ integration");

    // Publish events (will automatically route failures to DLQ)
    let request_id = Uuid::new_v4();
    let event = FeedbackEvent::UserFeedback(
        UserFeedbackEvent::new("session789", request_id)
            .with_rating(5.0)
            .with_liked(true)
            .with_task_completed(true)
    );

    // This will automatically use DLQ for permanent failures
    match producer_with_dlq.publish_event(&event).await {
        Ok(_) => println!("  ✓ Event published successfully"),
        Err(e) => println!("  ✗ Event failed (transient error): {}", e),
    }

    // Check DLQ stats
    if let Some(stats) = producer_with_dlq.dlq_stats().await {
        println!("  ✓ DLQ Stats:");
        println!("    - Total entries: {}", stats.total_entries);
        println!("    - Current depth: {}", stats.current_depth);
        println!("    - Recovered: {}", stats.total_recovered);
        println!("    - Failed: {}", stats.total_failed);
    }

    Ok(())
}
