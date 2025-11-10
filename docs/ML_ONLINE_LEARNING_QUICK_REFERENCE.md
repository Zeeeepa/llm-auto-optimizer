# Online Learning Quick Reference

Quick reference guide for online learning algorithms in LLM Auto-Optimizer.

---

## Algorithm Selection Guide

### Decision Tree

```
Is context important?
├─ YES → Use LinUCB or Contextual Thompson Sampling
│         Example: Model selection based on request features
└─ NO  → Is theoretical guarantee needed?
          ├─ YES → Use UCB1
          │         Example: Prompt selection with proven regret bounds
          └─ NO  → Use Thompson Sampling
                    Example: A/B testing variants

Is the problem continuous optimization?
├─ YES → Use Bayesian Optimization
│         Example: Hyperparameter tuning (temperature, top_p)
└─ NO  → Use bandits (see above)

Are decisions sequential?
├─ YES → Use Q-Learning
│         Example: Multi-step pipeline optimization
└─ NO  → Use bandits (see above)

Need to predict outcomes?
└─ YES → Use Online Gradient Descent
          Example: Quality score prediction
```

---

## Algorithm Cheat Sheet

### Thompson Sampling

**When to use:**
- A/B testing
- Simple variant selection
- Need Bayesian uncertainty

**Code:**
```rust
let mut ts = ThompsonSampling::new();
ts.add_variant(variant_a);
ts.add_variant(variant_b);

let selected = ts.select_variant()?;
// ... execute ...
ts.update(&selected, success)?;
```

**Pros:**
- Best empirical performance
- Natural uncertainty quantification
- Handles delayed feedback

**Cons:**
- Requires sampling (random)
- No deterministic guarantees

---

### UCB1

**When to use:**
- Need regret guarantees
- Deterministic selection preferred
- Simple arm selection

**Code:**
```rust
let mut ucb = UCB1::with_default();
ucb.add_arm(arm_id);

let selected = ucb.select_arm()?;
// ... execute ...
ucb.update(&selected, reward)?;
```

**Pros:**
- O(√(KT log T)) regret bound
- Deterministic
- Simple and fast

**Cons:**
- Not contextual
- More conservative than Thompson

---

### LinUCB

**When to use:**
- Context-aware decisions
- Model/configuration selection
- Request features available

**Code:**
```rust
let mut linucb = LinUCB::new(alpha, dimension);
linucb.add_arm(arm_id);

let context = RequestContext::new(100)
    .with_task_type("generation");

let selected = linucb.select_arm(&context)?;
// ... execute ...
linucb.update(&selected, &context, reward)?;
```

**Pros:**
- Leverages context features
- Theoretical guarantees
- Adaptive to user segments

**Cons:**
- O(Kd²) complexity
- Requires feature engineering

---

### Epsilon-Greedy

**When to use:**
- Baseline comparison
- Simple exploration needed
- Interpretability important

**Code:**
```rust
let mut eg = EpsilonGreedy::new(0.1, decay=true);
eg.add_arm(arm_id);

let selected = eg.select_arm()?;
// ... execute ...
eg.update(&selected, reward)?;
```

**Pros:**
- Extremely simple
- Easy to understand
- Fast

**Cons:**
- Suboptimal regret O(T^(2/3))
- Fixed exploration rate

---

### Bayesian Optimization

**When to use:**
- Continuous parameter tuning
- Expensive evaluations
- Few samples available

**Code:**
```rust
let kernel = RBFKernel::new(length_scale, variance);
let mut bo = BayesianOptimizer::new(bounds, kernel);

for _ in 0..max_iterations {
    let params = bo.suggest();
    let reward = evaluate(params);
    bo.observe(params, reward)?;
}
```

**Pros:**
- Sample efficient
- Handles continuous spaces
- Quantifies uncertainty

**Cons:**
- O(n³) complexity
- Slower than bandits

---

### Q-Learning

**When to use:**
- Sequential decisions
- Multi-step optimization
- State transitions matter

**Code:**
```rust
let mut q = QLearning::new(alpha, gamma, epsilon);

let state = get_state();
let action = q.select_action(&state, &actions);
// ... execute action ...
let (next_state, reward) = step(action);

q.update(state, action, reward, next_state, &next_actions);
```

**Pros:**
- Handles sequences
- Credit assignment
- Flexible

**Cons:**
- State space explosion
- Slower convergence

---

## Performance Comparison

| Algorithm | Latency | Memory | Sample Efficiency | Regret Bound |
|-----------|---------|--------|-------------------|--------------|
| Thompson Sampling | ~0.1ms | 32B/arm | High | O(√KT) |
| UCB1 | ~0.05ms | 24B/arm | High | O(√(KT log T)) |
| Epsilon-Greedy | ~0.05ms | 24B/arm | Medium | O(T^(2/3)) |
| LinUCB | ~5ms | 8d²+8d B/arm | High | O(d√(T log T)) |
| Bayesian Opt | ~100ms | O(n²) | Very High | O(√T) |
| Q-Learning | ~0.1ms | O(S×A) | Medium | O(√T) |

---

## Configuration Examples

### Production Model Selection

```rust
// Use LinUCB for context-aware selection
let optimizer = ParameterOptimizer::with_defaults();

let policy = OptimizationPolicy::new(
    "production_models",
    ParameterRange::default(),
    OptimizationMode::Balanced,
);

optimizer.create_policy(policy)?;
optimizer.initialize_with_search(
    "production_models",
    SearchStrategy::Random,
    20
)?;

// Select model based on context
let context = RequestContext::new(input_length)
    .with_task_type("code_generation")
    .with_priority(8);

let (config_id, config) = optimizer.select_parameters("production_models", &context)?;
```

