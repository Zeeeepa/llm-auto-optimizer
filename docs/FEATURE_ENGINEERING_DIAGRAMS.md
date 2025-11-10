# Feature Engineering Architecture Diagrams

**Visual representations of the ML Feature Engineering system**

## 1. System Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                  LLM Auto-Optimizer Feature Engineering                 │
└─────────────────────────────────────────────────────────────────────────┘

                         ┌──────────────────┐
                         │   Event Sources  │
                         ├──────────────────┤
                         │  • LLM Requests  │
                         │  • Performance   │
                         │  • User Feedback │
                         │  • System Events │
                         └────────┬─────────┘
                                  │
                                  ▼
                         ┌──────────────────┐
                         │  Event Stream    │
                         │     (Kafka)      │
                         └────────┬─────────┘
                                  │
                 ┌────────────────┼────────────────┐
                 ▼                ▼                ▼
        ┌────────────────┐┌────────────────┐┌────────────────┐
        │   Stateless    ││   Stateful     ││   Context      │
        │   Extraction   ││  Aggregation   ││   Extraction   │
        │                ││                ││                │
        │ • Prompt len   ││ • Windows      ││ • CPU/Memory   │
        │ • Task type    ││ • Percentiles  ││ • Queue depth  │
        │ • Temporal     ││ • Avg/Count    ││ • Budget       │
        └───────┬────────┘└───────┬────────┘└───────┬────────┘
                │                 │                 │
                └────────┬────────┴────────┬────────┘
                         ▼                 ▼
                 ┌────────────────┐ ┌────────────────┐
                 │  Validation &  │ │    Derived     │
                 │ Normalization  │ │   Features     │
                 │                │ │                │
                 │ • Range check  │ │ • Embeddings   │
                 │ • Outliers     │ │ • Ratios       │
                 │ • Imputation   │ │ • ML Models    │
                 └───────┬────────┘ └───────┬────────┘
                         │                 │
                         └────────┬────────┘
                                  ▼
                         ┌──────────────────┐
                         │  Feature Store   │
                         │                  │
                         │  L1: Memory      │
                         │  L2: Redis       │
                         │  L3: PostgreSQL  │
                         └────────┬─────────┘
                                  │
                 ┌────────────────┼────────────────┐
                 ▼                ▼                ▼
        ┌────────────────┐┌────────────────┐┌────────────────┐
        │  Decision      ││   Analyzer     ││  A/B Testing   │
        │  Engine        ││                ││                │
        │                ││                ││                │
        │ Model Select   ││ Perf Analysis  ││ Experiments    │
        └────────────────┘└────────────────┘└────────────────┘
