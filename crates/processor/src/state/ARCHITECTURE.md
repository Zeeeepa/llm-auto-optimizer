# Stream Processor State Management Architecture

## System Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Stream Processor Application                        │
│                                                                             │
│  ┌────────────────────┐      ┌────────────────────┐                        │
│  │  Event Stream      │─────▶│  Window Processor  │                        │
│  │  (Kafka/etc)       │      │                    │                        │
│  └────────────────────┘      │  - Tumbling        │                        │
│                              │  - Sliding         │                        │
│                              │  - Session         │                        │
│                              └─────────┬──────────┘                        │
│                                        │                                    │
│                                        ▼                                    │
│                              ┌────────────────────┐                        │
│                              │  Window State      │                        │
│                              │  Management        │                        │
│                              └─────────┬──────────┘                        │
│                                        │                                    │
│                                        ▼                                    │
│  ┌────────────────────────────────────────────────────────────────┐       │
│  │              StateBackend Trait Interface                      │       │
│  │  - async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> │       │
│  │  - async fn put(&self, key: &[u8], value: &[u8]) -> Result<()>│       │
│  │  - async fn delete(&self, key: &[u8]) -> Result<()>           │       │
│  │  - async fn list_keys(&self, prefix: &[u8]) -> Result<Vec<>>  │       │
│  └────────────────────────┬───────────────────────────────────────┘       │
│                           │                                                │
└───────────────────────────┼────────────────────────────────────────────────┘
                            │
                            ▼
┌───────────────────────────────────────────────────────────────────────────┐
│                    RedisStateBackend Implementation                       │
│                                                                           │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │  Configuration (RedisConfig)                                    │    │
│  │  - URL(s): redis://host:port                                    │    │
│  │  - Key Prefix: "stream_processor:"                              │    │
│  │  - Pool: min=10, max=50 connections                             │    │
│  │  - TTL: default 3600s                                           │    │
│  │  - Retries: max=3, base_delay=100ms, max_delay=5s              │    │
│  │  - Timeouts: connect=5s, read=3s, write=3s                     │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                                                           │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │  Core Components                                                │    │
│  │                                                                  │    │
│  │  ConnectionManager (Arc)                                        │    │
│  │    - Automatic connection pooling                               │    │
│  │    - Connection lifecycle management                            │    │
│  │    - Health monitoring                                          │    │
│  │                                                                  │    │
│  │  Stats Tracking (Arc<RwLock<RedisStats>>)                      │    │
│  │    - Operation counts (GET, PUT, DELETE)                        │    │
│  │    - Hit/miss rates                                             │    │
│  │    - Latency tracking                                           │    │
│  │    - Error rates                                                │    │
│  │                                                                  │    │
│  │  Lua Scripts (RedisScripts)                                     │    │
│  │    - Atomic get-and-delete                                      │    │
│  │    - Increment with TTL                                         │    │
│  │    - Set if not exists with TTL                                 │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                                                           │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │  Operations                                                      │    │
│  │                                                                  │    │
│  │  Basic Operations:                                              │    │
│  │    ✓ get(key) -> Option<Vec<u8>>                               │    │
│  │    ✓ put(key, value) -> Result<()>                             │    │
│  │    ✓ delete(key) -> Result<()>                                 │    │
│  │    ✓ list_keys(prefix) -> Vec<Vec<u8>>                         │    │
│  │                                                                  │    │
│  │  Advanced Operations:                                           │    │
│  │    ✓ put_with_ttl(key, value, ttl)                             │    │
│  │    ✓ batch_get(keys) -> Vec<Option<Vec<u8>>>                   │    │
│  │    ✓ batch_put(pairs) -> Result<()>                            │    │
│  │    ✓ batch_delete(keys) -> usize                               │    │
│  │    ✓ get_and_delete(key) -> Option<Vec<u8>>                    │    │
│  │    ✓ set_if_not_exists(key, value, ttl) -> bool                │    │
│  │                                                                  │    │
│  │  Monitoring:                                                    │    │
│  │    ✓ health_check() -> bool                                    │    │
│  │    ✓ stats() -> RedisStats                                     │    │
│  │    ✓ memory_usage() -> u64                                     │    │
│  │    ✓ ttl(key) -> Option<Duration>                              │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                                                           │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │  Retry Logic (with_retry)                                       │    │
│  │                                                                  │    │
│  │  1. Attempt operation                                           │    │
│  │  2. If error:                                                   │    │
│  │     a. Increment retry counter                                  │    │
│  │     b. Check if max_retries reached                             │    │
│  │     c. If not, sleep with exponential backoff                   │    │
│  │     d. Retry operation                                          │    │
│  │  3. Track stats (retry_count, error_count)                      │    │
│  │                                                                  │    │
│  │  Delay Formula: min(base_delay * 2^retry, max_delay)           │    │
│  └─────────────────────────────────────────────────────────────────┘    │
└───────────────────────────┬───────────────────────────────────────────────┘
                            │
                            ▼
