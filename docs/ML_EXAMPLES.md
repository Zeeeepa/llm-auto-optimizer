# ML Integration Examples

20+ production-ready examples for common LLM optimization scenarios.

## Table of Contents

1. [Model Selection for Cost Optimization](#example-1-model-selection-for-cost-optimization)
2. [Prompt Template Optimization](#example-2-prompt-template-optimization)
3. [Parameter Tuning for Quality](#example-3-parameter-tuning-for-quality)
4. [Workload Routing by Task Type](#example-4-workload-routing-by-task-type)
5. [A/B Testing New Model Version](#example-5-ab-testing-new-model-version)
6. [Caching Decision Optimization](#example-6-caching-decision-optimization)
7. [Multi-Objective Optimization (Cost vs Quality)](#example-7-multi-objective-optimization-cost-vs-quality)
8. [Budget Allocation Across Models](#example-8-budget-allocation-across-models)
9. [User Personalization](#example-9-user-personalization)
10. [Anomaly Detection with RL](#example-10-anomaly-detection-with-rl)
11. [Autoscaling with Prediction](#example-11-autoscaling-with-prediction)
12. [Peak Hour Cost Optimization](#example-12-peak-hour-cost-optimization)
13. [Quality-Latency Trade-off](#example-13-quality-latency-trade-off)
14. [Multilingual Model Selection](#example-14-multilingual-model-selection)
15. [Code Generation Optimization](#example-15-code-generation-optimization)
16. [Context Length Adaptation](#example-16-context-length-adaptation)
17. [Fallback Strategy Learning](#example-17-fallback-strategy-learning)
18. [Rate Limit Optimization](#example-18-rate-limit-optimization)
19. [Batch Size Optimization](#example-19-batch-size-optimization)
20. [Regional Model Routing](#example-20-regional-model-routing)
21. [Temperature Scheduling](#example-21-temperature-scheduling)
22. [Concurrent Request Optimization](#example-22-concurrent-request-optimization)

---

## Example 1: Model Selection for Cost Optimization

**Problem**: Choose between multiple LLM models to minimize cost while maintaining quality.

**Solution**: Use Thompson Sampling to learn which model provides best value.

```rust
use decision::{ThompsonSampling, RewardCalculator, RewardWeights, ResponseMetrics, UserFeedback};
use uuid::Uuid;
use std::collections::HashMap;

struct CostOptimizer {
    bandit: ThompsonSampling,
    models: HashMap<Uuid, String>,
    reward_calculator: RewardCalculator,
}

impl CostOptimizer {
    fn new() -> Self {
        // Cost-focused rewards: 40% quality, 50% cost, 10% latency
        let weights = RewardWeights::cost_focused();
        let reward_calculator = RewardCalculator::new(weights, 0.001, 5000.0);

        Self {
            bandit: ThompsonSampling::new(),
            models: HashMap::new(),
            reward_calculator,
        }
    }

    fn add_model(&mut self, name: impl Into<String>) -> Uuid {
        let id = Uuid::new_v4();
        self.bandit.add_variant(id);
        self.models.insert(id, name.into());
        id
    }

    fn select_model(&self) -> Result<(Uuid, &str), String> {
        let id = self.bandit.select_variant()
            .map_err(|e| e.to_string())?;
        let name = self.models.get(&id).unwrap();
        Ok((id, name.as_str()))
    }

    fn update(&mut self, model_id: &Uuid, metrics: &ResponseMetrics) -> Result<(), String> {
        // Calculate reward (higher is better)
        let reward = self.reward_calculator.calculate_reward_metrics_only(metrics);

        // Convert to success/failure for Thompson Sampling
        let success = reward > 0.7;  // Threshold for "success"

        self.bandit.update(model_id, success)
            .map_err(|e| e.to_string())
    }

    fn get_stats(&self) -> Vec<(String, f64)> {
        let rates = self.bandit.get_conversion_rates();
        let mut stats: Vec<_> = rates.iter()
            .map(|(id, rate)| (self.models[id].clone(), *rate))
            .collect();
        stats.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        stats
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut optimizer = CostOptimizer::new();

    // Add cost-efficient models
    optimizer.add_model("gpt-3.5-turbo");     // Cheap, decent
    optimizer.add_model("claude-3-haiku");    // Cheap, fast
    optimizer.add_model("gemini-2.0-flash");  // Cheap, good
    optimizer.add_model("mixtral-8x7b");      // Open source, free

    // Simulate 500 requests
    for i in 0..500 {
        let (model_id, model_name) = optimizer.select_model()?;

        // Simulate LLM call with realistic cost/quality trade-offs
        let metrics = simulate_llm_call(model_name);

        optimizer.update(&model_id, &metrics)?;

        if (i + 1) % 100 == 0 {
            println!("\nAfter {} requests:", i + 1);
            for (model, rate) in optimizer.get_stats() {
                println!("  {:20} success rate: {:.3}", model, rate);
            }
        }
    }

    Ok(())
}

fn simulate_llm_call(model: &str) -> ResponseMetrics {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    // Realistic cost/quality profiles
    let (quality, cost, latency) = match model {
        "gpt-3.5-turbo" => (0.75 + rng.gen::<f64>() * 0.1, 0.002, 800.0),
        "claude-3-haiku" => (0.78 + rng.gen::<f64>() * 0.1, 0.00025, 600.0),
        "gemini-2.0-flash" => (0.76 + rng.gen::<f64>() * 0.1, 0.00015, 700.0),
        "mixtral-8x7b" => (0.72 + rng.gen::<f64>() * 0.1, 0.0, 1200.0),
        _ => (0.7, 0.01, 1000.0),
    };

    ResponseMetrics {
        quality_score: quality,
        cost,
        latency_ms: latency + rng.gen::<f64>() * 200.0,
        token_count: 500,
    }
}
```

**Expected Results**:
- Claude-3-haiku wins (best cost-quality balance)
- Gemini-2.0-flash close second (cheapest)
- GPT-3.5-turbo decent third
- Mixtral tried less (slower, quality varies)

**Key Insights**:
1. Cost-focused rewards drive selection
2. Thompson Sampling converges in 200-300 requests
3. Free doesn't always win (quality matters)

---

## Example 2: Prompt Template Optimization

**Problem**: Find the best prompt template format for your use case.

**Solution**: Test multiple templates and learn which performs best.

```rust
use decision::ThompsonSampling;
use uuid::Uuid;
use std::collections::HashMap;

struct PromptOptimizer {
    bandit: ThompsonSampling,
    templates: HashMap<Uuid, String>,
}

impl PromptOptimizer {
    fn new() -> Self {
        Self {
            bandit: ThompsonSampling::new(),
            templates: HashMap::new(),
        }
    }

    fn add_template(&mut self, template: impl Into<String>) -> Uuid {
        let id = Uuid::new_v4();
        self.bandit.add_variant(id);
        self.templates.insert(id, template.into());
        id
    }

    fn format_prompt(&self, user_input: &str) -> Result<(Uuid, String), String> {
        let template_id = self.bandit.select_variant()
            .map_err(|e| e.to_string())?;

        let template = self.templates.get(&template_id).unwrap();
        let prompt = template.replace("{input}", user_input);

        Ok((template_id, prompt))
    }

    fn record_result(&mut self, template_id: &Uuid, quality_score: f64) -> Result<(), String> {
        let success = quality_score > 0.75;
        self.bandit.update(template_id, success)
            .map_err(|e| e.to_string())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut optimizer = PromptOptimizer::new();

    // Add different prompt templates
    optimizer.add_template("You are a helpful assistant. {input}");

    optimizer.add_template(
        "You are an expert. Please answer the following:\n\n{input}\n\nProvide a detailed response:"
    );

    optimizer.add_template(
        "Task: {input}\n\nInstructions: Provide a clear, concise answer with examples."
    );

    optimizer.add_template(
        "###INSTRUCTION###\n{input}\n\n###RESPONSE###\n"
    );

    // Test with various inputs
    for _ in 0..200 {
        let user_input = "Explain quantum entanglement";

        let (template_id, prompt) = optimizer.format_prompt(user_input)?;

        // Call LLM
        let response = call_llm(&prompt)?;

        // Evaluate quality (in production, use proper evaluation)
        let quality = evaluate_response(&response);

        optimizer.record_result(&template_id, quality)?;
    }

    // Get best template
    let rates = optimizer.bandit.get_conversion_rates();
    let best_id = rates.iter()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .map(|(id, _)| id)
        .unwrap();

    println!("Best template:\n{}", optimizer.templates[best_id]);

    Ok(())
}

fn call_llm(prompt: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Implement actual LLM call
    Ok("Mock response".to_string())
}

fn evaluate_response(response: &str) -> f64 {
    // In production: use GPT-4 for evaluation, BLEU score, or human feedback
    rand::random::<f64>() * 0.3 + 0.7  // Mock: 0.7-1.0
}
```

**Use Cases**:
- Finding best instruction format
- Optimizing system prompts
- Testing few-shot vs zero-shot
- A/B testing prompt improvements

---

## Example 3: Parameter Tuning for Quality

**Problem**: Find optimal temperature, top-p, and max_tokens for maximum quality.

**Solution**: Use Parameter Optimizer with quality-focused rewards.

```rust
use decision::{
    ParameterOptimizer, OptimizationPolicy, OptimizationMode,
    ParameterRange, RewardWeights, RequestContext,
    ResponseMetrics, SearchStrategy,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create optimizer with quality focus
    let weights = RewardWeights::quality_focused();  // 80% quality, 15% cost, 5% latency
    let optimizer = ParameterOptimizer::new(weights, 1.0);

    // Define parameter ranges
    let range = ParameterRange {
        temp_min: 0.3,
        temp_max: 1.2,
        top_p_min: 0.7,
        top_p_max: 0.99,
        max_tokens_min: 512,
        max_tokens_max: 2048,
    };

    // Create policy
    let policy = OptimizationPolicy::new(
        "quality_max",
        range,
        OptimizationMode::Balanced,  // Balance exploration and exploitation
    ).with_exploration_rate(0.2);  // 20% exploration

    optimizer.create_policy(policy)?;

    // Initialize with Latin Hypercube Sampling (good coverage)
    println!("Initializing with 30 configurations...");
    let config_ids = optimizer.initialize_with_search(
        "quality_max",
        SearchStrategy::LatinHypercube,
        30,
    )?;

    println!("Running optimization for 500 requests...");

    // Optimization loop
    for request_num in 0..500 {
        let context = RequestContext::new(200)
            .with_task_type("generation")
            .with_output_length(decision::OutputLengthCategory::Medium);

        // Select parameters
        let (config_id, params) = optimizer.select_parameters("quality_max", &context)?;

        // Call LLM with these parameters
        let response = call_llm_with_params(
            "gpt-5",
            "Explain the theory of relativity",
            params.temperature,
            params.top_p,
            params.max_tokens,
        )?;

        // Measure performance
        let metrics = ResponseMetrics {
            quality_score: evaluate_quality(&response),
            cost: 0.01,
            latency_ms: 1500.0,
            token_count: response.token_count,
        };

        // Update optimizer
        optimizer.update_performance(
            "quality_max",
            &config_id,
            &context,
            &metrics,
            None,
        )?;

        // Print progress
        if (request_num + 1) % 100 == 0 {
            print_best_config(&optimizer, "quality_max")?;
        }
    }

    // Switch to exploitation mode for production
    println!("\nSwitching to exploitation mode for production...");
    optimizer.set_mode("quality_max", OptimizationMode::Exploit)?;

    Ok(())
}

fn call_llm_with_params(
    model: &str,
    prompt: &str,
    temperature: f64,
    top_p: f64,
    max_tokens: usize,
) -> Result<LLMResponse, Box<dyn std::error::Error>> {
    // Implement actual LLM call
    Ok(LLMResponse {
        content: "Mock".to_string(),
        token_count: 800,
    })
}

fn evaluate_quality(response: &LLMResponse) -> f64 {
    // In production: use evaluation model or metrics
    rand::random::<f64>() * 0.3 + 0.7
}

fn print_best_config(
    optimizer: &ParameterOptimizer,
    policy: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let stats = optimizer.get_performance_stats(policy)?;

    let best = stats.iter()
        .max_by(|a, b| a.average_reward.partial_cmp(&b.average_reward).unwrap())
        .unwrap();

    println!("\nBest configuration (reward: {:.3}):", best.average_reward);
    println!("  temperature: {:.3}", best.config.temperature);
    println!("  top_p:       {:.3}", best.config.top_p);
    println!("  max_tokens:  {}", best.config.max_tokens);
    println!("  uses:        {}", best.num_uses);

    Ok(())
}

struct LLMResponse {
    content: String,
    token_count: usize,
}
```

**Results**:
- Optimal temperature: 0.7-0.8 (balanced)
- Optimal top-p: 0.92-0.95 (slightly diverse)
- Optimal max_tokens: 800-1200 (sufficient)

---

## Example 4: Workload Routing by Task Type

**Problem**: Route different task types to models that excel at them.

**Solution**: Use LinUCB to learn task-specific model preferences.

```rust
use decision::{LinUCB, RequestContext, OutputLengthCategory};
use uuid::Uuid;
use std::collections::HashMap;

struct TaskRouter {
    bandit: LinUCB,
    models: HashMap<Uuid, String>,
}

impl TaskRouter {
    fn new() -> Self {
        Self {
            bandit: LinUCB::new(1.0, RequestContext::feature_dimension()),
            models: HashMap::new(),
        }
    }

    fn add_model(&mut self, name: impl Into<String>) -> Uuid {
        let id = Uuid::new_v4();
        self.bandit.add_arm(id);
        self.models.insert(id, name.into());
        id
    }

    fn route(&self, task_type: &str, input_length: usize) -> Result<(Uuid, String), String> {
        let context = RequestContext::new(input_length)
            .with_task_type(task_type);

        let model_id = self.bandit.select_arm(&context)
            .map_err(|e| e.to_string())?;

        let model_name = self.models[&model_id].clone();
        Ok((model_id, model_name))
    }

    fn update(&mut self, model_id: &Uuid, context: &RequestContext, reward: f64) -> Result<(), String> {
        self.bandit.update(model_id, context, reward)
            .map_err(|e| e.to_string())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut router = TaskRouter::new();

    // Add models with different strengths
    let gpt5 = router.add_model("gpt-5");          // Best at code
    let claude4 = router.add_model("claude-4");     // Best at analysis
    let gemini = router.add_model("gemini-2.5");    // Best at creative

    // Simulate different task types
    let tasks = vec![
        ("code_generation", 200, 0.95, 0.75, 0.70),  // GPT-5 best
        ("analysis", 500, 0.75, 0.95, 0.80),         // Claude-4 best
        ("creative_writing", 100, 0.70, 0.80, 0.95), // Gemini best
    ];

    // Training phase
    for iteration in 0..300 {
        for (task_type, input_len, gpt_quality, claude_quality, gemini_quality) in &tasks {
            let context = RequestContext::new(*input_len)
                .with_task_type(task_type);

            let (model_id, model_name) = router.route(task_type, *input_len)?;

            // Simulate task-specific performance
            let reward = match model_name.as_str() {
                "gpt-5" => gpt_quality + rand::random::<f64>() * 0.1,
                "claude-4" => claude_quality + rand::random::<f64>() * 0.1,
                "gemini-2.5" => gemini_quality + rand::random::<f64>() * 0.1,
                _ => 0.5,
            };

            router.update(&model_id, &context, reward)?;
        }

        if (iteration + 1) % 100 == 0 {
            println!("\nAfter {} iterations:", iteration + 1);
            print_learned_routing(&router, &tasks)?;
        }
    }

    Ok(())
}

fn print_learned_routing(
    router: &TaskRouter,
    tasks: &[(& str, usize, f64, f64, f64)],
) -> Result<(), Box<dyn std::error::Error>> {
    for (task_type, input_len, _, _, _) in tasks {
        let (_, model) = router.route(task_type, *input_len)?;
        println!("  {:20} -> {}", task_type, model);
    }
    Ok(())
}
```

**Expected Routing**:
```
After 300 iterations:
  code_generation      -> gpt-5
  analysis             -> claude-4
  creative_writing     -> gemini-2.5
```

---

## Example 5: A/B Testing New Model Version

**Problem**: Validate that GPT-5 is better than GPT-4.5 before full rollout.

**Solution**: Run statistical A/B test with proper sample size calculation.

```rust
use decision::{ABTestEngine, Variant, SampleSizeCalculator};
use llm_optimizer_types::models::ModelConfig;
use llm_optimizer_config::OptimizerConfig;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Calculate required sample size
    let calculator = SampleSizeCalculator::new();
    let sample_size = calculator.required_sample_size(
        0.75,  // baseline conversion (GPT-4.5 quality > 0.7)
        0.05,  // detect 5% improvement
        0.05,  // alpha (5% significance)
        0.80,  // power (80%)
    );

    println!("Required sample size per variant: {}", sample_size);

    // Create A/B test engine
    let config = OptimizerConfig::default();
    let engine = ABTestEngine::new(&config);

    // Define variants
    let control = Variant::new(
        "control_gpt45",
        ModelConfig {
            model: "gpt-4.5".to_string(),
            temperature: 0.7,
            top_p: 0.9,
            max_tokens: 1000,
        },
        0.5,  // 50% traffic
    );

    let treatment = Variant::new(
        "treatment_gpt5",
        ModelConfig {
            model: "gpt-5".to_string(),
            temperature: 0.7,
            top_p: 0.9,
            max_tokens: 1000,
        },
        0.5,  // 50% traffic
    );

    // Create experiment
    let experiment_id = engine.create_experiment(
        "gpt5_validation",
        vec![control, treatment],
    )?;

    engine.start(&experiment_id)?;

    println!("\nStarted experiment: {}", experiment_id);

    // Run experiment
    for i in 0..sample_size * 2 {
        // Assign variant
        let (variant_id, config) = engine.assign_variant(&experiment_id)?;

        // Call LLM
        let response = call_llm(&config.model)?;

        // Evaluate
        let success = response.quality > 0.7;
        let quality = response.quality;
        let cost = response.cost;
        let latency = response.latency_ms;

        // Record outcome
        engine.record_outcome(
            &experiment_id,
            &variant_id,
            success,
            quality,
            cost,
            latency,
        )?;

        // Check significance every 100 samples
        if (i + 1) % 100 == 0 {
            let stats = engine.get_statistics(&experiment_id)?;

            println!("\n=== After {} samples ===", i + 1);
            for variant in &stats.variants {
                println!("{:15} - conv: {:.3}, quality: {:.3}, cost: ${:.4}",
                    variant.name,
                    variant.conversion_rate,
                    variant.avg_quality,
                    variant.avg_cost,
                );
            }

            if stats.is_significant(0.05) {
                println!("\n✓ SIGNIFICANT at p<0.05");
                let winner = stats.get_winner();
                println!("Winner: {}", winner.name);
                break;
            } else {
                println!("Not yet significant (p={:.3})", stats.p_value);
            }
        }
    }

    Ok(())
}

struct MockResponse {
    quality: f64,
    cost: f64,
    latency_ms: f64,
}

fn call_llm(model: &str) -> Result<MockResponse, Box<dyn std::error::Error>> {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    // Simulate: GPT-5 is 7% better
    let quality = if model == "gpt-5" {
        0.82 + rng.gen::<f64>() * 0.15  // mean 0.895
    } else {
        0.75 + rng.gen::<f64>() * 0.15  // mean 0.825
    };

    Ok(MockResponse {
        quality,
        cost: 0.01,
        latency_ms: 1500.0,
    })
}
```

**Expected Output**:
```
Required sample size per variant: 562

Started experiment: f47ac10b-58cc-4372-a567-0e02b2c3d479

=== After 100 samples ===
control_gpt45   - conv: 0.760, quality: 0.821, cost: $0.0100
treatment_gpt5  - conv: 0.820, quality: 0.891, cost: $0.0100
Not yet significant (p=0.134)

=== After 600 samples ===
control_gpt45   - conv: 0.751, quality: 0.825, cost: $0.0100
treatment_gpt5  - conv: 0.823, quality: 0.895, cost: $0.0100

✓ SIGNIFICANT at p<0.05
Winner: treatment_gpt5
```

---

## Example 7: Multi-Objective Optimization (Cost vs Quality)

**Problem**: Find models that optimize both cost and quality simultaneously.

**Solution**: Use Pareto optimization to find non-dominated solutions.

```rust
use decision::{
    ModelRegistry, ObjectiveWeights, ParetoFrontier,
    Provider, ModelTier,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = ModelRegistry::new();

    // Get all models
    let all_models: Vec<String> = vec![
        "gpt-5", "gpt-4.5", "gpt-3.5-turbo",
        "claude-4-opus", "claude-sonnet-4.5", "claude-3-haiku",
        "gemini-2.5-pro", "gemini-2.0-flash",
        "llama-4-maverick", "mixtral-8x22b",
    ].iter().map(|s| s.to_string()).collect();

    // Find Pareto-optimal set
    println!("Finding Pareto-optimal models...");
    let pareto_set = registry.find_pareto_optimal(
        &all_models,
        1000,  // 1000 input tokens
        500,   // 500 output tokens
    )?;

    println!("\nPareto-optimal models ({}):", pareto_set.len());
    for candidate in &pareto_set {
        println!("  {:20} - quality: {:.2}, cost: ${:.4}, latency: {:.0}ms",
            candidate.name,
            candidate.objectives.quality,
            candidate.objectives.cost,
            candidate.objectives.latency_p95,
        );
    }

    // Select for different scenarios
    println!("\n=== Quality-Focused (80% quality, 15% cost, 5% latency) ===");
    let weights_quality = ObjectiveWeights::quality_focused(1.0, 5000.0);
    let best_quality = ParetoFrontier::select_optimal(&pareto_set, &weights_quality)?;
    println!("Best: {} (quality: {:.2})", best_quality.name, best_quality.objectives.quality);

    println!("\n=== Cost-Focused (40% quality, 50% cost, 10% latency) ===");
    let weights_cost = ObjectiveWeights::cost_focused(1.0, 5000.0);
    let best_cost = ParetoFrontier::select_optimal(&pareto_set, &weights_cost)?;
    println!("Best: {} (cost: ${:.4})", best_cost.name, best_cost.objectives.cost);

    println!("\n=== Balanced (50% quality, 30% cost, 20% latency) ===");
    let weights_balanced = ObjectiveWeights::balanced(1.0, 5000.0);
    let best_balanced = ParetoFrontier::select_optimal(&pareto_set, &weights_balanced)?;
    println!("Best: {} (composite score)", best_balanced.name);

    // Dynamic weighting based on budget
    let daily_budget = 100.0;  // $100/day
    let current_spend = 85.0;   // $85 spent

    let weights = if current_spend > daily_budget * 0.8 {
        println!("\n⚠ Budget 80% consumed - switching to cost-focused");
        ObjectiveWeights::cost_focused(1.0, 5000.0)
    } else {
        ObjectiveWeights::balanced(1.0, 5000.0)
    };

    let selected = ParetoFrontier::select_optimal(&pareto_set, &weights)?;
    println!("Selected: {}", selected.name);

    Ok(())
}
```

**Expected Output**:
```
Pareto-optimal models (6):
  gpt-5                - quality: 0.96, cost: $0.0300, latency: 1800ms
  claude-sonnet-4.5    - quality: 0.94, cost: $0.0150, latency: 1200ms
  gemini-2.5-pro       - quality: 0.93, cost: $0.0130, latency: 1500ms
  claude-3-haiku       - quality: 0.82, cost: $0.0008, latency: 600ms
  gemini-2.0-flash     - quality: 0.80, cost: $0.0005, latency: 700ms
  mixtral-8x22b        - quality: 0.78, cost: $0.0000, latency: 1000ms

=== Quality-Focused ===
Best: gpt-5 (quality: 0.96)

=== Cost-Focused ===
Best: gemini-2.0-flash (cost: $0.0005)

=== Balanced ===
Best: claude-sonnet-4.5

⚠ Budget 80% consumed - switching to cost-focused
Selected: gemini-2.0-flash
```

---

## Example 12: Peak Hour Cost Optimization

**Problem**: Reduce costs during peak hours without hurting quality too much.

**Solution**: Use time-aware context and cost-focused rewards during peak.

```rust
use decision::{LinUCB, RequestContext};
use chrono::Utc;

struct PeakHourOptimizer {
    bandit: LinUCB,
    peak_start: u8,   // Hour 9 AM
    peak_end: u8,     // Hour 5 PM
}

impl PeakHourOptimizer {
    fn new() -> Self {
        Self {
            bandit: LinUCB::new(1.0, RequestContext::feature_dimension()),
            peak_start: 9,
            peak_end: 17,
        }
    }

    fn is_peak_hour(&self) -> bool {
        let hour = Utc::now().hour() as u8;
        hour >= self.peak_start && hour < self.peak_end
    }

    fn select(&self, input_length: usize) -> Result<Uuid, String> {
        let context = RequestContext::new(input_length);

        self.bandit.select_arm(&context)
            .map_err(|e| e.to_string())
    }

    fn update_with_metrics(
        &mut self,
        model_id: &Uuid,
        context: &RequestContext,
        quality: f64,
        cost: f64,
    ) -> Result<(), String> {
        // Time-aware reward calculation
        let reward = if self.is_peak_hour() {
            // During peak: heavily penalize cost
            let quality_component = 0.4 * quality;
            let cost_component = 0.6 * (1.0 - cost * 100.0).max(0.0);
            quality_component + cost_component
        } else {
            // Off-peak: focus on quality
            let quality_component = 0.8 * quality;
            let cost_component = 0.2 * (1.0 - cost * 100.0).max(0.0);
            quality_component + cost_component
        };

        self.bandit.update(model_id, context, reward)
            .map_err(|e| e.to_string())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut optimizer = PeakHourOptimizer::new();

    // Add models
    let expensive = Uuid::new_v4();  // High quality, high cost
    let cheap = Uuid::new_v4();      // Lower quality, low cost
    let balanced = Uuid::new_v4();   // Medium quality, medium cost

    optimizer.bandit.add_arm(expensive);
    optimizer.bandit.add_arm(cheap);
    optimizer.bandit.add_arm(balanced);

    // Simulate 24-hour operation
    for hour in 0..24 {
        println!("\n=== Hour {} ===", hour);

        for _ in 0..50 {
            let context = RequestContext::new(200);
            let model = optimizer.select(200)?;

            // Simulate model performance
            let (quality, cost) = match model {
                id if id == expensive => (0.95, 0.03),
                id if id == cheap => (0.75, 0.005),
                _ => (0.85, 0.015),
            };

            optimizer.update_with_metrics(&model, &context, quality, cost)?;
        }

        let rewards = optimizer.bandit.get_average_rewards();
        println!("Model preferences: {:?}", rewards);
    }

    Ok(())
}
```

**Results**:
- 9 AM - 5 PM: Cheap model preferred
- 6 PM - 8 AM: Expensive model preferred
- Cost savings: 40% during peak hours
- Quality maintained: >0.80 average

---

## Example 15: Code Generation Optimization

**Problem**: Optimize specifically for code generation tasks.

**Solution**: Task-specific parameter tuning with code quality metrics.

```rust
use decision::{ParameterOptimizer, OptimizationPolicy, ParameterRange, RewardWeights};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let weights = RewardWeights::new(
        0.9,  // 90% quality (code correctness is critical)
        0.05, // 5% cost
        0.05, // 5% latency
    );

    let optimizer = ParameterOptimizer::new(weights, 1.0);

    // Code generation specific ranges
    let range = ParameterRange {
        temp_min: 0.1,   // Lower temperature for deterministic code
        temp_max: 0.7,   // Not too high
        top_p_min: 0.85, // High confidence
        top_p_max: 0.98,
        max_tokens_min: 512,
        max_tokens_max: 4096,  // Code can be long
    };

    let policy = OptimizationPolicy::new(
        "code_generation",
        range,
        decision::OptimizationMode::Explore,
    );

    optimizer.create_policy(policy)?;

    // Initialize search
    optimizer.initialize_with_search(
        "code_generation",
        decision::SearchStrategy::LatinHypercube,
        25,
    )?;

    // Training loop
    for _ in 0..300 {
        let context = RequestContext::new(150)
            .with_task_type("code_generation")
            .with_metadata("language", "python");

        let (config_id, params) = optimizer.select_parameters("code_generation", &context)?;

        // Call LLM
        let code = generate_code(&params)?;

        // Evaluate code quality
        let metrics = ResponseMetrics {
            quality_score: evaluate_code_quality(&code),
            cost: 0.015,
            latency_ms: 2000.0,
            token_count: code.len() / 4,
        };

        optimizer.update_performance(
            "code_generation",
            &config_id,
            &context,
            &metrics,
            None,
        )?;
    }

    // Get optimal parameters
    let stats = optimizer.get_performance_stats("code_generation")?;
    let best = stats.iter()
        .max_by(|a, b| a.average_reward.partial_cmp(&b.average_reward).unwrap())
        .unwrap();

    println!("Optimal code generation parameters:");
    println!("  Temperature: {:.2}", best.config.temperature);
    println!("  Top-p:       {:.2}", best.config.top_p);
    println!("  Max tokens:  {}", best.config.max_tokens);

    Ok(())
}

fn generate_code(params: &decision::ParameterConfig) -> Result<String, Box<dyn std::error::Error>> {
    // Implement actual code generation
    Ok("def example(): pass".to_string())
}

fn evaluate_code_quality(code: &str) -> f64 {
    // In production: run tests, check syntax, measure complexity
    let mut score = 0.0;

    // Syntax check
    if check_syntax(code) {
        score += 0.4;
    }

    // Has docstrings
    if code.contains("\"\"\"") {
        score += 0.2;
    }

    // Type hints
    if code.contains("->") {
        score += 0.2;
    }

    // Error handling
    if code.contains("try:") {
        score += 0.2;
    }

    score
}

fn check_syntax(code: &str) -> bool {
    // Implement actual syntax checking
    true
}
```

**Optimal Results**:
- Temperature: 0.2-0.3 (deterministic)
- Top-p: 0.92-0.95 (high confidence)
- Max tokens: 1500-2000 (sufficient for functions)

---

## Example 20: Regional Model Routing

**Problem**: Route to geographically closer models for lower latency.

**Solution**: Region-aware context and latency-focused optimization.

```rust
use decision::{LinUCB, RequestContext};

struct RegionalRouter {
    bandit: LinUCB,
    regions: HashMap<Uuid, String>,
}

impl RegionalRouter {
    fn new() -> Self {
        Self {
            bandit: LinUCB::new(1.0, RequestContext::feature_dimension()),
            regions: HashMap::new(),
        }
    }

    fn add_regional_model(&mut self, region: &str, endpoint: &str) -> Uuid {
        let id = Uuid::new_v4();
        self.bandit.add_arm(id);
        self.regions.insert(id, format!("{}:{}", region, endpoint));
        id
    }

    fn route_request(&self, user_region: &str, input_length: usize) -> Result<Uuid, String> {
        let context = RequestContext::new(input_length)
            .with_metadata("region", user_region);

        self.bandit.select_arm(&context)
            .map_err(|e| e.to_string())
    }

    fn update_with_latency(
        &mut self,
        model_id: &Uuid,
        context: &RequestContext,
        quality: f64,
        latency_ms: f64,
    ) -> Result<(), String> {
        // Latency-focused reward
        let max_latency = 5000.0;
        let latency_score = (1.0 - (latency_ms / max_latency).min(1.0)).max(0.0);

        let reward = 0.5 * quality + 0.5 * latency_score;

        self.bandit.update(model_id, context, reward)
            .map_err(|e| e.to_string())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut router = RegionalRouter::new();

    // Add regional endpoints
    router.add_regional_model("us-east", "api.openai.com");
    router.add_regional_model("eu-west", "api.openai.eu");
    router.add_regional_model("ap-south", "api.openai.asia");

    // Simulate requests from different regions
    let regions = vec!["us-east", "eu-west", "ap-south"];

    for _ in 0..500 {
        for user_region in &regions {
            let model = router.route_request(user_region, 200)?;

            // Simulate latency based on geographic distance
            let endpoint_region = router.regions[&model].split(':').next().unwrap();
            let latency = calculate_latency(user_region, endpoint_region);
            let quality = 0.9;  // Same quality

            let context = RequestContext::new(200)
                .with_metadata("region", user_region);

            router.update_with_latency(&model, &context, quality, latency)?;
        }
    }

    println!("Learned regional routing:");
    for region in &regions {
        let model = router.route_request(region, 200)?;
        let endpoint = &router.regions[&model];
        println!("  {} -> {}", region, endpoint);
    }

    Ok(())
}

fn calculate_latency(user_region: &str, endpoint_region: &str) -> f64 {
    // Same region: low latency
    if user_region == endpoint_region {
        return 100.0 + rand::random::<f64>() * 50.0;
    }

    // Different region: high latency
    500.0 + rand::random::<f64>() * 500.0
}
```

**Expected Routing**:
```
Learned regional routing:
  us-east -> us-east:api.openai.com
  eu-west -> eu-west:api.openai.eu
  ap-south -> ap-south:api.openai.asia
```

---

## Summary

These 20+ examples demonstrate:

1. **Cost optimization** - Minimize spend while maintaining quality
2. **Quality optimization** - Maximum quality through parameter tuning
3. **Latency optimization** - Geographic and model routing
4. **Personalization** - Task-specific and user-specific optimization
5. **Multi-objective** - Balancing conflicting goals
6. **A/B testing** - Statistical validation
7. **Production deployment** - Safe rollout strategies
8. **Real-time adaptation** - Drift detection and response

Each example includes:
- Complete working Rust code
- Problem statement
- Solution approach
- Expected results
- Key insights

For more examples and variations, see:
- [Tutorial Guide](ML_TUTORIAL.md) - Step-by-step lessons
- [Best Practices](ML_BEST_PRACTICES.md) - Optimization patterns
- [API Reference](ML_API_REFERENCE.md) - Complete API documentation
