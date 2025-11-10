# Storage Layer Implementation Plan

**Version:** 1.0
**Date:** 2025-11-10
**Status:** Implementation Planning
**Target Completion:** 10-12 weeks
**Estimated LOC:** 8,000-10,000

---

## Executive Summary

This document provides a comprehensive implementation plan for building an enterprise-grade storage layer for the LLM Auto-Optimizer system. The storage layer will provide a unified interface across multiple backend implementations (PostgreSQL, Redis, and Sled) with support for high availability, scalability, and production-grade operations.

### Objectives

1. **Multi-Backend Support:** Unified interface supporting PostgreSQL (primary persistent store), Redis (distributed cache/state), and Sled (embedded storage)
2. **Enterprise Features:** Connection pooling, health checks, circuit breakers, automatic retries, and observability
3. **High Availability:** Support for read replicas, failover, and distributed coordination
4. **Performance:** Connection pooling, batch operations, pipelining, and compression
5. **Production Ready:** Comprehensive error handling, monitoring, migrations, and testing

### Current State

- **Storage Crate:** Exists but is a stub (placeholder with basic add function)
- **Dependencies:** Already defined in workspace Cargo.toml (sqlx, redis, sled, deadpool-redis, etc.)
- **Related Work:** Processor crate has state backend implementations that can inform this design

### Scope

- **New Lines of Code:** 8,000-10,000 LOC
- **Total Files:** 60+ files
- **Migrations:** 10+ SQL migration scripts
- **Tests:** 400+ unit and integration tests
- **Timeline:** 10-12 weeks (7 phases)

---

## Table of Contents

