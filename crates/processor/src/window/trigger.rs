//! Window triggers
//!
//! This module provides different strategies for determining when windows
//! should be evaluated and their results emitted.

use super::types::Window;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::fmt;

/// Result of trigger evaluation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerResult {
    /// Continue processing, don't fire
    Continue,
    /// Fire the window and emit results
    Fire,
    /// Fire the window and purge all state
    FireAndPurge,
    /// Purge the window without firing
    Purge,
}

/// Context provided to triggers for evaluation
#[derive(Debug, Clone)]
pub struct TriggerContext {
    /// The window being evaluated
    pub window: Window,
    /// Current event timestamp (if triggered by event)
    pub event_time: Option<DateTime<Utc>>,
    /// Current processing time
    pub processing_time: DateTime<Utc>,
    /// Current watermark
    pub watermark: Option<DateTime<Utc>>,
    /// Number of elements in the window
    pub element_count: usize,
}

/// Trait for window triggers
pub trait WindowTrigger: Send + Sync + fmt::Debug {
    /// Evaluate when an element is added to the window
    fn on_element(&mut self, ctx: &TriggerContext) -> TriggerResult {
        let _ = ctx;
        TriggerResult::Continue
    }

    /// Evaluate when the watermark advances
    fn on_watermark(&mut self, ctx: &TriggerContext) -> TriggerResult {
        let _ = ctx;
        TriggerResult::Continue
    }

    /// Evaluate on processing time timer
    fn on_processing_time(&mut self, ctx: &TriggerContext) -> TriggerResult {
        let _ = ctx;
        TriggerResult::Continue
    }

    /// Clear any state associated with the window
    fn clear(&mut self, window: &Window) {
        let _ = window;
    }
}

/// Trigger that fires when the watermark passes the end of the window
///
/// This is the standard trigger for event-time windows. It fires when
/// the watermark (which represents the progress of event time) passes
/// the end of the window, indicating that all events for that window
/// have likely been received.
#[derive(Debug, Clone)]
pub struct OnWatermarkTrigger;

impl OnWatermarkTrigger {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OnWatermarkTrigger {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowTrigger for OnWatermarkTrigger {
    fn on_watermark(&mut self, ctx: &TriggerContext) -> TriggerResult {
        if let Some(watermark) = ctx.watermark {
            if watermark >= ctx.window.bounds.end {
                return TriggerResult::Fire;
            }
        }
        TriggerResult::Continue
    }
}

/// Trigger that fires after a specified processing time delay
///
/// This trigger fires when processing time reaches the window end
/// plus a configured delay. Useful for tumbling and sliding windows
/// based on processing time.
#[derive(Debug, Clone)]
pub struct ProcessingTimeTrigger {
    /// Delay after window end before firing
    delay: Duration,
}

impl ProcessingTimeTrigger {
    pub fn new() -> Self {
        Self {
            delay: Duration::zero(),
        }
    }

    pub fn with_delay(delay: Duration) -> Self {
        Self { delay }
    }
}

impl Default for ProcessingTimeTrigger {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowTrigger for ProcessingTimeTrigger {
    fn on_element(&mut self, ctx: &TriggerContext) -> TriggerResult {
        let fire_time = ctx.window.bounds.end + self.delay;
        if ctx.processing_time >= fire_time {
            TriggerResult::Fire
        } else {
            TriggerResult::Continue
        }
    }

    fn on_processing_time(&mut self, ctx: &TriggerContext) -> TriggerResult {
        let fire_time = ctx.window.bounds.end + self.delay;
        if ctx.processing_time >= fire_time {
            TriggerResult::Fire
        } else {
            TriggerResult::Continue
        }
    }
}

/// Trigger that fires when a window contains a specified number of elements
///
/// This trigger maintains a count per window and fires when the count
/// reaches the threshold. Useful for micro-batching and early window
/// evaluation.
#[derive(Debug)]
pub struct CountTrigger {
    /// Number of elements required to trigger
    threshold: usize,
    /// Count per window
    counts: HashMap<String, usize>,
}

impl CountTrigger {
    pub fn new(threshold: usize) -> Self {
        assert!(threshold > 0, "Count threshold must be positive");
        Self {
            threshold,
            counts: HashMap::new(),
        }
    }

