//! Redis-based event deduplicator implementation

use super::{
    circuit_breaker::{CircuitBreaker, CircuitBreakerState},
    config::DeduplicationConfig,
    error::{DeduplicationError, DeduplicationResult},
    extractors::EventIdExtractor,
    id::EventId,
    stats::{DeduplicationStats, HealthState, HealthStatus},
    strategy::DeduplicationStrategy,
    EventDeduplicator,
};
use crate::state::RedisStateBackend;
use async_trait::async_trait;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, error, info, trace, warn};

/// Redis-based event deduplicator with comprehensive features
///
/// Provides O(1) duplicate detection using Redis SET NX operations,
/// configurable TTL, circuit breaker for fault tolerance, and
/// comprehensive metrics.
pub struct RedisEventDeduplicator<T, E> {
    /// Redis backend for storage
    redis: Arc<RedisStateBackend>,

    /// Event ID extraction strategy
    id_extractor: Arc<E>,

    /// Configuration
    config: DeduplicationConfig,

    /// Statistics tracking
    stats: Arc<RwLock<DeduplicationStats>>,

    /// Circuit breaker for Redis failures
    circuit_breaker: Arc<CircuitBreaker>,

    /// Phantom data for event type
    _phantom: PhantomData<T>,
}

impl<T, E> RedisEventDeduplicator<T, E>
where
    T: Send + Sync,
    E: EventIdExtractor<T> + Send + Sync,
{
    /// Create a new Redis-based deduplicator
    ///
    /// # Arguments
    ///
    /// * `redis` - Redis state backend
    /// * `id_extractor` - Event ID extraction strategy
    /// * `config` - Deduplication configuration
    pub async fn new(
        redis: RedisStateBackend,
        id_extractor: E,
        config: DeduplicationConfig,
    ) -> DeduplicationResult<Self> {
        // Validate configuration
        config.validate().map_err(|e| DeduplicationError::InvalidConfig {
            reason: e.to_string(),
        })?;

        let circuit_breaker = CircuitBreaker::new(
            config.circuit_breaker_threshold,
            config.circuit_breaker_timeout,
        );

        info!(
            strategy = %config.strategy,
            ttl_secs = config.ttl.as_secs(),
            "Redis event deduplicator initialized"
        );

        Ok(Self {
            redis: Arc::new(redis),
            id_extractor: Arc::new(id_extractor),
            config,
            stats: Arc::new(RwLock::new(DeduplicationStats::default())),
            circuit_breaker: Arc::new(circuit_breaker),
            _phantom: PhantomData,
        })
    }

    /// Build full Redis key for event ID
    fn build_dedup_key(&self, event_id: &EventId) -> Vec<u8> {
        format!("{}{}", self.config.key_prefix, event_id.as_str()).into_bytes()
    }

    /// Extract event ID with error handling
    async fn extract_event_id(&self, event: &T) -> DeduplicationResult<EventId> {
        match self.id_extractor.extract_id(event) {
            Ok(id) => {
                trace!(event_id = %id, "Event ID extracted");
                Ok(id)
            }
            Err(e) => {
                error!(error = %e, "Failed to extract event ID");
                let mut stats = self.stats.write().await;
                stats.extraction_failures += 1;

                match self.config.strategy {
                    DeduplicationStrategy::Strict => Err(e),
                    _ => {
                        warn!("Allowing event due to extraction failure");
                        stats.fallback_count += 1;
                        // Create a fallback ID to allow the event through
                        Err(e) // Still return error but caller handles it
                    }
                }
            }
        }
    }

    /// Check duplicate with circuit breaker protection
    async fn check_duplicate_internal(&self, event_id: &EventId) -> DeduplicationResult<bool> {
        // Check circuit breaker state
        if self.circuit_breaker.is_open().await {
            return self.handle_circuit_breaker_open();
        }

        let key = self.build_dedup_key(event_id);
        trace!(event_id = %event_id, "Checking duplicate in Redis");

        // Use Redis SET NX (set if not exists) - atomic operation
        match self
            .redis
            .set_if_not_exists(&key, b"1", self.config.ttl)
            .await
        {
            Ok(was_set) => {
                self.circuit_breaker.record_success().await;

                // If set succeeded, key didn't exist = unique event
                // If set failed, key existed = duplicate event
                let is_duplicate = !was_set;

                if is_duplicate {
                    debug!(event_id = %event_id, "Duplicate event detected");
                } else {
                    trace!(event_id = %event_id, "Unique event marked");
                }

                Ok(is_duplicate)
            }
            Err(e) => {
                error!(error = %e, event_id = %event_id, "Redis operation failed");
                self.circuit_breaker.record_failure().await;

                let mut stats = self.stats.write().await;
                stats.redis_errors += 1;
                stats.last_error_time = Some(chrono::Utc::now());

                match self.config.strategy {
                    DeduplicationStrategy::Strict => Err(e.into()),
                    _ => {
                        warn!(
                            strategy = %self.config.strategy,
                            "Redis unavailable, treating event as unique"
                        );
                        stats.fallback_count += 1;
                        Ok(false) // Treat as unique
                    }
                }
            }
        }
    }

    /// Handle circuit breaker being open
    fn handle_circuit_breaker_open(&self) -> DeduplicationResult<bool> {
        warn!("Circuit breaker is open");

        match self.config.strategy {
            DeduplicationStrategy::Strict => Err(DeduplicationError::CircuitBreakerOpen),
            _ => {
                info!("Allowing event through due to circuit breaker");
                Ok(false) // Treat as unique
            }
        }
    }

    /// Update latency statistics
    async fn update_latency(&self, duration: std::time::Duration) {
        let mut stats = self.stats.write().await;
        let latency_us = duration.as_micros() as u64;

        stats.total_latency_us += latency_us;

        // Update average
        if stats.total_checked > 0 {
            stats.avg_latency_us = stats.total_latency_us / stats.total_checked;
        }

        // TODO: Update percentiles using a proper histogram
        // For now, use a simple approximation
        if latency_us > stats.p99_latency_us {
            stats.p99_latency_us = latency_us;
        }
        if latency_us > stats.p95_latency_us {
            stats.p95_latency_us = latency_us;
        }
        if latency_us > stats.p50_latency_us || stats.p50_latency_us == 0 {
            stats.p50_latency_us = latency_us;
        }
    }
}

