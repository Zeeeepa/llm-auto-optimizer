# ML Training Infrastructure - Visual Diagrams

**Related**: [ML Training Infrastructure Architecture](ML_TRAINING_INFRASTRUCTURE_ARCHITECTURE.md)

---

## 1. High-Level System Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      LLM AUTO-OPTIMIZER ML INFRASTRUCTURE                    │
└─────────────────────────────────────────────────────────────────────────────┘

                                  ┌─────────────────┐
                                  │   Kafka Stream  │
                                  │ (Feedback Events)│
                                  └────────┬────────┘
                                           │
                  ┌────────────────────────┼────────────────────────┐
                  │                        │                        │
                  v                        v                        v
        ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
        │ Online Training │    │Feature Extractor│    │  Event Buffer   │
        │ (Real-time ML)  │    │   (Caching)     │    │  (Mini-batch)   │
        └────────┬────────┘    └─────────────────┘    └─────────────────┘
                 │
                 │ Checkpoints
                 v
        ┌─────────────────┐
        │ Checkpoint      │
        │ Manager         │
        │ (Every 5 min)   │
        └────────┬────────┘
                 │
                 │ Artifacts
                 v
        ┌─────────────────────────────────────────────────────────┐
        │                    MODEL REGISTRY                        │
        │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
        │  │ PostgreSQL   │  │     S3       │  │   Lineage    │  │
        │  │  (Metadata)  │  │  (Artifacts) │  │   Tracking   │  │
        │  └──────────────┘  └──────────────┘  └──────────────┘  │
        └────────┬─────────────────────────────────────────────────┘
                 │
                 │ Model Versions
                 v
        ┌─────────────────────────────────────────────────────────┐
        │                  DEPLOYMENT MANAGER                      │
        │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
        │  │   Canary     │  │  Blue-Green  │  │    Shadow    │  │
        │  │  Deployment  │  │  Deployment  │  │     Mode     │  │
        │  └──────────────┘  └──────────────┘  └──────────────┘  │
        └────────┬─────────────────────────────────────────────────┘
                 │
                 │ Model Updates
                 v
        ┌─────────────────────────────────────────────────────────┐
        │                   INFERENCE ENGINE                       │
        │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
        │  │ Model Cache  │  │Feature Cache │  │ Prediction   │  │
        │  │  (Arc/RwLock)│  │    (LRU)     │  │  Cache (TTL) │  │
        │  └──────────────┘  └──────────────┘  └──────────────┘  │
        └────────┬─────────────────────────────────────────────────┘
                 │
                 │ Predictions (<10ms)
                 v
        ┌─────────────────────────────────────────────────────────┐
        │                 CLIENT APPLICATIONS                      │
        └─────────────────────────────────────────────────────────┘

                 ┌─────────────────────────────────────┐
                 │      MODEL MONITORING               │
                 │  • Performance Tracking             │
                 │  • Drift Detection (ADWIN)          │
                 │  • Business Metrics (Regret)        │
                 │  • Alerting (Slack/PagerDuty)       │
                 └─────────────────────────────────────┘
```

---

## 2. Training Pipeline Data Flow

```
┌──────────────┐
│  Raw Events  │
│  (Kafka)     │
└──────┬───────┘
       │
       v
┌──────────────────────────────────────────────────────────┐
│              FEATURE EXTRACTION                          │
│  ┌────────────────┐  ┌────────────────┐                 │
│  │ Context        │  │  Normalization │                 │
│  │ Features       │→ │  & Scaling     │                 │
│  └────────────────┘  └────────────────┘                 │
└──────────────┬───────────────────────────────────────────┘
               │
               v
        ┌──────────────┐
        │ Event Buffer │
        │ (Size: 32)   │
        └──────┬───────┘
               │
               v
┌──────────────────────────────────────────────────────────┐
│               ONLINE LEARNING                            │
│  ┌─────────────────────────────────────────────────┐    │
│  │  FOR EACH MODEL TYPE:                           │    │
│  │    1. Thompson Sampling → Beta updates          │    │
│  │    2. LinUCB → Matrix updates                   │    │
│  │    3. Contextual Thompson → Gradient descent    │    │
│  └─────────────────────────────────────────────────┘    │
└──────────────┬───────────────────────────────────────────┘
               │
               v
        ┌──────────────┐
        │  Stability   │
        │  Monitor     │
        └──────┬───────┘
               │
               ├─ Stable ──────────────────┐
               │                            │
               ├─ Unstable ────> Pause     │
               │                            v
               └─ Diverged ──> Rollback  ┌──────────────┐
                                          │ Checkpoint   │
                                          │   Manager    │
                                          └──────────────┘
