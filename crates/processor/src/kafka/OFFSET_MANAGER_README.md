# Kafka Offset Manager Implementation

## Overview

The Kafka Offset Manager provides comprehensive offset tracking, persistence, and recovery capabilities for Kafka consumers. It integrates seamlessly with the state backend system to ensure exactly-once processing semantics and fault tolerance.

## File Location

`/workspaces/llm-auto-optimizer/crates/processor/src/kafka/offset_manager.rs`

## Features

### 1. OffsetManager Struct

The main coordinator for offset management with the following capabilities:

- **Per-partition offset tracking**: Independent offset tracking for each topic-partition combination
- **Watermark-based commits**: Commit offsets only after watermark passes to ensure consistency
- **Checkpoint integration**: Saves offsets as part of state checkpoints for recovery
- **Offset lag calculation**: Real-time monitoring of consumer lag
- **Reset strategies**: Support for earliest, latest, specific offset, and timestamp-based resets
- **Automatic commit strategies**: Periodic, event-based, watermark-based, or manual commits

### 2. Key Components

#### TopicPartition
```rust
pub struct TopicPartition {
    pub topic: String,
    pub partition: u32,
}
```
Represents a unique Kafka topic-partition pair with helper methods for conversion and display.

#### OffsetInfo
```rust
pub struct OffsetInfo {
    pub offset: i64,
    pub high_watermark: Option<i64>,
    pub last_updated: DateTime<Utc>,
    pub metadata: Option<String>,
}
```
Stores comprehensive offset information including high watermarks for lag calculation.

#### OffsetCommitStrategy
```rust
pub enum OffsetCommitStrategy {
    Periodic { interval: Duration },
    EveryNEvents { n: usize },
    OnWatermark,
    Manual,
}
```
Defines when and how offsets should be committed.

#### OffsetResetStrategy
```rust
pub enum OffsetResetStrategy {
    Earliest,
    Latest,
    Specific(i64),
    Timestamp(i64),
}
```
Defines strategies for resetting consumer positions.

### 3. OffsetStore Trait

Abstract offset storage with multiple implementations:

```rust
#[async_trait]
pub trait OffsetStore: Send + Sync {
    async fn store_offset(
        &self,
        consumer_group: &str,
        topic_partition: &TopicPartition,
        offset_info: &OffsetInfo,
    ) -> StateResult<()>;

    async fn get_offset(
        &self,
        consumer_group: &str,
        topic_partition: &TopicPartition,
    ) -> StateResult<Option<OffsetInfo>>;

    async fn get_all_offsets(
        &self,
        consumer_group: &str,
    ) -> StateResult<HashMap<TopicPartition, OffsetInfo>>;

    async fn delete_offset(
        &self,
        consumer_group: &str,
        topic_partition: &TopicPartition,
    ) -> StateResult<()>;

    async fn clear_offsets(&self, consumer_group: &str) -> StateResult<()>;
}
```

#### Implementations

1. **InMemoryOffsetStore**: Fast in-memory storage using DashMap for concurrent access
2. **StateBackendOffsetStore**: Persistent storage using any StateBackend implementation

## Usage Examples

### Basic Usage

```rust
use processor::kafka::{OffsetManager, OffsetCommitStrategy, InMemoryOffsetStore};
use processor::state::MemoryStateBackend;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let backend = Arc::new(MemoryStateBackend::new(None));
    let store = Arc::new(InMemoryOffsetStore::new());

    let mut manager = OffsetManager::new(
        "my-consumer-group".to_string(),
        backend,
        store,
        OffsetCommitStrategy::Periodic {
            interval: Duration::from_secs(5),
        },
    );

    // Update offset after processing event
    manager.update_offset("my-topic", 0, 100).await?;

    // Commit offsets
    manager.commit().await?;

    // Check lag
    let lag = manager.lag("my-topic", 0, 200).await?;
    println!("Current lag: {}", lag);

    Ok(())
}
```

### Watermark-Based Commits

```rust
use processor::kafka::{OffsetManager, OffsetCommitStrategy};
use processor::watermark::Watermark;

let mut manager = OffsetManager::new(
    "group".to_string(),
    backend,
    store,
    OffsetCommitStrategy::OnWatermark,
);

// Process events
manager.update_offset("topic", 0, 100).await?;

// Notify of watermark advancement - triggers commit
let watermark = Watermark::new(1000);
manager.notify_watermark(watermark).await?;
```

### Offset Restoration

```rust
// First consumer commits offsets
let mut manager1 = OffsetManager::new(
    "group".to_string(),
    backend.clone(),
    store.clone(),
    OffsetCommitStrategy::Manual,
);

manager1.update_offset("topic", 0, 100).await?;
manager1.commit().await?;

// Second consumer restores from same group
let mut manager2 = OffsetManager::new(
    "group".to_string(),
    backend,
    store,
    OffsetCommitStrategy::Manual,
);

let restored = manager2.restore().await?;
println!("Restored {} offsets", restored);
```

### Offset Reset

```rust
use processor::kafka::OffsetResetStrategy;

// Reset to earliest
manager.reset_offset(
    "topic",
    0,
    OffsetResetStrategy::Earliest,
    None,
).await?;

// Reset to latest
manager.reset_offset(
    "topic",
    0,
    OffsetResetStrategy::Latest,
    Some(500),
).await?;

// Reset to specific offset
manager.reset_offset(
    "topic",
    0,
    OffsetResetStrategy::Specific(250),
    None,
).await?;
```

### Lag Monitoring