#[async_trait]
impl<T, E> EventDeduplicator for RedisEventDeduplicator<T, E>
where
    T: Send + Sync,
    E: EventIdExtractor<T> + Send + Sync,
{
    async fn is_duplicate<U>(&self, event: &U) -> DeduplicationResult<bool>
    where
        U: Send + Sync,
    {
        // This is a type-erased version - in practice, we can't check type U
        // This is here for trait compatibility but won't be commonly used
        // Users should use check_and_mark instead
        Err(DeduplicationError::ExtractionFailed {
            reason: "Type mismatch - use check_and_mark with correct event type".to_string(),
        })
    }

    async fn mark_seen<U>(&self, event: &U) -> DeduplicationResult<()>
    where
        U: Send + Sync,
    {
        // Same type-erased version
        Err(DeduplicationError::ExtractionFailed {
            reason: "Type mismatch - use check_and_mark with correct event type".to_string(),
        })
    }

    async fn check_and_mark<U>(&self, event: &U) -> DeduplicationResult<bool>
    where
        U: Send + Sync,
    {
        // Same type-erased version
        Err(DeduplicationError::ExtractionFailed {
            reason: "Type mismatch - use the typed check_and_mark_event method".to_string(),
        })
    }

    async fn stats(&self) -> DeduplicationStats {
        self.stats.read().await.clone()
    }

    async fn health_check(&self) -> DeduplicationResult<HealthStatus> {
        let redis_available = self.redis.health_check().await.unwrap_or(false);
        let cb_state = self.circuit_breaker.state().await;
        let stats = self.stats.read().await.clone();

        let mut messages = Vec::new();

        // Determine overall health
        let status = match (redis_available, cb_state) {
            (true, CircuitBreakerState::Closed) => {
                if stats.error_rate() < 0.05 {
                    HealthState::Healthy
                } else {
                    messages.push(format!(
                        "High error rate: {:.1}%",
                        stats.error_rate() * 100.0
                    ));
                    HealthState::Degraded
                }
            }
            (true, CircuitBreakerState::HalfOpen) => {
                messages.push("Circuit breaker in half-open state".to_string());
                HealthState::Degraded
            }
            (false, _) | (_, CircuitBreakerState::Open) => {
                if !redis_available {
                    messages.push("Redis is unavailable".to_string());
                }
                if matches!(cb_state, CircuitBreakerState::Open) {
                    messages.push("Circuit breaker is open".to_string());
                }
                HealthState::Unhealthy
            }
        };

        Ok(HealthStatus {
            status,
            redis_available,
            circuit_breaker_state: cb_state,
            last_check_time: stats.last_check_time,
            last_error_time: stats.last_error_time,
            stats,
            messages,
        })
    }

    async fn clear(&self) -> DeduplicationResult<()> {
        warn!("Clearing all deduplication state");

        // This would clear all dedup keys, but we need to be careful
        // For now, we'll just clear stats
        let mut stats = self.stats.write().await;
        *stats = DeduplicationStats::default();

        // Reset circuit breaker
        self.circuit_breaker.reset().await;

        Ok(())
    }
}

