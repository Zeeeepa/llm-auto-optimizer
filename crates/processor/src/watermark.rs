//! Watermarking system for handling late events in stream processing
//!
//! This module provides watermarking capabilities to handle out-of-order and late-arriving events
//! in the stream processing pipeline. Watermarks represent a timestamp threshold indicating that
//! all events with timestamps below this threshold have been processed.
//!
//! # Overview
//!
//! The watermarking system supports:
//! - Bounded out-of-orderness handling
//! - Per-partition watermark tracking
//! - Idle partition detection
//! - Periodic and punctuated watermark generation
//! - Late event detection and handling
//!
//! # Example
//!
//! ```rust
//! use processor::watermark::{Watermark, BoundedOutOfOrdernessWatermark, WatermarkGenerator};
//! use std::time::Duration;
//!
//! let mut generator = BoundedOutOfOrdernessWatermark::new(
//!     Duration::from_secs(10),  // 10 second max out-of-orderness
//!     Some(Duration::from_secs(60)) // 60 second idle timeout
//! );
//!
//! // Process an event with timestamp 1000ms
//! if let Some(watermark) = generator.on_event(1000, 0) {
//!     println!("New watermark: {:?}", watermark);
//! }
//! ```

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::cmp::max;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{debug, trace, warn};

/// Represents a watermark timestamp
///
/// Watermarks indicate that all events with timestamps less than or equal to the watermark
/// have been processed. This allows the system to trigger time-based operations and
/// detect late-arriving events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Watermark {
    /// The watermark timestamp in milliseconds since epoch
    pub timestamp: i64,
}

impl Watermark {
    /// Creates a new watermark with the given timestamp
    pub fn new(timestamp: i64) -> Self {
        Self { timestamp }
    }

    /// Creates a watermark from a DateTime
    pub fn from_datetime(dt: DateTime<Utc>) -> Self {
        Self {
            timestamp: dt.timestamp_millis(),
        }
    }

    /// Converts the watermark to a DateTime
    pub fn to_datetime(&self) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(self.timestamp)
            .unwrap_or_else(|| Utc::now())
    }

    /// Returns the minimum possible watermark (beginning of time)
    pub fn min() -> Self {
        Self { timestamp: i64::MIN }
    }

    /// Returns the maximum possible watermark (end of time)
    pub fn max() -> Self {
        Self { timestamp: i64::MAX }
    }

    /// Checks if this watermark is before the given timestamp
    pub fn is_before(&self, timestamp: i64) -> bool {
        self.timestamp < timestamp
    }

    /// Checks if this watermark is after the given timestamp
    pub fn is_after(&self, timestamp: i64) -> bool {
        self.timestamp > timestamp
    }

    /// Returns true if this is the minimum watermark
    pub fn is_min(&self) -> bool {
        self.timestamp == i64::MIN
    }

    /// Returns true if this is the maximum watermark
    pub fn is_max(&self) -> bool {
        self.timestamp == i64::MAX
    }
}

impl Default for Watermark {
    fn default() -> Self {
        Self::min()
    }
}

impl std::fmt::Display for Watermark {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Watermark({})", self.to_datetime())
    }
}

/// Trait for generating watermarks from event streams
pub trait WatermarkGenerator: Send + Sync {
    /// Called when a new event arrives
    ///
    /// # Arguments
    /// * `timestamp` - The event timestamp in milliseconds
    /// * `partition` - The partition ID the event belongs to
    ///
    /// # Returns
    /// An optional watermark if one should be emitted
    fn on_event(&mut self, timestamp: i64, partition: u32) -> Option<Watermark>;

    /// Called periodically to check if a watermark should be emitted
    ///
    /// # Returns
    /// An optional watermark if one should be emitted
    fn on_periodic_check(&mut self) -> Option<Watermark>;

    /// Gets the current watermark without advancing it
    fn current_watermark(&self) -> Watermark;

    /// Resets the watermark generator to its initial state
    fn reset(&mut self);

    /// Gets the watermark for a specific partition
    fn partition_watermark(&self, partition: u32) -> Option<Watermark>;
}

