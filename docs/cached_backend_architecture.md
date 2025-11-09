# Tiered Caching Architecture - Technical Details

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Stream Processor Application                   │
└─────────────────────────┬───────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│              CachedStateBackend<B: StateBackend>                  │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  Layer 1: In-Memory Cache (Moka)                        │    │
│  │  ┌───────────────────────────────────────────────────┐  │    │
│  │  │ • Thread-safe, lock-free concurrent hash map     │  │    │
│  │  │ • LRU/LFU/FIFO eviction policies                 │  │    │
│  │  │ • TTL-based expiration (5-60 minutes)            │  │    │
│  │  │ • Size-bounded (10K-100K entries)                │  │    │
│  │  │ • Latency: ~1-10 microseconds                    │  │    │
│  │  │ • Throughput: 1M+ reads/sec, 500K+ writes/sec    │  │    │
│  │  └───────────────────────────────────────────────────┘  │    │
│  │                         │                                │    │
│  │                         │ Cache Miss                     │    │
│  │                         ▼                                │    │
│  │  ┌───────────────────────────────────────────────────┐  │    │
│  │  │ CacheEntry { value, inserted_at, ttl }           │  │    │
│  │  │ • Metadata tracking for refresh-ahead            │  │    │
│  │  │ • Expiration checking                            │  │    │
│  │  └───────────────────────────────────────────────────┘  │    │
│  └─────────────────────────────────────────────────────────┘    │
│                          │                                       │
│                          │ L1 Miss                               │
│                          ▼                                       │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  Layer 2: Distributed Cache (Redis) - OPTIONAL          │    │
│  │  ┌───────────────────────────────────────────────────┐  │    │
│  │  │ • Shared across processor instances              │  │    │
│  │  │ • Redis SET with EX (expiration)                 │  │    │
│  │  │ • TTL: 1-24 hours                                │  │    │
│  │  │ • Key format: "state:{hex_key}"                  │  │    │
│  │  │ • ConnectionManager for pooling                  │  │    │
│  │  │ • Latency: ~100-500 microseconds                 │  │    │
│  │  │ • Throughput: 100K+ ops/sec                      │  │    │
│  │  └───────────────────────────────────────────────────┘  │    │
│  └─────────────────────────────────────────────────────────┘    │
│                          │                                       │
│                          │ L2 Miss                               │
│                          ▼                                       │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  Layer 3: Persistent Backend (Source of Truth)          │    │
│  │  ┌───────────────────────────────────────────────────┐  │    │
│  │  │ • PostgreSQL: Durable, ACID transactions         │  │    │
│  │  │ • Sled: Embedded B-tree database                 │  │    │
│  │  │ • Memory: In-process testing backend             │  │    │
│  │  │ • Latency: ~1-10 milliseconds                    │  │    │
│  │  │ • Throughput: 10K-50K ops/sec                    │  │    │
│  │  └───────────────────────────────────────────────────┘  │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  Statistics Collector (Atomic Counters)                 │    │
│  │  ┌───────────────────────────────────────────────────┐  │    │
│  │  │ AtomicU64 counters:                              │  │    │
│  │  │ • l1_hits, l1_misses                             │  │    │
│  │  │ • l2_hits, l2_misses                             │  │    │
│  │  │ • l3_hits, l3_misses                             │  │    │
│  │  │ • l1_evictions, l2_evictions                     │  │    │
│  │  │ • writes_total, writes_batched                   │  │    │
│  │  │ • refresh_ahead_count                            │  │    │
│  │  │ • latency_sum_us (per layer)                     │  │    │
│  │  └───────────────────────────────────────────────────┘  │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  Write-Behind Queue (Optional)                          │    │
│  │  ┌───────────────────────────────────────────────────┐  │    │
│  │  │ • RwLock<Vec<(key, value)>>                      │  │    │
│  │  │ • Batch size: 1000-10000 entries                 │  │    │
│  │  │ • Flush interval: 10-100ms                       │  │    │
│  │  │ • Background tokio task                          │  │    │
│  │  └───────────────────────────────────────────────────┘  │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

## Data Flow Diagrams

