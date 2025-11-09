//! Kafka Stream Processing Pipeline Example
//!
//! This example demonstrates a complete Kafka-to-Kafka stream processing pipeline
//! that consumes events, applies windowing and aggregation, and produces results.
//!
//! # Setup
//!
//! 1. Start Kafka:
//!    ```bash
//!    docker-compose up -d kafka
//!    ```
//!
//! 2. Create topics:
//!    ```bash
//!    kafka-topics --create --topic sensor-events --bootstrap-server localhost:9092
//!    kafka-topics --create --topic sensor-aggregates --bootstrap-server localhost:9092
//!    ```
//!
//! 3. Run the example:
//!    ```bash
//!    cargo run --example kafka_pipeline
//!    ```
//!
//! 4. Produce test events:
//!    ```bash
//!    kafka-console-producer --topic sensor-events --bootstrap-server localhost:9092
//!    # Paste JSON events (see below for format)
//!    ```
//!
//! 5. Consume results:
//!    ```bash
//!    kafka-console-consumer --topic sensor-aggregates --bootstrap-server localhost:9092 --from-beginning
//!    ```

use chrono::{DateTime, Utc};
use processor::core::{EventKey, EventTimeExtractor, KeyExtractor, ProcessorEvent};
use processor::kafka::{KafkaStreamConfig, KafkaStreamPipeline};
use processor::pipeline::StreamPipelineBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio::signal;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

/// Sensor event from IoT devices
///
/// Example JSON:
/// ```json
/// {
///   "sensor_id": "sensor-001",
///   "location": "factory-floor",
///   "temperature": 72.5,
///   "humidity": 45.2,
///   "timestamp": 1699564800000
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SensorEvent {
    /// Sensor identifier
    sensor_id: String,

    /// Location of the sensor
    location: String,

    /// Temperature reading in Fahrenheit
    temperature: f64,

    /// Humidity percentage
    humidity: f64,

    /// Event timestamp (Unix milliseconds)
    timestamp: i64,
}

impl EventTimeExtractor<SensorEvent> for SensorEvent {
    fn extract_event_time(&self, event: &SensorEvent) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(event.timestamp).unwrap_or_else(Utc::now)
    }
}

impl KeyExtractor<SensorEvent> for SensorEvent {
    fn extract_key(&self, event: &SensorEvent) -> EventKey {
        // Group by location for location-based aggregates
        EventKey::composite(vec![
            ("location", &event.location),
            ("sensor", &event.sensor_id),
        ])
    }
}

/// Aggregated sensor statistics
///
/// This is the output of windowed aggregation, containing statistics
/// for all sensors in a location within a time window.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SensorAggregate {
    /// Location
    location: String,

    /// Sensor ID
    sensor_id: String,

    /// Window start time
    window_start: DateTime<Utc>,

    /// Window end time
    window_end: DateTime<Utc>,

    /// Number of events in window
    event_count: u64,

    /// Average temperature
    avg_temperature: f64,

    /// Min temperature
    min_temperature: f64,

    /// Max temperature
    max_temperature: f64,

    /// Average humidity
    avg_humidity: f64,

    /// Min humidity
    min_humidity: f64,

    /// Max humidity
    max_humidity: f64,

    /// Processing timestamp
    processed_at: DateTime<Utc>,
}

/// Simple aggregator that computes statistics from sensor events
struct SensorAggregator {
    location: String,
    sensor_id: String,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
    events: Vec<SensorEvent>,
}

impl SensorAggregator {
    fn new(location: String, sensor_id: String, window_start: DateTime<Utc>, window_end: DateTime<Utc>) -> Self {
        Self {
            location,
            sensor_id,
            window_start,
            window_end,
            events: Vec::new(),
        }
    }

    fn add_event(&mut self, event: SensorEvent) {
        self.events.push(event);
    }

    fn compute(&self) -> SensorAggregate {
        let event_count = self.events.len() as u64;

        let (avg_temp, min_temp, max_temp, avg_humid, min_humid, max_humid) = if !self.events.is_empty() {
            let temps: Vec<f64> = self.events.iter().map(|e| e.temperature).collect();
            let humids: Vec<f64> = self.events.iter().map(|e| e.humidity).collect();

            let avg_temp = temps.iter().sum::<f64>() / temps.len() as f64;
            let min_temp = temps.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            let max_temp = temps.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));

