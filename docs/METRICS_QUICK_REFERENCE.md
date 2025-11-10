# Metrics Export - Quick Reference Guide

**Quick access guide for developers implementing metrics in the Stream Processor**

---

## Quick Start

### 1. Add Metrics to Your Component (30 seconds)

```rust
use processor::metrics::{MetricsRegistry, Counter, Histogram};

pub struct MyComponent {
    metrics: ComponentMetrics,
}

struct ComponentMetrics {
    requests_total: Counter,
    request_duration_ms: Histogram,
}

impl MyComponent {
    pub fn new(registry: &MetricsRegistry) -> Result<Self> {
        let metrics = ComponentMetrics {
            requests_total: registry
                .register_counter("mycomponent_requests_total", "Total requests")?
                .default(),

            request_duration_ms: registry
                .register_histogram(
                    "mycomponent_request_duration_ms",
                    "Request duration",
                    &buckets::LATENCY_MS,
                )?
                .default(),
        };

        Ok(Self { metrics })
    }

    pub async fn handle_request(&self) {
        let start = Instant::now();

        // Your logic here

        self.metrics.requests_total.inc();
        self.metrics.request_duration_ms.observe(start.elapsed().as_millis() as f64);
    }
}
```

### 2. Start Metrics Server (15 seconds)

```rust
use processor::metrics::{MetricsServer, ServerConfig};

#[tokio::main]
async fn main() -> Result<()> {
    let registry = Arc::new(MetricsRegistry::default());

    // Start metrics server on :9090
    let server = MetricsServer::new(registry.clone(), ServerConfig::default());
    tokio::spawn(async move {
        server.serve().await
    });

    // Access at http://localhost:9090/metrics
}
```

### 3. Add OpenTelemetry Tracing (30 seconds)

```rust
use processor::metrics::otel::{OtelProvider, SpanInstrumentation};

let otel = OtelProvider::init(OtelConfig {
    service_name: "my-service".to_string(),
    otlp_endpoint: "http://localhost:4317".to_string(),
    ..Default::default()
})?;

let tracer = otel.tracer("my_component");

// Create span
let span = tracer.span_builder("my_operation")
    .with_kind(SpanKind::Internal)
    .start(&tracer);

// RAII span guard (auto-ends on drop)
let _guard = SpanGuard::new(span);

// Your logic here
// Span automatically ends when _guard drops
```

---

## Metric Types Cheat Sheet

### Counter (Monotonic)

**Use for**: Event counts, totals, accumulating values

```rust
// Create
let counter = registry.register_counter("events_total", "Total events")?;

// With labels
let counter = registry.register_counter("requests_total", "Requests")?
    .get_or_create(&[("method", "GET"), ("status", "200")]);

// Use
counter.inc();           // Increment by 1
counter.inc_by(5);       // Increment by 5
let value = counter.get(); // Read current value

// Performance: ~2ns per increment (single thread)
```

### Gauge (Up/Down)

**Use for**: Current values, levels, in-flight counts

```rust
// Create
let gauge = registry.register_gauge("active_connections", "Active connections")?;

// Use
gauge.set(42.0);        // Set to value
gauge.inc();            // Increment by 1
gauge.dec();            // Decrement by 1
gauge.add(5.0);         // Add delta
gauge.sub(3.0);         // Subtract delta
let value = gauge.get(); // Read current value

// Performance: ~5ns per operation
```

### Histogram (Distribution)

**Use for**: Latencies, sizes, durations

```rust
// Create with standard buckets
let histogram = registry.register_histogram(
    "request_duration_ms",
    "Request duration",
    &buckets::LATENCY_MS,  // Pre-defined buckets
)?;

// Use
histogram.observe(123.45);

// Get snapshot
let snapshot = histogram.snapshot();
println!("Count: {}, Sum: {}", snapshot.count, snapshot.sum);

// Standard bucket configs:
// - buckets::LATENCY_MS (1ms - 10s)
// - buckets::SIZE_BYTES (1KB - 100MB)
// - buckets::THROUGHPUT (1 - 100K events/sec)

// Performance: ~15ns per observation
```

### Summary (Quantiles)

**Use for**: Precise percentiles when histograms are too coarse

