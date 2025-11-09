# Distributed Locking System - Quick Reference

## Files

- **`distributed_lock.rs`** - Core trait and LockGuard implementation
- **`redis_lock.rs`** - Redis-based distributed lock
- **`postgres_lock.rs`** - PostgreSQL advisory locks
- **`distributed_lock_examples.rs`** - Usage examples and patterns
- **`DISTRIBUTED_LOCKING.md`** - Complete user guide (752 lines)
- **`DISTRIBUTED_LOCK_SUMMARY.md`** - Implementation summary

## Quick Start

### Redis Lock

```rust
use processor::state::{RedisDistributedLock, DistributedLock};
use std::time::Duration;

let lock = RedisDistributedLock::new("redis://localhost:6379").await?;
let guard = lock.acquire("resource", Duration::from_secs(30)).await?;
// Critical section
drop(guard);
```

### PostgreSQL Lock

```rust
use processor::state::{PostgresDistributedLock, DistributedLock};
use std::time::Duration;

let lock = PostgresDistributedLock::new("postgresql://localhost/db").await?;
let guard = lock.acquire("resource", Duration::from_secs(30)).await?;
// Critical section
drop(guard);
```

## Key Patterns

### Leader Election
See `distributed_lock_examples.rs::LeaderElection`

### Checkpoint Coordination
See `distributed_lock_examples.rs::coordinated_checkpoint`

### State Compaction
See `distributed_lock_examples.rs::compact_state_partition`

### Task Queue
See `distributed_lock_examples.rs::DistributedTaskQueue`

## Testing

```bash
# Unit tests (no external dependencies)
cargo test --package processor distributed_lock::tests

# Integration tests (requires Redis + PostgreSQL)
docker run -d -p 6379:6379 redis:7-alpine
docker run -d -p 5432:5432 -e POSTGRES_PASSWORD=test postgres:15-alpine
cargo test --package processor distributed_lock -- --ignored
```

## Documentation

For complete documentation, see **`DISTRIBUTED_LOCKING.md`**
