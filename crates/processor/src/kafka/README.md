# Kafka Integration

Production-ready Kafka source and sink implementations for the stream processor.

## Features

### KafkaSink (Producer)

High-performance Kafka producer with advanced features:

- **Batching & Compression**: Automatic batching with configurable compression (snappy, gzip, lz4, zstd)
- **Idempotent Producer**: Exactly-once message delivery within a partition
- **Transactional Support**: Full ACID transactions for exactly-once semantics across partitions
- **Circuit Breaker**: Automatic fault tolerance with configurable thresholds
- **Retry Logic**: Exponential backoff with configurable retry limits
- **Partitioning Strategies**: Key-based, round-robin, custom, or single partition
- **Custom Serialization**: JSON (default) or Bincode, with support for custom serializers
- **Comprehensive Metrics**: Latency, throughput, error rates, and more
- **Graceful Shutdown**: Ensures all pending messages are flushed

### KafkaSource (Consumer)

Robust Kafka consumer (implemented separately):

- Automatic offset management
- Multiple commit strategies
- Partition rebalancing support
- Consumer group coordination
- Message deserialization with error handling

## Usage

### Basic Message Sending

```rust
use processor::kafka::{KafkaSink, KafkaSinkConfig, SinkMessage};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct MetricResult {
    service: String,
    avg_latency: f64,
    count: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Configure sink
    let config = KafkaSinkConfig {
        brokers: "localhost:9092".to_string(),
        topic: "metrics-results".to_string(),
        enable_idempotence: true,
        ..Default::default()
    };

    // Create sink
    let sink = KafkaSink::<MetricResult>::new(config).await?;

    // Send message
    let result = MetricResult {
        service: "api-gateway".to_string(),
        avg_latency: 123.45,
        count: 1000,
    };

    let message = SinkMessage::new(result)
        .with_key("api-gateway".to_string())
        .with_header("region".to_string(), "us-east-1".to_string());

    sink.send(message).await?;

    // Graceful shutdown
    sink.close().await?;

    Ok(())
}
```

### Batch Sending

```rust
// Configure for high throughput
let config = KafkaSinkConfig {
    brokers: "localhost:9092".to_string(),
    topic: "high-volume-topic".to_string(),
    batch_size: 1000,
    linger_ms: 100,
    compression_type: "snappy".to_string(),
    ..Default::default()
};

let sink = KafkaSink::<MetricResult>::new(config).await?;

// Create batch
let messages: Vec<_> = results
    .into_iter()
    .map(|r| SinkMessage::new(r).with_key(r.service.clone()))
    .collect();

// Send batch
let results = sink.send_batch(messages).await?;

// Check results
let successful = results.iter().filter(|r| r.is_ok()).count();
println!("Sent {} messages", successful);
```

### Transactional Sending (Exactly-Once)

```rust
use processor::kafka::DeliveryGuarantee;

let config = KafkaSinkConfig {
    brokers: "localhost:9092".to_string(),
    topic: "transactional-topic".to_string(),
    enable_transactions: true,
    transactional_id: Some("my-producer-id".to_string()),
    enable_idempotence: true,
    ..Default::default()
};

let sink = KafkaSink::<MetricResult>::new(config)
    .await?
    .with_delivery_guarantee(DeliveryGuarantee::ExactlyOnce);

// Begin transaction
sink.begin_transaction().await?;

// Send messages
for result in results {
    let message = SinkMessage::new(result);
    sink.send(message).await?;
}

// Commit transaction (or abort on error)
sink.commit_transaction().await?;
```

### Custom Serialization

```rust
use processor::kafka::{BincodeSerializer, MessageSerializer};
use std::sync::Arc;

// Use Bincode for binary serialization
let sink = KafkaSink::<MetricResult>::with_serializer(
    config,
    Arc::new(BincodeSerializer),
).await?;

// Or implement custom serializer
struct CustomSerializer;

#[async_trait]
impl<T: Serialize + Send + Sync> MessageSerializer<T> for CustomSerializer {
    async fn serialize(&self, message: &T) -> Result<Vec<u8>> {
        // Custom serialization logic
        Ok(vec![])
    }

    fn content_type(&self) -> &str {
        "application/custom"
    }
}
```

