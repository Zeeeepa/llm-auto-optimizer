//! Event types for the optimizer

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Type of feedback event
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// Metric event from observatory
    Metric,
    /// Anomaly detected by sentinel
    Anomaly,
    /// User feedback signal
    Feedback,
    /// Performance alert
    PerformanceAlert,
    /// Cost alert
    CostAlert,
    /// Quality alert
    QualityAlert,
}

/// Source of the feedback event
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventSource {
    Observatory,
    Sentinel,
    User,
    System,
    Governance,
}

/// Core feedback event structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackEvent {
    /// Unique identifier for the event
    pub id: Uuid,
    /// Timestamp when the event occurred
    pub timestamp: DateTime<Utc>,
    /// Source of the event
    pub source: EventSource,
    /// Type of event
    pub event_type: EventType,
    /// Event data payload
    pub data: serde_json::Value,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl FeedbackEvent {
    /// Create a new feedback event
    pub fn new(
        source: EventSource,
        event_type: EventType,
        data: serde_json::Value,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            source,
            event_type,
            data,
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the event
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// CloudEvents-compliant event structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudEvent {
    /// CloudEvents specification version
    pub specversion: String,
    /// Event type identifier
    #[serde(rename = "type")]
    pub event_type: String,
    /// Event source identifier
    pub source: String,
    /// Unique event ID
    pub id: String,
    /// Event timestamp
    pub time: DateTime<Utc>,
    /// Content type of data
    pub datacontenttype: String,
    /// Event data
    pub data: serde_json::Value,
}

impl CloudEvent {
    /// Create a new CloudEvent with version 1.0
    pub fn new(
        event_type: impl Into<String>,
        source: impl Into<String>,
        data: serde_json::Value,
    ) -> Self {
        Self {
            specversion: "1.0".to_string(),
            event_type: event_type.into(),
            source: source.into(),
            id: Uuid::new_v4().to_string(),
            time: Utc::now(),
            datacontenttype: "application/json".to_string(),
            data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feedback_event_creation() {
        let event = FeedbackEvent::new(
            EventSource::Observatory,
            EventType::Metric,
            serde_json::json!({"metric": "latency", "value": 123.45}),
        );

        assert_eq!(event.source, EventSource::Observatory);
        assert_eq!(event.event_type, EventType::Metric);
    }

    #[test]
    fn test_feedback_event_with_metadata() {
        let event = FeedbackEvent::new(
            EventSource::User,
            EventType::Feedback,
            serde_json::json!({"rating": 5}),
        )
        .with_metadata("user_id", "12345")
        .with_metadata("session_id", "abc-def");

        assert_eq!(event.metadata.len(), 2);
        assert_eq!(event.metadata.get("user_id").unwrap(), "12345");
    }

    #[test]
    fn test_cloud_event_creation() {
        let event = CloudEvent::new(
            "com.llmdevops.observatory.metric.v1",
            "llm-observatory",
            serde_json::json!({"test": "data"}),
        );

        assert_eq!(event.specversion, "1.0");
        assert_eq!(event.event_type, "com.llmdevops.observatory.metric.v1");
    }
}
