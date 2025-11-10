//! Deduplication strategies and their behaviors

use serde::{Deserialize, Serialize};

/// Strategy for handling deduplication failures
///
/// Determines how the deduplicator behaves when Redis is unavailable
/// or other errors occur during duplicate detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeduplicationStrategy {
    /// Strict: Fail processing if deduplication check fails
    ///
    /// Guarantees no duplicates but may block processing if Redis
    /// is unavailable. Use for critical data where duplicates are
    /// unacceptable (e.g., financial transactions).
    ///
    /// Behavior on error: Return error, halt processing
    Strict,

    /// BestEffort: Allow events through if deduplication check fails
    ///
    /// Prioritizes availability over perfect deduplication. Events
    /// are allowed through if Redis is unavailable. Use for analytics,
    /// monitoring, or non-critical data.
    ///
    /// Behavior on error: Log warning, treat as unique, continue
    BestEffort,

    /// LogOnly: Never filter events, only log duplicates
    ///
    /// Useful for monitoring and validation without impacting
    /// processing. All events are allowed through, but duplicates
    /// are logged and counted in metrics.
    ///
    /// Behavior on duplicate: Log info, allow through
    /// Behavior on error: Log warning, allow through
    LogOnly,
}

impl Default for DeduplicationStrategy {
    fn default() -> Self {
        // Default to BestEffort for production resilience
        DeduplicationStrategy::BestEffort
    }
}

impl DeduplicationStrategy {
    /// Should this strategy fail on deduplication errors?
    pub fn is_strict(&self) -> bool {
        matches!(self, DeduplicationStrategy::Strict)
    }

    /// Should this strategy filter duplicate events?
    pub fn should_filter(&self) -> bool {
        !matches!(self, DeduplicationStrategy::LogOnly)
    }

    /// Should this strategy allow events through on errors?
    pub fn allows_on_error(&self) -> bool {
        matches!(
            self,
            DeduplicationStrategy::BestEffort | DeduplicationStrategy::LogOnly
        )
    }
}

impl std::fmt::Display for DeduplicationStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeduplicationStrategy::Strict => write!(f, "strict"),
            DeduplicationStrategy::BestEffort => write!(f, "best_effort"),
            DeduplicationStrategy::LogOnly => write!(f, "log_only"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strategy_default() {
        let strategy = DeduplicationStrategy::default();
        assert_eq!(strategy, DeduplicationStrategy::BestEffort);
    }

    #[test]
    fn test_strategy_methods() {
        assert!(DeduplicationStrategy::Strict.is_strict());
        assert!(!DeduplicationStrategy::BestEffort.is_strict());

        assert!(DeduplicationStrategy::Strict.should_filter());
        assert!(DeduplicationStrategy::BestEffort.should_filter());
        assert!(!DeduplicationStrategy::LogOnly.should_filter());

        assert!(!DeduplicationStrategy::Strict.allows_on_error());
        assert!(DeduplicationStrategy::BestEffort.allows_on_error());
        assert!(DeduplicationStrategy::LogOnly.allows_on_error());
    }

    #[test]
    fn test_strategy_display() {
        assert_eq!(DeduplicationStrategy::Strict.to_string(), "strict");
        assert_eq!(DeduplicationStrategy::BestEffort.to_string(), "best_effort");
        assert_eq!(DeduplicationStrategy::LogOnly.to_string(), "log_only");
    }

    #[test]
    fn test_strategy_serialization() {
        let strategy = DeduplicationStrategy::BestEffort;
        let json = serde_json::to_string(&strategy).unwrap();
        assert_eq!(json, "\"best_effort\"");

        let deserialized: DeduplicationStrategy = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, strategy);
    }
}
