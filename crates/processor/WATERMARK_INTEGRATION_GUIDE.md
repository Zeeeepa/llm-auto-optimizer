# Watermark Integration Guide for LLM Auto-Optimizer

This guide shows how to integrate the watermark module into the LLM Auto-Optimizer's stream processing pipeline.

## Integration Points

### 1. Feedback Event Stream Processing

```rust
use processor::watermark::{BoundedOutOfOrdernessWatermark, WatermarkGenerator};
use llm_optimizer_types::events::FeedbackEvent;
use std::time::Duration;

/// Process feedback events with watermarking
pub struct FeedbackProcessor {
    watermark_gen: BoundedOutOfOrdernessWatermark,
    late_event_handler: LateEventHandler,
}

impl FeedbackProcessor {
    pub fn new() -> Self {
        Self {
            watermark_gen: BoundedOutOfOrdernessWatermark::new(
                Duration::from_secs(10), // Allow 10s out-of-orderness
                Some(Duration::from_secs(300)), // 5 min idle timeout
            ),
            late_event_handler: LateEventHandler::new(LateEventConfig {
                allowed_lateness: Duration::from_secs(30),
                log_late_events: true,
                emit_metrics: true,
            }),
        }
    }

    pub async fn process_event(&mut self, event: FeedbackEvent) -> anyhow::Result<()> {
        let timestamp = event.timestamp.timestamp_millis();
        let partition = self.calculate_partition(&event);

        // Update watermark
        if let Some(new_watermark) = self.watermark_gen.on_event(timestamp, partition) {
            // Watermark advanced - trigger downstream operations
            self.trigger_window_computations(new_watermark).await?;
        }

        // Check if event is late
        if self.watermark_gen.is_late_event(timestamp) {
            let lateness = self.watermark_gen.get_lateness(timestamp);

            if self.late_event_handler.handle_late_event(lateness, partition) {
                // Process late event (within allowed lateness)
                tracing::warn!(
                    event_id = %event.id,
                    lateness_ms = lateness,
                    "Processing late feedback event"
                );
                self.process_late_event(event).await?;
            } else {
                // Drop event (beyond allowed lateness)
                tracing::error!(
                    event_id = %event.id,
                    lateness_ms = lateness,
                    "Dropping late feedback event"
                );
                self.emit_dropped_event_metric(&event);
            }
        } else {
            // On-time event - normal processing
            self.process_on_time_event(event).await?;
        }

        Ok(())
    }

    fn calculate_partition(&self, event: &FeedbackEvent) -> u32 {
        // Hash event source to partition
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        event.source.hash(&mut hasher);
        (hasher.finish() % 256) as u32
    }

    async fn trigger_window_computations(&self, watermark: Watermark) -> anyhow::Result<()> {
        // Trigger any window computations that are now complete
        tracing::debug!(watermark = %watermark, "Triggering window computations");
        Ok(())
    }

    async fn process_late_event(&self, event: FeedbackEvent) -> anyhow::Result<()> {
        // Special handling for late events
        Ok(())
    }

    async fn process_on_time_event(&self, event: FeedbackEvent) -> anyhow::Result<()> {
        // Normal event processing
        Ok(())
    }

    fn emit_dropped_event_metric(&self, event: &FeedbackEvent) {
        // Emit metrics for monitoring
    }
}
```

### 2. Metric Stream Processing

```rust
use processor::watermark::{PeriodicWatermark, PeriodicWatermarkConfig, WatermarkGenerator};
use llm_optimizer_types::metrics::MetricPoint;

/// Process metrics with periodic watermarking
pub struct MetricStreamProcessor {
    watermark_gen: PeriodicWatermark,
}

impl MetricStreamProcessor {
    pub fn new() -> Self {
        Self {
            watermark_gen: PeriodicWatermark::new(PeriodicWatermarkConfig {
                interval: Duration::from_secs(5), // Emit watermark every 5s
                max_out_of_orderness: Duration::from_secs(10),
            }),
        }
    }

    pub async fn process_metric(&mut self, metric: MetricPoint) -> anyhow::Result<()> {
        let timestamp = metric.timestamp.timestamp_millis();
        let partition = self.partition_for_metric(&metric);

        // Update internal state (doesn't emit on event)
        self.watermark_gen.on_event(timestamp, partition);

        // Process metric
        self.aggregate_metric(metric).await?;

        Ok(())
    }

    pub async fn periodic_tick(&mut self) -> anyhow::Result<()> {
        // Called periodically (e.g., every 5 seconds)
        if let Some(watermark) = self.watermark_gen.on_periodic_check() {
            // Emit watermark and trigger computations
            self.emit_watermark(watermark).await?;
            self.finalize_windows(watermark).await?;
        }
        Ok(())
    }

    fn partition_for_metric(&self, metric: &MetricPoint) -> u32 {
        // Partition by metric name or other key
        0
    }

    async fn aggregate_metric(&self, metric: MetricPoint) -> anyhow::Result<()> {
        Ok(())
    }

    async fn emit_watermark(&self, watermark: Watermark) -> anyhow::Result<()> {
        tracing::debug!(watermark = %watermark, "Emitting periodic watermark");
        Ok(())
    }

    async fn finalize_windows(&self, watermark: Watermark) -> anyhow::Result<()> {
        Ok(())
    }
}
```

