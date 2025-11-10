//! Event ID extraction strategies
//!
//! This module provides various strategies for extracting unique event IDs
//! from events for deduplication purposes.

use super::error::{DeduplicationError, DeduplicationResult};
use super::id::EventId;
use llm_optimizer_types::events::{CloudEvent, FeedbackEvent};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

/// Trait for extracting unique event identifiers
///
/// Implementations define how to extract a stable, unique ID from an event.
/// The ID must be deterministic (same event = same ID) and unique (different
/// events = different IDs).
pub trait EventIdExtractor<T>: Send + Sync {
    /// Extract a unique event ID from an event
    ///
    /// # Arguments
    ///
    /// * `event` - The event to extract an ID from
    ///
    /// # Returns
    ///
    /// A unique EventId or an error if extraction fails
    fn extract_id(&self, event: &T) -> DeduplicationResult<EventId>;
}

// ============================================================================
// UUID-Based Extractors
// ============================================================================

/// Extract event ID from FeedbackEvent's UUID field
#[derive(Debug, Clone, Default)]
pub struct FeedbackEventUuidExtractor;

impl EventIdExtractor<FeedbackEvent> for FeedbackEventUuidExtractor {
    fn extract_id(&self, event: &FeedbackEvent) -> DeduplicationResult<EventId> {
        Ok(EventId::from(event.id))
    }
}

/// Extract event ID from CloudEvent's ID field
#[derive(Debug, Clone, Default)]
pub struct CloudEventUuidExtractor;

impl EventIdExtractor<CloudEvent> for CloudEventUuidExtractor {
    fn extract_id(&self, event: &CloudEvent) -> DeduplicationResult<EventId> {
        Ok(EventId::from_string(event.id.clone()))
    }
}

// ============================================================================
// Hash-Based Extractor
// ============================================================================

/// Extract ID by hashing event content
///
/// Generates a deterministic hash from the event's content, optionally
/// excluding fields like timestamps that change on every retry.
#[derive(Debug, Clone)]
pub struct ContentHashExtractor<H = Sha256> {
    /// Fields to include in hash (None = all fields)
    include_fields: Option<Vec<String>>,

    /// Fields to exclude from hash (e.g., "timestamp", "id")
    exclude_fields: Vec<String>,

    /// Hash algorithm marker
    _phantom: PhantomData<H>,
}

impl ContentHashExtractor<Sha256> {
    /// Create a SHA256-based hash extractor
    ///
    /// By default, excludes "timestamp", "id", and "processing_time" fields
    /// to ensure idempotent retries produce the same ID.
    pub fn sha256() -> Self {
        Self {
            include_fields: None,
            exclude_fields: vec![
                "timestamp".to_string(),
                "id".to_string(),
                "processing_time".to_string(),
            ],
            _phantom: PhantomData,
        }
    }

    /// Create a hash extractor with custom excluded fields
    pub fn sha256_excluding(exclude_fields: Vec<String>) -> Self {
        Self {
            include_fields: None,
            exclude_fields,
            _phantom: PhantomData,
        }
    }

    /// Create a hash extractor with specific included fields
    pub fn sha256_including(include_fields: Vec<String>) -> Self {
        Self {
            include_fields: Some(include_fields),
            exclude_fields: Vec::new(),
            _phantom: PhantomData,
        }
    }

    /// Filter JSON value based on include/exclude rules
    fn filter_fields(&self, value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(map) => {
                let filtered: serde_json::Map<String, serde_json::Value> = map
                    .iter()
                    .filter(|(k, _)| {
                        // If include_fields is set, only include specified fields
                        if let Some(ref includes) = self.include_fields {
                            return includes.contains(&k.to_string());
                        }
                        // Otherwise, exclude specified fields
                        !self.exclude_fields.contains(&k.to_string())
                    })
                    .map(|(k, v)| (k.clone(), self.filter_fields(v)))
                    .collect();
                serde_json::Value::Object(filtered)
            }
            serde_json::Value::Array(arr) => {
                let filtered: Vec<serde_json::Value> =
                    arr.iter().map(|v| self.filter_fields(v)).collect();
                serde_json::Value::Array(filtered)
            }
            _ => value.clone(),
        }
    }
}

