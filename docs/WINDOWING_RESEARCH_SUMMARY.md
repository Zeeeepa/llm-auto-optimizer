# Stream Processing Windowing Research Summary

**Research Date**: 2025-11-09
**Researcher**: LLM DevOps Team
**Project**: LLM Auto-Optimizer Stream Processor

## Research Scope

Comprehensive research into production stream processing windowing patterns from Apache Flink, Kafka Streams, and Spark Streaming, with specific recommendations for LLM metrics processing in the Auto-Optimizer system.

## Key Documents Produced

1. **[STREAM_PROCESSING_WINDOWING_SPECIFICATION.md](./STREAM_PROCESSING_WINDOWING_SPECIFICATION.md)** (37KB)
   - Complete technical specification
   - Industry best practices analysis
   - Detailed implementation designs
   - 7-week implementation roadmap

2. **[WINDOWING_QUICK_REFERENCE.md](./WINDOWING_QUICK_REFERENCE.md)** (12KB)
   - Quick lookup guide
   - Code examples
   - Configuration snippets
   - Troubleshooting guide

## Executive Summary

### Recommended Architecture

**Windowing Strategy**: Hybrid approach using three window types based on event category:

| Event Type | Window | Configuration | Use Case |
|------------|--------|---------------|----------|
| Performance Metrics | Tumbling | 5 min, 2 min lateness | Trend analysis, SLA monitoring |
| User Feedback | Session | 30 min gap, 4 hr max | User journey analysis |
| Anomaly Detection | Sliding | 15 min size, 1 min slide | Real-time degradation detection |

**Watermarking**: Bounded Out-of-Order with per-partition tracking
- Observatory events: 2-minute max delay
- User feedback: 10-minute max delay
- System events: 10-second max delay

**State Management**: 3-tier storage hierarchy
- **Hot** (In-memory): 512MB, 1-hour TTL, current windows
- **Warm** (Sled DB): 1GB cache, 7-day TTL, recent windows
- **Cold** (PostgreSQL): 90-day retention, historical data

**Trigger Policy**: Event-time based with early firing
- Primary: On watermark (when window complete)
- Early: Every 30 seconds for performance metrics
- Late: Up to 2 minutes after watermark

### Memory Budget (4GB total)
- 50% (2GB): Hot state (in-memory windows)
- 30% (1.2GB): Warm state cache (Sled)
- 10% (400MB): Network buffers (Kafka)
- 10% (400MB): Overhead

## Analysis of Current Codebase

### Event Types (from /workspaces/llm-auto-optimizer/crates/types/)

**FeedbackEvent Variants**:
1. `UserFeedbackEvent` - User ratings, task completion, dwell time
2. `PerformanceMetricsEvent` - Latency, tokens, cost, quality scores
3. `ModelResponseEvent` - Request/response metadata
4. `SystemHealthEvent` - Component health, resource usage
5. `ExperimentResultEvent` - A/B test results

**Key Fields for Windowing**:
- `timestamp: DateTime<Utc>` - Event time (primary)
- `request_id: Uuid` - Correlation key
- `session_id: String` - Session grouping key
- `model: String` - Model partitioning key

### Collector Architecture (from /workspaces/llm-auto-optimizer/crates/collector/)

**Current Implementation**:
- Kafka-based event ingestion (rdkafka)
- Batch processing (100 events or 10 seconds)
- Circuit breaker pattern for resilience
- Rate limiting and backpressure handling
- Dead letter queue for failed events

**Integration Points for Windowing**:
1. Consumer: Read from Kafka topics with partition awareness
2. Timestamp: Extract from `FeedbackEvent.timestamp()`
3. Partitioning: By model, session, or request_id
4. Output: Write aggregated metrics to storage layer

### Processor Crate Status

**Current**: Placeholder implementation (add function only)