1. [Implementation Overview](#1-implementation-overview)
2. [Complete File Structure](#2-complete-file-structure)
3. [Implementation Phases](#3-implementation-phases)
4. [Database Schemas](#4-database-schemas)
5. [Module Interfaces](#5-module-interfaces)
6. [Testing Strategy](#6-testing-strategy)
7. [Lines of Code Estimates](#7-lines-of-code-estimates)
8. [Dependencies](#8-dependencies)
9. [Risk Mitigation](#9-risk-mitigation)
10. [Validation Criteria](#10-validation-criteria)
11. [Deployment Checklist](#11-deployment-checklist)

---

## 1. Implementation Overview

### 1.1 Architecture Diagram

```
┌────────────────────────────────────────────────────────────────────┐
│                    Storage Layer Architecture                       │
├────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────┐         ┌──────────────────┐                │
│  │   Application    │         │   Application    │                │
│  │   Crates         │         │   Services       │                │
│  │ (Processor, etc) │         │   (API, CLI)     │                │
│  └────────┬─────────┘         └────────┬─────────┘                │
│           │                             │                           │
│           └─────────────┬───────────────┘                           │
│                         │                                           │
│  ┌──────────────────────▼────────────────────────────────────────┐│
│  │              StorageManager (Facade)                          ││
│  │  • Multi-backend orchestration                                ││
│  │  • Routing logic (read/write separation)                      ││
│  │  • Health checking and failover                               ││
│  │  • Metrics aggregation                                        ││
│  └──────────────────────┬────────────────────────────────────────┘│
│                         │                                           │
│  ┌──────────────────────▼────────────────────────────────────────┐│
│  │              Storage Trait (Unified Interface)                ││
│  │  • get/put/delete operations                                  ││
│  │  • Batch operations                                           ││
│  │  • Transaction support                                        ││
│  │  • Query capabilities                                         ││
│  │  • Health/metrics methods                                     ││
│  └──────────────────────┬────────────────────────────────────────┘│
│                         │                                           │
│  ┌──────────┬───────────┴───────────┬──────────────┐              │
│  │          │                       │              │              │
│  ▼          ▼                       ▼              ▼              │
│  ┌─────────────┐  ┌─────────────┐  ┌───────────┐  ┌──────────┐  │
│  │ PostgreSQL  │  │    Redis    │  │   Sled    │  │  Hybrid  │  │
│  │  Backend    │  │  Backend    │  │  Backend  │  │ Backend  │  │
│  │             │  │             │  │           │  │          │  │
│  │ • Primary   │  │ • Cache     │  │ • Embed   │  │ • Tiered │  │
│  │ • ACID      │  │ • Fast      │  │ • Local   │  │ • Smart  │  │
│  │ • Durable   │  │ • Distrib   │  │ • NoSQL   │  │ • Route  │  │
│  └──────┬──────┘  └──────┬──────┘  └─────┬─────┘  └────┬─────┘  │
│         │                │                │             │         │
│  ┌──────▼────────────────▼────────────────▼─────────────▼──────┐ │
│  │            Connection Pool & Circuit Breaker Layer          │ │
│  │  • Connection pooling (deadpool, r2d2)                      │ │
│  │  • Health checks and heartbeats                             │ │
│  │  • Circuit breaker pattern                                  │ │
│  │  • Automatic retry with backoff                             │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │                  Observability Layer                        │  │
│  │  • Metrics (prometheus-client)                              │  │
│  │  • Tracing (opentelemetry)                                  │  │
│  │  • Structured logging (tracing)                             │  │
│  │  • Health endpoints                                         │  │
│  └─────────────────────────────────────────────────────────────┘  │
│                                                                     │
└────────────────────────────────────────────────────────────────────┘
```

### 1.2 Core Principles

1. **Unified Interface:** Single trait-based API regardless of backend
2. **Backend Flexibility:** Easy to swap, combine, or extend backends
3. **Production Ready:** Built-in observability, error handling, and resilience
4. **Performance First:** Connection pooling, batching, and caching built-in
5. **Type Safety:** Leverage Rust's type system for correctness
6. **Async Native:** Full tokio async/await throughout

### 1.3 Key Features

#### Storage Operations
- CRUD operations (Create, Read, Update, Delete)
- Batch operations for efficiency
- Range queries and prefix scans
- TTL (Time-To-Live) support
- Atomic operations (compare-and-swap)

#### Transaction Support
- Multi-key transactions
- ACID guarantees (PostgreSQL)
- Optimistic concurrency control
- Rollback capabilities

#### Enterprise Features
- Connection pooling and management
- Health checks and circuit breakers
- Automatic retries with exponential backoff
- Read/write separation
- Multi-region support (future)

#### Observability
- Prometheus metrics (latency, throughput, errors)
- OpenTelemetry traces
- Structured logging
- Health endpoints

---

## 2. Complete File Structure

```
crates/storage/
├── Cargo.toml                              # Dependencies and metadata
├── README.md                               # Crate documentation
├── src/
│   ├── lib.rs                              # Main library entry (150 LOC)
│   │
│   ├── error.rs                            # Error types (200 LOC)
│   ├── types.rs                            # Common types (150 LOC)
│   ├── config.rs                           # Configuration structs (200 LOC)
│   │
│   ├── traits/                             # Core trait definitions
│   │   ├── mod.rs                          # Trait module exports (50 LOC)
│   │   ├── storage.rs                      # Main Storage trait (300 LOC)
│   │   ├── transactional.rs                # Transaction trait (150 LOC)
│   │   ├── queryable.rs                    # Query trait (200 LOC)
│   │   └── observable.rs                   # Observability trait (100 LOC)
│   │
│   ├── postgres/                           # PostgreSQL backend
│   │   ├── mod.rs                          # Module exports (100 LOC)
│   │   ├── backend.rs                      # Main backend impl (500 LOC)
│   │   ├── pool.rs                         # Connection pool (200 LOC)
│   │   ├── transaction.rs                  # Transaction handling (250 LOC)
│   │   ├── query_builder.rs                # Query construction (300 LOC)
│   │   ├── migrations.rs                   # Migration runner (150 LOC)
│   │   ├── health.rs                       # Health checks (150 LOC)
│   │   ├── metrics.rs                      # Metrics collection (200 LOC)
│   │   └── tests.rs                        # Unit tests (400 LOC)
│   │
│   ├── redis/                              # Redis backend
│   │   ├── mod.rs                          # Module exports (100 LOC)
│   │   ├── backend.rs                      # Main backend impl (450 LOC)
│   │   ├── pool.rs                         # Connection pool (180 LOC)
│   │   ├── pipeline.rs                     # Redis pipelining (200 LOC)
│   │   ├── lua_scripts.rs                  # Lua script templates (150 LOC)
│   │   ├── cluster.rs                      # Cluster support (250 LOC)
│   │   ├── pubsub.rs                       # Pub/Sub features (200 LOC)
│   │   ├── health.rs                       # Health checks (130 LOC)
│   │   ├── metrics.rs                      # Metrics collection (180 LOC)
│   │   └── tests.rs                        # Unit tests (350 LOC)
│   │
│   ├── sled/                               # Sled backend
│   │   ├── mod.rs                          # Module exports (80 LOC)
│   │   ├── backend.rs                      # Main backend impl (400 LOC)
│   │   ├── transaction.rs                  # Transaction handling (180 LOC)
│   │   ├── iterator.rs                     # Iterator support (150 LOC)
│   │   ├── compaction.rs                   # Compaction strategy (120 LOC)
│   │   ├── health.rs                       # Health checks (100 LOC)
│   │   ├── metrics.rs                      # Metrics collection (150 LOC)
│   │   └── tests.rs                        # Unit tests (300 LOC)
│   │
│   ├── hybrid/                             # Hybrid/tiered storage
│   │   ├── mod.rs                          # Module exports (100 LOC)
│   │   ├── backend.rs                      # Tiered backend (350 LOC)
│   │   ├── routing.rs                      # Request routing (200 LOC)
│   │   ├── cache_policy.rs                 # Caching policies (180 LOC)
│   │   ├── sync.rs                         # Backend sync (220 LOC)
│   │   └── tests.rs                        # Unit tests (280 LOC)
│   │
│   ├── manager/                            # Storage manager (facade)
│   │   ├── mod.rs                          # Module exports (80 LOC)
│   │   ├── manager.rs                      # Main manager impl (400 LOC)
│   │   ├── registry.rs                     # Backend registry (180 LOC)
│   │   ├── router.rs                       # Request router (200 LOC)
│   │   ├── health.rs                       # Aggregate health (150 LOC)
│   │   └── tests.rs                        # Unit tests (250 LOC)
│   │
│   ├── resilience/                         # Resilience patterns
│   │   ├── mod.rs                          # Module exports (50 LOC)
│   │   ├── circuit_breaker.rs              # Circuit breaker (250 LOC)
│   │   ├── retry.rs                        # Retry logic (200 LOC)
│   │   ├── timeout.rs                      # Timeout handling (120 LOC)
│   │   └── bulkhead.rs                     # Bulkhead pattern (180 LOC)
│   │
│   ├── serialization/                      # Serialization layer
│   │   ├── mod.rs                          # Module exports (80 LOC)
│   │   ├── codec.rs                        # Codec trait (150 LOC)
│   │   ├── json.rs                         # JSON codec (100 LOC)
│   │   ├── bincode.rs                      # Bincode codec (100 LOC)
│   │   ├── msgpack.rs                      # MessagePack codec (100 LOC)
│   │   └── compression.rs                  # Compression support (150 LOC)
│   │
│   ├── migration/                          # Schema migrations
│   │   ├── mod.rs                          # Module exports (100 LOC)
│   │   ├── runner.rs                       # Migration runner (250 LOC)
│   │   ├── embedded.rs                     # Embedded migrations (150 LOC)
│   │   └── version.rs                      # Version tracking (120 LOC)
│   │
│   ├── observability/                      # Observability layer
│   │   ├── mod.rs                          # Module exports (80 LOC)
│   │   ├── metrics.rs                      # Metrics definitions (250 LOC)
│   │   ├── tracing.rs                      # Tracing setup (180 LOC)
│   │   └── health.rs                       # Health check aggregation (150 LOC)
│   │
│   └── utils/                              # Utility functions
│       ├── mod.rs                          # Module exports (50 LOC)
│       ├── connection.rs                   # Connection helpers (150 LOC)
│       ├── key.rs                          # Key utilities (120 LOC)
│       └── testing.rs                      # Test utilities (200 LOC)
│
├── migrations/                             # PostgreSQL migrations
│   ├── 001_create_storage_tables.sql       # Core tables (100 LOC)
│   ├── 002_add_indexes.sql                 # Indexing strategy (80 LOC)
│   ├── 003_add_partitions.sql              # Partitioning (60 LOC)
│   ├── 004_add_constraints.sql             # Constraints (50 LOC)
│   ├── 005_add_triggers.sql                # Triggers (80 LOC)
│   ├── 006_add_functions.sql               # Stored procedures (120 LOC)
│   ├── 007_add_views.sql                   # Materialized views (70 LOC)
│   ├── 008_add_monitoring.sql              # Monitoring tables (60 LOC)
│   ├── 009_add_audit_log.sql               # Audit logging (80 LOC)
│   └── 010_optimize_performance.sql        # Performance tuning (90 LOC)
│
├── benches/                                # Benchmarks
│   ├── postgres_bench.rs                   # PostgreSQL benchmarks (300 LOC)
│   ├── redis_bench.rs                      # Redis benchmarks (300 LOC)
│   ├── sled_bench.rs                       # Sled benchmarks (250 LOC)
│   └── hybrid_bench.rs                     # Hybrid benchmarks (300 LOC)
│
├── tests/                                  # Integration tests
│   ├── common/
│   │   └── mod.rs                          # Test utilities (200 LOC)
│   ├── postgres_integration.rs             # PostgreSQL tests (400 LOC)
│   ├── redis_integration.rs                # Redis tests (350 LOC)
│   ├── sled_integration.rs                 # Sled tests (300 LOC)
│   ├── hybrid_integration.rs               # Hybrid tests (350 LOC)
│   ├── transaction_tests.rs                # Transaction tests (300 LOC)
│   ├── failover_tests.rs                   # Failover tests (280 LOC)
│   └── performance_tests.rs                # Performance tests (300 LOC)
│
└── examples/                               # Example usage
    ├── basic_usage.rs                      # Basic operations (200 LOC)
    ├── transactions.rs                     # Transaction examples (250 LOC)
    ├── batch_operations.rs                 # Batch examples (200 LOC)
    ├── hybrid_storage.rs                   # Hybrid examples (250 LOC)
    ├── monitoring.rs                       # Monitoring setup (200 LOC)
    └── migration_example.rs                # Migration examples (180 LOC)
```

**Total Estimated Files:** 62 files
**Total Estimated LOC:** ~9,500 lines (excluding tests/examples)

---

## 3. Implementation Phases

### Phase 1: Core Framework (Week 1-2)

**Objective:** Establish foundational types, traits, and error handling.

#### Deliverables

1. **Error Handling** (`src/error.rs`)
   - Define StorageError enum with variants
   - Connection errors, serialization errors, timeout errors
   - Backend-specific error mappings
   - Error context and debugging information

2. **Core Types** (`src/types.rs`)
   - Key/Value types with serialization
   - QueryOptions, QueryResult types
   - StorageMetrics types
   - Health status types

3. **Configuration** (`src/config.rs`)
   - PostgresConfig
   - RedisConfig
   - SledConfig
   - HybridConfig with routing rules

4. **Storage Trait** (`src/traits/storage.rs`)
   - Core CRUD operations
   - Batch operations
   - Range queries
   - TTL support
   - Health and metrics methods

5. **Supporting Traits** (`src/traits/`)
   - Transactional trait for transactions
   - Queryable trait for advanced queries
   - Observable trait for metrics/tracing

#### Code Scaffold: Error Types

```rust
// src/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Operation timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Transaction error: {0}")]
    TransactionError(String),

    #[error("Backend error: {backend} - {message}")]
    BackendError { backend: String, message: String },

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Migration error: {0}")]
    MigrationError(String),

    #[error("Circuit breaker open for backend: {0}")]
    CircuitBreakerOpen(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("PostgreSQL error: {0}")]
    PostgresError(#[from] sqlx::Error),

    #[error("Redis error: {0}")]
    RedisError(#[from] redis::RedisError),

    #[error("Sled error: {0}")]
    SledError(#[from] sled::Error),
}

pub type Result<T> = std::result::Result<T, StorageError>;
```

#### Code Scaffold: Storage Trait

```rust
// src/traits/storage.rs
use async_trait::async_trait;
use std::time::Duration;

#[async_trait]
pub trait Storage: Send + Sync {
    /// Get a value by key
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;

    /// Put a key-value pair
    async fn put(&self, key: &str, value: Vec<u8>) -> Result<()>;

    /// Put with TTL (time-to-live)
    async fn put_with_ttl(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Duration,
    ) -> Result<()>;

    /// Delete a key
    async fn delete(&self, key: &str) -> Result<()>;

    /// Check if key exists
    async fn exists(&self, key: &str) -> Result<bool>;

    /// Get multiple values (batch operation)
    async fn get_many(&self, keys: &[&str]) -> Result<Vec<Option<Vec<u8>>>>;

    /// Put multiple key-value pairs (batch operation)
    async fn put_many(&self, items: &[(&str, Vec<u8>)]) -> Result<()>;

    /// Delete multiple keys (batch operation)
    async fn delete_many(&self, keys: &[&str]) -> Result<()>;

    /// Range query - get all keys with prefix
    async fn scan_prefix(&self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>>;

    /// Range query - get keys in range
    async fn scan_range(
        &self,
        start: &str,
        end: &str,
    ) -> Result<Vec<(String, Vec<u8>)>>;

    /// Compare and swap (atomic operation)
    async fn compare_and_swap(
        &self,
        key: &str,
        old_value: Option<Vec<u8>>,
        new_value: Vec<u8>,
    ) -> Result<bool>;

    /// Get health status
    async fn health_check(&self) -> Result<HealthStatus>;

    /// Get metrics
    async fn metrics(&self) -> Result<StorageMetrics>;

    /// Close and cleanup
    async fn close(&self) -> Result<()>;
}
```

#### Files Created (Week 1-2): 10 files, ~1,300 LOC

```
src/
├── lib.rs                    (150 LOC)
├── error.rs                  (200 LOC)
├── types.rs                  (150 LOC)
├── config.rs                 (200 LOC)
└── traits/
    ├── mod.rs                (50 LOC)
    ├── storage.rs            (300 LOC)
    ├── transactional.rs      (150 LOC)
    └── queryable.rs          (200 LOC)
```

---

### Phase 2: PostgreSQL Backend (Week 3-4)

**Objective:** Implement production-grade PostgreSQL backend with connection pooling, transactions, and migrations.

#### Deliverables

1. **PostgreSQL Backend** (`src/postgres/backend.rs`)
   - Implement Storage trait
   - Connection pool management (sqlx Pool)
   - Error handling and mapping
   - Prepared statements
   - Batch operations using COPY or VALUES

2. **Transaction Support** (`src/postgres/transaction.rs`)
   - ACID transaction handling
   - Savepoints
   - Rollback capabilities
   - Isolation level support

3. **Query Builder** (`src/postgres/query_builder.rs`)
   - Dynamic query construction
   - Safe parameter binding
   - Query optimization hints

4. **Health Checks** (`src/postgres/health.rs`)
   - Connection health verification
   - Query performance checks
   - Database statistics

5. **Metrics** (`src/postgres/metrics.rs`)
   - Query latency tracking
   - Connection pool metrics
   - Error rate monitoring

6. **Migrations** (`src/postgres/migrations.rs`)
   - Migration runner using sqlx::migrate
   - Version tracking
   - Rollback support

#### Database Schema

See [Section 4: Database Schemas](#4-database-schemas) for complete SQL schemas.

#### Code Scaffold: PostgreSQL Backend

```rust
// src/postgres/backend.rs
use sqlx::{PgPool, postgres::PgPoolOptions};
use async_trait::async_trait;
use std::time::Duration;

pub struct PostgresBackend {
    pool: PgPool,
    config: PostgresConfig,
    metrics: Arc<PostgresMetrics>,
}

impl PostgresBackend {
    pub async fn new(config: PostgresConfig) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(Duration::from_secs(config.connect_timeout_secs))
            .idle_timeout(Duration::from_secs(config.idle_timeout_secs))
            .max_lifetime(Duration::from_secs(config.max_lifetime_secs))
            .connect(&config.connection_string)
            .await?;

        Ok(Self {
            pool,
            config,
            metrics: Arc::new(PostgresMetrics::default()),
        })
    }

    pub async fn run_migrations(&self) -> Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await?;
        Ok(())
    }
}

#[async_trait]
impl Storage for PostgresBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let start = Instant::now();

        let result = sqlx::query_scalar!(
            r#"
            SELECT value
            FROM storage_kv
            WHERE key = $1
              AND (expires_at IS NULL OR expires_at > NOW())
            "#,
            key
        )
        .fetch_optional(&self.pool)
        .await?;

        self.metrics.record_operation("get", start.elapsed());
        Ok(result)
    }

    async fn put(&self, key: &str, value: Vec<u8>) -> Result<()> {
        let start = Instant::now();

        sqlx::query!(
            r#"
            INSERT INTO storage_kv (key, value, updated_at)
            VALUES ($1, $2, NOW())
            ON CONFLICT (key)
            DO UPDATE SET value = $2, updated_at = NOW()
            "#,
            key,
            value
        )
        .execute(&self.pool)
        .await?;

        self.metrics.record_operation("put", start.elapsed());
        Ok(())
    }

    async fn put_with_ttl(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Duration,
    ) -> Result<()> {
        let start = Instant::now();
        let expires_at = chrono::Utc::now() + chrono::Duration::from_std(ttl)?;

        sqlx::query!(
            r#"
            INSERT INTO storage_kv (key, value, expires_at, updated_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (key)
            DO UPDATE SET value = $2, expires_at = $3, updated_at = NOW()
            "#,
            key,
            value,
            expires_at
        )
        .execute(&self.pool)
        .await?;

        self.metrics.record_operation("put_with_ttl", start.elapsed());
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let start = Instant::now();

        sqlx::query!(
            "DELETE FROM storage_kv WHERE key = $1",
            key
        )
        .execute(&self.pool)
        .await?;

        self.metrics.record_operation("delete", start.elapsed());
        Ok(())
    }

    // ... additional methods
}
```

#### Files Created (Week 3-4): 10 files, ~2,500 LOC

```
src/postgres/
├── mod.rs                (100 LOC)
├── backend.rs            (500 LOC)
├── pool.rs               (200 LOC)
├── transaction.rs        (250 LOC)
├── query_builder.rs      (300 LOC)
├── migrations.rs         (150 LOC)
├── health.rs             (150 LOC)
├── metrics.rs            (200 LOC)
└── tests.rs              (400 LOC)

migrations/
├── 001_create_storage_tables.sql
├── 002_add_indexes.sql
└── ... (10 migration files)
```

---

### Phase 3: Redis Backend (Week 5-6)

**Objective:** Implement high-performance Redis backend with cluster support, pipelining, and Lua scripts.

#### Deliverables

1. **Redis Backend** (`src/redis/backend.rs`)
   - Implement Storage trait
   - Connection pool (deadpool-redis)
   - Pipeline support for batch operations
   - Cluster mode support

2. **Pipelining** (`src/redis/pipeline.rs`)
   - Batch operation optimization
   - Atomic pipeline execution
   - Error handling per command

3. **Lua Scripts** (`src/redis/lua_scripts.rs`)
   - Atomic multi-key operations
   - Conditional updates
   - Custom logic execution

4. **Cluster Support** (`src/redis/cluster.rs`)
   - Hash slot routing
   - Cluster topology discovery
   - Failover handling

5. **Pub/Sub** (`src/redis/pubsub.rs`)
   - Event notification system
   - Cache invalidation
   - Distributed coordination

6. **Health & Metrics** (`src/redis/health.rs`, `src/redis/metrics.rs`)
   - Connection health checks
   - Command latency tracking
   - Memory usage monitoring

#### Code Scaffold: Redis Backend

```rust
// src/redis/backend.rs
use deadpool_redis::{Pool, Config as RedisPoolConfig, Runtime};
use redis::AsyncCommands;
use async_trait::async_trait;

pub struct RedisBackend {
    pool: Pool,
    config: RedisConfig,
    metrics: Arc<RedisMetrics>,
}

impl RedisBackend {
    pub async fn new(config: RedisConfig) -> Result<Self> {
        let pool_config = RedisPoolConfig {
            url: Some(config.url.clone()),
            connection: None,
            pool: Some(deadpool_redis::PoolConfig {
                max_size: config.max_connections,
                timeouts: deadpool_redis::Timeouts {
                    wait: Some(Duration::from_secs(config.connect_timeout_secs)),
                    create: Some(Duration::from_secs(config.connect_timeout_secs)),
                    recycle: Some(Duration::from_secs(60)),
                },
            }),
        };

        let pool = pool_config.create_pool(Some(Runtime::Tokio1))?;

        Ok(Self {
            pool,
            config,
            metrics: Arc::new(RedisMetrics::default()),
        })
    }
}

#[async_trait]
impl Storage for RedisBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let start = Instant::now();
        let mut conn = self.pool.get().await?;

        let result: Option<Vec<u8>> = conn.get(key).await?;

        self.metrics.record_operation("get", start.elapsed());
        Ok(result)
    }

    async fn put(&self, key: &str, value: Vec<u8>) -> Result<()> {
        let start = Instant::now();
        let mut conn = self.pool.get().await?;

        conn.set(key, value).await?;

        self.metrics.record_operation("put", start.elapsed());
        Ok(())
    }

    async fn put_with_ttl(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Duration,
    ) -> Result<()> {
        let start = Instant::now();
        let mut conn = self.pool.get().await?;

        conn.set_ex(key, value, ttl.as_secs()).await?;

        self.metrics.record_operation("put_with_ttl", start.elapsed());
        Ok(())
    }

    async fn get_many(&self, keys: &[&str]) -> Result<Vec<Option<Vec<u8>>>> {
        let start = Instant::now();
        let mut conn = self.pool.get().await?;

        let result: Vec<Option<Vec<u8>>> = conn.get(keys).await?;

        self.metrics.record_operation("get_many", start.elapsed());
        Ok(result)
    }

    async fn put_many(&self, items: &[(&str, Vec<u8>)]) -> Result<()> {
        let start = Instant::now();
        let mut conn = self.pool.get().await?;

        let mut pipe = redis::pipe();
        for (key, value) in items {
            pipe.set(*key, value);
        }

        pipe.query_async(&mut *conn).await?;

        self.metrics.record_operation("put_many", start.elapsed());
        Ok(())
    }

    async fn scan_prefix(&self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>> {
        let start = Instant::now();
        let mut conn = self.pool.get().await?;

        let pattern = format!("{}*", prefix);
        let mut results = Vec::new();

        let keys: Vec<String> = conn.keys(&pattern).await?;
        if !keys.is_empty() {
            let values: Vec<Option<Vec<u8>>> = conn.get(&keys).await?;

            for (key, value) in keys.into_iter().zip(values.into_iter()) {
                if let Some(val) = value {
                    results.push((key, val));
                }
            }
        }

        self.metrics.record_operation("scan_prefix", start.elapsed());
        Ok(results)
    }

    // ... additional methods
}
```

#### Files Created (Week 5-6): 9 files, ~2,200 LOC

```
src/redis/
├── mod.rs              (100 LOC)
├── backend.rs          (450 LOC)
├── pool.rs             (180 LOC)
├── pipeline.rs         (200 LOC)
├── lua_scripts.rs      (150 LOC)
├── cluster.rs          (250 LOC)
├── pubsub.rs           (200 LOC)
├── health.rs           (130 LOC)
├── metrics.rs          (180 LOC)
└── tests.rs            (350 LOC)
```

---

### Phase 4: Sled Backend (Week 7)

**Objective:** Implement embedded Sled backend for local storage use cases.

#### Deliverables

1. **Sled Backend** (`src/sled/backend.rs`)
   - Implement Storage trait
   - Database initialization
   - Tree management
   - Batch operations

2. **Transaction Support** (`src/sled/transaction.rs`)
   - Transactional operations
   - Conflict detection
   - Rollback support

3. **Iterator Support** (`src/sled/iterator.rs`)
   - Efficient range scans
   - Prefix iteration
   - Reverse iteration

4. **Compaction** (`src/sled/compaction.rs`)
   - Background compaction
   - Space reclamation
   - Performance tuning

#### Code Scaffold: Sled Backend

```rust
// src/sled/backend.rs
use sled::{Db, Tree};
use async_trait::async_trait;

pub struct SledBackend {
    db: Db,
    tree: Tree,
    config: SledConfig,
    metrics: Arc<SledMetrics>,
}

impl SledBackend {
    pub async fn new(config: SledConfig) -> Result<Self> {
        let db = sled::Config::new()
            .path(&config.path)
            .cache_capacity(config.cache_capacity_bytes)
            .flush_every_ms(Some(config.flush_interval_ms))
            .mode(sled::Mode::HighThroughput)
            .use_compression(config.compression)
            .open()?;

        let tree = db.open_tree("storage_kv")?;

        Ok(Self {
            db,
            tree,
            config,
            metrics: Arc::new(SledMetrics::default()),
        })
    }
}

#[async_trait]
impl Storage for SledBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let start = Instant::now();

        let result = self.tree.get(key.as_bytes())?
            .map(|ivec| ivec.to_vec());

        self.metrics.record_operation("get", start.elapsed());
        Ok(result)
    }

    async fn put(&self, key: &str, value: Vec<u8>) -> Result<()> {
        let start = Instant::now();

        self.tree.insert(key.as_bytes(), value)?;

        self.metrics.record_operation("put", start.elapsed());
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let start = Instant::now();

        self.tree.remove(key.as_bytes())?;

        self.metrics.record_operation("delete", start.elapsed());
        Ok(())
    }

    async fn scan_prefix(&self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>> {
        let start = Instant::now();

        let mut results = Vec::new();
        for item in self.tree.scan_prefix(prefix.as_bytes()) {
            let (k, v) = item?;
            let key = String::from_utf8(k.to_vec())
                .map_err(|e| StorageError::SerializationError(e.to_string()))?;
            results.push((key, v.to_vec()));
        }

        self.metrics.record_operation("scan_prefix", start.elapsed());
        Ok(results)
    }

    async fn compare_and_swap(
        &self,
        key: &str,
        old_value: Option<Vec<u8>>,
        new_value: Vec<u8>,
    ) -> Result<bool> {
        let start = Instant::now();

        let result = self.tree.compare_and_swap(
            key.as_bytes(),
            old_value.as_deref(),
            Some(new_value),
        )?;

        self.metrics.record_operation("compare_and_swap", start.elapsed());
        Ok(result.is_ok())
    }

    // ... additional methods
}
```

#### Files Created (Week 7): 7 files, ~1,400 LOC

```
src/sled/
├── mod.rs           (80 LOC)
├── backend.rs       (400 LOC)
├── transaction.rs   (180 LOC)
├── iterator.rs      (150 LOC)
├── compaction.rs    (120 LOC)
├── health.rs        (100 LOC)
├── metrics.rs       (150 LOC)
└── tests.rs         (300 LOC)
```

---

### Phase 5: Storage Manager & Hybrid Backend (Week 8-9)

**Objective:** Implement storage manager for multi-backend orchestration and hybrid tiered storage.

#### Deliverables

1. **Storage Manager** (`src/manager/manager.rs`)
   - Backend registry and lifecycle management
   - Health monitoring across backends
   - Metrics aggregation
   - Graceful shutdown

2. **Backend Router** (`src/manager/router.rs`)
   - Request routing logic
   - Read/write separation
   - Backend selection strategies

3. **Hybrid Backend** (`src/hybrid/backend.rs`)
   - Tiered storage (L1: Redis, L2: PostgreSQL)
   - Cache-aside pattern
   - Write-through/write-back strategies
   - Cache warming

4. **Cache Policies** (`src/hybrid/cache_policy.rs`)
   - LRU, LFU, TTL policies
   - Eviction strategies
   - Promotion/demotion rules

5. **Sync Manager** (`src/hybrid/sync.rs`)
   - Background synchronization
   - Consistency guarantees
   - Conflict resolution

#### Code Scaffold: Storage Manager

```rust
// src/manager/manager.rs
use std::collections::HashMap;

pub struct StorageManager {
    backends: HashMap<String, Arc<dyn Storage>>,
    router: Arc<StorageRouter>,
    health_monitor: Arc<HealthMonitor>,
    metrics: Arc<AggregateMetrics>,
}

impl StorageManager {
    pub fn builder() -> StorageManagerBuilder {
        StorageManagerBuilder::default()
    }

    pub async fn get_backend(&self, name: &str) -> Option<Arc<dyn Storage>> {
        self.backends.get(name).cloned()
    }

    pub async fn route_request(&self, operation: &str) -> Arc<dyn Storage> {
        self.router.route(operation, &self.backends).await
    }

    pub async fn health_status(&self) -> HealthStatus {
        self.health_monitor.aggregate_health(&self.backends).await
    }

    pub async fn shutdown(&self) -> Result<()> {
        for (name, backend) in &self.backends {
            tracing::info!("Shutting down backend: {}", name);
            backend.close().await?;
        }
        Ok(())
    }
}

#[async_trait]
impl Storage for StorageManager {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let backend = self.route_request("get").await;
        backend.get(key).await
    }

    async fn put(&self, key: &str, value: Vec<u8>) -> Result<()> {
        let backend = self.route_request("put").await;
        backend.put(key, value).await
    }

    // ... delegate other operations
}
```

#### Code Scaffold: Hybrid Backend

```rust
// src/hybrid/backend.rs

pub struct HybridBackend {
    l1_cache: Arc<dyn Storage>,  // Redis
    l2_store: Arc<dyn Storage>,  // PostgreSQL
    policy: Arc<CachePolicy>,
    sync_manager: Arc<SyncManager>,
    metrics: Arc<HybridMetrics>,
}

#[async_trait]
impl Storage for HybridBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let start = Instant::now();

        // Try L1 cache first
        if let Some(value) = self.l1_cache.get(key).await? {
            self.metrics.record_cache_hit("l1");
            return Ok(Some(value));
        }

        // Cache miss - try L2
        if let Some(value) = self.l2_store.get(key).await? {
            self.metrics.record_cache_hit("l2");

            // Promote to L1 cache (async)
            if self.policy.should_promote(key) {
                let l1 = self.l1_cache.clone();
                let key_owned = key.to_string();
                let value_clone = value.clone();
                tokio::spawn(async move {
                    let _ = l1.put(&key_owned, value_clone).await;
                });
            }

            return Ok(Some(value));
        }

        self.metrics.record_cache_miss();
        Ok(None)
    }

    async fn put(&self, key: &str, value: Vec<u8>) -> Result<()> {
        let start = Instant::now();

        // Write-through: write to both L1 and L2
        let (r1, r2) = tokio::join!(
            self.l1_cache.put(key, value.clone()),
            self.l2_store.put(key, value)
        );

        r1?;
        r2?;

        self.metrics.record_operation("put", start.elapsed());
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        // Delete from both tiers
        let (r1, r2) = tokio::join!(
            self.l1_cache.delete(key),
            self.l2_store.delete(key)
        );

        r1?;
        r2?;

        Ok(())
    }

    // ... additional methods
}
```

#### Files Created (Week 8-9): 11 files, ~2,100 LOC

```
src/manager/
├── mod.rs           (80 LOC)
├── manager.rs       (400 LOC)
├── registry.rs      (180 LOC)
├── router.rs        (200 LOC)
├── health.rs        (150 LOC)
└── tests.rs         (250 LOC)

src/hybrid/
├── mod.rs           (100 LOC)
├── backend.rs       (350 LOC)
├── routing.rs       (200 LOC)
├── cache_policy.rs  (180 LOC)
├── sync.rs          (220 LOC)
└── tests.rs         (280 LOC)
```

---

### Phase 6: Supporting Infrastructure (Week 9-10)

**Objective:** Implement resilience patterns, serialization, and observability.

#### Deliverables

1. **Circuit Breaker** (`src/resilience/circuit_breaker.rs`)
   - State machine (Closed, Open, HalfOpen)
   - Failure threshold configuration
   - Automatic recovery

2. **Retry Logic** (`src/resilience/retry.rs`)
   - Exponential backoff
   - Jitter for thundering herd
   - Max retry configuration

3. **Serialization** (`src/serialization/`)
   - Codec trait
   - JSON, Bincode, MessagePack implementations
   - Compression support

4. **Observability** (`src/observability/`)
   - Prometheus metrics
   - OpenTelemetry traces
   - Health check aggregation

#### Code Scaffold: Circuit Breaker

```rust
// src/resilience/circuit_breaker.rs
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq)]
enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    failure_threshold: u32,
    success_threshold: u32,
    timeout: Duration,
    failure_count: Arc<RwLock<u32>>,
    success_count: Arc<RwLock<u32>>,
    last_failure_time: Arc<RwLock<Option<Instant>>>,
}

impl CircuitBreaker {
    pub fn new(
        failure_threshold: u32,
        success_threshold: u32,
        timeout: Duration,
    ) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_threshold,
            success_threshold,
            timeout,
            failure_count: Arc::new(RwLock::new(0)),
            success_count: Arc::new(RwLock::new(0)),
            last_failure_time: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn call<F, T, E>(&self, f: F) -> Result<T>
    where
        F: FnOnce() -> std::result::Result<T, E>,
        E: Into<StorageError>,
    {
        let state = *self.state.read().await;

        match state {
            CircuitState::Open => {
                let last_failure = self.last_failure_time.read().await;
                if let Some(time) = *last_failure {
                    if time.elapsed() > self.timeout {
                        // Try half-open
                        *self.state.write().await = CircuitState::HalfOpen;
                    } else {
                        return Err(StorageError::CircuitBreakerOpen(
                            "Circuit breaker is open".to_string()
                        ));
                    }
                }
            }
            CircuitState::HalfOpen => {
                // Allow one request through
            }
            CircuitState::Closed => {
                // Normal operation
            }
        }

        match f() {
            Ok(result) => {
                self.on_success().await;
                Ok(result)
            }
            Err(err) => {
                self.on_failure().await;
                Err(err.into())
            }
        }
    }

    async fn on_success(&self) {
        let state = *self.state.read().await;

        if state == CircuitState::HalfOpen {
            let mut success_count = self.success_count.write().await;
            *success_count += 1;

            if *success_count >= self.success_threshold {
                *self.state.write().await = CircuitState::Closed;
                *self.failure_count.write().await = 0;
                *success_count = 0;
            }
        }
    }

    async fn on_failure(&self) {
        let mut failure_count = self.failure_count.write().await;
        *failure_count += 1;

        if *failure_count >= self.failure_threshold {
            *self.state.write().await = CircuitState::Open;
            *self.last_failure_time.write().await = Some(Instant::now());
        }
    }
}
```

#### Files Created (Week 9-10): 13 files, ~1,850 LOC

```
src/resilience/
├── mod.rs              (50 LOC)
├── circuit_breaker.rs  (250 LOC)
├── retry.rs            (200 LOC)
├── timeout.rs          (120 LOC)
└── bulkhead.rs         (180 LOC)

src/serialization/
├── mod.rs              (80 LOC)
├── codec.rs            (150 LOC)
├── json.rs             (100 LOC)
├── bincode.rs          (100 LOC)
├── msgpack.rs          (100 LOC)
└── compression.rs      (150 LOC)

src/observability/
├── mod.rs              (80 LOC)
├── metrics.rs          (250 LOC)
├── tracing.rs          (180 LOC)
└── health.rs           (150 LOC)
```

---

### Phase 7: Integration, Testing, and Documentation (Week 10-12)

**Objective:** Comprehensive testing, benchmarking, examples, and documentation.

#### Deliverables

1. **Integration Tests** (`tests/`)
   - PostgreSQL integration tests
   - Redis integration tests
   - Sled integration tests
   - Hybrid backend tests
   - Transaction tests
   - Failover tests

2. **Benchmarks** (`benches/`)
   - Single operation benchmarks
   - Batch operation benchmarks
   - Concurrent access benchmarks
   - Backend comparison benchmarks

3. **Examples** (`examples/`)
   - Basic usage examples
   - Transaction examples
   - Hybrid storage examples
   - Monitoring and observability

4. **Documentation**
   - API documentation (rustdoc)
   - Usage guides
   - Configuration reference
   - Migration guide

#### Files Created (Week 10-12): 20 files, ~3,000 LOC

```
tests/
├── common/mod.rs              (200 LOC)
├── postgres_integration.rs    (400 LOC)
├── redis_integration.rs       (350 LOC)
├── sled_integration.rs        (300 LOC)
├── hybrid_integration.rs      (350 LOC)
├── transaction_tests.rs       (300 LOC)
├── failover_tests.rs          (280 LOC)
└── performance_tests.rs       (300 LOC)

benches/
├── postgres_bench.rs          (300 LOC)
├── redis_bench.rs             (300 LOC)
├── sled_bench.rs              (250 LOC)
└── hybrid_bench.rs            (300 LOC)

examples/
├── basic_usage.rs             (200 LOC)
├── transactions.rs            (250 LOC)
├── batch_operations.rs        (200 LOC)
├── hybrid_storage.rs          (250 LOC)
├── monitoring.rs              (200 LOC)
└── migration_example.rs       (180 LOC)
```

---

## 4. Database Schemas

### 4.1 PostgreSQL Schema

#### Migration 001: Core Storage Tables

```sql
-- migrations/001_create_storage_tables.sql

-- Main key-value storage table
CREATE TABLE IF NOT EXISTS storage_kv (
    id BIGSERIAL PRIMARY KEY,
    key VARCHAR(512) NOT NULL UNIQUE,
    value BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    version INTEGER NOT NULL DEFAULT 1,
    metadata JSONB DEFAULT '{}'::jsonb
);

-- Namespace/tenant support
CREATE TABLE IF NOT EXISTS storage_namespaces (
    id SERIAL PRIMARY KEY,
    namespace VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    quota_bytes BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata JSONB DEFAULT '{}'::jsonb
);

-- Key metadata for advanced features
CREATE TABLE IF NOT EXISTS storage_key_metadata (
    key VARCHAR(512) PRIMARY KEY REFERENCES storage_kv(key) ON DELETE CASCADE,
    namespace_id INTEGER REFERENCES storage_namespaces(id),
    content_type VARCHAR(255),
    encoding VARCHAR(50),
    checksum VARCHAR(64),
    size_bytes BIGINT,
    access_count BIGINT DEFAULT 0,
    last_accessed_at TIMESTAMPTZ,
    tags TEXT[],
    metadata JSONB DEFAULT '{}'::jsonb
);

-- Transaction log for audit and debugging
CREATE TABLE IF NOT EXISTS storage_transaction_log (
    id BIGSERIAL PRIMARY KEY,
    transaction_id UUID NOT NULL,
    operation VARCHAR(50) NOT NULL,
    key VARCHAR(512) NOT NULL,
    old_value BYTEA,
    new_value BYTEA,
    status VARCHAR(50) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata JSONB DEFAULT '{}'::jsonb
);

-- Migration version tracking
CREATE TABLE IF NOT EXISTS storage_migrations (
    id SERIAL PRIMARY KEY,
    version INTEGER NOT NULL UNIQUE,
    name VARCHAR(255) NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    checksum VARCHAR(64)
);

COMMENT ON TABLE storage_kv IS 'Primary key-value storage table';
COMMENT ON TABLE storage_namespaces IS 'Namespace/tenant isolation';
COMMENT ON TABLE storage_key_metadata IS 'Extended metadata for keys';
COMMENT ON TABLE storage_transaction_log IS 'Transaction audit log';
COMMENT ON TABLE storage_migrations IS 'Schema migration tracking';
```

#### Migration 002: Indexes

```sql
-- migrations/002_add_indexes.sql

-- Primary access patterns
CREATE INDEX IF NOT EXISTS idx_storage_kv_key ON storage_kv(key);
CREATE INDEX IF NOT EXISTS idx_storage_kv_created_at ON storage_kv(created_at);
CREATE INDEX IF NOT EXISTS idx_storage_kv_updated_at ON storage_kv(updated_at);
CREATE INDEX IF NOT EXISTS idx_storage_kv_expires_at ON storage_kv(expires_at)
    WHERE expires_at IS NOT NULL;

-- Prefix search support
CREATE INDEX IF NOT EXISTS idx_storage_kv_key_prefix ON storage_kv(key text_pattern_ops);

-- Metadata queries
CREATE INDEX IF NOT EXISTS idx_storage_kv_metadata ON storage_kv USING gin(metadata);

-- Namespace lookups
CREATE INDEX IF NOT EXISTS idx_storage_key_metadata_namespace
    ON storage_key_metadata(namespace_id);

-- Transaction log queries
CREATE INDEX IF NOT EXISTS idx_storage_transaction_log_txn_id
    ON storage_transaction_log(transaction_id);
CREATE INDEX IF NOT EXISTS idx_storage_transaction_log_key
    ON storage_transaction_log(key);
CREATE INDEX IF NOT EXISTS idx_storage_transaction_log_created_at
    ON storage_transaction_log(created_at);

-- Tag searches (GIN index for array contains)
CREATE INDEX IF NOT EXISTS idx_storage_key_metadata_tags
    ON storage_key_metadata USING gin(tags);

COMMENT ON INDEX idx_storage_kv_key IS 'Primary key lookups';
COMMENT ON INDEX idx_storage_kv_key_prefix IS 'Prefix scan support';
COMMENT ON INDEX idx_storage_kv_expires_at IS 'TTL expiration queries';
```

#### Migration 003: Partitioning (for high-volume deployments)

```sql
-- migrations/003_add_partitions.sql

-- Convert storage_kv to partitioned table (for new deployments)
-- Note: This would be applied differently for existing data

-- Range partitioning by created_at (monthly)
CREATE TABLE IF NOT EXISTS storage_kv_partitioned (
    LIKE storage_kv INCLUDING ALL
) PARTITION BY RANGE (created_at);

-- Create partitions for current and next 12 months
CREATE TABLE storage_kv_y2025m01 PARTITION OF storage_kv_partitioned
    FOR VALUES FROM ('2025-01-01') TO ('2025-02-01');
CREATE TABLE storage_kv_y2025m02 PARTITION OF storage_kv_partitioned
    FOR VALUES FROM ('2025-02-01') TO ('2025-03-01');
-- ... continue for 12 months

-- Hash partitioning for transaction log (by transaction_id)
CREATE TABLE IF NOT EXISTS storage_transaction_log_partitioned (
    LIKE storage_transaction_log INCLUDING ALL
) PARTITION BY HASH (transaction_id);

-- Create 16 hash partitions
DO $$
BEGIN
    FOR i IN 0..15 LOOP
        EXECUTE format(
            'CREATE TABLE storage_transaction_log_p%s PARTITION OF storage_transaction_log_partitioned
             FOR VALUES WITH (MODULUS 16, REMAINDER %s)',
            i, i
        );
    END LOOP;
END $$;

COMMENT ON TABLE storage_kv_partitioned IS 'Partitioned key-value storage for high volume';
```

#### Migration 004: Constraints

```sql
-- migrations/004_add_constraints.sql

-- Key constraints
ALTER TABLE storage_kv
    ADD CONSTRAINT chk_storage_kv_key_not_empty
    CHECK (length(key) > 0);

ALTER TABLE storage_kv
    ADD CONSTRAINT chk_storage_kv_key_length
    CHECK (length(key) <= 512);

-- Value size constraints (e.g., max 10MB)
ALTER TABLE storage_kv
    ADD CONSTRAINT chk_storage_kv_value_size
    CHECK (octet_length(value) <= 10485760);

-- Version must be positive
ALTER TABLE storage_kv
    ADD CONSTRAINT chk_storage_kv_version_positive
    CHECK (version > 0);

-- Expiration must be in future (if set)
ALTER TABLE storage_kv
    ADD CONSTRAINT chk_storage_kv_expires_future
    CHECK (expires_at IS NULL OR expires_at > created_at);

-- Namespace constraints
ALTER TABLE storage_namespaces
    ADD CONSTRAINT chk_storage_namespace_name_valid
    CHECK (namespace ~ '^[a-zA-Z0-9_-]+$');

ALTER TABLE storage_namespaces
    ADD CONSTRAINT chk_storage_namespace_quota_positive
    CHECK (quota_bytes IS NULL OR quota_bytes > 0);

-- Transaction log status values
ALTER TABLE storage_transaction_log
    ADD CONSTRAINT chk_storage_txn_log_status
    CHECK (status IN ('pending', 'committed', 'rolled_back', 'failed'));

COMMENT ON CONSTRAINT chk_storage_kv_key_not_empty
    ON storage_kv IS 'Keys cannot be empty';
COMMENT ON CONSTRAINT chk_storage_kv_value_size
    ON storage_kv IS 'Values limited to 10MB';
```

#### Migration 005: Triggers

```sql
-- migrations/005_add_triggers.sql

-- Update updated_at timestamp automatically
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_storage_kv_updated_at
    BEFORE UPDATE ON storage_kv
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Increment version on update
CREATE OR REPLACE FUNCTION increment_version()
RETURNS TRIGGER AS $$
BEGIN
    NEW.version = OLD.version + 1;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_storage_kv_version
    BEFORE UPDATE ON storage_kv
    FOR EACH ROW
    EXECUTE FUNCTION increment_version();

-- Update access count and timestamp
CREATE OR REPLACE FUNCTION update_access_stats()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE storage_key_metadata
    SET access_count = access_count + 1,
        last_accessed_at = NOW()
    WHERE key = NEW.key;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_storage_kv_access
    AFTER SELECT ON storage_kv
    FOR EACH ROW
    EXECUTE FUNCTION update_access_stats();

-- Audit log trigger
CREATE OR REPLACE FUNCTION log_storage_change()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        INSERT INTO storage_transaction_log
            (transaction_id, operation, key, old_value, status)
        VALUES
            (gen_random_uuid(), 'DELETE', OLD.key, OLD.value, 'committed');
        RETURN OLD;
    ELSIF TG_OP = 'UPDATE' THEN
        INSERT INTO storage_transaction_log
            (transaction_id, operation, key, old_value, new_value, status)
        VALUES
            (gen_random_uuid(), 'UPDATE', NEW.key, OLD.value, NEW.value, 'committed');
        RETURN NEW;
    ELSIF TG_OP = 'INSERT' THEN
        INSERT INTO storage_transaction_log
            (transaction_id, operation, key, new_value, status)
        VALUES
            (gen_random_uuid(), 'INSERT', NEW.key, NEW.value, 'committed');
        RETURN NEW;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_storage_kv_audit
    AFTER INSERT OR UPDATE OR DELETE ON storage_kv
    FOR EACH ROW
    EXECUTE FUNCTION log_storage_change();

COMMENT ON FUNCTION update_updated_at_column() IS 'Automatically update updated_at timestamp';
COMMENT ON FUNCTION increment_version() IS 'Auto-increment version on update';
```

#### Migration 006: Stored Procedures

```sql
-- migrations/006_add_functions.sql

-- Bulk insert with conflict handling
CREATE OR REPLACE FUNCTION storage_bulk_upsert(
    keys TEXT[],
    values BYTEA[],
    ttl_seconds INTEGER DEFAULT NULL
)
RETURNS INTEGER AS $$
DECLARE
    inserted INTEGER := 0;
    expires_at_val TIMESTAMPTZ;
BEGIN
    IF ttl_seconds IS NOT NULL THEN
        expires_at_val := NOW() + (ttl_seconds || ' seconds')::INTERVAL;
    END IF;

    FOR i IN 1..array_length(keys, 1) LOOP
        INSERT INTO storage_kv (key, value, expires_at)
        VALUES (keys[i], values[i], expires_at_val)
        ON CONFLICT (key)
        DO UPDATE SET
            value = EXCLUDED.value,
            expires_at = EXCLUDED.expires_at,
            updated_at = NOW();

        inserted := inserted + 1;
    END LOOP;

    RETURN inserted;
END;
$$ LANGUAGE plpgsql;

-- Clean expired keys
CREATE OR REPLACE FUNCTION storage_clean_expired()
RETURNS INTEGER AS $$
DECLARE
    deleted INTEGER;
BEGIN
    DELETE FROM storage_kv
    WHERE expires_at IS NOT NULL
      AND expires_at <= NOW();

    GET DIAGNOSTICS deleted = ROW_COUNT;
    RETURN deleted;
END;
$$ LANGUAGE plpgsql;

-- Get keys by prefix with pagination
CREATE OR REPLACE FUNCTION storage_get_by_prefix(
    prefix TEXT,
    page_size INTEGER DEFAULT 100,
    page_offset INTEGER DEFAULT 0
)
RETURNS TABLE (
    key VARCHAR(512),
    value BYTEA,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ
) AS $$
BEGIN
    RETURN QUERY
    SELECT kv.key, kv.value, kv.created_at, kv.updated_at
    FROM storage_kv kv
    WHERE kv.key LIKE (prefix || '%')
      AND (kv.expires_at IS NULL OR kv.expires_at > NOW())
    ORDER BY kv.key
    LIMIT page_size
    OFFSET page_offset;
END;
$$ LANGUAGE plpgsql;

-- Compare and swap
CREATE OR REPLACE FUNCTION storage_compare_and_swap(
    key_param VARCHAR(512),
    expected_version INTEGER,
    new_value BYTEA
)
RETURNS BOOLEAN AS $$
DECLARE
    updated INTEGER;
BEGIN
    UPDATE storage_kv
    SET value = new_value,
        updated_at = NOW()
    WHERE key = key_param
      AND version = expected_version;

    GET DIAGNOSTICS updated = ROW_COUNT;
    RETURN updated > 0;
END;
$$ LANGUAGE plpgsql;

-- Get storage statistics
CREATE OR REPLACE FUNCTION storage_get_stats()
RETURNS TABLE (
    total_keys BIGINT,
    total_size_bytes BIGINT,
    expired_keys BIGINT,
    namespaces INTEGER
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        COUNT(*)::BIGINT as total_keys,
        SUM(octet_length(value))::BIGINT as total_size_bytes,
        COUNT(*) FILTER (WHERE expires_at IS NOT NULL AND expires_at <= NOW())::BIGINT as expired_keys,
        (SELECT COUNT(*)::INTEGER FROM storage_namespaces) as namespaces
    FROM storage_kv;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION storage_bulk_upsert IS 'Efficiently insert/update multiple keys';
COMMENT ON FUNCTION storage_clean_expired IS 'Remove expired keys and return count';
COMMENT ON FUNCTION storage_get_by_prefix IS 'Get keys by prefix with pagination';
COMMENT ON FUNCTION storage_compare_and_swap IS 'Atomic compare-and-swap operation';
COMMENT ON FUNCTION storage_get_stats IS 'Get storage statistics';
```

#### Migration 007: Materialized Views

```sql
-- migrations/007_add_views.sql

-- Storage statistics view
CREATE MATERIALIZED VIEW IF NOT EXISTS storage_stats_mv AS
SELECT
    COUNT(*) as total_keys,
    SUM(octet_length(value)) as total_size_bytes,
    AVG(octet_length(value)) as avg_value_size,
    MIN(created_at) as oldest_key_created,
    MAX(updated_at) as newest_key_updated,
    COUNT(*) FILTER (WHERE expires_at IS NOT NULL) as keys_with_ttl,
    COUNT(*) FILTER (WHERE expires_at IS NOT NULL AND expires_at <= NOW()) as expired_keys
FROM storage_kv;

CREATE UNIQUE INDEX ON storage_stats_mv ((true));

-- Namespace statistics view
CREATE MATERIALIZED VIEW IF NOT EXISTS storage_namespace_stats_mv AS
SELECT
    n.id,
    n.namespace,
    COUNT(m.key) as key_count,
    COALESCE(SUM(m.size_bytes), 0) as total_size_bytes,
    n.quota_bytes,
    CASE
        WHEN n.quota_bytes IS NOT NULL
        THEN (COALESCE(SUM(m.size_bytes), 0)::FLOAT / n.quota_bytes * 100)
        ELSE NULL
    END as quota_usage_percent
FROM storage_namespaces n
LEFT JOIN storage_key_metadata m ON n.id = m.namespace_id
GROUP BY n.id, n.namespace, n.quota_bytes;

CREATE UNIQUE INDEX ON storage_namespace_stats_mv (id);

-- Hot keys view (most accessed)
CREATE MATERIALIZED VIEW IF NOT EXISTS storage_hot_keys_mv AS
SELECT
    m.key,
    m.access_count,
    m.last_accessed_at,
    kv.created_at,
    m.size_bytes
FROM storage_key_metadata m
JOIN storage_kv kv ON m.key = kv.key
ORDER BY m.access_count DESC
LIMIT 1000;

CREATE UNIQUE INDEX ON storage_hot_keys_mv (key);

-- Refresh function for materialized views
CREATE OR REPLACE FUNCTION refresh_storage_views()
RETURNS void AS $$
BEGIN
    REFRESH MATERIALIZED VIEW CONCURRENTLY storage_stats_mv;
    REFRESH MATERIALIZED VIEW CONCURRENTLY storage_namespace_stats_mv;
    REFRESH MATERIALIZED VIEW CONCURRENTLY storage_hot_keys_mv;
END;
$$ LANGUAGE plpgsql;

COMMENT ON MATERIALIZED VIEW storage_stats_mv IS 'Cached storage statistics';
COMMENT ON MATERIALIZED VIEW storage_namespace_stats_mv IS 'Per-namespace usage statistics';
COMMENT ON MATERIALIZED VIEW storage_hot_keys_mv IS 'Most frequently accessed keys';
```

#### Migration 008: Monitoring Tables

```sql
-- migrations/008_add_monitoring.sql

-- Operation metrics table
CREATE TABLE IF NOT EXISTS storage_operation_metrics (
    id BIGSERIAL PRIMARY KEY,
    operation VARCHAR(50) NOT NULL,
    backend VARCHAR(50) NOT NULL,
    latency_ms INTEGER NOT NULL,
    success BOOLEAN NOT NULL,
    error_type VARCHAR(100),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata JSONB DEFAULT '{}'::jsonb
);

CREATE INDEX idx_storage_operation_metrics_operation
    ON storage_operation_metrics(operation);
CREATE INDEX idx_storage_operation_metrics_backend
    ON storage_operation_metrics(backend);
CREATE INDEX idx_storage_operation_metrics_recorded_at
    ON storage_operation_metrics(recorded_at);

-- Health check results
CREATE TABLE IF NOT EXISTS storage_health_checks (
    id BIGSERIAL PRIMARY KEY,
    backend VARCHAR(50) NOT NULL,
    status VARCHAR(20) NOT NULL,
    latency_ms INTEGER,
    error_message TEXT,
    checked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata JSONB DEFAULT '{}'::jsonb
);

CREATE INDEX idx_storage_health_checks_backend
    ON storage_health_checks(backend);
CREATE INDEX idx_storage_health_checks_checked_at
    ON storage_health_checks(checked_at);

-- Connection pool metrics
CREATE TABLE IF NOT EXISTS storage_pool_metrics (
    id BIGSERIAL PRIMARY KEY,
    backend VARCHAR(50) NOT NULL,
    total_connections INTEGER NOT NULL,
    idle_connections INTEGER NOT NULL,
    active_connections INTEGER NOT NULL,
    wait_count BIGINT NOT NULL DEFAULT 0,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_storage_pool_metrics_backend
    ON storage_pool_metrics(backend);
CREATE INDEX idx_storage_pool_metrics_recorded_at
    ON storage_pool_metrics(recorded_at);

-- Retention policy: keep only last 7 days of metrics
CREATE OR REPLACE FUNCTION cleanup_old_metrics()
RETURNS void AS $$
BEGIN
    DELETE FROM storage_operation_metrics
    WHERE recorded_at < NOW() - INTERVAL '7 days';

    DELETE FROM storage_health_checks
    WHERE checked_at < NOW() - INTERVAL '7 days';

    DELETE FROM storage_pool_metrics
    WHERE recorded_at < NOW() - INTERVAL '7 days';
END;
$$ LANGUAGE plpgsql;

COMMENT ON TABLE storage_operation_metrics IS 'Storage operation performance metrics';
COMMENT ON TABLE storage_health_checks IS 'Backend health check history';
COMMENT ON TABLE storage_pool_metrics IS 'Connection pool statistics';
```

#### Migration 009: Audit Log

```sql
-- migrations/009_add_audit_log.sql

-- Enhanced audit log table
CREATE TABLE IF NOT EXISTS storage_audit_log (
    id BIGSERIAL PRIMARY KEY,
    event_id UUID NOT NULL DEFAULT gen_random_uuid(),
    event_type VARCHAR(50) NOT NULL,
    key VARCHAR(512),
    namespace_id INTEGER REFERENCES storage_namespaces(id),
    operation VARCHAR(50) NOT NULL,
    status VARCHAR(20) NOT NULL,
    user_id VARCHAR(255),
    source_ip INET,
    old_value_hash VARCHAR(64),
    new_value_hash VARCHAR(64),
    metadata JSONB DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_storage_audit_log_event_type
    ON storage_audit_log(event_type);
CREATE INDEX idx_storage_audit_log_key
    ON storage_audit_log(key);
CREATE INDEX idx_storage_audit_log_namespace
    ON storage_audit_log(namespace_id);
CREATE INDEX idx_storage_audit_log_created_at
    ON storage_audit_log(created_at);
CREATE INDEX idx_storage_audit_log_user
    ON storage_audit_log(user_id);

-- Partition by month
ALTER TABLE storage_audit_log
    PARTITION BY RANGE (created_at);

-- Audit log retention (90 days)
CREATE OR REPLACE FUNCTION cleanup_old_audit_logs()
RETURNS void AS $$
BEGIN
    DELETE FROM storage_audit_log
    WHERE created_at < NOW() - INTERVAL '90 days';
END;
$$ LANGUAGE plpgsql;

COMMENT ON TABLE storage_audit_log IS 'Comprehensive audit trail for all storage operations';
```

#### Migration 010: Performance Optimizations

```sql
-- migrations/010_optimize_performance.sql

-- Enable parallel query execution
ALTER TABLE storage_kv SET (parallel_workers = 4);
ALTER TABLE storage_transaction_log SET (parallel_workers = 4);

-- Adjust autovacuum settings for high-throughput tables
ALTER TABLE storage_kv SET (
    autovacuum_vacuum_scale_factor = 0.05,
    autovacuum_analyze_scale_factor = 0.02,
    autovacuum_vacuum_cost_limit = 2000
);

ALTER TABLE storage_transaction_log SET (
    autovacuum_vacuum_scale_factor = 0.05,
    autovacuum_analyze_scale_factor = 0.02
);

-- Fill factor optimization for frequently updated tables
ALTER TABLE storage_kv SET (fillfactor = 80);

-- Covering indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_storage_kv_key_value_covering
    ON storage_kv(key) INCLUDE (value, updated_at);

CREATE INDEX IF NOT EXISTS idx_storage_kv_expires_covering
    ON storage_kv(expires_at) INCLUDE (key)
    WHERE expires_at IS NOT NULL;

-- Statistics targets for better query planning
ALTER TABLE storage_kv ALTER COLUMN key SET STATISTICS 1000;
ALTER TABLE storage_kv ALTER COLUMN metadata SET STATISTICS 1000;

-- Create statistics object for multi-column queries
CREATE STATISTICS storage_kv_key_created_stats
    ON key, created_at FROM storage_kv;

COMMENT ON INDEX idx_storage_kv_key_value_covering IS 'Covering index for key-value lookups';
```

### 4.2 Redis Data Structures

Redis is schema-less, but we define data structures and key patterns:

```
Key Patterns:
-----------
storage:kv:{key}              - Key-value data (STRING)
storage:meta:{key}            - Key metadata (HASH)
storage:ttl:{key}             - TTL tracking (STRING with EXPIRE)
storage:namespace:{ns}        - Namespace keys (SET)
storage:locks:{key}           - Distributed locks (STRING with EXPIRE)
storage:stats                 - Global statistics (HASH)
storage:health                - Health check status (HASH)

Data Structures:
---------------
1. Key-Value: STRING type
   - SET storage:kv:mykey "binary_data"
   - GET storage:kv:mykey

2. Metadata: HASH type
   - HSET storage:meta:mykey created_at "2025-11-10T10:00:00Z"
   - HSET storage:meta:mykey size_bytes "1024"
   - HSET storage:meta:mykey version "3"

3. Namespace Index: SET type
   - SADD storage:namespace:prod "key1" "key2" "key3"

4. Statistics: HASH type
   - HINCRBY storage:stats total_keys 1
   - HINCRBY storage:stats total_operations 1
   - HSET storage:stats last_operation_at "2025-11-10T10:00:00Z"
```

### 4.3 Sled Database Structure

Sled is a key-value embedded database. Structure:

```
Tree Structure:
--------------
default tree: Main key-value storage
- Keys: Raw bytes
- Values: Serialized data (bincode/msgpack)

metadata tree: Key metadata
- Keys: Same as main tree
- Values: JSON metadata

System Trees:
------------
_stats: System statistics
_config: Runtime configuration
_checkpoints: Checkpoint markers
```

---

## 5. Module Interfaces

### 5.1 Storage Trait (Core Interface)

```rust
// src/traits/storage.rs

use async_trait::async_trait;
use std::time::Duration;
use crate::{Result, types::*};

/// Core storage interface implemented by all backends
#[async_trait]
pub trait Storage: Send + Sync {
    // Basic CRUD operations
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn put(&self, key: &str, value: Vec<u8>) -> Result<()>;
    async fn put_with_ttl(&self, key: &str, value: Vec<u8>, ttl: Duration) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<()>;
    async fn exists(&self, key: &str) -> Result<bool>;

    // Batch operations
    async fn get_many(&self, keys: &[&str]) -> Result<Vec<Option<Vec<u8>>>>;
    async fn put_many(&self, items: &[(&str, Vec<u8>)]) -> Result<()>;
    async fn delete_many(&self, keys: &[&str]) -> Result<()>;

    // Range queries
    async fn scan_prefix(&self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>>;
    async fn scan_range(&self, start: &str, end: &str) -> Result<Vec<(String, Vec<u8>)>>;

    // Atomic operations
    async fn compare_and_swap(
        &self,
        key: &str,
        old_value: Option<Vec<u8>>,
        new_value: Vec<u8>,
    ) -> Result<bool>;

    // Observability
    async fn health_check(&self) -> Result<HealthStatus>;
    async fn metrics(&self) -> Result<StorageMetrics>;

    // Lifecycle
    async fn close(&self) -> Result<()>;
}
```

### 5.2 Transactional Trait

```rust
// src/traits/transactional.rs

use async_trait::async_trait;
use crate::{Result, types::*};

/// Transaction support for backends that support ACID transactions
#[async_trait]
pub trait Transactional: Storage {
    type Transaction: Transaction;

    /// Begin a new transaction
    async fn begin(&self) -> Result<Self::Transaction>;

    /// Begin with specific isolation level
    async fn begin_with_isolation(
        &self,
        isolation: IsolationLevel,
    ) -> Result<Self::Transaction>;
}

/// Transaction handle
#[async_trait]
pub trait Transaction: Send {
    async fn get(&mut self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn put(&mut self, key: &str, value: Vec<u8>) -> Result<()>;
    async fn delete(&mut self, key: &str) -> Result<()>;

    /// Commit the transaction
    async fn commit(self) -> Result<()>;

    /// Rollback the transaction
    async fn rollback(self) -> Result<()>;

    /// Create a savepoint
    async fn savepoint(&mut self, name: &str) -> Result<()>;

    /// Rollback to savepoint
    async fn rollback_to(&mut self, name: &str) -> Result<()>;
}

#[derive(Debug, Clone, Copy)]
pub enum IsolationLevel {
    ReadUncommitted,
    ReadCommitted,
    RepeatableRead,
    Serializable,
}
```

### 5.3 Configuration Types

```rust
// src/config.rs

use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresConfig {
    pub connection_string: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout_secs: u64,
    pub idle_timeout_secs: u64,
    pub max_lifetime_secs: u64,
    pub enable_statement_cache: bool,
    pub statement_cache_capacity: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub max_connections: usize,
    pub connect_timeout_secs: u64,
    pub response_timeout_secs: u64,
    pub cluster_enabled: bool,
    pub sentinel_enabled: bool,
    pub sentinel_master_name: Option<String>,
    pub sentinel_nodes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SledConfig {
    pub path: String,
    pub cache_capacity_bytes: u64,
    pub flush_interval_ms: u64,
    pub compression: bool,
    pub mode: SledMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SledMode {
    HighThroughput,
    LowSpace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridConfig {
    pub l1_backend: BackendType,
    pub l2_backend: BackendType,
    pub cache_policy: CachePolicy,
    pub write_strategy: WriteStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackendType {
    Postgres(PostgresConfig),
    Redis(RedisConfig),
    Sled(SledConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CachePolicy {
    LRU,
    LFU,
    TTL { duration: Duration },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WriteStrategy {
    WriteThrough,
    WriteBack { flush_interval: Duration },
    WriteBehind,
}
```

### 5.4 Types and Common Structures

```rust
// src/types.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub healthy: bool,
    pub backend: String,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
    pub checked_at: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMetrics {
    pub backend: String,
    pub total_operations: u64,
    pub successful_operations: u64,
    pub failed_operations: u64,
    pub avg_latency_ms: f64,
    pub p50_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub total_keys: Option<u64>,
    pub total_size_bytes: Option<u64>,
    pub connection_pool_size: Option<u32>,
    pub idle_connections: Option<u32>,
    pub collected_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMetadata {
    pub key: String,
    pub size_bytes: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub version: u32,
    pub checksum: Option<String>,
    pub content_type: Option<String>,
    pub tags: Vec<String>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct QueryOptions {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub order: QueryOrder,
}

#[derive(Debug, Clone, Copy)]
pub enum QueryOrder {
    Ascending,
    Descending,
}
```

---

## 6. Testing Strategy

### 6.1 Unit Tests

**Location:** Within each module (`src/*/tests.rs`)

**Coverage:**
- Error handling for all code paths
- Edge cases (empty keys, large values, special characters)
- Serialization/deserialization
- Configuration validation
- Utility functions

**Example:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_postgres_backend_basic_operations() {
        let config = PostgresConfig::default();
        let backend = PostgresBackend::new(config).await.unwrap();

        // Put
        backend.put("test_key", b"test_value".to_vec()).await.unwrap();

        // Get
        let value = backend.get("test_key").await.unwrap();
        assert_eq!(value, Some(b"test_value".to_vec()));

        // Delete
        backend.delete("test_key").await.unwrap();
        assert_eq!(backend.get("test_key").await.unwrap(), None);
    }
}
```

### 6.2 Integration Tests

**Location:** `tests/` directory

**Requirements:**
- Docker Compose for test infrastructure (PostgreSQL, Redis)
- Temporary directories for Sled
- Test data generators
- Cleanup between tests

**Test Categories:**

1. **Backend Integration** (`tests/*_integration.rs`)
   - All CRUD operations
   - Batch operations
   - Range queries
   - TTL expiration
   - Error scenarios

2. **Transaction Tests** (`tests/transaction_tests.rs`)
   - ACID guarantees
   - Rollback scenarios
   - Isolation levels
   - Concurrent transactions

3. **Failover Tests** (`tests/failover_tests.rs`)
   - Connection loss recovery
   - Circuit breaker activation
   - Retry mechanisms
   - Health check failures

4. **Performance Tests** (`tests/performance_tests.rs`)
   - Throughput under load
   - Latency percentiles
   - Connection pool saturation
   - Memory usage

### 6.3 Benchmarks

**Location:** `benches/` directory

**Benchmark Suites:**

1. **Single Operation Benchmarks**
   - Get latency
   - Put latency
   - Delete latency
   - Compare backend performance

2. **Batch Operation Benchmarks**
   - Batch get (10, 100, 1000 keys)
   - Batch put
   - Pipeline efficiency

3. **Concurrent Access Benchmarks**
   - Read concurrency (10, 50, 100 threads)
   - Write concurrency
   - Mixed workloads

4. **Memory Benchmarks**
   - Memory usage per key
   - Connection pool overhead
   - Cache efficiency

**Example:**
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn postgres_get_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let backend = rt.block_on(async {
        PostgresBackend::new(test_config()).await.unwrap()
    });

    c.bench_function("postgres_get", |b| {
        b.to_async(&rt).iter(|| async {
            backend.get(black_box("bench_key")).await.unwrap()
        });
    });
}

criterion_group!(benches, postgres_get_benchmark);
criterion_main!(benches);
```

### 6.4 Property-Based Testing

Use `proptest` for property-based testing:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_roundtrip_any_data(key in "[a-zA-Z0-9_-]{1,100}",
                                value in prop::collection::vec(any::<u8>(), 0..1000)) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let backend = PostgresBackend::new(test_config()).await.unwrap();
            backend.put(&key, value.clone()).await.unwrap();
            let retrieved = backend.get(&key).await.unwrap();
            assert_eq!(retrieved, Some(value));
        });
    }
}
```

### 6.5 Test Infrastructure

**Docker Compose** (`tests/docker-compose.yml`):
```yaml
version: '3.8'
services:
  postgres:
    image: postgres:16
    environment:
      POSTGRES_PASSWORD: test
      POSTGRES_DB: storage_test
    ports:
      - "5432:5432"
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 5s
      timeout: 5s
      retries: 5

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 5s
      timeout: 5s
      retries: 5
```

### 6.6 Continuous Integration

GitHub Actions workflow for CI:
```yaml
name: Storage Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: postgres:16
        env:
          POSTGRES_PASSWORD: test
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5

      redis:
        image: redis:7-alpine
        options: >-
          --health-cmd "redis-cli ping"
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5

    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Run tests
        run: cargo test --all-features

      - name: Run benchmarks
        run: cargo bench --no-run
```

---

## 7. Lines of Code Estimates

### 7.1 Breakdown by Phase

| Phase | Component | Files | LOC | Tests | Total LOC |
|-------|-----------|-------|-----|-------|-----------|
| 1 | Core Framework | 10 | 1,300 | 200 | 1,500 |
| 2 | PostgreSQL Backend | 10 | 2,500 | 400 | 2,900 |
| 3 | Redis Backend | 9 | 2,200 | 350 | 2,550 |
| 4 | Sled Backend | 7 | 1,400 | 300 | 1,700 |
| 5 | Manager & Hybrid | 11 | 2,100 | 530 | 2,630 |
| 6 | Supporting | 13 | 1,850 | 400 | 2,250 |
| 7 | Integration & Docs | 20 | 1,000 | 2,480 | 3,480 |
| **Total** | | **62** | **9,500** | **4,000** | **13,500** |

### 7.2 Code Distribution

```
Core Implementation:     9,500 LOC (70%)
Unit Tests:             1,500 LOC (11%)
Integration Tests:      2,500 LOC (19%)
Total:                 13,500 LOC
```

### 7.3 Complexity Breakdown

| Complexity | LOC | Percentage |
|------------|-----|------------|
| Simple (CRUD, utils) | 3,500 | 37% |
| Medium (backends, pools) | 4,000 | 42% |
| Complex (transactions, hybrid) | 2,000 | 21% |

---

## 8. Dependencies

### 8.1 Core Dependencies

Already defined in workspace `Cargo.toml`:

```toml
[dependencies]
# Async runtime
tokio = { workspace = true }
async-trait = { workspace = true }
futures = { workspace = true }

# Database
sqlx = { workspace = true }
redis = { workspace = true }
sled = { workspace = true }
deadpool-redis = { workspace = true }

# Serialization
serde = { workspace = true }
serde_json = { workspace = true }
bincode = { workspace = true }
rmp-serde = { workspace = true }

# Observability
tracing = { workspace = true }
prometheus-client = { workspace = true }
opentelemetry = { workspace = true }

# Error handling
thiserror = { workspace = true }
anyhow = { workspace = true }

# Utilities
uuid = { workspace = true }
chrono = { workspace = true }
dashmap = { workspace = true }
```

### 8.2 Storage Crate Cargo.toml

```toml
[package]
name = "llm-optimizer-storage"
version.workspace = true
edition.workspace = true

[dependencies]
# Core
tokio = { workspace = true }
async-trait = { workspace = true }
futures = { workspace = true }

# Database backends
sqlx = { workspace = true }
redis = { workspace = true }
sled = { workspace = true }
deadpool-redis = { workspace = true }

# Serialization
serde = { workspace = true }
serde_json = { workspace = true }
bincode = { workspace = true }
rmp-serde = { workspace = true }

# Compression
lz4 = { workspace = true }
snap = { workspace = true }
zstd = { workspace = true }

# Observability
tracing = { workspace = true }
prometheus-client = { workspace = true }
opentelemetry = { workspace = true }

# Resilience
failsafe = { workspace = true }

# Utilities
thiserror = { workspace = true }
anyhow = { workspace = true }
uuid = { workspace = true }
chrono = { workspace = true }
dashmap = { workspace = true }
sha2 = { workspace = true }

[dev-dependencies]
tokio-test = "0.4"
mockall = { workspace = true }
tempfile = { workspace = true }
criterion = { workspace = true }
proptest = "1.4"
testcontainers = "0.15"

[[bench]]
name = "postgres_bench"
harness = false

[[bench]]
name = "redis_bench"
harness = false

[[bench]]
name = "sled_bench"
harness = false
```

### 8.3 Dependency Justification

| Dependency | Purpose | Alternatives |
|------------|---------|--------------|
| sqlx | PostgreSQL with compile-time checked queries | diesel, tokio-postgres |
| redis | High-level Redis client | redis-rs (lower level) |
| sled | Embedded key-value store | rocksdb, redb |
| deadpool-redis | Connection pooling for Redis | r2d2-redis |
| failsafe | Circuit breaker implementation | Custom implementation |
| lz4/snap/zstd | Compression algorithms | flate2 |

---

## 9. Risk Mitigation

### 9.1 Technical Risks

| Risk | Impact | Likelihood | Mitigation |
|------|--------|------------|------------|
| Database migration failures | High | Medium | Comprehensive migration tests, rollback scripts, version tracking |
| Connection pool exhaustion | High | Medium | Configurable pool sizes, monitoring, circuit breakers |
| Data corruption | Critical | Low | Checksums, integrity checks, backups, transaction logging |
| Performance degradation | High | Medium | Benchmarking, performance tests, query optimization |
| Redis cluster failover | Medium | Medium | Sentinel support, automatic reconnection, health checks |
| Sled data loss | Medium | Low | Regular flushes, transaction logs, backup strategy |

### 9.2 Operational Risks

| Risk | Impact | Likelihood | Mitigation |
|------|--------|------------|------------|
| Dependency vulnerabilities | Medium | Medium | Regular dependency audits, automated security scanning |
| Breaking API changes | High | Low | Semantic versioning, deprecation warnings, migration guides |
| Insufficient documentation | Medium | Medium | Comprehensive rustdoc, examples, guides |
| Performance bottlenecks | High | Medium | Continuous benchmarking, profiling, optimization |

### 9.3 Mitigation Strategies

1. **Comprehensive Testing**
   - 80%+ code coverage
   - Integration tests with real databases
   - Property-based testing for edge cases
   - Chaos engineering for resilience

2. **Observability**
   - Detailed metrics for all operations
   - Distributed tracing
   - Health checks and alerting
   - Performance dashboards

3. **Graceful Degradation**
   - Circuit breakers for failing backends
   - Fallback strategies
   - Retry with exponential backoff
   - Request timeouts

4. **Documentation**
   - API documentation (rustdoc)
   - Architecture diagrams
   - Configuration guides
   - Migration guides
   - Troubleshooting guides

5. **Validation**
   - Schema validation
   - Configuration validation
   - Runtime assertions
   - Integrity checks

---

## 10. Validation Criteria

### 10.1 Functional Requirements

| Requirement | Validation Method | Success Criteria |
|-------------|-------------------|------------------|
| All CRUD operations work | Integration tests | 100% pass rate |
| Batch operations complete | Performance tests | <100ms for 100 items |
| Transactions are ACID-compliant | Transaction tests | All isolation levels work |
| TTL expiration works | Time-based tests | Keys expire within 1s of TTL |
| Range queries return correct data | Integration tests | Sorted, complete results |
| Health checks detect failures | Failover tests | Detection within 5s |

### 10.2 Non-Functional Requirements

| Requirement | Validation Method | Success Criteria |
|-------------|-------------------|------------------|
| Read latency | Benchmarks | p95 <10ms (PostgreSQL), <2ms (Redis) |
| Write latency | Benchmarks | p95 <20ms (PostgreSQL), <5ms (Redis) |
| Throughput | Load tests | >10,000 ops/sec per backend |
| Memory usage | Profiling | <100MB per 100k keys |
| Connection pool efficiency | Monitoring | >90% connection reuse |
| Error rate | Integration tests | <0.1% under normal load |

### 10.3 Quality Gates

1. **Code Quality**
   - Clippy lints pass (no warnings)
   - Code coverage >80%
   - No compiler warnings
   - Rustfmt compliance

2. **Performance**
   - No performance regression >10%
   - Benchmarks complete successfully
   - Memory leaks detected (valgrind)
   - CPU profiling acceptable

3. **Documentation**
   - All public APIs documented
   - Examples compile and run
   - README comprehensive
   - Migration guide complete

4. **Testing**
   - All tests pass
   - Integration tests on CI
   - Property tests pass (10000 iterations)
   - Stress tests complete

---

## 11. Deployment Checklist

### 11.1 Pre-Deployment

- [ ] All tests passing (unit, integration, performance)
- [ ] Benchmarks show acceptable performance
- [ ] Code review completed
- [ ] Documentation updated
- [ ] Migration scripts tested
- [ ] Rollback plan documented
- [ ] Monitoring dashboards created
- [ ] Alert rules configured

### 11.2 Infrastructure Setup

**PostgreSQL:**
- [ ] Database created
- [ ] User and permissions configured
- [ ] Connection pooling configured
- [ ] Backups configured
- [ ] Replication setup (if needed)
- [ ] Monitoring enabled

**Redis:**
- [ ] Redis instance deployed
- [ ] Persistence configured (RDB/AOF)
- [ ] Memory limits set
- [ ] Eviction policy configured
- [ ] Sentinel/Cluster setup (if needed)
- [ ] Monitoring enabled

**Sled:**
- [ ] Storage directory created
- [ ] Permissions set correctly
- [ ] Backup strategy defined
- [ ] Disk space monitored

### 11.3 Deployment Steps

1. **Database Migrations**
   ```bash
   # Run migrations
   sqlx migrate run

   # Verify migration status
   sqlx migrate info
   ```

2. **Configuration**
   ```bash
   # Set environment variables
   export DATABASE_URL="postgresql://..."
   export REDIS_URL="redis://..."
   export SLED_PATH="/var/lib/storage/sled"

   # Validate configuration
   cargo run --bin storage-cli -- config validate
   ```

3. **Service Deployment**
   ```bash
   # Build release binary
   cargo build --release --package llm-optimizer-storage

   # Run health check
   cargo run --bin storage-cli -- health-check

   # Start service
   systemctl start llm-optimizer
   ```

4. **Smoke Tests**
   ```bash
   # Basic operations
   cargo run --bin storage-cli -- put test_key test_value
   cargo run --bin storage-cli -- get test_key
   cargo run --bin storage-cli -- delete test_key

   # Performance check
   cargo run --bin storage-cli -- benchmark
   ```

### 11.4 Post-Deployment

- [ ] Verify all backends healthy
- [ ] Check metrics dashboard
- [ ] Verify alerts working
- [ ] Test key operations
- [ ] Monitor error rates
- [ ] Check connection pools
- [ ] Verify data persistence
- [ ] Test failover (if applicable)

### 11.5 Monitoring Setup

**Metrics to Monitor:**
- Operation latency (p50, p95, p99)
- Throughput (ops/sec)
- Error rate
- Connection pool usage
- Memory usage
- Disk usage (Sled)
- Cache hit rate (hybrid)

**Alerts to Configure:**
- Error rate >1%
- Latency p95 >100ms
- Connection pool >90% utilized
- Disk space <10% free
- Backend health check failures
- Circuit breaker open

### 11.6 Documentation Deliverables

- [ ] API documentation (rustdoc)
- [ ] Configuration reference
- [ ] Deployment guide
- [ ] Migration guide
- [ ] Troubleshooting guide
- [ ] Performance tuning guide
- [ ] Backup and recovery procedures

---

## 12. Timeline and Milestones

### 12.1 Detailed Timeline

| Week | Phase | Deliverables | Status |
|------|-------|--------------|--------|
| 1-2 | Phase 1: Core Framework | Traits, types, errors, config | Not Started |
| 3-4 | Phase 2: PostgreSQL Backend | Backend impl, migrations, tests | Not Started |
| 5-6 | Phase 3: Redis Backend | Backend impl, cluster support, tests | Not Started |
| 7 | Phase 4: Sled Backend | Backend impl, transactions, tests | Not Started |
| 8-9 | Phase 5: Manager & Hybrid | Storage manager, tiered storage | Not Started |
| 9-10 | Phase 6: Supporting | Resilience, serialization, observability | Not Started |
| 10-12 | Phase 7: Integration & Docs | Tests, benchmarks, examples, docs | Not Started |

### 12.2 Milestones

1. **M1: Core Framework Complete** (End of Week 2)
   - All traits defined
   - Type system complete
   - Error handling implemented
   - Configuration structures ready

2. **M2: PostgreSQL Backend Complete** (End of Week 4)
   - Full Storage trait implementation
   - All migrations written and tested
   - Connection pooling working
   - Integration tests passing

3. **M3: All Backends Complete** (End of Week 7)
   - PostgreSQL, Redis, Sled all implemented
   - All backend tests passing
   - Performance benchmarks baseline

4. **M4: Storage Manager Complete** (End of Week 9)
   - Multi-backend orchestration
   - Hybrid tiered storage
   - Health monitoring
   - Metrics aggregation

5. **M5: Production Ready** (End of Week 12)
   - All tests passing
   - Documentation complete
   - Benchmarks satisfactory
   - Deployment tested

---

## 13. Success Metrics

### 13.1 Development Metrics

- **Code Coverage:** Target 80%+
- **Documentation Coverage:** Target 100% for public APIs
- **Test Pass Rate:** Target 100%
- **Benchmark Performance:** No regression >10%

### 13.2 Operational Metrics

- **Availability:** Target 99.9%
- **Latency (p95):**
  - PostgreSQL: <10ms reads, <20ms writes
  - Redis: <2ms reads, <5ms writes
  - Sled: <1ms reads, <5ms writes
- **Throughput:** >10,000 ops/sec per backend
- **Error Rate:** <0.1% under normal load

### 13.3 Quality Metrics

- **Zero critical bugs** in production
- **Mean Time to Recovery (MTTR):** <5 minutes
- **Change failure rate:** <5%
- **Deployment frequency:** Weekly (during development)

---

## 14. Future Enhancements

Beyond the initial implementation, consider these enhancements:

1. **Multi-Region Support**
   - Cross-region replication
   - Geo-routing
   - Conflict resolution (CRDTs)

2. **Advanced Caching**
   - Adaptive cache sizing
   - Machine learning-based eviction
   - Predictive cache warming

3. **Query Optimization**
   - Query planner
   - Automatic index creation
   - Query result caching

4. **Data Lifecycle Management**
   - Automatic archiving
   - Tiered storage policies
   - Compliance features (GDPR)

5. **Enhanced Security**
   - Encryption at rest
   - Field-level encryption
   - Audit logging
   - Access control (RBAC)

6. **Additional Backends**
   - S3-compatible object storage
   - DynamoDB
   - Cassandra
   - ScyllaDB

---

## 15. References

### 15.1 External Documentation

- **PostgreSQL:** https://www.postgresql.org/docs/
- **Redis:** https://redis.io/documentation
- **Sled:** https://docs.rs/sled/
- **sqlx:** https://docs.rs/sqlx/
- **deadpool-redis:** https://docs.rs/deadpool-redis/

### 15.2 Internal Documentation

- Architecture: `/docs/ARCHITECTURE.md`
- Stream Processor State: `/docs/STREAM_PROCESSOR_STATE_IMPLEMENTATION_SUMMARY.md`
- Distributed State: `/docs/DISTRIBUTED_STATE_ARCHITECTURE.md`
- Deployment Guide: `/docs/DEPLOYMENT_GUIDE.md`

### 15.3 Related RFCs

- RFC-001: Storage Layer Design
- RFC-002: Multi-Backend Strategy
- RFC-003: Transaction Semantics

---

## Appendix A: Example Usage

### A.1 Basic Usage

```rust
use llm_optimizer_storage::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize PostgreSQL backend
    let config = PostgresConfig {
        connection_string: "postgresql://localhost/storage".to_string(),
        max_connections: 20,
        ..Default::default()
    };

    let storage = PostgresBackend::new(config).await?;

    // Put a value
    storage.put("my_key", b"my_value".to_vec()).await?;

    // Get a value
    let value = storage.get("my_key").await?;
    assert_eq!(value, Some(b"my_value".to_vec()));

    // Delete
    storage.delete("my_key").await?;

    Ok(())
}
```

### A.2 Hybrid Storage

```rust
use llm_optimizer_storage::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup Redis (L1 cache)
    let redis_config = RedisConfig {
        url: "redis://localhost:6379".to_string(),
        ..Default::default()
    };
    let redis = RedisBackend::new(redis_config).await?;

    // Setup PostgreSQL (L2 persistent)
    let postgres_config = PostgresConfig {
        connection_string: "postgresql://localhost/storage".to_string(),
        ..Default::default()
    };
    let postgres = PostgresBackend::new(postgres_config).await?;

    // Create hybrid backend
    let hybrid = HybridBackend::new(
        Arc::new(redis),
        Arc::new(postgres),
        CachePolicy::LRU,
        WriteStrategy::WriteThrough,
    ).await?;

    // Use hybrid storage (automatically uses both tiers)
    hybrid.put("cached_key", b"cached_value".to_vec()).await?;
    let value = hybrid.get("cached_key").await?;

    Ok(())
}
```

### A.3 Storage Manager

```rust
use llm_optimizer_storage::*;

#[tokio::main]
async fn main() -> Result<()> {
    let manager = StorageManager::builder()
        .add_backend("postgres", postgres_backend)
        .add_backend("redis", redis_backend)
        .add_backend("sled", sled_backend)
        .default_backend("postgres")
        .build()
        .await?;

    // Use manager (automatically routes requests)
    manager.put("key", b"value".to_vec()).await?;

    // Check health of all backends
    let health = manager.health_status().await;
    println!("All backends healthy: {}", health.healthy);

    Ok(())
}
```

---

## Appendix B: Configuration Examples

### B.1 TOML Configuration

```toml
[storage]
default_backend = "postgres"

[storage.postgres]
connection_string = "postgresql://user:pass@localhost:5432/storage"
max_connections = 20
min_connections = 5
connect_timeout_secs = 10
idle_timeout_secs = 600
max_lifetime_secs = 1800
enable_statement_cache = true
statement_cache_capacity = 100

[storage.redis]
url = "redis://localhost:6379"
max_connections = 50
connect_timeout_secs = 5
response_timeout_secs = 10
cluster_enabled = false
sentinel_enabled = false

[storage.sled]
path = "/var/lib/storage/sled"
cache_capacity_bytes = 1073741824  # 1GB
flush_interval_ms = 1000
compression = true
mode = "HighThroughput"

[storage.hybrid]
l1_backend = "redis"
l2_backend = "postgres"
cache_policy = "LRU"
write_strategy = "WriteThrough"
```

### B.2 Environment Variables

```bash
# PostgreSQL
export STORAGE_POSTGRES_URL="postgresql://localhost/storage"
export STORAGE_POSTGRES_MAX_CONNECTIONS=20

# Redis
export STORAGE_REDIS_URL="redis://localhost:6379"
export STORAGE_REDIS_MAX_CONNECTIONS=50

# Sled
export STORAGE_SLED_PATH="/var/lib/storage/sled"
export STORAGE_SLED_CACHE_MB=1024
```

---

## Appendix C: Performance Tuning

### C.1 PostgreSQL Tuning

```sql
-- Connection pooling
ALTER SYSTEM SET max_connections = 200;
ALTER SYSTEM SET shared_buffers = '4GB';

-- Query performance
ALTER SYSTEM SET effective_cache_size = '12GB';
ALTER SYSTEM SET random_page_cost = 1.1;
ALTER SYSTEM SET work_mem = '64MB';

-- Write performance
ALTER SYSTEM SET wal_buffers = '16MB';
ALTER SYSTEM SET checkpoint_completion_target = 0.9;

-- Autovacuum
ALTER SYSTEM SET autovacuum_max_workers = 4;
```

### C.2 Redis Tuning

```redis
# Memory
maxmemory 4gb
maxmemory-policy allkeys-lru

# Persistence
save 900 1
save 300 10
save 60 10000

# Performance
tcp-backlog 511
timeout 300
```

### C.3 Sled Tuning

```rust
let config = SledConfig {
    cache_capacity_bytes: 2 * 1024 * 1024 * 1024, // 2GB
    flush_interval_ms: 500,
    compression: true,
    mode: SledMode::HighThroughput,
};
```

---

## Conclusion

This implementation plan provides a comprehensive roadmap for building an enterprise-grade storage layer for the LLM Auto-Optimizer system. The plan encompasses:

- **Multi-backend support** with PostgreSQL, Redis, and Sled
- **Production-ready features** including connection pooling, health checks, and observability
- **Comprehensive testing** strategy with unit, integration, and performance tests
- **Clear timelines** and milestones for tracking progress
- **Detailed schemas** and migration scripts
- **Risk mitigation** strategies and validation criteria

By following this plan, the storage layer will provide a robust, scalable, and maintainable foundation for the entire LLM Auto-Optimizer system.

**Total Estimated Effort:** 10-12 weeks
**Total Estimated LOC:** 9,500 (implementation) + 4,000 (tests) = 13,500 LOC
**Team Size:** 2-3 engineers

---

**Document Version:** 1.0
**Last Updated:** 2025-11-10
**Status:** Ready for Implementation
**Next Review:** After Phase 1 Completion
