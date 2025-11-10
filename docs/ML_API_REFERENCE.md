# ML Integration API Reference

Complete Rust API documentation for all ML components.

## Table of Contents

1. [Feature Store API](#feature-store-api)
2. [Online Learner Trait](#online-learner-trait)
3. [Thompson Sampling API](#thompson-sampling-api)
4. [LinUCB API](#linucb-api)
5. [Contextual Thompson API](#contextual-thompson-api)
6. [Parameter Optimizer API](#parameter-optimizer-api)
7. [Model Registry API](#model-registry-api)
8. [A/B Testing API](#ab-testing-api)
9. [Pareto Optimization API](#pareto-optimization-api)
10. [Drift Detection API](#drift-detection-api)
11. [Anomaly Detection API](#anomaly-detection-api)
12. [Reward Calculation API](#reward-calculation-api)
13. [Context API](#context-api)

---

## Feature Store API

### RequestContext

Context features for contextual bandits.

```rust
pub struct RequestContext {
    pub user_id: Option<String>,
    pub task_type: Option<String>,
    pub input_length: usize,
    pub output_length_category: OutputLengthCategory,
    pub priority: u8,
    pub region: Option<String>,
    pub hour_of_day: u8,
    pub day_of_week: u8,
    pub language: Option<String>,
    pub metadata: HashMap<String, String>,
}

impl RequestContext {
    /// Create new context with input length
    pub fn new(input_length: usize) -> Self;

    /// Convert to feature vector (10 dimensions)
    pub fn to_feature_vector(&self) -> Vec<f64>;

    /// Get feature dimension (always 10)
    pub fn feature_dimension() -> usize;

    /// Builder: set task type
    pub fn with_task_type(self, task_type: impl Into<String>) -> Self;

    /// Builder: set output length category
    pub fn with_output_length(self, category: OutputLengthCategory) -> Self;

    /// Builder: set priority (1-10)
    pub fn with_priority(self, priority: u8) -> Self;

    /// Builder: set user ID
    pub fn with_user_id(self, user_id: impl Into<String>) -> Self;

    /// Builder: set language
    pub fn with_language(self, language: impl Into<String>) -> Self;

    /// Builder: add custom metadata
    pub fn with_metadata(
        self,
        key: impl Into<String>,
        value: impl Into<String>
    ) -> Self;
}
```

### OutputLengthCategory

```rust
pub enum OutputLengthCategory {
    Short,   // < 100 tokens
    Medium,  // 100-500 tokens
    Long,    // > 500 tokens
}
```

**Example:**

```rust
use decision::{RequestContext, OutputLengthCategory};

let context = RequestContext::new(256)
    .with_task_type("code_generation")
    .with_output_length(OutputLengthCategory::Long)
    .with_priority(8)
    .with_user_id("user_123")
    .with_metadata("language", "python");

let features = context.to_feature_vector();
assert_eq!(features.len(), RequestContext::feature_dimension());
```

---

## Thompson Sampling API

### ThompsonSampling

Multi-armed bandit using Thompson Sampling.

```rust
pub struct ThompsonSampling {
    arms: HashMap<Uuid, BanditArm>,
    total_samples: u64,
}

impl ThompsonSampling {
    /// Create new Thompson Sampling instance
    pub fn new() -> Self;

    /// Add a variant (arm)
    pub fn add_variant(&mut self, variant_id: Uuid);

    /// Remove a variant
    pub fn remove_variant(&mut self, variant_id: &Uuid);

    /// Select variant using Thompson Sampling
    ///
    /// Samples from each arm's Beta distribution and returns
    /// the arm with the highest sampled value.
    ///
    /// # Returns
    /// - `Ok(Uuid)`: Selected variant ID
    /// - `Err(DecisionError)`: If no variants available
    pub fn select_variant(&self) -> Result<Uuid>;

    /// Update variant with observation
    ///
    /// # Arguments
    /// * `variant_id` - Which variant was used
    /// * `success` - Whether outcome was successful
    pub fn update(
        &mut self,
        variant_id: &Uuid,
        success: bool
    ) -> Result<()>;

    /// Get conversion rates for all variants
    pub fn get_conversion_rates(&self) -> HashMap<Uuid, f64>;

    /// Get arm statistics
    pub fn get_arm(&self, variant_id: &Uuid) -> Option<&BanditArm>;

    /// Get all arms
    pub fn get_arms(&self) -> &HashMap<Uuid, BanditArm>;

    /// Calculate cumulative regret
    ///
    /// Regret = expected reward if always chose best arm
    ///        - actual cumulative reward
    pub fn calculate_regret(&self) -> f64;

    /// Check if converged (low regret rate)
    pub fn has_converged(&self, threshold: f64) -> bool;

    /// Get total number of samples
    pub fn total_samples(&self) -> u64;
}
```

### BanditArm

```rust
pub struct BanditArm {
    pub variant_id: Uuid,
    pub successes: f64,     // Beta alpha parameter
    pub failures: f64,      // Beta beta parameter
    pub trials: u64,
}

impl BanditArm {
    /// Create new arm with Beta(1, 1) prior
    pub fn new(variant_id: Uuid) -> Self;

    /// Update with success/failure
    pub fn update(&mut self, success: bool);

    /// Get conversion rate (mean of Beta distribution)
    pub fn conversion_rate(&self) -> f64;

    /// Get Bayesian credible interval
    ///
    /// # Arguments
    /// * `confidence` - Confidence level (e.g., 0.95 for 95%)
    ///
    /// # Returns
    /// (lower_bound, upper_bound)
    pub fn credible_interval(&self, confidence: f64) -> (f64, f64);

    /// Sample from Beta distribution
    pub fn sample(&self) -> Result<f64>;
}
```

**Example:**

```rust
use decision::ThompsonSampling;
use uuid::Uuid;

let mut ts = ThompsonSampling::new();

let variant_a = Uuid::new_v4();
let variant_b = Uuid::new_v4();

ts.add_variant(variant_a);
ts.add_variant(variant_b);

// Select and update
let selected = ts.select_variant()?;
let success = call_llm(selected)?;
ts.update(&selected, success)?;

// Check convergence
if ts.has_converged(0.01) {
    let rates = ts.get_conversion_rates();
    let best = rates.iter()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .unwrap();
    println!("Converged. Best variant: {:?}", best.0);
}
```

---

## LinUCB API

### LinUCB

Linear Upper Confidence Bound contextual bandit.

```rust
pub struct LinUCB {
    arms: HashMap<Uuid, LinUCBArm>,
    alpha: f64,
    dimension: usize,
}

impl LinUCB {
    /// Create new LinUCB bandit
    ///
    /// # Arguments
    /// * `alpha` - Exploration parameter (typically 0.1-2.0)
    /// * `dimension` - Feature dimension (must match context)
    pub fn new(alpha: f64, dimension: usize) -> Self;

    /// Add an arm
    pub fn add_arm(&mut self, arm_id: Uuid);

    /// Remove an arm
    pub fn remove_arm(&mut self, arm_id: &Uuid);

    /// Select best arm for given context
    ///
    /// Calculates UCB score for each arm:
    /// UCB(a) = theta_a^T x + alpha * sqrt(x^T A_a^{-1} x)
    ///
    /// Returns arm with highest UCB score.
    pub fn select_arm(&self, context: &RequestContext) -> Result<Uuid>;

    /// Update arm with observed reward
    ///
    /// # Arguments
    /// * `arm_id` - Which arm was selected
    /// * `context` - Request context (features)
    /// * `reward` - Observed reward (real-valued)
    pub fn update(
        &mut self,
        arm_id: &Uuid,
        context: &RequestContext,
        reward: f64
    ) -> Result<()>;

    /// Get arm statistics
    pub fn get_arm(&self, arm_id: &Uuid) -> Option<&LinUCBArm>;

    /// Get all arms
    pub fn get_arms(&self) -> &HashMap<Uuid, LinUCBArm>;

    /// Get average rewards for all arms
    pub fn get_average_rewards(&self) -> HashMap<Uuid, f64>;

    /// Set exploration parameter
    pub fn set_alpha(&mut self, alpha: f64);

    /// Get exploration parameter
    pub fn alpha(&self) -> f64;
}
```

### LinUCBArm

```rust
pub struct LinUCBArm {
    pub arm_id: Uuid,
    pub a_matrix: Vec<Vec<f64>>,  // D^T D + I (d x d)
    pub b_vector: Vec<f64>,       // D^T c (d)
    pub d: usize,
    pub num_selections: u64,
    pub total_reward: f64,
}

impl LinUCBArm {
    /// Create new LinUCB arm
    ///
    /// Initializes A as identity matrix, b as zero vector.
    pub fn new(arm_id: Uuid, dimension: usize) -> Self;

    /// Calculate UCB score for context
    pub fn calculate_ucb(
        &self,
        context: &[f64],
        alpha: f64
    ) -> Result<f64>;

    /// Update arm with context and reward
    ///
    /// Updates: A = A + x x^T, b = b + r x
    pub fn update(&mut self, context: &[f64], reward: f64) -> Result<()>;

    /// Get average reward
    pub fn average_reward(&self) -> f64;
}
```

**Example:**

```rust
use decision::{LinUCB, RequestContext};
use uuid::Uuid;

let mut bandit = LinUCB::new(
    1.0,                              // alpha
    RequestContext::feature_dimension() // 10 features
);

let model_a = Uuid::new_v4();
let model_b = Uuid::new_v4();

bandit.add_arm(model_a);
bandit.add_arm(model_b);

// Context-aware selection
let context = RequestContext::new(200)
    .with_task_type("code_generation");

let selected = bandit.select_arm(&context)?;

// Update with reward
let reward = 0.85;
bandit.update(&selected, &context, reward)?;

// Get average rewards
let avg_rewards = bandit.get_average_rewards();
```

---

## Parameter Optimizer API

### ParameterOptimizer

High-level API for parameter optimization.

```rust
pub struct ParameterOptimizer {
    tuners: Arc<DashMap<String, AdaptiveParameterTuner>>,
    bandits: Arc<DashMap<String, LinUCB>>,
    policies: Arc<DashMap<String, OptimizationPolicy>>,
    reward_calculator: RewardCalculator,
}

impl ParameterOptimizer {
    /// Create new parameter optimizer
    pub fn new(reward_weights: RewardWeights, alpha: f64) -> Self;

    /// Create with default configuration
    pub fn with_defaults() -> Self;

    /// Create new optimization policy
    pub fn create_policy(&self, policy: OptimizationPolicy) -> Result<()>;

    /// Initialize policy with search strategy
    ///
    /// # Arguments
    /// * `policy_name` - Name of policy
    /// * `strategy` - Search strategy (Grid, Random, LHS)
    /// * `num_configs` - Number of configurations to generate
    ///
    /// # Returns
    /// Vector of configuration IDs
    pub fn initialize_with_search(
        &self,
        policy_name: &str,
        strategy: SearchStrategy,
        num_configs: usize,
    ) -> Result<Vec<Uuid>>;

    /// Select parameters for request
    ///
    /// # Arguments
    /// * `policy_name` - Policy to use
    /// * `context` - Request context
    ///
    /// # Returns
    /// (config_id, parameters)
    pub fn select_parameters(
        &self,
        policy_name: &str,
        context: &RequestContext,
    ) -> Result<(Uuid, ParameterConfig)>;

    /// Update with performance feedback
    pub fn update_performance(
        &self,
        policy_name: &str,
        config_id: &Uuid,
        context: &RequestContext,
        metrics: &ResponseMetrics,
        feedback: Option<&UserFeedback>,
    ) -> Result<()>;

    /// Get performance statistics
    pub fn get_performance_stats(
        &self,
        policy_name: &str
    ) -> Result<Vec<ParameterStats>>;

    /// Get best configuration for task type
    pub fn get_best_for_task(
        &self,
        policy_name: &str,
        task_type: &str,
    ) -> Result<Option<(Uuid, ParameterConfig)>>;

    /// Update task-specific best configurations
    pub fn update_task_bests(
        &self,
        policy_name: &str,
        task_types: &[String]
    ) -> Result<()>;

    /// Change optimization mode
    pub fn set_mode(
        &self,
        policy_name: &str,
        mode: OptimizationMode
    ) -> Result<()>;

    /// Get current optimization mode
    pub fn get_mode(&self, policy_name: &str) -> Result<OptimizationMode>;

    /// Update reward weights
    pub fn set_reward_weights(&mut self, weights: RewardWeights);

    /// Get number of policies
    pub fn num_policies(&self) -> usize;
}
```

### OptimizationPolicy

```rust
pub struct OptimizationPolicy {
    pub name: String,
    pub range: ParameterRange,
    pub mode: OptimizationMode,
    pub exploration_rate: f64,
}

impl OptimizationPolicy {
    /// Create new policy
    pub fn new(
        name: impl Into<String>,
        range: ParameterRange,
        mode: OptimizationMode
    ) -> Self;

    /// Set exploration rate (for Balanced mode)
    pub fn with_exploration_rate(self, rate: f64) -> Self;
}
```

### OptimizationMode

```rust
pub enum OptimizationMode {
    Explore,   // Exploration phase (search)
    Exploit,   // Exploitation phase (use best)
    Balanced,  // Mix of both
}
```

### ParameterConfig

```rust
pub struct ParameterConfig {
    pub temperature: f64,
    pub top_p: f64,
    pub max_tokens: usize,
}

impl ParameterConfig {
    /// Create new configuration
    pub fn new(
        temperature: f64,
        top_p: f64,
        max_tokens: usize
    ) -> Result<Self>;

    /// Validate configuration
    pub fn validate(&self) -> Result<()>;
}
```

### ParameterRange

```rust
pub struct ParameterRange {
    pub temp_min: f64,
    pub temp_max: f64,
    pub top_p_min: f64,
    pub top_p_max: f64,
    pub max_tokens_min: usize,
    pub max_tokens_max: usize,
}

impl ParameterRange {
    /// Default ranges for general use
    pub fn default() -> Self;

    /// Ranges optimized for specific task type
    pub fn for_task_type(task_type: &str) -> Self;
}
```

**Example:**

```rust
use decision::{
    ParameterOptimizer, OptimizationPolicy, OptimizationMode,
    ParameterRange, RewardWeights, RequestContext,
    ResponseMetrics, SearchStrategy,
};

let optimizer = ParameterOptimizer::with_defaults();

let policy = OptimizationPolicy::new(
    "quality_opt",
    ParameterRange::default(),
    OptimizationMode::Explore,
);

optimizer.create_policy(policy)?;

optimizer.initialize_with_search(
    "quality_opt",
    SearchStrategy::LatinHypercube,
    20,
)?;

// Select and use
let context = RequestContext::new(200);
let (config_id, params) = optimizer.select_parameters("quality_opt", &context)?;

// Call LLM with params
let metrics = ResponseMetrics {
    quality_score: 0.9,
    cost: 0.01,
    latency_ms: 1500.0,
    token_count: 500,
};

optimizer.update_performance("quality_opt", &config_id, &context, &metrics, None)?;
```

---

## Reward Calculation API

### RewardCalculator

```rust
pub struct RewardCalculator {
    weights: RewardWeights,
    min_cost: f64,
    max_latency: f64,
}

impl RewardCalculator {
    /// Create new reward calculator
    pub fn new(
        weights: RewardWeights,
        min_cost: f64,
        max_latency: f64
    ) -> Self;

    /// Calculate reward from metrics and feedback
    pub fn calculate_reward(
        &self,
        metrics: &ResponseMetrics,
        feedback: &UserFeedback
    ) -> f64;

    /// Calculate reward from metrics only
    pub fn calculate_reward_metrics_only(
        &self,
        metrics: &ResponseMetrics
    ) -> f64;

    /// Set weights
    pub fn set_weights(&mut self, weights: RewardWeights);

    /// Get weights
    pub fn weights(&self) -> &RewardWeights;
}
```

### RewardWeights

```rust
pub struct RewardWeights {
    pub quality: f64,
    pub cost: f64,
    pub latency: f64,
}

impl RewardWeights {
    /// Create custom weights (normalized to sum to 1)
    pub fn new(quality: f64, cost: f64, latency: f64) -> Self;

    /// Default: 60% quality, 30% cost, 10% latency
    pub fn default_weights() -> Self;

    /// Quality-focused: 80% quality, 15% cost, 5% latency
    pub fn quality_focused() -> Self;

    /// Cost-focused: 40% quality, 50% cost, 10% latency
    pub fn cost_focused() -> Self;

    /// Latency-focused: 40% quality, 30% cost, 30% latency
    pub fn latency_focused() -> Self;
}
```

### ResponseMetrics

```rust
pub struct ResponseMetrics {
    pub quality_score: f64,  // [0, 1]
    pub cost: f64,           // USD
    pub latency_ms: f64,
    pub token_count: usize,
}
```

### UserFeedback

```rust
pub struct UserFeedback {
    pub explicit_rating: Option<f64>,
    pub task_completed: bool,
    pub retry_occurred: bool,
    pub response_edited: bool,
    pub dwell_time_seconds: Option<f64>,
    pub response_saved: bool,
    pub response_shared: bool,
}

impl UserFeedback {
    /// Create new feedback with defaults
    pub fn new() -> Self;

    /// Infer quality score from implicit signals
    pub fn infer_quality(&self) -> f64;
}
```

---

## Drift Detection API

### ADWIN

```rust
pub struct ADWIN {
    delta: f64,
    window: VecDeque<f64>,
    max_window_size: usize,
}

impl ADWIN {
    /// Create new ADWIN detector
    ///
    /// # Arguments
    /// * `delta` - Confidence parameter (0, 1), typically 0.002
    /// * `max_window_size` - Maximum window size
    pub fn new(delta: f64, max_window_size: usize) -> Result<Self>;

    /// Add observation and check for drift
    pub fn add(&mut self, value: f64) -> DriftStatus;

    /// Reset detector
    pub fn reset(&mut self) -> Result<()>;

    /// Get window size
    pub fn window_size(&self) -> usize;

    /// Get window mean
    pub fn window_mean(&self) -> f64;

    /// Check if drift detected
    pub fn drift_detected(&self) -> bool;
}
```

### DriftStatus

```rust
pub enum DriftStatus {
    Stable,   // No drift
    Warning,  // Possible drift
    Drift,    // Drift detected
}
```

---

## Complete API Index

### Core Types

- `RequestContext` - Feature store for contextual bandits
- `OutputLengthCategory` - Output length classification
- `ParameterConfig` - LLM parameter configuration
- `ParameterRange` - Parameter search space
- `ResponseMetrics` - LLM response metrics
- `UserFeedback` - User feedback signals
- `RewardWeights` - Reward function weights

### Algorithms

- `ThompsonSampling` - Multi-armed bandit
- `LinUCB` - Contextual bandit (linear)
- `ContextualThompson` - Contextual bandit (non-linear)
- `ParameterOptimizer` - High-level parameter tuning
- `GridSearch` - Systematic parameter search
- `RandomSearch` - Random parameter sampling
- `LatinHypercubeSampling` - Space-filling sampling

### Utilities

- `RewardCalculator` - Reward computation
- `ADWIN` - Drift detection
- `ZScoreDetector` - Anomaly detection
- `ParetoFrontier` - Multi-objective optimization
- `ModelRegistry` - Model catalog

### Error Types

```rust
pub enum DecisionError {
    InvalidConfig(String),
    InvalidParameter(String),
    InvalidState(String),
    VariantNotFound(String),
    ExperimentNotFound(String),
    AllocationError(String),
    StatisticalError(String),
}

pub type Result<T> = std::result::Result<T, DecisionError>;
```

For complete examples and usage patterns, see:
- [User Guide](ML_USER_GUIDE.md)
- [Tutorial](ML_TUTORIAL.md)
- [Examples](ML_EXAMPLES.md)
