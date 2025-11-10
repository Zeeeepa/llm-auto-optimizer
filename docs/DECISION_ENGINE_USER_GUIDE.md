# Decision Engine User Guide

**Intelligent Multi-Strategy Optimization for LLM Infrastructure**

Version: 1.0
Last Updated: November 2025

---

## Table of Contents

1. [Quick Start Guide](#quick-start-guide)
2. [Overview of Decision Engine](#overview-of-decision-engine)
3. [Strategy 1: Model Selection Strategy](#strategy-1-model-selection-strategy)
4. [Strategy 2: Caching Strategy](#strategy-2-caching-strategy)
5. [Strategy 3: Rate Limiting Strategy](#strategy-3-rate-limiting-strategy)
6. [Strategy 4: Request Batching Strategy](#strategy-4-request-batching-strategy)
7. [Strategy 5: Prompt Optimization Strategy](#strategy-5-prompt-optimization-strategy)
8. [Integration with Analyzer Engine](#integration-with-analyzer-engine)
9. [Configuration Reference](#configuration-reference)
10. [Usage Examples](#usage-examples)
11. [Best Practices](#best-practices)
12. [Troubleshooting](#troubleshooting)
13. [Performance Tuning](#performance-tuning)
14. [Production Deployment Guide](#production-deployment-guide)

---

## Quick Start Guide

### Installation

```bash
# Clone the repository
git clone https://github.com/globalbusinessadvisors/llm-auto-optimizer.git
cd llm-auto-optimizer

# Build the decision engine
cargo build --release -p decision

# Run tests to verify installation
cargo test -p decision --all
```

### 5-Minute Setup

```rust
use llm_auto_optimizer::decision::{
    ModelRegistry, ReinforcementEngine, AdaptiveParameterTuner,
    ABTestEngine, ParetoFrontier, ThresholdMonitoringSystem,
};
use llm_auto_optimizer::config::OptimizerConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = OptimizerConfig::from_file("config.yaml")?;

    // Initialize Decision Engine components
    let model_registry = ModelRegistry::new();
    let ab_engine = ABTestEngine::new(&config);
    let param_tuner = AdaptiveParameterTuner::with_defaults();
    let reinforcement = ReinforcementEngine::with_linucb(
        1.0,
        RewardWeights::default_weights()
    );

    println!("Decision Engine initialized successfully!");
    println!("Available models: {}", model_registry.available_models().len());

    Ok(())
}
```

### Your First Optimization

```rust
use llm_auto_optimizer::decision::*;
use uuid::Uuid;

// 1. Register available models
let registry = ModelRegistry::new();
let available = registry.available_models();

// 2. Find optimal model using Pareto optimization
let model_ids: Vec<String> = available
    .iter()
    .map(|m| m.id.clone())
    .collect();

let optimal = registry.find_pareto_optimal(
    &model_ids,
    1000,  // input tokens
    500    // output tokens
)?;

println!("Found {} Pareto-optimal models", optimal.len());

// 3. Start A/B test to validate
let mut ab_engine = ABTestEngine::new(&config);
let experiment = ab_engine.create_experiment(
    "model_comparison",
    vec![
        Variant::new("gpt-4-turbo", config_gpt4, 0.5),
        Variant::new("claude-3.5-sonnet", config_claude, 0.5),
    ]
)?;

ab_engine.start(&experiment)?;
```

---

## Overview of Decision Engine

### What is the Decision Engine?

The **Decision Engine** is the intelligence layer of the LLM Auto Optimizer that makes real-time decisions about which optimization strategies to apply based on continuous performance feedback. It uses five core optimization strategies to balance quality, cost, and latency:

1. **Model Selection** - Choose optimal LLM based on task requirements
2. **Caching** - Reduce costs by reusing previous results
3. **Rate Limiting** - Control traffic and prevent overload
4. **Request Batching** - Increase throughput via request grouping
5. **Prompt Optimization** - Improve prompts using A/B testing and reinforcement learning

### Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│              Decision Engine Architecture                │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌────────────┐  ┌────────────┐  ┌──────────────┐      │
│  │   Model    │  │  Pareto    │  │ Reinforcement│      │
│  │  Registry  │─▶│  Frontier  │◀─│   Engine     │      │
│  └────────────┘  └────────────┘  └──────────────┘      │
│         │              │                  │              │
│         ▼              ▼                  ▼              │
│  ┌─────────────────────────────────────────────┐       │
│  │        Decision Orchestrator                 │       │
│  │  • Context Analysis                          │       │
│  │  • Strategy Selection                        │       │
│  │  • Multi-objective Optimization              │       │
│  └─────────────────────────────────────────────┘       │
│         │              │                  │              │
│         ▼              ▼                  ▼              │
│  ┌────────────┐  ┌────────────┐  ┌──────────────┐      │
│  │  A/B Test  │  │ Adaptive   │  │  Threshold   │      │
│  │   Engine   │  │ Parameter  │  │   Monitor    │      │
│  │            │  │   Tuner    │  │              │      │
│  └────────────┘  └────────────┘  └──────────────┘      │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

### Key Features

- **Multi-Objective Optimization**: Balance quality, cost, and latency simultaneously
- **Contextual Adaptation**: Decisions adapt based on request context and task type
- **Continuous Learning**: Reinforcement learning improves decisions over time
- **Statistical Rigor**: A/B tests with p-value < 0.05 significance threshold
- **Production-Ready**: Sub-second decision latency, 99.9% availability

### Core Components

| Component | Purpose | Implementation |
|-----------|---------|----------------|
| **Model Registry** | Catalog of 30+ LLM models with pricing/performance | `ModelRegistry` |
| **Pareto Frontier** | Multi-objective optimization for model selection | `ParetoFrontier` |
| **A/B Testing** | Statistical experiment framework | `ABTestEngine` |
| **Thompson Sampling** | Bayesian exploration/exploitation | `ThompsonSampling` |
| **Contextual Bandits** | Context-aware reinforcement learning | `LinUCB`, `ContextualThompson` |
| **Adaptive Tuner** | Dynamic parameter optimization | `AdaptiveParameterTuner` |
| **Drift Detection** | Detect performance degradation | `ADWIN`, `PageHinkley`, `CUSUM` |
| **Anomaly Detection** | Identify outliers and issues | `ZScoreDetector`, `IQRDetector` |
| **Threshold Monitor** | Rule-based alerts and triggers | `ThresholdMonitoringSystem` |

---

## Strategy 1: Model Selection Strategy

### Overview

The **Model Selection Strategy** automatically chooses the optimal LLM model for each request based on quality, cost, and latency requirements. It uses Pareto optimization to identify models on the efficiency frontier and reinforcement learning to adapt over time.

### When to Use

✅ **Use Model Selection when:**
- You have access to multiple LLM providers (OpenAI, Anthropic, Google, etc.)
- Different tasks have different quality/cost requirements
- You want to minimize costs while maintaining quality
- You need to optimize latency for real-time applications
- Traffic patterns vary across task types

❌ **Don't use when:**
- You're locked into a single provider
- All requests require the highest quality model
- Cost is not a concern

### How It Works

```
┌─────────────────────────────────────────────────────────┐
│          Model Selection Decision Flow                   │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  1. Request Context Analysis                            │
│     ├─ Task type (code, creative, analytical, etc.)     │
│     ├─ Input/output token estimate                      │
│     ├─ Quality requirements                             │
│     └─ Latency constraints                              │
│                                                          │
│  2. Candidate Filtering                                 │
│     ├─ Filter by capabilities (vision, function calls)  │
│     ├─ Filter by availability                           │
│     └─ Filter by cost constraints                       │
│                                                          │
│  3. Pareto Optimization                                 │
│     ├─ Calculate quality/cost/latency objectives        │
│     ├─ Find non-dominated models                        │
│     └─ Apply objective weights                          │
│                                                          │
│  4. Final Selection                                     │
│     ├─ Use reinforcement learning (contextual bandit)   │
│     ├─ Exploration vs exploitation (ε-greedy)           │
│     └─ Return selected model configuration              │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

### Configuration

```yaml
# config.yaml - Model Selection Strategy
strategies:
  model_selection:
    enabled: true

    # Objective weights (must sum to 1.0)
    weights:
      quality: 0.5    # 50% weight on quality
      cost: 0.3       # 30% weight on cost
      latency: 0.2    # 20% weight on latency

    # Constraints
    max_cost_per_request: 1.0      # Maximum $1 per request
    max_latency_p95_ms: 5000       # Maximum 5 second p95 latency
    min_quality_score: 0.7         # Minimum 0.7 quality score

    # Provider preferences
    providers:
      - openai
      - anthropic
      - google
      - meta
      - mistral

    # Reinforcement learning
    algorithm: "linucb"              # linucb or contextual_thompson
    exploration_factor: 1.0          # UCB exploration parameter

    # Fallback behavior
    fallback_model: "gpt-4-turbo"
    fallback_on_error: true
```

### Code Examples

#### Basic Model Selection

```rust
use llm_auto_optimizer::decision::*;

// Initialize registry
let registry = ModelRegistry::new();

// Define your task context
let context = RequestContext::new(1500)  // 1500 input tokens
    .with_task_type("code_generation")
    .with_output_length(OutputLengthCategory::Medium);

// Get available models
let available_models: Vec<String> = registry
    .available_models()
    .iter()
    .map(|m| m.id.clone())
    .collect();

// Find Pareto-optimal models
let optimal_models = registry.find_pareto_optimal(
    &available_models,
    1500,  // input tokens
    800    // output tokens
)?;

println!("Found {} Pareto-optimal models:", optimal_models.len());
for model in &optimal_models {
    println!("  - {} (quality: {:.2}, cost: ${:.4})",
        model.name,
        model.objectives.quality,
        model.objectives.cost
    );
}
```

#### Model Selection with Custom Weights

```rust
use llm_auto_optimizer::decision::*;

let registry = ModelRegistry::new();

// Define custom weights (cost-focused)
let weights = ObjectiveWeights::cost_focused(1.0, 5000.0);

// Get candidate models
let models = vec![
    "gpt-4-turbo".to_string(),
    "claude-3.5-sonnet".to_string(),
    "gemini-1.5-pro".to_string(),
    "gpt-3.5-turbo".to_string(),
];

// Find optimal with custom weights
let candidates = registry.find_pareto_optimal(&models, 1000, 500)?;

// Select best based on composite score
let best = candidates
    .iter()
    .max_by(|a, b| {
        let score_a = a.composite_score(&weights);
        let score_b = b.composite_score(&weights);
        score_a.partial_cmp(&score_b).unwrap()
    })
    .unwrap();

println!("Best model for cost optimization: {}", best.name);
println!("  Quality: {:.2}", best.objectives.quality);
println!("  Cost: ${:.4}", best.objectives.cost);
println!("  Latency P95: {:.0}ms", best.objectives.latency_p95);
```

#### Reinforcement Learning-Based Selection

```rust
use llm_auto_optimizer::decision::*;
use llm_optimizer_types::models::ModelConfig;

// Initialize reinforcement engine with LinUCB
let engine = ReinforcementEngine::with_linucb(
    1.0,  // alpha (exploration factor)
    RewardWeights::balanced_weights()
);

// Create a policy with multiple model variants
let variants = vec![
    (Uuid::new_v4(), ModelConfig {
        provider: "openai".to_string(),
        model: "gpt-4-turbo".to_string(),
        temperature: 0.7,
        max_tokens: 2048,
        ..Default::default()
    }),
    (Uuid::new_v4(), ModelConfig {
        provider: "anthropic".to_string(),
        model: "claude-3.5-sonnet".to_string(),
        temperature: 0.7,
        max_tokens: 2048,
        ..Default::default()
    }),
    (Uuid::new_v4(), ModelConfig {
        provider: "google".to_string(),
        model: "gemini-1.5-pro".to_string(),
        temperature: 0.7,
        max_tokens: 2048,
        ..Default::default()
    }),
];

// Create policy
engine.create_policy("model_selection_policy", variants)?;

// Select model based on context
let context = RequestContext::new(800)
    .with_task_type("general_qa");

let (variant_id, config) = engine.select_variant(
    "model_selection_policy",
    &context
)?;

println!("Selected model: {}", config.model);

// After execution, update with feedback
let metrics = ResponseMetrics {
    quality_score: 0.92,
    cost: 0.045,
    latency_ms: 1234.0,
    token_count: 1200,
};

let feedback = UserFeedback {
    task_completed: true,
    explicit_rating: Some(4.5),
    thumbs_up: true,
    ..Default::default()
};

engine.update_from_feedback(
    "model_selection_policy",
    &variant_id,
    &context,
    &metrics,
    &feedback
)?;
```

#### Task-Specific Model Selection

```rust
use llm_auto_optimizer::decision::*;

fn select_model_for_task(
    task_type: &str,
    registry: &ModelRegistry
) -> Result<ModelDefinition> {
    match task_type {
        "code_generation" => {
            // Prefer models with low temperature, high accuracy
            let candidates = vec![
                "gpt-4-turbo",
                "claude-3.5-sonnet",
                "deepseek-v3",
            ];

            let optimal = registry.find_pareto_optimal(
                &candidates.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                1500,
                2000
            )?;

            // Select highest quality for code
            let best = optimal.iter()
                .max_by(|a, b| {
                    a.objectives.quality
                        .partial_cmp(&b.objectives.quality)
                        .unwrap()
                })
                .unwrap();

            Ok(registry.get(&best.name).unwrap().clone())
        },

        "creative_writing" => {
            // Prefer models with good creativity/cost balance
            let candidates = vec![
                "gpt-4-turbo",
                "claude-3.5-sonnet",
                "mixtral-8x22b",
            ];

            let weights = ObjectiveWeights::new(
                0.6, 0.3, 0.1,  // Quality-focused
                1.0, 5000.0
            );

            let optimal = registry.find_pareto_optimal(
                &candidates.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                500,
                1500
            )?;

            let best = optimal.iter()
                .max_by(|a, b| {
                    a.composite_score(&weights)
                        .partial_cmp(&b.composite_score(&weights))
                        .unwrap()
                })
                .unwrap();

            Ok(registry.get(&best.name).unwrap().clone())
        },

        "simple_qa" => {
            // Prefer low-cost models
            let candidates = vec![
                "gpt-3.5-turbo",
                "claude-3-haiku",
                "gemini-2.0-flash",
            ];

            let weights = ObjectiveWeights::cost_focused(0.1, 3000.0);

            let optimal = registry.find_pareto_optimal(
                &candidates.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                200,
                300
            )?;

            let best = optimal.iter()
                .max_by(|a, b| {
                    a.composite_score(&weights)
                        .partial_cmp(&b.composite_score(&weights))
                        .unwrap()
                })
                .unwrap();

            Ok(registry.get(&best.name).unwrap().clone())
        },

        _ => {
            // Default: balanced approach
            let all_available = registry.available_models();
            let candidates: Vec<String> = all_available
                .iter()
                .map(|m| m.id.clone())
                .collect();

            let optimal = registry.find_pareto_optimal(&candidates, 500, 500)?;

            let weights = ObjectiveWeights::balanced(1.0, 5000.0);
            let best = optimal.iter()
                .max_by(|a, b| {
                    a.composite_score(&weights)
                        .partial_cmp(&b.composite_score(&weights))
                        .unwrap()
                })
                .unwrap();

            Ok(registry.get(&best.name).unwrap().clone())
        }
    }
}
```

### Performance Metrics

Track these metrics to evaluate model selection effectiveness:

```rust
use llm_auto_optimizer::decision::*;

// Get policy statistics
let stats = engine.get_policy_stats("model_selection_policy")?;

for variant_stats in stats {
    println!("Model: {:?}", variant_stats.variant_id);
    println!("  Average reward: {:.3}", variant_stats.average_reward);
    println!("  Selections: {}", variant_stats.num_selections);
    println!("  Total reward: {:.2}", variant_stats.total_reward);
}
```

**Key Metrics:**
- Average reward per model
- Selection frequency distribution
- Cost savings vs baseline
- Quality score by model
- Latency p50/p95/p99 by model

### Best Practices

1. **Start with Pareto Optimization**: Use Pareto frontier to identify candidate models before applying RL
2. **Set Realistic Constraints**: Define max_cost and max_latency based on actual requirements
3. **Use Task-Specific Weights**: Adjust quality/cost/latency weights per task type
4. **Monitor Model Drift**: Track if model performance degrades over time
5. **Implement Fallbacks**: Always configure a reliable fallback model
6. **Regular Retraining**: Re-evaluate Pareto frontier monthly as model pricing/performance changes
7. **A/B Test Changes**: Use A/B testing when switching default models

---

## Strategy 2: Caching Strategy

### Overview

The **Caching Strategy** reduces costs and latency by detecting and reusing responses for semantically similar or identical requests. It uses semantic similarity, exact matching, and prompt normalization to maximize cache hit rates.

### When to Use

✅ **Use Caching when:**
- You have repeated or similar queries
- Request patterns are predictable
- Cost reduction is a priority
- Latency improvement is critical
- Content doesn't change frequently

❌ **Don't use when:**
- Every request must be unique
- Real-time data is required
- Responses must never be reused
- Compliance prohibits caching

### How It Works

```
┌─────────────────────────────────────────────────────────┐
│              Caching Strategy Flow                       │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  1. Request Normalization                               │
│     ├─ Remove whitespace variations                     │
│     ├─ Normalize casing                                 │
│     ├─ Strip irrelevant metadata                        │
│     └─ Generate canonical form                          │
│                                                          │
│  2. Cache Lookup                                        │
│     ├─ Exact match (hash-based)                         │
│     ├─ Semantic similarity (embeddings)                 │
│     └─ Prefix matching (for completions)                │
│                                                          │
│  3. Freshness Check                                     │
│     ├─ Verify TTL not expired                           │
│     ├─ Check content validity                           │
│     └─ Validate context compatibility                   │
│                                                          │
│  4. Decision                                            │
│     ├─ Cache HIT → Return cached response               │
│     ├─ Cache MISS → Execute request                     │
│     └─ Store result in cache                            │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

### Configuration

```yaml
# config.yaml - Caching Strategy
strategies:
  caching:
    enabled: true

    # Cache backend
    backend: "redis"  # redis, postgres, or memory

    # Redis configuration
    redis:
      host: "localhost"
      port: 6379
      db: 0
      password: null
      tls_enabled: false

    # Matching strategies
    exact_match:
      enabled: true
      normalize_whitespace: true
      case_sensitive: false

    semantic_match:
      enabled: true
      similarity_threshold: 0.95  # 95% similarity required
      embedding_model: "text-embedding-3-small"
      max_candidates: 100

    # TTL settings
    default_ttl_seconds: 3600       # 1 hour default
    max_ttl_seconds: 86400          # 24 hour maximum

    # Cache eviction
    max_cache_size_mb: 1024         # 1GB cache limit
    eviction_policy: "lru"          # lru, lfu, or ttl

    # Performance
    cache_timeout_ms: 100           # Max time to wait for cache
    async_store: true               # Store asynchronously

    # Exclusions
    exclude_patterns:
      - ".*unique.*"                # Don't cache "unique" requests
      - ".*realtime.*"

    exclude_endpoints:
      - "/api/stream"
      - "/api/chat/stream"
```

### Code Examples

#### Basic Caching Implementation

```rust
use llm_auto_optimizer::decision::*;
use redis::aio::ConnectionManager;
use sha2::{Sha256, Digest};

pub struct CacheStrategy {
    redis: ConnectionManager,
    ttl_seconds: u64,
    similarity_threshold: f64,
}

impl CacheStrategy {
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url)?;
        let manager = client.get_tokio_connection_manager().await?;

        Ok(Self {
            redis: manager,
            ttl_seconds: 3600,
            similarity_threshold: 0.95,
        })
    }

    /// Check cache for exact match
    pub async fn check_exact_match(
        &mut self,
        prompt: &str
    ) -> Result<Option<String>> {
        // Normalize prompt
        let normalized = self.normalize_prompt(prompt);

        // Generate cache key
        let key = self.generate_cache_key(&normalized);

        // Look up in Redis
        let cached: Option<String> = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut self.redis)
            .await?;

        if cached.is_some() {
            tracing::info!("Cache HIT: {}", key);
        } else {
            tracing::info!("Cache MISS: {}", key);
        }

        Ok(cached)
    }

    /// Store response in cache
    pub async fn store(
        &mut self,
        prompt: &str,
        response: &str
    ) -> Result<()> {
        let normalized = self.normalize_prompt(prompt);
        let key = self.generate_cache_key(&normalized);

        // Store with TTL
        redis::cmd("SETEX")
            .arg(&key)
            .arg(self.ttl_seconds)
            .arg(response)
            .query_async(&mut self.redis)
            .await?;

        tracing::info!("Stored in cache: {}", key);

        Ok(())
    }

    /// Normalize prompt for consistent caching
    fn normalize_prompt(&self, prompt: &str) -> String {
        prompt
            .trim()
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Generate cache key from normalized prompt
    fn generate_cache_key(&self, normalized: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(normalized.as_bytes());
        let hash = hasher.finalize();
        format!("llm:cache:{:x}", hash)
    }
}
```

#### Semantic Similarity Caching

```rust
use llm_auto_optimizer::decision::*;

pub struct SemanticCacheStrategy {
    embedding_model: EmbeddingModel,
    similarity_threshold: f64,
    cache: VectorCache,
}

impl SemanticCacheStrategy {
    pub async fn check_semantic_match(
        &mut self,
        prompt: &str
    ) -> Result<Option<(String, f64)>> {
        // Generate embedding for query
        let query_embedding = self.embedding_model
            .encode(prompt)
            .await?;

        // Search vector cache for similar prompts
        let candidates = self.cache
            .search(&query_embedding, 10)  // Top 10
            .await?;

        // Find best match above threshold
        for (cached_prompt, cached_response, similarity) in candidates {
            if similarity >= self.similarity_threshold {
                tracing::info!(
                    "Semantic cache HIT: similarity={:.3}",
                    similarity
                );
                return Ok(Some((cached_response, similarity)));
            }
        }

        tracing::info!("Semantic cache MISS");
        Ok(None)
    }

    pub async fn store_with_embedding(
        &mut self,
        prompt: &str,
        response: &str
    ) -> Result<()> {
        let embedding = self.embedding_model
            .encode(prompt)
            .await?;

        self.cache
            .insert(prompt, response, &embedding)
            .await?;

        Ok(())
    }
}
```

#### Multi-Level Caching

```rust
use llm_auto_optimizer::decision::*;

pub struct MultiLevelCache {
    l1_memory: LruCache<String, String>,
    l2_redis: ConnectionManager,
    l3_postgres: PgPool,
}

impl MultiLevelCache {
    pub async fn get(&mut self, key: &str) -> Result<Option<String>> {
        // L1: In-memory cache (fastest)
        if let Some(value) = self.l1_memory.get(key) {
            tracing::debug!("L1 cache HIT");
            return Ok(Some(value.clone()));
        }

        // L2: Redis cache (fast)
        if let Some(value) = self.get_from_redis(key).await? {
            tracing::debug!("L2 cache HIT");
            // Promote to L1
            self.l1_memory.put(key.to_string(), value.clone());
            return Ok(Some(value));
        }

        // L3: PostgreSQL cache (slower but persistent)
        if let Some(value) = self.get_from_postgres(key).await? {
            tracing::debug!("L3 cache HIT");
            // Promote to L2 and L1
            self.store_in_redis(key, &value).await?;
            self.l1_memory.put(key.to_string(), value.clone());
            return Ok(Some(value));
        }

        tracing::debug!("Cache MISS at all levels");
        Ok(None)
    }

    pub async fn set(&mut self, key: &str, value: &str) -> Result<()> {
        // Store at all levels
        self.l1_memory.put(key.to_string(), value.to_string());
        self.store_in_redis(key, value).await?;
        self.store_in_postgres(key, value).await?;
        Ok(())
    }

    async fn get_from_redis(&mut self, key: &str) -> Result<Option<String>> {
        Ok(redis::cmd("GET")
            .arg(key)
            .query_async(&mut self.l2_redis)
            .await?)
    }

    async fn store_in_redis(&mut self, key: &str, value: &str) -> Result<()> {
        redis::cmd("SETEX")
            .arg(key)
            .arg(3600)  // 1 hour TTL
            .arg(value)
            .query_async(&mut self.l2_redis)
            .await?;
        Ok(())
    }

    async fn get_from_postgres(&self, key: &str) -> Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT response FROM cache WHERE key = $1 AND expires_at > NOW()"
        )
        .bind(key)
        .fetch_optional(&self.l3_postgres)
        .await?;

        Ok(row.map(|r| r.0))
    }

    async fn store_in_postgres(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO cache (key, response, expires_at)
             VALUES ($1, $2, NOW() + INTERVAL '1 day')
             ON CONFLICT (key) DO UPDATE SET
                response = EXCLUDED.response,
                expires_at = EXCLUDED.expires_at"
        )
        .bind(key)
        .bind(value)
        .execute(&self.l3_postgres)
        .await?;

        Ok(())
    }
}
```

#### Context-Aware Caching

```rust
use llm_auto_optimizer::decision::*;

pub struct ContextAwareCache {
    cache: HashMap<String, CachedResponse>,
}

#[derive(Clone)]
struct CachedResponse {
    response: String,
    context: RequestContext,
    timestamp: SystemTime,
    ttl: Duration,
}

impl ContextAwareCache {
    /// Check if cached response is valid for current context
    pub fn get_for_context(
        &self,
        key: &str,
        context: &RequestContext
    ) -> Option<String> {
        if let Some(cached) = self.cache.get(key) {
            // Check if TTL expired
            if cached.timestamp.elapsed().ok()? > cached.ttl {
                return None;
            }

            // Check context compatibility
            if !self.is_context_compatible(&cached.context, context) {
                return None;
            }

            return Some(cached.response.clone());
        }

        None
    }

    /// Determine if contexts are compatible for cache reuse
    fn is_context_compatible(
        &self,
        cached: &RequestContext,
        current: &RequestContext
    ) -> bool {
        // Task type must match
        if cached.task_type != current.task_type {
            return false;
        }

        // Token length should be similar (within 20%)
        let token_diff = (cached.estimated_input_tokens as f64
            - current.estimated_input_tokens as f64).abs();
        let token_threshold = cached.estimated_input_tokens as f64 * 0.2;

        if token_diff > token_threshold {
            return false;
        }

        // Output length category should match
        if cached.output_length != current.output_length {
            return false;
        }

        true
    }

    pub fn store_with_context(
        &mut self,
        key: String,
        response: String,
        context: RequestContext,
        ttl: Duration
    ) {
        self.cache.insert(key, CachedResponse {
            response,
            context,
            timestamp: SystemTime::now(),
            ttl,
        });
    }
}
```

### Cache Metrics

Track these metrics to optimize caching strategy:

```rust
pub struct CacheMetrics {
    pub hits: AtomicU64,
    pub misses: AtomicU64,
    pub stores: AtomicU64,
    pub evictions: AtomicU64,
    pub total_latency_saved_ms: AtomicU64,
}

impl CacheMetrics {
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed) as f64;
        let total = hits + self.misses.load(Ordering::Relaxed) as f64;
        if total > 0.0 {
            hits / total
        } else {
            0.0
        }
    }

    pub fn average_latency_saved(&self) -> f64 {
        let total_saved = self.total_latency_saved_ms.load(Ordering::Relaxed) as f64;
        let hits = self.hits.load(Ordering::Relaxed) as f64;
        if hits > 0.0 {
            total_saved / hits
        } else {
            0.0
        }
    }

    pub fn report(&self) {
        println!("Cache Metrics:");
        println!("  Hit rate: {:.2}%", self.hit_rate() * 100.0);
        println!("  Hits: {}", self.hits.load(Ordering::Relaxed));
        println!("  Misses: {}", self.misses.load(Ordering::Relaxed));
        println!("  Stores: {}", self.stores.load(Ordering::Relaxed));
        println!("  Evictions: {}", self.evictions.load(Ordering::Relaxed));
        println!("  Avg latency saved: {:.0}ms", self.average_latency_saved());
    }
}
```

### Best Practices

1. **Set Appropriate TTLs**: Balance freshness vs hit rate (1-24 hours typical)
2. **Use Semantic Similarity Carefully**: 0.95+ threshold recommended to avoid incorrect matches
3. **Normalize Inputs**: Consistent normalization improves exact match hit rate
4. **Monitor Hit Rates**: Target 30-60% hit rate depending on use case
5. **Implement Warming**: Pre-populate cache with common queries
6. **Use Multi-Level Caching**: Memory → Redis → Postgres for optimal performance
7. **Handle Cache Failures Gracefully**: Always fall through to actual LLM call
8. **Exclude Sensitive Data**: Don't cache PII or confidential information

---

## Strategy 3: Rate Limiting Strategy

### Overview

The **Rate Limiting Strategy** controls the flow of requests to LLM APIs to prevent overload, manage costs, and ensure fair usage across users/tenants. It implements token bucket, sliding window, and adaptive rate limiting algorithms.

### When to Use

✅ **Use Rate Limiting when:**
- You need to control costs by capping requests
- API providers have rate limits
- You want to prevent system overload
- Multi-tenant systems need fair allocation
- Burst traffic needs to be smoothed

❌ **Don't use when:**
- You have unlimited quota
- All requests are equally critical
- No cost constraints exist

### How It Works

```
┌─────────────────────────────────────────────────────────┐
│           Rate Limiting Strategy Flow                    │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  1. Request Classification                              │
│     ├─ Identify user/tenant                             │
│     ├─ Determine priority level                         │
│     └─ Calculate request weight                         │
│                                                          │
│  2. Rate Limit Check                                    │
│     ├─ Check global rate limit                          │
│     ├─ Check per-user limit                             │
│     ├─ Check per-tenant limit                           │
│     └─ Check per-endpoint limit                         │
│                                                          │
│  3. Token Bucket Algorithm                              │
│     ├─ Refill tokens at configured rate                 │
│     ├─ Check if tokens available                        │
│     ├─ Consume tokens for request                       │
│     └─ Update bucket state                              │
│                                                          │
│  4. Decision                                            │
│     ├─ ALLOW → Process request                          │
│     ├─ DENY → Return 429 Too Many Requests              │
│     └─ QUEUE → Add to priority queue                    │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

### Configuration

```yaml
# config.yaml - Rate Limiting Strategy
strategies:
  rate_limiting:
    enabled: true

    # Global limits
    global:
      requests_per_second: 100
      requests_per_minute: 5000
      requests_per_hour: 200000
      tokens_per_minute: 1000000

    # Per-user limits
    per_user:
      requests_per_second: 10
      requests_per_minute: 500
      tokens_per_minute: 100000
      burst_size: 20  # Allow bursts up to 20

    # Per-tenant limits
    per_tenant:
      requests_per_second: 50
      requests_per_minute: 2500
      tokens_per_minute: 500000

    # Algorithm
    algorithm: "token_bucket"  # token_bucket, sliding_window, or adaptive

    # Token bucket settings
    token_bucket:
      refill_rate: 10.0         # Tokens per second
      bucket_capacity: 100      # Maximum tokens
      cost_per_request: 1       # Tokens consumed per request

    # Adaptive rate limiting
    adaptive:
      enabled: true
      target_error_rate: 0.01   # 1% error rate threshold
      adjustment_period_seconds: 60
      min_rate: 10              # Minimum requests/second
      max_rate: 1000            # Maximum requests/second

    # Priority classes
    priorities:
      critical: 1.0             # Weight multiplier
      high: 0.75
      medium: 0.5
      low: 0.25

    # Behavior
    on_limit_exceeded:
      action: "queue"           # deny, queue, or degraded
      queue_timeout_ms: 5000    # Max wait in queue
      retry_after_seconds: 60   # Retry-After header value
```

### Code Examples

#### Token Bucket Rate Limiter

```rust
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::{Duration, Instant};

pub struct TokenBucket {
    capacity: f64,
    tokens: Arc<Mutex<f64>>,
    refill_rate: f64,  // tokens per second
    last_refill: Arc<Mutex<Instant>>,
}

impl TokenBucket {
    pub fn new(capacity: f64, refill_rate: f64) -> Self {
        Self {
            capacity,
            tokens: Arc::new(Mutex::new(capacity)),
            refill_rate,
            last_refill: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// Try to consume tokens, return true if allowed
    pub async fn try_consume(&self, tokens_needed: f64) -> bool {
        // Refill tokens based on time elapsed
        self.refill().await;

        let mut tokens = self.tokens.lock().await;

        if *tokens >= tokens_needed {
            *tokens -= tokens_needed;
            true
        } else {
            false
        }
    }

    /// Refill tokens based on elapsed time
    async fn refill(&self) {
        let mut last_refill = self.last_refill.lock().await;
        let now = Instant::now();
        let elapsed = now.duration_since(*last_refill).as_secs_f64();

        let mut tokens = self.tokens.lock().await;
        let new_tokens = elapsed * self.refill_rate;
        *tokens = (*tokens + new_tokens).min(self.capacity);

        *last_refill = now;
    }

    /// Get current token count
    pub async fn available_tokens(&self) -> f64 {
        self.refill().await;
        *self.tokens.lock().await
    }
}
```

#### Sliding Window Rate Limiter

```rust
use std::collections::VecDeque;
use std::time::{Duration, Instant};

pub struct SlidingWindowLimiter {
    window_size: Duration,
    max_requests: usize,
    requests: Mutex<VecDeque<Instant>>,
}

impl SlidingWindowLimiter {
    pub fn new(window_size: Duration, max_requests: usize) -> Self {
        Self {
            window_size,
            max_requests,
            requests: Mutex::new(VecDeque::new()),
        }
    }

    pub async fn is_allowed(&self) -> bool {
        let mut requests = self.requests.lock().await;
        let now = Instant::now();
        let window_start = now - self.window_size;

        // Remove requests outside window
        while let Some(&front) = requests.front() {
            if front < window_start {
                requests.pop_front();
            } else {
                break;
            }
        }

        // Check if we're under limit
        if requests.len() < self.max_requests {
            requests.push_back(now);
            true
        } else {
            false
        }
    }

    pub async fn current_count(&self) -> usize {
        let requests = self.requests.lock().await;
        requests.len()
    }
}
```

#### Adaptive Rate Limiter

```rust
use std::sync::atomic::{AtomicU64, Ordering};

pub struct AdaptiveRateLimiter {
    current_rate: Arc<AtomicU64>,  // requests per second * 1000
    min_rate: f64,
    max_rate: f64,
    target_error_rate: f64,

    total_requests: Arc<AtomicU64>,
    failed_requests: Arc<AtomicU64>,

    adjustment_interval: Duration,
    last_adjustment: Arc<Mutex<Instant>>,
}

impl AdaptiveRateLimiter {
    pub fn new(
        initial_rate: f64,
        min_rate: f64,
        max_rate: f64,
        target_error_rate: f64
    ) -> Self {
        Self {
            current_rate: Arc::new(AtomicU64::new((initial_rate * 1000.0) as u64)),
            min_rate,
            max_rate,
            target_error_rate,
            total_requests: Arc::new(AtomicU64::new(0)),
            failed_requests: Arc::new(AtomicU64::new(0)),
            adjustment_interval: Duration::from_secs(60),
            last_adjustment: Arc::new(Mutex::new(Instant::now())),
        }
    }

    pub async fn try_acquire(&self) -> bool {
        self.adjust_rate().await;

        let rate = self.current_rate.load(Ordering::Relaxed) as f64 / 1000.0;
        let bucket = TokenBucket::new(rate * 2.0, rate);

        bucket.try_consume(1.0).await
    }

    pub fn record_success(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_failure(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.failed_requests.fetch_add(1, Ordering::Relaxed);
    }

    async fn adjust_rate(&self) {
        let mut last_adjustment = self.last_adjustment.lock().await;

        if last_adjustment.elapsed() < self.adjustment_interval {
            return;
        }

        let total = self.total_requests.load(Ordering::Relaxed);
        let failed = self.failed_requests.load(Ordering::Relaxed);

        if total < 10 {
            return;  // Not enough data
        }

        let error_rate = failed as f64 / total as f64;
        let current_rate = self.current_rate.load(Ordering::Relaxed) as f64 / 1000.0;

        let new_rate = if error_rate > self.target_error_rate {
            // Too many errors, decrease rate
            (current_rate * 0.9).max(self.min_rate)
        } else {
            // Doing well, increase rate
            (current_rate * 1.1).min(self.max_rate)
        };

        self.current_rate.store((new_rate * 1000.0) as u64, Ordering::Relaxed);

        // Reset counters
        self.total_requests.store(0, Ordering::Relaxed);
        self.failed_requests.store(0, Ordering::Relaxed);
        *last_adjustment = Instant::now();

        tracing::info!(
            "Adjusted rate: {:.1} → {:.1} req/s (error rate: {:.2}%)",
            current_rate, new_rate, error_rate * 100.0
        );
    }

    pub fn current_rate(&self) -> f64 {
        self.current_rate.load(Ordering::Relaxed) as f64 / 1000.0
    }
}
```

#### Multi-Tier Rate Limiter

```rust
pub struct MultiTierRateLimiter {
    global: Arc<TokenBucket>,
    per_user: Arc<DashMap<String, TokenBucket>>,
    per_tenant: Arc<DashMap<String, TokenBucket>>,
}

impl MultiTierRateLimiter {
    pub fn new(
        global_rate: f64,
        user_rate: f64,
        tenant_rate: f64
    ) -> Self {
        Self {
            global: Arc::new(TokenBucket::new(global_rate * 2.0, global_rate)),
            per_user: Arc::new(DashMap::new()),
            per_tenant: Arc::new(DashMap::new()),
        }
    }

    pub async fn check_limit(
        &self,
        user_id: &str,
        tenant_id: &str
    ) -> Result<(), RateLimitError> {
        // Check global limit first
        if !self.global.try_consume(1.0).await {
            return Err(RateLimitError::GlobalLimit);
        }

        // Check per-tenant limit
        let tenant_bucket = self.per_tenant
            .entry(tenant_id.to_string())
            .or_insert_with(|| TokenBucket::new(100.0, 50.0));

        if !tenant_bucket.try_consume(1.0).await {
            return Err(RateLimitError::TenantLimit(tenant_id.to_string()));
        }

        // Check per-user limit
        let user_bucket = self.per_user
            .entry(user_id.to_string())
            .or_insert_with(|| TokenBucket::new(20.0, 10.0));

        if !user_bucket.try_consume(1.0).await {
            return Err(RateLimitError::UserLimit(user_id.to_string()));
        }

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RateLimitError {
    #[error("Global rate limit exceeded")]
    GlobalLimit,

    #[error("Tenant rate limit exceeded: {0}")]
    TenantLimit(String),

    #[error("User rate limit exceeded: {0}")]
    UserLimit(String),
}
```

#### Priority-Based Rate Limiting

```rust
pub enum Priority {
    Critical = 4,
    High = 3,
    Medium = 2,
    Low = 1,
}

pub struct PriorityRateLimiter {
    buckets: HashMap<Priority, TokenBucket>,
    queue: Arc<Mutex<BinaryHeap<PriorityRequest>>>,
}

#[derive(Eq, PartialEq)]
struct PriorityRequest {
    priority: Priority,
    timestamp: Instant,
    request_id: Uuid,
}

impl Ord for PriorityRequest {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority.cmp(&other.priority)
            .then_with(|| other.timestamp.cmp(&self.timestamp))
    }
}

impl PriorityRateLimiter {
    pub async fn try_acquire(&self, priority: Priority) -> Result<()> {
        if let Some(bucket) = self.buckets.get(&priority) {
            if bucket.try_consume(1.0).await {
                return Ok(());
            }
        }

        // Add to queue
        let mut queue = self.queue.lock().await;
        queue.push(PriorityRequest {
            priority,
            timestamp: Instant::now(),
            request_id: Uuid::new_v4(),
        });

        Err(anyhow::anyhow!("Rate limited, added to queue"))
    }

    pub async fn process_queue(&self) {
        let mut queue = self.queue.lock().await;

        while let Some(request) = queue.pop() {
            if let Some(bucket) = self.buckets.get(&request.priority) {
                if bucket.try_consume(1.0).await {
                    // Process request
                    tracing::info!(
                        "Processing queued request {} with priority {:?}",
                        request.request_id,
                        request.priority
                    );
                } else {
                    // Put back in queue
                    queue.push(request);
                    break;
                }
            }
        }
    }
}
```

### Best Practices

1. **Set Conservative Limits**: Start with 80% of API provider limits
2. **Use Multi-Tier Limiting**: Global, tenant, and user limits
3. **Implement Priority Classes**: Critical requests get preferential treatment
4. **Monitor Rejection Rates**: Target <1% rejection rate
5. **Use Adaptive Limiting**: Auto-adjust based on error rates
6. **Provide Clear Feedback**: Return Retry-After headers
7. **Queue Non-Critical Requests**: Don't immediately reject low-priority requests
8. **Handle Bursts**: Allow temporary bursts with token bucket

---

## Strategy 4: Request Batching Strategy

### Overview

The **Request Batching Strategy** groups multiple requests together to improve throughput, reduce per-request overhead, and optimize token usage. It's particularly effective for high-volume, low-latency scenarios.

### When to Use

✅ **Use Request Batching when:**
- You have high request volumes (>100/sec)
- Requests can tolerate small latency increases (50-200ms)
- Processing similar requests concurrently
- API supports batch endpoints
- Need to maximize throughput

❌ **Don't use when:**
- Real-time responses required
- Each request is unique and unrelated
- Batch API not available
- Latency is critical (< 50ms requirement)

### How It Works

```
┌─────────────────────────────────────────────────────────┐
│           Request Batching Strategy Flow                 │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  1. Request Buffering                                   │
│     ├─ Accumulate requests in buffer                    │
│     ├─ Group by similarity (embeddings, task type)      │
│     ├─ Apply batching window (time-based)               │
│     └─ Check batch size limits                          │
│                                                          │
│  2. Batch Formation                                     │
│     ├─ Determine optimal batch size                     │
│     ├─ Pack requests into batch                         │
│     ├─ Add batch metadata                               │
│     └─ Preserve request order/IDs                       │
│                                                          │
│  3. Batch Execution                                     │
│     ├─ Send batch to LLM API                            │
│     ├─ Parallel processing where possible               │
│     ├─ Handle partial failures                          │
│     └─ Collect all responses                            │
│                                                          │
│  4. Response Distribution                               │
│     ├─ Map responses to original requests               │
│     ├─ Return individual results                        │
│     ├─ Update metrics (latency, cost)                   │
│     └─ Log batch statistics                             │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

### Configuration

```yaml
# config.yaml - Request Batching Strategy
strategies:
  batching:
    enabled: true

    # Batch size constraints
    min_batch_size: 2
    max_batch_size: 10
    target_batch_size: 5

    # Timing
    max_wait_ms: 100            # Maximum time to wait for batch
    flush_interval_ms: 50       # Periodic flush interval

    # Grouping strategy
    grouping:
      enabled: true
      strategy: "similarity"     # similarity, task_type, or none
      similarity_threshold: 0.85 # For similarity-based grouping

    # Token optimization
    token_packing:
      enabled: true
      max_total_tokens: 8000     # Maximum tokens per batch
      reserve_tokens: 500        # Reserve for batch overhead

    # Concurrency
    max_concurrent_batches: 10
    batch_executor_threads: 4

    # Retry behavior
    retry_failed_items: true
    max_retries_per_item: 2

    # Metrics
    track_batch_efficiency: true
    log_batch_statistics: true
```

### Code Examples

#### Basic Request Batcher

```rust
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration, Instant};

pub struct RequestBatcher<T> {
    sender: mpsc::UnboundedSender<BatchItem<T>>,
    max_batch_size: usize,
    max_wait: Duration,
}

struct BatchItem<T> {
    request: T,
    response_tx: oneshot::Sender<Result<String>>,
}

impl<T: Clone + Send + 'static> RequestBatcher<T> {
    pub fn new(
        max_batch_size: usize,
        max_wait: Duration,
        executor: impl Fn(Vec<T>) -> BoxFuture<'static, Result<Vec<String>>> + Send + 'static
    ) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel();

        // Spawn batch processor
        tokio::spawn(async move {
            let mut buffer: Vec<BatchItem<T>> = Vec::new();
            let mut last_flush = Instant::now();

            loop {
                tokio::select! {
                    Some(item) = rx.recv() => {
                        buffer.push(item);

                        // Flush if batch is full
                        if buffer.len() >= max_batch_size {
                            Self::flush_batch(&mut buffer, &executor).await;
                            last_flush = Instant::now();
                        }
                    }

                    _ = sleep(max_wait), if !buffer.is_empty() => {
                        // Flush if max wait time exceeded
                        if last_flush.elapsed() >= max_wait {
                            Self::flush_batch(&mut buffer, &executor).await;
                            last_flush = Instant::now();
                        }
                    }
                }
            }
        });

        Self {
            sender: tx,
            max_batch_size,
            max_wait,
        }
    }

    async fn flush_batch<F>(
        buffer: &mut Vec<BatchItem<T>>,
        executor: &F
    ) where
        F: Fn(Vec<T>) -> BoxFuture<'static, Result<Vec<String>>>
    {
        if buffer.is_empty() {
            return;
        }

        let batch: Vec<_> = buffer.drain(..).collect();
        let requests: Vec<T> = batch.iter()
            .map(|item| item.request.clone())
            .collect();

        tracing::info!("Executing batch of {} requests", requests.len());

        match executor(requests).await {
            Ok(responses) => {
                for (item, response) in batch.into_iter().zip(responses) {
                    let _ = item.response_tx.send(Ok(response));
                }
            }
            Err(e) => {
                for item in batch {
                    let _ = item.response_tx.send(Err(anyhow::anyhow!("{}", e)));
                }
            }
        }
    }

    pub async fn submit(&self, request: T) -> Result<String> {
        let (tx, rx) = oneshot::channel();

        self.sender.send(BatchItem {
            request,
            response_tx: tx,
        })?;

        rx.await?
    }
}
```

#### Similarity-Based Batching

```rust
pub struct SimilarityBatcher {
    embedding_model: EmbeddingModel,
    similarity_threshold: f64,
    max_batch_size: usize,
}

impl SimilarityBatcher {
    pub async fn group_by_similarity(
        &self,
        requests: Vec<Request>
    ) -> Vec<Vec<Request>> {
        if requests.is_empty() {
            return vec![];
        }

        // Generate embeddings for all requests
        let embeddings = self.embedding_model
            .batch_encode(&requests.iter().map(|r| &r.prompt).collect::<Vec<_>>())
            .await?;

        // Cluster similar requests
        let mut batches: Vec<Vec<Request>> = Vec::new();
        let mut used = vec![false; requests.len()];

        for i in 0..requests.len() {
            if used[i] {
                continue;
            }

            let mut batch = vec![requests[i].clone()];
            used[i] = true;

            // Find similar requests
            for j in (i + 1)..requests.len() {
                if used[j] || batch.len() >= self.max_batch_size {
                    break;
                }

                let similarity = cosine_similarity(&embeddings[i], &embeddings[j]);

                if similarity >= self.similarity_threshold {
                    batch.push(requests[j].clone());
                    used[j] = true;
                }
            }

            batches.push(batch);
        }

        batches
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    (dot / (norm_a * norm_b)) as f64
}
```

#### Token-Aware Batching

```rust
pub struct TokenAwareBatcher {
    max_total_tokens: usize,
    reserve_tokens: usize,
    tokenizer: Tokenizer,
}

impl TokenAwareBatcher {
    pub fn create_batches(&self, requests: Vec<Request>) -> Vec<Vec<Request>> {
        let mut batches = Vec::new();
        let mut current_batch = Vec::new();
        let mut current_tokens = self.reserve_tokens;

        for request in requests {
            let request_tokens = self.tokenizer.encode(&request.prompt).len();

            // Check if adding this request would exceed limit
            if current_tokens + request_tokens > self.max_total_tokens {
                if !current_batch.is_empty() {
                    batches.push(current_batch);
                }
                current_batch = vec![request];
                current_tokens = self.reserve_tokens + request_tokens;
            } else {
                current_batch.push(request);
                current_tokens += request_tokens;
            }
        }

        if !current_batch.is_empty() {
            batches.push(current_batch);
        }

        batches
    }
}
```

#### Adaptive Batch Size

```rust
pub struct AdaptiveBatcher {
    min_batch_size: usize,
    max_batch_size: usize,
    current_batch_size: Arc<AtomicUsize>,

    // Performance metrics
    avg_latency_ms: Arc<AtomicU64>,
    avg_throughput: Arc<AtomicU64>,
}

impl AdaptiveBatcher {
    pub fn adjust_batch_size(&self, latency_ms: u64, throughput: f64) {
        let current = self.current_batch_size.load(Ordering::Relaxed);

        // Update moving averages
        let old_latency = self.avg_latency_ms.load(Ordering::Relaxed);
        let new_latency = ((old_latency as f64 * 0.9) + (latency_ms as f64 * 0.1)) as u64;
        self.avg_latency_ms.store(new_latency, Ordering::Relaxed);

        let old_throughput = self.avg_throughput.load(Ordering::Relaxed);
        let new_throughput = ((old_throughput as f64 * 0.9) + (throughput * 0.1)) as u64;
        self.avg_throughput.store(new_throughput, Ordering::Relaxed);

        // Adjust batch size based on performance
        let new_size = if new_latency > 200 && current > self.min_batch_size {
            // Latency too high, decrease batch size
            current - 1
        } else if new_latency < 100 && current < self.max_batch_size {
            // Latency acceptable, increase batch size
            current + 1
        } else {
            current
        };

        self.current_batch_size.store(new_size, Ordering::Relaxed);

        tracing::debug!(
            "Adjusted batch size: {} → {} (latency: {}ms, throughput: {:.1}/s)",
            current, new_size, new_latency, new_throughput
        );
    }

    pub fn optimal_batch_size(&self) -> usize {
        self.current_batch_size.load(Ordering::Relaxed)
    }
}
```

### Batch Metrics

```rust
pub struct BatchMetrics {
    pub total_batches: AtomicU64,
    pub total_requests: AtomicU64,
    pub total_batch_latency_ms: AtomicU64,
    pub total_item_latency_ms: AtomicU64,
}

impl BatchMetrics {
    pub fn average_batch_size(&self) -> f64 {
        let batches = self.total_batches.load(Ordering::Relaxed) as f64;
        let requests = self.total_requests.load(Ordering::Relaxed) as f64;
        if batches > 0.0 {
            requests / batches
        } else {
            0.0
        }
    }

    pub fn average_batch_latency(&self) -> f64 {
        let total_latency = self.total_batch_latency_ms.load(Ordering::Relaxed) as f64;
        let batches = self.total_batches.load(Ordering::Relaxed) as f64;
        if batches > 0.0 {
            total_latency / batches
        } else {
            0.0
        }
    }

    pub fn average_item_latency(&self) -> f64 {
        let total_latency = self.total_item_latency_ms.load(Ordering::Relaxed) as f64;
        let requests = self.total_requests.load(Ordering::Relaxed) as f64;
        if requests > 0.0 {
            total_latency / requests
        } else {
            0.0
        }
    }

    pub fn batching_efficiency(&self) -> f64 {
        let avg_batch_latency = self.average_batch_latency();
        let avg_item_latency = self.average_item_latency();
        if avg_item_latency > 0.0 {
            1.0 - (avg_batch_latency / avg_item_latency)
        } else {
            0.0
        }
    }
}
```

### Best Practices

1. **Start with Conservative Batch Sizes**: Begin with 2-5, adjust based on metrics
2. **Set Appropriate Wait Times**: Balance latency (50-100ms typical) vs throughput
3. **Group Similar Requests**: Improve cache efficiency with similarity-based batching
4. **Monitor Batch Efficiency**: Target >30% latency reduction per request
5. **Handle Partial Failures**: Retry individual items on batch failure
6. **Use Adaptive Sizing**: Auto-adjust batch size based on performance
7. **Consider Token Limits**: Pack batches to stay within model context limits
8. **Implement Timeouts**: Don't wait indefinitely for batch to fill

---

## Strategy 5: Prompt Optimization Strategy

### Overview

The **Prompt Optimization Strategy** uses A/B testing and reinforcement learning to continuously improve prompt templates, achieving better quality and lower token usage through data-driven experimentation.

### When to Use

✅ **Use Prompt Optimization when:**
- You have standardized prompt templates
- Quality improvements are measurable
- Sufficient traffic for A/B tests (>1000 samples/day)
- Prompt engineering is important to your use case
- You want to reduce token usage

❌ **Don't use when:**
- Every prompt is unique
- Low traffic volume
- No metrics to measure quality
- Prompts are simple and already optimal

### How It Works

```
┌─────────────────────────────────────────────────────────┐
│         Prompt Optimization Strategy Flow                │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  1. Variant Generation                                  │
│     ├─ Create prompt variations                         │
│     ├─ Apply transformation strategies                  │
│     ├─ Validate syntax and structure                    │
│     └─ Register experiment                              │
│                                                          │
│  2. A/B Test Execution                                  │
│     ├─ Assign users to variants (Thompson Sampling)     │
│     ├─ Track quality, cost, latency                     │
│     ├─ Calculate statistical significance               │
│     └─ Continue until p < 0.05 or timeout               │
│                                                          │
│  3. Winner Selection                                    │
│     ├─ Identify statistically significant winner        │
│     ├─ Compare against baseline                         │
│     ├─ Validate improvement is meaningful               │
│     └─ Graduate winner to production                    │
│                                                          │
│  4. Continuous Improvement                              │
│     ├─ Generate new variants from winner                │
│     ├─ Start new experiment                             │
│     ├─ Build on previous learnings                      │
│     └─ Iterate continuously                             │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

### Configuration

```yaml
# config.yaml - Prompt Optimization Strategy
strategies:
  prompt_optimization:
    enabled: true

    # A/B testing configuration
    ab_testing:
      min_sample_size: 100          # Minimum samples per variant
      significance_level: 0.05      # p-value threshold
      max_duration_seconds: 604800  # 7 days
      power: 0.8                    # Statistical power

    # Variant generation
    variant_generation:
      strategies:
        - "conciseness"             # Reduce token count
        - "clarity"                 # Improve clarity
        - "specificity"             # Add specificity
        - "examples"                # Include examples
        - "structure"               # Improve structure

      max_variants_per_experiment: 4

    # Thompson Sampling
    thompson_sampling:
      enabled: true
      alpha: 1.0                    # Prior successes
      beta: 1.0                     # Prior failures

    # Quality metrics
    metrics:
      primary: "quality_score"       # Primary metric to optimize
      secondary:
        - "cost"
        - "latency"
        - "token_usage"

      quality_weights:
        task_completion: 0.4
        user_satisfaction: 0.3
        correctness: 0.3

    # Graduation criteria
    graduation:
      min_improvement: 0.05         # 5% minimum improvement
      min_confidence: 0.95          # 95% confidence required
      require_cost_neutral: false   # Don't require cost to stay same
```

### Code Examples

#### Basic A/B Test for Prompts

```rust
use llm_auto_optimizer::decision::*;
use llm_optimizer_config::OptimizerConfig;
use llm_optimizer_types::models::ModelConfig;

// Initialize A/B testing engine
let config = OptimizerConfig::from_file("config.yaml")?;
let ab_engine = ABTestEngine::new(&config);

// Define base configuration
let base_config = ModelConfig {
    provider: "openai".to_string(),
    model: "gpt-4-turbo".to_string(),
    temperature: 0.7,
    max_tokens: 1024,
    ..Default::default()
};

// Create variants with different prompts
let control_variant = Variant::new(
    "control",
    base_config.clone(),
    0.5  // 50% traffic
);

let mut treatment_config = base_config.clone();
treatment_config.system_prompt = Some(
    "You are a helpful assistant. Please provide clear, concise answers.".to_string()
);

let treatment_variant = Variant::new(
    "treatment_concise",
    treatment_config,
    0.5  // 50% traffic
);

// Create experiment
let experiment_id = ab_engine.create_experiment(
    "prompt_conciseness_test",
    vec![control_variant, treatment_variant]
)?;

// Start experiment
ab_engine.start(&experiment_id)?;

println!("Started A/B test: {}", experiment_id);
```

#### Prompt Variant Generation

```rust
use llm_auto_optimizer::decision::*;

pub struct PromptVariantGenerator;

impl PromptVariantGenerator {
    /// Generate variants using different strategies
    pub fn generate_variants(
        base_prompt: &str,
        strategies: &[VariantStrategy]
    ) -> Result<Vec<String>> {
        let mut variants = vec![base_prompt.to_string()];  // Include original

        for strategy in strategies {
            match strategy {
                VariantStrategy::Conciseness => {
                    variants.push(Self::make_concise(base_prompt));
                }
                VariantStrategy::Clarity => {
                    variants.push(Self::improve_clarity(base_prompt));
                }
                VariantStrategy::Specificity => {
                    variants.push(Self::add_specificity(base_prompt));
                }
                VariantStrategy::Examples => {
                    variants.push(Self::add_examples(base_prompt));
                }
                VariantStrategy::Structure => {
                    variants.push(Self::improve_structure(base_prompt));
                }
            }
        }

        Ok(variants)
    }

    fn make_concise(prompt: &str) -> String {
        // Remove unnecessary words, combine sentences
        prompt
            .replace("Please ", "")
            .replace("kindly ", "")
            .replace("I would like you to ", "")
            .trim()
            .to_string()
    }

    fn improve_clarity(prompt: &str) -> String {
        // Add clear instructions and structure
        format!(
            "Task: {}\n\nInstructions:\n1. Read the task carefully\n2. Provide a clear response\n3. Be specific and accurate",
            prompt
        )
    }

    fn add_specificity(prompt: &str) -> String {
        // Add constraints and requirements
        format!(
            "{}\n\nRequirements:\n\
             - Be specific and detailed\n\
             - Use concrete examples\n\
             - Provide step-by-step explanations",
            prompt
        )
    }

    fn add_examples(prompt: &str) -> String {
        // Include few-shot examples
        format!(
            "{}\n\nExample 1:\nInput: ...\nOutput: ...\n\n\
             Example 2:\nInput: ...\nOutput: ...\n\n\
             Now complete the task:",
            prompt
        )
    }

    fn improve_structure(prompt: &str) -> String {
        // Add clear sections and formatting
        format!(
            "# Task\n{}\n\n# Output Format\n\
             Please provide your response in the following format:\n\
             - Summary:\n- Details:\n- Conclusion:",
            prompt
        )
    }
}
```

#### Running and Monitoring Experiments

```rust
use llm_auto_optimizer::decision::*;

pub async fn run_prompt_experiment(
    ab_engine: &ABTestEngine,
    experiment_id: &Uuid
) -> Result<()> {
    // Simulate traffic
    for i in 0..1000 {
        // Get variant assignment
        let (variant_id, config) = ab_engine.assign_variant(experiment_id)?;

        // Execute request (simulated)
        let (success, quality, cost, latency) = execute_llm_request(&config).await?;

        // Record outcome
        ab_engine.record_outcome(
            experiment_id,
            &variant_id,
            success,
            quality,
            cost,
            latency
        )?;

        if i % 100 == 0 {
            // Check statistics periodically
            let stats = ab_engine.get_statistics(experiment_id)?;
            println!("Progress: {}/{} samples", i, 1000);
            println!("  Variants: {}", stats.variant_stats.len());

            for variant_stat in &stats.variant_stats {
                println!("    {}: quality={:.2}, samples={}",
                    variant_stat.variant_name,
                    variant_stat.mean_quality,
                    variant_stat.sample_count
                );
            }
        }
    }

    Ok(())
}

async fn execute_llm_request(
    config: &ModelConfig
) -> Result<(bool, f64, f64, f64)> {
    // Execute actual LLM request
    // Returns: (success, quality, cost, latency_ms)

    // Simulated for example
    Ok((
        true,
        rand::random::<f64>() * 0.5 + 0.5,  // quality 0.5-1.0
        0.01,  // cost
        1234.0  // latency
    ))
}
```

#### Automated Experiment Management

```rust
use llm_auto_optimizer::decision::*;

pub struct PromptOptimizationManager {
    ab_engine: ABTestEngine,
    active_experiments: Arc<DashMap<String, Uuid>>,  // template_name -> experiment_id
    best_prompts: Arc<DashMap<String, String>>,  // template_name -> best_prompt
}

impl PromptOptimizationManager {
    pub fn new(config: &OptimizerConfig) -> Self {
        Self {
            ab_engine: ABTestEngine::new(config),
            active_experiments: Arc::new(DashMap::new()),
            best_prompts: Arc::new(DashMap::new()),
        }
    }

    /// Start optimization for a prompt template
    pub async fn start_optimization(
        &self,
        template_name: &str,
        base_prompt: &str,
        base_config: &ModelConfig
    ) -> Result<Uuid> {
        // Generate variants
        let strategies = vec![
            VariantStrategy::Conciseness,
            VariantStrategy::Clarity,
            VariantStrategy::Specificity,
        ];

        let prompts = PromptVariantGenerator::generate_variants(
            base_prompt,
            &strategies
        )?;

        // Create experiment variants
        let variants: Vec<Variant> = prompts
            .into_iter()
            .enumerate()
            .map(|(i, prompt)| {
                let mut config = base_config.clone();
                config.system_prompt = Some(prompt);

                Variant::new(
                    format!("variant_{}", i),
                    config,
                    1.0 / 4.0  // Equal distribution
                )
            })
            .collect();

        // Create and start experiment
        let exp_id = self.ab_engine.create_experiment(
            format!("optimize_{}", template_name),
            variants
        )?;

        self.ab_engine.start(&exp_id)?;
        self.active_experiments.insert(template_name.to_string(), exp_id);

        Ok(exp_id)
    }

    /// Check if experiments are complete and graduate winners
    pub async fn check_and_graduate(&self) -> Result<()> {
        for entry in self.active_experiments.iter() {
            let template_name = entry.key();
            let exp_id = entry.value();

            // Check if experiment should conclude
            let stats = self.ab_engine.get_statistics(exp_id)?;

            if stats.is_complete {
                // Get winner
                if let Some(winner) = stats.winner {
                    let experiment = self.ab_engine.conclude(exp_id)?;

                    // Get winning variant
                    let winning_variant = experiment.variants
                        .iter()
                        .find(|v| v.id == winner.variant_id)
                        .unwrap();

                    // Extract prompt
                    if let Some(prompt) = &winning_variant.config.system_prompt {
                        // Graduate to production
                        self.best_prompts.insert(
                            template_name.clone(),
                            prompt.clone()
                        );

                        tracing::info!(
                            "Graduated winning prompt for {}: improvement={:.1}%",
                            template_name,
                            winner.improvement * 100.0
                        );

                        // Start new experiment with winner as baseline
                        let new_base = winning_variant.config.clone();
                        self.start_optimization(template_name, prompt, &new_base).await?;
                    }
                }

                self.active_experiments.remove(template_name);
            }
        }

        Ok(())
    }

    /// Get current best prompt for a template
    pub fn get_best_prompt(&self, template_name: &str) -> Option<String> {
        self.best_prompts.get(template_name).map(|p| p.clone())
    }
}
```

#### Adaptive Parameter Tuning

```rust
use llm_auto_optimizer::decision::*;

// Initialize adaptive tuner
let mut tuner = AdaptiveParameterTuner::with_defaults();

// Register configurations for different task types
let code_config = ParameterConfig::code_generation();
let creative_config = ParameterConfig::creative();
let analytical_config = ParameterConfig::analytical();

let code_id = tuner.register_config(code_config)?;
let creative_id = tuner.register_config(creative_config)?;
let analytical_id = tuner.register_config(analytical_config)?;

// Select configuration based on context
let context = RequestContext::new(800)
    .with_task_type("code_generation");

let (config_id, config) = tuner.select_config(&context)?;

println!("Selected config for code generation:");
println!("  Temperature: {}", config.temperature);
println!("  Top-p: {}", config.top_p);
println!("  Max tokens: {}", config.max_tokens);

// After execution, provide feedback
let metrics = ResponseMetrics {
    quality_score: 0.93,
    cost: 0.05,
    latency_ms: 1500.0,
    token_count: 1200,
};

let feedback = UserFeedback {
    task_completed: true,
    explicit_rating: Some(5.0),
    ..Default::default()
};

// Calculate reward
let reward_calc = RewardCalculator::new(
    RewardWeights::balanced_weights(),
    1.0,
    5000.0
);
let reward = reward_calc.calculate_reward(&metrics, &feedback);

// Update tuner
tuner.update_config(&config_id, reward, &metrics, Some(&feedback))?;

// After many iterations, get improvement suggestions
if let Ok(improved_config) = tuner.suggest_improvement(&config_id) {
    println!("Suggested improvement:");
    println!("  Temperature: {} → {}",
        config.temperature,
        improved_config.temperature
    );
}
```

### Best Practices

1. **Define Clear Metrics**: Use measurable quality metrics (task completion, ratings)
2. **Run to Statistical Significance**: Don't stop tests early, wait for p < 0.05
3. **Test One Change at a Time**: Isolate variables for clear learnings
4. **Start with Small Changes**: Incremental improvements compound over time
5. **Use Thompson Sampling**: Better than random assignment for faster convergence
6. **Monitor All Metrics**: Watch for cost/latency degradation
7. **Document Learnings**: Build a knowledge base of what works
8. **Iterate Continuously**: Always be testing new variants

---

## Integration with Analyzer Engine

The Decision Engine works in tight integration with the Analyzer Engine to form a closed feedback loop.

### Architecture Integration

```
┌─────────────────────────────────────────────────────────┐
│          Analyzer ←→ Decision Integration                │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌────────────────────────────────────────────┐         │
│  │         Analyzer Engine                     │         │
│  │  • Performance metrics aggregation          │         │
│  │  • Cost analysis                            │         │
│  │  • Quality scoring                          │         │
│  │  • Drift detection                          │         │
│  │  • Anomaly detection                        │         │
│  └────────────────┬───────────────────────────┘         │
│                   │ Metrics Feed                         │
│                   ▼                                      │
│  ┌────────────────────────────────────────────┐         │
│  │      Decision Orchestrator                  │         │
│  │  • Context analysis                         │         │
│  │  • Strategy selection                       │         │
│  │  • Multi-objective optimization             │         │
│  └────────────────┬───────────────────────────┘         │
│                   │ Decisions                            │
│                   ▼                                      │
│  ┌────────────────────────────────────────────┐         │
│  │      Optimization Strategies                │         │
│  │  1. Model Selection                         │         │
│  │  2. Caching                                 │         │
│  │  3. Rate Limiting                           │         │
│  │  4. Request Batching                        │         │
│  │  5. Prompt Optimization                     │         │
│  └────────────────┬───────────────────────────┘         │
│                   │ Actions                              │
│                   ▼                                      │
│  ┌────────────────────────────────────────────┐         │
│  │        Actuator Engine                      │         │
│  │  • Apply configuration changes              │         │
│  │  • Canary deployments                       │         │
│  │  • Rollback on degradation                  │         │
│  └────────────────────────────────────────────┘         │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

### Integration Example

```rust
use llm_auto_optimizer::{
    analyzer::AnalyzerEngine,
    decision::DecisionEngine,
    processor::StreamProcessor,
};

pub struct IntegratedOptimizer {
    analyzer: Arc<AnalyzerEngine>,
    decision: Arc<DecisionEngine>,
    processor: Arc<StreamProcessor>,
}

impl IntegratedOptimizer {
    pub async fn optimize_request(
        &self,
        request: &Request
    ) -> Result<OptimizedRequest> {
        // 1. Analyze current performance
        let perf_metrics = self.analyzer
            .get_current_metrics()
            .await?;

        // 2. Check for anomalies
        let anomalies = self.analyzer
            .detect_anomalies(&perf_metrics)
            .await?;

        if !anomalies.is_empty() {
            tracing::warn!("Detected {} anomalies", anomalies.len());
        }

        // 3. Make optimization decisions
        let context = self.create_context(request, &perf_metrics)?;

        let model = self.decision
            .select_optimal_model(&context)
            .await?;

        let should_cache = self.decision
            .should_use_cache(request)
            .await?;

        let batch_decision = self.decision
            .should_batch(request, &context)
            .await?;

        // 4. Apply decisions
        Ok(OptimizedRequest {
            original: request.clone(),
            model,
            use_cache: should_cache,
            batch_with: batch_decision,
            priority: self.calculate_priority(request, &anomalies),
        })
    }
}
```

### Data Flow

```rust
// Metrics flow from Analyzer to Decision
pub struct MetricsFeed {
    analyzer: Arc<AnalyzerEngine>,
    decision: Arc<DecisionEngine>,
}

impl MetricsFeed {
    pub async fn stream_metrics(&self) -> Result<()> {
        let mut interval = tokio::time::interval(Duration::from_secs(10));

        loop {
            interval.tick().await;

            // Get latest metrics from analyzer
            let metrics = self.analyzer.get_metrics_snapshot().await?;

            // Update decision engine
            self.decision.update_metrics(&metrics).await?;

            // Check if decisions should be adjusted
            if metrics.needs_adjustment() {
                self.decision.trigger_reoptimization().await?;
            }
        }
    }
}
```

---

## Configuration Reference

### Complete Configuration File

```yaml
# LLM Auto Optimizer - Decision Engine Configuration

decision_engine:
  # Global settings
  enabled: true
  log_level: "info"

  # Strategy priority order (higher priority first)
  strategy_priority:
    - model_selection
    - caching
    - prompt_optimization
    - rate_limiting
    - batching

  # Model Selection Strategy
  model_selection:
    enabled: true

    weights:
      quality: 0.5
      cost: 0.3
      latency: 0.2

    constraints:
      max_cost_per_request: 1.0
      max_latency_p95_ms: 5000
      min_quality_score: 0.7

    providers:
      - openai
      - anthropic
      - google
      - meta
      - mistral

    algorithm: "linucb"
    exploration_factor: 1.0
    fallback_model: "gpt-4-turbo"

  # Caching Strategy
  caching:
    enabled: true
    backend: "redis"

    redis:
      host: "localhost"
      port: 6379
      db: 0

    exact_match:
      enabled: true
      normalize_whitespace: true

    semantic_match:
      enabled: true
      similarity_threshold: 0.95

    default_ttl_seconds: 3600
    max_cache_size_mb: 1024

  # Rate Limiting Strategy
  rate_limiting:
    enabled: true

    global:
      requests_per_second: 100
      requests_per_minute: 5000

    per_user:
      requests_per_second: 10
      requests_per_minute: 500

    algorithm: "token_bucket"

    token_bucket:
      refill_rate: 10.0
      bucket_capacity: 100

  # Request Batching Strategy
  batching:
    enabled: true

    min_batch_size: 2
    max_batch_size: 10
    max_wait_ms: 100

    grouping:
      enabled: true
      strategy: "similarity"
      similarity_threshold: 0.85

    token_packing:
      enabled: true
      max_total_tokens: 8000

  # Prompt Optimization Strategy
  prompt_optimization:
    enabled: true

    ab_testing:
      min_sample_size: 100
      significance_level: 0.05
      max_duration_seconds: 604800

    variant_generation:
      strategies:
        - conciseness
        - clarity
        - specificity
        - examples

    thompson_sampling:
      enabled: true
      alpha: 1.0
      beta: 1.0

# Analyzer Engine Integration
analyzer:
  enabled: true
  metrics_refresh_interval_seconds: 10

  performance:
    window_size_seconds: 300
    aggregation_interval_seconds: 60

  drift_detection:
    enabled: true
    algorithm: "adwin"
    sensitivity: 0.05

  anomaly_detection:
    enabled: true
    algorithms:
      - zscore
      - iqr
    threshold: 3.0
```

### Environment Variables

```bash
# Decision Engine
DECISION_ENGINE_ENABLED=true
DECISION_LOG_LEVEL=info

# Model Selection
MODEL_SELECTION_STRATEGY=pareto
EXPLORATION_FACTOR=1.0

# Caching
CACHE_BACKEND=redis
REDIS_URL=redis://localhost:6379
CACHE_TTL_SECONDS=3600

# Rate Limiting
RATE_LIMIT_GLOBAL_RPS=100
RATE_LIMIT_USER_RPS=10

# Batching
BATCH_MAX_SIZE=10
BATCH_MAX_WAIT_MS=100

# A/B Testing
AB_TEST_MIN_SAMPLES=100
AB_TEST_SIGNIFICANCE=0.05
```

---

## Usage Examples

### Example 1: E-Commerce Chatbot

```rust
use llm_auto_optimizer::decision::*;

async fn ecommerce_chatbot_optimization() -> Result<()> {
    let registry = ModelRegistry::new();
    let mut tuner = AdaptiveParameterTuner::with_defaults();

    // Configure for different customer query types

    // 1. Product questions (balance quality/cost)
    let product_config = ParameterConfig::new(0.6, 0.9, 512)?;
    let product_id = tuner.register_config(product_config)?;

    // 2. Technical support (high quality)
    let support_config = ParameterConfig::new(0.3, 0.85, 1024)?;
    let support_id = tuner.register_config(support_config)?;

    // 3. Casual chat (low cost)
    let chat_config = ParameterConfig::new(0.9, 0.95, 256)?;
    let chat_id = tuner.register_config(chat_config)?;

    // Handle incoming query
    let customer_query = "What's the return policy for this item?";
    let query_type = classify_query(customer_query);  // "product_question"

    // Select optimal model
    let context = RequestContext::new(estimate_tokens(customer_query))
        .with_task_type(&query_type);

    let candidates = vec![
        "gpt-4-turbo".to_string(),
        "claude-3.5-sonnet".to_string(),
        "gpt-3.5-turbo".to_string(),
    ];

    let optimal_models = registry.find_pareto_optimal(&candidates, 200, 300)?;
    let model = &optimal_models[0];

    // Get optimal parameters
    let (_, params) = tuner.select_config(&context)?;

    println!("Selected optimization:");
    println!("  Model: {}", model.name);
    println!("  Temperature: {}", params.temperature);
    println!("  Estimated cost: ${:.4}", model.objectives.cost);

    Ok(())
}

fn classify_query(query: &str) -> String {
    // Simple classification logic
    if query.contains("return") || query.contains("refund") {
        "product_question".to_string()
    } else if query.contains("error") || query.contains("not working") {
        "technical_support".to_string()
    } else {
        "casual_chat".to_string()
    }
}

fn estimate_tokens(text: &str) -> usize {
    // Rough estimate: 1 token ≈ 4 characters
    text.len() / 4
}
```

### Example 2: Code Generation Service

```rust
async fn code_generation_optimization() -> Result<()> {
    // Setup
    let registry = ModelRegistry::new();
    let ab_engine = ABTestEngine::new(&config);

    // Create experiment for prompt optimization
    let base_prompt = "Generate Python code for the following task:";

    let variants = vec![
        // Control
        Variant::new(
            "control",
            create_config(base_prompt),
            0.25
        ),

        // Variant 1: With examples
        Variant::new(
            "with_examples",
            create_config(&format!(
                "{}\n\nExample:\nTask: Sort a list\n\
                 Code: ```python\nlist.sort()```\n\nNow complete:",
                base_prompt
            )),
            0.25
        ),

        // Variant 2: Structured output
        Variant::new(
            "structured",
            create_config(&format!(
                "{}\n\nProvide:\n1. Code\n2. Explanation\n3. Test cases",
                base_prompt
            )),
            0.25
        ),

        // Variant 3: Step-by-step
        Variant::new(
            "step_by_step",
            create_config(&format!(
                "{}\n\nThink step by step:\n1. Understand the requirements\n\
                 2. Plan the solution\n3. Write the code\n4. Add comments",
                base_prompt
            )),
            0.25
        ),
    ];

    let experiment_id = ab_engine.create_experiment(
        "code_gen_prompt_optimization",
        variants
    )?;

    ab_engine.start(&experiment_id)?;

    // Run experiment
    for _ in 0..500 {
        let (variant_id, config) = ab_engine.assign_variant(&experiment_id)?;

        // Execute code generation
        let result = execute_code_generation(&config).await?;

        // Evaluate quality (compile success, test pass, etc.)
        let quality = evaluate_code_quality(&result)?;

        ab_engine.record_outcome(
            &experiment_id,
            &variant_id,
            result.compiles,
            quality,
            result.cost,
            result.latency_ms
        )?;
    }

    // Get results
    let winner = ab_engine.conclude(&experiment_id)?;
    println!("Winning variant: {:?}", winner.winner);

    Ok(())
}
```

### Example 3: Content Moderation Pipeline

```rust
async fn content_moderation_optimization() -> Result<()> {
    let registry = ModelRegistry::new();
    let rate_limiter = MultiTierRateLimiter::new(100.0, 10.0, 50.0);

    // Tiered approach based on content risk
    async fn moderate_content(
        content: &str,
        user_id: &str,
        tenant_id: &str,
        registry: &ModelRegistry,
        rate_limiter: &MultiTierRateLimiter
    ) -> Result<ModerationResult> {
        // Check rate limits
        rate_limiter.check_limit(user_id, tenant_id).await?;

        // Quick heuristic check
        let risk_score = quick_risk_assessment(content);

        if risk_score < 0.2 {
            // Low risk: use cheap, fast model
            let model = registry.get("gpt-3.5-turbo").unwrap();
            return moderate_with_model(content, model, Priority::Low).await;
        } else if risk_score < 0.7 {
            // Medium risk: balanced model
            let model = registry.get("claude-3.5-sonnet").unwrap();
            return moderate_with_model(content, model, Priority::Medium).await;
        } else {
            // High risk: use best model + human review
            let model = registry.get("gpt-4-turbo").unwrap();
            let result = moderate_with_model(content, model, Priority::High).await?;

            if result.needs_human_review {
                queue_for_human_review(content, &result).await?;
            }

            return Ok(result);
        }
    }

    Ok(())
}

fn quick_risk_assessment(content: &str) -> f64 {
    // Simple heuristic scoring
    let mut score = 0.0;

    let flagged_words = ["explicit", "violence", "hate"];
    for word in flagged_words {
        if content.to_lowercase().contains(word) {
            score += 0.3;
        }
    }

    score.min(1.0)
}
```

---

## Best Practices

### General Principles

1. **Start Simple, Iterate**: Begin with one strategy, add complexity as needed
2. **Measure Everything**: Track all metrics before and after optimizations
3. **A/B Test Changes**: Never deploy optimization changes without testing
4. **Set Clear Goals**: Define success metrics upfront (cost reduction, latency, quality)
5. **Monitor Continuously**: Watch for degradation and drift
6. **Document Learnings**: Build institutional knowledge of what works
7. **Fail Safe**: Always have fallback mechanisms
8. **Consider Trade-offs**: Every optimization has costs, balance carefully

### Strategy-Specific Best Practices

#### Model Selection
- ✅ Regularly update model registry as new models are released
- ✅ Use task-specific weights (code → quality, chat → cost)
- ✅ Monitor for model deprecation and API changes
- ❌ Don't optimize for a single metric (quality OR cost)
- ❌ Don't ignore latency requirements

#### Caching
- ✅ Set TTLs based on content freshness requirements
- ✅ Use semantic similarity carefully (0.95+ threshold)
- ✅ Monitor hit rates and adjust strategies
- ❌ Don't cache sensitive/personal information
- ❌ Don't set TTLs too long for dynamic content

#### Rate Limiting
- ✅ Set limits at 80% of provider limits
- ✅ Use adaptive limiting in production
- ✅ Implement priority classes for critical requests
- ❌ Don't be too aggressive (increases user frustration)
- ❌ Don't ignore burst patterns

#### Request Batching
- ✅ Start with small batch sizes (2-5)
- ✅ Monitor latency per request
- ✅ Use similarity-based grouping where possible
- ❌ Don't batch unrelated requests
- ❌ Don't wait too long to fill batches (>200ms)

#### Prompt Optimization
- ✅ Run A/B tests to statistical significance
- ✅ Test one change at a time
- ✅ Use Thompson Sampling for faster convergence
- ❌ Don't stop tests early
- ❌ Don't optimize prompts with low traffic

### Production Deployment

1. **Canary Rollouts**: Deploy to 5% traffic, monitor, then increase
2. **Feature Flags**: Enable/disable strategies without code changes
3. **Circuit Breakers**: Disable optimization on errors
4. **Graceful Degradation**: Fall back to simple strategies on failure
5. **Comprehensive Logging**: Log all decisions for debugging
6. **Alerting**: Set up alerts for anomalies and degradation
7. **Regular Reviews**: Weekly review of optimization effectiveness

---

## Troubleshooting

### Common Issues

#### Issue: Poor Model Selection Performance

**Symptoms:**
- Selected models frequently underperform
- High cost with low quality
- Latency spikes

**Diagnosis:**
```rust
// Check Pareto frontier
let candidates = registry.find_pareto_optimal(&all_models, 1000, 500)?;
println!("Pareto set size: {}", candidates.len());

// Check if weights are appropriate
let weights = ObjectiveWeights::balanced(1.0, 5000.0);
for candidate in &candidates {
    println!("{}: score={:.3}",
        candidate.name,
        candidate.composite_score(&weights)
    );
}

// Check reinforcement learning stats
let stats = engine.get_policy_stats("model_selection")?;
for stat in stats {
    println!("Model {}: avg_reward={:.3}, selections={}",
        stat.variant_id,
        stat.average_reward,
        stat.num_selections
    );
}
```

**Solutions:**
- Adjust objective weights based on actual priorities
- Increase exploration factor if getting stuck in local optimum
- Verify model pricing and performance data is up to date
- Check if request context is being set correctly

#### Issue: Low Cache Hit Rate

**Symptoms:**
- Cache hit rate <20%
- Not seeing expected cost savings
- High cache misses

**Diagnosis:**
```rust
// Check cache metrics
let metrics = cache.get_metrics();
println!("Hit rate: {:.1}%", metrics.hit_rate() * 100.0);
println!("Hits: {}, Misses: {}", metrics.hits, metrics.misses);

// Analyze cache keys
let keys = cache.get_all_keys().await?;
println!("Unique keys: {}", keys.len());

// Check normalization
let original = "  What is   the capital of France?  ";
let normalized = cache.normalize(original);
println!("Original: '{}'", original);
println!("Normalized: '{}'", normalized);
```

**Solutions:**
- Improve normalization (lowercase, trim whitespace)
- Reduce semantic similarity threshold (0.95 → 0.90)
- Increase TTL if content doesn't change often
- Check if requests are actually similar enough to cache

#### Issue: Rate Limiting Too Aggressive

**Symptoms:**
- High rejection rate (>5%)
- User complaints about throttling
- Underutilizing API quota

**Diagnosis:**
```rust
// Check rejection metrics
let limiter_stats = rate_limiter.get_stats();
println!("Total requests: {}", limiter_stats.total);
println!("Rejected: {} ({:.1}%)",
    limiter_stats.rejected,
    (limiter_stats.rejected as f64 / limiter_stats.total as f64) * 100.0
);

// Check bucket state
let available = bucket.available_tokens().await;
println!("Available tokens: {:.1}/{}", available, bucket.capacity);
```

**Solutions:**
- Increase bucket capacity
- Increase refill rate
- Implement priority queuing instead of rejecting
- Use adaptive rate limiting

#### Issue: Batching Increases Latency

**Symptoms:**
- P95 latency increased after enabling batching
- Users complaining about slow responses
- Batch wait time too long

**Diagnosis:**
```rust
// Check batch metrics
let batch_metrics = batcher.get_metrics();
println!("Avg batch size: {:.1}", batch_metrics.average_batch_size());
println!("Avg batch latency: {:.0}ms", batch_metrics.average_batch_latency());
println!("Avg item latency: {:.0}ms", batch_metrics.average_item_latency());
println!("Efficiency: {:.1}%", batch_metrics.batching_efficiency() * 100.0);
```

**Solutions:**
- Decrease max_wait_ms (100ms → 50ms)
- Reduce target batch size
- Use adaptive batch sizing
- Disable batching for latency-sensitive endpoints

#### Issue: A/B Test Not Reaching Significance

**Symptoms:**
- Experiments running for weeks
- p-value stuck above 0.05
- No clear winner emerging

**Diagnosis:**
```rust
// Check experiment statistics
let stats = ab_engine.get_statistics(&experiment_id)?;
println!("Duration: {}s", stats.duration_seconds);
println!("Samples: {}", stats.total_samples);
println!("P-value: {:.4}", stats.p_value.unwrap_or(1.0));

for variant in &stats.variant_stats {
    println!("Variant {}: mean={:.3}, std={:.3}, n={}",
        variant.variant_name,
        variant.mean_quality,
        variant.std_quality,
        variant.sample_count
    );
}
```

**Solutions:**
- Increase traffic allocation to experiment
- Reduce number of variants (4 → 2)
- Check if variants are actually different
- Increase sample size requirement
- Consider if effect size is too small to detect

### Debug Mode

Enable debug logging for detailed diagnostics:

```rust
use tracing_subscriber;

// Initialize logging
tracing_subscriber::fmt()
    .with_max_level(tracing::Level::DEBUG)
    .init();

// Decision engine will now log detailed information
let decision = DecisionEngine::new(&config);
```

### Performance Profiling

Profile decision latency:

```rust
use std::time::Instant;

let start = Instant::now();
let decision = engine.make_decision(&request).await?;
let latency = start.elapsed();

println!("Decision latency: {:.1}ms", latency.as_secs_f64() * 1000.0);

if latency.as_millis() > 100 {
    tracing::warn!("Slow decision: {}ms", latency.as_millis());
}
```

---

## Performance Tuning

### Optimization Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Decision Latency | <50ms (p95) | Total time to make optimization decision |
| Model Selection | <20ms (p95) | Time to select optimal model |
| Cache Lookup | <5ms (p95) | Redis/memory cache lookup |
| Rate Limit Check | <1ms (p95) | Token bucket check |
| Batch Formation | <10ms (p95) | Group requests into batches |

### Latency Optimization

```rust
// Use async operations for I/O
tokio::join!(
    check_cache(request),
    check_rate_limit(user_id),
    select_model(context)
);

// Use connection pooling
let redis_pool = deadpool_redis::Pool::builder(manager)
    .max_size(100)
    .build()?;

// Cache frequently accessed data
let model_registry = Arc::new(ModelRegistry::new());

// Use local caching
let local_cache = LruCache::new(1000);
```

### Memory Optimization

```rust
// Limit cache sizes
let cache_config = CacheConfig {
    max_entries: 10_000,
    max_size_mb: 512,
    eviction_policy: EvictionPolicy::LRU,
};

// Use efficient data structures
use dashmap::DashMap;  // Lock-free concurrent hash map

// Periodic cleanup
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(300));
    loop {
        interval.tick().await;
        cache.evict_expired().await;
    }
});
```

### Throughput Optimization

```rust
// Increase concurrency
let decision_engine = DecisionEngine::new(&config)
    .with_max_concurrent_decisions(1000);

// Use batching for similar operations
let batch_selector = ModelBatchSelector::new(50);

// Pipeline operations
use futures::stream::{self, StreamExt};

let results = stream::iter(requests)
    .map(|req| async move {
        optimize_request(&req).await
    })
    .buffer_unordered(100)  // Process 100 concurrently
    .collect::<Vec<_>>()
    .await;
```

### Resource Limits

```yaml
# config.yaml - Resource limits
decision_engine:
  resource_limits:
    max_memory_mb: 2048
    max_cpu_percent: 80
    max_concurrent_operations: 1000

    # Per-strategy limits
    model_selection:
      max_evaluations_per_second: 10000

    caching:
      max_cache_size_mb: 512
      max_connections: 100

    batching:
      max_concurrent_batches: 50
```

---

## Production Deployment Guide

### Pre-Deployment Checklist

- [ ] Configuration validated and tested
- [ ] All strategies have appropriate fallbacks
- [ ] Metrics and logging configured
- [ ] Alerts set up for critical metrics
- [ ] Rate limits set conservatively
- [ ] Cache TTLs appropriate for use case
- [ ] A/B tests have sufficient traffic
- [ ] Load testing completed
- [ ] Rollback plan documented
- [ ] Team trained on monitoring/troubleshooting

### Deployment Steps

#### 1. Staging Deployment

```bash
# Deploy to staging
./deploy.sh staging

# Run integration tests
cargo test --test '*' -- --test-threads=1

# Run load tests
k6 run loadtest.js --duration=30m

# Verify metrics
./scripts/check_metrics.sh staging
```

#### 2. Canary Deployment (5% Traffic)

```yaml
# kubernetes/canary-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: decision-engine-canary
spec:
  replicas: 1
  template:
    metadata:
      labels:
        app: decision-engine
        version: canary
    spec:
      containers:
      - name: decision-engine
        image: decision-engine:v2.0.0
        env:
        - name: CANARY_TRAFFIC_PERCENT
          value: "5"
```

```bash
# Deploy canary
kubectl apply -f kubernetes/canary-deployment.yaml

# Monitor for 1 hour
./scripts/monitor_canary.sh

# Check error rates
kubectl logs -l version=canary | grep ERROR | wc -l
```

#### 3. Progressive Rollout

```bash
# Increase to 25%
./scripts/update_traffic.sh 25
sleep 1800  # Wait 30 minutes

# Increase to 50%
./scripts/update_traffic.sh 50
sleep 1800

# Increase to 100%
./scripts/update_traffic.sh 100
```

#### 4. Post-Deployment Validation

```bash
# Verify all instances healthy
kubectl get pods -l app=decision-engine

# Check metrics
curl http://decision-engine/metrics | grep decision_latency_ms

# Verify cost savings
./scripts/cost_report.sh --compare-baseline

# Run smoke tests
./scripts/smoke_tests.sh production
```

### Monitoring Setup

#### Prometheus Metrics

```rust
use prometheus::{Counter, Histogram, Registry};

lazy_static! {
    static ref DECISION_LATENCY: Histogram = Histogram::new(
        "decision_latency_seconds",
        "Time to make optimization decision"
    ).unwrap();

    static ref MODEL_SELECTIONS: Counter = Counter::new(
        "model_selections_total",
        "Total number of model selections"
    ).unwrap();

    static ref CACHE_HITS: Counter = Counter::new(
        "cache_hits_total",
        "Total cache hits"
    ).unwrap();
}

// Record metrics
let timer = DECISION_LATENCY.start_timer();
let decision = make_decision().await?;
timer.observe_duration();

MODEL_SELECTIONS.inc();
```

#### Grafana Dashboards

Create dashboards for:
- Decision latency (p50, p95, p99)
- Strategy effectiveness (cost savings, quality improvement)
- Cache hit rates
- Rate limit rejection rates
- Batch efficiency
- A/B test progress

#### Alerts

```yaml
# prometheus/alerts.yaml
groups:
  - name: decision_engine
    rules:
      - alert: HighDecisionLatency
        expr: histogram_quantile(0.95, decision_latency_seconds) > 0.1
        for: 5m
        annotations:
          summary: Decision latency p95 > 100ms

      - alert: LowCacheHitRate
        expr: rate(cache_hits_total[5m]) / rate(cache_lookups_total[5m]) < 0.2
        for: 10m
        annotations:
          summary: Cache hit rate < 20%

      - alert: HighRateLimitRejections
        expr: rate(rate_limit_rejections_total[5m]) > 10
        for: 5m
        annotations:
          summary: Rate limit rejecting >10 req/s
```

### Rollback Procedure

```bash
# Quick rollback script
#!/bin/bash

echo "Rolling back decision engine to previous version"

# Revert deployment
kubectl rollout undo deployment/decision-engine

# Wait for rollout
kubectl rollout status deployment/decision-engine

# Verify health
kubectl get pods -l app=decision-engine

# Check metrics
./scripts/check_metrics.sh production

echo "Rollback complete"
```

### Scaling Guidelines

#### Horizontal Scaling

```yaml
# kubernetes/hpa.yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: decision-engine-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: decision-engine
  minReplicas: 3
  maxReplicas: 20
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Pods
    pods:
      metric:
        name: decision_latency_p95_ms
      target:
        type: AverageValue
        averageValue: "50"
```

#### Vertical Scaling

Recommended resources per pod:

```yaml
resources:
  requests:
    memory: "1Gi"
    cpu: "500m"
  limits:
    memory: "2Gi"
    cpu: "2000m"
```

Adjust based on:
- Request rate: +200Mi memory per 100 req/s
- Cache size: +500Mi per 10k cache entries
- Batch size: +100Mi per 10 concurrent batches

---

## Summary

The Decision Engine provides five powerful optimization strategies that work together to optimize your LLM infrastructure:

1. **Model Selection** - Choose the optimal model balancing quality, cost, and latency
2. **Caching** - Reduce costs by reusing previous results intelligently
3. **Rate Limiting** - Control traffic and prevent overload
4. **Request Batching** - Improve throughput by grouping similar requests
5. **Prompt Optimization** - Continuously improve prompts through A/B testing

### Expected Impact

With proper configuration and tuning, expect:
- **30-60% cost reduction** through intelligent model selection and caching
- **50-70% latency improvement** for cached requests
- **2-3x throughput increase** with batching
- **10-20% quality improvement** from prompt optimization
- **99.9% availability** with rate limiting and graceful degradation

### Next Steps

1. Review your specific use case and requirements
2. Start with Model Selection strategy
3. Add Caching for high-volume repeated queries
4. Implement Rate Limiting to protect systems
5. Enable Batching for high-throughput scenarios
6. Run Prompt Optimization experiments continuously

### Additional Resources

- [Architecture Guide](ARCHITECTURE.md)
- [Analyzer Engine User Guide](ANALYZER_ENGINE_USER_GUIDE.md)
- [API Reference](../README.md)
- [GitHub Repository](https://github.com/globalbusinessadvisors/llm-auto-optimizer)

---

**Version:** 1.0
**Last Updated:** November 2025
**Maintainer:** LLM Auto Optimizer Team

For support, please visit our [GitHub Issues](https://github.com/globalbusinessadvisors/llm-auto-optimizer/issues).
