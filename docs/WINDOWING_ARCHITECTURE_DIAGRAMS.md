# Stream Processing Windowing - Architecture Diagrams

> Visual reference for [STREAM_PROCESSING_WINDOWING_SPECIFICATION.md](./STREAM_PROCESSING_WINDOWING_SPECIFICATION.md)

## System Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     LLM Auto-Optimizer Stream Processor                 │
└─────────────────────────────────────────────────────────────────────────┘

                              ┌──────────────┐
                              │    Kafka     │
                              │   Topics     │
                              └──────┬───────┘
                                     │
                        ┌────────────┼────────────┐
                        │            │            │
                   ┌────▼───┐   ┌───▼────┐  ┌───▼────┐
                   │ Perf   │   │ User   │  │ System │
                   │Metrics │   │Feedback│  │ Health │
                   └────┬───┘   └───┬────┘  └───┬────┘
                        │            │            │
                        └────────────┼────────────┘
                                     │
                              ┌──────▼───────┐
                              │   Windowing   │
                              │   Operator    │
                              └──────┬───────┘
                                     │
                        ┌────────────┼────────────┐
                        │            │            │
                   ┌────▼───┐   ┌───▼────┐  ┌───▼────┐
                   │Tumbling│   │Session │  │Sliding │
                   │Windows │   │Windows │  │Windows │
                   └────┬───┘   └───┬────┘  └───┬────┘
                        │            │            │
                        └────────────┼────────────┘
                                     │
                              ┌──────▼───────┐
                              │  Aggregator   │
                              │   Operator    │
                              └──────┬───────┘
                                     │
                        ┌────────────┼────────────┐
                        │            │            │
                   ┌────▼───┐   ┌───▼────┐  ┌───▼────┐
                   │Storage │   │  API   │  │ Stream │
                   │ Layer  │   │Output  │  │ Output │
                   └────────┘   └────────┘  └────────┘
```

## Event Flow with Watermarks

```
Time ─────────────────────────────────────────────────────────────▶

Events:     E1      E2           E3    E4         E5
            │       │            │     │          │
Timestamp:  10:00   10:01        10:03 10:02      10:06
            │       │            │     │          │
            ▼       ▼            ▼     ▼          ▼
         ┌──────────────────────────────────────────────┐
         │         Event Stream (out of order)          │
         └──────────────────────────────────────────────┘
                             │
                             │ Event Time Assignment
                             ▼
         ┌──────────────────────────────────────────────┐
         │  E1    E2    E4    E3              E5        │
         │ 10:00 10:01 10:02 10:03           10:06      │
         └──────────────────────────────────────────────┘
                             │
                             │ Watermark Generation
                             ▼
Watermark:     9:58  9:59  10:00  10:01       10:04
               │     │     │      │           │
               │     │     │      │           └─ Window [10:00-10:05) CLOSES
               │     │     │      └─ E3 is late but within allowed lateness
               │     │     └─ Window [10:00-10:05) still open
               │     └─ E1, E2 assigned to window [10:00-10:05)
               └─ max_out_of_order_delay = 2 minutes

Windows:
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│  [09:55-10:00)  │  │  [10:00-10:05)  │  │  [10:05-10:10)  │
│    CLOSED       │  │     OPEN        │  │     OPEN        │
└─────────────────┘  └─────────────────┘  └─────────────────┘
                          ▲  ▲  ▲
                          │  │  │
                          E1 E2 E4,E3              E5 ─────▶
```

## Window Types Comparison

```
┌─────────────────────────────────────────────────────────────┐
│                    TUMBLING WINDOWS                         │
│  Fixed-size, non-overlapping, aligned                      │
└─────────────────────────────────────────────────────────────┘

Timeline:  10:00      10:05      10:10      10:15      10:20
           ├──────────┼──────────┼──────────┼──────────┤
           │ Window 1 │ Window 2 │ Window 3 │ Window 4 │
           └──────────┴──────────┴──────────┴──────────┘

Use Case: Performance metrics aggregation
Example:  Compute p95 latency every 5 minutes


┌─────────────────────────────────────────────────────────────┐
│                    SLIDING WINDOWS                          │
│  Fixed-size, overlapping, sliding interval                 │
└─────────────────────────────────────────────────────────────┘

Timeline:  10:00      10:05      10:10      10:15      10:20
           ├──────────┼──────────┼──────────┼──────────┤
           │   Window 1 (15 min)   │
           │          │   Window 2 (15 min)   │
           │          │          │   Window 3 (15 min)   │
           └──────────┴──────────┴──────────┴──────────┘
                       ▲          ▲          ▲
                    Slide 5m   Slide 5m   Slide 5m

