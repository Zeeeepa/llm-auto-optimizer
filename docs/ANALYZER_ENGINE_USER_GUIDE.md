# Analyzer Engine User Guide

> **Comprehensive guide to the LLM Auto Optimizer's Analyzer Engine with 5 specialized analyzers**

---

## Table of Contents

1. [Overview](#overview)
2. [Quick Start](#quick-start)
3. [Analyzer Guides](#analyzer-guides)
4. [Configuration Reference](#configuration-reference)
5. [Integration Examples](#integration-examples)
6. [API Reference](#api-reference)
7. [Monitoring and Observability](#monitoring-and-observability)
8. [Best Practices](#best-practices)
9. [Troubleshooting](#troubleshooting)
10. [Advanced Topics](#advanced-topics)

---

## Overview

### What is the Analyzer Engine?

The Analyzer Engine is the intelligence layer of the LLM Auto Optimizer that continuously monitors, analyzes, and detects patterns in your LLM infrastructure. It consists of five specialized analyzers working in concert to provide comprehensive insights into performance, cost, quality, patterns, and anomalies.

### Why Use It?

- **Real-time insights**: Sub-second analysis of LLM metrics and events
- **Automated detection**: Identify issues before they impact users
- **Multi-dimensional analysis**: Performance, cost, quality, patterns, and anomalies in one system
- **Actionable recommendations**: Get specific optimization suggestions
- **Production-ready**: Built for high-throughput, low-latency environments

### Key Features

| Feature | Description | Status |
|---------|-------------|--------|
| **Performance Analysis** | Latency percentiles, throughput, error rates | ✅ Ready |
| **Cost Tracking** | Token usage, pricing, budget monitoring | ✅ Ready |
| **Quality Metrics** | Accuracy, relevance, coherence scoring | ✅ Ready |
| **Pattern Detection** | Temporal patterns, traffic analysis | ✅ Ready |
| **Anomaly Detection** | Statistical outlier detection with multiple algorithms | ✅ Ready |

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Analyzer Engine                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐  │
│  │  Performance  │  │     Cost      │  │    Quality    │  │
│  │   Analyzer    │  │   Analyzer    │  │   Analyzer    │  │
│  │               │  │               │  │               │  │
│  │ • Latency     │  │ • Token Cost  │  │ • Accuracy    │  │
│  │ • Throughput  │  │ • Budget      │  │ • Relevance   │  │
│  │ • Error Rate  │  │ • Trends      │  │ • Coherence   │  │
│  └───────────────┘  └───────────────┘  └───────────────┘  │
│                                                             │
│  ┌───────────────┐  ┌───────────────┐                      │
│  │    Pattern    │  │    Anomaly    │                      │
│  │   Analyzer    │  │   Analyzer    │                      │
│  │               │  │               │                      │
│  │ • Temporal    │  │ • Z-Score     │                      │
│  │ • Traffic     │  │ • IQR         │                      │
│  │ • Drift       │  │ • MAD         │                      │
│  └───────────────┘  └───────────────┘                      │
│                                                             │
│                    ▼                                        │
│            ┌──────────────────┐                             │
│            │ Threshold Monitor │                            │
│            │                  │                             │
│            │ • Alerts         │                             │
│            │ • Thresholds     │                             │
│            │ • Actions        │                             │
│            └──────────────────┘                             │
└─────────────────────────────────────────────────────────────┘
```

**Data Flow:**
1. Events flow from Feedback Collector → Stream Processor → Analyzer Engine
2. Each analyzer processes relevant metrics in parallel
3. Results feed into the Threshold Monitoring System
4. Alerts trigger automated responses via the Decision Engine

---

## Quick Start

### Basic Setup Example

```rust
use llm_auto_optimizer::analyzer::{
    AnalyzerEngine, AnalyzerConfig,
    PerformanceAnalyzer, CostAnalyzer, QualityAnalyzer,
    PatternAnalyzer, AnomalyAnalyzer
};
use llm_auto_optimizer::threshold_monitor::{
    ThresholdMonitoringSystem, ThresholdConfig
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the analyzer engine
    let config = AnalyzerConfig::default();
    let engine = AnalyzerEngine::new(config).await?;

    // Start all analyzers
    engine.start().await?;

    // The engine is now monitoring your LLM infrastructure
    println!("Analyzer Engine running!");

    Ok(())
}
```

### Enabling Analyzers

Enable specific analyzers based on your needs:

```rust
use llm_auto_optimizer::analyzer::AnalyzerConfig;

let config = AnalyzerConfig {
    // Enable/disable analyzers
    enable_performance: true,
    enable_cost: true,
    enable_quality: true,
    enable_pattern: true,
    enable_anomaly: true,

    // Configure thresholds
    max_latency_p95_ms: 5000.0,
    max_cost_per_request: 0.10,
    min_quality_score: 0.7,

    // Anomaly detection sensitivity
    anomaly_threshold: 3.0, // Z-score threshold

    ..Default::default()
};

let engine = AnalyzerEngine::new(config).await?;
```

### Viewing Results

Query analyzer results in real-time:

```rust
use llm_auto_optimizer::threshold_monitor::ThresholdMonitoringSystem;

// Create monitoring system
let monitor = ThresholdMonitoringSystem::new();

// Register metrics
monitor.register_metric("latency_p95", ThresholdConfig::latency(5000.0))?;
monitor.register_metric("cost_per_request", ThresholdConfig::cost(0.10))?;
monitor.register_metric("quality_score", ThresholdConfig::quality())?;

// Record metrics
let alerts = monitor.record("latency_p95", 3500.0);

// Check for alerts
for alert in alerts {
    println!("Alert: {} - {}", alert.metric_name, alert.message);
    println!("  Severity: {:?}", alert.severity);
    println!("  Value: {}", alert.value);
}

// Get recent alerts
let recent = monitor.get_recent_alerts("latency_p95");
println!("Recent alerts: {}", recent.len());
```

### Common Configurations

#### Production Configuration

```rust
let config = AnalyzerConfig {
    enable_performance: true,
    enable_cost: true,
    enable_quality: true,
    enable_pattern: true,
    enable_anomaly: true,

    // Performance thresholds
    max_latency_p95_ms: 3000.0,
    max_error_rate: 0.01, // 1% error rate
    min_throughput_qps: 10.0,

    // Cost controls
    max_cost_per_request: 0.05,
    cost_budget_daily: 1000.0,

    // Quality gates
    min_quality_score: 0.8,
    min_accuracy: 0.85,

    // Anomaly detection
    anomaly_threshold: 2.5, // More sensitive in production
    drift_detection_algorithm: DriftAlgorithm::ADWIN,

    // Analysis windows
    performance_window_secs: 300,  // 5 minutes
    cost_aggregation_interval_secs: 3600, // 1 hour
    pattern_analysis_interval_secs: 900, // 15 minutes
};
```

#### Development Configuration

```rust
let config = AnalyzerConfig {
    enable_performance: true,
    enable_cost: false, // Disable in dev
    enable_quality: true,
    enable_pattern: false,
    enable_anomaly: false,

    // Relaxed thresholds
    max_latency_p95_ms: 10000.0,
    min_quality_score: 0.5,

    // Shorter windows for faster feedback
    performance_window_secs: 60,
};
```

#### Cost-Optimization Configuration

```rust
let config = AnalyzerConfig {
    enable_performance: true,
    enable_cost: true,
    enable_quality: true,
    enable_pattern: true,
    enable_anomaly: true,

    // Prioritize cost optimization
    max_cost_per_request: 0.02, // Aggressive cost limit
    cost_budget_daily: 500.0,

    // Accept slightly higher latency for cost savings
    max_latency_p95_ms: 5000.0,

    // Maintain quality floor
    min_quality_score: 0.75,
};
```

---

## Analyzer Guides

### 1. Performance Analyzer

#### What It Analyzes

The Performance Analyzer tracks request latencies, throughput, and error rates across your LLM infrastructure.

**Key Metrics:**
- Latency percentiles (p50, p95, p99)
- Throughput (requests per second)
- Error rates and types
- Request success rates

#### Configuration Options

```rust
use llm_auto_optimizer::threshold_monitor::ThresholdConfig;

// Configure performance monitoring
let perf_config = ThresholdConfig::latency(5000.0);

// Or customize:
let custom_config = ThresholdConfig {
    min_value: Some(0.0),
    max_value: Some(5000.0),      // Max p95 latency: 5 seconds
    warning_min: None,
    warning_max: Some(4000.0),    // Warning at 4 seconds
    enable_drift_detection: true,
    drift_algorithm: DriftAlgorithm::CUSUM, // Good for latency
    enable_anomaly_detection: true,
    anomaly_threshold: 2.5,
};
```

#### Key Metrics Tracked

```rust
use llm_auto_optimizer::types::metrics::PerformanceMetrics;

// Example performance metrics structure
pub struct PerformanceMetrics {
    pub avg_latency_ms: f64,       // Mean latency
    pub p50_latency_ms: f64,       // Median latency
    pub p95_latency_ms: f64,       // 95th percentile
    pub p99_latency_ms: f64,       // 99th percentile
    pub error_rate: f64,           // Error rate (0.0-1.0)
    pub throughput_qps: f64,       // Queries per second
    pub total_requests: u64,       // Request count
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
}
```

#### Interpretation Guide

| Metric | Good | Warning | Critical |
|--------|------|---------|----------|
| **p95 Latency** | < 2000ms | 2000-5000ms | > 5000ms |
| **Error Rate** | < 0.1% | 0.1-1% | > 1% |
| **Throughput** | > target QPS | 70-100% of target | < 70% of target |

#### Example Insights/Recommendations

```rust
// Example: Automated performance analysis
use llm_auto_optimizer::decision::threshold_monitor::{
    ThresholdMonitoringSystem, Alert, AlertType
};

let monitor = ThresholdMonitoringSystem::new();

// Register performance metrics
monitor.register_metric("latency_p95", ThresholdConfig::latency(5000.0))?;
monitor.register_metric("error_rate", ThresholdConfig {
    max_value: Some(0.01), // 1% max error rate
    warning_max: Some(0.005), // 0.5% warning
    ..Default::default()
})?;

// Process incoming metrics
let latency_alerts = monitor.record("latency_p95", 6200.0);

for alert in latency_alerts {
    match alert.alert_type {
        AlertType::ThresholdViolation => {
            println!("⚠️  Latency exceeded threshold!");
            println!("   Current: {:.0}ms | Threshold: {:.0}ms",
                alert.value, alert.threshold.unwrap_or(0.0));
            println!("   Recommendation: Consider scaling up or using a faster model");
        }
        AlertType::Drift => {
            println!("📊 Performance drift detected");
            println!("   Recommendation: Check for infrastructure changes or traffic patterns");
        }
        AlertType::Anomaly => {
            println!("🚨 Latency anomaly detected");
            println!("   Recommendation: Investigate recent deployments or external dependencies");
        }
        _ => {}
    }
}
```

**Common Recommendations:**

- **High Latency**: Switch to smaller/faster models, enable caching, increase concurrency
- **High Error Rate**: Check model availability, review prompt complexity, validate inputs
- **Low Throughput**: Scale horizontally, optimize batch processing, review rate limits
- **Drift Detected**: Compare with historical baselines, check for infrastructure changes

---

### 2. Cost Analyzer

#### Cost Tracking Mechanisms

The Cost Analyzer tracks token usage and associated costs across different models and providers.

```rust
use llm_auto_optimizer::types::metrics::CostMetrics;

pub struct CostMetrics {
    pub total_cost: f64,              // Total cost in USD
    pub avg_cost_per_request: f64,    // Average cost per request
    pub total_tokens: u64,            // Total tokens processed
    pub input_tokens: u64,            // Input tokens
    pub output_tokens: u64,           // Output tokens
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
}
```

#### Budget Configuration

```rust
use llm_auto_optimizer::threshold_monitor::ThresholdConfig;

// Set up cost monitoring with budget
let cost_config = ThresholdConfig::cost(0.10); // Max $0.10 per request

// Custom budget configuration
let budget_config = ThresholdConfig {
    min_value: Some(0.0),
    max_value: Some(0.10),           // Hard limit: $0.10/request
    warning_min: None,
    warning_max: Some(0.08),         // Warning at $0.08/request
    enable_drift_detection: true,
    drift_algorithm: DriftAlgorithm::PageHinkley, // Good for cost trends
    enable_anomaly_detection: true,
    anomaly_threshold: 3.0,
};

let monitor = ThresholdMonitoringSystem::new();
monitor.register_metric("cost_per_request", budget_config)?;

// Track costs
let alerts = monitor.record("cost_per_request", 0.095);

if !alerts.is_empty() {
    println!("Cost approaching limit!");
}
```

#### Optimization Recommendations

```rust
use llm_auto_optimizer::decision::pareto::{
    ModelCandidate, Objectives, ObjectiveWeights, ParetoFrontier
};

// Example: Find cost-optimal model
async fn find_cost_optimal_model() -> Result<ModelCandidate, Box<dyn std::error::Error>> {
    let candidates = vec![
        ModelCandidate::new(
            uuid::Uuid::new_v4(),
            "claude-3-opus",
            Objectives::new(0.95, 0.015, 2000.0) // quality, cost, latency
        )?,
        ModelCandidate::new(
            uuid::Uuid::new_v4(),
            "claude-3-sonnet",
            Objectives::new(0.90, 0.003, 1500.0)
        )?,
        ModelCandidate::new(
            uuid::Uuid::new_v4(),
            "claude-3-haiku",
            Objectives::new(0.80, 0.00025, 800.0)
        )?,
    ];

    // Cost-focused weights
    let weights = ObjectiveWeights::cost_focused(0.02, 5000.0);

    // Find Pareto optimal candidates
    let pareto_set = ParetoFrontier::find_pareto_optimal(&candidates);

    // Select best based on cost priority
    let best = ParetoFrontier::select_best(&pareto_set, &weights)
        .ok_or("No optimal candidate found")?;

    println!("Cost-optimal model: {}", best.name);
    println!("  Quality: {:.2}", best.objectives.quality);
    println!("  Cost: ${:.5}/request", best.objectives.cost);
    println!("  Latency: {:.0}ms", best.objectives.latency_p95);

    Ok(best)
}
```

#### Example Scenarios

**Scenario 1: Budget Exceeded**

```rust
let monitor = ThresholdMonitoringSystem::new();
monitor.register_metric("daily_cost", ThresholdConfig {
    max_value: Some(1000.0), // $1000/day budget
    warning_max: Some(800.0), // Warning at $800
    ..Default::default()
})?;

let current_spend = 850.0;
let alerts = monitor.record("daily_cost", current_spend);

for alert in alerts {
    if alert.severity == AlertSeverity::Warning {
        println!("📊 Cost warning: ${:.2} / $1000.00 daily budget", current_spend);
        println!("Recommendations:");
        println!("  1. Switch to smaller models for non-critical requests");
        println!("  2. Implement request throttling");
        println!("  3. Enable aggressive caching");
    }
}
```

**Scenario 2: Cost Drift Detection**

```rust
// Detect gradual cost increases
let cost_monitor = ThresholdConfig {
    enable_drift_detection: true,
    drift_algorithm: DriftAlgorithm::PageHinkley,
    ..Default::default()
};

// Over time, if costs gradually increase, drift will be detected
let alerts = monitor.record("avg_cost_per_request", 0.045);

for alert in alerts {
    if alert.alert_type == AlertType::Drift {
        println!("📈 Cost drift detected!");
        println!("Recommendation: Review recent prompt changes or model updates");
    }
}
```

---

### 3. Quality Analyzer

#### Quality Metrics Tracked

```rust
use llm_auto_optimizer::types::metrics::QualityMetrics;

pub struct QualityMetrics {
    pub overall_score: f64,           // Overall quality (0.0-1.0)
    pub accuracy: f64,                // Answer accuracy
    pub relevance: f64,               // Response relevance
    pub coherence: f64,               // Output coherence
    pub user_satisfaction: Option<f64>, // User ratings
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
}
```

#### Error Classification

```rust
use llm_auto_optimizer::decision::pareto::QualityMetrics;

// Track different error types
#[derive(Debug, Clone)]
pub enum QualityIssue {
    LowAccuracy { score: f64, threshold: f64 },
    PoorRelevance { score: f64 },
    IncoherentOutput { score: f64 },
    UserDissatisfaction { rating: f64 },
}

// Example quality checker
fn check_quality(metrics: &QualityMetrics) -> Vec<QualityIssue> {
    let mut issues = Vec::new();

    if metrics.accuracy < 0.8 {
        issues.push(QualityIssue::LowAccuracy {
            score: metrics.accuracy,
            threshold: 0.8,
        });
    }

    if metrics.relevance < 0.75 {
        issues.push(QualityIssue::PoorRelevance {
            score: metrics.relevance,
        });
    }

    if metrics.coherence < 0.7 {
        issues.push(QualityIssue::IncoherentOutput {
            score: metrics.coherence,
        });
    }

    if let Some(satisfaction) = metrics.user_satisfaction {
        if satisfaction < 0.6 {
            issues.push(QualityIssue::UserDissatisfaction {
                rating: satisfaction,
            });
        }
    }

    issues
}
```

#### SLA Monitoring

```rust
use llm_auto_optimizer::threshold_monitor::{ThresholdMonitoringSystem, ThresholdConfig};

// Set up quality SLA monitoring
let monitor = ThresholdMonitoringSystem::new();

// SLA: 90% quality score minimum
monitor.register_metric("quality_score", ThresholdConfig::quality())?;

// Monitor quality in real-time
async fn monitor_quality_sla(monitor: &ThresholdMonitoringSystem) {
    let current_quality = 0.88; // Example score
    let alerts = monitor.record("quality_score", current_quality);

    for alert in alerts {
        match alert.severity {
            AlertSeverity::Critical => {
                println!("🚨 SLA VIOLATION: Quality below minimum threshold");
                println!("  Current: {:.1}% | Required: 90%", current_quality * 100.0);
                println!("  Action: Immediate rollback or model switch");
            }
            AlertSeverity::Warning => {
                println!("⚠️  Quality warning: Approaching SLA limit");
                println!("  Recommendation: Review recent changes");
            }
            _ => {}
        }
    }
}
```

#### Quality Score Interpretation

| Score Range | Interpretation | Action |
|-------------|----------------|--------|
| **0.9 - 1.0** | Excellent | Maintain current configuration |
| **0.8 - 0.9** | Good | Monitor for drift |
| **0.7 - 0.8** | Acceptable | Consider optimization |
| **0.6 - 0.7** | Poor | Investigate and fix |
| **< 0.6** | Critical | Immediate action required |

```rust
fn interpret_quality_score(score: f64) -> &'static str {
    match score {
        s if s >= 0.9 => "Excellent - Maintain current configuration",
        s if s >= 0.8 => "Good - Monitor for drift",
        s if s >= 0.7 => "Acceptable - Consider optimization",
        s if s >= 0.6 => "Poor - Investigate and fix",
        _ => "Critical - Immediate action required",
    }
}
```

---

### 4. Pattern Analyzer

#### Pattern Types Detected

The Pattern Analyzer identifies recurring patterns in your LLM usage and performance.

```rust
#[derive(Debug, Clone)]
pub enum PatternType {
    // Temporal patterns
    DailyPeakHours { hours: Vec<u8> },
    WeeklyTrends { day_weights: Vec<f64> },
    SeasonalVariation { period_days: u32 },

    // Traffic patterns
    BurstTraffic { threshold_multiplier: f64 },
    SteadyState { avg_qps: f64 },
    GradualIncrease { growth_rate: f64 },

    // Performance patterns
    LatencySpikes { frequency: u32 },
    ErrorClusters { window_minutes: u32 },
    DegradationTrend { slope: f64 },
}
```

#### Temporal Analysis

```rust
use llm_auto_optimizer::decision::drift_detection::{ADWIN, DriftStatus};

// Detect temporal patterns with drift detection
async fn analyze_temporal_patterns() -> Result<(), Box<dyn std::error::Error>> {
    let mut drift_detector = ADWIN::new(0.002, 100)?;

    // Simulate hourly request counts
    let hourly_requests = vec![
        100, 120, 150, 200, 350, 500, 450, 300, // Peak hours
        250, 200, 180, 150, 140, 130, 120, 110, // Decline
        100, 95, 90, 85, 80, 75, 70, 80,       // Off-peak
    ];

    for (hour, count) in hourly_requests.iter().enumerate() {
        let status = drift_detector.add(*count as f64);

        match status {
            DriftStatus::Drift => {
                println!("📊 Traffic pattern change detected at hour {}", hour);
                println!("   Previous mean: {:.0}", drift_detector.mean().unwrap_or(0.0));
            }
            DriftStatus::Warning => {
                println!("⚠️  Possible traffic pattern change at hour {}", hour);
            }
            DriftStatus::Stable => {}
        }
    }

    Ok(())
}
```

#### Traffic Patterns

```rust
use llm_auto_optimizer::decision::drift_detection::{CUSUM, DriftStatus};

// Detect traffic anomalies using CUSUM
async fn detect_traffic_anomalies() -> Result<(), Box<dyn std::error::Error>> {
    let baseline_qps = 100.0;
    let mut cusum = CUSUM::new(
        5.0,           // Threshold
        baseline_qps,  // Target mean
        10.0           // Delta (minimum change to detect)
    )?;

    // Monitor traffic in real-time
    let current_qps = vec![
        95.0, 98.0, 102.0, 100.0,  // Normal
        120.0, 135.0, 150.0,        // Gradual increase
        165.0, 180.0, 200.0,        // Burst traffic
    ];

    for qps in current_qps {
        let status = cusum.add(qps);

        match status {
            DriftStatus::Drift => {
                if cusum.drift_direction() == Some(true) {
                    println!("📈 Traffic spike detected: {:.0} QPS", qps);
                    println!("   Recommendation: Scale up capacity");
                } else {
                    println!("📉 Traffic drop detected: {:.0} QPS", qps);
                    println!("   Recommendation: Check for issues");
                }
            }
            _ => {}
        }
    }

    Ok(())
}
```

#### Actionable Insights

```rust
// Example: Pattern-based optimization decisions
#[derive(Debug)]
struct PatternInsight {
    pattern_type: String,
    description: String,
    recommendations: Vec<String>,
}

fn generate_pattern_insights() -> Vec<PatternInsight> {
    vec![
        PatternInsight {
            pattern_type: "Daily Peak Hours".to_string(),
            description: "Traffic peaks between 9 AM - 5 PM EST".to_string(),
            recommendations: vec![
                "Pre-warm caches before 9 AM".to_string(),
                "Scale up instances at 8:30 AM".to_string(),
                "Use faster models during peak hours".to_string(),
            ],
        },
        PatternInsight {
            pattern_type: "Weekend Low Traffic".to_string(),
            description: "40% reduction in traffic on weekends".to_string(),
            recommendations: vec![
                "Scale down to 60% capacity on Sat-Sun".to_string(),
                "Use cheaper models for cost optimization".to_string(),
                "Schedule maintenance windows".to_string(),
            ],
        },
        PatternInsight {
            pattern_type: "Latency Spikes".to_string(),
            description: "Latency spikes every 6 hours".to_string(),
            recommendations: vec![
                "Investigate batch processing jobs".to_string(),
                "Implement request queuing".to_string(),
                "Stagger background tasks".to_string(),
            ],
        },
    ]
}
```

---

### 5. Anomaly Analyzer

#### Anomaly Detection Methods

The Anomaly Analyzer supports multiple statistical methods for detecting outliers:

```rust
use llm_auto_optimizer::decision::anomaly_detection::{
    ZScoreDetector, IQRDetector, MADDetector, MahalanobisDetector,
    AnomalyResult
};

// Method 1: Z-Score Detection (parametric)
let mut zscore = ZScoreDetector::new(
    50,   // Window size
    3.0   // Z-score threshold (3σ)
)?;

// Method 2: IQR (Interquartile Range) - robust to outliers
let mut iqr = IQRDetector::new(
    50,   // Window size
    1.5   // IQR multiplier (1.5 = standard, 3.0 = extreme outliers)
)?;

// Method 3: MAD (Median Absolute Deviation) - most robust
let mut mad = MADDetector::new(
    50,   // Window size
    3.5   // Modified Z-score threshold
)?;

// Method 4: Mahalanobis Distance (multi-dimensional)
let mut mahalanobis = MahalanobisDetector::new(
    30,   // Window size
    3,    // Number of dimensions
    5.0   // Distance threshold
)?;
```

#### Sensitivity Tuning

```rust
// Configure anomaly detection sensitivity
#[derive(Debug, Clone)]
pub struct AnomalyConfig {
    pub method: AnomalyMethod,
    pub sensitivity: Sensitivity,
    pub window_size: usize,
}

#[derive(Debug, Clone)]
pub enum AnomalyMethod {
    ZScore { threshold: f64 },
    IQR { multiplier: f64 },
    MAD { threshold: f64 },
    Mahalanobis { dimensions: usize, threshold: f64 },
}

#[derive(Debug, Clone)]
pub enum Sensitivity {
    Low,      // Fewer false positives, may miss some anomalies
    Medium,   // Balanced
    High,     // More sensitive, more false positives
}

impl Sensitivity {
    fn zscore_threshold(&self) -> f64 {
        match self {
            Sensitivity::Low => 4.0,     // Very strict
            Sensitivity::Medium => 3.0,   // Standard
            Sensitivity::High => 2.0,     // Sensitive
        }
    }

    fn iqr_multiplier(&self) -> f64 {
        match self {
            Sensitivity::Low => 3.0,      // Extreme outliers only
            Sensitivity::Medium => 1.5,   // Standard
            Sensitivity::High => 1.0,     // More sensitive
        }
    }
}

// Example: Tune for production
let production_config = AnomalyConfig {
    method: AnomalyMethod::MAD { threshold: 3.5 },
    sensitivity: Sensitivity::Medium,
    window_size: 100,
};

// Example: Tune for development (more lenient)
let dev_config = AnomalyConfig {
    method: AnomalyMethod::ZScore { threshold: 4.0 },
    sensitivity: Sensitivity::Low,
    window_size: 50,
};
```

#### Alert Configuration

```rust
use llm_auto_optimizer::threshold_monitor::{
    ThresholdMonitoringSystem, ThresholdConfig, Alert, AlertSeverity
};

// Configure anomaly alerting
async fn setup_anomaly_alerts() -> Result<(), Box<dyn std::error::Error>> {
    let monitor = ThresholdMonitoringSystem::new();

    // Register latency anomaly detection
    monitor.register_metric("latency_p95", ThresholdConfig {
        enable_anomaly_detection: true,
        anomaly_threshold: 3.0,
        enable_drift_detection: true,
        drift_algorithm: DriftAlgorithm::ADWIN,
        ..Default::default()
    })?;

    // Simulate latency measurements
    for value in vec![1000.0, 1050.0, 980.0, 5000.0] { // 5000ms is anomaly
        let alerts = monitor.record("latency_p95", value);

        for alert in alerts {
            if alert.alert_type == AlertType::Anomaly {
                println!("🚨 Anomaly detected!");
                println!("   Metric: {}", alert.metric_name);
                println!("   Value: {:.0}ms", alert.value);
                println!("   Severity: {:?}", alert.severity);
                println!("   Score: {:.2}", alert.context);

                // Take action based on severity
                match alert.severity {
                    AlertSeverity::Critical => {
                        println!("   Action: Page on-call engineer");
                        // trigger_pagerduty(&alert).await?;
                    }
                    AlertSeverity::Warning => {
                        println!("   Action: Send Slack notification");
                        // send_slack_alert(&alert).await?;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
```

#### False Positive Handling

```rust
use std::collections::VecDeque;

/// Reduce false positives with confirmation windows
pub struct ConfirmedAnomalyDetector {
    detector: ZScoreDetector,
    confirmation_window: usize,
    recent_anomalies: VecDeque<bool>,
}

impl ConfirmedAnomalyDetector {
    pub fn new(
        window_size: usize,
        threshold: f64,
        confirmation_window: usize,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            detector: ZScoreDetector::new(window_size, threshold)?,
            confirmation_window,
            recent_anomalies: VecDeque::with_capacity(confirmation_window),
        })
    }

    /// Only report anomaly if confirmed by multiple consecutive detections
    pub fn add(&mut self, value: f64) -> bool {
        let result = self.detector.add(value);

        // Track recent anomaly detections
        if self.recent_anomalies.len() >= self.confirmation_window {
            self.recent_anomalies.pop_front();
        }
        self.recent_anomalies.push_back(result.is_anomaly);

        // Require at least 2 out of last 3 to be anomalies
        let anomaly_count = self.recent_anomalies.iter()
            .filter(|&&is_anomaly| is_anomaly)
            .count();

        anomaly_count >= 2
    }
}

// Usage
let mut detector = ConfirmedAnomalyDetector::new(50, 3.0, 3)?;

for value in vec![1000.0, 5000.0, 1050.0, 980.0] {
    let confirmed_anomaly = detector.add(value);

    if confirmed_anomaly {
        println!("✓ Confirmed anomaly: {}", value);
    }
}
```

**Best Practices for Reducing False Positives:**

1. **Use MAD instead of Z-Score**: More robust to existing outliers
2. **Increase window size**: Larger windows = more stable baselines
3. **Confirmation windows**: Require multiple consecutive anomalies
4. **Seasonal adjustments**: Account for known patterns
5. **Multi-method voting**: Combine multiple detection methods

---

## Configuration Reference

### AnalyzerConfig Structure

```rust
use llm_auto_optimizer::decision::drift_detection::DriftAlgorithm;

#[derive(Debug, Clone)]
pub struct AnalyzerConfig {
    // Analyzer toggles
    pub enable_performance: bool,
    pub enable_cost: bool,
    pub enable_quality: bool,
    pub enable_pattern: bool,
    pub enable_anomaly: bool,

    // Performance thresholds
    pub max_latency_p95_ms: f64,
    pub max_latency_p99_ms: f64,
    pub max_error_rate: f64,
    pub min_throughput_qps: f64,

    // Cost thresholds
    pub max_cost_per_request: f64,
    pub cost_budget_daily: f64,
    pub cost_budget_monthly: f64,

    // Quality thresholds
    pub min_quality_score: f64,
    pub min_accuracy: f64,
    pub min_relevance: f64,
    pub min_coherence: f64,

    // Pattern detection
    pub pattern_detection_window_secs: u64,
    pub min_pattern_confidence: f64,

    // Anomaly detection
    pub anomaly_threshold: f64,
    pub anomaly_window_size: usize,
    pub drift_detection_algorithm: DriftAlgorithm,

    // Analysis windows
    pub performance_window_secs: u64,
    pub cost_aggregation_interval_secs: u64,
    pub quality_evaluation_interval_secs: u64,
    pub pattern_analysis_interval_secs: u64,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            enable_performance: true,
            enable_cost: true,
            enable_quality: true,
            enable_pattern: true,
            enable_anomaly: true,

            max_latency_p95_ms: 5000.0,
            max_latency_p99_ms: 10000.0,
            max_error_rate: 0.01,
            min_throughput_qps: 1.0,

            max_cost_per_request: 0.10,
            cost_budget_daily: 1000.0,
            cost_budget_monthly: 30000.0,

            min_quality_score: 0.7,
            min_accuracy: 0.75,
            min_relevance: 0.7,
            min_coherence: 0.7,

            pattern_detection_window_secs: 3600,
            min_pattern_confidence: 0.8,

            anomaly_threshold: 3.0,
            anomaly_window_size: 50,
            drift_detection_algorithm: DriftAlgorithm::ADWIN,

            performance_window_secs: 300,
            cost_aggregation_interval_secs: 3600,
            quality_evaluation_interval_secs: 600,
            pattern_analysis_interval_secs: 900,
        }
    }
}
```

### Per-Analyzer Configuration

#### Performance Analyzer Configuration

```rust
#[derive(Debug, Clone)]
pub struct PerformanceAnalyzerConfig {
    pub latency_percentiles: Vec<u8>,  // e.g., [50, 95, 99]
    pub window_duration_secs: u64,
    pub drift_detection: bool,
    pub drift_algorithm: DriftAlgorithm,
    pub anomaly_detection: bool,
    pub anomaly_threshold: f64,
}

impl Default for PerformanceAnalyzerConfig {
    fn default() -> Self {
        Self {
            latency_percentiles: vec![50, 95, 99],
            window_duration_secs: 300, // 5 minutes
            drift_detection: true,
            drift_algorithm: DriftAlgorithm::CUSUM,
            anomaly_detection: true,
            anomaly_threshold: 2.5,
        }
    }
}
```

#### Cost Analyzer Configuration

```rust
#[derive(Debug, Clone)]
pub struct CostAnalyzerConfig {
    pub aggregation_interval_secs: u64,
    pub budget_alert_threshold: f64,  // Percentage of budget (0.0-1.0)
    pub track_by_model: bool,
    pub track_by_user: bool,
    pub drift_detection: bool,
}

impl Default for CostAnalyzerConfig {
    fn default() -> Self {
        Self {
            aggregation_interval_secs: 3600, // 1 hour
            budget_alert_threshold: 0.8,      // Alert at 80%
            track_by_model: true,
            track_by_user: true,
            drift_detection: true,
        }
    }
}
```

#### Quality Analyzer Configuration

```rust
#[derive(Debug, Clone)]
pub struct QualityAnalyzerConfig {
    pub evaluation_interval_secs: u64,
    pub min_samples_for_evaluation: usize,
    pub quality_components: QualityComponents,
    pub sla_threshold: f64,
}

#[derive(Debug, Clone)]
pub struct QualityComponents {
    pub accuracy_weight: f64,
    pub relevance_weight: f64,
    pub coherence_weight: f64,
    pub user_satisfaction_weight: f64,
}

impl Default for QualityAnalyzerConfig {
    fn default() -> Self {
        Self {
            evaluation_interval_secs: 600, // 10 minutes
            min_samples_for_evaluation: 10,
            quality_components: QualityComponents {
                accuracy_weight: 0.4,
                relevance_weight: 0.3,
                coherence_weight: 0.2,
                user_satisfaction_weight: 0.1,
            },
            sla_threshold: 0.8,
        }
    }
}
```

### Tuning Parameters

| Parameter | Range | Recommended | Impact |
|-----------|-------|-------------|---------|
| `anomaly_threshold` | 1.0 - 5.0 | 2.5 - 3.0 | Higher = fewer false positives |
| `anomaly_window_size` | 20 - 200 | 50 - 100 | Larger = more stable baseline |
| `performance_window_secs` | 60 - 3600 | 300 | Shorter = faster detection |
| `drift_detection_algorithm` | ADWIN, PageHinkley, CUSUM | ADWIN | ADWIN = best for unknown changes |

### Best Practices

1. **Start Conservative**: Begin with higher thresholds (3.0-4.0) and tune down
2. **Use ADWIN**: Best general-purpose drift detector
3. **Match Window to Traffic**: Higher traffic = shorter windows
4. **Monitor All Five**: Each analyzer provides unique insights
5. **Tune Per Environment**: Production vs. Development vs. Staging

---

## Integration Examples

### Example 1: Basic Usage

```rust
use llm_auto_optimizer::threshold_monitor::{
    ThresholdMonitoringSystem, ThresholdConfig
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create monitoring system
    let monitor = ThresholdMonitoringSystem::new();

    // Register metrics
    monitor.register_metric("latency_p95", ThresholdConfig::latency(5000.0))?;
    monitor.register_metric("cost_per_request", ThresholdConfig::cost(0.10))?;
    monitor.register_metric("quality_score", ThresholdConfig::quality())?;

    // Simulate incoming metrics
    loop {
        // Record latency
        let latency = measure_latency().await;
        let alerts = monitor.record("latency_p95", latency);

        for alert in alerts {
            println!("Alert: {}", alert.message);
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}

async fn measure_latency() -> f64 {
    // Your latency measurement logic
    1500.0
}
```

### Example 2: Custom Configuration

```rust
use llm_auto_optimizer::threshold_monitor::{
    ThresholdMonitoringSystem, ThresholdConfig, AlertSeverity
};
use llm_auto_optimizer::decision::drift_detection::DriftAlgorithm;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let monitor = ThresholdMonitoringSystem::new();

    // Custom latency config with tight SLAs
    let latency_config = ThresholdConfig {
        min_value: Some(0.0),
        max_value: Some(3000.0),          // Hard limit: 3s
        warning_min: None,
        warning_max: Some(2000.0),        // Warning: 2s
        enable_drift_detection: true,
        drift_algorithm: DriftAlgorithm::CUSUM,
        enable_anomaly_detection: true,
        anomaly_threshold: 2.0,           // Sensitive detection
    };

    monitor.register_metric("api_latency_p95", latency_config)?;

    // Custom cost config with budget tracking
    let cost_config = ThresholdConfig {
        min_value: Some(0.0),
        max_value: Some(0.05),            // Max $0.05/request
        warning_min: None,
        warning_max: Some(0.04),          // Warning at $0.04
        enable_drift_detection: true,
        drift_algorithm: DriftAlgorithm::PageHinkley,
        enable_anomaly_detection: true,
        anomaly_threshold: 3.0,
    };

    monitor.register_metric("request_cost", cost_config)?;

    // Process metrics with custom handling
    let latency_alerts = monitor.record("api_latency_p95", 2500.0);
    let cost_alerts = monitor.record("request_cost", 0.045);

    // Handle alerts
    for alert in latency_alerts.iter().chain(cost_alerts.iter()) {
        match alert.severity {
            AlertSeverity::Critical => {
                eprintln!("🚨 CRITICAL: {}", alert.message);
                // Trigger incident response
            }
            AlertSeverity::Warning => {
                println!("⚠️  WARNING: {}", alert.message);
                // Log for review
            }
            AlertSeverity::Info => {
                println!("ℹ️  INFO: {}", alert.message);
            }
        }
    }

    Ok(())
}
```

### Example 3: Querying Results

```rust
use llm_auto_optimizer::threshold_monitor::{
    ThresholdMonitoringSystem, Alert, AlertType
};

async fn query_analysis_results(
    monitor: &ThresholdMonitoringSystem
) -> Result<(), Box<dyn std::error::Error>> {
    // Get recent alerts for a metric
    let recent_alerts = monitor.get_recent_alerts("latency_p95");

    println!("Recent alerts: {}", recent_alerts.len());

    // Analyze alert patterns
    let critical_count = recent_alerts.iter()
        .filter(|a| a.severity == AlertSeverity::Critical)
        .count();

    let drift_count = recent_alerts.iter()
        .filter(|a| a.alert_type == AlertType::Drift)
        .count();

    let anomaly_count = recent_alerts.iter()
        .filter(|a| a.alert_type == AlertType::Anomaly)
        .count();

    println!("Analysis:");
    println!("  Critical alerts: {}", critical_count);
    println!("  Drift detections: {}", drift_count);
    println!("  Anomalies: {}", anomaly_count);

    // Generate summary report
    if critical_count > 0 {
        println!("\n⚠️  Action required: {} critical alerts", critical_count);
    }

    if drift_count > 3 {
        println!("\n📊 Pattern change detected: Review recent deployments");
    }

    if anomaly_count > 5 {
        println!("\n🚨 High anomaly rate: Investigate system health");
    }

    Ok(())
}
```

### Example 4: Alert Handling

```rust
use llm_auto_optimizer::threshold_monitor::{
    ThresholdMonitoringSystem, Alert, AlertSeverity, AlertType
};

struct AlertHandler {
    monitor: ThresholdMonitoringSystem,
}

impl AlertHandler {
    fn new() -> Self {
        Self {
            monitor: ThresholdMonitoringSystem::new(),
        }
    }

    async fn setup(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Register metrics
        self.monitor.register_metric(
            "latency_p95",
            ThresholdConfig::latency(5000.0)
        )?;

        Ok(())
    }

    async fn process_metric(&self, name: &str, value: f64) {
        let alerts = self.monitor.record(name, value);

        for alert in alerts {
            self.handle_alert(&alert).await;
        }
    }

    async fn handle_alert(&self, alert: &Alert) {
        match (alert.severity, alert.alert_type) {
            // Critical threshold violations
            (AlertSeverity::Critical, AlertType::ThresholdViolation) => {
                self.trigger_pagerduty(alert).await;
                self.send_slack_alert(alert).await;
                self.log_alert(alert);
            }

            // Performance degradation
            (AlertSeverity::Critical, AlertType::Drift) => {
                self.send_slack_alert(alert).await;
                self.create_incident(alert).await;
                self.log_alert(alert);
            }

            // Anomalies
            (_, AlertType::Anomaly) => {
                self.send_slack_alert(alert).await;
                self.log_alert(alert);
            }

            // Warnings
            (AlertSeverity::Warning, _) => {
                self.log_alert(alert);
            }

            _ => {}
        }
    }

    async fn trigger_pagerduty(&self, alert: &Alert) {
        println!("📟 Paging on-call: {}", alert.message);
        // PagerDuty integration here
    }

    async fn send_slack_alert(&self, alert: &Alert) {
        println!("💬 Slack notification: {}", alert.message);
        // Slack webhook here
    }

    async fn create_incident(&self, alert: &Alert) {
        println!("🎫 Creating incident: {}", alert.message);
        // Incident management system here
    }

    fn log_alert(&self, alert: &Alert) {
        println!("📝 Logging alert: {}", alert.message);
        // Structured logging here
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let handler = AlertHandler::new();
    handler.setup().await?;

    // Process metrics
    handler.process_metric("latency_p95", 6000.0).await;

    Ok(())
}
```

### Example 5: Production Deployment

```rust
use llm_auto_optimizer::{
    threshold_monitor::{ThresholdMonitoringSystem, ThresholdConfig},
    decision::drift_detection::DriftAlgorithm,
};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
struct ProductionAnalyzer {
    monitor: Arc<ThresholdMonitoringSystem>,
    metrics_buffer: Arc<RwLock<Vec<(String, f64)>>>,
}

impl ProductionAnalyzer {
    fn new() -> Self {
        Self {
            monitor: Arc::new(ThresholdMonitoringSystem::new()),
            metrics_buffer: Arc::new(RwLock::new(Vec::new())),
        }
    }

    async fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Performance metrics
        self.monitor.register_metric(
            "latency_p95",
            ThresholdConfig::latency(3000.0)
        )?;

        self.monitor.register_metric(
            "latency_p99",
            ThresholdConfig::latency(8000.0)
        )?;

        self.monitor.register_metric(
            "error_rate",
            ThresholdConfig {
                max_value: Some(0.01),
                warning_max: Some(0.005),
                enable_drift_detection: true,
                drift_algorithm: DriftAlgorithm::PageHinkley,
                ..Default::default()
            }
        )?;

        // Cost metrics
        self.monitor.register_metric(
            "cost_per_request",
            ThresholdConfig::cost(0.05)
        )?;

        // Quality metrics
        self.monitor.register_metric(
            "quality_score",
            ThresholdConfig::quality()
        )?;

        Ok(())
    }

    async fn record_metric(&self, name: String, value: f64) {
        let mut buffer = self.metrics_buffer.write().await;
        buffer.push((name, value));
    }

    async fn process_batch(&self) {
        let mut buffer = self.metrics_buffer.write().await;

        for (name, value) in buffer.drain(..) {
            let alerts = self.monitor.record(&name, value);

            if !alerts.is_empty() {
                // Handle alerts asynchronously
                tokio::spawn(async move {
                    for alert in alerts {
                        eprintln!("Alert: {}", alert.message);
                    }
                });
            }
        }
    }

    async fn health_check(&self) -> bool {
        // Check if all expected metrics are registered
        let expected_metrics = vec![
            "latency_p95",
            "latency_p99",
            "error_rate",
            "cost_per_request",
            "quality_score",
        ];

        expected_metrics.iter()
            .all(|metric| self.monitor.has_metric(metric))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let analyzer = ProductionAnalyzer::new();
    analyzer.initialize().await?;

    // Health check
    if !analyzer.health_check().await {
        eprintln!("Health check failed!");
        return Ok(());
    }

    // Spawn processing loop
    let analyzer_clone = analyzer.clone();
    tokio::spawn(async move {
        loop {
            analyzer_clone.process_batch().await;
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    });

    // Record metrics
    analyzer.record_metric("latency_p95".to_string(), 2500.0).await;
    analyzer.record_metric("cost_per_request".to_string(), 0.03).await;
    analyzer.record_metric("quality_score".to_string(), 0.92).await;

    // Keep running
    tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;

    Ok(())
}
```

---

## API Reference

### Starting/Stopping Analyzers

```rust
use llm_auto_optimizer::threshold_monitor::ThresholdMonitoringSystem;

pub struct AnalyzerEngine {
    monitor: ThresholdMonitoringSystem,
    running: Arc<AtomicBool>,
}

impl AnalyzerEngine {
    /// Create new analyzer engine
    pub fn new() -> Self {
        Self {
            monitor: ThresholdMonitoringSystem::new(),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start the analyzer engine
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.running.store(true, Ordering::SeqCst);
        println!("✓ Analyzer engine started");
        Ok(())
    }

    /// Stop the analyzer engine
    pub async fn stop(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.running.store(false, Ordering::SeqCst);
        println!("✓ Analyzer engine stopped");
        Ok(())
    }

    /// Check if engine is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}
```

### Querying Current Analysis

```rust
use llm_auto_optimizer::threshold_monitor::{ThresholdMonitoringSystem, Alert};

pub struct AnalysisQuery {
    monitor: Arc<ThresholdMonitoringSystem>,
}

impl AnalysisQuery {
    /// Get recent alerts for a metric
    pub fn get_alerts(&self, metric: &str) -> Vec<Alert> {
        self.monitor.get_recent_alerts(metric)
    }

    /// Get all monitored metrics
    pub fn get_metrics(&self) -> Vec<String> {
        self.monitor.get_metrics()
    }

    /// Clear alerts for a metric
    pub fn clear_alerts(&self, metric: &str) {
        self.monitor.clear_alerts(metric);
    }

    /// Reset a metric's monitoring state
    pub fn reset_metric(&self, metric: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.monitor.reset_metric(metric)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }
}
```

### Retrieving Reports

```rust
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisReport {
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub metrics_analyzed: usize,
    pub total_alerts: usize,
    pub critical_alerts: usize,
    pub warnings: usize,
    pub drift_detections: usize,
    pub anomalies_detected: usize,
    pub top_issues: Vec<String>,
}

impl AnalysisReport {
    pub fn generate(monitor: &ThresholdMonitoringSystem) -> Self {
        let metrics = monitor.get_metrics();
        let mut total_alerts = 0;
        let mut critical_alerts = 0;
        let mut warnings = 0;
        let mut drift_detections = 0;
        let mut anomalies_detected = 0;

        for metric in &metrics {
            let alerts = monitor.get_recent_alerts(metric);
            total_alerts += alerts.len();

            for alert in alerts {
                match alert.severity {
                    AlertSeverity::Critical => critical_alerts += 1,
                    AlertSeverity::Warning => warnings += 1,
                    _ => {}
                }

                match alert.alert_type {
                    AlertType::Drift => drift_detections += 1,
                    AlertType::Anomaly => anomalies_detected += 1,
                    _ => {}
                }
            }
        }

        Self {
            period_start: Utc::now() - chrono::Duration::hours(1),
            period_end: Utc::now(),
            metrics_analyzed: metrics.len(),
            total_alerts,
            critical_alerts,
            warnings,
            drift_detections,
            anomalies_detected,
            top_issues: vec![],
        }
    }
}
```

### Exporting Metrics

```rust
use prometheus::{Encoder, TextEncoder, Registry, IntGauge, Histogram};

pub struct MetricsExporter {
    registry: Registry,
    alerts_total: IntGauge,
    latency_histogram: Histogram,
}

impl MetricsExporter {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let registry = Registry::new();

        let alerts_total = IntGauge::new(
            "analyzer_alerts_total",
            "Total number of alerts"
        )?;
        registry.register(Box::new(alerts_total.clone()))?;

        let latency_histogram = Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "llm_latency_seconds",
                "LLM request latency"
            )
        )?;
        registry.register(Box::new(latency_histogram.clone()))?;

        Ok(Self {
            registry,
            alerts_total,
            latency_histogram,
        })
    }

    pub fn record_alert(&self) {
        self.alerts_total.inc();
    }

    pub fn record_latency(&self, latency_ms: f64) {
        self.latency_histogram.observe(latency_ms / 1000.0);
    }

    pub fn export(&self) -> Result<String, Box<dyn std::error::Error>> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer)?;
        Ok(String::from_utf8(buffer)?)
    }
}
```

---

## Monitoring and Observability

### Built-in Metrics

The Analyzer Engine exposes the following metrics:

```rust
// Prometheus metrics
analyzer_alerts_total{severity="critical",type="threshold"} 12
analyzer_alerts_total{severity="warning",type="drift"} 5
analyzer_alerts_total{severity="info",type="anomaly"} 3

analyzer_metrics_processed_total{analyzer="performance"} 1543
analyzer_metrics_processed_total{analyzer="cost"} 1543
analyzer_metrics_processed_total{analyzer="quality"} 256

analyzer_drift_detections_total{algorithm="adwin"} 8
analyzer_anomaly_detections_total{method="zscore"} 15

analyzer_processing_duration_seconds{analyzer="performance"} 0.002
```

### Prometheus Integration

```rust
use prometheus::{Registry, IntCounter, Histogram, Opts, HistogramOpts};
use axum::{Router, routing::get};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create Prometheus registry
    let registry = Registry::new();

    // Register metrics
    let alerts_counter = IntCounter::with_opts(
        Opts::new("analyzer_alerts_total", "Total alerts")
    )?;
    registry.register(Box::new(alerts_counter.clone()))?;

    // Expose metrics endpoint
    let app = Router::new()
        .route("/metrics", get(|| async move {
            let encoder = prometheus::TextEncoder::new();
            let metric_families = registry.gather();
            let mut buffer = Vec::new();
            encoder.encode(&metric_families, &mut buffer).unwrap();
            String::from_utf8(buffer).unwrap()
        }));

    // Start server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:9090").await?;
    axum::serve(listener, app).await?;

    Ok(())
}
```

### Logging

```rust
use tracing::{info, warn, error, debug, instrument};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize structured logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().json())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Analyzer engine starting");

    run_analyzer().await?;

    Ok(())
}

#[instrument]
async fn run_analyzer() -> Result<(), Box<dyn std::error::Error>> {
    let monitor = ThresholdMonitoringSystem::new();

    monitor.register_metric("latency", ThresholdConfig::latency(5000.0))?;

    let alerts = monitor.record("latency", 6000.0);

    for alert in alerts {
        match alert.severity {
            AlertSeverity::Critical => {
                error!(
                    metric = %alert.metric_name,
                    value = alert.value,
                    threshold = alert.threshold,
                    "Critical alert triggered"
                );
            }
            AlertSeverity::Warning => {
                warn!(
                    metric = %alert.metric_name,
                    value = alert.value,
                    "Warning alert triggered"
                );
            }
            _ => {
                debug!(
                    metric = %alert.metric_name,
                    "Info alert triggered"
                );
            }
        }
    }

    Ok(())
}
```

### Debugging Tips

**Enable Debug Logging:**

```bash
export RUST_LOG=llm_auto_optimizer=debug
cargo run
```

**Inspect Analyzer State:**

```rust
use llm_auto_optimizer::decision::anomaly_detection::ZScoreDetector;

let mut detector = ZScoreDetector::new(50, 3.0)?;

for value in test_data {
    let result = detector.add(value);

    // Debug output
    println!("Value: {}", value);
    println!("  Mean: {:?}", detector.mean());
    println!("  StdDev: {:?}", detector.std_dev());
    println!("  Is Anomaly: {}", result.is_anomaly);
    println!("  Score: {:.2}", result.score);
}
```

**Test Drift Detection:**

```rust
use llm_auto_optimizer::decision::drift_detection::ADWIN;

let mut adwin = ADWIN::new(0.002, 100)?;

println!("Testing drift detection...");

// Stable phase
for i in 0..30 {
    let status = adwin.add(100.0);
    println!("  [{}] Stable: {:?}", i, status);
}

// Drift phase
for i in 0..30 {
    let status = adwin.add(150.0);
    println!("  [{}] Drifted: {:?}", i, status);

    if status == DriftStatus::Drift {
        println!("  ✓ Drift detected at sample {}", i);
        break;
    }
}
```

---

## Best Practices

### When to Enable Each Analyzer

| Analyzer | Enable When | Skip When |
|----------|-------------|-----------|
| **Performance** | Always | Never (critical for all deployments) |
| **Cost** | Production, high-volume | Development, low-volume testing |
| **Quality** | Production, user-facing | Internal testing |
| **Pattern** | Established workloads | New deployments (< 1 week) |
| **Anomaly** | Stable baselines exist | Highly variable workloads |

### Performance Tuning

```rust
// High-throughput configuration (>1000 QPS)
let config = AnalyzerConfig {
    performance_window_secs: 60,           // Shorter windows
    anomaly_window_size: 100,              // Larger windows for stability
    anomaly_threshold: 3.5,                // Less sensitive
    cost_aggregation_interval_secs: 300,   // More frequent cost checks
    ..Default::default()
};

// Low-throughput configuration (<10 QPS)
let config = AnalyzerConfig {
    performance_window_secs: 900,          // Longer windows
    anomaly_window_size: 30,               // Smaller windows OK
    anomaly_threshold: 2.5,                // More sensitive
    cost_aggregation_interval_secs: 3600,  // Less frequent
    ..Default::default()
};
```

### Memory Management

```rust
use llm_auto_optimizer::threshold_monitor::ThresholdMonitoringSystem;

// Configure memory-efficient monitoring
let monitor = ThresholdMonitoringSystem::new();

// Regularly clear old alerts to prevent memory growth
tokio::spawn(async move {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;

        for metric in monitor.get_metrics() {
            monitor.clear_alerts(&metric);
        }

        println!("✓ Cleared old alerts");
    }
});
```

### Alert Fatigue Prevention

```rust
use std::time::{Duration, Instant};
use std::collections::HashMap;

pub struct AlertThrottler {
    last_alerts: HashMap<String, Instant>,
    throttle_duration: Duration,
}

impl AlertThrottler {
    pub fn new(throttle_duration: Duration) -> Self {
        Self {
            last_alerts: HashMap::new(),
            throttle_duration,
        }
    }

    pub fn should_alert(&mut self, alert_key: &str) -> bool {
        let now = Instant::now();

        if let Some(last_time) = self.last_alerts.get(alert_key) {
            if now.duration_since(*last_time) < self.throttle_duration {
                return false; // Throttled
            }
        }

        self.last_alerts.insert(alert_key.to_string(), now);
        true
    }
}

// Usage
let mut throttler = AlertThrottler::new(Duration::from_secs(300)); // 5 min

for alert in alerts {
    let key = format!("{}:{}", alert.metric_name, alert.alert_type);

    if throttler.should_alert(&key) {
        send_alert(&alert).await;
    } else {
        println!("Alert throttled: {}", alert.message);
    }
}
```

---

## Troubleshooting

### Common Issues

#### Issue 1: No Alerts Generated

**Symptoms:**
- Metrics recorded but no alerts generated
- Analyzer appears to be running but silent

**Diagnosis:**

```rust
let monitor = ThresholdMonitoringSystem::new();

// Check if metric is registered
if !monitor.has_metric("latency_p95") {
    println!("❌ Metric not registered!");
}

// Check registered metrics
let metrics = monitor.get_metrics();
println!("Registered metrics: {:?}", metrics);

// Verify threshold configuration
monitor.register_metric("latency_p95", ThresholdConfig {
    max_value: Some(5000.0),
    enable_drift_detection: true,
    enable_anomaly_detection: true,
    ..Default::default()
})?;

// Test with extreme value
let alerts = monitor.record("latency_p95", 50000.0);
println!("Test alerts generated: {}", alerts.len());
```

**Solutions:**
1. Ensure metrics are registered before recording
2. Verify threshold values are appropriate
3. Check that analyzers are enabled
4. Confirm values exceed thresholds

#### Issue 2: Too Many False Positives

**Symptoms:**
- Constant anomaly alerts
- Drift detected too frequently
- Alert fatigue

**Solutions:**

```rust
// Solution 1: Increase thresholds
let config = ThresholdConfig {
    anomaly_threshold: 4.0,  // Increased from 3.0
    ..Default::default()
};

// Solution 2: Use MAD instead of Z-Score (more robust)
let mut detector = MADDetector::new(50, 3.5)?;

// Solution 3: Increase window size
let mut detector = ZScoreDetector::new(
    100,  // Increased from 50
    3.0
)?;

// Solution 4: Implement confirmation windows
let mut recent_anomalies = VecDeque::with_capacity(3);

for value in values {
    let result = detector.add(value);
    recent_anomalies.push_back(result.is_anomaly);

    if recent_anomalies.len() > 3 {
        recent_anomalies.pop_front();
    }

    // Only alert if 2 out of last 3 are anomalies
    let anomaly_count = recent_anomalies.iter()
        .filter(|&&is_anom| is_anom)
        .count();

    if anomaly_count >= 2 {
        println!("Confirmed anomaly!");
    }
}
```

### Performance Problems

**Issue: High CPU Usage**

```rust
// Check processing frequency
let start = Instant::now();
let iterations = 10000;

for _ in 0..iterations {
    monitor.record("latency", 1000.0);
}

let duration = start.elapsed();
println!("Processing rate: {:.0} ops/sec",
    iterations as f64 / duration.as_secs_f64());

// If slow, reduce analyzer frequency
let config = AnalyzerConfig {
    performance_window_secs: 600,  // Increase from 300
    ..Default::default()
};
```

**Issue: High Memory Usage**

```rust
use std::mem::size_of_val;

// Check memory usage
let monitor = ThresholdMonitoringSystem::new();

// Register many metrics
for i in 0..100 {
    monitor.register_metric(
        &format!("metric_{}", i),
        ThresholdConfig::default()
    )?;
}

// Record lots of data
for i in 0..1000 {
    for j in 0..100 {
        monitor.record(&format!("metric_{}", j), i as f64);
    }
}

// Clear periodically
for i in 0..100 {
    monitor.clear_alerts(&format!("metric_{}", i));
}
```

### Alert Tuning

**Scenario: Latency Spikes During Deployment**

```rust
// Temporarily relax thresholds during deployment
async fn deploy_with_relaxed_thresholds() {
    let monitor = ThresholdMonitoringSystem::new();

    // Normal thresholds
    monitor.register_metric("latency", ThresholdConfig::latency(3000.0))?;

    // Start deployment
    println!("Starting deployment...");

    // Relax thresholds
    monitor.reset_metric("latency")?;
    monitor.register_metric("latency", ThresholdConfig {
        max_value: Some(10000.0),  // Temporarily higher
        ..Default::default()
    })?;

    // Wait for deployment
    tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;

    // Restore normal thresholds
    monitor.reset_metric("latency")?;
    monitor.register_metric("latency", ThresholdConfig::latency(3000.0))?;

    println!("Deployment complete, thresholds restored");
}
```

---

## Advanced Topics

### Custom Analyzers

Create your own analyzer by implementing the analyzer pattern:

```rust
use llm_auto_optimizer::decision::anomaly_detection::AnomalyResult;

pub struct CustomAnalyzer {
    threshold: f64,
    history: Vec<f64>,
}

impl CustomAnalyzer {
    pub fn new(threshold: f64) -> Self {
        Self {
            threshold,
            history: Vec::new(),
        }
    }

    pub fn analyze(&mut self, value: f64) -> AnomalyResult {
        self.history.push(value);

        // Custom logic: check if value differs from median by threshold
        if self.history.len() < 5 {
            return AnomalyResult::normal(0.0);
        }

        let mut sorted = self.history.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = sorted[sorted.len() / 2];

        let deviation = (value - median).abs();

        if deviation > self.threshold {
            AnomalyResult::anomaly(deviation, deviation / self.threshold)
        } else {
            AnomalyResult::normal(deviation)
        }
    }
}
```

### Extending the Engine

Add new metrics and analyzers:

```rust
use llm_auto_optimizer::threshold_monitor::{
    ThresholdMonitoringSystem, ThresholdConfig
};

pub struct ExtendedAnalyzerEngine {
    monitor: ThresholdMonitoringSystem,
}

impl ExtendedAnalyzerEngine {
    pub fn new() -> Self {
        Self {
            monitor: ThresholdMonitoringSystem::new(),
        }
    }

    pub async fn setup_custom_metrics(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Standard metrics
        self.monitor.register_metric("latency", ThresholdConfig::latency(5000.0))?;

        // Custom metric: token efficiency
        self.monitor.register_metric("tokens_per_second", ThresholdConfig {
            min_value: Some(10.0),
            max_value: Some(1000.0),
            enable_drift_detection: true,
            ..Default::default()
        })?;

        // Custom metric: cache hit rate
        self.monitor.register_metric("cache_hit_rate", ThresholdConfig {
            min_value: Some(0.5),
            max_value: Some(1.0),
            warning_min: Some(0.7),
            ..Default::default()
        })?;

        Ok(())
    }
}
```

### Integration Patterns

#### Pattern 1: Event-Driven Architecture

```rust
use tokio::sync::mpsc;

pub struct EventDrivenAnalyzer {
    tx: mpsc::Sender<AnalysisEvent>,
}

pub struct AnalysisEvent {
    pub metric: String,
    pub value: f64,
    pub timestamp: DateTime<Utc>,
}

impl EventDrivenAnalyzer {
    pub fn new() -> (Self, mpsc::Receiver<AnalysisEvent>) {
        let (tx, rx) = mpsc::channel(1000);
        (Self { tx }, rx)
    }

    pub async fn record(&self, metric: String, value: f64) {
        let event = AnalysisEvent {
            metric,
            value,
            timestamp: Utc::now(),
        };

        let _ = self.tx.send(event).await;
    }
}

// Consumer
async fn process_events(mut rx: mpsc::Receiver<AnalysisEvent>) {
    let monitor = ThresholdMonitoringSystem::new();

    while let Some(event) = rx.recv().await {
        let alerts = monitor.record(&event.metric, event.value);

        for alert in alerts {
            println!("Alert: {}", alert.message);
        }
    }
}
```

#### Pattern 2: Batch Processing

```rust
pub struct BatchAnalyzer {
    monitor: ThresholdMonitoringSystem,
    batch_size: usize,
    batch: Vec<(String, f64)>,
}

impl BatchAnalyzer {
    pub fn new(batch_size: usize) -> Self {
        Self {
            monitor: ThresholdMonitoringSystem::new(),
            batch_size,
            batch: Vec::with_capacity(batch_size),
        }
    }

    pub async fn record(&mut self, metric: String, value: f64) {
        self.batch.push((metric, value));

        if self.batch.len() >= self.batch_size {
            self.flush().await;
        }
    }

    pub async fn flush(&mut self) {
        for (metric, value) in self.batch.drain(..) {
            let alerts = self.monitor.record(&metric, value);

            for alert in alerts {
                println!("Alert: {}", alert.message);
            }
        }
    }
}
```

#### Pattern 3: Streaming Pipeline

```rust
use tokio_stream::{Stream, StreamExt};

pub struct StreamingAnalyzer {
    monitor: Arc<ThresholdMonitoringSystem>,
}

impl StreamingAnalyzer {
    pub fn new() -> Self {
        Self {
            monitor: Arc::new(ThresholdMonitoringSystem::new()),
        }
    }

    pub async fn process_stream<S>(&self, mut stream: S)
    where
        S: Stream<Item = (String, f64)> + Unpin,
    {
        while let Some((metric, value)) = stream.next().await {
            let alerts = self.monitor.record(&metric, value);

            for alert in alerts {
                println!("Streaming alert: {}", alert.message);
            }
        }
    }
}
```

---

## Conclusion

The Analyzer Engine provides comprehensive monitoring and analysis capabilities for LLM infrastructure. By combining performance tracking, cost monitoring, quality assessment, pattern detection, and anomaly detection, it enables data-driven optimization and proactive issue detection.

**Key Takeaways:**

1. Enable all five analyzers in production for comprehensive coverage
2. Start with conservative thresholds and tune based on your workload
3. Use ADWIN for drift detection and MAD for anomaly detection
4. Implement alert throttling to prevent fatigue
5. Monitor analyzer performance and memory usage
6. Integrate with existing observability stack (Prometheus, Grafana)

**Next Steps:**

- Review [Decision Engine Guide](DECISION_ENGINE_USER_GUIDE.md) for optimization strategies
- Check [Deployment Guide](DEPLOYMENT_GUIDE.md) for production setup
- Explore [Integration Examples](INTEGRATION_CODE_EXAMPLES.md) for specific use cases

---

**Generated by LLM Auto Optimizer Documentation Team**
Version 1.0.0 | Last Updated: 2025-01-10
