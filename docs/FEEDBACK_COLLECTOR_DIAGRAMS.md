# Feedback Collector - Architecture Diagrams

**Version**: 1.0.0
**Date**: 2025-11-09

This document contains visual diagrams for the Feedback Collector architecture.

---

## 1. High-Level System Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                    FEEDBACK COLLECTOR SYSTEM                        │
└─────────────────────────────────────────────────────────────────────┘

    ┌────────────────┐
    │  Applications  │
    │  (LLM Users)   │
    └────────┬───────┘
             │
             │ HTTP/gRPC/SDK
             ▼
┌────────────────────────────────────────────────────────────────────┐
│                      FEEDBACK COLLECTOR                             │
├────────────────────────────────────────────────────────────────────┤
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │ Ingestion Layer                                              │  │
│  │ - HTTP API                                                    │  │
│  │ - gRPC API                                                    │  │
│  │ - Direct SDK                                                  │  │
│  │ - Rate Limiting: 1000 events/sec per client                  │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                      │
│  ┌──────────────────────────▼───────────────────────────────────┐  │
│  │ Processing Layer                                             │  │
│  │ - Event Validation & Enrichment                              │  │
│  │ - Ring Buffer (10K capacity)                                 │  │
│  │ - Batching Engine (100 events / 10s)                         │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                      │
│  ┌──────────────────────────▼───────────────────────────────────┐  │
│  │ Streaming Layer                                              │  │
│  │ - Kafka Producer (rdkafka)                                   │  │
│  │ - Idempotent Writes                                          │  │
│  │ - Gzip Compression (60-80% reduction)                        │  │
│  │ - Retry Logic (3 attempts)                                   │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                      │
│  ┌──────────────────────────▼───────────────────────────────────┐  │
│  │ Observability Layer                                          │  │
│  │ - OpenTelemetry SDK                                          │  │
│  │ - Metrics: Prometheus (OTLP)                                 │  │
│  │ - Traces: Jaeger (OTLP/gRPC)                                 │  │
│  │ - Logs: Structured JSON                                      │  │
│  └──────────────────────────────────────────────────────────────┘  │
└────────────────────────────┬───────────────────────────────────────┘
                             │
         ┌───────────────────┼───────────────────┐
         │                   │                   │
         ▼                   ▼                   ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│ Kafka Cluster   │ │ OTEL Collector  │ │ Prometheus      │
│ Topic: feedback │ │ Port: 4317      │ │ & Jaeger        │
│ Partitions: 12  │ │                 │ │                 │
└─────────────────┘ └─────────────────┘ └─────────────────┘
```

---

## 2. Component Interaction Flow

```
┌──────────────────────────────────────────────────────────────────┐
│                    Event Ingestion Flow                          │
└──────────────────────────────────────────────────────────────────┘

Application
    │
    │ 1. Submit Event (HTTP POST /api/v1/events)
    ▼
┌─────────────────────┐
│  IngestHandler      │
└──────────┬──────────┘
           │
           │ 2. Validate Event
           ▼
┌─────────────────────┐
│  EventValidator     │
│  - Schema check     │
│  - Range validation │
│  - PII redaction    │
└──────────┬──────────┘
           │ Valid ✓
           │
           │ 3. Send to Buffer
           ▼
┌─────────────────────┐
│  Ring Buffer        │
│  (mpsc channel)     │
│  Capacity: 10K      │
└──────────┬──────────┘
           │
           │ 4. Dequeue Event
           ▼
┌─────────────────────┐
│  BatchingEngine     │
│  - Collect events   │
│  - Max: 100 events  │
│  - Max age: 10s     │
└──────────┬──────────┘
           │
           │ 5. Batch Ready
           ▼
┌─────────────────────┐
│  KafkaProducer      │
│  - Serialize        │
│  - Compress (gzip)  │
│  - Publish async    │
└──────────┬──────────┘
           │
           │ 6. Send to Kafka
           ▼
┌─────────────────────┐
│  Kafka Cluster      │
│  Topic: feedback    │
│  Partition: hash(id)│
└─────────────────────┘

[Parallel Track: Telemetry]

Every Step ──────────────┐
                         │
                         ▼
              ┌─────────────────────┐
              │  TelemetryMetrics   │
              │  - Record latency   │
              │  - Count events     │
              │  - Track errors     │
              └──────────┬──────────┘
                         │
                         │ Export (10s interval)
                         ▼
              ┌─────────────────────┐
              │  OTLP Exporter      │
              │  (gRPC:4317)        │
              └──────────┬──────────┘
                         │
                         ▼
              ┌─────────────────────┐
              │  OTEL Collector     │
              └─────────────────────┘
