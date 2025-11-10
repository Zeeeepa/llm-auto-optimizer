# SQL Interface Implementation Plan

**Version:** 1.0
**Date:** 2025-11-10
**Status:** Implementation Roadmap
**Owner:** LLM Auto-Optimizer Team
**Estimated Effort:** 8-12 weeks, 5,000-10,000 lines of code

---

## Executive Summary

This document provides a comprehensive, phased implementation plan for adding a SQL interface to the LLM Auto-Optimizer's stream processing engine. The SQL interface will enable users to query streaming LLM metrics, feedback events, and performance data using familiar SQL syntax with streaming-specific extensions.

### Vision

```sql
-- Example: Real-time LLM performance monitoring with SQL
SELECT
    model_id,
    TUMBLE_START(event_time, INTERVAL '5' MINUTE) as window_start,
    AVG(latency_ms) as avg_latency,
    PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY latency_ms) as p95_latency,
    COUNT(*) as request_count,
    SUM(cost_usd) as total_cost
FROM llm_requests
WHERE event_time BETWEEN CURRENT_TIMESTAMP - INTERVAL '1' HOUR AND CURRENT_TIMESTAMP
GROUP BY model_id, TUMBLE(event_time, INTERVAL '5' MINUTE)
HAVING AVG(latency_ms) > 1000
ORDER BY window_start DESC;
```

### Key Benefits

1. **Familiar Interface**: SQL is widely known, reducing learning curve
2. **Ad-hoc Analysis**: Enable data analysts to query streaming data without writing Rust code
3. **Integration**: Easy integration with BI tools (Grafana, Tableau, Metabase)
4. **Optimization**: Query planner can optimize execution automatically
5. **Standardization**: Follow SQL:2016 and SQL:2023 streaming extensions

---

## Table of Contents