### Partitioning Strategies

```rust
use processor::kafka::PartitionStrategy;

// Key-based partitioning (default)
let sink = KafkaSink::<MetricResult>::new(config)
    .await?
    .with_partition_strategy(PartitionStrategy::KeyHash);

// Custom partition assignment
let sink = KafkaSink::<MetricResult>::new(config)
    .await?
    .with_partition_strategy(PartitionStrategy::Custom);

let message = SinkMessage::new(result)
    .with_partition(2); // Send to partition 2

sink.send(message).await?;
```

### Monitoring and Metrics

```rust
// Get metrics
let metrics = sink.metrics().await;

println!("Messages sent: {}", metrics.messages_sent);
println!("Messages failed: {}", metrics.messages_failed);
println!("Average latency: {}us", metrics.avg_latency_us);
println!("Max latency: {}us", metrics.max_latency_us);
println!("Bytes sent: {}", metrics.bytes_sent);
println!("Retries: {}", metrics.retries);
println!("Circuit breaker trips: {}", metrics.circuit_breaker_trips);

if let Some(ref err) = metrics.last_error_msg {
    println!("Last error: {}", err);
}
```

### Error Handling with Circuit Breaker

```rust
let config = KafkaSinkConfig {
    brokers: "localhost:9092".to_string(),
    topic: "resilient-topic".to_string(),
    enable_circuit_breaker: true,
    circuit_breaker_threshold: 5,      // Open after 5 failures
    circuit_breaker_timeout_secs: 60,  // Try again after 60 seconds
    max_retries: 3,
    base_backoff_ms: 100,
    ..Default::default()
};

let sink = KafkaSink::<MetricResult>::new(config).await?;

match sink.send(message).await {
    Ok(_) => println!("Message sent"),
    Err(e) => {
        println!("Send failed: {}", e);

        let metrics = sink.metrics().await;
        if metrics.circuit_breaker_trips > 0 {
            println!("Circuit breaker is open, backing off...");
        }
    }
}
```

## Configuration

### KafkaSinkConfig

```rust
pub struct KafkaSinkConfig {
    /// Kafka bootstrap servers (required)
    pub brokers: String,

    /// Default topic for messages (required)
    pub topic: String,

    /// Client ID for this producer
    pub client_id: String,

    /// Batch size for batching messages
    pub batch_size: usize,  // Default: 100

    /// Timeout for sending messages (milliseconds)
    pub send_timeout_ms: u64,  // Default: 30000

    /// Enable idempotent producer
    pub enable_idempotence: bool,  // Default: true

    /// Enable transactions for exactly-once semantics
    pub enable_transactions: bool,  // Default: false

    /// Transaction ID (required if transactions enabled)
    pub transactional_id: Option<String>,

    /// Compression type (none, gzip, snappy, lz4, zstd)
    pub compression_type: String,  // Default: "snappy"

    /// Acknowledgment level (0, 1, all)
    pub acks: String,  // Default: "all"

    /// Maximum retries for failed sends
    pub max_retries: u32,  // Default: 5

    /// Base delay for exponential backoff (milliseconds)
    pub base_backoff_ms: u64,  // Default: 100

    /// Maximum in-flight requests
    pub max_in_flight_requests: usize,  // Default: 5

    /// Linger time for batching (milliseconds)
    pub linger_ms: u64,  // Default: 0

    /// Buffer memory size (bytes)
    pub buffer_memory: usize,  // Default: 33554432 (32MB)

    /// Enable circuit breaker
    pub enable_circuit_breaker: bool,  // Default: true

    /// Circuit breaker failure threshold
    pub circuit_breaker_threshold: u32,  // Default: 5

    /// Circuit breaker timeout (seconds)
    pub circuit_breaker_timeout_secs: u64,  // Default: 60
}
```

## Performance Tuning

### High Throughput

```rust
let config = KafkaSinkConfig {
    batch_size: 10000,
    linger_ms: 100,
    compression_type: "lz4".to_string(),
    buffer_memory: 67108864,  // 64MB
    max_in_flight_requests: 10,
    ..Default::default()
};
```

### Low Latency

