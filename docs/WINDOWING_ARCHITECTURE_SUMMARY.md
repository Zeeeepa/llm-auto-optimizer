# Windowing & Aggregation Architecture - Executive Summary

**Quick Reference for Implementation Teams**

---

## Key Design Decisions

### 1. Window Types Support

| Type | Implementation | Use Case | Memory Impact |
|------|---------------|----------|---------------|
| **Tumbling** | `TumblingWindowManager` | Fixed intervals (5min, 1hr) | Low - 1 window per key |
| **Sliding** | `SlidingWindowManager` | Moving averages, trends | Medium - N windows per event |
| **Session** | `SessionWindowManager` | User activity, bursts | Variable - merges windows |

### 2. State Management

**Primary Backend:** Redis
- Connection pooling (10-100 connections)
- TTL: 2 hours default
- Batch operations for efficiency
- Atomic updates via Lua scripts

**State Schema:**
```
window:{type}:{key}:{window_id} -> WindowState
checkpoint:{id} -> Checkpoint
watermark:{partition} -> Watermark
```

### 3. Aggregation Pipeline

**Standard Aggregators:**
- Count, Sum, Average, Min, Max
- **Percentiles (p50, p95, p99)** - HdrHistogram for bounded memory
- Standard Deviation

**Performance:**
- Aggregator update: <10ms
- Memory per window: ~2KB (with HdrHistogram)
- Supports distributed merging

### 4. Checkpointing Strategy

**Configuration:**
- Interval: 5 minutes
- Retention: 10 checkpoints
- Distributed coordination via Redis locks
- Automatic cleanup of old checkpoints

**Content:**
- All window states
- Watermarks per partition
- Kafka consumer offsets
- Aggregator accumulators

### 5. Latency Budget (Target: <100ms p99)

```
Operation                  Budget    Strategy
─────────────────────────────────────────────────────
State read (Redis)         20ms      Connection pooling
State write (Redis)        20ms      Batch operations
Aggregation                10ms      HdrHistogram
Window assignment          5ms       In-memory
Serialization              10ms      Bincode + compression
Network (Kafka)            25ms      Pipelining
Buffer                     10ms      Safety margin
─────────────────────────────────────────────────────
Total                      100ms
```

---

## Critical Implementation Details

### Window Assignment Examples

**Tumbling (5-minute windows):**
```
Event at 12:03:45 → Window [12:00:00, 12:05:00)
Event at 12:07:23 → Window [12:05:00, 12:10:00)
```

**Sliding (10-min window, 1-min slide):**
```
Event at 12:05:30 → 6 windows:
  [12:00:00, 12:10:00)
  [12:01:00, 12:11:00)
  [12:02:00, 12:12:00)
  [12:03:00, 12:13:00)
  [12:04:00, 12:14:00)
  [12:05:00, 12:15:00)
```

**Session (5-second gap):**
```
Events: t=0s, t=2s, t=10s
Result: Two sessions:
  [0s, 7s)     ← merged from t=0s and t=2s
  [10s, 15s)   ← separate session (gap > 5s)
```

### State Structure

```rust
WindowState {
  window: Window,
  key: String,
  created_at: DateTime<Utc>,
  updated_at: DateTime<Utc>,
  event_count: u64,
  aggregators: HashMap<String, Vec<u8>>,  // Serialized
  status: WindowStatus,
  min_event_time: Option<DateTime<Utc>>,
  max_event_time: Option<DateTime<Utc>>,
}
```

### Event Processing Pipeline

```
1. Consume from Kafka
2. Deserialize event
3. Update watermark
4. Check if late (drop if beyond allowed lateness)
5. Assign to windows (tumbling/sliding/session)
6. For each window:
   - Load state from Redis
   - Update aggregators
   - Save state to Redis
7. Check triggers (watermark-based)
8. Finalize triggered windows:
   - Compute final metrics
   - Emit to Kafka
   - Mark for cleanup
9. Record metrics
10. Commit Kafka offset
```

---

## Redis Configuration

```rust
RedisConfig {
  urls: ["redis://localhost:6379"],
  key_prefix: "llm-optimizer:state:",
  default_ttl: Some(Duration::from_secs(7200)),  // 2 hours
  min_connections: 10,
  max_connections: 100,
  connect_timeout: Duration::from_secs(5),
  read_timeout: Duration::from_secs(3),
  write_timeout: Duration::from_secs(3),
  max_retries: 3,
  pipeline_batch_size: 100,
}
```

---

## Performance Optimizations

### 1. Batch Operations
```rust
// Instead of individual puts
for (key, value) in data {
  backend.put(key, value).await?;  // ❌ Slow
}

// Use batch operations
backend.batch_put(&data).await?;     // ✅ Fast (10x improvement)
```

