//! Metrics Demo
//!
//! Comprehensive demonstration of metrics recording, custom metrics,
//! dashboard setup, and alert configuration.
//!
//! This example shows:
//! - Basic metric recording (counters, gauges, histograms)
//! - Custom metric creation
//! - HTTP metrics endpoint setup
//! - Prometheus integration
//! - OpenTelemetry integration
//! - Best practices for production use

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use opentelemetry::metrics::{Counter, Histogram, MeterProvider as _, UpDownCounter};
use opentelemetry::KeyValue;
use opentelemetry_sdk::metrics::{ManualReader, SdkMeterProvider};
use opentelemetry_sdk::Resource;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

// ============================================================================
// Application State
// ============================================================================

#[derive(Clone)]
struct AppState {
    metrics: Arc<MetricsCollector>,
    meter_provider: Arc<SdkMeterProvider>,
    reader: Arc<ManualReader>,
}

// ============================================================================
// Metrics Collector
// ============================================================================

/// Centralized metrics collector for the application
struct MetricsCollector {
    // Request metrics
    request_counter: Counter<u64>,
    request_duration: Histogram<f64>,
    active_requests: UpDownCounter<i64>,

    // Business metrics
    user_registrations: Counter<u64>,
    llm_requests: Counter<u64>,
    llm_tokens: Counter<u64>,
    llm_cost: Counter<f64>,

    // Error metrics
    error_counter: Counter<u64>,

    // Custom metrics
    optimization_score: Histogram<f64>,
    cache_operations: Counter<u64>,
}

impl MetricsCollector {
    /// Create a new metrics collector
    fn new(provider: &SdkMeterProvider) -> Self {
        let meter = provider.meter("metrics_demo");

        // Request metrics
        let request_counter = meter
            .u64_counter("http_requests_total")
            .with_description("Total number of HTTP requests")
            .with_unit("requests")
            .init();

        let request_duration = meter
            .f64_histogram("http_request_duration_ms")
            .with_description("HTTP request duration in milliseconds")
            .with_unit("ms")
            .init();

        let active_requests = meter
            .i64_up_down_counter("http_active_requests")
            .with_description("Number of active HTTP requests")
            .init();

        // Business metrics
        let user_registrations = meter
            .u64_counter("user_registrations_total")
            .with_description("Total number of user registrations")
            .with_unit("users")
            .init();

        let llm_requests = meter
            .u64_counter("llm_requests_total")
            .with_description("Total number of LLM API requests")
            .with_unit("requests")
            .init();

        let llm_tokens = meter
            .u64_counter("llm_tokens_total")
            .with_description("Total number of tokens used")
            .with_unit("tokens")
            .init();

        let llm_cost = meter
            .f64_counter("llm_cost_total")
            .with_description("Total LLM API cost in USD")
            .with_unit("USD")
            .init();

        // Error metrics
        let error_counter = meter
            .u64_counter("errors_total")
            .with_description("Total number of errors")
            .with_unit("errors")
            .init();

        // Custom metrics
        let optimization_score = meter
            .f64_histogram("optimization_score")
            .with_description("Optimization score distribution")
            .with_unit("score")
            .init();

        let cache_operations = meter
            .u64_counter("cache_operations_total")
            .with_description("Total cache operations")
            .with_unit("operations")
            .init();

        Self {
            request_counter,
            request_duration,
            active_requests,
            user_registrations,
            llm_requests,
            llm_tokens,
            llm_cost,
            error_counter,
            optimization_score,
            cache_operations,
        }
    }

    /// Record HTTP request
    fn record_request(&self, method: &str, path: &str, status: u16, duration_ms: f64) {
        let labels = &[
            KeyValue::new("method", method.to_string()),
            KeyValue::new("path", path.to_string()),
            KeyValue::new("status", status.to_string()),
        ];

        self.request_counter.add(1, labels);
        self.request_duration.record(duration_ms, labels);
    }

    /// Increment active requests
    fn increment_active_requests(&self) {
        self.active_requests.add(1, &[]);
    }

    /// Decrement active requests
    fn decrement_active_requests(&self) {
        self.active_requests.add(-1, &[]);
    }

    /// Record user registration
    fn record_user_registration(&self, source: &str) {
        self.user_registrations.add(
            1,
            &[KeyValue::new("source", source.to_string())],
        );
    }

