//! Label management for Prometheus metrics
//!
//! This module provides type-safe label management for metrics, ensuring
//! proper validation and consistent naming.

use std::fmt;

/// Common label names used across metrics
pub struct LabelNames;

impl LabelNames {
    pub const RESULT: &'static str = "result";
    pub const OPERATION: &'static str = "operation";
    pub const BACKEND: &'static str = "backend";
    pub const LAYER: &'static str = "layer";
    pub const STRATEGY: &'static str = "strategy";
    pub const WINDOW_TYPE: &'static str = "window_type";
    pub const AGGREGATE_TYPE: &'static str = "aggregate_type";
    pub const PARTITION: &'static str = "partition";
    pub const TOPIC: &'static str = "topic";
    pub const ERROR_TYPE: &'static str = "error_type";
}

/// Label value for processing results
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResultLabel {
    Success,
    Error,
    Dropped,
    Filtered,
}

impl fmt::Display for ResultLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResultLabel::Success => write!(f, "success"),
            ResultLabel::Error => write!(f, "error"),
            ResultLabel::Dropped => write!(f, "dropped"),
            ResultLabel::Filtered => write!(f, "filtered"),
        }
    }
}

/// Label value for state operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperationLabel {
    Get,
    Put,
    Delete,
    Scan,
    BatchGet,
    BatchPut,
}

impl fmt::Display for OperationLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OperationLabel::Get => write!(f, "get"),
            OperationLabel::Put => write!(f, "put"),
            OperationLabel::Delete => write!(f, "delete"),
            OperationLabel::Scan => write!(f, "scan"),
            OperationLabel::BatchGet => write!(f, "batch_get"),
            OperationLabel::BatchPut => write!(f, "batch_put"),
        }
    }
}

/// Label value for state backends
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackendLabel {
    InMemory,
    Sled,
    Redis,
    Postgres,
}

impl fmt::Display for BackendLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendLabel::InMemory => write!(f, "in_memory"),
            BackendLabel::Sled => write!(f, "sled"),
            BackendLabel::Redis => write!(f, "redis"),
            BackendLabel::Postgres => write!(f, "postgres"),
        }
    }
}

/// Label value for cache layers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheLayerLabel {
    L1,
    L2,
    L3,
}

impl fmt::Display for CacheLayerLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CacheLayerLabel::L1 => write!(f, "l1"),
            CacheLayerLabel::L2 => write!(f, "l2"),
            CacheLayerLabel::L3 => write!(f, "l3"),
        }
    }
}

/// Label value for fill strategies in normalization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StrategyLabel {
    Forward,
    Backward,
    Linear,
    Zero,
    Drop,
}

impl fmt::Display for StrategyLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StrategyLabel::Forward => write!(f, "forward"),
            StrategyLabel::Backward => write!(f, "backward"),
            StrategyLabel::Linear => write!(f, "linear"),
            StrategyLabel::Zero => write!(f, "zero"),
            StrategyLabel::Drop => write!(f, "drop"),
        }
    }
}

/// Label value for window types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowTypeLabel {
    Tumbling,
    Sliding,
    Session,
}

impl fmt::Display for WindowTypeLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WindowTypeLabel::Tumbling => write!(f, "tumbling"),
            WindowTypeLabel::Sliding => write!(f, "sliding"),
            WindowTypeLabel::Session => write!(f, "session"),
        }
    }
}

/// Type-safe label value wrapper
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LabelValue {
    Result(ResultLabel),
    Operation(OperationLabel),
    Backend(BackendLabel),
    CacheLayer(CacheLayerLabel),
    Strategy(StrategyLabel),
    WindowType(WindowTypeLabel),
    Custom(String),
}

impl fmt::Display for LabelValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LabelValue::Result(v) => write!(f, "{}", v),
            LabelValue::Operation(v) => write!(f, "{}", v),
            LabelValue::Backend(v) => write!(f, "{}", v),
            LabelValue::CacheLayer(v) => write!(f, "{}", v),
            LabelValue::Strategy(v) => write!(f, "{}", v),
            LabelValue::WindowType(v) => write!(f, "{}", v),
            LabelValue::Custom(v) => write!(f, "{}", v),
        }
    }
}

impl From<ResultLabel> for LabelValue {
    fn from(label: ResultLabel) -> Self {
        LabelValue::Result(label)
    }
}

impl From<OperationLabel> for LabelValue {
    fn from(label: OperationLabel) -> Self {
        LabelValue::Operation(label)
    }
}

impl From<BackendLabel> for LabelValue {
    fn from(label: BackendLabel) -> Self {
        LabelValue::Backend(label)
    }
}

impl From<CacheLayerLabel> for LabelValue {
    fn from(label: CacheLayerLabel) -> Self {
        LabelValue::CacheLayer(label)
    }
}

impl From<StrategyLabel> for LabelValue {
    fn from(label: StrategyLabel) -> Self {
        LabelValue::Strategy(label)
    }
}

impl From<WindowTypeLabel> for LabelValue {
    fn from(label: WindowTypeLabel) -> Self {
        LabelValue::WindowType(label)
    }
}

impl From<String> for LabelValue {
    fn from(s: String) -> Self {
        LabelValue::Custom(s)
    }
}

impl From<&str> for LabelValue {
    fn from(s: &str) -> Self {
        LabelValue::Custom(s.to_string())
    }
}

/// A set of metric labels
#[derive(Debug, Clone, Default)]
pub struct LabelSet {
    labels: Vec<(String, String)>,
}

impl LabelSet {
    /// Create a new empty label set
    pub fn new() -> Self {
        Self {
            labels: Vec::new(),
        }
    }

    /// Add a label to the set
    pub fn with(mut self, name: impl Into<String>, value: impl Into<LabelValue>) -> Self {
        let value = value.into();
        self.labels.push((name.into(), value.to_string()));
        self
    }

    /// Get all labels as a slice
    pub fn as_slice(&self) -> &[(String, String)] {
        &self.labels
    }
}

/// Trait for types that can provide metric labels
pub trait MetricLabels {
    fn labels(&self) -> LabelSet;
}
