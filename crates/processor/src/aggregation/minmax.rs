use super::trait_::{Aggregator, ToF64};
use anyhow::anyhow;
use serde::{Deserialize, Serialize};

/// Accumulator for min aggregation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MinAccumulator {
    min: Option<f64>,
    count: u64,
}

/// Accumulator for max aggregation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MaxAccumulator {
    max: Option<f64>,
    count: u64,
}

/// Min aggregator - tracks the minimum value seen
///
/// This aggregator tracks the minimum value across all values processed.
/// Useful for tracking best-case latency, minimum cost, etc.
///
/// # Examples
///
/// ```
/// use processor::aggregation::{Aggregator, MinAggregator};
///
/// let mut agg = MinAggregator::new();
/// agg.update(30.0).unwrap();
/// agg.update(10.0).unwrap();
/// agg.update(20.0).unwrap();
///
/// assert_eq!(agg.finalize().unwrap(), 10.0);
/// ```
#[derive(Debug, Clone)]
pub struct MinAggregator<T> {
    min: Option<f64>,
    count: u64,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: ToF64 + Clone> MinAggregator<T> {
    /// Create a new min aggregator
    pub fn new() -> Self {
        Self {
            min: None,
            count: 0,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get the current minimum value
    pub fn min(&self) -> Option<f64> {
        self.min
    }
}

impl<T: ToF64 + Clone> Default for MinAggregator<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: ToF64 + Clone + Send + Sync + std::fmt::Debug> Aggregator for MinAggregator<T> {
    type Input = T;
    type Output = f64;
    type Accumulator = MinAccumulator;

    fn new() -> Self {
        Self::new()
    }

    fn update(&mut self, value: <Self as Aggregator>::Input) -> anyhow::Result<()> {
        let val = value.to_f64();
        self.min = Some(match self.min {
            Some(current) => current.min(val),
            None => val,
        });
        self.count += 1;
        Ok(())
    }

    fn finalize(&self) -> anyhow::Result<<Self as Aggregator>::Output> {
        self.min.ok_or_else(|| anyhow!("Cannot compute minimum of zero values"))
    }

    fn accumulator(&self) -> <Self as Aggregator>::Accumulator {
        MinAccumulator {
            min: self.min,
            count: self.count,
        }
    }

    fn merge(&mut self, other: <Self as Aggregator>::Accumulator) -> anyhow::Result<()> {
        self.min = match (self.min, other.min) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        self.count += other.count;
        Ok(())
    }

    fn reset(&mut self) {
        self.min = None;
        self.count = 0;
    }

    fn count(&self) -> u64 {
        self.count
    }
}

/// Max aggregator - tracks the maximum value seen
///
/// This aggregator tracks the maximum value across all values processed.
/// Useful for tracking worst-case latency, maximum cost, peak usage, etc.
///
/// # Examples
///
/// ```
/// use processor::aggregation::{Aggregator, MaxAggregator};
///
/// let mut agg = MaxAggregator::new();
/// agg.update(10.0).unwrap();
/// agg.update(30.0).unwrap();
/// agg.update(20.0).unwrap();
///
/// assert_eq!(agg.finalize().unwrap(), 30.0);
/// ```
#[derive(Debug, Clone)]
pub struct MaxAggregator<T> {
    max: Option<f64>,
    count: u64,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: ToF64 + Clone> MaxAggregator<T> {
    /// Create a new max aggregator
    pub fn new() -> Self {
        Self {
            max: None,
            count: 0,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get the current maximum value
    pub fn max(&self) -> Option<f64> {
        self.max
    }
}

impl<T: ToF64 + Clone> Default for MaxAggregator<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: ToF64 + Clone + Send + Sync + std::fmt::Debug> Aggregator for MaxAggregator<T> {
    type Input = T;
    type Output = f64;
    type Accumulator = MaxAccumulator;

    fn new() -> Self {
        Self::new()
    }

    fn update(&mut self, value: <Self as Aggregator>::Input) -> anyhow::Result<()> {
        let val = value.to_f64();
        self.max = Some(match self.max {
            Some(current) => current.max(val),
            None => val,
        });
        self.count += 1;
        Ok(())
    }

    fn finalize(&self) -> anyhow::Result<<Self as Aggregator>::Output> {
        self.max.ok_or_else(|| anyhow!("Cannot compute maximum of zero values"))
    }

    fn accumulator(&self) -> <Self as Aggregator>::Accumulator {
        MaxAccumulator {
            max: self.max,
            count: self.count,
        }
    }

    fn merge(&mut self, other: <Self as Aggregator>::Accumulator) -> anyhow::Result<()> {
        self.max = match (self.max, other.max) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        self.count += other.count;
        Ok(())
    }

    fn reset(&mut self) {
        self.max = None;
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
    fn test_min_basic() {
        let mut agg = MinAggregator::<f64>::new();
        assert!(agg.is_empty());

        agg.update(30.0).unwrap();
        agg.update(10.0).unwrap();
        agg.update(20.0).unwrap();

        assert_eq!(agg.count(), 3);
        assert_eq!(agg.finalize().unwrap(), 10.0);
        assert_eq!(agg.min().unwrap(), 10.0);
    }

    #[test]
    fn test_max_basic() {
        let mut agg = MaxAggregator::<f64>::new();
        assert!(agg.is_empty());

        agg.update(10.0).unwrap();
        agg.update(30.0).unwrap();
        agg.update(20.0).unwrap();

        assert_eq!(agg.count(), 3);
        assert_eq!(agg.finalize().unwrap(), 30.0);
        assert_eq!(agg.max().unwrap(), 30.0);
    }

    #[test]
    fn test_min_batch() {
        let mut agg = MinAggregator::<f64>::new();
        agg.update_batch(&[50.0, 10.0, 30.0, 20.0, 40.0]).unwrap();

        assert_eq!(agg.finalize().unwrap(), 10.0);
    }

    #[test]
    fn test_max_batch() {
        let mut agg = MaxAggregator::<f64>::new();
        agg.update_batch(&[10.0, 50.0, 30.0, 20.0, 40.0]).unwrap();

        assert_eq!(agg.finalize().unwrap(), 50.0);
    }

    #[test]
    fn test_min_merge() {
        let mut agg1 = MinAggregator::<f64>::new();
        agg1.update(30.0).unwrap();
        agg1.update(20.0).unwrap();

        let mut agg2 = MinAggregator::<f64>::new();
        agg2.update(10.0).unwrap();
        agg2.update(40.0).unwrap();

        let acc2 = agg2.accumulator();
        agg1.merge(acc2).unwrap();

        assert_eq!(agg1.finalize().unwrap(), 10.0);
        assert_eq!(agg1.count(), 4);
    }

    #[test]
    fn test_max_merge() {
        let mut agg1 = MaxAggregator::<f64>::new();
        agg1.update(20.0).unwrap();
        agg1.update(30.0).unwrap();

        let mut agg2 = MaxAggregator::<f64>::new();
        agg2.update(50.0).unwrap();
        agg2.update(10.0).unwrap();

        let acc2 = agg2.accumulator();
        agg1.merge(acc2).unwrap();

        assert_eq!(agg1.finalize().unwrap(), 50.0);
        assert_eq!(agg1.count(), 4);
    }

    #[test]
    fn test_min_reset() {
        let mut agg = MinAggregator::<f64>::new();
        agg.update(10.0).unwrap();
        agg.update(20.0).unwrap();

        assert_eq!(agg.finalize().unwrap(), 10.0);

        agg.reset();
        assert_eq!(agg.count(), 0);
        assert!(agg.min().is_none());
    }

    #[test]
    fn test_max_reset() {
        let mut agg = MaxAggregator::<f64>::new();
        agg.update(10.0).unwrap();
        agg.update(20.0).unwrap();

        assert_eq!(agg.finalize().unwrap(), 20.0);

        agg.reset();
        assert_eq!(agg.count(), 0);
        assert!(agg.max().is_none());
    }

    #[test]
    fn test_min_serialization() {
        let mut agg = MinAggregator::<f64>::new();
        agg.update(10.0).unwrap();
        agg.update(20.0).unwrap();

        let acc = agg.accumulator();
        let serialized = serde_json::to_string(&acc).unwrap();
        let deserialized: MinAccumulator = serde_json::from_str(&serialized).unwrap();

        assert_eq!(acc, deserialized);
        assert_eq!(deserialized.min.unwrap(), 10.0);
    }

    #[test]
    fn test_max_serialization() {
        let mut agg = MaxAggregator::<f64>::new();
        agg.update(10.0).unwrap();
        agg.update(20.0).unwrap();

        let acc = agg.accumulator();
        let serialized = serde_json::to_string(&acc).unwrap();
        let deserialized: MaxAccumulator = serde_json::from_str(&serialized).unwrap();

        assert_eq!(acc, deserialized);
        assert_eq!(deserialized.max.unwrap(), 20.0);
    }

    #[test]
    fn test_min_empty() {
        let agg = MinAggregator::<f64>::new();
        assert!(agg.finalize().is_err());
        assert!(agg.min().is_none());
    }

    #[test]
    fn test_max_empty() {
        let agg = MaxAggregator::<f64>::new();
        assert!(agg.finalize().is_err());
        assert!(agg.max().is_none());
    }

    #[test]
    fn test_min_negative() {
        let mut agg = MinAggregator::<f64>::new();
        agg.update(-10.0).unwrap();
        agg.update(5.0).unwrap();
        agg.update(-20.0).unwrap();

        assert_eq!(agg.finalize().unwrap(), -20.0);
    }

    #[test]
    fn test_max_negative() {
        let mut agg = MaxAggregator::<f64>::new();
        agg.update(-10.0).unwrap();
        agg.update(-5.0).unwrap();
        agg.update(-20.0).unwrap();

        assert_eq!(agg.finalize().unwrap(), -5.0);
    }

    #[test]
    fn test_min_with_different_types() {
        let mut agg = MinAggregator::<i32>::new();
        agg.update(30).unwrap();
        agg.update(10).unwrap();
        agg.update(20).unwrap();
        assert_eq!(agg.finalize().unwrap(), 10.0);
    }

    #[test]
    fn test_max_with_different_types() {
        let mut agg = MaxAggregator::<i32>::new();
        agg.update(10).unwrap();
        agg.update(30).unwrap();
        agg.update(20).unwrap();
        assert_eq!(agg.finalize().unwrap(), 30.0);
    }
}