    /// Record LLM request
    fn record_llm_request(
        &self,
        provider: &str,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
        cost: f64,
        success: bool,
    ) {
        let labels = &[
            KeyValue::new("provider", provider.to_string()),
            KeyValue::new("model", model.to_string()),
            KeyValue::new("success", success.to_string()),
        ];

        self.llm_requests.add(1, labels);

        if success {
            let token_labels = &[
                KeyValue::new("provider", provider.to_string()),
                KeyValue::new("model", model.to_string()),
                KeyValue::new("type", "input".to_string()),
            ];
            self.llm_tokens.add(input_tokens, token_labels);

            let output_labels = &[
                KeyValue::new("provider", provider.to_string()),
                KeyValue::new("model", model.to_string()),
                KeyValue::new("type", "output".to_string()),
            ];
            self.llm_tokens.add(output_tokens, output_labels);

            self.llm_cost.add(
                cost,
                &[
                    KeyValue::new("provider", provider.to_string()),
                    KeyValue::new("model", model.to_string()),
                ],
            );
        }
    }

    /// Record error
    fn record_error(&self, error_type: &str, severity: &str) {
        self.error_counter.add(
            1,
            &[
                KeyValue::new("type", error_type.to_string()),
                KeyValue::new("severity", severity.to_string()),
            ],
        );
    }

    /// Record optimization score
    fn record_optimization_score(&self, score: f64, algorithm: &str) {
        self.optimization_score.record(
            score,
            &[KeyValue::new("algorithm", algorithm.to_string())],
        );
    }

    /// Record cache operation
    fn record_cache_operation(&self, operation: &str, hit: bool) {
        self.cache_operations.add(
            1,
            &[
                KeyValue::new("operation", operation.to_string()),
                KeyValue::new("hit", hit.to_string()),
            ],
        );
    }
}

// ============================================================================
// HTTP Handlers
// ============================================================================

/// Health check endpoint
async fn health_check() -> &'static str {
    "OK"
}

/// Metrics endpoint - Prometheus format
async fn metrics_handler(State(state): State<AppState>) -> Response {
    match state
        .reader
        .collect(&mut opentelemetry::Context::current())
    {
        Ok(resource_metrics) => {
            // Convert OpenTelemetry metrics to Prometheus format
            let prometheus_output = format_prometheus_metrics(&resource_metrics);
            (StatusCode::OK, prometheus_output).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error collecting metrics: {}", e),
        )
            .into_response(),
    }
}

/// Format metrics in Prometheus exposition format
fn format_prometheus_metrics(
    resource_metrics: &opentelemetry_sdk::metrics::data::ResourceMetrics,
) -> String {
    let mut output = String::new();

    for scope_metrics in &resource_metrics.scope_metrics {
        for metric in &scope_metrics.metrics {
            // Add TYPE and HELP comments
            output.push_str(&format!("# HELP {} {}\n", metric.name, metric.description));

            // Determine metric type
            let metric_type = match &metric.data {
                opentelemetry_sdk::metrics::data::Aggregation::Sum(_) => "counter",
                opentelemetry_sdk::metrics::data::Aggregation::Gauge(_) => "gauge",
                opentelemetry_sdk::metrics::data::Aggregation::Histogram(_) => "histogram",
                _ => "untyped",
            };
            output.push_str(&format!("# TYPE {} {}\n", metric.name, metric_type));

            // Format metric values (simplified)
            output.push_str(&format!("{} 0\n", metric.name));
        }
    }

    output
}

