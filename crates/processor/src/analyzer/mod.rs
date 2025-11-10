//! Analyzer Engine
//!
//! This module provides real-time analysis capabilities for LLM optimization metrics.
//! It includes 5 specialized analyzers:
//!
//! - **Performance Analyzer**: Latency, throughput, token usage, performance degradation
//! - **Cost Analyzer**: Token costs, budget utilization, cost optimization opportunities
//! - **Quality Analyzer**: Response quality, error rates, SLA compliance
//! - **Pattern Analyzer**: Temporal patterns, traffic bursts, seasonality, trends
//! - **Anomaly Analyzer**: Statistical anomaly detection using Z-score and IQR methods
//!
//! # Example
//!
//! ```rust,no_run
//! use processor::analyzer::{PerformanceAnalyzer, Analyzer, AnalyzerEvent};
//! use chrono::Utc;
//! use std::collections::HashMap;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Create and start an analyzer
//!     let mut analyzer = PerformanceAnalyzer::with_defaults();
//!     analyzer.start().await?;
//!
//!     // Process an event
//!     let event = AnalyzerEvent::Response {
//!         timestamp: Utc::now(),
//!         request_id: "req-123".to_string(),
//!         model: "gpt-4".to_string(),
//!         completion_tokens: 100,
//!         total_tokens: 200,
//!         latency_ms: 500,
//!         success: true,
//!         error: None,
//!         metadata: HashMap::new(),
//!     };
//!
//!     analyzer.process_event(event).await?;
//!
//!     // Generate a report
//!     let report = analyzer.generate_report().await?;
//!     println!("Insights: {}", report.insights.len());
//!     println!("Recommendations: {}", report.recommendations.len());
//!
//!     Ok(())
//! }
//! ```

// Core modules
pub mod traits;
pub mod types;
pub mod stats;

// Analyzer implementations
pub mod performance_analyzer;
pub mod cost_analyzer;
pub mod quality_analyzer;
pub mod pattern_analyzer;
pub mod anomaly_analyzer;

// Re-exports for convenience
pub use traits::{
    Analyzer, AnalyzerConfig, AnalyzerConfigBuilder, AnalyzerError, AnalyzerResult, AnalyzerState,
};

pub use types::{
    Action, ActionType, Alert, AlertStatus, AnalysisReport, AnalyzerEvent, AnalyzerStats,
    Confidence, Evidence, EvidenceType, FeedbackType, Impact, ImpactMetric, Insight,
    InsightCategory, Priority, Recommendation, ReportSummary, RiskLevel, Severity, Threshold,
    ThresholdOperator,
};

pub use performance_analyzer::{PerformanceAnalyzer, PerformanceAnalyzerConfig};
pub use cost_analyzer::{CostAnalyzer, CostAnalyzerConfig, ModelPricing};
pub use quality_analyzer::{QualityAnalyzer, QualityAnalyzerConfig};
pub use pattern_analyzer::{PatternAnalyzer, PatternAnalyzerConfig, PatternType};
pub use anomaly_analyzer::{AnomalyAnalyzer, AnomalyAnalyzerConfig, AnomalyType};

// Statistical utilities
pub use stats::{
    calculate_linear_trend, calculate_percentiles, is_outlier_iqr, is_outlier_zscore, mean,
    percentage_change, std_dev, z_score, CircularBuffer, ExponentialMovingAverage, LinearTrend,
    SimpleMovingAverage,
};
