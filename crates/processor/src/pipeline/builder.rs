//! Stream pipeline builder for fluent API configuration
//!
//! This module provides a builder pattern for constructing stream processing pipelines
//! with a clean, composable API.

use crate::config::{ProcessorConfig, StateBackend, WindowConfig, WatermarkConfig};
use crate::error::{ProcessorError, Result};
use crate::pipeline::executor::StreamExecutor;
use crate::pipeline::operator::StreamOperator;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

/// Configuration for a stream processing pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    /// Pipeline name for identification
    pub name: String,

    /// Processor configuration
    pub processor: ProcessorConfig,

    /// Description of the pipeline
    pub description: Option<String>,

    /// Tags for categorization
    pub tags: Vec<String>,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            name: "unnamed-pipeline".to_string(),
            processor: ProcessorConfig::default(),
            description: None,
            tags: Vec::new(),
        }
    }
}

impl PipelineConfig {
    /// Validate the pipeline configuration
    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(ProcessorError::Configuration {
                source: "pipeline name cannot be empty".into(),
            });
        }

        self.processor.validate()?;
        Ok(())
    }
}

/// Builder for constructing stream processing pipelines
///
/// Provides a fluent API for configuring all aspects of a stream processing pipeline
/// including windows, watermarks, state backends, and operators.
///
/// # Example
///
/// ```rust
/// use processor::pipeline::StreamPipelineBuilder;
/// use processor::config::{WindowConfig, StateBackend};
/// use std::time::Duration;
///
/// # fn example() -> anyhow::Result<()> {
/// let pipeline = StreamPipelineBuilder::new()
///     .with_name("my-pipeline")
///     .with_description("Processes user events")
///     .with_parallelism(8)
///     .with_window(WindowConfig::tumbling(60_000))
///     .with_watermark_delay(Duration::from_secs(10))
///     .with_state_backend(StateBackend::Memory)
///     .with_buffer_size(50_000)
///     .with_tag("production")
///     .with_tag("user-events")
///     .build()?;
/// # Ok(())
/// # }
/// ```
pub struct StreamPipelineBuilder {
    config: PipelineConfig,
}

impl Default for StreamPipelineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamPipelineBuilder {
    /// Create a new pipeline builder with default configuration
    pub fn new() -> Self {
        Self {
            config: PipelineConfig::default(),
        }
    }

    /// Set the pipeline name
    ///
    /// # Example
    ///
    /// ```rust
    /// # use processor::pipeline::StreamPipelineBuilder;
    /// let builder = StreamPipelineBuilder::new()
    ///     .with_name("user-event-processor");
    /// ```
    pub fn with_name<S: Into<String>>(mut self, name: S) -> Self {
        self.config.name = name.into();
        self
    }

    /// Set the pipeline description
    pub fn with_description<S: Into<String>>(mut self, description: S) -> Self {
        self.config.description = Some(description.into());
        self
    }

    /// Add a tag to the pipeline
    pub fn with_tag<S: Into<String>>(mut self, tag: S) -> Self {
        self.config.tags.push(tag.into());
        self
    }

