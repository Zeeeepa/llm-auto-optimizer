# Distributed State Architecture Diagrams

**Version:** 1.0
**Date:** 2025-11-10

This document contains detailed architectural diagrams for the distributed state system.

---

## Table of Contents

1. [System Overview](#1-system-overview)
2. [High Availability Architectures](#2-high-availability-architectures)
3. [Distributed Coordination](#3-distributed-coordination)
4. [Data Flow Diagrams](#4-data-flow-diagrams)
5. [Failure Scenarios](#5-failure-scenarios)
6. [Deployment Architectures](#6-deployment-architectures)

---

## 1. System Overview

### 1.1 Complete System Architecture

```
┌────────────────────────────────────────────────────────────────────────────────┐
│                          LLM Auto Optimizer                                    │
│                       Distributed State Layer v2                               │
└────────────────────────────────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────────────────────────────────┐
│                           Application Layer                                    │
├────────────────────────────────────────────────────────────────────────────────┤
│                                                                                │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │  Collector   │  │  Processor   │  │   Analyzer   │  │  Decision    │     │
│  │   Service    │  │   Service    │  │   Service    │  │   Engine     │     │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘     │
│         │                 │                 │                 │               │
│         └─────────────────┴─────────────────┴─────────────────┘               │
│                                    │                                           │
└────────────────────────────────────┼───────────────────────────────────────────┘
                                     │
┌────────────────────────────────────▼───────────────────────────────────────────┐
│                      StateCoordinator Interface                                │
├────────────────────────────────────────────────────────────────────────────────┤
│                                                                                │
│  ┌─────────────────────┐  ┌──────────────────────┐  ┌────────────────────┐  │
│  │  Distributed Locks  │  │  Leader Election     │  │  Coordination      │  │
│  │  • Redlock          │  │  • Raft-based        │  │  • Barriers        │  │
│  │  • Try-lock         │  │  • Heartbeat         │  │  • Semaphores      │  │
│  │  • Lock extension   │  │  • Term management   │  │  • Latches         │  │
│  └─────────────────────┘  └──────────────────────┘  └────────────────────┘  │
│                                                                                │
└────────────────────────────────────┬───────────────────────────────────────────┘
                                     │
┌────────────────────────────────────▼───────────────────────────────────────────┐
│                      Enhanced StateBackend Trait                               │
├────────────────────────────────────────────────────────────────────────────────┤
│                                                                                │
│  Core Operations         Transactions           Advanced                      │
│  ┌────────────────┐     ┌───────────────┐     ┌─────────────────┐           │
│  │ • get()        │     │ • begin_tx()  │     │ • get_versioned │           │
│  │ • put()        │     │ • commit()    │     │ • compare_and_  │           │
│  │ • delete()     │     │ • rollback()  │     │   set()         │           │
│  │ • batch_*()    │     │ • isolation   │     │ • merge_crdt()  │           │
│  └────────────────┘     └───────────────┘     └─────────────────┘           │
│                                                                                │
└────────────────────────────────────┬───────────────────────────────────────────┘
                                     │
          ┌──────────────────────────┴──────────────────────────┐
          │                                                       │
┌─────────▼──────────────────────┐                  ┌───────────▼──────────────┐
│    Redis State Backend         │                  │  PostgreSQL Backend      │
│         (Enhanced)             │                  │      (Enhanced)          │
├────────────────────────────────┤                  ├──────────────────────────┤
│                                │                  │                          │
│  ┌────────────────────────┐   │                  │  ┌───────────────────┐  │
│  │  Connection Manager    │   │                  │  │  Connection Pool  │  │
│  │  • Pool (min-max)      │   │                  │  │  • sqlx PgPool    │  │
│  │  • Health checks       │   │                  │  │  • Health checks  │  │
│  │  • Circuit breaker     │   │                  │  │  • Retry logic    │  │
│  └────────────────────────┘   │                  │  └───────────────────┘  │
│                                │                  │                          │
│  ┌────────────────────────┐   │                  │  ┌───────────────────┐  │
│  │  Mode Selection        │   │                  │  │  Replication      │  │
│  │  ┌──────────────────┐  │   │                  │  │  • Primary        │  │
│  │  │  Standalone      │  │   │                  │  │  • Replicas (N)   │  │
│  │  ├──────────────────┤  │   │                  │  │  • Read routing   │  │
│  │  │  Sentinel        │  │   │                  │  │  • Lag monitor    │  │
│  │  ├──────────────────┤  │   │                  │  └───────────────────┘  │
│  │  │  Cluster         │  │   │                  │                          │
│  │  └──────────────────┘  │   │                  │  ┌───────────────────┐  │
│  └────────────────────────┘   │                  │  │  Transactions     │  │
│                                │                  │  │  • BEGIN/COMMIT   │  │
│  ┌────────────────────────┐   │                  │  │  • Isolation      │  │
│  │  Performance           │   │                  │  │  • Savepoints     │  │
│  │  • Pipelining          │   │                  │  └───────────────────┘  │
│  │  • Batch ops           │   │                  │                          │
│  │  • LUA scripts         │   │                  │  ┌───────────────────┐  │
│  └────────────────────────┘   │                  │  │  Advanced Queries │  │
│                                │                  │  │  • Range scans    │  │
└────────────────────────────────┘                  │  │  • Joins          │  │
                                                    │  │  • Indexes        │  │
                                                    │  └───────────────────┘  │
                                                    │                          │
                                                    └──────────────────────────┘

┌────────────────────────────────────────────────────────────────────────────────┐
│                          Cross-Cutting Concerns                                │
├────────────────────────────────────────────────────────────────────────────────┤
│                                                                                │
│  Caching              Observability           Security                        │
│  ┌──────────────┐    ┌─────────────────┐    ┌──────────────────┐            │
│  │ L1: Memory   │    │ • Metrics       │    │ • TLS 1.3        │            │
│  │ L2: Redis    │    │ • Tracing       │    │ • Authentication │            │
│  │ L3: Backend  │    │ • Health checks │    │ • Authorization  │            │
│  └──────────────┘    │ • Audit logs    │    │ • Encryption     │            │
│                      └─────────────────┘    └──────────────────┘            │
│                                                                                │
└────────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. High Availability Architectures

### 2.1 Redis Sentinel Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       Redis Sentinel Deployment                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│                        Sentinel Cluster (Quorum: 2)                         │
│                                                                             │
│         ┌──────────────┐      ┌──────────────┐      ┌──────────────┐      │
│         │  Sentinel 1  │      │  Sentinel 2  │      │  Sentinel 3  │      │
│         │  Port: 26379 │      │  Port: 26379 │      │  Port: 26379 │      │
│         └──────┬───────┘      └──────┬───────┘      └──────┬───────┘      │
│                │                     │                     │               │
│                │      Monitor & Vote │                     │               │
│                └─────────────────────┴─────────────────────┘               │
│                                      │                                      │
│                                      │                                      │
│              ┌───────────────────────┴───────────────────────┐             │
│              │                                                │             │
│              │            Master Discovery                    │             │
│              │                                                │             │
│        ┌─────▼──────┐                                  ┌─────▼──────┐      │
│        │   Master   │                                  │  Client    │      │
│        │   Redis    │◄─────────────────────────────────│  (App)     │      │
│        │  Port:6379 │  Read/Write                      │            │      │
│        └─────┬──────┘                                  └────────────┘      │
│              │                                                              │
│              │ Replication                                                  │
│              │                                                              │
│      ┌───────┴────────┐                                                    │
│      │                │                                                     │
│  ┌───▼────┐      ┌────▼────┐                                              │
│  │Replica1│      │Replica2 │                                               │
│  │Port:679│      │Port:6379│                                               │
│  └────────┘      └─────────┘                                               │
│  (Read-only)     (Read-only)                                               │
│                                                                             │
│                                                                             │
│  Failover Sequence:                                                        │
│  ─────────────────                                                         │
│  1. Master fails → Sentinels detect (within 1-5s)                          │
│  2. Sentinels vote (quorum: 2/3)                                           │
│  3. Replica promoted to master                                             │
│  4. Other replica reconfigured                                             │
│  5. Clients notified of new master                                         │
│  6. Total time: 5-10 seconds                                               │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Redis Cluster Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Redis Cluster Deployment                            │
│                         16384 Hash Slots Distributed                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│    Hash Slot Distribution:                                                 │
│    Slots 0-5460      Slots 5461-10922    Slots 10923-16383                │
│         ▼                   ▼                    ▼                          │
│                                                                             │
│  ┌────────────────┐    ┌────────────────┐    ┌────────────────┐          │
│  │   Master M1    │    │   Master M2    │    │   Master M3    │          │
│  │  192.168.1.1   │    │  192.168.1.2   │    │  192.168.1.3   │          │
│  │  Port: 6379    │    │  Port: 6379    │    │  Port: 6379    │          │
│  │ Slots: 0-5460  │    │Slots: 5461-    │    │Slots: 10923-   │          │
│  │                │    │      10922     │    │      16383     │          │
│  └────────┬───────┘    └────────┬───────┘    └────────┬───────┘          │
│           │                     │                     │                    │
│           │ Replicate           │ Replicate           │ Replicate          │
│           │                     │                     │                    │
│  ┌────────▼───────┐    ┌────────▼───────┐    ┌────────▼───────┐          │
│  │  Replica R1    │    │  Replica R2    │    │  Replica R3    │          │
│  │  192.168.1.4   │    │  192.168.1.5   │    │  192.168.1.6   │          │
│  │  Port: 6379    │    │  Port: 6379    │    │  Port: 6379    │          │
│  └────────────────┘    └────────────────┘    └────────────────┘          │
│                                                                             │
│                                                                             │
│  Client Connection:                                                        │
│  ──────────────────                                                        │
│                                                                             │
│       ┌────────────┐                                                       │
│       │   Client   │                                                       │
│       │  (App)     │                                                       │
│       └─────┬──────┘                                                       │
│             │                                                               │
│             │ 1. Request for key "user:123"                                │
│             │    CRC16("user:123") % 16384 = slot 7823                     │
│             │                                                               │
│             ▼                                                               │
│       ┌────────────────┐                                                   │
│       │   Master M2    │ ◄─── Slot 7823 belongs to M2                     │
│       │ Slots 5461-    │                                                   │
│       │     10922      │                                                   │
│       └────────────────┘                                                   │
│                                                                             │
│                                                                             │
│  Failover Scenario:                                                        │
│  ──────────────────                                                        │
│                                                                             │
│  1. Master M2 fails                                                        │
│  2. Replica R2 promoted to master (automatic)                              │
│  3. Cluster topology updated                                               │
│  4. Clients redirected to new master                                       │
│  5. Slots 5461-10922 still available                                       │
│  6. Total downtime: < 5 seconds                                            │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.3 PostgreSQL Streaming Replication

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                  PostgreSQL Streaming Replication                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│                         ┌──────────────────┐                               │
│                         │  Primary Server  │                               │
│                         │  (Read/Write)    │                               │
│                         │                  │                               │
│                         │  WAL Sender      │                               │
│                         │  Processes       │                               │
│                         └────────┬─────────┘                               │
│                                  │                                          │
│                                  │                                          │
│            WAL Stream            │            WAL Stream                    │
│            (Async)               │            (Sync)                        │
│                                  │                                          │
│        ┌─────────────────────────┼─────────────────────────┐               │
│        │                         │                         │               │
│  ┌─────▼──────┐           ┌─────▼──────┐           ┌─────▼──────┐        │
│  │  Standby 1 │           │  Standby 2 │           │  Standby 3 │        │
│  │ (Read-only)│           │ (Read-only)│           │ (Read-only)│        │
│  │            │           │            │           │            │        │
│  │ Replication│           │ Replication│           │ Replication│        │
│  │ Slot: s1   │           │ Slot: s2   │           │ Slot: s3   │        │
│  │            │           │            │           │            │        │
│  │ Lag: 10ms  │           │ Lag: 5ms   │           │ Lag: 100ms │        │
│  └────────────┘           └────────────┘           └────────────┘        │
│  US-EAST-1a              US-EAST-1b                US-EAST-1c            │
│                                                                             │
│                                                                             │
│  Application Read Routing:                                                 │
│  ────────────────────────                                                  │
│                                                                             │
│       ┌────────────┐                                                       │
│       │   Client   │                                                       │
│       │            │                                                       │
│       └──┬──────┬──┘                                                       │
│          │      │                                                           │
│    Write │      │ Read                                                     │
│          │      │                                                           │
│   ┌──────▼──┐   └──────────┐                                              │
│   │ Primary │              │                                               │
│   │         │         ┌────▼────────┐                                      │
│   │ (R/W)   │         │ Read Router │                                      │
│   └─────────┘         │             │                                      │
│                       │ Strategy:   │                                      │
│                       │ • Least lag │                                      │
│                       │ • Round     │                                      │
│                       │   robin     │                                      │
│                       │ • Random    │                                      │
│                       └─────┬───────┘                                      │
│                             │                                               │
│                    ┌────────┼────────┐                                     │
│                    │        │        │                                      │
│              ┌─────▼──┐ ┌──▼────┐ ┌─▼──────┐                              │
│              │Standby1│ │Standby2│ │Standby3│                             │
│              │(Read)  │ │(Read)  │ │(Read)  │                             │
│              └────────┘ └────────┘ └────────┘                             │
│                                                                             │
│                                                                             │
│  Failover Process:                                                         │
│  ────────────────                                                          │
│                                                                             │
│  1. Primary fails or is shut down                                          │
│  2. Standby with lowest lag promoted (Standby 2)                           │
│  3. Other standbys reconfigured to follow new primary                      │
│  4. Applications reconnect to new primary                                  │
│  5. Old primary (if recoverable) becomes standby                           │
│  6. Total time: 10-30 seconds                                              │
│                                                                             │
│  Synchronous Replication:                                                  │
│  ────────────────────────                                                  │
│  Primary waits for Standby 2 to confirm write before committing            │
│  → Zero data loss guarantee                                                │
│  → Slight latency increase (5-10ms)                                        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Distributed Coordination

### 3.1 Redlock Algorithm

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Redlock Algorithm Flow                              │
│                    (Distributed Lock Across 5 Redis Instances)              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   Client wants to acquire lock on resource "checkpoint"                    │
│                                                                             │
│   Step 1: Generate unique token                                            │
│   ────────────────────────────────                                         │
│   token = UUID::new_v4()  // e.g., "abc-123-def-456"                       │
│                                                                             │
│                                                                             │
│   Step 2: Try to acquire lock on all instances                             │
│   ──────────────────────────────────────────                               │
│                                                                             │
│       ┌────────┐                                                           │
│       │ Client │                                                           │
│       └───┬────┘                                                           │
│           │                                                                 │
│           │  SET checkpoint abc-123-def-456 NX PX 30000                    │
│           │                                                                 │
│     ┌─────┼─────┬─────┬─────┬─────┐                                       │
│     │     │     │     │     │     │                                        │
│ ┌───▼──┐ ┌▼───┐ ┌▼───┐ ┌▼───┐ ┌───▼┐                                      │
│ │Redis1│ │Red2│ │Red3│ │Red4│ │Red5│                                      │
│ └──┬───┘ └─┬──┘ └─┬──┘ └─┬──┘ └──┬─┘                                      │
│    │       │      │      │       │                                         │
│    │ OK    │ OK   │ OK   │FAIL   │ OK                                      │
│    │       │      │      │       │                                         │
│    └───┬───┴───┬──┴───┬──┴───┬───┘                                        │
│        │       │      │      │                                             │
│        └───────┴──────┴──────┘                                             │
│                │                                                            │
│                ▼                                                            │
│         Result: 4/5 success                                                │
│                                                                             │
│                                                                             │
│   Step 3: Check quorum and validity time                                   │
│   ────────────────────────────────────────                                 │
│                                                                             │
│   Quorum: N/2 + 1 = 5/2 + 1 = 3                                            │
│   Success count: 4 ✓ (>= quorum)                                           │
│                                                                             │
│   Time spent: 50ms                                                         │
│   TTL: 30000ms                                                             │
│   Clock drift: 1% = 300ms                                                  │
│   Validity time = 30000 - 50 - 300 = 29650ms ✓                            │
│                                                                             │
│   → Lock ACQUIRED successfully                                             │
│                                                                             │
│                                                                             │
│   Step 4: Perform critical operation                                       │
│   ────────────────────────────────────                                     │
│                                                                             │
│   // Critical section                                                      │
│   // ... do work ...                                                       │
│                                                                             │
│                                                                             │
│   Step 5: Release lock on all instances                                    │
│   ───────────────────────────────────────                                  │
│                                                                             │
│   LUA script: if redis.call("get",KEYS[1]) == ARGV[1]                      │
│               then return redis.call("del",KEYS[1])                        │
│               else return 0 end                                            │
│                                                                             │
│       ┌────────┐                                                           │
│       │ Client │                                                           │
│       └───┬────┘                                                           │
│           │                                                                 │
│           │  EVAL <script> checkpoint abc-123-def-456                      │
│           │                                                                 │
│     ┌─────┼─────┬─────┬─────┬─────┐                                       │
│     │     │     │     │     │     │                                        │
│ ┌───▼──┐ ┌▼───┐ ┌▼───┐ ┌▼───┐ ┌───▼┐                                      │
│ │Redis1│ │Red2│ │Red3│ │Red4│ │Red5│                                      │
│ └──┬───┘ └─┬──┘ └─┬──┘ └─┬──┘ └──┬─┘                                      │
│    │ 1    │ 1    │ 1    │ 0    │ 1                                         │
│    └──────┴──────┴──────┴──────┘                                          │
│                                                                             │
│   → Lock RELEASED successfully                                             │
│                                                                             │
│                                                                             │
│   Safety Properties:                                                       │
│   ──────────────────                                                       │
│   • Mutual exclusion: At most one client holds lock                        │
│   • Deadlock free: Locks auto-expire via TTL                               │
│   • Fault tolerance: Works with minority node failures                     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Leader Election (Raft-style)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     Leader Election State Machine                           │
│                         (Raft Algorithm)                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   Initial State: All nodes are Followers                                   │
│   ──────────────────────────────────────────                               │
│                                                                             │
│   ┌──────────────┐       ┌──────────────┐       ┌──────────────┐          │
│   │   Node A     │       │   Node B     │       │   Node C     │          │
│   │  Follower    │       │  Follower    │       │  Follower    │          │
│   │  Term: 0     │       │  Term: 0     │       │  Term: 0     │          │
│   └──────────────┘       └──────────────┘       └──────────────┘          │
│                                                                             │
│   Each node has election timeout: random(150-300ms)                        │
│                                                                             │
│                                                                             │
│   Scenario 1: Node B timeout expires first                                 │
│   ─────────────────────────────────────────                                │
│                                                                             │
│   ┌──────────────┐       ┌──────────────┐       ┌──────────────┐          │
│   │   Node A     │       │   Node B     │       │   Node C     │          │
│   │  Follower    │       │  Candidate   │◄──────┤  Follower    │          │
│   │  Term: 0     │       │  Term: 1     │       │  Term: 0     │          │
│   └──────┬───────┘       └──────┬───────┘       └──────┬───────┘          │
│          │                      │                      │                   │
│          │  VoteRequest(term=1) │                      │                   │
│          │◄─────────────────────┤                      │                   │
│          │                      │  VoteRequest(term=1) │                   │
│          │                      ├──────────────────────►                   │
│          │                      │                      │                   │
│          │  VoteResponse(Y)     │                      │                   │
│          ├──────────────────────►                      │                   │
│          │                      │  VoteResponse(Y)     │                   │
│          │                      ◄──────────────────────┤                   │
│          │                      │                      │                   │
│                                                                             │
│   Node B receives majority votes (2/3) → Becomes Leader                    │
│                                                                             │
│                                                                             │
│   Steady State: Leader sends heartbeats                                    │
│   ──────────────────────────────────────                                   │
│                                                                             │
│   ┌──────────────┐       ┌──────────────┐       ┌──────────────┐          │
│   │   Node A     │       │   Node B     │       │   Node C     │          │
│   │  Follower    │◄──────┤   Leader     ├──────►│  Follower    │          │
│   │  Term: 1     │       │  Term: 1     │       │  Term: 1     │          │
│   └──────────────┘       └──────────────┘       └──────────────┘          │
│                          Heartbeat every 50ms                               │
│                                                                             │
│                                                                             │
│   Scenario 2: Leader fails                                                 │
│   ──────────────────────                                                   │
│                                                                             │
│   ┌──────────────┐       ┌──────────────┐       ┌──────────────┐          │
│   │   Node A     │       │   Node B     │       │   Node C     │          │
│   │  Follower    │       │   FAILED     │       │  Follower    │          │
│   │  Term: 1     │       │      X       │       │  Term: 1     │          │
│   └──────────────┘       └──────────────┘       └──────┬───────┘          │
│                                                         │                   │
│   Node A and C stop receiving heartbeats                │                   │
│   Election timeouts expire                              │                   │
│   Node C becomes candidate first (random timeout)       │                   │
│                                                         │                   │
│   ┌──────────────┐                               ┌─────▼────────┐          │
│   │   Node A     │◄──────────────────────────────┤   Node C     │          │
│   │  Follower    │   VoteRequest(term=2)         │  Candidate   │          │
│   │  Term: 2     ├──────────────────────────────►│  Term: 2     │          │
│   └──────────────┘   VoteResponse(Y)             └──────────────┘          │
│                                                                             │
│   Node C receives majority (2/3) → Becomes new Leader                      │
│                                                                             │
│   ┌──────────────┐                               ┌──────────────┐          │
│   │   Node A     │◄──────────────────────────────┤   Node C     │          │
│   │  Follower    │       Heartbeat               │   Leader     │          │
│   │  Term: 2     │                               │  Term: 2     │          │
│   └──────────────┘                               └──────────────┘          │
│                                                                             │
│                                                                             │
│   State Transitions:                                                       │
│   ──────────────────                                                       │
│                                                                             │
│                    timeout                                                  │
│                    ┌──────┐                                                │
│                    │      ▼                                                 │
│              ┌──────────────┐                                              │
│              │  Candidate   │                                              │
│     timeout  │              │  receives votes                              │
│      ┌───────┤              │  from majority                               │
│      │       └──────────────┘       │                                      │
│      │              │               │                                       │
│      │              │               │                                       │
│      ▼              ▼               ▼                                       │
│  ┌──────────┐  discovers  ┌──────────────┐                                │
│  │ Follower │  current    │    Leader    │                                │
│  │          │  leader or  │              │                                │
│  │          ◄─new term    │              │                                │
│  └──────────┘             └──────┬───────┘                                │
│      ▲                           │                                         │
│      │                           │ discovers server                        │
│      │                           │ with higher term                        │
│      └───────────────────────────┘                                         │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Data Flow Diagrams

### 4.1 Write Path with Caching

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Write Operation Flow                                 │
│                   (Write-Through Strategy)                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   Application                                                               │
│   ────────────                                                              │
│      │                                                                       │
│      │ put("user:123", data)                                                │
│      │                                                                       │
│      ▼                                                                       │
│   ┌──────────────────────┐                                                 │
│   │ CachedStateBackend   │                                                 │
│   └──────────┬───────────┘                                                 │
│              │                                                               │
│              ├──────────────────┐                                           │
│              │                  │                                           │
│              ▼                  ▼                                           │
│   ┌─────────────────┐   ┌──────────────────┐                              │
│   │  L1: In-Memory  │   │  L3: Backend     │                              │
│   │  Cache (Moka)   │   │  (PostgreSQL)    │                              │
│   │                 │   │                  │                              │
│   │  • Invalidate   │   │  1. Begin TX     │                              │
│   │    old entry    │   │  2. Write data   │                              │
│   │  • Write new    │   │  3. Commit TX    │                              │
│   │    entry        │   │  4. Confirm      │                              │
│   │  • TTL: 5min    │   │                  │                              │
│   └─────────────────┘   └────────┬─────────┘                              │
│              │                    │                                         │
│              │                    │ Success                                 │
│              │                    │                                         │
│              │                    ▼                                         │
│              │         ┌──────────────────┐                                │
│              └────────►│  L2: Redis       │                                │
│                        │  Cache           │                                │
│                        │                  │                                │
│                        │  • Write entry   │                                │
│                        │  • TTL: 1hour    │                                │
│                        │  • Async         │                                │
│                        └──────────────────┘                                │
│                                 │                                           │
│                                 │ Success                                   │
│                                 ▼                                           │
│                          Return to caller                                  │
│                                                                             │
│                                                                             │
│   Timeline:                                                                │
│   ────────                                                                 │
│                                                                             │
│   t=0ms    │ put() called                                                  │
│   t=1ms    │ L1 cache updated                                              │
│   t=5ms    │ PostgreSQL write completed                                    │
│   t=6ms    │ L2 Redis write initiated (async)                              │
│   t=5ms    │ Return success to caller                                      │
│   t=8ms    │ L2 write completed (background)                               │
│                                                                             │
│                                                                             │
│   Failure Handling:                                                        │
│   ─────────────────                                                        │
│                                                                             │
│   If PostgreSQL write fails:                                               │
│   • Rollback L1 cache update                                               │
│   • Return error to caller                                                 │
│   • L2 write skipped                                                       │
│   • Data consistency maintained                                            │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Read Path with Multi-Tier Cache

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Read Operation Flow                                 │
│                    (Cache-Aside Pattern)                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   Application                                                               │
│   ────────────                                                              │
│      │                                                                       │
│      │ get("user:123")                                                      │
│      │                                                                       │
│      ▼                                                                       │
│   ┌──────────────────────┐                                                 │
│   │ CachedStateBackend   │                                                 │
│   └──────────┬───────────┘                                                 │
│              │                                                               │
│              ▼                                                               │
│   ┌─────────────────────┐                                                  │
│   │ L1: In-Memory Cache │                                                  │
│   │     (Moka)          │                                                  │
│   └──────────┬──────────┘                                                  │
│              │                                                               │
│              │ Check cache                                                  │
│              │                                                               │
│        ┌─────┴─────┐                                                       │
│        │           │                                                        │
│    HIT │           │ MISS                                                   │
│        │           │                                                        │
│        ▼           ▼                                                        │
│   ┌────────┐   ┌─────────────────────┐                                    │
│   │ Return │   │ L2: Redis Cache     │                                    │
│   │  data  │   └──────────┬──────────┘                                    │
│   └────────┘              │                                                 │
│    ~1-10μs                │ Check cache                                    │
│                           │                                                 │
│                     ┌─────┴─────┐                                          │
│                     │           │                                           │
│                 HIT │           │ MISS                                      │
│                     │           │                                           │
│                     ▼           ▼                                           │
│                ┌────────┐   ┌──────────────────────┐                       │
│                │Populate│   │ L3: PostgreSQL       │                       │
│                │  L1    │   └──────────┬───────────┘                       │
│                │        │              │                                    │
│                │ Return │              │ Query database                     │
│                │  data  │              │                                    │
│                └────────┘              │                                    │
│                ~100-500μs        ┌─────┴─────┐                             │
│                                  │           │                              │
│                              HIT │           │ MISS                         │
│                                  │           │                              │
│                                  ▼           ▼                              │
│                             ┌────────┐   ┌────────┐                        │
│                             │Populate│   │ Return │                        │
│                             │L1 + L2 │   │  None  │                        │
│                             │        │   └────────┘                        │
│                             │ Return │                                     │
│                             │  data  │                                     │
│                             └────────┘                                     │
│                             ~1-10ms                                        │
│                                                                             │
│                                                                             │
│   Performance Breakdown:                                                   │
│   ──────────────────────                                                   │
│                                                                             │
│   Scenario 1: L1 hit (90% of requests)                                     │
│   • Latency: 1-10μs                                                        │
│   • No network I/O                                                         │
│   • Best performance                                                       │
│                                                                             │
│   Scenario 2: L1 miss, L2 hit (8% of requests)                             │
│   • Latency: 100-500μs                                                     │
│   • 1 Redis network call                                                   │
│   • L1 populated for next access                                           │
│                                                                             │
│   Scenario 3: L1 miss, L2 miss, L3 hit (2% of requests)                    │
│   • Latency: 1-10ms                                                        │
│   • 1 Redis + 1 PostgreSQL network calls                                   │
│   • L1 and L2 populated                                                    │
│                                                                             │
│   Scenario 4: Complete miss (<0.1% of requests)                            │
│   • Latency: 1-10ms                                                        │
│   • Key not found                                                          │
│   • Negative caching (optional)                                            │
│                                                                             │
│                                                                             │
│   Overall Cache Hit Rate: 98% (L1 90% + L2 8%)                             │
│   Average Latency: ~50μs                                                   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Failure Scenarios

### 5.1 Redis Master Failure and Recovery

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                  Redis Master Failure Sequence                              │
│                  (Sentinel Automatic Failover)                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Time: t=0s                                                                 │
│  ──────────────                                                             │
│  Normal Operation                                                           │
│                                                                             │
│         Client                    Sentinel                                  │
│           │                          │                                      │
│           │                          │                                      │
│       ┌───▼────┐                ┌───▼────┐                                 │
│       │ Master │                │Sentinel│                                 │
│       │  (OK)  │◄───monitor────│  (OK)  │                                 │
│       └───┬────┘                └────────┘                                 │
│           │                                                                 │
│    ┌──────┴──────┐                                                         │
│    │             │                                                          │
│ ┌──▼──┐       ┌──▼──┐                                                      │
│ │Rep 1│       │Rep 2│                                                      │
│ └─────┘       └─────┘                                                      │
│                                                                             │
│                                                                             │
│  Time: t=1s                                                                 │
│  ──────────────                                                             │
│  Master Crashes                                                             │
│                                                                             │
│       Client                    Sentinel                                    │
│         │                          │                                        │
│         │ write fails              │                                        │
│         │ (connection error)       │                                        │
│         │                          │                                        │
│       ┌─▼──┐                  ┌───▼────┐                                   │
│       │ ✗  │                  │Sentinel│                                   │
│       │FAIL│  no response     │ detects│                                   │
│       └────┘◄─────────────────│ failure│                                   │
│                                └───┬────┘                                   │
│                                    │                                        │
│                             ┌──────┴──────┐                                │
│                             │             │                                 │
│                          ┌──▼──┐       ┌──▼──┐                             │
│                          │Rep 1│       │Rep 2│                             │
│                          │(OK) │       │(OK) │                             │
│                          └─────┘       └─────┘                             │
│                                                                             │
│                                                                             │
│  Time: t=2s                                                                 │
│  ──────────────                                                             │
│  Sentinels Vote                                                             │
│                                                                             │
│                       ┌──────────────┐                                      │
│                       │  Sentinel 1  │                                      │
│                       │   (quorum)   │                                      │
│                       └──────┬───────┘                                      │
│                              │                                              │
│                   ┌──────────┼──────────┐                                  │
│                   │          │          │                                   │
│              ┌────▼───┐ ┌────▼───┐ ┌───▼────┐                              │
│              │Sentinel│ │Sentinel│ │Sentinel│                             │
│              │   2    │ │   3    │ │   4    │                             │
│              └────────┘ └────────┘ └────────┘                             │
│                   │          │          │                                   │
│                   │    VOTE  │          │                                   │
│                   └──────────┼──────────┘                                  │
│                              │                                              │
│                   "Promote Replica 1"                                       │
│                                                                             │
│                                                                             │
│  Time: t=4s                                                                 │
│  ──────────────                                                             │
│  Promotion Complete                                                         │
│                                                                             │
│       Client                    Sentinel                                    │
│         │                          │                                        │
│         │ discover new master      │                                        │
│         │◄─────────────────────────┤                                        │
│         │                          │                                        │
│       ┌─▼─────┐                ┌───▼────┐                                  │
│       │ Rep 1 │                │Sentinel│                                  │
│       │ (NEW  │◄───monitor────│  (OK)  │                                  │
│       │MASTER)│                └────────┘                                  │
│       └───┬───┘                                                             │
│           │                                                                 │
│           │ reconfigure                                                     │
│           │                                                                 │
│        ┌──▼──┐                                                              │
│        │Rep 2│                                                              │
│        │(NEW │                                                              │
│        │REP) │                                                              │
│        └─────┘                                                              │
│                                                                             │
│                                                                             │
│  Time: t=5s                                                                 │
│  ──────────────                                                             │
│  Normal Operation Resumed                                                   │
│                                                                             │
│       Client                    Sentinel                                    │
│         │                          │                                        │
│         │ writes successful        │                                        │
│         │                          │                                        │
│       ┌─▼─────┐                ┌───▼────┐                                  │
│       │Rep 1  │                │Sentinel│                                  │
│       │(MASTER│◄───monitor────│  (OK)  │                                  │
│       │  OK)  │                └────────┘                                  │
│       └───┬───┘                                                             │
│           │                                                                 │
│        ┌──▼──┐                                                              │
│        │Rep 2│                                                              │
│        └─────┘                                                              │
│                                                                             │
│  Total Downtime: ~5 seconds                                                │
│  Data Loss: 0 (replication was up-to-date)                                 │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. Deployment Architectures

### 6.1 Multi-Region Deployment

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      Multi-Region Deployment                                │
│                  (Active-Active Configuration)                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                         Region: US-EAST-1                           │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │                                                                     │   │
│  │   ┌──────────────────┐                  ┌──────────────────┐      │   │
│  │   │  Application     │                  │  Application     │      │   │
│  │   │  Instance 1      │                  │  Instance 2      │      │   │
│  │   └────────┬─────────┘                  └────────┬─────────┘      │   │
│  │            │                                      │                │   │
│  │            └──────────────┬───────────────────────┘                │   │
│  │                           │                                        │   │
│  │                  ┌────────▼─────────┐                             │   │
│  │                  │ Load Balancer    │                             │   │
│  │                  └────────┬─────────┘                             │   │
│  │                           │                                        │   │
│  │         ┌─────────────────┼─────────────────┐                     │   │
│  │         │                 │                 │                     │   │
│  │   ┌─────▼──────┐    ┌─────▼──────┐   ┌─────▼──────┐             │   │
│  │   │   Redis    │    │PostgreSQL  │   │   Redis    │             │   │
│  │   │  Cluster   │    │  Primary   │   │L2 Cache    │             │   │
│  │   │  (R/W)     │    │  (R/W)     │   │            │             │   │
│  │   └─────┬──────┘    └─────┬──────┘   └────────────┘             │   │
│  │         │                 │                                       │   │
│  └─────────┼─────────────────┼───────────────────────────────────────┘   │
│            │                 │                                            │
│            │                 │                                            │
│            │  Replication    │  Replication                               │
│            │  (Async)        │  (Logical)                                 │
│            │                 │                                            │
│  ┌─────────┼─────────────────┼───────────────────────────────────────┐   │
│  │         │                 │                   Region: EU-WEST-1   │   │
│  ├─────────┼─────────────────┼───────────────────────────────────────┤   │
│  │         │                 │                                       │   │
│  │   ┌─────▼──────┐    ┌─────▼──────┐   ┌──────────────┐           │   │
│  │   │   Redis    │    │PostgreSQL  │   │   Redis      │           │   │
│  │   │  Cluster   │    │  Primary   │   │ L2 Cache     │           │   │
│  │   │  (R/W)     │    │  (R/W)     │   │              │           │   │
│  │   └─────┬──────┘    └─────┬──────┘   └──────────────┘           │   │
│  │         │                 │                                       │   │
│  │         └─────────────────┼────────────┐                          │   │
│  │                           │            │                          │   │
│  │                  ┌────────▼─────────┐  │                          │   │
│  │                  │ Load Balancer    │  │                          │   │
│  │                  └────────┬─────────┘  │                          │   │
│  │                           │            │                          │   │
│  │            ┌──────────────┴─────────┐  │                          │   │
│  │            │                        │  │                          │   │
│  │   ┌────────▼─────────┐   ┌─────────▼──┴───────┐                  │   │
│  │   │  Application     │   │  Application        │                  │   │
│  │   │  Instance 1      │   │  Instance 2         │                  │   │
│  │   └──────────────────┘   └─────────────────────┘                  │   │
│  │                                                                    │   │
│  └────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│                                                                             │
│  Conflict Resolution Strategy:                                             │
│  ────────────────────────────                                              │
│  • CRDTs for counters and sets                                             │
│  • Last-Write-Wins (LWW) for simple updates                                │
│  • Application-level conflict resolution for complex data                  │
│                                                                             │
│  Consistency Guarantees:                                                   │
│  ──────────────────────                                                    │
│  • Eventual consistency across regions (typical lag: 50-200ms)             │
│  • Strong consistency within region                                        │
│  • Causal consistency via version vectors                                  │
│                                                                             │
│  Failure Handling:                                                         │
│  ─────────────────                                                         │
│  • Region failure: Traffic routed to healthy region                        │
│  • Cross-region latency increase: Clients prefer local region              │
│  • Split brain prevention: Leader election per region                      │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Kubernetes Deployment

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                   Kubernetes Cluster Deployment                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Namespace: llm-optimizer                                                  │
│  ──────────────────────────                                                │
│                                                                             │
│  ┌──────────────────────────────────────────────────────────────────┐      │
│  │                    Application Pods                              │      │
│  ├──────────────────────────────────────────────────────────────────┤      │
│  │                                                                  │      │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │      │
│  │  │   Pod 1      │  │   Pod 2      │  │   Pod 3      │          │      │
│  │  │              │  │              │  │              │          │      │
│  │  │ ┌──────────┐ │  │ ┌──────────┐ │  │ ┌──────────┐ │          │      │
│  │  │ │App       │ │  │ │App       │ │  │ │App       │ │          │      │
│  │  │ │Container │ │  │ │Container │ │  │ │Container │ │          │      │
│  │  │ └────┬─────┘ │  │ └────┬─────┘ │  │ └────┬─────┘ │          │      │
│  │  │      │       │  │      │       │  │      │       │          │      │
│  │  │ ┌────▼─────┐ │  │ ┌────▼─────┐ │  │ ┌────▼─────┐ │          │      │
│  │  │ │Metrics   │ │  │ │Metrics   │ │  │ │Metrics   │ │          │      │
│  │  │ │Sidecar   │ │  │ │Sidecar   │ │  │ │Sidecar   │ │          │      │
│  │  │ └──────────┘ │  │ └──────────┘ │  │ └──────────┘ │          │      │
│  │  │              │  │              │  │              │          │      │
│  │  │ Liveness:    │  │ Liveness:    │  │ Liveness:    │          │      │
│  │  │ /health/live │  │ /health/live │  │ /health/live │          │      │
│  │  │ Readiness:   │  │ Readiness:   │  │ Readiness:   │          │      │
│  │  │ /health/ready│  │ /health/ready│  │ /health/ready│          │      │
│  │  └──────────────┘  └──────────────┘  └──────────────┘          │      │
│  │                                                                  │      │
│  └────────────────────────┬─────────────────────────────────────────┘      │
│                           │                                                │
│                           │                                                │
│                  ┌────────▼─────────┐                                      │
│                  │   Service        │                                      │
│                  │   (ClusterIP)    │                                      │
│                  └────────┬─────────┘                                      │
│                           │                                                │
│                           │                                                │
│           ┌───────────────┼───────────────┐                               │
│           │               │               │                                │
│  ┌────────▼─────────┐  ┌─▼──────────┐  ┌─▼──────────────┐                │
│  │  Redis StatefulSet│ │ PostgreSQL │  │ Prometheus     │                │
│  │                   │ │ StatefulSet│  │ (Monitoring)   │                │
│  ├───────────────────┤ ├────────────┤  └────────────────┘                │
│  │                   │ │            │                                      │
│  │ • Master Pod      │ │ • Primary  │                                      │
│  │ • Replica Pod 1   │ │ • Replica1 │                                      │
│  │ • Replica Pod 2   │ │ • Replica2 │                                      │
│  │                   │ │            │                                      │
│  │ PVC:              │ │ PVC:       │                                      │
│  │ • redis-master-0  │ │ • pg-0     │                                      │
│  │ • redis-replica-1 │ │ • pg-1     │                                      │
│  │ • redis-replica-2 │ │ • pg-2     │                                      │
│  │                   │ │            │                                      │
│  └───────────────────┘ └────────────┘                                      │
│                                                                             │
│                                                                             │
│  ConfigMaps:                      Secrets:                                 │
│  ───────────                      ────────                                 │
│  • redis-config                   • redis-password                         │
│  • postgres-config                • postgres-credentials                   │
│  • app-config                     • tls-certificates                       │
│  • monitoring-config              • encryption-keys                        │
│                                                                             │
│                                                                             │
│  Ingress:                                                                  │
│  ────────                                                                  │
│  ┌────────────────────────────────────────┐                               │
│  │  Ingress Controller (nginx)            │                               │
│  │                                         │                               │
│  │  Routes:                                │                               │
│  │  • /api/v1/*  → app-service:8080       │                               │
│  │  • /metrics   → prometheus:9090        │                               │
│  │  • /health/*  → app-service:8080       │                               │
│  │                                         │                               │
│  │  TLS: enabled                           │                               │
│  │  Certificate: letsencrypt               │                               │
│  └────────────────────────────────────────┘                               │
│                                                                             │
│                                                                             │
│  Resource Limits:                                                          │
│  ───────────────                                                           │
│  Application Pods:                                                         │
│  • CPU: 500m - 2000m                                                       │
│  • Memory: 512Mi - 2Gi                                                     │
│                                                                             │
│  Redis Pods:                                                               │
│  • CPU: 1000m - 4000m                                                      │
│  • Memory: 2Gi - 8Gi                                                       │
│                                                                             │
│  PostgreSQL Pods:                                                          │
│  • CPU: 2000m - 8000m                                                      │
│  • Memory: 4Gi - 16Gi                                                      │
│                                                                             │
│                                                                             │
│  Auto-Scaling:                                                             │
│  ─────────────                                                             │
│  HPA (Horizontal Pod Autoscaler):                                          │
│  • Target CPU: 70%                                                         │
│  • Min replicas: 3                                                         │
│  • Max replicas: 10                                                        │
│                                                                             │
│  VPA (Vertical Pod Autoscaler):                                            │
│  • Mode: Auto                                                              │
│  • Update policy: Recreate                                                 │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Summary

This document provides comprehensive visual representations of the distributed state architecture, covering:

1. **System Overview**: Complete architecture with all components
2. **High Availability**: Redis Sentinel, Cluster, and PostgreSQL replication
3. **Distributed Coordination**: Redlock and Raft-based leader election
4. **Data Flows**: Read and write paths through multi-tier cache
5. **Failure Scenarios**: Automatic failover sequences
6. **Deployment**: Multi-region and Kubernetes architectures

These diagrams should be used in conjunction with the main architecture specification document for a complete understanding of the system design.

---

**Document Version:** 1.0
**Date:** 2025-11-10
**Status:** Complete
