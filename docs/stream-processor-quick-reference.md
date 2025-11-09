# Stream Processor Quick Reference Card

One-page reference for the LLM Auto-Optimizer Stream Processor.

---

## Core Concepts

### Windows
```rust
// Tumbling: 5-minute non-overlapping
TumblingWindowAssigner::new(Duration::from_secs(300))

// Sliding: 10-minute window, 1-minute slide
SlidingWindowAssigner::new(Duration::from_secs(600), Duration::from_secs(60))

// Session: 5-minute gap
SessionWindowAssigner::new(Duration::from_secs(300))
```

### Aggregations
```rust
// Available aggregators
count       // Count of events
sum         // Sum of values
average     // Mean value
min         // Minimum value
max         // Maximum value
percentile  // p50, p95, p99
stddev      // Standard deviation
```

### Watermarks
```rust
// Bounded out-of-orderness: 30 seconds
BoundedOutOfOrdernessWatermark::new(Duration::from_secs(30))

// Watermark = max_event_time - max_out_of_orderness
```

---

## Basic Pipeline

```rust
use llm_optimizer_processor::*;

let pipeline = create_pipeline::<FeedbackEvent>()
    .with_kafka_source(kafka_config)
    .with_watermark_generator(
        BoundedOutOfOrdernessWatermark::new(Duration::from_secs(30))
    )
    .with_tumbling_window(Duration::from_secs(300))
    .with_state_backend(Arc::new(RocksDBStateBackend::new(path, cp_path)?))
    .with_sink(KafkaAggregationSink::new(producer_config))
    .build()?;

pipeline.run().await?;
```

---

## Configuration Snippets

### Minimal Config
```yaml
processor:
  parallelism: 4
  windows:
    default_type: "tumbling"
    tumbling:
      size_secs: 300
  kafka:
    consumer:
      bootstrap_servers: "localhost:9092"
      topics: ["feedback-events"]
```

### Production Config
```yaml
processor:
  parallelism: 8
  buffer_size: 10000
  checkpoint_interval_secs: 60

  state:
    backend: "rocksdb"
    path: "/var/lib/optimizer/state"
    checkpoint_path: "/var/lib/optimizer/checkpoints"

  windows:
    tumbling:
      size_secs: 300
      allowed_lateness_secs: 60

  watermark:
    max_out_of_orderness_secs: 30

  aggregation:
    enabled: ["count", "average", "percentile"]
    percentiles: [50, 95, 99]
```

---

## State Operations

```rust
// Get/Put state
let state_mgr = WindowStateManager::<AggregationState>::new(backend, "namespace");

let acc = state_mgr.get_accumulator(&window).await?;
state_mgr.put_accumulator(&window, &acc).await?;
state_mgr.delete_window(&window).await?;

// Checkpoint
state_backend.checkpoint(checkpoint_id).await?;
state_backend.restore(checkpoint_id).await?;
```

---

## Aggregation Usage

```rust
// Create aggregation state
let mut state = AggregationState::new(window);

// Add values
state.add_value(42.0);
state.add_value(50.0);

// Get results
let result = state.to_result();
println!("Count: {}", result.results["count"]);
println!("Average: {}", result.results["average"]);
println!("p95: {}", result.results["p95"]);
```

---

## Error Handling

```rust
match result {
    Ok(value) => { /* ... */ },
    Err(ProcessorError::Kafka(e)) => { /* retry */ },
    Err(ProcessorError::State(e)) => { /* checkpoint */ },
    Err(e) => { /* log and continue */ },
}
```

---

## Metrics

### Expose Metrics
```rust
let metrics = ProcessorMetrics::new();
metrics.record_event_processed();
metrics.record_processing_latency(duration);
metrics.record_error();
```

### Prometheus Queries
```promql
# Events per second
rate(events_processed_total[1m])

# p95 latency
histogram_quantile(0.95, processing_latency_seconds)

# Consumer lag
kafka_consumer_lag

# State size
state_size_bytes
```

---

## Common Patterns

