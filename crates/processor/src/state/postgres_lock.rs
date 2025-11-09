//! PostgreSQL-based distributed locking implementation
//!
//! This module implements distributed locking using PostgreSQL advisory locks.
//! Advisory locks are a PostgreSQL feature that provides application-level locks
//! with excellent performance and automatic cleanup on connection termination.
//!
//! ## Features
//!
//! - **Advisory Locks**: Uses PostgreSQL's built-in advisory lock system
//! - **Session/Transaction Level**: Support for both session and transaction locks
//! - **Automatic Cleanup**: Locks automatically released on connection close
//! - **No External Dependencies**: Uses your existing PostgreSQL database
//! - **Deadlock Detection**: PostgreSQL handles deadlock detection automatically
//!
//! ## Lock Types
//!
//! PostgreSQL advisory locks come in two flavors:
//!
//! 1. **Session Locks**: Held until explicitly released or connection closes
//! 2. **Transaction Locks**: Automatically released at transaction end
//!
//! This implementation uses session locks for consistency with the trait API.
//!
//! ## Example
//!
//! ```rust,no_run
//! use processor::state::distributed_lock::{PostgresDistributedLock, DistributedLock};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let lock = PostgresDistributedLock::new("postgresql://localhost/mydb").await?;
//!
//!     // Acquire lock
//!     let guard = lock.acquire("checkpoint", Duration::from_secs(30)).await?;
//!
//!     // Do critical work
//!     println!("Performing checkpoint...");
//!
//!     // Lock automatically released on drop
//!     drop(guard);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Implementation Notes
//!
//! PostgreSQL advisory locks use a 64-bit integer key. We hash the resource name
//! to generate this key. While hash collisions are theoretically possible, they
//! are extremely rare with a good hash function (we use SipHash via the standard
//! library's hasher).

use async_trait::async_trait;
use chrono::Utc;
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use super::distributed_lock::{DistributedLock, LockGuard, LockToken};
use crate::error::{StateError, StateResult};

/// PostgreSQL-based distributed lock implementation
///
/// Uses PostgreSQL advisory locks for distributed coordination. Advisory locks
/// are fast, automatically cleaned up on connection termination, and support
/// both blocking and non-blocking acquisition.
pub struct PostgresDistributedLock {
    /// Database connection pool
    pool: PgPool,

    /// Lock metadata storage
    /// Maps resource name to (lock_id, token, expiry_time)
    metadata: Arc<RwLock<std::collections::HashMap<String, (i64, LockToken, i64)>>>,

    /// Background task handle for TTL monitoring
    _monitor_handle: Option<tokio::task::JoinHandle<()>>,
}

impl std::fmt::Debug for PostgresDistributedLock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostgresDistributedLock")
            .field("metadata_count", &self.metadata.try_read().map(|m| m.len()))
            .finish()
    }
}

impl PostgresDistributedLock {
    /// Create a new PostgreSQL distributed lock
    ///
    /// # Arguments
    ///
    /// * `database_url` - PostgreSQL connection URL
    ///
    /// # Returns
    ///
    /// * `Ok(PostgresDistributedLock)` - Lock instance created
    /// * `Err(StateError)` - Failed to connect to database
    pub async fn new(database_url: &str) -> StateResult<Self> {
        Self::with_pool_size(database_url, 10).await
    }

    /// Create a new PostgreSQL distributed lock with custom pool size
    ///
    /// # Arguments
    ///
    /// * `database_url` - PostgreSQL connection URL
    /// * `max_connections` - Maximum number of connections in the pool
    pub async fn with_pool_size(database_url: &str, max_connections: u32) -> StateResult<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(database_url)
            .await
            .map_err(|e| StateError::StorageError {
                backend_type: "PostgreSQL".to_string(),
                details: format!("Failed to connect to database: {}", e),
            })?;

        let metadata = Arc::new(RwLock::new(std::collections::HashMap::new()));

        // Start background task to monitor TTLs
        let monitor_handle = Self::start_ttl_monitor(metadata.clone(), pool.clone());

