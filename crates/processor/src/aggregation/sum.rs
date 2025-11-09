use super::trait_::{Aggregator, ToF64};

use serde::{Deserialize, Serialize};

/// Accumulator for sum aggregation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SumAccumulator {
    sum: f64,
    count: u64,
}

/// Sum aggregator - computes the sum of all values
///
/// This aggregator computes the running sum of all values processed.
/// Useful for tracking total latency, total cost, total tokens, etc.
///
/// # Examples
///
/// ```
/// use processor::aggregation::{Aggregator, SumAggregator};
///
/// let mut agg = SumAggregator::new();
/// agg.update(1.0).unwrap();
/// agg.update(2.0).unwrap();
/// agg.update(3.0).unwrap();
///
/// assert_eq!(agg.finalize().unwrap(), 6.0);
/// ```
#[derive(Debug, Clone)]
pub struct SumAggregator<T> {
    sum: f64,
    count: u64,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: ToF64 + Clone> SumAggregator<T> {
    /// Create a new sum aggregator
    pub fn new() -> Self {
        Self {
            sum: 0.0,
            count: 0,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T: ToF64 + Clone> Default for SumAggregator<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: ToF64 + Clone + Send + Sync + std::fmt::Debug> Aggregator for SumAggregator<T> {
    type Input = T;
    type Output = f64;
    type Accumulator = SumAccumulator;

    fn new() -> Self {
        Self::new()
    }

    fn update(&mut self, value: <Self as Aggregator>::Input) -> anyhow::Result<()> {
        self.sum += value.to_f64();
        self.count += 1;
        Ok(())
    }

    fn finalize(&self) -> anyhow::Result<<Self as Aggregator>::Output> {
        Ok(self.sum)
    }

    fn accumulator(&self) -> <Self as Aggregator>::Accumulator {
        SumAccumulator {
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
    fn test_sum_basic() {
        let mut agg = SumAggregator::<f64>::new();
        assert_eq!(agg.count(), 0);
        assert!(agg.is_empty());

        agg.update(1.0).unwrap();
        agg.update(2.0).unwrap();
        agg.update(3.0).unwrap();

        assert_eq!(agg.count(), 3);
        assert_eq!(agg.finalize().unwrap(), 6.0);
    }

    #[test]
    fn test_sum_batch() {
        let mut agg = SumAggregator::<f64>::new();
        agg.update_batch(&[1.0, 2.0, 3.0, 4.0, 5.0]).unwrap();

        assert_eq!(agg.count(), 5);
        assert_eq!(agg.finalize().unwrap(), 15.0);
    }

    #[test]
    fn test_sum_merge() {
        let mut agg1 = SumAggregator::<i32>::new();
        agg1.update(1).unwrap();
        agg1.update(2).unwrap();

        let mut agg2 = SumAggregator::<i32>::new();
        agg2.update(3).unwrap();
        agg2.update(4).unwrap();
        agg2.update(5).unwrap();

        let acc2 = agg2.accumulator();
        agg1.merge(acc2).unwrap();

        assert_eq!(agg1.finalize().unwrap(), 15.0);
        assert_eq!(agg1.count(), 5);
    }

    #[test]
    fn test_sum_reset() {
        let mut agg = SumAggregator::<f64>::new();
        agg.update(1.0).unwrap();
        agg.update(2.0).unwrap();

        assert_eq!(agg.finalize().unwrap(), 3.0);

        agg.reset();
        assert_eq!(agg.count(), 0);
        assert_eq!(agg.finalize().unwrap(), 0.0);
    }

    #[test]
    fn test_sum_serialization() {
        let mut agg = SumAggregator::<f64>::new();
        agg.update(1.5).unwrap();
        agg.update(2.5).unwrap();

        let acc = agg.accumulator();
        let serialized = serde_json::to_string(&acc).unwrap();
        let deserialized: SumAccumulator = serde_json::from_str(&serialized).unwrap();

        assert_eq!(acc, deserialized);
        assert_eq!(deserialized.sum, 4.0);
        assert_eq!(deserialized.count, 2);
    }

    #[test]
    fn test_sum_with_different_types() {
        // Test with i64
        let mut agg = SumAggregator::<i64>::new();
        agg.update(10).unwrap();
        agg.update(20).unwrap();
        agg.update(30).unwrap();
        assert_eq!(agg.finalize().unwrap(), 60.0);

        // Test with f32
        let mut agg = SumAggregator::<f32>::new();
        agg.update(1.5).unwrap();
        agg.update(2.5).unwrap();
        assert_eq!(agg.finalize().unwrap(), 4.0);
    }

    #[test]
    fn test_sum_negative_values() {
        let mut agg = SumAggregator::<f64>::new();
        agg.update(10.0).unwrap();
        agg.update(-5.0).unwrap();
        agg.update(3.0).unwrap();

        assert_eq!(agg.finalize().unwrap(), 8.0);
    }

    #[test]
    fn test_sum_empty() {
        let agg = SumAggregator::<f64>::new();
        assert_eq!(agg.finalize().unwrap(), 0.0);
    }
}
