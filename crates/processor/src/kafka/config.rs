//! Kafka configuration structures for sources and sinks.
//!
//! This module provides comprehensive configuration types for Kafka consumers and producers,
//! with support for all major rdkafka settings including security, performance tuning,
//! and reliability options.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Kafka source (consumer) configuration.
///
/// This structure provides all necessary settings for consuming from Kafka topics,
/// including consumer group management, offset handling, and performance tuning.
///
/// # Examples
///
/// ```rust
/// use processor::kafka::config::{KafkaSourceConfig, SecurityProtocol};
/// use std::time::Duration;
///
/// let config = KafkaSourceConfig::builder()
///     .bootstrap_servers(vec!["localhost:9092".to_string()])
///     .group_id("my-consumer-group".to_string())
///     .topics(vec!["events".to_string(), "metrics".to_string()])
///     .auto_offset_reset("earliest".to_string())
///     .enable_auto_commit(false)
///     .max_poll_records(500)
///     .session_timeout(Duration::from_secs(30))
///     .build()
///     .expect("valid configuration");
///
/// assert!(config.validate().is_ok());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaSourceConfig {
    // Connection settings
    /// Kafka broker addresses (host:port).
    pub bootstrap_servers: Vec<String>,

    /// Consumer group ID for coordinated consumption.
    pub group_id: String,

    /// Topics to subscribe to.
    pub topics: Vec<String>,

    // Consumer settings
    /// What to do when there is no initial offset: "earliest", "latest", "none".
    #[serde(default = "default_auto_offset_reset")]
    pub auto_offset_reset: String,

    /// Enable automatic offset commits.
    #[serde(default = "default_enable_auto_commit")]
    pub enable_auto_commit: bool,

    /// Frequency of auto-commits if enabled.
    #[serde(
        default = "default_auto_commit_interval",
        with = "duration_millis"
    )]
    pub auto_commit_interval: Duration,

    /// Maximum number of records to poll in a single batch.
    #[serde(default = "default_max_poll_records")]
    pub max_poll_records: usize,

    /// Maximum time to wait for poll batch to fill.
    #[serde(default = "default_fetch_wait_max", with = "duration_millis")]
    pub fetch_wait_max: Duration,

    /// Minimum bytes to fetch per request.
    #[serde(default = "default_fetch_min_bytes")]
    pub fetch_min_bytes: usize,

    /// Maximum bytes to fetch per partition.
    #[serde(default = "default_max_partition_fetch_bytes")]
    pub max_partition_fetch_bytes: usize,

    // Session and heartbeat
    /// Session timeout for consumer group membership.
    #[serde(default = "default_session_timeout", with = "duration_millis")]
    pub session_timeout: Duration,

    /// Heartbeat interval to coordinator.
    #[serde(default = "default_heartbeat_interval", with = "duration_millis")]
    pub heartbeat_interval: Duration,

    // Security settings
    /// Security protocol to use.
    #[serde(default)]
    pub security_protocol: SecurityProtocol,

    /// SASL mechanism if using SASL authentication.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sasl_mechanism: Option<SaslMechanism>,

    /// SASL username.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sasl_username: Option<String>,

    /// SASL password.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sasl_password: Option<String>,

    /// SSL configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssl_config: Option<SslConfig>,

    // Performance tuning
    /// Number of consumer threads for parallel consumption.
    #[serde(default = "default_num_consumer_threads")]
    pub num_consumer_threads: usize,

    /// Size of the internal queue for buffering messages.
    #[serde(default = "default_queued_max_messages")]
    pub queued_max_messages: usize,

    /// Socket receive buffer size.
    #[serde(default = "default_socket_receive_buffer_bytes")]
    pub socket_receive_buffer_bytes: usize,

    // Isolation level
    /// Transaction isolation level: "read_uncommitted" or "read_committed".
    #[serde(default = "default_isolation_level")]
    pub isolation_level: String,

    // Additional properties
    /// Additional rdkafka configuration properties.
    #[serde(default)]
    pub additional_properties: HashMap<String, String>,
}

