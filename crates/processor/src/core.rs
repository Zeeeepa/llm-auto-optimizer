//! Core event processing structures and traits
//!
//! This module provides the fundamental types for event processing in the stream processor:
//! - ProcessorEvent: Wrapper around events with metadata
//! - EventKey: Different types of aggregation keys
//! - EventTimeExtractor: Trait for extracting event timestamps
//! - KeyExtractor: Trait for extracting aggregation keys

use chrono::{DateTime, Utc};
use llm_optimizer_types::events::{CloudEvent, FeedbackEvent};
use llm_optimizer_types::metrics::MetricPoint;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Wrapper around events with metadata for stream processing
///
/// This structure wraps any event type with additional metadata needed for
/// processing, including event time, processing time, and aggregation key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorEvent<T> {
    /// The actual event payload
    pub event: T,
    /// Timestamp when the event occurred (event time semantics)
    pub event_time: DateTime<Utc>,
    /// Timestamp when the event was processed (processing time semantics)
    pub processing_time: DateTime<Utc>,
    /// Aggregation key for grouping related events
    pub key: String,
}

impl<T> ProcessorEvent<T> {
    /// Create a new ProcessorEvent
    pub fn new(event: T, event_time: DateTime<Utc>, key: String) -> Self {
        Self {
            event,
            event_time,
            processing_time: Utc::now(),
            key,
        }
    }

    /// Create a ProcessorEvent with a custom processing time
    pub fn with_processing_time(
        event: T,
        event_time: DateTime<Utc>,
        processing_time: DateTime<Utc>,
        key: String,
    ) -> Self {
        Self {
            event,
            event_time,
            processing_time,
            key,
        }
    }

    /// Calculate the processing delay (processing_time - event_time)
    pub fn processing_delay(&self) -> chrono::Duration {
        self.processing_time - self.event_time
    }

    /// Check if the event is late (processing delay exceeds threshold)
    pub fn is_late(&self, threshold: chrono::Duration) -> bool {
        self.processing_delay() > threshold
    }

    /// Map the event to a different type while preserving metadata
    pub fn map<U, F>(self, f: F) -> ProcessorEvent<U>
    where
        F: FnOnce(T) -> U,
    {
        ProcessorEvent {
            event: f(self.event),
            event_time: self.event_time,
            processing_time: self.processing_time,
            key: self.key,
        }
    }

    /// Get a reference to the event
    pub fn event_ref(&self) -> &T {
        &self.event
    }

    /// Get a mutable reference to the event
    pub fn event_mut(&mut self) -> &mut T {
        &mut self.event
    }
}

impl<T: fmt::Display> fmt::Display for ProcessorEvent<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ProcessorEvent {{ key: {}, event_time: {}, event: {} }}",
            self.key, self.event_time, self.event
        )
    }
}

/// Enum representing different types of aggregation keys
///
/// Events can be grouped by different dimensions for aggregation:
/// - By model for model-specific metrics
/// - By session for user session analysis
/// - By user for user-specific metrics
/// - By composite keys for multi-dimensional aggregation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventKey {
    /// Key by model identifier
    Model(String),
    /// Key by session identifier
    Session(String),
    /// Key by user identifier
    User(String),
    /// Composite key with multiple dimensions
    Composite(Vec<(String, String)>),
}

impl EventKey {
    /// Convert the key to a string representation for use as aggregation key
    pub fn to_key_string(&self) -> String {
        match self {
            EventKey::Model(id) => format!("model:{}", id),
            EventKey::Session(id) => format!("session:{}", id),
            EventKey::User(id) => format!("user:{}", id),
            EventKey::Composite(parts) => {
                let mut key_parts: Vec<String> = parts
                    .iter()
                    .map(|(k, v)| format!("{}:{}", k, v))
                    .collect();
                key_parts.sort(); // Ensure consistent ordering
                format!("composite:{}", key_parts.join(","))
            }
        }
    }

