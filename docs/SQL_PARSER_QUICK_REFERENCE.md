# SQL Parser Quick Reference

**Version:** 1.0
**Date:** 2025-11-10
**Related:** [SQL_PARSER_ARCHITECTURE.md](SQL_PARSER_ARCHITECTURE.md)

## Quick Decision Summary

### Recommended Technology: sqlparser-rs (Apache DataFusion)

**Why?**
- Production-proven (used by DataFusion, Polars, GreptimeDB)
- 90%+ SQL coverage
- Extensible dialect system
- Active maintenance
- 2-3 weeks to extend vs 2-4 months to build from scratch

**Trade-off**: Accept lack of built-in streaming SQL support, extend via custom dialect

---

## Implementation Phases

| Phase | Duration | Deliverable |
|-------|----------|-------------|
| **Phase 1** | Week 1 | Setup crate, parse standard SQL |
| **Phase 2** | Week 2-3 | Add streaming extensions (WINDOW, WATERMARK, EMIT) |
| **Phase 3** | Week 4 | Semantic analysis & validation |
| **Phase 4** | Week 5 | Logical plan generation |
| **Phase 5** | Week 6 | Physical pipeline translation |

**Total**: 6 weeks to production-ready parser

---

## Core Streaming SQL Extensions

### 1. WINDOW Clause

```sql
-- Tumbling window
WINDOW TUMBLING(event_time, INTERVAL '5' MINUTE)

-- Sliding window
WINDOW SLIDING(event_time, INTERVAL '15' MINUTE, INTERVAL '1' MINUTE)

-- Session window
WINDOW SESSION(event_time, INTERVAL '30' MINUTE) MAX INTERVAL '4' HOUR

-- Hop window
WINDOW HOP(event_time, INTERVAL '10' MINUTE, INTERVAL '5' MINUTE)
```

### 2. WATERMARK Clause

```sql
WATERMARK event_time DELAY OF INTERVAL '2' MINUTE
```

### 3. EMIT Clause

```sql
EMIT ON WATERMARK           -- Default, emit when window completes
EMIT ON WINDOW CLOSE        -- Same as ON WATERMARK
EMIT EVERY INTERVAL '1' MINUTE   -- Emit partial results every minute
EMIT AFTER INTERVAL '30' SECOND  -- Emit once after 30 seconds
EMIT ON CHANGE              -- Emit when state changes
```

### 4. CREATE STREAM

```sql
CREATE STREAM performance_metrics (
    request_id UUID,
    model VARCHAR NOT NULL,
    latency_ms DOUBLE NOT NULL,
    event_time TIMESTAMP NOT NULL
)
WITH (
    KAFKA_TOPIC = 'llm.metrics.performance',
    KAFKA_BROKERS = 'localhost:9092',
    FORMAT = 'JSON',
    WATERMARK_COLUMN = 'event_time',
    WATERMARK_DELAY = INTERVAL '2' MINUTE
);
```

### 5. CREATE MATERIALIZED VIEW

```sql
CREATE MATERIALIZED VIEW model_performance_5min AS
SELECT
    model,
    COUNT(*) as request_count,
    AVG(latency_ms) as avg_latency,
    window_start,
    window_end
FROM performance_metrics
WINDOW TUMBLING(event_time, INTERVAL '5' MINUTE)
GROUP BY model, window_start, window_end
WATERMARK event_time DELAY OF INTERVAL '2' MINUTE
EMIT ON WATERMARK
WITH (
    SINK_TYPE = 'POSTGRES',
    SINK_TABLE = 'model_performance_metrics'
);
```

---

## Example Queries

### Basic Windowed Aggregation

```sql
SELECT
    model,
    COUNT(*) as request_count,
    AVG(latency_ms) as avg_latency,
    PERCENTILE(latency_ms, 0.95) as p95_latency,
    window_start,
    window_end
FROM performance_metrics
WINDOW TUMBLING(event_time, INTERVAL '5' MINUTE)
GROUP BY model, window_start, window_end
WATERMARK event_time DELAY OF INTERVAL '2' MINUTE
EMIT ON WATERMARK;
```

### Sliding Window for Anomaly Detection

```sql
SELECT
    model,
    AVG(error_rate) as avg_error_rate,
    STDDEV(latency_ms) as latency_stddev,
    window_start,
    window_end
FROM performance_metrics
WINDOW SLIDING(event_time, INTERVAL '15' MINUTE, INTERVAL '1' MINUTE)
GROUP BY model, window_start, window_end
WATERMARK event_time DELAY OF INTERVAL '1' MINUTE
EMIT EVERY INTERVAL '1' MINUTE;
```

### Session-Based User Feedback

```sql
SELECT
    session_id,
    COUNT(*) as interaction_count,
    AVG(rating) as avg_rating,
    window_start,
    window_end
FROM user_feedback
WINDOW SESSION(event_time, INTERVAL '30' MINUTE)
    MAX INTERVAL '4' HOUR
GROUP BY session_id, window_start, window_end
WATERMARK event_time DELAY OF INTERVAL '10' MINUTE
EMIT ON WINDOW CLOSE;
```

### Time-Bounded JOIN

```sql
SELECT
    pm.model,
    pm.request_id,
    pm.latency_ms,
    uf.rating
FROM performance_metrics pm
JOIN user_feedback uf
    ON pm.request_id = uf.request_id
    WITHIN INTERVAL '5' MINUTE
WHERE pm.error IS NULL
WATERMARK pm.event_time DELAY OF INTERVAL '2' MINUTE
WATERMARK uf.event_time DELAY OF INTERVAL '10' MINUTE;
```

