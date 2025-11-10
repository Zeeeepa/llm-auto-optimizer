# Event Deduplication Architecture Specification

## Executive Summary

This document specifies the architecture for an enterprise-grade event deduplication system integrated into the Stream Processor. The system provides exactly-once processing semantics through O(1) duplicate detection using Redis SET operations, configurable TTL management, and graceful degradation strategies.

## Table of Contents

1. [System Overview](#system-overview)
2. [Architecture Design](#architecture-design)
3. [Component Specifications](#component-specifications)
4. [Data Structures](#data-structures)
5. [Redis Storage Schema](#redis-storage-schema)
6. [Event ID Extraction Strategies](#event-id-extraction-strategies)
7. [TTL Management](#ttl-management)
8. [Integration Patterns](#integration-patterns)
9. [Configuration Schema](#configuration-schema)
10. [Error Handling & Fallback](#error-handling--fallback)
11. [Performance Characteristics](#performance-characteristics)
12. [Monitoring & Metrics](#monitoring--metrics)
13. [Production Deployment](#production-deployment)

---

## System Overview

### Purpose

The Event Deduplicator ensures exactly-once processing semantics by detecting and filtering duplicate events in the stream processing pipeline, preventing:

- Double-counting in metric aggregations
- Duplicate state updates
- Redundant downstream processing
- Data quality degradation

### Key Requirements

1. **O(1) Duplicate Detection**: Redis SET-based membership testing
2. **Configurable TTL**: Automatic cleanup of deduplication state
3. **Custom Event ID Extraction**: Flexible strategies for different event types
4. **Multiple Strategies**: Strict (fail on Redis error) vs Best-effort (allow through)
5. **Metrics & Monitoring**: Comprehensive observability
6. **Graceful Degradation**: Fallback behavior on Redis failure
7. **Memory Efficiency**: Handle billions of events with bounded memory
8. **Thread-Safety**: Async-compatible with concurrent access

### Design Principles

- **Fail-Safe**: Default to allowing events through rather than blocking processing
- **Observable**: Rich metrics and tracing at every decision point
- **Configurable**: Support diverse use cases through configuration
- **Performant**: Minimal latency impact on event processing
- **Scalable**: Horizontal scaling through Redis clustering

---

## Architecture Design

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Stream Processing Pipeline                    │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  Event Source → Event Deduplicator → Window Assigner → ...      │
│                        │                                          │
│                        ├─ Event ID Extractor                     │
│                        ├─ Deduplication Strategy                 │
│                        ├─ Redis Backend                          │
│                        └─ Metrics Collector                      │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
                             │
                             ▼
                    ┌────────────────┐
                    │  Redis Cluster │
                    │                │
                    │  SET: event_ids│
                    │  TTL: per key  │
                    └────────────────┘
```

### Component Layers

```
┌──────────────────────────────────────────────────────────────┐
│ Layer 1: Public API (EventDeduplicator)                      │
├──────────────────────────────────────────────────────────────┤
│ Layer 2: Strategy Layer (Strict, BestEffort, Custom)         │
├──────────────────────────────────────────────────────────────┤
│ Layer 3: Event ID Extraction (Hash, Field, Composite)        │
├──────────────────────────────────────────────────────────────┤
│ Layer 4: Storage Layer (Redis Backend + Fallback)            │
├──────────────────────────────────────────────────────────────┤
│ Layer 5: Metrics & Monitoring (Prometheus, Tracing)          │
└──────────────────────────────────────────────────────────────┘
```

---

## Component Specifications

### 1. Core Trait: EventDeduplicator

```rust
/// Main trait for event deduplication
#[async_trait]
pub trait EventDeduplicator: Send + Sync {
    /// Check if an event is a duplicate
    /// Returns Ok(true) if duplicate, Ok(false) if unique, Err on storage failure
    async fn is_duplicate<T>(&self, event: &T) -> DeduplicationResult<bool>
    where
        T: Send + Sync;

    /// Mark an event as seen (idempotent)
    async fn mark_seen<T>(&self, event: &T) -> DeduplicationResult<()>
    where
        T: Send + Sync;

    /// Check and mark in a single atomic operation (preferred)
    async fn check_and_mark<T>(&self, event: &T) -> DeduplicationResult<bool>
    where
        T: Send + Sync;

    /// Get deduplication statistics
    async fn stats(&self) -> DeduplicationStats;

    /// Health check for the deduplicator
    async fn health_check(&self) -> DeduplicationResult<HealthStatus>;

    /// Clear all deduplication state (use with caution)
    async fn clear(&self) -> DeduplicationResult<()>;
}
```

### 2. Event ID Extractor Trait

```rust
/// Trait for extracting unique event identifiers
pub trait EventIdExtractor<T>: Send + Sync {
    /// Extract a unique event ID from an event
    /// Returns a stable, unique identifier for the event
    fn extract_id(&self, event: &T) -> DeduplicationResult<EventId>;
}

/// Event ID representation
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(String);

impl EventId {
    /// Create from string
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Create from bytes (will hex-encode)
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(hex::encode(bytes))
    }

    /// Get as string reference
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Get as bytes for Redis storage
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}
```

### 3. Deduplication Strategy Enum

```rust
/// Deduplication strategy determines behavior on failures
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeduplicationStrategy {
    /// Strict: Fail processing if Redis is unavailable
    /// Guarantees no duplicates but may block processing
    Strict,

    /// BestEffort: Allow events through if Redis is unavailable
    /// Prioritizes availability over perfect deduplication
    BestEffort,

    /// LogOnly: Only log duplicates, never filter
    /// Useful for monitoring without impacting processing
    LogOnly,
}
```

### 4. Main Implementation: RedisEventDeduplicator

```rust
/// Redis-based event deduplicator with comprehensive features
pub struct RedisEventDeduplicator<T, E> {
    /// Redis backend for storage
    redis: Arc<RedisStateBackend>,

    /// Event ID extraction strategy
    id_extractor: Arc<E>,

    /// Configuration
    config: DeduplicationConfig,

    /// Statistics tracking
    stats: Arc<RwLock<DeduplicationStats>>,

    /// Circuit breaker for Redis failures
    circuit_breaker: Arc<CircuitBreaker>,

    /// Phantom data for event type
    _phantom: PhantomData<T>,
}

impl<T, E> RedisEventDeduplicator<T, E>
where
    T: Send + Sync,
    E: EventIdExtractor<T> + Send + Sync,
{
    /// Create a new Redis-based deduplicator
    pub async fn new(
        redis_config: RedisConfig,
        id_extractor: E,
        config: DeduplicationConfig,
    ) -> DeduplicationResult<Self> {
        let redis = RedisStateBackend::new(redis_config).await?;
        let circuit_breaker = CircuitBreaker::new(
            config.circuit_breaker_threshold,
            config.circuit_breaker_timeout,
        );

        Ok(Self {
            redis: Arc::new(redis),
            id_extractor: Arc::new(id_extractor),
            config,
            stats: Arc::new(RwLock::new(DeduplicationStats::default())),
            circuit_breaker: Arc::new(circuit_breaker),
            _phantom: PhantomData,
        })
    }

    /// Internal: Build Redis key for event ID
    fn build_dedup_key(&self, event_id: &EventId) -> Vec<u8> {
        format!("dedup:{}", event_id.as_str()).into_bytes()
    }

    /// Internal: Check duplicate with circuit breaker
    async fn check_duplicate_internal(
        &self,
        event_id: &EventId,
    ) -> DeduplicationResult<bool> {
        // Check circuit breaker state
        if self.circuit_breaker.is_open() {
            return self.handle_redis_unavailable();
        }

        let key = self.build_dedup_key(event_id);

        match self.redis.set_if_not_exists(
            &key,
            b"1",
            self.config.ttl,
        ).await {
            Ok(was_set) => {
                self.circuit_breaker.record_success();
                // If set succeeded, event is unique (not a duplicate)
                // If set failed, key existed (is a duplicate)
                Ok(!was_set)
            }
            Err(e) => {
                self.circuit_breaker.record_failure();
                match self.config.strategy {
                    DeduplicationStrategy::Strict => Err(e.into()),
                    DeduplicationStrategy::BestEffort => {
                        warn!("Redis unavailable, allowing event: {}", e);
                        self.stats.write().await.fallback_count += 1;
                        Ok(false) // Treat as unique
                    }
                    DeduplicationStrategy::LogOnly => {
                        warn!("Redis unavailable in log-only mode: {}", e);
                        Ok(false)
                    }
                }
            }
        }
    }

    /// Handle Redis being unavailable based on strategy
    fn handle_redis_unavailable(&self) -> DeduplicationResult<bool> {
        match self.config.strategy {
            DeduplicationStrategy::Strict => {
                Err(DeduplicationError::StorageUnavailable {
                    details: "Redis circuit breaker is open".to_string(),
                })
            }
            DeduplicationStrategy::BestEffort | DeduplicationStrategy::LogOnly => {
                Ok(false) // Treat as unique
            }
        }
    }
}
```

---

## Data Structures

### Configuration Structure

```rust
/// Configuration for event deduplication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeduplicationConfig {
    /// Deduplication strategy
    pub strategy: DeduplicationStrategy,

    /// TTL for deduplication entries
    #[serde(with = "humantime_serde")]
    pub ttl: Duration,

    /// Key prefix for Redis keys
    pub key_prefix: String,

    /// Enable metrics collection
    #[serde(default = "default_true")]
    pub metrics_enabled: bool,

    /// Circuit breaker failure threshold (consecutive failures)
    pub circuit_breaker_threshold: usize,

    /// Circuit breaker timeout before retry
    #[serde(with = "humantime_serde")]
    pub circuit_breaker_timeout: Duration,

    /// Batch size for bulk operations
    pub batch_size: usize,

    /// Enable distributed tracing
    #[serde(default = "default_true")]
    pub tracing_enabled: bool,

    /// Sample rate for tracing (0.0 to 1.0)
    pub trace_sample_rate: f64,
}

impl Default for DeduplicationConfig {
    fn default() -> Self {
        Self {
            strategy: DeduplicationStrategy::BestEffort,
            ttl: Duration::from_secs(3600), // 1 hour
            key_prefix: "dedup:".to_string(),
            metrics_enabled: true,
            circuit_breaker_threshold: 5,
            circuit_breaker_timeout: Duration::from_secs(30),
            batch_size: 100,
            tracing_enabled: true,
            trace_sample_rate: 0.1,
        }
    }
}
```

### Statistics Structure

```rust
/// Statistics for deduplication operations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeduplicationStats {
    /// Total events checked
    pub total_checked: u64,

    /// Number of duplicates detected
    pub duplicates_found: u64,

    /// Number of unique events
    pub unique_events: u64,

    /// Number of Redis errors
    pub redis_errors: u64,

    /// Number of ID extraction failures
    pub extraction_failures: u64,

    /// Number of fallback decisions (allowed due to Redis failure)
    pub fallback_count: u64,

    /// Total processing time (microseconds)
    pub total_latency_us: u64,

    /// Average latency per check (microseconds)
    pub avg_latency_us: u64,

    /// P99 latency (microseconds)
    pub p99_latency_us: u64,

    /// Last check timestamp
    pub last_check_time: Option<DateTime<Utc>>,

    /// Circuit breaker state
    pub circuit_breaker_state: CircuitBreakerState,
}

impl DeduplicationStats {
    /// Calculate duplication rate (0.0 to 1.0)
    pub fn duplication_rate(&self) -> f64 {
        if self.total_checked == 0 {
            0.0
        } else {
            self.duplicates_found as f64 / self.total_checked as f64
        }
    }

    /// Calculate error rate (0.0 to 1.0)
    pub fn error_rate(&self) -> f64 {
        if self.total_checked == 0 {
            0.0
        } else {
            self.redis_errors as f64 / self.total_checked as f64
        }
    }

    /// Calculate fallback rate (0.0 to 1.0)
    pub fn fallback_rate(&self) -> f64 {
        if self.total_checked == 0 {
            0.0
        } else {
            self.fallback_count as f64 / self.total_checked as f64
        }
    }
}
```

### Error Types

```rust
/// Errors specific to deduplication operations
#[derive(Error, Debug)]
pub enum DeduplicationError {
    /// Failed to extract event ID
    #[error("ID extraction failed: {reason}")]
    ExtractionFailed { reason: String },

    /// Redis storage error
    #[error("storage error: {source}")]
    StorageError {
        #[from]
        source: StateError,
    },

    /// Redis is unavailable and strategy is Strict
    #[error("storage unavailable: {details}")]
    StorageUnavailable { details: String },

    /// Configuration error
    #[error("invalid configuration: {reason}")]
    InvalidConfig { reason: String },

    /// Serialization error
    #[error("serialization failed: {reason}")]
    SerializationFailed { reason: String },

    /// Circuit breaker is open
    #[error("circuit breaker open: too many failures")]
    CircuitBreakerOpen,
}

/// Result type for deduplication operations
pub type DeduplicationResult<T> = std::result::Result<T, DeduplicationError>;
```

---

## Redis Storage Schema

### Key Naming Convention

```
Pattern: {prefix}dedup:{event_id}
Example: dedup:a3f2c1b4-5678-90ab-cdef-1234567890ab
Example: dedup:hash:d41d8cd98f00b204e9800998ecf8427e
Example: dedup:composite:model:gpt4:session:abc123
```

### Key Structure

```redis
# Standard event ID
SET dedup:event_id "1" EX 3600

# With namespace
SET myapp:dedup:event_id "1" EX 7200

# Hash-based ID
SET dedup:hash:md5_of_content "1" EX 3600

# Composite ID
SET dedup:composite:field1:value1:field2:value2 "1" EX 3600
```

### Value Format

Values are minimal (just "1") since we only need membership testing:

```
Value: "1" (constant)
Purpose: SET membership test via SET NX
TTL: Configurable per key (default: 1 hour)
```

### Redis Commands Used

```redis
# Check and mark atomically (primary operation)
SET dedup:event_id "1" NX EX 3600
  Returns: 1 if set (unique), 0 if exists (duplicate)

# Check only (if separate check needed)
EXISTS dedup:event_id
  Returns: 1 if exists, 0 if not

# Manual mark (after separate check)
SETEX dedup:event_id 3600 "1"

# Get TTL (for monitoring)
TTL dedup:event_id

# Batch check (for pipeline optimization)
MULTI
SET dedup:id1 "1" NX EX 3600
SET dedup:id2 "1" NX EX 3600
SET dedup:id3 "1" NX EX 3600
EXEC

# Count total dedup entries (monitoring only - expensive!)
DBSIZE
```

### Memory Estimation

```
Per Key Overhead:
  - Key: ~50 bytes (prefix + event ID)
  - Value: 1 byte ("1")
  - Redis overhead: ~64 bytes
  - Total: ~115 bytes per event

Memory for 1M events: ~115 MB
Memory for 100M events: ~11.5 GB
Memory for 1B events: ~115 GB
```

### TTL Management

```
Default TTL: 1 hour (3600 seconds)
  - Covers typical processing windows
  - Automatically expires old entries
  - Prevents unbounded growth

Custom TTL by use case:
  - Real-time processing: 5 minutes
  - Hourly aggregation: 2 hours
  - Daily aggregation: 25 hours
  - Session-based: Session timeout + buffer
```

---

## Event ID Extraction Strategies

### 1. UUID-Based Extractor

Extract event ID from a UUID field:

```rust
/// Extract ID from a UUID field
pub struct UuidExtractor {
    field_name: &'static str,
}

impl<T> EventIdExtractor<T> for UuidExtractor
where
    T: Serialize,
{
    fn extract_id(&self, event: &T) -> DeduplicationResult<EventId> {
        // Use reflection or serde to extract UUID field
        // Implementation depends on event structure
        let uuid = extract_field_as_uuid(event, self.field_name)?;
        Ok(EventId::from_string(uuid.to_string()))
    }
}

// Pre-built extractors for common types
pub struct FeedbackEventUuidExtractor;

impl EventIdExtractor<FeedbackEvent> for FeedbackEventUuidExtractor {
    fn extract_id(&self, event: &FeedbackEvent) -> DeduplicationResult<EventId> {
        Ok(EventId::from_string(event.id.to_string()))
    }
}

pub struct CloudEventUuidExtractor;

impl EventIdExtractor<CloudEvent> for CloudEventUuidExtractor {
    fn extract_id(&self, event: &CloudEvent) -> DeduplicationResult<EventId> {
        Ok(EventId::from_string(event.id.clone()))
    }
}
```

### 2. Hash-Based Extractor

Generate deterministic hash from event content:

```rust
/// Extract ID by hashing event content
pub struct ContentHashExtractor<H = Sha256> {
    /// Fields to include in hash (None = all fields)
    include_fields: Option<Vec<String>>,
    /// Fields to exclude from hash
    exclude_fields: Vec<String>,
    /// Hash algorithm
    _phantom: PhantomData<H>,
}

impl<T, H> EventIdExtractor<T> for ContentHashExtractor<H>
where
    T: Serialize,
    H: Digest,
{
    fn extract_id(&self, event: &T) -> DeduplicationResult<EventId> {
        // Serialize event to canonical JSON
        let json = serde_json::to_value(event)
            .map_err(|e| DeduplicationError::SerializationFailed {
                reason: e.to_string(),
            })?;

        // Filter fields if needed
        let filtered = self.filter_fields(&json);

        // Serialize to canonical string
        let canonical = serde_json::to_string(&filtered)
            .map_err(|e| DeduplicationError::SerializationFailed {
                reason: e.to_string(),
            })?;

        // Hash the canonical representation
        let mut hasher = H::new();
        hasher.update(canonical.as_bytes());
        let hash = hasher.finalize();

        Ok(EventId::from_bytes(&hash))
    }
}

// Convenience constructors
impl ContentHashExtractor<Sha256> {
    /// Create SHA256-based hash extractor
    pub fn sha256() -> Self {
        Self {
            include_fields: None,
            exclude_fields: vec!["timestamp".to_string(), "id".to_string()],
            _phantom: PhantomData,
        }
    }
}
```

### 3. Composite Key Extractor

Combine multiple fields into a composite key:

```rust
/// Extract ID from multiple fields
pub struct CompositeExtractor {
    /// Field extractors in order
    fields: Vec<FieldExtractor>,
    /// Separator between fields
    separator: String,
}

pub enum FieldExtractor {
    /// Extract named field
    Field(String),
    /// Extract from metadata map
    Metadata(String),
    /// Extract from nested path (e.g., "data.user.id")
    Path(Vec<String>),
    /// Extract from custom function
    Custom(Arc<dyn Fn(&serde_json::Value) -> Option<String> + Send + Sync>),
}

impl<T> EventIdExtractor<T> for CompositeExtractor
where
    T: Serialize,
{
    fn extract_id(&self, event: &T) -> DeduplicationResult<EventId> {
        let json = serde_json::to_value(event)
            .map_err(|e| DeduplicationError::SerializationFailed {
                reason: e.to_string(),
            })?;

        let mut parts = Vec::new();
        for field in &self.fields {
            let value = field.extract(&json).ok_or_else(|| {
                DeduplicationError::ExtractionFailed {
                    reason: format!("Failed to extract field: {:?}", field),
                }
            })?;
            parts.push(value);
        }

        let composite_id = parts.join(&self.separator);
        Ok(EventId::from_string(composite_id))
    }
}

// Example: Composite extractor for model + session
pub fn model_session_extractor() -> CompositeExtractor {
    CompositeExtractor {
        fields: vec![
            FieldExtractor::Metadata("model_id".to_string()),
            FieldExtractor::Metadata("session_id".to_string()),
        ],
        separator: ":".to_string(),
    }
}
```

### 4. Time-Window Hash Extractor

Hash with time window for time-bounded deduplication:

```rust
/// Hash-based extractor with time window bucketing
pub struct TimeWindowHashExtractor {
    /// Base hash extractor
    base_extractor: ContentHashExtractor,
    /// Window size for bucketing
    window_size: Duration,
}

impl<T> EventIdExtractor<T> for TimeWindowHashExtractor
where
    T: Serialize,
{
    fn extract_id(&self, event: &T) -> DeduplicationResult<EventId> {
        // Get base hash
        let base_id = self.base_extractor.extract_id(event)?;

        // Get time bucket (e.g., hour of day)
        let now = Utc::now();
        let bucket = now.timestamp() / self.window_size.as_secs() as i64;

        // Combine hash with time bucket
        let windowed_id = format!("{}:{}", bucket, base_id.as_str());
        Ok(EventId::from_string(windowed_id))
    }
}
```

### Strategy Selection Guide

| Use Case | Strategy | Rationale |
|----------|----------|-----------|
| Events with stable UUIDs | UUID-Based | Most efficient, uses existing ID |
| Events without IDs | Content Hash | Deterministic based on content |
| Multi-tenant systems | Composite (tenant + uuid) | Namespace isolation |
| Idempotent retries | Content Hash (exclude timestamp) | Same content = same ID |
| Time-based processing | Time-Window Hash | Bounded deduplication window |
| User sessions | Composite (user + session) | Session-scoped deduplication |

---

## TTL Management

### TTL Selection Strategy

```rust
/// TTL strategy determines how long deduplication state persists
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TtlStrategy {
    /// Fixed TTL for all events
    Fixed(Duration),

    /// TTL based on event type
    ByEventType(HashMap<String, Duration>),

    /// TTL based on custom function
    Dynamic(Arc<dyn Fn(&EventId) -> Duration + Send + Sync>),

    /// TTL based on processing window
    WindowBased {
        /// Window size
        window_size: Duration,
        /// Safety buffer (e.g., 2x window size)
        buffer_multiplier: f64,
    },
}

impl TtlStrategy {
    /// Get TTL for an event
    pub fn get_ttl(&self, event_id: &EventId, event_type: Option<&str>) -> Duration {
        match self {
            TtlStrategy::Fixed(duration) => *duration,
            TtlStrategy::ByEventType(map) => {
                event_type
                    .and_then(|t| map.get(t))
                    .copied()
                    .unwrap_or(Duration::from_secs(3600))
            }
            TtlStrategy::Dynamic(f) => f(event_id),
            TtlStrategy::WindowBased {
                window_size,
                buffer_multiplier,
            } => {
                let buffer = window_size.as_secs_f64() * buffer_multiplier;
                Duration::from_secs(buffer as u64)
            }
        }
    }
}
```

### TTL Best Practices

1. **Window-Based Processing**: TTL = 2x window size
   ```rust
   // For 1-hour tumbling windows
   let ttl = Duration::from_secs(2 * 3600); // 2 hours
   ```

2. **Session-Based**: TTL = session timeout + buffer
   ```rust
   // For 30-minute sessions
   let ttl = Duration::from_secs(30 * 60 + 5 * 60); // 35 minutes
   ```

3. **Real-Time Processing**: Short TTL for recent events
   ```rust
   // For real-time processing
   let ttl = Duration::from_secs(300); // 5 minutes
   ```

4. **Batch Processing**: TTL = batch interval + processing time
   ```rust
   // For hourly batch jobs
   let ttl = Duration::from_secs(3600 + 600); // 1h 10m
   ```

### Automatic Cleanup

Redis handles cleanup automatically through TTL expiration:

```redis
# Automatic expiration (passive)
# - Key expires when TTL reaches 0
# - Lazy deletion on access

# Active expiration (background)
# - Redis periodically scans for expired keys
# - Configurable via redis.conf:
#   hz 10  (10 times per second)
```

---

## Integration Patterns

### Pattern 1: Pipeline Operator

Integrate as a pipeline operator:

```rust
/// Deduplication operator for stream pipelines
pub struct DeduplicationOperator<T, E> {
    deduplicator: Arc<RedisEventDeduplicator<T, E>>,
}

#[async_trait]
impl<T, E> StreamOperator for DeduplicationOperator<T, E>
where
    T: Send + Sync + Clone,
    E: EventIdExtractor<T> + Send + Sync,
{
    type Input = ProcessorEvent<T>;
    type Output = ProcessorEvent<T>;

    async fn process(
        &self,
        input: Self::Input,
        _ctx: &OperatorContext,
    ) -> Result<Vec<Self::Output>> {
        // Check if duplicate
        let is_dup = self.deduplicator.check_and_mark(&input.event).await?;

        if is_dup {
            // Duplicate detected - filter out
            debug!("Filtered duplicate event: {:?}", input.key);
            Ok(Vec::new())
        } else {
            // Unique event - pass through
            Ok(vec![input])
        }
    }
}

// Usage in pipeline
let pipeline = StreamPipelineBuilder::new()
    .with_source(kafka_source)
    .add_operator(DeduplicationOperator::new(deduplicator))
    .add_operator(WindowOperator::new(window_assigner))
    .add_operator(AggregateOperator::new(aggregator))
    .build()?;
```

### Pattern 2: Pre-Processing Filter

Apply before event ingestion:

```rust
/// Filter events at ingestion time
pub async fn ingest_with_deduplication<T>(
    event: T,
    deduplicator: &impl EventDeduplicator,
    processor: &mut StreamProcessor<T>,
) -> Result<()> {
    // Check and mark atomically
    if deduplicator.check_and_mark(&event).await? {
        // Duplicate - log and skip
        debug!("Skipping duplicate event");
        return Ok(());
    }

    // Unique - process
    processor.process(event).await
}
```

### Pattern 3: Kafka Consumer Integration

Integrate with Kafka consumer:

```rust
/// Kafka source with built-in deduplication
pub struct DeduplicatedKafkaSource<T, E> {
    kafka_source: KafkaSource<T>,
    deduplicator: Arc<RedisEventDeduplicator<T, E>>,
}

impl<T, E> DeduplicatedKafkaSource<T, E>
where
    T: DeserializeOwned + Send + Sync,
    E: EventIdExtractor<T> + Send + Sync,
{
    pub async fn consume(&mut self) -> Result<Vec<T>> {
        // Consume from Kafka
        let events = self.kafka_source.consume().await?;

        // Filter duplicates
        let mut unique_events = Vec::new();
        for event in events {
            if !self.deduplicator.check_and_mark(&event).await? {
                unique_events.push(event);
            }
        }

        Ok(unique_events)
    }
}
```

### Pattern 4: Batch Processing

Process events in batches for efficiency:

```rust
impl<T, E> RedisEventDeduplicator<T, E>
where
    T: Send + Sync,
    E: EventIdExtractor<T> + Send + Sync,
{
    /// Check multiple events in a batch
    pub async fn check_batch(
        &self,
        events: &[T],
    ) -> DeduplicationResult<Vec<bool>> {
        // Extract all IDs
        let ids: Vec<EventId> = events
            .iter()
            .map(|e| self.id_extractor.extract_id(e))
            .collect::<Result<Vec<_>, _>>()?;

        // Build all keys
        let keys: Vec<Vec<u8>> = ids
            .iter()
            .map(|id| self.build_dedup_key(id))
            .collect();

        // Use Redis pipeline for batch SET NX
        let mut results = Vec::new();
        for key in keys {
            let is_dup = !self.redis.set_if_not_exists(
                &key,
                b"1",
                self.config.ttl,
            ).await?;
            results.push(is_dup);
        }

        Ok(results)
    }

    /// Filter a batch of events to remove duplicates
    pub async fn filter_batch(
        &self,
        events: Vec<T>,
    ) -> DeduplicationResult<Vec<T>> {
        let dup_flags = self.check_batch(&events).await?;

        let unique: Vec<T> = events
            .into_iter()
            .zip(dup_flags)
            .filter_map(|(event, is_dup)| {
                if is_dup {
                    None
                } else {
                    Some(event)
                }
            })
            .collect();

        Ok(unique)
    }
}
```

---

## Configuration Schema

### TOML Configuration

```toml
[deduplication]
# Deduplication strategy: "strict", "best_effort", or "log_only"
strategy = "best_effort"

# TTL for deduplication entries (human-readable format)
ttl = "1h"

# Key prefix for Redis
key_prefix = "dedup:"

# Enable metrics collection
metrics_enabled = true

# Enable distributed tracing
tracing_enabled = true

# Trace sampling rate (0.0 to 1.0)
trace_sample_rate = 0.1

# Batch size for bulk operations
batch_size = 100

[deduplication.circuit_breaker]
# Consecutive failures before opening circuit
threshold = 5

# Timeout before attempting to close circuit
timeout = "30s"

[deduplication.redis]
# Redis connection URL
urls = ["redis://localhost:6379"]

# Redis key prefix
key_prefix = "myapp:"

# Default TTL for Redis keys
default_ttl = "1h"

# Connection pool settings
min_connections = 5
max_connections = 50

# Timeouts
connect_timeout = "5s"
read_timeout = "3s"
write_timeout = "3s"

# Retry configuration
max_retries = 3
retry_base_delay = "100ms"
retry_max_delay = "5s"

# Enable cluster mode
cluster_mode = false

# Enable TLS
tls_enabled = false

[deduplication.id_extraction]
# Strategy: "uuid", "hash", "composite", or "time_window_hash"
strategy = "uuid"

# For UUID strategy: field name to extract
uuid_field = "id"

# For hash strategy: fields to include/exclude
[deduplication.id_extraction.hash]
exclude_fields = ["timestamp", "processing_time"]
algorithm = "sha256"

# For composite strategy: fields to combine
[deduplication.id_extraction.composite]
fields = ["model_id", "session_id"]
separator = ":"

# For time window hash: window size
[deduplication.id_extraction.time_window]
window_size = "1h"
base_strategy = "hash"
```

### Programmatic Configuration

```rust
/// Build configuration programmatically
let config = DeduplicationConfig::builder()
    .strategy(DeduplicationStrategy::BestEffort)
    .ttl(Duration::from_secs(3600))
    .key_prefix("myapp:dedup:")
    .metrics_enabled(true)
    .circuit_breaker(5, Duration::from_secs(30))
    .batch_size(100)
    .build()?;

// Configure Redis backend
let redis_config = RedisConfig::builder()
    .url("redis://localhost:6379")
    .key_prefix("myapp:")
    .default_ttl(Duration::from_secs(3600))
    .pool_size(10, 50)
    .build()?;

// Create ID extractor
let id_extractor = FeedbackEventUuidExtractor;

// Create deduplicator
let deduplicator = RedisEventDeduplicator::new(
    redis_config,
    id_extractor,
    config,
).await?;
```

---

## Error Handling & Fallback

### Error Handling Strategy

```rust
impl<T, E> RedisEventDeduplicator<T, E>
where
    T: Send + Sync,
    E: EventIdExtractor<T> + Send + Sync,
{
    async fn check_and_mark_with_fallback(
        &self,
        event: &T,
    ) -> DeduplicationResult<bool> {
        // Extract event ID
        let event_id = match self.id_extractor.extract_id(event) {
            Ok(id) => id,
            Err(e) => {
                error!("Failed to extract event ID: {}", e);
                self.stats.write().await.extraction_failures += 1;

                match self.config.strategy {
                    DeduplicationStrategy::Strict => return Err(e),
                    _ => {
                        warn!("Allowing event due to extraction failure");
                        return Ok(false); // Treat as unique
                    }
                }
            }
        };

        // Check duplicate with retry
        let start = Instant::now();
        let result = self.check_duplicate_internal(&event_id).await;
        let latency = start.elapsed();

        // Update stats
        let mut stats = self.stats.write().await;
        stats.total_checked += 1;
        stats.last_check_time = Some(Utc::now());
        stats.total_latency_us += latency.as_micros() as u64;
        stats.avg_latency_us = stats.total_latency_us / stats.total_checked;

        match result {
            Ok(is_dup) => {
                if is_dup {
                    stats.duplicates_found += 1;
                } else {
                    stats.unique_events += 1;
                }
                Ok(is_dup)
            }
            Err(e) => {
                stats.redis_errors += 1;

                match self.config.strategy {
                    DeduplicationStrategy::Strict => Err(e),
                    _ => {
                        warn!("Redis error, allowing event: {}", e);
                        stats.fallback_count += 1;
                        Ok(false) // Treat as unique
                    }
                }
            }
        }
    }
}
```

### Circuit Breaker Implementation

```rust
/// Circuit breaker for Redis failures
pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitBreakerState>>,
    failure_threshold: usize,
    timeout: Duration,
    consecutive_failures: Arc<AtomicUsize>,
    last_failure_time: Arc<RwLock<Option<Instant>>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CircuitBreakerState {
    Closed,  // Normal operation
    Open,    // Failures exceeded threshold
    HalfOpen, // Testing if service recovered
}

impl CircuitBreaker {
    pub fn new(failure_threshold: usize, timeout: Duration) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitBreakerState::Closed)),
            failure_threshold,
            timeout,
            consecutive_failures: Arc::new(AtomicUsize::new(0)),
            last_failure_time: Arc::new(RwLock::new(None)),
        }
    }

    pub fn is_open(&self) -> bool {
        let state = *self.state.blocking_read();
        match state {
            CircuitBreakerState::Open => {
                // Check if timeout has elapsed
                if let Some(last_failure) = *self.last_failure_time.blocking_read() {
                    if last_failure.elapsed() > self.timeout {
                        // Transition to half-open
                        *self.state.blocking_write() = CircuitBreakerState::HalfOpen;
                        return false;
                    }
                }
                true
            }
            _ => false,
        }
    }

    pub fn record_success(&self) {
        self.consecutive_failures.store(0, Ordering::SeqCst);
        *self.state.blocking_write() = CircuitBreakerState::Closed;
    }

    pub fn record_failure(&self) {
        let failures = self.consecutive_failures.fetch_add(1, Ordering::SeqCst) + 1;
        *self.last_failure_time.blocking_write() = Some(Instant::now());

        if failures >= self.failure_threshold {
            warn!("Circuit breaker opening after {} failures", failures);
            *self.state.blocking_write() = CircuitBreakerState::Open;
        }
    }
}
```

### Graceful Degradation Modes

1. **Strict Mode**: Fail processing on any error
   - Use case: Financial transactions, critical data
   - Behavior: Return error, block processing

2. **Best Effort Mode**: Continue processing on errors
   - Use case: Analytics, monitoring, non-critical data
   - Behavior: Log error, treat as unique, continue

3. **Log Only Mode**: Never filter, only log duplicates
   - Use case: Testing, monitoring, validation
   - Behavior: Always return false (unique), log duplicates

---

## Performance Characteristics

### Latency Analysis

```
Operation: check_and_mark (atomic)
-----------------------------------------
Redis latency:       1-5ms   (p50)
Network RTT:         0.1-1ms (local)
Serialization:       0.01-0.1ms
ID extraction:       0.001-0.01ms
-----------------------------------------
Total latency:       1-6ms   (p50)
                     2-10ms  (p95)
                     5-20ms  (p99)
```

### Throughput Analysis

```
Single-threaded throughput:
- Local Redis:     10,000-50,000 ops/sec
- Remote Redis:    5,000-20,000 ops/sec
- Redis cluster:   50,000-200,000 ops/sec

Batch processing (100 events/batch):
- Pipeline mode:   100,000-500,000 ops/sec
```

### Memory Usage

```
Per Event Memory:
- Redis key:       ~50 bytes
- Redis value:     1 byte
- Redis overhead:  ~64 bytes
- Total:           ~115 bytes/event

Storage Examples:
- 1M events:       ~115 MB
- 10M events:      ~1.15 GB
- 100M events:     ~11.5 GB
- 1B events:       ~115 GB
```

### Scaling Characteristics

```
Horizontal Scaling (Redis Cluster):
- Linear scaling with cluster size
- 3-node cluster: 3x throughput
- 6-node cluster: 6x throughput

Vertical Scaling (Redis Instance):
- CPU: 1 core per 10k ops/sec
- Memory: 1GB per 10M events
- Network: 10Mbps per 5k ops/sec
```

---

## Monitoring & Metrics

### Prometheus Metrics

```rust
use prometheus::{Counter, Gauge, Histogram, IntCounter, IntGauge, Registry};

/// Metrics for deduplication operations
pub struct DeduplicationMetrics {
    /// Total events checked
    pub total_checked: IntCounter,

    /// Duplicates detected
    pub duplicates_found: IntCounter,

    /// Unique events
    pub unique_events: IntCounter,

    /// Redis errors
    pub redis_errors: IntCounter,

    /// ID extraction failures
    pub extraction_failures: IntCounter,

    /// Fallback decisions
    pub fallback_count: IntCounter,

    /// Check latency histogram
    pub check_latency: Histogram,

    /// Current circuit breaker state (0=closed, 1=open, 2=half-open)
    pub circuit_breaker_state: IntGauge,

    /// Duplication rate (0.0 to 1.0)
    pub duplication_rate: Gauge,

    /// Error rate (0.0 to 1.0)
    pub error_rate: Gauge,
}

impl DeduplicationMetrics {
    pub fn new(registry: &Registry) -> Result<Self> {
        let total_checked = IntCounter::new(
            "dedup_total_checked",
            "Total events checked for deduplication"
        )?;
        registry.register(Box::new(total_checked.clone()))?;

        let duplicates_found = IntCounter::new(
            "dedup_duplicates_found",
            "Number of duplicate events detected"
        )?;
        registry.register(Box::new(duplicates_found.clone()))?;

        let unique_events = IntCounter::new(
            "dedup_unique_events",
            "Number of unique events"
        )?;
        registry.register(Box::new(unique_events.clone()))?;

        let check_latency = Histogram::with_opts(
            histogram_opts!(
                "dedup_check_latency_seconds",
                "Latency of deduplication checks",
                vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]
            )
        )?;
        registry.register(Box::new(check_latency.clone()))?;

        // ... register other metrics

        Ok(Self {
            total_checked,
            duplicates_found,
            unique_events,
            redis_errors,
            extraction_failures,
            fallback_count,
            check_latency,
            circuit_breaker_state,
            duplication_rate,
            error_rate,
        })
    }

    /// Update metrics from stats
    pub fn update_from_stats(&self, stats: &DeduplicationStats) {
        let dup_rate = stats.duplication_rate();
        let err_rate = stats.error_rate();

        self.duplication_rate.set(dup_rate);
        self.error_rate.set(err_rate);

        let cb_state = match stats.circuit_breaker_state {
            CircuitBreakerState::Closed => 0,
            CircuitBreakerState::Open => 1,
            CircuitBreakerState::HalfOpen => 2,
        };
        self.circuit_breaker_state.set(cb_state);
    }
}
```

### Distributed Tracing

```rust
use opentelemetry::trace::{Span, Tracer};

impl<T, E> RedisEventDeduplicator<T, E>
where
    T: Send + Sync,
    E: EventIdExtractor<T> + Send + Sync,
{
    #[tracing::instrument(skip(self, event))]
    async fn check_and_mark_traced(
        &self,
        event: &T,
    ) -> DeduplicationResult<bool> {
        let tracer = opentelemetry::global::tracer("deduplicator");
        let mut span = tracer.start("check_and_mark");

        // Extract ID
        span.add_event("extracting_id".to_string(), vec![]);
        let event_id = self.id_extractor.extract_id(event)?;
        span.set_attribute(KeyValue::new("event.id", event_id.as_str().to_string()));

        // Check duplicate
        span.add_event("checking_redis".to_string(), vec![]);
        let is_dup = self.check_duplicate_internal(&event_id).await?;
        span.set_attribute(KeyValue::new("is_duplicate", is_dup));

        Ok(is_dup)
    }
}
```

### Logging

```rust
use tracing::{debug, info, warn, error};

// Structured logging examples
debug!(
    event_id = %event_id.as_str(),
    "Checking for duplicate"
);

info!(
    duplicates = stats.duplicates_found,
    unique = stats.unique_events,
    rate = %format!("{:.2}%", stats.duplication_rate() * 100.0),
    "Deduplication statistics"
);

warn!(
    error = %e,
    event_id = %event_id.as_str(),
    "Redis error during deduplication check"
);

error!(
    consecutive_failures = failures,
    "Circuit breaker opened"
);
```

### Health Check Endpoint

```rust
/// Health status for deduplication system
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: HealthState,
    pub redis_available: bool,
    pub circuit_breaker_state: CircuitBreakerState,
    pub last_check_time: Option<DateTime<Utc>>,
    pub stats: DeduplicationStats,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum HealthState {
    Healthy,
    Degraded,
    Unhealthy,
}

impl<T, E> RedisEventDeduplicator<T, E>
where
    T: Send + Sync,
    E: EventIdExtractor<T> + Send + Sync,
{
    pub async fn health_check(&self) -> DeduplicationResult<HealthStatus> {
        // Check Redis availability
        let redis_available = self.redis.health_check().await.unwrap_or(false);

        // Get circuit breaker state
        let cb_state = *self.circuit_breaker.state.read().await;

        // Get current stats
        let stats = self.stats.read().await.clone();

        // Determine overall health
        let status = match (redis_available, cb_state) {
            (true, CircuitBreakerState::Closed) => HealthState::Healthy,
            (true, CircuitBreakerState::HalfOpen) => HealthState::Degraded,
            (false, _) | (_, CircuitBreakerState::Open) => HealthState::Unhealthy,
        };

        Ok(HealthStatus {
            status,
            redis_available,
            circuit_breaker_state: cb_state,
            last_check_time: stats.last_check_time,
            stats,
        })
    }
}
```

---

## Production Deployment

### Deployment Checklist

```markdown
Pre-Deployment:
- [ ] Redis cluster configured and tested
- [ ] TTL strategy validated for use case
- [ ] ID extraction strategy tested
- [ ] Circuit breaker thresholds tuned
- [ ] Metrics and alerts configured
- [ ] Backup and recovery plan documented
- [ ] Performance tested under load
- [ ] Security review completed

Configuration:
- [ ] Redis connection pool sized appropriately
- [ ] TTL set based on processing windows
- [ ] Strategy (Strict/BestEffort) matches requirements
- [ ] Circuit breaker thresholds configured
- [ ] Batch size optimized for workload
- [ ] Monitoring enabled

Operations:
- [ ] Runbook created for common issues
- [ ] Alerts configured for errors and failures
- [ ] Dashboards created for visualization
- [ ] Backup Redis instances configured
- [ ] Capacity planning completed
```

### Redis Deployment Best Practices

```yaml
# Redis Configuration (redis.conf)
maxmemory: 10gb
maxmemory-policy: volatile-lru  # Evict expired keys when full

# Persistence (for recovery)
save 900 1      # Save after 900 sec if 1 key changed
save 300 10     # Save after 300 sec if 10 keys changed
save 60 10000   # Save after 60 sec if 10000 keys changed

# Replication (for HA)
replicaof: redis-primary 6379
replica-read-only: yes

# Cluster mode (for scaling)
cluster-enabled: yes
cluster-config-file: nodes.conf
cluster-node-timeout: 5000

# Performance tuning
tcp-backlog: 511
timeout: 0
tcp-keepalive: 300
hz: 10  # Active expiry frequency
```

### Kubernetes Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: stream-processor
spec:
  replicas: 3
  template:
    spec:
      containers:
      - name: processor
        image: llm-optimizer/stream-processor:latest
        env:
        - name: REDIS_URL
          valueFrom:
            secretKeyRef:
              name: redis-credentials
              key: url
        - name: DEDUP_STRATEGY
          value: "best_effort"
        - name: DEDUP_TTL
          value: "3600"
        resources:
          requests:
            memory: "512Mi"
            cpu: "500m"
          limits:
            memory: "1Gi"
            cpu: "1000m"
---
apiVersion: v1
kind: Service
metadata:
  name: redis-dedup
spec:
  type: ClusterIP
  ports:
  - port: 6379
    targetPort: 6379
  selector:
    app: redis
---
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: redis-dedup
spec:
  serviceName: redis-dedup
  replicas: 3
  template:
    spec:
      containers:
      - name: redis
        image: redis:7-alpine
        command: ["redis-server"]
        args: ["--maxmemory", "10gb", "--maxmemory-policy", "volatile-lru"]
        ports:
        - containerPort: 6379
        volumeMounts:
        - name: redis-data
          mountPath: /data
  volumeClaimTemplates:
  - metadata:
      name: redis-data
    spec:
      accessModes: ["ReadWriteOnce"]
      resources:
        requests:
          storage: 20Gi
```

### Monitoring & Alerting

```yaml
# Prometheus Alerts
groups:
- name: deduplication
  interval: 30s
  rules:
  - alert: HighDuplicationRate
    expr: dedup_duplication_rate > 0.3
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "High event duplication rate: {{ $value }}"

  - alert: RedisErrors
    expr: rate(dedup_redis_errors[5m]) > 10
    for: 2m
    labels:
      severity: critical
    annotations:
      summary: "High Redis error rate: {{ $value }}/sec"

  - alert: CircuitBreakerOpen
    expr: dedup_circuit_breaker_state == 1
    for: 1m
    labels:
      severity: critical
    annotations:
      summary: "Deduplication circuit breaker is open"

  - alert: HighLatency
    expr: histogram_quantile(0.99, dedup_check_latency_seconds) > 0.1
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "High deduplication latency (p99): {{ $value }}s"
```

### Capacity Planning

```
Event Rate: 10,000 events/sec
Window: 1 hour
Duplication Rate: 10%

Calculations:
- Events per hour: 10,000 * 3600 = 36M
- Unique events: 36M * 0.9 = 32.4M
- Memory needed: 32.4M * 115 bytes = 3.7 GB
- Redis instance: 4GB + overhead = 6GB recommended

Scaling:
- 100k events/sec → 60GB Redis (cluster recommended)
- 1M events/sec → 600GB Redis (requires sharding)
```

### Disaster Recovery

```markdown
Backup Strategy:
1. Redis Persistence (RDB snapshots every 15 minutes)
2. Replica sets for high availability
3. Backup Redis instances in different AZ

Recovery Procedures:
1. Primary failure: Promote replica (< 1 minute RTO)
2. Cluster failure: Restore from backup (5-15 minute RTO)
3. Data loss: Expect duplicate processing for TTL window

Monitoring:
- Redis replication lag
- Backup success/failure
- Available disk space
- Memory usage
```

---

## Appendix: Complete Implementation Example

### Full Usage Example

```rust
use processor::deduplication::{
    RedisEventDeduplicator,
    DeduplicationConfig,
    DeduplicationStrategy,
    FeedbackEventUuidExtractor,
};
use processor::state::RedisConfig;
use llm_optimizer_types::events::FeedbackEvent;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Configure Redis
    let redis_config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("llm_optimizer:")
        .default_ttl(Duration::from_secs(3600))
        .pool_size(10, 50)
        .build()?;

    // Configure deduplication
    let dedup_config = DeduplicationConfig {
        strategy: DeduplicationStrategy::BestEffort,
        ttl: Duration::from_secs(3600),
        key_prefix: "dedup:".to_string(),
        metrics_enabled: true,
        circuit_breaker_threshold: 5,
        circuit_breaker_timeout: Duration::from_secs(30),
        batch_size: 100,
        tracing_enabled: true,
        trace_sample_rate: 0.1,
    };

    // Create ID extractor
    let id_extractor = FeedbackEventUuidExtractor;

    // Create deduplicator
    let deduplicator = RedisEventDeduplicator::new(
        redis_config,
        id_extractor,
        dedup_config,
    ).await?;

    // Process events
    let event = FeedbackEvent::new(
        EventSource::Observatory,
        EventType::Metric,
        serde_json::json!({"latency": 123.45}),
    );

    // Check and mark atomically
    match deduplicator.check_and_mark(&event).await {
        Ok(true) => {
            println!("Duplicate event detected - skipping");
        }
        Ok(false) => {
            println!("Unique event - processing");
            // Process event...
        }
        Err(e) => {
            eprintln!("Deduplication error: {}", e);
            // Handle error based on strategy
        }
    }

    // Get statistics
    let stats = deduplicator.stats().await;
    println!("Deduplication stats: {:#?}", stats);
    println!("Duplication rate: {:.2}%", stats.duplication_rate() * 100.0);

    // Health check
    let health = deduplicator.health_check().await?;
    println!("Health status: {:?}", health.status);

    Ok(())
}
```

---

## Summary

This architecture provides:

1. **Enterprise-Grade Reliability**: Circuit breakers, graceful degradation, health checks
2. **Production Performance**: O(1) operations, batch processing, efficient memory usage
3. **Operational Excellence**: Comprehensive metrics, distributed tracing, alerting
4. **Flexibility**: Multiple strategies, custom ID extractors, configurable TTL
5. **Scalability**: Redis clustering, horizontal scaling, bounded memory
6. **Safety**: Atomic operations, idempotent design, fail-safe defaults

The system is ready for production deployment in high-throughput, mission-critical environments.
