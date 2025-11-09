# Stream Processor Architecture Diagrams

Visual diagrams to accompany the Stream Processor architecture design.

---

## System Context Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          External Systems                                │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                           │
│   ┌─────────────┐         ┌─────────────┐         ┌─────────────┐      │
│   │    LLM      │         │     LLM     │         │     LLM     │      │
│   │ Observatory │────────▶│  Collector  │────────▶│   Kafka     │      │
│   │  (Metrics)  │         │             │         │   Cluster   │      │
│   └─────────────┘         └─────────────┘         └─────────────┘      │
│                                                            │              │
│   ┌─────────────┐                                         │              │
│   │     LLM     │                                         │              │
│   │   Sentinel  │─────────────────────────────────────────┘              │
│   │  (Anomaly)  │                                                        │
│   └─────────────┘                                                        │
│                                                                           │
└───────────────────────────────────────────┬───────────────────────────────┘
                                            │
                                            │ Events
                                            │
                                            ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                        Stream Processor                                  │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                           │
│  ┌───────────────────────────────────────────────────────────────┐     │
│  │                    Processing Pipeline                          │     │
│  │                                                                 │     │
│  │  Events → Windows → Aggregation → State → Results             │     │
│  │                                                                 │     │
│  └───────────────────────────────────────────────────────────────┘     │
│                                                                           │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐              │
│  │   RocksDB    │    │    Memory    │    │  Checkpoint  │              │
│  │    State     │    │    Buffer    │    │   Storage    │              │
│  └──────────────┘    └──────────────┘    └──────────────┘              │
│                                                                           │
└───────────────────────────────────────────┬───────────────────────────────┘
                                            │
                                            │ Aggregated Results
                                            │
                                            ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                        Downstream Consumers                              │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                           │
│   ┌─────────────┐         ┌─────────────┐         ┌─────────────┐      │
│   │   Analyzer  │         │   Decision  │         │  Dashboard  │      │
│   │   Engine    │         │   Engine    │         │   (Grafana) │      │
│   └─────────────┘         └─────────────┘         └─────────────┘      │
│                                                                           │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Processing Pipeline Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         Event Processing Flow                            │
└─────────────────────────────────────────────────────────────────────────┘

   Kafka Topic
       │
       │ Consume
       ▼
┌──────────────┐
│    Event     │  { id, timestamp, data, metadata }
│  Ingestion   │
└──────────────┘
       │
       │ Extract Timestamp
       ▼
┌──────────────┐
│  Timestamp   │  Event Time: 2025-01-01 12:34:56
│  Extraction  │  Processing Time: 2025-01-01 12:35:02
└──────────────┘
       │
       │ Generate Watermark
       ▼
┌──────────────┐
│  Watermark   │  Watermark: Event Time - 30s
│  Generator   │  Current: 2025-01-01 12:34:26
└──────────────┘
       │
       │ Check Late Arrival
       ├─────────────────────────────┐
       │                             │
       ▼                             ▼
┌──────────────┐            ┌──────────────┐
│  On-Time     │            │    Late      │
│   Event      │            │    Event     │
└──────────────┘            └──────────────┘
       │                             │
       │ Assign Windows              │ Check Lateness
       ▼                             ▼
┌──────────────┐            ┌──────────────┐
│   Window     │            │   Allowed?   │
│  Assigner    │            │   Yes/No     │
└──────────────┘            └──────────────┘
       │                             │
       │                             ├─────── No ──▶ Dead Letter Queue
       │                             │
       │                             └─────── Yes
       │                                      │
       │◀─────────────────────────────────────┘
       │
       │ Windows: [W1, W2, W3]
       ▼
┌──────────────┐
│  For Each    │
│   Window     │  ┌─────────────┐
└──────────────┘  │  W1: 12:30  │
       │          │  W2: 12:35  │
       │          │  W3: 12:40  │
       │          └─────────────┘
       │ Get State
       ▼
┌──────────────┐
│    State     │  Load accumulator from RocksDB
│   Backend    │  or create new if not exists
└──────────────┘
       │
       │ Update Accumulator
       ▼
