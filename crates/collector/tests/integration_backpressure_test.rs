//! Integration tests for backpressure and buffer overflow scenarios
//!
//! These tests verify that the collector handles high-load scenarios,
//! buffer saturation, and flow control correctly.

mod common;

use collector::{FeedbackCollector, FeedbackEvent, UserFeedbackEvent, PerformanceMetricsEvent};
use common::{test_collector_config, test_kafka_config, wait_for_condition, KafkaTestContainer};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use testcontainers::clients::Cli;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Test collector handles buffer overflow with try_submit
#[tokio::test]
async fn test_buffer_overflow_with_try_submit() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "overflow-test";

    let kafka_config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let mut config = test_collector_config(kafka_config);
    config.buffer_size = 10; // Very small buffer
    config.max_batch_size = 100; // Large batch to prevent quick flushing
    config.flush_interval_ms = 10000; // Slow flush

    let (mut collector, _) = FeedbackCollector::new(config.clone());
    let (tx, rx) = mpsc::channel(config.buffer_size);

    collector.start(rx).await.expect("Failed to start");

    // Try to submit more events than buffer can hold
    let mut success_count = 0;
    let mut failure_count = 0;

    for i in 0..50 {
        let event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new(format!("session-{}", i), Uuid::new_v4()),
        );

        match collector.try_submit(event) {
            Ok(_) => success_count += 1,
            Err(_) => failure_count += 1,
        }
    }

    // Should have some failures due to buffer overflow
    assert!(failure_count > 0, "Expected some failures due to buffer overflow");
    assert!(success_count > 0, "Expected some successes");
    assert_eq!(
        success_count + failure_count,
        50,
        "Total attempts should be 50"
    );

    drop(tx);
    tokio::time::sleep(Duration::from_secs(2)).await;
    collector.shutdown().await.expect("Shutdown failed");
}

/// Test blocking submit waits when buffer is full
#[tokio::test]
async fn test_blocking_submit_with_full_buffer() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "blocking-test";

    let kafka_config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let mut config = test_collector_config(kafka_config);
    config.buffer_size = 20;
    config.max_batch_size = 5; // Will flush periodically

    let (mut collector, _) = FeedbackCollector::new(config.clone());
    let (tx, rx) = mpsc::channel(config.buffer_size);

    collector.start(rx).await.expect("Failed to start");

    // Submit events using blocking submit
    let submitted = Arc::new(AtomicUsize::new(0));
    let submitted_clone = submitted.clone();

    let submit_task = tokio::spawn(async move {
        for i in 0..30 {
            let event = FeedbackEvent::UserFeedback(
                UserFeedbackEvent::new(format!("session-{}", i), Uuid::new_v4()),
            );
            collector.submit(event).await.expect("Submit failed");
            submitted_clone.fetch_add(1, Ordering::SeqCst);
        }
    });

    // Wait for task to complete
    tokio::time::timeout(Duration::from_secs(10), submit_task)
        .await
        .expect("Task should complete within timeout")
        .expect("Task should succeed");

    // All events should have been submitted (blocking waited for space)
    assert_eq!(submitted.load(Ordering::SeqCst), 30, "All 30 events should be submitted");

    drop(tx);
    tokio::time::sleep(Duration::from_secs(2)).await;
    // Note: collector was moved into task, no need to shutdown
}

/// Test high-throughput scenario
#[tokio::test]
async fn test_high_throughput_sustained_load() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "throughput-test";

    let kafka_config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let mut config = test_collector_config(kafka_config);
    config.buffer_size = 1000;
    config.max_batch_size = 50;
    config.flush_interval_ms = 500;

    let (mut collector, _) = FeedbackCollector::new(config.clone());
    let (tx, rx) = mpsc::channel(config.buffer_size);

    collector.start(rx).await.expect("Failed to start");

    // Submit 500 events as fast as possible
    let start = std::time::Instant::now();

    for i in 0..500 {
        let event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new(format!("session-{}", i), Uuid::new_v4()),
        );
        tx.send(event).await.expect("Send failed");
    }

    let send_duration = start.elapsed();
    println!("Sent 500 events in {:?}", send_duration);

    // Wait for processing
    tokio::time::sleep(Duration::from_secs(5)).await;
    drop(tx);
    tokio::time::sleep(Duration::from_secs(2)).await;

    let stats = collector.stats();
    assert_eq!(stats.events_received, 500, "All events should be received");
    assert_eq!(stats.events_published, 500, "All events should be published");

    println!("Processed {} batches", stats.batches_processed);
    assert!(stats.batches_processed >= 10, "Should have processed multiple batches");

    collector.shutdown().await.expect("Shutdown failed");
}

