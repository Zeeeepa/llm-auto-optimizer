# ML Algorithm Reference

Comprehensive reference for all machine learning algorithms in the LLM Auto-Optimizer.

## Table of Contents

1. [Thompson Sampling](#thompson-sampling)
2. [LinUCB (Linear Upper Confidence Bound)](#linucb)
3. [Contextual Thompson Sampling](#contextual-thompson-sampling)
4. [Grid Search](#grid-search)
5. [Random Search](#random-search)
6. [Latin Hypercube Sampling](#latin-hypercube-sampling)
7. [Pareto Optimization](#pareto-optimization)
8. [ADWIN (Drift Detection)](#adwin)
9. [Page-Hinkley (Drift Detection)](#page-hinkley)
10. [CUSUM (Drift Detection)](#cusum)
11. [Z-Score Anomaly Detection](#z-score-anomaly-detection)
12. [IQR Anomaly Detection](#iqr-anomaly-detection)

---

## Thompson Sampling

### What It Does

Thompson Sampling is a Bayesian algorithm for the multi-armed bandit problem. It samples from the posterior distribution of each arm's reward and selects the arm with the highest sample.

### When to Use It

- **Simple model selection**: Choosing between 2-10 discrete options
- **No context**: Same choice is optimal for all users
- **Need fast convergence**: Requires minimal samples
- **Want confidence intervals**: Bayesian approach provides credible intervals

### How It Works (Theory)

Thompson Sampling uses Bayesian inference to maintain a belief about each arm's reward distribution:

1. **Prior**: Start with Beta(1,1) uniform prior for each arm
2. **Sampling**: For each request, sample θᵢ ~ Beta(αᵢ, βᵢ) for each arm i
3. **Selection**: Choose arm with highest sampled θᵢ
4. **Update**: After observing reward r ∈ {0,1}:
   - If r=1: αᵢ ← αᵢ + 1 (success)
   - If r=0: βᵢ ← βᵢ + 1 (failure)

**Intuition**: Arms with uncertain rewards (wide posterior) are sampled more often, providing natural exploration. As arms are tried, posteriors narrow and the best arm is exploited.

**Theoretical Guarantees**:
- Regret: O(√(N log N)) where N is number of trials
- Optimal in practice for finite-time horizon
- Matches lower bounds asymptotically

### Configuration Parameters

```rust
pub struct ThompsonSampling {
    // No hyperparameters! Self-tuning.
}

pub struct BanditArm {
    pub successes: f64,  // Alpha parameter (prior: 1.0)
    pub failures: f64,   // Beta parameter (prior: 1.0)
    pub trials: u64,     // Total observations
}
```

**successes/failures**: Beta distribution parameters. Start with 1.0 for uniform prior.

**Tuning**: No tuning needed! Thompson Sampling is "parameter-free" in practice.

### Performance Characteristics

| Metric | Value |
|--------|-------|
| Time Complexity | O(K) per selection, K = number of arms |
| Space Complexity | O(K) |
| Convergence Rate | Fast (50-200 samples typically) |
| Regret Bound | O(√(NK log K)) optimal |
| Best Case | 2-10 arms, clear winner |
| Worst Case | Many arms (>50), all similar |

### Example Use Cases

**1. Model Selection**
```rust
use decision::ThompsonSampling;

let mut ts = ThompsonSampling::new();
ts.add_variant(gpt5_id);
ts.add_variant(claude4_id);
ts.add_variant(gemini25_id);

// Select and update
let variant = ts.select_variant()?;
let success = call_llm(variant) > quality_threshold;
ts.update(&variant, success)?;
```

**2. Prompt Template Selection**
```rust
// Try different prompt formats
let prompts = vec![
    "You are a helpful assistant...",
    "As an expert...",
    "Task: ...",
];

for prompt_id in prompts {
    ts.add_variant(prompt_id);
}
```

**3. Cache Strategy Selection**
```rust
// Which caching approach works best?
ts.add_variant(semantic_cache_id);
ts.add_variant(exact_match_cache_id);
ts.add_variant(no_cache_id);
```

### Rust API Documentation

```rust
impl ThompsonSampling {
    /// Create new Thompson Sampling instance
    pub fn new() -> Self;

    /// Add a variant (arm) to the bandit
    pub fn add_variant(&mut self, variant_id: Uuid);

    /// Remove a variant
    pub fn remove_variant(&mut self, variant_id: &Uuid);

    /// Select a variant using Thompson Sampling
    ///
    /// Samples from each arm's Beta distribution and returns
    /// the variant with highest sampled value.
    pub fn select_variant(&self) -> Result<Uuid>;

    /// Update with observation
    ///
    /// # Arguments
    /// * `variant_id` - Which variant was used
    /// * `success` - Whether outcome was successful (true/false)
    pub fn update(&mut self, variant_id: &Uuid, success: bool) -> Result<()>;

    /// Get current conversion rates (means)
    pub fn get_conversion_rates(&self) -> HashMap<Uuid, f64>;

    /// Get arm statistics
    pub fn get_arm(&self, variant_id: &Uuid) -> Option<&BanditArm>;

    /// Calculate cumulative regret
    pub fn calculate_regret(&self) -> f64;

    /// Check if converged (low regret)
    pub fn has_converged(&self, threshold: f64) -> bool;
}

impl BanditArm {
    /// Create new arm with uniform prior Beta(1, 1)
    pub fn new(variant_id: Uuid) -> Self;

    /// Update with observation
    pub fn update(&mut self, success: bool);

    /// Get conversion rate (mean of Beta)
    pub fn conversion_rate(&self) -> f64;

    /// Get Bayesian credible interval
    pub fn credible_interval(&self, confidence: f64) -> (f64, f64);

    /// Sample from Beta distribution
    pub fn sample(&self) -> Result<f64>;
}
```

---

## LinUCB

### What It Does

LinUCB (Linear Upper Confidence Bound) is a contextual bandit algorithm that uses linear regression to predict rewards and adds an uncertainty bonus for exploration.

### When to Use It

- **Context matters**: Different users/tasks need different choices
- **Linear relationships**: Reward is approximately linear in features
- **Need guarantees**: Want provable regret bounds
- **Feature-based**: Have good feature representation

### How It Works (Theory)

LinUCB maintains a linear model θ for each arm to predict rewards:

1. **Model**: Reward rₜ ≈ θₐᵀ xₜ where xₜ is context, θₐ are arm parameters
2. **Estimation**: Use ridge regression: θₐ = (DₐᵀDₐ + I)⁻¹ Dₐᵀcₐ
   - Dₐ = matrix of observed contexts for arm a
   - cₐ = vector of observed rewards
3. **UCB Score**: UCBₐ(xₜ) = θₐᵀxₜ + α√(xₜᵀAₐ⁻¹xₜ)
   - First term: predicted reward
   - Second term: uncertainty bonus
4. **Selection**: Choose arm with highest UCB score

**Intuition**: Uncertainty bonus encourages exploring arms with high prediction uncertainty. As arm is tried, uncertainty decreases and predicted reward dominates.

**Theoretical Guarantees**:
- Regret: O(√(dT log T)) where d is feature dimension, T is trials
- Optimal up to logarithmic factors
- Works with arbitrary context distributions

### Configuration Parameters

```rust
pub struct LinUCB {
    alpha: f64,        // Exploration parameter (typically 0.1-2.0)
    dimension: usize,  // Feature dimension
}

pub struct LinUCBArm {
    pub a_matrix: Vec<Vec<f64>>,  // Dᵀ D + I (d × d matrix)
    pub b_vector: Vec<f64>,       // Dᵀ c (d vector)
    pub d: usize,                 // Feature dimension
}
```

**alpha**: Exploration parameter controlling uncertainty bonus
- Higher α → more exploration
- Lower α → more exploitation
- Typical range: 0.1 to 2.0
- Start with 1.0, tune based on performance

**dimension**: Number of features in context vector
- Should match RequestContext::feature_dimension()
- Typically 5-20 features
- Higher dimension → slower (matrix inversion)

### Performance Characteristics

| Metric | Value |
|--------|-------|
| Time Complexity | O(Kd³) per selection (K arms, d features) |
| Space Complexity | O(Kd²) |
| Convergence Rate | Medium (100-500 samples) |
| Regret Bound | O(√(dT log T)) optimal |
| Best Case | Clear linear relationship, d<20 |
| Worst Case | Non-linear, high dimension (d>50) |

### Example Use Cases

**1. Personalized Model Routing**
```rust
use decision::{LinUCB, RequestContext};

let mut bandit = LinUCB::new(1.0, RequestContext::feature_dimension());

// Add models
bandit.add_arm(gpt5_id);
bandit.add_arm(claude4_id);

// Context-aware selection
let context = RequestContext::new(256)
    .with_task_type("code_generation")
    .with_priority(8);

let model_id = bandit.select_arm(&context)?;

// Update with reward
let reward = calculate_reward(&response);
bandit.update(&model_id, &context, reward)?;
```

**2. Task-Specific Optimization**
```rust
// Different tasks may prefer different models
let context_qa = RequestContext::new(100)
    .with_task_type("question_answering");

let context_code = RequestContext::new(500)
    .with_task_type("code_generation");

// LinUCB learns task-specific preferences
let qa_model = bandit.select_arm(&context_qa)?;      // Might pick GPT-5
let code_model = bandit.select_arm(&context_code)?;  // Might pick Claude-4
```

**3. Time-Based Routing**
```rust
// Route to cheaper models during peak hours
let context_peak = RequestContext::new(200);  // hour_of_day = 14

let context_offpeak = RequestContext::new(200);  // hour_of_day = 3

// LinUCB learns time-based cost optimization
```

### Rust API Documentation

```rust
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

    /// Select best arm for context
    ///
    /// Calculates UCB score for each arm and returns highest.
    pub fn select_arm(&self, context: &RequestContext) -> Result<Uuid>;

    /// Update arm with observed reward
    ///
    /// # Arguments
    /// * `arm_id` - Which arm was selected
    /// * `context` - Request context (features)
    /// * `reward` - Observed reward (real-valued)
    pub fn update(&mut self, arm_id: &Uuid, context: &RequestContext, reward: f64) -> Result<()>;

    /// Get arm statistics
    pub fn get_arm(&self, arm_id: &Uuid) -> Option<&LinUCBArm>;

    /// Get all arms
    pub fn get_arms(&self) -> &HashMap<Uuid, LinUCBArm>;

    /// Get average rewards
    pub fn get_average_rewards(&self) -> HashMap<Uuid, f64>;

    /// Set exploration parameter
    pub fn set_alpha(&mut self, alpha: f64);

    /// Get exploration parameter
    pub fn alpha(&self) -> f64;
}

impl LinUCBArm {
    /// Create new arm with identity matrix initialization
    pub fn new(arm_id: Uuid, dimension: usize) -> Self;

    /// Calculate UCB score for context
    pub fn calculate_ucb(&self, context: &[f64], alpha: f64) -> Result<f64>;

    /// Update with context and reward (ridge regression update)
    pub fn update(&mut self, context: &[f64], reward: f64) -> Result<()>;

    /// Get average reward
    pub fn average_reward(&self) -> f64;
}
```

---

## Contextual Thompson Sampling

### What It Does

Extends Thompson Sampling to contextual settings using Bayesian linear regression with Gaussian priors.

### When to Use It

- **Non-linear rewards**: Relationships aren't purely linear
- **Probabilistic exploration**: Want stochastic selection
- **Uncertainty quantification**: Need posterior distributions
- **Complex contexts**: Rich feature representations

### How It Works (Theory)

Maintains Gaussian posterior over weight vectors:

1. **Model**: Reward rₜ ≈ θₐᵀ xₜ + ε where ε ~ N(0, σ²)
2. **Prior**: θₐ ~ N(μₐ, Σₐ) initialized to N(0, I)
3. **Sampling**: For each arm, sample θ̃ₐ ~ N(μₐ, Σₐ)
4. **Selection**: Choose arm a* = argmax θ̃ₐᵀ xₜ
5. **Update**: Bayesian update using gradient descent:
   - μₐ ← μₐ + η(rₜ - θₐᵀxₜ)xₜ
   - Σₐ ← 0.99 Σₐ (variance decay)

**Intuition**: Sample plausible reward functions from posterior, select arm that looks best under sampled function. More uncertain arms have wider posteriors, so occasionally sample high values.

### Configuration Parameters

```rust
pub struct ContextualThompsonArm {
    pub theta_mean: Vec<f64>,      // Mean of weight posterior
    pub theta_variance: Vec<f64>,  // Variance (diagonal only)
    pub learning_rate: f64,        // Gradient step size (default: 0.1)
}
```

**learning_rate**: How quickly to update weights
- Higher → faster learning, less stable
- Lower → slower learning, more stable
- Typical: 0.05 to 0.2
- Default: 0.1

### Performance Characteristics

| Metric | Value |
|--------|-------|
| Time Complexity | O(Kd) per selection |
| Space Complexity | O(Kd) |
| Convergence Rate | Slow (500-2000 samples) |
| Regret Bound | No formal guarantee |
| Best Case | Non-linear, stochastic exploration |
| Worst Case | Requires more data than LinUCB |

### Example Use Cases

**1. Complex Personalization**
```rust
use decision::ContextualThompson;

let mut bandit = ContextualThompson::new(RequestContext::feature_dimension());

bandit.add_arm(model1_id);
bandit.add_arm(model2_id);

// Thompson sampling naturally explores
let model = bandit.select_arm(&context)?;
bandit.update(&model, &context, reward)?;
```

**2. Multi-Modal Optimization**
```rust
// When reward landscape has multiple local optima
// Stochastic exploration helps escape local optima
```

### Rust API Documentation

```rust
impl ContextualThompson {
    /// Create new contextual Thompson sampling bandit
    pub fn new(dimension: usize) -> Self;

    /// Add an arm
    pub fn add_arm(&mut self, arm_id: Uuid);

    /// Remove an arm
    pub fn remove_arm(&mut self, arm_id: &Uuid);

    /// Select arm using Thompson sampling
    ///
    /// Samples weight vector from posterior for each arm,
    /// computes predicted reward, returns arm with highest.
    pub fn select_arm(&self, context: &RequestContext) -> Result<Uuid>;

    /// Update arm with observed reward
    pub fn update(&mut self, arm_id: &Uuid, context: &RequestContext, reward: f64) -> Result<()>;

    /// Get arm statistics
    pub fn get_arm(&self, arm_id: &Uuid) -> Option<&ContextualThompsonArm>;

    /// Get average rewards
    pub fn get_average_rewards(&self) -> HashMap<Uuid, f64>;
}

impl ContextualThompsonArm {
    /// Create new arm with N(0, I) prior
    pub fn new(arm_id: Uuid, dimension: usize) -> Self;

    /// Sample weight vector from posterior
    pub fn sample_theta(&self) -> Result<Vec<f64>>;

    /// Calculate expected reward
    pub fn expected_reward(&self, context: &[f64]) -> f64;

    /// Bayesian update with gradient descent
    pub fn update(&mut self, context: &[f64], reward: f64) -> Result<()>;

    /// Get average reward
    pub fn average_reward(&self) -> f64;
}
```

---

## Grid Search

### What It Does

Systematically explores parameter space by evaluating all points on a grid.

### When to Use It

- **Exhaustive search**: Want to try all combinations
- **Small parameter space**: 2-4 parameters, 3-5 values each
- **Reproducible**: Need deterministic exploration
- **Initial exploration**: Bootstrap before online learning

### How It Works

Creates Cartesian product of parameter ranges:

```
temperature: [0.3, 0.5, 0.7, 0.9, 1.1]
top_p: [0.7, 0.8, 0.9, 0.95]
max_tokens: [256, 512, 1024, 2048]

Total configurations: 5 × 4 × 4 = 80
```

### Configuration Parameters

```rust
pub struct GridSearchConfig {
    pub temp_steps: usize,       // Number of temperature values
    pub top_p_steps: usize,      // Number of top-p values
    pub max_tokens_steps: usize, // Number of max_tokens values
}
```

### Performance Characteristics

| Metric | Value |
|--------|-------|
| Configurations | Product of all step counts |
| Best Case | Small space (< 100 configs) |
| Worst Case | Exponential growth in parameters |

### Example Use Cases

```rust
use decision::{GridSearch, GridSearchConfig, ParameterRange};

let range = ParameterRange::default();
let config = GridSearchConfig {
    temp_steps: 5,
    top_p_steps: 4,
    max_tokens_steps: 3,
};

let mut search = GridSearch::new(range, config);

while let Some(params) = search.next() {
    let result = evaluate(params);
    record_result(params, result);
}
```

### Rust API Documentation

```rust
impl GridSearch {
    /// Create new grid search
    pub fn new(range: ParameterRange, config: GridSearchConfig) -> Self;

    /// Create with default config
    pub fn with_defaults(range: ParameterRange) -> Self;

    /// Get next configuration
    pub fn next(&mut self) -> Option<ParameterConfig>;

    /// Get all configurations
    pub fn all_configs(&self) -> Vec<ParameterConfig>;

    /// Get total number of configurations
    pub fn total_configs(&self) -> usize;

    /// Check if complete
    pub fn is_complete(&self) -> bool;
}
```

---

## Random Search

### What It Does

Randomly samples parameter configurations from the parameter space.

### When to Use It

- **Large parameter space**: Too many combinations for grid
- **Continuous parameters**: Not discrete values
- **Budget constrained**: Limited evaluations
- **Exploration**: Avoid grid bias

### How It Works

Uniformly samples from parameter ranges:

```rust
temperature ~ Uniform(temp_min, temp_max)
top_p ~ Uniform(top_p_min, top_p_max)
max_tokens ~ DiscreteUniform(min, max)
```

### Performance Characteristics

| Metric | Value |
|--------|-------|
| Configurations | User-specified |
| Coverage | Probabilistic |
| Best Case | Large continuous space |
| Worst Case | Small discrete space (use grid) |

### Example Use Cases

```rust
use decision::RandomSearch;

let mut search = RandomSearch::new(range, 100);  // 100 random samples

while let Some(params) = search.next() {
    evaluate(params);
}
```

### Rust API Documentation

```rust
impl RandomSearch {
    /// Create new random search
    ///
    /// # Arguments
    /// * `range` - Parameter ranges to sample from
    /// * `num_samples` - Number of random samples
    pub fn new(range: ParameterRange, num_samples: usize) -> Self;

    /// Generate next random configuration
    pub fn next(&mut self) -> Option<ParameterConfig>;

    /// Check if complete
    pub fn is_complete(&self) -> bool;

    /// Get number of remaining samples
    pub fn remaining(&self) -> usize;
}
```

---

## Latin Hypercube Sampling

### What It Does

Stratified sampling that ensures good coverage of parameter space with fewer samples than grid search.

### When to Use It

- **Efficient exploration**: Better than random with same budget
- **Space-filling**: Want even coverage
- **Medium space**: 10-100 configurations
- **Before optimization**: Initialize for Bayesian optimization

### How It Works

LHS divides each parameter range into N equal intervals and samples once from each:

1. Divide each dimension into N strata
2. Randomly sample once per stratum
3. Randomly permute samples across dimensions
4. Result: N samples with guaranteed coverage

### Performance Characteristics

| Metric | Value |
|--------|-------|
| Configurations | User-specified (N) |
| Coverage | Guaranteed stratification |
| Best Case | Medium space, expensive evaluations |

### Example Use Cases

```rust
use decision::LatinHypercubeSampling;

let search = LatinHypercubeSampling::new(range, 50);  // 50 well-distributed samples
let configs = search.all_configs();

for config in configs {
    evaluate(config);
}
```

### Rust API Documentation

```rust
impl LatinHypercubeSampling {
    /// Create new LHS
    ///
    /// # Arguments
    /// * `range` - Parameter ranges
    /// * `num_samples` - Number of samples (N)
    pub fn new(range: ParameterRange, num_samples: usize) -> Self;

    /// Get all configurations
    pub fn all_configs(&self) -> Vec<ParameterConfig>;

    /// Get sample count
    pub fn num_samples(&self) -> usize;
}
```

---

## Pareto Optimization

### What It Does

Finds the set of non-dominated solutions for multi-objective optimization (quality, cost, latency).

### When to Use It

- **Multiple objectives**: Optimizing quality AND cost AND latency
- **Trade-off analysis**: Understand quality-cost frontier
- **Objective flexibility**: Can change weights later
- **No single best**: Want all Pareto-optimal choices

### How It Works

A solution A dominates B if:
- A is better than B in at least one objective
- A is at least as good as B in all other objectives

Pareto frontier = set of non-dominated solutions

**Algorithm**:
```
1. Start with all candidates
2. For each pair (A, B):
   - If A dominates B, remove B
   - If B dominates A, remove A
3. Remaining candidates = Pareto frontier
```

### Configuration Parameters

```rust
pub struct ObjectiveWeights {
    pub quality: f64,     // Weight for quality [0, 1]
    pub cost: f64,        // Weight for cost [0, 1]
    pub latency: f64,     // Weight for latency [0, 1]
    pub max_cost: f64,    // Max acceptable cost (normalization)
    pub max_latency: f64, // Max acceptable latency (normalization)
}
```

Weights should sum to 1.0.

### Performance Characteristics

| Metric | Value |
|--------|-------|
| Time Complexity | O(N² M) where N = candidates, M = objectives |
| Space Complexity | O(N) |
| Pareto Set Size | Varies (1 to N) |

### Example Use Cases

**1. Model Selection with Trade-offs**
```rust
use decision::{ModelRegistry, ObjectiveWeights, ParetoFrontier};

let registry = ModelRegistry::new();

// Find Pareto-optimal models
let candidates = vec!["gpt-5", "claude-4", "gemini-2.5"];
let pareto_set = registry.find_pareto_optimal(&candidates, 1000, 500)?;

println!("Found {} Pareto-optimal models", pareto_set.len());

// Select based on current priorities
let weights = ObjectiveWeights::quality_focused(1.0, 5000.0);
let best = ParetoFrontier::select_optimal(&pareto_set, &weights)?;
```

**2. Dynamic Objective Weighting**
```rust
// Change weights based on time of day
let weights = if is_peak_hours() {
    ObjectiveWeights::cost_focused(1.0, 5000.0)  // Save money during peak
} else {
    ObjectiveWeights::quality_focused(1.0, 5000.0)  // Maximize quality off-peak
};

let best = ParetoFrontier::select_optimal(&pareto_set, &weights)?;
```

### Rust API Documentation

```rust
impl ParetoFrontier {
    /// Find Pareto-optimal set
    ///
    /// Returns all non-dominated candidates.
    pub fn find_pareto_optimal(candidates: &[ModelCandidate]) -> Result<Vec<ModelCandidate>>;

    /// Select best from Pareto set using weights
    ///
    /// Computes composite score for each candidate:
    /// score = w_q * quality + w_c * (1 - cost/max_cost) + w_l * (1 - latency/max_latency)
    pub fn select_optimal(
        pareto_set: &[ModelCandidate],
        weights: &ObjectiveWeights
    ) -> Result<&ModelCandidate>;

    /// Check if candidate A dominates candidate B
    pub fn dominates(a: &ModelCandidate, b: &ModelCandidate) -> bool;

    /// Get Pareto set size
    pub fn size(pareto_set: &[ModelCandidate]) -> usize;
}

impl ObjectiveWeights {
    /// Create new weights (normalized to sum to 1)
    pub fn new(quality: f64, cost: f64, latency: f64, max_cost: f64, max_latency: f64) -> Self;

    /// Quality-focused: 80% quality, 15% cost, 5% latency
    pub fn quality_focused(max_cost: f64, max_latency: f64) -> Self;

    /// Balanced: 50% quality, 30% cost, 20% latency
    pub fn balanced(max_cost: f64, max_latency: f64) -> Self;

    /// Cost-focused: 40% quality, 50% cost, 10% latency
    pub fn cost_focused(max_cost: f64, max_latency: f64) -> Self;

    /// Latency-focused: 40% quality, 30% cost, 30% latency
    pub fn latency_focused(max_cost: f64, max_latency: f64) -> Self;
}
```

---

## ADWIN

### What It Does

Adaptive Windowing (ADWIN) detects concept drift by monitoring distribution changes in sliding windows.

### When to Use It

- **Drift detection**: Detect when data distribution changes
- **Automatic adaptation**: No manual retraining schedule
- **Unknown drift**: Don't know when drift occurs
- **Real-time**: Need immediate detection

### How It Works

ADWIN maintains an adaptive sliding window:

1. **Window**: Keep recent observations
2. **Split**: Try all possible splits of window into two sub-windows
3. **Test**: For each split, test if means are significantly different
4. **Detect**: If means differ, drift detected → drop old data
5. **Adapt**: Window size adapts to drift rate

**Statistical Test**: Uses Hoeffding bound for distribution testing.

### Configuration Parameters

```rust
pub struct ADWIN {
    delta: f64,             // Confidence parameter (0, 1)
    max_window_size: usize, // Maximum window size
}
```

**delta**: Confidence level (typically 0.002)
- Lower δ → more sensitive (detects smaller changes)
- Higher δ → less sensitive (fewer false alarms)
- Typical: 0.001 to 0.01

**max_window_size**: Memory limit
- Larger → detect slower drifts
- Smaller → faster computation
- Typical: 500 to 5000

### Performance Characteristics

| Metric | Value |
|--------|-------|
| Time Complexity | O(W) per observation (W = window size) |
| Space Complexity | O(W) |
| Sensitivity | Depends on δ |
| False Positive Rate | ≤ δ |

### Example Use Cases

```rust
use decision::{ADWIN, DriftStatus};

let mut detector = ADWIN::new(0.002, 1000)?;

loop {
    let metric = get_current_metric();  // e.g., average reward
    let status = detector.add(metric);

    match status {
        DriftStatus::Drift => {
            println!("Drift detected! Retraining model...");
            retrain_model();
            detector.reset()?;
        }
        DriftStatus::Warning => {
            println!("Possible drift warning");
        }
        DriftStatus::Stable => {
            // Continue
        }
    }
}
```

### Rust API Documentation

```rust
impl ADWIN {
    /// Create new ADWIN detector
    ///
    /// # Arguments
    /// * `delta` - Confidence parameter (0, 1), typically 0.002
    /// * `max_window_size` - Maximum window size
    pub fn new(delta: f64, max_window_size: usize) -> Result<Self>;

    /// Add observation and check for drift
    ///
    /// Returns drift status.
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

---

## Z-Score Anomaly Detection

### What It Does

Detects anomalies using Z-score (standard deviations from mean).

### When to Use It

- **Outlier detection**: Identify unusual values
- **Normally distributed**: Data approximately Gaussian
- **Simple**: Easy to understand and explain
- **Real-time**: Low overhead detection

### How It Works

1. **Maintain statistics**: Mean μ and std deviation σ over sliding window
2. **Calculate Z-score**: z = (x - μ) / σ
3. **Threshold**: If |z| > threshold, flag as anomaly
4. **Typical threshold**: 3.0 (3σ rule covers 99.7% of normal data)

### Configuration Parameters

```rust
pub struct ZScoreDetector {
    window_size: usize,  // Sliding window size
    threshold: f64,      // Z-score threshold
}
```

**window_size**: Recent history to consider
- Larger → more stable, slower adaptation
- Smaller → faster adaptation, more sensitive
- Typical: 50 to 500

**threshold**: How many σ for anomaly
- 2.0 → ~5% false positive rate
- 3.0 → ~0.3% false positive rate
- 4.0 → ~0.01% false positive rate

### Performance Characteristics

| Metric | Value |
|--------|-------|
| Time Complexity | O(1) per observation |
| Space Complexity | O(W) where W = window size |
| False Positive Rate | Depends on threshold |

### Example Use Cases

```rust
use decision::{ZScoreDetector, AnomalyResult};

let mut detector = ZScoreDetector::new(100, 3.0)?;

loop {
    let cost = get_current_cost();
    let result = detector.add(cost);

    if result.is_anomaly {
        println!("Anomaly! Cost: {:.4}, Score: {:.2}, Severity: {:.2}",
            cost, result.score, result.severity);
        investigate();
    }
}
```

### Rust API Documentation

```rust
impl ZScoreDetector {
    /// Create new Z-score detector
    ///
    /// # Arguments
    /// * `window_size` - Sliding window size (min 2)
    /// * `threshold` - Z-score threshold (typically 2.0-4.0)
    pub fn new(window_size: usize, threshold: f64) -> Result<Self>;

    /// Add observation and check for anomaly
    pub fn add(&mut self, value: f64) -> AnomalyResult;

    /// Get current mean
    pub fn mean(&self) -> f64;

    /// Get current std deviation
    pub fn std_dev(&self) -> f64;

    /// Reset detector
    pub fn reset(&mut self);
}
```

---

## Summary Table

| Algorithm | Use Case | Complexity | Convergence | Regret Bound |
|-----------|----------|-----------|-------------|--------------|
| Thompson Sampling | Simple MAB | O(K) | Fast (50-200) | O(√(NK log K)) |
| LinUCB | Contextual, linear | O(Kd³) | Medium (100-500) | O(√(dT log T)) |
| Contextual Thompson | Contextual, non-linear | O(Kd) | Slow (500-2000) | None |
| Grid Search | Small discrete | O(∏ steps) | N/A | N/A |
| Random Search | Large continuous | O(N) | N/A | N/A |
| LHS | Space-filling | O(Nd) | N/A | N/A |
| Pareto | Multi-objective | O(N²M) | N/A | N/A |
| ADWIN | Drift detection | O(W) | N/A | N/A |
| Z-Score | Anomaly detection | O(1) | N/A | N/A |

Where:
- K = number of arms
- d = feature dimension
- N = number of samples
- W = window size
- M = number of objectives
- T = time horizon
