# Time-Series Normalization Integration

This document describes the integration of time-series normalization into the Stream Processor configuration and pipeline.

## Overview

Time-series normalization has been integrated into the processor crate to provide:
- Configurable time-series data alignment to fixed intervals
- Multiple fill strategies for missing data points
- Out-of-order event handling with buffering
- Outlier detection and removal
- Seamless integration with existing pipeline operators

## Architecture

### Configuration Layer

The normalization system is configured through the `NormalizationConfig` struct, which is part of the main `ProcessorConfig`:

```rust
pub struct ProcessorConfig {
    pub window: WindowConfig,
    pub watermark: WatermarkConfig,
    pub state: StateConfig,
    pub aggregation: AggregationConfig,
    pub deduplication: DeduplicationConfig,
    pub normalization: NormalizationConfig,  // NEW
    // ... other fields
}
```

### NormalizationConfig Structure

```rust
pub struct NormalizationConfig {
    pub enabled: bool,
    pub interval: Duration,
    pub fill_strategy: FillStrategy,
    pub alignment: AlignmentStrategy,
    pub buffer_size: usize,
    pub interpolation_max_gap: Duration,
    pub drop_outliers: bool,
    pub outlier_threshold: f64,
    pub emit_metrics: bool,
}
```

#### Fill Strategies

The system supports multiple strategies for handling missing data:

- **Zero**: Fill with zero value
- **ForwardFill**: Use last known value (default)
- **BackwardFill**: Use next known value
- **LinearInterpolation**: Interpolate linearly between known values
- **Drop**: Skip missing intervals
- **Constant**: Fill with a constant value
- **Mean**: Use mean of surrounding values

#### Alignment Strategies

Timestamps can be aligned to interval boundaries using:

- **Start**: Align to start of interval (default)
- **End**: Align to end of interval
- **Center**: Align to center of interval
- **Nearest**: Round to nearest interval boundary

### Pipeline Integration

The `NormalizationOperator` implements the `StreamOperator` trait and can be inserted into the processing pipeline:

```rust
pub struct NormalizationOperator {
    name: String,
    config: NormalizationConfig,
    buffer: Arc<RwLock<BTreeMap<i64, Vec<f64>>>>,
    last_emitted: Arc<RwLock<Option<DateTime<Utc>>>>,
    stats: Arc<RwLock<NormalizationStats>>,
    recent_values: Arc<RwLock<VecDeque<f64>>>,
}
```

#### Processing Flow

1. **Event Reception**: Timestamped events enter the operator
2. **Outlier Detection**: Events are checked for outliers using z-score
3. **Timestamp Alignment**: Event timestamps are aligned to configured intervals
4. **Buffering**: Events are buffered to handle out-of-order arrivals
5. **Gap Filling**: Missing intervals are filled using the configured strategy
6. **Emission**: Aligned events are emitted at consistent intervals

## Usage

### Basic Configuration

```rust
use processor::config::{NormalizationConfig, FillStrategy, AlignmentStrategy};
use std::time::Duration;

// Create normalization config
let norm_config = NormalizationConfig::new()
    .enabled()
    .with_interval(Duration::from_secs(5))
    .with_fill_strategy(FillStrategy::LinearInterpolation)
    .with_alignment(AlignmentStrategy::Start)
    .with_buffer_size(1000);
```

### Pipeline Builder Integration

```rust
use processor::pipeline::StreamPipelineBuilder;
use std::time::Duration;

let pipeline = StreamPipelineBuilder::new()
    .with_name("normalized-pipeline")
    .with_normalization(true, Duration::from_secs(5))
    .with_fill_strategy(FillStrategy::LinearInterpolation)
    .with_alignment_strategy(AlignmentStrategy::Start)
    .with_normalization_buffer_size(2000)
    .with_interpolation_max_gap(Duration::from_secs(15))
    .with_normalization_outlier_detection(3.0)
    .build()?;
```

### Using NormalizationConfig Directly

```rust
let norm_config = NormalizationConfig::new()
    .enabled()
    .with_interval(Duration::from_secs(5))
    .with_fill_strategy(FillStrategy::ForwardFill)
    .with_alignment(AlignmentStrategy::End)
    .with_outlier_detection(2.5);

let pipeline = StreamPipelineBuilder::new()
    .with_name("pipeline")
    .with_normalization_config(norm_config)
    .build()?;
```

### Creating a Normalization Operator

```rust
use processor::pipeline::NormalizationOperator;

let config = NormalizationConfig::new()
    .enabled()
    .with_interval(Duration::from_secs(5));

let operator = NormalizationOperator::new("normalization", config);

// Add to pipeline
pipeline.add_operator(operator);
```

## Builder Methods

The `StreamPipelineBuilder` provides the following normalization-related methods:

### Basic Configuration

- `with_normalization(enabled: bool, interval: Duration)` - Enable normalization with interval
- `with_normalization_config(config: NormalizationConfig)` - Set complete config
- `with_normalization_interval(interval: Duration)` - Set alignment interval

### Strategy Configuration

- `with_fill_strategy(strategy: FillStrategy)` - Set fill strategy
- `with_alignment_strategy(strategy: AlignmentStrategy)` - Set alignment strategy

### Advanced Options

- `with_normalization_buffer_size(size: usize)` - Set buffer size for out-of-order events
- `with_interpolation_max_gap(gap: Duration)` - Set max gap for interpolation
- `with_normalization_outlier_detection(threshold: f64)` - Enable outlier detection

## Statistics

The `NormalizationOperator` tracks the following statistics:

```rust
pub struct NormalizationStats {
    pub events_processed: u64,
    pub events_aligned: u64,
    pub intervals_filled: u64,
    pub outliers_dropped: u64,
    pub out_of_order_events: u64,
    pub events_dropped: u64,
}
```