Use Case: Anomaly detection with smooth updates
Example:  Monitor error rate over last 15 minutes, update every 1 min


┌─────────────────────────────────────────────────────────────┐
│                    SESSION WINDOWS                          │
│  Variable-size, non-overlapping, gap-based                 │
└─────────────────────────────────────────────────────────────┘

Timeline:  Events
           ▼    ▼  ▼        ▼  ▼                    ▼    ▼
           │────────│        │────│                  │────│
           │Session1│        │S2  │                  │S3  │
           └────────┘        └────┘                  └────┘
              ▲                 ▲                       ▲
              │                 │                       │
           30min gap        30min gap              30min gap

Use Case: User feedback sessions
Example:  Aggregate all user actions in a session (30min inactivity gap)
```

## Watermark Propagation Architecture

```
┌────────────────────────────────────────────────────────────────┐
│                   Kafka Partitions                             │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐       │
│  │  Part 0  │  │  Part 1  │  │  Part 2  │  │  Part 3  │       │
│  │ t=10:05  │  │ t=10:04  │  │ t=10:07  │  │ t=10:03  │       │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘       │
└───────┼─────────────┼─────────────┼─────────────┼──────────────┘
        │             │             │             │
        │             │             │             │
        ▼             ▼             ▼             ▼
┌─────────────────────────────────────────────────────────────────┐
│              Per-Partition Watermark Tracker                    │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │  partition_times = {                                     │   │
│  │    0: 10:05,                                             │   │
│  │    1: 10:04,                                             │   │
│  │    2: 10:07,                                             │   │
│  │    3: 10:03  <── min timestamp                           │   │
│  │  }                                                       │   │
│  └──────────────────────────────────────────────────────────┘   │
│                           │                                     │
│                           ▼                                     │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │  watermark = min(partition_times) - max_delay           │   │
│  │             = 10:03 - 2min                               │   │
│  │             = 10:01                                      │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Window Operator                              │
│  - Close windows with end <= 10:01                             │
│  - Emit final results                                          │
│  - Accept late data until watermark + allowed_lateness         │
└─────────────────────────────────────────────────────────────────┘
```

## State Storage Architecture

```
┌───────────────────────────────────────────────────────────────────┐
│                     Window State Manager                          │
└───────────────────────────────────────────────────────────────────┘
                                │
                    ┌───────────┼──────────┐
                    │           │          │
                    ▼           ▼          ▼
        ┌─────────────┐  ┌──────────┐  ┌──────────┐
        │   L1: HOT   │  │ L2: WARM │  │ L3: COLD │
        │  (In-Memory)│  │  (Sled)  │  │(Postgres)│
        └─────────────┘  └──────────┘  └──────────┘
        │ 512 MB      │  │ 1 GB     │  │ 90 days  │
        │ 1 hour TTL  │  │ 7 day TTL│  │ retention│
        └─────────────┘  └──────────┘  └──────────┘

Access Pattern:
┌─────────────────────────────────────────────────────────────┐
│ 1. get(window_id)                                           │
│    ├─ Check L1 (hot) ────────────────────────▶ HIT (fast)  │
│    ├─ If miss, check L2 (warm) ──────────────▶ HIT (slow)  │
│    └─ If miss, check L3 (cold) ──────────────▶ HIT (slower)│
│                                                             │
│ 2. put(window_id, state)                                    │
│    ├─ Write to L1 (always) ──────────────────▶ Immediate   │
│    ├─ Async write to L2 (background) ────────▶ Eventually  │
│    └─ Flush to L3 on TTL expiry ─────────────▶ Periodic    │
│                                                             │
│ 3. Eviction Policy                                          │
│    ├─ L1: LRU when >512MB ───────────────────▶ Move to L2  │
│    ├─ L2: TTL 7 days ────────────────────────▶ Move to L3  │
│    └─ L3: TTL 90 days ───────────────────────▶ Delete      │
└─────────────────────────────────────────────────────────────┘