┌──────────────┐
│ Aggregation  │  count++, sum+=value, min/max, etc.
│    Engine    │  Update percentiles, stddev
└──────────────┘
       │
       │ Save State
       ▼
┌──────────────┐
│    State     │  Persist updated accumulator
│   Backend    │  to RocksDB
└──────────────┘
       │
       │ Check Trigger
       ▼
┌──────────────┐
│   Window     │  Should window fire?
│   Trigger    │  Watermark >= Window.end?
└──────────────┘
       │
       ├─────────────────────────────┐
       │                             │
       ▼                             ▼
┌──────────────┐            ┌──────────────┐
│  Window Not  │            │   Window     │
│    Ready     │            │    Ready     │
└──────────────┘            └──────────────┘
       │                             │
       │ Wait for more events        │ Emit Results
       │                             ▼
       │                    ┌──────────────┐
       │                    │   Result     │  {
       │                    │  Formatter   │    window,
       │                    └──────────────┘    results: {
       │                             │            count, sum, avg,
       │                             │            min, max, p50,
       │                             │            p95, p99, stddev
       │                             │          }
       │                             │        }
       │                             ▼
       │                    ┌──────────────┐
       │                    │   Output     │
       │                    │    Sinks     │
       │                    └──────────────┘
       │                             │
       │                             ├────▶ Kafka Topic
       │                             │
       │                             ├────▶ Database
       │                             │
       │                             └────▶ Metrics/Dashboard
       │
       │ Purge Old State
       ▼
┌──────────────┐
│   Cleanup    │  Delete windows beyond allowed lateness
│   Manager    │  Free memory, checkpoint state
└──────────────┘
```

---

## Window Types Visualization

### Tumbling Windows

```
Timeline: ────────────────────────────────────────────────▶

Events:   •    •  •      •  •     •        •    •
          1    2  3      4  5     6        7    8

Windows:  [──────────]
               W1

                    [──────────]
                         W2

                                [──────────]
                                     W3

Properties:
- Fixed size (e.g., 5 minutes)
- Non-overlapping
- Each event belongs to exactly ONE window
- Simple, efficient

Use case: Hourly/daily aggregations
```

### Sliding Windows

```
Timeline: ────────────────────────────────────────────────▶

Events:   •    •  •      •  •     •        •    •
          1    2  3      4  5     6        7    8

Windows:  [──────────]
               W1

            [──────────]
                W2

               [──────────]
                   W3

                  [──────────]
                      W4

                     [──────────]
                         W5

Properties:
- Fixed size (e.g., 10 minutes)
- Fixed slide (e.g., 1 minute)
- Overlapping
- Each event belongs to MULTIPLE windows
- Higher memory usage

Use case: Moving averages, rolling statistics
```

### Session Windows

```
Timeline: ────────────────────────────────────────────────▶

Events:   •  • •              •  •            •
          1  2 3              4  5            6

                gap                  gap
                ◀──▶                ◀──▶

Windows:  [─────────]        [─────────]     [────]
           Session 1          Session 2       S3

Properties:
- Dynamic size based on activity
- Gap-based (e.g., 5-minute inactivity)
- Non-overlapping
- Windows merge if events arrive within gap
- Variable window sizes

Use case: User sessions, click streams
```

---

## Watermark and Late Events

```
Event Time
    ▲
    │                                           • Late Event (dropped)
    │                                          /
12:36│                              • On-time /
    │                             /          /
12:35│                 • On-time /          /
    │                /          /          /
12:34│    • On-time /          /          /
    │            /   /         /          /
12:33│          /   /         /  • Late Event (accepted)
    │         /   /         /   /
12:32│        /   /         /  /
    │       /   /         /  /
12:31│      /   /         /  /
    │     /   /         /  /         Watermark = Event Time - 30s
