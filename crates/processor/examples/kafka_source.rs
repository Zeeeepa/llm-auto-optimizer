//! Example: Kafka Source Consumer
//!
//! This example demonstrates how to use KafkaSource to consume events from Kafka
//! with different commit strategies and metrics tracking.
//!
//! # Prerequisites
//!
//! 1. Start Kafka locally:
//!    ```bash
//!    docker run -d --name kafka -p 9092:9092 \
//!      apache/kafka:latest
//!    ```
//!
//! 2. Create a test topic:
//!    ```bash
//!    docker exec -it kafka /opt/kafka/bin/kafka-topics.sh \
//!      --create --topic test-events \
//!      --bootstrap-server localhost:9092
//!    ```
//!
//! 3. Produce some test messages:
//!    ```bash
//!    docker exec -it kafka /opt/kafka/bin/kafka-console-producer.sh \
//!      --topic test-events --bootstrap-server localhost:9092
//!    ```
//!    Then paste JSON messages like:
//!    ```json
//!    {"id": "event-1", "value": 42.5, "timestamp": 1234567890}
//!    {"id": "event-2", "value": 100.0, "timestamp": 1234567891}
//!    ```

use processor::kafka::{CommitStrategy, KafkaSource, KafkaSourceConfig};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{info, Level};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestEvent {
    id: String,
    value: f64,
    timestamp: i64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("Starting Kafka source consumer example");

    // Example 1: Periodic commit strategy
    example_periodic_commit().await?;

    // Example 2: Manual commit strategy
    example_manual_commit().await?;

    // Example 3: Metrics tracking
    example_metrics_tracking().await?;

    Ok(())
}

/// Example 1: Periodic commit strategy
async fn example_periodic_commit() -> anyhow::Result<()> {
    info!("=== Example 1: Periodic Commit Strategy ===");

    let config = KafkaSourceConfig {
        brokers: "localhost:9092".to_string(),
        group_id: "example-periodic-consumer".to_string(),
        topics: vec!["test-events".to_string()],
        commit_strategy: CommitStrategy::Periodic,
        commit_interval_secs: Some(5),
        auto_offset_reset: Some("earliest".to_string()),
        ..Default::default()
    };

    let source = KafkaSource::<TestEvent>::new(config, None).await?;

    let (tx, mut rx) = mpsc::channel(100);

    // Spawn consumer task
    let _consumer_handle = tokio::spawn(async move {
        if let Err(e) = source.start(tx).await {
            eprintln!("Consumer error: {}", e);
        }
    });

    // Process messages for 30 seconds
    let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(30));
    tokio::pin!(timeout);

    let mut count = 0;
    loop {
        tokio::select! {
            Some(msg) = rx.recv() => {
                count += 1;
                info!(
                    "Received message {}: id={}, value={}, partition={}, offset={}",
                    count, msg.payload.id, msg.payload.value, msg.partition, msg.offset
                );
            }
            _ = &mut timeout => {
                info!("Timeout reached, stopping consumer");
                break;
            }
        }
    }

    info!("Processed {} messages with periodic commits", count);
    Ok(())
}

/// Example 2: Manual commit strategy
async fn example_manual_commit() -> anyhow::Result<()> {
    info!("=== Example 2: Manual Commit Strategy ===");

    let config = KafkaSourceConfig {
        brokers: "localhost:9092".to_string(),
        group_id: "example-manual-consumer".to_string(),
        topics: vec!["test-events".to_string()],
        commit_strategy: CommitStrategy::Manual,
        auto_offset_reset: Some("earliest".to_string()),
        ..Default::default()
    };

    let source = KafkaSource::<TestEvent>::new(config, None).await?;

    // Subscribe to topics
    source.subscribe(&["test-events".to_string()]).await?;

    // Consume a few messages manually
    for i in 1..=10 {
        match source.poll().await? {
            Some(msg) => {
                info!(
                    "Message {}: id={}, value={}, offset={}",
                    i, msg.payload.id, msg.payload.value, msg.offset
                );

                // Commit after each message (manual strategy)
                source.commit().await?;
                info!("Committed offset {}", msg.offset);
            }
            None => {
                info!("No more messages or timeout");
                break;
            }
        }
    }

    source.close().await?;
    Ok(())
}

/// Example 3: Metrics tracking
async fn example_metrics_tracking() -> anyhow::Result<()> {
    info!("=== Example 3: Metrics Tracking ===");

    let config = KafkaSourceConfig {
        brokers: "localhost:9092".to_string(),
        group_id: "example-metrics-consumer".to_string(),
        topics: vec!["test-events".to_string()],
        commit_strategy: CommitStrategy::Periodic,
        commit_interval_secs: Some(5),
        auto_offset_reset: Some("earliest".to_string()),
        ..Default::default()
    };

    let source = KafkaSource::<TestEvent>::new(config, None).await?;

    let (tx, mut rx) = mpsc::channel(100);

    // Spawn consumer task
    let source_clone = std::sync::Arc::new(source);
    let source_for_consumer = source_clone.clone();

    let _consumer_handle = tokio::spawn(async move {
        if let Err(e) = source_for_consumer.start(tx).await {
            eprintln!("Consumer error: {}", e);
        }
    });

    // Spawn metrics reporter task
    let source_for_metrics = source_clone.clone();
    let metrics_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
        for _ in 0..6 {
            interval.tick().await;

            let metrics = source_for_metrics.metrics().await;
            info!("=== Kafka Source Metrics ===");
            info!("Messages consumed: {}", metrics.messages_consumed);
            info!("Messages failed: {}", metrics.messages_failed);
            info!("Bytes consumed: {}", metrics.bytes_consumed);
            info!("Throughput: {:.2} msgs/sec", metrics.throughput_msgs_per_sec);
            info!("Throughput: {:.2} KB/sec", metrics.throughput_bytes_per_sec / 1024.0);
            info!("Avg message size: {:.2} bytes", metrics.avg_message_size);
            info!("Total commits: {}", metrics.total_commits);
            info!("Rebalances: {}", metrics.rebalance_count);
            info!("Assigned partitions: {:?}", metrics.assigned_partitions);

            // Get lag information
            if let Ok(lag) = source_for_metrics.get_lag().await {
                info!("Partition lag: {:?}", lag);
            }
        }
    });

    // Process messages
    let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(30));
    tokio::pin!(timeout);

    let mut count = 0;
    loop {
        tokio::select! {
            Some(msg) = rx.recv() => {
                count += 1;
                info!("Processed message {}: {}", count, msg.payload.id);
            }
            _ = &mut timeout => {
                break;
            }
        }
    }

    // Close consumer
    source_clone.close().await?;

    // Wait for tasks
    let _ = tokio::join!(metrics_handle);

    Ok(())
}