    /// Add multiple tags to the pipeline
    pub fn with_tags<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.config.tags.extend(tags.into_iter().map(|s| s.into()));
        self
    }

    /// Set the number of parallel processing workers
    ///
    /// # Example
    ///
    /// ```rust
    /// # use processor::pipeline::StreamPipelineBuilder;
    /// let builder = StreamPipelineBuilder::new()
    ///     .with_parallelism(16);
    /// ```
    pub fn with_parallelism(mut self, parallelism: usize) -> Self {
        self.config.processor.parallelism = parallelism;
        self
    }

    /// Set the event buffer size
    pub fn with_buffer_size(mut self, buffer_size: usize) -> Self {
        self.config.processor.buffer_size = buffer_size;
        self
    }

    /// Enable or disable metrics collection
    pub fn with_metrics(mut self, enabled: bool) -> Self {
        self.config.processor.metrics_enabled = enabled;
        self
    }

    /// Set the window configuration
    ///
    /// # Example
    ///
    /// ```rust
    /// # use processor::pipeline::StreamPipelineBuilder;
    /// # use processor::config::WindowConfig;
    /// let builder = StreamPipelineBuilder::new()
    ///     .with_window(WindowConfig::tumbling(60_000));
    /// ```
    pub fn with_window(mut self, window: WindowConfig) -> Self {
        self.config.processor.window = window;
        self
    }

    /// Set a tumbling window with the given size
    ///
    /// # Arguments
    /// * `size_ms` - Window size in milliseconds
    ///
    /// # Example
    ///
    /// ```rust
    /// # use processor::pipeline::StreamPipelineBuilder;
    /// let builder = StreamPipelineBuilder::new()
    ///     .with_tumbling_window(60_000); // 1 minute windows
    /// ```
    pub fn with_tumbling_window(mut self, size_ms: u64) -> Self {
        self.config.processor.window = WindowConfig::tumbling(size_ms);
        self
    }

    /// Set a sliding window with the given size and slide
    ///
    /// # Arguments
    /// * `size_ms` - Window size in milliseconds
    /// * `slide_ms` - Slide interval in milliseconds
    ///
    /// # Example
    ///
    /// ```rust
    /// # use processor::pipeline::StreamPipelineBuilder;
    /// let builder = StreamPipelineBuilder::new()
    ///     .with_sliding_window(300_000, 60_000); // 5 min window, 1 min slide
    /// ```
    pub fn with_sliding_window(mut self, size_ms: u64, slide_ms: u64) -> Self {
        self.config.processor.window = WindowConfig::sliding(size_ms, slide_ms);
        self
    }

    /// Set a session window with the given gap
    ///
    /// # Arguments
    /// * `gap_ms` - Session gap in milliseconds
    ///
    /// # Example
    ///
    /// ```rust
    /// # use processor::pipeline::StreamPipelineBuilder;
    /// let builder = StreamPipelineBuilder::new()
    ///     .with_session_window(300_000); // 5 minute session gap
    /// ```
    pub fn with_session_window(mut self, gap_ms: u64) -> Self {
        self.config.processor.window = WindowConfig::session(gap_ms);
        self
    }

    /// Set allowed lateness for late events
    pub fn with_allowed_lateness(mut self, lateness_ms: u64) -> Self {
        self.config.processor.window.allowed_lateness_ms = lateness_ms;
        self
    }

    /// Set whether to drop late events
    pub fn with_drop_late_events(mut self, drop: bool) -> Self {
        self.config.processor.window.drop_late_events = drop;
        self
    }

    /// Set the watermark configuration
    pub fn with_watermark_config(mut self, watermark: WatermarkConfig) -> Self {
        self.config.processor.watermark = watermark;
        self
    }

    /// Set the watermark delay (max out-of-orderness)
    ///
    /// # Example
    ///
    /// ```rust
    /// # use processor::pipeline::StreamPipelineBuilder;
    /// # use std::time::Duration;
    /// let builder = StreamPipelineBuilder::new()
    ///     .with_watermark_delay(Duration::from_secs(10));
    /// ```
    pub fn with_watermark_delay(mut self, delay: Duration) -> Self {
        self.config.processor.watermark.max_delay_ms = delay.as_millis() as u64;
        self
    }

    /// Set the idle source timeout
    pub fn with_idle_timeout(mut self, timeout: Duration) -> Self {
        self.config.processor.watermark.idle_timeout_ms = timeout.as_millis() as u64;
        self
    }

    /// Set the watermark update interval
    pub fn with_watermark_interval(mut self, interval: Duration) -> Self {
        self.config.processor.watermark.update_interval_ms = interval.as_millis() as u64;
        self
    }

    /// Set the state backend
    ///
    /// # Example
    ///
    /// ```rust
    /// # use processor::pipeline::StreamPipelineBuilder;
    /// # use processor::config::StateBackend;
    /// let builder = StreamPipelineBuilder::new()
    ///     .with_state_backend(StateBackend::Memory);
    /// ```
    pub fn with_state_backend(mut self, backend: StateBackend) -> Self {
        self.config.processor.state.backend = backend;
        self
    }

    /// Set the state storage path (for file-based backends)
    pub fn with_state_path<S: Into<std::path::PathBuf>>(mut self, path: S) -> Self {
        self.config.processor.state.storage_path = Some(path.into());
        self
    }

    /// Set the checkpoint interval
    pub fn with_checkpoint_interval(mut self, interval: Duration) -> Self {
        self.config.processor.state.checkpoint_interval_ms = interval.as_millis() as u64;
        self
    }

    /// Enable or disable incremental checkpoints
    pub fn with_incremental_checkpoints(mut self, enabled: bool) -> Self {
        self.config.processor.state.incremental_checkpoints = enabled;
        self
    }

    /// Set the number of checkpoints to retain
    pub fn with_checkpoint_retention(mut self, count: usize) -> Self {
        self.config.processor.state.checkpoint_retention = count;
        self
    }

    /// Set state TTL (time-to-live)
    pub fn with_state_ttl(mut self, ttl: Duration) -> Self {
        self.config.processor.state.ttl_ms = Some(ttl.as_millis() as u64);
        self
    }

    /// Set maximum state size for memory backends
    pub fn with_max_state_size(mut self, size_bytes: u64) -> Self {
        self.config.processor.state.max_size_bytes = Some(size_bytes);
        self
    }

    /// Enable state compression
    pub fn with_state_compression(mut self, enabled: bool) -> Self {
        self.config.processor.state.compression_enabled = enabled;
        self
    }

    /// Set aggregation percentiles to compute
    pub fn with_percentiles(mut self, percentiles: Vec<u8>) -> Self {
        self.config.processor.aggregation.percentiles = percentiles;
        self
    }

    /// Set histogram buckets
    pub fn with_histogram_buckets(mut self, buckets: Vec<f64>) -> Self {
        self.config.processor.aggregation.histogram_buckets = Some(buckets);
        self
    }

    /// Set minimum samples required for aggregation
    pub fn with_min_samples(mut self, min: usize) -> Self {
        self.config.processor.aggregation.min_samples = min;
        self
    }

    /// Set maximum samples to keep in memory
    pub fn with_max_samples(mut self, max: usize) -> Self {
        self.config.processor.aggregation.max_samples = max;
        self
    }

    /// Enable outlier detection and removal
    pub fn with_outlier_removal(mut self, threshold: f64) -> Self {
        self.config.processor.aggregation.remove_outliers = true;
        self.config.processor.aggregation.outlier_threshold = threshold;
        self
    }

    /// Build the pipeline with the configured settings
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use processor::pipeline::StreamPipelineBuilder;
    /// # fn example() -> anyhow::Result<()> {
    /// let pipeline = StreamPipelineBuilder::new()
    ///     .with_name("my-pipeline")
    ///     .with_parallelism(4)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn build(self) -> Result<StreamPipeline> {
        self.config.validate()?;

        Ok(StreamPipeline {
            config: Arc::new(self.config),
            operators: Vec::new(),
        })
    }
}

