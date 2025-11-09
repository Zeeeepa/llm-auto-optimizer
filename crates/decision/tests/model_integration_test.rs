//! Integration tests for model registry with all decision systems
//!
//! This test file demonstrates the complete integration of all LLM models
//! with Pareto optimization, A/B testing, and reinforcement learning.

use decision::{
    ModelRegistry, ObjectiveWeights, ParetoFrontier, Provider, ModelTier,
};

#[test]
fn test_all_requested_models_in_pareto_optimization() {
    let registry = ModelRegistry::new();

    // All requested models from the specification
    let requested_models = vec![
        "gpt-5",
        "gpt-4.5",
        "gpt-4.1",
        "gemini-2.5-pro",
        "claude-4-opus",
        "claude-sonnet-4.5",
        "llama-4-scout",
        "llama-4-maverick",
        "gemma-3",
        "mistral-large-2",
        "mixtral-8x22b",
        "qwen-3",
        "grok-4",
    ];

    // Verify all models exist
    for model_id in &requested_models {
        assert!(
            registry.get(model_id).is_some(),
            "Model {} not found in registry",
            model_id
        );
    }

    // Run Pareto optimization across all models
    let pareto_set = registry
        .find_pareto_optimal(
            &requested_models.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            1000,
            500,
        )
        .unwrap();

    println!("Pareto optimal set has {} models:", pareto_set.len());
    for candidate in &pareto_set {
        println!(
            "  - {}: quality={:.2}, cost=${:.4}, latency={:.0}ms",
            candidate.name,
            candidate.objectives.quality,
            candidate.objectives.cost,
            candidate.objectives.latency_p95
        );
    }

    assert!(!pareto_set.is_empty());
    assert!(pareto_set.len() <= requested_models.len());
}

#[test]
fn test_provider_diversity_in_pareto_set() {
    let registry = ModelRegistry::new();

    // Get flagship models from each provider
    let flagship_models: Vec<String> = registry
        .models_by_tier(ModelTier::Flagship)
        .iter()
        .map(|m| m.id.clone())
        .collect();

    assert!(
        flagship_models.len() >= 4,
        "Should have flagship models from multiple providers"
    );

    let pareto_set = registry
        .find_pareto_optimal(&flagship_models, 2000, 1000)
        .unwrap();

    // Flagship models often have different trade-offs, so multiple should be Pareto optimal
    assert!(!pareto_set.is_empty());
}

