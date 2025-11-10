# Storage Layer Requirements: LLM Auto-Optimizer

**Version:** 1.0
**Status:** Requirements Definition
**Last Updated:** 2025-11-10
**Authors:** LLM Auto-Optimizer Team

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [System Overview and Purpose](#2-system-overview-and-purpose)
3. [Storage Backend Requirements](#3-storage-backend-requirements)
4. [Data Models](#4-data-models)
5. [Storage Layer Architecture](#5-storage-layer-architecture)
6. [Multi-Backend Strategy](#6-multi-backend-strategy)
7. [Performance Requirements](#7-performance-requirements)
8. [Consistency and Durability](#8-consistency-and-durability)
9. [Backup and Recovery](#9-backup-and-recovery)
10. [Testing Requirements](#10-testing-requirements)
11. [Acceptance Criteria](#11-acceptance-criteria)
12. [Appendix](#12-appendix)

---

## 1. Executive Summary

### 1.1 Purpose

The Storage Layer is the foundational data persistence subsystem for the LLM Auto-Optimizer platform. It provides a unified abstraction over multiple storage backends (PostgreSQL, Redis, Sled) to support diverse data access patterns while maintaining ACID guarantees, high availability, and optimal performance across all system components.

### 1.2 Business Drivers

**Cost Optimization**: 30-60% reduction in LLM operational costs through intelligent configuration optimization
**Performance**: Sub-5-minute optimization cycles with <1-second decision latency (p99)
**Reliability**: 99.9% system availability with automatic failover and recovery
**Scale**: Support 10,000+ events/second ingestion across distributed deployments

### 1.3 Key Requirements Summary

| Requirement | Target | Priority |
|------------|--------|----------|
| **Write Throughput** | 10,000+ writes/sec | P0 |
| **Read Latency (p95)** | <10ms (Redis), <50ms (PostgreSQL) | P0 |
| **Data Durability** | 99.999% (PostgreSQL primary) | P0 |
| **Availability** | 99.9% uptime | P0 |
| **Multi-Backend Support** | PostgreSQL + Redis + Sled | P0 |
| **ACID Compliance** | Full ACID for PostgreSQL | P0 |
| **Backup Recovery Time** | <30 minutes (RTO) | P1 |
| **Point-in-Time Recovery** | Last 30 days | P1 |

### 1.4 Scope

**In Scope**:
- Multi-backend storage abstraction (PostgreSQL, Redis, Sled)
- Time-series metrics storage and retrieval
- Decision and experiment lifecycle management
- Configuration versioning and snapshots
- Distributed state management with locking
- Backup and recovery mechanisms
- Connection pooling and caching strategies
- Schema migrations and version management

**Out of Scope**:
- Real-time streaming analytics (handled by Stream Processor)
- ML model storage (handled by Decision Engine)
- Object/blob storage for large artifacts
- Graph database functionality
- Full-text search indexes

---

## 2. System Overview and Purpose

### 2.1 Role in LLM Auto-Optimizer Architecture

The Storage Layer sits at the foundation of the LLM Auto-Optimizer system, serving as the persistence layer for all components:

```
┌─────────────────────────────────────────────────────────────────┐
│                  LLM Auto-Optimizer Components                   │
├─────────────┬──────────────┬──────────────┬─────────────────────┤
│  Collector  │  Processor   │   Analyzer   │  Decision Engine    │
│  (Metrics)  │  (Windows)   │  (Insights)  │  (Optimization)     │
└──────┬──────┴──────┬───────┴──────┬───────┴──────┬──────────────┘
       │             │              │              │
       └─────────────┴──────────────┴──────────────┘
                           │
                           ▼
       ┌────────────────────────────────────────────────┐
       │            Storage Layer (This Document)        │
       ├────────────────────────────────────────────────┤
       │  Unified Storage API & Multi-Backend Manager   │
       └──────┬─────────────┬─────────────┬─────────────┘
              │             │             │
       ┌──────▼──────┐ ┌───▼────┐  ┌────▼─────┐
       │ PostgreSQL  │ │ Redis  │  │   Sled   │
       │  (Primary)  │ │(Cache) │  │ (Local)  │
       └─────────────┘ └────────┘  └──────────┘
```

### 2.2 Design Principles

#### 2.2.1 Multi-Backend Abstraction
- **Unified Interface**: Single API for all storage operations regardless of backend
- **Backend Selection**: Automatic routing based on data type and access pattern
- **Polyglot Persistence**: Use the right tool for each job

#### 2.2.2 Performance-First Design
- **Read Optimization**: Multi-tier caching with Redis in front of PostgreSQL
- **Write Optimization**: Batch operations, connection pooling, async I/O
- **Query Optimization**: Indexes, materialized views, query planning

#### 2.2.3 Reliability and Safety
- **ACID Guarantees**: Full transactional support for critical data
- **Automatic Retries**: Exponential backoff for transient failures
- **Circuit Breakers**: Fail fast to prevent cascade failures
- **Health Checks**: Continuous monitoring of backend connectivity

#### 2.2.4 Operational Excellence
- **Observable**: Rich metrics, logging, and tracing
- **Maintainable**: Clear abstractions, comprehensive documentation
- **Testable**: Full test coverage with integration tests
- **Scalable**: Horizontal scaling for all backends

### 2.3 Data Classification

The Storage Layer handles four primary data categories:

| Data Category | Storage Backend | Access Pattern | Retention | ACID Required |
|--------------|----------------|----------------|-----------|---------------|
| **Metrics** | PostgreSQL + Redis cache | High write, time-range reads | 90 days | No |
| **Decisions** | PostgreSQL | CRUD, versioned | Indefinite | Yes |
| **Experiments** | PostgreSQL | CRUD, statistical queries | 1 year | Yes |
| **State** | Redis + PostgreSQL backup | High read/write, low latency | Session-based | Partial |
| **Configuration** | PostgreSQL + Redis cache | Read-heavy, versioned | Indefinite | Yes |
| **Snapshots** | PostgreSQL | Write-once, read-rarely | 30 days | Yes |

---

## 3. Storage Backend Requirements

### 3.1 PostgreSQL: Primary Persistent Storage

#### 3.1.1 Overview

PostgreSQL serves as the primary durable storage for all critical system data requiring ACID guarantees, complex queries, and long-term retention.

#### 3.1.2 Functional Requirements

**FR-PG-001**: PostgreSQL shall store all time-series metrics data with nanosecond timestamp precision
**Priority**: P0
**Rationale**: Required for accurate performance tracking and trend analysis

**FR-PG-002**: PostgreSQL shall maintain complete decision lifecycle history with immutable audit trail
**Priority**: P0
**Rationale**: Compliance, debugging, and optimization feedback loops

**FR-PG-003**: PostgreSQL shall support full-text search on decision rationales and metadata
**Priority**: P1
**Rationale**: Operational debugging and analysis

**FR-PG-004**: PostgreSQL shall enforce foreign key constraints across all relational tables
**Priority**: P0
**Rationale**: Data integrity and consistency

**FR-PG-005**: PostgreSQL shall support JSON/JSONB columns for flexible metadata storage
**Priority**: P0
**Rationale**: Extensibility without schema migrations

#### 3.1.3 Schema Requirements

**Core Tables**:

```sql
-- Metrics: Time-series performance data
CREATE TABLE metrics (
    id BIGSERIAL PRIMARY KEY,
    timestamp TIMESTAMPTZ NOT NULL,
    service_id VARCHAR(255) NOT NULL,
    metric_name VARCHAR(100) NOT NULL,
    metric_value DOUBLE PRECISION NOT NULL,
    unit VARCHAR(50) NOT NULL,
    tags JSONB NOT NULL DEFAULT '{}',
    aggregation_type VARCHAR(50),
    aggregation_data JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_metrics_timestamp ON metrics USING BRIN (timestamp);
CREATE INDEX idx_metrics_service_name ON metrics (service_id, metric_name);
CREATE INDEX idx_metrics_tags ON metrics USING GIN (tags);

-- Decisions: Optimization decision records
CREATE TABLE decisions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    strategy VARCHAR(50) NOT NULL,
    target_services TEXT[] NOT NULL,
    changes JSONB NOT NULL,
    rationale TEXT NOT NULL,
    expected_impact JSONB NOT NULL,
    constraints JSONB NOT NULL DEFAULT '[]',
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    deployed_at TIMESTAMPTZ,
    rolled_back_at TIMESTAMPTZ,
    actual_impact JSONB,
    metadata JSONB NOT NULL DEFAULT '{}',
    version INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX idx_decisions_created_at ON decisions (created_at DESC);
CREATE INDEX idx_decisions_status ON decisions (status);
CREATE INDEX idx_decisions_strategy ON decisions (strategy);
CREATE INDEX idx_decisions_services ON decisions USING GIN (target_services);
CREATE INDEX idx_decisions_metadata ON decisions USING GIN (metadata);

-- Experiments: A/B test and experiment tracking
CREATE TABLE experiments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    status VARCHAR(50) NOT NULL DEFAULT 'draft',
    variants JSONB NOT NULL,
    traffic_allocation JSONB NOT NULL,
    metrics JSONB NOT NULL DEFAULT '[]',
    start_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    end_time TIMESTAMPTZ,
    results JSONB,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_experiments_status ON experiments (status);
CREATE INDEX idx_experiments_start_time ON experiments (start_time DESC);

-- Variant Results: Detailed experiment variant performance
CREATE TABLE variant_results (
    id BIGSERIAL PRIMARY KEY,
    experiment_id UUID NOT NULL REFERENCES experiments(id) ON DELETE CASCADE,
    variant_id UUID NOT NULL,
    variant_name VARCHAR(100) NOT NULL,
    total_requests BIGINT NOT NULL DEFAULT 0,
    conversions BIGINT NOT NULL DEFAULT 0,
    avg_quality DOUBLE PRECISION,
    avg_cost DOUBLE PRECISION,
    avg_latency_ms DOUBLE PRECISION,
    metrics JSONB NOT NULL DEFAULT '{}',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(experiment_id, variant_id)
);

CREATE INDEX idx_variant_results_experiment ON variant_results (experiment_id);

-- Deployments: Configuration deployment history
CREATE TABLE deployments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    decision_id UUID NOT NULL REFERENCES decisions(id),
    deployed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deployed_by VARCHAR(255) NOT NULL,
    deployment_type VARCHAR(50) NOT NULL, -- 'canary', 'blue_green', 'immediate'
    traffic_percentage INTEGER NOT NULL DEFAULT 100,
    rollback_threshold JSONB,
    status VARCHAR(50) NOT NULL DEFAULT 'deploying',
    completed_at TIMESTAMPTZ,
    rolled_back_at TIMESTAMPTZ,
    rollback_reason TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'
);

CREATE INDEX idx_deployments_decision ON deployments (decision_id);
CREATE INDEX idx_deployments_status ON deployments (status);
CREATE INDEX idx_deployments_deployed_at ON deployments (deployed_at DESC);

-- Configuration Snapshots: Point-in-time configuration backups
CREATE TABLE configuration_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    snapshot_timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    service_id VARCHAR(255) NOT NULL,
    configuration JSONB NOT NULL,
    checksum VARCHAR(64) NOT NULL,
    parent_snapshot_id UUID REFERENCES configuration_snapshots(id),
    created_by VARCHAR(255),
    description TEXT,
    tags JSONB NOT NULL DEFAULT '{}'
);

CREATE INDEX idx_config_snapshots_timestamp ON configuration_snapshots (snapshot_timestamp DESC);
CREATE INDEX idx_config_snapshots_service ON configuration_snapshots (service_id);

-- Model Performance Profiles: Historical model performance data
CREATE TABLE model_performance_profiles (
    id BIGSERIAL PRIMARY KEY,
    model_id VARCHAR(255) NOT NULL,
    provider VARCHAR(100) NOT NULL,
    avg_latency_ms DOUBLE PRECISION NOT NULL,
    p50_latency_ms DOUBLE PRECISION NOT NULL,
    p95_latency_ms DOUBLE PRECISION NOT NULL,
    p99_latency_ms DOUBLE PRECISION NOT NULL,
    cost_per_1k_tokens DOUBLE PRECISION NOT NULL,
    quality_score DOUBLE PRECISION NOT NULL CHECK (quality_score >= 0 AND quality_score <= 1),
    error_rate DOUBLE PRECISION NOT NULL CHECK (error_rate >= 0 AND error_rate <= 1),
    throughput_qps DOUBLE PRECISION NOT NULL,
    sample_size BIGINT NOT NULL,
    measurement_period_start TIMESTAMPTZ NOT NULL,
    measurement_period_end TIMESTAMPTZ NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_model_profiles_model_provider ON model_performance_profiles (model_id, provider);
CREATE INDEX idx_model_profiles_created_at ON model_performance_profiles (created_at DESC);

-- Analyzer Insights: ML-derived insights and recommendations
CREATE TABLE analyzer_insights (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    insight_type VARCHAR(100) NOT NULL, -- 'drift', 'anomaly', 'trend', 'recommendation'
    severity VARCHAR(50) NOT NULL, -- 'info', 'warning', 'critical'
    service_id VARCHAR(255) NOT NULL,
    metric_name VARCHAR(100),
    detected_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    time_range_start TIMESTAMPTZ NOT NULL,
    time_range_end TIMESTAMPTZ NOT NULL,
    confidence DOUBLE PRECISION NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
    description TEXT NOT NULL,
    recommendations JSONB,
    affected_metrics JSONB,
    metadata JSONB NOT NULL DEFAULT '{}',
    acknowledged BOOLEAN NOT NULL DEFAULT FALSE,
    acknowledged_at TIMESTAMPTZ,
    acknowledged_by VARCHAR(255)
);

CREATE INDEX idx_insights_detected_at ON analyzer_insights (detected_at DESC);
CREATE INDEX idx_insights_service ON analyzer_insights (service_id);
CREATE INDEX idx_insights_type_severity ON analyzer_insights (insight_type, severity);
CREATE INDEX idx_insights_acknowledged ON analyzer_insights (acknowledged) WHERE NOT acknowledged;
```

#### 3.1.4 Partitioning Strategy

**FR-PG-006**: Metrics table shall be partitioned by time range (monthly partitions)
**Priority**: P0
**Rationale**: Efficient data retention and query performance

```sql
-- Partitioning for metrics table
CREATE TABLE metrics (
    -- columns as above
) PARTITION BY RANGE (timestamp);

-- Create partitions for current and future months
CREATE TABLE metrics_2025_01 PARTITION OF metrics
    FOR VALUES FROM ('2025-01-01') TO ('2025-02-01');

CREATE TABLE metrics_2025_02 PARTITION OF metrics
    FOR VALUES FROM ('2025-02-01') TO ('2025-03-01');

-- Automated partition creation function
CREATE OR REPLACE FUNCTION create_metrics_partition()
RETURNS void AS $$
DECLARE
    start_date DATE;
    end_date DATE;
    partition_name TEXT;
BEGIN
    start_date := DATE_TRUNC('month', CURRENT_DATE + INTERVAL '1 month');
    end_date := start_date + INTERVAL '1 month';
    partition_name := 'metrics_' || TO_CHAR(start_date, 'YYYY_MM');

    EXECUTE format(
        'CREATE TABLE IF NOT EXISTS %I PARTITION OF metrics FOR VALUES FROM (%L) TO (%L)',
        partition_name, start_date, end_date
    );
END;
$$ LANGUAGE plpgsql;
```

#### 3.1.5 Indexing Strategy

**FR-PG-007**: All timestamp columns shall have BRIN indexes for time-range queries
**Priority**: P0
**Rationale**: Time-range queries are the primary access pattern

**FR-PG-008**: JSONB columns shall have GIN indexes for key-value queries
**Priority**: P0
**Rationale**: Enable efficient filtering on metadata and tags

**FR-PG-009**: Foreign key columns shall have B-tree indexes
**Priority**: P0
**Rationale**: Optimize JOIN operations

#### 3.1.6 Materialized Views

**FR-PG-010**: Materialized views shall be created for frequently accessed aggregations
**Priority**: P1
**Rationale**: Reduce query latency for dashboard and reporting

```sql
-- Hourly metrics aggregation
CREATE MATERIALIZED VIEW metrics_hourly AS
SELECT
    DATE_TRUNC('hour', timestamp) AS hour,
    service_id,
    metric_name,
    AVG(metric_value) AS avg_value,
    MIN(metric_value) AS min_value,
    MAX(metric_value) AS max_value,
    COUNT(*) AS sample_count,
    PERCENTILE_CONT(0.50) WITHIN GROUP (ORDER BY metric_value) AS p50,
    PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY metric_value) AS p95,
    PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY metric_value) AS p99
FROM metrics
GROUP BY hour, service_id, metric_name;

CREATE UNIQUE INDEX idx_metrics_hourly_hour_service_metric
    ON metrics_hourly (hour, service_id, metric_name);

-- Refresh strategy
CREATE OR REPLACE FUNCTION refresh_metrics_hourly()
RETURNS void AS $$
BEGIN
    REFRESH MATERIALIZED VIEW CONCURRENTLY metrics_hourly;
END;
$$ LANGUAGE plpgsql;
```

#### 3.1.7 Connection Pooling

**FR-PG-011**: PostgreSQL connections shall use pooling with configurable min/max pool size
**Priority**: P0
**Rationale**: Optimize connection overhead and resource utilization

**Configuration**:
- Minimum connections: 5
- Maximum connections: 50
- Connection timeout: 30 seconds
- Idle timeout: 10 minutes
- Max lifetime: 1 hour

#### 3.1.8 Transaction Management

**FR-PG-012**: All multi-table operations shall be executed within transactions
**Priority**: P0
**Rationale**: Maintain data consistency

**FR-PG-013**: Long-running transactions (>5 seconds) shall be monitored and logged
**Priority**: P1
**Rationale**: Identify performance bottlenecks

#### 3.1.9 Non-Functional Requirements

**NFR-PG-001**: PostgreSQL shall support concurrent writes of 10,000+ rows/second
**NFR-PG-002**: Query latency (p95) shall be <50ms for simple queries, <500ms for complex aggregations
**NFR-PG-003**: Database size shall support 10TB+ with TimescaleDB extension for time-series optimization
**NFR-PG-004**: Replication lag shall be <1 second for read replicas

---

### 3.2 Redis: High-Performance Caching and Pub/Sub

#### 3.2.1 Overview

Redis provides low-latency caching, distributed state management, and pub/sub messaging for real-time system coordination.

#### 3.2.2 Functional Requirements

**FR-RD-001**: Redis shall cache frequently accessed data with configurable TTL
**Priority**: P0
**Rationale**: Reduce load on PostgreSQL and improve read latency

**FR-RD-002**: Redis shall support distributed locking for state coordination
**Priority**: P0
**Rationale**: Prevent race conditions in distributed deployments

**FR-RD-003**: Redis shall provide pub/sub channels for real-time event distribution
**Priority**: P0
**Rationale**: Enable reactive system behavior

**FR-RD-004**: Redis shall maintain session state for windowing operations
**Priority**: P0
**Rationale**: Stream processor state management

**FR-RD-005**: Redis shall support atomic operations (INCR, DECR, GET/SET)
**Priority**: P0
**Rationale**: Counters and rate limiting

#### 3.2.3 Data Structures and Use Cases

| Data Structure | Use Case | TTL Strategy | Example Key Pattern |
|----------------|----------|--------------|---------------------|
| **String** | Cached queries, counters | 5-60 minutes | `cache:decision:{id}` |
| **Hash** | User sessions, object cache | 30 minutes | `session:{id}` |
| **List** | Recent events, queues | 10 minutes | `events:recent:{service}` |
| **Set** | Unique tracking, deduplication | 24 hours | `seen:request_ids` |
| **Sorted Set** | Leaderboards, time-ordered data | 1 hour | `metrics:top_latency` |
| **Streams** | Event log, CDC | 7 days | `stream:metrics:{service}` |

#### 3.2.4 Key Naming Conventions

**FR-RD-006**: All Redis keys shall follow hierarchical naming: `{domain}:{entity}:{id}[:{attribute}]`
**Priority**: P0
**Rationale**: Namespace organization and key scanning

**Examples**:
```
cache:decision:550e8400-e29b-41d4-a716-446655440000
cache:experiment:ab-test-2025-01:results
state:window:tumbling:service-a:1640000000
lock:deployment:service-b:config-update
counter:requests:service-c:2025-01-10
pubsub:events:decision-deployed
metrics:latency:p95:service-d
```

#### 3.2.5 Caching Strategy

**Cache-Aside Pattern**:

```rust
async fn get_decision(id: Uuid, cache: &Redis, db: &PostgreSQL) -> Result<Decision> {
    let cache_key = format!("cache:decision:{}", id);

    // Try cache first
    if let Some(cached) = cache.get(&cache_key).await? {
        return Ok(serde_json::from_slice(&cached)?);
    }

    // Cache miss - query database
    let decision = db.query_decision(id).await?;

    // Populate cache with 30-minute TTL
    cache.set_ex(&cache_key, serde_json::to_vec(&decision)?, 1800).await?;

    Ok(decision)
}
```

**Cache Invalidation Strategy**:

**FR-RD-007**: Cache entries shall be invalidated on write operations
**Priority**: P0
**Rationale**: Maintain cache consistency

```rust
async fn update_decision(
    id: Uuid,
    decision: Decision,
    cache: &Redis,
    db: &PostgreSQL
) -> Result<()> {
    // Update database
    db.update_decision(id, &decision).await?;

    // Invalidate cache
    let cache_key = format!("cache:decision:{}", id);
    cache.del(&cache_key).await?;

    // Publish invalidation event
    cache.publish("events:cache-invalidation", &cache_key).await?;

    Ok(())
}
```

#### 3.2.6 Distributed Locking

**FR-RD-008**: Redis shall implement distributed locks using Redlock algorithm
**Priority**: P0
**Rationale**: Prevent concurrent modifications

```rust
// Distributed lock example
async fn acquire_lock(
    resource: &str,
    ttl: Duration,
    redis: &Redis
) -> Result<Option<String>> {
    let lock_id = Uuid::new_v4().to_string();
    let lock_key = format!("lock:{}", resource);

    // SET NX (only if not exists) with TTL
    let acquired = redis
        .set_nx(&lock_key, &lock_id, ttl.as_secs())
        .await?;

    if acquired {
        Ok(Some(lock_id))
    } else {
        Ok(None)
    }
}

async fn release_lock(
    resource: &str,
    lock_id: &str,
    redis: &Redis
) -> Result<bool> {
    let lock_key = format!("lock:{}", resource);

    // Lua script for atomic check-and-delete
    let script = r#"
        if redis.call("GET", KEYS[1]) == ARGV[1] then
            return redis.call("DEL", KEYS[1])
        else
            return 0
        end
    "#;

    let deleted = redis.eval(script, &[&lock_key], &[lock_id]).await?;
    Ok(deleted == 1)
}
```

#### 3.2.7 Pub/Sub Channels

**FR-RD-009**: Redis shall provide pub/sub for system-wide events
**Priority**: P0
**Rationale**: Decouple components with event-driven architecture

**Channel Naming**:
```
events:decision-created
events:decision-deployed
events:decision-rolled-back
events:experiment-started
events:experiment-completed
events:metric-threshold-exceeded
events:cache-invalidation
events:configuration-updated
```

**Event Payload Structure**:
```json
{
  "event_type": "decision_deployed",
  "timestamp": "2025-01-10T14:30:00Z",
  "source": "actuator-service",
  "decision_id": "550e8400-e29b-41d4-a716-446655440000",
  "service_ids": ["service-a", "service-b"],
  "metadata": {
    "deployment_type": "canary",
    "traffic_percentage": 10
  }
}
```

#### 3.2.8 Connection Pooling

**FR-RD-010**: Redis connections shall use multiplexing with single connection manager
**Priority**: P0
**Rationale**: Redis protocol supports pipelining and multiplexing

**Configuration**:
- Connection mode: Multiplexed
- Pipeline size: 100 commands
- Reconnection attempts: 3 with exponential backoff
- Command timeout: 3 seconds

#### 3.2.9 Persistence Configuration

**FR-RD-011**: Redis shall use RDB + AOF hybrid persistence
**Priority**: P0
**Rationale**: Balance durability and performance

**RDB Configuration**:
```
save 900 1      # Save after 15 minutes if 1 key changed
save 300 10     # Save after 5 minutes if 10 keys changed
save 60 10000   # Save after 1 minute if 10000 keys changed
```

**AOF Configuration**:
```
appendonly yes
appendfsync everysec  # Fsync every second (balanced durability/performance)
auto-aof-rewrite-percentage 100
auto-aof-rewrite-min-size 64mb
```

#### 3.2.9 Non-Functional Requirements

**NFR-RD-001**: Redis operations shall have <1ms latency (p95)
**NFR-RD-002**: Cache hit rate shall exceed 80% for frequently accessed data
**NFR-RD-003**: Redis shall support 100,000+ operations per second
**NFR-RD-004**: Memory usage shall not exceed 16GB per instance
**NFR-RD-005**: Redis shall support cluster mode for horizontal scaling (>32 nodes)

---

### 3.3 Sled: Embedded Local Storage

#### 3.3.1 Overview

Sled is an embedded key-value store providing fast local persistence for single-node deployments and edge cases where network storage is unavailable.

#### 3.3.2 Functional Requirements

**FR-SL-001**: Sled shall provide embedded storage for single-node deployments
**Priority**: P1
**Rationale**: Simplify deployment for edge and development environments

**FR-SL-002**: Sled shall maintain local state for stream processor windows
**Priority**: P1
**Rationale**: Fast local access for windowing operations

**FR-SL-003**: Sled shall support atomic batch operations
**Priority**: P1
**Rationale**: Consistent multi-key updates

**FR-SL-004**: Sled shall provide snapshot and restore capabilities
**Priority**: P1
**Rationale**: Backup and disaster recovery

#### 3.3.3 Use Cases

| Use Case | Description | Data Size | Access Pattern |
|----------|-------------|-----------|----------------|
| **Development Mode** | Local testing without external dependencies | <1GB | Read/Write mixed |
| **Edge Deployment** | Single-node deployments without network storage | <10GB | Read-heavy |
| **Window State** | Temporary windowing state for stream processor | <100MB | Write-heavy |
| **Configuration Cache** | Local copy of configuration for fast startup | <10MB | Read-only |

#### 3.3.4 Key Encoding

**FR-SL-005**: Sled keys shall use binary encoding for efficiency
**Priority**: P1
**Rationale**: Reduce storage overhead

```rust
// Key encoding strategy
struct SledKey {
    prefix: u8,      // Key type discriminator
    entity_id: [u8; 16], // UUID or hash
    timestamp: u64,  // Optional timestamp for time-ordered keys
}

impl SledKey {
    fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(25);
        buf.push(self.prefix);
        buf.extend_from_slice(&self.entity_id);
        buf.extend_from_slice(&self.timestamp.to_be_bytes());
        buf
    }
}

// Key prefixes
const PREFIX_METRIC: u8 = 0x01;
const PREFIX_DECISION: u8 = 0x02;
const PREFIX_CONFIG: u8 = 0x03;
const PREFIX_WINDOW_STATE: u8 = 0x04;
```

#### 3.3.5 Atomic Batch Operations

**FR-SL-006**: Sled shall support transactional batches with commit/abort
**Priority**: P1
**Rationale**: Maintain consistency for related updates

```rust
async fn batch_update_window_state(
    window_id: &str,
    updates: Vec<(Vec<u8>, Vec<u8>)>,
    db: &sled::Db
) -> Result<()> {
    let mut batch = sled::Batch::default();

    for (key, value) in updates {
        batch.insert(key, value);
    }

    db.apply_batch(batch)?;
    db.flush_async().await?;

    Ok(())
}
```

#### 3.3.6 Snapshot and Recovery

**FR-SL-007**: Sled shall support filesystem-level snapshots
**Priority**: P1
**Rationale**: Point-in-time recovery for local state

```rust
async fn create_snapshot(db: &sled::Db, path: &Path) -> Result<()> {
    // Flush in-memory data
    db.flush_async().await?;

    // Create filesystem snapshot
    let snapshot = db.export();

    // Write to snapshot file
    tokio::fs::create_dir_all(path.parent().unwrap()).await?;
    let mut file = tokio::fs::File::create(path).await?;
    file.write_all(&snapshot).await?;

    Ok(())
}
```

#### 3.3.7 Configuration

**FR-SL-008**: Sled shall be configurable for different performance profiles
**Priority**: P1
**Rationale**: Optimize for different deployment scenarios

**Performance Profiles**:

```rust
// Development: Safety-first
sled::Config::new()
    .path("data/dev.sled")
    .cache_capacity(64 * 1024 * 1024)  // 64MB cache
    .flush_every_ms(Some(100))         // Flush every 100ms
    .mode(sled::Mode::HighThroughput);

// Production: Balanced
sled::Config::new()
    .path("data/prod.sled")
    .cache_capacity(512 * 1024 * 1024) // 512MB cache
    .flush_every_ms(Some(1000))        // Flush every 1s
    .mode(sled::Mode::LowSpace);

// Edge: Performance-first
sled::Config::new()
    .path("data/edge.sled")
    .cache_capacity(256 * 1024 * 1024) // 256MB cache
    .flush_every_ms(Some(5000))        // Flush every 5s
    .mode(sled::Mode::HighThroughput);
```

#### 3.3.8 Non-Functional Requirements

**NFR-SL-001**: Sled operations shall have <100µs latency (p95)
**NFR-SL-002**: Sled database size shall not exceed 10GB
**NFR-SL-003**: Sled shall support 10,000+ operations per second on SSD
**NFR-SL-004**: Crash recovery shall complete in <5 seconds

---

## 4. Data Models

### 4.1 Metrics and Time-Series Data

#### 4.1.1 Metric Event Structure

```rust
/// Raw metric event from collector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricEvent {
    /// Unique event identifier (for deduplication)
    pub id: Uuid,

    /// Timestamp with nanosecond precision
    pub timestamp: DateTime<Utc>,

    /// Service identifier
    pub service_id: String,

    /// Metric name (e.g., "latency", "cost", "error_rate")
    pub metric_name: String,

    /// Metric value
    pub value: f64,

    /// Unit of measurement
    pub unit: String,

    /// Tags for dimensionality
    pub tags: HashMap<String, String>,

    /// Optional aggregation metadata
    pub aggregation: Option<AggregationMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregationMetadata {
    pub aggregation_type: AggregationType,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub sample_count: u64,
    pub min: f64,
    pub max: f64,
    pub sum: f64,
    pub percentiles: HashMap<u8, f64>, // p50, p95, p99
}
```

#### 4.1.2 Storage Schema Mapping

**PostgreSQL**:
```sql
INSERT INTO metrics (
    timestamp, service_id, metric_name, metric_value, unit, tags,
    aggregation_type, aggregation_data
) VALUES (
    $1, $2, $3, $4, $5, $6::jsonb, $7, $8::jsonb
);
```

**Redis Cache**:
```
Key: metrics:latest:{service_id}:{metric_name}
Value: JSON serialized MetricEvent
TTL: 300 seconds (5 minutes)
```

#### 4.1.3 Query Patterns

**Time Range Query**:
```rust
async fn query_metrics(
    service_id: &str,
    metric_name: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    db: &PostgreSQL
) -> Result<Vec<MetricEvent>> {
    db.query(
        "SELECT * FROM metrics
         WHERE service_id = $1
           AND metric_name = $2
           AND timestamp >= $3
           AND timestamp < $4
         ORDER BY timestamp ASC",
        &[service_id, metric_name, &start, &end]
    ).await
}
```

**Aggregated Query**:
```rust
async fn query_metrics_aggregated(
    service_id: &str,
    metric_name: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    granularity: Granularity,
    db: &PostgreSQL
) -> Result<Vec<AggregatedMetric>> {
    let interval = match granularity {
        Granularity::Minute => "1 minute",
        Granularity::Hour => "1 hour",
        Granularity::Day => "1 day",
    };

    db.query(
        &format!("
            SELECT
                DATE_TRUNC($5, timestamp) as bucket,
                AVG(metric_value) as avg,
                MIN(metric_value) as min,
                MAX(metric_value) as max,
                COUNT(*) as count,
                PERCENTILE_CONT(0.50) WITHIN GROUP (ORDER BY metric_value) as p50,
                PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY metric_value) as p95,
                PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY metric_value) as p99
            FROM metrics
            WHERE service_id = $1
              AND metric_name = $2
              AND timestamp >= $3
              AND timestamp < $4
            GROUP BY bucket
            ORDER BY bucket ASC
        "),
        &[service_id, metric_name, &start, &end, &interval]
    ).await
}
```

---

### 4.2 Decisions and Outcomes

#### 4.2.1 Decision Record Structure

```rust
/// Optimization decision record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    /// Unique identifier
    pub id: Uuid,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Optimization strategy
    pub strategy: OptimizationStrategy,

    /// Target services
    pub target_services: Vec<String>,

    /// Configuration changes
    pub changes: Vec<ConfigurationChange>,

    /// Decision rationale
    pub rationale: String,

    /// Expected impact
    pub expected_impact: ExpectedImpact,

    /// Constraints
    pub constraints: Vec<Constraint>,

    /// Current status
    pub status: DecisionStatus,

    /// Deployment timestamp
    pub deployed_at: Option<DateTime<Utc>>,

    /// Rollback timestamp
    pub rolled_back_at: Option<DateTime<Utc>>,

    /// Actual impact (measured post-deployment)
    pub actual_impact: Option<ActualImpact>,

    /// Additional metadata
    pub metadata: HashMap<String, String>,

    /// Version for optimistic locking
    pub version: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigurationChange {
    pub parameter: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: serde_json::Value,
    pub change_type: ChangeType, // Replace, Add, Remove, Update
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedImpact {
    pub cost_reduction_pct: f64,
    pub quality_delta_pct: f64,
    pub latency_delta_pct: f64,
    pub confidence: f64, // 0.0-1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActualImpact {
    pub cost_reduction_pct: f64,
    pub quality_delta_pct: f64,
    pub latency_delta_pct: f64,
    pub requests_affected: u64,
    pub measured_from: DateTime<Utc>,
    pub measured_until: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DecisionStatus {
    Pending,
    Validating,
    ValidationFailed,
    Deploying,
    Deployed,
    DeploymentFailed,
    Monitoring,
    RolledBack,
    Completed,
    Cancelled,
}
```

#### 4.2.2 Decision Lifecycle State Machine

```
[Pending]
    ↓ (validate)
[Validating]
    ↓ (validation passed)     ↓ (validation failed)
[Deploying]                [ValidationFailed]
    ↓ (deployed)              ↓ (manual override)
[Deployed]                 [Cancelled]
    ↓ (start monitoring)
[Monitoring]
    ↓ (degradation detected)  ↓ (success confirmed)
[RolledBack]               [Completed]
```

#### 4.2.3 Query Patterns

**Retrieve Decision by ID**:
```rust
async fn get_decision(id: Uuid, storage: &StorageLayer) -> Result<Decision> {
    // Try cache first
    if let Some(cached) = storage.redis.get_decision(id).await? {
        return Ok(cached);
    }

    // Query database
    let decision = storage.postgres.query_one(
        "SELECT * FROM decisions WHERE id = $1",
        &[&id]
    ).await?;

    // Populate cache
    storage.redis.cache_decision(&decision, Duration::from_secs(1800)).await?;

    Ok(decision)
}
```

**List Recent Decisions**:
```rust
async fn list_recent_decisions(
    service_id: Option<&str>,
    status: Option<DecisionStatus>,
    limit: usize,
    storage: &StorageLayer
) -> Result<Vec<Decision>> {
    let mut query = String::from("SELECT * FROM decisions WHERE 1=1");
    let mut params: Vec<Box<dyn postgres::types::ToSql + Sync>> = Vec::new();

    if let Some(service) = service_id {
        query.push_str(&format!(" AND $1 = ANY(target_services)"));
        params.push(Box::new(service.to_string()));
    }

    if let Some(s) = status {
        query.push_str(&format!(" AND status = ${}", params.len() + 1));
        params.push(Box::new(s.to_string()));
    }

    query.push_str(&format!(" ORDER BY created_at DESC LIMIT {}", limit));

    storage.postgres.query(&query, &params).await
}
```

**Update Decision Status**:
```rust
async fn update_decision_status(
    id: Uuid,
    new_status: DecisionStatus,
    storage: &StorageLayer
) -> Result<()> {
    // Update database
    storage.postgres.execute(
        "UPDATE decisions SET status = $1, version = version + 1 WHERE id = $2",
        &[&new_status, &id]
    ).await?;

    // Invalidate cache
    storage.redis.delete_cached_decision(id).await?;

    // Publish event
    storage.redis.publish(
        "events:decision-status-changed",
        &serde_json::json!({
            "decision_id": id,
            "new_status": new_status,
            "timestamp": Utc::now()
        })
    ).await?;

    Ok(())
}
```

---

### 4.3 Deployments and Rollbacks

#### 4.3.1 Deployment Record Structure

```rust
/// Deployment record for canary and blue-green deployments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deployment {
    /// Unique deployment identifier
    pub id: Uuid,

    /// Associated decision
    pub decision_id: Uuid,

    /// Deployment timestamp
    pub deployed_at: DateTime<Utc>,

    /// User or system that initiated deployment
    pub deployed_by: String,

    /// Deployment type
    pub deployment_type: DeploymentType,

    /// Traffic percentage (for canary deployments)
    pub traffic_percentage: u8, // 0-100

    /// Rollback threshold criteria
    pub rollback_threshold: RollbackThreshold,

    /// Current status
    pub status: DeploymentStatus,

    /// Completion timestamp
    pub completed_at: Option<DateTime<Utc>>,

    /// Rollback timestamp
    pub rolled_back_at: Option<DateTime<Utc>>,

    /// Rollback reason
    pub rollback_reason: Option<String>,

    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DeploymentType {
    Immediate,
    Canary,
    BlueGreen,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackThreshold {
    pub error_rate_pct: f64,
    pub latency_increase_pct: f64,
    pub quality_drop_pct: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DeploymentStatus {
    Deploying,
    Active,
    Monitoring,
    Promoting, // Canary → 100%
    Completed,
    RollingBack,
    RolledBack,
    Failed,
}
```

#### 4.3.2 Canary Deployment Tracking

```rust
/// Track canary deployment progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanaryProgress {
    pub deployment_id: Uuid,
    pub current_traffic_pct: u8,
    pub target_traffic_pct: u8,
    pub increment_pct: u8,
    pub increment_interval_minutes: u32,
    pub last_incremented_at: DateTime<Utc>,
    pub metrics: CanaryMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanaryMetrics {
    pub canary_error_rate: f64,
    pub baseline_error_rate: f64,
    pub canary_avg_latency_ms: f64,
    pub baseline_avg_latency_ms: f64,
    pub canary_avg_quality: f64,
    pub baseline_avg_quality: f64,
    pub sample_size: u64,
}
```

#### 4.3.3 Rollback Record Structure

```rust
/// Rollback execution record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rollback {
    pub id: Uuid,
    pub deployment_id: Uuid,
    pub triggered_at: DateTime<Utc>,
    pub triggered_by: RollbackTrigger,
    pub reason: String,
    pub previous_config: serde_json::Value,
    pub status: RollbackStatus,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RollbackTrigger {
    Automatic,      // Threshold exceeded
    Manual,         // User-initiated
    HealthCheck,    // Service health degraded
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum RollbackStatus {
    Initiated,
    InProgress,
    Completed,
    Failed,
}
```

---

### 4.4 Configurations and Snapshots

#### 4.4.1 Configuration Snapshot Structure

```rust
/// Point-in-time configuration snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigurationSnapshot {
    /// Unique snapshot identifier
    pub id: Uuid,

    /// Snapshot timestamp
    pub snapshot_timestamp: DateTime<Utc>,

    /// Service identifier
    pub service_id: String,

    /// Complete configuration state
    pub configuration: ServiceConfiguration,

    /// SHA-256 checksum for integrity
    pub checksum: String,

    /// Parent snapshot (for delta tracking)
    pub parent_snapshot_id: Option<Uuid>,

    /// Creator
    pub created_by: String,

    /// Description
    pub description: String,

    /// Tags for categorization
    pub tags: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfiguration {
    /// Model configuration
    pub model: ModelConfig,

    /// Rate limiting settings
    pub rate_limits: RateLimitConfig,

    /// Circuit breaker settings
    pub circuit_breaker: CircuitBreakerConfig,

    /// Retry policy
    pub retry_policy: RetryPolicy,

    /// Timeout settings
    pub timeouts: TimeoutConfig,

    /// Feature flags
    pub features: HashMap<String, bool>,

    /// Custom parameters
    pub custom: serde_json::Value,
}
```

#### 4.4.2 Configuration Versioning

**Version Graph**:
```
snapshot-1 (baseline)
    ↓
snapshot-2 (model change)
    ↓
snapshot-3 (temperature adjustment)
    ├─→ snapshot-4 (experiment branch)
    └─→ snapshot-5 (production branch)
```

**Delta Calculation**:
```rust
async fn compute_config_delta(
    from_snapshot_id: Uuid,
    to_snapshot_id: Uuid,
    storage: &StorageLayer
) -> Result<ConfigDelta> {
    let from = storage.get_snapshot(from_snapshot_id).await?;
    let to = storage.get_snapshot(to_snapshot_id).await?;

    let changes = vec![];

    // Compare model config
    if from.configuration.model != to.configuration.model {
        changes.push(ConfigChange {
            path: "model".to_string(),
            old_value: serde_json::to_value(&from.configuration.model)?,
            new_value: serde_json::to_value(&to.configuration.model)?,
        });
    }

    // ... compare other fields

    Ok(ConfigDelta {
        from_snapshot_id,
        to_snapshot_id,
        changes,
        timestamp: Utc::now(),
    })
}
```

#### 4.4.3 Snapshot Cleanup Policy

**FR-CFG-001**: Configuration snapshots shall be retained according to policy
**Priority**: P1
**Rationale**: Balance storage costs with recovery requirements

**Retention Policy**:
- Keep all snapshots from last 7 days
- Keep daily snapshots for last 30 days
- Keep weekly snapshots for last 90 days
- Keep monthly snapshots indefinitely

```sql
-- Snapshot cleanup query
DELETE FROM configuration_snapshots
WHERE snapshot_timestamp < NOW() - INTERVAL '7 days'
  AND id NOT IN (
    -- Keep one snapshot per day for last 30 days
    SELECT DISTINCT ON (DATE_TRUNC('day', snapshot_timestamp)) id
    FROM configuration_snapshots
    WHERE snapshot_timestamp >= NOW() - INTERVAL '30 days'
      AND snapshot_timestamp < NOW() - INTERVAL '7 days'
    ORDER BY DATE_TRUNC('day', snapshot_timestamp), snapshot_timestamp DESC
  )
  AND id NOT IN (
    -- Keep one snapshot per week for last 90 days
    SELECT DISTINCT ON (DATE_TRUNC('week', snapshot_timestamp)) id
    FROM configuration_snapshots
    WHERE snapshot_timestamp >= NOW() - INTERVAL '90 days'
      AND snapshot_timestamp < NOW() - INTERVAL '30 days'
    ORDER BY DATE_TRUNC('week', snapshot_timestamp), snapshot_timestamp DESC
  );
```

---

### 4.5 Analyzer Insights and Recommendations

#### 4.5.1 Insight Record Structure

```rust
/// Analyzer-generated insight or anomaly
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzerInsight {
    /// Unique insight identifier
    pub id: Uuid,

    /// Insight type
    pub insight_type: InsightType,

    /// Severity level
    pub severity: Severity,

    /// Affected service
    pub service_id: String,

    /// Related metric (if applicable)
    pub metric_name: Option<String>,

    /// Detection timestamp
    pub detected_at: DateTime<Utc>,

    /// Time range of analysis
    pub time_range_start: DateTime<Utc>,
    pub time_range_end: DateTime<Utc>,

    /// Confidence score
    pub confidence: f64, // 0.0-1.0

    /// Human-readable description
    pub description: String,

    /// Actionable recommendations
    pub recommendations: Vec<Recommendation>,

    /// Affected metrics details
    pub affected_metrics: serde_json::Value,

    /// Additional metadata
    pub metadata: HashMap<String, String>,

    /// Acknowledgement status
    pub acknowledged: bool,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub acknowledged_by: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum InsightType {
    Drift,          // Model performance degradation
    Anomaly,        // Unusual pattern detected
    Trend,          // Sustained directional change
    Threshold,      // Metric exceeded threshold
    Recommendation, // Optimization opportunity
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub action: String,
    pub expected_impact: String,
    pub implementation: serde_json::Value,
    pub priority: u8, // 1-5
}
```

#### 4.5.2 Drift Detection Record

```rust
/// Model performance drift detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftDetection {
    pub insight_id: Uuid,
    pub service_id: String,
    pub model_id: String,
    pub drift_type: DriftType,
    pub baseline_period: DateRange,
    pub detection_period: DateRange,
    pub drift_magnitude: f64,
    pub statistical_significance: f64, // p-value
    pub metrics: DriftMetrics,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DriftType {
    QualityDegradation,
    LatencyIncrease,
    ErrorRateIncrease,
    CostIncrease,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftMetrics {
    pub baseline_mean: f64,
    pub current_mean: f64,
    pub change_pct: f64,
    pub baseline_stddev: f64,
    pub current_stddev: f64,
}
```

#### 4.5.3 Query Patterns

**Recent Unacknowledged Insights**:
```rust
async fn query_unacknowledged_insights(
    service_id: Option<&str>,
    severity: Option<Severity>,
    limit: usize,
    storage: &StorageLayer
) -> Result<Vec<AnalyzerInsight>> {
    let mut query = String::from(
        "SELECT * FROM analyzer_insights WHERE acknowledged = FALSE"
    );

    // Add filters...

    query.push_str(" ORDER BY detected_at DESC LIMIT $1");

    storage.postgres.query(&query, &[&(limit as i64)]).await
}
```

---

## 5. Storage Layer Architecture

### 5.1 Component Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Storage Layer API                         │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │              StorageLayer (Facade)                         │ │
│  │  - Unified interface for all storage operations            │ │
│  │  - Routing logic for multi-backend strategy                │ │
│  │  - Cross-cutting concerns (logging, metrics, tracing)      │ │
│  └──────────────────┬───────────────────────────────────────┬─┘ │
│                     │                                       │    │
│  ┌──────────────────▼─────────────┐  ┌────────────────────▼──┐ │
│  │   MetricsRepository            │  │  DecisionRepository   │ │
│  │   - Time-series operations     │  │  - CRUD operations    │ │
│  │   - Aggregation queries        │  │  - Versioning         │ │
│  │   - Retention policy           │  │  - Lifecycle mgmt     │ │
│  └──────────────┬─────────────────┘  └────────┬──────────────┘ │
│                 │                              │                │
│  ┌──────────────▼─────────────┐  ┌────────────▼──────────────┐ │
│  │   ExperimentRepository     │  │  ConfigRepository         │ │
│  │   - A/B test management    │  │  - Snapshot management    │ │
│  │   - Statistical queries    │  │  - Version control        │ │
│  │   - Variant tracking       │  │  - Delta computation      │ │
│  └──────────────┬─────────────┘  └────────┬──────────────────┘ │
│                 │                          │                    │
└─────────────────┼──────────────────────────┼────────────────────┘
                  │                          │
     ┌────────────┴──────────────────────────┴─────────────┐
     │              Backend Abstraction Layer               │
     │  ┌──────────────────────────────────────────────┐   │
     │  │  StateBackend Trait                          │   │
     │  │  - get(key) → Option<value>                  │   │
     │  │  - put(key, value)                           │   │
     │  │  - delete(key)                               │   │
     │  │  - list_keys(prefix) → Vec<key>              │   │
     │  │  - clear()                                   │   │
     │  └─────────┬──────────────────┬─────────────────┘   │
     │            │                  │                      │
     │  ┌─────────▼────────┐  ┌──────▼──────────────────┐  │
     │  │ PostgresBackend  │  │  RedisBackend           │  │
     │  │ - Connection     │  │  - Connection manager   │  │
     │  │   pooling        │  │  - Pipeline ops         │  │
     │  │ - Transactions   │  │  - Pub/sub              │  │
     │  │ - Migrations     │  │  - Distributed locks    │  │
     │  └──────────────────┘  └─────────────────────────┘  │
     │                                                       │
     │  ┌──────────────────────────────────────────────┐   │
     │  │  SledBackend                                 │   │
     │  │  - Embedded database                         │   │
     │  │  - Atomic batches                            │   │
     │  │  - Snapshots                                 │   │
     │  └──────────────────────────────────────────────┘   │
     └───────────────────────────────────────────────────────┘
                           │
     ┌─────────────────────┴────────────────────────┐
     │       Caching and Coordination Layer          │
     │  ┌────────────────────────────────────────┐  │
     │  │  CachedBackend (Redis + PostgreSQL)    │  │
     │  │  - L1 Cache: Redis (hot data)          │  │
     │  │  - L2 Store: PostgreSQL (durable)      │  │
     │  │  - Write-through / Write-back          │  │
     │  │  - Cache invalidation                  │  │
     │  └────────────────────────────────────────┘  │
     │                                               │
     │  ┌────────────────────────────────────────┐  │
     │  │  DistributedLock (Redis)               │  │
     │  │  - Redlock algorithm                   │  │
     │  │  - Lease management                    │  │
     │  │  - Deadlock detection                  │  │
     │  └────────────────────────────────────────┘  │
     └───────────────────────────────────────────────┘
```

### 5.2 API Design

#### 5.2.1 StorageLayer Facade

```rust
/// Unified storage layer interface
pub struct StorageLayer {
    postgres: Arc<PostgresStateBackend>,
    redis: Arc<RedisStateBackend>,
    sled: Option<Arc<SledStateBackend>>,
    cached_backend: Arc<CachedBackend>,
    config: StorageConfig,
}

impl StorageLayer {
    /// Initialize storage layer with all backends
    pub async fn new(config: StorageConfig) -> Result<Self> {
        let postgres = Arc::new(
            PostgresStateBackend::new(config.postgres.clone()).await?
        );

        let redis = Arc::new(
            RedisStateBackend::new(config.redis.clone()).await?
        );

        let sled = if config.enable_sled {
            Some(Arc::new(SledStateBackend::new(config.sled.clone())?))
        } else {
            None
        };

        let cached_backend = Arc::new(
            CachedBackend::new(postgres.clone(), redis.clone())
        );

        Ok(Self {
            postgres,
            redis,
            sled,
            cached_backend,
            config,
        })
    }

    // Repository accessors
    pub fn metrics(&self) -> MetricsRepository { /* ... */ }
    pub fn decisions(&self) -> DecisionRepository { /* ... */ }
    pub fn experiments(&self) -> ExperimentRepository { /* ... */ }
    pub fn config(&self) -> ConfigRepository { /* ... */ }
    pub fn insights(&self) -> InsightRepository { /* ... */ }

    // Health checks
    pub async fn health_check(&self) -> Result<HealthStatus> { /* ... */ }
}
```

#### 5.2.2 Repository Pattern

```rust
/// Metrics repository for time-series data
pub struct MetricsRepository {
    postgres: Arc<PostgresStateBackend>,
    redis: Arc<RedisStateBackend>,
}

impl MetricsRepository {
    /// Store a single metric event
    pub async fn store(&self, event: MetricEvent) -> Result<()> {
        // Write to PostgreSQL for durability
        self.postgres.insert_metric(&event).await?;

        // Cache latest value in Redis
        let cache_key = format!(
            "metrics:latest:{}:{}",
            event.service_id, event.metric_name
        );
        self.redis.set_ex(
            &cache_key,
            &serde_json::to_vec(&event)?,
            300 // 5 minutes TTL
        ).await?;

        Ok(())
    }

    /// Batch store multiple metric events
    pub async fn store_batch(&self, events: Vec<MetricEvent>) -> Result<()> {
        // Bulk insert into PostgreSQL
        self.postgres.insert_metrics_batch(&events).await?;

        // Update Redis cache for latest values
        for event in events {
            let cache_key = format!(
                "metrics:latest:{}:{}",
                event.service_id, event.metric_name
            );
            self.redis.set_ex(
                &cache_key,
                &serde_json::to_vec(&event)?,
                300
            ).await?;
        }

        Ok(())
    }

    /// Query metrics by time range
    pub async fn query_range(
        &self,
        service_id: &str,
        metric_name: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>
    ) -> Result<Vec<MetricEvent>> {
        self.postgres.query_metrics(service_id, metric_name, start, end).await
    }

    /// Get latest metric value (cached)
    pub async fn get_latest(
        &self,
        service_id: &str,
        metric_name: &str
    ) -> Result<Option<MetricEvent>> {
        let cache_key = format!("metrics:latest:{}:{}", service_id, metric_name);

        // Try cache first
        if let Some(cached) = self.redis.get(&cache_key).await? {
            return Ok(Some(serde_json::from_slice(&cached)?));
        }

        // Fallback to database
        self.postgres.query_latest_metric(service_id, metric_name).await
    }
}
```

#### 5.2.3 Transaction Support

```rust
/// Transaction context for multi-step operations
pub struct Transaction<'a> {
    postgres_tx: sqlx::Transaction<'a, sqlx::Postgres>,
    redis: Arc<RedisStateBackend>,
    cache_invalidations: Vec<String>,
}

impl<'a> Transaction<'a> {
    /// Create a new transaction
    pub async fn begin(storage: &'a StorageLayer) -> Result<Self> {
        let postgres_tx = storage.postgres.pool.begin().await?;

        Ok(Self {
            postgres_tx,
            redis: storage.redis.clone(),
            cache_invalidations: Vec::new(),
        })
    }

    /// Execute operation within transaction
    pub async fn execute<F, T>(&mut self, f: F) -> Result<T>
    where
        F: FnOnce(&mut sqlx::Transaction<sqlx::Postgres>) -> Result<T>
    {
        f(&mut self.postgres_tx)
    }

    /// Mark cache key for invalidation
    pub fn invalidate_cache(&mut self, key: impl Into<String>) {
        self.cache_invalidations.push(key.into());
    }

    /// Commit transaction and invalidate caches
    pub async fn commit(self) -> Result<()> {
        // Commit PostgreSQL transaction
        self.postgres_tx.commit().await?;

        // Invalidate Redis caches
        for key in self.cache_invalidations {
            self.redis.del(&key).await?;
        }

        Ok(())
    }

    /// Rollback transaction
    pub async fn rollback(self) -> Result<()> {
        self.postgres_tx.rollback().await?;
        Ok(())
    }
}

// Usage example
pub async fn update_decision_with_deployment(
    decision: &Decision,
    deployment: &Deployment,
    storage: &StorageLayer
) -> Result<()> {
    let mut tx = Transaction::begin(storage).await?;

    tx.execute(|postgres_tx| {
        // Update decision
        sqlx::query("UPDATE decisions SET status = $1 WHERE id = $2")
            .bind(&decision.status)
            .bind(&decision.id)
            .execute(postgres_tx)
            .await?;

        // Insert deployment
        sqlx::query("INSERT INTO deployments (...) VALUES (...)")
            .bind(/* ... */)
            .execute(postgres_tx)
            .await?;

        Ok(())
    }).await?;

    // Invalidate caches
    tx.invalidate_cache(format!("cache:decision:{}", decision.id));

    tx.commit().await?;
    Ok(())
}
```

### 5.3 Error Handling

```rust
/// Storage layer error types
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Backend unavailable: {backend}")]
    BackendUnavailable { backend: String },

    #[error("Connection failed: {details}")]
    ConnectionFailed { details: String },

    #[error("Transaction failed: {operation} - {reason}")]
    TransactionFailed { operation: String, reason: String },

    #[error("Serialization failed for key {key}: {reason}")]
    SerializationFailed { key: String, reason: String },

    #[error("Deserialization failed for key {key}: {reason}")]
    DeserializationFailed { key: String, reason: String },

    #[error("Key not found: {key}")]
    KeyNotFound { key: String },

    #[error("Optimistic lock failed: {resource} (version conflict)")]
    OptimisticLockFailed { resource: String },

    #[error("Constraint violation: {constraint}")]
    ConstraintViolation { constraint: String },

    #[error("Timeout: operation {operation} exceeded {timeout_ms}ms")]
    Timeout { operation: String, timeout_ms: u64 },

    #[error("Quota exceeded: {quota_type} limit reached")]
    QuotaExceeded { quota_type: String },
}

pub type Result<T> = std::result::Result<T, StorageError>;
```

### 5.4 Configuration

```rust
/// Storage layer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// PostgreSQL configuration
    pub postgres: PostgresConfig,

    /// Redis configuration
    pub redis: RedisConfig,

    /// Sled configuration (optional)
    pub sled: SledConfig,

    /// Enable Sled backend
    pub enable_sled: bool,

    /// Caching strategy
    pub cache_strategy: CacheStrategy,

    /// Retry policy
    pub retry_policy: RetryPolicy,

    /// Health check interval
    pub health_check_interval: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheStrategy {
    WriteThrough,  // Write to cache and DB simultaneously
    WriteBack,     // Write to cache, async write to DB
    CacheAside,    // Read from cache, populate on miss
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub exponential_backoff: bool,
}
```

---

## 6. Multi-Backend Strategy

### 6.1 Backend Selection Matrix

| Data Type | Primary Backend | Cache Backend | Local Backend | Rationale |
|-----------|----------------|---------------|---------------|-----------|
| **Metrics (raw)** | PostgreSQL | Redis (5min TTL) | Sled (dev) | Durability + fast recent reads |
| **Metrics (aggregated)** | PostgreSQL MV | Redis (1hr TTL) | - | Pre-computed, cached |
| **Decisions** | PostgreSQL | Redis (30min TTL) | - | ACID + fast reads |
| **Experiments** | PostgreSQL | Redis (15min TTL) | - | Complex queries + cache |
| **Deployments** | PostgreSQL | - | - | Audit trail (no cache) |
| **Config Snapshots** | PostgreSQL | Redis (1hr TTL) | Sled (local copy) | Versioning + fast startup |
| **Window State** | Redis | - | Sled (fallback) | Low latency critical |
| **Distributed Locks** | Redis | - | - | Atomic operations required |
| **Pub/Sub Events** | Redis | - | - | Real-time messaging |
| **Insights** | PostgreSQL | Redis (10min TTL) | - | Search + cache |

### 6.2 Data Flow Patterns

#### 6.2.1 Write Path: Metrics Ingestion

```
[Collector]
    ↓
[MetricsRepository::store_batch()]
    ↓
    ├─→ [PostgreSQL] ← Primary write (durable)
    │       ↓
    │   [Partition: metrics_2025_01]
    │
    └─→ [Redis] ← Cache latest values
            ↓
        [Key: metrics:latest:{service}:{metric}]
        [TTL: 5 minutes]
```

#### 6.2.2 Read Path: Query Aggregated Metrics

```
[Dashboard]
    ↓
[MetricsRepository::query_aggregated()]
    ↓
[Check Redis Cache]
    ├─→ [Cache Hit] → Return cached result
    │
    └─→ [Cache Miss]
            ↓
        [Query PostgreSQL Materialized View]
            ↓
        [Populate Redis Cache with 1hr TTL]
            ↓
        [Return result]
```

#### 6.2.3 Write Path: Decision Creation with Transaction

```
[Decision Engine]
    ↓
[DecisionRepository::create()]
    ↓
[Begin PostgreSQL Transaction]
    ↓
    ├─→ [INSERT INTO decisions]
    ├─→ [INSERT INTO deployments] (if applicable)
    └─→ [Commit Transaction]
            ↓
        [Invalidate Redis cache keys]
            ↓
        [Publish Redis event: "decision-created"]
```

#### 6.2.4 Read Path: Get Decision (Multi-Tier Cache)

```
[Analyzer]
    ↓
[DecisionRepository::get(id)]
    ↓
[Check L1: Redis Cache]
    ├─→ [Cache Hit] → Return cached decision
    │
    └─→ [Cache Miss]
            ↓
        [Check L2: PostgreSQL]
            ↓
        [Decision Found]
            ↓
        [Populate Redis with 30min TTL]
            ↓
        [Return decision]
```

### 6.3 Consistency Models

#### 6.3.1 Strong Consistency (PostgreSQL)

For critical data requiring ACID guarantees:
- Decisions
- Experiments
- Deployments
- Configuration Snapshots
- Audit Logs

**Guarantee**: Linearizability within single PostgreSQL instance, eventual consistency across read replicas

#### 6.3.2 Eventual Consistency (Redis + PostgreSQL)

For cached data with acceptable staleness:
- Metrics (recent values)
- Aggregated statistics
- Insight summaries

**Guarantee**: Eventually consistent within TTL window (5-60 minutes)

#### 6.3.3 Session Consistency (Redis)

For ephemeral state:
- Window state
- Distributed locks
- Rate limiting counters

**Guarantee**: Consistent within Redis instance, no cross-instance guarantees

### 6.4 Conflict Resolution

#### 6.4.1 Optimistic Locking (PostgreSQL)

```rust
pub async fn update_decision_with_optimistic_lock(
    id: Uuid,
    update_fn: impl Fn(&mut Decision),
    repo: &DecisionRepository
) -> Result<Decision> {
    loop {
        // Read current version
        let mut decision = repo.get(id).await?;
        let current_version = decision.version;

        // Apply updates
        update_fn(&mut decision);
        decision.version += 1;

        // Attempt update with version check
        let result = repo.postgres.execute(
            "UPDATE decisions
             SET changes = $1, version = $2, updated_at = NOW()
             WHERE id = $3 AND version = $4",
            &[&decision.changes, &decision.version, &id, &current_version]
        ).await?;

        if result.rows_affected() == 1 {
            // Success
            repo.invalidate_cache(id).await?;
            return Ok(decision);
        } else {
            // Version conflict - retry
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}
```

#### 6.4.2 Cache Invalidation on Write

```rust
pub async fn invalidate_on_write(
    entity_type: &str,
    entity_id: Uuid,
    redis: &RedisStateBackend
) -> Result<()> {
    // Direct cache key
    let cache_key = format!("cache:{}:{}", entity_type, entity_id);
    redis.del(&cache_key).await?;

    // Invalidate related keys (e.g., list caches)
    let list_key = format!("cache:{}:list:*", entity_type);
    let keys = redis.keys(&list_key).await?;
    for key in keys {
        redis.del(&key).await?;
    }

    // Publish invalidation event
    redis.publish(
        "events:cache-invalidation",
        &serde_json::json!({
            "entity_type": entity_type,
            "entity_id": entity_id,
            "timestamp": Utc::now()
        })
    ).await?;

    Ok(())
}
```

### 6.5 Failover and Fallback

#### 6.5.1 Redis Unavailable → PostgreSQL Fallback

```rust
pub async fn get_with_fallback(
    key: &str,
    redis: &RedisStateBackend,
    postgres: &PostgresStateBackend
) -> Result<Option<Vec<u8>>> {
    // Try Redis first
    match redis.get(key).await {
        Ok(value) => Ok(value),
        Err(e) if e.is_connection_error() => {
            warn!("Redis unavailable, falling back to PostgreSQL: {}", e);

            // Query PostgreSQL directly
            postgres.get(key).await
        }
        Err(e) => Err(e),
    }
}
```

#### 6.5.2 PostgreSQL Unavailable → Circuit Breaker

```rust
pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    failure_threshold: u32,
    timeout: Duration,
}

enum CircuitState {
    Closed { failures: u32 },
    Open { opened_at: Instant },
    HalfOpen,
}

impl CircuitBreaker {
    pub async fn call<F, T>(&self, f: F) -> Result<T>
    where
        F: Future<Output = Result<T>>
    {
        let state = self.state.read().await;

        match *state {
            CircuitState::Open { opened_at } => {
                if opened_at.elapsed() > self.timeout {
                    drop(state);
                    let mut state = self.state.write().await;
                    *state = CircuitState::HalfOpen;
                } else {
                    return Err(StorageError::BackendUnavailable {
                        backend: "PostgreSQL (circuit open)".to_string()
                    });
                }
            }
            _ => {}
        }
        drop(state);

        // Execute operation
        match f.await {
            Ok(result) => {
                self.on_success().await;
                Ok(result)
            }
            Err(e) => {
                self.on_failure().await;
                Err(e)
            }
        }
    }

    async fn on_success(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::Closed { failures: 0 };
    }

    async fn on_failure(&self) {
        let mut state = self.state.write().await;
        match *state {
            CircuitState::Closed { failures } => {
                if failures + 1 >= self.failure_threshold {
                    *state = CircuitState::Open {
                        opened_at: Instant::now()
                    };
                } else {
                    *state = CircuitState::Closed { failures: failures + 1 };
                }
            }
            CircuitState::HalfOpen => {
                *state = CircuitState::Open {
                    opened_at: Instant::now()
                };
            }
            _ => {}
        }
    }
}
```

---

## 7. Performance Requirements

### 7.1 Throughput Targets

| Operation | Target (p50) | Target (p95) | Target (p99) | Priority |
|-----------|--------------|--------------|--------------|----------|
| **Metric write (single)** | 1ms | 5ms | 10ms | P0 |
| **Metric write (batch 100)** | 10ms | 50ms | 100ms | P0 |
| **Decision read (cached)** | 0.5ms | 2ms | 5ms | P0 |
| **Decision read (uncached)** | 10ms | 30ms | 50ms | P0 |
| **Decision write** | 20ms | 50ms | 100ms | P0 |
| **Experiment query** | 50ms | 200ms | 500ms | P1 |
| **Config snapshot** | 30ms | 100ms | 200ms | P1 |
| **Aggregated metric query** | 100ms | 500ms | 1000ms | P1 |
| **Insight generation** | 500ms | 2000ms | 5000ms | P2 |

### 7.2 Latency Requirements

#### 7.2.1 Redis Operations

**FR-PERF-001**: Redis GET operations shall complete in <1ms (p95)
**FR-PERF-002**: Redis SET operations shall complete in <2ms (p95)
**FR-PERF-003**: Redis pipeline operations (100 commands) shall complete in <10ms (p95)

#### 7.2.2 PostgreSQL Operations

**FR-PERF-004**: PostgreSQL simple SELECT queries shall complete in <10ms (p95)
**FR-PERF-005**: PostgreSQL INSERT operations shall complete in <20ms (p95)
**FR-PERF-006**: PostgreSQL UPDATE operations shall complete in <30ms (p95)
**FR-PERF-007**: PostgreSQL aggregation queries shall complete in <500ms (p95)

#### 7.2.3 Sled Operations

**FR-PERF-008**: Sled GET operations shall complete in <100µs (p95)
**FR-PERF-009**: Sled PUT operations shall complete in <200µs (p95)

### 7.3 Throughput Requirements

**FR-PERF-010**: Storage layer shall support 10,000+ writes/second sustained
**FR-PERF-011**: Storage layer shall support 50,000+ reads/second sustained
**FR-PERF-012**: PostgreSQL connection pool shall handle 100+ concurrent connections
**FR-PERF-013**: Redis shall support 100,000+ operations/second

### 7.4 Scalability Requirements

**FR-SCALE-001**: PostgreSQL shall scale to 10TB+ database size
**FR-SCALE-002**: Redis cluster shall scale to 32+ nodes
**FR-SCALE-003**: Storage layer shall support horizontal scaling via read replicas
**FR-SCALE-004**: Query performance shall degrade <10% as data volume increases 10x

### 7.5 Resource Limits

| Resource | Limit | Rationale |
|----------|-------|-----------|
| **PostgreSQL Max Connections** | 200 | Prevent connection exhaustion |
| **PostgreSQL Connection Pool (per instance)** | 50 | Balance throughput and overhead |
| **Redis Memory (per instance)** | 16GB | Typical cache size |
| **Redis Connection Pool** | 10 | Multiplexed protocol |
| **Sled Database Size** | 10GB | Embedded storage limit |
| **Transaction Duration (max)** | 30 seconds | Prevent long-running locks |
| **Query Timeout** | 60 seconds | Fail fast on slow queries |

### 7.6 Performance Optimization Strategies

#### 7.6.1 Indexing

```sql
-- Time-range queries (BRIN for large tables)
CREATE INDEX idx_metrics_timestamp ON metrics USING BRIN (timestamp);

-- Equality and range queries (B-tree)
CREATE INDEX idx_decisions_status ON decisions (status);
CREATE INDEX idx_decisions_created_at ON decisions (created_at DESC);

-- Multi-column queries
CREATE INDEX idx_metrics_service_metric ON metrics (service_id, metric_name);

-- JSONB queries (GIN)
CREATE INDEX idx_decisions_metadata ON decisions USING GIN (metadata);
CREATE INDEX idx_metrics_tags ON metrics USING GIN (tags);

-- Partial indexes (filtered)
CREATE INDEX idx_insights_unacknowledged
    ON analyzer_insights (detected_at DESC)
    WHERE NOT acknowledged;
```

#### 7.6.2 Query Optimization

```sql
-- Use prepared statements
PREPARE get_decision (uuid) AS
    SELECT * FROM decisions WHERE id = $1;
EXECUTE get_decision('550e8400-e29b-41d4-a716-446655440000');

-- Avoid SELECT * in production
SELECT id, status, created_at, target_services
FROM decisions
WHERE status = 'pending'
ORDER BY created_at DESC
LIMIT 100;

-- Use EXPLAIN ANALYZE to profile queries
EXPLAIN ANALYZE
SELECT
    service_id,
    metric_name,
    AVG(metric_value) as avg_value
FROM metrics
WHERE timestamp >= NOW() - INTERVAL '1 hour'
GROUP BY service_id, metric_name;
```

#### 7.6.3 Connection Pooling Best Practices

```rust
// PostgreSQL connection pool configuration
let pool = PgPoolOptions::new()
    .min_connections(5)           // Keep minimum idle connections
    .max_connections(50)          // Limit max connections
    .acquire_timeout(Duration::from_secs(10)) // Fail fast
    .idle_timeout(Duration::from_secs(600))   // 10 min idle timeout
    .max_lifetime(Duration::from_secs(3600))  // Recycle after 1 hour
    .connect(&database_url)
    .await?;
```

#### 7.6.4 Batch Operations

```rust
// Batch insert for metrics (10x faster than individual inserts)
pub async fn insert_metrics_batch(
    events: &[MetricEvent],
    pool: &PgPool
) -> Result<()> {
    let mut query_builder = sqlx::QueryBuilder::new(
        "INSERT INTO metrics (timestamp, service_id, metric_name, metric_value, unit, tags)"
    );

    query_builder.push_values(events, |mut b, event| {
        b.push_bind(event.timestamp)
         .push_bind(&event.service_id)
         .push_bind(&event.metric_name)
         .push_bind(event.value)
         .push_bind(&event.unit)
         .push_bind(&event.tags);
    });

    query_builder.build().execute(pool).await?;

    Ok(())
}
```

---

## 8. Consistency and Durability

### 8.1 ACID Guarantees (PostgreSQL)

#### 8.1.1 Atomicity

**FR-ACID-001**: All multi-step operations shall execute atomically within transactions
**Priority**: P0

```rust
// Example: Atomic decision + deployment creation
pub async fn create_decision_with_deployment(
    decision: Decision,
    deployment: Deployment,
    pool: &PgPool
) -> Result<(Decision, Deployment)> {
    let mut tx = pool.begin().await?;

    // Insert decision
    let decision_id = sqlx::query_scalar(
        "INSERT INTO decisions (...) VALUES (...) RETURNING id"
    )
    .fetch_one(&mut *tx)
    .await?;

    // Insert deployment
    let deployment_id = sqlx::query_scalar(
        "INSERT INTO deployments (...) VALUES (...) RETURNING id"
    )
    .fetch_one(&mut *tx)
    .await?;

    // Commit transaction (atomic)
    tx.commit().await?;

    Ok((decision, deployment))
}
```

#### 8.1.2 Consistency

**FR-ACID-002**: All foreign key constraints shall be enforced
**FR-ACID-003**: CHECK constraints shall validate data integrity
**FR-ACID-004**: Triggers shall maintain derived data consistency

```sql
-- Foreign key constraints
ALTER TABLE deployments
    ADD CONSTRAINT fk_deployments_decision
    FOREIGN KEY (decision_id) REFERENCES decisions(id)
    ON DELETE RESTRICT;

-- Check constraints
ALTER TABLE decisions
    ADD CONSTRAINT check_expected_impact_confidence
    CHECK ((expected_impact->>'confidence')::float BETWEEN 0 AND 1);

-- Triggers for audit log
CREATE TRIGGER audit_decision_updates
    AFTER UPDATE ON decisions
    FOR EACH ROW
    EXECUTE FUNCTION log_decision_change();
```

#### 8.1.3 Isolation

**FR-ACID-005**: PostgreSQL shall use READ COMMITTED isolation level by default
**FR-ACID-006**: SERIALIZABLE isolation shall be used for conflict-sensitive operations

```rust
// Serializable transaction for critical operations
pub async fn execute_with_serializable_isolation<F, T>(
    pool: &PgPool,
    f: F
) -> Result<T>
where
    F: FnOnce(&mut sqlx::Transaction<sqlx::Postgres>) -> Result<T>
{
    let mut tx = pool.begin().await?;

    // Set isolation level to SERIALIZABLE
    sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
        .execute(&mut *tx)
        .await?;

    let result = f(&mut tx)?;

    tx.commit().await?;

    Ok(result)
}
```

#### 8.1.4 Durability

**FR-ACID-007**: PostgreSQL shall use synchronous_commit = on for critical tables
**FR-ACID-008**: WAL (Write-Ahead Logging) shall be enabled with fsync = on
**FR-ACID-009**: Transaction logs shall be replicated to standby servers

```ini
# postgresql.conf
wal_level = replica
fsync = on
synchronous_commit = on
full_page_writes = on
wal_log_hints = on
max_wal_senders = 10
wal_keep_size = 1GB
```

### 8.2 Replication Strategy

#### 8.2.1 PostgreSQL Streaming Replication

**FR-REP-001**: PostgreSQL shall use streaming replication with 1+ standby servers
**FR-REP-002**: Replication lag shall be <1 second (p99)
**FR-REP-003**: Standby servers shall be read-only replicas for query load distribution

```ini
# Primary server configuration
wal_level = replica
max_wal_senders = 10
wal_keep_size = 1GB
hot_standby = on

# Standby server recovery.conf
primary_conninfo = 'host=primary-db port=5432 user=replicator password=...'
primary_slot_name = 'standby_1'
hot_standby = on
```

#### 8.2.2 Redis Replication

**FR-REP-004**: Redis shall use master-replica replication with 1+ replicas
**FR-REP-005**: Redis Sentinel shall monitor cluster health and perform automatic failover
**FR-REP-006**: Redis replication shall be asynchronous (eventual consistency)

```ini
# redis.conf (replica)
replicaof redis-master-host 6379
replica-read-only yes
replica-priority 100

# Sentinel configuration
sentinel monitor mymaster redis-master-host 6379 2
sentinel down-after-milliseconds mymaster 5000
sentinel failover-timeout mymaster 60000
sentinel parallel-syncs mymaster 1
```

### 8.3 Data Durability Guarantees

| Data Type | Durability Target | Strategy | RPO | RTO |
|-----------|------------------|----------|-----|-----|
| **Decisions** | 99.999% | PostgreSQL + replication + backups | <1 min | <30 min |
| **Metrics (critical)** | 99.99% | PostgreSQL + replication | <5 min | <30 min |
| **Metrics (non-critical)** | 99.9% | PostgreSQL + daily backups | <24 hours | <2 hours |
| **Experiments** | 99.999% | PostgreSQL + replication + backups | <1 min | <30 min |
| **Config Snapshots** | 99.999% | PostgreSQL + replication + S3 backups | <1 min | <1 hour |
| **Window State (Redis)** | 99% | Redis RDB + AOF | <1 second | <5 min |

**RPO (Recovery Point Objective)**: Maximum acceptable data loss
**RTO (Recovery Time Objective)**: Maximum acceptable downtime

---

## 9. Backup and Recovery

### 9.1 Backup Strategy

#### 9.1.1 PostgreSQL Continuous Archiving

**FR-BACKUP-001**: PostgreSQL shall use continuous WAL archiving to S3/compatible storage
**FR-BACKUP-002**: Full database backups shall be taken daily at 02:00 UTC
**FR-BACKUP-003**: Incremental backups (WAL segments) shall be archived every 5 minutes
**FR-BACKUP-004**: Backups shall be retained for 30 days

```bash
#!/bin/bash
# PostgreSQL backup script using pgBackRest

# Full backup (daily)
pgbackrest --stanza=optimizer --type=full backup

# Incremental backup (continuous via archive_command)
archive_command = 'pgbackrest --stanza=optimizer archive-push %p'
```

#### 9.1.2 Redis Persistence

**FR-BACKUP-005**: Redis shall use RDB snapshots every hour
**FR-BACKUP-006**: Redis shall use AOF with everysec fsync policy
**FR-BACKUP-007**: Redis RDB files shall be backed up to S3 daily

```bash
#!/bin/bash
# Redis backup script

# Trigger RDB snapshot
redis-cli BGSAVE

# Wait for completion
while [ $(redis-cli LASTSAVE) -eq $LASTSAVE ]; do
    sleep 1
done

# Copy RDB to backup location
aws s3 cp /var/lib/redis/dump.rdb s3://backups/redis/dump-$(date +%Y%m%d).rdb

# Copy AOF
aws s3 cp /var/lib/redis/appendonly.aof s3://backups/redis/appendonly-$(date +%Y%m%d).aof
```

#### 9.1.3 Configuration Snapshots

**FR-BACKUP-008**: Configuration snapshots shall be exported to JSON and stored in S3
**FR-BACKUP-009**: Snapshot exports shall include full relationship graph
**FR-BACKUP-010**: Exports shall be versioned with SHA-256 checksums

```rust
pub async fn export_configuration_snapshot(
    snapshot_id: Uuid,
    storage: &StorageLayer,
    s3_client: &S3Client
) -> Result<()> {
    // Query snapshot from database
    let snapshot = storage.config().get_snapshot(snapshot_id).await?;

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&snapshot)?;

    // Compute checksum
    let checksum = sha256::digest(json.as_bytes());

    // Upload to S3
    let key = format!(
        "config-snapshots/{}/{}-{}.json",
        snapshot.service_id,
        snapshot.snapshot_timestamp.format("%Y%m%d"),
        snapshot_id
    );

    s3_client.put_object()
        .bucket("optimizer-backups")
        .key(key)
        .body(json.into_bytes().into())
        .metadata("checksum", checksum)
        .send()
        .await?;

    Ok(())
}
```

### 9.2 Recovery Procedures

#### 9.2.1 Point-in-Time Recovery (PostgreSQL)

**FR-RECOVERY-001**: PostgreSQL shall support PITR to any point within retention period
**FR-RECOVERY-002**: PITR shall be tested monthly
**FR-RECOVERY-003**: Recovery time shall be <30 minutes for last 24 hours

```bash
#!/bin/bash
# PostgreSQL point-in-time recovery using pgBackRest

# Restore to specific timestamp
pgbackrest --stanza=optimizer \
    --type=time \
    --target="2025-01-10 14:30:00" \
    --target-action=promote \
    restore

# Start PostgreSQL
systemctl start postgresql

# Verify recovery
psql -U postgres -d optimizer -c "SELECT NOW();"
```

#### 9.2.2 Redis Recovery

**FR-RECOVERY-004**: Redis shall restore from RDB snapshot within 5 minutes
**FR-RECOVERY-005**: Redis shall replay AOF for data after last snapshot

```bash
#!/bin/bash
# Redis recovery procedure

# Stop Redis
systemctl stop redis

# Restore RDB from backup
aws s3 cp s3://backups/redis/dump-20250110.rdb /var/lib/redis/dump.rdb

# Restore AOF from backup (optional, for additional durability)
aws s3 cp s3://backups/redis/appendonly-20250110.aof /var/lib/redis/appendonly.aof

# Set ownership
chown redis:redis /var/lib/redis/dump.rdb /var/lib/redis/appendonly.aof

# Start Redis
systemctl start redis

# Verify data
redis-cli PING
redis-cli DBSIZE
```

#### 9.2.3 Disaster Recovery Plan

**FR-RECOVERY-006**: Complete disaster recovery runbook shall be documented
**FR-RECOVERY-007**: Disaster recovery shall be tested quarterly
**FR-RECOVERY-008**: Recovery from complete data loss shall complete within 4 hours

**DR Checklist**:

1. **Assess Scope**
   - [ ] Identify affected components (PostgreSQL, Redis, Sled)
   - [ ] Determine last known good state
   - [ ] Estimate data loss window

2. **Prepare Recovery Environment**
   - [ ] Provision new database servers if needed
   - [ ] Restore network connectivity
   - [ ] Verify backup accessibility

3. **PostgreSQL Recovery**
   - [ ] Restore from latest full backup
   - [ ] Apply WAL segments up to recovery point
   - [ ] Verify data integrity
   - [ ] Update DNS/connection strings

4. **Redis Recovery**
   - [ ] Restore from latest RDB snapshot
   - [ ] Replay AOF if available
   - [ ] Verify cache warmth
   - [ ] Resume pub/sub subscriptions

5. **Configuration Recovery**
   - [ ] Restore latest configuration snapshots from S3
   - [ ] Validate checksums
   - [ ] Apply to target services

6. **Verification**
   - [ ] Run health checks
   - [ ] Verify read/write operations
   - [ ] Check data consistency
   - [ ] Monitor replication lag

7. **Post-Recovery**
   - [ ] Document incident
   - [ ] Conduct postmortem
   - [ ] Update runbooks
   - [ ] Implement preventive measures

### 9.3 Backup Validation

**FR-BACKUP-011**: All backups shall be validated weekly via test restores
**FR-BACKUP-012**: Backup integrity checks (checksums) shall run daily
**FR-BACKUP-013**: Backup failures shall trigger alerts within 5 minutes

```rust
pub async fn validate_backup(
    backup_path: &str,
    s3_client: &S3Client
) -> Result<BackupValidation> {
    // Download backup metadata
    let metadata = s3_client.head_object()
        .bucket("optimizer-backups")
        .key(backup_path)
        .send()
        .await?;

    let stored_checksum = metadata.metadata()
        .and_then(|m| m.get("checksum"))
        .ok_or(StorageError::ValidationFailed)?;

    // Download backup file
    let backup_data = s3_client.get_object()
        .bucket("optimizer-backups")
        .key(backup_path)
        .send()
        .await?
        .body
        .collect()
        .await?
        .into_bytes();

    // Compute checksum
    let computed_checksum = sha256::digest(&backup_data);

    // Compare
    let valid = stored_checksum == &computed_checksum;

    Ok(BackupValidation {
        backup_path: backup_path.to_string(),
        valid,
        size_bytes: backup_data.len(),
        checksum: computed_checksum,
        validated_at: Utc::now(),
    })
}
```

---

## 10. Testing Requirements

### 10.1 Unit Testing

**FR-TEST-001**: All repository methods shall have unit tests with >90% coverage
**FR-TEST-002**: Mock backends shall be used for unit testing
**FR-TEST-003**: Edge cases and error paths shall be explicitly tested

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;
    use mockall::mock;

    mock! {
        PostgresBackend {}

        #[async_trait]
        impl StateBackend for PostgresBackend {
            async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;
            async fn put(&self, key: &[u8], value: &[u8]) -> Result<()>;
            async fn delete(&self, key: &[u8]) -> Result<()>;
        }
    }

    #[tokio::test]
    async fn test_decision_repository_get_cached() {
        // Arrange
        let mut redis = MockRedisBackend::new();
        redis.expect_get()
            .with(eq(b"cache:decision:123"))
            .times(1)
            .returning(|_| Ok(Some(vec![/* serialized decision */])));

        let postgres = MockPostgresBackend::new();
        // Should not be called (cache hit)

        let repo = DecisionRepository::new(Arc::new(postgres), Arc::new(redis));

        // Act
        let decision = repo.get(Uuid::parse_str("...").unwrap()).await;

        // Assert
        assert!(decision.is_ok());
    }

    #[tokio::test]
    async fn test_decision_repository_optimistic_lock_conflict() {
        // Test version conflict handling
        // ...
    }

    #[tokio::test]
    async fn test_metrics_repository_batch_insert_partial_failure() {
        // Test error handling in batch operations
        // ...
    }
}
```

### 10.2 Integration Testing

**FR-TEST-004**: Integration tests shall use real databases (PostgreSQL, Redis, Sled)
**FR-TEST-005**: Integration tests shall use Docker Compose for test environment
**FR-TEST-006**: Integration tests shall verify end-to-end data flows

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use testcontainers::*;

    async fn setup_test_storage() -> (StorageLayer, TestCleanup) {
        // Start PostgreSQL container
        let postgres_container = clients::Cli::default()
            .run(images::postgres::Postgres::default());
        let postgres_port = postgres_container.get_host_port_ipv4(5432);

        // Start Redis container
        let redis_container = clients::Cli::default()
            .run(images::redis::Redis::default());
        let redis_port = redis_container.get_host_port_ipv4(6379);

        // Configure storage layer
        let config = StorageConfig {
            postgres: PostgresConfig::new(&format!(
                "postgresql://postgres:postgres@localhost:{})/test",
                postgres_port
            )),
            redis: RedisConfig::builder()
                .url(&format!("redis://localhost:{}", redis_port))
                .build()
                .unwrap(),
            // ...
        };

        let storage = StorageLayer::new(config).await.unwrap();

        // Run migrations
        storage.postgres.run_migrations().await.unwrap();

        let cleanup = TestCleanup {
            _postgres: postgres_container,
            _redis: redis_container,
        };

        (storage, cleanup)
    }

    #[tokio::test]
    async fn test_end_to_end_decision_lifecycle() {
        let (storage, _cleanup) = setup_test_storage().await;

        // Create decision
        let decision = Decision::new(/* ... */);
        storage.decisions().create(decision.clone()).await.unwrap();

        // Verify in PostgreSQL
        let retrieved = storage.decisions().get(decision.id).await.unwrap();
        assert_eq!(retrieved.id, decision.id);

        // Verify cached in Redis
        let cache_key = format!("cache:decision:{}", decision.id);
        let cached = storage.redis.get(cache_key.as_bytes()).await.unwrap();
        assert!(cached.is_some());

        // Update decision
        storage.decisions().update_status(
            decision.id,
            DecisionStatus::Deployed
        ).await.unwrap();

        // Verify cache invalidated
        let cached_after = storage.redis.get(cache_key.as_bytes()).await.unwrap();
        assert!(cached_after.is_none());
    }

    #[tokio::test]
    async fn test_transaction_rollback_on_error() {
        let (storage, _cleanup) = setup_test_storage().await;

        let result = storage.decisions().create_with_deployment(
            Decision::new(/* ... */),
            Deployment { /* invalid deployment */ }
        ).await;

        // Should fail and rollback
        assert!(result.is_err());

        // Verify no partial data committed
        let count = storage.postgres.query_scalar::<i64>(
            "SELECT COUNT(*) FROM decisions"
        ).await.unwrap();
        assert_eq!(count, 0);
    }
}
```

### 10.3 Performance Testing

**FR-TEST-007**: Load tests shall verify throughput targets (10k writes/sec)
**FR-TEST-008**: Latency tests shall verify p95/p99 targets
**FR-TEST-009**: Soak tests shall run for 24 hours to detect memory leaks

```rust
#[cfg(test)]
mod performance_tests {
    use super::*;
    use criterion::{black_box, criterion_group, criterion_main, Criterion};

    fn benchmark_metric_insert(c: &mut Criterion) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let storage = rt.block_on(setup_test_storage()).0;

        c.bench_function("metric_insert_single", |b| {
            b.to_async(&rt).iter(|| async {
                let event = MetricEvent::new(/* ... */);
                storage.metrics().store(black_box(event)).await.unwrap();
            });
        });
    }

    fn benchmark_metric_insert_batch(c: &mut Criterion) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let storage = rt.block_on(setup_test_storage()).0;

        c.bench_function("metric_insert_batch_100", |b| {
            b.to_async(&rt).iter(|| async {
                let events: Vec<_> = (0..100)
                    .map(|_| MetricEvent::new(/* ... */))
                    .collect();
                storage.metrics().store_batch(black_box(events)).await.unwrap();
            });
        });
    }

    criterion_group!(benches, benchmark_metric_insert, benchmark_metric_insert_batch);
    criterion_main!(benches);
}

// Load test with Locust or k6
// k6 script example
import http from 'k6/http';
import { check, sleep } from 'k6';

export let options = {
    stages: [
        { duration: '2m', target: 100 },  // Ramp up to 100 users
        { duration: '5m', target: 100 },  // Stay at 100 users
        { duration: '2m', target: 200 },  // Ramp up to 200 users
        { duration: '5m', target: 200 },  // Stay at 200 users
        { duration: '2m', target: 0 },    // Ramp down to 0 users
    ],
    thresholds: {
        http_req_duration: ['p(95)<100'], // 95% of requests must complete below 100ms
        http_req_failed: ['rate<0.01'],   // Error rate must be below 1%
    },
};

export default function () {
    // Test metric insertion
    let payload = JSON.stringify({
        timestamp: new Date().toISOString(),
        service_id: 'test-service',
        metric_name: 'latency',
        value: Math.random() * 1000,
        unit: 'ms',
        tags: { environment: 'test' }
    });

    let res = http.post('http://localhost:8080/api/metrics', payload, {
        headers: { 'Content-Type': 'application/json' },
    });

    check(res, {
        'status is 200': (r) => r.status === 200,
        'duration < 50ms': (r) => r.timings.duration < 50,
    });

    sleep(0.1); // 100ms think time
}
```

### 10.4 Chaos Testing

**FR-TEST-010**: Chaos tests shall verify resilience to backend failures
**FR-TEST-011**: Chaos tests shall simulate network partitions
**FR-TEST-012**: Chaos tests shall validate automatic recovery mechanisms

```rust
#[cfg(test)]
mod chaos_tests {
    use super::*;

    #[tokio::test]
    async fn test_redis_failure_fallback_to_postgres() {
        let (storage, _cleanup) = setup_test_storage().await;

        // Create decision
        let decision = Decision::new(/* ... */);
        storage.decisions().create(decision.clone()).await.unwrap();

        // Stop Redis
        _cleanup.stop_redis();

        // Should still be able to read from PostgreSQL
        let retrieved = storage.decisions().get(decision.id).await;
        assert!(retrieved.is_ok());
    }

    #[tokio::test]
    async fn test_postgres_connection_pool_exhaustion() {
        let (storage, _cleanup) = setup_test_storage().await;

        // Spawn 200 concurrent operations (exceeds pool size)
        let mut handles = vec![];
        for _ in 0..200 {
            let storage = storage.clone();
            let handle = tokio::spawn(async move {
                storage.decisions().list_recent(None, None, 10).await
            });
            handles.push(handle);
        }

        // All should complete successfully (may queue)
        let results = futures::future::join_all(handles).await;
        let errors = results.iter().filter(|r| r.is_err()).count();

        // Allow some failures, but not all
        assert!(errors < 10); // <5% failure rate
    }

    #[tokio::test]
    async fn test_network_partition_recovery() {
        // Simulate network partition using toxiproxy or similar
        // Verify system recovers when partition heals
        // ...
    }
}
```

---

## 11. Acceptance Criteria

### 11.1 Functional Acceptance Criteria

| ID | Criteria | Verification Method | Status |
|----|----------|---------------------|--------|
| **AC-F-001** | Storage layer supports PostgreSQL, Redis, and Sled backends | Integration tests | ⬜ |
| **AC-F-002** | All CRUD operations work correctly for all entity types | Unit + integration tests | ⬜ |
| **AC-F-003** | Transactions provide ACID guarantees | Integration tests | ⬜ |
| **AC-F-004** | Cache invalidation works correctly on writes | Integration tests | ⬜ |
| **AC-F-005** | Distributed locks prevent concurrent modifications | Integration tests | ⬜ |
| **AC-F-006** | Configuration snapshots can be created and restored | Integration tests | ⬜ |
| **AC-F-007** | Time-series queries return correct aggregations | Integration tests | ⬜ |
| **AC-F-008** | Pub/sub events are delivered reliably | Integration tests | ⬜ |

### 11.2 Performance Acceptance Criteria

| ID | Criteria | Target | Verification Method | Status |
|----|----------|--------|---------------------|--------|
| **AC-P-001** | Metric write throughput | 10,000+ writes/sec | Load test | ⬜ |
| **AC-P-002** | Metric read latency (cached) | <5ms (p95) | Benchmark | ⬜ |
| **AC-P-003** | Decision read latency (cached) | <2ms (p95) | Benchmark | ⬜ |
| **AC-P-004** | Decision write latency | <50ms (p95) | Benchmark | ⬜ |
| **AC-P-005** | Query aggregated metrics | <500ms (p95) | Benchmark | ⬜ |
| **AC-P-006** | Cache hit rate | >80% | Monitoring | ⬜ |
| **AC-P-007** | Database size support | 10TB+ | Scale test | ⬜ |

### 11.3 Reliability Acceptance Criteria

| ID | Criteria | Target | Verification Method | Status |
|----|----------|--------|---------------------|--------|
| **AC-R-001** | Data durability (PostgreSQL) | 99.999% | SLA monitoring | ⬜ |
| **AC-R-002** | System availability | 99.9% | Uptime monitoring | ⬜ |
| **AC-R-003** | Replication lag | <1 second (p99) | Monitoring | ⬜ |
| **AC-R-004** | Automatic failover time | <30 seconds | Chaos test | ⬜ |
| **AC-R-005** | Recovery time objective (RTO) | <30 minutes | DR drill | ⬜ |
| **AC-R-006** | Recovery point objective (RPO) | <1 minute | DR drill | ⬜ |
| **AC-R-007** | Backup success rate | 100% | Monitoring | ⬜ |
| **AC-R-008** | Circuit breaker activation | <100ms | Integration test | ⬜ |

### 11.4 Operational Acceptance Criteria

| ID | Criteria | Verification Method | Status |
|----|----------|---------------------|--------|
| **AC-O-001** | Comprehensive documentation exists | Documentation review | ⬜ |
| **AC-O-002** | Health check endpoints work | Manual test | ⬜ |
| **AC-O-003** | Metrics exported to Prometheus | Manual test | ⬜ |
| **AC-O-004** | Alerts configured for all critical failures | Alert review | ⬜ |
| **AC-O-005** | Runbooks exist for common operations | Documentation review | ⬜ |
| **AC-O-006** | DR procedures documented and tested | DR drill | ⬜ |
| **AC-O-007** | Schema migrations tested and automated | CI/CD verification | ⬜ |

### 11.5 Sign-Off Criteria

**Storage Layer is considered production-ready when**:

✅ All functional acceptance criteria pass
✅ Performance benchmarks meet or exceed targets
✅ Reliability targets demonstrated over 7-day soak test
✅ Security review completed with no critical findings
✅ Documentation complete and reviewed
✅ Disaster recovery successfully tested
✅ Monitoring and alerting operational
✅ Load tests pass at 2x expected traffic

**Sign-Off Approvals Required**:
- [ ] Tech Lead - Storage Layer
- [ ] Principal Engineer - LLM Auto-Optimizer
- [ ] SRE Lead
- [ ] Security Engineer
- [ ] Product Manager

---

## 12. Appendix

### 12.1 Glossary

| Term | Definition |
|------|------------|
| **ACID** | Atomicity, Consistency, Isolation, Durability - properties of database transactions |
| **AOF** | Append-Only File - Redis persistence mechanism |
| **BRIN** | Block Range Index - PostgreSQL index type for large sequential data |
| **Canary Deployment** | Gradual rollout strategy with small percentage of traffic |
| **Circuit Breaker** | Design pattern to prevent cascading failures |
| **GIN** | Generalized Inverted Index - PostgreSQL index type for JSONB and arrays |
| **MVCC** | Multi-Version Concurrency Control - PostgreSQL concurrency mechanism |
| **PITR** | Point-In-Time Recovery |
| **RDB** | Redis Database - snapshot file format |
| **Redlock** | Distributed locking algorithm for Redis |
| **RPO** | Recovery Point Objective - maximum acceptable data loss |
| **RTO** | Recovery Time Objective - maximum acceptable downtime |
| **WAL** | Write-Ahead Log - PostgreSQL transaction log |

### 12.2 References

**PostgreSQL Documentation**:
- [PostgreSQL 15 Official Documentation](https://www.postgresql.org/docs/15/)
- [PostgreSQL Performance Tuning](https://wiki.postgresql.org/wiki/Performance_Optimization)
- [PostgreSQL Replication](https://www.postgresql.org/docs/15/high-availability.html)

**Redis Documentation**:
- [Redis Official Documentation](https://redis.io/documentation)
- [Redis Persistence](https://redis.io/docs/management/persistence/)
- [Redis Sentinel](https://redis.io/docs/management/sentinel/)
- [Redlock Algorithm](https://redis.io/docs/reference/patterns/distributed-locks/)

**Sled Documentation**:
- [Sled Embedded Database](https://docs.rs/sled/)

**Rust Libraries**:
- [sqlx - Async PostgreSQL](https://github.com/launchbadge/sqlx)
- [redis-rs - Redis Client](https://github.com/redis-rs/redis-rs)
- [sled - Embedded KV Store](https://github.com/spacejam/sled)

**Architecture References**:
- [Designing Data-Intensive Applications](https://dataintensive.net/) by Martin Kleppmann
- [Database Reliability Engineering](https://www.oreilly.com/library/view/database-reliability-engineering/9781491925935/)

### 12.3 Change Log

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2025-01-10 | Storage Team | Initial requirements document |

### 12.4 Open Questions

1. **Metric Retention**: Should we implement automatic downsampling for metrics older than 30 days? (e.g., hourly → daily averages)
   - **Impact**: Storage cost vs. query granularity trade-off
   - **Owner**: Storage Team
   - **Due**: Before implementation

2. **Multi-Tenancy**: Do we need to support multiple tenants in a single PostgreSQL instance?
   - **Impact**: Schema design, row-level security
   - **Owner**: Architecture Team
   - **Due**: Phase 2 planning

3. **Global Distribution**: Will we need multi-region deployments with cross-region replication?
   - **Impact**: Latency, consistency model, costs
   - **Owner**: Infrastructure Team
   - **Due**: Before production deployment

4. **Data Privacy**: Are there GDPR/CCPA requirements for metrics and decision data?
   - **Impact**: Data retention, encryption, access controls
   - **Owner**: Legal/Compliance Team
   - **Due**: Before production deployment

---

## Summary

This requirements document defines a comprehensive Storage Layer for the LLM Auto-Optimizer system, providing:

✅ **Multi-Backend Support**: PostgreSQL (durable), Redis (cache/coordination), Sled (embedded)
✅ **Complete Data Models**: Metrics, decisions, experiments, configurations, insights
✅ **Performance Targets**: 10k+ writes/sec, <10ms read latency (p95)
✅ **ACID Guarantees**: Full transactional support with optimistic locking
✅ **High Availability**: 99.9% uptime with automatic failover
✅ **Disaster Recovery**: PITR, <30min RTO, <1min RPO
✅ **Production-Ready**: Monitoring, alerting, runbooks, DR procedures

**Next Steps**:
1. Review and approve requirements document
2. Begin detailed design phase
3. Implement PostgreSQL schema and migrations
4. Implement repository layer with caching
5. Integration testing with Stream Processor
6. Performance benchmarking
7. Production deployment

---

**Document Status**: ✅ Ready for Review
**Target Approval Date**: 2025-01-15
**Implementation Start**: 2025-01-20
