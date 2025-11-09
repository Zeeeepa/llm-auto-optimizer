# Dead Letter Queue (DLQ) System

## Overview

The Dead Letter Queue (DLQ) system provides a robust mechanism for handling events that fail to publish to Kafka. It ensures zero data loss by preserving failed events for later analysis and replay.

## Features

- **Multiple DLQ Types**: Support for Kafka-based, file-based, and hybrid DLQ implementations
- **Automatic Retry**: Exponential backoff retry mechanism with configurable attempts
- **File Rotation**: Automatic file rotation for file-based DLQ with configurable size limits
- **Background Recovery**: Automatic recovery task that attempts to replay DLQ entries
- **Observability**: Comprehensive metrics and statistics for monitoring
- **Thread-Safe**: Concurrent access support using async primitives
- **Configurable Retention**: Automatic cleanup of old entries

## Architecture

### DLQ Types

#### 1. Kafka DLQ
Routes failed events to a separate Kafka topic (`feedback-events-dlq`).

**Pros:**
- Leverages Kafka's durability and replication
- Natural integration with existing Kafka infrastructure
- Scalable and distributed

**Cons:**
- Requires Kafka to be available (won't work if Kafka is completely down)
- Additional Kafka topic to manage

#### 2. File DLQ
Persists failed events to local rotating log files.

**Pros:**
- Works even when Kafka is completely unavailable
- Simple and reliable
- Easy to inspect and debug

**Cons:**
- Local storage dependency
- File rotation and management overhead
- Limited to single-node recovery

#### 3. Hybrid DLQ (Recommended)
Attempts to publish to Kafka DLQ first, falls back to file storage on failure.

**Pros:**
- Best of both worlds
- Maximum reliability
- Graceful degradation

**Cons:**
- Slightly more complex configuration
- Requires both Kafka and file storage setup

## Configuration

### Basic Configuration

```yaml
dead_letter_queue:
  enabled: true
  type: hybrid  # kafka | file | hybrid

  # Kafka DLQ settings
  kafka_topic: feedback-events-dlq

  # File DLQ settings
  file_path: /var/lib/collector/dlq
  max_file_size_mb: 100
  max_files: 10

  # Retry settings
  retry_interval_seconds: 300  # 5 minutes
  max_retry_attempts: 3

  # Retention
  retention_days: 7

  # Recovery
  enable_recovery: true
```

### Rust Configuration

```rust
use collector::{DLQConfig, DLQType};
use std::path::PathBuf;

let dlq_config = DLQConfig {
    enabled: true,
    dlq_type: DLQType::Hybrid,
    kafka_topic: "feedback-events-dlq".to_string(),
    kafka_config: Some(kafka_config),
    file_path: PathBuf::from("/var/lib/collector/dlq"),
    max_file_size_mb: 100,
    max_files: 10,
    retry_interval_seconds: 300,
    max_retry_attempts: 3,
    retention_days: 7,
    enable_recovery: true,
};
```

## Usage

### Basic Usage with DLQ Manager

```rust
use std::sync::Arc;
use collector::{
    DLQManager, DLQConfig, DLQType,
    FeedbackProducer, KafkaProducerConfig,
    FeedbackEvent, UserFeedbackEvent,
};

// Create Kafka producer
let kafka_config = KafkaProducerConfig::default();
let producer = Arc::new(FeedbackProducer::new(kafka_config)?);

// Create DLQ manager
let dlq_config = DLQConfig::default();
let mut dlq_manager = DLQManager::new(dlq_config, producer)?;

// Start automatic recovery task
dlq_manager.start_recovery_task();

// Add failed event to DLQ
let event = FeedbackEvent::UserFeedback(/* ... */);
dlq_manager.add_failed_event(
    event,
    "Kafka broker unavailable".to_string()
).await?;

// Get statistics
let stats = dlq_manager.stats().await?;
println!("DLQ depth: {}", stats.current_depth);

// Shutdown gracefully
dlq_manager.shutdown().await?;
```

### Integrated Producer with DLQ

```rust
use collector::{
    KafkaProducerWithDLQ,
    DLQManager, FeedbackProducer,
};
use std::sync::Arc;

// Setup producer and DLQ
let producer = Arc::new(FeedbackProducer::new(kafka_config)?);
let dlq_manager = Arc::new(DLQManager::new(dlq_config, producer.clone())?);

// Create integrated producer
let producer_with_dlq = KafkaProducerWithDLQ::new(
    producer,
    Some(dlq_manager)
);

// Publish events - failures automatically go to DLQ
producer_with_dlq.publish_event(&event).await?;

// Publish batch with resilient error handling
let (success, dlq, errors) = producer_with_dlq.publish_batch_resilient(&batch).await;
println!("Success: {}, DLQ: {}, Errors: {}", success, dlq, errors);
```

### Manual DLQ Management

```rust
// List all DLQ entries
let entries = dlq_manager.list_entries(0, 100).await?;
for entry in entries {
    println!("Entry {}: {}", entry.dlq_id, entry.failure_reason);
    println!("  Retry count: {}", entry.retry_count);
    println!("  Next retry: {:?}", entry.next_retry_timestamp);
}

// Manually replay a specific entry
let success = dlq_manager.replay_entry(&entry_id).await?;
if success {
    println!("Entry replayed successfully");
}

// Purge old entries (older than 7 days)
let purged = dlq_manager.purge_old_entries(7).await?;
println!("Purged {} old entries", purged);
```

## DLQ Entry Structure

Each DLQ entry contains:

```rust
pub struct DLQEntry {
    /// Unique DLQ entry ID
    pub dlq_id: Uuid,

    /// Original event that failed
    pub original_event: FeedbackEvent,

    /// Reason for initial failure
    pub failure_reason: String,

    /// Timestamp when first failed
    pub failure_timestamp: DateTime<Utc>,

    /// Number of retry attempts
    pub retry_count: u32,

    /// Last error message
    pub last_error: String,

    /// Last retry timestamp
    pub last_retry_timestamp: Option<DateTime<Utc>>,

    /// Next retry timestamp (for exponential backoff)
    pub next_retry_timestamp: Option<DateTime<Utc>>,
}
```

## Retry Mechanism

### Exponential Backoff

The DLQ uses exponential backoff for retries:

```
Retry 1: base_interval * 2^0 = 300 seconds (5 minutes)
Retry 2: base_interval * 2^1 = 600 seconds (10 minutes)
Retry 3: base_interval * 2^2 = 1200 seconds (20 minutes)
Max: 3600 seconds (1 hour)
```

### Retry Logic

```rust
// Calculate backoff for current retry count
let backoff = entry.calculate_backoff(config.retry_interval_seconds);

// Increment retry count and schedule next retry
entry.increment_retry("Error message".to_string(), backoff);

// Check if entry is ready for retry
if entry.is_ready_for_retry() {
    // Attempt replay
}

// Discard if max retries exceeded
if entry.retry_count >= config.max_retry_attempts {
    // Remove from DLQ
}
```

## File Storage Format

DLQ entries are stored in JSONL (JSON Lines) format:

```jsonl
{"dlq_id":"550e8400-e29b-41d4-a716-446655440000","original_event":{...},"failure_reason":"Kafka timeout",...}
{"dlq_id":"6ba7b810-9dad-11d1-80b4-00c04fd430c8","original_event":{...},"failure_reason":"Network error",...}
```

### File Structure

```
/var/lib/collector/dlq/
├── dlq_current.jsonl          # Current active file
├── dlq_20250109_120000.jsonl  # Rotated file 1
├── dlq_20250109_110000.jsonl  # Rotated file 2
└── dlq_20250109_100000.jsonl  # Rotated file 3
```

## Monitoring and Observability

### DLQ Statistics

```rust
pub struct DLQStats {
    /// Total entries added to DLQ
    pub total_entries: u64,

    /// Currently in DLQ
    pub current_depth: usize,

    /// Total successful recoveries
    pub total_recovered: u64,

    /// Total failed recoveries
    pub total_failed: u64,

    /// Total entries discarded (max retries exceeded)
    pub total_discarded: u64,

    /// File size in bytes (for file-based DLQ)
    pub file_size_bytes: u64,

    /// Number of rotated files
    pub rotated_files: usize,
}
```

### Health Checks

The DLQ statistics can be integrated into your health check endpoints:

```rust
// In health check handler
let dlq_stats = dlq_manager.stats().await?;
let health = ComponentHealth {
    component: "dead_letter_queue".to_string(),
    status: if dlq_stats.current_depth < 1000 {
        HealthStatus::Healthy
    } else {
        HealthStatus::Degraded
    },
    message: format!("DLQ depth: {}", dlq_stats.current_depth),
    metrics: HashMap::from([
        ("dlq_depth".to_string(), dlq_stats.current_depth as f64),
        ("dlq_recovered".to_string(), dlq_stats.total_recovered as f64),
        ("dlq_failed".to_string(), dlq_stats.total_failed as f64),
    ]),
};
```

### Metrics to Monitor

1. **DLQ Depth** (`dlq_depth`): Number of entries currently in DLQ
   - Alert if > 1000 (high backlog)
   - Alert if continuously growing

2. **Recovery Rate** (`dlq_recovered / time`): Successful replays per minute
   - Should be positive when DLQ has entries

3. **Failure Rate** (`dlq_failed / time`): Failed replays per minute
   - Alert if persistently high

4. **Discard Rate** (`dlq_discarded / time`): Entries discarded due to max retries
   - Alert on any discards (potential data loss)

5. **File Size** (`dlq_file_size_bytes`): Size of DLQ files
   - Alert if approaching disk limits

## Error Handling

### Permanent vs Transient Failures

The system distinguishes between permanent and transient failures:

**Permanent Failures** (sent to DLQ):
- Serialization errors (malformed data)
- Configuration errors (bad setup)
- Invalid message errors (rejected by Kafka)
- Message size too large

**Transient Failures** (retried immediately):
- Timeout errors (network issues)
- Connection errors (temporary network failure)
- Broker not available (cluster issues)

```rust
fn is_permanent_failure(&self, error: &KafkaError) -> bool {
    match error {
        KafkaError::SerializationError(_) => true,
        KafkaError::ConfigError(_) => true,
        KafkaError::TimeoutError(_) => false,
        KafkaError::PublishError(msg) => {
            msg.contains("invalid") || msg.contains("malformed")
        }
        // ... more cases
    }
}
```

## Best Practices

### 1. Choose the Right DLQ Type

- **Development**: Use `File` DLQ for simplicity
- **Production**: Use `Hybrid` DLQ for maximum reliability
- **Kafka-only environments**: Use `Kafka` DLQ if file storage is not available

### 2. Configure Appropriate Retention

```yaml
# For high-volume systems
retention_days: 3
max_file_size_mb: 500
max_files: 20

# For low-volume systems
retention_days: 7
max_file_size_mb: 100
max_files: 10
```

### 3. Monitor DLQ Depth

Set up alerts:
```
# Warning: DLQ depth > 100
# Critical: DLQ depth > 1000
# Alert: DLQ depth continuously growing
```

### 4. Regular Maintenance

- Review and analyze discarded entries
- Investigate recurring failures
- Adjust retry configuration based on patterns
- Clean up old DLQ files periodically

### 5. Test Recovery Process

Regularly test the recovery mechanism:
```rust
// Simulate failures and verify recovery
let test_event = create_test_event();
dlq_manager.add_failed_event(test_event, "Test".to_string()).await?;

// Wait for recovery
tokio::time::sleep(Duration::from_secs(recovery_interval + 10)).await;

// Verify recovery
let stats = dlq_manager.stats().await?;
assert!(stats.total_recovered > 0);
```

## Troubleshooting

### High DLQ Depth

**Symptoms**: DLQ depth continuously growing

**Possible Causes**:
1. Kafka cluster is down or unreachable
2. Network connectivity issues
3. Kafka topic configuration issues
4. Serialization problems with events

**Solutions**:
1. Check Kafka cluster health
2. Verify network connectivity
3. Review Kafka logs for errors
4. Examine DLQ entries for patterns

### Entries Not Being Recovered

**Symptoms**: Recovery task running but entries remain in DLQ

**Possible Causes**:
1. Same underlying issue persists
2. Retry interval too short
3. Max retry attempts too low
4. Permanent failures being retried

**Solutions**:
1. Investigate root cause of failures
2. Increase retry interval
3. Increase max retry attempts
4. Check if failures are truly permanent

### DLQ Files Growing Too Large

**Symptoms**: Disk space filling up

**Possible Causes**:
1. File rotation not working
2. Retention not cleaning up old files
3. High failure rate

**Solutions**:
1. Verify file rotation configuration
2. Run manual purge of old entries
3. Reduce retention period
4. Fix underlying issues causing failures

## Security Considerations

### File Permissions

Ensure DLQ directory has appropriate permissions:
```bash
mkdir -p /var/lib/collector/dlq
chown collector:collector /var/lib/collector/dlq
chmod 750 /var/lib/collector/dlq
```

### Sensitive Data

Be aware that DLQ entries contain the full original event:
- Ensure DLQ files are encrypted at rest
- Implement access controls
- Consider data retention policies
- Apply same security measures as primary event storage

## Performance Considerations

### File I/O Optimization

The DLQ uses buffered writes for efficiency:
```rust
let file = OpenOptions::new().append(true).open(path)?;
let mut writer = BufWriter::new(file);  // Buffered I/O
writeln!(writer, "{}", json)?;
writer.flush()?;
```

### Async Operations

All DLQ operations are async to prevent blocking:
- Non-blocking file I/O
- Async Kafka publishing
- Background recovery task

### Batch Processing

Recovery processes entries in batches:
```rust
let entries = dlq.get_retry_ready(100).await?;  // Batch of 100
```

## API Reference

See the [API documentation](../crates/collector/src/dead_letter_queue.rs) for detailed API reference.

## Examples

See the [examples directory](../crates/collector/examples/) for complete working examples:
- `dlq_integration.rs`: Comprehensive integration examples
- `dlq_example.yaml`: Configuration examples

## Testing

Run DLQ tests:
```bash
cargo test --package collector --test dlq_integration_tests
```

Run all collector tests including DLQ:
```bash
cargo test --package collector
```
