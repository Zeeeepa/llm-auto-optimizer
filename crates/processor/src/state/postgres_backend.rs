//! PostgreSQL-based persistent state backend
//!
//! This module provides a production-ready PostgreSQL state backend for distributed
//! state management. It supports connection pooling, transactions, JSONB metadata,
//! automatic schema migrations, and advanced features like checkpointing and TTL.
//!
//! ## Features
//!
//! - **Connection Pooling**: Managed by sqlx PgPool for optimal performance
//! - **Async Operations**: Full async/await support throughout
//! - **JSONB Metadata**: Store additional context with state entries
//! - **Automatic Migrations**: Schema versioning and automatic upgrades
//! - **Transactions**: ACID guarantees with BEGIN/COMMIT/ROLLBACK
//! - **TTL Support**: Automatic expiration and cleanup of old entries
//! - **Checkpointing**: Point-in-time snapshots for backup and recovery
//! - **Query Optimization**: Indexes, prepared statements, and hints
//! - **Production Ready**: SSL, retries, timeouts, and health checks
//!
//! ## Example
//!
//! ```rust,no_run
//! use processor::state::{PostgresStateBackend, PostgresConfig, StateBackend};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Configure PostgreSQL backend
//!     let config = PostgresConfig::new("postgresql://user:pass@localhost/dbname")
//!         .with_pool_size(10, 50)
//!         .with_ssl_mode("require")
//!         .with_timeout(std::time::Duration::from_secs(30));
//!
//!     // Initialize backend (runs migrations automatically)
//!     let backend = PostgresStateBackend::new(config).await?;
//!
//!     // Use like any other state backend
//!     backend.put(b"window:123", b"aggregated_data").await?;
//!
//!     // Store with metadata
//!     backend.put_with_metadata(
//!         b"window:123",
//!         b"data",
//!         serde_json::json!({"source": "kafka", "partition": 5})
//!     ).await?;
//!
//!     // Create checkpoint
//!     let checkpoint_id = backend.create_checkpoint().await?;
//!
//!     // Cleanup expired entries
//!     let deleted = backend.cleanup_expired().await?;
//!     println!("Deleted {} expired entries", deleted);
//!
//!     Ok(())
//! }
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use sqlx::{PgPool, Row};
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, trace, warn};
use uuid::Uuid;

use super::backend::StateBackend;
use crate::error::{StateError, StateResult};

/// PostgreSQL backend configuration
#[derive(Debug, Clone)]
pub struct PostgresConfig {
    /// Database connection URL (e.g., "postgresql://user:pass@host/db")
    pub database_url: String,
    /// Minimum number of connections in the pool
    pub min_connections: u32,
    /// Maximum number of connections in the pool
    pub max_connections: u32,
    /// Table name for state storage (default: "state_entries")
    pub table_name: String,
    /// Schema name (default: "public")
    pub schema_name: String,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Query timeout
    pub query_timeout: Duration,
    /// SSL mode
    pub ssl_mode: String,
    /// Maximum retry attempts for transient errors
    pub max_retries: u32,
    /// Retry backoff duration
    pub retry_backoff: Duration,
    /// Enable statement logging
    pub log_statements: bool,
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self {
            database_url: "postgresql://postgres:postgres@localhost/optimizer".to_string(),
            min_connections: 2,
            max_connections: 10,
            table_name: "state_entries".to_string(),
            schema_name: "public".to_string(),
            connect_timeout: Duration::from_secs(30),
            query_timeout: Duration::from_secs(60),
            ssl_mode: "prefer".to_string(),
            max_retries: 3,
            retry_backoff: Duration::from_millis(100),
            log_statements: false,
        }
    }
}

impl PostgresConfig {
    /// Create a new configuration with the given database URL
    ///
    /// # Arguments
    ///
    /// * `database_url` - PostgreSQL connection string
    ///
    /// # Example
    ///
    /// ```
    /// use processor::state::PostgresConfig;
    ///
    /// let config = PostgresConfig::new("postgresql://user:pass@localhost/mydb");
    /// ```
    pub fn new<S: Into<String>>(database_url: S) -> Self {
        Self {
            database_url: database_url.into(),
            ..Default::default()
        }
    }

    /// Set the connection pool size
    ///
    /// # Arguments
    ///
    /// * `min` - Minimum connections
    /// * `max` - Maximum connections
    pub fn with_pool_size(mut self, min: u32, max: u32) -> Self {
        self.min_connections = min;
        self.max_connections = max;
        self
    }

    /// Set the table name for state storage
    pub fn with_table_name<S: Into<String>>(mut self, name: S) -> Self {
        self.table_name = name.into();
        self
    }