```rust
let config = KafkaSinkConfig {
    batch_size: 1,
    linger_ms: 0,
    compression_type: "none".to_string(),
    max_in_flight_requests: 1,
    ..Default::default()
};
```

### Reliability (Exactly-Once)

```rust
let config = KafkaSinkConfig {
    enable_idempotence: true,
    enable_transactions: true,
    transactional_id: Some("unique-id".to_string()),
    acks: "all".to_string(),
    max_retries: 10,
    max_in_flight_requests: 1,
    ..Default::default()
};
```

## Testing

Run unit tests:
```bash
cargo test --package processor --lib kafka::sink
```

Run integration tests (requires Kafka):
```bash
# Start Kafka
docker-compose up -d kafka

# Run tests
cargo test --package processor --test kafka_sink_tests -- --ignored
```

Run examples:
```bash
# Start Kafka
docker-compose up -d kafka

# Run example
cargo run --package processor --example kafka_sink_example
```

## Architecture

### Component Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        KafkaSink<T>                          │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌────────────────┐      ┌──────────────────┐               │
│  │  Serializer    │─────▶│  FutureProducer  │               │
│  │  (JSON/Bincode)│      │    (rdkafka)     │               │
│  └────────────────┘      └──────────────────┘               │
│                                   │                          │
│                                   │                          │
│  ┌────────────────┐      ┌──────────────────┐               │
│  │ Circuit Breaker│      │  Retry Logic     │               │
│  │  (Fault Tol.)  │      │  (Exponential)   │               │
│  └────────────────┘      └──────────────────┘               │
│                                   │                          │
│                                   │                          │
│  ┌────────────────┐      ┌──────────────────┐               │
│  │ Metrics Tracker│      │   Semaphore      │               │
│  │  (Atomic)      │      │ (Concurrency)    │               │
│  └────────────────┘      └──────────────────┘               │
│                                                               │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
                    ┌──────────────┐
                    │    Kafka     │
                    │   Cluster    │
                    └──────────────┘
```

### Send Flow

1. **Serialize**: Convert message to bytes using configured serializer
2. **Check Circuit Breaker**: Verify circuit is not open
3. **Build Record**: Create Kafka record with headers, key, partition
4. **Send with Retry**: Send with exponential backoff on retriable errors
5. **Update Metrics**: Track latency, success/failure, bytes sent
6. **Update Circuit Breaker**: Record success/failure

### Transaction Flow

1. **Begin Transaction**: Initialize transaction context
2. **Send Messages**: All sends are part of the transaction
3. **Commit/Abort**: Either commit all or abort all
4. **Atomic**: Messages appear together or not at all

## Best Practices

1. **Use Idempotence**: Always enable for production
2. **Enable Compression**: Use snappy or lz4 for good balance
3. **Set Appropriate Timeouts**: Based on your latency requirements
4. **Monitor Metrics**: Track send rates and error rates
5. **Handle Errors**: Implement proper error handling and alerting
6. **Graceful Shutdown**: Always call `close()` to flush pending messages
7. **Tune Batching**: Balance between latency and throughput
8. **Use Transactions**: For exactly-once semantics when needed
9. **Circuit Breaker**: Enable for resilience against broker failures
10. **Test Thoroughly**: Use integration tests with real Kafka

## Troubleshooting

### Messages Not Sending

- Check Kafka broker connectivity
- Verify topic exists
- Check producer configuration (acks, retries)
- Review error logs and metrics

### High Latency

- Reduce batch size
- Decrease linger_ms
- Disable compression
- Reduce max_in_flight_requests

### Low Throughput

- Increase batch size
- Increase linger_ms
- Enable compression
- Increase buffer_memory
- Increase max_in_flight_requests

### Transaction Failures

- Ensure transactional_id is unique per producer
- Check Kafka version (0.11+ required)
- Verify broker configuration
- Check transaction timeout settings

### Circuit Breaker Opening

- Check broker health
- Increase circuit_breaker_threshold
- Review error patterns
- Increase circuit_breaker_timeout_secs

## See Also

- [KafkaSource Documentation](source/README.md)
- [rdkafka Documentation](https://docs.rs/rdkafka)
- [Kafka Producer Configuration](https://kafka.apache.org/documentation/#producerconfigs)
