//! Integration tests for Dead Letter Queue functionality

use collector::{
    FeedbackEvent, UserFeedbackEvent, PerformanceMetricsEvent,
    FeedbackProducer, KafkaProducerConfig,
    DLQManager, DLQConfig, DLQType, DLQEntry,
    FileDLQ, DeadLetterQueue,
};
use std::sync::Arc;
use std::path::PathBuf;
use uuid::Uuid;
use chrono::Utc;

/// Test DLQ entry creation and serialization
#[tokio::test]
async fn test_dlq_entry_creation_and_serialization() {
    let request_id = Uuid::new_v4();
    let event = FeedbackEvent::UserFeedback(
        UserFeedbackEvent::new("test_session", request_id)
            .with_rating(4.0)
    );

    let entry = DLQEntry::new(event.clone(), "Test failure".to_string());

    assert_eq!(entry.retry_count, 0);
    assert_eq!(entry.failure_reason, "Test failure");
    assert!(entry.is_ready_for_retry());

    // Test serialization
    let json = serde_json::to_string(&entry).unwrap();
    let deserialized: DLQEntry = serde_json::from_str(&json).unwrap();

    assert_eq!(entry.dlq_id, deserialized.dlq_id);
    assert_eq!(entry.retry_count, deserialized.retry_count);
}

/// Test exponential backoff calculation
#[test]
fn test_exponential_backoff() {
    let request_id = Uuid::new_v4();
    let event = FeedbackEvent::UserFeedback(UserFeedbackEvent::new("session", request_id));
    let mut entry = DLQEntry::new(event, "Initial failure".to_string());

    // Test backoff progression
    assert_eq!(entry.calculate_backoff(60), 60);   // 60 * 2^0
    entry.increment_retry("Retry 1".to_string(), 60);

    assert_eq!(entry.calculate_backoff(60), 120);  // 60 * 2^1
    entry.increment_retry("Retry 2".to_string(), 120);

    assert_eq!(entry.calculate_backoff(60), 240);  // 60 * 2^2
    entry.increment_retry("Retry 3".to_string(), 240);

    assert_eq!(entry.calculate_backoff(60), 480);  // 60 * 2^3

    assert_eq!(entry.retry_count, 3);
}

/// Test file-based DLQ add and retrieve
#[tokio::test]
async fn test_file_dlq_add_and_retrieve() {
    let temp_dir = std::env::temp_dir().join(format!("dlq_test_{}", Uuid::new_v4()));
    let mut config = DLQConfig::default();
    config.file_path = temp_dir.clone();
    config.dlq_type = DLQType::File;

    let dlq = FileDLQ::new(config).unwrap();

    // Add entries
    let request_id = Uuid::new_v4();
    let event1 = FeedbackEvent::UserFeedback(
        UserFeedbackEvent::new("session1", request_id).with_rating(4.5)
    );
    let entry1 = DLQEntry::new(event1, "Kafka timeout".to_string());
    let dlq_id1 = entry1.dlq_id;

    dlq.add(entry1).await.unwrap();

    let event2 = FeedbackEvent::PerformanceMetrics(
        PerformanceMetricsEvent::new(request_id, "gpt-4", 1500.0, 100, 200, 0.05)
    );
    let entry2 = DLQEntry::new(event2, "Network error".to_string());
    let dlq_id2 = entry2.dlq_id;

    dlq.add(entry2).await.unwrap();

    // Check stats
    let stats = dlq.stats().await.unwrap();
    assert_eq!(stats.total_entries, 2);
    assert_eq!(stats.current_depth, 2);

    // List all entries
    let entries = dlq.list_all(0, 10).await.unwrap();
    assert_eq!(entries.len(), 2);

    // Remove one entry
    dlq.remove(&dlq_id1).await.unwrap();

    let stats = dlq.stats().await.unwrap();
    assert_eq!(stats.current_depth, 1);

    // Verify remaining entry
    let entries = dlq.list_all(0, 10).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].dlq_id, dlq_id2);

    // Cleanup
    let _ = std::fs::remove_dir_all(temp_dir);
}