```

---

## 3. Model Registry Workflow

```
┌──────────────────────────────────────────────────────────┐
│                   TRAINING COMPLETE                      │
└────────────────────┬─────────────────────────────────────┘
                     │
                     v
              ┌──────────────┐
              │ Serialize    │
              │ Model        │
              └──────┬───────┘
                     │
                     v
              ┌──────────────┐
              │ Calculate    │
              │ SHA-256 Hash │
              └──────┬───────┘
                     │
                     v
              ┌──────────────┐
              │ Upload to S3 │
              │ (Artifacts)  │
              └──────┬───────┘
                     │
                     v
         ┌───────────────────────┐
         │ Insert into PostgreSQL│
         │  - version: "1.2.3"   │
         │  - metrics: {...}     │
         │  - artifact_path      │
         │  - status: "dev"      │
         └───────────┬───────────┘
                     │
                     v
         ┌───────────────────────┐
         │ Record Lineage        │
         │  - event: "trained"   │
         │  - timestamp          │
         │  - metadata           │
         └───────────┬───────────┘
                     │
                     v
         ┌───────────────────────┐
         │ Model Ready for       │
         │ Testing/Deployment    │
         └───────────────────────┘
```

---

## 4. A/B Testing Flow

```
┌──────────────────────────────────────────────────────────┐
│                  CREATE EXPERIMENT                       │
│  • Name: "thompson_vs_linucb"                           │
│  • Variants: [control, treatment]                        │
│  • Traffic: 50/50 split                                  │
│  • Primary Metric: average_reward                        │
│  • Significance: p < 0.05                                │
└────────────────────┬─────────────────────────────────────┘
                     │
                     v
          ┌──────────────────────┐
          │   START EXPERIMENT   │
          │   status: "running"  │
          └──────────┬───────────┘
                     │
                     v
┌────────────────────┴────────────────────┐
│          TRAFFIC ROUTING                │
│                                          │
│  ┌────────────────┐  ┌────────────────┐│
│  │  Variant A     │  │  Variant B     ││
│  │  (Control)     │  │  (Treatment)   ││
│  │                │  │                ││
│  │  Users: 50%    │  │  Users: 50%    ││
│  └────────┬───────┘  └────────┬───────┘│
└───────────┼──────────────────┼─────────┘
            │                  │
            v                  v
     ┌──────────┐       ┌──────────┐
     │ Observe  │       │ Observe  │
     │ Rewards  │       │ Rewards  │
     └─────┬────┘       └─────┬────┘
           │                  │
           └────────┬─────────┘
                    │
                    v
         ┌──────────────────────┐
         │ STATISTICAL ANALYSIS │
         │  • Welch's t-test    │
         │  • p-value < 0.05?   │
         │  • Effect size       │
         │  • Confidence interval│
         └──────────┬───────────┘
                    │
            ┌───────┴───────┐
            │               │
            v               v
    ┌───────────┐    ┌──────────────┐
    │ Winner    │    │ No Difference│
    │ Detected  │    │ (Continue)   │
    └─────┬─────┘    └──────────────┘
          │
          v
    ┌──────────────┐
    │ PROMOTE      │
    │ WINNER       │
    │ to Production│
    └──────────────┘
```

---

## 5. Inference Engine Architecture

```
┌──────────────────────────────────────────────────────────┐
│                    CLIENT REQUEST                        │
└────────────────────┬─────────────────────────────────────┘
                     │
                     v
              ┌──────────────┐
              │ Prediction   │
              │ Cache Check  │ ← TTL: 60s
              └──────┬───────┘
                     │
              ┌──────┴──────┐
              │             │
         Hit  v             v  Miss
      ┌──────────┐    ┌──────────┐
      │ Return   │    │ Feature  │
      │ Cached   │    │ Cache    │
      └──────────┘    └────┬─────┘
                           │
                     ┌─────┴──────┐
                     │            │
                Hit  v            v  Miss
              ┌──────────┐  ┌──────────┐
              │ Features │  │ Extract  │
              │ Ready    │  │ Features │
              └────┬─────┘  └────┬─────┘
                   │             │
                   └──────┬──────┘
                          │
                          v
                   ┌──────────────┐
                   │ Get Model    │
                   │ (Arc::clone) │ ← Read Lock (fast)
                   └──────┬───────┘
                          │
                          v
                   ┌──────────────┐
                   │  Predict     │
                   │  (inference) │ ← <10ms
                   └──────┬───────┘
                          │
                          v
                   ┌──────────────┐
                   │ Cache Result │
                   └──────┬───────┘
                          │
                          v
                   ┌──────────────┐
                   │ Return       │
                   │ Prediction   │
                   └──────────────┘

