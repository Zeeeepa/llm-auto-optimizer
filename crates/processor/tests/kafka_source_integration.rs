//! Integration tests for KafkaSource
//!
//! These tests require a running Kafka instance.
//! To run these tests:
//!
//! 1. Start Kafka:
//!    ```bash
//!    docker run -d --name kafka -p 9092:9092 apache/kafka:latest
//!    ```
//!
//! 2. Run tests:
//!    ```bash
//!    cargo test --test kafka_source_integration -- --ignored --nocapture
//!    ```

use processor::kafka::{CommitStrategy, KafkaSource, KafkaSourceConfig};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestEvent {
    id: String,
    value: f64,
    timestamp: i64,
}

/// Helper to create a test configuration
fn create_test_config(group_id: &str, topic: &str) -> KafkaSourceConfig {
    KafkaSourceConfig {
        brokers: "localhost:9092".to_string(),
        group_id: group_id.to_string(),
        topics: vec![topic.to_string()],
        commit_strategy: CommitStrategy::Periodic,
        commit_interval_secs: Some(1),
        auto_offset_reset: Some("earliest".to_string()),
        poll_timeout_ms: Some(1000),
        ..Default::default()
    }
}

#[tokio::test]
#[ignore] // Requires Kafka running
async fn test_kafka_source_creation() {
    let config = create_test_config("test-group-1", "test-topic-1");
    let result = KafkaSource::<TestEvent>::new(config, None).await;
    assert!(result.is_ok());
}

#[tokio::test]
#[ignore] // Requires Kafka running
async fn test_kafka_source_subscribe() {
    let config = create_test_config("test-group-2", "test-topic-2");
    let source = KafkaSource::<TestEvent>::new(config, None)
        .await
        .expect("Failed to create source");

    let result = source.subscribe(&["test-topic-2".to_string()]).await;
    assert!(result.is_ok());
}

#[tokio::test]
#[ignore] // Requires Kafka running
async fn test_kafka_source_poll_timeout() {
    let config = KafkaSourceConfig {
        poll_timeout_ms: Some(100),
        ..create_test_config("test-group-3", "empty-topic")
    };

    let source = KafkaSource::<TestEvent>::new(config, None)
        .await
        .expect("Failed to create source");

    source.subscribe(&["empty-topic".to_string()]).await.unwrap();

    // Poll on an empty topic should timeout and return None
    let result = source.poll().await;
    assert!(result.is_ok());
    // Note: result might be None (timeout) or Some (if there are existing messages)
}

#[tokio::test]
#[ignore] // Requires Kafka running
async fn test_kafka_source_metrics() {
    let config = create_test_config("test-group-4", "test-topic-4");
    let source = KafkaSource::<TestEvent>::new(config, None)
        .await
        .expect("Failed to create source");

    let metrics = source.metrics().await;
    assert_eq!(metrics.messages_consumed, 0);
    assert_eq!(metrics.messages_failed, 0);
    assert_eq!(metrics.total_commits, 0);
}

#[tokio::test]
#[ignore] // Requires Kafka running
async fn test_kafka_source_start_and_stop() {
    let config = create_test_config("test-group-5", "test-topic-5");
    let source = KafkaSource::<TestEvent>::new(config, None)
        .await
        .expect("Failed to create source");

    let (tx, mut rx) = mpsc::channel(10);

    // Start consumer in background
    let source_clone = std::sync::Arc::new(source);
    let source_for_task = source_clone.clone();

    let consumer_task = tokio::spawn(async move {
        source_for_task.start(tx).await
    });

    // Let it run for a bit
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Stop consumer
    source_clone.close().await.expect("Failed to close source");

    // Wait for consumer to finish
    let result = tokio::time::timeout(Duration::from_secs(5), consumer_task).await;
    assert!(result.is_ok());
}

#[tokio::test]
#[ignore] // Requires Kafka running
async fn test_kafka_source_commit_strategies() {
    // Test manual commit
    let config = KafkaSourceConfig {
        commit_strategy: CommitStrategy::Manual,
        ..create_test_config("test-group-6", "test-topic-6")
    };

    let source = KafkaSource::<TestEvent>::new(config, None)
        .await
        .expect("Failed to create source");

    // Manual commit should succeed even with no messages
    let result = source.commit().await;
    assert!(result.is_ok());

    // Test periodic commit
    let config = KafkaSourceConfig {
        commit_strategy: CommitStrategy::Periodic,
        commit_interval_secs: Some(1),
        ..create_test_config("test-group-7", "test-topic-7")
    };

    let source = KafkaSource::<TestEvent>::new(config, None)
        .await
        .expect("Failed to create source");

    let result = source.commit().await;
    assert!(result.is_ok());
}

#[tokio::test]
#[ignore] // Requires Kafka running
async fn test_kafka_source_multiple_topics() {
    let config = KafkaSourceConfig {
        topics: vec!["topic-a".to_string(), "topic-b".to_string()],
        ..create_test_config("test-group-8", "topic-a")
    };

    let source = KafkaSource::<TestEvent>::new(config, None)
        .await
        .expect("Failed to create source");

    let result = source
        .subscribe(&["topic-a".to_string(), "topic-b".to_string()])
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
#[ignore] // Requires Kafka running and messages
async fn test_kafka_source_message_consumption() {
    // This test requires messages to be produced to the topic first
    let config = create_test_config("test-group-9", "test-messages");
    let source = KafkaSource::<TestEvent>::new(config, None)
        .await
        .expect("Failed to create source");

    let (tx, mut rx) = mpsc::channel(100);

    let source = std::sync::Arc::new(source);
    let source_for_task = source.clone();

    // Start consumer
    let consumer_task = tokio::spawn(async move {
        source_for_task.start(tx).await
    });

    // Consume messages for a few seconds
    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::pin!(timeout);

    let mut count = 0;
    loop {
        tokio::select! {
            Some(msg) = rx.recv() => {
                count += 1;
                assert!(msg.offset >= 0);
                assert!(!msg.topic.is_empty());
                println!("Consumed message: {:?}", msg.payload);

                if count >= 10 {
                    break;
                }
            }
            _ = &mut timeout => {
                break;
            }
        }
    }

    println!("Consumed {} messages", count);

    // Stop consumer
    source.close().await.expect("Failed to close");

    // Check metrics
    let metrics = source.metrics().await;
    assert_eq!(metrics.messages_consumed, count);
}

#[tokio::test]
#[ignore] // Requires Kafka running
async fn test_kafka_source_lag_tracking() {
    let config = create_test_config("test-group-10", "test-lag");
    let source = KafkaSource::<TestEvent>::new(config, None)
        .await
        .expect("Failed to create source");

    source.subscribe(&["test-lag".to_string()]).await.unwrap();

    // Give Kafka time to initialize
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Get lag - should not error even if no partitions are assigned yet
    let result = source.get_lag().await;
    // This might fail if no partitions are assigned, which is okay
    if let Ok(lag) = result {
        println!("Lag information: {:?}", lag);
    }
}