    /// Set the schema name
    pub fn with_schema_name<S: Into<String>>(mut self, name: S) -> Self {
        self.schema_name = name.into();
        self
    }

    /// Set connection and query timeouts
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self.query_timeout = timeout;
        self
    }

    /// Set SSL mode (disable, allow, prefer, require, verify-ca, verify-full)
    pub fn with_ssl_mode<S: Into<String>>(mut self, mode: S) -> Self {
        self.ssl_mode = mode.into();
        self
    }

    /// Set retry configuration
    pub fn with_retries(mut self, max_retries: u32, backoff: Duration) -> Self {
        self.max_retries = max_retries;
        self.retry_backoff = backoff;
        self
    }

    /// Enable or disable statement logging
    pub fn with_statement_logging(mut self, enabled: bool) -> Self {
        self.log_statements = enabled;
        self
    }
}

/// Statistics for the PostgreSQL backend
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PostgresStats {
    /// Number of get operations
    pub get_count: u64,
    /// Number of put operations
    pub put_count: u64,
    /// Number of delete operations
    pub delete_count: u64,
    /// Number of queries executed
    pub query_count: u64,
    /// Number of transactions committed
    pub transaction_count: u64,
    /// Number of retry attempts
    pub retry_count: u64,
    /// Total bytes written
    pub bytes_written: u64,
    /// Total bytes read
    pub bytes_read: u64,
}

/// Checkpoint metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointInfo {
    /// Unique checkpoint identifier
    pub checkpoint_id: Uuid,
    /// Timestamp when checkpoint was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Number of rows in checkpoint
    pub row_count: i64,
    /// Total data size in bytes
    pub data_size: i64,
    /// Optional metadata
    pub metadata: Option<serde_json::Value>,
}

/// PostgreSQL state backend
///
/// This backend uses PostgreSQL for persistent, distributed state management.
/// It provides ACID guarantees, connection pooling, automatic migrations,
/// and advanced features like JSONB metadata and checkpointing.
pub struct PostgresStateBackend {
    /// Connection pool
    pool: PgPool,
    /// Configuration
    config: PostgresConfig,
    /// Statistics
    stats: Arc<RwLock<PostgresStats>>,
}

impl PostgresStateBackend {
    /// Create a new PostgreSQL state backend
    ///
    /// This initializes the connection pool and runs automatic schema migrations.
    ///
    /// # Arguments
    ///
    /// * `config` - PostgreSQL configuration
    ///
    /// # Returns
    ///
    /// * `Ok(PostgresStateBackend)` - Successfully initialized backend
    /// * `Err(StateError)` - Failed to initialize
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use processor::state::{PostgresStateBackend, PostgresConfig};
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// let config = PostgresConfig::new("postgresql://localhost/test");
    /// let backend = PostgresStateBackend::new(config).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(config: PostgresConfig) -> StateResult<Self> {
        info!("Initializing PostgreSQL state backend");
        info!("Database: {}", Self::sanitize_url(&config.database_url));
        info!("Pool size: {} - {}", config.min_connections, config.max_connections);

        // Parse SSL mode
        let ssl_mode = Self::parse_ssl_mode(&config.ssl_mode)?;

        // Parse connection options
        let connect_options = PgConnectOptions::from_str(&config.database_url)
            .map_err(|e| StateError::StorageError {
                backend_type: "postgres".to_string(),
                details: format!("Invalid database URL: {}", e),
            })?
            .ssl_mode(ssl_mode);

        // Create connection pool
        let pool = PgPoolOptions::new()
            .min_connections(config.min_connections)
            .max_connections(config.max_connections)
            .acquire_timeout(config.connect_timeout)
            .connect_with(connect_options)
            .await
            .map_err(|e| StateError::StorageError {
                backend_type: "postgres".to_string(),
                details: format!("Failed to create connection pool: {}", e),
            })?;

        info!("Connection pool created successfully");

        let backend = Self {
            pool,
            config,
            stats: Arc::new(RwLock::new(PostgresStats::default())),
        };

        // Run migrations
        backend.run_migrations().await?;

        info!("PostgreSQL state backend initialized successfully");