```rust
// Create
let summary = registry.register_summary(
    "request_duration_ms",
    "Request duration",
    &[0.5, 0.9, 0.99],  // p50, p90, p99
)?;

// Use
summary.observe(123.45);

// Get quantiles
let quantiles = summary.quantiles();
for (q, value) in quantiles {
    println!("p{}: {}", q * 100.0, value);
}

// Performance: ~50ns per observation (has write lock)
```

---

## Naming Conventions

### Format
```
<namespace>_<subsystem>_<name>_<unit>_<suffix>
```

### Examples

```
# Counters (always _total suffix)
stream_processor_events_received_total
stream_processor_errors_total
kafka_messages_sent_total

# Gauges (no suffix)
stream_processor_active_windows
process_memory_bytes
kafka_consumer_lag

# Histograms (no suffix, but generates _bucket, _sum, _count)
stream_processor_processing_duration_ms
kafka_produce_latency_ms

# Units
_bytes          # Size in bytes
_seconds        # Duration in seconds
_ms             # Duration in milliseconds
_ratio          # Ratio (0.0-1.0)
_percent        # Percentage (0-100)
```

### Rules

1. Use `snake_case`
2. Include unit suffix
3. Counters MUST end with `_total`
4. Be descriptive but concise
5. Namespace first, then subsystem

---

## Label Best Practices

### Good Labels (Low Cardinality)

```rust
// Status values (3-5 values)
labels: &[("status", "success")]  // success, error, timeout

// HTTP methods (4-9 values)
labels: &[("method", "GET")]  // GET, POST, PUT, DELETE, ...

// Component names (5-20 values)
labels: &[("component", "processor")]

// Operations (5-15 values)
labels: &[("operation", "create")]  // create, read, update, delete

// Example: Good label combination
counter.get_or_create(&[
    ("method", "GET"),
    ("status", "200"),
    ("endpoint", "/api/events"),
]);
```

### Bad Labels (High Cardinality)

```rust
// AVOID: User IDs (unbounded)
labels: &[("user_id", "12345")]  // ❌

// AVOID: Session IDs (unbounded)
labels: &[("session_id", "abc-123")]  // ❌

// AVOID: Request IDs (unbounded)
labels: &[("request_id", "uuid-here")]  // ❌

// AVOID: Timestamps (infinite)
labels: &[("timestamp", "2025-01-01")]  // ❌

// AVOID: IP addresses (unbounded)
labels: &[("ip", "192.168.1.1")]  // ❌
```

### Cardinality Calculation

```
Total Series = Product of all label value counts

Example:
- method: 5 values (GET, POST, PUT, DELETE, PATCH)
- status: 6 values (200, 201, 400, 404, 500, 503)
- endpoint: 20 values

Total = 5 × 6 × 20 = 600 series ✓ (acceptable)

With user_id (10,000 users):
Total = 5 × 6 × 20 × 10,000 = 6,000,000 series ❌ (too high!)
```

---

## OpenTelemetry Spans

### Basic Span

```rust
let tracer = otel_provider.tracer("my_component");

let span = tracer.span_builder("operation_name")
    .with_kind(SpanKind::Internal)
    .with_attributes(vec![
        KeyValue::new("key", "value"),
    ])
    .start(&tracer);

// Do work

span.end();
```

### RAII Span Guard (Recommended)

```rust
let span = tracer.span_builder("operation").start(&tracer);
let _guard = SpanGuard::new(span);

// Span automatically ends when _guard drops
// Duration automatically recorded
```

### Span with Error Handling

```rust
let span = tracer.span_builder("operation").start(&tracer);
let guard = SpanGuard::new(span);

match do_work().await {
    Ok(result) => {
        guard.record_event("work_completed", vec![
            KeyValue::new("result_size", result.len() as i64),
        ]);
    }
    Err(e) => {
        guard.record_error(&e);
    }
}
```

### Context Propagation (Kafka)

```rust
// Producer: Inject context
let span = tracer.span_builder("kafka_produce").start(&tracer);
let mut message = KafkaMessage::new(topic, payload);

propagate_trace_context(&mut message, &span);
producer.send(message).await?;

// Consumer: Extract context
let message = consumer.recv().await?;
let parent_context = extract_trace_context(&message);

let span = tracer.span_builder("kafka_consume")
    .with_parent_context(parent_context)
    .start(&tracer);
```