12:30│────/───/─────────/──/──────────────────────────────────
    │   /   /         /  /
    │  /   /    ┌────/──/─┐  Allowed Lateness (60s)
    │ /   /     │    /  /  │
    │/   /      │   /  /   │
    ├───/───────┼──/──/────┼──────────────────────────────────
    │  /        │ /  /     │
    │ /         │/  /      │  Drop Zone
    │/          └──/───────┘
    │              /
    │             /  Window.end
    │            /
    │───────────/─────────────────────────────▶ Processing Time
             Window closes
           (Watermark passes)

Legend:
- Watermark: Event Time - Max Out-of-Orderness
- On-time: Event Time > Watermark
- Late (accepted): Window.end < Event Time < Watermark + Allowed Lateness
- Late (dropped): Event Time < Watermark - Allowed Lateness
```

---

## State Management Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          State Management                                │
└─────────────────────────────────────────────────────────────────────────┘

                        ┌────────────────────┐
                        │  Window Operator   │
                        └────────────────────┘
                                  │
                    ┌─────────────┼─────────────┐
                    │             │             │
                    ▼             ▼             ▼
           ┌────────────┐ ┌────────────┐ ┌────────────┐
           │  Window 1  │ │  Window 2  │ │  Window 3  │
           │ State Mgr  │ │ State Mgr  │ │ State Mgr  │
           └────────────┘ └────────────┘ └────────────┘
                    │             │             │
                    └─────────────┼─────────────┘
                                  │
                                  ▼
                    ┌──────────────────────────┐
                    │   State Backend (API)    │
                    │                          │
                    │  get(key) → value        │
                    │  put(key, value)         │
                    │  delete(key)             │
                    │  checkpoint(id)          │
                    │  restore(id)             │
                    └──────────────────────────┘
                                  │
                    ┌─────────────┼─────────────┐
                    │             │             │
                    ▼             ▼             ▼
           ┌────────────┐ ┌────────────┐ ┌────────────┐
           │   Memory   │ │  RocksDB   │ │   Redis    │
           │  Backend   │ │  Backend   │ │  Backend   │
           └────────────┘ └────────────┘ └────────────┘
                    │             │             │
                    │             │             │
                    ▼             ▼             ▼
           ┌────────────┐ ┌────────────┐ ┌────────────┐
           │ HashMap    │ │ Disk (LSM) │ │  Network   │
           └────────────┘ └────────────┘ └────────────┘


State Key Structure:
  namespace:grouping_key:window_id
     │          │            │
     │          │            └─── Window start_end timestamp
     │          └────────────────  e.g., "model=claude:region=us"
     └───────────────────────────  e.g., "latency_metrics"

Example:
  "latency_metrics:model=claude-opus:1704110400000_1704110700000"


State Value (Serialized AggregationState):
  {
    "window": { "start": ..., "end": ... },
    "count": 1234,
    "sum": 45678.9,
    "min": 10.5,
    "max": 500.2,
    "percentiles": [10.5, 20.3, ...],
    ...
  }
```

---

## Checkpoint and Recovery Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      Checkpoint Process                                  │
└─────────────────────────────────────────────────────────────────────────┘

Normal Processing
       │
       │ Checkpoint Timer (every 60s)
       ▼
┌──────────────┐
│   Trigger    │
│  Checkpoint  │
└──────────────┘
       │
       │ Barrier event
       ▼
┌──────────────┐
│   Pause New  │  Continue processing in-flight events
│   Events     │  until all are complete
└──────────────┘
       │
       │ Flush State
       ▼
┌──────────────┐
│  Checkpoint  │  State Backend:
│    State     │  - Snapshot current state
│   Backend    │  - Write to checkpoint directory
└──────────────┘  - Create metadata file
       │
       │ Checkpoint ID
       ▼
┌──────────────┐
│    Kafka     │  Commit offsets to checkpoint
│   Offsets    │  Associate with checkpoint ID
└──────────────┘
       │
       │ Success
       ▼
┌──────────────┐
│   Resume     │  Continue normal processing
│ Processing   │
└──────────────┘


┌─────────────────────────────────────────────────────────────────────────┐
│                      Recovery Process                                    │
└─────────────────────────────────────────────────────────────────────────┘

