# Watermark Module

A comprehensive watermarking system for handling late events in stream processing.

## Overview

The watermark module provides robust mechanisms to handle out-of-order and late-arriving events in real-time data streams. Watermarks represent a timestamp threshold indicating that all events with timestamps below this threshold have been processed.

## Key Components

### 1. Watermark

The core data structure representing a watermark timestamp.

```rust
use processor::watermark::Watermark;

let watermark = Watermark::new(1000);
assert!(watermark.is_before(2000));
assert!(watermark.is_after(500));
```

**Features:**
- Timestamp-based ordering
- DateTime conversion utilities
- Min/Max constants for boundaries
- Display and comparison implementations

### 2. WatermarkGenerator Trait

The trait that all watermark generators must implement.

```rust
pub trait WatermarkGenerator: Send + Sync {
    fn on_event(&mut self, timestamp: i64, partition: u32) -> Option<Watermark>;
    fn on_periodic_check(&mut self) -> Option<Watermark>;
    fn current_watermark(&self) -> Watermark;
    fn reset(&mut self);
    fn partition_watermark(&self, partition: u32) -> Option<Watermark>;
}
```

### 3. BoundedOutOfOrdernessWatermark

The main watermark generator that handles bounded out-of-orderness.

```rust
use processor::watermark::{BoundedOutOfOrdernessWatermark, WatermarkGenerator};
use std::time::Duration;

let mut generator = BoundedOutOfOrdernessWatermark::new(
    Duration::from_secs(5),  // Max 5 seconds out-of-orderness
    Some(Duration::from_secs(60)) // 60 second idle timeout
);

// Process events
generator.on_event(10000, 0);
generator.on_event(15000, 0);
generator.on_event(12000, 0); // Late event within bounds

let watermark = generator.current_watermark();
```

**Features:**
- Tracks max timestamp per partition
- Applies configurable delay for out-of-orderness
- Merges partition watermarks (minimum across all partitions)
- Detects idle partitions
- Late event detection and lateness calculation

**Key Methods:**
- `is_late_event(timestamp)` - Check if an event is late
- `get_lateness(timestamp)` - Calculate how late an event is
- `active_partitions()` - Get list of active partitions
- `idle_partitions()` - Get list of idle partitions

### 4. PeriodicWatermark

Emits watermarks at regular intervals based on wall-clock time.

```rust
use processor::watermark::{PeriodicWatermark, PeriodicWatermarkConfig, WatermarkGenerator};
use std::time::Duration;

let config = PeriodicWatermarkConfig {
    interval: Duration::from_secs(1),
    max_out_of_orderness: Duration::from_secs(5),
};

let mut generator = PeriodicWatermark::new(config);

// Process events (doesn't emit on event)
generator.on_event(10000, 0);

// Check periodically
if let Some(watermark) = generator.on_periodic_check() {
    println!("Watermark: {}", watermark);
}
```

**Use Cases:**
- Systems requiring predictable watermark progression
- Time-based windowing with regular triggers
- Low-throughput streams that need guaranteed advancement

### 5. PunctuatedWatermark

Emits watermarks based on specific patterns or markers in the event stream.

```rust
use processor::watermark::{PunctuatedWatermark, WatermarkGenerator};

// Emit watermark every 3 events
let mut generator = PunctuatedWatermark::every_n_events(3);

generator.on_event(1000, 0); // No watermark
generator.on_event(2000, 0); // No watermark
if let Some(wm) = generator.on_event(3000, 0) {
    println!("Watermark emitted: {}", wm);
}

// Emit on timestamp boundaries (e.g., every 10 seconds)
let mut boundary_gen = PunctuatedWatermark::on_timestamp_boundary(10000);
```

**Pre-built Strategies:**
- `every_n_events(n)` - Emit every N events
- `on_timestamp_boundary(boundary_ms)` - Emit when crossing time boundaries

**Custom Strategy:**
```rust
let custom_gen = PunctuatedWatermark::new(|timestamp, partition| {
    // Custom logic to determine if watermark should be emitted
    if some_condition(timestamp) {
        Some(calculate_watermark(timestamp))
    } else {
        None
    }
});
```

### 6. LateEventHandler

Handles late events with configurable allowed lateness.

```rust
use processor::watermark::{LateEventHandler, LateEventConfig};
use std::time::Duration;

let config = LateEventConfig {
    allowed_lateness: Duration::from_secs(10),
    log_late_events: true,
    emit_metrics: true,
};

let handler = LateEventHandler::new(config);

// Handle a late event
let lateness_ms = 5000; // 5 seconds late
if handler.handle_late_event(lateness_ms, 0) {
    // Event accepted - process it
} else {
    // Event dropped - too late
}

// Get statistics
let stats = handler.get_stats();
println!("Accepted: {}, Dropped: {}", stats.accepted_count, stats.dropped_count);
```

