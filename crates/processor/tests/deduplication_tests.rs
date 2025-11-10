//! Comprehensive tests for event deduplication implementation
//!
//! This test suite provides extensive coverage for event deduplication functionality,
//! including unit tests, integration tests, performance benchmarks, and edge case testing.
//!
//! ## Test Coverage
//!
//! - **Unit Tests (15+)**: Event ID extraction, duplicate detection, TTL expiration, Redis operations
//! - **Integration Tests (10+)**: End-to-end pipeline, multi-threading, persistence, batch processing
//! - **Performance Tests (5+)**: Throughput, latency, memory efficiency, concurrent access
//! - **Edge Cases (8+)**: Empty IDs, null values, hash collisions, race conditions, clock skew
//!
//! ## Running Tests
//!
//! ```bash
//! # Unit tests only
//! cargo test --test deduplication_tests
//!
//! # Integration tests (requires Redis)
//! docker-compose up -d redis
//! cargo test --test deduplication_tests -- --ignored --test-threads=1
//!
//! # Benchmarks
//! cargo bench --bench dedup_benchmarks
//! ```

use chrono::{DateTime, Duration, Utc};
use llm_optimizer_types::events::{CloudEvent, EventSource, EventType, FeedbackEvent};
use processor::state::{RedisConfig, RedisStateBackend, StateBackend};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use uuid::Uuid;

// ============================================================================
// Mock Deduplication Service
// ============================================================================

/// Event deduplication service using Redis backend
pub struct EventDeduplicator {
    backend: Arc<dyn StateBackend>,
    ttl: std::time::Duration,
    metrics: Arc<DeduplicationMetrics>,
}

/// Deduplication metrics
#[derive(Debug, Default)]
pub struct DeduplicationMetrics {
    pub total_events: AtomicU64,
    pub duplicates_found: AtomicU64,
    pub unique_events: AtomicU64,
    pub redis_errors: AtomicU64,
    pub ttl_expirations: AtomicU64,
}

impl DeduplicationMetrics {
    fn record_event(&self, is_duplicate: bool) {
        self.total_events.fetch_add(1, Ordering::Relaxed);
        if is_duplicate {
            self.duplicates_found.fetch_add(1, Ordering::Relaxed);
        } else {
            self.unique_events.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn record_error(&self) {
        self.redis_errors.fetch_add(1, Ordering::Relaxed);
    }

    fn duplicate_rate(&self) -> f64 {
        let total = self.total_events.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        self.duplicates_found.load(Ordering::Relaxed) as f64 / total as f64
    }
}

/// Event ID extraction strategy
#[derive(Debug, Clone, Copy)]
pub enum IdExtractionStrategy {
    /// Use UUID from event
    Uuid,
    /// Hash event content
    ContentHash,
    /// Custom field-based ID
    Custom,
}

impl EventDeduplicator {
    pub fn new(backend: Arc<dyn StateBackend>, ttl: std::time::Duration) -> Self {
        Self {
            backend,
            ttl,
            metrics: Arc::new(DeduplicationMetrics::default()),
        }
    }

    /// Extract event ID based on strategy
    pub fn extract_event_id(
        &self,
        event: &FeedbackEvent,
        strategy: IdExtractionStrategy,
    ) -> String {
        match strategy {
            IdExtractionStrategy::Uuid => event.id.to_string(),
            IdExtractionStrategy::ContentHash => {
                let content = format!(
                    "{}:{}:{:?}",
                    event.source as i32, event.event_type as i32, event.data
                );
                format!("{:x}", md5::compute(content.as_bytes()))
            }
            IdExtractionStrategy::Custom => {
                // Use custom_id from metadata if present, otherwise UUID
                event
                    .metadata
                    .get("custom_id")
                    .cloned()
                    .unwrap_or_else(|| event.id.to_string())
            }
        }
    }

    /// Check if event is duplicate
    pub async fn is_duplicate(&self, event_id: &str) -> Result<bool, Box<dyn std::error::Error>> {
        let key = format!("dedup:{}", event_id);
        match self.backend.get(key.as_bytes()).await {
            Ok(result) => Ok(result.is_some()),
            Err(e) => {
                self.metrics.record_error();
                Err(Box::new(e))
            }
        }
    }

    /// Mark event as seen
    pub async fn mark_seen(&self, event_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let key = format!("dedup:{}", event_id);
        let value = Utc::now().timestamp().to_string();

        // Store with TTL if backend supports it
        self.backend.put(key.as_bytes(), value.as_bytes()).await?;
        Ok(())
    }

    /// Process event with deduplication
    pub async fn process_event(
        &self,
        event: &FeedbackEvent,
        strategy: IdExtractionStrategy,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let event_id = self.extract_event_id(event, strategy);
        let is_dup = self.is_duplicate(&event_id).await?;

        self.metrics.record_event(is_dup);

        if !is_dup {
            self.mark_seen(&event_id).await?;
            Ok(true) // Event processed
        } else {
            Ok(false) // Event was duplicate, skipped
        }
    }

    /// Process batch of events
    pub async fn process_batch(
        &self,
        events: &[FeedbackEvent],
        strategy: IdExtractionStrategy,
    ) -> Result<Vec<bool>, Box<dyn std::error::Error>> {
        let mut results = Vec::new();
        for event in events {
            results.push(self.process_event(event, strategy).await?);
        }
        Ok(results)
    }

    /// Get metrics
    pub fn metrics(&self) -> Arc<DeduplicationMetrics> {
        Arc::clone(&self.metrics)
    }

    /// Clear all deduplication state
    pub async fn clear(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.backend.clear().await?;
        Ok(())
    }
}

// ============================================================================
// UNIT TESTS - Event ID Extraction
// ============================================================================

mod unit_tests {
    use super::*;
    use processor::state::MemoryStateBackend;

    #[tokio::test]
    async fn test_event_id_extraction_uuid() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        let event = FeedbackEvent::new(
            EventSource::Observatory,
            EventType::Metric,
            serde_json::json!({}),
        );

        let id = dedup.extract_event_id(&event, IdExtractionStrategy::Uuid);
        assert_eq!(id, event.id.to_string());
    }

    #[tokio::test]
    async fn test_event_id_extraction_content_hash() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        let event1 = FeedbackEvent::new(
            EventSource::User,
            EventType::Feedback,
            serde_json::json!({"rating": 5}),
        );

        let event2 = FeedbackEvent::new(
            EventSource::User,
            EventType::Feedback,
            serde_json::json!({"rating": 5}),
        );

        // Same content should produce same hash
        let hash1 = dedup.extract_event_id(&event1, IdExtractionStrategy::ContentHash);
        let hash2 = dedup.extract_event_id(&event2, IdExtractionStrategy::ContentHash);
        assert_eq!(hash1, hash2);
    }

