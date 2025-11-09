//! Window types and bounds
//!
//! This module defines the core window types used for time-based aggregations
//! and event processing in the stream processor.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents the time bounds of a window
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WindowBounds {
    /// Start time of the window (inclusive)
    pub start: DateTime<Utc>,
    /// End time of the window (exclusive)
    pub end: DateTime<Utc>,
}

impl WindowBounds {
    /// Create a new window bounds
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        assert!(start < end, "Window start must be before end");
        Self { start, end }
    }

    /// Get the duration of the window
    pub fn duration(&self) -> Duration {
        self.end.signed_duration_since(self.start)
    }

    /// Check if a timestamp falls within this window
    pub fn contains(&self, timestamp: DateTime<Utc>) -> bool {
        timestamp >= self.start && timestamp < self.end
    }

    /// Check if this window overlaps with another window
    pub fn overlaps(&self, other: &WindowBounds) -> bool {
        self.start < other.end && other.start < self.end
    }

    /// Merge two overlapping windows into a single window
    pub fn merge(&self, other: &WindowBounds) -> WindowBounds {
        WindowBounds {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// Get the gap between two windows (returns None if windows overlap)
    pub fn gap(&self, other: &WindowBounds) -> Option<Duration> {
        if self.overlaps(other) {
            return None;
        }

        if self.end <= other.start {
            Some(other.start.signed_duration_since(self.end))
        } else {
            Some(self.start.signed_duration_since(other.end))
        }
    }
}

impl fmt::Display for WindowBounds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{} - {})",
            self.start.format("%Y-%m-%d %H:%M:%S%.3f"),
            self.end.format("%Y-%m-%d %H:%M:%S%.3f")
        )
    }
}

impl PartialOrd for WindowBounds {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for WindowBounds {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.start.cmp(&other.start)
            .then_with(|| self.end.cmp(&other.end))
    }
}

/// Represents a window in the stream processing pipeline
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Window {
    /// The time bounds of this window
    pub bounds: WindowBounds,
    /// Window identifier for tracking
    pub id: String,
}

impl Window {
    /// Create a new window with the given bounds
    pub fn new(bounds: WindowBounds) -> Self {
        let id = format!("{}_{}", bounds.start.timestamp_millis(), bounds.end.timestamp_millis());
        Self { bounds, id }
    }

    /// Check if a timestamp falls within this window
    pub fn contains(&self, timestamp: DateTime<Utc>) -> bool {
        self.bounds.contains(timestamp)
    }

    /// Get the duration of the window
    pub fn duration(&self) -> Duration {
        self.bounds.duration()
    }

    /// Check if this window can be merged with another
    pub fn can_merge(&self, other: &Window, max_gap: Duration) -> bool {
        if self.bounds.overlaps(&other.bounds) {
            return true;
        }

        if let Some(gap) = self.bounds.gap(&other.bounds) {
            gap <= max_gap
        } else {
            false
        }
    }

    /// Merge this window with another window
    pub fn merge(&self, other: &Window) -> Window {
        Window::new(self.bounds.merge(&other.bounds))
    }
}

impl fmt::Display for Window {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Window[{}]", self.bounds)
    }
}

