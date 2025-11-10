# Actuator Implementation Plan: Canary Deployments and Rollback

**Version:** 1.0
**Status:** Planning
**Target Release:** Phase 3 - Production Readiness
**Estimated Effort:** 2-3 weeks
**Lines of Code:** ~5,500-7,000

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Implementation Overview](#2-implementation-overview)
3. [File Structure](#3-file-structure)
4. [Implementation Phases](#4-implementation-phases)
5. [Module Interfaces](#5-module-interfaces)
6. [Testing Strategy](#6-testing-strategy)
7. [Lines of Code Estimates](#7-lines-of-code-estimates)
8. [Dependencies](#8-dependencies)
9. [Risk Mitigation](#9-risk-mitigation)
10. [Validation Criteria](#10-validation-criteria)
11. [Deployment Checklist](#11-deployment-checklist)
12. [Integration Points](#12-integration-points)
13. [Code Scaffolding Examples](#13-code-scaffolding-examples)

---

## 1. Executive Summary

The **Actuator Engine** is the execution layer of the LLM Auto-Optimizer system, responsible for safely deploying configuration changes recommended by the Decision Engine. It implements progressive canary deployments with automatic rollback capabilities to ensure zero-downtime updates and rapid recovery from degraded performance.

### Key Features

- **Canary Deployment Engine**: Progressive rollout with configurable traffic percentages (1% → 5% → 25% → 50% → 100%)
- **Health Monitoring**: Real-time health checks across all deployment stages
- **Automatic Rollback**: Intelligent rollback based on performance thresholds and SLO violations
- **Configuration Versioning**: Full audit trail with version history and diff tracking
- **Safety Mechanisms**: Circuit breakers, rate limiters, and manual override capabilities
- **Multi-Strategy Support**: Blue-green, canary, shadow, and A/B testing deployments

### Success Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| **Deployment Success Rate** | >99.5% | Successful canary completions without rollback |
| **Rollback Time** | <30 seconds (p95) | Time from anomaly detection to rollback completion |
| **Mean Time to Detect (MTTD)** | <60 seconds | Time to detect performance degradation |
| **Configuration Propagation** | <5 seconds (p99) | Time to propagate config to all instances |
| **Zero Downtime** | 100% | No service interruptions during deployments |

---

## 2. Implementation Overview

### Architecture

The Actuator consists of five core subsystems:

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Actuator Engine                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────┐      ┌──────────────────┐                    │
│  │   Configuration  │──────│   Health Monitor │                    │
│  │     Manager      │      │                  │                    │
│  │                  │      │ • Metrics Check  │                    │
│  │ • Versioning     │      │ • SLO Validation │                    │
│  │ • Storage        │      │ • Circuit Breaker│                    │
│  │ • Audit Log      │      └────────┬─────────┘                    │
│  └────────┬─────────┘               │                              │
│           │                         │                              │
│           ▼                         ▼                              │
│  ┌──────────────────────────────────────────┐                      │
│  │      Canary Deployment Engine            │                      │
│  │                                          │                      │
│  │  • Traffic Splitting                    │                      │
│  │  • Progressive Rollout                  │                      │
│  │  • Stage Management                     │                      │
│  │  • Deployment Strategies                │                      │
│  └────────┬─────────────────────────────────┘                      │
│           │                                                        │
│           ▼                                                        │
│  ┌──────────────────┐      ┌──────────────────┐                    │
│  │  Rollback Engine │      │  Integration API │                    │
│  │                  │      │                  │                    │
│  │ • Auto Rollback  │      │ • Decision Engine│                    │
│  │ • Manual Trigger │      │ • LLM Providers  │                    │
│  │ • State Recovery │      │ • Observability  │                    │
│  └──────────────────┘      └──────────────────┘                    │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Core Principles

1. **Safety First**: Every deployment stage has multiple safety checks
2. **Observability**: Comprehensive logging, metrics, and tracing
3. **Resilience**: Automatic recovery with manual override capabilities
4. **Auditability**: Complete configuration history and change tracking
5. **Performance**: Sub-second configuration propagation

---

## 3. File Structure

All files will be created under `/workspaces/llm-auto-optimizer/crates/actuator/src/`:

```
crates/actuator/
├── Cargo.toml                          # Updated with dependencies
├── src/
│   ├── lib.rs                          # Main module exports (~150 LOC)
│   ├── error.rs                        # Error types and Result alias (~200 LOC)
│   ├── types.rs                        # Core data structures (~300 LOC)
│   │
│   ├── config/                         # Configuration Management
│   │   ├── mod.rs                      # Module exports (~50 LOC)
│   │   ├── manager.rs                  # Configuration manager (~400 LOC)
│   │   ├── version.rs                  # Version tracking (~200 LOC)
│   │   ├── storage.rs                  # Configuration storage (~350 LOC)
│   │   └── audit.rs                    # Audit logging (~250 LOC)
│   │
│   ├── health/                         # Health Monitoring
│   │   ├── mod.rs                      # Module exports (~50 LOC)
│   │   ├── monitor.rs                  # Health monitor (~450 LOC)
│   │   ├── metrics.rs                  # Metrics collection (~300 LOC)
│   │   ├── slo.rs                      # SLO validation (~200 LOC)
│   │   └── checker.rs                  # Health checks (~250 LOC)
│   │
│   ├── canary/                         # Canary Deployment
│   │   ├── mod.rs                      # Module exports (~50 LOC)
│   │   ├── engine.rs                   # Main canary engine (~600 LOC)
│   │   ├── stage.rs                    # Stage management (~300 LOC)
│   │   ├── traffic.rs                  # Traffic splitting (~350 LOC)
│   │   ├── strategy.rs                 # Deployment strategies (~400 LOC)
│   │   └── evaluator.rs                # Stage evaluation (~300 LOC)
│   │
│   ├── rollback/                       # Rollback System
│   │   ├── mod.rs                      # Module exports (~50 LOC)
│   │   ├── engine.rs                   # Rollback engine (~400 LOC)
│   │   ├── trigger.rs                  # Trigger conditions (~250 LOC)
│   │   └── recovery.rs                 # State recovery (~300 LOC)
│   │
│   ├── integration/                    # External Integrations
│   │   ├── mod.rs                      # Module exports (~50 LOC)
│   │   ├── decision.rs                 # Decision engine client (~200 LOC)
│   │   ├── provider.rs                 # LLM provider integration (~300 LOC)
│   │   └── telemetry.rs                # Telemetry integration (~200 LOC)
│   │
│   └── traits.rs                       # Common traits (~150 LOC)
│
├── examples/
│   ├── basic_canary.rs                 # Basic canary deployment (~100 LOC)
│   ├── blue_green.rs                   # Blue-green deployment (~120 LOC)
│   ├── rollback_demo.rs                # Rollback demonstration (~100 LOC)
│   └── integration_demo.rs             # Full integration example (~150 LOC)
│
└── tests/
    ├── common/
    │   └── mod.rs                      # Test utilities (~200 LOC)
    ├── config_tests.rs                 # Config manager tests (~300 LOC)
    ├── health_tests.rs                 # Health monitor tests (~350 LOC)
    ├── canary_tests.rs                 # Canary engine tests (~400 LOC)
    ├── rollback_tests.rs               # Rollback tests (~300 LOC)
    └── integration_tests.rs            # Full integration tests (~500 LOC)
```

**Total Estimated Files**: 43 files
**Total Estimated LOC**: ~6,800 lines

---

## 4. Implementation Phases

### Phase 1: Core Framework (Days 1-2)

**Objective**: Establish foundational types, traits, and error handling.

#### Tasks

1. **Define Core Types** (`types.rs`)
   - `DeploymentConfig`: Configuration to be deployed
   - `DeploymentStatus`: Current deployment state
   - `CanaryStage`: Deployment stage enumeration
   - `HealthMetrics`: Health check data structures
   - `RollbackReason`: Rollback trigger reasons

2. **Create Error Types** (`error.rs`)
   - `ActuatorError`: Comprehensive error enumeration
   - Result type alias
   - Error conversion implementations
   - Error context and tracing

3. **Define Traits** (`traits.rs`)
   - `ConfigurationStore`: Storage abstraction
   - `HealthChecker`: Health monitoring abstraction
   - `DeploymentStrategy`: Strategy pattern interface
   - `MetricsProvider`: Metrics collection abstraction

#### Code Example - Core Types

```rust
// types.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Configuration to be deployed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentConfig {
    /// Unique configuration ID
    pub id: Uuid,
    /// Configuration version
    pub version: u64,
    /// Model selection
    pub model: String,
    /// Model parameters
    pub parameters: HashMap<String, serde_json::Value>,
    /// Prompt template
    pub prompt_template: Option<String>,
    /// Metadata
    pub metadata: HashMap<String, String>,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
}

/// Canary deployment stages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanaryStage {
    /// Initial validation (0% traffic)
    Validation,
    /// 1% traffic
    Stage1Percent,
    /// 5% traffic
    Stage5Percent,
    /// 25% traffic
    Stage25Percent,
    /// 50% traffic
    Stage50Percent,
    /// 100% traffic (complete)
    Complete,
}

impl CanaryStage {
    /// Get traffic percentage for this stage
    pub fn traffic_percentage(&self) -> f64 {
        match self {
            Self::Validation => 0.0,
            Self::Stage1Percent => 1.0,
            Self::Stage5Percent => 5.0,
            Self::Stage25Percent => 25.0,
            Self::Stage50Percent => 50.0,
            Self::Complete => 100.0,
        }
    }

    /// Get next stage in progression
    pub fn next(&self) -> Option<Self> {
        match self {
            Self::Validation => Some(Self::Stage1Percent),
            Self::Stage1Percent => Some(Self::Stage5Percent),
            Self::Stage5Percent => Some(Self::Stage25Percent),
            Self::Stage25Percent => Some(Self::Stage50Percent),
            Self::Stage50Percent => Some(Self::Complete),
            Self::Complete => None,
        }
    }

    /// Get minimum observation duration for this stage
    pub fn min_observation_duration(&self) -> std::time::Duration {
        use std::time::Duration;
        match self {
            Self::Validation => Duration::from_secs(30),
            Self::Stage1Percent => Duration::from_secs(60),
            Self::Stage5Percent => Duration::from_secs(120),
            Self::Stage25Percent => Duration::from_secs(180),
            Self::Stage50Percent => Duration::from_secs(300),
            Self::Complete => Duration::from_secs(0),
        }
    }
}

/// Deployment status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentStatus {
    /// Deployment ID
    pub deployment_id: Uuid,
    /// Configuration being deployed
    pub config: DeploymentConfig,
    /// Current stage
    pub current_stage: CanaryStage,
    /// Stage start time
    pub stage_started_at: DateTime<Utc>,
    /// Overall deployment state
    pub state: DeploymentState,
    /// Health metrics
    pub health_metrics: HealthMetrics,
    /// Deployment history
    pub history: Vec<StageHistory>,
}

/// Deployment state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentState {
    /// Deployment in progress
    InProgress,
    /// Deployment paused (waiting for manual approval)
    Paused,
    /// Deployment succeeded
    Succeeded,
    /// Deployment failed
    Failed,
    /// Deployment rolling back
    RollingBack,
    /// Deployment rolled back
    RolledBack,
}

/// Health metrics for a deployment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMetrics {
    /// Success rate (0.0-1.0)
    pub success_rate: f64,
    /// Average latency (milliseconds)
    pub avg_latency_ms: f64,
    /// P95 latency (milliseconds)
    pub p95_latency_ms: f64,
    /// P99 latency (milliseconds)
    pub p99_latency_ms: f64,
    /// Error rate (0.0-1.0)
    pub error_rate: f64,
    /// Cost per request (USD)
    pub cost_per_request: f64,
    /// Sample size
    pub sample_size: usize,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Stage history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageHistory {
    /// Stage
    pub stage: CanaryStage,
    /// Started at
    pub started_at: DateTime<Utc>,
    /// Completed at (if finished)
    pub completed_at: Option<DateTime<Utc>>,
    /// Final health metrics
    pub final_metrics: Option<HealthMetrics>,
    /// Outcome
    pub outcome: StageOutcome,
}

/// Stage outcome
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageOutcome {
    /// Stage passed health checks
    Passed,
    /// Stage failed health checks
    Failed,
    /// Stage skipped
    Skipped,
    /// Stage in progress
    InProgress,
}

/// Rollback reason
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RollbackReason {
    /// High error rate detected
    HighErrorRate {
        current: f64,
        threshold: f64,
    },
    /// Increased latency detected
    HighLatency {
        current_p95: f64,
        threshold_p95: f64,
    },
    /// SLO violation detected
    SloViolation {
        slo_name: String,
        details: String,
    },
    /// Manual rollback triggered
    ManualTrigger {
        triggered_by: String,
        reason: String,
    },
    /// Health check failure
    HealthCheckFailure {
        check_name: String,
        error: String,
    },
    /// Circuit breaker opened
    CircuitBreakerOpen {
        component: String,
    },
}
```

#### Deliverables

- `types.rs` with complete type definitions (~300 LOC)
- `error.rs` with error handling (~200 LOC)
- `traits.rs` with core traits (~150 LOC)
- Unit tests for type conversions and validations (~150 LOC)

---

### Phase 2: Configuration Manager (Days 3-4)

**Objective**: Implement configuration storage, versioning, and audit logging.

#### Tasks

1. **Configuration Manager** (`config/manager.rs`)
   - Store and retrieve configurations
   - Version management
   - Configuration validation
   - Atomic updates with CAS (Compare-And-Swap)

2. **Version Control** (`config/version.rs`)
   - Semantic versioning support
   - Version comparison and validation
   - Version history tracking
   - Diff generation between versions

3. **Storage Backend** (`config/storage.rs`)
   - In-memory storage (for testing)
   - Redis-backed storage (production)
   - PostgreSQL storage (audit trail)
   - Sled embedded storage (optional)

4. **Audit Logging** (`config/audit.rs`)
   - Comprehensive change tracking
   - User attribution
   - Diff generation
   - Query and reporting APIs

#### Code Example - Configuration Manager

```rust
// config/manager.rs
use crate::error::{ActuatorError, Result};
use crate::traits::ConfigurationStore;
use crate::types::{DeploymentConfig};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Configuration manager
pub struct ConfigurationManager {
    /// Storage backend
    store: Arc<dyn ConfigurationStore>,
    /// Current active configuration
    active_config: Arc<RwLock<Option<DeploymentConfig>>>,
    /// Configuration history (ID -> Config)
    history: Arc<RwLock<Vec<DeploymentConfig>>>,
    /// Audit logger
    audit_logger: Arc<dyn AuditLogger>,
}

impl ConfigurationManager {
    /// Create new configuration manager
    pub fn new(
        store: Arc<dyn ConfigurationStore>,
        audit_logger: Arc<dyn AuditLogger>,
    ) -> Self {
        Self {
            store,
            active_config: Arc::new(RwLock::new(None)),
            history: Arc::new(RwLock::new(Vec::new())),
            audit_logger,
        }
    }

    /// Store a new configuration
    pub async fn store_config(&self, config: DeploymentConfig) -> Result<()> {
        // Validate configuration
        self.validate_config(&config)?;

        // Store in backend
        self.store.store(&config).await?;

        // Add to history
        let mut history = self.history.write().await;
        history.push(config.clone());

        // Log audit event
        self.audit_logger
            .log_config_stored(&config)
            .await?;

        Ok(())
    }

    /// Get configuration by ID
    pub async fn get_config(&self, id: Uuid) -> Result<DeploymentConfig> {
        self.store.get(id).await
    }

    /// Get current active configuration
    pub async fn get_active_config(&self) -> Result<Option<DeploymentConfig>> {
        let active = self.active_config.read().await;
        Ok(active.clone())
    }

    /// Activate a configuration (atomic operation)
    pub async fn activate_config(&self, id: Uuid) -> Result<()> {
        // Get configuration
        let config = self.store.get(id).await?;

        // Get previous config for audit
        let prev_config = {
            let active = self.active_config.read().await;
            active.clone()
        };

        // Atomic update
        {
            let mut active = self.active_config.write().await;
            *active = Some(config.clone());
        }

        // Mark as active in store
        self.store.mark_active(id).await?;

        // Log audit event
        self.audit_logger
            .log_config_activated(&config, prev_config.as_ref())
            .await?;

        Ok(())
    }

    /// Get configuration history
    pub async fn get_history(&self, limit: usize) -> Result<Vec<DeploymentConfig>> {
        let history = self.history.read().await;
        Ok(history.iter().rev().take(limit).cloned().collect())
    }

    /// Compare two configurations
    pub async fn diff_configs(
        &self,
        id1: Uuid,
        id2: Uuid,
    ) -> Result<ConfigDiff> {
        let config1 = self.store.get(id1).await?;
        let config2 = self.store.get(id2).await?;
        Ok(ConfigDiff::compute(&config1, &config2))
    }

    /// Validate configuration
    fn validate_config(&self, config: &DeploymentConfig) -> Result<()> {
        // Validate model name is not empty
        if config.model.is_empty() {
            return Err(ActuatorError::InvalidConfiguration(
                "Model name cannot be empty".to_string(),
            ));
        }

        // Validate parameters
        if config.parameters.is_empty() {
            return Err(ActuatorError::InvalidConfiguration(
                "Parameters cannot be empty".to_string(),
            ));
        }

        // Additional validation rules...
        Ok(())
    }

    /// Rollback to previous configuration
    pub async fn rollback_to_previous(&self) -> Result<()> {
        let history = self.history.read().await;

        if history.len() < 2 {
            return Err(ActuatorError::InvalidState(
                "No previous configuration to rollback to".to_string(),
            ));
        }

        // Get second-to-last config (last is current)
        let prev_config = &history[history.len() - 2];

        drop(history); // Release read lock

        // Activate previous config
        self.activate_config(prev_config.id).await
    }
}

/// Configuration diff
#[derive(Debug, Clone)]
pub struct ConfigDiff {
    /// Model changed
    pub model_changed: bool,
    /// Parameter changes (key -> (old, new))
    pub parameter_changes: HashMap<String, (serde_json::Value, serde_json::Value)>,
    /// Prompt template changed
    pub prompt_changed: bool,
}

impl ConfigDiff {
    /// Compute diff between two configurations
    pub fn compute(old: &DeploymentConfig, new: &DeploymentConfig) -> Self {
        let model_changed = old.model != new.model;
        let prompt_changed = old.prompt_template != new.prompt_template;

        let mut parameter_changes = HashMap::new();
        for (key, new_val) in &new.parameters {
            if let Some(old_val) = old.parameters.get(key) {
                if old_val != new_val {
                    parameter_changes.insert(
                        key.clone(),
                        (old_val.clone(), new_val.clone()),
                    );
                }
            } else {
                // New parameter
                parameter_changes.insert(
                    key.clone(),
                    (serde_json::Value::Null, new_val.clone()),
                );
            }
        }

        Self {
            model_changed,
            parameter_changes,
            prompt_changed,
        }
    }

    /// Check if diff is empty
    pub fn is_empty(&self) -> bool {
        !self.model_changed && self.parameter_changes.is_empty() && !self.prompt_changed
    }
}
```

#### Deliverables

- Configuration manager with full CRUD operations (~400 LOC)
- Version tracking system (~200 LOC)
- Storage backend implementations (~350 LOC)
- Audit logging system (~250 LOC)
- Integration tests (~300 LOC)

---

### Phase 3: Health Monitor (Days 5-6)

**Objective**: Implement comprehensive health monitoring with SLO validation.

#### Tasks

1. **Health Monitor** (`health/monitor.rs`)
   - Continuous health metric collection
   - Statistical analysis (mean, median, percentiles)
   - Anomaly detection
   - Alert generation

2. **Metrics Collection** (`health/metrics.rs`)
   - Integration with OpenTelemetry
   - Custom metric definitions
   - Aggregation and windowing
   - Real-time metric updates

3. **SLO Validation** (`health/slo.rs`)
   - SLO definition and parsing
   - Error budget tracking
   - Burn rate calculation
   - Multi-window alerting

4. **Health Checkers** (`health/checker.rs`)
   - Liveness checks
   - Readiness checks
   - Component-specific checks
   - Batch health validation

#### Code Example - Health Monitor

```rust
// health/monitor.rs
use crate::error::{ActuatorError, Result};
use crate::types::{HealthMetrics, DeploymentConfig};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Health monitoring configuration
#[derive(Debug, Clone)]
pub struct HealthMonitorConfig {
    /// Sample window duration
    pub window_duration: Duration,
    /// Minimum sample size for statistical validity
    pub min_sample_size: usize,
    /// Success rate threshold (0.0-1.0)
    pub success_rate_threshold: f64,
    /// Error rate threshold (0.0-1.0)
    pub error_rate_threshold: f64,
    /// P95 latency threshold (milliseconds)
    pub p95_latency_threshold_ms: f64,
    /// P99 latency threshold (milliseconds)
    pub p99_latency_threshold_ms: f64,
}

impl Default for HealthMonitorConfig {
    fn default() -> Self {
        Self {
            window_duration: Duration::from_secs(60),
            min_sample_size: 100,
            success_rate_threshold: 0.99,
            error_rate_threshold: 0.01,
            p95_latency_threshold_ms: 500.0,
            p99_latency_threshold_ms: 1000.0,
        }
    }
}

/// Health monitor
pub struct HealthMonitor {
    /// Configuration
    config: HealthMonitorConfig,
    /// Metrics for control group (baseline)
    control_metrics: Arc<RwLock<MetricsWindow>>,
    /// Metrics for canary group
    canary_metrics: Arc<RwLock<MetricsWindow>>,
    /// SLO validator
    slo_validator: Arc<SloValidator>,
}

impl HealthMonitor {
    /// Create new health monitor
    pub fn new(
        config: HealthMonitorConfig,
        slo_validator: Arc<SloValidator>,
    ) -> Self {
        Self {
            config: config.clone(),
            control_metrics: Arc::new(RwLock::new(MetricsWindow::new(
                config.window_duration,
            ))),
            canary_metrics: Arc::new(RwLock::new(MetricsWindow::new(
                config.window_duration,
            ))),
            slo_validator,
        }
    }

    /// Record control group metric
    pub async fn record_control_metric(&self, metric: RequestMetric) {
        let mut control = self.control_metrics.write().await;
        control.add_metric(metric);
    }

    /// Record canary group metric
    pub async fn record_canary_metric(&self, metric: RequestMetric) {
        let mut canary = self.canary_metrics.write().await;
        canary.add_metric(metric);
    }

    /// Evaluate health status
    pub async fn evaluate_health(&self) -> Result<HealthEvaluation> {
        let control = self.control_metrics.read().await;
        let canary = self.canary_metrics.read().await;

        // Check minimum sample size
        if canary.sample_size() < self.config.min_sample_size {
            return Ok(HealthEvaluation::Insufficient {
                current_samples: canary.sample_size(),
                required_samples: self.config.min_sample_size,
            });
        }

        // Compute health metrics
        let control_health = control.compute_health_metrics();
        let canary_health = canary.compute_health_metrics();

        // Check absolute thresholds
        let threshold_check = self.check_absolute_thresholds(&canary_health)?;
        if !threshold_check.is_healthy() {
            return Ok(HealthEvaluation::Unhealthy {
                reason: threshold_check,
                control_metrics: control_health,
                canary_metrics: canary_health,
            });
        }

        // Compare with control group
        let comparison = self.compare_with_control(&control_health, &canary_health)?;
        if !comparison.is_healthy() {
            return Ok(HealthEvaluation::Degraded {
                reason: comparison,
                control_metrics: control_health,
                canary_metrics: canary_health,
            });
        }

        // Validate SLOs
        let slo_result = self.slo_validator.validate(&canary_health).await?;
        if !slo_result.is_passing() {
            return Ok(HealthEvaluation::SloViolation {
                violations: slo_result.violations,
                control_metrics: control_health,
                canary_metrics: canary_health,
            });
        }

        Ok(HealthEvaluation::Healthy {
            control_metrics: control_health,
            canary_metrics: canary_health,
        })
    }

    /// Check absolute thresholds
    fn check_absolute_thresholds(
        &self,
        metrics: &HealthMetrics,
    ) -> Result<ThresholdCheck> {
        let mut violations = Vec::new();

        // Check success rate
        if metrics.success_rate < self.config.success_rate_threshold {
            violations.push(format!(
                "Success rate {:.2}% below threshold {:.2}%",
                metrics.success_rate * 100.0,
                self.config.success_rate_threshold * 100.0
            ));
        }

        // Check error rate
        if metrics.error_rate > self.config.error_rate_threshold {
            violations.push(format!(
                "Error rate {:.2}% above threshold {:.2}%",
                metrics.error_rate * 100.0,
                self.config.error_rate_threshold * 100.0
            ));
        }

        // Check P95 latency
        if metrics.p95_latency_ms > self.config.p95_latency_threshold_ms {
            violations.push(format!(
                "P95 latency {:.1}ms above threshold {:.1}ms",
                metrics.p95_latency_ms,
                self.config.p95_latency_threshold_ms
            ));
        }

        // Check P99 latency
        if metrics.p99_latency_ms > self.config.p99_latency_threshold_ms {
            violations.push(format!(
                "P99 latency {:.1}ms above threshold {:.1}ms",
                metrics.p99_latency_ms,
                self.config.p99_latency_threshold_ms
            ));
        }

        if violations.is_empty() {
            Ok(ThresholdCheck::Passed)
        } else {
            Ok(ThresholdCheck::Failed { violations })
        }
    }

    /// Compare canary metrics with control
    fn compare_with_control(
        &self,
        control: &HealthMetrics,
        canary: &HealthMetrics,
    ) -> Result<ComparisonResult> {
        let mut issues = Vec::new();

        // Success rate regression (allow 1% degradation)
        let success_rate_diff = control.success_rate - canary.success_rate;
        if success_rate_diff > 0.01 {
            issues.push(format!(
                "Success rate degraded by {:.2}%",
                success_rate_diff * 100.0
            ));
        }

        // Latency regression (allow 10% increase)
        let p95_increase = (canary.p95_latency_ms - control.p95_latency_ms)
            / control.p95_latency_ms;
        if p95_increase > 0.10 {
            issues.push(format!(
                "P95 latency increased by {:.1}%",
                p95_increase * 100.0
            ));
        }

        // Error rate increase (allow 0.5% increase)
        let error_rate_diff = canary.error_rate - control.error_rate;
        if error_rate_diff > 0.005 {
            issues.push(format!(
                "Error rate increased by {:.2}%",
                error_rate_diff * 100.0
            ));
        }

        if issues.is_empty() {
            Ok(ComparisonResult::Passed)
        } else {
            Ok(ComparisonResult::Degraded { issues })
        }
    }

    /// Reset metrics (e.g., when moving to new stage)
    pub async fn reset_metrics(&self) {
        let mut control = self.control_metrics.write().await;
        let mut canary = self.canary_metrics.write().await;
        control.clear();
        canary.clear();
    }
}

/// Health evaluation result
#[derive(Debug, Clone)]
pub enum HealthEvaluation {
    /// Healthy - canary is performing well
    Healthy {
        control_metrics: HealthMetrics,
        canary_metrics: HealthMetrics,
    },
    /// Degraded - canary has minor issues
    Degraded {
        reason: ComparisonResult,
        control_metrics: HealthMetrics,
        canary_metrics: HealthMetrics,
    },
    /// Unhealthy - canary has critical issues
    Unhealthy {
        reason: ThresholdCheck,
        control_metrics: HealthMetrics,
        canary_metrics: HealthMetrics,
    },
    /// SLO violation detected
    SloViolation {
        violations: Vec<String>,
        control_metrics: HealthMetrics,
        canary_metrics: HealthMetrics,
    },
    /// Insufficient data for evaluation
    Insufficient {
        current_samples: usize,
        required_samples: usize,
    },
}

impl HealthEvaluation {
    /// Check if evaluation is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self, HealthEvaluation::Healthy { .. })
    }

    /// Check if should rollback
    pub fn should_rollback(&self) -> bool {
        matches!(
            self,
            HealthEvaluation::Unhealthy { .. } | HealthEvaluation::SloViolation { .. }
        )
    }
}

/// Request metric
#[derive(Debug, Clone)]
pub struct RequestMetric {
    pub success: bool,
    pub latency_ms: f64,
    pub cost: f64,
    pub timestamp: DateTime<Utc>,
}

/// Metrics window (sliding window of metrics)
struct MetricsWindow {
    window_duration: Duration,
    metrics: VecDeque<RequestMetric>,
}

impl MetricsWindow {
    fn new(window_duration: Duration) -> Self {
        Self {
            window_duration,
            metrics: VecDeque::new(),
        }
    }

    fn add_metric(&mut self, metric: RequestMetric) {
        // Remove old metrics outside window
        let cutoff = Utc::now() - chrono::Duration::from_std(self.window_duration).unwrap();
        while let Some(front) = self.metrics.front() {
            if front.timestamp < cutoff {
                self.metrics.pop_front();
            } else {
                break;
            }
        }

        // Add new metric
        self.metrics.push_back(metric);
    }

    fn sample_size(&self) -> usize {
        self.metrics.len()
    }

    fn compute_health_metrics(&self) -> HealthMetrics {
        if self.metrics.is_empty() {
            return HealthMetrics {
                success_rate: 0.0,
                avg_latency_ms: 0.0,
                p95_latency_ms: 0.0,
                p99_latency_ms: 0.0,
                error_rate: 1.0,
                cost_per_request: 0.0,
                sample_size: 0,
                timestamp: Utc::now(),
            };
        }

        let successes = self.metrics.iter().filter(|m| m.success).count();
        let success_rate = successes as f64 / self.metrics.len() as f64;
        let error_rate = 1.0 - success_rate;

        let mut latencies: Vec<f64> = self.metrics.iter()
            .map(|m| m.latency_ms)
            .collect();
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let avg_latency = latencies.iter().sum::<f64>() / latencies.len() as f64;
        let p95_idx = (latencies.len() as f64 * 0.95) as usize;
        let p99_idx = (latencies.len() as f64 * 0.99) as usize;

        let total_cost: f64 = self.metrics.iter().map(|m| m.cost).sum();
        let cost_per_request = total_cost / self.metrics.len() as f64;

        HealthMetrics {
            success_rate,
            avg_latency_ms: avg_latency,
            p95_latency_ms: latencies[p95_idx],
            p99_latency_ms: latencies[p99_idx],
            error_rate,
            cost_per_request,
            sample_size: self.metrics.len(),
            timestamp: Utc::now(),
        }
    }

    fn clear(&mut self) {
        self.metrics.clear();
    }
}

/// Threshold check result
#[derive(Debug, Clone)]
pub enum ThresholdCheck {
    Passed,
    Failed { violations: Vec<String> },
}

impl ThresholdCheck {
    pub fn is_healthy(&self) -> bool {
        matches!(self, ThresholdCheck::Passed)
    }
}

/// Comparison result
#[derive(Debug, Clone)]
pub enum ComparisonResult {
    Passed,
    Degraded { issues: Vec<String> },
}

impl ComparisonResult {
    pub fn is_healthy(&self) -> bool {
        matches!(self, ComparisonResult::Passed)
    }
}
```

#### Deliverables

- Health monitor with metric collection (~450 LOC)
- Metrics aggregation and windowing (~300 LOC)
- SLO validation system (~200 LOC)
- Health check implementations (~250 LOC)
- Unit and integration tests (~350 LOC)

---

### Phase 4: Canary Deployment Engine (Days 7-10)

**Objective**: Implement progressive canary deployment with traffic splitting.

#### Tasks

1. **Canary Engine** (`canary/engine.rs`)
   - Main deployment orchestration
   - Stage progression logic
   - Traffic management integration
   - Event-driven architecture

2. **Stage Management** (`canary/stage.rs`)
   - Stage transition logic
   - Observation period enforcement
   - Stage-specific validation
   - Manual approval gates

3. **Traffic Splitting** (`canary/traffic.rs`)
   - Percentage-based routing
   - Sticky sessions support
   - Gradual traffic shift
   - Integration with load balancers

4. **Deployment Strategies** (`canary/strategy.rs`)
   - Canary strategy
   - Blue-green strategy
   - Shadow deployment
   - A/B testing strategy

5. **Stage Evaluator** (`canary/evaluator.rs`)
   - Health-based evaluation
   - Statistical significance testing
   - Comparison with baseline
   - Decision making

#### Code Example - Canary Engine

```rust
// canary/engine.rs
use crate::config::ConfigurationManager;
use crate::error::{ActuatorError, Result};
use crate::health::HealthMonitor;
use crate::types::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::{interval, sleep};
use tracing::{debug, info, warn, error};
use uuid::Uuid;

/// Canary deployment engine
pub struct CanaryEngine {
    /// Configuration manager
    config_manager: Arc<ConfigurationManager>,
    /// Health monitor
    health_monitor: Arc<HealthMonitor>,
    /// Traffic controller
    traffic_controller: Arc<dyn TrafficController>,
    /// Current deployment status
    current_deployment: Arc<RwLock<Option<DeploymentStatus>>>,
    /// Deployment configuration
    config: CanaryConfig,
}

/// Canary deployment configuration
#[derive(Debug, Clone)]
pub struct CanaryConfig {
    /// Enable automatic progression
    pub auto_progress: bool,
    /// Require manual approval for final stage
    pub require_final_approval: bool,
    /// Maximum deployment duration
    pub max_deployment_duration: Duration,
    /// Health check interval
    pub health_check_interval: Duration,
}

impl Default for CanaryConfig {
    fn default() -> Self {
        Self {
            auto_progress: true,
            require_final_approval: true,
            max_deployment_duration: Duration::from_secs(3600), // 1 hour
            health_check_interval: Duration::from_secs(10),
        }
    }
}

impl CanaryEngine {
    /// Create new canary engine
    pub fn new(
        config_manager: Arc<ConfigurationManager>,
        health_monitor: Arc<HealthMonitor>,
        traffic_controller: Arc<dyn TrafficController>,
        config: CanaryConfig,
    ) -> Self {
        Self {
            config_manager,
            health_monitor,
            traffic_controller,
            current_deployment: Arc::new(RwLock::new(None)),
            config,
        }
    }

    /// Start a new canary deployment
    pub async fn start_deployment(
        &self,
        config: DeploymentConfig,
    ) -> Result<Uuid> {
        // Check if deployment already in progress
        {
            let current = self.current_deployment.read().await;
            if let Some(ref deployment) = *current {
                if matches!(deployment.state, DeploymentState::InProgress) {
                    return Err(ActuatorError::DeploymentInProgress(
                        deployment.deployment_id,
                    ));
                }
            }
        }

        // Store configuration
        self.config_manager.store_config(config.clone()).await?;

        // Create deployment status
        let deployment_id = Uuid::new_v4();
        let status = DeploymentStatus {
            deployment_id,
            config,
            current_stage: CanaryStage::Validation,
            stage_started_at: Utc::now(),
            state: DeploymentState::InProgress,
            health_metrics: HealthMetrics::default(),
            history: Vec::new(),
        };

        // Store deployment status
        {
            let mut current = self.current_deployment.write().await;
            *current = Some(status.clone());
        }

        info!(
            deployment_id = %deployment_id,
            "Started canary deployment"
        );

        // Start deployment loop in background
        let engine = Arc::new(self.clone());
        tokio::spawn(async move {
            if let Err(e) = engine.run_deployment_loop().await {
                error!(error = %e, "Deployment loop failed");
            }
        });

        Ok(deployment_id)
    }

    /// Main deployment loop
    async fn run_deployment_loop(&self) -> Result<()> {
        let mut check_interval = interval(self.config.health_check_interval);

        loop {
            check_interval.tick().await;

            // Get current deployment
            let deployment = {
                let current = self.current_deployment.read().await;
                match current.as_ref() {
                    Some(d) if matches!(d.state, DeploymentState::InProgress) => {
                        d.clone()
                    }
                    _ => break, // No active deployment
                }
            };

            // Check if max duration exceeded
            let deployment_duration = Utc::now() - deployment.stage_started_at;
            if deployment_duration.num_seconds() as u64
                > self.config.max_deployment_duration.as_secs()
            {
                warn!("Deployment exceeded max duration, initiating rollback");
                self.initiate_rollback(RollbackReason::ManualTrigger {
                    triggered_by: "system".to_string(),
                    reason: "Max deployment duration exceeded".to_string(),
                })
                .await?;
                break;
            }

            // Process current stage
            match self.process_current_stage(&deployment).await {
                Ok(StageResult::Continue) => {
                    debug!("Stage in progress, continuing");
                }
                Ok(StageResult::ProgressToNext) => {
                    info!(
                        current_stage = ?deployment.current_stage,
                        "Progressing to next stage"
                    );
                    self.progress_to_next_stage(&deployment).await?;
                }
                Ok(StageResult::Complete) => {
                    info!("Deployment completed successfully");
                    self.complete_deployment(&deployment).await?;
                    break;
                }
                Err(e) => {
                    error!(error = %e, "Stage processing failed");
                    self.initiate_rollback(RollbackReason::ManualTrigger {
                        triggered_by: "system".to_string(),
                        reason: format!("Stage processing error: {}", e),
                    })
                    .await?;
                    break;
                }
            }
        }

        Ok(())
    }

    /// Process current deployment stage
    async fn process_current_stage(
        &self,
        deployment: &DeploymentStatus,
    ) -> Result<StageResult> {
        let stage = deployment.current_stage;

        // Check if minimum observation period has elapsed
        let stage_duration = Utc::now() - deployment.stage_started_at;
        let min_duration = stage.min_observation_duration();

        if stage_duration.num_seconds() as u64 < min_duration.as_secs() {
            debug!(
                stage = ?stage,
                elapsed = stage_duration.num_seconds(),
                required = min_duration.as_secs(),
                "Waiting for minimum observation period"
            );
            return Ok(StageResult::Continue);
        }

        // Evaluate health
        let health_eval = self.health_monitor.evaluate_health().await?;

        match health_eval {
            HealthEvaluation::Healthy { .. } => {
                info!(stage = ?stage, "Stage health check passed");

                // Check if this is the last stage
                if stage == CanaryStage::Stage50Percent {
                    if self.config.require_final_approval {
                        info!("Final stage requires manual approval");
                        self.pause_deployment(deployment).await?;
                        return Ok(StageResult::Continue);
                    } else {
                        return Ok(StageResult::ProgressToNext);
                    }
                }

                Ok(StageResult::ProgressToNext)
            }
            HealthEvaluation::Insufficient { .. } => {
                debug!("Insufficient data for health evaluation");
                Ok(StageResult::Continue)
            }
            HealthEvaluation::Degraded { reason, .. } => {
                warn!(stage = ?stage, reason = ?reason, "Canary degraded");
                // For degradation, continue monitoring but alert
                // Don't auto-rollback unless it becomes unhealthy
                Ok(StageResult::Continue)
            }
            HealthEvaluation::Unhealthy { reason, .. } => {
                error!(stage = ?stage, reason = ?reason, "Canary unhealthy");
                Err(ActuatorError::HealthCheckFailed(
                    format!("Health check failed: {:?}", reason),
                ))
            }
            HealthEvaluation::SloViolation { violations, .. } => {
                error!(stage = ?stage, violations = ?violations, "SLO violation");
                Err(ActuatorError::SloViolation(violations))
            }
        }
    }

    /// Progress to next deployment stage
    async fn progress_to_next_stage(
        &self,
        deployment: &DeploymentStatus,
    ) -> Result<()> {
        let next_stage = deployment.current_stage.next()
            .ok_or_else(|| ActuatorError::InvalidState(
                "No next stage available".to_string(),
            ))?;

        info!(
            from_stage = ?deployment.current_stage,
            to_stage = ?next_stage,
            "Transitioning to next stage"
        );

        // Update traffic split
        self.traffic_controller
            .set_canary_traffic(next_stage.traffic_percentage())
            .await?;

        // Reset health metrics for new stage
        self.health_monitor.reset_metrics().await;

        // Update deployment status
        {
            let mut current = self.current_deployment.write().await;
            if let Some(ref mut status) = *current {
                // Add current stage to history
                status.history.push(StageHistory {
                    stage: status.current_stage,
                    started_at: status.stage_started_at,
                    completed_at: Some(Utc::now()),
                    final_metrics: Some(status.health_metrics.clone()),
                    outcome: StageOutcome::Passed,
                });

                // Update to next stage
                status.current_stage = next_stage;
                status.stage_started_at = Utc::now();
            }
        }

        Ok(())
    }

    /// Complete deployment
    async fn complete_deployment(&self, deployment: &DeploymentStatus) -> Result<()> {
        info!(deployment_id = %deployment.deployment_id, "Completing deployment");

        // Activate configuration
        self.config_manager
            .activate_config(deployment.config.id)
            .await?;

        // Set traffic to 100%
        self.traffic_controller.set_canary_traffic(100.0).await?;

        // Update deployment status
        {
            let mut current = self.current_deployment.write().await;
            if let Some(ref mut status) = *current {
                status.state = DeploymentState::Succeeded;
                status.history.push(StageHistory {
                    stage: status.current_stage,
                    started_at: status.stage_started_at,
                    completed_at: Some(Utc::now()),
                    final_metrics: Some(status.health_metrics.clone()),
                    outcome: StageOutcome::Passed,
                });
            }
        }

        Ok(())
    }

    /// Pause deployment (for manual approval)
    async fn pause_deployment(&self, deployment: &DeploymentStatus) -> Result<()> {
        let mut current = self.current_deployment.write().await;
        if let Some(ref mut status) = *current {
            status.state = DeploymentState::Paused;
        }
        Ok(())
    }

    /// Resume paused deployment
    pub async fn resume_deployment(&self) -> Result<()> {
        let mut current = self.current_deployment.write().await;
        if let Some(ref mut status) = *current {
            if status.state != DeploymentState::Paused {
                return Err(ActuatorError::InvalidState(
                    "Deployment is not paused".to_string(),
                ));
            }
            status.state = DeploymentState::InProgress;
        }
        Ok(())
    }

    /// Initiate rollback
    pub async fn initiate_rollback(&self, reason: RollbackReason) -> Result<()> {
        error!(reason = ?reason, "Initiating rollback");

        // Update deployment state
        {
            let mut current = self.current_deployment.write().await;
            if let Some(ref mut status) = *current {
                status.state = DeploymentState::RollingBack;
            }
        }

        // Execute rollback (delegated to RollbackEngine)
        // This would be called from the RollbackEngine

        Ok(())
    }

    /// Get current deployment status
    pub async fn get_deployment_status(&self) -> Option<DeploymentStatus> {
        let current = self.current_deployment.read().await;
        current.clone()
    }
}

/// Stage processing result
enum StageResult {
    /// Continue observing current stage
    Continue,
    /// Progress to next stage
    ProgressToNext,
    /// Deployment complete
    Complete,
}

/// Traffic controller trait
#[async_trait::async_trait]
pub trait TrafficController: Send + Sync {
    /// Set canary traffic percentage
    async fn set_canary_traffic(&self, percentage: f64) -> Result<()>;

    /// Get current traffic split
    async fn get_traffic_split(&self) -> Result<TrafficSplit>;
}

/// Traffic split information
#[derive(Debug, Clone)]
pub struct TrafficSplit {
    pub control_percentage: f64,
    pub canary_percentage: f64,
}
```

#### Deliverables

- Canary deployment engine (~600 LOC)
- Stage management system (~300 LOC)
- Traffic splitting implementation (~350 LOC)
- Deployment strategies (~400 LOC)
- Stage evaluator (~300 LOC)
- Integration tests (~400 LOC)

---

### Phase 5: Rollback Engine (Days 11-12)

**Objective**: Implement automatic and manual rollback capabilities.

#### Tasks

1. **Rollback Engine** (`rollback/engine.rs`)
   - Automatic rollback triggering
   - Manual rollback support
   - State restoration
   - Rollback verification

2. **Trigger Conditions** (`rollback/trigger.rs`)
   - Health-based triggers
   - SLO violation triggers
   - Manual triggers
   - Circuit breaker integration

3. **State Recovery** (`rollback/recovery.rs`)
   - Configuration restoration
   - Traffic restoration
   - Cleanup procedures
   - Post-rollback verification

#### Code Example - Rollback Engine

```rust
// rollback/engine.rs
use crate::config::ConfigurationManager;
use crate::error::{ActuatorError, Result};
use crate::types::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn, error};

/// Rollback engine
pub struct RollbackEngine {
    /// Configuration manager
    config_manager: Arc<ConfigurationManager>,
    /// Traffic controller
    traffic_controller: Arc<dyn TrafficController>,
    /// Health monitor
    health_monitor: Arc<HealthMonitor>,
    /// Rollback configuration
    config: RollbackConfig,
}

/// Rollback configuration
#[derive(Debug, Clone)]
pub struct RollbackConfig {
    /// Maximum rollback duration
    pub max_rollback_duration: Duration,
    /// Verification wait time after rollback
    pub verification_wait_time: Duration,
    /// Enable automatic rollback
    pub enable_auto_rollback: bool,
}

impl Default for RollbackConfig {
    fn default() -> Self {
        Self {
            max_rollback_duration: Duration::from_secs(60),
            verification_wait_time: Duration::from_secs(30),
            enable_auto_rollback: true,
        }
    }
}

impl RollbackEngine {
    /// Create new rollback engine
    pub fn new(
        config_manager: Arc<ConfigurationManager>,
        traffic_controller: Arc<dyn TrafficController>,
        health_monitor: Arc<HealthMonitor>,
        config: RollbackConfig,
    ) -> Self {
        Self {
            config_manager,
            traffic_controller,
            health_monitor,
            config,
        }
    }

    /// Execute rollback
    pub async fn execute_rollback(
        &self,
        deployment_id: Uuid,
        reason: RollbackReason,
    ) -> Result<RollbackResult> {
        info!(
            deployment_id = %deployment_id,
            reason = ?reason,
            "Executing rollback"
        );

        let start_time = std::time::Instant::now();

        // Step 1: Immediately shift traffic back to control
        info!("Step 1: Redirecting traffic to control group");
        self.traffic_controller.set_canary_traffic(0.0).await?;

        // Step 2: Restore previous configuration
        info!("Step 2: Restoring previous configuration");
        self.config_manager.rollback_to_previous().await?;

        // Step 3: Wait for stabilization
        info!("Step 3: Waiting for stabilization");
        sleep(self.config.verification_wait_time).await;

        // Step 4: Verify health after rollback
        info!("Step 4: Verifying health after rollback");
        let health_eval = self.health_monitor.evaluate_health().await?;

        let rollback_duration = start_time.elapsed();

        if health_eval.is_healthy() {
            info!(
                duration_ms = rollback_duration.as_millis(),
                "Rollback completed successfully"
            );

            Ok(RollbackResult {
                success: true,
                duration: rollback_duration,
                reason,
                verification: Some(health_eval),
            })
        } else {
            error!("Rollback verification failed");
            Err(ActuatorError::RollbackFailed(
                "Health check failed after rollback".to_string(),
            ))
        }
    }

    /// Check if automatic rollback should be triggered
    pub async fn should_trigger_rollback(
        &self,
        health_eval: &HealthEvaluation,
    ) -> Option<RollbackReason> {
        if !self.config.enable_auto_rollback {
            return None;
        }

        match health_eval {
            HealthEvaluation::Unhealthy { reason, .. } => {
                Some(RollbackReason::HealthCheckFailure {
                    check_name: "threshold_check".to_string(),
                    error: format!("{:?}", reason),
                })
            }
            HealthEvaluation::SloViolation { violations, .. } => {
                Some(RollbackReason::SloViolation {
                    slo_name: "deployment_slo".to_string(),
                    details: violations.join(", "),
                })
            }
            _ => None,
        }
    }
}

/// Rollback result
#[derive(Debug, Clone)]
pub struct RollbackResult {
    /// Success status
    pub success: bool,
    /// Rollback duration
    pub duration: Duration,
    /// Rollback reason
    pub reason: RollbackReason,
    /// Post-rollback health verification
    pub verification: Option<HealthEvaluation>,
}
```

#### Deliverables

- Rollback engine with automatic triggering (~400 LOC)
- Trigger condition system (~250 LOC)
- State recovery implementation (~300 LOC)
- Rollback tests (~300 LOC)

---

### Phase 6: Integration and Testing (Days 13-14)

**Objective**: Integrate all components and comprehensive testing.

#### Tasks

1. **Component Integration**
   - Wire all subsystems together
   - Create main `ActuatorEngine` facade
   - Add telemetry and observability
   - Documentation and examples

2. **Testing**
   - Unit tests for all modules
   - Integration tests for end-to-end flows
   - Chaos testing (simulated failures)
   - Performance benchmarks

3. **Examples and Documentation**
   - Basic usage examples
   - Advanced scenarios
   - API documentation
   - Runbooks and troubleshooting

#### Code Example - Main Actuator Engine

```rust
// lib.rs
//! Actuator Engine for LLM Auto-Optimizer
//!
//! This crate provides safe configuration deployment with canary rollouts
//! and automatic rollback capabilities.

pub mod error;
pub mod types;
pub mod traits;

pub mod config;
pub mod health;
pub mod canary;
pub mod rollback;
pub mod integration;

pub use error::{ActuatorError, Result};
pub use types::*;
pub use traits::*;

pub use config::ConfigurationManager;
pub use health::HealthMonitor;
pub use canary::CanaryEngine;
pub use rollback::RollbackEngine;

use std::sync::Arc;

/// Main actuator engine facade
pub struct ActuatorEngine {
    config_manager: Arc<ConfigurationManager>,
    health_monitor: Arc<HealthMonitor>,
    canary_engine: Arc<CanaryEngine>,
    rollback_engine: Arc<RollbackEngine>,
}

impl ActuatorEngine {
    /// Create new actuator engine
    pub async fn new(config: ActuatorConfig) -> Result<Self> {
        // Initialize storage backend
        let storage = config.create_storage_backend()?;
        let audit_logger = config.create_audit_logger()?;

        // Initialize configuration manager
        let config_manager = Arc::new(ConfigurationManager::new(
            storage,
            audit_logger,
        ));

        // Initialize health monitor
        let slo_validator = config.create_slo_validator()?;
        let health_monitor = Arc::new(HealthMonitor::new(
            config.health_config,
            slo_validator,
        ));

        // Initialize traffic controller
        let traffic_controller = config.create_traffic_controller()?;

        // Initialize canary engine
        let canary_engine = Arc::new(CanaryEngine::new(
            Arc::clone(&config_manager),
            Arc::clone(&health_monitor),
            Arc::clone(&traffic_controller),
            config.canary_config,
        ));

        // Initialize rollback engine
        let rollback_engine = Arc::new(RollbackEngine::new(
            Arc::clone(&config_manager),
            traffic_controller,
            Arc::clone(&health_monitor),
            config.rollback_config,
        ));

        Ok(Self {
            config_manager,
            health_monitor,
            canary_engine,
            rollback_engine,
        })
    }

    /// Deploy a new configuration with canary rollout
    pub async fn deploy(
        &self,
        config: DeploymentConfig,
    ) -> Result<Uuid> {
        self.canary_engine.start_deployment(config).await
    }

    /// Get current deployment status
    pub async fn get_status(&self) -> Option<DeploymentStatus> {
        self.canary_engine.get_deployment_status().await
    }

    /// Manually rollback current deployment
    pub async fn rollback(&self, reason: String) -> Result<()> {
        let status = self.get_status().await
            .ok_or_else(|| ActuatorError::NoActiveDeployment)?;

        self.rollback_engine
            .execute_rollback(
                status.deployment_id,
                RollbackReason::ManualTrigger {
                    triggered_by: "user".to_string(),
                    reason,
                },
            )
            .await?;

        Ok(())
    }

    /// Resume a paused deployment
    pub async fn resume(&self) -> Result<()> {
        self.canary_engine.resume_deployment().await
    }
}

/// Actuator configuration
#[derive(Debug, Clone)]
pub struct ActuatorConfig {
    pub health_config: HealthMonitorConfig,
    pub canary_config: CanaryConfig,
    pub rollback_config: RollbackConfig,
    pub storage_backend: StorageBackendType,
    pub redis_url: Option<String>,
    pub postgres_url: Option<String>,
}

/// Storage backend type
#[derive(Debug, Clone)]
pub enum StorageBackendType {
    Memory,
    Redis,
    Postgres,
    Sled,
}
```

#### Deliverables

- Main `ActuatorEngine` facade (~150 LOC)
- Comprehensive integration tests (~500 LOC)
- Examples demonstrating key use cases (~400 LOC)
- API documentation and user guide

---

## 5. Module Interfaces

### 5.1 ConfigurationManager

```rust
pub trait ConfigurationStore: Send + Sync {
    async fn store(&self, config: &DeploymentConfig) -> Result<()>;
    async fn get(&self, id: Uuid) -> Result<DeploymentConfig>;
    async fn mark_active(&self, id: Uuid) -> Result<()>;
    async fn get_active(&self) -> Result<Option<DeploymentConfig>>;
    async fn list(&self, limit: usize) -> Result<Vec<DeploymentConfig>>;
}

pub trait AuditLogger: Send + Sync {
    async fn log_config_stored(&self, config: &DeploymentConfig) -> Result<()>;
    async fn log_config_activated(&self, config: &DeploymentConfig, prev: Option<&DeploymentConfig>) -> Result<()>;
    async fn log_rollback(&self, reason: &RollbackReason) -> Result<()>;
}
```

### 5.2 HealthMonitor

```rust
pub trait MetricsProvider: Send + Sync {
    async fn get_metrics(&self, group: MetricsGroup) -> Result<Vec<RequestMetric>>;
    async fn subscribe_metrics(&self) -> Result<mpsc::Receiver<RequestMetric>>;
}

pub trait SloValidator: Send + Sync {
    async fn validate(&self, metrics: &HealthMetrics) -> Result<SloValidationResult>;
    async fn add_slo(&self, slo: SloDefinition) -> Result<()>;
}
```

### 5.3 CanaryEngine

```rust
#[async_trait::async_trait]
pub trait DeploymentStrategy: Send + Sync {
    async fn execute(&self, config: &DeploymentConfig) -> Result<()>;
    fn strategy_type(&self) -> StrategyType;
}

#[async_trait::async_trait]
pub trait TrafficController: Send + Sync {
    async fn set_canary_traffic(&self, percentage: f64) -> Result<()>;
    async fn get_traffic_split(&self) -> Result<TrafficSplit>;
}
```

### 5.4 RollbackEngine

```rust
pub trait RollbackTrigger: Send + Sync {
    fn should_trigger(&self, health: &HealthEvaluation) -> Option<RollbackReason>;
    fn trigger_type(&self) -> TriggerType;
}
```

---

## 6. Testing Strategy

### 6.1 Unit Tests

**Coverage Target**: >85%

- All public APIs tested
- Edge cases and error conditions
- Type conversions and validations
- Mock-based testing for external dependencies

### 6.2 Integration Tests

**Key Scenarios**:

1. **Successful Canary Deployment**
   - Progressive rollout through all stages
   - Health checks passing at each stage
   - Final configuration activation

2. **Automatic Rollback**
   - Trigger rollback on high error rate
   - Trigger rollback on latency degradation
   - Trigger rollback on SLO violation

3. **Manual Operations**
   - Manual rollback trigger
   - Pause and resume deployment
   - Manual approval for final stage

4. **Configuration Management**
   - Store and retrieve configurations
   - Version tracking and history
   - Configuration diff generation

5. **Traffic Splitting**
   - Gradual traffic percentage changes
   - Traffic distribution verification
   - Sticky session support

### 6.3 Chaos Testing

**Failure Scenarios**:

1. Health check timeout
2. Storage backend failure
3. Traffic controller failure
4. Network partitions
5. Concurrent deployment attempts

### 6.4 Performance Testing

**Benchmarks**:

- Configuration propagation latency
- Health check overhead
- Traffic split performance
- Rollback time
- Memory usage under load

---

## 7. Lines of Code Estimates

| Module | LOC | Tests | Examples | Total |
|--------|-----|-------|----------|-------|
| **Core Types** | 300 | 150 | 0 | 450 |
| **Error Handling** | 200 | 50 | 0 | 250 |
| **Traits** | 150 | 0 | 0 | 150 |
| **Config Manager** | 1,250 | 300 | 50 | 1,600 |
| **Health Monitor** | 1,250 | 350 | 50 | 1,650 |
| **Canary Engine** | 2,000 | 400 | 150 | 2,550 |
| **Rollback Engine** | 950 | 300 | 100 | 1,350 |
| **Integration** | 750 | 0 | 0 | 750 |
| **Main Facade** | 150 | 500 | 100 | 750 |
| **Total** | **7,000** | **2,050** | **450** | **9,500** |

**Note**: Estimates include comments and documentation.

---

## 8. Dependencies

### 8.1 Required Crates

Update `/workspaces/llm-auto-optimizer/crates/actuator/Cargo.toml`:

```toml
[package]
name = "actuator"
version.workspace = true
edition.workspace = true

[dependencies]
# Core async runtime
tokio = { version = "1.35", features = ["full"] }
async-trait = "0.1"

# Error handling
thiserror = "1.0"
anyhow = "1.0"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
uuid = { version = "1.6", features = ["v4", "serde"] }

# Time handling
chrono = { version = "0.4", features = ["serde"] }

# Logging and tracing
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Storage backends
redis = { version = "0.24", optional = true, features = ["tokio-comp", "connection-manager"] }
sqlx = { version = "0.7", optional = true, features = ["runtime-tokio-rustls", "postgres", "chrono", "uuid"] }
sled = { version = "0.34", optional = true }

# Internal dependencies
config = { path = "../config" }
types = { path = "../types" }

[dev-dependencies]
tokio-test = "0.4"
mockall = "0.12"
proptest = "1.4"

[features]
default = ["redis-backend"]
redis-backend = ["redis"]
postgres-backend = ["sqlx"]
sled-backend = ["sled"]
```

### 8.2 Internal Dependencies

- `config`: Configuration management
- `types`: Common data types
- `decision`: Integration with Decision Engine
- `processor`: Metrics collection integration

### 8.3 External Service Dependencies

- **Redis**: Configuration caching and distributed locking
- **PostgreSQL**: Audit log and configuration history
- **OpenTelemetry**: Metrics and observability
- **Load Balancer** (e.g., Envoy, HAProxy): Traffic splitting

---

## 9. Risk Mitigation

### 9.1 Technical Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| **Traffic split failures** | Medium | High | Implement circuit breaker, fallback to control group |
| **Health check false positives** | Medium | High | Multiple validation stages, statistical significance testing |
| **Storage backend failures** | Low | High | Multi-backend support, degraded mode operation |
| **Rollback failures** | Low | Critical | Pre-validate rollback path, keep multiple versions |
| **Race conditions** | Medium | Medium | Atomic operations, proper locking, state machines |

### 9.2 Operational Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| **Stuck deployments** | Medium | Medium | Timeout enforcement, manual override capabilities |
| **Cascading failures** | Low | Critical | Circuit breakers, rate limiting, isolation |
| **Data inconsistency** | Low | High | ACID transactions, audit logging, reconciliation |
| **Monitoring blind spots** | Medium | High | Comprehensive observability, alerting, dashboards |

### 9.3 Safety Mechanisms

1. **Circuit Breakers**: Prevent cascading failures
2. **Rate Limiting**: Control deployment frequency
3. **Manual Override**: Emergency stop and rollback
4. **Dry Run Mode**: Test deployments without traffic impact
5. **Shadow Deployments**: Validate before production traffic

---

## 10. Validation Criteria

### 10.1 Functional Requirements

- [ ] Successfully deploy configuration through all canary stages
- [ ] Automatic rollback on health check failure
- [ ] Manual rollback execution
- [ ] Configuration versioning and history
- [ ] Audit logging of all changes
- [ ] Traffic splitting with configurable percentages
- [ ] SLO validation and alerting
- [ ] Pause and resume deployments

### 10.2 Non-Functional Requirements

| Requirement | Target | Measurement |
|-------------|--------|-------------|
| **Rollback Time** | <30s (p95) | Time from detection to complete rollback |
| **Config Propagation** | <5s (p99) | Time to propagate to all instances |
| **Health Check Overhead** | <2% CPU | Additional CPU usage for monitoring |
| **Deployment Success Rate** | >99.5% | Deployments completing without rollback |
| **Zero Downtime** | 100% | No service interruptions |
| **Memory Usage** | <100MB | Memory footprint in idle state |

### 10.3 Test Coverage

- [ ] Unit test coverage >85%
- [ ] Integration tests for all major flows
- [ ] Chaos tests for failure scenarios
- [ ] Performance benchmarks established
- [ ] Documentation complete and reviewed

---

## 11. Deployment Checklist

### 11.1 Pre-Deployment

- [ ] All unit tests passing
- [ ] All integration tests passing
- [ ] Code review completed
- [ ] Documentation updated
- [ ] Configuration examples provided
- [ ] Runbooks created
- [ ] Monitoring dashboards created
- [ ] Alerting rules configured

### 11.2 Deployment Steps

1. **Environment Setup**
   - [ ] Redis cluster available
   - [ ] PostgreSQL database created
   - [ ] Network connectivity verified
   - [ ] Access credentials configured

2. **Service Deployment**
   - [ ] Deploy actuator service
   - [ ] Verify health endpoints
   - [ ] Test configuration storage
   - [ ] Validate traffic controller integration

3. **Smoke Testing**
   - [ ] Test canary deployment (dry run)
   - [ ] Test manual rollback
   - [ ] Verify health monitoring
   - [ ] Check audit logs

4. **Production Rollout**
   - [ ] Deploy to staging environment
   - [ ] Run comprehensive tests
   - [ ] Deploy to production (canary)
   - [ ] Monitor for 24 hours
   - [ ] Complete production rollout

### 11.3 Post-Deployment

- [ ] Monitor error rates
- [ ] Review audit logs
- [ ] Check performance metrics
- [ ] Validate alerting
- [ ] Conduct post-mortem review
- [ ] Update documentation with learnings

---

## 12. Integration Points

### 12.1 Decision Engine Integration

```rust
// integration/decision.rs
use decision::{DecisionEngine, OptimizationDecision};

pub struct DecisionEngineClient {
    engine: Arc<DecisionEngine>,
}

impl DecisionEngineClient {
    pub async fn get_recommended_config(&self) -> Result<DeploymentConfig> {
        let decision = self.engine.get_optimization_decision().await?;
        Ok(self.convert_decision_to_config(decision))
    }
}
```

### 12.2 LLM Provider Integration

```rust
// integration/provider.rs
pub trait LlmProvider: Send + Sync {
    async fn apply_config(&self, config: &DeploymentConfig) -> Result<()>;
    async fn get_current_config(&self) -> Result<DeploymentConfig>;
}

pub struct OpenAiProvider {
    client: OpenAiClient,
}

pub struct AnthropicProvider {
    client: AnthropicClient,
}
```

### 12.3 Observability Integration

```rust
// integration/telemetry.rs
use opentelemetry::metrics::Counter;

pub struct ActuatorMetrics {
    deployments_started: Counter<u64>,
    deployments_succeeded: Counter<u64>,
    deployments_failed: Counter<u64>,
    rollbacks_triggered: Counter<u64>,
}
```

---

## 13. Code Scaffolding Examples

### 13.1 Basic Usage Example

```rust
// examples/basic_canary.rs
use actuator::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize actuator
    let config = ActuatorConfig {
        health_config: HealthMonitorConfig::default(),
        canary_config: CanaryConfig::default(),
        rollback_config: RollbackConfig::default(),
        storage_backend: StorageBackendType::Redis,
        redis_url: Some("redis://localhost:6379".to_string()),
        postgres_url: Some("postgresql://localhost/actuator".to_string()),
    };

    let actuator = ActuatorEngine::new(config).await?;

    // Create deployment configuration
    let deployment_config = DeploymentConfig {
        id: Uuid::new_v4(),
        version: 1,
        model: "gpt-4".to_string(),
        parameters: HashMap::from([
            ("temperature".to_string(), json!(0.7)),
            ("max_tokens".to_string(), json!(1000)),
        ]),
        prompt_template: Some("You are a helpful assistant...".to_string()),
        metadata: HashMap::new(),
        created_at: Utc::now(),
    };

    // Start deployment
    let deployment_id = actuator.deploy(deployment_config).await?;
    println!("Deployment started: {}", deployment_id);

    // Monitor deployment
    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;

        if let Some(status) = actuator.get_status().await {
            println!("Stage: {:?}, State: {:?}",
                status.current_stage, status.state);

            if matches!(status.state, DeploymentState::Succeeded | DeploymentState::Failed) {
                break;
            }
        }
    }

    Ok(())
}
```

### 13.2 Rollback Example

```rust
// examples/rollback_demo.rs
use actuator::*;

#[tokio::main]
async fn main() -> Result<()> {
    let actuator = ActuatorEngine::new(ActuatorConfig::default()).await?;

    // Start deployment
    let deployment_id = actuator.deploy(create_config()).await?;

    // Simulate waiting
    tokio::time::sleep(Duration::from_secs(30)).await;

    // Manually trigger rollback
    actuator.rollback("Performance degradation detected".to_string()).await?;

    println!("Rollback completed");

    Ok(())
}
```

### 13.3 Custom Health Check Example

```rust
// examples/custom_health_check.rs
use actuator::health::*;

struct CustomHealthChecker {
    threshold: f64,
}

#[async_trait::async_trait]
impl HealthChecker for CustomHealthChecker {
    async fn check(&self, metrics: &HealthMetrics) -> Result<HealthStatus> {
        if metrics.success_rate < self.threshold {
            Ok(HealthStatus::Unhealthy)
        } else {
            Ok(HealthStatus::Healthy)
        }
    }
}
```

---

## Conclusion

This implementation plan provides a comprehensive roadmap for building the Actuator Engine with enterprise-grade canary deployments and rollback capabilities. The phased approach ensures systematic development with clear milestones and validation criteria at each stage.

### Key Success Factors

1. **Incremental Development**: Build foundational layers first
2. **Comprehensive Testing**: Test each component thoroughly before integration
3. **Safety Mechanisms**: Multiple layers of protection against failures
4. **Observability**: Rich metrics and logging for debugging and monitoring
5. **Documentation**: Clear examples and runbooks for operations

### Next Steps

1. Review and approve this implementation plan
2. Set up development environment and dependencies
3. Begin Phase 1 implementation (Core Framework)
4. Conduct weekly progress reviews
5. Adjust timeline based on learnings and feedback

---

**Document Version**: 1.0
**Last Updated**: 2025-11-10
**Author**: LLM Auto-Optimizer Team
**Status**: Ready for Implementation