```

---

## 3. Data Flow Diagram

```
┌──────────────────────────────────────────────────────────────────┐
│                    Data Flow & Transformations                   │
└──────────────────────────────────────────────────────────────────┘

┌─────────────────────┐
│  Raw Event          │
│  {                  │
│    type: "user...", │
│    session: "...",  │
│    rating: 4.5      │
│  }                  │
└──────────┬──────────┘
           │
           │ Deserialize + Validate
           ▼
┌─────────────────────┐
│  FeedbackEvent      │
│  (Rust struct)      │
│  - event_id: UUID   │
│  - timestamp: Time  │
│  - data: Typed      │
└──────────┬──────────┘
           │
           │ Enrich
           ▼
┌─────────────────────┐
│  Enriched Event     │
│  + metadata: {      │
│      collector_id,  │
│      collector_ver, │
│      hostname       │
│    }                │
└──────────┬──────────┘
           │
           │ Batch (100 events)
           ▼
┌─────────────────────┐
│  EventBatch         │
│  {                  │
│    batch_id: UUID,  │
│    events: [100],   │
│    created_at: Time │
│  }                  │
└──────────┬──────────┘
           │
           │ Serialize (JSON) + Compress (gzip)
           ▼
┌─────────────────────┐
│  Kafka Message      │
│  - Key: event_id    │
│  - Value: bytes[]   │
│  - Headers: {       │
│      event_type,    │
│      schema_version │
│    }                │
└──────────┬──────────┘
           │
           │ Publish
           ▼
┌─────────────────────┐
│  Kafka Topic        │
│  feedback-events    │
│  Partition: 0-11    │
└─────────────────────┘
           │
           │ Consume (Stream Processor)
           ▼
┌─────────────────────┐
│  Downstream System  │
│  - Aggregation      │
│  - Analysis         │
│  - Storage          │
└─────────────────────┘
```

---

## 4. Error Handling Flow

```
┌──────────────────────────────────────────────────────────────────┐
│                    Error Handling Strategy                       │
└──────────────────────────────────────────────────────────────────┘

Event Submission
    │
    ▼
┌─────────────────┐
│ Validate Event  │
└────────┬────────┘
         │
    ┌────┴────┐
    │ Valid?  │
    └────┬────┘
         │
    ┌────┴─────┬───────────┐
    │          │           │
   YES        NO          ERROR
    │          │           │
    ▼          ▼           ▼
┌────────┐ ┌─────────┐ ┌──────────┐
│ Buffer │ │ 400 Bad │ │ 500 Error│
│        │ │ Request │ │          │
└───┬────┘ └─────────┘ └──────────┘
    │
    ▼
┌─────────────────┐
│ Buffer Full?    │
└────────┬────────┘
         │
    ┌────┴────┐
    │         │
   NO        YES
    │         │
    │         ▼
    │    ┌─────────────┐
    │    │ 503 Service │
    │    │ Unavailable │
    │    └─────────────┘
    │
    ▼
┌─────────────────┐
│ Send to Kafka   │
└────────┬────────┘
         │
    ┌────┴────┐
    │ Success?│
    └────┬────┘
         │
    ┌────┴─────┬───────────┐
    │          │           │
   YES        NO        TIMEOUT
    │          │           │
    ▼          ▼           ▼
┌────────┐ ┌─────────┐ ┌──────────┐
│  OK    │ │ Retry?  │ │  Retry   │
│        │ └────┬────┘ │ (backoff)│
└────────┘      │      └──────────┘
                │
           ┌────┴─────┬───────────┐
           │          │           │
       Attempt 1  Attempt 2   Attempt 3
           │          │           │
           │          │           │
           ▼          ▼           ▼
        Retry      Retry    ┌──────────┐
        (1s)       (2s)     │   DLQ    │
                            │ (Failed) │
                            └──────────┘

[Parallel: Circuit Breaker]

