//! Tests for Kafka integration
//!
//! Note: These are unit tests that don't require a running Kafka cluster.
//! For integration tests, see the examples directory.

use super::*;
use crate::core::{EventKey, EventTimeExtractor, KeyExtractor, ProcessorEvent};
use crate::pipeline::StreamPipelineBuilder;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestEvent {
    id: String,
    value: f64,
    timestamp: i64,
}

impl EventTimeExtractor<TestEvent> for TestEvent {
    fn extract_event_time(&self, event: &TestEvent) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(event.timestamp).unwrap_or_else(Utc::now)
    }
}

impl KeyExtractor<TestEvent> for TestEvent {
    fn extract_key(&self, event: &TestEvent) -> EventKey {
        EventKey::Session(event.id.clone())
    }
}

#[test]
fn test_kafka_stream_stats_creation() {
    let stats = KafkaStreamStats::new();

    assert_eq!(stats.messages_consumed, 0);
    assert_eq!(stats.messages_produced, 0);
    assert_eq!(stats.consumer_lag, 0);
    assert_eq!(stats.consume_errors, 0);
    assert_eq!(stats.produce_errors, 0);
    assert_eq!(stats.offsets_committed, 0);
    assert_eq!(stats.commit_errors, 0);
    assert!(stats.committed_offsets.is_empty());
}

#[test]
fn test_kafka_stream_stats_increments() {
    let mut stats = KafkaStreamStats::new();

    stats.inc_consumed();
    assert_eq!(stats.messages_consumed, 1);

    stats.inc_produced();
    assert_eq!(stats.messages_produced, 1);

    stats.inc_consume_errors();
    assert_eq!(stats.consume_errors, 1);

    stats.inc_produce_errors();
    assert_eq!(stats.produce_errors, 1);

    stats.inc_offsets_committed();
    assert_eq!(stats.offsets_committed, 1);

    stats.inc_commit_errors();
    assert_eq!(stats.commit_errors, 1);
}

#[test]
fn test_kafka_stream_stats_offset_tracking() {
    let mut stats = KafkaStreamStats::new();

    stats.update_committed_offset(0, 100);
    stats.update_committed_offset(1, 200);
    stats.update_committed_offset(0, 150); // Update partition 0

    assert_eq!(stats.committed_offsets.get(&0), Some(&150));
    assert_eq!(stats.committed_offsets.get(&1), Some(&200));
    assert_eq!(stats.committed_offsets.get(&2), None);
}

#[test]
fn test_kafka_stream_config_default() {
    let config = KafkaStreamConfig::default();

    assert_eq!(config.brokers, "localhost:9092");
    assert_eq!(config.group_id, "stream-processor");
    assert_eq!(config.input_topics.len(), 1);
    assert_eq!(config.input_topics[0], "input");
    assert_eq!(config.output_topic, "output");
    assert_eq!(config.auto_commit_interval_ms, 5000);
    assert_eq!(config.poll_timeout_ms, 100);
    assert_eq!(config.delivery_timeout_ms, 5000);
    assert_eq!(config.max_produce_retries, 3);
    assert!(config.consumer_config.is_empty());
    assert!(config.producer_config.is_empty());
}

#[test]
fn test_kafka_stream_config_custom() {
    let mut consumer_config = HashMap::new();
    consumer_config.insert("session.timeout.ms".to_string(), "30000".to_string());

    let mut producer_config = HashMap::new();
    producer_config.insert("compression.type".to_string(), "lz4".to_string());

    let config = KafkaStreamConfig {
        brokers: "kafka:9093".to_string(),
        group_id: "custom-group".to_string(),
        input_topics: vec!["topic1".to_string(), "topic2".to_string()],
        output_topic: "results".to_string(),
        auto_commit_interval_ms: 10_000,
        poll_timeout_ms: 200,
        delivery_timeout_ms: 10_000,
        max_produce_retries: 5,
        consumer_config,
        producer_config,
    };

    assert_eq!(config.brokers, "kafka:9093");
    assert_eq!(config.group_id, "custom-group");
    assert_eq!(config.input_topics.len(), 2);
    assert_eq!(config.output_topic, "results");
    assert_eq!(config.auto_commit_interval_ms, 10_000);
    assert_eq!(config.max_produce_retries, 5);
    assert_eq!(
        config.consumer_config.get("session.timeout.ms"),
        Some(&"30000".to_string())
    );
    assert_eq!(
        config.producer_config.get("compression.type"),
        Some(&"lz4".to_string())
    );
}

#[test]
fn test_kafka_stream_stats_serialization() {
    let mut stats = KafkaStreamStats::new();
    stats.inc_consumed();
    stats.inc_produced();
    stats.update_committed_offset(0, 100);

    // Serialize
    let json = serde_json::to_string(&stats).unwrap();

    // Deserialize
    let deserialized: KafkaStreamStats = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.messages_consumed, 1);
    assert_eq!(deserialized.messages_produced, 1);
    assert_eq!(deserialized.committed_offsets.get(&0), Some(&100));
}

#[tokio::test]
async fn test_test_event_extractors() {
    let event = TestEvent {
        id: "test-123".to_string(),
        value: 42.5,
        timestamp: 1699564800000,
    };

    // Test time extraction
    let time = event.extract_event_time(&event);
    assert_eq!(time.timestamp_millis(), 1699564800000);

    // Test key extraction
    let key = event.extract_key(&event);
    match key {
        EventKey::Session(id) => assert_eq!(id, "test-123"),
        _ => panic!("Expected Session key"),
    }
}