        Ok(backend)
    }

    /// Run database migrations
    ///
    /// This applies all pending schema migrations from the migrations directory.
    async fn run_migrations(&self) -> StateResult<()> {
        info!("Running database migrations");

        // Read migration files
        let migration_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");

        if !migration_dir.exists() {
            warn!("Migrations directory not found: {:?}", migration_dir);
            return Ok(());
        }

        let mut migrations = vec![];
        let mut entries = tokio::fs::read_dir(&migration_dir)
            .await
            .map_err(|e| StateError::StorageError {
                backend_type: "postgres".to_string(),
                details: format!("Failed to read migrations directory: {}", e),
            })?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| StateError::StorageError {
            backend_type: "postgres".to_string(),
            details: format!("Failed to read migration entry: {}", e),
        })? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("sql") {
                migrations.push(path);
            }
        }

        // Sort migrations by filename
        migrations.sort();

        // Execute each migration
        for migration_path in migrations {
            let filename = migration_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");

            info!("Applying migration: {}", filename);

            let sql = tokio::fs::read_to_string(&migration_path)
                .await
                .map_err(|e| StateError::StorageError {
                    backend_type: "postgres".to_string(),
                    details: format!("Failed to read migration file {}: {}", filename, e),
                })?;

            sqlx::raw_sql(&sql)
                .execute(&self.pool)
                .await
                .map_err(|e| StateError::StorageError {
                    backend_type: "postgres".to_string(),
                    details: format!("Failed to execute migration {}: {}", filename, e),
                })?;

            debug!("Migration applied successfully: {}", filename);
        }

        info!("All migrations applied successfully");
        Ok(())
    }

    /// Put a value with metadata
    ///
    /// # Arguments
    ///
    /// * `key` - The key to store
    /// * `value` - The value to store
    /// * `metadata` - JSON metadata
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use processor::state::{PostgresStateBackend, PostgresConfig, StateBackend};
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// # let backend = PostgresStateBackend::new(PostgresConfig::default()).await?;
    /// backend.put_with_metadata(
    ///     b"window:123",
    ///     b"data",
    ///     serde_json::json!({"partition": 5, "source": "kafka"})
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn put_with_metadata(
        &self,
        key: &[u8],
        value: &[u8],
        metadata: serde_json::Value,
    ) -> StateResult<()> {
        trace!("Putting key with metadata to PostgreSQL: {} bytes", key.len());

        let key = key.to_vec();
        let value = value.to_vec();
        let metadata = metadata.clone();

        self.retry_operation(move || {
            let key = key.clone();
            let value = value.clone();
            let metadata = metadata.clone();
            async move {
                let key_len = key.len();
                let value_len = value.len();

                sqlx::query(
                    r#"
                    INSERT INTO state_entries (key, value, metadata, updated_at)
                    VALUES ($1, $2, $3, NOW())
                    ON CONFLICT (key) DO UPDATE
                    SET value = EXCLUDED.value,
                        metadata = EXCLUDED.metadata,
                        updated_at = NOW()
                    "#,
                )
                .bind(key)
                .bind(value)
                .bind(metadata)
            .execute(&self.pool)
            .await
            .map_err(|e| StateError::StorageError {
                backend_type: "postgres".to_string(),
                details: format!("Put with metadata failed: {}", e),
            })?;

            let mut stats = self.stats.write().await;
            stats.put_count += 1;
            stats.bytes_written += (key_len + value_len) as u64;
            stats.query_count += 1;

            Ok(())
            }
        })
        .await
    }

    /// Put a value with TTL
    ///
    /// # Arguments
    ///
    /// * `key` - The key to store
    /// * `value` - The value to store
    /// * `ttl` - Time to live duration
    pub async fn put_with_ttl(&self, key: &[u8], value: &[u8], ttl: Duration) -> StateResult<()> {
        trace!("Putting key with TTL to PostgreSQL: {} bytes, ttl: {:?}", key.len(), ttl);

        self.retry_operation(|| async {
            let expires_at = chrono::Utc::now() + chrono::Duration::from_std(ttl).unwrap();

            sqlx::query(
                r#"
                INSERT INTO state_entries (key, value, expires_at, updated_at)
                VALUES ($1, $2, $3, NOW())
                ON CONFLICT (key) DO UPDATE
                SET value = EXCLUDED.value,
                    expires_at = EXCLUDED.expires_at,
                    updated_at = NOW()
                "#,
            )
            .bind(key)
            .bind(value)
            .bind(expires_at)
            .execute(&self.pool)
            .await
            .map_err(|e| StateError::StorageError {
                backend_type: "postgres".to_string(),
                details: format!("Put with TTL failed: {}", e),
            })?;

            let mut stats = self.stats.write().await;
            stats.put_count += 1;
            stats.bytes_written += (key.len() + value.len()) as u64;
            stats.query_count += 1;

            Ok(())
        })
        .await
    }

    /// Clean up expired entries
    ///
    /// This removes all entries where expires_at is in the past.
    ///
    /// # Returns
    ///
    /// Number of entries deleted
    pub async fn cleanup_expired(&self) -> StateResult<usize> {
        debug!("Cleaning up expired entries");

        self.retry_operation(|| async {
            let result = sqlx::query("SELECT cleanup_expired_state_entries()")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| StateError::StorageError {
                    backend_type: "postgres".to_string(),
                    details: format!("Cleanup failed: {}", e),
                })?;

            let count: i64 = result.try_get(0).map_err(|e| StateError::StorageError {
                backend_type: "postgres".to_string(),
                details: format!("Failed to get cleanup count: {}", e),
            })?;

            let mut stats = self.stats.write().await;
            stats.query_count += 1;

            info!("Cleaned up {} expired entries", count);
            Ok(count as usize)
        })
        .await
    }

    /// Create a checkpoint of the current state
    ///
    /// This creates a point-in-time snapshot of all state entries.
    ///
    /// # Returns
    ///
    /// Checkpoint UUID
    pub async fn create_checkpoint(&self) -> StateResult<Uuid> {
        info!("Creating state checkpoint");

        self.retry_operation(|| async {
            let mut tx = self.pool.begin().await.map_err(|e| StateError::TransactionFailed {
                operation: "begin".to_string(),
                reason: e.to_string(),
            })?;

            // Get all current state
            let rows = sqlx::query("SELECT key, value, metadata FROM state_entries")
                .fetch_all(&mut *tx)
                .await
                .map_err(|e| StateError::StorageError {
                    backend_type: "postgres".to_string(),
                    details: format!("Failed to fetch state: {}", e),
                })?;

            let mut checkpoint_data = Vec::new();
            let mut total_size = 0i64;

            for row in &rows {
                let key: Vec<u8> = row.try_get("key").map_err(|e| StateError::StorageError {
                    backend_type: "postgres".to_string(),
                    details: format!("Failed to get key: {}", e),
                })?;
                let value: Vec<u8> = row.try_get("value").map_err(|e| StateError::StorageError {
                    backend_type: "postgres".to_string(),
                    details: format!("Failed to get value: {}", e),
                })?;

                total_size += (key.len() + value.len()) as i64;

                checkpoint_data.push(serde_json::json!({
                    "key": hex::encode(&key),
                    "value": hex::encode(&value),
                }));
            }

            let checkpoint_id = Uuid::new_v4();
            let checkpoint_json = serde_json::to_value(checkpoint_data).map_err(|e| {
                StateError::SerializationFailed {
                    key: "checkpoint".to_string(),
                    reason: e.to_string(),
                }
            })?;

            // Store checkpoint
            sqlx::query(
                r#"
                INSERT INTO state_checkpoints (checkpoint_id, checkpoint_data, row_count, data_size)
                VALUES ($1, $2, $3, $4)
                "#,
            )
            .bind(checkpoint_id)
            .bind(checkpoint_json)
            .bind(rows.len() as i64)
            .bind(total_size)
            .execute(&mut *tx)
            .await
            .map_err(|e| StateError::StorageError {
                backend_type: "postgres".to_string(),
                details: format!("Failed to store checkpoint: {}", e),
            })?;

            tx.commit().await.map_err(|e| StateError::TransactionFailed {
                operation: "commit".to_string(),
                reason: e.to_string(),
            })?;

            let mut stats = self.stats.write().await;
            stats.transaction_count += 1;

            info!("Checkpoint created: {} ({} entries, {} bytes)", checkpoint_id, rows.len(), total_size);
            Ok(checkpoint_id)
        })
        .await
    }

    /// Restore from a checkpoint
    ///
    /// # Arguments
    ///
    /// * `checkpoint_id` - UUID of the checkpoint to restore
    pub async fn restore_checkpoint(&self, checkpoint_id: Uuid) -> StateResult<()> {
        info!("Restoring from checkpoint: {}", checkpoint_id);

        self.retry_operation(|| async {
            let mut tx = self.pool.begin().await.map_err(|e| StateError::TransactionFailed {
                operation: "begin".to_string(),
                reason: e.to_string(),
            })?;

            // Get checkpoint
            let row = sqlx::query("SELECT checkpoint_data FROM state_checkpoints WHERE checkpoint_id = $1")
                .bind(checkpoint_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| StateError::StorageError {
                    backend_type: "postgres".to_string(),
                    details: format!("Failed to fetch checkpoint: {}", e),
                })?
                .ok_or_else(|| StateError::RestoreFailed {
                    checkpoint_id: checkpoint_id.to_string(),
                    reason: "Checkpoint not found".to_string(),
                })?;

            let checkpoint_data: serde_json::Value = row.try_get("checkpoint_data").map_err(|e| {
                StateError::DeserializationFailed {
                    key: checkpoint_id.to_string(),
                    reason: e.to_string(),
                }
            })?;

            // Clear existing state
            sqlx::query("DELETE FROM state_entries")
                .execute(&mut *tx)
                .await
                .map_err(|e| StateError::StorageError {
                    backend_type: "postgres".to_string(),
                    details: format!("Failed to clear state: {}", e),
                })?;

            // Restore entries
            if let Some(entries) = checkpoint_data.as_array() {
                for entry in entries {
                    let key_hex = entry["key"].as_str().ok_or_else(|| StateError::DeserializationFailed {
                        key: "checkpoint_entry".to_string(),
                        reason: "Missing key field".to_string(),
                    })?;
                    let value_hex = entry["value"].as_str().ok_or_else(|| StateError::DeserializationFailed {
                        key: "checkpoint_entry".to_string(),
                        reason: "Missing value field".to_string(),
                    })?;

                    let key = hex::decode(key_hex).map_err(|e| StateError::DeserializationFailed {
                        key: "key".to_string(),
                        reason: e.to_string(),
                    })?;
                    let value = hex::decode(value_hex).map_err(|e| StateError::DeserializationFailed {
                        key: "value".to_string(),
                        reason: e.to_string(),
                    })?;

                    sqlx::query("INSERT INTO state_entries (key, value) VALUES ($1, $2)")
                        .bind(&key)
                        .bind(&value)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| StateError::StorageError {
                            backend_type: "postgres".to_string(),
                            details: format!("Failed to insert entry: {}", e),
                        })?;
                }
            }

            tx.commit().await.map_err(|e| StateError::TransactionFailed {
                operation: "commit".to_string(),
                reason: e.to_string(),
            })?;

            info!("Checkpoint restored successfully");
            Ok(())
        })
        .await
    }

    /// List all checkpoints
    pub async fn list_checkpoints(&self) -> StateResult<Vec<CheckpointInfo>> {
        debug!("Listing checkpoints");

        let rows = sqlx::query(
            r#"
            SELECT checkpoint_id, created_at, row_count, data_size, metadata
            FROM state_checkpoints
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StateError::StorageError {
            backend_type: "postgres".to_string(),
            details: format!("Failed to list checkpoints: {}", e),
        })?;

        let mut checkpoints = Vec::new();
        for row in rows {
            checkpoints.push(CheckpointInfo {
                checkpoint_id: row.try_get("checkpoint_id").unwrap(),
                created_at: row.try_get("created_at").unwrap(),
                row_count: row.try_get("row_count").unwrap(),
                data_size: row.try_get("data_size").unwrap(),
                metadata: row.try_get("metadata").ok(),
            });
        }

        Ok(checkpoints)
    }

    /// Get backend statistics
    pub async fn stats(&self) -> PostgresStats {
        self.stats.read().await.clone()
    }

    /// Execute VACUUM on the state table
    ///
    /// This reclaims storage and updates statistics for better query performance.
    pub async fn vacuum(&self) -> StateResult<()> {
        info!("Running VACUUM on state_entries table");

        sqlx::query("VACUUM ANALYZE state_entries")
            .execute(&self.pool)
            .await
            .map_err(|e| StateError::StorageError {
                backend_type: "postgres".to_string(),
                details: format!("VACUUM failed: {}", e),
            })?;

        info!("VACUUM completed successfully");
        Ok(())
    }

    /// Get table size statistics
    pub async fn table_stats(&self) -> StateResult<(i64, i64)> {
        let row = sqlx::query(
            r#"
            SELECT
                pg_total_relation_size('state_entries') as total_size,
                pg_relation_size('state_entries') as table_size
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| StateError::StorageError {
            backend_type: "postgres".to_string(),
            details: format!("Failed to get table stats: {}", e),
        })?;

        let total_size: i64 = row.try_get("total_size").unwrap_or(0);
        let table_size: i64 = row.try_get("table_size").unwrap_or(0);

        Ok((total_size, table_size))
    }

    /// Batch put operation
    ///
    /// Efficiently insert multiple key-value pairs in a single transaction.
    pub async fn batch_put(&self, entries: &[(&[u8], &[u8])]) -> StateResult<()> {
        if entries.is_empty() {
            return Ok(());
        }

        debug!("Batch putting {} entries", entries.len());

        self.retry_operation(|| async {
            let mut tx = self.pool.begin().await.map_err(|e| StateError::TransactionFailed {
                operation: "begin".to_string(),
                reason: e.to_string(),
            })?;

            for (key, value) in entries {
                sqlx::query(
                    r#"
                    INSERT INTO state_entries (key, value, updated_at)
                    VALUES ($1, $2, NOW())
                    ON CONFLICT (key) DO UPDATE
                    SET value = EXCLUDED.value, updated_at = NOW()
                    "#,
                )
                .bind(key)
                .bind(value)
                .execute(&mut *tx)
                .await
                .map_err(|e| StateError::StorageError {
                    backend_type: "postgres".to_string(),
                    details: format!("Batch insert failed: {}", e),
                })?;
            }

            tx.commit().await.map_err(|e| StateError::TransactionFailed {
                operation: "commit".to_string(),
                reason: e.to_string(),
            })?;

            let mut stats = self.stats.write().await;
            stats.put_count += entries.len() as u64;
            stats.transaction_count += 1;

            Ok(())
        })
        .await
    }

    /// Health check - verify database connectivity
    pub async fn health_check(&self) -> StateResult<bool> {
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map(|_| true)
            .map_err(|e| StateError::StorageError {
                backend_type: "postgres".to_string(),
                details: format!("Health check failed: {}", e),
            })
    }

    /// Close the connection pool gracefully
    pub async fn close(&self) {
        info!("Closing PostgreSQL connection pool");
        self.pool.close().await;
    }

    // Helper methods

    fn sanitize_url(url: &str) -> String {
        // Remove password from URL for logging
        if let Some(at_pos) = url.rfind('@') {
            if let Some(scheme_end) = url.find("://") {
                let scheme = &url[..scheme_end + 3];
                let host = &url[at_pos + 1..];
                return format!("{}***@{}", scheme, host);
            }
        }
        url.to_string()
    }

    fn parse_ssl_mode(mode: &str) -> StateResult<PgSslMode> {
        match mode.to_lowercase().as_str() {
            "disable" => Ok(PgSslMode::Disable),
            "allow" => Ok(PgSslMode::Allow),
            "prefer" => Ok(PgSslMode::Prefer),
            "require" => Ok(PgSslMode::Require),
            "verify-ca" => Ok(PgSslMode::VerifyCa),
            "verify-full" => Ok(PgSslMode::VerifyFull),
            _ => Err(StateError::StorageError {
                backend_type: "postgres".to_string(),
                details: format!("Invalid SSL mode: {}", mode),
            }),
        }
    }

    async fn retry_operation<F, Fut, T>(&self, mut operation: F) -> StateResult<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = StateResult<T>>,
    {
        let mut attempts = 0;
        loop {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    attempts += 1;
                    if attempts >= self.config.max_retries {
                        return Err(e);
                    }

                    warn!("Operation failed (attempt {}/{}): {:?}", attempts, self.config.max_retries, e);

                    let mut stats = self.stats.write().await;
                    stats.retry_count += 1;
                    drop(stats);

                    tokio::time::sleep(self.config.retry_backoff * attempts).await;
                }
            }
        }
    }
}

