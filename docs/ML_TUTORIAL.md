# ML Integration Tutorial

Step-by-step tutorials for mastering machine learning in the LLM Auto-Optimizer.

## Table of Contents

1. [Lesson 1: Getting Started with Multi-Armed Bandits](#lesson-1-getting-started-with-multi-armed-bandits)
2. [Lesson 2: Model Selection with Thompson Sampling](#lesson-2-model-selection-with-thompson-sampling)
3. [Lesson 3: Contextual Bandits for Personalization](#lesson-3-contextual-bandits-for-personalization)
4. [Lesson 4: Parameter Tuning with Bayesian Optimization](#lesson-4-parameter-tuning-with-bayesian-optimization)
5. [Lesson 5: Feature Engineering for LLM Tasks](#lesson-5-feature-engineering-for-llm-tasks)
6. [Lesson 6: Setting Up A/B Tests](#lesson-6-setting-up-ab-tests)
7. [Lesson 7: Monitoring Model Performance](#lesson-7-monitoring-model-performance)
8. [Lesson 8: Handling Cold Start Problems](#lesson-8-handling-cold-start-problems)
9. [Lesson 9: Advanced - Custom Reward Functions](#lesson-9-advanced-custom-reward-functions)
10. [Lesson 10: Advanced - Multi-Objective Optimization](#lesson-10-advanced-multi-objective-optimization)
11. [Lesson 11: Production Deployment Strategies](#lesson-11-production-deployment-strategies)
12. [Lesson 12: Drift Detection and Adaptation](#lesson-12-drift-detection-and-adaptation)

---

## Lesson 1: Getting Started with Multi-Armed Bandits

**Goal**: Learn the basics of multi-armed bandits for LLM model selection.

**Duration**: 30 minutes

**Prerequisites**: Basic Rust knowledge

### What You'll Build

A simple system that chooses between three LLM models and learns which performs best.

### Step 1: Setup Dependencies

Add to your `Cargo.toml`:

```toml
[dependencies]
decision = { path = "../../crates/decision" }
uuid = "1.0"
```

### Step 2: Create Thompson Sampling Instance

```rust
use decision::ThompsonSampling;
use uuid::Uuid;

fn main() {
    // Create bandit
    let mut bandit = ThompsonSampling::new();

    // Define three models to choose from
    let gpt5 = Uuid::new_v4();
    let claude4 = Uuid::new_v4();
    let gemini25 = Uuid::new_v4();

    // Add as arms
    bandit.add_variant(gpt5);
    bandit.add_variant(claude4);
    bandit.add_variant(gemini25);

    println!("Initialized bandit with 3 models");
}
```

### Step 3: Simulate Selection and Feedback

```rust
// Simulate 100 requests
for i in 0..100 {
    // Select model
    let model = bandit.select_variant().unwrap();

    // Simulate LLM call with different success rates
    // In reality, you'd call actual LLM and measure quality
    let success = simulate_llm_call(model, gpt5, claude4, gemini25);

    // Update bandit with outcome
    bandit.update(&model, success).unwrap();

    // Print progress every 20 requests
    if (i + 1) % 20 == 0 {
        println!("\nAfter {} requests:", i + 1);
        print_stats(&bandit, gpt5, claude4, gemini25);
    }
}

fn simulate_llm_call(model: Uuid, gpt5: Uuid, claude4: Uuid, gemini25: Uuid) -> bool {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    // Simulate different success rates
    let success_rate = if model == gpt5 {
        0.85  // GPT-5 is best
    } else if model == claude4 {
        0.75  // Claude-4 is good
    } else {
        0.65  // Gemini-2.5 is okay
    };

    rng.gen::<f64>() < success_rate
}

fn print_stats(bandit: &ThompsonSampling, gpt5: Uuid, claude4: Uuid, gemini25: Uuid) {
    let rates = bandit.get_conversion_rates();

    println!("  GPT-5:       {:.3}", rates[&gpt5]);
    println!("  Claude-4:    {:.3}", rates[&claude4]);
    println!("  Gemini-2.5:  {:.3}", rates[&gemini25]);

    let regret = bandit.calculate_regret();
    println!("  Regret:      {:.2}", regret);
}
```

### Expected Output

```
Initialized bandit with 3 models

After 20 requests:
  GPT-5:       0.556
  Claude-4:    0.545
  Gemini-2.5:  0.538
  Regret:      1.23

After 40 requests:
  GPT-5:       0.615
  Claude-4:    0.591
  Gemini-2.5:  0.571
  Regret:      3.45

After 100 requests:
  GPT-5:       0.815  ← Converged to true rate
  Claude-4:    0.724
  Gemini-2.5:  0.658
  Regret:      5.67
```

### Key Takeaways

1. **Thompson Sampling is simple**: Just 3 lines to initialize and use
2. **Automatic exploration**: No tuning needed, algorithm explores naturally
3. **Fast convergence**: Identifies best model within 50-100 trials
4. **Low regret**: Quickly shifts traffic to best model

### Exercise

Modify the code to:
1. Add a fourth model with 0.90 success rate
2. Track how many times each model is selected
3. Plot the selection distribution over time

---

## Lesson 2: Model Selection with Thompson Sampling

**Goal**: Build a production-ready model selection system with real LLM providers.

**Duration**: 45 minutes

**Prerequisites**: Lesson 1, LLM API keys

### Step 1: Define Model Configs

```rust
use decision::ThompsonSampling;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelConfig {
    name: String,
    provider: String,
    api_key: String,
    endpoint: String,
}

#[derive(Debug, Clone)]
struct ModelSelector {
    bandit: ThompsonSampling,
    configs: HashMap<Uuid, ModelConfig>,
}

impl ModelSelector {
    fn new() -> Self {
        Self {
            bandit: ThompsonSampling::new(),
            configs: HashMap::new(),
        }
    }

    fn add_model(&mut self, config: ModelConfig) -> Uuid {
        let id = Uuid::new_v4();
        self.bandit.add_variant(id);
        self.configs.insert(id, config);
        id
    }

    fn select(&self) -> Result<(Uuid, &ModelConfig), String> {
        let id = self.bandit.select_variant()
            .map_err(|e| format!("Selection failed: {}", e))?;
        let config = self.configs.get(&id)
            .ok_or_else(|| "Config not found".to_string())?;
        Ok((id, config))
    }

    fn update(&mut self, model_id: &Uuid, success: bool) -> Result<(), String> {
        self.bandit.update(model_id, success)
            .map_err(|e| format!("Update failed: {}", e))
    }
}
```

### Step 2: Implement LLM Calling

```rust
use reqwest;
use serde_json::json;

async fn call_llm(config: &ModelConfig, prompt: &str) -> Result<LLMResponse, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();

    let payload = json!({
        "model": config.name,
        "messages": [{
            "role": "user",
            "content": prompt
        }],
        "temperature": 0.7,
        "max_tokens": 1000,
    });

    let response = client
        .post(&config.endpoint)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .json(&payload)
        .send()
        .await?
        .json::<LLMResponse>()
        .await?;

    Ok(response)
}

#[derive(Debug, Deserialize)]
struct LLMResponse {
    choices: Vec<Choice>,
    usage: Usage,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Debug, Deserialize)]
struct Message {
    content: String,
}

#[derive(Debug, Deserialize)]
struct Usage {
    total_tokens: usize,
}
```

### Step 3: Define Success Criteria

```rust
fn evaluate_response(response: &LLMResponse, expected: &str) -> bool {
    // In production, use more sophisticated evaluation
    let content = &response.choices[0].message.content;

    // Example: Check if response contains expected keywords
    let quality_check = content.len() > 50 && content.contains("important");

    // Check token efficiency
    let token_check = response.usage.total_tokens < 2000;

    quality_check && token_check
}
```

### Step 4: Main Selection Loop

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut selector = ModelSelector::new();

    // Add models
    selector.add_model(ModelConfig {
        name: "gpt-5".to_string(),
        provider: "openai".to_string(),
        api_key: std::env::var("OPENAI_API_KEY")?,
        endpoint: "https://api.openai.com/v1/chat/completions".to_string(),
    });

    selector.add_model(ModelConfig {
        name: "claude-sonnet-4.5".to_string(),
        provider: "anthropic".to_string(),
        api_key: std::env::var("ANTHROPIC_API_KEY")?,
        endpoint: "https://api.anthropic.com/v1/messages".to_string(),
    });

    // Process requests
    loop {
        // Get request from queue
        let prompt = get_next_request().await?;

        // Select model
        let (model_id, config) = selector.select()?;
        println!("Selected: {}", config.name);

        // Call LLM
        let response = call_llm(config, &prompt).await?;

        // Evaluate
        let success = evaluate_response(&response, &prompt);

        // Update bandit
        selector.update(&model_id, success)?;

        // Send response to user
        send_response(response).await?;
    }
}
```

### Key Takeaways

1. **Real-world integration**: Connect Thompson Sampling to actual LLM APIs
2. **Success criteria**: Define what "success" means for your use case
3. **Async handling**: Use tokio for concurrent requests
4. **Error handling**: Robust error handling for production

---

## Lesson 3: Contextual Bandits for Personalization

**Goal**: Use request context to personalize model selection.

**Duration**: 60 minutes

**Prerequisites**: Lesson 2

### Step 1: Understand Request Context

```rust
use decision::{LinUCB, RequestContext, OutputLengthCategory};

fn main() {
    // Create contextual bandit
    let mut bandit = LinUCB::new(
        1.0,  // alpha (exploration parameter)
        RequestContext::feature_dimension()  // 10 features
    );

    // Add models
    let gpt5 = Uuid::new_v4();
    let claude4 = Uuid::new_v4();

    bandit.add_arm(gpt5);
    bandit.add_arm(claude4);

    println!("Created contextual bandit with LinUCB");
}
```

### Step 2: Create Rich Contexts

```rust
// Example 1: Code generation request
let code_context = RequestContext::new(200)  // 200 token input
    .with_task_type("code_generation")
    .with_language("python")
    .with_priority(8)  // high priority
    .with_output_length(OutputLengthCategory::Long);

// Example 2: Simple Q&A
let qa_context = RequestContext::new(50)
    .with_task_type("question_answering")
    .with_priority(5)  // normal priority
    .with_output_length(OutputLengthCategory::Short);

// Example 3: Creative writing
let creative_context = RequestContext::new(100)
    .with_task_type("generation")
    .with_priority(6)
    .with_output_length(OutputLengthCategory::Medium)
    .with_metadata("style", "creative");
```

### Step 3: Context-Aware Selection

```rust
// Select based on context
let model_for_code = bandit.select_arm(&code_context)?;
let model_for_qa = bandit.select_arm(&qa_context)?;

println!("For code:  {:?}", model_for_code);
println!("For Q&A:   {:?}", model_for_qa);

// Models can be different based on context!
```

### Step 4: Learning from Feedback

```rust
// Simulate different models being better at different tasks
for i in 0..200 {
    let is_code_task = i % 2 == 0;

    let context = if is_code_task {
        RequestContext::new(200).with_task_type("code_generation")
    } else {
        RequestContext::new(50).with_task_type("question_answering")
    };

    // Select model
    let model = bandit.select_arm(&context)?;

    // Simulate: GPT-5 is better at code, Claude-4 at Q&A
    let reward = if is_code_task {
        if model == gpt5 { 0.9 } else { 0.7 }  // GPT-5 better
    } else {
        if model == claude4 { 0.9 } else { 0.7 }  // Claude-4 better
    };

    // Update
    bandit.update(&model, &context, reward)?;
}

// Check learned preferences
println!("\nLearned task-specific preferences:");

let code_ctx = RequestContext::new(200).with_task_type("code_generation");
let code_model = bandit.select_arm(&code_ctx)?;
println!("Code generation: {:?}", if code_model == gpt5 { "GPT-5" } else { "Claude-4" });

let qa_ctx = RequestContext::new(50).with_task_type("question_answering");
let qa_model = bandit.select_arm(&qa_ctx)?;
println!("Q&A:            {:?}", if qa_model == claude4 { "Claude-4" } else { "GPT-5" });
```

### Expected Output

```
Created contextual bandit with LinUCB

Learned task-specific preferences:
Code generation: GPT-5     ← Learned preference
Q&A:            Claude-4   ← Learned preference
```

### Step 5: Time-Based Routing

```rust
// Route to cheaper models during peak hours
let context_peak = RequestContext::new(100);  // hour_of_day auto-set to current
let context_offpeak = RequestContext::new(100);

// During training, give higher rewards to cheap models at peak
let hour = chrono::Utc::now().hour();
let is_peak = hour >= 9 && hour <= 17;

let cost_factor = if is_peak { 1.5 } else { 1.0 };
let quality_reward = 0.8;
let cost_penalty = 0.2 * cost_factor;

let total_reward = quality_reward - cost_penalty;
```

### Key Takeaways

1. **Personalization**: Different contexts get different models
2. **Feature engineering**: RequestContext provides 10 engineered features
3. **Learning**: LinUCB learns task-specific preferences automatically
4. **Flexibility**: Add custom metadata for domain-specific features

### Exercise

1. Add user_id to context and track per-user preferences
2. Implement language-specific routing (English→GPT, French→Claude)
3. Add cost-awareness to peak-hour routing

---

## Lesson 4: Parameter Tuning with Bayesian Optimization

**Goal**: Automatically tune temperature, top-p, and max_tokens for optimal quality.

**Duration**: 60 minutes

### Step 1: Setup Parameter Optimizer

```rust
use decision::{
    ParameterOptimizer, OptimizationPolicy, OptimizationMode,
    ParameterRange, RewardWeights, RequestContext,
    ResponseMetrics, SearchStrategy,
};

fn main() {
    // Create optimizer with quality-focused rewards
    let weights = RewardWeights::quality_focused();
    let optimizer = ParameterOptimizer::new(weights, 1.0);

    // Define parameter ranges
    let range = ParameterRange {
        temp_min: 0.3,
        temp_max: 1.2,
        top_p_min: 0.7,
        top_p_max: 0.99,
        max_tokens_min: 256,
        max_tokens_max: 2048,
    };

    // Create optimization policy
    let policy = OptimizationPolicy::new(
        "quality_optimization",
        range,
        OptimizationMode::Explore,  // Start with exploration
    );

    optimizer.create_policy(policy).unwrap();

    println!("Created parameter optimizer");
}
```

### Step 2: Initialize with Search

```rust
// Use Latin Hypercube Sampling for good coverage
let config_ids = optimizer.initialize_with_search(
    "quality_optimization",
    SearchStrategy::LatinHypercube,
    20,  // 20 well-distributed configurations
).unwrap();

println!("Initialized with {} configurations", config_ids.len());
```

### Step 3: Selection and Feedback Loop

```rust
// Request loop
for request_num in 0..1000 {
    // Get context
    let context = RequestContext::new(150)
        .with_task_type("generation");

    // Select parameters
    let (config_id, params) = optimizer.select_parameters(
        "quality_optimization",
        &context,
    ).unwrap();

    // Call LLM with these parameters
    let response = call_llm_with_params(
        "gpt-5",
        &prompt,
        params.temperature,
        params.top_p,
        params.max_tokens,
    ).await.unwrap();

    // Measure performance
    let metrics = ResponseMetrics {
        quality_score: evaluate_quality(&response),
        cost: calculate_cost(&response),
        latency_ms: response.latency_ms,
        token_count: response.token_count,
    };

    // Update optimizer
    optimizer.update_performance(
        "quality_optimization",
        &config_id,
        &context,
        &metrics,
        None,  // No explicit user feedback
    ).unwrap();

    // Print progress
    if request_num % 100 == 0 {
        print_best_params(&optimizer, "quality_optimization");
    }
}

fn print_best_params(optimizer: &ParameterOptimizer, policy: &str) {
    let stats = optimizer.get_performance_stats(policy).unwrap();

    let best = stats.iter()
        .max_by(|a, b| a.average_reward.partial_cmp(&b.average_reward).unwrap())
        .unwrap();

    println!("\nBest parameters (reward: {:.3}):", best.average_reward);
    println!("  temperature: {:.2}", best.config.temperature);
    println!("  top_p:       {:.2}", best.config.top_p);
    println!("  max_tokens:  {}", best.config.max_tokens);
}
```

### Expected Output

```
Created parameter optimizer
Initialized with 20 configurations

Best parameters (reward: 0.712):
  temperature: 0.75
  top_p:       0.92
  max_tokens:  512

Best parameters (reward: 0.801):
  temperature: 0.68
  top_p:       0.95
  max_tokens:  768

Best parameters (reward: 0.845):  ← Converged
  temperature: 0.70
  top_p:       0.93
  max_tokens:  800
```

### Step 4: Task-Specific Tuning

```rust
// Different tasks may need different parameters
optimizer.update_task_bests(
    "quality_optimization",
    &[
        "code_generation".to_string(),
        "question_answering".to_string(),
        "creative_writing".to_string(),
    ],
).unwrap();

// Get task-specific best
let code_params = optimizer.get_best_for_task(
    "quality_optimization",
    "code_generation",
).unwrap();

if let Some((config_id, config)) = code_params {
    println!("Best for code: temp={:.2}", config.temperature);
}
```

### Key Takeaways

1. **Automated tuning**: No manual hyperparameter search
2. **Continuous optimization**: Adapts as data changes
3. **Task-specific**: Different parameters for different tasks
4. **Multi-objective**: Balances quality, cost, and latency

---

## Lesson 5: Feature Engineering for LLM Tasks

**Goal**: Design effective features for contextual bandits.

**Duration**: 45 minutes

### Understanding Feature Vectors

```rust
let context = RequestContext::new(256)
    .with_task_type("code_generation")
    .with_output_length(OutputLengthCategory::Long)
    .with_priority(8);

let features = context.to_feature_vector();

println!("Feature vector (10 dimensions):");
for (i, f) in features.iter().enumerate() {
    println!("  [{}]: {:.4}", i, f);
}
```

Output:
```
Feature vector (10 dimensions):
  [0]: 0.554   ← log(input_length)/10
  [1]: 1.000   ← output category (Long)
  [2]: 0.800   ← priority (8/10)
  [3]: 0.500   ← hour cos
  [4]: 0.866   ← hour sin
  [5]: -0.901  ← day cos
  [6]: 0.434   ← day sin
  [7]: 0.330   ← task type encoding
  [8]: 1.000   ← bias term
  [9]: 0.000   ← reserved
```

### Adding Custom Features

```rust
// Extend with domain-specific metadata
let context = RequestContext::new(256)
    .with_task_type("code_generation")
    .with_metadata("language", "python")
    .with_metadata("complexity", "high")
    .with_metadata("framework", "pytorch");

// Access in custom feature engineering
fn extract_custom_features(context: &RequestContext) -> Vec<f64> {
    let mut features = context.to_feature_vector();

    // Add language feature
    let lang_feature = match context.metadata.get("language") {
        Some(lang) if lang == "python" => 1.0,
        Some(lang) if lang == "javascript" => 0.5,
        _ => 0.0,
    };
    features.push(lang_feature);

    // Add complexity feature
    let complexity = match context.metadata.get("complexity") {
        Some(c) if c == "high" => 1.0,
        Some(c) if c == "medium" => 0.5,
        _ => 0.0,
    };
    features.push(complexity);

    features
}
```

### Key Takeaways

1. **Built-in features**: 10 carefully engineered features
2. **Cyclical encoding**: Proper handling of time
3. **Normalization**: All features scaled to [0,1] or [-1,1]
4. **Extensibility**: Add custom features via metadata

---

## Lesson 6: Setting Up A/B Tests

**Goal**: Run statistically rigorous A/B tests for model validation.

**Duration**: 45 minutes

### Complete A/B Test Example

See [ML_EXAMPLES.md](ML_EXAMPLES.md#example-5-ab-testing-new-model-version) for full code.

Key steps:
1. Define variants (control vs treatment)
2. Calculate required sample size
3. Run experiment with random assignment
4. Monitor significance continuously
5. Declare winner when significant

---

## Lesson 7: Monitoring Model Performance

**Goal**: Set up comprehensive monitoring and alerting.

**Duration**: 45 minutes

### Metrics Dashboard

```rust
use decision::{ThresholdMonitoringSystem, ThresholdConfig, AlertType, Comparison};
use std::collections::HashMap;

struct MLMonitor {
    threshold_system: ThresholdMonitoringSystem,
    drift_detector: ADWIN,
    anomaly_detector: ZScoreDetector,
}

impl MLMonitor {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut system = ThresholdMonitoringSystem::new();

        // Define thresholds
        system.add_threshold(ThresholdConfig {
            metric_name: "average_reward".to_string(),
            threshold: 0.7,
            comparison: Comparison::GreaterThan,
            alert_type: AlertType::Warning,
            window_size: 100,
        });

        system.add_threshold(ThresholdConfig {
            metric_name: "cost_per_request".to_string(),
            threshold: 0.05,
            comparison: Comparison::LessThan,
            alert_type: AlertType::Critical,
            window_size: 50,
        });

        Ok(Self {
            threshold_system: system,
            drift_detector: ADWIN::new(0.002, 1000)?,
            anomaly_detector: ZScoreDetector::new(100, 3.0)?,
        })
    }

    fn check_metrics(&mut self, metrics: &HashMap<String, f64>) -> Vec<Alert> {
        let mut alerts = Vec::new();

        // Threshold checks
        alerts.extend(self.threshold_system.check_all(metrics).unwrap());

        // Drift detection
        if let Some(&reward) = metrics.get("average_reward") {
            let status = self.drift_detector.add(reward);
            if status == DriftStatus::Drift {
                alerts.push(create_drift_alert());
            }
        }

        // Anomaly detection
        if let Some(&cost) = metrics.get("cost_per_request") {
            let result = self.anomaly_detector.add(cost);
            if result.is_anomaly {
                alerts.push(create_anomaly_alert(result));
            }
        }

        alerts
    }
}
```

---

## Lesson 8: Handling Cold Start Problems

**Goal**: Effectively handle new arms with no data.

**Duration**: 30 minutes

### Strategy 1: Optimistic Initialization

```rust
// LinUCB naturally handles cold start through uncertainty
let mut bandit = LinUCB::new(2.0, 10);  // High alpha for more exploration

// New arm has high uncertainty → explored early
bandit.add_arm(new_model_id);
```

### Strategy 2: Forced Exploration

```rust
const MIN_SAMPLES: u64 = 10;

fn select_with_warmup(bandit: &LinUCB, context: &RequestContext) -> Uuid {
    // Check if any arm needs warmup
    for (arm_id, arm) in bandit.get_arms() {
        if arm.num_selections < MIN_SAMPLES {
            return *arm_id;  // Force selection
        }
    }

    // All warmed up, use normal selection
    bandit.select_arm(context).unwrap()
}
```

### Strategy 3: Transfer Learning

```rust
// Use similar model's statistics as prior
fn initialize_from_similar(
    new_arm: &mut LinUCBArm,
    similar_arm: &LinUCBArm,
    transfer_rate: f64,
) {
    // Transfer knowledge with decay
    new_arm.a_matrix = similar_arm.a_matrix.clone();
    new_arm.b_vector = similar_arm.b_vector.iter()
        .map(|&v| v * transfer_rate)
        .collect();
}
```

---

## Lesson 9: Advanced - Custom Reward Functions

**Goal**: Design reward functions for complex objectives.

**Duration**: 60 minutes

### Multi-Component Reward

```rust
use decision::{ResponseMetrics, UserFeedback, RewardCalculator};

struct CustomRewardCalculator {
    quality_weight: f64,
    cost_weight: f64,
    latency_weight: f64,
    user_satisfaction_weight: f64,
}

impl CustomRewardCalculator {
    fn calculate(&self, metrics: &ResponseMetrics, feedback: &UserFeedback) -> f64 {
        // Quality component [0, 1]
        let quality = metrics.quality_score;

        // Cost component (inverted, normalized)
        let max_acceptable_cost = 0.10;
        let cost = (1.0 - (metrics.cost / max_acceptable_cost).min(1.0)).max(0.0);

        // Latency component (inverted, normalized)
        let max_acceptable_latency = 5000.0;
        let latency = (1.0 - (metrics.latency_ms / max_acceptable_latency).min(1.0)).max(0.0);

        // User satisfaction (from implicit feedback)
        let satisfaction = feedback.infer_quality();

        // Weighted sum
        self.quality_weight * quality
            + self.cost_weight * cost
            + self.latency_weight * latency
            + self.user_satisfaction_weight * satisfaction
    }
}
```

### Time-Varying Rewards

```rust
fn calculate_time_aware_reward(
    metrics: &ResponseMetrics,
    hour: u8,
) -> f64 {
    let base_reward = metrics.quality_score;

    // Penalize cost more during peak hours
    let is_peak = hour >= 9 && hour <= 17;
    let cost_penalty = if is_peak {
        metrics.cost * 2.0  // Double cost penalty
    } else {
        metrics.cost
    };

    base_reward - cost_penalty
}
```

---

## Lesson 10: Advanced - Multi-Objective Optimization

**Goal**: Optimize multiple conflicting objectives using Pareto optimization.

**Duration**: 60 minutes

See [ML_EXAMPLES.md](ML_EXAMPLES.md#example-7-multi-objective-optimization-cost-vs-quality) for complete implementation.

---

## Lesson 11: Production Deployment Strategies

**Goal**: Safely deploy ML systems to production.

**Duration**: 45 minutes

### Gradual Rollout

```rust
struct GradualRollout {
    ml_system: LinUCB,
    fallback_strategy: Box<dyn FallbackStrategy>,
    rollout_percentage: f64,
}

impl GradualRollout {
    fn route_request(&self, context: &RequestContext) -> Uuid {
        let roll_dice = rand::random::<f64>();

        if roll_dice < self.rollout_percentage {
            // Use ML system
            self.ml_system.select_arm(context)
                .unwrap_or_else(|_| self.fallback_strategy.select())
        } else {
            // Use fallback
            self.fallback_strategy.select()
        }
    }

    fn increase_rollout(&mut self, step: f64) {
        self.rollout_percentage = (self.rollout_percentage + step).min(1.0);
    }
}

// Deployment schedule
// Day 1:  5% traffic
// Day 3: 10% traffic
// Day 7: 25% traffic
// Day 14: 50% traffic
// Day 21: 100% traffic
```

### Shadow Mode

```rust
async fn shadow_mode_request(
    context: &RequestContext,
    production_strategy: &impl Strategy,
    ml_strategy: &LinUCB,
) -> Uuid {
    // Production selection
    let prod_model = production_strategy.select(context);

    // Shadow ML selection (don't use result)
    let ml_model = ml_strategy.select_arm(context).unwrap();

    // Log comparison
    log_shadow_comparison(prod_model, ml_model);

    // Always use production
    prod_model
}
```

---

## Lesson 12: Drift Detection and Adaptation

**Goal**: Automatically detect and adapt to changing conditions.

**Duration**: 45 minutes

### Complete Drift Pipeline

```rust
use decision::{ADWIN, DriftStatus, LinUCB};

struct AdaptiveSystem {
    bandit: LinUCB,
    drift_detector: ADWIN,
    performance_history: VecDeque<f64>,
}

impl AdaptiveSystem {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            bandit: LinUCB::new(1.0, 10),
            drift_detector: ADWIN::new(0.002, 1000)?,
            performance_history: VecDeque::with_capacity(1000),
        })
    }

    fn process_request(&mut self, context: &RequestContext) -> Result<Uuid, String> {
        // Select arm
        let arm = self.bandit.select_arm(context)
            .map_err(|e| format!("Selection failed: {}", e))?;

        Ok(arm)
    }

    fn update_with_feedback(
        &mut self,
        arm: &Uuid,
        context: &RequestContext,
        reward: f64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Update bandit
        self.bandit.update(arm, context, reward)?;

        // Track performance
        self.performance_history.push_back(reward);

        // Check for drift
        let status = self.drift_detector.add(reward);

        match status {
            DriftStatus::Drift => {
                println!("Drift detected! Adapting...");
                self.handle_drift()?;
            }
            DriftStatus::Warning => {
                println!("Drift warning");
            }
            DriftStatus::Stable => {
                // All good
            }
        }

        Ok(())
    }

    fn handle_drift(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Strategy 1: Increase exploration
        self.bandit.set_alpha(self.bandit.alpha() * 1.5);

        // Strategy 2: Reset drift detector
        self.drift_detector = ADWIN::new(0.002, 1000)?;

        // Strategy 3: Log event
        log::warn!("Drift detected, increased exploration to {}", self.bandit.alpha());

        Ok(())
    }
}
```

---

## Summary

You've learned:

1. Basic multi-armed bandits (Thompson Sampling)
2. Production model selection
3. Contextual bandits and personalization
4. Automated parameter tuning
5. Feature engineering
6. A/B testing
7. Monitoring and alerting
8. Cold start handling
9. Custom reward functions
10. Multi-objective optimization
11. Production deployment
12. Drift detection and adaptation

## Next Steps

- [Examples](ML_EXAMPLES.md) - 20+ real-world use cases
- [Best Practices](ML_BEST_PRACTICES.md) - Advanced optimization tips
- [Troubleshooting](ML_TROUBLESHOOTING.md) - Debug common issues
- [API Reference](ML_API_REFERENCE.md) - Complete Rust API