#[test]
fn test_kafka_stream_config_builder() {
    let config = KafkaStreamConfig {
        brokers: "localhost:9092,localhost:9093".to_string(),
        group_id: "test-processor".to_string(),
        input_topics: vec!["events".to_string()],
        output_topic: "aggregates".to_string(),
        auto_commit_interval_ms: 1000,
        poll_timeout_ms: 50,
        delivery_timeout_ms: 3000,
        max_produce_retries: 10,
        consumer_config: {
            let mut cfg = HashMap::new();
            cfg.insert("enable.partition.eof".to_string(), "true".to_string());
            cfg
        },
        producer_config: {
            let mut cfg = HashMap::new();
            cfg.insert("acks".to_string(), "all".to_string());
            cfg
        },
    };

    assert!(config.brokers.contains("localhost:9092"));
    assert!(config.brokers.contains("localhost:9093"));
    assert_eq!(config.input_topics[0], "events");
    assert_eq!(config.output_topic, "aggregates");
}

#[test]
fn test_kafka_stream_stats_clone() {
    let mut stats1 = KafkaStreamStats::new();
    stats1.inc_consumed();
    stats1.inc_produced();

    let stats2 = stats1.clone();

    assert_eq!(stats2.messages_consumed, stats1.messages_consumed);
    assert_eq!(stats2.messages_produced, stats1.messages_produced);
}

#[test]
fn test_kafka_stream_stats_default() {
    let stats = KafkaStreamStats::default();

    assert_eq!(stats.messages_consumed, 0);
    assert_eq!(stats.messages_produced, 0);
}

/// Test that verifies the configuration can be created with environment variables
#[test]
fn test_kafka_config_from_env_pattern() {
    // This test demonstrates the pattern used in the example
    let brokers = std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string());
    let group_id = std::env::var("KAFKA_GROUP_ID").unwrap_or_else(|_| "default-group".to_string());

    // Should use defaults when env vars are not set
    assert_eq!(brokers, "localhost:9092");
    assert_eq!(group_id, "default-group");
}

#[tokio::test]
async fn test_pipeline_integration_setup() {
    // Test that we can create a pipeline and executor for Kafka integration
    let pipeline = StreamPipelineBuilder::new()
        .with_name("test-kafka-pipeline")
        .with_tumbling_window(60_000)
        .build()
        .unwrap();

    let executor = pipeline.create_executor::<TestEvent>();

    // Test event ingestion
    let event = TestEvent {
        id: "test-1".to_string(),
        value: 100.0,
        timestamp: Utc::now().timestamp_millis(),
    };

    executor.ingest(event).await.unwrap();

    // Verify stats
    let stats = executor.stats().await;
    assert_eq!(stats.events_processed, 0); // Not processed yet as executor is not running
}

#[test]
fn test_kafka_config_validation() {
    let config = KafkaStreamConfig::default();

    // Validate brokers are set
    assert!(!config.brokers.is_empty());

    // Validate group ID is set
    assert!(!config.group_id.is_empty());

    // Validate at least one input topic
    assert!(!config.input_topics.is_empty());

    // Validate output topic is set
    assert!(!config.output_topic.is_empty());

    // Validate timeouts are reasonable
    assert!(config.auto_commit_interval_ms > 0);
    assert!(config.poll_timeout_ms > 0);
    assert!(config.delivery_timeout_ms > 0);

    // Validate retries
    assert!(config.max_produce_retries > 0);
}

#[test]
fn test_multiple_input_topics() {
    let config = KafkaStreamConfig {
        input_topics: vec![
            "topic-a".to_string(),
            "topic-b".to_string(),
            "topic-c".to_string(),
        ],
        ..Default::default()
    };

    assert_eq!(config.input_topics.len(), 3);
    assert!(config.input_topics.contains(&"topic-a".to_string()));
    assert!(config.input_topics.contains(&"topic-b".to_string()));
    assert!(config.input_topics.contains(&"topic-c".to_string()));
}

#[tokio::test]
async fn test_stats_concurrent_updates() {
    let stats = Arc::new(RwLock::new(KafkaStreamStats::new()));

    // Simulate concurrent updates
    let handles: Vec<_> = (0..10)
        .map(|_| {
            let stats = Arc::clone(&stats);
            tokio::spawn(async move {
                for _ in 0..100 {
                    stats.write().await.inc_consumed();
                    stats.write().await.inc_produced();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.await.unwrap();
    }

    let final_stats = stats.read().await;
    assert_eq!(final_stats.messages_consumed, 1000);
    assert_eq!(final_stats.messages_produced, 1000);
}

#[test]
fn test_processor_event_with_test_event() {
    let event = TestEvent {
        id: "event-1".to_string(),
        value: 42.0,
        timestamp: 1699564800000,
    };

    let event_time = event.extract_event_time(&event);
    let key = event.extract_key(&event);

    let processor_event = ProcessorEvent::new(event, event_time, key.to_key_string());

    assert_eq!(processor_event.event.id, "event-1");
    assert_eq!(processor_event.event.value, 42.0);
    assert!(processor_event.key.contains("session:event-1"));
}
