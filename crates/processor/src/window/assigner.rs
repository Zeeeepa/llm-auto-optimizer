//! Window assigners
//!
//! This module provides different strategies for assigning events to windows
//! based on their timestamps.

use super::types::{Window, WindowBounds};
use chrono::{DateTime, Duration, TimeZone, Utc};
use std::fmt;

/// Trait for assigning events to windows
pub trait WindowAssigner: Send + Sync + fmt::Debug {
    /// Assign a timestamp to one or more windows
    fn assign_windows(&self, timestamp: DateTime<Utc>) -> Vec<Window>;

    /// Get the maximum number of windows an event can be assigned to
    fn max_windows_per_event(&self) -> usize {
        1
    }

    /// Check if this is a session window assigner (requires merging)
    fn is_session_window(&self) -> bool {
        false
    }
}

/// Tumbling window assigner
///
/// Creates fixed-size, non-overlapping windows. Each event is assigned to
/// exactly one window based on its timestamp.
///
/// # Example
/// ```text
/// Window size: 5 seconds
/// Event at timestamp 7 -> Window [5, 10)
/// Event at timestamp 12 -> Window [10, 15)
/// ```
#[derive(Debug, Clone)]
pub struct TumblingWindowAssigner {
    /// Size of each window
    size: Duration,
    /// Optional offset for window alignment
    offset: Duration,
}

impl TumblingWindowAssigner {
    /// Create a new tumbling window assigner
    pub fn new(size: Duration) -> Self {
        assert!(size > Duration::zero(), "Window size must be positive");
        Self {
            size,
            offset: Duration::zero(),
        }
    }

    /// Create a tumbling window assigner with an offset
    pub fn with_offset(mut self, offset: Duration) -> Self {
        self.offset = offset;
        self
    }

    /// Calculate the window start for a given timestamp
    fn window_start(&self, timestamp: DateTime<Utc>) -> DateTime<Utc> {
        let ts_millis = timestamp.timestamp_millis();
        let offset_millis = self.offset.num_milliseconds();
        let size_millis = self.size.num_milliseconds();

        let aligned = ((ts_millis - offset_millis) / size_millis) * size_millis + offset_millis;

        Utc.timestamp_millis_opt(aligned).unwrap()
    }
}

impl WindowAssigner for TumblingWindowAssigner {
    fn assign_windows(&self, timestamp: DateTime<Utc>) -> Vec<Window> {
        let start = self.window_start(timestamp);
        let end = start + self.size;
        let bounds = WindowBounds::new(start, end);
        vec![Window::new(bounds)]
    }
}

/// Sliding window assigner
///
/// Creates fixed-size, overlapping windows. Each event may be assigned to
/// multiple windows based on the slide interval.
///
/// # Example
/// ```text
/// Window size: 10 seconds, Slide: 5 seconds
/// Event at timestamp 7 -> Windows [0, 10), [5, 15)
/// Event at timestamp 12 -> Windows [5, 15), [10, 20)
/// ```
#[derive(Debug, Clone)]
pub struct SlidingWindowAssigner {
    /// Size of each window
    size: Duration,
    /// Slide interval between windows
    slide: Duration,
    /// Optional offset for window alignment
    offset: Duration,
}

impl SlidingWindowAssigner {
    /// Create a new sliding window assigner
    pub fn new(size: Duration, slide: Duration) -> Self {
        assert!(size > Duration::zero(), "Window size must be positive");
        assert!(slide > Duration::zero(), "Slide must be positive");
        assert!(slide <= size, "Slide must not be larger than window size");

        Self {
            size,
            slide,
            offset: Duration::zero(),
        }
    }

    /// Create a sliding window assigner with an offset
    pub fn with_offset(mut self, offset: Duration) -> Self {
        self.offset = offset;
        self
    }

    /// Calculate all window starts that contain the timestamp
    fn window_starts(&self, timestamp: DateTime<Utc>) -> Vec<DateTime<Utc>> {
        let ts_millis = timestamp.timestamp_millis();
        let offset_millis = self.offset.num_milliseconds();
        let size_millis = self.size.num_milliseconds();
        let slide_millis = self.slide.num_milliseconds();

        // Find the first window that could contain this timestamp
        let first_start = ((ts_millis - offset_millis - size_millis + 1) / slide_millis) * slide_millis + offset_millis;

        let mut starts = Vec::new();
        let mut current_start = first_start;

        // Find all windows that contain this timestamp
        while current_start <= ts_millis - offset_millis {
            let start_time = Utc.timestamp_millis_opt(current_start).unwrap();
            let end_time = start_time + self.size;

            if timestamp >= start_time && timestamp < end_time {
                starts.push(start_time);
            }

            current_start += slide_millis;
        }

        starts
    }
}

impl WindowAssigner for SlidingWindowAssigner {
    fn assign_windows(&self, timestamp: DateTime<Utc>) -> Vec<Window> {
        self.window_starts(timestamp)
            .into_iter()
            .map(|start| {
                let end = start + self.size;
                let bounds = WindowBounds::new(start, end);
                Window::new(bounds)
            })
            .collect()
    }

    fn max_windows_per_event(&self) -> usize {
        (self.size.num_milliseconds() / self.slide.num_milliseconds()) as usize
    }
}

/// Session window assigner
///
/// Creates variable-size windows based on inactivity gaps. Events are grouped
/// into the same window if they occur within the session gap of each other.
/// Session windows require merging when new events arrive.
///
/// # Example
/// ```text
/// Session gap: 5 seconds
/// Events at 0, 2, 10 -> Windows [0, 7), [10, 15)
/// Event at 4 arrives -> Windows merge to [0, 9), [10, 15)
/// ```
#[derive(Debug, Clone)]
pub struct SessionWindowAssigner {
    /// Maximum gap between events in a session
    gap: Duration,
}