### Read Path (Get Operation)

```
┌─────────┐
│  get()  │
└────┬────┘
     │
     ▼
┌─────────────────┐
│ Check L1 Cache  │────Yes─────┐
│  (Moka)         │            │
└────┬────────────┘            │
     │ No                      │
     ▼                         │
┌─────────────────┐            │
│ Check L2 Cache  │────Yes────┐│
│  (Redis)        │           ││
└────┬────────────┘           ││
     │ No                     ││
     ▼                        ││
┌─────────────────┐           ││
│ Read from L3    │───Yes────┐││
│  (Backend)      │          │││
└────┬────────────┘          │││
     │ No (Not found)        │││
     ▼                       │││
┌─────────────────┐          │││
│ Return None     │          │││
└─────────────────┘          │││
                             │││
┌────────────────────────────┘││
│ Populate L1 + L2            ││
│ (Read-through)              ││
└─────────────┬───────────────┘│
              │                 │
              ▼                 │
        ┌──────────┐            │
        │ Record   │            │
        │ L3 Hit   │            │
        │ Latency  │            │
        └────┬─────┘            │
             │                  │
             └──────────────────┘
                     │
            ┌────────┴────────┐
            │ Return Value    │
            └─────────────────┘
```

### Write Path (Put Operation)

#### Write-Through Strategy
```
┌─────────┐
│  put()  │
└────┬────┘
     │
     ├──────────────────┬──────────────────┐
     ▼                  ▼                  ▼
┌─────────┐      ┌─────────┐      ┌─────────┐
│ Write   │      │ Write   │      │ Write   │
│ to L1   │      │ to L2   │      │ to L3   │
│ (sync)  │      │ (sync)  │      │ (sync)  │
└────┬────┘      └────┬────┘      └────┬────┘
     │                │                │
     └────────────────┴────────────────┘
                      │
                      ▼
              ┌───────────────┐
              │ Return OK     │
              │ (Strong       │
              │  Consistency) │
              └───────────────┘
```

#### Write-Back Strategy
```
┌─────────┐
│  put()  │
└────┬────┘
     │
     ├──────────────────┐
     ▼                  ▼
┌─────────┐      ┌─────────┐
│ Write   │      │ Write   │
│ to L1   │      │ to L2   │
│ (sync)  │      │ (sync)  │
└────┬────┘      └────┬────┘
     │                │
     └────────┬───────┘
              │
              ▼
      ┌───────────────┐
      │ Return OK     │
      │ (Immediate)   │
      └───────────────┘
              │
              ▼
      ┌───────────────┐
      │ Spawn Async   │
      │ Task:         │
      │ Write to L3   │
      └───────────────┘
```

#### Write-Behind Strategy
```
┌─────────┐
│  put()  │
└────┬────┘
     │
     ├──────────────────┐
     ▼                  ▼
┌─────────┐      ┌─────────┐
│ Write   │      │ Write   │
│ to L1   │      │ to L2   │
│ (sync)  │      │ (sync)  │
└────┬────┘      └────┬────┘
     │                │
     └────────┬───────┘
              │
              ▼
      ┌───────────────┐
      │ Enqueue to    │
      │ Write-Behind  │
      │ Buffer        │
      └───────┬───────┘
              │
              ▼
      ┌───────────────┐
      │ Return OK     │
      │ (Fastest)     │
      └───────────────┘

              ┌───────────────┐
              │ Background    │
              │ Task:         │
              │ Every 10-100ms│
              │ or 1K entries │
              └───────┬───────┘
                      │
                      ▼
              ┌───────────────┐
              │ Batch Write   │
              │ to L3         │
              │ (1K entries)  │
              └───────────────┘
```

## Refresh-Ahead Flow

