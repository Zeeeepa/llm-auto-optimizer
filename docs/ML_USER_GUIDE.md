# ML Integration User Guide

A comprehensive guide to using machine learning for LLM optimization in the LLM Auto-Optimizer.

## Table of Contents

1. [Introduction to Online Learning](#introduction-to-online-learning)
2. [When to Use ML vs A/B Testing](#when-to-use-ml-vs-ab-testing)
3. [Algorithm Selection Guide](#algorithm-selection-guide)
4. [Feature Engineering Guide](#feature-engineering-guide)
5. [Model Selection Strategy](#model-selection-strategy)
6. [A/B Testing Guide](#ab-testing-guide)
7. [Monitoring and Debugging](#monitoring-and-debugging)
8. [Best Practices](#best-practices)

## Introduction to Online Learning

Online learning enables your LLM system to continuously improve by learning from each request and feedback. Unlike traditional batch learning, online learning:

- **Learns incrementally** from each observation
- **Adapts to changing conditions** (concept drift, user preferences)
- **No retraining required** - updates happen in real-time
- **Low latency** - inference happens in milliseconds
- **Handles cold start** - works even with minimal data

### Key Concepts

**Multi-Armed Bandits (MAB)**: Algorithms that balance exploration (trying new options) vs exploitation (using best known option).

**Contextual Bandits**: MAB that considers request context (user, task type, time) for personalized decisions.

**Reinforcement Learning**: Learning optimal actions by receiving rewards/penalties from environment.

**Concept Drift**: When the statistical properties of data change over time, requiring model adaptation.

## When to Use ML vs A/B Testing

### Use Traditional A/B Testing When:

- **Fixed variants**: You have 2-5 specific configurations to compare
- **Statistical rigor**: You need p-values and confidence intervals
- **Regulatory compliance**: You need auditable, reproducible results
- **Simple decisions**: Choose between discrete options
- **Long-term strategy**: Results inform future development

### Use Online Learning When:

- **Many variants**: Testing 10+ configurations or continuous parameters
- **Dynamic optimization**: Parameters need real-time tuning
- **Personalization**: Different users need different models
- **Rapid adaptation**: Traffic patterns or costs change frequently
- **Resource efficiency**: Can't afford equal traffic to poor variants

### Use Hybrid Approach When:

- **Best of both worlds**: Use A/B testing for major decisions, ML for optimization
- **Staged rollout**: A/B test validates concept, ML optimizes parameters
- **Safety critical**: A/B testing provides guardrails, ML optimizes within bounds

## Algorithm Selection Guide

Choose the right algorithm based on your use case:

### Thompson Sampling

**Best for:** Simple multi-armed bandits without context

**Use when:**
- Choosing between 2-10 discrete options (models, prompts)
- Context doesn't matter (same best choice for all users)
- Need fast convergence with good exploration
- Want Bayesian confidence intervals

**Pros:**
- Theoretically optimal regret bounds
- Natural exploration-exploitation balance
- Works well with limited data

**Cons:**
- No personalization (ignores context)
- Requires success/failure signal
- Can be slow with many arms (>20)

**Example use cases:**
- Model selection (GPT-5 vs Claude-4 vs Gemini-2.5)
- Prompt template selection
- Cache strategy selection

### LinUCB (Linear Upper Confidence Bound)

**Best for:** Contextual bandits with linear rewards

**Use when:**
- Context matters (user, task type, time of day)
- Reward is approximately linear in features
- Need provable regret bounds
- Want deterministic exploration

**Pros:**
- Strong theoretical guarantees
- Handles high-dimensional contexts
- Controlled exploration via alpha parameter

**Cons:**
- Assumes linear reward model
- Matrix inversion can be slow (10+ dimensions)
- Requires feature engineering

**Example use cases:**
- Personalized model routing
- Context-aware parameter tuning
- Task-specific optimization

### Contextual Thompson Sampling

**Best for:** Contextual bandits with non-linear rewards

**Use when:**
- Reward function is non-linear
- Want probabilistic exploration
- Need uncertainty quantification
- Contexts are complex

**Pros:**
- Handles non-linear relationships
- Natural Bayesian inference
- Good exploration in practice

**Cons:**
- No convergence guarantees
- Slower than LinUCB
- Requires tuning learning rate

**Example use cases:**
- Complex personalization
- Multi-feature optimization
- Non-linear cost models

### Bayesian Optimization (Grid/Random/LHS)

**Best for:** Parameter search in continuous spaces

**Use when:**
- Tuning continuous parameters (temperature, top-p)
- Function evaluations are expensive
- Need efficient exploration
- Have 2-10 dimensions

**Pros:**
- Efficient for expensive functions
- Handles continuous spaces
- Good for parameter tuning

**Cons:**
- Slower than bandits
- Doesn't adapt online
- Needs initial exploration phase

**Example use cases:**
- Hyperparameter tuning
- Temperature/top-p optimization
- Prompt engineering

### Pareto Optimization

**Best for:** Multi-objective trade-offs

**Use when:**
- Optimizing multiple objectives (cost vs quality vs latency)
- No single "best" solution
- Need to understand trade-offs
- Want to switch objectives dynamically

**Pros:**
- Shows all trade-offs
- Objective weighting flexibility
- No loss of information

**Cons:**
- Can have many solutions
- Requires post-hoc selection
- More complex to implement

**Example use cases:**
- Cost-quality trade-offs
- Latency-quality balance
- Budget-constrained optimization

## Feature Engineering Guide

Good features are critical for contextual bandits. Here's how to design effective features:

### Feature Design Principles

1. **Relevance**: Features should predict reward
2. **Low correlation**: Avoid redundant features
3. **Normalization**: Scale features to similar ranges [0, 1] or [-1, 1]
4. **Encoding**: Use appropriate encoding for categorical features
5. **Dimensionality**: Keep it manageable (5-20 features typically)

### Built-in Context Features

The `RequestContext` provides these features:

```rust
pub struct RequestContext {
    pub input_length: usize,           // Request size
    pub output_length_category: enum,  // Expected output size
    pub priority: u8,                  // Request priority (1-10)
    pub hour_of_day: u8,              // Time of day (0-23)
    pub day_of_week: u8,              // Day of week (0-6)
    pub task_type: Option<String>,     // Task category
    pub user_id: Option<String>,       // User identifier
    pub region: Option<String>,        // Geographic region
    pub language: Option<String>,      // Language code
}
```

### Feature Vector Encoding

Features are automatically encoded to a 10-dimensional vector:

1. **Input length** (log-scaled, normalized)
2. **Output category** (0.0=short, 0.5=medium, 1.0=long)
3. **Priority** (0.0-1.0)
4. **Hour cos** (cyclical encoding)
5. **Hour sin** (cyclical encoding)
6. **Day cos** (cyclical encoding)
7. **Day sin** (cyclical encoding)
8. **Task type** (ordinal encoding)
9. **Bias term** (always 1.0)
10. **Reserved** (padding)

### Custom Feature Engineering

For advanced use cases, extend the context:

```rust
// Add domain-specific metadata
let context = RequestContext::new(256)
    .with_task_type("code_generation")
    .with_user_id("user_123")
    .with_language("python")
    .with_metadata("complexity", "high")
    .with_metadata("framework", "pytorch");
```

### Feature Engineering Best Practices

**Do:**
- Use cyclical encoding for time (cos/sin)
- Log-scale for heavy-tailed distributions (token counts)
- One-hot or ordinal encoding for categories
- Include interaction terms for non-linear relationships
- Add bias term (constant 1.0)

**Don't:**
- Include high-cardinality features (user IDs without encoding)
- Use raw timestamps (not cyclical)
- Mix scales (normalize!)
- Include target leakage
- Over-engineer (start simple)

## Model Selection Strategy

Choosing the right LLM model is critical for cost and quality. Here's a systematic approach:

### Step 1: Define Objectives

What matters most?

- **Quality-focused**: 80% quality, 15% cost, 5% latency
- **Balanced**: 50% quality, 30% cost, 20% latency
- **Cost-focused**: 40% quality, 50% cost, 10% latency
- **Latency-focused**: 40% quality, 30% cost, 30% latency

### Step 2: Filter Candidates

Start with a reasonable set:

```rust
use decision::{ModelRegistry, Provider, ModelTier};

let registry = ModelRegistry::new();

// Filter by tier
let candidates = registry.models_by_tier(ModelTier::Advanced);

// Or filter by provider
let candidates = registry.models_by_provider(Provider::Anthropic);

// Or specific models
let candidates = vec![
    "gpt-5",
    "claude-sonnet-4.5",
    "gemini-2.5-pro",
];
```

### Step 3: Find Pareto Optimal Set

Eliminate dominated models:

```rust
let pareto_set = registry.find_pareto_optimal(
    &candidates,
    1000,  // input tokens
    500,   // output tokens
)?;

// Pareto set contains non-dominated models
println!("Found {} Pareto-optimal models", pareto_set.len());
```

### Step 4: Select Based on Weights

Choose best for your objectives:

```rust
use decision::{ObjectiveWeights, ParetoFrontier};

let weights = ObjectiveWeights::quality_focused(1.0, 5000.0);
let best = ParetoFrontier::select_optimal(&pareto_set, &weights)?;

println!("Selected: {}", best.name);
println!("Quality: {:.2}", best.objectives.quality);
println!("Cost: ${:.4}", best.objectives.cost);
```

### Step 5: Run Online Optimization

Let the system learn which model works best:

```rust
use decision::{LinUCB, RequestContext};

let mut bandit = LinUCB::new(1.0, RequestContext::feature_dimension());

// Add Pareto-optimal models as arms
for candidate in pareto_set {
    bandit.add_arm(candidate.id);
}

// Online selection and learning
loop {
    let context = get_request_context();
    let model_id = bandit.select_arm(&context)?;

    let response = call_llm(model_id, &context);
    let reward = calculate_reward(&response);

    bandit.update(&model_id, &context, reward)?;
}
```

## A/B Testing Guide

A/B testing provides statistical rigor for validating model changes.

### When to A/B Test

- **Major model upgrades**: GPT-4 → GPT-5
- **Prompt changes**: Significant rewrites
- **Feature launches**: New capabilities
- **Cost optimization**: Validating cheaper models
- **Compliance**: Auditable decisions

### Setting Up an A/B Test

```rust
use decision::{ABTestEngine, Variant};
use llm_optimizer_types::models::ModelConfig;

let engine = ABTestEngine::new(&config);

// Define variants
let control = Variant::new(
    "control",
    ModelConfig {
        model: "gpt-4.5",
        temperature: 0.7,
        max_tokens: 1000,
    },
    0.5,  // 50% traffic
);

let treatment = Variant::new(
    "treatment",
    ModelConfig {
        model: "gpt-5",
        temperature: 0.7,
        max_tokens: 1000,
    },
    0.5,  // 50% traffic
);

// Create experiment
let experiment_id = engine.create_experiment(
    "gpt-5-validation",
    vec![control, treatment],
)?;

engine.start(&experiment_id)?;
```

### Running the Test

```rust
// Assign variant
let (variant_id, config) = engine.assign_variant(&experiment_id)?;

// Run LLM
let response = call_llm(&config);

// Record outcome
engine.record_outcome(
    &experiment_id,
    &variant_id,
    response.success,
    response.quality,
    response.cost,
    response.latency_ms,
)?;
```

### Analyzing Results

```rust
// Get statistics
let stats = engine.get_statistics(&experiment_id)?;

for variant in &stats.variants {
    println!("{}: {:.1}% conversion, {:.3} quality",
        variant.name,
        variant.conversion_rate * 100.0,
        variant.avg_quality,
    );
}

// Check for significance
if stats.is_significant(0.05) {
    let winner = stats.get_winner();
    println!("Winner: {}", winner.name);
}
```

### Sample Size Calculation

```rust
use decision::SampleSizeCalculator;

let calculator = SampleSizeCalculator::new();

let sample_size = calculator.required_sample_size(
    0.25,  // baseline conversion rate
    0.05,  // minimum detectable effect (5%)
    0.05,  // significance level (alpha)
    0.80,  // power (1 - beta)
);

println!("Need {} samples per variant", sample_size);
```

## Monitoring and Debugging

Monitor your ML systems to ensure they're working correctly.

### Key Metrics to Track

**Learning Metrics:**
- **Regret**: Cumulative difference from optimal choice
- **Exploration rate**: % of requests used for exploration
- **Convergence**: Has the model stabilized?
- **Arm selection distribution**: Are arms being tried fairly?

**Performance Metrics:**
- **Average reward**: Overall system performance
- **Reward variance**: Stability of rewards
- **Cost per request**: Financial impact
- **Latency p95**: User experience

**System Health:**
- **Drift detection**: Has data distribution changed?
- **Anomaly detection**: Unusual patterns
- **Error rates**: System failures
- **Update frequency**: Learning velocity

### Monitoring Dashboard

```rust
use decision::{ThresholdMonitoringSystem, ThresholdConfig, AlertType};

let mut monitor = ThresholdMonitoringSystem::new();

// Define thresholds
monitor.add_threshold(ThresholdConfig {
    metric_name: "average_reward",
    threshold: 0.7,
    comparison: Comparison::GreaterThan,
    alert_type: AlertType::Critical,
});

// Check metrics
let metrics = collect_metrics();
let alerts = monitor.check_all(&metrics)?;

for alert in alerts {
    if alert.severity == AlertSeverity::Critical {
        send_alert(&alert);
    }
}
```

### Drift Detection

```rust
use decision::{ADWIN, DriftStatus};

let mut detector = ADWIN::new(0.002, 1000)?;

loop {
    let reward = get_reward();
    let status = detector.add(reward);

    match status {
        DriftStatus::Drift => {
            println!("Drift detected! Retraining...");
            retrain_model();
        }
        DriftStatus::Warning => {
            println!("Possible drift warning");
        }
        DriftStatus::Stable => {
            // All good
        }
    }
}
```

### Anomaly Detection

```rust
use decision::{ZScoreDetector, AnomalyResult};

let mut detector = ZScoreDetector::new(100, 3.0)?;

let result = detector.add(current_cost);

if result.is_anomaly {
    println!("Anomaly detected! Severity: {:.2}", result.severity);
    investigate_anomaly();
}
```

## Best Practices

### 1. Start Simple, Then Optimize

Don't over-engineer from the start:

**Phase 1**: Thompson Sampling for model selection
- Simple, proven algorithm
- Fast convergence
- Easy to understand

**Phase 2**: Add context with LinUCB
- Personalization
- Context-aware routing
- Better performance

**Phase 3**: Advanced optimization
- Parameter tuning
- Multi-objective optimization
- Custom reward functions

### 2. Design Reward Functions Carefully

Your reward function determines what the system optimizes:

```rust
use decision::{RewardCalculator, RewardWeights};

// Quality-focused (default)
let weights = RewardWeights::default_weights();  // 60% quality, 30% cost, 10% latency

// Cost-focused
let weights = RewardWeights::cost_focused();     // 40% quality, 50% cost, 10% latency

// Custom
let weights = RewardWeights::new(0.7, 0.2, 0.1);
```

### 3. Balance Exploration vs Exploitation

Too much exploration → high costs, poor user experience
Too little exploration → miss better options, stagnate

**For LinUCB:**
- Start with alpha=1.0 (balanced)
- Increase to 2.0 for more exploration
- Decrease to 0.5 for more exploitation

**For Thompson Sampling:**
- Exploration is automatic
- Converges naturally over time
- Use epsilon-greedy if needed

### 4. Handle Cold Start

When adding new arms with no data:

```rust
// Option 1: Optimistic initialization
let mut arm = LinUCBArm::new(arm_id, dimension);
// LinUCB naturally handles this via uncertainty

// Option 2: Forced exploration
let exploration_rounds = 10;
if arm.num_selections < exploration_rounds {
    // Force selection of new arms
}

// Option 3: Transfer learning
// Use similar arm's statistics as prior
```

### 5. Monitor Regret

Regret measures how much performance you've lost due to exploration:

```rust
let regret = bandit.calculate_regret();
let regret_rate = regret / bandit.total_samples() as f64;

if regret_rate > 0.1 {
    println!("High regret! Consider reducing exploration");
}
```

### 6. Test Before Production

Always validate in staging:

```rust
// Use offline evaluation first
let mut bandit = LinUCB::new(1.0, 10);

for (context, reward, arm_id) in historical_data {
    let predicted_arm = bandit.select_arm(&context)?;
    bandit.update(&arm_id, &context, reward)?;

    if predicted_arm == arm_id {
        correct_predictions += 1;
    }
}

let accuracy = correct_predictions as f64 / total as f64;
println!("Offline accuracy: {:.2}%", accuracy * 100.0);
```

### 7. Use Safety Guardrails

Prevent catastrophic failures:

```rust
// Minimum traffic allocation
if arm.traffic_allocation < 0.05 {
    arm.traffic_allocation = 0.05;  // Always give 5% minimum
}

// Maximum cost threshold
if predicted_cost > max_cost_threshold {
    return Err("Cost too high");
}

// Quality floor
if predicted_quality < min_quality {
    return Err("Quality too low");
}
```

### 8. Version Control Everything

Track changes to:
- Reward functions
- Feature engineering
- Algorithm parameters
- Model configurations

```rust
#[derive(Serialize, Deserialize)]
struct OptimizationConfig {
    version: String,
    timestamp: DateTime<Utc>,
    algorithm: String,
    parameters: HashMap<String, f64>,
    reward_weights: RewardWeights,
}
```

### 9. A/B Test Major Changes

Before deploying new ML algorithms:

1. Run A/B test: current system vs new system
2. Measure: regret, reward, cost, latency
3. Validate: statistical significance
4. Rollout: gradual traffic increase

### 10. Document Your Decisions

Keep a decision log:

```
2025-11-10: Switched from Thompson to LinUCB
  Reason: Enable personalization by user segment
  Impact: 12% improvement in average reward

2025-11-15: Added drift detection
  Reason: Traffic patterns changing weekly
  Impact: Detected 3 drift events, auto-retrained
```

## Next Steps

- [Algorithm Reference](ML_ALGORITHM_REFERENCE.md) - Detailed algorithm documentation
- [Tutorial Guide](ML_TUTORIAL.md) - Step-by-step lessons
- [Examples](ML_EXAMPLES.md) - 20+ real-world use cases
- [API Reference](ML_API_REFERENCE.md) - Complete Rust API
- [Best Practices](ML_BEST_PRACTICES.md) - Advanced optimization tips
- [Troubleshooting](ML_TROUBLESHOOTING.md) - Common issues and solutions
