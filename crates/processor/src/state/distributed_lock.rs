//! Distributed locking for coordinating state access across processor instances
//!
//! This module provides distributed locking mechanisms to coordinate critical operations
//! across multiple processor instances, preventing race conditions in distributed scenarios.
//!
//! ## Key Features
//!
//! - **Multiple Backends**: Redis and PostgreSQL implementations
//! - **RAII Pattern**: Automatic lock release via `LockGuard` drop
//! - **Lock Extension**: Extend TTL for long-running operations
//! - **Deadlock Detection**: Automatic expiration via TTL
//! - **Fair Locking**: FIFO queue for waiting locks (Redis)
//! - **Atomic Operations**: LUA scripts and database transactions
//!
//! ## Use Cases
//!
//! - **Checkpoint Coordination**: Only one instance checkpoints at a time
//! - **State Compaction**: Prevent concurrent compaction operations
//! - **Leader Election**: Single coordinator for distributed tasks
//! - **Critical Section Protection**: Serialize access to shared resources
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use processor::state::distributed_lock::{RedisDistributedLock, DistributedLock};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Create Redis-based distributed lock
//!     let lock = RedisDistributedLock::new("redis://localhost:6379").await?;
//!
//!     // Acquire lock with 30-second TTL
//!     let guard = lock.acquire("checkpoint:state1", Duration::from_secs(30)).await?;
//!
//!     // Perform critical operation
//!     println!("Performing checkpoint...");
//!
//!     // Lock is automatically released when guard is dropped
//!     drop(guard);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Lock Extension
//!
//! For long-running operations, extend the lock TTL:
//!
//! ```rust,no_run
//! # use processor::state::distributed_lock::{RedisDistributedLock, DistributedLock};
//! # use std::time::Duration;
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! # let lock = RedisDistributedLock::new("redis://localhost:6379").await?;
//! let mut guard = lock.acquire("compaction:state1", Duration::from_secs(30)).await?;
//!
//! // Start long-running operation
//! for i in 0..10 {
//!     // Do work...
//!     tokio::time::sleep(Duration::from_secs(10)).await;
//!
//!     // Extend lock before it expires
//!     lock.extend(&mut guard, Duration::from_secs(30)).await?;
//! }
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use crate::error::StateResult;

/// Lock ownership token
///
/// A unique identifier for a lock instance, used to verify ownership
/// during extension and release operations.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LockToken(String);

impl LockToken {
    /// Create a new random lock token
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Get the token value
    pub fn value(&self) -> &str {
        &self.0
    }

    /// Create from an existing string (for deserialization)
    pub fn from_string(s: String) -> Self {
        Self(s)
    }
}

impl Default for LockToken {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for LockToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// RAII guard for distributed locks
///
/// Automatically releases the lock when dropped. Contains metadata about
/// the lock for verification and monitoring purposes.
#[derive(Debug)]
pub struct LockGuard {
    /// Resource being locked
    resource: String,

    /// Ownership token
    token: LockToken,

    /// When the lock was acquired
    acquired_at: DateTime<Utc>,

    /// Time-to-live for the lock
    ttl: Duration,

    /// Reference to the lock backend for cleanup
    backend: Arc<dyn DistributedLock>,
}

impl LockGuard {
    /// Create a new lock guard
    pub fn new(
        resource: String,
        token: LockToken,
        acquired_at: DateTime<Utc>,
        ttl: Duration,
        backend: Arc<dyn DistributedLock>,
    ) -> Self {
        Self {
            resource,
            token,
            acquired_at,
            ttl,
            backend,
        }
    }

    /// Get the locked resource name
    pub fn resource(&self) -> &str {
        &self.resource
    }

    /// Get the lock token
    pub fn token(&self) -> &LockToken {
        &self.token
    }

    /// Get the acquisition timestamp
    pub fn acquired_at(&self) -> DateTime<Utc> {
        self.acquired_at
    }

    /// Get the lock TTL
    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    /// Check if the lock has expired (based on TTL)
    pub fn is_expired(&self) -> bool {
        let elapsed = Utc::now().signed_duration_since(self.acquired_at);
        elapsed.to_std().unwrap_or_default() >= self.ttl
    }

    /// Get time remaining before expiration
    pub fn time_remaining(&self) -> Duration {
        let elapsed = Utc::now().signed_duration_since(self.acquired_at);
        let elapsed_std = elapsed.to_std().unwrap_or_default();

        if elapsed_std >= self.ttl {
            Duration::from_secs(0)
        } else {
            self.ttl - elapsed_std
        }
    }

