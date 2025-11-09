//! End-to-end integration tests for FeedbackCollector
//!
//! These tests verify the complete flow from event submission through
//! batching, processing, and publishing to Kafka.

mod common;

use collector::{
    FeedbackCollector, FeedbackEvent, PerformanceMetricsEvent, SystemHealthEvent,
    UserFeedbackEvent, HealthStatus,
};
use common::{test_collector_config, test_kafka_config, wait_for_condition, KafkaTestContainer};
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::ClientConfig;
use std::time::Duration;
use testcontainers::clients::Cli;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Test basic collector creation and startup
#[tokio::test]
async fn test_collector_creation_and_startup() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);

    let kafka_config = test_kafka_config(&kafka.bootstrap_servers(), "test-events");
    let config = test_collector_config(kafka_config);

    let (mut collector, _) = FeedbackCollector::new(config.clone());
    let (tx, rx) = mpsc::channel(config.buffer_size);

    let result = collector.start(rx).await;
    assert!(result.is_ok(), "Failed to start collector: {:?}", result);

    // Cleanup
    drop(tx);
    let shutdown_result = collector.shutdown().await;
    assert!(shutdown_result.is_ok(), "Failed to shutdown: {:?}", shutdown_result);
}

/// Test end-to-end event submission and Kafka publishing
#[tokio::test]
async fn test_end_to_end_event_flow() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "e2e-events";

    let kafka_config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let config = test_collector_config(kafka_config);

    let (mut collector, _) = FeedbackCollector::new(config.clone());
    let (tx, rx) = mpsc::channel(config.buffer_size);

    collector.start(rx).await.expect("Failed to start collector");

    // Submit events through the sender
    let request_id = Uuid::new_v4();
    let event1 = FeedbackEvent::UserFeedback(
        UserFeedbackEvent::new("session-1", request_id).with_rating(4.5),
    );
    let event2 = FeedbackEvent::PerformanceMetrics(PerformanceMetricsEvent::new(
        request_id,
        "gpt-4",
        1200.0,
        120,
        250,
        0.06,
    ));

    tx.send(event1.clone()).await.expect("Failed to send event1");
    tx.send(event2.clone()).await.expect("Failed to send event2");

    // Wait a bit for processing
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Drop sender to trigger flush
    drop(tx);

    // Give time for flush
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify events in Kafka
    let consumer = create_test_consumer(&kafka.bootstrap_servers(), topic, "e2e-group");
    let mut consumed_events = vec![];

    for _ in 0..2 {
        if let Some(msg) = consume_single_event(&consumer, Duration::from_secs(10)).await {
            if let Some(payload) = msg.payload() {
                let event: FeedbackEvent = serde_json::from_slice(payload).unwrap();
                consumed_events.push(event);
            }
        }
    }

    assert_eq!(consumed_events.len(), 2, "Expected 2 events in Kafka");

    // Shutdown
    collector.shutdown().await.expect("Failed to shutdown");
}

/// Test collector statistics tracking
#[tokio::test]
async fn test_collector_statistics() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "stats-events";

    let kafka_config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let config = test_collector_config(kafka_config);

    let (mut collector, _) = FeedbackCollector::new(config.clone());
    let (tx, rx) = mpsc::channel(config.buffer_size);

    collector.start(rx).await.expect("Failed to start");

    // Initial stats
    let initial_stats = collector.stats();
    assert_eq!(initial_stats.events_received, 0);
    assert_eq!(initial_stats.events_published, 0);

    // Send multiple events
    for i in 0..5 {
        let event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new(format!("session-{}", i), Uuid::new_v4()),
        );
        tx.send(event).await.expect("Failed to send event");
    }

    // Wait for processing
    tokio::time::sleep(Duration::from_secs(3)).await;
    drop(tx);
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Check stats
    let final_stats = collector.stats();
    assert_eq!(final_stats.events_received, 5, "Expected 5 events received");
    assert_eq!(final_stats.events_published, 5, "Expected 5 events published");
    assert!(final_stats.batches_processed >= 1, "Expected at least 1 batch");

    collector.shutdown().await.expect("Failed to shutdown");
}

