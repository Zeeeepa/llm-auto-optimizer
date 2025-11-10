# Event Deduplication - Quick Reference

## Basic Usage

```rust
use processor::deduplication::{EventDeduplicator, DeduplicationConfig, EventIdExtractor};

// 1. Configure
let config = DeduplicationConfig::builder()
    .redis_url("redis://localhost:6379")
    .key_prefix("myapp:")
    .default_ttl(Duration::from_secs(3600))
    .build()?;

// 2. Create extractor
let extractor = EventIdExtractor::string(|event: &MyEvent| event.id.clone());

// 3. Initialize
let dedup = EventDeduplicator::new(config, extractor).await?;

// 4. Use
if !dedup.is_duplicate(&event).await? {
    process_event(&event);
    dedup.mark_seen(&event).await?;
}
```

## Common Patterns

### Pattern 1: Simple Deduplication
```rust
if !dedup.is_duplicate(&event).await? {
    process_event(&event);
    dedup.mark_seen(&event).await?;
}
```

### Pattern 2: Atomic Check-and-Mark
```rust
if !dedup.check_and_mark(&event).await? {
    process_event(&event);
}
```

### Pattern 3: Batch Processing
```rust
let results = dedup.batch_check_duplicates(&events).await?;
let new_events: Vec<_> = events.iter()
    .zip(results.iter())
    .filter_map(|(e, &dup)| if !dup { Some(e) } else { None })
    .collect();

dedup.batch_mark_seen(&new_events).await?;
```

## ID Extractors

### UUID Extractor
```rust
EventIdExtractor::uuid(|event: &Event| event.uuid)
```

### String Extractor
```rust
EventIdExtractor::string(|event: &Event| event.id.clone())
```

### Hash Extractor
```rust
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

EventIdExtractor::hash(|event: &Event| {
    let mut hasher = DefaultHasher::new();
    event.content.hash(&mut hasher);
    hasher.finish()
})
```

### Custom Extractor
```rust
EventIdExtractor::custom(|event: &Event| {
    format!("{}:{}:{}", event.user_id, event.action, event.timestamp)
})
```

## Configuration Options

```rust
DeduplicationConfig::builder()
    .redis_url("redis://localhost:6379")      // Redis URL
    .key_prefix("myapp:")                     // Namespace
    .default_ttl(Duration::from_secs(3600))   // 1 hour TTL
    .connect_timeout(Duration::from_secs(5))  // Connection timeout
    .max_retries(3)                           // Max retries
    .retry_base_delay(Duration::from_millis(100)) // Initial delay
    .retry_max_delay(Duration::from_secs(5))  // Max delay
    .pipeline_batch_size(100)                 // Batch size
    .build()?
```

## Monitoring

### Get Statistics
```rust
let stats = dedup.stats().await;
println!("Duplicates: {}/{} ({:.2}%)",
    stats.duplicates_found,
    stats.events_checked,
    stats.duplicate_rate() * 100.0
);
```

### Health Check
```rust
if !dedup.health_check().await? {
    eprintln!("Redis is down!");
}
```

## Error Handling

```rust
match dedup.is_duplicate(&event).await {
    Ok(is_dup) => { /* handle */ },
    Err(StateError::StorageError { backend_type, details }) => {
        eprintln!("Redis error: {}", details);
        // Fallback logic
    },
    Err(e) => {
        eprintln!("Error: {}", e);
    }
}
```

## Performance Tips

1. **Use batch operations** for high throughput (100K+ ops/sec)
2. **Tune batch size** based on workload (default: 100)
3. **Set appropriate TTL** to manage memory
4. **Monitor stats** to detect issues early
5. **Use check_and_mark** for atomic operations

## Common TTL Values

```rust
// Real-time (1 hour)
.default_ttl(Duration::from_secs(3600))

// Daily batch (24 hours)
.default_ttl(Duration::from_secs(86400))

// Weekly (7 days)
.default_ttl(Duration::from_secs(604800))

// No expiry (use with caution!)
.default_ttl(None)
```

## Testing

```bash
# Run all deduplication tests
cargo test -p processor deduplication

# Run specific test
cargo test -p processor test_basic_deduplication

# Run with output
cargo test -p processor deduplication -- --nocapture
```

## Maintenance

### Clear All State
```rust
// WARNING: Removes all deduplication data!
let deleted = dedup.clear().await?;
println!("Cleared {} entries", deleted);
```

## Integration Examples

### Kafka Consumer
```rust
while let Some(msg) = kafka_source.next().await? {
    let event: MyEvent = serde_json::from_slice(&msg.payload)?;

    if !dedup.check_and_mark(&event).await? {
        process_event(&event);
    }

    kafka_source.commit().await?;
}
```

### Stream Processor
```rust
let processor = StreamProcessorBuilder::new()
    .with_deduplication(dedup)
    .with_window_size(Duration::from_secs(60))
    .build()?;
```