/// Test DLQ entry update
#[tokio::test]
async fn test_file_dlq_update_entry() {
    let temp_dir = std::env::temp_dir().join(format!("dlq_test_{}", Uuid::new_v4()));
    let mut config = DLQConfig::default();
    config.file_path = temp_dir.clone();

    let dlq = FileDLQ::new(config).unwrap();

    // Add entry
    let request_id = Uuid::new_v4();
    let event = FeedbackEvent::UserFeedback(UserFeedbackEvent::new("session", request_id));
    let mut entry = DLQEntry::new(event, "Initial failure".to_string());
    let dlq_id = entry.dlq_id;

    dlq.add(entry.clone()).await.unwrap();

    // Update entry with retry
    entry.increment_retry("Retry failed".to_string(), 120);
    dlq.update(entry.clone()).await.unwrap();

    // Retrieve and verify
    let entries = dlq.list_all(0, 10).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].dlq_id, dlq_id);
    assert_eq!(entries[0].retry_count, 1);
    assert_eq!(entries[0].last_error, "Retry failed");

    // Cleanup
    let _ = std::fs::remove_dir_all(temp_dir);
}

/// Test DLQ purge old entries
#[tokio::test]
async fn test_file_dlq_purge_old() {
    let temp_dir = std::env::temp_dir().join(format!("dlq_test_{}", Uuid::new_v4()));
    let mut config = DLQConfig::default();
    config.file_path = temp_dir.clone();

    let dlq = FileDLQ::new(config).unwrap();

    // Add entries
    let request_id = Uuid::new_v4();
    for i in 0..5 {
        let event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new(format!("session{}", i), request_id)
        );
        let entry = DLQEntry::new(event, format!("Failure {}", i));
        dlq.add(entry).await.unwrap();
    }

    let stats = dlq.stats().await.unwrap();
    assert_eq!(stats.current_depth, 5);

    // Purge entries older than now (should remove all)
    let future = Utc::now() + chrono::Duration::seconds(1);
    let removed = dlq.purge_old(future).await.unwrap();
    assert_eq!(removed, 5);

    let stats = dlq.stats().await.unwrap();
    assert_eq!(stats.current_depth, 0);

    // Cleanup
    let _ = std::fs::remove_dir_all(temp_dir);
}

/// Test DLQ get retry ready entries
#[tokio::test]
async fn test_file_dlq_get_retry_ready() {
    let temp_dir = std::env::temp_dir().join(format!("dlq_test_{}", Uuid::new_v4()));
    let mut config = DLQConfig::default();
    config.file_path = temp_dir.clone();

    let dlq = FileDLQ::new(config).unwrap();

    // Add entry that's ready for retry
    let request_id = Uuid::new_v4();
    let event = FeedbackEvent::UserFeedback(UserFeedbackEvent::new("session", request_id));
    let entry = DLQEntry::new(event, "Failure".to_string());

    dlq.add(entry).await.unwrap();

    // Get retry-ready entries
    let ready = dlq.get_retry_ready(10).await.unwrap();
    assert_eq!(ready.len(), 1);
    assert!(ready[0].is_ready_for_retry());

    // Cleanup
    let _ = std::fs::remove_dir_all(temp_dir);
}

/// Test DLQ manager creation and configuration
#[tokio::test]
async fn test_dlq_manager_creation() {
    let temp_dir = std::env::temp_dir().join(format!("dlq_test_{}", Uuid::new_v4()));

    let dlq_config = DLQConfig {
        enabled: true,
        dlq_type: DLQType::File,
        kafka_topic: "test-dlq".to_string(),
        kafka_config: None,
        file_path: temp_dir.clone(),
        max_file_size_mb: 10,
        max_files: 5,
        retry_interval_seconds: 60,
        max_retry_attempts: 3,
        retention_days: 7,
        enable_recovery: false,
    };

    let kafka_config = KafkaProducerConfig::default();
    let producer = Arc::new(FeedbackProducer::new(kafka_config).unwrap());

    let dlq_manager = DLQManager::new(dlq_config, producer);
    assert!(dlq_manager.is_ok());

    // Cleanup
    let _ = std::fs::remove_dir_all(temp_dir);
}

/// Test DLQ manager add failed event
#[tokio::test]
async fn test_dlq_manager_add_failed_event() {
    let temp_dir = std::env::temp_dir().join(format!("dlq_test_{}", Uuid::new_v4()));

    let dlq_config = DLQConfig {
        enabled: true,
        dlq_type: DLQType::File,
        kafka_topic: "test-dlq".to_string(),
        kafka_config: None,
        file_path: temp_dir.clone(),
        max_file_size_mb: 10,
        max_files: 5,
        retry_interval_seconds: 60,
        max_retry_attempts: 3,
        retention_days: 7,
        enable_recovery: false,
    };

    let kafka_config = KafkaProducerConfig::default();
    let producer = Arc::new(FeedbackProducer::new(kafka_config).unwrap());
    let dlq_manager = DLQManager::new(dlq_config, producer).unwrap();

    // Add failed event
    let request_id = Uuid::new_v4();
    let event = FeedbackEvent::UserFeedback(
        UserFeedbackEvent::new("session", request_id).with_rating(5.0)
    );

    dlq_manager.add_failed_event(event, "Kafka unavailable".to_string()).await.unwrap();

    // Check stats
    let stats = dlq_manager.stats().await.unwrap();
    assert_eq!(stats.total_entries, 1);
    assert_eq!(stats.current_depth, 1);

    // List entries
    let entries = dlq_manager.list_entries(0, 10).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].failure_reason, "Kafka unavailable");

    // Cleanup
    let _ = std::fs::remove_dir_all(temp_dir);
}

