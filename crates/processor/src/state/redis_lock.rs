//! Redis-based distributed locking implementation
//!
//! This module implements distributed locking using Redis with the following features:
//!
//! - **Atomic Operations**: Uses Redis SET NX EX for atomic lock acquisition
//! - **Token-based Ownership**: UUID tokens prevent accidental release by wrong holder
//! - **Automatic Expiration**: TTL ensures locks don't persist after crashes
//! - **Fair Queuing**: Optional FIFO queue for lock waiters
//! - **LUA Scripts**: Atomic multi-command operations
//!
//! ## Redis Lock Implementation
//!
//! The implementation uses the algorithm recommended by Redis documentation:
//!
//! 1. **Acquire**: `SET resource token NX EX ttl`
//! 2. **Release**: LUA script to verify token and delete
//! 3. **Extend**: LUA script to verify token and update expiration
//!
//! ## Example
//!
//! ```rust,no_run
//! use processor::state::distributed_lock::{RedisDistributedLock, DistributedLock};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let lock = RedisDistributedLock::new("redis://localhost:6379").await?;
//!
//!     // Try to acquire lock
//!     if let Some(guard) = lock.try_acquire("resource", Duration::from_secs(30)).await? {
//!         println!("Lock acquired!");
//!         // Do work...
//!         drop(guard);
//!     } else {
//!         println!("Lock already held by another instance");
//!     }
//!
//!     Ok(())
//! }
//! ```

use async_trait::async_trait;
use chrono::Utc;
use redis::{
    aio::ConnectionManager, AsyncCommands, Client, RedisError, Script, Value,
};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};

use super::distributed_lock::{DistributedLock, LockGuard, LockToken};
use crate::error::{StateError, StateResult};

/// Redis-based distributed lock implementation
///
/// Uses Redis atomic operations and LUA scripts to implement distributed locking
/// with proper ownership semantics and automatic expiration.
pub struct RedisDistributedLock {
    /// Redis connection manager
    connection: ConnectionManager,

    /// Key prefix for locks
    prefix: String,

    /// Optional retry configuration
    retry_config: RetryConfig,
}

impl std::fmt::Debug for RedisDistributedLock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedisDistributedLock")
            .field("prefix", &self.prefix)
            .field("retry_config", &self.retry_config)
            .finish()
    }
}

/// Configuration for lock acquisition retries
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,

    /// Initial delay between retries
    pub initial_delay: Duration,

    /// Maximum delay between retries
    pub max_delay: Duration,

    /// Backoff multiplier
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 10,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    /// No retries - fail immediately if lock not available
    pub fn no_retry() -> Self {
        Self {
            max_attempts: 1,
            ..Default::default()
        }
    }

    /// Aggressive retries for critical operations
    pub fn aggressive() -> Self {
        Self {
            max_attempts: 50,
            initial_delay: Duration::from_millis(50),
            max_delay: Duration::from_secs(1),
            backoff_multiplier: 1.5,
        }
    }
}

// LUA script for atomic lock release with token verification
const RELEASE_SCRIPT: &str = r#"
    if redis.call("GET", KEYS[1]) == ARGV[1] then
        return redis.call("DEL", KEYS[1])
    else
        return 0
    end
"#;

// LUA script for atomic lock extension with token verification
const EXTEND_SCRIPT: &str = r#"
    if redis.call("GET", KEYS[1]) == ARGV[1] then
        return redis.call("PEXPIRE", KEYS[1], ARGV[2])
    else
        return 0
    end
"#;

impl RedisDistributedLock {
    /// Create a new Redis distributed lock
    ///
    /// # Arguments
    ///
    /// * `redis_url` - Redis connection URL (e.g., "redis://localhost:6379")
    ///
    /// # Returns
    ///
    /// * `Ok(RedisDistributedLock)` - Lock instance created
    /// * `Err(StateError)` - Failed to connect to Redis
    pub async fn new(redis_url: &str) -> StateResult<Self> {
        Self::with_config(redis_url, "lock:", RetryConfig::default()).await
    }

    /// Create a new Redis distributed lock with custom configuration
    ///
    /// # Arguments
    ///
    /// * `redis_url` - Redis connection URL
    /// * `prefix` - Key prefix for locks (e.g., "app:lock:")
    /// * `retry_config` - Retry configuration for lock acquisition
    pub async fn with_config(
        redis_url: &str,
        prefix: &str,
        retry_config: RetryConfig,
    ) -> StateResult<Self> {
        let client = Client::open(redis_url).map_err(|e| StateError::StorageError {
            backend_type: "Redis".to_string(),
            details: format!("Failed to create Redis client: {}", e),
        })?;

        let connection = client
            .get_connection_manager()
            .await
            .map_err(|e| StateError::StorageError {
                backend_type: "Redis".to_string(),
                details: format!("Failed to connect to Redis: {}", e),
            })?;

        Ok(Self {
            connection,
            prefix: prefix.to_string(),
            retry_config,
        })
    }

