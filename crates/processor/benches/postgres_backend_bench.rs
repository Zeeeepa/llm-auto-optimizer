//! Benchmarks for PostgreSQL state backend
//!
//! Run with:
//! ```bash
//! cargo bench --bench postgres_backend_bench
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use processor::state::{PostgresConfig, PostgresStateBackend, StateBackend};
use std::sync::Arc;
use tokio::runtime::Runtime;

// Helper to create backend
async fn create_backend() -> Arc<PostgresStateBackend> {
    let config =
        PostgresConfig::new("postgresql://postgres:postgres@localhost/optimizer_bench")
            .with_pool_size(10, 50);

    let backend = PostgresStateBackend::new(config)
        .await
        .expect("Failed to create backend");

    backend.clear().await.expect("Failed to clear backend");

    Arc::new(backend)
}

fn bench_put(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let backend = rt.block_on(create_backend());

    let mut group = c.benchmark_group("postgres_put");

    // Benchmark different value sizes
    for size in [64, 256, 1024, 4096, 16384].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let value = vec![0u8; size];
            let mut counter = 0u64;

            b.to_async(&rt).iter(|| {
                let backend = Arc::clone(&backend);
                let value = value.clone();
                let key = format!("bench:put:{}", counter).into_bytes();
                counter += 1;

                async move {
                    backend
                        .put(black_box(&key), black_box(&value))
                        .await
                        .unwrap();
                }
            });
        });
    }

    group.finish();
}

fn bench_get(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let backend = rt.block_on(create_backend());

    // Prepare data
    rt.block_on(async {
        for i in 0..1000 {
            let key = format!("bench:get:{}", i).into_bytes();
            let value = vec![0u8; 1024];
            backend.put(&key, &value).await.unwrap();
        }
    });

    let mut group = c.benchmark_group("postgres_get");

    group.bench_function("get_existing", |b| {
        let mut counter = 0usize;
        b.to_async(&rt).iter(|| {
            let backend = Arc::clone(&backend);
            let key = format!("bench:get:{}", counter % 1000).into_bytes();
            counter += 1;

            async move {
                let result = backend.get(black_box(&key)).await.unwrap();
                black_box(result);
            }
        });
    });

    group.bench_function("get_missing", |b| {
        b.to_async(&rt).iter(|| {
            let backend = Arc::clone(&backend);
            let key = b"bench:missing:key";

            async move {
                let result = backend.get(black_box(key)).await.unwrap();
                black_box(result);
            }
        });
    });

    group.finish();
}

fn bench_batch_put(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let backend = rt.block_on(create_backend());

    let mut group = c.benchmark_group("postgres_batch_put");

    for batch_size in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                b.to_async(&rt).iter(|| {
                    let backend = Arc::clone(&backend);

                    async move {
                        let entries: Vec<(Vec<u8>, Vec<u8>)> = (0..batch_size)
                            .map(|i| {
                                let key = format!("bench:batch:{}", i).into_bytes();
                                let value = vec![0u8; 256];
                                (key, value)
                            })
                            .collect();

                        let entries_ref: Vec<(&[u8], &[u8])> = entries
                            .iter()
                            .map(|(k, v)| (k.as_slice(), v.as_slice()))
                            .collect();

                        backend
                            .batch_put(black_box(&entries_ref))
                            .await
                            .unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_list_keys(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let backend = rt.block_on(create_backend());

    // Prepare data with different prefixes
    rt.block_on(async {
        for i in 0..1000 {
            let key = format!("prefix1:{}", i).into_bytes();
            backend.put(&key, b"value").await.unwrap();
        }
        for i in 0..500 {
            let key = format!("prefix2:{}", i).into_bytes();
            backend.put(&key, b"value").await.unwrap();
        }
    });

    let mut group = c.benchmark_group("postgres_list_keys");

    group.bench_function("list_all", |b| {
        b.to_async(&rt).iter(|| {
            let backend = Arc::clone(&backend);
            async move {
                let keys = backend.list_keys(black_box(b"")).await.unwrap();
                black_box(keys);
            }
        });
    });

    group.bench_function("list_prefix_1000", |b| {
        b.to_async(&rt).iter(|| {
            let backend = Arc::clone(&backend);
            async move {
                let keys = backend.list_keys(black_box(b"prefix1:")).await.unwrap();
                black_box(keys);
            }
        });
    });

    group.bench_function("list_prefix_500", |b| {
        b.to_async(&rt).iter(|| {
            let backend = Arc::clone(&backend);
            async move {
                let keys = backend.list_keys(black_box(b"prefix2:")).await.unwrap();
                black_box(keys);
            }
        });
    });

    group.finish();
}

fn bench_checkpoint(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let backend = rt.block_on(create_backend());

    // Prepare data
    rt.block_on(async {
        for i in 0..100 {
            let key = format!("checkpoint:data:{}", i).into_bytes();
            let value = vec![0u8; 1024];
            backend.put(&key, &value).await.unwrap();
        }
    });

    let mut group = c.benchmark_group("postgres_checkpoint");

    group.bench_function("create_checkpoint_100_entries", |b| {
        b.to_async(&rt).iter(|| {
            let backend = Arc::clone(&backend);
            async move {
                let checkpoint_id = backend.create_checkpoint().await.unwrap();
                black_box(checkpoint_id);
            }
        });
    });

    // Create a checkpoint for restore testing
    let checkpoint_id = rt.block_on(async { backend.create_checkpoint().await.unwrap() });

    group.bench_function("restore_checkpoint_100_entries", |b| {
        b.to_async(&rt).iter(|| {
            let backend = Arc::clone(&backend);
            async move {
                backend
                    .restore_checkpoint(black_box(checkpoint_id))
                    .await
                    .unwrap();
            }
        });
    });

    group.finish();
}

fn bench_concurrent_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let backend = rt.block_on(create_backend());

    let mut group = c.benchmark_group("postgres_concurrent");

    for num_tasks in [1, 5, 10, 20].iter() {
        group.throughput(Throughput::Elements(*num_tasks as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(num_tasks),
            num_tasks,
            |b, &num_tasks| {
                b.to_async(&rt).iter(|| {
                    let backend = Arc::clone(&backend);
                    async move {
                        let mut handles = Vec::new();

                        for i in 0..num_tasks {
                            let backend = Arc::clone(&backend);
                            let handle = tokio::spawn(async move {
                                let key = format!("concurrent:{}", i).into_bytes();
                                let value = vec![0u8; 256];
                                backend.put(&key, &value).await.unwrap();
                                backend.get(&key).await.unwrap();
                            });
                            handles.push(handle);
                        }

                        for handle in handles {
                            handle.await.unwrap();
                        }
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_metadata_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let backend = rt.block_on(create_backend());

    let mut group = c.benchmark_group("postgres_metadata");

    group.bench_function("put_with_metadata", |b| {
        let mut counter = 0u64;
        b.to_async(&rt).iter(|| {
            let backend = Arc::clone(&backend);
            let key = format!("metadata:{}", counter).into_bytes();
            counter += 1;

            async move {
                let metadata = serde_json::json!({
                    "source": "kafka",
                    "partition": 5,
                    "offset": counter,
                    "timestamp": "2024-01-15T10:30:00Z"
                });

                backend
                    .put_with_metadata(black_box(&key), black_box(b"value"), metadata)
                    .await
                    .unwrap();
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_put,
    bench_get,
    bench_batch_put,
    bench_list_keys,
    bench_checkpoint,
    bench_concurrent_operations,
    bench_metadata_operations,
);
criterion_main!(benches);