**Features:**
- Configurable allowed lateness threshold
- Event acceptance/rejection logic
- Statistics tracking (accepted, dropped, average lateness)
- Optional logging and metrics emission

## Architecture

### Watermark Propagation

Watermarks flow through the system as follows:

1. **Event Arrival**: Events arrive with timestamps
2. **Watermark Generation**: Generator produces watermark based on strategy
3. **Watermark Advancement**: Only advance if new watermark is greater
4. **Late Event Detection**: Compare event timestamp with current watermark
5. **Late Event Handling**: Accept or drop based on allowed lateness

### Per-Partition Watermarks

The system maintains watermarks per partition and merges them:

```
Partition 0: Events at 10s, 15s, 20s → Watermark at 15s (20s - 5s delay)
Partition 1: Events at 5s, 8s, 12s  → Watermark at 7s (12s - 5s delay)
Partition 2: Events at 18s, 22s     → Watermark at 17s (22s - 5s delay)

Global Watermark: min(15s, 7s, 17s) = 7s
```

This ensures no partition is left behind and data completeness is maintained.

### Idle Partition Detection

Partitions that haven't received events within the idle timeout are marked as idle:

```rust
let mut generator = BoundedOutOfOrdernessWatermark::new(
    Duration::from_secs(5),
    Some(Duration::from_secs(60)), // Idle timeout
);

// After 60 seconds of inactivity...
let idle = generator.idle_partitions();
let active = generator.active_partitions();

// Only active partitions contribute to global watermark
```

## Usage Patterns

### Pattern 1: Simple Event Stream

```rust
use processor::watermark::{BoundedOutOfOrdernessWatermark, WatermarkGenerator};
use std::time::Duration;

let mut generator = BoundedOutOfOrdernessWatermark::new(
    Duration::from_secs(5),
    None,
);

for event in event_stream {
    let timestamp = event.timestamp_millis();
    let partition = event.partition();

    generator.on_event(timestamp, partition);

    if generator.is_late_event(timestamp) {
        // Handle late event
    } else {
        // Process on-time event
    }
}
```

### Pattern 2: Windowed Processing

```rust
use processor::watermark::{BoundedOutOfOrdernessWatermark, WatermarkGenerator};
use std::time::Duration;

let mut generator = BoundedOutOfOrdernessWatermark::new(
    Duration::from_secs(10),
    None,
);

for event in event_stream {
    let timestamp = event.timestamp_millis();

    if let Some(new_watermark) = generator.on_event(timestamp, event.partition()) {
        // Watermark advanced - trigger window computations
        trigger_window_computation(new_watermark);
    }
}
```

### Pattern 3: Complete Pipeline with Late Event Handling

```rust
use processor::watermark::{
    BoundedOutOfOrdernessWatermark, LateEventHandler,
    LateEventConfig, WatermarkGenerator
};
use std::time::Duration;

let mut watermark_gen = BoundedOutOfOrdernessWatermark::new(
    Duration::from_secs(5),
    None,
);

let late_handler = LateEventHandler::new(LateEventConfig {
    allowed_lateness: Duration::from_secs(10),
    log_late_events: true,
    emit_metrics: true,
});

for event in event_stream {
    let timestamp = event.timestamp_millis();

    watermark_gen.on_event(timestamp, event.partition());

    if watermark_gen.is_late_event(timestamp) {
        let lateness = watermark_gen.get_lateness(timestamp);

        if late_handler.handle_late_event(lateness, event.partition()) {
            // Event within allowed lateness - process it
            process_late_event(event);
        } else {
            // Event too late - drop it
            metrics::increment_dropped_events();
        }
    } else {
        // On-time event - normal processing
        process_event(event);
    }
}

// Periodically emit statistics
let stats = late_handler.get_stats();
log::info!("Late events - Accepted: {}, Dropped: {}, Avg lateness: {}ms",
    stats.accepted_count, stats.dropped_count, stats.average_lateness_ms);
```

### Pattern 4: Multi-Strategy Watermarking

```rust
use processor::watermark::{PeriodicWatermark, PeriodicWatermarkConfig, WatermarkGenerator};
use std::time::Duration;

// Use periodic watermarks for guaranteed progression
let config = PeriodicWatermarkConfig {
    interval: Duration::from_secs(1),
    max_out_of_orderness: Duration::from_secs(5),
};

let mut generator = PeriodicWatermark::new(config);

// Process events
for event in event_stream {
    generator.on_event(event.timestamp_millis(), event.partition());
}

// Periodic check (e.g., every second)
loop {
    std::thread::sleep(Duration::from_secs(1));

    if let Some(watermark) = generator.on_periodic_check() {
        // Emit watermark downstream
        emit_watermark(watermark);
    }
}
```