System Start/Crash
       │
       │ Find Latest Checkpoint
       ▼
┌──────────────┐
│     Load     │  Read checkpoint metadata
│  Checkpoint  │  checkpoint_id, timestamp, offsets
│   Metadata   │
└──────────────┘
       │
       │ Checkpoint ID
       ▼
┌──────────────┐
│   Restore    │  State Backend:
│    State     │  - Load state snapshot
│   Backend    │  - Restore to memory/RocksDB
└──────────────┘
       │
       │ Kafka Offsets
       ▼
┌──────────────┐
│    Reset     │  Seek to checkpointed offsets
│    Kafka     │  for each partition
│   Consumer   │
└──────────────┘
       │
       │ Ready
       ▼
┌──────────────┐
│    Resume    │  Start consuming from checkpoint
│  Processing  │  Replay events since checkpoint
└──────────────┘


Checkpoint Directory Structure:
  /var/lib/optimizer/checkpoints/
  ├── checkpoint-1/
  │   ├── metadata.json     # { id, timestamp, offsets }
  │   └── state/            # RocksDB snapshot
  ├── checkpoint-2/
  │   ├── metadata.json
  │   └── state/
  └── checkpoint-3/
      ├── metadata.json
      └── state/
```

---

## Aggregation Engine Details

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Aggregation State Update                              │
└─────────────────────────────────────────────────────────────────────────┘

Event: { value: 42.5 }
       │
       ▼
┌──────────────────────────────────────────────────────────────────────┐
│  AggregationState                                                     │
│                                                                       │
│  ┌────────────────────┐                                              │
│  │  Count Aggregator  │   count++                                    │
│  │                    │   count: 999 → 1000                          │
│  └────────────────────┘                                              │
│           │                                                           │
│           ▼                                                           │
│  ┌────────────────────┐                                              │
│  │   Sum Aggregator   │   sum += value                               │
│  │                    │   sum: 41957.5 → 42000.0                     │
│  └────────────────────┘                                              │
│           │                                                           │
│           ▼                                                           │
│  ┌────────────────────┐                                              │
│  │ Average Aggregator │   sum += value, count++                      │
│  │                    │   avg: 42.0 → 42.0                           │
│  └────────────────────┘                                              │
│           │                                                           │
│           ▼                                                           │
│  ┌────────────────────┐                                              │
│  │  Min/Max Agg       │   min = min(min, value)                      │
│  │                    │   min: 10.0 → 10.0                           │
│  │                    │   max = max(max, value)                      │
│  │                    │   max: 500.0 → 500.0                         │
│  └────────────────────┘                                              │
│           │                                                           │
│           ▼                                                           │
│  ┌────────────────────┐                                              │
│  │Percentile Agg      │   values.push(value)                         │
│  │                    │   values: [..., 42.5]                        │
│  │                    │   sorted: false                              │
│  └────────────────────┘                                              │
│           │                                                           │
│           ▼                                                           │
│  ┌────────────────────┐                                              │
│  │  StdDev Agg        │   sum_of_squares += value²                   │
│  │                    │   sum_of_squares: ... → ... + 1806.25        │
│  └────────────────────┘                                              │
│                                                                       │
└───────────────────────────────────────────────────────────────────────┘

Result Computation (when window fires):
  {
    "count": 1000,
    "sum": 42000.0,
    "average": 42.0,
    "min": 10.0,
    "max": 500.0,
    "p50": 40.5,     # Computed from sorted values
    "p95": 95.2,
    "p99": 99.8,
    "stddev": 15.7   # sqrt((sum_of_squares / count) - (mean²))
  }
```

---

## Parallelism and Partitioning