┌───────────────────────────────────────────────────────────────────────────┐
│                        Redis Server/Cluster                               │
│                                                                           │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐         │
│  │  Redis Node 1   │  │  Redis Node 2   │  │  Redis Node 3   │         │
│  │                 │  │                 │  │                 │         │
│  │  - In-memory    │  │  - In-memory    │  │  - In-memory    │         │
│  │  - Persistence  │  │  - Persistence  │  │  - Persistence  │         │
│  │  - Replication  │  │  - Replication  │  │  - Replication  │         │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘         │
└───────────────────────────────────────────────────────────────────────────┘
```

## Checkpoint System Architecture

```
┌───────────────────────────────────────────────────────────────────────────┐
│                      CheckpointCoordinator                                │
│                                                                           │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │  Configuration (CheckpointOptions)                              │    │
│  │  - checkpoint_dir: /var/lib/checkpoints                         │    │
│  │  - interval: 10 seconds                                         │    │
│  │  - max_checkpoints: 10                                          │    │
│  │  - compress: true                                               │    │
│  │  - min_interval: 10 seconds                                     │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                                                           │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │  Background Task (Tokio)                                        │    │
│  │                                                                  │    │
│  │  loop {                                                         │    │
│  │    select! {                                                    │    │
│  │      _ = interval.tick() => {                                  │    │
│  │        // Check min_interval elapsed                           │    │
│  │        if elapsed >= min_interval {                            │    │
│  │          create_checkpoint()                                   │    │
│  │        }                                                        │    │
│  │      }                                                          │    │
│  │      _ = shutdown_rx => {                                      │    │
│  │        // Create final checkpoint                              │    │
│  │        create_checkpoint()                                     │    │
│  │        break;                                                   │    │
│  │      }                                                          │    │
│  │    }                                                            │    │
│  │  }                                                              │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                                                           │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │  Checkpoint Creation Flow                                       │    │
│  │                                                                  │    │
│  │  1. Generate checkpoint_id (timestamp-based)                    │    │
│  │  2. Export all state from backend                               │    │
│  │     - list_keys("") to get all keys                            │    │
│  │     - get(key) for each key                                    │    │
│  │  3. Create Checkpoint object                                    │    │
│  │     - Calculate size (sum of key+value lengths)                │    │
│  │     - Calculate SHA-256 checksum                               │    │
│  │     - Add metadata                                              │    │
│  │  4. Serialize checkpoint (bincode)                              │    │
│  │  5. Write to file: checkpoint_{timestamp}.ckpt                 │    │
│  │  6. Update statistics                                           │    │
│  │  7. Clean up old checkpoints (keep max_checkpoints)            │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                                                           │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │  Checkpoint Restoration Flow                                    │    │
│  │                                                                  │    │
│  │  1. Find latest checkpoint file                                 │    │
│  │     - Scan checkpoint_dir                                       │    │
│  │     - Filter *.ckpt files                                      │    │
│  │     - Sort by modification time                                 │    │
│  │  2. Load checkpoint from file                                   │    │
│  │     - Read file                                                 │    │
│  │     - Deserialize (bincode)                                     │    │
│  │  3. Validate checkpoint                                         │    │
│  │     - Recalculate checksum                                      │    │
│  │     - Compare with stored checksum                              │    │
│  │     - Fail if mismatch                                          │    │
│  │  4. Restore to backend                                          │    │
│  │     - Clear existing state                                      │    │
│  │     - Put each key-value pair                                   │    │
│  │  5. Update statistics                                           │    │
│  └─────────────────────────────────────────────────────────────────┘    │
└───────────────────────────────────────────────────────────────────────────┘
```

## Data Flow

### Window State Write Path

```
Event → Window Processor → Serialize (bincode) → RedisStateBackend
                                                         │
                                                         ▼
                                                  with_retry()
                                                         │
                                                         ▼
                                                  ConnectionManager
                                                         │
                                                         ▼
                                              [Retry Loop with backoff]
                                                         │
                                                         ▼
                                                    Redis Server
                                                         │
                                                         ▼
                                                   Update Stats
                                                   (put_count,
                                                    bytes_written,
                                                    latency)