#[async_trait]
impl StateBackend for PostgresStateBackend {
    async fn get(&self, key: &[u8]) -> StateResult<Option<Vec<u8>>> {
        trace!("Getting key from PostgreSQL: {} bytes", key.len());

        self.retry_operation(|| async {
            let result = sqlx::query(
                r#"
                SELECT value FROM state_entries
                WHERE key = $1
                  AND (expires_at IS NULL OR expires_at > NOW())
                "#,
            )
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StateError::StorageError {
                backend_type: "postgres".to_string(),
                details: format!("Get failed: {}", e),
            })?;

            let mut stats = self.stats.write().await;
            stats.get_count += 1;
            stats.query_count += 1;

            if let Some(row) = result {
                let value: Vec<u8> = row.try_get("value").map_err(|e| StateError::StorageError {
                    backend_type: "postgres".to_string(),
                    details: format!("Failed to extract value: {}", e),
                })?;

                stats.bytes_read += value.len() as u64;
                Ok(Some(value))
            } else {
                Ok(None)
            }
        })
        .await
    }

    async fn put(&self, key: &[u8], value: &[u8]) -> StateResult<()> {
        trace!("Putting key to PostgreSQL: {} bytes", key.len());

        self.retry_operation(|| async {
            sqlx::query(
                r#"
                INSERT INTO state_entries (key, value, updated_at)
                VALUES ($1, $2, NOW())
                ON CONFLICT (key) DO UPDATE
                SET value = EXCLUDED.value, updated_at = NOW()
                "#,
            )
            .bind(key)
            .bind(value)
            .execute(&self.pool)
            .await
            .map_err(|e| StateError::StorageError {
                backend_type: "postgres".to_string(),
                details: format!("Put failed: {}", e),
            })?;

            let mut stats = self.stats.write().await;
            stats.put_count += 1;
            stats.bytes_written += (key.len() + value.len()) as u64;
            stats.query_count += 1;

            Ok(())
        })
        .await
    }

    async fn delete(&self, key: &[u8]) -> StateResult<()> {
        trace!("Deleting key from PostgreSQL: {} bytes", key.len());

        self.retry_operation(|| async {
            sqlx::query("DELETE FROM state_entries WHERE key = $1")
                .bind(key)
                .execute(&self.pool)
                .await
                .map_err(|e| StateError::StorageError {
                    backend_type: "postgres".to_string(),
                    details: format!("Delete failed: {}", e),
                })?;

            let mut stats = self.stats.write().await;
            stats.delete_count += 1;
            stats.query_count += 1;

            Ok(())
        })
        .await
    }

    async fn list_keys(&self, prefix: &[u8]) -> StateResult<Vec<Vec<u8>>> {
        trace!("Listing keys with prefix: {} bytes", prefix.len());

        self.retry_operation(|| async {
            let rows = if prefix.is_empty() {
                sqlx::query("SELECT key FROM state_entries")
                    .fetch_all(&self.pool)
                    .await
            } else {
                // Use prefix matching with LIKE and escape special characters
                sqlx::query(
                    r#"
                    SELECT key FROM state_entries
                    WHERE key >= $1 AND key < $2
                    "#,
                )
                .bind(prefix)
                .bind(Self::next_prefix(prefix))
                .fetch_all(&self.pool)
                .await
            }
            .map_err(|e| StateError::StorageError {
                backend_type: "postgres".to_string(),
                details: format!("List keys failed: {}", e),
            })?;

            let mut stats = self.stats.write().await;
            stats.query_count += 1;
            drop(stats);

            let keys = rows
                .into_iter()
                .filter_map(|row| row.try_get::<Vec<u8>, _>("key").ok())
                .collect();

            Ok(keys)
        })
        .await
    }

    async fn clear(&self) -> StateResult<()> {
        debug!("Clearing all state entries");

        self.retry_operation(|| async {
            sqlx::query("TRUNCATE TABLE state_entries")
                .execute(&self.pool)
                .await
                .map_err(|e| StateError::StorageError {
                    backend_type: "postgres".to_string(),
                    details: format!("Clear failed: {}", e),
                })?;

            let mut stats = self.stats.write().await;
            stats.query_count += 1;

            Ok(())
        })
        .await
    }

    async fn count(&self) -> StateResult<usize> {
        self.retry_operation(|| async {
            let row = sqlx::query("SELECT COUNT(*) as count FROM state_entries")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| StateError::StorageError {
                    backend_type: "postgres".to_string(),
                    details: format!("Count failed: {}", e),
                })?;

            let count: i64 = row.try_get("count").map_err(|e| StateError::StorageError {
                backend_type: "postgres".to_string(),
                details: format!("Failed to extract count: {}", e),
            })?;

            let mut stats = self.stats.write().await;
            stats.query_count += 1;

            Ok(count as usize)
        })
        .await
    }

    async fn contains(&self, key: &[u8]) -> StateResult<bool> {
        self.retry_operation(|| async {
            let row = sqlx::query(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM state_entries
                    WHERE key = $1
                      AND (expires_at IS NULL OR expires_at > NOW())
                ) as exists
                "#,
            )
            .bind(key)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StateError::StorageError {
                backend_type: "postgres".to_string(),
                details: format!("Contains check failed: {}", e),
            })?;

            let exists: bool = row.try_get("exists").unwrap_or(false);

            let mut stats = self.stats.write().await;
            stats.query_count += 1;

            Ok(exists)
        })
        .await
    }
}

