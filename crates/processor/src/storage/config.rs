//! Storage configuration

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Storage manager configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// PostgreSQL configuration
    pub postgresql: Option<PostgreSQLConfig>,

    /// Redis configuration
    pub redis: Option<RedisConfig>,

    /// Sled configuration
    pub sled: Option<SledConfig>,

    /// Connection pool settings
    pub pool: PoolConfig,

    /// Retry settings
    pub retry: RetryConfig,

    /// Monitoring settings
    pub monitoring: MonitoringConfig,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            postgresql: None,
            redis: None,
            sled: None,
            pool: PoolConfig::default(),
            retry: RetryConfig::default(),
            monitoring: MonitoringConfig::default(),
        }
    }
}

/// PostgreSQL configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgreSQLConfig {
    /// Database host
    pub host: String,

    /// Database port
    pub port: u16,

    /// Database name
    pub database: String,

    /// Username
    pub username: String,

    /// Password
    pub password: String,

    /// SSL mode
    pub ssl_mode: SSLMode,

    /// Connection timeout
    pub connect_timeout: Duration,

    /// Query timeout
    pub query_timeout: Duration,

    /// Enable migrations
    pub enable_migrations: bool,

    /// Migrations directory
    pub migrations_dir: Option<PathBuf>,

    /// Schema name
    pub schema: String,

    /// Application name
    pub application_name: String,
}

impl Default for PostgreSQLConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 5432,
            database: "llm_optimizer".to_string(),
            username: "postgres".to_string(),
            password: String::new(),
            ssl_mode: SSLMode::Prefer,
            connect_timeout: Duration::from_secs(10),
            query_timeout: Duration::from_secs(30),
            enable_migrations: true,
            migrations_dir: None,
            schema: "public".to_string(),
            application_name: "llm-auto-optimizer".to_string(),
        }
    }
}

impl PostgreSQLConfig {
    /// Build connection URL
    pub fn connection_url(&self) -> String {
        format!(
            "postgresql://{}:{}@{}:{}/{}?application_name={}",
            self.username, self.password, self.host, self.port, self.database, self.application_name
        )
    }
}

/// SSL mode for PostgreSQL
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SSLMode {
    Disable,
    Prefer,
    Require,
    VerifyCA,
    VerifyFull,
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis host
    pub host: String,

    /// Redis port
    pub port: u16,

    /// Database number (0-15)
    pub database: u8,

    /// Password (if required)
    pub password: Option<String>,

    /// Username (Redis 6+)
    pub username: Option<String>,

    /// Connection timeout
    pub connect_timeout: Duration,

    /// Command timeout
    pub command_timeout: Duration,

    /// Enable cluster mode
    pub cluster_mode: bool,

    /// Cluster nodes (if cluster_mode = true)
    pub cluster_nodes: Vec<String>,

    /// Key prefix for namespacing
    pub key_prefix: String,

    /// Default TTL for entries (seconds)
    pub default_ttl: Option<i64>,

    /// Enable pub/sub
    pub enable_pubsub: bool,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 6379,
            database: 0,
            password: None,
            username: None,
            connect_timeout: Duration::from_secs(5),
            command_timeout: Duration::from_secs(10),
            cluster_mode: false,
            cluster_nodes: Vec::new(),
            key_prefix: "llm_optimizer:".to_string(),
            default_ttl: None,
            enable_pubsub: false,
        }
    }
}

impl RedisConfig {
    /// Build connection URL
    pub fn connection_url(&self) -> String {
        let auth = if let Some(ref username) = self.username {
            if let Some(ref password) = self.password {
                format!("{}:{}@", username, password)
            } else {
                format!("{}@", username)
            }
        } else if let Some(ref password) = self.password {
            format!(":{}@", password)
        } else {
            String::new()
        };

        format!(
            "redis://{}{}:{}/{}",
            auth, self.host, self.port, self.database
        )
    }
}

/// Sled configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SledConfig {
    /// Database path
    pub path: PathBuf,

    /// Cache size in bytes
    pub cache_capacity_bytes: u64,

    /// Enable compression
    pub use_compression: bool,

    /// Flush interval
    pub flush_every_ms: Option<u64>,

    /// Mode (high throughput or low space)
    pub mode: SledMode,

    /// Enable temporary mode (no durability)
    pub temporary: bool,

    /// Create directory if missing
    pub create_dir: bool,
}

impl Default for SledConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("./data/sled"),
            cache_capacity_bytes: 1024 * 1024 * 1024, // 1 GB
            use_compression: true,
            flush_every_ms: Some(1000),
            mode: SledMode::HighThroughput,
            temporary: false,
            create_dir: true,
        }
    }
}

/// Sled operation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SledMode {
    /// Optimize for throughput
    HighThroughput,
    /// Optimize for low space usage
    LowSpace,
}

/// Connection pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    /// Minimum connections
    pub min_connections: u32,

    /// Maximum connections
    pub max_connections: u32,

    /// Connection idle timeout
    pub idle_timeout: Duration,

    /// Maximum connection lifetime
    pub max_lifetime: Duration,

    /// Connection acquire timeout
    pub acquire_timeout: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            min_connections: 2,
            max_connections: 10,
            idle_timeout: Duration::from_secs(600), // 10 minutes
            max_lifetime: Duration::from_secs(1800), // 30 minutes
            acquire_timeout: Duration::from_secs(30),
        }
    }
}

/// Retry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Enable retries
    pub enabled: bool,

    /// Maximum retry attempts
    pub max_attempts: usize,

    /// Initial retry delay
    pub initial_delay: Duration,

    /// Maximum retry delay
    pub max_delay: Duration,

    /// Exponential backoff multiplier
    pub backoff_multiplier: f64,

    /// Jitter factor (0.0-1.0)
    pub jitter_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter_factor: 0.2,
        }
    }
}

impl RetryConfig {
    /// Calculate delay for retry attempt
    pub fn delay_for_attempt(&self, attempt: usize) -> Duration {
        if attempt == 0 || !self.enabled {
            return Duration::from_secs(0);
        }

        // Exponential backoff
        let delay_ms = self.initial_delay.as_millis() as f64
            * self.backoff_multiplier.powi((attempt - 1) as i32);

        let delay_ms = delay_ms.min(self.max_delay.as_millis() as f64);

        // Add jitter
        let jitter = delay_ms * self.jitter_factor * (rand::random::<f64>() - 0.5);
        let final_delay_ms = (delay_ms + jitter).max(0.0);

        Duration::from_millis(final_delay_ms as u64)
    }
}

/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable metrics collection
    pub enable_metrics: bool,

    /// Metrics collection interval
    pub metrics_interval: Duration,

    /// Enable health checks
    pub enable_health_checks: bool,

    /// Health check interval
    pub health_check_interval: Duration,

    /// Enable query logging
    pub enable_query_logging: bool,

    /// Slow query threshold
    pub slow_query_threshold: Duration,

    /// Enable connection pool monitoring
    pub enable_pool_monitoring: bool,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enable_metrics: true,
            metrics_interval: Duration::from_secs(60),
            enable_health_checks: true,
            health_check_interval: Duration::from_secs(30),
            enable_query_logging: true,
            slow_query_threshold: Duration::from_millis(1000),
            enable_pool_monitoring: true,
        }
    }
}
