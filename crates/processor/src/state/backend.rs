//! State backend trait definition
//!
//! This module defines the core `StateBackend` trait that all state storage
//! implementations must implement. The trait provides a simple key-value
//! interface with support for listing keys by prefix.

use async_trait::async_trait;

use crate::error::StateResult;

/// Core trait for state backend implementations
///
/// This trait defines the interface for storing and retrieving state in the
/// stream processor. All operations are async to support both in-memory and
/// persistent backends efficiently.
///
/// ## Key Design Principles
///
/// - **Byte-oriented**: Keys and values are byte slices for maximum flexibility
/// - **Async-first**: All operations return futures for non-blocking I/O
/// - **Simple interface**: CRUD operations plus prefix listing
/// - **Error handling**: All operations return `StateResult<T>`
///
/// ## Implementation Requirements
///
/// Implementations must ensure:
///
/// - **Thread safety**: Concurrent access from multiple tasks
/// - **Atomicity**: Individual operations are atomic
/// - **Durability** (optional): Persistent backends should fsync writes
/// - **Cleanup**: Automatic cleanup of expired/deleted state
///
/// ## Example Implementation
///
/// ```rust,no_run
/// use async_trait::async_trait;
/// use processor::state::{StateBackend, StateResult};
/// use std::collections::HashMap;
/// use tokio::sync::RwLock;
///
/// struct SimpleBackend {
///     data: RwLock<HashMap<Vec<u8>, Vec<u8>>>,
/// }
///
/// #[async_trait]
/// impl StateBackend for SimpleBackend {
///     async fn get(&self, key: &[u8]) -> StateResult<Option<Vec<u8>>> {
///         Ok(self.data.read().await.get(key).cloned())
///     }
///
///     async fn put(&self, key: &[u8], value: &[u8]) -> StateResult<()> {
///         self.data.write().await.insert(key.to_vec(), value.to_vec());
///         Ok(())
///     }
///
///     async fn delete(&self, key: &[u8]) -> StateResult<()> {
///         self.data.write().await.remove(key);
///         Ok(())
///     }
///
///     async fn list_keys(&self, prefix: &[u8]) -> StateResult<Vec<Vec<u8>>> {
///         Ok(self.data.read().await
///             .keys()
///             .filter(|k| k.starts_with(prefix))
///             .cloned()
///             .collect())
///     }
/// }
/// ```
#[async_trait]
pub trait StateBackend: Send + Sync {
    /// Retrieve a value for the given key
    ///
    /// Returns `Ok(None)` if the key does not exist or has expired.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to look up (as byte slice)
    ///
    /// # Returns
    ///
    /// * `Ok(Some(value))` - Value found
    /// * `Ok(None)` - Key not found or expired
    /// * `Err(StateError)` - Backend error occurred
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use processor::state::{StateBackend, MemoryStateBackend};
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// let backend = MemoryStateBackend::new(None);
    /// backend.put(b"key1", b"value1").await?;
    ///
    /// match backend.get(b"key1").await? {
    ///     Some(value) => println!("Found: {:?}", value),
    ///     None => println!("Not found"),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    async fn get(&self, key: &[u8]) -> StateResult<Option<Vec<u8>>>;

    /// Store a value for the given key
    ///
    /// If the key already exists, its value is overwritten.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to store (as byte slice)
    /// * `value` - The value to store (as byte slice)
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Value stored successfully
    /// * `Err(StateError)` - Backend error occurred
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use processor::state::{StateBackend, MemoryStateBackend};
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// let backend = MemoryStateBackend::new(None);
    /// backend.put(b"window:123", b"aggregated_state").await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn put(&self, key: &[u8], value: &[u8]) -> StateResult<()>;

    /// Delete a key and its associated value
    ///
    /// This operation is idempotent - deleting a non-existent key is not an error.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to delete (as byte slice)
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Key deleted or didn't exist
    /// * `Err(StateError)` - Backend error occurred
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use processor::state::{StateBackend, MemoryStateBackend};
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// let backend = MemoryStateBackend::new(None);
    /// backend.put(b"temp:123", b"data").await?;
    /// backend.delete(b"temp:123").await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn delete(&self, key: &[u8]) -> StateResult<()>;

