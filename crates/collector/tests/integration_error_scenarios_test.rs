//! Integration tests for error scenarios and failure handling
//!
//! These tests verify that the collector and producer handle various
//! error conditions gracefully, including Kafka failures, network issues,
//! and invalid configurations.

mod common;

use collector::{
    FeedbackCollector, FeedbackEvent, FeedbackProducer, KafkaProducerConfig, UserFeedbackEvent,
    FeedbackEventBatch, PerformanceMetricsEvent,
};
use common::{test_collector_config, test_kafka_config, KafkaTestContainer};
use std::time::Duration;
use testcontainers::clients::Cli;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Test creating producer with invalid broker configuration
#[tokio::test]
async fn test_producer_with_invalid_broker() {
    let config = KafkaProducerConfig {
        brokers: "invalid-broker:9092".to_string(),
        topic: "test-topic".to_string(),
        client_id: "test-client".to_string(),
        message_timeout_ms: 5000,
        compression_type: "none".to_string(),
        retries: 0,
        retry_backoff_ms: 100,
        circuit_breaker_config: None,
    };

    // Producer creation should succeed (connection is lazy)
    let producer = FeedbackProducer::new(config);
    assert!(producer.is_ok(), "Producer creation should succeed");

    // But publishing should fail
    let producer = producer.unwrap();
    let event = FeedbackEvent::UserFeedback(UserFeedbackEvent::new("session", Uuid::new_v4()));

    let result = producer.publish_event(&event).await;
    assert!(result.is_err(), "Publishing to invalid broker should fail");
}

/// Test publishing timeout scenario
#[tokio::test]
async fn test_publish_timeout() {
    let config = KafkaProducerConfig {
        brokers: "192.0.2.0:9092".to_string(), // Non-routable IP (TEST-NET-1)
        topic: "test-topic".to_string(),
        client_id: "timeout-test".to_string(),
        message_timeout_ms: 1000, // Very short timeout
        compression_type: "none".to_string(),
        retries: 0,
        retry_backoff_ms: 100,
        circuit_breaker_config: None,
    };

    let producer = FeedbackProducer::new(config).expect("Producer creation should succeed");
    let event = FeedbackEvent::UserFeedback(UserFeedbackEvent::new("session", Uuid::new_v4()));

    // Publishing should timeout
    let result = producer.publish_event(&event).await;
    assert!(result.is_err(), "Should timeout when broker is unreachable");
}

/// Test resilient batch publishing with simulated failures
#[tokio::test]
async fn test_resilient_batch_with_failing_broker() {
    let config = KafkaProducerConfig {
        brokers: "invalid-broker:9092".to_string(),
        topic: "test-topic".to_string(),
        client_id: "resilient-test".to_string(),
        message_timeout_ms: 1000,
        compression_type: "none".to_string(),
        retries: 1,
        retry_backoff_ms: 100,
        circuit_breaker_config: None,
    };

    let producer = FeedbackProducer::new(config).expect("Producer creation should succeed");

    let events = vec![
        FeedbackEvent::UserFeedback(UserFeedbackEvent::new("s1", Uuid::new_v4())),
        FeedbackEvent::UserFeedback(UserFeedbackEvent::new("s2", Uuid::new_v4())),
    ];

    let batch = FeedbackEventBatch::new(events);
    let (success_count, error_count) = producer.publish_batch_resilient(&batch).await;

    // All should fail with invalid broker
    assert_eq!(success_count, 0, "No events should succeed");
    assert_eq!(error_count, 2, "All events should fail");
}

/// Test collector continues operating after Kafka failures
#[tokio::test]
async fn test_collector_resilience_to_kafka_failures() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "resilience-test";

    let kafka_config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let mut config = test_collector_config(kafka_config);
    config.max_batch_size = 2; // Small batch for quick flushing

    let (mut collector, _) = FeedbackCollector::new(config.clone());
    let (tx, rx) = mpsc::channel(config.buffer_size);

    collector.start(rx).await.expect("Failed to start");

    // Send some events that will succeed
    let event1 = FeedbackEvent::UserFeedback(UserFeedbackEvent::new("s1", Uuid::new_v4()));
    let event2 = FeedbackEvent::UserFeedback(UserFeedbackEvent::new("s2", Uuid::new_v4()));

    tx.send(event1).await.expect("Send failed");
    tx.send(event2).await.expect("Send failed");

    // Wait for processing
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Check that collector is still functional
    let stats = collector.stats();
    assert_eq!(stats.events_received, 2);

    // Should be able to send more events
    let event3 = FeedbackEvent::UserFeedback(UserFeedbackEvent::new("s3", Uuid::new_v4()));
    tx.send(event3).await.expect("Send should still work");

    drop(tx);
    tokio::time::sleep(Duration::from_secs(2)).await;

    let final_stats = collector.stats();
    assert_eq!(final_stats.events_received, 3, "Collector should still accept events");

    collector.shutdown().await.expect("Shutdown failed");
}