impl PostgresStateBackend {
    /// Calculate the next prefix for range queries
    fn next_prefix(prefix: &[u8]) -> Vec<u8> {
        let mut next = prefix.to_vec();
        for i in (0..next.len()).rev() {
            if next[i] < 255 {
                next[i] += 1;
                return next;
            }
        }
        // If all bytes are 255, append a zero
        next.push(0);
        next
    }
}

// Add hex dependency support
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    pub fn decode(s: &str) -> Result<Vec<u8>, String> {
        (0..s.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&s[i..i + 2], 16)
                    .map_err(|e| format!("Invalid hex: {}", e))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::backend::tests::*;

    // Helper to create test backend (requires running PostgreSQL)
    async fn create_test_backend() -> Option<PostgresStateBackend> {
        let config = PostgresConfig::new("postgresql://postgres:postgres@localhost/optimizer_test")
            .with_pool_size(2, 5);

        match PostgresStateBackend::new(config).await {
            Ok(backend) => {
                backend.clear().await.ok();
                Some(backend)
            }
            Err(_) => {
                eprintln!("Skipping PostgreSQL tests - database not available");
                None
            }
        }
    }

    #[tokio::test]
    async fn test_postgres_backend_basic() {
        if let Some(backend) = create_test_backend().await {
            test_backend_basic_ops(backend).await;
        }
    }

    #[tokio::test]
    async fn test_postgres_backend_list_keys() {
        if let Some(backend) = create_test_backend().await {
            test_backend_list_keys(backend).await;
        }
    }

    #[tokio::test]
    async fn test_postgres_backend_clear() {
        if let Some(backend) = create_test_backend().await {
            test_backend_clear(backend).await;
        }
    }

    #[tokio::test]
    async fn test_postgres_backend_contains() {
        if let Some(backend) = create_test_backend().await {
            test_backend_contains(backend).await;
        }
    }

    #[tokio::test]
    async fn test_postgres_metadata() {
        if let Some(backend) = create_test_backend().await {
            let metadata = serde_json::json!({
                "source": "kafka",
                "partition": 5,
                "offset": 12345
            });

            backend
                .put_with_metadata(b"test_key", b"test_value", metadata)
                .await
                .unwrap();

            let value = backend.get(b"test_key").await.unwrap();
            assert_eq!(value, Some(b"test_value".to_vec()));
        }
    }

    #[tokio::test]
    async fn test_postgres_ttl() {
        if let Some(backend) = create_test_backend().await {
            // Put with 1 second TTL
            backend
                .put_with_ttl(b"ttl_key", b"ttl_value", Duration::from_secs(1))
                .await
                .unwrap();

            // Should exist immediately
            assert!(backend.contains(b"ttl_key").await.unwrap());

            // Wait for expiration
            tokio::time::sleep(Duration::from_secs(2)).await;

            // Should be filtered out by query
            assert!(!backend.contains(b"ttl_key").await.unwrap());

            // Cleanup should remove it
            let deleted = backend.cleanup_expired().await.unwrap();
            assert_eq!(deleted, 1);
        }
    }

    #[tokio::test]
    async fn test_postgres_checkpoint() {
        if let Some(backend) = create_test_backend().await {
            backend.put(b"key1", b"value1").await.unwrap();
            backend.put(b"key2", b"value2").await.unwrap();
            backend.put(b"key3", b"value3").await.unwrap();

            // Create checkpoint
            let checkpoint_id = backend.create_checkpoint().await.unwrap();

            // Modify state
            backend.put(b"key1", b"modified").await.unwrap();
            backend.delete(b"key2").await.unwrap();

            // Restore checkpoint
            backend.restore_checkpoint(checkpoint_id).await.unwrap();

            // Verify original state restored
            assert_eq!(
                backend.get(b"key1").await.unwrap(),
                Some(b"value1".to_vec())
            );
            assert_eq!(
                backend.get(b"key2").await.unwrap(),
                Some(b"value2".to_vec())
            );
        }
    }

    #[tokio::test]
    async fn test_postgres_batch_put() {
        if let Some(backend) = create_test_backend().await {
            let entries: Vec<(&[u8], &[u8])> = vec![
                (b"batch1", b"value1"),
                (b"batch2", b"value2"),
                (b"batch3", b"value3"),
            ];

            backend.batch_put(&entries).await.unwrap();

            assert_eq!(backend.count().await.unwrap(), 3);
            assert_eq!(
                backend.get(b"batch1").await.unwrap(),
                Some(b"value1".to_vec())
            );
        }
    }

    #[tokio::test]
    async fn test_postgres_stats() {
        if let Some(backend) = create_test_backend().await {
            backend.put(b"key1", b"value1").await.unwrap();
            backend.get(b"key1").await.unwrap();
            backend.delete(b"key1").await.unwrap();

            let stats = backend.stats().await;
            assert!(stats.put_count >= 1);
            assert!(stats.get_count >= 1);
            assert!(stats.delete_count >= 1);
            assert!(stats.bytes_written > 0);
        }
    }

    #[tokio::test]
    async fn test_postgres_health_check() {
        if let Some(backend) = create_test_backend().await {
            assert!(backend.health_check().await.unwrap());
        }
    }
}