/// Configuration for bounded out-of-orderness watermarking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundedOutOfOrdernessConfig {
    /// Maximum allowed out-of-orderness (delay to apply)
    pub max_out_of_orderness: Duration,
    /// Timeout after which a partition is considered idle
    pub idle_timeout: Option<Duration>,
    /// Whether to emit watermarks on every event
    pub emit_per_event: bool,
}

impl Default for BoundedOutOfOrdernessConfig {
    fn default() -> Self {
        Self {
            max_out_of_orderness: Duration::from_secs(5),
            idle_timeout: Some(Duration::from_secs(60)),
            emit_per_event: false,
        }
    }
}

/// Bounded out-of-orderness watermark generator
///
/// This generator tracks the maximum timestamp seen and generates watermarks by subtracting
/// a fixed delay. It handles per-partition watermarks and can detect idle partitions.
///
/// # Example
///
/// ```rust
/// use processor::watermark::{BoundedOutOfOrdernessWatermark, WatermarkGenerator};
/// use std::time::Duration;
///
/// let mut generator = BoundedOutOfOrdernessWatermark::new(
///     Duration::from_secs(5),
///     None,
/// );
///
/// // Process events
/// generator.on_event(1000, 0);
/// generator.on_event(2000, 0);
/// if let Some(wm) = generator.on_event(3000, 0) {
///     // Watermark at 3000 - 5000 = -2000 (clamped to min)
/// }
/// ```
pub struct BoundedOutOfOrdernessWatermark {
    /// Maximum timestamp seen across all partitions
    max_timestamp: i64,
    /// Per-partition maximum timestamps
    partition_max_timestamps: DashMap<u32, i64>,
    /// Per-partition last activity time
    partition_last_activity: DashMap<u32, DateTime<Utc>>,
    /// Configuration
    config: BoundedOutOfOrdernessConfig,
    /// Current global watermark
    current_watermark: Arc<Mutex<Watermark>>,
    /// Per-partition watermarks
    partition_watermarks: DashMap<u32, Watermark>,
}

impl BoundedOutOfOrdernessWatermark {
    /// Creates a new bounded out-of-orderness watermark generator
    ///
    /// # Arguments
    /// * `max_out_of_orderness` - The maximum allowed delay for late events
    /// * `idle_timeout` - Optional timeout for detecting idle partitions
    pub fn new(max_out_of_orderness: Duration, idle_timeout: Option<Duration>) -> Self {
        Self {
            max_timestamp: i64::MIN,
            partition_max_timestamps: DashMap::new(),
            partition_last_activity: DashMap::new(),
            config: BoundedOutOfOrdernessConfig {
                max_out_of_orderness,
                idle_timeout,
                emit_per_event: false,
            },
            current_watermark: Arc::new(Mutex::new(Watermark::min())),
            partition_watermarks: DashMap::new(),
        }
    }

    /// Creates a new watermark generator with custom configuration
    pub fn with_config(config: BoundedOutOfOrdernessConfig) -> Self {
        Self {
            max_timestamp: i64::MIN,
            partition_max_timestamps: DashMap::new(),
            partition_last_activity: DashMap::new(),
            config,
            current_watermark: Arc::new(Mutex::new(Watermark::min())),
            partition_watermarks: DashMap::new(),
        }
    }

    /// Computes the watermark based on the maximum timestamp and configured delay
    fn compute_watermark(&self, max_ts: i64) -> Watermark {
        let delay_ms = self.config.max_out_of_orderness.as_millis() as i64;
        let watermark_ts = max_ts.saturating_sub(delay_ms);
        Watermark::new(watermark_ts)
    }

    /// Updates the partition activity timestamp
    fn update_partition_activity(&self, partition: u32) {
        self.partition_last_activity.insert(partition, Utc::now());
    }

    /// Checks if a partition is idle based on the configured timeout
    fn is_partition_idle(&self, partition: u32) -> bool {
        if let Some(timeout) = self.config.idle_timeout {
            if let Some(last_activity) = self.partition_last_activity.get(&partition) {
                let elapsed = Utc::now().signed_duration_since(*last_activity);
                return elapsed > ChronoDuration::from_std(timeout).unwrap_or_default();
            }
        }
        false
    }

