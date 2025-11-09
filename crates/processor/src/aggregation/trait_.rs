use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// Core trait for all aggregators
///
/// This trait defines the interface for incremental aggregation with support for:
/// - Incremental updates (feed values one at a time)
/// - Accumulator merging (for distributed processing)
/// - State serialization (for persistence and recovery)
pub trait Aggregator: Send + Sync + Debug {
    /// The type of values this aggregator accepts
    type Input: Clone;

    /// The type of the final aggregation result
    type Output: Clone;

    /// The type of the internal accumulator state (must be serializable)
    type Accumulator: Clone + Serialize + for<'de> Deserialize<'de>;

    /// Create a new aggregator instance
    fn new() -> Self
    where
        Self: Sized;

    /// Update the aggregator with a new value
    fn update(&mut self, value: Self::Input) -> anyhow::Result<()>;

    /// Update with multiple values at once
    fn update_batch(&mut self, values: &[Self::Input]) -> anyhow::Result<()> {
        for value in values {
            self.update(value.clone())?;
        }
        Ok(())
    }

    /// Compute the final aggregation result
    fn finalize(&self) -> anyhow::Result<Self::Output>;

    /// Get the current accumulator state (for serialization/merging)
    fn accumulator(&self) -> Self::Accumulator;

    /// Merge another accumulator into this one (for distributed processing)
    fn merge(&mut self, other: Self::Accumulator) -> anyhow::Result<()>;

    /// Reset the aggregator to initial state
    fn reset(&mut self);

    /// Get the number of values processed so far
    fn count(&self) -> u64;

    /// Check if the aggregator has processed any values
    fn is_empty(&self) -> bool {
        self.count() == 0
    }
}

/// A trait for aggregators that can be cloned
pub trait CloneableAggregator: Aggregator {
    fn clone_box(&self) -> Box<dyn CloneableAggregator<Input = Self::Input, Output = Self::Output, Accumulator = Self::Accumulator>>;
}

/// Helper trait for converting values to f64 for numeric aggregations
pub trait ToF64 {
    fn to_f64(&self) -> f64;
}

impl ToF64 for f64 {
    fn to_f64(&self) -> f64 {
        *self
    }
}

impl ToF64 for f32 {
    fn to_f64(&self) -> f64 {
        *self as f64
    }
}

impl ToF64 for i64 {
    fn to_f64(&self) -> f64 {
        *self as f64
    }
}

impl ToF64 for i32 {
    fn to_f64(&self) -> f64 {
        *self as f64
    }
}

impl ToF64 for u64 {
    fn to_f64(&self) -> f64 {
        *self as f64
    }
}

impl ToF64 for u32 {
    fn to_f64(&self) -> f64 {
        *self as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_f64_conversions() {
        assert_eq!(42_f64.to_f64(), 42.0);
        assert_eq!(42_f32.to_f64(), 42.0);
        assert_eq!(42_i64.to_f64(), 42.0);
        assert_eq!(42_i32.to_f64(), 42.0);
        assert_eq!(42_u64.to_f64(), 42.0);
        assert_eq!(42_u32.to_f64(), 42.0);
    }
}
