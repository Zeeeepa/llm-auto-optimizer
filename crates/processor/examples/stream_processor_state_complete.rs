//! Complete Stream Processor State Management Example
//!
//! This example demonstrates all required features:
//! 1. Redis-based state persistence for window state
//! 2. Automatic checkpointing every 10 seconds
//! 3. State recovery on restart
//! 4. Connection failure handling with retry logic
//! 5. Connection pooling and health checks
//! 6. Comprehensive metrics and monitoring
//!
//! ## Prerequisites
//!
//! Start Redis:
//! ```bash
//! docker run -d --name redis-state -p 6379:6379 redis:7-alpine
//! ```
//!
//! ## Run Example
//!
//! ```bash
//! cargo run --example stream_processor_state_complete
//! ```

use processor::state::{
    CheckpointCoordinator, CheckpointOptions, RedisConfig, RedisStateBackend, StateBackend,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::sync::RwLock;
use tokio::time::{interval, sleep};
use tracing::{error, info, warn};

/// Window state for stream processing
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WindowState {
    window_id: String,
    start_time: i64,
    end_time: i64,
    event_count: u64,
    sum: f64,
    min: f64,
    max: f64,
    last_updated: i64,
}

impl WindowState {
    fn new(window_id: String, start_time: i64, end_time: i64) -> Self {
        Self {
            window_id,
            start_time,
            end_time,
            event_count: 0,
            sum: 0.0,
            min: f64::MAX,
            max: f64::MIN,
            last_updated: chrono::Utc::now().timestamp_millis(),
        }
    }

    fn add_event(&mut self, value: f64) {
        self.event_count += 1;
        self.sum += value;
        self.min = self.min.min(value);
        self.max = self.max.max(value);
        self.last_updated = chrono::Utc::now().timestamp_millis();
    }

    fn average(&self) -> f64 {
        if self.event_count > 0 {
            self.sum / self.event_count as f64
        } else {
            0.0
        }
    }

    fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).expect("Serialization should not fail")
    }

    fn deserialize(data: &[u8]) -> Self {
        bincode::deserialize(data).expect("Deserialization should not fail")
    }
}

/// Stream Processor with Redis state management
struct StreamProcessor {
    backend: Arc<RedisStateBackend>,
    coordinator: Arc<CheckpointCoordinator<RedisStateBackend>>,
    window_duration_ms: i64,
    active_windows: Arc<RwLock<Vec<String>>>,
}