/// Kafka sink (producer) configuration.
///
/// This structure provides all necessary settings for producing to Kafka topics,
/// including delivery guarantees, batching, compression, and performance tuning.
///
/// # Examples
///
/// ```rust
/// use processor::kafka::config::{KafkaSinkConfig, CompressionType};
/// use std::time::Duration;
///
/// let config = KafkaSinkConfig::builder()
///     .bootstrap_servers(vec!["localhost:9092".to_string()])
///     .topic("processed-events".to_string())
///     .compression(CompressionType::Lz4)
///     .batch_size(16384)
///     .linger(Duration::from_millis(10))
///     .acks("all".to_string())
///     .max_in_flight_requests(5)
///     .enable_idempotence(true)
///     .build()
///     .expect("valid configuration");
///
/// assert!(config.validate().is_ok());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaSinkConfig {
    // Connection settings
    /// Kafka broker addresses (host:port).
    pub bootstrap_servers: Vec<String>,

    /// Default topic to produce to.
    pub topic: String,

    // Producer settings
    /// Number of acknowledgments required: "0", "1", or "all".
    #[serde(default = "default_acks")]
    pub acks: String,

    /// Number of retries for transient errors.
    #[serde(default = "default_retries")]
    pub retries: u32,

    /// Maximum in-flight requests per connection.
    #[serde(default = "default_max_in_flight_requests")]
    pub max_in_flight_requests: usize,

    /// Enable idempotent producer.
    #[serde(default = "default_enable_idempotence")]
    pub enable_idempotence: bool,

    /// Request timeout.
    #[serde(default = "default_request_timeout", with = "duration_millis")]
    pub request_timeout: Duration,

    // Batching and buffering
    /// Batch size in bytes.
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Time to wait before sending a batch (even if not full).
    #[serde(default = "default_linger", with = "duration_millis")]
    pub linger: Duration,

    /// Total memory buffer for producer.
    #[serde(default = "default_buffer_memory")]
    pub buffer_memory: usize,

    /// Maximum time to block on send.
    #[serde(default = "default_max_block", with = "duration_millis")]
    pub max_block: Duration,

    // Compression
    /// Compression type for messages.
    #[serde(default)]
    pub compression: CompressionType,

    // Message size
    /// Maximum size of a single message.
    #[serde(default = "default_max_request_size")]
    pub max_request_size: usize,

    // Security settings
    /// Security protocol to use.
    #[serde(default)]
    pub security_protocol: SecurityProtocol,

    /// SASL mechanism if using SASL authentication.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sasl_mechanism: Option<SaslMechanism>,

    /// SASL username.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sasl_username: Option<String>,

    /// SASL password.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sasl_password: Option<String>,

    /// SSL configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssl_config: Option<SslConfig>,

    // Performance tuning
    /// Socket send buffer size.
    #[serde(default = "default_socket_send_buffer_bytes")]
    pub socket_send_buffer_bytes: usize,

    /// Size of the internal queue for buffering messages.
    #[serde(default = "default_queued_max_messages_producer")]
    pub queued_max_messages: usize,

    // Partitioning
    /// Custom partitioner class (rdkafka specific).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partitioner: Option<String>,

    // Additional properties
    /// Additional rdkafka configuration properties.
    #[serde(default)]
    pub additional_properties: HashMap<String, String>,
}

/// Security protocol for Kafka connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SecurityProtocol {
    /// Plaintext connection (no encryption).
    #[default]
    Plaintext,
    /// SSL/TLS encryption.
    Ssl,
    /// SASL authentication over plaintext.
    SaslPlaintext,
    /// SASL authentication over SSL/TLS.
    SaslSsl,
}

impl SecurityProtocol {
    /// Convert to rdkafka string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            SecurityProtocol::Plaintext => "PLAINTEXT",
            SecurityProtocol::Ssl => "SSL",
            SecurityProtocol::SaslPlaintext => "SASL_PLAINTEXT",
            SecurityProtocol::SaslSsl => "SASL_SSL",
        }
    }
}

/// SASL authentication mechanism.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaslMechanism {
    /// PLAIN authentication.
    Plain,
    /// SCRAM-SHA-256 authentication.
    #[serde(rename = "SCRAM-SHA-256")]
    ScramSha256,
    /// SCRAM-SHA-512 authentication.
    #[serde(rename = "SCRAM-SHA-512")]
    ScramSha512,
    /// GSSAPI (Kerberos) authentication.
    Gssapi,
    /// OAUTHBEARER authentication.
    OauthBearer,
}

