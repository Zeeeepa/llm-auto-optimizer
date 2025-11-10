# ML Feature Engineering Quick Reference

**Quick access guide to the Feature Engineering Architecture**

## Overview

The LLM Auto-Optimizer computes **55+ real-time features** for intelligent model selection and optimization.

## Feature Categories

| Category | Count | Examples | Update Frequency |
|----------|-------|----------|-----------------|
| **Request** | 15 | prompt_length, task_type, hour_of_day | Real-time |
| **Model** | 10 | context_length, cost, capabilities | On change |
| **Performance** | 12 | latency_p95, success_rate, throughput | 5min/1hr windows |
| **Feedback** | 8 | user_rating, regeneration_rate | Session/1hr |
| **Context** | 10 | cpu_load, queue_depth, budget | Every 10s |
| **Derived** | 10 | cost_latency_score, quality_ratio | On request |

**Total: 55 features**

## Architecture at a Glance

```
Event Stream → Feature Extraction → Aggregation → Feature Store → Serving API
                                    (Windows)      (3-tier cache)  (<5ms latency)
```

## Feature Store

### 3-Tier Cache Architecture

1. **L1 (Memory)**: 512 MB, <1ms latency, 1 hour TTL
2. **L2 (Redis)**: 10 GB, <5ms latency, 24 hour TTL
3. **L3 (PostgreSQL)**: Unlimited, <50ms latency, 7 days retention

### Storage Schema

```
Redis Key Pattern: feature:{entity_type}:{entity_id}:{feature_group}:{version}
Example: feature:model:gpt-4:performance:v1
```

## Feature Serving API

### REST Endpoints

```bash
# Get features for single entity
GET /features/:entity_type/:entity_id?features=REQ_001,REQ_002,PERF_001

# Batch get features
POST /features/batch
{
  "requests": [
    {
      "entity_id": "gpt-4",
      "entity_type": "model",
      "features": ["PERF_001", "PERF_002", "PERF_003"]
    }
  ]
}
```

### Response Format

```json
{
  "entity_id": "gpt-4",
  "features": {
    "latency_p95_5min": 234.56,
    "success_rate_5min": 0.98,
    "avg_cost_per_request_5min": 0.0023
  },
  "metadata": {
    "computed_at": "2025-11-10T05:23:00Z",
    "version": 1,
    "cache_hit": true,
    "freshness_seconds": 12
  },
  "latency_ms": 2.3
}
```

## Key Features by Use Case

### Model Selection

**Critical Features:**
- `prompt_length` (REQ_001) - Route short prompts to cheaper models
- `task_type_classification` (REQ_005) - Match task to model capability
- `model_capability_score` (MDL_004) - Quality vs cost tradeoff
- `latency_p95_5min` (PERF_002) - Recent latency performance
- `cost_latency_score` (DRV_002) - Combined efficiency metric

### Cost Optimization

**Critical Features:**
- `model_cost_per_1k_input` (MDL_002)
- `model_cost_per_1k_output` (MDL_003)
- `avg_cost_per_request_5min` (PERF_009)
- `available_budget_ratio` (CTX_005)
- `quality_cost_ratio` (DRV_003)

### Quality Assurance

**Critical Features:**
- `success_rate_5min` (PERF_006)
- `error_rate_5min` (PERF_008)
- `avg_user_rating_1hr` (FDBK_002)
- `regeneration_rate_1hr` (FDBK_004)

### Load Balancing

**Critical Features:**
- `concurrent_requests` (CTX_004)
- `queue_depth_requests` (CTX_003)
- `system_load_cpu` (CTX_001)
- `model_availability` (CTX_007)

## Window Configurations

### Performance Windows (5 min Tumbling)

```yaml
window_type: tumbling
size: 300s
features:
  - latency_p50_5min (PERF_001)
  - latency_p95_5min (PERF_002)
  - latency_p99_5min (PERF_003)
  - success_rate_5min (PERF_006)
  - error_rate_5min (PERF_008)
```

### Performance Windows (1 hr Sliding)

```yaml
window_type: sliding
size: 3600s
slide: 300s
features:
  - latency_p50_1hr (PERF_004)
  - latency_p95_1hr (PERF_005)
  - success_rate_1hr (PERF_007)
```

### Feedback Windows (Session)

```yaml
window_type: session
gap: 1800s  # 30 min inactivity
max_duration: 14400s  # 4 hours
features:
  - avg_user_rating_session (FDBK_001)
  - task_completion_rate_session (FDBK_005)
  - follow_up_rate_session (FDBK_008)
```

## Performance SLAs