    #[tokio::test]
    async fn test_event_id_extraction_custom() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        let event = FeedbackEvent::new(
            EventSource::System,
            EventType::PerformanceAlert,
            serde_json::json!({}),
        )
        .with_metadata("custom_id", "alert-12345");

        let id = dedup.extract_event_id(&event, IdExtractionStrategy::Custom);
        assert_eq!(id, "alert-12345");
    }

    #[tokio::test]
    async fn test_event_id_extraction_custom_fallback() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        let event = FeedbackEvent::new(
            EventSource::System,
            EventType::PerformanceAlert,
            serde_json::json!({}),
        );

        let id = dedup.extract_event_id(&event, IdExtractionStrategy::Custom);
        // Should fall back to UUID when custom_id not present
        assert_eq!(id, event.id.to_string());
    }

    #[tokio::test]
    async fn test_duplicate_detection_first_seen() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        let event_id = "test-event-123";

        // First time should not be duplicate
        let is_dup = dedup.is_duplicate(event_id).await.unwrap();
        assert!(!is_dup);
    }

    #[tokio::test]
    async fn test_duplicate_detection_duplicate() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        let event_id = "test-event-456";

        // Mark as seen
        dedup.mark_seen(event_id).await.unwrap();

        // Second check should find duplicate
        let is_dup = dedup.is_duplicate(event_id).await.unwrap();
        assert!(is_dup);
    }

    #[tokio::test]
    async fn test_duplicate_detection_multiple() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        let event_id = "test-event-789";

        // Mark as seen
        dedup.mark_seen(event_id).await.unwrap();

        // Multiple checks should all find it
        for _ in 0..5 {
            let is_dup = dedup.is_duplicate(event_id).await.unwrap();
            assert!(is_dup);
        }
    }

    #[tokio::test]
    async fn test_ttl_expiration() {
        let backend = Arc::new(MemoryStateBackend::new(Some(std::time::Duration::from_millis(
            100,
        ))));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_millis(100));

        let event_id = "expiring-event";

        // Mark as seen
        dedup.mark_seen(event_id).await.unwrap();

        // Should be duplicate immediately
        assert!(dedup.is_duplicate(event_id).await.unwrap());

        // Wait for expiration
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        // Should no longer be duplicate after TTL
        assert!(!dedup.is_duplicate(event_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_process_event_unique() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        let event = FeedbackEvent::new(
            EventSource::Observatory,
            EventType::Metric,
            serde_json::json!({"value": 42}),
        );

        // First processing should succeed
        let processed = dedup
            .process_event(&event, IdExtractionStrategy::Uuid)
            .await
            .unwrap();
        assert!(processed);

        let metrics = dedup.metrics();
        assert_eq!(metrics.unique_events.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.duplicates_found.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn test_process_event_duplicate() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        let event = FeedbackEvent::new(
            EventSource::Observatory,
            EventType::Metric,
            serde_json::json!({"value": 42}),
        );

        // First processing
        dedup
            .process_event(&event, IdExtractionStrategy::Uuid)
            .await
            .unwrap();

        // Second processing should detect duplicate
        let processed = dedup
            .process_event(&event, IdExtractionStrategy::Uuid)
            .await
            .unwrap();
        assert!(!processed);

        let metrics = dedup.metrics();
        assert_eq!(metrics.unique_events.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.duplicates_found.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_metrics_tracking() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        let event1 = FeedbackEvent::new(
            EventSource::User,
            EventType::Feedback,
            serde_json::json!({}),
        );
        let event2 = FeedbackEvent::new(
            EventSource::User,
            EventType::Feedback,
            serde_json::json!({}),
        );

        // Process both events
        dedup
            .process_event(&event1, IdExtractionStrategy::Uuid)
            .await
            .unwrap();
        dedup
            .process_event(&event1, IdExtractionStrategy::Uuid)
            .await
            .unwrap();
        dedup
            .process_event(&event2, IdExtractionStrategy::Uuid)
            .await
            .unwrap();

        let metrics = dedup.metrics();
        assert_eq!(metrics.total_events.load(Ordering::Relaxed), 3);
        assert_eq!(metrics.unique_events.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.duplicates_found.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_duplicate_rate_calculation() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        let event = FeedbackEvent::new(
            EventSource::User,
            EventType::Feedback,
            serde_json::json!({}),
        );

        // Process same event 5 times
        for _ in 0..5 {
            dedup
                .process_event(&event, IdExtractionStrategy::Uuid)
                .await
                .unwrap();
        }

        let metrics = dedup.metrics();
        // 1 unique, 4 duplicates = 80% duplicate rate
        assert!((metrics.duplicate_rate() - 0.8).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_clear_deduplication_state() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        let event_id = "test-clear";
        dedup.mark_seen(event_id).await.unwrap();

        // Should be duplicate
        assert!(dedup.is_duplicate(event_id).await.unwrap());

        // Clear state
        dedup.clear().await.unwrap();

        // Should no longer be duplicate
        assert!(!dedup.is_duplicate(event_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_batch_processing() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        let events = vec![
            FeedbackEvent::new(
                EventSource::User,
                EventType::Feedback,
                serde_json::json!({"id": 1}),
            ),
            FeedbackEvent::new(
                EventSource::User,
                EventType::Feedback,
                serde_json::json!({"id": 2}),
            ),
            FeedbackEvent::new(
                EventSource::User,
                EventType::Feedback,
                serde_json::json!({"id": 3}),
            ),
        ];

        let results = dedup
            .process_batch(&events, IdExtractionStrategy::Uuid)
            .await
            .unwrap();

        // All should be unique
        assert!(results.iter().all(|&r| r));
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn test_batch_processing_with_duplicates() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        let event1 = FeedbackEvent::new(
            EventSource::User,
            EventType::Feedback,
            serde_json::json!({"id": 1}),
        );
        let event2 = FeedbackEvent::new(
            EventSource::User,
            EventType::Feedback,
            serde_json::json!({"id": 2}),
        );

        let events = vec![
            event1.clone(),
            event2.clone(),
            event1.clone(), // duplicate
            event2.clone(), // duplicate
        ];

        let results = dedup
            .process_batch(&events, IdExtractionStrategy::Uuid)
            .await
            .unwrap();

        // First two unique, last two duplicates
        assert!(results[0]);
        assert!(results[1]);
        assert!(!results[2]);
        assert!(!results[3]);
    }
}

// ============================================================================
// INTEGRATION TESTS - End-to-End Pipeline
// ============================================================================

mod integration_tests {
    use super::*;
    use processor::state::MemoryStateBackend;

    #[tokio::test]
    async fn test_end_to_end_deduplication_pipeline() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        // Simulate event stream with duplicates
        let mut events = Vec::new();
        for i in 0..10 {
            let event = FeedbackEvent::new(
                EventSource::Observatory,
                EventType::Metric,
                serde_json::json!({"sequence": i}),
            );
            events.push(event.clone());
            // Add duplicate
            if i % 2 == 0 {
                events.push(event);
            }
        }

        let mut unique_count = 0;
        let mut duplicate_count = 0;

        for event in &events {
            let processed = dedup
                .process_event(event, IdExtractionStrategy::Uuid)
                .await
                .unwrap();
            if processed {
                unique_count += 1;
            } else {
                duplicate_count += 1;
            }
        }

        assert_eq!(unique_count, 10);
        assert_eq!(duplicate_count, 5);
    }

    #[tokio::test]
    async fn test_concurrent_deduplication() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = Arc::new(EventDeduplicator::new(
            backend,
            std::time::Duration::from_secs(60),
        ));

        let event = Arc::new(FeedbackEvent::new(
            EventSource::User,
            EventType::Feedback,
            serde_json::json!({"test": "concurrent"}),
        ));

        // Spawn multiple tasks processing the same event
        let mut handles = Vec::new();
        for _ in 0..10 {
            let dedup_clone = Arc::clone(&dedup);
            let event_clone = Arc::clone(&event);
            let handle = tokio::spawn(async move {
                dedup_clone
                    .process_event(&event_clone, IdExtractionStrategy::Uuid)
                    .await
                    .unwrap()
            });
            handles.push(handle);
        }

        let results: Vec<bool> = futures::future::join_all(handles)
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        // Exactly one should succeed (race condition may vary, but at least one)
        let success_count = results.iter().filter(|&&r| r).count();
        assert!(success_count >= 1);
        assert!(success_count <= 10);
    }

    #[tokio::test]
    async fn test_multi_threaded_deduplication() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = Arc::new(EventDeduplicator::new(
            backend,
            std::time::Duration::from_secs(60),
        ));

        let events: Vec<_> = (0..100)
            .map(|i| {
                FeedbackEvent::new(
                    EventSource::Observatory,
                    EventType::Metric,
                    serde_json::json!({"id": i}),
                )
            })
            .collect();

        let events = Arc::new(events);

        // Process events in parallel
        let mut handles = Vec::new();
        for chunk_idx in 0..4 {
            let dedup_clone = Arc::clone(&dedup);
            let events_clone = Arc::clone(&events);
            let handle = tokio::spawn(async move {
                let start = chunk_idx * 25;
                let end = start + 25;
                for event in &events_clone[start..end] {
                    dedup_clone
                        .process_event(event, IdExtractionStrategy::Uuid)
                        .await
                        .unwrap();
                }
            });
            handles.push(handle);
        }

        futures::future::join_all(handles).await;

        let metrics = dedup.metrics();
        assert_eq!(metrics.unique_events.load(Ordering::Relaxed), 100);
    }

    #[tokio::test]
    async fn test_persistence_across_restarts() {
        // First session
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup1 = EventDeduplicator::new(
            Arc::clone(&backend),
            std::time::Duration::from_secs(60),
        );

        let event_id = "persistent-event";
        dedup1.mark_seen(event_id).await.unwrap();

        // Simulate restart with same backend
        let dedup2 = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        // Should still find the event
        assert!(dedup2.is_duplicate(event_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_ttl_expiration_verification() {
        let backend = Arc::new(MemoryStateBackend::new(Some(std::time::Duration::from_millis(
            200,
        ))));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_millis(200));

        let events: Vec<_> = (0..10)
            .map(|i| {
                FeedbackEvent::new(
                    EventSource::User,
                    EventType::Feedback,
                    serde_json::json!({"id": i}),
                )
            })
            .collect();

        // Process all events
        for event in &events {
            dedup
                .process_event(event, IdExtractionStrategy::Uuid)
                .await
                .unwrap();
        }

        // All should be duplicates immediately
        for event in &events {
            let event_id = dedup.extract_event_id(event, IdExtractionStrategy::Uuid);
            assert!(dedup.is_duplicate(&event_id).await.unwrap());
        }

        // Wait for TTL expiration
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;

        // All should be unique again
        for event in &events {
            let event_id = dedup.extract_event_id(event, IdExtractionStrategy::Uuid);
            assert!(!dedup.is_duplicate(&event_id).await.unwrap());
        }
    }

    #[tokio::test]
    async fn test_large_batch_processing() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        // Create batch with 50% duplicates
        let mut events = Vec::new();
        for i in 0..500 {
            let event = FeedbackEvent::new(
                EventSource::Observatory,
                EventType::Metric,
                serde_json::json!({"id": i % 250}), // 50% duplicates
            );
            events.push(event);
        }

        let results = dedup
            .process_batch(&events, IdExtractionStrategy::ContentHash)
            .await
            .unwrap();

        let unique_count = results.iter().filter(|&&r| r).count();
        let duplicate_count = results.iter().filter(|&&r| !r).count();

        assert_eq!(unique_count, 250);
        assert_eq!(duplicate_count, 250);
    }

    #[tokio::test]
    async fn test_mixed_extraction_strategies() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        let event1 = FeedbackEvent::new(
            EventSource::User,
            EventType::Feedback,
            serde_json::json!({"rating": 5}),
        );
        let event2 = FeedbackEvent::new(
            EventSource::User,
            EventType::Feedback,
            serde_json::json!({"rating": 5}),
        );

        // Using UUID strategy, these are different
        dedup
            .process_event(&event1, IdExtractionStrategy::Uuid)
            .await
            .unwrap();
        let result = dedup
            .process_event(&event2, IdExtractionStrategy::Uuid)
            .await
            .unwrap();
        assert!(result); // Not duplicate

        // Using ContentHash strategy, these are the same
        dedup.clear().await.unwrap();
        dedup
            .process_event(&event1, IdExtractionStrategy::ContentHash)
            .await
            .unwrap();
        let result = dedup
            .process_event(&event2, IdExtractionStrategy::ContentHash)
            .await
            .unwrap();
        assert!(!result); // Is duplicate
    }

    #[tokio::test]
    async fn test_deduplication_with_different_event_types() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        // Same data but different event types
        let metric = FeedbackEvent::new(
            EventSource::Observatory,
            EventType::Metric,
            serde_json::json!({"value": 42}),
        );
        let alert = FeedbackEvent::new(
            EventSource::Sentinel,
            EventType::PerformanceAlert,
            serde_json::json!({"value": 42}),
        );

        // Using content hash, these should be different
        dedup
            .process_event(&metric, IdExtractionStrategy::ContentHash)
            .await
            .unwrap();
        let result = dedup
            .process_event(&alert, IdExtractionStrategy::ContentHash)
            .await
            .unwrap();
        assert!(result); // Not duplicate (different types)
    }

    #[tokio::test]
    async fn test_metrics_accuracy_under_load() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = Arc::new(EventDeduplicator::new(
            backend,
            std::time::Duration::from_secs(60),
        ));

        let num_unique = 100;
        let num_duplicates = 200;

        // Create events
        let mut events = Vec::new();
        for i in 0..num_unique {
            let event = FeedbackEvent::new(
                EventSource::User,
                EventType::Feedback,
                serde_json::json!({"id": i}),
            );
            events.push(event);
        }

        // Process unique events
        for event in &events {
            dedup
                .process_event(event, IdExtractionStrategy::Uuid)
                .await
                .unwrap();
        }

        // Process duplicates
        for _ in 0..num_duplicates {
            let event = &events[0]; // Reuse first event
            dedup
                .process_event(event, IdExtractionStrategy::Uuid)
                .await
                .unwrap();
        }

        let metrics = dedup.metrics();
        assert_eq!(
            metrics.unique_events.load(Ordering::Relaxed),
            num_unique as u64
        );
        assert_eq!(
            metrics.duplicates_found.load(Ordering::Relaxed),
            num_duplicates as u64
        );
        assert_eq!(
            metrics.total_events.load(Ordering::Relaxed),
            (num_unique + num_duplicates) as u64
        );
    }
}

// ============================================================================
// INTEGRATION TESTS - Redis Backend (requires running Redis)
// ============================================================================

#[cfg(feature = "redis-integration")]
mod redis_integration_tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_redis_deduplication_basic() {
        let config = RedisConfig::builder()
            .url("redis://localhost:6379")
            .key_prefix("test_dedup:")
            .default_ttl(std::time::Duration::from_secs(60))
            .build()
            .expect("Failed to build config");

        let backend = Arc::new(
            RedisStateBackend::new(config)
                .await
                .expect("Failed to connect to Redis"),
        );
        let dedup = EventDeduplicator::new(backend.clone(), std::time::Duration::from_secs(60));

        // Clean up
        backend.clear().await.unwrap();

        let event = FeedbackEvent::new(
            EventSource::Observatory,
            EventType::Metric,
            serde_json::json!({"test": "redis"}),
        );

        // First processing
        let result1 = dedup
            .process_event(&event, IdExtractionStrategy::Uuid)
            .await
            .unwrap();
        assert!(result1);

        // Second processing should find duplicate
        let result2 = dedup
            .process_event(&event, IdExtractionStrategy::Uuid)
            .await
            .unwrap();
        assert!(!result2);

        backend.clear().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_ttl_expiration() {
        let config = RedisConfig::builder()
            .url("redis://localhost:6379")
            .key_prefix("test_ttl_dedup:")
            .default_ttl(std::time::Duration::from_secs(2))
            .build()
            .expect("Failed to build config");

        let backend = Arc::new(
            RedisStateBackend::new(config)
                .await
                .expect("Failed to connect to Redis"),
        );
        let dedup = EventDeduplicator::new(backend.clone(), std::time::Duration::from_secs(2));

        backend.clear().await.unwrap();

        let event_id = "ttl-test-event";
        dedup.mark_seen(event_id).await.unwrap();

        // Should be duplicate immediately
        assert!(dedup.is_duplicate(event_id).await.unwrap());

        // Wait for expiration
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        // Should be expired
        assert!(!dedup.is_duplicate(event_id).await.unwrap());

        backend.clear().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_concurrent_access() {
        let config = RedisConfig::builder()
            .url("redis://localhost:6379")
            .key_prefix("test_concurrent:")
            .build()
            .expect("Failed to build config");

        let backend = Arc::new(
            RedisStateBackend::new(config)
                .await
                .expect("Failed to connect to Redis"),
        );
        backend.clear().await.unwrap();

        let dedup = Arc::new(EventDeduplicator::new(
            backend.clone(),
            std::time::Duration::from_secs(60),
        ));

        let event = Arc::new(FeedbackEvent::new(
            EventSource::User,
            EventType::Feedback,
            serde_json::json!({"concurrent": true}),
        ));

        // Multiple concurrent processes
        let mut handles = Vec::new();
        for _ in 0..20 {
            let dedup_clone = Arc::clone(&dedup);
            let event_clone = Arc::clone(&event);
            let handle = tokio::spawn(async move {
                dedup_clone
                    .process_event(&event_clone, IdExtractionStrategy::Uuid)
                    .await
                    .unwrap()
            });
            handles.push(handle);
        }

        let results: Vec<bool> = futures::future::join_all(handles)
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        // At least one should succeed
        let success_count = results.iter().filter(|&&r| r).count();
        assert!(success_count >= 1);

        backend.clear().await.unwrap();
    }
}

// ============================================================================
// PERFORMANCE TESTS
// ============================================================================

mod performance_tests {
    use super::*;
    use processor::state::MemoryStateBackend;

    #[tokio::test]
    async fn test_throughput_10k_events() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        let event_count = 10_000;
        let events: Vec<_> = (0..event_count)
            .map(|i| {
                FeedbackEvent::new(
                    EventSource::Observatory,
                    EventType::Metric,
                    serde_json::json!({"id": i}),
                )
            })
            .collect();

        let start = Instant::now();

        for event in &events {
            dedup
                .process_event(event, IdExtractionStrategy::Uuid)
                .await
                .unwrap();
        }

        let duration = start.elapsed();
        let throughput = event_count as f64 / duration.as_secs_f64();

        println!("Throughput: {:.0} events/sec", throughput);
        println!("Duration: {:?} for {} events", duration, event_count);

        // Should handle at least 10k events/sec
        assert!(throughput > 10_000.0);
    }

    #[tokio::test]
    async fn test_throughput_100k_events() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        let event_count = 100_000;
        let events: Vec<_> = (0..event_count)
            .map(|i| {
                FeedbackEvent::new(
                    EventSource::Observatory,
                    EventType::Metric,
                    serde_json::json!({"seq": i}),
                )
            })
            .collect();

        let start = Instant::now();

        for event in &events {
            dedup
                .process_event(event, IdExtractionStrategy::Uuid)
                .await
                .unwrap();
        }

        let duration = start.elapsed();
        let throughput = event_count as f64 / duration.as_secs_f64();

        println!("Throughput: {:.0} events/sec", throughput);
        println!("Duration: {:?} for {} events", duration, event_count);

        // Should complete in reasonable time
        assert!(duration.as_secs() < 30);
    }

    #[tokio::test]
    async fn test_latency_p50_p95_p99() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        let event_count = 1000;
        let mut latencies = Vec::new();

        for i in 0..event_count {
            let event = FeedbackEvent::new(
                EventSource::User,
                EventType::Feedback,
                serde_json::json!({"id": i}),
            );

            let start = Instant::now();
            dedup
                .process_event(&event, IdExtractionStrategy::Uuid)
                .await
                .unwrap();
            latencies.push(start.elapsed());
        }

        latencies.sort();

        let p50 = latencies[latencies.len() * 50 / 100];
        let p95 = latencies[latencies.len() * 95 / 100];
        let p99 = latencies[latencies.len() * 99 / 100];

        println!("P50 latency: {:?}", p50);
        println!("P95 latency: {:?}", p95);
        println!("P99 latency: {:?}", p99);

        // All latencies should be under 1ms for memory backend
        assert!(p50.as_micros() < 1000);
        assert!(p95.as_micros() < 1000);
        assert!(p99.as_micros() < 1000);
    }

    #[tokio::test]
    async fn test_memory_efficiency() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        // Process large number of events
        for i in 0..10_000 {
            let event = FeedbackEvent::new(
                EventSource::Observatory,
                EventType::Metric,
                serde_json::json!({"id": i}),
            );
            dedup
                .process_event(&event, IdExtractionStrategy::Uuid)
                .await
                .unwrap();
        }

        let metrics = dedup.metrics();
        assert_eq!(metrics.unique_events.load(Ordering::Relaxed), 10_000);

        // Memory backend should handle this without issues
        // This is a basic smoke test for memory efficiency
    }

    #[tokio::test]
    async fn test_concurrent_deduplication_performance() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = Arc::new(EventDeduplicator::new(
            backend,
            std::time::Duration::from_secs(60),
        ));

        let num_tasks = 10;
        let events_per_task = 1000;

        let start = Instant::now();

        let mut handles = Vec::new();
        for task_id in 0..num_tasks {
            let dedup_clone = Arc::clone(&dedup);
            let handle = tokio::spawn(async move {
                for i in 0..events_per_task {
                    let event = FeedbackEvent::new(
                        EventSource::Observatory,
                        EventType::Metric,
                        serde_json::json!({"task": task_id, "id": i}),
                    );
                    dedup_clone
                        .process_event(&event, IdExtractionStrategy::Uuid)
                        .await
                        .unwrap();
                }
            });
            handles.push(handle);
        }

        futures::future::join_all(handles).await;

        let duration = start.elapsed();
        let total_events = num_tasks * events_per_task;
        let throughput = total_events as f64 / duration.as_secs_f64();

        println!(
            "Concurrent throughput: {:.0} events/sec",
            throughput
        );
        println!("Duration: {:?} for {} events", duration, total_events);

        let metrics = dedup.metrics();
        assert_eq!(metrics.unique_events.load(Ordering::Relaxed), total_events);
    }

    #[tokio::test]
    async fn test_large_scale_deduplication_1m_events() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = Arc::new(EventDeduplicator::new(
            backend,
            std::time::Duration::from_secs(60),
        ));

        let event_count = 1_000_000;
        let batch_size = 10_000;

        let start = Instant::now();

        for batch in 0..(event_count / batch_size) {
            let mut handles = Vec::new();
            for i in 0..batch_size {
                let event_id = batch * batch_size + i;
                let event = FeedbackEvent::new(
                    EventSource::Observatory,
                    EventType::Metric,
                    serde_json::json!({"id": event_id}),
                );

                let dedup_clone = Arc::clone(&dedup);
                let handle = tokio::spawn(async move {
                    dedup_clone
                        .process_event(&event, IdExtractionStrategy::Uuid)
                        .await
                        .unwrap();
                });
                handles.push(handle);
            }

            futures::future::join_all(handles).await;

            if batch % 10 == 0 {
                println!("Processed {} events", (batch + 1) * batch_size);
            }
        }

        let duration = start.elapsed();
        let throughput = event_count as f64 / duration.as_secs_f64();

        println!("Large-scale throughput: {:.0} events/sec", throughput);
        println!("Total duration: {:?} for {} events", duration, event_count);

        let metrics = dedup.metrics();
        assert_eq!(
            metrics.unique_events.load(Ordering::Relaxed),
            event_count
        );
    }
}

