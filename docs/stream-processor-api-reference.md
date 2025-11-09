# Stream Processor API Reference

Quick reference guide for the LLM Auto-Optimizer Stream Processor API.

---

## Quick Start

### Basic Tumbling Window Example

```rust
use llm_optimizer_processor::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a simple 5-minute tumbling window pipeline
    let pipeline = create_pipeline::<FeedbackEvent>()
        .with_kafka_source(KafkaConfig {
            bootstrap_servers: "localhost:9092".to_string(),
            group_id: "my-processor".to_string(),
            topics: vec!["feedback-events".to_string()],
            ..Default::default()
        })
        .with_watermark_generator(
            BoundedOutOfOrdernessWatermark::new(Duration::from_secs(30))
        )
        .with_tumbling_window(Duration::from_secs(300))
        .with_state_backend(Arc::new(MemoryStateBackend::new()))
        .with_sink(ConsoleSink::new())
        .build()?;

    pipeline.run().await?;
    Ok(())
}
```

---

## Window Types

### Tumbling Windows

Fixed-size, non-overlapping windows.

```rust
// Create 5-minute tumbling windows
let assigner = TumblingWindowAssigner::new(Duration::from_secs(300));

// With offset (e.g., start at :00 instead of epoch alignment)
let assigner = TumblingWindowAssigner::new(Duration::from_secs(300))
    .with_offset(Duration::from_secs(0))
    .with_allowed_lateness(Duration::from_secs(60));
```

**Use Cases**:
- Hourly/daily aggregations
- Fixed reporting periods
- Simple counting and averaging

### Sliding Windows

Fixed-size, overlapping windows.

```rust
// Create 10-minute windows sliding every 1 minute
let assigner = SlidingWindowAssigner::new(
    Duration::from_secs(600),  // window size
    Duration::from_secs(60),   // slide interval
);
```

**Use Cases**:
- Moving averages
- Rolling statistics
- Continuous monitoring

### Session Windows

Dynamic windows based on inactivity gaps.

```rust
// Create session windows with 5-minute gap
let assigner = SessionWindowAssigner::new(Duration::from_secs(300));
```

**Use Cases**:
- User session analysis
- Click stream analysis
- Activity clustering

---

## Aggregation Functions

### Count

```rust
use llm_optimizer_processor::aggregation::*;

let aggregator = CountAggregator;
let mut acc = aggregator.create_accumulator();

aggregator.add(&mut acc, &value);
let result = aggregator.get_result(&acc);
println!("Count: {}", result.count);
```

### Sum and Average

```rust
let sum_agg = SumAggregator;
let avg_agg = AverageAggregator;

// Average accumulator
let mut acc = avg_agg.create_accumulator();
avg_agg.add(&mut acc, &10.0);
avg_agg.add(&mut acc, &20.0);
avg_agg.add(&mut acc, &30.0);

println!("Average: {}", acc.average());  // 20.0
println!("Sum: {}", acc.sum);            // 60.0
println!("Count: {}", acc.count);        // 3
```

### Min and Max

```rust
let mut state = AggregationState::new(window);

state.add_value(10.0);
state.add_value(50.0);
state.add_value(30.0);

println!("Min: {}", state.min);  // 10.0
println!("Max: {}", state.max);  // 50.0
```

### Percentiles

```rust
let percentile_agg = PercentileAggregator;
let mut acc = percentile_agg.create_accumulator();

for value in &[10.0, 20.0, 30.0, 40.0, 50.0] {
    percentile_agg.add(&mut acc, value);
}

println!("p50: {:?}", acc.p50());  // 30.0
println!("p95: {:?}", acc.p95());  // 50.0
println!("p99: {:?}", acc.p99());  // 50.0
```

### Standard Deviation

```rust
let mut state = AggregationState::new(window);

for value in &[10.0, 20.0, 30.0, 40.0, 50.0] {
    state.add_value(*value);
}

println!("StdDev: {}", state.standard_deviation());
```

### Composite Aggregation

```rust
// The AggregationState provides all metrics at once
let mut state = AggregationState::new(window);

state.add_value(10.0);
state.add_value(20.0);
state.add_value(30.0);

// Get comprehensive results
let result = state.to_result();

println!("Count: {}", result.results["count"]);
println!("Sum: {}", result.results["sum"]);
println!("Average: {}", result.results["average"]);
println!("Min: {}", result.results["min"]);
println!("Max: {}", result.results["max"]);
println!("StdDev: {}", result.results["stddev"]);
println!("p50: {}", result.results["p50"]);
println!("p95: {}", result.results["p95"]);
println!("p99: {}", result.results["p99"]);
```

---

## Watermarking

