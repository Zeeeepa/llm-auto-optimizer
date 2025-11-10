# Event Deduplication Guide

Complete guide to implementing, deploying, and operating the event deduplication system in the LLM Auto-Optimizer stream processor.

## Table of Contents

1. [Overview](#overview)
2. [Architecture Deep Dive](#architecture-deep-dive)
3. [Implementation Details](#implementation-details)
4. [Redis Schema and Operations](#redis-schema-and-operations)
5. [Performance Tuning](#performance-tuning)
6. [Production Deployment](#production-deployment)
7. [Monitoring and Alerting](#monitoring-and-alerting)
8. [Migration Guide](#migration-guide)
9. [Troubleshooting](#troubleshooting)
10. [Best Practices](#best-practices)

## Overview

### What is Event Deduplication?

Event deduplication ensures that each unique event is processed exactly once, even when:
- Message brokers deliver duplicates (at-least-once semantics)
- Applications retry failed requests
- Multiple consumers process the same stream
- System failures cause event replay

### Why Deduplication Matters

**Data Accuracy**
- Aggregations and metrics reflect true event counts
- No double-counting in analytics and reporting
- Accurate billing and financial calculations

**Cost Efficiency**
- Avoid processing the same LLM request multiple times
- Reduce unnecessary API calls and token usage
- Save compute resources and storage

**System Correctness**
- Maintain exactly-once processing semantics
- Prevent duplicate actions (emails, notifications, transactions)
- Ensure idempotency in distributed systems

### Key Concepts

**Event ID**: Unique identifier for each event
- Primary: Explicit event/message ID (UUID, sequence number)
- Secondary: Transaction ID, idempotency key
- Derived: Content hash, composite key

**TTL (Time-To-Live)**: How long to track an event
- Too short → Miss duplicates outside window
- Too long → Excessive memory usage
- Typical: 24-48 hours for most use cases

**Deduplication Window**: Time range for duplicate detection
- Processing Time: Clock time when event arrives
- Event Time: Timestamp embedded in event
- Best Practice: Use event time + buffer for late arrivals

## Architecture Deep Dive

### System Architecture

```
┌────────────────────────────────────────────────────────────┐
│                     Event Sources                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  │
│  │  Kafka   │  │ Webhook  │  │   API    │  │  Queue   │  │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘  │
└───────┼─────────────┼─────────────┼─────────────┼─────────┘
        │             │             │             │
        └─────────────┴─────────────┴─────────────┘
                        │
                        ▼
        ┌───────────────────────────────────────┐
        │      Event ID Extraction              │
        │  • UUID from event metadata           │
        │  • Transaction ID from payload        │
        │  • Content hash (MD5/SHA256)          │
        │  • Composite key (user+action+time)   │
        └───────────────┬───────────────────────┘
                        │
                        ▼
        ┌───────────────────────────────────────┐
        │    Deduplication Check                │
        │                                       │
        │  ┌─────────────────────────────┐     │
        │  │  Tier 1: Bloom Filter       │     │
        │  │  • Probabilistic check      │     │
        │  │  • < 1μs latency            │     │
        │  │  • 95%+ filter rate         │     │
        │  └──────────┬──────────────────┘     │
        │             │ (might be duplicate)   │
        │             ▼                         │
        │  ┌─────────────────────────────┐     │
        │  │  Tier 2: State Backend      │     │
        │  │  • Exact check (Redis/Sled) │     │
        │  │  • 1-5ms latency            │     │
        │  │  • 100% accuracy            │     │
        │  └──────────┬──────────────────┘     │
        └─────────────┼───────────────────────┘
                      │
           ┌──────────┴──────────┐
           │                     │
           ▼                     ▼
      Duplicate             First Time
       (Drop)               (Process)
                                │
                                ▼
                   ┌────────────────────────┐
                   │  Store Event ID        │
                   │  with TTL in Redis     │
                   └────────────────────────┘
                                │
                                ▼
                   ┌────────────────────────┐
                   │  Process Event         │
                   │  • Aggregate           │
                   │  • Store               │
                   │  • Forward             │
                   └────────────────────────┘
```

### Component Details

#### 1. Event ID Extractor

Responsible for deriving a unique identifier from each event.

**Built-in Extractors**:
```rust
// UUID extractor
let extractor = |event: &CloudEvent| event.id.clone();

// Transaction ID extractor
let extractor = |event: &PaymentEvent| event.transaction_id.clone();

// Composite key extractor
let extractor = |event: &UserAction| {
    format!("{}-{}-{}",
        event.user_id,
        event.action_type,
        event.timestamp.date()
    )
};

// Content hash extractor
let extractor = |event: &GenericEvent| {
    let content = serde_json::to_string(event).unwrap();
    format!("{:x}", sha256::digest(content.as_bytes()))
};
```

**Custom Extractor Interface**:
```rust
pub trait EventIdExtractor<T>: Send + Sync {
    fn extract_id(&self, event: &T) -> String;
}
```

#### 2. Bloom Filter (Optional, High-Performance)

Probabilistic data structure for fast negative lookups.

**How It Works**:
```
Event ID → Hash Functions → Bit Array
          (h1, h2, ..., hk)

Check:
- All bits set? → MAYBE duplicate (check backend)
- Any bit unset? → DEFINITELY new (skip backend)

Insert:
- Set bits at positions h1(id), h2(id), ..., hk(id)
```

**Configuration**:
```rust
struct BloomFilterConfig {
    // Expected number of events to track
    capacity: usize,        // 10M, 100M, etc.

    // Acceptable false positive rate
    fpr: f64,               // 0.001 = 0.1%

    // Number of hash functions (optimal = ln(2) * m/n)
    hash_count: usize,      // Typically 5-10

    // Size in bits (computed from capacity and FPR)
    size_bits: usize,
}
```

**Memory Calculation**:
```
m = -(n * ln(p)) / (ln(2)^2)

Where:
- m = number of bits
- n = capacity (expected events)
- p = false positive rate

Example:
n = 10M events
p = 0.001 (0.1% FPR)
m = -(10M * ln(0.001)) / (ln(2)^2) ≈ 143.8M bits ≈ 18 MB
```

#### 3. State Backend

Persistent storage for authoritative duplicate detection.

**Redis Backend** (Distributed):
```
Pros:
- Shared across multiple instances
- Sub-millisecond reads
- Built-in TTL support
- Horizontal scalability
- HA with Redis Cluster

Cons:
- Network latency (1-5ms)
- External dependency
- Requires Redis infrastructure
```

**Sled Backend** (Local):
```
Pros:
- Ultra-fast reads (< 100μs)
- No network overhead
- Embedded (no external deps)
- ACID guarantees

Cons:
- Per-instance only (no sharing)
- Disk I/O for persistence
- Single-threaded writes
```

**Memory Backend** (Testing):
```
Pros:
- Fastest (< 10μs)
- Zero external dependencies
- Perfect for unit tests

Cons:
- Lost on restart
- No persistence
- Limited to single instance
```

## Implementation Details

### Basic Implementation

```rust
use processor::state::{StateBackend, RedisStateBackend, RedisConfig};
use std::sync::Arc;
use std::time::Duration;

pub struct EventDeduplicator<B: StateBackend> {
    backend: Arc<B>,
    key_prefix: String,
    default_ttl: Duration,
}

impl<B: StateBackend> EventDeduplicator<B> {
    pub fn new(backend: Arc<B>, key_prefix: String, default_ttl: Duration) -> Self {
        Self {
            backend,
            key_prefix,
            default_ttl,
        }
    }

    pub async fn is_duplicate(&self, event_id: &str) -> Result<bool, Box<dyn std::error::Error>> {
        let key = format!("{}{}", self.key_prefix, event_id);

        // Check if event ID exists
        if self.backend.get(key.as_bytes()).await?.is_some() {
            return Ok(true);
        }

        // Mark as seen with TTL
        self.backend
            .put_with_ttl(key.as_bytes(), b"1", self.default_ttl)
            .await?;

        Ok(false)
    }

    pub async fn mark_seen(&self, event_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let key = format!("{}{}", self.key_prefix, event_id);
        self.backend
            .put_with_ttl(key.as_bytes(), b"1", self.default_ttl)
            .await?;
        Ok(())
    }

    pub async fn check_batch(&self, event_ids: &[String]) -> Result<Vec<bool>, Box<dyn std::error::Error>> {
        let mut results = Vec::with_capacity(event_ids.len());

        for event_id in event_ids {
            results.push(self.is_duplicate(event_id).await?);
        }

        Ok(results)
    }
}
```

### Advanced Implementation with Metrics

```rust
use std::sync::atomic::{AtomicU64, Ordering};

pub struct AdvancedDeduplicator<B: StateBackend> {
    backend: Arc<B>,
    key_prefix: String,
    default_ttl: Duration,

    // Metrics
    total_checks: AtomicU64,
    duplicates_found: AtomicU64,
    backend_hits: AtomicU64,
    backend_misses: AtomicU64,
    errors: AtomicU64,
}

impl<B: StateBackend> AdvancedDeduplicator<B> {
    pub async fn is_duplicate_with_metrics(&self, event_id: &str) -> Result<bool, Box<dyn std::error::Error>> {
        self.total_checks.fetch_add(1, Ordering::Relaxed);

        let key = format!("{}{}", self.key_prefix, event_id);

        match self.backend.get(key.as_bytes()).await {
            Ok(Some(_)) => {
                self.backend_hits.fetch_add(1, Ordering::Relaxed);
                self.duplicates_found.fetch_add(1, Ordering::Relaxed);
                Ok(true)
            }
            Ok(None) => {
                self.backend_misses.fetch_add(1, Ordering::Relaxed);

                // Store with TTL
                if let Err(e) = self.backend
                    .put_with_ttl(key.as_bytes(), b"1", self.default_ttl)
                    .await
                {
                    self.errors.fetch_add(1, Ordering::Relaxed);
                    return Err(e.into());
                }

                Ok(false)
            }
            Err(e) => {
                self.errors.fetch_add(1, Ordering::Relaxed);
                Err(e.into())
            }
        }
    }

    pub fn metrics(&self) -> DedupMetrics {
        DedupMetrics {
            total_checks: self.total_checks.load(Ordering::Relaxed),
            duplicates_found: self.duplicates_found.load(Ordering::Relaxed),
            backend_hits: self.backend_hits.load(Ordering::Relaxed),
            backend_misses: self.backend_misses.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DedupMetrics {
    pub total_checks: u64,
    pub duplicates_found: u64,
    pub backend_hits: u64,
    pub backend_misses: u64,
    pub errors: u64,
}

impl DedupMetrics {
    pub fn duplicate_rate(&self) -> f64 {
        if self.total_checks == 0 {
            0.0
        } else {
            self.duplicates_found as f64 / self.total_checks as f64
        }
    }

    pub fn hit_rate(&self) -> f64 {
        let total = self.backend_hits + self.backend_misses;
        if total == 0 {
            0.0
        } else {
            self.backend_hits as f64 / total as f64
        }
    }

    pub fn error_rate(&self) -> f64 {
        if self.total_checks == 0 {
            0.0
        } else {
            self.errors as f64 / self.total_checks as f64
        }
    }
}
```

## Redis Schema and Operations

### Key Schema

**Format**: `{prefix}:{event_id}`

Examples:
```
dedup:evt_550e8400-e29b-41d4-a716-446655440000
dedup:tx_abc123def456
dedup:user_123_purchase_2025-11-10
dedup:hash_a1b2c3d4e5f6
```

**Key Components**:
- **Prefix**: Namespace for deduplication (default: `dedup:`)
- **Event ID**: Unique identifier extracted from event

### Redis Operations

#### 1. Check and Set (Primary Operation)

**Pseudocode**:
```
IF EXISTS(key):
    RETURN true  // Duplicate
ELSE:
    SET key 1 EX ttl_seconds
    RETURN false  // First time
```

**Actual Implementation** (using Lua script for atomicity):
```lua
-- dedup_check_and_set.lua
local key = KEYS[1]
local ttl = ARGV[1]

if redis.call('EXISTS', key) == 1 then
    return 1  -- Duplicate
else
    redis.call('SETEX', key, ttl, '1')
    return 0  -- New event
end
```

**Rust Usage**:
```rust
use redis::Script;

let script = Script::new(r#"
    local key = KEYS[1]
    local ttl = ARGV[1]
    if redis.call('EXISTS', key) == 1 then
        return 1
    else
        redis.call('SETEX', key, ttl, '1')
        return 0
    end
"#);

let is_duplicate: bool = script
    .key(&key)
    .arg(ttl_seconds)
    .invoke_async(&mut connection)
    .await?;
```

#### 2. Batch Check (Pipeline)

**Redis Pipeline**:
```
PIPELINE
  EXISTS dedup:evt_001
  EXISTS dedup:evt_002
  EXISTS dedup:evt_003
  ...
EXEC
```

**Rust Implementation**:
```rust
use redis::Pipeline;

let mut pipe = redis::Pipeline::new();

for event_id in event_ids {
    let key = format!("dedup:{}", event_id);
    pipe.exists(&key);
}

let results: Vec<bool> = pipe.query_async(&mut connection).await?;
```

#### 3. TTL Management

**Automatic Expiration**:
```
SET dedup:evt_123 1 EX 86400  // 24 hours

After 86400 seconds, key is automatically deleted
```

**Check Remaining TTL**:
```
TTL dedup:evt_123
→ Returns seconds remaining (or -1 if no expiry, -2 if key doesn't exist)
```

**Update TTL**:
```
EXPIRE dedup:evt_123 3600  // Reset to 1 hour
```

#### 4. Cleanup Operations

**List All Deduplication Keys**:
```
SCAN 0 MATCH dedup:* COUNT 1000
```

**Bulk Delete**:
```lua
-- cleanup.lua
local cursor = '0'
local count = 0

repeat
    local result = redis.call('SCAN', cursor, 'MATCH', 'dedup:*', 'COUNT', 1000)
    cursor = result[1]
    local keys = result[2]

    if #keys > 0 then
        redis.call('DEL', unpack(keys))
        count = count + #keys
    end
until cursor == '0'

return count
```

### Redis Configuration for Deduplication

**Memory Optimization**:
```
# redis.conf

# Eviction policy - delete keys when memory limit reached
maxmemory-policy allkeys-lru

# Maximum memory
maxmemory 2gb

# Enable keyspace notifications for expired keys (optional, for monitoring)
notify-keyspace-events Ex
```

**Performance Tuning**:
```
# Number of databases (use separate DB for deduplication)
databases 16

# TCP backlog
tcp-backlog 511

# Disable slow operations in production
# (deduplication only needs GET/SET/EXISTS)
rename-command FLUSHDB ""
rename-command FLUSHALL ""
rename-command KEYS ""
```

## Performance Tuning

### Latency Optimization

#### 1. Connection Pooling

**Problem**: Creating new connections for each check (100ms+ overhead)

**Solution**: Maintain connection pool
```rust
let redis_config = RedisConfig::builder()
    .url("redis://localhost:6379")
    .min_connections(10)   // Always maintain 10 connections
    .max_connections(100)  // Scale up to 100 under load
    .connect_timeout(Duration::from_secs(5))
    .build()?;
```

**Impact**: Reduces latency from 100ms → 1-5ms

#### 2. Pipeline Batching

**Problem**: Individual checks require round-trip per event

**Solution**: Batch checks in pipeline
```rust
// Before: N round trips
for event_id in event_ids {
    let is_dup = dedup.is_duplicate(&event_id).await?;
}

// After: 1 round trip
let results = dedup.check_batch(&event_ids).await?;
```

**Impact**: 100 events: 100 round trips → 1 round trip (100x faster)

#### 3. Local Caching

**Problem**: Redis check for every event (network latency)

**Solution**: Two-tier caching
```rust
pub struct CachedDeduplicator {
    local_cache: Arc<Mutex<LruCache<String, Instant>>>,
    redis_backend: Arc<RedisStateBackend>,
    cache_ttl: Duration,
}

impl CachedDeduplicator {
    async fn is_duplicate(&self, event_id: &str) -> Result<bool> {
        // Check local cache first
        {
            let cache = self.local_cache.lock().await;
            if let Some(&seen_at) = cache.get(event_id) {
                if seen_at.elapsed() < self.cache_ttl {
                    return Ok(true);  // Cached duplicate
                }
            }
        }

        // Check Redis
        let is_dup = self.redis_backend.get(key.as_bytes()).await?.is_some();

        if is_dup {
            // Update local cache
            let mut cache = self.local_cache.lock().await;
            cache.put(event_id.to_string(), Instant::now());
        }

        Ok(is_dup)
    }
}
```

**Impact**: Reduces Redis calls by 60-80% for repeated duplicates

### Throughput Optimization

#### 1. Parallel Processing

**Problem**: Sequential checks limit throughput

**Solution**: Process in parallel
```rust
use futures::stream::{self, StreamExt};

let results: Vec<bool> = stream::iter(event_ids)
    .map(|event_id| async move {
        dedup.is_duplicate(&event_id).await
    })
    .buffer_unordered(100)  // Process 100 concurrently
    .collect()
    .await;
```

**Impact**: 10K events/sec → 100K+ events/sec

#### 2. Bloom Filter Pre-filtering

**Problem**: Every check requires Redis round-trip

**Solution**: Bloom filter for negative lookups
```rust
// Check bloom filter first (< 1μs)
if !bloom_filter.might_contain(&event_id) {
    // Definitely new - skip Redis check
    bloom_filter.insert(&event_id);
    return Ok(false);
}

// Bloom filter says "maybe" - check Redis for confirmation
let is_dup = redis.exists(&key).await?;
```

**Impact**: 95%+ reduction in Redis calls for unique events

#### 3. Batching Strategy

**Optimal batch size** depends on latency and throughput trade-offs:

```rust
pub struct BatchConfig {
    // Maximum batch size
    max_batch_size: usize,      // 100-1000

    // Maximum wait time before flushing batch
    max_batch_delay: Duration,  // 10-100ms

    // Flush batch when this full
    flush_threshold: f64,       // 0.8 = 80%
}
```

**Example**:
```
Batch size: 1000
Latency: 5ms per batch
Throughput: 1000 / 0.005 = 200,000 events/sec

Batch size: 100
Latency: 2ms per batch
Throughput: 100 / 0.002 = 50,000 events/sec
```

### Memory Optimization

#### 1. TTL Strategy

**Problem**: Unbounded memory growth

**Solution**: Appropriate TTL based on use case
```rust
fn determine_ttl(event_type: &EventType) -> Duration {
    match event_type {
        EventType::PageView => Duration::from_secs(3600),        // 1 hour
        EventType::Purchase => Duration::from_secs(604800),      // 7 days
        EventType::UserSignup => Duration::from_secs(2592000),   // 30 days
        EventType::Debug => Duration::from_secs(300),            // 5 minutes
    }
}
```

#### 2. Key Compression

**Problem**: Long keys consume memory

**Solution**: Hash long IDs
```rust
fn compress_key(event_id: &str) -> String {
    if event_id.len() > 32 {
        // Hash long IDs to fixed size
        format!("dedup:{:x}", md5::compute(event_id.as_bytes()))
    } else {
        format!("dedup:{}", event_id)
    }
}
```

**Impact**: 64-byte keys → 24-byte keys (62% reduction)

#### 3. Value Minimization

**Problem**: Storing event payloads wastes memory

**Solution**: Store only marker (1 byte)
```rust
// Instead of storing full event
backend.put(key, serde_json::to_vec(&event)?);  // 100s-1000s of bytes

// Store minimal marker
backend.put(key, b"1");  // 1 byte
```

## Production Deployment

### Deployment Checklist

#### Pre-Deployment

- [ ] **Redis Infrastructure**
  - [ ] Redis Cluster or Sentinel for HA
  - [ ] Memory sizing: events/sec × TTL × 100 bytes
  - [ ] Backup strategy (RDB + AOF)
  - [ ] Monitoring and alerting set up

- [ ] **Configuration**
  - [ ] TTL values reviewed and validated
  - [ ] Key prefix configured for namespace isolation
  - [ ] Connection pool size tuned for load
  - [ ] Batch sizes optimized
  - [ ] Bloom filter parameters calculated

- [ ] **Testing**
  - [ ] Load testing completed (target throughput)
  - [ ] Failure scenarios tested (Redis down, network issues)
  - [ ] Duplicate detection accuracy validated
  - [ ] Memory growth verified (should stabilize)
  - [ ] Latency percentiles measured (p50, p95, p99)

#### Deployment

- [ ] **Gradual Rollout**
  - [ ] Deploy to canary environment (5% traffic)
  - [ ] Monitor metrics for 24 hours
  - [ ] Increase to 25% traffic
  - [ ] Full rollout after validation

- [ ] **Monitoring Setup**
  - [ ] Deduplication metrics (duplicate rate, latency)
  - [ ] Redis metrics (memory, connections, latency)
  - [ ] Application metrics (throughput, errors)
  - [ ] Alerts configured for anomalies

#### Post-Deployment

- [ ] **Verification**
  - [ ] Duplicate detection working (check logs)
  - [ ] No increase in errors or latency
  - [ ] Memory usage stable
  - [ ] Redis performance acceptable

- [ ] **Documentation**
  - [ ] Runbook updated with deduplication specifics
  - [ ] On-call playbook includes deduplication issues
  - [ ] Configuration documented

### Redis Infrastructure Setup

#### Option 1: Standalone Redis (Development/Testing)

```bash
# Docker deployment
docker run -d \
  --name redis-dedup \
  -p 6379:6379 \
  -v redis-data:/data \
  redis:7-alpine \
  redis-server --appendonly yes --maxmemory 2gb --maxmemory-policy allkeys-lru
```

#### Option 2: Redis Sentinel (High Availability)

```yaml
# docker-compose.yml
version: '3.8'
services:
  redis-master:
    image: redis:7-alpine
    command: redis-server --maxmemory 2gb --maxmemory-policy allkeys-lru
    volumes:
      - redis-master-data:/data

  redis-replica-1:
    image: redis:7-alpine
    command: redis-server --replicaof redis-master 6379 --maxmemory 2gb
    depends_on:
      - redis-master

  redis-replica-2:
    image: redis:7-alpine
    command: redis-server --replicaof redis-master 6379 --maxmemory 2gb
    depends_on:
      - redis-master

  sentinel-1:
    image: redis:7-alpine
    command: redis-sentinel /etc/sentinel.conf
    volumes:
      - ./sentinel.conf:/etc/sentinel.conf
    depends_on:
      - redis-master

  sentinel-2:
    image: redis:7-alpine
    command: redis-sentinel /etc/sentinel.conf
    volumes:
      - ./sentinel.conf:/etc/sentinel.conf
    depends_on:
      - redis-master

  sentinel-3:
    image: redis:7-alpine
    command: redis-sentinel /etc/sentinel.conf
    volumes:
      - ./sentinel.conf:/etc/sentinel.conf
    depends_on:
      - redis-master

volumes:
  redis-master-data:
```

**sentinel.conf**:
```
sentinel monitor mymaster redis-master 6379 2
sentinel down-after-milliseconds mymaster 5000
sentinel parallel-syncs mymaster 1
sentinel failover-timeout mymaster 10000
```

#### Option 3: Redis Cluster (Horizontal Scaling)

```bash
# Create cluster with 6 nodes (3 masters, 3 replicas)
docker-compose -f redis-cluster.yml up -d

# Initialize cluster
docker exec -it redis-1 redis-cli --cluster create \
  redis-1:6379 redis-2:6379 redis-3:6379 \
  redis-4:6379 redis-5:6379 redis-6:6379 \
  --cluster-replicas 1
```

### Kubernetes Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: stream-processor-dedup
spec:
  replicas: 3
  selector:
    matchLabels:
      app: stream-processor
  template:
    metadata:
      labels:
        app: stream-processor
    spec:
      containers:
      - name: processor
        image: llm-auto-optimizer/processor:latest
        env:
        - name: REDIS_URL
          value: "redis://redis-cluster:6379"
        - name: DEDUP_KEY_PREFIX
          value: "dedup:prod:"
        - name: DEDUP_TTL_SECONDS
          value: "86400"
        - name: DEDUP_BATCH_SIZE
          value: "1000"
        resources:
          requests:
            memory: "512Mi"
            cpu: "500m"
          limits:
            memory: "2Gi"
            cpu: "2000m"
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 5

---
apiVersion: v1
kind: Service
metadata:
  name: stream-processor
spec:
  selector:
    app: stream-processor
  ports:
  - port: 8080
    targetPort: 8080
  type: ClusterIP
```

## Monitoring and Alerting

### Key Metrics

#### 1. Deduplication Metrics

```rust
// Export Prometheus metrics
use prometheus::{Counter, Histogram, Gauge};

lazy_static! {
    static ref DEDUP_CHECKS_TOTAL: Counter = Counter::new(
        "dedup_checks_total",
        "Total number of deduplication checks"
    ).unwrap();

    static ref DEDUP_DUPLICATES_TOTAL: Counter = Counter::new(
        "dedup_duplicates_total",
        "Total number of duplicates detected"
    ).unwrap();

    static ref DEDUP_LATENCY: Histogram = Histogram::with_opts(
        histogram_opts!(
            "dedup_check_duration_seconds",
            "Deduplication check latency"
        )
    ).unwrap();

    static ref DEDUP_DUPLICATE_RATE: Gauge = Gauge::new(
        "dedup_duplicate_rate",
        "Current duplicate rate (duplicates / total checks)"
    ).unwrap();
}

// Update metrics
DEDUP_CHECKS_TOTAL.inc();
if is_duplicate {
    DEDUP_DUPLICATES_TOTAL.inc();
}
DEDUP_LATENCY.observe(duration.as_secs_f64());
DEDUP_DUPLICATE_RATE.set(duplicate_rate);
```

#### 2. Redis Metrics

Monitor via Redis exporter or direct commands:

```bash
# Memory usage
redis-cli INFO memory

# Key count
redis-cli DBSIZE

# Operations per second
redis-cli INFO stats | grep instantaneous_ops_per_sec

# Latency
redis-cli --latency

# Slow log
redis-cli SLOWLOG GET 10
```

#### 3. Application Metrics

```rust
// Track business-level metrics
lazy_static! {
    static ref EVENTS_PROCESSED: Counter = Counter::new(
        "events_processed_total",
        "Total unique events processed"
    ).unwrap();

    static ref EVENTS_DROPPED: Counter = Counter::new(
        "events_dropped_total",
        "Total events dropped as duplicates"
    ).unwrap();
}
```

### Grafana Dashboard

```json
{
  "dashboard": {
    "title": "Event Deduplication",
    "panels": [
      {
        "title": "Duplicate Rate",
        "targets": [
          {
            "expr": "rate(dedup_duplicates_total[5m]) / rate(dedup_checks_total[5m])"
          }
        ]
      },
      {
        "title": "Deduplication Latency (p95)",
        "targets": [
          {
            "expr": "histogram_quantile(0.95, dedup_check_duration_seconds_bucket)"
          }
        ]
      },
      {
        "title": "Redis Memory Usage",
        "targets": [
          {
            "expr": "redis_memory_used_bytes"
          }
        ]
      },
      {
        "title": "Events Processed vs Dropped",
        "targets": [
          {
            "expr": "rate(events_processed_total[5m])",
            "legendFormat": "Processed"
          },
          {
            "expr": "rate(events_dropped_total[5m])",
            "legendFormat": "Dropped"
          }
        ]
      }
    ]
  }
}
```

### Alerting Rules

```yaml
# prometheus-alerts.yml
groups:
- name: deduplication
  rules:
  # High duplicate rate (> 20%)
  - alert: HighDuplicateRate
    expr: rate(dedup_duplicates_total[5m]) / rate(dedup_checks_total[5m]) > 0.2
    for: 10m
    labels:
      severity: warning
    annotations:
      summary: "High duplicate rate detected"
      description: "Duplicate rate is {{ $value | humanizePercentage }} (threshold: 20%)"

  # Deduplication latency high
  - alert: DeduplicationLatencyHigh
    expr: histogram_quantile(0.95, dedup_check_duration_seconds_bucket) > 0.01
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "Deduplication latency is high"
      description: "P95 latency is {{ $value }}s (threshold: 10ms)"

  # Redis memory usage high
  - alert: RedisMemoryHigh
    expr: redis_memory_used_bytes / redis_memory_max_bytes > 0.9
    for: 5m
    labels:
      severity: critical
    annotations:
      summary: "Redis memory usage is critical"
      description: "Redis is using {{ $value | humanizePercentage }} of max memory"

  # Deduplication errors
  - alert: DeduplicationErrors
    expr: rate(dedup_errors_total[5m]) > 0.01
    for: 5m
    labels:
      severity: critical
    annotations:
      summary: "Deduplication errors detected"
      description: "Error rate: {{ $value }} errors/sec"
```

## Migration Guide

### Migrating from No Deduplication

#### Phase 1: Measurement (Week 1)

1. **Deploy in shadow mode** (check but don't drop duplicates)
```rust
if dedup.is_duplicate(&event_id).await? {
    metrics::counter!("would_have_dropped").increment(1);
    // But still process the event
}
process_event(&event).await?;
```

2. **Analyze duplicate rate**
   - Expected: 1-5% for well-behaved producers
   - High (> 10%): Investigate event sources

3. **Validate event ID extraction**
   - Ensure uniqueness
   - Check for collisions
   - Verify stability across restarts

#### Phase 2: Partial Rollout (Week 2-3)

1. **Enable for non-critical event types**
```rust
if event.event_type.is_critical() {
    // Skip deduplication for now
} else {
    if dedup.is_duplicate(&event_id).await? {
        return Ok(()); // Drop duplicate
    }
}
```

2. **Monitor metrics**
   - Duplicate rate should match shadow mode
   - No increase in errors
   - Latency acceptable

3. **Verify correctness**
   - Aggregations match expected values
   - No missing events
   - No false positives

#### Phase 3: Full Rollout (Week 4)

1. **Enable for all events**
2. **Monitor for 48 hours**
3. **Document any issues**
4. **Create runbook**

### Migrating Between Backends

#### From Memory to Redis

**Reason**: Enable distributed deduplication

**Steps**:
1. **Deploy Redis infrastructure**
2. **Update configuration**
```rust
// Old: memory backend
let backend = MemoryStateBackend::new(Some(ttl));

// New: Redis backend
let redis_config = RedisConfig::builder()
    .url(redis_url)
    .key_prefix("dedup:")
    .default_ttl(ttl)
    .build()?;
let backend = RedisStateBackend::new(redis_config).await?;
```
3. **Rolling restart** (no downtime, just lose in-memory state)
4. **Monitor Redis performance**

#### From Sled to Redis

**Reason**: Share deduplication state across instances

**Steps**:
1. **Export Sled state to Redis** (optional, if continuity critical)
```rust
let sled_backend = SledStateBackend::new("./dedup.db")?;
let redis_backend = RedisStateBackend::new(redis_config).await?;

// Migrate keys
let keys = sled_backend.list_keys(b"dedup:").await?;
for key in keys {
    if let Some(value) = sled_backend.get(&key).await? {
        redis_backend.put(&key, &value).await?;
    }
}
```
2. **Switch configuration**
3. **Verify migration** (check key count)
4. **Clean up Sled files**

## Troubleshooting

### Common Issues

#### Issue 1: High Memory Usage in Redis

**Symptoms**:
- Redis memory growing unbounded
- OOM errors
- Eviction warnings

**Diagnosis**:
```bash
# Check memory stats
redis-cli INFO memory

# Check key count
redis-cli DBSIZE

# Sample keys
redis-cli --scan --pattern "dedup:*" | head -20

# Check TTLs
for key in $(redis-cli --scan --pattern "dedup:*" | head -10); do
    echo "$key: $(redis-cli TTL $key) seconds"
done
```

**Solutions**:
1. **Reduce TTL**
```rust
// Was: 7 days
let ttl = Duration::from_secs(604800);

// Now: 24 hours
let ttl = Duration::from_secs(86400);
```

2. **Enable eviction policy**
```
# redis.conf
maxmemory 2gb
maxmemory-policy allkeys-lru
```

3. **Increase Redis memory** (if budget allows)

4. **Use bloom filter** to reduce Redis storage

#### Issue 2: High Duplicate Rate

**Symptoms**:
- Duplicate rate > 10%
- Unexpected dropped events
- Metrics show inflated duplicate counts

**Diagnosis**:
```rust
// Log duplicate events for analysis
if dedup.is_duplicate(&event_id).await? {
    warn!(
        "Duplicate detected: id={}, source={}, timestamp={}",
        event_id, event.source, event.timestamp
    );
}
```

**Root Causes**:
1. **Producer sending duplicates**
   - Check producer logs
   - Verify producer deduplication
   - Fix at source

2. **Consumer replay**
   - Check Kafka offsets
   - Verify checkpoint/commit logic
   - Ensure consumer group consistency

3. **Event ID collisions**
   - Validate ID uniqueness
   - Use longer IDs (UUID instead of sequential)
   - Add namespace to IDs

#### Issue 3: Deduplication Latency High

**Symptoms**:
- P95 latency > 10ms
- Timeouts
- Degraded throughput

**Diagnosis**:
```rust
// Measure latency distribution
let start = Instant::now();
let is_dup = dedup.is_duplicate(&event_id).await?;
let latency = start.elapsed();

histogram!("dedup_latency_ms").record(latency.as_millis() as f64);
```

**Solutions**:
1. **Check Redis latency**
```bash
redis-cli --latency
redis-cli --latency-history
```

2. **Optimize network**
   - Ensure low-latency network to Redis
   - Use Redis Cluster with local replicas
   - Enable keepalive on connections

3. **Add local caching**
```rust
let cached_dedup = CachedDeduplicator::new(
    redis_backend,
    Duration::from_secs(60),  // Cache for 1 minute
);
```

4. **Use batch operations**
```rust
// Instead of individual checks
let results = dedup.check_batch(&event_ids).await?;
```

#### Issue 4: False Positives (Bloom Filter)

**Symptoms**:
- Events incorrectly marked as duplicates
- Lower throughput than expected
- Bloom filter hit rate > expected FPR

**Diagnosis**:
```rust
let stats = bloom_dedup.stats().await;
let fpr = stats.false_positives as f64 / stats.bloom_hits as f64;
info!("Bloom filter FPR: {:.4}%", fpr * 100.0);
```

**Solutions**:
1. **Increase bloom filter size**
```rust
// Was: 10M capacity
let bloom = BloomFilter::new(10_000_000, 0.01);

// Now: 100M capacity
let bloom = BloomFilter::new(100_000_000, 0.01);
```

2. **Lower false positive rate**
```rust
// Was: 1% FPR
let bloom = BloomFilter::new(capacity, 0.01);

// Now: 0.1% FPR
let bloom = BloomFilter::new(capacity, 0.001);
```

3. **Reset bloom filter periodically**
```rust
// Clear bloom filter every hour
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(3600));
    loop {
        interval.tick().await;
        bloom.clear();
        info!("Bloom filter reset");
    }
});
```

## Best Practices

### 1. Event ID Selection

**DO**:
- Use UUIDs or cryptographic hashes
- Include namespace in composite keys
- Use stable, deterministic IDs
- Document ID format

**DON'T**:
- Use timestamps alone (not unique)
- Use user IDs alone (multiple events per user)
- Use sequential numbers (collision-prone in distributed systems)
- Change ID format without migration

### 2. TTL Configuration

**Guidelines**:
```
TTL = Maximum Expected Delay + Buffer

Examples:
- Real-time events: 1-6 hours
- Batch processing: 24-48 hours
- Financial transactions: 7-30 days
- Compliance/audit: 90 days - 1 year
```

**Consider**:
- Event replay window
- Message broker retention
- Business requirements (billing, compliance)
- Memory constraints

### 3. Monitoring

**Essential Metrics**:
- Duplicate rate (should be stable)
- Deduplication latency (P50, P95, P99)
- Redis memory usage (should plateau)
- Error rate (should be near zero)

**Dashboards**:
- Real-time duplicate rate
- Latency distribution
- Redis health
- Event throughput

**Alerts**:
- Duplicate rate spikes
- Latency degradation
- Redis memory high
- Error rate increase

### 4. Testing

**Unit Tests**:
```rust
#[tokio::test]
async fn test_basic_deduplication() {
    let backend = Arc::new(MemoryStateBackend::new(None));
    let dedup = EventDeduplicator::new(backend, "test:".to_string(), Duration::from_secs(60));

    // First check - should be unique
    assert!(!dedup.is_duplicate("event_1").await.unwrap());

    // Second check - should be duplicate
    assert!(dedup.is_duplicate("event_1").await.unwrap());
}
```

**Integration Tests**:
```rust
#[tokio::test]
async fn test_redis_deduplication() {
    let redis_config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("test:dedup:")
        .build()
        .unwrap();

    let backend = Arc::new(RedisStateBackend::new(redis_config).await.unwrap());
    let dedup = EventDeduplicator::new(backend, "test:".to_string(), Duration::from_secs(60));

    // Test with real Redis
    assert!(!dedup.is_duplicate("event_redis_1").await.unwrap());
    assert!(dedup.is_duplicate("event_redis_1").await.unwrap());
}
```

**Load Tests**:
```rust
#[tokio::test]
async fn test_high_throughput() {
    let dedup = create_deduplicator().await;

    let event_ids: Vec<_> = (0..100_000)
        .map(|i| format!("evt_{}", i))
        .collect();

    let start = Instant::now();
    let results = dedup.check_batch(&event_ids).await.unwrap();
    let duration = start.elapsed();

    let throughput = event_ids.len() as f64 / duration.as_secs_f64();
    println!("Throughput: {:.0} events/sec", throughput);

    assert!(throughput > 10_000.0, "Throughput should be > 10K events/sec");
}
```

### 5. Security

**Redis Security**:
```
# redis.conf

# Require password
requirepass your_strong_password

# Bind to specific interface
bind 127.0.0.1

# Enable TLS
tls-port 6380
tls-cert-file /path/to/cert.pem
tls-key-file /path/to/key.pem
tls-ca-cert-file /path/to/ca.pem

# Disable dangerous commands
rename-command FLUSHDB ""
rename-command FLUSHALL ""
rename-command CONFIG ""
```

**Application Security**:
```rust
let redis_config = RedisConfig::builder()
    .url("rediss://redis:6380")  // TLS enabled
    .password("your_strong_password")
    .tls_enabled(true)
    .build()?;
```

### 6. Disaster Recovery

**Backup Strategy**:
```bash
# RDB snapshots
redis-cli BGSAVE

# AOF persistence
redis-cli BGREWRITEAOF

# Backup to S3
aws s3 cp dump.rdb s3://backup-bucket/redis/$(date +%Y%m%d)/
```

**Recovery**:
```bash
# Restore from backup
aws s3 cp s3://backup-bucket/redis/20251110/dump.rdb /var/lib/redis/

# Restart Redis
systemctl restart redis
```

**Note**: Losing deduplication state is often acceptable (just process duplicates temporarily). Prioritize availability over deduplication.

## Conclusion

Event deduplication is a critical component for ensuring data accuracy and system correctness in distributed streaming applications. This guide has covered:

- Architecture and implementation patterns
- Redis schema and operations
- Performance optimization techniques
- Production deployment strategies
- Comprehensive monitoring and alerting
- Migration paths and troubleshooting

By following these guidelines, you can build a robust, production-ready deduplication system that scales to millions of events per second while maintaining correctness and reliability.

## Additional Resources

- [Redis Documentation](https://redis.io/documentation)
- [Stream Processor Architecture](./crates/processor/STREAM_PROCESSOR.md)
- [State Backend Guide](./crates/processor/src/state/README.md)
- [Examples](./crates/processor/examples/)
  - [event_deduplication_demo.rs](./crates/processor/examples/event_deduplication_demo.rs)
  - [advanced_deduplication.rs](./crates/processor/examples/advanced_deduplication.rs)

## License

Apache-2.0