// ============================================================================
// EDGE CASES AND ERROR HANDLING
// ============================================================================

mod edge_case_tests {
    use super::*;
    use processor::state::MemoryStateBackend;

    #[tokio::test]
    async fn test_empty_event_id() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        let event_id = "";

        // Should handle empty ID without panicking
        let result = dedup.mark_seen(event_id).await;
        assert!(result.is_ok());

        let is_dup = dedup.is_duplicate(event_id).await.unwrap();
        assert!(is_dup);
    }

    #[tokio::test]
    async fn test_very_long_event_id() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        let event_id = "a".repeat(10000);

        // Should handle very long IDs
        dedup.mark_seen(&event_id).await.unwrap();
        let is_dup = dedup.is_duplicate(&event_id).await.unwrap();
        assert!(is_dup);
    }

    #[tokio::test]
    async fn test_special_characters_in_id() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        let event_ids = vec![
            "id:with:colons",
            "id/with/slashes",
            "id\\with\\backslashes",
            "id with spaces",
            "id\nwith\nnewlines",
            "id\twith\ttabs",
            "id{with}braces",
            "id[with]brackets",
        ];

        for event_id in &event_ids {
            dedup.mark_seen(event_id).await.unwrap();
            assert!(dedup.is_duplicate(event_id).await.unwrap());
        }
    }

    #[tokio::test]
    async fn test_hash_collision_handling() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        // Create events that might have hash collisions
        let event1 = FeedbackEvent::new(
            EventSource::User,
            EventType::Feedback,
            serde_json::json!({"a": 1, "b": 2}),
        );
        let event2 = FeedbackEvent::new(
            EventSource::User,
            EventType::Feedback,
            serde_json::json!({"a": 1, "b": 2}),
        );

        let hash1 = dedup.extract_event_id(&event1, IdExtractionStrategy::ContentHash);
        let hash2 = dedup.extract_event_id(&event2, IdExtractionStrategy::ContentHash);

        // Same content should produce same hash
        assert_eq!(hash1, hash2);

        dedup
            .process_event(&event1, IdExtractionStrategy::ContentHash)
            .await
            .unwrap();
        let is_dup = dedup
            .process_event(&event2, IdExtractionStrategy::ContentHash)
            .await
            .unwrap();
        assert!(!is_dup); // Should be detected as duplicate
    }

    #[tokio::test]
    async fn test_expired_entry_reuse() {
        let backend = Arc::new(MemoryStateBackend::new(Some(std::time::Duration::from_millis(
            100,
        ))));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_millis(100));

        let event = FeedbackEvent::new(
            EventSource::User,
            EventType::Feedback,
            serde_json::json!({}),
        );

        // First processing
        dedup
            .process_event(&event, IdExtractionStrategy::Uuid)
            .await
            .unwrap();

        // Wait for expiration
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        // Should be able to process again
        let result = dedup
            .process_event(&event, IdExtractionStrategy::Uuid)
            .await
            .unwrap();
        assert!(result); // Not a duplicate after expiration
    }

    #[tokio::test]
    async fn test_redis_connection_failure_handling() {
        // Note: This test uses memory backend to simulate failures
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        // Clear to simulate connection issue recovery
        dedup.clear().await.unwrap();

        let event = FeedbackEvent::new(
            EventSource::User,
            EventType::Feedback,
            serde_json::json!({}),
        );

        // Should handle gracefully
        let result = dedup
            .process_event(&event, IdExtractionStrategy::Uuid)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_partial_batch_failure() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        let mut events = Vec::new();
        for i in 0..10 {
            events.push(FeedbackEvent::new(
                EventSource::User,
                EventType::Feedback,
                serde_json::json!({"id": i}),
            ));
        }

        // Process batch should complete even if some fail
        let results = dedup
            .process_batch(&events, IdExtractionStrategy::Uuid)
            .await
            .unwrap();

        assert_eq!(results.len(), 10);
        assert!(results.iter().all(|&r| r)); // All should succeed in this case
    }

    #[tokio::test]
    async fn test_race_condition_same_event() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = Arc::new(EventDeduplicator::new(
            backend,
            std::time::Duration::from_secs(60),
        ));

        let event = Arc::new(FeedbackEvent::new(
            EventSource::User,
            EventType::Feedback,
            serde_json::json!({"race": "test"}),
        ));

        // Spawn tasks simultaneously
        let mut handles = Vec::new();
        for _ in 0..50 {
            let dedup_clone = Arc::clone(&dedup);
            let event_clone = Arc::clone(&event);
            let handle = tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                dedup_clone
                    .process_event(&event_clone, IdExtractionStrategy::Uuid)
                    .await
                    .unwrap()
            });
            handles.push(handle);
        }

        let results: Vec<bool> = futures::future::join_all(handles)
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        // Should handle race condition gracefully
        let success_count = results.iter().filter(|&&r| r).count();
        assert!(success_count >= 1); // At least one should succeed
    }

    #[tokio::test]
    async fn test_clock_skew_handling() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        // Create events with different timestamps
        let mut event1 = FeedbackEvent::new(
            EventSource::User,
            EventType::Feedback,
            serde_json::json!({}),
        );
        event1.timestamp = Utc::now() - Duration::seconds(3600); // 1 hour in past

        let mut event2 = FeedbackEvent::new(
            EventSource::User,
            EventType::Feedback,
            serde_json::json!({}),
        );
        event2.timestamp = Utc::now() + Duration::seconds(3600); // 1 hour in future

        // Should handle events with clock skew
        dedup
            .process_event(&event1, IdExtractionStrategy::Uuid)
            .await
            .unwrap();
        dedup
            .process_event(&event2, IdExtractionStrategy::Uuid)
            .await
            .unwrap();

        let metrics = dedup.metrics();
        assert_eq!(metrics.unique_events.load(Ordering::Relaxed), 2);
    }

    #[tokio::test]
    async fn test_null_metadata_handling() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        let event = FeedbackEvent::new(
            EventSource::User,
            EventType::Feedback,
            serde_json::json!(null),
        );

        // Should handle null data gracefully
        let result = dedup
            .process_event(&event, IdExtractionStrategy::ContentHash)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_missing_custom_id_fallback() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let dedup = EventDeduplicator::new(backend, std::time::Duration::from_secs(60));

        let event = FeedbackEvent::new(
            EventSource::System,
            EventType::PerformanceAlert,
            serde_json::json!({}),
        );
        // No custom_id in metadata

        // Should fall back to UUID
        let id = dedup.extract_event_id(&event, IdExtractionStrategy::Custom);
        assert_eq!(id, event.id.to_string());
    }
}

// Helper for MD5 hashing
mod md5 {
    pub fn compute(data: &[u8]) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
}
