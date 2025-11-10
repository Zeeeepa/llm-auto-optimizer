//! Performance benchmarks for Stream Processor
//!
//! Validates the <100ms latency requirement and 10,000 events/sec throughput

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use processor::aggregation::*;
use processor::core::*;
use processor::watermark::*;
use processor::window::*;
use chrono::{Duration, TimeZone, Utc};
use std::time::Duration as StdDuration;
use tokio::runtime::Runtime;

/// Benchmark single event processing latency (target: <100ms for batch)
fn bench_event_processing_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("event_processing_latency");
    group.throughput(Throughput::Elements(1));

    let assigner = TumblingWindowAssigner::new(Duration::milliseconds(1000));
    let mut aggregator = AverageAggregator::new();

    group.bench_function("window_assign", |b| {
        b.iter(|| {
            let timestamp = Utc.timestamp_millis_opt(500).unwrap();
            black_box(assigner.assign_windows(timestamp));
        });
    });

    group.bench_function("aggregation_update", |b| {
        b.iter(|| {
            let mut agg = aggregator.clone();
            black_box(agg.update(42.0).unwrap());
        });
    });

    group.bench_function("end_to_end", |b| {
        b.iter(|| {
            let timestamp = Utc.timestamp_millis_opt(500).unwrap();
            let _windows = black_box(assigner.assign_windows(timestamp));
            let mut agg = AverageAggregator::new();
            black_box(agg.update(42.0).unwrap());
        });
    });

    group.finish();
}

/// Benchmark throughput capacity (target: 10,000 events/sec)
fn bench_event_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("event_throughput");

    for batch_size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));

        group.bench_with_input(
            BenchmarkId::new("aggregation", batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    let mut agg = AverageAggregator::new();
                    for i in 0..size {
                        black_box(agg.update(i as f64).unwrap());
                    }
                    black_box(agg.finalize().unwrap());
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("composite_aggregation", batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    let mut composite = CompositeAggregator::new();
                    composite
                        .add_count("count")
                        .add_avg("avg")
                        .add_min("min")
                        .add_max("max")
                        .add_p95("p95");

                    for i in 0..size {
                        black_box(composite.update(i as f64).unwrap());
                    }
                    black_box(composite.finalize().unwrap());
                });
            },
        );
    }

    group.finish();
}

/// Benchmark window assignment performance
fn bench_window_assignment(c: &mut Criterion) {
    let mut group = c.benchmark_group("window_assignment");
    group.throughput(Throughput::Elements(1));

    // Tumbling windows
    let tumbling = TumblingWindowAssigner::new(Duration::milliseconds(1000));
    group.bench_function("tumbling", |b| {
        b.iter(|| {
            let ts = Utc.timestamp_millis_opt(1500).unwrap();
            black_box(tumbling.assign_windows(ts));
        });
    });

    // Sliding windows
    let sliding = SlidingWindowAssigner::new(
        Duration::milliseconds(2000),
        Duration::milliseconds(500),
    );
    group.bench_function("sliding", |b| {
        b.iter(|| {
            let ts = Utc.timestamp_millis_opt(1500).unwrap();
            black_box(sliding.assign_windows(ts));
        });
    });

    // Session windows
    let session = SessionWindowAssigner::new(Duration::milliseconds(1000));
    group.bench_function("session", |b| {
        b.iter(|| {
            let ts = Utc.timestamp_millis_opt(1500).unwrap();
            black_box(session.assign_windows(ts));
        });
    });

    group.finish();
}

/// Benchmark watermark generation performance
fn bench_watermark_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("watermark_generation");
    group.throughput(Throughput::Elements(1));

    let mut generator = BoundedOutOfOrdernessWatermark::new(
        StdDuration::from_secs(5),
        None,
    );

    group.bench_function("single_partition", |b| {
        b.iter(|| {
            black_box(generator.on_event(10000, 0));
        });
    });

    group.bench_function("multi_partition", |b| {
        b.iter(|| {
            black_box(generator.on_event(10000, 0));
            black_box(generator.on_event(11000, 1));
            black_box(generator.on_event(12000, 2));
            black_box(generator.on_event(13000, 3));
        });
    });

    group.finish();
}

