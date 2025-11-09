//! Configuration management for LLM Auto-Optimizer

use figment::{Figment, providers::{Format, Yaml, Env}};
use llm_optimizer_types::strategies::StrategyConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to load configuration: {0}")]
    LoadError(String),

    #[error("Invalid configuration: {0}")]
    ValidationError(String),
}

pub type Result<T> = std::result::Result<T, ConfigError>;

/// Main optimizer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizerConfig {
    /// Service configuration
    pub service: ServiceConfig,

    /// Database configuration
    pub database: DatabaseConfig,

    /// Integration endpoints
    pub integrations: IntegrationConfig,

    /// Optimization strategies
    pub strategies: StrategyConfig,

    /// Observability settings
    pub observability: ObservabilityConfig,
}

impl OptimizerConfig {
    /// Load configuration from file and environment
    pub fn load(config_path: Option<PathBuf>) -> Result<Self> {
        let mut figment = Figment::new();

        // Load from file if provided
        if let Some(path) = config_path {
            figment = figment.merge(Yaml::file(path));
        }

        // Override with environment variables (prefixed with OPTIMIZER_)
        figment = figment.merge(Env::prefixed("OPTIMIZER_").split("__"));

        figment.extract().map_err(|e| ConfigError::LoadError(e.to_string()))
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.service.port == 0 || self.service.port > 65535 {
            return Err(ConfigError::ValidationError("Invalid service port".to_string()));
        }

        if self.database.connection_string.is_empty() {
            return Err(ConfigError::ValidationError("Database connection string required".to_string()));
        }

        Ok(())
    }
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            service: ServiceConfig::default(),
            database: DatabaseConfig::default(),
            integrations: IntegrationConfig::default(),
            strategies: StrategyConfig::default(),
            observability: ObservabilityConfig::default(),
        }
    }
}

/// Service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Service name
    pub name: String,

    /// Service host
    pub host: String,

    /// Service port
    pub port: u16,

    /// Deployment mode
    pub mode: DeploymentMode,

    /// Optimization loop interval in seconds
    pub optimization_interval_secs: u64,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            name: "llm-auto-optimizer".to_string(),
            host: "0.0.0.0".to_string(),
            port: 8080,
            mode: DeploymentMode::Standalone,
            optimization_interval_secs: 900, // 15 minutes
        }
    }
}

/// Deployment mode
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentMode {
    Sidecar,
    Standalone,
    Daemon,
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database connection string
    pub connection_string: String,

    /// Maximum number of connections
    pub max_connections: u32,

    /// Connection timeout in seconds
    pub timeout_secs: u64,

    /// Enable migrations on startup
    pub auto_migrate: bool,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            connection_string: "sqlite://optimizer.db".to_string(),
            max_connections: 10,
            timeout_secs: 30,
            auto_migrate: true,
        }
    }
}

/// Integration endpoints configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationConfig {
    /// LLM Observatory endpoint
    pub observatory_url: Option<String>,

    /// LLM Orchestrator endpoint
    pub orchestrator_url: Option<String>,

    /// LLM Sentinel Kafka brokers
    pub sentinel_kafka_brokers: Option<Vec<String>>,

    /// LLM Governance endpoint
    pub governance_url: Option<String>,

    /// LLM Registry endpoint
    pub registry_url: Option<String>,
}

impl Default for IntegrationConfig {
    fn default() -> Self {
        Self {
            observatory_url: None,
            orchestrator_url: None,
            sentinel_kafka_brokers: None,
            governance_url: None,
            registry_url: None,
        }
    }
}

/// Observability configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    /// Log level
    pub log_level: String,

    /// Enable structured JSON logging
    pub json_logging: bool,

    /// Metrics export endpoint
    pub metrics_endpoint: Option<String>,

    /// Traces export endpoint
    pub traces_endpoint: Option<String>,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
            json_logging: false,
            metrics_endpoint: None,
            traces_endpoint: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = OptimizerConfig::default();
        assert_eq!(config.service.port, 8080);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation() {
        let mut config = OptimizerConfig::default();
        config.service.port = 0;
        assert!(config.validate().is_err());

        config.service.port = 8080;
        config.database.connection_string = String::new();
        assert!(config.validate().is_err());
    }
}
