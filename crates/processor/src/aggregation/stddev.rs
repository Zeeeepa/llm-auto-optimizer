use super::trait_::{Aggregator, ToF64};
use anyhow::anyhow;
use serde::{Deserialize, Serialize};

/// Accumulator for standard deviation aggregation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StdDevAccumulator {
    count: u64,
    mean: f64,
    m2: f64, // Sum of squared differences from mean
}

/// Standard deviation aggregator - computes sample standard deviation
///
/// This aggregator uses Welford's online algorithm for numerical stability.
/// Computes the sample standard deviation (using Bessel's correction, n-1).
///
/// Useful for measuring variability in latency, cost, throughput, etc.
///
/// # Examples
///
/// ```
/// use processor::aggregation::{Aggregator, StandardDeviationAggregator};
///
/// let mut agg = StandardDeviationAggregator::new();
/// agg.update(10.0).unwrap();
/// agg.update(12.0).unwrap();
/// agg.update(14.0).unwrap();
/// agg.update(16.0).unwrap();
/// agg.update(18.0).unwrap();
///
/// let stddev = agg.finalize().unwrap();
/// assert!((stddev - 3.1622).abs() < 0.01); // ~3.16
/// ```
#[derive(Debug, Clone)]
pub struct StandardDeviationAggregator<T> {
    count: u64,
    mean: f64,
    m2: f64,
    sample: bool, // If true, use sample std dev (n-1), otherwise population (n)
    _phantom: std::marker::PhantomData<T>,
}

