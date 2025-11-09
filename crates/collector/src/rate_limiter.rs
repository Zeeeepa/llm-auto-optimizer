//! Rate Limiting
//!
//! This module provides production-grade token bucket rate limiting to prevent
//! event floods and protect downstream systems from DoS attacks and misconfigured clients.
//!
//! Features:
//! - Global rate limiting (default: 10,000 events/sec)
//! - Per-source rate limiting with keyed quotas
//! - Configurable burst allowance
//! - Low overhead (< 100 microseconds per check)
//! - Thread-safe concurrent access
//! - Memory-efficient LRU cache for source tracking
//! - Comprehensive metrics

use std::net::IpAddr;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use governor::{
    clock::{Clock, DefaultClock},
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter as GovernorRateLimiter,
};
use moka::future::Cache;
use thiserror::Error;
use tracing::{debug, warn};

use crate::feedback_events::FeedbackEvent;
use crate::telemetry::TelemetryMetrics;

/// Rate limit error
#[derive(Debug, Error)]
pub enum RateLimitError {
    /// Rate limit exceeded
    #[error("Rate limit exceeded. Retry after {retry_after:?}")]
    Exceeded {
        /// Duration to wait before retrying
        retry_after: Duration,
    },

    /// Invalid configuration
    #[error("Invalid rate limit configuration: {0}")]
    InvalidConfig(String),

    /// Source identifier extraction failed
    #[error("Failed to extract source identifier: {0}")]
    SourceExtractionFailed(String),
}

/// Source identifier type
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SourceIdentifier {
    /// IP address
    IpAddress(IpAddr),
    /// API key
    ApiKey(String),
    /// User ID
    UserId(String),
    /// Session ID
    SessionId(String),
    /// Custom identifier
    Custom(String),
}

impl SourceIdentifier {
    /// Extract source identifier from feedback event
    pub fn from_event(event: &FeedbackEvent, strategy: &SourceIdentifierStrategy) -> Result<Self, RateLimitError> {
        match strategy {
            SourceIdentifierStrategy::IpAddress => {
                // In a real implementation, IP would come from request context
                Err(RateLimitError::SourceExtractionFailed(
                    "IP address not available in event context".to_string(),
                ))
            }
            SourceIdentifierStrategy::ApiKey => {
                // API key would come from request headers in real implementation
                Err(RateLimitError::SourceExtractionFailed(
                    "API key not available in event context".to_string(),
                ))
            }
            SourceIdentifierStrategy::UserId => {
                let user_id = match event {
                    FeedbackEvent::UserFeedback(e) => e.user_id.clone(),
                    FeedbackEvent::PerformanceMetrics(_) => None,
                    FeedbackEvent::ModelResponse(_) => None,
                    FeedbackEvent::SystemHealth(_) => None,
                    FeedbackEvent::ExperimentResult(_) => None,
                };

                user_id
                    .map(SourceIdentifier::UserId)
                    .ok_or_else(|| RateLimitError::SourceExtractionFailed(
                        "User ID not found in event".to_string(),
                    ))
            }
            SourceIdentifierStrategy::SessionId => {
                let session_id = match event {
                    FeedbackEvent::UserFeedback(e) => Some(e.session_id.clone()),
                    FeedbackEvent::ModelResponse(e) => Some(e.session_id.clone()),
                    _ => None,
                };

                session_id
                    .map(SourceIdentifier::SessionId)
                    .ok_or_else(|| RateLimitError::SourceExtractionFailed(
                        "Session ID not found in event".to_string(),
                    ))
            }
            SourceIdentifierStrategy::Custom(field) => {
                // Extract from event metadata
                match event {
                    FeedbackEvent::UserFeedback(e) => {
                        e.metadata.get(field)
                            .map(|v| SourceIdentifier::Custom(v.clone()))
                            .ok_or_else(|| RateLimitError::SourceExtractionFailed(
                                format!("Custom field '{}' not found in metadata", field),
                            ))
                    }
                    _ => Err(RateLimitError::SourceExtractionFailed(
                        "Event type does not support metadata".to_string(),
                    )),
                }
            }
        }
    }
}

/// Strategy for identifying rate limit sources
#[derive(Debug, Clone)]
pub enum SourceIdentifierStrategy {
    /// Use IP address
    IpAddress,
    /// Use API key
    ApiKey,
    /// Use user ID from event
    UserId,
    /// Use session ID from event
    SessionId,
    /// Use custom field from event metadata
    Custom(String),
}