impl SaslMechanism {
    /// Convert to rdkafka string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            SaslMechanism::Plain => "PLAIN",
            SaslMechanism::ScramSha256 => "SCRAM-SHA-256",
            SaslMechanism::ScramSha512 => "SCRAM-SHA-512",
            SaslMechanism::Gssapi => "GSSAPI",
            SaslMechanism::OauthBearer => "OAUTHBEARER",
        }
    }
}

/// SSL/TLS configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SslConfig {
    /// Path to CA certificate file.
    pub ca_location: Option<String>,

    /// Path to client certificate file.
    pub certificate_location: Option<String>,

    /// Path to client private key file.
    pub key_location: Option<String>,

    /// Password for private key.
    pub key_password: Option<String>,
}

/// Compression type for messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CompressionType {
    /// No compression.
    #[default]
    None,
    /// Gzip compression.
    Gzip,
    /// Snappy compression.
    Snappy,
    /// LZ4 compression.
    Lz4,
    /// Zstandard compression.
    Zstd,
}

impl CompressionType {
    /// Convert to rdkafka string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            CompressionType::None => "none",
            CompressionType::Gzip => "gzip",
            CompressionType::Snappy => "snappy",
            CompressionType::Lz4 => "lz4",
            CompressionType::Zstd => "zstd",
        }
    }
}

// Default value functions for KafkaSourceConfig
fn default_auto_offset_reset() -> String {
    "latest".to_string()
}

fn default_enable_auto_commit() -> bool {
    true
}

fn default_auto_commit_interval() -> Duration {
    Duration::from_secs(5)
}

fn default_max_poll_records() -> usize {
    500
}

fn default_fetch_wait_max() -> Duration {
    Duration::from_millis(500)
}

fn default_fetch_min_bytes() -> usize {
    1
}

fn default_max_partition_fetch_bytes() -> usize {
    1048576 // 1MB
}

fn default_session_timeout() -> Duration {
    Duration::from_secs(30)
}

fn default_heartbeat_interval() -> Duration {
    Duration::from_secs(3)
}

fn default_num_consumer_threads() -> usize {
    1
}

fn default_queued_max_messages() -> usize {
    100000
}

fn default_socket_receive_buffer_bytes() -> usize {
    65536 // 64KB
}

fn default_isolation_level() -> String {
    "read_uncommitted".to_string()
}

// Default value functions for KafkaSinkConfig
fn default_acks() -> String {
    "all".to_string()
}

fn default_retries() -> u32 {
    2147483647 // Max retries
}

fn default_max_in_flight_requests() -> usize {
    5
}

fn default_enable_idempotence() -> bool {
    true
}

fn default_request_timeout() -> Duration {
    Duration::from_secs(30)
}

fn default_batch_size() -> usize {
    16384 // 16KB
}

fn default_linger() -> Duration {
    Duration::from_millis(0)
}

fn default_buffer_memory() -> usize {
    33554432 // 32MB
}

fn default_max_block() -> Duration {
    Duration::from_secs(60)
}

fn default_max_request_size() -> usize {
    1048576 // 1MB
}

fn default_socket_send_buffer_bytes() -> usize {
    131072 // 128KB
}

fn default_queued_max_messages_producer() -> usize {
    100000
}

// Serde helper for Duration as milliseconds
mod duration_millis {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_millis() as u64)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(millis))
    }
}

impl KafkaSourceConfig {
    /// Create a new builder for KafkaSourceConfig.
    pub fn builder() -> KafkaSourceConfigBuilder {
        KafkaSourceConfigBuilder::default()
    }

    /// Validate the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Bootstrap servers are empty
    /// - Group ID is empty
    /// - Topics are empty
    /// - Invalid auto_offset_reset value
    /// - Invalid isolation_level value
    /// - SASL credentials missing when SASL is enabled
    pub fn validate(&self) -> Result<(), String> {
        if self.bootstrap_servers.is_empty() {
            return Err("bootstrap_servers cannot be empty".to_string());
        }

        if self.group_id.is_empty() {
            return Err("group_id cannot be empty".to_string());
        }

        if self.topics.is_empty() {
            return Err("topics cannot be empty".to_string());
        }

        if !["earliest", "latest", "none"].contains(&self.auto_offset_reset.as_str()) {
            return Err(format!(
                "invalid auto_offset_reset: {}. Must be 'earliest', 'latest', or 'none'",
                self.auto_offset_reset
            ));
        }

        if !["read_uncommitted", "read_committed"].contains(&self.isolation_level.as_str()) {
            return Err(format!(
                "invalid isolation_level: {}. Must be 'read_uncommitted' or 'read_committed'",
                self.isolation_level
            ));
        }

        // Validate SASL configuration
        if matches!(
            self.security_protocol,
            SecurityProtocol::SaslPlaintext | SecurityProtocol::SaslSsl
        ) {
            if self.sasl_mechanism.is_none() {
                return Err("sasl_mechanism required when using SASL security protocol".to_string());
            }
            if self.sasl_username.is_none() {
                return Err("sasl_username required when using SASL security protocol".to_string());
            }
            if self.sasl_password.is_none() {
                return Err("sasl_password required when using SASL security protocol".to_string());
            }
        }

        Ok(())
    }

