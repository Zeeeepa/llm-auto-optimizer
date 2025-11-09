# Distributed Locking System

This document provides comprehensive documentation for the distributed locking implementation in the LLM Auto-Optimizer stream processor.

## Overview

The distributed locking system enables coordination of critical operations across multiple processor instances in a distributed deployment. It prevents race conditions and ensures only one instance executes certain operations at a time.

## Architecture

### Components

1. **DistributedLock Trait** (`distributed_lock.rs`)
   - Core interface for all lock implementations
   - RAII pattern with automatic cleanup
   - Async operations throughout

2. **RedisDistributedLock** (`redis_lock.rs`)
   - Redis-based implementation
   - Atomic operations via LUA scripts
   - Configurable retry with exponential backoff
   - Best for: High-throughput, low-latency locking

3. **PostgresDistributedLock** (`postgres_lock.rs`)
   - PostgreSQL advisory locks
   - Automatic cleanup on connection termination
   - Built-in deadlock detection
   - Best for: When you already use PostgreSQL

4. **LockGuard** (RAII wrapper)
   - Automatically releases lock on drop
   - Tracks ownership via unique token
   - Monitors TTL and expiration

## Usage Examples

### Basic Lock Acquisition

```rust
use processor::state::distributed_lock::{RedisDistributedLock, DistributedLock};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create lock instance
    let lock = RedisDistributedLock::new("redis://localhost:6379").await?;

    // Acquire lock (blocks until available)
    let guard = lock.acquire("my_resource", Duration::from_secs(30)).await?;

    // Critical section - only one instance executes this
    println!("Performing critical operation...");

    // Lock automatically released when guard is dropped
    drop(guard);

    Ok(())
}
```

### Non-blocking Lock Acquisition

```rust
use processor::state::distributed_lock::{RedisDistributedLock, DistributedLock};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let lock = RedisDistributedLock::new("redis://localhost:6379").await?;

    // Try to acquire without blocking
    if let Some(guard) = lock.try_acquire("resource", Duration::from_secs(30)).await? {
        println!("Lock acquired!");
        // Do work...
        drop(guard);
    } else {
        println!("Lock held by another instance");
    }

    Ok(())
}
```

### Lock Extension for Long Operations

```rust
use processor::state::distributed_lock::{RedisDistributedLock, DistributedLock};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let lock = RedisDistributedLock::new("redis://localhost:6379").await?;

    // Acquire with initial 30-second TTL
    let mut guard = lock.acquire("long_operation", Duration::from_secs(30)).await?;

    for i in 0..10 {
        // Do work (takes ~20 seconds)
        perform_work_phase(i).await;

        // Extend lock before it expires
        lock.extend(&mut guard, Duration::from_secs(30)).await?;
    }

    Ok(())
}

async fn perform_work_phase(phase: u32) {
    tokio::time::sleep(Duration::from_secs(20)).await;
}
```

## Use Cases

### 1. Checkpoint Coordination

Ensure only one processor instance creates checkpoints at a time:

```rust
use processor::state::distributed_lock::{RedisDistributedLock, DistributedLock};
use processor::state::{CheckpointCoordinator, StateBackend};
use std::sync::Arc;
use std::time::Duration;

async fn coordinated_checkpoint<B: StateBackend + 'static>(
    backend: Arc<B>,
    lock: Arc<dyn DistributedLock>,
) -> anyhow::Result<()> {
    // Acquire global checkpoint lock
    let guard = lock.acquire("global:checkpoint", Duration::from_secs(60)).await?;

    // Create checkpoint coordinator
    let coordinator = CheckpointCoordinator::new(
        backend,
        Duration::from_secs(300),
        "/checkpoints".into(),
    );

    // Only one instance executes this
    let checkpoint = coordinator.create_checkpoint().await?;

    println!("Checkpoint created: {}", checkpoint.metadata().id);

    Ok(())
}
```

### 2. Leader Election

Elect a single leader instance for coordination tasks:

