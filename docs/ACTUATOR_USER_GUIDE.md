# Actuator User Guide: Safe Deployment with Canary Rollouts and Automatic Rollback

**Version:** 1.0
**Last Updated:** November 2025
**Target Audience:** Platform Operators, DevOps Engineers, Site Reliability Engineers

---

## Table of Contents

1. [Quick Start Guide](#1-quick-start-guide)
2. [Overview of the Actuator](#2-overview-of-the-actuator)
3. [Canary Deployment Guide](#3-canary-deployment-guide)
4. [Rollback Guide](#4-rollback-guide)
5. [Configuration Management](#5-configuration-management)
6. [Health Monitoring Setup](#6-health-monitoring-setup)
7. [Integration with Decision Engine](#7-integration-with-decision-engine)
8. [Usage Examples](#8-usage-examples)
9. [Best Practices](#9-best-practices)
10. [Troubleshooting](#10-troubleshooting)
11. [Production Operations Guide](#11-production-operations-guide)

---

## 1. Quick Start Guide

### 1.1 Prerequisites

Before using the Actuator, ensure you have:

- **LLM Auto Optimizer** installed and configured
- **PostgreSQL 15+** or SQLite for state persistence
- **Redis** (optional, for distributed deployments)
- **Monitoring infrastructure** (Prometheus + Grafana recommended)
- **Sufficient permissions** to modify LLM configurations

### 1.2 5-Minute Setup

```bash
# 1. Verify installation
cargo build --release -p actuator

# 2. Configure actuator settings
cp config.example.yaml config.yaml
# Edit the actuator section in config.yaml

# 3. Initialize the actuator database
./target/release/optimizer migrate --component actuator

# 4. Test connectivity
./target/release/optimizer actuator health-check

# 5. Deploy your first canary
./target/release/optimizer actuator deploy \
  --config new-config.yaml \
  --strategy canary \
  --initial-percentage 5
```

### 1.3 Verify Deployment

```bash
# Check deployment status
./target/release/optimizer actuator status

# Monitor canary metrics
./target/release/optimizer actuator metrics --deployment-id <id>

# View rollback history
./target/release/optimizer actuator rollback-history
```

---

## 2. Overview of the Actuator

### 2.1 What is the Actuator?

The **Actuator** is the execution component of the LLM Auto Optimizer responsible for safely deploying configuration changes to your LLM infrastructure. It acts as the bridge between optimization decisions and production systems, ensuring changes are applied gradually with built-in safety mechanisms.

### 2.2 Core Capabilities

| Capability | Description | Benefit |
|------------|-------------|---------|
| **Canary Deployments** | Progressive rollout with traffic splitting | Minimize blast radius of changes |
| **Automatic Rollback** | Detect and revert bad deployments | Prevent production incidents |
| **Health Monitoring** | Continuous validation during rollout | Early detection of issues |
| **Version Control** | Track all configuration changes | Audit trail and compliance |
| **Multi-Strategy Deployment** | Blue-green, canary, rolling updates | Flexibility for different scenarios |
| **Safety Constraints** | Enforce quality, cost, latency bounds | Prevent unsafe optimizations |

### 2.3 Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                     Actuator Engine                         │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐ │
│  │   Decision   │───▶│  Deployment  │───▶│    Health    │ │
│  │   Receiver   │    │   Planner    │    │   Monitor    │ │
│  └──────────────┘    └──────────────┘    └──────────────┘ │
│         │                    │                    │         │
│         ▼                    ▼                    ▼         │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐ │
│  │   Safety     │    │   Traffic    │    │   Rollback   │ │
│  │  Validator   │    │   Splitter   │    │   Manager    │ │
│  └──────────────┘    └──────────────┘    └──────────────┘ │
│         │                    │                    │         │
│         └────────────────────┴────────────────────┘         │
│                              │                              │
│                              ▼                              │
│                    ┌──────────────────┐                     │
│                    │   Configuration  │                     │
│                    │     Updater      │                     │
│                    └──────────────────┘                     │
│                              │                              │
└──────────────────────────────┼──────────────────────────────┘
                               │
                               ▼
                    ┌──────────────────┐
                    │  LLM Services    │
                    │  (Orchestrator)  │
                    └──────────────────┘
```

### 2.4 Deployment Lifecycle

```
Request → Validate → Plan → Execute → Monitor → Complete/Rollback
    │         │        │        │          │            │
    │         │        │        │          │            └─▶ Finalize
    │         │        │        │          └─────────────▶ Assess
    │         │        │        └────────────────────────▶ Deploy
    │         │        └─────────────────────────────────▶ Schedule
    │         └──────────────────────────────────────────▶ Check Safety
    └────────────────────────────────────────────────────▶ Parse Request
```

---

## 3. Canary Deployment Guide

### 3.1 What is a Canary Deployment?

A **canary deployment** progressively rolls out changes to a small subset of traffic first, monitors performance, and gradually increases traffic if metrics remain healthy. This approach minimizes risk by limiting the impact of potential issues.

### 3.2 Canary Deployment Phases

#### Phase 1: Initialization (0-5%)
- Deploy to minimal traffic (typically 5%)
- Establish baseline metrics
- Duration: 5-15 minutes

#### Phase 2: Early Validation (5-25%)
- Increase to 25% if Phase 1 succeeds
- Monitor for early warning signs
- Duration: 15-30 minutes

#### Phase 3: Majority Rollout (25-75%)
- Increase to 75% if Phase 2 succeeds
- Validate at scale
- Duration: 30-60 minutes

#### Phase 4: Full Deployment (75-100%)
- Complete rollout to 100%
- Final validation
- Duration: 15-30 minutes

#### Phase 5: Stabilization
- Monitor for 24 hours
- Keep previous version for quick rollback

### 3.3 Configuring Canary Deployments

#### Basic Configuration

```yaml
# config.yaml - Actuator section
actuator:
  # Deployment strategy
  default_strategy: "canary"

  # Canary configuration
  canary:
    # Initial traffic percentage
    initial_percentage: 5

    # Rollout steps (percentage, duration)
    steps:
      - percentage: 5
        duration_minutes: 15

      - percentage: 25
        duration_minutes: 30

      - percentage: 50
        duration_minutes: 30

      - percentage: 75
        duration_minutes: 30

      - percentage: 100
        duration_minutes: 15

    # Success criteria for each step
    success_criteria:
      # Maximum error rate increase
      max_error_rate_increase: 0.02  # 2%

      # Maximum latency increase
      max_latency_p95_increase: 0.15  # 15%

      # Maximum cost increase
      max_cost_increase: 0.10  # 10%

      # Minimum quality score
      min_quality_score: 0.7

      # Statistical confidence level
      confidence_level: 0.95  # 95%

      # Minimum sample size per step
      min_sample_size: 1000

    # Automatic rollback triggers
    auto_rollback:
      enabled: true

      # Critical thresholds for immediate rollback
      critical_error_rate: 0.10  # 10% errors
      critical_latency_p95_ms: 10000  # 10 seconds
      critical_quality_drop: 0.20  # 20% drop

      # Warning thresholds (pause and alert)
      warning_error_rate: 0.05  # 5% errors
      warning_latency_increase: 0.25  # 25% increase

    # Progressive timeout
    max_total_duration_minutes: 180  # 3 hours
```

#### Advanced Configuration

```yaml
actuator:
  canary:
    # Traffic splitting strategy
    traffic_split:
      method: "weighted_random"  # or "consistent_hash", "round_robin"

      # For consistent hashing
      hash_key: "user_id"  # or "session_id", "request_id"

      # For sticky sessions
      sticky_sessions: true
      sticky_duration_minutes: 60

    # A/B test integration
    ab_test_mode: true
    control_group_size: 0.05  # 5% control group

    # Advanced monitoring
    monitoring:
      # Metrics to track
      metrics:
        - "error_rate"
        - "latency_p50"
        - "latency_p95"
        - "latency_p99"
        - "cost_per_request"
        - "tokens_per_request"
        - "quality_score"

      # Comparison window
      baseline_window_minutes: 60
      comparison_window_minutes: 15

      # Anomaly detection
      enable_anomaly_detection: true
      anomaly_sensitivity: "medium"  # low, medium, high

    # Gradual rollout strategies
    rollout_strategy:
      # Linear: equal steps
      type: "linear"

      # Exponential: start slow, accelerate
      # type: "exponential"
      # base: 2

      # Custom: define your own steps
      # type: "custom"
      # steps: [5, 10, 25, 50, 75, 90, 100]
```

### 3.4 Traffic Splitting Strategies

#### Weighted Random

Best for: General use, stateless requests

```yaml
traffic_split:
  method: "weighted_random"
```

**Behavior:**
- Each request randomly assigned to canary or stable
- Probability based on canary percentage
- No user stickiness

**Pros:**
- Simple and predictable
- Even distribution
- No session affinity needed

**Cons:**
- Same user may see different versions
- Not suitable for stateful operations

#### Consistent Hashing

Best for: Stateful operations, user consistency

```yaml
traffic_split:
  method: "consistent_hash"
  hash_key: "user_id"
  sticky_sessions: true
  sticky_duration_minutes: 60
```

**Behavior:**
- Users consistently routed to same version
- Hash based on specified key (user_id, session_id, etc.)
- Maintains user experience consistency

**Pros:**
- Consistent user experience
- Good for stateful workflows
- Reduces confusion

**Cons:**
- May create imbalanced distribution
- Requires identifier in request

#### Round Robin

Best for: Load testing, uniform distribution

```yaml
traffic_split:
  method: "round_robin"
```

**Behavior:**
- Requests alternately routed between versions
- Cycle based on canary percentage
- Deterministic distribution

**Pros:**
- Perfectly balanced load
- Predictable behavior
- Good for benchmarking

**Cons:**
- No user consistency
- Sequential patterns may bias results

### 3.5 Rollout Phases and Timing

#### Conservative Rollout (Low Risk Tolerance)

```yaml
canary:
  steps:
    - percentage: 1
      duration_minutes: 30
    - percentage: 5
      duration_minutes: 30
    - percentage: 10
      duration_minutes: 30
    - percentage: 25
      duration_minutes: 60
    - percentage: 50
      duration_minutes: 60
    - percentage: 75
      duration_minutes: 60
    - percentage: 100
      duration_minutes: 30

  max_total_duration_minutes: 360  # 6 hours
```

**Use when:**
- Deploying to critical production systems
- Making significant model changes
- Low confidence in optimization
- First-time deployment of new strategy

#### Standard Rollout (Balanced)

```yaml
canary:
  steps:
    - percentage: 5
      duration_minutes: 15
    - percentage: 25
      duration_minutes: 30
    - percentage: 50
      duration_minutes: 30
    - percentage: 75
      duration_minutes: 30
    - percentage: 100
      duration_minutes: 15

  max_total_duration_minutes: 180  # 3 hours
```

**Use when:**
- Normal optimization deployments
- Medium confidence in changes
- Established deployment pattern
- Standard business hours deployment

#### Aggressive Rollout (High Confidence)

```yaml
canary:
  steps:
    - percentage: 10
      duration_minutes: 10
    - percentage: 50
      duration_minutes: 20
    - percentage: 100
      duration_minutes: 10

  max_total_duration_minutes: 60  # 1 hour
```

**Use when:**
- Minor parameter adjustments
- High confidence from A/B testing
- Off-peak hours
- Emergency optimizations (cost spike)

### 3.6 Success Criteria Configuration

#### Defining Success Metrics

```yaml
success_criteria:
  # Error rate (0.0 - 1.0)
  max_error_rate_increase: 0.02  # 2% increase allowed
  absolute_max_error_rate: 0.05  # 5% absolute maximum

  # Latency (relative increase)
  max_latency_p50_increase: 0.10  # 10% p50 increase
  max_latency_p95_increase: 0.15  # 15% p95 increase
  max_latency_p99_increase: 0.20  # 20% p99 increase

  # Latency (absolute thresholds in milliseconds)
  absolute_max_latency_p95: 5000  # 5 seconds
  absolute_max_latency_p99: 10000  # 10 seconds

  # Cost (relative increase)
  max_cost_increase: 0.10  # 10% cost increase allowed

  # Quality (minimum acceptable)
  min_quality_score: 0.7  # 0.0 - 1.0 scale
  max_quality_degradation: 0.05  # 5% degradation

  # Statistical requirements
  confidence_level: 0.95  # 95% confidence
  min_sample_size: 1000  # Minimum requests per step

  # Custom metrics
  custom_metrics:
    - name: "user_satisfaction"
      min_value: 0.8

    - name: "token_efficiency"
      min_value: 0.9
```

#### Statistical Validation

The Actuator uses statistical tests to determine if canary metrics significantly differ from baseline:

```yaml
statistical_validation:
  # Method for comparing distributions
  test_method: "welch_t_test"  # or "mann_whitney", "bootstrap"

  # Significance level (alpha)
  alpha: 0.05  # 5% significance level (95% confidence)

  # Multiple comparison correction
  correction: "bonferroni"  # or "holm", "none"

  # Minimum effect size
  min_effect_size: 0.1  # Cohen's d

  # Bootstrap parameters (if using bootstrap)
  bootstrap_iterations: 10000
  bootstrap_confidence_level: 0.95
```

**Interpretation:**
- **p < 0.05**: Significant difference detected (good or bad)
- **p >= 0.05**: No significant difference (proceed with caution)
- **Effect size**: Magnitude of difference (small, medium, large)

---

## 4. Rollback Guide

### 4.1 Automatic Rollback Triggers

The Actuator automatically initiates rollback when critical thresholds are breached:

#### Critical Triggers (Immediate Rollback)

```yaml
auto_rollback:
  critical_triggers:
    # Error rate spike
    error_rate:
      threshold: 0.10  # 10% error rate
      window_minutes: 5

    # Latency degradation
    latency_p95:
      threshold_ms: 10000  # 10 seconds
      increase_pct: 2.0  # 200% increase
      window_minutes: 5

    # Quality collapse
    quality_score:
      min_threshold: 0.5
      degradation_pct: 0.30  # 30% drop
      window_minutes: 10

    # Cost explosion
    cost_per_request:
      threshold_usd: 1.0
      increase_pct: 0.50  # 50% increase
      window_minutes: 5

    # Service health
    service_health:
      consecutive_failures: 3
      health_check_interval_seconds: 30
```

#### Warning Triggers (Pause and Alert)

```yaml
auto_rollback:
  warning_triggers:
    # Moderate error rate increase
    error_rate:
      threshold: 0.05  # 5%
      window_minutes: 10

    # Latency increase
    latency_p95:
      increase_pct: 0.25  # 25% increase
      window_minutes: 10

    # Quality degradation
    quality_score:
      degradation_pct: 0.10  # 10% drop
      window_minutes: 15

  # Action on warning
  warning_action: "pause"  # or "continue", "alert_only"
  warning_pause_duration_minutes: 15
  require_manual_approval_after_warning: true
```

### 4.2 Manual Rollback Procedures

#### Using CLI

```bash
# Immediate rollback to previous version
./target/release/optimizer actuator rollback \
  --deployment-id <deployment-id> \
  --reason "High error rate observed"

# Rollback with options
./target/release/optimizer actuator rollback \
  --deployment-id <deployment-id> \
  --reason "Manual intervention required" \
  --immediate \
  --scope all

# Gradual rollback (recommended)
./target/release/optimizer actuator rollback \
  --deployment-id <deployment-id> \
  --reason "Gradual rollback for safety" \
  --gradual \
  --duration-minutes 30

# Rollback to specific version
./target/release/optimizer actuator rollback \
  --to-version <version-id> \
  --reason "Revert to known good configuration"
```

#### Using API

```bash
# Immediate rollback
curl -X POST http://localhost:8080/api/v1/actuator/rollback \
  -H "Content-Type: application/json" \
  -d '{
    "deployment_id": "deploy-123",
    "reason": "High error rate",
    "immediate": true,
    "scope": "all"
  }'

# Gradual rollback
curl -X POST http://localhost:8080/api/v1/actuator/rollback \
  -H "Content-Type: application/json" \
  -d '{
    "deployment_id": "deploy-123",
    "reason": "Gradual safety rollback",
    "gradual": true,
    "duration_minutes": 30,
    "scope": "canary"
  }'

# Check rollback status
curl http://localhost:8080/api/v1/actuator/rollback/status/<rollback-id>
```

#### Using Configuration File

```yaml
# rollback-request.yaml
deployment_id: "deploy-123"
reason: "Quality degradation detected"
options:
  immediate: false
  gradual: true
  duration_minutes: 30
  scope: "canary"
  notify_channels:
    - "slack"
    - "email"
  approval_required: true
```

```bash
./target/release/optimizer actuator rollback \
  --config rollback-request.yaml
```

### 4.3 Configuration Restoration

When a rollback occurs, the Actuator restores the previous configuration:

#### Version Management

```yaml
version_control:
  # Retention policy
  max_versions: 10
  retention_days: 90

  # Tagging
  auto_tag: true
  tag_format: "v{timestamp}-{strategy}"

  # Metadata
  include_metadata: true
  metadata_fields:
    - "decision_engine_version"
    - "optimization_strategy"
    - "performance_baseline"
    - "cost_baseline"
    - "deployment_timestamp"
    - "operator"
```

#### Restoration Process

```
1. Identify Target Version
   └─▶ Retrieve configuration from storage

2. Validate Configuration
   └─▶ Schema validation
   └─▶ Compatibility check
   └─▶ Safety constraints

3. Plan Restoration
   └─▶ Traffic split strategy
   └─▶ Rollback duration
   └─▶ Health checks

4. Execute Restoration
   └─▶ Apply configuration
   └─▶ Monitor metrics
   └─▶ Validate success

5. Finalize
   └─▶ Mark rollback complete
   └─▶ Archive canary version
   └─▶ Update audit log
```

### 4.4 Recovery Validation

After rollback, the Actuator validates recovery:

```yaml
recovery_validation:
  # Health checks
  health_checks:
    - type: "http"
      endpoint: "/health"
      expected_status: 200
      timeout_seconds: 10

    - type: "metrics"
      metric: "error_rate"
      max_value: 0.02
      window_minutes: 5

    - type: "latency"
      percentile: 95
      max_ms: 3000
      window_minutes: 5

  # Validation duration
  validation_duration_minutes: 15

  # Success criteria
  success_criteria:
    all_health_checks_pass: true
    metrics_within_bounds: true
    no_new_errors: true

  # Actions on failed validation
  on_failure:
    action: "alert_and_hold"  # or "rollback_further"
    escalation_channels:
      - "pagerduty"
      - "slack"
```

#### Validation Metrics

After rollback, monitor these metrics for 15-30 minutes:

| Metric | Target | Action if Failed |
|--------|--------|------------------|
| Error Rate | < 2% | Escalate to on-call |
| Latency P95 | < baseline + 10% | Continue monitoring |
| Quality Score | > 0.7 | Investigate root cause |
| Cost per Request | < baseline + 5% | Review configuration |
| Service Health | 100% | Check orchestrator |

---

## 5. Configuration Management

### 5.1 Configuration Schema

```yaml
# Configuration version and metadata
version: "1.0"
metadata:
  name: "gpt-4-turbo-optimization"
  description: "Optimized for cost and quality balance"
  created_at: "2025-11-10T10:00:00Z"
  created_by: "optimizer-decision-engine"
  tags:
    - "cost-optimization"
    - "quality-maintained"

# LLM configuration
llm:
  # Model selection
  model: "gpt-4-turbo"
  provider: "openai"

  # Prompt template
  prompt_template: |
    You are a helpful assistant.

    Context: {context}

    Question: {question}

  # Generation parameters
  parameters:
    temperature: 0.7
    top_p: 0.9
    max_tokens: 2048
    frequency_penalty: 0.0
    presence_penalty: 0.0

  # Retry and timeout
  retry_config:
    max_retries: 3
    retry_delay_ms: 1000
    timeout_ms: 30000

# Routing rules
routing:
  # Default route
  default_model: "gpt-4-turbo"

  # Conditional routing
  rules:
    - condition: "task.complexity == 'low'"
      model: "gpt-3.5-turbo"

    - condition: "task.latency_budget < 1000"
      model: "gpt-3.5-turbo"

    - condition: "task.quality_required > 0.9"
      model: "gpt-4"

# Cost controls
cost_controls:
  daily_budget_usd: 1000.0
  per_request_limit_usd: 0.50
  alert_threshold_usd: 800.0

# Quality controls
quality_controls:
  min_quality_score: 0.7
  enable_quality_monitoring: true
  quality_check_sample_rate: 0.1

# Performance targets
performance:
  target_latency_p95_ms: 3000
  target_latency_p99_ms: 5000
  max_concurrent_requests: 100
```

### 5.2 Configuration Versioning

```yaml
versioning:
  # Auto-versioning
  auto_version: true
  version_scheme: "semantic"  # or "timestamp", "sequential"

  # Diff tracking
  track_diffs: true
  diff_format: "json_patch"  # or "yaml_diff", "full_snapshot"

  # Change metadata
  require_change_reason: true
  require_approver: true
  approval_roles:
    - "platform-admin"
    - "sre-lead"

  # Storage
  storage_backend: "postgresql"  # or "s3", "git"
  retention_policy:
    max_versions: 50
    retention_days: 365
    compress_old_versions: true
```

### 5.3 Configuration Validation

```yaml
validation:
  # Schema validation
  schema_validation: true
  schema_path: "/etc/optimizer/schemas/config.schema.json"

  # Compatibility checks
  compatibility_checks:
    - "model_availability"
    - "parameter_ranges"
    - "cost_budget"
    - "performance_targets"

  # Dry run
  enable_dry_run: true
  dry_run_duration_minutes: 5
  dry_run_traffic_percentage: 1

  # Safety checks
  safety_checks:
    - name: "cost_explosion"
      max_cost_increase: 0.50  # 50%

    - name: "quality_degradation"
      max_quality_drop: 0.10  # 10%

    - name: "latency_spike"
      max_latency_increase: 0.25  # 25%
```

### 5.4 Configuration Templates

#### Cost-Optimized Template

```yaml
# config-templates/cost-optimized.yaml
metadata:
  template: "cost-optimized"
  use_case: "High-volume, cost-sensitive workloads"

llm:
  model: "gpt-3.5-turbo"
  parameters:
    temperature: 0.5
    max_tokens: 1024

routing:
  rules:
    - condition: "task.complexity == 'low'"
      model: "gpt-3.5-turbo"
    - condition: "task.complexity == 'medium'"
      model: "gpt-4-turbo"
    - condition: "task.complexity == 'high'"
      model: "gpt-4"

cost_controls:
  daily_budget_usd: 500.0
  per_request_limit_usd: 0.10
```

#### Quality-Optimized Template

```yaml
# config-templates/quality-optimized.yaml
metadata:
  template: "quality-optimized"
  use_case: "High-quality, accuracy-critical workloads"

llm:
  model: "gpt-4"
  parameters:
    temperature: 0.3
    max_tokens: 4096
    top_p: 0.95

routing:
  default_model: "gpt-4"
  rules:
    - condition: "task.quality_required > 0.95"
      model: "gpt-4"

quality_controls:
  min_quality_score: 0.85
  enable_quality_monitoring: true
  quality_check_sample_rate: 0.5
```

#### Balanced Template

```yaml
# config-templates/balanced.yaml
metadata:
  template: "balanced"
  use_case: "General-purpose workloads"

llm:
  model: "gpt-4-turbo"
  parameters:
    temperature: 0.7
    max_tokens: 2048

routing:
  default_model: "gpt-4-turbo"
  rules:
    - condition: "task.complexity == 'low'"
      model: "gpt-3.5-turbo"
    - condition: "task.complexity == 'high' && task.quality_required > 0.9"
      model: "gpt-4"

cost_controls:
  daily_budget_usd: 1000.0
  per_request_limit_usd: 0.25

quality_controls:
  min_quality_score: 0.75
```

---

## 6. Health Monitoring Setup

### 6.1 Health Check Configuration

```yaml
health_monitoring:
  # Health check endpoints
  endpoints:
    # Service health
    - name: "service_health"
      url: "http://orchestrator:8080/health"
      method: "GET"
      timeout_seconds: 5
      interval_seconds: 30
      success_codes: [200, 204]

    # Metrics health
    - name: "metrics_health"
      url: "http://prometheus:9090/-/healthy"
      method: "GET"
      timeout_seconds: 5
      interval_seconds: 60

    # Database health
    - name: "database_health"
      type: "postgres"
      connection_string: "${DATABASE_URL}"
      timeout_seconds: 10
      interval_seconds: 60

  # Health thresholds
  thresholds:
    error_rate:
      warning: 0.02  # 2%
      critical: 0.05  # 5%
      window_minutes: 5

    latency_p95:
      warning_ms: 3000
      critical_ms: 5000
      window_minutes: 5

    quality_score:
      warning: 0.75
      critical: 0.65
      window_minutes: 10

  # Alerting
  alerts:
    channels:
      - type: "slack"
        webhook_url: "${SLACK_WEBHOOK_URL}"
        severity_levels: ["warning", "critical"]

      - type: "pagerduty"
        integration_key: "${PAGERDUTY_KEY}"
        severity_levels: ["critical"]

      - type: "email"
        recipients: ["ops@company.com"]
        severity_levels: ["warning", "critical"]
```

### 6.2 Metrics Collection

```yaml
metrics:
  # Collection interval
  collection_interval_seconds: 10

  # Metrics to collect
  metrics:
    # Request metrics
    - name: "requests_total"
      type: "counter"
      labels: ["model", "status", "deployment"]

    - name: "request_duration_seconds"
      type: "histogram"
      labels: ["model", "deployment"]
      buckets: [0.1, 0.5, 1.0, 2.5, 5.0, 10.0]

    - name: "request_cost_usd"
      type: "histogram"
      labels: ["model", "deployment"]
      buckets: [0.001, 0.01, 0.05, 0.1, 0.5, 1.0]

    # Quality metrics
    - name: "quality_score"
      type: "histogram"
      labels: ["model", "deployment"]
      buckets: [0.5, 0.6, 0.7, 0.8, 0.9, 1.0]

    # Deployment metrics
    - name: "deployment_status"
      type: "gauge"
      labels: ["deployment_id", "phase"]

    - name: "canary_traffic_percentage"
      type: "gauge"
      labels: ["deployment_id"]

    - name: "rollback_total"
      type: "counter"
      labels: ["reason", "deployment_id"]

  # Export configuration
  export:
    # Prometheus
    prometheus:
      enabled: true
      port: 9090
      path: "/metrics"

    # OTLP (OpenTelemetry)
    otlp:
      enabled: true
      endpoint: "http://collector:4317"

    # Custom webhook
    webhook:
      enabled: false
      url: "${METRICS_WEBHOOK_URL}"
      interval_seconds: 60
```

### 6.3 Dashboard Setup

#### Grafana Dashboard

```json
{
  "dashboard": {
    "title": "LLM Auto Optimizer - Actuator",
    "panels": [
      {
        "title": "Canary Deployment Status",
        "type": "stat",
        "targets": [
          {
            "expr": "deployment_status{phase='active'}"
          }
        ]
      },
      {
        "title": "Traffic Split",
        "type": "gauge",
        "targets": [
          {
            "expr": "canary_traffic_percentage"
          }
        ]
      },
      {
        "title": "Error Rate Comparison",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(requests_total{status='error',deployment='canary'}[5m])",
            "legendFormat": "Canary"
          },
          {
            "expr": "rate(requests_total{status='error',deployment='stable'}[5m])",
            "legendFormat": "Stable"
          }
        ]
      },
      {
        "title": "Latency P95 Comparison",
        "type": "graph",
        "targets": [
          {
            "expr": "histogram_quantile(0.95, request_duration_seconds{deployment='canary'})",
            "legendFormat": "Canary P95"
          },
          {
            "expr": "histogram_quantile(0.95, request_duration_seconds{deployment='stable'})",
            "legendFormat": "Stable P95"
          }
        ]
      },
      {
        "title": "Cost Per Request",
        "type": "graph",
        "targets": [
          {
            "expr": "avg(request_cost_usd{deployment='canary'})",
            "legendFormat": "Canary"
          },
          {
            "expr": "avg(request_cost_usd{deployment='stable'})",
            "legendFormat": "Stable"
          }
        ]
      },
      {
        "title": "Quality Score",
        "type": "graph",
        "targets": [
          {
            "expr": "avg(quality_score{deployment='canary'})",
            "legendFormat": "Canary"
          },
          {
            "expr": "avg(quality_score{deployment='stable'})",
            "legendFormat": "Stable"
          }
        ]
      }
    ]
  }
}
```

### 6.4 Alerting Rules

```yaml
# alerting-rules.yaml
groups:
  - name: actuator_deployment
    interval: 30s
    rules:
      # Critical: High error rate
      - alert: CanaryHighErrorRate
        expr: |
          rate(requests_total{status="error",deployment="canary"}[5m]) > 0.05
        for: 2m
        labels:
          severity: critical
          component: actuator
        annotations:
          summary: "Canary deployment has high error rate"
          description: "Error rate is {{ $value }} (threshold: 5%)"

      # Critical: High latency
      - alert: CanaryHighLatency
        expr: |
          histogram_quantile(0.95, request_duration_seconds{deployment="canary"}) > 5
        for: 5m
        labels:
          severity: critical
          component: actuator
        annotations:
          summary: "Canary deployment has high latency"
          description: "P95 latency is {{ $value }}s (threshold: 5s)"

      # Warning: Quality degradation
      - alert: CanaryQualityDegradation
        expr: |
          avg(quality_score{deployment="canary"}) < 0.75
        for: 10m
        labels:
          severity: warning
          component: actuator
        annotations:
          summary: "Canary quality score below threshold"
          description: "Quality score is {{ $value }} (threshold: 0.75)"

      # Info: Deployment started
      - alert: DeploymentStarted
        expr: |
          deployment_status{phase="initializing"} == 1
        for: 1m
        labels:
          severity: info
          component: actuator
        annotations:
          summary: "New canary deployment started"
          description: "Deployment {{ $labels.deployment_id }} initiated"

      # Critical: Rollback triggered
      - alert: RollbackTriggered
        expr: |
          increase(rollback_total[5m]) > 0
        labels:
          severity: critical
          component: actuator
        annotations:
          summary: "Automatic rollback triggered"
          description: "Rollback reason: {{ $labels.reason }}"
```

---

## 7. Integration with Decision Engine

### 7.1 Decision-to-Deployment Flow

```
Decision Engine                    Actuator
      │                               │
      │ 1. Optimization Decision      │
      │──────────────────────────────▶│
      │                               │
      │                               │ 2. Validate Decision
      │                               │────┐
      │                               │    │
      │                               │◀───┘
      │                               │
      │ 3. Validation Result          │
      │◀──────────────────────────────│
      │                               │
      │ 4. Deployment Plan            │
      │──────────────────────────────▶│
      │                               │
      │                               │ 5. Execute Canary
      │                               │────┐
      │                               │    │
      │                               │◀───┘
      │                               │
      │ 6. Health Metrics (Streaming) │
      │◀──────────────────────────────│
      │                               │
      │ 7. Deployment Status          │
      │◀──────────────────────────────│
```

### 7.2 Decision Payload Format

```json
{
  "decision_id": "dec-20251110-001",
  "timestamp": "2025-11-10T10:00:00Z",
  "decision_type": "model_switch",
  "confidence": 0.92,
  "expected_improvement": {
    "cost_reduction": 0.35,
    "quality_change": -0.02,
    "latency_change": -0.10
  },
  "configuration": {
    "model": "gpt-4-turbo",
    "parameters": {
      "temperature": 0.7,
      "max_tokens": 2048
    }
  },
  "deployment_strategy": {
    "type": "canary",
    "initial_percentage": 5,
    "auto_rollback": true
  },
  "metadata": {
    "optimization_strategy": "cost_performance",
    "baseline_metrics": {
      "cost_per_request": 0.05,
      "latency_p95": 2500,
      "quality_score": 0.82
    }
  }
}
```

### 7.3 Actuator Response Format

```json
{
  "deployment_id": "deploy-20251110-001",
  "decision_id": "dec-20251110-001",
  "status": "in_progress",
  "phase": "phase_2",
  "current_percentage": 25,
  "started_at": "2025-11-10T10:05:00Z",
  "expected_completion": "2025-11-10T12:05:00Z",
  "metrics": {
    "canary": {
      "error_rate": 0.018,
      "latency_p95": 2300,
      "cost_per_request": 0.032,
      "quality_score": 0.80,
      "sample_size": 5420
    },
    "stable": {
      "error_rate": 0.020,
      "latency_p95": 2500,
      "cost_per_request": 0.050,
      "quality_score": 0.82,
      "sample_size": 18730
    },
    "comparison": {
      "error_rate_delta": -0.002,
      "latency_delta_pct": -0.08,
      "cost_delta_pct": -0.36,
      "quality_delta": -0.02,
      "statistical_significance": true,
      "p_value": 0.012
    }
  },
  "next_step": {
    "action": "increase_traffic",
    "percentage": 50,
    "scheduled_at": "2025-11-10T10:35:00Z"
  }
}
```

### 7.4 Feedback Loop

```yaml
feedback_loop:
  # Send metrics to Decision Engine
  enable_feedback: true
  feedback_interval_seconds: 60

  # Metrics to include
  feedback_metrics:
    - "error_rate"
    - "latency_p50"
    - "latency_p95"
    - "latency_p99"
    - "cost_per_request"
    - "quality_score"
    - "sample_size"

  # Aggregation
  aggregation_window_minutes: 5

  # Destination
  decision_engine_endpoint: "http://decision-engine:8080/api/v1/feedback"

  # Include deployment context
  include_context:
    - "deployment_id"
    - "decision_id"
    - "phase"
    - "traffic_percentage"
```

---

## 8. Usage Examples

### 8.1 Example 1: Cost Optimization Deployment

**Scenario:** Deploy a cost-optimized configuration with careful monitoring.

```bash
# 1. Create cost-optimized configuration
cat > cost-optimized.yaml <<EOF
metadata:
  name: "cost-reduction-nov-2025"
  description: "Switch to GPT-3.5-Turbo for low-complexity tasks"

llm:
  model: "gpt-3.5-turbo"
  parameters:
    temperature: 0.5
    max_tokens: 1024

routing:
  rules:
    - condition: "task.complexity == 'low'"
      model: "gpt-3.5-turbo"
    - condition: "task.complexity == 'medium'"
      model: "gpt-4-turbo"

cost_controls:
  daily_budget_usd: 500.0
EOF

# 2. Validate configuration
./target/release/optimizer actuator validate \
  --config cost-optimized.yaml

# 3. Deploy with conservative canary
./target/release/optimizer actuator deploy \
  --config cost-optimized.yaml \
  --strategy canary \
  --initial-percentage 5 \
  --rollout-duration 180 \
  --auto-rollback

# 4. Monitor deployment
./target/release/optimizer actuator status --follow

# 5. Check metrics
./target/release/optimizer actuator metrics \
  --deployment-id <id> \
  --compare-baseline
```

**Expected Outcome:**
- 30-40% cost reduction
- Minor quality impact (<5%)
- Gradual rollout over 3 hours
- Auto-rollback if quality drops >10%

### 8.2 Example 2: Emergency Performance Fix

**Scenario:** Latency spike detected, need quick rollout of optimized config.

```bash
# 1. Create performance-optimized configuration
cat > perf-fix.yaml <<EOF
metadata:
  name: "latency-fix-emergency"
  description: "Reduce max_tokens and optimize parameters"

llm:
  model: "gpt-4-turbo"
  parameters:
    temperature: 0.7
    max_tokens: 1024  # Reduced from 2048
    timeout_ms: 20000

performance:
  target_latency_p95_ms: 2000
  max_concurrent_requests: 150
EOF

# 2. Aggressive rollout (1 hour)
./target/release/optimizer actuator deploy \
  --config perf-fix.yaml \
  --strategy canary \
  --initial-percentage 10 \
  --rollout-duration 60 \
  --steps "10,50,100" \
  --auto-rollback

# 3. Monitor latency improvement
watch -n 10 './target/release/optimizer actuator metrics \
  --deployment-id <id> \
  --metric latency_p95'
```

**Expected Outcome:**
- 20-30% latency reduction
- Faster rollout (1 hour)
- Immediate impact on user experience

### 8.3 Example 3: Quality Improvement Rollout

**Scenario:** Deploy higher-quality model for critical workload.

```bash
# 1. Create quality-focused configuration
cat > quality-upgrade.yaml <<EOF
metadata:
  name: "gpt-4-quality-upgrade"
  description: "Upgrade critical paths to GPT-4"

llm:
  model: "gpt-4"
  parameters:
    temperature: 0.3
    max_tokens: 4096
    top_p: 0.95

routing:
  default_model: "gpt-4"

quality_controls:
  min_quality_score: 0.85
  enable_quality_monitoring: true
EOF

# 2. Deploy with extensive validation
./target/release/optimizer actuator deploy \
  --config quality-upgrade.yaml \
  --strategy canary \
  --initial-percentage 5 \
  --rollout-duration 240 \
  --min-sample-size 2000 \
  --confidence-level 0.99

# 3. A/B test comparison
./target/release/optimizer actuator ab-test \
  --deployment-id <id> \
  --metric quality_score \
  --min-effect-size 0.05
```

**Expected Outcome:**
- 10-15% quality improvement
- Higher costs accepted
- Statistical validation (p<0.01)
- 4-hour careful rollout

### 8.4 Example 4: Multi-Model Routing Strategy

**Scenario:** Deploy intelligent routing based on task complexity.

```bash
# 1. Create routing configuration
cat > smart-routing.yaml <<EOF
metadata:
  name: "intelligent-routing-v2"
  description: "Route based on complexity and latency budget"

routing:
  default_model: "gpt-4-turbo"

  rules:
    # Fast, cheap requests
    - condition: "task.latency_budget < 1000 && task.complexity == 'low'"
      model: "gpt-3.5-turbo"
      parameters:
        temperature: 0.5
        max_tokens: 512

    # Standard requests
    - condition: "task.complexity == 'medium'"
      model: "gpt-4-turbo"
      parameters:
        temperature: 0.7
        max_tokens: 2048

    # High-quality requests
    - condition: "task.quality_required > 0.9"
      model: "gpt-4"
      parameters:
        temperature: 0.3
        max_tokens: 4096

    # Budget-constrained
    - condition: "cost_budget < 0.05"
      model: "gpt-3.5-turbo"

cost_controls:
  daily_budget_usd: 1000.0
  per_request_limit_usd: 0.50
EOF

# 2. Deploy with monitoring per route
./target/release/optimizer actuator deploy \
  --config smart-routing.yaml \
  --strategy canary \
  --initial-percentage 10 \
  --track-routing-metrics

# 3. Analyze routing distribution
./target/release/optimizer actuator routing-stats \
  --deployment-id <id>
```

**Expected Outcome:**
- 25-35% cost reduction
- Maintained quality for high-value requests
- Optimized latency for time-sensitive requests

### 8.5 Example 5: Gradual Parameter Tuning

**Scenario:** Fine-tune temperature parameter based on feedback.

```bash
# 1. Create parameter variants
cat > param-tuning.yaml <<EOF
metadata:
  name: "temperature-optimization"
  description: "Test temperature values 0.5, 0.7, 0.9"

variants:
  - name: "conservative"
    parameters:
      temperature: 0.5
    weight: 0.33

  - name: "balanced"
    parameters:
      temperature: 0.7
    weight: 0.34

  - name: "creative"
    parameters:
      temperature: 0.9
    weight: 0.33

ab_test:
  metric: "quality_score"
  min_sample_size: 5000
  significance_level: 0.05
  max_duration_hours: 48
EOF

# 2. Deploy A/B test
./target/release/optimizer actuator ab-test \
  --config param-tuning.yaml \
  --strategy thompson-sampling

# 3. Monitor convergence
./target/release/optimizer actuator ab-test-status \
  --test-id <id> \
  --watch

# 4. Promote winner
./target/release/optimizer actuator ab-test-promote \
  --test-id <id> \
  --winner auto
```

**Expected Outcome:**
- Identify optimal temperature value
- Statistical significance (p<0.05)
- Automatic promotion of best variant

### 8.6 Example 6: Blue-Green Deployment

**Scenario:** Zero-downtime model switch with instant rollback capability.

```bash
# 1. Prepare blue-green deployment
cat > blue-green.yaml <<EOF
metadata:
  name: "model-switch-blue-green"
  description: "Switch from GPT-4 to GPT-4-Turbo"

deployment_strategy:
  type: "blue-green"

  blue:
    model: "gpt-4"
    parameters:
      temperature: 0.7

  green:
    model: "gpt-4-turbo"
    parameters:
      temperature: 0.7

  cutover:
    validation_duration_minutes: 30
    traffic_shift_duration_seconds: 10
EOF

# 2. Deploy blue-green
./target/release/optimizer actuator deploy \
  --config blue-green.yaml \
  --strategy blue-green \
  --keep-blue-duration 24h

# 3. Validate green environment
./target/release/optimizer actuator validate-green \
  --deployment-id <id>

# 4. Switch traffic
./target/release/optimizer actuator cutover \
  --deployment-id <id>

# 5. Monitor and rollback if needed
./target/release/optimizer actuator monitor \
  --deployment-id <id> \
  --auto-rollback-to-blue
```

**Expected Outcome:**
- Instant cutover (<10 seconds)
- 24-hour rollback window
- Zero downtime

---

## 9. Best Practices

### 9.1 Deployment Strategy Selection

| Scenario | Recommended Strategy | Rationale |
|----------|---------------------|-----------|
| **Minor parameter tweaks** | Aggressive canary (1 hour) | Low risk, quick validation |
| **Model version update** | Standard canary (3 hours) | Medium risk, needs validation |
| **Model switch (e.g., GPT-3.5 to GPT-4)** | Conservative canary (6 hours) | High impact, extensive testing |
| **Critical production fix** | Blue-green | Need instant rollback |
| **A/B testing** | Thompson sampling canary | Optimize while testing |
| **Off-peak optimization** | Aggressive canary | Lower traffic = lower risk |

### 9.2 Success Criteria Guidelines

```yaml
# Conservative (critical production systems)
success_criteria:
  max_error_rate_increase: 0.01  # 1%
  max_latency_increase: 0.10  # 10%
  max_quality_degradation: 0.03  # 3%
  confidence_level: 0.99  # 99%
  min_sample_size: 5000

# Standard (normal production)
success_criteria:
  max_error_rate_increase: 0.02  # 2%
  max_latency_increase: 0.15  # 15%
  max_quality_degradation: 0.05  # 5%
  confidence_level: 0.95  # 95%
  min_sample_size: 1000

# Aggressive (development/staging)
success_criteria:
  max_error_rate_increase: 0.05  # 5%
  max_latency_increase: 0.25  # 25%
  max_quality_degradation: 0.10  # 10%
  confidence_level: 0.90  # 90%
  min_sample_size: 500
```

### 9.3 Rollback Decision Matrix

| Condition | Error Rate | Latency Increase | Quality Drop | Action |
|-----------|-----------|------------------|--------------|--------|
| **Critical** | >10% | >200% or >10s | >30% | Immediate rollback |
| **Severe** | 5-10% | 50-200% or 5-10s | 20-30% | Pause and alert |
| **Warning** | 2-5% | 25-50% | 10-20% | Continue with caution |
| **Normal** | <2% | <25% | <10% | Proceed |

### 9.4 Monitoring Best Practices

#### Essential Metrics to Track

1. **Error Rate**
   - Track: 5-minute rolling window
   - Alert: >2% (warning), >5% (critical)
   - Compare: Canary vs. Stable

2. **Latency**
   - Track: P50, P95, P99
   - Alert: P95 >5s or >50% increase
   - Compare: Both percentiles

3. **Cost**
   - Track: Per-request cost
   - Alert: >25% increase
   - Compare: Daily spend

4. **Quality**
   - Track: Quality score (if available)
   - Alert: <0.7 or >10% drop
   - Compare: Sample-based

5. **Traffic Distribution**
   - Track: Requests per deployment
   - Verify: Matches expected split
   - Alert: Imbalance >5%

#### Dashboard Requirements

```yaml
dashboard_requirements:
  refresh_interval_seconds: 10

  panels:
    # Overview
    - "Deployment status and phase"
    - "Traffic split percentage"
    - "Time remaining in phase"

    # Metrics comparison
    - "Error rate: Canary vs Stable"
    - "Latency P95: Canary vs Stable"
    - "Cost per request: Canary vs Stable"
    - "Quality score: Canary vs Stable"

    # Statistical validation
    - "Sample sizes"
    - "P-values for each metric"
    - "Confidence intervals"

    # Deployment history
    - "Recent deployments"
    - "Rollback history"
    - "Success rate"
```

### 9.5 Safety and Compliance

#### Pre-Deployment Checklist

- [ ] Configuration validated against schema
- [ ] Safety constraints verified
- [ ] Baseline metrics recorded
- [ ] Rollback plan documented
- [ ] Monitoring dashboard configured
- [ ] Alerts configured and tested
- [ ] Approval obtained (if required)
- [ ] Communication sent to stakeholders
- [ ] Deployment window scheduled
- [ ] On-call engineer notified

#### Post-Deployment Checklist

- [ ] All phases completed successfully
- [ ] Metrics within acceptable bounds
- [ ] No automatic rollbacks triggered
- [ ] Sample size sufficient for validation
- [ ] Statistical significance confirmed
- [ ] Documentation updated
- [ ] Post-mortem scheduled (if issues)
- [ ] Feedback sent to Decision Engine
- [ ] Configuration archived
- [ ] Audit log reviewed

### 9.6 Common Pitfalls to Avoid

#### 1. Insufficient Sample Size

**Problem:** Making decisions with too few requests
**Solution:** Set `min_sample_size` appropriately (1000+ recommended)

```yaml
# Bad
min_sample_size: 100  # Too small

# Good
min_sample_size: 1000  # Adequate for statistical validity
```

#### 2. Ignoring Statistical Significance

**Problem:** Promoting canary despite no significant improvement
**Solution:** Require p-value < 0.05

```yaml
success_criteria:
  require_statistical_significance: true
  p_value_threshold: 0.05
```

#### 3. Too Aggressive Rollout

**Problem:** Jumping to 100% too quickly
**Solution:** Use gradual steps with validation

```yaml
# Bad
steps:
  - percentage: 50
    duration_minutes: 10
  - percentage: 100
    duration_minutes: 10

# Good
steps:
  - percentage: 5
    duration_minutes: 15
  - percentage: 25
    duration_minutes: 30
  - percentage: 50
    duration_minutes: 30
  - percentage: 75
    duration_minutes: 30
  - percentage: 100
    duration_minutes: 15
```

#### 4. Not Monitoring Quality

**Problem:** Focusing only on cost/latency
**Solution:** Always track quality metrics

```yaml
success_criteria:
  # Always include quality
  min_quality_score: 0.7
  max_quality_degradation: 0.05
```

#### 5. Disabled Auto-Rollback

**Problem:** Manual intervention required for failures
**Solution:** Enable auto-rollback with appropriate thresholds

```yaml
auto_rollback:
  enabled: true  # Always enable
  critical_error_rate: 0.10
  critical_latency_p95_ms: 10000
```

---

## 10. Troubleshooting

### 10.1 Common Issues and Solutions

#### Issue 1: Deployment Stuck in Phase

**Symptoms:**
- Deployment not progressing to next phase
- Metrics collection stalled
- No errors in logs

**Diagnosis:**
```bash
# Check deployment status
./target/release/optimizer actuator status --deployment-id <id>

# Check metrics collection
./target/release/optimizer actuator metrics \
  --deployment-id <id> \
  --detailed

# Check sample size
./target/release/optimizer actuator sample-size \
  --deployment-id <id>
```

**Solutions:**

1. **Insufficient traffic:**
   ```bash
   # Increase canary percentage manually
   ./target/release/optimizer actuator adjust-traffic \
     --deployment-id <id> \
     --percentage 10
   ```

2. **Sample size not reached:**
   ```bash
   # Extend phase duration
   ./target/release/optimizer actuator extend-phase \
     --deployment-id <id> \
     --additional-minutes 30
   ```

3. **Metrics not being collected:**
   ```bash
   # Restart metrics collection
   ./target/release/optimizer actuator restart-monitoring \
     --deployment-id <id>
   ```

#### Issue 2: False Positive Rollback

**Symptoms:**
- Automatic rollback triggered unnecessarily
- Metrics appear normal
- Transient spike caused rollback

**Diagnosis:**
```bash
# Check rollback reason
./target/release/optimizer actuator rollback-details \
  --rollback-id <id>

# Review metrics at rollback time
./target/release/optimizer actuator metrics \
  --deployment-id <id> \
  --time-range "2025-11-10T10:00:00Z/2025-11-10T10:30:00Z"
```

**Solutions:**

1. **Adjust thresholds:**
   ```yaml
   auto_rollback:
     # Increase threshold
     critical_error_rate: 0.15  # Was 0.10

     # Require sustained violation
     violation_duration_minutes: 5  # Not instant
   ```

2. **Increase window size:**
   ```yaml
   success_criteria:
     # Longer window = less noise
     comparison_window_minutes: 15  # Was 5
   ```

3. **Disable during known issues:**
   ```bash
   # Temporarily disable auto-rollback
   ./target/release/optimizer actuator pause-auto-rollback \
     --deployment-id <id> \
     --duration-minutes 60
   ```

#### Issue 3: Uneven Traffic Distribution

**Symptoms:**
- Canary receiving more/less traffic than configured
- Metrics not comparable
- Hash distribution issues

**Diagnosis:**
```bash
# Check traffic distribution
./target/release/optimizer actuator traffic-stats \
  --deployment-id <id>

# Verify hash distribution
./target/release/optimizer actuator hash-analysis \
  --deployment-id <id>
```

**Solutions:**

1. **Switch splitting method:**
   ```yaml
   traffic_split:
     method: "weighted_random"  # More predictable than hash
   ```

2. **Rebalance traffic:**
   ```bash
   ./target/release/optimizer actuator rebalance \
     --deployment-id <id>
   ```

3. **Check load balancer:**
   ```bash
   # Verify orchestrator routing
   curl http://orchestrator:8080/api/v1/routing/stats
   ```

#### Issue 4: Configuration Validation Failures

**Symptoms:**
- Deployment rejected during validation
- Schema errors
- Incompatible parameters

**Diagnosis:**
```bash
# Validate configuration
./target/release/optimizer actuator validate \
  --config config.yaml \
  --verbose

# Check schema version
./target/release/optimizer actuator schema-version
```

**Solutions:**

1. **Fix schema errors:**
   ```bash
   # Get detailed validation errors
   ./target/release/optimizer actuator validate \
     --config config.yaml \
     --output-format json
   ```

2. **Update schema:**
   ```bash
   # Update to latest schema
   ./target/release/optimizer actuator update-schema
   ```

3. **Override validation (caution):**
   ```bash
   # Skip validation (not recommended)
   ./target/release/optimizer actuator deploy \
     --config config.yaml \
     --skip-validation
   ```

#### Issue 5: Metrics Not Available

**Symptoms:**
- No metrics in dashboard
- Comparison shows "No data"
- Prometheus/OTLP export failing

**Diagnosis:**
```bash
# Check metrics export
./target/release/optimizer actuator check-metrics-export

# Test Prometheus endpoint
curl http://localhost:9090/metrics

# Check OTLP collector
curl http://collector:13133/
```

**Solutions:**

1. **Restart metrics exporter:**
   ```bash
   ./target/release/optimizer actuator restart-metrics
   ```

2. **Verify Prometheus config:**
   ```yaml
   # prometheus.yml
   scrape_configs:
     - job_name: 'optimizer-actuator'
       static_configs:
         - targets: ['localhost:9090']
       scrape_interval: 10s
   ```

3. **Check OTLP endpoint:**
   ```bash
   # Test OTLP connection
   grpcurl -plaintext collector:4317 list
   ```

### 10.2 Diagnostic Commands

```bash
# General health check
./target/release/optimizer actuator health-check

# Detailed deployment status
./target/release/optimizer actuator status \
  --deployment-id <id> \
  --verbose

# View deployment logs
./target/release/optimizer actuator logs \
  --deployment-id <id> \
  --tail 100 \
  --follow

# Export deployment data
./target/release/optimizer actuator export \
  --deployment-id <id> \
  --format json \
  --output deployment-data.json

# Analyze rollback history
./target/release/optimizer actuator rollback-history \
  --days 7 \
  --format table

# Test configuration
./target/release/optimizer actuator dry-run \
  --config config.yaml \
  --duration-minutes 5

# Validate Decision Engine integration
./target/release/optimizer actuator test-integration \
  --component decision-engine

# Check database connectivity
./target/release/optimizer actuator db-check

# Verify Orchestrator connectivity
./target/release/optimizer actuator orchestrator-check
```

### 10.3 Debugging Mode

Enable detailed logging for troubleshooting:

```yaml
# config.yaml
observability:
  log_level: "debug"
  json_logging: true

  # Enable trace logging for actuator
  component_log_levels:
    actuator: "trace"
    decision_receiver: "debug"
    traffic_splitter: "debug"
    health_monitor: "debug"
    rollback_manager: "debug"
```

```bash
# Run with debug logging
RUST_LOG=actuator=trace ./target/release/optimizer

# Or via CLI
./target/release/optimizer \
  --log-level trace \
  --component actuator
```

---

## 11. Production Operations Guide

### 11.1 Daily Operations

#### Morning Checks

```bash
#!/bin/bash
# daily-actuator-check.sh

echo "=== Daily Actuator Health Check ==="

# 1. Check active deployments
echo "Active deployments:"
./target/release/optimizer actuator list-active

# 2. Review overnight deployments
echo "Overnight deployment summary:"
./target/release/optimizer actuator summary --since 24h

# 3. Check rollback count
echo "Rollbacks in last 24h:"
./target/release/optimizer actuator rollback-count --since 24h

# 4. Verify metrics export
echo "Metrics export status:"
./target/release/optimizer actuator metrics-health

# 5. Check database size
echo "Database size:"
./target/release/optimizer actuator db-size

# 6. Review alerts
echo "Active alerts:"
./target/release/optimizer actuator active-alerts
```

#### Deployment Approval Workflow

```bash
# 1. Operator receives deployment request
./target/release/optimizer actuator pending-approvals

# 2. Review deployment plan
./target/release/optimizer actuator review \
  --deployment-id <id>

# 3. Check safety constraints
./target/release/optimizer actuator safety-check \
  --deployment-id <id>

# 4. Approve or reject
./target/release/optimizer actuator approve \
  --deployment-id <id> \
  --approver "john.doe@company.com" \
  --comment "Approved after review"

# Or reject
./target/release/optimizer actuator reject \
  --deployment-id <id> \
  --reason "Cost increase too high"
```

### 11.2 Incident Response

#### Incident Response Runbook

**Phase 1: Detection (0-5 minutes)**

```bash
# 1. Check active deployments
./target/release/optimizer actuator list-active

# 2. Identify problematic deployment
./target/release/optimizer actuator find-issues

# 3. View recent metrics
./target/release/optimizer actuator metrics \
  --deployment-id <id> \
  --last 15m
```

**Phase 2: Mitigation (5-10 minutes)**

```bash
# Option A: Immediate rollback
./target/release/optimizer actuator rollback \
  --deployment-id <id> \
  --immediate \
  --reason "Incident response: high error rate"

# Option B: Reduce canary traffic
./target/release/optimizer actuator reduce-traffic \
  --deployment-id <id> \
  --percentage 1

# Option C: Pause deployment
./target/release/optimizer actuator pause \
  --deployment-id <id>
```

**Phase 3: Investigation (10-30 minutes)**

```bash
# 1. Export deployment data
./target/release/optimizer actuator export \
  --deployment-id <id> \
  --output incident-data.json

# 2. Analyze logs
./target/release/optimizer actuator logs \
  --deployment-id <id> \
  --level error \
  --since 30m

# 3. Compare configurations
./target/release/optimizer actuator diff \
  --deployment-id <id> \
  --compare-baseline
```

**Phase 4: Resolution (30+ minutes)**

```bash
# 1. Document incident
./target/release/optimizer actuator incident-report \
  --deployment-id <id> \
  --output incident-report.md

# 2. Create fix
# (Edit configuration based on root cause)

# 3. Re-deploy with fix
./target/release/optimizer actuator deploy \
  --config fixed-config.yaml \
  --strategy canary \
  --incident-reference INC-2025-001
```

### 11.3 Capacity Planning

#### Storage Requirements

```bash
# Check current storage usage
./target/release/optimizer actuator storage-stats

# Estimate future storage
./target/release/optimizer actuator storage-forecast \
  --deployments-per-day 10 \
  --retention-days 90

# Cleanup old data
./target/release/optimizer actuator cleanup \
  --older-than 90d \
  --dry-run
```

#### Performance Tuning

```yaml
# config.yaml - Performance tuning
actuator:
  # Concurrency
  max_concurrent_deployments: 5
  max_concurrent_health_checks: 20

  # Batching
  metrics_batch_size: 100
  metrics_flush_interval_seconds: 10

  # Caching
  cache:
    enabled: true
    ttl_seconds: 300
    max_entries: 10000

  # Connection pooling
  database:
    connection_pool_size: 20
    connection_timeout_seconds: 30

  orchestrator:
    connection_pool_size: 50
    request_timeout_seconds: 10
```

### 11.4 Backup and Recovery

#### Backup Configuration

```yaml
# backup-config.yaml
backup:
  # Automatic backups
  enabled: true
  schedule: "0 2 * * *"  # 2 AM daily

  # Retention
  retention:
    daily: 7
    weekly: 4
    monthly: 12

  # What to backup
  include:
    - deployments
    - configurations
    - rollback_history
    - audit_logs

  # Storage
  storage:
    type: "s3"
    bucket: "llm-optimizer-backups"
    prefix: "actuator/"
    encryption: true
```

#### Recovery Procedures

```bash
# List available backups
./target/release/optimizer actuator backup-list

# Restore from backup
./target/release/optimizer actuator restore \
  --backup-id backup-20251110-020000 \
  --components deployments,configurations

# Verify restoration
./target/release/optimizer actuator verify-restore
```

### 11.5 Upgrade and Maintenance

#### Actuator Upgrade Process

```bash
# 1. Check current version
./target/release/optimizer actuator version

# 2. Backup current state
./target/release/optimizer actuator backup-now \
  --tag pre-upgrade

# 3. Drain active deployments
./target/release/optimizer actuator drain \
  --timeout 3600

# 4. Stop actuator
systemctl stop llm-optimizer-actuator

# 5. Upgrade binary
cargo build --release -p actuator
cp target/release/optimizer /usr/local/bin/

# 6. Run migrations
./target/release/optimizer actuator migrate

# 7. Validate upgrade
./target/release/optimizer actuator validate-upgrade

# 8. Start actuator
systemctl start llm-optimizer-actuator

# 9. Verify health
./target/release/optimizer actuator health-check

# 10. Resume operations
./target/release/optimizer actuator resume
```

### 11.6 Audit and Compliance

#### Audit Log Configuration

```yaml
audit:
  # Enable audit logging
  enabled: true

  # Events to audit
  events:
    - deployment_started
    - deployment_completed
    - deployment_failed
    - rollback_triggered
    - rollback_completed
    - configuration_changed
    - approval_granted
    - approval_denied
    - manual_intervention

  # Storage
  storage:
    type: "postgresql"
    retention_days: 365

  # Export
  export:
    enabled: true
    format: "json"
    destination: "s3://audit-logs/actuator/"
    schedule: "0 0 * * *"  # Daily
```

#### Compliance Reports

```bash
# Generate deployment report
./target/release/optimizer actuator report \
  --type deployment \
  --start-date 2025-11-01 \
  --end-date 2025-11-30 \
  --output deployment-report-nov-2025.pdf

# Generate rollback report
./target/release/optimizer actuator report \
  --type rollback \
  --start-date 2025-11-01 \
  --end-date 2025-11-30 \
  --output rollback-report-nov-2025.pdf

# Generate audit trail
./target/release/optimizer actuator audit-trail \
  --deployment-id <id> \
  --output audit-trail.json
```

---

## Appendix A: Configuration Reference

See the full configuration reference at: `/docs/ACTUATOR_CONFIG_REFERENCE.md`

## Appendix B: API Reference

See the full API reference at: `/docs/ACTUATOR_API_REFERENCE.md`

## Appendix C: Metrics Reference

See the full metrics reference at: `/docs/ACTUATOR_METRICS_REFERENCE.md`

## Appendix D: Glossary

- **Canary Deployment**: Progressive rollout strategy that deploys changes to a small percentage of traffic first
- **Rollback**: Reverting to a previous configuration due to issues or failures
- **Traffic Splitting**: Dividing requests between different configurations/versions
- **Blue-Green Deployment**: Deployment strategy with two identical environments for zero-downtime updates
- **Statistical Significance**: Confidence that observed differences are not due to random chance
- **Health Check**: Automated validation of service health and metrics
- **Safety Constraints**: Limits that prevent unsafe optimizations
- **Baseline Metrics**: Reference metrics from stable version for comparison

---

## Support and Resources

- **Documentation**: https://docs.llmdevops.dev/auto-optimizer/actuator
- **GitHub Issues**: https://github.com/globalbusinessadvisors/llm-auto-optimizer/issues
- **Slack Community**: #llm-auto-optimizer
- **Email Support**: support@llmdevops.dev

---

**Document Version:** 1.0
**Last Updated:** November 10, 2025
**Next Review:** February 10, 2026