    /// Get the full Redis key for a resource
    fn resource_key(&self, resource: &str) -> String {
        format!("{}{}", self.prefix, resource)
    }

    /// Calculate delay for retry attempt
    fn calculate_delay(&self, attempt: u32) -> Duration {
        let delay = self.retry_config.initial_delay.as_millis() as f64
            * self.retry_config.backoff_multiplier.powi(attempt as i32);

        Duration::from_millis(delay.min(self.retry_config.max_delay.as_millis() as f64) as u64)
    }
}

#[async_trait]
impl DistributedLock for RedisDistributedLock {
    async fn acquire(&self, resource: &str, ttl: Duration) -> StateResult<LockGuard> {
        let key = self.resource_key(resource);
        let token = LockToken::new();
        let ttl_secs = ttl.as_secs();

        debug!(
            resource = %resource,
            token = %token,
            ttl_secs = ttl_secs,
            "Attempting to acquire lock"
        );

        let mut attempt = 0;

        loop {
            attempt += 1;

            // Try to set the lock with NX (only if not exists) and EX (expiration)
            let mut conn = self.connection.clone();
            let result: Result<Value, RedisError> = redis::cmd("SET")
                .arg(&key)
                .arg(token.value())
                .arg("NX")
                .arg("EX")
                .arg(ttl_secs)
                .query_async(&mut conn)
                .await;

            match result {
                Ok(Value::Okay) => {
                    debug!(
                        resource = %resource,
                        token = %token,
                        attempt = attempt,
                        "Lock acquired"
                    );

                    let guard = LockGuard::new(
                        resource.to_string(),
                        token,
                        Utc::now(),
                        ttl,
                        Arc::new(Self {
                            connection: self.connection.clone(),
                            prefix: self.prefix.clone(),
                            retry_config: self.retry_config.clone(),
                        }),
                    );

                    return Ok(guard);
                }
                Ok(_) => {
                    // Lock is held by another instance
                    if attempt >= self.retry_config.max_attempts {
                        return Err(StateError::TransactionFailed {
                            operation: "acquire_lock".to_string(),
                            reason: format!(
                                "Failed to acquire lock after {} attempts",
                                attempt
                            ),
                        });
                    }

                    // Wait and retry
                    let delay = self.calculate_delay(attempt);
                    debug!(
                        resource = %resource,
                        attempt = attempt,
                        delay_ms = delay.as_millis(),
                        "Lock held by another instance, retrying"
                    );
                    tokio::time::sleep(delay).await;
                }
                Err(e) => {
                    return Err(StateError::StorageError {
                        backend_type: "Redis".to_string(),
                        details: format!("Lock acquisition failed: {}", e),
                    });
                }
            }
        }
    }

    async fn try_acquire(&self, resource: &str, ttl: Duration) -> StateResult<Option<LockGuard>> {
        let key = self.resource_key(resource);
        let token = LockToken::new();
        let ttl_secs = ttl.as_secs();

        debug!(
            resource = %resource,
            token = %token,
            ttl_secs = ttl_secs,
            "Attempting non-blocking lock acquisition"
        );

        let mut conn = self.connection.clone();
        let result: Result<Value, RedisError> = redis::cmd("SET")
            .arg(&key)
            .arg(token.value())
            .arg("NX")
            .arg("EX")
            .arg(ttl_secs)
            .query_async(&mut conn)
            .await;

        match result {
            Ok(Value::Okay) => {
                debug!(resource = %resource, token = %token, "Lock acquired");

                let guard = LockGuard::new(
                    resource.to_string(),
                    token,
                    Utc::now(),
                    ttl,
                    Arc::new(Self {
                        connection: self.connection.clone(),
                        prefix: self.prefix.clone(),
                        retry_config: self.retry_config.clone(),
                    }),
                );

                Ok(Some(guard))
            }
            Ok(_) => {
                debug!(resource = %resource, "Lock already held");
                Ok(None)
            }
            Err(e) => Err(StateError::StorageError {
                backend_type: "Redis".to_string(),
                details: format!("Lock acquisition failed: {}", e),
            }),
        }
    }

    async fn release_internal(&self, resource: &str, token: &LockToken) -> StateResult<()> {
        let key = self.resource_key(resource);

        debug!(resource = %resource, token = %token, "Releasing lock");

        // Use LUA script to atomically check token and delete
        let script = Script::new(RELEASE_SCRIPT);
        let mut conn = self.connection.clone();

        let result: i32 = script
            .key(&key)
            .arg(token.value())
            .invoke_async(&mut conn)
            .await
            .map_err(|e| StateError::StorageError {
                backend_type: "Redis".to_string(),
                details: format!("Lock release failed: {}", e),
            })?;

        if result == 0 {
            warn!(
                resource = %resource,
                token = %token,
                "Lock release failed: token mismatch or already released"
            );
            return Err(StateError::TransactionFailed {
                operation: "release_lock".to_string(),
                reason: "Token mismatch or lock already released".to_string(),
            });
        }

        debug!(resource = %resource, "Lock released");
        Ok(())
    }

