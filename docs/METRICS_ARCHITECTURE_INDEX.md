# Metrics Export Architecture - Index

**Complete guide to enterprise-grade observability for the Stream Processor**

---

## Overview

This index provides a complete reference to the metrics export architecture documentation, implementation plans, and operational guides for the LLM Auto Optimizer stream processor.

**Status**: Design Complete ✅
**Version**: 1.0
**Last Updated**: 2025-11-10

---

## Document Hierarchy

```
Metrics Export Architecture
│
├── 📋 Architecture & Design
│   ├── METRICS_EXPORT_ARCHITECTURE.md (Main specification)
│   └── METRICS_ARCHITECTURE_INDEX.md (This file)
│
├── 🚀 Implementation
│   ├── METRICS_IMPLEMENTATION_PLAN.md (Detailed roadmap)
│   └── METRICS_QUICK_REFERENCE.md (Developer guide)
│
├── 📊 Operations
│   └── METRICS_DASHBOARDS_AND_ALERTS.md (Grafana & Prometheus)
│
└── 🔗 Related Documentation
    ├── DISTRIBUTED_STATE_ARCHITECTURE.md
    ├── WINDOWING_AGGREGATION_ARCHITECTURE.md
    └── stream-processor-architecture.md
```

---

## Quick Navigation

### For Architects

**Start here**: [METRICS_EXPORT_ARCHITECTURE.md](./METRICS_EXPORT_ARCHITECTURE.md)
- Complete architecture specification
- Performance requirements
- Design patterns and trade-offs
- Technology selection rationale