```
┌─────────────────┐
│ get() on hot key│
└────────┬────────┘
         │
         ▼
┌─────────────────────┐
│ Check CacheEntry    │
│ inserted_at + ttl   │
└────────┬────────────┘
         │
         ▼
┌──────────────────────┐
│ Age > threshold*TTL? │───No──▶ Return value
│  (e.g., 80% of TTL)  │
└────────┬─────────────┘
         │ Yes
         ▼
┌──────────────────────┐
│ Spawn async refresh: │
│ 1. Fetch from L3     │
│ 2. Update L1         │
│ 3. Update L2         │
│ 4. Increment counter │
└──────────────────────┘
         │
         ▼
┌──────────────────────┐
│ Return current value │
│ (non-blocking)       │
└──────────────────────┘
```

## Cache Invalidation Flow

```
┌──────────────┐
│ invalidate() │
└──────┬───────┘
       │
       ├────────────────┬────────────────┐
       ▼                ▼                │
┌──────────┐     ┌──────────┐           │
│ Remove   │     │ Remove   │           │
│ from L1  │     │ from L2  │           │
│ (sync)   │     │ (sync)   │           │
└────┬─────┘     └────┬─────┘           │
     │                │                 │
     └────────┬───────┘                 │
              │                         │
              ▼                         │
      ┌───────────────┐                 │
      │ Return OK     │                 │
      └───────────────┘                 │
                                        │
      Note: L3 (backend) unchanged ────┘
      Next get() will reload from L3
```

## Statistics Collection

```
┌─────────────────────────────────────────┐
│         Every Cache Operation           │
└────────────┬────────────────────────────┘
             │
             ▼
┌──────────────────────────────────────────┐
│ AtomicU64 Counter Updates:               │
│                                          │
│ On L1 Hit:                               │
│   • l1_hits.fetch_add(1)                 │
│   • l1_latency_sum.fetch_add(latency)    │
│                                          │
│ On L1 Miss:                              │
│   • l1_misses.fetch_add(1)               │
│                                          │
│ On L2 Hit:                               │
│   • l2_hits.fetch_add(1)                 │
│   • l2_latency_sum.fetch_add(latency)    │
│                                          │
│ On L3 Hit:                               │
│   • l3_hits.fetch_add(1)                 │
│   • l3_latency_sum.fetch_add(latency)    │
│                                          │
│ On Write:                                │
│   • writes_total.fetch_add(1)            │
│   • writes_batched.fetch_add(1) (if batch)│
│                                          │
│ On Refresh-Ahead:                        │
│   • refresh_ahead_count.fetch_add(1)     │
└──────────────────────────────────────────┘
             │
             ▼
┌──────────────────────────────────────────┐
│ stats() call computes:                   │
│                                          │
│ hit_rate = hits / (hits + misses)        │
│ avg_latency = latency_sum / hits         │
│ overall_hit_rate = total_hits / total    │
└──────────────────────────────────────────┘
```

## Concurrency Model

```
┌─────────────────────────────────────────────┐
│         Multiple Tokio Tasks               │
│  ┌────┐  ┌────┐  ┌────┐  ┌────┐          │
│  │T1  │  │T2  │  │T3  │  │T4  │          │
│  └─┬──┘  └─┬──┘  └─┬──┘  └─┬──┘          │
└────┼───────┼───────┼───────┼──────────────┘
     │       │       │       │
     ▼       ▼       ▼       ▼
┌─────────────────────────────────────────────┐
│    CachedStateBackend (Arc-wrapped)         │
│                                             │
│  L1: Moka Cache                             │
│    ┌────────────────────────────┐           │
│    │ Lock-free concurrent       │           │
│    │ hash map                   │           │
│    │ • No RwLock needed         │           │
│    │ • Optimistic locking       │           │
│    └────────────────────────────┘           │
│                                             │
│  L2: Redis ConnectionManager                │
│    ┌────────────────────────────┐           │
│    │ Connection pool            │           │
│    │ • Async multiplexing       │           │
│    │ • Multiple connections     │           │
│    └────────────────────────────┘           │
│                                             │
│  L3: Backend (Arc<B>)                       │
│    ┌────────────────────────────┐           │
│    │ Backend-specific locking   │           │
│    │ • Memory: DashMap          │           │
│    │ • Sled: Internal locking   │           │
│    │ • PostgreSQL: Connection   │           │
│    │   pool                     │           │
│    └────────────────────────────┘           │
│                                             │
│  Stats: StatsCollector                      │
│    ┌────────────────────────────┐           │
│    │ AtomicU64 counters         │           │
│    │ • Lock-free updates        │           │
│    │ • Relaxed ordering         │           │
│    └────────────────────────────┘           │
│                                             │
│  Write-Behind: RwLock<Vec<>>                │
│    ┌────────────────────────────┐           │
│    │ Short-lived write lock     │           │
│    │ • Only during enqueue      │           │
│    │ • Read-heavy workload      │           │
│    └────────────────────────────┘           │
└─────────────────────────────────────────────┘
```

