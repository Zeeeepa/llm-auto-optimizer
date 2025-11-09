# Stream Processing Windowing - Quick Reference

> Companion guide to [STREAM_PROCESSING_WINDOWING_SPECIFICATION.md](./STREAM_PROCESSING_WINDOWING_SPECIFICATION.md)

## TL;DR - Recommendations for LLM Auto-Optimizer

### Window Types by Event Category

| Event Type | Window Type | Size | Rationale |
|------------|-------------|------|-----------|
| **Performance Metrics** | Tumbling | 5 minutes | Fixed intervals for trend analysis |
| **User Feedback** | Session | 30min gap, 4hr max | Capture complete user journeys |
| **Anomaly Detection** | Sliding | 15min size, 1min slide | Smooth detection of degradation |

### Watermark Strategy

**Primary**: Bounded Out-of-Order Watermark

```rust
// Configuration per event source
max_out_of_order_delay:
  - Observatory (metrics): 2 minutes
  - User (feedback): 10 minutes
  - System (health): 10 seconds
```

**Why**: Balances completeness with latency for distributed LLM systems.

### State Storage

**3-Tier Architecture**:
1. **Hot** (In-memory): Current windows, 1 hour TTL, 512MB
2. **Warm** (Sled DB): Recent windows, 7 day TTL, 1GB cache
3. **Cold** (PostgreSQL): Historical data, 90 day retention

### Trigger Policy

**Performance Metrics**:
```yaml
primary: on_watermark          # Emit when window complete
early_firing: every 30 seconds # Emit preliminary results
allowed_lateness: 2 minutes    # Accept late data
```

**Critical Anomalies**:
```yaml
primary: on_critical_event  # Immediate emission
fallback: every 5 seconds   # Regular updates
```

### Memory Budget

```yaml
total: 4GB
- 50% hot state (in-memory windows)
- 30% warm cache (Sled)
- 10% network buffers (Kafka)
- 10% overhead
```

## Event Time Extraction

```rust
use llm_optimizer_types::events::FeedbackEvent;

// All events have timestamp field
let event_time = feedback_event.timestamp();

// Use for windowing
let window_key = (event_time.timestamp() / 300) * 300;  // 5-min tumbling
```

## Window Assignment Examples

### Tumbling Window (5 minutes)

```rust
use chrono::{DateTime, Duration, Utc};

fn assign_tumbling_window(
    event_time: DateTime<Utc>,
    size: Duration,
) -> (DateTime<Utc>, DateTime<Utc>) {
    let window_start_ts = (event_time.timestamp() / size.num_seconds()) * size.num_seconds();
    let window_start = DateTime::from_timestamp(window_start_ts, 0).unwrap();
    let window_end = window_start + size;
    (window_start, window_end)
}

// Example
let event_time = Utc::now();
let (start, end) = assign_tumbling_window(event_time, Duration::minutes(5));
// Event at 14:37:22 -> Window [14:35:00, 14:40:00)
```

### Session Window (30-minute gap)

```rust
struct SessionWindow {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    gap: Duration,
}

impl SessionWindow {
    fn new(first_event: DateTime<Utc>, gap: Duration) -> Self {
        Self {
            start: first_event,
            end: first_event,
            gap,
        }
    }

    fn add_event(&mut self, event_time: DateTime<Utc>) -> bool {
        if event_time.signed_duration_since(self.end) <= self.gap {
            // Extend session
            self.end = event_time;
            true
        } else {
            // Session expired
            false
        }
    }

    fn is_expired(&self, current_time: DateTime<Utc>) -> bool {
        current_time.signed_duration_since(self.end) > self.gap
    }
}
```

## Watermark Calculation

### Simple Watermark (for single-partition)

```rust
use chrono::{DateTime, Duration, Utc};

struct SimpleWatermark {
    max_delay: Duration,
    last_event_time: DateTime<Utc>,
}

impl SimpleWatermark {
    fn on_event(&mut self, event_time: DateTime<Utc>) -> DateTime<Utc> {
        self.last_event_time = self.last_event_time.max(event_time);
        self.last_event_time - self.max_delay
    }
}

// Example
let mut watermark = SimpleWatermark {
    max_delay: Duration::minutes(2),
    last_event_time: Utc::now(),
};

// Event arrives with timestamp 14:35:00
let wm = watermark.on_event(
    DateTime::from_timestamp(1699887300, 0).unwrap()
);
// Watermark = 14:33:00 (2 minutes behind)
```