### 3. Windowed Aggregation with Watermarks

```rust
use processor::watermark::{BoundedOutOfOrdernessWatermark, WatermarkGenerator};
use std::collections::BTreeMap;

/// Time window for aggregation
#[derive(Debug, Clone)]
pub struct TimeWindow {
    pub start: i64,
    pub end: i64,
}

/// Windowed aggregator using watermarks
pub struct WindowedAggregator {
    watermark_gen: BoundedOutOfOrdernessWatermark,
    windows: BTreeMap<i64, WindowState>,
    window_size_ms: i64,
}

#[derive(Debug)]
struct WindowState {
    start: i64,
    end: i64,
    count: u64,
    sum: f64,
}

impl WindowedAggregator {
    pub fn new(window_size: Duration) -> Self {
        Self {
            watermark_gen: BoundedOutOfOrdernessWatermark::new(
                Duration::from_secs(5),
                None,
            ),
            windows: BTreeMap::new(),
            window_size_ms: window_size.as_millis() as i64,
        }
    }

    pub async fn process_value(&mut self, timestamp: i64, value: f64, partition: u32)
        -> anyhow::Result<Vec<WindowResult>>
    {
        // Update watermark
        let old_watermark = self.watermark_gen.current_watermark();
        self.watermark_gen.on_event(timestamp, partition);
        let new_watermark = self.watermark_gen.current_watermark();

        // Add value to appropriate window
        let window_start = (timestamp / self.window_size_ms) * self.window_size_ms;
        let window = self.windows.entry(window_start).or_insert(WindowState {
            start: window_start,
            end: window_start + self.window_size_ms,
            count: 0,
            sum: 0.0,
        });

        window.count += 1;
        window.sum += value;

        // Check if watermark advanced and finalize windows
        let mut results = Vec::new();
        if new_watermark > old_watermark {
            results = self.finalize_completed_windows(new_watermark).await?;
        }

        Ok(results)
    }

    async fn finalize_completed_windows(&mut self, watermark: Watermark)
        -> anyhow::Result<Vec<WindowResult>>
    {
        let mut results = Vec::new();

        // Find windows that are complete (end < watermark)
        let completed_windows: Vec<_> = self.windows
            .iter()
            .filter(|(_, w)| w.end < watermark.timestamp)
            .map(|(start, _)| *start)
            .collect();

        // Finalize and emit results
        for window_start in completed_windows {
            if let Some(window) = self.windows.remove(&window_start) {
                let avg = if window.count > 0 {
                    window.sum / window.count as f64
                } else {
                    0.0
                };

                results.push(WindowResult {
                    window_start: window.start,
                    window_end: window.end,
                    count: window.count,
                    average: avg,
                });

                tracing::info!(
                    window_start = window.start,
                    window_end = window.end,
                    count = window.count,
                    average = avg,
                    "Finalized window"
                );
            }
        }

        Ok(results)
    }
}

#[derive(Debug, Clone)]
pub struct WindowResult {
    pub window_start: i64,
    pub window_end: i64,
    pub count: u64,
    pub average: f64,
}
```

### 4. Multi-Source Stream Merger

