# Kafka Stream Processing Integration

This document describes the integration between Kafka and the stream processing pipeline, enabling complete end-to-end Kafka-to-Kafka streaming applications.

## Overview

The Kafka integration provides three main components that work together to create a complete stream processing pipeline:

1. **KafkaStreamSource** - Consumes events from Kafka and feeds them into the pipeline
2. **KafkaStreamSink** - Receives aggregated results from the pipeline and produces them to Kafka
3. **KafkaStreamPipeline** - Orchestrates the complete end-to-end flow with automatic offset management

## Architecture

```
┌─────────────────┐
│  Kafka Topics   │
│  (Input Data)   │
└────────┬────────┘
         │
         ▼
┌─────────────────────┐
│ KafkaStreamSource   │
│ - Poll messages     │
│ - Deserialize       │
│ - Extract metadata  │
└────────┬────────────┘
         │
         ▼
┌─────────────────────┐
│  StreamExecutor     │
│ - Event ingestion   │
│ - Window assignment │
│ - Watermarking      │
│ - Aggregation       │
└────────┬────────────┘
         │
         ▼
┌─────────────────────┐
│  KafkaStreamSink    │
│ - Serialize results │
│ - Produce to Kafka  │
│ - Handle retries    │
└────────┬────────────┘
         │
         ▼
┌─────────────────┐
│  Kafka Topics   │
│  (Results)      │
└─────────────────┘
```

## Components

### 1. KafkaStreamSource

Integrates the Kafka consumer with the StreamExecutor, handling:

- **Message Polling**: Continuously polls messages from Kafka topics
- **Deserialization**: Converts Kafka message payloads to event types
- **Event Time Extraction**: Extracts timestamps from events for event-time processing
- **Key Extraction**: Extracts aggregation keys from events
- **Offset Tracking**: Tracks message offsets for commit after processing

```rust
pub struct KafkaStreamSource<T> {
    consumer: Arc<StreamConsumer>,
    topics: Vec<String>,
    executor: Arc<StreamExecutor<T>>,
    offset_manager: Arc<OffsetManager>,
    stats: Arc<RwLock<KafkaStreamStats>>,
    running: Arc<AtomicBool>,
    poll_timeout: Duration,
}
```

**Key Methods:**
- `run()`: Start consuming messages and feeding to executor
- `process_message()`: Process a single Kafka message
- `stop()`: Stop consuming
- `stats()`: Get consumption statistics

### 2. KafkaStreamSink

Receives aggregated results from the pipeline and produces them to Kafka:

- **Result Reception**: Receives ProcessorEvents from the executor output
- **Serialization**: Converts results to JSON for Kafka
- **Delivery**: Produces messages to output topic with delivery guarantees
- **Retry Logic**: Handles transient failures with exponential backoff
- **Header Management**: Adds event metadata as Kafka headers

```rust
pub struct KafkaStreamSink<R> {
    producer: Arc<FutureProducer>,
    output_topic: String,
    result_rx: Arc<RwLock<mpsc::Receiver<ProcessorEvent<R>>>>,
    stats: Arc<RwLock<KafkaStreamStats>>,
    running: Arc<AtomicBool>,
    delivery_timeout: Duration,
    max_retries: u32,
}
```

**Key Methods:**
- `run()`: Start consuming results and producing to Kafka
- `produce_result()`: Produce a single result with retries
- `stop()`: Stop producing
- `stats()`: Get production statistics

### 3. KafkaStreamPipeline

Orchestrates the complete pipeline:

- **Component Lifecycle**: Manages source, executor, and sink lifecycles
- **Offset Management**: Automatic offset tracking and committing
- **Graceful Shutdown**: Proper cleanup on termination
- **Health Checking**: Monitor component health
- **Error Recovery**: Handle errors and maintain processing

```rust
pub struct KafkaStreamPipeline<T, R> {
    config: KafkaStreamConfig,
    executor: Arc<StreamExecutor<T>>,
    consumer: Arc<StreamConsumer>,
    producer: Arc<FutureProducer>,
    offset_manager: Arc<OffsetManager>,
    source: Option<KafkaStreamSource<T>>,
    sink: Option<KafkaStreamSink<R>>,
    stats: Arc<RwLock<KafkaStreamStats>>,
    running: Arc<AtomicBool>,
}
```

**Key Methods:**
- `new()`: Create pipeline with configuration
- `run()`: Run the complete pipeline
- `shutdown()`: Gracefully shutdown
- `health_check()`: Check pipeline health
- `stats()`: Get pipeline statistics

### 4. OffsetManager

Manages Kafka offset tracking and committing:

- **Offset Tracking**: Track offsets for processed messages
- **Auto-Commit**: Periodic automatic offset commits
- **Manual Commit**: On-demand offset commits
- **Partition Management**: Track offsets per partition

```rust
pub struct OffsetManager {
    pending_offsets: Arc<RwLock<HashMap<(String, i32), i64>>>,
    consumer: Arc<StreamConsumer>,
    stats: Arc<RwLock<KafkaStreamStats>>,
    auto_commit_interval: Duration,
    running: Arc<AtomicBool>,
}
```

**Key Methods:**
- `track_offset()`: Track an offset for commit
- `commit_offsets()`: Commit all pending offsets
- `start_auto_commit()`: Start automatic committing
- `stop()`: Stop auto-commit

## Configuration

### KafkaStreamConfig

Complete configuration for the pipeline:

```rust
pub struct KafkaStreamConfig {
    /// Kafka brokers (comma-separated)
    pub brokers: String,

    /// Consumer group ID
    pub group_id: String,

    /// Input topics to consume from
    pub input_topics: Vec<String>,

    /// Output topic to produce to
    pub output_topic: String,

    /// Auto-commit interval (milliseconds)
    pub auto_commit_interval_ms: u64,

    /// Poll timeout (milliseconds)
    pub poll_timeout_ms: u64,

    /// Delivery timeout (milliseconds)
    pub delivery_timeout_ms: u64,

    /// Max retries for producing
    pub max_produce_retries: u32,

    /// Additional consumer config
    pub consumer_config: HashMap<String, String>,

    /// Additional producer config
    pub producer_config: HashMap<String, String>,
}
```

**Default Values:**
- `brokers`: "localhost:9092"
- `group_id`: "stream-processor"
- `input_topics`: ["input"]
- `output_topic`: "output"
- `auto_commit_interval_ms`: 5000
- `poll_timeout_ms`: 100
- `delivery_timeout_ms`: 5000
- `max_produce_retries`: 3

## Statistics

### KafkaStreamStats

Comprehensive statistics for monitoring:

```rust
pub struct KafkaStreamStats {
    pub messages_consumed: u64,
    pub messages_produced: u64,
    pub consumer_lag: i64,
    pub consume_errors: u64,
    pub produce_errors: u64,
    pub offsets_committed: u64,
    pub commit_errors: u64,
    pub committed_offsets: HashMap<i32, i64>,
}
```

## Usage Example

### Basic Pipeline

```rust
use processor::kafka::{KafkaStreamConfig, KafkaStreamPipeline};
use processor::pipeline::StreamPipelineBuilder;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MyEvent {
    value: f64,
    timestamp: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MyResult {
    avg: f64,
    count: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create stream pipeline
    let pipeline = StreamPipelineBuilder::new()
        .with_name("my-pipeline")
        .with_tumbling_window(60_000) // 1-minute windows
        .build()?;

    let executor = pipeline.create_executor();

    // Configure Kafka
    let config = KafkaStreamConfig {
        brokers: "localhost:9092".to_string(),
        group_id: "my-processor".to_string(),
        input_topics: vec!["input-events".to_string()],
        output_topic: "output-results".to_string(),
        ..Default::default()
    };

    // Create pipeline
    let mut kafka_pipeline = KafkaStreamPipeline::new(config, executor)?;

    // Create result channel
    let (tx, rx) = mpsc::channel(1000);

    // Run pipeline
    kafka_pipeline.run(rx).await?;

    Ok(())
}
```

### With Custom Configuration

```rust
use std::collections::HashMap;

let mut consumer_config = HashMap::new();
consumer_config.insert("session.timeout.ms".to_string(), "30000".to_string());
consumer_config.insert("heartbeat.interval.ms".to_string(), "10000".to_string());
consumer_config.insert("max.poll.records".to_string(), "500".to_string());

let mut producer_config = HashMap::new();
producer_config.insert("compression.type".to_string(), "lz4".to_string());
producer_config.insert("acks".to_string(), "all".to_string());
producer_config.insert("max.in.flight.requests.per.connection".to_string(), "5".to_string());

let config = KafkaStreamConfig {
    brokers: "broker1:9092,broker2:9092".to_string(),
    group_id: "my-processor-group".to_string(),
    input_topics: vec![
        "topic1".to_string(),
        "topic2".to_string(),
    ],
    output_topic: "aggregated-results".to_string(),
    auto_commit_interval_ms: 10_000,
    poll_timeout_ms: 100,
    delivery_timeout_ms: 5000,
    max_produce_retries: 5,
    consumer_config,
    producer_config,
};
```

### Graceful Shutdown