    /// Gets all active partitions (non-idle)
    pub fn active_partitions(&self) -> Vec<u32> {
        self.partition_max_timestamps
            .iter()
            .filter(|entry| !self.is_partition_idle(*entry.key()))
            .map(|entry| *entry.key())
            .collect()
    }

    /// Gets all idle partitions
    pub fn idle_partitions(&self) -> Vec<u32> {
        self.partition_max_timestamps
            .iter()
            .filter(|entry| self.is_partition_idle(*entry.key()))
            .map(|entry| *entry.key())
            .collect()
    }

    /// Merges partition watermarks to compute the global watermark
    ///
    /// The global watermark is the minimum of all partition watermarks to ensure
    /// we don't advance past events that haven't been processed yet
    fn merge_partition_watermarks(&self) -> Option<Watermark> {
        let active_partitions: Vec<_> = self.active_partitions();

        if active_partitions.is_empty() {
            return None;
        }

        let min_watermark = active_partitions
            .iter()
            .filter_map(|&partition| self.partition_watermarks.get(&partition).map(|w| *w))
            .min()?;

        Some(min_watermark)
    }

    /// Checks if an event is late based on the current watermark
    pub fn is_late_event(&self, timestamp: i64) -> bool {
        let current = self.current_watermark.lock().unwrap();
        timestamp < current.timestamp
    }

    /// Gets the lateness of an event (how late it is compared to the watermark)
    ///
    /// Returns 0 if the event is not late
    pub fn get_lateness(&self, timestamp: i64) -> i64 {
        let current = self.current_watermark.lock().unwrap();
        if timestamp < current.timestamp {
            current.timestamp - timestamp
        } else {
            0
        }
    }
}

impl WatermarkGenerator for BoundedOutOfOrdernessWatermark {
    fn on_event(&mut self, timestamp: i64, partition: u32) -> Option<Watermark> {
        trace!(
            timestamp = timestamp,
            partition = partition,
            "Processing event for watermark"
        );

        // Update partition activity
        self.update_partition_activity(partition);

        // Update partition max timestamp
        self.partition_max_timestamps
            .entry(partition)
            .and_modify(|max_ts| *max_ts = max(*max_ts, timestamp))
            .or_insert(timestamp);

        // Update global max timestamp
        self.max_timestamp = max(self.max_timestamp, timestamp);

        // Compute partition watermark
        let partition_watermark = self.compute_watermark(timestamp);
        self.partition_watermarks.insert(partition, partition_watermark);

        // Merge partition watermarks to get global watermark
        if let Some(new_watermark) = self.merge_partition_watermarks() {
            let mut current = self.current_watermark.lock().unwrap();
            if new_watermark > *current {
                *current = new_watermark;
                debug!(
                    watermark = %new_watermark,
                    "Advanced watermark"
                );
                return if self.config.emit_per_event {
                    Some(new_watermark)
                } else {
                    None
                };
            }
        }

        None
    }

    fn on_periodic_check(&mut self) -> Option<Watermark> {
        // Check for idle partitions and warn
        let idle = self.idle_partitions();
        if !idle.is_empty() {
            warn!(partitions = ?idle, "Detected idle partitions");
        }

        // Recompute watermark based on active partitions
        if let Some(new_watermark) = self.merge_partition_watermarks() {
            let mut current = self.current_watermark.lock().unwrap();
            if new_watermark > *current {
                *current = new_watermark;
                debug!(watermark = %new_watermark, "Periodic watermark update");
                return Some(new_watermark);
            }
        }

        None
    }

    fn current_watermark(&self) -> Watermark {
        *self.current_watermark.lock().unwrap()
    }

    fn reset(&mut self) {
        self.max_timestamp = i64::MIN;
        self.partition_max_timestamps.clear();
        self.partition_last_activity.clear();
        self.partition_watermarks.clear();
        *self.current_watermark.lock().unwrap() = Watermark::min();
        debug!("Watermark generator reset");
    }

    fn partition_watermark(&self, partition: u32) -> Option<Watermark> {
        self.partition_watermarks.get(&partition).map(|w| *w)
    }
}

/// Configuration for periodic watermark emission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeriodicWatermarkConfig {
    /// Interval at which to emit watermarks
    pub interval: Duration,
    /// Maximum allowed out-of-orderness
    pub max_out_of_orderness: Duration,
}

