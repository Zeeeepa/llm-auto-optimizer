# SQL Parser Architecture for LLM Auto-Optimizer

**Version:** 1.0
**Date:** 2025-11-10
**Status:** Technical Architecture
**Owner:** LLM DevOps Team

## Executive Summary

This document provides a comprehensive architecture for implementing a streaming SQL parser in the LLM Auto-Optimizer system. After extensive research of Rust SQL parsing technologies, we recommend **sqlparser-rs** (Apache DataFusion) as the foundation, extended with streaming-specific constructs. This approach balances production-readiness, extensibility, and performance while avoiding the complexity of building a parser from scratch.

### Key Recommendations

1. **Parser Technology**: Use `sqlparser-rs` (Apache DataFusion SQL parser) as the base
2. **Extension Strategy**: Add streaming SQL extensions via custom dialect
3. **AST Design**: Leverage sqlparser-rs AST with streaming-specific nodes
4. **Integration**: Translate SQL AST to processor pipeline operations
5. **Performance Target**: <10ms parse time for typical queries (<1KB)

---

## Table of Contents

1. [Parser Technology Evaluation](#1-parser-technology-evaluation)
2. [Recommended Architecture](#2-recommended-architecture)
3. [Streaming SQL Grammar](#3-streaming-sql-grammar)
4. [AST Structure](#4-ast-structure)
5. [Parser Implementation Plan](#5-parser-implementation-plan)
6. [Error Handling Strategy](#6-error-handling-strategy)
7. [Performance Characteristics](#7-performance-characteristics)
8. [Testing Approach](#8-testing-approach)
9. [Integration with Stream Processor](#9-integration-with-stream-processor)
10. [Future Extensions](#10-future-extensions)

---

## 1. Parser Technology Evaluation

### 1.1 Evaluation Criteria

| Criterion | Weight | Description |
|-----------|--------|-------------|
| **Maturity** | 30% | Production use, active maintenance, community size |
| **Performance** | 25% | Parse speed, memory efficiency, streaming support |
| **Extensibility** | 20% | Ability to add custom syntax, dialect support |
| **SQL Coverage** | 15% | Standard SQL support, vendor dialect coverage |
| **Rust Idioms** | 10% | API design, error handling, async support |

### 1.2 Technology Comparison

#### Option 1: sqlparser-rs (Apache DataFusion)

**Official Repository**: https://github.com/apache/datafusion-sqlparser-rs

**Scores**:
- Maturity: 9/10 (Used by DataFusion, Polars, GreptimeDB, ParadeDB)
- Performance: 8/10 (Pratt parser for expressions, hand-written recursive descent)
- Extensibility: 9/10 (Dialect system, visitor pattern, custom statements)
- SQL Coverage: 9/10 (ANSI SQL, PostgreSQL, MySQL, BigQuery, Snowflake dialects)
- Rust Idioms: 8/10 (Good error types, no async needed for parsing)

**Pros**:
- ✅ Production-proven in Apache Arrow ecosystem
- ✅ Excellent SQL coverage (90%+ of standard SQL)
- ✅ Clean AST design with comprehensive node types
- ✅ Extensible dialect system for custom syntax
- ✅ Active development (v0.54.0 released Jan 2025)
- ✅ MIT/Apache 2.0 license
- ✅ Comprehensive documentation and examples

**Cons**:
- ❌ No built-in streaming SQL support (needs extension)
- ❌ No watermark/window function syntax (needs custom dialect)
- ⚠️ Large dependency (but well-maintained)

**Verdict**: **Recommended** - Best balance of maturity and extensibility

---

#### Option 2: nom (Parser Combinators)

**Official Repository**: https://github.com/rust-bakery/nom

**Scores**:
- Maturity: 9/10 (Widely used, stable API)
- Performance: 9/10 (Zero-copy, streaming, excellent for binary formats)
- Extensibility: 10/10 (Build any grammar)
- SQL Coverage: 0/10 (No SQL grammar provided)
- Rust Idioms: 9/10 (Excellent error handling, composable)

**Pros**:
- ✅ Maximum flexibility - build exactly what you need
- ✅ Excellent streaming support
- ✅ Zero-copy parsing
- ✅ Composable parser functions

**Cons**:
- ❌ Build SQL grammar from scratch (3-6 months effort)
- ❌ Maintain grammar as SQL evolves
- ❌ Complex error messages without significant work
- ❌ No existing SQL AST design

**Verdict**: **Not Recommended** - Too much implementation overhead

---

#### Option 3: pest (PEG Parser Generator)

**Official Repository**: https://github.com/pest-parser/pest

**Scores**:
- Maturity: 8/10 (Stable, active community)
- Performance: 7/10 (Good but not as fast as hand-written)
- Extensibility: 9/10 (PEG grammars are flexible)
- SQL Coverage: 0/10 (No SQL grammar in ecosystem)
- Rust Idioms: 8/10 (Good integration, proc macros)

**Pros**:
- ✅ Clean grammar files (.pest format)
- ✅ Good error reporting with spans
- ✅ Flexible PEG semantics

**Cons**:
- ❌ Build SQL grammar from scratch (2-4 months effort)
- ❌ PEG can have subtle ambiguities with SQL
- ❌ Performance overhead vs hand-written parsers
- ❌ Need to design AST separately

**Verdict**: **Not Recommended** - High implementation cost

---

#### Option 4: lalrpop (LR Parser Generator)

**Official Repository**: https://github.com/lalrpop/lalrpop

**Scores**:
- Maturity: 7/10 (Stable but smaller community)
- Performance: 8/10 (Efficient LR(1) parsing)
- Extensibility: 7/10 (LR grammars can be tricky)
- SQL Coverage: 2/10 (lemon-rs has SQLite grammar but different approach)
- Rust Idioms: 8/10 (Good integration)

**Pros**:
- ✅ Efficient LR(1) parsing
- ✅ Good error recovery potential
- ⚠️ lemon-rs has SQLite grammar (different tool)

**Cons**:
- ❌ Build grammar from scratch or port from lemon-rs
- ❌ LR grammars harder to write than recursive descent
- ❌ Less flexible than PEG for experimental syntax
- ⚠️ Author notes: "does not support fallback/streaming"

**Verdict**: **Not Recommended** - Streaming limitations

---

### 1.3 Final Recommendation

**Winner: sqlparser-rs (Apache DataFusion SQL Parser)**

**Rationale**:
1. **Production Ready**: Used in production by Apache Arrow DataFusion, Polars, GreptimeDB
2. **Comprehensive SQL**: Covers 90%+ of standard SQL, multiple dialects
3. **Extensibility**: Dialect system allows adding streaming extensions
4. **Maintenance**: Active Apache project with monthly releases
5. **Time to Market**: 2-3 weeks to add streaming extensions vs 2-4 months to build from scratch
6. **AST Design**: Well-designed AST with visitor pattern for transformations

**Trade-off**: We accept lack of built-in streaming SQL support in exchange for production readiness and comprehensive SQL coverage. We'll extend the parser with a custom dialect for streaming constructs.

---

## 2. Recommended Architecture

### 2.1 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    SQL Parser Layer                          │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐  │
│  │   Lexer      │───▶│   Parser     │───▶│  AST Builder │  │
│  │  (Tokenize)  │    │  (Syntax)    │    │              │  │
│  └──────────────┘    └──────────────┘    └──────────────┘  │
│         │                     │                     │         │
│         │                     │                     ▼         │
│         │                     │            ┌──────────────┐  │
│         │                     │            │ Streaming    │  │
│         │                     │            │ SQL AST      │  │
│         │                     │            └──────────────┘  │
│         │                     │                     │         │
│         ▼                     ▼                     ▼         │
│  ┌──────────────────────────────────────────────────────┐   │
│  │         Error Handler & Position Tracking            │   │
│  └──────────────────────────────────────────────────────┘   │
│                               │                               │
└───────────────────────────────┼───────────────────────────────┘
                                ▼
                       ┌──────────────┐
                       │  Semantic    │
                       │  Analyzer    │
                       └──────────────┘
                                │
                                ▼
                       ┌──────────────┐
                       │  Logical     │
                       │  Plan        │
                       └──────────────┘
                                │
                                ▼
                       ┌──────────────┐
                       │  Physical    │
                       │  Pipeline    │
                       └──────────────┘
```

### 2.2 Component Overview

| Component | Technology | Responsibility |
|-----------|-----------|----------------|
| **Lexer** | sqlparser-rs Tokenizer | Convert SQL text to tokens |
| **Parser** | sqlparser-rs Parser + Custom Dialect | Build AST from tokens |
| **AST** | sqlparser-rs AST + Extensions | Represent parsed SQL structure |
| **Semantic Analyzer** | Custom | Type checking, validation, optimization |
| **Logical Plan** | Custom | Platform-independent query plan |
| **Physical Pipeline** | Processor Crate | Executable stream processing pipeline |

### 2.3 Crate Structure

```
llm-auto-optimizer/
└── crates/
    └── sql-parser/              # NEW CRATE
        ├── Cargo.toml
        ├── src/
        │   ├── lib.rs
        │   ├── dialect.rs       # Streaming SQL dialect
        │   ├── ast_ext.rs       # Streaming AST extensions
        │   ├── parser.rs        # Parser wrapper
        │   ├── semantic.rs      # Semantic analysis
        │   ├── planner.rs       # Logical plan generation
        │   ├── translator.rs    # AST → Pipeline translation
        │   └── error.rs         # Error types
        ├── tests/
        │   ├── parser_tests.rs
        │   ├── window_tests.rs
        │   ├── watermark_tests.rs
        │   └── integration_tests.rs
        └── examples/
            ├── simple_query.rs
            ├── window_query.rs
            └── complex_query.rs
```

---

## 3. Streaming SQL Grammar

### 3.1 Grammar Extensions (EBNF)

This section defines the streaming SQL grammar extensions beyond standard SQL.

```ebnf
(* Top-level statements *)
streaming_statement ::=
    | create_stream_statement
    | create_materialized_view_statement
    | select_statement
    | insert_statement
    ;

(* CREATE STREAM *)
create_stream_statement ::=
    "CREATE" "STREAM" [IF NOT EXISTS] stream_name
    "(" column_definition_list ")"
    [WITH "(" stream_properties ")"]
    ;

stream_properties ::= property ("," property)*
    ;

property ::= property_name "=" property_value
    ;

(* Examples:
    WATERMARK_COLUMN = 'event_time'
    WATERMARK_DELAY = INTERVAL '5' SECOND
    FORMAT = 'JSON'
    KAFKA_TOPIC = 'events'
*)

(* CREATE MATERIALIZED VIEW *)
create_materialized_view_statement ::=
    "CREATE" "MATERIALIZED" "VIEW" [IF NOT EXISTS] view_name
    "AS" select_statement
    [WITH "(" view_properties ")"]
    ;

(* SELECT with streaming extensions *)
select_statement ::=
    "SELECT" select_list
    "FROM" from_clause
    [WHERE where_clause]
    [GROUP BY group_by_clause]
    [HAVING having_clause]
    [WINDOW window_clause]
    [WATERMARK watermark_clause]
    [EMIT emit_clause]
    [ORDER BY order_by_clause]
    [LIMIT limit_clause]
    ;

(* WINDOW clause *)
window_clause ::= window_definition ("," window_definition)*
    ;

window_definition ::=
    | tumbling_window
    | sliding_window
    | session_window
    | hop_window
    ;

tumbling_window ::=
    "TUMBLING" "(" column_name "," interval ")"
    [ALIGNED aligned_to]
    ;

sliding_window ::=
    "SLIDING" "(" column_name "," size: interval "," slide: interval ")"
    [ALIGNED aligned_to]
    ;

session_window ::=
    "SESSION" "(" column_name "," gap: interval ")"
    [MAX max_duration: interval]
    ;

hop_window ::=
    "HOP" "(" column_name "," size: interval "," hop: interval ")"
    ;

aligned_to ::=
    | "TO" "EPOCH"
    | "TO" timestamp_literal
    ;

(* WATERMARK clause *)
watermark_clause ::=
    "WATERMARK" watermark_expression "DELAY" "OF" interval
    ;

watermark_expression ::=
    | column_name
    | function_call  (* e.g., WATERMARK CAST(ts AS TIMESTAMP) DELAY OF INTERVAL '5' SECOND *)
    ;

(* EMIT clause *)
emit_clause ::=
    "EMIT" emit_strategy
    ;

emit_strategy ::=
    | "ON" "WATERMARK"
    | "ON" "WINDOW" "CLOSE"
    | "AFTER" interval
    | "EVERY" interval
    | "ON" "CHANGE"
    ;

(* Window functions *)
window_function ::=
    aggregate_function "OVER" window_spec
    ;

window_spec ::=
    "("
    ["PARTITION" "BY" partition_by_list]
    ["ORDER" "BY" order_by_list]
    [window_frame]
    ")"
    ;

window_frame ::=
    frame_units frame_extent
    ;

frame_units ::= "ROWS" | "RANGE"
    ;

frame_extent ::=
    | "BETWEEN" frame_bound "AND" frame_bound
    | frame_bound
    ;

frame_bound ::=
    | "UNBOUNDED" "PRECEDING"
    | "UNBOUNDED" "FOLLOWING"
    | "CURRENT" "ROW"
    | interval "PRECEDING"
    | interval "FOLLOWING"
    ;

(* Interval literals *)
interval ::=
    "INTERVAL" string_literal time_unit
    ;

time_unit ::=
    | "MICROSECOND" | "MICROSECONDS"
    | "MILLISECOND" | "MILLISECONDS"
    | "SECOND" | "SECONDS"
    | "MINUTE" | "MINUTES"
    | "HOUR" | "HOURS"
    | "DAY" | "DAYS"
    | "WEEK" | "WEEKS"
    | "MONTH" | "MONTHS"
    | "YEAR" | "YEARS"
    ;

(* Time functions *)
time_function ::=
    | "NOW" "(" ")"
    | "CURRENT_TIMESTAMP" "(" ")"
    | "CURRENT_TIME" "(" ")"
    | "CURRENT_DATE" "(" ")"
    | "UNIX_TIMESTAMP" "(" [expression] ")"
    | "TO_TIMESTAMP" "(" expression ")"
    | "DATE_TRUNC" "(" string_literal "," expression ")"
    ;

(* Aggregation functions *)
aggregate_function ::=
    | "COUNT" "(" ("*" | expression) ")"
    | "SUM" "(" expression ")"
    | "AVG" "(" expression ")"
    | "MIN" "(" expression ")"
    | "MAX" "(" expression ")"
    | "STDDEV" "(" expression ")"
    | "VARIANCE" "(" expression ")"
    | "PERCENTILE" "(" expression "," numeric_literal ")"
    | "FIRST_VALUE" "(" expression ")"
    | "LAST_VALUE" "(" expression ")"
    | "LAG" "(" expression ["," offset ["," default_value]] ")"
    | "LEAD" "(" expression ["," offset ["," default_value]] ")"
    ;

(* JOIN with time bounds *)
join_clause ::=
    join_type "JOIN" table_reference
    "ON" join_condition
    [WITHIN within_clause]
    ;

within_clause ::=
    "WITHIN" interval
    ;

join_type ::=
    | [INNER] JOIN
    | LEFT [OUTER] JOIN
    | RIGHT [OUTER] JOIN
    | FULL [OUTER] JOIN
    ;
```

### 3.2 Example Queries

#### Example 1: Basic Tumbling Window

```sql
-- Calculate average latency per model every 5 minutes
SELECT
    model,
    COUNT(*) as request_count,
    AVG(latency_ms) as avg_latency,
    PERCENTILE(latency_ms, 0.95) as p95_latency,
    SUM(cost) as total_cost,
    window_start,
    window_end
FROM
    performance_metrics
WINDOW TUMBLING(event_time, INTERVAL '5' MINUTE)
GROUP BY
    model,
    window_start,
    window_end
WATERMARK event_time DELAY OF INTERVAL '2' MINUTE
EMIT ON WATERMARK;
```

#### Example 2: Sliding Window for Anomaly Detection

```sql
-- Detect anomalies with 15-minute window, sliding every 1 minute
SELECT
    model,
    AVG(error_rate) as avg_error_rate,
    STDDEV(latency_ms) as latency_stddev,
    COUNT(*) as sample_count,
    window_start,
    window_end
FROM
    performance_metrics
WINDOW SLIDING(event_time, INTERVAL '15' MINUTE, INTERVAL '1' MINUTE)
GROUP BY
    model,
    window_start,
    window_end
WATERMARK event_time DELAY OF INTERVAL '1' MINUTE
EMIT EVERY INTERVAL '1' MINUTE;
```

#### Example 3: Session Window for User Feedback

```sql
-- Aggregate user feedback by session (30-minute inactivity gap)
SELECT
    session_id,
    COUNT(*) as interaction_count,
    AVG(rating) as avg_rating,
    SUM(CASE WHEN regenerated THEN 1 ELSE 0 END) as regeneration_count,
    MIN(event_time) as session_start,
    MAX(event_time) as session_end,
    window_start,
    window_end
FROM
    user_feedback
WINDOW SESSION(event_time, INTERVAL '30' MINUTE)
    MAX INTERVAL '4' HOUR
GROUP BY
    session_id,
    window_start,
    window_end
WATERMARK event_time DELAY OF INTERVAL '10' MINUTE
EMIT ON WINDOW CLOSE;
```

#### Example 4: Complex Query with JOIN

```sql
-- Join performance metrics with user feedback within 5-minute window
SELECT
    pm.model,
    pm.request_id,
    pm.latency_ms,
    uf.rating,
    uf.feedback_text,
    pm.event_time as metric_time,
    uf.event_time as feedback_time
FROM
    performance_metrics pm
JOIN
    user_feedback uf
    ON pm.request_id = uf.request_id
    WITHIN INTERVAL '5' MINUTE
WHERE
    pm.error IS NULL
    AND uf.rating IS NOT NULL
WATERMARK pm.event_time DELAY OF INTERVAL '2' MINUTE
WATERMARK uf.event_time DELAY OF INTERVAL '10' MINUTE;
```

#### Example 5: CREATE STREAM

```sql
-- Define a new stream from Kafka topic
CREATE STREAM performance_metrics (
    request_id UUID,
    model VARCHAR NOT NULL,
    latency_ms DOUBLE NOT NULL,
    cost DOUBLE,
    input_tokens INT,
    output_tokens INT,
    error VARCHAR,
    event_time TIMESTAMP NOT NULL,
    ingestion_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP
)
WITH (
    KAFKA_TOPIC = 'llm.metrics.performance',
    KAFKA_BROKERS = 'localhost:9092',
    FORMAT = 'JSON',
    WATERMARK_COLUMN = 'event_time',
    WATERMARK_DELAY = INTERVAL '2' MINUTE,
    TIMESTAMP_FORMAT = 'RFC3339'
);
```

#### Example 6: CREATE MATERIALIZED VIEW

```sql
-- Create materialized view with continuous query
CREATE MATERIALIZED VIEW model_performance_5min AS
SELECT
    model,
    COUNT(*) as request_count,
    AVG(latency_ms) as avg_latency,
    PERCENTILE(latency_ms, 0.50) as p50_latency,
    PERCENTILE(latency_ms, 0.95) as p95_latency,
    PERCENTILE(latency_ms, 0.99) as p99_latency,
    SUM(cost) as total_cost,
    SUM(CASE WHEN error IS NOT NULL THEN 1 ELSE 0 END) as error_count,
    window_start,
    window_end
FROM
    performance_metrics
WINDOW TUMBLING(event_time, INTERVAL '5' MINUTE)
GROUP BY
    model,
    window_start,
    window_end
WATERMARK event_time DELAY OF INTERVAL '2' MINUTE
EMIT ON WATERMARK
WITH (
    SINK_TYPE = 'POSTGRES',
    SINK_TABLE = 'model_performance_metrics',
    CHECKPOINT_INTERVAL = INTERVAL '1' MINUTE
);
```

---

## 4. AST Structure

### 4.1 Core AST Types

We extend sqlparser-rs AST with streaming-specific nodes:

```rust
// File: crates/sql-parser/src/ast_ext.rs

use sqlparser::ast::{
    Expr, Ident, ObjectName, Query, Select, SelectItem,
    SetExpr, Statement, TableFactor, TableWithJoins,
};
use chrono::Duration;

/// Streaming SQL statement extensions
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StreamingStatement {
    /// CREATE STREAM statement
    CreateStream(CreateStream),

    /// CREATE MATERIALIZED VIEW statement
    CreateMaterializedView(CreateMaterializedView),

    /// Standard SQL statement with streaming extensions
    Query(StreamingQuery),
}

/// CREATE STREAM statement
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CreateStream {
    /// Stream name
    pub name: ObjectName,

    /// Column definitions
    pub columns: Vec<ColumnDef>,

    /// IF NOT EXISTS
    pub if_not_exists: bool,

    /// Stream properties (WITH clause)
    pub properties: Vec<StreamProperty>,
}

/// Stream property (key-value pair)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StreamProperty {
    pub key: String,
    pub value: PropertyValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PropertyValue {
    String(String),
    Number(i64),
    Interval(IntervalValue),
    Boolean(bool),
}

/// Column definition for CREATE STREAM
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColumnDef {
    pub name: Ident,
    pub data_type: DataType,
    pub nullable: bool,
    pub default: Option<Expr>,
}

/// CREATE MATERIALIZED VIEW statement
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CreateMaterializedView {
    pub name: ObjectName,
    pub query: Box<StreamingQuery>,
    pub if_not_exists: bool,
    pub properties: Vec<StreamProperty>,
}

/// Query with streaming extensions
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StreamingQuery {
    /// Standard SQL query
    pub query: Query,

    /// WINDOW clause
    pub window: Option<WindowClause>,

    /// WATERMARK clause
    pub watermark: Option<WatermarkClause>,

    /// EMIT clause
    pub emit: Option<EmitClause>,
}

/// WINDOW clause
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WindowClause {
    pub windows: Vec<WindowDefinition>,
}

/// Window definition
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WindowDefinition {
    Tumbling(TumblingWindow),
    Sliding(SlidingWindow),
    Session(SessionWindow),
    Hop(HopWindow),
}

/// Tumbling window
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TumblingWindow {
    /// Time column
    pub time_column: Ident,

    /// Window size
    pub size: IntervalValue,

    /// Alignment
    pub aligned_to: Option<WindowAlignment>,
}

/// Sliding window
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SlidingWindow {
    pub time_column: Ident,
    pub size: IntervalValue,
    pub slide: IntervalValue,
    pub aligned_to: Option<WindowAlignment>,
}

/// Session window
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionWindow {
    pub time_column: Ident,
    pub gap: IntervalValue,
    pub max_duration: Option<IntervalValue>,
}

/// Hop window
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HopWindow {
    pub time_column: Ident,
    pub size: IntervalValue,
    pub hop: IntervalValue,
}

/// Window alignment
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WindowAlignment {
    Epoch,
    Timestamp(i64),  // Unix timestamp
}

/// WATERMARK clause
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WatermarkClause {
    /// Watermark expression (column or function)
    pub expression: Expr,

    /// Maximum allowed delay
    pub delay: IntervalValue,
}

/// EMIT clause
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EmitClause {
    /// EMIT ON WATERMARK
    OnWatermark,

    /// EMIT ON WINDOW CLOSE
    OnWindowClose,

    /// EMIT AFTER INTERVAL
    After(IntervalValue),

    /// EMIT EVERY INTERVAL
    Every(IntervalValue),

    /// EMIT ON CHANGE
    OnChange,
}

/// Interval value
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IntervalValue {
    pub value: i64,
    pub unit: TimeUnit,
}

impl IntervalValue {
    pub fn to_duration(&self) -> Duration {
        match self.unit {
            TimeUnit::Microsecond => Duration::microseconds(self.value),
            TimeUnit::Millisecond => Duration::milliseconds(self.value),
            TimeUnit::Second => Duration::seconds(self.value),
            TimeUnit::Minute => Duration::minutes(self.value),
            TimeUnit::Hour => Duration::hours(self.value),
            TimeUnit::Day => Duration::days(self.value),
            TimeUnit::Week => Duration::weeks(self.value),
            TimeUnit::Month => Duration::days(self.value * 30), // Approximate
            TimeUnit::Year => Duration::days(self.value * 365), // Approximate
        }
    }
}

/// Time unit for intervals
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeUnit {
    Microsecond,
    Millisecond,
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
}

/// Data type for column definitions
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DataType {
    // Numeric types
    TinyInt,
    SmallInt,
    Int,
    BigInt,
    Float,
    Double,
    Decimal(Option<u8>, Option<u8>),

    // String types
    Char(Option<u64>),
    Varchar(Option<u64>),
    Text,

    // Date/Time types
    Date,
    Time,
    Timestamp,
    TimestampTz,
    Interval,

    // Other types
    Boolean,
    Uuid,
    Json,
    Jsonb,
    Binary,

    // Custom/Unknown
    Custom(String),
}

/// JOIN with time bounds
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TimeWindowJoin {
    pub left: TableFactor,
    pub right: TableFactor,
    pub join_type: JoinType,
    pub condition: Expr,
    pub within: IntervalValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JoinType {
    Inner,
    LeftOuter,
    RightOuter,
    FullOuter,
}
```

### 4.2 AST Visitor Pattern

```rust
// File: crates/sql-parser/src/visitor.rs

use crate::ast_ext::*;

/// Visitor trait for traversing streaming SQL AST
pub trait StreamingAstVisitor: Sized {
    type Error;

    fn visit_statement(&mut self, stmt: &StreamingStatement) -> Result<(), Self::Error> {
        match stmt {
            StreamingStatement::CreateStream(cs) => self.visit_create_stream(cs),
            StreamingStatement::CreateMaterializedView(cmv) => self.visit_create_materialized_view(cmv),
            StreamingStatement::Query(q) => self.visit_query(q),
        }
    }

    fn visit_create_stream(&mut self, stmt: &CreateStream) -> Result<(), Self::Error> {
        for col in &stmt.columns {
            self.visit_column_def(col)?;
        }
        for prop in &stmt.properties {
            self.visit_property(prop)?;
        }
        Ok(())
    }

    fn visit_create_materialized_view(&mut self, stmt: &CreateMaterializedView) -> Result<(), Self::Error> {
        self.visit_query(&stmt.query)?;
        for prop in &stmt.properties {
            self.visit_property(prop)?;
        }
        Ok(())
    }

    fn visit_query(&mut self, query: &StreamingQuery) -> Result<(), Self::Error> {
        // Visit standard query parts (using sqlparser-rs visitors)

        if let Some(window) = &query.window {
            self.visit_window_clause(window)?;
        }

        if let Some(watermark) = &query.watermark {
            self.visit_watermark_clause(watermark)?;
        }

        if let Some(emit) = &query.emit {
            self.visit_emit_clause(emit)?;
        }

        Ok(())
    }

    fn visit_window_clause(&mut self, clause: &WindowClause) -> Result<(), Self::Error> {
        for window in &clause.windows {
            self.visit_window_definition(window)?;
        }
        Ok(())
    }

    fn visit_window_definition(&mut self, window: &WindowDefinition) -> Result<(), Self::Error> {
        match window {
            WindowDefinition::Tumbling(w) => self.visit_tumbling_window(w),
            WindowDefinition::Sliding(w) => self.visit_sliding_window(w),
            WindowDefinition::Session(w) => self.visit_session_window(w),
            WindowDefinition::Hop(w) => self.visit_hop_window(w),
        }
    }

    fn visit_tumbling_window(&mut self, _window: &TumblingWindow) -> Result<(), Self::Error> {
        Ok(())
    }

    fn visit_sliding_window(&mut self, _window: &SlidingWindow) -> Result<(), Self::Error> {
        Ok(())
    }

    fn visit_session_window(&mut self, _window: &SessionWindow) -> Result<(), Self::Error> {
        Ok(())
    }

    fn visit_hop_window(&mut self, _window: &HopWindow) -> Result<(), Self::Error> {
        Ok(())
    }

    fn visit_watermark_clause(&mut self, _clause: &WatermarkClause) -> Result<(), Self::Error> {
        Ok(())
    }

    fn visit_emit_clause(&mut self, _clause: &EmitClause) -> Result<(), Self::Error> {
        Ok(())
    }

    fn visit_column_def(&mut self, _col: &ColumnDef) -> Result<(), Self::Error> {
        Ok(())
    }

    fn visit_property(&mut self, _prop: &StreamProperty) -> Result<(), Self::Error> {
        Ok(())
    }
}
```

---

## 5. Parser Implementation Plan

### 5.1 Phase 1: Setup & Basic Parsing (Week 1)

**Goal**: Parse standard SQL using sqlparser-rs

**Tasks**:
1. Create `sql-parser` crate
2. Add sqlparser-rs dependency
3. Implement basic parser wrapper
4. Add error handling
5. Write unit tests for standard SQL

**Deliverables**:
```rust
// File: crates/sql-parser/src/lib.rs

pub mod ast_ext;
pub mod dialect;
pub mod error;
pub mod parser;
pub mod semantic;
pub mod planner;
pub mod translator;
pub mod visitor;

pub use parser::{SqlParser, ParseOptions};
pub use error::{ParseError, ParseResult};
pub use ast_ext::StreamingStatement;
```

**Example Usage**:
```rust
use sql_parser::{SqlParser, ParseOptions};

let sql = "SELECT * FROM events WHERE timestamp > NOW()";
let parser = SqlParser::new();
let ast = parser.parse(sql)?;
```

### 5.2 Phase 2: Streaming Extensions (Week 2-3)

**Goal**: Parse streaming SQL constructs

**Tasks**:
1. Implement custom dialect extending sqlparser-rs
2. Parse WINDOW clause
3. Parse WATERMARK clause
4. Parse EMIT clause
5. Parse CREATE STREAM
6. Parse CREATE MATERIALIZED VIEW
7. Parse time-bounded JOINs

**Example**:
```rust
// File: crates/sql-parser/src/dialect.rs

use sqlparser::dialect::Dialect;
use sqlparser::keywords::Keyword;

#[derive(Debug, Clone)]
pub struct StreamingSqlDialect {
    base: Box<dyn Dialect>,
}

impl StreamingSqlDialect {
    pub fn new() -> Self {
        Self {
            // Use PostgreSQL as base dialect
            base: Box::new(sqlparser::dialect::PostgreSqlDialect {}),
        }
    }
}

impl Dialect for StreamingSqlDialect {
    fn is_identifier_start(&self, ch: char) -> bool {
        self.base.is_identifier_start(ch)
    }

    fn is_identifier_part(&self, ch: char) -> bool {
        self.base.is_identifier_part(ch)
    }

    fn is_delimited_identifier_start(&self, ch: char) -> bool {
        self.base.is_delimited_identifier_start(ch)
    }

    // Add custom keywords
    fn supports_keyword(&self, keyword: Keyword) -> bool {
        matches!(
            keyword,
            Keyword::WINDOW
            | Keyword::WATERMARK
            | Keyword::EMIT
            | Keyword::TUMBLING
            | Keyword::SLIDING
            | Keyword::SESSION
            | Keyword::HOP
            | Keyword::STREAM
        ) || self.base.supports_keyword(keyword)
    }
}
```

### 5.3 Phase 3: Semantic Analysis (Week 4)

**Goal**: Validate parsed AST

**Tasks**:
1. Type checking
2. Schema validation
3. Window validation
4. Watermark validation
5. Aggregation validation

**Example**:
```rust
// File: crates/sql-parser/src/semantic.rs

use crate::ast_ext::*;
use crate::error::{ParseError, ParseResult};
use std::collections::HashMap;

pub struct SemanticAnalyzer {
    /// Registered streams and their schemas
    streams: HashMap<String, StreamSchema>,

    /// Type inference engine
    type_checker: TypeChecker,
}

impl SemanticAnalyzer {
    pub fn analyze(&mut self, stmt: &StreamingStatement) -> ParseResult<AnalyzedStatement> {
        match stmt {
            StreamingStatement::Query(query) => self.analyze_query(query),
            StreamingStatement::CreateStream(create) => self.analyze_create_stream(create),
            StreamingStatement::CreateMaterializedView(view) => self.analyze_create_view(view),
        }
    }

    fn analyze_query(&mut self, query: &StreamingQuery) -> ParseResult<AnalyzedStatement> {
        // 1. Validate FROM clause - check stream exists
        let streams = self.extract_streams(&query.query)?;
        for stream_name in &streams {
            if !self.streams.contains_key(stream_name) {
                return Err(ParseError::UnknownStream(stream_name.clone()));
            }
        }

        // 2. Validate window clause
        if let Some(window) = &query.window {
            self.validate_window(window, &streams)?;
        }

        // 3. Validate watermark clause
        if let Some(watermark) = &query.watermark {
            self.validate_watermark(watermark, &streams)?;
        }

        // 4. Type check expressions
        self.type_check_query(query)?;

        // 5. Validate aggregations in GROUP BY
        self.validate_aggregations(query)?;

        Ok(AnalyzedStatement::Query(/* ... */))
    }

    fn validate_window(&self, window: &WindowClause, streams: &[String]) -> ParseResult<()> {
        for def in &window.windows {
            match def {
                WindowDefinition::Tumbling(w) => {
                    // Validate time column exists and is timestamp type
                    self.validate_time_column(&w.time_column, streams)?;

                    // Validate size is positive
                    if w.size.value <= 0 {
                        return Err(ParseError::InvalidWindowSize(w.size.value));
                    }
                }
                WindowDefinition::Sliding(w) => {
                    self.validate_time_column(&w.time_column, streams)?;

                    if w.size.value <= 0 || w.slide.value <= 0 {
                        return Err(ParseError::InvalidWindowSize(w.size.value));
                    }

                    if w.slide.value > w.size.value {
                        return Err(ParseError::InvalidSlidingWindow {
                            size: w.size.value,
                            slide: w.slide.value,
                        });
                    }
                }
                WindowDefinition::Session(w) => {
                    self.validate_time_column(&w.time_column, streams)?;

                    if w.gap.value <= 0 {
                        return Err(ParseError::InvalidSessionGap(w.gap.value));
                    }

                    if let Some(max) = &w.max_duration {
                        if max.value <= w.gap.value {
                            return Err(ParseError::InvalidSessionMaxDuration);
                        }
                    }
                }
                WindowDefinition::Hop(w) => {
                    self.validate_time_column(&w.time_column, streams)?;

                    if w.size.value <= 0 || w.hop.value <= 0 {
                        return Err(ParseError::InvalidWindowSize(w.size.value));
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_watermark(&self, watermark: &WatermarkClause, streams: &[String]) -> ParseResult<()> {
        // Validate watermark expression resolves to timestamp type
        // Validate delay is non-negative
        if watermark.delay.value < 0 {
            return Err(ParseError::NegativeWatermarkDelay(watermark.delay.value));
        }
        Ok(())
    }

    fn validate_time_column(&self, column: &Ident, streams: &[String]) -> ParseResult<()> {
        // Check column exists in one of the streams and has timestamp type
        for stream_name in streams {
            if let Some(schema) = self.streams.get(stream_name) {
                if let Some(col_type) = schema.get_column_type(&column.value) {
                    if matches!(col_type, DataType::Timestamp | DataType::TimestampTz) {
                        return Ok(());
                    }
                }
            }
        }

        Err(ParseError::InvalidTimeColumn(column.value.clone()))
    }
}

pub struct StreamSchema {
    pub columns: Vec<(String, DataType)>,
}

impl StreamSchema {
    pub fn get_column_type(&self, name: &str) -> Option<&DataType> {
        self.columns.iter()
            .find(|(col_name, _)| col_name == name)
            .map(|(_, data_type)| data_type)
    }
}

pub struct TypeChecker {
    // Type inference and checking logic
}
```

### 5.4 Phase 4: Logical Plan Generation (Week 5)

**Goal**: Convert validated AST to logical plan

```rust
// File: crates/sql-parser/src/planner.rs

use crate::ast_ext::*;
use crate::error::ParseResult;
use chrono::Duration;

/// Logical query plan
#[derive(Debug, Clone)]
pub struct LogicalPlan {
    pub root: Box<LogicalNode>,
}

#[derive(Debug, Clone)]
pub enum LogicalNode {
    /// Scan from stream
    Scan(ScanNode),

    /// Filter rows
    Filter(FilterNode),

    /// Project columns
    Project(ProjectNode),

    /// Aggregate with window
    WindowAggregate(WindowAggregateNode),

    /// Join streams
    Join(JoinNode),

    /// Sink to output
    Sink(SinkNode),
}

#[derive(Debug, Clone)]
pub struct ScanNode {
    pub stream_name: String,
    pub schema: Vec<(String, DataType)>,
}

#[derive(Debug, Clone)]
pub struct FilterNode {
    pub input: Box<LogicalNode>,
    pub predicate: Expression,
}

#[derive(Debug, Clone)]
pub struct ProjectNode {
    pub input: Box<LogicalNode>,
    pub expressions: Vec<(String, Expression)>,
}

#[derive(Debug, Clone)]
pub struct WindowAggregateNode {
    pub input: Box<LogicalNode>,
    pub window_type: WindowType,
    pub time_column: String,
    pub group_by: Vec<String>,
    pub aggregates: Vec<AggregateExpression>,
    pub watermark: WatermarkSpec,
    pub emit_strategy: EmitStrategy,
}

#[derive(Debug, Clone)]
pub enum WindowType {
    Tumbling { size: Duration },
    Sliding { size: Duration, slide: Duration },
    Session { gap: Duration, max_duration: Option<Duration> },
    Hop { size: Duration, hop: Duration },
}

#[derive(Debug, Clone)]
pub struct WatermarkSpec {
    pub column: String,
    pub delay: Duration,
}

#[derive(Debug, Clone)]
pub enum EmitStrategy {
    OnWatermark,
    OnWindowClose,
    After(Duration),
    Every(Duration),
    OnChange,
}

#[derive(Debug, Clone)]
pub struct AggregateExpression {
    pub name: String,
    pub function: AggregateFunction,
    pub argument: Option<Expression>,
}

#[derive(Debug, Clone)]
pub enum AggregateFunction {
    Count,
    Sum,
    Avg,
    Min,
    Max,
    Stddev,
    Variance,
    Percentile(f64),
    FirstValue,
    LastValue,
}

#[derive(Debug, Clone)]
pub enum Expression {
    Column(String),
    Literal(LiteralValue),
    BinaryOp {
        left: Box<Expression>,
        op: BinaryOperator,
        right: Box<Expression>,
    },
    UnaryOp {
        op: UnaryOperator,
        expr: Box<Expression>,
    },
    FunctionCall {
        name: String,
        args: Vec<Expression>,
    },
}

#[derive(Debug, Clone)]
pub enum LiteralValue {
    Int(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Null,
    Timestamp(i64),
}

pub struct LogicalPlanner {
    // Planner state
}

impl LogicalPlanner {
    pub fn plan(&mut self, stmt: &StreamingStatement) -> ParseResult<LogicalPlan> {
        match stmt {
            StreamingStatement::Query(query) => self.plan_query(query),
            StreamingStatement::CreateStream(_) => {
                // CREATE STREAM doesn't produce a logical plan
                // It's a DDL operation that updates the catalog
                Ok(LogicalPlan {
                    root: Box::new(LogicalNode::Scan(ScanNode {
                        stream_name: "".into(),
                        schema: vec![],
                    })),
                })
            }
            StreamingStatement::CreateMaterializedView(view) => {
                self.plan_query(&view.query)
            }
        }
    }

    fn plan_query(&mut self, query: &StreamingQuery) -> ParseResult<LogicalPlan> {
        // Extract FROM clause and create Scan nodes
        let scan = self.plan_from_clause(&query.query)?;

        // Add WHERE clause as Filter node
        let filtered = if let Some(where_expr) = self.extract_where(&query.query) {
            LogicalNode::Filter(FilterNode {
                input: Box::new(scan),
                predicate: self.convert_expr(where_expr)?,
            })
        } else {
            scan
        };

        // Add window aggregation if present
        let aggregated = if let Some(window) = &query.window {
            self.plan_window_aggregate(filtered, window, query)?
        } else {
            filtered
        };

        // Add projection (SELECT clause)
        let projected = self.plan_projection(aggregated, query)?;

        Ok(LogicalPlan {
            root: Box::new(projected),
        })
    }

    fn plan_window_aggregate(
        &mut self,
        input: LogicalNode,
        window: &WindowClause,
        query: &StreamingQuery,
    ) -> ParseResult<LogicalNode> {
        // Extract window type
        let window_type = self.convert_window_type(&window.windows[0])?;

        // Extract watermark spec
        let watermark = if let Some(wm) = &query.watermark {
            WatermarkSpec {
                column: self.extract_column_name(&wm.expression)?,
                delay: wm.delay.to_duration(),
            }
        } else {
            return Err(ParseError::MissingWatermark);
        };

        // Extract emit strategy
        let emit_strategy = if let Some(emit) = &query.emit {
            self.convert_emit_clause(emit)?
        } else {
            EmitStrategy::OnWatermark  // Default
        };

        // Extract aggregates and group by
        let (aggregates, group_by) = self.extract_aggregates(&query.query)?;

        Ok(LogicalNode::WindowAggregate(WindowAggregateNode {
            input: Box::new(input),
            window_type,
            time_column: "event_time".into(),  // Extract from query
            group_by,
            aggregates,
            watermark,
            emit_strategy,
        }))
    }
}
```

### 5.5 Phase 5: Physical Pipeline Translation (Week 6)

**Goal**: Translate logical plan to processor pipeline

```rust
// File: crates/sql-parser/src/translator.rs

use crate::planner::{LogicalPlan, LogicalNode, WindowType};
use crate::error::ParseResult;
use processor::{
    StreamPipelineBuilder, WindowConfig, WatermarkConfig, AggregationConfig,
};

pub struct PipelineTranslator {
    // Translator state
}

impl PipelineTranslator {
    pub fn translate(&mut self, plan: &LogicalPlan) -> ParseResult<StreamPipeline> {
        let mut builder = StreamPipelineBuilder::new();

        self.translate_node(&plan.root, &mut builder)?;

        Ok(builder.build()?)
    }

    fn translate_node(
        &mut self,
        node: &LogicalNode,
        builder: &mut StreamPipelineBuilder,
    ) -> ParseResult<()> {
        match node {
            LogicalNode::Scan(scan) => {
                // Configure source (e.g., Kafka)
                builder.source(/* ... */);
            }

            LogicalNode::Filter(filter) => {
                self.translate_node(&filter.input, builder)?;

                // Add filter operator
                builder.filter(|event| {
                    // Evaluate filter predicate
                    // This requires runtime expression evaluation
                    true
                });
            }

            LogicalNode::WindowAggregate(agg) => {
                self.translate_node(&agg.input, builder)?;

                // Configure windowing
                let window_config = match &agg.window_type {
                    WindowType::Tumbling { size } => WindowConfig::tumbling(*size),
                    WindowType::Sliding { size, slide } => WindowConfig::sliding(*size, *slide),
                    WindowType::Session { gap, max_duration } => {
                        WindowConfig::session(*gap, *max_duration)
                    }
                    WindowType::Hop { size, hop } => WindowConfig::hop(*size, *hop),
                };

                builder.window(window_config);

                // Configure watermark
                let watermark_config = WatermarkConfig {
                    column: agg.watermark.column.clone(),
                    delay: agg.watermark.delay,
                    strategy: WatermarkStrategy::BoundedOutOfOrder,
                };

                builder.watermark(watermark_config);

                // Configure aggregations
                for agg_expr in &agg.aggregates {
                    let agg_config = self.convert_aggregate(agg_expr)?;
                    builder.aggregate(agg_config);
                }
            }

            LogicalNode::Sink(sink) => {
                // Configure sink (e.g., Kafka, Postgres)
                builder.sink(/* ... */);
            }

            _ => {
                // Handle other node types
            }
        }

        Ok(())
    }
}
```

---

## 6. Error Handling Strategy

### 6.1 Error Types

```rust
// File: crates/sql-parser/src/error.rs

use thiserror::Error;
use std::fmt;

pub type ParseResult<T> = Result<T, ParseError>;

#[derive(Debug, Error)]
pub enum ParseError {
    /// Syntax error from sqlparser-rs
    #[error("Syntax error: {0}")]
    SyntaxError(#[from] sqlparser::parser::ParserError),

    /// Unknown stream reference
    #[error("Unknown stream: {0}")]
    UnknownStream(String),

    /// Invalid time column
    #[error("Invalid time column: {0}")]
    InvalidTimeColumn(String),

    /// Invalid window size
    #[error("Invalid window size: {0}")]
    InvalidWindowSize(i64),

    /// Invalid sliding window (slide > size)
    #[error("Invalid sliding window: slide ({slide}) > size ({size})")]
    InvalidSlidingWindow { size: i64, slide: i64 },

    /// Invalid session gap
    #[error("Invalid session gap: {0}")]
    InvalidSessionGap(i64),

    /// Invalid session max duration
    #[error("Invalid session max duration (must be > gap)")]
    InvalidSessionMaxDuration,

    /// Negative watermark delay
    #[error("Watermark delay cannot be negative: {0}")]
    NegativeWatermarkDelay(i64),

    /// Missing watermark clause
    #[error("WATERMARK clause required for windowed queries")]
    MissingWatermark,

    /// Type mismatch
    #[error("Type mismatch: expected {expected}, found {found}")]
    TypeMismatch { expected: String, found: String },

    /// Unsupported aggregation
    #[error("Unsupported aggregation: {0}")]
    UnsupportedAggregation(String),

    /// Invalid join
    #[error("Invalid join: {0}")]
    InvalidJoin(String),

    /// Semantic error
    #[error("Semantic error: {0}")]
    SemanticError(String),
}

impl ParseError {
    /// Get error location if available
    pub fn location(&self) -> Option<Location> {
        match self {
            ParseError::SyntaxError(e) => Some(Location {
                line: e.location.line,
                column: e.location.column,
            }),
            _ => None,
        }
    }

    /// Get user-friendly error message with context
    pub fn display_with_context(&self, sql: &str) -> String {
        let mut output = format!("Error: {}\n", self);

        if let Some(loc) = self.location() {
            // Show the problematic line with caret
            let lines: Vec<&str> = sql.lines().collect();
            if loc.line > 0 && loc.line <= lines.len() {
                let line = lines[loc.line - 1];
                output.push_str(&format!("\n{}\n", line));
                output.push_str(&format!("{}^\n", " ".repeat(loc.column.saturating_sub(1))));
            }
        }

        output
    }
}

#[derive(Debug, Clone)]
pub struct Location {
    pub line: usize,
    pub column: usize,
}
```

### 6.2 Error Recovery

For production use, implement error recovery strategies:

1. **Partial Parsing**: Continue parsing after errors to collect multiple errors
2. **Suggestions**: Provide "did you mean?" suggestions for typos
3. **Context**: Include surrounding SQL context in error messages
4. **Validation Levels**: Support strict vs lenient validation modes

---

## 7. Performance Characteristics

### 7.1 Performance Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| **Parse Time** | <10ms (p99) for queries <1KB | Sub-100ms query compilation |
| **Memory Usage** | <10KB per AST | Support 10,000s of queries in memory |
| **Throughput** | >10,000 queries/sec | Batch compilation for materialized views |
| **Startup Time** | <100ms | Include dialect initialization |

### 7.2 Optimization Strategies

#### 7.2.1 AST Reuse

```rust
use std::collections::HashMap;
use std::sync::Arc;

pub struct QueryCache {
    cache: HashMap<String, Arc<LogicalPlan>>,
}

impl QueryCache {
    pub fn get_or_parse(&mut self, sql: &str, parser: &SqlParser) -> ParseResult<Arc<LogicalPlan>> {
        if let Some(plan) = self.cache.get(sql) {
            return Ok(Arc::clone(plan));
        }

        let ast = parser.parse(sql)?;
        let plan = Arc::new(planner.plan(&ast)?);
        self.cache.insert(sql.to_string(), Arc::clone(&plan));

        Ok(plan)
    }
}
```

#### 7.2.2 Parallel Parsing

For batch scenarios (e.g., loading many materialized views):

```rust
use rayon::prelude::*;

pub fn parse_batch(queries: Vec<String>) -> Vec<ParseResult<LogicalPlan>> {
    queries.par_iter()
        .map(|sql| {
            let parser = SqlParser::new();
            parser.parse(sql)
        })
        .collect()
}
```

### 7.3 Benchmarks

```rust
// File: crates/sql-parser/benches/parser_bench.rs

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use sql_parser::SqlParser;

fn bench_parse_simple(c: &mut Criterion) {
    let sql = "SELECT * FROM events WHERE timestamp > NOW()";
    let parser = SqlParser::new();

    c.bench_function("parse_simple", |b| {
        b.iter(|| parser.parse(black_box(sql)))
    });
}

fn bench_parse_window(c: &mut Criterion) {
    let sql = r#"
        SELECT
            model,
            COUNT(*) as count,
            AVG(latency_ms) as avg_latency
        FROM performance_metrics
        WINDOW TUMBLING(event_time, INTERVAL '5' MINUTE)
        GROUP BY model, window_start, window_end
        WATERMARK event_time DELAY OF INTERVAL '2' MINUTE
        EMIT ON WATERMARK
    "#;

    let parser = SqlParser::new();

    c.bench_function("parse_window", |b| {
        b.iter(|| parser.parse(black_box(sql)))
    });
}

fn bench_parse_complex(c: &mut Criterion) {
    let sql = r#"
        SELECT
            pm.model,
            pm.request_id,
            pm.latency_ms,
            uf.rating,
            pm.event_time as metric_time,
            uf.event_time as feedback_time
        FROM performance_metrics pm
        JOIN user_feedback uf
            ON pm.request_id = uf.request_id
            WITHIN INTERVAL '5' MINUTE
        WHERE pm.error IS NULL
            AND uf.rating IS NOT NULL
        WATERMARK pm.event_time DELAY OF INTERVAL '2' MINUTE
        WATERMARK uf.event_time DELAY OF INTERVAL '10' MINUTE
    "#;

    let parser = SqlParser::new();

    c.bench_function("parse_complex", |b| {
        b.iter(|| parser.parse(black_box(sql)))
    });
}

criterion_group!(benches, bench_parse_simple, bench_parse_window, bench_parse_complex);
criterion_main!(benches);
```

**Expected Results** (on modern hardware):
- Simple query: ~500ns
- Window query: ~5-10μs
- Complex query: ~20-30μs

---

## 8. Testing Approach

### 8.1 Test Strategy

| Test Level | Coverage | Tools |
|------------|----------|-------|
| **Unit Tests** | Individual parser components | `cargo test` |
| **Integration Tests** | End-to-end SQL parsing | `cargo test --test '*'` |
| **Property Tests** | Grammar invariants | `proptest` |
| **Fuzzing** | Random input robustness | `cargo-fuzz` |
| **Benchmarks** | Performance regression | `criterion` |

### 8.2 Test Organization

```
crates/sql-parser/
├── tests/
│   ├── parser_tests.rs           # Basic parsing tests
│   ├── window_tests.rs            # Window clause tests
│   ├── watermark_tests.rs         # Watermark clause tests
│   ├── emit_tests.rs              # Emit clause tests
│   ├── join_tests.rs              # Join tests
│   ├── create_stream_tests.rs     # CREATE STREAM tests
│   ├── create_view_tests.rs       # CREATE MATERIALIZED VIEW tests
│   ├── semantic_tests.rs          # Semantic analysis tests
│   ├── error_tests.rs             # Error handling tests
│   ├── integration_tests.rs       # End-to-end tests
│   └── regression_tests.rs        # Known bug tests
└── benches/
    └── parser_bench.rs
```

### 8.3 Example Tests

```rust
// File: crates/sql-parser/tests/window_tests.rs

use sql_parser::{SqlParser, StreamingStatement, WindowDefinition};

#[test]
fn test_parse_tumbling_window() {
    let sql = r#"
        SELECT
            model,
            COUNT(*) as count
        FROM events
        WINDOW TUMBLING(event_time, INTERVAL '5' MINUTE)
        GROUP BY model
        WATERMARK event_time DELAY OF INTERVAL '2' MINUTE
    "#;

    let parser = SqlParser::new();
    let result = parser.parse(sql).unwrap();

    match result {
        StreamingStatement::Query(query) => {
            let window = query.window.unwrap();
            assert_eq!(window.windows.len(), 1);

            match &window.windows[0] {
                WindowDefinition::Tumbling(w) => {
                    assert_eq!(w.time_column.value, "event_time");
                    assert_eq!(w.size.value, 5);
                    assert_eq!(w.size.unit, TimeUnit::Minute);
                }
                _ => panic!("Expected tumbling window"),
            }
        }
        _ => panic!("Expected query statement"),
    }
}

#[test]
fn test_parse_sliding_window() {
    let sql = r#"
        SELECT AVG(latency)
        FROM metrics
        WINDOW SLIDING(ts, INTERVAL '15' MINUTE, INTERVAL '1' MINUTE)
        WATERMARK ts DELAY OF INTERVAL '1' MINUTE
    "#;

    let parser = SqlParser::new();
    let result = parser.parse(sql).unwrap();

    // Assertions...
}

#[test]
fn test_invalid_window_size() {
    let sql = r#"
        SELECT * FROM events
        WINDOW TUMBLING(ts, INTERVAL '-5' MINUTE)
    "#;

    let parser = SqlParser::new();
    let result = parser.parse(sql);

    assert!(result.is_err());
    match result.unwrap_err() {
        ParseError::InvalidWindowSize(size) => assert_eq!(size, -5),
        _ => panic!("Expected InvalidWindowSize error"),
    }
}

#[test]
fn test_sliding_window_validation() {
    // Slide > Size should fail
    let sql = r#"
        SELECT * FROM events
        WINDOW SLIDING(ts, INTERVAL '5' MINUTE, INTERVAL '10' MINUTE)
    "#;

    let parser = SqlParser::new();
    let result = parser.parse(sql);

    assert!(result.is_err());
}
```

### 8.4 Property-Based Testing

```rust
// File: crates/sql-parser/tests/property_tests.rs

use proptest::prelude::*;
use sql_parser::SqlParser;

proptest! {
    #[test]
    fn test_parse_roundtrip(sql in any_valid_sql()) {
        let parser = SqlParser::new();

        // Parse SQL
        let ast = parser.parse(&sql).unwrap();

        // Convert back to SQL
        let sql2 = ast.to_string();

        // Parse again
        let ast2 = parser.parse(&sql2).unwrap();

        // ASTs should be equal
        prop_assert_eq!(ast, ast2);
    }

    #[test]
    fn test_window_sizes(size in 1i64..1000) {
        let sql = format!(
            "SELECT * FROM events WINDOW TUMBLING(ts, INTERVAL '{}' MINUTE)",
            size
        );

        let parser = SqlParser::new();
        let result = parser.parse(&sql);

        prop_assert!(result.is_ok());
    }
}

fn any_valid_sql() -> impl Strategy<Value = String> {
    // Generate valid SQL strings
    prop::string::string_regex(r"SELECT .* FROM .*").unwrap()
}
```

---

## 9. Integration with Stream Processor

### 9.1 Integration Architecture

```rust
// File: crates/sql-parser/examples/integration.rs

use sql_parser::{SqlParser, PipelineTranslator};
use processor::{StreamPipelineBuilder, StreamExecutor};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Parse SQL
    let sql = r#"
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
    "#;

    let parser = SqlParser::new();
    let ast = parser.parse(sql)?;

    // 2. Generate logical plan
    let mut planner = LogicalPlanner::new();
    let logical_plan = planner.plan(&ast)?;

    // 3. Translate to physical pipeline
    let mut translator = PipelineTranslator::new();
    let pipeline = translator.translate(&logical_plan)?;

    // 4. Execute pipeline
    let executor = StreamExecutor::new(pipeline);
    executor.run().await?;

    Ok(())
}
```

### 9.2 SQL Interface for Processor

Add SQL support to the processor crate:

```rust
// File: crates/processor/src/sql.rs

use sql_parser::{SqlParser, LogicalPlanner, PipelineTranslator};
use crate::{StreamPipeline, ProcessorError};

pub struct SqlStreamProcessor {
    parser: SqlParser,
    planner: LogicalPlanner,
    translator: PipelineTranslator,
}

impl SqlStreamProcessor {
    pub fn new() -> Self {
        Self {
            parser: SqlParser::new(),
            planner: LogicalPlanner::new(),
            translator: PipelineTranslator::new(),
        }
    }

    /// Compile SQL query to executable pipeline
    pub fn compile(&mut self, sql: &str) -> Result<StreamPipeline, ProcessorError> {
        // Parse
        let ast = self.parser.parse(sql)
            .map_err(|e| ProcessorError::SqlParseError(e.to_string()))?;

        // Plan
        let logical_plan = self.planner.plan(&ast)
            .map_err(|e| ProcessorError::PlanningError(e.to_string()))?;

        // Translate
        let pipeline = self.translator.translate(&logical_plan)
            .map_err(|e| ProcessorError::TranslationError(e.to_string()))?;

        Ok(pipeline)
    }

    /// Execute SQL query directly
    pub async fn execute(&mut self, sql: &str) -> Result<(), ProcessorError> {
        let pipeline = self.compile(sql)?;

        let executor = StreamExecutor::new(pipeline);
        executor.run().await?;

        Ok(())
    }
}
```

---

## 10. Future Extensions

### 10.1 Near-Term Extensions (3-6 months)

1. **User-Defined Functions (UDFs)**
   ```sql
   CREATE FUNCTION calculate_quality(latency DOUBLE, error INT)
   RETURNS DOUBLE
   AS 'llm_quality_scorer';

   SELECT model, calculate_quality(latency_ms, error_count) as quality
   FROM metrics;
   ```

2. **User-Defined Aggregates (UDAFs)**
   ```sql
   CREATE AGGREGATE FUNCTION custom_percentile(value DOUBLE, p DOUBLE)
   RETURNS DOUBLE
   AS 'custom_percentile_impl';
   ```

3. **Pattern Matching (CEP)**
   ```sql
   SELECT *
   FROM performance_metrics
   MATCH_RECOGNIZE (
       PARTITION BY model
       ORDER BY event_time
       MEASURES A.latency as first_latency, B.latency as second_latency
       PATTERN (A B+)
       DEFINE B AS B.latency > A.latency * 1.5
   );
   ```

4. **Versioned Streams**
   ```sql
   CREATE STREAM metrics_v2 (
       -- new schema
   ) WITH (
       VERSION = '2.0',
       UPGRADE_FROM = 'metrics_v1',
       MIGRATION = 'metrics_migration_v1_to_v2'
   );
   ```

### 10.2 Long-Term Extensions (6-12 months)

1. **Machine Learning Integration**
   ```sql
   CREATE MODEL anomaly_detector
   USING ISOLATION_FOREST
   WITH (
       CONTAMINATION = 0.05,
       FEATURES = ['latency_ms', 'error_rate', 'tokens_per_sec']
   );

   SELECT model, PREDICT(anomaly_detector, *) as is_anomaly
   FROM metrics;
   ```

2. **Distributed Query Execution**
   - Multi-node query planning
   - Shuffle operations
   - Distributed joins
   - Dynamic scaling

3. **Query Optimization**
   - Cost-based optimization
   - Join reordering
   - Predicate pushdown
   - Projection pruning

4. **Materialized View Maintenance**
   - Incremental view updates
   - View dependencies
   - Automatic view refresh

---

## Appendix A: Dependencies

Add to `crates/sql-parser/Cargo.toml`:

```toml
[package]
name = "sql-parser"
version = "0.1.0"
edition = "2021"

[dependencies]
# SQL Parser
sqlparser = "0.54"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Date/Time
chrono = { version = "0.4", features = ["serde"] }

# Error Handling
thiserror = "1.0"
anyhow = "1.0"

# Utilities
uuid = { version = "1.0", features = ["v4", "serde"] }

# Internal
llm-optimizer-types = { path = "../types" }
processor = { path = "../processor" }

[dev-dependencies]
# Testing
proptest = "1.0"
criterion = { version = "0.5", features = ["async_tokio"] }
tokio = { version = "1.0", features = ["full"] }

[[bench]]
name = "parser_bench"
harness = false
```

---

## Appendix B: Example Usage

### B.1 Simple Query Execution

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

### B.2 Query Compilation and Reuse

```rust
let mut processor = SqlStreamProcessor::new();

// Compile once
let pipeline = processor.compile(sql)?;

// Execute many times
for _ in 0..100 {
    let executor = StreamExecutor::new(pipeline.clone());
    executor.run().await?;
}
```

### B.3 Error Handling

```rust
match processor.compile(sql) {
    Ok(pipeline) => {
        println!("Query compiled successfully");
    }
    Err(ProcessorError::SqlParseError(msg)) => {
        eprintln!("Parse error: {}", msg);
    }
    Err(ProcessorError::PlanningError(msg)) => {
        eprintln!("Planning error: {}", msg);
    }
    Err(e) => {
        eprintln!("Unexpected error: {}", e);
    }
}
```

---

## Appendix C: Migration Path

For existing processor pipelines built with the Rust API, provide a migration path:

### C.1 Before (Rust API)

```rust
let pipeline = StreamPipelineBuilder::new()
    .source(kafka_source)
    .window(WindowConfig::tumbling(Duration::minutes(5)))
    .watermark(WatermarkConfig {
        column: "event_time".into(),
        delay: Duration::minutes(2),
        strategy: WatermarkStrategy::BoundedOutOfOrder,
    })
    .aggregate(AggregationConfig {
        function: AggregationType::Avg,
        column: "latency_ms".into(),
    })
    .sink(postgres_sink)
    .build()?;
```

### C.2 After (SQL)

```sql
SELECT AVG(latency_ms)
FROM metrics
WINDOW TUMBLING(event_time, INTERVAL '5' MINUTE)
WATERMARK event_time DELAY OF INTERVAL '2' MINUTE
```

Both produce identical execution plans.

---

## Appendix D: References

1. **sqlparser-rs**: https://github.com/apache/datafusion-sqlparser-rs
2. **Apache DataFusion**: https://datafusion.apache.org/
3. **Apache Flink SQL**: https://nightlies.apache.org/flink/flink-docs-master/docs/dev/table/sql/
4. **ksqlDB Documentation**: https://docs.ksqldb.io/
5. **SQL:2023 Standard**: ISO/IEC 9075:2023
6. **Streaming SQL Extensions (Calcite)**: https://calcite.apache.org/docs/stream.html
7. **RisingWave**: https://docs.risingwave.com/
8. **Timeplus**: https://docs.timeplus.com/

---

**Document Version**: 1.0
**Last Updated**: 2025-11-10
**Next Review**: 2025-12-10
**Status**: Ready for Implementation