**Key Sections**:
1. [Architecture Overview](./METRICS_EXPORT_ARCHITECTURE.md#architecture-overview)
2. [Prometheus Metrics System](./METRICS_EXPORT_ARCHITECTURE.md#prometheus-metrics-system)
3. [OpenTelemetry Integration](./METRICS_EXPORT_ARCHITECTURE.md#opentelemetry-integration)
4. [Performance Architecture](./METRICS_EXPORT_ARCHITECTURE.md#performance-architecture)

### For Developers

**Start here**: [METRICS_QUICK_REFERENCE.md](./METRICS_QUICK_REFERENCE.md)
- Quick start guide (30 seconds to metrics)
- Code examples and patterns
- Common pitfalls and solutions
- Performance tips

**Then read**: [METRICS_IMPLEMENTATION_PLAN.md](./METRICS_IMPLEMENTATION_PLAN.md)
- Phase-by-phase implementation
- File structure and scaffolding
- Code templates
- Testing strategy

**Key Sections**:
1. [Quick Start](./METRICS_QUICK_REFERENCE.md#quick-start)
2. [Metric Types Cheat Sheet](./METRICS_QUICK_REFERENCE.md#metric-types-cheat-sheet)
3. [Common Patterns](./METRICS_QUICK_REFERENCE.md#common-patterns)
4. [Implementation Roadmap](./METRICS_IMPLEMENTATION_PLAN.md#implementation-roadmap)

### For SREs & Operators

**Start here**: [METRICS_DASHBOARDS_AND_ALERTS.md](./METRICS_DASHBOARDS_AND_ALERTS.md)
- Grafana dashboard configurations
- Prometheus alert rules
- Runbook examples
- Troubleshooting guides

**Key Sections**:
1. [Dashboard Overview](./METRICS_DASHBOARDS_AND_ALERTS.md#dashboard-overview)
2. [Alert Rules](./METRICS_DASHBOARDS_AND_ALERTS.md#alert-rules)
3. [Runbook Examples](./METRICS_DASHBOARDS_AND_ALERTS.md#runbook-examples)
4. [SLO Dashboard](./METRICS_DASHBOARDS_AND_ALERTS.md#slo-dashboard)

### For Product Managers

**Read this**:
- [Executive Summary](./METRICS_EXPORT_ARCHITECTURE.md#executive-summary)
- [Performance Targets](./METRICS_EXPORT_ARCHITECTURE.md#performance-targets)
- [SLO Dashboard](./METRICS_DASHBOARDS_AND_ALERTS.md#slo-dashboard)
- [Implementation Roadmap](./METRICS_IMPLEMENTATION_PLAN.md#implementation-roadmap)

---

## Architecture Summary

### Key Components

| Component | Purpose | Technology | Status |
|-----------|---------|------------|--------|
| **Metrics Registry** | Central metric storage | Lock-free atomics, DashMap | Ready for impl |
| **Counter** | Monotonic event counts | AtomicU64 | Ready for impl |
| **Gauge** | Up/down values | AtomicI64 (f64 bits) | Ready for impl |
| **Histogram** | Latency distributions | Pre-allocated buckets | Ready for impl |
| **Summary** | Streaming quantiles | T-Digest algorithm | Ready for impl |
| **Prometheus Exporter** | /metrics HTTP endpoint | Axum server | Ready for impl |
| **OTel Provider** | Trace export | OTLP/gRPC | Ready for impl |
| **Labels** | Multi-dimensional metrics | Pre-computed hashes | Ready for impl |
| **Cardinality Tracker** | Prevent label explosion | Active monitoring | Ready for impl |

### Performance Characteristics

| Metric | Target | Implementation Strategy |
|--------|--------|-------------------------|
| Recording Overhead | <1μs (p99) | Lock-free atomic operations |
| Memory Overhead | <50MB | Shared bucket configs, Arc |
| Export Latency | <100ms (p95) | Pre-computed label encoding |
| HTTP /metrics | <50ms (p95) | Efficient Prometheus renderer |
| Cardinality Limit | 10,000 series | Active tracking with alerts |
| Thread Contention | Zero | Lock-free counters, RwLock for rare ops |

### Technology Stack

```yaml
Metrics Infrastructure:
  Core:
    - Rust std::sync::atomic (lock-free primitives)
    - DashMap (concurrent hash map)
    - Arc (zero-copy sharing)

  Prometheus:
    - prometheus-client (v0.22)
    - Axum (HTTP server)
    - Text format 0.0.4

  OpenTelemetry:
    - opentelemetry (v0.24)
    - opentelemetry-otlp (v0.17)
    - opentelemetry_sdk (v0.24)

  HTTP Server:
    - Axum (v0.7)
    - Tower (middleware)
    - Tokio (async runtime)
```

---

## Metrics Catalog

### Application Metrics

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `stream_processor_events_received_total` | Counter | `event_type` | Total events received |
| `stream_processor_events_processed_total` | Counter | `event_type` | Total events processed |
| `stream_processor_events_dropped_total` | Counter | `event_type`, `reason` | Total events dropped |
| `stream_processor_processing_duration_ms` | Histogram | `event_type` | Processing latency |
| `stream_processor_end_to_end_latency_ms` | Histogram | `event_type` | Event time to completion |
| `stream_processor_errors_total` | Counter | `error_type` | Total errors |
| `stream_processor_active_windows` | Gauge | `window_type` | Active window count |
| `stream_processor_window_triggers_total` | Counter | `window_type` | Window triggers |
| `stream_processor_aggregations_computed_total` | Counter | `aggregation_type` | Aggregations computed |
| `stream_processor_deduplication_hits_total` | Counter | - | Duplicate events detected |

### Kafka Metrics

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `kafka_messages_sent_total` | Counter | `topic` | Messages sent to Kafka |
| `kafka_messages_received_total` | Counter | `topic`, `partition` | Messages consumed |
| `kafka_bytes_sent_total` | Counter | `topic` | Bytes sent |
| `kafka_bytes_received_total` | Counter | `topic` | Bytes received |
| `kafka_produce_latency_ms` | Histogram | `topic` | Produce latency |
| `kafka_consume_latency_ms` | Histogram | `topic` | Consume latency |
| `kafka_consumer_lag` | Gauge | `topic`, `partition` | Consumer lag |
| `kafka_errors_total` | Counter | `operation`, `error_type` | Kafka errors |

### State Backend Metrics

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `state_operations_total` | Counter | `backend`, `operation` | State operations |
| `state_operation_duration_ms` | Histogram | `backend`, `operation` | Operation latency |
| `state_cache_hits_total` | Counter | `cache_tier` | Cache hits |
| `state_cache_misses_total` | Counter | `cache_tier` | Cache misses |
| `state_size_bytes` | Gauge | `backend` | State size |
| `state_active_locks` | Gauge | `lock_type` | Active locks |
| `state_lock_wait_duration_ms` | Histogram | `lock_type` | Lock wait time |

### System Metrics

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `process_cpu_usage_ratio` | Gauge | - | CPU usage (0.0-1.0) |
| `process_memory_bytes` | Gauge | - | Memory usage |
| `process_active_tasks` | Gauge | - | Active async tasks |
| `process_tasks_spawned_total` | Counter | - | Total tasks spawned |

---

## Implementation Phases

### Phase 1: Core Infrastructure (Weeks 1-2)

**Goal**: Lock-free metric primitives

**Deliverables**:
- ✅ Lock-free Counter
- ✅ Atomic Gauge
- ✅ Histogram with buckets
- ✅ Summary with quantiles
- ✅ Label system
- ✅ Metrics registry
- ✅ Cardinality tracking

**Files**: 15 new Rust files, ~3,000 LOC

**Success Criteria**:
- <1μs recording overhead
- Zero allocations in hot path
- 100% test coverage

### Phase 2: Prometheus Export (Week 3)

**Goal**: HTTP /metrics endpoint

**Deliverables**:
- ✅ Prometheus text renderer
- ✅ Axum HTTP server
- ✅ /metrics endpoint
- ✅ /health endpoint

**Files**: 4 new files, ~1,000 LOC

**Success Criteria**:
- <50ms response time
- Valid Prometheus format
- Concurrent scraping support

### Phase 3: OpenTelemetry (Week 4)

**Goal**: Distributed tracing

**Deliverables**:
- ✅ OTel provider
- ✅ Span instrumentation
- ✅ Context propagation
- ✅ OTLP exporter

**Files**: 5 new files, ~1,200 LOC

**Success Criteria**:
- Successful trace export
- <1% overhead
- Kafka header propagation

### Phase 4: Application Integration (Week 5)

**Goal**: Instrument all components

**Deliverables**:
- ✅ Processor metrics
- ✅ Window metrics
- ✅ State backend metrics
- ✅ Kafka metrics

**Files**: 8 new files, ~2,000 LOC

**Success Criteria**:
- 100% component coverage
- End-to-end tracing
- Complete observability

### Phase 5: Operations (Week 6)

**Goal**: Production readiness

**Deliverables**:
- ✅ Grafana dashboards (4)
- ✅ Alert rules (15+)
- ✅ Runbooks (5)
- ✅ Documentation

**Files**: 10 config files, 20 pages docs

**Success Criteria**:
- Working dashboards
- Tested alerts
- Complete runbooks

---

## Configuration Examples

### Minimal (Development)

```yaml
observability:
  service:
    name: "stream-processor"
    environment: "dev"

  prometheus:
    enabled: true
    endpoint:
      port: 9090

  opentelemetry:
    enabled: false
```

### Production

```yaml
observability:
  service:
    name: "llm-optimizer-processor"
    version: "1.0.0"
    environment: "production"
    instance_id: "${HOSTNAME}"

  prometheus:
    enabled: true
    endpoint:
      host: "0.0.0.0"
      port: 9090
    registry:
      max_cardinality: 10000
      max_labels_per_metric: 10

  opentelemetry:
    enabled: true
    otlp:
      endpoint: "http://otel-collector:4317"
    tracing:
      sampling:
        strategy: "parent_based"
        ratio: 0.1
    metrics:
      enabled: true
      export_interval_seconds: 60
```

---

## Key PromQL Queries

### Golden Signals

```promql
# 1. Latency (p95)
histogram_quantile(0.95,
  rate(stream_processor_processing_duration_ms_bucket[5m]))

# 2. Traffic (throughput)
rate(stream_processor_events_processed_total[5m])

# 3. Errors (error rate)
rate(stream_processor_errors_total[5m]) /
rate(stream_processor_events_received_total[5m])

# 4. Saturation (queue depth)
kafka_consumer_lag
```

### SLI Queries

```promql
# Availability (99.9% SLO)
1 - (
  rate(stream_processor_errors_total[5m]) /
  rate(stream_processor_events_received_total[5m])
)

# Latency SLI (p95 < 100ms)
histogram_quantile(0.95,
  rate(stream_processor_processing_duration_ms_bucket[5m])) < 100

# Throughput SLI (>1000 events/sec)
rate(stream_processor_events_processed_total[5m]) > 1000
```

---

## Common Patterns

### Pattern 1: Measure Duration

```rust
let start = Instant::now();

// Your operation
let result = do_work().await?;

// Record duration
metrics.duration_ms.observe(start.elapsed().as_millis() as f64);
```

### Pattern 2: Count with Labels

```rust
match result {
    Ok(_) => metrics.requests.get_or_create(&[("status", "success")]).inc(),
    Err(e) => {
        let error_type = error_type_label(&e);
        metrics.errors.get_or_create(&[("error_type", error_type)]).inc();
    }
}
```

### Pattern 3: Track In-Flight

```rust
metrics.in_flight.inc();
let _guard = scopeguard::guard((), |_| {
    metrics.in_flight.dec();
});

// Process request
// Gauge auto-decrements on drop
```

---

## Best Practices

### Naming

✅ **DO**:
- Use `snake_case`
- Include namespace: `stream_processor_*`
- Add units: `_bytes`, `_ms`, `_ratio`
- End counters with `_total`

❌ **DON'T**:
- Use camelCase or kebab-case
- Mix units (use `_ms` not `_seconds`)
- Omit `_total` from counters

### Labels

✅ **DO**:
- Keep cardinality low (<100 values)
- Use labels: `status`, `method`, `type`
- Pre-compute label sets
- Sort labels consistently

❌ **DON'T**:
- Use: `user_id`, `session_id`, `request_id`
- Create labels dynamically
- Exceed 10 labels per metric

### Performance

✅ **DO**:
- Reuse metric instances
- Use lock-free counters
- Pre-compute encodings
- Batch observations

❌ **DON'T**:
- Look up metrics in hot path
- Use Summary when Histogram works
- Observe per-event in tight loops

---

## Troubleshooting

### Issue: High Cardinality

**Symptom**: Prometheus memory growing, slow queries

**Fix**:
```bash
# Check current cardinality
curl http://localhost:9090/api/v1/label/__name__/values | jq '. | length'

# Find high-cardinality metrics
curl -s http://localhost:9090/metrics | grep -v "^#" | cut -d'{' -f1 | sort | uniq -c | sort -rn

# Reduce by aggregating labels
# BEFORE: labels: &[("user_id", user_id)]
# AFTER:  labels: &[("user_tier", tier)]
```

### Issue: Slow /metrics

**Symptom**: Scrapes timeout (>10s)

**Fix**:
```rust
// Check render time
let start = Instant::now();
let output = registry.render_prometheus();
println!("Render: {:?}", start.elapsed());

// Solutions:
// 1. Reduce cardinality
// 2. Increase scrape interval (30s → 60s)
// 3. Enable metric filtering
```

### Issue: Missing Traces

**Symptom**: Spans not appearing in Jaeger

**Fix**:
```bash
# Check OTLP endpoint
curl http://otel-collector:4317/v1/traces

# Verify sampling
# If always_off, no traces exported

# Check for errors
kubectl logs -l app=stream-processor | grep -i otel
```

---

## Resources

### Internal Documentation

- [Stream Processor Architecture](./stream-processor-architecture.md)
- [Windowing & Aggregation](./WINDOWING_AGGREGATION_ARCHITECTURE.md)
- [Distributed State](./DISTRIBUTED_STATE_ARCHITECTURE.md)
- [Kafka Integration](./KAFKA_INTEGRATION_SUMMARY.md)

### External Resources

- [Prometheus Best Practices](https://prometheus.io/docs/practices/)
- [OpenTelemetry Tracing](https://opentelemetry.io/docs/concepts/signals/traces/)
- [Grafana Dashboard Design](https://grafana.com/docs/grafana/latest/dashboards/)
- [SLO/SLI Guide](https://sre.google/workbook/implementing-slos/)

### Code Examples

- [Prometheus Client Rust](https://github.com/prometheus/client_rust)
- [OpenTelemetry Rust](https://github.com/open-telemetry/opentelemetry-rust)
- [Axum Examples](https://github.com/tokio-rs/axum/tree/main/examples)

---

## Quick Commands

```bash
# View metrics
curl http://localhost:9090/metrics

# Query Prometheus
curl 'http://prometheus:9090/api/v1/query?query=up'

# Export dashboard
curl http://grafana:3000/api/dashboards/uid/stream-processor

# Tail logs with metrics
kubectl logs -f -l app=stream-processor | grep -E "metric|trace"

# Check cardinality
curl http://localhost:9090/metrics | grep -v "^#" | wc -l
```

---

## Contact & Support

- **Architecture Questions**: See [METRICS_EXPORT_ARCHITECTURE.md](./METRICS_EXPORT_ARCHITECTURE.md)
- **Implementation Help**: See [METRICS_IMPLEMENTATION_PLAN.md](./METRICS_IMPLEMENTATION_PLAN.md)
- **Operational Issues**: See [METRICS_DASHBOARDS_AND_ALERTS.md](./METRICS_DASHBOARDS_AND_ALERTS.md)
- **Quick Reference**: See [METRICS_QUICK_REFERENCE.md](./METRICS_QUICK_REFERENCE.md)

---

## Document Status

| Document | Status | Completeness | Review Status |
|----------|--------|--------------|---------------|
| METRICS_EXPORT_ARCHITECTURE.md | ✅ Complete | 100% | Ready for review |
| METRICS_IMPLEMENTATION_PLAN.md | ✅ Complete | 100% | Ready for review |
| METRICS_QUICK_REFERENCE.md | ✅ Complete | 100% | Ready for review |
| METRICS_DASHBOARDS_AND_ALERTS.md | ✅ Complete | 100% | Ready for review |
| METRICS_ARCHITECTURE_INDEX.md | ✅ Complete | 100% | Ready for review |

**Last Updated**: 2025-11-10
**Version**: 1.0
**Next Review**: Before Phase 1 implementation

---

## Changelog

### 2025-11-10 - v1.0 (Initial Release)
- Complete architecture specification
- Implementation plan with code scaffolding
- Quick reference guide
- Dashboard and alert configurations
- Index and navigation guide

**Ready for Implementation**: Yes ✅