### 2. Connection Pooling
```rust
// Reuse connections across operations
let pool = RedisConnectionPool::new(config).await?;
let conn = pool.get_connection();  // Round-robin
```

### 3. HdrHistogram for Percentiles
```rust
// ❌ Exact percentile (O(n) memory, O(n log n) computation)
let mut values = Vec::new();
for v in events {
  values.push(v);
}
values.sort();
let p99 = values[values.len() * 99 / 100];

// ✅ HdrHistogram (O(1) memory, O(1) update, O(1) query)
let mut hist = Histogram::new_with_bounds(1, 1_000_000, 2)?;
for v in events {
  hist.record(v)?;
}
let p99 = hist.value_at_quantile(0.99);
```

### 4. Async Parallel Updates
```rust
// Update multiple windows in parallel
let futures: Vec<_> = windows.iter()
  .map(|w| executor.update_window(w, key, value))
  .collect();
futures::future::join_all(futures).await;
```

---

## Error Handling

### Recovery Strategy

**On Startup:**
1. Find latest checkpoint
2. Validate checkpoint integrity
3. Restore all window states
4. Restore watermarks
5. Restore Kafka offsets
6. Resume processing

**On State Corruption:**
1. Log error with details
2. Delete corrupted state
3. Emit alert/metric
4. Continue processing (graceful degradation)

**On Redis Failure:**
1. Circuit breaker opens after 3 failures
2. Reject new requests
3. Attempt recovery after 60s timeout
4. Half-open state for testing
5. Close circuit on success

### Late Event Handling

```rust
// Configuration
LateEventConfig {
  allowed_lateness: Duration::from_secs(60),  // Accept up to 1min late
  log_late_events: true,
  emit_metrics: true,
}

// Processing
if event_time < watermark {
  let lateness = watermark - event_time;
  if lateness > allowed_lateness {
    // Drop event
    metrics.inc_dropped_late_events();
    return;
  }
  // Accept late event
  metrics.inc_accepted_late_events();
  // ... process normally
}
```

---

## Monitoring Metrics

### Essential Metrics

```rust
// Throughput
events_per_second: Gauge
events_processed_total: Counter

// Latency
processing_latency_p50: Histogram
processing_latency_p95: Histogram
processing_latency_p99: Histogram

// Windows
active_windows: Gauge
triggered_windows_total: Counter
expired_windows_total: Counter

// Watermarks
watermark_lag_seconds: Gauge
late_events_total: Counter
dropped_late_events_total: Counter

// State
state_size_bytes: Gauge
redis_operations_total: Counter
redis_errors_total: Counter

// Checkpoints
checkpoint_duration_seconds: Histogram
checkpoint_size_bytes: Gauge
checkpoint_failures_total: Counter
```

### Alert Conditions

```yaml
alerts:
  - name: HighProcessingLatency
    condition: processing_latency_p99 > 100ms
    severity: warning

  - name: HighWatermarkLag
    condition: watermark_lag_seconds > 60
    severity: warning

  - name: RedisConnectionFailure
    condition: redis_errors_total > 10 in 1m
    severity: critical

  - name: CheckpointFailure
    condition: checkpoint_failures_total > 0
    severity: critical

  - name: HighLateEventRate
    condition: dropped_late_events_total / events_processed_total > 0.01
    severity: warning
```

---

## Testing Strategy

### Unit Tests
- Window assignment correctness
- Aggregator accuracy
- State serialization/deserialization
- Watermark progression

### Integration Tests
- End-to-end pipeline with Kafka
- Redis state persistence
- Checkpoint creation and recovery
- Late event handling

### Performance Tests
- Throughput: >10,000 events/sec
- Latency: p99 < 100ms
- Memory: <500MB per instance
- State size: <10GB total

### Chaos Tests
- Redis connection failures
- Kafka broker failures
- Process crashes during checkpoint
- Corrupted state recovery
- Network partitions

---

## Implementation Phases

### Phase 1: Window Management (Weeks 1-2)
- [ ] Implement `TumblingWindowManager`
- [ ] Implement `SlidingWindowManager`
- [ ] Implement `SessionWindowManager`
- [ ] Window state serialization
- [ ] Unit tests (100% coverage)

### Phase 2: Aggregation (Weeks 2-3)
- [ ] Standard aggregators (count, sum, avg, min, max)
- [ ] HdrHistogram percentile aggregator
- [ ] `AggregatorRegistry`
- [ ] `WindowAggregationExecutor`
- [ ] Aggregator merging tests

