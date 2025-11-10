//! Rollback Engine for the Actuator
//!
//! This module provides automatic and manual rollback capabilities with:
//! - Automatic rollback triggers (health, error rate, latency, quality, cost)
//! - Fast rollback mode (immediate revert)
//! - Gradual rollback (reverse canary phases)
//! - Configuration snapshot restoration
//! - State recovery and validation
//! - Rollback history tracking
//! - Notification support

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use super::config::RollbackConfig;
use super::error::{ActuatorError, ActuatorResult, DeploymentState};
use super::traits::{ConfigurationManager, HealthMonitor};
use super::types::{
    ConfigurationSnapshot, DeploymentMetrics, DeploymentStatus, HealthStatus, RollbackReason,
    RollbackRequest, RollbackResult,
};
use crate::decision::{ConfigChange, SystemMetrics};

/// Rollback mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RollbackMode {
    /// Fast rollback - immediate revert to previous configuration
    Fast,
    /// Gradual rollback - reverse through canary phases
    Gradual,
}

/// Rollback phase for gradual rollback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackPhase {
    /// Phase number (decreasing)
    pub phase: usize,
    /// Traffic percentage to old config
    pub traffic_percent: f64,
    /// Start time
    pub start_time: Option<DateTime<Utc>>,
    /// End time
    pub end_time: Option<DateTime<Utc>>,
    /// Whether this phase is complete
    pub completed: bool,
}

/// Rollback execution status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackStatus {
    /// Rollback ID
    pub rollback_id: String,
    /// Deployment being rolled back
    pub deployment_id: String,
    /// Rollback mode used
    pub mode: RollbackMode,
    /// Current phase (for gradual rollback)
    pub current_phase: Option<usize>,
    /// Start time
    pub start_time: DateTime<Utc>,
    /// Completion time
    pub end_time: Option<DateTime<Utc>>,
    /// Whether rollback is complete
    pub completed: bool,
    /// Whether rollback was successful
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Phases (for gradual rollback)
    pub phases: Vec<RollbackPhase>,
}

/// Rollback history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackHistoryEntry {
    /// Rollback ID
    pub rollback_id: String,
    /// Deployment ID
    pub deployment_id: String,
    /// Request details
    pub request: RollbackRequest,
    /// Result
    pub result: RollbackResult,
    /// Status
    pub status: RollbackStatus,
    /// Trigger details
    pub trigger: RollbackTrigger,
}

/// Rollback trigger details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackTrigger {
    /// Trigger type
    pub trigger_type: RollbackReason,
    /// Timestamp when triggered
    pub timestamp: DateTime<Utc>,
    /// Automatic or manual
    pub automatic: bool,
    /// Metrics at trigger time
    pub metrics: Option<DeploymentMetrics>,
    /// Health status at trigger time
    pub health_status: Option<HealthStatus>,
    /// Threshold that was exceeded (if applicable)
    pub threshold_exceeded: Option<ThresholdViolation>,
}

/// Threshold violation details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdViolation {
    /// Metric name
    pub metric: String,
    /// Actual value
    pub actual_value: f64,
    /// Threshold value
    pub threshold_value: f64,
    /// Comparison operator
    pub operator: String,
}

/// Rollback notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackNotification {
    /// Notification ID
    pub id: String,
    /// Deployment ID
    pub deployment_id: String,
    /// Rollback ID
    pub rollback_id: String,
    /// Reason for rollback
    pub reason: RollbackReason,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Severity
    pub severity: NotificationSeverity,
    /// Message
    pub message: String,
    /// Additional details
    pub details: HashMap<String, serde_json::Value>,
}

/// Notification severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Rollback engine statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RollbackStats {
    /// Total rollbacks executed
    pub total_rollbacks: u64,
    /// Automatic rollbacks
    pub automatic_rollbacks: u64,
    /// Manual rollbacks
    pub manual_rollbacks: u64,
    /// Successful rollbacks
    pub successful_rollbacks: u64,
    /// Failed rollbacks
    pub failed_rollbacks: u64,
    /// Fast rollbacks
    pub fast_rollbacks: u64,
    /// Gradual rollbacks
    pub gradual_rollbacks: u64,
    /// Average rollback duration (seconds)
    pub avg_rollback_duration_secs: f64,
    /// Fastest rollback (seconds)
    pub fastest_rollback_secs: f64,
    /// Rollbacks by reason
    pub rollbacks_by_reason: HashMap<String, u64>,
}

