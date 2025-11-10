# SQL API Reference

**LLM Auto Optimizer - SQL Interface**
**Version:** 1.0
**Last Updated:** 2025-11-10

---

## Table of Contents

1. [Introduction](#introduction)
2. [Getting Started](#getting-started)
3. [SqlEngine API](#sqlengine-api)
4. [Query Execution](#query-execution)
5. [Result Handling](#result-handling)
6. [Error Handling](#error-handling)
7. [Integration Examples](#integration-examples)
8. [Advanced Usage](#advanced-usage)

---

## Introduction

The SQL API Reference provides comprehensive documentation for embedding the SQL Interface into Rust applications. This enables programmatic stream processing using SQL queries while maintaining full type safety and performance.

### Key Features

- **Type-safe SQL execution**: Compile-time checked query parameters
- **Streaming results**: Efficient handling of unbounded query results
- **Async/await support**: Built on Tokio for async execution
- **Connection pooling**: Efficient resource management
- **State management**: Configurable state backends
- **Error handling**: Rich error types with context

---

## Getting Started

### Installation

Add the SQL interface to your `Cargo.toml`:

```toml
[dependencies]
llm-optimizer-sql = "0.1.0"
tokio = { version = "1.0", features = ["full"] }
anyhow = "1.0"
```

### Basic Example

```rust
use llm_optimizer_sql::{SqlEngine, SqlEngineConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create SQL engine
    let config = SqlEngineConfig::default();
    let engine = SqlEngine::new(config).await?;

    // Execute query
    let query = "SELECT COUNT(*) FROM feedback_events";
    let result = engine.execute(query).await?;

    // Process results
    for row in result.rows() {
        println!("Count: {}", row.get::<i64>(0)?);
    }

    Ok(())
}
```

---

## SqlEngine API

### SqlEngine

The main entry point for SQL execution.

```rust
pub struct SqlEngine {
    // Internal state
}

impl SqlEngine {
    /// Create a new SQL engine with default configuration
    pub async fn new(config: SqlEngineConfig) -> Result<Self>;

    /// Execute a SQL query
    pub async fn execute(&self, query: &str) -> Result<QueryResult>;

    /// Execute a parameterized query
    pub async fn execute_with_params(
        &self,
        query: &str,
        params: &[SqlValue],
    ) -> Result<QueryResult>;

    /// Create a prepared statement
    pub async fn prepare(&self, query: &str) -> Result<PreparedStatement>;

    /// Get catalog information
    pub fn catalog(&self) -> &Catalog;

    /// Shutdown the engine gracefully
    pub async fn shutdown(self) -> Result<()>;
}
```

### SqlEngineConfig

Configuration for the SQL engine.

```rust
pub struct SqlEngineConfig {
    /// State backend configuration
    pub state_backend: StateBackendConfig,

    /// Parallelism configuration
    pub parallelism: usize,

    /// Batch size for query execution
    pub batch_size: usize,

    /// Checkpoint configuration
    pub checkpoint: CheckpointConfig,

    /// Memory limits
    pub memory_limit: MemoryLimit,

    /// Metrics configuration
    pub metrics: MetricsConfig,
}

impl Default for SqlEngineConfig {
    fn default() -> Self {
        Self {
            state_backend: StateBackendConfig::Memory,
            parallelism: num_cpus::get(),
            batch_size: 1000,
            checkpoint: CheckpointConfig::default(),
            memory_limit: MemoryLimit::default(),
            metrics: MetricsConfig::default(),
        }
    }
}

impl SqlEngineConfig {
    /// Create a configuration for development
    pub fn development() -> Self;

    /// Create a configuration for production
    pub fn production() -> Self;

    /// Set state backend
    pub fn with_state_backend(mut self, backend: StateBackendConfig) -> Self;

    /// Set parallelism
    pub fn with_parallelism(mut self, parallelism: usize) -> Self;

    /// Set batch size
    pub fn with_batch_size(mut self, batch_size: usize) -> Self;
}
```

### StateBackendConfig

State backend configuration options.

```rust
pub enum StateBackendConfig {
    /// In-memory state (for development)
    Memory,

    /// Sled embedded database
    Sled {
        path: PathBuf,
    },

    /// PostgreSQL state backend
    Postgres {
        url: String,
        pool_size: u32,
    },

    /// Redis state backend
    Redis {
        url: String,
        pool_size: u32,
        cluster: bool,
    },

    /// Custom state backend
    Custom(Box<dyn StateBackend>),
}
```

### CheckpointConfig

Checkpoint configuration for fault tolerance.

```rust
pub struct CheckpointConfig {
    /// Enable checkpointing
    pub enabled: bool,

    /// Checkpoint interval
    pub interval: Duration,

    /// Minimum pause between checkpoints
    pub min_pause: Duration,

    /// Checkpoint timeout
    pub timeout: Duration,

    /// Checkpoint storage path
    pub storage_path: PathBuf,
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval: Duration::from_secs(60),
            min_pause: Duration::from_secs(10),
            timeout: Duration::from_secs(600),
            storage_path: PathBuf::from("/var/lib/optimizer/checkpoints"),
        }
    }
}
```

---

## Query Execution

### Execute Simple Query

```rust
use llm_optimizer_sql::{SqlEngine, SqlEngineConfig};

async fn execute_simple_query() -> anyhow::Result<()> {
    let engine = SqlEngine::new(SqlEngineConfig::default()).await?;

    let query = r#"
        SELECT
            model_name,
            COUNT(*) as count,
            AVG(latency_ms) as avg_latency
        FROM feedback_events
        WHERE event_time >= NOW() - INTERVAL '1' HOUR
        GROUP BY model_name
    "#;

    let result = engine.execute(query).await?;

    for row in result.rows() {
        let model_name: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        let avg_latency: f64 = row.get(2)?;

        println!("{}: {} requests, avg latency: {}ms",
            model_name, count, avg_latency);
    }

    Ok(())
}
```

### Execute Parameterized Query

```rust
use llm_optimizer_sql::{SqlEngine, SqlValue};

async fn execute_parameterized_query(
    engine: &SqlEngine,
    model_name: &str,
    min_latency: i64,
) -> anyhow::Result<()> {
    let query = r#"
        SELECT
            event_id,
            user_id,
            latency_ms,
            cost_usd
        FROM feedback_events
        WHERE model_name = $1
          AND latency_ms > $2
    "#;

    let params = vec![
        SqlValue::String(model_name.to_string()),
        SqlValue::Int64(min_latency),
    ];

    let result = engine.execute_with_params(query, &params).await?;

    for row in result.rows() {
        let event_id: String = row.get(0)?;
        let user_id: String = row.get(1)?;
        let latency: i64 = row.get(2)?;
        let cost: f64 = row.get(3)?;

        println!("{}: user={}, latency={}ms, cost=${:.4}",
            event_id, user_id, latency, cost);
    }

    Ok(())
}
```

### Prepared Statements

```rust
use llm_optimizer_sql::{SqlEngine, PreparedStatement};

async fn use_prepared_statement(engine: &SqlEngine) -> anyhow::Result<()> {
    // Prepare statement once
    let stmt = engine.prepare(r#"
        SELECT
            COUNT(*) as count,
            AVG(latency_ms) as avg_latency
        FROM feedback_events
        WHERE model_name = $1
          AND event_time >= $2
    "#).await?;

    // Execute multiple times with different parameters
    let models = vec!["gpt-4", "claude-3", "gemini-pro"];

    for model_name in models {
        let result = stmt.execute(&[
            SqlValue::String(model_name.to_string()),
            SqlValue::Timestamp(Utc::now() - Duration::hours(1)),
        ]).await?;

        let row = result.rows().next().unwrap();
        let count: i64 = row.get(0)?;
        let avg_latency: f64 = row.get(1)?;

        println!("{}: {} requests, avg latency: {:.2}ms",
            model_name, count, avg_latency);
    }

    Ok(())
}
```

### Streaming Queries

```rust
use llm_optimizer_sql::{SqlEngine, StreamingResult};
use futures::StreamExt;

async fn streaming_query(engine: &SqlEngine) -> anyhow::Result<()> {
    let query = "SELECT * FROM feedback_events";

    // Get streaming result
    let mut stream = engine.execute_streaming(query).await?;

    // Process events as they arrive
    while let Some(result) = stream.next().await {
        let row = result?;
        let event_id: String = row.get(0)?;
        let model_name: String = row.get(2)?;
        let latency: i64 = row.get(6)?;

        println!("Event {}: model={}, latency={}ms",
            event_id, model_name, latency);

        // Can break early
        if latency > 10000 {
            break;
        }
    }

    Ok(())
}
```

---

## Result Handling

### QueryResult

Result of a SQL query execution.

```rust
pub struct QueryResult {
    schema: Arc<Schema>,
    rows: Vec<Row>,
    metadata: ResultMetadata,
}

impl QueryResult {
    /// Get the schema
    pub fn schema(&self) -> &Schema;

    /// Get all rows
    pub fn rows(&self) -> &[Row];

    /// Get number of rows
    pub fn row_count(&self) -> usize;

    /// Get result metadata
    pub fn metadata(&self) -> &ResultMetadata;

    /// Convert to vector of typed records
    pub fn into_records<T: FromRow>(self) -> Result<Vec<T>>;
}
```

### Row

A single row from query results.

```rust
pub struct Row {
    values: Vec<SqlValue>,
}

impl Row {
    /// Get column value by index
    pub fn get<T: FromSqlValue>(&self, index: usize) -> Result<T>;

    /// Get column value by name
    pub fn get_by_name<T: FromSqlValue>(&self, name: &str) -> Result<T>;

    /// Get column count
    pub fn column_count(&self) -> usize;

    /// Check if column is NULL
    pub fn is_null(&self, index: usize) -> bool;
}
```

### SqlValue

Enumeration of SQL value types.

```rust
pub enum SqlValue {
    Null,
    Boolean(bool),
    Int8(i8),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    UInt8(u8),
    UInt16(u16),
    UInt32(u32),
    UInt64(u64),
    Float32(f32),
    Float64(f64),
    String(String),
    Binary(Vec<u8>),
    Timestamp(DateTime<Utc>),
    Date(NaiveDate),
    Time(NaiveTime),
    Interval(Duration),
    Array(Vec<SqlValue>),
    Map(HashMap<String, SqlValue>),
    Struct(Vec<(String, SqlValue)>),
    Json(serde_json::Value),
}

impl SqlValue {
    /// Check if value is NULL
    pub fn is_null(&self) -> bool;

    /// Convert to specific type
    pub fn as_i64(&self) -> Option<i64>;
    pub fn as_f64(&self) -> Option<f64>;
    pub fn as_str(&self) -> Option<&str>;
    pub fn as_bool(&self) -> Option<bool>;
    pub fn as_timestamp(&self) -> Option<DateTime<Utc>>;
}
```

### Type Conversions

```rust
use llm_optimizer_sql::{FromSqlValue, Row};

// Built-in conversions
impl FromSqlValue for i64 { /* ... */ }
impl FromSqlValue for f64 { /* ... */ }
impl FromSqlValue for String { /* ... */ }
impl FromSqlValue for bool { /* ... */ }
impl FromSqlValue for DateTime<Utc> { /* ... */ }
impl FromSqlValue for Option<T> where T: FromSqlValue { /* ... */ }
impl FromSqlValue for Vec<T> where T: FromSqlValue { /* ... */ }

// Custom type conversion
#[derive(Debug)]
struct ModelMetrics {
    model_name: String,
    count: i64,
    avg_latency: f64,
    total_cost: f64,
}

impl FromRow for ModelMetrics {
    fn from_row(row: &Row) -> Result<Self> {
        Ok(Self {
            model_name: row.get(0)?,
            count: row.get(1)?,
            avg_latency: row.get(2)?,
            total_cost: row.get(3)?,
        })
    }
}

// Usage
async fn typed_results(engine: &SqlEngine) -> anyhow::Result<()> {
    let query = r#"
        SELECT
            model_name,
            COUNT(*) as count,
            AVG(latency_ms) as avg_latency,
            SUM(cost_usd) as total_cost
        FROM feedback_events
        GROUP BY model_name
    "#;

    let result = engine.execute(query).await?;
    let metrics: Vec<ModelMetrics> = result.into_records()?;

    for m in metrics {
        println!("{}: {} requests, avg latency: {:.2}ms, total cost: ${:.4}",
            m.model_name, m.count, m.avg_latency, m.total_cost);
    }

    Ok(())
}
```

---

## Error Handling

### SqlError

Comprehensive error type for SQL operations.

```rust
#[derive(Debug, thiserror::Error)]
pub enum SqlError {
    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Planning error: {0}")]
    PlanningError(String),

    #[error("Execution error: {0}")]
    ExecutionError(String),

    #[error("Type error: expected {expected}, got {actual}")]
    TypeError {
        expected: String,
        actual: String,
    },

    #[error("Column not found: {0}")]
    ColumnNotFound(String),

    #[error("State error: {0}")]
    StateError(#[from] StateError),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, SqlError>;
```

### Error Handling Patterns

```rust
use llm_optimizer_sql::{SqlEngine, SqlError};

async fn handle_errors(engine: &SqlEngine) -> anyhow::Result<()> {
    let query = "SELECT * FROM non_existent_table";

    match engine.execute(query).await {
        Ok(result) => {
            println!("Success: {} rows", result.row_count());
        }
        Err(SqlError::ParseError(msg)) => {
            eprintln!("SQL syntax error: {}", msg);
        }
        Err(SqlError::PlanningError(msg)) => {
            eprintln!("Query planning failed: {}", msg);
        }
        Err(SqlError::ExecutionError(msg)) => {
            eprintln!("Query execution failed: {}", msg);
        }
        Err(e) => {
            eprintln!("Unexpected error: {}", e);
        }
    }

    Ok(())
}

// Type-safe error handling
async fn type_safe_query(engine: &SqlEngine) -> anyhow::Result<()> {
    let result = engine.execute("SELECT latency_ms FROM feedback_events").await?;

    for row in result.rows() {
        match row.get::<i64>(0) {
            Ok(latency) => println!("Latency: {}ms", latency),
            Err(SqlError::TypeError { expected, actual }) => {
                eprintln!("Type mismatch: expected {}, got {}", expected, actual);
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    Ok(())
}
```

---

## Integration Examples

### Example 1: Embedded Stream Processing

```rust
use llm_optimizer_sql::{SqlEngine, SqlEngineConfig, StateBackendConfig};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Configure SQL engine
    let config = SqlEngineConfig::default()
        .with_state_backend(StateBackendConfig::Sled {
            path: PathBuf::from("/var/lib/optimizer/state"),
        })
        .with_parallelism(8)
        .with_batch_size(1000);

    let engine = SqlEngine::new(config).await?;

    // Create stream
    engine.execute(r#"
        CREATE STREAM feedback_events (
            event_id VARCHAR,
            user_id VARCHAR,
            model_name VARCHAR,
            latency_ms BIGINT,
            cost_usd DOUBLE,
            rating DOUBLE,
            event_time TIMESTAMP ROWTIME,
            WATERMARK FOR event_time AS event_time - INTERVAL '30' SECOND
        )
        WITH (
            'connector' = 'kafka',
            'topic' = 'llm-feedback',
            'properties.bootstrap.servers' = 'localhost:9092',
            'format' = 'json'
        )
    "#).await?;

    // Create continuous query
    let monitoring_query = engine.execute_streaming(r#"
        SELECT
            model_name,
            TUMBLE_START(event_time, INTERVAL '5' MINUTE) as window_start,
            COUNT(*) as requests,
            AVG(latency_ms) as avg_latency,
            APPROX_PERCENTILE(latency_ms, 0.95) as p95_latency,
            SUM(cost_usd) as total_cost,
            AVG(rating) as avg_rating
        FROM feedback_events
        GROUP BY
            model_name,
            TUMBLE(event_time, INTERVAL '5' MINUTE)
    "#).await?;

    // Process results
    use futures::StreamExt;
    tokio::pin!(monitoring_query);

    while let Some(result) = monitoring_query.next().await {
        let row = result?;
        let model_name: String = row.get(0)?;
        let window_start: DateTime<Utc> = row.get(1)?;
        let requests: i64 = row.get(2)?;
        let avg_latency: f64 = row.get(3)?;
        let p95_latency: f64 = row.get(4)?;
        let total_cost: f64 = row.get(5)?;
        let avg_rating: f64 = row.get(6)?;

        println!(
            "[{}] {}: {} requests, avg latency: {:.2}ms, p95: {:.2}ms, cost: ${:.4}, rating: {:.2}",
            window_start, model_name, requests, avg_latency, p95_latency, total_cost, avg_rating
        );
    }

    Ok(())
}
```

### Example 2: REST API Integration

```rust
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use llm_optimizer_sql::{SqlEngine, SqlEngineConfig};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    sql_engine: Arc<SqlEngine>,
}

#[derive(Deserialize)]
struct QueryRequest {
    sql: String,
}

#[derive(Serialize)]
struct QueryResponse {
    columns: Vec<String>,
    rows: Vec<Vec<serde_json::Value>>,
    row_count: usize,
}

async fn execute_query(
    State(state): State<AppState>,
    Json(request): Json<QueryRequest>,
) -> Result<Json<QueryResponse>, StatusCode> {
    let result = state.sql_engine
        .execute(&request.sql)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let columns = result.schema()
        .fields()
        .iter()
        .map(|f| f.name().to_string())
        .collect();

    let rows = result.rows()
        .iter()
        .map(|row| {
            (0..row.column_count())
                .map(|i| row.get::<serde_json::Value>(i).unwrap())
                .collect()
        })
        .collect();

    Ok(Json(QueryResponse {
        columns,
        rows,
        row_count: result.row_count(),
    }))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let sql_engine = SqlEngine::new(SqlEngineConfig::production()).await?;

    let app_state = AppState {
        sql_engine: Arc::new(sql_engine),
    };

    let app = Router::new()
        .route("/query", post(execute_query))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    axum::serve(listener, app).await?;

    Ok(())
}
```

### Example 3: Metrics Export

```rust
use llm_optimizer_sql::{SqlEngine, SqlEngineConfig};
use prometheus::{Encoder, GaugeVec, IntCounterVec, TextEncoder, Registry};
use std::sync::Arc;
use tokio::time::{interval, Duration};

async fn export_metrics(engine: Arc<SqlEngine>) -> anyhow::Result<()> {
    let registry = Registry::new();

    let request_count = IntCounterVec::new(
        prometheus::Opts::new("llm_requests_total", "Total LLM requests"),
        &["model"],
    )?;
    registry.register(Box::new(request_count.clone()))?;

    let avg_latency = GaugeVec::new(
        prometheus::Opts::new("llm_latency_avg_ms", "Average latency in ms"),
        &["model"],
    )?;
    registry.register(Box::new(avg_latency.clone()))?;

    let total_cost = GaugeVec::new(
        prometheus::Opts::new("llm_cost_total_usd", "Total cost in USD"),
        &["model"],
    )?;
    registry.register(Box::new(total_cost.clone()))?;

    let mut ticker = interval(Duration::from_secs(10));

    loop {
        ticker.tick().await;

        let query = r#"
            SELECT
                model_name,
                COUNT(*) as count,
                AVG(latency_ms) as avg_latency,
                SUM(cost_usd) as total_cost
            FROM feedback_events
            WHERE event_time >= NOW() - INTERVAL '1' MINUTE
            GROUP BY model_name
        "#;

        let result = engine.execute(query).await?;

        for row in result.rows() {
            let model: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            let latency: f64 = row.get(2)?;
            let cost: f64 = row.get(3)?;

            request_count.with_label_values(&[&model]).inc_by(count as u64);
            avg_latency.with_label_values(&[&model]).set(latency);
            total_cost.with_label_values(&[&model]).add(cost);
        }
    }
}
```

---

## Advanced Usage

### Custom State Backend

```rust
use llm_optimizer_sql::{StateBackend, StateBackendConfig};
use async_trait::async_trait;

struct CustomStateBackend {
    // Your state implementation
}

#[async_trait]
impl StateBackend for CustomStateBackend {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;
    async fn put(&self, key: &[u8], value: &[u8]) -> Result<()>;
    async fn delete(&self, key: &[u8]) -> Result<()>;
    async fn scan(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>>;
}

// Use custom backend
let config = SqlEngineConfig::default()
    .with_state_backend(StateBackendConfig::Custom(
        Box::new(CustomStateBackend::new())
    ));
```

### Query Planning and Optimization

```rust
use llm_optimizer_sql::{SqlEngine, LogicalPlan, PhysicalPlan};

async fn inspect_query_plan(engine: &SqlEngine) -> anyhow::Result<()> {
    let query = r#"
        SELECT
            model_name,
            AVG(latency_ms) as avg_latency
        FROM feedback_events
        WHERE latency_ms > 1000
        GROUP BY model_name
    "#;

    // Get logical plan
    let logical_plan = engine.create_logical_plan(query)?;
    println!("Logical Plan:\n{}", logical_plan.display_indent());

    // Get physical plan
    let physical_plan = engine.create_physical_plan(&logical_plan).await?;
    println!("Physical Plan:\n{}", physical_plan.display_indent());

    // Get estimated cost
    let cost = engine.estimate_cost(&logical_plan).await?;
    println!("Estimated cost: {:?}", cost);

    Ok(())
}
```

### Catalog Management

```rust
use llm_optimizer_sql::{SqlEngine, CatalogProvider};

async fn manage_catalog(engine: &SqlEngine) -> anyhow::Result<()> {
    let catalog = engine.catalog();

    // List all streams
    for stream in catalog.list_streams() {
        println!("Stream: {}", stream.name());
        println!("Schema: {:?}", stream.schema());
    }

    // List all tables
    for table in catalog.list_tables() {
        println!("Table: {}", table.name());
        println!("Schema: {:?}", table.schema());
    }

    // Get specific stream
    if let Some(stream) = catalog.get_stream("feedback_events") {
        println!("Found stream: {}", stream.name());
        for field in stream.schema().fields() {
            println!("  Column: {} ({})", field.name(), field.data_type());
        }
    }

    Ok(())
}
```

### Metrics and Monitoring

```rust
use llm_optimizer_sql::{SqlEngine, QueryMetrics};

async fn monitor_query_performance(engine: &SqlEngine) -> anyhow::Result<()> {
    let query = "SELECT COUNT(*) FROM feedback_events";

    let result = engine.execute(query).await?;
    let metrics = result.metadata().metrics();

    println!("Query Metrics:");
    println!("  Execution time: {:?}", metrics.execution_time);
    println!("  Records processed: {}", metrics.records_processed);
    println!("  Bytes processed: {}", metrics.bytes_processed);
    println!("  Memory used: {}", metrics.memory_used);
    println!("  State size: {}", metrics.state_size);

    Ok(())
}
```

---

## API Summary

### Core Types

| Type | Description |
|------|-------------|
| `SqlEngine` | Main SQL execution engine |
| `SqlEngineConfig` | Configuration for SQL engine |
| `QueryResult` | Result of query execution |
| `Row` | Single row from results |
| `SqlValue` | Enumeration of SQL values |
| `PreparedStatement` | Prepared statement for repeated execution |

### Configuration Types

| Type | Description |
|------|-------------|
| `StateBackendConfig` | State backend configuration |
| `CheckpointConfig` | Checkpoint configuration |
| `MemoryLimit` | Memory limit configuration |
| `MetricsConfig` | Metrics collection configuration |

### Error Types

| Type | Description |
|------|-------------|
| `SqlError` | SQL-specific errors |
| `StateError` | State management errors |
| `Result<T>` | Result type alias |

---

## Performance Considerations

### Connection Pooling

```rust
use llm_optimizer_sql::{SqlEngine, ConnectionPool};

let config = SqlEngineConfig::production()
    .with_connection_pool(ConnectionPool {
        min_size: 10,
        max_size: 100,
        connection_timeout: Duration::from_secs(30),
        idle_timeout: Duration::from_secs(300),
    });
```

### Batch Execution

```rust
async fn batch_execution(engine: &SqlEngine) -> anyhow::Result<()> {
    let queries = vec![
        "SELECT COUNT(*) FROM feedback_events WHERE model_name = 'gpt-4'",
        "SELECT AVG(latency_ms) FROM feedback_events WHERE model_name = 'gpt-4'",
        "SELECT SUM(cost_usd) FROM feedback_events WHERE model_name = 'gpt-4'",
    ];

    // Execute in parallel
    let results = futures::future::try_join_all(
        queries.iter().map(|q| engine.execute(q))
    ).await?;

    for (query, result) in queries.iter().zip(results.iter()) {
        println!("{}: {} rows", query, result.row_count());
    }

    Ok(())
}
```

---

## Next Steps

- [SQL Reference Guide](SQL_REFERENCE_GUIDE.md): Complete SQL syntax
- [SQL Examples](SQL_EXAMPLES.md): 56 practical examples
- [SQL Tutorial](SQL_TUTORIAL.md): Step-by-step learning
- [SQL Best Practices](SQL_BEST_PRACTICES.md): Optimization tips

---

## Support

For questions and support:

- GitHub Issues: https://github.com/globalbusinessadvisors/llm-auto-optimizer/issues
- Documentation: https://docs.llmdevops.dev
- API Docs: https://docs.rs/llm-optimizer-sql