### Multi-Partition Watermark

```rust
use std::collections::HashMap;

struct PartitionedWatermark {
    partition_times: HashMap<u32, DateTime<Utc>>,
    max_delay: Duration,
}

impl PartitionedWatermark {
    fn on_event(&mut self, partition: u32, event_time: DateTime<Utc>) -> DateTime<Utc> {
        self.partition_times
            .entry(partition)
            .and_modify(|t| *t = (*t).max(event_time))
            .or_insert(event_time);

        // Watermark = min across all partitions - delay
        let min_time = self.partition_times.values().min().unwrap();
        *min_time - self.max_delay
    }
}
```

## State Store Pattern

```rust
use std::collections::HashMap;
use sled::Db;

struct WindowStateStore {
    // Hot: in-memory
    hot: HashMap<WindowId, WindowState>,

    // Warm: embedded DB
    warm: Db,
}

impl WindowStateStore {
    async fn get(&self, window_id: &WindowId) -> Option<WindowState> {
        // L1: check hot cache
        if let Some(state) = self.hot.get(window_id) {
            return Some(state.clone());
        }

        // L2: check warm storage
        if let Ok(Some(bytes)) = self.warm.get(window_id.as_bytes()) {
            let state: WindowState = bincode::deserialize(&bytes).ok()?;
            return Some(state);
        }

        None
    }

    async fn put(&mut self, window_id: WindowId, state: WindowState) {
        // Always write to hot
        self.hot.insert(window_id.clone(), state.clone());

        // Async flush to warm (background)
        let bytes = bincode::serialize(&state).unwrap();
        self.warm.insert(window_id.as_bytes(), bytes).ok();
    }
}
```

## Aggregation Pattern for LLM Metrics

```rust
use llm_optimizer_types::metrics::PerformanceMetrics;

#[derive(Default)]
struct WindowAggregator {
    latencies: Vec<f64>,
    costs: Vec<f64>,
    errors: u64,
    count: u64,
}

impl WindowAggregator {
    fn add_event(&mut self, event: &PerformanceMetricsEvent) {
        self.latencies.push(event.latency_ms);
        self.costs.push(event.cost);
        if event.error.is_some() {
            self.errors += 1;
        }
        self.count += 1;
    }

    fn compute(&self) -> PerformanceMetrics {
        use statrs::statistics::{Data, OrderStatistics, Statistics};

        let mut lat_data = Data::new(self.latencies.clone());

        PerformanceMetrics {
            avg_latency_ms: lat_data.mean().unwrap_or(0.0),
            p50_latency_ms: lat_data.percentile(50).unwrap_or(0.0),
            p95_latency_ms: lat_data.percentile(95).unwrap_or(0.0),
            p99_latency_ms: lat_data.percentile(99).unwrap_or(0.0),
            error_rate: self.errors as f64 / self.count as f64,
            throughput_qps: self.count as f64 / 300.0,  // 5-min window
            total_requests: self.count,
            period_start: Utc::now(),  // from window
            period_end: Utc::now(),
        }
    }
}
```

## Configuration Snippets

### Development (Low Latency)

```yaml
windowing:
  performance_metrics:
    type: tumbling
    size: 60s           # 1-minute windows
    allowed_lateness: 10s

  watermarks:
    max_out_of_order_delay: 5s
    emit_interval: 500ms

  state:
    hot:
      max_size_mb: 256
    warm:
      disabled: true    # In-memory only for dev

  checkpointing:
    interval_seconds: 30
```

### Production (High Throughput)