/// Test concurrent producers with single collector
#[tokio::test]
async fn test_multiple_producers_single_collector() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "multi-producer-test";

    let kafka_config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let mut config = test_collector_config(kafka_config);
    config.buffer_size = 500;
    config.max_batch_size = 20;

    let (mut collector, _) = FeedbackCollector::new(config.clone());
    let (tx, rx) = mpsc::channel(config.buffer_size);

    collector.start(rx).await.expect("Failed to start");

    // Spawn 10 concurrent producers
    let mut handles = vec![];
    for producer_id in 0..10 {
        let tx_clone = tx.clone();
        let handle = tokio::spawn(async move {
            for i in 0..20 {
                let event = FeedbackEvent::UserFeedback(
                    UserFeedbackEvent::new(
                        format!("producer-{}-event-{}", producer_id, i),
                        Uuid::new_v4(),
                    ),
                );
                tx_clone.send(event).await.expect("Send failed");
            }
        });
        handles.push(handle);
    }

    // Wait for all producers to finish
    for handle in handles {
        handle.await.expect("Producer task failed");
    }

    // Wait for processing
    tokio::time::sleep(Duration::from_secs(3)).await;
    drop(tx);
    tokio::time::sleep(Duration::from_secs(2)).await;

    let stats = collector.stats();
    assert_eq!(
        stats.events_received, 200,
        "All 200 events (10 producers * 20 events) should be received"
    );
    assert_eq!(stats.events_published, 200, "All events should be published");

    collector.shutdown().await.expect("Shutdown failed");
}

/// Test burst traffic pattern
#[tokio::test]
async fn test_burst_traffic_pattern() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "burst-test";

    let kafka_config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let mut config = test_collector_config(kafka_config);
    config.buffer_size = 200;
    config.max_batch_size = 25;

    let (mut collector, _) = FeedbackCollector::new(config.clone());
    let (tx, rx) = mpsc::channel(config.buffer_size);

    collector.start(rx).await.expect("Failed to start");

    // Simulate 3 bursts of traffic
    for burst in 0..3 {
        // Send burst of 50 events
        for i in 0..50 {
            let event = FeedbackEvent::UserFeedback(
                UserFeedbackEvent::new(format!("burst-{}-event-{}", burst, i), Uuid::new_v4()),
            );
            tx.send(event).await.expect("Send failed");
        }

        // Wait between bursts
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    // Wait for processing
    tokio::time::sleep(Duration::from_secs(3)).await;
    drop(tx);
    tokio::time::sleep(Duration::from_secs(2)).await;

    let stats = collector.stats();
    assert_eq!(stats.events_received, 150, "All 150 events should be received");
    assert_eq!(stats.events_published, 150, "All events should be published");

    collector.shutdown().await.expect("Shutdown failed");
}

