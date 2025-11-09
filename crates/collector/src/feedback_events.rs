//! Feedback Event Types and Schemas
//!
//! This module defines the event types for collecting user feedback,
//! performance metrics, and system telemetry.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Feedback event type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FeedbackEvent {
    /// User provided explicit feedback
    UserFeedback(UserFeedbackEvent),
    /// Performance metrics collected
    PerformanceMetrics(PerformanceMetricsEvent),
    /// Model response received
    ModelResponse(ModelResponseEvent),
    /// System health event
    SystemHealth(SystemHealthEvent),
    /// Experiment result
    ExperimentResult(ExperimentResultEvent),
}

impl FeedbackEvent {
    /// Get event ID
    pub fn id(&self) -> &Uuid {
        match self {
            FeedbackEvent::UserFeedback(e) => &e.id,
            FeedbackEvent::PerformanceMetrics(e) => &e.id,
            FeedbackEvent::ModelResponse(e) => &e.id,
            FeedbackEvent::SystemHealth(e) => &e.id,
            FeedbackEvent::ExperimentResult(e) => &e.id,
        }
    }

    /// Get event timestamp
    pub fn timestamp(&self) -> &DateTime<Utc> {
        match self {
            FeedbackEvent::UserFeedback(e) => &e.timestamp,
            FeedbackEvent::PerformanceMetrics(e) => &e.timestamp,
            FeedbackEvent::ModelResponse(e) => &e.timestamp,
            FeedbackEvent::SystemHealth(e) => &e.timestamp,
            FeedbackEvent::ExperimentResult(e) => &e.timestamp,
        }
    }

    /// Get event type as string
    pub fn event_type(&self) -> &'static str {
        match self {
            FeedbackEvent::UserFeedback(_) => "user_feedback",
            FeedbackEvent::PerformanceMetrics(_) => "performance_metrics",
            FeedbackEvent::ModelResponse(_) => "model_response",
            FeedbackEvent::SystemHealth(_) => "system_health",
            FeedbackEvent::ExperimentResult(_) => "experiment_result",
        }
    }
}

/// User feedback event
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserFeedbackEvent {
    /// Event ID
    pub id: Uuid,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// User ID (optional, may be anonymous)
    pub user_id: Option<String>,
    /// Session ID
    pub session_id: String,
    /// Request ID that this feedback is for
    pub request_id: Uuid,
    /// Explicit rating (e.g., 1-5)
    pub rating: Option<f64>,
    /// Binary feedback (thumbs up/down)
    pub liked: Option<bool>,
    /// Task completed successfully
    pub task_completed: bool,
    /// Response was edited by user
    pub response_edited: bool,
    /// Response was regenerated
    pub response_regenerated: bool,
    /// Dwell time in seconds
    pub dwell_time_seconds: Option<f64>,
    /// Response was saved
    pub response_saved: bool,
    /// Response was shared
    pub response_shared: bool,
    /// Free-form comment
    pub comment: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl UserFeedbackEvent {
    /// Create new user feedback event
    pub fn new(session_id: impl Into<String>, request_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            user_id: None,
            session_id: session_id.into(),
            request_id,
            rating: None,
            liked: None,
            task_completed: false,
            response_edited: false,
            response_regenerated: false,
            dwell_time_seconds: None,
            response_saved: false,
            response_shared: false,
            comment: None,
            metadata: HashMap::new(),
        }
    }

    /// Set user ID
    pub fn with_user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Set rating
    pub fn with_rating(mut self, rating: f64) -> Self {
        self.rating = Some(rating);
        self
    }

    /// Set liked status
    pub fn with_liked(mut self, liked: bool) -> Self {
        self.liked = Some(liked);
        self
    }

    /// Set task completed
    pub fn with_task_completed(mut self, completed: bool) -> Self {
        self.task_completed = completed;
        self
    }
}