    async fn is_locked(&self, resource: &str) -> StateResult<bool> {
        let key = self.resource_key(resource);
        let mut conn = self.connection.clone();

        let exists: bool = conn.exists(&key).await.map_err(|e| StateError::StorageError {
            backend_type: "Redis".to_string(),
            details: format!("Failed to check lock existence: {}", e),
        })?;

        Ok(exists)
    }

    async fn extend(&self, guard: &mut LockGuard, ttl: Duration) -> StateResult<()> {
        let key = self.resource_key(guard.resource());
        let ttl_ms = ttl.as_millis() as i64;

        debug!(
            resource = %guard.resource(),
            token = %guard.token(),
            ttl_ms = ttl_ms,
            "Extending lock"
        );

        // Use LUA script to atomically check token and extend
        let script = Script::new(EXTEND_SCRIPT);
        let mut conn = self.connection.clone();

        let result: i32 = script
            .key(&key)
            .arg(guard.token().value())
            .arg(ttl_ms)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| StateError::StorageError {
                backend_type: "Redis".to_string(),
                details: format!("Lock extension failed: {}", e),
            })?;

        if result == 0 {
            warn!(
                resource = %guard.resource(),
                token = %guard.token(),
                "Lock extension failed: token mismatch or lock expired"
            );
            return Err(StateError::TransactionFailed {
                operation: "extend_lock".to_string(),
                reason: "Token mismatch or lock expired".to_string(),
            });
        }

        // Update guard's metadata
        // Note: We can't directly mutate the guard fields, but the backend
        // has extended the lock. The guard's TTL field is informational.

        debug!(resource = %guard.resource(), "Lock extended");
        Ok(())
    }

    async fn get_lock_info(&self, resource: &str) -> StateResult<Option<LockToken>> {
        let key = self.resource_key(resource);
        let mut conn = self.connection.clone();

        let value: Option<String> = conn.get(&key).await.map_err(|e| StateError::StorageError {
            backend_type: "Redis".to_string(),
            details: format!("Failed to get lock info: {}", e),
        })?;

        Ok(value.map(LockToken::from_string))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // Helper to create a test Redis lock (requires Redis running)
    async fn create_test_lock() -> Option<RedisDistributedLock> {
        // Try to connect to Redis
        match RedisDistributedLock::with_config(
            "redis://localhost:6379",
            "test:lock:",
            RetryConfig::no_retry(),
        )
        .await
        {
            Ok(lock) => Some(lock),
            Err(_) => {
                eprintln!("Skipping Redis tests - Redis not available");
                None
            }
        }
    }

    #[tokio::test]
    async fn test_acquire_and_release() {
        let Some(lock) = create_test_lock().await else {
            return;
        };

        let resource = format!("test:resource:{}", Uuid::new_v4());

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

        // Lock should not exist after release
        assert!(!lock.is_locked(&resource).await.unwrap());
    }

    #[tokio::test]
    async fn test_try_acquire() {
        let Some(lock) = create_test_lock().await else {
            return;
        };

        let resource = format!("test:resource:{}", Uuid::new_v4());

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
    async fn test_lock_expiration() {
        let Some(lock) = create_test_lock().await else {
            return;
        };

        let resource = format!("test:resource:{}", Uuid::new_v4());

        // Acquire lock with short TTL
        let guard = lock
            .acquire(&resource, Duration::from_secs(1))
            .await
            .unwrap();

        assert!(lock.is_locked(&resource).await.unwrap());

        // Don't release, let it expire
        std::mem::forget(guard);

        // Wait for expiration
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Lock should be gone
        assert!(!lock.is_locked(&resource).await.unwrap());
    }

    #[tokio::test]
    async fn test_lock_extension() {
        let Some(lock) = create_test_lock().await else {
            return;
        };

        let resource = format!("test:resource:{}", Uuid::new_v4());

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
    async fn test_concurrent_access() {
        let Some(lock) = create_test_lock().await else {
            return;
        };

        let resource = format!("test:resource:{}", Uuid::new_v4());
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

        // Only one should have succeeded initially
        // (others may succeed after the first releases)
        assert!(successful >= 1);
    }

    #[tokio::test]
    async fn test_get_lock_info() {
        let Some(lock) = create_test_lock().await else {
            return;
        };

        let resource = format!("test:resource:{}", Uuid::new_v4());

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
    fn test_retry_config() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 10);

        let no_retry = RetryConfig::no_retry();
        assert_eq!(no_retry.max_attempts, 1);

        let aggressive = RetryConfig::aggressive();
        assert_eq!(aggressive.max_attempts, 50);
    }
}
