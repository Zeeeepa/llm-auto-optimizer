//! Performance benchmarks for Sled backend compression
//!
//! This benchmark suite measures the performance characteristics of different
//! compression algorithms (LZ4, Snappy, Zstd) with the Sled state backend.

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use processor::state::{CompressionConfig, SledConfig, SledStateBackend, StateBackend};
use std::time::Duration;
use tokio::runtime::Runtime;

/// Create test data of given size with specified compressibility
fn create_test_data(size: usize, compressibility: f64) -> Vec<u8> {
    let unique_bytes = ((1.0 - compressibility) * 256.0).max(1.0) as usize;
    (0..size).map(|i| (i % unique_bytes) as u8).collect()
}

/// Benchmark compression throughput for different algorithms
fn bench_compression_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("compression_throughput");

    // Test with different data sizes
    for size_kb in [1, 10, 100, 1000] {
        let size = size_kb * 1024;
        let data = create_test_data(size, 0.8); // Highly compressible
        group.throughput(Throughput::Bytes(size as u64));

        // Benchmark LZ4 (fast)
        group.bench_with_input(
            BenchmarkId::new("lz4", format!("{}KB", size_kb)),
            &data,
            |b, data| {
                let backend = rt.block_on(async {
                    let temp_dir = std::env::temp_dir()
                        .join(format!("bench_lz4_{}", uuid::Uuid::new_v4()));
                    let config = SledConfig::new(&temp_dir).with_fast_compression();
                    SledStateBackend::open(config).await.unwrap()
                });

                b.to_async(&rt).iter(|| async {
                    backend.put(b"bench_key", data).await.unwrap();
                });
            },
        );

        // Benchmark Snappy (balanced)
        group.bench_with_input(
            BenchmarkId::new("snappy", format!("{}KB", size_kb)),
            &data,
            |b, data| {
                let backend = rt.block_on(async {
                    let temp_dir = std::env::temp_dir()
                        .join(format!("bench_snappy_{}", uuid::Uuid::new_v4()));
                    let config = SledConfig::new(&temp_dir).with_balanced_compression();
                    SledStateBackend::open(config).await.unwrap()
                });

                b.to_async(&rt).iter(|| async {
                    backend.put(b"bench_key", data).await.unwrap();
                });
            },
        );

        // Benchmark Zstd (best compression)
        group.bench_with_input(
            BenchmarkId::new("zstd", format!("{}KB", size_kb)),
            &data,
            |b, data| {
                let backend = rt.block_on(async {
                    let temp_dir = std::env::temp_dir()
                        .join(format!("bench_zstd_{}", uuid::Uuid::new_v4()));
                    let config = SledConfig::new(&temp_dir).with_best_compression();
                    SledStateBackend::open(config).await.unwrap()
                });

                b.to_async(&rt).iter(|| async {
                    backend.put(b"bench_key", data).await.unwrap();
                });
            },
        );

        // Benchmark no compression (baseline)
        group.bench_with_input(
            BenchmarkId::new("none", format!("{}KB", size_kb)),
            &data,
            |b, data| {
                let backend = rt.block_on(async {
                    let temp_dir = std::env::temp_dir()
                        .join(format!("bench_none_{}", uuid::Uuid::new_v4()));
                    let config = SledConfig::new(&temp_dir).without_compression();
                    SledStateBackend::open(config).await.unwrap()
                });

                b.to_async(&rt).iter(|| async {
                    backend.put(b"bench_key", data).await.unwrap();
                });
            },
        );
    }

    group.finish();
}

/// Benchmark decompression throughput
fn bench_decompression_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("decompression_throughput");

    for size_kb in [1, 10, 100, 1000] {
        let size = size_kb * 1024;
        let data = create_test_data(size, 0.8);
        group.throughput(Throughput::Bytes(size as u64));

        // LZ4 decompression
        group.bench_with_input(
            BenchmarkId::new("lz4", format!("{}KB", size_kb)),
            &data,
            |b, data| {
                let backend = rt.block_on(async {
                    let temp_dir = std::env::temp_dir()
                        .join(format!("bench_decomp_lz4_{}", uuid::Uuid::new_v4()));
                    let config = SledConfig::new(&temp_dir).with_fast_compression();
                    let backend = SledStateBackend::open(config).await.unwrap();
                    backend.put(b"bench_key", data).await.unwrap();
                    backend
                });

                b.to_async(&rt).iter(|| async {
                    black_box(backend.get(b"bench_key").await.unwrap());
                });
            },
        );

        // Snappy decompression
        group.bench_with_input(
            BenchmarkId::new("snappy", format!("{}KB", size_kb)),
            &data,
            |b, data| {
                let backend = rt.block_on(async {
                    let temp_dir = std::env::temp_dir()
                        .join(format!("bench_decomp_snappy_{}", uuid::Uuid::new_v4()));
                    let config = SledConfig::new(&temp_dir).with_balanced_compression();
                    let backend = SledStateBackend::open(config).await.unwrap();
                    backend.put(b"bench_key", data).await.unwrap();
                    backend
                });

                b.to_async(&rt).iter(|| async {
                    black_box(backend.get(b"bench_key").await.unwrap());
                });
            },
        );

        // Zstd decompression
        group.bench_with_input(
            BenchmarkId::new("zstd", format!("{}KB", size_kb)),
            &data,
            |b, data| {
                let backend = rt.block_on(async {
                    let temp_dir = std::env::temp_dir()
                        .join(format!("bench_decomp_zstd_{}", uuid::Uuid::new_v4()));
                    let config = SledConfig::new(&temp_dir).with_best_compression();
                    let backend = SledStateBackend::open(config).await.unwrap();
                    backend.put(b"bench_key", data).await.unwrap();
                    backend
                });

                b.to_async(&rt).iter(|| async {
                    black_box(backend.get(b"bench_key").await.unwrap());
                });
            },
        );
    }

    group.finish();
}