┌─────────────────────────────────────────────────────────┐
│               HOT MODEL SWAPPING                        │
│                                                          │
│  1. Load new model from registry                        │
│  2. Warmup (pre-allocate resources)                     │
│  3. Validate (smoke tests)                              │
│  4. Acquire WRITE lock (brief!)                         │
│  5. Swap model (atomic)                                 │
│  6. Release lock                                        │
│  7. Invalidate caches                                   │
│                                                          │
│  Total Downtime: <100ms                                 │
└─────────────────────────────────────────────────────────┘
```

---

## 6. Canary Deployment Timeline

```
Time: 0min                10min               20min               30min
│                         │                   │                   │
├─────────────────────────┼───────────────────┼───────────────────┤
│                         │                   │                   │
│   1% Traffic            │  10% Traffic      │  50% Traffic      │  100%
│   ┌─┐                   │  ┌───────────┐    │  ┌──────────────────────┐
│   │C│ New Model         │  │CCCCCCCCCC │    │  │CCCCCCCCCCCCCCCCCCCCC │
│   └─┘                   │  └───────────┘    │  └──────────────────────┘
│   ┌──────────────────┐  │  ┌─────────┐      │  ┌──────────┐            │
│   │OOOOOOOOOOOOOOOOOO│  │  │OOOOOOOOO│      │  │OOOOOOOOO │            │
│   └──────────────────┘  │  └─────────┘      │  └──────────┘            │
│   Old Model             │                   │                           │
│                         │                   │                           │
│   Monitor:              │   Check:          │   Check:                  │
│   • Latency p99         │   • Error rate    │   • Performance           │
│   • Error rate          │   • Drift         │   • Business metrics      │
│   • Basic metrics       │   • Comparison    │   • Final validation      │
│                         │                   │                           │
└─────────────────────────┴───────────────────┴───────────────────────────┘

Legend:
  C = New Model (Canary)
  O = Old Model (Stable)

If ANY stage fails:
  ↓
  ┌──────────────────┐
  │ AUTO ROLLBACK    │
  │ to Old Model     │
  │ (Instant)        │
  └──────────────────┘
```

---

## 7. Monitoring & Drift Detection

```
┌─────────────────────────────────────────────────────────┐
│                   MONITORING PIPELINE                    │
└─────────────────────────────────────────────────────────┘

  ┌───────────────┐
  │ Prediction    │
  │ Made          │
  └───────┬───────┘
          │
          ├───────────────────────────────────────┐
          │                                       │
          v                                       v
  ┌───────────────┐                      ┌───────────────┐
  │ Performance   │                      │ Business      │
  │ Metrics       │                      │ Metrics       │
  ├───────────────┤                      ├───────────────┤
  │ • Latency     │                      │ • Reward      │
  │ • Throughput  │                      │ • Regret      │
  │ • Error Rate  │                      │ • Exploration │
  │ • CPU/Memory  │                      │ • Actions     │
  └───────┬───────┘                      └───────┬───────┘
          │                                      │
          v                                      v
  ┌───────────────┐                      ┌───────────────┐
  │ Prometheus    │                      │ Time Series   │
  │ Export        │                      │ Database      │
  └───────┬───────┘                      └───────┬───────┘
          │                                      │
          └──────────────┬───────────────────────┘
                         │
                         v
                  ┌──────────────┐
                  │ Drift        │
                  │ Detectors    │
                  ├──────────────┤
                  │ • ADWIN      │
                  │ • Page-Hinkley│
                  │ • CUSUM      │
                  └──────┬───────┘
                         │
                    ┌────┴─────┐
                    │          │
            Stable  v          v  Drift
          ┌──────────┐    ┌──────────┐
          │ Continue │    │ ALERT    │
          │ Monitoring│   │ & Trigger│
          └──────────┘    │ Retrain  │
                          └──────────┘