┌─────────────────────┐
│ Circuit Breaker     │
│                     │
│ States:             │
│ ┌────────┐          │
│ │ Closed │          │
│ └───┬────┘          │
│     │               │
│ 5 failures          │
│     │               │
│     ▼               │
│ ┌────────┐          │
│ │  Open  │──────┐   │
│ └───┬────┘      │   │
│     │ 30s timeout  │
│     │           │   │
│     ▼           │   │
│ ┌─────────┐    │   │
│ │HalfOpen │    │   │
│ └───┬─────┘    │   │
│     │          │   │
│ Success    Failure │
│     │          │   │
│     ▼          ▼   │
│  Closed      Open  │
└─────────────────────┘
```

---

## 5. Deployment Patterns

### Pattern A: Sidecar

```
┌────────────────────────────────────────────┐
│          Kubernetes Pod                    │
├────────────────────────────────────────────┤
│                                            │
│  ┌──────────────────┐                      │
│  │  App Container   │                      │
│  │                  │                      │
│  │  Port: 8080      │                      │
│  │                  │                      │
│  └────────┬─────────┘                      │
│           │                                │
│           │ localhost:9090                 │
│           │ (< 1ms latency)                │
│           ▼                                │
│  ┌──────────────────┐                      │
│  │  Feedback        │                      │
│  │  Collector       │                      │
│  │  (Sidecar)       │                      │
│  │                  │                      │
│  │  Port: 9090      │                      │
│  │                  │                      │
│  └────────┬─────────┘                      │
│           │                                │
└───────────┼────────────────────────────────┘
            │
            │ Network
            ▼
    ┌───────────────┐
    │ Kafka Cluster │
    └───────────────┘

Benefits:
✓ Ultra-low latency (<1ms)
✓ Automatic scaling
✓ Isolated failure domain
✓ Simple service discovery
```

### Pattern B: Standalone Service

```
┌────────────┐  ┌────────────┐  ┌────────────┐
│  App Pod 1 │  │  App Pod 2 │  │  App Pod 3 │
└──────┬─────┘  └──────┬─────┘  └──────┬─────┘
       │               │               │
       │ HTTP/gRPC     │               │
       └───────┬───────┴───────┬───────┘
               │               │
               ▼               ▼
       ┌───────────────────────────────┐
       │  Feedback Collector Service   │
       │  (Kubernetes Service)          │
       └───────────────┬───────────────┘
                       │
       ┌───────────────┼───────────────┐
       │               │               │
       ▼               ▼               ▼
┌─────────────┐ ┌─────────────┐ ┌─────────────┐
│ Collector   │ │ Collector   │ │ Collector   │
│ Pod 1       │ │ Pod 2       │ │ Pod 3       │
└──────┬──────┘ └──────┬──────┘ └──────┬──────┘
       │               │               │
       └───────────────┼───────────────┘
                       │
                       ▼
               ┌───────────────┐
               │ Kafka Cluster │
               └───────────────┘

Benefits:
✓ Centralized management
✓ Independent scaling
✓ Shared resource pool
✓ Easier upgrades
```

### Pattern C: DaemonSet

```
┌─────────────────────────────────────────────┐
│              Kubernetes Cluster             │
├─────────────────────────────────────────────┤
│                                             │
│  Node 1          Node 2          Node 3     │
│  ┌──────┐        ┌──────┐        ┌──────┐  │
│  │ App  │        │ App  │        │ App  │  │
│  │ Pods │        │ Pods │        │ Pods │  │
│  └──┬───┘        └──┬───┘        └──┬───┘  │
│     │               │               │       │
│     │ Write to      │               │       │
│     │ /var/lib/     │               │       │
│     │ feedback/     │               │       │
│     ▼               ▼               ▼       │
│  ┌──────────┐   ┌──────────┐   ┌──────────┐│
│  │Collector │   │Collector │   │Collector ││
│  │Daemon    │   │Daemon    │   │Daemon    ││
│  │(Pod)     │   │(Pod)     │   │(Pod)     ││
│  └────┬─────┘   └────┬─────┘   └────┬─────┘│
│       │              │              │       │
└───────┼──────────────┼──────────────┼───────┘
        │              │              │
        └──────────────┼──────────────┘
                       │
                       ▼
               ┌───────────────┐
               │ Kafka Cluster │
               └───────────────┘

Benefits:
✓ Batch processing
✓ Lower overhead
✓ Resilient to failures
✓ Cost-optimized
```

---

## 6. Scalability Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                    Horizontal Scaling                            │
└──────────────────────────────────────────────────────────────────┘

Load Increase
    │
    │ Metrics: CPU > 70%, Buffer > 70%
    ▼
┌─────────────────────┐
│ HorizontalPodAuto-  │
│ scaler (HPA)        │
│ - Min: 3            │
│ - Max: 20           │
│ - Target: 70% CPU   │
└──────────┬──────────┘
           │
           │ Scale Up (+2 pods)
           ▼
┌─────────────────────────────────────────────────────┐
│  Feedback Collector Deployment                     │
├─────────────────────────────────────────────────────┤
│                                                     │
│  Before:                    After:                  │
│  ┌────┐ ┌────┐ ┌────┐      ┌────┐ ┌────┐ ┌────┐  │
│  │Pod1│ │Pod2│ │Pod3│  →   │Pod1│ │Pod2│ │Pod3│  │
│  └────┘ └────┘ └────┘      └────┘ └────┘ └────┘  │
│                              ┌────┐ ┌────┐         │
│                              │Pod4│ │Pod5│         │
│                              └────┘ └────┘         │
│                                                     │
└──────────────────┬──────────────────────────────────┘
                   │
                   │ All pods publish to
                   ▼
┌──────────────────────────────────────────────────────┐
│  Kafka Topic: feedback-events                        │
├──────────────────────────────────────────────────────┤
│  Partition 0  │  Partition 1  │ ... │ Partition 11  │
├───────────────┼───────────────┼─────┼───────────────┤
│  Pod 1,4      │  Pod 2,5      │ ... │  Pod 3        │
└──────────────────────────────────────────────────────┘

Note: Kafka partitioner distributes events across partitions
      Multiple pods can write to same partition (idempotent)
```

