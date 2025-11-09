# Watermark Module - Quick Reference

## File Location
```
/workspaces/llm-auto-optimizer/crates/processor/src/watermark.rs
```

## Quick Import
```rust
use processor::watermark::{
    Watermark, WatermarkGenerator,
    BoundedOutOfOrdernessWatermark,
    PeriodicWatermark, PunctuatedWatermark,
    LateEventHandler, LateEventConfig,
};
use std::time::Duration;
```

## 1-Minute Quick Start

### Basic Usage
```rust
// Create generator
let mut gen = BoundedOutOfOrdernessWatermark::new(
    Duration::from_secs(5),  // 5s out-of-orderness
    None
);

// Process event
gen.on_event(10000, 0);  // timestamp=10000ms, partition=0

// Check current watermark
let wm = gen.current_watermark();
println!("Watermark: {}", wm);  // Watermark: 5000

// Check if event is late
if gen.is_late_event(4000) {
    println!("Late by {}ms", gen.get_lateness(4000));
}
```

## Common Patterns Cheat Sheet

### Pattern 1: Simple Stream Processing
```rust
let mut gen = BoundedOutOfOrdernessWatermark::new(Duration::from_secs(5), None);

for event in events {
    gen.on_event(event.timestamp, event.partition);
    if !gen.is_late_event(event.timestamp) {
        process(event);
    }
}
```

### Pattern 2: With Late Event Handling
```rust
let mut gen = BoundedOutOfOrdernessWatermark::new(Duration::from_secs(5), None);
let handler = LateEventHandler::new(LateEventConfig {
    allowed_lateness: Duration::from_secs(10),
    log_late_events: true,
    emit_metrics: true,
});

for event in events {
    gen.on_event(event.timestamp, event.partition);

    if gen.is_late_event(event.timestamp) {
        let lateness = gen.get_lateness(event.timestamp);
        if handler.handle_late_event(lateness, event.partition) {
            process_late(event);
        }
    } else {
        process(event);
    }
}
```

### Pattern 3: Window Triggering
```rust
let mut gen = BoundedOutOfOrdernessWatermark::new(Duration::from_secs(5), None);

for event in events {
    if let Some(new_wm) = gen.on_event(event.timestamp, event.partition) {
        finalize_windows(new_wm);  // Watermark advanced
    }
    add_to_window(event);
}
```

### Pattern 4: Periodic Watermarks
```rust
let mut gen = PeriodicWatermark::new(PeriodicWatermarkConfig {
    interval: Duration::from_secs(1),
    max_out_of_orderness: Duration::from_secs(5),
});

// In event loop
gen.on_event(timestamp, partition);

// In periodic timer (every 1 second)
if let Some(wm) = gen.on_periodic_check() {
    emit_watermark(wm);
}
```

### Pattern 5: Punctuated Watermarks
```rust
// Every N events
let mut gen = PunctuatedWatermark::every_n_events(100);

// Time boundaries
let mut gen = PunctuatedWatermark::on_timestamp_boundary(60_000); // 1 min

// Custom
let gen = PunctuatedWatermark::new(|ts, part| {
    if is_watermark_event(ts) { Some(ts) } else { None }
});
```

## Key Methods Reference

### Watermark
```rust
Watermark::new(timestamp)              // Create from timestamp
Watermark::from_datetime(dt)           // Create from DateTime
wm.to_datetime()                       // Convert to DateTime
wm.is_before(timestamp)                // Compare
wm.is_after(timestamp)                 // Compare
Watermark::min() / Watermark::max()    // Boundaries
```

### WatermarkGenerator Trait
```rust
gen.on_event(timestamp, partition)     // Process event, returns Option<Watermark>
gen.on_periodic_check()                // Periodic check, returns Option<Watermark>
gen.current_watermark()                // Get current watermark
gen.reset()                            // Reset to initial state
gen.partition_watermark(partition)     // Get partition-specific watermark
```

### BoundedOutOfOrdernessWatermark
```rust
// Creation
BoundedOutOfOrdernessWatermark::new(max_delay, idle_timeout)
BoundedOutOfOrdernessWatermark::with_config(config)

// Late event methods
gen.is_late_event(timestamp) -> bool
gen.get_lateness(timestamp) -> i64

// Partition methods
gen.active_partitions() -> Vec<u32>
gen.idle_partitions() -> Vec<u32>
```