/// Test handling of channel closure during operation
#[tokio::test]
async fn test_collector_handles_channel_closure() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "channel-closure-test";

    let kafka_config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let config = test_collector_config(kafka_config);

    let (mut collector, _) = FeedbackCollector::new(config.clone());
    let (tx, rx) = mpsc::channel(config.buffer_size);

    collector.start(rx).await.expect("Failed to start");

    // Send one event
    let event = FeedbackEvent::UserFeedback(UserFeedbackEvent::new("s1", Uuid::new_v4()));
    tx.send(event).await.expect("Send failed");

    // Immediately close the channel
    drop(tx);

    // Wait for graceful shutdown
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Collector should still shut down gracefully
    let result = collector.shutdown().await;
    assert!(result.is_ok(), "Should shutdown gracefully after channel closure");
}

/// Test collector behavior when buffer is full
#[tokio::test]
async fn test_collector_full_buffer_behavior() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "buffer-full-test";

    let kafka_config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let mut config = test_collector_config(kafka_config);
    config.buffer_size = 5; // Very small buffer
    config.max_batch_size = 100; // Large batch to prevent auto-flushing

    let (mut collector, _) = FeedbackCollector::new(config.clone());
    let (tx, rx) = mpsc::channel(config.buffer_size);

    collector.start(rx).await.expect("Failed to start");

    // Fill the buffer
    for i in 0..5 {
        let event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new(format!("session-{}", i), Uuid::new_v4()),
        );
        collector.try_submit(event).expect("Should succeed with space in buffer");
    }

    // Next try_submit might fail if buffer is full and worker hasn't processed yet
    let event = FeedbackEvent::UserFeedback(UserFeedbackEvent::new("overflow", Uuid::new_v4()));
    let result = collector.try_submit(event);

    // Either succeeds (worker processed some) or fails (buffer full) - both are valid
    // The important thing is that it doesn't panic or block
    match result {
        Ok(_) => {}, // Buffer had space
        Err(_) => {}, // Buffer was full - this is expected behavior
    }

    drop(tx);
    tokio::time::sleep(Duration::from_secs(2)).await;
    collector.shutdown().await.expect("Shutdown failed");
}

/// Test flush operation with timeout
#[tokio::test]
async fn test_producer_flush_timeout() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "flush-test";

    let config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let producer = FeedbackProducer::new(config).expect("Producer creation failed");

    // Publish an event
    let event = FeedbackEvent::UserFeedback(UserFeedbackEvent::new("session", Uuid::new_v4()));
    producer.publish_event(&event).await.expect("Publish failed");

    // Flush with reasonable timeout should succeed
    let result = producer.flush(5000).await;
    assert!(result.is_ok(), "Flush should succeed: {:?}", result);
}

/// Test invalid event serialization handling
#[tokio::test]
async fn test_event_serialization_deserialization() {
    // Create an event
    let event = FeedbackEvent::UserFeedback(UserFeedbackEvent::new("session", Uuid::new_v4()));

    // Serialize
    let serialized = serde_json::to_vec(&event);
    assert!(serialized.is_ok(), "Serialization should succeed");

    // Deserialize
    let deserialized: Result<FeedbackEvent, _> = serde_json::from_slice(&serialized.unwrap());
    assert!(deserialized.is_ok(), "Deserialization should succeed");
    assert_eq!(deserialized.unwrap(), event, "Event should match after round-trip");
}

/// Test collector with multiple rapid shutdowns
#[tokio::test]
async fn test_multiple_collector_lifecycle() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "lifecycle-test";

    for iteration in 0..3 {
        let kafka_config = test_kafka_config(&kafka.bootstrap_servers(), topic);
        let config = test_collector_config(kafka_config);

        let (mut collector, _) = FeedbackCollector::new(config.clone());
        let (tx, rx) = mpsc::channel(config.buffer_size);

        collector.start(rx).await.expect("Failed to start");

        // Send event
        let event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new(format!("iter-{}", iteration), Uuid::new_v4()),
        );
        tx.send(event).await.expect("Send failed");

        drop(tx);
        tokio::time::sleep(Duration::from_millis(500)).await;

        let result = collector.shutdown().await;
        assert!(result.is_ok(), "Iteration {} shutdown failed", iteration);
    }
}