    /// Convert to rdkafka ClientConfig properties.
    pub fn to_rdkafka_properties(&self) -> HashMap<String, String> {
        let mut props = HashMap::new();

        // Connection settings
        props.insert(
            "bootstrap.servers".to_string(),
            self.bootstrap_servers.join(","),
        );
        props.insert("group.id".to_string(), self.group_id.clone());

        // Consumer settings
        props.insert(
            "auto.offset.reset".to_string(),
            self.auto_offset_reset.clone(),
        );
        props.insert(
            "enable.auto.commit".to_string(),
            self.enable_auto_commit.to_string(),
        );
        props.insert(
            "auto.commit.interval.ms".to_string(),
            self.auto_commit_interval.as_millis().to_string(),
        );
        props.insert(
            "fetch.wait.max.ms".to_string(),
            self.fetch_wait_max.as_millis().to_string(),
        );
        props.insert(
            "fetch.min.bytes".to_string(),
            self.fetch_min_bytes.to_string(),
        );
        props.insert(
            "max.partition.fetch.bytes".to_string(),
            self.max_partition_fetch_bytes.to_string(),
        );

        // Session and heartbeat
        props.insert(
            "session.timeout.ms".to_string(),
            self.session_timeout.as_millis().to_string(),
        );
        props.insert(
            "heartbeat.interval.ms".to_string(),
            self.heartbeat_interval.as_millis().to_string(),
        );

        // Security
        props.insert(
            "security.protocol".to_string(),
            self.security_protocol.as_str().to_string(),
        );

        if let Some(mechanism) = &self.sasl_mechanism {
            props.insert("sasl.mechanism".to_string(), mechanism.as_str().to_string());
        }

        if let Some(username) = &self.sasl_username {
            props.insert("sasl.username".to_string(), username.clone());
        }

        if let Some(password) = &self.sasl_password {
            props.insert("sasl.password".to_string(), password.clone());
        }

        if let Some(ssl_config) = &self.ssl_config {
            if let Some(ca_location) = &ssl_config.ca_location {
                props.insert("ssl.ca.location".to_string(), ca_location.clone());
            }
            if let Some(cert_location) = &ssl_config.certificate_location {
                props.insert("ssl.certificate.location".to_string(), cert_location.clone());
            }
            if let Some(key_location) = &ssl_config.key_location {
                props.insert("ssl.key.location".to_string(), key_location.clone());
            }
            if let Some(key_password) = &ssl_config.key_password {
                props.insert("ssl.key.password".to_string(), key_password.clone());
            }
        }

        // Performance tuning
        props.insert(
            "queued.max.messages.kbytes".to_string(),
            (self.queued_max_messages / 1000).to_string(),
        );
        props.insert(
            "receive.message.max.bytes".to_string(),
            self.socket_receive_buffer_bytes.to_string(),
        );

        // Isolation level
        props.insert("isolation.level".to_string(), self.isolation_level.clone());

        // Additional properties (can override defaults)
        props.extend(self.additional_properties.clone());

        props
    }
}

impl KafkaSinkConfig {
    /// Create a new builder for KafkaSinkConfig.
    pub fn builder() -> KafkaSinkConfigBuilder {
        KafkaSinkConfigBuilder::default()
    }

