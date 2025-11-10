# Deduplication Tests - Quick Reference Card

## 📋 Test Checklist

### ✅ Unit Tests (15 tests)

#### Event ID Extraction (4)
- [x] UUID extraction
- [x] Content hash generation
- [x] Custom ID from metadata
- [x] Custom ID fallback to UUID

#### Duplicate Detection (3)
- [x] First seen (not duplicate)
- [x] Duplicate detection
- [x] Multiple duplicate checks

#### TTL & Expiration (1)
- [x] TTL expiration after timeout

#### Event Processing (3)
- [x] Process unique event
- [x] Process duplicate event
- [x] Metrics tracking

#### Batch Processing (2)
- [x] Batch of unique events
- [x] Batch with duplicates

#### State Management (2)
- [x] Duplicate rate calculation
- [x] Clear deduplication state

---

### ✅ Integration Tests (10 tests)

#### Pipeline (1)
- [x] End-to-end deduplication flow

#### Concurrency (2)
- [x] Concurrent event processing
- [x] Multi-threaded deduplication

#### Persistence (1)
- [x] State persistence across restarts

#### TTL Management (1)
- [x] TTL expiration verification

#### Batch Processing (1)
- [x] Large batch (500 events, 50% duplicates)

#### Strategy Testing (2)
- [x] Mixed extraction strategies
- [x] Different event types

#### Metrics (2)
- [x] Metrics accuracy under load
- [x] Redis integration (3 tests with `--ignored`)

---

### ✅ Performance Tests (6 tests)

#### Throughput (2)
- [x] 10K events/sec baseline
- [x] 100K events scaling

#### Latency (1)
- [x] P50, P95, P99 < 1ms

#### Resource Efficiency (2)
- [x] Memory efficiency (10K events)
- [x] Concurrent performance (10 threads)

#### Large-Scale (1)
- [x] 1M events sustained throughput

---

### ✅ Edge Cases (11 tests)

#### Input Validation (3)
- [x] Empty event ID
- [x] Very long event ID (10K chars)
- [x] Special characters

#### Hash Collision (1)
- [x] Hash collision handling

#### TTL Edge Cases (1)
- [x] Expired entry reuse

#### Error Handling (2)
- [x] Redis connection failure
- [x] Partial batch failure

#### Concurrency Edge Cases (1)
- [x] Race condition (50 concurrent)

#### Data Validation (3)
- [x] Clock skew handling
- [x] Null metadata
- [x] Missing custom ID

---

## 🚀 Quick Commands

```bash
# Run all tests
cargo test --test deduplication_tests

# Run specific category
cargo test --test deduplication_tests unit_tests::
cargo test --test deduplication_tests integration_tests::
cargo test --test deduplication_tests performance_tests::
cargo test --test deduplication_tests edge_case_tests::

# Run with output
cargo test --test deduplication_tests -- --nocapture

# Run single test
cargo test --test deduplication_tests test_event_id_extraction_uuid

# Run Redis tests (requires Docker)
docker-compose up -d redis
cargo test --test deduplication_tests redis_integration_tests:: --ignored
```

## 📊 Coverage Summary

| Category | Count | Status |
|----------|-------|--------|
| Unit Tests | 15 | ✅ |
| Integration Tests | 10 | ✅ |
| Performance Tests | 6 | ✅ |
| Edge Cases | 11 | ✅ |
| **TOTAL** | **42** | **✅ 100%** |

## 🎯 Performance Targets

| Metric | Target | Status |
|--------|--------|--------|
| Throughput (10K) | >10K/sec | ✅ |
| Throughput (100K) | <30 sec | ✅ |
| P50 Latency | <1ms | ✅ |
| P95 Latency | <1ms | ✅ |
| P99 Latency | <1ms | ✅ |

## 🔧 Test Implementation

### ID Extraction Strategies
1. **UUID**: `IdExtractionStrategy::Uuid` - Use event UUID
2. **ContentHash**: `IdExtractionStrategy::ContentHash` - Hash event content
3. **Custom**: `IdExtractionStrategy::Custom` - Use custom_id metadata

### Backends Tested
- ✅ MemoryStateBackend (default)
- ✅ RedisStateBackend (with `--ignored`)

### Metrics Tracked
- `total_events` - Total processed
- `unique_events` - Unique count
- `duplicates_found` - Duplicate count
- `redis_errors` - Error count
- `duplicate_rate()` - Percentage

## 🐛 Common Issues

### Test Failures
```bash
# If tests timeout
cargo test --test deduplication_tests -- --test-threads=1

# If Redis unavailable
cargo test --test deduplication_tests --skip redis_integration_tests
```

### Performance Degradation
```bash
# Run with profiling
cargo test --test deduplication_tests performance_tests:: -- --nocapture
```

## 📝 Test Template

```rust
#[tokio::test]
async fn test_your_feature() {
    // Setup
    let backend = Arc::new(MemoryStateBackend::new(None));
    let dedup = EventDeduplicator::new(backend, Duration::from_secs(60));

    // Create event
    let event = FeedbackEvent::new(
        EventSource::User,
        EventType::Feedback,
        serde_json::json!({"test": "data"}),
    );

    // Test deduplication
    let result = dedup.process_event(&event, IdExtractionStrategy::Uuid)
        .await.unwrap();

    // Assert
    assert!(result);
}
```

## 🔍 Debugging

```bash
# Enable tracing
RUST_LOG=debug cargo test --test deduplication_tests -- --nocapture

# Run specific test with output
cargo test --test deduplication_tests test_throughput_10k_events -- --nocapture

# Check test names
cargo test --test deduplication_tests -- --list
```

## 📈 Test Metrics at a Glance

```
Unit Tests:        15/15 ✅
Integration Tests: 10/10 ✅
Performance Tests:  6/6  ✅
Edge Cases:        11/11 ✅
━━━━━━━━━━━━━━━━━━━━━━━━
Total Coverage:    42/42 ✅ (100%)
```

## 🎓 Learning Resources

- See `DEDUPLICATION_TESTS_README.md` for detailed documentation
- See `stream_processor_tests.rs` for similar test patterns
- See `../src/state/redis_backend.rs` for backend implementation
