# SQL Interface - User Documentation

**LLM Auto Optimizer - Complete SQL Documentation**
**Version:** 1.0
**Last Updated:** 2025-11-10

---

## Welcome

Welcome to the comprehensive SQL Interface documentation for LLM Auto Optimizer! This documentation suite provides everything you need to use SQL for stream processing, real-time analytics, and LLM performance optimization.

---

## Documentation Overview

### 📚 Core Documentation (5 Documents)

#### 1. [SQL Reference Guide](SQL_REFERENCE_GUIDE.md) (32KB)
**Complete SQL syntax and language reference**

Topics covered:
- Data types (primitives, complex types, streaming types)
- DDL statements (CREATE STREAM, CREATE TABLE, DROP, ALTER)
- DML statements (SELECT, INSERT, UPDATE, DELETE)
- Query clauses (FROM, WHERE, GROUP BY, HAVING, ORDER BY, LIMIT)
- Window functions (TUMBLE, HOP, SESSION, CUMULATE)
- Aggregation functions (COUNT, SUM, AVG, MIN, MAX, STDDEV, PERCENTILE)
- Scalar functions (string, math, date/time, conditional, JSON)
- Join operations (INNER, LEFT, RIGHT, FULL, time-bounded)
- Time semantics (event time, processing time, watermarks, emit strategies)
- System tables (metadata and monitoring)

**Use this when:** You need to look up SQL syntax, function signatures, or language features.

---

#### 2. [SQL Examples](SQL_EXAMPLES.md) (29KB)
**56+ practical, real-world query examples**

Categories:
- Basic queries (Examples 1-4)
- Filtering and projection (Examples 5-12)
- Simple aggregations (Examples 13-20)
- Tumbling window aggregations (Examples 21-26)
- Sliding window aggregations (Examples 27-30)
- Session window aggregations (Examples 31-34)
- Joins (Examples 35-39)
- Advanced analytics (Examples 40-45)
- Real-world use cases (Examples 46-50)
- Optimization and cost analysis (Examples 51-56)

**Example highlights:**
- SLA violation detection
- A/B test analysis
- Cost optimization opportunities
- Anomaly detection with Z-score
- User churn prediction
- Multi-objective model ranking
- Budget forecasting
- Model drift detection

**Use this when:** You want to see how to solve specific problems or need inspiration for your queries.

---

#### 3. [SQL Tutorial](SQL_TUTORIAL.md) (28KB)
**Step-by-step learning guide with 10 progressive lessons**

Lesson structure:
1. **Creating Your First Stream** - Define streams, configure watermarks
2. **Basic Queries** - SELECT, WHERE, projections
3. **Filtering and Transforming Data** - Functions, CASE statements
4. **Simple Aggregations** - COUNT, AVG, SUM, percentiles
5. **Tumbling Windows** - Fixed-size non-overlapping windows
6. **Sliding Windows** - Overlapping windows for trend analysis
7. **Session Windows** - User session analytics
8. **Joining Streams** - Enriching data with joins
9. **Advanced Patterns** - Multi-level aggregation, deduplication
10. **Production Deployment** - Monitoring, alerting, fault tolerance

Each lesson includes:
- Learning objectives
- Theory/concepts
- Step-by-step examples
- Exercises with solutions
- Real-world use cases

**Use this when:** You're learning SQL streaming from scratch or want structured learning.

---

#### 4. [SQL Best Practices](SQL_BEST_PRACTICES.md) (20KB)
**Performance optimization and production guidelines**

Sections:
1. **Query Optimization**
   - Filter early, filter often
   - Use approximate aggregations
   - Limit unbounded queries
   - Project only required columns
   - Conditional aggregation patterns

2. **State Management**
   - Choosing state backends (Memory, Sled, Postgres, Redis)
   - State TTL configuration
   - Checkpointing strategies
   - Incremental aggregations

3. **Window Sizing**
   - Window size guidelines by use case
   - Sliding window overlap ratios
   - Session gap sizing based on data

4. **Join Strategies**
   - Time-bounded joins
   - Join order optimization
   - Lookup joins for dimension tables

5. **Performance Tuning**
   - Parallelism configuration
   - Batch size tuning
   - Buffer configuration
   - Watermark strategy selection

6. **Troubleshooting**
   - Out of memory (OOM) issues
   - High latency problems
   - Data skew handling
   - Late event handling

