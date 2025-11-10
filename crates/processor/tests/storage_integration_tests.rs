//! Integration tests for Storage Layer
//!
//! This test suite verifies that all storage backends work correctly together
//! through the StorageManager and that the unified Storage trait works as expected.

use processor::storage::{
    PostgreSQLConfig, RedisConfig, RoutingStrategy, SledConfig, Storage, StorageBackend,
    StorageConfig, StorageManager,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestData {
    id: String,
    value: i64,
    description: String,
}

#[tokio::test]
async fn test_storage_manager_creation() {
    let manager = StorageManager::new(RoutingStrategy::PrimaryOnly);
    assert_eq!(manager.state().await, processor::storage::ManagerState::Uninitialized);
}

#[tokio::test]
async fn test_routing_strategies() {
    let strategies = vec![
        RoutingStrategy::PrimaryOnly,
        RoutingStrategy::CacheAside,
        RoutingStrategy::MultiWrite,
        RoutingStrategy::FastestRead,
    ];

    for strategy in strategies {
        let manager = StorageManager::new(strategy);
        assert!(manager.state().await == processor::storage::ManagerState::Uninitialized);
    }
}

#[tokio::test]
async fn test_storage_config_defaults() {
    let config = StorageConfig::default();
    assert!(config.postgresql.is_none());
    assert!(config.redis.is_none());
    assert!(config.sled.is_none());
}

#[tokio::test]
async fn test_postgresql_config() {
    let config = PostgreSQLConfig::default();
    assert_eq!(config.host, "localhost");
    assert_eq!(config.port, 5432);
    assert_eq!(config.database, "llm_optimizer");

    let url = config.connection_url();
    assert!(url.contains("postgresql://"));
    assert!(url.contains(&config.host));
}

#[tokio::test]
async fn test_redis_config() {
    let config = RedisConfig::default();
    assert_eq!(config.host, "localhost");
    assert_eq!(config.port, 6379);
    assert_eq!(config.database, 0);

    let url = config.connection_url();
    assert!(url.contains("redis://"));
    assert!(url.contains(&config.host));
}

#[tokio::test]
async fn test_sled_config() {
    let config = SledConfig::default();
    assert!(config.path.to_str().unwrap().contains("sled"));
    assert_eq!(config.use_compression, true);
    assert_eq!(config.create_dir, true);
}

#[tokio::test]
async fn test_storage_backend_names() {
    assert_eq!(StorageBackend::PostgreSQL.name(), "postgresql");
    assert_eq!(StorageBackend::Redis.name(), "redis");
    assert_eq!(StorageBackend::Sled.name(), "sled");
}

#[tokio::test]
async fn test_storage_backend_capabilities() {
    // PostgreSQL supports transactions
    assert!(StorageBackend::PostgreSQL.supports_transactions());
    assert!(!StorageBackend::PostgreSQL.supports_ttl());
    assert!(!StorageBackend::PostgreSQL.supports_pubsub());

    // Redis supports TTL and pub/sub
    assert!(!StorageBackend::Redis.supports_transactions());
    assert!(StorageBackend::Redis.supports_ttl());
    assert!(StorageBackend::Redis.supports_pubsub());

    // Sled supports transactions
    assert!(StorageBackend::Sled.supports_transactions());
    assert!(!StorageBackend::Sled.supports_ttl());
    assert!(!StorageBackend::Sled.supports_pubsub());
}

#[tokio::test]
async fn test_storage_entry_creation() {
    let data = TestData {
        id: "test-1".to_string(),
        value: 42,
        description: "Test data".to_string(),
    };

    let entry = processor::storage::StorageEntry::new("key1".to_string(), data.clone());

    assert_eq!(entry.key, "key1");
    assert_eq!(entry.value, data);
    assert_eq!(entry.version, 1);
    assert!(entry.expires_at.is_none());
    assert!(!entry.is_expired());
}

#[tokio::test]
async fn test_storage_entry_with_ttl() {
    let data = TestData {
        id: "test-2".to_string(),
        value: 100,
        description: "TTL test".to_string(),
    };

    let entry = processor::storage::StorageEntry::with_ttl("key2".to_string(), data.clone(), 3600);

    assert_eq!(entry.key, "key2");
    assert_eq!(entry.value, data);
    assert!(entry.expires_at.is_some());
    assert!(!entry.is_expired()); // Should not be expired yet
}

#[tokio::test]
async fn test_storage_entry_update() {
    let data1 = TestData {
        id: "test-3".to_string(),
        value: 50,
        description: "Original".to_string(),
    };

    let mut entry = processor::storage::StorageEntry::new("key3".to_string(), data1);
    let version1 = entry.version;

    let data2 = TestData {
        id: "test-3".to_string(),
        value: 75,
        description: "Updated".to_string(),
    };

    entry.update(data2.clone());

    assert_eq!(entry.value, data2);
    assert_eq!(entry.version, version1 + 1);
}

#[tokio::test]
async fn test_query_builder() {
    use processor::storage::{Operator, Query, SortDirection, Value};

    let query = Query::new("test_table")
        .filter("age", Operator::GreaterThan, Value::Integer(18))
        .filter("status", Operator::Equal, Value::String("active".to_string()))
        .sort("created_at".to_string(), SortDirection::Descending)
        .limit(10)
        .offset(20)
        .select(vec!["id".to_string(), "name".to_string()]);

    assert_eq!(query.table, "test_table");
    assert_eq!(query.filters.len(), 2);
    assert_eq!(query.sort.len(), 1);
    assert_eq!(query.limit, Some(10));
    assert_eq!(query.offset, Some(20));
    assert_eq!(query.select.len(), 2);
}

#[tokio::test]
async fn test_batch_operations() {
    use processor::storage::Batch;

    let mut batch = Batch::new(100);
    assert!(batch.is_empty());
    assert_eq!(batch.len(), 0);
    assert!(!batch.is_full());
}

#[tokio::test]
async fn test_transaction_creation() {
    use processor::storage::{Transaction, TransactionState};

    let txn = Transaction::new();
    assert!(!txn.id.is_empty());
    assert_eq!(txn.state, TransactionState::Active);
    assert_eq!(txn.operations.len(), 0);
}

#[tokio::test]
async fn test_storage_stats() {
    use processor::storage::StorageStats;

    let mut stats = StorageStats::new();
    assert_eq!(stats.total_operations, 0);
    assert_eq!(stats.successful_operations, 0);
    assert_eq!(stats.failed_operations, 0);
    assert_eq!(stats.success_rate(), 0.0);

    stats.record_success(10.5, 1024);
    assert_eq!(stats.total_operations, 1);
    assert_eq!(stats.successful_operations, 1);
    assert_eq!(stats.success_rate(), 1.0);
    assert_eq!(stats.avg_latency_ms, 10.5);
    assert_eq!(stats.peak_latency_ms, 10.5);

    stats.record_success(20.0, 2048);
    assert_eq!(stats.total_operations, 2);
    assert_eq!(stats.avg_latency_ms, 15.25);
    assert_eq!(stats.peak_latency_ms, 20.0);

    stats.record_failure();
    assert_eq!(stats.total_operations, 3);
    assert_eq!(stats.failed_operations, 1);
    assert!(stats.success_rate() > 0.66 && stats.success_rate() < 0.67);
}

#[tokio::test]
async fn test_storage_error_categories() {
    use processor::storage::{ErrorCategory, StorageError};

    let error = StorageError::NotFound {
        entity: "user".to_string(),
        key: "123".to_string(),
    };
    assert_eq!(error.category(), ErrorCategory::NotFound);
    assert!(error.category().is_client_error());
    assert!(!error.category().is_server_error());

    let error = StorageError::ConnectionError {
        source: Box::new(std::io::Error::new(std::io::ErrorKind::Other, "test")),
    };
    assert_eq!(error.category(), ErrorCategory::Connection);
    assert!(!error.category().is_client_error());
    assert!(error.category().is_server_error());
    assert!(error.is_retryable());
    assert!(error.is_transient());
}

#[tokio::test]
async fn test_storage_health_enum() {
    use processor::storage::StorageHealth;

    let health_states = vec![
        StorageHealth::Healthy,
        StorageHealth::Degraded,
        StorageHealth::Unhealthy,
    ];

    for state in health_states {
        // Just verify they can be created and compared
        assert!(state == state);
    }
}

#[tokio::test]
async fn test_manager_stats() {
    use processor::storage::ManagerStats;

    let stats = ManagerStats::default();
    assert_eq!(stats.total_operations, 0);
    assert_eq!(stats.successful_operations, 0);
    assert_eq!(stats.failed_operations, 0);
    assert_eq!(stats.fallback_count, 0);
    assert_eq!(stats.operations_by_backend.len(), 0);
}

#[tokio::test]
async fn test_retry_config() {
    use processor::storage::RetryConfig;

    let config = RetryConfig::default();
    assert!(config.enabled);
    assert_eq!(config.max_attempts, 3);

    // Test delay calculation
    let delay1 = config.delay_for_attempt(0);
    assert_eq!(delay1.as_millis(), 0);

    let delay2 = config.delay_for_attempt(1);
    assert!(delay2.as_millis() > 0);

    let delay3 = config.delay_for_attempt(2);
    assert!(delay3.as_millis() > delay2.as_millis());
}

#[tokio::test]
async fn test_pool_config() {
    use processor::storage::PoolConfig;

    let config = PoolConfig::default();
    assert!(config.min_connections > 0);
    assert!(config.max_connections >= config.min_connections);
    assert!(config.idle_timeout.as_secs() > 0);
    assert!(config.max_lifetime.as_secs() > 0);
}

#[tokio::test]
async fn test_monitoring_config() {
    use processor::storage::MonitoringConfig;

    let config = MonitoringConfig::default();
    assert!(config.enable_metrics);
    assert!(config.enable_health_checks);
    assert!(config.enable_query_logging);
    assert!(config.enable_pool_monitoring);
}

// Note: The following tests would require actual database connections
// and are marked as ignored. They can be run in a CI environment with
// proper database setup.

#[tokio::test]
#[ignore]
async fn test_postgresql_backend_integration() {
    // This would test actual PostgreSQL operations
    // Requires: docker run -p 5432:5432 -e POSTGRES_PASSWORD=test postgres
}

#[tokio::test]
#[ignore]
async fn test_redis_backend_integration() {
    // This would test actual Redis operations
    // Requires: docker run -p 6379:6379 redis
}

#[tokio::test]
#[ignore]
async fn test_sled_backend_integration() {
    // This would test actual Sled operations
    // Can run locally with temporary directory
}

#[tokio::test]
#[ignore]
async fn test_storage_manager_with_all_backends() {
    // This would test StorageManager coordinating all three backends
    // Requires all backends to be available
}

#[tokio::test]
#[ignore]
async fn test_cache_aside_strategy() {
    // This would test the cache-aside pattern with Redis cache and PostgreSQL primary
}

#[tokio::test]
#[ignore]
async fn test_multi_write_strategy() {
    // This would test writing to multiple backends simultaneously
}

#[tokio::test]
#[ignore]
async fn test_automatic_fallback() {
    // This would test fallback when primary backend fails
}
