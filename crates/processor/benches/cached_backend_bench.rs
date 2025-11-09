//! Performance benchmarks for the cached state backend

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use processor::state::{
    CacheConfig, CachedStateBackend, EvictionPolicy, MemoryStateBackend, StateBackend,
    WriteStrategy,
};
use std::time::Duration;
use tokio::runtime::Runtime;

/// Benchmark L1 cache hit performance
fn bench_l1_hit(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("l1_cache_hit");
    group.throughput(Throughput::Elements(1));

    let backend = rt.block_on(async {
        let backend = MemoryStateBackend::new(None);
        let config = CacheConfig::default();
        let cached = CachedStateBackend::new(backend, None, config)
            .await
            .unwrap();

        // Prepopulate cache
        cached.put(b"hot_key", b"hot_value").await.unwrap();

        cached
    });

    group.bench_function("get", |b| {
        b.to_async(&rt).iter(|| async {
            black_box(backend.get(b"hot_key").await.unwrap());
        });
    });

    group.finish();
}

/// Benchmark L3 backend hit performance
fn bench_l3_hit(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("l3_backend_hit");
    group.throughput(Throughput::Elements(1));

    let (backend_base, cached) = rt.block_on(async {
        let backend = MemoryStateBackend::new(None);
        backend.put(b"backend_key", b"backend_value").await.unwrap();

        let config = CacheConfig::default();
        let cached = CachedStateBackend::new(backend.clone(), None, config)
            .await
            .unwrap();

        (backend, cached)
    });

    // Clear cache to force L3 access
    rt.block_on(async {
        cached.clear_cache().await.unwrap();
    });

    group.bench_function("get", |b| {
        b.to_async(&rt).iter(|| async {
            black_box(cached.get(b"backend_key").await.unwrap());
        });
    });

    group.finish();
}

/// Benchmark write-through performance
fn bench_write_through(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("write_strategy");
    group.throughput(Throughput::Elements(1));

    let cached = rt.block_on(async {
        let backend = MemoryStateBackend::new(None);
        let config = CacheConfig {
            write_strategy: WriteStrategy::WriteThrough,
            ..Default::default()
        };
        CachedStateBackend::new(backend, None, config)
            .await
            .unwrap()
    });

    group.bench_function("write_through", |b| {
        b.to_async(&rt).iter(|| async {
            black_box(cached.put(b"key", b"value").await.unwrap());
        });
    });

    group.finish();
}

/// Benchmark write-back performance
fn bench_write_back(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("write_strategy");
    group.throughput(Throughput::Elements(1));

    let cached = rt.block_on(async {
        let backend = MemoryStateBackend::new(None);
        let config = CacheConfig {
            write_strategy: WriteStrategy::WriteBack,
            ..Default::default()
        };
        CachedStateBackend::new(backend, None, config)
            .await
            .unwrap()
    });

    group.bench_function("write_back", |b| {
        b.to_async(&rt).iter(|| async {
            black_box(cached.put(b"key", b"value").await.unwrap());
        });
    });

    group.finish();
}

/// Benchmark batch operations
fn bench_batch_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("batch_operations");

    for size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        let cached = rt.block_on(async {
            let backend = MemoryStateBackend::new(None);
            let config = CacheConfig::default();
            CachedStateBackend::new(backend, None, config)
                .await
                .unwrap()
        });

        // Batch put
        group.bench_with_input(BenchmarkId::new("put", size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let pairs: Vec<_> = (0..size)
                    .map(|i| {
                        (
                            format!("key{}", i).into_bytes(),
                            format!("value{}", i).into_bytes(),
                        )
                    })
                    .collect();

                black_box(cached.put_batch(&pairs).await.unwrap());
            });
        });

        // Batch get
        rt.block_on(async {
            // Prepopulate
            for i in 0..*size {
                let key = format!("key{}", i);
                let value = format!("value{}", i);
                cached.put(key.as_bytes(), value.as_bytes()).await.unwrap();
            }
        });

        group.bench_with_input(BenchmarkId::new("get", size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let keys: Vec<_> = (0..size)
                    .map(|i| format!("key{}", i).into_bytes())
                    .collect();

                black_box(cached.get_batch(&keys).await.unwrap());
            });
        });
    }

    group.finish();
}

/// Benchmark cache warming
fn bench_cache_warming(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("cache_warming");

    for size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        let (backend, keys) = rt.block_on(async {
            let backend = MemoryStateBackend::new(None);

            // Prepopulate backend
            let mut keys = Vec::new();
            for i in 0..*size {
                let key = format!("key{}", i).into_bytes();
                let value = format!("value{}", i).into_bytes();
                backend.put(&key, &value).await.unwrap();
                keys.push(key);
            }

            (backend, keys)
        });

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.to_async(&rt).iter(|| async {
                let config = CacheConfig::default();
                let cached = CachedStateBackend::new(backend.clone(), None, config)
                    .await
                    .unwrap();

                black_box(cached.warm_cache(&keys).await.unwrap());
            });
        });
    }

    group.finish();
}