/// Rollback Engine implementation
pub struct RollbackEngine {
    /// Configuration
    config: RollbackConfig,
    /// Configuration manager for applying/reverting changes
    config_manager: Arc<RwLock<dyn ConfigurationManager>>,
    /// Health monitor for validation
    health_monitor: Arc<RwLock<dyn HealthMonitor>>,
    /// Rollback history
    history: Arc<RwLock<VecDeque<RollbackHistoryEntry>>>,
    /// Active rollback status
    active_rollbacks: Arc<RwLock<HashMap<String, RollbackStatus>>>,
    /// Configuration snapshots
    snapshots: Arc<RwLock<HashMap<String, ConfigurationSnapshot>>>,
    /// Statistics
    stats: Arc<RwLock<RollbackStats>>,
    /// Notification handlers
    notification_handlers: Arc<RwLock<Vec<Box<dyn NotificationHandler>>>>,
}

impl RollbackEngine {
    /// Create a new rollback engine
    pub fn new(
        config: RollbackConfig,
        config_manager: Arc<RwLock<dyn ConfigurationManager>>,
        health_monitor: Arc<RwLock<dyn HealthMonitor>>,
    ) -> Self {
        Self {
            config,
            config_manager,
            health_monitor,
            history: Arc::new(RwLock::new(VecDeque::new())),
            active_rollbacks: Arc::new(RwLock::new(HashMap::new())),
            snapshots: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(RollbackStats::default())),
            notification_handlers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Check if rollback should be triggered automatically
    pub async fn should_trigger_rollback(
        &self,
        deployment_id: &str,
        status: &DeploymentStatus,
        metrics: &SystemMetrics,
    ) -> ActuatorResult<Option<RollbackTrigger>> {
        if !self.config.auto_rollback {
            return Ok(None);
        }

        // Check health status
        if !status.health.healthy {
            return Ok(Some(RollbackTrigger {
                trigger_type: RollbackReason::HealthCheckFailed,
                timestamp: Utc::now(),
                automatic: true,
                metrics: Some(status.metrics.clone()),
                health_status: Some(status.health.clone()),
                threshold_exceeded: None,
            }));
        }

        // Check error rate
        if metrics.error_rate_pct > 5.0 {
            return Ok(Some(RollbackTrigger {
                trigger_type: RollbackReason::HighErrorRate,
                timestamp: Utc::now(),
                automatic: true,
                metrics: Some(status.metrics.clone()),
                health_status: Some(status.health.clone()),
                threshold_exceeded: Some(ThresholdViolation {
                    metric: "error_rate_pct".to_string(),
                    actual_value: metrics.error_rate_pct,
                    threshold_value: 5.0,
                    operator: ">".to_string(),
                }),
            }));
        }

        // Check latency (P99)
        if metrics.p99_latency_ms > 2000.0 {
            return Ok(Some(RollbackTrigger {
                trigger_type: RollbackReason::HighLatency,
                timestamp: Utc::now(),
                automatic: true,
                metrics: Some(status.metrics.clone()),
                health_status: Some(status.health.clone()),
                threshold_exceeded: Some(ThresholdViolation {
                    metric: "p99_latency_ms".to_string(),
                    actual_value: metrics.p99_latency_ms,
                    threshold_value: 2000.0,
                    operator: ">".to_string(),
                }),
            }));
        }

        // Check quality degradation
        if let Some(avg_rating) = metrics.avg_rating {
            if avg_rating < 3.5 {
                return Ok(Some(RollbackTrigger {
                    trigger_type: RollbackReason::QualityDegradation,
                    timestamp: Utc::now(),
                    automatic: true,
                    metrics: Some(status.metrics.clone()),
                    health_status: Some(status.health.clone()),
                    threshold_exceeded: Some(ThresholdViolation {
                        metric: "avg_rating".to_string(),
                        actual_value: avg_rating,
                        threshold_value: 3.5,
                        operator: "<".to_string(),
                    }),
                }));
            }
        }

        // Check cost increase
        let cost_increase_pct = if status.metrics.control_cost_per_request > 0.0 {
            ((status.metrics.canary_cost_per_request - status.metrics.control_cost_per_request)
                / status.metrics.control_cost_per_request)
                * 100.0
        } else {
            0.0
        };

        if cost_increase_pct > 20.0 {
            return Ok(Some(RollbackTrigger {
                trigger_type: RollbackReason::HighCost,
                timestamp: Utc::now(),
                automatic: true,
                metrics: Some(status.metrics.clone()),
                health_status: Some(status.health.clone()),
                threshold_exceeded: Some(ThresholdViolation {
                    metric: "cost_increase_pct".to_string(),
                    actual_value: cost_increase_pct,
                    threshold_value: 20.0,
                    operator: ">".to_string(),
                }),
            }));
        }

        Ok(None)
    }

    /// Execute rollback
    pub async fn execute_rollback(
        &self,
        request: RollbackRequest,
        deployment_status: &DeploymentStatus,
        config_changes: Vec<ConfigChange>,
    ) -> ActuatorResult<RollbackResult> {
        let start_time = Utc::now();
        let rollback_id = format!("rollback-{}", uuid::Uuid::new_v4());

        // Determine rollback mode
        let mode = if self.config.fast_rollback || request.reason == RollbackReason::HealthCheckFailed
        {
            RollbackMode::Fast
        } else {
            RollbackMode::Gradual
        };

        // Create rollback status
        let mut status = RollbackStatus {
            rollback_id: rollback_id.clone(),
            deployment_id: request.deployment_id.clone(),
            mode,
            current_phase: None,
            start_time,
            end_time: None,
            completed: false,
            success: false,
            error: None,
            phases: Vec::new(),
        };

        // Store active rollback
        {
            let mut active = self.active_rollbacks.write().await;
            active.insert(request.deployment_id.clone(), status.clone());
        }

        // Send notification
        if self.config.enable_notifications {
            let notification = RollbackNotification {
                id: format!("notif-{}", uuid::Uuid::new_v4()),
                deployment_id: request.deployment_id.clone(),
                rollback_id: rollback_id.clone(),
                reason: request.reason.clone(),
                timestamp: Utc::now(),
                severity: if request.automatic {
                    NotificationSeverity::Warning
                } else {
                    NotificationSeverity::Info
                },
                message: format!(
                    "Rollback initiated for deployment {} ({})",
                    request.deployment_id,
                    if request.automatic { "automatic" } else { "manual" }
                ),
                details: request.metadata.clone(),
            };
            self.send_notification(notification).await?;
        }

        // Execute rollback based on mode
        let result = match mode {
            RollbackMode::Fast => {
                self.execute_fast_rollback(&request, &config_changes, &mut status)
                    .await
            }
            RollbackMode::Gradual => {
                self.execute_gradual_rollback(&request, &config_changes, deployment_status, &mut status)
                    .await
            }
        };

        // Update status
        let end_time = Utc::now();
        let duration = end_time.signed_duration_since(start_time);
        status.end_time = Some(end_time);
        status.completed = true;

        let rollback_result = match result {
            Ok(_) => {
                status.success = true;

                // Validate system health after rollback
                let validation_result = self.validate_after_rollback(&request.deployment_id).await;

                let success = validation_result.unwrap_or(false);
                if !success {
                    status.success = false;
                    status.error = Some("Post-rollback validation failed".to_string());
                }

                RollbackResult {
                    deployment_id: request.deployment_id.clone(),
                    success,
                    error: status.error.clone(),
                    reverted_changes: config_changes.clone(),
                    timestamp: end_time,
                    duration: Duration::from_secs(duration.num_seconds().max(0) as u64),
                }
            }
            Err(e) => {
                status.success = false;
                status.error = Some(e.to_string());

                RollbackResult {
                    deployment_id: request.deployment_id.clone(),
                    success: false,
                    error: Some(e.to_string()),
                    reverted_changes: Vec::new(),
                    timestamp: end_time,
                    duration: Duration::from_secs(duration.num_seconds().max(0) as u64),
                }
            }
        };

        // Update statistics
        self.update_stats(&rollback_result, mode, request.automatic).await;

        // Store in history
        self.store_history(
            rollback_id.clone(),
            request.clone(),
            rollback_result.clone(),
            status.clone(),
        )
        .await?;

        // Remove from active rollbacks
        {
            let mut active = self.active_rollbacks.write().await;
            active.remove(&request.deployment_id);
        }

        // Send completion notification
        if self.config.enable_notifications {
            let notification = RollbackNotification {
                id: format!("notif-{}", uuid::Uuid::new_v4()),
                deployment_id: request.deployment_id.clone(),
                rollback_id,
                reason: request.reason,
                timestamp: Utc::now(),
                severity: if rollback_result.success {
                    NotificationSeverity::Info
                } else {
                    NotificationSeverity::Critical
                },
                message: format!(
                    "Rollback {} for deployment {}",
                    if rollback_result.success { "completed" } else { "failed" },
                    request.deployment_id
                ),
                details: HashMap::new(),
            };
            self.send_notification(notification).await?;
        }

        Ok(rollback_result)
    }

    /// Execute fast rollback (immediate revert)
    async fn execute_fast_rollback(
        &self,
        request: &RollbackRequest,
        config_changes: &[ConfigChange],
        status: &mut RollbackStatus,
    ) -> ActuatorResult<()> {
        // Revert configuration changes immediately
        let mut config_manager = self.config_manager.write().await;
        config_manager.revert_config(config_changes).await?;

        Ok(())
    }

    /// Execute gradual rollback (reverse through canary phases)
    async fn execute_gradual_rollback(
        &self,
        request: &RollbackRequest,
        config_changes: &[ConfigChange],
        deployment_status: &DeploymentStatus,
        status: &mut RollbackStatus,
    ) -> ActuatorResult<()> {
        // Determine rollback phases (reverse of deployment phases)
        let phases = if let Some(current_phase) = deployment_status.current_phase {
            // Reverse from current phase back to 0%
            (0..=current_phase)
                .rev()
                .map(|phase| RollbackPhase {
                    phase,
                    traffic_percent: match phase {
                        0 => 0.0,
                        1 => 1.0,
                        2 => 5.0,
                        3 => 25.0,
                        4 => 50.0,
                        _ => 100.0,
                    },
                    start_time: None,
                    end_time: None,
                    completed: false,
                })
                .collect()
        } else {
            vec![RollbackPhase {
                phase: 0,
                traffic_percent: 0.0,
                start_time: None,
                end_time: None,
                completed: false,
            }]
        };

        status.phases = phases;

        // Execute each phase
        for (idx, phase) in status.phases.iter_mut().enumerate() {
            status.current_phase = Some(idx);
            phase.start_time = Some(Utc::now());

            // Gradually shift traffic back
            // In a real implementation, this would interact with traffic routing
            tokio::time::sleep(Duration::from_secs(10)).await;

            phase.end_time = Some(Utc::now());
            phase.completed = true;
        }

        // Finally, revert the configuration changes
        let mut config_manager = self.config_manager.write().await;
        config_manager.revert_config(config_changes).await?;

        Ok(())
    }

    /// Validate system health after rollback
    async fn validate_after_rollback(&self, deployment_id: &str) -> ActuatorResult<bool> {
        // Wait a brief period for system to stabilize
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Check health
        let health_monitor = self.health_monitor.read().await;
        let health_summary = health_monitor.get_health_summary(deployment_id).await?;

        Ok(health_summary.healthy)
    }

    /// Store snapshot before deployment
    pub async fn store_snapshot(
        &self,
        deployment_id: &str,
        snapshot: ConfigurationSnapshot,
    ) -> ActuatorResult<()> {
        if !self.config.enable_backup {
            return Ok(());
        }

        let mut snapshots = self.snapshots.write().await;
        snapshots.insert(deployment_id.to_string(), snapshot);
        Ok(())
    }

    /// Restore from snapshot
    pub async fn restore_from_snapshot(&self, deployment_id: &str) -> ActuatorResult<()> {
        let snapshot = {
            let snapshots = self.snapshots.read().await;
            snapshots
                .get(deployment_id)
                .cloned()
                .ok_or_else(|| {
                    ActuatorError::RollbackFailed(format!(
                        "No snapshot found for deployment {}",
                        deployment_id
                    ))
                })?
        };

        let mut config_manager = self.config_manager.write().await;
        config_manager.restore_snapshot(&snapshot.id).await?;

        Ok(())
    }

    /// Get rollback history
    pub async fn get_history(&self, limit: Option<usize>) -> Vec<RollbackHistoryEntry> {
        let history = self.history.read().await;
        let limit = limit.unwrap_or(100);
        history.iter().take(limit).cloned().collect()
    }

    /// Get active rollback status
    pub async fn get_active_rollback(
        &self,
        deployment_id: &str,
    ) -> Option<RollbackStatus> {
        let active = self.active_rollbacks.read().await;
        active.get(deployment_id).cloned()
    }

    /// Get statistics
    pub async fn get_stats(&self) -> RollbackStats {
        self.stats.read().await.clone()
    }

    /// Add notification handler
    pub async fn add_notification_handler(
        &self,
        handler: Box<dyn NotificationHandler>,
    ) {
        let mut handlers = self.notification_handlers.write().await;
        handlers.push(handler);
    }

    /// Send notification
    async fn send_notification(&self, notification: RollbackNotification) -> ActuatorResult<()> {
        let handlers = self.notification_handlers.read().await;
        for handler in handlers.iter() {
            handler.handle(notification.clone()).await?;
        }
        Ok(())
    }

    /// Store rollback in history
    async fn store_history(
        &self,
        rollback_id: String,
        request: RollbackRequest,
        result: RollbackResult,
        status: RollbackStatus,
    ) -> ActuatorResult<()> {
        let trigger = RollbackTrigger {
            trigger_type: request.reason.clone(),
            timestamp: request.timestamp,
            automatic: request.automatic,
            metrics: None,
            health_status: None,
            threshold_exceeded: None,
        };

        let entry = RollbackHistoryEntry {
            rollback_id,
            deployment_id: request.deployment_id.clone(),
            request,
            result,
            status,
            trigger,
        };

        let mut history = self.history.write().await;
        history.push_front(entry);

        // Trim history to max size
        while history.len() > self.config.max_rollback_history {
            history.pop_back();
        }

        Ok(())
    }

    /// Update statistics
    async fn update_stats(&self, result: &RollbackResult, mode: RollbackMode, automatic: bool) {
        let mut stats = self.stats.write().await;

        stats.total_rollbacks += 1;

        if automatic {
            stats.automatic_rollbacks += 1;
        } else {
            stats.manual_rollbacks += 1;
        }

        if result.success {
            stats.successful_rollbacks += 1;
        } else {
            stats.failed_rollbacks += 1;
        }

        match mode {
            RollbackMode::Fast => stats.fast_rollbacks += 1,
            RollbackMode::Gradual => stats.gradual_rollbacks += 1,
        }

        let duration_secs = result.duration.as_secs_f64();

        // Update average duration
        if stats.total_rollbacks == 1 {
            stats.avg_rollback_duration_secs = duration_secs;
            stats.fastest_rollback_secs = duration_secs;
        } else {
            stats.avg_rollback_duration_secs = (stats.avg_rollback_duration_secs
                * (stats.total_rollbacks - 1) as f64
                + duration_secs)
                / stats.total_rollbacks as f64;

            if duration_secs < stats.fastest_rollback_secs {
                stats.fastest_rollback_secs = duration_secs;
            }
        }
    }
}

/// Trait for notification handlers
#[async_trait]
pub trait NotificationHandler: Send + Sync {
    /// Handle a rollback notification
    async fn handle(&self, notification: RollbackNotification) -> ActuatorResult<()>;
}

/// Mock configuration manager for testing
#[cfg(test)]
pub struct MockConfigManager;

#[cfg(test)]
#[async_trait]
impl ConfigurationManager for MockConfigManager {
    async fn apply_config(&mut self, _changes: &[ConfigChange]) -> ActuatorResult<()> {
        Ok(())
    }

