# Event Deduplication System

Enterprise-grade event deduplication implementation for the Stream Processor with Redis backend support, configurable event ID extraction, and comprehensive metrics tracking.

## Overview

The deduplication system provides at-most-once processing semantics by tracking event IDs in Redis with TTL-based automatic cleanup. It's designed for high-throughput distributed streaming applications with zero data loss guarantees.

## Architecture

```
┌─────────────────────────────────────────┐
│        Application Layer                │
│  (Stream Processor, Kafka Consumer)     │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│      EventDeduplicator<T>               │
│  • Type-safe API                        │
│  • Configurable ID extraction           │
│  • Batch operations support             │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│      EventIdExtractor                   │
│  • UUID extraction                      │
│  • Hash-based IDs                       │
│  • Custom extractors                    │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│  RedisDeduplicationBackend              │
│  • Connection pooling                   │
│  • Retry with exponential backoff       │
│  • Batch operations (pipeline)          │
│  • TTL-based cleanup                    │
│  • Metrics tracking                     │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│         Redis Storage                    │
│  • SET NX with TTL                      │
│  • EXISTS checks                        │
│  • Automatic expiration                 │
└─────────────────────────────────────────┘
```

## Features

### Core Capabilities

- **Multiple ID Extraction Strategies**: UUID, hash-based, string, and custom extractors
- **Redis-Backed Storage**: Production-ready distributed deduplication with connection pooling
- **TTL-Based Cleanup**: Automatic memory management with configurable expiration
- **Batch Operations**: Efficient bulk deduplication checks using Redis pipelines
- **Retry Logic**: Exponential backoff for transient failures with configurable limits
- **Comprehensive Metrics**: Track duplicates, cache hits, latency, and error rates
- **Thread-Safe**: Concurrent operations using Arc and async/await
- **Zero Data Loss**: Guaranteed at-most-once processing semantics

### Performance Features

- **Connection Pooling**: Reuse Redis connections for optimal performance
- **Pipeline Support**: Batch operations reduce network round trips
- **Lock-Free Design**: No mutex contention for reads
- **Async-First**: Non-blocking I/O throughout
- **Configurable Batch Size**: Tune for your workload

## Quick Start

```rust
use processor::deduplication::{
    EventDeduplicator, DeduplicationConfig, EventIdExtractor,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MyEvent {
    id: String,
    data: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = DeduplicationConfig::builder()
        .redis_url("redis://localhost:6379")
        .key_prefix("dedup:")
        .default_ttl(Duration::from_secs(3600))
        .build()?;

    let extractor = EventIdExtractor::string(|event: &MyEvent| {
        event.id.clone()
    });

    let dedup = EventDeduplicator::new(config, extractor).await?;

    let event = MyEvent {
        id: "event-123".to_string(),
        data: "payload".to_string(),
    };

    if !dedup.is_duplicate(&event).await? {
        println!("Processing new event");
        dedup.mark_seen(&event).await?;
    }

    Ok(())
}
```

See [README.md](README.md) for complete documentation.