/// Test batch size triggering
#[tokio::test]
async fn test_batch_size_flush_trigger() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "batch-size-events";

    let kafka_config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let mut config = test_collector_config(kafka_config);
    config.max_batch_size = 5; // Small batch size for testing

    let (mut collector, _) = FeedbackCollector::new(config.clone());
    let (tx, rx) = mpsc::channel(config.buffer_size);

    collector.start(rx).await.expect("Failed to start");

    // Send exactly batch_size events
    for i in 0..5 {
        let event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new(format!("session-{}", i), Uuid::new_v4()),
        );
        tx.send(event).await.expect("Failed to send");
    }

    // Wait for batch to be flushed
    tokio::time::sleep(Duration::from_secs(2)).await;

    let stats = collector.stats();
    assert_eq!(stats.batches_processed, 1, "Expected 1 batch to be flushed");

    drop(tx);
    collector.shutdown().await.expect("Failed to shutdown");
}

/// Test batch age triggering
#[tokio::test]
async fn test_batch_age_flush_trigger() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "batch-age-events";

    let kafka_config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let mut config = test_collector_config(kafka_config);
    config.max_batch_size = 100; // Large batch size
    config.max_batch_age_secs = 2; // Short age for testing
    config.flush_interval_ms = 500; // Frequent flush checks

    let (mut collector, _) = FeedbackCollector::new(config.clone());
    let (tx, rx) = mpsc::channel(config.buffer_size);

    collector.start(rx).await.expect("Failed to start");

    // Send just 2 events (won't trigger batch size)
    for i in 0..2 {
        let event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new(format!("session-{}", i), Uuid::new_v4()),
        );
        tx.send(event).await.expect("Failed to send");
    }

    // Wait longer than max_batch_age_secs
    tokio::time::sleep(Duration::from_secs(4)).await;

    let stats = collector.stats();
    assert_eq!(stats.events_published, 2, "Expected 2 events published");
    assert!(stats.batches_processed >= 1, "Expected at least 1 batch due to age");

    drop(tx);
    collector.shutdown().await.expect("Failed to shutdown");
}

/// Test graceful shutdown with pending events
#[tokio::test]
async fn test_graceful_shutdown_with_pending_events() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "shutdown-events";

    let kafka_config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let mut config = test_collector_config(kafka_config);
    config.max_batch_size = 100; // Large batch - won't flush automatically

    let (mut collector, _) = FeedbackCollector::new(config.clone());
    let (tx, rx) = mpsc::channel(config.buffer_size);

    collector.start(rx).await.expect("Failed to start");

    // Send some events
    for i in 0..3 {
        let event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new(format!("session-{}", i), Uuid::new_v4()),
        );
        tx.send(event).await.expect("Failed to send");
    }

    // Immediately close channel and shutdown
    drop(tx);
    collector.shutdown().await.expect("Failed to shutdown");

    // Verify all events were flushed during shutdown
    let consumer = create_test_consumer(&kafka.bootstrap_servers(), topic, "shutdown-group");
    let mut count = 0;
    for _ in 0..3 {
        if consume_single_event(&consumer, Duration::from_secs(5)).await.is_some() {
            count += 1;
        }
    }

    assert_eq!(count, 3, "All events should be flushed during shutdown");
}

