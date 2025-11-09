//! Windowing module for stream processing
//!
//! This module provides comprehensive windowing functionality for time-based
//! aggregations and event processing. It supports three main types of windows:
//!
//! # Window Types
//!
//! ## Tumbling Windows
//! Fixed-size, non-overlapping windows. Each event belongs to exactly one window.
//!
//! ```text
//! Time:     0----5----10---15---20---25---30
//! Windows:  [----][----][----][----][----]
//! ```
//!
//! **Use cases:**
//! - Periodic aggregations (e.g., metrics every 5 minutes)
//! - Non-overlapping batch processing
//! - Fixed-interval reporting
//!
//! ## Sliding Windows
//! Fixed-size, overlapping windows. Events can belong to multiple windows.
//!
//! ```text
//! Time:     0----5----10---15---20---25---30
//! Windows:  [----------]
//!                [----------]
//!                     [----------]
//! ```
//!
//! **Use cases:**
//! - Moving averages
//! - Smoothed metrics
//! - Trend detection
//! - Rate calculations over time
//!
//! ## Session Windows
//! Variable-size windows based on inactivity gaps. Events are grouped into
//! sessions separated by periods of inactivity.
//!
//! ```text
//! Time:     0-2--5----------12-14----20
//! Sessions: [----]           [---]   [-]
//! Gap:           ^^^^^^^^^^^^     ^^^^
//! ```
//!
//! **Use cases:**
//! - User session analysis
//! - Burst detection
//! - Activity clustering
//! - Pattern recognition in irregular events
//!
//! # Window Assignment
//!
//! Windows are assigned using the [`WindowAssigner`] trait, which determines
//! which window(s) an event belongs to based on its timestamp.
//!
//! # Window Triggers
//!
//! Triggers determine when a window should be evaluated and emit results.
//! Multiple trigger types are supported:
//!
//! - **OnWatermarkTrigger**: Fires when watermark passes window end (event-time)
//! - **ProcessingTimeTrigger**: Fires based on processing time
//! - **CountTrigger**: Fires after N elements
//! - **CompositeTrigger**: Combines multiple triggers with AND/OR logic
//! - **ImmediateTrigger**: Fires on every element
//! - **NeverTrigger**: Never fires automatically
//!
//! # Example Usage
//!
//! ```rust
//! use processor::window::{
//!     assigner::{TumblingWindowAssigner, SlidingWindowAssigner, SessionWindowAssigner},
//!     trigger::{OnWatermarkTrigger, ProcessingTimeTrigger, CountTrigger},
//! };
//! use chrono::Duration;
//!
//! // Tumbling window: 5-minute non-overlapping windows
//! let tumbling = TumblingWindowAssigner::new(Duration::minutes(5));
//!
//! // Sliding window: 10-minute windows, sliding every 1 minute
//! let sliding = SlidingWindowAssigner::new(
//!     Duration::minutes(10),
//!     Duration::minutes(1),
//! );
//!
//! // Session window: 30-second inactivity gap
//! let session = SessionWindowAssigner::new(Duration::seconds(30));
//!
//! // Triggers
//! let watermark_trigger = OnWatermarkTrigger::new();
//! let processing_trigger = ProcessingTimeTrigger::with_delay(Duration::seconds(5));
//! let count_trigger = CountTrigger::new(100); // Fire after 100 elements
//! ```
//!
//! # Window Merging
//!
//! Session windows support merging when events arrive within the session gap.
//! The window module handles this automatically:
//!
//! ```text
//! Initial state:    Window A [0, 5)    Window B [10, 15)
//! Event at t=7:     Window A [0, 12)   (merged, extends to cover gap)
//! Event at t=12:    Window A [0, 17)   (merged with B)
//! ```

pub mod types;
pub mod assigner;
pub mod trigger;

pub use types::{Window, WindowBounds, WindowType};
pub use assigner::{
    WindowAssigner,
    TumblingWindowAssigner,
    SlidingWindowAssigner,
    SessionWindowAssigner,
};
pub use trigger::{
    WindowTrigger,
    TriggerResult,
    TriggerContext,
    OnWatermarkTrigger,
    ProcessingTimeTrigger,
    CountTrigger,
    CompositeTrigger,
    CompositeMode,
    ImmediateTrigger,
    NeverTrigger,
};

use chrono::DateTime;
use std::collections::HashMap;

/// Window merger for session windows
///
/// Handles merging of session windows when events arrive within the session gap.
#[derive(Debug)]
pub struct WindowMerger {
    /// Active windows indexed by their ID
    windows: HashMap<String, Window>,
    /// Session gap for merging
    session_gap: chrono::Duration,
}

