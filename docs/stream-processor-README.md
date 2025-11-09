# Stream Processor Documentation

Complete documentation for the LLM Auto-Optimizer Stream Processor component.

## Overview

The Stream Processor is an enterprise-grade event processing engine that transforms raw feedback events from Kafka into aggregated metrics for the LLM Auto-Optimizer. It provides real-time windowing, aggregation, and state management capabilities.

### Key Capabilities

- **Multi-Window Support**: Tumbling, sliding, and session windows
- **Rich Aggregations**: Count, sum, average, min, max, percentiles (p50, p95, p99), standard deviation
- **Watermarking**: Handle late events with configurable allowed lateness
- **State Persistence**: RocksDB-backed state with checkpoint/recovery
- **High Performance**: 10,000+ events/sec throughput, <100ms p99 latency
- **Fault Tolerance**: Automatic recovery from failures

## Documentation Index

### 1. Architecture Design
**File**: [stream-processor-architecture.md](./stream-processor-architecture.md)

Comprehensive technical design covering:
- System architecture and components
- Module structure and file organization
- Core data structures (Window, Aggregator, State)
- Processing pipeline flow
- Watermarking and late event handling
- State management and persistence
- Error handling strategy
- Configuration structure
- API design

**When to read**: Start here for a complete understanding of the system architecture.

### 2. Implementation Plan
**File**: [stream-processor-implementation-plan.md](./stream-processor-implementation-plan.md)

Detailed 11-week implementation roadmap:
- Phase-by-phase development plan
- Task breakdown with checklists
- Dependencies and prerequisites
- Success metrics and KPIs
- Risk mitigation strategies
- Deployment strategy
- Monitoring and alerting setup

**When to read**: Use this as a project management guide for implementation.

### 3. API Reference
**File**: [stream-processor-api-reference.md](./stream-processor-api-reference.md)

Quick reference guide with code examples:
- Quick start examples
- Window type usage
- Aggregation function APIs
- Watermarking strategies
- State management operations
- Kafka integration
- Pipeline building
- Error handling
- Configuration examples
- Performance tuning
- Troubleshooting guide

**When to read**: Reference this while coding or integrating with the stream processor.

### 4. Visual Diagrams
**File**: [stream-processor-diagrams.md](./stream-processor-diagrams.md)

Visual representations of:
- System context and integration
- Processing pipeline flow
- Window type visualizations
- Watermark and late event handling
- State management architecture
- Checkpoint and recovery flow
- Aggregation engine details
- Parallelism and partitioning
- Error handling decision tree
- Deployment architecture
- Metrics and monitoring

**When to read**: Use for visual understanding and presentations.

## Quick Start

### 1. Read the Architecture
Start with the [Architecture Design](./stream-processor-architecture.md) to understand:
- How events flow through the system
- What data structures are used
- How windows and aggregations work

### 2. Review the Diagrams
Check the [Visual Diagrams](./stream-processor-diagrams.md) to see:
- How components interact
- How data flows through the pipeline
- How windows work visually

### 3. Explore the API
Look at the [API Reference](./stream-processor-api-reference.md) for:
- Code examples
- Configuration options
- Common usage patterns

### 4. Plan Implementation
Use the [Implementation Plan](./stream-processor-implementation-plan.md) to:
- Understand development phases
- Track progress
- Identify dependencies

## Key Concepts

### Windows
Time-based groupings of events for aggregation:

- **Tumbling**: Fixed-size, non-overlapping (e.g., hourly aggregations)
- **Sliding**: Fixed-size, overlapping (e.g., moving averages)
- **Session**: Dynamic, gap-based (e.g., user sessions)

### Aggregations
Statistical functions computed over windows:

- **Basic**: Count, sum, average, min, max
- **Advanced**: Percentiles (p50, p95, p99), standard deviation

### Watermarks
Mechanism for handling out-of-order events:

- Track progress in event time
- Allow late events within configured lateness
- Trigger window computations

### State
Persistent storage of window accumulators:

- In-memory (development)
- RocksDB (production)
- Checkpointing for fault tolerance

## Architecture Summary

```
Events (Kafka)
    │
    ▼
┌───────────────────────────────────────┐
│       Stream Processor                │
│                                       │
│  Watermark → Windows → Aggregation   │
│                  ↕                    │
│              State (RocksDB)          │
└───────────────────────────────────────┘
    │
    ▼
Aggregated Metrics
    │
    ├─→ Analyzer Engine
    ├─→ Decision Engine
    └─→ Dashboard
```

## File Structure

The processor is organized into these modules:

```
crates/processor/src/
├── core/           # Event wrappers and core types
├── window/         # Window assignment and triggers
├── aggregation/    # Aggregation functions
├── watermark/      # Watermark generation
├── state/          # State persistence
├── pipeline/       # Processing pipeline
├── kafka/          # Kafka integration
├── metrics/        # Observability
├── error.rs        # Error types
└── config.rs       # Configuration
```

## Configuration Example

```yaml
processor:
  parallelism: 4
  buffer_size: 10000
  checkpoint_interval_secs: 60

  windows:
    default_type: "tumbling"
    tumbling:
      size_secs: 300          # 5 minutes
      allowed_lateness_secs: 60

  watermark:
    strategy: "bounded_out_of_orderness"
    max_out_of_orderness_secs: 30

  aggregation:
    enabled: ["count", "sum", "average", "min", "max", "percentile", "stddev"]
    percentiles: [50, 95, 99]

  kafka:
    consumer:
      bootstrap_servers: "localhost:9092"
      group_id: "processor-group"
      topics: ["feedback-events"]

    producer:
      result_topic: "aggregated-metrics"
```