### Keyed Aggregation
```rust
let key = GroupingKey::from_fields(&[
    ("model", "claude-3-opus"),
    ("region", "us-west-2"),
]);

let event = ProcessorEvent {
    key: Some(key),
    // ...
};
```

### Custom Sink
```rust
struct CustomSink;

#[async_trait]
impl EventSink<AggregationResult> for CustomSink {
    async fn send(&self, result: AggregationResult) -> ProcessorResult<()> {
        // Custom output logic
        Ok(())
    }
}
```

---

## Performance Tuning

### High Throughput
```yaml
parallelism: 16
buffer_size: 50000
kafka:
  consumer:
    max_poll_records: 2000
```

### Low Latency
```yaml
parallelism: 2
buffer_size: 1000
watermark:
  emit_interval_secs: 1
```

### Memory Efficient
```yaml
state:
  backend: "rocksdb"
aggregation:
  enabled: ["count", "average"]  # minimal set
```

---

## Troubleshooting

| Issue | Solution |
|-------|----------|
| High consumer lag | Increase parallelism or scale horizontally |
| Late events dropped | Increase allowed_lateness_secs or max_out_of_orderness_secs |
| State size growth | Reduce allowed_lateness_secs, increase checkpoint frequency |
| High memory usage | Use RocksDB backend, reduce percentile tracking |
| Slow checkpoints | Reduce state size, use SSD storage |

---

## Commands

### Run Processor
```bash
# Development
cargo run --bin processor -- --config config/processor-dev.yaml

# Production
./target/release/processor --config /etc/optimizer/processor.yaml

# Docker
docker run -v /var/lib/optimizer:/var/lib/optimizer \
  llm-optimizer-processor:latest
```

### Monitor
```bash
# Check metrics
curl http://localhost:8080/metrics

# Check health
curl http://localhost:8080/health

# View logs
kubectl logs -f processor-0 -n llm-optimizer
```

---

## Key Formulas

### Watermark
```
watermark = max_observed_event_time - max_out_of_orderness
```

### Late Event Decision
```
if event_time >= watermark:
    accept (on-time)
elif event_time >= watermark - allowed_lateness:
    accept (late but allowed)
else:
    drop (too late)
```

### Window Assignment (Tumbling)
```
window_start = (event_timestamp / window_size) * window_size
window_end = window_start + window_size
```

### Window Assignment (Sliding)
```
for each window where:
    window_start = n * slide_interval
    window_start <= event_timestamp < window_start + window_size
```

---

## Data Structures

### ProcessorEvent
```rust
ProcessorEvent {
    data: T,                        // Original event
    timestamp: EventTimestamp,      // Event/processing/ingestion time
    processing_time: DateTime<Utc>, // When received
    key: Option<GroupingKey>,       // Partition key
    metadata: EventMetadata,        // Partition, offset, etc.
}
```

### Window
```rust
Window {
    start: DateTime<Utc>,  // Inclusive
    end: DateTime<Utc>,    // Exclusive
    key: Option<GroupingKey>,
}
```

### AggregationResult
```rust
AggregationResult {
    window: Window,
    results: HashMap<String, Value>,  // "count", "average", etc.
    timestamp: DateTime<Utc>,
}
```

---

## Environment Variables

```bash
# Override config values
PROCESSOR_PARALLELISM=8
PROCESSOR_STATE__BACKEND=rocksdb
PROCESSOR_KAFKA__CONSUMER__BOOTSTRAP_SERVERS=kafka:9092
PROCESSOR_WATERMARK__MAX_OUT_OF_ORDERNESS_SECS=60
```

---

## Links

- **Full Architecture**: [stream-processor-architecture.md](./stream-processor-architecture.md)
- **API Reference**: [stream-processor-api-reference.md](./stream-processor-api-reference.md)
- **Implementation Plan**: [stream-processor-implementation-plan.md](./stream-processor-implementation-plan.md)
- **Diagrams**: [stream-processor-diagrams.md](./stream-processor-diagrams.md)
- **Main README**: [stream-processor-README.md](./stream-processor-README.md)

---

**Print this page for quick reference during development!**