```

## 2. Feature Store 3-Tier Cache Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Feature Store Architecture                   │
│                                                                     │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │                        Read Path                              │ │
│  │                                                               │ │
│  │   Feature Request                                             │ │
│  │         │                                                     │ │
│  │         ▼                                                     │ │
│  │   ┌───────────┐                                              │ │
│  │   │ L1 Cache  │  ←─ Hot Features (1 hour)                    │ │
│  │   │ (Memory)  │     512 MB                                   │ │
│  │   │ <1ms p99  │     LRU eviction                             │ │
│  │   └─────┬─────┘                                              │ │
│  │         │                                                     │ │
│  │         │ Cache Miss                                          │ │
│  │         ▼                                                     │ │
│  │   ┌───────────┐                                              │ │
│  │   │ L2 Cache  │  ←─ Warm Features (24 hours)                 │ │
│  │   │  (Redis)  │     10 GB per node                           │ │
│  │   │ <5ms p99  │     3 replicas                               │ │
│  │   │           │     Hash slot sharding                       │ │
│  │   └─────┬─────┘                                              │ │
│  │         │                                                     │ │
│  │         │ Cache Miss                                          │ │
│  │         ▼                                                     │ │
│  │   ┌───────────┐                                              │ │
│  │   │ L3 Store  │  ←─ All Features (7 days)                    │ │
│  │   │(PostgreSQL)│    Indexed by entity + timestamp            │ │
│  │   │ <50ms p99 │     Partitioned by month                     │ │
│  │   └───────────┘                                              │ │
│  │                                                               │ │
│  └───────────────────────────────────────────────────────────────┘ │
│                                                                     │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │                       Write Path                              │ │
│  │                                                               │ │
│  │   Feature Update                                              │ │
│  │         │                                                     │ │
│  │         ├──────────────────────┬──────────────────────┐      │ │
│  │         ▼                      ▼                      ▼      │ │
│  │   ┌───────────┐          ┌───────────┐        ┌───────────┐ │ │
│  │   │ L3 Store  │          │ L2 Cache  │        │ L1 Cache  │ │ │
│  │   │(PostgreSQL)│          │  (Redis)  │        │ (Memory)  │ │ │
│  │   │  (Write)  │          │  (Update) │        │(Invalidate)│ │ │
│  │   └───────────┘          └───────────┘        └───────────┘ │ │
│  │         │                      │                      │      │ │
│  │         └──────────────────────┴──────────────────────┘      │ │
│  │                                │                             │ │
│  │                                ▼                             │ │
│  │                          Write Success                        │ │
│  │                                                               │ │
│  └───────────────────────────────────────────────────────────────┘ │
│                                                                     │
│  Cache Hit Rates:                                                  │
│  • L1: 70-80% (hot path)                                           │
│  • L2: 20-25% (warm path)                                          │
│  • L3: <5% (cold path)                                             │
│  • Combined: >95%                                                  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## 3. Real-Time Feature Pipeline

```
┌──────────────────────────────────────────────────────────────────────┐
│                   Real-Time Feature Pipeline                         │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Event Stream (Kafka Topic: llm-events)                             │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐           │
│  │ Request  │  │  Perf    │  │  User    │  │  System  │           │
│  │  Events  │  │ Metrics  │  │ Feedback │  │  Events  │           │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘           │
│       │             │             │             │                  │
│       └─────────────┴─────────────┴─────────────┘                  │
│                      │                                              │
│                      ▼                                              │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │              Stream Processor                                │  │
│  │  • Deserialize events                                        │  │
│  │  • Validate schema                                           │  │
│  │  • Extract event time                                        │  │
│  │  • Update watermark                                          │  │
│  │  • Assign to windows                                         │  │
│  └────────────────────────────┬─────────────────────────────────┘  │
│                                │                                    │
│       ┌────────────────────────┼────────────────────────┐          │
│       ▼                        ▼                        ▼          │
│  ┌─────────┐            ┌──────────┐            ┌──────────┐      │
│  │Request  │            │  Model   │            │ Context  │      │
│  │Features │            │ Features │            │ Features │      │
│  │         │            │          │            │          │      │
│  │REQ_001  │            │ MDL_001  │            │ CTX_001  │      │
│  │  ...    │            │   ...    │            │   ...    │      │
│  │REQ_015  │            │ MDL_010  │            │ CTX_010  │      │
│  └────┬────┘            └────┬─────┘            └────┬─────┘      │
│       │                      │                       │             │
│       │   Immediate (< 1ms)  │                       │             │
│       └──────────────────────┴───────────────────────┘             │
│                              │                                     │
│                              ▼                                     │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │              Stateful Aggregation (Windows)                  │  │
│  │                                                              │  │
│  │  ┌──────────────────────────────────────────────────────┐   │  │
│  │  │ 5-Min Tumbling Window                                │   │  │
│  │  │  [00:00 - 05:00) [05:00 - 10:00) [10:00 - 15:00)    │   │  │
│  │  │                                                      │   │  │
│  │  │  Aggregators:                                        │   │  │
│  │  │  • PercentileAgg(latency) → P50, P95, P99           │   │  │
│  │  │  • AvgAgg(cost)           → avg_cost_per_request    │   │  │
│  │  │  • CountAgg(errors/total) → error_rate              │   │  │
│  │  │  • CountAgg(total)        → throughput_qps          │   │  │
│  │  └──────────────────────────────────────────────────────┘   │  │
│  │                                                              │  │
│  │  ┌──────────────────────────────────────────────────────┐   │  │
│  │  │ 1-Hour Sliding Window (slide every 5 min)           │   │  │
│  │  │  [00:00-01:00) [00:05-01:05) [00:10-01:10)          │   │  │
│  │  │                                                      │   │  │
│  │  │  Aggregators:                                        │   │  │
│  │  │  • PercentileAgg(latency) → P50, P95 (1hr)          │   │  │
│  │  │  • AvgAgg(success_rate)   → success_rate_1hr        │   │  │
│  │  └──────────────────────────────────────────────────────┘   │  │
│  │                                                              │  │
│  │  ┌──────────────────────────────────────────────────────┐   │  │
│  │  │ Session Window (30-min gap, 4-hr max)               │   │  │
│  │  │                                                      │   │  │
│  │  │  User A: [00:00-00:45) ─gap─ [01:20-02:15)          │   │  │
│  │  │  User B: [00:10-01:50)                              │   │  │
│  │  │                                                      │   │  │
│  │  │  Aggregators:                                        │   │  │
│  │  │  • AvgAgg(rating)         → avg_user_rating_session │   │  │
│  │  │  • CountAgg(regeneration) → regeneration_rate       │   │  │
│  │  └──────────────────────────────────────────────────────┘   │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                     │
│                              │ Window Close (on watermark)         │
│                              ▼                                     │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │           Derived Feature Computation                        │  │
│  │  • cost_latency_score = (cost_norm + latency_norm) / 2      │  │
│  │  • quality_cost_ratio = success_rate / cost                 │  │
│  │  • prompt_embedding (8D via BERT → PCA)                     │  │
│  │  • user_behavior_cluster (K-means online)                   │  │
│  └────────────────────────────┬─────────────────────────────────┘  │
│                                │                                    │
│                                ▼                                    │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │          Validation & Normalization                          │  │
│  │  • Range check: 0.0 ≤ value ≤ 1.0 (for normalized features) │  │
│  │  • Null handling: Impute with default or error              │  │
│  │  • Outlier detection: |z-score| > 3 → log warning           │  │
│  │  • Min-max normalization for unbounded features             │  │
│  └────────────────────────────┬─────────────────────────────────┘  │
│                                │                                    │
│                                ▼                                    │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │              Feature Store Writer                            │  │
│  │                                                              │  │
│  │  Write to:                                                   │  │
│  │  1. PostgreSQL (offline + online)                           │  │
│  │  2. Redis (L2 cache) with TTL                               │  │
│  │  3. Emit metrics (latency, count, errors)                   │  │
│  │  4. Invalidate L1 cache                                     │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                      │
│  Latency Budget:                                                    │
│  • Stateless extraction:    < 1ms                                   │
│  • Window aggregation:      < 0.1ms per event                       │
│  • Derived computation:     < 2ms                                   │
│  • Validation:              < 0.1ms                                 │
│  • Store write (async):     < 5ms                                   │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