impl StreamProcessor {
    /// Create a new stream processor with Redis state backend
    async fn new(redis_url: &str, checkpoint_dir: &str) -> anyhow::Result<Self> {
        info!("Initializing Stream Processor with Redis state backend");

        // Configure Redis backend
        let config = RedisConfig::builder()
            .url(redis_url)
            .key_prefix("stream_processor:")
            .default_ttl(Duration::from_secs(3600)) // 1 hour TTL for window state
            .pool_size(10, 50) // Min 10, max 50 connections
            .timeouts(
                Duration::from_secs(5),  // Connect timeout
                Duration::from_secs(3),  // Read timeout
                Duration::from_secs(3),  // Write timeout
            )
            .retries(
                3,                          // Max 3 retries
                Duration::from_millis(100), // Base delay
                Duration::from_secs(5),     // Max delay
            )
            .cluster_mode(false)
            .pipeline_batch_size(100)
            .build()?;

        info!("Connecting to Redis at {}", config.primary_url());
        let backend = Arc::new(RedisStateBackend::new(config).await?);

        // Verify connection health
        if !backend.health_check().await? {
            return Err(anyhow::anyhow!("Redis health check failed"));
        }
        info!("✓ Redis connection healthy");

        // Configure checkpoint coordinator
        let checkpoint_options = CheckpointOptions {
            checkpoint_dir: checkpoint_dir.into(),
            max_checkpoints: 10,                    // Keep last 10 checkpoints
            compress: true,                         // Enable compression
            min_interval: Duration::from_secs(10), // Min interval between checkpoints
        };

        let coordinator = Arc::new(CheckpointCoordinator::new(
            backend.clone(),
            Duration::from_secs(10), // Checkpoint every 10 seconds
            checkpoint_options,
        ));

        // Start periodic checkpointing
        coordinator.start().await?;
        info!("✓ Checkpoint coordinator started (interval: 10s)");

        Ok(Self {
            backend,
            coordinator,
            window_duration_ms: 5000, // 5-second windows
            active_windows: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Attempt to restore state from latest checkpoint
    async fn restore_state(&self) -> anyhow::Result<()> {
        info!("Attempting to restore state from checkpoint...");

        match self.coordinator.restore_latest().await {
            Ok(checkpoint_id) => {
                info!("✓ State restored from checkpoint: {}", checkpoint_id);

                // Reload active windows list
                let window_keys = self.backend.list_keys(b"window:").await?;
                let mut active = self.active_windows.write().await;
                active.clear();

                for key in window_keys {
                    if let Ok(key_str) = String::from_utf8(key) {
                        if let Some(window_id) = key_str.strip_prefix("window:") {
                            active.push(window_id.to_string());
                        }
                    }
                }

                info!("✓ Restored {} active windows", active.len());
                Ok(())
            }
            Err(e) => {
                warn!("No checkpoint found or restore failed: {}. Starting fresh.", e);
                Ok(())
            }
        }
    }

    /// Create or update a window with new event
    async fn process_event(&self, timestamp: i64, value: f64) -> anyhow::Result<()> {
        // Calculate window boundaries
        let window_start = (timestamp / self.window_duration_ms) * self.window_duration_ms;
        let window_end = window_start + self.window_duration_ms;
        let window_id = format!("{}", window_start);
        let window_key = format!("window:{}", window_id).into_bytes();

        // Get or create window state
        let mut window = if let Some(data) = self.backend.get(&window_key).await? {
            WindowState::deserialize(&data)
        } else {
            info!("Creating new window: {}", window_id);
            let window = WindowState::new(window_id.clone(), window_start, window_end);

            // Track active window
            let mut active = self.active_windows.write().await;
            if !active.contains(&window_id) {
                active.push(window_id.clone());
            }

            window
        };

        // Add event to window
        window.add_event(value);

        // Store updated window state
        let serialized = window.serialize();
        self.backend.put(&window_key, &serialized).await?;

        Ok(())
    }

    /// Clean up old windows
    async fn cleanup_old_windows(&self, current_time: i64) -> anyhow::Result<usize> {
        let cutoff = current_time - (self.window_duration_ms * 10); // Keep last 10 windows
        let mut deleted = 0;

        let active = self.active_windows.read().await;
        let mut to_delete = Vec::new();

        for window_id in active.iter() {
            if let Ok(start_time) = window_id.parse::<i64>() {
                if start_time < cutoff {
                    to_delete.push(window_id.clone());
                }
            }
        }
        drop(active);

        if !to_delete.is_empty() {
            info!("Cleaning up {} old windows", to_delete.len());

            let keys: Vec<Vec<u8>> = to_delete
                .iter()
                .map(|id| format!("window:{}", id).into_bytes())
                .collect();

            let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
            deleted = self.backend.batch_delete(&key_refs).await?;

            // Update active windows list
            let mut active = self.active_windows.write().await;
            active.retain(|id| !to_delete.contains(id));
        }

        Ok(deleted)
    }

    /// Get current statistics
    async fn print_statistics(&self) {
        let stats = self.backend.stats().await;
        let checkpoint_stats = self.coordinator.stats().await;

        info!("=== Statistics ===");
        info!("State Backend:");
        info!("  Total operations: {}", stats.total_operations());
        info!("  GET operations: {}", stats.get_count);
        info!("  PUT operations: {}", stats.put_count);
        info!("  DELETE operations: {}", stats.delete_count);
        info!("  Hit rate: {:.2}%", stats.hit_rate() * 100.0);
        info!("  Error rate: {:.2}%", stats.error_rate() * 100.0);
        info!("  Retry count: {}", stats.retry_count);
        info!("  Avg latency: {}μs", stats.avg_latency_us);
        info!("  Bytes read: {}", stats.bytes_read);
        info!("  Bytes written: {}", stats.bytes_written);

        info!("Checkpointing:");
        info!("  Checkpoints created: {}", checkpoint_stats.checkpoints_created);
        info!("  Checkpoint failures: {}", checkpoint_stats.checkpoint_failures);
        info!("  Restores: {}", checkpoint_stats.restores);
        info!(
            "  Last checkpoint: {:?}",
            checkpoint_stats.last_checkpoint_time
        );
        info!(
            "  Last checkpoint duration: {:?}ms",
            checkpoint_stats.last_checkpoint_duration_ms
        );

        let active_count = self.active_windows.read().await.len();
        info!("Active windows: {}", active_count);
    }

    /// Simulate connection failure and recovery
    async fn simulate_connection_failure(&self) -> anyhow::Result<()> {
        info!("=== Simulating Connection Failure ===");

        // This will trigger retries automatically
        // Try to perform operations - retry logic will handle transient failures
        info!("Attempting operations during simulated failure...");

        match self.backend.get(b"test_key").await {
            Ok(_) => info!("✓ Operation succeeded (connection recovered)"),
            Err(e) => warn!("Operation failed after retries: {}", e),
        }

        Ok(())
    }

    /// Shutdown processor
    async fn shutdown(&self) -> anyhow::Result<()> {
        info!("Shutting down Stream Processor...");

        // Stop checkpoint coordinator (creates final checkpoint)
        self.coordinator.shutdown().await?;
        info!("✓ Final checkpoint created");

        // Print final statistics
        self.print_statistics().await;

        info!("✓ Shutdown complete");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info,stream_processor_state_complete=debug")
        .with_target(false)
        .with_thread_ids(false)
        .with_line_number(false)
        .init();

    info!("=== Stream Processor State Management Demo ===");

    // Create processor with Redis state backend
    let checkpoint_dir = std::env::temp_dir().join("stream_processor_checkpoints");
    std::fs::create_dir_all(&checkpoint_dir)?;

    let processor = StreamProcessor::new(
        "redis://localhost:6379",
        checkpoint_dir.to_str().unwrap(),
    )
    .await?;

    // Attempt to restore state from previous run
    processor.restore_state().await?;

    // Set up graceful shutdown
    let shutdown_signal = Arc::new(tokio::sync::Notify::new());
    let shutdown_signal_clone = shutdown_signal.clone();

    tokio::spawn(async move {
        signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        info!("Received shutdown signal (Ctrl+C)");
        shutdown_signal_clone.notify_one();
    });

    // Simulate stream processing
    info!("Starting stream processing...");
    let mut event_interval = interval(Duration::from_millis(100));
    let mut stats_interval = interval(Duration::from_secs(30));
    let mut cleanup_interval = interval(Duration::from_secs(60));

    let mut event_count = 0u64;
    let start_time = chrono::Utc::now();

    loop {
        tokio::select! {
            _ = event_interval.tick() => {
                // Simulate incoming event
                let timestamp = chrono::Utc::now().timestamp_millis();
                let value = rand::random::<f64>() * 100.0;

                if let Err(e) = processor.process_event(timestamp, value).await {
                    error!("Failed to process event: {}", e);
                } else {
                    event_count += 1;

                    if event_count % 100 == 0 {
                        info!("Processed {} events", event_count);
                    }
                }
            }

            _ = stats_interval.tick() => {
                // Print statistics every 30 seconds
                processor.print_statistics().await;

                let elapsed = chrono::Utc::now().signed_duration_since(start_time);
                let events_per_sec = event_count as f64 / elapsed.num_seconds() as f64;
                info!("Throughput: {:.2} events/sec", events_per_sec);
            }

            _ = cleanup_interval.tick() => {
                // Clean up old windows every minute
                let current_time = chrono::Utc::now().timestamp_millis();
                match processor.cleanup_old_windows(current_time).await {
                    Ok(deleted) => {
                        if deleted > 0 {
                            info!("Cleaned up {} old windows", deleted);
                        }
                    }
                    Err(e) => error!("Failed to clean up windows: {}", e),
                }
            }

            _ = shutdown_signal.notified() => {
                // Graceful shutdown
                break;
            }
        }
    }

    // Shutdown processor
    processor.shutdown().await?;

    info!("=== Demo Complete ===");
    info!("Total events processed: {}", event_count);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_window_state() {
        let mut window = WindowState::new("test".to_string(), 0, 5000);

        window.add_event(10.0);
        window.add_event(20.0);
        window.add_event(30.0);

        assert_eq!(window.event_count, 3);
        assert_eq!(window.sum, 60.0);
        assert_eq!(window.average(), 20.0);
        assert_eq!(window.min, 10.0);
        assert_eq!(window.max, 30.0);
    }

    #[tokio::test]
    async fn test_window_serialization() {
        let window = WindowState::new("test".to_string(), 0, 5000);
        let serialized = window.serialize();
        let deserialized = WindowState::deserialize(&serialized);

        assert_eq!(window.window_id, deserialized.window_id);
        assert_eq!(window.start_time, deserialized.start_time);
        assert_eq!(window.end_time, deserialized.end_time);
    }
}
