//! Integration tests for adaptive parameter tuning
//!
//! This test file demonstrates the complete integration of adaptive parameter
//! tuning with contextual bandits and performance-based optimization.

use decision::{
    AdaptiveParameterTuner, OptimizationMode, OptimizationPolicy, ParameterConfig,
    ParameterOptimizer, ParameterRange, RequestContext, ResponseMetrics, RewardWeights,
    SearchStrategy, UserFeedback,
};

#[test]
fn test_adaptive_parameter_tuning_workflow() {
    // Create optimizer
    let optimizer = ParameterOptimizer::new(RewardWeights::default_weights(), 1.0);

    // Create policy for code generation
    let policy = OptimizationPolicy::new(
        "code_gen",
        ParameterRange::for_task_type("code"),
        OptimizationMode::Explore,
    );

    optimizer.create_policy(policy).unwrap();

    // Initialize with random search
    let config_ids = optimizer
        .initialize_with_search("code_gen", SearchStrategy::Random, 10)
        .unwrap();

    assert_eq!(config_ids.len(), 10);

    // Simulate requests and feedback
    let context = RequestContext::new(500).with_task_type("code");

    for _ in 0..30 {
        let (config_id, config) = optimizer.select_parameters("code_gen", &context).unwrap();

        // Validate selected config
        assert!(config.validate().is_ok());
        assert!(config.temperature <= 0.5); // Should be low for code

        // Simulate performance metrics
        let metrics = ResponseMetrics {
            quality_score: 0.85,
            cost: 0.05,
            latency_ms: 1200.0,
            token_count: 600,
        };

        optimizer
            .update_performance("code_gen", &config_id, &context, &metrics, None)
            .unwrap();
    }

    // Get performance statistics
    let stats = optimizer.get_performance_stats("code_gen").unwrap();
    assert!(!stats.is_empty());

    // Update task best
    optimizer
        .update_task_bests("code_gen", &["code".to_string()])
        .unwrap();

    // Switch to exploit mode
    optimizer
        .set_mode("code_gen", OptimizationMode::Exploit)
        .unwrap();

    // Select should now use best known config
    let (_, best_config) = optimizer.select_parameters("code_gen", &context).unwrap();
    assert!(best_config.temperature <= 0.5);
}

#[test]
fn test_multi_task_optimization() {
    let optimizer = ParameterOptimizer::with_defaults();

    // Create policies for different task types
    let creative_policy = OptimizationPolicy::new(
        "creative",
        ParameterRange::for_task_type("creative"),
        OptimizationMode::Balanced,
    );

    let code_policy = OptimizationPolicy::new(
        "code",
        ParameterRange::for_task_type("code"),
        OptimizationMode::Balanced,
    );

    optimizer.create_policy(creative_policy).unwrap();
    optimizer.create_policy(code_policy).unwrap();

    // Initialize both
    optimizer
        .initialize_with_search("creative", SearchStrategy::Random, 8)
        .unwrap();
    optimizer
        .initialize_with_search("code", SearchStrategy::Random, 8)
        .unwrap();

    // Simulate creative task
    let creative_context = RequestContext::new(200).with_task_type("creative");
    for _ in 0..20 {
        let (config_id, config) = optimizer
            .select_parameters("creative", &creative_context)
            .unwrap();

        assert!(config.temperature >= 0.8); // Should be high for creative

        let metrics = ResponseMetrics {
            quality_score: 0.88,
            cost: 0.08,
            latency_ms: 1500.0,
            token_count: 800,
        };

        optimizer
            .update_performance("creative", &config_id, &creative_context, &metrics, None)
            .unwrap();
    }

    // Simulate code task
    let code_context = RequestContext::new(500).with_task_type("code");
    for _ in 0..20 {
        let (config_id, config) = optimizer.select_parameters("code", &code_context).unwrap();

        assert!(config.temperature <= 0.5); // Should be low for code

        let metrics = ResponseMetrics {
            quality_score: 0.90,
            cost: 0.05,
            latency_ms: 1000.0,
            token_count: 600,
        };

        optimizer
            .update_performance("code", &config_id, &code_context, &metrics, None)
            .unwrap();
    }

    // Verify both have learned
    let creative_stats = optimizer.get_performance_stats("creative").unwrap();
    let code_stats = optimizer.get_performance_stats("code").unwrap();

    assert!(!creative_stats.is_empty());
    assert!(!code_stats.is_empty());
}

#[test]
fn test_grid_search_initialization() {
    let optimizer = ParameterOptimizer::with_defaults();
    let policy = OptimizationPolicy::new(
        "test",
        ParameterRange::default(),
        OptimizationMode::Explore,
    );

    optimizer.create_policy(policy).unwrap();

    // Initialize with grid search
    let config_ids = optimizer
        .initialize_with_search("test", SearchStrategy::Grid, 8)
        .unwrap();

    assert!(!config_ids.is_empty());

    // All configs should be valid
    let stats = optimizer.get_performance_stats("test").unwrap();
    for stat in stats {
        assert!(stat.config.validate().is_ok());
    }
}