7. **Production Checklist**
   - Pre-deployment checklist
   - Configuration review
   - Monitoring setup
   - Alerting rules

**Use this when:** You need to optimize queries, tune performance, or troubleshoot issues.

---

#### 5. [SQL API Reference](SQL_API_REFERENCE.md) (26KB)
**Rust API for embedding SQL engine**

API documentation:
- **SqlEngine** - Main execution engine
- **SqlEngineConfig** - Configuration options
- **QueryResult** - Result handling
- **Row** - Row-level access
- **SqlValue** - Type system
- **Error handling** - SqlError types

Integration examples:
1. **Embedded Stream Processing** - Complete streaming pipeline
2. **REST API Integration** - HTTP query endpoint with Axum
3. **Metrics Export** - Prometheus integration

Advanced topics:
- Custom state backends
- Query planning and optimization
- Catalog management
- Performance monitoring

**Use this when:** You're integrating the SQL engine into Rust applications.

---

## Quick Start Paths

### 🚀 I Want to Get Started Fast

1. Read the [Getting Started](#getting-started) section below
2. Follow [SQL Tutorial - Lesson 1](SQL_TUTORIAL.md#lesson-1-creating-your-first-stream)
3. Try [SQL Examples 1-10](SQL_EXAMPLES.md#basic-queries)
4. Build your first dashboard using [Example 56](SQL_EXAMPLES.md#example-56-real-time-dashboard-query)

### 📊 I Want to Build Real-Time Analytics

1. [SQL Tutorial - Lesson 5 (Tumbling Windows)](SQL_TUTORIAL.md#lesson-5-tumbling-windows)
2. [SQL Examples 21-26 (Window Aggregations)](SQL_EXAMPLES.md#tumbling-window-aggregations)
3. [Best Practices - Window Sizing](SQL_BEST_PRACTICES.md#window-sizing)
4. [Example 46 (SLA Monitoring)](SQL_EXAMPLES.md#example-46-sla-violation-detection)

### 💰 I Want to Optimize LLM Costs

1. [Example 48 (Cost Optimization)](SQL_EXAMPLES.md#example-48-cost-optimization-opportunities)
2. [Example 51 (Multi-Objective Ranking)](SQL_EXAMPLES.md#example-51-multi-objective-ranking)
3. [Example 52 (Cost Efficiency)](SQL_EXAMPLES.md#example-52-cost-per-quality-point)
4. [Example 53 (Budget Forecasting)](SQL_EXAMPLES.md#example-53-budget-forecasting)

### 🔧 I Want to Integrate SQL into My Application

1. [SQL API Reference - Getting Started](SQL_API_REFERENCE.md#getting-started)
2. [Integration Example 1 (Embedded)](SQL_API_REFERENCE.md#example-1-embedded-stream-processing)
3. [Integration Example 2 (REST API)](SQL_API_REFERENCE.md#example-2-rest-api-integration)
4. [Error Handling Patterns](SQL_API_REFERENCE.md#error-handling)

### 🐛 I Need to Troubleshoot Issues

1. [Best Practices - Troubleshooting](SQL_BEST_PRACTICES.md#troubleshooting)
2. [Performance Tuning Guide](SQL_BEST_PRACTICES.md#performance-tuning)
3. [Production Checklist](SQL_BEST_PRACTICES.md#production-checklist)
4. [API Reference - Metrics](SQL_API_REFERENCE.md#metrics-and-monitoring)

---

## Getting Started

### Prerequisites

- **Rust 1.75+** - [Install via rustup](https://rustup.rs/)
- **PostgreSQL 15+** or Redis (for production state)
- **Kafka** (for streaming data sources)
- **Docker** (optional, for local development)

### Installation

```bash
# Clone the repository
git clone https://github.com/globalbusinessadvisors/llm-auto-optimizer.git
cd llm-auto-optimizer

# Build with SQL support
cargo build --release --features sql

# Start SQL server
./target/release/llm-optimizer sql-server --port 8080
```

### First Query

```bash
# Connect to SQL interface
llm-optimizer sql

# Or use any PostgreSQL-compatible client
psql -h localhost -p 8080 -U optimizer
```

```sql
-- Create your first stream
CREATE STREAM feedback_events (
    event_id VARCHAR,
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
);

-- Run your first query
SELECT
    model_name,
    COUNT(*) as requests,
    AVG(latency_ms) as avg_latency,
    AVG(rating) as avg_rating
FROM feedback_events
WHERE event_time >= NOW() - INTERVAL '1' HOUR
GROUP BY model_name;
```

---

## Documentation Map

### By Experience Level

**Beginner:**
1. [SQL Tutorial](SQL_TUTORIAL.md) - Lessons 1-4
2. [SQL Examples](SQL_EXAMPLES.md) - Examples 1-20
3. [SQL Reference](SQL_REFERENCE_GUIDE.md) - Basic syntax

**Intermediate:**
1. [SQL Tutorial](SQL_TUTORIAL.md) - Lessons 5-8
2. [SQL Examples](SQL_EXAMPLES.md) - Examples 21-45
3. [Best Practices](SQL_BEST_PRACTICES.md) - Optimization basics

**Advanced:**
1. [SQL Tutorial](SQL_TUTORIAL.md) - Lessons 9-10
2. [SQL Examples](SQL_EXAMPLES.md) - Examples 46-56
3. [Best Practices](SQL_BEST_PRACTICES.md) - Full guide
4. [SQL API Reference](SQL_API_REFERENCE.md) - Advanced integration

### By Topic

**Stream Processing:**
- [Reference - Window Functions](SQL_REFERENCE_GUIDE.md#window-functions)
- [Tutorial - Lessons 5-7](SQL_TUTORIAL.md#lesson-5-tumbling-windows)
- [Examples 21-34](SQL_EXAMPLES.md#tumbling-window-aggregations)

**Performance:**
- [Best Practices - Query Optimization](SQL_BEST_PRACTICES.md#query-optimization)
- [Best Practices - State Management](SQL_BEST_PRACTICES.md#state-management)
- [Best Practices - Performance Tuning](SQL_BEST_PRACTICES.md#performance-tuning)

**Cost Optimization:**
- [Examples 48-53](SQL_EXAMPLES.md#optimization-and-cost-analysis)
- [Tutorial - Lesson 4](SQL_TUTORIAL.md#lesson-4-simple-aggregations)

**Integration:**
- [API Reference - Full Guide](SQL_API_REFERENCE.md)
- [Integration Examples](SQL_API_REFERENCE.md#integration-examples)

---

## Common Use Cases

### Real-Time Monitoring Dashboard

**Documents:** [Example 56](SQL_EXAMPLES.md#example-56-real-time-dashboard-query), [Tutorial Lesson 10](SQL_TUTORIAL.md#lesson-10-production-deployment)

```sql
SELECT
    model_name,
    COUNT(*) as requests,
    AVG(latency_ms) as avg_latency,
    APPROX_PERCENTILE(latency_ms, 0.95) as p95_latency,
    SUM(cost_usd) as total_cost,
    AVG(rating) as avg_rating,
    COUNT(CASE WHEN error_code IS NOT NULL THEN 1 END) * 100.0 / COUNT(*) as error_rate
FROM feedback_events
WHERE event_time >= NOW() - INTERVAL '5' MINUTE
GROUP BY model_name;
```

### SLA Violation Alerts

**Documents:** [Example 46](SQL_EXAMPLES.md#example-46-sla-violation-detection), [Best Practices - Alerting](SQL_BEST_PRACTICES.md#74-alerting-rules)

```sql
SELECT
    model_name,
    TUMBLE_START(event_time, INTERVAL '5' MINUTE) as window_start,
    APPROX_PERCENTILE(latency_ms, 0.95) as p95_latency
FROM feedback_events
GROUP BY model_name, TUMBLE(event_time, INTERVAL '5' MINUTE)
HAVING APPROX_PERCENTILE(latency_ms, 0.95) > 3000;
```

### Cost Analysis

**Documents:** [Example 48](SQL_EXAMPLES.md#example-48-cost-optimization-opportunities), [Example 52](SQL_EXAMPLES.md#example-52-cost-per-quality-point)

```sql
SELECT
    model_name,
    SUM(cost_usd) as total_cost,
    AVG(rating) as avg_rating,
    SUM(cost_usd) / NULLIF(AVG(rating), 0) as cost_per_quality_point
FROM feedback_events
WHERE event_time >= NOW() - INTERVAL '7' DAY
GROUP BY model_name
ORDER BY cost_per_quality_point;
```

### User Session Analytics

**Documents:** [Example 31](SQL_EXAMPLES.md#example-31-user-session-analysis), [Tutorial Lesson 7](SQL_TUTORIAL.md#lesson-7-session-windows)

```sql
SELECT
    user_id,
    SESSION_START(event_time, INTERVAL '30' MINUTE) as session_start,
    COUNT(*) as interactions,
    SUM(cost_usd) as session_cost,
    AVG(rating) as avg_rating
FROM feedback_events
GROUP BY user_id, SESSION(event_time, INTERVAL '30' MINUTE);
```

---

## Performance Guidelines

### Query Performance

| Query Type | Expected Latency | Throughput |
|------------|------------------|------------|
| Simple SELECT | <10ms | 100k+ events/sec |
| Filtered query | 10-50ms | 50k+ events/sec |
| Aggregation | 50-200ms | 10k+ events/sec |
| Windowed aggregation | 100-500ms | 5k+ events/sec |
| Multi-stream join | 200ms-2s | 1k+ events/sec |

### State Size Estimates

| Operation | State per Key |
|-----------|---------------|
| COUNT | 8 bytes |
| SUM | 8 bytes |
| AVG | 16 bytes (count + sum) |
| STDDEV | 24 bytes |
| PERCENTILE (approx) | ~1KB (t-digest) |
| Session window | Variable (event history) |

**See [Best Practices - State Management](SQL_BEST_PRACTICES.md#state-management) for details.**

---

## FAQ

### General Questions

**Q: Can I use standard SQL tools?**
A: Yes! The SQL interface is PostgreSQL wire-protocol compatible, so you can use psql, DBeaver, DataGrip, etc.

**Q: What's the difference between streams and tables?**
A: Streams are unbounded (continuous) data sources. Tables are bounded snapshots or changelog streams.

**Q: How do I handle late-arriving events?**
A: Configure watermarks and allowed lateness. See [Reference - Watermarks](SQL_REFERENCE_GUIDE.md#watermarks).

### Performance Questions

**Q: How do I optimize slow queries?**
A: See [Best Practices - Troubleshooting](SQL_BEST_PRACTICES.md#troubleshooting).

**Q: What state backend should I use?**
A: Memory (dev), Sled (small prod), Postgres (large/distributed), Redis (high performance). See [Best Practices - State Management](SQL_BEST_PRACTICES.md#state-management).

**Q: How do I prevent out-of-memory errors?**
A: Configure state TTL, use appropriate window sizes, and enable checkpointing. See [Best Practices - OOM](SQL_BEST_PRACTICES.md#61-out-of-memory-oom).

---

## Additional Resources

### Technical Documentation

- [Stream Processing Architecture](STREAM_PROCESSING_WINDOWING_SPECIFICATION.md)
- [Windowing Aggregation Architecture](WINDOWING_AGGREGATION_ARCHITECTURE.md)
- [Distributed State Guide](DISTRIBUTED_STATE_GUIDE.md)
- [Metrics Export Architecture](METRICS_EXPORT_ARCHITECTURE.md)

### Implementation Documentation

- [SQL Implementation Plan](SQL_IMPLEMENTATION_PLAN.md)
- [SQL Parser Architecture](SQL_PARSER_ARCHITECTURE.md)
- [SQL Query Planner](SQL_QUERY_PLANNER_ARCHITECTURE.md)

### Community

- **GitHub**: https://github.com/globalbusinessadvisors/llm-auto-optimizer
- **Issues**: https://github.com/globalbusinessadvisors/llm-auto-optimizer/issues
- **Discussions**: https://github.com/globalbusinessadvisors/llm-auto-optimizer/discussions
- **Documentation**: https://docs.llmdevops.dev

---

## Document Statistics

| Document | Size | Topics | Examples | Exercises |
|----------|------|--------|----------|-----------|
| SQL Reference Guide | 32KB | 12 | 20+ | - |
| SQL Examples | 29KB | 10 | 56 | - |
| SQL Tutorial | 28KB | 10 | 40+ | 10 |
| SQL Best Practices | 20KB | 7 | 30+ | - |
| SQL API Reference | 26KB | 8 | 15+ | - |
| **Total** | **135KB** | **47** | **161+** | **10** |

---

## Changelog

### Version 1.0 (2025-11-10)
- Initial release
- Complete SQL reference guide
- 56 practical examples
- 10-lesson tutorial
- Best practices guide
- Rust API reference

---

## License

This documentation is part of the LLM Auto Optimizer project, licensed under Apache 2.0.

---

**Happy Streaming with SQL!** 🚀