/// Rate limit configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Global rate limit (events per second)
    pub global_rate: NonZeroU32,

    /// Global burst capacity
    pub global_burst: NonZeroU32,

    /// Per-source rate limit (events per second)
    pub per_source_rate: Option<NonZeroU32>,

    /// Per-source burst capacity
    pub per_source_burst: Option<NonZeroU32>,

    /// Source identifier strategy
    pub source_identifier: SourceIdentifierStrategy,

    /// Maximum number of sources to track (LRU eviction)
    pub max_sources: usize,

    /// Time-to-live for inactive sources (seconds)
    pub source_ttl_secs: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            global_rate: NonZeroU32::new(10_000).unwrap(), // 10,000 events/sec
            global_burst: NonZeroU32::new(10_000).unwrap(),
            per_source_rate: Some(NonZeroU32::new(100).unwrap()), // 100 events/sec per source
            per_source_burst: Some(NonZeroU32::new(200).unwrap()),
            source_identifier: SourceIdentifierStrategy::SessionId,
            max_sources: 10_000,
            source_ttl_secs: 300, // 5 minutes
        }
    }
}

impl RateLimitConfig {
    /// Validate configuration
    pub fn validate(&self) -> Result<(), RateLimitError> {
        if self.max_sources == 0 {
            return Err(RateLimitError::InvalidConfig(
                "max_sources must be greater than 0".to_string(),
            ));
        }

        if self.source_ttl_secs == 0 {
            return Err(RateLimitError::InvalidConfig(
                "source_ttl_secs must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }

    /// Create config with custom global rate
    pub fn with_global_rate(mut self, rate: u32, burst: u32) -> Result<Self, RateLimitError> {
        self.global_rate = NonZeroU32::new(rate)
            .ok_or_else(|| RateLimitError::InvalidConfig("global_rate must be > 0".to_string()))?;
        self.global_burst = NonZeroU32::new(burst)
            .ok_or_else(|| RateLimitError::InvalidConfig("global_burst must be > 0".to_string()))?;
        Ok(self)
    }

    /// Create config with custom per-source rate
    pub fn with_per_source_rate(mut self, rate: u32, burst: u32) -> Result<Self, RateLimitError> {
        self.per_source_rate = Some(NonZeroU32::new(rate)
            .ok_or_else(|| RateLimitError::InvalidConfig("per_source_rate must be > 0".to_string()))?);
        self.per_source_burst = Some(NonZeroU32::new(burst)
            .ok_or_else(|| RateLimitError::InvalidConfig("per_source_burst must be > 0".to_string()))?);
        Ok(self)
    }
}

/// Rate limiter statistics
#[derive(Debug, Clone, Default)]
pub struct RateLimitStats {
    /// Total checks performed
    pub checks_total: u64,
    /// Total checks allowed
    pub checks_allowed: u64,
    /// Total checks denied
    pub checks_denied: u64,
    /// Current number of tracked sources
    pub active_sources: usize,
}

/// Token bucket rate limiter
pub struct RateLimiter {
    /// Configuration
    config: RateLimitConfig,

    /// Global rate limiter
    global_limiter: Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>>,

    /// Per-source rate limiters (cached with LRU eviction)
    source_limiters: Cache<SourceIdentifier, Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>>>,

    /// Statistics
    stats: Arc<DashMap<String, u64>>,

    /// Clock for time operations
    clock: DefaultClock,

    /// Optional telemetry metrics
    metrics: Option<Arc<TelemetryMetrics>>,
}

impl RateLimiter {
    /// Create new rate limiter
    pub fn new(config: RateLimitConfig) -> Result<Self, RateLimitError> {
        config.validate()?;

        // Create global rate limiter
        let global_quota = Quota::per_second(config.global_rate)
            .allow_burst(config.global_burst);
        let global_limiter = Arc::new(GovernorRateLimiter::direct(global_quota));

        // Create LRU cache for per-source limiters
        let source_limiters = Cache::builder()
            .max_capacity(config.max_sources as u64)
            .time_to_idle(Duration::from_secs(config.source_ttl_secs))
            .build();

        // Initialize stats
        let stats = Arc::new(DashMap::new());
        stats.insert("checks_total".to_string(), 0);
        stats.insert("checks_allowed".to_string(), 0);
        stats.insert("checks_denied".to_string(), 0);

        Ok(Self {
            config,
            global_limiter,
            source_limiters,
            stats,
            clock: DefaultClock::default(),
            metrics: None,
        })
    }

    /// Set telemetry metrics
    pub fn with_metrics(mut self, metrics: Arc<TelemetryMetrics>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Check rate limit for an event
    pub async fn check_limit(&self, event: &FeedbackEvent) -> Result<(), RateLimitError> {
        self.increment_stat("checks_total");

        // Check global rate limit first
        if let Err(not_until) = self.global_limiter.check() {
            let retry_after = not_until.wait_time_from(self.clock.now());
            self.increment_stat("checks_denied");

            // Record metric
            if let Some(ref m) = self.metrics {
                m.record_rate_limit_check(false);
            }

            warn!(
                "Global rate limit exceeded. Retry after {:?}",
                retry_after
            );

            return Err(RateLimitError::Exceeded { retry_after });
        }

        // Check per-source rate limit if enabled
        if self.config.per_source_rate.is_some() {
            match SourceIdentifier::from_event(event, &self.config.source_identifier) {
                Ok(source_id) => {
                    if let Err(not_until) = self.check_source_limit(&source_id).await {
                        let retry_after = not_until.wait_time_from(self.clock.now());
                        self.increment_stat("checks_denied");

                        // Record metric
                        if let Some(ref m) = self.metrics {
                            m.record_rate_limit_check(false);
                        }

                        debug!(
                            "Per-source rate limit exceeded for {:?}. Retry after {:?}",
                            source_id, retry_after
                        );

                        return Err(RateLimitError::Exceeded { retry_after });
                    }
                }
                Err(e) => {
                    // If we can't extract source ID, only apply global limit
                    debug!("Could not extract source identifier: {}", e);
                }
            }
        }

        self.increment_stat("checks_allowed");

        // Record successful check
        if let Some(ref m) = self.metrics {
            m.record_rate_limit_check(true);
        }

        Ok(())
    }

    /// Check rate limit for a specific source
    async fn check_source_limit(
        &self,
        source_id: &SourceIdentifier,
    ) -> Result<(), governor::NotUntil<governor::clock::QuantaInstant>> {
        let limiter = self.source_limiters
            .try_get_with(source_id.clone(), async {
                // Create new rate limiter for this source
                let rate = self.config.per_source_rate.unwrap();
                let burst = self.config.per_source_burst.unwrap();
                let quota = Quota::per_second(rate).allow_burst(burst);
                Ok::<_, std::convert::Infallible>(Arc::new(GovernorRateLimiter::direct(quota)))
            })
            .await
            .expect("Infallible");

        limiter.check()
    }

    /// Check rate limit by source ID directly
    pub async fn check_limit_by_source(&self, source_id: &SourceIdentifier) -> Result<(), RateLimitError> {
        self.increment_stat("checks_total");

        // Check global rate limit
        if let Err(not_until) = self.global_limiter.check() {
            let retry_after = not_until.wait_time_from(self.clock.now());
            self.increment_stat("checks_denied");
            return Err(RateLimitError::Exceeded { retry_after });
        }

        // Check per-source rate limit if enabled
        if self.config.per_source_rate.is_some() {
            if let Err(not_until) = self.check_source_limit(source_id).await {
                let retry_after = not_until.wait_time_from(self.clock.now());
                self.increment_stat("checks_denied");
                return Err(RateLimitError::Exceeded { retry_after });
            }
        }

        self.increment_stat("checks_allowed");
        Ok(())
    }

    /// Get current statistics
    pub async fn stats(&self) -> RateLimitStats {
        RateLimitStats {
            checks_total: self.stats.get("checks_total").map_or(0, |r| *r),
            checks_allowed: self.stats.get("checks_allowed").map_or(0, |r| *r),
            checks_denied: self.stats.get("checks_denied").map_or(0, |r| *r),
            active_sources: self.source_limiters.entry_count() as usize,
        }
    }

    /// Reset statistics (useful for testing)
    pub fn reset_stats(&self) {
        self.stats.insert("checks_total".to_string(), 0);
        self.stats.insert("checks_allowed".to_string(), 0);
        self.stats.insert("checks_denied".to_string(), 0);
    }

    /// Get configuration
    pub fn config(&self) -> &RateLimitConfig {
        &self.config
    }

    /// Increment a statistic
    fn increment_stat(&self, key: &str) {
        self.stats.entry(key.to_string())
            .and_modify(|v| *v += 1)
            .or_insert(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feedback_events::UserFeedbackEvent;
    use std::time::Duration;
    use tokio::time::sleep;
    use uuid::Uuid;

    #[test]
    fn test_rate_limit_config_default() {
        let config = RateLimitConfig::default();
        assert_eq!(config.global_rate.get(), 10_000);
        assert_eq!(config.global_burst.get(), 10_000);
        assert_eq!(config.per_source_rate.unwrap().get(), 100);
        assert_eq!(config.max_sources, 10_000);
    }

    #[test]
    fn test_rate_limit_config_validation() {
        let mut config = RateLimitConfig::default();
        config.max_sources = 0;
        assert!(config.validate().is_err());

        config.max_sources = 100;
        config.source_ttl_secs = 0;
        assert!(config.validate().is_err());

        config.source_ttl_secs = 60;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_rate_limit_config_builder() {
        let config = RateLimitConfig::default()
            .with_global_rate(5000, 5000)
            .unwrap()
            .with_per_source_rate(50, 100)
            .unwrap();

        assert_eq!(config.global_rate.get(), 5000);
        assert_eq!(config.per_source_rate.unwrap().get(), 50);
    }

    #[tokio::test]
    async fn test_rate_limiter_creation() {
        let config = RateLimitConfig::default();
        let limiter = RateLimiter::new(config);
        assert!(limiter.is_ok());
    }

    #[tokio::test]
    async fn test_global_rate_limit_allows_within_quota() {
        let config = RateLimitConfig::default()
            .with_global_rate(100, 100)
            .unwrap();

        let limiter = RateLimiter::new(config).unwrap();

        // Create test event
        let request_id = Uuid::new_v4();
        let event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new("session1", request_id)
        );

        // First few requests should succeed
        for i in 0..10 {
            let result = limiter.check_limit(&event).await;
            assert!(result.is_ok(), "Request {} should be allowed", i);
        }
    }

    #[tokio::test]
    async fn test_global_rate_limit_denies_over_quota() {
        // Very low rate for testing
        let config = RateLimitConfig::default()
            .with_global_rate(10, 10)
            .unwrap();

        let limiter = RateLimiter::new(config).unwrap();

        let request_id = Uuid::new_v4();
        let event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new("session1", request_id)
        );

        // Exhaust the quota
        let mut allowed = 0;
        let mut denied = 0;

        for _ in 0..20 {
            match limiter.check_limit(&event).await {
                Ok(_) => allowed += 1,
                Err(RateLimitError::Exceeded { .. }) => denied += 1,
                Err(e) => panic!("Unexpected error: {}", e),
            }
        }

        assert!(allowed <= 10, "Should allow at most 10 requests");
        assert!(denied > 0, "Should deny some requests");
    }

    #[tokio::test]
    async fn test_per_source_rate_limit() {
        let config = RateLimitConfig::default()
            .with_global_rate(1000, 1000)
            .unwrap()
            .with_per_source_rate(5, 5)
            .unwrap();

        let limiter = RateLimiter::new(config).unwrap();

        let request_id = Uuid::new_v4();

        // Create events from different sessions
        let session1_event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new("session1", request_id)
        );
        let session2_event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new("session2", request_id)
        );

        // Both sessions should get their own quota
        for _ in 0..5 {
            assert!(limiter.check_limit(&session1_event).await.is_ok());
            assert!(limiter.check_limit(&session2_event).await.is_ok());
        }

        // Next requests should be rate limited
        assert!(limiter.check_limit(&session1_event).await.is_err());
        assert!(limiter.check_limit(&session2_event).await.is_err());
    }

    #[tokio::test]
    async fn test_source_identifier_extraction_session_id() {
        let request_id = Uuid::new_v4();
        let event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new("session123", request_id)
        );

        let strategy = SourceIdentifierStrategy::SessionId;
        let source_id = SourceIdentifier::from_event(&event, &strategy).unwrap();

        match source_id {
            SourceIdentifier::SessionId(id) => assert_eq!(id, "session123"),
            _ => panic!("Expected SessionId"),
        }
    }

    #[tokio::test]
    async fn test_source_identifier_extraction_user_id() {
        let request_id = Uuid::new_v4();
        let event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new("session1", request_id)
                .with_user_id("user456")
        );

        let strategy = SourceIdentifierStrategy::UserId;
        let source_id = SourceIdentifier::from_event(&event, &strategy).unwrap();

        match source_id {
            SourceIdentifier::UserId(id) => assert_eq!(id, "user456"),
            _ => panic!("Expected UserId"),
        }
    }

    #[tokio::test]
    async fn test_rate_limit_stats() {
        let config = RateLimitConfig::default()
            .with_global_rate(10, 10)
            .unwrap();

        let limiter = RateLimiter::new(config).unwrap();

        let request_id = Uuid::new_v4();
        let event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new("session1", request_id)
        );

        // Make some requests
        for _ in 0..15 {
            let _ = limiter.check_limit(&event).await;
        }

        let stats = limiter.stats().await;
        assert_eq!(stats.checks_total, 15);
        assert!(stats.checks_allowed > 0);
        assert!(stats.checks_denied > 0);
    }

    #[tokio::test]
    async fn test_burst_allowance() {
        let config = RateLimitConfig::default()
            .with_global_rate(10, 20)
            .unwrap();

        let limiter = RateLimiter::new(config).unwrap();

        let request_id = Uuid::new_v4();
        let event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new("session1", request_id)
        );

        // Burst should allow more than the per-second rate initially
        let mut allowed = 0;
        for _ in 0..25 {
            if limiter.check_limit(&event).await.is_ok() {
                allowed += 1;
            }
        }

        // Should allow around 20 (burst capacity)
        assert!(allowed >= 10 && allowed <= 20, "Allowed: {}", allowed);
    }