## Memory Layout

```
CachedStateBackend<B>               ~240 bytes
├─ l1_cache: MokaCache              ~64 bytes (pointer)
│  └─ Internal storage               ~(capacity * 200) bytes
│     • Key: Vec<u8>                 ~24 + key_len
│     • Value: CacheEntry            ~80 + value_len
│     • Metadata                     ~96 bytes
│
├─ l2_cache: Option<ConnectionMgr>  ~72 bytes
│  └─ Connection pool                ~(num_conns * 1KB)
│
├─ backend: Arc<B>                   ~16 bytes (pointer)
│  └─ Actual backend                 Backend-specific
│
├─ config: CacheConfig               ~88 bytes
│
├─ stats: Arc<StatsCollector>        ~16 bytes (pointer)
│  └─ Atomic counters                ~176 bytes (11 * 16)
│
└─ write_behind_queue: Option<Arc>   ~16 bytes (pointer)
   └─ RwLock<Vec<(Vec<u8>, Vec<u8>)>> ~(batch_size * (48 + data_len))

Total overhead (excluding data): ~500 bytes
L1 cache overhead per entry: ~200 bytes
Memory footprint at 10K entries: ~2MB + data size
```

## Performance Characteristics

### Latency Distribution (Typical)

```
Operation          P50      P95      P99      P999
─────────────────────────────────────────────────
L1 Hit            2μs      5μs      10μs     50μs
L2 Hit           150μs    300μs    500μs     1ms
L3 Hit (Memory)   50μs    100μs    200μs    500μs
L3 Hit (Sled)      2ms      5ms     10ms     50ms
L3 Hit (Postgres)  3ms      8ms     15ms     50ms
Write-Through      5ms     15ms     30ms    100ms
Write-Back        10μs     50μs    100μs    500μs
Write-Behind       5μs     10μs     20μs     50μs
```

### Throughput Scaling

```
Concurrent Tasks    L1 Reads/sec    L1 Writes/sec
─────────────────────────────────────────────────
1                      500K              250K
4                      1.5M              800K
8                      2.0M              1.0M
16                     2.5M              1.2M
32                     2.8M              1.3M
```

## Configuration Profiles

### Low-Latency Profile
```rust
CacheConfig {
    l1_capacity: 100_000,                    // Large cache
    l1_ttl: Duration::from_secs(3600),       // 1 hour
    l2_ttl: Duration::from_secs(86400),      // 24 hours
    write_strategy: WriteStrategy::WriteBack, // Async writes
    eviction_policy: EvictionPolicy::LRU,
    refresh_ahead_threshold: Some(0.9),      // Aggressive
}
```

### High-Consistency Profile
```rust
CacheConfig {
    l1_capacity: 10_000,                     // Smaller cache
    l1_ttl: Duration::from_secs(60),         // 1 minute
    l2_ttl: Duration::from_secs(300),        // 5 minutes
    write_strategy: WriteStrategy::WriteThrough,
    eviction_policy: EvictionPolicy::LRU,
    refresh_ahead_threshold: None,           // No refresh-ahead
}
```

### High-Throughput Profile
```rust
CacheConfig {
    l1_capacity: 50_000,
    l1_ttl: Duration::from_secs(600),
    l2_ttl: Duration::from_secs(3600),
    write_strategy: WriteStrategy::WriteBehind,
    eviction_policy: EvictionPolicy::LFU,    // Keep hot keys
    write_behind_interval: Some(Duration::from_millis(50)),
    write_behind_batch_size: Some(10000),
}
```