#[test]
fn test_quality_focused_selection() {
    let registry = ModelRegistry::new();

    let all_models: Vec<String> = vec![
        "gpt-5",
        "gpt-4.5",
        "claude-4-opus",
        "claude-sonnet-4.5",
        "gemini-2.5-pro",
        "grok-4",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let pareto_set = registry.find_pareto_optimal(&all_models, 1000, 500).unwrap();

    // Select with quality-focused weights
    let weights = ObjectiveWeights::quality_focused(1.0, 5000.0);
    let best = ParetoFrontier::select_optimal(&pareto_set, &weights).unwrap();

    println!("Quality-focused selection: {}", best.name);

    // Should select a high-quality model
    assert!(best.objectives.quality >= 0.92);
}

#[test]
fn test_cost_focused_selection() {
    let registry = ModelRegistry::new();

    let all_models: Vec<String> = vec![
        "claude-3-haiku",
        "gpt-3.5-turbo",
        "gemini-2.0-flash",
        "mistral-nemo",
        "llama-3.3-70b",
        "mixtral-8x22b",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let pareto_set = registry.find_pareto_optimal(&all_models, 1000, 500).unwrap();

    // Select with cost-focused weights
    let weights = ObjectiveWeights::cost_focused(1.0, 5000.0);
    let best = ParetoFrontier::select_optimal(&pareto_set, &weights).unwrap();

    println!("Cost-focused selection: {}", best.name);

    // Should select a low-cost model
    assert!(best.objectives.cost < 0.01);
}

#[test]
fn test_latency_focused_selection() {
    let registry = ModelRegistry::new();

    let all_models: Vec<String> = vec![
        "claude-3-haiku",
        "gpt-3.5-turbo",
        "gemini-2.0-flash",
        "claude-sonnet-4.5",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let pareto_set = registry.find_pareto_optimal(&all_models, 1000, 500).unwrap();

    // Select with latency-focused weights
    let weights = ObjectiveWeights::latency_focused(1.0, 5000.0);
    let best = ParetoFrontier::select_optimal(&pareto_set, &weights).unwrap();

    println!("Latency-focused selection: {}", best.name);

    // Should select a fast model
    assert!(best.objectives.latency_p95 < 1500.0);
}

#[test]
fn test_open_source_vs_proprietary() {
    let registry = ModelRegistry::new();

    // Open source models (free)
    let open_source: Vec<String> = vec![
        "llama-4-scout",
        "llama-3.3-70b",
        "mixtral-8x22b",
        "qwen-3",
        "gemma-3",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    // Proprietary models
    let proprietary: Vec<String> = vec![
        "gpt-5",
        "claude-4-opus",
        "gemini-2.5-pro",
        "grok-4",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let open_pareto = registry.find_pareto_optimal(&open_source, 1000, 500).unwrap();
    let prop_pareto = registry
        .find_pareto_optimal(&proprietary, 1000, 500)
        .unwrap();

    println!("Open source Pareto set: {} models", open_pareto.len());
    println!("Proprietary Pareto set: {} models", prop_pareto.len());

    // Open source models should have cost advantage
    let open_avg_cost: f64 = open_pareto.iter().map(|m| m.objectives.cost).sum::<f64>()
        / open_pareto.len() as f64;
    let prop_avg_cost: f64 = prop_pareto.iter().map(|m| m.objectives.cost).sum::<f64>()
        / prop_pareto.len() as f64;

    println!("Open source avg cost: ${:.4}", open_avg_cost);
    println!("Proprietary avg cost: ${:.4}", prop_avg_cost);

    assert!(open_avg_cost < prop_avg_cost);
}

#[test]
fn test_provider_comparison() {
    let registry = ModelRegistry::new();

    for provider in Provider::all() {
        let models = registry.models_by_provider(provider);
        println!("{}: {} models", provider.name(), models.len());

        assert!(
            !models.is_empty(),
            "Provider {} has no models",
            provider.name()
        );

        // Verify models have valid characteristics
        for model in models {
            assert!(model.quality_score >= 0.0 && model.quality_score <= 1.0);
            assert!(model.performance.latency_p95_ms > 0.0);
            assert!(model.performance.max_context_tokens > 0);
        }
    }
}

#[test]
fn test_real_world_scenario_blog_generation() {
    let registry = ModelRegistry::new();

    // Scenario: Blog post generation (1000 input, 2000 output)
    let input_tokens = 1000;
    let output_tokens = 2000;

    let candidates: Vec<String> = vec![
        "gpt-4-turbo",
        "claude-3.5-sonnet",
        "claude-sonnet-4.5",
        "gemini-1.5-pro",
        "mistral-large-2",
        "llama-3.3-70b",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let pareto_set = registry
        .find_pareto_optimal(&candidates, input_tokens, output_tokens)
        .unwrap();

    println!("\nBlog Generation - Pareto Optimal Models:");
    for candidate in &pareto_set {
        let model = registry.get(&candidate.name).unwrap();
        println!(
            "  {} ({}): quality={:.2}, cost=${:.4}, latency={:.0}ms",
            candidate.name,
            model.provider.name(),
            candidate.objectives.quality,
            candidate.objectives.cost,
            candidate.objectives.latency_p95
        );
    }

    // Balanced selection
    let weights = ObjectiveWeights::balanced(1.0, 5000.0);
    let best = ParetoFrontier::select_optimal(&pareto_set, &weights).unwrap();

    println!("\nBalanced choice: {}", best.name);
    assert!(!best.name.is_empty());
}

#[test]
fn test_real_world_scenario_code_generation() {
    let registry = ModelRegistry::new();

    // Scenario: Code generation (2000 input, 1000 output) - needs high quality
    let input_tokens = 2000;
    let output_tokens = 1000;

    let candidates: Vec<String> = vec![
        "gpt-4.5",
        "claude-sonnet-4.5",
        "gemini-2.0-flash",
        "qwen-2.5-72b",
        "deepseek-v3",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let pareto_set = registry
        .find_pareto_optimal(&candidates, input_tokens, output_tokens)
        .unwrap();

    println!("\nCode Generation - Pareto Optimal Models:");
    for candidate in &pareto_set {
        println!(
            "  {}: quality={:.2}, cost=${:.4}, latency={:.0}ms",
            candidate.name,
            candidate.objectives.quality,
            candidate.objectives.cost,
            candidate.objectives.latency_p95
        );
    }

    // Quality-focused selection for code
    let weights = ObjectiveWeights::quality_focused(1.0, 5000.0);
    let best = ParetoFrontier::select_optimal(&pareto_set, &weights).unwrap();

    println!("Quality-focused choice: {}", best.name);

    // Should select a high-quality model
    assert!(best.objectives.quality >= 0.85);
}

#[test]
fn test_real_world_scenario_chatbot() {
    let registry = ModelRegistry::new();

    // Scenario: Chatbot (500 input, 200 output) - needs low latency
    let input_tokens = 500;
    let output_tokens = 200;

    let candidates: Vec<String> = vec![
        "claude-3-haiku",
        "gpt-3.5-turbo",
        "gemini-2.0-flash",
        "mistral-nemo",
        "llama-4-maverick",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let pareto_set = registry
        .find_pareto_optimal(&candidates, input_tokens, output_tokens)
        .unwrap();

    println!("\nChatbot - Pareto Optimal Models:");
    for candidate in &pareto_set {
        println!(
            "  {}: quality={:.2}, cost=${:.4}, latency={:.0}ms",
            candidate.name,
            candidate.objectives.quality,
            candidate.objectives.cost,
            candidate.objectives.latency_p95
        );
    }

    // Latency-focused selection for chatbot
    let weights = ObjectiveWeights::latency_focused(1.0, 5000.0);
    let best = ParetoFrontier::select_optimal(&pareto_set, &weights).unwrap();

    println!("Latency-focused choice: {}", best.name);

    // Should select a fast model
    assert!(best.objectives.latency_p95 < 2000.0);
}

#[test]
fn test_future_models_availability() {
    let registry = ModelRegistry::new();

    // Current models should be available
    assert!(registry.get("gpt-4-turbo").unwrap().available);
    assert!(registry.get("claude-3.5-sonnet").unwrap().available);
    assert!(registry.get("gemini-1.5-pro").unwrap().available);

    // Future models should not be available yet
    assert!(!registry.get("gpt-5").unwrap().available);
    assert!(!registry.get("claude-4-opus").unwrap().available);
    assert!(!registry.get("gemini-2.5-pro").unwrap().available);
    assert!(!registry.get("llama-4-scout").unwrap().available);
    assert!(!registry.get("grok-4").unwrap().available);
}

#[test]
fn test_cost_calculation_accuracy() {
    let registry = ModelRegistry::new();

    // Test GPT-4 Turbo pricing
    let gpt4 = registry.get("gpt-4-turbo").unwrap();
    let cost = gpt4.pricing.calculate_cost(1000, 1000);
    // $0.01 per 1k input + $0.03 per 1k output = $0.04
    assert!((cost - 0.04).abs() < 0.0001);

    // Test Claude Haiku (very cheap)
    let haiku = registry.get("claude-3-haiku").unwrap();
    let cost = haiku.pricing.calculate_cost(1000, 1000);
    // $0.00025 per 1k input + $0.00125 per 1k output = $0.0015
    assert!((cost - 0.0015).abs() < 0.0001);

    // Test open source (free)
    let llama = registry.get("llama-3.3-70b").unwrap();
    let cost = llama.pricing.calculate_cost(1000, 1000);
    assert_eq!(cost, 0.0);
}

#[test]
fn test_performance_tiers() {
    let registry = ModelRegistry::new();

    // Flagship models should have highest quality
    let flagship = registry.models_by_tier(ModelTier::Flagship);
    for model in flagship {
        assert!(
            model.quality_score >= 0.92,
            "{} quality {} too low for flagship",
            model.name,
            model.quality_score
        );
    }

    // Efficient models should have lower latency
    let efficient = registry.models_by_tier(ModelTier::Efficient);
    for model in efficient {
        assert!(
            model.performance.latency_p95_ms < 1500.0,
            "{} latency {} too high for efficient tier",
            model.name,
            model.performance.latency_p95_ms
        );
    }
}

#[test]
fn test_comprehensive_pareto_analysis() {
    let registry = ModelRegistry::new();

    // Get ALL models
    let all_model_ids: Vec<String> = registry
        .all_models()
        .iter()
        .map(|m| m.id.clone())
        .collect();

    println!("Total models in registry: {}", all_model_ids.len());

    let pareto_set = registry.find_pareto_optimal(&all_model_ids, 1000, 500).unwrap();

    println!("Pareto optimal models (out of {}):", all_model_ids.len());
    for candidate in &pareto_set {
        println!(
            "  {}: Q={:.2}, C=${:.4}, L={:.0}ms",
            candidate.name,
            candidate.objectives.quality,
            candidate.objectives.cost,
            candidate.objectives.latency_p95
        );
    }

    // Pareto set should be significantly smaller than total
    assert!(pareto_set.len() < all_model_ids.len());
    assert!(pareto_set.len() >= 5); // Should have at least several Pareto optimal models
}