    /// Manually release the lock early
    ///
    /// This consumes the guard and releases the lock. Normally not needed
    /// as the lock is released automatically on drop.
    pub async fn release(self) -> StateResult<()> {
        // Release happens automatically via Drop, but we can force it here
        drop(self);
        Ok(())
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        // Schedule lock release in background
        // We can't await in drop, so we spawn a task
        let backend = self.backend.clone();
        let resource = self.resource.clone();
        let token = self.token.clone();

        tokio::spawn(async move {
            // Best effort release - ignore errors as lock will expire anyway
            let _ = backend.release_internal(&resource, &token).await;
        });
    }
}

/// Distributed lock trait
///
/// Defines the interface for distributed locking implementations.
/// All operations are async and use the RAII pattern via `LockGuard`.
#[async_trait]
pub trait DistributedLock: Send + Sync + std::fmt::Debug {
    /// Acquire a lock for the given resource
    ///
    /// Blocks until the lock is acquired or an error occurs.
    /// The lock will automatically expire after the TTL.
    ///
    /// # Arguments
    ///
    /// * `resource` - Name of the resource to lock
    /// * `ttl` - Time-to-live for the lock
    ///
    /// # Returns
    ///
    /// * `Ok(LockGuard)` - Lock acquired successfully
    /// * `Err(StateError)` - Failed to acquire lock
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use processor::state::distributed_lock::{RedisDistributedLock, DistributedLock};
    /// # use std::time::Duration;
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// # let lock = RedisDistributedLock::new("redis://localhost:6379").await?;
    /// let guard = lock.acquire("checkpoint", Duration::from_secs(30)).await?;
    /// // Critical section
    /// drop(guard); // Lock released
    /// # Ok(())
    /// # }
    /// ```
    async fn acquire(&self, resource: &str, ttl: Duration) -> StateResult<LockGuard>;

    /// Try to acquire a lock without blocking
    ///
    /// Returns immediately with success or failure. Useful for non-blocking scenarios.
    ///
    /// # Arguments
    ///
    /// * `resource` - Name of the resource to lock
    /// * `ttl` - Time-to-live for the lock
    ///
    /// # Returns
    ///
    /// * `Ok(Some(LockGuard))` - Lock acquired
    /// * `Ok(None)` - Lock already held by another instance
    /// * `Err(StateError)` - Backend error
    async fn try_acquire(&self, resource: &str, ttl: Duration) -> StateResult<Option<LockGuard>>;

    /// Release a lock
    ///
    /// Normally not needed as locks are released automatically when the guard
    /// is dropped. This is provided for explicit control.
    ///
    /// # Arguments
    ///
    /// * `guard` - The lock guard to release
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Lock released
    /// * `Err(StateError)` - Failed to release (token mismatch, already released)
    async fn release(&self, guard: LockGuard) -> StateResult<()> {
        self.release_internal(guard.resource(), guard.token()).await
    }

    /// Internal release method (used by Drop)
    async fn release_internal(&self, resource: &str, token: &LockToken) -> StateResult<()>;

    /// Check if a resource is currently locked
    ///
    /// # Arguments
    ///
    /// * `resource` - Name of the resource to check
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Resource is locked
    /// * `Ok(false)` - Resource is not locked
    /// * `Err(StateError)` - Backend error
    async fn is_locked(&self, resource: &str) -> StateResult<bool>;

    /// Extend the TTL of an existing lock
    ///
    /// Useful for long-running operations that might exceed the initial TTL.
    /// The guard's TTL is updated on success.
    ///
    /// # Arguments
    ///
    /// * `guard` - The lock guard to extend
    /// * `ttl` - New TTL from current time
    ///
    /// # Returns
    ///
    /// * `Ok(())` - TTL extended
    /// * `Err(StateError)` - Failed to extend (token mismatch, lock expired)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use processor::state::distributed_lock::{RedisDistributedLock, DistributedLock};
    /// # use std::time::Duration;
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// # let lock = RedisDistributedLock::new("redis://localhost:6379").await?;
    /// let mut guard = lock.acquire("long_operation", Duration::from_secs(30)).await?;
    ///
    /// // Long operation...
    /// tokio::time::sleep(Duration::from_secs(20)).await;
    ///
    /// // Extend before expiration
    /// lock.extend(&mut guard, Duration::from_secs(30)).await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn extend(&self, guard: &mut LockGuard, ttl: Duration) -> StateResult<()>;

    /// Get information about who holds a lock
    ///
    /// Returns the token of the current lock holder, if any.
    ///
    /// # Arguments
    ///
    /// * `resource` - Name of the resource to query
    ///
    /// # Returns
    ///
    /// * `Ok(Some(LockToken))` - Lock is held
    /// * `Ok(None)` - Lock is not held
    /// * `Err(StateError)` - Backend error
    async fn get_lock_info(&self, resource: &str) -> StateResult<Option<LockToken>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_token_creation() {
        let token1 = LockToken::new();
        let token2 = LockToken::new();

        assert_ne!(token1, token2);
        assert!(!token1.value().is_empty());
    }

    #[test]
    fn test_lock_token_from_string() {
        let original = "test-token-123";
        let token = LockToken::from_string(original.to_string());

        assert_eq!(token.value(), original);
    }

    #[test]
    fn test_lock_token_display() {
        let token = LockToken::from_string("test-123".to_string());
        assert_eq!(format!("{}", token), "test-123");
    }

    // Note: LockGuard tests require a backend implementation
    // See redis_lock.rs and postgres_lock.rs for integration tests
}