---

## Configuration Examples

### Minimal (Development)

```yaml
observability:
  service:
    name: "my-service"
    environment: "dev"

  prometheus:
    enabled: true
    endpoint:
      port: 9090

  opentelemetry:
    enabled: false
```

### Production

```yaml
observability:
  service:
    name: "llm-optimizer-processor"
    version: "1.0.0"
    environment: "production"

  prometheus:
    enabled: true
    endpoint:
      host: "0.0.0.0"
      port: 9090
    registry:
      max_cardinality: 10000
      max_labels_per_metric: 10

  opentelemetry:
    enabled: true
    otlp:
      endpoint: "http://otel-collector:4317"
      protocol: "grpc"
    tracing:
      sampling:
        strategy: "parent_based"
        ratio: 0.1  # 10% sampling
      batch:
        max_queue_size: 2048
        max_export_batch_size: 512
```

---

## Common Patterns

### Pattern 1: Measure Function Duration

```rust
pub async fn my_function(&self) -> Result<()> {
    let start = Instant::now();

    // Your logic
    let result = do_work().await?;

    // Record duration
    self.metrics.duration_ms.observe(start.elapsed().as_millis() as f64);

    Ok(result)
}
```

### Pattern 2: Track In-Flight Requests

```rust
pub async fn handle_request(&self) {
    self.metrics.in_flight_requests.inc();
    let _guard = scopeguard::guard((), |_| {
        self.metrics.in_flight_requests.dec();
    });

    // Process request
    // Gauge automatically decrements on return or panic
}
```

### Pattern 3: Error Counting with Types

```rust
pub async fn process(&self) -> Result<()> {
    match do_work().await {
        Ok(_) => {
            self.metrics.requests_total
                .get_or_create(&[("status", "success")])
                .inc();
        }
        Err(e) => {
            let error_type = match e {
                Error::Timeout => "timeout",
                Error::Validation(_) => "validation",
                Error::Connection(_) => "connection",
                _ => "unknown",
            };

            self.metrics.errors_total
                .get_or_create(&[("error_type", error_type)])
                .inc();

            return Err(e);
        }
    }

    Ok(())
}
```

### Pattern 4: Batch Processing Metrics

```rust
pub async fn process_batch(&self, events: Vec<Event>) {
    let batch_size = events.len() as u64;
    let start = Instant::now();

    // Record batch size
    self.metrics.batch_size.observe(batch_size as f64);

    // Process
    let mut processed = 0;
    let mut errors = 0;

    for event in events {
        match self.process_event(event).await {
            Ok(_) => processed += 1,
            Err(_) => errors += 1,
        }
    }

    // Record results
    self.metrics.events_processed_total.inc_by(processed);
    self.metrics.events_failed_total.inc_by(errors);
    self.metrics.batch_duration_ms.observe(
        start.elapsed().as_millis() as f64
    );
}
```

---

## Prometheus Queries

### Basic Queries

```promql
# Current value of gauge
stream_processor_active_windows

# Rate of counter (events/sec)
rate(stream_processor_events_received_total[5m])

# Rate by label
rate(stream_processor_events_received_total{event_type="feedback"}[5m])

# Sum across labels
sum(rate(stream_processor_events_received_total[5m])) by (event_type)
```

### Histogram Queries

```promql
# Average latency
rate(stream_processor_processing_duration_ms_sum[5m]) /
rate(stream_processor_processing_duration_ms_count[5m])

# p95 latency
histogram_quantile(0.95,
  rate(stream_processor_processing_duration_ms_bucket[5m]))

# p99 latency by label
histogram_quantile(0.99,
  rate(stream_processor_processing_duration_ms_bucket[5m])) by (event_type)
```

### SLI Queries

```promql
# Error rate
rate(stream_processor_errors_total[5m]) /
rate(stream_processor_events_received_total[5m])

# Availability (1 - error_rate)
1 - (
  rate(stream_processor_errors_total[5m]) /
  rate(stream_processor_events_received_total[5m])
)

# Throughput
rate(stream_processor_events_processed_total[5m])
```

---

## Performance Tips