```
┌─────────────────────────────────────────────────────────────────────────┐
│                  Kafka Partitioning Strategy                             │
└─────────────────────────────────────────────────────────────────────────┘

Kafka Topic: feedback-events
Partitions: 8

   P0   P1   P2   P3   P4   P5   P6   P7
   │    │    │    │    │    │    │    │
   │    │    │    │    │    │    │    │  Events partitioned by key
   └────┴────┴────┴────┴────┴────┴────┘  (model, region, etc.)
              │
              │ Consumer Group: processor-group
              │
   ┌──────────┼──────────┐
   │          │          │
   ▼          ▼          ▼
┌────┐    ┌────┐    ┌────┐
│ C1 │    │ C2 │    │ C3 │  Consumer Instances
└────┘    └────┘    └────┘  (processor parallelism=3)
  │          │          │
  │          │          │   Partition Assignment:
  │          │          │   C1: P0, P1, P2
  │          │          │   C2: P3, P4, P5
  │          │          │   C3: P6, P7
  │          │          │
  ▼          ▼          ▼
┌────┐    ┌────┐    ┌────┐
│ W1 │    │ W2 │    │ W3 │  Window Operators
│    │    │    │    │    │  (isolated state per consumer)
└────┘    └────┘    └────┘


State Partitioning:

RocksDB State:
  /var/lib/optimizer/state/
  ├── partition-0/     # State for C1
  ├── partition-1/
  ├── partition-2/
  ├── partition-3/     # State for C2
  ├── partition-4/
  ├── partition-5/
  ├── partition-6/     # State for C3
  └── partition-7/

Each consumer only manages state for its assigned partitions.
No coordination needed between consumers.
```

---

## Error Handling Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      Error Handling Decision Tree                        │
└─────────────────────────────────────────────────────────────────────────┘

Event Processing Error
       │
       ▼
┌──────────────┐
│   Classify   │
│    Error     │
└──────────────┘
       │
       ├─── Transient Error (network, timeout)
       │         │
       │         ▼
       │    ┌──────────────┐
       │    │  Retry with  │
       │    │  Backoff     │
       │    └──────────────┘
       │         │
       │         ├─── Retry 1 (100ms delay)
       │         │
       │         ├─── Retry 2 (200ms delay)
       │         │
       │         ├─── Retry 3 (400ms delay)
       │         │
       │         ▼
       │    Max Retries?
       │         │
       │         ├─── No ──▶ Continue Retrying
       │         │
       │         └─── Yes
       │              │
       │              ▼
       │         Dead Letter Queue
       │
       │
       ├─── Permanent Error (serialization, validation)
       │         │
       │         ▼
       │    ┌──────────────┐
       │    │     Log      │
       │    │    Error     │
       │    └──────────────┘
       │         │
       │         ▼
       │    Dead Letter Queue
       │
       │
       └─── Critical Error (state corruption, OOM)
                 │
                 ▼
            ┌──────────────┐
            │   Shutdown   │
            │   Graceful   │
            └──────────────┘
                 │
                 ▼
            Restart & Recover


Dead Letter Queue Structure:
  {
    "original_event": { ... },
    "error": "Deserialization failed: ...",
    "retry_count": 3,
    "timestamp": "2025-01-01T12:34:56Z",
    "processor_instance": "processor-1",
    "partition": 5,
    "offset": 12345
  }