    /// Validate the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Bootstrap servers are empty
    /// - Topic is empty
    /// - Invalid acks value
    /// - SASL credentials missing when SASL is enabled
    pub fn validate(&self) -> Result<(), String> {
        if self.bootstrap_servers.is_empty() {
            return Err("bootstrap_servers cannot be empty".to_string());
        }

        if self.topic.is_empty() {
            return Err("topic cannot be empty".to_string());
        }

        if !["0", "1", "all", "-1"].contains(&self.acks.as_str()) {
            return Err(format!(
                "invalid acks: {}. Must be '0', '1', or 'all'",
                self.acks
            ));
        }

        // Validate SASL configuration
        if matches!(
            self.security_protocol,
            SecurityProtocol::SaslPlaintext | SecurityProtocol::SaslSsl
        ) {
            if self.sasl_mechanism.is_none() {
                return Err("sasl_mechanism required when using SASL security protocol".to_string());
            }
            if self.sasl_username.is_none() {
                return Err("sasl_username required when using SASL security protocol".to_string());
            }
            if self.sasl_password.is_none() {
                return Err("sasl_password required when using SASL security protocol".to_string());
            }
        }

        Ok(())
    }

    /// Convert to rdkafka ClientConfig properties.
    pub fn to_rdkafka_properties(&self) -> HashMap<String, String> {
        let mut props = HashMap::new();

        // Connection settings
        props.insert(
            "bootstrap.servers".to_string(),
            self.bootstrap_servers.join(","),
        );

        // Producer settings
        props.insert("acks".to_string(), self.acks.clone());
        props.insert("retries".to_string(), self.retries.to_string());
        props.insert(
            "max.in.flight.requests.per.connection".to_string(),
            self.max_in_flight_requests.to_string(),
        );
        props.insert(
            "enable.idempotence".to_string(),
            self.enable_idempotence.to_string(),
        );
        props.insert(
            "request.timeout.ms".to_string(),
            self.request_timeout.as_millis().to_string(),
        );

        // Batching and buffering
        props.insert("batch.size".to_string(), self.batch_size.to_string());
        props.insert(
            "linger.ms".to_string(),
            self.linger.as_millis().to_string(),
        );
        props.insert("buffer.memory".to_string(), self.buffer_memory.to_string());
        props.insert(
            "max.block.ms".to_string(),
            self.max_block.as_millis().to_string(),
        );

        // Compression
        props.insert(
            "compression.type".to_string(),
            self.compression.as_str().to_string(),
        );

        // Message size
        props.insert(
            "max.request.size".to_string(),
            self.max_request_size.to_string(),
        );

        // Security
        props.insert(
            "security.protocol".to_string(),
            self.security_protocol.as_str().to_string(),
        );

        if let Some(mechanism) = &self.sasl_mechanism {
            props.insert("sasl.mechanism".to_string(), mechanism.as_str().to_string());
        }

        if let Some(username) = &self.sasl_username {
            props.insert("sasl.username".to_string(), username.clone());
        }

        if let Some(password) = &self.sasl_password {
            props.insert("sasl.password".to_string(), password.clone());
        }

        if let Some(ssl_config) = &self.ssl_config {
            if let Some(ca_location) = &ssl_config.ca_location {
                props.insert("ssl.ca.location".to_string(), ca_location.clone());
            }
            if let Some(cert_location) = &ssl_config.certificate_location {
                props.insert("ssl.certificate.location".to_string(), cert_location.clone());
            }
            if let Some(key_location) = &ssl_config.key_location {
                props.insert("ssl.key.location".to_string(), key_location.clone());
            }
            if let Some(key_password) = &ssl_config.key_password {
                props.insert("ssl.key.password".to_string(), key_password.clone());
            }
        }

        // Performance tuning
        props.insert(
            "send.buffer.bytes".to_string(),
            self.socket_send_buffer_bytes.to_string(),
        );
        props.insert(
            "queue.buffering.max.messages".to_string(),
            self.queued_max_messages.to_string(),
        );

        // Partitioner
        if let Some(partitioner) = &self.partitioner {
            props.insert("partitioner".to_string(), partitioner.clone());
        }

        // Additional properties (can override defaults)
        props.extend(self.additional_properties.clone());

        props
    }
}

/// Builder for KafkaSourceConfig.
#[derive(Default)]
pub struct KafkaSourceConfigBuilder {
    bootstrap_servers: Option<Vec<String>>,
    group_id: Option<String>,
    topics: Option<Vec<String>>,
    auto_offset_reset: Option<String>,
    enable_auto_commit: Option<bool>,
    auto_commit_interval: Option<Duration>,
    max_poll_records: Option<usize>,
    fetch_wait_max: Option<Duration>,
    fetch_min_bytes: Option<usize>,
    max_partition_fetch_bytes: Option<usize>,
    session_timeout: Option<Duration>,
    heartbeat_interval: Option<Duration>,
    security_protocol: Option<SecurityProtocol>,
    sasl_mechanism: Option<SaslMechanism>,
    sasl_username: Option<String>,
    sasl_password: Option<String>,
    ssl_config: Option<SslConfig>,
    num_consumer_threads: Option<usize>,
    queued_max_messages: Option<usize>,
    socket_receive_buffer_bytes: Option<usize>,
    isolation_level: Option<String>,
    additional_properties: Option<HashMap<String, String>>,
}

