# Metrics Dashboards and Alerts

**Production-ready Grafana dashboards and Prometheus alert rules for Stream Processor observability**

---

## Table of Contents

1. [Dashboard Overview](#dashboard-overview)
2. [Stream Processor Overview Dashboard](#stream-processor-overview-dashboard)
3. [Kafka Metrics Dashboard](#kafka-metrics-dashboard)
4. [State Backend Dashboard](#state-backend-dashboard)
5. [SLO Dashboard](#slo-dashboard)
6. [Alert Rules](#alert-rules)
7. [Runbook Examples](#runbook-examples)

---

## Dashboard Overview

### Dashboard Hierarchy

```
Grafana Dashboards
├── 1. Overview (High-level health)
├── 2. Stream Processor (Detailed processing metrics)
├── 3. Kafka Integration (Message flow)
├── 4. State Backend (Storage and caching)
├── 5. SLO & SLIs (Service level objectives)
└── 6. Resource Utilization (CPU, memory, network)
```

### Dashboard Design Principles

1. **Top-down**: Start with high-level health, drill down to details
2. **Golden Signals**: Latency, Traffic, Errors, Saturation (USE method)
3. **Actionable**: Every panel should inform decisions
4. **Contextual**: Link related dashboards
5. **Consistent**: Uniform color scheme and layout

---

## Stream Processor Overview Dashboard

### Panel 1: Event Throughput

**Panel Type**: Graph (Time Series)
**Position**: Top-left (full width)

```yaml
Panel Config:
  Title: "Event Throughput (events/sec)"
  Targets:
    - Query: |
        rate(stream_processor_events_received_total[5m])
      Legend: "Received - {{event_type}}"

    - Query: |
        rate(stream_processor_events_processed_total[5m])
      Legend: "Processed - {{event_type}}"

    - Query: |
        rate(stream_processor_events_dropped_total[5m])
      Legend: "Dropped - {{event_type}}"

  Visualization:
    Line Width: 2
    Fill Opacity: 10
    Gradient Mode: opacity
    Y-Axis: events/sec
```

**PromQL Queries**:
```promql
# Events received per second
rate(stream_processor_events_received_total[5m])

# Events processed per second by type
sum(rate(stream_processor_events_processed_total[5m])) by (event_type)

# Processing rate vs receive rate (should be ~1.0)
rate(stream_processor_events_processed_total[5m]) /
rate(stream_processor_events_received_total[5m])
```

### Panel 2: Processing Latency

**Panel Type**: Graph (Time Series)
**Position**: Row 2, Left

```yaml
Panel Config:
  Title: "Processing Latency (ms)"
  Targets:
    - Query: |
        histogram_quantile(0.50,
          rate(stream_processor_processing_duration_ms_bucket[5m]))
      Legend: "p50"

    - Query: |
        histogram_quantile(0.95,
          rate(stream_processor_processing_duration_ms_bucket[5m]))
      Legend: "p95"
      Color: Yellow

    - Query: |
        histogram_quantile(0.99,
          rate(stream_processor_processing_duration_ms_bucket[5m]))
      Legend: "p99"
      Color: Red

  Thresholds:
    - Value: 100
      Color: Yellow
      Label: "SLO: 100ms"
```

**Advanced Queries**:
```promql
# Latency by event type
histogram_quantile(0.95,
  rate(stream_processor_processing_duration_ms_bucket[5m])
) by (event_type)

# Average latency
rate(stream_processor_processing_duration_ms_sum[5m]) /
rate(stream_processor_processing_duration_ms_count[5m])

# Latency spike detection (p99 > 2x p50)
histogram_quantile(0.99, rate(stream_processor_processing_duration_ms_bucket[5m])) >
2 * histogram_quantile(0.50, rate(stream_processor_processing_duration_ms_bucket[5m]))
```

### Panel 3: Error Rate

**Panel Type**: Graph with Alert Threshold
**Position**: Row 2, Right

```yaml
Panel Config:
  Title: "Error Rate (errors/sec)"
  Targets:
    - Query: |
        rate(stream_processor_errors_total[5m])
      Legend: "{{error_type}}"
      Color: Red

  Alert:
    Condition: Error rate > 10/sec for 5 minutes
    Severity: Critical
```

**PromQL**:
```promql
# Total error rate
sum(rate(stream_processor_errors_total[5m]))

# Error rate by type
rate(stream_processor_errors_total[5m]) by (error_type)

# Error ratio (errors/events)
rate(stream_processor_errors_total[5m]) /
rate(stream_processor_events_received_total[5m])
```

### Panel 4: Active Windows

**Panel Type**: Stat with Sparkline
**Position**: Row 3, Left

```yaml
Panel Config:
  Title: "Active Windows"
  Target:
    Query: stream_processor_active_windows
    Legend: "{{window_type}}"

  Visualization:
    Display: Value + Sparkline
    Unit: short
    Color: Green if stable
```

### Panel 5: Window Trigger Rate

**Panel Type**: Graph
**Position**: Row 3, Center

```promql
# Windows triggered per second
rate(stream_processor_window_triggers_total[5m]) by (window_type)

# Average events per window trigger
rate(stream_processor_events_processed_total[5m]) /
rate(stream_processor_window_triggers_total[5m])
```

### Panel 6: Deduplication Efficiency

**Panel Type**: Gauge
**Position**: Row 3, Right

```promql
# Deduplication hit rate
rate(stream_processor_deduplication_hits_total[5m]) /
(rate(stream_processor_events_received_total[5m]) +
 rate(stream_processor_deduplication_hits_total[5m]))

# Duplicate ratio
rate(stream_processor_deduplication_hits_total[5m]) /
rate(stream_processor_events_received_total[5m])
```

### Panel 7: End-to-End Latency

**Panel Type**: Heatmap
**Position**: Row 4, Full Width

```yaml
Panel Config:
  Title: "End-to-End Latency Distribution (Event Time → Processing)"
  Target:
    Query: |
      sum(increase(stream_processor_end_to_end_latency_ms_bucket[5m])) by (le)

  Visualization:
    Type: Heatmap
    Color Scheme: interpolateRdYlGn
```

---

## Kafka Metrics Dashboard

### Panel 1: Message Throughput

```yaml
Title: "Kafka Message Throughput"
Targets:
  - Query: rate(kafka_messages_sent_total[5m])
    Legend: "Sent - {{topic}}"
  - Query: rate(kafka_messages_received_total[5m])
    Legend: "Received - {{topic}}"
```

**PromQL**:
```promql
# Producer throughput by topic
sum(rate(kafka_messages_sent_total[5m])) by (topic)

# Consumer throughput by topic/partition
sum(rate(kafka_messages_received_total[5m])) by (topic, partition)

# Bytes produced/consumed
rate(kafka_bytes_sent_total[5m])
rate(kafka_bytes_received_total[5m])
```

### Panel 2: Consumer Lag

```yaml
Title: "Consumer Lag by Topic/Partition"
Target:
  Query: kafka_consumer_lag
  Legend: "{{topic}}-p{{partition}}"

Alert:
  Condition: kafka_consumer_lag > 10000
  Severity: Warning
```

**PromQL**:
```promql
# Current lag
kafka_consumer_lag

# Lag growth rate (increasing lag = backpressure)
rate(kafka_consumer_lag[5m])

# Max lag across all partitions
max(kafka_consumer_lag) by (topic)

# Partitions with lag > threshold
kafka_consumer_lag > 10000
```

### Panel 3: Produce/Consume Latency

```promql
# Producer latency p99
histogram_quantile(0.99, rate(kafka_produce_latency_ms_bucket[5m]))

# Consumer poll latency p95
histogram_quantile(0.95, rate(kafka_consume_latency_ms_bucket[5m]))

# Latency by topic
histogram_quantile(0.95,
  rate(kafka_produce_latency_ms_bucket[5m])
) by (topic)
```

### Panel 4: Kafka Errors

```promql
# Error rate by type
rate(kafka_errors_total[5m]) by (error_type)

# Producer errors vs total sends
rate(kafka_errors_total{operation="produce"}[5m]) /
rate(kafka_messages_sent_total[5m])

# Consumer errors vs total receives
rate(kafka_errors_total{operation="consume"}[5m]) /
rate(kafka_messages_received_total[5m])
```

---

## State Backend Dashboard

### Panel 1: Cache Hit Rate

```yaml
Title: "Cache Hit Rate by Tier"
Target:
  Query: |
    rate(state_cache_hits_total[5m]) /
    (rate(state_cache_hits_total[5m]) + rate(state_cache_misses_total[5m]))
  Legend: "{{cache_tier}}"

Thresholds:
  - 0.8: Green (Good)
  - 0.5: Yellow (Warning)
  - 0.0: Red (Critical)
```

**PromQL**:
```promql
# L1 cache hit rate
rate(state_cache_hits_total{cache_tier="l1"}[5m]) /
(rate(state_cache_hits_total{cache_tier="l1"}[5m]) +
 rate(state_cache_misses_total{cache_tier="l1"}[5m]))

# Overall cache efficiency
sum(rate(state_cache_hits_total[5m])) /
(sum(rate(state_cache_hits_total[5m])) +
 sum(rate(state_cache_misses_total[5m])))

# Cache hit rate by operation
rate(state_cache_hits_total[5m]) /
(rate(state_cache_hits_total[5m]) + rate(state_cache_misses_total[5m]))
by (operation)
```

### Panel 2: State Operation Latency

```promql
# Get operation latency by backend
histogram_quantile(0.95,
  rate(state_operation_duration_ms_bucket{operation="get"}[5m])
) by (backend)

# Put operation latency by backend
histogram_quantile(0.95,
  rate(state_operation_duration_ms_bucket{operation="put"}[5m])
) by (backend)

# Delete operation latency
histogram_quantile(0.99,
  rate(state_operation_duration_ms_bucket{operation="delete"}[5m])
)
```

### Panel 3: State Size

```yaml
Title: "State Size by Backend"
Target:
  Query: state_size_bytes
  Legend: "{{backend}}"
Unit: bytes
Format: IEC (KB, MB, GB)
```

### Panel 4: Lock Contention

```promql
# Lock wait time
rate(state_lock_wait_duration_ms_sum[5m]) /
rate(state_lock_wait_duration_ms_count[5m])

# Lock acquisition failures
rate(state_lock_failures_total[5m])

# Active locks by type
state_active_locks by (lock_type)
```

---

## SLO Dashboard

### Service Level Indicators (SLIs)

#### SLI 1: Availability

```yaml
Title: "Availability (Error Rate)"
Target:
  Query: |
    1 - (
      rate(stream_processor_errors_total[5m]) /
      rate(stream_processor_events_received_total[5m])
    )

SLO: 99.9% (0.999)
Display: Percentage
Thresholds:
  - 0.999: Green
  - 0.99: Yellow
  - 0.95: Red
```

**PromQL**:
```promql
# Current availability (5m window)
1 - (
  sum(rate(stream_processor_errors_total[5m])) /
  sum(rate(stream_processor_events_received_total[5m]))
)

# Availability over 30 days
1 - (
  sum(increase(stream_processor_errors_total[30d])) /
  sum(increase(stream_processor_events_received_total[30d]))
)

# Error budget remaining (99.9% SLO)
(0.001 - (
  sum(increase(stream_processor_errors_total[30d])) /
  sum(increase(stream_processor_events_received_total[30d]))
)) / 0.001
```

#### SLI 2: Latency

```yaml
Title: "Latency SLI (p95 < 100ms)"
Target:
  Query: |
    histogram_quantile(0.95,
      rate(stream_processor_processing_duration_ms_bucket[5m]))

SLO: 100ms (p95)
Thresholds:
  - 100: Green
  - 150: Yellow
  - 200: Red
```

#### SLI 3: Throughput

```yaml
Title: "Throughput SLI (>1000 events/sec)"
Target:
  Query: rate(stream_processor_events_processed_total[5m])

SLO: 1000 events/sec
Display: events/sec
```

### Error Budget Dashboard

```promql
# Error budget remaining (30 day window)
# SLO: 99.9% = 0.1% error budget
(0.001 * sum(increase(stream_processor_events_received_total[30d]))) -
sum(increase(stream_processor_errors_total[30d]))

# Days of error budget remaining at current rate
(
  (0.001 * sum(increase(stream_processor_events_received_total[30d]))) -
  sum(increase(stream_processor_errors_total[30d]))
) /
sum(rate(stream_processor_errors_total[24h]))
/ 86400

# Burn rate (how fast we're consuming error budget)
(rate(stream_processor_errors_total[1h]) /
 rate(stream_processor_events_received_total[1h])) /
0.001
```

---

## Alert Rules

### Prometheus Alert Configuration

**File**: `monitoring/prometheus/alerts.yaml`

```yaml
groups:
  - name: stream_processor_critical
    interval: 30s
    rules:
      # High Error Rate (Critical)
      - alert: StreamProcessorHighErrorRate
        expr: |
          rate(stream_processor_errors_total[5m]) > 10
        for: 5m
        labels:
          severity: critical
          component: stream_processor
        annotations:
          summary: "High error rate detected in stream processor"
          description: |
            Error rate is {{ $value | humanize }} errors/sec (threshold: 10/sec)
            Event type: {{ $labels.event_type }}
          dashboard: "https://grafana.example.com/d/stream-processor"
          runbook: "https://docs.example.com/runbooks/high-error-rate"

      # Processing Latency SLO Breach (Critical)
      - alert: StreamProcessorLatencySLOBreach
        expr: |
          histogram_quantile(0.95,
            rate(stream_processor_processing_duration_ms_bucket[5m])
          ) > 100
        for: 10m
        labels:
          severity: critical
          component: stream_processor
          slo: latency
        annotations:
          summary: "Processing latency exceeds SLO (p95 > 100ms)"
          description: |
            P95 latency: {{ $value | humanize }}ms
            SLO target: 100ms
            Current breach duration: 10+ minutes

      # Consumer Lag (Warning)
      - alert: KafkaConsumerLagHigh
        expr: |
          kafka_consumer_lag > 10000
        for: 5m
        labels:
          severity: warning
          component: kafka
        annotations:
          summary: "Kafka consumer lag is high"
          description: |
            Consumer lag: {{ $value | humanize }} messages
            Topic: {{ $labels.topic }}
            Partition: {{ $labels.partition }}

      # Consumer Lag Critical
      - alert: KafkaConsumerLagCritical
        expr: |
          kafka_consumer_lag > 100000
        for: 5m
        labels:
          severity: critical
          component: kafka
        annotations:
          summary: "Kafka consumer lag is critical"
          description: |
            Consumer lag: {{ $value | humanize }} messages
            This indicates severe backpressure or processing bottleneck
            Topic: {{ $labels.topic }}

      # Cache Hit Rate Low
      - alert: StateCacheHitRateLow
        expr: |
          (
            rate(state_cache_hits_total[5m]) /
            (rate(state_cache_hits_total[5m]) + rate(state_cache_misses_total[5m]))
          ) < 0.5
        for: 10m
        labels:
          severity: warning
          component: state
        annotations:
          summary: "State cache hit rate is low"
          description: |
            Cache hit rate: {{ $value | humanizePercentage }}
            Tier: {{ $labels.cache_tier }}
            Consider cache size tuning or TTL adjustment

      # Cardinality Limit Warning
      - alert: MetricsCardinalityHigh
        expr: |
          metrics_registry_total_series > 8000
        for: 10m
        labels:
          severity: warning
          component: metrics
        annotations:
          summary: "Metrics cardinality approaching limit"
          description: |
            Current series: {{ $value }}
            Limit: 10000 (80% reached)
            Check for high-cardinality labels

      # Cardinality Limit Critical
      - alert: MetricsCardinalityCritical
        expr: |
          metrics_registry_total_series > 9500
        for: 5m
        labels:
          severity: critical
          component: metrics
        annotations:
          summary: "Metrics cardinality critical"
          description: |
            Current series: {{ $value }}
            Limit: 10000 (95% reached)
            IMMEDIATE ACTION REQUIRED - metrics may stop recording

  - name: stream_processor_warning
    interval: 1m
    rules:
      # Increasing Latency Trend
      - alert: StreamProcessorLatencyIncreasing
        expr: |
          deriv(
            histogram_quantile(0.95,
              rate(stream_processor_processing_duration_ms_bucket[5m])
            )[10m:]
          ) > 0
        for: 15m
        labels:
          severity: warning
          component: stream_processor
        annotations:
          summary: "Processing latency is trending upward"
          description: "Latency has been increasing for 15+ minutes"

      # Error Rate Spike
      - alert: StreamProcessorErrorSpike
        expr: |
          (
            rate(stream_processor_errors_total[5m]) /
            rate(stream_processor_events_received_total[5m])
          ) > 0.01
        for: 5m
        labels:
          severity: warning
          component: stream_processor
        annotations:
          summary: "Error rate spike detected (>1%)"
          description: "Error ratio: {{ $value | humanizePercentage }}"

      # Low Throughput
      - alert: StreamProcessorLowThroughput
        expr: |
          rate(stream_processor_events_processed_total[5m]) < 100
        for: 10m
        labels:
          severity: warning
          component: stream_processor
        annotations:
          summary: "Processing throughput is low"
          description: |
            Current: {{ $value | humanize }} events/sec
            Expected: >1000 events/sec

      # Memory Usage High
      - alert: ProcessMemoryHigh
        expr: |
          process_memory_bytes > 450 * 1024 * 1024
        for: 5m
        labels:
          severity: warning
          component: system
        annotations:
          summary: "Process memory usage is high"
          description: |
            Current: {{ $value | humanize1024 }}B
            Target: <500MB

  - name: slo_alerts
    interval: 1m
    rules:
      # Error Budget Burn Rate (Fast)
      - alert: ErrorBudgetBurnRateFast
        expr: |
          (
            (rate(stream_processor_errors_total[1h]) /
             rate(stream_processor_events_received_total[1h]))
            / 0.001
          ) > 14.4
        for: 5m
        labels:
          severity: critical
          slo: availability
        annotations:
          summary: "Error budget burning too fast"
          description: |
            Burn rate: {{ $value }}x normal
            At this rate, monthly error budget will be exhausted in <2 days

      # Error Budget Burn Rate (Slow)
      - alert: ErrorBudgetBurnRateSlow
        expr: |
          (
            (rate(stream_processor_errors_total[6h]) /
             rate(stream_processor_events_received_total[6h]))
            / 0.001
          ) > 6
        for: 30m
        labels:
          severity: warning
          slo: availability
        annotations:
          summary: "Error budget burning above normal rate"
          description: "Burn rate: {{ $value }}x normal"

      # Error Budget Low
      - alert: ErrorBudgetLow
        expr: |
          (
            (0.001 * sum(increase(stream_processor_events_received_total[30d]))) -
            sum(increase(stream_processor_errors_total[30d]))
          ) /
          (0.001 * sum(increase(stream_processor_events_received_total[30d])))
          < 0.1
        labels:
          severity: warning
          slo: availability
        annotations:
          summary: "Error budget is running low"
          description: "Only {{ $value | humanizePercentage }} of monthly error budget remains"
```

---

## Runbook Examples

### Runbook: High Error Rate

**Alert**: `StreamProcessorHighErrorRate`

#### Symptoms
- Error rate >10 errors/sec
- Dashboard shows spike in `stream_processor_errors_total`

#### Impact
- Events may be dropped
- SLO breach risk
- User-facing failures possible

#### Diagnosis

1. **Check error types**:
   ```promql
   rate(stream_processor_errors_total[5m]) by (error_type)
   ```

2. **Identify affected event types**:
   ```promql
   rate(stream_processor_events_received_total[5m]) by (event_type)
   ```

3. **Check recent deployments**:
   ```bash
   kubectl get events --sort-by='.lastTimestamp'
   ```

4. **View application logs**:
   ```bash
   kubectl logs -l app=stream-processor --tail=100
   ```

#### Resolution Steps

1. **If validation errors**: Check for schema changes in upstream
2. **If timeout errors**: Investigate downstream dependencies (state backend, Kafka)
3. **If connection errors**: Check network/firewall rules
4. **If resource errors**: Scale up pods or increase resource limits

#### Escalation
If errors persist >30 minutes, escalate to on-call engineer.

---

### Runbook: Kafka Consumer Lag

**Alert**: `KafkaConsumerLagCritical`

#### Symptoms
- Consumer lag >100K messages
- Increasing lag over time

#### Impact
- Delayed event processing
- Potential data staleness
- Risk of message expiration

#### Diagnosis

1. **Check lag trend**:
   ```promql
   rate(kafka_consumer_lag[10m])
   ```
   - If positive: Lag is growing (backpressure)
   - If negative: Lag is decreasing (catching up)

2. **Check processing rate**:
   ```promql
   rate(stream_processor_events_processed_total[5m])
   ```

3. **Check error rate**:
   ```promql
   rate(stream_processor_errors_total[5m])
   ```

#### Resolution Steps

1. **Scale horizontally**: Increase consumer replicas
   ```bash
   kubectl scale deployment stream-processor --replicas=6
   ```

2. **Optimize processing**: Check for slow operations in traces

3. **Increase partitions**: If single partition is bottleneck

4. **Adjust consumer config**: Increase `max.poll.records` or `fetch.max.bytes`

---

## Summary

This document provides:

1. **4 Production Dashboards**: Overview, Kafka, State, SLO
2. **15+ Alert Rules**: Critical and warning levels
3. **PromQL Query Library**: 50+ queries for metrics analysis
4. **Runbook Templates**: Incident response procedures

All dashboards and alerts are ready to deploy to production Grafana and Prometheus instances.

### Quick Setup

```bash
# Install dashboards
kubectl apply -f monitoring/grafana/dashboards/

# Install alert rules
kubectl apply -f monitoring/prometheus/alerts.yaml

# Access Grafana
kubectl port-forward svc/grafana 3000:3000
```

Then import dashboard JSONs via Grafana UI or provision via ConfigMap.
