# Decision Engine Implementation Plan

## Executive Summary

This document provides a comprehensive implementation plan for the Decision Engine, a critical component of the LLM Auto-Optimizer that makes intelligent optimization decisions based on real-time analysis from the Analyzer Engine. The engine implements 5 core optimization strategies: **Model Selection**, **Caching**, **Rate Limiting**, **Request Batching**, and **Prompt Optimization**. This is production-grade, enterprise-ready software designed for commercial deployment.

**Key Metrics:**
- Total Lines of Code: 6,800-7,500
- Number of Files: 25
- Test Cases: 120+
- Code Coverage Target: >92%
- Implementation Timeline: 6 weeks
- Team Size: 3-4 developers
- Performance Target: <10ms decision latency (p99)
- Throughput Target: 10,000 decisions/sec

**Strategic Positioning:**
- Building upon existing Decision Engine foundation (A/B testing, reinforcement learning, Pareto optimization)
- Bridges Analyzer Engine (insights) → Actuator Engine (execution)
- Production-ready with comprehensive error handling, observability, and fail-safes
- Enterprise-grade with audit logging, compliance, and rollback capabilities

---

## 1. Architecture Overview

### 1.1 Component Relationships

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Decision Engine                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────┐           ┌──────────────────┐              │
│  │ Analyzer Engine  │──────────▶│ Decision Engine  │              │
│  │ (Input)          │           │   Coordinator    │              │
│  │                  │           │                  │              │
│  │ • Performance    │           │ • Strategy       │              │
│  │ • Cost           │           │   Selection      │              │
│  │ • Quality        │           │ • Policy Engine  │              │
│  │ • Anomaly        │           │ • Priority Queue │              │
│  │ • Drift          │           │ • Decision Cache │              │
│  └──────────────────┘           └──────────────────┘              │
│                                          │                         │
│                                          ▼                         │
│  ┌──────────────────────────────────────────────────────────────┐ │
│  │            5 Optimization Strategies                         │ │
│  ├──────────────────────────────────────────────────────────────┤ │
│  │                                                              │ │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │ │
│  │  │    Model     │  │   Caching    │  │     Rate     │     │ │
│  │  │  Selection   │  │   Strategy   │  │   Limiting   │     │ │
│  │  │              │  │              │  │              │     │ │
│  │  │ • Quality    │  │ • Semantic   │  │ • Token      │     │ │
│  │  │ • Cost       │  │ • Response   │  │   Budget     │     │ │
│  │  │ • Latency    │  │ • Embedding  │  │ • Throughput │     │ │
│  │  └──────────────┘  └──────────────┘  └──────────────┘     │ │
│  │                                                              │ │
│  │  ┌──────────────┐  ┌──────────────┐                        │ │
│  │  │   Request    │  │    Prompt    │                        │ │
│  │  │   Batching   │  │ Optimization │                        │ │
│  │  │              │  │              │                        │ │
│  │  │ • Dynamic    │  │ • Template   │                        │ │
│  │  │   Grouping   │  │   Selection  │                        │ │
│  │  │ • Priority   │  │ • Parameter  │                        │ │
│  │  │ • Timeout    │  │   Tuning     │                        │ │
│  │  └──────────────┘  └──────────────┘                        │ │
│  │                                                              │ │
│  └──────────────────────────────────────────────────────────────┘ │
│                                          │                         │
│                                          ▼                         │
│  ┌──────────────────┐           ┌──────────────────┐              │
│  │   Decision       │           │    Actuator      │              │
│  │   Validator      │──────────▶│    Engine        │              │
│  │                  │           │    (Output)      │              │
│  │ • Safety Checks  │           │                  │              │
│  │ • Constraints    │           │ • Canary Deploy  │              │
│  │ • Rollback Rules │           │ • Config Update  │              │
│  │ • Audit Logging  │           │ • Health Monitor │              │
│  └──────────────────┘           └──────────────────┘              │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.2 Data Flow

```
Input (AnalyzerResult)
      │
      ▼
┌─────────────────┐
│ Decision Engine │
│   Coordinator   │
└─────────────────┘
      │
      ├─────────────────────────────────┐
      │                                 │
      ▼                                 ▼
┌─────────────────┐           ┌─────────────────┐
│  Context        │           │   Policy        │
│  Builder        │           │   Engine        │
│                 │           │                 │
│ • Current State │           │ • Constraints   │
│ • History       │           │ • Priorities    │
│ • Constraints   │           │ • Rules         │
└─────────────────┘           └─────────────────┘
      │                                 │
      └────────────┬────────────────────┘
                   ▼
         ┌─────────────────┐
         │   Strategy      │
         │   Selector      │
         │                 │
         │ • Prioritize    │
         │ • Execute       │
         │ • Aggregate     │
         └─────────────────┘
                   │
      ┏━━━━━━━━━━┻━━━━━━━━━━┓
      ▼            ▼           ▼
[Model Sel] [Caching] [Rate Lim] [Batching] [Prompt Opt]
      │            │           │
      └────────────┼───────────┘
                   ▼
         ┌─────────────────┐
         │   Decision      │
         │   Aggregator    │
         │                 │
         │ • Merge Results │
         │ • Resolve       │
         │   Conflicts     │
         │ • Validate      │
         └─────────────────┘
                   │
                   ▼
         ┌─────────────────┐
         │   Validator     │
         │                 │
         │ • Safety Check  │
         │ • Constraints   │
         │ • Audit Log     │
         └─────────────────┘
                   │
                   ▼
Output (OptimizationDecision) → Actuator Engine
```

---

## 2. File Structure

### 2.1 Directory Layout

```
crates/decision/src/
├── engine/                          # Core decision engine (NEW)
│   ├── mod.rs                       # Module exports (80 LOC)
│   ├── coordinator.rs               # Main decision coordinator (650 LOC)
│   ├── traits.rs                    # Core traits (300 LOC)
│   ├── types.rs                     # Common types (400 LOC)
│   ├── context.rs                   # Decision context builder (380 LOC)
│   ├── policy.rs                    # Policy engine (450 LOC)
│   ├── validator.rs                 # Decision validator (420 LOC)
│   └── aggregator.rs                # Decision aggregator (380 LOC)
│
├── strategies/                      # 5 Optimization Strategies (NEW)
│   ├── mod.rs                       # Strategy exports (100 LOC)
│   ├── traits.rs                    # Strategy trait (200 LOC)
│   │
│   ├── model_selection/             # Strategy 1: Model Selection
│   │   ├── mod.rs                   # Exports (50 LOC)
│   │   ├── strategy.rs              # Core strategy (580 LOC)
│   │   ├── scorer.rs                # Model scoring (420 LOC)
│   │   ├── selector.rs              # Selection logic (350 LOC)
│   │   └── fallback.rs              # Fallback handling (280 LOC)
│   │
│   ├── caching/                     # Strategy 2: Caching
│   │   ├── mod.rs                   # Exports (50 LOC)
│   │   ├── strategy.rs              # Core strategy (520 LOC)
│   │   ├── semantic_cache.rs        # Semantic caching (480 LOC)
│   │   ├── embedding_cache.rs       # Embedding caching (380 LOC)
│   │   └── cache_policy.rs          # Cache policies (320 LOC)
│   │
│   ├── rate_limiting/               # Strategy 3: Rate Limiting
│   │   ├── mod.rs                   # Exports (50 LOC)
│   │   ├── strategy.rs              # Core strategy (480 LOC)
│   │   ├── token_bucket.rs          # Token bucket algorithm (380 LOC)
│   │   ├── adaptive_limiter.rs      # Adaptive rate limiting (420 LOC)
│   │   └── budget_manager.rs        # Budget management (350 LOC)
│   │
│   ├── batching/                    # Strategy 4: Request Batching
│   │   ├── mod.rs                   # Exports (50 LOC)
│   │   ├── strategy.rs              # Core strategy (550 LOC)
│   │   ├── batch_builder.rs         # Batch construction (450 LOC)
│   │   ├── priority_queue.rs        # Priority queueing (380 LOC)
│   │   └── timeout_manager.rs       # Timeout handling (320 LOC)
│   │
│   └── prompt_optimization/         # Strategy 5: Prompt Optimization
│       ├── mod.rs                   # Exports (50 LOC)
│       ├── strategy.rs              # Core strategy (580 LOC)
│       ├── template_selector.rs     # Template selection (420 LOC)
│       ├── parameter_tuner.rs       # Parameter optimization (450 LOC)
│       └── compression.rs           # Prompt compression (380 LOC)
│
├── integration/                     # Integration layer (NEW)
│   ├── mod.rs                       # Exports (60 LOC)
│   ├── analyzer_adapter.rs          # Analyzer input adapter (320 LOC)
│   └── actuator_adapter.rs          # Actuator output adapter (280 LOC)
│
├── errors.rs                        # Error types (ENHANCED - 180 LOC)
└── lib.rs                           # Main library (ENHANCED - 220 LOC)

# Existing modules (to be integrated)
├── ab_testing.rs                    # Existing A/B testing
├── thompson_sampling.rs             # Existing Thompson sampling
├── contextual_bandit.rs             # Existing contextual bandits
├── pareto.rs                        # Existing Pareto optimization
├── model_registry.rs                # Existing model registry
└── ...                              # Other existing modules
```

### 2.2 File Count and LOC Summary

**New Files:**
- Core Engine: 8 files, ~3,060 LOC
- Strategy 1 (Model Selection): 5 files, ~1,680 LOC
- Strategy 2 (Caching): 5 files, ~1,750 LOC
- Strategy 3 (Rate Limiting): 5 files, ~1,630 LOC
- Strategy 4 (Batching): 5 files, ~1,700 LOC
- Strategy 5 (Prompt Optimization): 5 files, ~1,880 LOC
- Integration: 3 files, ~660 LOC

**Total New Implementation:**
- **36 files**
- **~12,360 LOC** (production code)
- **~6,500 LOC** (tests)
- **~2,100 LOC** (documentation and examples)

**Enhanced Existing Files:**
- `errors.rs`: +100 LOC
- `lib.rs`: +150 LOC

**Grand Total:** ~21,110 LOC

---

## 3. Implementation Phases

### Phase 1: Core Framework (Week 1)

#### Goals
- Establish decision engine architecture
- Define core traits and interfaces
- Create context and policy management
- Set up testing infrastructure

#### Day 1-2: Core Traits and Types

**File:** `crates/decision/src/engine/traits.rs`