    /// Create a composite key from multiple dimensions
    pub fn composite(parts: Vec<(&str, &str)>) -> Self {
        EventKey::Composite(
            parts
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )
    }

    /// Extract a specific dimension from a composite key
    pub fn get_dimension(&self, dimension: &str) -> Option<String> {
        match self {
            EventKey::Composite(parts) => parts
                .iter()
                .find(|(k, _)| k == dimension)
                .map(|(_, v)| v.clone()),
            _ => None,
        }
    }
}

impl fmt::Display for EventKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_key_string())
    }
}

impl From<EventKey> for String {
    fn from(key: EventKey) -> String {
        key.to_key_string()
    }
}

/// Trait for extracting event time from events
///
/// Implement this trait to define how to extract the timestamp from your event type.
/// This is used for event time semantics in stream processing.
pub trait EventTimeExtractor<T> {
    /// Extract the event timestamp from the event
    fn extract_event_time(&self, event: &T) -> DateTime<Utc>;
}

/// Trait for extracting aggregation keys from events
///
/// Implement this trait to define how to extract the aggregation key from your event type.
/// This is used for grouping events in stream processing operations.
pub trait KeyExtractor<T> {
    /// Extract the aggregation key from the event
    fn extract_key(&self, event: &T) -> EventKey;
}

// ============================================================================
// Implementations for llm-optimizer-types events
// ============================================================================

/// Default event time extractor for FeedbackEvent
#[derive(Debug, Clone, Default)]
pub struct FeedbackEventTimeExtractor;

impl EventTimeExtractor<FeedbackEvent> for FeedbackEventTimeExtractor {
    fn extract_event_time(&self, event: &FeedbackEvent) -> DateTime<Utc> {
        event.timestamp
    }
}

/// Default event time extractor for CloudEvent
#[derive(Debug, Clone, Default)]
pub struct CloudEventTimeExtractor;

impl EventTimeExtractor<CloudEvent> for CloudEventTimeExtractor {
    fn extract_event_time(&self, event: &CloudEvent) -> DateTime<Utc> {
        event.time
    }
}

/// Default event time extractor for MetricPoint
#[derive(Debug, Clone, Default)]
pub struct MetricPointTimeExtractor;

impl EventTimeExtractor<MetricPoint> for MetricPointTimeExtractor {
    fn extract_event_time(&self, event: &MetricPoint) -> DateTime<Utc> {
        event.timestamp
    }
}

/// Key extractor for FeedbackEvent based on metadata
///
/// Extracts keys in the following priority:
/// 1. If both model_id and session_id exist, create composite key
/// 2. If only model_id exists, create Model key
/// 3. If only session_id exists, create Session key
/// 4. If user_id exists, create User key
/// 5. Otherwise, use event ID as key
#[derive(Debug, Clone, Default)]
pub struct FeedbackEventKeyExtractor;

impl KeyExtractor<FeedbackEvent> for FeedbackEventKeyExtractor {
    fn extract_key(&self, event: &FeedbackEvent) -> EventKey {
        let model_id = event.metadata.get("model_id");
        let session_id = event.metadata.get("session_id");
        let user_id = event.metadata.get("user_id");

        match (model_id, session_id, user_id) {
            (Some(m), Some(s), _) => {
                EventKey::composite(vec![("model", m.as_str()), ("session", s.as_str())])
            }
            (Some(m), None, _) => EventKey::Model(m.clone()),
            (None, Some(s), _) => EventKey::Session(s.clone()),
            (None, None, Some(u)) => EventKey::User(u.clone()),
            _ => EventKey::Session(event.id.to_string()),
        }
    }
}

/// Key extractor for CloudEvent based on source
#[derive(Debug, Clone, Default)]
pub struct CloudEventKeyExtractor;

impl KeyExtractor<CloudEvent> for CloudEventKeyExtractor {
    fn extract_key(&self, event: &CloudEvent) -> EventKey {
        // Use source as the primary key dimension
        EventKey::Session(event.source.clone())
    }
}

