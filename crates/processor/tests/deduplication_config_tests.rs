//! Integration tests for deduplication configuration
//!
//! These tests verify that the deduplication system is properly integrated
//! into the processor configuration and pipeline builder.

use processor::config::{DeduplicationConfig, DeduplicationStrategy, ProcessorConfig};
use processor::pipeline::{DeduplicationOperator, StreamPipelineBuilder};
use std::time::Duration;

#[test]
fn test_processor_config_with_deduplication() {
    let mut config = ProcessorConfig::default();

    // Deduplication should be disabled by default
    assert!(!config.deduplication.enabled);

    // Enable deduplication
    config.deduplication.enabled = true;
    config.deduplication.ttl = Duration::from_secs(3600);

    // Validate configuration
    assert!(config.validate().is_ok());
}

#[test]
fn test_deduplication_config_validation() {
    // Valid disabled config
    let config = DeduplicationConfig::default();
    assert!(config.validate().is_ok());

    // Valid enabled config
    let config = DeduplicationConfig::new()
        .enabled()
        .with_ttl(Duration::from_secs(3600));
    assert!(config.validate().is_ok());

    // Invalid: zero TTL
    let mut invalid_config = DeduplicationConfig::new().enabled();
    invalid_config.ttl = Duration::from_secs(0);
    assert!(invalid_config.validate().is_err());

    // Invalid: empty Redis key prefix
    let mut invalid_config = DeduplicationConfig::new().enabled();
    invalid_config.redis_key_prefix = String::new();
    assert!(invalid_config.validate().is_err());
}

#[test]
fn test_deduplication_config_builder_pattern() {
    let config = DeduplicationConfig::new()
        .enabled()
        .with_ttl(Duration::from_secs(7200))
        .with_strategy(DeduplicationStrategy::EventId)
        .with_redis_key_prefix("test:dedup")
        .with_max_retries(5)
        .with_cleanup_interval(Duration::from_secs(600));

    assert!(config.enabled);
    assert_eq!(config.ttl, Duration::from_secs(7200));
    assert_eq!(config.strategy, DeduplicationStrategy::EventId);
    assert_eq!(config.redis_key_prefix, "test:dedup");
    assert_eq!(config.max_retries, 5);
    assert_eq!(config.cleanup_interval, Duration::from_secs(600));
}

#[test]
fn test_deduplication_strategies() {
    let strategies = vec![
        DeduplicationStrategy::ContentHash,
        DeduplicationStrategy::EventId,
        DeduplicationStrategy::CompositeKey,
    ];

    for strategy in strategies {
        let config = DeduplicationConfig::new()
            .enabled()
            .with_strategy(strategy);

        assert_eq!(config.strategy, strategy);
        assert!(config.validate().is_ok());
    }
}

#[test]
fn test_pipeline_builder_with_deduplication() {
    let pipeline = StreamPipelineBuilder::new()
        .with_name("test-pipeline")
        .with_deduplication(true, Duration::from_secs(3600))
        .build()
        .unwrap();

    assert!(pipeline.config().processor.deduplication.enabled);
    assert_eq!(
        pipeline.config().processor.deduplication.ttl,
        Duration::from_secs(3600)
    );
}

#[test]
fn test_pipeline_builder_with_deduplication_config() {
    let dedup_config = DeduplicationConfig::new()
        .enabled()
        .with_ttl(Duration::from_secs(7200))
        .with_strategy(DeduplicationStrategy::CompositeKey)
        .with_redis_key_prefix("pipeline:dedup");

    let pipeline = StreamPipelineBuilder::new()
        .with_name("test-pipeline")
        .with_deduplication_config(dedup_config)
        .build()
        .unwrap();

    let config = &pipeline.config().processor.deduplication;
    assert!(config.enabled);
    assert_eq!(config.ttl, Duration::from_secs(7200));
    assert_eq!(config.strategy, DeduplicationStrategy::CompositeKey);
    assert_eq!(config.redis_key_prefix, "pipeline:dedup");
}

#[test]
fn test_pipeline_builder_deduplication_methods() {
    let pipeline = StreamPipelineBuilder::new()
        .with_name("test-pipeline")
        .with_deduplication(true, Duration::from_secs(1800))
        .with_deduplication_strategy(DeduplicationStrategy::EventId)
        .with_deduplication_redis_prefix("app:dedup")
        .with_deduplication_ttl(Duration::from_secs(3600))
        .build()
        .unwrap();

    let config = &pipeline.config().processor.deduplication;
    assert!(config.enabled);
    assert_eq!(config.ttl, Duration::from_secs(3600)); // Last set value wins
    assert_eq!(config.strategy, DeduplicationStrategy::EventId);
    assert_eq!(config.redis_key_prefix, "app:dedup");
}