    async fn revert_config(&mut self, _changes: &[ConfigChange]) -> ActuatorResult<()> {
        Ok(())
    }

    async fn create_snapshot(&mut self) -> ActuatorResult<ConfigurationSnapshot> {
        Ok(ConfigurationSnapshot {
            id: "test-snapshot".to_string(),
            timestamp: Utc::now(),
            config: HashMap::new(),
            deployment_id: None,
            version: 1,
            tags: Vec::new(),
        })
    }

    async fn restore_snapshot(&mut self, _snapshot_id: &str) -> ActuatorResult<()> {
        Ok(())
    }

    async fn get_config(&self) -> ActuatorResult<HashMap<String, serde_json::Value>> {
        Ok(HashMap::new())
    }

    async fn validate_config(&self, _changes: &[ConfigChange]) -> ActuatorResult<()> {
        Ok(())
    }
}

/// Mock health monitor for testing
#[cfg(test)]
pub struct MockHealthMonitor {
    pub healthy: bool,
}

#[cfg(test)]
#[async_trait]
impl HealthMonitor for MockHealthMonitor {
    async fn check_health(
        &self,
        _deployment_id: &str,
        _metrics: &SystemMetrics,
    ) -> ActuatorResult<Vec<super::types::HealthCheckResult>> {
        Ok(Vec::new())
    }