    #[tokio::test]
    async fn test_rate_limit_recovery() {
        let config = RateLimitConfig::default()
            .with_global_rate(10, 10)
            .unwrap();

        let limiter = RateLimiter::new(config).unwrap();

        let request_id = Uuid::new_v4();
        let event = FeedbackEvent::UserFeedback(
            UserFeedbackEvent::new("session1", request_id)
        );

        // Exhaust quota
        for _ in 0..15 {
            let _ = limiter.check_limit(&event).await;
        }

        // Should be rate limited
        assert!(limiter.check_limit(&event).await.is_err());

        // Wait for quota to refill
        sleep(Duration::from_millis(200)).await;

        // Should work again
        assert!(limiter.check_limit(&event).await.is_ok());
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let config = RateLimitConfig::default()
            .with_global_rate(100, 100)
            .unwrap();

        let limiter = Arc::new(RateLimiter::new(config).unwrap());

        let mut handles = vec![];

        // Spawn multiple concurrent tasks
        for i in 0..10 {
            let limiter = limiter.clone();
            let handle = tokio::spawn(async move {
                let request_id = Uuid::new_v4();
                let event = FeedbackEvent::UserFeedback(
                    UserFeedbackEvent::new(format!("session{}", i), request_id)
                );

                limiter.check_limit(&event).await
            });
            handles.push(handle);
        }

        // Wait for all tasks
        for handle in handles {
            let result = handle.await.unwrap();
            // Some should succeed
            if result.is_err() {
                match result.unwrap_err() {
                    RateLimitError::Exceeded { .. } => {},
                    e => panic!("Unexpected error: {}", e),
                }
            }
        }
    }