### 1. Reuse Metric Instances

```rust
// ✓ GOOD: Reuse counter instance
struct MyMetrics {
    requests: Counter,
}

impl MyMetrics {
    pub fn record_request(&self) {
        self.requests.inc();  // Fast: ~2ns
    }
}

// ✗ BAD: Look up every time
impl MyMetrics {
    pub fn record_request(&self, registry: &Registry) {
        let counter = registry.get_counter("requests_total").unwrap();
        counter.inc();  // Slow: hash lookup every time
    }
}
```

### 2. Pre-compute Label Sets

```rust
// ✓ GOOD: Pre-compute label sets
struct ComponentMetrics {
    success_counter: Counter,
    error_counter: Counter,
}

impl ComponentMetrics {
    pub fn new(registry: &Registry) -> Self {
        let family = registry.register_counter("requests_total", "Requests").unwrap();

        Self {
            success_counter: family.get_or_create(&[("status", "success")]),
            error_counter: family.get_or_create(&[("status", "error")]),
        }
    }
}

// ✗ BAD: Create labels every time
impl ComponentMetrics {
    pub fn record(&self, success: bool) {
        let labels = if success {
            &[("status", "success")]
        } else {
            &[("status", "error")]
        };
        self.family.get_or_create(labels).inc();  // Allocates every time
    }
}
```

### 3. Use Histograms for Latency

```rust
// ✓ GOOD: Histogram for latency
histogram.observe(duration_ms);

// ✗ BAD: Summary (slower due to lock)
summary.observe(duration_ms);

// Histograms are:
// - Lock-free
// - Faster (~15ns vs ~50ns)
// - More efficient for aggregation
// - Better for alerting
```

### 4. Batch Observations

```rust
// ✓ GOOD: Single observation per batch
let total_duration = events.iter()
    .map(|e| process(e))
    .sum();
histogram.observe(total_duration);

// ✗ AVOID: Observation per event (creates noise)
for event in events {
    let duration = process(event);
    histogram.observe(duration);  // Too granular
}
```

---

## Troubleshooting

### Issue: Cardinality Explosion

**Symptom**: Prometheus memory usage growing, slow queries

**Fix**:
```rust
// Check current cardinality
let cardinality = registry.cardinality.total_series();
println!("Current series: {}", cardinality);

// Breakdown by metric
let breakdown = registry.cardinality.series_by_metric();
for (metric, count) in breakdown {
    println!("{}: {} series", metric, count);
}

// Solution: Reduce label values
// BEFORE: user_id (10K values)
labels: &[("user_id", user_id)]

// AFTER: user_tier (3 values)
let tier = user.tier();  // "free", "pro", "enterprise"
labels: &[("user_tier", tier)]
```

### Issue: Slow /metrics Endpoint

**Symptom**: Prometheus scrapes timing out

**Fix**:
```rust
// Check render duration
let start = Instant::now();
let output = registry.render_prometheus();
println!("Render took: {:?}", start.elapsed());

// If >50ms:
// 1. Reduce cardinality
// 2. Enable metric filtering
// 3. Increase scrape interval
```

### Issue: Missing Metrics

**Symptom**: Metrics not appearing in Prometheus

**Debug**:
```bash
# Check /metrics endpoint
curl http://localhost:9090/metrics

# Verify metric registration
# Metrics must be registered before use

# Check Prometheus targets
# http://prometheus:9090/targets
```

---

## References

- Full Architecture: [METRICS_EXPORT_ARCHITECTURE.md](./METRICS_EXPORT_ARCHITECTURE.md)
- Prometheus Docs: https://prometheus.io/docs/
- OpenTelemetry Docs: https://opentelemetry.io/docs/
- Best Practices: https://prometheus.io/docs/practices/naming/

---

## Quick Checklist

Before committing metrics code:

- [ ] Metric names follow `<namespace>_<name>_<unit>_<suffix>` pattern
- [ ] Counters end with `_total`
- [ ] Labels have low cardinality (<100 values each)
- [ ] Histograms use appropriate buckets
- [ ] Help text is descriptive
- [ ] Metric instances are reused (not created in hot path)
- [ ] Added to relevant Grafana dashboard
- [ ] Alert rules defined for critical metrics