```rust
use processor::state::distributed_lock::{DistributedLock, LockGuard};
use std::sync::Arc;
use std::time::Duration;

pub struct LeaderElection {
    lock: Arc<dyn DistributedLock>,
    resource: String,
    ttl: Duration,
    guard: Option<LockGuard>,
}

impl LeaderElection {
    pub fn new(lock: Arc<dyn DistributedLock>, cluster_id: &str) -> Self {
        Self {
            lock,
            resource: format!("leader:{}", cluster_id),
            ttl: Duration::from_secs(30),
            guard: None,
        }
    }

    pub async fn try_become_leader(&mut self) -> anyhow::Result<bool> {
        if self.guard.is_some() {
            return Ok(true);
        }

        match self.lock.try_acquire(&self.resource, self.ttl).await? {
            Some(guard) => {
                self.guard = Some(guard);
                Ok(true)
            }
            None => Ok(false),
        }
    }

    pub fn is_leader(&self) -> bool {
        self.guard.is_some()
    }

    pub async fn maintain_leadership(&mut self) -> anyhow::Result<()> {
        if let Some(ref mut guard) = self.guard {
            self.lock.extend(guard, self.ttl).await?;
        }
        Ok(())
    }
}
```

### 3. State Compaction

Prevent concurrent compaction of state partitions:

```rust
use processor::state::distributed_lock::{DistributedLock, RedisDistributedLock};
use std::sync::Arc;
use std::time::Duration;

async fn compact_partition(
    lock: Arc<dyn DistributedLock>,
    partition_id: u32,
) -> anyhow::Result<()> {
    let resource = format!("compaction:partition:{}", partition_id);

    // Try to acquire - skip if another instance is compacting
    if let Some(guard) = lock.try_acquire(&resource, Duration::from_secs(300)).await? {
        println!("Starting compaction for partition {}", partition_id);

        // Perform compaction
        perform_compaction(partition_id).await?;

        println!("Compaction completed for partition {}", partition_id);

        drop(guard);
    } else {
        println!("Partition {} already being compacted", partition_id);
    }

    Ok(())
}

async fn perform_compaction(partition_id: u32) -> anyhow::Result<()> {
    // Compaction logic here
    Ok(())
}
```

### 4. Distributed Task Queue

Prevent work stealing from task queues:

```rust
use processor::state::distributed_lock::DistributedLock;
use std::sync::Arc;
use std::time::Duration;

pub struct DistributedTaskQueue {
    lock: Arc<dyn DistributedLock>,
    queue_name: String,
}

impl DistributedTaskQueue {
    pub async fn claim_task(&self, task_id: &str) -> anyhow::Result<bool> {
        let resource = format!("task:{}:{}", self.queue_name, task_id);

        match self.lock.try_acquire(&resource, Duration::from_secs(60)).await? {
            Some(guard) => {
                // Task claimed, execute it
                self.execute_task(task_id).await?;
                drop(guard);
                Ok(true)
            }
            None => {
                // Task already claimed by another worker
                Ok(false)
            }
        }
    }

    async fn execute_task(&self, task_id: &str) -> anyhow::Result<()> {
        println!("Executing task: {}", task_id);
        Ok(())
    }
}
```

## Implementation Details

### Redis Implementation

**Key Features:**
- Uses `SET resource token NX EX ttl` for atomic lock acquisition
- LUA scripts for atomic release and extension
- Configurable retry with exponential backoff
- Lock queue for fair ordering (optional)

**Lock Key Format:**
```
{prefix}{resource_name}
```

**Value:**
```
{lock_token}  # UUID for ownership verification
```

**LUA Scripts:**

Release script:
```lua
if redis.call("GET", KEYS[1]) == ARGV[1] then
    return redis.call("DEL", KEYS[1])
else
    return 0
end
```

Extend script:
```lua
if redis.call("GET", KEYS[1]) == ARGV[1] then
    return redis.call("PEXPIRE", KEYS[1], ARGV[2])
else
    return 0
end
```

### PostgreSQL Implementation

**Key Features:**
- Uses `pg_advisory_lock` for session-level locks
- Automatic cleanup on connection termination
- Built-in deadlock detection
- Background TTL monitor for expiration

**Lock ID Generation:**
```rust
fn resource_to_lock_id(resource: &str) -> i64 {
    let mut hasher = DefaultHasher::new();
    resource.hash(&mut hasher);
    hasher.finish() as i64
}
```

