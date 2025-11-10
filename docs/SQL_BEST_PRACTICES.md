# SQL Best Practices

**LLM Auto Optimizer - SQL Interface**
**Version:** 1.0
**Last Updated:** 2025-11-10

---

## Table of Contents

1. [Query Optimization](#query-optimization)
2. [State Management](#state-management)
3. [Window Sizing](#window-sizing)
4. [Join Strategies](#join-strategies)
5. [Performance Tuning](#performance-tuning)
6. [Troubleshooting](#troubleshooting)
7. [Production Checklist](#production-checklist)

---

## Query Optimization

### 1.1 Filter Early, Filter Often

**Bad Practice:**

```sql
-- Aggregating first, then filtering
SELECT
    model_name,
    avg_latency
FROM (
    SELECT
        model_name,
        AVG(latency_ms) as avg_latency
    FROM feedback_events
    GROUP BY model_name
)
WHERE avg_latency > 1000;
```

**Good Practice:**

```sql
-- Filter before aggregation when possible
SELECT
    model_name,
    AVG(latency_ms) as avg_latency
FROM feedback_events
WHERE latency_ms > 500  -- Pre-filter to reduce data
GROUP BY model_name
HAVING AVG(latency_ms) > 1000;
```

**Why:** Reduces the amount of data processed in aggregations.

---

### 1.2 Use Approximate Aggregations for Large Datasets

**Bad Practice:**

```sql
-- Exact percentile (slow for large datasets)
SELECT
    model_name,
    PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY latency_ms) as p95_latency
FROM feedback_events
GROUP BY model_name;
```

**Good Practice:**

```sql
-- Approximate percentile (much faster, 99.9% accurate)
SELECT
    model_name,
    APPROX_PERCENTILE(latency_ms, 0.95) as p95_latency
FROM feedback_events
GROUP BY model_name;
```

**Why:** `APPROX_PERCENTILE` uses t-digest algorithm with O(1) space vs O(n) for exact percentiles.

**Performance Impact:**
- Exact: 10-100x slower for large datasets
- Approximate: Constant memory, ~1% error margin

---

### 1.3 Limit Unbounded Queries

**Bad Practice:**

```sql
-- Unbounded continuous query
SELECT * FROM feedback_events;
```

**Good Practice:**

```sql
-- Time-bounded query
SELECT * FROM feedback_events
WHERE event_time >= NOW() - INTERVAL '1' HOUR;

-- Or use windows
SELECT
    model_name,
    TUMBLE_START(event_time, INTERVAL '5' MINUTE) as window_start,
    COUNT(*) as events
FROM feedback_events
GROUP BY
    model_name,
    TUMBLE(event_time, INTERVAL '5' MINUTE);
```

**Why:** Prevents unbounded state growth and OOM errors.

---

### 1.4 Project Only Required Columns

**Bad Practice:**

```sql
-- Selecting all columns
SELECT * FROM feedback_events
WHERE latency_ms > 5000;
```

**Good Practice:**

```sql
-- Select only needed columns
SELECT
    event_id,
    model_name,
    latency_ms,
    event_time
FROM feedback_events
WHERE latency_ms > 5000;
```

**Why:** Reduces network I/O and memory usage by 50-80%.

---

### 1.5 Use CASE Instead of Multiple Aggregations

**Bad Practice:**

```sql
-- Multiple passes over data
SELECT
    model_name,
    (SELECT COUNT(*) FROM feedback_events WHERE latency_ms < 1000 AND model_name = f.model_name) as fast,
    (SELECT COUNT(*) FROM feedback_events WHERE latency_ms BETWEEN 1000 AND 3000 AND model_name = f.model_name) as medium,
    (SELECT COUNT(*) FROM feedback_events WHERE latency_ms > 3000 AND model_name = f.model_name) as slow
FROM feedback_events f
GROUP BY model_name;
```

**Good Practice:**

```sql
-- Single pass with conditional aggregation
SELECT
    model_name,
    COUNT(CASE WHEN latency_ms < 1000 THEN 1 END) as fast,
    COUNT(CASE WHEN latency_ms BETWEEN 1000 AND 3000 THEN 1 END) as medium,
    COUNT(CASE WHEN latency_ms > 3000 THEN 1 END) as slow
FROM feedback_events
GROUP BY model_name;
```

**Why:** Single pass vs N passes over the data.

---

### 1.6 Avoid SELECT DISTINCT When Possible

**Bad Practice:**

```sql
-- Expensive deduplication
SELECT DISTINCT user_id
FROM feedback_events;
```

**Good Practice:**

```sql
-- Use GROUP BY or aggregation
SELECT user_id
FROM feedback_events
GROUP BY user_id;

-- Or COUNT(DISTINCT) if needed
SELECT COUNT(DISTINCT user_id) as unique_users
FROM feedback_events;
```

**Why:** GROUP BY is often optimized better than DISTINCT.

---

## State Management

### 2.1 Choose Appropriate State Backend

**Development:**

```sql
SET 'state.backend' = 'memory';
```

**Production (Small State < 100MB):**

```sql
SET 'state.backend' = 'sled';
SET 'state.sled.path' = '/var/lib/optimizer/state';
```

**Production (Large State or Distributed):**

```sql
SET 'state.backend' = 'postgres';
SET 'state.postgres.url' = 'postgresql://localhost:5432/optimizer';
SET 'state.postgres.pool.size' = '20';
```

**Or Redis for High Performance:**

```sql
SET 'state.backend' = 'redis';
SET 'state.redis.url' = 'redis://localhost:6379';
SET 'state.redis.pool.size' = '50';
```

### 2.2 Configure State TTL

**Bad Practice:**

```sql
-- No state cleanup (state grows unbounded)
SELECT
    user_id,
    COUNT(*) as interaction_count
FROM feedback_events
GROUP BY user_id;
```

**Good Practice:**

```sql
-- State cleanup after 7 days of inactivity
SELECT
    user_id,
    COUNT(*) as interaction_count
FROM feedback_events
GROUP BY user_id
WITH (
    'state.ttl' = '7d',
    'state.cleanup.strategy' = 'incremental'
);
```

**State Size Estimation:**
- Average key size: 50 bytes (user_id)
- Average value size: 100 bytes (aggregation state)
- 1M unique users = ~150MB state
- With TTL: ~50MB (assuming 30% active users)

---

### 2.3 Enable Checkpointing

```sql
SET 'state.checkpoints.enabled' = 'true';
SET 'state.checkpoints.interval' = '60s';  -- Every 1 minute
SET 'state.checkpoints.min-pause' = '10s';  -- Min time between checkpoints
SET 'state.checkpoints.timeout' = '10m';    -- Checkpoint timeout
```

**Checkpoint Interval Guidelines:**
- High throughput (>10k events/sec): 30-60 seconds
- Medium throughput (1k-10k events/sec): 1-5 minutes
- Low throughput (<1k events/sec): 5-15 minutes

---

### 2.4 Use Incremental Aggregations

**Bad Practice:**

```sql
-- Recompute entire aggregation on every event
SELECT
    model_name,
    AVG(latency_ms) as avg_latency
FROM feedback_events
GROUP BY model_name;
```

**Good Practice:**

```sql
-- Use incremental aggregators (built-in optimization)
SELECT
    model_name,
    AVG(latency_ms) as avg_latency,  -- Incremental avg
    STDDEV(latency_ms) as stddev,    -- Incremental stddev
    COUNT(*) as count                 -- Incremental count
FROM feedback_events
GROUP BY model_name;
```

**State Size Comparison:**
- Incremental avg: 24 bytes (count + sum)
- Full history: N * 8 bytes (all values)

---

## Window Sizing

### 3.1 Choose Appropriate Window Size

**Guidelines:**

| Use Case | Window Type | Size | Slide |
|----------|-------------|------|-------|
| Real-time alerts | Tumbling | 1-5 min | N/A |
| Dashboard metrics | Tumbling | 5-15 min | N/A |
| Smooth trends | Sliding | 15-30 min | 1-5 min |
| Anomaly detection | Sliding | 10-60 min | 1-10 min |
| User sessions | Session | 15-60 min gap | N/A |
| Daily reports | Tumbling | 1 day | N/A |

**Bad Practice:**

```sql
-- Window too small (excessive overhead)
SELECT
    model_name,
    TUMBLE_START(event_time, INTERVAL '10' SECOND) as window_start,
    AVG(latency_ms) as avg_latency
FROM feedback_events
GROUP BY
    model_name,
    TUMBLE(event_time, INTERVAL '10' SECOND);
```

**Good Practice:**

```sql
-- Appropriate window size
SELECT
    model_name,
    TUMBLE_START(event_time, INTERVAL '5' MINUTE) as window_start,
    AVG(latency_ms) as avg_latency
FROM feedback_events
GROUP BY
    model_name,
    TUMBLE(event_time, INTERVAL '5' MINUTE);
```

**Why:** 10-second windows create 360 windows/hour vs 12 windows/hour for 5-minute windows.

---

### 3.2 Sliding Window Overlap Ratio

**Bad Practice:**

```sql
-- 99% overlap (slide 1 min, size 100 min)
-- Each event appears in 100 windows!
SELECT
    model_name,
    HOP_START(event_time, INTERVAL '1' MINUTE, INTERVAL '100' MINUTE) as window_start,
    AVG(latency_ms) as avg_latency
FROM feedback_events
GROUP BY
    model_name,
    HOP(event_time, INTERVAL '1' MINUTE, INTERVAL '100' MINUTE);
```

**Good Practice:**

```sql
-- 66% overlap (slide 5 min, size 15 min)
-- Each event appears in 3 windows
SELECT
    model_name,
    HOP_START(event_time, INTERVAL '5' MINUTE, INTERVAL '15' MINUTE) as window_start,
    AVG(latency_ms) as avg_latency
FROM feedback_events
GROUP BY
    model_name,
    HOP(event_time, INTERVAL '5' MINUTE, INTERVAL '15' MINUTE);
```

**State Multiplier:**
- Overlap 50%: State × 2
- Overlap 66%: State × 3
- Overlap 90%: State × 10
- Overlap 99%: State × 100

**Recommended Overlap:** 50-75% for most use cases.

---

### 3.3 Session Window Gap Sizing

**User Behavior Analysis:**

```sql
-- Analyze actual gaps to choose optimal timeout
WITH gaps AS (
    SELECT
        user_id,
        event_time - LAG(event_time) OVER (PARTITION BY user_id ORDER BY event_time) as gap
    FROM feedback_events
)
SELECT
    PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY EXTRACT(EPOCH FROM gap) / 60) as median_gap_minutes,
    PERCENTILE_CONT(0.75) WITHIN GROUP (ORDER BY EXTRACT(EPOCH FROM gap) / 60) as p75_gap_minutes,
    PERCENTILE_CONT(0.90) WITHIN GROUP (ORDER BY EXTRACT(EPOCH FROM gap) / 60) as p90_gap_minutes,
    PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY EXTRACT(EPOCH FROM gap) / 60) as p95_gap_minutes
FROM gaps
WHERE gap IS NOT NULL;
```

**Guideline:** Set session gap to p75-p90 of actual gaps.

---

## Join Strategies

### 4.1 Time-Bound All Stream-to-Stream Joins

**Bad Practice:**

```sql
-- Unbounded join (infinite state!)
SELECT
    f.event_id,
    p.metric_value
FROM feedback_events f
JOIN performance_metrics p
ON f.model_id = p.model_id;
```

**Good Practice:**

```sql
-- Time-bounded join
SELECT
    f.event_id,
    p.metric_value
FROM feedback_events f
JOIN performance_metrics p
ON f.model_id = p.model_id
AND f.event_time BETWEEN p.timestamp - INTERVAL '1' MINUTE
                     AND p.timestamp + INTERVAL '1' MINUTE;
```

**State Size:**
- Unbounded: Infinite (OOM guaranteed)
- 1-minute window: ~60 seconds × throughput

---

### 4.2 Join Order Matters

**Bad Practice:**

```sql
-- Large stream first
SELECT *
FROM feedback_events f  -- 10M events/day
JOIN model_configs m    -- 100 rows
ON f.model_id = m.model_id;
```

**Good Practice:**

```sql
-- Small table first (broadcast join)
SELECT *
FROM model_configs m    -- 100 rows (broadcast)
JOIN feedback_events f  -- 10M events/day
ON m.model_id = f.model_id;
```

**Or explicitly specify:**

```sql
SELECT /*+ BROADCAST(m) */ *
FROM feedback_events f
JOIN model_configs m
ON f.model_id = m.model_id;
```

---

### 4.3 Use Lookup Joins for Dimension Tables

**Good Practice:**

```sql
-- Enable lookup join for static dimension table
CREATE TABLE model_configs (
    model_id VARCHAR PRIMARY KEY,
    model_name VARCHAR,
    temperature DOUBLE
)
WITH (
    'connector' = 'postgres',
    'table-name' = 'model_configs',
    'lookup.cache.enabled' = 'true',
    'lookup.cache.ttl' = '1h',
    'lookup.cache.max-rows' = '10000'
);

SELECT
    f.event_id,
    m.model_name,
    m.temperature
FROM feedback_events f
LEFT JOIN model_configs FOR SYSTEM_TIME AS OF f.event_time AS m
ON f.model_id = m.model_id;
```

**Benefits:**
- Cached lookups (microsecond latency)
- Automatic cache refresh
- Bounded memory usage

---

## Performance Tuning

### 5.1 Parallelism Configuration

```sql
-- Set parallelism based on CPU cores
SET 'parallelism.default' = '8';  -- 8 cores

-- For I/O-bound workloads
SET 'parallelism.default' = '16';  -- 2× cores

-- Per-query override
SELECT /*+ PARALLELISM(4) */
    model_name,
    COUNT(*) as events
FROM feedback_events
GROUP BY model_name;
```

**Guidelines:**
- CPU-bound: 0.5× to 1× CPU cores
- I/O-bound: 1× to 2× CPU cores
- Mixed: 1× CPU cores

---

### 5.2 Batch Size Tuning

```sql
SET 'execution.batch.size' = '1000';  -- Events per batch
SET 'execution.batch.timeout' = '100ms';  -- Max wait time
```

**Latency vs Throughput Trade-off:**

| Batch Size | Latency | Throughput |
|------------|---------|------------|
| 10 | 5ms | Low |
| 100 | 20ms | Medium |
| 1000 | 100ms | High |
| 10000 | 500ms | Very High |

**Recommendation:**
- Real-time alerts: 10-100
- Dashboard updates: 100-1000
- Batch analytics: 1000-10000

---

### 5.3 Buffer Configuration

```sql
SET 'buffer.memory.max' = '512MB';  -- Total buffer memory
SET 'buffer.spill.threshold' = '0.8';  -- Spill to disk at 80%
SET 'buffer.spill.path' = '/tmp/optimizer-spill';
```

**Memory Budget Calculation:**

```
Total Memory = JVM Heap Size
Buffer Memory = 0.3 × Total Memory (30%)
State Memory = 0.4 × Total Memory (40%)
Overhead = 0.3 × Total Memory (30%)
```

**Example (8GB heap):**
- Buffer: 2.4 GB
- State: 3.2 GB
- Overhead: 2.4 GB

---

### 5.4 Watermark Strategy Selection

**Low-latency (strict ordering):**

```sql
WATERMARK FOR event_time AS event_time - INTERVAL '1' SECOND
```

**Balanced:**

```sql
WATERMARK FOR event_time AS event_time - INTERVAL '30' SECOND
```

**High out-of-order tolerance:**

```sql
WATERMARK FOR event_time AS event_time - INTERVAL '5' MINUTE
```

**Trade-offs:**

| Allowed Lateness | Late Events Lost | Window Delay | State Size |
|------------------|------------------|--------------|------------|
| 1 second | 5-10% | Low | Small |
| 30 seconds | 1-2% | Medium | Medium |
| 5 minutes | <0.1% | High | Large |

---

### 5.5 Monitoring Query Performance

```sql
-- Check query statistics
SELECT
    job_id,
    query,
    records_processed,
    records_per_second,
    avg_latency_ms,
    p95_latency_ms,
    errors
FROM SYSTEM.JOBS
ORDER BY records_per_second DESC;

-- Check backpressure
SELECT
    job_id,
    operator_name,
    backpressure_ratio  -- 0.0-1.0
FROM SYSTEM.OPERATOR_STATS
WHERE backpressure_ratio > 0.5;

-- Check watermark lag
SELECT
    stream_name,
    partition,
    EXTRACT(EPOCH FROM (NOW() - watermark_timestamp)) as watermark_lag_seconds
FROM SYSTEM.WATERMARKS
WHERE watermark_lag_seconds > 60;
```

---

## Troubleshooting

### 6.1 Out of Memory (OOM)

**Symptoms:**
- Job crashes with OOM error
- Slow performance before crash
- High garbage collection activity

**Diagnosis:**

```sql
-- Check state size
SELECT
    job_id,
    operator_name,
    state_size_bytes,
    state_size_bytes / (1024 * 1024) as state_size_mb
FROM SYSTEM.OPERATOR_STATS
ORDER BY state_size_bytes DESC;
```

**Solutions:**

1. **Add state TTL:**

```sql
WITH (
    'state.ttl' = '1d',
    'state.cleanup.strategy' = 'incremental'
)
```

2. **Reduce window size:**

```sql
-- Before: 1-hour windows
TUMBLE(event_time, INTERVAL '1' HOUR)

-- After: 15-minute windows
TUMBLE(event_time, INTERVAL '15' MINUTE)
```

3. **Use approximate aggregations:**

```sql
-- Replace exact with approximate
APPROX_PERCENTILE(latency_ms, 0.95)
```

4. **Switch to disk-backed state:**

```sql
SET 'state.backend' = 'postgres';
```

---

### 6.2 High Latency

**Symptoms:**
- Slow query results
- High p95/p99 latency
- Watermark lag

**Diagnosis:**

```sql
-- Check operator latency
SELECT
    operator_name,
    avg_processing_time_ms,
    p95_processing_time_ms,
    p99_processing_time_ms
FROM SYSTEM.OPERATOR_STATS
ORDER BY p99_processing_time_ms DESC;
```

**Solutions:**

1. **Increase parallelism:**

```sql
SET 'parallelism.default' = '16';
```

2. **Optimize filters:**

```sql
-- Push filters down
WHERE latency_ms > 1000  -- Before aggregation
```

3. **Use approximate functions:**

```sql
APPROX_PERCENTILE instead of PERCENTILE_CONT
```

4. **Add indexes (for table lookups):**

```sql
CREATE INDEX idx_model_id ON model_configs(model_id);
```

---

### 6.3 Data Skew

**Symptoms:**
- Some partitions process much slower
- Uneven CPU utilization
- Stragglers in distributed processing

**Diagnosis:**

```sql
-- Check partition distribution
SELECT
    partition,
    COUNT(*) as event_count
FROM feedback_events
GROUP BY partition
ORDER BY event_count DESC;
```

**Solutions:**

1. **Add salt to key:**

```sql
-- Before: Single key
GROUP BY user_id

-- After: Salted key (4× parallelism)
GROUP BY user_id, MOD(HASH(event_id), 4) as salt
```

2. **Use different partitioning:**

```sql
-- Partition by composite key
PARTITION BY model_id, DATE(event_time)
```

---

### 6.4 Late Events Being Dropped

**Symptoms:**
- Data loss warnings in logs
- Actual counts lower than expected
- Gaps in aggregations

**Diagnosis:**

```sql
-- Check watermark lag
SELECT
    stream_name,
    watermark_timestamp,
    latest_event_timestamp,
    EXTRACT(EPOCH FROM (latest_event_timestamp - watermark_timestamp)) as lag_seconds
FROM SYSTEM.WATERMARKS;
```

**Solutions:**

1. **Increase allowed lateness:**

```sql
-- Before: 10 seconds
WATERMARK FOR event_time AS event_time - INTERVAL '10' SECOND

-- After: 2 minutes
WATERMARK FOR event_time AS event_time - INTERVAL '2' MINUTE
```

2. **Emit strategy for late data:**

```sql
EMIT AFTER WATERMARK + INTERVAL '1' MINUTE
```

3. **Side output late events:**

```sql
WITH (
    'late.events.output' = 'late_events_stream'
)
```

---

## Production Checklist

### 7.1 Pre-Deployment

- [ ] State backend configured (not memory)
- [ ] Checkpointing enabled
- [ ] State TTL configured
- [ ] All joins time-bounded
- [ ] Watermark strategy tested
- [ ] Resource limits set
- [ ] Monitoring queries deployed
- [ ] Alerting configured
- [ ] Backup strategy defined
- [ ] Disaster recovery tested

### 7.2 Configuration Review

```sql
-- Production-ready configuration
SET 'state.backend' = 'postgres';
SET 'state.postgres.url' = 'postgresql://localhost:5432/optimizer';
SET 'state.checkpoints.enabled' = 'true';
SET 'state.checkpoints.interval' = '60s';
SET 'state.ttl' = '7d';
SET 'parallelism.default' = '8';
SET 'execution.batch.size' = '1000';
SET 'buffer.memory.max' = '512MB';
SET 'metrics.export.enabled' = 'true';
SET 'metrics.export.interval' = '10s';
```

### 7.3 Monitoring Setup

**Essential Metrics:**

```sql
-- Query performance
SELECT
    job_id,
    records_per_second,
    avg_latency_ms,
    p99_latency_ms,
    error_rate
FROM SYSTEM.JOBS;

-- Resource utilization
SELECT
    cpu_usage_percent,
    memory_usage_bytes,
    disk_usage_bytes
FROM SYSTEM.RESOURCES;

-- Backpressure
SELECT
    operator_name,
    backpressure_ratio
FROM SYSTEM.OPERATOR_STATS
WHERE backpressure_ratio > 0.3;
```

### 7.4 Alerting Rules

**Critical Alerts:**

1. **High Error Rate:**

```sql
CREATE ALERT high_error_rate
WHEN (
    SELECT error_rate
    FROM SYSTEM.JOBS
    WHERE job_id = 'production-pipeline'
) > 0.05  -- 5% error rate
THEN NOTIFY 'pagerduty';
```

2. **High Latency:**

```sql
CREATE ALERT high_latency
WHEN (
    SELECT p99_latency_ms
    FROM SYSTEM.JOBS
    WHERE job_id = 'production-pipeline'
) > 5000  -- 5 seconds
THEN NOTIFY 'slack';
```

3. **Watermark Lag:**

```sql
CREATE ALERT watermark_lag
WHEN (
    SELECT MAX(EXTRACT(EPOCH FROM (NOW() - watermark_timestamp)))
    FROM SYSTEM.WATERMARKS
) > 300  -- 5 minutes
THEN NOTIFY 'pagerduty';
```

---

## Performance Benchmarks

### Typical Performance Numbers

| Metric | Development | Production |
|--------|-------------|------------|
| Throughput | 1k events/sec | 10-100k events/sec |
| Latency (p50) | 100ms | 50ms |
| Latency (p99) | 1s | 500ms |
| State size | <100MB | 1-100GB |
| Memory usage | 512MB | 4-32GB |
| CPU cores | 2-4 | 8-64 |

### Query Optimization Impact

| Optimization | Speedup | Memory Reduction |
|--------------|---------|------------------|
| Approximate percentiles | 10-50× | 90% |
| Conditional aggregation | 2-5× | N/A |
| Projection pushdown | 1.5-3× | 50-80% |
| Filter pushdown | 2-10× | 50-90% |
| State TTL | N/A | 50-90% |
| Join broadcast | 5-20× | 80-95% |

---

## Summary

Key takeaways for production SQL interfaces:

1. **Always filter early** to reduce data volume
2. **Use approximate aggregations** for large datasets
3. **Configure state TTL** to prevent unbounded growth
4. **Time-bound all joins** to prevent infinite state
5. **Choose appropriate window sizes** (5-15 min for most cases)
6. **Monitor watermark lag** to detect late data issues
7. **Enable checkpointing** for fault tolerance
8. **Use disk-backed state** (Postgres/Redis) for production

### Additional Resources

- [SQL Reference Guide](SQL_REFERENCE_GUIDE.md): Complete syntax
- [SQL Examples](SQL_EXAMPLES.md): 56 practical examples
- [SQL Tutorial](SQL_TUTORIAL.md): Step-by-step learning
- [SQL API Reference](SQL_API_REFERENCE.md): Rust integration