```

### Window State Read Path

```
Window Processor → RedisStateBackend.get(key)
                         │
                         ▼
                   with_retry()
                         │
                         ▼
                   ConnectionManager
                         │
                         ▼
              [Retry Loop with backoff]
                         │
                         ▼
                    Redis Server
                         │
                         ▼
                   Update Stats
                   (get_count,
                    hit/miss,
                    bytes_read,
                    latency)
                         │
                         ▼
                   Deserialize (bincode)
                         │
                         ▼
                   Window State Object
```

### Checkpoint Path

```
Timer (10s) → CheckpointCoordinator
                     │
                     ▼
               Export State
            (list_keys + batch_get)
                     │
                     ▼
              Create Checkpoint
          (data + metadata + checksum)
                     │
                     ▼
              Serialize (bincode)
                     │
                     ▼
          Write to File System
    (checkpoint_{timestamp}.ckpt)
                     │
                     ▼
            Update Statistics
                     │
                     ▼
          Cleanup Old Checkpoints
        (keep max_checkpoints)
```

### Recovery Path

```
Application Start → CheckpointCoordinator.restore_latest()
                              │
                              ▼
                    Find Latest Checkpoint
                   (scan dir, sort by mtime)
                              │
                              ▼
                      Load from File
                              │
                              ▼
                   Deserialize (bincode)
                              │
                              ▼
                     Validate Checksum
                     (SHA-256 verify)
                              │
                              ▼
                      Clear Backend
                              │
                              ▼
                Restore All Key-Value Pairs
                   (batch_put to Redis)
                              │
                              ▼
                  Application Ready
                  (with restored state)
```

## Component Interaction Diagram

```
┌────────────────────────────────────────────────────────────────┐
│  Application Layer                                             │
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │   Kafka      │  │   Window     │  │   Metrics    │        │
│  │   Source     │─▶│   Processor  │─▶│   Collector  │        │
│  └──────────────┘  └──────┬───────┘  └──────────────┘        │
│                            │                                    │
│                            │ Store Window State                 │
│                            ▼                                    │
└────────────────────────────┼───────────────────────────────────┘
                             │