impl KafkaSourceConfigBuilder {
    pub fn bootstrap_servers(mut self, servers: Vec<String>) -> Self {
        self.bootstrap_servers = Some(servers);
        self
    }

    pub fn group_id(mut self, group_id: String) -> Self {
        self.group_id = Some(group_id);
        self
    }

    pub fn topics(mut self, topics: Vec<String>) -> Self {
        self.topics = Some(topics);
        self
    }

    pub fn auto_offset_reset(mut self, reset: String) -> Self {
        self.auto_offset_reset = Some(reset);
        self
    }

    pub fn enable_auto_commit(mut self, enable: bool) -> Self {
        self.enable_auto_commit = Some(enable);
        self
    }

    pub fn auto_commit_interval(mut self, interval: Duration) -> Self {
        self.auto_commit_interval = Some(interval);
        self
    }

    pub fn max_poll_records(mut self, records: usize) -> Self {
        self.max_poll_records = Some(records);
        self
    }

    pub fn fetch_wait_max(mut self, duration: Duration) -> Self {
        self.fetch_wait_max = Some(duration);
        self
    }

    pub fn fetch_min_bytes(mut self, bytes: usize) -> Self {
        self.fetch_min_bytes = Some(bytes);
        self
    }

    pub fn max_partition_fetch_bytes(mut self, bytes: usize) -> Self {
        self.max_partition_fetch_bytes = Some(bytes);
        self
    }

    pub fn session_timeout(mut self, timeout: Duration) -> Self {
        self.session_timeout = Some(timeout);
        self
    }

    pub fn heartbeat_interval(mut self, interval: Duration) -> Self {
        self.heartbeat_interval = Some(interval);
        self
    }

    pub fn security_protocol(mut self, protocol: SecurityProtocol) -> Self {
        self.security_protocol = Some(protocol);
        self
    }

    pub fn sasl_mechanism(mut self, mechanism: SaslMechanism) -> Self {
        self.sasl_mechanism = Some(mechanism);
        self
    }

    pub fn sasl_username(mut self, username: String) -> Self {
        self.sasl_username = Some(username);
        self
    }

    pub fn sasl_password(mut self, password: String) -> Self {
        self.sasl_password = Some(password);
        self
    }

    pub fn ssl_config(mut self, config: SslConfig) -> Self {
        self.ssl_config = Some(config);
        self
    }

    pub fn num_consumer_threads(mut self, threads: usize) -> Self {
        self.num_consumer_threads = Some(threads);
        self
    }

    pub fn queued_max_messages(mut self, messages: usize) -> Self {
        self.queued_max_messages = Some(messages);
        self
    }

    pub fn socket_receive_buffer_bytes(mut self, bytes: usize) -> Self {
        self.socket_receive_buffer_bytes = Some(bytes);
        self
    }

    pub fn isolation_level(mut self, level: String) -> Self {
        self.isolation_level = Some(level);
        self
    }

    pub fn additional_properties(mut self, properties: HashMap<String, String>) -> Self {
        self.additional_properties = Some(properties);
        self
    }