### Bounded Out-of-Orderness

Most common strategy for handling late events.

```rust
let watermark_gen = BoundedOutOfOrdernessWatermark::new(
    Duration::from_secs(30)  // Allow up to 30 seconds of disorder
);

// Use in pipeline
let pipeline = create_pipeline()
    .with_watermark_generator(watermark_gen)
    // ...
    .build()?;
```

### Periodic Watermarks

Emit watermarks at regular intervals.

```rust
let base_gen = BoundedOutOfOrdernessWatermark::new(Duration::from_secs(30));
let periodic_gen = PeriodicWatermarkGenerator::new(
    Duration::from_secs(10),  // Emit every 10 seconds
    Box::new(base_gen),
);
```

### Late Event Handling

```rust
let late_handler = LateEventHandler::new(Duration::from_secs(60))
    .with_side_output(DeadLetterSink::new("late-events-topic"));

// Late events beyond 60 seconds are sent to dead letter queue
```

---

## State Management

### In-Memory State Backend

For development and testing.

```rust
let state_backend = Arc::new(MemoryStateBackend::new());

let pipeline = create_pipeline()
    .with_state_backend(state_backend)
    .build()?;
```

### RocksDB State Backend

For production use with persistence.

```rust
let state_backend = Arc::new(
    RocksDBStateBackend::new(
        "/var/lib/optimizer/state",
        "/var/lib/optimizer/checkpoints",
    )?
);

let pipeline = create_pipeline()
    .with_state_backend(state_backend)
    .build()?;
```

### State Operations

```rust
use llm_optimizer_processor::state::*;

// Create state manager
let state_manager = WindowStateManager::<AggregationState>::new(
    state_backend,
    "my-aggregation",
);

// Get accumulator for window
let acc = state_manager.get_accumulator(&window).await?;

// Update and save
let mut acc = acc.unwrap_or_else(|| AggregationState::new(window.clone()));
acc.add_value(42.0);
state_manager.put_accumulator(&window, &acc).await?;

// Delete window state
state_manager.delete_window(&window).await?;
```

### Checkpointing

```rust
// Checkpoint state
state_backend.checkpoint(checkpoint_id).await?;

// Restore from checkpoint
state_backend.restore(checkpoint_id).await?;
```

---

## Kafka Integration

### Kafka Source Configuration

```rust
let kafka_config = KafkaConfig {
    bootstrap_servers: "localhost:9092".to_string(),
    group_id: "processor-group".to_string(),
    topics: vec!["feedback-events".to_string()],
    enable_auto_commit: false,
    auto_offset_reset: "earliest".to_string(),
    session_timeout_ms: 30000,
    max_poll_records: 500,
};

let pipeline = create_pipeline()
    .with_kafka_source(kafka_config)
    .build()?;
```

### Kafka Sink Configuration

```rust
let producer_config = KafkaProducerConfig {
    bootstrap_servers: "localhost:9092".to_string(),
    result_topic: "aggregated-metrics".to_string(),
    compression_type: "lz4".to_string(),
    acks: "all".to_string(),
    retries: 3,
};

let sink = KafkaAggregationSink::new(producer_config);

let pipeline = create_pipeline()
    .with_sink(sink)
    .build()?;
```

---

## Pipeline Building

### Builder Pattern

```rust
let pipeline = create_pipeline::<FeedbackEvent>()
    // Source configuration
    .with_kafka_source(kafka_config)

    // Watermark strategy
    .with_watermark_generator(
        BoundedOutOfOrdernessWatermark::new(Duration::from_secs(30))
    )

    // Window type (choose one)
    .with_tumbling_window(Duration::from_secs(300))
    // .with_sliding_window(Duration::from_secs(600), Duration::from_secs(60))
    // .with_session_window(Duration::from_secs(300))

    // State backend
    .with_state_backend(state_backend)

    // Sinks (can add multiple)
    .with_sink(KafkaAggregationSink::new(producer_config))
    .with_sink(ConsoleSink::new())
    .with_sink(MetricsSink::new(prometheus_registry))

    // Build and validate
    .build()?;

// Run pipeline
pipeline.run().await?;
```

### Custom Event Processing

```rust
// Define custom event type
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CustomEvent {
    metric_name: String,
    value: f64,
    timestamp: DateTime<Utc>,
}

// Create pipeline for custom events
let pipeline = create_pipeline::<CustomEvent>()
    .with_kafka_source(kafka_config)
    // ... rest of configuration
    .build()?;
```

---

## Error Handling

### Error Types

