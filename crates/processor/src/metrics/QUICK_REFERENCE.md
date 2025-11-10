# Metrics Quick Reference Guide

Quick reference for all Prometheus metrics in the Stream Processor.

## Setup (Copy-Paste Ready)

```rust
use processor::metrics::{
    MetricsRegistry, ProcessorMetrics, MetricsServer, MetricsServerConfig,
    labels::*,
};
use std::sync::Arc;

// Initialize metrics
let registry = MetricsRegistry::global();
let mut reg = registry.registry().write();
let metrics = Arc::new(ProcessorMetrics::new(&mut reg));
drop(reg);

// Start HTTP server (optional)
let config = MetricsServerConfig::new("0.0.0.0", 9090);
let server = MetricsServer::with_registry(config, registry);
tokio::spawn(async move {
    server.start().await.expect("Metrics server failed");
});
```

## Metric Quick Reference

### Stream Processing

```rust
// Event received
metrics.record_event_received();

// Event processed
metrics.record_event_processed(ResultLabel::Success);
// ResultLabel: Success | Error | Dropped | Filtered

// Processing duration (seconds)
metrics.record_event_processing_duration(0.001, ResultLabel::Success);

// Pipeline lag (seconds)
metrics.set_pipeline_lag(1.5);

// Backpressure
metrics.set_backpressure_events(100);
```

### Windows

```rust
// Window created
metrics.record_window_created(WindowTypeLabel::Tumbling);
// WindowTypeLabel: Tumbling | Sliding | Session

// Window triggered
metrics.record_window_triggered(WindowTypeLabel::Tumbling);

// Active windows
metrics.set_windows_active(WindowTypeLabel::Tumbling, 5);

// Window size
metrics.record_window_size(WindowTypeLabel::Tumbling, 100.0);

// Late event
metrics.record_late_event();
```

### Aggregations

```rust
// Aggregation computed
metrics.aggregations_computed_total
    .get_or_create(&vec![("aggregate_type".to_string(), "sum".to_string())])
    .inc();

// Aggregation duration
metrics.aggregation_duration_seconds
    .get_or_create(&vec![("aggregate_type".to_string(), "avg".to_string())])
    .observe(0.005);

// Aggregation error
metrics.aggregation_errors_total
    .get_or_create(&vec![("aggregate_type".to_string(), "max".to_string())])
    .inc();
```

### State Backend

```rust
// State operation
metrics.record_state_operation(
    OperationLabel::Get,
    BackendLabel::Redis,
    0.001, // duration in seconds
);
// OperationLabel: Get | Put | Delete | Scan | BatchGet | BatchPut
// BackendLabel: InMemory | Sled | Redis | Postgres

// Cache hit
metrics.record_state_cache_hit(CacheLayerLabel::L1);
// CacheLayerLabel: L1 | L2 | L3

// Cache miss
metrics.record_state_cache_miss(CacheLayerLabel::L2);

// State size (bytes)
metrics.set_state_size_bytes(BackendLabel::Redis, 1048576);

// State eviction
metrics.state_evictions_total
    .get_or_create(&vec![("backend".to_string(), "redis".to_string())])
    .inc();
```

### Deduplication

```rust
// Check for duplicate (automatically increments both metrics)
metrics.record_dedup_check(is_duplicate);

// Manual increment
metrics.dedup_events_checked_total.inc();
metrics.dedup_duplicates_found_total.inc();

// Cache size
metrics.set_dedup_cache_size(1000);

// Cache eviction
metrics.dedup_cache_evictions_total.inc();
```

### Normalization

```rust
// Event normalized
metrics.normalization_events_total.inc();

// Fill operation
metrics.record_normalization_fill(StrategyLabel::Forward);
// StrategyLabel: Forward | Backward | Linear | Zero | Drop

// Gap filled
metrics.normalization_gaps_filled_total.inc();

// Outlier detected
metrics.normalization_outliers_detected_total.inc();
```

### Watermarks

```rust
// Watermark lag (seconds)
metrics.set_watermark_lag(2.5);

// Watermark update
metrics.record_watermark_update();

// Out-of-order event
metrics.record_out_of_order_event();
```

### Kafka