        Ok(Self {
            pool,
            metadata,
            _monitor_handle: Some(monitor_handle),
        })
    }

    /// Start background task to monitor and release expired locks
    fn start_ttl_monitor(
        metadata: Arc<RwLock<std::collections::HashMap<String, (i64, LockToken, i64)>>>,
        pool: PgPool,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));

            loop {
                interval.tick().await;

                let now = Utc::now().timestamp_millis();
                let mut metadata = metadata.write().await;
                let mut expired = Vec::new();

                // Find expired locks
                for (resource, (lock_id, token, expiry)) in metadata.iter() {
                    if *expiry <= now {
                        expired.push((resource.clone(), *lock_id, token.clone()));
                    }
                }

                // Release expired locks
                for (resource, lock_id, token) in expired {
                    debug!(
                        resource = %resource,
                        lock_id = lock_id,
                        token = %token,
                        "Releasing expired lock"
                    );

                    // Release the advisory lock
                    let result = sqlx::query("SELECT pg_advisory_unlock($1)")
                        .bind(lock_id)
                        .execute(&pool)
                        .await;

                    if let Err(e) = result {
                        warn!(
                            resource = %resource,
                            error = %e,
                            "Failed to release expired lock"
                        );
                    }

                    metadata.remove(&resource);
                }
            }
        })
    }

    /// Hash resource name to lock ID
    ///
    /// PostgreSQL advisory locks use a 64-bit integer. We hash the resource
    /// name to generate a consistent lock ID.
    fn resource_to_lock_id(resource: &str) -> i64 {
        let mut hasher = DefaultHasher::new();
        resource.hash(&mut hasher);
        hasher.finish() as i64
    }

    /// Calculate expiry timestamp from TTL
    fn calculate_expiry(ttl: Duration) -> i64 {
        Utc::now().timestamp_millis() + ttl.as_millis() as i64
    }
}

#[async_trait]
impl DistributedLock for PostgresDistributedLock {
    async fn acquire(&self, resource: &str, ttl: Duration) -> StateResult<LockGuard> {
        let lock_id = Self::resource_to_lock_id(resource);
        let token = LockToken::new();
        let expiry = Self::calculate_expiry(ttl);

        debug!(
            resource = %resource,
            lock_id = lock_id,
            token = %token,
            ttl_ms = ttl.as_millis(),
            "Attempting to acquire PostgreSQL advisory lock"
        );

        // Try to acquire the advisory lock (blocking)
        // pg_advisory_lock blocks until the lock is available
        let result = sqlx::query("SELECT pg_advisory_lock($1)")
            .bind(lock_id)
            .execute(&self.pool)
            .await
            .map_err(|e| StateError::StorageError {
                backend_type: "PostgreSQL".to_string(),
                details: format!("Failed to acquire advisory lock: {}", e),
            })?;

        debug!(
            resource = %resource,
            lock_id = lock_id,
            "Advisory lock acquired"
        );

        // Store metadata
        {
            let mut metadata = self.metadata.write().await;
            metadata.insert(resource.to_string(), (lock_id, token.clone(), expiry));
        }

        let guard = LockGuard::new(
            resource.to_string(),
            token,
            Utc::now(),
            ttl,
            Arc::new(Self {
                pool: self.pool.clone(),
                metadata: self.metadata.clone(),
                _monitor_handle: None, // Don't duplicate monitor task
            }),
        );

        Ok(guard)
    }

    async fn try_acquire(&self, resource: &str, ttl: Duration) -> StateResult<Option<LockGuard>> {
        let lock_id = Self::resource_to_lock_id(resource);
        let token = LockToken::new();
        let expiry = Self::calculate_expiry(ttl);

        debug!(
            resource = %resource,
            lock_id = lock_id,
            token = %token,
            ttl_ms = ttl.as_millis(),
            "Attempting non-blocking lock acquisition"
        );

        // Try to acquire the advisory lock (non-blocking)
        // pg_try_advisory_lock returns immediately with true/false
        let row = sqlx::query("SELECT pg_try_advisory_lock($1) as acquired")
            .bind(lock_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StateError::StorageError {
                backend_type: "PostgreSQL".to_string(),
                details: format!("Failed to try advisory lock: {}", e),
            })?;

        let acquired: bool = row.get("acquired");

        if acquired {
            debug!(
                resource = %resource,
                lock_id = lock_id,
                "Advisory lock acquired"
            );

            // Store metadata
            {
                let mut metadata = self.metadata.write().await;
                metadata.insert(resource.to_string(), (lock_id, token.clone(), expiry));
            }

            let guard = LockGuard::new(
                resource.to_string(),
                token,
                Utc::now(),
                ttl,
                Arc::new(Self {
                    pool: self.pool.clone(),
                    metadata: self.metadata.clone(),
                    _monitor_handle: None,
                }),
            );

            Ok(Some(guard))
        } else {
            debug!(resource = %resource, "Lock already held");
            Ok(None)
        }
    }

