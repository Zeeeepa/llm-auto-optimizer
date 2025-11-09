//! Stream executor for running stream processing pipelines
//!
//! This module provides the execution engine for stream processing pipelines,
//! handling event ingestion, watermark propagation, window management, and result emission.

use crate::core::{EventTimeExtractor, KeyExtractor, ProcessorEvent};
use crate::error::{ProcessorError, Result};
use crate::pipeline::builder::StreamPipeline;
use crate::watermark::{BoundedOutOfOrdernessWatermark, Watermark, WatermarkGenerator};
use crate::window::{
    SlidingWindowAssigner, SessionWindowAssigner,
    TumblingWindowAssigner, Window, WindowAssigner,
};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time;
use tracing::{debug, error, info, trace, warn};

/// Statistics for the stream executor
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutorStats {
    /// Total events processed
    pub events_processed: u64,

    /// Total events dropped (late events)
    pub events_dropped: u64,

    /// Total windows created
    pub windows_created: u64,

    /// Total windows triggered
    pub windows_triggered: u64,

    /// Total aggregations computed
    pub aggregations_computed: u64,

    /// Current watermark timestamp
    pub current_watermark: i64,

    /// Number of active windows
    pub active_windows: u64,

    /// Total bytes processed
    pub bytes_processed: u64,

    /// Processing errors
    pub errors: u64,
}

impl ExecutorStats {
    /// Create new empty stats
    pub fn new() -> Self {
        Self::default()
    }

    /// Increment events processed
    pub fn inc_events_processed(&mut self) {
        self.events_processed += 1;
    }

    /// Increment events dropped
    pub fn inc_events_dropped(&mut self) {
        self.events_dropped += 1;
    }

    /// Increment windows created
    pub fn inc_windows_created(&mut self) {
        self.windows_created += 1;
    }

    /// Increment windows triggered
    pub fn inc_windows_triggered(&mut self) {
        self.windows_triggered += 1;
    }

    /// Increment aggregations computed
    pub fn inc_aggregations_computed(&mut self) {
        self.aggregations_computed += 1;
    }

    /// Update watermark
    pub fn update_watermark(&mut self, watermark: i64) {
        self.current_watermark = watermark;
    }

    /// Set active windows count
    pub fn set_active_windows(&mut self, count: u64) {
        self.active_windows = count;
    }

    /// Add bytes processed
    pub fn add_bytes_processed(&mut self, bytes: u64) {
        self.bytes_processed += bytes;
    }

    /// Increment errors
    pub fn inc_errors(&mut self) {
        self.errors += 1;
    }

    /// Get events per second
    pub fn events_per_second(&self, elapsed_seconds: f64) -> f64 {
        if elapsed_seconds > 0.0 {
            self.events_processed as f64 / elapsed_seconds
        } else {
            0.0
        }
    }

    /// Get throughput in MB/s
    pub fn throughput_mbps(&self, elapsed_seconds: f64) -> f64 {
        if elapsed_seconds > 0.0 {
            (self.bytes_processed as f64 / 1_048_576.0) / elapsed_seconds
        } else {
            0.0
        }
    }
}

/// Window state for tracking window lifecycle
#[derive(Debug, Clone)]
struct WindowState<T> {
    /// The window
    window: Window,

    /// Events in this window
    events: Vec<T>,

    /// Last update timestamp
    last_update: i64,

    /// Whether the window has been triggered
    triggered: bool,
}

impl<T> WindowState<T> {
    fn new(window: Window) -> Self {
        Self {
            window,
            events: Vec::new(),
            last_update: chrono::Utc::now().timestamp_millis(),
            triggered: false,
        }
    }

    fn add_event(&mut self, event: T) {
        self.events.push(event);
        self.last_update = chrono::Utc::now().timestamp_millis();
    }

    fn event_count(&self) -> usize {
        self.events.len()
    }
}