### Phase 3: State & Checkpointing (Weeks 3-4)
- [ ] Redis state backend integration
- [ ] Batch operations
- [ ] `DistributedCheckpointCoordinator`
- [ ] Recovery logic
- [ ] Checkpoint/recovery tests

### Phase 4: Integration (Weeks 4-5)
- [ ] Complete event processing pipeline
- [ ] Kafka source/sink integration
- [ ] End-to-end integration tests
- [ ] Performance benchmarks
- [ ] Chaos testing

---

## Quick Start Code

### Basic Setup

```rust
use processor::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Configure Redis
    let redis_config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("optimizer:state:")
        .default_ttl(Duration::from_secs(7200))
        .pool_size(10, 100)
        .build()?;

    let redis_backend = Arc::new(RedisStateBackend::new(redis_config).await?);

    // 2. Create window managers
    let tumbling_mgr = TumblingWindowManager::new(Duration::minutes(5));
    let sliding_mgr = SlidingWindowManager::new(
        Duration::minutes(10),
        Duration::minutes(1),
    );
    let session_mgr = SessionWindowManager::new(Duration::seconds(30));

    // 3. Create watermark generator
    let watermark_gen = Arc::new(RwLock::new(
        BoundedOutOfOrdernessWatermark::new(
            Duration::from_secs(10),
            Some(Duration::from_secs(60)),
        )
    ));

    // 4. Create checkpoint coordinator
    let checkpoint_coord = CheckpointCoordinator::new(
        redis_backend.clone(),
        Duration::from_secs(300),
        CheckpointOptions::default(),
    );

    // 5. Start checkpoint coordinator
    checkpoint_coord.start().await?;

    // 6. Create stream processor
    let processor = StreamProcessor::new(
        tumbling_mgr,
        sliding_mgr,
        session_mgr,
        watermark_gen,
        checkpoint_coord,
        redis_backend,
    );

    // 7. Run processor
    processor.run().await?;

    Ok(())
}
```

### Processing a Single Event

```rust
// Process event through pipeline
let event = ProcessorEvent::new(
    feedback_event,
    event_time,
    key.to_string(),
);

// Assign to windows
let tumbling_window = tumbling_mgr.assign_window(event.event_time);
let sliding_windows = sliding_mgr.assign_windows(event.event_time);
let session_window = session_mgr.process_event(
    event.key.clone(),
    event.event_time,
).await?;

// Update aggregations
let value = 123.45; // Extract from event
for window in &sliding_windows {
    aggregation_executor.update_window(window, &event.key, value).await?;
}

// Check triggers
let watermark = watermark_gen.read().await.current_watermark();
if watermark.is_after(tumbling_window.bounds.end.timestamp_millis()) {
    // Trigger window
    let result = aggregation_executor.finalize_window(
        &tumbling_window,
        &event.key,
    ).await?;

    // Emit result
    kafka_sink.send(result).await?;
}
```

---

## FAQ

**Q: Why HdrHistogram instead of exact percentiles?**
A: HdrHistogram provides:
- O(1) memory (bounded at ~2KB) vs O(n) for exact
- O(1) update time vs O(n log n) sort
- Acceptable accuracy (±1% relative error)
- Supports distributed merging

**Q: How to handle late events?**
A: Configure `allowed_lateness` (default 60s). Events within this bound are accepted and processed. Events beyond this are logged and dropped.

**Q: What happens if Redis goes down?**
A: Circuit breaker activates after 3 failures, processor enters degraded mode. On recovery, restore from latest checkpoint.

**Q: How to scale horizontally?**
A: Use Kafka partitioning. Each processor instance handles a subset of partitions. State is partitioned by key in Redis.

**Q: What's the memory footprint?**
A: Per active window:
- Base state: ~500 bytes
- Aggregators: ~2KB (with HdrHistogram)
- Total: ~2.5KB per window

For 10,000 active windows: ~25MB

**Q: How long does checkpoint take?**
A: Typically 1-5 seconds depending on state size. Non-blocking, processing continues during checkpoint.

**Q: Can windows span multiple time zones?**
A: All times are in UTC internally. Display/reporting can convert to local time zones.

---

## References

- Full Architecture: [WINDOWING_AGGREGATION_ARCHITECTURE.md](./WINDOWING_AGGREGATION_ARCHITECTURE.md)
- Implementation Plan: [stream-processor-implementation-plan.md](./stream-processor-implementation-plan.md)
- API Reference: [stream-processor-api-reference.md](./stream-processor-api-reference.md)
- HdrHistogram: https://docs.rs/hdrhistogram/latest/hdrhistogram/

---

**Version:** 1.0
**Last Updated:** 2025-11-10
**Status:** Ready for Implementation