┌────────────────────────────┼───────────────────────────────────┐
│  State Management Layer    │                                    │
│                            ▼                                    │
│  ┌─────────────────────────────────────┐                       │
│  │     StateBackend Trait              │                       │
│  │  (Abstraction for all backends)     │                       │
│  └──────────────┬──────────────────────┘                       │
│                 │                                               │
│                 ├──────────┬──────────┬──────────┐            │
│                 │          │          │          │            │
│                 ▼          ▼          ▼          ▼            │
│  ┌──────────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐    │
│  │   Memory     │ │   Sled   │ │  Redis   │ │ Postgres │    │
│  │   Backend    │ │  Backend │ │ Backend  │ │  Backend │    │
│  └──────────────┘ └──────────┘ └────┬─────┘ └──────────┘    │
│                                      │                         │
│                                      │                         │
│  ┌───────────────────────────────────┼───────────────────┐   │
│  │  RedisStateBackend                │                    │   │
│  │                                    ▼                    │   │
│  │  ┌────────────────┐  ┌────────────────────┐           │   │
│  │  │ Connection     │  │  Retry Logic       │           │   │
│  │  │ Manager        │  │  (exponential      │           │   │
│  │  │ (Pooling)      │  │   backoff)         │           │   │
│  │  └────────────────┘  └────────────────────┘           │   │
│  │                                                        │   │
│  │  ┌────────────────┐  ┌────────────────────┐           │   │
│  │  │ Stats          │  │  Lua Scripts       │           │   │
│  │  │ Tracking       │  │  (Atomic Ops)      │           │   │
│  │  └────────────────┘  └────────────────────┘           │   │
│  └────────────────────────────────────────────────────────┘   │
│                                                                │
│  ┌────────────────────────────────────────────────────────┐   │
│  │  CheckpointCoordinator                                 │   │
│  │                                                        │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │   │
│  │  │  Periodic    │  │  Checkpoint  │  │   Restore    │ │   │
│  │  │  Timer       │─▶│  Creation    │  │   on Start   │ │   │
│  │  │  (10s)       │  │              │  │              │ │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘ │   │
│  │                                                        │   │
│  │  ┌──────────────┐  ┌──────────────┐                   │   │
│  │  │  Retention   │  │  Validation  │                   │   │
│  │  │  Policy      │  │  (SHA-256)   │                   │   │
│  │  └──────────────┘  └──────────────┘                   │   │
│  └────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────┘
                             │
                             │ Network
                             ▼
┌────────────────────────────────────────────────────────────────┐
│  Storage Layer                                                 │
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │   Redis      │  │   Postgres   │  │   File       │        │
│  │   Cluster    │  │   Database   │  │   System     │        │
│  │              │  │              │  │              │        │
│  │  - State     │  │  - State     │  │  - Checkpts  │        │
│  │  - TTL       │  │  - Locks     │  │  - Backups   │        │
│  └──────────────┘  └──────────────┘  └──────────────┘        │
└────────────────────────────────────────────────────────────────┘
```

## Error Handling Flow

```
Operation Request
      │
      ▼
┌─────────────────┐
│  with_retry()   │
└────────┬────────┘
         │
         ▼
┌─────────────────────┐
│  Execute Operation  │
└────────┬────────────┘
         │
         ├──── Success ───────────────────────┐
         │                                    │
         └──── Error                          │
               │                              │
               ▼                              │
         ┌──────────────┐                     │
         │ Retry < Max? │                     │
         └──────┬───────┘                     │
                │                             │
          Yes   │   No                        │
                │    │                        │
                │    └──▶ Return Error        │
                │                             │
                ▼                             │
         ┌──────────────┐                     │
         │ Update Stats │                     │
         │ retry_count  │                     │
         │ error_count  │                     │
         └──────┬───────┘                     │
                │                             │
                ▼                             │
         ┌──────────────┐                     │
         │ Sleep with   │                     │
         │ Exponential  │                     │
         │ Backoff      │                     │
         │              │                     │
         │ delay = min( │                     │
         │   base * 2^n,│                     │
         │   max_delay) │                     │
         └──────┬───────┘                     │
                │                             │
                └──▶ Retry ───────────────────┘
                                              │
                                              ▼
                                        Update Stats
                                        (operation_count,
                                         latency,
                                         bytes_*)
                                              │
                                              ▼
                                        Return Success
