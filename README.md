<div align="center">

# LLM Auto Optimizer

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Status](https://img.shields.io/badge/status-alpha-yellow.svg)](https://github.com/globalbusinessadvisors/llm-auto-optimizer)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](CONTRIBUTING.md)

**Automatically optimize your LLM infrastructure with intelligent, real-time feedback loops**

[Features](#features) •
[Quick Start](#quick-start) •
[Architecture](#architecture) •
[Documentation](#documentation) •
[Contributing](#contributing)

</div>

---

## Overview

The **LLM Auto Optimizer** is a continuous feedback-loop agent that automatically adjusts model selection, prompt templates, and configuration parameters based on real-time performance, drift, latency, and cost data. Built with Rust for maximum performance and reliability.

### Why LLM Auto Optimizer?

- 💰 **Reduce LLM costs by 30-60%** through intelligent model selection and prompt optimization
- ⚡ **Sub-5-minute optimization cycles** for rapid adaptation to changing conditions
- 🎯 **Multi-objective optimization** balancing quality, cost, and latency
- 🛡️ **Production-grade reliability** with 99.9% availability target
- 🚀 **Progressive canary deployments** with automatic rollback on degradation
- 🔒 **Enterprise-ready** with comprehensive audit logging and compliance

---

## Features

### Core Capabilities

| Feature | Description | Status |
|---------|-------------|--------|
| **Feedback Collection** | OpenTelemetry + Kafka integration with circuit breaker, DLQ, rate limiting | ✅ Complete |
| **Stream Processing** | Windowing (tumbling, sliding, session), aggregation, watermarking | ✅ Complete |
| **Distributed State** | Redis/PostgreSQL backends with distributed locking, 3-tier caching | ✅ Complete |
| **Analyzer Engine** | 5 analyzers: Performance, Cost, Quality, Pattern, Anomaly detection | ✅ Complete |
| **Decision Engine** | 5 strategies: Model Selection, Caching, Rate Limiting, Batching, Prompt Optimization | ✅ Complete |
| **Canary Deployments** | Progressive rollouts with automatic rollback and health monitoring | ✅ Complete |
| **Storage Layer** | Multi-backend storage (PostgreSQL, Redis, Sled) with unified interface | ✅ Complete |
| **REST & gRPC APIs** | Full API coverage for integration | 📋 Planned |

### Optimization Strategies

<details>
<summary><b>1. A/B Prompt Testing</b></summary>

Test multiple prompt variations with statistical significance testing (p < 0.05) to identify the most effective prompts.

```rust
// Example: Test two prompt variations
let experiment = ExperimentBuilder::new()
    .name("greeting_test")
    .variant("control", "Hello, how can I help?")
    .variant("treatment", "Hi there! What can I assist you with today?")
    .metric("user_satisfaction")
    .significance_level(0.05)
    .build();
```
</details>

<details>
<summary><b>2. Reinforcement Feedback</b></summary>

Learn from user feedback using contextual bandits and Thompson Sampling to continuously improve model selection.
</details>

<details>
<summary><b>3. Cost-Performance Scoring</b></summary>

Multi-objective Pareto optimization balancing quality, cost, and latency to find the optimal configuration.
</details>

<details>
<summary><b>4. Adaptive Parameter Tuning</b></summary>

Dynamically adjust temperature, top-p, max tokens based on task characteristics and historical performance.
</details>

<details>
<summary><b>5. Threshold-Based Heuristics</b></summary>

Detect performance degradation, drift, and anomalies with automatic response and alerting.
</details>

---

## Quick Start

### Prerequisites

- **Rust 1.75+** - [Install via rustup](https://rustup.rs/)
- **PostgreSQL 15+** or SQLite for development
- **Docker & Docker Compose** (optional, recommended)

### Installation

```bash
# Clone the repository
git clone https://github.com/globalbusinessadvisors/llm-auto-optimizer.git
cd llm-auto-optimizer

# Build the project
cargo build --release

# Run tests
cargo test --all

# Run benchmarks
cargo bench
```

### Docker Setup (Recommended)

```bash
# Start all services (PostgreSQL, Redis, Kafka)
docker-compose up -d

# View logs
docker-compose logs -f optimizer

# Stop services
docker-compose down
```

### Configuration

```bash
# Copy example configuration
cp config.example.yaml config.yaml

# Edit configuration to match your environment
# Set environment variables for sensitive values
export DATABASE_URL="postgresql://user:pass@localhost/optimizer"
export KAFKA_BROKERS="localhost:9092"
```

### Basic Usage

```rust
use llm_auto_optimizer::{Optimizer, Config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = Config::from_file("config.yaml")?;

    // Initialize optimizer
    let optimizer = Optimizer::new(config).await?;

    // Start optimization loop
    optimizer.run().await?;

    Ok(())
}
```

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     LLM-Auto-Optimizer                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐     │
│  │   Feedback   │───▶│   Stream     │───▶│   Analyzer   │     │
│  │  Collector   │    │  Processor   │    │   Engine     │     │
│  │              │    │              │    │              │     │
│  │ • OpenTelemetry  │ • Windowing   │    │ • Performance│     │
│  │ • Kafka      │    │ • Aggregation│    │ • Cost       │     │
│  │ • Circuit    │    │ • Watermarks │    │ • Drift      │     │
│  │   Breaker    │    │              │    │ • Anomaly    │     │
│  └──────────────┘    └──────────────┘    └──────────────┘     │
│         │                                        │              │
│         │                                        ▼              │
│         │                                 ┌──────────────┐     │
│         │                                 │   Decision   │     │
│         │                                 │    Engine    │     │
│         │                                 │              │     │
│         │                                 │ • A/B Testing│     │
│         │                                 │ • RL Feedback│     │
│         │                                 │ • Pareto Opt │     │
│         │                                 └──────────────┘     │
│         │                                        │              │
│         │                                        ▼              │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐     │
│  │   Storage    │◀───│ Configuration│◀───│   Actuator   │     │
│  │    Layer     │    │   Updater    │    │   Engine     │     │
│  │              │    │              │    │              │     │
│  │ • PostgreSQL │    │ • Versioning │    │ • Canary     │     │
│  │ • Redis      │    │ • Rollback   │    │ • Rollout    │     │
│  │ • Sled       │    │ • Audit Log  │    │ • Health     │     │
│  └──────────────┘    └──────────────┘    └──────────────┘     │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Component Overview

| Component | Responsibility | Key Technologies | Status |
|-----------|----------------|------------------|--------|
| **Collector** | Gather feedback from LLM services | OpenTelemetry, Kafka, Circuit Breaker | ✅ |
| **Processor** | Stream processing and aggregation | Windowing, Watermarks, State Management | ✅ |
| **Analyzer** | Detect patterns and anomalies | 5 analyzers with statistical analysis | ✅ |
| **Decision** | Determine optimal configurations | 5 optimization strategies, Pareto optimization | ✅ |
| **Actuator** | Deploy configuration changes | Canary rollouts, Automatic rollback | ✅ |
| **Storage** | Persist state and history | PostgreSQL, Redis, Sled with unified interface | ✅ |

---

## Project Structure

```
llm-auto-optimizer/
├── crates/
│   ├── types/          # Core data models and types ✅
│   ├── config/         # Configuration management ✅
│   ├── collector/      # Feedback collection (OpenTelemetry, Kafka) ✅
│   ├── processor/      # Stream processing and aggregation ✅
│   │   ├── analyzer/   # 5 analyzers: Performance, Cost, Quality, Pattern, Anomaly ✅
│   │   ├── decision/   # 5 optimization strategies with decision coordinator ✅
│   │   ├── actuator/   # Canary deployments with rollback engine ✅
│   │   └── storage/    # Multi-backend storage layer (PostgreSQL, Redis, Sled) ✅
│   ├── integrations/   # External service integrations 📋
│   ├── api/            # REST/gRPC API 📋
│   └── cli/            # Command-line interface 📋
├── migrations/         # Database migrations
├── config/             # Example configurations
├── docker/             # Docker deployment files
├── docs/               # Documentation
├── monitoring/         # Grafana/Prometheus configs
└── tests/              # Integration tests ✅
```

**Legend:** ✅ Complete | 🚧 In Progress | 📋 Planned

---

## Deployment Modes

### 1. Sidecar Pattern
Deploy alongside your LLM service for ultra-low latency (<1ms) optimization decisions.

```yaml
# Kubernetes sidecar example
containers:
  - name: llm-service
    image: your-llm-service:latest
  - name: optimizer
    image: llm-auto-optimizer:latest
    env:
      - name: MODE
        value: "sidecar"
```

### 2. Standalone Microservice
Centralized optimization for multiple services with high availability.

```bash
# Run as standalone service
docker run -d \
  -p 8080:8080 \
  -e MODE=standalone \
  llm-auto-optimizer:latest
```

### 3. Background Daemon
Cost-optimized batch processing for large-scale deployments.

```bash
# Run as background daemon
cargo run --release -- daemon \
  --batch-size 1000 \
  --interval 5m
```

---

## Performance Targets

| Metric | Target | Current Status |
|--------|--------|----------------|
| **Cost Reduction** | 30-60% | 🎯 On Track |
| **Optimization Cycle** | <5 minutes (p95) | 🎯 On Track |
| **Decision Latency** | <1 second (p99) | 🎯 On Track |
| **Availability** | 99.9% | 🎯 On Track |
| **Event Ingestion** | 10,000/sec | 🎯 On Track |
| **Memory Usage** | <500MB (idle) | 🎯 On Track |

---

## Documentation

### Core Documentation
- 📘 [Architecture Guide](docs/ARCHITECTURE.md)
- 🚀 [Quick Start Guide](docs/DEPLOYMENT_GUIDE.md)
- 📊 [Stream Processing](docs/stream-processor-README.md)
- 🔧 [Configuration Reference](config.example.yaml)
- 🗺️ [Project Roadmap](plans/ROADMAP.md)
- 📈 [Performance Benchmarks](docs/BUILD_SUCCESS.md)

### Component Documentation (✅ Complete)
- 🔍 [Analyzer Engine](docs/ANALYZER_ENGINE_IMPLEMENTATION_COMPLETE.md) - 5 analyzers, 6,458 LOC, 49 tests
- 🧠 [Decision Engine](docs/DECISION_ENGINE_IMPLEMENTATION_COMPLETE.md) - 5 strategies, 8,930 LOC, 88 tests
- 🚀 [Actuator](docs/ACTUATOR_IMPLEMENTATION_COMPLETE.md) - Canary deployments, 5,853 LOC, 61 tests
- 💾 [Storage Layer](docs/STORAGE_LAYER_IMPLEMENTATION_COMPLETE.md) - 3 backends, 8,718 LOC, 83 tests

---

## Development

### Building from Source

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run specific crate tests
cargo test -p collector
cargo test -p processor

# Run with logging
RUST_LOG=debug cargo run
```

### Running Tests

```bash
# Run all tests
cargo test --all

# Run integration tests
cargo test --test '*'

# Run with coverage
cargo tarpaulin --out Html
```

### Benchmarking

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench kafka_sink_benchmark
```

---

## Contributing

We welcome contributions! Here's how you can help:

1. 🐛 **Report bugs** - Open an issue with details and reproduction steps
2. 💡 **Suggest features** - Share your ideas for improvements
3. 📝 **Improve documentation** - Help us make docs clearer
4. 🔧 **Submit PRs** - Fix bugs or add features

Please read our [Contributing Guidelines](CONTRIBUTING.md) before submitting PRs.

### Development Setup

```bash
# Fork and clone the repository
git clone https://github.com/YOUR_USERNAME/llm-auto-optimizer.git
cd llm-auto-optimizer

# Create a feature branch
git checkout -b feature/your-feature-name

# Make your changes and test
cargo test --all
cargo clippy -- -D warnings
cargo fmt --check

# Commit and push
git commit -m "Add your feature"
git push origin feature/your-feature-name
```

---

## Roadmap

### Phase 1: MVP Foundation ✅
- [x] Core type system and configuration
- [x] Feedback collector with Kafka integration
- [x] Stream processor with windowing
- [x] Distributed state management

### Phase 2: Intelligence Layer ✅
- [x] Analyzer engine (5 analyzers: Performance, Cost, Quality, Pattern, Anomaly)
- [x] Decision engine (5 optimization strategies)
- [x] Statistical significance testing for A/B testing
- [x] Multi-objective Pareto optimization

### Phase 3: Deployment & Storage ✅
- [x] Actuator engine with canary deployments
- [x] Rollback engine with automatic health monitoring
- [x] Storage layer with PostgreSQL, Redis, and Sled backends
- [x] Configuration management with versioning and audit logs

### Phase 4: Production Readiness 📋
- [ ] REST & gRPC APIs
- [ ] Integration with LLM DevOps ecosystem
- [ ] Comprehensive monitoring dashboards
- [ ] CLI tool for administration

See the full [Roadmap](plans/ROADMAP.md) for detailed milestones.

---

## Community & Support

- 💬 **Discussions** - [GitHub Discussions](https://github.com/globalbusinessadvisors/llm-auto-optimizer/discussions)
- 🐛 **Bug Reports** - [GitHub Issues](https://github.com/globalbusinessadvisors/llm-auto-optimizer/issues)
- 📧 **Email** - Contact the maintainers
- 📖 **Documentation** - [docs.llmdevops.dev](https://docs.llmdevops.dev/auto-optimizer)

---

## License

This project is licensed under the **Apache License 2.0** - see the [LICENSE](LICENSE) file for details.

---

## Acknowledgments

Built with modern Rust technologies:

- [Tokio](https://tokio.rs/) - Async runtime
- [rdkafka](https://github.com/fede1024/rust-rdkafka) - Kafka client
- [sqlx](https://github.com/launchbadge/sqlx) - PostgreSQL driver
- [redis](https://github.com/redis-rs/redis-rs) - Redis client
- [OpenTelemetry](https://opentelemetry.io/) - Observability

---

<div align="center">

**[⬆ back to top](#llm-auto-optimizer)**

Made with ❤️ by the LLM DevOps Community

[GitHub](https://github.com/globalbusinessadvisors/llm-auto-optimizer) •
[Documentation](https://docs.llmdevops.dev) •
[Contributing](CONTRIBUTING.md)

</div>