```rust
// Message consumed
metrics.kafka_messages_consumed_total
    .get_or_create(&vec![
        ("topic".to_string(), "events".to_string()),
        ("partition".to_string(), "0".to_string()),
    ])
    .inc();

// Message produced
metrics.kafka_messages_produced_total
    .get_or_create(&vec![
        ("topic".to_string(), "results".to_string()),
        ("partition".to_string(), "1".to_string()),
    ])
    .inc();

// Consumer lag
metrics.kafka_consumer_lag
    .get_or_create(&vec![
        ("topic".to_string(), "events".to_string()),
        ("partition".to_string(), "0".to_string()),
    ])
    .set(1000);

// Offset commit error
metrics.kafka_offset_commit_errors_total
    .get_or_create(&vec![
        ("topic".to_string(), "events".to_string()),
        ("partition".to_string(), "0".to_string()),
    ])
    .inc();
```

### Pipeline

```rust
// Pipeline error
metrics.pipeline_errors_total
    .get_or_create(&vec![("error_type".to_string(), "timeout".to_string())])
    .inc();

// Active operators
metrics.pipeline_operators_active.set(5);

// Operator duration
metrics.operator_duration_seconds
    .get_or_create(&vec![("operator".to_string(), "filter".to_string())])
    .observe(0.002);
```

## Label Enums

All available label enums for type-safe metric labeling:

```rust
// Result labels
ResultLabel::Success
ResultLabel::Error
ResultLabel::Dropped
ResultLabel::Filtered

// Operation labels
OperationLabel::Get
OperationLabel::Put
OperationLabel::Delete
OperationLabel::Scan
OperationLabel::BatchGet
OperationLabel::BatchPut

// Backend labels
BackendLabel::InMemory
BackendLabel::Sled
BackendLabel::Redis
BackendLabel::Postgres

// Cache layer labels
CacheLayerLabel::L1
CacheLayerLabel::L2
CacheLayerLabel::L3

// Strategy labels (normalization)
StrategyLabel::Forward
StrategyLabel::Backward
StrategyLabel::Linear
StrategyLabel::Zero
StrategyLabel::Drop

// Window type labels
WindowTypeLabel::Tumbling
WindowTypeLabel::Sliding
WindowTypeLabel::Session
```

## HTTP Endpoints

```bash
# Metrics endpoint
curl http://localhost:9090/metrics

# Health check
curl http://localhost:9090/health

# Readiness probe
curl http://localhost:9090/ready
```

## Common Patterns

### Recording Event with Duration

```rust
use std::time::Instant;

let start = Instant::now();

// Process event
let result = process_event(&event).await;

// Record metrics
metrics.record_event_received();
let duration = start.elapsed().as_secs_f64();

match result {
    Ok(_) => {
        metrics.record_event_processing_duration(duration, ResultLabel::Success);
        metrics.record_event_processed(ResultLabel::Success);
    }
    Err(_) => {
        metrics.record_event_processing_duration(duration, ResultLabel::Error);
        metrics.record_event_processed(ResultLabel::Error);
    }
}
```

### State Operation with Metrics

```rust
async fn get_state(key: &str, metrics: &ProcessorMetrics) -> Result<Value> {
    let start = Instant::now();

    // Try L1 cache
    if let Some(value) = l1_cache.get(key) {
        metrics.record_state_cache_hit(CacheLayerLabel::L1);
        return Ok(value);
    }
    metrics.record_state_cache_miss(CacheLayerLabel::L1);

    // Try L2 cache
    if let Some(value) = l2_cache.get(key).await? {
        metrics.record_state_cache_hit(CacheLayerLabel::L2);
        return Ok(value);
    }
    metrics.record_state_cache_miss(CacheLayerLabel::L2);

    // Fetch from backend
    let value = backend.get(key).await?;
    let duration = start.elapsed().as_secs_f64();

    metrics.record_state_operation(
        OperationLabel::Get,
        BackendLabel::Redis,
        duration
    );

    Ok(value)
}
```

### Window Processing with Metrics

```rust
async fn process_window(window: Window, metrics: &ProcessorMetrics) {
    let window_type = WindowTypeLabel::Tumbling;

    metrics.record_window_triggered(window_type);
    metrics.record_window_size(window_type, window.len() as f64);

    let start = Instant::now();

    // Process window
    let result = aggregate_window(&window).await;

    let duration = start.elapsed().as_secs_f64();

    match result {
        Ok(_) => {
            metrics.aggregations_computed_total
                .get_or_create(&vec![("aggregate_type".to_string(), "sum".to_string())])
                .inc();
            metrics.aggregation_duration_seconds
                .get_or_create(&vec![("aggregate_type".to_string(), "sum".to_string())])
                .observe(duration);
        }
        Err(_) => {
            metrics.aggregation_errors_total
                .get_or_create(&vec![("aggregate_type".to_string(), "sum".to_string())])
                .inc();
        }
    }
}
```