/// Benchmark compression with different compressibility levels
fn bench_compressibility_impact(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("compressibility_impact");
    let size = 100 * 1024; // 100KB

    for compressibility in [0.2, 0.5, 0.8, 0.95] {
        let data = create_test_data(size, compressibility);
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(
            BenchmarkId::new("zstd", format!("{:.0}%", compressibility * 100.0)),
            &data,
            |b, data| {
                let backend = rt.block_on(async {
                    let temp_dir = std::env::temp_dir()
                        .join(format!("bench_compress_{}", uuid::Uuid::new_v4()));
                    let config = SledConfig::new(&temp_dir).with_best_compression();
                    SledStateBackend::open(config).await.unwrap()
                });

                b.to_async(&rt).iter(|| async {
                    backend.put(b"bench_key", data).await.unwrap();
                });
            },
        );
    }

    group.finish();
}

/// Benchmark roundtrip (put + get) performance
fn bench_roundtrip(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("roundtrip");
    let data = create_test_data(10 * 1024, 0.8); // 10KB, 80% compressible

    group.throughput(Throughput::Elements(1));

    // LZ4 roundtrip
    group.bench_function("lz4_roundtrip", |b| {
        let backend = rt.block_on(async {
            let temp_dir = std::env::temp_dir().join(format!("bench_rt_lz4_{}", uuid::Uuid::new_v4()));
            let config = SledConfig::new(&temp_dir).with_fast_compression();
            SledStateBackend::open(config).await.unwrap()
        });

        b.to_async(&rt).iter(|| async {
            backend.put(b"rt_key", &data).await.unwrap();
            black_box(backend.get(b"rt_key").await.unwrap());
        });
    });

    // Snappy roundtrip
    group.bench_function("snappy_roundtrip", |b| {
        let backend = rt.block_on(async {
            let temp_dir =
                std::env::temp_dir().join(format!("bench_rt_snappy_{}", uuid::Uuid::new_v4()));
            let config = SledConfig::new(&temp_dir).with_balanced_compression();
            SledStateBackend::open(config).await.unwrap()
        });

        b.to_async(&rt).iter(|| async {
            backend.put(b"rt_key", &data).await.unwrap();
            black_box(backend.get(b"rt_key").await.unwrap());
        });
    });

    // Zstd roundtrip
    group.bench_function("zstd_roundtrip", |b| {
        let backend = rt.block_on(async {
            let temp_dir =
                std::env::temp_dir().join(format!("bench_rt_zstd_{}", uuid::Uuid::new_v4()));
            let config = SledConfig::new(&temp_dir).with_best_compression();
            SledStateBackend::open(config).await.unwrap()
        });

        b.to_async(&rt).iter(|| async {
            backend.put(b"rt_key", &data).await.unwrap();
            black_box(backend.get(b"rt_key").await.unwrap());
        });
    });

    // No compression roundtrip (baseline)
    group.bench_function("none_roundtrip", |b| {
        let backend = rt.block_on(async {
            let temp_dir =
                std::env::temp_dir().join(format!("bench_rt_none_{}", uuid::Uuid::new_v4()));
            let config = SledConfig::new(&temp_dir).without_compression();
            SledStateBackend::open(config).await.unwrap()
        });

        b.to_async(&rt).iter(|| async {
            backend.put(b"rt_key", &data).await.unwrap();
            black_box(backend.get(b"rt_key").await.unwrap());
        });
    });

    group.finish();
}

/// Benchmark compression with small values (below threshold)
fn bench_small_values(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("small_values");

    for size in [64, 128, 256, 512] {
        let data = create_test_data(size, 0.8);
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), &data, |b, data| {
            let backend = rt.block_on(async {
                let temp_dir =
                    std::env::temp_dir().join(format!("bench_small_{}", uuid::Uuid::new_v4()));
                let config = SledConfig::new(&temp_dir).with_balanced_compression();
                SledStateBackend::open(config).await.unwrap()
            });

            b.to_async(&rt).iter(|| async {
                backend.put(b"small_key", data).await.unwrap();
                black_box(backend.get(b"small_key").await.unwrap());
            });
        });
    }

    group.finish();
}

/// Benchmark custom compression levels
fn bench_compression_levels(c: &mut Criterion) {
    use processor::state::compression::{CompressionAlgorithm, CompressionConfig};

    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("compression_levels");
    let data = create_test_data(100 * 1024, 0.8); // 100KB, 80% compressible

    group.throughput(Throughput::Bytes((100 * 1024) as u64));

    // Zstd with different compression levels
    for level in [1, 3, 9, 15, 19] {
        group.bench_with_input(
            BenchmarkId::new("zstd_level", level),
            &level,
            |b, &level| {
                let backend = rt.block_on(async {
                    let temp_dir = std::env::temp_dir()
                        .join(format!("bench_level_{}_{}", level, uuid::Uuid::new_v4()));
                    let compression = CompressionConfig {
                        algorithm: CompressionAlgorithm::Zstd,
                        level,
                        threshold: 128,
                        enable_stats: true,
                    };
                    let config = SledConfig::new(&temp_dir).with_compression(compression);
                    SledStateBackend::open(config).await.unwrap()
                });

                b.to_async(&rt).iter(|| async {
                    backend.put(b"level_key", &data).await.unwrap();
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_compression_throughput,
    bench_decompression_throughput,
    bench_compressibility_impact,
    bench_roundtrip,
    bench_small_values,
    bench_compression_levels,
);

criterion_main!(benches);
