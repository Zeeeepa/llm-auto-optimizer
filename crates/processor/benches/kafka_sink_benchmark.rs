//! Benchmarks for KafkaSink
//!
//! Run with: cargo bench --bench kafka_sink_benchmark

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use processor::kafka::{BincodeSerializer, JsonSerializer, KafkaSink, KafkaSinkConfig, SinkMessage};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::runtime::Runtime;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BenchmarkEvent {
    id: String,
    value: f64,
    count: u64,
    timestamp: i64,
    metadata: Vec<String>,
}

impl BenchmarkEvent {
    fn new(size: usize) -> Self {
        Self {
            id: format!("event-{}", size),
            value: 123.45,
            count: 1000,
            timestamp: chrono::Utc::now().timestamp_millis(),
            metadata: vec!["meta".to_string(); size],
        }
    }
}

fn get_bench_config() -> KafkaSinkConfig {
    KafkaSinkConfig {
        brokers: "localhost:9092".to_string(),
        topic: "benchmark-topic".to_string(),
        enable_idempotence: true,
        batch_size: 1000,
        linger_ms: 10,
        compression_type: "snappy".to_string(),
        ..Default::default()
    }
}

fn benchmark_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization");

    for size in [1, 10, 100].iter() {
        let event = BenchmarkEvent::new(*size);

        group.throughput(Throughput::Elements(1));

        group.bench_with_input(
            BenchmarkId::new("json", size),
            size,
            |b, _| {
                let rt = Runtime::new().unwrap();
                let serializer = JsonSerializer;
                b.to_async(&rt).iter(|| async {
                    let _ = black_box(serializer.serialize(&event).await);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("bincode", size),
            size,
            |b, _| {
                let rt = Runtime::new().unwrap();
                let serializer = BincodeSerializer;
                b.to_async(&rt).iter(|| async {
                    let _ = black_box(serializer.serialize(&event).await);
                });
            },
        );
    }

    group.finish();
}

fn benchmark_message_builder(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_builder");

    let event = BenchmarkEvent::new(10);

    group.bench_function("simple", |b| {
        b.iter(|| {
            black_box(SinkMessage::new(event.clone()));
        });
    });

    group.bench_function("with_key", |b| {
        b.iter(|| {
            black_box(
                SinkMessage::new(event.clone())
                    .with_key("test-key".to_string())
            );
        });
    });

    group.bench_function("with_headers", |b| {
        b.iter(|| {
            black_box(
                SinkMessage::new(event.clone())
                    .with_key("test-key".to_string())
                    .with_header("h1".to_string(), "v1".to_string())
                    .with_header("h2".to_string(), "v2".to_string())
                    .with_header("h3".to_string(), "v3".to_string())
            );
        });
    });

    group.finish();
}

fn benchmark_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("metrics");

    let rt = Runtime::new().unwrap();
    let config = get_bench_config();

    // Note: This requires a running Kafka instance
    // Comment out if Kafka is not available
    /*
    let sink = rt.block_on(async {
        KafkaSink::<BenchmarkEvent>::new(config).await.unwrap()
    });

    group.bench_function("get_metrics", |b| {
        b.to_async(&rt).iter(|| async {
            black_box(sink.metrics().await);
        });
    });
    */

    group.finish();
}

criterion_group!(
    benches,
    benchmark_serialization,
    benchmark_message_builder,
    benchmark_metrics,
);

criterion_main!(benches);