---

## Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Parse Time | <10ms (p99) | For queries <1KB |
| Memory per AST | <10KB | Support 10,000s in memory |
| Throughput | >10,000 queries/sec | Batch compilation |
| Startup Time | <100ms | Include dialect init |

---

## Integration Example

```rust
use sql_parser::SqlStreamProcessor;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut processor = SqlStreamProcessor::new();

    let sql = r#"
        SELECT model, COUNT(*) as count
        FROM metrics
        WINDOW TUMBLING(event_time, INTERVAL '1' MINUTE)
        GROUP BY model
        WATERMARK event_time DELAY OF INTERVAL '30' SECOND
    "#;

    processor.execute(sql).await?;

    Ok(())
}
```

---

## Crate Structure

```
crates/sql-parser/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public API
│   ├── dialect.rs          # Streaming SQL dialect
│   ├── ast_ext.rs          # Streaming AST extensions
│   ├── parser.rs           # Parser wrapper
│   ├── semantic.rs         # Semantic analysis
│   ├── planner.rs          # Logical plan generation
│   ├── translator.rs       # AST → Pipeline translation
│   ├── visitor.rs          # AST visitor pattern
│   └── error.rs            # Error types
├── tests/
│   ├── parser_tests.rs
│   ├── window_tests.rs
│   ├── watermark_tests.rs
│   └── integration_tests.rs
├── benches/
│   └── parser_bench.rs
└── examples/
    ├── simple_query.rs
    └── window_query.rs
```

---

## Dependencies

```toml
[dependencies]
sqlparser = "0.54"          # Apache DataFusion SQL parser
serde = { version = "1.0", features = ["derive"] }
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1.0"
uuid = { version = "1.0", features = ["v4", "serde"] }
```

---

## Key AST Types

```rust
// Streaming statement types
pub enum StreamingStatement {
    CreateStream(CreateStream),
    CreateMaterializedView(CreateMaterializedView),
    Query(StreamingQuery),
}

// Window definitions
pub enum WindowDefinition {
    Tumbling(TumblingWindow),
    Sliding(SlidingWindow),
    Session(SessionWindow),
    Hop(HopWindow),
}

// Emit strategies
pub enum EmitClause {
    OnWatermark,
    OnWindowClose,
    After(IntervalValue),
    Every(IntervalValue),
    OnChange,
}
```

---

## Error Handling

```rust
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Syntax error: {0}")]
    SyntaxError(#[from] sqlparser::parser::ParserError),

    #[error("Unknown stream: {0}")]
    UnknownStream(String),

    #[error("Invalid window size: {0}")]
    InvalidWindowSize(i64),

    #[error("WATERMARK clause required for windowed queries")]
    MissingWatermark,

    #[error("Type mismatch: expected {expected}, found {found}")]
    TypeMismatch { expected: String, found: String },
}
```

---

## Testing Strategy

| Test Type | Tool | Purpose |
|-----------|------|---------|
| Unit Tests | `cargo test` | Component testing |
| Integration | `cargo test --test '*'` | End-to-end parsing |
| Property Tests | `proptest` | Grammar invariants |
| Fuzzing | `cargo-fuzz` | Robustness |
| Benchmarks | `criterion` | Performance regression |

---

## Comparison with Alternatives

| Feature | sqlparser-rs | nom | pest | lalrpop |
|---------|--------------|-----|------|---------|
| **SQL Coverage** | 90%+ | 0% | 0% | ~20% (SQLite via lemon-rs) |
| **Streaming Support** | Extension needed | Native | Extension needed | Limited |
| **Time to Production** | 2-3 weeks | 3-6 months | 2-4 months | 2-4 months |
| **Maintenance** | Apache project | Stable | Active | Moderate |
| **Performance** | Excellent | Excellent | Good | Excellent |
| **Extensibility** | Dialect system | Maximum | PEG flexibility | LR(1) constraints |

**Verdict**: sqlparser-rs offers the best ROI for production deployment

---

## Migration from Rust API

### Before (Rust API)

```rust
StreamPipelineBuilder::new()
    .window(WindowConfig::tumbling(Duration::minutes(5)))
    .watermark(WatermarkConfig {
        column: "event_time".into(),
        delay: Duration::minutes(2),
        strategy: WatermarkStrategy::BoundedOutOfOrder,
    })
    .aggregate(AggregationConfig::avg("latency_ms"))
    .build()
```

### After (SQL)

```sql
SELECT AVG(latency_ms)
FROM metrics
WINDOW TUMBLING(event_time, INTERVAL '5' MINUTE)
WATERMARK event_time DELAY OF INTERVAL '2' MINUTE
```

Both produce identical execution plans.

---

## Future Extensions

### Near-Term (3-6 months)
- User-Defined Functions (UDFs)
- User-Defined Aggregates (UDAFs)
- Pattern matching (CEP)
- Versioned streams

### Long-Term (6-12 months)
- ML model integration
- Distributed query execution
- Cost-based optimization
- Materialized view maintenance

---

## Resources

- **Full Architecture**: [SQL_PARSER_ARCHITECTURE.md](SQL_PARSER_ARCHITECTURE.md)
- **sqlparser-rs**: https://github.com/apache/datafusion-sqlparser-rs
- **Apache DataFusion**: https://datafusion.apache.org/
- **Apache Flink SQL**: https://nightlies.apache.org/flink/flink-docs-master/docs/dev/table/sql/
- **ksqlDB**: https://docs.ksqldb.io/

---

**Document Version**: 1.0
**Last Updated**: 2025-11-10
