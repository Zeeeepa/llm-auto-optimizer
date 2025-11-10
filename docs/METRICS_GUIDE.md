# LLM Auto-Optimizer Metrics Guide

**Version:** 1.0.0
**Last Updated:** 2025-11-10
**Status:** Production

## Table of Contents

- [Overview](#overview)
- [Metrics Reference](#metrics-reference)
  - [System Metrics](#system-metrics)
  - [HTTP/API Metrics](#httpapi-metrics)
  - [Stream Processing Metrics](#stream-processing-metrics)
  - [State Backend Metrics](#state-backend-metrics)
  - [Cache Metrics](#cache-metrics)
  - [Kafka Metrics](#kafka-metrics)
  - [LLM Provider Metrics](#llm-provider-metrics)
  - [Optimization Metrics](#optimization-metrics)
- [Dashboard Usage Guide](#dashboard-usage-guide)
- [Alert Interpretation](#alert-interpretation)
- [Troubleshooting Runbook](#troubleshooting-runbook)
- [Best Practices](#best-practices)
- [Appendix](#appendix)

---

## Overview

The LLM Auto-Optimizer uses a comprehensive observability stack built on:

- **Prometheus** - Metrics collection and storage
- **Grafana** - Visualization and dashboards
- **OpenTelemetry** - Distributed tracing and metrics instrumentation
- **Structured Logging** - JSON-formatted logs with context

### Architecture

```
┌─────────────────┐
│  Application    │
│   (Rust)        │
│  ┌───────────┐  │
│  │ OpenTel   │  │
│  │ SDK       │  │
│  └─────┬─────┘  │
└────────┼────────┘
         │
         ├─────────────┐
         │             │
    ┌────▼────┐   ┌────▼────┐
    │Prometheus│   │ OTLP    │
    │ Scrape  │   │Collector│
    └────┬────┘   └────┬────┘
         │             │
    ┌────▼─────────────▼────┐
    │      Prometheus       │
    │      TSDB             │
    └────┬──────────────────┘
         │
    ┌────▼─────┐
    │ Grafana  │
    │Dashboards│
    └──────────┘
```

### Metric Types

| Type | Description | Use Case | Example |
|------|-------------|----------|---------|
| **Counter** | Monotonically increasing value | Request counts, error counts | `http_requests_total` |
| **Gauge** | Value that can go up or down | Current connections, queue size | `active_connections` |
| **Histogram** | Distribution of values | Latency, response sizes | `http_request_duration_ms` |
| **Summary** | Similar to histogram, pre-calculated percentiles | Client-side percentiles | `request_latency_summary` |

---

## Metrics Reference

### System Metrics

#### CPU Metrics

```promql
# Total CPU usage percentage
process_cpu_seconds_total

# CPU usage rate
rate(process_cpu_seconds_total[5m]) * 100
```

**Labels:**
- `instance` - Instance identifier
- `job` - Job name (llm-optimizer)

**Thresholds:**
- Normal: < 70%
- Warning: 70-85%
- Critical: > 85%

#### Memory Metrics

```promql
# Process memory usage in bytes
process_memory_bytes

# Memory usage percentage
(process_memory_bytes / machine_memory_bytes) * 100

# Heap memory allocation
process_heap_bytes

# Heap memory usage percentage
(process_heap_bytes / process_heap_max_bytes) * 100
```

**Labels:**
- `instance` - Instance identifier
- `memory_type` - heap, stack, native

**Thresholds:**
- Normal: < 75%
- Warning: 75-90%
- Critical: > 90%

**Troubleshooting:**
```bash
# Check memory usage trends
curl -s 'http://prometheus:9090/api/v1/query?query=process_memory_bytes' | jq

# Get heap dump for analysis
kill -3 <pid>  # Sends SIGQUIT for thread dump

# Monitor GC activity
# Review GC logs and tune if necessary
```

#### Thread Metrics

```promql
# Active thread count
process_threads_total

# Thread pool utilization
(threadpool_active_threads / threadpool_max_threads) * 100
```

**Labels:**
- `pool_name` - Thread pool identifier
- `instance` - Instance identifier

**Thresholds:**
- Normal: < 80%
- Warning: 80-95%
- Critical: > 95%

#### Disk I/O Metrics

```promql
# Disk read bytes
rate(disk_read_bytes_total[5m])

# Disk write bytes
rate(disk_write_bytes_total[5m])

# Disk usage percentage
(disk_used_bytes / disk_total_bytes) * 100
```

**Labels:**
- `device` - Disk device name
- `mount_point` - Mount point path

**Thresholds:**
- Normal: < 80% utilization
- Warning: 80-90%
- Critical: > 90%

---

### HTTP/API Metrics

#### Request Metrics

```promql
# Total requests
http_requests_total

# Request rate (QPS)
rate(http_requests_total[1m])

# Success rate
rate(http_requests_total{status=~"2.."}[5m])
/
rate(http_requests_total[5m]) * 100

# Error rate
rate(http_requests_total{status=~"[45].."}[5m])
/
rate(http_requests_total[5m]) * 100
```

**Labels:**
- `method` - HTTP method (GET, POST, etc.)
- `endpoint` - API endpoint path
- `status` - HTTP status code
- `instance` - Instance identifier

**Thresholds:**
- Success Rate: > 99.9%
- Error Rate: < 0.1%

#### Latency Metrics

```promql
# P50 latency
histogram_quantile(0.50,
  rate(http_request_duration_ms_bucket[5m])
)

# P95 latency
histogram_quantile(0.95,
  rate(http_request_duration_ms_bucket[5m])
)

# P99 latency
histogram_quantile(0.99,
  rate(http_request_duration_ms_bucket[5m])
)

# Average latency
rate(http_request_duration_ms_sum[5m])
/
rate(http_request_duration_ms_count[5m])
```

**Labels:**
- `method` - HTTP method
- `endpoint` - API endpoint
- `status` - Response status

**SLOs:**
- P50: < 100ms
- P95: < 500ms
- P99: < 1000ms

#### Connection Metrics

```promql
# Active HTTP connections
http_connections_active

# Connection rate
rate(http_connections_total[5m])

# Keep-alive connections
http_connections_keepalive
```

**Labels:**
- `state` - Connection state (active, idle, closed)
- `instance` - Instance identifier

---

### Stream Processing Metrics

#### Event Metrics

```promql
# Events received rate
rate(stream_events_received_total[5m])

# Events processed rate
rate(stream_events_processed_total[5m])

# Events dropped
rate(stream_events_dropped_total[5m])

# Processing lag
stream_events_received_total - stream_events_processed_total

# Late events rate
rate(late_events_total[5m])
```

**Labels:**
- `pipeline` - Pipeline name
- `operator` - Operator name
- `event_type` - Event type
- `reason` - Drop/failure reason

**Thresholds:**
- Processing lag: < 1000 events
- Drop rate: < 0.1%
- Late event rate: < 1%

#### Processing Latency

```promql
# Processing latency percentiles
histogram_quantile(0.95,
  rate(stream_processing_duration_ms_bucket[5m])
)

# End-to-end latency
histogram_quantile(0.99,
  rate(stream_e2e_latency_ms_bucket[5m])
)
```

**Labels:**
- `pipeline` - Pipeline name
- `stage` - Processing stage

**SLOs:**
- P95 processing latency: < 100ms
- P99 end-to-end latency: < 1000ms

#### Backpressure Metrics

```promql
# Queue size
stream_backpressure_queue_size

# Queue utilization percentage
(stream_backpressure_queue_size
/
stream_backpressure_queue_capacity) * 100

# Backpressure events
rate(stream_backpressure_events_total[5m])
```

**Labels:**
- `pipeline` - Pipeline identifier
- `operator` - Operator name

**Thresholds:**
- Queue size: < 500 (warning), < 1000 (critical)
- Utilization: < 75% (warning), < 90% (critical)

#### Window Metrics

```promql
# Windows created
rate(window_created_total[5m])

# Windows triggered
rate(window_triggered_total[5m])

# Active windows
window_active_count

# Window size distribution
window_size_bytes

# Late data dropped from windows
rate(window_late_data_dropped_total[5m])
```

**Labels:**
- `window_type` - tumbling, sliding, session
- `window_size` - Window duration
- `watermark_strategy` - Watermark strategy

#### Watermark Metrics

```promql
# Current watermark
watermark_current_timestamp_ms

# Watermark lag
watermark_lag_ms

# Watermark progress rate
rate(watermark_current_timestamp_ms[5m])
```

**Labels:**
- `pipeline` - Pipeline name
- `source` - Data source

**Thresholds:**
- Watermark lag: < 30s (normal), < 60s (warning), < 120s (critical)

---

### State Backend Metrics

#### State Operations

```promql
# GET operations
rate(state_backend_get_total[5m])

# PUT operations
rate(state_backend_put_total[5m])

# DELETE operations
rate(state_backend_delete_total[5m])

# Operation errors
rate(state_backend_errors_total[5m])

# Operation error rate
rate(state_backend_errors_total[5m])
/
rate(state_backend_operations_total[5m]) * 100
```

**Labels:**
- `backend_type` - memory, redis, postgres, sled
- `operation` - get, put, delete, scan
- `error_type` - timeout, connection, serialization

**Thresholds:**
- Error rate: < 0.1%

#### State Latency

```promql
# GET latency
histogram_quantile(0.95,
  rate(state_backend_get_duration_ms_bucket[5m])
)

# PUT latency
histogram_quantile(0.95,
  rate(state_backend_put_duration_ms_bucket[5m])
)

# DELETE latency
histogram_quantile(0.95,
  rate(state_backend_delete_duration_ms_bucket[5m])
)
```

**Labels:**
- `backend_type` - Backend type
- `cache_hit` - Cache hit (true/false)

**SLOs:**
- P95 GET latency: < 10ms (cache hit), < 50ms (cache miss)
- P95 PUT latency: < 20ms
- P95 DELETE latency: < 10ms

#### State Size

```promql
# Total state size in bytes
state_backend_size_bytes

# State entry count
state_backend_entries_count

# Average entry size
state_backend_size_bytes / state_backend_entries_count

# State size growth rate
rate(state_backend_size_bytes[1h])
```

**Labels:**
- `backend_type` - Backend type
- `namespace` - State namespace

**Thresholds:**
- Size growth: < 1GB/hour (warning), < 10GB/hour (critical)

#### Checkpoint Metrics

```promql
# Checkpoint count
rate(state_checkpoint_count_total[5m])

# Checkpoint duration
histogram_quantile(0.95,
  rate(state_checkpoint_duration_ms_bucket[5m])
)

# Checkpoint size
state_checkpoint_size_bytes

# Checkpoint failures
rate(state_checkpoint_failures_total[5m])
```

**Labels:**
- `checkpoint_type` - full, incremental
- `backend_type` - Backend type

---

### Cache Metrics

#### Cache Operations

```promql
# Cache hits
rate(state_cache_hits_total[5m])

# Cache misses
rate(state_cache_misses_total[5m])

# Cache hit rate
rate(state_cache_hits_total[5m])
/
(rate(state_cache_hits_total[5m]) + rate(state_cache_misses_total[5m])) * 100

# Cache evictions
rate(state_cache_evictions_total[5m])

# Cache expirations
rate(state_cache_expirations_total[5m])
```

**Labels:**
- `cache_layer` - l1, l2, l3
- `cache_type` - lru, lfu, ttl

**Thresholds:**
- Hit rate: > 80% (L1), > 60% (L2), > 40% (L3)

#### Cache Size

```promql
# Cache entries
cache_entries_count

# Cache size in bytes
cache_size_bytes

# Cache capacity utilization
(cache_size_bytes / cache_capacity_bytes) * 100
```

**Labels:**
- `cache_layer` - Cache layer identifier

**Thresholds:**
- Utilization: < 90%

#### Cache Latency

```promql
# Cache GET latency
histogram_quantile(0.95,
  rate(cache_get_duration_ms_bucket[5m])
)

# Cache PUT latency
histogram_quantile(0.95,
  rate(cache_put_duration_ms_bucket[5m])
)
```

**SLOs:**
- P95 GET: < 1ms (L1), < 5ms (L2), < 20ms (L3)
- P95 PUT: < 2ms (L1), < 10ms (L2), < 30ms (L3)

---

### Kafka Metrics

#### Producer Metrics

```promql
# Messages sent
rate(kafka_producer_messages_sent_total[5m])

# Message send errors
rate(kafka_producer_errors_total[5m])

# Message send latency
histogram_quantile(0.95,
  rate(kafka_producer_latency_ms_bucket[5m])
)

# Batch size
histogram_quantile(0.95,
  rate(kafka_producer_batch_size_bucket[5m])
)
```

**Labels:**
- `topic` - Kafka topic
- `partition` - Partition ID
- `error_type` - Error type

**Thresholds:**
- Error rate: < 0.1%
- P95 latency: < 100ms

#### Consumer Metrics

```promql
# Messages consumed
rate(kafka_consumer_messages_consumed_total[5m])

# Consumer lag
kafka_consumer_lag

# Lag ratio (lag / total messages)
kafka_consumer_lag
/
(kafka_consumer_lag + kafka_consumer_offset) * 100

# Rebalances
rate(kafka_consumer_rebalances_total[5m])
```

**Labels:**
- `topic` - Kafka topic
- `partition` - Partition ID
- `consumer_group` - Consumer group ID

**Thresholds:**
- Consumer lag: < 1000 messages
- Lag ratio: < 1%

---

### LLM Provider Metrics

#### Request Metrics

```promql
# LLM requests
rate(llm_requests_total[5m])

# LLM errors
rate(llm_errors_total[5m])

# LLM error rate
rate(llm_errors_total[5m])
/
rate(llm_requests_total[5m]) * 100

# LLM latency
histogram_quantile(0.95,
  rate(llm_request_duration_ms_bucket[5m])
)
```

**Labels:**
- `provider` - anthropic, openai, cohere, etc.
- `model` - Model name
- `error_type` - rate_limit, timeout, invalid_request

**Thresholds:**
- Error rate: < 1%
- P95 latency: varies by model

#### Token Metrics

```promql
# Total tokens
rate(llm_tokens_total[5m])

# Input tokens
rate(llm_input_tokens_total[5m])

# Output tokens
rate(llm_output_tokens_total[5m])

# Cached tokens
rate(llm_cached_tokens_total[5m])

# Cache hit rate
rate(llm_cached_tokens_total[5m])
/
rate(llm_input_tokens_total[5m]) * 100
```

**Labels:**
- `provider` - Provider name
- `model` - Model name

#### Cost Metrics

```promql
# Total cost (USD)
rate(llm_cost_total[1h]) * 3600

# Cost per request
rate(llm_cost_total[1h]) * 3600
/
rate(llm_requests_total[1h]) * 3600

# Cost by model
sum by(model) (rate(llm_cost_total[1h]) * 3600)
```

**Labels:**
- `provider` - Provider name
- `model` - Model name

**Budget Monitoring:**
```promql
# Daily spend
increase(llm_cost_total[1d])

# Projected monthly spend
increase(llm_cost_total[1d]) * 30

# Budget utilization
increase(llm_cost_total[1d]) / daily_budget * 100
```

---

### Optimization Metrics

#### A/B Testing Metrics

```promql
# Active experiments
ab_test_active_count

# Experiment assignments
rate(ab_test_assignments_total[5m])

# Experiment conversions
rate(ab_test_conversions_total[5m])

# Conversion rate by variant
rate(ab_test_conversions_total[5m])
/
rate(ab_test_assignments_total[5m]) * 100

# Confidence score
ab_test_confidence
```

**Labels:**
- `experiment_id` - Experiment identifier
- `variant_id` - Variant identifier
- `metric_name` - Success metric

#### Bandit Metrics

```promql
# Arm pulls
rate(bandit_arm_pulls_total[5m])

# Arm rewards
rate(bandit_arm_rewards_total[5m])

# Average reward
rate(bandit_arm_rewards_total[5m])
/
rate(bandit_arm_pulls_total[5m])

# Cumulative regret
bandit_cumulative_regret

# Exploration rate
rate(bandit_explorations_total[5m])
/
rate(bandit_arm_pulls_total[5m]) * 100
```

**Labels:**
- `bandit_id` - Bandit algorithm ID
- `arm_id` - Arm identifier
- `parameter` - Parameter being optimized

#### Parameter Optimization Metrics

```promql
# Parameter updates
rate(param_optimization_updates_total[5m])

# Current parameter value
param_optimization_current_value

# Parameter performance
param_optimization_performance_score

# Optimization iterations
rate(param_optimization_iterations_total[5m])
```

**Labels:**
- `parameter_name` - Parameter name
- `optimization_algorithm` - Algorithm used

---

## Dashboard Usage Guide

### Overview Dashboard

**Purpose:** High-level system health and performance monitoring

**Key Panels:**
1. **Service Health** - Up/down status
2. **Request Rate** - Current QPS
3. **Error Rate** - Error percentage
4. **Latency Percentiles** - P50/P95/P99 over time

**How to Use:**
```
1. Start here for overall health check
2. Check service status panel (should be green/UP)
3. Review request rate trends (look for anomalies)
4. Monitor error rate (should be < 1%)
5. Check latency percentiles (compare to SLOs)
6. Use time range selector for historical analysis
```

**Common Patterns:**
- **Spike in latency + error rate**: Downstream service issue
- **Drop in request rate**: Client connectivity or load balancer issue
- **Steady error rate > 1%**: Code bug or configuration issue

### Stream Processing Dashboard

**Purpose:** Monitor stream processing pipelines

**Key Panels:**
1. **Events Received vs Processed** - Processing throughput
2. **Processing Latency** - Event processing speed
3. **Backpressure Queue** - Processing bottlenecks
4. **Pipeline Lag** - End-to-end delay

**How to Use:**
```
1. Compare received vs processed rates (should be equal)
2. Check backpressure queue (should be < 500)
3. Monitor pipeline lag (should be < 5 seconds)
4. Review error distribution by type
5. Use pipeline filter to isolate specific pipelines
```

**Troubleshooting:**
- **Received > Processed**: Scale up processing capacity
- **High backpressure**: Check operator performance
- **High lag**: Review watermark configuration

### State Backend Dashboard

**Purpose:** Monitor state operations and performance

**Key Panels:**
1. **Operation Counts** - GET/PUT/DELETE rates
2. **Operation Latency** - P50/P95/P99 latencies
3. **Cache Hit Rate** - Cache effectiveness
4. **State Size** - Storage utilization

**How to Use:**
```
1. Review operation patterns (read vs write ratio)
2. Check latency percentiles against SLOs
3. Monitor cache hit rates (should be > 80%)
4. Track state size growth over time
5. Use backend_type filter to compare backends
```

**Optimization:**
- **Low cache hit rate**: Increase cache size or improve key locality
- **High PUT latency**: Review batch sizes or consider async writes
- **Rapid state growth**: Review TTL settings

---

## Alert Interpretation

### Critical Alerts

#### HighErrorRate
**Trigger:** Error rate > 5% for 5 minutes

**What It Means:**
System is experiencing high error rates affecting user requests.

**Immediate Actions:**
1. Check recent deployments (rollback if necessary)
2. Review error logs for patterns
3. Check external service health (LLM APIs, databases)
4. Verify configuration changes

**Investigation:**
```bash
# View recent errors
curl -s 'http://prometheus:9090/api/v1/query?query=rate(http_requests_errors_total[5m])' | jq

# Check error distribution
curl -s 'http://prometheus:9090/api/v1/query?query=sum%20by(error_type)%20(rate(http_requests_errors_total[5m]))' | jq

# Review logs
journalctl -u llm-optimizer --since "10 minutes ago" | grep ERROR
```

#### ServiceDown
**Trigger:** Service unreachable for 1 minute

**What It Means:**
Instance is not responding to health checks or metrics scraping.

**Immediate Actions:**
```bash
# Check service status
systemctl status llm-optimizer

# View recent logs
journalctl -u llm-optimizer --since "10 minutes ago" --no-pager

# Check process
ps aux | grep llm-optimizer

# Test connectivity
curl -v http://localhost:8080/health

# Restart if needed
systemctl restart llm-optimizer
```

#### HighLatency
**Trigger:** P99 latency > 1000ms for 5 minutes

**What It Means:**
1% of requests are taking longer than 1 second.

**Investigation Steps:**
```bash
# Check CPU usage
top -b -n 1 | grep llm-optimizer

# Check memory
free -h

# Review slow requests
# Check distributed traces for slow spans

# Database performance
# Review slow query logs

# Network latency
ping <downstream-service>
```

### Warning Alerts

#### HighBackpressure
**Trigger:** Queue size > 1000 for 5 minutes

**What It Means:**
Processing cannot keep up with incoming rate.

**Actions:**
1. Monitor for queue growth
2. Review operator performance metrics
3. Consider scaling processing capacity
4. Check for downstream bottlenecks

#### LowCacheHitRate
**Trigger:** Hit rate < 50% for 10 minutes

**What It Means:**
Cache is not effectively reducing database load.

**Optimization:**
```bash
# Analyze cache patterns
curl -s 'http://prometheus:9090/api/v1/query?query=state_cache_hits_total' | jq

# Review cache sizing
# Check cache configuration

# Consider:
# 1. Increasing cache size
# 2. Adjusting TTL
# 3. Reviewing access patterns
# 4. Pre-warming cache
```

---

## Troubleshooting Runbook

### High CPU Usage

**Symptoms:**
- CPU usage > 80%
- Request latency increasing
- Thread pool exhaustion

**Diagnosis:**
```bash
# Check CPU usage
top -H -p $(pgrep llm-optimizer)

# Get thread dump
kill -3 $(pgrep llm-optimizer)

# Review thread dump
less /var/log/llm-optimizer/thread_dump.log

# Profile CPU usage (if profiler enabled)
curl http://localhost:9090/debug/pprof/profile?seconds=30 > cpu.prof
```

**Solutions:**
1. Review hot paths in profiler
2. Check for inefficient algorithms
3. Review thread pool sizing
4. Consider horizontal scaling
5. Optimize database queries

### High Memory Usage

**Symptoms:**
- Memory usage > 80%
- GC pressure increasing
- OOM errors in logs

**Diagnosis:**
```bash
# Check memory usage
ps aux | grep llm-optimizer

# Review heap usage
# If using metrics endpoint:
curl http://localhost:9090/metrics | grep heap

# Get heap dump (causes pause!)
kill -3 $(pgrep llm-optimizer)

# Analyze heap dump
# Use memory profiler tools
```

**Solutions:**
1. Review memory allocation patterns
2. Check for memory leaks
3. Optimize cache sizes
4. Review state retention policies
5. Tune GC settings
6. Increase heap size if appropriate

### Database Connection Pool Exhaustion

**Symptoms:**
- Connection timeouts
- High connection wait times
- Request failures

**Diagnosis:**
```promql
# Check pool utilization
db_connection_pool_active / db_connection_pool_max * 100

# Review connection wait times
histogram_quantile(0.95, rate(db_connection_wait_ms_bucket[5m]))

# Check for connection leaks
# Monitor connection lifetime
```

**Solutions:**
```bash
# Review pool configuration
cat /etc/llm-optimizer/database.toml

# Check for connection leaks
# Review connection usage patterns

# Temporary mitigation:
# Increase pool size (if database can handle it)

# Long-term:
# Optimize query patterns
# Implement connection pooling best practices
# Use connection timeout
```

### High Kafka Consumer Lag

**Symptoms:**
- Consumer lag > 10000 messages
- Increasing end-to-end latency
- Data freshness issues

**Diagnosis:**
```bash
# Check consumer lag
kafka-consumer-groups.sh --bootstrap-server localhost:9092 \
  --describe --group llm-optimizer-consumer

# Review consumer metrics
curl -s 'http://prometheus:9090/api/v1/query?query=kafka_consumer_lag' | jq

# Check processing rate
curl -s 'http://prometheus:9090/api/v1/query?query=rate(stream_events_processed_total[5m])' | jq
```

**Solutions:**
1. **Scale consumers:** Add more consumer instances
2. **Optimize processing:** Review operator performance
3. **Increase parallelism:** Add more partitions
4. **Batch processing:** Increase batch sizes
5. **Skip and catch up:** Consider skipping to recent offset (if acceptable)

```bash
# Scale consumers (Kubernetes example)
kubectl scale deployment llm-optimizer --replicas=5

# Reset consumer offset (CAUTION!)
kafka-consumer-groups.sh --bootstrap-server localhost:9092 \
  --group llm-optimizer-consumer \
  --topic feedback-events \
  --reset-offsets --to-latest --execute
```

### State Size Growth

**Symptoms:**
- Rapid state size increase
- Memory pressure
- Slow state operations

**Diagnosis:**
```promql
# Check state size growth
deriv(state_backend_size_bytes[1h])

# Review state entry count
state_backend_entries_count

# Check average entry size
state_backend_size_bytes / state_backend_entries_count
```

**Solutions:**
1. **Review TTL settings:** Ensure state expires appropriately
2. **Implement cleanup jobs:** Periodic state pruning
3. **Optimize state structure:** Reduce entry sizes
4. **Consider compaction:** Manual or automatic compaction
5. **Archive old data:** Move to cold storage

```bash
# Review state retention configuration
cat /etc/llm-optimizer/state.toml

# Trigger manual cleanup
curl -X POST http://localhost:8080/admin/state/cleanup

# Monitor cleanup progress
watch -n 5 'curl -s http://localhost:9090/metrics | grep state_backend_size_bytes'
```

---

## Best Practices

### Metric Naming

Follow Prometheus naming conventions:

```
<namespace>_<subsystem>_<name>_<unit>

Examples:
- http_requests_total
- kafka_consumer_lag
- state_backend_size_bytes
- cache_hit_rate_percent
```

**Rules:**
- Use snake_case
- Include unit suffix (bytes, seconds, total)
- Use `_total` for counters
- Be descriptive but concise

### Label Guidelines

**Good Labels:**
```promql
http_requests_total{method="GET", endpoint="/api/users", status="200"}
```

**Avoid:**
- High cardinality labels (user_id, request_id)
- Unbounded labels (timestamp, dynamic values)
- Redundant labels

**Label Cardinality:**
```
Total series = product of all label values

Example:
- methods: 5 (GET, POST, PUT, DELETE, PATCH)
- endpoints: 20
- status: 10
Total: 5 × 20 × 10 = 1,000 series ✓

Bad example:
- user_id: 1,000,000
- request_id: unlimited
Total: explosive cardinality ✗
```

### Query Performance

**Efficient Queries:**
```promql
# Use recording rules for complex queries
rate(http_requests_total[5m])

# Aggregate early
sum by(status) (rate(http_requests_total[5m]))

# Use appropriate ranges
rate(metric[5m])  # Not rate(metric[1h])
```

**Avoid:**
```promql
# Don't use large ranges unnecessarily
rate(metric[24h])  # Usually too large

# Don't compute on the fly if can be pre-aggregated
sum(rate(metric1[5m])) * sum(rate(metric2[5m]))  # Use recording rule
```

### Recording Rules

Create recording rules for expensive queries:

```yaml
groups:
  - name: performance
    interval: 30s
    rules:
      - record: job:http_request_duration_ms:p95
        expr: |
          histogram_quantile(0.95,
            rate(http_request_duration_ms_bucket[5m])
          )

      - record: job:error_rate:5m
        expr: |
          rate(http_requests_errors_total[5m])
          /
          rate(http_requests_total[5m]) * 100
```

### Dashboard Design

**Principles:**
1. **Start broad, drill down:** Overview → Details
2. **Use variables:** Allow filtering by instance, environment
3. **Show trends:** Use time series graphs
4. **Highlight thresholds:** Mark SLO lines
5. **Add context:** Include descriptions and links

**Layout:**
```
┌─────────────────────────────────┐
│  Key Metrics (Stats)            │  ← Most important
├─────────────────────────────────┤
│  Time Series Graphs             │  ← Trends
├─────────────────────────────────┤
│  Distribution (Histograms)      │  ← Detailed analysis
├─────────────────────────────────┤
│  Tables (Details)               │  ← Drill-down
└─────────────────────────────────┘
```

### Alert Fatigue Prevention

**Alert Design:**
1. **Actionable:** Every alert should require action
2. **Meaningful:** Alert on symptoms, not causes
3. **Proper severity:** Don't over-use critical
4. **Appropriate duration:** Allow for transient issues
5. **Clear runbook:** Link to troubleshooting steps

**Example:**
```yaml
# Good alert
- alert: HighErrorRate
  expr: error_rate > 5
  for: 5m  # Wait 5 minutes to avoid flapping
  annotations:
    runbook: https://runbook.com/high-error-rate

# Bad alert
- alert: SingleError
  expr: errors_total > 0
  for: 0s  # Fires on every single error
```

---

## Appendix

### Useful PromQL Queries

#### Request Rate Trending
```promql
# Request rate change (current vs 1 hour ago)
(
  rate(http_requests_total[5m])
  -
  rate(http_requests_total[5m] offset 1h)
)
/
rate(http_requests_total[5m] offset 1h) * 100
```

#### Latency Heatmap
```promql
sum(rate(http_request_duration_ms_bucket[5m])) by (le)
```

#### Top Endpoints by Error Rate
```promql
topk(10,
  sum by(endpoint) (rate(http_requests_errors_total[5m]))
)
```

#### Resource Utilization Correlation
```promql
# CPU vs Request Rate
rate(process_cpu_seconds_total[5m]) * 100
  and
rate(http_requests_total[5m])
```

### Grafana Tips

**Variables:**
```
$datasource  - Datasource selector
$instance    - Instance filter
$interval    - Auto-calculated interval
$__rate_interval - Optimal rate interval
```

**Functions:**
```
$__range     - Selected time range
$__from      - Start of time range
$__to        - End of time range
$__interval  - Dashboard interval
```

**Provisioning:**
```bash
# Dashboard provisioning
/etc/grafana/provisioning/dashboards/
  ├── dashboard.yml
  └── dashboards/
      ├── overview.json
      ├── stream-processing.json
      └── state-backend.json
```

### Metric Export Format

**Prometheus Exposition Format:**
```
# HELP http_requests_total Total number of HTTP requests
# TYPE http_requests_total counter
http_requests_total{method="GET",status="200"} 1234
http_requests_total{method="POST",status="201"} 567

# HELP http_request_duration_ms HTTP request latency
# TYPE http_request_duration_ms histogram
http_request_duration_ms_bucket{le="100"} 950
http_request_duration_ms_bucket{le="500"} 1200
http_request_duration_ms_bucket{le="1000"} 1450
http_request_duration_ms_bucket{le="+Inf"} 1500
http_request_duration_ms_sum 125000
http_request_duration_ms_count 1500
```

### OpenTelemetry Integration

**Span Attributes:**
```rust
span.set_attribute("http.method", "GET");
span.set_attribute("http.status_code", 200);
span.set_attribute("http.url", "/api/users");
span.set_attribute("llm.model", "claude-3-opus");
span.set_attribute("llm.tokens", 1234);
```

**Trace Context Propagation:**
```rust
// Extract context from headers
let context = extract_context(headers);

// Start child span
let span = tracer.start_with_context("operation", &context);

// Inject context into outgoing request
inject_context(&mut headers, &span.context());
```

### Resource Links

- **Prometheus Documentation:** https://prometheus.io/docs/
- **Grafana Documentation:** https://grafana.com/docs/
- **OpenTelemetry Specification:** https://opentelemetry.io/docs/
- **PromQL Basics:** https://prometheus.io/docs/prometheus/latest/querying/basics/
- **Grafana Best Practices:** https://grafana.com/docs/grafana/latest/best-practices/

---

**Document Version:** 1.0.0
**Maintained By:** DevOps Team
**Last Review:** 2025-11-10
**Next Review:** 2026-02-10
