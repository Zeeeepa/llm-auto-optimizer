//! Integration tests for Kafka producer functionality
//!
//! These tests use testcontainers to spin up a real Kafka instance
//! and verify that the FeedbackProducer can publish events correctly.

mod common;

use collector::{
    FeedbackEvent, FeedbackEventBatch, FeedbackProducer, PerformanceMetricsEvent,
    UserFeedbackEvent,
};
use common::{test_kafka_config, KafkaTestContainer};
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::Message;
use rdkafka::ClientConfig;
use std::time::Duration;
use testcontainers::clients::Cli;
use uuid::Uuid;

/// Test that we can create a Kafka producer with real Kafka
#[tokio::test]
async fn test_kafka_producer_creation() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);

    let config = test_kafka_config(&kafka.bootstrap_servers(), "test-events");
    let producer = FeedbackProducer::new(config);

    assert!(producer.is_ok());
}

/// Test publishing a single user feedback event to Kafka
#[tokio::test]
async fn test_publish_single_user_feedback_event() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "user-feedback-events";

    // Create producer
    let config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let producer = FeedbackProducer::new(config).expect("Failed to create producer");

    // Create event
    let request_id = Uuid::new_v4();
    let event = FeedbackEvent::UserFeedback(
        UserFeedbackEvent::new("session123", request_id)
            .with_user_id("user456")
            .with_rating(4.5)
            .with_liked(true),
    );

    // Publish event
    let result = producer.publish_event(&event).await;
    assert!(result.is_ok(), "Failed to publish event: {:?}", result);

    // Flush to ensure event is sent
    let flush_result = producer.flush(5000).await;
    assert!(flush_result.is_ok(), "Failed to flush: {:?}", flush_result);

    // Verify event was published by consuming it
    let consumer = create_test_consumer(&kafka.bootstrap_servers(), topic, "test-group-1");
    let consumed_event = consume_single_event(&consumer, Duration::from_secs(10)).await;

    assert!(consumed_event.is_some(), "No event was consumed");
    let consumed_event = consumed_event.unwrap();

    // Verify event content
    let deserialized: FeedbackEvent =
        serde_json::from_slice(consumed_event.payload().unwrap()).expect("Failed to deserialize");
    assert_eq!(deserialized, event);
}

/// Test publishing a performance metrics event
#[tokio::test]
async fn test_publish_performance_metrics_event() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "performance-events";

    let config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let producer = FeedbackProducer::new(config).expect("Failed to create producer");

    let request_id = Uuid::new_v4();
    let event = FeedbackEvent::PerformanceMetrics(
        PerformanceMetricsEvent::new(request_id, "gpt-4", 1500.0, 150, 300, 0.08)
            .with_quality_score(0.95),
    );

    let result = producer.publish_event(&event).await;
    assert!(result.is_ok());

    producer.flush(5000).await.expect("Failed to flush");

    // Verify by consuming
    let consumer = create_test_consumer(&kafka.bootstrap_servers(), topic, "test-group-2");
    let consumed_event = consume_single_event(&consumer, Duration::from_secs(10)).await;
    assert!(consumed_event.is_some());
}

/// Test publishing a batch of events
#[tokio::test]
async fn test_publish_event_batch() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "batch-events";

    let config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let producer = FeedbackProducer::new(config).expect("Failed to create producer");

    // Create batch of events
    let events = vec![
        FeedbackEvent::UserFeedback(UserFeedbackEvent::new("s1", Uuid::new_v4())),
        FeedbackEvent::UserFeedback(UserFeedbackEvent::new("s2", Uuid::new_v4())),
        FeedbackEvent::PerformanceMetrics(PerformanceMetricsEvent::new(
            Uuid::new_v4(),
            "gpt-3.5-turbo",
            800.0,
            100,
            150,
            0.02,
        )),
    ];

    let batch = FeedbackEventBatch::new(events.clone());
    let results = producer.publish_batch(&batch).await;

    assert!(results.is_ok());
    let results = results.unwrap();
    assert_eq!(results.len(), 3);

    // All should succeed
    for result in results {
        assert!(result.is_ok(), "One of the events failed to publish: {:?}", result);
    }

    producer.flush(5000).await.expect("Failed to flush");

    // Verify all events were published
    let consumer = create_test_consumer(&kafka.bootstrap_servers(), topic, "test-group-3");
    let mut consumed_count = 0;

    for _ in 0..3 {
        if consume_single_event(&consumer, Duration::from_secs(10)).await.is_some() {
            consumed_count += 1;
        }
    }

    assert_eq!(consumed_count, 3, "Expected 3 events but got {}", consumed_count);
}