/// Stream executor for processing events through a pipeline
///
/// The executor handles:
/// - Event ingestion from sources
/// - Watermark generation and propagation
/// - Window assignment and lifecycle management
/// - Trigger evaluation
/// - Aggregation computation
/// - Result emission to sinks
///
/// # Example
///
/// ```rust,no_run
/// use processor::pipeline::{StreamPipelineBuilder, StreamExecutor};
/// use processor::config::WindowConfig;
///
/// # async fn example() -> anyhow::Result<()> {
/// let pipeline = StreamPipelineBuilder::new()
///     .with_name("example")
///     .with_tumbling_window(60_000)
///     .build()?;
///
/// let mut executor = pipeline.create_executor::<f64>();
///
/// // Ingest events
/// executor.ingest(42.0).await?;
/// executor.ingest(100.0).await?;
///
/// // Process and emit results
/// // executor.run().await?;
/// # Ok(())
/// # }
/// ```
pub struct StreamExecutor<T>
where
    T: Clone + Send + Sync + 'static,
{
    /// The pipeline configuration
    pipeline: StreamPipeline,

    /// Event input channel
    input_tx: mpsc::Sender<T>,
    input_rx: Arc<RwLock<mpsc::Receiver<T>>>,

    /// Output channel
    output_tx: mpsc::Sender<ProcessorEvent<T>>,
    output_rx: Arc<RwLock<mpsc::Receiver<ProcessorEvent<T>>>>,

    /// Watermark generator
    watermark_generator: Arc<RwLock<Box<dyn WatermarkGenerator>>>,

    /// Window states per key
    window_states: Arc<DashMap<String, Vec<WindowState<T>>>>,

    /// Execution statistics
    stats: Arc<RwLock<ExecutorStats>>,

    /// Running flag
    running: Arc<AtomicU64>,

    /// Partition ID
    partition: u32,
}

