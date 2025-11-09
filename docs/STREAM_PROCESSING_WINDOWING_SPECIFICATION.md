# Stream Processing Windowing Specification for LLM Metrics

**Version:** 1.0
**Date:** 2025-11-09
**Status:** Technical Specification
**Owner:** LLM DevOps Team

## Executive Summary

This document specifies the stream processing windowing architecture for the LLM Auto-Optimizer, designed to handle real-time aggregation of LLM performance metrics, user feedback, and system events. The specification draws from production-proven patterns in Apache Flink, Kafka Streams, and Spark Streaming, adapted for Rust and the unique characteristics of LLM workloads.

## Table of Contents

1. [Windowing Strategies](#1-windowing-strategies)
2. [Watermarking Algorithms](#2-watermarking-algorithms)
3. [State Management](#3-state-management)
4. [Memory Management](#4-memory-management)
5. [Trigger Policies](#5-trigger-policies)
6. [LLM-Specific Recommendations](#6-llm-specific-recommendations)
7. [Implementation Roadmap](#7-implementation-roadmap)

---

## 1. Windowing Strategies

### 1.1 Industry Best Practices

#### Apache Flink Windowing Model
Flink provides the most flexible windowing semantics:
- **Tumbling Windows**: Fixed-size, non-overlapping windows (e.g., every 5 minutes)
- **Sliding Windows**: Fixed-size, overlapping windows (e.g., 15-minute window, sliding every 5 minutes)
- **Session Windows**: Dynamic windows based on inactivity gaps
- **Global Windows**: Custom-triggered windows for complex patterns

**Key Insight**: Flink separates window assignment from triggering, allowing complex event-time semantics with late data handling.

#### Kafka Streams Windowing
Kafka Streams optimizes for changelog-based state management:
- **Hopping Windows**: Similar to sliding windows but more efficient
- **Time-based Windows**: Aligned to wall-clock or event time
- **Grace Period**: Built-in support for late-arriving data

**Key Insight**: Kafka Streams uses RocksDB for state storage, enabling large window states that exceed memory.

#### Spark Streaming (Structured Streaming)
Spark focuses on micro-batch processing:
- **Window Operations**: DataFrame-based windowing with watermarks
- **Late Data Handling**: Configurable watermark delays
- **State Timeout**: Automatic cleanup of old state

**Key Insight**: Spark's declarative API simplifies complex windowing logic but has higher latency than true streaming.

### 1.2 Recommended Windowing for LLM Metrics

Based on analysis of the codebase event types (`FeedbackEvent`, `PerformanceMetricsEvent`, `UserFeedbackEvent`):

#### Primary Window Type: **Hybrid Tumbling + Session Windows**

```yaml
# Tumbling Windows for Performance Metrics
performance_metrics:
  window_type: tumbling
  size: 5 minutes
  allowed_lateness: 2 minutes

  # Aggregations
  aggregations:
    - latency_p50, latency_p95, latency_p99
    - error_rate
    - throughput_qps
    - cost_per_request

# Session Windows for User Feedback
user_feedback:
  window_type: session
  inactivity_gap: 30 minutes
  max_session_duration: 4 hours

  # Aggregations
  aggregations:
    - average_rating
    - task_completion_rate
    - regeneration_count

# Sliding Windows for Anomaly Detection
anomaly_detection:
  window_type: sliding
  size: 15 minutes
  slide: 1 minute
  allowed_lateness: 1 minute
```

**Rationale**:
1. **Tumbling for Metrics**: LLM performance metrics (latency, cost, tokens) are best aggregated over fixed intervals for trend analysis
2. **Session for User Feedback**: User interactions are naturally session-based; session windows capture complete user journeys
3. **Sliding for Anomalies**: Overlapping windows provide smoother detection of gradual degradation

### 1.3 Window Alignment Strategy

```rust
// Event-time alignment to avoid clock skew
pub struct WindowAlignment {
    /// Align windows to UTC boundaries (e.g., :00, :05, :10 for 5-min windows)
    align_to_utc: bool,

    /// Offset from UTC in seconds (for timezone-specific analysis)
    utc_offset_seconds: i64,

    /// Start alignment (for historical replay)
    alignment_epoch: Option<DateTime<Utc>>,
}

impl Default for WindowAlignment {
    fn default() -> Self {
        Self {
            align_to_utc: true,
            utc_offset_seconds: 0,
            alignment_epoch: None,
        }
    }
}
```

---

## 2. Watermarking Algorithms

### 2.1 Industry Approaches

#### Google Dataflow (Beam) Watermarks
- **Heuristic Watermarks**: Track min(event_time) across all sources
- **Perfect Watermarks**: For finite, ordered sources
- **Watermark Propagation**: Downstream operators inherit watermarks

**Key Formula**: `watermark(t) = min(event_time_of_oldest_incomplete_event) - max_out_of_order_delay`

#### Apache Flink Watermarks
- **Periodic Watermarks**: Emitted at fixed intervals (e.g., every 200ms)
- **Punctuated Watermarks**: Emitted based on special markers in stream
- **Per-Partition Watermarks**: Track watermarks independently per Kafka partition

**Key Insight**: Flink's watermark strategy is customizable per source, allowing mixed patterns.

#### Kafka Streams Watermarks
- **Automatic Watermarks**: Derived from record timestamps
- **Grace Period**: Additional wait time after watermark for late data
- **Suppress Operator**: Holds results until watermark passes

### 2.2 Recommended Watermarking for LLM Metrics

#### Bounded Out-of-Order Watermark (Primary Strategy)

```rust
use chrono::{DateTime, Duration, Utc};
use std::collections::BTreeMap;

/// Watermark generator tracking event-time progress
pub struct BoundedOutOfOrderWatermark {
    /// Maximum allowed out-of-order delay
    max_out_of_order_delay: Duration,

    /// Last observed event time per partition
    partition_timestamps: BTreeMap<u32, DateTime<Utc>>,

    /// Current watermark
    current_watermark: DateTime<Utc>,

    /// Idle timeout - mark partition as inactive after this duration
    idle_timeout: Duration,

    /// Last update time per partition (for idle detection)
    partition_last_update: BTreeMap<u32, DateTime<Utc>>,
}

impl BoundedOutOfOrderWatermark {
    pub fn new(max_out_of_order_delay: Duration, idle_timeout: Duration) -> Self {
        Self {
            max_out_of_order_delay,
            partition_timestamps: BTreeMap::new(),
            current_watermark: DateTime::<Utc>::MIN_UTC,
            idle_timeout,
            partition_last_update: BTreeMap::new(),
        }
    }

    /// Update watermark based on new event
    pub fn on_event(&mut self, partition: u32, event_time: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let now = Utc::now();

        // Update partition timestamp
        self.partition_timestamps
            .entry(partition)
            .and_modify(|t| *t = (*t).max(event_time))
            .or_insert(event_time);

        self.partition_last_update.insert(partition, now);

        // Calculate new watermark: min across all active partitions - delay
        let active_min = self.partition_timestamps
            .iter()
            .filter(|(part_id, _)| {
                // Exclude idle partitions
                if let Some(last_update) = self.partition_last_update.get(part_id) {
                    now.signed_duration_since(*last_update) < self.idle_timeout
                } else {
                    false
                }
            })
            .map(|(_, timestamp)| *timestamp)
            .min()?;

        let new_watermark = active_min - self.max_out_of_order_delay;

        // Watermarks must be monotonically increasing
        if new_watermark > self.current_watermark {
            self.current_watermark = new_watermark;
            Some(new_watermark)
        } else {
            None
        }
    }

    /// Get current watermark
    pub fn current(&self) -> DateTime<Utc> {
        self.current_watermark
    }

    /// Force watermark advancement (for idle partitions)
    pub fn advance_idle_partitions(&mut self) -> Option<DateTime<Utc>> {
        let now = Utc::now();

        // Remove idle partitions
        self.partition_timestamps.retain(|part_id, _| {
            if let Some(last_update) = self.partition_last_update.get(part_id) {
                now.signed_duration_since(*last_update) < self.idle_timeout
            } else {
                false
            }
        });

        // Recalculate watermark
        if let Some(min_time) = self.partition_timestamps.values().min() {
            let new_watermark = *min_time - self.max_out_of_order_delay;
            if new_watermark > self.current_watermark {
                self.current_watermark = new_watermark;
                return Some(new_watermark);
            }
        }

        None
    }
}
```

#### Configuration for LLM Events

```yaml
# Watermark configuration per event type
watermarks:
  performance_metrics:
    strategy: bounded_out_of_order
    max_out_of_order_delay: 120s  # 2 minutes for network delays
    idle_timeout: 300s             # 5 minutes idle partition timeout
    emit_interval: 1s              # Emit watermark updates every second

  user_feedback:
    strategy: bounded_out_of_order
    max_out_of_order_delay: 600s  # 10 minutes (users can submit delayed feedback)
    idle_timeout: 1800s            # 30 minutes idle timeout
    emit_interval: 5s

  system_health:
    strategy: periodic
    period: 30s                    # Health events are regular
    max_out_of_order_delay: 10s
```

### 2.3 Event Time Extraction Strategy

```rust
use llm_optimizer_types::events::FeedbackEvent;
use chrono::{DateTime, Utc};

/// Extract event time from various event types
pub trait EventTimeExtractor {
    fn extract_event_time(&self) -> DateTime<Utc>;
}

impl EventTimeExtractor for FeedbackEvent {
    fn extract_event_time(&self) -> DateTime<Utc> {
        // Use the timestamp field from the event
        // FeedbackEvent already has a timestamp() method
        *self.timestamp()
    }
}

/// Timestamp assignment strategy
#[derive(Debug, Clone)]
pub enum TimestampStrategy {
    /// Use event's embedded timestamp
    EventTime,

    /// Use Kafka record timestamp
    KafkaRecordTime,

    /// Use ingestion time (processing time when event enters the system)
    IngestionTime,

    /// Custom extractor function
    Custom(fn(&FeedbackEvent) -> DateTime<Utc>),
}

impl Default for TimestampStrategy {
    fn default() -> Self {
        // Prefer event time for accurate windowing
        TimestampStrategy::EventTime
    }
}
```

**Recommendation**: Use `EventTime` strategy for LLM metrics to ensure accurate analysis even during:
- Replay scenarios (reprocessing historical data)
- Network partitions causing delayed delivery
- Multi-datacenter deployments with clock skew

---

## 3. State Management and Checkpointing

### 3.1 Industry Patterns

#### Apache Flink State Management
- **Keyed State**: Per-key state (e.g., per model_id)
- **Operator State**: Per-operator-instance state
- **State Backends**: Memory, RocksDB (disk), or custom
- **Checkpointing**: Periodic snapshots to durable storage (HDFS, S3)
- **Savepoints**: Manual checkpoints for version upgrades

**Key Metrics**:
- Checkpoint interval: 30-60 seconds (production)
- State size: 10GB-100GB per operator is common
- Recovery time: 10-30 seconds for most workloads

#### Kafka Streams State
- **RocksDB State Stores**: Disk-backed, LSM-tree based
- **Changelog Topics**: Kafka topics storing state changes
- **Standby Replicas**: Hot standbys for fast failover
- **Interactive Queries**: Query state from external applications

**Key Insight**: Kafka Streams co-locates state with computation, enabling local lookups.

#### Spark Streaming State
- **Stateful Operations**: mapGroupsWithState, flatMapGroupsWithState
- **State Timeout**: Automatic cleanup based on time or processing time
- **Checkpoint Storage**: HDFS or S3 for fault tolerance

### 3.2 Recommended State Storage for LLM Metrics

#### Hybrid State Storage Architecture

```rust
use sled::Db;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Multi-tier state storage
pub struct StateStore {
    /// Hot state - in-memory for current windows (L1 cache)
    hot_state: Arc<RwLock<HashMap<StateKey, WindowState>>>,

    /// Warm state - embedded database for recent windows (L2 cache)
    warm_state: Arc<Db>,

    /// Cold state - distributed database for historical data
    cold_state: Option<Arc<dyn ColdStateBackend>>,

    /// State TTL configuration
    ttl_config: StateTtlConfig,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct StateKey {
    /// Window identifier
    window_id: WindowId,

    /// Partition key (e.g., model_id, session_id)
    partition_key: String,
}

/// Window state accumulator
#[derive(Debug, Clone)]
pub struct WindowState {
    /// Window metadata
    window: Window,

    /// Aggregated metrics
    metrics: MetricAggregation,

    /// Event count
    count: u64,

    /// First event time
    first_event_time: DateTime<Utc>,

    /// Last event time
    last_event_time: DateTime<Utc>,

    /// State creation timestamp
    created_at: DateTime<Utc>,

    /// Last access timestamp (for LRU eviction)
    last_accessed: DateTime<Utc>,
}

/// State TTL configuration
#[derive(Debug, Clone)]
pub struct StateTtlConfig {
    /// Hot state TTL (evict to warm state)
    hot_ttl: Duration,

    /// Warm state TTL (evict to cold state or delete)
    warm_ttl: Duration,

    /// Enable automatic cleanup
    auto_cleanup: bool,

    /// Cleanup interval
    cleanup_interval: Duration,
}

impl Default for StateTtlConfig {
    fn default() -> Self {
        Self {
            hot_ttl: Duration::hours(1),
            warm_ttl: Duration::days(7),
            auto_cleanup: true,
            cleanup_interval: Duration::minutes(10),
        }
    }
}
```

#### State Backend Recommendations

```yaml
# State storage configuration
state:
  # Hot state (in-memory)
  hot:
    max_size_mb: 512
    eviction_policy: lru
    ttl_seconds: 3600  # 1 hour

  # Warm state (embedded Sled DB)
  warm:
    backend: sled
    path: /var/lib/optimizer/state
    cache_size_mb: 1024
    ttl_seconds: 604800  # 7 days
    compression: zstd

  # Cold state (PostgreSQL for long-term)
  cold:
    backend: postgres
    connection_string: ${DATABASE_URL}
    table_prefix: window_state_
    retention_days: 90
```

### 3.3 Checkpointing Strategy

```rust
use tokio::time::interval;
use std::path::PathBuf;

/// Checkpoint coordinator
pub struct CheckpointCoordinator {
    /// Checkpoint interval
    interval: Duration,

    /// Checkpoint storage path
    checkpoint_dir: PathBuf,

    /// Number of checkpoints to retain
    num_retained_checkpoints: usize,

    /// Checkpoint format
    format: CheckpointFormat,
}

#[derive(Debug, Clone)]
pub enum CheckpointFormat {
    /// Binary serialization (bincode)
    Binary,

    /// JSON (for debugging)
    Json,

    /// MessagePack (compact)
    MessagePack,
}

impl CheckpointCoordinator {
    /// Create new checkpoint
    pub async fn checkpoint(&self, state: &StateStore) -> Result<CheckpointId, CheckpointError> {
        let checkpoint_id = CheckpointId::new();
        let checkpoint_path = self.checkpoint_dir.join(checkpoint_id.to_string());

        // 1. Snapshot hot state
        let hot_snapshot = state.hot_state.read().await.clone();

        // 2. Sync warm state (flush pending writes)
        state.warm_state.flush_async().await?;

        // 3. Serialize and write checkpoint
        let checkpoint_data = CheckpointData {
            checkpoint_id: checkpoint_id.clone(),
            timestamp: Utc::now(),
            hot_state: hot_snapshot,
            warm_state_path: state.warm_state.path().to_path_buf(),
        };

        match self.format {
            CheckpointFormat::Binary => {
                let encoded = bincode::serialize(&checkpoint_data)?;
                tokio::fs::write(&checkpoint_path, encoded).await?;
            }
            CheckpointFormat::Json => {
                let json = serde_json::to_string_pretty(&checkpoint_data)?;
                tokio::fs::write(&checkpoint_path, json).await?;
            }
            CheckpointFormat::MessagePack => {
                let encoded = rmp_serde::to_vec(&checkpoint_data)?;
                tokio::fs::write(&checkpoint_path, encoded).await?;
            }
        }

        // 4. Cleanup old checkpoints
        self.cleanup_old_checkpoints().await?;

        Ok(checkpoint_id)
    }

    /// Restore from checkpoint
    pub async fn restore(&self, checkpoint_id: &CheckpointId) -> Result<StateStore, CheckpointError> {
        // Implementation omitted for brevity
        todo!()
    }
}
```

**Checkpoint Configuration**:
```yaml
checkpointing:
  enabled: true
  interval_seconds: 60        # Checkpoint every minute
  format: binary              # binary, json, or messagepack
  retention_count: 10         # Keep last 10 checkpoints
  storage:
    type: local               # local, s3, or postgres
    path: /var/lib/optimizer/checkpoints
```

---

## 4. Memory Management for Long-Running Windows

### 4.1 Industry Approaches

#### Flink Memory Management
- **Managed Memory**: Flink controls heap allocation for state and sorting
- **Network Buffers**: Separate pool for network I/O
- **Off-Heap State**: RocksDB state lives off-heap
- **Memory Budgets**: Configurable % for different components

**Typical Configuration**:
```
taskmanager.memory.managed.fraction: 0.4  # 40% for state
taskmanager.memory.network.fraction: 0.1  # 10% for network
```

#### Kafka Streams Memory
- **RocksDB Block Cache**: LRU cache for frequently accessed data
- **Write Buffers**: In-memory buffers before flush to disk
- **Compaction**: Background process to merge SSTables

**Key Setting**: `rocksdb.block.cache.size` - typically 100MB-1GB

### 4.2 Memory Management Strategy for LLM Metrics

#### Memory Budget Allocation

```rust
/// Memory budget for window operators
#[derive(Debug, Clone)]
pub struct MemoryBudget {
    /// Total memory limit in bytes
    total_limit: usize,

    /// Allocation percentages
    hot_state_fraction: f64,
    warm_state_cache_fraction: f64,
    network_buffer_fraction: f64,
    overhead_fraction: f64,
}

impl Default for MemoryBudget {
    fn default() -> Self {
        Self {
            total_limit: 4 * 1024 * 1024 * 1024,  // 4GB default
            hot_state_fraction: 0.5,               // 50% for hot state
            warm_state_cache_fraction: 0.3,        // 30% for warm cache
            network_buffer_fraction: 0.1,          // 10% for network
            overhead_fraction: 0.1,                // 10% overhead
        }
    }
}

impl MemoryBudget {
    pub fn hot_state_limit(&self) -> usize {
        (self.total_limit as f64 * self.hot_state_fraction) as usize
    }

    pub fn warm_cache_limit(&self) -> usize {
        (self.total_limit as f64 * self.warm_state_cache_fraction) as usize
    }
}
```

#### Adaptive Window Pruning

```rust
use std::collections::BTreeMap;

/// Adaptive pruning based on memory pressure
pub struct AdaptiveWindowPruner {
    /// Memory monitor
    memory_monitor: MemoryMonitor,

    /// Pruning strategy
    strategy: PruningStrategy,
}

#[derive(Debug, Clone)]
pub enum PruningStrategy {
    /// Prune windows below watermark
    Watermark,

    /// LRU eviction of oldest windows
    LeastRecentlyUsed,

    /// Prune windows with low event count
    LowActivity,

    /// Hybrid strategy
    Adaptive,
}

impl AdaptiveWindowPruner {
    pub async fn prune_if_needed(
        &self,
        windows: &mut BTreeMap<WindowId, WindowState>,
        watermark: DateTime<Utc>,
    ) -> usize {
        let memory_usage = self.memory_monitor.current_usage();
        let memory_limit = self.memory_monitor.limit();

        if memory_usage < memory_limit * 0.8 {
            // Below threshold - only prune completed windows
            return self.prune_completed_windows(windows, watermark);
        }

        // High memory pressure - aggressive pruning
        match self.strategy {
            PruningStrategy::Watermark => {
                self.prune_below_watermark(windows, watermark)
            }
            PruningStrategy::LeastRecentlyUsed => {
                self.prune_lru(windows, memory_limit)
            }
            PruningStrategy::LowActivity => {
                self.prune_low_activity(windows)
            }
            PruningStrategy::Adaptive => {
                // Try strategies in order until memory usage drops
                let mut pruned = self.prune_below_watermark(windows, watermark);

                if self.memory_monitor.current_usage() > memory_limit * 0.7 {
                    pruned += self.prune_low_activity(windows);
                }

                if self.memory_monitor.current_usage() > memory_limit * 0.6 {
                    pruned += self.prune_lru(windows, memory_limit);
                }

                pruned
            }
        }
    }

    fn prune_completed_windows(
        &self,
        windows: &mut BTreeMap<WindowId, WindowState>,
        watermark: DateTime<Utc>,
    ) -> usize {
        let to_remove: Vec<_> = windows
            .iter()
            .filter(|(id, state)| {
                // Prune windows that have ended and passed watermark
                state.window.end() < watermark
            })
            .map(|(id, _)| id.clone())
            .collect();

        let count = to_remove.len();
        for id in to_remove {
            windows.remove(&id);
        }
        count
    }
}
```

#### Memory Configuration

```yaml
# Memory management configuration
memory:
  # Total memory budget
  total_limit_mb: 4096

  # Allocation fractions
  allocation:
    hot_state: 0.5           # 50% for in-memory windows
    warm_cache: 0.3          # 30% for Sled cache
    network_buffers: 0.1     # 10% for Kafka I/O
    overhead: 0.1            # 10% overhead

  # Pruning strategy
  pruning:
    strategy: adaptive       # watermark, lru, low_activity, or adaptive
    high_watermark: 0.8      # Start pruning at 80% memory
    low_watermark: 0.6       # Stop pruning at 60% memory
    check_interval_ms: 5000  # Check every 5 seconds
```

---

## 5. Trigger Policies (Processing Time vs Event Time)

### 5.1 Industry Trigger Patterns

#### Flink Triggers
- **EventTimeTrigger**: Fire when watermark passes window end
- **ProcessingTimeTrigger**: Fire based on wall-clock time
- **CountTrigger**: Fire after N elements
- **ContinuousEventTimeTrigger**: Fire periodically during window
- **Custom Triggers**: Combine multiple conditions

#### Kafka Streams Suppression
- **Suppress Until Window Close**: Hold results until window completes
- **Suppress Until Time Limit**: Emit after maximum delay
- **Suppress With Emit Strategy**: Control intermediate emissions

### 5.2 Recommended Trigger Policy for LLM Metrics

#### Hybrid Trigger System

```rust
use chrono::{DateTime, Duration, Utc};

/// Trigger policy for window emission
#[derive(Debug, Clone)]
pub enum TriggerPolicy {
    /// Fire when watermark passes window end (event-time)
    OnWatermark,

    /// Fire at fixed processing-time intervals
    ProcessingTime(Duration),

    /// Fire when element count reaches threshold
    OnCount(u64),

    /// Fire early on high-priority events
    OnCriticalEvent,

    /// Composite trigger (OR semantics)
    Or(Vec<TriggerPolicy>),

    /// Composite trigger (AND semantics)
    And(Vec<TriggerPolicy>),
}

/// Trigger context
pub struct TriggerContext {
    /// Current watermark
    watermark: DateTime<Utc>,

    /// Current processing time
    processing_time: DateTime<Utc>,

    /// Window state
    window_state: WindowState,

    /// Is window complete (watermark passed end)
    is_complete: bool,
}

impl TriggerPolicy {
    /// Check if trigger should fire
    pub fn should_fire(&self, ctx: &TriggerContext) -> bool {
        match self {
            TriggerPolicy::OnWatermark => {
                ctx.is_complete
            }

            TriggerPolicy::ProcessingTime(interval) => {
                let elapsed = ctx.processing_time
                    .signed_duration_since(ctx.window_state.created_at);
                elapsed >= *interval
            }

            TriggerPolicy::OnCount(threshold) => {
                ctx.window_state.count >= *threshold
            }

            TriggerPolicy::OnCriticalEvent => {
                // Check if last event was critical (e.g., high error rate)
                false  // Simplified for example
            }

            TriggerPolicy::Or(policies) => {
                policies.iter().any(|p| p.should_fire(ctx))
            }

            TriggerPolicy::And(policies) => {
                policies.iter().all(|p| p.should_fire(ctx))
            }
        }
    }

    /// Check if window should be purged after firing
    pub fn should_purge(&self) -> bool {
        match self {
            TriggerPolicy::OnWatermark => true,  // Final result
            TriggerPolicy::ProcessingTime(_) => false,  // Intermediate result
            TriggerPolicy::OnCount(_) => false,
            TriggerPolicy::OnCriticalEvent => false,
            TriggerPolicy::Or(policies) => {
                policies.iter().any(|p| p.should_purge())
            }
            TriggerPolicy::And(policies) => {
                policies.iter().all(|p| p.should_purge())
            }
        }
    }
}
```

#### Trigger Configuration by Event Type

```yaml
# Trigger policies per window type
triggers:
  # Performance metrics - require completeness
  performance_metrics:
    primary: on_watermark
    early_firing:
      enabled: true
      policy: processing_time
      interval: 30s
    allowed_lateness: 120s

  # User feedback - emit on session close
  user_feedback:
    primary: on_watermark
    early_firing:
      enabled: false

  # Anomaly detection - low latency required
  anomaly_detection:
    primary: or
    policies:
      - processing_time: 5s      # Emit every 5 seconds
      - on_watermark              # Also emit complete results
      - on_critical_event         # Immediate for critical anomalies
```

### 5.3 Early Firing and Late Data Handling

```rust
/// Window emission strategy
#[derive(Debug, Clone)]
pub struct EmissionStrategy {
    /// Main trigger policy
    main_trigger: TriggerPolicy,

    /// Early firing policy (before watermark)
    early_firing: Option<TriggerPolicy>,

    /// Late firing policy (after watermark)
    late_firing: Option<LateFiringPolicy>,

    /// Accumulation mode
    accumulation_mode: AccumulationMode,
}

#[derive(Debug, Clone)]
pub enum AccumulationMode {
    /// Emit cumulative results (update previous)
    Accumulating,

    /// Emit only new data since last fire
    Discarding,

    /// Emit retractions and new values (for exactly-once)
    AccumulatingAndRetracting,
}

#[derive(Debug, Clone)]
pub struct LateFiringPolicy {
    /// Maximum delay after watermark to accept late data
    allowed_lateness: Duration,

    /// How often to emit updates for late data
    late_firing_interval: Option<Duration>,
}

impl EmissionStrategy {
    /// Process event and determine if window should fire
    pub fn on_event(
        &self,
        event: &FeedbackEvent,
        window_state: &mut WindowState,
        watermark: DateTime<Utc>,
    ) -> Vec<WindowEmission> {
        let mut emissions = Vec::new();

        let ctx = TriggerContext {
            watermark,
            processing_time: Utc::now(),
            window_state: window_state.clone(),
            is_complete: window_state.window.end() < watermark,
        };

        // Check early firing
        if !ctx.is_complete {
            if let Some(ref early) = self.early_firing {
                if early.should_fire(&ctx) {
                    emissions.push(WindowEmission {
                        window: window_state.window.clone(),
                        metrics: window_state.metrics.clone(),
                        emission_type: EmissionType::Early,
                        timestamp: Utc::now(),
                    });
                }
            }
        }

        // Check main trigger
        if self.main_trigger.should_fire(&ctx) {
            let emission_type = if ctx.is_complete {
                EmissionType::OnTime
            } else {
                EmissionType::Early
            };

            emissions.push(WindowEmission {
                window: window_state.window.clone(),
                metrics: window_state.metrics.clone(),
                emission_type,
                timestamp: Utc::now(),
            });
        }

        emissions
    }
}

#[derive(Debug, Clone)]
pub enum EmissionType {
    /// Early emission before watermark
    Early,

    /// On-time emission when watermark passes
    OnTime,

    /// Late emission after watermark
    Late,
}
```

---

## 6. LLM-Specific Recommendations

### 6.1 Event Time Extraction

Based on analysis of `/workspaces/llm-auto-optimizer/crates/types/src/events.rs` and `/workspaces/llm-auto-optimizer/crates/collector/src/feedback_events.rs`:

```rust
use llm_optimizer_types::events::{FeedbackEvent, EventSource};
use chrono::{DateTime, Utc};

/// Event time extraction for LLM events
impl FeedbackEvent {
    /// Get event time (primary timestamp for windowing)
    pub fn event_time(&self) -> DateTime<Utc> {
        *self.timestamp()
    }

    /// Get ingestion time (when event entered the system)
    pub fn ingestion_time(&self) -> Option<DateTime<Utc>> {
        // Could be added as metadata
        self.metadata()
            .get("ingestion_time")
            .and_then(|s| s.parse::<DateTime<Utc>>().ok())
    }

    /// Get correlation ID for joining related events
    pub fn correlation_id(&self) -> Option<String> {
        match self {
            FeedbackEvent::UserFeedback(e) => Some(e.request_id.to_string()),
            FeedbackEvent::PerformanceMetrics(e) => Some(e.request_id.to_string()),
            FeedbackEvent::ModelResponse(e) => Some(e.request_id.to_string()),
            _ => None,
        }
    }
}
```

### 6.2 Watermark Configuration by Event Source

```yaml
# Source-specific watermark strategies
watermark_strategies:
  # Observatory metrics (OpenTelemetry)
  observatory:
    source: EventSource::Observatory
    strategy: bounded_out_of_order
    max_delay: 60s
    idle_timeout: 300s
    reason: "Network delays in distributed tracing"

  # User feedback (client-side events)
  user:
    source: EventSource::User
    strategy: bounded_out_of_order
    max_delay: 600s  # Users can submit delayed feedback
    idle_timeout: 1800s
    reason: "User actions can be significantly delayed"

  # System events (internally generated)
  system:
    source: EventSource::System
    strategy: bounded_out_of_order
    max_delay: 10s
    idle_timeout: 60s
    reason: "System events are near real-time"
```

### 6.3 Window Partitioning Strategy

```rust
use uuid::Uuid;

/// Window partitioning key for LLM metrics
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum PartitionKey {
    /// Partition by model identifier
    Model(String),

    /// Partition by user/session
    Session(String),

    /// Partition by request ID (for joins)
    Request(Uuid),

    /// Partition by task type
    TaskType(String),

    /// Composite key
    Composite(Vec<String>),
}

impl PartitionKey {
    /// Extract partition key from event
    pub fn from_event(event: &FeedbackEvent) -> Vec<Self> {
        match event {
            FeedbackEvent::PerformanceMetrics(e) => {
                vec![
                    PartitionKey::Model(e.model.clone()),
                    PartitionKey::Request(e.request_id),
                ]
            }
            FeedbackEvent::UserFeedback(e) => {
                vec![
                    PartitionKey::Session(e.session_id.clone()),
                    PartitionKey::Request(e.request_id),
                ]
            }
            FeedbackEvent::ModelResponse(e) => {
                vec![
                    PartitionKey::Model(e.model.clone()),
                    PartitionKey::Session(e.session_id.clone()),
                    PartitionKey::Request(e.request_id),
                ]
            }
            _ => vec![],
        }
    }
}
```

**Recommended Partitioning**:
1. **By Model**: For model-specific performance analysis
2. **By Session**: For user journey analysis
3. **By Request**: For joining metrics with feedback

### 6.4 Aggregation Functions for LLM Metrics

```rust
use statrs::statistics::{Data, OrderStatistics, Statistics};

/// Metric aggregator for window state
#[derive(Debug, Clone)]
pub struct MetricAggregator {
    /// Latency samples (in milliseconds)
    latencies: Vec<f64>,

    /// Cost accumulator
    total_cost: f64,

    /// Token counts
    input_tokens: Vec<usize>,
    output_tokens: Vec<usize>,

    /// Error counter
    errors: u64,

    /// Total events
    count: u64,
}

impl MetricAggregator {
    pub fn add_performance_event(&mut self, event: &PerformanceMetricsEvent) {
        self.latencies.push(event.latency_ms);
        self.total_cost += event.cost;
        self.input_tokens.push(event.input_tokens);
        self.output_tokens.push(event.output_tokens);

        if event.error.is_some() {
            self.errors += 1;
        }

        self.count += 1;
    }

    pub fn compute_aggregates(&self) -> PerformanceMetrics {
        let mut latency_data = Data::new(self.latencies.clone());

        PerformanceMetrics {
            avg_latency_ms: latency_data.mean().unwrap_or(0.0),
            p50_latency_ms: latency_data.percentile(50).unwrap_or(0.0),
            p95_latency_ms: latency_data.percentile(95).unwrap_or(0.0),
            p99_latency_ms: latency_data.percentile(99).unwrap_or(0.0),
            error_rate: self.errors as f64 / self.count as f64,
            throughput_qps: self.count as f64 / 300.0,  // Assuming 5-min window
            total_requests: self.count,
            period_start: DateTime::<Utc>::MIN_UTC,  // Set from window
            period_end: Utc::now(),
        }
    }
}
```

---

## 7. Implementation Roadmap

### Phase 1: Core Windowing (Weeks 1-2)

**Deliverables**:
1. Window types: Tumbling, Sliding, Session
2. Basic watermark generator (bounded out-of-order)
3. In-memory state store
4. Event-time assignment

**Files to Create**:
- `/workspaces/llm-auto-optimizer/crates/processor/src/windowing/mod.rs`
- `/workspaces/llm-auto-optimizer/crates/processor/src/windowing/window.rs`
- `/workspaces/llm-auto-optimizer/crates/processor/src/windowing/watermark.rs`
- `/workspaces/llm-auto-optimizer/crates/processor/src/windowing/state.rs`

### Phase 2: State Management (Weeks 3-4)

**Deliverables**:
1. Multi-tier state store (hot/warm/cold)
2. Sled integration for warm state
3. Checkpointing system
4. State TTL and cleanup

**Files to Create**:
- `/workspaces/llm-auto-optimizer/crates/processor/src/state/mod.rs`
- `/workspaces/llm-auto-optimizer/crates/processor/src/state/checkpoint.rs`
- `/workspaces/llm-auto-optimizer/crates/processor/src/state/tiered_store.rs`

### Phase 3: Advanced Features (Weeks 5-6)

**Deliverables**:
1. Trigger policies (early/on-time/late firing)
2. Memory management and adaptive pruning
3. Late data handling
4. Metrics and monitoring

**Files to Create**:
- `/workspaces/llm-auto-optimizer/crates/processor/src/triggers/mod.rs`
- `/workspaces/llm-auto-optimizer/crates/processor/src/memory/mod.rs`

### Phase 4: Integration (Week 7)

**Deliverables**:
1. Kafka consumer integration
2. Aggregation operators for LLM metrics
3. Output to storage layer
4. End-to-end tests

**Files to Create**:
- `/workspaces/llm-auto-optimizer/crates/processor/src/operators/mod.rs`
- `/workspaces/llm-auto-optimizer/crates/processor/src/operators/aggregate.rs`

---

## Appendix A: Configuration Template

```yaml
# Complete stream processing configuration
stream_processing:
  # Windowing configuration
  windows:
    performance_metrics:
      type: tumbling
      size: 300s  # 5 minutes
      allowed_lateness: 120s

    user_feedback:
      type: session
      gap: 1800s  # 30 minutes
      max_duration: 14400s  # 4 hours

    anomaly_detection:
      type: sliding
      size: 900s  # 15 minutes
      slide: 60s  # 1 minute

  # Watermarking
  watermarks:
    strategy: bounded_out_of_order
    max_out_of_order_delay: 120s
    idle_timeout: 300s
    emit_interval: 1s

  # State management
  state:
    hot:
      max_size_mb: 512
      ttl_seconds: 3600
    warm:
      backend: sled
      path: /var/lib/optimizer/state
      cache_size_mb: 1024
      ttl_seconds: 604800
    cold:
      backend: postgres
      retention_days: 90

  # Checkpointing
  checkpointing:
    enabled: true
    interval_seconds: 60
    format: binary
    retention_count: 10

  # Memory management
  memory:
    total_limit_mb: 4096
    allocation:
      hot_state: 0.5
      warm_cache: 0.3
      network_buffers: 0.1
      overhead: 0.1
    pruning:
      strategy: adaptive
      high_watermark: 0.8

  # Triggers
  triggers:
    performance_metrics:
      primary: on_watermark
      early_firing:
        enabled: true
        interval: 30s
```

---

## References

1. **Apache Flink Documentation**: https://flink.apache.org/
2. **Kafka Streams Documentation**: https://kafka.apache.org/documentation/streams/
3. **Google Dataflow Model**: "The Dataflow Model" (2015)
4. **State Management in Flink**: https://nightlies.apache.org/flink/flink-docs-stable/docs/dev/datastream/fault-tolerance/state/
5. **Watermarking Best Practices**: "Streaming 101" and "Streaming 102" by Tyler Akidau

---

**Document Version**: 1.0
**Last Updated**: 2025-11-09
**Next Review**: 2025-12-09