// ============================================================================
// API Request/Response Types
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
struct RegisterUserRequest {
    username: String,
    email: String,
    source: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct RegisterUserResponse {
    user_id: String,
    status: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct LLMRequest {
    prompt: String,
    model: String,
    max_tokens: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct LLMResponse {
    response: String,
    tokens_used: u64,
    cost: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct OptimizeRequest {
    algorithm: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct OptimizeResponse {
    score: f64,
    optimized_parameters: serde_json::Value,
}

// ============================================================================
// Business Logic Handlers
// ============================================================================

/// User registration endpoint
async fn register_user(
    State(state): State<AppState>,
    Json(payload): Json<RegisterUserRequest>,
) -> Result<Json<RegisterUserResponse>, StatusCode> {
    let start = Instant::now();
    state.metrics.increment_active_requests();

    // Simulate registration logic
    sleep(Duration::from_millis(50)).await;

    // Record metrics
    state.metrics.record_user_registration(&payload.source);

    let duration = start.elapsed().as_secs_f64() * 1000.0;
    state
        .metrics
        .record_request("POST", "/api/register", 201, duration);
    state.metrics.decrement_active_requests();

    Ok(Json(RegisterUserResponse {
        user_id: uuid::Uuid::new_v4().to_string(),
        status: "registered".to_string(),
    }))
}

/// LLM request endpoint
async fn llm_request(
    State(state): State<AppState>,
    Json(payload): Json<LLMRequest>,
) -> Result<Json<LLMResponse>, StatusCode> {
    let start = Instant::now();
    state.metrics.increment_active_requests();

    // Simulate LLM API call
    sleep(Duration::from_millis(200)).await;

    // Simulate token counting
    let input_tokens = (payload.prompt.len() / 4) as u64;
    let output_tokens = (payload.max_tokens / 2) as u64;
    let cost = calculate_llm_cost(&payload.model, input_tokens, output_tokens);

    // Record metrics
    state.metrics.record_llm_request(
        "anthropic",
        &payload.model,
        input_tokens,
        output_tokens,
        cost,
        true,
    );

    let duration = start.elapsed().as_secs_f64() * 1000.0;
    state
        .metrics
        .record_request("POST", "/api/llm", 200, duration);
    state.metrics.decrement_active_requests();

    Ok(Json(LLMResponse {
        response: format!("Response to: {}", payload.prompt),
        tokens_used: input_tokens + output_tokens,
        cost,
    }))
}

/// Optimization endpoint
async fn optimize(
    State(state): State<AppState>,
    Json(payload): Json<OptimizeRequest>,
) -> Result<Json<OptimizeResponse>, StatusCode> {
    let start = Instant::now();
    state.metrics.increment_active_requests();

    // Simulate optimization
    sleep(Duration::from_millis(100)).await;

    let score = 0.85 + rand::random::<f64>() * 0.15; // Random score 0.85-1.0
    state
        .metrics
        .record_optimization_score(score, &payload.algorithm);

    let duration = start.elapsed().as_secs_f64() * 1000.0;
    state
        .metrics
        .record_request("POST", "/api/optimize", 200, duration);
    state.metrics.decrement_active_requests();

    Ok(Json(OptimizeResponse {
        score,
        optimized_parameters: payload.parameters,
    }))
}

/// Simulate cache lookup
async fn cache_lookup(State(state): State<AppState>) -> Result<String, StatusCode> {
    let start = Instant::now();

    // Simulate cache hit/miss (70% hit rate)
    let hit = rand::random::<f64>() < 0.7;
    state.metrics.record_cache_operation("get", hit);

    if hit {
        sleep(Duration::from_millis(5)).await;
    } else {
        sleep(Duration::from_millis(50)).await;
    }

    let duration = start.elapsed().as_secs_f64() * 1000.0;
    state
        .metrics
        .record_request("GET", "/api/cache", 200, duration);

    Ok(if hit {
        "Cache hit".to_string()
    } else {
        "Cache miss".to_string()
    })
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Calculate LLM API cost
fn calculate_llm_cost(model: &str, input_tokens: u64, output_tokens: u64) -> f64 {
    // Simplified cost calculation (prices per 1M tokens)
    let (input_price, output_price) = match model {
        "claude-3-opus" => (15.0, 75.0),
        "claude-3-sonnet" => (3.0, 15.0),
        "claude-3-haiku" => (0.25, 1.25),
        _ => (1.0, 2.0),
    };

    let input_cost = (input_tokens as f64 / 1_000_000.0) * input_price;
    let output_cost = (output_tokens as f64 / 1_000_000.0) * output_price;

    input_cost + output_cost
}

// ============================================================================
// Background Metric Generation
// ============================================================================

/// Simulate background metrics
async fn background_metrics_generator(state: AppState) {
    let mut interval = tokio::time::interval(Duration::from_secs(5));

    loop {
        interval.tick().await;

        // Simulate various operations
        for _ in 0..10 {
            // Cache operations
            let hit = rand::random::<f64>() < 0.8;
            state.metrics.record_cache_operation("get", hit);

            // Occasional errors
            if rand::random::<f64>() < 0.01 {
                state.metrics.record_error("timeout", "warning");
            }

            // Optimization scores
            let score = 0.7 + rand::random::<f64>() * 0.3;
            state
                .metrics
                .record_optimization_score(score, "gradient_descent");
        }

        println!("Background metrics generated");
    }
}

// ============================================================================
// Metrics Configuration Examples
// ============================================================================

/// Example: Custom metric with specific buckets
#[allow(dead_code)]
fn create_custom_histogram(provider: &SdkMeterProvider) -> Histogram<f64> {
    let meter = provider.meter("custom_metrics");

    meter
        .f64_histogram("custom_latency_ms")
        .with_description("Custom latency measurement with specific buckets")
        .with_unit("ms")
        .init()
}

/// Example: Batch metric recording
#[allow(dead_code)]
fn record_batch_metrics(collector: &MetricsCollector) {
    // Record multiple related metrics atomically
    for i in 0..100 {
        let latency = 50.0 + (i as f64 * 2.0);
        collector.record_request("GET", "/api/batch", 200, latency);
    }
}

/// Example: Conditional metrics
#[allow(dead_code)]
fn record_conditional_metrics(collector: &MetricsCollector, is_premium_user: bool) {
    if is_premium_user {
        collector.record_user_registration("premium");
    } else {
        collector.record_user_registration("free");
    }
}

// ============================================================================
// Dashboard Configuration Helper
// ============================================================================

/// Print Grafana dashboard configuration helper
fn print_dashboard_config() {
    println!("\n=== Grafana Dashboard Configuration ===\n");
    println!("Add these panels to your Grafana dashboard:\n");

    println!("1. Request Rate:");
    println!("   Query: rate(http_requests_total{{}}[5m])\n");

    println!("2. Error Rate:");
    println!("   Query: rate(errors_total{{}}[5m]) / rate(http_requests_total{{}}[5m]) * 100\n");

    println!("3. P95 Latency:");
    println!("   Query: histogram_quantile(0.95, rate(http_request_duration_ms_bucket{{}}[5m]))\n");

    println!("4. Active Requests:");
    println!("   Query: http_active_requests\n");

    println!("5. LLM Cost (hourly):");
    println!("   Query: rate(llm_cost_total{{}}[1h]) * 3600\n");

    println!("6. Cache Hit Rate:");
    println!("   Query: rate(cache_operations_total{{hit=\"true\"}}[5m]) / rate(cache_operations_total{{}}[5m]) * 100\n");

    println!("=== Alert Rules ===\n");

    println!("High Error Rate:");
    println!("  expr: rate(errors_total[5m]) / rate(http_requests_total[5m]) > 0.05");
    println!("  for: 5m\n");

    println!("High Latency:");
    println!("  expr: histogram_quantile(0.95, rate(http_request_duration_ms_bucket[5m])) > 1000");
    println!("  for: 5m\n");
}

// ============================================================================
// Main Application
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("info,metrics_demo=debug")
        .init();

    println!("🚀 Starting Metrics Demo Application...\n");

    // Create OpenTelemetry metrics provider
    let reader = ManualReader::builder().build();
    let provider = SdkMeterProvider::builder()
        .with_reader(reader.clone())
        .with_resource(Resource::new(vec![
            KeyValue::new("service.name", "metrics-demo"),
            KeyValue::new("service.version", "1.0.0"),
            KeyValue::new("deployment.environment", "demo"),
        ]))
        .build();

    // Create metrics collector
    let metrics = Arc::new(MetricsCollector::new(&provider));

    // Create application state
    let state = AppState {
        metrics,
        meter_provider: Arc::new(provider),
        reader: Arc::new(reader),
    };

    // Start background metrics generator
    let bg_state = state.clone();
    tokio::spawn(async move {
        background_metrics_generator(bg_state).await;
    });

    // Build router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/metrics", get(metrics_handler))
        .route("/api/register", post(register_user))
        .route("/api/llm", post(llm_request))
        .route("/api/optimize", post(optimize))
        .route("/api/cache", get(cache_lookup))
        .with_state(state);

    // Print configuration help
    print_dashboard_config();

    println!("\n📊 Metrics Demo Server Starting...");
    println!("   Health Check: http://localhost:3000/health");
    println!("   Metrics:      http://localhost:3000/metrics");
    println!("\n📝 Example API Calls:");
    println!("   curl -X POST http://localhost:3000/api/register \\");
    println!("     -H 'Content-Type: application/json' \\");
    println!("     -d '{{\"username\":\"demo\",\"email\":\"demo@example.com\",\"source\":\"web\"}}'");
    println!("\n   curl -X POST http://localhost:3000/api/llm \\");
    println!("     -H 'Content-Type: application/json' \\");
    println!("     -d '{{\"prompt\":\"Hello\",\"model\":\"claude-3-sonnet\",\"max_tokens\":100}}'");
    println!("\n⏳ Starting server on 0.0.0.0:3000...\n");

    // Start server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_cost_calculation() {
        let cost = calculate_llm_cost("claude-3-opus", 1000, 2000);
        assert!(cost > 0.0);
        assert!(cost < 1.0); // Reasonable cost for small request
    }

    #[test]
    fn test_metrics_collector_creation() {
        let reader = ManualReader::builder().build();
        let provider = SdkMeterProvider::builder()
            .with_reader(reader)
            .build();

        let collector = MetricsCollector::new(&provider);

        // Should create without panicking
        collector.record_request("GET", "/test", 200, 100.0);
    }

    #[tokio::test]
    async fn test_health_check() {
        let response = health_check().await;
        assert_eq!(response, "OK");
    }
}