/// Performance metrics event
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerformanceMetricsEvent {
    /// Event ID
    pub id: Uuid,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Request ID
    pub request_id: Uuid,
    /// Model used
    pub model: String,
    /// Configuration ID (if from A/B test)
    pub config_id: Option<Uuid>,
    /// Latency in milliseconds
    pub latency_ms: f64,
    /// Input tokens
    pub input_tokens: usize,
    /// Output tokens
    pub output_tokens: usize,
    /// Cost in dollars
    pub cost: f64,
    /// Quality score (if available)
    pub quality_score: Option<f64>,
    /// Error occurred
    pub error: Option<String>,
    /// Additional metrics
    pub metrics: HashMap<String, f64>,
}

impl PerformanceMetricsEvent {
    /// Create new performance metrics event
    pub fn new(
        request_id: Uuid,
        model: impl Into<String>,
        latency_ms: f64,
        input_tokens: usize,
        output_tokens: usize,
        cost: f64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            request_id,
            model: model.into(),
            config_id: None,
            latency_ms,
            input_tokens,
            output_tokens,
            cost,
            quality_score: None,
            error: None,
            metrics: HashMap::new(),
        }
    }

    /// Set quality score
    pub fn with_quality_score(mut self, score: f64) -> Self {
        self.quality_score = Some(score);
        self
    }

    /// Set config ID
    pub fn with_config_id(mut self, config_id: Uuid) -> Self {
        self.config_id = Some(config_id);
        self
    }

    /// Set error
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }
}

/// Model response event
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelResponseEvent {
    /// Event ID
    pub id: Uuid,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Request ID
    pub request_id: Uuid,
    /// Session ID
    pub session_id: String,
    /// Model used
    pub model: String,
    /// Prompt (truncated for privacy)
    pub prompt_hash: String,
    /// Response (truncated)
    pub response_hash: String,
    /// Response length
    pub response_length: usize,
    /// Configuration parameters
    pub config: HashMap<String, serde_json::Value>,
    /// Context information
    pub context: HashMap<String, String>,
}

impl ModelResponseEvent {
    /// Create new model response event
    pub fn new(
        request_id: Uuid,
        session_id: impl Into<String>,
        model: impl Into<String>,
        prompt_hash: impl Into<String>,
        response_hash: impl Into<String>,
        response_length: usize,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            request_id,
            session_id: session_id.into(),
            model: model.into(),
            prompt_hash: prompt_hash.into(),
            response_hash: response_hash.into(),
            response_length,
            config: HashMap::new(),
            context: HashMap::new(),
        }
    }
}

/// System health event
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SystemHealthEvent {
    /// Event ID
    pub id: Uuid,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Component name
    pub component: String,
    /// Health status
    pub status: HealthStatus,
    /// CPU usage percentage
    pub cpu_usage: Option<f64>,
    /// Memory usage in bytes
    pub memory_usage: Option<u64>,
    /// Active requests
    pub active_requests: Option<usize>,
    /// Queue depth
    pub queue_depth: Option<usize>,
    /// Error rate (errors per second)
    pub error_rate: Option<f64>,
    /// Additional metrics
    pub metrics: HashMap<String, f64>,
}

/// Health status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    /// Healthy
    Healthy,
    /// Degraded
    Degraded,
    /// Unhealthy
    Unhealthy,
}

impl SystemHealthEvent {
    /// Create new system health event
    pub fn new(component: impl Into<String>, status: HealthStatus) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            component: component.into(),
            status,
            cpu_usage: None,
            memory_usage: None,
            active_requests: None,
            queue_depth: None,
            error_rate: None,
            metrics: HashMap::new(),
        }
    }
}

/// Experiment result event
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExperimentResultEvent {
    /// Event ID
    pub id: Uuid,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Experiment ID
    pub experiment_id: String,
    /// Variant ID
    pub variant_id: Uuid,
    /// Metric name
    pub metric_name: String,
    /// Metric value
    pub metric_value: f64,
    /// Sample size
    pub sample_size: usize,
    /// Confidence level
    pub confidence: Option<f64>,
    /// Is winner
    pub is_winner: bool,
    /// Additional data
    pub data: HashMap<String, serde_json::Value>,
}