Access statistics via:

```rust
let stats = operator.stats();
println!("Events processed: {}", stats.events_processed);
println!("Outliers dropped: {}", stats.outliers_dropped);
```

## Configuration Validation

The configuration is automatically validated when building a pipeline:

```rust
impl NormalizationConfig {
    pub fn validate(&self) -> Result<()> {
        // Validates:
        // - interval > 0
        // - buffer_size > 0
        // - interpolation_max_gap > 0
        // - outlier_threshold > 0
        // - interpolation_max_gap >= interval
    }
}
```

## Integration Points

### Before Windowing

Normalization should be applied **before** windowing operations to ensure:
- Consistent event time alignment
- Reduced skew in time-based aggregations
- Proper handling of out-of-order events

### Pipeline Position

Recommended operator order:
1. Deduplication (optional)
2. **Normalization** (if enabled)
3. Key-by (partitioning)
4. Windowing
5. Aggregation

### Example Complete Pipeline

```rust
use processor::pipeline::{StreamPipelineBuilder, NormalizationOperator, DeduplicationOperator};
use processor::config::{
    DeduplicationConfig, NormalizationConfig,
    FillStrategy, AlignmentStrategy
};
use std::time::Duration;

// Configure deduplication
let dedup_config = DeduplicationConfig::new()
    .enabled()
    .with_ttl(Duration::from_secs(3600));

// Configure normalization
let norm_config = NormalizationConfig::new()
    .enabled()
    .with_interval(Duration::from_secs(5))
    .with_fill_strategy(FillStrategy::LinearInterpolation)
    .with_alignment(AlignmentStrategy::Start);

// Build pipeline
let mut pipeline = StreamPipelineBuilder::new()
    .with_name("production-pipeline")
    .with_deduplication_config(dedup_config)
    .with_normalization_config(norm_config)
    .with_tumbling_window(60_000)  // 1-minute windows
    .with_parallelism(8)
    .build()?;

// Add operators in order
pipeline.add_operator(DeduplicationOperator::new("dedup", dedup_config));
pipeline.add_operator(NormalizationOperator::new("norm", norm_config));
// ... add other operators
```

## Event Types

The normalization operator works with `TimestampedEvent`:

```rust
pub struct TimestampedEvent {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
}
```

Events are serialized/deserialized using bincode for efficient processing.

## Outlier Detection

Outlier detection uses z-score method:

1. Maintains a rolling window of recent values (up to 100)
2. Calculates mean and standard deviation
3. Computes z-score: `|value - mean| / std_dev`
4. Drops events where z-score > threshold

Default threshold: 3.0 standard deviations

## Performance Considerations

### Buffer Management

- Buffer size limits memory usage
- Oldest events dropped on overflow
- Configurable via `buffer_size` parameter
- Default: 1000 events

### Gap Filling

- Only fills gaps up to `interpolation_max_gap`
- Linear interpolation most CPU-intensive
- ForwardFill/BackwardFill most efficient
- Drop strategy skips gap filling entirely

### Metrics Emission

- Can be disabled to reduce overhead
- Set `emit_metrics: false` in config

## Testing

Comprehensive tests are included:

- Configuration validation
- Alignment strategy tests
- Fill strategy tests
- Outlier detection
- Buffer overflow handling
- Statistics tracking
- Builder API fluency

Run tests with:

```bash
cargo test --package processor normalization
```

## Backward Compatibility

The integration maintains backward compatibility:

- Normalization disabled by default
- All existing pipelines work unchanged
- Optional configuration field
- No breaking changes to existing APIs

## Example Use Cases

### High-Frequency Metrics

```rust
// Normalize high-frequency metrics to 1-second intervals
let config = NormalizationConfig::new()
    .enabled()
    .with_interval(Duration::from_secs(1))
    .with_fill_strategy(FillStrategy::LinearInterpolation)
    .with_outlier_detection(3.0);
```

### Sparse Data

```rust
// Handle sparse data with forward fill
let config = NormalizationConfig::new()
    .enabled()
    .with_interval(Duration::from_secs(60))
    .with_fill_strategy(FillStrategy::ForwardFill)
    .with_interpolation_max_gap(Duration::from_secs(300));
```

### Real-Time Analytics

```rust
// Real-time with large buffer for out-of-order tolerance
let config = NormalizationConfig::new()
    .enabled()
    .with_interval(Duration::from_secs(5))
    .with_buffer_size(5000)
    .with_fill_strategy(FillStrategy::LinearInterpolation)
    .with_alignment(AlignmentStrategy::Start);
```

## Files Modified

### New Files

- `crates/processor/src/pipeline/normalization_operator.rs` - Normalization operator implementation

### Modified Files

- `crates/processor/src/config.rs` - Added NormalizationConfig, FillStrategy, AlignmentStrategy
- `crates/processor/src/lib.rs` - Exported new types
- `crates/processor/src/pipeline/mod.rs` - Added normalization operator module
- `crates/processor/src/pipeline/builder.rs` - Added builder methods

## Future Enhancements

Potential improvements:

1. **Advanced Interpolation**: Spline, polynomial interpolation
2. **Adaptive Intervals**: Dynamic interval adjustment based on data density
3. **Multi-value Aggregation**: Support for events with multiple metric values
4. **Persistence**: Save/restore operator state for recovery
5. **Metrics Integration**: Deeper integration with existing metrics framework
6. **Custom Fill Functions**: User-defined fill strategies

## References

- Main config: `crates/processor/src/config.rs`
- Operator implementation: `crates/processor/src/pipeline/normalization_operator.rs`
- Builder API: `crates/processor/src/pipeline/builder.rs`
- Tests: Included in each file's test module
