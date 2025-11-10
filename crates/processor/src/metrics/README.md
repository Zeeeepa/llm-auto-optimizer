# Prometheus Metrics for Stream Processor

This module provides comprehensive Prometheus metrics export for the Stream Processor, enabling production-ready monitoring and observability.

## Architecture

The metrics system is organized into several modules:

- **`mod.rs`**: Main module with error types and exports
- **`labels.rs`**: Type-safe label management with enums for common label values
- **`registry.rs`**: Global singleton registry for thread-safe metric registration
- **`prometheus.rs`**: Core metrics definitions (30+ metrics)
- **`server.rs`**: HTTP server exposing `/metrics`, `/health`, and `/ready` endpoints

## Features

- **30+ Comprehensive Metrics**: Covering all aspects of stream processing
- **Type-Safe Labels**: Strongly-typed label enums prevent typos and ensure consistency
- **Thread-Safe**: Lock-free counters using `AtomicU64` for high performance
- **Efficient Histograms**: Exponential bucketing for duration and size distributions
- **HTTP Server**: Built-in Axum-based server with health and readiness probes
- **Prometheus Convention**: All metrics follow official naming conventions

## Metrics Categories

### Stream Processing (5 metrics)

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `processor_events_received` | Counter | - | Total events received |
| `processor_events_processed` | Counter | `result` | Events processed by result (success/error/dropped/filtered) |
| `processor_event_processing_duration_seconds` | Histogram | `result` | Event processing duration |
| `processor_pipeline_lag_seconds` | Gauge | - | Current pipeline lag |
| `processor_backpressure_events` | Gauge | - | Events waiting due to backpressure |

### Windows (5 metrics)

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `processor_windows_created` | Counter | `window_type` | Windows created by type (tumbling/sliding/session) |
| `processor_windows_triggered` | Counter | `window_type` | Windows triggered by type |
| `processor_windows_active` | Gauge | `window_type` | Currently active windows |
| `processor_window_size_events` | Histogram | `window_type` | Distribution of window sizes |
| `processor_late_events` | Counter | - | Late events arriving after watermark |

### Aggregations (3 metrics)

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `processor_aggregations_computed` | Counter | `aggregate_type` | Aggregations computed by type |
| `processor_aggregation_duration_seconds` | Histogram | `aggregate_type` | Aggregation computation duration |
| `processor_aggregation_errors` | Counter | `aggregate_type` | Aggregation errors by type |

### State Backend (6 metrics)

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `processor_state_operations` | Counter | `operation`, `backend` | State operations (get/put/delete/scan) |
| `processor_state_operation_duration_seconds` | Histogram | `operation`, `backend` | State operation duration |
| `processor_state_cache_hits` | Counter | `layer` | Cache hits by layer (L1/L2/L3) |
| `processor_state_cache_misses` | Counter | `layer` | Cache misses by layer |
| `processor_state_size_bytes` | Gauge | `backend` | Current state size in bytes |
| `processor_state_evictions` | Counter | `backend` | State evictions by backend |

### Deduplication (4 metrics)

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `processor_dedup_events_checked` | Counter | - | Events checked for deduplication |
| `processor_dedup_duplicates_found` | Counter | - | Duplicate events found |
| `processor_dedup_cache_size` | Gauge | - | Current deduplication cache size |
| `processor_dedup_cache_evictions` | Counter | - | Deduplication cache evictions |

### Normalization (4 metrics)

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `processor_normalization_events` | Counter | - | Events normalized |
| `processor_normalization_fill_operations` | Counter | `strategy` | Fill operations by strategy (forward/backward/linear/zero/drop) |
| `processor_normalization_gaps_filled` | Counter | - | Gaps filled in time series |
| `processor_normalization_outliers_detected` | Counter | - | Outliers detected |

### Watermarks (3 metrics)

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `processor_watermark_lag_seconds` | Gauge | - | Current watermark lag |
| `processor_watermark_updates` | Counter | - | Watermark updates |
| `processor_out_of_order_events` | Counter | - | Out-of-order events |

### Kafka (4 metrics)

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `processor_kafka_messages_consumed` | Counter | `topic`, `partition` | Messages consumed from Kafka |
| `processor_kafka_messages_produced` | Counter | `topic`, `partition` | Messages produced to Kafka |
| `processor_kafka_consumer_lag` | Gauge | `topic`, `partition` | Consumer lag in messages |
| `processor_kafka_offset_commit_errors` | Counter | `topic`, `partition` | Offset commit errors |