State Size Estimation:
┌────────────────────────────────────────────────────┐
│ Per-window state size: ~5 KB                       │
│ - Window metadata: 100 bytes                       │
│ - Aggregated metrics: 200 bytes                    │
│ - Event samples: ~4.7 KB (for percentiles)         │
│                                                    │
│ Active windows at any time:                        │
│ - 5-min tumbling: 12 windows/hour = ~60 KB        │
│ - Sessions: ~1000 active = ~5 MB                  │
│ - Sliding: 15 windows = ~75 KB                    │
│                                                    │
│ Total L1 capacity: 512 MB = ~100,000 windows      │
└────────────────────────────────────────────────────┘
```

## Checkpointing Flow

```
┌─────────────────────────────────────────────────────────────┐
│                 Checkpoint Coordinator                      │
│                 (Every 60 seconds)                          │
└─────────────────────────────────────────────────────────────┘
                          │
                          │ 1. Initiate checkpoint
                          ▼
        ┌──────────────────────────────────────┐
        │  Snapshot Window State Manager       │
        │  - Freeze current state              │
        │  - Assign checkpoint ID: cp_12345    │
        └──────────────────────────────────────┘
                          │
                          │ 2. Snapshot each layer
                          ▼
    ┌───────────┬─────────────────┬──────────────┐
    │           │                 │              │
    ▼           ▼                 ▼              ▼
┌───────┐  ┌────────┐       ┌─────────┐   ┌──────────┐
│ Hot   │  │ Warm   │       │ Kafka   │   │ Metadata │
│ State │  │ State  │       │ Offsets │   │          │
└───┬───┘  └───┬────┘       └────┬────┘   └────┬─────┘
    │          │                 │             │
    │          │ 3. Serialize    │             │
    ▼          ▼                 ▼             ▼
┌──────────────────────────────────────────────────────┐
│            Checkpoint File: cp_12345                 │
│  {                                                   │
│    "checkpoint_id": "cp_12345",                      │
│    "timestamp": "2025-11-09T10:30:00Z",              │
│    "watermark": "2025-11-09T10:28:00Z",              │
│    "hot_state": { ... },                             │
│    "warm_state_path": "/var/lib/optimizer/state",    │
│    "kafka_offsets": { "partition_0": 12345, ... },   │
│    "window_count": 1523                              │
│  }                                                   │
└──────────────────────────────────────────────────────┘
                          │
                          │ 4. Write to storage
                          ▼
              ┌──────────────────────┐
              │  Checkpoint Storage  │
              │  (Local FS or S3)    │
              └──────────────────────┘

Recovery Flow:
┌─────────────────────────────────────────────────────────────┐
│ 1. Read latest checkpoint: cp_12345                         │
│ 2. Restore hot state from checkpoint                        │
│ 3. Open warm state DB at saved path                         │
│ 4. Seek Kafka consumers to saved offsets                    │
│ 5. Resume watermark from checkpoint                         │
│ 6. Continue processing                                      │
│                                                             │
│ Recovery Time Objective (RTO): < 30 seconds                 │
└─────────────────────────────────────────────────────────────┘
```

## Trigger Policies Visualization

```
┌─────────────────────────────────────────────────────────────────┐
│               Window [10:00 - 10:05)                            │
└─────────────────────────────────────────────────────────────────┘

Events:     E1    E2    E3    E4    E5    E6    E7    E8
            │     │     │     │     │     │     │     │
Timestamp:  10:00 10:01 10:01 10:02 10:03 10:04 10:04 10:06 (late)
            │     │     │     │     │     │     │     │
            ▼     ▼     ▼     ▼     ▼     ▼     ▼     ▼
Processing: ───────────────────────────────────────────────▶

Emissions:
            │           │                 │     │     │
            ▼           ▼                 ▼     ▼     ▼
         Early(30s)  Early(30s)      OnTime  Late  Late
         [E1,E2]     [E1-E4]         [E1-E7] [E8]  [discarded]
                                                    (too late)

Timeline:
10:00:00 ─ Window opens, E1 arrives
10:00:01 ─ E2 arrives
10:00:30 ─ EARLY FIRE (30s trigger)
          ├─ Emit preliminary: {count: 2, avg_latency: 125ms}
          └─ State: Keep accumulating
10:01:00 ─ E3, E4 arrive
10:01:30 ─ EARLY FIRE (30s trigger)
          ├─ Emit update: {count: 4, avg_latency: 135ms}
          └─ State: Keep accumulating
10:03:00 ─ E5, E6 arrive, watermark = 10:01
10:04:00 ─ E7 arrives, watermark = 10:02
10:05:00 ─ Watermark advances to 10:03
          ├─ ONTIME FIRE (watermark >= window.end)
          ├─ Emit final: {count: 7, avg_latency: 142ms}
          └─ State: Keep for allowed_lateness (2min)