```rust
//! Core traits for decision engine

use async_trait::async_trait;
use crate::errors::Result;
use super::types::{DecisionContext, OptimizationDecision, StrategyResult};

/// Core trait for optimization strategies
#[async_trait]
pub trait OptimizationStrategy: Send + Sync {
    /// Get strategy name
    fn name(&self) -> &str;

    /// Get strategy priority (higher = executed first)
    fn priority(&self) -> i32;

    /// Check if strategy is applicable for the given context
    async fn is_applicable(&self, context: &DecisionContext) -> Result<bool>;

    /// Execute strategy and return optimization decision
    async fn execute(&self, context: &DecisionContext) -> Result<StrategyResult>;

    /// Estimate impact of applying this strategy
    async fn estimate_impact(&self, context: &DecisionContext) -> Result<ImpactEstimate>;

    /// Validate strategy configuration
    fn validate_config(&self) -> Result<()>;
}

/// Decision engine coordinator trait
#[async_trait]
pub trait DecisionEngine: Send + Sync {
    /// Register an optimization strategy
    async fn register_strategy(&mut self, strategy: Box<dyn OptimizationStrategy>) -> Result<()>;

    /// Make optimization decision based on analyzer input
    async fn decide(&self, context: DecisionContext) -> Result<OptimizationDecision>;

    /// Validate a decision before execution
    async fn validate_decision(&self, decision: &OptimizationDecision) -> Result<()>;

    /// Get engine statistics
    async fn stats(&self) -> Result<EngineStats>;
}

/// Impact estimation for a strategy
#[derive(Debug, Clone)]
pub struct ImpactEstimate {
    /// Expected cost reduction (0.0-1.0)
    pub cost_reduction: f64,
    /// Expected latency change (negative = improvement)
    pub latency_delta_ms: f64,
    /// Expected quality change (0.0-1.0)
    pub quality_delta: f64,
    /// Confidence in estimate (0.0-1.0)
    pub confidence: f64,
    /// Risk score (0.0-1.0, higher = riskier)
    pub risk: f64,
}

/// Engine statistics
#[derive(Debug, Clone)]
pub struct EngineStats {
    pub total_decisions: u64,
    pub successful_decisions: u64,
    pub failed_decisions: u64,
    pub avg_decision_latency_ms: f64,
    pub strategies_registered: usize,
}

/// Policy enforcement trait
#[async_trait]
pub trait PolicyEngine: Send + Sync {
    /// Check if a decision violates any policies
    async fn check_policies(&self, decision: &OptimizationDecision) -> Result<Vec<PolicyViolation>>;

    /// Get active constraints
    async fn get_constraints(&self) -> Result<Vec<Constraint>>;
}

/// Policy violation
#[derive(Debug, Clone)]
pub struct PolicyViolation {
    pub policy_name: String,
    pub severity: ViolationSeverity,
    pub message: String,
    pub can_override: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Constraint definition
#[derive(Debug, Clone)]
pub struct Constraint {
    pub name: String,
    pub constraint_type: ConstraintType,
    pub value: ConstraintValue,
}

#[derive(Debug, Clone)]
pub enum ConstraintType {
    MaxCostPerRequest,
    MaxLatencyMs,
    MinQualityScore,
    MaxTokensPerMinute,
    RequiredProvider,
    ForbiddenProvider,
}

#[derive(Debug, Clone)]
pub enum ConstraintValue {
    Float(f64),
    Integer(i64),
    String(String),
    StringList(Vec<String>),
}
```

**File:** `crates/decision/src/engine/types.rs`

```rust
//! Core types for decision engine

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::collections::HashMap;

use crate::model_registry::{Provider, ModelDefinition};

/// Decision context containing all information needed for decision-making
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionContext {
    /// Unique request ID
    pub request_id: Uuid,

    /// Timestamp of the decision request
    pub timestamp: DateTime<Utc>,

    /// Current analyzer results
    pub analyzer_results: AnalyzerResults,

    /// Historical context (last N decisions)
    pub history: Vec<HistoricalDecision>,

    /// User/tenant context
    pub user_context: UserContext,

    /// System constraints
    pub constraints: Vec<Constraint>,

    /// Request characteristics
    pub request_info: RequestInfo,

    /// Configuration overrides
    pub config_overrides: HashMap<String, serde_json::Value>,
}

/// Results from the analyzer engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzerResults {
    /// Performance metrics
    pub performance: PerformanceMetrics,

    /// Cost metrics
    pub cost: CostMetrics,

    /// Quality metrics
    pub quality: QualityMetrics,

    /// Detected anomalies
    pub anomalies: Vec<Anomaly>,

    /// Drift indicators
    pub drift: DriftMetrics,
}

/// Performance metrics from analyzer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub latency_p50_ms: f64,
    pub latency_p95_ms: f64,
    pub latency_p99_ms: f64,
    pub throughput_rps: f64,
    pub error_rate: f64,
    pub timeout_rate: f64,
}

/// Cost metrics from analyzer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostMetrics {
    pub cost_per_request: f64,
    pub tokens_per_request: f64,
    pub monthly_burn_rate: f64,
    pub budget_utilization: f64,
}

/// Quality metrics from analyzer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityMetrics {
    pub overall_score: f64,
    pub user_satisfaction: Option<f64>,
    pub task_success_rate: f64,
    pub hallucination_rate: Option<f64>,
}

/// Anomaly detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    pub anomaly_type: AnomalyType,
    pub severity: f64,
    pub description: String,
    pub detected_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnomalyType {
    LatencySpike,
    CostSpike,
    QualityDrop,
    ErrorRateIncrease,
    ThroughputDrop,
}

/// Drift metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftMetrics {
    pub is_drifting: bool,
    pub drift_magnitude: f64,
    pub drift_dimensions: Vec<String>,
}

/// Historical decision for context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalDecision {
    pub timestamp: DateTime<Utc>,
    pub decision: OptimizationDecision,
    pub outcome: DecisionOutcome,
}

/// User/tenant context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserContext {
    pub tenant_id: String,
    pub user_tier: UserTier,
    pub quota: Option<Quota>,
    pub preferences: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserTier {
    Free,
    Pro,
    Enterprise,
    Custom,
}

/// Resource quota
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quota {
    pub max_requests_per_minute: u32,
    pub max_tokens_per_minute: u32,
    pub max_cost_per_day: f64,
}

/// Request information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestInfo {
    pub estimated_tokens: u32,
    pub task_type: TaskType,
    pub priority: Priority,
    pub deadline: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskType {
    Chat,
    Completion,
    Embedding,
    Classification,
    Summarization,
    Translation,
    CodeGeneration,
    Other(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

/// Final optimization decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationDecision {
    /// Unique decision ID
    pub decision_id: Uuid,

    /// Request ID this decision is for
    pub request_id: Uuid,

    /// Timestamp of decision
    pub timestamp: DateTime<Utc>,

    /// Strategies that contributed to this decision
    pub applied_strategies: Vec<AppliedStrategy>,

    /// Recommended model
    pub model_selection: Option<ModelSelection>,

    /// Caching decision
    pub caching: Option<CachingDecision>,

    /// Rate limiting decision
    pub rate_limiting: Option<RateLimitingDecision>,

    /// Batching decision
    pub batching: Option<BatchingDecision>,

    /// Prompt optimization decision
    pub prompt_optimization: Option<PromptOptimizationDecision>,

    /// Overall confidence (0.0-1.0)
    pub confidence: f64,

    /// Estimated impact
    pub estimated_impact: ImpactEstimate,

    /// Metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Strategy application result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedStrategy {
    pub strategy_name: String,
    pub execution_time_ms: f64,
    pub success: bool,
    pub impact: ImpactEstimate,
}

/// Model selection decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSelection {
    pub selected_model: ModelDefinition,
    pub fallback_models: Vec<ModelDefinition>,
    pub reason: String,
    pub score: f64,
}

/// Caching decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachingDecision {
    pub should_cache: bool,
    pub cache_strategy: CacheStrategy,
    pub ttl_seconds: u64,
    pub cache_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheStrategy {
    Semantic { similarity_threshold: f64 },
    Exact,
    Embedding,
    Hybrid,
}

/// Rate limiting decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitingDecision {
    pub should_throttle: bool,
    pub delay_ms: u64,
    pub tokens_available: u64,
    pub quota_exceeded: bool,
}

/// Batching decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchingDecision {
    pub should_batch: bool,
    pub batch_id: Option<Uuid>,
    pub batch_size: usize,
    pub max_wait_ms: u64,
}

/// Prompt optimization decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptOptimizationDecision {
    pub optimized_prompt: String,
    pub template_id: Option<String>,
    pub parameters: HashMap<String, f64>,
    pub compression_applied: bool,
    pub token_reduction: u32,
}

/// Result of a strategy execution
#[derive(Debug, Clone)]
pub struct StrategyResult {
    pub strategy_name: String,
    pub success: bool,
    pub contribution: StrategyContribution,
    pub execution_time_ms: f64,
    pub error: Option<String>,
}

/// Strategy contribution to the final decision
#[derive(Debug, Clone)]
pub enum StrategyContribution {
    ModelSelection(ModelSelection),
    Caching(CachingDecision),
    RateLimiting(RateLimitingDecision),
    Batching(BatchingDecision),
    PromptOptimization(PromptOptimizationDecision),
}

/// Outcome of applying a decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionOutcome {
    pub actual_cost: f64,
    pub actual_latency_ms: f64,
    pub actual_quality: f64,
    pub errors: Vec<String>,
}

/// Impact estimate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactEstimate {
    pub cost_reduction: f64,
    pub latency_delta_ms: f64,
    pub quality_delta: f64,
    pub confidence: f64,
    pub risk: f64,
}
```

**Deliverables:**
- Core traits defined (`traits.rs`)
- Complete type system (`types.rs`)
- Comprehensive unit tests (15 tests)
- Documentation (100% coverage)

**Testing:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_context_creation() {
        let context = DecisionContext {
            request_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            // ... test data
        };
        assert!(context.request_id.to_string().len() > 0);
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Critical > Priority::High);
        assert!(Priority::High > Priority::Normal);
        assert!(Priority::Normal > Priority::Low);
    }

    #[test]
    fn test_impact_estimate_validation() {
        let estimate = ImpactEstimate {
            cost_reduction: 0.5,
            latency_delta_ms: -100.0,
            quality_delta: 0.0,
            confidence: 0.8,
            risk: 0.2,
        };
        assert!(estimate.confidence > 0.0 && estimate.confidence <= 1.0);
        assert!(estimate.risk >= 0.0 && estimate.risk <= 1.0);
    }
}
```

#### Day 3-4: Decision Coordinator

**File:** `crates/decision/src/engine/coordinator.rs`

```rust
//! Decision Engine Coordinator

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use uuid::Uuid;
use chrono::Utc;
use async_trait::async_trait;

use crate::errors::{Result, DecisionError};
use super::traits::{DecisionEngine, OptimizationStrategy, PolicyEngine};
use super::types::{
    DecisionContext, OptimizationDecision, StrategyResult,
    AppliedStrategy, EngineStats,
};
use super::validator::DecisionValidator;
use super::aggregator::DecisionAggregator;
use super::policy::DefaultPolicyEngine;

/// Configuration for the decision coordinator
#[derive(Debug, Clone)]
pub struct CoordinatorConfig {
    /// Maximum strategies to execute in parallel
    pub max_parallel_strategies: usize,

    /// Timeout for strategy execution (ms)
    pub strategy_timeout_ms: u64,

    /// Enable decision caching
    pub enable_caching: bool,

    /// Cache TTL (seconds)
    pub cache_ttl_seconds: u64,

    /// Maximum decision history to keep
    pub max_history_size: usize,

    /// Fail fast on strategy errors
    pub fail_fast: bool,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            max_parallel_strategies: 5,
            strategy_timeout_ms: 100,
            enable_caching: true,
            cache_ttl_seconds: 300,
            max_history_size: 1000,
            fail_fast: false,
        }
    }
}

