# OpenTelemetry Telemetry Guide

Complete guide for implementing and using OpenTelemetry telemetry in the LLM Auto-Optimizer Stream Processor.

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Quick Start](#quick-start)
4. [Configuration](#configuration)
5. [Distributed Tracing](#distributed-tracing)
6. [Metrics](#metrics)
7. [Context Propagation](#context-propagation)
8. [Best Practices](#best-practices)
9. [Testing](#testing)
10. [Troubleshooting](#troubleshooting)

## Overview

The telemetry module provides comprehensive observability for the stream processor using OpenTelemetry. It includes:

- **Distributed Tracing**: Track requests across service boundaries
- **Metrics Export**: Export metrics to OTLP collectors
- **Context Propagation**: W3C Trace Context and Baggage
- **Resource Detection**: Automatic service metadata detection
- **Multiple Exporters**: Support for gRPC and HTTP protocols
- **Async Support**: Full compatibility with Tokio runtime

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Stream Processor                          │
│                                                              │
│  ┌────────────┐    ┌────────────┐    ┌────────────┐       │
│  │   Tracing  │    │  Metrics   │    │  Context   │       │
│  │  Provider  │    │  Provider  │    │Propagation │       │
│  └─────┬──────┘    └─────┬──────┘    └─────┬──────┘       │
│        │                  │                  │              │
│        └──────────────────┼──────────────────┘              │
│                           │                                 │
└───────────────────────────┼─────────────────────────────────┘
                            │
                            ▼
                   ┌────────────────┐
                   │ OTLP Collector │
                   └────────┬───────┘
                            │
              ┌─────────────┴─────────────┐
              │                           │
              ▼                           ▼
       ┌──────────┐              ┌───────────┐
       │  Jaeger  │              │Prometheus │
       │ (Traces) │              │ (Metrics) │
       └──────────┘              └───────────┘
```

## Quick Start

### 1. Add Dependencies

The telemetry module is already integrated into the processor crate. No additional dependencies needed.

### 2. Start OTLP Collector

```bash
# Using Docker Compose
cd crates/processor
docker-compose -f docker-compose.telemetry.yml up -d

# Verify services are running
docker-compose -f docker-compose.telemetry.yml ps
```

### 3. Initialize Telemetry

```rust
use processor::telemetry::{TelemetryConfig, init_telemetry};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize telemetry
    let config = TelemetryConfig::new("my-processor")
        .with_otlp_endpoint("http://localhost:4317")
        .with_service_version("1.0.0")
        .with_environment("production");

    let providers = init_telemetry(config)?;

    // Your application code...

    // Shutdown when done
    providers.shutdown().await?;
    Ok(())
}
```

### 4. Run Demo

```bash
# Run the telemetry demo example
cargo run --example telemetry_demo

# View traces in Jaeger
open http://localhost:16686

# View metrics in Prometheus
open http://localhost:9090
```

## Configuration

### Telemetry Configuration

```rust
use processor::telemetry::{
    TelemetryConfig, OtlpProtocol, SamplingStrategy,
    Temporality, BatchConfiguration, ExportConfig,
};

let config = TelemetryConfig {
    tracing: TraceConfig {
        enabled: true,
        service_name: "my-processor".to_string(),
        service_version: "1.0.0".to_string(),
        otlp_endpoint: "http://localhost:4317".to_string(),
        otlp_protocol: OtlpProtocol::Grpc,
        sampling: SamplingStrategy::ParentBased {
            root: Box::new(SamplingStrategy::TraceIdRatio { ratio: 0.1 }),
        },
        batch_config: BatchConfiguration {
            max_queue_size: 2048,
            max_export_batch_size: 512,
            scheduled_delay: Duration::from_secs(5),
            export_timeout: Duration::from_secs(30),
        },
        headers: HashMap::new(),
        environment: Some("production".to_string()),
    },
    metrics: MetricsConfig {
        enabled: true,
        service_name: "my-processor".to_string(),
        service_version: "1.0.0".to_string(),
        otlp_endpoint: "http://localhost:4317".to_string(),
        temporality: Temporality::Cumulative,
        export_config: ExportConfig {
            export_interval: Duration::from_secs(60),
            export_timeout: Duration::from_secs(30),
        },
        headers: HashMap::new(),
        environment: Some("production".to_string()),
    },
};
```

### Environment Variables

You can also configure via environment variables:

```bash
export OTEL_SERVICE_NAME="my-processor"
export OTEL_EXPORTER_OTLP_ENDPOINT="http://localhost:4317"
export OTEL_EXPORTER_OTLP_PROTOCOL="grpc"
export OTEL_TRACES_SAMPLER="traceidratio"
export OTEL_TRACES_SAMPLER_ARG="0.1"
```

## Distributed Tracing

### Creating Spans

```rust
use processor::telemetry::SpanBuilder;
use processor::{SpanKind, OtelKeyValue};

// Using SpanBuilder
let span = SpanBuilder::new("process_event")
    .with_kind(SpanKind::Internal)
    .with_attribute("event.type", "metric")
    .with_attribute("event.id", "12345")
    .start();

// Or using the global tracer directly
use processor::otel_global;

let tracer = otel_global::tracer("processor");
let span = tracer
    .span_builder("process_event")
    .with_kind(SpanKind::Internal)
    .with_attributes(vec![
        OtelKeyValue::new("event.type", "metric"),
    ])
    .start(&tracer);
```

### Nested Spans

```rust
use processor::telemetry::context_with_span;

let tracer = otel_global::tracer("processor");

// Parent span
let parent = tracer
    .span_builder("parent_operation")
    .with_kind(SpanKind::Server)
    .start(&tracer);

let parent_context = context_with_span(parent);
let _guard = parent_context.attach();

// Child span (automatically linked to parent)
let child = tracer
    .span_builder("child_operation")
    .with_kind(SpanKind::Internal)
    .start(&tracer);
```

### Span Events

```rust
let mut span = tracer
    .span_builder("operation")
    .start(&tracer);

span.add_event("Validation started".to_string(), vec![]);

// ... validation logic ...

span.add_event(
    "Validation complete".to_string(),
    vec![OtelKeyValue::new("result", "success")],
);
```

### Error Recording

```rust
use processor::telemetry::record_exception;

let mut span = tracer
    .span_builder("operation")
    .start(&tracer);

match risky_operation() {
    Ok(result) => { /* handle success */ },
    Err(e) => {
        record_exception(&mut span, &e);
    }
}
```

## Metrics

### Recording Counters

```rust
use processor::telemetry::{MetricsRecorder, metric_names, metric_attributes};
use processor::OtelKeyValue;

let recorder = MetricsRecorder::new()
    .with_attribute("service", "processor");

recorder.record_counter(
    "processor",
    metric_names::EVENTS_PROCESSED,
    1,
    vec![OtelKeyValue::new(metric_attributes::EVENT_TYPE, "metric")],
);
```

### Recording Histograms

```rust
recorder.record_histogram(
    "processor",
    metric_names::PROCESSING_DURATION,
    123.45, // duration in milliseconds
    vec![OtelKeyValue::new(metric_attributes::OPERATION_TYPE, "aggregation")],
);
```

### Recording Gauges

```rust
recorder.record_gauge(
    "processor",
    metric_names::WATERMARK_LAG,
    1000, // lag in milliseconds
    vec![OtelKeyValue::new(metric_attributes::PARTITION_ID, "0")],
);
```

### Available Metrics

#### Counters
- `processor.events.processed`: Count of processed events
- `processor.events.dropped`: Count of dropped events
- `processor.window.triggers`: Window trigger count
- `processor.deduplication.count`: Deduplication count

#### Histograms
- `processor.processing.duration`: Processing duration
- `processor.aggregation.duration`: Aggregation duration
- `processor.state.operation.duration`: State operation duration

#### Gauges
- `processor.watermark.lag`: Watermark lag

## Context Propagation

### Injecting Context

```rust
use processor::telemetry::inject_into_headers;
use std::collections::HashMap;

let context = OtelContext::current();
let mut headers = HashMap::new();

inject_into_headers(&context, &mut headers);

// Send headers with outgoing request
```

### Extracting Context

```rust
use processor::telemetry::extract_from_headers;

// Receive headers from incoming request
let context = extract_from_headers(&headers);
let _guard = context.attach();

// Now create spans that will be linked to the extracted trace
```

### Async Context Propagation

```rust
use processor::telemetry::propagate_context;

let handle = tokio::spawn(propagate_context(async {
    // This task inherits the parent context
    let span = tracer
        .span_builder("async_operation")
        .start(&tracer);
    // ...
}));
```

### Using Baggage

```rust
use processor::telemetry::{with_baggage, get_baggage, BaggageItem};

let context = OtelContext::current();

// Add baggage
let items = vec![
    BaggageItem::new("user-id", "12345"),
    BaggageItem::new("tenant-id", "acme-corp"),
];
let context = with_baggage(&context, items);
let _guard = context.attach();

// Retrieve baggage
if let Some(user_id) = get_baggage(&context, "user-id") {
    println!("User ID: {}", user_id);
}
```

## Best Practices

### 1. Sampling Strategy

Use appropriate sampling in production to reduce overhead:

```rust
// For high-throughput services, sample 10% of traces
let config = TraceConfig {
    sampling: SamplingStrategy::TraceIdRatio { ratio: 0.1 },
    ..Default::default()
};
```

### 2. Span Attributes

Add meaningful attributes to make traces searchable:

```rust
let span = SpanBuilder::new("process_event")
    .with_attribute("event.type", "metric")
    .with_attribute("event.source", "kafka")
    .with_attribute("partition.id", "0")
    .with_attribute("event.size", "1024")
    .start();
```

### 3. Batch Configuration

Tune batch settings for your workload:

```rust
let batch_config = BatchConfiguration {
    max_queue_size: 4096,        // Higher for high throughput
    max_export_batch_size: 1024,  // Batch more for efficiency
    scheduled_delay: Duration::from_secs(10), // Less frequent for lower latency impact
    export_timeout: Duration::from_secs(30),
};
```

### 4. Resource Attributes

Include environment-specific metadata:

```rust
let config = TelemetryConfig::new("processor")
    .with_environment("production")
    .with_service_version(env!("CARGO_PKG_VERSION"));
```

### 5. Error Handling

Always record exceptions in spans:

```rust
match process_event(&event).await {
    Ok(_) => {
        span.add_event("Event processed successfully".to_string(), vec![]);
    }
    Err(e) => {
        record_exception(&mut span, &e);
        return Err(e);
    }
}
```

## Testing

### Unit Tests

```bash
# Run unit tests
cargo test --lib telemetry

# Run specific test
cargo test --lib telemetry::tests::test_span_builder
```

### Integration Tests

```bash
# Start OTLP collector
docker-compose -f docker-compose.telemetry.yml up -d

# Set environment variable
export OTLP_ENDPOINT=http://localhost:4317

# Run integration tests
cargo test --test telemetry_integration_tests -- --ignored

# View results in Jaeger
open http://localhost:16686
```

### Running the Demo

```bash
# Start infrastructure
docker-compose -f docker-compose.telemetry.yml up -d

# Run demo
cargo run --example telemetry_demo

# View traces
open http://localhost:16686

# View metrics
open http://localhost:9090
```

## Troubleshooting

### Traces Not Appearing

**Problem**: Traces are not showing up in Jaeger.

**Solutions**:
1. Check OTLP endpoint is correct: `http://localhost:4317`
2. Verify sampling is not set to `AlwaysOff`
3. Check batch export timeout is sufficient
4. Look at OTLP collector logs: `docker-compose logs otel-collector`
5. Ensure spans are being created with correct context

### High Memory Usage

**Problem**: OpenTelemetry is using too much memory.

**Solutions**:
1. Reduce `max_queue_size` in batch configuration
2. Increase `scheduled_delay` to export more frequently
3. Reduce sampling ratio
4. Check for span leaks (ensure spans are dropped)

### Context Not Propagating

**Problem**: Child spans are not linked to parent spans.

**Solutions**:
1. Ensure context is attached before creating child spans
2. Verify headers are correctly injected/extracted
3. Check W3C Trace Context propagator is set
4. Use `propagate_context` for async tasks

### Metrics Not Updating

**Problem**: Metrics are not showing up in Prometheus.

**Solutions**:
1. Check export interval is appropriate
2. Verify Prometheus is scraping the correct endpoint
3. Look at OTLP collector logs
4. Ensure metric names match expected format

### Connection Refused

**Problem**: Cannot connect to OTLP collector.

**Solutions**:
1. Verify OTLP collector is running: `docker-compose ps`
2. Check endpoint URL (should be `http://localhost:4317` for gRPC)
3. For HTTP, use port `4318`
4. Check firewall settings

## Additional Resources

- [OpenTelemetry Documentation](https://opentelemetry.io/docs/)
- [OpenTelemetry Rust SDK](https://github.com/open-telemetry/opentelemetry-rust)
- [W3C Trace Context](https://www.w3.org/TR/trace-context/)
- [OTLP Specification](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/protocol/otlp.md)
- [Semantic Conventions](https://github.com/open-telemetry/semantic-conventions)

## Support

For issues or questions:
- Check the [troubleshooting section](#troubleshooting)
- Review [examples](examples/telemetry_demo.rs)
- See [module documentation](src/telemetry/README.md)
