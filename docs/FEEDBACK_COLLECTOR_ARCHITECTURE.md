# Feedback Collector Architecture

**Version:** 1.0.0
**Status:** Design Complete - Ready for Implementation
**Last Updated:** 2025-11-09

---

## Executive Summary

This document defines the comprehensive enterprise-grade architecture for the Feedback Collector component of the LLM Auto-Optimizer system. The Feedback Collector is responsible for collecting, processing, and streaming LLM interaction feedback data using OpenTelemetry for observability and Apache Kafka for reliable message streaming.

### Key Design Goals

- **Production-Ready**: Enterprise-grade reliability with 99.9% availability target
- **High Throughput**: Support 10,000+ events/second ingestion rate
- **Low Latency**: Sub-10ms event ingestion, <100ms end-to-end processing
- **Resilient**: Automatic retries, circuit breakers, graceful degradation
- **Observable**: Comprehensive metrics, tracing, and logging
- **Scalable**: Horizontal scaling via Kafka partitioning
- **Secure**: TLS encryption, authentication, authorization

---

## Table of Contents

1. [System Overview](#1-system-overview)
2. [Component Architecture](#2-component-architecture)
3. [Data Models](#3-data-models)
4. [Technology Stack](#4-technology-stack)
5. [OpenTelemetry Integration](#5-opentelemetry-integration)
6. [Kafka Integration](#6-kafka-integration)
7. [Error Handling Strategy](#7-error-handling-strategy)
8. [Configuration Management](#8-configuration-management)
9. [Deployment Architecture](#9-deployment-architecture)
10. [Scalability & Performance](#10-scalability--performance)
11. [Security Considerations](#11-security-considerations)
12. [Monitoring & Observability](#12-monitoring--observability)
13. [Testing Strategy](#13-testing-strategy)
14. [Implementation Roadmap](#14-implementation-roadmap)

---

## 1. System Overview

### 1.1 Purpose

The Feedback Collector is the primary data ingestion component of the LLM Auto-Optimizer. It collects feedback events from multiple sources:

- **LLM Interaction Metrics**: Latency, token usage, cost, quality scores
- **User Feedback**: Explicit ratings, thumbs up/down, comments
- **System Health Events**: Error rates, availability, resource utilization
- **Experiment Results**: A/B test outcomes, bandit algorithm rewards
- **Model Performance**: Response quality, accuracy, relevance

### 1.2 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                      FEEDBACK COLLECTOR                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    INGESTION LAYER                           │  │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐             │  │
│  │  │   HTTP     │  │   gRPC     │  │  Direct    │             │  │
│  │  │   API      │  │   API      │  │  SDK       │             │  │
│  │  └─────┬──────┘  └─────┬──────┘  └─────┬──────┘             │  │
│  └────────┼─────────────────┼─────────────────┼──────────────────┘  │
│           │                 │                 │                     │
│           └─────────────────┴─────────────────┘                     │
│                             │                                       │
│  ┌──────────────────────────▼───────────────────────────────────┐  │
│  │                    PROCESSING LAYER                          │  │
│  │  ┌────────────────────────────────────────────────────────┐  │  │
│  │  │           Event Validation & Enrichment                │  │  │
│  │  └────────────────────────────────────────────────────────┘  │  │
│  │  ┌────────────────────────────────────────────────────────┐  │  │
│  │  │           Ring Buffer (10K capacity)                   │  │  │
│  │  └────────────────────────────────────────────────────────┘  │  │
│  │  ┌────────────────────────────────────────────────────────┐  │  │
│  │  │           Batching Engine                              │  │  │
│  │  │           - Max size: 100 events                       │  │  │
│  │  │           - Max age: 10 seconds                        │  │  │
│  │  └────────────────────────────────────────────────────────┘  │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                             │                                       │
│  ┌──────────────────────────▼───────────────────────────────────┐  │
│  │                    STREAMING LAYER                           │  │
│  │  ┌────────────────────────────────────────────────────────┐  │  │
│  │  │           Kafka Producer (rdkafka)                     │  │  │
│  │  │           - Idempotent writes                          │  │  │
│  │  │           - Gzip compression                           │  │  │
│  │  │           - Async with retries                         │  │  │
│  │  └────────────────────────────────────────────────────────┘  │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                             │                                       │
│  ┌──────────────────────────▼───────────────────────────────────┐  │
│  │                  OBSERVABILITY LAYER                         │  │
│  │  ┌────────────────────────────────────────────────────────┐  │  │
│  │  │           OpenTelemetry SDK                            │  │  │
│  │  │           - Metrics: Prometheus                        │  │  │
│  │  │           - Traces: OTLP/gRPC                          │  │  │
│  │  │           - Logs: Structured JSON                      │  │  │
│  │  └────────────────────────────────────────────────────────┘  │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
                    ┌──────────────────┐
                    │  Kafka Cluster   │
                    │  Topic: feedback │
                    │  Partitions: 12  │
                    └──────────────────┘
```

### 1.3 Design Principles

1. **Fail Fast, Recover Gracefully**: Validate early, handle errors explicitly
2. **Async by Default**: Non-blocking I/O throughout the pipeline
3. **Backpressure Aware**: Bounded buffers, flow control
4. **Observable**: Instrument every critical path
5. **Idempotent**: Safe to retry operations
6. **Stateless**: No local state, horizontally scalable

---

## 2. Component Architecture

### 2.1 Component Diagram

```
┌────────────────────────────────────────────────────────────────────┐
│                    FeedbackCollector                               │
├────────────────────────────────────────────────────────────────────┤
│                                                                    │
│  ┌──────────────────┐      ┌──────────────────┐                   │
│  │  IngestHandler   │─────▶│  EventValidator  │                   │
│  └────────┬─────────┘      └──────────────────┘                   │
│           │                                                        │
│           ▼                                                        │
│  ┌──────────────────┐      ┌──────────────────┐                   │
│  │  RingBuffer      │─────▶│  EventEnricher   │                   │
│  │  (mpsc channel)  │      └──────────────────┘                   │
│  └────────┬─────────┘                                              │
│           │                                                        │
│           ▼                                                        │
│  ┌──────────────────┐      ┌──────────────────┐                   │
│  │  BatchingEngine  │─────▶│  KafkaProducer   │                   │
│  └──────────────────┘      └────────┬─────────┘                   │
│           │                          │                             │
│           │                          │                             │
│  ┌────────▼─────────┐      ┌────────▼─────────┐                   │
│  │  TelemetryMetrics│      │  RetryHandler    │                   │
│  └──────────────────┘      └──────────────────┘                   │
│                                                                    │
└────────────────────────────────────────────────────────────────────┘
```

### 2.2 Core Components

#### 2.2.1 IngestHandler

**Responsibility**: Accept events from multiple sources

**Implementation**:
```rust
pub struct IngestHandler {
    event_sender: mpsc::Sender<FeedbackEvent>,
    validator: Arc<EventValidator>,
    metrics: Arc<TelemetryMetrics>,
}

impl IngestHandler {
    /// Accept event via HTTP/gRPC API
    pub async fn accept_event(&self, event: FeedbackEvent) -> Result<(), IngestError> {
        // 1. Validate event
        self.validator.validate(&event)?;

        // 2. Record ingestion metric
        self.metrics.record_event_ingested(&event);

        // 3. Send to buffer (non-blocking)
        self.event_sender.try_send(event)
            .map_err(|e| IngestError::BufferFull)?;

        Ok(())
    }
}
```

**Error Handling**:
- Reject invalid events with 400 Bad Request
- Return 503 Service Unavailable if buffer is full
- Implement rate limiting per client

#### 2.2.2 EventValidator

**Responsibility**: Validate event schema and constraints

**Validation Rules**:
- Required fields present
- Timestamps within acceptable range
- Numeric values within bounds
- String lengths within limits
- Event type matches schema

**Implementation**:
```rust
pub struct EventValidator {
    schema_registry: Arc<SchemaRegistry>,
}

impl EventValidator {
    pub fn validate(&self, event: &FeedbackEvent) -> Result<(), ValidationError> {
        match event {
            FeedbackEvent::UserFeedback(e) => self.validate_user_feedback(e),
            FeedbackEvent::PerformanceMetrics(e) => self.validate_performance_metrics(e),
            // ... other event types
        }
    }

    fn validate_user_feedback(&self, event: &UserFeedbackEvent) -> Result<(), ValidationError> {
        // Validate rating range
        if let Some(rating) = event.rating {
            if rating < 0.0 || rating > 5.0 {
                return Err(ValidationError::InvalidRating(rating));
            }
        }

        // Validate session ID format
        if event.session_id.is_empty() {
            return Err(ValidationError::MissingSessionId);
        }

        Ok(())
    }
}
```

#### 2.2.3 RingBuffer

**Responsibility**: Bounded in-memory queue for event buffering

**Implementation**:
```rust
pub struct RingBuffer {
    sender: mpsc::Sender<FeedbackEvent>,
    receiver: mpsc::Receiver<FeedbackEvent>,
    capacity: usize,
    metrics: Arc<TelemetryMetrics>,
}

impl RingBuffer {
    pub fn new(capacity: usize, metrics: Arc<TelemetryMetrics>) -> Self {
        let (sender, receiver) = mpsc::channel(capacity);
        Self { sender, receiver, capacity, metrics }
    }

    /// Get sender for producers
    pub fn sender(&self) -> mpsc::Sender<FeedbackEvent> {
        self.sender.clone()
    }

    /// Get receiver for consumers
    pub fn receiver(&mut self) -> &mut mpsc::Receiver<FeedbackEvent> {
        &mut self.receiver
    }
}
```

**Characteristics**:
- Tokio mpsc channel (multi-producer, single-consumer)
- Default capacity: 10,000 events
- Backpressure: Rejects new events when full
- Memory bounded: ~10MB for 10K events

#### 2.2.4 BatchingEngine

**Responsibility**: Aggregate events into batches for efficient Kafka publishing

**Batching Strategy**:
- **Size-based**: Flush when batch reaches 100 events
- **Time-based**: Flush when oldest event is >10 seconds old
- **Shutdown**: Flush all pending events on graceful shutdown

**Implementation**:
```rust
pub struct BatchingEngine {
    batch: Vec<FeedbackEvent>,
    batch_created_at: Instant,
    max_batch_size: usize,
    max_batch_age: Duration,
    producer: Arc<KafkaProducer>,
    metrics: Arc<TelemetryMetrics>,
}

impl BatchingEngine {
    pub async fn run(&mut self, mut receiver: mpsc::Receiver<FeedbackEvent>) {
        let mut flush_timer = interval(Duration::from_secs(1));

        loop {
            tokio::select! {
                // Receive event
                Some(event) = receiver.recv() => {
                    self.batch.push(event);

                    if self.batch.len() >= self.max_batch_size {
                        self.flush_batch().await;
                    }
                }

                // Periodic flush check
                _ = flush_timer.tick() => {
                    if self.should_flush() {
                        self.flush_batch().await;
                    }
                }

                // Channel closed
                else => {
                    self.flush_batch().await;
                    break;
                }
            }
        }
    }

    fn should_flush(&self) -> bool {
        !self.batch.is_empty() &&
        self.batch_created_at.elapsed() >= self.max_batch_age
    }

    async fn flush_batch(&mut self) {
        if self.batch.is_empty() {
            return;
        }

        let start = Instant::now();
        let batch = FeedbackEventBatch::new(self.batch.drain(..).collect());

        match self.producer.publish_batch(&batch).await {
            Ok(_) => {
                self.metrics.record_batch_published(batch.size(), start.elapsed());
            }
            Err(e) => {
                self.metrics.record_batch_failed(batch.size());
                error!("Failed to publish batch: {}", e);
            }
        }

        self.batch_created_at = Instant::now();
    }
}
```

#### 2.2.5 KafkaProducer

**Responsibility**: Publish events to Kafka with reliability guarantees

**Configuration**:
```rust
pub struct KafkaProducerConfig {
    pub brokers: String,                    // "kafka1:9092,kafka2:9092,kafka3:9092"
    pub topic: String,                      // "feedback-events"
    pub client_id: String,                  // "llm-optimizer-collector"
    pub message_timeout_ms: u64,            // 30000 (30 seconds)
    pub compression_type: String,           // "gzip"
    pub retries: u32,                       // 3
    pub retry_backoff_ms: u64,              // 1000 (1 second)
    pub acks: String,                       // "all" (wait for all replicas)
    pub enable_idempotence: bool,           // true
    pub max_in_flight_requests: u32,        // 5
    pub linger_ms: u64,                     // 10 (batch for 10ms)
    pub batch_size: usize,                  // 16384 (16KB)
}
```

**Key Features**:
- **Idempotent Writes**: Prevent duplicates on retry
- **Exactly-Once Semantics**: Transactions for critical events
- **Compression**: Gzip compression (60-80% size reduction)
- **Async Publishing**: Non-blocking with Future-based API
- **Automatic Retries**: Exponential backoff up to 3 retries

**Implementation**:
```rust
pub struct KafkaProducer {
    producer: FutureProducer,
    config: KafkaProducerConfig,
    metrics: Arc<TelemetryMetrics>,
}

impl KafkaProducer {
    pub async fn publish_batch(&self, batch: &FeedbackEventBatch) -> Result<(), KafkaError> {
        let mut futures = Vec::with_capacity(batch.events.len());

        for event in &batch.events {
            let future = self.publish_event(event);
            futures.push(future);
        }

        // Wait for all publishes to complete
        let results = futures::future::join_all(futures).await;

        // Count successes and failures
        let (successes, failures): (Vec<_>, Vec<_>) =
            results.into_iter().partition(Result::is_ok);

        self.metrics.record_kafka_publish_results(successes.len(), failures.len());

        if !failures.is_empty() {
            return Err(KafkaError::PartialFailure(failures.len()));
        }

        Ok(())
    }

    async fn publish_event(&self, event: &FeedbackEvent) -> Result<(), KafkaError> {
        let payload = serde_json::to_vec(event)?;
        let key = event.id().to_string();

        let record = FutureRecord::to(&self.config.topic)
            .payload(&payload)
            .key(&key)
            .headers(self.create_headers(event));

        let timeout = Timeout::After(Duration::from_millis(
            self.config.message_timeout_ms
        ));

        self.producer
            .send(record, timeout)
            .await
            .map_err(|(err, _)| KafkaError::PublishError(err))?;

        Ok(())
    }

    fn create_headers(&self, event: &FeedbackEvent) -> OwnedHeaders {
        OwnedHeaders::new()
            .insert(Header { key: "event_type", value: Some(event.event_type()) })
            .insert(Header { key: "event_id", value: Some(event.id().as_bytes()) })
            .insert(Header { key: "timestamp", value: Some(event.timestamp().to_rfc3339().as_bytes()) })
            .insert(Header { key: "schema_version", value: Some(b"1.0") })
    }
}
```

#### 2.2.6 TelemetryMetrics

**Responsibility**: Collect and export observability metrics

**Metrics Collected**:
```rust
pub struct TelemetryMetrics {
    // Counter metrics
    events_ingested_total: Counter<u64>,
    events_validated_total: Counter<u64>,
    events_rejected_total: Counter<u64>,
    events_published_total: Counter<u64>,
    events_failed_total: Counter<u64>,
    batches_published_total: Counter<u64>,
    kafka_retries_total: Counter<u64>,

    // Gauge metrics
    buffer_size_current: Gauge<u64>,
    active_collectors: Gauge<u64>,

    // Histogram metrics
    ingestion_latency_ms: Histogram<f64>,
    batch_size: Histogram<u64>,
    batch_age_ms: Histogram<f64>,
    kafka_publish_latency_ms: Histogram<f64>,

    // Labels
    event_type_label: String,
    status_label: String,
}
```

**OpenTelemetry Integration**:
```rust
impl TelemetryMetrics {
    pub fn new(meter: Meter) -> Self {
        let events_ingested_total = meter
            .u64_counter("feedback_collector.events_ingested")
            .with_description("Total number of events ingested")
            .init();

        let ingestion_latency_ms = meter
            .f64_histogram("feedback_collector.ingestion_latency_ms")
            .with_description("Event ingestion latency in milliseconds")
            .init();

        // ... initialize other metrics

        Self {
            events_ingested_total,
            ingestion_latency_ms,
            // ... other metrics
        }
    }

    pub fn record_event_ingested(&self, event: &FeedbackEvent) {
        let labels = [KeyValue::new("event_type", event.event_type())];
        self.events_ingested_total.add(1, &labels);
    }

    pub fn record_ingestion_latency(&self, latency_ms: f64, event_type: &str) {
        let labels = [KeyValue::new("event_type", event_type)];
        self.ingestion_latency_ms.record(latency_ms, &labels);
    }
}
```

---

## 3. Data Models

### 3.1 Event Type Hierarchy

```rust
/// Top-level feedback event enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FeedbackEvent {
    UserFeedback(UserFeedbackEvent),
    PerformanceMetrics(PerformanceMetricsEvent),
    ModelResponse(ModelResponseEvent),
    SystemHealth(SystemHealthEvent),
    ExperimentResult(ExperimentResultEvent),
}

impl FeedbackEvent {
    pub fn id(&self) -> &Uuid {
        match self {
            Self::UserFeedback(e) => &e.event_id,
            Self::PerformanceMetrics(e) => &e.event_id,
            Self::ModelResponse(e) => &e.event_id,
            Self::SystemHealth(e) => &e.event_id,
            Self::ExperimentResult(e) => &e.event_id,
        }
    }

    pub fn timestamp(&self) -> &DateTime<Utc> {
        match self {
            Self::UserFeedback(e) => &e.timestamp,
            Self::PerformanceMetrics(e) => &e.timestamp,
            Self::ModelResponse(e) => &e.timestamp,
            Self::SystemHealth(e) => &e.timestamp,
            Self::ExperimentResult(e) => &e.timestamp,
        }
    }

    pub fn event_type(&self) -> &str {
        match self {
            Self::UserFeedback(_) => "user_feedback",
            Self::PerformanceMetrics(_) => "performance_metrics",
            Self::ModelResponse(_) => "model_response",
            Self::SystemHealth(_) => "system_health",
            Self::ExperimentResult(_) => "experiment_result",
        }
    }
}
```

### 3.2 User Feedback Event

```rust
/// User feedback event
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserFeedbackEvent {
    /// Event ID
    pub event_id: Uuid,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Session ID
    pub session_id: String,
    /// Request ID
    pub request_id: Uuid,
    /// User ID (optional)
    pub user_id: Option<String>,
    /// Rating (0-5 scale)
    pub rating: Option<f64>,
    /// Thumbs up/down
    pub thumbs: Option<ThumbsFeedback>,
    /// Textual feedback
    pub comment: Option<String>,
    /// Feedback tags
    pub tags: Vec<String>,
    /// Custom metadata
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThumbsFeedback {
    Up,
    Down,
}
```

### 3.3 Performance Metrics Event

```rust
/// Performance metrics event
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerformanceMetricsEvent {
    /// Event ID
    pub event_id: Uuid,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Request ID
    pub request_id: Uuid,
    /// Model ID
    pub model_id: String,
    /// Provider (e.g., "anthropic", "openai")
    pub provider: String,
    /// Latency in milliseconds
    pub latency_ms: f64,
    /// Input tokens
    pub tokens_input: u64,
    /// Output tokens
    pub tokens_output: u64,
    /// Total tokens
    pub tokens_total: u64,
    /// Cost in USD
    pub cost_usd: f64,
    /// Success flag
    pub success: bool,
    /// Error type (if failed)
    pub error_type: Option<String>,
    /// Cache hit
    pub cache_hit: bool,
    /// Additional metrics
    pub custom_metrics: HashMap<String, f64>,
}
```

### 3.4 Model Response Event

```rust
/// Model response event
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelResponseEvent {
    /// Event ID
    pub event_id: Uuid,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Request ID
    pub request_id: Uuid,
    /// Model ID
    pub model_id: String,
    /// Response quality score (0-100)
    pub quality_score: Option<f64>,
    /// Response quality dimensions
    pub quality_dimensions: QualityDimensions,
    /// Prompt hash (for deduplication)
    pub prompt_hash: String,
    /// Response length (characters)
    pub response_length: usize,
    /// Finish reason
    pub finish_reason: String,
    /// Model parameters used
    pub model_params: ModelParameters,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QualityDimensions {
    pub accuracy: Option<f64>,
    pub relevance: Option<f64>,
    pub coherence: Option<f64>,
    pub completeness: Option<f64>,
    pub safety: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelParameters {
    pub temperature: f64,
    pub top_p: f64,
    pub max_tokens: u64,
    pub top_k: Option<u64>,
}
```

### 3.5 System Health Event

```rust
/// System health event
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SystemHealthEvent {
    /// Event ID
    pub event_id: Uuid,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Service name
    pub service_name: String,
    /// Health status
    pub status: HealthStatus,
    /// Error rate (0-1)
    pub error_rate: f64,
    /// Availability (0-1)
    pub availability: f64,
    /// Resource utilization
    pub resources: ResourceUtilization,
    /// Active requests
    pub active_requests: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceUtilization {
    pub cpu_percent: f64,
    pub memory_mb: f64,
    pub memory_percent: f64,
    pub network_mbps: f64,
}
```

### 3.6 Experiment Result Event

```rust
/// Experiment result event
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExperimentResultEvent {
    /// Event ID
    pub event_id: Uuid,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Experiment ID
    pub experiment_id: Uuid,
    /// Experiment type
    pub experiment_type: ExperimentType,
    /// Variant ID
    pub variant_id: String,
    /// Request ID
    pub request_id: Uuid,
    /// Outcome/reward
    pub reward: f64,
    /// Metrics collected
    pub metrics: ExperimentMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentType {
    ABTest,
    MultiArmedBandit,
    ParameterTuning,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExperimentMetrics {
    pub cost: f64,
    pub quality: f64,
    pub latency_ms: f64,
    pub user_satisfaction: Option<f64>,
}
```

### 3.7 Feedback Event Batch

```rust
/// Batch of feedback events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackEventBatch {
    /// Batch ID
    pub batch_id: Uuid,
    /// Batch creation timestamp
    pub created_at: DateTime<Utc>,
    /// Events in batch
    pub events: Vec<FeedbackEvent>,
    /// Batch metadata
    pub metadata: BatchMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchMetadata {
    /// Collector instance ID
    pub collector_id: String,
    /// Collector version
    pub collector_version: String,
    /// Batch size
    pub size: usize,
    /// Batch compression
    pub compression: Option<String>,
}

impl FeedbackEventBatch {
    pub fn new(events: Vec<FeedbackEvent>) -> Self {
        Self {
            batch_id: Uuid::new_v4(),
            created_at: Utc::now(),
            size: events.len(),
            events,
            metadata: BatchMetadata {
                collector_id: hostname::get()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                collector_version: env!("CARGO_PKG_VERSION").to_string(),
                size: events.len(),
                compression: Some("gzip".to_string()),
            },
        }
    }

    pub fn size(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}
```

---

## 4. Technology Stack

### 4.1 Core Dependencies

```toml
[dependencies]
# Async Runtime
tokio = { version = "1.40", features = ["full"] }
tokio-stream = "0.1"
async-trait = "0.1"
futures = "0.3"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Observability - OpenTelemetry
opentelemetry = { version = "0.24", features = ["metrics", "trace"] }
opentelemetry-otlp = { version = "0.17", features = ["metrics", "trace", "grpc-tonic"] }
opentelemetry_sdk = { version = "0.24", features = ["rt-tokio", "metrics", "trace"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
tracing-opentelemetry = "0.25"

# Kafka Client
rdkafka = { version = "0.36", features = ["tokio", "ssl", "sasl"] }

# Utilities
uuid = { version = "1.10", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1.0"
anyhow = "1.0"
dashmap = "6.0"

# Configuration
figment = { version = "0.10", features = ["toml", "yaml", "env"] }

# Monitoring
prometheus-client = "0.22"
```

### 4.2 Version Requirements

- **Rust**: 1.75+ (2021 edition)
- **OpenTelemetry**: 0.24.x (latest stable)
- **rdkafka**: 0.36.x (wraps librdkafka 2.3+)
- **Tokio**: 1.40+ (async runtime)

### 4.3 External Dependencies

- **Kafka Cluster**: 3.6+ (minimum 3 brokers for production)
- **OpenTelemetry Collector**: 0.90+ (for metrics/traces aggregation)
- **Prometheus**: 2.47+ (metrics storage)
- **Jaeger**: 1.51+ (distributed tracing backend)

---

## 5. OpenTelemetry Integration

### 5.1 Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                    Application Code                          │
│  ┌────────────────────────────────────────────────────────┐  │
│  │          Feedback Collector                            │  │
│  │  (instrumented with OpenTelemetry SDK)                 │  │
│  └────────────────────────────────────────────────────────┘  │
│                          │                                    │
│                          ▼                                    │
│  ┌────────────────────────────────────────────────────────┐  │
│  │          OpenTelemetry SDK                             │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌─────────────┐  │  │
│  │  │   Metrics    │  │   Traces     │  │    Logs     │  │  │
│  │  │   Provider   │  │   Provider   │  │   Provider  │  │  │
│  │  └──────┬───────┘  └──────┬───────┘  └──────┬──────┘  │  │
│  └─────────┼──────────────────┼──────────────────┼─────────┘  │
│            │                  │                  │            │
│            │                  │                  │            │
│  ┌─────────▼──────────────────▼──────────────────▼─────────┐  │
│  │          OTLP Exporter (gRPC)                           │  │
│  │  - Batch processor                                       │  │
│  │  - Periodic export (10s)                                 │  │
│  │  - Compression: gzip                                     │  │
│  └────────────────────────────────────────────────────────┘  │
└──────────────────────────┼───────────────────────────────────┘
                           │
                           ▼
           ┌───────────────────────────────┐
           │  OpenTelemetry Collector      │
           │  - Receive: OTLP/gRPC (4317)  │
           │  - Process: Filter, enrich    │
           │  - Export: Prometheus, Jaeger │
           └───────────────┬───────────────┘
                           │
           ┌───────────────┴───────────────┐
           │                               │
           ▼                               ▼
    ┌──────────────┐              ┌──────────────┐
    │  Prometheus  │              │    Jaeger    │
    │  (Metrics)   │              │   (Traces)   │
    └──────────────┘              └──────────────┘
```

### 5.2 Initialization

```rust
use opentelemetry::{global, KeyValue};
use opentelemetry_sdk::{
    metrics::{PeriodicReader, SdkMeterProvider},
    trace::{BatchConfig, TracerProvider},
    Resource,
};
use opentelemetry_otlp::{
    WithExportConfig, WithGrpcConfig,
};

pub struct TelemetryProvider {
    meter_provider: SdkMeterProvider,
    tracer_provider: TracerProvider,
}

impl TelemetryProvider {
    pub fn init(config: TelemetryConfig) -> Result<Self, TelemetryError> {
        // Create resource with service attributes
        let resource = Resource::new(vec![
            KeyValue::new("service.name", "llm-optimizer-collector"),
            KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
            KeyValue::new("service.namespace", config.namespace),
            KeyValue::new("deployment.environment", config.environment),
        ]);

        // Initialize metrics provider
        let meter_provider = Self::init_metrics(&resource, &config)?;

        // Initialize trace provider
        let tracer_provider = Self::init_traces(&resource, &config)?;

        // Set global providers
        global::set_meter_provider(meter_provider.clone());
        global::set_tracer_provider(tracer_provider.clone());

        Ok(Self {
            meter_provider,
            tracer_provider,
        })
    }

    fn init_metrics(
        resource: &Resource,
        config: &TelemetryConfig,
    ) -> Result<SdkMeterProvider, TelemetryError> {
        let exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint(&config.otlp_endpoint)
            .with_timeout(Duration::from_secs(10))
            .with_compression(opentelemetry_otlp::Compression::Gzip)
            .build_metrics_exporter(
                Box::new(opentelemetry_sdk::metrics::aggregation::default_temporality_selector()),
                Box::new(opentelemetry_sdk::metrics::data::DeltaAggregation),
            )?;

        let reader = PeriodicReader::builder(exporter)
            .with_interval(Duration::from_secs(config.export_interval_secs))
            .build();

        let provider = SdkMeterProvider::builder()
            .with_resource(resource.clone())
            .with_reader(reader)
            .build();

        Ok(provider)
    }

    fn init_traces(
        resource: &Resource,
        config: &TelemetryConfig,
    ) -> Result<TracerProvider, TelemetryError> {
        let exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint(&config.otlp_endpoint)
            .with_timeout(Duration::from_secs(10))
            .with_compression(opentelemetry_otlp::Compression::Gzip)
            .build_span_exporter()?;

        let batch_config = BatchConfig::default()
            .with_max_queue_size(2048)
            .with_max_export_batch_size(512)
            .with_scheduled_delay(Duration::from_secs(5));

        let provider = TracerProvider::builder()
            .with_resource(resource.clone())
            .with_batch_exporter(exporter, batch_config)
            .with_sampler(opentelemetry_sdk::trace::Sampler::ParentBased(
                Box::new(opentelemetry_sdk::trace::Sampler::TraceIdRatioBased(0.1))
            ))
            .build();

        Ok(provider)
    }

    pub fn metrics(&self) -> Arc<TelemetryMetrics> {
        let meter = global::meter("llm-optimizer-collector");
        Arc::new(TelemetryMetrics::new(meter))
    }

    pub fn shutdown(&self) -> Result<(), TelemetryError> {
        // Flush all pending metrics and traces
        global::shutdown_tracer_provider();
        Ok(())
    }
}
```

### 5.3 Metrics Collection

**Key Metrics**:

```rust
pub struct TelemetryMetrics {
    // Counters
    pub events_ingested_total: Counter<u64>,
    pub events_published_total: Counter<u64>,
    pub events_failed_total: Counter<u64>,
    pub kafka_retries_total: Counter<u64>,

    // Gauges
    pub buffer_size_current: Gauge<u64>,
    pub active_collectors: Gauge<u64>,

    // Histograms
    pub ingestion_latency_ms: Histogram<f64>,
    pub batch_size: Histogram<u64>,
    pub kafka_publish_latency_ms: Histogram<f64>,
}

impl TelemetryMetrics {
    pub fn record_event_ingested(&self, event: &FeedbackEvent) {
        let labels = [
            KeyValue::new("event_type", event.event_type()),
        ];
        self.events_ingested_total.add(1, &labels);
    }

    pub fn record_batch_published(&self, size: usize, latency: Duration) {
        self.events_published_total.add(size as u64, &[]);
        self.batch_size.record(size as u64, &[]);
        self.kafka_publish_latency_ms.record(
            latency.as_secs_f64() * 1000.0,
            &[KeyValue::new("status", "success")]
        );
    }

    pub fn record_kafka_retry(&self, attempt: u32) {
        self.kafka_retries_total.add(1, &[
            KeyValue::new("attempt", attempt.to_string())
        ]);
    }
}
```

### 5.4 Distributed Tracing

**Trace Spans**:

```rust
use opentelemetry::trace::{Tracer, SpanKind, Status};
use tracing::{instrument, info_span};

impl FeedbackCollector {
    #[instrument(skip(self, event), fields(
        event_id = %event.id(),
        event_type = event.event_type()
    ))]
    pub async fn submit(&self, event: FeedbackEvent) -> Result<(), IngestError> {
        let tracer = global::tracer("feedback_collector");
        let span = tracer
            .span_builder("submit_event")
            .with_kind(SpanKind::Internal)
            .start(&tracer);

        let _guard = span.enter();

        // Validate event
        let validate_span = tracer
            .span_builder("validate_event")
            .with_kind(SpanKind::Internal)
            .start(&tracer);

        self.validator.validate(&event)?;
        validate_span.end();

        // Send to buffer
        let buffer_span = tracer
            .span_builder("send_to_buffer")
            .with_kind(SpanKind::Internal)
            .start(&tracer);

        self.event_sender.send(event).await?;
        buffer_span.set_status(Status::Ok);
        buffer_span.end();

        span.set_status(Status::Ok);
        span.end();

        Ok(())
    }
}
```

### 5.5 Graceful Shutdown

```rust
impl Drop for TelemetryProvider {
    fn drop(&mut self) {
        // Ensure all pending telemetry is flushed
        let _ = self.shutdown();
    }
}

// Application shutdown
async fn shutdown_gracefully(collector: FeedbackCollector, telemetry: TelemetryProvider) {
    info!("Initiating graceful shutdown");

    // Stop accepting new events
    collector.stop_ingestion().await;

    // Flush remaining events
    collector.flush_remaining_events().await;

    // Shutdown Kafka producer
    collector.shutdown().await.expect("Failed to shutdown collector");

    // Flush telemetry
    telemetry.shutdown().expect("Failed to shutdown telemetry");

    info!("Graceful shutdown complete");
}
```

---

## 6. Kafka Integration

### 6.1 Producer Configuration

**Optimal Settings for Production**:

```rust
pub struct KafkaProducerConfig {
    // Connection
    pub brokers: String,                        // "kafka1:9092,kafka2:9092,kafka3:9092"
    pub topic: String,                          // "feedback-events"
    pub client_id: String,                      // "llm-optimizer-collector-{instance}"

    // Reliability
    pub acks: String,                           // "all" (wait for all in-sync replicas)
    pub enable_idempotence: bool,               // true (prevent duplicates)
    pub max_in_flight_requests: u32,            // 5 (balance throughput and ordering)
    pub retries: u32,                           // 3 (automatic retries)
    pub retry_backoff_ms: u64,                  // 1000 (exponential backoff)
    pub message_timeout_ms: u64,                // 30000 (30 seconds)

    // Performance
    pub compression_type: String,               // "gzip" (60-80% compression)
    pub linger_ms: u64,                         // 10 (batch for 10ms)
    pub batch_size: usize,                      // 16384 (16KB batches)
    pub buffer_memory: usize,                   // 33554432 (32MB buffer)

    // Partitioning
    pub partitioner: String,                    // "consistent_random" or "murmur2"

    // Security (optional)
    pub security_protocol: Option<String>,      // "SASL_SSL"
    pub sasl_mechanism: Option<String>,         // "SCRAM-SHA-512"
    pub sasl_username: Option<String>,
    pub sasl_password: Option<String>,
    pub ssl_ca_location: Option<String>,
}

impl Default for KafkaProducerConfig {
    fn default() -> Self {
        Self {
            brokers: "localhost:9092".to_string(),
            topic: "feedback-events".to_string(),
            client_id: format!("llm-optimizer-collector-{}", hostname::get()
                .unwrap_or_default()
                .to_string_lossy()),
            acks: "all".to_string(),
            enable_idempotence: true,
            max_in_flight_requests: 5,
            retries: 3,
            retry_backoff_ms: 1000,
            message_timeout_ms: 30000,
            compression_type: "gzip".to_string(),
            linger_ms: 10,
            batch_size: 16384,
            buffer_memory: 33554432,
            partitioner: "consistent_random".to_string(),
            security_protocol: None,
            sasl_mechanism: None,
            sasl_username: None,
            sasl_password: None,
            ssl_ca_location: None,
        }
    }
}
```

### 6.2 Topic Configuration

**Recommended Kafka Topic Settings**:

```bash
kafka-topics.sh --create \
  --topic feedback-events \
  --partitions 12 \
  --replication-factor 3 \
  --config retention.ms=604800000 \
  --config compression.type=producer \
  --config min.insync.replicas=2 \
  --config cleanup.policy=delete \
  --config segment.ms=3600000
```

**Explanation**:
- **Partitions**: 12 (allows up to 12 concurrent consumers)
- **Replication Factor**: 3 (high availability)
- **Retention**: 7 days (604800000 ms)
- **Min In-Sync Replicas**: 2 (durability)
- **Compression**: Delegated to producer
- **Segment**: 1 hour (3600000 ms)

### 6.3 Partitioning Strategy

**Event Key Selection**:

```rust
impl FeedbackEvent {
    /// Get partition key for Kafka
    pub fn partition_key(&self) -> String {
        match self {
            // Partition by session for user feedback (maintain order per session)
            Self::UserFeedback(e) => e.session_id.clone(),

            // Partition by request_id for performance metrics
            Self::PerformanceMetrics(e) => e.request_id.to_string(),

            // Partition by request_id for model responses
            Self::ModelResponse(e) => e.request_id.to_string(),

            // Partition by service name for system health
            Self::SystemHealth(e) => e.service_name.clone(),

            // Partition by experiment_id for experiments
            Self::ExperimentResult(e) => e.experiment_id.to_string(),
        }
    }
}
```

**Benefits**:
- **Session Ordering**: User feedback events for same session go to same partition
- **Request Correlation**: All events for a request are co-located
- **Load Distribution**: Random distribution across partitions
- **Parallelism**: Enable parallel processing downstream

### 6.4 Error Handling & Retries

**Retry Strategy**:

```rust
pub struct KafkaRetryPolicy {
    pub max_retries: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub multiplier: f64,
}

impl Default for KafkaRetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff_ms: 1000,     // 1 second
            max_backoff_ms: 30000,         // 30 seconds
            multiplier: 2.0,               // Exponential backoff
        }
    }
}

impl KafkaProducer {
    async fn publish_with_retry(
        &self,
        event: &FeedbackEvent,
        policy: &KafkaRetryPolicy,
    ) -> Result<(), KafkaError> {
        let mut attempt = 0;
        let mut backoff_ms = policy.initial_backoff_ms;

        loop {
            match self.publish_event(event).await {
                Ok(_) => return Ok(()),
                Err(e) if Self::is_retryable(&e) && attempt < policy.max_retries => {
                    attempt += 1;
                    warn!("Kafka publish failed (attempt {}): {}", attempt, e);

                    self.metrics.record_kafka_retry(attempt);

                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;

                    backoff_ms = (backoff_ms as f64 * policy.multiplier) as u64;
                    backoff_ms = backoff_ms.min(policy.max_backoff_ms);
                }
                Err(e) => {
                    error!("Kafka publish failed permanently: {}", e);
                    return Err(e);
                }
            }
        }
    }

    fn is_retryable(error: &KafkaError) -> bool {
        match error {
            KafkaError::TimeoutError(_) => true,
            KafkaError::PublishError(msg) if msg.contains("broker") => true,
            KafkaError::PublishError(msg) if msg.contains("network") => true,
            _ => false,
        }
    }
}
```

**Dead Letter Queue (DLQ)**:

```rust
pub struct DeadLetterQueue {
    dlq_topic: String,
    producer: Arc<FutureProducer>,
}

impl DeadLetterQueue {
    pub async fn send(&self, event: &FeedbackEvent, error: &KafkaError) -> Result<(), KafkaError> {
        let dlq_payload = DeadLetterPayload {
            original_event: event.clone(),
            error_message: error.to_string(),
            failed_at: Utc::now(),
            retry_count: 0,
        };

        let payload = serde_json::to_vec(&dlq_payload)?;

        let record = FutureRecord::to(&self.dlq_topic)
            .payload(&payload)
            .key(&event.id().to_string());

        self.producer
            .send(record, Timeout::After(Duration::from_secs(10)))
            .await
            .map_err(|(err, _)| KafkaError::PublishError(err.to_string()))?;

        Ok(())
    }
}
```

### 6.5 Consumer Considerations

**Downstream Consumer Configuration**:

```rust
// Example consumer configuration (for Stream Processor)
pub struct KafkaConsumerConfig {
    pub brokers: String,
    pub group_id: String,
    pub topics: Vec<String>,

    // Offset management
    pub auto_offset_reset: String,           // "earliest" or "latest"
    pub enable_auto_commit: bool,            // false (manual commit for exactly-once)
    pub auto_commit_interval_ms: u64,        // 5000 (if auto-commit enabled)

    // Session management
    pub session_timeout_ms: u64,             // 30000 (30 seconds)
    pub heartbeat_interval_ms: u64,          // 3000 (3 seconds)

    // Performance
    pub fetch_min_bytes: usize,              // 1024 (1KB)
    pub fetch_max_wait_ms: u64,              // 500 (500ms)
    pub max_partition_fetch_bytes: usize,    // 1048576 (1MB)

    // Isolation level
    pub isolation_level: String,             // "read_committed" for exactly-once
}
```

### 6.6 Monitoring Kafka

**Kafka-Specific Metrics**:

```rust
pub struct KafkaMetrics {
    // Producer metrics
    pub kafka_produce_requests_total: Counter<u64>,
    pub kafka_produce_errors_total: Counter<u64>,
    pub kafka_produce_retries_total: Counter<u64>,
    pub kafka_produce_latency_ms: Histogram<f64>,
    pub kafka_batch_queue_size: Gauge<u64>,

    // Message metrics
    pub kafka_message_bytes_total: Counter<u64>,
    pub kafka_message_size_bytes: Histogram<u64>,
    pub kafka_compression_ratio: Histogram<f64>,

    // Connection metrics
    pub kafka_connection_errors_total: Counter<u64>,
    pub kafka_broker_reconnects_total: Counter<u64>,
}

impl KafkaMetrics {
    pub fn record_produce_success(&self, latency: Duration, size_bytes: usize) {
        self.kafka_produce_requests_total.add(1, &[
            KeyValue::new("status", "success")
        ]);
        self.kafka_produce_latency_ms.record(
            latency.as_secs_f64() * 1000.0,
            &[]
        );
        self.kafka_message_bytes_total.add(size_bytes as u64, &[]);
        self.kafka_message_size_bytes.record(size_bytes as u64, &[]);
    }

    pub fn record_produce_error(&self, error_type: &str) {
        self.kafka_produce_errors_total.add(1, &[
            KeyValue::new("error_type", error_type)
        ]);
    }
}
```

---

## 7. Error Handling Strategy

### 7.1 Error Classification

```rust
#[derive(Error, Debug)]
pub enum CollectorError {
    // Validation errors (4xx - client error)
    #[error("Invalid event: {0}")]
    InvalidEvent(#[from] ValidationError),

    #[error("Event too large: {size} bytes (max: {max})")]
    EventTooLarge { size: usize, max: usize },

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    // Resource errors (5xx - server error, retryable)
    #[error("Buffer full")]
    BufferFull,

    #[error("Kafka error: {0}")]
    KafkaError(#[from] KafkaError),

    #[error("Telemetry error: {0}")]
    TelemetryError(#[from] TelemetryError),

    // System errors (5xx - server error, non-retryable)
    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Shutdown in progress")]
    ShuttingDown,
}

impl CollectorError {
    /// Determine if error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::BufferFull => true,
            Self::KafkaError(e) => e.is_retryable(),
            Self::InvalidEvent(_) => false,
            Self::EventTooLarge { .. } => false,
            Self::RateLimitExceeded => true,
            Self::TelemetryError(_) => false,
            Self::InternalError(_) => false,
            Self::ShuttingDown => false,
        }
    }

    /// Get HTTP status code
    pub fn http_status(&self) -> u16 {
        match self {
            Self::InvalidEvent(_) => 400,
            Self::EventTooLarge { .. } => 413,
            Self::RateLimitExceeded => 429,
            Self::BufferFull => 503,
            Self::KafkaError(_) => 503,
            Self::InternalError(_) => 500,
            Self::ShuttingDown => 503,
            Self::TelemetryError(_) => 500,
        }
    }
}
```

### 7.2 Validation Error Handling

```rust
#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid field value: {field} = {value}")]
    InvalidValue { field: String, value: String },

    #[error("Field out of range: {field} must be between {min} and {max}")]
    OutOfRange { field: String, min: f64, max: f64 },

    #[error("Invalid format: {field}")]
    InvalidFormat { field: String },

    #[error("Schema validation failed: {0}")]
    SchemaViolation(String),
}

impl EventValidator {
    pub fn validate(&self, event: &FeedbackEvent) -> Result<(), ValidationError> {
        // Type-specific validation
        match event {
            FeedbackEvent::UserFeedback(e) => self.validate_user_feedback(e),
            FeedbackEvent::PerformanceMetrics(e) => self.validate_performance_metrics(e),
            FeedbackEvent::ModelResponse(e) => self.validate_model_response(e),
            FeedbackEvent::SystemHealth(e) => self.validate_system_health(e),
            FeedbackEvent::ExperimentResult(e) => self.validate_experiment_result(e),
        }
    }

    fn validate_user_feedback(&self, event: &UserFeedbackEvent) -> Result<(), ValidationError> {
        // Check session ID
        if event.session_id.is_empty() {
            return Err(ValidationError::MissingField("session_id".to_string()));
        }

        // Check rating range
        if let Some(rating) = event.rating {
            if rating < 0.0 || rating > 5.0 {
                return Err(ValidationError::OutOfRange {
                    field: "rating".to_string(),
                    min: 0.0,
                    max: 5.0,
                });
            }
        }

        // Check comment length
        if let Some(comment) = &event.comment {
            if comment.len() > 1000 {
                return Err(ValidationError::InvalidValue {
                    field: "comment".to_string(),
                    value: format!("length {} exceeds 1000", comment.len()),
                });
            }
        }

        Ok(())
    }

    fn validate_performance_metrics(&self, event: &PerformanceMetricsEvent) -> Result<(), ValidationError> {
        // Check latency is positive
        if event.latency_ms < 0.0 {
            return Err(ValidationError::InvalidValue {
                field: "latency_ms".to_string(),
                value: event.latency_ms.to_string(),
            });
        }

        // Check token counts are consistent
        if event.tokens_total != event.tokens_input + event.tokens_output {
            return Err(ValidationError::SchemaViolation(
                "tokens_total must equal tokens_input + tokens_output".to_string()
            ));
        }

        // Check model ID is not empty
        if event.model_id.is_empty() {
            return Err(ValidationError::MissingField("model_id".to_string()));
        }

        Ok(())
    }
}
```

### 7.3 Circuit Breaker Pattern

```rust
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,    // Normal operation
    Open,      // Failing, reject requests
    HalfOpen,  // Testing recovery
}

pub struct CircuitBreaker {
    state: AtomicU8,  // Represents CircuitState
    failure_count: AtomicU64,
    success_count: AtomicU64,
    last_failure_time: AtomicU64,

    // Configuration
    failure_threshold: u64,     // Open circuit after N failures
    success_threshold: u64,     // Close circuit after N successes in half-open
    timeout_ms: u64,            // Try half-open after timeout
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u64, success_threshold: u64, timeout_ms: u64) -> Self {
        Self {
            state: AtomicU8::new(CircuitState::Closed as u8),
            failure_count: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            last_failure_time: AtomicU64::new(0),
            failure_threshold,
            success_threshold,
            timeout_ms,
        }
    }

    pub fn state(&self) -> CircuitState {
        match self.state.load(Ordering::Relaxed) {
            0 => CircuitState::Closed,
            1 => CircuitState::Open,
            2 => CircuitState::HalfOpen,
            _ => CircuitState::Closed,
        }
    }

    pub fn call<F, T, E>(&self, f: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: FnOnce() -> Result<T, E>,
    {
        // Check circuit state
        match self.state() {
            CircuitState::Open => {
                // Check if timeout has elapsed
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;

                let last_failure = self.last_failure_time.load(Ordering::Relaxed);

                if now - last_failure > self.timeout_ms {
                    // Try half-open
                    self.state.store(CircuitState::HalfOpen as u8, Ordering::Relaxed);
                } else {
                    return Err(CircuitBreakerError::CircuitOpen);
                }
            }
            CircuitState::Closed | CircuitState::HalfOpen => {}
        }

        // Execute function
        match f() {
            Ok(result) => {
                self.on_success();
                Ok(result)
            }
            Err(e) => {
                self.on_failure();
                Err(CircuitBreakerError::CallFailed(e))
            }
        }
    }

    fn on_success(&self) {
        match self.state() {
            CircuitState::Closed => {
                // Reset failure count
                self.failure_count.store(0, Ordering::Relaxed);
            }
            CircuitState::HalfOpen => {
                let success_count = self.success_count.fetch_add(1, Ordering::Relaxed) + 1;

                if success_count >= self.success_threshold {
                    // Close circuit
                    self.state.store(CircuitState::Closed as u8, Ordering::Relaxed);
                    self.failure_count.store(0, Ordering::Relaxed);
                    self.success_count.store(0, Ordering::Relaxed);
                }
            }
            CircuitState::Open => {}
        }
    }

    fn on_failure(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        self.last_failure_time.store(now, Ordering::Relaxed);

        match self.state() {
            CircuitState::Closed => {
                let failure_count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;

                if failure_count >= self.failure_threshold {
                    // Open circuit
                    self.state.store(CircuitState::Open as u8, Ordering::Relaxed);
                }
            }
            CircuitState::HalfOpen => {
                // Back to open
                self.state.store(CircuitState::Open as u8, Ordering::Relaxed);
                self.success_count.store(0, Ordering::Relaxed);
            }
            CircuitState::Open => {}
        }
    }
}

#[derive(Error, Debug)]
pub enum CircuitBreakerError<E> {
    #[error("Circuit breaker is open")]
    CircuitOpen,

    #[error("Call failed: {0}")]
    CallFailed(E),
}
```

### 7.4 Fallback Strategies

**Graceful Degradation**:

```rust
pub struct FeedbackCollectorWithFallback {
    primary: FeedbackCollector,
    fallback_storage: Arc<dyn FallbackStorage>,
    circuit_breaker: Arc<CircuitBreaker>,
}

impl FeedbackCollectorWithFallback {
    pub async fn submit(&self, event: FeedbackEvent) -> Result<(), CollectorError> {
        // Try primary path with circuit breaker
        match self.circuit_breaker.call(|| {
            self.primary.try_submit(event.clone())
        }) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => {
                warn!("Primary collector failed: {}", e);
                self.fallback(event).await
            }
            Err(CircuitBreakerError::CircuitOpen) => {
                warn!("Circuit breaker open, using fallback");
                self.fallback(event).await
            }
            Err(CircuitBreakerError::CallFailed(e)) => {
                warn!("Primary collector failed: {:?}", e);
                self.fallback(event).await
            }
        }
    }

    async fn fallback(&self, event: FeedbackEvent) -> Result<(), CollectorError> {
        // Store in local fallback storage (disk, memory, etc.)
        self.fallback_storage.store(event).await
            .map_err(|e| CollectorError::InternalError(e.to_string()))?;

        // Attempt to recover later via background task
        self.schedule_recovery();

        Ok(())
    }

    fn schedule_recovery(&self) {
        // Background task will periodically retry failed events
    }
}

#[async_trait]
pub trait FallbackStorage: Send + Sync {
    async fn store(&self, event: FeedbackEvent) -> Result<(), Box<dyn std::error::Error>>;
    async fn retrieve_pending(&self) -> Result<Vec<FeedbackEvent>, Box<dyn std::error::Error>>;
    async fn mark_processed(&self, event_id: &Uuid) -> Result<(), Box<dyn std::error::Error>>;
}
```

---

## 8. Configuration Management

### 8.1 Configuration Schema

```rust
use serde::{Deserialize, Serialize};
use figment::{Figment, providers::{Format, Yaml, Env}};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackCollectorConfig {
    /// Service configuration
    pub service: ServiceConfig,

    /// Ingestion configuration
    pub ingestion: IngestionConfig,

    /// Buffer configuration
    pub buffer: BufferConfig,

    /// Batching configuration
    pub batching: BatchingConfig,

    /// Kafka configuration
    pub kafka: KafkaProducerConfig,

    /// Telemetry configuration
    pub telemetry: TelemetryConfig,

    /// Error handling configuration
    pub error_handling: ErrorHandlingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Service name
    pub name: String,
    /// Service namespace
    pub namespace: String,
    /// Deployment environment
    pub environment: String,
    /// Service version
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionConfig {
    /// Maximum event size in bytes
    pub max_event_size_bytes: usize,
    /// Rate limit per client (events/second)
    pub rate_limit_per_client: u32,
    /// Enable request validation
    pub enable_validation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferConfig {
    /// Buffer capacity (number of events)
    pub capacity: usize,
    /// Enable backpressure (reject when full)
    pub enable_backpressure: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchingConfig {
    /// Maximum batch size (events)
    pub max_batch_size: usize,
    /// Maximum batch age (seconds)
    pub max_batch_age_secs: u64,
    /// Flush interval (milliseconds)
    pub flush_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    /// OpenTelemetry Collector endpoint
    pub otlp_endpoint: String,
    /// Export interval (seconds)
    pub export_interval_secs: u64,
    /// Trace sampling rate (0.0-1.0)
    pub trace_sampling_rate: f64,
    /// Enable metrics
    pub enable_metrics: bool,
    /// Enable tracing
    pub enable_tracing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorHandlingConfig {
    /// Enable circuit breaker
    pub enable_circuit_breaker: bool,
    /// Circuit breaker failure threshold
    pub circuit_breaker_failure_threshold: u64,
    /// Circuit breaker timeout (milliseconds)
    pub circuit_breaker_timeout_ms: u64,
    /// Enable dead letter queue
    pub enable_dlq: bool,
    /// Dead letter queue topic
    pub dlq_topic: String,
}
```

### 8.2 Configuration Loading

```rust
impl FeedbackCollectorConfig {
    /// Load configuration from multiple sources
    pub fn load() -> Result<Self, figment::Error> {
        Figment::new()
            // 1. Default values
            .merge(Serialized::defaults(Self::default()))
            // 2. YAML config file
            .merge(Yaml::file("config/collector.yaml"))
            // 3. Environment variables (COLLECTOR_*)
            .merge(Env::prefixed("COLLECTOR_").split("__"))
            .extract()
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.buffer.capacity == 0 {
            return Err(ConfigError::InvalidValue {
                field: "buffer.capacity",
                reason: "must be greater than 0",
            });
        }

        if self.batching.max_batch_size > self.buffer.capacity {
            return Err(ConfigError::InvalidValue {
                field: "batching.max_batch_size",
                reason: "cannot exceed buffer capacity",
            });
        }

        if self.kafka.brokers.is_empty() {
            return Err(ConfigError::MissingValue {
                field: "kafka.brokers",
            });
        }

        if self.telemetry.trace_sampling_rate < 0.0 || self.telemetry.trace_sampling_rate > 1.0 {
            return Err(ConfigError::OutOfRange {
                field: "telemetry.trace_sampling_rate",
                min: 0.0,
                max: 1.0,
            });
        }

        Ok(())
    }
}

impl Default for FeedbackCollectorConfig {
    fn default() -> Self {
        Self {
            service: ServiceConfig {
                name: "llm-optimizer-collector".to_string(),
                namespace: "llm-optimizer".to_string(),
                environment: "development".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            ingestion: IngestionConfig {
                max_event_size_bytes: 1024 * 1024,  // 1MB
                rate_limit_per_client: 1000,         // 1000 events/sec
                enable_validation: true,
            },
            buffer: BufferConfig {
                capacity: 10000,
                enable_backpressure: true,
            },
            batching: BatchingConfig {
                max_batch_size: 100,
                max_batch_age_secs: 10,
                flush_interval_ms: 1000,
            },
            kafka: KafkaProducerConfig::default(),
            telemetry: TelemetryConfig {
                otlp_endpoint: "http://localhost:4317".to_string(),
                export_interval_secs: 10,
                trace_sampling_rate: 0.1,
                enable_metrics: true,
                enable_tracing: true,
            },
            error_handling: ErrorHandlingConfig {
                enable_circuit_breaker: true,
                circuit_breaker_failure_threshold: 5,
                circuit_breaker_timeout_ms: 30000,
                enable_dlq: true,
                dlq_topic: "feedback-events-dlq".to_string(),
            },
        }
    }
}
```

### 8.3 Example Configuration File

```yaml
# config/collector.yaml

service:
  name: "llm-optimizer-collector"
  namespace: "llm-optimizer"
  environment: "production"
  version: "0.1.0"

ingestion:
  max_event_size_bytes: 1048576  # 1MB
  rate_limit_per_client: 1000    # events/sec
  enable_validation: true

buffer:
  capacity: 10000
  enable_backpressure: true

batching:
  max_batch_size: 100
  max_batch_age_secs: 10
  flush_interval_ms: 1000

kafka:
  brokers: "kafka1:9092,kafka2:9092,kafka3:9092"
  topic: "feedback-events"
  client_id: "llm-optimizer-collector"
  acks: "all"
  enable_idempotence: true
  compression_type: "gzip"
  retries: 3
  retry_backoff_ms: 1000
  message_timeout_ms: 30000

  # Security (optional)
  security_protocol: "SASL_SSL"
  sasl_mechanism: "SCRAM-SHA-512"
  sasl_username: "${KAFKA_USERNAME}"
  sasl_password: "${KAFKA_PASSWORD}"
  ssl_ca_location: "/etc/kafka/ca-cert.pem"

telemetry:
  otlp_endpoint: "http://otel-collector:4317"
  export_interval_secs: 10
  trace_sampling_rate: 0.1
  enable_metrics: true
  enable_tracing: true

error_handling:
  enable_circuit_breaker: true
  circuit_breaker_failure_threshold: 5
  circuit_breaker_timeout_ms: 30000
  enable_dlq: true
  dlq_topic: "feedback-events-dlq"
```

---

## 9. Deployment Architecture

### 9.1 Deployment Modes

#### 9.1.1 Sidecar Pattern

```
┌────────────────────────────────────────────────────┐
│                  Kubernetes Pod                    │
├────────────────────────────────────────────────────┤
│                                                    │
│  ┌─────────────────┐     ┌──────────────────────┐ │
│  │                 │     │  Feedback Collector  │ │
│  │  Application    │────▶│  (Sidecar)           │ │
│  │  Container      │     │  localhost:9090      │ │
│  │                 │     └──────────┬───────────┘ │
│  └─────────────────┘                │             │
│                                     │             │
└─────────────────────────────────────┼─────────────┘
                                      │
                                      ▼
                              ┌───────────────┐
                              │ Kafka Cluster │
                              └───────────────┘
```

**Benefits**:
- Ultra-low latency (<1ms localhost communication)
- Automatic scaling with application
- Isolated failure domain
- Simple service discovery

**Deployment**:
```yaml
# k8s/deployment-sidecar.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: llm-application
spec:
  replicas: 3
  template:
    spec:
      containers:
      # Application container
      - name: app
        image: llm-application:1.0
        ports:
        - containerPort: 8080
        env:
        - name: COLLECTOR_ENDPOINT
          value: "http://localhost:9090"

      # Feedback Collector sidecar
      - name: feedback-collector
        image: llm-optimizer-collector:0.1.0
        ports:
        - containerPort: 9090
        env:
        - name: COLLECTOR_KAFKA__BROKERS
          value: "kafka:9092"
        - name: COLLECTOR_TELEMETRY__OTLP_ENDPOINT
          value: "http://otel-collector:4317"
        resources:
          requests:
            cpu: "100m"
            memory: "128Mi"
          limits:
            cpu: "500m"
            memory: "512Mi"
```

#### 9.1.2 Standalone Service Pattern

```
┌────────────────────────┐
│    Application Pod 1   │
└───────────┬────────────┘
            │
            │  HTTP/gRPC
            ▼
┌────────────────────────────────────┐
│  Feedback Collector Service        │
│  (Kubernetes Service + Deployment) │
│  ┌────────┐  ┌────────┐            │
│  │ Pod 1  │  │ Pod 2  │   ...      │
│  └───┬────┘  └───┬────┘            │
└──────┼───────────┼─────────────────┘
       │           │
       └─────┬─────┘
             ▼
     ┌───────────────┐
     │ Kafka Cluster │
     └───────────────┘
```

**Benefits**:
- Centralized management
- Independent scaling
- Shared resource pool
- Easier upgrades

**Deployment**:
```yaml
# k8s/deployment-standalone.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: feedback-collector
spec:
  replicas: 3
  selector:
    matchLabels:
      app: feedback-collector
  template:
    metadata:
      labels:
        app: feedback-collector
    spec:
      containers:
      - name: collector
        image: llm-optimizer-collector:0.1.0
        ports:
        - containerPort: 8080
          name: http
        - containerPort: 9090
          name: grpc
        env:
        - name: COLLECTOR_KAFKA__BROKERS
          valueFrom:
            configMapKeyRef:
              name: collector-config
              key: kafka.brokers
        resources:
          requests:
            cpu: "500m"
            memory: "512Mi"
          limits:
            cpu: "2000m"
            memory: "2Gi"
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
---
apiVersion: v1
kind: Service
metadata:
  name: feedback-collector
spec:
  selector:
    app: feedback-collector
  ports:
  - name: http
    port: 8080
    targetPort: 8080
  - name: grpc
    port: 9090
    targetPort: 9090
  type: ClusterIP
```

#### 9.1.3 Background Daemon Pattern

```
┌────────────────────────────────────┐
│  Application writes to disk/DB     │
└────────────┬───────────────────────┘
             │
             │ Periodic scan
             ▼
┌─────────────────────────────────┐
│  Feedback Collector DaemonSet   │
│  (runs on each node)            │
│  ┌─────────┐  ┌─────────┐       │
│  │ Node 1  │  │ Node 2  │  ...  │
│  └────┬────┘  └────┬────┘       │
└───────┼────────────┼────────────┘
        │            │
        └──────┬─────┘
               ▼
       ┌───────────────┐
       │ Kafka Cluster │
       └───────────────┘
```

**Benefits**:
- Batch processing efficiency
- Lower resource overhead
- Resilient to transient failures
- Cost-optimized

**Deployment**:
```yaml
# k8s/daemonset.yaml
apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: feedback-collector-daemon
spec:
  selector:
    matchLabels:
      app: feedback-collector-daemon
  template:
    metadata:
      labels:
        app: feedback-collector-daemon
    spec:
      containers:
      - name: collector
        image: llm-optimizer-collector:0.1.0
        env:
        - name: COLLECTOR_MODE
          value: "daemon"
        - name: COLLECTOR_SCAN_INTERVAL_SECS
          value: "60"
        volumeMounts:
        - name: event-storage
          mountPath: /var/lib/feedback-events
        resources:
          requests:
            cpu: "100m"
            memory: "128Mi"
          limits:
            cpu: "500m"
            memory: "512Mi"
      volumes:
      - name: event-storage
        hostPath:
          path: /var/lib/feedback-events
          type: DirectoryOrCreate
```

### 9.2 Horizontal Scaling

**Scaling Strategy**:

```yaml
# k8s/hpa.yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: feedback-collector-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: feedback-collector
  minReplicas: 3
  maxReplicas: 20
  metrics:
  # Scale based on CPU
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70

  # Scale based on memory
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80

  # Scale based on custom metric (buffer utilization)
  - type: Pods
    pods:
      metric:
        name: collector_buffer_utilization
      target:
        type: AverageValue
        averageValue: "0.7"

  # Scale based on request rate
  - type: Pods
    pods:
      metric:
        name: collector_ingestion_rate
      target:
        type: AverageValue
        averageValue: "5000"  # 5000 events/sec per pod

  behavior:
    scaleDown:
      stabilizationWindowSeconds: 300
      policies:
      - type: Percent
        value: 50
        periodSeconds: 60
    scaleUp:
      stabilizationWindowSeconds: 0
      policies:
      - type: Percent
        value: 100
        periodSeconds: 30
      - type: Pods
        value: 4
        periodSeconds: 60
      selectPolicy: Max
```

### 9.3 Resource Requirements

**Sizing Guidelines**:

| Deployment | CPU Request | CPU Limit | Memory Request | Memory Limit | Events/sec |
|------------|-------------|-----------|----------------|--------------|------------|
| Sidecar    | 100m        | 500m      | 128Mi          | 512Mi        | 1,000      |
| Small      | 500m        | 2000m     | 512Mi          | 2Gi          | 5,000      |
| Medium     | 1000m       | 4000m     | 1Gi            | 4Gi          | 10,000     |
| Large      | 2000m       | 8000m     | 2Gi            | 8Gi          | 20,000     |

**Storage Requirements**:
- **Logs**: ~1GB/day per instance (rotated daily)
- **Metrics**: Negligible (exported to Prometheus)
- **Fallback Storage**: 10GB for local DLQ (optional)

### 9.4 High Availability

**Multi-AZ Deployment**:

```yaml
# k8s/deployment-ha.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: feedback-collector
spec:
  replicas: 6  # 2 per AZ
  template:
    spec:
      # Anti-affinity: spread across nodes
      affinity:
        podAntiAffinity:
          preferredDuringSchedulingIgnoredDuringExecution:
          - weight: 100
            podAffinityTerm:
              labelSelector:
                matchExpressions:
                - key: app
                  operator: In
                  values:
                  - feedback-collector
              topologyKey: kubernetes.io/hostname

        # Node affinity: spread across AZs
        nodeAffinity:
          preferredDuringSchedulingIgnoredDuringExecution:
          - weight: 100
            preference:
              matchExpressions:
              - key: topology.kubernetes.io/zone
                operator: In
                values:
                - us-east-1a
                - us-east-1b
                - us-east-1c

      # Pod disruption budget
      topologySpreadConstraints:
      - maxSkew: 1
        topologyKey: topology.kubernetes.io/zone
        whenUnsatisfiable: DoNotSchedule
        labelSelector:
          matchLabels:
            app: feedback-collector
---
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: feedback-collector-pdb
spec:
  minAvailable: 50%
  selector:
    matchLabels:
      app: feedback-collector
```

---

## 10. Scalability & Performance

### 10.1 Performance Characteristics

**Throughput**:
- Single instance: 10,000 events/sec
- Horizontal scaling: Linear up to Kafka partition limit
- Batching: 60-80% reduction in Kafka requests

**Latency**:
- Event ingestion: <5ms (p99)
- Batch flush: <100ms (p99)
- End-to-end (ingestion → Kafka): <200ms (p99)

**Resource Utilization**:
- CPU: ~30% at 10K events/sec
- Memory: ~512MB baseline + ~100MB per 10K buffered events
- Network: ~10Mbps at 10K events/sec (with gzip compression)

### 10.2 Bottleneck Analysis

**Potential Bottlenecks**:

1. **Ring Buffer Capacity**
   - Symptom: Events rejected, 503 errors
   - Mitigation: Increase buffer size, add more instances
   - Monitoring: `collector_buffer_utilization`

2. **Kafka Publish Latency**
   - Symptom: Batches queuing up, memory growth
   - Mitigation: Tune Kafka producer settings, add partitions
   - Monitoring: `kafka_publish_latency_ms`

3. **CPU Saturation**
   - Symptom: High CPU usage, increased latency
   - Mitigation: Horizontal scaling, optimize serialization
   - Monitoring: CPU usage metrics

4. **Network Bandwidth**
   - Symptom: Slow Kafka publishing
   - Mitigation: Enable compression, larger instances
   - Monitoring: Network throughput

### 10.3 Optimization Techniques

#### 10.3.1 Batching Optimization

```rust
// Dynamic batch sizing based on load
pub struct AdaptiveBatchingEngine {
    current_batch_size: AtomicUsize,
    min_batch_size: usize,
    max_batch_size: usize,
    target_latency_ms: f64,
    recent_latencies: RingBuffer<f64>,
}

impl AdaptiveBatchingEngine {
    pub fn adjust_batch_size(&self) {
        let avg_latency = self.recent_latencies.average();

        if avg_latency > self.target_latency_ms * 1.2 {
            // Latency too high, reduce batch size
            let new_size = (self.current_batch_size.load(Ordering::Relaxed) * 9) / 10;
            self.current_batch_size.store(new_size.max(self.min_batch_size), Ordering::Relaxed);
        } else if avg_latency < self.target_latency_ms * 0.8 {
            // Latency low, increase batch size for better throughput
            let new_size = (self.current_batch_size.load(Ordering::Relaxed) * 11) / 10;
            self.current_batch_size.store(new_size.min(self.max_batch_size), Ordering::Relaxed);
        }
    }
}
```

#### 10.3.2 Zero-Copy Serialization

```rust
use bytes::BytesMut;

// Reuse buffer for serialization
pub struct SerializationPool {
    pool: Arc<Mutex<Vec<BytesMut>>>,
}

impl SerializationPool {
    pub fn acquire(&self) -> BytesMut {
        self.pool.lock().unwrap().pop()
            .unwrap_or_else(|| BytesMut::with_capacity(4096))
    }

    pub fn release(&self, mut buffer: BytesMut) {
        buffer.clear();
        if buffer.capacity() <= 16384 {  // Don't cache huge buffers
            self.pool.lock().unwrap().push(buffer);
        }
    }
}
```

#### 10.3.3 Concurrent Processing

```rust
// Multiple worker threads for CPU-bound tasks
pub struct ConcurrentBatchProcessor {
    worker_pool: rayon::ThreadPool,
}

impl ConcurrentBatchProcessor {
    pub fn process_batch(&self, batch: &FeedbackEventBatch) -> Vec<ProcessedEvent> {
        use rayon::prelude::*;

        batch.events.par_iter()
            .map(|event| self.process_event(event))
            .collect()
    }
}
```

### 10.4 Load Testing

**Load Test Scenarios**:

```rust
// Benchmarking with criterion
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

fn benchmark_ingestion(c: &mut Criterion) {
    let mut group = c.benchmark_group("ingestion");

    for size in [100, 1000, 5000, 10000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let collector = create_test_collector();
            let events = generate_events(size);

            b.iter(|| {
                for event in &events {
                    black_box(collector.try_submit(event.clone()));
                }
            });
        });
    }

    group.finish();
}

fn benchmark_batching(c: &mut Criterion) {
    let mut group = c.benchmark_group("batching");

    for batch_size in [10, 50, 100, 200].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(batch_size), batch_size, |b, &size| {
            let engine = create_batching_engine(size);
            let events = generate_events(1000);

            b.iter(|| {
                black_box(engine.create_batch(&events));
            });
        });
    }

    group.finish();
}

criterion_group!(benches, benchmark_ingestion, benchmark_batching);
criterion_main!(benches);
```

**Expected Results**:
- Ingestion: >100K events/sec (single thread)
- Batching: <1ms per batch (100 events)
- Serialization: <50μs per event

---

## 11. Security Considerations

### 11.1 Authentication & Authorization

**API Authentication**:

```rust
use jsonwebtoken::{decode, DecodingKey, Validation};

pub struct AuthMiddleware {
    jwt_secret: String,
    required_scopes: Vec<String>,
}

impl AuthMiddleware {
    pub fn validate_token(&self, token: &str) -> Result<Claims, AuthError> {
        let validation = Validation::default();
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &validation,
        )?;

        // Check scopes
        for required_scope in &self.required_scopes {
            if !token_data.claims.scopes.contains(required_scope) {
                return Err(AuthError::InsufficientPermissions);
            }
        }

        Ok(token_data.claims)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    sub: String,        // Subject (client ID)
    scopes: Vec<String>,
    exp: usize,         // Expiration time
    iat: usize,         // Issued at
}
```

**Kafka Security**:

```rust
// SASL/SSL configuration
impl KafkaProducerConfig {
    pub fn with_security(mut self) -> Self {
        self.security_protocol = Some("SASL_SSL".to_string());
        self.sasl_mechanism = Some("SCRAM-SHA-512".to_string());
        self.sasl_username = Some(std::env::var("KAFKA_USERNAME").unwrap());
        self.sasl_password = Some(std::env::var("KAFKA_PASSWORD").unwrap());
        self.ssl_ca_location = Some("/etc/kafka/ca-cert.pem".to_string());
        self
    }
}
```

### 11.2 Data Privacy

**PII Redaction**:

```rust
pub struct PIIRedactor {
    patterns: Vec<(Regex, String)>,
}

impl PIIRedactor {
    pub fn new() -> Self {
        Self {
            patterns: vec![
                // Email addresses
                (Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b").unwrap(),
                 "[EMAIL_REDACTED]".to_string()),

                // Phone numbers
                (Regex::new(r"\b\d{3}[-.]?\d{3}[-.]?\d{4}\b").unwrap(),
                 "[PHONE_REDACTED]".to_string()),

                // Credit cards
                (Regex::new(r"\b\d{4}[-\s]?\d{4}[-\s]?\d{4}[-\s]?\d{4}\b").unwrap(),
                 "[CC_REDACTED]".to_string()),

                // SSN
                (Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap(),
                 "[SSN_REDACTED]".to_string()),
            ],
        }
    }

    pub fn redact(&self, text: &str) -> String {
        let mut result = text.to_string();

        for (pattern, replacement) in &self.patterns {
            result = pattern.replace_all(&result, replacement).to_string();
        }

        result
    }
}

// Apply to user feedback
impl UserFeedbackEvent {
    pub fn redact_pii(mut self, redactor: &PIIRedactor) -> Self {
        if let Some(comment) = self.comment {
            self.comment = Some(redactor.redact(&comment));
        }
        self
    }
}
```

**Encryption at Rest**:

```yaml
# Use encrypted Kafka topics
apiVersion: v1
kind: Secret
metadata:
  name: kafka-encryption-key
type: Opaque
data:
  encryption-key: <base64-encoded-key>
```

### 11.3 Rate Limiting

```rust
use governor::{Quota, RateLimiter};
use std::net::IpAddr;

pub struct RateLimitMiddleware {
    limiters: DashMap<String, RateLimiter<String, governor::DefaultKeyedStateStore<String>, DefaultClock>>,
    quota: Quota,
}

impl RateLimitMiddleware {
    pub fn new(requests_per_second: u32) -> Self {
        let quota = Quota::per_second(nonzero!(requests_per_second));

        Self {
            limiters: DashMap::new(),
            quota,
        }
    }

    pub fn check_rate_limit(&self, client_id: &str) -> Result<(), RateLimitError> {
        let limiter = self.limiters.entry(client_id.to_string())
            .or_insert_with(|| RateLimiter::keyed(self.quota));

        limiter.check_key(&client_id.to_string())
            .map_err(|_| RateLimitError::LimitExceeded)?;

        Ok(())
    }
}
```

### 11.4 Input Validation

```rust
// Prevent injection attacks
pub struct InputSanitizer;

impl InputSanitizer {
    pub fn sanitize_string(input: &str) -> Result<String, ValidationError> {
        // Check length
        if input.len() > 10000 {
            return Err(ValidationError::TooLong);
        }

        // Check for null bytes
        if input.contains('\0') {
            return Err(ValidationError::ContainsNullByte);
        }

        // Check for control characters (except newline, tab)
        if input.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
            return Err(ValidationError::InvalidCharacters);
        }

        Ok(input.to_string())
    }

    pub fn sanitize_json(value: &serde_json::Value) -> Result<serde_json::Value, ValidationError> {
        // Limit JSON depth
        if Self::json_depth(value) > 10 {
            return Err(ValidationError::TooDeep);
        }

        // Limit JSON size
        let serialized = serde_json::to_string(value)?;
        if serialized.len() > 1_000_000 {  // 1MB
            return Err(ValidationError::TooLarge);
        }

        Ok(value.clone())
    }

    fn json_depth(value: &serde_json::Value) -> usize {
        match value {
            serde_json::Value::Object(map) => {
                1 + map.values().map(Self::json_depth).max().unwrap_or(0)
            }
            serde_json::Value::Array(arr) => {
                1 + arr.iter().map(Self::json_depth).max().unwrap_or(0)
            }
            _ => 1,
        }
    }
}
```

### 11.5 Audit Logging

```rust
pub struct AuditLogger {
    logger: Arc<StructuredLogger>,
}

impl AuditLogger {
    pub fn log_event_ingestion(&self, event: &FeedbackEvent, client_id: &str, ip_addr: IpAddr) {
        self.logger.info("event_ingested", json!({
            "event_id": event.id(),
            "event_type": event.event_type(),
            "client_id": client_id,
            "ip_addr": ip_addr.to_string(),
            "timestamp": Utc::now().to_rfc3339(),
        }));
    }

    pub fn log_authentication(&self, client_id: &str, success: bool, ip_addr: IpAddr) {
        let level = if success { "info" } else { "warn" };

        self.logger.log(level, "authentication_attempt", json!({
            "client_id": client_id,
            "success": success,
            "ip_addr": ip_addr.to_string(),
            "timestamp": Utc::now().to_rfc3339(),
        }));
    }

    pub fn log_rate_limit_exceeded(&self, client_id: &str, ip_addr: IpAddr) {
        self.logger.warn("rate_limit_exceeded", json!({
            "client_id": client_id,
            "ip_addr": ip_addr.to_string(),
            "timestamp": Utc::now().to_rfc3339(),
        }));
    }
}
```

---

## 12. Monitoring & Observability

### 12.1 Key Metrics

**Application Metrics**:

```rust
// Prometheus metrics
use prometheus_client::metrics::{counter::Counter, gauge::Gauge, histogram::Histogram};
use prometheus_client::registry::Registry;

pub struct CollectorMetrics {
    // Ingestion
    pub events_ingested_total: Counter,
    pub events_rejected_total: Counter,
    pub ingestion_latency: Histogram,

    // Buffer
    pub buffer_size: Gauge,
    pub buffer_utilization: Gauge,

    // Batching
    pub batches_created_total: Counter,
    pub batch_size: Histogram,
    pub batch_age: Histogram,

    // Kafka
    pub kafka_publish_success_total: Counter,
    pub kafka_publish_failure_total: Counter,
    pub kafka_publish_latency: Histogram,
    pub kafka_retries_total: Counter,

    // Errors
    pub validation_errors_total: Counter,
    pub kafka_errors_total: Counter,
    pub circuit_breaker_opens_total: Counter,
}

impl CollectorMetrics {
    pub fn register(registry: &mut Registry) -> Self {
        let events_ingested_total = Counter::default();
        registry.register(
            "collector_events_ingested",
            "Total number of events ingested",
            events_ingested_total.clone(),
        );

        // ... register other metrics

        Self {
            events_ingested_total,
            // ... other metrics
        }
    }
}
```

**SLI/SLO Metrics**:

```yaml
# Service Level Indicators
slis:
  - name: availability
    description: "Percentage of successful event ingestions"
    query: "sum(rate(collector_events_ingested_total[5m])) / sum(rate(collector_events_received_total[5m]))"
    target: 0.999  # 99.9%

  - name: latency_p99
    description: "99th percentile ingestion latency"
    query: "histogram_quantile(0.99, collector_ingestion_latency_ms)"
    target: 10  # 10ms

  - name: kafka_success_rate
    description: "Percentage of successful Kafka publishes"
    query: "sum(rate(kafka_publish_success_total[5m])) / sum(rate(kafka_publish_total[5m]))"
    target: 0.999  # 99.9%
```

### 12.2 Alerting Rules

```yaml
# Prometheus alerting rules
groups:
  - name: feedback_collector
    interval: 30s
    rules:
    # High error rate
    - alert: HighEventRejectionRate
      expr: |
        (sum(rate(collector_events_rejected_total[5m])) /
         sum(rate(collector_events_received_total[5m]))) > 0.05
      for: 5m
      labels:
        severity: critical
      annotations:
        summary: "High event rejection rate"
        description: "{{ $value | humanizePercentage }} of events are being rejected"

    # Buffer saturation
    - alert: BufferNearlyFull
      expr: collector_buffer_utilization > 0.9
      for: 2m
      labels:
        severity: warning
      annotations:
        summary: "Buffer nearly full"
        description: "Buffer is {{ $value | humanizePercentage }} full"

    # Kafka publish failures
    - alert: KafkaPublishFailures
      expr: |
        (sum(rate(kafka_publish_failure_total[5m])) /
         sum(rate(kafka_publish_total[5m]))) > 0.01
      for: 5m
      labels:
        severity: critical
      annotations:
        summary: "High Kafka publish failure rate"
        description: "{{ $value | humanizePercentage }} of Kafka publishes are failing"

    # Circuit breaker open
    - alert: CircuitBreakerOpen
      expr: collector_circuit_breaker_state == 1
      for: 1m
      labels:
        severity: warning
      annotations:
        summary: "Circuit breaker is open"
        description: "Circuit breaker has been open for over 1 minute"

    # High latency
    - alert: HighIngestionLatency
      expr: |
        histogram_quantile(0.99,
          rate(collector_ingestion_latency_ms_bucket[5m])
        ) > 50
      for: 5m
      labels:
        severity: warning
      annotations:
        summary: "High ingestion latency"
        description: "P99 latency is {{ $value }}ms"
```

### 12.3 Dashboards

**Grafana Dashboard JSON** (excerpt):

```json
{
  "dashboard": {
    "title": "Feedback Collector",
    "panels": [
      {
        "title": "Event Ingestion Rate",
        "targets": [
          {
            "expr": "sum(rate(collector_events_ingested_total[1m]))",
            "legendFormat": "Events/sec"
          }
        ],
        "type": "graph"
      },
      {
        "title": "Ingestion Latency",
        "targets": [
          {
            "expr": "histogram_quantile(0.50, rate(collector_ingestion_latency_ms_bucket[5m]))",
            "legendFormat": "p50"
          },
          {
            "expr": "histogram_quantile(0.95, rate(collector_ingestion_latency_ms_bucket[5m]))",
            "legendFormat": "p95"
          },
          {
            "expr": "histogram_quantile(0.99, rate(collector_ingestion_latency_ms_bucket[5m]))",
            "legendFormat": "p99"
          }
        ],
        "type": "graph"
      },
      {
        "title": "Buffer Utilization",
        "targets": [
          {
            "expr": "collector_buffer_utilization",
            "legendFormat": "Utilization"
          }
        ],
        "type": "gauge",
        "thresholds": [
          { "value": 0.7, "color": "yellow" },
          { "value": 0.9, "color": "red" }
        ]
      },
      {
        "title": "Kafka Publish Success Rate",
        "targets": [
          {
            "expr": "sum(rate(kafka_publish_success_total[5m])) / sum(rate(kafka_publish_total[5m]))",
            "legendFormat": "Success Rate"
          }
        ],
        "type": "stat"
      }
    ]
  }
}
```

### 12.4 Distributed Tracing

**Trace Visualization**:

```
Trace ID: 550e8400-e29b-41d4-a716-446655440000
Duration: 45ms

┌─ submit_event (45ms)
│  ├─ validate_event (2ms)
│  │  └─ check_schema (1ms)
│  ├─ send_to_buffer (1ms)
│  └─ publish_batch (42ms)
│     ├─ serialize_events (5ms)
│     └─ kafka_send (37ms)
│        ├─ compression (8ms)
│        ├─ network_send (25ms)
│        └─ ack_wait (4ms)
```

**Trace Attributes**:
- `event.id`: Unique event identifier
- `event.type`: Event type
- `batch.id`: Batch identifier
- `batch.size`: Number of events in batch
- `kafka.topic`: Kafka topic
- `kafka.partition`: Kafka partition
- `kafka.offset`: Kafka offset

---

## 13. Testing Strategy

### 13.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_validation() {
        let validator = EventValidator::new();

        // Valid event
        let event = UserFeedbackEvent {
            event_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            session_id: "session123".to_string(),
            request_id: Uuid::new_v4(),
            rating: Some(4.5),
            ..Default::default()
        };
        assert!(validator.validate_user_feedback(&event).is_ok());

        // Invalid rating
        let invalid_event = UserFeedbackEvent {
            rating: Some(6.0),  // Out of range
            ..event
        };
        assert!(validator.validate_user_feedback(&invalid_event).is_err());
    }

    #[tokio::test]
    async fn test_batching_size_limit() {
        let mut engine = BatchingEngine::new(10, Duration::from_secs(60));

        // Add 9 events
        for _ in 0..9 {
            engine.add_event(create_test_event());
        }
        assert_eq!(engine.batch_size(), 9);
        assert!(!engine.should_flush());

        // Add 10th event
        engine.add_event(create_test_event());
        assert_eq!(engine.batch_size(), 10);
        assert!(engine.should_flush());
    }

    #[tokio::test]
    async fn test_batching_age_limit() {
        let mut engine = BatchingEngine::new(100, Duration::from_secs(1));

        engine.add_event(create_test_event());
        assert!(!engine.should_flush());

        // Wait for batch to age
        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert!(engine.should_flush());
    }
}
```

### 13.2 Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use testcontainers::*;

    #[tokio::test]
    async fn test_kafka_publish() {
        // Start Kafka container
        let docker = clients::Cli::default();
        let kafka = docker.run(images::kafka::Kafka::default());
        let bootstrap_servers = format!("localhost:{}", kafka.get_host_port_ipv4(9092));

        // Create producer
        let config = KafkaProducerConfig {
            brokers: bootstrap_servers,
            topic: "test-events".to_string(),
            ..Default::default()
        };
        let producer = KafkaProducer::new(config).unwrap();

        // Publish event
        let event = create_test_event();
        let result = producer.publish_event(&event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_end_to_end() {
        // Setup: Kafka + OTEL Collector
        let docker = clients::Cli::default();
        let kafka = docker.run(images::kafka::Kafka::default());

        // Create collector
        let config = FeedbackCollectorConfig {
            kafka: KafkaProducerConfig {
                brokers: format!("localhost:{}", kafka.get_host_port_ipv4(9092)),
                ..Default::default()
            },
            ..Default::default()
        };

        let mut collector = FeedbackCollector::new(config);
        let (sender, receiver) = mpsc::channel(100);
        collector.start(receiver).unwrap();

        // Submit events
        let events: Vec<_> = (0..100).map(|_| create_test_event()).collect();
        for event in &events {
            sender.send(event.clone()).await.unwrap();
        }

        // Wait for processing
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Verify stats
        let stats = collector.stats();
        assert_eq!(stats.events_received, 100);
        assert_eq!(stats.events_published, 100);
    }
}
```

### 13.3 Performance Tests

```rust
#[cfg(test)]
mod bench_tests {
    use criterion::{black_box, criterion_group, criterion_main, Criterion};

    fn benchmark_serialization(c: &mut Criterion) {
        let event = create_test_event();

        c.bench_function("event_serialization", |b| {
            b.iter(|| {
                black_box(serde_json::to_vec(&event).unwrap())
            });
        });
    }

    fn benchmark_validation(c: &mut Criterion) {
        let validator = EventValidator::new();
        let event = create_test_event();

        c.bench_function("event_validation", |b| {
            b.iter(|| {
                black_box(validator.validate(&event))
            });
        });
    }

    criterion_group!(benches, benchmark_serialization, benchmark_validation);
    criterion_main!(benches);
}
```

### 13.4 Chaos Testing

```rust
// Chaos testing scenarios
#[tokio::test]
async fn test_kafka_unavailable() {
    // Simulate Kafka being down
    let config = KafkaProducerConfig {
        brokers: "localhost:9999".to_string(),  // Non-existent broker
        ..Default::default()
    };

    let collector = FeedbackCollectorWithFallback::new(config);
    let event = create_test_event();

    // Should use fallback
    let result = collector.submit(event).await;
    assert!(result.is_ok());

    // Verify event in fallback storage
    let pending = collector.fallback_storage.retrieve_pending().await.unwrap();
    assert_eq!(pending.len(), 1);
}

#[tokio::test]
async fn test_high_latency() {
    // Simulate slow Kafka
    let collector = create_test_collector_with_latency(Duration::from_secs(5));

    // Submit events
    let start = Instant::now();
    for _ in 0..10 {
        collector.submit(create_test_event()).await.unwrap();
    }

    // Should complete quickly due to async processing
    assert!(start.elapsed() < Duration::from_millis(100));
}
```

---

## 14. Implementation Roadmap

### 14.1 Phase 1: Core Foundation (Week 1-2)

**Deliverables**:
- [x] Data models (events, batches)
- [x] Basic collector structure
- [ ] Event validation
- [ ] Ring buffer implementation
- [ ] Unit tests

**Success Criteria**:
- All event types defined and serializable
- Events can be queued and retrieved
- 100% test coverage for data models

### 14.2 Phase 2: Kafka Integration (Week 3-4)

**Deliverables**:
- [ ] Kafka producer implementation
- [ ] Batch publishing logic
- [ ] Retry mechanism
- [ ] Error handling
- [ ] Integration tests with Kafka

**Success Criteria**:
- Events successfully published to Kafka
- Retries work correctly
- At-least-once delivery guaranteed

### 14.3 Phase 3: OpenTelemetry Integration (Week 5-6)

**Deliverables**:
- [ ] Metrics collection
- [ ] Distributed tracing
- [ ] OTLP exporter
- [ ] Grafana dashboards
- [ ] Alert rules

**Success Criteria**:
- All key metrics exported to Prometheus
- Traces visible in Jaeger
- Dashboards operational

### 14.4 Phase 4: Production Hardening (Week 7-8)

**Deliverables**:
- [ ] Circuit breaker
- [ ] Rate limiting
- [ ] Security (authentication, encryption)
- [ ] Configuration management
- [ ] Load testing

**Success Criteria**:
- Circuit breaker prevents cascading failures
- Rate limiting works correctly
- Security audit passed
- 10K events/sec sustained throughput

### 14.5 Phase 5: Deployment & Operations (Week 9-10)

**Deliverables**:
- [ ] Kubernetes manifests
- [ ] Helm charts
- [ ] CI/CD pipeline
- [ ] Runbooks
- [ ] Production deployment

**Success Criteria**:
- Deployed to production
- All SLOs met
- Zero critical bugs in 48 hours

---

## 15. Appendices

### 15.1 Glossary

- **At-Least-Once Delivery**: Guarantee that every message is delivered at least once (may be delivered multiple times)
- **Exactly-Once Semantics**: Guarantee that every message is delivered exactly once (no duplicates)
- **Circuit Breaker**: Pattern that prevents cascading failures by stopping requests to failing service
- **Dead Letter Queue**: Queue for messages that failed processing after retries
- **Idempotent**: Operation that can be applied multiple times with same result
- **OTLP**: OpenTelemetry Protocol for transmitting telemetry data
- **Partitioning**: Distributing data across multiple nodes for parallelism
- **Backpressure**: Flow control mechanism to prevent overwhelming downstream systems

### 15.2 References

1. **OpenTelemetry**:
   - [Official Documentation](https://opentelemetry.io/docs/languages/rust/)
   - [Rust SDK GitHub](https://github.com/open-telemetry/opentelemetry-rust)

2. **Kafka**:
   - [Apache Kafka Documentation](https://kafka.apache.org/documentation/)
   - [rdkafka Rust Client](https://github.com/fede1024/rust-rdkafka)

3. **Rust Async**:
   - [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
   - [Async Book](https://rust-lang.github.io/async-book/)

4. **Patterns**:
   - [Circuit Breaker Pattern](https://martinfowler.com/bliki/CircuitBreaker.html)
   - [Bulkhead Pattern](https://docs.microsoft.com/en-us/azure/architecture/patterns/bulkhead)

### 15.3 Diagrams Source

All diagrams in this document are text-based ASCII art for easy version control and editing. For presentation purposes, they can be rendered using tools like:
- [Monodraw](https://monodraw.helftone.com/) (macOS)
- [ASCIIFlow](https://asciiflow.com/) (Web)

---

**End of Architecture Document**

**Approval**:
- [ ] Tech Lead Review
- [ ] Security Review
- [ ] Operations Review
- [ ] Product Owner Approval

**Version History**:
- 1.0.0 (2025-11-09): Initial architecture design