impl WindowMerger {
    /// Create a new window merger with the specified session gap
    pub fn new(session_gap: chrono::Duration) -> Self {
        Self {
            windows: HashMap::new(),
            session_gap,
        }
    }

    /// Add a window and merge with overlapping/nearby windows
    ///
    /// Returns the final merged window and a list of windows that were merged
    pub fn add_and_merge(&mut self, window: Window) -> (Window, Vec<Window>) {
        let mut merged_windows = Vec::new();
        let mut result_window = window.clone();

        // Find all windows that can be merged
        let mut to_remove = Vec::new();
        for (id, existing_window) in &self.windows {
            if result_window.can_merge(existing_window, self.session_gap) {
                result_window = result_window.merge(existing_window);
                merged_windows.push(existing_window.clone());
                to_remove.push(id.clone());
            }
        }

        // Remove merged windows
        for id in to_remove {
            self.windows.remove(&id);
        }

        // Add the new merged window
        self.windows.insert(result_window.id.clone(), result_window.clone());

        (result_window, merged_windows)
    }

    /// Get all active windows
    pub fn windows(&self) -> Vec<&Window> {
        self.windows.values().collect()
    }

    /// Remove a window by ID
    pub fn remove(&mut self, window_id: &str) -> Option<Window> {
        self.windows.remove(window_id)
    }

    /// Remove windows older than the given watermark
    pub fn remove_old_windows(&mut self, watermark: DateTime<chrono::Utc>) -> Vec<Window> {
        let mut removed = Vec::new();
        self.windows.retain(|_, window| {
            if window.bounds.end <= watermark {
                removed.push(window.clone());
                false
            } else {
                true
            }
        });
        removed
    }

    /// Clear all windows
    pub fn clear(&mut self) {
        self.windows.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone, Utc};