/// Benchmark different cache sizes
fn bench_cache_sizes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("cache_sizes");
    group.throughput(Throughput::Elements(1));

    for capacity in [100, 1000, 10000, 100000].iter() {
        let cached = rt.block_on(async {
            let backend = MemoryStateBackend::new(None);
            let config = CacheConfig {
                l1_capacity: *capacity,
                ..Default::default()
            };
            let cached = CachedStateBackend::new(backend, None, config)
                .await
                .unwrap();

            // Fill cache to capacity
            for i in 0..*capacity {
                let key = format!("key{}", i);
                let value = format!("value{}", i);
                cached.put(key.as_bytes(), value.as_bytes()).await.unwrap();
            }

            cached
        });

        group.bench_with_input(BenchmarkId::from_parameter(capacity), capacity, |b, _| {
            b.to_async(&rt).iter(|| async {
                // Access random key
                let key = format!("key{}", capacity / 2);
                black_box(cached.get(key.as_bytes()).await.unwrap());
            });
        });
    }

    group.finish();
}

/// Benchmark eviction policies
fn bench_eviction_policies(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("eviction_policies");
    group.throughput(Throughput::Elements(1));

    for policy in [EvictionPolicy::LRU, EvictionPolicy::LFU, EvictionPolicy::FIFO].iter() {
        let cached = rt.block_on(async {
            let backend = MemoryStateBackend::new(None);
            let config = CacheConfig {
                l1_capacity: 1000,
                eviction_policy: *policy,
                ..Default::default()
            };
            let cached = CachedStateBackend::new(backend, None, config)
                .await
                .unwrap();

            // Fill cache
            for i in 0..1000 {
                let key = format!("key{}", i);
                let value = format!("value{}", i);
                cached.put(key.as_bytes(), value.as_bytes()).await.unwrap();
            }

            cached
        });

        group.bench_with_input(
            BenchmarkId::new("put_evict", format!("{:?}", policy)),
            policy,
            |b, _| {
                b.to_async(&rt).iter(|| async {
                    // This will trigger eviction
                    black_box(cached.put(b"new_key", b"new_value").await.unwrap());
                });
            },
        );
    }

    group.finish();
}

/// Benchmark concurrent access
fn bench_concurrent_access(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("concurrent_access");

    for num_tasks in [1, 4, 8, 16].iter() {
        group.throughput(Throughput::Elements(*num_tasks as u64));

        let cached = rt.block_on(async {
            let backend = MemoryStateBackend::new(None);
            let config = CacheConfig {
                l1_capacity: 10000,
                ..Default::default()
            };
            let cached = CachedStateBackend::new(backend, None, config)
                .await
                .unwrap();

            // Prepopulate
            for i in 0..1000 {
                let key = format!("key{}", i);
                let value = format!("value{}", i);
                cached.put(key.as_bytes(), value.as_bytes()).await.unwrap();
            }

            std::sync::Arc::new(cached)
        });

        group.bench_with_input(
            BenchmarkId::from_parameter(num_tasks),
            num_tasks,
            |b, &num_tasks| {
                b.to_async(&rt).iter(|| async {
                    let mut handles = Vec::new();

                    for i in 0..num_tasks {
                        let cached_clone = cached.clone();
                        let handle = tokio::spawn(async move {
                            let key = format!("key{}", i % 1000);
                            black_box(cached_clone.get(key.as_bytes()).await.unwrap());
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.await.unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark value sizes
fn bench_value_sizes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("value_sizes");

    for size in [64, 1024, 16384, 65536].iter() {
        // 64B, 1KB, 16KB, 64KB
        group.throughput(Throughput::Bytes(*size as u64));

        let (cached, value) = rt.block_on(async {
            let backend = MemoryStateBackend::new(None);
            let config = CacheConfig::default();
            let cached = CachedStateBackend::new(backend, None, config)
                .await
                .unwrap();

            let value = vec![0u8; *size];
            (cached, value)
        });

        group.bench_with_input(BenchmarkId::new("put", size), size, |b, _| {
            b.to_async(&rt).iter(|| async {
                black_box(cached.put(b"key", &value).await.unwrap());
            });
        });

        // Prepopulate for get benchmark
        rt.block_on(async {
            cached.put(b"key", &value).await.unwrap();
        });

        group.bench_with_input(BenchmarkId::new("get", size), size, |b, _| {
            b.to_async(&rt).iter(|| async {
                black_box(cached.get(b"key").await.unwrap());
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_l1_hit,
    bench_l3_hit,
    bench_write_through,
    bench_write_back,
    bench_batch_operations,
    bench_cache_warming,
    bench_cache_sizes,
    bench_eviction_policies,
    bench_concurrent_access,
    bench_value_sizes,
);

criterion_main!(benches);
