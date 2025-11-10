//! Event ID types and utilities

use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for an event
///
/// Event IDs are used for deduplication and must be:
/// - Stable: Same event always produces the same ID
/// - Unique: Different events produce different IDs
/// - Serializable: Can be stored in Redis
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(String);

impl EventId {
    /// Create an EventId from a string
    ///
    /// # Arguments
    ///
    /// * `s` - String representation of the ID
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Create an EventId from bytes (will hex-encode)
    ///
    /// Useful for hash-based IDs or binary identifiers.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Binary data to encode as ID
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(hex::encode(bytes))
    }

    /// Get the ID as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Get the ID as bytes for Redis storage
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Get the length of the ID in bytes
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if the ID is empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Create a composite ID from multiple parts
    ///
    /// # Arguments
    ///
    /// * `parts` - Parts to combine
    /// * `separator` - Separator between parts (default: ":")
    pub fn composite(parts: &[&str], separator: &str) -> Self {
        Self(parts.join(separator))
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for EventId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for EventId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<uuid::Uuid> for EventId {
    fn from(uuid: uuid::Uuid) -> Self {
        Self(uuid.to_string())
    }
}

impl AsRef<str> for EventId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_string() {
        let id = EventId::from_string("test-id-123");
        assert_eq!(id.as_str(), "test-id-123");
    }

    #[test]
    fn test_from_bytes() {
        let bytes = b"hello";
        let id = EventId::from_bytes(bytes);
        assert_eq!(id.as_str(), "68656c6c6f"); // hex encoding of "hello"
    }

    #[test]
    fn test_composite() {
        let id = EventId::composite(&["part1", "part2", "part3"], ":");
        assert_eq!(id.as_str(), "part1:part2:part3");
    }

    #[test]
    fn test_from_uuid() {
        let uuid = uuid::Uuid::new_v4();
        let id = EventId::from(uuid);
        assert_eq!(id.as_str(), uuid.to_string());
    }

    #[test]
    fn test_equality() {
        let id1 = EventId::from_string("test");
        let id2 = EventId::from_string("test");
        let id3 = EventId::from_string("other");

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_display() {
        let id = EventId::from_string("display-test");
        assert_eq!(format!("{}", id), "display-test");
    }

    #[test]
    fn test_serialization() {
        let id = EventId::from_string("serialize-test");
        let json = serde_json::to_string(&id).unwrap();
        let deserialized: EventId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, deserialized);
    }
}
