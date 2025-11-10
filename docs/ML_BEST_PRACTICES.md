# ML Integration Best Practices

Advanced optimization tips and patterns for production ML systems.

## Table of Contents

1. [Feature Engineering](#feature-engineering)
2. [Algorithm Selection](#algorithm-selection)
3. [Hyperparameter Tuning](#hyperparameter-tuning)
4. [Reward Design Patterns](#reward-design-patterns)
5. [Exploration Strategies](#exploration-strategies)
6. [Safety Mechanisms](#safety-mechanisms)
7. [Performance Optimization](#performance-optimization)
8. [Debugging Techniques](#debugging-techniques)
9. [Common Pitfalls](#common-pitfalls)
10. [Production Checklist](#production-checklist)

---

## Feature Engineering

### Design Effective Features

**Do:**
- Use domain knowledge to create meaningful features
- Normalize features to similar scales
- Use cyclical encoding for periodic features (time, angles)
- Include interaction terms when relationships are non-linear
- Keep feature vectors small (5-20 dimensions optimal)

**Don't:**
- Use high-cardinality categorical features directly
- Mix scales (normalize!)
- Include target leakage
- Over-engineer (start simple, add complexity if needed)

### Example: Good Feature Engineering

```rust
// Good: Cyclical time encoding
let hour_rad = (hour as f64 / 24.0) * 2.0 * PI;
let hour_cos = hour_rad.cos();
let hour_sin = hour_rad.sin();

// Good: Log-scale for heavy-tailed distributions
let input_length_feature = (input_length as f64 + 1.0).ln() / 10.0;

// Good: Ordinal encoding for ordered categories
let complexity = match level {
    "low" => 0.0,
    "medium" => 0.5,
    "high" => 1.0,
};
```

### Anti-Patterns

```rust
// Bad: Raw timestamp (not cyclical)
let hour_feature = hour as f64;  // ❌ 23 and 0 are far apart

// Bad: High-cardinality without encoding
let user_feature = user_id.hash() as f64;  // ❌ No structure

// Bad: Mixed scales
let features = vec![
    input_length as f64,  // 0-10000
    priority as f64,      // 1-10
    ...
];  // ❌ Different scales
```

---

## Algorithm Selection

### Decision Matrix

| Scenario | Best Algorithm | Why |
|----------|---------------|-----|
| 2-10 discrete options, no context | Thompson Sampling | Fast, proven, no tuning |
| Context-aware, linear rewards | LinUCB | Theoretical guarantees, efficient |
| Context-aware, non-linear rewards | Contextual Thompson | Handles non-linearity |
| Continuous parameter tuning | Parameter Optimizer | Systematic exploration |
| Multiple objectives | Pareto Optimization | Shows all trade-offs |

### When to Upgrade

Start simple, upgrade when needed:

1. **Start:** Thompson Sampling (no context)
2. **Add context:** LinUCB (user/task differences matter)
3. **Add complexity:** Contextual Thompson (non-linear relationships)
4. **Optimize parameters:** Parameter Optimizer (tune temperature/top-p)
5. **Multi-objective:** Pareto (balance cost/quality/latency)

---

## Hyperparameter Tuning

### Thompson Sampling

**No tuning needed!** Thompson Sampling is parameter-free.

### LinUCB Alpha

**Alpha** controls exploration vs exploitation.

```rust
// Conservative (more exploitation)
let bandit = LinUCB::new(0.3, dimension);

// Balanced (default)
let bandit = LinUCB::new(1.0, dimension);

// Aggressive (more exploration)
let bandit = LinUCB::new(2.0, dimension);
```

**Tuning guide:**
- Start with 1.0
- Increase if not exploring enough (high regret)
- Decrease if exploring too much (poor performance)
- Typical range: 0.3 to 2.0

### Parameter Optimizer Exploration Rate

```rust
let policy = OptimizationPolicy::new(
    "my_policy",
    range,
    OptimizationMode::Balanced,
).with_exploration_rate(0.2);  // 20% exploration
```

**Guidelines:**
- 0.1 (10%): Conservative, mostly exploit
- 0.2 (20%): Balanced (default)
- 0.3 (30%): Aggressive, more exploration

---

## Reward Design Patterns

### Pattern 1: Weighted Multi-Component

```rust
let reward =
    0.6 * quality_score +
    0.3 * (1.0 - cost / max_cost) +
    0.1 * (1.0 - latency / max_latency);
```

**When:** Standard case with multiple objectives

### Pattern 2: Threshold-Based

```rust
let reward = if quality_score >= min_quality {
    // Only optimize cost if quality threshold met
    1.0 - (cost / max_cost)
} else {
    // Penalize heavily if below quality threshold
    quality_score * 0.5
};
```

**When:** Hard constraints on quality

### Pattern 3: Time-Varying

```rust
let is_peak = hour >= 9 && hour <= 17;
let cost_weight = if is_peak { 0.5 } else { 0.2 };

let reward =
    (1.0 - cost_weight) * quality_score +
    cost_weight * (1.0 - cost / max_cost);
```

**When:** Different objectives at different times

### Pattern 4: User Satisfaction Focus

```rust
let implicit_quality = feedback.infer_quality();
let reward =
    0.4 * measured_quality +
    0.6 * implicit_quality;  // User behavior matters more
```

**When:** User experience is critical

### Best Practices

1. **Normalize components** to [0, 1] before combining
2. **Test sensitivity** to weight changes
3. **Monitor alignment** with business goals
4. **Log everything** for offline analysis
5. **Version control** reward functions

---

## Exploration Strategies

### Cold Start: New Arms

**Problem:** New arms have no data, may never be tried.

**Solution 1: Optimistic Initialization**

```rust
// LinUCB automatically handles this via uncertainty
let bandit = LinUCB::new(2.0, dimension);  // High alpha = optimistic
bandit.add_arm(new_model);
// New arm has high uncertainty → explored early
```

**Solution 2: Forced Exploration**

```rust
const WARMUP_SAMPLES: u64 = 10;

fn select_with_warmup(bandit: &LinUCB, context: &RequestContext) -> Uuid {
    // Force exploration of under-sampled arms
    for (id, arm) in bandit.get_arms() {
        if arm.num_selections < WARMUP_SAMPLES {
            return *id;
        }
    }

    // All warmed up, use normal selection
    bandit.select_arm(context).unwrap()
}
```

**Solution 3: Transfer Learning**

```rust
// Initialize new arm from similar arm
fn warm_start_from_similar(
    new_arm: &mut LinUCBArm,
    similar_arm: &LinUCBArm,
    confidence: f64,  // 0.0-1.0
) {
    new_arm.a_matrix = similar_arm.a_matrix.clone();
    new_arm.b_vector = similar_arm.b_vector.iter()
        .map(|&v| v * confidence)
        .collect();
}
```

### Exploration-Exploitation Balance

**Too much exploration:**
- High regret
- Poor user experience
- Wasted budget

**Too little exploration:**
- Miss better options
- Stagnate at local optimum
- Can't adapt to changes

**Indicators of good balance:**
- Regret rate < 5%
- Best arm receives 60-80% traffic
- Other arms still tried occasionally

---

## Safety Mechanisms

### 1. Minimum Traffic Allocation

Never completely abandon an arm:

```rust
const MIN_TRAFFIC: f64 = 0.05;  // 5% minimum

fn safe_selection(
    bandit: &LinUCB,
    context: &RequestContext
) -> Uuid {
    // Check if any arm is below minimum
    let total = bandit.total_samples();
    for (id, arm) in bandit.get_arms() {
        let traffic_share = arm.num_selections as f64 / total as f64;
        if traffic_share < MIN_TRAFFIC {
            return *id;  // Force selection
        }
    }

    bandit.select_arm(context).unwrap()
}
```

### 2. Quality Floor

Don't use models below quality threshold:

```rust
const MIN_QUALITY: f64 = 0.7;

fn quality_safe_selection(
    bandit: &LinUCB,
    context: &RequestContext,
    quality_estimates: &HashMap<Uuid, f64>
) -> Result<Uuid> {
    let selected = bandit.select_arm(context)?;

    if quality_estimates[&selected] < MIN_QUALITY {
        // Fall back to safe default
        return Ok(fallback_model_id);
    }

    Ok(selected)
}
```

### 3. Cost Ceiling

Prevent overspending:

```rust
const MAX_COST: f64 = 0.05;  // $0.05 per request

fn cost_safe_selection(
    bandit: &LinUCB,
    context: &RequestContext,
    cost_estimates: &HashMap<Uuid, f64>
) -> Result<Uuid> {
    let selected = bandit.select_arm(context)?;

    if cost_estimates[&selected] > MAX_COST {
        // Select cheapest acceptable arm
        return Ok(cheap_fallback_id);
    }

    Ok(selected)
}
```

### 4. Circuit Breaker

Stop using failing models:

```rust
struct CircuitBreaker {
    failure_counts: HashMap<Uuid, usize>,
    threshold: usize,
    blocked: HashSet<Uuid>,
}

impl CircuitBreaker {
    fn record_failure(&mut self, model_id: &Uuid) {
        *self.failure_counts.entry(*model_id).or_insert(0) += 1;

        if self.failure_counts[model_id] >= self.threshold {
            self.blocked.insert(*model_id);
            eprintln!("Circuit breaker: blocking {}", model_id);
        }
    }

    fn is_blocked(&self, model_id: &Uuid) -> bool {
        self.blocked.contains(model_id)
    }

    fn reset(&mut self, model_id: &Uuid) {
        self.failure_counts.remove(model_id);
        self.blocked.remove(model_id);
    }
}
```

### 5. Gradual Rollout

Increase traffic gradually:

```rust
struct GradualRollout {
    ml_system: LinUCB,
    fallback: Box<dyn Strategy>,
    rollout_percentage: f64,
}

impl GradualRollout {
    fn select(&self, context: &RequestContext) -> Uuid {
        if rand::random::<f64>() < self.rollout_percentage {
            self.ml_system.select_arm(context).unwrap_or_else(|_| {
                self.fallback.select()
            })
        } else {
            self.fallback.select()
        }
    }

    fn increase_rollout(&mut self, step: f64) {
        self.rollout_percentage = (self.rollout_percentage + step).min(1.0);
    }
}

// Rollout schedule:
// Day 1:  5%
// Day 3: 10%
// Day 7: 25%
// Day 14: 50%
// Day 21: 100%
```

---

## Performance Optimization

### 1. Batching

Process multiple requests together:

```rust
fn batch_select(
    bandit: &LinUCB,
    contexts: &[RequestContext]
) -> Vec<Uuid> {
    contexts.iter()
        .map(|ctx| bandit.select_arm(ctx).unwrap())
        .collect()
}
```

### 2. Caching

Cache feature vectors:

```rust
use lru::LruCache;

struct CachedFeatureExtractor {
    cache: LruCache<String, Vec<f64>>,
}

impl CachedFeatureExtractor {
    fn get_features(&mut self, context: &RequestContext) -> Vec<f64> {
        let key = format!("{:?}", context);

        if let Some(features) = self.cache.get(&key) {
            return features.clone();
        }

        let features = context.to_feature_vector();
        self.cache.put(key, features.clone());
        features
    }
}
```

### 3. Async Processing

Don't block on updates:

```rust
use tokio::sync::mpsc;

struct AsyncUpdater {
    update_tx: mpsc::Sender<Update>,
}

struct Update {
    arm_id: Uuid,
    context: RequestContext,
    reward: f64,
}

async fn update_worker(
    mut update_rx: mpsc::Receiver<Update>,
    bandit: Arc<Mutex<LinUCB>>
) {
    while let Some(update) = update_rx.recv().await {
        let mut bandit = bandit.lock().await;
        bandit.update(&update.arm_id, &update.context, update.reward).ok();
    }
}
```

### 4. Periodic Persistence

Save state periodically:

```rust
use tokio::time::{interval, Duration};

async fn periodic_save(
    bandit: Arc<Mutex<LinUCB>>,
    path: &str
) {
    let mut ticker = interval(Duration::from_secs(300));  // Every 5 minutes

    loop {
        ticker.tick().await;

        let bandit = bandit.lock().await;
        let serialized = serde_json::to_string(&*bandit).unwrap();
        tokio::fs::write(path, serialized).await.ok();
    }
}
```

---

## Debugging Techniques

### 1. Logging Selection Decisions

```rust
struct LoggingBandit {
    inner: LinUCB,
}

impl LoggingBandit {
    fn select(&self, context: &RequestContext) -> Result<Uuid> {
        let selected = self.inner.select_arm(context)?;

        // Log decision
        log::info!(
            "Selected arm {} for context task_type={:?}, input_length={}",
            selected,
            context.task_type,
            context.input_length
        );

        // Log UCB scores for all arms
        for (id, arm) in self.inner.get_arms() {
            let features = context.to_feature_vector();
            let ucb = arm.calculate_ucb(&features, self.inner.alpha()).unwrap();
            log::debug!("Arm {} UCB score: {:.3}", id, ucb);
        }

        Ok(selected)
    }
}
```

### 2. Reward Distribution Analysis

```rust
use statrs::statistics::Statistics;

fn analyze_rewards(
    bandit: &LinUCB
) -> HashMap<Uuid, RewardStats> {
    let mut stats = HashMap::new();

    for (id, arm) in bandit.get_arms() {
        let avg = arm.average_reward();
        let count = arm.num_selections;

        stats.insert(*id, RewardStats {
            mean: avg,
            count,
            std_dev: estimate_std_dev(arm),
        });
    }

    stats
}

struct RewardStats {
    mean: f64,
    count: u64,
    std_dev: f64,
}
```

### 3. Regret Tracking

```rust
fn track_regret(bandit: &ThompsonSampling) -> RegretMetrics {
    let regret = bandit.calculate_regret();
    let total_samples = bandit.total_samples();
    let regret_rate = regret / total_samples as f64;

    RegretMetrics {
        total_regret: regret,
        regret_rate,
        samples: total_samples,
    }
}

struct RegretMetrics {
    total_regret: f64,
    regret_rate: f64,
    samples: u64,
}
```

---

## Common Pitfalls

### 1. Not Enough Exploration

**Symptom:** Model converges quickly but to suboptimal choice

**Fix:** Increase exploration parameter

```rust
// Before (too greedy)
let bandit = LinUCB::new(0.1, dimension);

// After (more exploration)
let bandit = LinUCB::new(1.5, dimension);
```

### 2. Incorrect Reward Scale

**Symptom:** Model doesn't learn or learns slowly

**Fix:** Ensure rewards are in reasonable range [0, 1]

```rust
// Bad: Unbounded rewards
let reward = quality - cost;  // Can be negative or huge

// Good: Normalized rewards
let reward = 0.6 * quality + 0.4 * (1.0 - cost / max_cost).max(0.0);
```

### 3. Context Mismatch

**Symptom:** LinUCB performance worse than Thompson Sampling

**Fix:** Verify context features match prediction task

```rust
// Bad: Irrelevant features
let context = RequestContext::new(100)
    .with_metadata("user_favorite_color", "blue");  // ❌ Irrelevant

// Good: Relevant features
let context = RequestContext::new(100)
    .with_task_type("code_generation")  // ✓ Relevant
    .with_priority(8);
```

### 4. Delayed Feedback

**Symptom:** System doesn't adapt to changes

**Fix:** Update as soon as feedback available

```rust
// Bad: Batched updates once per day
collect_feedback_all_day();
update_at_midnight();  // ❌ Too late

// Good: Immediate updates
let reward = get_immediate_feedback();
bandit.update(&arm_id, &context, reward)?;  // ✓ Real-time
```

### 5. Ignoring Drift

**Symptom:** Performance degrades over time

**Fix:** Implement drift detection

```rust
let mut detector = ADWIN::new(0.002, 1000)?;

loop {
    let metric = get_current_metric();
    let status = detector.add(metric);

    if status == DriftStatus::Drift {
        // Retrain or reset
        bandit.set_alpha(bandit.alpha() * 1.5);  // Increase exploration
        log::warn!("Drift detected, increasing exploration");
    }
}
```

---

## Production Checklist

### Before Launch

- [ ] Test offline with historical data
- [ ] Validate reward function aligns with business goals
- [ ] Implement safety mechanisms (quality floor, cost ceiling)
- [ ] Set up monitoring and alerting
- [ ] Configure drift detection
- [ ] Plan gradual rollout strategy
- [ ] Document decision logic and parameters
- [ ] Test fallback behavior

### During Launch

- [ ] Start with 5% traffic
- [ ] Monitor key metrics (reward, regret, cost)
- [ ] Check for anomalies
- [ ] Verify logging is working
- [ ] Compare with baseline
- [ ] Gradually increase traffic

### Post-Launch

- [ ] Weekly performance review
- [ ] Monthly reward function review
- [ ] Quarterly algorithm assessment
- [ ] Track long-term trends
- [ ] Document learnings
- [ ] Update parameters as needed

### Monitoring Checklist

- [ ] Average reward trending up
- [ ] Regret rate < 5%
- [ ] No anomalies in cost
- [ ] Drift detector stable
- [ ] Best arm receives 60-80% traffic
- [ ] All arms tried occasionally
- [ ] Quality above minimum threshold
- [ ] Latency within SLA

---

## Summary

Key principles for production ML systems:

1. **Start simple** - Thompson Sampling before LinUCB
2. **Safety first** - Guardrails, fallbacks, circuit breakers
3. **Monitor everything** - Rewards, regret, metrics
4. **Adapt continuously** - Drift detection, retraining
5. **Test thoroughly** - Offline eval before production
6. **Document decisions** - Why you chose parameters
7. **Gradual rollout** - Start small, increase slowly
8. **Version control** - Track changes to algorithms and rewards

For more details:
- [User Guide](ML_USER_GUIDE.md)
- [API Reference](ML_API_REFERENCE.md)
- [Troubleshooting](ML_TROUBLESHOOTING.md)
