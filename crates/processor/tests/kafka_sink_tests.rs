//! Integration tests for KafkaSink
//!
//! These tests require a running Kafka instance.
//! To run: docker-compose up -d kafka
//! Then: cargo test --test kafka_sink_tests -- --ignored

use processor::kafka::{
    BincodeSerializer, DeliveryGuarantee, JsonSerializer, KafkaSink, KafkaSinkConfig,
    PartitionStrategy, SinkMessage,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestEvent {
    id: String,
    value: i32,
    timestamp: i64,
}

fn get_test_config() -> KafkaSinkConfig {
    KafkaSinkConfig {
        brokers: std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string()),
        topic: "test-sink-topic".to_string(),
        client_id: "test-producer".to_string(),
        enable_idempotence: true,
        max_retries: 3,
        send_timeout_ms: 5000,
        ..Default::default()
    }
}

#[tokio::test]
#[ignore] // Requires Kafka
async fn test_send_single_message() {
    let config = get_test_config();
    let sink = KafkaSink::<TestEvent>::new(config).await.unwrap();

    let event = TestEvent {
        id: "test-1".to_string(),
        value: 42,
        timestamp: chrono::Utc::now().timestamp_millis(),
    };

    let message = SinkMessage::new(event.clone())
        .with_key("test-key".to_string());

    let result = sink.send(message).await;
    assert!(result.is_ok());

    let metrics = sink.metrics().await;
    assert_eq!(metrics.messages_sent, 1);
    assert_eq!(metrics.messages_failed, 0);

    sink.close().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires Kafka
async fn test_send_batch() {
    let config = get_test_config();
    let sink = KafkaSink::<TestEvent>::new(config).await.unwrap();

    let messages: Vec<_> = (0..10)
        .map(|i| {
            let event = TestEvent {
                id: format!("test-{}", i),
                value: i,
                timestamp: chrono::Utc::now().timestamp_millis(),
            };
            SinkMessage::new(event).with_key(format!("key-{}", i))
        })
        .collect();

    let results = sink.send_batch(messages).await.unwrap();
    assert_eq!(results.len(), 10);
    assert!(results.iter().all(|r| r.is_ok()));

    let metrics = sink.metrics().await;
    assert_eq!(metrics.messages_sent, 10);

    sink.close().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires Kafka
async fn test_transactional_send() {
    let mut config = get_test_config();
    config.enable_transactions = true;
    config.transactional_id = Some("test-txn-producer".to_string());

    let sink = KafkaSink::<TestEvent>::new(config)
        .await
        .unwrap()
        .with_delivery_guarantee(DeliveryGuarantee::ExactlyOnce);

    // Begin transaction
    sink.begin_transaction().await.unwrap();

    // Send messages
    for i in 0..5 {
        let event = TestEvent {
            id: format!("txn-{}", i),
            value: i,
            timestamp: chrono::Utc::now().timestamp_millis(),
        };
        let message = SinkMessage::new(event);
        sink.send(message).await.unwrap();
    }

    // Commit transaction
    sink.commit_transaction().await.unwrap();

    let metrics = sink.metrics().await;
    assert_eq!(metrics.messages_sent, 5);

    sink.close().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires Kafka
async fn test_transaction_abort() {
    let mut config = get_test_config();
    config.enable_transactions = true;
    config.transactional_id = Some("test-abort-producer".to_string());

    let sink = KafkaSink::<TestEvent>::new(config).await.unwrap();

    // Begin transaction
    sink.begin_transaction().await.unwrap();

    // Send a message
    let event = TestEvent {
        id: "abort-test".to_string(),
        value: 999,
        timestamp: chrono::Utc::now().timestamp_millis(),
    };
    let message = SinkMessage::new(event);
    sink.send(message).await.unwrap();

    // Abort transaction
    sink.abort_transaction().await.unwrap();

    // Messages should not be committed
    let metrics = sink.metrics().await;
    // Note: message was sent but transaction was aborted
    // The sent counter is incremented, but Kafka won't commit it
    assert!(metrics.messages_sent > 0);

    sink.close().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires Kafka
async fn test_bincode_serialization() {
    let config = get_test_config();
    let sink = KafkaSink::<TestEvent>::with_serializer(config, Arc::new(BincodeSerializer))
        .await
        .unwrap();

    let event = TestEvent {
        id: "bincode-test".to_string(),
        value: 123,
        timestamp: chrono::Utc::now().timestamp_millis(),
    };

    let message = SinkMessage::new(event);
    sink.send(message).await.unwrap();

    let metrics = sink.metrics().await;
    assert_eq!(metrics.messages_sent, 1);
    // Bincode should be more compact than JSON
    assert!(metrics.bytes_sent > 0);

    sink.close().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires Kafka
async fn test_message_with_headers() {
    let config = get_test_config();
    let sink = KafkaSink::<TestEvent>::new(config).await.unwrap();

    let event = TestEvent {
        id: "header-test".to_string(),
        value: 42,
        timestamp: chrono::Utc::now().timestamp_millis(),
    };

    let message = SinkMessage::new(event)
        .with_key("test-key".to_string())
        .with_header("trace-id".to_string(), "12345".to_string())
        .with_header("span-id".to_string(), "67890".to_string())
        .with_header("environment".to_string(), "test".to_string());

    sink.send(message).await.unwrap();

    let metrics = sink.metrics().await;
    assert_eq!(metrics.messages_sent, 1);

    sink.close().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires Kafka
async fn test_custom_partition() {
    let config = get_test_config();
    let sink = KafkaSink::<TestEvent>::new(config)
        .await
        .unwrap()
        .with_partition_strategy(PartitionStrategy::Custom);

    // Send to specific partitions
    for partition in 0..3 {
        let event = TestEvent {
            id: format!("partition-{}", partition),
            value: partition,
            timestamp: chrono::Utc::now().timestamp_millis(),
        };

        let message = SinkMessage::new(event).with_partition(partition);

        sink.send(message).await.unwrap();
    }

    let metrics = sink.metrics().await;
    assert_eq!(metrics.messages_sent, 3);

    sink.close().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires Kafka
async fn test_metrics_tracking() {
    let config = get_test_config();
    let sink = KafkaSink::<TestEvent>::new(config).await.unwrap();

    // Initial metrics
    let initial_metrics = sink.metrics().await;
    assert_eq!(initial_metrics.messages_sent, 0);
    assert_eq!(initial_metrics.messages_failed, 0);

    // Send messages
    for i in 0..5 {
        let event = TestEvent {
            id: format!("metrics-{}", i),
            value: i,
            timestamp: chrono::Utc::now().timestamp_millis(),
        };
        let message = SinkMessage::new(event);
        sink.send(message).await.unwrap();
    }

    // Check updated metrics
    let metrics = sink.metrics().await;
    assert_eq!(metrics.messages_sent, 5);
    assert_eq!(metrics.send_attempts, 5);
    assert!(metrics.bytes_sent > 0);
    assert!(metrics.avg_latency_us > 0);
    assert!(metrics.max_latency_us >= metrics.avg_latency_us);

    sink.close().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires Kafka
async fn test_flush() {
    let config = get_test_config();
    let sink = KafkaSink::<TestEvent>::new(config).await.unwrap();

    // Send messages asynchronously
    for i in 0..10 {
        let event = TestEvent {
            id: format!("flush-{}", i),
            value: i,
            timestamp: chrono::Utc::now().timestamp_millis(),
        };
        let message = SinkMessage::new(event);
        sink.send(message).await.unwrap();
    }

    // Flush to ensure all messages are sent
    sink.flush().await.unwrap();

    let metrics = sink.metrics().await;
    assert_eq!(metrics.messages_sent, 10);

    sink.close().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires Kafka
async fn test_concurrent_sends() {
    let config = get_test_config();
    let sink = Arc::new(KafkaSink::<TestEvent>::new(config).await.unwrap());

    let mut handles = vec![];

    // Spawn multiple concurrent senders
    for i in 0..10 {
        let sink_clone = Arc::clone(&sink);
        let handle = tokio::spawn(async move {
            let event = TestEvent {
                id: format!("concurrent-{}", i),
                value: i,
                timestamp: chrono::Utc::now().timestamp_millis(),
            };
            let message = SinkMessage::new(event);
            sink_clone.send(message).await
        });
        handles.push(handle);
    }

    // Wait for all sends to complete
    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    let metrics = sink.metrics().await;
    assert_eq!(metrics.messages_sent, 10);

    sink.close().await.unwrap();
}

#[tokio::test]
async fn test_config_validation() {
    // Test that transactions require transactional_id
    let mut config = get_test_config();
    config.enable_transactions = true;
    config.transactional_id = None;

    let result = KafkaSink::<TestEvent>::new(config).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_config_defaults() {
    let config = KafkaSinkConfig::default();

    assert_eq!(config.brokers, "localhost:9092");
    assert_eq!(config.topic, "llm-optimizer-results");
    assert!(config.enable_idempotence);
    assert!(!config.enable_transactions);
    assert_eq!(config.compression_type, "snappy");
    assert_eq!(config.acks, "all");
    assert_eq!(config.max_retries, 5);
}

#[tokio::test]
async fn test_partition_strategy() {
    assert_eq!(PartitionStrategy::KeyHash, PartitionStrategy::KeyHash);
    assert_ne!(PartitionStrategy::KeyHash, PartitionStrategy::RoundRobin);
}

#[tokio::test]
async fn test_delivery_guarantee() {
    assert_eq!(
        DeliveryGuarantee::AtLeastOnce,
        DeliveryGuarantee::AtLeastOnce
    );
    assert_ne!(
        DeliveryGuarantee::AtLeastOnce,
        DeliveryGuarantee::ExactlyOnce
    );
}