## 4. Feature Catalog Organization

```
┌──────────────────────────────────────────────────────────────────┐
│                    Feature Catalog (55 Features)                 │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ REQUEST FEATURES (15)                                      │ │
│  │ ─────────────────────────                                  │ │
│  │ REQ_001: prompt_length              (stateless)            │ │
│  │ REQ_002: prompt_char_count          (stateless)            │ │
│  │ REQ_003: prompt_complexity          (stateless)            │ │
│  │ REQ_004: expected_output_length     (stateless)            │ │
│  │ REQ_005: task_type_classification   (stateless)            │ │
│  │ REQ_006: prompt_language            (stateless)            │ │
│  │ REQ_007: prompt_domain              (stateless)            │ │
│  │ REQ_008: request_priority           (stateless)            │ │
│  │ REQ_009: hour_of_day_cos            (stateless, temporal)  │ │
│  │ REQ_010: hour_of_day_sin            (stateless, temporal)  │ │
│  │ REQ_011: day_of_week_cos            (stateless, temporal)  │ │
│  │ REQ_012: day_of_week_sin            (stateless, temporal)  │ │
│  │ REQ_013: is_weekend                 (stateless, temporal)  │ │
│  │ REQ_014: session_depth              (stateful, session)    │ │
│  │ REQ_015: user_tier                  (stateless, lookup)    │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ MODEL FEATURES (10)                                        │ │
│  │ ────────────────────                                       │ │
│  │ MDL_001: model_context_length       (static, registry)     │ │
│  │ MDL_002: model_cost_per_1k_input    (static, registry)     │ │
│  │ MDL_003: model_cost_per_1k_output   (static, registry)     │ │
│  │ MDL_004: model_capability_score     (static, registry)     │ │
│  │ MDL_005: model_training_cutoff_age  (static, computed)     │ │
│  │ MDL_006: model_supports_functions   (static, registry)     │ │
│  │ MDL_007: model_supports_vision      (static, registry)     │ │
│  │ MDL_008: model_supports_json_mode   (static, registry)     │ │
│  │ MDL_009: model_provider             (static, registry)     │ │
│  │ MDL_010: model_size_class           (static, registry)     │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ PERFORMANCE FEATURES (12) - Windowed Aggregations         │ │
│  │ ──────────────────────────────────────────────────────     │ │
│  │ PERF_001: latency_p50_5min          (5min tumbling)        │ │
│  │ PERF_002: latency_p95_5min          (5min tumbling)        │ │
│  │ PERF_003: latency_p99_5min          (5min tumbling)        │ │
│  │ PERF_004: latency_p50_1hr           (1hr sliding)          │ │
│  │ PERF_005: latency_p95_1hr           (1hr sliding)          │ │
│  │ PERF_006: success_rate_5min         (5min tumbling)        │ │
│  │ PERF_007: success_rate_1hr          (1hr sliding)          │ │
│  │ PERF_008: error_rate_5min           (5min tumbling)        │ │
│  │ PERF_009: avg_cost_per_request_5min (5min tumbling)        │ │
│  │ PERF_010: throughput_qps_5min       (5min tumbling)        │ │
│  │ PERF_011: token_efficiency_5min     (5min tumbling)        │ │
│  │ PERF_012: retry_rate_5min           (5min tumbling)        │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ FEEDBACK FEATURES (8) - User Engagement                    │ │
│  │ ────────────────────────────────────                       │ │
│  │ FDBK_001: avg_user_rating_session   (session window)       │ │
│  │ FDBK_002: avg_user_rating_1hr       (1hr sliding)          │ │
│  │ FDBK_003: thumbs_up_ratio_1hr       (1hr sliding)          │ │
│  │ FDBK_004: regeneration_rate_1hr     (1hr sliding)          │ │
│  │ FDBK_005: task_completion_rate      (session window)       │ │
│  │ FDBK_006: avg_response_time_user    (session window)       │ │
│  │ FDBK_007: copy_paste_rate_1hr       (1hr sliding)          │ │
│  │ FDBK_008: follow_up_rate_session    (session window)       │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ CONTEXTUAL FEATURES (10) - System State                    │ │
│  │ ─────────────────────────────────────                      │ │
│  │ CTX_001: system_load_cpu            (real-time)            │ │
│  │ CTX_002: system_load_memory         (real-time)            │ │
│  │ CTX_003: queue_depth_requests       (real-time)            │ │
│  │ CTX_004: concurrent_requests        (real-time)            │ │
│  │ CTX_005: available_budget_ratio     (real-time)            │ │
│  │ CTX_006: time_until_budget_reset    (real-time)            │ │
│  │ CTX_007: model_availability         (real-time)            │ │
│  │ CTX_008: recent_timeout_rate_5min   (5min tumbling)        │ │
│  │ CTX_009: api_quota_remaining        (real-time)            │ │
│  │ CTX_010: region_load                (real-time)            │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ DERIVED FEATURES (10) - Computed/ML-based                  │ │
│  │ ───────────────────────────────────────                    │ │
│  │ DRV_001: prompt_embedding_dim_1-8   (BERT + PCA)           │ │
│  │ DRV_002: cost_latency_score         (formula)              │ │
│  │ DRV_003: quality_cost_ratio         (formula)              │ │
│  │ DRV_004: prompt_similarity_to_avg   (cosine similarity)    │ │
│  │ DRV_005: model_fit_score            (dot product)          │ │
│  │ DRV_006: user_behavior_cluster      (K-means online)       │ │
│  │ DRV_007: time_pressure_indicator    (formula)              │ │
│  │ DRV_008: load_adjusted_latency      (formula)              │ │
│  │ DRV_009: model_fatigue_score        (exp decay)            │ │
│  │ DRV_010: pareto_efficiency_score    (distance calc)        │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘

Feature Access Patterns:
  • Stateless (immediate):    25 features (REQ + MDL + CTX subset)
  • Windowed (5min delay):    16 features (PERF + CTX subset)
  • Session-based (variable): 4 features (FDBK subset)
  • Derived (on-demand):      10 features (DRV)
```

