# Rust Technical Dependencies Guide
## LLM-Auto-Optimizer Project

**Version:** 1.0
**Last Updated:** 2025-11-09
**Target Rust Version:** 1.75+

---

## Table of Contents
1. [Async Scheduling & Runtime](#1-async-scheduling--runtime)
2. [Data Analysis & Statistics](#2-data-analysis--statistics)
3. [Configuration Management](#3-configuration-management)
4. [Metrics & Telemetry](#4-metrics--telemetry)
5. [State Management & Storage](#5-state-management--storage)
6. [HTTP & API Integration](#6-http--api-integration)
7. [Recommended Cargo.toml](#7-recommended-cargotoml)
8. [Integration Patterns](#8-integration-patterns)

---

## 1. Async Scheduling & Runtime

### 1.1 Primary Recommendations

#### **tokio** (v1.40+)
**Recommendation:** PRIMARY CHOICE for async runtime

```toml
tokio = { version = "1.40", features = ["full"] }
# Or minimal feature set:
tokio = { version = "1.40", features = ["rt-multi-thread", "macros", "time", "sync"] }
```

**Justification:**
- Industry standard with massive ecosystem support
- Excellent performance for multi-threaded workloads
- Work-stealing scheduler ideal for CPU-bound optimization tasks
- Superior debugging tools (tokio-console)
- Best compatibility with other crates

**Performance Characteristics:**
- Work-stealing scheduler: ~500ns task spawn overhead
- Excellent scalability: Linear up to 32+ cores
- Memory efficient: ~2KB per task

**Integration Pattern:**
```rust
use tokio::runtime::Runtime;

#[tokio::main]
async fn main() {
    // Multi-threaded runtime automatically configured
    start_optimizer().await;
}

// For custom runtime configuration:
fn custom_runtime() -> Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .thread_name("llm-optimizer")
        .enable_all()
        .build()
        .unwrap()
}
```

#### **tokio-cron-scheduler** (v0.10+)
**Recommendation:** PRIMARY for scheduling optimization runs

```toml
tokio-cron-scheduler = "0.10"
```

**Justification:**
- Native tokio integration
- Full cron syntax support
- Built-in timezone handling
- Job metadata and error handling

**Alternative:**
```toml
# For simpler use cases:
tokio-schedule = "0.3"  # Lighter weight, less features
```

**Integration Pattern:**
```rust
use tokio_cron_scheduler::{JobScheduler, Job};

async fn setup_optimization_scheduler() -> Result<JobScheduler, Error> {
    let scheduler = JobScheduler::new().await?;

    // Run optimization every hour
    scheduler.add(
        Job::new_async("0 0 * * * *", |_uuid, _lock| {
            Box::pin(async move {
                run_optimization_cycle().await;
            })
        })?
    ).await?;

    // Run metric collection every 5 minutes
    scheduler.add(
        Job::new_async("0 */5 * * * *", |_uuid, _lock| {
            Box::pin(async move {
                collect_metrics().await;
            })
        })?
    ).await?;

    scheduler.start().await?;
    Ok(scheduler)
}
```

### 1.2 Rate Limiting & Throttling

#### **governor** (v0.6+)
**Recommendation:** PRIMARY for rate limiting

```toml
governor = "0.6"
```

**Justification:**
- Token bucket and leaky bucket algorithms
- Lock-free implementation
- Built-in jitter support
- Excellent performance: <100ns per check

**Integration Pattern:**
```rust
use governor::{Quota, RateLimiter};
use std::num::NonZeroU32;

// Rate limit LLM API calls
let limiter = RateLimiter::direct(
    Quota::per_minute(NonZeroU32::new(60).unwrap())
);

async fn call_llm_api() -> Result<Response, Error> {
    limiter.until_ready().await;
    // Make API call
    Ok(make_request().await?)
}
```

**Alternative:**
```toml
# For distributed rate limiting:
redis-rate-limiter = "0.3"  # Requires Redis
```

#### **leaky-bucket** (v1.0+)
**Recommendation:** ALTERNATIVE for advanced throttling

```toml
leaky-bucket = "1.0"
```

**Trade-offs:**
- More flexible than governor
- Better for bursty traffic
- Slightly higher overhead (~200ns)

### 1.3 Background Task Management

#### **tokio-util** (v0.7+)
**Recommendation:** ESSENTIAL utility crate

```toml
tokio-util = { version = "0.7", features = ["full"] }
```

**Features:**
- Task tracking and cancellation
- Codec utilities for streaming
- Time-based utilities

**Integration Pattern:**
```rust
use tokio_util::task::TaskTracker;
use tokio::time::{timeout, Duration};

async fn manage_optimization_tasks() {
    let tracker = TaskTracker::new();

    // Spawn tracked background tasks
    tracker.spawn(async {
        continuous_monitoring().await;
    });

    tracker.spawn(async {
        periodic_analysis().await;
    });

    // Wait for all tasks with timeout
    let _ = timeout(
        Duration::from_secs(300),
        tracker.wait()
    ).await;
}
```

---

## 2. Data Analysis & Statistics

### 2.1 Statistical Analysis

#### **statrs** (v0.17+)
**Recommendation:** PRIMARY for statistical functions

```toml
statrs = "0.17"
```

**Justification:**
- Comprehensive statistical distributions
- Hypothesis testing
- Regression analysis
- Well-tested, numerically stable

**Capabilities:**
- Distributions: Normal, Beta, Gamma, Chi-squared, T-distribution
- Statistics: Mean, variance, skewness, kurtosis
- Regression: Linear, polynomial
- Hypothesis tests: t-test, chi-squared test

**Integration Pattern:**
```rust
use statrs::distribution::{Normal, ContinuousCDF};
use statrs::statistics::{Statistics, OrderStatistics};

fn analyze_latency_distribution(latencies: &[f64]) -> AnalysisResult {
    // Calculate basic statistics
    let mean = latencies.mean();
    let std_dev = latencies.std_dev();

    // Fit normal distribution
    let dist = Normal::new(mean, std_dev).unwrap();

    // Calculate percentiles
    let p95 = latencies.percentile(95);
    let p99 = latencies.percentile(99);

    // Detect anomalies (3-sigma rule)
    let anomalies: Vec<f64> = latencies.iter()
        .filter(|&&x| (x - mean).abs() > 3.0 * std_dev)
        .copied()
        .collect();

    AnalysisResult {
        mean,
        std_dev,
        p95,
        p99,
        anomalies,
    }
}
```

#### **ndarray** (v0.15+) + **ndarray-stats** (v0.5+)
**Recommendation:** For advanced numerical operations

```toml
ndarray = "0.15"
ndarray-stats = "0.5"
```

**Justification:**
- N-dimensional arrays (like NumPy)
- SIMD optimizations
- Linear algebra operations
- Essential for ML pipelines

**Integration Pattern:**
```rust
use ndarray::prelude::*;
use ndarray_stats::QuantileExt;

fn compute_metric_trends(data: Vec<Vec<f64>>) -> Array2<f64> {
    let arr = Array2::from_shape_vec(
        (data.len(), data[0].len()),
        data.into_iter().flatten().collect()
    ).unwrap();

    // Compute rolling average
    let window_size = 10;
    // ... implementation

    arr
}
```

### 2.2 Time-Series Analysis

#### **time-series** (v0.4+)
**Recommendation:** For basic time-series operations

```toml
time-series = "0.4"
```

**Alternative - Build Custom:**
For LLM optimization, custom time-series logic may be better:

```rust
use std::collections::VecDeque;

struct TimeSeriesWindow<T> {
    data: VecDeque<(std::time::Instant, T)>,
    window_duration: std::time::Duration,
}

impl<T: Clone> TimeSeriesWindow<T> {
    fn new(window_duration: std::time::Duration) -> Self {
        Self {
            data: VecDeque::new(),
            window_duration,
        }
    }

    fn push(&mut self, value: T) {
        let now = std::time::Instant::now();
        self.data.push_back((now, value));
        self.evict_old();
    }

    fn evict_old(&mut self) {
        let cutoff = std::time::Instant::now() - self.window_duration;
        while let Some((timestamp, _)) = self.data.front() {
            if *timestamp < cutoff {
                self.data.pop_front();
            } else {
                break;
            }
        }
    }

    fn values(&self) -> Vec<T> {
        self.data.iter().map(|(_, v)| v.clone()).collect()
    }
}
```

### 2.3 Anomaly Detection

#### **changepoint** (v0.3+)
**Recommendation:** For detecting regime changes

```toml
changepoint = "0.3"
```

**Use Case:** Detect when LLM performance characteristics change

**Alternative - Custom Implementation:**
```rust
use statrs::statistics::Statistics;

fn detect_anomalies_zscore(
    values: &[f64],
    threshold: f64
) -> Vec<usize> {
    let mean = values.mean();
    let std_dev = values.std_dev();

    values.iter()
        .enumerate()
        .filter_map(|(i, &v)| {
            let z_score = (v - mean).abs() / std_dev;
            if z_score > threshold {
                Some(i)
            } else {
                None
            }
        })
        .collect()
}

// Exponential Moving Average for trend detection
struct EMA {
    alpha: f64,
    value: Option<f64>,
}

impl EMA {
    fn new(alpha: f64) -> Self {
        Self { alpha, value: None }
    }

    fn update(&mut self, new_value: f64) -> f64 {
        match self.value {
            None => {
                self.value = Some(new_value);
                new_value
            }
            Some(old) => {
                let new = self.alpha * new_value + (1.0 - self.alpha) * old;
                self.value = Some(new);
                new
            }
        }
    }
}
```

### 2.4 Machine Learning

#### **linfa** (v0.7+)
**Recommendation:** PRIMARY ML ecosystem for Rust

```toml
linfa = "0.7"
linfa-clustering = "0.7"  # K-means, DBSCAN
linfa-reduction = "0.7"   # PCA, dimensionality reduction
```

**Justification:**
- Pure Rust, no Python dependencies
- Scikit-learn inspired API
- Good for lightweight ML tasks
- Clustering for workload classification

**Integration Pattern:**
```rust
use linfa::prelude::*;
use linfa_clustering::KMeans;
use ndarray::Array2;

fn cluster_llm_workloads(features: Array2<f64>) -> Vec<usize> {
    let dataset = DatasetBase::from(features);

    // Cluster into 3 workload types: light, medium, heavy
    let model = KMeans::params(3)
        .max_n_iterations(200)
        .tolerance(1e-4)
        .fit(&dataset)
        .unwrap();

    model.predict(&dataset).to_vec()
}
```

**Alternative: smartcore** (v0.3+)
```toml
smartcore = "0.3"
```

**Trade-offs:**
- More algorithms than linfa
- Less idiomatic Rust API
- Better for complex ML pipelines
- Higher compilation time

---

## 3. Configuration Management

### 3.1 Configuration Parsing

#### **figment** (v0.10+)
**Recommendation:** PRIMARY for complex configuration

```toml
figment = { version = "0.10", features = ["toml", "json", "yaml", "env"] }
```

**Justification:**
- Provider-based architecture (files, env vars, etc.)
- Hierarchical configuration merging
- Type-safe with serde
- Excellent error messages
- Profile support (dev, prod, test)

**Integration Pattern:**
```rust
use figment::{Figment, providers::{Format, Toml, Json, Env}};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct OptimizerConfig {
    optimization: OptimizationConfig,
    metrics: MetricsConfig,
    llm: LLMConfig,
}

#[derive(Debug, Deserialize, Serialize)]
struct OptimizationConfig {
    #[serde(default = "default_schedule")]
    schedule: String,
    #[serde(default = "default_window_size")]
    analysis_window_hours: u32,
    enabled_strategies: Vec<String>,
}

fn default_schedule() -> String { "0 0 * * * *".to_string() }
fn default_window_size() -> u32 { 24 }

fn load_config() -> Result<OptimizerConfig, figment::Error> {
    Figment::new()
        .merge(Toml::file("config/default.toml"))
        .merge(Toml::file("config/production.toml"))
        .merge(Env::prefixed("OPTIMIZER_"))
        .extract()
}
```

**Alternative: config-rs** (v0.14+)
```toml
config = "0.14"
```

**Trade-offs:**
- Simpler API than figment
- Less flexible merging
- Good for basic use cases
- Slightly better performance

### 3.2 Dynamic Configuration

#### **hot-config** (Custom or use **notify** v6.0+)
**Recommendation:** Use **notify** for file watching + custom reload logic

```toml
notify = "6.0"
```

**Integration Pattern:**
```rust
use notify::{Watcher, RecursiveMode, Event};
use tokio::sync::RwLock;
use std::sync::Arc;

struct DynamicConfig {
    config: Arc<RwLock<OptimizerConfig>>,
}

impl DynamicConfig {
    async fn watch_and_reload(self: Arc<Self>) {
        let (tx, mut rx) = tokio::sync::mpsc::channel(100);

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
            if let Ok(event) = res {
                let _ = tx.blocking_send(event);
            }
        }).unwrap();

        watcher.watch(
            Path::new("config/"),
            RecursiveMode::Recursive
        ).unwrap();

        while let Some(_event) = rx.recv().await {
            // Debounce and reload
            tokio::time::sleep(Duration::from_millis(500)).await;

            if let Ok(new_config) = load_config() {
                let mut config = self.config.write().await;
                *config = new_config;
                tracing::info!("Configuration reloaded");
            }
        }
    }
}
```

### 3.3 Feature Flags & A/B Testing

#### **flippers** (v0.4+) or Custom Implementation
**Recommendation:** Build custom feature flag system

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FeatureFlags {
    flags: HashMap<String, FeatureFlag>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FeatureFlag {
    enabled: bool,
    #[serde(default)]
    rollout_percentage: f64,  // 0.0 to 100.0
    #[serde(default)]
    conditions: Vec<Condition>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Condition {
    Environment { values: Vec<String> },
    UserSegment { segment: String },
    Random { percentage: f64 },
}

impl FeatureFlags {
    pub fn is_enabled(&self, flag_name: &str, context: &Context) -> bool {
        if let Some(flag) = self.flags.get(flag_name) {
            if !flag.enabled {
                return false;
            }

            // Check conditions
            for condition in &flag.conditions {
                if !self.check_condition(condition, context) {
                    return false;
                }
            }

            // Check rollout percentage
            if flag.rollout_percentage < 100.0 {
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                context.user_id.hash(&mut hasher);
                flag_name.hash(&mut hasher);
                let hash = hasher.finish();
                let percentage = (hash % 10000) as f64 / 100.0;
                return percentage < flag.rollout_percentage;
            }

            true
        } else {
            false
        }
    }

    fn check_condition(&self, condition: &Condition, context: &Context) -> bool {
        match condition {
            Condition::Environment { values } => {
                values.contains(&context.environment)
            }
            Condition::UserSegment { segment } => {
                context.segments.contains(segment)
            }
            Condition::Random { percentage } => {
                use rand::Rng;
                rand::thread_rng().gen::<f64>() * 100.0 < *percentage
            }
        }
    }
}

pub struct Context {
    user_id: String,
    environment: String,
    segments: Vec<String>,
}
```

### 3.4 Environment-Based Config

**Integration with figment:**
```rust
use figment::{Figment, providers::{Format, Toml, Env}};

fn load_environment_config() -> Result<OptimizerConfig, figment::Error> {
    let env = std::env::var("OPTIMIZER_ENV")
        .unwrap_or_else(|_| "development".to_string());

    Figment::new()
        // Load base config
        .merge(Toml::file("config/default.toml"))
        // Load environment-specific config
        .merge(Toml::file(format!("config/{}.toml", env)))
        // Override with environment variables
        .merge(Env::prefixed("OPTIMIZER_").split("__"))
        .extract()
}
```

**Example config/production.toml:**
```toml
[optimization]
schedule = "0 0 */2 * * *"  # Every 2 hours
analysis_window_hours = 48

[metrics]
enabled = true
prometheus_port = 9090

[llm]
provider = "anthropic"
rate_limit_rpm = 50
timeout_seconds = 30
```

---

## 4. Metrics & Telemetry

### 4.1 Prometheus Integration

#### **prometheus-client** (v0.22+)
**Recommendation:** PRIMARY for Prometheus metrics

```toml
prometheus-client = "0.22"
```

**Justification:**
- Official Prometheus client
- Type-safe metric definitions
- Zero-cost abstractions
- Excellent performance: <50ns per metric update

**Integration Pattern:**
```rust
use prometheus_client::encoding::text::encode;
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::histogram::Histogram;
use prometheus_client::registry::Registry;

#[derive(Clone, Hash, PartialEq, Eq)]
struct Labels {
    optimization_type: String,
    model: String,
}

struct OptimizerMetrics {
    registry: Registry,

    // Counters
    optimization_runs: Family<Labels, Counter>,
    optimization_errors: Family<Labels, Counter>,

    // Histograms
    optimization_duration: Family<Labels, Histogram>,
    cost_reduction: Family<Labels, Histogram>,

    // Gauges
    active_experiments: prometheus_client::metrics::gauge::Gauge,
}

impl OptimizerMetrics {
    fn new() -> Self {
        let mut registry = Registry::default();

        let optimization_runs = Family::<Labels, Counter>::default();
        registry.register(
            "optimizer_runs_total",
            "Total number of optimization runs",
            optimization_runs.clone(),
        );

        let optimization_duration = Family::<Labels, Histogram>::new_with_constructor(
            || Histogram::new(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0].into_iter())
        );
        registry.register(
            "optimizer_duration_seconds",
            "Duration of optimization runs in seconds",
            optimization_duration.clone(),
        );

        let active_experiments = prometheus_client::metrics::gauge::Gauge::default();
        registry.register(
            "optimizer_active_experiments",
            "Number of active experiments",
            active_experiments.clone(),
        );

        Self {
            registry,
            optimization_runs,
            optimization_errors: Family::default(),
            optimization_duration,
            cost_reduction: Family::default(),
            active_experiments,
        }
    }

    fn record_optimization(&self, labels: Labels, duration: f64, cost_saved: f64) {
        self.optimization_runs.get_or_create(&labels).inc();
        self.optimization_duration.get_or_create(&labels).observe(duration);
        self.cost_reduction.get_or_create(&labels).observe(cost_saved);
    }

    fn export(&self) -> String {
        let mut buffer = String::new();
        encode(&mut buffer, &self.registry).unwrap();
        buffer
    }
}

// HTTP endpoint for Prometheus scraping
use axum::{routing::get, Router};

async fn metrics_handler(
    metrics: Arc<OptimizerMetrics>
) -> String {
    metrics.export()
}

fn create_metrics_server(metrics: Arc<OptimizerMetrics>) -> Router {
    Router::new()
        .route("/metrics", get(move || {
            let metrics = metrics.clone();
            async move { metrics_handler(metrics).await }
        }))
}
```

### 4.2 OpenTelemetry

#### **opentelemetry** (v0.24+) + **opentelemetry-otlp** (v0.17+)
**Recommendation:** For distributed tracing and advanced telemetry

```toml
opentelemetry = { version = "0.24", features = ["trace", "metrics"] }
opentelemetry-otlp = "0.17"
opentelemetry-semantic-conventions = "0.16"
```

**Integration Pattern:**
```rust
use opentelemetry::trace::{Tracer, TracerProvider};
use opentelemetry::global;
use opentelemetry_otlp::WithExportConfig;

fn init_telemetry() -> Result<(), Box<dyn std::error::Error>> {
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint("http://localhost:4317")
        )
        .install_batch(opentelemetry::runtime::Tokio)?;

    global::set_tracer_provider(tracer);

    Ok(())
}

// Usage with tracing crate integration
use tracing::{info, span, Level};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::layer::SubscriberExt;

fn setup_tracing() {
    let tracer = global::tracer("llm-optimizer");
    let telemetry = OpenTelemetryLayer::new(tracer);

    let subscriber = tracing_subscriber::Registry::default()
        .with(telemetry);

    tracing::subscriber::set_global_default(subscriber).unwrap();
}

async fn optimize_workflow() {
    let span = span!(Level::INFO, "optimize_workflow");
    let _enter = span.enter();

    info!("Starting optimization workflow");
    // ... work ...
}
```

### 4.3 Structured Logging

#### **tracing** (v0.1+) + **tracing-subscriber** (v0.3+)
**Recommendation:** ESSENTIAL for logging

```toml
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
tracing-appender = "0.2"
```

**Justification:**
- Structured, contextual logging
- Async-aware
- Excellent ecosystem integration
- Performance: <100ns per log statement (when filtered out)

**Integration Pattern:**
```rust
use tracing::{info, warn, error, debug};
use tracing_subscriber::{fmt, EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use tracing_appender::rolling::{RollingFileAppender, Rotation};

fn setup_logging() {
    let file_appender = RollingFileAppender::new(
        Rotation::DAILY,
        "logs",
        "optimizer.log"
    );

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "info,llm_optimizer=debug".into()))
        .with(fmt::layer()
            .with_writer(std::io::stdout)
            .with_target(true)
            .with_thread_ids(true))
        .with(fmt::layer()
            .with_writer(file_appender)
            .json())
        .init();
}

// Usage
async fn run_optimization() {
    info!(
        optimization_type = "prompt_optimization",
        model = "claude-3-5-sonnet",
        "Starting optimization run"
    );

    let start = std::time::Instant::now();

    match perform_optimization().await {
        Ok(result) => {
            info!(
                duration_ms = start.elapsed().as_millis(),
                cost_saved = result.cost_saved,
                "Optimization completed successfully"
            );
        }
        Err(e) => {
            error!(
                error = ?e,
                duration_ms = start.elapsed().as_millis(),
                "Optimization failed"
            );
        }
    }
}
```

### 4.4 Custom Metrics Collection

**Efficient in-memory metrics aggregation:**
```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::collections::HashMap;
use parking_lot::RwLock;

pub struct MetricsCollector {
    counters: RwLock<HashMap<String, AtomicU64>>,
    histograms: RwLock<HashMap<String, Histogram>>,
}

impl MetricsCollector {
    pub fn increment(&self, name: &str) {
        let counters = self.counters.read();
        if let Some(counter) = counters.get(name) {
            counter.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn record(&self, name: &str, value: f64) {
        let mut histograms = self.histograms.write();
        histograms.entry(name.to_string())
            .or_insert_with(Histogram::new)
            .record(value);
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        // Create immutable snapshot for export
        MetricsSnapshot::new(&self.counters.read(), &self.histograms.read())
    }
}
```

---

## 5. State Management & Storage

### 5.1 Embedded Databases

#### **sled** (v0.34+)
**Recommendation:** PRIMARY for persistent state storage

```toml
sled = "0.34"
```

**Justification:**
- Pure Rust embedded database
- ACID transactions
- Zero-copy reads
- Excellent performance: ~1μs reads, ~10μs writes
- Crash-safe
- No external dependencies

**Integration Pattern:**
```rust
use sled::{Db, IVec};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct OptimizationState {
    last_run: std::time::SystemTime,
    active_experiments: Vec<String>,
    performance_baseline: HashMap<String, f64>,
}

struct StateStore {
    db: Db,
}

impl StateStore {
    fn new(path: &str) -> Result<Self, sled::Error> {
        let db = sled::open(path)?;
        Ok(Self { db })
    }

    fn save_state(&self, key: &str, state: &OptimizationState) -> Result<(), Box<dyn std::error::Error>> {
        let bytes = bincode::serialize(state)?;
        self.db.insert(key, bytes)?;
        self.db.flush()?;
        Ok(())
    }

    fn load_state(&self, key: &str) -> Result<Option<OptimizationState>, Box<dyn std::error::Error>> {
        if let Some(bytes) = self.db.get(key)? {
            let state = bincode::deserialize(&bytes)?;
            Ok(Some(state))
        } else {
            Ok(None)
        }
    }

    // Store time-series metrics
    fn append_metric(&self, metric_name: &str, timestamp: u64, value: f64) -> Result<(), sled::Error> {
        let tree = self.db.open_tree(metric_name)?;
        let key = timestamp.to_be_bytes();
        let value_bytes = value.to_le_bytes();
        tree.insert(key, value_bytes.as_ref())?;
        Ok(())
    }

    fn get_metrics_range(
        &self,
        metric_name: &str,
        start_ts: u64,
        end_ts: u64
    ) -> Result<Vec<(u64, f64)>, sled::Error> {
        let tree = self.db.open_tree(metric_name)?;
        let start_key = start_ts.to_be_bytes();
        let end_key = end_ts.to_be_bytes();

        let results: Vec<(u64, f64)> = tree
            .range(start_key..=end_key)
            .filter_map(|r| r.ok())
            .map(|(k, v)| {
                let ts = u64::from_be_bytes(k.as_ref().try_into().unwrap());
                let val = f64::from_le_bytes(v.as_ref().try_into().unwrap());
                (ts, val)
            })
            .collect();

        Ok(results)
    }
}
```

**Alternative: redb** (v2.0+)
```toml
redb = "2.0"
```

**Trade-offs:**
- Simpler API than sled
- Better type safety
- Slightly better write performance
- Less mature ecosystem

#### **rocksdb** (v0.22+)
**Recommendation:** For high-throughput scenarios

```toml
rocksdb = "0.22"
```

**Trade-offs:**
- Higher performance than sled for write-heavy workloads
- More complex configuration
- Larger binary size
- C++ dependency (requires compilation)

**Use Case:** If storing >100k metrics/hour

### 5.2 In-Memory Stores

#### **dashmap** (v6.0+)
**Recommendation:** PRIMARY for concurrent in-memory state

```toml
dashmap = "6.0"
```

**Justification:**
- Sharded concurrent HashMap
- Lock-free reads
- Excellent performance: <50ns reads, ~100ns writes
- Drop-in replacement for RwLock<HashMap>

**Integration Pattern:**
```rust
use dashmap::DashMap;
use std::sync::Arc;

pub struct LiveMetrics {
    current_values: Arc<DashMap<String, f64>>,
    aggregates: Arc<DashMap<String, Aggregate>>,
}

#[derive(Clone)]
struct Aggregate {
    count: u64,
    sum: f64,
    min: f64,
    max: f64,
}

impl LiveMetrics {
    pub fn new() -> Self {
        Self {
            current_values: Arc::new(DashMap::new()),
            aggregates: Arc::new(DashMap::new()),
        }
    }

    pub fn record(&self, key: String, value: f64) {
        self.current_values.insert(key.clone(), value);

        self.aggregates
            .entry(key)
            .and_modify(|agg| {
                agg.count += 1;
                agg.sum += value;
                agg.min = agg.min.min(value);
                agg.max = agg.max.max(value);
            })
            .or_insert(Aggregate {
                count: 1,
                sum: value,
                min: value,
                max: value,
            });
    }

    pub fn get_average(&self, key: &str) -> Option<f64> {
        self.aggregates.get(key).map(|agg| agg.sum / agg.count as f64)
    }

    pub fn snapshot(&self) -> HashMap<String, f64> {
        self.current_values
            .iter()
            .map(|entry| (entry.key().clone(), *entry.value()))
            .collect()
    }
}
```

#### **moka** (v0.12+)
**Recommendation:** For caching with TTL and eviction policies

```toml
moka = { version = "0.12", features = ["future"] }
```

**Justification:**
- LRU/LFU/TinyLFU eviction policies
- TTL support
- Async-aware
- High performance: ~100ns cache hits

**Integration Pattern:**
```rust
use moka::future::Cache;
use std::time::Duration;

pub struct OptimizationCache {
    results: Cache<String, OptimizationResult>,
}

impl OptimizationCache {
    pub fn new() -> Self {
        let results = Cache::builder()
            .max_capacity(10_000)
            .time_to_live(Duration::from_hours(24))
            .time_to_idle(Duration::from_hours(2))
            .build();

        Self { results }
    }

    pub async fn get_or_compute(
        &self,
        key: String,
        compute: impl Future<Output = OptimizationResult>
    ) -> OptimizationResult {
        self.results
            .get_or_insert_with(key, async { compute.await })
            .await
    }
}
```

### 5.3 Serialization

#### **serde** (v1.0+) + **bincode** (v1.3+)
**Recommendation:** PRIMARY serialization stack

```toml
serde = { version = "1.0", features = ["derive"] }
bincode = "1.3"
```

**Justification:**
- De facto standard
- Zero-copy deserialization
- Excellent performance: ~10ns/field
- bincode is compact and fast

**Integration Pattern:**
```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExperimentState {
    pub id: String,
    pub created_at: u64,
    pub status: ExperimentStatus,
    pub metrics: Vec<MetricPoint>,
    pub config: serde_json::Value,  // Flexible config
}

// Serialize to bytes
let bytes = bincode::serialize(&state)?;

// Deserialize from bytes
let state: ExperimentState = bincode::deserialize(&bytes)?;
```

**Alternative for JSON:**
```toml
serde_json = "1.0"
```

**Alternative for MessagePack:**
```toml
rmp-serde = "1.1"  # More compact than JSON, faster than bincode for nested data
```

#### **prost** (v0.12+)
**Recommendation:** For Protocol Buffers (if needed for gRPC)

```toml
prost = "0.12"
prost-types = "0.12"
```

### 5.4 State Persistence Strategies

**Snapshot + WAL Pattern:**
```rust
use sled::Db;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct PersistentState<T> {
    in_memory: Arc<RwLock<T>>,
    db: Db,
    snapshot_key: String,
}

impl<T> PersistentState<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Clone,
{
    pub async fn new(db_path: &str, snapshot_key: String) -> Result<Self, Box<dyn std::error::Error>> {
        let db = sled::open(db_path)?;

        // Load from snapshot
        let in_memory = if let Some(bytes) = db.get(&snapshot_key)? {
            Arc::new(RwLock::new(bincode::deserialize(&bytes)?))
        } else {
            Arc::new(RwLock::new(T::default()))
        };

        Ok(Self {
            in_memory,
            db,
            snapshot_key,
        })
    }

    pub async fn update<F>(&self, f: F) -> Result<(), Box<dyn std::error::Error>>
    where
        F: FnOnce(&mut T),
    {
        let mut state = self.in_memory.write().await;
        f(&mut state);

        // Persist immediately (or batch for performance)
        let bytes = bincode::serialize(&*state)?;
        self.db.insert(&self.snapshot_key, bytes)?;

        Ok(())
    }

    pub async fn read<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        let state = self.in_memory.read().await;
        f(&*state)
    }

    pub async fn snapshot(&self) -> Result<(), Box<dyn std::error::Error>> {
        let state = self.in_memory.read().await;
        let bytes = bincode::serialize(&*state)?;
        self.db.insert(&self.snapshot_key, bytes)?;
        self.db.flush_async().await?;
        Ok(())
    }
}
```

---

## 6. HTTP & API Integration

### 6.1 HTTP Clients

#### **reqwest** (v0.12+)
**Recommendation:** PRIMARY HTTP client

```toml
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
```

**Justification:**
- High-level, ergonomic API
- Async support with tokio
- Connection pooling
- Automatic retries and timeouts
- TLS support

**Integration Pattern:**
```rust
use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize)]
struct LLMRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: u32,
}

#[derive(Deserialize)]
struct LLMResponse {
    id: String,
    content: Vec<ContentBlock>,
    usage: Usage,
}

pub struct LLMClient {
    client: Client,
    api_key: String,
    base_url: String,
}

impl LLMClient {
    pub fn new(api_key: String) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .pool_max_idle_per_host(10)
            .tcp_keepalive(Duration::from_secs(30))
            .build()?;

        Ok(Self {
            client,
            api_key,
            base_url: "https://api.anthropic.com/v1".to_string(),
        })
    }

    pub async fn send_request(
        &self,
        request: LLMRequest
    ) -> Result<LLMResponse, Box<dyn std::error::Error>> {
        let response = self.client
            .post(format!("{}/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header(header::CONTENT_TYPE, "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("API error: {}", error_text).into());
        }

        let llm_response: LLMResponse = response.json().await?;
        Ok(llm_response)
    }
}
```

**Advanced: Retry logic with exponential backoff:**
```rust
use tokio::time::{sleep, Duration};

async fn send_with_retry<T, F, Fut>(
    mut operation: F,
    max_retries: u32
) -> Result<T, Box<dyn std::error::Error>>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, Box<dyn std::error::Error>>>,
{
    let mut retries = 0;

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if retries < max_retries => {
                retries += 1;
                let backoff = Duration::from_millis(100 * 2u64.pow(retries - 1));
                tracing::warn!(
                    error = ?e,
                    retry = retries,
                    backoff_ms = backoff.as_millis(),
                    "Request failed, retrying"
                );
                sleep(backoff).await;
            }
            Err(e) => return Err(e),
        }
    }
}
```

#### **hyper** (v1.0+)
**Recommendation:** For low-level HTTP or building custom clients

```toml
hyper = { version = "1.0", features = ["full"] }
hyper-util = "0.1"
```

**Trade-offs:**
- Lower-level than reqwest
- More control over connections
- Better for custom protocols
- More complex API

**Use Case:** Building custom proxy or middleware

### 6.2 gRPC

#### **tonic** (v0.12+)
**Recommendation:** PRIMARY for gRPC

```toml
tonic = "0.12"
prost = "0.12"
tonic-build = "0.12"  # Build dependency
```

**Justification:**
- Native async/await support
- Generated code from .proto files
- Streaming support
- Excellent performance

**Integration Pattern:**

**build.rs:**
```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("proto/optimizer.proto")?;
    Ok(())
}
```

**proto/optimizer.proto:**
```protobuf
syntax = "proto3";

package optimizer;

service OptimizerService {
  rpc RunOptimization (OptimizationRequest) returns (OptimizationResponse);
  rpc StreamMetrics (MetricsRequest) returns (stream MetricPoint);
}

message OptimizationRequest {
  string model = 1;
  string optimization_type = 2;
}

message OptimizationResponse {
  string job_id = 1;
  string status = 2;
}
```

**Implementation:**
```rust
use tonic::{transport::Server, Request, Response, Status};
use optimizer::optimizer_service_server::{OptimizerService, OptimizerServiceServer};

pub struct OptimizerServiceImpl {
    // ... state
}

#[tonic::async_trait]
impl OptimizerService for OptimizerServiceImpl {
    async fn run_optimization(
        &self,
        request: Request<OptimizationRequest>,
    ) -> Result<Response<OptimizationResponse>, Status> {
        let req = request.into_inner();

        // Start optimization
        let job_id = start_optimization_job(req.model, req.optimization_type).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(OptimizationResponse {
            job_id,
            status: "started".to_string(),
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let service = OptimizerServiceImpl::new();

    Server::builder()
        .add_service(OptimizerServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
```

### 6.3 WebSocket Support

#### **tokio-tungstenite** (v0.21+)
**Recommendation:** For WebSocket clients

```toml
tokio-tungstenite = "0.21"
```

**Integration Pattern:**
```rust
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};

async fn stream_real_time_metrics() -> Result<(), Box<dyn std::error::Error>> {
    let (ws_stream, _) = connect_async("ws://localhost:8080/metrics").await?;
    let (mut write, mut read) = ws_stream.split();

    // Send subscription message
    write.send(Message::Text(r#"{"subscribe": "all"}"#.to_string())).await?;

    // Receive metrics
    while let Some(msg) = read.next().await {
        match msg? {
            Message::Text(text) => {
                let metric: MetricPoint = serde_json::from_str(&text)?;
                process_metric(metric).await;
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    Ok(())
}
```

### 6.4 API Client Generation

#### **openapi-generator** + **reqwest**
**Recommendation:** Generate clients from OpenAPI specs

```bash
# Install openapi-generator-cli
npm install @openapitools/openapi-generator-cli -g

# Generate Rust client
openapi-generator-cli generate \
  -i api-spec.yaml \
  -g rust \
  -o generated/api-client
```

**Alternative: progenitor** (for OpenAPI)
```toml
progenitor = "0.5"
```

---

## 7. Recommended Cargo.toml

```toml
[package]
name = "llm-auto-optimizer"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"

[dependencies]
# Async Runtime
tokio = { version = "1.40", features = ["rt-multi-thread", "macros", "time", "sync", "signal"] }
tokio-util = { version = "0.7", features = ["rt", "time"] }
tokio-cron-scheduler = "0.10"

# Rate Limiting
governor = "0.6"

# HTTP & API
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
axum = "0.7"  # For serving metrics endpoints
tower = "0.4"  # Middleware
tower-http = { version = "0.5", features = ["trace", "timeout"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
bincode = "1.3"

# Configuration
figment = { version = "0.10", features = ["toml", "json", "env"] }

# Metrics & Telemetry
prometheus-client = "0.22"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
tracing-appender = "0.2"

# Statistics & Analysis
statrs = "0.17"
ndarray = "0.15"
ndarray-stats = "0.5"

# Machine Learning (optional)
linfa = { version = "0.7", optional = true }
linfa-clustering = { version = "0.7", optional = true }

# State Management
sled = "0.34"
dashmap = "6.0"
moka = { version = "0.12", features = ["future"] }
parking_lot = "0.12"  # Better RwLock

# Error Handling
anyhow = "1.0"
thiserror = "1.0"

# Utilities
chrono = "0.4"
uuid = { version = "1.6", features = ["v4", "serde"] }
rand = "0.8"

# File Watching (for dynamic config)
notify = "6.0"

[dev-dependencies]
criterion = "0.5"  # Benchmarking
mockito = "1.2"    # HTTP mocking for tests
proptest = "1.4"   # Property-based testing

[features]
default = ["ml"]
ml = ["linfa", "linfa-clustering"]
grpc = ["tonic", "prost"]

[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true

[profile.bench]
inherits = "release"
```

---

## 8. Integration Patterns

### 8.1 Complete Application Structure

```rust
use tokio::sync::RwLock;
use std::sync::Arc;

pub struct OptimizerApp {
    config: Arc<RwLock<OptimizerConfig>>,
    state: Arc<StateStore>,
    metrics: Arc<OptimizerMetrics>,
    llm_client: Arc<LLMClient>,
    scheduler: JobScheduler,
}

impl OptimizerApp {
    pub async fn new(config_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // Load configuration
        let config = Arc::new(RwLock::new(load_config()?));

        // Initialize state store
        let state = Arc::new(StateStore::new("data/optimizer.db")?);

        // Initialize metrics
        let metrics = Arc::new(OptimizerMetrics::new());

        // Initialize LLM client
        let api_key = std::env::var("ANTHROPIC_API_KEY")?;
        let llm_client = Arc::new(LLMClient::new(api_key)?);

        // Setup scheduler
        let scheduler = JobScheduler::new().await?;

        Ok(Self {
            config,
            state,
            metrics,
            llm_client,
            scheduler,
        })
    }

    pub async fn run(self: Arc<Self>) -> Result<(), Box<dyn std::error::Error>> {
        // Setup logging
        setup_logging();

        // Start metrics server
        let metrics_server = self.clone().start_metrics_server();

        // Start scheduler
        self.setup_scheduled_jobs().await?;

        // Start config watcher
        let config_watcher = self.clone().watch_config();

        // Wait for shutdown signal
        tokio::select! {
            _ = metrics_server => {},
            _ = config_watcher => {},
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Shutdown signal received");
            }
        }

        Ok(())
    }

    async fn setup_scheduled_jobs(self: &Arc<Self>) -> Result<(), Box<dyn std::error::Error>> {
        let app = self.clone();
        self.scheduler.add(
            Job::new_async("0 0 * * * *", move |_uuid, _lock| {
                let app = app.clone();
                Box::pin(async move {
                    if let Err(e) = app.run_optimization_cycle().await {
                        tracing::error!(error = ?e, "Optimization cycle failed");
                    }
                })
            })?
        ).await?;

        self.scheduler.start().await?;
        Ok(())
    }

    async fn run_optimization_cycle(&self) -> Result<(), Box<dyn std::error::Error>> {
        let start = std::time::Instant::now();

        tracing::info!("Starting optimization cycle");

        // Collect metrics from past window
        let metrics = self.collect_historical_metrics().await?;

        // Analyze metrics
        let analysis = self.analyze_metrics(&metrics)?;

        // Determine optimization strategy
        let strategy = self.select_strategy(&analysis);

        // Execute optimization
        let result = self.execute_optimization(strategy).await?;

        // Record results
        self.metrics.record_optimization(
            Labels {
                optimization_type: strategy.name(),
                model: "claude-3-5-sonnet".to_string(),
            },
            start.elapsed().as_secs_f64(),
            result.cost_saved,
        );

        // Persist state
        self.state.save_optimization_result(&result)?;

        tracing::info!(
            duration_ms = start.elapsed().as_millis(),
            cost_saved = result.cost_saved,
            "Optimization cycle completed"
        );

        Ok(())
    }

    async fn start_metrics_server(self: Arc<Self>) -> Result<(), Box<dyn std::error::Error>> {
        let metrics = self.metrics.clone();

        let app = Router::new()
            .route("/metrics", get(move || {
                let metrics = metrics.clone();
                async move { metrics.export() }
            }))
            .route("/health", get(|| async { "OK" }));

        let addr = "0.0.0.0:9090".parse()?;
        tracing::info!("Metrics server listening on {}", addr);

        axum::serve(
            tokio::net::TcpListener::bind(addr).await?,
            app
        ).await?;

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = Arc::new(OptimizerApp::new("config/default.toml").await?);
    app.run().await
}
```

### 8.2 Error Handling Pattern

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OptimizerError {
    #[error("Configuration error: {0}")]
    Config(#[from] figment::Error),

    #[error("Database error: {0}")]
    Database(#[from] sled::Error),

    #[error("LLM API error: {0}")]
    LLMApi(String),

    #[error("Analysis error: {0}")]
    Analysis(String),

    #[error("Insufficient data: {0}")]
    InsufficientData(String),
}

type Result<T> = std::result::Result<T, OptimizerError>;
```

### 8.3 Testing Pattern

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    #[tokio::test]
    async fn test_llm_client() {
        let mut server = Server::new_async().await;

        let mock = server.mock("POST", "/v1/messages")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "test", "content": [], "usage": {}}"#)
            .create_async()
            .await;

        let client = LLMClient::new("test-key".to_string()).unwrap();
        // Test implementation...

        mock.assert_async().await;
    }

    #[test]
    fn test_metrics_analysis() {
        let latencies = vec![100.0, 150.0, 120.0, 200.0, 5000.0];
        let result = analyze_latency_distribution(&latencies);

        assert!(result.anomalies.len() > 0);
        assert!(result.anomalies.contains(&5000.0));
    }
}
```

---

## Additional Recommendations

### Performance Monitoring
```toml
# For production profiling
pprof = { version = "0.13", features = ["flamegraph"] }
```

### Security
```toml
# For secure secret management
secrecy = "0.8"  # Zero-on-drop secret types
```

### CLI
```toml
# If building CLI tools
clap = { version = "4.4", features = ["derive"] }
```

### Async Utilities
```toml
futures = "0.3"
async-trait = "0.1"
```

---

## Version Compatibility Matrix

| Crate | Min Version | Recommended | MSRV |
|-------|-------------|-------------|------|
| tokio | 1.35 | 1.40+ | 1.70 |
| reqwest | 0.11 | 0.12+ | 1.70 |
| serde | 1.0.190 | 1.0.200+ | 1.60 |
| axum | 0.7 | 0.7+ | 1.70 |
| prometheus-client | 0.22 | 0.22+ | 1.70 |
| sled | 0.34 | 0.34 | 1.65 |
| dashmap | 5.5 | 6.0+ | 1.65 |

**Recommended Rust Version:** 1.75+ for best compatibility

---

## Conclusion

This technical dependency guide provides a comprehensive foundation for the LLM-Auto-Optimizer project. Key takeaways:

1. **Async Runtime:** tokio with tokio-cron-scheduler for robust scheduling
2. **Statistics:** statrs + ndarray for comprehensive analysis
3. **Configuration:** figment for flexible, hierarchical config management
4. **Metrics:** prometheus-client + tracing for production-grade telemetry
5. **State:** sled for persistence + dashmap for in-memory performance
6. **HTTP:** reqwest for LLM API integration with retry logic

All recommendations prioritize:
- Production readiness
- Performance (sub-microsecond operations where possible)
- Type safety
- Ecosystem maturity
- Active maintenance

For questions or clarifications on any dependency, consult the individual crate documentation or the Rust community.