/// Main decision engine coordinator
pub struct DecisionCoordinator {
    config: CoordinatorConfig,
    strategies: Arc<RwLock<Vec<Box<dyn OptimizationStrategy>>>>,
    policy_engine: Arc<dyn PolicyEngine>,
    validator: Arc<DecisionValidator>,
    aggregator: Arc<DecisionAggregator>,
    stats: Arc<RwLock<EngineStats>>,
    // Decision cache: context hash -> decision
    decision_cache: Arc<moka::future::Cache<u64, OptimizationDecision>>,
}

impl DecisionCoordinator {
    /// Create new decision coordinator
    pub async fn new(config: CoordinatorConfig) -> Result<Self> {
        let cache = moka::future::Cache::builder()
            .max_capacity(10_000)
            .time_to_live(std::time::Duration::from_secs(config.cache_ttl_seconds))
            .build();

        Ok(Self {
            config: config.clone(),
            strategies: Arc::new(RwLock::new(Vec::new())),
            policy_engine: Arc::new(DefaultPolicyEngine::new()),
            validator: Arc::new(DecisionValidator::new()),
            aggregator: Arc::new(DecisionAggregator::new()),
            stats: Arc::new(RwLock::new(EngineStats::default())),
            decision_cache: Arc::new(cache),
        })
    }

    /// Register a strategy
    async fn register_strategy_impl(&self, strategy: Box<dyn OptimizationStrategy>) -> Result<()> {
        // Validate strategy configuration
        strategy.validate_config()?;

        let mut strategies = self.strategies.write().await;
        let name = strategy.name().to_string();

        info!("Registering strategy: {}", name);
        strategies.push(strategy);

        // Sort by priority (highest first)
        strategies.sort_by(|a, b| b.priority().cmp(&a.priority()));

        Ok(())
    }

    /// Execute all applicable strategies
    async fn execute_strategies(&self, context: &DecisionContext) -> Result<Vec<StrategyResult>> {
        let strategies = self.strategies.read().await;
        let mut results = Vec::new();

        info!("Executing {} strategies", strategies.len());

        for strategy in strategies.iter() {
            // Check if strategy is applicable
            let is_applicable = match strategy.is_applicable(context).await {
                Ok(applicable) => applicable,
                Err(e) => {
                    warn!("Strategy {} applicability check failed: {}", strategy.name(), e);
                    if self.config.fail_fast {
                        return Err(e);
                    }
                    false
                }
            };

            if !is_applicable {
                debug!("Strategy {} not applicable, skipping", strategy.name());
                continue;
            }

            // Execute strategy with timeout
            let start = std::time::Instant::now();
            let result = tokio::time::timeout(
                std::time::Duration::from_millis(self.config.strategy_timeout_ms),
                strategy.execute(context)
            ).await;

            let elapsed = start.elapsed().as_millis() as f64;

            match result {
                Ok(Ok(strategy_result)) => {
                    info!("Strategy {} completed in {:.2}ms", strategy.name(), elapsed);
                    results.push(strategy_result);
                }
                Ok(Err(e)) => {
                    error!("Strategy {} failed: {}", strategy.name(), e);
                    if self.config.fail_fast {
                        return Err(e);
                    }
                    results.push(StrategyResult {
                        strategy_name: strategy.name().to_string(),
                        success: false,
                        contribution: StrategyContribution::Empty,
                        execution_time_ms: elapsed,
                        error: Some(e.to_string()),
                    });
                }
                Err(_) => {
                    error!("Strategy {} timed out after {}ms",
                           strategy.name(), self.config.strategy_timeout_ms);
                    if self.config.fail_fast {
                        return Err(DecisionError::StrategyTimeout(strategy.name().to_string()));
                    }
                }
            }
        }

        Ok(results)
    }

    /// Update engine statistics
    async fn update_stats(&self, success: bool, latency_ms: f64) {
        let mut stats = self.stats.write().await;
        stats.total_decisions += 1;
        if success {
            stats.successful_decisions += 1;
        } else {
            stats.failed_decisions += 1;
        }

        // Update rolling average
        let total = stats.total_decisions as f64;
        stats.avg_decision_latency_ms =
            (stats.avg_decision_latency_ms * (total - 1.0) + latency_ms) / total;
    }
}

#[async_trait]
impl DecisionEngine for DecisionCoordinator {
    async fn register_strategy(&mut self, strategy: Box<dyn OptimizationStrategy>) -> Result<()> {
        self.register_strategy_impl(strategy).await
    }

    async fn decide(&self, context: DecisionContext) -> Result<OptimizationDecision> {
        let start = std::time::Instant::now();

        // Check cache if enabled
        if self.config.enable_caching {
            let cache_key = context.cache_key();
            if let Some(cached) = self.decision_cache.get(&cache_key).await {
                debug!("Cache hit for request {}", context.request_id);
                return Ok(cached);
            }
        }

        // Execute strategies
        let strategy_results = self.execute_strategies(&context).await?;

        // Aggregate results
        let decision = self.aggregator.aggregate(
            context.request_id,
            strategy_results
        ).await?;

        // Validate decision
        self.validator.validate(&decision, &context).await?;

        // Check policies
        let violations = self.policy_engine.check_policies(&decision).await?;
        if !violations.is_empty() {
            error!("Policy violations detected: {:?}", violations);
            return Err(DecisionError::PolicyViolation(violations));
        }

        // Cache decision
        if self.config.enable_caching {
            let cache_key = context.cache_key();
            self.decision_cache.insert(cache_key, decision.clone()).await;
        }

        // Update stats
        let elapsed = start.elapsed().as_millis() as f64;
        self.update_stats(true, elapsed).await;

        info!("Decision {} completed in {:.2}ms", decision.decision_id, elapsed);

        Ok(decision)
    }

    async fn validate_decision(&self, decision: &OptimizationDecision) -> Result<()> {
        self.validator.validate(decision, &DecisionContext::default()).await
    }

    async fn stats(&self) -> Result<EngineStats> {
        Ok(self.stats.read().await.clone())
    }
}

impl DecisionContext {
    /// Generate cache key from context
    fn cache_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();
        // Hash relevant fields for cache key
        self.request_info.task_type.hash(&mut hasher);
        self.user_context.tenant_id.hash(&mut hasher);
        (self.request_info.estimated_tokens / 100).hash(&mut hasher); // Bucket tokens

        hasher.finish()
    }
}
```

**Deliverables:**
- Complete coordinator implementation
- Strategy execution pipeline
- Caching and statistics
- Integration tests (12 tests)

#### Day 5: Policy Engine and Validator

**File:** `crates/decision/src/engine/policy.rs` (450 LOC)
**File:** `crates/decision/src/engine/validator.rs` (420 LOC)
**File:** `crates/decision/src/engine/aggregator.rs` (380 LOC)

**Deliverables:**
- Policy enforcement
- Decision validation
- Result aggregation
- Unit tests (18 tests)

**Phase 1 Summary:**
- **LOC:** ~3,060
- **Tests:** 45
- **Files:** 8
- **Coverage:** >95%

---

### Phase 2: Model Selection Strategy (Week 2)

#### Goals
- Implement intelligent model selection based on quality/cost/latency
- Multi-objective optimization with Pareto frontier
- Fallback and retry logic
- Integration with existing model registry

#### Implementation Structure

**File:** `crates/decision/src/strategies/model_selection/strategy.rs`

```rust
//! Model Selection Strategy

use async_trait::async_trait;
use tracing::{info, debug};
use crate::errors::Result;
use crate::engine::traits::OptimizationStrategy;
use crate::engine::types::*;
use crate::model_registry::{ModelRegistry, Provider};
use super::scorer::ModelScorer;
use super::selector::ModelSelector;
use super::fallback::FallbackManager;

/// Configuration for model selection strategy
#[derive(Debug, Clone)]
pub struct ModelSelectionConfig {
    /// Quality weight (0.0-1.0)
    pub quality_weight: f64,

    /// Cost weight (0.0-1.0)
    pub cost_weight: f64,

    /// Latency weight (0.0-1.0)
    pub latency_weight: f64,

    /// Minimum quality threshold (0.0-1.0)
    pub min_quality: f64,

    /// Maximum cost per request (USD)
    pub max_cost: f64,

    /// Maximum latency (ms)
    pub max_latency_ms: f64,

    /// Number of fallback models
    pub num_fallbacks: usize,

    /// Enable Pareto optimization
    pub use_pareto: bool,

    /// Allowed providers (empty = all)
    pub allowed_providers: Vec<Provider>,

    /// Forbidden providers
    pub forbidden_providers: Vec<Provider>,
}

impl Default for ModelSelectionConfig {
    fn default() -> Self {
        Self {
            quality_weight: 0.4,
            cost_weight: 0.4,
            latency_weight: 0.2,
            min_quality: 0.7,
            max_cost: 1.0,
            max_latency_ms: 5000.0,
            num_fallbacks: 2,
            use_pareto: true,
            allowed_providers: Vec::new(),
            forbidden_providers: Vec::new(),
        }
    }
}

/// Model selection strategy
pub struct ModelSelectionStrategy {
    config: ModelSelectionConfig,
    registry: ModelRegistry,
    scorer: ModelScorer,
    selector: ModelSelector,
    fallback_manager: FallbackManager,
}

impl ModelSelectionStrategy {
    /// Create new model selection strategy
    pub fn new(config: ModelSelectionConfig) -> Result<Self> {
        let registry = ModelRegistry::new();
        let scorer = ModelScorer::new(config.clone());
        let selector = ModelSelector::new(config.clone());
        let fallback_manager = FallbackManager::new(config.num_fallbacks);

        Ok(Self {
            config,
            registry,
            scorer,
            selector,
            fallback_manager,
        })
    }

    /// Get candidate models based on context
    async fn get_candidates(&self, context: &DecisionContext) -> Result<Vec<ModelCandidate>> {
        let mut candidates = self.registry.get_all_models();

        // Filter by allowed/forbidden providers
        if !self.config.allowed_providers.is_empty() {
            candidates.retain(|m| self.config.allowed_providers.contains(&m.provider));
        }
        candidates.retain(|m| !self.config.forbidden_providers.contains(&m.provider));

        // Filter by hard constraints
        candidates.retain(|m| {
            let price = m.pricing.estimate_cost(
                context.request_info.estimated_tokens,
                context.request_info.estimated_tokens
            );
            price <= self.config.max_cost
        });

        // Convert to ModelCandidate with context-aware objectives
        let candidates = candidates.into_iter()
            .map(|model| self.scorer.create_candidate(&model, context))
            .collect();

        Ok(candidates)
    }

    /// Select best model using scoring or Pareto optimization
    async fn select_best_model(
        &self,
        candidates: Vec<ModelCandidate>,
        context: &DecisionContext
    ) -> Result<ModelSelection> {
        if candidates.is_empty() {
            return Err(DecisionError::NoViableModels);
        }

        let selected = if self.config.use_pareto {
            // Pareto frontier optimization
            self.selector.select_pareto(candidates, context).await?
        } else {
            // Weighted scoring
            self.selector.select_weighted(candidates, context).await?
        };

        // Get fallback models
        let fallbacks = self.fallback_manager.select_fallbacks(
            &selected,
            &candidates
        ).await?;

        Ok(ModelSelection {
            selected_model: selected.model,
            fallback_models: fallbacks,
            reason: format!("Selected based on Q:{:.2} C:{:.2} L:{:.2}",
                selected.quality, selected.cost, selected.latency),
            score: selected.overall_score,
        })
    }
}

