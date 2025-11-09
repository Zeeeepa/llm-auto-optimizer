//! State management for stream processing
//!
//! This module provides state backend abstractions and implementations for maintaining
//! window state across the distributed stream processing system. It supports:
//!
//! - **Multiple Backends**: In-memory (with TTL), persistent (Sled), and distributed (Redis) storage
//! - **Checkpointing**: Periodic state snapshots for fault tolerance
//! - **Distributed Locking**: Coordinate state access across multiple processor instances
//! - **Async Operations**: All state operations are async-first
//! - **Type Safety**: Generic state serialization/deserialization
//!
//! ## Architecture
//!
//! The state module is organized around four core concepts:
//!
//! 1. **StateBackend Trait**: Defines the interface for all state backends
//! 2. **Backend Implementations**: Memory, Sled-based persistent, and Redis distributed storage
//! 3. **Checkpoint Coordination**: Manages periodic snapshots and recovery
//! 4. **Distributed Locking**: Redis and PostgreSQL-based distributed locks
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use processor::state::{MemoryStateBackend, StateBackend};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Create an in-memory state backend with TTL
//!     let backend = MemoryStateBackend::new(Some(Duration::from_secs(3600)));
//!
//!     // Store state
//!     backend.put(b"window:123", b"state_data").await?;
//!
//!     // Retrieve state
//!     if let Some(data) = backend.get(b"window:123").await? {
//!         println!("Retrieved: {:?}", data);
//!     }
//!
//!     // List all windows
//!     let keys = backend.list_keys(b"window:").await?;
//!     println!("Found {} windows", keys.len());
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Checkpointing
//!
//! The checkpoint coordinator automatically creates state snapshots:
//!
//! ```rust,no_run
//! use processor::state::{CheckpointCoordinator, MemoryStateBackend};
//! use std::sync::Arc;
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let backend = Arc::new(MemoryStateBackend::new(None));
//!
//!     // Create coordinator with 5-minute intervals
//!     let coordinator = CheckpointCoordinator::new(
//!         backend.clone(),
//!         Duration::from_secs(300),
//!         "/tmp/checkpoints".into(),
//!     );
//!
//!     // Start periodic checkpointing
//!     coordinator.start().await?;
//!
//!     // Work with state...
//!
//!     // Checkpoint is created automatically
//!     coordinator.shutdown().await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Distributed Locking
//!
//! Coordinate critical operations across multiple processor instances:
//!
//! ```rust,no_run
//! use processor::state::distributed_lock::{RedisDistributedLock, DistributedLock};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let lock = RedisDistributedLock::new("redis://localhost:6379").await?;
//!
//!     // Acquire lock for checkpoint coordination
//!     let guard = lock.acquire("checkpoint:state1", Duration::from_secs(30)).await?;
//!
//!     // Perform checkpoint - only one instance will execute this
//!     println!("Creating checkpoint...");
//!
//!     // Lock automatically released when guard is dropped
//!     drop(guard);
//!
//!     Ok(())
//! }
//! ```

pub mod backend;
pub mod cached_backend;
pub mod checkpoint;
pub mod distributed_lock;
pub mod memory;
pub mod postgres_backend;
pub mod postgres_lock;
pub mod redis_backend;
pub mod redis_lock;
pub mod sled_backend;

// Re-export main types
pub use backend::StateBackend;
pub use cached_backend::{
    CacheConfig, CacheStats, CachedStateBackend, EvictionPolicy, WriteStrategy,
};
pub use checkpoint::{
    Checkpoint, CheckpointCoordinator, CheckpointMetadata, CheckpointOptions, CheckpointStats,
};
pub use distributed_lock::{DistributedLock, LockGuard, LockToken};
pub use memory::MemoryStateBackend;
pub use postgres_backend::{CheckpointInfo, PostgresConfig, PostgresStateBackend, PostgresStats};
pub use postgres_lock::PostgresDistributedLock;
pub use redis_backend::{RedisConfig, RedisConfigBuilder, RedisStateBackend, RedisStats};
pub use redis_lock::{RedisDistributedLock, RetryConfig};
pub use sled_backend::SledStateBackend;

/// Type alias for state operation results
pub type StateResult<T> = Result<T, crate::error::StateError>;
