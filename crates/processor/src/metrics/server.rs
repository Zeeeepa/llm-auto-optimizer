//! HTTP server for Prometheus metrics endpoint
//!
//! This module provides an HTTP server that exposes metrics at /metrics
//! along with health check and readiness probe endpoints.

use super::registry::MetricsRegistry;
use super::MetricsError;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tracing::{debug, error, info};

/// Configuration for the metrics server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsServerConfig {
    /// Bind address for the metrics server
    pub bind_address: String,

    /// Port for the metrics server
    pub port: u16,
}

impl Default for MetricsServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_string(),
            port: 9090,
        }
    }
}

impl MetricsServerConfig {
    /// Create a new metrics server config
    pub fn new(bind_address: impl Into<String>, port: u16) -> Self {
        Self {
            bind_address: bind_address.into(),
            port,
        }
    }

    /// Get the socket address
    pub fn socket_addr(&self) -> Result<SocketAddr, MetricsError> {
        format!("{}:{}", self.bind_address, self.port)
            .parse()
            .map_err(|e| MetricsError::BindError {
                address: format!("{}:{}", self.bind_address, self.port),
                source: std::io::Error::new(std::io::ErrorKind::InvalidInput, e),
            })
    }
}

/// Health status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
}

impl HealthStatus {
    /// Create a healthy status
    pub fn healthy() -> Self {
        Self {
            status: "healthy".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: 0,
        }
    }

    /// Create a healthy status with uptime
    pub fn healthy_with_uptime(uptime: Duration) -> Self {
        Self {
            status: "healthy".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: uptime.as_secs(),
        }
    }
}

/// Readiness status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessStatus {
    pub ready: bool,
    pub checks: Vec<ReadinessCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessCheck {
    pub name: String,
    pub ready: bool,
    pub message: Option<String>,
}

impl ReadinessStatus {
    /// Create a ready status
    pub fn ready() -> Self {
        Self {
            ready: true,
            checks: vec![ReadinessCheck {
                name: "processor".to_string(),
                ready: true,
                message: None,
            }],
        }
    }

    /// Create a not ready status
    pub fn not_ready(message: impl Into<String>) -> Self {
        Self {
            ready: false,
            checks: vec![ReadinessCheck {
                name: "processor".to_string(),
                ready: false,
                message: Some(message.into()),
            }],
        }
    }
}

/// Shared state for the metrics server
#[derive(Clone)]
struct ServerState {
    registry: Arc<MetricsRegistry>,
    start_time: Instant,
}

/// HTTP server for exposing Prometheus metrics
pub struct MetricsServer {
    config: MetricsServerConfig,
    registry: Arc<MetricsRegistry>,
}

impl MetricsServer {
    /// Create a new metrics server
    pub fn new(config: MetricsServerConfig) -> Self {
        Self {
            config,
            registry: MetricsRegistry::global(),
        }
    }

    /// Create a metrics server with custom registry
    pub fn with_registry(config: MetricsServerConfig, registry: Arc<MetricsRegistry>) -> Self {
        Self { config, registry }
    }

    /// Start the metrics server
    pub async fn start(self) -> Result<(), MetricsError> {
        let addr = self.config.socket_addr()?;

        info!(
            "Starting metrics server on http://{}:{}",
            self.config.bind_address, self.config.port
        );

        let state = ServerState {
            registry: self.registry,
            start_time: Instant::now(),
        };

        let app = Router::new()
            .route("/metrics", get(metrics_handler))
            .route("/health", get(health_handler))
            .route("/ready", get(ready_handler))
            .with_state(state);

        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| MetricsError::BindError {
                address: format!("{}:{}", self.config.bind_address, self.config.port),
                source: e,
            })?;

        info!(
            "Metrics server listening on http://{}",
            listener.local_addr().map_err(|e| MetricsError::BindError {
                address: format!("{}:{}", self.config.bind_address, self.config.port),
                source: e,
            })?
        );

        axum::serve(listener, app)
            .await
            .map_err(|e| MetricsError::ServerStartError(e.to_string()))?;

        Ok(())
    }

    /// Get the metrics server configuration
    pub fn config(&self) -> &MetricsServerConfig {
        &self.config
    }
}

/// Handler for /metrics endpoint
async fn metrics_handler(State(state): State<ServerState>) -> Response {
    debug!("Metrics endpoint called");

    match state.registry.encode() {
        Ok(metrics) => {
            debug!("Successfully encoded metrics");
            (
                StatusCode::OK,
                [("content-type", "text/plain; version=0.0.4")],
                metrics,
            )
                .into_response()
        }
        Err(e) => {
            error!("Failed to encode metrics: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to encode metrics: {}", e),
            )
                .into_response()
        }
    }
}

/// Handler for /health endpoint
async fn health_handler(State(state): State<ServerState>) -> Json<HealthStatus> {
    debug!("Health check endpoint called");
    let uptime = state.start_time.elapsed();
    Json(HealthStatus::healthy_with_uptime(uptime))
}

/// Handler for /ready endpoint
async fn ready_handler() -> Response {
    debug!("Readiness probe endpoint called");
    let status = ReadinessStatus::ready();

    if status.ready {
        (StatusCode::OK, Json(status)).into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(status)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = MetricsServerConfig::default();
        assert_eq!(config.bind_address, "0.0.0.0");
        assert_eq!(config.port, 9090);
    }

    #[test]
    fn test_config_custom() {
        let config = MetricsServerConfig::new("127.0.0.1", 8080);
        assert_eq!(config.bind_address, "127.0.0.1");
        assert_eq!(config.port, 8080);
    }

    #[test]
    fn test_socket_addr() {
        let config = MetricsServerConfig::new("127.0.0.1", 8080);
        let addr = config.socket_addr().unwrap();
        assert_eq!(addr.port(), 8080);
    }

    #[test]
    fn test_health_status() {
        let status = HealthStatus::healthy();
        assert_eq!(status.status, "healthy");
        assert_eq!(status.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn test_readiness_status() {
        let ready = ReadinessStatus::ready();
        assert!(ready.ready);
        assert_eq!(ready.checks.len(), 1);

        let not_ready = ReadinessStatus::not_ready("Not initialized");
        assert!(!not_ready.ready);
        assert!(not_ready.checks[0].message.is_some());
    }

    #[tokio::test]
    async fn test_metrics_server_creation() {
        let config = MetricsServerConfig::new("127.0.0.1", 9091);
        let server = MetricsServer::new(config);
        assert_eq!(server.config().port, 9091);
    }
}