            let avg_humid = humids.iter().sum::<f64>() / humids.len() as f64;
            let min_humid = humids.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            let max_humid = humids.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));

            (avg_temp, min_temp, max_temp, avg_humid, min_humid, max_humid)
        } else {
            (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
        };

        SensorAggregate {
            location: self.location.clone(),
            sensor_id: self.sensor_id.clone(),
            window_start: self.window_start,
            window_end: self.window_end,
            event_count,
            avg_temperature: avg_temp,
            min_temperature: min_temp,
            max_temperature: max_temp,
            avg_humidity: avg_humid,
            min_humidity: min_humid,
            max_humidity: max_humid,
            processed_at: Utc::now(),
        }
    }
}

/// Event processor that aggregates sensor events and emits results
struct EventProcessor {
    aggregators: HashMap<String, SensorAggregator>,
    result_tx: mpsc::Sender<ProcessorEvent<SensorAggregate>>,
}

impl EventProcessor {
    fn new(result_tx: mpsc::Sender<ProcessorEvent<SensorAggregate>>) -> Self {
        Self {
            aggregators: HashMap::new(),
            result_tx,
        }
    }

    async fn process_event(&mut self, event: ProcessorEvent<SensorEvent>) {
        let key = event.key.clone();
        let location = event.event.location.clone();
        let sensor_id = event.event.sensor_id.clone();

        // Get or create aggregator
        let aggregator = self.aggregators.entry(key.clone()).or_insert_with(|| {
            SensorAggregator::new(
                location.clone(),
                sensor_id.clone(),
                event.event_time,
                event.event_time,
            )
        });

        // Update window bounds
        if event.event_time < aggregator.window_start {
            aggregator.window_start = event.event_time;
        }
        if event.event_time > aggregator.window_end {
            aggregator.window_end = event.event_time;
        }

        // Add event
        aggregator.add_event(event.event);
    }

