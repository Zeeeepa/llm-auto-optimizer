# ML Integration Troubleshooting Guide

Common issues and solutions for ML-based LLM optimization.

## Table of Contents

1. [Model Not Learning](#model-not-learning)
2. [High Regret](#high-regret)
3. [Poor Performance](#poor-performance)
4. [Drift Detected](#drift-detected)
5. [Slow Inference](#slow-inference)
6. [Memory Issues](#memory-issues)
7. [Convergence Problems](#convergence-problems)
8. [Data Quality Issues](#data-quality-issues)
9. [Configuration Errors](#configuration-errors)
10. [Production Issues](#production-issues)

---

## Model Not Learning

### Symptom

- Conversion rates not changing
- All arms have similar rewards
- No clear winner emerging
- Regret not decreasing

### Diagnosis

```rust
// Check if model is being updated
let stats = bandit.get_arms();
for (id, arm) in stats {
    println!("Arm {}: selections={}, reward={:.3}",
        id, arm.num_selections, arm.average_reward());
}
```

### Common Causes & Solutions

#### 1. Rewards Not Scaled Properly

**Problem:** Rewards outside [0, 1] range

```rust
// Bad
let reward = quality - cost;  // Can be negative or >1

// Fix
let reward = (quality * 0.8 + (1.0 - cost / max_cost) * 0.2).clamp(0.0, 1.0);
```

#### 2. No Variance in Rewards

**Problem:** All arms have same performance

```rust
// Check reward variance
let rewards: Vec<f64> = stats.values()
    .map(|arm| arm.average_reward())
    .collect();

let mean = rewards.iter().sum::<f64>() / rewards.len() as f64;
let variance = rewards.iter()
    .map(|r| (r - mean).powi(2))
    .sum::<f64>() / rewards.len() as f64;

if variance < 0.001 {
    println!("⚠ Low variance: All arms perform similarly");
}
```

**Fix:** Check if arms are actually different

#### 3. Wrong Success Criterion

**Problem:** Using success/failure incorrectly

```rust
// Bad: Everything is "success"
let success = true;  // ❌

// Fix: Use meaningful threshold
let success = reward > 0.75;  // ✓
```

#### 4. Not Enough Data

**Problem:** Need more samples

```rust
if bandit.total_samples() < 100 {
    println!("⚠ Need at least 100 samples for convergence");
}
```

**Fix:** Run longer or increase traffic

---

## High Regret

### Symptom

- Regret rate > 10%
- Best arm not receiving most traffic
- Exploring too much

### Diagnosis

```rust
let regret = bandit.calculate_regret();
let regret_rate = regret / bandit.total_samples() as f64;

println!("Regret rate: {:.1}%", regret_rate * 100.0);

if regret_rate > 0.1 {
    println!("⚠ High regret!");
}
```

### Solutions

#### 1. Reduce Exploration

**LinUCB:** Lower alpha

```rust
// Before
let mut bandit = LinUCB::new(2.0, dimension);  // Too much exploration

// After
bandit.set_alpha(0.5);  // More exploitation
```

**Parameter Optimizer:** Lower exploration rate

```rust
// Before
let policy = policy.with_exploration_rate(0.3);  // Too much

// After
let policy = policy.with_exploration_rate(0.1);  // Less exploration
```

#### 2. Switch to Exploitation Mode

```rust
optimizer.set_mode("policy_name", OptimizationMode::Exploit)?;
```

#### 3. Remove Poor Performers

```rust
// Remove arms with consistently low rewards
let avg_rewards = bandit.get_average_rewards();
let overall_avg: f64 = avg_rewards.values().sum::<f64>() / avg_rewards.len() as f64;

for (id, reward) in avg_rewards {
    if reward < overall_avg * 0.7 {
        println!("Removing poor performer: {}", id);
        bandit.remove_arm(&id);
    }
}
```

---

## Poor Performance

### Symptom

- System performs worse than baseline
- Quality degraded
- Costs increased

### Diagnosis

#### 1. Compare to Baseline

```rust
let ml_quality = calculate_average_quality(&ml_responses);
let baseline_quality = calculate_average_quality(&baseline_responses);

if ml_quality < baseline_quality * 0.95 {
    println!("⚠ ML system {:.1}% worse than baseline",
        (1.0 - ml_quality / baseline_quality) * 100.0);
}
```

#### 2. Check Feature Quality

```rust
// Verify features are meaningful
let context = RequestContext::new(200);
let features = context.to_feature_vector();

println!("Features:");
for (i, f) in features.iter().enumerate() {
    println!("  [{}]: {:.4}", i, f);
    if f.is_nan() || f.is_infinite() {
        println!("    ⚠ Invalid feature!");
    }
}
```

### Solutions

#### 1. Feature Engineering Issues

**Problem:** Features don't predict rewards

```rust
// Check feature-reward correlation
let mut feature_reward_corr = vec![0.0; 10];

for (context, reward) in history {
    let features = context.to_feature_vector();
    for i in 0..10 {
        feature_reward_corr[i] += features[i] * reward;
    }
}

for (i, corr) in feature_reward_corr.iter().enumerate() {
    println!("Feature {} correlation: {:.3}", i, corr);
    if corr.abs() < 0.01 {
        println!("  ⚠ Low correlation - feature may not be useful");
    }
}
```

**Fix:** Add better features or switch to Thompson Sampling (no features needed)

#### 2. Wrong Algorithm Choice

**Problem:** LinUCB when Thompson Sampling would work better

```rust
// If context doesn't help, use Thompson Sampling
if feature_reward_corr.iter().all(|&c| c.abs() < 0.05) {
    println!("Context doesn't help - switch to Thompson Sampling");
}
```

#### 3. Reward Function Misalignment

**Problem:** Optimizing wrong objective

```rust
// Check what system is optimizing vs what you want
let actual_cost_reduction = measure_cost_reduction();
let predicted_reward_increase = measure_reward_increase();

if actual_cost_reduction < 0.0 && predicted_reward_increase > 0.0 {
    println!("⚠ Reward function not aligned with business goal!");
}
```

**Fix:** Redesign reward function

---

## Drift Detected

### Symptom

- ADWIN detects drift
- Performance suddenly degrades
- Best arm changes unexpectedly

### Diagnosis

```rust
let mut detector = ADWIN::new(0.002, 1000)?;

for reward in recent_rewards {
    let status = detector.add(reward);
    if status == DriftStatus::Drift {
        println!("Drift detected at sample {}", i);
    }
}
```

### Solutions

#### 1. Increase Exploration

```rust
// Adapt to drift by exploring more
if drift_detected {
    let current_alpha = bandit.alpha();
    bandit.set_alpha(current_alpha * 1.5);

    log::warn!("Drift detected, increased alpha to {}", bandit.alpha());
}
```

#### 2. Reset Learning

```rust
// Nuclear option: start fresh
if severe_drift {
    log::error!("Severe drift - resetting bandit");

    let old_bandit = bandit;
    let mut new_bandit = LinUCB::new(1.5, dimension);

    // Re-add arms
    for arm_id in old_bandit.get_arms().keys() {
        new_bandit.add_arm(*arm_id);
    }

    bandit = new_bandit;
}
```

#### 3. Transfer Learning

```rust
// Soft reset: reduce confidence in old data
fn soft_reset(arm: &mut LinUCBArm, decay: f64) {
    arm.a_matrix = arm.a_matrix.iter()
        .map(|row| row.iter().map(|&v| v * decay).collect())
        .collect();

    arm.b_vector = arm.b_vector.iter()
        .map(|&v| v * decay)
        .collect();
}
```

---

## Slow Inference

### Symptom

- High latency in model selection
- Timeouts
- Poor user experience

### Diagnosis

```rust
use std::time::Instant;

let start = Instant::now();
let model = bandit.select_arm(&context)?;
let duration = start.elapsed();

if duration.as_millis() > 10 {
    println!("⚠ Slow selection: {}ms", duration.as_millis());
}
```

### Solutions

#### 1. Reduce Feature Dimension

```rust
// Instead of 20 features, use 10
impl RequestContext {
    fn to_compact_feature_vector(&self) -> Vec<f64> {
        vec![
            self.input_length_normalized(),
            self.output_category_encoded(),
            self.priority_normalized(),
            // ... only essential features
        ]
    }
}
```

#### 2. Cache Feature Computation

```rust
use lru::LruCache;

struct CachedBandit {
    bandit: LinUCB,
    feature_cache: LruCache<String, Vec<f64>>,
}

impl CachedBandit {
    fn select(&mut self, context: &RequestContext) -> Result<Uuid> {
        let key = context_cache_key(context);

        let features = if let Some(cached) = self.feature_cache.get(&key) {
            cached.clone()
        } else {
            let features = context.to_feature_vector();
            self.feature_cache.put(key, features.clone());
            features
        };

        // Use cached features for selection
        self.bandit.select_arm_with_features(&features)
    }
}
```

#### 3. Async Updates

```rust
// Don't block on updates
tokio::spawn(async move {
    bandit.update(&arm_id, &context, reward).ok();
});
```

#### 4. Use Faster Algorithm

```rust
// LinUCB is O(d³), Thompson Sampling is O(1)
if dimension > 20 {
    println!("Consider Thompson Sampling for lower latency");
}
```

---

## Memory Issues

### Symptom

- OOM errors
- Increasing memory usage
- Slow garbage collection

### Diagnosis

```rust
// Check arm count
println!("Number of arms: {}", bandit.get_arms().len());

// Check window sizes
println!("ADWIN window size: {}", drift_detector.window_size());
```

### Solutions

#### 1. Limit Window Sizes

```rust
// Bounded windows
let detector = ADWIN::new(0.002, 1000)?;  // Max 1000 samples
let anomaly = ZScoreDetector::new(100, 3.0)?;  // Max 100 samples
```

#### 2. Prune Arms

```rust
const MAX_ARMS: usize = 50;

if bandit.get_arms().len() > MAX_ARMS {
    // Remove least-used arms
    let mut arms: Vec<_> = bandit.get_arms()
        .iter()
        .map(|(id, arm)| (*id, arm.num_selections))
        .collect();

    arms.sort_by_key(|(_, count)| *count);

    // Remove bottom 10%
    let to_remove = arms.len() / 10;
    for (id, _) in arms.iter().take(to_remove) {
        bandit.remove_arm(id);
    }
}
```

#### 3. Periodic Compaction

```rust
// Periodically reset old arms
if total_samples % 10000 == 0 {
    for (_, arm) in bandit.get_arms_mut() {
        if arm.num_selections < 10 {
            // Reset rarely-used arms
            *arm = LinUCBArm::new(arm.arm_id, arm.d);
        }
    }
}
```

---

## Convergence Problems

### Symptom

- Model doesn't converge
- Oscillating between arms
- Unstable selection

### Diagnosis

```rust
// Track selection distribution over time
let mut selection_history: Vec<HashMap<Uuid, usize>> = Vec::new();

for window in recent_selections.chunks(100) {
    let mut dist = HashMap::new();
    for &arm in window {
        *dist.entry(arm).or_insert(0) += 1;
    }
    selection_history.push(dist);
}

// Check if distribution is stabilizing
let stability = calculate_stability(&selection_history);
if stability < 0.8 {
    println!("⚠ Unstable convergence");
}
```

### Solutions

#### 1. Reduce Learning Rate

```rust
// For Contextual Thompson Sampling
arm.learning_rate = 0.05;  // Slower, more stable
```

#### 2. Increase Sample Size

```rust
if total_samples < 500 {
    println!("Need more data for convergence (current: {})", total_samples);
}
```

#### 3. Check for Contradictory Feedback

```rust
// Same context, different rewards
let mut context_rewards: HashMap<String, Vec<f64>> = HashMap::new();

for (context, reward) in history {
    let key = format!("{:?}", context);
    context_rewards.entry(key).or_insert_with(Vec::new).push(reward);
}

for (ctx, rewards) in context_rewards {
    let variance = calculate_variance(&rewards);
    if variance > 0.1 {
        println!("⚠ High variance for context: {}", ctx);
    }
}
```

---

## Data Quality Issues

### Symptom

- Inconsistent rewards
- Missing context data
- Extreme outliers

### Diagnosis

```rust
// Check reward distribution
let rewards: Vec<f64> = collect_recent_rewards();

let mean = rewards.iter().sum::<f64>() / rewards.len() as f64;
let std_dev = (rewards.iter()
    .map(|r| (r - mean).powi(2))
    .sum::<f64>() / rewards.len() as f64)
    .sqrt();

println!("Reward mean: {:.3}, std: {:.3}", mean, std_dev);

// Check for outliers
let outliers: Vec<_> = rewards.iter()
    .filter(|&&r| (r - mean).abs() > 3.0 * std_dev)
    .collect();

if !outliers.is_empty() {
    println!("⚠ Found {} outliers", outliers.len());
}
```

### Solutions

#### 1. Filter Outliers

```rust
fn robust_reward(raw_reward: f64, history: &[f64]) -> f64 {
    let median = calculate_median(history);
    let mad = calculate_mad(history);

    // Clip to [median - 3*MAD, median + 3*MAD]
    let lower = median - 3.0 * mad;
    let upper = median + 3.0 * mad;

    raw_reward.clamp(lower, upper)
}
```

#### 2. Validate Input

```rust
fn validate_context(context: &RequestContext) -> Result<()> {
    if context.input_length == 0 {
        return Err("Invalid input length");
    }

    if context.priority > 10 {
        return Err("Invalid priority");
    }

    Ok(())
}
```

#### 3. Handle Missing Data

```rust
fn handle_missing_feedback(
    arm_id: &Uuid,
    context: &RequestContext,
    timeout: Duration
) -> Option<f64> {
    match timeout_future(get_feedback(), timeout).await {
        Ok(reward) => Some(reward),
        Err(_) => {
            // Use pessimistic default
            log::warn!("Feedback timeout for arm {}", arm_id);
            Some(0.5)  // Neutral reward
        }
    }
}
```

---

## Configuration Errors

### Common Errors & Fixes

#### 1. Feature Dimension Mismatch

**Error:**
```
Context dimension 5 doesn't match arm dimension 10
```

**Fix:**
```rust
// Ensure consistency
let dimension = RequestContext::feature_dimension();
let bandit = LinUCB::new(1.0, dimension);
```

#### 2. Invalid Alpha

**Error:**
```
Alpha must be positive
```

**Fix:**
```rust
// Valid range: 0.1 to 5.0
let alpha = alpha.clamp(0.1, 5.0);
let bandit = LinUCB::new(alpha, dimension);
```

#### 3. Empty Arms

**Error:**
```
No arms available for selection
```

**Fix:**
```rust
if bandit.get_arms().is_empty() {
    // Add default arms
    bandit.add_arm(default_model_id);
}
```

#### 4. Invalid Parameter Range

**Error:**
```
Temperature must be > 0
```

**Fix:**
```rust
let config = ParameterConfig::new(
    temperature.max(0.01),  // Ensure positive
    top_p.clamp(0.0, 1.0),   // Ensure [0, 1]
    max_tokens.max(1),       // Ensure positive
)?;
```

---

## Production Issues

### Issue 1: Traffic Imbalance

**Problem:** One arm gets all traffic

```rust
let distribution = get_traffic_distribution();

for (arm, percentage) in distribution {
    println!("Arm {}: {:.1}%", arm, percentage * 100.0);
}
```

**Solution:** Force minimum allocation

```rust
const MIN_TRAFFIC: f64 = 0.05;

for (arm_id, arm) in bandit.get_arms() {
    let traffic_share = arm.num_selections as f64 / total as f64;
    if traffic_share < MIN_TRAFFIC {
        // Force selection
        return Some(*arm_id);
    }
}
```

### Issue 2: Delayed Metrics

**Problem:** Rewards arrive late

**Solution:** Use time-windowed updates

```rust
struct DelayedUpdate {
    arm_id: Uuid,
    context: RequestContext,
    timestamp: Instant,
}

// Queue updates
let mut pending_updates = VecDeque::new();
pending_updates.push_back(DelayedUpdate { ... });

// Process when reward arrives
if let Some(reward) = check_for_reward(request_id) {
    if let Some(update) = pending_updates.pop_front() {
        bandit.update(&update.arm_id, &update.context, reward)?;
    }
}
```

### Issue 3: Failed LLM Calls

**Problem:** How to handle failures

**Solution:** Separate error handling

```rust
match call_llm(model_id) {
    Ok(response) => {
        let reward = calculate_reward(&response);
        bandit.update(&model_id, &context, reward)?;
    }
    Err(e) => {
        // Don't update bandit for infrastructure failures
        log::error!("LLM call failed: {}", e);

        // But do track model failures
        if e.is_model_error() {
            bandit.update(&model_id, &context, 0.0)?;  // Penalize
        }
    }
}
```

---

## Debug Checklist

When things go wrong, check:

- [ ] Are rewards in [0, 1] range?
- [ ] Is there variance in rewards?
- [ ] Are features scaled/normalized?
- [ ] Does context match algorithm?
- [ ] Is there enough data (>100 samples)?
- [ ] Are updates happening?
- [ ] Is regret decreasing?
- [ ] Are all arms being tried?
- [ ] Is drift detector working?
- [ ] Are logs showing selections?

## Getting Help

If issues persist:

1. **Enable debug logging**:
   ```rust
   env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
   ```

2. **Export diagnostics**:
   ```rust
   let diagnostics = collect_diagnostics(&bandit);
   save_to_file("diagnostics.json", &diagnostics)?;
   ```

3. **Check documentation**:
   - [User Guide](ML_USER_GUIDE.md)
   - [API Reference](ML_API_REFERENCE.md)
   - [Best Practices](ML_BEST_PRACTICES.md)

4. **File an issue** with:
   - Algorithm used
   - Configuration
   - Sample data
   - Observed behavior
   - Expected behavior