**Required Components** (from specification):
```
crates/processor/src/
├── windowing/
│   ├── mod.rs
│   ├── window.rs          # Window types and assignment
│   ├── watermark.rs       # Watermark generation
│   └── state.rs           # Window state management
├── state/
│   ├── mod.rs
│   ├── checkpoint.rs      # Checkpointing system
│   └── tiered_store.rs    # Hot/warm/cold storage
├── triggers/
│   └── mod.rs             # Trigger policies
├── memory/
│   └── mod.rs             # Memory management
└── operators/
    ├── mod.rs
    └── aggregate.rs       # Aggregation operators
```

## Industry Best Practices Applied

### From Apache Flink
1. **Separation of Concerns**: Window assignment separate from triggering
2. **Watermark Semantics**: Per-partition watermarks with idle detection
3. **State Backends**: Pluggable storage (memory, RocksDB equivalent)
4. **Checkpointing**: Periodic snapshots for fault tolerance

### From Kafka Streams
1. **Changelog-Based State**: Leverage Kafka for state replication
2. **Grace Period**: Built-in late data handling
3. **Interactive Queries**: Query state from external apps
4. **RocksDB Integration**: Disk-backed state for large windows

### From Spark Streaming
1. **Declarative API**: Simple window operations API
2. **State Timeout**: Automatic cleanup of old state
3. **Watermark Delays**: Configurable per operation

## LLM-Specific Optimizations

### 1. Event Time Characteristics
- **Performance Metrics**: Near real-time (1-2 min delay typical)
- **User Feedback**: Delayed submission common (5-10 min)
- **System Health**: Periodic, minimal delay (<10 sec)

**Solution**: Per-source watermark strategies with different delays

### 2. High Cardinality Keys
- Model IDs: 10-50 active models
- Session IDs: Thousands of concurrent sessions
- Request IDs: 10,000+ requests per second

**Solution**: Session windows with max duration to prevent unbounded growth

### 3. Variable Event Rates
- Peak hours: 10,000 events/sec
- Off-peak: 100 events/sec
- Bursty patterns during deployments

**Solution**: Adaptive pruning based on memory pressure

### 4. Multi-Metric Aggregation
- Latency percentiles (p50, p95, p99)
- Cost aggregation across tokens
- Error rate calculation
- Quality score averaging

**Solution**: Specialized aggregators with incremental computation

## Configuration Recommendations

### Development Environment
```yaml
windowing:
  size: 60s                    # 1-minute windows
  allowed_lateness: 10s
  watermark_delay: 5s

state:
  hot_only: true               # In-memory only
  max_size_mb: 256

checkpointing:
  interval_seconds: 30
```

### Production Environment
```yaml
windowing:
  size: 300s                   # 5-minute windows
  allowed_lateness: 120s
  watermark_delay: 120s
  idle_timeout: 300s

state:
  hot:
    max_size_mb: 2048
  warm:
    backend: sled
    cache_size_mb: 4096
  cold:
    backend: postgres
    retention_days: 90

checkpointing:
  interval_seconds: 60
  format: binary
  storage: s3                  # or local
```

## Implementation Roadmap

### Phase 1: Core Windowing (Weeks 1-2)
- Tumbling, sliding, and session windows
- Basic bounded out-of-order watermarks
- In-memory state store
- Event-time assignment

**Deliverable**: Process performance metrics in 5-minute tumbling windows

### Phase 2: State Management (Weeks 3-4)
- Sled integration for warm state
- Checkpointing to disk
- State TTL and cleanup
- Multi-tier storage

**Deliverable**: Handle 24+ hours of window state

### Phase 3: Advanced Features (Weeks 5-6)
- Trigger policies (early, on-time, late)
- Memory-aware adaptive pruning
- Late data handling with grace period
- Comprehensive metrics

**Deliverable**: Production-ready windowing with monitoring

### Phase 4: Integration (Week 7)
- Kafka consumer with partition awareness
- LLM metric aggregators
- Storage layer output
- End-to-end testing