    pub fn build(self) -> Result<KafkaSourceConfig, String> {
        let config = KafkaSourceConfig {
            bootstrap_servers: self
                .bootstrap_servers
                .ok_or("bootstrap_servers is required")?,
            group_id: self.group_id.ok_or("group_id is required")?,
            topics: self.topics.ok_or("topics is required")?,
            auto_offset_reset: self.auto_offset_reset.unwrap_or_else(default_auto_offset_reset),
            enable_auto_commit: self
                .enable_auto_commit
                .unwrap_or_else(default_enable_auto_commit),
            auto_commit_interval: self
                .auto_commit_interval
                .unwrap_or_else(default_auto_commit_interval),
            max_poll_records: self.max_poll_records.unwrap_or_else(default_max_poll_records),
            fetch_wait_max: self.fetch_wait_max.unwrap_or_else(default_fetch_wait_max),
            fetch_min_bytes: self.fetch_min_bytes.unwrap_or_else(default_fetch_min_bytes),
            max_partition_fetch_bytes: self
                .max_partition_fetch_bytes
                .unwrap_or_else(default_max_partition_fetch_bytes),
            session_timeout: self.session_timeout.unwrap_or_else(default_session_timeout),
            heartbeat_interval: self
                .heartbeat_interval
                .unwrap_or_else(default_heartbeat_interval),
            security_protocol: self.security_protocol.unwrap_or_default(),
            sasl_mechanism: self.sasl_mechanism,
            sasl_username: self.sasl_username,
            sasl_password: self.sasl_password,
            ssl_config: self.ssl_config,
            num_consumer_threads: self
                .num_consumer_threads
                .unwrap_or_else(default_num_consumer_threads),
            queued_max_messages: self
                .queued_max_messages
                .unwrap_or_else(default_queued_max_messages),
            socket_receive_buffer_bytes: self
                .socket_receive_buffer_bytes
                .unwrap_or_else(default_socket_receive_buffer_bytes),
            isolation_level: self.isolation_level.unwrap_or_else(default_isolation_level),
            additional_properties: self.additional_properties.unwrap_or_default(),
        };

        config.validate()?;
        Ok(config)
    }
}

/// Builder for KafkaSinkConfig.
#[derive(Default)]
pub struct KafkaSinkConfigBuilder {
    bootstrap_servers: Option<Vec<String>>,
    topic: Option<String>,
    acks: Option<String>,
    retries: Option<u32>,
    max_in_flight_requests: Option<usize>,
    enable_idempotence: Option<bool>,
    request_timeout: Option<Duration>,
    batch_size: Option<usize>,
    linger: Option<Duration>,
    buffer_memory: Option<usize>,
    max_block: Option<Duration>,
    compression: Option<CompressionType>,
    max_request_size: Option<usize>,
    security_protocol: Option<SecurityProtocol>,
    sasl_mechanism: Option<SaslMechanism>,
    sasl_username: Option<String>,
    sasl_password: Option<String>,
    ssl_config: Option<SslConfig>,
    socket_send_buffer_bytes: Option<usize>,
    queued_max_messages: Option<usize>,
    partitioner: Option<String>,
    additional_properties: Option<HashMap<String, String>>,
}

impl KafkaSinkConfigBuilder {
    pub fn bootstrap_servers(mut self, servers: Vec<String>) -> Self {
        self.bootstrap_servers = Some(servers);
        self
    }

    pub fn topic(mut self, topic: String) -> Self {
        self.topic = Some(topic);
        self
    }

    pub fn acks(mut self, acks: String) -> Self {
        self.acks = Some(acks);
        self
    }

    pub fn retries(mut self, retries: u32) -> Self {
        self.retries = Some(retries);
        self
    }

    pub fn max_in_flight_requests(mut self, requests: usize) -> Self {
        self.max_in_flight_requests = Some(requests);
        self
    }