## 5. Feature Serving Flow

```
┌────────────────────────────────────────────────────────────────────┐
│                    Feature Serving Request                         │
├────────────────────────────────────────────────────────────────────┤
│                                                                    │
│  Client Request:                                                   │
│  GET /features/model/gpt-4?features=PERF_002,PERF_006,MDL_004      │
│                                                                    │
│         │                                                          │
│         ▼                                                          │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │ Feature Serving API                                         │  │
│  │ 1. Parse request (entity_id="gpt-4", features=[...])        │  │
│  │ 2. Validate request (feature names exist, not too many)     │  │
│  │ 3. Map features to groups                                   │  │
│  │    PERF_002, PERF_006 → "model:perf_5min"                   │  │
│  │    MDL_004            → "model:static"                      │  │
│  └────────────────────────┬────────────────────────────────────┘  │
│                           │                                        │
│                           ▼                                        │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │ Parallel Feature Group Fetch                                │  │
│  │                                                             │  │
│  │  ┌──────────────────────┐    ┌──────────────────────┐      │  │
│  │  │ Fetch "perf_5min"    │    │ Fetch "static"       │      │  │
│  │  └──────────┬───────────┘    └──────────┬───────────┘      │  │
│  │             │                             │                 │  │
│  │             ▼                             ▼                 │  │
│  │  ┌──────────────────────┐    ┌──────────────────────┐      │  │
│  │  │ Try L1 Cache         │    │ Try L1 Cache         │      │  │
│  │  │ key="feature:model:  │    │ key="feature:model:  │      │  │
│  │  │      gpt-4:perf_5min"│    │      gpt-4:static"   │      │  │
│  │  └──────────┬───────────┘    └──────────┬───────────┘      │  │
│  │             │ HIT (70%)                  │ HIT (90%)        │  │
│  │             │                             │                 │  │
│  │     MISS    │                     MISS    │                 │  │
│  │      (30%)  │                      (10%)  │                 │  │
│  │             ▼                             ▼                 │  │
│  │  ┌──────────────────────┐    ┌──────────────────────┐      │  │
│  │  │ Try L2 Redis         │    │ Try L2 Redis         │      │  │
│  │  │ HGETALL              │    │ HGETALL              │      │  │
│  │  └──────────┬───────────┘    └──────────┬───────────┘      │  │
│  │             │ HIT (25%)                  │ HIT (9%)         │  │
│  │             │ ← Populate L1              │ ← Populate L1    │  │
│  │     MISS    │                     MISS   │                  │  │
│  │      (5%)   │                      (1%)  │                  │  │
│  │             ▼                             ▼                 │  │
│  │  ┌──────────────────────┐    ┌──────────────────────┐      │  │
│  │  │ Try L3 PostgreSQL    │    │ Try L3 PostgreSQL    │      │  │
│  │  │ SELECT ... WHERE     │    │ SELECT ... WHERE     │      │  │
│  │  └──────────┬───────────┘    └──────────┬───────────┘      │  │
│  │             │ HIT (4.5%)                 │ HIT (0.9%)       │  │
│  │             │ ← Populate L2 & L1         │ ← Populate L2+L1 │  │
│  │             │                             │                 │  │
│  │             └──────────┬──────────────────┘                 │  │
│  └────────────────────────┼────────────────────────────────────┘  │
│                           │                                        │
│                           ▼                                        │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │ Merge Feature Groups                                        │  │
│  │ {                                                           │  │
│  │   "latency_p95_5min": 234.56,  // from perf_5min           │  │
│  │   "success_rate_5min": 0.98,   // from perf_5min           │  │
│  │   "model_capability_score": 0.92 // from static            │  │
│  │ }                                                           │  │
│  └────────────────────────┬────────────────────────────────────┘  │
│                           │                                        │
│                           ▼                                        │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │ Handle Missing Features (if any)                            │  │
│  │ • Look up default values from registry                      │  │
│  │ • Impute or return error based on config                   │  │
│  └────────────────────────┬────────────────────────────────────┘  │
│                           │                                        │
│                           ▼                                        │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │ Compute Derived Features (if requested)                     │  │
│  │ • Calculate ratios, scores                                  │  │
│  │ • Run lightweight ML models                                 │  │
│  └────────────────────────┬────────────────────────────────────┘  │
│                           │                                        │
│                           ▼                                        │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │ Build Response                                              │  │
│  │ {                                                           │  │
│  │   "entity_id": "gpt-4",                                     │  │
│  │   "features": { ... },                                      │  │
│  │   "metadata": {                                             │  │
│  │     "computed_at": "2025-11-10T05:23:00Z",                  │  │
│  │     "version": 1,                                           │  │
│  │     "cache_hit": true,                                      │  │
│  │     "freshness_seconds": 12                                 │  │
│  │   },                                                        │  │
│  │   "latency_ms": 2.3                                         │  │
│  │ }                                                           │  │
│  └────────────────────────┬────────────────────────────────────┘  │
│                           │                                        │
│                           ▼                                        │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │ Record Metrics                                              │  │
│  │ • Serving latency histogram                                │  │
│  │ • Cache hit/miss counters                                  │  │
│  │ • Feature request counter                                  │  │
│  └────────────────────────┬────────────────────────────────────┘  │
│                           │                                        │
│                           ▼                                        │
│                     Return Response                                │
│                                                                    │
│  Latency Breakdown:                                                │
│  • L1 cache hit:     0.5 - 1 ms                                    │
│  • L2 cache hit:     2 - 5 ms                                      │
│  • L3 database hit:  20 - 50 ms                                    │
│  • Derived compute:  +1 - 3 ms                                     │
│                                                                    │
│  p99 Target: <5ms for L1+L2 (95%+ of requests)                     │
│                                                                    │
└────────────────────────────────────────────────────────────────────┘
```