**Advisory Lock Functions:**
- `pg_advisory_lock(key)` - Blocking acquisition
- `pg_try_advisory_lock(key)` - Non-blocking acquisition
- `pg_advisory_unlock(key)` - Release lock

## Configuration

### Redis Configuration

```rust
use processor::state::distributed_lock::{RedisDistributedLock, RetryConfig};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Custom retry configuration
    let retry_config = RetryConfig {
        max_attempts: 20,
        initial_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(5),
        backoff_multiplier: 2.0,
    };

    let lock = RedisDistributedLock::with_config(
        "redis://localhost:6379",
        "myapp:lock:",  // Custom prefix
        retry_config,
    ).await?;

    Ok(())
}
```

**Retry Presets:**
- `RetryConfig::default()` - Moderate retries (10 attempts)
- `RetryConfig::no_retry()` - No retries (fail fast)
- `RetryConfig::aggressive()` - Many retries (50 attempts)

### PostgreSQL Configuration

```rust
use processor::state::distributed_lock::PostgresDistributedLock;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // With custom pool size
    let lock = PostgresDistributedLock::with_pool_size(
        "postgresql://localhost/mydb",
        20,  // Max connections
    ).await?;

    Ok(())
}
```

## Best Practices

### 1. Choose Appropriate TTL

```rust
// Short operations: 10-30 seconds
let guard = lock.acquire("quick_task", Duration::from_secs(10)).await?;

// Medium operations: 1-5 minutes
let guard = lock.acquire("checkpoint", Duration::from_secs(60)).await?;

// Long operations: Use extension
let mut guard = lock.acquire("compaction", Duration::from_secs(30)).await?;
// Extend periodically...
```

### 2. Handle Lock Acquisition Failures

```rust
match lock.try_acquire("resource", Duration::from_secs(30)).await {
    Ok(Some(guard)) => {
        // Lock acquired, proceed
    }
    Ok(None) => {
        // Lock held by another instance, skip or retry
    }
    Err(e) => {
        // Backend error, log and handle
        eprintln!("Lock error: {}", e);
    }
}
```

### 3. Use try_acquire for Non-Critical Operations

```rust
// For optional optimizations
if let Some(guard) = lock.try_acquire("cache_cleanup", Duration::from_secs(30)).await? {
    cleanup_cache().await?;
} else {
    // Skip cleanup, another instance is doing it
}
```

### 4. Avoid Deadlocks with Multiple Locks

```rust
// Always acquire locks in consistent order
async fn acquire_multiple(
    lock: Arc<dyn DistributedLock>,
    resource1: &str,
    resource2: &str,
) -> anyhow::Result<()> {
    // Sort resources alphabetically
    let (first, second) = if resource1 < resource2 {
        (resource1, resource2)
    } else {
        (resource2, resource1)
    };

    let _guard1 = lock.acquire(first, Duration::from_secs(30)).await?;
    let _guard2 = lock.acquire(second, Duration::from_secs(30)).await?;

    // Both locks acquired safely
    Ok(())
}
```

### 5. Monitor Lock Metrics

```rust
use tracing::{info, warn};

let start = std::time::Instant::now();
let guard = lock.acquire("resource", Duration::from_secs(30)).await?;
let wait_time = start.elapsed();

info!(
    resource = "resource",
    wait_ms = wait_time.as_millis(),
    "Lock acquired"
);

// Do work...

if guard.time_remaining() < Duration::from_secs(5) {
    warn!("Lock about to expire, extending");
    lock.extend(&mut guard, Duration::from_secs(30)).await?;
}
```

## Performance Considerations

### Redis vs PostgreSQL

**Redis:**
- ✅ Lower latency (~1-2ms)
- ✅ Higher throughput
- ✅ Better for high-frequency locking
- ❌ Requires separate Redis instance
- ❌ No built-in deadlock detection

**PostgreSQL:**
- ✅ Uses existing database
- ✅ Built-in deadlock detection
- ✅ Automatic cleanup on crash
- ❌ Higher latency (~5-10ms)
- ❌ Uses connection pool slots

### Optimization Tips

1. **Use connection pooling** - Reuse connections across lock operations
2. **Choose appropriate TTLs** - Longer TTLs reduce extension frequency
3. **Use try_acquire for optional tasks** - Avoid blocking on non-critical operations
4. **Monitor lock contention** - Track wait times and acquisition failures
5. **Consider lock granularity** - Fine-grained locks reduce contention