```

## Concurrency Model

```
┌─────────────────────────────────────────────────────────────┐
│  Multiple Tokio Tasks                                       │
│                                                             │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  │
│  │  Task 1  │  │  Task 2  │  │  Task 3  │  │  Task N  │  │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘  │
│       │             │             │             │         │
│       └─────────────┴─────────────┴─────────────┘         │
│                           │                                │
└───────────────────────────┼────────────────────────────────┘
                            │
                            ▼
         ┌──────────────────────────────────┐
         │  Arc<RedisStateBackend>          │
         │  (Shared, thread-safe)           │
         └─────────────┬────────────────────┘
                       │
         ┌─────────────┴────────────┐
         │                          │
         ▼                          ▼
┌────────────────┐        ┌────────────────────┐
│ ConnectionMgr  │        │ Arc<RwLock<Stats>> │
│ (Arc)          │        │                    │
│                │        │ - Read: Parallel   │
│ - Pooled       │        │ - Write: Exclusive │
│ - Thread-safe  │        └────────────────────┘
│ - Async clone  │
└────────────────┘
         │
         ▼
┌────────────────┐
│ Redis Server   │
│                │
│ - Concurrent   │
│   requests     │
│ - Pipelining   │
│ - Transactions │
└────────────────┘
```

## State Transition Diagram

```
Application Lifecycle:

    [START]
       │
       ▼
┌──────────────┐
│  Initialize  │
│   Backend    │
└──────┬───────┘
       │
       ▼
┌──────────────┐
│ Restore from │
│  Checkpoint  │──── No checkpoint ───┐
└──────┬───────┘                      │
       │                              │
    Found                             │
       │                              │
       ▼                              │
┌──────────────┐                      │
│   Validate   │                      │
│  Checkpoint  │                      │
└──────┬───────┘                      │
       │                              │
    Valid                             │
       │                              │
       ▼                              │
┌──────────────┐                      │
│   Restore    │                      │
│    State     │                      │
└──────┬───────┘                      │
       │                              │
       └──────────────┬───────────────┘
                      │
                      ▼
              ┌──────────────┐
              │    Start     │
              │ Checkpointing│
              └──────┬───────┘
                     │
                     ▼
              ┌──────────────┐
              │   RUNNING    │◀───────┐
              │              │        │
              │ - Process    │        │
              │   Events     │        │
              │ - Store      │        │
              │   State      │        │
              │ - Periodic   │        │
              │   Checkpoint │────────┘
              └──────┬───────┘   (every 10s)
                     │
              Shutdown Signal
                     │
                     ▼
              ┌──────────────┐
              │    Final     │
              │  Checkpoint  │
              └──────┬───────┘
                     │
                     ▼
              ┌──────────────┐
              │    STOPPED   │
              └──────────────┘
```

## Key Design Decisions

1. **Trait-based Abstraction**: StateBackend trait allows switching between backends (Memory, Sled, Redis, Postgres) without changing application code

2. **Async-first**: All operations use async/await for non-blocking I/O, essential for high-throughput stream processing

3. **Connection Pooling**: Redis ConnectionManager provides automatic pooling, eliminating connection overhead

4. **Retry Logic**: Exponential backoff with configurable limits ensures resilience to transient failures

5. **Bincode Serialization**: Binary format provides 10x performance improvement over JSON with smaller payload sizes

6. **Metrics Tracking**: Comprehensive stats enable monitoring, alerting, and performance optimization

7. **Checkpoint Isolation**: Separate checkpoint files allow point-in-time recovery without corrupting active state

8. **Atomic Operations**: Lua scripts ensure consistency for critical operations like distributed locking

9. **Graceful Shutdown**: Final checkpoint on shutdown ensures no data loss

10. **Type Safety**: Rust's type system with serde ensures compile-time correctness of state serialization
