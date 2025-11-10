# Time-Series Normalization - Quick Start Guide

## Installation

The normalization module is part of the `processor` crate. Import it:

```rust
use processor::normalization::{
    TimeSeriesNormalizer, NormalizerBuilder, NormalizationConfig,
    FillStrategy, TimeSeriesEvent, NormalizationStats
};
```

## 30-Second Start

```rust
use processor::normalization::*;
use chrono::Utc;

// 1. Create normalizer (1-second intervals)
let normalizer = NormalizerBuilder::new()
    .interval_ms(1000)
    .fill_strategy(FillStrategy::LinearInterpolate)
    .build()?;

// 2. Process events
let event = TimeSeriesEvent::new(Utc::now(), 42.0);
let results = normalizer.normalize(event)?;

// 3. Done! Results contain normalized events
for result in results {
    println!("{}: {}", result.timestamp, result.value);
}
```

## Common Patterns

### 1. Fixed Interval with Linear Interpolation (Recommended)
```rust
let normalizer = NormalizerBuilder::new()
    .interval_ms(1000)  // 1 second
    .fill_strategy(FillStrategy::LinearInterpolate)
    .build()?;
```

### 2. Auto-Detect Interval
```rust
let normalizer = NormalizerBuilder::new()
    .fill_strategy(FillStrategy::LinearInterpolate)
    .min_samples_for_detection(10)
    .build()?;

// Interval will be detected after 10 samples
```

### 3. Forward Fill (Fastest)
```rust
let normalizer = NormalizerBuilder::new()
    .interval_ms(5000)  // 5 seconds
    .fill_strategy(FillStrategy::ForwardFill)
    .build()?;
```

### 4. Sparse Output (Skip Missing)
```rust
let normalizer = NormalizerBuilder::new()
    .interval_ms(1000)
    .fill_strategy(FillStrategy::Skip)
    .build()?;
```

### 5. Batch Processing
```rust
let events = vec![
    TimeSeriesEvent::new(timestamp1, 10.0),
    TimeSeriesEvent::new(timestamp2, 20.0),
    TimeSeriesEvent::new(timestamp3, 30.0),
];

let results = normalizer.normalize_batch(events)?;
```

## Fill Strategy Cheat Sheet

| Strategy | Use Case | Speed | Memory | Output |
|----------|----------|-------|--------|--------|
| `Skip` | Sparse data, no gaps | ⚡⚡⚡ Fastest | ✓ Low | Sparse |
| `ForwardFill` | Step functions | ⚡⚡ Fast | ✓ Low | Dense |
| `BackwardFill` | Predictions | ⚡⚡ Fast | ⚠ Medium | Dense |
| `LinearInterpolate` | Continuous data | ⚡ Moderate | ⚠ Medium | Dense |
| `Zero` | Rate metrics | ⚡⚡⚡ Fastest | ✓ Low | Dense |

## Configuration Quick Reference

```rust
NormalizerBuilder::new()
    .interval_ms(1000)              // Target interval (ms) or None for auto
    .fill_strategy(FillStrategy::LinearInterpolate)  // Gap filling
    .max_buffer_size(1000)          // Event buffer size
    .max_out_of_order_ms(60000)     // Late event tolerance (ms)
    .min_samples_for_detection(10)  // Samples for auto-detect
    .interval_tolerance(0.1)        // 10% detection tolerance
    .warn_on_late_events(true)      // Log warnings
    .build()?;
```

## Complete Example

```rust
use processor::normalization::*;
use chrono::{Utc, Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Configure normalizer
    let normalizer = NormalizerBuilder::new()
        .interval_ms(1000)  // 1-second intervals
        .fill_strategy(FillStrategy::LinearInterpolate)
        .max_buffer_size(500)
        .max_out_of_order_ms(30000)  // 30 seconds tolerance
        .build()?;

    // 2. Create sample events with gaps
    let base = Utc::now();
    let events = vec![
        TimeSeriesEvent::new(base, 10.0),
        TimeSeriesEvent::new(base + Duration::seconds(1), 20.0),
        // Gap here - will be filled
        TimeSeriesEvent::new(base + Duration::seconds(4), 50.0),
    ];

    // 3. Process events
    let mut all_results = Vec::new();
    for event in events {
        let results = normalizer.normalize(event)?;
        all_results.extend(results);
    }

    // 4. Flush remaining buffered events
    let final_results = normalizer.flush()?;
    all_results.extend(final_results);

    // 5. Check statistics
    let stats = normalizer.stats()?;
    println!("Processed: {} events", stats.events_received);
    println!("Normalized: {} points", stats.events_normalized);
    println!("Filled: {} points", stats.points_filled);
    println!("Fill rate: {:.1}%", stats.fill_rate() * 100.0);
    println!("Avg latency: {:.2}µs", stats.avg_processing_time_us());

    Ok(())
}
```

## Monitoring

### Get Statistics
```rust
let stats = normalizer.stats()?;

println!("Events: received={}, normalized={}",
    stats.events_received,
    stats.events_normalized);

println!("Dropped: late={}, overflow={}",
    stats.events_dropped_late,
    stats.events_dropped_overflow);

println!("Buffer: current={}, max={}",
    stats.current_buffer_size,
    stats.max_buffer_size_reached);

println!("Metrics: fill_rate={:.2}%, drop_rate={:.2}%",
    stats.fill_rate() * 100.0,
    stats.drop_rate() * 100.0);
```

