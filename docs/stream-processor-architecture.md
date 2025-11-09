# Stream Processor Architecture Design
**LLM Auto-Optimizer - Enterprise-Grade Event Stream Processing**

Version: 1.0
Date: 2025-11-09
Status: Design Document

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Architecture Overview](#architecture-overview)
3. [Module Structure](#module-structure)
4. [Core Data Structures](#core-data-structures)
5. [Processing Pipeline](#processing-pipeline)
6. [Window Management](#window-management)
7. [Aggregation Engine](#aggregation-engine)
8. [Watermarking and Late Events](#watermarking-and-late-events)
9. [State Management](#state-management)
10. [Error Handling Strategy](#error-handling-strategy)
11. [Configuration Structure](#configuration-structure)
12. [API Design](#api-design)
13. [Performance Considerations](#performance-considerations)
14. [Testing Strategy](#testing-strategy)

---

## Executive Summary

The Stream Processor is a critical component of the LLM Auto-Optimizer that processes feedback events from Kafka and the Collector in real-time. It provides enterprise-grade windowing, aggregation, and state management capabilities to transform raw events into actionable insights.

**Key Features:**
- Multi-window support (tumbling, sliding, session)
- Rich aggregation functions (count, sum, avg, percentiles, stddev)
- Watermarking for late event handling
- Persistent state management
- High throughput (10,000+ events/sec)
- Low latency processing (<100ms p99)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                     Stream Processor                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  ┌──────────────┐         ┌──────────────┐                      │
│  │   Kafka      │────────▶│    Event     │                      │
│  │  Consumer    │         │   Ingester   │                      │
│  └──────────────┘         └──────────────┘                      │
│                                   │                              │
│                                   ▼                              │
│                           ┌──────────────┐                      │
│                           │  Watermark   │                      │
│                           │  Generator   │                      │
│                           └──────────────┘                      │
│                                   │                              │
│                                   ▼                              │
│                           ┌──────────────┐                      │
│                           │   Window     │                      │
│                           │  Assigner    │                      │
│                           └──────────────┘                      │
│                                   │                              │
│                   ┌───────────────┼───────────────┐             │
│                   ▼               ▼               ▼             │
│            ┌──────────┐    ┌──────────┐   ┌──────────┐         │
│            │ Tumbling │    │ Sliding  │   │ Session  │         │
│            │  Window  │    │  Window  │   │  Window  │         │
│            └──────────┘    └──────────┘   └──────────┘         │
│                   │               │               │             │
│                   └───────────────┼───────────────┘             │
│                                   ▼                              │
│                           ┌──────────────┐                      │
│                           │ Aggregation  │                      │
│                           │   Engine     │                      │
│                           └──────────────┘                      │
│                                   │                              │
│                                   ▼                              │
│                           ┌──────────────┐                      │
│                           │    State     │◀────┐                │
│                           │   Manager    │     │                │
│                           └──────────────┘     │                │
│                                   │            │                │
│                                   ▼            │                │
│                           ┌──────────────┐     │                │
│                           │   Result     │     │                │
│                           │   Emitter    │     │                │
│                           └──────────────┘     │                │
│                                   │            │                │
│                                   ├────────────┘                │
│                                   │                              │
│                                   ▼                              │
│                           ┌──────────────┐                      │
│                           │   Output     │                      │
│                           │   Sinks      │                      │
│                           └──────────────┘                      │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
```

---

## Module Structure

### File Organization

```
crates/processor/
├── Cargo.toml
├── src/
│   ├── lib.rs                      # Public API exports
│   │
│   ├── core/
│   │   ├── mod.rs                  # Core module exports
│   │   ├── event.rs                # Event wrapper with metadata
│   │   ├── timestamp.rs            # Timestamp and time utilities
│   │   └── key.rs                  # Grouping key abstractions
│   │
│   ├── window/
│   │   ├── mod.rs                  # Window module exports
│   │   ├── types.rs                # Window type definitions
│   │   ├── assigner.rs             # Window assignment logic
│   │   ├── tumbling.rs             # Tumbling window implementation
│   │   ├── sliding.rs              # Sliding window implementation
│   │   ├── session.rs              # Session window implementation
│   │   └── trigger.rs              # Window trigger policies
│   │
│   ├── aggregation/
│   │   ├── mod.rs                  # Aggregation module exports
│   │   ├── aggregator.rs           # Aggregator trait and registry
│   │   ├── count.rs                # Count aggregator
│   │   ├── sum.rs                  # Sum aggregator
│   │   ├── average.rs              # Average aggregator
│   │   ├── minmax.rs               # Min/Max aggregators
│   │   ├── percentile.rs           # Percentile aggregator (p50, p95, p99)
│   │   ├── stddev.rs               # Standard deviation aggregator
│   │   └── composite.rs            # Composite aggregator for multiple metrics
│   │
│   ├── watermark/
│   │   ├── mod.rs                  # Watermark module exports
│   │   ├── generator.rs            # Watermark generation strategies
│   │   ├── bounded.rs              # Bounded out-of-orderness watermark
│   │   ├── periodic.rs             # Periodic watermark generator
│   │   └── alignment.rs            # Timestamp alignment utilities
│   │
│   ├── state/
│   │   ├── mod.rs                  # State module exports
│   │   ├── manager.rs              # State management coordinator
│   │   ├── backend.rs              # State backend trait
│   │   ├── memory.rs               # In-memory state backend
│   │   ├── rocksdb.rs              # RocksDB state backend
│   │   ├── checkpoint.rs           # Checkpointing logic
│   │   └── recovery.rs             # State recovery logic
│   │
│   ├── pipeline/
│   │   ├── mod.rs                  # Pipeline module exports
│   │   ├── builder.rs              # Pipeline builder API
│   │   ├── executor.rs             # Pipeline execution engine
│   │   ├── operator.rs             # Stream operators (map, filter, etc.)
│   │   └── sink.rs                 # Output sink implementations
│   │
│   ├── kafka/
│   │   ├── mod.rs                  # Kafka module exports
│   │   ├── consumer.rs             # Kafka consumer wrapper
│   │   ├── producer.rs             # Kafka producer wrapper
│   │   └── config.rs               # Kafka configuration
│   │
│   ├── metrics/
│   │   ├── mod.rs                  # Metrics module exports
│   │   └── collector.rs            # Processor metrics collection
│   │
│   ├── error.rs                    # Error types
│   └── config.rs                   # Configuration structures
│
├── examples/
│   ├── basic_window.rs             # Basic windowing example
│   ├── session_window.rs           # Session window example
│   └── multi_aggregation.rs        # Multiple aggregation example
│
├── benches/
│   ├── window_performance.rs       # Window performance benchmarks
│   └── aggregation_performance.rs  # Aggregation benchmarks
│
└── tests/
    ├── integration/
    │   ├── end_to_end.rs           # End-to-end processing tests
    │   ├── late_events.rs          # Late event handling tests
    │   └── state_recovery.rs       # State recovery tests
    └── unit/
        ├── window_tests.rs         # Window unit tests
        └── aggregation_tests.rs    # Aggregation unit tests
```

### Module Purposes

#### **core/**
Fundamental types and utilities used across the stream processor.
- Event wrappers with timestamp extraction
- Time utilities and alignment
- Grouping key abstractions

#### **window/**
Window management and assignment logic.
- Window type definitions
- Assignment strategies for different window types
- Trigger policies for window emission

#### **aggregation/**
Aggregation functions and composites.
- Trait-based aggregator system
- Individual aggregator implementations
- Composite aggregators for complex metrics

#### **watermark/**
Watermark generation and late event handling.
- Various watermark generation strategies
- Bounded disorder handling
- Timestamp alignment

#### **state/**
State persistence and recovery.
- State backend abstraction
- Multiple backend implementations
- Checkpointing and recovery

#### **pipeline/**
Stream processing pipeline construction and execution.
- Builder API for pipeline configuration
- Execution engine
- Stream operators

#### **kafka/**
Kafka integration for event ingestion and output.
- Consumer wrapper with offset management
- Producer wrapper for results
- Configuration management

#### **metrics/**
Internal metrics collection for processor monitoring.
- Processing latency metrics
- Throughput metrics
- State size metrics

---

## Core Data Structures

### Event Wrapper

```rust
/// Enriched event with processing metadata
#[derive(Debug, Clone)]
pub struct ProcessorEvent<T> {
    /// Original event data
    pub data: T,

    /// Event timestamp (extracted or assigned)
    pub timestamp: EventTimestamp,

    /// Processing timestamp (when received)
    pub processing_time: DateTime<Utc>,

    /// Grouping key for partitioning
    pub key: Option<GroupingKey>,

    /// Event metadata
    pub metadata: EventMetadata,
}

#[derive(Debug, Clone)]
pub enum EventTimestamp {
    /// Event time (from event itself)
    Event(DateTime<Utc>),

    /// Processing time (when received)
    Processing(DateTime<Utc>),

    /// Ingestion time (when entered system)
    Ingestion(DateTime<Utc>),
}

#[derive(Debug, Clone)]
pub struct EventMetadata {
    /// Source partition (for Kafka)
    pub partition: Option<i32>,

    /// Event offset
    pub offset: Option<i64>,

    /// Custom metadata
    pub attributes: HashMap<String, String>,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct GroupingKey(String);

impl GroupingKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    pub fn from_fields(fields: &[(&str, &str)]) -> Self {
        let key = fields
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("::");
        Self(key)
    }
}
```

### Window Definitions

```rust
/// Window type discriminator
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowType {
    Tumbling,
    Sliding,
    Session,
}

/// Window specification
#[derive(Debug, Clone)]
pub struct WindowSpec {
    /// Type of window
    pub window_type: WindowType,

    /// Window size
    pub size: Duration,

    /// Slide interval (for sliding windows)
    pub slide: Option<Duration>,

    /// Gap duration (for session windows)
    pub gap: Option<Duration>,

    /// Allowed lateness
    pub allowed_lateness: Duration,
}

/// Concrete window instance
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Window {
    /// Window start time (inclusive)
    pub start: DateTime<Utc>,

    /// Window end time (exclusive)
    pub end: DateTime<Utc>,

    /// Grouping key (for keyed windows)
    pub key: Option<GroupingKey>,
}

impl Window {
    /// Check if timestamp falls within window
    pub fn contains(&self, timestamp: &DateTime<Utc>) -> bool {
        timestamp >= &self.start && timestamp < &self.end
    }

    /// Get window duration
    pub fn duration(&self) -> Duration {
        self.end.signed_duration_since(self.start)
            .to_std()
            .unwrap_or(Duration::from_secs(0))
    }

    /// Get window ID for serialization
    pub fn id(&self) -> String {
        format!("{}_{}",
            self.start.timestamp_millis(),
            self.end.timestamp_millis()
        )
    }
}
```

### Window Assigners

```rust
/// Window assignment trait
#[async_trait]
pub trait WindowAssigner: Send + Sync {
    /// Assign event to windows
    fn assign_windows(
        &self,
        timestamp: DateTime<Utc>,
        key: &Option<GroupingKey>,
    ) -> Vec<Window>;

    /// Get window spec
    fn spec(&self) -> &WindowSpec;
}

/// Tumbling window assigner
pub struct TumblingWindowAssigner {
    spec: WindowSpec,
    offset: Duration,
}

impl TumblingWindowAssigner {
    pub fn new(size: Duration) -> Self {
        Self {
            spec: WindowSpec {
                window_type: WindowType::Tumbling,
                size,
                slide: None,
                gap: None,
                allowed_lateness: Duration::from_secs(0),
            },
            offset: Duration::from_secs(0),
        }
    }

    pub fn with_offset(mut self, offset: Duration) -> Self {
        self.offset = offset;
        self
    }

    pub fn with_allowed_lateness(mut self, lateness: Duration) -> Self {
        self.spec.allowed_lateness = lateness;
        self
    }
}

impl WindowAssigner for TumblingWindowAssigner {
    fn assign_windows(
        &self,
        timestamp: DateTime<Utc>,
        key: &Option<GroupingKey>,
    ) -> Vec<Window> {
        let size_millis = self.spec.size.as_millis() as i64;
        let offset_millis = self.offset.as_millis() as i64;
        let ts_millis = timestamp.timestamp_millis();

        let start_millis = ((ts_millis - offset_millis) / size_millis)
            * size_millis + offset_millis;
        let end_millis = start_millis + size_millis;

        let start = DateTime::from_timestamp_millis(start_millis)
            .unwrap_or(timestamp);
        let end = DateTime::from_timestamp_millis(end_millis)
            .unwrap_or(timestamp + chrono::Duration::milliseconds(size_millis));

        vec![Window {
            start,
            end,
            key: key.clone(),
        }]
    }

    fn spec(&self) -> &WindowSpec {
        &self.spec
    }
}

/// Sliding window assigner
pub struct SlidingWindowAssigner {
    spec: WindowSpec,
}

impl SlidingWindowAssigner {
    pub fn new(size: Duration, slide: Duration) -> Self {
        Self {
            spec: WindowSpec {
                window_type: WindowType::Sliding,
                size,
                slide: Some(slide),
                gap: None,
                allowed_lateness: Duration::from_secs(0),
            },
        }
    }
}

impl WindowAssigner for SlidingWindowAssigner {
    fn assign_windows(
        &self,
        timestamp: DateTime<Utc>,
        key: &Option<GroupingKey>,
    ) -> Vec<Window> {
        let size_millis = self.spec.size.as_millis() as i64;
        let slide_millis = self.spec.slide.unwrap().as_millis() as i64;
        let ts_millis = timestamp.timestamp_millis();

        let mut windows = Vec::new();

        // Calculate the first window that contains this timestamp
        let last_start = (ts_millis / slide_millis) * slide_millis;

        // Generate all windows that contain this timestamp
        let mut start_millis = last_start;
        while start_millis > ts_millis - size_millis {
            let end_millis = start_millis + size_millis;

            if let (Some(start), Some(end)) = (
                DateTime::from_timestamp_millis(start_millis),
                DateTime::from_timestamp_millis(end_millis),
            ) {
                if timestamp >= start && timestamp < end {
                    windows.push(Window {
                        start,
                        end,
                        key: key.clone(),
                    });
                }
            }

            start_millis -= slide_millis;
        }

        windows
    }

    fn spec(&self) -> &WindowSpec {
        &self.spec
    }
}

/// Session window assigner
pub struct SessionWindowAssigner {
    spec: WindowSpec,
}

impl SessionWindowAssigner {
    pub fn new(gap: Duration) -> Self {
        Self {
            spec: WindowSpec {
                window_type: WindowType::Session,
                size: Duration::from_secs(0), // Dynamic
                slide: None,
                gap: Some(gap),
                allowed_lateness: Duration::from_secs(0),
            },
        }
    }
}

impl WindowAssigner for SessionWindowAssigner {
    fn assign_windows(
        &self,
        timestamp: DateTime<Utc>,
        key: &Option<GroupingKey>,
    ) -> Vec<Window> {
        // Session windows are initially created as single-event windows
        // and merged during aggregation
        vec![Window {
            start: timestamp,
            end: timestamp + chrono::Duration::from_std(self.spec.gap.unwrap())
                .unwrap_or(chrono::Duration::zero()),
            key: key.clone(),
        }]
    }

    fn spec(&self) -> &WindowSpec {
        &self.spec
    }
}
```

### Aggregator Trait

```rust
/// Aggregator trait for window computations
pub trait Aggregator<T, R>: Send + Sync {
    /// Initialize accumulator
    fn create_accumulator(&self) -> R;

    /// Add element to accumulator
    fn add(&self, accumulator: &mut R, value: &T);

    /// Merge two accumulators
    fn merge(&self, acc1: &R, acc2: &R) -> R;

    /// Extract final result
    fn get_result(&self, accumulator: &R) -> R;

    /// Get aggregator name
    fn name(&self) -> &str;
}

/// Accumulator for count aggregation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CountAccumulator {
    pub count: u64,
}

/// Count aggregator
pub struct CountAggregator;

impl<T> Aggregator<T, CountAccumulator> for CountAggregator {
    fn create_accumulator(&self) -> CountAccumulator {
        CountAccumulator { count: 0 }
    }

    fn add(&self, acc: &mut CountAccumulator, _value: &T) {
        acc.count += 1;
    }

    fn merge(&self, acc1: &CountAccumulator, acc2: &CountAccumulator) -> CountAccumulator {
        CountAccumulator {
            count: acc1.count + acc2.count,
        }
    }

    fn get_result(&self, acc: &CountAccumulator) -> CountAccumulator {
        acc.clone()
    }

    fn name(&self) -> &str {
        "count"
    }
}

/// Accumulator for average aggregation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AverageAccumulator {
    pub sum: f64,
    pub count: u64,
}

impl AverageAccumulator {
    pub fn average(&self) -> f64 {
        if self.count > 0 {
            self.sum / self.count as f64
        } else {
            0.0
        }
    }
}

/// Average aggregator
pub struct AverageAggregator;

impl Aggregator<f64, AverageAccumulator> for AverageAggregator {
    fn create_accumulator(&self) -> AverageAccumulator {
        AverageAccumulator::default()
    }

    fn add(&self, acc: &mut AverageAccumulator, value: &f64) {
        acc.sum += value;
        acc.count += 1;
    }

    fn merge(&self, acc1: &AverageAccumulator, acc2: &AverageAccumulator) -> AverageAccumulator {
        AverageAccumulator {
            sum: acc1.sum + acc2.sum,
            count: acc1.count + acc2.count,
        }
    }

    fn get_result(&self, acc: &AverageAccumulator) -> AverageAccumulator {
        acc.clone()
    }

    fn name(&self) -> &str {
        "average"
    }
}

/// Accumulator for percentile aggregation using T-Digest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PercentileAccumulator {
    pub values: Vec<f64>,
    pub sorted: bool,
}

impl Default for PercentileAccumulator {
    fn default() -> Self {
        Self {
            values: Vec::new(),
            sorted: false,
        }
    }
}

impl PercentileAccumulator {
    pub fn percentile(&mut self, p: f64) -> Option<f64> {
        if self.values.is_empty() {
            return None;
        }

        if !self.sorted {
            self.values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            self.sorted = true;
        }

        let index = ((p / 100.0) * (self.values.len() - 1) as f64).round() as usize;
        Some(self.values[index])
    }

    pub fn p50(&mut self) -> Option<f64> {
        self.percentile(50.0)
    }

    pub fn p95(&mut self) -> Option<f64> {
        self.percentile(95.0)
    }

    pub fn p99(&mut self) -> Option<f64> {
        self.percentile(99.0)
    }
}

/// Percentile aggregator
pub struct PercentileAggregator;

impl Aggregator<f64, PercentileAccumulator> for PercentileAggregator {
    fn create_accumulator(&self) -> PercentileAccumulator {
        PercentileAccumulator::default()
    }

    fn add(&self, acc: &mut PercentileAccumulator, value: &f64) {
        acc.values.push(*value);
        acc.sorted = false;
    }

    fn merge(&self, acc1: &PercentileAccumulator, acc2: &PercentileAccumulator) -> PercentileAccumulator {
        let mut values = acc1.values.clone();
        values.extend(&acc2.values);
        PercentileAccumulator {
            values,
            sorted: false,
        }
    }

    fn get_result(&self, acc: &PercentileAccumulator) -> PercentileAccumulator {
        acc.clone()
    }

    fn name(&self) -> &str {
        "percentile"
    }
}

/// Composite aggregator for multiple metrics
pub struct CompositeAggregator {
    pub aggregators: Vec<Box<dyn Any + Send + Sync>>,
    pub names: Vec<String>,
}

/// Aggregation result container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregationResult {
    /// Window information
    pub window: Window,

    /// Aggregation results by name
    pub results: HashMap<String, serde_json::Value>,

    /// Timestamp when aggregation completed
    pub timestamp: DateTime<Utc>,
}
```

### Watermark

```rust
/// Watermark represents progress in event time
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Watermark {
    pub timestamp: DateTime<Utc>,
}

impl Watermark {
    pub fn new(timestamp: DateTime<Utc>) -> Self {
        Self { timestamp }
    }

    pub fn min() -> Self {
        Self {
            timestamp: DateTime::from_timestamp(0, 0).unwrap(),
        }
    }

    pub fn max() -> Self {
        Self {
            timestamp: DateTime::from_timestamp(i64::MAX, 0).unwrap(),
        }
    }
}

/// Watermark generation strategy
pub trait WatermarkGenerator: Send + Sync {
    /// Generate watermark based on observed timestamps
    fn generate(&mut self, timestamp: DateTime<Utc>) -> Option<Watermark>;

    /// Get current watermark
    fn current_watermark(&self) -> Watermark;
}

/// Bounded out-of-orderness watermark generator
pub struct BoundedOutOfOrdernessWatermark {
    max_out_of_orderness: Duration,
    current_watermark: Watermark,
    max_timestamp: DateTime<Utc>,
}

impl BoundedOutOfOrdernessWatermark {
    pub fn new(max_out_of_orderness: Duration) -> Self {
        Self {
            max_out_of_orderness,
            current_watermark: Watermark::min(),
            max_timestamp: DateTime::from_timestamp(0, 0).unwrap(),
        }
    }
}

impl WatermarkGenerator for BoundedOutOfOrdernessWatermark {
    fn generate(&mut self, timestamp: DateTime<Utc>) -> Option<Watermark> {
        if timestamp > self.max_timestamp {
            self.max_timestamp = timestamp;

            let new_watermark = Watermark::new(
                timestamp - chrono::Duration::from_std(self.max_out_of_orderness)
                    .unwrap_or(chrono::Duration::zero())
            );

            if new_watermark > self.current_watermark {
                self.current_watermark = new_watermark;
                return Some(self.current_watermark);
            }
        }

        None
    }

    fn current_watermark(&self) -> Watermark {
        self.current_watermark
    }
}

/// Periodic watermark generator (emits watermarks at intervals)
pub struct PeriodicWatermarkGenerator {
    interval: Duration,
    generator: Box<dyn WatermarkGenerator>,
    last_emission: Instant,
}

impl PeriodicWatermarkGenerator {
    pub fn new(interval: Duration, generator: Box<dyn WatermarkGenerator>) -> Self {
        Self {
            interval,
            generator,
            last_emission: Instant::now(),
        }
    }

    pub fn should_emit(&self) -> bool {
        self.last_emission.elapsed() >= self.interval
    }
}
```

### State Management

```rust
/// State backend trait
#[async_trait]
pub trait StateBackend: Send + Sync {
    /// Get state value
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;

    /// Put state value
    async fn put(&self, key: &[u8], value: &[u8]) -> Result<()>;

    /// Delete state value
    async fn delete(&self, key: &[u8]) -> Result<()>;

    /// Create checkpoint
    async fn checkpoint(&self, checkpoint_id: u64) -> Result<()>;

    /// Restore from checkpoint
    async fn restore(&self, checkpoint_id: u64) -> Result<()>;

    /// List all keys (for recovery)
    async fn list_keys(&self, prefix: &[u8]) -> Result<Vec<Vec<u8>>>;
}

/// Window state manager
pub struct WindowStateManager<A> {
    backend: Arc<dyn StateBackend>,
    namespace: String,
    _phantom: PhantomData<A>,
}

impl<A: Serialize + DeserializeOwned> WindowStateManager<A> {
    pub fn new(backend: Arc<dyn StateBackend>, namespace: impl Into<String>) -> Self {
        Self {
            backend,
            namespace: namespace.into(),
            _phantom: PhantomData,
        }
    }

    /// Get accumulator for window
    pub async fn get_accumulator(&self, window: &Window) -> Result<Option<A>> {
        let key = self.window_key(window);

        if let Some(bytes) = self.backend.get(&key).await? {
            let acc = bincode::deserialize(&bytes)
                .map_err(|e| OptimizerError::SerializationError(e.to_string()))?;
            Ok(Some(acc))
        } else {
            Ok(None)
        }
    }

    /// Put accumulator for window
    pub async fn put_accumulator(&self, window: &Window, accumulator: &A) -> Result<()> {
        let key = self.window_key(window);
        let bytes = bincode::serialize(accumulator)
            .map_err(|e| OptimizerError::SerializationError(e.to_string()))?;

        self.backend.put(&key, &bytes).await
    }

    /// Delete window state
    pub async fn delete_window(&self, window: &Window) -> Result<()> {
        let key = self.window_key(window);
        self.backend.delete(&key).await
    }

    fn window_key(&self, window: &Window) -> Vec<u8> {
        let key_str = match &window.key {
            Some(k) => format!("{}:{}:{}", self.namespace, k.0, window.id()),
            None => format!("{}:{}", self.namespace, window.id()),
        };
        key_str.into_bytes()
    }
}
```

---

## Processing Pipeline

### Pipeline Flow

```
Event Ingestion → Watermark Generation → Window Assignment → Aggregation → State Update → Output Emission
```

### Pipeline Builder API

```rust
/// Stream processing pipeline builder
pub struct StreamPipelineBuilder<T> {
    source: Option<Box<dyn EventSource<T>>>,
    watermark_generator: Option<Box<dyn WatermarkGenerator>>,
    window_assigner: Option<Arc<dyn WindowAssigner>>,
    aggregators: Vec<Box<dyn Any>>,
    state_backend: Option<Arc<dyn StateBackend>>,
    sinks: Vec<Box<dyn EventSink<AggregationResult>>>,
}

impl<T> StreamPipelineBuilder<T>
where
    T: Send + Sync + Clone + 'static,
{
    pub fn new() -> Self {
        Self {
            source: None,
            watermark_generator: None,
            window_assigner: None,
            aggregators: Vec::new(),
            state_backend: None,
            sinks: Vec::new(),
        }
    }

    pub fn with_kafka_source(mut self, config: KafkaConfig) -> Self {
        self.source = Some(Box::new(KafkaEventSource::new(config)));
        self
    }

    pub fn with_watermark_generator(
        mut self,
        generator: impl WatermarkGenerator + 'static
    ) -> Self {
        self.watermark_generator = Some(Box::new(generator));
        self
    }

    pub fn with_tumbling_window(mut self, size: Duration) -> Self {
        self.window_assigner = Some(Arc::new(TumblingWindowAssigner::new(size)));
        self
    }

    pub fn with_sliding_window(mut self, size: Duration, slide: Duration) -> Self {
        self.window_assigner = Some(Arc::new(SlidingWindowAssigner::new(size, slide)));
        self
    }

    pub fn with_session_window(mut self, gap: Duration) -> Self {
        self.window_assigner = Some(Arc::new(SessionWindowAssigner::new(gap)));
        self
    }

    pub fn with_state_backend(mut self, backend: Arc<dyn StateBackend>) -> Self {
        self.state_backend = Some(backend);
        self
    }

    pub fn with_sink(mut self, sink: impl EventSink<AggregationResult> + 'static) -> Self {
        self.sinks.push(Box::new(sink));
        self
    }

    pub fn build(self) -> Result<StreamPipeline<T>> {
        // Validation
        let source = self.source.ok_or_else(||
            OptimizerError::ConfigError("Source is required".into()))?;
        let watermark_generator = self.watermark_generator.ok_or_else(||
            OptimizerError::ConfigError("Watermark generator is required".into()))?;
        let window_assigner = self.window_assigner.ok_or_else(||
            OptimizerError::ConfigError("Window assigner is required".into()))?;
        let state_backend = self.state_backend.ok_or_else(||
            OptimizerError::ConfigError("State backend is required".into()))?;

        Ok(StreamPipeline {
            source,
            watermark_generator,
            window_assigner,
            state_backend,
            sinks: self.sinks,
            metrics: Arc::new(ProcessorMetrics::new()),
        })
    }
}

/// Stream processing pipeline
pub struct StreamPipeline<T> {
    source: Box<dyn EventSource<T>>,
    watermark_generator: Box<dyn WatermarkGenerator>,
    window_assigner: Arc<dyn WindowAssigner>,
    state_backend: Arc<dyn StateBackend>,
    sinks: Vec<Box<dyn EventSink<AggregationResult>>>,
    metrics: Arc<ProcessorMetrics>,
}

impl<T> StreamPipeline<T>
where
    T: Send + Sync + Clone + 'static,
{
    /// Start the pipeline
    pub async fn run(mut self) -> Result<()> {
        let mut event_stream = self.source.stream().await?;

        while let Some(event) = event_stream.next().await {
            let start = Instant::now();

            // Process event
            if let Err(e) = self.process_event(event).await {
                tracing::error!("Failed to process event: {}", e);
                self.metrics.record_error();
            }

            self.metrics.record_processing_latency(start.elapsed());
        }

        Ok(())
    }

    async fn process_event(&mut self, event: ProcessorEvent<T>) -> Result<()> {
        // Generate watermark
        if let Some(watermark) = self.watermark_generator.generate(event.timestamp.time()) {
            self.handle_watermark(watermark).await?;
        }

        // Assign to windows
        let windows = self.window_assigner.assign_windows(
            event.timestamp.time(),
            &event.key,
        );

        // Update window state
        for window in windows {
            self.update_window_state(&window, &event).await?;
        }

        Ok(())
    }

    async fn handle_watermark(&mut self, watermark: Watermark) -> Result<()> {
        // Trigger windows that are before the watermark
        // Emit results
        // Clean up old state
        Ok(())
    }

    async fn update_window_state(
        &self,
        window: &Window,
        event: &ProcessorEvent<T>
    ) -> Result<()> {
        // Get or create accumulator
        // Update with event
        // Save state
        Ok(())
    }
}
```

---

## Window Management

### Window Trigger Policies

```rust
/// Window trigger determines when to emit results
pub trait WindowTrigger: Send + Sync {
    /// Check if window should fire
    fn should_fire(
        &self,
        window: &Window,
        watermark: &Watermark,
        element_count: u64,
    ) -> bool;

    /// Check if window should be purged
    fn should_purge(
        &self,
        window: &Window,
        watermark: &Watermark,
    ) -> bool;
}

/// Event time trigger (fires when watermark passes window end)
pub struct EventTimeTrigger;

impl WindowTrigger for EventTimeTrigger {
    fn should_fire(
        &self,
        window: &Window,
        watermark: &Watermark,
        _element_count: u64,
    ) -> bool {
        watermark.timestamp >= window.end
    }

    fn should_purge(
        &self,
        window: &Window,
        watermark: &Watermark,
    ) -> bool {
        // Purge after allowed lateness
        watermark.timestamp >= window.end + chrono::Duration::hours(1)
    }
}

/// Processing time trigger (fires at wall-clock time)
pub struct ProcessingTimeTrigger {
    fire_interval: Duration,
}

/// Count trigger (fires after N elements)
pub struct CountTrigger {
    threshold: u64,
}

impl WindowTrigger for CountTrigger {
    fn should_fire(
        &self,
        _window: &Window,
        _watermark: &Watermark,
        element_count: u64,
    ) -> bool {
        element_count >= self.threshold
    }

    fn should_purge(
        &self,
        window: &Window,
        watermark: &Watermark,
    ) -> bool {
        watermark.timestamp >= window.end
    }
}
```

---

## Aggregation Engine

### Multi-Stage Aggregation

```rust
/// Aggregation engine coordinates multiple aggregators
pub struct AggregationEngine {
    state_manager: Arc<WindowStateManager<AggregationState>>,
    trigger: Arc<dyn WindowTrigger>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregationState {
    pub window: Window,
    pub count: CountAccumulator,
    pub average: AverageAccumulator,
    pub percentiles: PercentileAccumulator,
    pub min: f64,
    pub max: f64,
    pub sum: f64,
    pub sum_of_squares: f64, // For stddev
    pub element_count: u64,
}

impl AggregationState {
    pub fn new(window: Window) -> Self {
        Self {
            window,
            count: CountAccumulator::default(),
            average: AverageAccumulator::default(),
            percentiles: PercentileAccumulator::default(),
            min: f64::MAX,
            max: f64::MIN,
            sum: 0.0,
            sum_of_squares: 0.0,
            element_count: 0,
        }
    }

    pub fn add_value(&mut self, value: f64) {
        // Update all aggregators
        self.count.count += 1;
        self.average.sum += value;
        self.average.count += 1;
        self.percentiles.values.push(value);
        self.percentiles.sorted = false;
        self.min = self.min.min(value);
        self.max = self.max.max(value);
        self.sum += value;
        self.sum_of_squares += value * value;
        self.element_count += 1;
    }

    pub fn standard_deviation(&self) -> f64 {
        if self.element_count <= 1 {
            return 0.0;
        }

        let mean = self.sum / self.element_count as f64;
        let variance = (self.sum_of_squares / self.element_count as f64) - (mean * mean);
        variance.sqrt()
    }

    pub fn to_result(&mut self) -> AggregationResult {
        let mut results = HashMap::new();

        results.insert("count".to_string(), json!(self.count.count));
        results.insert("sum".to_string(), json!(self.sum));
        results.insert("average".to_string(), json!(self.average.average()));
        results.insert("min".to_string(), json!(self.min));
        results.insert("max".to_string(), json!(self.max));
        results.insert("stddev".to_string(), json!(self.standard_deviation()));

        if let Some(p50) = self.percentiles.p50() {
            results.insert("p50".to_string(), json!(p50));
        }
        if let Some(p95) = self.percentiles.p95() {
            results.insert("p95".to_string(), json!(p95));
        }
        if let Some(p99) = self.percentiles.p99() {
            results.insert("p99".to_string(), json!(p99));
        }

        AggregationResult {
            window: self.window.clone(),
            results,
            timestamp: Utc::now(),
        }
    }
}
```

---

## Watermarking and Late Events

### Late Event Handling Strategy

```rust
/// Late event handler
pub struct LateEventHandler {
    allowed_lateness: Duration,
    side_output: Option<Box<dyn EventSink<LateEvent>>>,
}

#[derive(Debug, Clone)]
pub struct LateEvent {
    pub event: ProcessorEvent<FeedbackEvent>,
    pub watermark: Watermark,
    pub lateness: Duration,
}

impl LateEventHandler {
    pub fn new(allowed_lateness: Duration) -> Self {
        Self {
            allowed_lateness,
            side_output: None,
        }
    }

    pub fn with_side_output(mut self, sink: impl EventSink<LateEvent> + 'static) -> Self {
        self.side_output = Some(Box::new(sink));
        self
    }

    pub async fn handle_event(
        &self,
        event: &ProcessorEvent<FeedbackEvent>,
        window: &Window,
        watermark: &Watermark,
    ) -> LateEventDecision {
        let event_time = event.timestamp.time();

        if watermark.timestamp <= window.end {
            // Not late
            return LateEventDecision::Accept;
        }

        let lateness = watermark.timestamp
            .signed_duration_since(event_time)
            .to_std()
            .unwrap_or(Duration::from_secs(0));

        if lateness <= self.allowed_lateness {
            // Within allowed lateness
            LateEventDecision::AcceptLate
        } else {
            // Too late, drop
            if let Some(sink) = &self.side_output {
                let late_event = LateEvent {
                    event: event.clone(),
                    watermark: *watermark,
                    lateness,
                };
                // Send to side output (async, fire and forget)
            }
            LateEventDecision::Drop
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum LateEventDecision {
    Accept,
    AcceptLate,
    Drop,
}
```

---

## State Management

### State Backends

```rust
/// In-memory state backend (for testing)
pub struct MemoryStateBackend {
    state: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
}

#[async_trait]
impl StateBackend for MemoryStateBackend {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let state = self.state.read().await;
        Ok(state.get(key).cloned())
    }

    async fn put(&self, key: &[u8], value: &[u8]) -> Result<()> {
        let mut state = self.state.write().await;
        state.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    async fn delete(&self, key: &[u8]) -> Result<()> {
        let mut state = self.state.write().await;
        state.remove(key);
        Ok(())
    }

    async fn checkpoint(&self, _checkpoint_id: u64) -> Result<()> {
        Ok(())
    }

    async fn restore(&self, _checkpoint_id: u64) -> Result<()> {
        Ok(())
    }

    async fn list_keys(&self, prefix: &[u8]) -> Result<Vec<Vec<u8>>> {
        let state = self.state.read().await;
        Ok(state
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect())
    }
}

/// RocksDB state backend (for production)
pub struct RocksDBStateBackend {
    db: Arc<rocksdb::DB>,
    checkpoint_path: PathBuf,
}

impl RocksDBStateBackend {
    pub fn new(path: impl AsRef<Path>, checkpoint_path: impl AsRef<Path>) -> Result<Self> {
        let mut opts = rocksdb::Options::default();
        opts.create_if_missing(true);
        opts.set_compression_type(rocksdb::DBCompressionType::Lz4);

        let db = rocksdb::DB::open(&opts, path)
            .map_err(|e| OptimizerError::StorageError(e.to_string()))?;

        Ok(Self {
            db: Arc::new(db),
            checkpoint_path: checkpoint_path.as_ref().to_path_buf(),
        })
    }
}

#[async_trait]
impl StateBackend for RocksDBStateBackend {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let db = self.db.clone();
        let key = key.to_vec();

        tokio::task::spawn_blocking(move || {
            db.get(&key)
                .map_err(|e| OptimizerError::StorageError(e.to_string()))
        })
        .await
        .map_err(|e| OptimizerError::InternalError(e.to_string()))?
    }

    async fn put(&self, key: &[u8], value: &[u8]) -> Result<()> {
        let db = self.db.clone();
        let key = key.to_vec();
        let value = value.to_vec();

        tokio::task::spawn_blocking(move || {
            db.put(&key, &value)
                .map_err(|e| OptimizerError::StorageError(e.to_string()))
        })
        .await
        .map_err(|e| OptimizerError::InternalError(e.to_string()))?
    }

    async fn delete(&self, key: &[u8]) -> Result<()> {
        let db = self.db.clone();
        let key = key.to_vec();

        tokio::task::spawn_blocking(move || {
            db.delete(&key)
                .map_err(|e| OptimizerError::StorageError(e.to_string()))
        })
        .await
        .map_err(|e| OptimizerError::InternalError(e.to_string()))?
    }

    async fn checkpoint(&self, checkpoint_id: u64) -> Result<()> {
        let checkpoint_path = self.checkpoint_path.join(format!("cp-{}", checkpoint_id));
        let db = self.db.clone();

        tokio::task::spawn_blocking(move || {
            let checkpoint = rocksdb::checkpoint::Checkpoint::new(&db)
                .map_err(|e| OptimizerError::StorageError(e.to_string()))?;

            checkpoint
                .create_checkpoint(&checkpoint_path)
                .map_err(|e| OptimizerError::StorageError(e.to_string()))
        })
        .await
        .map_err(|e| OptimizerError::InternalError(e.to_string()))?
    }

    async fn restore(&self, checkpoint_id: u64) -> Result<()> {
        // Restore logic would involve closing current DB and opening checkpoint
        Err(OptimizerError::NotImplemented("Restore not yet implemented".into()))
    }

    async fn list_keys(&self, prefix: &[u8]) -> Result<Vec<Vec<u8>>> {
        let db = self.db.clone();
        let prefix = prefix.to_vec();

        tokio::task::spawn_blocking(move || {
            let mut keys = Vec::new();
            let iter = db.prefix_iterator(&prefix);

            for item in iter {
                match item {
                    Ok((key, _)) => {
                        if key.starts_with(&prefix) {
                            keys.push(key.to_vec());
                        } else {
                            break;
                        }
                    }
                    Err(e) => {
                        return Err(OptimizerError::StorageError(e.to_string()));
                    }
                }
            }

            Ok(keys)
        })
        .await
        .map_err(|e| OptimizerError::InternalError(e.to_string()))?
    }
}
```

---

## Error Handling Strategy

### Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum ProcessorError {
    #[error("Window assignment error: {0}")]
    WindowAssignment(String),

    #[error("Aggregation error: {0}")]
    Aggregation(String),

    #[error("State error: {0}")]
    State(String),

    #[error("Watermark error: {0}")]
    Watermark(String),

    #[error("Kafka error: {0}")]
    Kafka(#[from] rdkafka::error::KafkaError),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type ProcessorResult<T> = std::result::Result<T, ProcessorError>;
```

### Error Recovery

```rust
/// Error handling policy
pub enum ErrorPolicy {
    /// Retry with exponential backoff
    Retry {
        max_attempts: u32,
        initial_delay: Duration,
        max_delay: Duration,
    },

    /// Skip and continue
    Skip,

    /// Send to dead letter queue
    DeadLetter,

    /// Fail and stop processing
    Fail,
}

/// Error handler
pub struct ErrorHandler {
    policy: ErrorPolicy,
    dead_letter_sink: Option<Box<dyn EventSink<FailedEvent>>>,
    metrics: Arc<ProcessorMetrics>,
}

#[derive(Debug, Clone)]
pub struct FailedEvent {
    pub event: ProcessorEvent<FeedbackEvent>,
    pub error: String,
    pub timestamp: DateTime<Utc>,
    pub retry_count: u32,
}

impl ErrorHandler {
    pub async fn handle_error(
        &self,
        event: ProcessorEvent<FeedbackEvent>,
        error: ProcessorError,
        retry_count: u32,
    ) -> ErrorHandlingDecision {
        match &self.policy {
            ErrorPolicy::Retry { max_attempts, initial_delay, max_delay } => {
                if retry_count < *max_attempts {
                    let delay = (*initial_delay * 2_u32.pow(retry_count))
                        .min(*max_delay);
                    ErrorHandlingDecision::Retry { delay }
                } else {
                    self.send_to_dead_letter(event, error, retry_count).await;
                    ErrorHandlingDecision::Drop
                }
            }
            ErrorPolicy::Skip => {
                self.metrics.record_skipped_event();
                ErrorHandlingDecision::Skip
            }
            ErrorPolicy::DeadLetter => {
                self.send_to_dead_letter(event, error, retry_count).await;
                ErrorHandlingDecision::Drop
            }
            ErrorPolicy::Fail => {
                ErrorHandlingDecision::Fail
            }
        }
    }

    async fn send_to_dead_letter(
        &self,
        event: ProcessorEvent<FeedbackEvent>,
        error: ProcessorError,
        retry_count: u32,
    ) {
        if let Some(sink) = &self.dead_letter_sink {
            let failed_event = FailedEvent {
                event,
                error: error.to_string(),
                timestamp: Utc::now(),
                retry_count,
            };

            if let Err(e) = sink.send(failed_event).await {
                tracing::error!("Failed to send to dead letter queue: {}", e);
            }
        }
    }
}

pub enum ErrorHandlingDecision {
    Retry { delay: Duration },
    Skip,
    Drop,
    Fail,
}
```

---

## Configuration Structure

```yaml
# Stream Processor Configuration

processor:
  # Number of parallel processing tasks
  parallelism: 4

  # Buffer size for event queue
  buffer_size: 10000

  # Checkpoint interval
  checkpoint_interval_secs: 60

  # State backend
  state:
    backend: "rocksdb"  # or "memory"
    path: "/var/lib/optimizer/state"
    checkpoint_path: "/var/lib/optimizer/checkpoints"

  # Window configuration
  windows:
    # Default window type
    default_type: "tumbling"

    # Tumbling window
    tumbling:
      size_secs: 300  # 5 minutes
      allowed_lateness_secs: 60

    # Sliding window
    sliding:
      size_secs: 600  # 10 minutes
      slide_secs: 60  # 1 minute
      allowed_lateness_secs: 60

    # Session window
    session:
      gap_secs: 300  # 5 minutes
      allowed_lateness_secs: 60

  # Watermark configuration
  watermark:
    strategy: "bounded_out_of_orderness"
    max_out_of_orderness_secs: 30
    emit_interval_secs: 10

  # Aggregation configuration
  aggregation:
    # Enabled aggregators
    enabled:
      - "count"
      - "sum"
      - "average"
      - "min"
      - "max"
      - "percentile"
      - "stddev"

    # Percentile configuration
    percentiles:
      - 50
      - 95
      - 99

  # Error handling
  error_handling:
    policy: "retry"  # or "skip", "dead_letter", "fail"
    max_retries: 3
    initial_retry_delay_ms: 100
    max_retry_delay_ms: 5000
    dead_letter_topic: "processor-dead-letter"

  # Kafka source
  kafka:
    # Consumer configuration
    consumer:
      bootstrap_servers: "localhost:9092"
      group_id: "llm-optimizer-processor"
      topics:
        - "feedback-events"
        - "performance-metrics"

      # Consumer settings
      enable_auto_commit: false
      auto_offset_reset: "earliest"
      session_timeout_ms: 30000
      max_poll_records: 500

    # Producer configuration (for results)
    producer:
      bootstrap_servers: "localhost:9092"
      result_topic: "aggregated-metrics"
      compression_type: "lz4"
      acks: "all"
      retries: 3

  # Metrics
  metrics:
    enabled: true
    export_interval_secs: 60
```

### Configuration Structures

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ProcessorConfig {
    pub parallelism: usize,
    pub buffer_size: usize,
    pub checkpoint_interval_secs: u64,
    pub state: StateConfig,
    pub windows: WindowConfig,
    pub watermark: WatermarkConfig,
    pub aggregation: AggregationConfig,
    pub error_handling: ErrorHandlingConfig,
    pub kafka: KafkaConfig,
    pub metrics: MetricsConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StateConfig {
    pub backend: StateBackendType,
    pub path: PathBuf,
    pub checkpoint_path: PathBuf,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StateBackendType {
    Memory,
    RocksDB,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WindowConfig {
    pub default_type: WindowType,
    pub tumbling: TumblingWindowConfig,
    pub sliding: SlidingWindowConfig,
    pub session: SessionWindowConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TumblingWindowConfig {
    pub size_secs: u64,
    pub allowed_lateness_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlidingWindowConfig {
    pub size_secs: u64,
    pub slide_secs: u64,
    pub allowed_lateness_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionWindowConfig {
    pub gap_secs: u64,
    pub allowed_lateness_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WatermarkConfig {
    pub strategy: WatermarkStrategy,
    pub max_out_of_orderness_secs: u64,
    pub emit_interval_secs: u64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatermarkStrategy {
    BoundedOutOfOrderness,
    Periodic,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AggregationConfig {
    pub enabled: Vec<String>,
    pub percentiles: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ErrorHandlingConfig {
    pub policy: String,
    pub max_retries: u32,
    pub initial_retry_delay_ms: u64,
    pub max_retry_delay_ms: u64,
    pub dead_letter_topic: Option<String>,
}
```

---

## API Design

### Public API

```rust
// lib.rs

pub use core::{ProcessorEvent, EventTimestamp, GroupingKey};
pub use window::{Window, WindowSpec, WindowType};
pub use aggregation::{AggregationResult, Aggregator};
pub use watermark::{Watermark, WatermarkGenerator};
pub use state::{StateBackend, WindowStateManager};
pub use pipeline::{StreamPipelineBuilder, StreamPipeline};
pub use error::{ProcessorError, ProcessorResult};
pub use config::ProcessorConfig;

/// Create a new stream pipeline builder
pub fn create_pipeline<T>() -> StreamPipelineBuilder<T>
where
    T: Send + Sync + Clone + 'static,
{
    StreamPipelineBuilder::new()
}

/// Load configuration from file
pub async fn load_config(path: impl AsRef<Path>) -> ProcessorResult<ProcessorConfig> {
    let content = tokio::fs::read_to_string(path).await
        .map_err(|e| ProcessorError::Config(e.to_string()))?;

    serde_yaml::from_str(&content)
        .map_err(|e| ProcessorError::Config(e.to_string()))
}
```

### Usage Example

```rust
use llm_optimizer_processor::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = load_config("processor-config.yaml").await?;

    // Create state backend
    let state_backend = Arc::new(
        RocksDBStateBackend::new(
            &config.state.path,
            &config.state.checkpoint_path,
        )?
    );

    // Build pipeline
    let pipeline = create_pipeline::<FeedbackEvent>()
        .with_kafka_source(config.kafka.clone())
        .with_watermark_generator(
            BoundedOutOfOrdernessWatermark::new(
                Duration::from_secs(config.watermark.max_out_of_orderness_secs)
            )
        )
        .with_tumbling_window(Duration::from_secs(config.windows.tumbling.size_secs))
        .with_state_backend(state_backend)
        .with_sink(KafkaAggregationSink::new(config.kafka.producer))
        .build()?;

    // Run pipeline
    pipeline.run().await?;

    Ok(())
}
```

---

## Performance Considerations

### Throughput Optimization

1. **Parallel Processing**: Multiple worker tasks process events concurrently
2. **Batching**: Group state updates and checkpoints
3. **Async I/O**: Non-blocking state backend operations
4. **Zero-Copy**: Minimize data copying in hot paths

### Memory Management

1. **Bounded Buffers**: Prevent unbounded memory growth
2. **State Eviction**: Clear old window state after expiration
3. **Compression**: Use LZ4 compression for state serialization
4. **Streaming Percentiles**: Use approximate algorithms (T-Digest) for large datasets

### Latency Optimization

1. **Fast Path**: Optimize common case (in-order events)
2. **Lock-Free Structures**: Use concurrent data structures where possible
3. **State Caching**: Cache frequently accessed state in memory
4. **Async Checkpointing**: Don't block processing during checkpoints

### Scalability

1. **Horizontal Scaling**: Partition by key across multiple instances
2. **State Sharding**: Distribute state across multiple backends
3. **Kafka Partitioning**: Leverage Kafka partitions for parallelism

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tumbling_window_assignment() {
        let assigner = TumblingWindowAssigner::new(Duration::from_secs(300));
        let timestamp = Utc.with_ymd_and_hms(2025, 1, 1, 12, 34, 56).unwrap();

        let windows = assigner.assign_windows(timestamp, &None);

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].duration(), Duration::from_secs(300));
    }

    #[test]
    fn test_sliding_window_assignment() {
        let assigner = SlidingWindowAssigner::new(
            Duration::from_secs(600),
            Duration::from_secs(60),
        );
        let timestamp = Utc.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap();

        let windows = assigner.assign_windows(timestamp, &None);

        assert!(windows.len() > 1);
    }

    #[tokio::test]
    async fn test_aggregation_state() {
        let mut state = AggregationState::new(Window {
            start: Utc::now(),
            end: Utc::now() + chrono::Duration::minutes(5),
            key: None,
        });

        state.add_value(10.0);
        state.add_value(20.0);
        state.add_value(30.0);

        assert_eq!(state.count.count, 3);
        assert_eq!(state.average.average(), 20.0);
        assert_eq!(state.min, 10.0);
        assert_eq!(state.max, 30.0);
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_end_to_end_processing() {
    // Setup
    let state_backend = Arc::new(MemoryStateBackend::new());

    let pipeline = create_pipeline()
        .with_kafka_source(test_kafka_config())
        .with_watermark_generator(BoundedOutOfOrdernessWatermark::new(
            Duration::from_secs(5)
        ))
        .with_tumbling_window(Duration::from_secs(60))
        .with_state_backend(state_backend)
        .with_sink(TestSink::new())
        .build()
        .unwrap();

    // Test event processing
    // Assertions on output
}
```

### Performance Benchmarks

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_window_assignment(c: &mut Criterion) {
    let assigner = TumblingWindowAssigner::new(Duration::from_secs(300));
    let timestamp = Utc::now();

    c.bench_function("tumbling_window_assignment", |b| {
        b.iter(|| {
            assigner.assign_windows(black_box(timestamp), &None)
        })
    });
}

fn benchmark_aggregation(c: &mut Criterion) {
    let mut state = AggregationState::new(Window {
        start: Utc::now(),
        end: Utc::now() + chrono::Duration::minutes(5),
        key: None,
    });

    c.bench_function("add_value_to_aggregation", |b| {
        b.iter(|| {
            state.add_value(black_box(42.0))
        })
    });
}

criterion_group!(benches, benchmark_window_assignment, benchmark_aggregation);
criterion_main!(benches);
```

---

## Summary

This architecture provides a comprehensive, enterprise-grade stream processing system for the LLM Auto-Optimizer with:

1. **Flexible Windowing**: Support for tumbling, sliding, and session windows
2. **Rich Aggregations**: Count, sum, average, min, max, percentiles, standard deviation
3. **Late Event Handling**: Watermarking with configurable allowed lateness
4. **Persistent State**: Multiple backend options (memory, RocksDB)
5. **High Performance**: 10,000+ events/sec throughput, <100ms p99 latency
6. **Production-Ready**: Comprehensive error handling, metrics, and recovery

The modular design allows for easy extension and customization while maintaining high performance and reliability.
