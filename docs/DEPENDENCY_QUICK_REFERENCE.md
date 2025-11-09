# Rust Dependencies Quick Reference
## LLM-Auto-Optimizer

Quick lookup table for all recommended crates and their purposes.

---

## Core Dependencies (Always Include)

```toml
[dependencies]
# Async Runtime - ESSENTIAL
tokio = { version = "1.40", features = ["rt-multi-thread", "macros", "time", "sync"] }
tokio-cron-scheduler = "0.10"

# Serialization - ESSENTIAL
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
bincode = "1.3"

# Logging - ESSENTIAL
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# Error Handling - ESSENTIAL
anyhow = "1.0"
thiserror = "1.0"
```

---

## By Use Case

### 1. Async Scheduling

| Crate | Purpose | When to Use |
|-------|---------|-------------|
| `tokio` | Async runtime | Always (foundation) |
| `tokio-cron-scheduler` | Cron-style scheduling | Scheduled optimization runs |
| `tokio-util` | Task utilities | Task tracking, cancellation |
| `governor` | Rate limiting | API rate limiting |

### 2. Data Analysis

| Crate | Purpose | When to Use |
|-------|---------|-------------|
| `statrs` | Statistical functions | Metric analysis, anomaly detection |
| `ndarray` | N-dimensional arrays | Matrix operations, ML features |
| `ndarray-stats` | Array statistics | Percentiles, correlations |
| `linfa` | Machine learning | Clustering, classification |

### 3. Configuration

| Crate | Purpose | When to Use |
|-------|---------|-------------|
| `figment` | Config management | Complex, hierarchical config |
| `notify` | File watching | Hot-reload configuration |

### 4. Metrics & Telemetry

| Crate | Purpose | When to Use |
|-------|---------|-------------|
| `prometheus-client` | Prometheus metrics | Production metrics export |
| `tracing` | Structured logging | All logging needs |
| `tracing-subscriber` | Log formatting | Setting up logging |
| `opentelemetry` | Distributed tracing | Advanced observability |

### 5. State Management

| Crate | Purpose | When to Use |
|-------|---------|-------------|
| `sled` | Embedded database | Persistent state (< 1M records) |
| `rocksdb` | High-perf database | Heavy writes (> 100k/hour) |
| `dashmap` | Concurrent HashMap | In-memory concurrent state |
| `moka` | Caching with eviction | LRU/TTL caches |
| `parking_lot` | Better locks | When RwLock is needed |

### 6. HTTP & API

| Crate | Purpose | When to Use |
|-------|---------|-------------|
| `reqwest` | HTTP client | LLM API calls |
| `axum` | HTTP server | Metrics endpoints, webhooks |
| `tower` | Middleware | Request processing |
| `tonic` | gRPC | If using gRPC |

---

## Minimal Starter Cargo.toml

For a basic optimizer implementation:

```toml
[package]
name = "llm-auto-optimizer"
version = "0.1.0"
edition = "2021"

[dependencies]
# Runtime
tokio = { version = "1.40", features = ["full"] }
tokio-cron-scheduler = "0.10"

# HTTP
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
axum = "0.7"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Config
figment = { version = "0.10", features = ["toml", "env"] }

# Metrics
prometheus-client = "0.22"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# State
sled = "0.34"
dashmap = "6.0"

# Statistics
statrs = "0.17"

# Error handling
anyhow = "1.0"
thiserror = "1.0"

# Utilities
chrono = "0.4"
uuid = { version = "1.6", features = ["v4"] }
```

---

## Performance Characteristics

| Operation | Crate | Latency | Throughput |
|-----------|-------|---------|------------|
| Task spawn | tokio | ~500ns | 2M tasks/sec |
| Rate limit check | governor | <100ns | 10M checks/sec |
| HashMap read | dashmap | <50ns | 20M reads/sec |
| HashMap write | dashmap | ~100ns | 10M writes/sec |
| Cache hit | moka | ~100ns | 10M hits/sec |
| DB read | sled | ~1μs | 1M reads/sec |
| DB write | sled | ~10μs | 100k writes/sec |
| Metric update | prometheus-client | <50ns | 20M updates/sec |
| HTTP request | reqwest | ~10ms | 1k req/sec |

