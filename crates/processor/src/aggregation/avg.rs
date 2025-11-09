use super::trait_::{Aggregator, ToF64};
use anyhow::anyhow;
use serde::{Deserialize, Serialize};

/// Accumulator for average aggregation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AverageAccumulator {
    sum: f64,
    count: u64,
}

/// Average aggregator - computes the mean of all values
///
/// This aggregator computes the running average using incremental mean calculation.
/// Useful for tracking average latency, average cost per request, etc.
///
/// # Examples
///
/// ```
/// use processor::aggregation::{Aggregator, AverageAggregator};
///
/// let mut agg = AverageAggregator::new();
/// agg.update(10.0).unwrap();
/// agg.update(20.0).unwrap();
/// agg.update(30.0).unwrap();
///
/// assert_eq!(agg.finalize().unwrap(), 20.0);
/// ```
#[derive(Debug, Clone)]
pub struct AverageAggregator<T> {
    sum: f64,
    count: u64,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: ToF64 + Clone> AverageAggregator<T> {
    /// Create a new average aggregator
    pub fn new() -> Self {
        Self {
            sum: 0.0,
            count: 0,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get the current mean value
    pub fn mean(&self) -> Option<f64> {
        if self.count == 0 {
            None
        } else {
            Some(self.sum / self.count as f64)
        }
    }
}

impl<T: ToF64 + Clone> Default for AverageAggregator<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: ToF64 + Clone + Send + Sync + std::fmt::Debug> Aggregator for AverageAggregator<T> {
    type Input = T;
    type Output = f64;
    type Accumulator = AverageAccumulator;

    fn new() -> Self {
        Self::new()
    }

    fn update(&mut self, value: <Self as Aggregator>::Input) -> anyhow::Result<()> {
        self.sum += value.to_f64();
        self.count += 1;
        Ok(())
    }

    fn finalize(&self) -> anyhow::Result<<Self as Aggregator>::Output> {
        if self.count == 0 {
            return Err(anyhow!("Cannot compute average of zero values"));
        }
        Ok(self.sum / self.count as f64)
    }

    fn accumulator(&self) -> <Self as Aggregator>::Accumulator {
        AverageAccumulator {
            sum: self.sum,
            count: self.count,
        }
    }

    fn merge(&mut self, other: <Self as Aggregator>::Accumulator) -> anyhow::Result<()> {
        self.sum += other.sum;
        self.count += other.count;
        Ok(())
    }

    fn reset(&mut self) {
        self.sum = 0.0;
        self.count = 0;
    }

    fn count(&self) -> u64 {
        self.count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_avg_basic() {
        let mut agg = AverageAggregator::<f64>::new();
        assert_eq!(agg.count(), 0);
        assert!(agg.is_empty());

        agg.update(10.0).unwrap();
        agg.update(20.0).unwrap();
        agg.update(30.0).unwrap();

        assert_eq!(agg.count(), 3);
        assert_eq!(agg.finalize().unwrap(), 20.0);
        assert_eq!(agg.mean().unwrap(), 20.0);
    }

    #[test]
    fn test_avg_batch() {
        let mut agg = AverageAggregator::<f64>::new();
        agg.update_batch(&[10.0, 20.0, 30.0, 40.0, 50.0]).unwrap();

        assert_eq!(agg.count(), 5);
        assert_eq!(agg.finalize().unwrap(), 30.0);
    }

    #[test]
    fn test_avg_merge() {
        let mut agg1 = AverageAggregator::<f64>::new();
        agg1.update(10.0).unwrap();
        agg1.update(20.0).unwrap();
        // avg1: (10 + 20) / 2 = 15

        let mut agg2 = AverageAggregator::<f64>::new();
        agg2.update(30.0).unwrap();
        agg2.update(40.0).unwrap();
        agg2.update(50.0).unwrap();
        // avg2: (30 + 40 + 50) / 3 = 40

        let acc2 = agg2.accumulator();
        agg1.merge(acc2).unwrap();

        // merged: (10 + 20 + 30 + 40 + 50) / 5 = 30
        assert_eq!(agg1.finalize().unwrap(), 30.0);
        assert_eq!(agg1.count(), 5);
    }

    #[test]
    fn test_avg_reset() {
        let mut agg = AverageAggregator::<f64>::new();
        agg.update(10.0).unwrap();
        agg.update(20.0).unwrap();

        assert_eq!(agg.finalize().unwrap(), 15.0);

        agg.reset();
        assert_eq!(agg.count(), 0);
        assert!(agg.mean().is_none());
    }

    #[test]
    fn test_avg_serialization() {
        let mut agg = AverageAggregator::<f64>::new();
        agg.update(10.0).unwrap();
        agg.update(20.0).unwrap();

        let acc = agg.accumulator();
        let serialized = serde_json::to_string(&acc).unwrap();
        let deserialized: AverageAccumulator = serde_json::from_str(&serialized).unwrap();

        assert_eq!(acc, deserialized);
        assert_eq!(deserialized.sum, 30.0);
        assert_eq!(deserialized.count, 2);
    }

    #[test]
    fn test_avg_with_different_types() {
        // Test with i64
        let mut agg = AverageAggregator::<i64>::new();
        agg.update(10).unwrap();
        agg.update(20).unwrap();
        agg.update(30).unwrap();
        assert_eq!(agg.finalize().unwrap(), 20.0);

        // Test with f32
        let mut agg = AverageAggregator::<f32>::new();
        agg.update(1.5).unwrap();
        agg.update(2.5).unwrap();
        assert_eq!(agg.finalize().unwrap(), 2.0);
    }

    #[test]
    fn test_avg_empty() {
        let agg = AverageAggregator::<f64>::new();
        assert!(agg.finalize().is_err());
        assert!(agg.mean().is_none());
    }

    #[test]
    fn test_avg_single_value() {
        let mut agg = AverageAggregator::<f64>::new();
        agg.update(42.0).unwrap();
        assert_eq!(agg.finalize().unwrap(), 42.0);
    }

    #[test]
    fn test_avg_negative_values() {
        let mut agg = AverageAggregator::<f64>::new();
        agg.update(-10.0).unwrap();
        agg.update(10.0).unwrap();
        agg.update(20.0).unwrap();

        assert_eq!(agg.finalize().unwrap(), 20.0 / 3.0);
    }
}