**Deliverable**: Complete stream processor pipeline

## Dependencies and Technologies

### Rust Crates (from Cargo.toml analysis)
- `tokio` - Async runtime for stream processing
- `tokio-stream` - Stream utilities
- `rdkafka` - Kafka integration
- `sled` - Embedded database for warm state
- `chrono` - Time handling
- `statrs` - Statistical calculations (percentiles)
- `bincode` - State serialization
- `dashmap` - Concurrent hash maps

### External Systems
- **Kafka** - Event streaming (already integrated in collector)
- **PostgreSQL** - Cold state storage (via sqlx)
- **Redis** - Optional hot state replication (via redis crate)
- **S3/Local FS** - Checkpoint storage

## Success Metrics

### Performance Targets
- **Throughput**: 10,000 events/sec sustained
- **Latency**: p99 < 1 second for window emission
- **Memory**: Stable at 4GB under load
- **Recovery**: < 30 seconds from checkpoint

### Correctness Guarantees
- **Exactly-once**: Per window aggregation (via checkpointing)
- **Completeness**: 99.9% of events within watermark processed
- **Ordering**: Event-time ordering within windows
- **Late data**: Configurable handling up to 2 minutes

## Risk Assessment

### Low Risk
- Core windowing logic (well-understood patterns)
- In-memory state (sufficient for MVP)
- Event-time extraction (already in event types)

### Medium Risk
- Sled integration (new to team, but stable library)
- Watermark coordination across partitions
- Memory management under high load

### High Risk
- Exactly-once semantics (requires careful checkpoint design)
- State migration during upgrades
- Large session windows (unbounded memory growth)

**Mitigation**: Start with at-least-once, add idempotency keys, implement max session duration

## Open Questions

1. **Exactly-once vs. At-least-once**: Which semantic for MVP?
   - **Recommendation**: At-least-once for MVP, add exactly-once in Phase 3

2. **State Replication**: Active-passive or multi-master?
   - **Recommendation**: Single-node for MVP, add replication in Phase 4

3. **Backfill Support**: How to reprocess historical data?
   - **Recommendation**: Use event-time windowing, replay from Kafka

4. **Cross-Window Joins**: Join user feedback with performance metrics?
   - **Recommendation**: Defer to Phase 5, use request_id correlation

## References

### Industry Documentation
1. Apache Flink: https://flink.apache.org/
2. Kafka Streams: https://kafka.apache.org/documentation/streams/
3. Google Dataflow Model: "The Dataflow Model" (VLDB 2015)
4. Tyler Akidau's Streaming 101/102: https://www.oreilly.com/radar/the-world-beyond-batch-streaming-101/

### Codebase Analysis
- `/workspaces/llm-auto-optimizer/crates/types/src/events.rs`
- `/workspaces/llm-auto-optimizer/crates/types/src/metrics.rs`
- `/workspaces/llm-auto-optimizer/crates/collector/src/feedback_events.rs`
- `/workspaces/llm-auto-optimizer/crates/collector/src/kafka_client.rs`
- `/workspaces/llm-auto-optimizer/crates/collector/src/feedback_collector.rs`

### Related Documentation
- [ARCHITECTURE.md](./ARCHITECTURE.md)
- [DATA_FLOW_AND_INTERFACES.md](./DATA_FLOW_AND_INTERFACES.md)
- [FEEDBACK_COLLECTOR_ARCHITECTURE.md](./FEEDBACK_COLLECTOR_ARCHITECTURE.md)

## Next Actions

1. **Review**: Team review of windowing specification
2. **Prototype**: Build Phase 1 core windowing (2 weeks)
3. **Test**: Benchmark with synthetic LLM event workload
4. **Iterate**: Adjust configuration based on test results
5. **Integrate**: Connect to collector and storage layers

---

**Document Status**: Complete
**Last Updated**: 2025-11-09
**Next Review**: After Phase 1 implementation
**Owner**: LLM DevOps Team
