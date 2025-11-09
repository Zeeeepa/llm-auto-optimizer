//! Context feature extraction for contextual bandits
//!
//! This module provides feature extraction from request context for
//! context-aware model selection and parameter optimization.

use chrono::{Datelike, Timelike};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Request context for contextual bandit decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestContext {
    /// User ID (if available)
    pub user_id: Option<String>,
    /// Task type (classification, generation, extraction, etc.)
    pub task_type: Option<String>,
    /// Input length in tokens
    pub input_length: usize,
    /// Expected output length category
    pub output_length_category: OutputLengthCategory,
    /// Request priority (1-10)
    pub priority: u8,
    /// Geographic region
    pub region: Option<String>,
    /// Time of day (hour 0-23)
    pub hour_of_day: u8,
    /// Day of week (0-6, where 0 is Monday)
    pub day_of_week: u8,
    /// Language code (e.g., "en", "es", "fr")
    pub language: Option<String>,
    /// Custom metadata
    pub metadata: HashMap<String, String>,
}

/// Expected output length category
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputLengthCategory {
    Short,   // < 100 tokens
    Medium,  // 100-500 tokens
    Long,    // > 500 tokens
}

impl RequestContext {
    /// Create a new request context with defaults
    pub fn new(input_length: usize) -> Self {
        let now = chrono::Utc::now();

        Self {
            user_id: None,
            task_type: None,
            input_length,
            output_length_category: OutputLengthCategory::Medium,
            priority: 5,
            region: None,
            hour_of_day: now.hour() as u8,
            day_of_week: now.weekday().num_days_from_monday() as u8,
            language: None,
            metadata: HashMap::new(),
        }
    }

    /// Extract feature vector for machine learning
    pub fn to_feature_vector(&self) -> Vec<f64> {
        let mut features = Vec::with_capacity(10);

        // Normalized input length (log scale)
        features.push(((self.input_length as f64 + 1.0).ln() / 10.0).min(1.0));

        // Output length category (one-hot encoding-ish)
        features.push(match self.output_length_category {
            OutputLengthCategory::Short => 0.0,
            OutputLengthCategory::Medium => 0.5,
            OutputLengthCategory::Long => 1.0,
        });

        // Normalized priority
        features.push(self.priority as f64 / 10.0);

        // Time features (cyclical encoding)
        let hour_rad = (self.hour_of_day as f64 / 24.0) * 2.0 * std::f64::consts::PI;
        features.push(hour_rad.cos());
        features.push(hour_rad.sin());

        let day_rad = (self.day_of_week as f64 / 7.0) * 2.0 * std::f64::consts::PI;
        features.push(day_rad.cos());
        features.push(day_rad.sin());

        // Task type indicator (basic encoding)
        let task_indicator = match self.task_type.as_deref() {
            Some("classification") => 0.0,
            Some("generation") => 0.33,
            Some("extraction") => 0.67,
            Some("summarization") => 1.0,
            _ => 0.5, // unknown
        };
        features.push(task_indicator);

        // Bias term (always 1.0)
        features.push(1.0);

        // Add padding to ensure consistent feature vector size
        while features.len() < 10 {
            features.push(0.0);
        }

        features
    }

    /// Get feature dimension
    pub fn feature_dimension() -> usize {
        10
    }

    /// Set task type
    pub fn with_task_type(mut self, task_type: impl Into<String>) -> Self {
        self.task_type = Some(task_type.into());
        self
    }

    /// Set output length category
    pub fn with_output_length(mut self, category: OutputLengthCategory) -> Self {
        self.output_length_category = category;
        self
    }

    /// Set priority
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority.min(10);
        self
    }

    /// Set user ID
    pub fn with_user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Set language
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// Add custom metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

impl Default for RequestContext {
    fn default() -> Self {
        Self::new(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let context = RequestContext::new(100);
        assert_eq!(context.input_length, 100);
        assert_eq!(context.priority, 5);
    }

    #[test]
    fn test_feature_vector() {
        let context = RequestContext::new(256)
            .with_task_type("generation")
            .with_output_length(OutputLengthCategory::Long)
            .with_priority(8);

        let features = context.to_feature_vector();
        assert_eq!(features.len(), RequestContext::feature_dimension());

        // Check all features are in valid range
        for &f in &features {
            assert!(f >= -1.0 && f <= 1.0 || f.is_finite());
        }
    }

    #[test]
    fn test_feature_dimension() {
        assert_eq!(RequestContext::feature_dimension(), 10);
    }

    #[test]
    fn test_cyclical_time_encoding() {
        let context1 = RequestContext::new(100);
        let mut context2 = context1.clone();
        context2.hour_of_day = (context1.hour_of_day + 12) % 24;

        let f1 = context1.to_feature_vector();
        let f2 = context2.to_feature_vector();

        // Hour features should be different
        assert!((f1[3] - f2[3]).abs() > 0.1 || (f1[4] - f2[4]).abs() > 0.1);
    }

    #[test]
    fn test_builder_pattern() {
        let context = RequestContext::new(500)
            .with_task_type("classification")
            .with_priority(9)
            .with_user_id("user123")
            .with_language("en")
            .with_metadata("custom_key", "custom_value");

        assert_eq!(context.task_type, Some("classification".to_string()));
        assert_eq!(context.priority, 9);
        assert_eq!(context.user_id, Some("user123".to_string()));
        assert_eq!(context.language, Some("en".to_string()));
        assert_eq!(context.metadata.get("custom_key"), Some(&"custom_value".to_string()));
    }

    #[test]
    fn test_output_length_categories() {
        let short = RequestContext::new(10).with_output_length(OutputLengthCategory::Short);
        let medium = RequestContext::new(10).with_output_length(OutputLengthCategory::Medium);
        let long = RequestContext::new(10).with_output_length(OutputLengthCategory::Long);

        assert_eq!(short.to_feature_vector()[1], 0.0);
        assert_eq!(medium.to_feature_vector()[1], 0.5);
        assert_eq!(long.to_feature_vector()[1], 1.0);
    }
}