    async fn should_rollback(
        &self,
        _deployment_id: &str,
        _status: &DeploymentStatus,
    ) -> ActuatorResult<bool> {
        Ok(!self.healthy)
    }

    async fn get_health_summary(
        &self,
        _deployment_id: &str,
    ) -> ActuatorResult<HealthStatus> {
        Ok(HealthStatus {
            healthy: self.healthy,
            checks_passed: if self.healthy { 5 } else { 0 },
            checks_failed: if self.healthy { 0 } else { 5 },
            check_results: Vec::new(),
            last_check_time: Utc::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_engine() -> RollbackEngine {
        let config = RollbackConfig::default();
        let config_manager = Arc::new(RwLock::new(MockConfigManager));
        let health_monitor = Arc::new(RwLock::new(MockHealthMonitor { healthy: true }));

        RollbackEngine::new(config, config_manager, health_monitor)
    }

    fn create_test_deployment_status() -> DeploymentStatus {
        DeploymentStatus {
            deployment_id: "test-deployment".to_string(),
            state: DeploymentState::RollingOut,
            current_phase: Some(2),
            traffic_percent: 25.0,
            start_time: Utc::now(),
            phase_start_time: Some(Utc::now()),
            health: HealthStatus {
                healthy: true,
                checks_passed: 5,
                checks_failed: 0,
                check_results: Vec::new(),
                last_check_time: Utc::now(),
            },
            metrics: DeploymentMetrics::default(),
            errors: Vec::new(),
            last_updated: Utc::now(),
        }
    }

    fn create_test_metrics() -> SystemMetrics {
        SystemMetrics {
            avg_latency_ms: 100.0,
            p95_latency_ms: 200.0,
            p99_latency_ms: 300.0,
            success_rate_pct: 99.5,
            error_rate_pct: 0.5,
            throughput_rps: 1000.0,
            total_cost_usd: 100.0,
            daily_cost_usd: 10.0,
            avg_rating: Some(4.5),
            cache_hit_rate_pct: Some(80.0),
            request_count: 10000,
            timestamp: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_no_rollback_trigger_when_healthy() {
        let engine = create_test_engine();
        let status = create_test_deployment_status();
        let metrics = create_test_metrics();

        let trigger = engine
            .should_trigger_rollback("test-deployment", &status, &metrics)
            .await
            .unwrap();

        assert!(trigger.is_none());
    }

    #[tokio::test]
    async fn test_rollback_trigger_on_high_error_rate() {
        let engine = create_test_engine();
        let status = create_test_deployment_status();
        let mut metrics = create_test_metrics();
        metrics.error_rate_pct = 10.0;

        let trigger = engine
            .should_trigger_rollback("test-deployment", &status, &metrics)
            .await
            .unwrap();

        assert!(trigger.is_some());
        let trigger = trigger.unwrap();
        assert_eq!(trigger.trigger_type, RollbackReason::HighErrorRate);
        assert!(trigger.automatic);
    }

    #[tokio::test]
    async fn test_rollback_trigger_on_high_latency() {
        let engine = create_test_engine();
        let status = create_test_deployment_status();
        let mut metrics = create_test_metrics();
        metrics.p99_latency_ms = 3000.0;

        let trigger = engine
            .should_trigger_rollback("test-deployment", &status, &metrics)
            .await
            .unwrap();

        assert!(trigger.is_some());
        let trigger = trigger.unwrap();
        assert_eq!(trigger.trigger_type, RollbackReason::HighLatency);
    }

    #[tokio::test]
    async fn test_rollback_trigger_on_quality_degradation() {
        let engine = create_test_engine();
        let status = create_test_deployment_status();
        let mut metrics = create_test_metrics();
        metrics.avg_rating = Some(2.5);

        let trigger = engine
            .should_trigger_rollback("test-deployment", &status, &metrics)
            .await
            .unwrap();

        assert!(trigger.is_some());
        let trigger = trigger.unwrap();
        assert_eq!(trigger.trigger_type, RollbackReason::QualityDegradation);
    }

    #[tokio::test]
    async fn test_rollback_trigger_on_health_check_failure() {
        let engine = create_test_engine();
        let mut status = create_test_deployment_status();
        status.health.healthy = false;
        let metrics = create_test_metrics();

        let trigger = engine
            .should_trigger_rollback("test-deployment", &status, &metrics)
            .await
            .unwrap();

        assert!(trigger.is_some());
        let trigger = trigger.unwrap();
        assert_eq!(trigger.trigger_type, RollbackReason::HealthCheckFailed);
    }

    #[tokio::test]
    async fn test_fast_rollback_execution() {
        let engine = create_test_engine();
        let status = create_test_deployment_status();

        let request = RollbackRequest {
            deployment_id: "test-deployment".to_string(),
            reason: RollbackReason::HealthCheckFailed,
            timestamp: Utc::now(),
            automatic: true,
            metadata: HashMap::new(),
        };

        let config_changes = vec![ConfigChange {
            config_type: crate::decision::ConfigType::Model,
            path: "model.name".to_string(),
            old_value: Some(serde_json::json!("gpt-3.5-turbo")),
            new_value: serde_json::json!("gpt-4"),
            description: "Model upgrade".to_string(),
        }];

        let result = engine
            .execute_rollback(request, &status, config_changes.clone())
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.reverted_changes.len(), config_changes.len());
    }

    #[tokio::test]
    async fn test_gradual_rollback_execution() {
        let mut config = RollbackConfig::default();
        config.fast_rollback = false;

        let config_manager = Arc::new(RwLock::new(MockConfigManager));
        let health_monitor = Arc::new(RwLock::new(MockHealthMonitor { healthy: true }));
        let engine = RollbackEngine::new(config, config_manager, health_monitor);

        let status = create_test_deployment_status();

        let request = RollbackRequest {
            deployment_id: "test-deployment".to_string(),
            reason: RollbackReason::Manual,
            timestamp: Utc::now(),
            automatic: false,
            metadata: HashMap::new(),
        };

        let config_changes = vec![ConfigChange {
            config_type: crate::decision::ConfigType::Model,
            path: "model.name".to_string(),
            old_value: Some(serde_json::json!("gpt-3.5-turbo")),
            new_value: serde_json::json!("gpt-4"),
            description: "Model upgrade".to_string(),
        }];

        let result = engine
            .execute_rollback(request, &status, config_changes)
            .await
            .unwrap();

        assert!(result.success);
    }

    #[tokio::test]
    async fn test_snapshot_storage_and_restoration() {
        let engine = create_test_engine();

        let snapshot = ConfigurationSnapshot {
            id: "snapshot-1".to_string(),
            timestamp: Utc::now(),
            config: HashMap::new(),
            deployment_id: Some("test-deployment".to_string()),
            version: 1,
            tags: vec!["production".to_string()],
        };

        engine
            .store_snapshot("test-deployment", snapshot)
            .await
            .unwrap();

        let result = engine.restore_from_snapshot("test-deployment").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_rollback_history_tracking() {
        let engine = create_test_engine();
        let status = create_test_deployment_status();

        let request = RollbackRequest {
            deployment_id: "test-deployment".to_string(),
            reason: RollbackReason::Manual,
            timestamp: Utc::now(),
            automatic: false,
            metadata: HashMap::new(),
        };

        let config_changes = vec![ConfigChange {
            config_type: crate::decision::ConfigType::Model,
            path: "model.name".to_string(),
            old_value: Some(serde_json::json!("gpt-3.5-turbo")),
            new_value: serde_json::json!("gpt-4"),
            description: "Model upgrade".to_string(),
        }];

        engine
            .execute_rollback(request, &status, config_changes)
            .await
            .unwrap();

        let history = engine.get_history(None).await;
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].deployment_id, "test-deployment");
    }

    #[tokio::test]
    async fn test_rollback_statistics() {
        let engine = create_test_engine();
        let status = create_test_deployment_status();

        // Execute a few rollbacks
        for i in 0..3 {
            let request = RollbackRequest {
                deployment_id: format!("test-deployment-{}", i),
                reason: if i % 2 == 0 {
                    RollbackReason::Manual
                } else {
                    RollbackReason::HighErrorRate
                },
                timestamp: Utc::now(),
                automatic: i % 2 != 0,
                metadata: HashMap::new(),
            };

            let config_changes = vec![ConfigChange {
                config_type: crate::decision::ConfigType::Model,
                path: "model.name".to_string(),
                old_value: Some(serde_json::json!("gpt-3.5-turbo")),
                new_value: serde_json::json!("gpt-4"),
                description: "Model upgrade".to_string(),
            }];

            engine
                .execute_rollback(request, &status, config_changes)
                .await
                .unwrap();
        }

        let stats = engine.get_stats().await;
        assert_eq!(stats.total_rollbacks, 3);
        assert_eq!(stats.manual_rollbacks, 2);
        assert_eq!(stats.automatic_rollbacks, 1);
        assert_eq!(stats.successful_rollbacks, 3);
    }

    #[tokio::test]
    async fn test_active_rollback_tracking() {
        let engine = create_test_engine();
        let status = create_test_deployment_status();

        let request = RollbackRequest {
            deployment_id: "test-deployment".to_string(),
            reason: RollbackReason::Manual,
            timestamp: Utc::now(),
            automatic: false,
            metadata: HashMap::new(),
        };

        let config_changes = vec![ConfigChange {
            config_type: crate::decision::ConfigType::Model,
            path: "model.name".to_string(),
            old_value: Some(serde_json::json!("gpt-3.5-turbo")),
            new_value: serde_json::json!("gpt-4"),
            description: "Model upgrade".to_string(),
        }];

        engine
            .execute_rollback(request, &status, config_changes)
            .await
            .unwrap();

        // Should be removed from active after completion
        let active = engine.get_active_rollback("test-deployment").await;
        assert!(active.is_none());
    }

    #[tokio::test]
    async fn test_post_rollback_validation() {
        let config_manager = Arc::new(RwLock::new(MockConfigManager));
        let health_monitor = Arc::new(RwLock::new(MockHealthMonitor { healthy: true }));
        let engine = RollbackEngine::new(
            RollbackConfig::default(),
            config_manager,
            health_monitor,
        );

        let result = engine.validate_after_rollback("test-deployment").await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_post_rollback_validation_failure() {
        let config_manager = Arc::new(RwLock::new(MockConfigManager));
        let health_monitor = Arc::new(RwLock::new(MockHealthMonitor { healthy: false }));
        let engine = RollbackEngine::new(
            RollbackConfig::default(),
            config_manager,
            health_monitor,
        );

        let result = engine.validate_after_rollback("test-deployment").await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }
}