## Usage Example

```rust
use llm_optimizer_processor::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create pipeline
    let pipeline = create_pipeline::<FeedbackEvent>()
        .with_kafka_source(kafka_config)
        .with_watermark_generator(
            BoundedOutOfOrdernessWatermark::new(Duration::from_secs(30))
        )
        .with_tumbling_window(Duration::from_secs(300))
        .with_state_backend(state_backend)
        .with_sink(kafka_sink)
        .build()?;

    // Run
    pipeline.run().await?;
    Ok(())
}
```

## Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Throughput | 10,000+ events/sec | Per processor instance |
| Latency (p99) | <100ms | End-to-end processing |
| State Recovery | <30 seconds | From checkpoint |
| Memory Usage | <2GB | For 100k active windows |
| Availability | 99.9% | With proper deployment |

## Development Phases

1. **Phase 1-2** (Weeks 1-3): Core foundation and window management
2. **Phase 3-4** (Weeks 3-5): Watermarking and aggregation engine
3. **Phase 5-6** (Weeks 5-7): State management and Kafka integration
4. **Phase 7-8** (Weeks 7-8): Pipeline and observability
5. **Phase 9-10** (Weeks 9-11): Testing and production hardening

**Total**: 11 weeks (2.5 months)

## Testing Strategy

### Unit Tests
- Core types and utilities
- Window assignment logic
- Aggregation functions
- Watermark generation

### Integration Tests
- End-to-end processing
- Late event handling
- State recovery
- Multi-window scenarios

### Performance Tests
- Throughput benchmarks
- Latency benchmarks
- Memory profiling
- State backend performance

## Deployment

### Development
```bash
cargo run --bin processor -- --config config/processor-dev.yaml
```

### Production (Kubernetes)
```bash
kubectl apply -f k8s/processor-deployment.yaml
kubectl apply -f k8s/processor-service.yaml
kubectl apply -f k8s/processor-pvc.yaml
```

## Monitoring

### Key Metrics
- Events processed per second
- Processing latency (p50, p95, p99)
- Kafka consumer lag
- State size (MB)
- Checkpoint duration
- Error rate

### Dashboards
- Real-time processing metrics
- Window statistics
- State size trends
- Error tracking

### Alerts
- Consumer lag > 10,000 messages
- p99 latency > 1 second
- Error rate > 1%
- State size > 10GB

## Troubleshooting

### High Consumer Lag
- Increase `parallelism` in configuration
- Increase `max_poll_records` for Kafka consumer
- Scale horizontally (add more instances)

### State Size Growth
- Reduce `allowed_lateness_secs`
- More frequent checkpoints
- Review window sizes

### Late Events Dropped
- Increase `allowed_lateness_secs`
- Increase `max_out_of_orderness_secs`
- Review event timestamp accuracy

## Contributing

See the [Implementation Plan](./stream-processor-implementation-plan.md) for:
- Current development status
- Open tasks and issues
- How to contribute

## References

### Internal Documentation
- [Architecture Design](./stream-processor-architecture.md) - Complete technical design
- [Implementation Plan](./stream-processor-implementation-plan.md) - Development roadmap
- [API Reference](./stream-processor-api-reference.md) - Code examples and usage
- [Visual Diagrams](./stream-processor-diagrams.md) - Architecture diagrams

### External Resources
- [Apache Kafka](https://kafka.apache.org/documentation/)
- [RocksDB](https://github.com/facebook/rocksdb/wiki)
- [Apache Flink Concepts](https://nightlies.apache.org/flink/flink-docs-master/docs/concepts/overview/) - Similar streaming concepts
- [Dataflow Model](https://research.google/pubs/pub43864/) - Streaming systems theory

## FAQ

**Q: What's the difference between tumbling and sliding windows?**
A: Tumbling windows are non-overlapping (each event belongs to one window), while sliding windows overlap (each event can belong to multiple windows). Use tumbling for simple periodic aggregations, sliding for moving averages.

**Q: How does watermarking work?**
A: Watermarks track progress in event time. They're calculated as `max_event_time - max_out_of_orderness`. Events arriving before the watermark minus allowed lateness are dropped.

**Q: Can I use multiple aggregation functions?**
A: Yes! The `AggregationState` computes all enabled aggregations simultaneously. Configure which ones to enable in the config file.

**Q: How do I scale the processor?**
A: Scale horizontally by adding more processor instances. Each instance consumes from different Kafka partitions, so parallelism is limited by the number of partitions.

**Q: What happens if the processor crashes?**
A: The processor automatically recovers from the last checkpoint. It restores state from RocksDB and resumes Kafka consumption from the checkpointed offsets.

**Q: How do I monitor the processor?**
A: The processor exposes Prometheus metrics at `/metrics`. Use Grafana dashboards to visualize metrics and set up alerts in Alertmanager.

## Support

For questions or issues:
1. Check this documentation
2. Review the [API Reference](./stream-processor-api-reference.md)
3. Check the [Troubleshooting](#troubleshooting) section
4. Open an issue in the repository

---

**Next Steps**: Start with the [Architecture Design](./stream-processor-architecture.md) to understand the complete system.