/// Benchmark aggregation accuracy and performance
fn bench_aggregation_functions(c: &mut Criterion) {
    let mut group = c.benchmark_group("aggregation_functions");

    let data: Vec<f64> = (0..1000).map(|i| i as f64).collect();

    group.bench_function("count", |b| {
        b.iter(|| {
            let mut agg = CountAggregator::new();
            agg.update_batch(black_box(&data)).unwrap();
            black_box(agg.finalize().unwrap());
        });
    });

    group.bench_function("sum", |b| {
        b.iter(|| {
            let mut agg = SumAggregator::new();
            agg.update_batch(black_box(&data)).unwrap();
            black_box(agg.finalize().unwrap());
        });
    });

    group.bench_function("average", |b| {
        b.iter(|| {
            let mut agg = AverageAggregator::new();
            agg.update_batch(black_box(&data)).unwrap();
            black_box(agg.finalize().unwrap());
        });
    });

    group.bench_function("min", |b| {
        b.iter(|| {
            let mut agg = MinAggregator::new();
            agg.update_batch(black_box(&data)).unwrap();
            black_box(agg.finalize().unwrap());
        });
    });

    group.bench_function("max", |b| {
        b.iter(|| {
            let mut agg = MaxAggregator::new();
            agg.update_batch(black_box(&data)).unwrap();
            black_box(agg.finalize().unwrap());
        });
    });

    group.bench_function("p50", |b| {
        b.iter(|| {
            let mut agg = PercentileAggregator::p50();
            agg.update_batch(black_box(&data)).unwrap();
            black_box(agg.finalize().unwrap());
        });
    });

    group.bench_function("p95", |b| {
        b.iter(|| {
            let mut agg = PercentileAggregator::p95();
            agg.update_batch(black_box(&data)).unwrap();
            black_box(agg.finalize().unwrap());
        });
    });

    group.bench_function("p99", |b| {
        b.iter(|| {
            let mut agg = PercentileAggregator::p99();
            agg.update_batch(black_box(&data)).unwrap();
            black_box(agg.finalize().unwrap());
        });
    });

    group.bench_function("stddev", |b| {
        b.iter(|| {
            let mut agg = StandardDeviationAggregator::new();
            agg.update_batch(black_box(&data)).unwrap();
            black_box(agg.finalize().unwrap());
        });
    });

    group.finish();
}

/// Benchmark distributed aggregation (merge operations)
fn bench_distributed_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("distributed_aggregation");

    for worker_count in [2, 4, 8, 16].iter() {
        group.throughput(Throughput::Elements(*worker_count as u64 * 1000));

        group.bench_with_input(
            BenchmarkId::new("merge", worker_count),
            worker_count,
            |b, &workers| {
                b.iter(|| {
                    // Create worker aggregators
                    let mut worker_aggs: Vec<AverageAggregator<f64>> = (0..workers)
                        .map(|_| {
                            let mut agg = AverageAggregator::new();
                            for i in 0..1000 {
                                agg.update(i as f64).unwrap();
                            }
                            agg
                        })
                        .collect();

                    // Merge all results
                    let mut final_agg = AverageAggregator::new();
                    for agg in worker_aggs {
                        final_agg.merge(agg.accumulator()).unwrap();
                    }

                    black_box(final_agg.finalize().unwrap());
                });
            },
        );
    }

    group.finish();
}

/// Benchmark window trigger performance
fn bench_window_triggers(c: &mut Criterion) {
    let mut group = c.benchmark_group("window_triggers");
    group.throughput(Throughput::Elements(1));

    let window = Window::new(WindowBounds::new(
        Utc.timestamp_millis_opt(0).unwrap(),
        Utc.timestamp_millis_opt(1000).unwrap(),
    ));

    let ctx = TriggerContext {
        window: window.clone(),
        event_time: Some(Utc.timestamp_millis_opt(500).unwrap()),
        processing_time: Utc.timestamp_millis_opt(1500).unwrap(),
        watermark: Some(Utc.timestamp_millis_opt(1000).unwrap()),
        element_count: 100,
    };

    // Watermark trigger
    let mut watermark_trigger = OnWatermarkTrigger::new();
    group.bench_function("watermark_trigger", |b| {
        b.iter(|| {
            black_box(watermark_trigger.on_watermark(&ctx));
        });
    });

    // Count trigger
    let mut count_trigger = CountTrigger::new(100);
    group.bench_function("count_trigger", |b| {
        b.iter(|| {
            black_box(count_trigger.on_element(&ctx));
        });
    });

    // Processing time trigger
    let mut processing_trigger = ProcessingTimeTrigger::with_delay(Duration::seconds(5));
    group.bench_function("processing_time_trigger", |b| {
        b.iter(|| {
            black_box(processing_trigger.on_element(&ctx));
        });
    });

    group.finish();
}

