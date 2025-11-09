//! Comprehensive examples for KafkaSink usage
//!
//! This example demonstrates various features of the KafkaSink:
//! - Basic message sending
//! - Batch sending
//! - Transactional sending
//! - Custom serialization
//! - Metrics tracking
//! - Error handling and retries

use processor::kafka::{
    BincodeSerializer, DeliveryGuarantee, JsonSerializer, KafkaSink, KafkaSinkConfig,
    MessageSerializer, PartitionStrategy, SinkMessage,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MetricResult {
    service: String,
    avg_latency_ms: f64,
    p95_latency_ms: f64,
    request_count: u64,
    error_count: u64,
    timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OptimizationResult {
    model: String,
    original_cost: f64,
    optimized_cost: f64,
    cost_savings: f64,
    performance_impact: f64,
    recommendation: String,
}

/// Example 1: Basic message sending
async fn example_basic_send() -> anyhow::Result<()> {
    println!("\n=== Example 1: Basic Message Sending ===\n");

    let config = KafkaSinkConfig {
        brokers: "localhost:9092".to_string(),
        topic: "metrics-results".to_string(),
        client_id: "example-producer".to_string(),
        enable_idempotence: true,
        ..Default::default()
    };

    let sink = KafkaSink::<MetricResult>::new(config).await?;

    // Create and send a message
    let result = MetricResult {
        service: "api-gateway".to_string(),
        avg_latency_ms: 123.45,
        p95_latency_ms: 256.78,
        request_count: 1000,
        error_count: 5,
        timestamp: chrono::Utc::now().timestamp_millis(),
    };

    let message = SinkMessage::new(result)
        .with_key("api-gateway".to_string())
        .with_header("region".to_string(), "us-east-1".to_string())
        .with_header("environment".to_string(), "production".to_string());

    sink.send(message).await?;

    println!("Message sent successfully!");

    // Get metrics
    let metrics = sink.metrics().await;
    println!("Sink metrics: {:#?}", metrics);

    sink.close().await?;

    Ok(())
}

/// Example 2: Batch sending for high throughput
async fn example_batch_send() -> anyhow::Result<()> {
    println!("\n=== Example 2: Batch Message Sending ===\n");

    let config = KafkaSinkConfig {
        brokers: "localhost:9092".to_string(),
        topic: "metrics-batch".to_string(),
        batch_size: 100,
        linger_ms: 10,
        compression_type: "snappy".to_string(),
        ..Default::default()
    };

    let sink = KafkaSink::<MetricResult>::new(config).await?;

    // Create a batch of messages
    let mut messages = Vec::new();
    let services = vec!["api-gateway", "auth-service", "payment-service", "inventory-service"];

    for i in 0..50 {
        let service = services[i % services.len()];
        let result = MetricResult {
            service: service.to_string(),
            avg_latency_ms: 100.0 + (i as f64 * 2.5),
            p95_latency_ms: 200.0 + (i as f64 * 5.0),
            request_count: 1000 + (i * 10),
            error_count: i % 10,
            timestamp: chrono::Utc::now().timestamp_millis(),
        };

        let message = SinkMessage::new(result)
            .with_key(service.to_string())
            .with_partition((i % 4) as i32);

        messages.push(message);
    }

    println!("Sending {} messages in batch...", messages.len());
    let start = std::time::Instant::now();

    let results = sink.send_batch(messages).await?;

    let elapsed = start.elapsed();
    let successful = results.iter().filter(|r| r.is_ok()).count();
    let failed = results.iter().filter(|r| r.is_err()).count();

    println!("Batch send completed in {:?}", elapsed);
    println!("Successful: {}, Failed: {}", successful, failed);

    // Display metrics
    let metrics = sink.metrics().await;
    println!("Total sent: {}", metrics.messages_sent);
    println!("Average latency: {}us", metrics.avg_latency_us);
    println!("Max latency: {}us", metrics.max_latency_us);
    println!("Total bytes: {}", metrics.bytes_sent);

    sink.close().await?;

    Ok(())
}

/// Example 3: Transactional sending for exactly-once semantics
async fn example_transactional_send() -> anyhow::Result<()> {
    println!("\n=== Example 3: Transactional Sending ===\n");

    let config = KafkaSinkConfig {
        brokers: "localhost:9092".to_string(),
        topic: "optimization-results".to_string(),
        enable_transactions: true,
        transactional_id: Some("example-txn-producer".to_string()),
        enable_idempotence: true,
        ..Default::default()
    };

    let sink = KafkaSink::<OptimizationResult>::new(config)
        .await?
        .with_delivery_guarantee(DeliveryGuarantee::ExactlyOnce);

    // Begin transaction
    sink.begin_transaction().await?;
    println!("Transaction started");

    // Send messages within transaction
    let results = vec![
        OptimizationResult {
            model: "gpt-4".to_string(),
            original_cost: 100.0,
            optimized_cost: 75.0,
            cost_savings: 25.0,
            performance_impact: 0.95,
            recommendation: "Switch to gpt-3.5-turbo for non-critical requests".to_string(),
        },
        OptimizationResult {
            model: "claude-2".to_string(),
            original_cost: 80.0,
            optimized_cost: 65.0,
            cost_savings: 15.0,
            performance_impact: 0.98,
            recommendation: "Use caching for repeated queries".to_string(),
        },
    ];

    for (i, result) in results.into_iter().enumerate() {
        let message = SinkMessage::new(result.clone())
            .with_key(result.model.clone());

        sink.send(message).await?;
        println!("Message {} sent in transaction", i + 1);
    }

    // Commit transaction
    sink.commit_transaction().await?;
    println!("Transaction committed successfully");

    // Example: Transaction abort on error
    sink.begin_transaction().await?;

    let error_result = OptimizationResult {
        model: "invalid-model".to_string(),
        original_cost: 0.0,
        optimized_cost: 0.0,
        cost_savings: 0.0,
        performance_impact: 0.0,
        recommendation: "This will be rolled back".to_string(),
    };

    let message = SinkMessage::new(error_result);
    sink.send(message).await?;

    // Simulate error and abort
    println!("Simulating error - aborting transaction");
    sink.abort_transaction().await?;
    println!("Transaction aborted successfully");

    sink.close().await?;

    Ok(())
}

/// Example 4: Custom serialization with Bincode
async fn example_custom_serialization() -> anyhow::Result<()> {
    println!("\n=== Example 4: Custom Serialization (Bincode) ===\n");

    let config = KafkaSinkConfig {
        brokers: "localhost:9092".to_string(),
        topic: "binary-results".to_string(),
        compression_type: "lz4".to_string(),
        ..Default::default()
    };

    // Use Bincode serializer for more efficient binary encoding
    let sink = KafkaSink::<MetricResult>::with_serializer(
        config,
        Arc::new(BincodeSerializer),
    )
    .await?;

    let result = MetricResult {
        service: "binary-service".to_string(),
        avg_latency_ms: 50.0,
        p95_latency_ms: 100.0,
        request_count: 5000,
        error_count: 2,
        timestamp: chrono::Utc::now().timestamp_millis(),
    };

    let message = SinkMessage::new(result)
        .with_key("binary-service".to_string());

    sink.send(message).await?;

    let metrics = sink.metrics().await;
    println!("Binary message sent: {} bytes", metrics.bytes_sent);

    sink.close().await?;

    Ok(())
}

/// Example 5: Different partitioning strategies
async fn example_partitioning_strategies() -> anyhow::Result<()> {
    println!("\n=== Example 5: Partitioning Strategies ===\n");

    let config = KafkaSinkConfig {
        brokers: "localhost:9092".to_string(),
        topic: "partitioned-results".to_string(),
        ..Default::default()
    };

    // Key-based partitioning (default)
    let sink = KafkaSink::<MetricResult>::new(config.clone())
        .await?
        .with_partition_strategy(PartitionStrategy::KeyHash);

    for i in 0..5 {
        let result = MetricResult {
            service: format!("service-{}", i),
            avg_latency_ms: 100.0,
            p95_latency_ms: 200.0,
            request_count: 1000,
            error_count: 0,
            timestamp: chrono::Utc::now().timestamp_millis(),
        };

        // Key hash will determine partition
        let message = SinkMessage::new(result.clone())
            .with_key(format!("service-{}", i));

        sink.send(message).await?;
        println!("Sent to partition (key hash): service-{}", i);
    }

    sink.close().await?;

    // Custom partition assignment
    let sink2 = KafkaSink::<MetricResult>::new(config.clone())
        .await?
        .with_partition_strategy(PartitionStrategy::Custom);

    for partition in 0..3 {
        let result = MetricResult {
            service: format!("custom-partition-{}", partition),
            avg_latency_ms: 100.0,
            p95_latency_ms: 200.0,
            request_count: 1000,
            error_count: 0,
            timestamp: chrono::Utc::now().timestamp_millis(),
        };

        let message = SinkMessage::new(result)
            .with_partition(partition);

        sink2.send(message).await?;
        println!("Sent to custom partition: {}", partition);
    }

    sink2.close().await?;

    Ok(())
}

/// Example 6: Error handling and circuit breaker
async fn example_error_handling() -> anyhow::Result<()> {
    println!("\n=== Example 6: Error Handling and Circuit Breaker ===\n");

    let config = KafkaSinkConfig {
        brokers: "localhost:9092".to_string(),
        topic: "error-test".to_string(),
        max_retries: 3,
        base_backoff_ms: 100,
        enable_circuit_breaker: true,
        circuit_breaker_threshold: 5,
        circuit_breaker_timeout_secs: 60,
        send_timeout_ms: 5000,
        ..Default::default()
    };

    let sink = KafkaSink::<MetricResult>::new(config).await?;

    // Send messages and handle errors
    for i in 0..10 {
        let result = MetricResult {
            service: format!("service-{}", i),
            avg_latency_ms: 100.0,
            p95_latency_ms: 200.0,
            request_count: 1000,
            error_count: 0,
            timestamp: chrono::Utc::now().timestamp_millis(),
        };

        let message = SinkMessage::new(result)
            .with_key(format!("service-{}", i));

        match sink.send(message).await {
            Ok(_) => println!("Message {} sent successfully", i),
            Err(e) => {
                println!("Failed to send message {}: {}", i, e);

                // Check if circuit breaker is open
                let metrics = sink.metrics().await;
                if metrics.circuit_breaker_trips > 0 {
                    println!("Circuit breaker tripped {} times", metrics.circuit_breaker_trips);
                    println!("Last error: {:?}", metrics.last_error_msg);
                    break;
                }
            }
        }

        sleep(Duration::from_millis(100)).await;
    }

    let metrics = sink.metrics().await;
    println!("\nFinal metrics:");
    println!("  Messages sent: {}", metrics.messages_sent);
    println!("  Messages failed: {}", metrics.messages_failed);
    println!("  Retries: {}", metrics.retries);
    println!("  Circuit breaker trips: {}", metrics.circuit_breaker_trips);

    sink.close().await?;

    Ok(())
}

/// Example 7: Monitoring and metrics
async fn example_metrics_monitoring() -> anyhow::Result<()> {
    println!("\n=== Example 7: Metrics Monitoring ===\n");

    let config = KafkaSinkConfig {
        brokers: "localhost:9092".to_string(),
        topic: "monitoring-test".to_string(),
        ..Default::default()
    };

    let sink = KafkaSink::<MetricResult>::new(config).await?;

    // Send messages while monitoring metrics
    let monitor_sink = sink.clone_for_send();
    let monitor_handle = tokio::spawn(async move {
        for _ in 0..10 {
            sleep(Duration::from_secs(1)).await;
            let metrics = monitor_sink.metrics().await;

            println!("\n--- Metrics Update ---");
            println!("Messages sent: {}", metrics.messages_sent);
            println!("Messages failed: {}", metrics.messages_failed);
            println!("Bytes sent: {}", metrics.bytes_sent);
            println!("Avg latency: {}us", metrics.avg_latency_us);
            println!("Max latency: {}us", metrics.max_latency_us);
            println!("Send attempts: {}", metrics.send_attempts);
            println!("Retries: {}", metrics.retries);
        }
    });

    // Send messages concurrently
    for i in 0..100 {
        let result = MetricResult {
            service: format!("service-{}", i % 10),
            avg_latency_ms: 100.0 + (i as f64),
            p95_latency_ms: 200.0 + (i as f64 * 2.0),
            request_count: 1000,
            error_count: 0,
            timestamp: chrono::Utc::now().timestamp_millis(),
        };

        let message = SinkMessage::new(result)
            .with_key(format!("service-{}", i % 10));

        sink.send(message).await?;

        if i % 10 == 0 {
            sleep(Duration::from_millis(100)).await;
        }
    }

    // Wait for monitor to finish
    monitor_handle.abort();

    println!("\n=== Final Metrics ===");
    let final_metrics = sink.metrics().await;
    println!("{:#?}", final_metrics);

    sink.close().await?;

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Kafka Sink Examples");
    println!("===================\n");
    println!("These examples demonstrate various features of KafkaSink.");
    println!("Make sure Kafka is running at localhost:9092\n");

    // Run examples (comment out ones you don't want to run)

    // Basic usage
    if let Err(e) = example_basic_send().await {
        eprintln!("Example 1 failed: {}", e);
    }

    // Batch sending
    if let Err(e) = example_batch_send().await {
        eprintln!("Example 2 failed: {}", e);
    }

    // Transactions (requires Kafka 0.11+)
    if let Err(e) = example_transactional_send().await {
        eprintln!("Example 3 failed: {}", e);
    }

    // Custom serialization
    if let Err(e) = example_custom_serialization().await {
        eprintln!("Example 4 failed: {}", e);
    }

    // Partitioning strategies
    if let Err(e) = example_partitioning_strategies().await {
        eprintln!("Example 5 failed: {}", e);
    }

    // Error handling
    if let Err(e) = example_error_handling().await {
        eprintln!("Example 6 failed: {}", e);
    }

    // Metrics monitoring
    if let Err(e) = example_metrics_monitoring().await {
        eprintln!("Example 7 failed: {}", e);
    }

    println!("\n=== All Examples Completed ===");

    Ok(())
}