### LateEventHandler
```rust
// Creation
LateEventHandler::new(config)

// Handle event
handler.handle_late_event(lateness, partition) -> bool  // true = accept, false = drop

// Statistics
handler.get_stats() -> LateEventStats
handler.reset_stats()
```

## Configuration Quick Reference

### Development (Lenient)
```rust
BoundedOutOfOrdernessWatermark::new(
    Duration::from_secs(10),  // Large delay
    Some(Duration::from_secs(300))  // 5 min idle
)

LateEventConfig {
    allowed_lateness: Duration::from_secs(60),  // 1 minute
    log_late_events: true,
    emit_metrics: true,
}
```

### Production (Balanced)
```rust
BoundedOutOfOrdernessWatermark::new(
    Duration::from_secs(5),   // Moderate delay
    Some(Duration::from_secs(600))  // 10 min idle
)

LateEventConfig {
    allowed_lateness: Duration::from_secs(30),
    log_late_events: false,  // Performance
    emit_metrics: true,
}
```

### Real-time (Strict)
```rust
BoundedOutOfOrdernessWatermark::new(
    Duration::from_secs(1),   // Minimal delay
    Some(Duration::from_secs(60))
)

LateEventConfig {
    allowed_lateness: Duration::from_secs(5),
    log_late_events: false,
    emit_metrics: true,
}
```

## Common Issues & Solutions

### Issue: Watermark not advancing
**Solution:**
```rust
// Check for idle partitions
let idle = gen.idle_partitions();
println!("Idle: {:?}", idle);

// Check current watermark
println!("Watermark: {}", gen.current_watermark());
```

### Issue: Too many late events
**Solution:**
```rust
// Increase max out-of-orderness
BoundedOutOfOrdernessWatermark::new(
    Duration::from_secs(10),  // Increased from 5
    None
)
```

### Issue: High memory usage
**Solution:**
```rust
// Check if windows are being finalized
if let Some(wm) = gen.on_event(ts, part) {
    finalize_windows_before(wm);  // Clean up old windows
}
```

## Test Commands

```bash
# Run all watermark tests
cargo test -p processor watermark

# Run specific test
cargo test -p processor test_bounded_watermark_basic

# Run example
cargo run -p processor --example watermark_demo

# Build docs
cargo doc -p processor --no-deps --open
```

## Performance Characteristics

| Operation | Complexity | Notes |
|-----------|-----------|-------|
| `on_event()` | O(P) | P = partitions |
| `is_late_event()` | O(1) | Fast check |
| `get_lateness()` | O(1) | Fast check |
| `current_watermark()` | O(1) | Lock acquisition |
| `partition_watermark()` | O(1) | HashMap lookup |

Memory: O(P) where P = number of partitions

## Statistics

```rust
let stats = handler.get_stats();
println!("Accepted: {}", stats.accepted_count);
println!("Dropped: {}", stats.dropped_count);
println!("Avg lateness: {}ms", stats.average_lateness_ms);
```

## Typical Values

| Parameter | Low Latency | Moderate | High Latency |
|-----------|------------|----------|--------------|
| Max out-of-orderness | 1-2s | 5-10s | 30-60s |
| Allowed lateness | 5-10s | 30-60s | 2-5min |
| Idle timeout | 1min | 5min | 15min |
| Periodic interval | 100ms-1s | 1-5s | 5-30s |

## Examples Directory

See comprehensive examples in:
```
/workspaces/llm-auto-optimizer/crates/processor/examples/watermark_demo.rs
```

Covers:
- Bounded watermarks
- Multi-partition tracking
- Periodic emission
- Punctuated patterns
- Late event handling
- Complete pipelines

## Documentation

| File | Description |
|------|-------------|
| `WATERMARK_README.md` | Complete user guide |
| `WATERMARK_IMPLEMENTATION_SUMMARY.md` | Implementation details |
| `WATERMARK_INTEGRATION_GUIDE.md` | Integration examples |
| `WATERMARK_QUICK_REFERENCE.md` | This file |

## Module Size

- **Lines of code**: ~991
- **Tests**: 19 comprehensive tests
- **Examples**: 7 usage patterns
- **Documentation**: 4 comprehensive guides

---

**Pro Tip**: Start with `BoundedOutOfOrdernessWatermark` for most use cases. It's the most flexible and commonly used strategy.