---

## 7. Observability Stack

```
┌──────────────────────────────────────────────────────────────────┐
│                    Observability Pipeline                        │
└──────────────────────────────────────────────────────────────────┘

┌─────────────────────┐
│ Feedback Collector  │
│                     │
│ - Metrics           │
│ - Traces            │
│ - Logs              │
└──────────┬──────────┘
           │
           │ OTLP/gRPC (port 4317)
           ▼
┌─────────────────────────────────────────────┐
│  OpenTelemetry Collector                    │
├─────────────────────────────────────────────┤
│  Receivers:                                 │
│  - OTLP (gRPC)                              │
│                                             │
│  Processors:                                │
│  - Batch                                    │
│  - Filter                                   │
│  - Resource Detection                       │
│                                             │
│  Exporters:                                 │
│  - Prometheus (metrics)                     │
│  - Jaeger (traces)                          │
│  - Loki (logs)                              │
└──────────┬────────────┬────────────┬────────┘
           │            │            │
     ┌─────┴─────┐ ┌────┴────┐ ┌────┴────┐
     │           │ │         │ │         │
     ▼           ▼ ▼         ▼ ▼         ▼
┌──────────┐ ┌──────────┐ ┌──────────┐
│Prometheus│ │  Jaeger  │ │   Loki   │
│(Metrics) │ │ (Traces) │ │  (Logs)  │
└────┬─────┘ └────┬─────┘ └────┬─────┘
     │            │            │
     │            │            │
     └────────────┼────────────┘
                  │
                  ▼
           ┌──────────────┐
           │   Grafana    │
           │  Dashboards  │
           │              │
           │ - System     │
           │ - Kafka      │
           │ - Business   │
           └──────────────┘
```

---

## 8. Security Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                    Security Layers                               │
└──────────────────────────────────────────────────────────────────┘

External Client
    │
    │ HTTPS (TLS 1.3)
    ▼
┌─────────────────────┐
│  API Gateway        │
│  - TLS termination  │
│  - Rate limiting    │
│  - WAF              │
└──────────┬──────────┘
           │
           │ JWT Token
           ▼
┌─────────────────────┐
│  Auth Middleware    │
│  - Validate JWT     │
│  - Check scopes     │
│  - Extract claims   │
└──────────┬──────────┘
           │
           │ Authenticated Request
           ▼
┌─────────────────────┐
│  Input Sanitizer    │
│  - PII redaction    │
│  - Length limits    │
│  - Type validation  │
└──────────┬──────────┘
           │
           │ Sanitized Event
           ▼
┌─────────────────────┐
│  Feedback Collector │
│  - Process event    │
│  - Audit log        │
└──────────┬──────────┘
           │
           │ TLS (mTLS optional)
           ▼
┌─────────────────────┐
│  Kafka Cluster      │
│  - SASL/SSL         │
│  - ACLs             │
│  - Encryption       │
└─────────────────────┘

Security Controls:
✓ TLS 1.3 for all network traffic
✓ JWT authentication with scopes
✓ PII redaction before storage
✓ Input validation and sanitization
✓ Rate limiting per client
✓ Audit logging for all operations
✓ Kafka SASL/SSL authentication
✓ Network policies (Kubernetes)
✓ RBAC for API access
✓ Secrets management (Vault/K8s)
```

---

## 9. Disaster Recovery

```
┌──────────────────────────────────────────────────────────────────┐
│                    Failure Scenarios & Recovery                  │
└──────────────────────────────────────────────────────────────────┘

Scenario 1: Kafka Cluster Down
    │
    │ Detect: Circuit breaker opens
    ▼
┌─────────────────────┐
│ Circuit Breaker     │
│ State: OPEN         │
└──────────┬──────────┘
           │
           │ Fallback Activated
           ▼