    fn increment_count(&mut self, window_id: &str) -> usize {
        let count = self.counts.entry(window_id.to_string()).or_insert(0);
        *count += 1;
        *count
    }
}

impl WindowTrigger for CountTrigger {
    fn on_element(&mut self, ctx: &TriggerContext) -> TriggerResult {
        let count = self.increment_count(&ctx.window.id);
        if count >= self.threshold {
            TriggerResult::Fire
        } else {
            TriggerResult::Continue
        }
    }

    fn clear(&mut self, window: &Window) {
        self.counts.remove(&window.id);
    }
}

/// Composite trigger that combines multiple triggers with AND/OR logic
#[derive(Debug)]
pub struct CompositeTrigger {
    triggers: Vec<Box<dyn WindowTrigger>>,
    mode: CompositeMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositeMode {
    /// Fire when ANY trigger fires
    Any,
    /// Fire when ALL triggers fire
    All,
}

impl CompositeTrigger {
    pub fn any(triggers: Vec<Box<dyn WindowTrigger>>) -> Self {
        assert!(!triggers.is_empty(), "Composite trigger must have at least one trigger");
        Self {
            triggers,
            mode: CompositeMode::Any,
        }
    }

    pub fn all(triggers: Vec<Box<dyn WindowTrigger>>) -> Self {
        assert!(!triggers.is_empty(), "Composite trigger must have at least one trigger");
        Self {
            triggers,
            mode: CompositeMode::All,
        }
    }

    fn evaluate<F>(&mut self, ctx: &TriggerContext, f: F) -> TriggerResult
    where
        F: Fn(&mut Box<dyn WindowTrigger>, &TriggerContext) -> TriggerResult,
    {
        let mut results = Vec::new();

        for trigger in &mut self.triggers {
            let result = f(trigger, ctx);
            results.push(result);
        }

        match self.mode {
            CompositeMode::Any => {
                // Fire if any trigger fires
                if results.iter().any(|r| matches!(r, TriggerResult::Fire | TriggerResult::FireAndPurge)) {
                    TriggerResult::Fire
                } else {
                    TriggerResult::Continue
                }
            }
            CompositeMode::All => {
                // Fire only if all triggers fire
                if results.iter().all(|r| matches!(r, TriggerResult::Fire | TriggerResult::FireAndPurge)) {
                    TriggerResult::Fire
                } else {
                    TriggerResult::Continue
                }
            }
        }
    }
}

impl WindowTrigger for CompositeTrigger {
    fn on_element(&mut self, ctx: &TriggerContext) -> TriggerResult {
        self.evaluate(ctx, |trigger, ctx| trigger.on_element(ctx))
    }

    fn on_watermark(&mut self, ctx: &TriggerContext) -> TriggerResult {
        self.evaluate(ctx, |trigger, ctx| trigger.on_watermark(ctx))
    }

    fn on_processing_time(&mut self, ctx: &TriggerContext) -> TriggerResult {
        self.evaluate(ctx, |trigger, ctx| trigger.on_processing_time(ctx))
    }

    fn clear(&mut self, window: &Window) {
        for trigger in &mut self.triggers {
            trigger.clear(window);
        }
    }
}

/// Trigger that fires on every element (for immediate evaluation)
#[derive(Debug, Clone)]
pub struct ImmediateTrigger;

impl ImmediateTrigger {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ImmediateTrigger {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowTrigger for ImmediateTrigger {
    fn on_element(&mut self, _ctx: &TriggerContext) -> TriggerResult {
        TriggerResult::Fire
    }
}

/// Trigger that never fires automatically (manual trigger only)
#[derive(Debug, Clone)]
pub struct NeverTrigger;

impl NeverTrigger {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NeverTrigger {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowTrigger for NeverTrigger {
    // All methods return Continue by default
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::types::WindowBounds;
    use chrono::TimeZone;