## 6. Drift Detection Workflow

```
┌──────────────────────────────────────────────────────────────────┐
│                  Feature Drift Detection                         │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Background Process (runs every 5 minutes)                       │
│                                                                  │
│  For each feature:                                               │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ 1. Load Baseline Statistics                               │ │
│  │                                                            │ │
│  │    Baseline (computed from 30 days historical data):      │ │
│  │    • Mean: μ₀ = 234.5 ms                                  │ │
│  │    • Std Dev: σ₀ = 45.2 ms                                │ │
│  │    • Percentiles: P25, P50, P75, P95, P99                 │ │
│  │    • Sample count: N = 1,000,000                          │ │
│  └────────────────────────────────────────────────────────────┘ │
│                           │                                      │
│                           ▼                                      │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ 2. Collect Current Window Statistics                      │ │
│  │                                                            │ │
│  │    Current (last 10,000 samples):                         │ │
│  │    • Mean: μ₁ = 312.8 ms                                  │ │
│  │    • Std Dev: σ₁ = 52.1 ms                                │ │
│  │    • Sample count: n = 10,000                             │ │
│  └────────────────────────────────────────────────────────────┘ │
│                           │                                      │
│                           ▼                                      │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ 3. Compute Z-Score for Mean Shift                         │ │
│  │                                                            │ │
│  │    z = |μ₁ - μ₀| / σ₀                                     │ │
│  │      = |312.8 - 234.5| / 45.2                             │ │
│  │      = 78.3 / 45.2                                        │ │
│  │      = 1.73                                               │ │
│  │                                                            │ │
│  │    Threshold: 3.0 (3-sigma rule)                          │ │
│  │    Result: 1.73 < 3.0 → No mean shift detected            │ │
│  └────────────────────────────────────────────────────────────┘ │
│                           │                                      │
│                           ▼                                      │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ 4. Compute PSI (Population Stability Index)               │ │
│  │                                                            │ │
│  │    Bin data into 10 buckets:                              │ │
│  │                                                            │ │
│  │    Bin     Baseline%   Current%    PSI Contribution       │ │
│  │    ───────────────────────────────────────────────────    │ │
│  │    [0-50)     5%         3%       (0.03-0.05)*ln(0.03/0.05)│ │
│  │    [50-100)   8%         6%       (0.06-0.08)*ln(0.06/0.08)│ │
│  │    ...                                                     │ │
│  │    [450-500)  5%         12%      (0.12-0.05)*ln(0.12/0.05)│ │
│  │                                                            │ │
│  │    PSI = Σ (actual% - expected%) × ln(actual%/expected%)  │ │
│  │        = 0.15                                              │ │
│  │                                                            │ │
│  │    Threshold: 0.1                                          │ │
│  │    Result: 0.15 > 0.1 → Distribution shift detected! ⚠️   │ │
│  └────────────────────────────────────────────────────────────┘ │
│                           │                                      │
│                           ▼                                      │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ 5. Determine Drift Severity                               │ │
│  │                                                            │ │
│  │    PSI Severity Levels:                                    │ │
│  │    • PSI < 0.1:  No drift                                 │ │
│  │    • 0.1 ≤ PSI < 0.25: Medium drift ⚠️                    │ │
│  │    • PSI ≥ 0.25: High drift 🔴                            │ │
│  │                                                            │ │
│  │    Current PSI = 0.15 → Medium Severity                   │ │
│  └────────────────────────────────────────────────────────────┘ │
│                           │                                      │
│                           ▼                                      │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ 6. Generate Drift Alert                                   │ │
│  │                                                            │ │
│  │    DriftAlert {                                            │ │
│  │      feature_name: "latency_p95_5min",                     │ │
│  │      drift_type: DistributionShift,                        │ │
│  │      severity: Medium,                                     │ │
│  │      baseline_mean: 234.5,                                 │ │
│  │      current_mean: 312.8,                                  │ │
│  │      z_score: 0.15,  // PSI value                          │ │
│  │      detected_at: "2025-11-10T05:23:00Z"                   │ │
│  │    }                                                       │ │
│  └────────────────────────────────────────────────────────────┘ │
│                           │                                      │
│                           ▼                                      │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ 7. Dispatch Alert                                         │ │
│  │                                                            │ │
│  │    Actions based on severity:                             │ │
│  │    • Medium:  Log warning, update dashboard               │ │
│  │    • High:    Send Slack alert, page on-call              │ │
│  │    • Critical: Trigger incident, auto-rollback (optional) │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  Monitoring Dashboard:                                           │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ Feature Drift Status                                      │ │
│  │                                                            │ │
│  │ latency_p95_5min         ⚠️ Medium Drift (PSI: 0.15)      │ │
│  │ success_rate_5min        ✅ No Drift                       │ │
│  │ avg_cost_per_request     ✅ No Drift                       │ │
│  │ prompt_length            ✅ No Drift                       │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

## 7. Feature Versioning and A/B Testing

```
┌──────────────────────────────────────────────────────────────────┐
│                Feature Versioning for A/B Testing                │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Scenario: Testing new latency feature calculation              │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ Feature Registry                                           │ │
│  │                                                            │ │
│  │ Feature: latency_p95_5min                                  │ │
│  │                                                            │ │
│  │ Version 1 (Control) - 90% traffic                          │ │
│  │ ─────────────────────────────────                          │ │
│  │ • Computation: Percentile(latency_samples, 95)            │ │
│  │ • Active since: 2025-01-01                                │ │
│  │ • Traffic: 90%                                             │ │
│  │                                                            │ │
│  │ Version 2 (Treatment) - 10% traffic                        │ │
│  │ ──────────────────────────────────                         │ │
│  │ • Computation: TDigest(latency_samples).quantile(0.95)    │ │
│  │ • Active since: 2025-11-01                                │ │
│  │ • Traffic: 10%                                             │ │
│  │ • Reason: More accurate for streaming percentiles         │ │
│  └────────────────────────────────────────────────────────────┘ │
│                           │                                      │
│                           ▼                                      │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ Feature Serving Request                                    │ │
│  │                                                            │ │
│  │ GET /features/model/gpt-4?features=PERF_002                │ │
│  │     &user_id=user_12345                                    │ │
│  └────────────────────────────────────────────────────────────┘ │
│                           │                                      │
│                           ▼                                      │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ Version Selection (Hash-based Assignment)                  │ │
│  │                                                            │ │
│  │ assignment = hash("user_12345") % 100                      │ │
│  │            = 73                                            │ │
│  │                                                            │ │
│  │ if assignment < 90:  # 90% traffic to v1                   │ │
│  │     version = 1      # Control                            │ │
│  │ else:                                                      │ │
│  │     version = 2      # Treatment                          │ │
│  │                                                            │ │
│  │ Selected: Version 1 (Control)                              │ │
│  └────────────────────────────────────────────────────────────┘ │
│                           │                                      │
│                           ▼                                      │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ Fetch Feature Value                                        │ │
│  │                                                            │ │
│  │ Redis key: "feature:model:gpt-4:perf_5min:v1"              │ │
│  │                                                            │ │
│  │ Value: {                                                   │ │
│  │   "latency_p95_5min": 234.56,  # v1 computation           │ │
│  │   ...                                                      │ │
│  │ }                                                          │ │
│  └────────────────────────────────────────────────────────────┘ │
│                           │                                      │
│                           ▼                                      │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ Response with Version Metadata                             │ │
│  │                                                            │ │
│  │ {                                                          │ │
│  │   "entity_id": "gpt-4",                                    │ │
│  │   "features": {                                            │ │
│  │     "latency_p95_5min": 234.56                             │ │
│  │   },                                                       │ │
│  │   "metadata": {                                            │ │
│  │     "version": 1,  ← Track for experiment analysis        │ │
│  │     "experiment": "latency_p95_ab_test",                   │ │
│  │     "variant": "control"                                   │ │
│  │   }                                                        │ │
│  │ }                                                          │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ A/B Test Analysis (after 7 days)                           │ │
│  │                                                            │ │
│  │                Control (v1)    Treatment (v2)              │ │
│  │                ─────────────   ──────────────              │ │
│  │ Users:         90,000          10,000                      │ │
│  │ Avg Latency:   234.5 ms        231.2 ms   ✓ -1.4%         │ │
│  │ P95 Latency:   312.8 ms        308.1 ms   ✓ -1.5%         │ │
│  │ Accuracy:      95.2%           96.1%      ✓ +0.9%         │ │
│  │                                                            │ │
│  │ Statistical Significance:                                  │ │
│  │ • T-test p-value: 0.03 (< 0.05) → Significant! ✅         │ │
│  │                                                            │ │
│  │ Decision: Roll out v2 to 100% traffic                      │ │
│  └────────────────────────────────────────────────────────────┘ │
│                           │                                      │
│                           ▼                                      │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ Gradual Rollout                                            │ │
│  │                                                            │ │
│  │ Day 1:  90% v1, 10% v2  (initial)                          │ │
│  │ Day 8:  50% v1, 50% v2  (expand)                           │ │
│  │ Day 9:  25% v1, 75% v2  (confidence)                       │ │
│  │ Day 10:  0% v1, 100% v2 (full rollout) ✅                  │ │
│  │                                                            │ │
│  │ Monitor metrics at each stage, rollback if degradation     │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

---

**Document Version:** 1.0
**Last Updated:** 2025-11-10
**Related:** [ML_FEATURE_ENGINEERING_ARCHITECTURE.md](ML_FEATURE_ENGINEERING_ARCHITECTURE.md)