    fn create_timestamp(millis: i64) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(millis).unwrap()
    }

    fn create_window(start_millis: i64, end_millis: i64) -> Window {
        Window::new(WindowBounds::new(
            create_timestamp(start_millis),
            create_timestamp(end_millis),
        ))
    }

    #[test]
    fn test_window_merger_no_overlap() {
        let mut merger = WindowMerger::new(Duration::milliseconds(1000));

        // Add first window
        let window1 = create_window(0, 1000);
        let (result, merged) = merger.add_and_merge(window1.clone());
        assert_eq!(result, window1);
        assert_eq!(merged.len(), 0);

        // Add non-overlapping window beyond gap
        let window2 = create_window(3000, 4000);
        let (result, merged) = merger.add_and_merge(window2.clone());
        assert_eq!(result, window2);
        assert_eq!(merged.len(), 0);

        // Should have 2 separate windows
        assert_eq!(merger.windows().len(), 2);
    }

    #[test]
    fn test_window_merger_with_overlap() {
        let mut merger = WindowMerger::new(Duration::milliseconds(1000));

        // Add first window [0, 1000)
        let window1 = create_window(0, 1000);
        merger.add_and_merge(window1);

        // Add overlapping window [500, 1500)
        let window2 = create_window(500, 1500);
        let (result, merged) = merger.add_and_merge(window2);

        // Should merge into [0, 1500)
        assert_eq!(result.bounds.start, create_timestamp(0));
        assert_eq!(result.bounds.end, create_timestamp(1500));
        assert_eq!(merged.len(), 1);

        // Should have 1 merged window
        assert_eq!(merger.windows().len(), 1);
    }

    #[test]
    fn test_window_merger_within_gap() {
        let mut merger = WindowMerger::new(Duration::milliseconds(1000));

        // Add first window [0, 1000)
        let window1 = create_window(0, 1000);
        merger.add_and_merge(window1);

        // Add window [1500, 2500) within gap
        let window2 = create_window(1500, 2500);
        let (result, merged) = merger.add_and_merge(window2);

        // Should merge into [0, 2500)
        assert_eq!(result.bounds.start, create_timestamp(0));
        assert_eq!(result.bounds.end, create_timestamp(2500));
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn test_window_merger_multiple_merges() {
        let mut merger = WindowMerger::new(Duration::milliseconds(500));

        // Add multiple windows that won't initially merge (gap > 500ms)
        merger.add_and_merge(create_window(0, 1000));
        merger.add_and_merge(create_window(2000, 3000));
        merger.add_and_merge(create_window(5000, 6000)); // Beyond gap from previous window

        // Verify we have 3 separate windows
        assert_eq!(merger.windows().len(), 3);

        // Add window that bridges first two windows (fills the gap)
        let window = create_window(1000, 2000);
        let (result, merged) = merger.add_and_merge(window);

        // Should merge [0, 1000) and [2000, 3000) via the bridge window
        assert_eq!(result.bounds.start, create_timestamp(0));
        assert_eq!(result.bounds.end, create_timestamp(3000));
        assert_eq!(merged.len(), 2);

        // Should have 2 windows now: merged one [0, 3000) and [5000, 6000)
        assert_eq!(merger.windows().len(), 2);
    }

    #[test]
    fn test_window_merger_remove_old() {
        let mut merger = WindowMerger::new(Duration::milliseconds(500));

        // Add windows that won't merge (gap > 500ms between them)
        merger.add_and_merge(create_window(0, 1000));
        merger.add_and_merge(create_window(2000, 3000));
        merger.add_and_merge(create_window(4000, 5000));

        // Remove windows older than 3000
        let removed = merger.remove_old_windows(create_timestamp(3000));
        assert_eq!(removed.len(), 2); // [0, 1000) and [2000, 3000)
        assert_eq!(merger.windows().len(), 1); // Only [4000, 5000) remains
    }

    #[test]
    fn test_tumbling_window_complete_flow() {
        use assigner::TumblingWindowAssigner;
        use trigger::OnWatermarkTrigger;

        let assigner = TumblingWindowAssigner::new(Duration::milliseconds(1000));
        let mut trigger = OnWatermarkTrigger::new();

        // Assign event at 500ms to window
        let windows = assigner.assign_windows(create_timestamp(500));
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].bounds.start, create_timestamp(0));
        assert_eq!(windows[0].bounds.end, create_timestamp(1000));

        // Check trigger with watermark before window end
        let ctx = TriggerContext {
            window: windows[0].clone(),
            event_time: Some(create_timestamp(500)),
            processing_time: create_timestamp(1500),
            watermark: Some(create_timestamp(900)),
            element_count: 1,
        };
        assert_eq!(trigger.on_watermark(&ctx), TriggerResult::Continue);

        // Check trigger with watermark past window end
        let ctx = TriggerContext {
            window: windows[0].clone(),
            event_time: Some(create_timestamp(500)),
            processing_time: create_timestamp(1500),
            watermark: Some(create_timestamp(1000)),
            element_count: 1,
        };
        assert_eq!(trigger.on_watermark(&ctx), TriggerResult::Fire);
    }

    #[test]
    fn test_sliding_window_complete_flow() {
        use assigner::SlidingWindowAssigner;

        let assigner = SlidingWindowAssigner::new(
            Duration::milliseconds(1000),
            Duration::milliseconds(500),
        );

        // Event at 700ms should be in 2 windows
        let windows = assigner.assign_windows(create_timestamp(700));
        assert_eq!(windows.len(), 2);

        // Verify windows
        assert_eq!(windows[0].bounds.start, create_timestamp(0));
        assert_eq!(windows[0].bounds.end, create_timestamp(1000));
        assert_eq!(windows[1].bounds.start, create_timestamp(500));
        assert_eq!(windows[1].bounds.end, create_timestamp(1500));
    }

    #[test]
    fn test_session_window_complete_flow() {
        use assigner::SessionWindowAssigner;

        let assigner = SessionWindowAssigner::new(Duration::milliseconds(1000));
        let mut merger = WindowMerger::new(Duration::milliseconds(1000));

        // First event at 0ms -> window [0, 1000)
        let windows = assigner.assign_windows(create_timestamp(0));
        let (_window1, _) = merger.add_and_merge(windows[0].clone());

        // Second event at 500ms -> window [500, 1500)
        let windows = assigner.assign_windows(create_timestamp(500));
        let (window2, merged) = merger.add_and_merge(windows[0].clone());

        // Should merge with first window
        assert_eq!(merged.len(), 1);
        assert_eq!(window2.bounds.start, create_timestamp(0));
        assert_eq!(window2.bounds.end, create_timestamp(1500));

        // Third event at 3000ms -> window [3000, 4000)
        let windows = assigner.assign_windows(create_timestamp(3000));
        let (window3, merged) = merger.add_and_merge(windows[0].clone());

        // Should not merge (gap too large)
        assert_eq!(merged.len(), 0);
        assert_eq!(window3.bounds.start, create_timestamp(3000));
        assert_eq!(window3.bounds.end, create_timestamp(4000));
    }
}
