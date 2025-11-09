use super::trait_::{Aggregator, ToF64};
use anyhow::anyhow;
use serde::{Deserialize, Serialize};

/// Accumulator for percentile aggregation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PercentileAccumulator {
    values: Vec<f64>,
    percentile: f64,
}

/// Percentile aggregator - computes percentiles (p50, p95, p99, etc.)
///
/// This aggregator computes the specified percentile of all values.
/// Uses a simple but memory-efficient approach by storing all values.
/// For very large datasets, consider using approximate percentile algorithms.
///
/// Common percentiles:
/// - p50 (median): 50th percentile
/// - p95: 95th percentile (useful for SLAs)
/// - p99: 99th percentile (worst-case performance)
///
/// # Examples
///
/// ```
/// use processor::aggregation::{Aggregator, PercentileAggregator};
///
/// // Calculate p95 latency
/// let mut agg = PercentileAggregator::p95();
/// for i in 1..=100 {
///     agg.update(i as f64).unwrap();
/// }
///
/// let p95 = agg.finalize().unwrap();
/// assert!((p95 - 95.0).abs() < 1.0);
/// ```
#[derive(Debug, Clone)]
pub struct PercentileAggregator<T> {
    values: Vec<f64>,
    percentile: f64,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: ToF64 + Clone> PercentileAggregator<T> {
    /// Create a new percentile aggregator with a custom percentile (0.0 to 100.0)
    pub fn new(percentile: f64) -> anyhow::Result<Self> {
        if !(0.0..=100.0).contains(&percentile) {
            return Err(anyhow!("Percentile must be between 0 and 100"));
        }
        Ok(Self {
            values: Vec::new(),
            percentile,
            _phantom: std::marker::PhantomData,
        })
    }

    /// Create a p50 (median) aggregator
    pub fn p50() -> Self {
        Self {
            values: Vec::new(),
            percentile: 50.0,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Create a p95 aggregator
    pub fn p95() -> Self {
        Self {
            values: Vec::new(),
            percentile: 95.0,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Create a p99 aggregator
    pub fn p99() -> Self {
        Self {
            values: Vec::new(),
            percentile: 99.0,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Create a p99.9 aggregator
    pub fn p999() -> Self {
        Self {
            values: Vec::new(),
            percentile: 99.9,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Calculate percentile from sorted values using linear interpolation
    fn calculate_percentile(sorted_values: &[f64], percentile: f64) -> Option<f64> {
        if sorted_values.is_empty() {
            return None;
        }

        if sorted_values.len() == 1 {
            return Some(sorted_values[0]);
        }

        let rank = (percentile / 100.0) * (sorted_values.len() - 1) as f64;
        let lower_idx = rank.floor() as usize;
        let upper_idx = rank.ceil() as usize;

        if lower_idx == upper_idx {
            Some(sorted_values[lower_idx])
        } else {
            let fraction = rank - lower_idx as f64;
            Some(sorted_values[lower_idx] * (1.0 - fraction)
                + sorted_values[upper_idx] * fraction)
        }
    }
}

impl<T: ToF64 + Clone> Default for PercentileAggregator<T> {
    fn default() -> Self {
        Self::p50()
    }
}

impl<T: ToF64 + Clone + Send + Sync + std::fmt::Debug> Aggregator for PercentileAggregator<T> {
    type Input = T;
    type Output = f64;
    type Accumulator = PercentileAccumulator;

    fn new() -> Self {
        Self::p50()
    }

    fn update(&mut self, value: <Self as Aggregator>::Input) -> anyhow::Result<()> {
        self.values.push(value.to_f64());
        Ok(())
    }

    fn finalize(&self) -> anyhow::Result<<Self as Aggregator>::Output> {
        if self.values.is_empty() {
            return Err(anyhow!("Cannot compute percentile of zero values"));
        }

        let mut sorted = self.values.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        Self::calculate_percentile(&sorted, self.percentile)
            .ok_or_else(|| anyhow!("Failed to calculate percentile"))
    }

    fn accumulator(&self) -> <Self as Aggregator>::Accumulator {
        PercentileAccumulator {
            values: self.values.clone(),
            percentile: self.percentile,
        }
    }

    fn merge(&mut self, other: <Self as Aggregator>::Accumulator) -> anyhow::Result<()> {
        if (self.percentile - other.percentile).abs() > f64::EPSILON {
            return Err(anyhow!(
                "Cannot merge percentile aggregators with different percentiles: {} vs {}",
                self.percentile,
                other.percentile
            ));
        }
        self.values.extend(other.values);
        Ok(())
    }

    fn reset(&mut self) {
        self.values.clear();
    }

    fn count(&self) -> u64 {
        self.values.len() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percentile_p50() {
        let mut agg = PercentileAggregator::<f64>::p50();
        for i in 1..=100 {
            agg.update(i as f64).unwrap();
        }

        let p50 = agg.finalize().unwrap();
        assert!((p50 - 50.5).abs() < 0.1);
    }

    #[test]
    fn test_percentile_p95() {
        let mut agg = PercentileAggregator::<f64>::p95();
        for i in 1..=100 {
            agg.update(i as f64).unwrap();
        }

        let p95 = agg.finalize().unwrap();
        assert!((p95 - 95.05).abs() < 0.1);
    }

    #[test]
    fn test_percentile_p99() {
        let mut agg = PercentileAggregator::<f64>::p99();
        for i in 1..=100 {
            agg.update(i as f64).unwrap();
        }

        let p99 = agg.finalize().unwrap();
        assert!((p99 - 99.01).abs() < 0.1);
    }

    #[test]
    fn test_percentile_custom() {
        let mut agg = PercentileAggregator::<f64>::new(75.0).unwrap();
        for i in 1..=100 {
            agg.update(i as f64).unwrap();
        }

        let p75 = agg.finalize().unwrap();
        assert!((p75 - 75.25).abs() < 0.5);
    }

    #[test]
    fn test_percentile_batch() {
        let mut agg = PercentileAggregator::<f64>::p50();
        let values: Vec<f64> = (1..=100).map(|i| i as f64).collect();
        agg.update_batch(&values).unwrap();

        let p50 = agg.finalize().unwrap();
        assert!((p50 - 50.5).abs() < 0.1);
    }

    #[test]
    fn test_percentile_merge() {
        let mut agg1 = PercentileAggregator::<f64>::p50();
        for i in 1..=50 {
            agg1.update(i as f64).unwrap();
        }

        let mut agg2 = PercentileAggregator::<f64>::p50();
        for i in 51..=100 {
            agg2.update(i as f64).unwrap();
        }

        let acc2 = agg2.accumulator();
        agg1.merge(acc2).unwrap();

        let p50 = agg1.finalize().unwrap();
        assert!((p50 - 50.5).abs() < 0.1);
        assert_eq!(agg1.count(), 100);
    }

    #[test]
    fn test_percentile_merge_different_percentiles_fails() {
        let mut agg1 = PercentileAggregator::<f64>::p50();
        agg1.update(1.0).unwrap();

        let mut agg2 = PercentileAggregator::<f64>::p95();
        agg2.update(2.0).unwrap();

        let acc2 = agg2.accumulator();
        assert!(agg1.merge(acc2).is_err());
    }

    #[test]
    fn test_percentile_reset() {
        let mut agg = PercentileAggregator::<f64>::p50();
        agg.update(1.0).unwrap();
        agg.update(2.0).unwrap();

        assert_eq!(agg.count(), 2);

        agg.reset();
        assert_eq!(agg.count(), 0);
        assert!(agg.finalize().is_err());
    }

    #[test]
    fn test_percentile_serialization() {
        let mut agg = PercentileAggregator::<f64>::p95();
        agg.update(10.0).unwrap();
        agg.update(20.0).unwrap();

        let acc = agg.accumulator();
        let serialized = serde_json::to_string(&acc).unwrap();
        let deserialized: PercentileAccumulator = serde_json::from_str(&serialized).unwrap();

        assert_eq!(acc, deserialized);
        assert_eq!(deserialized.percentile, 95.0);
        assert_eq!(deserialized.values.len(), 2);
    }

    #[test]
    fn test_percentile_empty() {
        let agg = PercentileAggregator::<f64>::p50();
        assert!(agg.finalize().is_err());
    }

    #[test]
    fn test_percentile_single_value() {
        let mut agg = PercentileAggregator::<f64>::p95();
        agg.update(42.0).unwrap();

        assert_eq!(agg.finalize().unwrap(), 42.0);
    }

    #[test]
    fn test_percentile_invalid_value() {
        assert!(PercentileAggregator::<f64>::new(-1.0).is_err());
        assert!(PercentileAggregator::<f64>::new(101.0).is_err());
    }

    #[test]
    fn test_percentile_edge_cases() {
        // p0 should give minimum
        let mut agg = PercentileAggregator::<f64>::new(0.0).unwrap();
        agg.update_batch(&[10.0, 20.0, 30.0]).unwrap();
        assert_eq!(agg.finalize().unwrap(), 10.0);

        // p100 should give maximum
        let mut agg = PercentileAggregator::<f64>::new(100.0).unwrap();
        agg.update_batch(&[10.0, 20.0, 30.0]).unwrap();
        assert_eq!(agg.finalize().unwrap(), 30.0);
    }

    #[test]
    fn test_percentile_with_duplicates() {
        let mut agg = PercentileAggregator::<f64>::p50();
        agg.update_batch(&[1.0, 2.0, 2.0, 2.0, 3.0]).unwrap();

        // Median of [1, 2, 2, 2, 3] should be 2.0
        assert_eq!(agg.finalize().unwrap(), 2.0);
    }

    #[test]
    fn test_percentile_unsorted_input() {
        let mut agg = PercentileAggregator::<f64>::p50();
        agg.update_batch(&[50.0, 10.0, 30.0, 20.0, 40.0]).unwrap();

        // Median of [10, 20, 30, 40, 50] should be 30.0
        assert_eq!(agg.finalize().unwrap(), 30.0);
    }
}