impl<T: ToF64 + Clone> StandardDeviationAggregator<T> {
    /// Create a new sample standard deviation aggregator (uses n-1)
    pub fn new() -> Self {
        Self {
            count: 0,
            mean: 0.0,
            m2: 0.0,
            sample: true,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Create a population standard deviation aggregator (uses n)
    pub fn population() -> Self {
        Self {
            count: 0,
            mean: 0.0,
            m2: 0.0,
            sample: false,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get the current variance
    pub fn variance(&self) -> Option<f64> {
        if self.count == 0 {
            return None;
        }

        let divisor = if self.sample {
            if self.count < 2 {
                return None;
            }
            self.count - 1
        } else {
            self.count
        } as f64;

        Some(self.m2 / divisor)
    }

    /// Get the current standard deviation
    pub fn std_dev(&self) -> Option<f64> {
        self.variance().map(|v| v.sqrt())
    }

    /// Get the current mean
    pub fn mean(&self) -> Option<f64> {
        if self.count == 0 {
            None
        } else {
            Some(self.mean)
        }
    }
}

impl<T: ToF64 + Clone> Default for StandardDeviationAggregator<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: ToF64 + Clone + Send + Sync + std::fmt::Debug> Aggregator for StandardDeviationAggregator<T> {
    type Input = T;
    type Output = f64;
    type Accumulator = StdDevAccumulator;

    fn new() -> Self {
        Self::new()
    }

    fn update(&mut self, value: <Self as Aggregator>::Input) -> anyhow::Result<()> {
        let x = value.to_f64();
        self.count += 1;

        // Welford's online algorithm
        let delta = x - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = x - self.mean;
        self.m2 += delta * delta2;

        Ok(())
    }

    fn finalize(&self) -> anyhow::Result<<Self as Aggregator>::Output> {
        self.std_dev()
            .ok_or_else(|| anyhow!("Cannot compute standard deviation with less than 2 values"))
    }

    fn accumulator(&self) -> <Self as Aggregator>::Accumulator {
        StdDevAccumulator {
            count: self.count,
            mean: self.mean,
            m2: self.m2,
        }
    }

    fn merge(&mut self, other: <Self as Aggregator>::Accumulator) -> anyhow::Result<()> {
        if other.count == 0 {
            return Ok(());
        }

        if self.count == 0 {
            self.count = other.count;
            self.mean = other.mean;
            self.m2 = other.m2;
            return Ok(());
        }

        // Parallel variance formula (Chan et al.)
        let delta = other.mean - self.mean;
        let total_count = self.count + other.count;

        self.m2 = self.m2 + other.m2
            + delta * delta * (self.count as f64 * other.count as f64) / total_count as f64;

        self.mean = (self.count as f64 * self.mean + other.count as f64 * other.mean)
            / total_count as f64;

        self.count = total_count;

        Ok(())
    }

    fn reset(&mut self) {
        self.count = 0;
        self.mean = 0.0;
        self.m2 = 0.0;
    }

    fn count(&self) -> u64 {
        self.count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stddev_basic() {
        let mut agg = StandardDeviationAggregator::<f64>::new();

        // Values: 10, 12, 14, 16, 18
        // Mean: 14
        // Variance: ((4 + 4 + 0 + 4 + 16) / 4) = 10
        // StdDev: sqrt(10) = 3.162...
        agg.update(10.0).unwrap();
        agg.update(12.0).unwrap();
        agg.update(14.0).unwrap();
        agg.update(16.0).unwrap();
        agg.update(18.0).unwrap();

        assert_eq!(agg.count(), 5);
        assert!((agg.mean().unwrap() - 14.0).abs() < 0.01);
        assert!((agg.variance().unwrap() - 10.0).abs() < 0.01);
        assert!((agg.std_dev().unwrap() - 3.162).abs() < 0.01);
        assert!((agg.finalize().unwrap() - 3.162).abs() < 0.01);
    }

    #[test]
    fn test_stddev_population() {
        let mut agg = StandardDeviationAggregator::<f64>::population();

        agg.update(10.0).unwrap();
        agg.update(12.0).unwrap();
        agg.update(14.0).unwrap();
        agg.update(16.0).unwrap();
        agg.update(18.0).unwrap();

        // Population variance: 28/5 = 5.6
        // Population stddev: sqrt(5.6) = 2.828...
        assert!((agg.variance().unwrap() - 8.0).abs() < 0.01);
        assert!((agg.std_dev().unwrap() - 2.828).abs() < 0.01);
    }

    #[test]
    fn test_stddev_batch() {
        let mut agg = StandardDeviationAggregator::<f64>::new();
        agg.update_batch(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]).unwrap();

        assert_eq!(agg.count(), 8);
        assert!((agg.mean().unwrap() - 5.0).abs() < 0.01);
        assert!((agg.std_dev().unwrap() - 2.138).abs() < 0.01);
    }

    #[test]
    fn test_stddev_merge() {
        let mut agg1 = StandardDeviationAggregator::<f64>::new();
        agg1.update(10.0).unwrap();
        agg1.update(12.0).unwrap();
        agg1.update(14.0).unwrap();

        let mut agg2 = StandardDeviationAggregator::<f64>::new();
        agg2.update(16.0).unwrap();
        agg2.update(18.0).unwrap();

        let acc2 = agg2.accumulator();
        agg1.merge(acc2).unwrap();

        assert_eq!(agg1.count(), 5);
        assert!((agg1.mean().unwrap() - 14.0).abs() < 0.01);
        assert!((agg1.std_dev().unwrap() - 3.162).abs() < 0.01);
    }

    #[test]
    fn test_stddev_reset() {
        let mut agg = StandardDeviationAggregator::<f64>::new();
        agg.update(10.0).unwrap();
        agg.update(20.0).unwrap();
        agg.update(30.0).unwrap();

        assert_eq!(agg.count(), 3);

        agg.reset();
        assert_eq!(agg.count(), 0);
        assert!(agg.mean().is_none());
        assert!(agg.variance().is_none());
        assert!(agg.std_dev().is_none());
    }

    #[test]
    fn test_stddev_serialization() {
        let mut agg = StandardDeviationAggregator::<f64>::new();
        agg.update(10.0).unwrap();
        agg.update(20.0).unwrap();

        let acc = agg.accumulator();
        let serialized = serde_json::to_string(&acc).unwrap();
        let deserialized: StdDevAccumulator = serde_json::from_str(&serialized).unwrap();

        assert_eq!(acc, deserialized);
    }

    #[test]
    fn test_stddev_empty() {
        let agg = StandardDeviationAggregator::<f64>::new();
        assert!(agg.finalize().is_err());
        assert!(agg.mean().is_none());
        assert!(agg.variance().is_none());
    }

    #[test]
    fn test_stddev_single_value() {
        let mut agg = StandardDeviationAggregator::<f64>::new();
        agg.update(42.0).unwrap();

        // Sample stddev requires at least 2 values
        assert!(agg.finalize().is_err());
        assert_eq!(agg.mean().unwrap(), 42.0);
    }

    #[test]
    fn test_stddev_two_values() {
        let mut agg = StandardDeviationAggregator::<f64>::new();
        agg.update(10.0).unwrap();
        agg.update(20.0).unwrap();

        // Mean: 15, variance: 50, stddev: 7.071
        assert!((agg.mean().unwrap() - 15.0).abs() < 0.01);
        assert!((agg.variance().unwrap() - 50.0).abs() < 0.01);
        assert!((agg.std_dev().unwrap() - 7.071).abs() < 0.01);
    }

    #[test]
    fn test_stddev_zero_variance() {
        let mut agg = StandardDeviationAggregator::<f64>::new();
        agg.update(5.0).unwrap();
        agg.update(5.0).unwrap();
        agg.update(5.0).unwrap();

        assert_eq!(agg.mean().unwrap(), 5.0);
        assert_eq!(agg.variance().unwrap(), 0.0);
        assert_eq!(agg.std_dev().unwrap(), 0.0);
    }

    #[test]
    fn test_stddev_negative_values() {
        let mut agg = StandardDeviationAggregator::<f64>::new();
        agg.update(-10.0).unwrap();
        agg.update(-5.0).unwrap();
        agg.update(0.0).unwrap();
        agg.update(5.0).unwrap();
        agg.update(10.0).unwrap();

        assert!((agg.mean().unwrap() - 0.0).abs() < 0.01);
        assert!((agg.std_dev().unwrap() - 7.905).abs() < 0.01);
    }

    #[test]
    fn test_stddev_with_integer_types() {
        let mut agg = StandardDeviationAggregator::<i32>::new();
        agg.update(10).unwrap();
        agg.update(12).unwrap();
        agg.update(14).unwrap();
        agg.update(16).unwrap();
        agg.update(18).unwrap();

        assert!((agg.mean().unwrap() - 14.0).abs() < 0.01);
        assert!((agg.std_dev().unwrap() - 3.162).abs() < 0.01);
    }
}