impl ExperimentResultEvent {
    /// Create new experiment result event
    pub fn new(
        experiment_id: impl Into<String>,
        variant_id: Uuid,
        metric_name: impl Into<String>,
        metric_value: f64,
        sample_size: usize,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            experiment_id: experiment_id.into(),
            variant_id,
            metric_name: metric_name.into(),
            metric_value,
            sample_size,
            confidence: None,
            is_winner: false,
            data: HashMap::new(),
        }
    }
}

/// Batch of feedback events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackEventBatch {
    /// Batch ID
    pub batch_id: Uuid,
    /// Timestamp when batch was created
    pub created_at: DateTime<Utc>,
    /// Events in this batch
    pub events: Vec<FeedbackEvent>,
}

impl FeedbackEventBatch {
    /// Create new batch
    pub fn new(events: Vec<FeedbackEvent>) -> Self {
        Self {
            batch_id: Uuid::new_v4(),
            created_at: Utc::now(),
            events,
        }
    }

    /// Get batch size
    pub fn size(&self) -> usize {
        self.events.len()
    }

    /// Check if batch is empty
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_feedback_event_creation() {
        let request_id = Uuid::new_v4();
        let event = UserFeedbackEvent::new("session123", request_id)
            .with_user_id("user456")
            .with_rating(4.5)
            .with_liked(true)
            .with_task_completed(true);

        assert_eq!(event.session_id, "session123");
        assert_eq!(event.request_id, request_id);
        assert_eq!(event.user_id, Some("user456".to_string()));
        assert_eq!(event.rating, Some(4.5));
        assert_eq!(event.liked, Some(true));
        assert!(event.task_completed);
    }

    #[test]
    fn test_performance_metrics_event() {
        let request_id = Uuid::new_v4();
        let event = PerformanceMetricsEvent::new(request_id, "gpt-4", 1500.0, 100, 200, 0.05)
            .with_quality_score(0.92);

        assert_eq!(event.request_id, request_id);
        assert_eq!(event.model, "gpt-4");
        assert_eq!(event.latency_ms, 1500.0);
        assert_eq!(event.quality_score, Some(0.92));
    }

    #[test]
    fn test_feedback_event_type() {
        let request_id = Uuid::new_v4();
        let user_feedback = FeedbackEvent::UserFeedback(UserFeedbackEvent::new("s1", request_id));
        let perf_metrics = FeedbackEvent::PerformanceMetrics(PerformanceMetricsEvent::new(
            request_id,
            "gpt-4",
            1000.0,
            100,
            200,
            0.05,
        ));

        assert_eq!(user_feedback.event_type(), "user_feedback");
        assert_eq!(perf_metrics.event_type(), "performance_metrics");
    }

    #[test]
    fn test_feedback_event_serialization() {
        let request_id = Uuid::new_v4();
        let event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new("session123", request_id).with_rating(5.0),
        );

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: FeedbackEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_feedback_event_batch() {
        let request_id = Uuid::new_v4();
        let events = vec![
            FeedbackEvent::UserFeedback(UserFeedbackEvent::new("s1", request_id)),
            FeedbackEvent::PerformanceMetrics(PerformanceMetricsEvent::new(
                request_id,
                "gpt-4",
                1000.0,
                100,
                200,
                0.05,
            )),
        ];

        let batch = FeedbackEventBatch::new(events);
        assert_eq!(batch.size(), 2);
        assert!(!batch.is_empty());
    }

    #[test]
    fn test_system_health_event() {
        let event = SystemHealthEvent::new("api-server", HealthStatus::Healthy);
        assert_eq!(event.component, "api-server");
        assert_eq!(event.status, HealthStatus::Healthy);
    }

    #[test]
    fn test_experiment_result_event() {
        let variant_id = Uuid::new_v4();
        let event = ExperimentResultEvent::new("exp1", variant_id, "conversion_rate", 0.25, 1000);

        assert_eq!(event.experiment_id, "exp1");
        assert_eq!(event.variant_id, variant_id);
        assert_eq!(event.metric_name, "conversion_rate");
        assert_eq!(event.metric_value, 0.25);
    }
}