10:06:00 ─ E8 arrives (late), watermark = 10:04
          ├─ LATE FIRE (within allowed_lateness)
          ├─ Emit retraction + update: {count: 8, avg_latency: 145ms}
          └─ State: Keep until 10:07
10:07:00 ─ Watermark = 10:05
          ├─ Window.end + allowed_lateness passed
          ├─ PURGE window state
          └─ Future late events discarded
```

## Memory Management Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                  Memory Monitor                                 │
│  Current: 3.5 GB / 4.0 GB (87.5% usage)                        │
└─────────────────────────────────────────────────────────────────┘
                          │
                          ▼
              ┌───────────────────────┐
              │ Threshold Check       │
              │ 87.5% > 80% HIGH      │
              └───────────────────────┘
                          │
                          ▼
              ┌───────────────────────────────────────────┐
              │ Adaptive Pruning Strategy                 │
              └───────────────────────────────────────────┘
                          │
        ┌─────────────────┼─────────────────┐
        │                 │                 │
        ▼                 ▼                 ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│ Step 1       │  │ Step 2       │  │ Step 3       │
│ Watermark    │  │ Low Activity │  │ LRU          │
│ Pruning      │  │ Pruning      │  │ Eviction     │
└──────────────┘  └──────────────┘  └──────────────┘
│ Remove       │  │ Remove       │  │ Evict oldest │
│ completed    │  │ windows with │  │ accessed     │
│ windows      │  │ <10 events   │  │ windows      │
│              │  │              │  │              │
│ Freed: 500MB │  │ Freed: 200MB │  │ Freed: 300MB │
└──────────────┘  └──────────────┘  └──────────────┘
        │                 │                 │
        └─────────────────┼─────────────────┘
                          ▼
              ┌───────────────────────┐
              │ New Usage Check       │
              │ 2.5 GB / 4.0 GB (62%) │
              │ Below 70% LOW ✓       │
              └───────────────────────┘

Pruning Decision Tree:
┌─────────────────────────────────────────────────────────┐
│ Memory < 80%?                                           │
│   YES ──▶ Only prune completed windows (below watermark)│
│   NO  ──▶ Continue                                      │
│           │                                             │
│           ▼                                             │
│         Memory < 90%?                                   │
│           YES ──▶ Prune completed + low activity       │
│           NO  ──▶ Continue                              │
│                   │                                     │
│                   ▼                                     │
│                 CRITICAL: Aggressive LRU eviction       │
│                 - Flush to warm storage                 │
│                 - Free oldest 50% of windows            │
└─────────────────────────────────────────────────────────┘
```

## End-to-End Processing Pipeline

```
┌────────────────────────────────────────────────────────────────────────┐
│                        INPUT: Kafka Topics                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                │
│  │ perf-metrics │  │user-feedback │  │system-health │                │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘                │
└─────────┼──────────────────┼──────────────────┼─────────────────────────┘
          │                  │                  │
          ▼                  ▼                  ▼
┌────────────────────────────────────────────────────────────────────────┐
│                   STAGE 1: Event Time Assignment                       │
│  ┌──────────────────────────────────────────────────────────────┐     │
│  │ Extract timestamp from FeedbackEvent.timestamp()             │     │
│  │ Assign event time: event.event_time = event.timestamp        │     │
│  └──────────────────────────────────────────────────────────────┘     │
└────────────────────────────────────────────────────────────────────────┘
          │
          ▼
┌────────────────────────────────────────────────────────────────────────┐
│                   STAGE 2: Watermark Generation                        │
│  ┌──────────────────────────────────────────────────────────────┐     │
│  │ Per-partition watermark tracking                             │     │
│  │ watermark = min(partition_times) - max_out_of_order_delay    │     │
│  │ Emit watermark updates every 1 second                        │     │
│  └──────────────────────────────────────────────────────────────┘     │
└────────────────────────────────────────────────────────────────────────┘
          │
          ▼
┌────────────────────────────────────────────────────────────────────────┐
│                   STAGE 3: Window Assignment                           │
│  ┌──────────────────────────────────────────────────────────────┐     │
│  │ Tumbling: Assign to [(ts / size) * size, ...)               │     │
│  │ Sliding:  Assign to multiple overlapping windows            │     │
│  │ Session:  Create/extend session based on gap                │     │
│  └──────────────────────────────────────────────────────────────┘     │
└────────────────────────────────────────────────────────────────────────┘
          │
          ▼
┌────────────────────────────────────────────────────────────────────────┐
│                   STAGE 4: State Update                                │
│  ┌──────────────────────────────────────────────────────────────┐     │
│  │ 1. Fetch window state from tiered storage                   │     │
│  │ 2. Add event to aggregator (latencies, costs, errors)       │     │
│  │ 3. Update window metadata (count, timestamps)               │     │
│  │ 4. Write back to state store                                │     │
│  └──────────────────────────────────────────────────────────────┘     │
└────────────────────────────────────────────────────────────────────────┘
          │
          ▼
┌────────────────────────────────────────────────────────────────────────┐
│                   STAGE 5: Trigger Evaluation                          │
│  ┌──────────────────────────────────────────────────────────────┐     │
│  │ Check trigger policies:                                      │     │
│  │ - Early firing (every 30s) ──▶ Emit preliminary result      │     │
│  │ - On watermark (window complete) ──▶ Emit final result      │     │
│  │ - On late data ──▶ Emit update/retraction                   │     │
│  └──────────────────────────────────────────────────────────────┘     │
└────────────────────────────────────────────────────────────────────────┘
          │
          ▼
┌────────────────────────────────────────────────────────────────────────┐
│                   STAGE 6: Aggregation                                 │
│  ┌──────────────────────────────────────────────────────────────┐     │
│  │ Compute metrics from accumulated state:                     │     │
│  │ - p50, p95, p99 latency (using statrs)                      │     │
│  │ - Average cost, total cost                                  │     │
│  │ - Error rate (errors / count)                               │     │
│  │ - Throughput QPS (count / window_duration)                  │     │
│  └──────────────────────────────────────────────────────────────┘     │
└────────────────────────────────────────────────────────────────────────┘
          │
          ▼
┌────────────────────────────────────────────────────────────────────────┐
│                   OUTPUT: Storage & Downstream                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                │
│  │  PostgreSQL  │  │  Kafka Topic │  │  REST API    │                │
│  │ (metrics DB) │  │ (downstream) │  │  (query)     │                │
│  └──────────────┘  └──────────────┘  └──────────────┘                │
└────────────────────────────────────────────────────────────────────────┘
```