    pub fn enable_idempotence(mut self, enable: bool) -> Self {
        self.enable_idempotence = Some(enable);
        self
    }

    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = Some(timeout);
        self
    }

    pub fn batch_size(mut self, size: usize) -> Self {
        self.batch_size = Some(size);
        self
    }

    pub fn linger(mut self, duration: Duration) -> Self {
        self.linger = Some(duration);
        self
    }

    pub fn buffer_memory(mut self, memory: usize) -> Self {
        self.buffer_memory = Some(memory);
        self
    }

    pub fn max_block(mut self, duration: Duration) -> Self {
        self.max_block = Some(duration);
        self
    }

    pub fn compression(mut self, compression: CompressionType) -> Self {
        self.compression = Some(compression);
        self
    }

    pub fn max_request_size(mut self, size: usize) -> Self {
        self.max_request_size = Some(size);
        self
    }

    pub fn security_protocol(mut self, protocol: SecurityProtocol) -> Self {
        self.security_protocol = Some(protocol);
        self
    }

    pub fn sasl_mechanism(mut self, mechanism: SaslMechanism) -> Self {
        self.sasl_mechanism = Some(mechanism);
        self
    }

    pub fn sasl_username(mut self, username: String) -> Self {
        self.sasl_username = Some(username);
        self
    }

    pub fn sasl_password(mut self, password: String) -> Self {
        self.sasl_password = Some(password);
        self
    }

    pub fn ssl_config(mut self, config: SslConfig) -> Self {
        self.ssl_config = Some(config);
        self
    }

    pub fn socket_send_buffer_bytes(mut self, bytes: usize) -> Self {
        self.socket_send_buffer_bytes = Some(bytes);
        self
    }

    pub fn queued_max_messages(mut self, messages: usize) -> Self {
        self.queued_max_messages = Some(messages);
        self
    }

    pub fn partitioner(mut self, partitioner: String) -> Self {
        self.partitioner = Some(partitioner);
        self
    }

    pub fn additional_properties(mut self, properties: HashMap<String, String>) -> Self {
        self.additional_properties = Some(properties);
        self
    }

    pub fn build(self) -> Result<KafkaSinkConfig, String> {
        let config = KafkaSinkConfig {
            bootstrap_servers: self
                .bootstrap_servers
                .ok_or("bootstrap_servers is required")?,
            topic: self.topic.ok_or("topic is required")?,
            acks: self.acks.unwrap_or_else(default_acks),
            retries: self.retries.unwrap_or_else(default_retries),
            max_in_flight_requests: self
                .max_in_flight_requests
                .unwrap_or_else(default_max_in_flight_requests),
            enable_idempotence: self
                .enable_idempotence
                .unwrap_or_else(default_enable_idempotence),
            request_timeout: self.request_timeout.unwrap_or_else(default_request_timeout),
            batch_size: self.batch_size.unwrap_or_else(default_batch_size),
            linger: self.linger.unwrap_or_else(default_linger),
            buffer_memory: self.buffer_memory.unwrap_or_else(default_buffer_memory),
            max_block: self.max_block.unwrap_or_else(default_max_block),
            compression: self.compression.unwrap_or_default(),
            max_request_size: self.max_request_size.unwrap_or_else(default_max_request_size),
            security_protocol: self.security_protocol.unwrap_or_default(),
            sasl_mechanism: self.sasl_mechanism,
            sasl_username: self.sasl_username,
            sasl_password: self.sasl_password,
            ssl_config: self.ssl_config,
            socket_send_buffer_bytes: self
                .socket_send_buffer_bytes
                .unwrap_or_else(default_socket_send_buffer_bytes),
            queued_max_messages: self
                .queued_max_messages
                .unwrap_or_else(default_queued_max_messages_producer),
            partitioner: self.partitioner,
            additional_properties: self.additional_properties.unwrap_or_default(),
        };

        config.validate()?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kafka_source_config_validation() {
        let config = KafkaSourceConfig::builder()
            .bootstrap_servers(vec!["localhost:9092".to_string()])
            .group_id("test-group".to_string())
            .topics(vec!["test-topic".to_string()])
            .build();

        assert!(config.is_ok());
    }

    #[test]
    fn test_kafka_sink_config_validation() {
        let config = KafkaSinkConfig::builder()
            .bootstrap_servers(vec!["localhost:9092".to_string()])
            .topic("test-topic".to_string())
            .build();

        assert!(config.is_ok());
    }

    #[test]
    fn test_security_protocol_string_conversion() {
        assert_eq!(SecurityProtocol::Plaintext.as_str(), "PLAINTEXT");
        assert_eq!(SecurityProtocol::Ssl.as_str(), "SSL");
        assert_eq!(SecurityProtocol::SaslPlaintext.as_str(), "SASL_PLAINTEXT");
        assert_eq!(SecurityProtocol::SaslSsl.as_str(), "SASL_SSL");
    }

    #[test]
    fn test_compression_type_string_conversion() {
        assert_eq!(CompressionType::None.as_str(), "none");
        assert_eq!(CompressionType::Gzip.as_str(), "gzip");
        assert_eq!(CompressionType::Snappy.as_str(), "snappy");
        assert_eq!(CompressionType::Lz4.as_str(), "lz4");
        assert_eq!(CompressionType::Zstd.as_str(), "zstd");
    }

    #[test]
    fn test_sasl_mechanism_string_conversion() {
        assert_eq!(SaslMechanism::Plain.as_str(), "PLAIN");
        assert_eq!(SaslMechanism::ScramSha256.as_str(), "SCRAM-SHA-256");
        assert_eq!(SaslMechanism::ScramSha512.as_str(), "SCRAM-SHA-512");
    }
}
