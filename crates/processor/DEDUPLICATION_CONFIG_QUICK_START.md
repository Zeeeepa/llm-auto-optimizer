# Deduplication Configuration Quick Start Guide

## Table of Contents

1. [Quick Start](#quick-start)
2. [Configuration Options](#configuration-options)
3. [Builder Methods](#builder-methods)
4. [Common Patterns](#common-patterns)
5. [Examples](#examples)

---

## Quick Start

### Enable Deduplication (Minimal)

```rust
use processor::pipeline::StreamPipelineBuilder;
use std::time::Duration;

let pipeline = StreamPipelineBuilder::new()
    .with_name("my-pipeline")
    .with_deduplication(true, Duration::from_secs(3600))  // Enable with 1 hour TTL
    .build()?;
```

### Enable Deduplication (Full Config)

```rust
use processor::config::{DeduplicationConfig, DeduplicationStrategy};
use processor::pipeline::StreamPipelineBuilder;
use std::time::Duration;

let dedup_config = DeduplicationConfig::new()
    .enabled()
    .with_ttl(Duration::from_secs(3600))
    .with_strategy(DeduplicationStrategy::ContentHash)
    .with_redis_key_prefix("myapp:dedup");

let pipeline = StreamPipelineBuilder::new()
    .with_name("my-pipeline")
    .with_deduplication_config(dedup_config)
    .build()?;
```

---

## Configuration Options

### DeduplicationConfig Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | `bool` | `false` | Enable/disable deduplication |
| `ttl` | `Duration` | `3600s` | Time-to-live for dedup records |
| `strategy` | `DeduplicationStrategy` | `ContentHash` | Deduplication strategy |
| `redis_key_prefix` | `String` | `"dedup"` | Redis key prefix |
| `max_retries` | `u32` | `3` | Max Redis operation retries |
| `cleanup_interval` | `Duration` | `300s` | Cleanup interval for expired records |

### Deduplication Strategies

| Strategy | Description | Use Case |
|----------|-------------|----------|
| `ContentHash` | SHA-256 of full payload | Default, works for any event |
| `EventId` | Event ID field matching | Events with unique IDs |
| `CompositeKey` | source + type + timestamp | Structured events |

---

## Builder Methods

### 1. Simple Enable/Disable

```rust
.with_deduplication(enabled: bool, ttl: Duration)
```

**Example:**
```rust
.with_deduplication(true, Duration::from_secs(3600))
```

### 2. Full Configuration Object

```rust
.with_deduplication_config(config: DeduplicationConfig)
```

**Example:**
```rust
let config = DeduplicationConfig::new()
    .enabled()
    .with_ttl(Duration::from_secs(7200));

.with_deduplication_config(config)
```

### 3. Set TTL Only

```rust
.with_deduplication_ttl(ttl: Duration)
```

**Example:**
```rust
.with_deduplication_ttl(Duration::from_secs(1800))
```

### 4. Set Strategy

```rust
.with_deduplication_strategy(strategy: DeduplicationStrategy)
```

**Example:**
```rust
.with_deduplication_strategy(DeduplicationStrategy::EventId)
```

### 5. Set Redis Prefix

```rust
.with_deduplication_redis_prefix(prefix: impl Into<String>)
```

**Example:**
```rust
.with_deduplication_redis_prefix("production:dedup")
```

---

## Common Patterns

### Pattern 1: Default Settings

```rust
// Disabled by default - no configuration needed
let pipeline = StreamPipelineBuilder::new()
    .with_name("my-pipeline")
    .build()?;
```

### Pattern 2: Simple Enablement

```rust
// Enable with default strategy and custom TTL
let pipeline = StreamPipelineBuilder::new()
    .with_name("my-pipeline")
    .with_deduplication(true, Duration::from_secs(7200))
    .build()?;
```

### Pattern 3: Custom Strategy

```rust
// Enable with specific strategy
let pipeline = StreamPipelineBuilder::new()
    .with_name("my-pipeline")
    .with_deduplication(true, Duration::from_secs(3600))
    .with_deduplication_strategy(DeduplicationStrategy::EventId)
    .build()?;
```

### Pattern 4: Production Configuration

```rust
// Full production configuration
let pipeline = StreamPipelineBuilder::new()
    .with_name("production-pipeline")
    .with_deduplication(true, Duration::from_secs(7200))
    .with_deduplication_strategy(DeduplicationStrategy::ContentHash)
    .with_deduplication_redis_prefix("prod:dedup")
    .with_parallelism(16)
    .with_tumbling_window(60_000)
    .build()?;
```

### Pattern 5: Builder Chain

```rust
// Incremental configuration
let pipeline = StreamPipelineBuilder::new()
    .with_name("my-pipeline")
    .with_deduplication(true, Duration::from_secs(1800))
    .with_deduplication_strategy(DeduplicationStrategy::CompositeKey)
    .with_deduplication_ttl(Duration::from_secs(3600))  // Override TTL
    .build()?;
```

---

## Examples

### Example 1: Basic Event Deduplication

```rust
use processor::pipeline::StreamPipelineBuilder;
use std::time::Duration;

fn create_pipeline() -> anyhow::Result<StreamPipeline> {
    StreamPipelineBuilder::new()
        .with_name("event-processor")
        .with_deduplication(true, Duration::from_secs(3600))
        .build()
}
```

### Example 2: High-Throughput Pipeline

```rust
use processor::pipeline::StreamPipelineBuilder;
use processor::config::DeduplicationStrategy;
use std::time::Duration;

fn create_high_throughput_pipeline() -> anyhow::Result<StreamPipeline> {
    StreamPipelineBuilder::new()
        .with_name("high-throughput")
        .with_parallelism(32)
        .with_buffer_size(100_000)
        .with_deduplication(true, Duration::from_secs(1800))
        .with_deduplication_strategy(DeduplicationStrategy::ContentHash)
        .with_tumbling_window(60_000)
        .build()
}
```

### Example 3: Multi-Environment Configuration

```rust
use processor::config::{DeduplicationConfig, DeduplicationStrategy};
use processor::pipeline::StreamPipelineBuilder;
use std::time::Duration;

fn create_pipeline(env: &str) -> anyhow::Result<StreamPipeline> {
    let dedup_config = match env {
        "production" => DeduplicationConfig::new()
            .enabled()
            .with_ttl(Duration::from_secs(7200))
            .with_strategy(DeduplicationStrategy::ContentHash)
            .with_redis_key_prefix("prod:dedup")
            .with_max_retries(5),

        "staging" => DeduplicationConfig::new()
            .enabled()
            .with_ttl(Duration::from_secs(3600))
            .with_strategy(DeduplicationStrategy::ContentHash)
            .with_redis_key_prefix("staging:dedup"),

        _ => DeduplicationConfig::default(), // Disabled for dev
    };

    StreamPipelineBuilder::new()
        .with_name(format!("{}-pipeline", env))
        .with_deduplication_config(dedup_config)
        .build()
}
```

### Example 4: Configuration from File

```rust
use processor::config::ProcessorConfig;
use std::fs;

fn load_config_from_file(path: &str) -> anyhow::Result<ProcessorConfig> {
    let config_str = fs::read_to_string(path)?;
    let config: ProcessorConfig = serde_json::from_str(&config_str)?;
    config.validate()?;
    Ok(config)
}

// config.json
// {
//   "deduplication": {
//     "enabled": true,
//     "ttl": 3600000,
//     "strategy": "content_hash",
//     "redis_key_prefix": "app:dedup",
//     "max_retries": 3,
//     "cleanup_interval": 300000
//   }
// }
```

### Example 5: Using the Operator Directly

```rust
use processor::pipeline::{DeduplicationOperator, StreamOperator, OperatorContext};
use processor::config::DeduplicationConfig;
use processor::watermark::Watermark;
use std::time::Duration;

async fn process_with_deduplication() -> anyhow::Result<()> {
    let config = DeduplicationConfig::new()
        .enabled()
        .with_ttl(Duration::from_secs(3600));

    let operator = DeduplicationOperator::new("my-dedup", config);
    let ctx = OperatorContext::new(Watermark::min(), 0);

    // Process event
    let event_data = b"event payload".to_vec();
    let results = operator.process(event_data, &ctx).await?;

    if results.is_empty() {
        println!("Event was a duplicate");
    } else {
        println!("Event passed through");
    }

    // Check stats
    let stats = operator.stats();
    println!("Cached signatures: {}", stats.cached_signatures);

    Ok(())
}
```

### Example 6: Testing Deduplication

```rust
#[tokio::test]
async fn test_deduplication() {
    use processor::pipeline::{DeduplicationOperator, StreamOperator, OperatorContext};
    use processor::config::DeduplicationConfig;
    use processor::watermark::Watermark;
    use std::time::Duration;

    let config = DeduplicationConfig::new()
        .enabled()
        .with_ttl(Duration::from_secs(60));

    let operator = DeduplicationOperator::new("test", config);
    let ctx = OperatorContext::new(Watermark::min(), 0);

    let event = b"test event".to_vec();

    // First occurrence - should pass
    let result1 = operator.process(event.clone(), &ctx).await.unwrap();
    assert_eq!(result1.len(), 1);

    // Duplicate - should be filtered
    let result2 = operator.process(event.clone(), &ctx).await.unwrap();
    assert_eq!(result2.len(), 0);

    // Different event - should pass
    let other_event = b"different event".to_vec();
    let result3 = operator.process(other_event, &ctx).await.unwrap();
    assert_eq!(result3.len(), 1);
}
```

---

## Configuration Validation

### Valid Configuration

```rust
let config = DeduplicationConfig::new()
    .enabled()
    .with_ttl(Duration::from_secs(3600))
    .with_redis_key_prefix("myapp:dedup");

assert!(config.validate().is_ok());
```

### Invalid Configurations

```rust
// Invalid: Zero TTL
let mut config = DeduplicationConfig::new().enabled();
config.ttl = Duration::from_secs(0);
assert!(config.validate().is_err());

// Invalid: Empty Redis prefix
let mut config = DeduplicationConfig::new().enabled();
config.redis_key_prefix = String::new();
assert!(config.validate().is_err());

// Invalid: Zero cleanup interval
let mut config = DeduplicationConfig::new().enabled();
config.cleanup_interval = Duration::from_secs(0);
assert!(config.validate().is_err());
```

---

## Best Practices

### 1. TTL Selection

- **Short TTL (5-15 minutes)**: High-frequency events, limited memory
- **Medium TTL (1-6 hours)**: Standard use cases
- **Long TTL (24+ hours)**: Critical duplicate prevention

### 2. Strategy Selection

- **ContentHash**: Default choice, works for all events
- **EventId**: When events have reliable unique IDs
- **CompositeKey**: For structured events with predictable fields

### 3. Redis Configuration

- Use environment-specific prefixes: `{env}:dedup`
- Enable in production, optional in dev/test
- Monitor Redis memory usage

### 4. Performance

- Deduplication adds 1-2 μs per event
- Cache cleanup happens automatically
- Consider disabling for low-duplicate scenarios

### 5. Testing

- Test with and without deduplication
- Verify TTL expiration behavior
- Test different event types

---

## Troubleshooting

### Issue: High Memory Usage

**Solution**: Reduce TTL or increase cleanup frequency
```rust
.with_deduplication_ttl(Duration::from_secs(1800))
.with_cleanup_interval(Duration::from_secs(60))
```

### Issue: Duplicates Not Detected

**Solution**: Verify deduplication is enabled and TTL is appropriate
```rust
let stats = operator.stats();
assert!(stats.enabled);
```

### Issue: Too Many Events Filtered

**Solution**: Check if TTL is too long or strategy is incorrect
```rust
.with_deduplication_ttl(Duration::from_secs(300))  // Shorter TTL
.with_deduplication_strategy(DeduplicationStrategy::EventId)  // Different strategy
```

---

## Additional Resources

- [Full Integration Summary](./DEDUPLICATION_INTEGRATION_SUMMARY.md)
- [Deduplication Module Documentation](./src/deduplication/README.md)
- [Configuration API Documentation](./src/config.rs)
- [Pipeline Builder Documentation](./src/pipeline/builder.rs)

---

## Quick Reference Card

```rust
// ┌─────────────────────────────────────────────────────────────┐
// │              DEDUPLICATION QUICK REFERENCE                  │
// ├─────────────────────────────────────────────────────────────┤
// │ Enable:     .with_deduplication(true, Duration::from_secs(3600)) │
// │ Strategy:   .with_deduplication_strategy(ContentHash)      │
// │ TTL:        .with_deduplication_ttl(Duration::from_secs(N))│
// │ Prefix:     .with_deduplication_redis_prefix("app:dedup")  │
// │                                                              │
// │ Strategies: ContentHash | EventId | CompositeKey            │
// │ Default:    Disabled, 1h TTL, ContentHash                   │
// └─────────────────────────────────────────────────────────────┘
```
