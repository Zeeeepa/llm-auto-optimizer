# SQL Examples

**LLM Auto Optimizer - SQL Interface**
**Version:** 1.0
**Last Updated:** 2025-11-10

---

## Table of Contents

1. [Basic Queries](#basic-queries)
2. [Filtering and Projection](#filtering-and-projection)
3. [Simple Aggregations](#simple-aggregations)
4. [Tumbling Window Aggregations](#tumbling-window-aggregations)
5. [Sliding Window Aggregations](#sliding-window-aggregations)
6. [Session Window Aggregations](#session-window-aggregations)
7. [Joins](#joins)
8. [Advanced Analytics](#advanced-analytics)
9. [Real-World Use Cases](#real-world-use-cases)
10. [Optimization and Cost Analysis](#optimization-and-cost-analysis)

---

## Basic Queries

### Example 1: Select All Events

```sql
-- Stream all feedback events
SELECT * FROM feedback_events;
```

**Use Case:** Debug stream connectivity and data format.

---

### Example 2: Limit Results

```sql
-- Get first 100 events
SELECT * FROM feedback_events
LIMIT 100;
```

**Use Case:** Sample data for exploration.

---

### Example 3: Select Specific Columns

```sql
-- Get only key fields
SELECT
    event_id,
    model_name,
    latency_ms,
    cost_usd,
    event_time
FROM feedback_events;
```

**Use Case:** Reduce network bandwidth for large streams.

---

### Example 4: Column Aliases

```sql
-- Use friendly column names
SELECT
    event_id as id,
    model_name as model,
    latency_ms as latency,
    cost_usd * 1000 as cost_per_thousand_requests
FROM feedback_events;
```

**Use Case:** Create cleaner output for dashboards.

---

## Filtering and Projection

### Example 5: Filter by Model

```sql
-- Get events for specific model
SELECT *
FROM feedback_events
WHERE model_name = 'gpt-4';
```

**Use Case:** Monitor a specific LLM model.

---

### Example 6: Filter by Latency Threshold

```sql
-- Find slow requests (> 5 seconds)
SELECT
    event_id,
    user_id,
    model_name,
    latency_ms,
    event_time
FROM feedback_events
WHERE latency_ms > 5000;
```

**Use Case:** Identify performance issues.

---

### Example 7: Multiple Conditions

```sql
-- High latency AND low rating
SELECT
    event_id,
    model_name,
    latency_ms,
    rating,
    cost_usd
FROM feedback_events
WHERE latency_ms > 3000
  AND rating < 3.0;
```

**Use Case:** Correlate performance with user satisfaction.

---

### Example 8: Range Filter

```sql
-- Medium latency requests (1-3 seconds)
SELECT *
FROM feedback_events
WHERE latency_ms BETWEEN 1000 AND 3000;
```

**Use Case:** Analyze requests in acceptable latency range.

---

### Example 9: IN Clause

```sql
-- Compare specific models
SELECT
    model_name,
    latency_ms,
    cost_usd,
    rating
FROM feedback_events
WHERE model_name IN ('gpt-4', 'claude-3-opus', 'gemini-pro');
```

**Use Case:** A/B test comparison.

---

### Example 10: Pattern Matching

```sql
-- Find all GPT models
SELECT DISTINCT model_name
FROM feedback_events
WHERE model_name LIKE 'gpt-%';
```

**Use Case:** Group models by provider.

---

### Example 11: Date Range Filter

```sql
-- Events from last hour
SELECT *
FROM feedback_events
WHERE event_time >= NOW() - INTERVAL '1' HOUR;
```

**Use Case:** Recent activity monitoring.

---

### Example 12: NULL Handling

```sql
-- Find events with errors
SELECT
    event_id,
    model_name,
    error_code,
    error_message,
    event_time
FROM feedback_events
WHERE error_code IS NOT NULL;
```

**Use Case:** Error tracking and debugging.

---

## Simple Aggregations

### Example 13: Count Total Events

```sql
-- Total events processed
SELECT COUNT(*) as total_events
FROM feedback_events;
```

**Use Case:** Monitor throughput.

---

### Example 14: Count by Model

```sql
-- Events per model
SELECT
    model_name,
    COUNT(*) as event_count
FROM feedback_events
GROUP BY model_name
ORDER BY event_count DESC;
```

**Use Case:** Model usage statistics.

---

### Example 15: Average Latency

```sql
-- Average latency per model
SELECT
    model_name,
    AVG(latency_ms) as avg_latency,
    MIN(latency_ms) as min_latency,
    MAX(latency_ms) as max_latency
FROM feedback_events
GROUP BY model_name
ORDER BY avg_latency;
```

**Use Case:** Performance benchmarking.

---

### Example 16: Sum of Costs

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

**Use Case:** Cost attribution and budgeting.

---

### Example 17: Standard Deviation

```sql
-- Latency variability
SELECT
    model_name,
    AVG(latency_ms) as avg_latency,
    STDDEV(latency_ms) as stddev_latency,
    STDDEV(latency_ms) / AVG(latency_ms) * 100 as coefficient_of_variation
FROM feedback_events
GROUP BY model_name;
```

**Use Case:** Assess latency consistency.

---

### Example 18: Percentiles

```sql
-- Latency percentiles by model
SELECT
    model_name,
    APPROX_PERCENTILE(latency_ms, 0.5) as p50,
    APPROX_PERCENTILE(latency_ms, 0.95) as p95,
    APPROX_PERCENTILE(latency_ms, 0.99) as p99
FROM feedback_events
GROUP BY model_name;
```

**Use Case:** SLA monitoring (p95, p99).

---

### Example 19: HAVING Clause

```sql
-- Models with high average cost
SELECT
    model_name,
    AVG(cost_usd) as avg_cost,
    COUNT(*) as requests
FROM feedback_events
GROUP BY model_name
HAVING AVG(cost_usd) > 0.01
ORDER BY avg_cost DESC;
```

**Use Case:** Identify expensive models.

---

### Example 20: Count Distinct Users

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

**Use Case:** User engagement metrics.

---

## Tumbling Window Aggregations

### Example 21: 5-Minute Window Aggregation

```sql
-- Events per 5-minute window
SELECT
    TUMBLE_START(event_time, INTERVAL '5' MINUTE) as window_start,
    TUMBLE_END(event_time, INTERVAL '5' MINUTE) as window_end,
    COUNT(*) as event_count,
    AVG(latency_ms) as avg_latency
FROM feedback_events
GROUP BY TUMBLE(event_time, INTERVAL '5' MINUTE);
```

**Use Case:** Real-time monitoring dashboard.

---

### Example 22: Model Performance by Hour

```sql
-- Hourly model metrics
SELECT
    model_name,
    TUMBLE_START(event_time, INTERVAL '1' HOUR) as hour,
    COUNT(*) as requests,
    AVG(latency_ms) as avg_latency,
    SUM(cost_usd) as total_cost,
    AVG(rating) as avg_rating
FROM feedback_events
GROUP BY
    model_name,
    TUMBLE(event_time, INTERVAL '1' HOUR)
ORDER BY hour DESC, requests DESC;
```

**Use Case:** Hourly performance reports.

---

### Example 23: Error Rate by Minute

```sql
-- Real-time error rate monitoring
SELECT
    TUMBLE_START(event_time, INTERVAL '1' MINUTE) as minute,
    COUNT(*) as total_requests,
    COUNT(CASE WHEN error_code IS NOT NULL THEN 1 END) as errors,
    COUNT(CASE WHEN error_code IS NOT NULL THEN 1 END) * 100.0 / COUNT(*) as error_rate_pct
FROM feedback_events
GROUP BY TUMBLE(event_time, INTERVAL '1' MINUTE)
HAVING COUNT(CASE WHEN error_code IS NOT NULL THEN 1 END) > 0;
```

**Use Case:** Alert on error rate spikes.

---

### Example 24: Cost per Day

```sql
-- Daily cost tracking
SELECT
    TUMBLE_START(event_time, INTERVAL '1' DAY) as date,
    model_name,
    COUNT(*) as requests,
    SUM(cost_usd) as daily_cost,
    AVG(cost_usd) as avg_cost_per_request
FROM feedback_events
GROUP BY
    model_name,
    TUMBLE(event_time, INTERVAL '1' DAY)
ORDER BY date DESC, daily_cost DESC;
```

**Use Case:** Daily cost reports for finance.

---

### Example 25: Throughput (QPS) Calculation

```sql
-- Queries per second (QPS) every minute
SELECT
    model_name,
    TUMBLE_START(event_time, INTERVAL '1' MINUTE) as minute,
    COUNT(*) / 60.0 as qps
FROM feedback_events
GROUP BY
    model_name,
    TUMBLE(event_time, INTERVAL '1' MINUTE)
ORDER BY qps DESC;
```

**Use Case:** Capacity planning and scaling.

---

### Example 26: User Satisfaction Score

```sql
-- Customer satisfaction metrics (CSAT) per hour
SELECT
    TUMBLE_START(event_time, INTERVAL '1' HOUR) as hour,
    COUNT(*) as responses,
    AVG(rating) as avg_rating,
    COUNT(CASE WHEN rating >= 4.0 THEN 1 END) * 100.0 / COUNT(*) as satisfaction_pct,
    COUNT(CASE WHEN rating <= 2.0 THEN 1 END) * 100.0 / COUNT(*) as dissatisfaction_pct
FROM feedback_events
WHERE rating IS NOT NULL
GROUP BY TUMBLE(event_time, INTERVAL '1' HOUR);
```

**Use Case:** Customer satisfaction tracking.

---

## Sliding Window Aggregations

### Example 27: 15-Minute Moving Average

```sql
-- 15-minute sliding window, updated every 1 minute
SELECT
    model_name,
    HOP_START(event_time, INTERVAL '1' MINUTE, INTERVAL '15' MINUTE) as window_start,
    HOP_END(event_time, INTERVAL '1' MINUTE, INTERVAL '15' MINUTE) as window_end,
    COUNT(*) as events,
    AVG(latency_ms) as moving_avg_latency,
    STDDEV(latency_ms) as moving_stddev_latency
FROM feedback_events
GROUP BY
    model_name,
    HOP(event_time, INTERVAL '1' MINUTE, INTERVAL '15' MINUTE);
```

**Use Case:** Smooth trend detection, anomaly detection.

---

### Example 28: Latency Spike Detection

```sql
-- Detect latency spikes using sliding windows
WITH windowed_metrics AS (
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
    LAG(avg_latency, 1) OVER (PARTITION BY model_name ORDER BY window_start) as prev_avg_latency,
    CASE
        WHEN avg_latency > LAG(avg_latency, 1) OVER (PARTITION BY model_name ORDER BY window_start) * 1.5
        THEN 'SPIKE_DETECTED'
        ELSE 'NORMAL'
    END as status
FROM windowed_metrics;
```

**Use Case:** Real-time anomaly detection.

---

### Example 29: Cost Rate Trend

```sql
-- Cost per request trend (30-min window, 5-min slide)
SELECT
    model_name,
    HOP_START(event_time, INTERVAL '5' MINUTE, INTERVAL '30' MINUTE) as window_start,
    COUNT(*) as requests,
    SUM(cost_usd) as total_cost,
    SUM(cost_usd) / COUNT(*) as avg_cost_per_request,
    SUM(cost_usd) / 30.0 * 60.0 as estimated_cost_per_hour
FROM feedback_events
GROUP BY
    model_name,
    HOP(event_time, INTERVAL '5' MINUTE, INTERVAL '30' MINUTE);
```

**Use Case:** Cost forecasting and budget alerts.

---

### Example 30: Quality Score Trend

```sql
-- Quality score trend (1-hour window, 10-min slide)
SELECT
    model_name,
    HOP_START(event_time, INTERVAL '10' MINUTE, INTERVAL '1' HOUR) as window_start,
    AVG(rating) as avg_rating,
    COUNT(CASE WHEN rating >= 4 THEN 1 END) * 100.0 / COUNT(*) as satisfaction_rate,
    LAG(AVG(rating)) OVER (PARTITION BY model_name ORDER BY HOP_START(event_time, INTERVAL '10' MINUTE, INTERVAL '1' HOUR)) as prev_avg_rating
FROM feedback_events
WHERE rating IS NOT NULL
GROUP BY
    model_name,
    HOP(event_time, INTERVAL '10' MINUTE, INTERVAL '1' HOUR);
```

**Use Case:** Identify declining quality trends.

---

## Session Window Aggregations

### Example 31: User Session Analysis

```sql
-- Analyze user sessions with 30-minute inactivity gap
SELECT
    user_id,
    SESSION_START(event_time, INTERVAL '30' MINUTE) as session_start,
    SESSION_END(event_time, INTERVAL '30' MINUTE) as session_end,
    COUNT(*) as interactions,
    SUM(cost_usd) as session_cost,
    AVG(rating) as avg_rating,
    EXTRACT(EPOCH FROM (SESSION_END(event_time, INTERVAL '30' MINUTE) -
                        SESSION_START(event_time, INTERVAL '30' MINUTE))) / 60 as duration_minutes
FROM feedback_events
GROUP BY
    user_id,
    SESSION(event_time, INTERVAL '30' MINUTE)
ORDER BY interactions DESC;
```

**Use Case:** User behavior analysis, engagement metrics.

---

### Example 32: Models Used per Session

```sql
-- Track model switching within sessions
SELECT
    user_id,
    SESSION_START(event_time, INTERVAL '30' MINUTE) as session_start,
    ARRAY_AGG(DISTINCT model_name ORDER BY model_name) as models_used,
    COUNT(DISTINCT model_name) as unique_models,
    COUNT(*) as total_interactions
FROM feedback_events
GROUP BY
    user_id,
    SESSION(event_time, INTERVAL '30' MINUTE)
HAVING COUNT(DISTINCT model_name) > 1;
```

**Use Case:** Identify users comparing multiple models.

---

### Example 33: Session Cost Distribution

```sql
-- Analyze cost distribution across sessions
SELECT
    CASE
        WHEN session_cost < 0.01 THEN 'Low'
        WHEN session_cost < 0.10 THEN 'Medium'
        ELSE 'High'
    END as cost_category,
    COUNT(*) as session_count,
    AVG(session_cost) as avg_cost,
    AVG(interactions) as avg_interactions
FROM (
    SELECT
        user_id,
        SESSION(event_time, INTERVAL '30' MINUTE) as session_window,
        SUM(cost_usd) as session_cost,
        COUNT(*) as interactions
    FROM feedback_events
    GROUP BY
        user_id,
        SESSION(event_time, INTERVAL '30' MINUTE)
)
GROUP BY cost_category
ORDER BY avg_cost;
```

**Use Case:** User segmentation by spending.

---

### Example 34: Abandoned Sessions

```sql
-- Find sessions with low rating (potential churn)
SELECT
    user_id,
    SESSION_START(event_time, INTERVAL '30' MINUTE) as session_start,
    SESSION_END(event_time, INTERVAL '30' MINUTE) as session_end,
    COUNT(*) as interactions,
    AVG(rating) as avg_rating,
    SUM(CASE WHEN error_code IS NOT NULL THEN 1 ELSE 0 END) as errors
FROM feedback_events
GROUP BY
    user_id,
    SESSION(event_time, INTERVAL '30' MINUTE)
HAVING AVG(rating) < 2.5 OR SUM(CASE WHEN error_code IS NOT NULL THEN 1 ELSE 0 END) > 2;
```

**Use Case:** Identify at-risk users for retention campaigns.

---

## Joins

### Example 35: Enrich Events with Model Config

```sql
-- Join feedback with model configuration
SELECT
    f.event_id,
    f.model_name,
    f.latency_ms,
    f.cost_usd,
    m.temperature,
    m.max_tokens,
    m.prompt_template
FROM feedback_events f
INNER JOIN model_configs m
ON f.model_id = m.model_id;
```

**Use Case:** Correlate performance with configuration.

---

### Example 36: Time-Bounded Stream Join

```sql
-- Join feedback with performance metrics (within 1-minute window)
SELECT
    f.event_id,
    f.user_id,
    f.model_name,
    f.latency_ms as app_latency,
    p.cpu_usage,
    p.memory_mb,
    p.gpu_utilization
FROM feedback_events f
JOIN performance_metrics p
ON f.model_id = p.model_id
AND f.event_time BETWEEN p.timestamp - INTERVAL '30' SECOND
                     AND p.timestamp + INTERVAL '30' SECOND;
```

**Use Case:** Correlate application latency with system metrics.

---

### Example 37: Left Join for Optional Data

```sql
-- Include all events even if config not found
SELECT
    f.event_id,
    f.model_name,
    f.latency_ms,
    COALESCE(m.temperature, 0.7) as temperature,
    COALESCE(m.max_tokens, 2048) as max_tokens
FROM feedback_events f
LEFT JOIN model_configs m
ON f.model_id = m.model_id;
```

**Use Case:** Handle missing configuration gracefully.

---

### Example 38: Multi-Stream Join

```sql
-- Join feedback, configs, and user profiles
SELECT
    f.event_id,
    u.user_name,
    u.subscription_tier,
    f.model_name,
    m.cost_per_1k_tokens,
    f.latency_ms,
    f.rating
FROM feedback_events f
JOIN user_profiles u ON f.user_id = u.user_id
JOIN model_configs m ON f.model_id = m.model_id
WHERE u.subscription_tier = 'enterprise';
```

**Use Case:** Segment analysis by user tier.

---

### Example 39: Self-Join for Comparison

```sql
-- Compare consecutive requests from same user
SELECT
    f1.user_id,
    f1.event_id as first_event,
    f2.event_id as second_event,
    f1.model_name as first_model,
    f2.model_name as second_model,
    f1.rating as first_rating,
    f2.rating as second_rating
FROM feedback_events f1
JOIN feedback_events f2
ON f1.user_id = f2.user_id
AND f2.event_time > f1.event_time
AND f2.event_time < f1.event_time + INTERVAL '5' MINUTE
WHERE f1.model_name != f2.model_name;
```

**Use Case:** Detect model switching patterns.

---

## Advanced Analytics

### Example 40: Running Total

```sql
-- Cumulative cost per model
SELECT
    model_name,
    event_time,
    cost_usd,
    SUM(cost_usd) OVER (
        PARTITION BY model_name
        ORDER BY event_time
        ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    ) as cumulative_cost
FROM feedback_events
ORDER BY model_name, event_time;
```

**Use Case:** Track cumulative spending over time.

---

### Example 41: Rank by Performance

```sql
-- Rank models by latency within each hour
SELECT
    TUMBLE_START(event_time, INTERVAL '1' HOUR) as hour,
    model_name,
    AVG(latency_ms) as avg_latency,
    RANK() OVER (
        PARTITION BY TUMBLE(event_time, INTERVAL '1' HOUR)
        ORDER BY AVG(latency_ms)
    ) as latency_rank
FROM feedback_events
GROUP BY
    model_name,
    TUMBLE(event_time, INTERVAL '1' HOUR);
```

**Use Case:** Identify best-performing model each hour.

---

### Example 42: Moving Average with LAG

```sql
-- Compare current vs previous period
SELECT
    model_name,
    TUMBLE_START(event_time, INTERVAL '1' HOUR) as hour,
    AVG(latency_ms) as current_avg_latency,
    LAG(AVG(latency_ms), 1) OVER (
        PARTITION BY model_name
        ORDER BY TUMBLE_START(event_time, INTERVAL '1' HOUR)
    ) as previous_avg_latency,
    AVG(latency_ms) - LAG(AVG(latency_ms), 1) OVER (
        PARTITION BY model_name
        ORDER BY TUMBLE_START(event_time, INTERVAL '1' HOUR)
    ) as latency_change
FROM feedback_events
GROUP BY
    model_name,
    TUMBLE(event_time, INTERVAL '1' HOUR);
```

**Use Case:** Period-over-period performance comparison.

---

### Example 43: Correlation Analysis

```sql
-- Correlation between latency and rating
SELECT
    model_name,
    COUNT(*) as samples,
    CORR(latency_ms, rating) as latency_rating_correlation,
    CORR(cost_usd, rating) as cost_rating_correlation
FROM feedback_events
WHERE rating IS NOT NULL
GROUP BY model_name
HAVING COUNT(*) > 100;
```

**Use Case:** Understand factors affecting user satisfaction.

---

### Example 44: Conditional Aggregation

```sql
-- Segment performance by latency buckets
SELECT
    model_name,
    COUNT(*) as total_requests,
    COUNT(CASE WHEN latency_ms < 1000 THEN 1 END) as fast_requests,
    COUNT(CASE WHEN latency_ms BETWEEN 1000 AND 3000 THEN 1 END) as medium_requests,
    COUNT(CASE WHEN latency_ms > 3000 THEN 1 END) as slow_requests,
    AVG(CASE WHEN latency_ms < 1000 THEN rating END) as avg_rating_fast,
    AVG(CASE WHEN latency_ms > 3000 THEN rating END) as avg_rating_slow
FROM feedback_events
GROUP BY model_name;
```

**Use Case:** Understand impact of latency on ratings.

---

### Example 45: Deduplication

```sql
-- Remove duplicate events (keep first occurrence)
SELECT DISTINCT ON (event_id)
    event_id,
    user_id,
    model_name,
    latency_ms,
    event_time
FROM feedback_events
ORDER BY event_id, event_time;
```

**Use Case:** Handle at-least-once delivery semantics.

---

## Real-World Use Cases

### Example 46: SLA Violation Detection

```sql
-- Alert on SLA violations (p95 > 3 seconds)
SELECT
    model_name,
    TUMBLE_START(event_time, INTERVAL '5' MINUTE) as window_start,
    COUNT(*) as requests,
    APPROX_PERCENTILE(latency_ms, 0.95) as p95_latency,
    CASE
        WHEN APPROX_PERCENTILE(latency_ms, 0.95) > 3000 THEN 'SLA_VIOLATED'
        ELSE 'OK'
    END as sla_status
FROM feedback_events
GROUP BY
    model_name,
    TUMBLE(event_time, INTERVAL '5' MINUTE)
HAVING APPROX_PERCENTILE(latency_ms, 0.95) > 3000;
```

**Use Case:** Real-time SLA monitoring and alerting.

---

### Example 47: A/B Test Analysis

```sql
-- Compare two prompt variants
WITH variant_metrics AS (
    SELECT
        prompt_template,
        COUNT(*) as requests,
        AVG(rating) as avg_rating,
        STDDEV(rating) as stddev_rating,
        AVG(latency_ms) as avg_latency,
        SUM(cost_usd) as total_cost
    FROM feedback_events
    WHERE model_name = 'gpt-4'
      AND prompt_template IN ('variant_a', 'variant_b')
      AND event_time >= NOW() - INTERVAL '24' HOUR
    GROUP BY prompt_template
)
SELECT
    prompt_template,
    requests,
    avg_rating,
    stddev_rating,
    avg_latency,
    total_cost,
    -- Effect size (Cohen's d)
    (MAX(avg_rating) OVER () - MIN(avg_rating) OVER ()) /
    AVG(stddev_rating) OVER () as effect_size
FROM variant_metrics;
```

**Use Case:** Statistical A/B testing of prompts.

---

### Example 48: Cost Optimization Opportunities

```sql
-- Identify expensive queries that could use cheaper models
SELECT
    user_id,
    model_name,
    COUNT(*) as request_count,
    AVG(cost_usd) as avg_cost,
    AVG(rating) as avg_rating,
    SUM(cost_usd) as total_cost,
    CASE
        WHEN model_name = 'gpt-4' AND AVG(rating) >= 4.5 THEN 'Keep gpt-4'
        WHEN model_name = 'gpt-4' AND AVG(rating) < 4.5 THEN 'Consider downgrade to gpt-3.5'
        ELSE 'No action'
    END as optimization_suggestion
FROM feedback_events
WHERE event_time >= NOW() - INTERVAL '7' DAY
GROUP BY user_id, model_name
HAVING SUM(cost_usd) > 1.0
ORDER BY total_cost DESC;
```

**Use Case:** Identify cost reduction opportunities.

---

### Example 49: Anomaly Detection with Z-Score

```sql
-- Detect latency anomalies using statistical methods
WITH stats AS (
    SELECT
        model_name,
        AVG(latency_ms) as mean_latency,
        STDDEV(latency_ms) as stddev_latency
    FROM feedback_events
    WHERE event_time >= NOW() - INTERVAL '1' HOUR
    GROUP BY model_name
)
SELECT
    f.event_id,
    f.model_name,
    f.latency_ms,
    s.mean_latency,
    s.stddev_latency,
    (f.latency_ms - s.mean_latency) / NULLIF(s.stddev_latency, 0) as z_score,
    CASE
        WHEN ABS((f.latency_ms - s.mean_latency) / NULLIF(s.stddev_latency, 0)) > 3 THEN 'ANOMALY'
        ELSE 'NORMAL'
    END as status
FROM feedback_events f
JOIN stats s ON f.model_name = s.model_name
WHERE f.event_time >= NOW() - INTERVAL '5' MINUTE
  AND ABS((f.latency_ms - s.mean_latency) / NULLIF(s.stddev_latency, 0)) > 3;
```

**Use Case:** Statistical anomaly detection.

---

### Example 50: User Churn Prediction

```sql
-- Identify users at risk of churning
WITH user_activity AS (
    SELECT
        user_id,
        COUNT(*) as total_interactions,
        AVG(rating) as avg_rating,
        MAX(event_time) as last_activity,
        EXTRACT(DAY FROM NOW() - MAX(event_time)) as days_since_last_activity,
        COUNT(CASE WHEN error_code IS NOT NULL THEN 1 END) as error_count
    FROM feedback_events
    WHERE event_time >= NOW() - INTERVAL '30' DAY
    GROUP BY user_id
)
SELECT
    user_id,
    total_interactions,
    avg_rating,
    last_activity,
    days_since_last_activity,
    error_count,
    CASE
        WHEN days_since_last_activity > 7 AND avg_rating < 3.0 THEN 'High Risk'
        WHEN days_since_last_activity > 7 OR avg_rating < 3.0 THEN 'Medium Risk'
        WHEN error_count > 5 THEN 'Medium Risk'
        ELSE 'Low Risk'
    END as churn_risk
FROM user_activity
WHERE user_id IN (SELECT DISTINCT user_id FROM feedback_events WHERE event_time >= NOW() - INTERVAL '60' DAY)
ORDER BY
    CASE
        WHEN days_since_last_activity > 7 AND avg_rating < 3.0 THEN 1
        WHEN days_since_last_activity > 7 OR avg_rating < 3.0 THEN 2
        ELSE 3
    END,
    total_interactions DESC;
```

**Use Case:** Proactive user retention.

---

## Optimization and Cost Analysis

### Example 51: Multi-Objective Ranking

```sql
-- Rank models by quality, cost, and latency (Pareto frontier)
WITH model_scores AS (
    SELECT
        model_name,
        AVG(rating) as avg_quality,
        AVG(cost_usd) as avg_cost,
        AVG(latency_ms) as avg_latency,
        COUNT(*) as sample_size
    FROM feedback_events
    WHERE event_time >= NOW() - INTERVAL '24' HOUR
    GROUP BY model_name
    HAVING COUNT(*) > 100
)
SELECT
    model_name,
    avg_quality,
    avg_cost,
    avg_latency,
    sample_size,
    -- Normalize and compute composite score (higher is better)
    (avg_quality / MAX(avg_quality) OVER ()) * 0.5 +
    (1.0 - avg_cost / MAX(avg_cost) OVER ()) * 0.3 +
    (1.0 - avg_latency / MAX(avg_latency) OVER ()) * 0.2 as composite_score,
    RANK() OVER (ORDER BY
        (avg_quality / MAX(avg_quality) OVER ()) * 0.5 +
        (1.0 - avg_cost / MAX(avg_cost) OVER ()) * 0.3 +
        (1.0 - avg_latency / MAX(avg_latency) OVER ()) * 0.2 DESC
    ) as overall_rank
FROM model_scores
ORDER BY composite_score DESC;
```

**Use Case:** Multi-objective model selection.

---

### Example 52: Cost Per Quality Point

```sql
-- Calculate cost efficiency (cost per rating point)
SELECT
    model_name,
    COUNT(*) as requests,
    AVG(rating) as avg_rating,
    SUM(cost_usd) as total_cost,
    SUM(cost_usd) / NULLIF(AVG(rating), 0) as cost_per_quality_point
FROM feedback_events
WHERE rating IS NOT NULL
  AND event_time >= NOW() - INTERVAL '7' DAY
GROUP BY model_name
HAVING AVG(rating) > 0
ORDER BY cost_per_quality_point;
```

**Use Case:** Identify most cost-efficient models.

---

### Example 53: Budget Forecasting

```sql
-- Project next month's cost based on current trends
WITH daily_cost AS (
    SELECT
        DATE(event_time) as date,
        model_name,
        SUM(cost_usd) as daily_cost
    FROM feedback_events
    WHERE event_time >= NOW() - INTERVAL '30' DAY
    GROUP BY DATE(event_time), model_name
)
SELECT
    model_name,
    AVG(daily_cost) as avg_daily_cost,
    AVG(daily_cost) * 30 as projected_monthly_cost,
    STDDEV(daily_cost) as cost_volatility,
    MIN(daily_cost) as min_daily_cost,
    MAX(daily_cost) as max_daily_cost
FROM daily_cost
GROUP BY model_name
ORDER BY projected_monthly_cost DESC;
```

**Use Case:** Monthly budget planning.

---

### Example 54: Peak Usage Detection

```sql
-- Identify peak usage hours for capacity planning
SELECT
    EXTRACT(HOUR FROM event_time) as hour_of_day,
    EXTRACT(DOW FROM event_time) as day_of_week,
    COUNT(*) as request_count,
    AVG(latency_ms) as avg_latency,
    SUM(cost_usd) as total_cost
FROM feedback_events
WHERE event_time >= NOW() - INTERVAL '7' DAY
GROUP BY
    EXTRACT(HOUR FROM event_time),
    EXTRACT(DOW FROM event_time)
ORDER BY request_count DESC
LIMIT 10;
```

**Use Case:** Capacity planning and resource allocation.

---

### Example 55: Model Drift Detection

```sql
-- Detect performance degradation over time
WITH weekly_performance AS (
    SELECT
        model_name,
        DATE_TRUNC('week', event_time) as week,
        AVG(rating) as avg_rating,
        AVG(latency_ms) as avg_latency,
        COUNT(*) as requests
    FROM feedback_events
    WHERE event_time >= NOW() - INTERVAL '12' WEEK
    GROUP BY
        model_name,
        DATE_TRUNC('week', event_time)
)
SELECT
    model_name,
    week,
    avg_rating,
    avg_latency,
    requests,
    LAG(avg_rating, 1) OVER (PARTITION BY model_name ORDER BY week) as prev_week_rating,
    avg_rating - LAG(avg_rating, 1) OVER (PARTITION BY model_name ORDER BY week) as rating_change,
    CASE
        WHEN avg_rating < LAG(avg_rating, 1) OVER (PARTITION BY model_name ORDER BY week) * 0.9 THEN 'DEGRADATION'
        WHEN avg_rating > LAG(avg_rating, 1) OVER (PARTITION BY model_name ORDER BY week) * 1.1 THEN 'IMPROVEMENT'
        ELSE 'STABLE'
    END as trend
FROM weekly_performance
ORDER BY model_name, week DESC;
```

**Use Case:** Monitor and detect model performance drift.

---

### Example 56: Real-Time Dashboard Query

```sql
-- Comprehensive real-time dashboard (last 5 minutes)
SELECT
    model_name,
    COUNT(*) as requests,
    COUNT(DISTINCT user_id) as unique_users,
    AVG(latency_ms) as avg_latency,
    APPROX_PERCENTILE(latency_ms, 0.95) as p95_latency,
    APPROX_PERCENTILE(latency_ms, 0.99) as p99_latency,
    SUM(cost_usd) as total_cost,
    AVG(cost_usd) as avg_cost_per_request,
    AVG(rating) as avg_rating,
    COUNT(CASE WHEN rating >= 4 THEN 1 END) * 100.0 / NULLIF(COUNT(rating), 0) as satisfaction_pct,
    COUNT(CASE WHEN error_code IS NOT NULL THEN 1 END) as errors,
    COUNT(CASE WHEN error_code IS NOT NULL THEN 1 END) * 100.0 / COUNT(*) as error_rate_pct,
    MIN(event_time) as first_event,
    MAX(event_time) as last_event
FROM feedback_events
WHERE event_time >= NOW() - INTERVAL '5' MINUTE
GROUP BY model_name
ORDER BY requests DESC;
```

**Use Case:** Real-time operations dashboard.

---

## Summary

This document provides 56 practical SQL examples covering:

- **Basic queries**: Selection, filtering, projection
- **Aggregations**: Count, sum, average, percentiles, standard deviation
- **Window functions**: Tumbling, sliding, session windows
- **Joins**: Inner, left, time-bounded, multi-stream
- **Advanced analytics**: Ranking, moving averages, correlation
- **Real-world use cases**: SLA monitoring, A/B testing, cost optimization, anomaly detection, churn prediction

### Next Steps

- [SQL Reference Guide](SQL_REFERENCE_GUIDE.md): Complete SQL syntax reference
- [SQL Tutorial](SQL_TUTORIAL.md): Step-by-step learning guide
- [SQL Best Practices](SQL_BEST_PRACTICES.md): Optimization tips and patterns
- [SQL API Reference](SQL_API_REFERENCE.md): Embed SQL engine in Rust applications

### Additional Resources

- [Stream Processing Architecture](STREAM_PROCESSING_WINDOWING_SPECIFICATION.md)
- [Windowing Guide](WINDOWING_AGGREGATION_ARCHITECTURE.md)
- [Metrics Guide](METRICS_GUIDE.md)
