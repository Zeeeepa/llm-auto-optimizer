# KafkaSink Quick Start Guide

## Installation

Add to your `Cargo.toml`:
```toml
[dependencies]
processor = { path = "path/to/processor" }
```

## Basic Usage (3 Steps)

### 1. Define Your Message Type

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct MyResult {
    service: String,
    latency: f64,
    count: u64,
}
```

### 2. Create Sink

```rust
use processor::kafka::{KafkaSink, KafkaSinkConfig};

let config = KafkaSinkConfig {
    brokers: "localhost:9092".to_string(),
    topic: "my-topic".to_string(),
    ..Default::default()
};

let sink = KafkaSink::<MyResult>::new(config).await?;
```

### 3. Send Messages

```rust
use processor::kafka::SinkMessage;

let result = MyResult {
    service: "api".to_string(),
    latency: 123.45,
    count: 1000,
};

let message = SinkMessage::new(result)
    .with_key("api".to_string());

sink.send(message).await?;
sink.close().await?;
```

## Common Configurations

### High Throughput

```rust
KafkaSinkConfig {
    brokers: "localhost:9092".to_string(),
    topic: "high-volume".to_string(),
    batch_size: 10000,
    linger_ms: 100,
    compression_type: "lz4".to_string(),
    buffer_memory: 67108864,  // 64MB
    ..Default::default()
}
```

### Low Latency

```rust
KafkaSinkConfig {
    brokers: "localhost:9092".to_string(),
    topic: "low-latency".to_string(),
    batch_size: 1,
    linger_ms: 0,
    compression_type: "none".to_string(),
    ..Default::default()
}
```

### Production (Exactly-Once)

```rust
KafkaSinkConfig {
    brokers: "localhost:9092".to_string(),
    topic: "production".to_string(),
    enable_idempotence: true,
    enable_transactions: true,
    transactional_id: Some("my-producer".to_string()),
    acks: "all".to_string(),
    max_retries: 10,
    ..Default::default()
}
```

## Common Operations

### Send Batch

```rust
let messages: Vec<_> = results
    .into_iter()
    .map(|r| SinkMessage::new(r))
    .collect();

sink.send_batch(messages).await?;
```

### Use Transactions

```rust
sink.begin_transaction().await?;

for result in results {
    sink.send(SinkMessage::new(result)).await?;
}

sink.commit_transaction().await?;
```

### Add Headers

```rust
let message = SinkMessage::new(result)
    .with_key("service-a".to_string())
    .with_header("trace-id".to_string(), "12345".to_string())
    .with_header("region".to_string(), "us-east-1".to_string());
```

### Check Metrics

```rust
let metrics = sink.metrics().await;

println!("Sent: {}", metrics.messages_sent);
println!("Failed: {}", metrics.messages_failed);
println!("Avg latency: {}us", metrics.avg_latency_us);
```

### Custom Partition

```rust
use processor::kafka::PartitionStrategy;

let sink = KafkaSink::<MyResult>::new(config)
    .await?
    .with_partition_strategy(PartitionStrategy::Custom);

let message = SinkMessage::new(result)
    .with_partition(2);  // Send to partition 2
```

### Binary Serialization

```rust
use processor::kafka::BincodeSerializer;
use std::sync::Arc;

let sink = KafkaSink::<MyResult>::with_serializer(
    config,
    Arc::new(BincodeSerializer),
).await?;
```

## Error Handling

```rust
match sink.send(message).await {
    Ok(_) => println!("Sent successfully"),
    Err(e) => {
        eprintln!("Send failed: {}", e);

        let metrics = sink.metrics().await;
        if let Some(ref err_msg) = metrics.last_error_msg {
            eprintln!("Last error: {}", err_msg);
        }
    }
}
```

## Complete Example

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
    // 1. Configure
    let config = KafkaSinkConfig {
        brokers: "localhost:9092".to_string(),
        topic: "metrics".to_string(),
        enable_idempotence: true,
        compression_type: "snappy".to_string(),
        ..Default::default()
    };

    // 2. Create sink
    let sink = KafkaSink::<MetricResult>::new(config).await?;

    // 3. Send messages
    let results = vec![
        MetricResult {
            service: "api-gateway".to_string(),
            avg_latency: 123.45,
            count: 1000,
        },
        MetricResult {
            service: "auth-service".to_string(),
            avg_latency: 45.67,
            count: 500,
        },
    ];

    for result in results {
        let message = SinkMessage::new(result.clone())
            .with_key(result.service.clone());

        sink.send(message).await?;
    }

    // 4. Check metrics
    let metrics = sink.metrics().await;
    println!("Sent {} messages", metrics.messages_sent);
    println!("Avg latency: {}us", metrics.avg_latency_us);

    // 5. Graceful shutdown
    sink.close().await?;

    Ok(())
}
```

## Troubleshooting

### "Connection refused"
- Ensure Kafka is running: `docker-compose up -d kafka`
- Check broker address in config

### "Topic does not exist"
- Create topic: `kafka-topics --create --topic my-topic --bootstrap-server localhost:9092`
- Or enable auto-create in Kafka config

### "Transaction error"
- Ensure `transactional_id` is set
- Check Kafka version >= 0.11
- Verify `enable_idempotence` is true

### High latency
- Reduce `batch_size`
- Set `linger_ms` to 0
- Disable compression

### Low throughput
- Increase `batch_size`
- Increase `linger_ms`
- Enable compression (lz4 or snappy)

## Next Steps

- Read the [full documentation](README.md)
- Check out [examples](../../examples/kafka_sink_example.rs)
- Run [tests](../../tests/kafka_sink_tests.rs)
- See [implementation details](../../KAFKA_SINK_IMPLEMENTATION.md)

## Cheat Sheet

| Task | Code |
|------|------|
| Create sink | `KafkaSink::new(config).await?` |
| Send message | `sink.send(message).await?` |
| Send batch | `sink.send_batch(messages).await?` |
| Begin transaction | `sink.begin_transaction().await?` |
| Commit transaction | `sink.commit_transaction().await?` |
| Abort transaction | `sink.abort_transaction().await?` |
| Flush pending | `sink.flush().await?` |
| Get metrics | `sink.metrics().await` |
| Close sink | `sink.close().await?` |
| Add key | `.with_key("key".to_string())` |
| Add header | `.with_header("k".to_string(), "v".to_string())` |
| Set partition | `.with_partition(2)` |
| Set topic | `.with_topic("topic".to_string())` |