    async fn release_internal(&self, resource: &str, token: &LockToken) -> StateResult<()> {
        debug!(resource = %resource, token = %token, "Releasing lock");

        // Verify token and get lock ID
        let lock_id = {
            let mut metadata = self.metadata.write().await;

            match metadata.get(resource) {
                Some((lock_id, stored_token, _)) => {
                    if stored_token != token {
                        warn!(
                            resource = %resource,
                            token = %token,
                            "Token mismatch during release"
                        );
                        return Err(StateError::TransactionFailed {
                            operation: "release_lock".to_string(),
                            reason: "Token mismatch".to_string(),
                        });
                    }

                    let lock_id = *lock_id;
                    metadata.remove(resource);
                    lock_id
                }
                None => {
                    warn!(resource = %resource, "Lock not found in metadata");
                    return Err(StateError::KeyNotFound {
                        key: resource.to_string(),
                    });
                }
            }
        };

        // Release the advisory lock
        let row = sqlx::query("SELECT pg_advisory_unlock($1) as released")
            .bind(lock_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StateError::StorageError {
                backend_type: "PostgreSQL".to_string(),
                details: format!("Failed to release advisory lock: {}", e),
            })?;

        let released: bool = row.get("released");

        if !released {
            warn!(
                resource = %resource,
                lock_id = lock_id,
                "Advisory lock was not held"
            );
            return Err(StateError::TransactionFailed {
                operation: "release_lock".to_string(),
                reason: "Lock was not held".to_string(),
            });
        }

        debug!(resource = %resource, "Lock released");
        Ok(())
    }

    async fn is_locked(&self, resource: &str) -> StateResult<bool> {
        let metadata = self.metadata.read().await;
        let is_locked = metadata.contains_key(resource);

        // Also verify the lock hasn't expired
        if is_locked {
            if let Some((_, _, expiry)) = metadata.get(resource) {
                let now = Utc::now().timestamp_millis();
                return Ok(*expiry > now);
            }
        }

        Ok(is_locked)
    }

    async fn extend(&self, guard: &mut LockGuard, ttl: Duration) -> StateResult<()> {
        let new_expiry = Self::calculate_expiry(ttl);

        debug!(
            resource = %guard.resource(),
            token = %guard.token(),
            ttl_ms = ttl.as_millis(),
            "Extending lock"
        );

        // Update metadata with new expiry
        {
            let mut metadata = self.metadata.write().await;

            match metadata.get_mut(guard.resource()) {
                Some((_, stored_token, expiry)) => {
                    if stored_token != guard.token() {
                        warn!(
                            resource = %guard.resource(),
                            "Token mismatch during extension"
                        );
                        return Err(StateError::TransactionFailed {
                            operation: "extend_lock".to_string(),
                            reason: "Token mismatch".to_string(),
                        });
                    }

                    *expiry = new_expiry;
                }
                None => {
                    warn!(resource = %guard.resource(), "Lock not found during extension");
                    return Err(StateError::KeyNotFound {
                        key: guard.resource().to_string(),
                    });
                }
            }
        }

        debug!(resource = %guard.resource(), "Lock extended");
        Ok(())
    }

    async fn get_lock_info(&self, resource: &str) -> StateResult<Option<LockToken>> {
        let metadata = self.metadata.read().await;

        if let Some((_, token, expiry)) = metadata.get(resource) {
            let now = Utc::now().timestamp_millis();
            if *expiry > now {
                return Ok(Some(token.clone()));
            }
        }

        Ok(None)
    }
}