// Typed methods that work with the actual event type
impl<T, E> RedisEventDeduplicator<T, E>
where
    T: Send + Sync,
    E: EventIdExtractor<T> + Send + Sync,
{
    /// Check if an event is a duplicate (typed version)
    pub async fn is_duplicate_event(&self, event: &T) -> DeduplicationResult<bool> {
        let event_id = self.extract_event_id(event).await?;
        let key = self.build_dedup_key(&event_id);

        match self.redis.contains(&key).await {
            Ok(exists) => {
                self.circuit_breaker.record_success().await;
                Ok(exists)
            }
            Err(e) => {
                self.circuit_breaker.record_failure().await;
                match self.config.strategy {
                    DeduplicationStrategy::Strict => Err(e.into()),
                    _ => Ok(false),
                }
            }
        }
    }

    /// Mark an event as seen (typed version)
    pub async fn mark_seen_event(&self, event: &T) -> DeduplicationResult<()> {
        let event_id = self.extract_event_id(event).await?;
        let key = self.build_dedup_key(&event_id);

        match self.redis.put_with_ttl(&key, b"1", self.config.ttl).await {
            Ok(()) => {
                self.circuit_breaker.record_success().await;
                Ok(())
            }
            Err(e) => {
                self.circuit_breaker.record_failure().await;
                match self.config.strategy {
                    DeduplicationStrategy::Strict => Err(e.into()),
                    _ => Ok(()),
                }
            }
        }
    }

    /// Check and mark in a single atomic operation (typed version - PREFERRED)
    ///
    /// This is the recommended method as it combines duplicate checking and
    /// marking in a single Redis operation for better performance and atomicity.
    pub async fn check_and_mark_event(&self, event: &T) -> DeduplicationResult<bool> {
        let start = Instant::now();

        // Extract event ID
        let event_id = match self.extract_event_id(event).await {
            Ok(id) => id,
            Err(_) => {
                // Already logged and stats updated in extract_event_id
                return match self.config.strategy {
                    DeduplicationStrategy::Strict => Err(DeduplicationError::ExtractionFailed {
                        reason: "Failed to extract event ID".to_string(),
                    }),
                    _ => Ok(false), // Allow through on extraction failure
                };
            }
        };

        // Check duplicate
        let is_duplicate = self.check_duplicate_internal(&event_id).await?;

        // Update stats
        let latency = start.elapsed();
        let mut stats = self.stats.write().await;
        stats.total_checked += 1;
        stats.last_check_time = Some(chrono::Utc::now());
        stats.circuit_breaker_state = self.circuit_breaker.state().await;

        if is_duplicate {
            stats.duplicates_found += 1;
            if self.config.strategy.should_filter() {
                debug!(event_id = %event_id, "Filtering duplicate event");
            } else {
                info!(event_id = %event_id, "Duplicate detected (log-only mode)");
            }
        } else {
            stats.unique_events += 1;
        }

        drop(stats); // Release lock before updating latency
        self.update_latency(latency).await;

        // In log-only mode, always return false (allow through)
        if !self.config.strategy.should_filter() {
            Ok(false)
        } else {
            Ok(is_duplicate)
        }
    }

    /// Batch check and mark multiple events
    pub async fn check_and_mark_batch(&self, events: &[T]) -> DeduplicationResult<Vec<bool>> {
        let mut results = Vec::with_capacity(events.len());

        for event in events {
            let is_dup = self.check_and_mark_event(event).await?;
            results.push(is_dup);
        }

        Ok(results)
    }

    /// Filter a batch of events, removing duplicates
    pub async fn filter_batch(&self, events: Vec<T>) -> DeduplicationResult<Vec<T>>
    where
        T: Clone,
    {
        let mut unique = Vec::new();

        for event in events {
            let is_dup = self.check_and_mark_event(&event).await?;
            if !is_dup {
                unique.push(event);
            }
        }

        Ok(unique)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deduplication::extractors::FeedbackEventUuidExtractor;
    use crate::state::RedisConfig;
    use llm_optimizer_types::events::{EventSource, EventType, FeedbackEvent};
    use std::time::Duration;

    async fn create_test_deduplicator(
    ) -> Option<RedisEventDeduplicator<FeedbackEvent, FeedbackEventUuidExtractor>> {
        let redis_config = RedisConfig::builder()
            .url("redis://localhost:6379")
            .key_prefix("test:dedup:")
            .default_ttl(Duration::from_secs(60))
            .build()
            .ok()?;

        let redis = RedisStateBackend::new(redis_config).await.ok()?;

        let config = DeduplicationConfig::builder()
            .strategy(DeduplicationStrategy::BestEffort)
            .ttl(Duration::from_secs(60))
            .key_prefix("dedup:".to_string())
            .build()
            .ok()?;

        let deduplicator =
            RedisEventDeduplicator::new(redis, FeedbackEventUuidExtractor, config)
                .await
                .ok()?;

        // Clear existing state
        let _ = deduplicator.clear().await;

        Some(deduplicator)
    }

    #[tokio::test]
    async fn test_deduplicator_basic() {
        let Some(dedup) = create_test_deduplicator().await else {
            eprintln!("Skipping test: Redis not available");
            return;
        };

        let event = FeedbackEvent::new(
            EventSource::Observatory,
            EventType::Metric,
            serde_json::json!({"test": "data"}),
        );

        // First check should be unique
        let is_dup1 = dedup.check_and_mark_event(&event).await.unwrap();
        assert!(!is_dup1);

        // Second check should be duplicate
        let is_dup2 = dedup.check_and_mark_event(&event).await.unwrap();
        assert!(is_dup2);

        // Stats should reflect this
        let stats = dedup.stats().await;
        assert_eq!(stats.total_checked, 2);
        assert_eq!(stats.unique_events, 1);
        assert_eq!(stats.duplicates_found, 1);
    }

    #[tokio::test]
    async fn test_health_check() {
        let Some(dedup) = create_test_deduplicator().await else {
            eprintln!("Skipping test: Redis not available");
            return;
        };

        let health = dedup.health_check().await.unwrap();
        assert!(health.redis_available);
        assert_eq!(health.circuit_breaker_state, CircuitBreakerState::Closed);
    }
}