    #[tokio::test]
    async fn test_source_limiter_lru_eviction() {
        let config = RateLimitConfig {
            max_sources: 5,
            source_ttl_secs: 1,
            ..Default::default()
        };

        let limiter = RateLimiter::new(config).unwrap();

        let request_id = Uuid::new_v4();

        // Create events from many different sessions
        for i in 0..10 {
            let event = FeedbackEvent::UserFeedback(
                UserFeedbackEvent::new(format!("session{}", i), request_id)
            );
            let _ = limiter.check_limit(&event).await;
        }

        let stats = limiter.stats().await;
        // Should not exceed max_sources due to LRU eviction
        assert!(stats.active_sources <= 5, "Active sources: {}", stats.active_sources);
    }

    #[tokio::test]
    async fn test_check_limit_by_source() {
        let config = RateLimitConfig::default()
            .with_per_source_rate(5, 5)
            .unwrap();

        let limiter = RateLimiter::new(config).unwrap();

        let source_id = SourceIdentifier::SessionId("test-session".to_string());

        // First 5 should succeed
        for _ in 0..5 {
            assert!(limiter.check_limit_by_source(&source_id).await.is_ok());
        }

        // Next should fail
        assert!(limiter.check_limit_by_source(&source_id).await.is_err());
    }
}