1. [Phase-by-Phase Breakdown](#1-phase-by-phase-breakdown)
2. [Complete File Structure](#2-complete-file-structure)
3. [Module Interface Definitions](#3-module-interface-definitions)
4. [Implementation Schedule](#4-implementation-schedule)
5. [Testing Strategy](#5-testing-strategy)
6. [Dependencies and Integration](#6-dependencies-and-integration)
7. [Success Criteria](#7-success-criteria)
8. [Reference Implementation Examples](#8-reference-implementation-examples)

---

## 1. Phase-by-Phase Breakdown

### Phase 1: Basic SELECT Queries (Week 1-2)

**Goal**: Support simple SELECT statements with filtering and projection

**Deliverables**:
- SQL parser for SELECT, FROM, WHERE
- Basic data types (INTEGER, BIGINT, DOUBLE, VARCHAR, TIMESTAMP)
- Simple expressions (arithmetic, comparison, logical operators)
- Scan operator for reading from streams
- Filter operator for WHERE clauses
- Project operator for column selection

**SQL Support**:
```sql
-- Supported queries in Phase 1
SELECT model_id, latency_ms, cost_usd
FROM llm_requests
WHERE latency_ms > 100;

SELECT *
FROM user_feedback
WHERE rating >= 4 AND created_at > '2025-11-01';
```

**Files to Create** (8 files, ~1,200 LOC):
- `crates/sql/src/parser/mod.rs` - Parser infrastructure
- `crates/sql/src/parser/lexer.rs` - Tokenization
- `crates/sql/src/parser/select.rs` - SELECT parsing
- `crates/sql/src/parser/expr.rs` - Expression parsing
- `crates/sql/src/planner/mod.rs` - Planner infrastructure
- `crates/sql/src/planner/logical.rs` - Logical plan generation
- `crates/sql/src/executor/mod.rs` - Executor infrastructure
- `crates/sql/src/executor/operators/scan.rs` - Table scan

**Testing**:
- 20+ parser unit tests
- 15+ executor integration tests
- Property-based testing for expression evaluation

---

### Phase 2: Aggregations and GROUP BY (Week 3-4)

**Goal**: Support streaming aggregations with grouping

**Deliverables**:
- Aggregate functions (COUNT, SUM, AVG, MIN, MAX)
- GROUP BY clause support
- HAVING clause for post-aggregation filtering
- Hash-based aggregation operator
- Integration with existing windowing system

**SQL Support**:
```sql
-- Supported queries in Phase 2
SELECT model_id, COUNT(*) as request_count, AVG(latency_ms) as avg_latency
FROM llm_requests
GROUP BY model_id;

SELECT user_id, AVG(rating) as avg_rating
FROM user_feedback
GROUP BY user_id
HAVING AVG(rating) < 3.0;

-- Statistical aggregates
SELECT
    PERCENTILE_CONT(0.50) WITHIN GROUP (ORDER BY latency_ms) as p50,
    PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY latency_ms) as p95,
    PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY latency_ms) as p99
FROM llm_requests
GROUP BY model_id;
```

**Files to Create** (7 files, ~1,800 LOC):
- `crates/sql/src/parser/aggregate.rs` - Aggregate parsing
- `crates/sql/src/planner/aggregate.rs` - Aggregation planning
- `crates/sql/src/executor/operators/aggregate.rs` - Hash aggregate
- `crates/sql/src/functions/mod.rs` - Function registry
- `crates/sql/src/functions/aggregate.rs` - Aggregate functions
- `crates/sql/src/functions/math.rs` - Math functions
- `crates/sql/src/functions/string.rs` - String functions

**Testing**:
- 25+ aggregation correctness tests
- 10+ HAVING clause tests
- Performance benchmarks for hash aggregation

---

### Phase 3: Window Functions (Week 5-6)

**Goal**: Support streaming window operations (TUMBLING, SLIDING, SESSION)

**Deliverables**:
- TUMBLE, HOP (sliding), SESSION window functions
- Window assignment in planner
- Integration with existing `crates/processor/window` module
- Watermark-aware execution
- Late data handling

**SQL Support**:
```sql
-- Tumbling windows (non-overlapping)
SELECT
    model_id,
    TUMBLE_START(event_time, INTERVAL '5' MINUTE) as window_start,
    TUMBLE_END(event_time, INTERVAL '5' MINUTE) as window_end,
    COUNT(*) as request_count,
    AVG(latency_ms) as avg_latency
FROM llm_requests
GROUP BY model_id, TUMBLE(event_time, INTERVAL '5' MINUTE);

-- Sliding windows (overlapping)
SELECT
    HOP_START(event_time, INTERVAL '1' MINUTE, INTERVAL '5' MINUTE) as window_start,
    AVG(latency_ms) as avg_latency
FROM llm_requests
GROUP BY HOP(event_time, INTERVAL '1' MINUTE, INTERVAL '5' MINUTE);

-- Session windows (gap-based)
SELECT
    user_id,
    SESSION_START(event_time, INTERVAL '30' MINUTE) as session_start,
    SESSION_END(event_time, INTERVAL '30' MINUTE) as session_end,
    COUNT(*) as interaction_count
FROM user_feedback
GROUP BY user_id, SESSION(event_time, INTERVAL '30' MINUTE);
```

**Files to Create** (6 files, ~1,500 LOC):
- `crates/sql/src/parser/window.rs` - Window clause parsing
- `crates/sql/src/planner/window.rs` - Window assignment planning
- `crates/sql/src/executor/operators/window.rs` - Window operator
- `crates/sql/src/executor/watermark.rs` - Watermark handling
- `crates/sql/src/functions/window.rs` - Window functions
- `crates/sql/src/time.rs` - Time/interval handling

**Testing**:
- 30+ window function tests (TUMBLE, HOP, SESSION)
- 15+ watermark propagation tests
- Late data handling scenarios
- Window state cleanup tests

---

### Phase 4: Joins (Week 7-8)

**Goal**: Support INNER and LEFT joins between streams

**Deliverables**:
- INNER JOIN support
- LEFT OUTER JOIN support
- Join condition parsing (ON clause)
- Hash join and stream-stream join operators
- Time-bounded joins for streaming

**SQL Support**:
```sql
-- INNER JOIN between streams
SELECT
    r.model_id,
    r.request_id,
    f.rating,
    f.feedback_text
FROM llm_requests r
INNER JOIN user_feedback f
ON r.request_id = f.request_id
WHERE f.rating < 3;

-- LEFT OUTER JOIN with time bounds
SELECT
    r.request_id,
    r.latency_ms,
    f.rating
FROM llm_requests r
LEFT JOIN user_feedback f
ON r.request_id = f.request_id
WHERE r.event_time BETWEEN f.event_time - INTERVAL '5' MINUTE
                       AND f.event_time + INTERVAL '5' MINUTE;
```

**Files to Create** (5 files, ~1,400 LOC):
- `crates/sql/src/parser/join.rs` - Join parsing
- `crates/sql/src/planner/join.rs` - Join planning & optimization
- `crates/sql/src/executor/operators/hash_join.rs` - Hash join
- `crates/sql/src/executor/operators/stream_join.rs` - Stream-stream join
- `crates/sql/src/executor/state.rs` - Join state management

**Testing**:
- 20+ join correctness tests
- 10+ time-bounded join tests
- State eviction tests
- Join reordering optimization tests

---

### Phase 5: DDL (CREATE STREAM, CREATE TABLE) (Week 9)

**Goal**: Support data definition for streams and tables

**Deliverables**:
- CREATE STREAM statement
- CREATE TABLE statement
- Schema catalog
- Stream/table metadata management

**SQL Support**:
```sql
-- Define a stream
CREATE STREAM llm_requests (
    request_id VARCHAR NOT NULL,
    model_id VARCHAR NOT NULL,
    event_time TIMESTAMP NOT NULL,
    latency_ms BIGINT,
    cost_usd DOUBLE,
    token_count INTEGER,
    WATERMARK FOR event_time AS event_time - INTERVAL '30' SECOND
) WITH (
    'connector' = 'kafka',
    'topic' = 'llm.requests',
    'properties.bootstrap.servers' = 'localhost:9092',
    'format' = 'json',
    'scan.startup.mode' = 'latest-offset'
);

-- Define a table (for lookups)
CREATE TABLE model_metadata (
    model_id VARCHAR PRIMARY KEY,
    model_name VARCHAR,
    provider VARCHAR,
    cost_per_token DOUBLE
) WITH (
    'connector' = 'postgres',
    'url' = 'postgresql://localhost/optimizer'
);
```

**Files to Create** (6 files, ~1,200 LOC):
- `crates/sql/src/parser/ddl.rs` - DDL parsing
- `crates/sql/src/catalog/mod.rs` - Catalog infrastructure
- `crates/sql/src/catalog/schema.rs` - Schema definitions
- `crates/sql/src/catalog/stream.rs` - Stream catalog
- `crates/sql/src/catalog/table.rs` - Table catalog
- `crates/sql/src/connector/mod.rs` - Connector framework

**Testing**:
- 15+ DDL parsing tests
- Catalog persistence tests
- Schema evolution tests

---

### Phase 6: Advanced Features (Week 10-12)

**Goal**: Support subqueries, CTEs, FULL joins, and optimizations

**Deliverables**:
- Subquery support (scalar, correlated)
- Common Table Expressions (WITH clause)
- FULL OUTER JOIN
- Query optimization (predicate pushdown, projection pushdown)
- Cost-based optimization

**SQL Support**:
```sql
-- Common Table Expression
WITH high_latency_models AS (
    SELECT model_id, AVG(latency_ms) as avg_latency
    FROM llm_requests
    WHERE event_time > CURRENT_TIMESTAMP - INTERVAL '1' HOUR
    GROUP BY model_id
    HAVING AVG(latency_ms) > 500
)
SELECT
    r.request_id,
    r.model_id,
    r.latency_ms,
    h.avg_latency as model_avg_latency
FROM llm_requests r
INNER JOIN high_latency_models h ON r.model_id = h.model_id;

-- Scalar subquery
SELECT
    model_id,
    COUNT(*) as request_count,
    COUNT(*) * 100.0 / (SELECT COUNT(*) FROM llm_requests) as percentage
FROM llm_requests
GROUP BY model_id;

-- Window aggregate functions
SELECT
    request_id,
    latency_ms,
    AVG(latency_ms) OVER (
        PARTITION BY model_id
        ORDER BY event_time
        ROWS BETWEEN 10 PRECEDING AND CURRENT ROW
    ) as moving_avg_latency
FROM llm_requests;
```

**Files to Create** (8 files, ~1,900 LOC):
- `crates/sql/src/parser/subquery.rs` - Subquery parsing
- `crates/sql/src/parser/cte.rs` - CTE parsing
- `crates/sql/src/planner/subquery.rs` - Subquery planning
- `crates/sql/src/planner/optimizer/mod.rs` - Optimization framework
- `crates/sql/src/planner/optimizer/predicate_pushdown.rs`
- `crates/sql/src/planner/optimizer/projection_pushdown.rs`
- `crates/sql/src/planner/optimizer/join_reorder.rs`
- `crates/sql/src/planner/cost.rs` - Cost model

**Testing**:
- 25+ subquery tests
- 15+ CTE tests
- 20+ optimization rule tests
- Performance regression tests

---

## 2. Complete File Structure

```
crates/sql/
├── Cargo.toml                          # Dependencies and metadata
├── README.md                           # SQL crate documentation
├── benches/
│   ├── parser_benchmark.rs            # Parser performance tests
│   ├── executor_benchmark.rs          # Query execution benchmarks
│   └── end_to_end_benchmark.rs        # Full query benchmarks
├── examples/
│   ├── basic_select.rs                # Simple SELECT example
│   ├── aggregation.rs                 # GROUP BY example
│   ├── windowing.rs                   # TUMBLE/HOP/SESSION example
│   ├── joins.rs                       # JOIN example
│   └── advanced.rs                    # CTE, subquery example
├── src/
│   ├── lib.rs                         # Public API exports
│   ├── error.rs                       # Error types
│   ├── types.rs                       # SQL data types
│   ├── time.rs                        # Time and interval types
│   │
│   ├── parser/                        # SQL Parser (1,800 LOC)
│   │   ├── mod.rs                     # Parser infrastructure
│   │   ├── lexer.rs                   # Tokenizer/lexer
│   │   ├── token.rs                   # Token definitions
│   │   ├── select.rs                  # SELECT statement parsing
│   │   ├── expr.rs                    # Expression parsing
│   │   ├── aggregate.rs               # Aggregate parsing
│   │   ├── window.rs                  # Window clause parsing
│   │   ├── join.rs                    # JOIN parsing
│   │   ├── ddl.rs                     # DDL (CREATE STREAM/TABLE)
│   │   ├── subquery.rs                # Subquery parsing
│   │   ├── cte.rs                     # CTE (WITH) parsing
│   │   └── utils.rs                   # Parser utilities
│   │
│   ├── ast/                           # Abstract Syntax Tree (800 LOC)
│   │   ├── mod.rs                     # AST node definitions
│   │   ├── statement.rs               # Statement nodes
│   │   ├── expr.rs                    # Expression nodes
│   │   ├── data_type.rs               # Data type nodes
│   │   ├── visitor.rs                 # AST visitor pattern
│   │   └── display.rs                 # Pretty-printing AST
│   │
│   ├── planner/                       # Query Planner (2,200 LOC)
│   │   ├── mod.rs                     # Planner infrastructure
│   │   ├── logical.rs                 # Logical plan generation
│   │   ├── physical.rs                # Physical plan generation
│   │   ├── aggregate.rs               # Aggregation planning
│   │   ├── window.rs                  # Window planning
│   │   ├── join.rs                    # Join planning
│   │   ├── subquery.rs                # Subquery planning
│   │   ├── cost.rs                    # Cost model
│   │   ├── optimizer/
│   │   │   ├── mod.rs                 # Optimizer framework
│   │   │   ├── rule.rs                # Optimization rule trait
│   │   │   ├── predicate_pushdown.rs  # Push filters down
│   │   │   ├── projection_pushdown.rs # Push projections down
│   │   │   ├── join_reorder.rs        # Join reordering
│   │   │   ├── constant_folding.rs    # Constant folding
│   │   │   └── rule_set.rs            # Rule application
│   │   └── plan/
│   │       ├── mod.rs                 # Plan node definitions
│   │       ├── logical.rs             # Logical plan nodes
│   │       └── physical.rs            # Physical plan nodes
│   │
│   ├── executor/                      # Query Executor (2,500 LOC)
│   │   ├── mod.rs                     # Executor infrastructure
│   │   ├── context.rs                 # Execution context
│   │   ├── stream.rs                  # Stream abstraction
│   │   ├── batch.rs                   # Batch processing
│   │   ├── watermark.rs               # Watermark handling
│   │   ├── state.rs                   # State management
│   │   ├── operators/
│   │   │   ├── mod.rs                 # Operator trait
│   │   │   ├── scan.rs                # Table/stream scan
│   │   │   ├── filter.rs              # Filter operator
│   │   │   ├── project.rs             # Projection operator
│   │   │   ├── aggregate.rs           # Hash aggregate
│   │   │   ├── window.rs              # Window operator
│   │   │   ├── hash_join.rs           # Hash join
│   │   │   ├── stream_join.rs         # Stream-stream join
│   │   │   ├── sort.rs                # Sort operator
│   │   │   ├── limit.rs               # Limit operator
│   │   │   └── union.rs               # Union operator
│   │   └── runtime/
│   │       ├── mod.rs                 # Runtime infrastructure
│   │       ├── task.rs                # Task scheduling
│   │       └── pipeline.rs            # Pipeline execution
│   │
│   ├── functions/                     # Built-in Functions (1,200 LOC)
│   │   ├── mod.rs                     # Function registry
│   │   ├── aggregate.rs               # Aggregate functions
│   │   ├── scalar.rs                  # Scalar functions
│   │   ├── window.rs                  # Window functions
│   │   ├── math.rs                    # Math functions
│   │   ├── string.rs                  # String functions
│   │   ├── date_time.rs               # Date/time functions
│   │   ├── json.rs                    # JSON functions
│   │   └── udf.rs                     # User-defined functions
│   │
│   ├── catalog/                       # Metadata Catalog (900 LOC)
│   │   ├── mod.rs                     # Catalog infrastructure
│   │   ├── schema.rs                  # Schema definitions
│   │   ├── stream.rs                  # Stream catalog
│   │   ├── table.rs                   # Table catalog
│   │   ├── function.rs                # Function catalog
│   │   └── storage.rs                 # Catalog persistence
│   │
│   ├── connector/                     # Data Connectors (800 LOC)
│   │   ├── mod.rs                     # Connector framework
│   │   ├── kafka.rs                   # Kafka connector
│   │   ├── postgres.rs                # PostgreSQL connector
│   │   ├── file.rs                    # File connector (CSV, JSON)
│   │   └── memory.rs                  # In-memory connector (testing)
│   │
│   └── api/                           # Public API (300 LOC)
│       ├── mod.rs                     # API exports
│       ├── session.rs                 # SQL session
│       ├── statement.rs               # Statement execution
│       └── result.rs                  # Query results
│
└── tests/
    ├── integration/
    │   ├── basic_queries.rs           # Basic SELECT tests
    │   ├── aggregations.rs            # GROUP BY tests
    │   ├── windows.rs                 # Window function tests
    │   ├── joins.rs                   # JOIN tests
    │   ├── ddl.rs                     # DDL tests
    │   └── advanced.rs                # Subquery/CTE tests
    ├── compatibility/
    │   ├── postgres_compat.rs         # PostgreSQL compatibility
    │   └── flink_compat.rs            # Flink SQL compatibility
    └── performance/
        ├── tpc_h.rs                   # TPC-H inspired benchmarks
        └── streaming_bench.rs         # Streaming workload tests
```

**Total File Count**: 47 source files + 12 test files = 59 files
**Estimated Lines of Code**: 9,900 LOC (production) + 2,000 LOC (tests) = 11,900 LOC

---

## 3. Module Interface Definitions

### 3.1 Parser API

```rust
// crates/sql/src/parser/mod.rs

/// SQL parser for converting SQL text into an Abstract Syntax Tree (AST)
pub struct Parser {
    lexer: Lexer,
    current_token: Token,
    peek_token: Token,
}

impl Parser {
    /// Create a new parser from SQL text
    pub fn new(sql: &str) -> Self;

    /// Parse a SQL statement
    pub fn parse_statement(&mut self) -> Result<Statement, ParserError>;

    /// Parse a SELECT query
    pub fn parse_query(&mut self) -> Result<Query, ParserError>;

    /// Parse an expression
    pub fn parse_expr(&mut self) -> Result<Expr, ParserError>;

    /// Parse a data type
    pub fn parse_data_type(&mut self) -> Result<DataType, ParserError>;
}

/// SQL tokens
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    Select, From, Where, Group, By, Having, Order, Limit,
    Join, Inner, Left, Right, Full, Outer, On, Using,
    Create, Stream, Table, With, As,
    Tumble, Hop, Session, Interval,

    // Operators
    Plus, Minus, Star, Slash, Percent,
    Eq, Ne, Lt, Le, Gt, Ge,
    And, Or, Not,

    // Literals
    Number(String),
    String(String),
    Identifier(String),

    // Punctuation
    LParen, RParen, Comma, Semicolon, Dot,

    // Special
    Eof,
}

/// Abstract Syntax Tree nodes
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Query(Query),
    CreateStream(CreateStream),
    CreateTable(CreateTable),
    Insert(Insert),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    pub with: Option<Vec<Cte>>,
    pub select: SelectStatement,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectStatement {
    pub projection: Vec<SelectItem>,
    pub from: Vec<TableReference>,
    pub selection: Option<Expr>,
    pub group_by: Vec<Expr>,
    pub having: Option<Expr>,
    pub order_by: Vec<OrderByExpr>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Identifier(String),
    Literal(Literal),
    BinaryOp { left: Box<Expr>, op: BinaryOperator, right: Box<Expr> },
    UnaryOp { op: UnaryOperator, expr: Box<Expr> },
    FunctionCall { name: String, args: Vec<Expr> },
    Aggregate { func: AggregateFunction, args: Vec<Expr> },
    Window { func: WindowFunction, args: Vec<Expr> },
    Cast { expr: Box<Expr>, data_type: DataType },
    Case { conditions: Vec<(Expr, Expr)>, else_result: Option<Box<Expr>> },
    Subquery(Box<Query>),
}
```

---

### 3.2 Planner API

```rust
// crates/sql/src/planner/mod.rs

/// Query planner that converts AST to logical and physical plans
pub struct Planner {
    catalog: Arc<Catalog>,
    optimizer: Optimizer,
}

impl Planner {
    /// Create a new planner
    pub fn new(catalog: Arc<Catalog>) -> Self;

    /// Create a logical plan from a SQL statement
    pub fn create_logical_plan(&self, statement: &Statement)
        -> Result<LogicalPlan, PlannerError>;

    /// Optimize a logical plan
    pub fn optimize(&self, plan: LogicalPlan)
        -> Result<LogicalPlan, PlannerError>;

    /// Create a physical plan from a logical plan
    pub fn create_physical_plan(&self, logical_plan: &LogicalPlan)
        -> Result<PhysicalPlan, PlannerError>;
}

/// Logical plan nodes
#[derive(Debug, Clone)]
pub enum LogicalPlan {
    Scan {
        source: String,
        schema: SchemaRef,
        projection: Option<Vec<usize>>,
        filters: Vec<Expr>,
    },
    Filter {
        input: Box<LogicalPlan>,
        predicate: Expr,
    },
    Project {
        input: Box<LogicalPlan>,
        exprs: Vec<Expr>,
        schema: SchemaRef,
    },
    Aggregate {
        input: Box<LogicalPlan>,
        group_by: Vec<Expr>,
        aggregates: Vec<AggregateExpr>,
        schema: SchemaRef,
    },
    Window {
        input: Box<LogicalPlan>,
        window_type: WindowType,
        window_size: Duration,
        slide: Option<Duration>,
        time_column: String,
    },
    Join {
        left: Box<LogicalPlan>,
        right: Box<LogicalPlan>,
        join_type: JoinType,
        on: Vec<(Expr, Expr)>,
    },
    Sort {
        input: Box<LogicalPlan>,
        order_by: Vec<OrderByExpr>,
    },
    Limit {
        input: Box<LogicalPlan>,
        limit: usize,
    },
}

/// Physical plan nodes (executable)
#[derive(Debug, Clone)]
pub enum PhysicalPlan {
    StreamScan {
        source: Arc<dyn StreamSource>,
        schema: SchemaRef,
    },
    Filter {
        input: Box<PhysicalPlan>,
        predicate: Arc<dyn PhysicalExpr>,
    },
    Project {
        input: Box<PhysicalPlan>,
        exprs: Vec<Arc<dyn PhysicalExpr>>,
    },
    HashAggregate {
        input: Box<PhysicalPlan>,
        group_by: Vec<Arc<dyn PhysicalExpr>>,
        aggregates: Vec<Arc<dyn AggregateExpr>>,
    },
    WindowAggregate {
        input: Box<PhysicalPlan>,
        window_assigner: Arc<dyn WindowAssigner>,
        aggregates: Vec<Arc<dyn AggregateExpr>>,
    },
    HashJoin {
        left: Box<PhysicalPlan>,
        right: Box<PhysicalPlan>,
        join_type: JoinType,
        left_keys: Vec<Arc<dyn PhysicalExpr>>,
        right_keys: Vec<Arc<dyn PhysicalExpr>>,
    },
    Sort {
        input: Box<PhysicalPlan>,
        order_by: Vec<PhysicalOrderByExpr>,
    },
}

/// Optimization rules
pub trait OptimizationRule: Send + Sync {
    fn name(&self) -> &str;
    fn optimize(&self, plan: LogicalPlan) -> Result<LogicalPlan, PlannerError>;
}

pub struct Optimizer {
    rules: Vec<Arc<dyn OptimizationRule>>,
}

impl Optimizer {
    pub fn new() -> Self;
    pub fn add_rule(&mut self, rule: Arc<dyn OptimizationRule>);
    pub fn optimize(&self, plan: LogicalPlan) -> Result<LogicalPlan, PlannerError>;
}
```

---

### 3.3 Executor API

```rust
// crates/sql/src/executor/mod.rs

/// Query executor that runs physical plans
pub struct Executor {
    runtime: Arc<Runtime>,
    state_backend: Arc<dyn StateBackend>,
}

impl Executor {
    /// Create a new executor
    pub fn new(runtime: Arc<Runtime>, state_backend: Arc<dyn StateBackend>) -> Self;

    /// Execute a physical plan
    pub async fn execute(&self, plan: PhysicalPlan)
        -> Result<RecordBatchStream, ExecutorError>;

    /// Execute a plan and collect all results
    pub async fn collect(&self, plan: PhysicalPlan)
        -> Result<Vec<RecordBatch>, ExecutorError>;
}

/// Streaming record batch (Arrow-compatible)
pub trait RecordBatchStream: Stream<Item = Result<RecordBatch, ExecutorError>> + Send {
    fn schema(&self) -> SchemaRef;
}

/// Physical operator trait
#[async_trait]
pub trait PhysicalOperator: Send + Sync {
    /// Execute the operator and produce a stream of record batches
    async fn execute(&self, ctx: &ExecutionContext)
        -> Result<RecordBatchStream, ExecutorError>;

    /// Get the output schema
    fn schema(&self) -> SchemaRef;

    /// Get operator statistics
    fn statistics(&self) -> OperatorStats;
}

/// Execution context
pub struct ExecutionContext {
    pub session_id: String,
    pub state_backend: Arc<dyn StateBackend>,
    pub watermark: Option<Timestamp>,
    pub task_id: usize,
}

/// Physical expression trait
pub trait PhysicalExpr: Send + Sync {
    /// Evaluate the expression on a record batch
    fn evaluate(&self, batch: &RecordBatch) -> Result<ArrayRef, ExecutorError>;

    /// Get the data type of the result
    fn data_type(&self, input_schema: &Schema) -> Result<DataType, ExecutorError>;
}

/// Aggregate expression trait
pub trait AggregateExpr: Send + Sync {
    /// Create initial state
    fn create_state(&self) -> Box<dyn AggregateState>;

    /// Get the result data type
    fn data_type(&self) -> DataType;

    /// Get the name
    fn name(&self) -> String;
}

/// Aggregate state (for incremental computation)
pub trait AggregateState: Send {
    /// Update state with new values
    fn update(&mut self, values: &ArrayRef) -> Result<(), ExecutorError>;

    /// Merge with another state
    fn merge(&mut self, other: &dyn AggregateState) -> Result<(), ExecutorError>;

    /// Get the final result
    fn finalize(&self) -> Result<ScalarValue, ExecutorError>;
}
```

---

### 3.4 Catalog API

```rust
// crates/sql/src/catalog/mod.rs

/// Metadata catalog for streams, tables, and functions
pub struct Catalog {
    streams: RwLock<HashMap<String, StreamMetadata>>,
    tables: RwLock<HashMap<String, TableMetadata>>,
    functions: RwLock<HashMap<String, FunctionMetadata>>,
    persistence: Option<Arc<dyn CatalogStorage>>,
}

impl Catalog {
    /// Create a new catalog
    pub fn new() -> Self;

    /// Register a stream
    pub async fn register_stream(&self, name: String, metadata: StreamMetadata)
        -> Result<(), CatalogError>;

    /// Get stream metadata
    pub async fn get_stream(&self, name: &str)
        -> Result<StreamMetadata, CatalogError>;

    /// Register a table
    pub async fn register_table(&self, name: String, metadata: TableMetadata)
        -> Result<(), CatalogError>;

    /// Get table metadata
    pub async fn get_table(&self, name: &str)
        -> Result<TableMetadata, CatalogError>;

    /// Register a function
    pub fn register_function(&self, name: String, func: Arc<dyn ScalarFunction>);

    /// Get a function
    pub fn get_function(&self, name: &str) -> Option<Arc<dyn ScalarFunction>>;
}

/// Stream metadata
#[derive(Debug, Clone)]
pub struct StreamMetadata {
    pub name: String,
    pub schema: SchemaRef,
    pub connector: ConnectorConfig,
    pub watermark_strategy: Option<WatermarkStrategy>,
    pub properties: HashMap<String, String>,
}

/// Table metadata
#[derive(Debug, Clone)]
pub struct TableMetadata {
    pub name: String,
    pub schema: SchemaRef,
    pub connector: ConnectorConfig,
    pub primary_key: Option<Vec<String>>,
    pub properties: HashMap<String, String>,
}

/// Connector configuration
#[derive(Debug, Clone)]
pub enum ConnectorConfig {
    Kafka {
        topic: String,
        brokers: String,
        group_id: Option<String>,
        format: DataFormat,
    },
    Postgres {
        url: String,
        table: String,
    },
    File {
        path: String,
        format: DataFormat,
    },
    Memory {
        data: Vec<RecordBatch>,
    },
}

/// Data format
#[derive(Debug, Clone)]
pub enum DataFormat {
    Json,
    Avro,
    Parquet,
    Csv,
}
```

---

### 3.5 Function Registry API

```rust
// crates/sql/src/functions/mod.rs

/// Function registry for built-in and user-defined functions
pub struct FunctionRegistry {
    scalar_functions: HashMap<String, Arc<dyn ScalarFunction>>,
    aggregate_functions: HashMap<String, Arc<dyn AggregateFunction>>,
    window_functions: HashMap<String, Arc<dyn WindowFunction>>,
}

impl FunctionRegistry {
    /// Create a registry with built-in functions
    pub fn new() -> Self;

    /// Register a scalar function
    pub fn register_scalar(&mut self, name: String, func: Arc<dyn ScalarFunction>);

    /// Register an aggregate function
    pub fn register_aggregate(&mut self, name: String, func: Arc<dyn AggregateFunction>);

    /// Get a scalar function
    pub fn get_scalar(&self, name: &str) -> Option<Arc<dyn ScalarFunction>>;

    /// Get an aggregate function
    pub fn get_aggregate(&self, name: &str) -> Option<Arc<dyn AggregateFunction>>;
}

/// Scalar function trait
pub trait ScalarFunction: Send + Sync {
    fn name(&self) -> &str;
    fn signature(&self) -> &Signature;
    fn return_type(&self, arg_types: &[DataType]) -> Result<DataType, FunctionError>;
    fn invoke(&self, args: &[ArrayRef]) -> Result<ArrayRef, FunctionError>;
}

/// Aggregate function trait
pub trait AggregateFunction: Send + Sync {
    fn name(&self) -> &str;
    fn signature(&self) -> &Signature;
    fn return_type(&self, arg_types: &[DataType]) -> Result<DataType, FunctionError>;
    fn create_accumulator(&self) -> Box<dyn Accumulator>;
}

/// Accumulator for aggregate functions
pub trait Accumulator: Send {
    fn update(&mut self, values: &[ScalarValue]) -> Result<(), FunctionError>;
    fn merge(&mut self, other: &dyn Accumulator) -> Result<(), FunctionError>;
    fn evaluate(&self) -> Result<ScalarValue, FunctionError>;
}

/// Built-in scalar functions
pub fn create_builtin_scalar_functions() -> Vec<Arc<dyn ScalarFunction>> {
    vec![
        // Math functions
        Arc::new(AbsFunction),
        Arc::new(CeilFunction),
        Arc::new(FloorFunction),
        Arc::new(RoundFunction),
        Arc::new(SqrtFunction),

        // String functions
        Arc::new(LowerFunction),
        Arc::new(UpperFunction),
        Arc::new(SubstringFunction),
        Arc::new(ConcatFunction),
        Arc::new(LengthFunction),

        // Date/time functions
        Arc::new(CurrentTimestampFunction),
        Arc::new(DateTruncFunction),
        Arc::new(ExtractFunction),
    ]
}

/// Built-in aggregate functions
pub fn create_builtin_aggregate_functions() -> Vec<Arc<dyn AggregateFunction>> {
    vec![
        Arc::new(CountFunction),
        Arc::new(SumFunction),
        Arc::new(AvgFunction),
        Arc::new(MinFunction),
        Arc::new(MaxFunction),
        Arc::new(StdDevFunction),
        Arc::new(VarianceFunction),
        Arc::new(PercentileContFunction),
    ]
}
```

---

## 4. Implementation Schedule

### Timeline Overview

| Phase | Duration | Effort | Dependencies | Risk Level |
|-------|----------|--------|--------------|------------|
| Phase 1: Basic SELECT | 2 weeks | 1,200 LOC | None | Low |
| Phase 2: Aggregations | 2 weeks | 1,800 LOC | Phase 1 | Low |
| Phase 3: Windows | 2 weeks | 1,500 LOC | Phase 2, existing windowing | Medium |
| Phase 4: Joins | 2 weeks | 1,400 LOC | Phase 1-3 | Medium |
| Phase 5: DDL | 1 week | 1,200 LOC | Phase 1-4 | Low |
| Phase 6: Advanced | 3 weeks | 1,900 LOC | Phase 1-5 | High |
| **Total** | **12 weeks** | **9,900 LOC** | | |

### Detailed Schedule

#### Week 1-2: Phase 1 (Basic SELECT)

**Week 1**:
- Day 1-2: Lexer and tokenizer (250 LOC)
- Day 3-4: Expression parser (300 LOC)
- Day 5: SELECT/FROM/WHERE parser (200 LOC)

**Week 2**:
- Day 1-2: Logical plan generation (250 LOC)
- Day 3-4: Scan, Filter, Project operators (400 LOC)
- Day 5: Integration tests (100 tests)

**Milestone**: Execute `SELECT col1, col2 FROM stream WHERE col1 > 10`

---

#### Week 3-4: Phase 2 (Aggregations)

**Week 3**:
- Day 1-2: Aggregate function parsing (200 LOC)
- Day 3-4: GROUP BY/HAVING parsing (200 LOC)
- Day 5: Built-in aggregate functions (400 LOC)

**Week 4**:
- Day 1-2: Hash aggregate operator (500 LOC)
- Day 3-4: Aggregate accumulator framework (300 LOC)
- Day 5: Integration tests (80 tests)

**Milestone**: Execute `SELECT model_id, COUNT(*), AVG(latency_ms) FROM requests GROUP BY model_id`

---

#### Week 5-6: Phase 3 (Window Functions)

**Week 5**:
- Day 1-2: Window clause parsing (TUMBLE, HOP, SESSION) (300 LOC)
- Day 3-4: Window function implementations (400 LOC)
- Day 5: Time/interval handling (200 LOC)

**Week 6**:
- Day 1-3: Window operator integration with existing windowing (500 LOC)
- Day 4-5: Watermark handling and late data (300 LOC)

**Milestone**: Execute tumbling window aggregations with watermarks

---

#### Week 7-8: Phase 4 (Joins)

**Week 7**:
- Day 1-2: JOIN clause parsing (200 LOC)
- Day 3-5: Join planning and optimization (400 LOC)

**Week 8**:
- Day 1-3: Hash join operator (500 LOC)
- Day 4-5: Stream-stream join with time bounds (500 LOC)

**Milestone**: Execute stream-to-stream joins with time constraints

---

#### Week 9: Phase 5 (DDL)

**Week 9**:
- Day 1-2: DDL parsing (CREATE STREAM/TABLE) (300 LOC)
- Day 3-4: Catalog implementation (600 LOC)
- Day 5: Connector framework (300 LOC)

**Milestone**: Define and query custom streams via DDL

---

#### Week 10-12: Phase 6 (Advanced Features)

**Week 10**:
- Day 1-2: Subquery parsing and planning (400 LOC)
- Day 3-5: CTE (WITH clause) support (300 LOC)

**Week 11**:
- Day 1-3: Query optimization rules (600 LOC)
- Day 4-5: Cost-based optimization (200 LOC)

**Week 12**:
- Day 1-2: Window aggregate functions (OVER clause) (300 LOC)
- Day 3-5: Performance tuning and documentation (100 LOC)

**Milestone**: Full SQL support with optimizations

---

### Parallel Work Streams

To accelerate development, these tasks can be parallelized:

1. **Parser Development** (1 developer)
   - Focus on lexer, parser, AST

2. **Planner Development** (1 developer)
   - Focus on logical/physical planning, optimization

3. **Executor Development** (1 developer)
   - Focus on operators, runtime

4. **Testing & Documentation** (0.5 developer)
   - Write tests, examples, docs in parallel

**With 3 developers**: Estimated 6-8 weeks instead of 12 weeks

---

## 5. Testing Strategy

### 5.1 Unit Testing

**Parser Tests** (200+ tests):
```rust
// crates/sql/tests/parser_tests.rs

#[test]
fn test_parse_simple_select() {
    let sql = "SELECT id, name FROM users WHERE age > 18";
    let mut parser = Parser::new(sql);
    let stmt = parser.parse_statement().unwrap();

    match stmt {
        Statement::Query(query) => {
            assert_eq!(query.select.projection.len(), 2);
            assert!(query.select.selection.is_some());
        }
        _ => panic!("Expected query statement"),
    }
}

#[test]
fn test_parse_aggregate() {
    let sql = "SELECT model_id, COUNT(*), AVG(latency) FROM requests GROUP BY model_id";
    let mut parser = Parser::new(sql);
    let stmt = parser.parse_statement().unwrap();
    // Assertions...
}

#[test]
fn test_parse_window_tumble() {
    let sql = r#"
        SELECT
            TUMBLE_START(event_time, INTERVAL '5' MINUTE) as window_start,
            COUNT(*) as count
        FROM events
        GROUP BY TUMBLE(event_time, INTERVAL '5' MINUTE)
    "#;
    let mut parser = Parser::new(sql);
    let stmt = parser.parse_statement().unwrap();
    // Assertions...
}
```

**Planner Tests** (150+ tests):
```rust
// crates/sql/tests/planner_tests.rs

#[test]
fn test_logical_plan_scan() {
    let catalog = create_test_catalog();
    let planner = Planner::new(Arc::new(catalog));

    let sql = "SELECT * FROM users";
    let mut parser = Parser::new(sql);
    let stmt = parser.parse_statement().unwrap();

    let logical_plan = planner.create_logical_plan(&stmt).unwrap();

    match logical_plan {
        LogicalPlan::Scan { source, .. } => {
            assert_eq!(source, "users");
        }
        _ => panic!("Expected scan plan"),
    }
}

#[test]
fn test_predicate_pushdown() {
    let catalog = create_test_catalog();
    let planner = Planner::new(Arc::new(catalog));

    let sql = "SELECT id FROM users WHERE age > 18";
    let logical_plan = planner.create_logical_plan_from_sql(sql).unwrap();
    let optimized = planner.optimize(logical_plan).unwrap();

    // Verify filter is pushed down to scan
    match optimized {
        LogicalPlan::Scan { filters, .. } => {
            assert!(!filters.is_empty());
        }
        _ => panic!("Expected optimized scan with filters"),
    }
}
```

**Executor Tests** (200+ tests):
```rust
// crates/sql/tests/executor_tests.rs

#[tokio::test]
async fn test_execute_simple_select() {
    let executor = create_test_executor().await;

    let sql = "SELECT id, name FROM users WHERE id < 100";
    let plan = create_physical_plan(sql).await.unwrap();

    let results = executor.collect(plan).await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].num_rows(), 99);
}

#[tokio::test]
async fn test_execute_aggregation() {
    let executor = create_test_executor().await;

    let sql = "SELECT model_id, COUNT(*), AVG(latency_ms) FROM requests GROUP BY model_id";
    let results = execute_sql(&executor, sql).await.unwrap();

    assert!(results.len() > 0);
    // Verify aggregation results
}

#[tokio::test]
async fn test_execute_window_tumble() {
    let executor = create_test_executor().await;

    let sql = r#"
        SELECT
            TUMBLE_START(event_time, INTERVAL '1' MINUTE) as window_start,
            COUNT(*) as count
        FROM events
        GROUP BY TUMBLE(event_time, INTERVAL '1' MINUTE)
    "#;

    let results = execute_sql(&executor, sql).await.unwrap();

    // Verify windowing works correctly
    assert!(results.len() > 0);
}
```

---

### 5.2 Integration Testing

**End-to-End Query Tests** (100+ tests):
```rust
// crates/sql/tests/integration/end_to_end.rs

#[tokio::test]
async fn test_e2e_kafka_to_aggregation() {
    // Setup: Create Kafka topic, populate with test data
    let kafka_broker = start_test_kafka_broker().await;
    populate_test_data(&kafka_broker, 10000).await;

    // Execute SQL
    let session = SqlSession::new().await;
    session.execute_ddl(r#"
        CREATE STREAM requests (
            request_id VARCHAR,
            model_id VARCHAR,
            event_time TIMESTAMP,
            latency_ms BIGINT,
            WATERMARK FOR event_time AS event_time - INTERVAL '10' SECOND
        ) WITH (
            'connector' = 'kafka',
            'topic' = 'test.requests',
            'properties.bootstrap.servers' = 'localhost:9092'
        )
    "#).await.unwrap();

    let results = session.execute_query(r#"
        SELECT
            model_id,
            TUMBLE_START(event_time, INTERVAL '1' MINUTE) as window_start,
            COUNT(*) as count,
            AVG(latency_ms) as avg_latency
        FROM requests
        GROUP BY model_id, TUMBLE(event_time, INTERVAL '1' MINUTE)
        ORDER BY window_start
    "#).await.unwrap();

    // Verify results
    assert!(results.len() > 0);
    validate_aggregation_correctness(&results);
}

#[tokio::test]
async fn test_e2e_stream_join() {
    let session = SqlSession::new().await;

    // Register two streams
    session.execute_ddl(CREATE_REQUESTS_STREAM).await.unwrap();
    session.execute_ddl(CREATE_FEEDBACK_STREAM).await.unwrap();

    // Join query
    let results = session.execute_query(r#"
        SELECT
            r.request_id,
            r.model_id,
            r.latency_ms,
            f.rating,
            f.feedback_text
        FROM requests r
        INNER JOIN feedback f
        ON r.request_id = f.request_id
        WHERE f.rating < 3
    "#).await.unwrap();

    // Verify join correctness
    validate_join_results(&results);
}
```

---

### 5.3 Compatibility Testing

**PostgreSQL Compatibility** (50+ tests):
```rust
// crates/sql/tests/compatibility/postgres_compat.rs

// Test that our SQL dialect is compatible with PostgreSQL syntax
#[tokio::test]
async fn test_postgres_aggregate_functions() {
    let queries = vec![
        "SELECT COUNT(*) FROM users",
        "SELECT AVG(age) FROM users",
        "SELECT STDDEV(salary) FROM employees",
        "SELECT PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY latency) FROM requests",
    ];

    for sql in queries {
        assert!(parse_and_validate(sql).is_ok());
    }
}
```

**Apache Flink Compatibility** (50+ tests):
```rust
// crates/sql/tests/compatibility/flink_compat.rs

// Test that our window functions match Flink SQL semantics
#[tokio::test]
async fn test_flink_tumbling_window() {
    let sql = r#"
        SELECT
            TUMBLE_START(rowtime, INTERVAL '1' MINUTE) as wstart,
            COUNT(*) as cnt
        FROM MyTable
        GROUP BY TUMBLE(rowtime, INTERVAL '1' MINUTE)
    "#;

    assert!(parse_and_validate(sql).is_ok());
}
```

---

### 5.4 Performance Testing

**Benchmarks** (10+ benchmarks):
```rust
// crates/sql/benches/query_benchmarks.rs

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_parser(c: &mut Criterion) {
    let sql = "SELECT model_id, COUNT(*), AVG(latency) FROM requests WHERE latency > 100 GROUP BY model_id";

    c.bench_function("parse_complex_query", |b| {
        b.iter(|| {
            let mut parser = Parser::new(black_box(sql));
            parser.parse_statement().unwrap()
        })
    });
}

fn benchmark_executor(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let executor = runtime.block_on(create_test_executor());

    c.bench_function("execute_aggregation_10k_rows", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let sql = "SELECT model_id, COUNT(*) FROM requests GROUP BY model_id";
                execute_sql(&executor, sql).await.unwrap()
            })
        })
    });
}

criterion_group!(benches, benchmark_parser, benchmark_executor);
criterion_main!(benches);
```

**Performance Targets**:
- Parsing: < 1ms for typical queries (< 500 chars)
- Planning: < 10ms for complex queries
- Execution: > 10,000 events/sec throughput
- Memory: < 100MB overhead for executor runtime
- Latency: < 100ms p99 for simple aggregations

---

### 5.5 Testing Checklist

- [ ] **Parser**
  - [ ] 50+ SELECT statement tests
  - [ ] 30+ expression tests (arithmetic, logical, comparison)
  - [ ] 20+ aggregate parsing tests
  - [ ] 30+ window function tests
  - [ ] 20+ JOIN parsing tests
  - [ ] 15+ DDL parsing tests
  - [ ] 20+ subquery/CTE tests
  - [ ] Error handling (invalid SQL)

- [ ] **Planner**
  - [ ] 40+ logical plan generation tests
  - [ ] 30+ optimization rule tests
  - [ ] 20+ physical plan generation tests
  - [ ] Cost model validation
  - [ ] Schema inference tests

- [ ] **Executor**
  - [ ] 50+ operator correctness tests
  - [ ] 30+ streaming window tests
  - [ ] 25+ join correctness tests
  - [ ] Watermark propagation tests
  - [ ] Late data handling tests
  - [ ] State management tests

- [ ] **Integration**
  - [ ] 20+ end-to-end Kafka integration tests
  - [ ] 15+ PostgreSQL connector tests
  - [ ] 10+ multi-source join tests
  - [ ] Performance regression tests

- [ ] **Compatibility**
  - [ ] 50+ PostgreSQL syntax compatibility tests
  - [ ] 50+ Apache Flink SQL compatibility tests

---

## 6. Dependencies and Integration

### 6.1 Rust Dependencies

Add to `crates/sql/Cargo.toml`:

```toml
[package]
name = "sql"
version = "0.1.0"
edition = "2021"

[dependencies]
# Parser
nom = "7.1"                           # Parser combinator library
logos = "0.13"                        # Fast lexer generator

# Data structures
arrow = "51.0"                        # Apache Arrow (columnar data)
datafusion-common = "37.0"            # Common utilities (optional, for reference)
sqlparser = "0.43"                    # SQL parser (for reference/fallback)

# Async runtime
tokio = { version = "1.36", features = ["full"] }
async-trait = "0.1"
futures = "0.3"

# Error handling
thiserror = "1.0"
anyhow = "1.0"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Logging
tracing = "0.1"
tracing-subscriber = "0.3"

# Time handling
chrono = { version = "0.4", features = ["serde"] }

# Collections
hashbrown = "0.14"                    # Fast HashMap
ahash = "0.8"                         # Fast hash function

# Internal dependencies
processor = { path = "../processor" }  # For window integration
types = { path = "../types" }          # For event types
storage = { path = "../storage" }      # For state backend

[dev-dependencies]
criterion = "0.5"                      # Benchmarking
proptest = "1.4"                       # Property-based testing
test-log = "0.2"                       # Test logging

[build-dependencies]
# Optional: Generate parser from grammar
lalrpop = "0.20"                       # LR(1) parser generator (if needed)
```

---

### 6.2 Integration with Existing Modules

#### Integration with `processor` Crate

```rust
// crates/sql/src/executor/operators/window.rs

use processor::{
    WindowAssigner, TumblingWindowAssigner, SlidingWindowAssigner, SessionWindowAssigner,
    WindowManager, WindowTrigger, OnWatermarkTrigger,
};

/// SQL Window operator that wraps existing processor windowing
pub struct WindowOperator {
    input: Box<dyn PhysicalOperator>,
    window_assigner: Arc<dyn WindowAssigner<Event = ProcessorEvent>>,
    window_manager: WindowManager,
    aggregates: Vec<Arc<dyn AggregateExpr>>,
}

impl WindowOperator {
    pub fn new(
        input: Box<dyn PhysicalOperator>,
        window_type: WindowType,
        window_size: Duration,
        slide: Option<Duration>,
    ) -> Self {
        let window_assigner: Arc<dyn WindowAssigner<Event = ProcessorEvent>> = match window_type {
            WindowType::Tumbling => {
                Arc::new(TumblingWindowAssigner::new(window_size))
            }
            WindowType::Sliding => {
                let slide = slide.unwrap_or(window_size);
                Arc::new(SlidingWindowAssigner::new(window_size, slide))
            }
            WindowType::Session => {
                Arc::new(SessionWindowAssigner::new(window_size))
            }
        };

        let window_manager = WindowManager::new(
            window_assigner.clone(),
            Box::new(OnWatermarkTrigger),
        );

        Self {
            input,
            window_assigner,
            window_manager,
            aggregates: vec![],
        }
    }
}

#[async_trait]
impl PhysicalOperator for WindowOperator {
    async fn execute(&self, ctx: &ExecutionContext) -> Result<RecordBatchStream> {
        // Leverage existing windowing infrastructure
        // ...
    }
}
```

#### Integration with `storage` Crate

```rust
// crates/sql/src/executor/state.rs

use storage::{StateBackend, RedisBackend, PostgresBackend};

/// State management for SQL operators (joins, aggregates)
pub struct SqlStateManager {
    backend: Arc<dyn StateBackend>,
    namespace: String,
}

impl SqlStateManager {
    pub async fn new(backend: Arc<dyn StateBackend>, namespace: String) -> Self {
        Self { backend, namespace }
    }

    /// Store join state
    pub async fn put_join_state(
        &self,
        key: &[u8],
        value: &RecordBatch,
    ) -> Result<()> {
        let serialized = serialize_record_batch(value)?;
        self.backend.put(
            &format!("{}:join", self.namespace),
            key,
            &serialized,
        ).await?;
        Ok(())
    }

    /// Get join state
    pub async fn get_join_state(&self, key: &[u8]) -> Result<Option<RecordBatch>> {
        if let Some(data) = self.backend.get(
            &format!("{}:join", self.namespace),
            key,
        ).await? {
            let batch = deserialize_record_batch(&data)?;
            Ok(Some(batch))
        } else {
            Ok(None)
        }
    }
}
```

#### Integration with `types` Crate

```rust
// crates/sql/src/catalog/stream.rs

use types::events::{FeedbackEvent, PerformanceMetricsEvent};

/// Map SQL streams to internal event types
pub fn create_stream_from_event_type(event_type: &str) -> Result<StreamMetadata> {
    match event_type {
        "FeedbackEvent" => {
            Ok(StreamMetadata {
                name: "feedback_events".to_string(),
                schema: Arc::new(Schema::new(vec![
                    Field::new("event_id", DataType::Utf8, false),
                    Field::new("timestamp", DataType::Timestamp(TimeUnit::Millisecond, None), false),
                    Field::new("model_id", DataType::Utf8, false),
                    Field::new("rating", DataType::Int32, true),
                    // ... more fields from FeedbackEvent
                ])),
                connector: ConnectorConfig::Kafka { /* ... */ },
                watermark_strategy: Some(WatermarkStrategy::BoundedOutOfOrderness {
                    max_delay: Duration::from_secs(30),
                }),
                properties: HashMap::new(),
            })
        }
        "PerformanceMetricsEvent" => {
            // Similar mapping for PerformanceMetricsEvent
            // ...
        }
        _ => Err(CatalogError::UnknownEventType(event_type.to_string())),
    }
}
```

---

### 6.3 API Exposure

```rust
// crates/sql/src/api/mod.rs

/// High-level SQL API for executing queries
pub struct SqlSession {
    catalog: Arc<Catalog>,
    planner: Planner,
    executor: Executor,
}

impl SqlSession {
    /// Create a new SQL session
    pub async fn new() -> Result<Self> {
        let catalog = Arc::new(Catalog::new());
        let planner = Planner::new(catalog.clone());
        let state_backend = Arc::new(RedisBackend::new(/* config */));
        let runtime = Arc::new(Runtime::new()?);
        let executor = Executor::new(runtime, state_backend);

        Ok(Self { catalog, planner, executor })
    }

    /// Execute DDL (CREATE STREAM, CREATE TABLE)
    pub async fn execute_ddl(&self, sql: &str) -> Result<()> {
        let mut parser = Parser::new(sql);
        let statement = parser.parse_statement()?;

        match statement {
            Statement::CreateStream(create_stream) => {
                let metadata = StreamMetadata::from_create_stream(&create_stream)?;
                self.catalog.register_stream(metadata.name.clone(), metadata).await?;
                Ok(())
            }
            Statement::CreateTable(create_table) => {
                let metadata = TableMetadata::from_create_table(&create_table)?;
                self.catalog.register_table(metadata.name.clone(), metadata).await?;
                Ok(())
            }
            _ => Err(SqlError::InvalidDDL),
        }
    }

    /// Execute a query and return results
    pub async fn execute_query(&self, sql: &str) -> Result<Vec<RecordBatch>> {
        let mut parser = Parser::new(sql);
        let statement = parser.parse_statement()?;

        let logical_plan = self.planner.create_logical_plan(&statement)?;
        let optimized_plan = self.planner.optimize(logical_plan)?;
        let physical_plan = self.planner.create_physical_plan(&optimized_plan)?;

        let results = self.executor.collect(physical_plan).await?;
        Ok(results)
    }

    /// Execute a query and return a stream (for large results)
    pub async fn execute_query_stream(&self, sql: &str) -> Result<RecordBatchStream> {
        let mut parser = Parser::new(sql);
        let statement = parser.parse_statement()?;

        let logical_plan = self.planner.create_logical_plan(&statement)?;
        let optimized_plan = self.planner.optimize(logical_plan)?;
        let physical_plan = self.planner.create_physical_plan(&optimized_plan)?;

        let stream = self.executor.execute(physical_plan).await?;
        Ok(stream)
    }
}
```

---

## 7. Success Criteria

### Phase 1 Success Criteria

- [ ] Parser can handle basic SELECT, FROM, WHERE
- [ ] Support for 10+ comparison operators (=, !=, <, <=, >, >=, LIKE, IN, IS NULL, etc.)
- [ ] Support for logical operators (AND, OR, NOT)
- [ ] Support for arithmetic expressions (+, -, *, /, %)
- [ ] Scan operator reads from memory stream
- [ ] Filter operator correctly filters rows
- [ ] Project operator selects columns
- [ ] 100+ passing unit tests
- [ ] Documentation with 5+ examples

### Phase 2 Success Criteria

- [ ] Support COUNT, SUM, AVG, MIN, MAX aggregates
- [ ] Support STDDEV, VARIANCE statistical functions
- [ ] Support PERCENTILE_CONT for percentiles
- [ ] GROUP BY with multiple columns
- [ ] HAVING clause works correctly
- [ ] Hash aggregate operator with <10% overhead vs. handwritten code
- [ ] 150+ passing unit tests
- [ ] Benchmark: >5,000 aggregations/sec

### Phase 3 Success Criteria

- [ ] TUMBLE windows work correctly
- [ ] HOP (sliding) windows work correctly
- [ ] SESSION windows work correctly
- [ ] Watermark propagation works
- [ ] Late data handling (allowed lateness)
- [ ] Window state cleanup (no memory leaks)
- [ ] Integration with existing `processor::window` module
- [ ] 200+ passing unit tests
- [ ] Benchmark: >10,000 events/sec with windowing

### Phase 4 Success Criteria

- [ ] INNER JOIN works correctly
- [ ] LEFT OUTER JOIN works correctly
- [ ] Join with multiple keys
- [ ] Time-bounded joins for streams
- [ ] Join state eviction (TTL)
- [ ] Join correctness validated against PostgreSQL
- [ ] 180+ passing unit tests
- [ ] Benchmark: >2,000 joins/sec

### Phase 5 Success Criteria

- [ ] CREATE STREAM syntax works
- [ ] CREATE TABLE syntax works
- [ ] Kafka connector integration
- [ ] PostgreSQL connector integration
- [ ] Catalog persistence (survive restarts)
- [ ] Schema inference from connectors
- [ ] 150+ passing unit tests

### Phase 6 Success Criteria

- [ ] Scalar subqueries work
- [ ] Correlated subqueries work
- [ ] CTEs (WITH clause) work
- [ ] FULL OUTER JOIN works
- [ ] Window aggregate functions (OVER clause)
- [ ] Predicate pushdown optimization
- [ ] Projection pushdown optimization
- [ ] Join reordering optimization
- [ ] 250+ passing unit tests
- [ ] Performance: <20% overhead vs. handwritten stream processing code
- [ ] Complete documentation with 20+ examples

### Overall Success Criteria

- [ ] **Functionality**: All SQL features implemented per spec
- [ ] **Performance**: <100ms p99 latency for simple queries, >10,000 events/sec throughput
- [ ] **Correctness**: 100% pass rate on 800+ unit/integration tests
- [ ] **Compatibility**: 90%+ compatibility with PostgreSQL syntax
- [ ] **Documentation**: Complete API docs, 10+ runnable examples
- [ ] **Integration**: Seamless integration with existing processor, storage, types crates
- [ ] **Usability**: Data analysts can write queries without Rust knowledge

---

## 8. Reference Implementation Examples

### Example 1: Real-time Latency Monitoring

```rust
// examples/latency_monitoring.rs

use sql::{SqlSession, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let session = SqlSession::new().await?;

    // Define the stream
    session.execute_ddl(r#"
        CREATE STREAM llm_requests (
            request_id VARCHAR NOT NULL,
            model_id VARCHAR NOT NULL,
            event_time TIMESTAMP NOT NULL,
            latency_ms BIGINT NOT NULL,
            cost_usd DOUBLE,
            token_count INTEGER,
            WATERMARK FOR event_time AS event_time - INTERVAL '30' SECOND
        ) WITH (
            'connector' = 'kafka',
            'topic' = 'llm.requests',
            'properties.bootstrap.servers' = 'localhost:9092',
            'format' = 'json'
        )
    "#).await?;

    // Query: Monitor p95 latency per model in 5-minute windows
    let results = session.execute_query(r#"
        SELECT
            model_id,
            TUMBLE_START(event_time, INTERVAL '5' MINUTE) as window_start,
            COUNT(*) as request_count,
            AVG(latency_ms) as avg_latency_ms,
            PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY latency_ms) as p95_latency_ms,
            MAX(latency_ms) as max_latency_ms,
            SUM(cost_usd) as total_cost_usd
        FROM llm_requests
        GROUP BY model_id, TUMBLE(event_time, INTERVAL '5' MINUTE)
        HAVING AVG(latency_ms) > 1000
        ORDER BY window_start DESC, avg_latency_ms DESC
    "#).await?;

    // Process results
    for batch in results {
        println!("Window: {} rows", batch.num_rows());
        println!("{}", batch);
    }

    Ok(())
}
```

---

### Example 2: User Feedback Analysis with Joins

```rust
// examples/feedback_analysis.rs

use sql::{SqlSession, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let session = SqlSession::new().await?;

    // Define streams
    session.execute_ddl(CREATE_REQUESTS_STREAM).await?;
    session.execute_ddl(CREATE_FEEDBACK_STREAM).await?;

    // Query: Analyze low-rated requests
    let results = session.execute_query(r#"
        SELECT
            r.request_id,
            r.model_id,
            r.latency_ms,
            r.cost_usd,
            f.rating,
            f.feedback_text,
            f.created_at
        FROM llm_requests r
        INNER JOIN user_feedback f
        ON r.request_id = f.request_id
        WHERE f.rating <= 2
          AND r.event_time BETWEEN f.created_at - INTERVAL '1' MINUTE
                                AND f.created_at + INTERVAL '1' MINUTE
        ORDER BY f.created_at DESC
        LIMIT 100
    "#).await?;

    // Export to alerts
    for batch in results {
        export_to_alerting_system(&batch).await?;
    }

    Ok(())
}
```

---

### Example 3: Cost Optimization Analysis

```rust
// examples/cost_optimization.rs

use sql::{SqlSession, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let session = SqlSession::new().await?;

    session.execute_ddl(CREATE_REQUESTS_STREAM).await?;

    // Query: Identify expensive models with low utilization
    let results = session.execute_query(r#"
        WITH model_stats AS (
            SELECT
                model_id,
                COUNT(*) as request_count,
                SUM(cost_usd) as total_cost,
                AVG(cost_usd) as avg_cost_per_request,
                AVG(latency_ms) as avg_latency
            FROM llm_requests
            WHERE event_time > CURRENT_TIMESTAMP - INTERVAL '24' HOUR
            GROUP BY model_id
        ),
        low_utilization AS (
            SELECT model_id
            FROM model_stats
            WHERE request_count < 100  -- Low traffic
        )
        SELECT
            s.model_id,
            s.request_count,
            s.total_cost,
            s.avg_cost_per_request,
            s.avg_latency,
            s.total_cost / NULLIF(s.request_count, 0) as cost_per_request
        FROM model_stats s
        INNER JOIN low_utilization lu ON s.model_id = lu.model_id
        WHERE s.total_cost > 10.0  -- High cost
        ORDER BY s.total_cost DESC
    "#).await?;

    // Output optimization recommendations
    for batch in results {
        generate_cost_recommendations(&batch).await?;
    }

    Ok(())
}
```

---

### Example 4: Session-based User Analytics

```rust
// examples/session_analytics.rs

use sql::{SqlSession, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let session = SqlSession::new().await?;

    session.execute_ddl(CREATE_USER_EVENTS_STREAM).await?;

    // Query: Analyze user sessions with 30-minute inactivity gap
    let results = session.execute_query(r#"
        SELECT
            user_id,
            SESSION_START(event_time, INTERVAL '30' MINUTE) as session_start,
            SESSION_END(event_time, INTERVAL '30' MINUTE) as session_end,
            COUNT(*) as interaction_count,
            COUNT(DISTINCT request_id) as unique_requests,
            AVG(CASE WHEN feedback_rating IS NOT NULL THEN feedback_rating END) as avg_rating,
            SUM(CASE WHEN feedback_rating <= 2 THEN 1 ELSE 0 END) as negative_feedback_count
        FROM user_events
        WHERE event_time > CURRENT_TIMESTAMP - INTERVAL '7' DAY
        GROUP BY user_id, SESSION(event_time, INTERVAL '30' MINUTE)
        HAVING COUNT(*) >= 5  -- At least 5 interactions per session
        ORDER BY session_start DESC
    "#).await?;

    // Analyze user engagement patterns
    for batch in results {
        analyze_user_engagement(&batch).await?;
    }

    Ok(())
}
```

---

## 9. Documentation Requirements

### 9.1 User Documentation

1. **Quick Start Guide** (`docs/sql/QUICKSTART.md`)
   - Installation
   - Basic SELECT examples
   - Common patterns

2. **SQL Reference** (`docs/sql/SQL_REFERENCE.md`)
   - Supported SQL syntax
   - Data types
   - Functions (scalar, aggregate, window)
   - Operators

3. **Streaming SQL Guide** (`docs/sql/STREAMING_SQL.md`)
   - Window functions (TUMBLE, HOP, SESSION)
   - Watermarks and late data
   - Time semantics

4. **DDL Reference** (`docs/sql/DDL_REFERENCE.md`)
   - CREATE STREAM
   - CREATE TABLE
   - Connector configuration

5. **Connector Guide** (`docs/sql/CONNECTORS.md`)
   - Kafka connector
   - PostgreSQL connector
   - File connector

### 9.2 Developer Documentation

1. **Architecture Overview** (`docs/sql/ARCHITECTURE.md`)
   - Parser architecture
   - Planner architecture
   - Executor architecture

2. **Query Optimization** (`docs/sql/OPTIMIZATION.md`)
   - Optimization rules
   - Cost model
   - Adding new optimizations

3. **Extending SQL** (`docs/sql/EXTENDING.md`)
   - Adding new functions
   - Adding new operators
   - Custom connectors

4. **Testing Guide** (`docs/sql/TESTING.md`)
   - Running tests
   - Writing new tests
   - Benchmarking

### 9.3 API Documentation

Generate comprehensive API docs:
```bash
cargo doc --no-deps --package sql --open
```

Ensure 100% public API coverage with doc comments.

---

## 10. Risk Mitigation

### High-Risk Areas

1. **Query Optimization Complexity**
   - **Risk**: Complex optimization rules may have bugs
   - **Mitigation**:
     - Extensive unit tests for each rule
     - Property-based testing
     - Disable optimizations by default, enable opt-in

2. **Streaming Join Performance**
   - **Risk**: Join state may grow unbounded
   - **Mitigation**:
     - Time-bounded joins only
     - State TTL and eviction
     - Memory limits with backpressure

3. **Watermark Correctness**
   - **Risk**: Incorrect watermark handling leads to wrong results
   - **Mitigation**:
     - Reuse existing tested watermark implementation
     - Extensive watermark propagation tests
     - Document watermark semantics clearly

4. **Parser Edge Cases**
   - **Risk**: Parser may fail on valid SQL or accept invalid SQL
   - **Mitigation**:
     - Fuzzing with `cargo-fuzz`
     - Test against PostgreSQL parser
     - Grammar specification review

### Medium-Risk Areas

1. **Dependency on `processor` crate**
   - **Risk**: Changes to processor may break SQL
   - **Mitigation**:
     - Clear interface contracts
     - Integration tests
     - Semantic versioning

2. **Performance Regression**
   - **Risk**: SQL overhead may be too high
   - **Mitigation**:
     - Continuous benchmarking
     - Performance budgets
     - Profiling and optimization

---

## 11. Alternatives Considered

### Alternative 1: Use Apache DataFusion

**Pros**:
- Battle-tested SQL engine
- Full SQL support
- Active community

**Cons**:
- Heavyweight dependency (100k+ LOC)
- Not designed for streaming
- Limited integration with our custom windowing

**Decision**: Rejected. We need streaming-first SQL with tight integration.

### Alternative 2: Embed DuckDB

**Pros**:
- Excellent SQL support
- Fast analytics

**Cons**:
- C++ dependency
- Not designed for streaming
- Complex embedding

**Decision**: Rejected. Streaming is a core requirement.

### Alternative 3: Custom DSL (not SQL)

**Pros**:
- Full control over syntax
- Optimized for our use case

**Cons**:
- Steep learning curve
- Reinventing the wheel
- Poor integration with BI tools

**Decision**: Rejected. SQL is industry standard.

---

## 12. Future Enhancements (Post-MVP)

After Phase 6, consider these enhancements:

1. **SQL:2023 Features**
   - Graph queries
   - JSON path expressions
   - Temporal tables (FOR SYSTEM_TIME)

2. **Machine Learning Integration**
   - `PREDICT` function for model inference
   - `TRAIN` statement for online learning

3. **Advanced Optimizations**
   - Adaptive query execution
   - Runtime statistics feedback
   - Query result caching

4. **Federation**
   - Cross-database queries
   - Push-down to remote databases

5. **Materialized Views**
   - Incremental view maintenance
   - Automatic view refresh

---

## Conclusion

This implementation plan provides a comprehensive roadmap for building a production-grade SQL interface for the LLM Auto-Optimizer. By following the phased approach, maintaining high test coverage, and leveraging existing infrastructure, we can deliver a powerful SQL layer that empowers users to query streaming LLM metrics with familiar syntax.

**Key Takeaways**:
- **12-week timeline** with 6 distinct phases
- **~10,000 lines of code** across 47 source files
- **800+ tests** ensuring correctness and performance
- **Tight integration** with existing processor, storage, and types crates
- **Clear success criteria** for each phase

The SQL interface will democratize access to LLM performance data, enabling data analysts, DevOps engineers, and business stakeholders to gain insights without writing Rust code.

---

**Next Steps**:
1. Review and approve this implementation plan
2. Set up `crates/sql` directory structure
3. Begin Phase 1 implementation
4. Schedule weekly progress reviews

**Questions?** Contact the LLM Auto-Optimizer team.