```rust
// Update with high watermark
manager.update_offset_with_watermark("topic", 0, 100, 200).await?;
manager.commit().await?;

// Get offset info with lag
let info = manager.get_offset_info("topic", 0).await?;
if let Some(info) = info {
    println!("Offset: {}", info.offset);
    println!("High watermark: {:?}", info.high_watermark);
    println!("Lag: {:?}", info.lag());
}

// Get total lag across all partitions
let total_lag = manager.total_lag().await?;
println!("Total lag: {}", total_lag);
```

## Methods

### Core Operations

- `update_offset(topic, partition, offset)`: Update offset for a partition
- `update_offset_with_watermark(topic, partition, offset, high_watermark)`: Update with lag tracking
- `get_offset(topic, partition)`: Get current committed offset
- `get_offset_info(topic, partition)`: Get detailed offset information
- `commit()`: Commit all pending offsets
- `restore()`: Restore offsets from storage

### Lag and Monitoring

- `lag(topic, partition, high_watermark)`: Calculate current lag
- `total_lag()`: Get total lag across all partitions
- `stats()`: Get commit statistics
- `get_all_committed()`: Get all committed offsets
- `get_all_pending()`: Get all pending offsets

### Reset and Management

- `reset_offset(topic, partition, strategy, high_watermark)`: Reset partition offset
- `notify_watermark(watermark)`: Notify of watermark advancement (for OnWatermark strategy)
- `clear()`: Clear all offsets for consumer group

## Integration with State Backend

The OffsetManager integrates with the state backend system for durability:

```rust
use processor::state::SledStateBackend;
use processor::kafka::StateBackendOffsetStore;

// Use persistent state backend
let backend = Arc::new(SledStateBackend::new("/var/lib/kafka/state").await?);
let store = Arc::new(StateBackendOffsetStore::new(backend.clone()));

let manager = OffsetManager::new(
    "persistent-group".to_string(),
    backend,
    store,
    OffsetCommitStrategy::Periodic {
        interval: Duration::from_secs(5),
    },
);
```

## Testing

The implementation includes comprehensive tests covering:

- Topic-partition operations
- In-memory offset store
- State backend offset store
- Update and commit workflows
- Lag calculation
- Offset restoration
- Reset strategies
- Periodic commit strategy
- Watermark-based commit strategy
- Statistics tracking
- Multiple partition handling
- High watermark tracking

Run tests:
```bash
cargo test -p processor --lib kafka::offset_manager
```

## Statistics and Monitoring

The OffsetManager tracks detailed statistics:

```rust
pub struct OffsetStats {
    pub total_commits: u64,
    pub successful_commits: u64,
    pub failed_commits: u64,
    pub total_offsets_committed: u64,
    pub last_commit_time: Option<DateTime<Utc>>,
    pub avg_commit_duration_ms: u64,
}
```

Access statistics:
```rust
let stats = manager.stats().await;
println!("Total commits: {}", stats.total_commits);
println!("Success rate: {:.2}%",
    stats.successful_commits as f64 / stats.total_commits as f64 * 100.0);
println!("Avg duration: {}ms", stats.avg_commit_duration_ms);
```

## Best Practices

1. **Choose appropriate commit strategy**: 
   - Use `Periodic` for time-based consistency
   - Use `EveryNEvents` for throughput optimization
   - Use `OnWatermark` for exactly-once semantics
   - Use `Manual` for custom control

2. **Monitor lag**: Regularly check `total_lag()` to detect processing delays

3. **Use persistent storage**: StateBackendOffsetStore for production environments

4. **Handle failures**: Check commit return values and stats for failures

5. **Integrate with checkpoints**: Coordinate offset commits with state checkpoints

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      OffsetManager                          │
├─────────────────────────────────────────────────────────────┤
│  - Consumer Group ID                                        │
│  - Commit Strategy                                          │
│  - Pending Offsets (DashMap)                               │
│  - Committed Offsets (DashMap)                             │
│  - Statistics                                               │
└─────────────────────────────────────────────────────────────┘
                          │
                          │ uses
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                     OffsetStore Trait                       │
├─────────────────────────────────────────────────────────────┤
│  - store_offset()                                           │
│  - get_offset()                                             │
│  - get_all_offsets()                                        │
│  - delete_offset()                                          │
│  - clear_offsets()                                          │
└─────────────────────────────────────────────────────────────┘
            │                                  │
            │                                  │
            ▼                                  ▼
┌────────────────────────┐      ┌────────────────────────────┐
│  InMemoryOffsetStore   │      │ StateBackendOffsetStore    │
├────────────────────────┤      ├────────────────────────────┤
│  - DashMap storage     │      │  - StateBackend            │
│  - Fast access         │      │  - Persistent storage      │
│  - No persistence      │      │  - Fault tolerant          │
└────────────────────────┘      └────────────────────────────┘
```

## Thread Safety

All operations are thread-safe and use async-aware concurrency primitives:
- DashMap for concurrent access
- RwLock for statistics
- Mutex for periodic timers and watermarks

## Performance Considerations

- **Batched commits**: Pending offsets are committed in batch for efficiency
- **Concurrent access**: DashMap allows lock-free reads
- **Lazy persistence**: Only committed offsets are persisted
- **Rolling averages**: Statistics use efficient rolling averages

## Error Handling

All operations return `StateResult<T>` for consistent error handling:
- Serialization errors
- Storage errors
- Transaction failures
- Concurrent modification detection

See `crates/processor/src/error.rs` for full error types.
