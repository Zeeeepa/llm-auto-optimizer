//! Actuator configuration

use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::types::{CanaryConfig, DeploymentStrategy, SuccessCriteria, TrafficSplitStrategy};

/// Configuration for the Actuator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActuatorConfig {
    /// Actuator ID
    pub id: String,

    /// Whether the actuator is enabled
    pub enabled: bool,

    /// Default deployment strategy
    pub default_strategy: DeploymentStrategy,

    /// Canary deployment configuration
    pub canary: CanaryConfig,

    /// Health monitoring configuration
    pub health_monitoring: HealthMonitoringConfig,

    /// Rollback configuration
    pub rollback: RollbackConfig,

    /// Configuration management
    pub configuration: ConfigurationManagementConfig,

    /// Maximum concurrent deployments
    pub max_concurrent_deployments: usize,

    /// Deployment timeout
    pub deployment_timeout: Duration,
}

impl Default for ActuatorConfig {
    fn default() -> Self {
        Self {
            id: "actuator".to_string(),
            enabled: true,
            default_strategy: DeploymentStrategy::Canary,
            canary: CanaryConfig::default(),
            health_monitoring: HealthMonitoringConfig::default(),
            rollback: RollbackConfig::default(),
            configuration: ConfigurationManagementConfig::default(),
            max_concurrent_deployments: 3,
            deployment_timeout: Duration::from_secs(3600), // 1 hour
        }
    }
}

/// Health monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMonitoringConfig {
    /// Health check interval
    pub check_interval: Duration,

    /// Health check timeout
    pub check_timeout: Duration,

    /// Consecutive failures before rollback
    pub consecutive_failures_threshold: usize,

    /// Success criteria
    pub success_criteria: SuccessCriteria,

    /// Enable automatic health checks
    pub auto_health_check: bool,

    /// Enable metrics collection
    pub enable_metrics_collection: bool,

    /// Metrics collection interval
    pub metrics_collection_interval: Duration,
}

impl Default for HealthMonitoringConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(30),
            check_timeout: Duration::from_secs(10),
            consecutive_failures_threshold: 3,
            success_criteria: SuccessCriteria::default(),
            auto_health_check: true,
            enable_metrics_collection: true,
            metrics_collection_interval: Duration::from_secs(10),
        }
    }
}

/// Rollback configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackConfig {
    /// Enable automatic rollback
    pub auto_rollback: bool,

    /// Rollback timeout
    pub rollback_timeout: Duration,

    /// Enable fast rollback (skip gradual phases)
    pub fast_rollback: bool,

    /// Enable configuration backup before deployment
    pub enable_backup: bool,

    /// Maximum rollback history to keep
    pub max_rollback_history: usize,

    /// Enable rollback notifications
    pub enable_notifications: bool,
}

impl Default for RollbackConfig {
    fn default() -> Self {
        Self {
            auto_rollback: true,
            rollback_timeout: Duration::from_secs(300), // 5 minutes
            fast_rollback: true,
            enable_backup: true,
            max_rollback_history: 100,
            enable_notifications: true,
        }
    }
}

/// Configuration management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigurationManagementConfig {
    /// Enable configuration versioning
    pub enable_versioning: bool,

    /// Enable configuration validation
    pub enable_validation: bool,

    /// Enable audit logging
    pub enable_audit_log: bool,

    /// Configuration snapshot interval
    pub snapshot_interval: Duration,

    /// Maximum snapshots to keep
    pub max_snapshots: usize,

    /// Enable dry-run mode
    pub enable_dry_run: bool,
}

impl Default for ConfigurationManagementConfig {
    fn default() -> Self {
        Self {
            enable_versioning: true,
            enable_validation: true,
            enable_audit_log: true,
            snapshot_interval: Duration::from_secs(3600), // 1 hour
            max_snapshots: 100,
            enable_dry_run: false,
        }
    }
}