impl<T, H> EventIdExtractor<T> for ContentHashExtractor<H>
where
    T: Serialize,
    H: Digest,
{
    fn extract_id(&self, event: &T) -> DeduplicationResult<EventId> {
        // Serialize event to JSON
        let json = serde_json::to_value(event)?;

        // Filter fields
        let filtered = self.filter_fields(&json);

        // Serialize to canonical string (sorted keys)
        let canonical = serde_json::to_string(&filtered)?;

        // Hash the canonical representation
        let mut hasher = H::new();
        hasher.update(canonical.as_bytes());
        let hash = hasher.finalize();

        Ok(EventId::from_bytes(&hash))
    }
}

// ============================================================================
// Composite Key Extractor
// ============================================================================

/// Field extraction strategy
#[derive(Debug, Clone)]
pub enum FieldExtractor {
    /// Extract from a named field in the event
    Field(String),

    /// Extract from metadata map (for FeedbackEvent)
    Metadata(String),

    /// Extract from nested path (e.g., "data.user.id")
    Path(Vec<String>),

    /// Extract using custom function
    Custom(Arc<dyn Fn(&serde_json::Value) -> Option<String> + Send + Sync>),
}

impl FieldExtractor {
    /// Extract value from JSON
    fn extract(&self, value: &serde_json::Value) -> Option<String> {
        match self {
            FieldExtractor::Field(name) => value.get(name)?.as_str().map(String::from),

            FieldExtractor::Metadata(key) => {
                value.get("metadata")?.get(key)?.as_str().map(String::from)
            }

            FieldExtractor::Path(path) => {
                let mut current = value;
                for segment in path {
                    current = current.get(segment)?;
                }
                current.as_str().map(String::from)
            }

            FieldExtractor::Custom(f) => f(value),
        }
    }
}

/// Extract ID from multiple fields combined
///
/// Creates a composite key from multiple event fields, useful for
/// multi-tenant systems or when events need scoped deduplication.
#[derive(Debug, Clone)]
pub struct CompositeExtractor {
    /// Field extractors in order
    fields: Vec<FieldExtractor>,

    /// Separator between fields
    separator: String,
}

impl CompositeExtractor {
    /// Create a new composite extractor
    ///
    /// # Arguments
    ///
    /// * `fields` - List of field extractors
    /// * `separator` - String to separate field values (default: ":")
    pub fn new(fields: Vec<FieldExtractor>, separator: String) -> Self {
        Self { fields, separator }
    }

    /// Create a composite extractor for model + session
    ///
    /// Extracts "model_id" and "session_id" from metadata.
    pub fn model_session() -> Self {
        Self {
            fields: vec![
                FieldExtractor::Metadata("model_id".to_string()),
                FieldExtractor::Metadata("session_id".to_string()),
            ],
            separator: ":".to_string(),
        }
    }

    /// Create a composite extractor for user + session
    pub fn user_session() -> Self {
        Self {
            fields: vec![
                FieldExtractor::Metadata("user_id".to_string()),
                FieldExtractor::Metadata("session_id".to_string()),
            ],
            separator: ":".to_string(),
        }
    }
}

impl<T> EventIdExtractor<T> for CompositeExtractor
where
    T: Serialize,
{
    fn extract_id(&self, event: &T) -> DeduplicationResult<EventId> {
        // Serialize to JSON
        let json = serde_json::to_value(event)?;

        // Extract all fields
        let mut parts = Vec::new();
        for field in &self.fields {
            let value = field.extract(&json).ok_or_else(|| {
                DeduplicationError::ExtractionFailed {
                    reason: format!("Failed to extract field: {:?}", field),
                }
            })?;
            parts.push(value);
        }

        // Join with separator
        let composite_id = parts.join(&self.separator);
        Ok(EventId::from_string(composite_id))
    }
}

// ============================================================================
// Time-Window Hash Extractor
// ============================================================================