#[test]
fn test_latin_hypercube_initialization() {
    let optimizer = ParameterOptimizer::with_defaults();
    let policy = OptimizationPolicy::new(
        "test",
        ParameterRange::default(),
        OptimizationMode::Explore,
    );

    optimizer.create_policy(policy).unwrap();

    // Initialize with LHS
    let config_ids = optimizer
        .initialize_with_search("test", SearchStrategy::LatinHypercube, 20)
        .unwrap();

    assert!(!config_ids.is_empty());

    // Should provide good coverage
    let stats = optimizer.get_performance_stats("test").unwrap();
    assert!(stats.len() >= 15); // Most should be registered
}

#[test]
fn test_parameter_learning_convergence() {
    let optimizer = ParameterOptimizer::with_defaults();
    let policy = OptimizationPolicy::new(
        "convergence_test",
        ParameterRange::default(),
        OptimizationMode::Explore,
    );

    optimizer.create_policy(policy).unwrap();

    // Initialize with a few configs
    let config_ids = optimizer
        .initialize_with_search("convergence_test", SearchStrategy::Random, 5)
        .unwrap();

    let context = RequestContext::new(300);

    // Simulate clear winner
    let winner_id = config_ids[0];
    let loser_id = config_ids[1];

    let good_metrics = ResponseMetrics {
        quality_score: 0.95,
        cost: 0.03,
        latency_ms: 800.0,
        token_count: 400,
    };

    let bad_metrics = ResponseMetrics {
        quality_score: 0.5,
        cost: 0.15,
        latency_ms: 2500.0,
        token_count: 900,
    };

    // Train heavily
    for _ in 0..50 {
        optimizer
            .update_performance("convergence_test", &winner_id, &context, &good_metrics, None)
            .unwrap();
        optimizer
            .update_performance("convergence_test", &loser_id, &context, &bad_metrics, None)
            .unwrap();
    }

    let stats = optimizer.get_performance_stats("convergence_test").unwrap();
    let winner_stats = stats.iter().find(|s| s.config_id == winner_id).unwrap();
    let loser_stats = stats.iter().find(|s| s.config_id == loser_id).unwrap();

    // Winner should have much higher reward
    assert!(winner_stats.average_reward > loser_stats.average_reward);
    assert!(winner_stats.avg_quality > loser_stats.avg_quality);
}

#[test]
fn test_exploration_exploitation_balance() {
    let optimizer = ParameterOptimizer::with_defaults();
    let policy = OptimizationPolicy::new(
        "balanced",
        ParameterRange::default(),
        OptimizationMode::Balanced,
    )
    .with_exploration_rate(0.5);

    optimizer.create_policy(policy).unwrap();

    optimizer
        .initialize_with_search("balanced", SearchStrategy::Random, 10)
        .unwrap();

    let context = RequestContext::new(200);

    // Should explore and exploit
    let mut configs_seen = std::collections::HashSet::new();

    for _ in 0..100 {
        let (config_id, _) = optimizer.select_parameters("balanced", &context).unwrap();
        configs_seen.insert(config_id);

        let metrics = ResponseMetrics {
            quality_score: 0.8,
            cost: 0.05,
            latency_ms: 1000.0,
            token_count: 500,
        };

        optimizer
            .update_performance("balanced", &config_id, &context, &metrics, None)
            .unwrap();
    }

    // Should have explored multiple configs
    assert!(configs_seen.len() > 1);
}

#[test]
fn test_task_specific_best_configs() {
    let mut tuner = AdaptiveParameterTuner::new(ParameterRange::for_task_type("code"));

    // Register a good config for code
    let code_config = ParameterConfig::code_generation();
    let config_id = tuner.register_config(code_config).unwrap();

    // Train it
    let metrics = ResponseMetrics {
        quality_score: 0.92,
        cost: 0.04,
        latency_ms: 900.0,
        token_count: 450,
    };

    for _ in 0..15 {
        tuner.update_config(&config_id, 0.88, &metrics, None).unwrap();
    }

    tuner.update_task_best("code".to_string());

    // Should find it as best
    let best = tuner.get_best_for_task("code");
    assert!(best.is_some());
    let (best_id, best_config) = best.unwrap();
    assert_eq!(best_id, config_id);
    assert!(best_config.temperature < 0.3);
}

#[test]
fn test_parameter_config_presets() {
    // Test all preset configs are valid
    assert!(ParameterConfig::creative().validate().is_ok());
    assert!(ParameterConfig::analytical().validate().is_ok());
    assert!(ParameterConfig::code_generation().validate().is_ok());
    assert!(ParameterConfig::balanced().validate().is_ok());

    // Verify characteristics
    let creative = ParameterConfig::creative();
    assert!(creative.temperature > 1.0);

    let analytical = ParameterConfig::analytical();
    assert!(analytical.temperature < 0.5);

    let code = ParameterConfig::code_generation();
    assert!(code.temperature < 0.3);
}