impl SessionWindowAssigner {
    /// Create a new session window assigner
    pub fn new(gap: Duration) -> Self {
        assert!(gap > Duration::zero(), "Session gap must be positive");
        Self { gap }
    }

    /// Get the session gap
    pub fn gap(&self) -> Duration {
        self.gap
    }
}

impl WindowAssigner for SessionWindowAssigner {
    fn assign_windows(&self, timestamp: DateTime<Utc>) -> Vec<Window> {
        // Session windows start at the event timestamp and end at timestamp + gap
        let start = timestamp;
        let end = timestamp + self.gap;
        let bounds = WindowBounds::new(start, end);
        vec![Window::new(bounds)]
    }

    fn is_session_window(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn create_timestamp(millis: i64) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(millis).unwrap()
    }

    #[test]
    fn test_tumbling_window_assignment() {
        let assigner = TumblingWindowAssigner::new(Duration::milliseconds(1000));

        // Event at 500ms -> Window [0, 1000)
        let windows = assigner.assign_windows(create_timestamp(500));
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].bounds.start, create_timestamp(0));
        assert_eq!(windows[0].bounds.end, create_timestamp(1000));

        // Event at 1500ms -> Window [1000, 2000)
        let windows = assigner.assign_windows(create_timestamp(1500));
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].bounds.start, create_timestamp(1000));
        assert_eq!(windows[0].bounds.end, create_timestamp(2000));

        // Event exactly at window boundary -> Next window
        let windows = assigner.assign_windows(create_timestamp(2000));
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].bounds.start, create_timestamp(2000));
        assert_eq!(windows[0].bounds.end, create_timestamp(3000));
    }

    #[test]
    fn test_tumbling_window_with_offset() {
        let assigner = TumblingWindowAssigner::new(Duration::milliseconds(1000))
            .with_offset(Duration::milliseconds(200));

        // Event at 500ms -> Window [200, 1200)
        let windows = assigner.assign_windows(create_timestamp(500));
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].bounds.start, create_timestamp(200));
        assert_eq!(windows[0].bounds.end, create_timestamp(1200));
    }

    #[test]
    fn test_sliding_window_assignment() {
        let assigner = SlidingWindowAssigner::new(
            Duration::milliseconds(1000),
            Duration::milliseconds(500),
        );

        // Event at 700ms should be in windows [0, 1000) and [500, 1500)
        let windows = assigner.assign_windows(create_timestamp(700));
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].bounds.start, create_timestamp(0));
        assert_eq!(windows[0].bounds.end, create_timestamp(1000));
        assert_eq!(windows[1].bounds.start, create_timestamp(500));
        assert_eq!(windows[1].bounds.end, create_timestamp(1500));

        // Event at 1200ms should be in windows [500, 1500), [1000, 2000)
        let windows = assigner.assign_windows(create_timestamp(1200));
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].bounds.start, create_timestamp(500));
        assert_eq!(windows[0].bounds.end, create_timestamp(1500));
        assert_eq!(windows[1].bounds.start, create_timestamp(1000));
        assert_eq!(windows[1].bounds.end, create_timestamp(2000));
    }

    #[test]
    fn test_sliding_window_max_windows() {
        let assigner = SlidingWindowAssigner::new(
            Duration::milliseconds(1000),
            Duration::milliseconds(500),
        );
        assert_eq!(assigner.max_windows_per_event(), 2);

        let assigner = SlidingWindowAssigner::new(
            Duration::milliseconds(1000),
            Duration::milliseconds(250),
        );
        assert_eq!(assigner.max_windows_per_event(), 4);
    }

    #[test]
    fn test_sliding_window_with_offset() {
        let assigner = SlidingWindowAssigner::new(
            Duration::milliseconds(1000),
            Duration::milliseconds(500),
        ).with_offset(Duration::milliseconds(100));

        let windows = assigner.assign_windows(create_timestamp(700));
        // Windows should be offset by 100ms
        assert!(windows.iter().all(|w| w.bounds.start.timestamp_millis() % 500 == 100));
    }

    #[test]
    fn test_session_window_assignment() {
        let assigner = SessionWindowAssigner::new(Duration::milliseconds(5000));

        // Event at 1000ms -> Window [1000, 6000)
        let windows = assigner.assign_windows(create_timestamp(1000));
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].bounds.start, create_timestamp(1000));
        assert_eq!(windows[0].bounds.end, create_timestamp(6000));

        // Event at 2000ms -> Window [2000, 7000)
        let windows = assigner.assign_windows(create_timestamp(2000));
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].bounds.start, create_timestamp(2000));
        assert_eq!(windows[0].bounds.end, create_timestamp(7000));
    }

    #[test]
    fn test_session_window_is_session() {
        let assigner = SessionWindowAssigner::new(Duration::milliseconds(5000));
        assert!(assigner.is_session_window());

        let tumbling = TumblingWindowAssigner::new(Duration::milliseconds(1000));
        assert!(!tumbling.is_session_window());
    }

    #[test]
    #[should_panic(expected = "Window size must be positive")]
    fn test_tumbling_invalid_size() {
        TumblingWindowAssigner::new(Duration::zero());
    }

    #[test]
    #[should_panic(expected = "Slide must not be larger than window size")]
    fn test_sliding_invalid_slide() {
        SlidingWindowAssigner::new(
            Duration::milliseconds(1000),
            Duration::milliseconds(2000),
        );
    }

    #[test]
    #[should_panic(expected = "Session gap must be positive")]
    fn test_session_invalid_gap() {
        SessionWindowAssigner::new(Duration::zero());
    }
}
