# SQL Tutorial

**LLM Auto Optimizer - SQL Interface**
**Version:** 1.0
**Last Updated:** 2025-11-10

---

## Table of Contents

1. [Introduction](#introduction)
2. [Getting Started](#getting-started)
3. [Lesson 1: Creating Your First Stream](#lesson-1-creating-your-first-stream)
4. [Lesson 2: Basic Queries](#lesson-2-basic-queries)
5. [Lesson 3: Filtering and Transforming Data](#lesson-3-filtering-and-transforming-data)
6. [Lesson 4: Simple Aggregations](#lesson-4-simple-aggregations)
7. [Lesson 5: Tumbling Windows](#lesson-5-tumbling-windows)
8. [Lesson 6: Sliding Windows](#lesson-6-sliding-windows)
9. [Lesson 7: Session Windows](#lesson-7-session-windows)
10. [Lesson 8: Joining Streams](#lesson-8-joining-streams)
11. [Lesson 9: Advanced Patterns](#lesson-9-advanced-patterns)
12. [Lesson 10: Production Deployment](#lesson-10-production-deployment)

---

## Introduction

Welcome to the SQL Interface tutorial for LLM Auto Optimizer! This tutorial will guide you through building stream processing pipelines using familiar SQL syntax. By the end, you'll be able to create real-time analytics for LLM performance monitoring, cost optimization, and quality tracking.

### What You'll Learn

- Create and manage data streams
- Write queries for real-time data processing
- Use window functions for time-based aggregations
- Join multiple streams for enriched analytics
- Deploy production-ready monitoring pipelines

### Prerequisites

- Basic SQL knowledge (SELECT, WHERE, GROUP BY)
- Understanding of streaming concepts (optional but helpful)
- LLM Auto Optimizer installed and running

---

## Getting Started

### Installation

First, ensure the LLM Auto Optimizer is installed:

```bash
# Clone the repository
git clone https://github.com/globalbusinessadvisors/llm-auto-optimizer.git
cd llm-auto-optimizer

# Build the project
cargo build --release

# Start the SQL interface
./target/release/llm-optimizer sql-server --port 8080
```

### Connecting to SQL Interface

```bash
# Using the CLI
llm-optimizer sql

# Or connect via your favorite SQL client
psql -h localhost -p 8080 -U optimizer -d streaming
```

### Sample Data

For this tutorial, we'll use LLM feedback events with the following schema:

```sql
event_id       VARCHAR      -- Unique event identifier
user_id        VARCHAR      -- User identifier
model_id       VARCHAR      -- Model identifier (e.g., gpt-4-id)
model_name     VARCHAR      -- Model name (e.g., gpt-4)
prompt_template VARCHAR     -- Prompt variant identifier
rating         DOUBLE       -- User rating (1.0 - 5.0)
latency_ms     BIGINT       -- Response latency in milliseconds
cost_usd       DOUBLE       -- Cost in USD
tokens_used    BIGINT       -- Total tokens (input + output)
error_code     VARCHAR      -- Error code (NULL if successful)
event_time     TIMESTAMP    -- When event occurred
```

---

## Lesson 1: Creating Your First Stream

### Objective

Define a stream to ingest LLM feedback events from Kafka.

### Theory

In stream processing:
- **Streams** are unbounded sequences of events
- **Event time** is when the event actually occurred
- **Watermarks** track progress and handle late data

### Step 1: Create a Kafka Stream

```sql
CREATE STREAM feedback_events (
    event_id VARCHAR,
    user_id VARCHAR,
    model_id VARCHAR,
    model_name VARCHAR,
    prompt_template VARCHAR,
    rating DOUBLE,
    latency_ms BIGINT,
    cost_usd DOUBLE,
    tokens_used BIGINT,
    error_code VARCHAR,
    event_time TIMESTAMP ROWTIME,
    WATERMARK FOR event_time AS event_time - INTERVAL '30' SECOND
)
WITH (
    'connector' = 'kafka',
    'topic' = 'llm-feedback',
    'properties.bootstrap.servers' = 'localhost:9092',
    'properties.group.id' = 'optimizer-consumer',
    'format' = 'json',
    'scan.startup.mode' = 'latest-offset'
);
```

**Key Points:**
- `ROWTIME` marks `event_time` as the event timestamp
- `WATERMARK` allows events up to 30 seconds late
- Kafka configuration is specified in `WITH` clause

### Step 2: Verify Stream Creation

```sql
-- List all streams
SELECT * FROM SYSTEM.STREAMS;

-- Check stream schema
DESCRIBE feedback_events;
```

### Step 3: Test Data Flow

```sql
-- View incoming events (limited to 10)
SELECT * FROM feedback_events
LIMIT 10;
```

### Exercise 1.1

Create a second stream for performance metrics:

```sql
CREATE STREAM performance_metrics (
    metric_id VARCHAR,
    model_id VARCHAR,
    metric_name VARCHAR,
    metric_value DOUBLE,
    labels MAP<VARCHAR, VARCHAR>,
    timestamp TIMESTAMP ROWTIME,
    WATERMARK FOR timestamp AS timestamp - INTERVAL '10' SECOND
)
WITH (
    'connector' = 'kafka',
    'topic' = 'llm-performance',
    'properties.bootstrap.servers' = 'localhost:9092',
    'format' = 'json'
);
```

**Expected Output:** Stream created successfully.

---

## Lesson 2: Basic Queries

### Objective

Learn to query streaming data with SELECT, WHERE, and projections.

### Step 1: Select All Events

```sql
-- Stream all events (continuous query)
SELECT * FROM feedback_events;
```

**Result:** Continuously displays new events as they arrive.

### Step 2: Select Specific Columns

```sql
-- Show only relevant fields
SELECT
    event_id,
    model_name,
    latency_ms,
    rating,
    event_time
FROM feedback_events;
```

### Step 3: Filtering Data

```sql
-- Show only slow requests (> 5 seconds)
SELECT
    event_id,
    model_name,
    latency_ms,
    user_id,
    event_time
FROM feedback_events
WHERE latency_ms > 5000;
```

### Step 4: Multiple Conditions

```sql
-- High latency AND specific model
SELECT
    event_id,
    model_name,
    latency_ms,
    cost_usd,
    rating
FROM feedback_events
WHERE latency_ms > 3000
  AND model_name = 'gpt-4';
```

### Step 5: Computed Columns

```sql
-- Calculate cost per second
SELECT
    event_id,
    model_name,
    latency_ms,
    cost_usd,
    (cost_usd / (latency_ms / 1000.0)) as cost_per_second,
    event_time
FROM feedback_events;
```

### Exercise 2.1

Write a query to find all error events with rating below 3.0:

<details>
<summary>Solution</summary>

```sql
SELECT
    event_id,
    user_id,
    model_name,
    error_code,
    rating,
    event_time
FROM feedback_events
WHERE error_code IS NOT NULL
  AND rating < 3.0;
```
</details>

### Exercise 2.2

Calculate cost per 1000 tokens for each request:

<details>
<summary>Solution</summary>

```sql
SELECT
    event_id,
    model_name,
    tokens_used,
    cost_usd,
    (cost_usd / tokens_used * 1000) as cost_per_1k_tokens,
    event_time
FROM feedback_events
WHERE tokens_used > 0;
```
</details>

---

## Lesson 3: Filtering and Transforming Data

### Objective

Master data transformation using SQL functions.

### Step 1: String Functions

```sql
-- Extract model provider from model name
SELECT
    model_name,
    CASE
        WHEN model_name LIKE 'gpt-%' THEN 'OpenAI'
        WHEN model_name LIKE 'claude-%' THEN 'Anthropic'
        WHEN model_name LIKE 'gemini-%' THEN 'Google'
        ELSE 'Other'
    END as provider,
    latency_ms,
    cost_usd
FROM feedback_events;
```

### Step 2: Date/Time Functions

```sql
-- Extract hour of day for usage patterns
SELECT
    EXTRACT(HOUR FROM event_time) as hour_of_day,
    EXTRACT(DOW FROM event_time) as day_of_week,
    model_name,
    latency_ms,
    cost_usd
FROM feedback_events;
```

### Step 3: Conditional Logic

```sql
-- Categorize requests by latency
SELECT
    event_id,
    model_name,
    latency_ms,
    CASE
        WHEN latency_ms < 1000 THEN 'Fast'
        WHEN latency_ms < 5000 THEN 'Medium'
        ELSE 'Slow'
    END as latency_category,
    rating
FROM feedback_events;
```

### Step 4: Handling NULLs

```sql
-- Replace NULL ratings with default
SELECT
    event_id,
    model_name,
    COALESCE(rating, 3.0) as rating,
    COALESCE(error_code, 'NO_ERROR') as error_status,
    latency_ms
FROM feedback_events;
```

### Exercise 3.1

Create a quality score that combines rating and error status:

<details>
<summary>Solution</summary>

```sql
SELECT
    event_id,
    model_name,
    rating,
    error_code,
    CASE
        WHEN error_code IS NOT NULL THEN 0.0
        WHEN rating >= 4.0 THEN 10.0
        WHEN rating >= 3.0 THEN 7.0
        WHEN rating >= 2.0 THEN 4.0
        ELSE 2.0
    END as quality_score
FROM feedback_events;
```
</details>

---

## Lesson 4: Simple Aggregations

### Objective

Perform basic aggregations (COUNT, AVG, SUM) without windows.

### Step 1: Count Events

```sql
-- Total events by model (continuous aggregation)
SELECT
    model_name,
    COUNT(*) as event_count
FROM feedback_events
GROUP BY model_name;
```

**Note:** This is a continuous query that updates as new events arrive.

### Step 2: Average Latency

```sql
-- Average latency per model
SELECT
    model_name,
    AVG(latency_ms) as avg_latency,
    MIN(latency_ms) as min_latency,
    MAX(latency_ms) as max_latency,
    STDDEV(latency_ms) as stddev_latency
FROM feedback_events
GROUP BY model_name;
```

### Step 3: Sum of Costs

```sql
-- Total cost per model
SELECT
    model_name,
    SUM(cost_usd) as total_cost,
    COUNT(*) as requests,
    SUM(cost_usd) / COUNT(*) as avg_cost_per_request
FROM feedback_events
GROUP BY model_name
ORDER BY total_cost DESC;
```

### Step 4: Count Distinct

```sql
-- Unique users per model
SELECT
    model_name,
    COUNT(DISTINCT user_id) as unique_users,
    COUNT(*) as total_requests,
    COUNT(*) * 1.0 / COUNT(DISTINCT user_id) as requests_per_user
FROM feedback_events
GROUP BY model_name;
```

### Step 5: Percentiles

```sql
-- Latency percentiles (p50, p95, p99)
SELECT
    model_name,
    APPROX_PERCENTILE(latency_ms, 0.5) as p50_latency,
    APPROX_PERCENTILE(latency_ms, 0.95) as p95_latency,
    APPROX_PERCENTILE(latency_ms, 0.99) as p99_latency
FROM feedback_events
GROUP BY model_name;
```

### Step 6: Conditional Aggregation

```sql
-- Count errors and successes
SELECT
    model_name,
    COUNT(*) as total_requests,
    COUNT(CASE WHEN error_code IS NOT NULL THEN 1 END) as errors,
    COUNT(CASE WHEN error_code IS NULL THEN 1 END) as successes,
    COUNT(CASE WHEN error_code IS NOT NULL THEN 1 END) * 100.0 / COUNT(*) as error_rate_pct
FROM feedback_events
GROUP BY model_name;
```

### Exercise 4.1

Calculate average rating and satisfaction rate (rating >= 4.0) per model:

<details>
<summary>Solution</summary>

```sql
SELECT
    model_name,
    AVG(rating) as avg_rating,
    COUNT(CASE WHEN rating >= 4.0 THEN 1 END) * 100.0 / COUNT(*) as satisfaction_rate,
    COUNT(*) as total_ratings
FROM feedback_events
WHERE rating IS NOT NULL
GROUP BY model_name
ORDER BY satisfaction_rate DESC;
```
</details>

---

## Lesson 5: Tumbling Windows

### Objective

Learn time-based aggregations using tumbling (non-overlapping) windows.

### Theory

Tumbling windows divide time into fixed-size, non-overlapping intervals:

```
Time:    0    5    10   15   20   25   30
         |----|----|----|----|----|----|
Windows: [0-5)[5-10)[10-15)[15-20)[20-25)[25-30)
```

### Step 1: 5-Minute Windows

```sql
-- Aggregate events every 5 minutes
SELECT
    TUMBLE_START(event_time, INTERVAL '5' MINUTE) as window_start,
    TUMBLE_END(event_time, INTERVAL '5' MINUTE) as window_end,
    COUNT(*) as event_count,
    AVG(latency_ms) as avg_latency
FROM feedback_events
GROUP BY TUMBLE(event_time, INTERVAL '5' MINUTE);
```

**Result:** Emits results every 5 minutes.

### Step 2: Per-Model Metrics

```sql
-- 5-minute metrics per model
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
    TUMBLE(event_time, INTERVAL '5' MINUTE);
```

### Step 3: Hourly Reports

```sql
-- Hourly performance report
SELECT
    model_name,
    TUMBLE_START(event_time, INTERVAL '1' HOUR) as hour,
    COUNT(*) as requests,
    COUNT(DISTINCT user_id) as unique_users,
    AVG(latency_ms) as avg_latency,
    APPROX_PERCENTILE(latency_ms, 0.99) as p99_latency,
    SUM(cost_usd) as hourly_cost,
    COUNT(CASE WHEN error_code IS NOT NULL THEN 1 END) * 100.0 / COUNT(*) as error_rate_pct
FROM feedback_events
GROUP BY
    model_name,
    TUMBLE(event_time, INTERVAL '1' HOUR)
ORDER BY hour DESC;
```

### Step 4: Daily Aggregations

```sql
-- Daily cost summary
SELECT
    DATE(TUMBLE_START(event_time, INTERVAL '1' DAY)) as date,
    model_name,
    COUNT(*) as daily_requests,
    SUM(cost_usd) as daily_cost,
    AVG(cost_usd) as avg_cost_per_request,
    SUM(tokens_used) as total_tokens
FROM feedback_events
GROUP BY
    model_name,
    TUMBLE(event_time, INTERVAL '1' DAY)
ORDER BY date DESC, daily_cost DESC;
```

### Step 5: Creating Derived Streams

```sql
-- Save aggregated metrics to a new stream
CREATE STREAM model_performance_5min AS
SELECT
    model_name,
    TUMBLE_START(event_time, INTERVAL '5' MINUTE) as window_start,
    TUMBLE_END(event_time, INTERVAL '5' MINUTE) as window_end,
    COUNT(*) as requests,
    AVG(latency_ms) as avg_latency,
    APPROX_PERCENTILE(latency_ms, 0.95) as p95_latency,
    SUM(cost_usd) as total_cost,
    AVG(rating) as avg_rating,
    COUNT(CASE WHEN error_code IS NOT NULL THEN 1 END) * 100.0 / COUNT(*) as error_rate
FROM feedback_events
GROUP BY
    model_name,
    TUMBLE(event_time, INTERVAL '5' MINUTE);
```

### Exercise 5.1

Create a 1-minute window query that calculates throughput (QPS):

<details>
<summary>Solution</summary>

```sql
SELECT
    model_name,
    TUMBLE_START(event_time, INTERVAL '1' MINUTE) as minute,
    COUNT(*) as requests_per_minute,
    COUNT(*) / 60.0 as qps,
    AVG(latency_ms) as avg_latency
FROM feedback_events
GROUP BY
    model_name,
    TUMBLE(event_time, INTERVAL '1' MINUTE);
```
</details>

---

## Lesson 6: Sliding Windows

### Objective

Use sliding (overlapping) windows for smooth trend analysis.

### Theory

Sliding windows overlap, providing smoother metrics:

```
Time:    0    5    10   15   20
         |----|----|----|----|
Windows: [0--------15)
              [5--------20)
                   [10-------25)
```

### Step 1: 15-Minute Sliding Window

```sql
-- 15-minute windows sliding every 1 minute
SELECT
    model_name,
    HOP_START(event_time, INTERVAL '1' MINUTE, INTERVAL '15' MINUTE) as window_start,
    HOP_END(event_time, INTERVAL '1' MINUTE, INTERVAL '15' MINUTE) as window_end,
    COUNT(*) as requests,
    AVG(latency_ms) as moving_avg_latency,
    STDDEV(latency_ms) as moving_stddev_latency
FROM feedback_events
GROUP BY
    model_name,
    HOP(event_time, INTERVAL '1' MINUTE, INTERVAL '15' MINUTE);
```

**Use Case:** Smooth latency monitoring, anomaly detection.

### Step 2: Anomaly Detection

```sql
-- Detect latency spikes using sliding windows
WITH moving_stats AS (
    SELECT
        model_name,
        HOP_START(event_time, INTERVAL '1' MINUTE, INTERVAL '10' MINUTE) as window_start,
        AVG(latency_ms) as avg_latency,
        STDDEV(latency_ms) as stddev_latency
    FROM feedback_events
    GROUP BY
        model_name,
        HOP(event_time, INTERVAL '1' MINUTE, INTERVAL '10' MINUTE)
)
SELECT
    model_name,
    window_start,
    avg_latency,
    stddev_latency,
    LAG(avg_latency, 1) OVER (PARTITION BY model_name ORDER BY window_start) as prev_avg,
    CASE
        WHEN avg_latency > LAG(avg_latency, 1) OVER (PARTITION BY model_name ORDER BY window_start) * 1.5
        THEN 'SPIKE'
        ELSE 'NORMAL'
    END as status
FROM moving_stats;
```

### Step 3: Cost Trend Analysis

```sql
-- 30-minute cost trends (5-minute slide)
SELECT
    model_name,
    HOP_START(event_time, INTERVAL '5' MINUTE, INTERVAL '30' MINUTE) as window_start,
    COUNT(*) as requests,
    SUM(cost_usd) as total_cost,
    SUM(cost_usd) / 30.0 * 60.0 as cost_per_hour_estimate
FROM feedback_events
GROUP BY
    model_name,
    HOP(event_time, INTERVAL '5' MINUTE, INTERVAL '30' MINUTE);
```

### Exercise 6.1

Create a sliding window query (10-min window, 2-min slide) to track quality score trends:

<details>
<summary>Solution</summary>

```sql
SELECT
    model_name,
    HOP_START(event_time, INTERVAL '2' MINUTE, INTERVAL '10' MINUTE) as window_start,
    AVG(rating) as avg_rating,
    COUNT(*) as sample_size,
    COUNT(CASE WHEN rating >= 4 THEN 1 END) * 100.0 / COUNT(*) as satisfaction_rate,
    LAG(AVG(rating)) OVER (
        PARTITION BY model_name
        ORDER BY HOP_START(event_time, INTERVAL '2' MINUTE, INTERVAL '10' MINUTE)
    ) as prev_avg_rating
FROM feedback_events
WHERE rating IS NOT NULL
GROUP BY
    model_name,
    HOP(event_time, INTERVAL '2' MINUTE, INTERVAL '10' MINUTE);
```
</details>

---

## Lesson 7: Session Windows

### Objective

Analyze user sessions using session windows (dynamic windows based on inactivity).

### Theory

Session windows group events with gaps smaller than a threshold:

```
Events: E1   E2       E3   E4              E5   E6
Time:   0    5        15   20              50   55
Gap:    30 minutes

Session1: [E1, E2, E3, E4] (0-50)
Session2: [E5, E6] (50-85)
```

### Step 1: Basic Session Analysis

```sql
-- User sessions with 30-minute inactivity gap
SELECT
    user_id,
    SESSION_START(event_time, INTERVAL '30' MINUTE) as session_start,
    SESSION_END(event_time, INTERVAL '30' MINUTE) as session_end,
    COUNT(*) as interactions,
    EXTRACT(EPOCH FROM (
        SESSION_END(event_time, INTERVAL '30' MINUTE) -
        SESSION_START(event_time, INTERVAL '30' MINUTE)
    )) / 60 as session_duration_minutes
FROM feedback_events
GROUP BY
    user_id,
    SESSION(event_time, INTERVAL '30' MINUTE);
```

### Step 2: Session Metrics

```sql
-- Detailed session analytics
SELECT
    user_id,
    SESSION_START(event_time, INTERVAL '30' MINUTE) as session_start,
    SESSION_END(event_time, INTERVAL '30' MINUTE) as session_end,
    COUNT(*) as interactions,
    COUNT(DISTINCT model_name) as models_tried,
    SUM(cost_usd) as session_cost,
    AVG(rating) as avg_session_rating,
    MAX(latency_ms) as max_latency,
    COUNT(CASE WHEN error_code IS NOT NULL THEN 1 END) as errors
FROM feedback_events
GROUP BY
    user_id,
    SESSION(event_time, INTERVAL '30' MINUTE)
ORDER BY interactions DESC;
```

### Step 3: Model Switching Analysis

```sql
-- Find users who switch models within a session
SELECT
    user_id,
    SESSION_START(event_time, INTERVAL '30' MINUTE) as session_start,
    COUNT(*) as interactions,
    ARRAY_AGG(DISTINCT model_name ORDER BY model_name) as models_used,
    COUNT(DISTINCT model_name) as unique_models
FROM feedback_events
GROUP BY
    user_id,
    SESSION(event_time, INTERVAL '30' MINUTE)
HAVING COUNT(DISTINCT model_name) > 1;
```

### Step 4: Abandoned Sessions

```sql
-- Identify problematic sessions (low rating or many errors)
SELECT
    user_id,
    SESSION_START(event_time, INTERVAL '30' MINUTE) as session_start,
    COUNT(*) as interactions,
    AVG(rating) as avg_rating,
    SUM(CASE WHEN error_code IS NOT NULL THEN 1 END) as error_count,
    SUM(cost_usd) as session_cost
FROM feedback_events
GROUP BY
    user_id,
    SESSION(event_time, INTERVAL '30' MINUTE)
HAVING AVG(rating) < 3.0 OR SUM(CASE WHEN error_code IS NOT NULL THEN 1 END) > 2;
```

### Exercise 7.1

Create a query to find the most expensive user sessions:

<details>
<summary>Solution</summary>

```sql
SELECT
    user_id,
    SESSION_START(event_time, INTERVAL '30' MINUTE) as session_start,
    SESSION_END(event_time, INTERVAL '30' MINUTE) as session_end,
    COUNT(*) as interactions,
    SUM(cost_usd) as total_cost,
    AVG(cost_usd) as avg_cost_per_interaction,
    ARRAY_AGG(model_name ORDER BY event_time) as models_sequence
FROM feedback_events
GROUP BY
    user_id,
    SESSION(event_time, INTERVAL '30' MINUTE)
ORDER BY total_cost DESC
LIMIT 20;
```
</details>

---

## Lesson 8: Joining Streams

### Objective

Enrich streams by joining with other streams or tables.

### Step 1: Create Configuration Table

```sql
-- Create model configuration table
CREATE TABLE model_configs (
    model_id VARCHAR PRIMARY KEY,
    model_name VARCHAR,
    temperature DOUBLE,
    max_tokens INTEGER,
    cost_per_1k_tokens DOUBLE,
    provider VARCHAR,
    updated_at TIMESTAMP
)
WITH (
    'connector' = 'postgres',
    'url' = 'postgresql://localhost:5432/optimizer',
    'table-name' = 'model_configs'
);
```

### Step 2: Inner Join

```sql
-- Enrich events with model configuration
SELECT
    f.event_id,
    f.user_id,
    f.model_name,
    f.latency_ms,
    f.cost_usd,
    m.temperature,
    m.max_tokens,
    m.provider
FROM feedback_events f
INNER JOIN model_configs m
ON f.model_id = m.model_id;
```

### Step 3: Left Join (Handle Missing Data)

```sql
-- Include all events even if config not found
SELECT
    f.event_id,
    f.model_name,
    f.latency_ms,
    COALESCE(m.temperature, 0.7) as temperature,
    COALESCE(m.max_tokens, 2048) as max_tokens,
    COALESCE(m.provider, 'Unknown') as provider
FROM feedback_events f
LEFT JOIN model_configs m
ON f.model_id = m.model_id;
```

### Step 4: Stream-to-Stream Join (Time-Bounded)

```sql
-- Join feedback with performance metrics within 1-minute window
SELECT
    f.event_id,
    f.user_id,
    f.model_name,
    f.latency_ms as app_latency,
    p.metric_name,
    p.metric_value,
    f.event_time
FROM feedback_events f
JOIN performance_metrics p
ON f.model_id = p.model_id
AND f.event_time BETWEEN p.timestamp - INTERVAL '30' SECOND
                     AND p.timestamp + INTERVAL '30' SECOND
WHERE p.metric_name IN ('cpu_usage', 'memory_usage', 'gpu_utilization');
```

### Step 5: Multi-Stream Join

```sql
-- Join feedback, configs, and user profiles
CREATE TABLE user_profiles (
    user_id VARCHAR PRIMARY KEY,
    user_name VARCHAR,
    subscription_tier VARCHAR,
    signup_date TIMESTAMP
)
WITH (
    'connector' = 'postgres',
    'table-name' = 'user_profiles'
);

SELECT
    f.event_id,
    u.user_name,
    u.subscription_tier,
    f.model_name,
    m.provider,
    f.latency_ms,
    f.cost_usd,
    f.rating
FROM feedback_events f
JOIN user_profiles u ON f.user_id = u.user_id
JOIN model_configs m ON f.model_id = m.model_id
WHERE u.subscription_tier = 'enterprise';
```

### Exercise 8.1

Create a query that joins feedback events with configs and calculates whether each request exceeded the expected cost:

<details>
<summary>Solution</summary>

```sql
SELECT
    f.event_id,
    f.model_name,
    f.tokens_used,
    f.cost_usd as actual_cost,
    (f.tokens_used / 1000.0 * m.cost_per_1k_tokens) as expected_cost,
    f.cost_usd - (f.tokens_used / 1000.0 * m.cost_per_1k_tokens) as cost_difference,
    CASE
        WHEN f.cost_usd > (f.tokens_used / 1000.0 * m.cost_per_1k_tokens) * 1.1 THEN 'OVERCHARGED'
        WHEN f.cost_usd < (f.tokens_used / 1000.0 * m.cost_per_1k_tokens) * 0.9 THEN 'UNDERCHARGED'
        ELSE 'NORMAL'
    END as cost_status
FROM feedback_events f
JOIN model_configs m ON f.model_id = m.model_id
WHERE f.tokens_used > 0;
```
</details>

---

## Lesson 9: Advanced Patterns

### Objective

Learn advanced patterns for production use.

### Pattern 1: Multi-Level Aggregation

```sql
-- Aggregate hourly data into daily summaries
WITH hourly_metrics AS (
    SELECT
        model_name,
        TUMBLE_START(event_time, INTERVAL '1' HOUR) as hour,
        COUNT(*) as requests,
        AVG(latency_ms) as avg_latency,
        SUM(cost_usd) as hourly_cost
    FROM feedback_events
    GROUP BY
        model_name,
        TUMBLE(event_time, INTERVAL '1' HOUR)
)
SELECT
    model_name,
    DATE(hour) as date,
    SUM(requests) as daily_requests,
    AVG(avg_latency) as daily_avg_latency,
    SUM(hourly_cost) as daily_cost
FROM hourly_metrics
GROUP BY model_name, DATE(hour);
```

### Pattern 2: Top-N per Window

```sql
-- Top 5 users by cost each hour
WITH user_hourly_cost AS (
    SELECT
        user_id,
        TUMBLE_START(event_time, INTERVAL '1' HOUR) as hour,
        SUM(cost_usd) as hourly_cost,
        COUNT(*) as requests
    FROM feedback_events
    GROUP BY
        user_id,
        TUMBLE(event_time, INTERVAL '1' HOUR)
)
SELECT
    hour,
    user_id,
    hourly_cost,
    requests,
    RANK() OVER (PARTITION BY hour ORDER BY hourly_cost DESC) as cost_rank
FROM user_hourly_cost
QUALIFY cost_rank <= 5;
```

### Pattern 3: Deduplication

```sql
-- Remove duplicate events (keep first occurrence)
SELECT DISTINCT ON (event_id)
    event_id,
    user_id,
    model_name,
    latency_ms,
    cost_usd,
    event_time
FROM feedback_events
ORDER BY event_id, event_time;
```

### Pattern 4: Late Data Handling

```sql
-- Configure allowed lateness and emit strategy
SELECT
    model_name,
    TUMBLE_START(event_time, INTERVAL '5' MINUTE) as window_start,
    COUNT(*) as event_count,
    AVG(latency_ms) as avg_latency
FROM feedback_events
GROUP BY
    model_name,
    TUMBLE(event_time, INTERVAL '5' MINUTE)
EMIT AFTER WATERMARK + INTERVAL '2' MINUTE;
```

### Pattern 5: Complex Event Processing

```sql
-- Detect sequences: high latency followed by low rating
SELECT
    e1.user_id,
    e1.event_id as first_event,
    e2.event_id as second_event,
    e1.latency_ms as first_latency,
    e2.rating as second_rating,
    e2.event_time - e1.event_time as time_between
FROM feedback_events e1
JOIN feedback_events e2
ON e1.user_id = e2.user_id
AND e2.event_time > e1.event_time
AND e2.event_time < e1.event_time + INTERVAL '5' MINUTE
WHERE e1.latency_ms > 5000
  AND e2.rating < 3.0;
```

---

## Lesson 10: Production Deployment

### Objective

Deploy production-ready monitoring pipelines.

### Step 1: Create Monitoring Dashboard Stream

```sql
CREATE STREAM monitoring_dashboard AS
SELECT
    model_name,
    TUMBLE_START(event_time, INTERVAL '1' MINUTE) as minute,
    COUNT(*) as requests,
    COUNT(DISTINCT user_id) as unique_users,
    AVG(latency_ms) as avg_latency,
    APPROX_PERCENTILE(latency_ms, 0.95) as p95_latency,
    APPROX_PERCENTILE(latency_ms, 0.99) as p99_latency,
    SUM(cost_usd) as total_cost,
    AVG(cost_usd) as avg_cost,
    AVG(rating) as avg_rating,
    COUNT(CASE WHEN rating >= 4 THEN 1 END) * 100.0 / NULLIF(COUNT(rating), 0) as satisfaction_pct,
    COUNT(CASE WHEN error_code IS NOT NULL THEN 1 END) as errors,
    COUNT(CASE WHEN error_code IS NOT NULL THEN 1 END) * 100.0 / COUNT(*) as error_rate_pct
FROM feedback_events
GROUP BY
    model_name,
    TUMBLE(event_time, INTERVAL '1' MINUTE);
```

### Step 2: Create Alerting Stream

```sql
CREATE STREAM sla_violations AS
SELECT
    model_name,
    TUMBLE_START(event_time, INTERVAL '5' MINUTE) as window_start,
    COUNT(*) as requests,
    APPROX_PERCENTILE(latency_ms, 0.95) as p95_latency,
    AVG(latency_ms) as avg_latency,
    'SLA_VIOLATED' as alert_type,
    NOW() as alert_time
FROM feedback_events
GROUP BY
    model_name,
    TUMBLE(event_time, INTERVAL '5' MINUTE)
HAVING APPROX_PERCENTILE(latency_ms, 0.95) > 3000
   OR AVG(latency_ms) > 2000;
```

### Step 3: Write to Kafka for Downstream Processing

```sql
INSERT INTO alert_topic
SELECT
    model_name,
    window_start,
    p95_latency,
    alert_type,
    alert_time
FROM sla_violations;
```

### Step 4: Persistent State Configuration

```sql
-- Configure state backend for fault tolerance
SET 'state.backend' = 'postgres';
SET 'state.postgres.url' = 'postgresql://localhost:5432/optimizer';
SET 'state.checkpoints.enabled' = 'true';
SET 'state.checkpoints.interval' = '60s';
```

### Step 5: Monitoring Query Performance

```sql
-- Check running jobs
SELECT
    job_id,
    query,
    status,
    start_time,
    records_processed,
    errors
FROM SYSTEM.JOBS
WHERE status = 'Running';

-- Check watermarks
SELECT
    stream_name,
    partition,
    watermark_timestamp,
    latest_event_timestamp
FROM SYSTEM.WATERMARKS;
```

---

## Summary

Congratulations! You've completed the SQL Interface tutorial. You've learned:

1. How to create and manage streams
2. Basic querying and filtering
3. Aggregation functions
4. Tumbling, sliding, and session windows
5. Joining streams and tables
6. Advanced patterns for production

### Next Steps

- [SQL Reference Guide](SQL_REFERENCE_GUIDE.md): Complete syntax reference
- [SQL Examples](SQL_EXAMPLES.md): 56 practical examples
- [SQL Best Practices](SQL_BEST_PRACTICES.md): Performance optimization
- [SQL API Reference](SQL_API_REFERENCE.md): Embed in Rust applications

### Additional Projects

Try building these on your own:

1. **Cost Forecasting**: Predict next month's cost based on trends
2. **Anomaly Detection**: Z-score based anomaly detection
3. **User Churn**: Identify at-risk users
4. **Model Optimization**: Recommend optimal model for each user

Happy streaming!