impl Default for PeriodicWatermarkConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(1),
            max_out_of_orderness: Duration::from_secs(5),
        }
    }
}

/// Periodic watermark generator
///
/// Emits watermarks at regular intervals based on wall-clock time rather than event time.
/// Useful for systems that need predictable watermark progression.
pub struct PeriodicWatermark {
    /// Last emission time
    last_emission: DateTime<Utc>,
    /// Configuration
    config: PeriodicWatermarkConfig,
    /// Underlying bounded watermark generator
    bounded_generator: BoundedOutOfOrdernessWatermark,
}

impl PeriodicWatermark {
    /// Creates a new periodic watermark generator
    pub fn new(config: PeriodicWatermarkConfig) -> Self {
        Self {
            last_emission: Utc::now(),
            config: config.clone(),
            bounded_generator: BoundedOutOfOrdernessWatermark::new(
                config.max_out_of_orderness,
                None,
            ),
        }
    }

    /// Checks if it's time to emit a watermark
    fn should_emit(&self) -> bool {
        let elapsed = Utc::now().signed_duration_since(self.last_emission);
        elapsed >= ChronoDuration::from_std(self.config.interval).unwrap_or_default()
    }
}

impl WatermarkGenerator for PeriodicWatermark {
    fn on_event(&mut self, timestamp: i64, partition: u32) -> Option<Watermark> {
        // Update internal state but don't emit
        self.bounded_generator.on_event(timestamp, partition);
        None
    }

    fn on_periodic_check(&mut self) -> Option<Watermark> {
        if self.should_emit() {
            self.last_emission = Utc::now();
            let watermark = self.bounded_generator.current_watermark();
            if !watermark.is_min() {
                debug!(watermark = %watermark, "Periodic watermark emission");
                return Some(watermark);
            }
        }
        None
    }

    fn current_watermark(&self) -> Watermark {
        self.bounded_generator.current_watermark()
    }

    fn reset(&mut self) {
        self.bounded_generator.reset();
        self.last_emission = Utc::now();
    }

    fn partition_watermark(&self, partition: u32) -> Option<Watermark> {
        self.bounded_generator.partition_watermark(partition)
    }
}

/// Punctuated watermark generator
///
/// Emits watermarks based on specific patterns or markers in the event stream.
/// For example, when a special "watermark event" is encountered.
pub struct PunctuatedWatermark {
    /// Function to extract watermark from event
    extractor: Box<dyn Fn(i64, u32) -> Option<i64> + Send + Sync>,
    /// Current watermark
    current_watermark: Watermark,
    /// Per-partition watermarks
    partition_watermarks: HashMap<u32, Watermark>,
}

impl PunctuatedWatermark {
    /// Creates a new punctuated watermark generator
    ///
    /// # Arguments
    /// * `extractor` - Function that returns Some(watermark_timestamp) if the event
    ///   should trigger a watermark emission, None otherwise
    pub fn new<F>(extractor: F) -> Self
    where
        F: Fn(i64, u32) -> Option<i64> + Send + Sync + 'static,
    {
        Self {
            extractor: Box::new(extractor),
            current_watermark: Watermark::min(),
            partition_watermarks: HashMap::new(),
        }
    }

    /// Creates a watermark generator that emits on every Nth event
    pub fn every_n_events(n: u64) -> Self {
        let counter = Arc::new(Mutex::new(0u64));
        Self::new(move |timestamp, _partition| {
            let mut count = counter.lock().unwrap();
            *count += 1;
            if *count % n == 0 {
                Some(timestamp)
            } else {
                None
            }
        })
    }

    /// Creates a watermark generator that emits when timestamp crosses boundaries
    pub fn on_timestamp_boundary(boundary_ms: i64) -> Self {
        let last_boundary = Arc::new(Mutex::new(0i64));
        Self::new(move |timestamp, _partition| {
            let mut last = last_boundary.lock().unwrap();
            let current_boundary = (timestamp / boundary_ms) * boundary_ms;
            if current_boundary > *last {
                *last = current_boundary;
                Some(current_boundary)
            } else {
                None
            }
        })
    }
}

