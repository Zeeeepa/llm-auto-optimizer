//! Common test utilities for integration tests

use collector::{FeedbackCollectorConfig, KafkaProducerConfig, TelemetryConfig, BackpressureConfig};
use std::time::Duration;
use testcontainers::{clients::Cli, Container, RunnableImage};
use testcontainers_modules::kafka::Kafka;

/// Test container wrapper for Kafka
pub struct KafkaTestContainer<'a> {
    pub container: Container<'a, Kafka>,
    pub bootstrap_servers: String,
}

impl<'a> KafkaTestContainer<'a> {
    /// Start a new Kafka test container
    pub fn start(docker: &'a Cli) -> Self {
        let kafka_image = RunnableImage::from(Kafka::default());
        let container = docker.run(kafka_image);
        let bootstrap_servers = format!("localhost:{}", container.get_host_port_ipv4(9093));

        // Wait for Kafka to be ready
        std::thread::sleep(Duration::from_secs(5));

        Self {
            container,
            bootstrap_servers,
        }
    }

    /// Get bootstrap servers for Kafka
    pub fn bootstrap_servers(&self) -> String {
        self.bootstrap_servers.clone()
    }
}

/// Create a test Kafka producer config
pub fn test_kafka_config(bootstrap_servers: &str, topic: &str) -> KafkaProducerConfig {
    KafkaProducerConfig {
        brokers: bootstrap_servers.to_string(),
        topic: topic.to_string(),
        client_id: "test-collector".to_string(),
        message_timeout_ms: 10000,
        compression_type: "none".to_string(),
        retries: 3,
        retry_backoff_ms: 500,
        circuit_breaker_config: None,
    }
}

/// Create a test collector config
pub fn test_collector_config(kafka_config: KafkaProducerConfig) -> FeedbackCollectorConfig {
    FeedbackCollectorConfig {
        buffer_size: 1000,
        max_batch_size: 10,
        max_batch_age_secs: 5,
        flush_interval_ms: 1000,
        num_workers: 1,
        kafka_config,
        telemetry_config: TelemetryConfig {
            service_name: "test-collector".to_string(),
            service_version: "0.1.0".to_string(),
            export_interval_secs: 60,
            otlp_endpoint: None, // Disabled for tests
        },
        rate_limit_config: None,
        backpressure_config: BackpressureConfig::default(),
        enable_recovery: false,
        shutdown_timeout_secs: 30,
    }
}

/// Wait for a condition with timeout
pub async fn wait_for_condition<F>(mut check: F, timeout: Duration, check_interval: Duration) -> bool
where
    F: FnMut() -> bool,
{
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if check() {
            return true;
        }
        tokio::time::sleep(check_interval).await;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kafka_config_creation() {
        let config = test_kafka_config("localhost:9092", "test-topic");
        assert_eq!(config.brokers, "localhost:9092");
        assert_eq!(config.topic, "test-topic");
        assert_eq!(config.retries, 3);
    }

    #[test]
    fn test_collector_config_creation() {
        let kafka_config = test_kafka_config("localhost:9092", "test-topic");
        let collector_config = test_collector_config(kafka_config);
        assert_eq!(collector_config.buffer_size, 1000);
        assert_eq!(collector_config.max_batch_size, 10);
        assert_eq!(collector_config.num_workers, 1);
    }

    #[tokio::test]
    async fn test_wait_for_condition_success() {
        let mut counter = 0;
        let result = wait_for_condition(
            || {
                counter += 1;
                counter >= 3
            },
            Duration::from_secs(5),
            Duration::from_millis(100),
        )
        .await;
        assert!(result);
        assert!(counter >= 3);
    }

    #[tokio::test]
    async fn test_wait_for_condition_timeout() {
        let result = wait_for_condition(
            || false,
            Duration::from_millis(200),
            Duration::from_millis(50),
        )
        .await;
        assert!(!result);
    }
}