    async fn flush_window(&mut self, window_key: &str) {
        if let Some(aggregator) = self.aggregators.remove(window_key) {
            let aggregate = aggregator.compute();
            let processor_event = ProcessorEvent::new(
                aggregate,
                aggregator.window_start,
                window_key.to_string(),
            );

            if let Err(e) = self.result_tx.send(processor_event).await {
                error!(error = %e, "Failed to send aggregate result");
            }
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging - set RUST_LOG environment variable to control verbosity
    // Example: RUST_LOG=info cargo run --example kafka_pipeline

    info!("Starting Kafka stream processing pipeline example");

    // Configuration
    let kafka_config = KafkaStreamConfig {
        brokers: std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string()),
        group_id: "sensor-processor".to_string(),
        input_topics: vec!["sensor-events".to_string()],
        output_topic: "sensor-aggregates".to_string(),
        auto_commit_interval_ms: 5000,
        poll_timeout_ms: 100,
        delivery_timeout_ms: 5000,
        max_produce_retries: 3,
        consumer_config: {
            let mut config = HashMap::new();
            config.insert("session.timeout.ms".to_string(), "30000".to_string());
            config.insert("heartbeat.interval.ms".to_string(), "10000".to_string());
            config
        },
        producer_config: {
            let mut config = HashMap::new();
            config.insert("compression.type".to_string(), "lz4".to_string());
            config.insert("acks".to_string(), "all".to_string());
            config
        },
    };

    // Create stream processing pipeline
    let pipeline = StreamPipelineBuilder::new()
        .with_name("sensor-aggregator")
        .with_tumbling_window(60_000) // 1-minute windows
        .with_watermark_delay(Duration::from_millis(5_000))   // 5-second delay
        .with_buffer_size(10_000)
        .build()?;

    info!(
        pipeline = %pipeline.name(),
        window_size = "60s",
        "Created stream pipeline"
    );

    // Create executor
    let executor = pipeline.create_executor::<SensorEvent>();

    // Create result channel for aggregates
    let (result_tx, result_rx) = mpsc::channel(1000);

    // Create Kafka pipeline
    let kafka_pipeline = KafkaStreamPipeline::new(kafka_config, executor)?;

    info!("Kafka pipeline initialized");

    // Spawn background task to handle aggregation
    let processor_handle = {
        let executor_clone = kafka_pipeline.executor.clone();
        tokio::spawn(async move {
            let mut processor = EventProcessor::new(result_tx);
            let mut window_timer = tokio::time::interval(tokio::time::Duration::from_secs(60));

            loop {
                tokio::select! {
                    // Process events from executor
                    Some(event) = executor_clone.next_output() => {
                        processor.process_event(event).await;
                    }

                    // Flush windows periodically
                    _ = window_timer.tick() => {
                        info!("Flushing aggregation windows");
                        let keys: Vec<String> = processor.aggregators.keys().cloned().collect();
                        for key in keys {
                            processor.flush_window(&key).await;
                        }
                    }
                }
            }
        })
    };

    // Clone stats for reporter before moving pipeline
    let stats_for_reporter = kafka_pipeline.stats.clone();

    // Run pipeline in background
    let pipeline_handle = {
        let mut pipeline = kafka_pipeline;
        tokio::spawn(async move {
            if let Err(e) = pipeline.run(result_rx).await {
                error!(error = %e, "Pipeline error");
            }

            // Graceful shutdown
            info!("Shutting down pipeline");
            if let Err(e) = pipeline.shutdown().await {
                error!(error = %e, "Shutdown error");
            }
        })
    };

    // Spawn stats reporter
    let stats_handle = {
        let stats = stats_for_reporter;
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(10));

            loop {
                ticker.tick().await;

                let stats = stats.read().await;
                info!(
                    consumed = stats.messages_consumed,
                    produced = stats.messages_produced,
                    consume_errors = stats.consume_errors,
                    produce_errors = stats.produce_errors,
                    offsets_committed = stats.offsets_committed,
                    "Pipeline statistics"
                );

                if stats.consume_errors > 0 || stats.produce_errors > 0 {
                    warn!("Errors detected in pipeline");
                }
            }
        })
    };

    info!("Pipeline running. Press Ctrl+C to stop.");

    // Wait for shutdown signal
    signal::ctrl_c().await?;

    info!("Received shutdown signal, stopping pipeline");

    // Cancel background tasks
    processor_handle.abort();
    stats_handle.abort();

    // Wait for pipeline to stop
    let _ = pipeline_handle.await;

    info!("Pipeline stopped successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sensor_event_serialization() {
        let event = SensorEvent {
            sensor_id: "sensor-001".to_string(),
            location: "factory-floor".to_string(),
            temperature: 72.5,
            humidity: 45.2,
            timestamp: 1699564800000,
        };

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: SensorEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.sensor_id, event.sensor_id);
        assert_eq!(deserialized.temperature, event.temperature);
    }

    #[test]
    fn test_sensor_event_key_extraction() {
        let event = SensorEvent {
            sensor_id: "sensor-001".to_string(),
            location: "factory-floor".to_string(),
            temperature: 72.5,
            humidity: 45.2,
            timestamp: 1699564800000,
        };

        let key = event.extract_key(&event);
        match key {
            EventKey::Composite(parts) => {
                assert_eq!(parts.len(), 2);
                assert!(parts.contains(&("location".to_string(), "factory-floor".to_string())));
                assert!(parts.contains(&("sensor".to_string(), "sensor-001".to_string())));
            }
            _ => panic!("Expected composite key"),
        }
    }

    #[test]
    fn test_sensor_event_time_extraction() {
        let event = SensorEvent {
            sensor_id: "sensor-001".to_string(),
            location: "factory-floor".to_string(),
            temperature: 72.5,
            humidity: 45.2,
            timestamp: 1699564800000,
        };

        let event_time = event.extract_event_time(&event);
        assert_eq!(event_time.timestamp_millis(), 1699564800000);
    }

    #[test]
    fn test_sensor_aggregator() {
        let location = "factory-floor".to_string();
        let sensor_id = "sensor-001".to_string();
        let start = Utc::now();
        let end = start + chrono::Duration::minutes(1);

        let mut aggregator = SensorAggregator::new(location, sensor_id, start, end);

        // Add events
        aggregator.add_event(SensorEvent {
            sensor_id: "sensor-001".to_string(),
            location: "factory-floor".to_string(),
            temperature: 70.0,
            humidity: 40.0,
            timestamp: start.timestamp_millis(),
        });

        aggregator.add_event(SensorEvent {
            sensor_id: "sensor-001".to_string(),
            location: "factory-floor".to_string(),
            temperature: 75.0,
            humidity: 50.0,
            timestamp: start.timestamp_millis() + 30000,
        });

        let aggregate = aggregator.compute();

        assert_eq!(aggregate.event_count, 2);
        assert_eq!(aggregate.avg_temperature, 72.5);
        assert_eq!(aggregate.min_temperature, 70.0);
        assert_eq!(aggregate.max_temperature, 75.0);
        assert_eq!(aggregate.avg_humidity, 45.0);
        assert_eq!(aggregate.min_humidity, 40.0);
        assert_eq!(aggregate.max_humidity, 50.0);
    }
}
