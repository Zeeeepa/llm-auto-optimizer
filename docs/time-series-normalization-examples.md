# Time-Series Normalization Implementation Examples

**Practical Guide and Code Examples**
**Version:** 1.0

---

## Table of Contents

1. [Basic Usage Examples](#basic-usage-examples)
2. [Advanced Configuration](#advanced-configuration)
3. [Integration Patterns](#integration-patterns)
4. [Real-World Scenarios](#real-world-scenarios)
5. [Testing Examples](#testing-examples)
6. [Performance Tuning](#performance-tuning)
7. [Troubleshooting](#troubleshooting)

---

## Basic Usage Examples

### Example 1: Simple Metric Normalization

```rust
use processor::normalization::{
    TimeSeriesNormalizer,
    NormalizationConfig,
    AlignmentStrategy,
    FillStrategyType,
};
use chrono::{Duration, Utc};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a normalizer for 5-second intervals
    let config = NormalizationConfig {
        normalization_interval: Duration::seconds(5),
        alignment_strategy: AlignmentStrategy::RoundDown,
        fill_strategy: FillStrategyType::ForwardFill,
        ..Default::default()
    };

    let normalizer = TimeSeriesNormalizer::<f64>::new(config);

    // Process incoming metrics
    let metrics = vec![
        (100.5, Utc::now()),
        (102.3, Utc::now() + Duration::seconds(4)),
        (105.1, Utc::now() + Duration::seconds(11)),
    ];

    for (value, timestamp) in metrics {
        let normalized = normalizer
            .process_event(value, timestamp, "latency")
            .await?;

        for event in normalized {
            println!("Normalized: {:?}", event);
        }
    }

    Ok(())
}
```

**Output:**
```
Normalized: NormalizedEvent { timestamp: 2025-11-10T10:00:00Z, value: 100.5, is_filled: false }
Normalized: NormalizedEvent { timestamp: 2025-11-10T10:00:05Z, value: 102.3, is_filled: false }
Normalized: NormalizedEvent { timestamp: 2025-11-10T10:00:10Z, value: 105.1, is_filled: false }
```

### Example 2: Gap Filling with Linear Interpolation

```rust
use processor::normalization::*;

async fn normalize_with_gaps() -> Result<(), Box<dyn std::error::Error>> {
    let config = NormalizationConfig {
        normalization_interval: Duration::seconds(5),
        alignment_strategy: AlignmentStrategy::RoundDown,
        fill_strategy: FillStrategyType::LinearInterpolate,
        ..Default::default()
    };

    let normalizer = TimeSeriesNormalizer::<f64>::new(config);

    // Events with a gap
    let events = vec![
        (100.0, Utc::now()),
        // 20-second gap here
        (120.0, Utc::now() + Duration::seconds(25)),
    ];

    // Update watermark to trigger normalization
    normalizer.update_watermark(Utc::now() + Duration::seconds(30)).await;

    for (value, timestamp) in events {
        let normalized = normalizer
            .process_event(value, timestamp, "sensor_temp")
            .await?;

        for event in normalized {
            println!(
                "Time: {}, Value: {}, Filled: {}",
                event.timestamp,
                event.value,
                event.is_filled
            );
        }
    }

    Ok(())
}
```

**Output:**
```
Time: 2025-11-10T10:00:00Z, Value: 100.0, Filled: false
Time: 2025-11-10T10:00:05Z, Value: 105.0, Filled: true   // Interpolated
Time: 2025-11-10T10:00:10Z, Value: 110.0, Filled: true   // Interpolated
Time: 2025-11-10T10:00:15Z, Value: 115.0, Filled: true   // Interpolated
Time: 2025-11-10T10:00:20Z, Value: 120.0, Filled: false
```

### Example 3: Out-of-Order Event Handling

```rust
async fn handle_out_of_order() -> Result<(), Box<dyn std::error::Error>> {
    let config = NormalizationConfig {
        normalization_interval: Duration::seconds(10),
        alignment_strategy: AlignmentStrategy::RoundDown,
        fill_strategy: FillStrategyType::ForwardFill,
        buffer_capacity: 1000,
        buffer_retention: Duration::minutes(5),
        allow_late_events: true,
        max_lateness: Duration::seconds(60),
        ..Default::default()
    };

    let normalizer = TimeSeriesNormalizer::<f64>::new(config);

    let base_time = Utc::now();

    // Events arrive out of order
    let events = vec![
        (100.0, base_time + Duration::seconds(20)),  // 3rd chronologically
        (95.0, base_time + Duration::seconds(10)),   // 2nd chronologically
        (90.0, base_time),                            // 1st chronologically
        (105.0, base_time + Duration::seconds(30)),  // 4th chronologically
    ];

    // Process events as they arrive
    for (value, timestamp) in events {
        println!("Processing event at {}", timestamp);

        let normalized = normalizer
            .process_event(value, timestamp, "metric")
            .await?;

        if !normalized.is_empty() {
            println!("  Emitted {} normalized events", normalized.len());
        }
    }

    // Advance watermark to flush remaining events
    normalizer.update_watermark(base_time + Duration::seconds(100)).await;
    let flushed = normalizer.flush("metric").await?;

    println!("Flushed {} events", flushed.len());
    for event in flushed {
        println!("  {}: {}", event.timestamp, event.value);
    }

    Ok(())
}
```

---

## Advanced Configuration

### Example 4: Multi-Strategy Configuration

```rust
/// Configuration for different time-series types
pub struct MultiStrategyConfig {
    pub latency_config: NormalizationConfig,
    pub throughput_config: NormalizationConfig,
    pub error_rate_config: NormalizationConfig,
}

impl MultiStrategyConfig {
    pub fn new() -> Self {
        Self {
            // Latency: Use linear interpolation for smooth curves
            latency_config: NormalizationConfig {
                normalization_interval: Duration::seconds(5),
                alignment_strategy: AlignmentStrategy::RoundNearest,
                fill_strategy: FillStrategyType::LinearInterpolate,
                interpolation_method: InterpolationMethod::Linear,
                ..Default::default()
            },

            // Throughput: Use zero fill for missing data
            throughput_config: NormalizationConfig {
                normalization_interval: Duration::seconds(10),
                alignment_strategy: AlignmentStrategy::RoundDown,
                fill_strategy: FillStrategyType::Zero,
                interpolation_method: InterpolationMethod::None,
                ..Default::default()
            },

            // Error Rate: Use forward fill to carry error state
            error_rate_config: NormalizationConfig {
                normalization_interval: Duration::seconds(30),
                alignment_strategy: AlignmentStrategy::RoundDown,
                fill_strategy: FillStrategyType::ForwardFill,
                interpolation_method: InterpolationMethod::None,
                allow_late_events: false,
                ..Default::default()
            },
        }
    }
}

async fn use_multi_strategy() -> Result<(), Box<dyn std::error::Error>> {
    let configs = MultiStrategyConfig::new();

    let latency_normalizer = TimeSeriesNormalizer::<f64>::new(configs.latency_config);
    let throughput_normalizer = TimeSeriesNormalizer::<u64>::new(configs.throughput_config);
    let error_rate_normalizer = TimeSeriesNormalizer::<f64>::new(configs.error_rate_config);

    // Use different normalizers for different metric types
    let latency = 125.5;
    let throughput = 1000_u64;
    let error_rate = 0.05;

    let timestamp = Utc::now();

    latency_normalizer.process_event(latency, timestamp, "service_a").await?;
    throughput_normalizer.process_event(throughput, timestamp, "service_a").await?;
    error_rate_normalizer.process_event(error_rate, timestamp, "service_a").await?;

    Ok(())
}
```

### Example 5: Dynamic Configuration Update

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct DynamicNormalizer<T> {
    normalizer: Arc<RwLock<TimeSeriesNormalizer<T>>>,
    config: Arc<RwLock<NormalizationConfig>>,
}

impl<T: Clone + Send + Sync + 'static> DynamicNormalizer<T> {
    pub fn new(initial_config: NormalizationConfig) -> Self {
        let normalizer = TimeSeriesNormalizer::new(initial_config.clone());

        Self {
            normalizer: Arc::new(RwLock::new(normalizer)),
            config: Arc::new(RwLock::new(initial_config)),
        }
    }

    /// Update configuration at runtime
    pub async fn update_config(&self, new_config: NormalizationConfig) {
        let mut config = self.config.write().await;
        *config = new_config.clone();

        let mut normalizer = self.normalizer.write().await;
        *normalizer = TimeSeriesNormalizer::new(new_config);
    }

    pub async fn process_event(
        &self,
        event: T,
        timestamp: DateTime<Utc>,
        key: &str,
    ) -> Result<Vec<NormalizedEvent<T>>, NormalizationError> {
        let normalizer = self.normalizer.read().await;
        normalizer.process_event(event, timestamp, key).await
    }
}

async fn dynamic_config_example() -> Result<(), Box<dyn std::error::Error>> {
    let initial_config = NormalizationConfig::default();
    let normalizer = DynamicNormalizer::<f64>::new(initial_config);

    // Process some events
    normalizer.process_event(100.0, Utc::now(), "key1").await?;

    // Update configuration based on workload
    let new_config = NormalizationConfig {
        normalization_interval: Duration::seconds(10), // Changed from 5s
        buffer_capacity: 20000, // Increased capacity
        ..Default::default()
    };

    normalizer.update_config(new_config).await;

    // Continue processing with new config
    normalizer.process_event(105.0, Utc::now(), "key1").await?;

    Ok(())
}
```

---

## Integration Patterns

### Example 6: Integration with Stream Processor

```rust
use processor::{
    StreamProcessor, StreamProcessorBuilder, StreamProcessorConfig,
    ProcessorEvent, WindowAssigner, TumblingWindowAssigner,
};
use processor::aggregation::AvgAggregator;

/// Create a complete processing pipeline with normalization
pub async fn create_normalized_pipeline() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Configure normalization
    let norm_config = NormalizationConfig {
        normalization_interval: Duration::seconds(5),
        alignment_strategy: AlignmentStrategy::RoundDown,
        fill_strategy: FillStrategyType::LinearInterpolate,
        ..Default::default()
    };

    let normalizer = Arc::new(TimeSeriesNormalizer::<f64>::new(norm_config));

    // 2. Configure window processing
    let window_config = StreamProcessorConfig {
        window_size: Duration::minutes(1),
        window_type: WindowType::Tumbling,
        ..Default::default()
    };

    // 3. Build pipeline
    let processor = StreamProcessorBuilder::new()
        .with_config(window_config)
        .with_window_assigner(TumblingWindowAssigner::new(Duration::minutes(1)))
        .with_aggregator(AvgAggregator::new())
        .build()?;

    // 4. Process events through normalization then aggregation
    let events = vec![
        (100.5, Utc::now()),
        (102.3, Utc::now() + Duration::seconds(7)),
        (105.1, Utc::now() + Duration::seconds(15)),
    ];

    for (value, timestamp) in events {
        // Normalize first
        let normalized = normalizer
            .process_event(value, timestamp, "latency")
            .await?;

        // Then aggregate
        for norm_event in normalized {
            let proc_event = ProcessorEvent::new(
                norm_event.value,
                norm_event.timestamp,
                "latency".to_string(),
            );

            // Feed to processor
            // processor.process_event(proc_event).await?;
        }
    }

    Ok(())
}
```

### Example 7: Kafka Integration

```rust
use processor::kafka::{KafkaSource, KafkaSourceConfig, KafkaSink, KafkaSinkConfig};
use rdkafka::Message;

pub struct NormalizationKafkaPipeline {
    source: KafkaSource,
    sink: KafkaSink,
    normalizer: TimeSeriesNormalizer<f64>,
}

impl NormalizationKafkaPipeline {
    pub async fn new(
        source_config: KafkaSourceConfig,
        sink_config: KafkaSinkConfig,
        norm_config: NormalizationConfig,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let source = KafkaSource::new(source_config)?;
        let sink = KafkaSink::new(sink_config).await?;
        let normalizer = TimeSeriesNormalizer::new(norm_config);

        Ok(Self {
            source,
            sink,
            normalizer,
        })
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            // Consume from Kafka
            let message = self.source.recv().await?;

            // Deserialize event
            let (value, timestamp, key) = self.deserialize_message(message)?;

            // Normalize
            let normalized = self
                .normalizer
                .process_event(value, timestamp, &key)
                .await?;

            // Produce normalized events to output topic
            for norm_event in normalized {
                let serialized = self.serialize_event(&norm_event)?;
                self.sink.send(serialized).await?;
            }
        }
    }

    fn deserialize_message(
        &self,
        message: rdkafka::message::BorrowedMessage,
    ) -> Result<(f64, DateTime<Utc>, String), Box<dyn std::error::Error>> {
        // Parse message payload
        let payload = message.payload().ok_or("Empty payload")?;
        let data: serde_json::Value = serde_json::from_slice(payload)?;

        let value = data["value"].as_f64().ok_or("Missing value")?;
        let timestamp_str = data["timestamp"].as_str().ok_or("Missing timestamp")?;
        let timestamp = DateTime::parse_from_rfc3339(timestamp_str)?.with_timezone(&Utc);
        let key = data["key"].as_str().unwrap_or("default").to_string();

        Ok((value, timestamp, key))
    }

    fn serialize_event(
        &self,
        event: &NormalizedEvent<f64>,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let json = serde_json::json!({
            "timestamp": event.timestamp.to_rfc3339(),
            "value": event.value,
            "is_filled": event.is_filled,
            "original_timestamp": event.original_timestamp.map(|t| t.to_rfc3339()),
        });

        Ok(serde_json::to_vec(&json)?)
    }
}
```

### Example 8: State Backend Integration

```rust
use processor::state::{StateBackend, RedisBackend, PostgresBackend};

/// Normalizer with persistent state
pub struct StatefulNormalizer<T, S: StateBackend> {
    normalizer: TimeSeriesNormalizer<T>,
    state_backend: Arc<S>,
    checkpoint_interval: Duration,
}

impl<T, S> StatefulNormalizer<T, S>
where
    T: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static,
    S: StateBackend + 'static,
{
    pub async fn new(
        config: NormalizationConfig,
        state_backend: Arc<S>,
        checkpoint_interval: Duration,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let normalizer = TimeSeriesNormalizer::new(config);

        let stateful = Self {
            normalizer,
            state_backend,
            checkpoint_interval,
        };

        // Start checkpoint task
        stateful.start_checkpoint_task();

        Ok(stateful)
    }

    fn start_checkpoint_task(&self) {
        let backend = Arc::clone(&self.state_backend);
        let interval = self.checkpoint_interval;

        tokio::spawn(async move {
            let mut timer = tokio::time::interval(
                interval.to_std().unwrap_or(std::time::Duration::from_secs(60))
            );

            loop {
                timer.tick().await;
                if let Err(e) = Self::checkpoint_state(&backend).await {
                    tracing::error!(error = ?e, "Checkpoint failed");
                }
            }
        });
    }

    async fn checkpoint_state(backend: &Arc<S>) -> Result<(), Box<dyn std::error::Error>> {
        // Serialize normalizer state
        let state_key = b"normalizer:checkpoint";

        // Get buffer state, watermark state, etc.
        // let state_data = serialize_state(&self.normalizer)?;

        // Persist to backend
        // backend.put(state_key, &state_data).await?;

        Ok(())
    }

    pub async fn restore_from_checkpoint(
        state_backend: Arc<S>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let state_key = b"normalizer:checkpoint";

        // Load state from backend
        if let Some(state_data) = state_backend.get(state_key).await? {
            // Deserialize and restore
            // let restored_state = deserialize_state(&state_data)?;
            // Create normalizer from restored state
        }

        // If no checkpoint exists, create new
        let config = NormalizationConfig::default();
        Self::new(config, state_backend, Duration::minutes(1)).await
    }
}
```

---

## Real-World Scenarios

### Scenario 1: LLM Metrics Processing

```rust
/// Normalize LLM latency metrics for consistent aggregation
pub async fn process_llm_metrics() -> Result<(), Box<dyn std::error::Error>> {
    // Configure for LLM metrics:
    // - 5-second intervals for responsive monitoring
    // - Linear interpolation for smooth latency curves
    // - Allow late events (distributed systems)
    let config = NormalizationConfig {
        normalization_interval: Duration::seconds(5),
        alignment_strategy: AlignmentStrategy::RoundDown,
        fill_strategy: FillStrategyType::LinearInterpolate,
        buffer_capacity: 50000,  // High throughput
        buffer_retention: Duration::minutes(10),
        allow_late_events: true,
        max_lateness: Duration::seconds(120),  // 2-minute tolerance
        ..Default::default()
    };

    let normalizer = TimeSeriesNormalizer::<f64>::new(config);

    // Simulate incoming LLM request metrics
    let metrics = vec![
        MetricEvent {
            model: "claude-3-opus".to_string(),
            latency_ms: 1234.5,
            timestamp: Utc::now(),
            request_id: "req-1".to_string(),
        },
        MetricEvent {
            model: "claude-3-opus".to_string(),
            latency_ms: 987.3,
            timestamp: Utc::now() + Duration::seconds(7),
            request_id: "req-2".to_string(),
        },
        // Gap here - no metrics for 20 seconds
        MetricEvent {
            model: "claude-3-opus".to_string(),
            latency_ms: 1567.8,
            timestamp: Utc::now() + Duration::seconds(30),
            request_id: "req-3".to_string(),
        },
    ];

    for metric in metrics {
        let key = format!("model:{}", metric.model);

        let normalized = normalizer
            .process_event(metric.latency_ms, metric.timestamp, &key)
            .await?;

        for norm_event in normalized {
            println!(
                "Model: {}, Time: {}, Latency: {:.2}ms, Filled: {}",
                metric.model,
                norm_event.timestamp,
                norm_event.value,
                norm_event.is_filled
            );
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct MetricEvent {
    model: String,
    latency_ms: f64,
    timestamp: DateTime<Utc>,
    request_id: String,
}
```

### Scenario 2: Cost Tracking

```rust
/// Normalize cost data for budget monitoring
pub async fn process_cost_metrics() -> Result<(), Box<dyn std::error::Error>> {
    // Configure for cost tracking:
    // - 1-minute intervals for billing alignment
    // - Zero fill (no usage = zero cost)
    // - Strict ordering (financial data)
    let config = NormalizationConfig {
        normalization_interval: Duration::minutes(1),
        alignment_strategy: AlignmentStrategy::RoundDown,
        fill_strategy: FillStrategyType::Zero,
        allow_late_events: false,  // Strict for financial data
        ..Default::default()
    };

    let normalizer = TimeSeriesNormalizer::<f64>::new(config);

    // Process cost events
    let cost_events = vec![
        (2.50, Utc::now()),  // $2.50 at T+0
        (3.25, Utc::now() + Duration::minutes(2)),  // $3.25 at T+2min
        (1.80, Utc::now() + Duration::minutes(5)),  // $1.80 at T+5min
    ];

    let mut total_normalized_cost = 0.0;

    for (cost, timestamp) in cost_events {
        let normalized = normalizer
            .process_event(cost, timestamp, "usage_cost")
            .await?;

        for norm_event in normalized {
            total_normalized_cost += norm_event.value;

            println!(
                "Minute: {}, Cost: ${:.2}, Filled: {}",
                norm_event.timestamp.minute(),
                norm_event.value,
                norm_event.is_filled
            );
        }
    }

    println!("Total normalized cost: ${:.2}", total_normalized_cost);

    Ok(())
}
```

### Scenario 3: Multi-Model Performance Comparison

```rust
/// Normalize metrics across multiple models for fair comparison
pub async fn compare_models() -> Result<(), Box<dyn std::error::Error>> {
    let config = NormalizationConfig {
        normalization_interval: Duration::seconds(10),
        alignment_strategy: AlignmentStrategy::RoundNearest,
        fill_strategy: FillStrategyType::ForwardFill,
        ..Default::default()
    };

    let normalizer = Arc::new(TimeSeriesNormalizer::<f64>::new(config));

    // Models to compare
    let models = vec!["claude-3-opus", "claude-3-sonnet", "claude-3-haiku"];

    // Simulate concurrent metric collection
    let mut handles = vec![];

    for model in models {
        let norm = Arc::clone(&normalizer);
        let model = model.to_string();

        let handle = tokio::spawn(async move {
            let latencies = vec![
                (120.5, Utc::now()),
                (118.3, Utc::now() + Duration::seconds(12)),
                (125.7, Utc::now() + Duration::seconds(25)),
            ];

            let key = format!("model:{}", model);

            for (latency, timestamp) in latencies {
                if let Ok(normalized) = norm.process_event(latency, timestamp, &key).await {
                    for event in normalized {
                        println!(
                            "Model: {}, Time: {}, Latency: {:.2}ms",
                            model, event.timestamp, event.value
                        );
                    }
                }
            }
        });

        handles.push(handle);
    }

    // Wait for all model metrics to be processed
    for handle in handles {
        handle.await?;
    }

    Ok(())
}
```

---

## Testing Examples

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[tokio::test]
    async fn test_basic_normalization() {
        let config = NormalizationConfig {
            normalization_interval: Duration::seconds(5),
            alignment_strategy: AlignmentStrategy::RoundDown,
            fill_strategy: FillStrategyType::ForwardFill,
            ..Default::default()
        };

        let normalizer = TimeSeriesNormalizer::<f64>::new(config);

        let timestamp = Utc.timestamp_opt(1000, 0).unwrap();
        let result = normalizer
            .process_event(100.0, timestamp, "test")
            .await
            .unwrap();

        assert!(!result.is_empty());
        assert_eq!(result[0].value, 100.0);
    }

    #[tokio::test]
    async fn test_gap_filling() {
        let config = NormalizationConfig {
            normalization_interval: Duration::seconds(5),
            alignment_strategy: AlignmentStrategy::RoundDown,
            fill_strategy: FillStrategyType::LinearInterpolate,
            ..Default::default()
        };

        let normalizer = TimeSeriesNormalizer::<f64>::new(config);

        let base = Utc.timestamp_opt(1000, 0).unwrap();

        // Create a gap
        normalizer.process_event(100.0, base, "test").await.unwrap();

        // Advance watermark
        normalizer.update_watermark(base + Duration::seconds(30)).await;

        // Event after gap
        let result = normalizer
            .process_event(120.0, base + Duration::seconds(20), "test")
            .await
            .unwrap();

        // Should have filled events at 5s, 10s, 15s
        assert!(result.len() > 2);

        // Check interpolated values
        for event in result {
            if event.is_filled {
                assert!(event.value > 100.0 && event.value < 120.0);
            }
        }
    }

    #[tokio::test]
    async fn test_late_event_handling() {
        let config = NormalizationConfig {
            normalization_interval: Duration::seconds(5),
            allow_late_events: true,
            max_lateness: Duration::seconds(60),
            ..Default::default()
        };

        let normalizer = TimeSeriesNormalizer::<f64>::new(config);

        let base = Utc.timestamp_opt(1000, 0).unwrap();

        // Advance watermark
        normalizer.update_watermark(base + Duration::seconds(100)).await;

        // Late event within tolerance
        let result = normalizer
            .process_event(100.0, base + Duration::seconds(50), "test")
            .await;

        assert!(result.is_ok());

        // Late event beyond tolerance
        let result = normalizer
            .process_event(100.0, base - Duration::seconds(10), "test")
            .await;

        assert!(result.is_err());
    }
}
```

### Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_end_to_end_pipeline() {
        // Setup normalizer
        let norm_config = NormalizationConfig::default();
        let normalizer = TimeSeriesNormalizer::<f64>::new(norm_config);

        // Setup processor
        let proc_config = StreamProcessorConfig::default();
        let processor = StreamProcessor::new(proc_config);

        // Generate test events
        let events = generate_test_events(1000);

        let mut normalized_count = 0;
        let mut aggregated_count = 0;

        for (value, timestamp) in events {
            // Normalize
            let normalized = normalizer
                .process_event(value, timestamp, "test")
                .await
                .unwrap();

            normalized_count += normalized.len();

            // Aggregate
            for norm_event in normalized {
                let proc_event = ProcessorEvent::new(
                    norm_event.value,
                    norm_event.timestamp,
                    "test".to_string(),
                );

                // Process through aggregation
                // let results = processor.process_event(proc_event).await.unwrap();
                // aggregated_count += results.len();
            }
        }

        println!("Normalized: {}, Aggregated: {}", normalized_count, aggregated_count);
        assert!(normalized_count > 0);
    }

    fn generate_test_events(count: usize) -> Vec<(f64, DateTime<Utc>)> {
        let mut events = Vec::new();
        let base = Utc::now();

        for i in 0..count {
            let value = 100.0 + (i as f64 * 0.5);
            // Random gaps
            let offset = if i % 10 == 0 {
                Duration::seconds(i as i64 * 7) // Create gaps
            } else {
                Duration::seconds(i as i64 * 3)
            };

            events.push((value, base + offset));
        }

        events
    }
}
```

---

## Performance Tuning

### Benchmark Example

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

fn benchmark_alignment(c: &mut Criterion) {
    let mut group = c.benchmark_group("alignment");

    for interval in [1000, 5000, 10000, 60000].iter() {
        let aligner = RoundDownAligner::new(Duration::milliseconds(*interval));
        let timestamp = Utc::now();

        group.bench_with_input(
            BenchmarkId::from_parameter(interval),
            interval,
            |b, _| {
                b.iter(|| {
                    black_box(aligner.align(black_box(timestamp)))
                });
            },
        );
    }

    group.finish();
}

fn benchmark_gap_filling(c: &mut Criterion) {
    let mut group = c.benchmark_group("gap_filling");

    for gap_size in [10, 50, 100, 500].iter() {
        let strategy = ForwardFillStrategy;
        let before = create_test_event(Utc::now(), 100.0);
        let after = create_test_event(
            Utc::now() + Duration::seconds(*gap_size),
            200.0
        );

        group.bench_with_input(
            BenchmarkId::from_parameter(gap_size),
            gap_size,
            |b, _| {
                b.iter(|| {
                    black_box(strategy.fill_gap(
                        black_box(&before),
                        black_box(&after),
                        black_box(Duration::seconds(1)),
                    ))
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, benchmark_alignment, benchmark_gap_filling);
criterion_main!(benches);
```

### Profiling Example

```rust
#[cfg(feature = "profiling")]
pub async fn profile_normalization() {
    use pprof::ProfilerGuard;

    let guard = ProfilerGuard::new(100).unwrap();

    // Run normalization workload
    let config = NormalizationConfig::default();
    let normalizer = TimeSeriesNormalizer::<f64>::new(config);

    for i in 0..10000 {
        let value = 100.0 + (i as f64);
        let timestamp = Utc::now() + Duration::seconds(i);

        normalizer
            .process_event(value, timestamp, "test")
            .await
            .unwrap();
    }

    // Generate flamegraph
    if let Ok(report) = guard.report().build() {
        let file = std::fs::File::create("flamegraph.svg").unwrap();
        report.flamegraph(file).unwrap();
    }
}
```

---

## Troubleshooting

### Common Issues and Solutions

#### Issue 1: High Memory Usage

```rust
// Problem: Buffer growing too large
// Solution: Adjust buffer configuration

let config = NormalizationConfig {
    buffer_capacity: 5000,  // Reduce from default 10000
    buffer_retention: Duration::minutes(2),  // Reduce from 5 minutes
    ..Default::default()
};

// Enable aggressive cleanup
let normalizer = TimeSeriesNormalizer::<f64>::new(config);

// Monitor buffer size
tokio::spawn(async move {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));

    loop {
        interval.tick().await;

        let stats = normalizer.event_buffer.read().await.stats();
        println!("Buffer stats: {:?}", stats);

        if stats.total_events > 4000 {
            println!("WARNING: Buffer approaching capacity");
        }
    }
});
```

#### Issue 2: Late Events Being Dropped

```rust
// Problem: Events arriving late due to network delays
// Solution: Increase lateness tolerance

let config = NormalizationConfig {
    allow_late_events: true,
    max_lateness: Duration::minutes(5),  // Increase tolerance
    ..Default::default()
};

// Add monitoring
let metrics = normalizer.metrics();

tokio::spawn(async move {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));

    loop {
        interval.tick().await;

        let snapshot = metrics.snapshot();
        let late_event_rate = snapshot.late_events as f64 /
                             snapshot.events_processed as f64;

        if late_event_rate > 0.05 {  // More than 5% late
            println!("WARNING: High late event rate: {:.2}%", late_event_rate * 100.0);
        }
    }
});
```

#### Issue 3: Performance Degradation

```rust
// Problem: Slow processing with many gaps
// Solution: Use skip strategy or increase interval

let config = NormalizationConfig {
    fill_strategy: FillStrategyType::Skip,  // Don't fill gaps
    // OR
    normalization_interval: Duration::seconds(30),  // Larger intervals
    ..Default::default()
};

// Enable batch processing
impl<T> TimeSeriesNormalizer<T> {
    pub async fn process_batch_optimized(
        &self,
        events: Vec<(T, DateTime<Utc>, String)>,
    ) -> Result<Vec<NormalizedEvent<T>>, NormalizationError> {
        // Sort events by key for better cache locality
        let mut sorted = events;
        sorted.sort_by(|a, b| a.2.cmp(&b.2));

        // Process in batches by key
        let mut results = Vec::new();

        for chunk in sorted.chunks(100) {
            let chunk_results = self.process_chunk(chunk).await?;
            results.extend(chunk_results);
        }

        Ok(results)
    }
}
```

---

## Conclusion

These examples demonstrate practical implementation patterns for the Time-Series Normalization system:

- **Basic Usage**: Simple cases for common scenarios
- **Advanced Configuration**: Multi-strategy and dynamic configs
- **Integration Patterns**: Kafka, state backends, stream processors
- **Real-World Scenarios**: LLM metrics, cost tracking, model comparison
- **Testing**: Comprehensive unit and integration tests
- **Performance**: Benchmarking and optimization techniques
- **Troubleshooting**: Common issues and solutions

Use these examples as templates for implementing normalization in your specific use case.