/// Test concurrent event submission
#[tokio::test]
async fn test_concurrent_event_submission() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "concurrent-collector-events";

    let kafka_config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let config = test_collector_config(kafka_config);

    let (mut collector, _) = FeedbackCollector::new(config.clone());
    let (tx, rx) = mpsc::channel(config.buffer_size);

    collector.start(rx).await.expect("Failed to start");

    // Spawn multiple tasks submitting concurrently
    let mut handles = vec![];
    for i in 0..10 {
        let tx_clone = tx.clone();
        let handle = tokio::spawn(async move {
            for j in 0..5 {
                let event = FeedbackEvent::UserFeedback(
                    UserFeedbackEvent::new(format!("session-{}-{}", i, j), Uuid::new_v4()),
                );
                tx_clone.send(event).await.expect("Failed to send");
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.expect("Task failed");
    }

    // Wait for processing
    tokio::time::sleep(Duration::from_secs(3)).await;
    drop(tx);
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Should have processed 50 events total (10 tasks * 5 events)
    let stats = collector.stats();
    assert_eq!(stats.events_received, 50, "Expected 50 events");
    assert_eq!(stats.events_published, 50, "Expected 50 events published");

    collector.shutdown().await.expect("Failed to shutdown");
}

/// Test mixed event types in same collector
#[tokio::test]
async fn test_mixed_event_types() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "mixed-events";

    let kafka_config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let config = test_collector_config(kafka_config);

    let (mut collector, _) = FeedbackCollector::new(config.clone());
    let (tx, rx) = mpsc::channel(config.buffer_size);

    collector.start(rx).await.expect("Failed to start");

    // Send different event types
    let events = vec![
        FeedbackEvent::UserFeedback(UserFeedbackEvent::new("s1", Uuid::new_v4())),
        FeedbackEvent::PerformanceMetrics(PerformanceMetricsEvent::new(
            Uuid::new_v4(),
            "gpt-4",
            1000.0,
            100,
            200,
            0.05,
        )),
        FeedbackEvent::SystemHealth(SystemHealthEvent::new("api-server", HealthStatus::Healthy)),
    ];

    for event in &events {
        tx.send(event.clone()).await.expect("Failed to send");
    }

    tokio::time::sleep(Duration::from_secs(3)).await;
    drop(tx);
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify all event types were published
    let consumer = create_test_consumer(&kafka.bootstrap_servers(), topic, "mixed-group");
    let mut consumed_types = vec![];

    for _ in 0..3 {
        if let Some(msg) = consume_single_event(&consumer, Duration::from_secs(5)).await {
            if let Some(payload) = msg.payload() {
                let event: FeedbackEvent = serde_json::from_slice(payload).unwrap();
                consumed_types.push(event.event_type().to_string());
            }
        }
    }

    assert_eq!(consumed_types.len(), 3);
    assert!(consumed_types.contains(&"user_feedback".to_string()));
    assert!(consumed_types.contains(&"performance_metrics".to_string()));
    assert!(consumed_types.contains(&"system_health".to_string()));

    collector.shutdown().await.expect("Failed to shutdown");
}

/// Test collector with non-blocking submit (try_submit)
#[tokio::test]
async fn test_try_submit_non_blocking() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "try-submit-events";

    let kafka_config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let mut config = test_collector_config(kafka_config);
    config.buffer_size = 10; // Small buffer

    let (mut collector, _) = FeedbackCollector::new(config.clone());
    let (tx, rx) = mpsc::channel(config.buffer_size);

    collector.start(rx).await.expect("Failed to start");

    // Try submitting events without blocking
    let mut success_count = 0;
    for i in 0..5 {
        let event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new(format!("session-{}", i), Uuid::new_v4()),
        );
        if collector.try_submit(event).is_ok() {
            success_count += 1;
        }
    }

    assert_eq!(success_count, 5, "All 5 events should succeed with buffer size 10");

    drop(tx);
    tokio::time::sleep(Duration::from_secs(2)).await;
    collector.shutdown().await.expect("Failed to shutdown");
}

// Helper functions

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
        .expect("Failed to subscribe");

    consumer
}

async fn consume_single_event(
    consumer: &StreamConsumer,
    timeout: Duration,
) -> Option<rdkafka::message::BorrowedMessage<'_>> {
    use futures::StreamExt;
    let mut stream = consumer.stream();

    match tokio::time::timeout(timeout, stream.next()).await {
        Ok(Some(Ok(message))) => Some(message),
        _ => None,
    }
}
