//! Actuator core traits

use async_trait::async_trait;

use super::error::{ActuatorError, ActuatorResult, ActuatorState};
use super::types::{
    ActuatorStats, DeploymentRequest, DeploymentStatus, HealthCheckResult, RollbackRequest,
    RollbackResult, SystemMetrics,
};
use crate::decision::Decision;

/// Core trait for the Actuator
#[async_trait]
pub trait Actuator: Send + Sync {
    /// Get the name of this actuator
    fn name(&self) -> &str;

    /// Get the current state
    fn state(&self) -> ActuatorState;

    /// Start the actuator
    async fn start(&mut self) -> ActuatorResult<()>;

    /// Stop the actuator
    async fn stop(&mut self) -> ActuatorResult<()>;

    /// Execute a deployment
    ///
    /// # Arguments
    /// * `request` - Deployment request with decision and strategy
    ///
    /// # Returns
    /// Deployment status
    async fn deploy(&mut self, request: DeploymentRequest) -> ActuatorResult<DeploymentStatus>;

    /// Get deployment status
    async fn get_deployment_status(&self, deployment_id: &str) -> ActuatorResult<DeploymentStatus>;

    /// Rollback a deployment
    async fn rollback(&mut self, request: RollbackRequest) -> ActuatorResult<RollbackResult>;

    /// Health check
    async fn health_check(&self) -> ActuatorResult<Vec<HealthCheckResult>>;

    /// Get statistics
    fn get_stats(&self) -> ActuatorStats;

    /// Reset the actuator
    async fn reset(&mut self) -> ActuatorResult<()>;
}

/// Trait for canary deployment
#[async_trait]
pub trait CanaryDeployment: Send + Sync {
    /// Start a canary deployment
    async fn start_canary(
        &mut self,
        request: DeploymentRequest,
    ) -> ActuatorResult<DeploymentStatus>;

    /// Promote to next canary phase
    async fn promote(&mut self, deployment_id: &str) -> ActuatorResult<DeploymentStatus>;

    /// Complete canary deployment (promote to 100%)
    async fn complete(&mut self, deployment_id: &str) -> ActuatorResult<DeploymentStatus>;

    /// Abort canary deployment
    async fn abort(&mut self, deployment_id: &str) -> ActuatorResult<RollbackResult>;

    /// Get current canary status
    async fn get_status(&self, deployment_id: &str) -> ActuatorResult<DeploymentStatus>;

    /// Check if phase can be promoted
    async fn can_promote(&self, deployment_id: &str) -> ActuatorResult<bool>;
}

/// Trait for health monitoring
#[async_trait]
pub trait HealthMonitor: Send + Sync {
    /// Perform health checks
    async fn check_health(
        &self,
        deployment_id: &str,
        metrics: &SystemMetrics,
    ) -> ActuatorResult<Vec<HealthCheckResult>>;

    /// Check if deployment should be rolled back
    async fn should_rollback(
        &self,
        deployment_id: &str,
        status: &DeploymentStatus,
    ) -> ActuatorResult<bool>;

    /// Get health status summary
    async fn get_health_summary(
        &self,
        deployment_id: &str,
    ) -> ActuatorResult<super::types::HealthStatus>;
}

/// Trait for configuration management
#[async_trait]
pub trait ConfigurationManager: Send + Sync {
    /// Apply configuration changes
    async fn apply_config(
        &mut self,
        changes: &[crate::decision::ConfigChange],
    ) -> ActuatorResult<()>;

    /// Revert configuration changes
    async fn revert_config(
        &mut self,
        changes: &[crate::decision::ConfigChange],
    ) -> ActuatorResult<()>;

    /// Create configuration snapshot
    async fn create_snapshot(&mut self) -> ActuatorResult<super::types::ConfigurationSnapshot>;

    /// Restore from snapshot
    async fn restore_snapshot(&mut self, snapshot_id: &str) -> ActuatorResult<()>;

    /// Get current configuration
    async fn get_config(&self) -> ActuatorResult<std::collections::HashMap<String, serde_json::Value>>;

    /// Validate configuration
    async fn validate_config(
        &self,
        changes: &[crate::decision::ConfigChange],
    ) -> ActuatorResult<()>;
}

/// Trait for traffic splitting
#[async_trait]
pub trait TrafficSplitter: Send + Sync {
    /// Route request to canary or control
    async fn route_request(&self, request_id: &str, canary_percent: f64) -> ActuatorResult<bool>;

    /// Get current routing statistics
    fn get_routing_stats(&self) -> std::collections::HashMap<String, u64>;
}