```rust
use llm_optimizer_processor::error::*;

match result {
    Ok(value) => println!("Success: {:?}", value),
    Err(ProcessorError::WindowAssignment(msg)) => {
        eprintln!("Window error: {}", msg);
    }
    Err(ProcessorError::Aggregation(msg)) => {
        eprintln!("Aggregation error: {}", msg);
    }
    Err(ProcessorError::State(msg)) => {
        eprintln!("State error: {}", msg);
    }
    Err(ProcessorError::Kafka(e)) => {
        eprintln!("Kafka error: {}", e);
    }
    Err(e) => {
        eprintln!("Other error: {}", e);
    }
}
```

### Error Recovery Policies

```rust
let error_handler = ErrorHandler::new(ErrorPolicy::Retry {
    max_attempts: 3,
    initial_delay: Duration::from_millis(100),
    max_delay: Duration::from_secs(5),
});

// Or use skip policy
let error_handler = ErrorHandler::new(ErrorPolicy::Skip);

// Or use dead letter queue
let error_handler = ErrorHandler::new(ErrorPolicy::DeadLetter)
    .with_sink(DeadLetterSink::new("errors-topic"));
```

---

## Configuration File

### Complete Configuration Example

```yaml
processor:
  parallelism: 4
  buffer_size: 10000
  checkpoint_interval_secs: 60

  state:
    backend: "rocksdb"
    path: "/var/lib/optimizer/state"
    checkpoint_path: "/var/lib/optimizer/checkpoints"

  windows:
    default_type: "tumbling"

    tumbling:
      size_secs: 300
      allowed_lateness_secs: 60

    sliding:
      size_secs: 600
      slide_secs: 60
      allowed_lateness_secs: 60

    session:
      gap_secs: 300
      allowed_lateness_secs: 60

  watermark:
    strategy: "bounded_out_of_orderness"
    max_out_of_orderness_secs: 30
    emit_interval_secs: 10

  aggregation:
    enabled:
      - "count"
      - "sum"
      - "average"
      - "min"
      - "max"
      - "percentile"
      - "stddev"
    percentiles: [50, 95, 99]

  error_handling:
    policy: "retry"
    max_retries: 3
    initial_retry_delay_ms: 100
    max_retry_delay_ms: 5000
    dead_letter_topic: "processor-dead-letter"

  kafka:
    consumer:
      bootstrap_servers: "localhost:9092"
      group_id: "llm-optimizer-processor"
      topics: ["feedback-events", "performance-metrics"]
      enable_auto_commit: false
      auto_offset_reset: "earliest"
      session_timeout_ms: 30000
      max_poll_records: 500

    producer:
      bootstrap_servers: "localhost:9092"
      result_topic: "aggregated-metrics"
      compression_type: "lz4"
      acks: "all"
      retries: 3

  metrics:
    enabled: true
    export_interval_secs: 60
```

### Loading Configuration

```rust
// From file
let config = load_config("processor-config.yaml").await?;

// From environment variables (overrides file)
// PROCESSOR_PARALLELISM=8
// PROCESSOR_STATE__BACKEND=memory
// PROCESSOR_KAFKA__CONSUMER__BOOTSTRAP_SERVERS=kafka:9092
```

---

## Metrics and Observability

### Built-in Metrics

```rust
use llm_optimizer_processor::metrics::*;

let metrics = ProcessorMetrics::new();

// Record processing latency
let start = Instant::now();
// ... process event ...
metrics.record_processing_latency(start.elapsed());

// Record throughput
metrics.record_event_processed();

// Record errors
metrics.record_error();

// Record state size
metrics.record_state_size(state_size_bytes);

// Export metrics
let prometheus_metrics = metrics.export();
```

### Custom Tracing

```rust
use tracing::{info, warn, error, debug, instrument};

#[instrument(skip(event))]
async fn process_event(event: ProcessorEvent<FeedbackEvent>) -> ProcessorResult<()> {
    info!("Processing event: {}", event.id);

    // ... processing logic ...

    debug!("Event processed successfully");
    Ok(())
}
```

---

## Advanced Usage

### Custom Aggregator

```rust
use llm_optimizer_processor::aggregation::*;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CustomAccumulator {
    pub sum_of_squares: f64,
    pub count: u64,
}

struct VarianceAggregator;

impl Aggregator<f64, CustomAccumulator> for VarianceAggregator {
    fn create_accumulator(&self) -> CustomAccumulator {
        CustomAccumulator::default()
    }

    fn add(&self, acc: &mut CustomAccumulator, value: &f64) {
        acc.sum_of_squares += value * value;
        acc.count += 1;
    }

    fn merge(&self, acc1: &CustomAccumulator, acc2: &CustomAccumulator) -> CustomAccumulator {
        CustomAccumulator {
            sum_of_squares: acc1.sum_of_squares + acc2.sum_of_squares,
            count: acc1.count + acc2.count,
        }
    }

    fn get_result(&self, acc: &CustomAccumulator) -> CustomAccumulator {
        acc.clone()
    }

    fn name(&self) -> &str {
        "variance"
    }
}
```

