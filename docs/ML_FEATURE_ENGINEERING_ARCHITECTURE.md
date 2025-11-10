# ML Feature Engineering Architecture for LLM Auto-Optimizer

**Version:** 1.0
**Date:** 2025-11-10
**Status:** Technical Specification
**Owner:** LLM DevOps Team

## Executive Summary

This document specifies a comprehensive real-time feature engineering system for the LLM Auto-Optimizer that computes 50+ ML features from streaming events with sub-5ms serving latency. The architecture leverages the existing stream processor's windowing and aggregation capabilities to build a production-ready feature store that enables intelligent model selection, parameter optimization, and cost-performance trade-off decisions.

**Key Goals:**
- **Real-time Feature Computation**: Process features in-stream with <100ms latency
- **Low-Latency Serving**: Serve features to ML models in <5ms (p99)
- **Feature Versioning**: Support A/B testing and gradual rollouts
- **Drift Detection**: Monitor feature distributions for data quality
- **Scalability**: Handle 10,000+ requests/second with horizontal scaling

---

## Table of Contents

1. [Feature Catalog (50+ Features)](#1-feature-catalog)
2. [Feature Store Architecture](#2-feature-store-architecture)
3. [Real-Time Feature Pipeline](#3-real-time-feature-pipeline)
4. [Integration with Stream Processor](#4-integration-with-stream-processor)
5. [Feature Serving API](#5-feature-serving-api)
6. [Feature Versioning & Validation](#6-feature-versioning--validation)
7. [Monitoring & Drift Detection](#7-monitoring--drift-detection)
8. [Rust Implementation Design](#8-rust-implementation-design)
9. [Testing Strategy](#9-testing-strategy)
10. [Performance Requirements](#10-performance-requirements)

---

## 1. Feature Catalog

### 1.1 Request Features (15 features)

These features are extracted directly from incoming LLM requests and are stateless.

| Feature ID | Name | Type | Description | Computation |
|------------|------|------|-------------|-------------|
| **REQ_001** | `prompt_length` | `f64` | Number of tokens in prompt (log-normalized) | `ln(token_count + 1) / 10.0` |
| **REQ_002** | `prompt_char_count` | `f64` | Character count (normalized) | `char_count / 10000.0` |
| **REQ_003** | `prompt_complexity` | `f64` | Syntactic complexity score (0-1) | Flesch-Kincaid readability |
| **REQ_004** | `expected_output_length` | `f64` | Expected response tokens (categorical) | `{Short: 0.0, Medium: 0.5, Long: 1.0}` |
| **REQ_005** | `task_type_classification` | `f64` | Task type encoding | `{classify: 0.0, generate: 0.33, extract: 0.67, summarize: 1.0}` |
| **REQ_006** | `prompt_language` | `f64` | Language code encoding | One-hot or hash encoding |
| **REQ_007** | `prompt_domain` | `f64` | Domain specificity (0-1) | Presence of domain terms (medical, legal, etc.) |
| **REQ_008** | `request_priority` | `f64` | User-specified priority (normalized) | `priority / 10.0` |
| **REQ_009** | `hour_of_day_cos` | `f64` | Hour cyclical encoding (cos) | `cos(hour / 24 * 2π)` |
| **REQ_010** | `hour_of_day_sin` | `f64` | Hour cyclical encoding (sin) | `sin(hour / 24 * 2π)` |
| **REQ_011** | `day_of_week_cos` | `f64` | Day cyclical encoding (cos) | `cos(day / 7 * 2π)` |
| **REQ_012** | `day_of_week_sin` | `f64` | Day cyclical encoding (sin) | `sin(day / 7 * 2π)` |
| **REQ_013** | `is_weekend` | `f64` | Weekend flag | `{weekday: 0.0, weekend: 1.0}` |
| **REQ_014** | `session_depth` | `f64` | Requests in current session | Normalized count |
| **REQ_015** | `user_tier` | `f64` | User subscription tier | `{free: 0.0, pro: 0.5, enterprise: 1.0}` |

### 1.2 Model Features (10 features)

Features describing each available LLM model's characteristics and capabilities.

| Feature ID | Name | Type | Description | Computation |
|------------|------|------|-------------|-------------|
| **MDL_001** | `model_context_length` | `f64` | Max context window (log-normalized) | `ln(context_len) / ln(200000)` |
| **MDL_002** | `model_cost_per_1k_input` | `f64` | Cost per 1K input tokens | Normalized pricing |
| **MDL_003** | `model_cost_per_1k_output` | `f64` | Cost per 1K output tokens | Normalized pricing |
| **MDL_004** | `model_capability_score` | `f64` | Model capability rating (0-1) | Benchmark aggregate (MMLU, HumanEval) |
| **MDL_005** | `model_training_cutoff_age` | `f64` | Days since training cutoff | `days_since_cutoff / 365.0` |
| **MDL_006** | `model_supports_functions` | `f64` | Function calling support | `{no: 0.0, yes: 1.0}` |
| **MDL_007** | `model_supports_vision` | `f64` | Vision capability | `{no: 0.0, yes: 1.0}` |
| **MDL_008** | `model_supports_json_mode` | `f64` | Structured output support | `{no: 0.0, yes: 1.0}` |
| **MDL_009** | `model_provider` | `f64` | Provider encoding | Hash encoding (OpenAI, Anthropic, etc.) |
| **MDL_010** | `model_size_class` | `f64` | Model size category | `{small: 0.0, medium: 0.5, large: 1.0}` |

### 1.3 Historical Performance Features (12 features)

Windowed aggregations of recent model performance (computed via stream processor).

| Feature ID | Name | Type | Window | Description |
|------------|------|------|--------|-------------|
| **PERF_001** | `latency_p50_5min` | `f64` | 5min tumbling | Median latency (ms) in last 5 minutes |
| **PERF_002** | `latency_p95_5min` | `f64` | 5min tumbling | 95th percentile latency (ms) |
| **PERF_003** | `latency_p99_5min` | `f64` | 5min tumbling | 99th percentile latency (ms) |
| **PERF_004** | `latency_p50_1hr` | `f64` | 1hr sliding | Median latency over 1 hour |
| **PERF_005** | `latency_p95_1hr` | `f64` | 1hr sliding | 95th percentile latency over 1 hour |
| **PERF_006** | `success_rate_5min` | `f64` | 5min tumbling | Success rate (no errors) |
| **PERF_007** | `success_rate_1hr` | `f64` | 1hr sliding | Success rate over 1 hour |
| **PERF_008** | `error_rate_5min` | `f64` | 5min tumbling | Error rate (failures / total) |
| **PERF_009** | `avg_cost_per_request_5min` | `f64` | 5min tumbling | Average cost per request |
| **PERF_010** | `throughput_qps_5min` | `f64` | 5min tumbling | Queries per second |
| **PERF_011** | `token_efficiency_5min` | `f64` | 5min tumbling | Output tokens / input tokens ratio |
| **PERF_012** | `retry_rate_5min` | `f64` | 5min tumbling | Retry attempts / total requests |

### 1.4 User Feedback Features (8 features)

Aggregations of user satisfaction and feedback signals.

| Feature ID | Name | Type | Window | Description |
|------------|------|------|--------|-------------|
| **FDBK_001** | `avg_user_rating_session` | `f64` | Session | Average rating (1-5) in session |
| **FDBK_002** | `avg_user_rating_1hr` | `f64` | 1hr sliding | Average rating over 1 hour |
| **FDBK_003** | `thumbs_up_ratio_1hr` | `f64` | 1hr sliding | Thumbs up / total feedback |
| **FDBK_004** | `regeneration_rate_1hr` | `f64` | 1hr sliding | Requests regenerated / total |
| **FDBK_005** | `task_completion_rate_session` | `f64` | Session | Tasks marked complete / total |
| **FDBK_006** | `avg_response_time_user` | `f64` | Session | Time user takes to respond (engagement) |
| **FDBK_007** | `copy_paste_rate_1hr` | `f64` | 1hr sliding | Responses copied / total |
| **FDBK_008** | `follow_up_rate_session` | `f64` | Session | Follow-up questions / total |

### 1.5 Contextual System Features (10 features)

Real-time system state and environmental features.

| Feature ID | Name | Type | Description | Source |
|------------|------|------|-------------|--------|
| **CTX_001** | `system_load_cpu` | `f64` | Current CPU utilization (0-1) | System metrics |
| **CTX_002** | `system_load_memory` | `f64` | Memory utilization (0-1) | System metrics |
| **CTX_003** | `queue_depth_requests` | `f64` | Pending requests (log-normalized) | Queue monitor |
| **CTX_004** | `concurrent_requests` | `f64` | Active requests (normalized) | Request tracker |
| **CTX_005** | `available_budget_ratio` | `f64` | Remaining budget / total | Budget monitor |
| **CTX_006** | `time_until_budget_reset` | `f64` | Hours until reset (normalized) | Budget monitor |
| **CTX_007** | `model_availability` | `f64` | Model uptime/availability | Health checker |
| **CTX_008** | `recent_timeout_rate_5min` | `f64` | Timeouts / requests (5min) | Stream processor |
| **CTX_009** | `api_quota_remaining` | `f64` | API quota left (0-1) | API manager |
| **CTX_010** | `region_load` | `f64` | Regional traffic load | Load balancer |

### 1.6 Derived Features (10 features)

Complex features derived from combinations or embeddings.

| Feature ID | Name | Type | Description | Computation |
|------------|------|------|-------------|-------------|
| **DRV_001** | `prompt_embedding_dim_1-8` | `[f64; 8]` | Prompt embedding (8D via dimensionality reduction) | BERT → PCA/UMAP |
| **DRV_002** | `cost_latency_score` | `f64` | Combined cost-latency metric | `(cost_norm + latency_norm) / 2` |
| **DRV_003** | `quality_cost_ratio` | `f64` | Quality per dollar | `success_rate / cost_per_request` |
| **DRV_004** | `prompt_similarity_to_avg` | `f64` | Similarity to average prompt | Cosine similarity to centroid |
| **DRV_005** | `model_fit_score` | `f64` | Model-prompt fit | Prompt features · Model features |
| **DRV_006** | `user_behavior_cluster` | `f64` | User cluster ID | K-means clustering (online) |
| **DRV_007** | `time_pressure_indicator` | `f64` | Urgency signal | `priority × (1 - available_budget)` |
| **DRV_008** | `load_adjusted_latency` | `f64` | Expected latency under load | `latency_p95 × (1 + queue_depth)` |
| **DRV_009** | `model_fatigue_score` | `f64` | Overuse penalty | Exponential decay on recent usage |
| **DRV_010** | `pareto_efficiency_score` | `f64` | Multi-objective optimality | Distance to Pareto frontier |

**Total Features: 55** (15 + 10 + 12 + 8 + 10 + 10)

---

## 2. Feature Store Architecture

### 2.1 Dual-Store Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                     Feature Store Architecture                      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    Offline Feature Store                     │  │
│  │  (PostgreSQL - Historical Features, Training Data)           │  │
│  │                                                              │  │
│  │  • Complete feature history (90+ days retention)            │  │
│  │  • Time-travel queries for training data generation         │  │
│  │  • Batch feature computation jobs                           │  │
│  │  • Feature lineage and metadata                             │  │
│  │  • A/B test variant history                                 │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                            │                                        │
│                            │ Sync (every 5 min)                     │
│                            ▼                                        │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                     Online Feature Store                     │  │
│  │  (3-Tier Cache: L1 Memory → L2 Redis → L3 PostgreSQL)       │  │
│  │                                                              │  │
│  │  ┌────────────────────────────────────────────────────────┐ │  │
│  │  │ L1: In-Memory Cache (HashMap)                          │ │  │
│  │  │ • Hot features (most recent 1 hour)                    │ │  │
│  │  │ • 512 MB capacity                                      │ │  │
│  │  │ • LRU eviction                                         │ │  │
│  │  │ • Latency: <1ms (p99)                                  │ │  │
│  │  └────────────────────────────────────────────────────────┘ │  │
│  │                            │                                 │  │
│  │                            ▼ (cache miss)                    │  │
│  │  ┌────────────────────────────────────────────────────────┐ │  │
│  │  │ L2: Redis Cluster                                      │ │  │
│  │  │ • Warm features (last 24 hours)                        │ │  │
│  │  │ • 10 GB capacity per node (3 replicas)                 │ │  │
│  │  │ • Hash slot sharding by feature key                    │ │  │
│  │  │ • Latency: <5ms (p99)                                  │ │  │
│  │  │ • TTL: 86400s (24 hours)                               │ │  │
│  │  └────────────────────────────────────────────────────────┘ │  │
│  │                            │                                 │  │
│  │                            ▼ (cache miss)                    │  │
│  │  ┌────────────────────────────────────────────────────────┐ │  │
│  │  │ L3: PostgreSQL (Fallback)                              │ │  │
│  │  │ • All computed features (7 days retention)             │ │  │
│  │  │ • Indexed by entity_id + feature_group + timestamp     │ │  │
│  │  │ • Latency: <50ms (p99)                                 │ │  │
│  │  └────────────────────────────────────────────────────────┘ │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    Feature Registry                          │  │
│  │  (Metadata Store - PostgreSQL)                               │  │
│  │                                                              │  │
│  │  • Feature schemas and types                                │  │
│  │  • Feature lineage (dependencies)                           │  │
│  │  • Feature versions and changelog                           │  │
│  │  • Validation rules (range, nullability)                    │  │
│  │  • Ownership and documentation                              │  │
│  │  • SLA requirements (latency, freshness)                    │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 2.2 Storage Layer Design

#### 2.2.1 Redis Schema Design

**Hash-based storage for feature groups:**

```rust
// Key pattern: "feature:{entity_type}:{entity_id}:{feature_group}:{version}"
// Example: "feature:model:gpt-4:performance:v1"

// Value: Hash map of features
{
    "latency_p50_5min": "123.45",
    "latency_p95_5min": "234.56",
    "latency_p99_5min": "345.67",
    "success_rate_5min": "0.98",
    "error_rate_5min": "0.02",
    "updated_at": "1699574400",  // Unix timestamp
    "ttl": "300"  // TTL in seconds
}
```

**Feature groups for efficient access:**

- `request:{request_id}` - Request features
- `model:{model_id}:static` - Static model features
- `model:{model_id}:perf_5min` - 5-minute performance features
- `model:{model_id}:perf_1hr` - 1-hour performance features
- `user:{user_id}:session:{session_id}` - User session features
- `system:context` - System contextual features
- `derived:{request_id}` - Derived features

#### 2.2.2 PostgreSQL Schema

**Feature values table (online store):**

```sql
CREATE TABLE feature_values (
    id BIGSERIAL PRIMARY KEY,
    entity_type VARCHAR(50) NOT NULL,  -- 'model', 'user', 'request', 'system'
    entity_id VARCHAR(255) NOT NULL,   -- e.g., 'gpt-4', 'user123'
    feature_group VARCHAR(100) NOT NULL,  -- e.g., 'performance', 'feedback'
    feature_name VARCHAR(100) NOT NULL,
    feature_value DOUBLE PRECISION,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version INTEGER NOT NULL DEFAULT 1,
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for fast lookups
CREATE INDEX idx_feature_entity_lookup
    ON feature_values (entity_type, entity_id, feature_group, timestamp DESC);
CREATE INDEX idx_feature_name_lookup
    ON feature_values (feature_name, timestamp DESC);
CREATE INDEX idx_feature_timestamp
    ON feature_values (timestamp DESC);

-- Partition by month for efficient archival
CREATE TABLE feature_values_2025_11 PARTITION OF feature_values
    FOR VALUES FROM ('2025-11-01') TO ('2025-12-01');
```

**Feature registry table:**

```sql
CREATE TABLE feature_registry (
    id SERIAL PRIMARY KEY,
    feature_name VARCHAR(100) UNIQUE NOT NULL,
    feature_id VARCHAR(20) UNIQUE NOT NULL,  -- e.g., 'REQ_001'
    feature_type VARCHAR(20) NOT NULL,  -- 'f64', 'i64', 'bool', 'string'
    feature_category VARCHAR(50) NOT NULL,  -- 'request', 'model', 'performance', etc.
    description TEXT,
    computation_logic TEXT,
    data_source VARCHAR(100),  -- 'stream', 'batch', 'real-time'
    update_frequency VARCHAR(50),  -- '5min', '1hr', 'on_request'
    version INTEGER NOT NULL DEFAULT 1,
    validation_rules JSONB,  -- {min: 0.0, max: 1.0, nullable: false}
    sla_latency_ms INTEGER,  -- p99 latency SLA
    sla_freshness_seconds INTEGER,  -- max age for feature value
    owner VARCHAR(100),
    tags TEXT[],
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Sample row
INSERT INTO feature_registry (
    feature_name, feature_id, feature_type, feature_category,
    description, computation_logic, data_source,
    update_frequency, sla_latency_ms, sla_freshness_seconds
) VALUES (
    'latency_p95_5min', 'PERF_002', 'f64', 'performance',
    '95th percentile latency in last 5 minutes',
    'Percentile aggregation over 5-minute tumbling window',
    'stream', '5min', 5, 300
);
```

### 2.3 Cache Coherency Strategy

**Write-through caching:**

```rust
// On feature computation
async fn update_feature(
    entity_id: &str,
    feature_group: &str,
    features: HashMap<String, f64>,
) -> Result<()> {
    // 1. Write to PostgreSQL (source of truth)
    db.insert_features(entity_id, feature_group, &features).await?;

    // 2. Update L2 Redis cache
    redis.hset_multiple(
        &format!("feature:{}:{}", entity_id, feature_group),
        &features,
        TTL_24_HOURS
    ).await?;

    // 3. Invalidate L1 memory cache
    l1_cache.invalidate(entity_id, feature_group).await;

    Ok(())
}
```

**Read-through caching:**

```rust
async fn get_features(
    entity_id: &str,
    feature_group: &str,
    feature_names: &[&str],
) -> Result<HashMap<String, f64>> {
    // 1. Try L1 memory cache
    if let Some(features) = l1_cache.get(entity_id, feature_group).await {
        return Ok(filter_features(features, feature_names));
    }

    // 2. Try L2 Redis cache
    if let Some(features) = redis.hgetall(
        &format!("feature:{}:{}", entity_id, feature_group)
    ).await? {
        // Populate L1 cache
        l1_cache.set(entity_id, feature_group, features.clone()).await;
        return Ok(filter_features(features, feature_names));
    }

    // 3. Fallback to L3 PostgreSQL
    let features = db.get_features(entity_id, feature_group, feature_names).await?;

    // Populate caches
    redis.hset_multiple(
        &format!("feature:{}:{}", entity_id, feature_group),
        &features,
        TTL_24_HOURS
    ).await?;
    l1_cache.set(entity_id, feature_group, features.clone()).await;

    Ok(features)
}
```

---

## 3. Real-Time Feature Pipeline

### 3.1 Pipeline Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│                    Real-Time Feature Pipeline                        │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Event Stream (Kafka)                                                │
│  ┌────────┐  ┌────────┐  ┌────────┐  ┌────────┐                    │
│  │ Request│  │ Perf   │  │ User   │  │ System │                    │
│  │ Events │  │Metrics │  │Feedback│  │ Events │                    │
│  └───┬────┘  └───┬────┘  └───┬────┘  └───┬────┘                    │
│      │           │           │           │                          │
│      └───────────┴───────────┴───────────┘                          │
│                  │                                                   │
│                  ▼                                                   │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │            Stream Processor (Existing)                       │  │
│  │  • Event deserialization and validation                      │  │
│  │  • Watermarking and event-time assignment                    │  │
│  │  • Window assignment (tumbling, sliding, session)            │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                  │                                                   │
│                  ▼                                                   │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │         Feature Extractor (Stateless)                        │  │
│  │  ┌────────────────────────────────────────────────────────┐  │  │
│  │  │ Request Feature Extractor                              │  │  │
│  │  │ • Extract prompt_length, complexity, task_type         │  │  │
│  │  │ • Extract temporal features (hour, day, weekend)       │  │  │
│  │  │ • Compute prompt embeddings (cached)                   │  │  │
│  │  └────────────────────────────────────────────────────────┘  │  │
│  │  ┌────────────────────────────────────────────────────────┐  │  │
│  │  │ Model Feature Extractor                                │  │  │
│  │  │ • Lookup static model features from registry           │  │  │
│  │  │ • Enrich with capability scores                        │  │  │
│  │  └────────────────────────────────────────────────────────┘  │  │
│  │  ┌────────────────────────────────────────────────────────┐  │  │
│  │  │ Context Feature Extractor                              │  │  │
│  │  │ • Query system metrics (CPU, memory, queue)            │  │  │
│  │  │ • Get budget and quota status                          │  │  │
│  │  └────────────────────────────────────────────────────────┘  │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                  │                                                   │
│                  ▼                                                   │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │      Feature Aggregator (Stateful - uses Windows)           │  │
│  │  ┌────────────────────────────────────────────────────────┐  │  │
│  │  │ Performance Aggregator (5min tumbling)                 │  │  │
│  │  │ • Percentile(latency) → PERF_001, PERF_002, PERF_003   │  │  │
│  │  │ • Avg(cost_per_request) → PERF_009                     │  │  │
│  │  │ • Count(errors) / Count(total) → PERF_008              │  │  │
│  │  └────────────────────────────────────────────────────────┘  │  │
│  │  ┌────────────────────────────────────────────────────────┐  │  │
│  │  │ Feedback Aggregator (session window)                   │  │  │
│  │  │ • Avg(user_rating) → FDBK_001                          │  │  │
│  │  │ • Count(regenerations) / Count(total) → FDBK_004       │  │  │
│  │  └────────────────────────────────────────────────────────┘  │  │
│  │  ┌────────────────────────────────────────────────────────┐  │  │
│  │  │ System Load Aggregator (1min tumbling)                 │  │  │
│  │  │ • Avg(concurrent_requests) → CTX_004                   │  │  │
│  │  │ • Max(queue_depth) → CTX_003                           │  │  │
│  │  └────────────────────────────────────────────────────────┘  │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                  │                                                   │
│                  ▼                                                   │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │         Derived Feature Computation                          │  │
│  │  • Combine features (cost_latency_score)                     │  │
│  │  • Compute ratios (quality_cost_ratio)                       │  │
│  │  • Apply ML models (user_behavior_cluster)                   │  │
│  │  • Similarity computations (prompt_similarity_to_avg)        │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                  │                                                   │
│                  ▼                                                   │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │             Feature Validation & Normalization               │  │
│  │  • Range validation (min/max checks)                         │  │
│  │  • Null handling (imputation or default)                     │  │
│  │  • Outlier detection (3-sigma rule)                          │  │
│  │  • Normalization (min-max or z-score)                        │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                  │                                                   │
│                  ▼                                                   │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │              Feature Store Writer                            │  │
│  │  • Write to PostgreSQL (online + offline)                    │  │
│  │  • Update Redis cache (L2)                                   │  │
│  │  • Emit feature update events                                │  │
│  │  • Update monitoring metrics                                 │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

### 3.2 Feature Extraction Components

#### 3.2.1 Stateless Feature Extractor

```rust
use crate::types::FeedbackEvent;
use chrono::{Datelike, Timelike, Utc};
use std::collections::HashMap;

/// Stateless feature extraction from individual events
pub struct StatelessFeatureExtractor {
    /// Prompt embedding cache (LRU)
    embedding_cache: Arc<RwLock<LruCache<String, Vec<f64>>>>,
}

impl StatelessFeatureExtractor {
    /// Extract request features from event
    pub async fn extract_request_features(
        &self,
        event: &FeedbackEvent,
    ) -> HashMap<String, f64> {
        let mut features = HashMap::new();

        // Extract from event data
        if let Some(prompt) = event.data.get("prompt").and_then(|v| v.as_str()) {
            // REQ_001: prompt_length (log-normalized)
            let token_count = self.count_tokens(prompt);
            features.insert(
                "prompt_length".to_string(),
                ((token_count as f64 + 1.0).ln() / 10.0).min(1.0),
            );

            // REQ_002: prompt_char_count
            features.insert(
                "prompt_char_count".to_string(),
                (prompt.len() as f64 / 10000.0).min(1.0),
            );

            // REQ_003: prompt_complexity (Flesch-Kincaid)
            features.insert(
                "prompt_complexity".to_string(),
                self.compute_complexity(prompt),
            );

            // REQ_007: prompt_domain
            features.insert(
                "prompt_domain".to_string(),
                self.detect_domain(prompt),
            );
        }

        // REQ_004: expected_output_length (from metadata)
        let output_category = event.metadata
            .get("output_length_category")
            .map(|s| s.as_str())
            .unwrap_or("medium");
        features.insert(
            "expected_output_length".to_string(),
            match output_category {
                "short" => 0.0,
                "medium" => 0.5,
                "long" => 1.0,
                _ => 0.5,
            },
        );

        // REQ_005: task_type_classification
        let task_type = event.metadata
            .get("task_type")
            .map(|s| s.as_str())
            .unwrap_or("unknown");
        features.insert(
            "task_type_classification".to_string(),
            match task_type {
                "classification" => 0.0,
                "generation" => 0.33,
                "extraction" => 0.67,
                "summarization" => 1.0,
                _ => 0.5,
            },
        );

        // Temporal features
        let timestamp = event.timestamp;

        // REQ_009, REQ_010: hour_of_day cyclical encoding
        let hour = timestamp.hour() as f64;
        let hour_rad = (hour / 24.0) * 2.0 * std::f64::consts::PI;
        features.insert("hour_of_day_cos".to_string(), hour_rad.cos());
        features.insert("hour_of_day_sin".to_string(), hour_rad.sin());

        // REQ_011, REQ_012: day_of_week cyclical encoding
        let day = timestamp.weekday().num_days_from_monday() as f64;
        let day_rad = (day / 7.0) * 2.0 * std::f64::consts::PI;
        features.insert("day_of_week_cos".to_string(), day_rad.cos());
        features.insert("day_of_week_sin".to_string(), day_rad.sin());

        // REQ_013: is_weekend
        let is_weekend = matches!(timestamp.weekday().num_days_from_monday(), 5 | 6);
        features.insert("is_weekend".to_string(), if is_weekend { 1.0 } else { 0.0 });

        features
    }

    /// Count tokens (simplified - use proper tokenizer in production)
    fn count_tokens(&self, text: &str) -> usize {
        // Rough approximation: 1 token ≈ 4 characters
        (text.len() as f64 / 4.0).ceil() as usize
    }

    /// Compute text complexity (Flesch-Kincaid readability)
    fn compute_complexity(&self, text: &str) -> f64 {
        // Simplified complexity score based on avg word length and sentence length
        let words: Vec<&str> = text.split_whitespace().collect();
        let word_count = words.len() as f64;
        let sentence_count = text.matches('.').count().max(1) as f64;
        let syllable_count: usize = words.iter()
            .map(|w| self.count_syllables(w))
            .sum();

        // Flesch Reading Ease: 206.835 - 1.015(words/sentences) - 84.6(syllables/words)
        let score = 206.835
            - 1.015 * (word_count / sentence_count)
            - 84.6 * (syllable_count as f64 / word_count);

        // Normalize to 0-1 (easier = 0, harder = 1)
        ((100.0 - score) / 100.0).clamp(0.0, 1.0)
    }

    /// Simple syllable counter
    fn count_syllables(&self, word: &str) -> usize {
        let vowels = ['a', 'e', 'i', 'o', 'u', 'y'];
        let mut count = 0;
        let mut prev_was_vowel = false;

        for c in word.to_lowercase().chars() {
            let is_vowel = vowels.contains(&c);
            if is_vowel && !prev_was_vowel {
                count += 1;
            }
            prev_was_vowel = is_vowel;
        }

        count.max(1)
    }

    /// Detect domain-specific language
    fn detect_domain(&self, text: &str) -> f64 {
        let medical_terms = ["patient", "diagnosis", "treatment", "symptoms", "medical"];
        let legal_terms = ["contract", "plaintiff", "defendant", "statute", "liability"];
        let technical_terms = ["algorithm", "function", "database", "server", "API"];

        let text_lower = text.to_lowercase();

        let medical_count = medical_terms.iter()
            .filter(|&term| text_lower.contains(term))
            .count();
        let legal_count = legal_terms.iter()
            .filter(|&term| text_lower.contains(term))
            .count();
        let technical_count = technical_terms.iter()
            .filter(|&term| text_lower.contains(term))
            .count();

        // Return max count normalized
        let max_count = medical_count.max(legal_count).max(technical_count);
        (max_count as f64 / 5.0).min(1.0)
    }

    /// Extract model features (from static registry)
    pub async fn extract_model_features(
        &self,
        model_id: &str,
        model_registry: &ModelRegistry,
    ) -> HashMap<String, f64> {
        let mut features = HashMap::new();

        if let Some(model) = model_registry.get(model_id).await {
            // MDL_001: model_context_length
            features.insert(
                "model_context_length".to_string(),
                (model.context_length as f64).ln() / (200000_f64).ln(),
            );

            // MDL_002, MDL_003: cost features
            features.insert(
                "model_cost_per_1k_input".to_string(),
                model.cost_per_1k_input,
            );
            features.insert(
                "model_cost_per_1k_output".to_string(),
                model.cost_per_1k_output,
            );

            // MDL_004: capability score
            features.insert(
                "model_capability_score".to_string(),
                model.capability_score,
            );

            // MDL_005: training cutoff age
            let cutoff_age_days = (Utc::now() - model.training_cutoff_date).num_days();
            features.insert(
                "model_training_cutoff_age".to_string(),
                (cutoff_age_days as f64 / 365.0).min(2.0),
            );

            // MDL_006, MDL_007, MDL_008: capability flags
            features.insert(
                "model_supports_functions".to_string(),
                if model.supports_functions { 1.0 } else { 0.0 },
            );
            features.insert(
                "model_supports_vision".to_string(),
                if model.supports_vision { 1.0 } else { 0.0 },
            );
            features.insert(
                "model_supports_json_mode".to_string(),
                if model.supports_json_mode { 1.0 } else { 0.0 },
            );
        }

        features
    }
}
```

#### 3.2.2 Stateful Feature Aggregator (using existing windowing)

```rust
use crate::processor::aggregation::{Aggregator, PercentileAggregator, AvgAggregator};
use crate::processor::window::{Window, WindowBounds};

/// Stateful aggregator that computes features over windows
pub struct StatefulFeatureAggregator {
    /// Window-based aggregators (leverage existing stream processor)
    window_aggregators: HashMap<String, Box<dyn Aggregator>>,
}

impl StatefulFeatureAggregator {
    /// Create aggregators for performance features
    pub fn new_performance_aggregators() -> Self {
        let mut aggregators: HashMap<String, Box<dyn Aggregator>> = HashMap::new();

        // PERF_001: latency_p50_5min
        aggregators.insert(
            "latency_p50_5min".to_string(),
            Box::new(PercentileAggregator::new(50.0)),
        );

        // PERF_002: latency_p95_5min
        aggregators.insert(
            "latency_p95_5min".to_string(),
            Box::new(PercentileAggregator::new(95.0)),
        );

        // PERF_003: latency_p99_5min
        aggregators.insert(
            "latency_p99_5min".to_string(),
            Box::new(PercentileAggregator::new(99.0)),
        );

        // PERF_009: avg_cost_per_request_5min
        aggregators.insert(
            "avg_cost_per_request_5min".to_string(),
            Box::new(AvgAggregator::new()),
        );

        Self { window_aggregators: aggregators }
    }

    /// Update aggregators with new event
    pub fn update(&mut self, event: &PerformanceEvent) -> Result<()> {
        // Update latency aggregators
        if let Some(agg) = self.window_aggregators.get_mut("latency_p50_5min") {
            agg.update(event.latency_ms)?;
        }
        if let Some(agg) = self.window_aggregators.get_mut("latency_p95_5min") {
            agg.update(event.latency_ms)?;
        }
        if let Some(agg) = self.window_aggregators.get_mut("latency_p99_5min") {
            agg.update(event.latency_ms)?;
        }

        // Update cost aggregator
        if let Some(agg) = self.window_aggregators.get_mut("avg_cost_per_request_5min") {
            agg.update(event.cost)?;
        }

        Ok(())
    }

    /// Finalize window and extract features
    pub fn finalize_window(&self, window: &Window) -> HashMap<String, f64> {
        let mut features = HashMap::new();

        for (name, agg) in &self.window_aggregators {
            if let Ok(value) = agg.finalize() {
                features.insert(name.clone(), value);
            }
        }

        // Compute derived features
        // PERF_006: success_rate_5min
        // (computed separately from success/failure counts)

        features
    }
}
```

---

## 4. Integration with Stream Processor

### 4.1 Leveraging Existing Windowing Infrastructure

The feature pipeline integrates tightly with the existing stream processor's windowing system:

```rust
use crate::processor::window::WindowAssigner;
use crate::processor::watermark::BoundedOutOfOrderWatermark;
use chrono::Duration;

/// Feature pipeline integrated with stream processor
pub struct FeaturePipeline {
    /// Stateless feature extractor
    stateless_extractor: StatelessFeatureExtractor,

    /// Windowed aggregators (one per window type)
    performance_5min: WindowedAggregator<StatefulFeatureAggregator>,
    performance_1hr: WindowedAggregator<StatefulFeatureAggregator>,
    feedback_session: WindowedAggregator<FeedbackAggregator>,

    /// Watermark generator
    watermark: BoundedOutOfOrderWatermark,

    /// Feature store writer
    feature_store: Arc<FeatureStore>,
}

impl FeaturePipeline {
    pub async fn process_event(&mut self, event: FeedbackEvent) -> Result<()> {
        let event_time = event.timestamp;

        // 1. Update watermark
        if let Some(new_watermark) = self.watermark.on_event(
            event.partition(),
            event_time,
        ) {
            // Trigger window closures for all aggregators
            self.trigger_windows(new_watermark).await?;
        }

        // 2. Extract stateless features
        let request_features = self.stateless_extractor
            .extract_request_features(&event)
            .await;

        // 3. Write stateless features to store (immediate)
        self.feature_store.write_features(
            &event.id.to_string(),
            "request",
            request_features,
        ).await?;

        // 4. Route event to windowed aggregators
        match event.event_type {
            EventType::Metric => {
                // Performance metrics go to windowed aggregators
                self.performance_5min.add_event(event.clone()).await?;
                self.performance_1hr.add_event(event.clone()).await?;
            }
            EventType::Feedback => {
                // User feedback goes to session windows
                self.feedback_session.add_event(event.clone()).await?;
            }
            _ => {}
        }

        Ok(())
    }

    async fn trigger_windows(&mut self, watermark: DateTime<Utc>) -> Result<()> {
        // Trigger and finalize completed windows
        let completed_5min = self.performance_5min.close_windows(watermark).await?;
        let completed_1hr = self.performance_1hr.close_windows(watermark).await?;
        let completed_session = self.feedback_session.close_windows(watermark).await?;

        // Write aggregated features to store
        for (window, features) in completed_5min {
            self.feature_store.write_windowed_features(
                &window.id,
                "performance_5min",
                features,
                window.bounds.end,
            ).await?;
        }

        for (window, features) in completed_1hr {
            self.feature_store.write_windowed_features(
                &window.id,
                "performance_1hr",
                features,
                window.bounds.end,
            ).await?;
        }

        for (window, features) in completed_session {
            self.feature_store.write_windowed_features(
                &window.id,
                "feedback_session",
                features,
                window.bounds.end,
            ).await?;
        }

        Ok(())
    }
}

/// Windowed aggregator wrapper
struct WindowedAggregator<A> {
    /// Window assigner
    assigner: WindowAssigner,

    /// Active windows and their aggregators
    windows: HashMap<String, (Window, A)>,
}

impl<A> WindowedAggregator<A>
where
    A: Default + Send + Sync,
{
    async fn add_event(&mut self, event: FeedbackEvent) -> Result<()> {
        // Assign event to windows
        let windows = self.assigner.assign_windows(event.timestamp);

        for window in windows {
            // Get or create aggregator for this window
            let (_, aggregator) = self.windows
                .entry(window.id.clone())
                .or_insert_with(|| (window.clone(), A::default()));

            // Update aggregator
            // (specific update logic depends on aggregator type)
        }

        Ok(())
    }

    async fn close_windows(
        &mut self,
        watermark: DateTime<Utc>,
    ) -> Result<Vec<(Window, HashMap<String, f64>)>> {
        let mut completed = Vec::new();

        // Find windows that should close
        let to_close: Vec<_> = self.windows
            .iter()
            .filter(|(_, (window, _))| window.bounds.end <= watermark)
            .map(|(id, _)| id.clone())
            .collect();

        // Finalize and remove closed windows
        for window_id in to_close {
            if let Some((window, aggregator)) = self.windows.remove(&window_id) {
                // Finalize features (specific to aggregator)
                let features = HashMap::new(); // aggregator.finalize();
                completed.push((window, features));
            }
        }

        Ok(completed)
    }
}
```

### 4.2 Window Configuration for Features

```yaml
# Feature-specific window configuration
feature_windows:
  performance_5min:
    window_type: tumbling
    size: 300s  # 5 minutes
    allowed_lateness: 60s
    features:
      - latency_p50_5min
      - latency_p95_5min
      - latency_p99_5min
      - success_rate_5min
      - error_rate_5min
      - avg_cost_per_request_5min
      - throughput_qps_5min
      - token_efficiency_5min
      - retry_rate_5min

  performance_1hr:
    window_type: sliding
    size: 3600s  # 1 hour
    slide: 300s  # slide every 5 minutes
    allowed_lateness: 120s
    features:
      - latency_p50_1hr
      - latency_p95_1hr
      - success_rate_1hr

  feedback_session:
    window_type: session
    gap: 1800s  # 30 minute inactivity gap
    max_duration: 14400s  # 4 hour max session
    features:
      - avg_user_rating_session
      - task_completion_rate_session
      - avg_response_time_user
      - follow_up_rate_session

  feedback_1hr:
    window_type: sliding
    size: 3600s
    slide: 300s
    features:
      - avg_user_rating_1hr
      - thumbs_up_ratio_1hr
      - regeneration_rate_1hr
      - copy_paste_rate_1hr
```

### 4.3 State Backend for Feature Aggregation

Reuse existing state backends with feature-specific configuration:

```rust
use crate::processor::state::{StateBackend, CachedStateBackend, RedisStateBackend};

/// Feature aggregation state backend
pub struct FeatureStateBackend {
    /// Underlying state backend (Redis with cache)
    backend: CachedStateBackend<RedisStateBackend>,
}

impl FeatureStateBackend {
    pub async fn new(config: RedisConfig) -> Result<Self> {
        let redis_backend = RedisStateBackend::new(config).await?;
        let cached_backend = CachedStateBackend::new(
            redis_backend,
            512 * 1024 * 1024,  // 512 MB L1 cache
            Duration::hours(1),  // 1 hour TTL
        );

        Ok(Self {
            backend: cached_backend,
        })
    }

    /// Store aggregator state for window
    pub async fn save_window_state<A>(
        &self,
        window_id: &str,
        aggregator: &A,
    ) -> Result<()>
    where
        A: Aggregator,
    {
        let state_key = format!("feature:window:{}", window_id);
        let accumulator = aggregator.accumulator();
        let serialized = bincode::serialize(&accumulator)?;

        self.backend.put(
            state_key.as_bytes(),
            &serialized,
        ).await?;

        Ok(())
    }

    /// Restore aggregator state for window
    pub async fn restore_window_state<A>(
        &self,
        window_id: &str,
    ) -> Result<Option<A::Accumulator>>
    where
        A: Aggregator,
    {
        let state_key = format!("feature:window:{}", window_id);

        if let Some(data) = self.backend.get(state_key.as_bytes()).await? {
            let accumulator = bincode::deserialize(&data)?;
            Ok(Some(accumulator))
        } else {
            Ok(None)
        }
    }
}
```

---

## 5. Feature Serving API

### 5.1 Serving Architecture

```rust
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::time::Instant;

/// Feature serving API
pub struct FeatureServingAPI {
    feature_store: Arc<FeatureStore>,
    metrics: Arc<ServingMetrics>,
}

/// Feature request
#[derive(Debug, Deserialize)]
pub struct FeatureRequest {
    /// Entity ID (e.g., model_id, user_id, request_id)
    pub entity_id: String,

    /// Entity type
    pub entity_type: String,

    /// List of feature names to retrieve
    pub features: Vec<String>,

    /// Optional: feature version
    pub version: Option<i32>,

    /// Optional: point-in-time timestamp for historical features
    pub timestamp: Option<DateTime<Utc>>,
}

/// Feature response
#[derive(Debug, Serialize)]
pub struct FeatureResponse {
    /// Entity ID
    pub entity_id: String,

    /// Feature values
    pub features: HashMap<String, f64>,

    /// Feature metadata
    pub metadata: FeatureMetadata,

    /// Serving latency in milliseconds
    pub latency_ms: f64,
}

#[derive(Debug, Serialize)]
pub struct FeatureMetadata {
    /// Timestamp when features were computed
    pub computed_at: DateTime<Utc>,

    /// Feature version
    pub version: i32,

    /// Cache hit/miss indicator
    pub cache_hit: bool,

    /// Data freshness (age in seconds)
    pub freshness_seconds: i64,
}

impl FeatureServingAPI {
    /// Serve features with low latency
    pub async fn serve_features(
        &self,
        request: FeatureRequest,
    ) -> Result<FeatureResponse, FeatureError> {
        let start = Instant::now();

        // Validate request
        self.validate_request(&request)?;

        // Determine feature groups to fetch
        let feature_groups = self.get_feature_groups(&request.features)?;

        // Fetch features (leverages 3-tier cache)
        let mut all_features = HashMap::new();
        let mut cache_hit = true;
        let mut computed_at = Utc::now();

        for group in feature_groups {
            let group_features = self.feature_store.get_features(
                &request.entity_id,
                &group,
                &request.features,
            ).await?;

            // Merge features
            all_features.extend(group_features.features);

            // Track cache hits
            cache_hit = cache_hit && group_features.cache_hit;

            // Use oldest timestamp
            if group_features.computed_at < computed_at {
                computed_at = group_features.computed_at;
            }
        }

        // Handle missing features
        let missing = self.handle_missing_features(
            &request.features,
            &all_features,
        )?;
        all_features.extend(missing);

        // Compute derived features if needed
        if request.features.iter().any(|f| f.starts_with("DRV_")) {
            let derived = self.compute_derived_features(&all_features).await?;
            all_features.extend(derived);
        }

        // Filter to requested features only
        let filtered_features: HashMap<_, _> = all_features
            .into_iter()
            .filter(|(k, _)| request.features.contains(k))
            .collect();

        let latency_ms = start.elapsed().as_secs_f64() * 1000.0;

        // Record metrics
        self.metrics.record_serving_latency(latency_ms);
        self.metrics.record_cache_hit(cache_hit);

        // Validate SLA
        if latency_ms > 5.0 {
            tracing::warn!(
                "Feature serving exceeded SLA: {:.2}ms > 5ms",
                latency_ms
            );
        }

        Ok(FeatureResponse {
            entity_id: request.entity_id,
            features: filtered_features,
            metadata: FeatureMetadata {
                computed_at,
                version: request.version.unwrap_or(1),
                cache_hit,
                freshness_seconds: (Utc::now() - computed_at).num_seconds(),
            },
            latency_ms,
        })
    }

    /// Validate feature request
    fn validate_request(&self, request: &FeatureRequest) -> Result<(), FeatureError> {
        if request.entity_id.is_empty() {
            return Err(FeatureError::InvalidRequest("entity_id is required".into()));
        }

        if request.features.is_empty() {
            return Err(FeatureError::InvalidRequest("features list is empty".into()));
        }

        if request.features.len() > 100 {
            return Err(FeatureError::InvalidRequest(
                "too many features requested (max 100)".into()
            ));
        }

        Ok(())
    }

    /// Get feature groups for requested features
    fn get_feature_groups(&self, features: &[String]) -> Result<Vec<String>, FeatureError> {
        let mut groups = std::collections::HashSet::new();

        for feature in features {
            // Map feature to group based on prefix
            let group = if feature.starts_with("REQ_") {
                "request"
            } else if feature.starts_with("MDL_") {
                "model:static"
            } else if feature.starts_with("PERF_") && feature.ends_with("5min") {
                "model:perf_5min"
            } else if feature.starts_with("PERF_") && feature.ends_with("1hr") {
                "model:perf_1hr"
            } else if feature.starts_with("FDBK_") && feature.contains("session") {
                "user:session"
            } else if feature.starts_with("FDBK_") && feature.ends_with("1hr") {
                "user:feedback_1hr"
            } else if feature.starts_with("CTX_") {
                "system:context"
            } else if feature.starts_with("DRV_") {
                "derived"
            } else {
                return Err(FeatureError::UnknownFeature(feature.clone()));
            };

            groups.insert(group.to_string());
        }

        Ok(groups.into_iter().collect())
    }

    /// Handle missing features (imputation or error)
    fn handle_missing_features(
        &self,
        requested: &[String],
        available: &HashMap<String, f64>,
    ) -> Result<HashMap<String, f64>, FeatureError> {
        let mut imputed = HashMap::new();

        for feature in requested {
            if !available.contains_key(feature) {
                // Get default value from registry
                if let Some(default) = self.get_default_value(feature)? {
                    imputed.insert(feature.clone(), default);
                    tracing::warn!("Feature {} not found, using default: {}", feature, default);
                } else {
                    return Err(FeatureError::FeatureNotFound(feature.clone()));
                }
            }
        }

        Ok(imputed)
    }

    /// Get default value for feature from registry
    fn get_default_value(&self, feature: &str) -> Result<Option<f64>, FeatureError> {
        // Look up in feature registry
        // Return None if no default, Some(value) if default exists
        Ok(Some(0.0))  // Simplified
    }

    /// Compute derived features
    async fn compute_derived_features(
        &self,
        base_features: &HashMap<String, f64>,
    ) -> Result<HashMap<String, f64>, FeatureError> {
        let mut derived = HashMap::new();

        // DRV_002: cost_latency_score
        if let (Some(cost), Some(latency)) = (
            base_features.get("avg_cost_per_request_5min"),
            base_features.get("latency_p95_5min"),
        ) {
            let cost_norm = (cost / 0.01).min(1.0);  // Normalize assuming $0.01 max
            let latency_norm = (latency / 10000.0).min(1.0);  // Normalize assuming 10s max
            derived.insert(
                "cost_latency_score".to_string(),
                (cost_norm + latency_norm) / 2.0,
            );
        }

        // DRV_003: quality_cost_ratio
        if let (Some(success_rate), Some(cost)) = (
            base_features.get("success_rate_5min"),
            base_features.get("avg_cost_per_request_5min"),
        ) {
            if *cost > 0.0 {
                derived.insert(
                    "quality_cost_ratio".to_string(),
                    success_rate / cost,
                );
            }
        }

        Ok(derived)
    }
}

/// REST API routes
pub fn create_router(api: Arc<FeatureServingAPI>) -> Router {
    Router::new()
        .route("/features/:entity_type/:entity_id", axum::routing::get(get_features))
        .route("/features/batch", axum::routing::post(batch_get_features))
        .with_state(api)
}

/// GET /features/:entity_type/:entity_id?features=REQ_001,REQ_002
async fn get_features(
    State(api): State<Arc<FeatureServingAPI>>,
    Path((entity_type, entity_id)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<FeatureResponse>, (StatusCode, String)> {
    let features: Vec<String> = params
        .get("features")
        .ok_or((StatusCode::BAD_REQUEST, "features parameter required".to_string()))?
        .split(',')
        .map(|s| s.to_string())
        .collect();

    let request = FeatureRequest {
        entity_id,
        entity_type,
        features,
        version: None,
        timestamp: None,
    };

    api.serve_features(request)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

/// POST /features/batch
async fn batch_get_features(
    State(api): State<Arc<FeatureServingAPI>>,
    Json(requests): Json<Vec<FeatureRequest>>,
) -> Result<Json<Vec<FeatureResponse>>, (StatusCode, String)> {
    let mut responses = Vec::new();

    for request in requests {
        match api.serve_features(request).await {
            Ok(response) => responses.push(response),
            Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
        }
    }

    Ok(Json(responses))
}
```

### 5.2 Feature Serving Metrics

```rust
use prometheus::{HistogramVec, IntCounterVec, Registry};

pub struct ServingMetrics {
    /// Serving latency histogram
    serving_latency: HistogramVec,

    /// Cache hit counter
    cache_hits: IntCounterVec,

    /// Feature request counter
    requests: IntCounterVec,

    /// Feature errors counter
    errors: IntCounterVec,
}

impl ServingMetrics {
    pub fn new(registry: &Registry) -> Self {
        let serving_latency = HistogramVec::new(
            histogram_opts!(
                "feature_serving_latency_ms",
                "Feature serving latency in milliseconds",
                vec![0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0]
            ),
            &["entity_type", "cache_tier"]
        ).unwrap();

        let cache_hits = IntCounterVec::new(
            opts!(
                "feature_cache_hits_total",
                "Total number of feature cache hits/misses"
            ),
            &["tier", "hit"]
        ).unwrap();

        let requests = IntCounterVec::new(
            opts!(
                "feature_requests_total",
                "Total number of feature requests"
            ),
            &["entity_type", "status"]
        ).unwrap();

        let errors = IntCounterVec::new(
            opts!(
                "feature_errors_total",
                "Total number of feature serving errors"
            ),
            &["error_type"]
        ).unwrap();

        registry.register(Box::new(serving_latency.clone())).unwrap();
        registry.register(Box::new(cache_hits.clone())).unwrap();
        registry.register(Box::new(requests.clone())).unwrap();
        registry.register(Box::new(errors.clone())).unwrap();

        Self {
            serving_latency,
            cache_hits,
            requests,
            errors,
        }
    }

    pub fn record_serving_latency(&self, latency_ms: f64) {
        self.serving_latency
            .with_label_values(&["unknown", "multi_tier"])
            .observe(latency_ms);
    }

    pub fn record_cache_hit(&self, hit: bool) {
        self.cache_hits
            .with_label_values(&["multi_tier", if hit { "hit" } else { "miss" }])
            .inc();
    }
}
```

---

## 6. Feature Versioning & Validation

### 6.1 Feature Versioning Strategy

```rust
/// Feature version management
pub struct FeatureVersion {
    /// Version number
    pub version: i32,

    /// Schema version (for breaking changes)
    pub schema_version: i32,

    /// Created timestamp
    pub created_at: DateTime<Utc>,

    /// Is active (for gradual rollouts)
    pub is_active: bool,

    /// Traffic percentage (0-100)
    pub traffic_percentage: f64,

    /// Changelog
    pub changelog: String,
}

impl FeatureVersion {
    /// Determine which version to serve based on experiment assignment
    pub fn select_version(
        &self,
        user_id: &str,
        experiment_config: &ExperimentConfig,
    ) -> i32 {
        // Hash user_id to determine assignment
        let hash = self.hash_user(user_id);
        let assignment = (hash % 100) as f64;

        if assignment < self.traffic_percentage {
            self.version
        } else {
            experiment_config.control_version
        }
    }

    fn hash_user(&self, user_id: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        user_id.hash(&mut hasher);
        hasher.finish()
    }
}
```

### 6.2 Feature Validation

```rust
use anyhow::Result;

/// Feature validator
pub struct FeatureValidator {
    /// Validation rules from registry
    rules: HashMap<String, ValidationRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRule {
    /// Feature name
    pub feature_name: String,

    /// Min value (inclusive)
    pub min_value: Option<f64>,

    /// Max value (inclusive)
    pub max_value: Option<f64>,

    /// Is nullable
    pub nullable: bool,

    /// Expected distribution (for drift detection)
    pub expected_mean: Option<f64>,
    pub expected_std: Option<f64>,

    /// Custom validation function
    pub custom_validation: Option<String>,
}

impl FeatureValidator {
    /// Validate feature value
    pub fn validate(
        &self,
        feature_name: &str,
        value: Option<f64>,
    ) -> Result<f64, ValidationError> {
        let rule = self.rules.get(feature_name)
            .ok_or_else(|| ValidationError::NoRule(feature_name.to_string()))?;

        // Check nullability
        let value = match value {
            Some(v) => v,
            None => {
                if rule.nullable {
                    return Ok(0.0);  // Default value
                } else {
                    return Err(ValidationError::NullValue(feature_name.to_string()));
                }
            }
        };

        // Check range
        if let Some(min) = rule.min_value {
            if value < min {
                return Err(ValidationError::BelowMin {
                    feature: feature_name.to_string(),
                    value,
                    min,
                });
            }
        }

        if let Some(max) = rule.max_value {
            if value > max {
                return Err(ValidationError::AboveMax {
                    feature: feature_name.to_string(),
                    value,
                    max,
                });
            }
        }

        // Check for NaN or infinite
        if !value.is_finite() {
            return Err(ValidationError::InvalidValue {
                feature: feature_name.to_string(),
                value,
                reason: "value is NaN or infinite".to_string(),
            });
        }

        Ok(value)
    }

    /// Validate entire feature vector
    pub fn validate_vector(
        &self,
        features: &HashMap<String, f64>,
    ) -> Result<HashMap<String, f64>, ValidationError> {
        let mut validated = HashMap::new();

        for (name, value) in features {
            let validated_value = self.validate(name, Some(*value))?;
            validated.insert(name.clone(), validated_value);
        }

        Ok(validated)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("No validation rule found for feature: {0}")]
    NoRule(String),

    #[error("Null value for non-nullable feature: {0}")]
    NullValue(String),

    #[error("Feature {feature} value {value} below minimum {min}")]
    BelowMin { feature: String, value: f64, min: f64 },

    #[error("Feature {feature} value {value} above maximum {max}")]
    AboveMax { feature: String, value: f64, max: f64 },

    #[error("Invalid value for feature {feature}: {value} ({reason})")]
    InvalidValue { feature: String, value: f64, reason: String },
}
```

---

## 7. Monitoring & Drift Detection

### 7.1 Feature Distribution Monitoring

```rust
use statrs::statistics::{Data, Distribution, Statistics};

/// Feature drift detector
pub struct FeatureDriftDetector {
    /// Historical statistics per feature
    baselines: HashMap<String, FeatureBaseline>,

    /// Current window statistics
    current_stats: HashMap<String, WindowedStats>,

    /// Drift detection config
    config: DriftConfig,
}

#[derive(Debug, Clone)]
pub struct FeatureBaseline {
    /// Feature name
    pub feature_name: String,

    /// Expected mean
    pub mean: f64,

    /// Expected standard deviation
    pub std_dev: f64,

    /// Expected min/max
    pub min: f64,
    pub max: f64,

    /// Percentiles
    pub p25: f64,
    pub p50: f64,
    pub p75: f64,
    pub p95: f64,
    pub p99: f64,

    /// Computed from N samples
    pub sample_count: usize,

    /// Last updated
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct WindowedStats {
    /// Recent values (rolling window)
    values: Vec<f64>,

    /// Window size
    window_size: usize,

    /// Current statistics
    mean: f64,
    std_dev: f64,
}

impl WindowedStats {
    pub fn new(window_size: usize) -> Self {
        Self {
            values: Vec::with_capacity(window_size),
            window_size,
            mean: 0.0,
            std_dev: 0.0,
        }
    }

    pub fn add_value(&mut self, value: f64) {
        if self.values.len() >= self.window_size {
            self.values.remove(0);
        }
        self.values.push(value);

        // Recompute stats
        if !self.values.is_empty() {
            let data = Data::new(self.values.clone());
            self.mean = data.mean().unwrap_or(0.0);
            self.std_dev = data.std_dev().unwrap_or(0.0);
        }
    }
}

#[derive(Debug, Clone)]
pub struct DriftConfig {
    /// Z-score threshold for drift detection
    pub zscore_threshold: f64,

    /// PSI (Population Stability Index) threshold
    pub psi_threshold: f64,

    /// Minimum samples before drift detection
    pub min_samples: usize,

    /// Window size for current statistics
    pub window_size: usize,
}

impl Default for DriftConfig {
    fn default() -> Self {
        Self {
            zscore_threshold: 3.0,  // 3-sigma rule
            psi_threshold: 0.1,     // PSI > 0.1 indicates drift
            min_samples: 1000,
            window_size: 10000,
        }
    }
}

impl FeatureDriftDetector {
    /// Detect drift for a feature
    pub fn detect_drift(
        &self,
        feature_name: &str,
    ) -> Option<DriftAlert> {
        let baseline = self.baselines.get(feature_name)?;
        let current = self.current_stats.get(feature_name)?;

        // Need minimum samples
        if current.values.len() < self.config.min_samples {
            return None;
        }

        // Compute z-score for mean shift
        let mean_diff = (current.mean - baseline.mean).abs();
        let z_score = mean_diff / baseline.std_dev;

        if z_score > self.config.zscore_threshold {
            return Some(DriftAlert {
                feature_name: feature_name.to_string(),
                drift_type: DriftType::MeanShift,
                severity: if z_score > 5.0 { DriftSeverity::Critical }
                         else if z_score > 4.0 { DriftSeverity::High }
                         else { DriftSeverity::Medium },
                baseline_mean: baseline.mean,
                current_mean: current.mean,
                z_score,
                detected_at: Utc::now(),
            });
        }

        // Compute PSI (Population Stability Index)
        let psi = self.compute_psi(baseline, current);

        if psi > self.config.psi_threshold {
            return Some(DriftAlert {
                feature_name: feature_name.to_string(),
                drift_type: DriftType::DistributionShift,
                severity: if psi > 0.25 { DriftSeverity::High }
                         else { DriftSeverity::Medium },
                baseline_mean: baseline.mean,
                current_mean: current.mean,
                z_score: psi,  // Use PSI as score
                detected_at: Utc::now(),
            });
        }

        None
    }

    /// Compute Population Stability Index
    fn compute_psi(
        &self,
        baseline: &FeatureBaseline,
        current: &WindowedStats,
    ) -> f64 {
        // Simplified PSI calculation
        // PSI = Σ (actual% - expected%) × ln(actual% / expected%)

        // Create 10 bins
        let num_bins = 10;
        let bin_size = (baseline.max - baseline.min) / num_bins as f64;

        let mut psi = 0.0;

        for i in 0..num_bins {
            let bin_start = baseline.min + i as f64 * bin_size;
            let bin_end = bin_start + bin_size;

            // Count values in bin for baseline (use percentiles as approximation)
            let expected_pct = 1.0 / num_bins as f64;

            // Count values in bin for current
            let actual_count = current.values.iter()
                .filter(|&&v| v >= bin_start && v < bin_end)
                .count();
            let actual_pct = actual_count as f64 / current.values.len() as f64;

            if actual_pct > 0.0 && expected_pct > 0.0 {
                psi += (actual_pct - expected_pct) * (actual_pct / expected_pct).ln();
            }
        }

        psi
    }
}

#[derive(Debug, Clone)]
pub struct DriftAlert {
    pub feature_name: String,
    pub drift_type: DriftType,
    pub severity: DriftSeverity,
    pub baseline_mean: f64,
    pub current_mean: f64,
    pub z_score: f64,
    pub detected_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftType {
    MeanShift,
    VarianceShift,
    DistributionShift,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DriftSeverity {
    Low,
    Medium,
    High,
    Critical,
}
```

### 7.2 Feature Quality Metrics

```rust
/// Feature quality metrics
pub struct FeatureQualityMetrics {
    /// Null rate (% of null values)
    pub null_rate: f64,

    /// Freshness (age of feature value in seconds)
    pub freshness_seconds: i64,

    /// Completeness (% of expected features present)
    pub completeness: f64,

    /// Consistency (% of values passing validation)
    pub consistency: f64,

    /// Drift score (0-1, higher = more drift)
    pub drift_score: f64,
}

impl FeatureQualityMetrics {
    /// Compute overall quality score (0-1)
    pub fn overall_score(&self) -> f64 {
        let weights = [0.2, 0.3, 0.2, 0.2, 0.1];  // [null, fresh, complete, consistent, drift]

        let scores = [
            1.0 - self.null_rate,
            self.freshness_score(),
            self.completeness,
            self.consistency,
            1.0 - self.drift_score,
        ];

        scores.iter()
            .zip(weights.iter())
            .map(|(score, weight)| score * weight)
            .sum()
    }

    fn freshness_score(&self) -> f64 {
        // Exponential decay: score = e^(-age/threshold)
        let threshold = 300.0;  // 5 minutes
        (-self.freshness_seconds as f64 / threshold).exp()
    }
}
```

---

## 8. Rust Implementation Design

### 8.1 Module Structure

```
crates/features/
├── src/
│   ├── lib.rs                    # Public API
│   ├── catalog.rs                # Feature catalog and metadata
│   ├── extraction/
│   │   ├── mod.rs
│   │   ├── stateless.rs          # Stateless feature extractors
│   │   ├── stateful.rs           # Stateful aggregators
│   │   └── derived.rs            # Derived feature computation
│   ├── store/
│   │   ├── mod.rs
│   │   ├── online.rs             # Online feature store (3-tier cache)
│   │   ├── offline.rs            # Offline feature store (PostgreSQL)
│   │   └── registry.rs           # Feature metadata registry
│   ├── serving/
│   │   ├── mod.rs
│   │   ├── api.rs                # Feature serving REST API
│   │   ├── batch.rs              # Batch feature serving
│   │   └── metrics.rs            # Serving metrics
│   ├── validation/
│   │   ├── mod.rs
│   │   ├── validator.rs          # Feature validation
│   │   └── drift.rs              # Drift detection
│   ├── versioning/
│   │   ├── mod.rs
│   │   └── version_manager.rs    # Feature versioning
│   ├── pipeline/
│   │   ├── mod.rs
│   │   ├── feature_pipeline.rs   # Main pipeline orchestration
│   │   └── window_integration.rs # Integration with stream processor
│   └── error.rs                  # Error types
├── tests/
│   ├── integration/
│   │   ├── pipeline_test.rs
│   │   ├── serving_test.rs
│   │   └── drift_test.rs
│   └── benchmarks/
│       ├── serving_benchmark.rs
│       └── extraction_benchmark.rs
└── Cargo.toml
```

### 8.2 Core Traits

```rust
/// Feature extractor trait
#[async_trait]
pub trait FeatureExtractor: Send + Sync {
    /// Feature names produced by this extractor
    fn feature_names(&self) -> &[String];

    /// Extract features from event
    async fn extract(&self, event: &FeedbackEvent) -> Result<HashMap<String, f64>>;

    /// Is this extractor stateless?
    fn is_stateless(&self) -> bool {
        true
    }
}

/// Feature store trait
#[async_trait]
pub trait FeatureStore: Send + Sync {
    /// Write features to store
    async fn write_features(
        &self,
        entity_id: &str,
        feature_group: &str,
        features: HashMap<String, f64>,
    ) -> Result<()>;

    /// Read features from store
    async fn read_features(
        &self,
        entity_id: &str,
        feature_group: &str,
        feature_names: &[String],
    ) -> Result<HashMap<String, f64>>;

    /// Read features at point-in-time (for training data)
    async fn read_features_at(
        &self,
        entity_id: &str,
        feature_group: &str,
        feature_names: &[String],
        timestamp: DateTime<Utc>,
    ) -> Result<HashMap<String, f64>>;

    /// Get feature metadata
    async fn get_feature_metadata(
        &self,
        feature_name: &str,
    ) -> Result<FeatureMetadata>;
}
```

### 8.3 Performance Optimizations

```rust
use tokio::task::JoinSet;

/// Parallel feature extraction
pub async fn extract_features_parallel(
    extractors: &[Arc<dyn FeatureExtractor>],
    event: &FeedbackEvent,
) -> Result<HashMap<String, f64>> {
    let mut join_set = JoinSet::new();

    // Spawn extraction tasks in parallel
    for extractor in extractors {
        let extractor = Arc::clone(extractor);
        let event = event.clone();

        join_set.spawn(async move {
            extractor.extract(&event).await
        });
    }

    // Collect results
    let mut all_features = HashMap::new();

    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Ok(features)) => all_features.extend(features),
            Ok(Err(e)) => tracing::error!("Feature extraction error: {}", e),
            Err(e) => tracing::error!("Join error: {}", e),
        }
    }

    Ok(all_features)
}

/// Feature batch prefetching
pub async fn prefetch_features(
    store: &Arc<dyn FeatureStore>,
    entity_ids: &[String],
    feature_groups: &[String],
) -> Result<()> {
    let mut join_set = JoinSet::new();

    for entity_id in entity_ids {
        for feature_group in feature_groups {
            let store = Arc::clone(store);
            let entity_id = entity_id.clone();
            let feature_group = feature_group.clone();

            join_set.spawn(async move {
                store.read_features(&entity_id, &feature_group, &[]).await
            });
        }
    }

    // Wait for all prefetch tasks
    while let Some(_) = join_set.join_next().await {}

    Ok(())
}
```

---

## 9. Testing Strategy

### 9.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stateless_extraction() {
        let extractor = StatelessFeatureExtractor::new();

        let event = FeedbackEvent {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            source: EventSource::User,
            event_type: EventType::Metric,
            data: serde_json::json!({
                "prompt": "Summarize this article in 100 words.",
            }),
            metadata: HashMap::new(),
        };

        let features = extractor.extract_request_features(&event).await.unwrap();

        // Verify extracted features
        assert!(features.contains_key("prompt_length"));
        assert!(features.contains_key("hour_of_day_cos"));

        // Verify ranges
        assert!(features["prompt_length"] >= 0.0 && features["prompt_length"] <= 1.0);
    }

    #[tokio::test]
    async fn test_feature_validation() {
        let validator = FeatureValidator::new();

        // Valid value
        assert!(validator.validate("latency_p95_5min", Some(123.45)).is_ok());

        // Null for non-nullable
        assert!(validator.validate("latency_p95_5min", None).is_err());

        // Out of range
        assert!(validator.validate("success_rate_5min", Some(1.5)).is_err());
    }
}
```

### 9.2 Integration Tests

```rust
#[tokio::test]
async fn test_end_to_end_feature_pipeline() {
    // Setup
    let config = FeatureStoreConfig::default();
    let feature_store = Arc::new(FeatureStore::new(config).await.unwrap());
    let pipeline = FeaturePipeline::new(feature_store.clone()).await.unwrap();

    // Generate test events
    let events = generate_test_events(100);

    // Process events
    for event in events {
        pipeline.process_event(event).await.unwrap();
    }

    // Wait for windows to close
    tokio::time::sleep(Duration::from_secs(6)).await;

    // Verify features were computed and stored
    let features = feature_store.read_features(
        "gpt-4",
        "performance_5min",
        &["latency_p95_5min", "success_rate_5min"],
    ).await.unwrap();

    assert_eq!(features.len(), 2);
    assert!(features["latency_p95_5min"] > 0.0);
}
```

### 9.3 Property-Based Tests

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_feature_normalization(
        value in 0.0..10000.0_f64,
    ) {
        let normalized = normalize_feature(value, 0.0, 10000.0);

        // Property: normalized value must be in [0, 1]
        prop_assert!(normalized >= 0.0 && normalized <= 1.0);
    }

    #[test]
    fn test_cyclical_encoding_invertible(
        hour in 0_u32..24,
    ) {
        let hour_rad = (hour as f64 / 24.0) * 2.0 * std::f64::consts::PI;
        let cos_val = hour_rad.cos();
        let sin_val = hour_rad.sin();

        // Property: can reconstruct hour from (cos, sin)
        let reconstructed = hour_rad.atan2(sin_val).to_degrees();
        let reconstructed_hour = (reconstructed / 15.0).round() as u32 % 24;

        prop_assert_eq!(reconstructed_hour, hour);
    }
}
```

---

## 10. Performance Requirements

### 10.1 Latency SLAs

| Operation | p50 | p95 | p99 | p99.9 |
|-----------|-----|-----|-----|-------|
| **Feature Serving (single entity)** | <1ms | <2ms | <5ms | <10ms |
| **Feature Serving (batch 100)** | <10ms | <20ms | <50ms | <100ms |
| **Stateless Extraction** | <0.5ms | <1ms | <2ms | <5ms |
| **Stateful Aggregation (per event)** | <0.1ms | <0.5ms | <1ms | <2ms |
| **Feature Store Write** | <2ms | <5ms | <10ms | <20ms |
| **Feature Validation** | <0.1ms | <0.2ms | <0.5ms | <1ms |
| **Drift Detection (per feature)** | <1ms | <2ms | <5ms | <10ms |

### 10.2 Throughput Requirements

- **Event Ingestion**: 10,000 events/second
- **Feature Serving**: 50,000 requests/second
- **Batch Feature Generation**: 1M features/minute
- **Window Closure**: 1000 windows/second

### 10.3 Resource Limits

- **Memory (per instance)**:
  - L1 Cache: 512 MB
  - Working memory: 1 GB
  - Total: <2 GB

- **CPU**:
  - Feature extraction: <10% per core
  - Aggregation: <20% per core
  - Serving: <30% per core

- **Network**:
  - Redis bandwidth: <100 MB/s
  - PostgreSQL bandwidth: <50 MB/s

### 10.4 Data Freshness

| Feature Type | Max Age | Update Frequency |
|--------------|---------|------------------|
| Request features | <1s | Real-time |
| Model static features | <1 hour | On change |
| Performance 5min | <5 min | Every 5 min |
| Performance 1hr | <5 min | Every 5 min (sliding) |
| User session | <30s | Real-time |
| System context | <10s | Every 10s |
| Derived features | <5s | On request |

### 10.5 Availability & Durability

- **Availability**: 99.9% (3 nines)
- **Durability**: 99.999% (5 nines) for offline store
- **Cache hit rate**: >95% (p99)
- **Data retention**:
  - Online store: 7 days
  - Offline store: 90 days
  - Archives: Indefinite (compressed)

---

## Implementation Roadmap

### Phase 1: Core Infrastructure (Weeks 1-2)
- [x] Feature catalog definition
- [ ] Stateless feature extractors
- [ ] Feature store (PostgreSQL + Redis)
- [ ] Feature registry

### Phase 2: Stateful Aggregation (Weeks 3-4)
- [ ] Integration with stream processor windows
- [ ] Performance aggregators (5min, 1hr)
- [ ] Feedback aggregators (session)
- [ ] State persistence

### Phase 3: Serving Layer (Weeks 5-6)
- [ ] Feature serving API
- [ ] 3-tier caching
- [ ] Batch serving
- [ ] Serving metrics

### Phase 4: Advanced Features (Weeks 7-8)
- [ ] Derived feature computation
- [ ] Feature versioning
- [ ] Drift detection
- [ ] Feature validation

### Phase 5: Production Hardening (Weeks 9-10)
- [ ] Performance optimization
- [ ] Load testing
- [ ] Monitoring dashboards
- [ ] Documentation

---

## Conclusion

This ML Feature Engineering Architecture provides a production-ready foundation for real-time feature computation and serving in the LLM Auto-Optimizer. Key highlights:

1. **Comprehensive Feature Catalog**: 55 features covering request, model, performance, feedback, context, and derived categories
2. **Low-Latency Serving**: 3-tier caching architecture achieving <5ms p99 latency
3. **Stream Processing Integration**: Leverages existing windowing and aggregation for stateful features
4. **Production-Grade Quality**: Validation, versioning, drift detection, and comprehensive monitoring
5. **Rust Performance**: Zero-copy serialization, parallel extraction, and efficient memory management

The architecture is designed to scale horizontally, handle 10,000+ events/second, and provide the feature infrastructure needed for intelligent model routing and optimization decisions.

---

**Next Steps:**
1. Review and approve architecture
2. Create implementation tickets
3. Set up feature development environment
4. Begin Phase 1 implementation
5. Establish monitoring and alerting

**Document Version:** 1.0
**Last Updated:** 2025-11-10
**Next Review:** 2025-12-10
