# SQL Query Planner Architecture for Stream Processing

**Version:** 1.0
**Date:** 2025-11-10
**Author:** Query Planner Architect
**Status:** Architecture Specification

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Architecture Overview](#architecture-overview)
3. [Logical Plan Layer](#logical-plan-layer)
4. [Physical Plan Layer](#physical-plan-layer)
5. [Query Optimizer](#query-optimizer)
6. [Cost Model](#cost-model)
7. [Integration with Stream Processor](#integration-with-stream-processor)
8. [Optimization Rules](#optimization-rules)
9. [Streaming-Specific Optimizations](#streaming-specific-optimizations)
10. [Execution Strategy](#execution-strategy)
11. [Performance Characteristics](#performance-characteristics)
12. [Implementation Roadmap](#implementation-roadmap)

---

## Executive Summary

This document specifies the architecture for a SQL query planner and optimizer that translates SQL queries into efficient stream processing execution plans. The design follows proven patterns from Apache Calcite and Apache Flink while being tailored for the existing stream processor infrastructure.

### Key Design Principles

- **Volcano/Cascade Framework**: Dynamic programming-based optimization with rule-based transformations
- **Trait-Based Physical Properties**: Physical properties (partitioning, ordering) modeled as operator traits
- **Streaming-First**: Native support for windows, watermarks, and unbounded streams
- **Cost-Based Optimization**: Cardinality estimation and cost models for plan selection
- **Incremental Execution**: Leverage existing windowing and aggregation operators

### Goals

| Goal | Target | Approach |
|------|--------|----------|
| Query Planning Latency | < 50ms for simple queries | Cached plan templates, rule memoization |
| Execution Performance | No overhead vs hand-coded pipelines | Direct translation to stream operators |
| Optimization Quality | 80%+ of optimal performance | Cost-based optimization with streaming heuristics |
| SQL Coverage | SELECT, WHERE, GROUP BY, JOIN, WINDOW | Incremental feature additions |

---

## Architecture Overview

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        SQL Query Planner                                │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌──────────────┐         ┌──────────────┐         ┌──────────────┐   │
│  │   SQL Parser │────────▶│   Validator  │────────▶│   Logical    │   │
│  │   (sqlparser)│         │              │         │   Planner    │   │
│  └──────────────┘         └──────────────┘         └──────────────┘   │
│                                                             │           │
│                                                             ▼           │
│                                                    ┌──────────────┐    │
│                                                    │   Logical    │    │
│                                                    │     Plan     │    │
│                                                    └──────────────┘    │
│                                                             │           │
│                                                             ▼           │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                     Query Optimizer                              │  │
│  │                                                                  │  │
│  │  ┌─────────────────┐      ┌─────────────────┐                  │  │
│  │  │  Rule-Based     │      │  Cost-Based     │                  │  │
│  │  │  Optimization   │◀────▶│  Optimization   │                  │  │
│  │  │                 │      │                 │                  │  │
│  │  │  • Predicate    │      │  • Cardinality  │                  │  │
│  │  │    pushdown     │      │    estimation   │                  │  │
│  │  │  • Projection   │      │  • Cost model   │                  │  │
│  │  │    pruning      │      │  • Plan         │                  │  │
│  │  │  • Constant     │      │    enumeration  │                  │  │
│  │  │    folding      │      │                 │                  │  │
│  │  └─────────────────┘      └─────────────────┘                  │  │
│  │                                    │                            │  │
│  └────────────────────────────────────┼────────────────────────────┘  │
│                                       ▼                                │
│                              ┌──────────────┐                          │
│                              │   Physical   │                          │
│                              │     Plan     │                          │
│                              └──────────────┘                          │
│                                       │                                │
│                                       ▼                                │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                   Execution Plan Generator                       │  │
│  │                                                                  │  │
│  │  Physical Plan → StreamPipeline Translation                     │  │
│  │                                                                  │  │
│  │  • Map to stream operators                                      │  │
│  │  • Configure state backends                                     │  │
│  │  • Set up watermark strategies                                  │  │
│  │  • Connect to sources/sinks                                     │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                       │                                │
│                                       ▼                                │
│                              ┌──────────────┐                          │
│                              │ StreamPipeline│                         │
│                              │   (Execution) │                         │
│                              └──────────────┘                          │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

1. **SQL Parser**: Parse SQL text into AST (using `sqlparser-rs`)
2. **Validator**: Type checking, schema validation, semantic analysis
3. **Logical Planner**: Create logical operator tree
4. **Query Optimizer**: Apply transformation rules and cost-based optimization
5. **Physical Planner**: Generate physical execution plan
6. **Execution Plan Generator**: Translate to StreamPipeline

---

## Logical Plan Layer

### Logical Plan Operators

Logical operators represent **what** to compute, not **how** to compute it.

```rust
/// Logical plan operator trait
pub trait LogicalOperator: Debug + Clone + Send + Sync {
    /// Get the operator type
    fn operator_type(&self) -> LogicalOperatorType;

    /// Get input operators
    fn inputs(&self) -> Vec<Arc<dyn LogicalOperator>>;

    /// Get the output schema
    fn schema(&self) -> SchemaRef;

    /// Get estimated cardinality (number of rows)
    fn estimated_cardinality(&self) -> Option<usize>;

    /// Accept a visitor for transformation
    fn accept<V: LogicalVisitor>(&self, visitor: &mut V) -> Result<()>;
}

/// Logical operator types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogicalOperatorType {
    Scan,
    Filter,
    Project,
    Aggregate,
    Window,
    Join,
    Sort,
    Limit,
    Union,
    Distinct,
}
```

### Logical Operator Definitions

#### 1. Scan (TableScan)

Reads from a data source.

```rust
/// Logical table scan operator
#[derive(Debug, Clone)]
pub struct LogicalScan {
    /// Table name
    pub table_name: String,

    /// Table schema
    pub schema: SchemaRef,

    /// Optional filter pushed down to source
    pub pushed_filter: Option<Expr>,

    /// Projected columns (None = all columns)
    pub projection: Option<Vec<usize>>,

    /// Estimated row count
    pub estimated_rows: Option<usize>,
}

impl LogicalOperator for LogicalScan {
    fn operator_type(&self) -> LogicalOperatorType {
        LogicalOperatorType::Scan
    }

    fn inputs(&self) -> Vec<Arc<dyn LogicalOperator>> {
        vec![] // Leaf operator
    }

    fn schema(&self) -> SchemaRef {
        if let Some(projection) = &self.projection {
            Arc::new(self.schema.project(projection))
        } else {
            self.schema.clone()
        }
    }

    fn estimated_cardinality(&self) -> Option<usize> {
        self.estimated_rows
    }
}
```

**SQL Mapping:**
```sql
FROM events
-- LogicalScan { table_name: "events", schema: events_schema, ... }
```

#### 2. Filter (WHERE clause)

Filters rows based on a predicate.

```rust
/// Logical filter operator
#[derive(Debug, Clone)]
pub struct LogicalFilter {
    /// Input operator
    pub input: Arc<dyn LogicalOperator>,

    /// Filter predicate
    pub predicate: Expr,

    /// Estimated selectivity (0.0 - 1.0)
    pub selectivity: Option<f64>,
}

impl LogicalOperator for LogicalFilter {
    fn operator_type(&self) -> LogicalOperatorType {
        LogicalOperatorType::Filter
    }

    fn inputs(&self) -> Vec<Arc<dyn LogicalOperator>> {
        vec![self.input.clone()]
    }

    fn schema(&self) -> SchemaRef {
        self.input.schema()
    }

    fn estimated_cardinality(&self) -> Option<usize> {
        if let (Some(input_card), Some(sel)) = (self.input.estimated_cardinality(), self.selectivity) {
            Some((input_card as f64 * sel) as usize)
        } else {
            None
        }
    }
}
```

**SQL Mapping:**
```sql
WHERE latency > 100
-- LogicalFilter { predicate: Gt(Column("latency"), Literal(100)), ... }
```

#### 3. Project (SELECT clause)

Selects and transforms columns.

```rust
/// Logical projection operator
#[derive(Debug, Clone)]
pub struct LogicalProject {
    /// Input operator
    pub input: Arc<dyn LogicalOperator>,

    /// Projected expressions
    pub exprs: Vec<Expr>,

    /// Output schema
    pub schema: SchemaRef,
}

impl LogicalOperator for LogicalProject {
    fn operator_type(&self) -> LogicalOperatorType {
        LogicalOperatorType::Project
    }

    fn inputs(&self) -> Vec<Arc<dyn LogicalOperator>> {
        vec![self.input.clone()]
    }

    fn schema(&self) -> SchemaRef {
        self.schema.clone()
    }

    fn estimated_cardinality(&self) -> Option<usize> {
        self.input.estimated_cardinality()
    }
}
```

**SQL Mapping:**
```sql
SELECT model_id, AVG(latency) as avg_latency
-- LogicalProject { exprs: [Column("model_id"), Avg(Column("latency"))], ... }
```

#### 4. Aggregate (GROUP BY)

Groups rows and applies aggregate functions.

```rust
/// Logical aggregation operator
#[derive(Debug, Clone)]
pub struct LogicalAggregate {
    /// Input operator
    pub input: Arc<dyn LogicalOperator>,

    /// Group by expressions
    pub group_exprs: Vec<Expr>,

    /// Aggregate expressions
    pub aggr_exprs: Vec<AggregateExpr>,

    /// Output schema
    pub schema: SchemaRef,
}

/// Aggregate expression
#[derive(Debug, Clone)]
pub struct AggregateExpr {
    /// Aggregate function
    pub func: AggregateFunction,

    /// Input expression
    pub expr: Expr,

    /// Output column name
    pub alias: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AggregateFunction {
    Count,
    Sum,
    Avg,
    Min,
    Max,
    Stddev,
    Percentile(u8), // e.g., Percentile(95) for p95
}
```

**SQL Mapping:**
```sql
SELECT model_id, COUNT(*), AVG(latency), PERCENTILE(latency, 0.95)
FROM events
GROUP BY model_id
-- LogicalAggregate {
--   group_exprs: [Column("model_id")],
--   aggr_exprs: [Count(*), Avg(latency), Percentile(latency, 95)],
-- }
```

#### 5. Window (Temporal Windows)

Streaming-specific operator for time-based windowing.

```rust
/// Logical window operator
#[derive(Debug, Clone)]
pub struct LogicalWindow {
    /// Input operator
    pub input: Arc<dyn LogicalOperator>,

    /// Window specification
    pub window_spec: WindowSpec,

    /// Time column
    pub time_column: String,

    /// Watermark configuration
    pub watermark: Option<WatermarkSpec>,
}

/// Window specification
#[derive(Debug, Clone)]
pub enum WindowSpec {
    /// Tumbling window (size)
    Tumbling {
        size: Duration,
    },

    /// Sliding window (size, slide)
    Sliding {
        size: Duration,
        slide: Duration,
    },

    /// Session window (gap)
    Session {
        gap: Duration,
    },

    /// Hopping window (alias for sliding)
    Hopping {
        size: Duration,
        hop: Duration,
    },
}

/// Watermark specification
#[derive(Debug, Clone)]
pub struct WatermarkSpec {
    /// Maximum out-of-order delay
    pub max_delay: Duration,

    /// Idle timeout
    pub idle_timeout: Option<Duration>,
}
```

**SQL Mapping:**
```sql
SELECT
    TUMBLE_START(event_time, INTERVAL '5' MINUTE) as window_start,
    model_id,
    COUNT(*) as event_count
FROM events
GROUP BY TUMBLE(event_time, INTERVAL '5' MINUTE), model_id
-- LogicalWindow { window_spec: Tumbling { size: 5min }, ... }
-- + LogicalAggregate { group_exprs: [window, model_id], ... }
```

#### 6. Join

Joins two streams or tables.

```rust
/// Logical join operator
#[derive(Debug, Clone)]
pub struct LogicalJoin {
    /// Left input
    pub left: Arc<dyn LogicalOperator>,

    /// Right input
    pub right: Arc<dyn LogicalOperator>,

    /// Join condition
    pub condition: Expr,

    /// Join type
    pub join_type: JoinType,

    /// Output schema
    pub schema: SchemaRef,

    /// Time window for temporal join (streaming)
    pub time_window: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinType {
    Inner,
    Left,
    Right,
    Full,
    Semi,
    Anti,
}
```

**SQL Mapping:**
```sql
SELECT e.*, m.model_name
FROM events e
JOIN models m ON e.model_id = m.id
-- LogicalJoin {
--   left: LogicalScan(events),
--   right: LogicalScan(models),
--   condition: Eq(e.model_id, m.id),
--   join_type: Inner,
-- }
```

#### 7. Sort (ORDER BY)

Orders output rows.

```rust
/// Logical sort operator
#[derive(Debug, Clone)]
pub struct LogicalSort {
    /// Input operator
    pub input: Arc<dyn LogicalOperator>,

    /// Sort expressions with direction
    pub sort_exprs: Vec<SortExpr>,

    /// Optional limit
    pub limit: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct SortExpr {
    pub expr: Expr,
    pub direction: SortDirection,
    pub nulls_first: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}
```

**SQL Mapping:**
```sql
SELECT * FROM events ORDER BY latency DESC LIMIT 100
-- LogicalSort {
--   sort_exprs: [SortExpr { expr: Column("latency"), direction: Desc }],
--   limit: Some(100),
-- }
```

#### 8. Limit

Limits the number of output rows.

```rust
/// Logical limit operator
#[derive(Debug, Clone)]
pub struct LogicalLimit {
    /// Input operator
    pub input: Arc<dyn LogicalOperator>,

    /// Number of rows to skip
    pub offset: usize,

    /// Maximum number of rows to return
    pub limit: usize,
}
```

**SQL Mapping:**
```sql
SELECT * FROM events LIMIT 100 OFFSET 50
-- LogicalLimit { offset: 50, limit: 100 }
```

### Expression System

```rust
/// SQL expression
#[derive(Debug, Clone)]
pub enum Expr {
    /// Column reference
    Column(String),

    /// Literal value
    Literal(ScalarValue),

    /// Binary operation
    BinaryExpr {
        left: Box<Expr>,
        op: BinaryOperator,
        right: Box<Expr>,
    },

    /// Unary operation
    UnaryExpr {
        op: UnaryOperator,
        expr: Box<Expr>,
    },

    /// Function call
    ScalarFunction {
        func: ScalarFunction,
        args: Vec<Expr>,
    },

    /// CASE expression
    Case {
        expr: Option<Box<Expr>>,
        when_then: Vec<(Expr, Expr)>,
        else_expr: Option<Box<Expr>>,
    },

    /// IN expression
    InList {
        expr: Box<Expr>,
        list: Vec<Expr>,
        negated: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinaryOperator {
    // Arithmetic
    Add, Sub, Mul, Div, Mod,

    // Comparison
    Eq, NotEq, Lt, LtEq, Gt, GtEq,

    // Logical
    And, Or,

    // String
    Like, NotLike,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnaryOperator {
    Not,
    Minus,
    IsNull,
    IsNotNull,
}
```

---

## Physical Plan Layer

### Physical Plan Operators

Physical operators specify **how** to execute the computation with specific algorithms and resource properties.

```rust
/// Physical plan operator trait
pub trait PhysicalOperator: Debug + Send + Sync {
    /// Get the operator type
    fn operator_type(&self) -> PhysicalOperatorType;

    /// Get input operators
    fn inputs(&self) -> Vec<Arc<dyn PhysicalOperator>>;

    /// Get the output schema
    fn schema(&self) -> SchemaRef;

    /// Get physical properties
    fn properties(&self) -> &PhysicalProperties;

    /// Estimate execution cost
    fn cost(&self, stats: &Statistics) -> Cost;

    /// Translate to stream pipeline operators
    fn to_stream_operator(&self) -> Result<Box<dyn StreamOperator>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhysicalOperatorType {
    KafkaSource,
    FileSource,
    FilterExec,
    ProjectExec,
    HashAggregateExec,
    StreamingAggregateExec,
    WindowAggregateExec,
    HashJoinExec,
    MergeJoinExec,
    StreamingJoinExec,
    SortExec,
    LimitExec,
    KafkaSink,
}

/// Physical properties of an operator
#[derive(Debug, Clone)]
pub struct PhysicalProperties {
    /// Partitioning scheme
    pub partitioning: Partitioning,

    /// Ordering guarantee
    pub ordering: Option<Vec<SortExpr>>,

    /// Parallelism level
    pub parallelism: usize,

    /// State backend requirement
    pub state_backend: Option<StateBackendType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Partitioning {
    /// Data is not partitioned
    UnknownPartitioning(usize),

    /// Data is partitioned by hash of specific columns
    Hash(Vec<String>, usize),

    /// Data is round-robin partitioned
    RoundRobin(usize),

    /// Single partition
    Single,
}
```

### Physical Operator Implementations

#### 1. Source Operators

```rust
/// Kafka source operator
#[derive(Debug)]
pub struct KafkaSourceExec {
    /// Kafka configuration
    pub config: KafkaSourceConfig,

    /// Output schema
    pub schema: SchemaRef,

    /// Watermark strategy
    pub watermark: WatermarkStrategy,

    /// Parallelism (number of partitions)
    pub parallelism: usize,
}

impl PhysicalOperator for KafkaSourceExec {
    fn to_stream_operator(&self) -> Result<Box<dyn StreamOperator>> {
        let source = KafkaSource::new(self.config.clone())?;
        Ok(Box::new(source))
    }

    fn properties(&self) -> &PhysicalProperties {
        &PhysicalProperties {
            partitioning: Partitioning::Hash(vec!["partition_key".to_string()], self.parallelism),
            ordering: None,
            parallelism: self.parallelism,
            state_backend: None,
        }
    }
}
```

#### 2. Filter Execution

```rust
/// Physical filter operator
#[derive(Debug)]
pub struct FilterExec {
    /// Input operator
    pub input: Arc<dyn PhysicalOperator>,

    /// Compiled predicate
    pub predicate: Arc<dyn PhysicalExpr>,
}

impl PhysicalOperator for FilterExec {
    fn to_stream_operator(&self) -> Result<Box<dyn StreamOperator>> {
        let predicate = self.predicate.clone();
        Ok(Box::new(FilterOperator::new(move |event| {
            predicate.evaluate(event)
        })))
    }
}
```

#### 3. Aggregation Execution

Multiple strategies based on use case:

```rust
/// Streaming window aggregation (for time-based GROUP BY)
#[derive(Debug)]
pub struct WindowAggregateExec {
    /// Input operator
    pub input: Arc<dyn PhysicalOperator>,

    /// Window specification
    pub window: WindowSpec,

    /// Group by keys
    pub group_by: Vec<Arc<dyn PhysicalExpr>>,

    /// Aggregation functions
    pub aggregates: Vec<Arc<dyn AggregateExec>>,

    /// State backend
    pub state_backend: StateBackendType,
}

impl PhysicalOperator for WindowAggregateExec {
    fn to_stream_operator(&self) -> Result<Box<dyn StreamOperator>> {
        // Map to existing WindowOperator + AggregateOperator
        let window_op = match &self.window {
            WindowSpec::Tumbling { size } => {
                WindowOperator::tumbling(*size)
            }
            WindowSpec::Sliding { size, slide } => {
                WindowOperator::sliding(*size, *slide)
            }
            WindowSpec::Session { gap } => {
                WindowOperator::session(*gap)
            }
        };

        // Configure aggregators
        let aggregators: Vec<Box<dyn Aggregator>> = self.aggregates
            .iter()
            .map(|agg| agg.to_aggregator())
            .collect::<Result<_>>()?;

        let aggregate_op = AggregateOperator::new(aggregators, self.state_backend);

        // Compose operators
        Ok(Box::new(CompositeOperator::new(vec![
            Box::new(window_op),
            Box::new(aggregate_op),
        ])))
    }

    fn cost(&self, stats: &Statistics) -> Cost {
        let input_cost = self.input.cost(stats);
        let state_cost = self.estimate_state_cost(stats);

        Cost {
            cpu: input_cost.cpu + (stats.row_count * 10), // Aggregation cost
            memory: state_cost,
            io: input_cost.io,
            network: 0,
        }
    }
}

/// Hash aggregation (for non-streaming GROUP BY)
#[derive(Debug)]
pub struct HashAggregateExec {
    /// Input operator
    pub input: Arc<dyn PhysicalOperator>,

    /// Group by keys
    pub group_by: Vec<Arc<dyn PhysicalExpr>>,

    /// Aggregation functions
    pub aggregates: Vec<Arc<dyn AggregateExec>>,
}
```

#### 4. Join Execution

```rust
/// Hash join (for bounded inputs or interval joins)
#[derive(Debug)]
pub struct HashJoinExec {
    /// Left input
    pub left: Arc<dyn PhysicalOperator>,

    /// Right input
    pub right: Arc<dyn PhysicalOperator>,

    /// Join condition
    pub condition: Arc<dyn PhysicalExpr>,

    /// Join type
    pub join_type: JoinType,

    /// Build side (left or right)
    pub build_side: JoinSide,

    /// State backend for hash table
    pub state_backend: StateBackendType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinSide {
    Left,
    Right,
}

/// Streaming temporal join
#[derive(Debug)]
pub struct StreamingJoinExec {
    /// Left stream
    pub left: Arc<dyn PhysicalOperator>,

    /// Right stream
    pub right: Arc<dyn PhysicalOperator>,

    /// Join condition
    pub condition: Arc<dyn PhysicalExpr>,

    /// Join type
    pub join_type: JoinType,

    /// Time window bounds
    pub time_bounds: Duration,

    /// State backend
    pub state_backend: StateBackendType,
}
```

---

## Query Optimizer

### Optimization Framework

The optimizer follows the Volcano/Cascade framework with two phases:

1. **Rule-Based Optimization**: Apply heuristic transformation rules
2. **Cost-Based Optimization**: Enumerate plans and select lowest cost

```rust
/// Query optimizer
pub struct QueryOptimizer {
    /// Logical optimization rules
    logical_rules: Vec<Box<dyn OptimizationRule>>,

    /// Physical optimization rules
    physical_rules: Vec<Box<dyn OptimizationRule>>,

    /// Cost estimator
    cost_estimator: Arc<CostEstimator>,

    /// Statistics provider
    statistics: Arc<dyn StatisticsProvider>,

    /// Configuration
    config: OptimizerConfig,
}

impl QueryOptimizer {
    /// Optimize a logical plan
    pub fn optimize(&self, logical_plan: LogicalPlan) -> Result<PhysicalPlan> {
        // Phase 1: Logical optimization (rule-based)
        let optimized_logical = self.apply_logical_rules(logical_plan)?;

        // Phase 2: Physical planning (enumerate alternatives)
        let physical_plans = self.enumerate_physical_plans(&optimized_logical)?;

        // Phase 3: Cost-based selection
        let best_plan = self.select_best_plan(physical_plans)?;

        // Phase 4: Physical optimization
        let optimized_physical = self.apply_physical_rules(best_plan)?;

        Ok(optimized_physical)
    }

    /// Apply logical optimization rules
    fn apply_logical_rules(&self, plan: LogicalPlan) -> Result<LogicalPlan> {
        let mut current = plan;
        let mut changed = true;
        let mut iterations = 0;
        const MAX_ITERATIONS: usize = 10;

        while changed && iterations < MAX_ITERATIONS {
            changed = false;
            iterations += 1;

            for rule in &self.logical_rules {
                if let Some(new_plan) = rule.apply(&current)? {
                    current = new_plan;
                    changed = true;
                }
            }
        }

        Ok(current)
    }

    /// Enumerate physical plan alternatives
    fn enumerate_physical_plans(&self, logical: &LogicalPlan) -> Result<Vec<PhysicalPlan>> {
        // Use dynamic programming to avoid redundant enumeration
        let mut memo = Memo::new();
        self.enumerate_recursive(logical, &mut memo)
    }

    /// Select the plan with lowest estimated cost
    fn select_best_plan(&self, plans: Vec<PhysicalPlan>) -> Result<PhysicalPlan> {
        let stats = self.statistics.get_statistics()?;

        plans.into_iter()
            .min_by_key(|plan| {
                let cost = plan.root().cost(&stats);
                cost.total()
            })
            .ok_or_else(|| anyhow!("No physical plans generated"))
    }
}

/// Optimization rule trait
pub trait OptimizationRule: Debug + Send + Sync {
    /// Try to apply the rule to a plan
    /// Returns Some(new_plan) if rule applies, None otherwise
    fn apply(&self, plan: &LogicalPlan) -> Result<Option<LogicalPlan>>;

    /// Rule name for debugging
    fn name(&self) -> &str;
}
```

---

## Optimization Rules

### Rule-Based Optimization Rules

#### 1. Predicate Pushdown

Push filters as close to the source as possible to reduce data volume.

```rust
/// Predicate pushdown rule
#[derive(Debug)]
pub struct PredicatePushdown;

impl OptimizationRule for PredicatePushdown {
    fn apply(&self, plan: &LogicalPlan) -> Result<Option<LogicalPlan>> {
        match plan.root() {
            LogicalOperator::Filter(filter) => {
                match filter.input.as_ref() {
                    // Push filter through projection
                    LogicalOperator::Project(project) => {
                        if self.can_push_through_project(&filter.predicate, project) {
                            // Rewrite: Project(Filter(input)) -> Filter(Project(input))
                            return Ok(Some(self.rewrite_filter_project(filter, project)?));
                        }
                    }

                    // Push filter into scan
                    LogicalOperator::Scan(scan) => {
                        // Merge filter into scan's pushed_filter
                        let new_scan = LogicalScan {
                            pushed_filter: Some(self.merge_predicates(
                                scan.pushed_filter.as_ref(),
                                &filter.predicate
                            )?),
                            ..scan.clone()
                        };
                        return Ok(Some(LogicalPlan::new(LogicalOperator::Scan(new_scan))));
                    }

                    // Push filter through join (when applicable)
                    LogicalOperator::Join(join) => {
                        if let Some(new_join) = self.push_filter_into_join(filter, join)? {
                            return Ok(Some(LogicalPlan::new(LogicalOperator::Join(new_join))));
                        }
                    }

                    _ => {}
                }
            }
            _ => {}
        }

        Ok(None)
    }

    fn name(&self) -> &str {
        "PredicatePushdown"
    }
}
```

**Example:**
```sql
-- Before optimization
SELECT model_id, latency FROM events WHERE latency > 100

-- Logical plan before:
Project([model_id, latency])
  Filter(latency > 100)
    Scan(events)

-- After predicate pushdown:
Project([model_id, latency])
  Scan(events, filter=latency > 100)
```

#### 2. Projection Pushdown

Prune unnecessary columns early to reduce data transfer.

```rust
/// Projection pushdown rule
#[derive(Debug)]
pub struct ProjectionPushdown;

impl OptimizationRule for ProjectionPushdown {
    fn apply(&self, plan: &LogicalPlan) -> Result<Option<LogicalPlan>> {
        let required_columns = self.compute_required_columns(plan)?;

        match plan.root() {
            LogicalOperator::Scan(scan) if scan.projection.is_none() => {
                // Add projection to scan
                let column_indices = self.get_column_indices(&scan.schema, &required_columns);
                let new_scan = LogicalScan {
                    projection: Some(column_indices),
                    ..scan.clone()
                };
                Ok(Some(LogicalPlan::new(LogicalOperator::Scan(new_scan))))
            }
            _ => Ok(None)
        }
    }

    fn name(&self) -> &str {
        "ProjectionPushdown"
    }
}
```

**Example:**
```sql
-- Before optimization
SELECT model_id FROM events

-- Logical plan before:
Project([model_id])
  Scan(events) -- reads all columns

-- After projection pushdown:
Project([model_id])
  Scan(events, projection=[0]) -- reads only model_id column
```

#### 3. Constant Folding

Evaluate constant expressions at planning time.

```rust
/// Constant folding rule
#[derive(Debug)]
pub struct ConstantFolding;

impl OptimizationRule for ConstantFolding {
    fn apply(&self, plan: &LogicalPlan) -> Result<Option<LogicalPlan>> {
        let mut rewriter = ConstantFoldingRewriter::new();
        plan.rewrite(&mut rewriter)
    }

    fn name(&self) -> &str {
        "ConstantFolding"
    }
}

struct ConstantFoldingRewriter;

impl ConstantFoldingRewriter {
    fn fold_expr(&self, expr: &Expr) -> Result<Expr> {
        match expr {
            Expr::BinaryExpr { left, op, right } => {
                if let (Expr::Literal(l), Expr::Literal(r)) = (left.as_ref(), right.as_ref()) {
                    // Evaluate at compile time
                    return Ok(Expr::Literal(self.evaluate_binary(l, op, r)?));
                }
            }
            _ => {}
        }
        Ok(expr.clone())
    }
}
```

**Example:**
```sql
-- Before optimization
WHERE latency > (100 + 50)

-- After constant folding:
WHERE latency > 150
```

#### 4. Join Reordering

Reorder joins to minimize intermediate result sizes.

```rust
/// Join reordering rule
#[derive(Debug)]
pub struct JoinReordering;

impl OptimizationRule for JoinReordering {
    fn apply(&self, plan: &LogicalPlan) -> Result<Option<LogicalPlan>> {
        // Collect all joins in the plan
        let joins = self.collect_joins(plan)?;

        if joins.len() < 2 {
            return Ok(None); // Nothing to reorder
        }

        // Use dynamic programming to find optimal join order
        let best_order = self.optimize_join_order(&joins)?;

        // Rebuild plan with new join order
        Ok(Some(self.rebuild_with_join_order(plan, best_order)?))
    }

    fn name(&self) -> &str {
        "JoinReordering"
    }

    /// Use Selinger-style dynamic programming for join ordering
    fn optimize_join_order(&self, joins: &[JoinNode]) -> Result<Vec<usize>> {
        let n = joins.len();
        let mut best_plan: HashMap<BitSet, (Cost, Vec<usize>)> = HashMap::new();

        // Base case: single relations
        for i in 0..n {
            let mut set = BitSet::new();
            set.insert(i);
            best_plan.insert(set, (joins[i].cost.clone(), vec![i]));
        }

        // Build up larger join sets
        for size in 2..=n {
            for subset in BitSet::combinations(n, size) {
                let mut min_cost = Cost::max();
                let mut best_order = vec![];

                // Try all ways to split this set
                for left_set in subset.proper_subsets() {
                    let right_set = &subset - &left_set;

                    let (left_cost, left_order) = &best_plan[&left_set];
                    let (right_cost, right_order) = &best_plan[&right_set];

                    let join_cost = self.estimate_join_cost(&left_set, &right_set);
                    let total_cost = left_cost + right_cost + join_cost;

                    if total_cost < min_cost {
                        min_cost = total_cost;
                        best_order = [left_order.clone(), right_order.clone()].concat();
                    }
                }

                best_plan.insert(subset, (min_cost, best_order));
            }
        }

        Ok(best_plan[&BitSet::all(n)].1.clone())
    }
}
```

#### 5. Window Coalescing

Merge multiple window operations with same specification.

```rust
/// Window coalescing rule
#[derive(Debug)]
pub struct WindowCoalescing;

impl OptimizationRule for WindowCoalescing {
    fn apply(&self, plan: &LogicalPlan) -> Result<Option<LogicalPlan>> {
        // Find window operations with identical specifications
        let windows = self.find_coalescable_windows(plan)?;

        if windows.len() < 2 {
            return Ok(None);
        }

        // Merge aggregations from multiple windows into one
        Ok(Some(self.merge_windows(plan, windows)?))
    }

    fn name(&self) -> &str {
        "WindowCoalescing"
    }
}
```

**Example:**
```sql
-- Before optimization
SELECT
    window_start,
    COUNT(*) as cnt,
    AVG(latency) as avg_lat
FROM events
GROUP BY TUMBLE(event_time, INTERVAL '5' MINUTE)

UNION ALL

SELECT
    window_start,
    MAX(latency) as max_lat,
    MIN(latency) as min_lat
FROM events
GROUP BY TUMBLE(event_time, INTERVAL '5' MINUTE)

-- After window coalescing: single window with all aggregations
SELECT
    window_start,
    COUNT(*) as cnt,
    AVG(latency) as avg_lat,
    MAX(latency) as max_lat,
    MIN(latency) as min_lat
FROM events
GROUP BY TUMBLE(event_time, INTERVAL '5' MINUTE)
```

### Additional Logical Rules

```rust
/// Common subexpression elimination
#[derive(Debug)]
pub struct CommonSubexpressionElimination;

/// Aggregate pushdown (push aggregation below join when possible)
#[derive(Debug)]
pub struct AggregatePushdown;

/// Limit pushdown
#[derive(Debug)]
pub struct LimitPushdown;

/// Subquery flattening
#[derive(Debug)]
pub struct SubqueryFlattening;
```

---

## Cost Model

### Cost Estimation

```rust
/// Cost of executing a plan
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cost {
    /// CPU cost (in arbitrary units)
    pub cpu: u64,

    /// Memory cost (in bytes)
    pub memory: u64,

    /// Disk I/O cost (in bytes)
    pub io: u64,

    /// Network transfer cost (in bytes)
    pub network: u64,
}

impl Cost {
    /// Calculate total weighted cost
    pub fn total(&self) -> u64 {
        // Weights based on relative expense
        const CPU_WEIGHT: u64 = 1;
        const MEMORY_WEIGHT: u64 = 10;
        const IO_WEIGHT: u64 = 100;
        const NETWORK_WEIGHT: u64 = 50;

        self.cpu * CPU_WEIGHT
            + self.memory * MEMORY_WEIGHT
            + self.io * IO_WEIGHT
            + self.network * NETWORK_WEIGHT
    }

    pub fn max() -> Self {
        Cost {
            cpu: u64::MAX,
            memory: u64::MAX,
            io: u64::MAX,
            network: u64::MAX,
        }
    }
}

impl Add for Cost {
    type Output = Cost;

    fn add(self, other: Cost) -> Cost {
        Cost {
            cpu: self.cpu.saturating_add(other.cpu),
            memory: self.memory.max(other.memory), // Peak memory
            io: self.io.saturating_add(other.io),
            network: self.network.saturating_add(other.network),
        }
    }
}

/// Cost estimator
pub struct CostEstimator {
    /// Configuration parameters
    config: CostConfig,
}

#[derive(Debug, Clone)]
pub struct CostConfig {
    /// Cost per row scanned
    pub scan_cost_per_row: u64,

    /// Cost per row filtered
    pub filter_cost_per_row: u64,

    /// Cost per row projected
    pub project_cost_per_row: u64,

    /// Cost per row aggregated
    pub aggregate_cost_per_row: u64,

    /// Cost per join (nested loop)
    pub nested_loop_join_cost: u64,

    /// Cost per join (hash)
    pub hash_join_cost_per_row: u64,

    /// Cost per byte of state
    pub state_cost_per_byte: u64,
}

impl CostEstimator {
    /// Estimate cost for a scan operator
    pub fn estimate_scan(&self, scan: &LogicalScan) -> Cost {
        let row_count = scan.estimated_rows.unwrap_or(1000);
        let row_size = self.estimate_row_size(&scan.schema);

        Cost {
            cpu: row_count as u64 * self.config.scan_cost_per_row,
            memory: 0, // Streaming scan
            io: (row_count as u64) * (row_size as u64),
            network: 0,
        }
    }

    /// Estimate cost for a filter operator
    pub fn estimate_filter(&self, filter: &LogicalFilter, input_cost: &Cost) -> Cost {
        let input_rows = filter.input.estimated_cardinality().unwrap_or(1000);

        Cost {
            cpu: input_cost.cpu + (input_rows as u64 * self.config.filter_cost_per_row),
            memory: input_cost.memory,
            io: input_cost.io,
            network: input_cost.network,
        }
    }

    /// Estimate cost for an aggregation operator
    pub fn estimate_aggregate(&self, agg: &LogicalAggregate, input_cost: &Cost) -> Cost {
        let input_rows = agg.input.estimated_cardinality().unwrap_or(1000);
        let group_count = self.estimate_group_count(agg);
        let state_size = group_count * 1024; // Estimate 1KB per group

        Cost {
            cpu: input_cost.cpu + (input_rows as u64 * self.config.aggregate_cost_per_row),
            memory: state_size as u64,
            io: input_cost.io,
            network: input_cost.network,
        }
    }

    /// Estimate cost for a hash join
    pub fn estimate_hash_join(&self, join: &LogicalJoin, left_cost: &Cost, right_cost: &Cost) -> Cost {
        let left_rows = join.left.estimated_cardinality().unwrap_or(1000);
        let right_rows = join.right.estimated_cardinality().unwrap_or(1000);

        // Build hash table on smaller side
        let (build_rows, probe_rows) = if left_rows < right_rows {
            (left_rows, right_rows)
        } else {
            (right_rows, left_rows)
        };

        let hash_table_size = build_rows * 128; // Estimate 128 bytes per entry

        Cost {
            cpu: left_cost.cpu + right_cost.cpu
                + ((build_rows + probe_rows) as u64 * self.config.hash_join_cost_per_row),
            memory: (hash_table_size as u64) + left_cost.memory.max(right_cost.memory),
            io: left_cost.io + right_cost.io,
            network: left_cost.network + right_cost.network,
        }
    }
}
```

### Cardinality Estimation

```rust
/// Statistics provider interface
pub trait StatisticsProvider: Send + Sync {
    /// Get statistics for the entire system
    fn get_statistics(&self) -> Result<Statistics>;

    /// Get table statistics
    fn get_table_stats(&self, table_name: &str) -> Result<TableStatistics>;

    /// Get column statistics
    fn get_column_stats(&self, table_name: &str, column_name: &str) -> Result<ColumnStatistics>;
}

/// Overall statistics
#[derive(Debug, Clone)]
pub struct Statistics {
    pub row_count: usize,
    pub byte_size: usize,
}

/// Table statistics
#[derive(Debug, Clone)]
pub struct TableStatistics {
    /// Table name
    pub table_name: String,

    /// Estimated row count
    pub row_count: usize,

    /// Estimated total size in bytes
    pub byte_size: usize,

    /// Column statistics
    pub column_stats: HashMap<String, ColumnStatistics>,
}

/// Column statistics
#[derive(Debug, Clone)]
pub struct ColumnStatistics {
    /// Column name
    pub column_name: String,

    /// Number of distinct values (NDV)
    pub distinct_count: Option<usize>,

    /// Number of null values
    pub null_count: usize,

    /// Minimum value
    pub min_value: Option<ScalarValue>,

    /// Maximum value
    pub max_value: Option<ScalarValue>,

    /// Histogram for distribution
    pub histogram: Option<Histogram>,
}

/// Histogram for value distribution
#[derive(Debug, Clone)]
pub struct Histogram {
    pub buckets: Vec<HistogramBucket>,
}

#[derive(Debug, Clone)]
pub struct HistogramBucket {
    pub lower_bound: ScalarValue,
    pub upper_bound: ScalarValue,
    pub count: usize,
}

/// Cardinality estimator
pub struct CardinalityEstimator {
    stats_provider: Arc<dyn StatisticsProvider>,
}

impl CardinalityEstimator {
    /// Estimate cardinality for a filter
    pub fn estimate_filter_cardinality(
        &self,
        input_cardinality: usize,
        predicate: &Expr,
        table_stats: &TableStatistics,
    ) -> usize {
        let selectivity = self.estimate_selectivity(predicate, table_stats);
        (input_cardinality as f64 * selectivity) as usize
    }

    /// Estimate selectivity of a predicate
    fn estimate_selectivity(&self, predicate: &Expr, stats: &TableStatistics) -> f64 {
        match predicate {
            Expr::BinaryExpr { left, op, right } => {
                match op {
                    BinaryOperator::Eq => {
                        // Equality: selectivity = 1 / NDV
                        if let Expr::Column(col) = left.as_ref() {
                            if let Some(col_stats) = stats.column_stats.get(col) {
                                if let Some(ndv) = col_stats.distinct_count {
                                    return 1.0 / (ndv as f64);
                                }
                            }
                        }
                        0.1 // Default assumption
                    }

                    BinaryOperator::Lt | BinaryOperator::Gt => {
                        // Range: use histogram if available
                        if let Expr::Column(col) = left.as_ref() {
                            if let Some(col_stats) = stats.column_stats.get(col) {
                                if let Some(histogram) = &col_stats.histogram {
                                    return self.estimate_range_selectivity(histogram, op, right);
                                }
                            }
                        }
                        0.33 // Default assumption
                    }

                    BinaryOperator::And => {
                        // Independence assumption
                        let left_sel = self.estimate_selectivity(left, stats);
                        let right_sel = self.estimate_selectivity(right, stats);
                        left_sel * right_sel
                    }

                    BinaryOperator::Or => {
                        // Inclusion-exclusion
                        let left_sel = self.estimate_selectivity(left, stats);
                        let right_sel = self.estimate_selectivity(right, stats);
                        left_sel + right_sel - (left_sel * right_sel)
                    }

                    _ => 0.5 // Default
                }
            }

            _ => 0.5 // Default selectivity
        }
    }

    /// Estimate join cardinality
    pub fn estimate_join_cardinality(
        &self,
        left_card: usize,
        right_card: usize,
        join_condition: &Expr,
        left_stats: &TableStatistics,
        right_stats: &TableStatistics,
    ) -> usize {
        // For equi-join on primary key: cardinality = max(left, right)
        // For foreign key join: cardinality = left (assuming FK to PK)
        // Default: cardinality = (left * right) / max(NDV_left, NDV_right)

        match join_condition {
            Expr::BinaryExpr { left, op: BinaryOperator::Eq, right } => {
                if let (Expr::Column(left_col), Expr::Column(right_col)) = (left.as_ref(), right.as_ref()) {
                    let left_ndv = left_stats.column_stats.get(left_col)
                        .and_then(|s| s.distinct_count)
                        .unwrap_or(left_card);

                    let right_ndv = right_stats.column_stats.get(right_col)
                        .and_then(|s| s.distinct_count)
                        .unwrap_or(right_card);

                    let max_ndv = left_ndv.max(right_ndv).max(1);
                    return (left_card * right_card) / max_ndv;
                }
            }
            _ => {}
        }

        // Default: assume moderate join selectivity
        ((left_card * right_card) as f64 * 0.1) as usize
    }
}
```

---

## Streaming-Specific Optimizations

### 1. Watermark Propagation

Track watermarks through the plan to enable window triggers.

```rust
/// Watermark propagation optimizer
pub struct WatermarkPropagation;

impl OptimizationRule for WatermarkPropagation {
    fn apply(&self, plan: &LogicalPlan) -> Result<Option<LogicalPlan>> {
        // Analyze watermark flow through operators
        let watermark_flow = self.analyze_watermark_flow(plan)?;

        // Annotate operators with watermark metadata
        Ok(Some(self.annotate_watermarks(plan, watermark_flow)?))
    }

    fn name(&self) -> &str {
        "WatermarkPropagation"
    }
}

impl WatermarkPropagation {
    /// Determine how watermarks flow through each operator
    fn analyze_watermark_flow(&self, plan: &LogicalPlan) -> Result<WatermarkFlow> {
        match plan.root() {
            LogicalOperator::Scan(scan) => {
                // Source generates watermarks
                Ok(WatermarkFlow::Generating {
                    column: scan.watermark_column.clone(),
                    strategy: scan.watermark_strategy.clone(),
                })
            }

            LogicalOperator::Filter(_) | LogicalOperator::Project(_) => {
                // Preserve watermarks from input
                Ok(WatermarkFlow::Preserving)
            }

            LogicalOperator::Join(join) => {
                // Join emits min watermark from both sides
                Ok(WatermarkFlow::Merging(MergeStrategy::Min))
            }

            LogicalOperator::Union(_) => {
                // Union emits min watermark from all inputs
                Ok(WatermarkFlow::Merging(MergeStrategy::Min))
            }

            LogicalOperator::Window(_) => {
                // Window operators consume and emit watermarks
                Ok(WatermarkFlow::Transforming)
            }

            _ => Ok(WatermarkFlow::Unknown)
        }
    }
}
```

### 2. State Minimization

Reduce state size for streaming aggregations.

```rust
/// State minimization optimizer
pub struct StateMinimization;

impl OptimizationRule for StateMinimization {
    fn apply(&self, plan: &LogicalPlan) -> Result<Option<LogicalPlan>> {
        match plan.root() {
            LogicalOperator::Aggregate(agg) => {
                // Use incremental aggregation when possible
                if self.can_use_incremental_agg(agg) {
                    return Ok(Some(self.convert_to_incremental(agg)?));
                }

                // For distinct aggregations, use HyperLogLog approximation
                if self.has_distinct_count(agg) && self.can_approximate(agg) {
                    return Ok(Some(self.use_approximate_distinct(agg)?));
                }
            }

            LogicalOperator::Join(join) => {
                // For temporal joins, use TTL to expire old state
                if self.is_temporal_join(join) {
                    return Ok(Some(self.add_state_ttl(join)?));
                }
            }

            _ => {}
        }

        Ok(None)
    }

    fn name(&self) -> &str {
        "StateMinimization"
    }
}
```

### 3. Local-Global Aggregation

Split aggregation into local (pre-aggregation) and global phases to handle skew.

```rust
/// Local-global aggregation optimizer
pub struct LocalGlobalAggregation;

impl OptimizationRule for LocalGlobalAggregation {
    fn apply(&self, plan: &LogicalPlan) -> Result<Option<LogicalPlan>> {
        match plan.root() {
            LogicalOperator::Aggregate(agg) => {
                if self.should_use_local_global(agg)? {
                    // Split into two stages:
                    // 1. Local aggregation (partial)
                    // 2. Global aggregation (final)
                    return Ok(Some(self.create_two_phase_aggregation(agg)?));
                }
            }
            _ => {}
        }

        Ok(None)
    }

    fn name(&self) -> &str {
        "LocalGlobalAggregation"
    }
}

impl LocalGlobalAggregation {
    fn create_two_phase_aggregation(&self, agg: &LogicalAggregate) -> Result<LogicalPlan> {
        // Local aggregation (partial)
        let local_agg = LogicalAggregate {
            input: agg.input.clone(),
            group_exprs: agg.group_exprs.clone(),
            aggr_exprs: agg.aggr_exprs.iter()
                .map(|expr| self.to_partial_aggregate(expr))
                .collect(),
            schema: self.partial_schema(&agg.schema),
        };

        // Repartition by group key
        let repartition = LogicalRepartition {
            input: Arc::new(LogicalOperator::Aggregate(local_agg)),
            partition_by: agg.group_exprs.clone(),
            partition_count: self.estimate_optimal_parallelism(agg),
        };

        // Global aggregation (final)
        let global_agg = LogicalAggregate {
            input: Arc::new(LogicalOperator::Repartition(repartition)),
            group_exprs: agg.group_exprs.clone(),
            aggr_exprs: agg.aggr_exprs.iter()
                .map(|expr| self.to_final_aggregate(expr))
                .collect(),
            schema: agg.schema.clone(),
        };

        Ok(LogicalPlan::new(LogicalOperator::Aggregate(global_agg)))
    }

    /// Convert aggregate function to partial (local) version
    fn to_partial_aggregate(&self, expr: &AggregateExpr) -> AggregateExpr {
        match expr.func {
            AggregateFunction::Count => {
                // Partial count: COUNT(*) -> COUNT(*)
                expr.clone()
            }
            AggregateFunction::Sum => {
                // Partial sum: SUM(x) -> SUM(x)
                expr.clone()
            }
            AggregateFunction::Avg => {
                // Partial avg: AVG(x) -> (SUM(x), COUNT(x))
                // Returns struct with sum and count
                AggregateExpr {
                    func: AggregateFunction::PartialAvg,
                    expr: expr.expr.clone(),
                    alias: format!("{}_partial", expr.alias),
                }
            }
            _ => expr.clone()
        }
    }

    /// Convert partial aggregate to final version
    fn to_final_aggregate(&self, expr: &AggregateExpr) -> AggregateExpr {
        match expr.func {
            AggregateFunction::Count => {
                // Final count: SUM(partial_count)
                AggregateExpr {
                    func: AggregateFunction::Sum,
                    expr: expr.expr.clone(),
                    alias: expr.alias.clone(),
                }
            }
            AggregateFunction::Avg => {
                // Final avg: SUM(partial_sum) / SUM(partial_count)
                AggregateExpr {
                    func: AggregateFunction::FinalAvg,
                    expr: expr.expr.clone(),
                    alias: expr.alias.clone(),
                }
            }
            _ => expr.clone()
        }
    }
}
```

### 4. Mini-Batch Optimization

Buffer events into mini-batches for more efficient processing.

```rust
/// Mini-batch optimizer
pub struct MiniBatchOptimization {
    /// Target batch size
    batch_size: usize,

    /// Maximum latency
    max_latency: Duration,
}

impl OptimizationRule for MiniBatchOptimization {
    fn apply(&self, plan: &LogicalPlan) -> Result<Option<LogicalPlan>> {
        // Add mini-batch operators before expensive operations
        match plan.root() {
            LogicalOperator::Aggregate(_) | LogicalOperator::Join(_) => {
                // Insert mini-batch buffer
                let buffer = LogicalBuffer {
                    input: plan.root_arc(),
                    buffer_size: self.batch_size,
                    max_latency: self.max_latency,
                };

                Ok(Some(LogicalPlan::new(LogicalOperator::Buffer(buffer))))
            }
            _ => Ok(None)
        }
    }

    fn name(&self) -> &str {
        "MiniBatchOptimization"
    }
}
```

---

## Integration with Stream Processor

### Translation to StreamPipeline

The physical plan is translated to the existing `StreamPipeline` API.

```rust
/// Execution plan generator
pub struct ExecutionPlanGenerator {
    /// State backend factory
    state_backend_factory: Arc<dyn StateBackendFactory>,

    /// Kafka configuration
    kafka_config: KafkaConfig,
}

impl ExecutionPlanGenerator {
    /// Generate executable stream pipeline from physical plan
    pub fn generate(&self, physical_plan: &PhysicalPlan) -> Result<StreamPipeline> {
        let mut builder = StreamPipelineBuilder::new();

        // Walk physical plan and translate to stream operators
        self.translate_recursive(physical_plan.root(), &mut builder)?;

        builder.build()
    }

    /// Recursively translate physical operators
    fn translate_recursive(
        &self,
        operator: &dyn PhysicalOperator,
        builder: &mut StreamPipelineBuilder,
    ) -> Result<()> {
        match operator.operator_type() {
            PhysicalOperatorType::KafkaSource => {
                let source = operator.downcast_ref::<KafkaSourceExec>()
                    .ok_or_else(|| anyhow!("Invalid operator type"))?;

                builder.with_kafka_source(source.config.clone());
            }

            PhysicalOperatorType::FilterExec => {
                let filter = operator.downcast_ref::<FilterExec>()?;
                let stream_op = filter.to_stream_operator()?;
                builder.with_operator(stream_op);
            }

            PhysicalOperatorType::WindowAggregateExec => {
                let window_agg = operator.downcast_ref::<WindowAggregateExec>()?;

                // Configure window
                match &window_agg.window {
                    WindowSpec::Tumbling { size } => {
                        builder.with_tumbling_window(size.as_millis() as u64);
                    }
                    WindowSpec::Sliding { size, slide } => {
                        builder.with_sliding_window(
                            size.as_millis() as u64,
                            slide.as_millis() as u64,
                        );
                    }
                    WindowSpec::Session { gap } => {
                        builder.with_session_window(gap.as_millis() as u64);
                    }
                    _ => {}
                }

                // Configure aggregation
                let stream_op = window_agg.to_stream_operator()?;
                builder.with_operator(stream_op);
            }

            PhysicalOperatorType::HashJoinExec => {
                let join = operator.downcast_ref::<HashJoinExec>()?;
                let stream_op = join.to_stream_operator()?;
                builder.with_operator(stream_op);
            }

            PhysicalOperatorType::KafkaSink => {
                let sink = operator.downcast_ref::<KafkaSinkExec>()?;
                builder.with_kafka_sink(sink.config.clone());
            }

            _ => {
                return Err(anyhow!("Unsupported operator type: {:?}", operator.operator_type()));
            }
        }

        Ok(())
    }
}
```

### Example: SQL to StreamPipeline

```sql
SELECT
    TUMBLE_START(event_time, INTERVAL '5' MINUTE) as window_start,
    model_id,
    COUNT(*) as event_count,
    AVG(latency) as avg_latency,
    PERCENTILE(latency, 0.95) as p95_latency
FROM kafka_events
WHERE event_type = 'inference'
GROUP BY TUMBLE(event_time, INTERVAL '5' MINUTE), model_id
```

**Logical Plan:**
```
LogicalProject [window_start, model_id, event_count, avg_latency, p95_latency]
  LogicalAggregate [
    group_by: [window, model_id],
    aggregates: [COUNT(*), AVG(latency), PERCENTILE(latency, 95)]
  ]
    LogicalWindow [TUMBLING, 5min]
      LogicalFilter [event_type = 'inference']
        LogicalScan [kafka_events]
```

**After Optimization:**
```
LogicalProject [window_start, model_id, event_count, avg_latency, p95_latency]
  LogicalAggregate [
    group_by: [window, model_id],
    aggregates: [COUNT(*), AVG(latency), PERCENTILE(latency, 95)]
  ]
    LogicalWindow [TUMBLING, 5min]
      LogicalScan [kafka_events, filter: event_type = 'inference']  // Predicate pushed down
```

**Physical Plan:**
```
ProjectExec
  WindowAggregateExec [
    window: Tumbling(5min),
    group_by: [model_id],
    aggregates: [CountAgg, AvgAgg, PercentileAgg(95)],
    state_backend: Redis
  ]
    KafkaSourceExec [
      topic: "events",
      filter: event_type = 'inference',
      watermark: BoundedOutOfOrderness(30s)
    ]
```

**StreamPipeline:**
```rust
StreamPipelineBuilder::new()
    .with_kafka_source(KafkaSourceConfig {
        topic: "events",
        watermark_strategy: WatermarkStrategy::BoundedOutOfOrderness(Duration::from_secs(30)),
        ..Default::default()
    })
    .with_filter(|event| event.event_type == "inference")
    .with_tumbling_window(Duration::from_secs(300))
    .with_key_by(|event| event.model_id.clone())
    .with_aggregate(vec![
        Box::new(CountAggregator::new()),
        Box::new(AvgAggregator::new()),
        Box::new(PercentileAggregator::new(95)),
    ])
    .with_state_backend(StateBackendType::Redis)
    .build()?
```

---

## Execution Strategy

### Query Execution Flow

```rust
/// Query execution coordinator
pub struct QueryExecutor {
    /// Query optimizer
    optimizer: Arc<QueryOptimizer>,

    /// Execution plan generator
    plan_generator: Arc<ExecutionPlanGenerator>,

    /// Running pipelines
    pipelines: Arc<DashMap<QueryId, RunningQuery>>,
}

impl QueryExecutor {
    /// Execute a SQL query
    pub async fn execute(&self, sql: &str) -> Result<QueryHandle> {
        // 1. Parse SQL
        let ast = self.parse_sql(sql)?;

        // 2. Create logical plan
        let logical_plan = self.create_logical_plan(&ast)?;

        // 3. Optimize
        let physical_plan = self.optimizer.optimize(logical_plan)?;

        // 4. Generate execution plan
        let pipeline = self.plan_generator.generate(&physical_plan)?;

        // 5. Execute
        let query_id = QueryId::new();
        let handle = self.start_pipeline(query_id, pipeline).await?;

        Ok(handle)
    }

    /// Start executing a pipeline
    async fn start_pipeline(&self, query_id: QueryId, pipeline: StreamPipeline) -> Result<QueryHandle> {
        let executor = pipeline.create_executor();

        // Spawn execution task
        let handle = tokio::spawn(async move {
            executor.run().await
        });

        let running_query = RunningQuery {
            query_id,
            pipeline,
            handle,
            start_time: Instant::now(),
        };

        self.pipelines.insert(query_id, running_query);

        Ok(QueryHandle { query_id })
    }

    /// Stop a running query
    pub async fn stop_query(&self, query_id: QueryId) -> Result<()> {
        if let Some((_, running)) = self.pipelines.remove(&query_id) {
            running.handle.abort();
        }
        Ok(())
    }
}

/// Handle to a running query
pub struct QueryHandle {
    query_id: QueryId,
}

impl QueryHandle {
    /// Get query results (for bounded queries)
    pub async fn results(&self) -> Result<Vec<RecordBatch>> {
        // Implementation depends on result collection strategy
        todo!()
    }

    /// Stream query results (for unbounded queries)
    pub fn result_stream(&self) -> impl Stream<Item = Result<RecordBatch>> {
        // Implementation depends on result streaming strategy
        todo!()
    }
}
```

---

## Performance Characteristics

### Latency Targets

| Operation | Target Latency | Notes |
|-----------|----------------|-------|
| Query Planning | < 50ms | Simple queries, cached templates |
| Query Planning | < 500ms | Complex queries with joins |
| Optimization | < 100ms | Rule-based optimizations |
| Optimization | < 1s | Cost-based with large plan space |
| First Result | < 100ms | For streaming queries |
| Window Trigger | < 10ms | After watermark advance |

### Throughput Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Events/sec | 10,000+ | Per pipeline |
| Queries/sec | 100+ | Concurrent query planning |
| Aggregations/sec | 1,000+ | Per window |
| State Operations/sec | 100,000+ | Redis backend |

### Memory Usage

| Component | Typical Memory | Peak Memory |
|-----------|----------------|-------------|
| Query Plan | 1-10 MB | Per query |
| Optimizer Memo | 10-100 MB | Complex queries |
| Window State | 100 MB - 1 GB | Per key, depends on window size |
| Join State | 1-10 GB | Depends on join cardinality |

---

## Implementation Roadmap

### Phase 1: Foundation (Weeks 1-2)

**Goal:** Basic query planning infrastructure

- [ ] Define logical operator traits and core types
- [ ] Implement SQL parser integration (sqlparser-rs)
- [ ] Create logical plan builder from AST
- [ ] Implement basic schema and type system
- [ ] Add expression evaluation framework

**Deliverables:**
- Logical plan data structures
- SQL parser integration
- Basic type checking

### Phase 2: Logical Optimization (Weeks 3-4)

**Goal:** Rule-based query optimization

- [ ] Implement optimization rule framework
- [ ] Add predicate pushdown rule
- [ ] Add projection pushdown rule
- [ ] Add constant folding rule
- [ ] Add common subexpression elimination
- [ ] Implement rule application engine

**Deliverables:**
- Optimization rule engine
- Core logical optimization rules
- Rule testing framework

### Phase 3: Physical Planning (Weeks 5-6)

**Goal:** Physical plan generation

- [ ] Define physical operator traits
- [ ] Implement physical operators for each logical operator
- [ ] Add operator property tracking (partitioning, ordering)
- [ ] Create physical plan enumeration
- [ ] Implement basic cost model

**Deliverables:**
- Physical operator implementations
- Plan enumeration logic
- Basic cost estimation

### Phase 4: Cost-Based Optimization (Weeks 7-8)

**Goal:** Cost-based plan selection

- [ ] Implement statistics collection framework
- [ ] Add cardinality estimation
- [ ] Implement cost model for each operator
- [ ] Add join ordering optimization
- [ ] Create plan comparison and selection logic

**Deliverables:**
- Cost model implementation
- Cardinality estimator
- Statistics provider interface

### Phase 5: Streaming Optimizations (Weeks 9-10)

**Goal:** Streaming-specific optimizations

- [ ] Implement watermark propagation analysis
- [ ] Add window coalescing optimization
- [ ] Implement state minimization rules
- [ ] Add local-global aggregation optimization
- [ ] Implement mini-batch optimization

**Deliverables:**
- Streaming optimization rules
- Watermark propagation analyzer
- State management optimizations

### Phase 6: Execution Integration (Weeks 11-12)

**Goal:** Integration with stream processor

- [ ] Implement physical-to-pipeline translation
- [ ] Add query execution coordinator
- [ ] Create result collection/streaming
- [ ] Implement query lifecycle management
- [ ] Add execution metrics and monitoring

**Deliverables:**
- Execution plan generator
- Query executor
- End-to-end SQL execution

### Phase 7: Advanced Features (Weeks 13-14)

**Goal:** Advanced SQL features

- [ ] Implement join support (hash join, merge join)
- [ ] Add subquery support
- [ ] Implement set operations (UNION, INTERSECT, EXCEPT)
- [ ] Add window functions (OVER clause)
- [ ] Implement temporal joins for streaming

**Deliverables:**
- Join operators
- Subquery rewriting
- Advanced SQL features

### Phase 8: Testing & Optimization (Weeks 15-16)

**Goal:** Validation and performance tuning

- [ ] Comprehensive unit tests for all components
- [ ] Integration tests for SQL queries
- [ ] Performance benchmarks
- [ ] Query plan visualization
- [ ] Documentation and examples

**Deliverables:**
- Test suite
- Benchmarks
- Documentation
- Query explain plan

---

## Appendix: SQL Feature Coverage

### Phase 1 Support

```sql
-- Basic SELECT with filtering
SELECT column1, column2
FROM table1
WHERE column1 > 100

-- Simple aggregation
SELECT COUNT(*), AVG(column1)
FROM table1

-- GROUP BY
SELECT column1, COUNT(*)
FROM table1
GROUP BY column1

-- Tumbling windows
SELECT
    TUMBLE_START(event_time, INTERVAL '5' MINUTE),
    COUNT(*)
FROM events
GROUP BY TUMBLE(event_time, INTERVAL '5' MINUTE)
```

### Phase 2 Support

```sql
-- Sliding windows
SELECT
    HOP_START(event_time, INTERVAL '5' MINUTE, INTERVAL '10' MINUTE),
    COUNT(*)
FROM events
GROUP BY HOP(event_time, INTERVAL '5' MINUTE, INTERVAL '10' MINUTE)

-- Session windows
SELECT
    SESSION_START(event_time, INTERVAL '30' SECOND),
    COUNT(*)
FROM events
GROUP BY SESSION(event_time, INTERVAL '30' SECOND)

-- ORDER BY and LIMIT
SELECT * FROM events
ORDER BY event_time DESC
LIMIT 100
```

### Phase 3 Support

```sql
-- JOINs
SELECT e.*, m.model_name
FROM events e
JOIN models m ON e.model_id = m.id

-- Subqueries
SELECT * FROM (
    SELECT model_id, COUNT(*) as cnt
    FROM events
    GROUP BY model_id
) WHERE cnt > 100

-- UNION
SELECT model_id FROM events_v1
UNION
SELECT model_id FROM events_v2
```

---

## References

### Academic Papers

1. **Graefe, G. (1995).** "The Cascades Framework for Query Optimization." IEEE Data Engineering Bulletin.
2. **Chaudhuri, S. (1998).** "An Overview of Query Optimization in Relational Systems." PODS.
3. **Begoli, E., et al. (2019).** "Apache Calcite: A Foundational Framework for Optimized Query Processing Over Heterogeneous Data Sources." SIGMOD.

### Industry Implementations

1. **Apache Calcite**: https://calcite.apache.org/
2. **Apache Flink SQL**: https://nightlies.apache.org/flink/flink-docs-stable/docs/dev/table/sql/
3. **ksqlDB**: https://docs.ksqldb.io/en/latest/

### Existing Codebase

1. Stream Processor: `/workspaces/llm-auto-optimizer/crates/processor`
2. Window Management: `/workspaces/llm-auto-optimizer/crates/processor/src/window`
3. Aggregation: `/workspaces/llm-auto-optimizer/crates/processor/src/aggregation`
4. State Backend: `/workspaces/llm-auto-optimizer/crates/processor/src/state`

---

**End of Document**