### Check Detected Interval
```rust
if let Some(interval) = normalizer.interval_ms()? {
    println!("Using interval: {}ms", interval);
}
```

### Reset State
```rust
normalizer.reset()?;
```

## Integration Examples

### With Stream Processor
```rust
use processor::{StreamProcessor, TimeSeriesNormalizer};

let normalizer = TimeSeriesNormalizer::default_config()?;

stream
    .flat_map(move |event| {
        normalizer.normalize(event).unwrap_or_default()
    })
    .for_each(|normalized| {
        // Process normalized event
    });
```

### With Kafka
```rust
use processor::kafka::KafkaSource;
use processor::normalization::TimeSeriesNormalizer;

let source = KafkaSource::new(kafka_config).await?;
let normalizer = TimeSeriesNormalizer::default_config()?;

while let Some(message) = source.next().await {
    let event = parse_to_timeseries(message)?;
    let normalized = normalizer.normalize(event)?;

    for output in normalized {
        publish_downstream(output).await?;
    }
}
```

### With Aggregation
```rust
use processor::aggregation::AvgAggregator;
use processor::normalization::TimeSeriesNormalizer;

let normalizer = TimeSeriesNormalizer::default_config()?;
let mut aggregator = AvgAggregator::new();

for event in raw_events {
    for normalized in normalizer.normalize(event)? {
        aggregator.update(normalized.value)?;
    }
}

let average = aggregator.finalize()?;
```

## Performance Tips

### 1. Use Batch Processing
```rust
// Good: Single lock acquisition
let results = normalizer.normalize_batch(events)?;

// Less efficient: Multiple lock acquisitions
for event in events {
    let results = normalizer.normalize(event)?;
}
```

### 2. Choose Appropriate Fill Strategy
- Use `Skip` if you don't need dense output
- Use `ForwardFill` for fastest gap filling
- Use `LinearInterpolate` only when accuracy matters

### 3. Tune Buffer Size
- Small buffer (100-500): Lower latency, may drop more
- Large buffer (1000-5000): Higher completeness, more memory

### 4. Adjust Out-of-Order Tolerance
- Short tolerance (1-10s): Faster processing
- Long tolerance (30-60s): Better completeness

### 5. Disable Warnings in Production
```rust
.warn_on_late_events(false)  // Reduce log volume
```

## Troubleshooting

### Events Being Dropped
**Problem**: `events_dropped_late` or `events_dropped_overflow` increasing

**Solutions**:
- Increase `max_out_of_order_ms`
- Increase `max_buffer_size`
- Check for clock skew in data sources

### High Memory Usage
**Problem**: Buffer consuming too much memory

**Solutions**:
- Decrease `max_buffer_size`
- Decrease `max_out_of_order_ms`
- Call `flush()` more frequently

### Interval Not Detected
**Problem**: `detected_interval_ms` is None

**Solutions**:
- Decrease `min_samples_for_detection`
- Increase `interval_tolerance`
- Check if data has regular intervals

### Slow Performance
**Problem**: High `avg_processing_time_us`

**Solutions**:
- Use simpler fill strategy (Skip or ForwardFill)
- Use batch processing
- Decrease buffer size
- Profile lock contention

## Common Mistakes

### ❌ Not Flushing at Stream End
```rust
// Wrong: Last events stay in buffer
for event in events {
    normalizer.normalize(event)?;
}
// Missing: normalizer.flush()?
```

### ❌ Ignoring Empty Results
```rust
// Wrong: Assumes always returns events
let result = normalizer.normalize(event)?;
let first = result[0];  // May panic!

// Right: Check result
let results = normalizer.normalize(event)?;
if !results.is_empty() {
    // Process results
}
```

### ❌ Blocking on Stats
```rust
// Wrong: Calling stats() too frequently
for event in millions_of_events {
    normalizer.normalize(event)?;
    let stats = normalizer.stats()?;  // Expensive!
}

// Right: Check stats periodically
for (i, event) in millions_of_events.iter().enumerate() {
    normalizer.normalize(event)?;
    if i % 10000 == 0 {
        let stats = normalizer.stats()?;
    }
}
```

## Need More Help?

- **Full Documentation**: See `README.md` in this directory
- **Implementation Details**: See `../NORMALIZATION_IMPLEMENTATION.md`
- **API Reference**: Check inline documentation with `cargo doc`
- **Tests**: See test functions in `mod.rs` for examples

## One-Line Solutions

| Task | Code |
|------|------|
| Default normalizer | `TimeSeriesNormalizer::default_config()?` |
| 1-sec intervals | `NormalizerBuilder::new().interval_ms(1000).build()?` |
| Auto-detect | `NormalizerBuilder::new().build()?` |
| Forward fill | `.fill_strategy(FillStrategy::ForwardFill)` |
| Sparse output | `.fill_strategy(FillStrategy::Skip)` |
| Get stats | `normalizer.stats()?` |
| Flush buffer | `normalizer.flush()?` |
| Reset state | `normalizer.reset()?` |
| Check interval | `normalizer.interval_ms()?` |

## License

See repository LICENSE file.
