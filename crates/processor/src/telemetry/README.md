# OpenTelemetry Telemetry Module

Comprehensive OpenTelemetry tracing and metrics for the LLM Auto-Optimizer Stream Processor.

## Features

- **Distributed Tracing**: Full support for distributed tracing with OTLP exporter
- **OTLP Metrics Export**: Export metrics to OTLP collectors with delta or cumulative temporality
- **Context Propagation**: W3C Trace Context and Baggage propagation across service boundaries
- **Resource Attributes**: Automatic detection of service metadata following semantic conventions
- **Multiple Exporters**: Support for gRPC and HTTP OTLP protocols
- **Sampling Strategies**: Configurable sampling (always on/off, trace ID ratio, parent-based)
- **Async Support**: Full async/await compatibility with Tokio runtime

## Quick Start

### Initialize Telemetry

```rust
use processor::telemetry::{TelemetryConfig, init_telemetry};

let config = TelemetryConfig::new("my-processor")
    .with_otlp_endpoint("http://localhost:4317")
    .with_service_version("1.0.0")
    .with_environment("production");

let providers = init_telemetry(config).await?;

// Use telemetry...

// Shutdown when done
providers.shutdown().await?;
```

### Create Spans

```rust
use processor::telemetry::SpanBuilder;
use opentelemetry::trace::SpanKind;

let span = SpanBuilder::new("process_event")
    .with_kind(SpanKind::Internal)
    .with_attribute("event.type", "metric")
    .with_attribute("event.id", "12345")
    .start();

// Use span...
```

### Record Metrics

```rust
use processor::telemetry::{MetricsRecorder, metric_names, metric_attributes};
use opentelemetry::KeyValue;

let recorder = MetricsRecorder::new()
    .with_attribute("service", "processor");

recorder.record_counter(
    "processor",
    metric_names::EVENTS_PROCESSED,
    1,
    vec![KeyValue::new(metric_attributes::EVENT_TYPE, "metric")],
);
```

### Context Propagation

```rust
use processor::telemetry::{inject_into_headers, extract_from_headers};
use opentelemetry::Context;
use std::collections::HashMap;

// Inject context into outgoing request
let mut headers = HashMap::new();
inject_into_headers(&Context::current(), &mut headers);

// Extract context from incoming request
let context = extract_from_headers(&headers);
```

## Module Structure

- **mod.rs**: Main module with re-exports and combined configuration
- **tracing.rs**: Trace provider initialization and span helpers
- **context.rs**: W3C Trace Context and Baggage propagation
- **metrics.rs**: OTLP metrics export configuration
- **resource.rs**: Resource attributes and automatic detection
- **examples.rs**: Usage examples and patterns

## Configuration

### Trace Configuration

```rust
use processor::telemetry::{TraceConfig, OtlpProtocol, SamplingStrategy};

let config = TraceConfig {
    enabled: true,
    service_name: "my-processor".to_string(),
    service_version: "1.0.0".to_string(),
    otlp_endpoint: "http://localhost:4317".to_string(),
    otlp_protocol: OtlpProtocol::Grpc,
    sampling: SamplingStrategy::TraceIdRatio { ratio: 0.5 },
    batch_config: Default::default(),
    headers: Default::default(),
    environment: Some("production".to_string()),
};
```

### Metrics Configuration

```rust
use processor::telemetry::{MetricsConfig, Temporality};

let config = MetricsConfig {
    enabled: true,
    service_name: "my-processor".to_string(),
    service_version: "1.0.0".to_string(),
    otlp_endpoint: "http://localhost:4317".to_string(),
    temporality: Temporality::Cumulative,
    export_config: Default::default(),
    headers: Default::default(),
    environment: Some("production".to_string()),
};
```

## Sampling Strategies

### Always On (100% sampling)

```rust
use processor::telemetry::SamplingStrategy;

let sampling = SamplingStrategy::AlwaysOn;
```

### Trace ID Ratio (sample a percentage)

```rust
let sampling = SamplingStrategy::TraceIdRatio { ratio: 0.1 }; // 10%
```

### Parent-Based (follow parent's decision)

```rust
let sampling = SamplingStrategy::ParentBased {
    root: Box::new(SamplingStrategy::TraceIdRatio { ratio: 0.5 }),
};
```

## Semantic Conventions

The module follows OpenTelemetry semantic conventions for:

### Resource Attributes

- `service.name`: Service name
- `service.version`: Service version
- `service.instance.id`: Unique instance identifier
- `deployment.environment`: Deployment environment
- `service.component`: Component type (e.g., "stream-processor")

### Span Attributes

- `event.type`: Type of event being processed
- `window.type`: Type of window operation
- `aggregation.type`: Type of aggregation
- `state.backend`: State backend type
- `operation.type`: Operation being performed

### Metric Names

- `processor.events.processed`: Count of processed events
- `processor.events.dropped`: Count of dropped events
- `processor.processing.duration`: Processing duration histogram
- `processor.window.triggers`: Window trigger count
- `processor.aggregation.duration`: Aggregation duration
- `processor.state.operation.duration`: State operation duration
- `processor.deduplication.count`: Deduplication count
- `processor.watermark.lag`: Watermark lag gauge

## Integration with Tracing Crate

```rust
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;
use processor::telemetry::otel_global;

let tracer = otel_global::tracer("processor");
let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

let subscriber = Registry::default().with(telemetry);
tracing::subscriber::set_global_default(subscriber)?;

// Now use tracing macros
tracing::info!("Processing event");
tracing::error!("Error occurred");
```

## Testing

### Unit Tests

```bash
cargo test --lib telemetry
```

### Integration Tests (requires OTLP collector)

```bash
# Start OTLP collector
docker run -p 4317:4317 -p 4318:4318 otel/opentelemetry-collector

# Run tests
OTLP_ENDPOINT=http://localhost:4317 cargo test --test telemetry_integration_tests -- --ignored
```

## Examples

See `examples.rs` for comprehensive usage examples including:

- Basic initialization
- Creating nested spans
- Async context propagation
- Cross-service context injection/extraction
- Recording metrics
- Adding span events
- Error handling
- Baggage usage
- Integration with tracing crate

## Best Practices

1. **Initialize Once**: Initialize telemetry providers once at application startup
2. **Use Sampling**: In high-throughput scenarios, use sampling to reduce overhead
3. **Propagate Context**: Always propagate context across async boundaries
4. **Add Meaningful Attributes**: Include relevant attributes to make traces searchable
5. **Record Errors**: Use `record_exception` to capture errors in spans
6. **Batch Configuration**: Tune batch settings for your throughput requirements
7. **Shutdown Gracefully**: Always shutdown providers to flush pending data

## Performance Considerations

- **Sampling**: Use trace ID ratio sampling to reduce overhead in production
- **Batch Size**: Increase batch size for high-throughput scenarios
- **Export Interval**: Adjust export interval based on latency requirements
- **Async Export**: Spans and metrics are exported asynchronously

## Troubleshooting

### Traces Not Appearing

- Verify OTLP endpoint is correct and reachable
- Check sampling strategy (ensure not AlwaysOff)
- Verify batch export is completing (check timeouts)
- Check OTLP collector logs

### High Overhead

- Reduce sampling ratio
- Increase batch size and export interval
- Disable telemetry for low-priority operations

### Context Not Propagating

- Ensure context is attached before creating child spans
- Verify headers are correctly injected/extracted
- Check W3C Trace Context propagator is set

## License

Apache-2.0
