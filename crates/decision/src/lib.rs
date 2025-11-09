//! Optimization decision engine for LLM Auto-Optimizer
//!
//! This crate provides the decision-making logic for optimizing LLM configurations,
//! including A/B testing, Thompson Sampling, statistical significance testing,
//! contextual bandits for reinforcement learning, Pareto optimization, adaptive
//! parameter tuning, drift & anomaly detection, and a comprehensive model registry
//! for all major LLM providers.

pub mod ab_testing;
pub mod thompson_sampling;
pub mod statistical;
pub mod variant_generator;
pub mod experiment_manager;
pub mod contextual_bandit;
pub mod context;
pub mod reward;
pub mod reinforcement_feedback;
pub mod pareto;
pub mod model_registry;
pub mod adaptive_params;
pub mod parameter_search;
pub mod parameter_optimizer;
pub mod drift_detection;
pub mod anomaly_detection;
pub mod threshold_monitor;
pub mod errors;

pub use ab_testing::ABTestEngine;
pub use thompson_sampling::ThompsonSampling;
pub use statistical::{StatisticalTest, ZTest};
pub use variant_generator::VariantGenerator;
pub use experiment_manager::ExperimentManager;
pub use contextual_bandit::{LinUCB, ContextualThompson};
pub use context::{RequestContext, OutputLengthCategory};
pub use reward::{RewardCalculator, RewardWeights, UserFeedback, ResponseMetrics};
pub use reinforcement_feedback::{ReinforcementEngine, BanditAlgorithm, VariantStats};
pub use pareto::{
    ModelCandidate, Objectives, ObjectiveWeights, ParetoFrontier, CostCalculator, QualityMetrics,
};
pub use model_registry::{
    ModelRegistry, ModelDefinition, ModelPricing, ModelPerformance, ModelCapabilities,
    Provider, ModelTier,
};
pub use adaptive_params::{
    AdaptiveParameterTuner, ParameterConfig, ParameterRange, ParameterStats,
};
pub use parameter_search::{
    GridSearch, GridSearchConfig, RandomSearch, LatinHypercubeSampling,
    ParameterSearchManager, SearchStrategy,
};
pub use parameter_optimizer::{
    ParameterOptimizer, OptimizationPolicy, OptimizationMode,
};
pub use drift_detection::{
    DriftStatus, DriftAlgorithm, ADWIN, PageHinkley, CUSUM, StatisticalDriftDetector,
};
pub use anomaly_detection::{
    AnomalyResult, ZScoreDetector, IQRDetector, MADDetector, MahalanobisDetector,
};
pub use threshold_monitor::{
    ThresholdMonitoringSystem, ThresholdConfig, Alert, AlertType, AlertSeverity,
};
pub use errors::{DecisionError, Result};