/// Hash-based extractor with time window bucketing
///
/// Combines content hash with time bucket for time-bounded deduplication.
/// Events in different time windows will have different IDs even if content
/// is identical.
#[derive(Debug, Clone)]
pub struct TimeWindowHashExtractor {
    /// Base hash extractor
    base_extractor: ContentHashExtractor,

    /// Window size for bucketing (in seconds)
    window_size_secs: i64,
}

impl TimeWindowHashExtractor {
    /// Create a new time-window hash extractor
    ///
    /// # Arguments
    ///
    /// * `window_size_secs` - Window size in seconds
    pub fn new(window_size_secs: i64) -> Self {
        Self {
            base_extractor: ContentHashExtractor::sha256(),
            window_size_secs,
        }
    }

    /// Create with 1-hour windows
    pub fn hourly() -> Self {
        Self::new(3600)
    }

    /// Create with 1-day windows
    pub fn daily() -> Self {
        Self::new(86400)
    }
}

impl<T> EventIdExtractor<T> for TimeWindowHashExtractor
where
    T: Serialize,
{
    fn extract_id(&self, event: &T) -> DeduplicationResult<EventId> {
        // Get base hash
        let base_id = self.base_extractor.extract_id(event)?;

        // Get current time bucket
        let now = chrono::Utc::now();
        let bucket = now.timestamp() / self.window_size_secs;

        // Combine hash with time bucket
        let windowed_id = format!("{}:{}", bucket, base_id.as_str());
        Ok(EventId::from_string(windowed_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_optimizer_types::events::{EventSource, EventType};

    #[test]
    fn test_feedback_event_uuid_extractor() {
        let event = FeedbackEvent::new(
            EventSource::Observatory,
            EventType::Metric,
            serde_json::json!({}),
        );

        let extractor = FeedbackEventUuidExtractor;
        let id = extractor.extract_id(&event).unwrap();

        assert_eq!(id.as_str(), event.id.to_string());
    }

    #[test]
    fn test_cloud_event_uuid_extractor() {
        let event = CloudEvent::new("test.event", "test-source", serde_json::json!({}));

        let extractor = CloudEventUuidExtractor;
        let id = extractor.extract_id(&event).unwrap();

        assert_eq!(id.as_str(), event.id);
    }

    #[test]
    fn test_content_hash_extractor() {
        #[derive(Serialize)]
        struct TestEvent {
            id: String,
            timestamp: i64,
            data: String,
        }

        let event = TestEvent {
            id: "test-1".to_string(),
            timestamp: 123456,
            data: "important-data".to_string(),
        };

        let extractor = ContentHashExtractor::sha256();
        let id = extractor.extract_id(&event).unwrap();

        // Same data should produce same hash (even with different timestamp/id)
        let event2 = TestEvent {
            id: "test-2".to_string(),
            timestamp: 789012,
            data: "important-data".to_string(),
        };

        let id2 = extractor.extract_id(&event2).unwrap();
        assert_eq!(id, id2); // Should be equal since timestamp/id are excluded
    }

    #[test]
    fn test_composite_extractor_model_session() {
        let mut event = FeedbackEvent::new(
            EventSource::Observatory,
            EventType::Metric,
            serde_json::json!({}),
        );
        event
            .metadata
            .insert("model_id".to_string(), "gpt-4".to_string());
        event
            .metadata
            .insert("session_id".to_string(), "sess-123".to_string());

        let extractor = CompositeExtractor::model_session();
        let id = extractor.extract_id(&event).unwrap();

        assert_eq!(id.as_str(), "gpt-4:sess-123");
    }

    #[test]
    fn test_time_window_hash_extractor() {
        #[derive(Serialize)]
        struct TestEvent {
            data: String,
        }

        let event = TestEvent {
            data: "test".to_string(),
        };

        let extractor = TimeWindowHashExtractor::new(3600);
        let id1 = extractor.extract_id(&event).unwrap();

        // Should start with time bucket
        assert!(id1.as_str().contains(':'));

        // Same event should produce same ID within same window
        let id2 = extractor.extract_id(&event).unwrap();
        assert_eq!(id1, id2);
    }
}