## Deployment Architecture

```
┌───────────────────────────────────────────────────────────────────────┐
│                      Kubernetes Deployment                            │
└───────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                    Stream Processor Pod                             │
│  ┌────────────────────────────────────────────────────────────┐    │
│  │                    Application Container                   │    │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │    │
│  │  │ Kafka        │  │ Windowing    │  │ Aggregation  │     │    │
│  │  │ Consumer     │─▶│ Operator     │─▶│ Operator     │     │    │
│  │  └──────────────┘  └──────────────┘  └──────────────┘     │    │
│  │                                                            │    │
│  │  ┌──────────────┐  ┌──────────────┐                       │    │
│  │  │ State        │  │ Checkpoint   │                       │    │
│  │  │ Manager      │  │ Coordinator  │                       │    │
│  │  └──────────────┘  └──────────────┘                       │    │
│  └────────────────────────────────────────────────────────────┘    │
│                                                                     │
│  ┌────────────────────────────────────────────────────────────┐    │
│  │                   Volume Mounts                            │    │
│  │  /var/lib/optimizer/state ──▶ Persistent Volume (Sled DB) │    │
│  │  /var/lib/optimizer/checkpoints ──▶ PV (Checkpoints)      │    │
│  └────────────────────────────────────────────────────────────┘    │
│                                                                     │
│  Resources:                                                         │
│  - CPU: 4 cores                                                     │
│  - Memory: 8 GB (4GB for processor, 4GB for OS/buffers)           │
│  - Storage: 50 GB SSD                                              │
└─────────────────────────────────────────────────────────────────────┘
                          │
                          │ Network
        ┌─────────────────┼─────────────────┐
        │                 │                 │
        ▼                 ▼                 ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│    Kafka     │  │  PostgreSQL  │  │    Redis     │
│   Cluster    │  │   Database   │  │   (Cache)    │
└──────────────┘  └──────────────┘  └──────────────┘
```

---

## Legend

```
Symbols:
  ▶  Data flow direction
  ──  Connection
  │  Vertical connection
  ├─ Branch
  └─ Terminal branch
  ▼  Downward flow
  ┌─┐ Box/Container
  [ ] Time interval
  ( ) Group/Scope
```

---

**Last Updated**: 2025-11-09
**Related Docs**:
- [STREAM_PROCESSING_WINDOWING_SPECIFICATION.md](./STREAM_PROCESSING_WINDOWING_SPECIFICATION.md)
- [WINDOWING_QUICK_REFERENCE.md](./WINDOWING_QUICK_REFERENCE.md)