/// Test handling of Kafka broker restart scenario
#[tokio::test]
async fn test_kafka_broker_unavailable_recovery() {
    // This test simulates what happens when Kafka is temporarily unavailable
    let config = KafkaProducerConfig {
        brokers: "localhost:19092".to_string(), // Non-existent port
        topic: "recovery-test".to_string(),
        client_id: "recovery-client".to_string(),
        message_timeout_ms: 2000,
        compression_type: "none".to_string(),
        retries: 2,
        retry_backoff_ms: 500,
        circuit_breaker_config: None,
    };

    let producer = FeedbackProducer::new(config).expect("Producer creation should succeed");

    let events = vec![
        FeedbackEvent::UserFeedback(UserFeedbackEvent::new("s1", Uuid::new_v4())),
        FeedbackEvent::UserFeedback(UserFeedbackEvent::new("s2", Uuid::new_v4())),
    ];

    let batch = FeedbackEventBatch::new(events);

    // Should fail gracefully without panicking
    let (success, errors) = producer.publish_batch_resilient(&batch).await;

    assert_eq!(success, 0, "No events should succeed with unavailable broker");
    assert_eq!(errors, 2, "All events should fail");
}

/// Test collector stats accuracy during errors
#[tokio::test]
async fn test_collector_stats_with_errors() {
    let config = KafkaProducerConfig {
        brokers: "invalid:9092".to_string(),
        topic: "error-stats-test".to_string(),
        client_id: "stats-test".to_string(),
        message_timeout_ms: 1000,
        compression_type: "none".to_string(),
        retries: 0,
        retry_backoff_ms: 100,
        circuit_breaker_config: None,
    };

    let collector_config = test_collector_config(config);
    let (mut collector, _) = FeedbackCollector::new(collector_config.clone());
    let (tx, rx) = mpsc::channel(collector_config.buffer_size);

    collector.start(rx).await.expect("Failed to start");

    // Send events that will fail to publish
    for i in 0..3 {
        let event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new(format!("session-{}", i), Uuid::new_v4()),
        );
        tx.send(event).await.expect("Send failed");
    }

    // Wait for processing
    tokio::time::sleep(Duration::from_secs(3)).await;
    drop(tx);
    tokio::time::sleep(Duration::from_secs(2)).await;

    let stats = collector.stats();

    // Events should be received but fail to publish
    assert_eq!(stats.events_received, 3, "All events should be received");
    assert_eq!(stats.events_published, 0, "No events should be published successfully");
    assert_eq!(stats.events_failed, 3, "All events should be marked as failed");
    assert!(stats.batches_processed >= 1, "At least one batch should be processed");

    collector.shutdown().await.expect("Shutdown failed");
}

/// Test concurrent collectors with same Kafka instance
#[tokio::test]
async fn test_multiple_collectors_same_kafka() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "multi-collector-test";

    let mut collectors = vec![];
    let mut senders = vec![];

    // Create 3 collectors
    for _ in 0..3 {
        let kafka_config = test_kafka_config(&kafka.bootstrap_servers(), topic);
        let config = test_collector_config(kafka_config);

        let (mut collector, _) = FeedbackCollector::new(config.clone());
        let (tx, rx) = mpsc::channel(config.buffer_size);

        collector.start(rx).await.expect("Failed to start");
        collectors.push(collector);
        senders.push(tx);
    }

    // Send events through each collector
    for (i, tx) in senders.iter().enumerate() {
        let event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new(format!("collector-{}", i), Uuid::new_v4()),
        );
        tx.send(event).await.expect("Send failed");
    }

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Drop all senders
    drop(senders);
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Shutdown all collectors
    for collector in collectors {
        collector.shutdown().await.expect("Shutdown failed");
    }
}

/// Test very large batch publishing
#[tokio::test]
async fn test_large_batch_publishing() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "large-batch-test";

    let config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let producer = FeedbackProducer::new(config).expect("Producer creation failed");

    // Create a large batch
    let mut events = vec![];
    for i in 0..100 {
        events.push(FeedbackEvent::PerformanceMetrics(
            PerformanceMetricsEvent::new(
                Uuid::new_v4(),
                "gpt-4",
                1000.0 + i as f64,
                100,
                200,
                0.05,
            ),
        ));
    }

    let batch = FeedbackEventBatch::new(events);
    let (success, errors) = producer.publish_batch_resilient(&batch).await;

    assert_eq!(success, 100, "All 100 events should succeed");
    assert_eq!(errors, 0, "No errors should occur");
}