    /// List all keys with the given prefix
    ///
    /// This is useful for finding all windows for a specific key, or all state
    /// entries of a certain type.
    ///
    /// # Arguments
    ///
    /// * `prefix` - The prefix to match (as byte slice). Empty prefix matches all keys.
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<Vec<u8>>)` - List of matching keys
    /// * `Err(StateError)` - Backend error occurred
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use processor::state::{StateBackend, MemoryStateBackend};
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// let backend = MemoryStateBackend::new(None);
    /// backend.put(b"window:1", b"data1").await?;
    /// backend.put(b"window:2", b"data2").await?;
    /// backend.put(b"meta:1", b"metadata").await?;
    ///
    /// // List all window keys
    /// let window_keys = backend.list_keys(b"window:").await?;
    /// assert_eq!(window_keys.len(), 2);
    /// # Ok(())
    /// # }
    /// ```
    async fn list_keys(&self, prefix: &[u8]) -> StateResult<Vec<Vec<u8>>>;

    /// Clear all state from the backend
    ///
    /// This is primarily used for testing and cleanup. Use with caution.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - All state cleared
    /// * `Err(StateError)` - Backend error occurred
    async fn clear(&self) -> StateResult<()> {
        let keys = self.list_keys(b"").await?;
        for key in keys {
            self.delete(&key).await?;
        }
        Ok(())
    }

    /// Get the number of keys in the backend
    ///
    /// This is an optional operation that may be expensive for some backends.
    ///
    /// # Returns
    ///
    /// * `Ok(count)` - Number of keys
    /// * `Err(StateError)` - Backend error occurred
    async fn count(&self) -> StateResult<usize> {
        Ok(self.list_keys(b"").await?.len())
    }

    /// Check if a key exists
    ///
    /// This is more efficient than `get()` when you only need to check existence.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to check (as byte slice)
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Key exists
    /// * `Ok(false)` - Key does not exist
    /// * `Err(StateError)` - Backend error occurred
    async fn contains(&self, key: &[u8]) -> StateResult<bool> {
        Ok(self.get(key).await?.is_some())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    // Helper to create a test key
    fn test_key(id: u32) -> Vec<u8> {
        format!("test:{}", id).into_bytes()
    }

    // Generic tests that any StateBackend implementation should pass
    pub async fn test_backend_basic_ops<B: StateBackend>(backend: B) {
        // Test put and get
        backend.put(&test_key(1), b"value1").await.unwrap();
        let value = backend.get(&test_key(1)).await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));

        // Test get non-existent key
        let value = backend.get(&test_key(999)).await.unwrap();
        assert_eq!(value, None);

        // Test overwrite
        backend.put(&test_key(1), b"new_value").await.unwrap();
        let value = backend.get(&test_key(1)).await.unwrap();
        assert_eq!(value, Some(b"new_value".to_vec()));

        // Test delete
        backend.delete(&test_key(1)).await.unwrap();
        let value = backend.get(&test_key(1)).await.unwrap();
        assert_eq!(value, None);

        // Test delete non-existent (should not error)
        backend.delete(&test_key(999)).await.unwrap();
    }

    pub async fn test_backend_list_keys<B: StateBackend>(backend: B) {
        // Clear any existing data
        backend.clear().await.unwrap();

        // Add some keys with different prefixes
        backend.put(b"window:1", b"w1").await.unwrap();
        backend.put(b"window:2", b"w2").await.unwrap();
        backend.put(b"window:3", b"w3").await.unwrap();
        backend.put(b"meta:1", b"m1").await.unwrap();
        backend.put(b"meta:2", b"m2").await.unwrap();

        // Test prefix matching
        let window_keys = backend.list_keys(b"window:").await.unwrap();
        assert_eq!(window_keys.len(), 3);

        let meta_keys = backend.list_keys(b"meta:").await.unwrap();
        assert_eq!(meta_keys.len(), 2);

        // Test empty prefix (all keys)
        let all_keys = backend.list_keys(b"").await.unwrap();
        assert_eq!(all_keys.len(), 5);
    }

    pub async fn test_backend_clear<B: StateBackend>(backend: B) {
        backend.put(b"key1", b"val1").await.unwrap();
        backend.put(b"key2", b"val2").await.unwrap();
        backend.put(b"key3", b"val3").await.unwrap();

        assert_eq!(backend.count().await.unwrap(), 3);

        backend.clear().await.unwrap();

        assert_eq!(backend.count().await.unwrap(), 0);
    }

    pub async fn test_backend_contains<B: StateBackend>(backend: B) {
        backend.clear().await.unwrap();

        assert!(!backend.contains(b"test").await.unwrap());

        backend.put(b"test", b"value").await.unwrap();

        assert!(backend.contains(b"test").await.unwrap());

        backend.delete(b"test").await.unwrap();

        assert!(!backend.contains(b"test").await.unwrap());
    }
}