    fn create_timestamp(millis: i64) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(millis).unwrap()
    }

    fn create_window(start_millis: i64, end_millis: i64) -> Window {
        Window::new(WindowBounds::new(
            create_timestamp(start_millis),
            create_timestamp(end_millis),
        ))
    }

    fn create_context(
        window: Window,
        watermark: Option<DateTime<Utc>>,
        processing_time: DateTime<Utc>,
        element_count: usize,
    ) -> TriggerContext {
        TriggerContext {
            window,
            event_time: None,
            processing_time,
            watermark,
            element_count,
        }
    }

    #[test]
    fn test_watermark_trigger() {
        let mut trigger = OnWatermarkTrigger::new();
        let window = create_window(1000, 2000);

        // Watermark before window end -> Continue
        let ctx = create_context(
            window.clone(),
            Some(create_timestamp(1500)),
            create_timestamp(3000),
            5,
        );
        assert_eq!(trigger.on_watermark(&ctx), TriggerResult::Continue);

        // Watermark at window end -> Fire
        let ctx = create_context(
            window.clone(),
            Some(create_timestamp(2000)),
            create_timestamp(3000),
            5,
        );
        assert_eq!(trigger.on_watermark(&ctx), TriggerResult::Fire);

        // Watermark past window end -> Fire
        let ctx = create_context(
            window.clone(),
            Some(create_timestamp(2500)),
            create_timestamp(3000),
            5,
        );
        assert_eq!(trigger.on_watermark(&ctx), TriggerResult::Fire);

        // No watermark -> Continue
        let ctx = create_context(
            window,
            None,
            create_timestamp(3000),
            5,
        );
        assert_eq!(trigger.on_watermark(&ctx), TriggerResult::Continue);
    }

    #[test]
    fn test_processing_time_trigger() {
        let mut trigger = ProcessingTimeTrigger::new();
        let window = create_window(1000, 2000);

        // Processing time before window end -> Continue
        let ctx = create_context(
            window.clone(),
            None,
            create_timestamp(1500),
            5,
        );
        assert_eq!(trigger.on_element(&ctx), TriggerResult::Continue);

        // Processing time at window end -> Fire
        let ctx = create_context(
            window.clone(),
            None,
            create_timestamp(2000),
            5,
        );
        assert_eq!(trigger.on_element(&ctx), TriggerResult::Fire);

        // Processing time past window end -> Fire
        let ctx = create_context(
            window,
            None,
            create_timestamp(2500),
            5,
        );
        assert_eq!(trigger.on_element(&ctx), TriggerResult::Fire);
    }

    #[test]
    fn test_processing_time_trigger_with_delay() {
        let mut trigger = ProcessingTimeTrigger::with_delay(Duration::milliseconds(1000));
        let window = create_window(1000, 2000);

        // Processing time before window end + delay -> Continue
        let ctx = create_context(
            window.clone(),
            None,
            create_timestamp(2500),
            5,
        );
        assert_eq!(trigger.on_element(&ctx), TriggerResult::Continue);

        // Processing time at window end + delay -> Fire
        let ctx = create_context(
            window.clone(),
            None,
            create_timestamp(3000),
            5,
        );
        assert_eq!(trigger.on_element(&ctx), TriggerResult::Fire);
    }