### Pipeline (3 metrics)

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `processor_pipeline_errors` | Counter | `error_type` | Pipeline errors by type |
| `processor_pipeline_operators_active` | Gauge | - | Active pipeline operators |
| `processor_operator_duration_seconds` | Histogram | `operator` | Operator execution duration |

## Usage

### Basic Setup

```rust
use processor::metrics::{
    MetricsRegistry, ProcessorMetrics, MetricsServer, MetricsServerConfig,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize metrics with global registry
    let registry = MetricsRegistry::global();
    let mut reg = registry.registry().write();
    let metrics = Arc::new(ProcessorMetrics::new(&mut reg));
    drop(reg);

    // Start HTTP server
    let config = MetricsServerConfig::new("0.0.0.0", 9090);
    let server = MetricsServer::with_registry(config, registry);

    tokio::spawn(async move {
        server.start().await.expect("Failed to start metrics server");
    });

    // Use metrics in your application
    metrics.record_event_received();
    metrics.record_event_processed(ResultLabel::Success);

    Ok(())
}
```

### Recording Metrics

#### Event Processing

```rust
use processor::metrics::labels::ResultLabel;

// Record event received
metrics.record_event_received();

// Record event processed with result
metrics.record_event_processed(ResultLabel::Success);

// Record processing duration (in seconds)
metrics.record_event_processing_duration(0.001, ResultLabel::Success);

// Update pipeline lag
metrics.set_pipeline_lag(1.5);

// Update backpressure
metrics.set_backpressure_events(100);
```

#### Window Operations

```rust
use processor::metrics::labels::WindowTypeLabel;

// Record window creation
metrics.record_window_created(WindowTypeLabel::Tumbling);

// Record window trigger
metrics.record_window_triggered(WindowTypeLabel::Tumbling);

// Update active windows count
metrics.set_windows_active(WindowTypeLabel::Tumbling, 5);

// Record window size
metrics.record_window_size(WindowTypeLabel::Tumbling, 100.0);

// Record late event
metrics.record_late_event();
```

#### State Operations

```rust
use processor::metrics::labels::{OperationLabel, BackendLabel, CacheLayerLabel};

// Record state operation with duration
metrics.record_state_operation(
    OperationLabel::Get,
    BackendLabel::Redis,
    0.001, // duration in seconds
);

// Record cache hits/misses
metrics.record_state_cache_hit(CacheLayerLabel::L1);
metrics.record_state_cache_miss(CacheLayerLabel::L2);

// Update state size
metrics.set_state_size_bytes(BackendLabel::Redis, 1024 * 1024);
```

#### Deduplication

```rust
// Record deduplication check
let is_duplicate = check_if_duplicate(&event);
metrics.record_dedup_check(is_duplicate);

// Update cache size
metrics.set_dedup_cache_size(1000);
```

#### Normalization

```rust
use processor::metrics::labels::StrategyLabel;

// Record normalization event
metrics.normalization_events_total.inc();

// Record fill operation
metrics.record_normalization_fill(StrategyLabel::Forward);

// Record gap filled
metrics.normalization_gaps_filled_total.inc();
```

#### Watermarks

```rust
// Update watermark lag
metrics.set_watermark_lag(2.5);

// Record watermark update
metrics.record_watermark_update();

// Record out-of-order event
metrics.record_out_of_order_event();
```

### HTTP Endpoints

Once the metrics server is running, the following endpoints are available:

#### `/metrics` - Prometheus Metrics

Returns metrics in Prometheus text exposition format:

```bash
curl http://localhost:9090/metrics
```

Example output:
```
# HELP processor_events_received Total number of events received by the processor
# TYPE processor_events_received counter
processor_events_received 1000

# HELP processor_events_processed Total number of events processed by result
# TYPE processor_events_processed counter
processor_events_processed{result="success"} 950
processor_events_processed{result="error"} 50

# HELP processor_event_processing_duration_seconds Duration of event processing
# TYPE processor_event_processing_duration_seconds histogram
processor_event_processing_duration_seconds_bucket{result="success",le="0.001"} 100
processor_event_processing_duration_seconds_bucket{result="success",le="0.002"} 250
...
```

#### `/health` - Health Check

Returns health status in JSON format:

```bash
curl http://localhost:9090/health
```

Response:
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "uptime_seconds": 3600
}
```

#### `/ready` - Readiness Probe

Returns readiness status (useful for Kubernetes):

```bash
curl http://localhost:9090/ready
```

Response (200 OK):
```json
{
  "ready": true,
  "checks": [
    {
      "name": "processor",
      "ready": true,
      "message": null
    }
  ]
}
```

### Configuration

#### Metrics Server Config

```rust
use processor::metrics::MetricsServerConfig;