/// Window type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowType {
    /// Tumbling window (fixed-size, non-overlapping)
    Tumbling,
    /// Sliding window (fixed-size, overlapping)
    Sliding,
    /// Session window (gap-based, variable-size)
    Session,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn create_timestamp(millis: i64) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(millis).unwrap()
    }

    #[test]
    fn test_window_bounds_creation() {
        let start = create_timestamp(1000);
        let end = create_timestamp(2000);
        let bounds = WindowBounds::new(start, end);

        assert_eq!(bounds.start, start);
        assert_eq!(bounds.end, end);
        assert_eq!(bounds.duration(), Duration::milliseconds(1000));
    }

    #[test]
    #[should_panic(expected = "Window start must be before end")]
    fn test_window_bounds_invalid() {
        let start = create_timestamp(2000);
        let end = create_timestamp(1000);
        WindowBounds::new(start, end);
    }

    #[test]
    fn test_window_bounds_contains() {
        let bounds = WindowBounds::new(
            create_timestamp(1000),
            create_timestamp(2000),
        );

        assert!(!bounds.contains(create_timestamp(999)));
        assert!(bounds.contains(create_timestamp(1000)));
        assert!(bounds.contains(create_timestamp(1500)));
        assert!(!bounds.contains(create_timestamp(2000)));
        assert!(!bounds.contains(create_timestamp(2001)));
    }

    #[test]
    fn test_window_bounds_overlaps() {
        let bounds1 = WindowBounds::new(
            create_timestamp(1000),
            create_timestamp(2000),
        );
        let bounds2 = WindowBounds::new(
            create_timestamp(1500),
            create_timestamp(2500),
        );
        let bounds3 = WindowBounds::new(
            create_timestamp(2500),
            create_timestamp(3500),
        );

        assert!(bounds1.overlaps(&bounds2));
        assert!(bounds2.overlaps(&bounds1));
        assert!(!bounds1.overlaps(&bounds3));
        assert!(!bounds3.overlaps(&bounds1));
    }

    #[test]
    fn test_window_bounds_merge() {
        let bounds1 = WindowBounds::new(
            create_timestamp(1000),
            create_timestamp(2000),
        );
        let bounds2 = WindowBounds::new(
            create_timestamp(1500),
            create_timestamp(2500),
        );

        let merged = bounds1.merge(&bounds2);
        assert_eq!(merged.start, create_timestamp(1000));
        assert_eq!(merged.end, create_timestamp(2500));
    }

    #[test]
    fn test_window_bounds_gap() {
        let bounds1 = WindowBounds::new(
            create_timestamp(1000),
            create_timestamp(2000),
        );
        let bounds2 = WindowBounds::new(
            create_timestamp(1500),
            create_timestamp(2500),
        );
        let bounds3 = WindowBounds::new(
            create_timestamp(3000),
            create_timestamp(4000),
        );

        // Overlapping windows have no gap
        assert_eq!(bounds1.gap(&bounds2), None);

        // Non-overlapping windows have a gap
        assert_eq!(bounds1.gap(&bounds3), Some(Duration::milliseconds(1000)));
        assert_eq!(bounds3.gap(&bounds1), Some(Duration::milliseconds(1000)));
    }

    #[test]
    fn test_window_creation() {
        let bounds = WindowBounds::new(
            create_timestamp(1000),
            create_timestamp(2000),
        );
        let window = Window::new(bounds);

        assert_eq!(window.bounds, bounds);
        assert_eq!(window.id, "1000_2000");
    }

    #[test]
    fn test_window_can_merge() {
        let window1 = Window::new(WindowBounds::new(
            create_timestamp(1000),
            create_timestamp(2000),
        ));
        let window2 = Window::new(WindowBounds::new(
            create_timestamp(1500),
            create_timestamp(2500),
        ));
        let window3 = Window::new(WindowBounds::new(
            create_timestamp(3000),
            create_timestamp(4000),
        ));

        // Overlapping windows can merge
        assert!(window1.can_merge(&window2, Duration::milliseconds(0)));

        // Non-overlapping windows within gap can merge
        assert!(window1.can_merge(&window3, Duration::milliseconds(1000)));

        // Non-overlapping windows beyond gap cannot merge
        assert!(!window1.can_merge(&window3, Duration::milliseconds(500)));
    }

    #[test]
    fn test_window_merge() {
        let window1 = Window::new(WindowBounds::new(
            create_timestamp(1000),
            create_timestamp(2000),
        ));
        let window2 = Window::new(WindowBounds::new(
            create_timestamp(1500),
            create_timestamp(2500),
        ));

        let merged = window1.merge(&window2);
        assert_eq!(merged.bounds.start, create_timestamp(1000));
        assert_eq!(merged.bounds.end, create_timestamp(2500));
    }

    #[test]
    fn test_window_ordering() {
        let bounds1 = WindowBounds::new(
            create_timestamp(1000),
            create_timestamp(2000),
        );
        let bounds2 = WindowBounds::new(
            create_timestamp(1500),
            create_timestamp(2500),
        );
        let bounds3 = WindowBounds::new(
            create_timestamp(3000),
            create_timestamp(4000),
        );

        assert!(bounds1 < bounds2);
        assert!(bounds2 < bounds3);
        assert!(bounds1 < bounds3);
    }
}