### Deduplication with Metrics

```rust
async fn check_duplicate(event: &Event, metrics: &ProcessorMetrics) -> bool {
    let event_id = event.id();
    let is_duplicate = dedup_cache.contains(&event_id);

    metrics.record_dedup_check(is_duplicate);

    if !is_duplicate {
        dedup_cache.insert(event_id);
        metrics.set_dedup_cache_size(dedup_cache.len() as i64);
    }

    is_duplicate
}
```

## Prometheus Queries

Common PromQL queries for monitoring:

```promql
# Event processing rate (events/sec)
rate(processor_events_received[5m])

# Success rate
rate(processor_events_processed{result="success"}[5m])
  / rate(processor_events_received[5m])

# P95 latency
histogram_quantile(0.95,
  rate(processor_event_processing_duration_seconds_bucket[5m]))

# P99 latency
histogram_quantile(0.99,
  rate(processor_event_processing_duration_seconds_bucket[5m]))

# Cache hit rate
rate(processor_state_cache_hits{layer="l1"}[5m])
  / (rate(processor_state_cache_hits{layer="l1"}[5m])
     + rate(processor_state_cache_misses{layer="l1"}[5m]))

# Deduplication rate
rate(processor_dedup_duplicates_found[5m])
  / rate(processor_dedup_events_checked[5m])

# Active windows by type
processor_windows_active

# Window trigger rate
rate(processor_windows_triggered[5m])

# Average window size
rate(processor_window_size_events_sum[5m])
  / rate(processor_window_size_events_count[5m])

# Watermark lag
processor_watermark_lag_seconds

# Pipeline lag
processor_pipeline_lag_seconds

# State size by backend
processor_state_size_bytes

# Kafka consumer lag
processor_kafka_consumer_lag

# Error rate by type
rate(processor_pipeline_errors[5m])
```

## Grafana Panels

### Event Processing Overview

```yaml
Title: "Event Processing Rate"
Query: rate(processor_events_received[5m])
Type: Graph
Unit: events/s
```

### Success Rate

```yaml
Title: "Success Rate"
Query: |
  rate(processor_events_processed{result="success"}[5m])
    / rate(processor_events_received[5m]) * 100
Type: Gauge
Unit: percent (0-100)
Thresholds:
  - Red: < 95
  - Yellow: < 99
  - Green: >= 99
```

### Latency Percentiles

```yaml
Title: "Processing Latency"
Queries:
  - P50: histogram_quantile(0.50, rate(processor_event_processing_duration_seconds_bucket[5m]))
  - P95: histogram_quantile(0.95, rate(processor_event_processing_duration_seconds_bucket[5m]))
  - P99: histogram_quantile(0.99, rate(processor_event_processing_duration_seconds_bucket[5m]))
Type: Graph
Unit: seconds
```

## Testing

```rust
#[tokio::test]
async fn test_metrics() {
    let registry = MetricsRegistry::new();
    let mut reg = registry.registry().write();
    let metrics = ProcessorMetrics::new(&mut reg);
    drop(reg);

    // Record some metrics
    metrics.record_event_received();
    metrics.record_event_processed(ResultLabel::Success);

    // Verify
    assert_eq!(metrics.events_received_total.get(), 1);
}
```

## Performance Tips

1. **Use convenience methods**: `record_event_processed()` is cleaner than manual label creation
2. **Reuse label vectors**: Store frequently-used label vectors as constants
3. **Batch observations**: Record multiple observations before encoding
4. **Monitor cardinality**: Keep unique label combinations under 10,000
5. **Use appropriate types**: Counters for totals, Gauges for current values, Histograms for distributions

## Common Mistakes

❌ **Don't**: Create unbounded label values
```rust
// BAD - creates unlimited time series
metrics.events_processed_total
    .get_or_create(&vec![("user_id".to_string(), user_id)])
    .inc();
```

✅ **Do**: Use bounded label sets
```rust
// GOOD - limited set of values
metrics.record_event_processed(ResultLabel::Success);
```

❌ **Don't**: Hold registry write lock
```rust
// BAD - blocks other threads
let reg = registry.registry().write();
// ... long operation ...
```

✅ **Do**: Release lock quickly
```rust
// GOOD - acquire, use, release
{
    let mut reg = registry.registry().write();
    ProcessorMetrics::new(&mut reg);
} // lock released here
```

## See Also

- [Full README](./README.md)
- [Prometheus Best Practices](https://prometheus.io/docs/practices/naming/)
- [Example: metrics_example.rs](../../examples/metrics_example.rs)