/// A configured stream processing pipeline
///
/// This represents a fully configured pipeline that can be used to create executors
/// and add operators.
#[derive(Clone)]
pub struct StreamPipeline {
    config: Arc<PipelineConfig>,
    operators: Vec<Arc<dyn StreamOperator>>,
}

impl StreamPipeline {
    /// Get the pipeline configuration
    pub fn config(&self) -> &PipelineConfig {
        &self.config
    }

    /// Get the pipeline name
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Get the pipeline description
    pub fn description(&self) -> Option<&str> {
        self.config.description.as_deref()
    }

    /// Get the pipeline tags
    pub fn tags(&self) -> &[String] {
        &self.config.tags
    }

    /// Add an operator to the pipeline
    ///
    /// # Example
    ///
    /// ```rust
    /// # use processor::pipeline::{StreamPipelineBuilder, StreamOperator};
    /// # use processor::pipeline::operator::MapOperator;
    /// # fn example() -> anyhow::Result<()> {
    /// let mut pipeline = StreamPipelineBuilder::new().build()?;
    ///
    /// // Add a map operator
    /// // let operator = MapOperator::new(|x| x * 2);
    /// // pipeline.add_operator(operator);
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_operator<O: StreamOperator + 'static>(&mut self, operator: O) {
        self.operators.push(Arc::new(operator));
    }

    /// Get all operators in the pipeline
    pub fn operators(&self) -> &[Arc<dyn StreamOperator>] {
        &self.operators
    }

    /// Create an executor for this pipeline
    ///
    /// # Example
    ///
    /// ```rust
    /// # use processor::pipeline::StreamPipelineBuilder;
    /// # async fn example() -> anyhow::Result<()> {
    /// let pipeline = StreamPipelineBuilder::new()
    ///     .with_name("test-pipeline")
    ///     .build()?;
    ///
    /// let executor = pipeline.create_executor();
    /// // executor.run().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn create_executor<T>(
        &self,
    ) -> StreamExecutor<T>
    where
        T: Clone + Send + Sync + 'static + Serialize + for<'de> Deserialize<'de>,
    {
        StreamExecutor::new(self.clone())
    }
}