#[async_trait]
impl OptimizationStrategy for ModelSelectionStrategy {
    fn name(&self) -> &str {
        "model_selection"
    }

    fn priority(&self) -> i32 {
        100 // Highest priority
    }

    async fn is_applicable(&self, _context: &DecisionContext) -> Result<bool> {
        // Always applicable
        Ok(true)
    }

    async fn execute(&self, context: &DecisionContext) -> Result<StrategyResult> {
        let start = std::time::Instant::now();

        info!("Executing model selection for request {}", context.request_id);

        // Get candidate models
        let candidates = self.get_candidates(context).await?;
        debug!("Found {} candidate models", candidates.len());

        // Select best model
        let selection = self.select_best_model(candidates, context).await?;

        info!("Selected model: {} (score: {:.3})",
              selection.selected_model.name, selection.score);

        let elapsed = start.elapsed().as_millis() as f64;

        Ok(StrategyResult {
            strategy_name: self.name().to_string(),
            success: true,
            contribution: StrategyContribution::ModelSelection(selection),
            execution_time_ms: elapsed,
            error: None,
        })
    }

    async fn estimate_impact(&self, context: &DecisionContext) -> Result<ImpactEstimate> {
        // Estimate impact based on current vs. recommended model
        let current_metrics = &context.analyzer_results;

        Ok(ImpactEstimate {
            cost_reduction: 0.3, // 30% expected cost reduction
            latency_delta_ms: -200.0, // 200ms improvement
            quality_delta: 0.05, // 5% quality improvement
            confidence: 0.85,
            risk: 0.15,
        })
    }