```rust
use processor::watermark::{BoundedOutOfOrdernessWatermark, WatermarkGenerator};
use tokio::sync::mpsc;

/// Merge multiple event streams with watermark coordination
pub struct MultiSourceMerger {
    sources: Vec<SourceWatermark>,
    output_tx: mpsc::Sender<ProcessedEvent>,
}

struct SourceWatermark {
    id: String,
    generator: BoundedOutOfOrdernessWatermark,
}

impl MultiSourceMerger {
    pub fn new(source_ids: Vec<String>, output_tx: mpsc::Sender<ProcessedEvent>) -> Self {
        let sources = source_ids.into_iter().map(|id| {
            SourceWatermark {
                id,
                generator: BoundedOutOfOrdernessWatermark::new(
                    Duration::from_secs(5),
                    Some(Duration::from_secs(60)),
                ),
            }
        }).collect();

        Self { sources, output_tx }
    }

    pub async fn process_event(&mut self, source_id: &str, event: Event)
        -> anyhow::Result<()>
    {
        // Find source watermark generator
        let source = self.sources.iter_mut()
            .find(|s| s.id == source_id)
            .ok_or_else(|| anyhow::anyhow!("Unknown source: {}", source_id))?;

        // Update source watermark
        let timestamp = event.timestamp;
        source.generator.on_event(timestamp, 0);

        // Calculate global watermark (min of all sources)
        let global_watermark = self.calculate_global_watermark();

        // Check if event is late relative to global watermark
        if timestamp < global_watermark.timestamp {
            tracing::warn!(
                source_id = source_id,
                event_timestamp = timestamp,
                global_watermark = %global_watermark,
                "Late event detected"
            );
        }

        // Forward event
        self.output_tx.send(ProcessedEvent {
            source_id: source_id.to_string(),
            event,
            watermark: global_watermark,
        }).await?;

        Ok(())
    }

    fn calculate_global_watermark(&self) -> Watermark {
        // Global watermark is minimum of all source watermarks
        self.sources
            .iter()
            .map(|s| s.generator.current_watermark())
            .min()
            .unwrap_or(Watermark::min())
    }

    pub fn get_source_status(&self) -> Vec<SourceStatus> {
        self.sources.iter().map(|s| {
            SourceStatus {
                id: s.id.clone(),
                watermark: s.generator.current_watermark(),
                is_idle: s.generator.idle_partitions().len() > 0,
            }
        }).collect()
    }
}

#[derive(Debug)]
pub struct Event {
    pub timestamp: i64,
    pub data: serde_json::Value,
}

#[derive(Debug)]
pub struct ProcessedEvent {
    pub source_id: String,
    pub event: Event,
    pub watermark: Watermark,
}

#[derive(Debug)]
pub struct SourceStatus {
    pub id: String,
    pub watermark: Watermark,
    pub is_idle: bool,
}
```

### 5. Kafka Consumer with Watermarking

```rust
use processor::watermark::{BoundedOutOfOrdernessWatermark, WatermarkGenerator, LateEventHandler};
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::Message;

/// Kafka consumer with built-in watermarking
pub struct WatermarkedKafkaConsumer {
    consumer: StreamConsumer,
    watermark_gen: BoundedOutOfOrdernessWatermark,
    late_handler: LateEventHandler,
}

impl WatermarkedKafkaConsumer {
    pub fn new(consumer: StreamConsumer) -> Self {
        Self {
            consumer,
            watermark_gen: BoundedOutOfOrdernessWatermark::new(
                Duration::from_secs(10),
                Some(Duration::from_secs(300)),
            ),
            late_handler: LateEventHandler::new(LateEventConfig {
                allowed_lateness: Duration::from_secs(30),
                log_late_events: true,
                emit_metrics: true,
            }),
        }
    }

    pub async fn consume_and_process(&mut self) -> anyhow::Result<()> {
        loop {
            match self.consumer.recv().await {
                Ok(message) => {
                    self.process_message(message).await?;
                }
                Err(e) => {
                    tracing::error!(error = ?e, "Kafka consumer error");
                }
            }
        }
    }

    async fn process_message(&mut self, message: rdkafka::message::BorrowedMessage<'_>)
        -> anyhow::Result<()>
    {
        // Extract event from Kafka message
        let payload = message.payload()
            .ok_or_else(|| anyhow::anyhow!("Empty message"))?;
        let event: FeedbackEvent = serde_json::from_slice(payload)?;

        let timestamp = event.timestamp.timestamp_millis();
        let partition = message.partition() as u32;

        // Update watermark
        self.watermark_gen.on_event(timestamp, partition);

        // Check if late
        if self.watermark_gen.is_late_event(timestamp) {
            let lateness = self.watermark_gen.get_lateness(timestamp);

            if !self.late_handler.handle_late_event(lateness, partition) {
                // Too late - skip processing
                return Ok(());
            }
        }

        // Process event
        self.process_feedback_event(event).await?;

        // Commit offset
        self.consumer.commit_message(&message, rdkafka::consumer::CommitMode::Async)?;

        Ok(())
    }

    async fn process_feedback_event(&self, event: FeedbackEvent) -> anyhow::Result<()> {
        // Process the event
        Ok(())
    }

    pub fn get_statistics(&self) -> ConsumerStatistics {
        let late_stats = self.late_handler.get_stats();
        ConsumerStatistics {
            current_watermark: self.watermark_gen.current_watermark(),
            active_partitions: self.watermark_gen.active_partitions(),
            idle_partitions: self.watermark_gen.idle_partitions(),
            late_events_accepted: late_stats.accepted_count,
            late_events_dropped: late_stats.dropped_count,
            avg_lateness_ms: late_stats.average_lateness_ms,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConsumerStatistics {
    pub current_watermark: Watermark,
    pub active_partitions: Vec<u32>,
    pub idle_partitions: Vec<u32>,
    pub late_events_accepted: u64,
    pub late_events_dropped: u64,
    pub avg_lateness_ms: i64,
}
```