/// Test slow consumer scenario (Kafka publishing is slow)
#[tokio::test]
async fn test_slow_kafka_publishing() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "slow-publish-test";

    let kafka_config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let mut config = test_collector_config(kafka_config);
    config.buffer_size = 100;
    config.max_batch_size = 10;
    config.flush_interval_ms = 2000; // Slower flush

    let (mut collector, _) = FeedbackCollector::new(config.clone());
    let (tx, rx) = mpsc::channel(config.buffer_size);

    collector.start(rx).await.expect("Failed to start");

    // Send events faster than they're flushed
    for i in 0..30 {
        let event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new(format!("session-{}", i), Uuid::new_v4()),
        );
        tx.send(event).await.expect("Send failed");
    }

    // Check stats while still processing
    tokio::time::sleep(Duration::from_secs(1)).await;
    let interim_stats = collector.stats();
    println!(
        "Interim stats: received={}, published={}",
        interim_stats.events_received, interim_stats.events_published
    );

    // Wait for full processing
    tokio::time::sleep(Duration::from_secs(5)).await;
    drop(tx);
    tokio::time::sleep(Duration::from_secs(2)).await;

    let final_stats = collector.stats();
    assert_eq!(final_stats.events_received, 30, "All events should be received");
    assert_eq!(final_stats.events_published, 30, "All events should eventually be published");

    collector.shutdown().await.expect("Shutdown failed");
}

/// Test very large events (stress test payload size)
#[tokio::test]
async fn test_large_event_payloads() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "large-payload-test";

    let kafka_config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let mut config = test_collector_config(kafka_config);
    config.max_batch_size = 5; // Smaller batches for large events

    let (mut collector, _) = FeedbackCollector::new(config.clone());
    let (tx, rx) = mpsc::channel(config.buffer_size);

    collector.start(rx).await.expect("Failed to start");

    // Create events with large metadata
    for i in 0..10 {
        let mut event = UserFeedbackEvent::new(format!("session-{}", i), Uuid::new_v4());

        // Add large metadata (simulate large comment)
        let large_comment = "x".repeat(10000); // 10KB comment
        event.comment = Some(large_comment);

        tx.send(FeedbackEvent::UserFeedback(event))
            .await
            .expect("Send failed");
    }

    // Wait for processing
    tokio::time::sleep(Duration::from_secs(5)).await;
    drop(tx);
    tokio::time::sleep(Duration::from_secs(2)).await;

    let stats = collector.stats();
    assert_eq!(stats.events_received, 10, "All large events should be received");
    assert_eq!(stats.events_published, 10, "All large events should be published");

    collector.shutdown().await.expect("Shutdown failed");
}

/// Test rapid start/stop cycles under load
#[tokio::test]
async fn test_rapid_lifecycle_under_load() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "rapid-lifecycle-test";

    for cycle in 0..3 {
        let kafka_config = test_kafka_config(&kafka.bootstrap_servers(), topic);
        let mut config = test_collector_config(kafka_config);
        config.max_batch_size = 5;

        let (mut collector, _) = FeedbackCollector::new(config.clone());
        let (tx, rx) = mpsc::channel(config.buffer_size);

        collector.start(rx).await.expect("Failed to start");

        // Send some events
        for i in 0..10 {
            let event = FeedbackEvent::UserFeedback(
                UserFeedbackEvent::new(format!("cycle-{}-event-{}", cycle, i), Uuid::new_v4()),
            );
            tx.send(event).await.expect("Send failed");
        }

        // Immediate shutdown
        drop(tx);
        tokio::time::sleep(Duration::from_millis(500)).await;

        let result = collector.shutdown().await;
        assert!(result.is_ok(), "Cycle {} shutdown failed", cycle);

        // Brief pause between cycles
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Test mixed event sizes in high volume
#[tokio::test]
async fn test_mixed_event_sizes_high_volume() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "mixed-size-test";

    let kafka_config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let mut config = test_collector_config(kafka_config);
    config.buffer_size = 500;
    config.max_batch_size = 20;

    let (mut collector, _) = FeedbackCollector::new(config.clone());
    let (tx, rx) = mpsc::channel(config.buffer_size);

    collector.start(rx).await.expect("Failed to start");

    // Send mix of small and large events
    for i in 0..100 {
        let event = if i % 3 == 0 {
            // Large event
            let mut user_event = UserFeedbackEvent::new(format!("session-{}", i), Uuid::new_v4());
            user_event.comment = Some("x".repeat(1000));
            FeedbackEvent::UserFeedback(user_event)
        } else {
            // Small event
            FeedbackEvent::PerformanceMetrics(PerformanceMetricsEvent::new(
                Uuid::new_v4(),
                "gpt-4",
                1000.0,
                100,
                200,
                0.05,
            ))
        };

        tx.send(event).await.expect("Send failed");
    }

    // Wait for processing
    tokio::time::sleep(Duration::from_secs(4)).await;
    drop(tx);
    tokio::time::sleep(Duration::from_secs(2)).await;

    let stats = collector.stats();
    assert_eq!(stats.events_received, 100, "All 100 events should be received");
    assert_eq!(stats.events_published, 100, "All 100 events should be published");

    collector.shutdown().await.expect("Shutdown failed");
}