## Testing

### Unit Tests

Run unit tests (no external dependencies):
```bash
cargo test --package processor distributed_lock::tests
```

### Integration Tests

Require Redis and PostgreSQL running:

```bash
# Start services
docker run -d -p 6379:6379 redis:7-alpine
docker run -d -p 5432:5432 -e POSTGRES_PASSWORD=test postgres:15-alpine

# Run tests
cargo test --package processor --features redis,postgres -- --ignored
```

### Example Tests

```rust
#[tokio::test]
async fn test_basic_lock() {
    let lock = RedisDistributedLock::new("redis://localhost:6379").await.unwrap();

    let guard = lock.acquire("test", Duration::from_secs(10)).await.unwrap();
    assert!(lock.is_locked("test").await.unwrap());

    drop(guard);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!lock.is_locked("test").await.unwrap());
}

#[tokio::test]
async fn test_concurrent_access() {
    let lock = Arc::new(RedisDistributedLock::new("redis://localhost:6379").await.unwrap());

    let mut handles = vec![];
    for _ in 0..10 {
        let lock = lock.clone();
        handles.push(tokio::spawn(async move {
            lock.try_acquire("shared", Duration::from_secs(1)).await.unwrap()
        }));
    }

    let results: Vec<_> = futures::future::join_all(handles).await;
    let acquired = results.iter().filter(|r| r.as_ref().unwrap().is_some()).count();

    // Only one should acquire the lock
    assert_eq!(acquired, 1);
}
```

## Troubleshooting

### Lock Not Released

**Symptom:** Lock persists after instance crash

**Solution:** TTL ensures automatic expiration. Verify TTL is set appropriately.

```rust
// Set shorter TTL for crash recovery
let guard = lock.acquire("resource", Duration::from_secs(30)).await?;
```

### High Lock Contention

**Symptom:** Long wait times for lock acquisition

**Solutions:**
1. Increase lock granularity (more fine-grained locks)
2. Use `try_acquire` to skip when locked
3. Implement backoff and retry logic

```rust
// Implement custom retry with backoff
async fn acquire_with_backoff(
    lock: &dyn DistributedLock,
    resource: &str,
) -> anyhow::Result<LockGuard> {
    let mut delay = Duration::from_millis(100);

    loop {
        if let Some(guard) = lock.try_acquire(resource, Duration::from_secs(30)).await? {
            return Ok(guard);
        }

        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(Duration::from_secs(5));
    }
}
```

### Token Mismatch Errors

**Symptom:** "Token mismatch" error on release

**Cause:** Lock expired or released by TTL monitor

**Solution:** Extend lock more frequently

```rust
// Extend lock well before expiration
if guard.time_remaining() < Duration::from_secs(10) {
    lock.extend(&mut guard, Duration::from_secs(30)).await?;
}
```

## Migration Guide

### From Manual Coordination to Distributed Locks

**Before:**
```rust
// Manual flag in database
if !is_checkpoint_running() {
    set_checkpoint_flag(true);
    create_checkpoint().await?;
    set_checkpoint_flag(false);
}
```

**After:**
```rust
// Automatic coordination with lock
let guard = lock.acquire("checkpoint", Duration::from_secs(60)).await?;
create_checkpoint().await?;
// Lock automatically released
```

### Adding to Existing Checkpoint Coordinator

```rust
use processor::state::{CheckpointCoordinator, distributed_lock::RedisDistributedLock};

// Create lock instance
let lock = Arc::new(RedisDistributedLock::new("redis://localhost:6379").await?);

// Wrap checkpoint creation
let guard = lock.acquire("global:checkpoint", Duration::from_secs(60)).await?;

let coordinator = CheckpointCoordinator::new(backend, interval, path);
coordinator.create_checkpoint().await?;

drop(guard);
```

## Future Enhancements

- [ ] Redis Cluster support with RedLock algorithm
- [ ] Lock metrics and monitoring integration
- [ ] Automatic lock renewal in background
- [ ] Lock priority and queue ordering
- [ ] Distributed semaphores (multiple concurrent holders)
- [ ] Read/write locks (shared/exclusive)