impl<T> StreamExecutor<T>
where
    T: Clone + Send + Sync + 'static + Serialize + for<'de> Deserialize<'de>,
{
    /// Create a new stream executor
    pub fn new(pipeline: StreamPipeline) -> Self {
        let buffer_size = pipeline.config().processor.buffer_size;

        let (input_tx, input_rx) = mpsc::channel(buffer_size);
        let (output_tx, output_rx) = mpsc::channel(buffer_size);

        // Create watermark generator based on configuration
        let watermark_config = &pipeline.config().processor.watermark;
        let watermark_generator: Box<dyn WatermarkGenerator> =
            Box::new(BoundedOutOfOrdernessWatermark::new(
                Duration::from_millis(watermark_config.max_delay_ms),
                Some(Duration::from_millis(watermark_config.idle_timeout_ms)),
            ));

        Self {
            pipeline,
            input_tx,
            input_rx: Arc::new(RwLock::new(input_rx)),
            output_tx,
            output_rx: Arc::new(RwLock::new(output_rx)),
            watermark_generator: Arc::new(RwLock::new(watermark_generator)),
            window_states: Arc::new(DashMap::new()),
            stats: Arc::new(RwLock::new(ExecutorStats::new())),
            running: Arc::new(AtomicU64::new(0)),
            partition: 0,
        }
    }

    /// Ingest an event into the pipeline
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use processor::pipeline::{StreamPipelineBuilder, StreamExecutor};
    /// # async fn example() -> anyhow::Result<()> {
    /// # let pipeline = StreamPipelineBuilder::new().build()?;
    /// let mut executor = pipeline.create_executor::<f64>();
    /// executor.ingest(42.5).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn ingest(&self, event: T) -> Result<()> {
        self.input_tx
            .send(event)
            .await
            .map_err(|e| ProcessorError::Execution {
                source: format!("Failed to ingest event: {}", e).into(),
            })?;
        Ok(())
    }

    /// Ingest a batch of events
    pub async fn ingest_batch(&self, events: Vec<T>) -> Result<()> {
        for event in events {
            self.ingest(event).await?;
        }
        Ok(())
    }

    /// Get the next output event
    pub async fn next_output(&self) -> Option<ProcessorEvent<T>> {
        let mut rx = self.output_rx.write().await;
        rx.recv().await
    }

    /// Get current statistics
    pub async fn stats(&self) -> ExecutorStats {
        self.stats.read().await.clone()
    }

    /// Get current watermark
    pub async fn current_watermark(&self) -> Watermark {
        self.watermark_generator.read().await.current_watermark()
    }

    /// Check if the executor is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed) == 1
    }

    /// Run the executor (process events continuously)
    ///
    /// This will process events from the input channel until stopped.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use processor::pipeline::{StreamPipelineBuilder, StreamExecutor};
    /// # async fn example() -> anyhow::Result<()> {
    /// # let pipeline = StreamPipelineBuilder::new().build()?;
    /// let executor = pipeline.create_executor::<f64>();
    ///
    /// // Run in a background task
    /// tokio::spawn(async move {
    ///     // executor.run().await.unwrap();
    /// });
    /// # Ok(())
    /// # }
    /// ```
    pub async fn run(&self) -> Result<()>
    where
        T: EventTimeExtractor<T> + KeyExtractor<T>,
    {
        info!(
            pipeline = %self.pipeline.name(),
            "Starting stream executor"
        );

        self.running.store(1, Ordering::Relaxed);

        // Spawn watermark update task
        let watermark_interval = self.pipeline.config().processor.watermark.update_interval_ms;
        self.spawn_watermark_updater(Duration::from_millis(watermark_interval));

        // Process events
        loop {
            if self.running.load(Ordering::Relaxed) == 0 {
                info!("Executor stopped");
                break;
            }

            let mut rx = self.input_rx.write().await;
            match time::timeout(Duration::from_millis(100), rx.recv()).await {
                Ok(Some(event)) => {
                    drop(rx); // Release the lock
                    if let Err(e) = self.process_event(event).await {
                        error!(error = %e, "Failed to process event");
                        self.stats.write().await.inc_errors();
                    }
                }
                Ok(None) => {
                    debug!("Input channel closed");
                    break;
                }
                Err(_) => {
                    // Timeout - continue
                    continue;
                }
            }
        }

        info!("Stream executor stopped");
        Ok(())
    }

    /// Stop the executor
    pub fn stop(&self) {
        info!("Stopping stream executor");
        self.running.store(0, Ordering::Relaxed);
    }

    /// Process a single event
    async fn process_event(&self, event: T) -> Result<()>
    where
        T: EventTimeExtractor<T> + KeyExtractor<T>,
    {
        trace!("Processing event");

        // Extract event time and key
        let event_time = event.extract_event_time(&event);
        let key = event.extract_key(&event);
        let key_string = key.to_key_string();

        let timestamp = event_time.timestamp_millis();

        // Update watermark
        let mut wm_gen = self.watermark_generator.write().await;
        let new_watermark = wm_gen.on_event(timestamp, self.partition);
        drop(wm_gen);

        if let Some(watermark) = new_watermark {
            debug!(watermark = %watermark, "Watermark advanced");
            self.stats.write().await.update_watermark(watermark.timestamp);

            // Check and trigger windows
            self.check_and_trigger_windows(watermark).await?;
        }

        // Check if event is late
        let current_watermark = self.current_watermark().await;
        if timestamp < current_watermark.timestamp {
            let config = &self.pipeline.config().processor.window;
            let lateness = current_watermark.timestamp - timestamp;

            if lateness > config.allowed_lateness_ms as i64 {
                if config.drop_late_events {
                    warn!(
                        lateness_ms = lateness,
                        "Dropping late event"
                    );
                    self.stats.write().await.inc_events_dropped();
                    return Ok(());
                } else {
                    warn!(
                        lateness_ms = lateness,
                        "Processing late event"
                    );
                }
            }
        }

        // Assign to windows
        let windows = self.assign_windows(timestamp, &key_string);

        // Add event to windows
        for window in windows {
            self.add_event_to_window(&key_string, window, event.clone())
                .await;
        }

        // Update stats
        let mut stats = self.stats.write().await;
        stats.inc_events_processed();
        stats.add_bytes_processed(std::mem::size_of::<T>() as u64);
        stats.set_active_windows(self.window_states.len() as u64);

        Ok(())
    }

    /// Assign event to windows
    fn assign_windows(&self, timestamp: i64, _key: &str) -> Vec<Window> {
        let window_config = &self.pipeline.config().processor.window;
        let dt = chrono::DateTime::from_timestamp_millis(timestamp)
            .unwrap_or_else(chrono::Utc::now);

        match window_config.window_type {
            crate::config::WindowType::Tumbling => {
                let size = chrono::Duration::milliseconds(window_config.size_ms.unwrap_or(60_000) as i64);
                let assigner = TumblingWindowAssigner::new(size);
                assigner.assign_windows(dt)
            }
            crate::config::WindowType::Sliding => {
                let size = chrono::Duration::milliseconds(window_config.size_ms.unwrap_or(60_000) as i64);
                let slide = chrono::Duration::milliseconds(window_config.slide_ms.unwrap_or(30_000) as i64);
                let assigner = SlidingWindowAssigner::new(size, slide);
                assigner.assign_windows(dt)
            }
            crate::config::WindowType::Session => {
                let gap = chrono::Duration::milliseconds(window_config.gap_ms.unwrap_or(30_000) as i64);
                let assigner = SessionWindowAssigner::new(gap);
                assigner.assign_windows(dt)
            }
            crate::config::WindowType::Count => {
                // For count windows, use a single global window
                let start = chrono::DateTime::from_timestamp(i64::MIN / 1000, 0).unwrap_or(chrono::DateTime::UNIX_EPOCH);
                let end = chrono::DateTime::from_timestamp(i64::MAX / 1000, 0).unwrap_or(chrono::DateTime::from_timestamp(253402300799, 999999999).unwrap());
                let bounds = crate::window::WindowBounds::new(start, end);
                vec![Window::new(bounds)]
            }
            crate::config::WindowType::Global => {
                let start = chrono::DateTime::from_timestamp(i64::MIN / 1000, 0).unwrap_or(chrono::DateTime::UNIX_EPOCH);
                let end = chrono::DateTime::from_timestamp(i64::MAX / 1000, 0).unwrap_or(chrono::DateTime::from_timestamp(253402300799, 999999999).unwrap());
                let bounds = crate::window::WindowBounds::new(start, end);
                vec![Window::new(bounds)]
            }
        }
    }

    /// Add event to a window
    async fn add_event_to_window(&self, key: &str, window: Window, event: T) {
        let mut windows = self
            .window_states
            .entry(key.to_string())
            .or_insert_with(Vec::new);

        // Find or create window state
        let window_state = windows
            .iter_mut()
            .find(|ws| ws.window == window);

        match window_state {
            Some(ws) => {
                ws.add_event(event);
                trace!(
                    key = key,
                    window = ?window,
                    event_count = ws.event_count(),
                    "Added event to existing window"
                );
            }
            None => {
                let mut ws = WindowState::new(window.clone());
                ws.add_event(event);
                windows.push(ws);
                self.stats.write().await.inc_windows_created();
                trace!(
                    key = key,
                    window = ?window,
                    "Created new window"
                );
            }
        }
    }

    /// Check and trigger windows based on watermark
    async fn check_and_trigger_windows(&self, watermark: Watermark) -> Result<()> {
        let mut windows_to_trigger = Vec::new();

        // Find windows that should be triggered
        for entry in self.window_states.iter() {
            let key = entry.key().clone();
            let windows = entry.value();

            for (idx, window_state) in windows.iter().enumerate() {
                let window_end_ms = window_state.window.bounds.end.timestamp_millis();
                if !window_state.triggered && window_end_ms <= watermark.timestamp {
                    windows_to_trigger.push((key.clone(), idx, window_state.window.clone()));
                }
            }
        }

        // Trigger windows
        for (key, idx, window) in windows_to_trigger {
            self.trigger_window(&key, idx, watermark).await?;
        }

        Ok(())
    }

    /// Trigger a window and emit results
    async fn trigger_window(
        &self,
        key: &str,
        window_idx: usize,
        watermark: Watermark,
    ) -> Result<()> {
        debug!(
            key = key,
            window_idx = window_idx,
            watermark = %watermark,
            "Triggering window"
        );

        // Get window state
        let mut window_states = self
            .window_states
            .get_mut(key)
            .ok_or_else(|| ProcessorError::Execution {
                source: format!("Window state not found for key: {}", key).into(),
            })?;

        if window_idx >= window_states.len() {
            return Err(ProcessorError::Execution {
                source: format!("Invalid window index: {}", window_idx).into(),
            });
        }

        let window_state = &mut window_states[window_idx];
        window_state.triggered = true;

        // For now, just emit all events in the window
        // In a full implementation, this would apply aggregations
        for event in &window_state.events {
            let processor_event = ProcessorEvent::new(
                event.clone(),
                window_state.window.bounds.start,
                key.to_string(),
            );

            self.output_tx
                .send(processor_event)
                .await
                .map_err(|e| ProcessorError::Execution {
                    source: format!("Failed to emit result: {}", e).into(),
                })?;
        }

        self.stats.write().await.inc_windows_triggered();

        // Clean up triggered windows (optional: keep for late events)
        // For now, we keep them
        trace!(
            key = key,
            event_count = window_state.events.len(),
            "Window triggered and results emitted"
        );

        Ok(())
    }

    /// Spawn a background task to periodically update watermarks
    fn spawn_watermark_updater(&self, interval: Duration) {
        let watermark_generator = Arc::clone(&self.watermark_generator);
        let running = Arc::clone(&self.running);
        let stats = Arc::clone(&self.stats);

        tokio::spawn(async move {
            let mut ticker = time::interval(interval);

            while running.load(Ordering::Relaxed) == 1 {
                ticker.tick().await;

                let mut gen = watermark_generator.write().await;
                if let Some(watermark) = gen.on_periodic_check() {
                    debug!(watermark = %watermark, "Periodic watermark update");
                    stats.write().await.update_watermark(watermark.timestamp);
                }
            }

            debug!("Watermark updater stopped");
        });
    }

    /// Get the number of active windows
    pub fn active_window_count(&self) -> usize {
        self.window_states.len()
    }

    /// Clear all window states
    pub fn clear_windows(&self) {
        self.window_states.clear();
    }
}