/// Test DLQ manager purge old entries
#[tokio::test]
async fn test_dlq_manager_purge() {
    let temp_dir = std::env::temp_dir().join(format!("dlq_test_{}", Uuid::new_v4()));

    let dlq_config = DLQConfig {
        enabled: true,
        dlq_type: DLQType::File,
        kafka_topic: "test-dlq".to_string(),
        kafka_config: None,
        file_path: temp_dir.clone(),
        max_file_size_mb: 10,
        max_files: 5,
        retry_interval_seconds: 60,
        max_retry_attempts: 3,
        retention_days: 7,
        enable_recovery: false,
    };

    let kafka_config = KafkaProducerConfig::default();
    let producer = Arc::new(FeedbackProducer::new(kafka_config).unwrap());
    let dlq_manager = DLQManager::new(dlq_config, producer).unwrap();

    // Add multiple events
    let request_id = Uuid::new_v4();
    for i in 0..3 {
        let event = FeedbackEvent::PerformanceMetrics(
            PerformanceMetricsEvent::new(request_id, "gpt-4", 1500.0, 100, 200, 0.05)
        );
        dlq_manager.add_failed_event(event, format!("Error {}", i)).await.unwrap();
    }

    let stats = dlq_manager.stats().await.unwrap();
    assert_eq!(stats.current_depth, 3);

    // Purge old entries (anything older than 0 days = all)
    let purged = dlq_manager.purge_old_entries(0).await.unwrap();
    assert_eq!(purged, 3);

    let stats = dlq_manager.stats().await.unwrap();
    assert_eq!(stats.current_depth, 0);

    // Cleanup
    let _ = std::fs::remove_dir_all(temp_dir);
}

/// Test DLQ with disabled configuration
#[tokio::test]
async fn test_dlq_disabled() {
    let temp_dir = std::env::temp_dir().join(format!("dlq_test_{}", Uuid::new_v4()));

    let dlq_config = DLQConfig {
        enabled: false, // Disabled
        dlq_type: DLQType::File,
        kafka_topic: "test-dlq".to_string(),
        kafka_config: None,
        file_path: temp_dir.clone(),
        max_file_size_mb: 10,
        max_files: 5,
        retry_interval_seconds: 60,
        max_retry_attempts: 3,
        retention_days: 7,
        enable_recovery: false,
    };

    let kafka_config = KafkaProducerConfig::default();
    let producer = Arc::new(FeedbackProducer::new(kafka_config).unwrap());
    let dlq_manager = DLQManager::new(dlq_config, producer).unwrap();

    // Add event (should be silently dropped)
    let request_id = Uuid::new_v4();
    let event = FeedbackEvent::UserFeedback(UserFeedbackEvent::new("session", request_id));

    dlq_manager.add_failed_event(event, "Test".to_string()).await.unwrap();

    // Stats should show no entries added
    let stats = dlq_manager.stats().await.unwrap();
    assert_eq!(stats.total_entries, 0);

    // Cleanup
    let _ = std::fs::remove_dir_all(temp_dir);
}

/// Test file rotation (conceptual - needs large data)
#[tokio::test]
async fn test_file_dlq_rotation_logic() {
    let temp_dir = std::env::temp_dir().join(format!("dlq_test_{}", Uuid::new_v4()));
    let mut config = DLQConfig::default();
    config.file_path = temp_dir.clone();
    config.max_file_size_mb = 1; // Small size to trigger rotation
    config.max_files = 3;

    let dlq = FileDLQ::new(config).unwrap();

    // Add some entries
    let request_id = Uuid::new_v4();
    for i in 0..10 {
        let event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new(format!("session{}", i), request_id)
        );
        let entry = DLQEntry::new(event, format!("Failure {}", i));
        dlq.add(entry).await.unwrap();
    }

    let stats = dlq.stats().await.unwrap();
    assert!(stats.total_entries > 0);
    assert!(stats.file_size_bytes > 0);

    // Cleanup
    let _ = std::fs::remove_dir_all(temp_dir);
}
