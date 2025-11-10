//! Prometheus metrics for the Stream Processor
//!
//! This module provides comprehensive metrics export for monitoring and observability
//! of the stream processing system. All metrics follow Prometheus naming conventions
//! and best practices.

mod labels;
mod prometheus;
mod registry;
mod server;

pub use labels::{LabelSet, LabelValue, MetricLabels};
pub use prometheus::{ProcessorMetrics, MetricsSnapshot};
pub use registry::{MetricsRegistry, METRICS_REGISTRY};
pub use server::{MetricsServer, MetricsServerConfig, HealthStatus};

use thiserror::Error;

/// Errors that can occur in the metrics subsystem
#[derive(Error, Debug)]
pub enum MetricsError {
    #[error("Failed to start metrics server: {0}")]
    ServerStartError(String),

    #[error("Failed to bind to address {address}: {source}")]
    BindError {
        address: String,
        source: std::io::Error,
    },

    #[error("Invalid metric label: {0}")]
    InvalidLabel(String),

    #[error("Metric registration failed: {0}")]
    RegistrationError(String),

    #[error("Metric encoding error: {0}")]
    EncodingError(String),
}

pub type Result<T> = std::result::Result<T, MetricsError>;
