# Time-Series Normalization Architecture Specification

**Version:** 1.0
**Status:** Design
**Author:** Time-Series Normalization Architect
**Date:** 2025-11-10

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Architecture Overview](#architecture-overview)
3. [Core Components](#core-components)
4. [Normalization Strategies](#normalization-strategies)
5. [Fill Strategies](#fill-strategies)
6. [Timestamp Alignment](#timestamp-alignment)
7. [Buffer Management](#buffer-management)
8. [Integration Patterns](#integration-patterns)
9. [Configuration Schema](#configuration-schema)
10. [Error Handling](#error-handling)
11. [Performance Characteristics](#performance-characteristics)
12. [Metrics and Monitoring](#metrics-and-monitoring)
13. [Implementation Roadmap](#implementation-roadmap)

---

## Executive Summary

This document defines an enterprise-grade time-series normalization system for the Stream Processor that handles irregular sampling, out-of-order events, missing data, and ensures consistent aggregation intervals.

### Key Features

- **Multiple Normalization Strategies**: Alignment, resampling, interpolation
- **Configurable Intervals**: 1s, 5s, 1min, 5min, custom intervals
- **Advanced Fill Strategies**: Forward fill, backward fill, linear interpolation, zero fill, skip
- **Out-of-Order Handling**: Integrated with watermark system for late event handling
- **Memory-Efficient**: Windowed buffering with automatic cleanup
- **Performance**: <10ms overhead per event target
- **Production-Ready**: Comprehensive metrics, error handling, and observability

### Design Principles

1. **Composability**: Integrates seamlessly with existing window and aggregation systems
2. **Configurability**: Runtime-configurable strategies and intervals
3. **Performance**: Zero-copy where possible, minimal allocations
4. **Reliability**: Graceful degradation, comprehensive error handling
5. **Observability**: Rich metrics for monitoring and debugging

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                   Time-Series Normalization System                  │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    TimeSeriesNormalizer                      │  │
│  │  ┌──────────────┐  ┌───────────────┐  ┌─────────────────┐  │  │
│  │  │  Event       │  │  Timestamp    │  │   Fill          │  │  │
│  │  │  Buffer      │─▶│  Aligner      │─▶│   Strategy      │  │  │
│  │  │              │  │               │  │                 │  │  │
│  │  └──────────────┘  └───────────────┘  └─────────────────┘  │  │
│  │         ▲                  ▲                    │           │  │
│  │         │                  │                    ▼           │  │
│  │    ┌────┴─────┐   ┌───────┴──────┐   ┌─────────────────┐  │  │
│  │    │ Watermark│   │ Normalization│   │   Interpolator  │  │  │
│  │    │  Tracker │   │   Strategy   │   │                 │  │  │
│  │    └──────────┘   └──────────────┘   └─────────────────┘  │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                │                                    │
│                                ▼                                    │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    NormalizedEventStream                     │  │
│  │  • Aligned to interval boundaries                            │  │
│  │  • Missing data filled using strategy                        │  │
│  │  • Ordered by normalized timestamp                           │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                │                                    │
└────────────────────────────────┼────────────────────────────────────┘
                                 ▼
                    ┌────────────────────────┐
                    │   Window Assigner      │
                    │   & Aggregator         │
                    └────────────────────────┘
```

### Component Interaction Flow

```
Raw Event Stream
       │
       ▼
┌─────────────────┐
│ Event Buffering │◀────── Watermark
│   & Ordering    │        System
└─────────────────┘
       │
       ▼
┌─────────────────┐
│   Timestamp     │◀────── Normalization
│   Alignment     │        Config
└─────────────────┘
       │
       ▼
┌─────────────────┐
│ Gap Detection & │◀────── Fill Strategy
│   Data Filling  │        Config
└─────────────────┘
       │
       ▼
┌─────────────────┐
│  Interpolation  │◀────── Interpolation
│   & Resampling  │        Method
└─────────────────┘
       │
       ▼
Normalized Event Stream
```

---

## Core Components

### 1. TimeSeriesNormalizer

**Purpose**: Main orchestrator for time-series normalization

**Rust Implementation**:

```rust
use chrono::{DateTime, Duration, Utc};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Main time-series normalizer component
pub struct TimeSeriesNormalizer<T> {
    /// Configuration for normalization
    config: NormalizationConfig,

    /// Event buffer for handling out-of-order events
    event_buffer: Arc<RwLock<EventBuffer<T>>>,

    /// Timestamp alignment strategy
    aligner: Box<dyn TimestampAligner>,

    /// Fill strategy for missing data
    fill_strategy: Box<dyn FillStrategy<T>>,

    /// Interpolation method
    interpolator: Box<dyn Interpolator<T>>,

    /// Watermark tracker for late event handling
    watermark_tracker: Arc<RwLock<WatermarkTracker>>,

    /// Metrics collector
    metrics: Arc<NormalizationMetrics>,
}

impl<T> TimeSeriesNormalizer<T>
where
    T: Clone + Send + Sync + 'static,
{
    /// Create a new normalizer with the given configuration
    pub fn new(config: NormalizationConfig) -> Self {
        let aligner = create_aligner(&config.alignment_strategy);
        let fill_strategy = create_fill_strategy(&config.fill_strategy);
        let interpolator = create_interpolator(&config.interpolation_method);

        Self {
            config,
            event_buffer: Arc::new(RwLock::new(EventBuffer::new(
                config.buffer_capacity,
                config.buffer_retention,
            ))),
            aligner,
            fill_strategy,
            interpolator,
            watermark_tracker: Arc::new(RwLock::new(WatermarkTracker::new())),
            metrics: Arc::new(NormalizationMetrics::new()),
        }
    }

    /// Process a single event through normalization
    pub async fn process_event(
        &self,
        event: T,
        timestamp: DateTime<Utc>,
        key: &str,
    ) -> Result<Vec<NormalizedEvent<T>>, NormalizationError> {
        let start = std::time::Instant::now();

        // 1. Check if event is late based on watermark
        let is_late = self.watermark_tracker.read().await.is_late(timestamp);
        if is_late {
            self.metrics.record_late_event(timestamp);
            if !self.config.allow_late_events {
                return Err(NormalizationError::LateEvent { timestamp });
            }
        }

        // 2. Add event to buffer
        self.event_buffer
            .write()
            .await
            .insert(timestamp, key.to_string(), event.clone())?;

        // 3. Check if we can emit normalized events
        let watermark = self.watermark_tracker.read().await.current_watermark();
        let normalized = self.normalize_buffered_events(key, watermark).await?;

        // 4. Update metrics
        let duration = start.elapsed();
        self.metrics.record_processing_time(duration);
        self.metrics.record_events_emitted(normalized.len());

        Ok(normalized)
    }

    /// Normalize all buffered events up to the watermark
    async fn normalize_buffered_events(
        &self,
        key: &str,
        watermark: DateTime<Utc>,
    ) -> Result<Vec<NormalizedEvent<T>>, NormalizationError> {
        let mut buffer = self.event_buffer.write().await;

        // 1. Get events ready for normalization (before watermark)
        let ready_events = buffer.get_ready_events(key, watermark)?;

        if ready_events.is_empty() {
            return Ok(Vec::new());
        }

        // 2. Align timestamps to interval boundaries
        let aligned = self.align_timestamps(ready_events)?;

        // 3. Detect gaps and fill missing data
        let filled = self.fill_gaps(aligned)?;

        // 4. Apply interpolation if needed
        let interpolated = self.interpolate_if_needed(filled)?;

        // 5. Remove processed events from buffer
        buffer.remove_before(key, watermark)?;

        Ok(interpolated)
    }

    /// Align event timestamps to interval boundaries
    fn align_timestamps(
        &self,
        events: Vec<TimestampedEvent<T>>,
    ) -> Result<Vec<TimestampedEvent<T>>, NormalizationError> {
        events
            .into_iter()
            .map(|event| {
                let aligned_ts = self.aligner.align(event.timestamp);
                Ok(TimestampedEvent {
                    timestamp: aligned_ts,
                    original_timestamp: Some(event.timestamp),
                    key: event.key,
                    value: event.value,
                })
            })
            .collect()
    }

    /// Fill gaps in the time series
    fn fill_gaps(
        &self,
        events: Vec<TimestampedEvent<T>>,
    ) -> Result<Vec<TimestampedEvent<T>>, NormalizationError> {
        if events.is_empty() {
            return Ok(events);
        }

        let mut filled = Vec::new();
        let interval = self.config.normalization_interval;

        for i in 0..events.len() {
            filled.push(events[i].clone());

            // Check for gap to next event
            if i + 1 < events.len() {
                let current = events[i].timestamp;
                let next = events[i + 1].timestamp;
                let expected_next = current + interval;

                // Fill gap if it exists
                if next > expected_next {
                    let gap_fills = self.fill_strategy.fill_gap(
                        &events[i],
                        &events[i + 1],
                        interval,
                    )?;
                    filled.extend(gap_fills);
                }
            }
        }

        Ok(filled)
    }

    /// Apply interpolation to events if configured
    fn interpolate_if_needed(
        &self,
        events: Vec<TimestampedEvent<T>>,
    ) -> Result<Vec<NormalizedEvent<T>>, NormalizationError> {
        match self.config.interpolation_method {
            InterpolationMethod::None => {
                // Convert to NormalizedEvent without interpolation
                Ok(events
                    .into_iter()
                    .map(|e| NormalizedEvent::from_timestamped(e))
                    .collect())
            }
            _ => {
                // Apply interpolation
                self.interpolator.interpolate(events)
            }
        }
    }

    /// Update the watermark
    pub async fn update_watermark(&self, watermark: DateTime<Utc>) {
        self.watermark_tracker.write().await.update(watermark);
    }

    /// Get current metrics
    pub fn metrics(&self) -> Arc<NormalizationMetrics> {
        Arc::clone(&self.metrics)
    }

    /// Flush any remaining buffered events
    pub async fn flush(&self, key: &str) -> Result<Vec<NormalizedEvent<T>>, NormalizationError> {
        let watermark = DateTime::<Utc>::from_timestamp(i64::MAX, 0)
            .unwrap_or_else(|| Utc::now());
        self.normalize_buffered_events(key, watermark).await
    }
}

/// Configuration for time-series normalization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizationConfig {
    /// Interval for normalization (e.g., 1s, 5s, 1min)
    pub normalization_interval: Duration,

    /// Strategy for timestamp alignment
    pub alignment_strategy: AlignmentStrategy,

    /// Strategy for filling missing data
    pub fill_strategy: FillStrategyType,

    /// Interpolation method
    pub interpolation_method: InterpolationMethod,

    /// Maximum buffer size (number of events)
    pub buffer_capacity: usize,

    /// How long to retain events in buffer
    pub buffer_retention: Duration,

    /// Whether to allow late events
    pub allow_late_events: bool,

    /// Maximum lateness allowed
    pub max_lateness: Duration,
}

impl Default for NormalizationConfig {
    fn default() -> Self {
        Self {
            normalization_interval: Duration::seconds(5),
            alignment_strategy: AlignmentStrategy::RoundDown,
            fill_strategy: FillStrategyType::ForwardFill,
            interpolation_method: InterpolationMethod::None,
            buffer_capacity: 10000,
            buffer_retention: Duration::minutes(5),
            allow_late_events: true,
            max_lateness: Duration::seconds(60),
        }
    }
}
```

### 2. EventBuffer

**Purpose**: Memory-efficient buffering for out-of-order events

```rust
use std::collections::BTreeMap;
use chrono::{DateTime, Duration, Utc};

/// Event buffer for handling out-of-order events
pub struct EventBuffer<T> {
    /// Events organized by key -> timestamp -> events
    events: BTreeMap<String, BTreeMap<DateTime<Utc>, Vec<T>>>,

    /// Maximum capacity per key
    capacity_per_key: usize,

    /// Retention duration
    retention: Duration,

    /// Total event count
    total_count: usize,

    /// Oldest timestamp per key
    oldest_timestamps: BTreeMap<String, DateTime<Utc>>,
}

impl<T: Clone> EventBuffer<T> {
    pub fn new(capacity_per_key: usize, retention: Duration) -> Self {
        Self {
            events: BTreeMap::new(),
            capacity_per_key,
            retention,
            total_count: 0,
            oldest_timestamps: BTreeMap::new(),
        }
    }

    /// Insert an event into the buffer
    pub fn insert(
        &mut self,
        timestamp: DateTime<Utc>,
        key: String,
        event: T,
    ) -> Result<(), NormalizationError> {
        // Check retention
        let now = Utc::now();
        if now - timestamp > self.retention {
            return Err(NormalizationError::EventTooOld {
                timestamp,
                retention: self.retention
            });
        }

        // Get or create key bucket
        let key_events = self.events.entry(key.clone()).or_insert_with(BTreeMap::new);

        // Check capacity
        let key_count: usize = key_events.values().map(|v| v.len()).sum();
        if key_count >= self.capacity_per_key {
            // Remove oldest to make space
            if let Some((&oldest_ts, _)) = key_events.iter().next() {
                key_events.remove(&oldest_ts);
            }
        }

        // Insert event
        key_events.entry(timestamp).or_insert_with(Vec::new).push(event);
        self.total_count += 1;

        // Update oldest timestamp
        if let Some(&first_ts) = key_events.keys().next() {
            self.oldest_timestamps.insert(key, first_ts);
        }

        Ok(())
    }

    /// Get all events for a key that are ready for processing (before watermark)
    pub fn get_ready_events(
        &self,
        key: &str,
        watermark: DateTime<Utc>,
    ) -> Result<Vec<TimestampedEvent<T>>, NormalizationError> {
        let mut result = Vec::new();

        if let Some(key_events) = self.events.get(key) {
            for (&timestamp, events) in key_events.iter() {
                if timestamp <= watermark {
                    for event in events {
                        result.push(TimestampedEvent {
                            timestamp,
                            original_timestamp: None,
                            key: key.to_string(),
                            value: event.clone(),
                        });
                    }
                }
            }
        }

        // Sort by timestamp
        result.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        Ok(result)
    }

    /// Remove events before the given timestamp
    pub fn remove_before(
        &mut self,
        key: &str,
        timestamp: DateTime<Utc>,
    ) -> Result<(), NormalizationError> {
        if let Some(key_events) = self.events.get_mut(key) {
            let to_remove: Vec<_> = key_events
                .range(..timestamp)
                .map(|(&ts, _)| ts)
                .collect();

            for ts in to_remove {
                if let Some(events) = key_events.remove(&ts) {
                    self.total_count -= events.len();
                }
            }

            // Update oldest timestamp
            if let Some(&first_ts) = key_events.keys().next() {
                self.oldest_timestamps.insert(key.to_string(), first_ts);
            } else {
                self.oldest_timestamps.remove(key);
            }
        }

        Ok(())
    }

    /// Get buffer statistics
    pub fn stats(&self) -> BufferStats {
        BufferStats {
            total_events: self.total_count,
            unique_keys: self.events.len(),
            oldest_event: self.oldest_timestamps.values().min().copied(),
        }
    }

    /// Clear all events
    pub fn clear(&mut self) {
        self.events.clear();
        self.oldest_timestamps.clear();
        self.total_count = 0;
    }
}

#[derive(Debug, Clone)]
pub struct BufferStats {
    pub total_events: usize,
    pub unique_keys: usize,
    pub oldest_event: Option<DateTime<Utc>>,
}
```

### 3. WatermarkTracker

**Purpose**: Track watermarks and detect late events

```rust
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Watermark tracker for normalization
pub struct WatermarkTracker {
    /// Global watermark
    global_watermark: DateTime<Utc>,

    /// Per-partition watermarks
    partition_watermarks: HashMap<u32, DateTime<Utc>>,

    /// Late event count
    late_event_count: u64,
}

impl WatermarkTracker {
    pub fn new() -> Self {
        Self {
            global_watermark: DateTime::from_timestamp(0, 0)
                .unwrap_or_else(|| Utc::now()),
            partition_watermarks: HashMap::new(),
            late_event_count: 0,
        }
    }

    /// Update the global watermark
    pub fn update(&mut self, watermark: DateTime<Utc>) {
        if watermark > self.global_watermark {
            self.global_watermark = watermark;
        }
    }

    /// Update partition watermark
    pub fn update_partition(&mut self, partition: u32, watermark: DateTime<Utc>) {
        self.partition_watermarks
            .entry(partition)
            .and_modify(|w| {
                if watermark > *w {
                    *w = watermark;
                }
            })
            .or_insert(watermark);

        // Update global watermark (minimum of all partitions)
        if let Some(min_watermark) = self.partition_watermarks.values().min() {
            self.update(*min_watermark);
        }
    }

    /// Get current watermark
    pub fn current_watermark(&self) -> DateTime<Utc> {
        self.global_watermark
    }

    /// Check if an event is late
    pub fn is_late(&self, timestamp: DateTime<Utc>) -> bool {
        timestamp < self.global_watermark
    }

    /// Get late event count
    pub fn late_event_count(&self) -> u64 {
        self.late_event_count
    }
}

impl Default for WatermarkTracker {
    fn default() -> Self {
        Self::new()
    }
}
```

---

## Normalization Strategies

### 1. Alignment Strategies

```rust
/// Timestamp alignment strategies
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AlignmentStrategy {
    /// Round down to nearest interval boundary (floor)
    RoundDown,

    /// Round up to nearest interval boundary (ceiling)
    RoundUp,

    /// Round to nearest interval boundary
    RoundNearest,

    /// Align to specific offset from epoch
    OffsetBased { offset: i64 },

    /// No alignment (passthrough)
    None,
}

/// Trait for timestamp alignment
pub trait TimestampAligner: Send + Sync {
    /// Align a timestamp to interval boundary
    fn align(&self, timestamp: DateTime<Utc>) -> DateTime<Utc>;

    /// Get the alignment strategy
    fn strategy(&self) -> AlignmentStrategy;
}

/// Round-down alignment implementation
pub struct RoundDownAligner {
    interval: Duration,
}

impl RoundDownAligner {
    pub fn new(interval: Duration) -> Self {
        Self { interval }
    }
}

impl TimestampAligner for RoundDownAligner {
    fn align(&self, timestamp: DateTime<Utc>) -> DateTime<Utc> {
        let ts_millis = timestamp.timestamp_millis();
        let interval_millis = self.interval.num_milliseconds();

        let aligned_millis = (ts_millis / interval_millis) * interval_millis;

        DateTime::from_timestamp_millis(aligned_millis)
            .unwrap_or(timestamp)
    }

    fn strategy(&self) -> AlignmentStrategy {
        AlignmentStrategy::RoundDown
    }
}

/// Round-up alignment implementation
pub struct RoundUpAligner {
    interval: Duration,
}

impl RoundUpAligner {
    pub fn new(interval: Duration) -> Self {
        Self { interval }
    }
}

impl TimestampAligner for RoundUpAligner {
    fn align(&self, timestamp: DateTime<Utc>) -> DateTime<Utc> {
        let ts_millis = timestamp.timestamp_millis();
        let interval_millis = self.interval.num_milliseconds();

        let aligned_millis = ((ts_millis + interval_millis - 1) / interval_millis) * interval_millis;

        DateTime::from_timestamp_millis(aligned_millis)
            .unwrap_or(timestamp)
    }

    fn strategy(&self) -> AlignmentStrategy {
        AlignmentStrategy::RoundUp
    }
}

/// Round-nearest alignment implementation
pub struct RoundNearestAligner {
    interval: Duration,
}

impl RoundNearestAligner {
    pub fn new(interval: Duration) -> Self {
        Self { interval }
    }
}

impl TimestampAligner for RoundNearestAligner {
    fn align(&self, timestamp: DateTime<Utc>) -> DateTime<Utc> {
        let ts_millis = timestamp.timestamp_millis();
        let interval_millis = self.interval.num_milliseconds();

        let aligned_millis = ((ts_millis + interval_millis / 2) / interval_millis) * interval_millis;

        DateTime::from_timestamp_millis(aligned_millis)
            .unwrap_or(timestamp)
    }

    fn strategy(&self) -> AlignmentStrategy {
        AlignmentStrategy::RoundNearest
    }
}

/// Factory for creating aligners
pub fn create_aligner(
    strategy: &AlignmentStrategy,
    interval: Duration,
) -> Box<dyn TimestampAligner> {
    match strategy {
        AlignmentStrategy::RoundDown => Box::new(RoundDownAligner::new(interval)),
        AlignmentStrategy::RoundUp => Box::new(RoundUpAligner::new(interval)),
        AlignmentStrategy::RoundNearest => Box::new(RoundNearestAligner::new(interval)),
        AlignmentStrategy::OffsetBased { offset } => {
            Box::new(OffsetBasedAligner::new(interval, *offset))
        }
        AlignmentStrategy::None => Box::new(PassthroughAligner),
    }
}
```

---

## Fill Strategies

```rust
/// Fill strategy types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum FillStrategyType {
    /// Forward fill - use last known value
    ForwardFill,

    /// Backward fill - use next known value
    BackwardFill,

    /// Linear interpolation between points
    LinearInterpolate,

    /// Fill with zero
    Zero,

    /// Fill with null/none (skip)
    Skip,

    /// Fill with a constant value
    Constant,

    /// Fill with average of surrounding values
    Average,
}

/// Trait for filling missing data points
pub trait FillStrategy<T>: Send + Sync {
    /// Fill the gap between two events
    fn fill_gap(
        &self,
        before: &TimestampedEvent<T>,
        after: &TimestampedEvent<T>,
        interval: Duration,
    ) -> Result<Vec<TimestampedEvent<T>>, NormalizationError>;

    /// Get the strategy type
    fn strategy_type(&self) -> FillStrategyType;
}

/// Forward fill strategy implementation
pub struct ForwardFillStrategy;

impl<T: Clone> FillStrategy<T> for ForwardFillStrategy {
    fn fill_gap(
        &self,
        before: &TimestampedEvent<T>,
        after: &TimestampedEvent<T>,
        interval: Duration,
    ) -> Result<Vec<TimestampedEvent<T>>, NormalizationError> {
        let mut filled = Vec::new();
        let mut current = before.timestamp + interval;

        while current < after.timestamp {
            filled.push(TimestampedEvent {
                timestamp: current,
                original_timestamp: None,
                key: before.key.clone(),
                value: before.value.clone(), // Forward fill
            });
            current = current + interval;
        }

        Ok(filled)
    }

    fn strategy_type(&self) -> FillStrategyType {
        FillStrategyType::ForwardFill
    }
}

/// Backward fill strategy implementation
pub struct BackwardFillStrategy;

impl<T: Clone> FillStrategy<T> for BackwardFillStrategy {
    fn fill_gap(
        &self,
        before: &TimestampedEvent<T>,
        after: &TimestampedEvent<T>,
        interval: Duration,
    ) -> Result<Vec<TimestampedEvent<T>>, NormalizationError> {
        let mut filled = Vec::new();
        let mut current = before.timestamp + interval;

        while current < after.timestamp {
            filled.push(TimestampedEvent {
                timestamp: current,
                original_timestamp: None,
                key: before.key.clone(),
                value: after.value.clone(), // Backward fill
            });
            current = current + interval;
        }

        Ok(filled)
    }

    fn strategy_type(&self) -> FillStrategyType {
        FillStrategyType::BackwardFill
    }
}

/// Linear interpolation strategy for numeric types
pub struct LinearInterpolateStrategy;

impl FillStrategy<f64> for LinearInterpolateStrategy {
    fn fill_gap(
        &self,
        before: &TimestampedEvent<f64>,
        after: &TimestampedEvent<f64>,
        interval: Duration,
    ) -> Result<Vec<TimestampedEvent<f64>>, NormalizationError> {
        let mut filled = Vec::new();

        let time_diff = (after.timestamp - before.timestamp).num_milliseconds() as f64;
        let value_diff = after.value - before.value;

        let mut current = before.timestamp + interval;

        while current < after.timestamp {
            let elapsed = (current - before.timestamp).num_milliseconds() as f64;
            let ratio = elapsed / time_diff;
            let interpolated_value = before.value + (value_diff * ratio);

            filled.push(TimestampedEvent {
                timestamp: current,
                original_timestamp: None,
                key: before.key.clone(),
                value: interpolated_value,
            });

            current = current + interval;
        }

        Ok(filled)
    }

    fn strategy_type(&self) -> FillStrategyType {
        FillStrategyType::LinearInterpolate
    }
}

/// Zero fill strategy
pub struct ZeroFillStrategy<T: Default + Clone> {
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Default + Clone> ZeroFillStrategy<T> {
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T: Default + Clone + Send + Sync> FillStrategy<T> for ZeroFillStrategy<T> {
    fn fill_gap(
        &self,
        before: &TimestampedEvent<T>,
        after: &TimestampedEvent<T>,
        interval: Duration,
    ) -> Result<Vec<TimestampedEvent<T>>, NormalizationError> {
        let mut filled = Vec::new();
        let mut current = before.timestamp + interval;

        while current < after.timestamp {
            filled.push(TimestampedEvent {
                timestamp: current,
                original_timestamp: None,
                key: before.key.clone(),
                value: T::default(), // Zero/default value
            });
            current = current + interval;
        }

        Ok(filled)
    }

    fn strategy_type(&self) -> FillStrategyType {
        FillStrategyType::Zero
    }
}

/// Skip strategy (no fill)
pub struct SkipFillStrategy;

impl<T> FillStrategy<T> for SkipFillStrategy {
    fn fill_gap(
        &self,
        _before: &TimestampedEvent<T>,
        _after: &TimestampedEvent<T>,
        _interval: Duration,
    ) -> Result<Vec<TimestampedEvent<T>>, NormalizationError> {
        Ok(Vec::new()) // Don't fill gaps
    }

    fn strategy_type(&self) -> FillStrategyType {
        FillStrategyType::Skip
    }
}
```

---

## Timestamp Alignment

### Alignment Algorithm

```pseudocode
ALGORITHM: TimestampAlignment
INPUT: timestamp, interval, strategy
OUTPUT: aligned_timestamp

CASE strategy:
  ROUND_DOWN:
    aligned = (timestamp / interval) * interval

  ROUND_UP:
    aligned = ((timestamp + interval - 1) / interval) * interval

  ROUND_NEAREST:
    aligned = ((timestamp + interval/2) / interval) * interval

  OFFSET_BASED:
    aligned = ((timestamp - offset) / interval) * interval + offset

  NONE:
    aligned = timestamp

RETURN aligned
```

### Edge Cases

1. **Boundary Alignment**: Events exactly on boundaries
2. **Negative Timestamps**: Historical data with negative epoch times
3. **Timezone Handling**: Convert all to UTC before alignment
4. **Leap Seconds**: Use monotonic time for intervals

---

## Buffer Management

### Memory-Efficient Windowed Buffering

```rust
/// Buffer configuration
#[derive(Debug, Clone)]
pub struct BufferConfig {
    /// Maximum events per key
    pub capacity_per_key: usize,

    /// Maximum total events
    pub total_capacity: usize,

    /// Retention duration
    pub retention: Duration,

    /// Cleanup interval
    pub cleanup_interval: Duration,

    /// Eviction strategy
    pub eviction_strategy: EvictionStrategy,
}

#[derive(Debug, Clone, Copy)]
pub enum EvictionStrategy {
    /// Evict oldest events first (FIFO)
    Oldest,

    /// Evict least recently used (LRU)
    LeastRecentlyUsed,

    /// Evict based on size
    LargestFirst,
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            capacity_per_key: 10000,
            total_capacity: 100000,
            retention: Duration::minutes(5),
            cleanup_interval: Duration::seconds(30),
            eviction_strategy: EvictionStrategy::Oldest,
        }
    }
}
```

### Automatic Cleanup

```rust
impl<T> EventBuffer<T> {
    /// Start automatic cleanup task
    pub fn start_cleanup_task(
        buffer: Arc<RwLock<Self>>,
        interval: Duration,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(
                interval.to_std().unwrap_or(std::time::Duration::from_secs(30))
            );

            loop {
                interval_timer.tick().await;

                let mut buf = buffer.write().await;
                buf.cleanup_expired();
            }
        })
    }

    /// Clean up expired events
    fn cleanup_expired(&mut self) {
        let now = Utc::now();
        let cutoff = now - self.retention;

        for (key, events) in self.events.iter_mut() {
            let to_remove: Vec<_> = events
                .range(..cutoff)
                .map(|(&ts, _)| ts)
                .collect();

            for ts in to_remove {
                if let Some(removed) = events.remove(&ts) {
                    self.total_count -= removed.len();
                }
            }
        }

        // Remove empty keys
        self.events.retain(|_, events| !events.is_empty());
        self.oldest_timestamps.retain(|k, _| self.events.contains_key(k));
    }
}
```

---

## Integration Patterns

### Integration with Stream Processor

```rust
use processor::{
    StreamProcessor, StreamProcessorBuilder,
    WindowAssigner, TumblingWindowAssigner,
};

/// Create a stream processor with normalization
pub fn create_normalized_processor<T>(
    normalizer_config: NormalizationConfig,
    window_size: Duration,
) -> StreamProcessor<T>
where
    T: Clone + Send + Sync + 'static,
{
    let normalizer = TimeSeriesNormalizer::new(normalizer_config);

    StreamProcessorBuilder::new()
        // Pre-processing: Normalize events
        .with_preprocessor(move |event, timestamp, key| {
            normalizer.process_event(event, timestamp, key).await
        })
        // Window assignment on normalized events
        .with_window_assigner(TumblingWindowAssigner::new(window_size))
        // Aggregation on normalized windows
        .with_aggregator(/* ... */)
        .build()
}
```

### Integration with Watermark System

```rust
impl<T> TimeSeriesNormalizer<T> {
    /// Connect to existing watermark generator
    pub fn connect_watermark_generator(
        &self,
        watermark_rx: tokio::sync::broadcast::Receiver<Watermark>,
    ) -> tokio::task::JoinHandle<()> {
        let tracker = Arc::clone(&self.watermark_tracker);

        tokio::spawn(async move {
            let mut rx = watermark_rx;

            while let Ok(watermark) = rx.recv().await {
                tracker.write().await.update(watermark.to_datetime());
            }
        })
    }
}
```

### Pipeline Integration

```rust
use processor::pipeline::{StreamPipelineBuilder, StreamOperator};

/// Normalization operator for pipelines
pub struct NormalizationOperator<T> {
    normalizer: Arc<TimeSeriesNormalizer<T>>,
}

impl<T: Clone + Send + Sync + 'static> StreamOperator for NormalizationOperator<T> {
    type Input = T;
    type Output = Vec<NormalizedEvent<T>>;

    async fn process(&self, input: Self::Input) -> Result<Self::Output, ProcessorError> {
        // Extract timestamp and key from input
        let timestamp = extract_timestamp(&input)?;
        let key = extract_key(&input)?;

        // Normalize
        self.normalizer
            .process_event(input, timestamp, &key)
            .await
            .map_err(|e| ProcessorError::Custom(e.to_string()))
    }
}

// Usage in pipeline
let pipeline = StreamPipelineBuilder::new()
    .add_operator(NormalizationOperator::new(normalizer_config))
    .add_operator(WindowOperator::new(window_config))
    .add_operator(AggregateOperator::new(agg_config))
    .build();
```

---

## Configuration Schema

### YAML Configuration

```yaml
normalization:
  # Normalization interval (ISO 8601 duration format)
  interval: PT5S  # 5 seconds

  # Timestamp alignment strategy
  alignment:
    strategy: round_down  # round_down, round_up, round_nearest, offset_based, none
    offset: 0  # For offset_based strategy (milliseconds)

  # Fill strategy for missing data
  fill_strategy:
    type: forward_fill  # forward_fill, backward_fill, linear_interpolate, zero, skip, constant, average
    constant_value: 0.0  # For constant strategy

  # Interpolation method
  interpolation:
    method: none  # none, linear, polynomial, spline
    order: 2  # For polynomial interpolation

  # Buffer configuration
  buffer:
    capacity_per_key: 10000
    total_capacity: 100000
    retention: PT5M  # 5 minutes
    cleanup_interval: PT30S  # 30 seconds
    eviction_strategy: oldest  # oldest, lru, largest_first

  # Late event handling
  late_events:
    allow: true
    max_lateness: PT1M  # 1 minute
    drop_beyond_lateness: true

  # Performance tuning
  performance:
    batch_size: 100
    parallelism: 4
    enable_zero_copy: true
```

### Rust Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizationConfig {
    pub interval: Duration,
    pub alignment: AlignmentConfig,
    pub fill_strategy: FillStrategyConfig,
    pub interpolation: InterpolationConfig,
    pub buffer: BufferConfig,
    pub late_events: LateEventConfig,
    pub performance: PerformanceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentConfig {
    pub strategy: AlignmentStrategy,
    pub offset: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FillStrategyConfig {
    #[serde(rename = "type")]
    pub fill_type: FillStrategyType,
    pub constant_value: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterpolationConfig {
    pub method: InterpolationMethod,
    pub order: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LateEventConfig {
    pub allow: bool,
    pub max_lateness: Duration,
    pub drop_beyond_lateness: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub batch_size: usize,
    pub parallelism: usize,
    pub enable_zero_copy: bool,
}
```

---

## Error Handling

### Error Types

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum NormalizationError {
    #[error("Late event: timestamp {timestamp} is before watermark")]
    LateEvent { timestamp: DateTime<Utc> },

    #[error("Event too old: timestamp {timestamp}, retention {retention:?}")]
    EventTooOld {
        timestamp: DateTime<Utc>,
        retention: Duration,
    },

    #[error("Buffer capacity exceeded: current {current}, max {max}")]
    BufferCapacityExceeded { current: usize, max: usize },

    #[error("Invalid interval: {interval:?}")]
    InvalidInterval { interval: Duration },

    #[error("Gap filling failed: {reason}")]
    GapFillingFailed { reason: String },

    #[error("Interpolation failed: {reason}")]
    InterpolationFailed { reason: String },

    #[error("Alignment failed: {reason}")]
    AlignmentFailed { reason: String },

    #[error("Configuration error: {message}")]
    ConfigError { message: String },

    #[error("State error: {0}")]
    StateError(#[from] crate::error::StateError),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type NormalizationResult<T> = Result<T, NormalizationError>;
```

### Error Recovery Strategy

```rust
impl<T> TimeSeriesNormalizer<T> {
    /// Process event with error recovery
    pub async fn process_event_with_recovery(
        &self,
        event: T,
        timestamp: DateTime<Utc>,
        key: &str,
    ) -> NormalizationResult<Vec<NormalizedEvent<T>>> {
        match self.process_event(event.clone(), timestamp, key).await {
            Ok(result) => Ok(result),
            Err(NormalizationError::LateEvent { .. }) => {
                // Handle late events
                self.metrics.record_late_event(timestamp);

                if self.config.late_events.allow {
                    // Try to process with relaxed constraints
                    self.process_late_event(event, timestamp, key).await
                } else {
                    Err(NormalizationError::LateEvent { timestamp })
                }
            }
            Err(NormalizationError::BufferCapacityExceeded { .. }) => {
                // Trigger cleanup and retry
                self.event_buffer.write().await.cleanup_expired();
                self.process_event(event, timestamp, key).await
            }
            Err(e) => {
                // Log and propagate other errors
                tracing::error!(error = ?e, "Normalization error");
                Err(e)
            }
        }
    }
}
```

---

## Performance Characteristics

### Performance Targets

| Metric | Target | Measurement Method |
|--------|--------|-------------------|
| Processing Latency (p50) | <5ms | Per-event processing time |
| Processing Latency (p99) | <10ms | Per-event processing time |
| Throughput | >100,000 events/sec | Single-threaded |
| Memory per Key | <1KB | Excluding buffered events |
| Buffer Overhead | <10MB per 10K events | Total buffer memory |
| Alignment Overhead | <100ns | Per timestamp alignment |
| Fill Overhead | <1ms | Per gap (up to 100 points) |

### Performance Optimization Techniques

```rust
/// Zero-copy optimization for alignment
impl RoundDownAligner {
    #[inline(always)]
    pub fn align_fast(&self, timestamp_millis: i64) -> i64 {
        let interval_millis = self.interval.num_milliseconds();
        (timestamp_millis / interval_millis) * interval_millis
    }
}

/// Batch processing for efficiency
impl<T> TimeSeriesNormalizer<T> {
    pub async fn process_batch(
        &self,
        events: Vec<(T, DateTime<Utc>, String)>,
    ) -> NormalizationResult<Vec<NormalizedEvent<T>>> {
        let batch_size = self.config.performance.batch_size;
        let mut results = Vec::with_capacity(events.len());

        for chunk in events.chunks(batch_size) {
            // Process chunk in parallel if configured
            let chunk_results = if self.config.performance.parallelism > 1 {
                self.process_chunk_parallel(chunk).await?
            } else {
                self.process_chunk_sequential(chunk).await?
            };

            results.extend(chunk_results);
        }

        Ok(results)
    }
}

/// Memory pool for event allocation
pub struct EventPool<T> {
    pool: Arc<Mutex<Vec<Box<T>>>>,
    capacity: usize,
}

impl<T: Default> EventPool<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            pool: Arc::new(Mutex::new(Vec::with_capacity(capacity))),
            capacity,
        }
    }

    pub fn acquire(&self) -> Box<T> {
        self.pool.lock().unwrap().pop().unwrap_or_else(|| Box::new(T::default()))
    }

    pub fn release(&self, item: Box<T>) {
        let mut pool = self.pool.lock().unwrap();
        if pool.len() < self.capacity {
            pool.push(item);
        }
    }
}
```

### Benchmarking

```rust
#[cfg(test)]
mod benchmarks {
    use super::*;
    use criterion::{black_box, Criterion};

    pub fn bench_alignment(c: &mut Criterion) {
        let aligner = RoundDownAligner::new(Duration::seconds(5));
        let timestamp = Utc::now();

        c.bench_function("timestamp_alignment", |b| {
            b.iter(|| {
                black_box(aligner.align(black_box(timestamp)))
            })
        });
    }

    pub fn bench_forward_fill(c: &mut Criterion) {
        let strategy = ForwardFillStrategy;
        let before = TimestampedEvent {
            timestamp: Utc::now(),
            original_timestamp: None,
            key: "test".to_string(),
            value: 100.0,
        };
        let after = TimestampedEvent {
            timestamp: Utc::now() + Duration::seconds(60),
            original_timestamp: None,
            key: "test".to_string(),
            value: 200.0,
        };

        c.bench_function("forward_fill_60s", |b| {
            b.iter(|| {
                black_box(strategy.fill_gap(
                    black_box(&before),
                    black_box(&after),
                    black_box(Duration::seconds(5)),
                ))
            })
        });
    }
}
```

---

## Metrics and Monitoring

### Metrics Collection

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration as StdDuration, Instant};

/// Metrics for normalization operations
pub struct NormalizationMetrics {
    // Counters
    events_processed: AtomicU64,
    events_aligned: AtomicU64,
    events_filled: AtomicU64,
    events_interpolated: AtomicU64,
    late_events: AtomicU64,
    dropped_events: AtomicU64,
    errors: AtomicU64,

    // Timing
    total_processing_time_ns: AtomicU64,
    alignment_time_ns: AtomicU64,
    fill_time_ns: AtomicU64,
    interpolation_time_ns: AtomicU64,

    // Buffer stats
    buffer_size: AtomicU64,
    buffer_capacity_violations: AtomicU64,

    // Gap stats
    gaps_detected: AtomicU64,
    total_gap_duration_ms: AtomicU64,
}

impl NormalizationMetrics {
    pub fn new() -> Self {
        Self {
            events_processed: AtomicU64::new(0),
            events_aligned: AtomicU64::new(0),
            events_filled: AtomicU64::new(0),
            events_interpolated: AtomicU64::new(0),
            late_events: AtomicU64::new(0),
            dropped_events: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            total_processing_time_ns: AtomicU64::new(0),
            alignment_time_ns: AtomicU64::new(0),
            fill_time_ns: AtomicU64::new(0),
            interpolation_time_ns: AtomicU64::new(0),
            buffer_size: AtomicU64::new(0),
            buffer_capacity_violations: AtomicU64::new(0),
            gaps_detected: AtomicU64::new(0),
            total_gap_duration_ms: AtomicU64::new(0),
        }
    }

    // Increment counters
    pub fn record_event_processed(&self) {
        self.events_processed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_late_event(&self, _timestamp: DateTime<Utc>) {
        self.late_events.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_events_emitted(&self, count: usize) {
        self.events_aligned.fetch_add(count as u64, Ordering::Relaxed);
    }

    // Timing
    pub fn record_processing_time(&self, duration: StdDuration) {
        self.total_processing_time_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }

    // Get snapshot
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            events_processed: self.events_processed.load(Ordering::Relaxed),
            events_aligned: self.events_aligned.load(Ordering::Relaxed),
            events_filled: self.events_filled.load(Ordering::Relaxed),
            events_interpolated: self.events_interpolated.load(Ordering::Relaxed),
            late_events: self.late_events.load(Ordering::Relaxed),
            dropped_events: self.dropped_events.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            avg_processing_time_ns: self.avg_processing_time_ns(),
            buffer_size: self.buffer_size.load(Ordering::Relaxed),
            gaps_detected: self.gaps_detected.load(Ordering::Relaxed),
        }
    }

    fn avg_processing_time_ns(&self) -> u64 {
        let total = self.total_processing_time_ns.load(Ordering::Relaxed);
        let count = self.events_processed.load(Ordering::Relaxed);
        if count > 0 {
            total / count
        } else {
            0
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub events_processed: u64,
    pub events_aligned: u64,
    pub events_filled: u64,
    pub events_interpolated: u64,
    pub late_events: u64,
    pub dropped_events: u64,
    pub errors: u64,
    pub avg_processing_time_ns: u64,
    pub buffer_size: u64,
    pub gaps_detected: u64,
}
```

### Prometheus Integration

```rust
use prometheus::{IntCounter, IntGauge, Histogram, Registry};

pub struct PrometheusMetrics {
    events_processed: IntCounter,
    events_aligned: IntCounter,
    late_events: IntCounter,
    processing_time: Histogram,
    buffer_size: IntGauge,
    gaps_detected: IntCounter,
}

impl PrometheusMetrics {
    pub fn new(registry: &Registry) -> Self {
        let events_processed = IntCounter::new(
            "normalization_events_processed_total",
            "Total number of events processed",
        ).unwrap();

        let events_aligned = IntCounter::new(
            "normalization_events_aligned_total",
            "Total number of events aligned",
        ).unwrap();

        let late_events = IntCounter::new(
            "normalization_late_events_total",
            "Total number of late events",
        ).unwrap();

        let processing_time = Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "normalization_processing_duration_seconds",
                "Time spent processing events",
            )
            .buckets(vec![0.001, 0.005, 0.010, 0.025, 0.050, 0.100]),
        ).unwrap();

        let buffer_size = IntGauge::new(
            "normalization_buffer_size",
            "Current buffer size",
        ).unwrap();

        let gaps_detected = IntCounter::new(
            "normalization_gaps_detected_total",
            "Total number of gaps detected",
        ).unwrap();

        registry.register(Box::new(events_processed.clone())).unwrap();
        registry.register(Box::new(events_aligned.clone())).unwrap();
        registry.register(Box::new(late_events.clone())).unwrap();
        registry.register(Box::new(processing_time.clone())).unwrap();
        registry.register(Box::new(buffer_size.clone())).unwrap();
        registry.register(Box::new(gaps_detected.clone())).unwrap();

        Self {
            events_processed,
            events_aligned,
            late_events,
            processing_time,
            buffer_size,
            gaps_detected,
        }
    }

    pub fn record_event_processed(&self) {
        self.events_processed.inc();
    }

    pub fn record_processing_time(&self, duration: StdDuration) {
        self.processing_time.observe(duration.as_secs_f64());
    }
}
```

### Logging and Observability

```rust
use tracing::{info, warn, error, debug, trace, span, Level};

impl<T> TimeSeriesNormalizer<T> {
    /// Process event with structured logging
    pub async fn process_event_with_logging(
        &self,
        event: T,
        timestamp: DateTime<Utc>,
        key: &str,
    ) -> NormalizationResult<Vec<NormalizedEvent<T>>> {
        let span = span!(
            Level::DEBUG,
            "normalize_event",
            key = %key,
            timestamp = %timestamp
        );
        let _enter = span.enter();

        trace!("Starting event normalization");

        let result = self.process_event(event, timestamp, key).await;

        match &result {
            Ok(normalized) => {
                debug!(
                    events_emitted = normalized.len(),
                    "Event normalization completed"
                );
            }
            Err(e) => {
                error!(error = ?e, "Event normalization failed");
            }
        }

        result
    }
}
```

---

## Implementation Roadmap

### Phase 1: Core Infrastructure (Week 1-2)

- [ ] Implement `TimeSeriesNormalizer` core structure
- [ ] Implement `EventBuffer` with buffering logic
- [ ] Implement `WatermarkTracker` integration
- [ ] Unit tests for core components

### Phase 2: Alignment Strategies (Week 2-3)

- [ ] Implement `RoundDownAligner`
- [ ] Implement `RoundUpAligner`
- [ ] Implement `RoundNearestAligner`
- [ ] Implement `OffsetBasedAligner`
- [ ] Alignment strategy tests

### Phase 3: Fill Strategies (Week 3-4)

- [ ] Implement `ForwardFillStrategy`
- [ ] Implement `BackwardFillStrategy`
- [ ] Implement `LinearInterpolateStrategy`
- [ ] Implement `ZeroFillStrategy`
- [ ] Implement `SkipFillStrategy`
- [ ] Fill strategy tests

### Phase 4: Integration (Week 4-5)

- [ ] Stream processor integration
- [ ] Watermark system integration
- [ ] Pipeline operator implementation
- [ ] Integration tests

### Phase 5: Performance & Optimization (Week 5-6)

- [ ] Performance benchmarks
- [ ] Memory optimization
- [ ] Zero-copy optimizations
- [ ] Parallel processing support

### Phase 6: Metrics & Observability (Week 6-7)

- [ ] Metrics collection implementation
- [ ] Prometheus integration
- [ ] Structured logging
- [ ] Monitoring dashboards

### Phase 7: Production Readiness (Week 7-8)

- [ ] Error handling enhancements
- [ ] Configuration validation
- [ ] Documentation
- [ ] Examples and tutorials
- [ ] Load testing

---

## Appendix

### A. Common Use Cases

#### Use Case 1: Metric Aggregation

```rust
// Configure normalizer for 5-second metric aggregation
let config = NormalizationConfig {
    normalization_interval: Duration::seconds(5),
    alignment_strategy: AlignmentStrategy::RoundDown,
    fill_strategy: FillStrategyType::LinearInterpolate,
    interpolation_method: InterpolationMethod::Linear,
    ..Default::default()
};

let normalizer = TimeSeriesNormalizer::<f64>::new(config);
```

#### Use Case 2: Log Event Normalization

```rust
// Configure normalizer for 1-minute log event bucketing
let config = NormalizationConfig {
    normalization_interval: Duration::minutes(1),
    alignment_strategy: AlignmentStrategy::RoundDown,
    fill_strategy: FillStrategyType::Zero,
    interpolation_method: InterpolationMethod::None,
    ..Default::default()
};

let normalizer = TimeSeriesNormalizer::<LogCount>::new(config);
```

#### Use Case 3: Sensor Data Resampling

```rust
// Configure normalizer for 10-second sensor readings
let config = NormalizationConfig {
    normalization_interval: Duration::seconds(10),
    alignment_strategy: AlignmentStrategy::RoundNearest,
    fill_strategy: FillStrategyType::ForwardFill,
    interpolation_method: InterpolationMethod::None,
    buffer_retention: Duration::minutes(10),
    ..Default::default()
};

let normalizer = TimeSeriesNormalizer::<SensorReading>::new(config);
```

### B. Performance Tuning Guide

| Scenario | Recommendation | Rationale |
|----------|---------------|-----------|
| High throughput | Increase `batch_size`, enable parallelism | Amortize processing overhead |
| Low latency | Decrease `buffer_retention`, use `RoundDown` | Minimize buffering time |
| Memory constrained | Reduce `capacity_per_key`, aggressive cleanup | Limit memory footprint |
| Irregular sampling | Use `LinearInterpolate`, longer retention | Better handle gaps |
| Out-of-order heavy | Larger buffer, longer retention | Accommodate reordering |

### C. Testing Strategy

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_normalization() {
        let config = NormalizationConfig::default();
        let normalizer = TimeSeriesNormalizer::<f64>::new(config);

        // Process events
        let events = vec![
            (100.0, Utc::now(), "key1"),
            (110.0, Utc::now() + Duration::seconds(5), "key1"),
            (120.0, Utc::now() + Duration::seconds(10), "key1"),
        ];

        for (value, ts, key) in events {
            let normalized = normalizer.process_event(value, ts, key).await.unwrap();
            assert!(!normalized.is_empty());
        }
    }

    #[tokio::test]
    async fn test_gap_filling() {
        // Test gap detection and filling
        // ...
    }

    #[tokio::test]
    async fn test_late_event_handling() {
        // Test late event scenarios
        // ...
    }
}
```

---

## Conclusion

This time-series normalization architecture provides an enterprise-grade, production-ready solution for handling irregular time-series data in the Stream Processor. The design emphasizes:

- **Performance**: <10ms overhead per event
- **Flexibility**: Multiple strategies and configurations
- **Reliability**: Comprehensive error handling and recovery
- **Observability**: Rich metrics and logging
- **Integration**: Seamless integration with existing systems

The modular design allows for easy extension and customization while maintaining high performance and reliability standards required for production deployment.