#[test]
fn test_deduplication_operator_creation() {
    let config = DeduplicationConfig::new()
        .enabled()
        .with_ttl(Duration::from_secs(3600));

    let operator = DeduplicationOperator::new("test-dedup", config.clone());

    let stats = operator.stats();
    assert_eq!(stats.cached_signatures, 0);
    assert_eq!(stats.enabled, config.enabled);
}

#[tokio::test]
async fn test_deduplication_in_pipeline() {
    use processor::pipeline::operator::{OperatorContext, StreamOperator};
    use processor::watermark::Watermark;

    let config = DeduplicationConfig::new()
        .enabled()
        .with_ttl(Duration::from_secs(3600));

    let operator = DeduplicationOperator::new("test-dedup", config);
    let ctx = OperatorContext::new(Watermark::min(), 0);

    // Test that operator processes events correctly
    let event_data = b"test event".to_vec();

    // First event should pass through
    let result = operator.process(event_data.clone(), &ctx).await.unwrap();
    assert_eq!(result.len(), 1);

    // Duplicate event should be filtered
    let result = operator.process(event_data.clone(), &ctx).await.unwrap();
    assert_eq!(result.len(), 0);
}

#[test]
fn test_deduplication_config_serialization() {
    let config = DeduplicationConfig::new()
        .enabled()
        .with_ttl(Duration::from_secs(3600))
        .with_strategy(DeduplicationStrategy::ContentHash);

    // Test serialization to JSON
    let json = serde_json::to_string(&config).unwrap();
    assert!(json.contains("enabled"));
    assert!(json.contains("ttl"));
    assert!(json.contains("strategy"));

    // Test deserialization from JSON
    let deserialized: DeduplicationConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.enabled, config.enabled);
    assert_eq!(deserialized.ttl, config.ttl);
    assert_eq!(deserialized.strategy, config.strategy);
}

#[test]
fn test_processor_config_serialization_with_deduplication() {
    let mut config = ProcessorConfig::default();
    config.deduplication.enabled = true;
    config.deduplication.ttl = Duration::from_secs(7200);

    // Test serialization
    let json = serde_json::to_string(&config).unwrap();

    // Test deserialization
    let deserialized: ProcessorConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.deduplication.enabled, config.deduplication.enabled);
    assert_eq!(deserialized.deduplication.ttl, config.deduplication.ttl);
}

#[test]
fn test_deduplication_config_ttl_conversion() {
    let config = DeduplicationConfig::new()
        .with_ttl(Duration::from_secs(60));

    assert_eq!(config.ttl_ms(), 60_000);

    let config = DeduplicationConfig::new()
        .with_ttl(Duration::from_millis(5000));

    assert_eq!(config.ttl_ms(), 5000);
}

#[test]
fn test_deduplication_config_cleanup_interval_conversion() {
    let config = DeduplicationConfig::new()
        .with_cleanup_interval(Duration::from_secs(300));

    assert_eq!(config.cleanup_interval_ms(), 300_000);
}

#[test]
fn test_multiple_pipelines_with_different_dedup_configs() {
    // Pipeline 1: Deduplication enabled with ContentHash
    let pipeline1 = StreamPipelineBuilder::new()
        .with_name("pipeline-1")
        .with_deduplication(true, Duration::from_secs(3600))
        .with_deduplication_strategy(DeduplicationStrategy::ContentHash)
        .build()
        .unwrap();

    // Pipeline 2: Deduplication enabled with EventId
    let pipeline2 = StreamPipelineBuilder::new()
        .with_name("pipeline-2")
        .with_deduplication(true, Duration::from_secs(7200))
        .with_deduplication_strategy(DeduplicationStrategy::EventId)
        .build()
        .unwrap();

    // Pipeline 3: Deduplication disabled
    let pipeline3 = StreamPipelineBuilder::new()
        .with_name("pipeline-3")
        .build()
        .unwrap();

    // Verify each pipeline has its own configuration
    assert!(pipeline1.config().processor.deduplication.enabled);
    assert_eq!(
        pipeline1.config().processor.deduplication.strategy,
        DeduplicationStrategy::ContentHash
    );

    assert!(pipeline2.config().processor.deduplication.enabled);
    assert_eq!(
        pipeline2.config().processor.deduplication.strategy,
        DeduplicationStrategy::EventId
    );

    assert!(!pipeline3.config().processor.deduplication.enabled);
}

#[test]
fn test_deduplication_backward_compatibility() {
    // Test that existing configurations without deduplication still work
    let config = ProcessorConfig::default();

    // Should have deduplication config with sensible defaults
    assert!(!config.deduplication.enabled); // Disabled by default
    assert_eq!(config.deduplication.ttl, Duration::from_secs(3600));
    assert_eq!(config.deduplication.strategy, DeduplicationStrategy::ContentHash);

    // Validation should pass
    assert!(config.validate().is_ok());
}