    #[test]
    fn test_count_trigger() {
        let mut trigger = CountTrigger::new(3);
        let window = create_window(1000, 2000);

        // First element -> Continue
        let ctx = create_context(
            window.clone(),
            None,
            create_timestamp(1500),
            1,
        );
        assert_eq!(trigger.on_element(&ctx), TriggerResult::Continue);

        // Second element -> Continue
        let ctx = create_context(
            window.clone(),
            None,
            create_timestamp(1600),
            2,
        );
        assert_eq!(trigger.on_element(&ctx), TriggerResult::Continue);

        // Third element -> Fire
        let ctx = create_context(
            window.clone(),
            None,
            create_timestamp(1700),
            3,
        );
        assert_eq!(trigger.on_element(&ctx), TriggerResult::Fire);

        // Fourth element -> Fire (threshold already met)
        let ctx = create_context(
            window.clone(),
            None,
            create_timestamp(1800),
            4,
        );
        assert_eq!(trigger.on_element(&ctx), TriggerResult::Fire);

        // Clear and start over
        trigger.clear(&window);
        let ctx = create_context(
            window.clone(),
            None,
            create_timestamp(1900),
            1,
        );
        assert_eq!(trigger.on_element(&ctx), TriggerResult::Continue);
    }

    #[test]
    fn test_composite_trigger_any() {
        let count_trigger: Box<dyn WindowTrigger> = Box::new(CountTrigger::new(5));
        let watermark_trigger: Box<dyn WindowTrigger> = Box::new(OnWatermarkTrigger::new());

        let mut trigger = CompositeTrigger::any(vec![count_trigger, watermark_trigger]);
        let window = create_window(1000, 2000);

        // Neither trigger fires -> Continue
        let ctx = create_context(
            window.clone(),
            Some(create_timestamp(1500)),
            create_timestamp(1500),
            1,
        );
        assert_eq!(trigger.on_watermark(&ctx), TriggerResult::Continue);

        // Watermark trigger fires (watermark past window end) -> Fire
        let ctx = create_context(
            window.clone(),
            Some(create_timestamp(2500)),
            create_timestamp(1500),
            2,
        );
        assert_eq!(trigger.on_watermark(&ctx), TriggerResult::Fire);
    }

    #[test]
    fn test_composite_trigger_all() {
        // Use two processing time triggers with different delays
        let trigger1: Box<dyn WindowTrigger> = Box::new(ProcessingTimeTrigger::new());
        let trigger2: Box<dyn WindowTrigger> = Box::new(ProcessingTimeTrigger::with_delay(Duration::milliseconds(500)));

        let mut trigger = CompositeTrigger::all(vec![trigger1, trigger2]);
        let window = create_window(1000, 2000);

        // Processing time at window end -> Continue (only first trigger fires)
        let ctx = create_context(
            window.clone(),
            None,
            create_timestamp(2000),
            1,
        );
        assert_eq!(trigger.on_processing_time(&ctx), TriggerResult::Continue);

        // Processing time at window end + 500ms -> Fire (both triggers fire)
        let ctx = create_context(
            window.clone(),
            None,
            create_timestamp(2500),
            1,
        );
        assert_eq!(trigger.on_processing_time(&ctx), TriggerResult::Fire);
    }

    #[test]
    fn test_immediate_trigger() {
        let mut trigger = ImmediateTrigger::new();
        let window = create_window(1000, 2000);

        let ctx = create_context(
            window,
            None,
            create_timestamp(1500),
            1,
        );
        assert_eq!(trigger.on_element(&ctx), TriggerResult::Fire);
    }

    #[test]
    fn test_never_trigger() {
        let mut trigger = NeverTrigger::new();
        let window = create_window(1000, 2000);

        let ctx = create_context(
            window,
            Some(create_timestamp(5000)),
            create_timestamp(5000),
            100,
        );

        assert_eq!(trigger.on_element(&ctx), TriggerResult::Continue);
        assert_eq!(trigger.on_watermark(&ctx), TriggerResult::Continue);
        assert_eq!(trigger.on_processing_time(&ctx), TriggerResult::Continue);
    }

    #[test]
    #[should_panic(expected = "Count threshold must be positive")]
    fn test_count_trigger_invalid_threshold() {
        CountTrigger::new(0);
    }

    #[test]
    #[should_panic(expected = "Composite trigger must have at least one trigger")]
    fn test_composite_trigger_empty() {
        CompositeTrigger::any(vec![]);
    }
}