impl std::fmt::Debug for StreamPipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamPipeline")
            .field("config", &self.config)
            .field("operator_count", &self.operators.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WindowType;

    #[test]
    fn test_builder_default() {
        let pipeline = StreamPipelineBuilder::new().build().unwrap();
        assert_eq!(pipeline.name(), "unnamed-pipeline");
    }

    #[test]
    fn test_builder_with_name() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test-pipeline")
            .build()
            .unwrap();

        assert_eq!(pipeline.name(), "test-pipeline");
    }

    #[test]
    fn test_builder_with_description() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test")
            .with_description("A test pipeline")
            .build()
            .unwrap();

        assert_eq!(pipeline.description(), Some("A test pipeline"));
    }

    #[test]
    fn test_builder_with_tags() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test")
            .with_tag("production")
            .with_tag("metrics")
            .build()
            .unwrap();

        assert_eq!(pipeline.tags(), &["production", "metrics"]);
    }

    #[test]
    fn test_builder_with_multiple_tags() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test")
            .with_tags(vec!["tag1", "tag2", "tag3"])
            .build()
            .unwrap();

        assert_eq!(pipeline.tags().len(), 3);
    }

    #[test]
    fn test_builder_with_parallelism() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test")
            .with_parallelism(16)
            .build()
            .unwrap();

        assert_eq!(pipeline.config().processor.parallelism, 16);
    }

    #[test]
    fn test_builder_with_buffer_size() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test")
            .with_buffer_size(50_000)
            .build()
            .unwrap();

        assert_eq!(pipeline.config().processor.buffer_size, 50_000);
    }

    #[test]
    fn test_builder_with_tumbling_window() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test")
            .with_tumbling_window(60_000)
            .build()
            .unwrap();

        assert_eq!(
            pipeline.config().processor.window.window_type,
            WindowType::Tumbling
        );
        assert_eq!(pipeline.config().processor.window.size_ms, Some(60_000));
    }

    #[test]
    fn test_builder_with_sliding_window() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test")
            .with_sliding_window(300_000, 60_000)
            .build()
            .unwrap();

        assert_eq!(
            pipeline.config().processor.window.window_type,
            WindowType::Sliding
        );
        assert_eq!(pipeline.config().processor.window.size_ms, Some(300_000));
        assert_eq!(pipeline.config().processor.window.slide_ms, Some(60_000));
    }

    #[test]
    fn test_builder_with_session_window() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test")
            .with_session_window(300_000)
            .build()
            .unwrap();

        assert_eq!(
            pipeline.config().processor.window.window_type,
            WindowType::Session
        );
        assert_eq!(pipeline.config().processor.window.gap_ms, Some(300_000));
    }

    #[test]
    fn test_builder_with_watermark_delay() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test")
            .with_watermark_delay(Duration::from_secs(10))
            .build()
            .unwrap();

        assert_eq!(pipeline.config().processor.watermark.max_delay_ms, 10_000);
    }

    #[test]
    fn test_builder_with_state_backend() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test")
            .with_state_backend(StateBackend::FileSystem)
            .with_state_path("/tmp/state")
            .build()
            .unwrap();

        assert_eq!(
            pipeline.config().processor.state.backend,
            StateBackend::FileSystem
        );
        assert!(pipeline.config().processor.state.storage_path.is_some());
    }

    #[test]
    fn test_builder_with_checkpoint_config() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test")
            .with_checkpoint_interval(Duration::from_secs(30))
            .with_checkpoint_retention(5)
            .with_incremental_checkpoints(false)
            .build()
            .unwrap();

        assert_eq!(
            pipeline.config().processor.state.checkpoint_interval_ms,
            30_000
        );
        assert_eq!(pipeline.config().processor.state.checkpoint_retention, 5);
        assert!(!pipeline.config().processor.state.incremental_checkpoints);
    }

    #[test]
    fn test_builder_with_state_ttl() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test")
            .with_state_ttl(Duration::from_secs(3600))
            .build()
            .unwrap();

        assert_eq!(pipeline.config().processor.state.ttl_ms, Some(3_600_000));
    }

    #[test]
    fn test_builder_with_aggregation_config() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test")
            .with_percentiles(vec![50, 90, 95, 99])
            .with_min_samples(10)
            .with_max_samples(200_000)
            .build()
            .unwrap();

        assert_eq!(
            pipeline.config().processor.aggregation.percentiles,
            vec![50, 90, 95, 99]
        );
        assert_eq!(pipeline.config().processor.aggregation.min_samples, 10);
        assert_eq!(pipeline.config().processor.aggregation.max_samples, 200_000);
    }

    #[test]
    fn test_builder_with_outlier_removal() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test")
            .with_outlier_removal(3.0)
            .build()
            .unwrap();

        assert!(pipeline.config().processor.aggregation.remove_outliers);
        assert_eq!(
            pipeline.config().processor.aggregation.outlier_threshold,
            3.0
        );
    }

    #[test]
    fn test_builder_fluent_api() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("complex-pipeline")
            .with_description("A complex stream processing pipeline")
            .with_tags(vec!["production", "metrics", "aggregation"])
            .with_parallelism(8)
            .with_buffer_size(100_000)
            .with_tumbling_window(60_000)
            .with_watermark_delay(Duration::from_secs(5))
            .with_state_backend(StateBackend::Memory)
            .with_percentiles(vec![50, 95, 99])
            .build()
            .unwrap();

        assert_eq!(pipeline.name(), "complex-pipeline");
        assert_eq!(pipeline.config().processor.parallelism, 8);
        assert_eq!(pipeline.config().processor.buffer_size, 100_000);
    }

    #[test]
    fn test_pipeline_validation_fails_on_empty_name() {
        let result = StreamPipelineBuilder::new()
            .with_name("")
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_pipeline_create_executor() {
        let pipeline = StreamPipelineBuilder::new()
            .with_name("test")
            .build()
            .unwrap();

        let _executor: StreamExecutor<i32> = pipeline.create_executor();
    }
}