    fn validate_config(&self) -> Result<()> {
        // Validate weights sum to reasonable value
        let sum = self.config.quality_weight +
                  self.config.cost_weight +
                  self.config.latency_weight;

        if (sum - 1.0).abs() > 0.1 {
            return Err(DecisionError::InvalidConfig(
                format!("Weight sum {:.2} should be close to 1.0", sum)
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_model_selection_basic() {
        let config = ModelSelectionConfig::default();
        let strategy = ModelSelectionStrategy::new(config).unwrap();

        let context = create_test_context();
        let result = strategy.execute(&context).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.success);
        assert_eq!(result.strategy_name, "model_selection");
    }

    #[tokio::test]
    async fn test_provider_filtering() {
        let mut config = ModelSelectionConfig::default();
        config.allowed_providers = vec![Provider::Anthropic];

        let strategy = ModelSelectionStrategy::new(config).unwrap();
        let context = create_test_context();

        let candidates = strategy.get_candidates(&context).await.unwrap();
        assert!(candidates.iter().all(|c| c.provider == Provider::Anthropic));
    }

    fn create_test_context() -> DecisionContext {
        // Create test context...
    }
}
```

**Additional Files:**
- `scorer.rs`: Model scoring logic (420 LOC)
- `selector.rs`: Selection algorithms (350 LOC)
- `fallback.rs`: Fallback management (280 LOC)

**Phase 2 Deliverables:**
- Complete model selection strategy
- Pareto optimization integration
- Fallback handling
- 25+ tests
- **LOC:** ~1,680

---

### Phase 3: Caching Strategy (Week 3)

#### Goals
- Semantic caching with similarity matching
- Response caching with exact/fuzzy matching
- Embedding-based cache lookup
- Cache policy management (TTL, eviction)

#### Implementation Highlights

**File:** `crates/decision/src/strategies/caching/strategy.rs`

```rust
//! Caching Strategy

use async_trait::async_trait;
use crate::engine::traits::OptimizationStrategy;
use super::semantic_cache::SemanticCache;
use super::embedding_cache::EmbeddingCache;
use super::cache_policy::CachePolicy;

/// Caching strategy configuration
#[derive(Debug, Clone)]
pub struct CachingConfig {
    /// Enable semantic caching
    pub enable_semantic: bool,

    /// Semantic similarity threshold (0.0-1.0)
    pub similarity_threshold: f64,

    /// Enable exact caching
    pub enable_exact: bool,

    /// Enable embedding caching
    pub enable_embedding: bool,

    /// Default TTL (seconds)
    pub default_ttl: u64,

    /// Maximum cache size (entries)
    pub max_cache_size: usize,
}

pub struct CachingStrategy {
    config: CachingConfig,
    semantic_cache: SemanticCache,
    embedding_cache: EmbeddingCache,
    policy: CachePolicy,
}

impl CachingStrategy {
    pub fn new(config: CachingConfig) -> Result<Self> {
        Ok(Self {
            semantic_cache: SemanticCache::new(config.similarity_threshold)?,
            embedding_cache: EmbeddingCache::new()?,
            policy: CachePolicy::new(config.default_ttl, config.max_cache_size),
            config,
        })
    }

    async fn check_semantic_cache(&self, context: &DecisionContext) -> Option<CachedResponse> {
        // Semantic similarity matching
        self.semantic_cache.lookup(&context.request_info).await
    }

    async fn should_cache_response(&self, context: &DecisionContext) -> bool {
        // Determine if response should be cached
        self.policy.should_cache(context).await
    }
}

#[async_trait]
impl OptimizationStrategy for CachingStrategy {
    fn name(&self) -> &str {
        "caching"
    }

    fn priority(&self) -> i32 {
        90 // High priority, after model selection
    }

    async fn execute(&self, context: &DecisionContext) -> Result<StrategyResult> {
        // Check for cached response
        if let Some(cached) = self.check_semantic_cache(context).await {
            info!("Cache hit! Saved ${:.4}", cached.cost_saved);
            return Ok(StrategyResult {
                strategy_name: self.name().to_string(),
                success: true,
                contribution: StrategyContribution::Caching(CachingDecision {
                    should_cache: false,
                    cache_strategy: CacheStrategy::Semantic {
                        similarity_threshold: self.config.similarity_threshold
                    },
                    ttl_seconds: cached.ttl,
                    cache_key: Some(cached.key),
                }),
                execution_time_ms: 5.0,
                error: None,
            });
        }

        // Determine caching decision for new request
        let should_cache = self.should_cache_response(context).await;
        let ttl = self.policy.calculate_ttl(context).await;

        Ok(StrategyResult {
            strategy_name: self.name().to_string(),
            success: true,
            contribution: StrategyContribution::Caching(CachingDecision {
                should_cache,
                cache_strategy: CacheStrategy::Hybrid,
                ttl_seconds: ttl,
                cache_key: None,
            }),
            execution_time_ms: 8.0,
            error: None,
        })
    }

    async fn estimate_impact(&self, context: &DecisionContext) -> Result<ImpactEstimate> {
        let hit_probability = self.policy.estimate_hit_rate(context).await;

        Ok(ImpactEstimate {
            cost_reduction: hit_probability * 0.95, // 95% cost saving on cache hit
            latency_delta_ms: hit_probability * -1000.0, // 1s saved on cache hit
            quality_delta: 0.0,
            confidence: 0.7,
            risk: 0.1,
        })
    }

    fn validate_config(&self) -> Result<()> {
        if self.config.similarity_threshold < 0.0 || self.config.similarity_threshold > 1.0 {
            return Err(DecisionError::InvalidConfig(
                "similarity_threshold must be between 0 and 1".to_string()
            ));
        }
        Ok(())
    }
}
```

**Phase 3 Deliverables:**
- Semantic caching with similarity matching
- Embedding cache implementation
- Cache policy management
- 22+ tests
- **LOC:** ~1,750

---

### Phase 4: Rate Limiting Strategy (Week 3)

#### Goals
- Token bucket algorithm implementation
- Adaptive rate limiting based on system load
- Budget management and quota enforcement
- Per-tenant rate limiting

**File:** `crates/decision/src/strategies/rate_limiting/strategy.rs` (480 LOC)
**File:** `crates/decision/src/strategies/rate_limiting/token_bucket.rs` (380 LOC)
**File:** `crates/decision/src/strategies/rate_limiting/adaptive_limiter.rs` (420 LOC)
**File:** `crates/decision/src/strategies/rate_limiting/budget_manager.rs` (350 LOC)

**Key Features:**
- Token bucket with configurable refill rate
- Adaptive rate limiting based on error rates and latency
- Budget tracking per tenant/user
- Quota enforcement with soft/hard limits
- Priority-based token allocation

**Phase 4 Deliverables:**
- Complete rate limiting strategy
- Token bucket implementation
- Budget management
- 20+ tests
- **LOC:** ~1,630

---

### Phase 5: Request Batching Strategy (Week 4)

#### Goals
- Dynamic request batching based on workload
- Priority-based queue management
- Timeout handling for batch formation
- Batch optimization for throughput

**File:** `crates/decision/src/strategies/batching/strategy.rs` (550 LOC)

```rust
//! Request Batching Strategy

use async_trait::async_trait;
use crate::engine::traits::OptimizationStrategy;
use super::batch_builder::BatchBuilder;
use super::priority_queue::PriorityQueue;
use super::timeout_manager::TimeoutManager;

/// Batching strategy configuration
#[derive(Debug, Clone)]
pub struct BatchingConfig {
    /// Minimum batch size
    pub min_batch_size: usize,

    /// Maximum batch size
    pub max_batch_size: usize,

    /// Maximum wait time for batch formation (ms)
    pub max_wait_ms: u64,

    /// Enable priority-based batching
    pub enable_priority: bool,

    /// Group similar requests
    pub group_by_similarity: bool,
}

pub struct BatchingStrategy {
    config: BatchingConfig,
    batch_builder: BatchBuilder,
    priority_queue: PriorityQueue,
    timeout_manager: TimeoutManager,
}

impl BatchingStrategy {
    pub fn new(config: BatchingConfig) -> Result<Self> {
        Ok(Self {
            batch_builder: BatchBuilder::new(config.max_batch_size),
            priority_queue: PriorityQueue::new(),
            timeout_manager: TimeoutManager::new(config.max_wait_ms),
            config,
        })
    }

    async fn should_batch(&self, context: &DecisionContext) -> bool {
        // Batching beneficial for:
        // 1. High-throughput scenarios
        // 2. Non-urgent requests
        // 3. Similar request types

        context.request_info.priority <= Priority::Normal
            && context.analyzer_results.performance.throughput_rps > 100.0
    }

    async fn find_or_create_batch(&self, context: &DecisionContext) -> Result<BatchInfo> {
        // Try to find existing batch
        if let Some(batch) = self.batch_builder.find_compatible_batch(context).await? {
            return Ok(batch);
        }

        // Create new batch
        self.batch_builder.create_batch(context).await
    }
}

#[async_trait]
impl OptimizationStrategy for BatchingStrategy {
    fn name(&self) -> &str {
        "batching"
    }

    fn priority(&self) -> i32 {
        70 // Medium priority
    }

    async fn execute(&self, context: &DecisionContext) -> Result<StrategyResult> {
        if !self.should_batch(context).await {
            return Ok(StrategyResult {
                strategy_name: self.name().to_string(),
                success: true,
                contribution: StrategyContribution::Batching(BatchingDecision {
                    should_batch: false,
                    batch_id: None,
                    batch_size: 1,
                    max_wait_ms: 0,
                }),
                execution_time_ms: 2.0,
                error: None,
            });
        }

        let batch = self.find_or_create_batch(context).await?;

        Ok(StrategyResult {
            strategy_name: self.name().to_string(),
            success: true,
            contribution: StrategyContribution::Batching(BatchingDecision {
                should_batch: true,
                batch_id: Some(batch.id),
                batch_size: batch.size,
                max_wait_ms: batch.max_wait_ms,
            }),
            execution_time_ms: 5.0,
            error: None,
        })
    }

    async fn estimate_impact(&self, context: &DecisionContext) -> Result<ImpactEstimate> {
        let batch_efficiency = 0.7; // 70% efficiency gain from batching

        Ok(ImpactEstimate {
            cost_reduction: 0.2 * batch_efficiency,
            latency_delta_ms: 50.0, // Small latency increase due to wait time
            quality_delta: 0.0,
            confidence: 0.8,
            risk: 0.2,
        })
    }

    fn validate_config(&self) -> Result<()> {
        if self.config.min_batch_size > self.config.max_batch_size {
            return Err(DecisionError::InvalidConfig(
                "min_batch_size must be <= max_batch_size".to_string()
            ));
        }
        Ok(())
    }
}
```

**Phase 5 Deliverables:**
- Request batching implementation
- Priority queue management
- Timeout handling
- 20+ tests
- **LOC:** ~1,700

---

### Phase 6: Prompt Optimization Strategy (Week 4-5)

#### Goals
- Template selection based on performance history
- Parameter tuning (temperature, top_p, max_tokens)
- Prompt compression to reduce token usage
- Integration with A/B testing framework

**File:** `crates/decision/src/strategies/prompt_optimization/strategy.rs` (580 LOC)

```rust
//! Prompt Optimization Strategy

use async_trait::async_trait;
use crate::engine::traits::OptimizationStrategy;
use crate::adaptive_params::AdaptiveParameterTuner;
use super::template_selector::TemplateSelector;
use super::parameter_tuner::ParameterTuner;
use super::compression::PromptCompressor;

/// Prompt optimization configuration
#[derive(Debug, Clone)]
pub struct PromptOptimizationConfig {
    /// Enable template optimization
    pub enable_template_optimization: bool,

    /// Enable parameter tuning
    pub enable_parameter_tuning: bool,

    /// Enable prompt compression
    pub enable_compression: bool,

    /// Target compression ratio (0.0-1.0)
    pub target_compression: f64,

    /// Minimum quality after optimization
    pub min_quality: f64,
}

pub struct PromptOptimizationStrategy {
    config: PromptOptimizationConfig,
    template_selector: TemplateSelector,
    parameter_tuner: ParameterTuner,
    compressor: PromptCompressor,
}

impl PromptOptimizationStrategy {
    pub fn new(config: PromptOptimizationConfig) -> Result<Self> {
        Ok(Self {
            template_selector: TemplateSelector::new()?,
            parameter_tuner: ParameterTuner::new()?,
            compressor: PromptCompressor::new(config.target_compression),
            config,
        })
    }

    async fn optimize_template(&self, context: &DecisionContext) -> Result<Option<String>> {
        if !self.config.enable_template_optimization {
            return Ok(None);
        }

        self.template_selector.select_best_template(context).await
    }

    async fn optimize_parameters(&self, context: &DecisionContext) -> Result<HashMap<String, f64>> {
        if !self.config.enable_parameter_tuning {
            return Ok(HashMap::new());
        }

        self.parameter_tuner.tune_parameters(context).await
    }

    async fn compress_prompt(&self, prompt: &str) -> Result<(String, u32)> {
        if !self.config.enable_compression {
            return Ok((prompt.to_string(), 0));
        }

        self.compressor.compress(prompt).await
    }
}

#[async_trait]
impl OptimizationStrategy for PromptOptimizationStrategy {
    fn name(&self) -> &str {
        "prompt_optimization"
    }

    fn priority(&self) -> i32 {
        80 // High priority
    }

    async fn execute(&self, context: &DecisionContext) -> Result<StrategyResult> {
        let mut optimized_prompt = String::new();
        let mut parameters = HashMap::new();
        let mut token_reduction = 0u32;

        // Select best template
        if let Some(template) = self.optimize_template(context).await? {
            optimized_prompt = template;
        }

        // Optimize parameters
        parameters = self.optimize_parameters(context).await?;

        // Compress prompt
        if !optimized_prompt.is_empty() {
            let (compressed, reduction) = self.compress_prompt(&optimized_prompt).await?;
            optimized_prompt = compressed;
            token_reduction = reduction;
        }

        Ok(StrategyResult {
            strategy_name: self.name().to_string(),
            success: true,
            contribution: StrategyContribution::PromptOptimization(PromptOptimizationDecision {
                optimized_prompt,
                template_id: None,
                parameters,
                compression_applied: token_reduction > 0,
                token_reduction,
            }),
            execution_time_ms: 15.0,
            error: None,
        })
    }

    async fn estimate_impact(&self, context: &DecisionContext) -> Result<ImpactEstimate> {
        let compression_ratio = self.config.target_compression;

        Ok(ImpactEstimate {
            cost_reduction: compression_ratio * 0.8, // Cost reduction from token savings
            latency_delta_ms: -50.0, // Faster due to fewer tokens
            quality_delta: -0.05, // Small quality trade-off
            confidence: 0.75,
            risk: 0.25,
        })
    }

    fn validate_config(&self) -> Result<()> {
        if self.config.target_compression < 0.0 || self.config.target_compression > 1.0 {
            return Err(DecisionError::InvalidConfig(
                "target_compression must be between 0 and 1".to_string()
            ));
        }
        Ok(())
    }
}
```

**Phase 6 Deliverables:**
- Prompt optimization strategy
- Template selection
- Parameter tuning
- Prompt compression
- 23+ tests
- **LOC:** ~1,880

---

### Phase 7: Integration and Testing (Week 5-6)

#### Goals
- Integration with Analyzer Engine
- Integration with Actuator Engine
- End-to-end testing
- Performance benchmarking
- Documentation

#### Integration Adapters

**File:** `crates/decision/src/integration/analyzer_adapter.rs`

```rust
//! Adapter for Analyzer Engine integration

use crate::engine::types::{DecisionContext, AnalyzerResults};
use crate::errors::Result;

/// Adapter to convert analyzer output to decision context
pub struct AnalyzerAdapter;

impl AnalyzerAdapter {
    pub fn create_context(
        analyzer_output: AnalyzerOutput,
        request_id: Uuid,
        user_context: UserContext,
    ) -> Result<DecisionContext> {
        Ok(DecisionContext {
            request_id,
            timestamp: Utc::now(),
            analyzer_results: Self::convert_results(analyzer_output)?,
            history: Vec::new(),
            user_context,
            constraints: Vec::new(),
            request_info: RequestInfo::default(),
            config_overrides: HashMap::new(),
        })
    }

    fn convert_results(output: AnalyzerOutput) -> Result<AnalyzerResults> {
        // Convert analyzer output to decision context format
        Ok(AnalyzerResults {
            performance: PerformanceMetrics {
                latency_p50_ms: output.performance.p50,
                latency_p95_ms: output.performance.p95,
                latency_p99_ms: output.performance.p99,
                throughput_rps: output.performance.throughput,
                error_rate: output.performance.error_rate,
                timeout_rate: output.performance.timeout_rate,
            },
            cost: CostMetrics {
                cost_per_request: output.cost.avg_cost,
                tokens_per_request: output.cost.avg_tokens as f64,
                monthly_burn_rate: output.cost.monthly_projection,
                budget_utilization: output.cost.budget_used_pct,
            },
            quality: QualityMetrics {
                overall_score: output.quality.score,
                user_satisfaction: output.quality.satisfaction,
                task_success_rate: output.quality.success_rate,
                hallucination_rate: output.quality.hallucination_rate,
            },
            anomalies: output.anomalies,
            drift: output.drift,
        })
    }
}
```

**File:** `crates/decision/src/integration/actuator_adapter.rs`

```rust
//! Adapter for Actuator Engine integration

use crate::engine::types::OptimizationDecision;
use crate::errors::Result;

/// Adapter to convert decision to actuator commands
pub struct ActuatorAdapter;

impl ActuatorAdapter {
    pub fn create_commands(decision: OptimizationDecision) -> Result<Vec<ActuatorCommand>> {
        let mut commands = Vec::new();

        // Model selection command
        if let Some(model_sel) = decision.model_selection {
            commands.push(ActuatorCommand::SwitchModel {
                model: model_sel.selected_model,
                fallbacks: model_sel.fallback_models,
            });
        }

        // Caching command
        if let Some(caching) = decision.caching {
            commands.push(ActuatorCommand::UpdateCache {
                strategy: caching.cache_strategy,
                ttl: caching.ttl_seconds,
            });
        }

        // Rate limiting command
        if let Some(rate_limit) = decision.rate_limiting {
            if rate_limit.should_throttle {
                commands.push(ActuatorCommand::ApplyRateLimit {
                    delay_ms: rate_limit.delay_ms,
                });
            }
        }

        // Batching command
        if let Some(batching) = decision.batching {
            if batching.should_batch {
                commands.push(ActuatorCommand::AddToBatch {
                    batch_id: batching.batch_id.unwrap(),
                    max_wait_ms: batching.max_wait_ms,
                });
            }
        }

        // Prompt optimization command
        if let Some(prompt_opt) = decision.prompt_optimization {
            commands.push(ActuatorCommand::OptimizePrompt {
                optimized_prompt: prompt_opt.optimized_prompt,
                parameters: prompt_opt.parameters,
            });
        }

        Ok(commands)
    }
}

#[derive(Debug, Clone)]
pub enum ActuatorCommand {
    SwitchModel {
        model: ModelDefinition,
        fallbacks: Vec<ModelDefinition>,
    },
    UpdateCache {
        strategy: CacheStrategy,
        ttl: u64,
    },
    ApplyRateLimit {
        delay_ms: u64,
    },
    AddToBatch {
        batch_id: Uuid,
        max_wait_ms: u64,
    },
    OptimizePrompt {
        optimized_prompt: String,
        parameters: HashMap<String, f64>,
    },
}
```

#### End-to-End Test

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_full_decision_pipeline() {
        // Setup
        let mut coordinator = DecisionCoordinator::new(CoordinatorConfig::default())
            .await
            .unwrap();

        // Register all strategies
        coordinator.register_strategy(
            Box::new(ModelSelectionStrategy::new(ModelSelectionConfig::default()).unwrap())
        ).await.unwrap();

        coordinator.register_strategy(
            Box::new(CachingStrategy::new(CachingConfig::default()).unwrap())
        ).await.unwrap();

        coordinator.register_strategy(
            Box::new(RateLimitingStrategy::new(RateLimitingConfig::default()).unwrap())
        ).await.unwrap();

        coordinator.register_strategy(
            Box::new(BatchingStrategy::new(BatchingConfig::default()).unwrap())
        ).await.unwrap();

        coordinator.register_strategy(
            Box::new(PromptOptimizationStrategy::new(PromptOptimizationConfig::default()).unwrap())
        ).await.unwrap();

        // Create context
        let context = create_test_context();

        // Execute decision
        let decision = coordinator.decide(context).await.unwrap();

        // Verify decision
        assert!(decision.confidence > 0.0);
        assert_eq!(decision.applied_strategies.len(), 5);
        assert!(decision.model_selection.is_some());

        // Verify all strategies executed
        let strategy_names: Vec<_> = decision.applied_strategies
            .iter()
            .map(|s| s.strategy_name.as_str())
            .collect();

        assert!(strategy_names.contains(&"model_selection"));
        assert!(strategy_names.contains(&"caching"));
        assert!(strategy_names.contains(&"rate_limiting"));
        assert!(strategy_names.contains(&"batching"));
        assert!(strategy_names.contains(&"prompt_optimization"));

        // Convert to actuator commands
        let commands = ActuatorAdapter::create_commands(decision).unwrap();
        assert!(!commands.is_empty());
    }

    #[tokio::test]
    async fn test_analyzer_integration() {
        // Test analyzer output conversion
        let analyzer_output = create_mock_analyzer_output();
        let context = AnalyzerAdapter::create_context(
            analyzer_output,
            Uuid::new_v4(),
            UserContext::default(),
        ).unwrap();

        assert!(context.analyzer_results.performance.latency_p95_ms > 0.0);
        assert!(context.analyzer_results.cost.cost_per_request > 0.0);
    }

    #[tokio::test]
    async fn test_performance_targets() {
        let coordinator = create_test_coordinator().await;
        let context = create_test_context();

        let start = std::time::Instant::now();
        let decision = coordinator.decide(context).await.unwrap();
        let elapsed = start.elapsed();

        // Verify performance targets
        assert!(elapsed.as_millis() < 10, "Decision latency > 10ms target");
        assert!(decision.confidence > 0.5, "Low confidence decision");
    }
}
```

**Phase 7 Deliverables:**
- Analyzer integration adapter
- Actuator integration adapter
- End-to-end tests (30+ tests)
- Performance benchmarks
- Integration documentation
- **LOC:** ~660

---

## 4. Module Interfaces

### 4.1 Public API

```rust
// Main entry point
pub use crate::engine::coordinator::{DecisionCoordinator, CoordinatorConfig};
pub use crate::engine::traits::{DecisionEngine, OptimizationStrategy};
pub use crate::engine::types::{
    DecisionContext, OptimizationDecision, StrategyResult,
    ModelSelection, CachingDecision, RateLimitingDecision,
    BatchingDecision, PromptOptimizationDecision,
};

// Strategies
pub use crate::strategies::model_selection::{
    ModelSelectionStrategy, ModelSelectionConfig,
};
pub use crate::strategies::caching::{
    CachingStrategy, CachingConfig,
};
pub use crate::strategies::rate_limiting::{
    RateLimitingStrategy, RateLimitingConfig,
};
pub use crate::strategies::batching::{
    BatchingStrategy, BatchingConfig,
};
pub use crate::strategies::prompt_optimization::{
    PromptOptimizationStrategy, PromptOptimizationConfig,
};

// Integration
pub use crate::integration::{AnalyzerAdapter, ActuatorAdapter};
```

### 4.2 Usage Example

```rust
use llm_optimizer_decision::{
    DecisionCoordinator, CoordinatorConfig,
    ModelSelectionStrategy, ModelSelectionConfig,
    CachingStrategy, CachingConfig,
    DecisionContext, UserContext,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize coordinator
    let mut coordinator = DecisionCoordinator::new(
        CoordinatorConfig::default()
    ).await?;

    // Register strategies
    coordinator.register_strategy(
        Box::new(ModelSelectionStrategy::new(
            ModelSelectionConfig::default()
        )?)
    ).await?;

    coordinator.register_strategy(
        Box::new(CachingStrategy::new(
            CachingConfig::default()
        )?)
    ).await?;

    // Create decision context from analyzer
    let context = DecisionContext {
        request_id: Uuid::new_v4(),
        timestamp: Utc::now(),
        analyzer_results: get_analyzer_results(),
        user_context: UserContext::default(),
        // ... other fields
    };

    // Make decision
    let decision = coordinator.decide(context).await?;

    println!("Decision: {:?}", decision);
    println!("Confidence: {:.2}", decision.confidence);
    println!("Estimated cost reduction: {:.1}%",
             decision.estimated_impact.cost_reduction * 100.0);

    Ok(())
}
```

---

## 5. Testing Strategy

### 5.1 Unit Tests

**Target:** 120+ unit tests, >92% code coverage

**Test Categories:**

1. **Core Engine Tests (45 tests)**
   - Trait implementations
   - Type conversions
   - Context building
   - Policy enforcement
   - Decision validation
   - Result aggregation

2. **Strategy Tests (60 tests)**
   - Model Selection: 25 tests
   - Caching: 22 tests
   - Rate Limiting: 20 tests
   - Batching: 20 tests
   - Prompt Optimization: 23 tests

3. **Integration Tests (30 tests)**
   - Analyzer adapter
   - Actuator adapter
   - End-to-end pipeline
   - Performance benchmarks

4. **Error Handling Tests (15 tests)**
   - Strategy failures
   - Timeout handling
   - Policy violations
   - Invalid configurations

### 5.2 Integration Tests

```rust
#[tokio::test]
async fn test_high_cost_scenario() {
    // Scenario: High cost detected, should recommend cheaper model
    let context = DecisionContext {
        analyzer_results: AnalyzerResults {
            cost: CostMetrics {
                cost_per_request: 5.0, // Very expensive
                budget_utilization: 0.95,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };

    let coordinator = create_test_coordinator().await;
    let decision = coordinator.decide(context).await.unwrap();

    // Should recommend cheaper model
    assert!(decision.model_selection.is_some());
    let model = decision.model_selection.unwrap();
    assert!(model.selected_model.pricing.input_price_per_1k < 0.01);
}

#[tokio::test]
async fn test_high_latency_scenario() {
    // Scenario: High latency, should recommend faster model or caching
    let context = create_context_with_high_latency();
    let coordinator = create_test_coordinator().await;
    let decision = coordinator.decide(context).await.unwrap();

    // Should recommend faster model or enable caching
    assert!(
        decision.model_selection.is_some() ||
        decision.caching.map_or(false, |c| c.should_cache)
    );
}

#[tokio::test]
async fn test_budget_exceeded_scenario() {
    // Scenario: Budget exceeded, should throttle
    let context = create_context_with_exceeded_budget();
    let coordinator = create_test_coordinator().await;
    let decision = coordinator.decide(context).await.unwrap();

    // Should apply rate limiting
    assert!(decision.rate_limiting.is_some());
    let rate_limit = decision.rate_limiting.unwrap();
    assert!(rate_limit.should_throttle);
}
```

### 5.3 Performance Benchmarks

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_decision_latency(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let coordinator = rt.block_on(async {
        create_test_coordinator().await
    });

    c.bench_function("decision_latency", |b| {
        b.to_async(&rt).iter(|| async {
            let context = create_test_context();
            let decision = coordinator.decide(black_box(context)).await.unwrap();
            black_box(decision);
        });
    });
}

fn bench_strategy_execution(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("model_selection", |b| {
        b.to_async(&rt).iter(|| async {
            let strategy = ModelSelectionStrategy::new(
                ModelSelectionConfig::default()
            ).unwrap();
            let context = create_test_context();
            let result = strategy.execute(black_box(&context)).await.unwrap();
            black_box(result);
        });
    });
}

criterion_group!(benches, bench_decision_latency, bench_strategy_execution);
criterion_main!(benches);
```

**Performance Targets:**
- Decision latency: <10ms (p99)
- Strategy execution: <5ms each (p99)
- Throughput: 10,000 decisions/sec
- Memory usage: <100MB (idle)

---

## 6. Lines of Code Estimates

### 6.1 Detailed Breakdown

| Component | Files | Production LOC | Test LOC | Total LOC |
|-----------|-------|----------------|----------|-----------|
| **Core Engine** | 8 | 3,060 | 1,800 | 4,860 |
| - Coordinator | 1 | 650 | 300 | 950 |
| - Traits | 1 | 300 | 150 | 450 |
| - Types | 1 | 400 | 200 | 600 |
| - Context | 1 | 380 | 200 | 580 |
| - Policy | 1 | 450 | 250 | 700 |
| - Validator | 1 | 420 | 250 | 670 |
| - Aggregator | 1 | 380 | 200 | 580 |
| - Module | 1 | 80 | 250 | 330 |
| **Strategies** | 26 | 8,640 | 4,200 | 12,840 |
| - Model Selection | 5 | 1,680 | 850 | 2,530 |
| - Caching | 5 | 1,750 | 800 | 2,550 |
| - Rate Limiting | 5 | 1,630 | 750 | 2,380 |
| - Batching | 5 | 1,700 | 800 | 2,500 |
| - Prompt Optimization | 5 | 1,880 | 1,000 | 2,880 |
| - Module | 1 | 100 | - | 100 |
| **Integration** | 3 | 660 | 500 | 1,160 |
| **Enhanced Files** | 2 | 250 | - | 250 |
| **Total** | **36** | **12,360** | **6,500** | **18,860** |

### 6.2 Additional Documentation

| Document Type | LOC |
|---------------|-----|
| Inline documentation | 2,500 |
| README examples | 400 |
| API documentation | 800 |
| **Total** | **3,700** |

### 6.3 Grand Total

**Total Implementation: ~22,560 LOC**
- Production code: 12,360 LOC
- Test code: 6,500 LOC
- Documentation: 3,700 LOC

---

## 7. Dependencies

### 7.1 Internal Dependencies

```toml
[dependencies]
# Internal crates
llm-optimizer-types = { workspace = true }
llm-optimizer-config = { workspace = true }
llm-optimizer-analyzer = { workspace = true }  # NEW - for integration
```

### 7.2 External Dependencies (Already in workspace)

```toml
[dependencies]
# Async
tokio = { workspace = true }
async-trait = { workspace = true }

# Serialization
serde = { workspace = true }
serde_json = { workspace = true }

# Statistics
statrs = { workspace = true }
rand = { workspace = true }
rand_distr = { workspace = true }

# Utilities
uuid = { workspace = true }
chrono = { workspace = true }
thiserror = { workspace = true }
anyhow = { workspace = true }
dashmap = { workspace = true }
moka = { workspace = true }  # For decision caching
tracing = { workspace = true }

[dev-dependencies]
criterion = { workspace = true }
approx = "0.5"
```

### 7.3 New Dependencies Required

```toml
[dependencies]
# Semantic similarity for caching
fastembed = "3.0"  # Fast embedding generation
rust-bert = "0.21"  # Transformer models for embeddings

# Token bucket for rate limiting
leaky-bucket = "1.0"

# Priority queue for batching
priority-queue = "1.3"
```

---

## 8. Risk Mitigation

### 8.1 Technical Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| **Performance degradation** | High | Medium | - Comprehensive benchmarking<br>- Caching decisions<br>- Parallel strategy execution<br>- Timeouts on strategies |
| **Strategy conflicts** | Medium | Medium | - Clear priority system<br>- Decision aggregation logic<br>- Conflict resolution rules<br>- Validator checks |
| **Integration failures** | High | Low | - Well-defined interfaces<br>- Adapter pattern<br>- Extensive integration tests<br>- Backward compatibility |
| **Incorrect decisions** | High | Medium | - Multi-strategy validation<br>- Confidence scoring<br>- Safe rollback mechanism<br>- Audit logging |
| **Memory leaks** | Medium | Low | - Bounded caches<br>- Resource cleanup<br>- Memory profiling<br>- Load testing |

### 8.2 Implementation Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| **Scope creep** | Medium | High | - Fixed feature set<br>- Clear acceptance criteria<br>- Regular reviews |
| **Timeline delays** | Medium | Medium | - Phased approach<br>- Parallel development<br>- Buffer time |
| **Knowledge gaps** | Low | Medium | - Documentation<br>- Code reviews<br>- Pair programming |
| **Testing gaps** | High | Low | - >92% coverage target<br>- Integration tests<br>- Benchmarks |

### 8.3 Mitigation Strategies

**1. Performance Protection**
```rust
// Example: Strategy timeout and circuit breaker
pub struct StrategyGuard {
    timeout: Duration,
    circuit_breaker: CircuitBreaker,
}

impl StrategyGuard {
    async fn execute_with_protection<F, T>(
        &self,
        strategy_name: &str,
        f: F
    ) -> Result<T>
    where
        F: Future<Output = Result<T>>,
    {
        // Check circuit breaker
        if !self.circuit_breaker.is_closed(strategy_name) {
            return Err(DecisionError::CircuitOpen(strategy_name.to_string()));
        }

        // Execute with timeout
        match tokio::time::timeout(self.timeout, f).await {
            Ok(Ok(result)) => {
                self.circuit_breaker.record_success(strategy_name);
                Ok(result)
            }
            Ok(Err(e)) => {
                self.circuit_breaker.record_failure(strategy_name);
                Err(e)
            }
            Err(_) => {
                self.circuit_breaker.record_failure(strategy_name);
                Err(DecisionError::StrategyTimeout(strategy_name.to_string()))
            }
        }
    }
}
```

**2. Decision Validation**
```rust
pub struct DecisionValidator {
    rules: Vec<Box<dyn ValidationRule>>,
}

impl DecisionValidator {
    pub async fn validate(
        &self,
        decision: &OptimizationDecision,
        context: &DecisionContext,
    ) -> Result<()> {
        for rule in &self.rules {
            rule.validate(decision, context)?;
        }
        Ok(())
    }
}

trait ValidationRule: Send + Sync {
    fn validate(
        &self,
        decision: &OptimizationDecision,
        context: &DecisionContext,
    ) -> Result<()>;
}

// Example: Cost validation
struct CostValidationRule {
    max_cost_increase: f64,
}

impl ValidationRule for CostValidationRule {
    fn validate(&self, decision: &OptimizationDecision, context: &DecisionContext) -> Result<()> {
        if decision.estimated_impact.cost_reduction < -self.max_cost_increase {
            return Err(DecisionError::CostIncreaseExceeded);
        }
        Ok(())
    }
}
```

**3. Audit Logging**
```rust
pub struct AuditLogger {
    backend: Arc<dyn AuditBackend>,
}

impl AuditLogger {
    pub async fn log_decision(
        &self,
        decision: &OptimizationDecision,
        context: &DecisionContext,
    ) -> Result<()> {
        let audit_entry = AuditEntry {
            timestamp: Utc::now(),
            decision_id: decision.decision_id,
            request_id: context.request_id,
            strategies_applied: decision.applied_strategies.clone(),
            confidence: decision.confidence,
            estimated_impact: decision.estimated_impact.clone(),
            user_context: context.user_context.clone(),
        };

        self.backend.write(audit_entry).await?;
        Ok(())
    }
}
```

---

## 9. Validation Criteria

### 9.1 Functional Requirements

| Requirement | Validation Method | Success Criteria |
|-------------|-------------------|------------------|
| **FR-1: Model Selection** | Unit + Integration tests | - Selects optimal model 95% of time<br>- Respects constraints 100%<br>- Fallback selection works |
| **FR-2: Caching** | Unit + Integration tests | - Cache hit rate >40%<br>- Semantic matching accurate<br>- TTL respected |
| **FR-3: Rate Limiting** | Unit + Load tests | - Budget enforced 100%<br>- Fair allocation<br>- No quota violations |
| **FR-4: Batching** | Unit + Integration tests | - Batch efficiency >60%<br>- Timeout respected<br>- Priority ordering correct |
| **FR-5: Prompt Optimization** | Unit + A/B tests | - Token reduction >20%<br>- Quality maintained >95%<br>- Template selection accurate |

### 9.2 Non-Functional Requirements

| Requirement | Validation Method | Success Criteria |
|-------------|-------------------|------------------|
| **NFR-1: Performance** | Benchmarks | - Decision latency <10ms (p99)<br>- Throughput >10K decisions/sec<br>- Memory <100MB idle |
| **NFR-2: Reliability** | Stress tests | - No crashes under load<br>- Graceful degradation<br>- Error recovery |
| **NFR-3: Scalability** | Load tests | - Linear scaling to 100K RPS<br>- Horizontal scaling<br>- No bottlenecks |
| **NFR-4: Maintainability** | Code review | - >92% test coverage<br>- Clear documentation<br>- Modular design |

### 9.3 Business Requirements

| Requirement | Validation Method | Success Criteria |
|-------------|-------------------|------------------|
| **BR-1: Cost Reduction** | Production metrics | - 30-60% cost reduction<br>- ROI >300% in 6 months |
| **BR-2: Quality Maintenance** | User feedback | - Quality score >4.5/5<br>- Success rate >95% |
| **BR-3: Production Readiness** | Security audit | - Zero critical vulnerabilities<br>- Compliance met<br>- Audit logging complete |

### 9.4 Acceptance Tests

```rust
#[tokio::test]
async fn acceptance_test_cost_reduction() {
    // Setup: High-cost baseline
    let baseline_cost = 10.0; // $10 per request

    let coordinator = create_production_coordinator().await;
    let context = create_high_cost_context(baseline_cost);

    // Execute decision
    let decision = coordinator.decide(context).await.unwrap();

    // Validate: Should reduce cost by at least 30%
    let expected_reduction = 0.30;
    assert!(
        decision.estimated_impact.cost_reduction >= expected_reduction,
        "Cost reduction {:.1}% below target {:.1}%",
        decision.estimated_impact.cost_reduction * 100.0,
        expected_reduction * 100.0
    );
}

#[tokio::test]
async fn acceptance_test_latency_target() {
    let coordinator = create_production_coordinator().await;

    // Run 1000 decisions and measure p99 latency
    let mut latencies = Vec::new();
    for _ in 0..1000 {
        let context = create_random_context();
        let start = std::time::Instant::now();
        let _ = coordinator.decide(context).await.unwrap();
        latencies.push(start.elapsed().as_millis() as f64);
    }

    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p99 = latencies[(latencies.len() * 99) / 100];

    assert!(p99 < 10.0, "P99 latency {:.2}ms exceeds 10ms target", p99);
}
```

---

## 10. Deployment Checklist

### 10.1 Pre-Deployment

- [ ] **Code Complete**
  - [ ] All 5 strategies implemented
  - [ ] Core engine complete
  - [ ] Integration adapters ready
  - [ ] Error handling comprehensive

- [ ] **Testing Complete**
  - [ ] 120+ unit tests passing
  - [ ] Integration tests passing
  - [ ] Benchmarks meeting targets
  - [ ] Load tests successful
  - [ ] Security audit passed

- [ ] **Documentation Complete**
  - [ ] API documentation
  - [ ] Integration guides
  - [ ] Configuration reference
  - [ ] Troubleshooting guide
  - [ ] Runbook

- [ ] **Infrastructure Ready**
  - [ ] CI/CD pipeline configured
  - [ ] Monitoring dashboards created
  - [ ] Alerting rules configured
  - [ ] Log aggregation setup

### 10.2 Deployment Steps

1. **Stage 1: Canary Deployment (10% traffic)**
   - [ ] Deploy to canary environment
   - [ ] Monitor for 24 hours
   - [ ] Validate metrics:
     - Decision latency <10ms
     - Error rate <0.1%
     - Cost reduction >20%
   - [ ] Rollback if issues detected

2. **Stage 2: Progressive Rollout (50% traffic)**
   - [ ] Deploy to 50% of production
   - [ ] Monitor for 48 hours
   - [ ] A/B test against baseline
   - [ ] Collect user feedback

3. **Stage 3: Full Deployment (100% traffic)**
   - [ ] Deploy to all production
   - [ ] Monitor for 1 week
   - [ ] Validate business metrics
   - [ ] Document lessons learned

### 10.3 Post-Deployment

- [ ] **Monitoring**
  - [ ] Decision latency alerts configured
  - [ ] Error rate alerts configured
  - [ ] Cost tracking dashboard
  - [ ] Weekly performance reports

- [ ] **Optimization**
  - [ ] Tune strategy parameters
  - [ ] Optimize cache hit rates
  - [ ] Adjust rate limits
  - [ ] Refine model selection

- [ ] **Maintenance**
  - [ ] Weekly health checks
  - [ ] Monthly performance reviews
  - [ ] Quarterly strategy updates
  - [ ] Continuous improvement

### 10.4 Rollback Plan

```rust
// Emergency rollback mechanism
pub struct RollbackManager {
    previous_decisions: Arc<RwLock<LruCache<Uuid, OptimizationDecision>>>,
}

impl RollbackManager {
    pub async fn rollback_decision(&self, decision_id: Uuid) -> Result<()> {
        // Revert to previous safe decision
        if let Some(previous) = self.previous_decisions.read().await.get(&decision_id) {
            self.apply_decision(previous.clone()).await?;
            warn!("Rolled back decision {}", decision_id);
            Ok(())
        } else {
            Err(DecisionError::NoPreviousDecision)
        }
    }

    pub async fn rollback_to_baseline(&self) -> Result<()> {
        // Revert to safe baseline configuration
        let baseline = DecisionBaseline::load().await?;
        self.apply_baseline(baseline).await?;
        warn!("Rolled back to baseline configuration");
        Ok(())
    }
}
```

---

## 11. Success Metrics

### 11.1 Technical Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Decision Latency (p99) | <10ms | Prometheus |
| Throughput | >10K/sec | Load tests |
| Error Rate | <0.1% | Logs |
| Test Coverage | >92% | Tarpaulin |
| Memory Usage (idle) | <100MB | Profiling |

### 11.2 Business Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Cost Reduction | 30-60% | Finance |
| Quality Score | >4.5/5 | User surveys |
| Uptime | >99.9% | Monitoring |
| Time to Optimization | <5 min | Analytics |

### 11.3 Key Performance Indicators (KPIs)

1. **Cost Efficiency**
   - Average cost per request
   - Monthly burn rate
   - Budget utilization

2. **Performance**
   - Average response latency
   - Throughput (RPS)
   - Resource utilization

3. **Quality**
   - User satisfaction score
   - Task success rate
   - Error rate

4. **Adoption**
   - Active users
   - Decision volume
   - Strategy usage distribution

---

## 12. Timeline and Milestones

### 12.1 Implementation Schedule

```
Week 1: Core Framework
├── Day 1-2: Traits and Types
├── Day 3-4: Coordinator
└── Day 5: Policy & Validator

Week 2: Model Selection Strategy
├── Day 1-3: Core strategy
└── Day 4-5: Scorer, selector, fallback

Week 3: Caching & Rate Limiting
├── Day 1-2: Caching strategy
└── Day 3-5: Rate limiting strategy

Week 4: Batching & Prompt Optimization (Part 1)
├── Day 1-2: Batching strategy
└── Day 3-5: Prompt optimization (part 1)

Week 5: Prompt Optimization (Part 2) & Integration
├── Day 1-2: Prompt optimization (part 2)
└── Day 3-5: Integration adapters

Week 6: Testing & Documentation
├── Day 1-2: Integration tests
├── Day 3-4: Performance testing
└── Day 5: Documentation
```

### 12.2 Milestones

| Milestone | Date | Deliverables |
|-----------|------|--------------|
| **M1: Core Foundation** | End Week 1 | - Core traits<br>- Types system<br>- Coordinator skeleton<br>- 45 tests |
| **M2: First Strategy** | End Week 2 | - Model selection complete<br>- Integration with registry<br>- 25 tests |
| **M3: Caching & Rate Limiting** | End Week 3 | - Caching strategy<br>- Rate limiting<br>- 42 tests |
| **M4: Batching & Prompt Opt** | End Week 4 | - Batching strategy<br>- Prompt optimization<br>- 43 tests |
| **M5: Integration** | End Week 5 | - Analyzer adapter<br>- Actuator adapter<br>- E2E tests |
| **M6: Production Ready** | End Week 6 | - All tests passing<br>- Documentation complete<br>- Benchmarks passing |

---

## 13. Team and Resources

### 13.1 Team Structure

**Core Team (3-4 developers)**

1. **Tech Lead** (1 person)
   - Architecture decisions
   - Code reviews
   - Integration coordination
   - Risk management

2. **Senior Engineers** (2 people)
   - Strategy implementation
   - Core engine development
   - Performance optimization
   - Testing

3. **Junior Engineer** (1 person, optional)
   - Unit testing
   - Documentation
   - Bug fixes
   - Integration testing

### 13.2 Skills Required

- **Must Have:**
  - Rust expertise (async/await, traits, generics)
  - Distributed systems knowledge
  - LLM operations experience
  - Testing and benchmarking

- **Nice to Have:**
  - ML/optimization algorithms
  - Production ops experience
  - DevOps/infrastructure

### 13.3 Infrastructure

- **Development:**
  - CI/CD pipeline (GitHub Actions)
  - Test infrastructure
  - Benchmarking servers

- **Production:**
  - Kubernetes cluster
  - Monitoring (Prometheus + Grafana)
  - Logging (ELK/Loki)
  - Tracing (Jaeger)

---

## 14. Future Enhancements

### 14.1 Phase 2 Features (Post-MVP)

1. **Advanced Strategies**
   - Multi-provider routing
   - Dynamic prompt templates
   - Adaptive timeout management
   - Request prioritization

2. **ML Integration**
   - Reinforcement learning for strategy selection
   - Predictive cost modeling
   - Anomaly prediction
   - Auto-tuning parameters

3. **Enterprise Features**
   - Multi-tenancy support
   - Custom strategy plugins
   - Advanced analytics
   - Compliance reporting

### 14.2 Optimization Opportunities

1. **Performance**
   - Strategy result caching
   - Parallel strategy execution
   - Pre-computation of scores
   - Connection pooling

2. **Intelligence**
   - Contextual strategy selection
   - Historical pattern learning
   - Cross-request optimization
   - Workload prediction

---

## 15. Conclusion

This implementation plan provides a comprehensive roadmap for building a production-grade Decision Engine with 5 optimization strategies. The phased approach ensures:

✅ **Quality**: >92% test coverage, comprehensive error handling
✅ **Performance**: <10ms decision latency, 10K decisions/sec
✅ **Reliability**: Fail-safes, rollback mechanisms, audit logging
✅ **Maintainability**: Modular design, clear interfaces, documentation
✅ **Business Value**: 30-60% cost reduction, quality maintenance

**Total Investment:**
- **6 weeks** development time
- **3-4 developers**
- **~22,560 LOC** (production + tests + docs)
- **120+ tests**

**Expected ROI:**
- 30-60% LLM cost reduction
- <5 minute optimization cycles
- 99.9% uptime
- Enterprise-ready production deployment

The Decision Engine is the critical intelligence layer that bridges analysis and action, enabling autonomous, real-time LLM optimization at scale.

---

## Appendix A: Code Scaffolding Examples

### A.1 Creating a New Strategy

```rust
// Template for creating a new optimization strategy

use async_trait::async_trait;
use crate::engine::traits::OptimizationStrategy;
use crate::engine::types::*;
use crate::errors::Result;

/// Configuration for YourStrategy
#[derive(Debug, Clone)]
pub struct YourStrategyConfig {
    // Add configuration fields
    pub setting1: f64,
    pub setting2: bool,
}

impl Default for YourStrategyConfig {
    fn default() -> Self {
        Self {
            setting1: 1.0,
            setting2: true,
        }
    }
}

/// Your optimization strategy
pub struct YourStrategy {
    config: YourStrategyConfig,
    // Add internal state
}

impl YourStrategy {
    /// Create new strategy instance
    pub fn new(config: YourStrategyConfig) -> Result<Self> {
        Ok(Self {
            config,
        })
    }

    // Add helper methods
    async fn helper_method(&self, context: &DecisionContext) -> Result<Something> {
        // Implementation
        Ok(Something)
    }
}

#[async_trait]
impl OptimizationStrategy for YourStrategy {
    fn name(&self) -> &str {
        "your_strategy"
    }

    fn priority(&self) -> i32 {
        50 // Adjust priority
    }

    async fn is_applicable(&self, context: &DecisionContext) -> Result<bool> {
        // Determine if strategy should run
        Ok(true)
    }

    async fn execute(&self, context: &DecisionContext) -> Result<StrategyResult> {
        let start = std::time::Instant::now();

        // Execute strategy logic
        let result = self.helper_method(context).await?;

        let elapsed = start.elapsed().as_millis() as f64;

        Ok(StrategyResult {
            strategy_name: self.name().to_string(),
            success: true,
            contribution: StrategyContribution::YourContribution(result),
            execution_time_ms: elapsed,
            error: None,
        })
    }

    async fn estimate_impact(&self, context: &DecisionContext) -> Result<ImpactEstimate> {
        Ok(ImpactEstimate {
            cost_reduction: 0.0,
            latency_delta_ms: 0.0,
            quality_delta: 0.0,
            confidence: 0.5,
            risk: 0.5,
        })
    }

    fn validate_config(&self) -> Result<()> {
        // Validate configuration
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_strategy_basic() {
        let config = YourStrategyConfig::default();
        let strategy = YourStrategy::new(config).unwrap();

        let context = create_test_context();
        let result = strategy.execute(&context).await;

        assert!(result.is_ok());
    }

    fn create_test_context() -> DecisionContext {
        // Create test context
        DecisionContext::default()
    }
}
```

### A.2 Registering Strategies

```rust
// In your application initialization

use llm_optimizer_decision::{
    DecisionCoordinator, CoordinatorConfig,
    ModelSelectionStrategy, ModelSelectionConfig,
    CachingStrategy, CachingConfig,
    RateLimitingStrategy, RateLimitingConfig,
    BatchingStrategy, BatchingConfig,
    PromptOptimizationStrategy, PromptOptimizationConfig,
};

pub async fn initialize_decision_engine() -> Result<DecisionCoordinator> {
    let mut coordinator = DecisionCoordinator::new(
        CoordinatorConfig {
            max_parallel_strategies: 5,
            strategy_timeout_ms: 100,
            enable_caching: true,
            cache_ttl_seconds: 300,
            max_history_size: 1000,
            fail_fast: false,
        }
    ).await?;

    // Register Model Selection Strategy
    coordinator.register_strategy(
        Box::new(ModelSelectionStrategy::new(ModelSelectionConfig {
            quality_weight: 0.4,
            cost_weight: 0.4,
            latency_weight: 0.2,
            min_quality: 0.7,
            max_cost: 1.0,
            ..Default::default()
        })?)
    ).await?;

    // Register Caching Strategy
    coordinator.register_strategy(
        Box::new(CachingStrategy::new(CachingConfig {
            enable_semantic: true,
            similarity_threshold: 0.85,
            enable_exact: true,
            default_ttl: 3600,
            ..Default::default()
        })?)
    ).await?;

    // Register Rate Limiting Strategy
    coordinator.register_strategy(
        Box::new(RateLimitingStrategy::new(RateLimitingConfig::default())?)
    ).await?;

    // Register Batching Strategy
    coordinator.register_strategy(
        Box::new(BatchingStrategy::new(BatchingConfig {
            min_batch_size: 5,
            max_batch_size: 20,
            max_wait_ms: 100,
            ..Default::default()
        })?)
    ).await?;

    // Register Prompt Optimization Strategy
    coordinator.register_strategy(
        Box::new(PromptOptimizationStrategy::new(PromptOptimizationConfig {
            enable_compression: true,
            target_compression: 0.3,
            min_quality: 0.8,
            ..Default::default()
        })?)
    ).await?;

    Ok(coordinator)
}
```

---

## Appendix B: Error Handling

### B.1 Enhanced Error Types

```rust
// crates/decision/src/errors.rs

use thiserror::Error;

pub type Result<T> = std::result::Result<T, DecisionError>;

#[derive(Error, Debug)]
pub enum DecisionError {
    // Existing errors...
    #[error("Invalid experiment configuration: {0}")]
    InvalidConfig(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    // New decision engine errors
    #[error("Strategy execution failed: {0}")]
    StrategyFailed(String),

    #[error("Strategy timeout: {0}")]
    StrategyTimeout(String),

    #[error("No viable models available")]
    NoViableModels,

    #[error("Policy violation: {0:?}")]
    PolicyViolation(Vec<PolicyViolation>),

    #[error("Decision validation failed: {0}")]
    ValidationFailed(String),

    #[error("Circuit breaker open for: {0}")]
    CircuitOpen(String),

    #[error("Cost increase exceeds threshold")]
    CostIncreaseExceeded,

    #[error("Quality below threshold: {0}")]
    QualityBelowThreshold(f64),

    #[error("No previous decision available")]
    NoPreviousDecision,

    #[error("Integration error: {0}")]
    IntegrationError(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
```

---

**Document Version:** 1.0
**Last Updated:** 2025-11-10
**Status:** Ready for Implementation
**Approver:** Engineering Leadership