#[test]
fn test_parameter_range_task_specific() {
    let creative_range = ParameterRange::for_task_type("creative");
    let code_range = ParameterRange::for_task_type("code");
    let analytical_range = ParameterRange::for_task_type("analytical");

    // Creative should allow higher temps
    assert!(creative_range.temp_min >= 0.8);
    assert!(creative_range.temp_max >= 1.0);

    // Code should restrict to low temps
    assert!(code_range.temp_max <= 0.5);

    // Analytical should be very restrictive
    assert!(analytical_range.temp_max <= 0.4);
}

#[test]
fn test_adaptive_tuning_with_feedback() {
    let optimizer = ParameterOptimizer::with_defaults();
    let policy = OptimizationPolicy::new(
        "feedback_test",
        ParameterRange::default(),
        OptimizationMode::Explore,
    );

    optimizer.create_policy(policy).unwrap();

    optimizer
        .initialize_with_search("feedback_test", SearchStrategy::Random, 5)
        .unwrap();

    let context = RequestContext::new(400);

    // Simulate with explicit user feedback
    for i in 0..20 {
        let (config_id, _) = optimizer.select_parameters("feedback_test", &context).unwrap();

        let metrics = ResponseMetrics {
            quality_score: 0.85,
            cost: 0.06,
            latency_ms: 1100.0,
            token_count: 550,
        };

        let mut feedback = UserFeedback::new();
        feedback.task_completed = i % 5 != 0; // 80% completion rate
        feedback.explicit_rating = Some(if i % 3 == 0 { 5.0 } else { 4.0 });

        optimizer
            .update_performance("feedback_test", &config_id, &context, &metrics, Some(&feedback))
            .unwrap();
    }

    let stats = optimizer.get_performance_stats("feedback_test").unwrap();
    assert!(!stats.is_empty());

    // Some configs should have learned
    assert!(stats.iter().any(|s| s.num_uses > 0));
}

#[test]
fn test_mode_switching() {
    let optimizer = ParameterOptimizer::with_defaults();
    let policy = OptimizationPolicy::new(
        "mode_test",
        ParameterRange::default(),
        OptimizationMode::Explore,
    );

    optimizer.create_policy(policy).unwrap();

    // Start in explore mode
    assert_eq!(
        optimizer.get_mode("mode_test").unwrap(),
        OptimizationMode::Explore
    );

    // Switch to exploit
    optimizer
        .set_mode("mode_test", OptimizationMode::Exploit)
        .unwrap();
    assert_eq!(
        optimizer.get_mode("mode_test").unwrap(),
        OptimizationMode::Exploit
    );

    // Switch to balanced
    optimizer
        .set_mode("mode_test", OptimizationMode::Balanced)
        .unwrap();
    assert_eq!(
        optimizer.get_mode("mode_test").unwrap(),
        OptimizationMode::Balanced
    );
}

#[test]
fn test_real_world_scenario_blog_writing() {
    let optimizer = ParameterOptimizer::with_defaults();

    // Blog writing needs creativity with some control
    let range = ParameterRange {
        temp_min: 0.6,
        temp_max: 1.2,
        top_p_min: 0.9,
        top_p_max: 0.98,
        max_tokens_min: 512,
        max_tokens_max: 3072,
    };

    let policy = OptimizationPolicy::new("blog_writing", range, OptimizationMode::Balanced)
        .with_exploration_rate(0.3);

    optimizer.create_policy(policy).unwrap();

    optimizer
        .initialize_with_search("blog_writing", SearchStrategy::LatinHypercube, 12)
        .unwrap();

    let context = RequestContext::new(300)
        .with_task_type("creative")
        .with_output_length(decision::OutputLengthCategory::Long);

    // Simulate blog writing sessions
    for _ in 0..30 {
        let (config_id, config) = optimizer
            .select_parameters("blog_writing", &context)
            .unwrap();

        // Should be in creative range
        assert!(config.temperature >= 0.6);
        assert!(config.temperature <= 1.2);

        let metrics = ResponseMetrics {
            quality_score: 0.87,
            cost: 0.07,
            latency_ms: 1400.0,
            token_count: 1200,
        };

        let mut feedback = UserFeedback::new();
        feedback.task_completed = true;
        feedback.explicit_rating = Some(4.5);

        optimizer
            .update_performance("blog_writing", &config_id, &context, &metrics, Some(&feedback))
            .unwrap();
    }

    // Should have learned good params
    optimizer
        .update_task_bests("blog_writing", &["creative".to_string()])
        .unwrap();

    let best = optimizer.get_best_for_task("blog_writing", "creative");
    if let Ok(Some((_, best_config))) = best {
        assert!(best_config.temperature >= 0.6);
        assert!(best_config.temperature <= 1.2);
    }
}