```yaml
windowing:
  performance_metrics:
    type: tumbling
    size: 300s          # 5-minute windows
    allowed_lateness: 120s

  watermarks:
    max_out_of_order_delay: 120s
    idle_timeout: 300s
    emit_interval: 1s

  state:
    hot:
      max_size_mb: 2048
    warm:
      backend: sled
      cache_size_mb: 4096
    cold:
      backend: postgres

  checkpointing:
    interval_seconds: 60
    format: binary
```

## Common Patterns

### Pattern 1: Late Data Handling

```rust
// Accept late data up to 2 minutes after watermark
if event_time > watermark - Duration::minutes(2) {
    // Add to window state
    window_state.add_event(event);

    // Re-emit updated result
    if should_emit_late_update() {
        emit_window_result(window_state, EmissionType::Late);
    }
} else {
    // Too late - send to dead letter queue
    dlq.send(event);
}
```

### Pattern 2: Early Firing

```rust
// Emit preliminary results every 30 seconds
if window_state.last_emission.elapsed() > Duration::seconds(30) {
    emit_window_result(window_state, EmissionType::Early);
    window_state.last_emission = Utc::now();
}

// Emit final result when watermark passes
if watermark >= window.end {
    emit_window_result(window_state, EmissionType::Final);
    purge_window(window_state);
}
```

### Pattern 3: Memory-Aware Pruning

```rust
// Check memory usage before adding to window
if memory_monitor.usage() > 0.8 * memory_limit {
    // Prune old windows
    for (id, state) in windows.iter() {
        if state.window.end < watermark {
            // Flush to warm storage
            warm_storage.put(id, state).await;
            windows.remove(id);
        }
    }
}
```

## Monitoring Metrics

### Key Metrics to Track

```yaml
# Window processing metrics
window_processing:
  - windows_created_total
  - windows_completed_total
  - windows_pruned_total
  - window_size_bytes (histogram)
  - window_event_count (histogram)

# Watermark metrics
watermarks:
  - watermark_lag_seconds (gauge per partition)
  - watermark_idle_partitions (gauge)
  - watermark_advancement_rate (counter)

# State metrics
state:
  - hot_state_size_bytes (gauge)
  - warm_state_size_bytes (gauge)
  - state_evictions_total (counter)
  - checkpoint_duration_seconds (histogram)

# Late data metrics
late_data:
  - late_events_total (counter)
  - dropped_events_total (counter)
  - allowed_lateness_violations (counter)
```

## Troubleshooting

### Issue: Watermark Not Advancing

**Symptoms**: Windows not closing, memory growing

**Diagnosis**:
```rust
// Check per-partition watermarks
for (partition, timestamp) in watermark.partition_times.iter() {
    println!("Partition {}: last event at {:?}", partition, timestamp);
}
```

**Solutions**:
1. Check for idle partitions (no events arriving)
2. Enable idle partition timeout
3. Verify event time extraction is correct

### Issue: High Memory Usage

**Symptoms**: OOM errors, slow processing

**Diagnosis**:
```rust
// Count active windows
let active_windows = windows.len();
let avg_window_size = total_memory / active_windows;
println!("Active windows: {}, avg size: {}KB", active_windows, avg_window_size / 1024);
```

**Solutions**:
1. Reduce allowed lateness
2. Enable more aggressive pruning
3. Increase checkpoint frequency
4. Enable warm state storage

### Issue: Too Many Late Events

**Symptoms**: High `late_events_total` metric

**Diagnosis**:
```rust
// Log late event distribution
if event_time < watermark {
    let lateness = watermark - event_time;
    metrics.record_lateness(lateness);
}
```

**Solutions**:
1. Increase `max_out_of_order_delay`
2. Increase `allowed_lateness`
3. Investigate upstream delays
4. Consider processing-time windows for less critical data

## Next Steps

1. Read full specification: [STREAM_PROCESSING_WINDOWING_SPECIFICATION.md](./STREAM_PROCESSING_WINDOWING_SPECIFICATION.md)
2. Review processor crate: `/workspaces/llm-auto-optimizer/crates/processor/`
3. Check implementation examples: [IMPLEMENTATION_EXAMPLES.md](./IMPLEMENTATION_EXAMPLES.md)
4. See roadmap: Phase 1 starts with core windowing implementation

---

**Last Updated**: 2025-11-09