## Configuration Best Practices

### Choosing Max Out-of-Orderness

The max out-of-orderness parameter should be based on:

- **Network latency**: Higher latency requires larger bounds
- **Data freshness requirements**: Smaller bounds for real-time systems
- **Cost of late events**: If late events are expensive, use larger bounds

```rust
// Low latency, real-time analytics
BoundedOutOfOrdernessWatermark::new(Duration::from_secs(1), None)

// Moderate latency, balanced approach
BoundedOutOfOrdernessWatermark::new(Duration::from_secs(5), None)

// High latency, batch-like processing
BoundedOutOfOrdernessWatermark::new(Duration::from_secs(30), None)
```

### Choosing Allowed Lateness

The allowed lateness parameter determines how late an event can be and still be processed:

```rust
// Strict - drop events late by more than 10 seconds
LateEventConfig {
    allowed_lateness: Duration::from_secs(10),
    ..Default::default()
}

// Lenient - accept events late by up to 5 minutes
LateEventConfig {
    allowed_lateness: Duration::from_secs(300),
    ..Default::default()
}

// Zero tolerance - drop all late events
LateEventConfig {
    allowed_lateness: Duration::from_secs(0),
    ..Default::default()
}
```

### Idle Timeout Configuration

Set idle timeout based on expected partition activity:

```rust
// Active streams - 1 minute timeout
Some(Duration::from_secs(60))

// Moderate activity - 5 minute timeout
Some(Duration::from_secs(300))

// Sparse streams - 15 minute timeout
Some(Duration::from_secs(900))

// Never timeout (not recommended for production)
None
```

## Performance Considerations

### Memory Usage

- **Per-partition state**: O(P) where P is the number of partitions
- **Watermark storage**: Minimal (one watermark per partition)
- **Statistics**: O(1) for counters

### Computational Complexity

- **on_event()**: O(P) for partition watermark merging
- **is_late_event()**: O(1) watermark comparison
- **partition_watermark()**: O(1) lookup

### Optimization Tips

1. **Use emit_per_event: false** for high-throughput streams
2. **Increase idle timeout** for many partitions to reduce overhead
3. **Disable logging** in production (`log_late_events: false`)
4. **Batch periodic checks** instead of checking every millisecond

## Testing

The module includes comprehensive tests covering:

- Watermark creation and ordering
- Bounded watermark with in-order events
- Out-of-order event handling
- Multi-partition watermark merging
- Late event detection and lateness calculation
- Periodic watermark emission
- Punctuated watermark patterns
- Late event handler acceptance/rejection
- Idle partition detection
- Watermark monotonicity guarantees

Run tests with:

```bash
cargo test -p processor watermark
```

## Examples

See `examples/watermark_demo.rs` for a comprehensive demonstration:

```bash
cargo run -p processor --example watermark_demo
```

The demo covers:
1. Basic bounded watermark usage
2. Multi-partition watermark tracking
3. Periodic watermark emission
4. Punctuated watermark patterns
5. Timestamp boundary watermarks
6. Late event handling
7. Complete pipeline integration

## Integration with LLM Auto-Optimizer

The watermark module integrates with the optimizer's stream processing pipeline:

```rust
use processor::watermark::{BoundedOutOfOrdernessWatermark, WatermarkGenerator};
use llm_optimizer_types::events::FeedbackEvent;

let mut watermark_gen = BoundedOutOfOrdernessWatermark::new(
    Duration::from_secs(5),
    None,
);

// Process feedback events
for event in feedback_stream {
    let timestamp = event.timestamp.timestamp_millis();
    let partition = calculate_partition(&event);

    watermark_gen.on_event(timestamp, partition);

    if !watermark_gen.is_late_event(timestamp) {
        process_feedback(event);
    }
}
```

## Future Enhancements

Potential improvements:

1. **Adaptive watermarking**: Automatically adjust out-of-orderness based on observed latency
2. **Watermark strategies**: More sophisticated strategies (percentile-based, ML-based)
3. **Distributed watermarking**: Coordinate watermarks across multiple processing nodes
4. **Watermark persistence**: Save/restore watermark state for fault tolerance
5. **Custom extractors**: User-defined timestamp extraction from events
6. **Watermark operators**: Compose multiple watermark strategies

## References

- Apache Flink Watermarks: https://nightlies.apache.org/flink/flink-docs-master/docs/concepts/time/
- Google Dataflow Model: https://research.google/pubs/pub43864/
- Streaming Systems Book: "Streaming Systems" by Akidau, Chernyak, and Lax