// Default configuration (0.0.0.0:9090)
let config = MetricsServerConfig::default();

// Custom configuration
let config = MetricsServerConfig::new("127.0.0.1", 8080);
```

### Integration with Prometheus

Add the following job to your `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'stream-processor'
    static_configs:
      - targets: ['localhost:9090']
    scrape_interval: 15s
    scrape_timeout: 10s
```

### Grafana Dashboard

Key queries for monitoring:

```promql
# Event processing rate
rate(processor_events_received[5m])

# Event processing success rate
rate(processor_events_processed{result="success"}[5m])
  / rate(processor_events_received[5m])

# P95 event processing latency
histogram_quantile(0.95,
  rate(processor_event_processing_duration_seconds_bucket[5m]))

# Active windows by type
processor_windows_active

# State cache hit rate
rate(processor_state_cache_hits[5m])
  / (rate(processor_state_cache_hits[5m])
     + rate(processor_state_cache_misses[5m]))

# Deduplication rate
rate(processor_dedup_duplicates_found[5m])
  / rate(processor_dedup_events_checked[5m])

# Watermark lag
processor_watermark_lag_seconds
```

## Performance Considerations

### Lock-Free Counters

All counters use `AtomicU64` for lock-free increments, ensuring minimal performance overhead:

```rust
// This is lock-free and very fast
metrics.record_event_received(); // ~10ns on modern hardware
```

### Histogram Bucketing

Histograms use exponential bucketing for efficient memory usage:

- **Duration buckets**: 1ms to 30s (16 buckets)
- **Size buckets**: 1 to 10,000 events (14 buckets)

### Memory Usage

Approximate memory per metric:
- Counter: 8 bytes
- Gauge: 8 bytes
- Histogram: ~200 bytes (depends on bucket count)

Total metrics memory: ~10 KB (excluding label cardinality)

### Label Cardinality

**Important**: Be careful with label cardinality. Each unique label combination creates a new time series.

✅ Good (low cardinality):
```rust
metrics.record_event_processed(ResultLabel::Success); // 4 possible values
```

❌ Bad (high cardinality):
```rust
// DON'T DO THIS - creates unbounded label values
metrics.events_processed_total
    .get_or_create(&vec![("user_id".to_string(), user_id)])
    .inc();
```

## Testing

### Unit Tests

Run unit tests for individual components:

```bash
cargo test --lib metrics
```

### Integration Tests

Run integration tests:

```bash
cargo test --test metrics_integration_test
```

### Example

Run the example to see metrics in action:

```bash
cargo run --example metrics_example
```

Then visit:
- http://localhost:9090/metrics
- http://localhost:9090/health
- http://localhost:9090/ready

## Thread Safety

All metrics are thread-safe and can be safely shared across async tasks:

```rust
let metrics = Arc::new(metrics);

for _ in 0..10 {
    let metrics_clone = Arc::clone(&metrics);
    tokio::spawn(async move {
        metrics_clone.record_event_received();
    });
}
```

## Best Practices

1. **Initialize Once**: Create `ProcessorMetrics` once at application startup
2. **Share Safely**: Use `Arc<ProcessorMetrics>` to share across tasks
3. **Label Consistency**: Use the provided label enums to ensure consistency
4. **Avoid High Cardinality**: Don't use unbounded label values (user IDs, request IDs, etc.)
5. **Monitor Costs**: Each metric has a small memory cost; monitor total memory usage
6. **Use Convenience Methods**: Prefer `record_event_processed()` over raw counter access
7. **Test Metrics**: Include metrics verification in integration tests

## Troubleshooting

### Metrics Not Appearing

1. Check that the metrics server is running:
   ```bash
   curl http://localhost:9090/health
   ```

2. Verify metrics are being recorded:
   ```rust
   println!("Events received: {}", metrics.events_received_total.get());
   ```

3. Check Prometheus scrape configuration

### High Memory Usage

1. Check label cardinality:
   ```bash
   curl http://localhost:9090/metrics | grep -c "processor_"
   ```

2. Review custom labels for unbounded values

3. Consider aggregation before recording

### Server Won't Start

1. Check port availability:
   ```bash
   lsof -i :9090
   ```

2. Try a different port:
   ```rust
   let config = MetricsServerConfig::new("0.0.0.0", 9091);
   ```

3. Check bind address (use "0.0.0.0" for all interfaces)

## See Also

- [Prometheus Best Practices](https://prometheus.io/docs/practices/naming/)
- [Prometheus Client Rust](https://github.com/prometheus/client_rust)
- [OpenTelemetry Metrics](https://opentelemetry.io/docs/specs/otel/metrics/)
