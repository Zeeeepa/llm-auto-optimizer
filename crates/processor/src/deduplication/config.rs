//! Configuration for event deduplication

use super::strategy::DeduplicationStrategy;
use crate::error::StateResult;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for event deduplication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeduplicationConfig {
    /// Deduplication strategy
    pub strategy: DeduplicationStrategy,

    /// TTL for deduplication entries
    #[serde(with = "humantime_serde")]
    pub ttl: Duration,

    /// Key prefix for Redis keys
    pub key_prefix: String,

    /// Enable metrics collection
    #[serde(default = "default_true")]
    pub metrics_enabled: bool,

    /// Circuit breaker failure threshold (consecutive failures)
    pub circuit_breaker_threshold: usize,

    /// Circuit breaker timeout before retry
    #[serde(with = "humantime_serde")]
    pub circuit_breaker_timeout: Duration,

    /// Batch size for bulk operations
    pub batch_size: usize,

    /// Enable distributed tracing
    #[serde(default = "default_true")]
    pub tracing_enabled: bool,

    /// Sample rate for tracing (0.0 to 1.0)
    pub trace_sample_rate: f64,
}

impl Default for DeduplicationConfig {
    fn default() -> Self {
        Self {
            strategy: DeduplicationStrategy::BestEffort,
            ttl: Duration::from_secs(3600), // 1 hour
            key_prefix: "dedup:".to_string(),
            metrics_enabled: true,
            circuit_breaker_threshold: 5,
            circuit_breaker_timeout: Duration::from_secs(30),
            batch_size: 100,
            tracing_enabled: true,
            trace_sample_rate: 0.1,
        }
    }
}

impl DeduplicationConfig {
    /// Create a new configuration builder
    pub fn builder() -> DeduplicationConfigBuilder {
        DeduplicationConfigBuilder::default()
    }

    /// Validate the configuration
    pub fn validate(&self) -> StateResult<()> {
        if self.key_prefix.is_empty() {
            return Err(crate::error::StateError::StorageError {
                backend_type: "deduplication".to_string(),
                details: "key_prefix cannot be empty".to_string(),
            });
        }

        if self.ttl.as_secs() == 0 {
            return Err(crate::error::StateError::StorageError {
                backend_type: "deduplication".to_string(),
                details: "TTL must be greater than 0".to_string(),
            });
        }

        if self.circuit_breaker_threshold == 0 {
            return Err(crate::error::StateError::StorageError {
                backend_type: "deduplication".to_string(),
                details: "circuit_breaker_threshold must be greater than 0".to_string(),
            });
        }

        if self.batch_size == 0 {
            return Err(crate::error::StateError::StorageError {
                backend_type: "deduplication".to_string(),
                details: "batch_size must be greater than 0".to_string(),
            });
        }

        if !(0.0..=1.0).contains(&self.trace_sample_rate) {
            return Err(crate::error::StateError::StorageError {
                backend_type: "deduplication".to_string(),
                details: "trace_sample_rate must be between 0.0 and 1.0".to_string(),
            });
        }

        Ok(())
    }
}

/// Builder for DeduplicationConfig
#[derive(Debug, Default)]
pub struct DeduplicationConfigBuilder {
    config: DeduplicationConfig,
}

impl DeduplicationConfigBuilder {
    /// Set deduplication strategy
    pub fn strategy(mut self, strategy: DeduplicationStrategy) -> Self {
        self.config.strategy = strategy;
        self
    }

    /// Set TTL for deduplication entries
    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.config.ttl = ttl;
        self
    }

    /// Set key prefix for Redis keys
    pub fn key_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.config.key_prefix = prefix.into();
        self
    }

    /// Enable or disable metrics collection
    pub fn metrics_enabled(mut self, enabled: bool) -> Self {
        self.config.metrics_enabled = enabled;
        self
    }

    /// Set circuit breaker configuration
    pub fn circuit_breaker(mut self, threshold: usize, timeout: Duration) -> Self {
        self.config.circuit_breaker_threshold = threshold;
        self.config.circuit_breaker_timeout = timeout;
        self
    }

    /// Set batch size for bulk operations
    pub fn batch_size(mut self, size: usize) -> Self {
        self.config.batch_size = size;
        self
    }

    /// Enable or disable distributed tracing
    pub fn tracing_enabled(mut self, enabled: bool) -> Self {
        self.config.tracing_enabled = enabled;
        self
    }

    /// Set trace sample rate (0.0 to 1.0)
    pub fn trace_sample_rate(mut self, rate: f64) -> Self {
        self.config.trace_sample_rate = rate;
        self
    }

    /// Build the configuration
    pub fn build(self) -> StateResult<DeduplicationConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = DeduplicationConfig::default();
        assert!(config.validate().is_ok());
        assert_eq!(config.strategy, DeduplicationStrategy::BestEffort);
        assert_eq!(config.ttl.as_secs(), 3600);
        assert_eq!(config.key_prefix, "dedup:");
    }

    #[test]
    fn test_config_builder() {
        let config = DeduplicationConfig::builder()
            .strategy(DeduplicationStrategy::Strict)
            .ttl(Duration::from_secs(7200))
            .key_prefix("myapp:dedup:")
            .metrics_enabled(true)
            .circuit_breaker(10, Duration::from_secs(60))
            .batch_size(200)
            .tracing_enabled(true)
            .trace_sample_rate(0.5)
            .build()
            .unwrap();

        assert_eq!(config.strategy, DeduplicationStrategy::Strict);
        assert_eq!(config.ttl.as_secs(), 7200);
        assert_eq!(config.key_prefix, "myapp:dedup:");
        assert_eq!(config.circuit_breaker_threshold, 10);
        assert_eq!(config.batch_size, 200);
        assert_eq!(config.trace_sample_rate, 0.5);
    }

    #[test]
    fn test_config_validation() {
        // Empty key prefix
        let mut config = DeduplicationConfig::default();
        config.key_prefix = String::new();
        assert!(config.validate().is_err());

        // Zero TTL
        let mut config = DeduplicationConfig::default();
        config.ttl = Duration::from_secs(0);
        assert!(config.validate().is_err());

        // Invalid trace sample rate
        let mut config = DeduplicationConfig::default();
        config.trace_sample_rate = 1.5;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_serialization() {
        let config = DeduplicationConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: DeduplicationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.strategy, deserialized.strategy);
    }
}