---

## Common Patterns

### Pattern 1: Basic Async App

```toml
tokio = { version = "1.40", features = ["rt-multi-thread", "macros"] }
anyhow = "1.0"
tracing = "0.1"
```

### Pattern 2: Web API

```toml
tokio = { version = "1.40", features = ["full"] }
axum = "0.7"
tower = "0.4"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### Pattern 3: Data Processing

```toml
tokio = { version = "1.40", features = ["full"] }
statrs = "0.17"
ndarray = "0.15"
serde = { version = "1.0", features = ["derive"] }
```

### Pattern 4: Stateful Service

```toml
tokio = { version = "1.40", features = ["full"] }
sled = "0.34"
dashmap = "6.0"
serde = { version = "1.0", features = ["derive"] }
bincode = "1.3"
```

---

## Trade-off Decision Tree

### Question 1: Need persistent storage?
- **Yes, < 1M records** → `sled`
- **Yes, > 1M records or heavy writes** → `rocksdb`
- **No, in-memory only** → `dashmap`
- **Need caching with eviction** → `moka`

### Question 2: Need configuration?
- **Simple, file-based** → `config-rs`
- **Complex, multiple sources** → `figment`
- **Need hot-reload** → `figment` + `notify`

### Question 3: Need HTTP?
- **Client only** → `reqwest`
- **Server only** → `axum`
- **Both** → `reqwest` + `axum`
- **gRPC** → `tonic`

### Question 4: Need ML/statistics?
- **Basic statistics** → `statrs`
- **Advanced numerical** → `ndarray` + `ndarray-stats`
- **Machine learning** → `linfa`
- **Custom algorithms** → Build on `ndarray`

### Question 5: Need metrics?
- **Prometheus only** → `prometheus-client`
- **Multiple exporters** → `opentelemetry`
- **Logs only** → `tracing` + `tracing-subscriber`

---

## Version Updates

When updating dependencies:

1. **Check breaking changes** in CHANGELOG
2. **Test incrementally** - update one major dep at a time
3. **Watch for MSRV changes** - some crates bump Rust version
4. **Use `cargo update`** for patch versions
5. **Use `cargo tree`** to check dependency conflicts

```bash
# Update to latest compatible versions
cargo update

# Check for outdated dependencies
cargo install cargo-outdated
cargo outdated

# Audit for security issues
cargo install cargo-audit
cargo audit
```

---

## Troubleshooting

### Issue: Slow compile times
**Solution:**
```toml
# In Cargo.toml
[profile.dev]
opt-level = 1  # Some optimization in dev

# Use mold linker (Linux)
# .cargo/config.toml
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]
```

### Issue: Large binary size
**Solution:**
```toml
[profile.release]
strip = true
lto = "thin"
codegen-units = 1
opt-level = "z"  # Optimize for size
```

### Issue: Runtime performance
**Solution:**
- Use `--release` mode
- Enable CPU-specific optimizations:
  ```bash
  RUSTFLAGS="-C target-cpu=native" cargo build --release
  ```
- Profile with:
  ```bash
  cargo install flamegraph
  cargo flamegraph
  ```

---

## Resources

- **Cargo Book:** https://doc.rust-lang.org/cargo/
- **Crates.io:** https://crates.io/
- **Lib.rs:** https://lib.rs/ (better search/filtering)
- **Blessed.rs:** https://blessed.rs/ (curated list)
- **Are We Web Yet:** https://www.arewewebyet.org/
- **Are We Async Yet:** https://areweasyncyet.rs/

---

## Next Steps

1. Review `/workspaces/llm-auto-optimizer/RUST_TECHNICAL_DEPENDENCIES.md` for detailed integration patterns
2. Start with minimal Cargo.toml above
3. Add dependencies as features are implemented
4. Set up CI to run `cargo audit` and `cargo outdated`
5. Benchmark critical paths with `criterion`

**Recommendation:** Start simple, add complexity as needed. The minimal starter is enough for a working prototype.