```

---

## Deployment Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Kubernetes Deployment                                 │
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│  Namespace: llm-optimizer                                                │
│                                                                           │
│  ┌────────────────────────────────────────────────────────────────┐     │
│  │  StatefulSet: stream-processor                                 │     │
│  │                                                                 │     │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐               │     │
│  │  │   Pod-0    │  │   Pod-1    │  │   Pod-2    │               │     │
│  │  │            │  │            │  │            │               │     │
│  │  │ Processor  │  │ Processor  │  │ Processor  │               │     │
│  │  │ Container  │  │ Container  │  │ Container  │               │     │
│  │  │            │  │            │  │            │               │     │
│  │  │ CPU: 1000m │  │ CPU: 1000m │  │ CPU: 1000m │               │     │
│  │  │ Mem: 2Gi   │  │ Mem: 2Gi   │  │ Mem: 2Gi   │               │     │
│  │  │            │  │            │  │            │               │     │
│  │  │ PVC: state │  │ PVC: state │  │ PVC: state │               │     │
│  │  └────────────┘  └────────────┘  └────────────┘               │     │
│  │       │               │               │                        │     │
│  └───────┼───────────────┼───────────────┼────────────────────────┘     │
│          │               │               │                              │
│          │               │               │                              │
└──────────┼───────────────┼───────────────┼──────────────────────────────┘
           │               │               │
           │               │               │  Network
           │               │               │
┌──────────┼───────────────┼───────────────┼──────────────────────────────┐
│          │               │               │                              │
│  ┌───────▼───────────────▼───────────────▼─────────┐                    │
│  │           Kafka Service (Headless)              │                    │
│  │           kafka.kafka.svc.cluster.local         │                    │
│  └──────────────────────────────────────────────────┘                    │
│                                                                           │
│  ┌──────────────────────────────────────────────────┐                    │
│  │         Prometheus Service                       │                    │
│  │         prometheus.monitoring.svc.cluster.local  │                    │
│  └──────────────────────────────────────────────────┘                    │
│                                                                           │
└───────────────────────────────────────────────────────────────────────────┘


Persistent Volumes:

PVC: state-processor-0
  ├── state/              # RocksDB state
  └── checkpoints/        # Checkpoints

PVC: state-processor-1
  ├── state/
  └── checkpoints/

PVC: state-processor-2
  ├── state/
  └── checkpoints/
```

---

## Metrics and Monitoring

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      Metrics Architecture                                │
└─────────────────────────────────────────────────────────────────────────┘

Stream Processor
       │
       │ Collect Metrics
       ▼
┌──────────────────────────────────────────────────┐
│  ProcessorMetrics                                 │
│                                                   │
│  • events_processed_total (counter)              │
│  • processing_latency_seconds (histogram)        │
│  • kafka_consumer_lag (gauge)                    │
│  • window_count (gauge)                          │
│  • state_size_bytes (gauge)                      │
│  • checkpoint_duration_seconds (histogram)       │
│  • errors_total (counter)                        │
│  • late_events_total (counter)                   │
│  • watermark_lag_seconds (gauge)                 │
└──────────────────────────────────────────────────┘
       │
       │ Export (Prometheus format)
       ▼
┌──────────────────────────────────────────────────┐
│  Prometheus Server                                │
│  - Scrape /metrics endpoint every 15s            │
│  - Store time-series data                        │
│  - Retention: 15 days                            │
└──────────────────────────────────────────────────┘
       │
       │ Query
       ▼
┌──────────────────────────────────────────────────┐
│  Grafana Dashboard                                │
│                                                   │
│  Panel 1: Events/sec (rate)                      │
│  Panel 2: Processing Latency (p50, p95, p99)    │
│  Panel 3: Consumer Lag                           │
│  Panel 4: State Size                             │
│  Panel 5: Error Rate                             │
│  Panel 6: Window Count                           │
└──────────────────────────────────────────────────┘
       │
       │ Alerts
       ▼
┌──────────────────────────────────────────────────┐
│  Alertmanager                                     │
│                                                   │
│  • Consumer lag > 10,000                         │
│  • p99 latency > 1s                              │
│  • Error rate > 1%                               │
│  • State size > 10GB                             │
└──────────────────────────────────────────────────┘
       │
       │ Notify
       ▼
┌──────────────────────────────────────────────────┐
│  Slack / PagerDuty / Email                       │
└──────────────────────────────────────────────────┘
```

---

## Summary

These diagrams illustrate the complete architecture of the Stream Processor:

1. **System Context**: Integration with external systems
2. **Processing Flow**: Step-by-step event processing
3. **Window Types**: Visual representation of windowing strategies
4. **Watermarking**: Late event handling mechanism
5. **State Management**: State storage and organization
6. **Checkpointing**: Fault tolerance and recovery
7. **Aggregation**: Multi-metric computation
8. **Parallelism**: Kafka partitioning and consumer scaling
9. **Error Handling**: Error classification and recovery
10. **Deployment**: Kubernetes architecture
11. **Monitoring**: Metrics collection and alerting

These diagrams should be used in conjunction with the architecture design document for a complete understanding of the system.