impl WatermarkGenerator for PunctuatedWatermark {
    fn on_event(&mut self, timestamp: i64, partition: u32) -> Option<Watermark> {
        if let Some(watermark_ts) = (self.extractor)(timestamp, partition) {
            let new_watermark = Watermark::new(watermark_ts);

            // Update partition watermark
            self.partition_watermarks.insert(partition, new_watermark);

            // Update global watermark if it's newer
            if new_watermark > self.current_watermark {
                self.current_watermark = new_watermark;
                debug!(
                    watermark = %new_watermark,
                    partition = partition,
                    "Punctuated watermark emission"
                );
                return Some(new_watermark);
            }
        }
        None
    }

    fn on_periodic_check(&mut self) -> Option<Watermark> {
        None // Punctuated watermarks only emit on events
    }

    fn current_watermark(&self) -> Watermark {
        self.current_watermark
    }

    fn reset(&mut self) {
        self.current_watermark = Watermark::min();
        self.partition_watermarks.clear();
    }

    fn partition_watermark(&self, partition: u32) -> Option<Watermark> {
        self.partition_watermarks.get(&partition).copied()
    }
}

/// Late event handler configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LateEventConfig {
    /// Maximum allowed lateness before dropping events
    pub allowed_lateness: Duration,
    /// Whether to log late events
    pub log_late_events: bool,
    /// Whether to emit metrics for late events
    pub emit_metrics: bool,
}

impl Default for LateEventConfig {
    fn default() -> Self {
        Self {
            allowed_lateness: Duration::from_secs(60),
            log_late_events: true,
            emit_metrics: true,
        }
    }
}

/// Handler for late events
pub struct LateEventHandler {
    /// Configuration
    config: LateEventConfig,
    /// Count of dropped late events
    dropped_count: Arc<Mutex<u64>>,
    /// Count of accepted late events
    accepted_count: Arc<Mutex<u64>>,
    /// Total lateness of accepted events
    total_lateness: Arc<Mutex<i64>>,
}

impl LateEventHandler {
    /// Creates a new late event handler
    pub fn new(config: LateEventConfig) -> Self {
        Self {
            config,
            dropped_count: Arc::new(Mutex::new(0)),
            accepted_count: Arc::new(Mutex::new(0)),
            total_lateness: Arc::new(Mutex::new(0)),
        }
    }

    /// Handles a late event
    ///
    /// # Returns
    /// True if the event should be processed, false if it should be dropped
    pub fn handle_late_event(&self, lateness: i64, partition: u32) -> bool {
        let lateness_duration = Duration::from_millis(lateness as u64);

        if lateness_duration > self.config.allowed_lateness {
            // Drop the event
            *self.dropped_count.lock().unwrap() += 1;

            if self.config.log_late_events {
                warn!(
                    lateness_ms = lateness,
                    partition = partition,
                    "Dropping late event beyond allowed lateness"
                );
            }

            false
        } else {
            // Accept the event
            *self.accepted_count.lock().unwrap() += 1;
            *self.total_lateness.lock().unwrap() += lateness;

            if self.config.log_late_events {
                debug!(
                    lateness_ms = lateness,
                    partition = partition,
                    "Accepting late event within allowed lateness"
                );
            }

            true
        }
    }

    /// Gets statistics about late event handling
    pub fn get_stats(&self) -> LateEventStats {
        let dropped = *self.dropped_count.lock().unwrap();
        let accepted = *self.accepted_count.lock().unwrap();
        let total_lateness = *self.total_lateness.lock().unwrap();

        let avg_lateness = if accepted > 0 {
            total_lateness / accepted as i64
        } else {
            0
        };

        LateEventStats {
            dropped_count: dropped,
            accepted_count: accepted,
            average_lateness_ms: avg_lateness,
        }
    }

    /// Resets the statistics
    pub fn reset_stats(&self) {
        *self.dropped_count.lock().unwrap() = 0;
        *self.accepted_count.lock().unwrap() = 0;
        *self.total_lateness.lock().unwrap() = 0;
    }
}

