//! Aggregation module for computing statistics over streaming data
//!
//! This module provides a comprehensive set of aggregators for computing various
//! statistics over streaming data. All aggregators support:
//!
//! - **Incremental updates**: Feed values one at a time
//! - **Batch updates**: Feed multiple values at once
//! - **Accumulator merging**: Combine results from distributed processing
//! - **State serialization**: Persist and restore aggregator state
//!
//! # Available Aggregators
//!
//! - [`CountAggregator`]: Count the number of values
//! - [`SumAggregator`]: Sum all values
//! - [`AverageAggregator`]: Compute the mean
//! - [`MinAggregator`]: Track the minimum value
//! - [`MaxAggregator`]: Track the maximum value
//! - [`PercentileAggregator`]: Compute percentiles (p50, p95, p99, etc.)
//! - [`StandardDeviationAggregator`]: Compute standard deviation
//! - [`CompositeAggregator`]: Run multiple aggregations in parallel
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```rust
//! use processor::aggregation::{Aggregator, AverageAggregator};
//!
//! let mut agg = AverageAggregator::new();
//! agg.update(10.0).unwrap();
//! agg.update(20.0).unwrap();
//! agg.update(30.0).unwrap();
//!
//! assert_eq!(agg.finalize().unwrap(), 20.0);
//! ```
//!
//! ## Distributed Processing
//!
//! ```rust
//! use processor::aggregation::{Aggregator, SumAggregator};
//!
//! // Process data in parallel
//! let mut agg1 = SumAggregator::new();
//! agg1.update_batch(&[1.0, 2.0, 3.0]).unwrap();
//!
//! let mut agg2 = SumAggregator::new();
//! agg2.update_batch(&[4.0, 5.0, 6.0]).unwrap();
//!
//! // Merge results
//! let acc2 = agg2.accumulator();
//! agg1.merge(acc2).unwrap();
//!
//! assert_eq!(agg1.finalize().unwrap(), 21.0);
//! ```
//!
//! ## Composite Aggregation
//!
//! ```rust
//! use processor::aggregation::CompositeAggregator;
//!
//! let mut composite = CompositeAggregator::new();
//! composite
//!     .add_count("request_count")
//!     .add_avg("avg_latency")
//!     .add_p95("p95_latency")
//!     .add_stddev("stddev_latency");
//!
//! // Feed data
//! for latency in vec![100.0, 150.0, 200.0, 250.0, 300.0] {
//!     composite.update(latency).unwrap();
//! }
//!
//! // Get all results at once
//! let results = composite.finalize().unwrap();
//! println!("Count: {}", results.get_count("request_count").unwrap());
//! println!("Avg: {:.2}", results.get_value("avg_latency").unwrap());
//! println!("P95: {:.2}", results.get_value("p95_latency").unwrap());
//! ```

// Re-name to avoid collision with trait
mod trait_;

mod count;
mod sum;
mod avg;
mod minmax;
mod percentile;
mod stddev;
mod composite;
pub mod statistics;

// Re-export the trait
pub use trait_::{Aggregator, ToF64};

// Re-export all aggregators
pub use count::{CountAggregator, CountAccumulator};
pub use sum::{SumAggregator, SumAccumulator};
pub use avg::{AverageAggregator, AverageAccumulator};
pub use minmax::{MinAggregator, MaxAggregator, MinAccumulator, MaxAccumulator};
pub use percentile::{PercentileAggregator, PercentileAccumulator};
pub use stddev::{StandardDeviationAggregator, StdDevAccumulator};
pub use composite::{CompositeAggregator, AggregationResult};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multiple_aggregators_together() {
        let data = vec![10.0, 20.0, 30.0, 40.0, 50.0];

        let mut count = CountAggregator::new();
        let mut sum = SumAggregator::new();
        let mut avg = AverageAggregator::new();
        let mut min = MinAggregator::new();
        let mut max = MaxAggregator::new();

        for value in &data {
            count.update(*value).unwrap();
            sum.update(*value).unwrap();
            avg.update(*value).unwrap();
            min.update(*value).unwrap();
            max.update(*value).unwrap();
        }

        assert_eq!(count.finalize().unwrap(), 5);
        assert_eq!(sum.finalize().unwrap(), 150.0);
        assert_eq!(avg.finalize().unwrap(), 30.0);
        assert_eq!(min.finalize().unwrap(), 10.0);
        assert_eq!(max.finalize().unwrap(), 50.0);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut avg = AverageAggregator::<f64>::new();
        avg.update_batch(&[10.0, 20.0, 30.0]).unwrap();

        // Serialize accumulator
        let acc = avg.accumulator();
        let serialized = serde_json::to_string(&acc).unwrap();

        // Deserialize and merge into new aggregator
        let deserialized: AverageAccumulator = serde_json::from_str(&serialized).unwrap();
        let mut new_avg = AverageAggregator::<f64>::new();
        new_avg.merge(deserialized).unwrap();

        assert_eq!(new_avg.finalize().unwrap(), 20.0);
    }

    #[test]
    fn test_distributed_processing_scenario() {
        // Simulate processing data across 3 workers
        let worker1_data = vec![1.0, 2.0, 3.0];
        let worker2_data = vec![4.0, 5.0, 6.0];
        let worker3_data = vec![7.0, 8.0, 9.0];

        // Each worker computes its own aggregation
        let mut agg1 = AverageAggregator::new();
        agg1.update_batch(&worker1_data).unwrap();

        let mut agg2 = AverageAggregator::new();
        agg2.update_batch(&worker2_data).unwrap();

        let mut agg3 = AverageAggregator::new();
        agg3.update_batch(&worker3_data).unwrap();

        // Merge all results
        let mut combined = AverageAggregator::<f64>::new();
        combined.merge(agg1.accumulator()).unwrap();
        combined.merge(agg2.accumulator()).unwrap();
        combined.merge(agg3.accumulator()).unwrap();

        // Result should be average of 1-9
        assert_eq!(combined.finalize().unwrap(), 5.0);
        assert_eq!(combined.count(), 9);
    }
}