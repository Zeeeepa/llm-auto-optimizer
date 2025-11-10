# SQL Reference Guide

**LLM Auto Optimizer - SQL Interface**
**Version:** 1.0
**Last Updated:** 2025-11-10

---

## Table of Contents

1. [Introduction](#introduction)
2. [SQL Dialects](#sql-dialects)
3. [Data Types](#data-types)
4. [DDL Statements](#ddl-statements)
5. [DML Statements](#dml-statements)
6. [Query Clauses](#query-clauses)
7. [Window Functions](#window-functions)
8. [Aggregation Functions](#aggregation-functions)
9. [Scalar Functions](#scalar-functions)
10. [Join Operations](#join-operations)
11. [Time Semantics](#time-semantics)
12. [System Tables](#system-tables)

---

## Introduction

The SQL Interface for LLM Auto Optimizer provides a declarative way to define stream processing pipelines using familiar SQL syntax. It supports both batch and streaming queries with advanced windowing, watermarking, and stateful operations.

### Key Features

- **ANSI SQL Compatible**: Standard SQL syntax for queries
- **Streaming Extensions**: Window functions, watermarks, and time semantics
- **Event-Time Processing**: Late-arrival handling and out-of-order processing
- **Stateful Operations**: Aggregations, joins, and deduplication with state management
- **Real-time Analytics**: Sub-second latency for stream processing

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                      SQL Interface                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  SQL Query ──▶ Parser ──▶ Optimizer ──▶ Execution Engine   │
│                   │           │              │              │
│                   ▼           ▼              ▼              │
│              Logical     Physical      Stream Processor     │
│               Plan         Plan                             │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## SQL Dialects

The SQL interface supports a PostgreSQL-compatible dialect with streaming extensions inspired by Apache Flink SQL and KSQL.

### Supported Standards

- **ANSI SQL-92**: Core SQL functionality
- **PostgreSQL Extensions**: Array operations, JSON functions
- **Streaming SQL**: Windowing, watermarks, and temporal operations

---

## Data Types

### Primitive Types

| Type | Description | Example |
|------|-------------|---------|
| `BOOLEAN` | True/false values | `TRUE`, `FALSE` |
| `TINYINT` | 8-bit signed integer | `127` |
| `SMALLINT` | 16-bit signed integer | `32767` |
| `INTEGER` / `INT` | 32-bit signed integer | `2147483647` |
| `BIGINT` | 64-bit signed integer | `9223372036854775807` |
| `FLOAT` | 32-bit floating point | `3.14` |
| `DOUBLE` | 64-bit floating point | `2.718281828` |
| `DECIMAL(p,s)` | Fixed-point decimal | `DECIMAL(10,2)` |
| `VARCHAR(n)` | Variable-length string | `'hello'` |
| `TEXT` | Unlimited-length string | `'long text'` |
| `TIMESTAMP` | Date and time | `'2025-11-10 12:30:00'` |
| `INTERVAL` | Time duration | `INTERVAL '5' MINUTE` |

### Complex Types

| Type | Description | Example |
|------|-------------|---------|
| `ARRAY<T>` | Array of type T | `ARRAY[1, 2, 3]` |
| `MAP<K,V>` | Key-value map | `MAP['key1', 'value1']` |
| `STRUCT<...>` | Named fields | `STRUCT(name VARCHAR, age INT)` |
| `JSON` | JSON document | `'{"name": "value"}'` |

### Streaming-Specific Types

| Type | Description | Example |
|------|-------------|---------|
| `WATERMARK` | Event-time watermark | See [Watermarks](#watermarks) |
| `ROWTIME` | Event timestamp column | `timestamp TIMESTAMP ROWTIME` |
| `PROCTIME` | Processing time column | `proc_time TIMESTAMP PROCTIME` |

---

## DDL Statements

### CREATE STREAM

Define a new stream (unbounded data source).

**Syntax:**

```sql
CREATE STREAM [IF NOT EXISTS] stream_name (
    column_name data_type [ROWTIME | PROCTIME],
    ...
    [WATERMARK FOR column_name AS watermark_strategy]
)
WITH (
    'connector' = 'connector_type',
    'property' = 'value',
    ...
);
```

**Example: Kafka Stream**

```sql
CREATE STREAM feedback_events (
    event_id VARCHAR,
    user_id VARCHAR,
    model_name VARCHAR,
    rating DOUBLE,
    latency_ms BIGINT,
    cost_usd DOUBLE,
    event_time TIMESTAMP ROWTIME,
    WATERMARK FOR event_time AS event_time - INTERVAL '10' SECOND
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

**Example: File Stream**

```sql
CREATE STREAM performance_metrics (
    metric_name VARCHAR,
    metric_value DOUBLE,
    labels MAP<VARCHAR, VARCHAR>,
    timestamp TIMESTAMP ROWTIME,
    WATERMARK FOR timestamp AS timestamp - INTERVAL '5' SECOND
)
WITH (
    'connector' = 'filesystem',
    'path' = '/data/metrics',
    'format' = 'json'
);
```

### CREATE TABLE

Define a materialized table (bounded data or changelog stream).

**Syntax:**

```sql
CREATE TABLE [IF NOT EXISTS] table_name (
    column_name data_type [PRIMARY KEY],
    ...
)
WITH (
    'connector' = 'connector_type',
    ...
);
```

**Example: State Table**

```sql
CREATE TABLE model_configs (
    model_id VARCHAR PRIMARY KEY,
    temperature DOUBLE,
    max_tokens INTEGER,
    prompt_template TEXT,
    updated_at TIMESTAMP
)
WITH (
    'connector' = 'postgres',
    'url' = 'postgresql://localhost:5432/optimizer',
    'table-name' = 'model_configs'
);
```

### CREATE VIEW

Create a logical view over streams or tables.

**Syntax:**

```sql
CREATE VIEW view_name AS
SELECT ...
FROM ...;
```

**Example:**

```sql
CREATE VIEW high_latency_events AS
SELECT
    event_id,
    user_id,
    model_name,
    latency_ms,
    event_time
FROM feedback_events
WHERE latency_ms > 5000;
```

### DROP

Remove a stream, table, or view.

**Syntax:**

```sql
DROP STREAM [IF EXISTS] stream_name;
DROP TABLE [IF EXISTS] table_name;
DROP VIEW [IF EXISTS] view_name;
```

### ALTER

Modify stream or table properties.

**Syntax:**

```sql
ALTER STREAM stream_name SET (
    'property' = 'new_value'
);
```

---

## DML Statements

### SELECT

Query data from streams or tables.

**Syntax:**

```sql
SELECT [DISTINCT] column_list
FROM source
[WHERE condition]
[GROUP BY grouping_columns]
[HAVING condition]
[ORDER BY sort_columns]
[LIMIT n];
```

See [Query Clauses](#query-clauses) for detailed information.

### INSERT

Insert query results into a stream or table.

**Syntax:**

```sql
INSERT INTO target_stream
SELECT ...
FROM source_stream;
```

**Example:**

```sql
INSERT INTO aggregated_metrics
SELECT
    model_name,
    TUMBLE_START(event_time, INTERVAL '5' MINUTE) as window_start,
    AVG(latency_ms) as avg_latency,
    COUNT(*) as request_count
FROM feedback_events
GROUP BY
    model_name,
    TUMBLE(event_time, INTERVAL '5' MINUTE);
```

### UPDATE

Update records in a table (not supported for streams).

**Syntax:**

```sql
UPDATE table_name
SET column = value
WHERE condition;
```

### DELETE

Delete records from a table (not supported for streams).

**Syntax:**

```sql
DELETE FROM table_name
WHERE condition;
```

---

## Query Clauses

### FROM

Specify the data source(s).

**Single Source:**

```sql
SELECT * FROM feedback_events;
```

**Multiple Sources (Join):**

```sql
SELECT
    f.event_id,
    m.temperature
FROM feedback_events f
JOIN model_configs m ON f.model_id = m.model_id;
```

**Subquery:**

```sql
SELECT *
FROM (
    SELECT model_name, AVG(latency_ms) as avg_latency
    FROM feedback_events
    GROUP BY model_name
) AS aggregated;
```

### WHERE

Filter rows based on conditions.

**Syntax:**

```sql
WHERE condition
```

**Supported Operators:**

- Comparison: `=`, `!=`, `<>`, `<`, `>`, `<=`, `>=`
- Logical: `AND`, `OR`, `NOT`
- Pattern matching: `LIKE`, `ILIKE`, `SIMILAR TO`, `REGEXP`
- Null checks: `IS NULL`, `IS NOT NULL`
- Set membership: `IN`, `NOT IN`, `EXISTS`
- Range: `BETWEEN ... AND ...`

**Examples:**

```sql
-- Simple condition
WHERE latency_ms > 1000

-- Multiple conditions
WHERE model_name = 'gpt-4'
  AND rating >= 4.0
  AND cost_usd < 0.01

-- Pattern matching
WHERE user_id LIKE 'user_%'

-- Range check
WHERE event_time BETWEEN
    TIMESTAMP '2025-11-10 00:00:00'
    AND TIMESTAMP '2025-11-10 23:59:59'

-- Array membership
WHERE model_name IN ('gpt-4', 'claude-3', 'llama-2')
```

### GROUP BY

Group rows for aggregation.

**Syntax:**

```sql
GROUP BY column1, column2, ...
```

**Examples:**

```sql
-- Simple grouping
SELECT model_name, AVG(latency_ms)
FROM feedback_events
GROUP BY model_name;

-- Multiple columns
SELECT model_name, user_id, COUNT(*)
FROM feedback_events
GROUP BY model_name, user_id;

-- With expressions
SELECT
    DATE_TRUNC('hour', event_time) as hour,
    COUNT(*) as events_per_hour
FROM feedback_events
GROUP BY DATE_TRUNC('hour', event_time);
```

### HAVING

Filter groups after aggregation.

**Syntax:**

```sql
HAVING condition
```

**Example:**

```sql
SELECT
    model_name,
    AVG(latency_ms) as avg_latency
FROM feedback_events
GROUP BY model_name
HAVING AVG(latency_ms) > 2000;
```

### ORDER BY

Sort results.

**Syntax:**

```sql
ORDER BY column [ASC | DESC] [NULLS FIRST | NULLS LAST], ...
```

**Example:**

```sql
SELECT model_name, AVG(latency_ms) as avg_latency
FROM feedback_events
GROUP BY model_name
ORDER BY avg_latency DESC;
```

**Note:** `ORDER BY` on unbounded streams requires caution. Use with `LIMIT` or within windows.

### LIMIT

Restrict the number of rows returned.

**Syntax:**

```sql
LIMIT n [OFFSET m]
```

**Example:**

```sql
-- Top 10 highest latency events
SELECT * FROM feedback_events
ORDER BY latency_ms DESC
LIMIT 10;

-- Pagination
SELECT * FROM feedback_events
ORDER BY event_time
LIMIT 100 OFFSET 200;
```

---

## Window Functions

Window functions enable time-based aggregations over streaming data.

### TUMBLE (Tumbling Window)

Fixed-size, non-overlapping windows.

**Syntax:**

```sql
TUMBLE(time_column, INTERVAL 'n' unit)
```

**Helper Functions:**

- `TUMBLE_START(time_column, interval)`: Window start time
- `TUMBLE_END(time_column, interval)`: Window end time
- `TUMBLE_ROWTIME(time_column, interval)`: Window rowtime (for cascading)
- `TUMBLE_PROCTIME(time_column, interval)`: Window processing time

**Example:**

```sql
SELECT
    model_name,
    TUMBLE_START(event_time, INTERVAL '5' MINUTE) as window_start,
    TUMBLE_END(event_time, INTERVAL '5' MINUTE) as window_end,
    COUNT(*) as event_count,
    AVG(latency_ms) as avg_latency,
    MAX(latency_ms) as max_latency,
    SUM(cost_usd) as total_cost
FROM feedback_events
GROUP BY
    model_name,
    TUMBLE(event_time, INTERVAL '5' MINUTE);
```

**Visualization:**

```
Time:    0    5    10   15   20   25   30
         |----|----|----|----|----|----|
Windows: [0-5)[5-10)[10-15)[15-20)[20-25)[25-30)
```

### HOP (Hopping/Sliding Window)

Fixed-size, overlapping windows.

**Syntax:**

```sql
HOP(time_column, INTERVAL 'slide' unit, INTERVAL 'size' unit)
```

**Helper Functions:**

- `HOP_START(time_column, slide, size)`: Window start time
- `HOP_END(time_column, slide, size)`: Window end time
- `HOP_ROWTIME(time_column, slide, size)`: Window rowtime
- `HOP_PROCTIME(time_column, slide, size)`: Window processing time

**Example:**

```sql
-- 15-minute windows sliding every 5 minutes
SELECT
    model_name,
    HOP_START(event_time, INTERVAL '5' MINUTE, INTERVAL '15' MINUTE) as window_start,
    HOP_END(event_time, INTERVAL '5' MINUTE, INTERVAL '15' MINUTE) as window_end,
    AVG(latency_ms) as avg_latency,
    STDDEV(latency_ms) as stddev_latency
FROM feedback_events
GROUP BY
    model_name,
    HOP(event_time, INTERVAL '5' MINUTE, INTERVAL '15' MINUTE);
```

**Visualization:**

```
Time:    0    5    10   15   20
         |----|----|----|----|
Windows: [0--------15)
              [5--------20)
                   [10-------25)
```

### SESSION (Session Window)

Dynamic windows based on inactivity gaps.

**Syntax:**

```sql
SESSION(time_column, INTERVAL 'gap' unit)
```

**Helper Functions:**

- `SESSION_START(time_column, gap)`: Session start time
- `SESSION_END(time_column, gap)`: Session end time
- `SESSION_ROWTIME(time_column, gap)`: Session rowtime
- `SESSION_PROCTIME(time_column, gap)`: Session processing time

**Example:**

```sql
-- Group user interactions into sessions with 30-minute inactivity gap
SELECT
    user_id,
    SESSION_START(event_time, INTERVAL '30' MINUTE) as session_start,
    SESSION_END(event_time, INTERVAL '30' MINUTE) as session_end,
    COUNT(*) as interactions_count,
    SUM(cost_usd) as session_cost,
    ARRAY_AGG(model_name) as models_used
FROM feedback_events
GROUP BY
    user_id,
    SESSION(event_time, INTERVAL '30' MINUTE);
```

**Visualization:**

```
Events: E1   E2       E3   E4              E5   E6
Time:   0    5        15   20              50   55
Gap:    30 minutes

Session1: [E1, E2, E3, E4] (0-50)
Session2: [E5, E6] (50-85)
```

### CUMULATE (Cumulative Window)

Windows that grow incrementally (useful for early results).

**Syntax:**

```sql
CUMULATE(time_column, INTERVAL 'step' unit, INTERVAL 'max_size' unit)
```

**Example:**

```sql
-- Daily windows that emit every hour
SELECT
    DATE(event_time) as date,
    CUMULATE_START(event_time, INTERVAL '1' HOUR, INTERVAL '1' DAY) as window_start,
    CUMULATE_END(event_time, INTERVAL '1' HOUR, INTERVAL '1' DAY) as window_end,
    COUNT(*) as cumulative_count,
    SUM(cost_usd) as cumulative_cost
FROM feedback_events
GROUP BY
    DATE(event_time),
    CUMULATE(event_time, INTERVAL '1' HOUR, INTERVAL '1' DAY);
```

---

## Aggregation Functions

### Count Functions

**COUNT(*)**

Count all rows (including nulls).

```sql
SELECT COUNT(*) FROM feedback_events;
```

**COUNT(column)**

Count non-null values.

```sql
SELECT COUNT(rating) FROM feedback_events;
```

**COUNT(DISTINCT column)**

Count distinct non-null values.

```sql
SELECT COUNT(DISTINCT user_id) FROM feedback_events;
```

### Numeric Aggregations

**SUM(column)**

Sum of all values.

```sql
SELECT SUM(cost_usd) as total_cost FROM feedback_events;
```

**AVG(column)**

Average of all values.

```sql
SELECT AVG(latency_ms) as avg_latency FROM feedback_events;
```

**MIN(column) / MAX(column)**

Minimum and maximum values.

```sql
SELECT
    MIN(latency_ms) as min_latency,
    MAX(latency_ms) as max_latency
FROM feedback_events;
```

**STDDEV(column) / STDDEV_POP(column) / STDDEV_SAMP(column)**

Standard deviation (population or sample).

```sql
SELECT
    STDDEV(latency_ms) as stddev_latency,
    STDDEV_POP(latency_ms) as stddev_pop,
    STDDEV_SAMP(latency_ms) as stddev_samp
FROM feedback_events;
```

**VARIANCE(column) / VAR_POP(column) / VAR_SAMP(column)**

Variance (population or sample).

```sql
SELECT
    VARIANCE(latency_ms) as variance,
    VAR_POP(latency_ms) as var_pop,
    VAR_SAMP(latency_ms) as var_samp
FROM feedback_events;
```

### Percentile Functions

**PERCENTILE_CONT(percentile) WITHIN GROUP (ORDER BY column)**

Continuous percentile (interpolated).

```sql
SELECT
    PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY latency_ms) as p50,
    PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY latency_ms) as p95,
    PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY latency_ms) as p99
FROM feedback_events;
```

**PERCENTILE_DISC(percentile) WITHIN GROUP (ORDER BY column)**

Discrete percentile (actual value).

```sql
SELECT
    PERCENTILE_DISC(0.5) WITHIN GROUP (ORDER BY latency_ms) as median
FROM feedback_events;
```

**APPROX_PERCENTILE(column, percentile [, accuracy])**

Approximate percentile (faster, uses t-digest algorithm).

```sql
SELECT
    APPROX_PERCENTILE(latency_ms, 0.5) as p50,
    APPROX_PERCENTILE(latency_ms, 0.95) as p95,
    APPROX_PERCENTILE(latency_ms, 0.99, 0.01) as p99
FROM feedback_events;
```

### Collection Aggregations

**ARRAY_AGG(column [ORDER BY ...])**

Collect values into an array.

```sql
SELECT
    user_id,
    ARRAY_AGG(model_name ORDER BY event_time) as models_used
FROM feedback_events
GROUP BY user_id;
```

**STRING_AGG(column, delimiter [ORDER BY ...])**

Concatenate strings with delimiter.

```sql
SELECT
    user_id,
    STRING_AGG(model_name, ', ' ORDER BY event_time) as models_list
FROM feedback_events
GROUP BY user_id;
```

**MAP_AGG(key_column, value_column)**

Collect key-value pairs into a map.

```sql
SELECT
    user_id,
    MAP_AGG(model_name, cost_usd) as cost_by_model
FROM feedback_events
GROUP BY user_id;
```

### Statistical Aggregations

**CORR(column1, column2)**

Pearson correlation coefficient.

```sql
SELECT CORR(latency_ms, cost_usd) as latency_cost_correlation
FROM feedback_events;
```

**COVAR_POP(column1, column2) / COVAR_SAMP(column1, column2)**

Covariance (population or sample).

```sql
SELECT COVAR_POP(latency_ms, rating) FROM feedback_events;
```

**REGR_SLOPE(y, x) / REGR_INTERCEPT(y, x)**

Linear regression slope and intercept.

```sql
SELECT
    REGR_SLOPE(cost_usd, latency_ms) as slope,
    REGR_INTERCEPT(cost_usd, latency_ms) as intercept
FROM feedback_events;
```

### First/Last Value

**FIRST_VALUE(column)**

First value in the group.

```sql
SELECT
    model_name,
    FIRST_VALUE(rating ORDER BY event_time) as first_rating
FROM feedback_events
GROUP BY model_name;
```

**LAST_VALUE(column)**

Last value in the group.

```sql
SELECT
    model_name,
    LAST_VALUE(rating ORDER BY event_time) as last_rating
FROM feedback_events
GROUP BY model_name;
```

---

## Scalar Functions

### String Functions

| Function | Description | Example |
|----------|-------------|---------|
| `UPPER(str)` | Convert to uppercase | `UPPER('hello')` → `'HELLO'` |
| `LOWER(str)` | Convert to lowercase | `LOWER('HELLO')` → `'hello'` |
| `LENGTH(str)` | String length | `LENGTH('hello')` → `5` |
| `SUBSTRING(str, start [, length])` | Extract substring | `SUBSTRING('hello', 2, 3)` → `'ell'` |
| `CONCAT(str1, str2, ...)` | Concatenate strings | `CONCAT('hello', ' ', 'world')` → `'hello world'` |
| `TRIM([chars FROM] str)` | Remove leading/trailing chars | `TRIM(' hello ')` → `'hello'` |
| `LTRIM(str)` | Remove leading whitespace | `LTRIM('  hello')` → `'hello'` |
| `RTRIM(str)` | Remove trailing whitespace | `RTRIM('hello  ')` → `'hello'` |
| `REPLACE(str, from, to)` | Replace substring | `REPLACE('hello', 'l', 'r')` → `'herro'` |
| `SPLIT(str, delimiter)` | Split into array | `SPLIT('a,b,c', ',')` → `['a','b','c']` |
| `REGEXP_MATCH(str, pattern)` | Regex match | `REGEXP_MATCH('user_123', '\d+')` → `['123']` |
| `REGEXP_REPLACE(str, pattern, replacement)` | Regex replace | `REGEXP_REPLACE('user_123', '\d+', 'XXX')` → `'user_XXX'` |

### Mathematical Functions

| Function | Description | Example |
|----------|-------------|---------|
| `ABS(x)` | Absolute value | `ABS(-5)` → `5` |
| `CEIL(x)` / `CEILING(x)` | Round up | `CEIL(4.3)` → `5` |
| `FLOOR(x)` | Round down | `FLOOR(4.7)` → `4` |
| `ROUND(x [, decimals])` | Round to decimals | `ROUND(4.567, 2)` → `4.57` |
| `TRUNC(x [, decimals])` | Truncate | `TRUNC(4.567, 2)` → `4.56` |
| `SQRT(x)` | Square root | `SQRT(16)` → `4` |
| `POW(x, y)` / `POWER(x, y)` | Power | `POW(2, 3)` → `8` |
| `EXP(x)` | Exponential | `EXP(1)` → `2.718...` |
| `LN(x)` | Natural log | `LN(2.718)` → `1` |
| `LOG10(x)` | Base-10 log | `LOG10(100)` → `2` |
| `MOD(x, y)` | Modulo | `MOD(10, 3)` → `1` |
| `SIGN(x)` | Sign (-1, 0, 1) | `SIGN(-5)` → `-1` |
| `RANDOM()` | Random 0-1 | `RANDOM()` → `0.734...` |

### Date/Time Functions

| Function | Description | Example |
|----------|-------------|---------|
| `NOW()` / `CURRENT_TIMESTAMP` | Current timestamp | `NOW()` → `'2025-11-10 12:30:00'` |
| `CURRENT_DATE` | Current date | `CURRENT_DATE` → `'2025-11-10'` |
| `CURRENT_TIME` | Current time | `CURRENT_TIME` → `'12:30:00'` |
| `DATE(timestamp)` | Extract date | `DATE('2025-11-10 12:30:00')` → `'2025-11-10'` |
| `TIME(timestamp)` | Extract time | `TIME('2025-11-10 12:30:00')` → `'12:30:00'` |
| `YEAR(timestamp)` | Extract year | `YEAR('2025-11-10')` → `2025` |
| `MONTH(timestamp)` | Extract month | `MONTH('2025-11-10')` → `11` |
| `DAY(timestamp)` | Extract day | `DAY('2025-11-10')` → `10` |
| `HOUR(timestamp)` | Extract hour | `HOUR('12:30:00')` → `12` |
| `MINUTE(timestamp)` | Extract minute | `MINUTE('12:30:00')` → `30` |
| `SECOND(timestamp)` | Extract second | `SECOND('12:30:45')` → `45` |
| `DATE_TRUNC(unit, timestamp)` | Truncate to unit | `DATE_TRUNC('hour', ts)` |
| `DATE_ADD(timestamp, interval)` | Add interval | `DATE_ADD(ts, INTERVAL '1' DAY)` |
| `DATE_SUB(timestamp, interval)` | Subtract interval | `DATE_SUB(ts, INTERVAL '1' HOUR)` |
| `DATE_DIFF(unit, ts1, ts2)` | Difference in units | `DATE_DIFF('day', ts1, ts2)` |
| `TO_TIMESTAMP(str [, format])` | Parse timestamp | `TO_TIMESTAMP('2025-11-10', 'YYYY-MM-DD')` |
| `TO_CHAR(timestamp, format)` | Format timestamp | `TO_CHAR(NOW(), 'YYYY-MM-DD')` |

### Conditional Functions

| Function | Description | Example |
|----------|-------------|---------|
| `COALESCE(val1, val2, ...)` | First non-null value | `COALESCE(NULL, NULL, 'default')` → `'default'` |
| `NULLIF(val1, val2)` | NULL if equal | `NULLIF(5, 5)` → `NULL` |
| `CASE` | Conditional expression | See examples below |

**CASE Examples:**

```sql
-- Simple CASE
SELECT
    model_name,
    CASE model_name
        WHEN 'gpt-4' THEN 'OpenAI'
        WHEN 'claude-3' THEN 'Anthropic'
        ELSE 'Other'
    END as provider
FROM feedback_events;

-- Searched CASE
SELECT
    latency_ms,
    CASE
        WHEN latency_ms < 1000 THEN 'Fast'
        WHEN latency_ms < 5000 THEN 'Medium'
        ELSE 'Slow'
    END as latency_category
FROM feedback_events;
```

### Type Conversion Functions

| Function | Description | Example |
|----------|-------------|---------|
| `CAST(value AS type)` | Convert type | `CAST('123' AS INTEGER)` → `123` |
| `TRY_CAST(value AS type)` | Safe cast (NULL on error) | `TRY_CAST('abc' AS INTEGER)` → `NULL` |
| `TO_JSON(value)` | Convert to JSON | `TO_JSON(array_col)` |
| `FROM_JSON(json, schema)` | Parse JSON | `FROM_JSON('{"a":1}', 'a INT')` |

### JSON Functions

| Function | Description | Example |
|----------|-------------|---------|
| `JSON_VALUE(json, path)` | Extract scalar value | `JSON_VALUE('{"a":1}', '$.a')` → `1` |
| `JSON_QUERY(json, path)` | Extract object/array | `JSON_QUERY('{"a":[1,2]}', '$.a')` → `[1,2]` |
| `JSON_EXISTS(json, path)` | Check if path exists | `JSON_EXISTS('{"a":1}', '$.a')` → `TRUE` |
| `JSON_OBJECT(...)` | Create JSON object | `JSON_OBJECT('key', 'value')` → `'{"key":"value"}'` |
| `JSON_ARRAY(...)` | Create JSON array | `JSON_ARRAY(1, 2, 3)` → `'[1,2,3]'` |

---

## Join Operations

### INNER JOIN

Returns rows with matching keys in both streams.

**Syntax:**

```sql
SELECT ...
FROM stream1
INNER JOIN stream2
ON stream1.key = stream2.key;
```

**Example:**

```sql
SELECT
    f.event_id,
    f.latency_ms,
    m.temperature,
    m.max_tokens
FROM feedback_events f
INNER JOIN model_configs m
ON f.model_id = m.model_id;
```

### LEFT JOIN

Returns all rows from left stream, with nulls for non-matching right rows.

**Syntax:**

```sql
SELECT ...
FROM stream1
LEFT JOIN stream2
ON stream1.key = stream2.key;
```

**Example:**

```sql
-- Include events even if model config not found
SELECT
    f.event_id,
    f.latency_ms,
    COALESCE(m.temperature, 0.7) as temperature
FROM feedback_events f
LEFT JOIN model_configs m
ON f.model_id = m.model_id;
```

### RIGHT JOIN

Returns all rows from right stream, with nulls for non-matching left rows.

**Syntax:**

```sql
SELECT ...
FROM stream1
RIGHT JOIN stream2
ON stream1.key = stream2.key;
```

### FULL OUTER JOIN

Returns all rows from both streams, with nulls for non-matching rows.

**Syntax:**

```sql
SELECT ...
FROM stream1
FULL OUTER JOIN stream2
ON stream1.key = stream2.key;
```

### Time-Bounded Joins

For stream-to-stream joins, specify time bounds to prevent unbounded state growth.

**Syntax:**

```sql
SELECT ...
FROM stream1 s1
JOIN stream2 s2
ON s1.key = s2.key
AND s1.event_time BETWEEN s2.event_time - INTERVAL '10' MINUTE
                      AND s2.event_time + INTERVAL '10' MINUTE;
```

**Example:**

```sql
-- Join feedback with performance metrics within 1-minute window
SELECT
    f.event_id,
    f.rating,
    p.cpu_usage,
    p.memory_usage
FROM feedback_events f
JOIN performance_metrics p
ON f.model_id = p.model_id
AND f.event_time BETWEEN p.timestamp - INTERVAL '1' MINUTE
                     AND p.timestamp + INTERVAL '1' MINUTE;
```

### Interval Joins

Specialized syntax for time-bounded joins.

**Syntax:**

```sql
SELECT ...
FROM stream1 s1
INTERVAL JOIN stream2 s2
ON s1.key = s2.key
WITHIN INTERVAL 'duration';
```

**Example:**

```sql
SELECT
    f.event_id,
    f.rating,
    m.metric_value
FROM feedback_events f
INTERVAL JOIN performance_metrics m
ON f.model_id = m.model_id
WITHIN INTERVAL '5' MINUTE;
```

---

## Time Semantics

### Event Time vs Processing Time

**Event Time**: The time when the event actually occurred (embedded in data).

**Processing Time**: The time when the event is processed by the system.

**Example:**

```sql
CREATE STREAM events (
    event_id VARCHAR,
    event_time TIMESTAMP ROWTIME,      -- Event time
    proc_time TIMESTAMP PROCTIME,      -- Processing time
    ...
);
```

### Watermarks

Watermarks track progress in event time and handle late-arriving events.

**Syntax:**

```sql
WATERMARK FOR time_column AS watermark_strategy
```

**Strategies:**

1. **Bounded Out-of-Orderness**

```sql
WATERMARK FOR event_time AS event_time - INTERVAL '10' SECOND
```

Allows events up to 10 seconds late.

2. **Monotonous Timestamps**

```sql
WATERMARK FOR event_time AS event_time
```

Assumes events arrive in order.

3. **Custom Strategy**

```sql
WATERMARK FOR event_time AS event_time - INTERVAL '1' MINUTE * 0.95
```

### Allowed Lateness

Specify how long to keep window state for late events.

**Example:**

```sql
SELECT
    model_name,
    TUMBLE_START(event_time, INTERVAL '5' MINUTE) as window_start,
    COUNT(*) as event_count
FROM feedback_events
GROUP BY
    model_name,
    TUMBLE(event_time, INTERVAL '5' MINUTE)
EMIT AFTER WATERMARK + INTERVAL '2' MINUTE;
```

### Emit Strategies

Control when to emit results from windows.

**EMIT AFTER WATERMARK**

Emit once watermark passes window end (default).

```sql
EMIT AFTER WATERMARK
```

**EMIT BEFORE WATERMARK**

Emit early results before window closes.

```sql
EMIT BEFORE WATERMARK EVERY INTERVAL '30' SECOND
```

**EMIT AFTER WATERMARK WITH DELAY**

Emit after window closes plus delay (for late data).

```sql
EMIT AFTER WATERMARK + INTERVAL '1' MINUTE
```

**EMIT ON UPDATE**

Emit every time window is updated (retraction mode).

```sql
EMIT ON UPDATE
```

---

## System Tables

### SYSTEM.STREAMS

List all defined streams.

```sql
SELECT * FROM SYSTEM.STREAMS;
```

**Columns:**
- `stream_name`: Name of the stream
- `schema`: Column definitions
- `connector`: Connector type
- `properties`: Connector properties
- `created_at`: Creation timestamp

### SYSTEM.TABLES

List all defined tables.

```sql
SELECT * FROM SYSTEM.TABLES;
```

### SYSTEM.VIEWS

List all views.

```sql
SELECT * FROM SYSTEM.VIEWS;
```

### SYSTEM.JOBS

List running stream processing jobs.

```sql
SELECT * FROM SYSTEM.JOBS;
```

**Columns:**
- `job_id`: Unique job identifier
- `query`: SQL query text
- `status`: Running, Paused, Failed
- `start_time`: Job start time
- `records_processed`: Total records processed
- `errors`: Error count

### SYSTEM.METRICS

Query system metrics.

```sql
SELECT
    metric_name,
    metric_value,
    labels
FROM SYSTEM.METRICS
WHERE metric_name = 'records_processed_total';
```

### SYSTEM.WATERMARKS

View current watermarks for all streams.

```sql
SELECT
    stream_name,
    partition,
    watermark_timestamp,
    latest_event_timestamp
FROM SYSTEM.WATERMARKS;
```

---

## Complete Example: Real-time LLM Monitoring

```sql
-- 1. Create feedback events stream
CREATE STREAM feedback_events (
    event_id VARCHAR,
    user_id VARCHAR,
    model_id VARCHAR,
    model_name VARCHAR,
    prompt_template VARCHAR,
    rating DOUBLE,
    latency_ms BIGINT,
    cost_usd DOUBLE,
    error_code VARCHAR,
    event_time TIMESTAMP ROWTIME,
    WATERMARK FOR event_time AS event_time - INTERVAL '30' SECOND
)
WITH (
    'connector' = 'kafka',
    'topic' = 'llm-feedback',
    'properties.bootstrap.servers' = 'localhost:9092',
    'format' = 'json'
);

-- 2. Create model configurations table
CREATE TABLE model_configs (
    model_id VARCHAR PRIMARY KEY,
    temperature DOUBLE,
    max_tokens INTEGER,
    cost_per_1k_tokens DOUBLE
)
WITH (
    'connector' = 'postgres',
    'table-name' = 'model_configs'
);

-- 3. Create real-time aggregations stream
CREATE STREAM model_performance AS
SELECT
    model_name,
    TUMBLE_START(event_time, INTERVAL '5' MINUTE) as window_start,
    TUMBLE_END(event_time, INTERVAL '5' MINUTE) as window_end,
    COUNT(*) as request_count,
    COUNT(DISTINCT user_id) as unique_users,

    -- Latency metrics
    AVG(latency_ms) as avg_latency_ms,
    PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY latency_ms) as p50_latency_ms,
    PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY latency_ms) as p95_latency_ms,
    PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY latency_ms) as p99_latency_ms,
    STDDEV(latency_ms) as stddev_latency_ms,

    -- Cost metrics
    SUM(cost_usd) as total_cost_usd,
    AVG(cost_usd) as avg_cost_per_request,

    -- Quality metrics
    AVG(rating) as avg_rating,
    COUNT(CASE WHEN rating >= 4.0 THEN 1 END) * 100.0 / COUNT(*) as satisfaction_rate,

    -- Error metrics
    COUNT(CASE WHEN error_code IS NOT NULL THEN 1 END) as error_count,
    COUNT(CASE WHEN error_code IS NOT NULL THEN 1 END) * 100.0 / COUNT(*) as error_rate
FROM feedback_events
GROUP BY
    model_name,
    TUMBLE(event_time, INTERVAL '5' MINUTE)
EMIT AFTER WATERMARK;

-- 4. Detect anomalies with sliding windows
CREATE VIEW anomaly_detection AS
SELECT
    model_name,
    window_start,
    avg_latency_ms,
    stddev_latency_ms,
    CASE
        WHEN avg_latency_ms > LAG(avg_latency_ms, 1) OVER (PARTITION BY model_name ORDER BY window_start) * 2 THEN 'LATENCY_SPIKE'
        WHEN error_rate > 5.0 THEN 'HIGH_ERROR_RATE'
        WHEN avg_rating < 3.0 THEN 'LOW_SATISFACTION'
        ELSE NULL
    END as anomaly_type
FROM (
    SELECT
        model_name,
        HOP_START(event_time, INTERVAL '1' MINUTE, INTERVAL '15' MINUTE) as window_start,
        AVG(latency_ms) as avg_latency_ms,
        STDDEV(latency_ms) as stddev_latency_ms,
        AVG(rating) as avg_rating,
        COUNT(CASE WHEN error_code IS NOT NULL THEN 1 END) * 100.0 / COUNT(*) as error_rate
    FROM feedback_events
    GROUP BY
        model_name,
        HOP(event_time, INTERVAL '1' MINUTE, INTERVAL '15' MINUTE)
)
WHERE anomaly_type IS NOT NULL;

-- 5. User session analysis
CREATE VIEW user_sessions AS
SELECT
    user_id,
    SESSION_START(event_time, INTERVAL '30' MINUTE) as session_start,
    SESSION_END(event_time, INTERVAL '30' MINUTE) as session_end,
    COUNT(*) as interactions_count,
    SUM(cost_usd) as session_cost,
    AVG(rating) as avg_rating,
    ARRAY_AGG(model_name ORDER BY event_time) as models_used,
    MAX(latency_ms) as max_latency_ms
FROM feedback_events
GROUP BY
    user_id,
    SESSION(event_time, INTERVAL '30' MINUTE);
```

---

## Next Steps

- [SQL Tutorial](SQL_TUTORIAL.md): Step-by-step guide to using the SQL interface
- [SQL Examples](SQL_EXAMPLES.md): 50+ practical query examples
- [SQL Best Practices](SQL_BEST_PRACTICES.md): Performance optimization tips
- [SQL API Reference](SQL_API_REFERENCE.md): Rust API for embedding SQL engine
