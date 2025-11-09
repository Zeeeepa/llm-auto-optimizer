# Multi-stage Dockerfile for LLM-Auto-Optimizer

# Stage 1: Build
FROM rust:1.75-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy manifest files
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/

# Build dependencies (cached layer)
RUN cargo build --release --locked

# Stage 2: Runtime
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 optimizer

# Create app directory
WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/llm-optimizer /app/llm-optimizer

# Copy default configuration
COPY config.example.yaml /app/config.yaml

# Set ownership
RUN chown -R optimizer:optimizer /app

# Switch to non-root user
USER optimizer

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/usr/bin/curl", "-f", "http://localhost:8080/health", "||", "exit", "1"]

# Run the optimizer
ENTRYPOINT ["/app/llm-optimizer"]
CMD ["--config", "/app/config.yaml"]