/// Test backpressure with slow batch processing
#[tokio::test]
async fn test_backpressure_with_batch_delays() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "backpressure-delay-test";

    let kafka_config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let mut config = test_collector_config(kafka_config);
    config.buffer_size = 50;
    config.max_batch_size = 10;
    config.flush_interval_ms = 1000;

    let (mut collector, _) = FeedbackCollector::new(config.clone());
    let (tx, rx) = mpsc::channel(config.buffer_size);

    collector.start(rx).await.expect("Failed to start");

    // Submit events and measure backpressure
    let submitted = Arc::new(AtomicUsize::new(0));
    let submitted_clone = submitted.clone();

    // Spawn task that submits as fast as possible
    let submit_task = tokio::spawn(async move {
        for i in 0..100 {
            let event = FeedbackEvent::UserFeedback(
                UserFeedbackEvent::new(format!("session-{}", i), Uuid::new_v4()),
            );
            // This will block when buffer is full
            collector.submit(event).await.expect("Submit failed");
            submitted_clone.fetch_add(1, Ordering::SeqCst);
        }
    });

    // Monitor submission rate
    tokio::time::sleep(Duration::from_secs(2)).await;
    let interim_submitted = submitted.load(Ordering::SeqCst);
    println!("Submitted {} events in 2 seconds", interim_submitted);

    // Wait for completion
    tokio::time::timeout(Duration::from_secs(15), submit_task)
        .await
        .expect("Should complete within timeout")
        .expect("Task should succeed");

    assert_eq!(submitted.load(Ordering::SeqCst), 100, "All events should eventually be submitted");

    drop(tx);
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// Test queue depth monitoring during high load
#[tokio::test]
async fn test_queue_depth_monitoring() {
    let docker = Cli::default();
    let kafka = KafkaTestContainer::start(&docker);
    let topic = "queue-depth-test";

    let kafka_config = test_kafka_config(&kafka.bootstrap_servers(), topic);
    let mut config = test_collector_config(kafka_config);
    config.buffer_size = 100;
    config.max_batch_size = 10;
    config.flush_interval_ms = 2000; // Slow flush to build up queue

    let (mut collector, _) = FeedbackCollector::new(config.clone());
    let (tx, rx) = mpsc::channel(config.buffer_size);

    collector.start(rx).await.expect("Failed to start");

    // Send many events quickly
    for i in 0..50 {
        let event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new(format!("session-{}", i), Uuid::new_v4()),
        );
        tx.send(event).await.expect("Send failed");
    }

    // Check stats at different intervals
    tokio::time::sleep(Duration::from_secs(1)).await;
    let stats_1s = collector.stats();

    tokio::time::sleep(Duration::from_secs(2)).await;
    let stats_3s = collector.stats();

    drop(tx);
    tokio::time::sleep(Duration::from_secs(3)).await;
    let final_stats = collector.stats();

    println!("Stats at 1s: received={}, published={}", stats_1s.events_received, stats_1s.events_published);
    println!("Stats at 3s: received={}, published={}", stats_3s.events_received, stats_3s.events_published);
    println!("Final stats: received={}, published={}", final_stats.events_received, final_stats.events_published);

    assert_eq!(final_stats.events_received, 50, "All events should be received");
    assert_eq!(final_stats.events_published, 50, "All events should be published");

    collector.shutdown().await.expect("Shutdown failed");
}
