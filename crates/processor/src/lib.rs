//! Stream processor for LLM Auto-Optimizer
//!
//! This crate provides stream processing capabilities for handling events,
//! metrics, and feedback in the optimizer system.

pub mod aggregation;
pub mod config;
pub mod core;
pub mod error;
pub mod kafka;
pub mod pipeline;
pub mod state;
pub mod watermark;
pub mod window;

// Re-export commonly used types
pub use core::{
    CloudEventKeyExtractor, CloudEventTimeExtractor, EventKey, EventTimeExtractor,
    FeedbackEventKeyExtractor, FeedbackEventTimeExtractor, KeyExtractor,
    MetricPointKeyExtractor, MetricPointTimeExtractor, ProcessorEvent,
};

pub use window::{
    Window, WindowBounds, WindowType, WindowAssigner, WindowTrigger, TriggerResult, TriggerContext,
    WindowMerger, TumblingWindowAssigner, SlidingWindowAssigner, SessionWindowAssigner,
    OnWatermarkTrigger, ProcessingTimeTrigger, CountTrigger, CompositeTrigger, CompositeMode,
    ImmediateTrigger, NeverTrigger,
};

pub use error::{
    ProcessorError, WindowError, AggregationError, StateError, WatermarkError,
    Result as ProcessorResult,
};

pub use config::{
    ProcessorConfig, WindowConfig, WindowType as ConfigWindowType,
    WatermarkConfig, WatermarkStrategy, StateConfig, StateBackend,
    AggregationConfig, AggregationType,
};

pub use pipeline::{
    StreamPipelineBuilder, StreamPipeline, PipelineConfig,
    StreamExecutor, ExecutorStats,
    StreamOperator, MapOperator, FilterOperator, FlatMapOperator,
    KeyByOperator, WindowOperator, AggregateOperator, OperatorContext,
};

pub use kafka::{
    // Offset management
    OffsetManager, OffsetStore, InMemoryOffsetStore, StateBackendOffsetStore,
    TopicPartition, OffsetInfo, OffsetCommitStrategy, OffsetResetStrategy,
    OffsetStats,
    // Source (consumer)
    CommitStrategy, KafkaSource, KafkaSourceConfig, KafkaSourceMetrics,
    SourceMessage,
    // Sink (producer)
    BincodeSerializer, DeliveryGuarantee, JsonSerializer, KafkaSink, KafkaSinkConfig,
    MessageSerializer, PartitionStrategy, SinkMessage, SinkMetrics,
};