| Metric | Target |
|--------|--------|
| Feature serving latency (p99) | <5ms |
| Feature extraction latency | <1ms |
| Event ingestion throughput | 10,000/s |
| Feature serving throughput | 50,000/s |
| Cache hit rate | >95% |
| Data freshness (real-time features) | <1s |
| Data freshness (windowed features) | <5min |

## Common Operations

### Extract Features from Event

```rust
use llm_optimizer_features::extraction::StatelessFeatureExtractor;

let extractor = StatelessFeatureExtractor::new();
let features = extractor.extract_request_features(&event).await?;
```

### Serve Features to ML Model

```rust
use llm_optimizer_features::serving::FeatureServingAPI;

let request = FeatureRequest {
    entity_id: "gpt-4".to_string(),
    entity_type: "model".to_string(),
    features: vec![
        "latency_p95_5min".to_string(),
        "success_rate_5min".to_string(),
        "avg_cost_per_request_5min".to_string(),
    ],
    version: None,
    timestamp: None,
};

let response = api.serve_features(request).await?;
```

### Validate Features

```rust
use llm_optimizer_features::validation::FeatureValidator;

let validator = FeatureValidator::new();
let validated = validator.validate_vector(&features)?;
```

### Detect Feature Drift

```rust
use llm_optimizer_features::validation::FeatureDriftDetector;

let detector = FeatureDriftDetector::new(config);
if let Some(alert) = detector.detect_drift("latency_p95_5min") {
    println!("Drift detected: {:?}", alert);
}
```

## Feature Groups for Efficient Access

| Feature Group | Key Pattern | Use Case |
|---------------|-------------|----------|
| `request` | `feature:request:{request_id}` | Per-request features |
| `model:static` | `feature:model:{model_id}:static` | Model metadata |
| `model:perf_5min` | `feature:model:{model_id}:perf_5min` | Recent performance |
| `model:perf_1hr` | `feature:model:{model_id}:perf_1hr` | Hourly performance |
| `user:session` | `feature:user:{user_id}:session:{session_id}` | User session |
| `system:context` | `feature:system:context` | System state |
| `derived` | `feature:derived:{request_id}` | Computed features |

## Monitoring Dashboards

### Key Metrics to Monitor

1. **Feature Serving Latency** (p50, p95, p99)
2. **Cache Hit Rate** (L1, L2, L3)
3. **Feature Freshness** (age distribution)
4. **Feature Drift Alerts** (count by severity)
5. **Validation Errors** (by feature)
6. **Serving Throughput** (requests/s)
7. **Feature Quality Score** (0-1)

### Prometheus Queries

```promql
# Feature serving latency p99
histogram_quantile(0.99, feature_serving_latency_ms_bucket)

# Cache hit rate
sum(rate(feature_cache_hits_total{hit="hit"}[5m])) /
sum(rate(feature_cache_hits_total[5m]))

# Feature drift alerts
sum by (feature_name, severity) (feature_drift_alerts_total)
```

## Troubleshooting

### High Latency (>5ms p99)

1. Check cache hit rate - should be >95%
2. Verify Redis cluster health
3. Check for hot keys (uneven load)
4. Review feature group organization
5. Consider adding more cache capacity

### Feature Drift Detected

1. Review baseline statistics
2. Check for data quality issues upstream
3. Validate event schema changes
4. Review recent deployments
5. Update baseline if legitimate distribution shift

### Missing Features

1. Check feature extraction pipeline health
2. Verify window closure logic
3. Review watermark progression
4. Check state backend connectivity
5. Validate feature registry entries

## Related Documentation

- [Full Architecture Document](/workspaces/llm-auto-optimizer/docs/ML_FEATURE_ENGINEERING_ARCHITECTURE.md)
- [Stream Processing Specification](/workspaces/llm-auto-optimizer/docs/STREAM_PROCESSING_WINDOWING_SPECIFICATION.md)
- [Redis State Backend Guide](/workspaces/llm-auto-optimizer/docs/redis-state-backend.md)
- [Decision Engine Context](/workspaces/llm-auto-optimizer/crates/decision/src/context.rs)

## Quick Start

```bash
# 1. Start feature pipeline
cargo run --bin feature-pipeline

# 2. Query features via API
curl http://localhost:8080/features/model/gpt-4?features=PERF_001,PERF_002

# 3. Monitor metrics
open http://localhost:9090/metrics  # Prometheus
open http://localhost:3000          # Grafana
```

---

**Document Version:** 1.0
**Last Updated:** 2025-11-10
**For detailed specifications, see:** [ML_FEATURE_ENGINEERING_ARCHITECTURE.md](/workspaces/llm-auto-optimizer/docs/ML_FEATURE_ENGINEERING_ARCHITECTURE.md)