```rust
use tokio::signal;

// Run pipeline in background
let mut kafka_pipeline = KafkaStreamPipeline::new(config, executor)?;
let (tx, rx) = mpsc::channel(1000);

let pipeline_handle = tokio::spawn(async move {
    kafka_pipeline.run(rx).await
});

// Wait for shutdown signal
signal::ctrl_c().await?;

// Graceful shutdown
pipeline_handle.abort();
kafka_pipeline.shutdown().await?;
```

## Features

### Automatic Offset Management

The pipeline automatically tracks and commits offsets:

1. **On Message Processing**: Offsets are tracked when messages are successfully processed
2. **Periodic Commits**: Offsets are committed at regular intervals
3. **On Shutdown**: Final offsets are committed before termination

### Error Recovery

The integration handles various error scenarios:

- **Transient Consumer Errors**: Automatically retries consumption
- **Transient Producer Errors**: Retries with exponential backoff
- **Deserialization Errors**: Logs and continues processing
- **Network Errors**: Reconnects and resumes

### Watermark Generation

Integrates seamlessly with the stream processing watermark system:

- Extracts event times from Kafka messages
- Updates watermarks as events are consumed
- Triggers windows based on watermark progress

### Monitoring

Provides comprehensive statistics:

- Messages consumed/produced
- Error counts
- Committed offsets per partition
- Consumer lag (when available)

## Complete Example

See `/workspaces/llm-auto-optimizer/crates/processor/examples/kafka_pipeline.rs` for a complete working example that demonstrates:

- Full Kafka-to-Kafka pipeline
- Event consumption from input topic
- Tumbling window aggregation
- Result production to output topic
- Graceful shutdown
- Statistics reporting
- Error handling

## Best Practices

### 1. Event Types

Implement required traits for your event types:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
struct MyEvent {
    timestamp: i64,
    key: String,
    value: f64,
}

impl EventTimeExtractor<MyEvent> for MyEvent {
    fn extract_event_time(&self, event: &MyEvent) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(event.timestamp)
            .unwrap_or_else(Utc::now)
    }
}

impl KeyExtractor<MyEvent> for MyEvent {
    fn extract_key(&self, event: &MyEvent) -> EventKey {
        EventKey::Session(event.key.clone())
    }
}
```

### 2. Buffer Sizing

Configure appropriate buffer sizes:

```rust
let pipeline = StreamPipelineBuilder::new()
    .with_buffer_size(10_000) // Internal executor buffer
    .build()?;

let (tx, rx) = mpsc::channel(1000); // Result channel buffer
```

### 3. Monitoring

Regularly check statistics:

```rust
tokio::spawn(async move {
    let mut ticker = tokio::time::interval(Duration::from_secs(10));
    loop {
        ticker.tick().await;
        let stats = kafka_pipeline.stats().await;
        info!(
            consumed = stats.messages_consumed,
            produced = stats.messages_produced,
            errors = stats.consume_errors + stats.produce_errors,
            "Pipeline stats"
        );
    }
});
```

### 4. Health Checks

Implement health checking:

```rust
let is_healthy = kafka_pipeline.health_check().await;
if !is_healthy {
    warn!("Pipeline health check failed");
    // Take corrective action
}
```

## Testing

The integration includes comprehensive unit tests in `/workspaces/llm-auto-optimizer/crates/processor/src/kafka/tests.rs`:

- Configuration validation
- Statistics tracking
- Concurrent access handling
- Event extractor behavior

Run tests with:

```bash
cargo test -p processor --lib kafka
```

## Performance Considerations

### Throughput

- Adjust `poll_timeout_ms` for optimal polling frequency
- Configure producer batching and compression
- Use multiple partitions for parallelism

### Latency

- Reduce `auto_commit_interval_ms` for lower latency
- Use smaller buffer sizes
- Tune watermark delay

### Resource Usage

- Monitor memory usage with large buffers
- Configure appropriate executor parallelism
- Use state backend cleanup policies

## Troubleshooting

### High Consumer Lag

- Increase parallelism
- Optimize aggregation logic
- Check for bottlenecks in processing

### Offset Commit Failures

- Check Kafka broker health
- Verify consumer group configuration
- Monitor commit error statistics

### Message Loss

- Ensure offsets are committed after processing
- Use appropriate delivery guarantees
- Monitor produce error counts

## Future Enhancements

Potential improvements:

- [ ] Exactly-once semantics with transactions
- [ ] Multiple output topics
- [ ] Dynamic topic subscription
- [ ] Advanced partitioning strategies
- [ ] State backend integration for offsets
- [ ] Metrics export (Prometheus, etc.)
- [ ] Dead letter queue support