### A/B Testing

```rust
// Use Thompson Sampling for traffic allocation
let mut ts = ThompsonSampling::new();
ts.add_variant(variant_a);
ts.add_variant(variant_b);

// Automatic exploration-exploitation balance
let selected = ts.select_variant()?;

// Update with success/failure
let success = user_satisfied(&response);
ts.update(&selected, success)?;
```

### Hyperparameter Tuning

```rust
// Use Bayesian Optimization for expensive tuning
let bounds = vec![
    (0.0, 2.0),   // temperature
    (0.0, 1.0),   // top_p
];

let kernel = RBFKernel::new(0.5, 1.0);
let mut bo = BayesianOptimizer::new(bounds, kernel);

for iteration in 0..50 {
    let params = bo.suggest();
    let quality = evaluate_llm(params);
    bo.observe(params, quality)?;
}
```

---

## Reward Design Patterns

### Quality-Cost Tradeoff

```rust
let reward = RewardCalculator::new(
    RewardWeights::new(0.6, 0.3, 0.1), // quality, cost, latency
    max_cost = 1.0,
    max_latency = 5000.0,
);

let r = reward.calculate_reward(&metrics, &feedback);
```

### Delayed Feedback

```rust
// Immediate update with metrics
handler.update_immediate(request_id, immediate_metrics);

// Later: correction update with user feedback
handler.update_delayed(request_id, user_feedback);
```

### Multi-Objective

```rust
// Pareto optimization for multiple objectives
let objectives = Objectives {
    quality: 0.9,
    cost: 0.1,
    latency: 1000.0,
};

let pareto_front = ParetoFrontier::new();
pareto_front.add_candidate(config, objectives);
let best = pareto_front.select_best(&weights)?;
```

---

## Exploration Strategies

### Decaying Epsilon

```rust
let strategy = DecayingEpsilonGreedy {
    epsilon_start: 0.5,
    epsilon_end: 0.01,
    decay_steps: 10000,
};

if strategy.should_explore(timestep) {
    // Explore
} else {
    // Exploit
}
```

### Forced Exploration

```rust
let forced = ForcedExploration {
    min_trials_per_arm: 10,
};

if forced.needs_exploration(&action) {
    // Force this action for minimum trials
}
```

### Safe Exploration

```rust
let safe = SafeExploration {
    sla_threshold: 0.8,
    safe_actions: known_safe_set,
};

if safe.is_safe(&action, estimated_quality) {
    // Safe to try this action
}
```

---

## Common Pitfalls

### 1. Insufficient Exploration

**Problem:** Algorithm converges too early to suboptimal arm.

**Solution:**
```rust
// Ensure minimum trials per arm
let forced_exploration = ForcedExploration::new(min_trials: 50);

// Or increase exploration parameter
ucb.set_c(2.0); // More exploration
linucb.set_alpha(2.0);
```

### 2. Context Feature Quality

**Problem:** Poor context features lead to poor LinUCB performance.

**Solution:**
```rust
// Normalize features
context.normalize();

// Use domain knowledge for feature engineering
let features = vec![
    input_length.ln(),              // Log scale
    hour_of_day.cos(),              // Cyclical
    is_premium_user as f64,         // Binary
    task_complexity_score,          // Continuous
];
```

### 3. Reward Signal Noise

**Problem:** Noisy rewards slow convergence.

**Solution:**
```rust
// Use reward smoothing
let smoothed_reward = 0.7 * current_reward + 0.3 * previous_avg;

// Or use multiple signals
let combined = 0.5 * quality_score + 0.5 * user_feedback;
```

### 4. Non-Stationary Environments

**Problem:** Best arm changes over time.

**Solution:**
```rust
// Use sliding window
let windowed_ts = WindowedThompsonSampling::new(window_size: 1000);

// Or discount old data
let discounted_reward = current_reward * discount_factor.powf(age);
```

---

## Monitoring and Debugging

### Key Metrics

```rust
// Track per algorithm
metrics.total_selections.inc();
metrics.average_reward.set(algo.stats().average_reward);
metrics.estimated_regret.set(algo.stats().estimated_regret);
metrics.selection_latency.observe(duration);
```

### Debug Logs

```rust
debug!("Algorithm: {}, Arm: {}, UCB: {:.3}, Mean: {:.3}, Bonus: {:.3}",
    algo_name, arm_id, ucb_score, mean_reward, exploration_bonus);
```

### Convergence Check

```rust
if algo.has_converged(threshold: 0.01) {
    info!("Algorithm converged, switching to exploit mode");
    optimizer.set_mode("policy_name", OptimizationMode::Exploit)?;
}
```

---

## Testing Checklist

- [ ] Unit tests for each algorithm
- [ ] Convergence tests (does it learn?)
- [ ] Regret bounds tests (within theory?)
- [ ] Performance benchmarks (<1ms target)
- [ ] Integration tests (end-to-end)
- [ ] Safety mechanism tests (SLA protection)
- [ ] Non-stationary tests (adapts to change?)
- [ ] Stress tests (high load)

---

## Further Reading

- Main Architecture: `/docs/ML_ONLINE_LEARNING_ARCHITECTURE.md`
- Existing Implementations:
  - Thompson Sampling: `/crates/decision/src/thompson_sampling.rs`
  - LinUCB: `/crates/decision/src/contextual_bandit.rs`
  - Reward Calculator: `/crates/decision/src/reward.rs`
- Parameter Optimizer: `/crates/decision/src/parameter_optimizer.rs`

---

**Last Updated:** 2025-11-10