### 6. Monitoring and Metrics

```rust
use processor::watermark::{Watermark, LateEventStats};
use prometheus_client::metrics::{counter::Counter, gauge::Gauge, histogram::Histogram};

/// Watermark metrics for Prometheus
pub struct WatermarkMetrics {
    current_watermark: Gauge,
    watermark_lag: Gauge,
    late_events_accepted: Counter,
    late_events_dropped: Counter,
    lateness_histogram: Histogram,
}

impl WatermarkMetrics {
    pub fn new(registry: &mut prometheus_client::registry::Registry) -> Self {
        let current_watermark = Gauge::default();
        registry.register(
            "watermark_current_timestamp",
            "Current watermark timestamp in milliseconds",
            current_watermark.clone(),
        );

        let watermark_lag = Gauge::default();
        registry.register(
            "watermark_lag_ms",
            "Lag between current time and watermark",
            watermark_lag.clone(),
        );

        let late_events_accepted = Counter::default();
        registry.register(
            "late_events_accepted_total",
            "Total number of late events accepted",
            late_events_accepted.clone(),
        );

        let late_events_dropped = Counter::default();
        registry.register(
            "late_events_dropped_total",
            "Total number of late events dropped",
            late_events_dropped.clone(),
        );

        let lateness_histogram = Histogram::new([10.0, 50.0, 100.0, 500.0, 1000.0, 5000.0].into_iter());
        registry.register(
            "late_event_lateness_ms",
            "Distribution of late event lateness in milliseconds",
            lateness_histogram.clone(),
        );

        Self {
            current_watermark,
            watermark_lag,
            late_events_accepted,
            late_events_dropped,
            lateness_histogram,
        }
    }

    pub fn update_watermark(&self, watermark: Watermark) {
        self.current_watermark.set(watermark.timestamp);

        let now = chrono::Utc::now().timestamp_millis();
        let lag = now - watermark.timestamp;
        self.watermark_lag.set(lag);
    }

    pub fn update_late_event_stats(&self, stats: &LateEventStats) {
        self.late_events_accepted.inc_by(stats.accepted_count);
        self.late_events_dropped.inc_by(stats.dropped_count);

        if stats.accepted_count > 0 {
            self.lateness_histogram.observe(stats.average_lateness_ms as f64);
        }
    }
}
```

## Configuration Examples

### Development Environment
```yaml
# config/development.yaml
processor:
  watermark:
    strategy: bounded_out_of_orderness
    max_out_of_orderness_secs: 10
    idle_timeout_secs: 300
    emit_per_event: false
  late_events:
    allowed_lateness_secs: 30
    log_late_events: true
    emit_metrics: true
```

### Production Environment
```yaml
# config/production.yaml
processor:
  watermark:
    strategy: bounded_out_of_orderness
    max_out_of_orderness_secs: 5
    idle_timeout_secs: 600
    emit_per_event: false
  late_events:
    allowed_lateness_secs: 60
    log_late_events: false  # Reduce logging overhead
    emit_metrics: true
```

### High-Latency Environment
```yaml
# config/high_latency.yaml
processor:
  watermark:
    strategy: bounded_out_of_orderness
    max_out_of_orderness_secs: 30
    idle_timeout_secs: 900
    emit_per_event: false
  late_events:
    allowed_lateness_secs: 120
    log_late_events: true
    emit_metrics: true
```

## Best Practices

### 1. Choose Appropriate Bounds
- Start with conservative bounds (larger delays)
- Monitor late event statistics
- Gradually reduce bounds based on observed behavior

### 2. Monitor Watermark Lag
- Track difference between current time and watermark
- Alert if lag exceeds threshold
- Investigate sources of delay

### 3. Handle Idle Partitions
- Configure reasonable idle timeouts
- Don't let idle partitions block watermark progression
- Monitor partition activity

### 4. Log Strategically
- Enable detailed logging in development
- Reduce logging in production (performance)
- Always emit metrics for monitoring

### 5. Test Edge Cases
- Very late events (beyond allowed lateness)
- Idle partitions
- Clock skew between sources
- Burst of out-of-order events

## Troubleshooting

### Watermark Not Advancing
- Check if all partitions are active
- Verify idle timeout configuration
- Look for stuck partitions

### Too Many Late Events
- Increase max out-of-orderness
- Increase allowed lateness
- Investigate event time assignment

### High Memory Usage
- Check number of open windows
- Verify windows are being finalized
- Ensure watermark is advancing

### Partition Skew
- Monitor per-partition watermarks
- Balance partition assignment
- Consider partition merging