/// Test resilient batch publishing (handles partial failures gracefully)
#[tokio::test]
async fn test_publish_batch_resilient() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "resilient-batch-events";

    let config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let producer = FeedbackProducer::new(config).expect("Failed to create producer");

    let events = vec![
        FeedbackEvent::UserFeedback(UserFeedbackEvent::new("s1", Uuid::new_v4())),
        FeedbackEvent::UserFeedback(UserFeedbackEvent::new("s2", Uuid::new_v4())),
        FeedbackEvent::UserFeedback(UserFeedbackEvent::new("s3", Uuid::new_v4())),
    ];

    let batch = FeedbackEventBatch::new(events);
    let (success_count, error_count) = producer.publish_batch_resilient(&batch).await;

    // All should succeed with a working Kafka
    assert_eq!(success_count, 3);
    assert_eq!(error_count, 0);

    producer.flush(5000).await.expect("Failed to flush");
}

/// Test event headers are set correctly
#[tokio::test]
async fn test_event_headers() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "headers-test";

    let config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let producer = FeedbackProducer::new(config).expect("Failed to create producer");

    let request_id = Uuid::new_v4();
    let event = FeedbackEvent::UserFeedback(UserFeedbackEvent::new("session-abc", request_id));
    let event_id = event.id().to_string();

    producer.publish_event(&event).await.expect("Failed to publish");
    producer.flush(5000).await.expect("Failed to flush");

    // Consume and check headers
    let consumer = create_test_consumer(&kafka.bootstrap_servers(), topic, "test-group-4");
    let consumed_message = consume_single_event(&consumer, Duration::from_secs(10)).await;

    assert!(consumed_message.is_some());
    let message = consumed_message.unwrap();

    // Check headers
    let headers = message.headers().expect("No headers found");

    let mut found_event_type = false;
    let mut found_event_id = false;
    let mut found_timestamp = false;

    for header in headers.iter() {
        match header.key {
            "event_type" => {
                let value = std::str::from_utf8(header.value.unwrap()).unwrap();
                assert_eq!(value, "user_feedback");
                found_event_type = true;
            }
            "event_id" => {
                let value = std::str::from_utf8(header.value.unwrap()).unwrap();
                assert_eq!(value, event_id);
                found_event_id = true;
            }
            "timestamp" => {
                found_timestamp = true;
            }
            _ => {}
        }
    }

    assert!(found_event_type, "event_type header not found");
    assert!(found_event_id, "event_id header not found");
    assert!(found_timestamp, "timestamp header not found");
}

/// Test that event key is set to event ID
#[tokio::test]
async fn test_event_key_partitioning() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "partition-test";

    let config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let producer = FeedbackProducer::new(config).expect("Failed to create producer");

    let event = FeedbackEvent::UserFeedback(UserFeedbackEvent::new("session", Uuid::new_v4()));
    let event_id = event.id().to_string();

    producer.publish_event(&event).await.expect("Failed to publish");
    producer.flush(5000).await.expect("Failed to flush");

    let consumer = create_test_consumer(&kafka.bootstrap_servers(), topic, "test-group-5");
    let message = consume_single_event(&consumer, Duration::from_secs(10)).await;

    assert!(message.is_some());
    let message = message.unwrap();

    let key = message.key().expect("No key found");
    let key_str = std::str::from_utf8(key).unwrap();
    assert_eq!(key_str, event_id);
}

/// Test concurrent publishing from multiple producers
#[tokio::test]
async fn test_concurrent_publishing() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "concurrent-test";

    let config = test_kafka_config(&kafka.bootstrap_servers(), topic);

    // Spawn multiple tasks publishing concurrently
    let mut handles = vec![];
    for i in 0..5 {
        let config_clone = config.clone();
        let handle = tokio::spawn(async move {
            let producer = FeedbackProducer::new(config_clone).expect("Failed to create producer");
            let event = FeedbackEvent::UserFeedback(
                UserFeedbackEvent::new(format!("session-{}", i), Uuid::new_v4())
            );
            producer.publish_event(&event).await.expect("Failed to publish");
            producer.flush(5000).await.expect("Failed to flush");
        });
        handles.push(handle);
    }

    // Wait for all to complete
    for handle in handles {
        handle.await.expect("Task failed");
    }

    // Verify all 5 events were published
    let consumer = create_test_consumer(&kafka.bootstrap_servers(), topic, "test-group-6");
    let mut count = 0;
    for _ in 0..5 {
        if consume_single_event(&consumer, Duration::from_secs(10)).await.is_some() {
            count += 1;
        }
    }
    assert_eq!(count, 5);
}

// Helper functions

/// Create a test Kafka consumer
fn create_test_consumer(bootstrap_servers: &str, topic: &str, group_id: &str) -> StreamConsumer {
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", bootstrap_servers)
        .set("group.id", group_id)
        .set("auto.offset.reset", "earliest")
        .set("enable.auto.commit", "false")
        .create()
        .expect("Failed to create consumer");

    consumer
        .subscribe(&[topic])
        .expect("Failed to subscribe to topic");

    consumer
}

/// Consume a single event from Kafka with timeout
async fn consume_single_event(
    consumer: &StreamConsumer,
    timeout: Duration,
) -> Option<rdkafka::message::BorrowedMessage<'_>> {
    use futures::StreamExt;

    let stream = consumer.stream();
    let mut stream = stream;

    match tokio::time::timeout(timeout, stream.next()).await {
        Ok(Some(Ok(message))) => Some(message),
        _ => None,
    }
}
