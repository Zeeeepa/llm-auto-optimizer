# Stream Processor: Windowing and Aggregation Architecture Specification

**Version:** 1.0
**Date:** 2025-11-10
**Author:** Architecture Designer Agent
**Status:** Design Document

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Architecture Overview](#architecture-overview)
3. [Window Management System](#window-management-system)
4. [Aggregation Pipeline](#aggregation-pipeline)
5. [State Management & Persistence](#state-management--persistence)
6. [Checkpointing Mechanism](#checkpointing-mechanism)
7. [Redis Integration](#redis-integration)
8. [Event Processing Flow](#event-processing-flow)
9. [Error Handling & Recovery](#error-handling--recovery)
10. [Performance Considerations](#performance-considerations)
11. [Implementation Guide](#implementation-guide)

---

## Executive Summary

This document specifies the complete architecture for the Stream Processor's windowing and aggregation system. The design supports:

- **Three window types**: Tumbling, Sliding, and Session windows
- **Multiple time granularities**: 1min, 5min, 15min, 1hr
- **Sub-100ms latency** for real-time aggregation
- **Distributed state** with Redis persistence
- **Fault tolerance** through checkpointing
- **Horizontal scalability** via partitioning

### Key Requirements

| Requirement | Target | Status |
|-------------|--------|--------|
| Processing Latency (p99) | < 100ms | Achievable |
| Event Throughput | 10,000+/sec | Achievable |
| Window Types | Tumbling, Sliding, Session | Supported |
| State Persistence | Redis/PostgreSQL | Supported |
| Fault Tolerance | Checkpointing + Recovery | Supported |
| Late Event Handling | Configurable lateness | Supported |

---

## Architecture Overview

### High-Level Component Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                      Stream Processor Pipeline                      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐         │
│  │   Kafka      │───▶│   Event      │───▶│   Window     │         │
│  │   Source     │    │  Processor   │    │  Manager     │         │
│  │              │    │              │    │              │         │
│  └──────────────┘    └──────────────┘    └──────────────┘         │
│                           │                      │                 │
│                           ▼                      ▼                 │
│                    ┌──────────────┐    ┌──────────────┐           │
│                    │  Watermark   │    │ Aggregation  │           │
│                    │  Generator   │    │   Engine     │           │
│                    │              │    │              │           │
│                    └──────────────┘    └──────────────┘           │
│                           │                      │                 │
│                           ▼                      ▼                 │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐        │
│  │ Checkpoint   │◀───│    State     │◀───│   Window     │        │
│  │ Coordinator  │    │   Backend    │    │   Trigger    │        │
│  │              │    │   (Redis)    │    │              │        │
│  └──────────────┘    └──────────────┘    └──────────────┘        │
│                           │                                        │
│                           ▼                                        │
│                    ┌──────────────┐                                │
│                    │    Kafka     │                                │
│                    │     Sink     │                                │
│                    │              │                                │
│                    └──────────────┘                                │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Core Components

1. **Window Manager**: Assigns events to windows and manages window lifecycle
2. **Aggregation Engine**: Computes statistics (count, avg, p95, p99, stddev)
3. **State Backend**: Persists window state to Redis
4. **Watermark Generator**: Tracks event time progress
5. **Checkpoint Coordinator**: Creates periodic state snapshots
6. **Trigger System**: Determines when to emit window results

---

## Window Management System

### Window Types Implementation

#### 1. Tumbling Windows

Fixed-size, non-overlapping time windows.

```rust
/// Tumbling window implementation
pub struct TumblingWindowManager<K: Hash + Eq> {
    /// Window size (e.g., Duration::minutes(5))
    window_size: Duration,

    /// Active windows by key
    active_windows: DashMap<K, Window>,

    /// Window state backend
    state_backend: Arc<dyn StateBackend>,

    /// Watermark tracker
    watermark: Arc<RwLock<Watermark>>,
}

impl<K: Hash + Eq> TumblingWindowManager<K> {
    /// Assign event to its tumbling window
    pub fn assign_window(&self, event_time: DateTime<Utc>) -> Window {
        let window_start = self.align_to_window_boundary(event_time);
        let window_end = window_start + self.window_size;

        Window::new(WindowBounds::new(window_start, window_end))
    }

    /// Align timestamp to window boundary
    fn align_to_window_boundary(&self, timestamp: DateTime<Utc>) -> DateTime<Utc> {
        let millis = timestamp.timestamp_millis();
        let window_millis = self.window_size.num_milliseconds();
        let aligned = (millis / window_millis) * window_millis;

        DateTime::from_timestamp_millis(aligned).unwrap()
    }
}
```

**Window Assignment Logic:**
- Event at `t=12:03:45` with 5-minute windows → Window `[12:00:00, 12:05:00)`
- Event at `t=12:07:23` with 5-minute windows → Window `[12:05:00, 12:10:00)`

#### 2. Sliding Windows

Fixed-size, overlapping windows with configurable slide interval.

```rust
/// Sliding window implementation
pub struct SlidingWindowManager<K: Hash + Eq> {
    /// Window size (e.g., Duration::minutes(10))
    window_size: Duration,

    /// Slide interval (e.g., Duration::minutes(1))
    slide: Duration,

    /// Active windows per key (events belong to multiple windows)
    active_windows: DashMap<K, Vec<Window>>,

    state_backend: Arc<dyn StateBackend>,
}

impl<K: Hash + Eq> SlidingWindowManager<K> {
    /// Assign event to all overlapping sliding windows
    pub fn assign_windows(&self, event_time: DateTime<Utc>) -> Vec<Window> {
        let mut windows = Vec::new();

        // Calculate all windows this event belongs to
        let slide_millis = self.slide.num_milliseconds();
        let window_millis = self.window_size.num_milliseconds();
        let event_millis = event_time.timestamp_millis();

        // Find the first window that contains this event
        let first_window_start =
            ((event_millis - window_millis) / slide_millis + 1) * slide_millis;

        // Generate all windows containing this event
        let mut window_start = first_window_start;
        while window_start <= event_millis {
            let window_end = window_start + window_millis;

            if event_millis >= window_start && event_millis < window_end {
                let start_dt = DateTime::from_timestamp_millis(window_start).unwrap();
                let end_dt = DateTime::from_timestamp_millis(window_end).unwrap();
                windows.push(Window::new(WindowBounds::new(start_dt, end_dt)));
            }

            window_start += slide_millis;
        }

        windows
    }
}
```

**Example:**
- Window size: 10 minutes
- Slide: 1 minute
- Event at `12:05:30` belongs to windows:
  - `[12:00:00, 12:10:00)`
  - `[12:01:00, 12:11:00)`
  - `[12:02:00, 12:12:00)`
  - `[12:03:00, 12:13:00)`
  - `[12:04:00, 12:14:00)`
  - `[12:05:00, 12:15:00)`

#### 3. Session Windows

Variable-size windows based on inactivity gaps.

```rust
/// Session window implementation
pub struct SessionWindowManager<K: Hash + Eq + Clone> {
    /// Session gap timeout
    session_gap: Duration,

    /// Window merger for combining overlapping sessions
    mergers: DashMap<K, WindowMerger>,

    state_backend: Arc<dyn StateBackend>,
}

impl<K: Hash + Eq + Clone> SessionWindowManager<K> {
    /// Process event and merge with existing session windows
    pub async fn process_event(
        &self,
        key: K,
        event_time: DateTime<Utc>,
    ) -> StateResult<Window> {
        // Create initial window for this event
        let event_window = Window::new(WindowBounds::new(
            event_time,
            event_time + self.session_gap,
        ));

        // Get or create merger for this key
        let mut merger = self.mergers
            .entry(key.clone())
            .or_insert_with(|| WindowMerger::new(self.session_gap));

        // Add and merge with existing windows
        let (merged_window, merged_list) = merger.add_and_merge(event_window);

        // Update state backend
        if !merged_list.is_empty() {
            // Remove old windows
            for old_window in &merged_list {
                let state_key = self.build_state_key(&key, &old_window.id);
                self.state_backend.delete(&state_key).await?;
            }
        }

        Ok(merged_window)
    }

    /// Clean up expired sessions based on watermark
    pub async fn cleanup_expired_sessions(&self, watermark: Watermark) {
        let watermark_dt = watermark.to_datetime();

        for mut entry in self.mergers.iter_mut() {
            let removed = entry.value_mut().remove_old_windows(watermark_dt);

            // Clean up state for removed windows
            for window in removed {
                let state_key = self.build_state_key(entry.key(), &window.id);
                let _ = self.state_backend.delete(&state_key).await;
            }
        }
    }

    fn build_state_key(&self, key: &K, window_id: &str) -> Vec<u8> {
        format!("session:{}:{}",
            std::any::type_name::<K>(),
            window_id
        ).into_bytes()
    }
}
```

**Session Merging Example:**
```
Events:     t=0s   t=2s   t=10s   t=12s   t=25s
Gap: 5s

Sessions:   [────────────][────────]      [───]
            0s          7s 10s    17s      25s 30s

After merge at t=2s:  [─────────]
                      0s        7s
```

### Window State Structure

```rust
/// Complete state for a single window
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState<T> {
    /// Window metadata
    pub window: Window,

    /// Aggregation key
    pub key: String,

    /// Window creation time
    pub created_at: DateTime<Utc>,

    /// Last update time
    pub updated_at: DateTime<Utc>,

    /// Number of events in window
    pub event_count: u64,

    /// Aggregator states (serialized)
    pub aggregators: HashMap<String, Vec<u8>>,

    /// Window status
    pub status: WindowStatus,

    /// Earliest event time in window
    pub min_event_time: Option<DateTime<Utc>>,

    /// Latest event time in window
    pub max_event_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WindowStatus {
    /// Window is actively receiving events
    Active,

    /// Window is closed, waiting for watermark to trigger
    Closed,

    /// Window has been triggered and emitted
    Triggered,

    /// Window is expired and ready for cleanup
    Expired,
}
```

### Window Lifecycle Management

```rust
/// Window lifecycle coordinator
pub struct WindowLifecycleManager {
    /// Tumbling windows
    tumbling_mgr: TumblingWindowManager<String>,

    /// Sliding windows
    sliding_mgr: SlidingWindowManager<String>,

    /// Session windows
    session_mgr: SessionWindowManager<String>,

    /// Watermark generator
    watermark_gen: Arc<RwLock<Box<dyn WatermarkGenerator>>>,

    /// Window triggers
    triggers: HashMap<WindowType, Arc<dyn WindowTrigger>>,
}

impl WindowLifecycleManager {
    /// Process incoming event through all window types
    pub async fn process_event(
        &self,
        event: ProcessorEvent<FeedbackEvent>,
    ) -> StateResult<Vec<WindowAssignment>> {
        let mut assignments = Vec::new();

        // 1. Update watermark
        let mut watermark_gen = self.watermark_gen.write().await;
        let new_watermark = watermark_gen.on_event(
            event.event_time.timestamp_millis(),
            0, // partition
        );
        drop(watermark_gen);

        // 2. Check if event is late
        if let Some(wm) = new_watermark {
            if event.event_time < wm.to_datetime() {
                // Handle late event
                tracing::warn!(
                    event_time = %event.event_time,
                    watermark = %wm,
                    "Late event detected"
                );
            }
        }

        // 3. Assign to tumbling windows
        let tumbling_window = self.tumbling_mgr.assign_window(event.event_time);
        assignments.push(WindowAssignment {
            window: tumbling_window,
            window_type: WindowType::Tumbling,
        });

        // 4. Assign to sliding windows
        let sliding_windows = self.sliding_mgr.assign_windows(event.event_time);
        for window in sliding_windows {
            assignments.push(WindowAssignment {
                window,
                window_type: WindowType::Sliding,
            });
        }

        // 5. Process session window
        let session_window = self.session_mgr
            .process_event(event.key.clone(), event.event_time)
            .await?;
        assignments.push(WindowAssignment {
            window: session_window,
            window_type: WindowType::Session,
        });

        Ok(assignments)
    }

    /// Trigger windows based on watermark
    pub async fn trigger_windows(&self, watermark: Watermark) -> Vec<TriggeredWindow> {
        let mut triggered = Vec::new();

        // Check all active windows against triggers
        // Implementation depends on window type

        triggered
    }

    /// Clean up expired windows
    pub async fn cleanup_expired(&self, watermark: Watermark) {
        self.session_mgr.cleanup_expired_sessions(watermark).await;
        // Cleanup other window types...
    }
}

#[derive(Debug)]
pub struct WindowAssignment {
    pub window: Window,
    pub window_type: WindowType,
}

#[derive(Debug)]
pub struct TriggeredWindow {
    pub window: Window,
    pub window_type: WindowType,
    pub key: String,
    pub state: WindowState<Vec<u8>>,
}
```

---

## Aggregation Pipeline

### Aggregator Registry

```rust
/// Registry of all available aggregators for a window
pub struct AggregatorRegistry {
    /// Registered aggregators by name
    aggregators: HashMap<String, Box<dyn AggregatorBox>>,
}

/// Type-erased aggregator trait for storage
trait AggregatorBox: Send + Sync {
    fn update(&mut self, value: f64) -> anyhow::Result<()>;
    fn finalize(&self) -> anyhow::Result<serde_json::Value>;
    fn accumulator(&self) -> Vec<u8>;
    fn merge(&mut self, other: &[u8]) -> anyhow::Result<()>;
    fn reset(&mut self);
}

impl AggregatorRegistry {
    /// Create registry with standard aggregators
    pub fn new_standard() -> Self {
        let mut registry = Self {
            aggregators: HashMap::new(),
        };

        // Register standard aggregators
        registry.register("count", Box::new(CountAggregator::new()));
        registry.register("sum", Box::new(SumAggregator::new()));
        registry.register("avg", Box::new(AverageAggregator::new()));
        registry.register("min", Box::new(MinAggregator::new()));
        registry.register("max", Box::new(MaxAggregator::new()));
        registry.register("p50", Box::new(PercentileAggregator::new(0.5)));
        registry.register("p95", Box::new(PercentileAggregator::new(0.95)));
        registry.register("p99", Box::new(PercentileAggregator::new(0.99)));
        registry.register("stddev", Box::new(StandardDeviationAggregator::new()));

        registry
    }

    /// Update all aggregators with a value
    pub fn update_all(&mut self, value: f64) -> anyhow::Result<()> {
        for agg in self.aggregators.values_mut() {
            agg.update(value)?;
        }
        Ok(())
    }

    /// Get final results from all aggregators
    pub fn finalize_all(&self) -> anyhow::Result<HashMap<String, serde_json::Value>> {
        let mut results = HashMap::new();

        for (name, agg) in &self.aggregators {
            results.insert(name.clone(), agg.finalize()?);
        }

        Ok(results)
    }
}
```

### Percentile Aggregation (Critical for p95, p99)

For percentile calculation with bounded memory, use a T-Digest or HdrHistogram:

```rust
/// Percentile aggregator using T-Digest for streaming percentiles
pub struct PercentileAggregator {
    /// Target percentile (0.0 to 1.0)
    percentile: f64,

    /// Values buffer (for exact calculation or T-Digest)
    values: Vec<f64>,

    /// Maximum values to store before switching to approximate
    max_exact_values: usize,

    /// Whether we're in approximate mode
    approximate_mode: bool,
}

impl PercentileAggregator {
    pub fn new(percentile: f64) -> Self {
        Self {
            percentile,
            values: Vec::new(),
            max_exact_values: 10_000,
            approximate_mode: false,
        }
    }

    /// For production: Use T-Digest or HdrHistogram
    pub fn new_with_tdigest(percentile: f64) -> Self {
        // TODO: Integrate with tdigest crate
        // https://docs.rs/tdigest/latest/tdigest/
        unimplemented!("T-Digest integration")
    }
}

impl Aggregator for PercentileAggregator {
    type Input = f64;
    type Output = f64;
    type Accumulator = PercentileAccumulator;

    fn update(&mut self, value: f64) -> anyhow::Result<()> {
        if !self.approximate_mode && self.values.len() >= self.max_exact_values {
            tracing::warn!(
                "Switching to approximate percentile mode at {} values",
                self.max_exact_values
            );
            self.approximate_mode = true;

            // TODO: Build T-Digest from existing values
            // For now, keep top N values
            self.values.sort_by(|a, b| b.partial_cmp(a).unwrap());
            self.values.truncate(self.max_exact_values / 2);
        }

        if !self.approximate_mode {
            self.values.push(value);
        } else {
            // Reservoir sampling for approximate percentile
            let random: f64 = rand::random();
            if random < (self.max_exact_values as f64 / self.values.len() as f64) {
                let idx = (random * self.values.len() as f64) as usize;
                if idx < self.values.len() {
                    self.values[idx] = value;
                }
            }
        }

        Ok(())
    }

    fn finalize(&self) -> anyhow::Result<f64> {
        if self.values.is_empty() {
            return Ok(0.0);
        }

        let mut sorted = self.values.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let index = (sorted.len() as f64 * self.percentile) as usize;
        let index = index.min(sorted.len() - 1);

        Ok(sorted[index])
    }

    // ... other trait methods
}
```

**Alternative: HdrHistogram**

```rust
/// High-performance percentile aggregator using HdrHistogram
use hdrhistogram::Histogram;

pub struct HdrPercentileAggregator {
    percentile: f64,
    histogram: Histogram<u64>,
}

impl HdrPercentileAggregator {
    pub fn new(percentile: f64, max_value: u64, significant_figures: u8) -> Self {
        Self {
            percentile,
            histogram: Histogram::new_with_bounds(1, max_value, significant_figures)
                .expect("Invalid histogram configuration"),
        }
    }
}

impl Aggregator for HdrPercentileAggregator {
    type Input = f64;
    type Output = f64;
    type Accumulator = Vec<u8>; // Serialized histogram

    fn update(&mut self, value: f64) -> anyhow::Result<()> {
        let value_u64 = value as u64;
        self.histogram.record(value_u64)
            .map_err(|e| anyhow::anyhow!("Failed to record value: {}", e))?;
        Ok(())
    }

    fn finalize(&self) -> anyhow::Result<f64> {
        let percentile_value = self.histogram.value_at_quantile(self.percentile);
        Ok(percentile_value as f64)
    }

    fn accumulator(&self) -> Vec<u8> {
        // Serialize histogram for merging
        let mut buffer = Vec::new();
        self.histogram.serialize(&mut buffer)
            .expect("Serialization failed");
        buffer
    }

    fn merge(&mut self, other: Vec<u8>) -> anyhow::Result<()> {
        let other_hist = Histogram::<u64>::deserialize(&mut &other[..])
            .map_err(|e| anyhow::anyhow!("Failed to deserialize: {}", e))?;

        self.histogram.add(&other_hist)
            .map_err(|e| anyhow::anyhow!("Failed to merge: {}", e))?;

        Ok(())
    }

    fn reset(&mut self) {
        self.histogram.clear();
    }

    fn count(&self) -> u64 {
        self.histogram.len()
    }
}
```

### Window Aggregation Executor

```rust
/// Executes aggregation for a specific window
pub struct WindowAggregationExecutor {
    /// State backend for persistence
    state_backend: Arc<dyn StateBackend>,

    /// Aggregator registry
    aggregator_registry: AggregatorRegistry,
}

impl WindowAggregationExecutor {
    /// Update window with new event
    pub async fn update_window(
        &mut self,
        window: &Window,
        key: &str,
        value: f64,
    ) -> StateResult<()> {
        // 1. Load window state from backend
        let state_key = self.build_state_key(key, &window.id);
        let mut window_state = match self.state_backend.get(&state_key).await? {
            Some(data) => bincode::deserialize::<WindowState<Vec<u8>>>(&data)?,
            None => WindowState::new(window.clone(), key.to_string()),
        };

        // 2. Update aggregators
        for (name, agg_data) in &mut window_state.aggregators {
            // Deserialize aggregator
            let mut agg = self.deserialize_aggregator(name, agg_data)?;

            // Update with new value
            agg.update(value)?;

            // Serialize back
            *agg_data = bincode::serialize(&agg.accumulator())?;
        }

        // 3. Update metadata
        window_state.event_count += 1;
        window_state.updated_at = Utc::now();
        window_state.max_event_time = Some(
            window_state.max_event_time
                .unwrap_or(Utc::now())
                .max(Utc::now())
        );

        // 4. Save back to state
        let serialized = bincode::serialize(&window_state)?;
        self.state_backend.put(&state_key, &serialized).await?;

        Ok(())
    }

    /// Finalize window and emit results
    pub async fn finalize_window(
        &self,
        window: &Window,
        key: &str,
    ) -> StateResult<AggregationResult> {
        let state_key = self.build_state_key(key, &window.id);
        let data = self.state_backend.get(&state_key).await?
            .ok_or_else(|| StateError::KeyNotFound {
                key: String::from_utf8_lossy(&state_key).to_string(),
            })?;

        let window_state = bincode::deserialize::<WindowState<Vec<u8>>>(&data)?;

        // Deserialize and finalize all aggregators
        let mut results = HashMap::new();
        for (name, agg_data) in &window_state.aggregators {
            let agg = self.deserialize_aggregator(name, agg_data)?;
            results.insert(name.clone(), agg.finalize()?);
        }

        Ok(AggregationResult {
            window: window.clone(),
            key: key.to_string(),
            event_count: window_state.event_count,
            metrics: results,
            window_start: window.bounds.start,
            window_end: window.bounds.end,
            computed_at: Utc::now(),
        })
    }

    fn build_state_key(&self, key: &str, window_id: &str) -> Vec<u8> {
        format!("window:{}:{}", key, window_id).into_bytes()
    }

    fn deserialize_aggregator(
        &self,
        name: &str,
        data: &[u8],
    ) -> anyhow::Result<Box<dyn AggregatorBox>> {
        // Implementation depends on aggregator type
        // Could use a factory pattern or match on name
        unimplemented!("Aggregator deserialization")
    }
}

/// Result of window aggregation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregationResult {
    pub window: Window,
    pub key: String,
    pub event_count: u64,
    pub metrics: HashMap<String, serde_json::Value>,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub computed_at: DateTime<Utc>,
}
```

---

## State Management & Persistence

### State Key Schema

```
State keys follow a hierarchical structure:

window:{window_type}:{key}:{window_id}
checkpoint:{checkpoint_id}
watermark:{partition}
metadata:{key}

Examples:
- window:tumbling:model:claude-3-opus:1699564800000_1699565100000
- window:sliding:session:sess-123:1699564740000_1699565040000
- window:session:user:user-456:session_abc123
- checkpoint:checkpoint_1699565234567
- watermark:partition_0
- metadata:last_processed_offset
```

### State Serialization Format

```rust
/// Serialization wrapper for state data
#[derive(Debug, Serialize, Deserialize)]
pub struct SerializedState {
    /// Format version for compatibility
    pub version: u32,

    /// Timestamp when state was serialized
    pub serialized_at: DateTime<Utc>,

    /// Compressed state data (using LZ4 or Snappy)
    pub data: Vec<u8>,

    /// Checksum for integrity
    pub checksum: u64,
}

impl SerializedState {
    /// Serialize window state with compression
    pub fn serialize<T: Serialize>(data: &T) -> anyhow::Result<Vec<u8>> {
        // 1. Serialize to bincode
        let serialized = bincode::serialize(data)?;

        // 2. Compress (optional, for large states)
        let compressed = if serialized.len() > 1024 {
            lz4_flex::compress_prepend_size(&serialized)
        } else {
            serialized
        };

        // 3. Calculate checksum
        let checksum = crc32fast::hash(&compressed) as u64;

        // 4. Wrap in SerializedState
        let wrapper = SerializedState {
            version: 1,
            serialized_at: Utc::now(),
            data: compressed,
            checksum,
        };

        bincode::serialize(&wrapper)
    }

    /// Deserialize and validate state
    pub fn deserialize<T: for<'de> Deserialize<'de>>(
        bytes: &[u8]
    ) -> anyhow::Result<T> {
        // 1. Deserialize wrapper
        let wrapper: SerializedState = bincode::deserialize(bytes)?;

        // 2. Verify checksum
        let calculated = crc32_fast::hash(&wrapper.data) as u64;
        if calculated != wrapper.checksum {
            anyhow::bail!("State checksum mismatch");
        }

        // 3. Decompress if needed
        let decompressed = if wrapper.data.len() > 1024 {
            lz4_flex::decompress_size_prepended(&wrapper.data)?
        } else {
            wrapper.data
        };

        // 4. Deserialize actual data
        bincode::deserialize(&decompressed)
    }
}
```

### Redis State Backend Configuration

```rust
/// Production Redis configuration for state backend
pub fn create_production_redis_backend() -> anyhow::Result<RedisStateBackend> {
    let config = RedisConfig::builder()
        // Connection
        .url(std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://localhost:6379".to_string()))

        // Namespace isolation
        .key_prefix("llm-optimizer:state:")

        // TTL for automatic cleanup
        .default_ttl(Duration::from_secs(7200)) // 2 hours

        // Connection pooling
        .pool_size(10, 100)

        // Timeouts
        .timeouts(
            Duration::from_secs(5),  // connect
            Duration::from_secs(3),  // read
            Duration::from_secs(3),  // write
        )

        // Retry configuration
        .retries(
            3,                              // max attempts
            Duration::from_millis(100),     // base delay
            Duration::from_secs(5),         // max delay
        )

        // Enable cluster mode if multiple URLs
        .cluster_mode(false)

        // Enable TLS for production
        .tls(std::env::var("REDIS_TLS").is_ok())

        .build()?;

    RedisStateBackend::new(config).await
}
```

---

## Checkpointing Mechanism

### Checkpoint Strategy

```rust
/// Checkpoint configuration for production
pub struct CheckpointStrategy {
    /// Checkpoint interval (e.g., every 5 minutes)
    pub interval: Duration,

    /// Minimum interval between checkpoints (prevent thrashing)
    pub min_interval: Duration,

    /// Maximum number of checkpoints to retain
    pub retention_count: usize,

    /// Checkpoint directory
    pub checkpoint_dir: PathBuf,

    /// Whether to compress checkpoints
    pub compress: bool,

    /// Checkpoint on shutdown
    pub checkpoint_on_shutdown: bool,
}

impl Default for CheckpointStrategy {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(300), // 5 minutes
            min_interval: Duration::from_secs(60), // 1 minute
            retention_count: 10,
            checkpoint_dir: PathBuf::from("/var/lib/optimizer/checkpoints"),
            compress: true,
            checkpoint_on_shutdown: true,
        }
    }
}
```

### Distributed Checkpoint Coordination

For distributed deployments, ensure only one instance creates checkpoints:

```rust
/// Distributed checkpoint coordinator using Redis locks
pub struct DistributedCheckpointCoordinator {
    /// Local checkpoint coordinator
    coordinator: CheckpointCoordinator<RedisStateBackend>,

    /// Distributed lock for coordination
    lock: RedisDistributedLock,

    /// Lock TTL
    lock_ttl: Duration,
}

impl DistributedCheckpointCoordinator {
    pub async fn new(
        backend: Arc<RedisStateBackend>,
        strategy: CheckpointStrategy,
        redis_url: String,
    ) -> anyhow::Result<Self> {
        let coordinator = CheckpointCoordinator::new(
            backend,
            strategy.interval,
            CheckpointOptions {
                checkpoint_dir: strategy.checkpoint_dir,
                max_checkpoints: strategy.retention_count,
                compress: strategy.compress,
                min_interval: strategy.min_interval,
            },
        );

        let lock = RedisDistributedLock::new(&redis_url).await?;

        Ok(Self {
            coordinator,
            lock,
            lock_ttl: Duration::from_secs(60),
        })
    }

    /// Create checkpoint with distributed coordination
    pub async fn create_checkpoint_coordinated(&self) -> StateResult<Option<String>> {
        // Try to acquire lock
        match self.lock.try_acquire("checkpoint:global", self.lock_ttl).await {
            Ok(Some(guard)) => {
                tracing::info!("Acquired checkpoint lock, creating checkpoint");

                // Create checkpoint
                let checkpoint_id = self.coordinator.create_checkpoint().await?;

                // Lock automatically released when guard drops
                drop(guard);

                Ok(Some(checkpoint_id))
            }
            Ok(None) => {
                // Another instance is checkpointing
                tracing::debug!("Checkpoint lock held by another instance");
                Ok(None)
            }
            Err(e) => {
                tracing::error!("Failed to acquire checkpoint lock: {}", e);
                Err(StateError::LockFailed {
                    lock_name: "checkpoint:global".to_string(),
                    reason: e.to_string(),
                })
            }
        }
    }
}
```

### Checkpoint Content Structure

```rust
/// Complete checkpoint containing all state
#[derive(Debug, Serialize, Deserialize)]
pub struct StreamProcessorCheckpoint {
    /// Checkpoint metadata
    pub metadata: CheckpointMetadata,

    /// All window states
    pub windows: Vec<WindowStateSnapshot>,

    /// Current watermark per partition
    pub watermarks: HashMap<u32, Watermark>,

    /// Kafka consumer offsets
    pub kafka_offsets: HashMap<TopicPartition, i64>,

    /// Aggregator states
    pub aggregator_states: HashMap<String, Vec<u8>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WindowStateSnapshot {
    pub window_type: WindowType,
    pub window_id: String,
    pub key: String,
    pub state_data: Vec<u8>,
    pub event_count: u64,
    pub last_update: DateTime<Utc>,
}
```

---

## Redis Integration

### Redis Connection Management

```rust
/// Redis connection pool manager
pub struct RedisConnectionPool {
    /// Primary connection manager
    primary: ConnectionManager,

    /// Connection pool for parallel operations
    pool: Vec<ConnectionManager>,

    /// Pool size
    pool_size: usize,

    /// Round-robin index
    next_conn: Arc<AtomicUsize>,
}

impl RedisConnectionPool {
    pub async fn new(config: &RedisConfig) -> anyhow::Result<Self> {
        let client = Client::open(config.primary_url())?;

        // Create primary connection
        let primary = ConnectionManager::new(client.clone()).await?;

        // Create connection pool
        let mut pool = Vec::with_capacity(config.max_connections as usize);
        for _ in 0..config.max_connections {
            let conn = ConnectionManager::new(client.clone()).await?;
            pool.push(conn);
        }

        Ok(Self {
            primary,
            pool,
            pool_size: config.max_connections as usize,
            next_conn: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// Get next connection from pool (round-robin)
    pub fn get_connection(&self) -> &ConnectionManager {
        let idx = self.next_conn.fetch_add(1, Ordering::Relaxed) % self.pool_size;
        &self.pool[idx]
    }
}
```

### Batch Operations for Performance

```rust
impl RedisStateBackend {
    /// Batch get multiple keys (pipeline)
    pub async fn batch_get(&self, keys: &[&[u8]]) -> StateResult<Vec<Option<Vec<u8>>>> {
        let mut pipe = redis::pipe();

        for key in keys {
            let full_key = self.build_full_key(key);
            pipe.get(full_key);
        }

        let mut conn = self.pool.get_connection().clone();
        let results: Vec<Option<Vec<u8>>> = pipe
            .query_async(&mut conn)
            .await
            .map_err(|e| StateError::BackendError {
                backend: "Redis".to_string(),
                operation: "batch_get".to_string(),
                details: e.to_string(),
            })?;

        Ok(results)
    }

    /// Batch put multiple key-value pairs (pipeline)
    pub async fn batch_put(&self, kvs: &[(&[u8], &[u8])]) -> StateResult<()> {
        let mut pipe = redis::pipe();

        for (key, value) in kvs {
            let full_key = self.build_full_key(key);
            pipe.set(&full_key, *value);

            if let Some(ttl) = self.config.default_ttl {
                pipe.expire(&full_key, ttl.as_secs() as usize);
            }
        }

        let mut conn = self.pool.get_connection().clone();
        pipe.query_async(&mut conn)
            .await
            .map_err(|e| StateError::BackendError {
                backend: "Redis".to_string(),
                operation: "batch_put".to_string(),
                details: e.to_string(),
            })?;

        Ok(())
    }

    /// Atomic update using Lua script
    pub async fn atomic_update_window(
        &self,
        key: &[u8],
        update_fn: impl FnOnce(&mut WindowState<Vec<u8>>),
    ) -> StateResult<()> {
        // Lua script for atomic read-modify-write
        let script = Script::new(r"
            local key = KEYS[1]
            local value = redis.call('GET', key)
            if value == false then
                return nil
            end
            -- Application updates the value via argument
            local new_value = ARGV[1]
            redis.call('SET', key, new_value)
            return 'OK'
        ");

        // Load current state
        let full_key = self.build_full_key(key);
        let current = self.get(key).await?
            .ok_or_else(|| StateError::KeyNotFound {
                key: String::from_utf8_lossy(key).to_string(),
            })?;

        let mut state: WindowState<Vec<u8>> = bincode::deserialize(&current)?;

        // Apply update
        update_fn(&mut state);

        // Serialize
        let new_value = bincode::serialize(&state)?;

        // Execute atomic update
        let mut conn = self.pool.get_connection().clone();
        script
            .key(&full_key)
            .arg(&new_value)
            .invoke_async::<_, ()>(&mut conn)
            .await
            .map_err(|e| StateError::BackendError {
                backend: "Redis".to_string(),
                operation: "atomic_update".to_string(),
                details: e.to_string(),
            })?;

        Ok(())
    }
}
```

---

## Event Processing Flow

### Complete Event Processing Pipeline

```rust
/// Main stream processing pipeline
pub struct StreamProcessor {
    /// Kafka event source
    kafka_source: KafkaSource,

    /// Window lifecycle manager
    window_manager: Arc<WindowLifecycleManager>,

    /// Aggregation executor
    aggregation_executor: Arc<WindowAggregationExecutor>,

    /// Watermark generator
    watermark_gen: Arc<RwLock<BoundedOutOfOrdernessWatermark>>,

    /// Checkpoint coordinator
    checkpoint_coord: Arc<DistributedCheckpointCoordinator>,

    /// Kafka sink for results
    kafka_sink: KafkaSink<AggregationResult>,

    /// Metrics collector
    metrics: Arc<ProcessorMetrics>,
}

impl StreamProcessor {
    /// Main processing loop
    pub async fn run(&mut self) -> anyhow::Result<()> {
        tracing::info!("Starting stream processor");

        // Start checkpoint coordinator
        self.checkpoint_coord.coordinator.start().await?;

        // Start watermark periodic check
        let watermark_gen = Arc::clone(&self.watermark_gen);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                let mut gen = watermark_gen.write().await;
                if let Some(wm) = gen.on_periodic_check() {
                    tracing::debug!("Periodic watermark: {}", wm);
                }
            }
        });

        // Main event processing loop
        loop {
            // 1. Consume event from Kafka
            let message = self.kafka_source.recv().await?;
            let start_time = std::time::Instant::now();

            // 2. Deserialize event
            let event: ProcessorEvent<FeedbackEvent> =
                serde_json::from_slice(&message.payload)?;

            // 3. Process event through windows
            let assignments = self.window_manager
                .process_event(event.clone())
                .await?;

            // 4. Update aggregations for each window
            for assignment in assignments {
                // Extract numeric value from event
                let value = self.extract_metric_value(&event.event)?;

                // Update window aggregation
                self.aggregation_executor
                    .update_window(
                        &assignment.window,
                        &event.key,
                        value,
                    )
                    .await?;
            }

            // 5. Check for triggered windows
            let watermark = self.watermark_gen.read().await.current_watermark();
            let triggered = self.window_manager
                .trigger_windows(watermark)
                .await;

            // 6. Finalize and emit triggered windows
            for triggered_window in triggered {
                let result = self.aggregation_executor
                    .finalize_window(
                        &triggered_window.window,
                        &triggered_window.key,
                    )
                    .await?;

                // Emit to Kafka
                self.kafka_sink.send(result).await?;

                // Clean up window state
                // (Keep for a grace period for late events)
            }

            // 7. Record metrics
            let processing_time = start_time.elapsed();
            self.metrics.record_processing_latency(processing_time);
            self.metrics.inc_events_processed();

            // 8. Commit Kafka offset
            message.commit().await?;
        }
    }

    fn extract_metric_value(&self, event: &FeedbackEvent) -> anyhow::Result<f64> {
        // Extract numeric value from event
        // Example: latency from metadata
        if let Some(latency) = event.metadata.get("latency") {
            Ok(latency.parse::<f64>()?)
        } else {
            Ok(1.0) // Default for count-based metrics
        }
    }
}
```

### Flowchart

```
┌─────────────────────────────────────────────────────────────────┐
│                    Event Processing Flow                        │
└─────────────────────────────────────────────────────────────────┘

                              START
                                │
                                ▼
                    ┌───────────────────────┐
                    │   Consume from Kafka  │
                    └───────────────────────┘
                                │
                                ▼
                    ┌───────────────────────┐
                    │   Deserialize Event   │
                    └───────────────────────┘
                                │
                                ▼
                    ┌───────────────────────┐
                    │   Update Watermark    │
                    └───────────────────────┘
                                │
                                ▼
                    ┌───────────────────────┐
                    │    Check if Late?     │
                    └───────────────────────┘
                         │          │
                    Yes  │          │  No
                         ▼          │
                ┌────────────┐      │
                │ Log/Drop?  │      │
                └────────────┘      │
                                    ▼
                        ┌───────────────────────┐
                        │   Assign to Windows   │
                        │  (Tumbling/Sliding/   │
                        │      Session)         │
                        └───────────────────────┘
                                    │
                                    ▼
                        ┌───────────────────────┐
                        │  For Each Window:     │
                        │  1. Load State        │
                        │  2. Update Aggregators│
                        │  3. Save State        │
                        └───────────────────────┘
                                    │
                                    ▼
                        ┌───────────────────────┐
                        │   Check Triggers      │
                        │  (Watermark-based)    │
                        └───────────────────────┘
                                    │
                         ┌──────────┴──────────┐
                         │                     │
                    Triggered               Not Triggered
                         │                     │
                         ▼                     │
                ┌─────────────────┐            │
                │ Finalize Window │            │
                │ Compute Results │            │
                └─────────────────┘            │
                         │                     │
                         ▼                     │
                ┌─────────────────┐            │
                │  Emit to Kafka  │            │
                └─────────────────┘            │
                         │                     │
                         ▼                     │
                ┌─────────────────┐            │
                │  Clean Up State │            │
                └─────────────────┘            │
                         │                     │
                         └──────────┬──────────┘
                                    │
                                    ▼
                        ┌───────────────────────┐
                        │   Record Metrics      │
                        │   Commit Offset       │
                        └───────────────────────┘
                                    │
                                    ▼
                                  END
                            (Loop back to START)
```

---

## Error Handling & Recovery

### Error Types

```rust
/// Stream processor specific errors
#[derive(Debug, thiserror::Error)]
pub enum ProcessorError {
    #[error("Window error: {0}")]
    Window(#[from] WindowError),

    #[error("State error: {0}")]
    State(#[from] StateError),

    #[error("Aggregation error: {0}")]
    Aggregation(#[from] AggregationError),

    #[error("Watermark error: {0}")]
    Watermark(#[from] WatermarkError),

    #[error("Checkpoint failed: {checkpoint_id}, reason: {reason}")]
    CheckpointFailed {
        checkpoint_id: String,
        reason: String,
    },

    #[error("Recovery failed: {checkpoint_id}, reason: {reason}")]
    RecoveryFailed {
        checkpoint_id: String,
        reason: String,
    },

    #[error("Late event beyond allowed lateness: event_time={event_time}, watermark={watermark}")]
    LateEventDropped {
        event_time: DateTime<Utc>,
        watermark: Watermark,
    },
}
```

### Recovery Strategy

```rust
/// Recovery coordinator for stream processor
pub struct RecoveryCoordinator {
    checkpoint_dir: PathBuf,
    state_backend: Arc<dyn StateBackend>,
}

impl RecoveryCoordinator {
    /// Recover from latest checkpoint
    pub async fn recover_from_checkpoint(&self) -> anyhow::Result<RecoveryState> {
        tracing::info!("Starting recovery from checkpoint");

        // 1. Find latest valid checkpoint
        let checkpoint_path = self.find_latest_checkpoint()?;
        tracing::info!("Found checkpoint: {:?}", checkpoint_path);

        // 2. Load and validate checkpoint
        let checkpoint = Checkpoint::load(&checkpoint_path).await?;
        checkpoint.validate()?;

        // 3. Restore window states
        let mut windows_restored = 0;
        for (key, value) in &checkpoint.data {
            if key.starts_with(b"window:") {
                self.state_backend.put(key, value).await?;
                windows_restored += 1;
            }
        }

        // 4. Restore watermarks
        let mut watermarks = HashMap::new();
        for (key, value) in &checkpoint.data {
            if key.starts_with(b"watermark:") {
                let watermark: Watermark = bincode::deserialize(value)?;
                let partition = self.extract_partition_from_key(key)?;
                watermarks.insert(partition, watermark);
            }
        }

        // 5. Restore Kafka offsets
        let mut kafka_offsets = HashMap::new();
        for (key, value) in &checkpoint.data {
            if key.starts_with(b"kafka_offset:") {
                let offset: i64 = bincode::deserialize(value)?;
                let topic_partition = self.parse_topic_partition(key)?;
                kafka_offsets.insert(topic_partition, offset);
            }
        }

        tracing::info!(
            "Recovery complete: {} windows restored, {} watermarks, {} kafka offsets",
            windows_restored,
            watermarks.len(),
            kafka_offsets.len()
        );

        Ok(RecoveryState {
            checkpoint_id: checkpoint.metadata.checkpoint_id,
            windows_restored,
            watermarks,
            kafka_offsets,
        })
    }

    /// Handle corrupted state
    pub async fn handle_corrupted_state(&self, key: &[u8]) -> anyhow::Result<()> {
        tracing::error!(
            "Detected corrupted state for key: {}",
            String::from_utf8_lossy(key)
        );

        // Option 1: Delete corrupted state and continue
        self.state_backend.delete(key).await?;

        // Option 2: Attempt recovery from backup
        // ...

        // Option 3: Mark window as failed and emit alert
        // ...

        Ok(())
    }

    fn find_latest_checkpoint(&self) -> anyhow::Result<PathBuf> {
        // Implementation similar to CheckpointCoordinator::find_latest_checkpoint
        unimplemented!()
    }

    fn extract_partition_from_key(&self, key: &[u8]) -> anyhow::Result<u32> {
        // Parse partition from key like "watermark:partition_5"
        unimplemented!()
    }

    fn parse_topic_partition(&self, key: &[u8]) -> anyhow::Result<TopicPartition> {
        // Parse from key like "kafka_offset:topic_name:3"
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct RecoveryState {
    pub checkpoint_id: String,
    pub windows_restored: usize,
    pub watermarks: HashMap<u32, Watermark>,
    pub kafka_offsets: HashMap<TopicPartition, i64>,
}
```

### Graceful Degradation

```rust
/// Circuit breaker for state backend
pub struct StateBackendCircuitBreaker {
    backend: Arc<dyn StateBackend>,
    failure_count: Arc<AtomicU64>,
    last_failure: Arc<RwLock<Option<Instant>>>,
    threshold: u64,
    timeout: Duration,
    state: Arc<RwLock<CircuitState>>,
}

#[derive(Debug, Clone, PartialEq)]
enum CircuitState {
    Closed,   // Normal operation
    Open,     // Failing, reject requests
    HalfOpen, // Testing recovery
}

impl StateBackendCircuitBreaker {
    pub async fn get(&self, key: &[u8]) -> StateResult<Option<Vec<u8>>> {
        // Check circuit state
        let state = self.state.read().await;
        if *state == CircuitState::Open {
            return Err(StateError::CircuitOpen {
                backend: "state_backend".to_string(),
            });
        }
        drop(state);

        // Attempt operation
        match self.backend.get(key).await {
            Ok(result) => {
                // Reset failure count on success
                self.failure_count.store(0, Ordering::Relaxed);
                Ok(result)
            }
            Err(e) => {
                // Record failure
                let failures = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
                *self.last_failure.write().await = Some(Instant::now());

                // Check if threshold exceeded
                if failures >= self.threshold {
                    *self.state.write().await = CircuitState::Open;
                    tracing::error!("Circuit breaker opened after {} failures", failures);
                }

                Err(e)
            }
        }
    }

    /// Periodic check to attempt recovery
    pub async fn try_reset(&self) {
        let state = self.state.read().await.clone();

        if state == CircuitState::Open {
            // Check if timeout has passed
            if let Some(last_fail) = *self.last_failure.read().await {
                if last_fail.elapsed() > self.timeout {
                    // Try half-open
                    drop(state);
                    *self.state.write().await = CircuitState::HalfOpen;
                    tracing::info!("Circuit breaker entering half-open state");
                }
            }
        }
    }
}
```

---

## Performance Considerations

### Latency Budget

For <100ms p99 latency target:

```
Operation                      Budget      Notes
──────────────────────────────────────────────────────────────
Kafka consume                  10ms        Network + deserialization
Event parsing                  5ms         JSON/bincode deserialization
Window assignment              5ms         CPU-bound, should be <1ms
State backend read             20ms        Redis GET (network)
Aggregator update              10ms        In-memory computation
State backend write            20ms        Redis SET (network)
Trigger check                  5ms         In-memory comparison
Result serialization           10ms        JSON serialization
Kafka produce                  15ms        Network + acks

Total (worst case):            100ms       Per-event budget
```

### Optimization Strategies

#### 1. Batch State Operations

```rust
/// Batch window updates for efficiency
pub struct BatchWindowUpdater {
    updates: Vec<(Vec<u8>, Vec<u8>)>,
    batch_size: usize,
    state_backend: Arc<dyn StateBackend>,
}

impl BatchWindowUpdater {
    pub async fn update(&mut self, key: Vec<u8>, value: Vec<u8>) -> StateResult<()> {
        self.updates.push((key, value));

        if self.updates.len() >= self.batch_size {
            self.flush().await?;
        }

        Ok(())
    }

    pub async fn flush(&mut self) -> StateResult<()> {
        if self.updates.is_empty() {
            return Ok(());
        }

        // Batch write to Redis
        let kvs: Vec<(&[u8], &[u8])> = self.updates
            .iter()
            .map(|(k, v)| (k.as_slice(), v.as_slice()))
            .collect();

        self.state_backend.batch_put(&kvs).await?;
        self.updates.clear();

        Ok(())
    }
}
```

#### 2. Async Aggregation Pipeline

```rust
/// Parallel aggregation for multiple windows
pub async fn update_windows_parallel(
    windows: Vec<Window>,
    key: &str,
    value: f64,
    executor: Arc<WindowAggregationExecutor>,
) -> Vec<StateResult<()>> {
    let futures: Vec<_> = windows
        .iter()
        .map(|window| {
            let executor = Arc::clone(&executor);
            let key = key.to_string();
            async move {
                executor.update_window(window, &key, value).await
            }
        })
        .collect();

    futures::future::join_all(futures).await
}
```

#### 3. Memory-Efficient Aggregators

```rust
/// Use HdrHistogram for bounded memory percentile calculation
/// Memory usage: O(1) instead of O(n) for exact percentiles
///
/// For 99th percentile with 2 significant digits:
/// - Memory: ~2KB per window
/// - Accuracy: ±1% relative error
/// - Update: O(1)
/// - Query: O(1)
pub fn create_efficient_percentile_aggregator() -> HdrPercentileAggregator {
    HdrPercentileAggregator::new(
        0.99,                    // p99
        1_000_000,               // max value (1 million ms = 16 minutes)
        2,                       // 2 significant figures
    )
}
```

#### 4. Connection Pooling

Reuse connections for Redis operations:

```rust
// Configure Redis with connection pool
let config = RedisConfig::builder()
    .pool_size(20, 100)  // min=20, max=100 connections
    .build()?;
```

### Monitoring & Metrics

```rust
/// Comprehensive metrics for stream processor
pub struct ProcessorMetrics {
    // Throughput
    events_processed: Counter,
    events_per_second: Gauge,

    // Latency
    processing_latency_histogram: Histogram,
    state_read_latency: Histogram,
    state_write_latency: Histogram,

    // Windows
    active_windows: Gauge,
    triggered_windows: Counter,
    expired_windows: Counter,

    // Watermarks
    watermark_lag: Gauge,
    late_events: Counter,
    dropped_late_events: Counter,

    // State
    state_size_bytes: Gauge,
    cache_hit_rate: Gauge,

    // Checkpoints
    checkpoint_duration: Histogram,
    checkpoint_size: Gauge,
    checkpoint_failures: Counter,
}

impl ProcessorMetrics {
    pub fn record_processing_latency(&self, duration: Duration) {
        self.processing_latency_histogram.observe(duration.as_secs_f64());
    }

    pub fn record_watermark_lag(&self, lag: Duration) {
        self.watermark_lag.set(lag.as_secs_f64());
    }

    pub async fn collect_stats(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            events_per_second: self.events_per_second.get(),
            p50_latency_ms: self.processing_latency_histogram.percentile(0.50) * 1000.0,
            p95_latency_ms: self.processing_latency_histogram.percentile(0.95) * 1000.0,
            p99_latency_ms: self.processing_latency_histogram.percentile(0.99) * 1000.0,
            active_windows: self.active_windows.get() as usize,
            watermark_lag_seconds: self.watermark_lag.get(),
            cache_hit_rate: self.cache_hit_rate.get(),
        }
    }
}
```

---

## Implementation Guide

### Phase 1: Core Window Management (Week 1-2)

**Tasks:**
1. Implement `WindowLifecycleManager`
2. Complete window assignment for all types
3. Add window state serialization
4. Unit tests for window logic

**Acceptance Criteria:**
- All window types (Tumbling, Sliding, Session) working
- Window assignment correctness verified
- State serialization/deserialization working
- 100% test coverage for window logic

### Phase 2: Aggregation Engine (Week 2-3)

**Tasks:**
1. Implement `AggregatorRegistry`
2. Add all standard aggregators (count, sum, avg, min, max)
3. Implement HdrHistogram-based percentile aggregator
4. Add `WindowAggregationExecutor`

**Acceptance Criteria:**
- All aggregators producing correct results
- Percentile aggregator using HdrHistogram
- Aggregator merging working for distributed processing
- Benchmark showing <10ms aggregation time

### Phase 3: State & Checkpointing (Week 3-4)

**Tasks:**
1. Integrate Redis state backend
2. Implement `DistributedCheckpointCoordinator`
3. Add recovery logic
4. Batch operations for state

**Acceptance Criteria:**
- Redis integration working
- Checkpointing creating valid snapshots
- Recovery restoring full state
- Batch operations improving throughput by >50%

### Phase 4: Integration & Testing (Week 4-5)

**Tasks:**
1. Complete end-to-end pipeline
2. Integration tests with Kafka
3. Performance testing
4. Chaos testing (simulated failures)

**Acceptance Criteria:**
- Full pipeline processing events
- P99 latency < 100ms
- Throughput > 10,000 events/sec
- Recovery working after simulated failures

### Testing Strategy

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tumbling_window_assignment() {
        let manager = TumblingWindowManager::new(Duration::minutes(5));

        // Test event at 12:03:45
        let event_time = Utc.ymd(2024, 1, 1).and_hms(12, 3, 45);
        let window = manager.assign_window(event_time);

        // Should be assigned to [12:00:00, 12:05:00)
        assert_eq!(window.bounds.start.hour(), 12);
        assert_eq!(window.bounds.start.minute(), 0);
        assert_eq!(window.bounds.end.hour(), 12);
        assert_eq!(window.bounds.end.minute(), 5);
    }

    #[tokio::test]
    async fn test_sliding_window_multiple_assignments() {
        let manager = SlidingWindowManager::new(
            Duration::minutes(10),
            Duration::minutes(1),
        );

        let event_time = Utc.ymd(2024, 1, 1).and_hms(12, 5, 30);
        let windows = manager.assign_windows(event_time);

        // Should be in 6 windows
        assert_eq!(windows.len(), 6);
    }

    #[tokio::test]
    async fn test_session_window_merging() {
        let manager = SessionWindowManager::new(Duration::seconds(5));

        // Process events within gap
        let w1 = manager.process_event("key1".to_string(),
            Utc.ymd(2024, 1, 1).and_hms(12, 0, 0)).await.unwrap();
        let w2 = manager.process_event("key1".to_string(),
            Utc.ymd(2024, 1, 1).and_hms(12, 0, 2)).await.unwrap();

        // Should merge into single session
        assert_eq!(w1.id, w2.id);
    }

    #[tokio::test]
    async fn test_aggregation_correctness() {
        let mut agg = AverageAggregator::new();
        agg.update_batch(&[10.0, 20.0, 30.0]).unwrap();

        assert_eq!(agg.finalize().unwrap(), 20.0);
        assert_eq!(agg.count(), 3);
    }

    #[tokio::test]
    async fn test_percentile_aggregator_hdr() {
        let mut agg = HdrPercentileAggregator::new(0.99, 1_000_000, 2);

        // Add 1000 samples
        for i in 1..=1000 {
            agg.update(i as f64).unwrap();
        }

        let p99 = agg.finalize().unwrap();

        // p99 should be around 990
        assert!((p99 - 990.0).abs() < 20.0);
    }

    #[tokio::test]
    async fn test_checkpoint_recovery() {
        let temp_dir = tempfile::tempdir().unwrap();
        let backend = Arc::new(MemoryStateBackend::new(None));

        // Add some state
        backend.put(b"window:1", b"state1").await.unwrap();
        backend.put(b"window:2", b"state2").await.unwrap();

        // Create checkpoint
        let coordinator = CheckpointCoordinator::new(
            backend.clone(),
            Duration::from_secs(60),
            CheckpointOptions {
                checkpoint_dir: temp_dir.path().to_path_buf(),
                max_checkpoints: 5,
                compress: false,
                min_interval: Duration::from_secs(1),
            },
        );

        let checkpoint_id = coordinator.create_checkpoint().await.unwrap();

        // Clear state
        backend.clear().await.unwrap();
        assert_eq!(backend.count().await.unwrap(), 0);

        // Restore
        coordinator.restore_latest().await.unwrap();
        assert_eq!(backend.count().await.unwrap(), 2);
    }
}
```

---

## Conclusion

This architecture provides a robust, scalable foundation for the Stream Processor's windowing and aggregation system. Key design principles:

1. **Performance First**: Sub-100ms latency through batching, connection pooling, and efficient algorithms
2. **Fault Tolerance**: Checkpointing and recovery for zero data loss
3. **Scalability**: Distributed state with Redis, horizontal scaling via partitioning
4. **Flexibility**: Support for multiple window types and aggregation functions
5. **Production Ready**: Comprehensive error handling, monitoring, and graceful degradation

### Next Steps

1. Review and approve architecture
2. Begin Phase 1 implementation
3. Set up monitoring infrastructure
4. Establish performance benchmarks
5. Plan integration with Analyzer and Decision engines

---

**Document Version:** 1.0
**Last Updated:** 2025-11-10
**Status:** Ready for Implementation