impl Drop for PostgresDistributedLock {
    fn drop(&mut self) {
        // Monitor task will be automatically cancelled when handle is dropped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a test PostgreSQL lock (requires PostgreSQL running)
    async fn create_test_lock() -> Option<PostgresDistributedLock> {
        // Try to connect to PostgreSQL
        match PostgresDistributedLock::new("postgresql://localhost/test").await {
            Ok(lock) => Some(lock),
            Err(_) => {
                eprintln!("Skipping PostgreSQL tests - PostgreSQL not available");
                None
            }
        }
    }

    #[tokio::test]
    async fn test_acquire_and_release() {
        let Some(lock) = create_test_lock().await else {
            return;
        };

        let resource = format!("test:resource:{}", uuid::Uuid::new_v4());

        // Lock should not exist initially
        assert!(!lock.is_locked(&resource).await.unwrap());

        // Acquire lock
        let guard = lock
            .acquire(&resource, Duration::from_secs(10))
            .await
            .unwrap();

        // Lock should exist now
        assert!(lock.is_locked(&resource).await.unwrap());

        // Release lock
        lock.release(guard).await.unwrap();

        // Wait a bit for async release
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Lock should not exist after release
        assert!(!lock.is_locked(&resource).await.unwrap());
    }

    #[tokio::test]
    async fn test_try_acquire() {
        let Some(lock) = create_test_lock().await else {
            return;
        };

        let resource = format!("test:resource:{}", uuid::Uuid::new_v4());

        // First acquisition should succeed
        let guard1 = lock
            .try_acquire(&resource, Duration::from_secs(10))
            .await
            .unwrap();
        assert!(guard1.is_some());

        // Second acquisition should fail
        let guard2 = lock
            .try_acquire(&resource, Duration::from_secs(10))
            .await
            .unwrap();
        assert!(guard2.is_none());

        // Release first lock
        drop(guard1);

        // Wait a bit for async drop to complete
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Third acquisition should succeed
        let guard3 = lock
            .try_acquire(&resource, Duration::from_secs(10))
            .await
            .unwrap();
        assert!(guard3.is_some());
    }

    #[tokio::test]
    async fn test_lock_extension() {
        let Some(lock) = create_test_lock().await else {
            return;
        };

        let resource = format!("test:resource:{}", uuid::Uuid::new_v4());

        // Acquire lock with short TTL
        let mut guard = lock
            .acquire(&resource, Duration::from_secs(2))
            .await
            .unwrap();

        // Wait most of the TTL
        tokio::time::sleep(Duration::from_millis(1500)).await;

        // Extend the lock
        lock.extend(&mut guard, Duration::from_secs(5))
            .await
            .unwrap();

        // Wait past original TTL
        tokio::time::sleep(Duration::from_millis(1000)).await;

        // Lock should still exist
        assert!(lock.is_locked(&resource).await.unwrap());
    }

    #[tokio::test]
    async fn test_lock_expiration() {
        let Some(lock) = create_test_lock().await else {
            return;
        };

        let resource = format!("test:resource:{}", uuid::Uuid::new_v4());

        // Acquire lock with short TTL
        let guard = lock
            .acquire(&resource, Duration::from_secs(1))
            .await
            .unwrap();

        assert!(lock.is_locked(&resource).await.unwrap());

        // Don't release, let it expire
        std::mem::forget(guard);

        // Wait for expiration + monitor cycle
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Lock should be gone
        assert!(!lock.is_locked(&resource).await.unwrap());
    }

    #[tokio::test]
    async fn test_get_lock_info() {
        let Some(lock) = create_test_lock().await else {
            return;
        };

        let resource = format!("test:resource:{}", uuid::Uuid::new_v4());

        // No lock initially
        assert!(lock.get_lock_info(&resource).await.unwrap().is_none());

        // Acquire lock
        let guard = lock
            .acquire(&resource, Duration::from_secs(10))
            .await
            .unwrap();

        // Should return the token
        let token = lock.get_lock_info(&resource).await.unwrap();
        assert!(token.is_some());
        assert_eq!(token.unwrap(), *guard.token());
    }

    #[test]
    fn test_resource_to_lock_id() {
        let id1 = PostgresDistributedLock::resource_to_lock_id("resource1");
        let id2 = PostgresDistributedLock::resource_to_lock_id("resource2");
        let id3 = PostgresDistributedLock::resource_to_lock_id("resource1");

        // Same resource should give same ID
        assert_eq!(id1, id3);

        // Different resources should give different IDs (with high probability)
        assert_ne!(id1, id2);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let Some(lock) = create_test_lock().await else {
            return;
        };

        let resource = format!("test:resource:{}", uuid::Uuid::new_v4());
        let lock = Arc::new(lock);

        let mut handles = vec![];

        // Spawn 5 tasks trying to acquire the same lock
        for i in 0..5 {
            let lock = lock.clone();
            let resource = resource.clone();

            let handle = tokio::spawn(async move {
                if let Some(guard) = lock
                    .try_acquire(&resource, Duration::from_secs(1))
                    .await
                    .unwrap()
                {
                    // Simulate work
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    drop(guard);
                    Some(i)
                } else {
                    None
                }
            });

            handles.push(handle);
        }

        // Collect results
        let mut successful = 0;
        for handle in handles {
            if handle.await.unwrap().is_some() {
                successful += 1;
            }
        }

        // At least one should have succeeded
        assert!(successful >= 1);
    }
}
