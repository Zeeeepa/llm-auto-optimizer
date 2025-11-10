# Metrics Export Implementation Plan

**Detailed implementation roadmap with file structure and scaffolding**

---

## Table of Contents

1. [Phase 1: Core Metrics Infrastructure](#phase-1-core-metrics-infrastructure)
2. [Phase 2: Prometheus Export](#phase-2-prometheus-export)
3. [Phase 3: OpenTelemetry Integration](#phase-3-opentelemetry-integration)
4. [Phase 4: Application Metrics](#phase-4-application-metrics)
5. [Phase 5: Testing & Documentation](#phase-5-testing--documentation)
6. [File Structure](#file-structure)
7. [Code Scaffolding](#code-scaffolding)

---

## Phase 1: Core Metrics Infrastructure

**Duration**: Week 1-2
**Goal**: Implement lock-free, high-performance metric primitives

### Tasks

#### 1.1 Project Setup

**Files to Create**:
```
crates/processor/src/metrics/
├── mod.rs
├── types.rs
├── error.rs
└── config.rs
```

**Code Scaffolding**:

```rust
// crates/processor/src/metrics/mod.rs
//! High-performance metrics infrastructure for stream processor
//!
//! This module provides lock-free metric primitives compatible with
//! Prometheus and OpenTelemetry.

pub mod types;
pub mod error;
pub mod config;
pub mod counter;
pub mod gauge;
pub mod histogram;
pub mod summary;
pub mod registry;
pub mod labels;
pub mod cardinality;
pub mod prometheus;
pub mod otel;
pub mod application;

pub use counter::{Counter, CounterFamily};
pub use gauge::{Gauge, GaugeFamily};
pub use histogram::{Histogram, HistogramFamily, buckets};
pub use summary::{Summary, SummaryFamily};
pub use registry::{MetricsRegistry, RegistryConfig};
pub use labels::{Labels, LabelSet};
pub use error::{MetricsError, Result};
pub use config::ObservabilityConfig;
```

```rust
// crates/processor/src/metrics/types.rs
//! Core metric types and metadata

use std::sync::Arc;

/// Metric metadata
#[derive(Debug, Clone)]
pub struct MetricMetadata {
    /// Metric name
    pub name: String,
    /// Help text
    pub help: String,
    /// Metric type
    pub metric_type: MetricType,
}

/// Metric types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Summary,
}

impl MetricType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MetricType::Counter => "counter",
            MetricType::Gauge => "gauge",
            MetricType::Histogram => "histogram",
            MetricType::Summary => "summary",
        }
    }
}
```

```rust
// crates/processor/src/metrics/error.rs
//! Metrics error types

use thiserror::Error;

#[derive(Error, Debug)]
pub enum MetricsError {
    #[error("Metric already registered: {0}")]
    AlreadyRegistered(String),

    #[error("Invalid metric name: {0}")]
    InvalidName(String),

    #[error("Invalid label name: {0}")]
    InvalidLabel(String),

    #[error("Cardinality limit exceeded: {current} > {max}")]
    CardinalityExceeded { current: usize, max: usize },

    #[error("Label value too long: {length} > {max}")]
    LabelValueTooLong { length: usize, max: usize },

    #[error("Too many labels: {count} > {max}")]
    TooManyLabels { count: usize, max: usize },

    #[error("Metric not found: {0}")]
    NotFound(String),

    #[error("OpenTelemetry error: {0}")]
    OpenTelemetryError(#[from] opentelemetry::metrics::MetricsError),

    #[error("OpenTelemetry trace error: {0}")]
    TraceError(String),
}

pub type Result<T> = std::result::Result<T, MetricsError>;
```

#### 1.2 Lock-Free Counter

**File**: `crates/processor/src/metrics/counter.rs`

```rust
//! Lock-free atomic counter implementation

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use dashmap::DashMap;

use super::{MetricMetadata, MetricType, Labels, LabelSet, Result};

/// Lock-free counter with cache line padding
#[repr(align(64))]
pub struct Counter {
    /// Atomic value (monotonic)
    value: AtomicU64,
    /// Metadata
    metadata: Arc<MetricMetadata>,
    /// Label set
    labels: Arc<Labels>,
    /// Padding to prevent false sharing
    _padding: [u8; 56],
}

impl Counter {
    /// Create new counter
    pub fn new(name: String, help: String, labels: Arc<Labels>) -> Self {
        Self {
            value: AtomicU64::new(0),
            metadata: Arc::new(MetricMetadata {
                name,
                help,
                metric_type: MetricType::Counter,
            }),
            labels,
            _padding: [0; 56],
        }
    }

    /// Increment counter by 1 (hot path)
    #[inline(always)]
    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment counter by delta
    #[inline(always)]
    pub fn inc_by(&self, delta: u64) {
        self.value.fetch_add(delta, Ordering::Relaxed);
    }

    /// Get current value
    #[inline]
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    /// Reset counter (for testing)
    #[cfg(test)]
    pub fn reset(&self) {
        self.value.store(0, Ordering::Relaxed);
    }

    /// Get metadata
    pub fn metadata(&self) -> &MetricMetadata {
        &self.metadata
    }

    /// Get labels
    pub fn labels(&self) -> &Labels {
        &self.labels
    }
}

impl Clone for Counter {
    fn clone(&self) -> Self {
        Self {
            value: AtomicU64::new(self.get()),
            metadata: self.metadata.clone(),
            labels: self.labels.clone(),
            _padding: [0; 56],
        }
    }
}

/// Counter family with label support
pub struct CounterFamily {
    /// Base metric metadata
    metadata: Arc<MetricMetadata>,
    /// Counters indexed by label set
    counters: DashMap<LabelSet, Counter>,
}

impl CounterFamily {
    /// Create new counter family
    pub fn new(name: String, help: String) -> Self {
        Self {
            metadata: Arc::new(MetricMetadata {
                name,
                help,
                metric_type: MetricType::Counter,
            }),
            counters: DashMap::new(),
        }
    }

    /// Get or create counter with labels
    pub fn get_or_create(&self, label_pairs: &[(&str, &str)]) -> Counter {
        let label_set = LabelSet::from_pairs(label_pairs);

        self.counters
            .entry(label_set.clone())
            .or_insert_with(|| {
                Counter::new(
                    self.metadata.name.clone(),
                    self.metadata.help.clone(),
                    Arc::new(label_set.into_labels()),
                )
            })
            .clone()
    }

    /// Get counter with default (empty) labels
    pub fn default(&self) -> Counter {
        self.get_or_create(&[])
    }

    /// Get all counters
    pub fn counters(&self) -> Vec<Counter> {
        self.counters.iter().map(|e| e.value().clone()).collect()
    }

    /// Get metadata
    pub fn metadata(&self) -> &MetricMetadata {
        &self.metadata
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter_increment() {
        let counter = Counter::new(
            "test".to_string(),
            "Test counter".to_string(),
            Arc::new(Labels::empty()),
        );

        assert_eq!(counter.get(), 0);

        counter.inc();
        assert_eq!(counter.get(), 1);

        counter.inc_by(5);
        assert_eq!(counter.get(), 6);
    }

    #[test]
    fn test_counter_concurrent() {
        use std::sync::Arc;
        use std::thread;

        let counter = Arc::new(Counter::new(
            "test".to_string(),
            "Test".to_string(),
            Arc::new(Labels::empty()),
        ));

        let threads = 8;
        let increments = 10000;

        let handles: Vec<_> = (0..threads)
            .map(|_| {
                let c = counter.clone();
                thread::spawn(move || {
                    for _ in 0..increments {
                        c.inc();
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(counter.get(), threads * increments);
    }

    #[test]
    fn test_counter_family() {
        let family = CounterFamily::new("requests_total".to_string(), "Total requests".to_string());

        let counter1 = family.get_or_create(&[("method", "GET")]);
        let counter2 = family.get_or_create(&[("method", "POST")]);
        let counter3 = family.get_or_create(&[("method", "GET")]);

        counter1.inc();
        counter2.inc_by(2);

        // Same labels should return same counter
        assert_eq!(counter1.get(), counter3.get());
        assert_eq!(counter1.get(), 1);
        assert_eq!(counter2.get(), 2);
    }
}
```

#### 1.3 Atomic Gauge

**File**: `crates/processor/src/metrics/gauge.rs`

```rust
//! Atomic gauge implementation (can increase or decrease)

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use dashmap::DashMap;

use super::{MetricMetadata, MetricType, Labels, LabelSet, Result};

/// Atomic gauge (up/down counter)
#[repr(align(64))]
pub struct Gauge {
    /// Atomic value (f64 as i64 bits)
    value: AtomicI64,
    /// Metadata
    metadata: Arc<MetricMetadata>,
    /// Labels
    labels: Arc<Labels>,
    /// Padding
    _padding: [u8; 56],
}

impl Gauge {
    /// Create new gauge
    pub fn new(name: String, help: String, labels: Arc<Labels>) -> Self {
        Self {
            value: AtomicI64::new(0),
            metadata: Arc::new(MetricMetadata {
                name,
                help,
                metric_type: MetricType::Gauge,
            }),
            labels,
            _padding: [0; 56],
        }
    }

    /// Set gauge to value
    #[inline]
    pub fn set(&self, value: f64) {
        let bits = value.to_bits() as i64;
        self.value.store(bits, Ordering::Relaxed);
    }

    /// Increment by 1
    #[inline]
    pub fn inc(&self) {
        self.add(1.0);
    }

    /// Decrement by 1
    #[inline]
    pub fn dec(&self) {
        self.sub(1.0);
    }

    /// Add delta
    pub fn add(&self, delta: f64) {
        loop {
            let current_bits = self.value.load(Ordering::Relaxed);
            let current = f64::from_bits(current_bits as u64);
            let new_value = current + delta;
            let new_bits = new_value.to_bits() as i64;

            if self
                .value
                .compare_exchange_weak(
                    current_bits,
                    new_bits,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                break;
            }
        }
    }

    /// Subtract delta
    #[inline]
    pub fn sub(&self, delta: f64) {
        self.add(-delta);
    }

    /// Get current value
    #[inline]
    pub fn get(&self) -> f64 {
        let bits = self.value.load(Ordering::Relaxed);
        f64::from_bits(bits as u64)
    }

    pub fn metadata(&self) -> &MetricMetadata {
        &self.metadata
    }

    pub fn labels(&self) -> &Labels {
        &self.labels
    }
}

impl Clone for Gauge {
    fn clone(&self) -> Self {
        Self {
            value: AtomicI64::new(self.value.load(Ordering::Relaxed)),
            metadata: self.metadata.clone(),
            labels: self.labels.clone(),
            _padding: [0; 56],
        }
    }
}

/// Gauge family
pub struct GaugeFamily {
    metadata: Arc<MetricMetadata>,
    gauges: DashMap<LabelSet, Gauge>,
}

impl GaugeFamily {
    pub fn new(name: String, help: String) -> Self {
        Self {
            metadata: Arc::new(MetricMetadata {
                name,
                help,
                metric_type: MetricType::Gauge,
            }),
            gauges: DashMap::new(),
        }
    }

    pub fn get_or_create(&self, label_pairs: &[(&str, &str)]) -> Gauge {
        let label_set = LabelSet::from_pairs(label_pairs);

        self.gauges
            .entry(label_set.clone())
            .or_insert_with(|| {
                Gauge::new(
                    self.metadata.name.clone(),
                    self.metadata.help.clone(),
                    Arc::new(label_set.into_labels()),
                )
            })
            .clone()
    }

    pub fn default(&self) -> Gauge {
        self.get_or_create(&[])
    }

    pub fn gauges(&self) -> Vec<Gauge> {
        self.gauges.iter().map(|e| e.value().clone()).collect()
    }

    pub fn metadata(&self) -> &MetricMetadata {
        &self.metadata
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gauge_operations() {
        let gauge = Gauge::new(
            "test".to_string(),
            "Test gauge".to_string(),
            Arc::new(Labels::empty()),
        );

        gauge.set(10.0);
        assert_eq!(gauge.get(), 10.0);

        gauge.inc();
        assert_eq!(gauge.get(), 11.0);

        gauge.dec();
        assert_eq!(gauge.get(), 10.0);

        gauge.add(5.5);
        assert_eq!(gauge.get(), 15.5);

        gauge.sub(3.5);
        assert_eq!(gauge.get(), 12.0);
    }

    #[test]
    fn test_gauge_negative() {
        let gauge = Gauge::new(
            "test".to_string(),
            "Test".to_string(),
            Arc::new(Labels::empty()),
        );

        gauge.set(-10.5);
        assert_eq!(gauge.get(), -10.5);

        gauge.inc();
        assert_eq!(gauge.get(), -9.5);
    }
}
```

#### 1.4 Label System

**File**: `crates/processor/src/metrics/labels.rs`

```rust
//! Efficient label encoding and storage

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// Label set with pre-computed hash and encoding
#[derive(Clone)]
pub struct LabelSet {
    /// Pre-computed hash
    hash: u64,
    /// Sorted label pairs
    labels: Arc<Vec<(String, String)>>,
    /// Cached Prometheus encoding
    encoded: Arc<String>,
}

impl LabelSet {
    /// Create from label pairs (sorted for consistency)
    pub fn from_pairs(pairs: &[(&str, &str)]) -> Self {
        let mut labels: Vec<_> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        // Sort for consistent hashing
        labels.sort_by(|a, b| a.0.cmp(&b.0));

        // Pre-compute hash
        let mut hasher = DefaultHasher::new();
        for (k, v) in &labels {
            k.hash(&mut hasher);
            v.hash(&mut hasher);
        }
        let hash = hasher.finish();

        // Pre-encode Prometheus format
        let encoded = Self::encode_prometheus(&labels);

        Self {
            hash,
            labels: Arc::new(labels),
            encoded: Arc::new(encoded),
        }
    }

    /// Get pre-encoded Prometheus string
    pub fn as_prometheus(&self) -> &str {
        &self.encoded
    }

    /// Convert to Labels
    pub fn into_labels(self) -> Labels {
        Labels {
            pairs: self.labels,
        }
    }

    /// Encode in Prometheus format: {key1="value1",key2="value2"}
    fn encode_prometheus(labels: &[(String, String)]) -> String {
        if labels.is_empty() {
            return String::new();
        }

        let mut s = String::with_capacity(64);
        s.push('{');

        for (i, (k, v)) in labels.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(k);
            s.push_str("=\"");
            // Escape backslashes and quotes
            s.push_str(&v.replace('\\', "\\\\").replace('"', "\\\""));
            s.push('"');
        }

        s.push('}');
        s
    }

    pub fn len(&self) -> usize {
        self.labels.len()
    }

    pub fn is_empty(&self) -> bool {
        self.labels.is_empty()
    }
}

impl PartialEq for LabelSet {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash && self.labels == other.labels
    }
}

impl Eq for LabelSet {}

impl Hash for LabelSet {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

/// Labels (immutable label set)
#[derive(Debug, Clone)]
pub struct Labels {
    pairs: Arc<Vec<(String, String)>>,
}

impl Labels {
    /// Create empty label set
    pub fn empty() -> Self {
        Self {
            pairs: Arc::new(Vec::new()),
        }
    }

    /// Create from pairs
    pub fn from_pairs(pairs: &[(&str, &str)]) -> Self {
        let pairs: Vec<_> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        Self {
            pairs: Arc::new(pairs),
        }
    }

    /// Get label value by key
    pub fn get(&self, key: &str) -> Option<&str> {
        self.pairs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Get all pairs
    pub fn pairs(&self) -> &[(String, String)] {
        &self.pairs
    }

    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_label_set_creation() {
        let labels = LabelSet::from_pairs(&[
            ("method", "GET"),
            ("status", "200"),
        ]);

        assert_eq!(labels.len(), 2);
        assert!(!labels.is_empty());
    }

    #[test]
    fn test_label_set_ordering() {
        let labels1 = LabelSet::from_pairs(&[("a", "1"), ("b", "2")]);
        let labels2 = LabelSet::from_pairs(&[("b", "2"), ("a", "1")]);

        // Should be equal regardless of input order
        assert_eq!(labels1, labels2);
        assert_eq!(labels1.as_prometheus(), labels2.as_prometheus());
    }

    #[test]
    fn test_prometheus_encoding() {
        let labels = LabelSet::from_pairs(&[
            ("method", "GET"),
            ("status", "200"),
        ]);

        assert_eq!(labels.as_prometheus(), r#"{method="GET",status="200"}"#);
    }

    #[test]
    fn test_prometheus_encoding_escaping() {
        let labels = LabelSet::from_pairs(&[
            ("key", r#"value with "quotes""#),
        ]);

        assert!(labels.as_prometheus().contains(r#"value with \"quotes\""#));
    }

    #[test]
    fn test_empty_labels() {
        let labels = LabelSet::from_pairs(&[]);
        assert_eq!(labels.as_prometheus(), "");
        assert!(labels.is_empty());
    }
}
```

### Deliverables Checklist

- [ ] Project structure created
- [ ] Lock-free counter implementation
- [ ] Atomic gauge implementation
- [ ] Histogram with buckets (TODO: next phase)
- [ ] Summary with quantiles (TODO: next phase)
- [ ] Label system with pre-computation
- [ ] Unit tests for all primitives
- [ ] Benchmarks demonstrating <1μs overhead

---

## Phase 2: Prometheus Export

**Duration**: Week 3
**Goal**: Implement Prometheus text format export and HTTP endpoint

### File Structure

```
crates/processor/src/metrics/prometheus/
├── mod.rs
├── renderer.rs
├── format.rs
└── server.rs
```

### Implementation Tasks

#### 2.1 Prometheus Renderer

**File**: `crates/processor/src/metrics/prometheus/renderer.rs`

```rust
//! Prometheus text format renderer

use std::fmt::Write;
use super::super::{Counter, Gauge, Histogram, Summary, MetricType};

/// Render metrics in Prometheus text format
pub struct PrometheusRenderer;

impl PrometheusRenderer {
    /// Render counter family
    pub fn render_counter_family(
        output: &mut String,
        family: &CounterFamily,
    ) -> std::fmt::Result {
        let metadata = family.metadata();

        // HELP line
        writeln!(output, "# HELP {} {}", metadata.name, metadata.help)?;

        // TYPE line
        writeln!(output, "# TYPE {} counter", metadata.name)?;

        // Samples
        for counter in family.counters() {
            let value = counter.get();
            let labels = counter.labels();

            if labels.is_empty() {
                writeln!(output, "{} {}", metadata.name, value)?;
            } else {
                let label_set = LabelSet::from_labels(labels);
                writeln!(
                    output,
                    "{}{} {}",
                    metadata.name,
                    label_set.as_prometheus(),
                    value
                )?;
            }
        }

        Ok(())
    }

    /// Render histogram family
    pub fn render_histogram_family(
        output: &mut String,
        family: &HistogramFamily,
    ) -> std::fmt::Result {
        let metadata = family.metadata();

        // HELP line
        writeln!(output, "# HELP {} {}", metadata.name, metadata.help)?;

        // TYPE line
        writeln!(output, "# TYPE {} histogram", metadata.name)?;

        // Samples for each histogram
        for histogram in family.histograms() {
            let snapshot = histogram.snapshot();
            let labels = histogram.labels();
            let label_prefix = if labels.is_empty() {
                String::new()
            } else {
                LabelSet::from_labels(labels).as_prometheus().to_string()
            };

            // Buckets
            for (i, count) in snapshot.buckets.iter().enumerate() {
                let bound = histogram.bucket_bounds()[i];
                let bound_str = if bound.is_infinite() {
                    "+Inf".to_string()
                } else {
                    bound.to_string()
                };

                writeln!(
                    output,
                    "{}_bucket{{{}le=\"{}\"}} {}",
                    metadata.name,
                    if label_prefix.is_empty() {
                        String::new()
                    } else {
                        format!("{},", &label_prefix[1..label_prefix.len()-1])
                    },
                    bound_str,
                    count
                )?;
            }

            // Sum
            writeln!(
                output,
                "{}_sum{} {}",
                metadata.name,
                label_prefix,
                snapshot.sum
            )?;

            // Count
            writeln!(
                output,
                "{}_count{} {}",
                metadata.name,
                label_prefix,
                snapshot.count
            )?;
        }

        Ok(())
    }

    /// Render all metrics
    pub fn render_all(registry: &MetricsRegistry) -> String {
        let mut output = String::with_capacity(4096);

        // Render counters
        for entry in registry.counters.iter() {
            Self::render_counter_family(&mut output, entry.value())
                .expect("Failed to render counter");
        }

        // Render gauges
        for entry in registry.gauges.iter() {
            Self::render_gauge_family(&mut output, entry.value())
                .expect("Failed to render gauge");
        }

        // Render histograms
        for entry in registry.histograms.iter() {
            Self::render_histogram_family(&mut output, entry.value())
                .expect("Failed to render histogram");
        }

        output
    }
}
```

#### 2.2 HTTP Server

**File**: `crates/processor/src/metrics/prometheus/server.rs`

```rust
//! Prometheus HTTP metrics server

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::sync::Arc;
use std::time::Instant;

use super::super::MetricsRegistry;
use super::renderer::PrometheusRenderer;

/// Metrics server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub path: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 9090,
            path: "/metrics".to_string(),
        }
    }
}

/// Metrics HTTP server
pub struct MetricsServer {
    registry: Arc<MetricsRegistry>,
    config: ServerConfig,
}

impl MetricsServer {
    pub fn new(registry: Arc<MetricsRegistry>, config: ServerConfig) -> Self {
        Self { registry, config }
    }

    /// Create Axum router
    pub fn router(&self) -> Router {
        Router::new()
            .route("/metrics", get(metrics_handler))
            .route("/health", get(health_handler))
            .with_state(self.registry.clone())
    }

    /// Start server
    pub async fn serve(self) -> Result<(), std::io::Error> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;

        tracing::info!("Metrics server listening on {}", addr);

        axum::serve(listener, self.router()).await
    }
}

/// GET /metrics handler
async fn metrics_handler(State(registry): State<Arc<MetricsRegistry>>) -> Response {
    let start = Instant::now();

    let output = PrometheusRenderer::render_all(&registry);

    let duration = start.elapsed();

    (
        StatusCode::OK,
        [
            ("Content-Type", "text/plain; version=0.0.4; charset=utf-8"),
            ("X-Render-Duration-Ms", &duration.as_millis().to_string()),
        ],
        output,
    )
        .into_response()
}

/// GET /health handler
async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}
```

### Deliverables Checklist

- [ ] Prometheus text format renderer
- [ ] HTTP server with /metrics endpoint
- [ ] Counter rendering
- [ ] Gauge rendering
- [ ] Histogram rendering (with buckets)
- [ ] Summary rendering (with quantiles)
- [ ] Integration tests
- [ ] Performance tests (<50ms render time)

---

## Phase 3: OpenTelemetry Integration

**Duration**: Week 4
**Goal**: Add OpenTelemetry tracing and OTLP export

### File Structure

```
crates/processor/src/metrics/otel/
├── mod.rs
├── provider.rs
├── instrumentation.rs
├── propagation.rs
└── config.rs
```

### Key Implementation

```rust
// crates/processor/src/metrics/otel/provider.rs
use opentelemetry::{trace::TracerProvider, KeyValue};
use opentelemetry_sdk::{trace, Resource};
use opentelemetry_otlp::WithExportConfig;

pub struct OtelProvider {
    tracer_provider: trace::TracerProvider,
    config: OtelConfig,
}

impl OtelProvider {
    pub fn init(config: OtelConfig) -> Result<Self, TraceError> {
        let resource = Resource::new(vec![
            KeyValue::new("service.name", config.service_name.clone()),
            KeyValue::new("service.version", config.service_version.clone()),
        ]);

        let tracer_provider = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint(&config.otlp_endpoint)
            )
            .with_trace_config(trace::Config::default().with_resource(resource))
            .install_batch(opentelemetry_sdk::runtime::Tokio)?;

        Ok(Self {
            tracer_provider,
            config,
        })
    }

    pub fn tracer(&self, name: &str) -> impl Tracer {
        self.tracer_provider.tracer(name)
    }
}
```

### Deliverables Checklist

- [ ] OTel provider setup
- [ ] Tracer creation
- [ ] Span instrumentation helpers
- [ ] Context propagation (Kafka headers)
- [ ] Sampling configuration
- [ ] Integration tests with OTLP collector

---

## Phase 4: Application Metrics

**Duration**: Week 5
**Goal**: Instrument all stream processor components

### File Structure

```
crates/processor/src/metrics/application/
├── mod.rs
├── processor.rs
├── window.rs
├── state.rs
└── kafka.rs
```

### Example Implementation

```rust
// crates/processor/src/metrics/application/processor.rs
pub struct ProcessorMetrics {
    pub events_received_total: Counter,
    pub events_processed_total: Counter,
    pub processing_duration_ms: Histogram,
    pub errors_total: CounterFamily,
}

impl ProcessorMetrics {
    pub fn new(registry: &MetricsRegistry) -> Result<Self> {
        Ok(Self {
            events_received_total: registry
                .register_counter(
                    "stream_processor_events_received_total",
                    "Total events received",
                )?
                .default(),

            events_processed_total: registry
                .register_counter(
                    "stream_processor_events_processed_total",
                    "Total events processed",
                )?
                .default(),

            processing_duration_ms: registry
                .register_histogram(
                    "stream_processor_processing_duration_ms",
                    "Processing duration",
                    &buckets::LATENCY_MS,
                )?
                .default(),

            errors_total: registry.register_counter(
                "stream_processor_errors_total",
                "Total errors",
            )?,
        })
    }
}
```

---

## File Structure

```
crates/processor/
├── Cargo.toml (updated dependencies)
└── src/
    ├── lib.rs (add metrics module export)
    └── metrics/
        ├── mod.rs
        ├── types.rs
        ├── error.rs
        ├── config.rs
        ├── counter.rs
        ├── gauge.rs
        ├── histogram.rs
        ├── summary.rs
        ├── labels.rs
        ├── cardinality.rs
        ├── registry.rs
        ├── prometheus/
        │   ├── mod.rs
        │   ├── renderer.rs
        │   ├── format.rs
        │   └── server.rs
        ├── otel/
        │   ├── mod.rs
        │   ├── provider.rs
        │   ├── instrumentation.rs
        │   ├── propagation.rs
        │   └── config.rs
        ├── application/
        │   ├── mod.rs
        │   ├── processor.rs
        │   ├── window.rs
        │   ├── state.rs
        │   └── kafka.rs
        ├── tests/
        │   ├── counter_test.rs
        │   ├── gauge_test.rs
        │   ├── histogram_test.rs
        │   ├── prometheus_format_test.rs
        │   └── otel_integration_test.rs
        ├── benches/
        │   ├── counter_bench.rs
        │   ├── histogram_bench.rs
        │   └── label_bench.rs
        └── examples/
            ├── basic_metrics.rs
            ├── custom_buckets.rs
            └── otel_tracing.rs
```

---

## Dependencies to Add

Update `crates/processor/Cargo.toml`:

```toml
[dependencies]
# Existing dependencies...

# Metrics (already in workspace)
prometheus-client = { workspace = true }
opentelemetry = { workspace = true }
opentelemetry-otlp = { workspace = true }
opentelemetry_sdk = { workspace = true }

# HTTP server
axum = { workspace = true }
tower = { workspace = true }
tower-http = { workspace = true }

# Utilities
dashmap = { workspace = true }
```

---

## Summary

This implementation plan provides:

1. **Complete file structure** for metrics infrastructure
2. **Code scaffolding** for core components
3. **Phased approach** with clear milestones
4. **Testing strategy** at each phase
5. **Performance targets** and validation

The architecture is ready for implementation, starting with Phase 1.