impl<T> Drop for StreamExecutor<T>
where
    T: Clone + Send + Sync,
{
    fn drop(&mut self) {
        // Stop the executor by setting the running flag to 0
        self.running.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WindowConfig;
    use crate::core::{EventKey, EventTimeExtractor, KeyExtractor};
    use crate::pipeline::StreamPipelineBuilder;
    use chrono::{DateTime, Utc};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestEvent {
        value: f64,
        timestamp: i64,
        key: String,
    }

    impl EventTimeExtractor<TestEvent> for TestEvent {
        fn extract_event_time(&self, _event: &TestEvent) -> DateTime<Utc> {
            DateTime::from_timestamp_millis(self.timestamp).unwrap_or_else(Utc::now)
        }
    }

    impl KeyExtractor<TestEvent> for TestEvent {
        fn extract_key(&self, event: &TestEvent) -> EventKey {
            EventKey::Session(event.key.clone())
        }
    }

    #[tokio::test]
    async fn test_executor_creation() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test")
            .build()
            .unwrap();

        let executor: StreamExecutor<TestEvent> = pipeline.create_executor();
        assert!(!executor.is_running());
    }

    #[tokio::test]
    async fn test_executor_ingest() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test")
            .build()
            .unwrap();

        let executor = pipeline.create_executor();

        let event = TestEvent {
            value: 42.0,
            timestamp: Utc::now().timestamp_millis(),
            key: "test-key".to_string(),
        };

        executor.ingest(event).await.unwrap();
    }

    #[tokio::test]
    async fn test_executor_ingest_batch() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test")
            .build()
            .unwrap();

        let executor = pipeline.create_executor();

        let events = vec![
            TestEvent {
                value: 1.0,
                timestamp: Utc::now().timestamp_millis(),
                key: "key1".to_string(),
            },
            TestEvent {
                value: 2.0,
                timestamp: Utc::now().timestamp_millis(),
                key: "key2".to_string(),
            },
        ];

        executor.ingest_batch(events).await.unwrap();
    }

    #[tokio::test]
    async fn test_executor_stats() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test")
            .build()
            .unwrap();

        let executor: StreamExecutor<TestEvent> = pipeline.create_executor();

        let stats = executor.stats().await;
        assert_eq!(stats.events_processed, 0);
        assert_eq!(stats.events_dropped, 0);
    }

    #[tokio::test]
    async fn test_executor_watermark() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test")
            .build()
            .unwrap();

        let executor: StreamExecutor<TestEvent> = pipeline.create_executor();

        let watermark = executor.current_watermark().await;
        assert!(watermark.is_min());
    }

    #[test]
    fn test_executor_stats_methods() {
        let mut stats = ExecutorStats::new();

        stats.inc_events_processed();
        assert_eq!(stats.events_processed, 1);

        stats.inc_events_dropped();
        assert_eq!(stats.events_dropped, 1);

        stats.inc_windows_created();
        assert_eq!(stats.windows_created, 1);

        stats.inc_windows_triggered();
        assert_eq!(stats.windows_triggered, 1);

        stats.inc_aggregations_computed();
        assert_eq!(stats.aggregations_computed, 1);

        stats.update_watermark(12345);
        assert_eq!(stats.current_watermark, 12345);

        stats.set_active_windows(10);
        assert_eq!(stats.active_windows, 10);

        stats.add_bytes_processed(1024);
        assert_eq!(stats.bytes_processed, 1024);

        stats.inc_errors();
        assert_eq!(stats.errors, 1);
    }

    #[test]
    fn test_executor_stats_metrics() {
        let mut stats = ExecutorStats::new();
        stats.events_processed = 1000;
        stats.bytes_processed = 10_485_760; // 10 MB

        let eps = stats.events_per_second(10.0);
        assert_eq!(eps, 100.0);

        let mbps = stats.throughput_mbps(10.0);
        assert_eq!(mbps, 1.0);
    }

    #[tokio::test]
    async fn test_executor_stop() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test")
            .build()
            .unwrap();

        let executor: StreamExecutor<TestEvent> = pipeline.create_executor();

        executor.running.store(1, Ordering::Relaxed);
        assert!(executor.is_running());

        executor.stop();
        assert!(!executor.is_running());
    }

    #[tokio::test]
    async fn test_assign_tumbling_windows() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test")
            .with_tumbling_window(60_000)
            .build()
            .unwrap();

        let executor: StreamExecutor<TestEvent> = pipeline.create_executor();

        let timestamp = 125_000; // 2 minutes and 5 seconds
        let windows = executor.assign_windows(timestamp, "test-key");

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].bounds.start.timestamp_millis(), 120_000); // Start of 3rd minute
        assert_eq!(windows[0].bounds.end.timestamp_millis(), 180_000); // End of 3rd minute
    }

    #[tokio::test]
    async fn test_assign_sliding_windows() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test")
            .with_sliding_window(120_000, 60_000)
            .build()
            .unwrap();

        let executor: StreamExecutor<TestEvent> = pipeline.create_executor();

        let timestamp = 125_000;
        let windows = executor.assign_windows(timestamp, "test-key");

        assert!(windows.len() >= 1);
    }

    #[tokio::test]
    async fn test_window_state() {
        let start = chrono::DateTime::from_timestamp(0, 0).unwrap();
        let end = chrono::DateTime::from_timestamp(60, 0).unwrap();
        let bounds = crate::window::WindowBounds::new(start, end);
        let window = Window::new(bounds);
        let mut state = WindowState::<i32>::new(window);

        assert_eq!(state.event_count(), 0);
        assert!(!state.triggered);

        state.add_event(42);
        assert_eq!(state.event_count(), 1);
        assert_eq!(state.events[0], 42);
    }

    #[tokio::test]
    async fn test_add_event_to_window() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test")
            .build()
            .unwrap();

        let executor: StreamExecutor<TestEvent> = pipeline.create_executor();

        let start = chrono::DateTime::from_timestamp(0, 0).unwrap();
        let end = chrono::DateTime::from_timestamp(60, 0).unwrap();
        let bounds = crate::window::WindowBounds::new(start, end);
        let window = Window::new(bounds);
        let event = TestEvent {
            value: 42.0,
            timestamp: 30_000,
            key: "test".to_string(),
        };

        executor.add_event_to_window("test-key", window, event).await;

        assert_eq!(executor.active_window_count(), 1);
    }

    #[tokio::test]
    async fn test_clear_windows() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test")
            .build()
            .unwrap();

        let executor: StreamExecutor<TestEvent> = pipeline.create_executor();

        let start = chrono::DateTime::from_timestamp(0, 0).unwrap();
        let end = chrono::DateTime::from_timestamp(60, 0).unwrap();
        let bounds = crate::window::WindowBounds::new(start, end);
        let window = Window::new(bounds);
        let event = TestEvent {
            value: 42.0,
            timestamp: 30_000,
            key: "test".to_string(),
        };

        executor.add_event_to_window("test-key", window, event).await;
        assert_eq!(executor.active_window_count(), 1);

        executor.clear_windows();
        assert_eq!(executor.active_window_count(), 0);
    }
}
