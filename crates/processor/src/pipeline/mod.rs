//! Stream processing pipeline module
//!
//! This module provides a flexible, composable API for building and executing stream
//! processing pipelines with support for:
//! - Fluent pipeline builder API
//! - Stream operators (map, filter, flatmap, key_by, window, aggregate)
//! - Event ingestion and processing
//! - Watermark propagation
//! - Window assignment and triggering
//! - State management
//! - Result emission
//!
//! # Example
//!
//! ```rust,no_run
//! use processor::pipeline::{StreamPipelineBuilder, StreamOperator};
//! use processor::config::{WindowConfig, StateBackend};
//! use std::time::Duration;
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Build a stream processing pipeline
//! let pipeline = StreamPipelineBuilder::new()
//!     .with_name("latency-aggregator")
//!     .with_parallelism(4)
//!     .with_window(WindowConfig::tumbling(60_000))
//!     .with_watermark_delay(Duration::from_secs(5))
//!     .with_state_backend(StateBackend::Memory)
//!     .build()?;
//!
//! // Create an executor and run the pipeline
//! // let executor = pipeline.create_executor();
//! // executor.run().await?;
//! # Ok(())
//! # }
//! ```

pub mod builder;
pub mod executor;
pub mod operator;
pub mod normalization_operator;

pub use builder::{StreamPipelineBuilder, StreamPipeline, PipelineConfig};
pub use executor::{StreamExecutor, ExecutorStats};
pub use operator::{
    StreamOperator, MapOperator, FilterOperator, FlatMapOperator,
    KeyByOperator, WindowOperator, AggregateOperator, DeduplicationOperator,
    OperatorContext, DeduplicationStats,
};
pub use normalization_operator::{NormalizationOperator, NormalizationStats, TimestampedEvent};