/// Statistics for late event handling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LateEventStats {
    /// Number of dropped late events
    pub dropped_count: u64,
    /// Number of accepted late events
    pub accepted_count: u64,
    /// Average lateness in milliseconds
    pub average_lateness_ms: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watermark_creation() {
        let wm = Watermark::new(1000);
        assert_eq!(wm.timestamp, 1000);
        assert!(wm.is_before(2000));
        assert!(wm.is_after(500));
    }

    #[test]
    fn test_watermark_ordering() {
        let wm1 = Watermark::new(1000);
        let wm2 = Watermark::new(2000);
        assert!(wm1 < wm2);
        assert!(wm2 > wm1);
    }

    #[test]
    fn test_watermark_min_max() {
        let min = Watermark::min();
        let max = Watermark::max();
        assert!(min.is_min());
        assert!(max.is_max());
        assert!(min < max);
    }

    #[test]
    fn test_bounded_watermark_basic() {
        let mut generator = BoundedOutOfOrdernessWatermark::new(
            Duration::from_secs(5),
            None,
        );

        // Process events
        generator.on_event(10000, 0);
        generator.on_event(15000, 0);
        generator.on_event(20000, 0);

        let wm = generator.current_watermark();
        // Watermark should be 20000 - 5000 = 15000
        assert_eq!(wm.timestamp, 15000);
    }

    #[test]
    fn test_bounded_watermark_out_of_order() {
        let mut generator = BoundedOutOfOrdernessWatermark::new(
            Duration::from_secs(10),
            None,
        );

        // Events arrive out of order
        generator.on_event(30000, 0);
        generator.on_event(20000, 0);
        generator.on_event(25000, 0);

        let wm = generator.current_watermark();
        // Watermark should be based on max timestamp 30000 - 10000 = 20000
        assert_eq!(wm.timestamp, 20000);
    }

    #[test]
    fn test_bounded_watermark_multiple_partitions() {
        let mut generator = BoundedOutOfOrdernessWatermark::new(
            Duration::from_secs(5),
            None,
        );

        // Events from different partitions
        generator.on_event(10000, 0);
        generator.on_event(20000, 1);
        generator.on_event(15000, 2);

        // Global watermark should be min of all partitions
        // Partition 0: 10000 - 5000 = 5000
        // Partition 1: 20000 - 5000 = 15000
        // Partition 2: 15000 - 5000 = 10000
        // Min = 5000
        let wm = generator.current_watermark();
        assert_eq!(wm.timestamp, 5000);
    }

    #[test]
    fn test_bounded_watermark_late_event_detection() {
        let mut generator = BoundedOutOfOrdernessWatermark::new(
            Duration::from_secs(5),
            None,
        );

        generator.on_event(20000, 0);

        // Current watermark is 15000
        assert!(generator.is_late_event(10000));
        assert!(!generator.is_late_event(16000));
    }

    #[test]
    fn test_bounded_watermark_lateness_calculation() {
        let mut generator = BoundedOutOfOrdernessWatermark::new(
            Duration::from_secs(5),
            None,
        );

        generator.on_event(20000, 0);

        // Current watermark is 15000
        assert_eq!(generator.get_lateness(10000), 5000);
        assert_eq!(generator.get_lateness(16000), 0);
    }

    #[test]
    fn test_periodic_watermark() {
        let config = PeriodicWatermarkConfig {
            interval: Duration::from_millis(100),
            max_out_of_orderness: Duration::from_secs(5),
        };
        let mut generator = PeriodicWatermark::new(config);

        // Process events
        generator.on_event(10000, 0);
        generator.on_event(20000, 0);

        // Should not emit on event
        assert!(generator.on_event(30000, 0).is_none());

        // Sleep and check periodically
        std::thread::sleep(Duration::from_millis(150));
        let wm = generator.on_periodic_check();
        assert!(wm.is_some());
    }

    #[test]
    fn test_punctuated_watermark_every_n() {
        let mut generator = PunctuatedWatermark::every_n_events(3);

        // First two events should not emit
        assert!(generator.on_event(1000, 0).is_none());
        assert!(generator.on_event(2000, 0).is_none());

        // Third event should emit
        let wm = generator.on_event(3000, 0);
        assert!(wm.is_some());
        assert_eq!(wm.unwrap().timestamp, 3000);
    }

    #[test]
    fn test_punctuated_watermark_boundary() {
        let mut generator = PunctuatedWatermark::on_timestamp_boundary(10000);

        // Events within same boundary
        assert!(generator.on_event(5000, 0).is_none());
        assert!(generator.on_event(8000, 0).is_none());

        // Event crossing boundary
        let wm = generator.on_event(15000, 0);
        assert!(wm.is_some());
        assert_eq!(wm.unwrap().timestamp, 10000);

        // Another event crossing boundary
        let wm2 = generator.on_event(25000, 0);
        assert!(wm2.is_some());
        assert_eq!(wm2.unwrap().timestamp, 20000);
    }

    #[test]
    fn test_late_event_handler_within_lateness() {
        let config = LateEventConfig {
            allowed_lateness: Duration::from_secs(10),
            log_late_events: false,
            emit_metrics: true,
        };
        let handler = LateEventHandler::new(config);

        // Event with 5 seconds lateness (within allowed)
        assert!(handler.handle_late_event(5000, 0));

        let stats = handler.get_stats();
        assert_eq!(stats.accepted_count, 1);
        assert_eq!(stats.dropped_count, 0);
    }

    #[test]
    fn test_late_event_handler_beyond_lateness() {
        let config = LateEventConfig {
            allowed_lateness: Duration::from_secs(10),
            log_late_events: false,
            emit_metrics: true,
        };
        let handler = LateEventHandler::new(config);

        // Event with 15 seconds lateness (beyond allowed)
        assert!(!handler.handle_late_event(15000, 0));

        let stats = handler.get_stats();
        assert_eq!(stats.accepted_count, 0);
        assert_eq!(stats.dropped_count, 1);
    }

    #[test]
    fn test_late_event_handler_stats() {
        let config = LateEventConfig::default();
        let handler = LateEventHandler::new(config);

        handler.handle_late_event(5000, 0);
        handler.handle_late_event(10000, 0);
        handler.handle_late_event(80000, 0); // Should be dropped

        let stats = handler.get_stats();
        assert_eq!(stats.accepted_count, 2);
        assert_eq!(stats.dropped_count, 1);
        assert_eq!(stats.average_lateness_ms, 7500);
    }

    #[test]
    fn test_watermark_generator_reset() {
        let mut generator = BoundedOutOfOrdernessWatermark::new(
            Duration::from_secs(5),
            None,
        );

        generator.on_event(20000, 0);
        assert_ne!(generator.current_watermark(), Watermark::min());

        generator.reset();
        assert_eq!(generator.current_watermark(), Watermark::min());
    }

    #[test]
    fn test_partition_watermark_tracking() {
        let mut generator = BoundedOutOfOrdernessWatermark::new(
            Duration::from_secs(5),
            None,
        );

        generator.on_event(10000, 0);
        generator.on_event(20000, 1);

        let wm0 = generator.partition_watermark(0).unwrap();
        let wm1 = generator.partition_watermark(1).unwrap();

        assert_eq!(wm0.timestamp, 5000);
        assert_eq!(wm1.timestamp, 15000);
    }

    #[test]
    fn test_watermark_datetime_conversion() {
        let dt = DateTime::from_timestamp(1000, 0).unwrap();
        let wm = Watermark::from_datetime(dt);
        let _dt2 = wm.to_datetime();

        assert_eq!(wm.timestamp, 1000000); // milliseconds
    }

    #[test]
    fn test_idle_partition_detection() {
        let mut generator = BoundedOutOfOrdernessWatermark::new(
            Duration::from_secs(5),
            Some(Duration::from_millis(100)),
        );

        generator.on_event(10000, 0);
        generator.on_event(20000, 1);

        // Initially no idle partitions
        assert_eq!(generator.idle_partitions().len(), 0);

        // Wait for idle timeout
        std::thread::sleep(Duration::from_millis(150));

        // Now both partitions should be idle
        let idle = generator.idle_partitions();
        assert_eq!(idle.len(), 2);
    }

    #[test]
    fn test_watermark_monotonicity() {
        let mut generator = BoundedOutOfOrdernessWatermark::new(
            Duration::from_secs(5),
            None,
        );

        generator.on_event(20000, 0);
        let wm1 = generator.current_watermark();

        // Try to advance with older event (should not go backwards)
        generator.on_event(15000, 0);
        let wm2 = generator.current_watermark();

        assert!(wm2 >= wm1);
    }
}