/// Benchmark session window merging
fn bench_session_window_merging(c: &mut Criterion) {
    let mut group = c.benchmark_group("session_window_merging");

    for window_count in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*window_count as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(window_count),
            window_count,
            |b, &count| {
                b.iter(|| {
                    let mut merger = WindowMerger::new(Duration::milliseconds(1000));

                    for i in 0..count {
                        let window = Window::new(WindowBounds::new(
                            Utc.timestamp_millis_opt(i * 100).unwrap(),
                            Utc.timestamp_millis_opt((i + 1) * 100 + 1000).unwrap(),
                        ));
                        black_box(merger.add_and_merge(window));
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark late event handling
fn bench_late_event_handling(c: &mut Criterion) {
    let mut group = c.benchmark_group("late_event_handling");
    group.throughput(Throughput::Elements(1));

    let mut generator = BoundedOutOfOrdernessWatermark::new(
        StdDuration::from_secs(5),
        None,
    );

    // Advance watermark
    generator.on_event(20000, 0);

    let handler = LateEventHandler::new(LateEventConfig {
        allowed_lateness: StdDuration::from_secs(10),
        log_late_events: false,
        emit_metrics: false,
    });

    group.bench_function("late_detection", |b| {
        b.iter(|| {
            black_box(generator.is_late_event(10000));
        });
    });

    group.bench_function("late_handling", |b| {
        b.iter(|| {
            let lateness = generator.get_lateness(10000);
            black_box(handler.handle_late_event(lateness, 0));
        });
    });

    group.finish();
}

/// Benchmark key extraction performance
fn bench_key_extraction(c: &mut Criterion) {
    use llm_optimizer_types::events::{EventSource, EventType, FeedbackEvent};
    use std::collections::HashMap;

    let mut group = c.benchmark_group("key_extraction");
    group.throughput(Throughput::Elements(1));

    let mut event = FeedbackEvent::new(
        EventSource::Observatory,
        EventType::Metric,
        serde_json::json!({}),
    );
    event.metadata.insert("model_id".to_string(), "claude-3-opus".to_string());
    event.metadata.insert("session_id".to_string(), "sess-123".to_string());

    let extractor = FeedbackEventKeyExtractor;

    group.bench_function("feedback_event", |b| {
        b.iter(|| {
            black_box(extractor.extract_key(&event));
        });
    });

    let metric_extractor = MetricPointKeyExtractor;
    let mut tags = HashMap::new();
    tags.insert("model".to_string(), "claude-3-opus".to_string());

    let point = llm_optimizer_types::metrics::MetricPoint {
        name: "latency".to_string(),
        value: 123.45,
        unit: "ms".to_string(),
        timestamp: Utc::now(),
        tags,
        aggregation: None,
    };

    group.bench_function("metric_point", |b| {
        b.iter(|| {
            black_box(metric_extractor.extract_key(&point));
        });
    });

    group.finish();
}

/// Benchmark complete windowed aggregation flow
fn bench_windowed_aggregation_flow(c: &mut Criterion) {
    let mut group = c.benchmark_group("windowed_aggregation_flow");

    for event_count in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*event_count as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(event_count),
            event_count,
            |b, &count| {
                b.iter(|| {
                    let assigner = TumblingWindowAssigner::new(Duration::milliseconds(1000));
                    let mut trigger = OnWatermarkTrigger::new();
                    let mut aggregator = AverageAggregator::new();

                    // Assign window
                    let window = assigner.assign_windows(Utc.timestamp_millis_opt(500).unwrap())[0].clone();

                    // Process events
                    for i in 0..count {
                        aggregator.update(i as f64).unwrap();
                    }

                    // Trigger window
                    let ctx = TriggerContext {
                        window: window.clone(),
                        event_time: Some(Utc.timestamp_millis_opt(1000).unwrap()),
                        processing_time: Utc.timestamp_millis_opt(1100).unwrap(),
                        watermark: Some(Utc.timestamp_millis_opt(1000).unwrap()),
                        element_count: count,
                    };

                    if trigger.on_watermark(&ctx) == TriggerResult::Fire {
                        black_box(aggregator.finalize().unwrap());
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark memory efficiency with many concurrent windows
fn bench_concurrent_windows(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_windows");

    for window_count in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*window_count as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(window_count),
            window_count,
            |b, &count| {
                b.iter(|| {
                    let mut windows = Vec::with_capacity(count);
                    let mut aggregators: Vec<AverageAggregator<f64>> = Vec::with_capacity(count);

                    for i in 0..count {
                        let window = Window::new(WindowBounds::new(
                            Utc.timestamp_millis_opt(i as i64 * 1000).unwrap(),
                            Utc.timestamp_millis_opt((i as i64 + 1) * 1000).unwrap(),
                        ));
                        windows.push(window);

                        let mut agg = AverageAggregator::new();
                        for j in 0..10 {
                            agg.update(j as f64).unwrap();
                        }
                        aggregators.push(agg);
                    }

                    black_box(windows.len());
                    black_box(aggregators.len());
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_event_processing_latency,
    bench_event_throughput,
    bench_window_assignment,
    bench_watermark_generation,
    bench_aggregation_functions,
    bench_distributed_aggregation,
    bench_window_triggers,
    bench_session_window_merging,
    bench_late_event_handling,
    bench_key_extraction,
    bench_windowed_aggregation_flow,
    bench_concurrent_windows,
);

criterion_main!(benches);