```

---

## 8. Fault Tolerance Architecture

```
┌─────────────────────────────────────────────────────────┐
│                 FAULT SCENARIOS                          │
└─────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────┐
│ Scenario 1: Model Unavailable                           │
├──────────────────────────────────────────────────────────┤
│  Request → Model Not Found → Fallback Policy            │
│                                    ↓                     │
│                           ┌────────┴────────┐            │
│                           │ Previous Version│            │
│                           └─────────────────┘            │
└──────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────┐
│ Scenario 2: Database Failure                             │
├──────────────────────────────────────────────────────────┤
│  Query → DB Error → Circuit Breaker Opens               │
│                                    ↓                     │
│                           ┌────────┴────────┐            │
│                           │ In-Memory Cache │            │
│                           └─────────────────┘            │
└──────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────┐
│ Scenario 3: High Latency                                 │
├──────────────────────────────────────────────────────────┤
│  Latency > Threshold → Traffic Throttle                 │
│                                    ↓                     │
│                           ┌────────┴────────┐            │
│                           │ Faster Model    │            │
│                           └─────────────────┘            │
└──────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────┐
│ Scenario 4: Training Divergence                          │
├──────────────────────────────────────────────────────────┤
│  Loss → ∞ → Stability Monitor Detects                   │
│                                    ↓                     │
│                           ┌────────┴────────┐            │
│                           │ Rollback to     │            │
│                           │ Last Checkpoint │            │
│                           └─────────────────┘            │
└──────────────────────────────────────────────────────────┘

                    ┌─────────────────┐
                    │ RECOVERY TIME   │
                    ├─────────────────┤
                    │ Model Fallback  │ <1ms
                    │ Cache Fallback  │ <5ms
                    │ Circuit Recovery│ 30s
                    │ Checkpoint Load │ 2s
                    └─────────────────┘
```

---

## 9. End-to-End Request Flow

```
1. Client Request Arrives
   │
   v
2. Load Balancer
   │
   v
3. Inference Service
   │
   ├─ Check Prediction Cache (TTL)
   │  └─ Hit? Return immediately
   │
   ├─ Check Feature Cache (LRU)
   │  └─ Miss? Extract features
   │
   ├─ Get Active Model (Arc clone)
   │  └─ Read lock (shared, fast)
   │
   ├─ Run Inference (<10ms target)
   │  └─ Vectorized operations
   │
   ├─ Cache Prediction
   │
   └─ Return Response
      │
      v
4. Log to Kafka (async)
   │
   v
5. Training Pipeline (background)
   │
   ├─ Buffer Events
   │
   ├─ Mini-batch Update (every 30s)
   │
   └─ Checkpoint (every 5 min)
      │
      v
6. Registry Update
   │
   v
7. Deployment Manager
   │
   └─ Should Deploy? → Canary Rollout
      │
      v
8. Model Hot Swap (<100ms)
   │
   v
9. Continue serving with new model

Total Latency Breakdown:
┌─────────────────────────┬──────────┐
│ Network                 │ 2-3ms    │
│ Cache Check             │ <1ms     │
│ Feature Extraction      │ 1-2ms    │
│ Model Inference         │ 3-5ms    │
│ Serialization           │ 1ms      │
│ Total (p50)             │ ~8ms     │
│ Total (p99)             │ <10ms    │
└─────────────────────────┴──────────┘
```

---

## 10. Data Flow Summary

```
┌─────────────┐
│   Events    │  (Millions/day)
└──────┬──────┘
       │
       v
┌──────────────────┐
│ Feature Pipeline │  (Extract, normalize, cache)
└──────┬───────────┘
       │
       ├──────────────────────────────┐
       │                              │
       v                              v
┌──────────────┐              ┌──────────────┐
│   Training   │              │  Inference   │
│   (Async)    │              │   (Sync)     │
└──────┬───────┘              └──────┬───────┘
       │                             │
       v                             v
┌──────────────┐              ┌──────────────┐
│   Registry   │              │  Monitoring  │
│  (Versions)  │              │  (Metrics)   │
└──────┬───────┘              └──────┬───────┘
       │                             │
       v                             v
┌──────────────┐              ┌──────────────┐
│  Deployment  │              │   Alerts     │
│   Manager    │              │  (Slack/PD)  │
└──────────────┘              └──────────────┘

Key Characteristics:
• Asynchronous training pipeline
• Synchronous inference path
• Decoupled components
• Event-driven architecture
• Observable at every stage
```

---

These diagrams provide visual representations of the ML Training Infrastructure architecture. Refer to the main architecture document for detailed specifications and implementation guidance.