### Custom Window Trigger

```rust
struct HybridTrigger {
    count_threshold: u64,
    time_threshold: Duration,
}

impl WindowTrigger for HybridTrigger {
    fn should_fire(
        &self,
        window: &Window,
        watermark: &Watermark,
        element_count: u64,
    ) -> bool {
        // Fire on count OR time
        element_count >= self.count_threshold ||
        watermark.timestamp >= window.end
    }

    fn should_purge(
        &self,
        window: &Window,
        watermark: &Watermark,
    ) -> bool {
        watermark.timestamp >= window.end + chrono::Duration::hours(1)
    }
}
```

### Custom Sink

```rust
use async_trait::async_trait;

struct PostgresSink {
    pool: sqlx::PgPool,
}

#[async_trait]
impl EventSink<AggregationResult> for PostgresSink {
    async fn send(&self, result: AggregationResult) -> ProcessorResult<()> {
        sqlx::query!(
            "INSERT INTO aggregations (window_start, window_end, results)
             VALUES ($1, $2, $3)",
            result.window.start,
            result.window.end,
            serde_json::to_value(&result.results).unwrap(),
        )
        .execute(&self.pool)
        .await
        .map_err(|e| ProcessorError::Internal(e.to_string()))?;

        Ok(())
    }
}
```

---

## Common Patterns

### Multi-Metric Aggregation

```rust
// Process multiple metrics from the same event stream
let pipeline = create_pipeline::<FeedbackEvent>()
    .with_kafka_source(kafka_config)
    .with_watermark_generator(watermark_gen)
    .with_tumbling_window(Duration::from_secs(300))
    .with_state_backend(state_backend)
    .build()?;

// The AggregationState automatically computes all metrics
// Access specific metrics from the result
```

### Keyed Aggregation

```rust
// Group events by key before aggregation
let key = GroupingKey::from_fields(&[
    ("model", "claude-3-opus"),
    ("region", "us-west-2"),
]);

let event = ProcessorEvent {
    data: feedback_event,
    timestamp: EventTimestamp::Event(Utc::now()),
    processing_time: Utc::now(),
    key: Some(key),
    metadata: EventMetadata::default(),
};
```

### Real-Time Dashboard

```rust
// Stream results to dashboard via WebSocket
let dashboard_sink = WebSocketSink::new("ws://dashboard:8080/metrics");

let pipeline = create_pipeline()
    .with_tumbling_window(Duration::from_secs(60))  // 1-minute windows
    .with_sink(dashboard_sink)
    .build()?;
```

---

## Performance Tuning

### Throughput Optimization

```yaml
processor:
  parallelism: 8              # Increase for more throughput
  buffer_size: 50000          # Larger buffer for burst traffic

  kafka:
    consumer:
      max_poll_records: 1000  # Fetch more records per poll
```

### Latency Optimization

```yaml
processor:
  parallelism: 2              # Lower parallelism for lower latency
  buffer_size: 1000           # Smaller buffer

  watermark:
    emit_interval_secs: 1     # More frequent watermarks
```

### Memory Optimization

```yaml
processor:
  state:
    backend: "rocksdb"        # Use disk-backed state

  aggregation:
    enabled: ["count", "average"]  # Only compute needed metrics
```

---

## Troubleshooting

### High Consumer Lag

```rust
// Increase parallelism
processor:
  parallelism: 16

// Or increase poll size
kafka:
  consumer:
    max_poll_records: 2000
```

### State Size Growth

```rust
// Reduce allowed lateness
windows:
  tumbling:
    allowed_lateness_secs: 30  # From 60 to 30

// More frequent checkpoints to clean up old state
processor:
  checkpoint_interval_secs: 30  # From 60 to 30
```

### Late Events Dropped

```rust
// Increase allowed lateness
windows:
  tumbling:
    allowed_lateness_secs: 120  # From 60 to 120

// Or increase watermark tolerance
watermark:
  max_out_of_orderness_secs: 60  # From 30 to 60
```

---

## References

- [Architecture Design](/workspaces/llm-auto-optimizer/docs/stream-processor-architecture.md)
- [Implementation Plan](/workspaces/llm-auto-optimizer/docs/stream-processor-implementation-plan.md)
- [Kafka Documentation](https://kafka.apache.org/documentation/)
- [RocksDB Documentation](https://github.com/facebook/rocksdb/wiki)