/// Key extractor for MetricPoint based on tags
///
/// Extracts keys in the following priority:
/// 1. If 'model' tag exists, create Model key
/// 2. If 'session_id' tag exists, create Session key
/// 3. If 'user_id' tag exists, create User key
/// 4. Otherwise, use metric name as key
#[derive(Debug, Clone, Default)]
pub struct MetricPointKeyExtractor;

impl KeyExtractor<MetricPoint> for MetricPointKeyExtractor {
    fn extract_key(&self, event: &MetricPoint) -> EventKey {
        let model = event.tags.get("model");
        let session_id = event.tags.get("session_id");
        let user_id = event.tags.get("user_id");

        match (model, session_id, user_id) {
            (Some(m), Some(s), _) => {
                EventKey::composite(vec![("model", m.as_str()), ("session", s.as_str())])
            }
            (Some(m), None, _) => EventKey::Model(m.clone()),
            (None, Some(s), _) => EventKey::Session(s.clone()),
            (None, None, Some(u)) => EventKey::User(u.clone()),
            _ => EventKey::Session(event.name.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_optimizer_types::events::{EventSource, EventType};
    use std::collections::HashMap;

    #[test]
    fn test_processor_event_creation() {
        let event_time = Utc::now();
        let event = ProcessorEvent::new(
            "test_event".to_string(),
            event_time,
            "test_key".to_string(),
        );

        assert_eq!(event.event, "test_event");
        assert_eq!(event.event_time, event_time);
        assert_eq!(event.key, "test_key");
        assert!(event.processing_time >= event_time);
    }

    #[test]
    fn test_processor_event_processing_delay() {
        let event_time = Utc::now() - chrono::Duration::seconds(10);
        let processing_time = Utc::now();

        let event = ProcessorEvent::with_processing_time(
            "test".to_string(),
            event_time,
            processing_time,
            "key".to_string(),
        );

        let delay = event.processing_delay();
        assert!(delay.num_seconds() >= 9 && delay.num_seconds() <= 11);
    }

    #[test]
    fn test_processor_event_is_late() {
        let event_time = Utc::now() - chrono::Duration::seconds(60);
        let event = ProcessorEvent::new("test".to_string(), event_time, "key".to_string());

        assert!(event.is_late(chrono::Duration::seconds(30)));
        assert!(!event.is_late(chrono::Duration::seconds(120)));
    }

    #[test]
    fn test_processor_event_map() {
        let event = ProcessorEvent::new(42, Utc::now(), "key".to_string());
        let mapped = event.map(|n| n.to_string());

        assert_eq!(mapped.event, "42");
        assert_eq!(mapped.key, "key");
    }

    #[test]
    fn test_event_key_model() {
        let key = EventKey::Model("claude-3-opus".to_string());
        assert_eq!(key.to_key_string(), "model:claude-3-opus");
    }

    #[test]
    fn test_event_key_session() {
        let key = EventKey::Session("session-123".to_string());
        assert_eq!(key.to_key_string(), "session:session-123");
    }

    #[test]
    fn test_event_key_user() {
        let key = EventKey::User("user-456".to_string());
        assert_eq!(key.to_key_string(), "user:user-456");
    }

    #[test]
    fn test_event_key_composite() {
        let key = EventKey::composite(vec![("model", "claude-3-opus"), ("session", "sess-1")]);

        let key_string = key.to_key_string();
        // Should contain both parts
        assert!(key_string.contains("model:claude-3-opus"));
        assert!(key_string.contains("session:sess-1"));
        assert!(key_string.starts_with("composite:"));
    }

    #[test]
    fn test_event_key_composite_ordering() {
        // Keys should be consistent regardless of input order
        let key1 = EventKey::composite(vec![("a", "1"), ("b", "2")]);
        let key2 = EventKey::composite(vec![("b", "2"), ("a", "1")]);

        assert_eq!(key1.to_key_string(), key2.to_key_string());
    }

    #[test]
    fn test_event_key_get_dimension() {
        let key = EventKey::composite(vec![("model", "claude-3-opus"), ("session", "sess-1")]);

        assert_eq!(
            key.get_dimension("model"),
            Some("claude-3-opus".to_string())
        );
        assert_eq!(key.get_dimension("session"), Some("sess-1".to_string()));
        assert_eq!(key.get_dimension("user"), None);
    }

    #[test]
    fn test_feedback_event_time_extractor() {
        let timestamp = Utc::now();
        let event = FeedbackEvent {
            id: uuid::Uuid::new_v4(),
            timestamp,
            source: EventSource::Observatory,
            event_type: EventType::Metric,
            data: serde_json::json!({}),
            metadata: HashMap::new(),
        };

        let extractor = FeedbackEventTimeExtractor;
        assert_eq!(extractor.extract_event_time(&event), timestamp);
    }

    #[test]
    fn test_feedback_event_key_extractor_model() {
        let mut event = FeedbackEvent::new(
            EventSource::Observatory,
            EventType::Metric,
            serde_json::json!({}),
        );
        event.metadata.insert("model_id".to_string(), "claude-3-opus".to_string());

        let extractor = FeedbackEventKeyExtractor;
        let key = extractor.extract_key(&event);

        match key {
            EventKey::Model(id) => assert_eq!(id, "claude-3-opus"),
            _ => panic!("Expected Model key"),
        }
    }

    #[test]
    fn test_feedback_event_key_extractor_composite() {
        let mut event = FeedbackEvent::new(
            EventSource::Observatory,
            EventType::Metric,
            serde_json::json!({}),
        );
        event.metadata.insert("model_id".to_string(), "claude-3-opus".to_string());
        event.metadata.insert("session_id".to_string(), "sess-123".to_string());

        let extractor = FeedbackEventKeyExtractor;
        let key = extractor.extract_key(&event);

        match key {
            EventKey::Composite(parts) => {
                assert_eq!(parts.len(), 2);
                assert!(parts.contains(&("model".to_string(), "claude-3-opus".to_string())));
                assert!(parts.contains(&("session".to_string(), "sess-123".to_string())));
            }
            _ => panic!("Expected Composite key"),
        }
    }

    #[test]
    fn test_feedback_event_key_extractor_session() {
        let mut event = FeedbackEvent::new(
            EventSource::Observatory,
            EventType::Metric,
            serde_json::json!({}),
        );
        event.metadata.insert("session_id".to_string(), "sess-123".to_string());

        let extractor = FeedbackEventKeyExtractor;
        let key = extractor.extract_key(&event);

        match key {
            EventKey::Session(id) => assert_eq!(id, "sess-123"),
            _ => panic!("Expected Session key"),
        }
    }

    #[test]
    fn test_feedback_event_key_extractor_user() {
        let mut event = FeedbackEvent::new(
            EventSource::User,
            EventType::Feedback,
            serde_json::json!({}),
        );
        event.metadata.insert("user_id".to_string(), "user-789".to_string());

        let extractor = FeedbackEventKeyExtractor;
        let key = extractor.extract_key(&event);

        match key {
            EventKey::User(id) => assert_eq!(id, "user-789"),
            _ => panic!("Expected User key"),
        }
    }

    #[test]
    fn test_feedback_event_key_extractor_fallback() {
        let event = FeedbackEvent::new(
            EventSource::System,
            EventType::PerformanceAlert,
            serde_json::json!({}),
        );

        let extractor = FeedbackEventKeyExtractor;
        let key = extractor.extract_key(&event);

        match key {
            EventKey::Session(id) => assert_eq!(id, event.id.to_string()),
            _ => panic!("Expected Session key with event ID"),
        }
    }

    #[test]
    fn test_cloud_event_time_extractor() {
        let time = Utc::now();
        let event = CloudEvent {
            specversion: "1.0".to_string(),
            event_type: "test.event".to_string(),
            source: "test-source".to_string(),
            id: "test-id".to_string(),
            time,
            datacontenttype: "application/json".to_string(),
            data: serde_json::json!({}),
        };

        let extractor = CloudEventTimeExtractor;
        assert_eq!(extractor.extract_event_time(&event), time);
    }

    #[test]
    fn test_cloud_event_key_extractor() {
        let event = CloudEvent::new(
            "test.event",
            "llm-observatory",
            serde_json::json!({}),
        );

        let extractor = CloudEventKeyExtractor;
        let key = extractor.extract_key(&event);

        match key {
            EventKey::Session(id) => assert_eq!(id, "llm-observatory"),
            _ => panic!("Expected Session key"),
        }
    }

    #[test]
    fn test_metric_point_time_extractor() {
        let timestamp = Utc::now();
        let point = MetricPoint {
            name: "latency".to_string(),
            value: 123.45,
            unit: "ms".to_string(),
            timestamp,
            tags: HashMap::new(),
            aggregation: None,
        };

        let extractor = MetricPointTimeExtractor;
        assert_eq!(extractor.extract_event_time(&point), timestamp);
    }

    #[test]
    fn test_metric_point_key_extractor_model() {
        let mut tags = HashMap::new();
        tags.insert("model".to_string(), "claude-3-opus".to_string());

        let point = MetricPoint {
            name: "latency".to_string(),
            value: 100.0,
            unit: "ms".to_string(),
            timestamp: Utc::now(),
            tags,
            aggregation: None,
        };

        let extractor = MetricPointKeyExtractor;
        let key = extractor.extract_key(&point);

        match key {
            EventKey::Model(id) => assert_eq!(id, "claude-3-opus"),
            _ => panic!("Expected Model key"),
        }
    }

    #[test]
    fn test_metric_point_key_extractor_composite() {
        let mut tags = HashMap::new();
        tags.insert("model".to_string(), "claude-3-opus".to_string());
        tags.insert("session_id".to_string(), "sess-123".to_string());

        let point = MetricPoint {
            name: "latency".to_string(),
            value: 100.0,
            unit: "ms".to_string(),
            timestamp: Utc::now(),
            tags,
            aggregation: None,
        };

        let extractor = MetricPointKeyExtractor;
        let key = extractor.extract_key(&point);

        match key {
            EventKey::Composite(parts) => {
                assert_eq!(parts.len(), 2);
                assert!(parts.contains(&("model".to_string(), "claude-3-opus".to_string())));
                assert!(parts.contains(&("session".to_string(), "sess-123".to_string())));
            }
            _ => panic!("Expected Composite key"),
        }
    }

    #[test]
    fn test_metric_point_key_extractor_fallback() {
        let point = MetricPoint::new("latency", 100.0, "ms");

        let extractor = MetricPointKeyExtractor;
        let key = extractor.extract_key(&point);

        match key {
            EventKey::Session(id) => assert_eq!(id, "latency"),
            _ => panic!("Expected Session key with metric name"),
        }
    }

    #[test]
    fn test_processor_event_with_feedback_event() {
        let feedback = FeedbackEvent::new(
            EventSource::Observatory,
            EventType::Metric,
            serde_json::json!({"latency": 123.45}),
        );

        let time_extractor = FeedbackEventTimeExtractor;
        let key_extractor = FeedbackEventKeyExtractor;

        let event_time = time_extractor.extract_event_time(&feedback);
        let key = key_extractor.extract_key(&feedback);

        let processor_event = ProcessorEvent::new(feedback, event_time, key.to_key_string());

        assert_eq!(processor_event.event_time, event_time);
        assert!(processor_event.key.contains("session:"));
    }

    #[test]
    fn test_processor_event_display() {
        let event = ProcessorEvent::new("test".to_string(), Utc::now(), "key123".to_string());
        let display = format!("{}", event);

        assert!(display.contains("key123"));
        assert!(display.contains("test"));
    }

    #[test]
    fn test_event_key_into_string() {
        let key = EventKey::Model("claude-3-opus".to_string());
        let key_string: String = key.into();

        assert_eq!(key_string, "model:claude-3-opus");
    }
}