┌─────────────────────┐
│ Local Storage (DLQ) │
│ - Write to disk     │
│ - WAL format        │
└──────────┬──────────┘
           │
           │ Kafka Recovers
           ▼
┌─────────────────────┐
│ Recovery Process    │
│ - Replay from DLQ   │
│ - Resume normal ops │
└─────────────────────┘

Scenario 2: Pod Crashes
    │
    │ Detect: Liveness probe fails
    ▼
┌─────────────────────┐
│ Kubernetes          │
│ - Restart pod       │
│ - Preserve PVC      │
└──────────┬──────────┘
           │
           │ Pod Restarts
           ▼
┌─────────────────────┐
│ Startup Sequence    │
│ 1. Load config      │
│ 2. Init telemetry   │
│ 3. Connect Kafka    │
│ 4. Accept traffic   │
└─────────────────────┘

Scenario 3: Zone Failure
    │
    │ AZ-1 goes down
    ▼
┌─────────────────────────────────────┐
│ Multi-AZ Deployment                 │
│                                     │
│ AZ-1 (DOWN)  AZ-2 (UP)  AZ-3 (UP)  │
│   Pod 1,2      Pod 3,4    Pod 5,6  │
│     ✗ ✗         ✓  ✓      ✓  ✓    │
└──────────────────┬──────────────────┘
                   │
                   │ Load redistributed
                   ▼
           ┌───────────────┐
           │ Remaining Pods │
           │ Handle 100%    │
           │ traffic        │
           └───────────────┘

Recovery Time:
- Pod restart: <30 seconds
- Zone failover: <60 seconds
- Data loss: Zero (Kafka replication)
```

---

## 10. Performance Optimization

```
┌──────────────────────────────────────────────────────────────────┐
│                    Performance Optimization                      │
└──────────────────────────────────────────────────────────────────┘

Optimization Layers:

┌─────────────────────────────────────────────┐
│ Layer 1: Network                            │
│ - HTTP/2 (multiplexing)                     │
│ - gRPC streaming                            │
│ - Connection pooling                        │
│ Improvement: 50% latency reduction          │
└─────────────────────────────────────────────┘

┌─────────────────────────────────────────────┐
│ Layer 2: Serialization                      │
│ - Zero-copy serialization                   │
│ - Buffer pooling                            │
│ - Optimized JSON encoder                    │
│ Improvement: 40% CPU reduction              │
└─────────────────────────────────────────────┘

┌─────────────────────────────────────────────┐
│ Layer 3: Batching                           │
│ - Adaptive batch sizing                     │
│ - Dynamic time windows                      │
│ - Load-based optimization                   │
│ Improvement: 10x throughput increase        │
└─────────────────────────────────────────────┘

┌─────────────────────────────────────────────┐
│ Layer 4: Compression                        │
│ - Gzip compression (level 6)                │
│ - Payload size reduction                    │
│ - Network bandwidth saving                  │
│ Improvement: 60-80% size reduction          │
└─────────────────────────────────────────────┘

┌─────────────────────────────────────────────┐
│ Layer 5: Concurrency                        │
│ - Async I/O (Tokio)                         │
│ - Thread pool for CPU tasks                 │
│ - Lock-free data structures                 │
│ Improvement: 5x concurrent throughput       │
└─────────────────────────────────────────────┘

Performance Metrics:
┌──────────────┬─────────┬──────────┬─────────┐
│ Metric       │ Before  │ After    │ Gain    │
├──────────────┼─────────┼──────────┼─────────┤
│ Throughput   │ 2K/sec  │ 10K/sec  │ 5x      │
│ Latency p99  │ 50ms    │ 10ms     │ 80%     │
│ CPU Usage    │ 80%     │ 30%      │ 62%     │
│ Memory       │ 2GB     │ 512MB    │ 75%     │
│ Network      │ 50Mbps  │ 10Mbps   │ 80%     │
└──────────────┴─────────┴──────────┴─────────┘
```

---

**End of Diagrams Document**

**Related Documents**:
- [FEEDBACK_COLLECTOR_ARCHITECTURE.md](./FEEDBACK_COLLECTOR_ARCHITECTURE.md) - Full architecture
- [FEEDBACK_COLLECTOR_ARCHITECTURE_SUMMARY.md](./FEEDBACK_COLLECTOR_ARCHITECTURE_SUMMARY.md) - Executive summary

**Diagram Formats**:
- All diagrams are ASCII art for easy version control
- Can be rendered with tools like Monodraw, ASCIIFlow
- Easily editable in any text editor
