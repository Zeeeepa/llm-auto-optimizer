use super::trait_::Aggregator;

use serde::{Deserialize, Serialize};

/// Accumulator for count aggregation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CountAccumulator {
    count: u64,
}

/// Count aggregator - counts the number of values
///
/// This is the simplest aggregator, it just counts how many values have been processed.
/// Useful for tracking event counts, request counts, etc.
///
/// # Examples
///
/// ```
/// use processor::aggregation::{Aggregator, CountAggregator};
///
/// let mut agg = CountAggregator::new();
/// agg.update(1.0).unwrap();
/// agg.update(2.0).unwrap();
/// agg.update(3.0).unwrap();
///
/// assert_eq!(agg.finalize().unwrap(), 3);
/// ```
#[derive(Debug, Clone)]
pub struct CountAggregator<T> {
    count: u64,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Clone> CountAggregator<T> {
    /// Create a new count aggregator
    pub fn new() -> Self {
        Self {
            count: 0,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T: Clone> Default for CountAggregator<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + Send + Sync + std::fmt::Debug> Aggregator for CountAggregator<T> {
    type Input = T;
    type Output = u64;
    type Accumulator = CountAccumulator;

    fn new() -> Self {
        Self::new()
    }

    fn update(&mut self, _value: <Self as Aggregator>::Input) -> anyhow::Result<()> {
        self.count += 1;
        Ok(())
    }

    fn finalize(&self) -> anyhow::Result<<Self as Aggregator>::Output> {
        Ok(self.count)
    }

    fn accumulator(&self) -> <Self as Aggregator>::Accumulator {
        CountAccumulator {
            count: self.count,
        }
    }

    fn merge(&mut self, other: <Self as Aggregator>::Accumulator) -> anyhow::Result<()> {
        self.count += other.count;
        Ok(())
    }

    fn reset(&mut self) {
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
    fn test_count_basic() {
        let mut agg = CountAggregator::<f64>::new();
        assert_eq!(agg.count(), 0);
        assert!(agg.is_empty());

        agg.update(1.0).unwrap();
        agg.update(2.0).unwrap();
        agg.update(3.0).unwrap();

        assert_eq!(agg.count(), 3);
        assert!(!agg.is_empty());
        assert_eq!(agg.finalize().unwrap(), 3);
    }

    #[test]
    fn test_count_batch() {
        let mut agg = CountAggregator::<f64>::new();
        agg.update_batch(&[1.0, 2.0, 3.0, 4.0, 5.0]).unwrap();

        assert_eq!(agg.count(), 5);
        assert_eq!(agg.finalize().unwrap(), 5);
    }

    #[test]
    fn test_count_merge() {
        let mut agg1 = CountAggregator::<i32>::new();
        agg1.update(1).unwrap();
        agg1.update(2).unwrap();

        let mut agg2 = CountAggregator::<i32>::new();
        agg2.update(3).unwrap();
        agg2.update(4).unwrap();
        agg2.update(5).unwrap();

        let acc2 = agg2.accumulator();
        agg1.merge(acc2).unwrap();

        assert_eq!(agg1.finalize().unwrap(), 5);
    }

    #[test]
    fn test_count_reset() {
        let mut agg = CountAggregator::<f64>::new();
        agg.update(1.0).unwrap();
        agg.update(2.0).unwrap();

        assert_eq!(agg.count(), 2);

        agg.reset();
        assert_eq!(agg.count(), 0);
        assert!(agg.is_empty());
    }

    #[test]
    fn test_count_serialization() {
        let mut agg = CountAggregator::<f64>::new();
        agg.update(1.0).unwrap();
        agg.update(2.0).unwrap();

        let acc = agg.accumulator();
        let serialized = serde_json::to_string(&acc).unwrap();
        let deserialized: CountAccumulator = serde_json::from_str(&serialized).unwrap();

        assert_eq!(acc, deserialized);
        assert_eq!(deserialized.count, 2);
    }

    #[test]
    fn test_count_with_different_types() {
        // Test with strings
        let mut agg = CountAggregator::<String>::new();
        agg.update("hello".to_string()).unwrap();
        agg.update("world".to_string()).unwrap();
        assert_eq!(agg.finalize().unwrap(), 2);

        // Test with tuples
        let mut agg = CountAggregator::<(i32, String)>::new();
        agg.update((1, "a".to_string())).unwrap();
        agg.update((2, "b".to_string())).unwrap();
        assert_eq!(agg.finalize().unwrap(), 2);
    }
}
