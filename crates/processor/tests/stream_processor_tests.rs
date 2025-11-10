//! Comprehensive tests for Stream Processor implementation
//!
//! This test suite provides comprehensive coverage for the stream processor,
//! including unit tests, integration tests, performance benchmarks, and edge case testing.
//!
//! Test Coverage:
//! - Core event processing components
//! - Window lifecycle and management
//! - Statistical aggregations
//! - Watermarking and late events
//! - State persistence and recovery
//! - End-to-end processing flows
//! - Performance under load
//! - Error handling and recovery

use chrono::{DateTime, Duration, TimeZone, Utc};
use processor::aggregation::*;
use processor::core::*;
use processor::error::*;
use processor::state::*;
use processor::watermark::*;
use processor::window::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use llm_optimizer_types::events::{CloudEvent, EventSource, EventType, FeedbackEvent};
use llm_optimizer_types::metrics::MetricPoint;

// ============================================================================
// UNIT TESTS - Core Components
// ============================================================================

mod core_tests {
    use super::*;

    #[test]
    fn test_processor_event_creation() {
        let event_time = Utc::now();
        let event = ProcessorEvent::new(
            "test_payload".to_string(),
            event_time,
            "test_key".to_string(),
        );

        assert_eq!(event.event, "test_payload");
        assert_eq!(event.event_time, event_time);
        assert_eq!(event.key, "test_key");
        assert!(event.processing_time >= event_time);
    }

    #[test]
    fn test_processor_event_with_custom_processing_time() {
        let event_time = Utc::now() - Duration::seconds(10);
        let processing_time = Utc::now();

        let event = ProcessorEvent::with_processing_time(
            42,
            event_time,
            processing_time,
            "key".to_string(),
        );

        assert_eq!(event.event, 42);
        assert_eq!(event.event_time, event_time);
        assert_eq!(event.processing_time, processing_time);
    }

    #[test]
    fn test_processor_event_delay_calculation() {
        let event_time = Utc::now() - Duration::seconds(30);
        let processing_time = Utc::now();

        let event = ProcessorEvent::with_processing_time(
            "test".to_string(),
            event_time,
            processing_time,
            "key".to_string(),
        );

        let delay = event.processing_delay();
        assert!(delay.num_seconds() >= 29 && delay.num_seconds() <= 31);
        assert!(event.is_late(Duration::seconds(10)));
        assert!(!event.is_late(Duration::seconds(60)));
    }

    #[test]
    fn test_processor_event_map() {
        let event = ProcessorEvent::new(100, Utc::now(), "key".to_string());
        let mapped = event.map(|n| format!("value_{}", n));

        assert_eq!(mapped.event, "value_100");
        assert_eq!(mapped.key, "key");
    }

    #[test]
    fn test_event_key_types() {
        // Model key
        let model_key = EventKey::Model("claude-3-opus".to_string());
        assert_eq!(model_key.to_key_string(), "model:claude-3-opus");

        // Session key
        let session_key = EventKey::Session("sess-123".to_string());
        assert_eq!(session_key.to_key_string(), "session:sess-123");

        // User key
        let user_key = EventKey::User("user-456".to_string());
        assert_eq!(user_key.to_key_string(), "user:user-456");
    }

    #[test]
    fn test_event_key_composite() {
        let key = EventKey::composite(vec![
            ("model", "claude-3-opus"),
            ("session", "sess-1"),
            ("region", "us-west"),
        ]);

        let key_str = key.to_key_string();
        assert!(key_str.starts_with("composite:"));
        assert!(key_str.contains("model:claude-3-opus"));
        assert!(key_str.contains("session:sess-1"));
        assert!(key_str.contains("region:us-west"));
    }

    #[test]
    fn test_event_key_composite_ordering() {
        // Keys should be deterministic regardless of input order
        let key1 = EventKey::composite(vec![("a", "1"), ("b", "2"), ("c", "3")]);
        let key2 = EventKey::composite(vec![("c", "3"), ("a", "1"), ("b", "2")]);

        assert_eq!(key1.to_key_string(), key2.to_key_string());
    }

    #[test]
    fn test_event_key_dimension_extraction() {
        let key = EventKey::composite(vec![
            ("model", "claude-3-opus"),
            ("session", "sess-1"),
        ]);

        assert_eq!(key.get_dimension("model"), Some("claude-3-opus".to_string()));
        assert_eq!(key.get_dimension("session"), Some("sess-1".to_string()));
        assert_eq!(key.get_dimension("user"), None);
    }

    #[test]
    fn test_feedback_event_key_extractor() {
        let extractor = FeedbackEventKeyExtractor;

        // Test with model_id
        let mut event1 = FeedbackEvent::new(
            EventSource::Observatory,
            EventType::Metric,
            serde_json::json!({}),
        );
        event1.metadata.insert("model_id".to_string(), "claude-3-opus".to_string());

        let key1 = extractor.extract_key(&event1);
        match key1 {
            EventKey::Model(id) => assert_eq!(id, "claude-3-opus"),
            _ => panic!("Expected Model key"),
        }

        // Test with model_id and session_id (composite)
        event1.metadata.insert("session_id".to_string(), "sess-123".to_string());
        let key2 = extractor.extract_key(&event1);
        match key2 {
            EventKey::Composite(parts) => {
                assert_eq!(parts.len(), 2);
            }
            _ => panic!("Expected Composite key"),
        }

        // Test with only session_id
        let mut event2 = FeedbackEvent::new(
            EventSource::User,
            EventType::Feedback,
            serde_json::json!({}),
        );
        event2.metadata.insert("session_id".to_string(), "sess-456".to_string());
        let key3 = extractor.extract_key(&event2);
        match key3 {
            EventKey::Session(id) => assert_eq!(id, "sess-456"),
            _ => panic!("Expected Session key"),
        }
    }

    #[test]
    fn test_metric_point_extractors() {
        let time_extractor = MetricPointTimeExtractor;
        let key_extractor = MetricPointKeyExtractor;

        let timestamp = Utc::now();
        let mut tags = HashMap::new();
        tags.insert("model".to_string(), "claude-3-opus".to_string());

        let point = MetricPoint {
            name: "latency".to_string(),
            value: 123.45,
            unit: "ms".to_string(),
            timestamp,
            tags,
            aggregation: None,
        };

        // Test time extraction
        assert_eq!(time_extractor.extract_event_time(&point), timestamp);

        // Test key extraction
        let key = key_extractor.extract_key(&point);
        match key {
            EventKey::Model(id) => assert_eq!(id, "claude-3-opus"),
            _ => panic!("Expected Model key"),
        }
    }
}

// ============================================================================
// UNIT TESTS - Window Functionality
// ============================================================================

mod window_tests {
    use super::*;

    fn create_timestamp(millis: i64) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(millis).unwrap()
    }

    #[test]
    fn test_tumbling_window_assignment() {
        let assigner = TumblingWindowAssigner::new(Duration::milliseconds(1000));

        // Event at 500ms should be in window [0, 1000)
        let windows = assigner.assign_windows(create_timestamp(500));
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].bounds.start, create_timestamp(0));
        assert_eq!(windows[0].bounds.end, create_timestamp(1000));

        // Event at 1500ms should be in window [1000, 2000)
        let windows = assigner.assign_windows(create_timestamp(1500));
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].bounds.start, create_timestamp(1000));
        assert_eq!(windows[0].bounds.end, create_timestamp(2000));
    }

    #[test]
    fn test_tumbling_window_non_overlapping() {
        let assigner = TumblingWindowAssigner::new(Duration::milliseconds(1000));

        let windows1 = assigner.assign_windows(create_timestamp(500));
        let windows2 = assigner.assign_windows(create_timestamp(1500));

        // Windows should not overlap
        assert!(windows1[0].bounds.end <= windows2[0].bounds.start);
    }

    #[test]
    fn test_sliding_window_assignment() {
        let assigner = SlidingWindowAssigner::new(
            Duration::milliseconds(1000), // window size
            Duration::milliseconds(500),  // slide
        );

        // Event at 700ms should be in 2 overlapping windows
        let windows = assigner.assign_windows(create_timestamp(700));
        assert_eq!(windows.len(), 2);

        // First window: [0, 1000)
        assert_eq!(windows[0].bounds.start, create_timestamp(0));
        assert_eq!(windows[0].bounds.end, create_timestamp(1000));

        // Second window: [500, 1500)
        assert_eq!(windows[1].bounds.start, create_timestamp(500));
        assert_eq!(windows[1].bounds.end, create_timestamp(1500));
    }

    #[test]
    fn test_sliding_window_overlapping() {
        let assigner = SlidingWindowAssigner::new(
            Duration::milliseconds(2000),
            Duration::milliseconds(1000),
        );

        // Event at 2500ms should be in multiple windows
        let windows = assigner.assign_windows(create_timestamp(2500));
        assert!(windows.len() >= 2);

        // Verify windows overlap
        for i in 0..windows.len() - 1 {
            assert!(windows[i].bounds.end > windows[i + 1].bounds.start);
        }
    }

    #[test]
    fn test_session_window_assignment() {
        let assigner = SessionWindowAssigner::new(Duration::milliseconds(1000));

        // First event creates a window
        let windows1 = assigner.assign_windows(create_timestamp(0));
        assert_eq!(windows1.len(), 1);
        assert_eq!(windows1[0].bounds.start, create_timestamp(0));
        assert_eq!(windows1[0].bounds.end, create_timestamp(1000));

        // Event within gap extends the window
        let windows2 = assigner.assign_windows(create_timestamp(500));
        assert_eq!(windows2[0].bounds.start, create_timestamp(500));
        assert_eq!(windows2[0].bounds.end, create_timestamp(1500));
    }

    #[test]
    fn test_session_window_merger() {
        let mut merger = WindowMerger::new(Duration::milliseconds(1000));

        // Add first window
        let window1 = Window::new(WindowBounds::new(
            create_timestamp(0),
            create_timestamp(1000),
        ));
        let (result1, merged1) = merger.add_and_merge(window1.clone());
        assert_eq!(result1.bounds.start, create_timestamp(0));
        assert_eq!(merged1.len(), 0);

        // Add overlapping window - should merge
        let window2 = Window::new(WindowBounds::new(
            create_timestamp(500),
            create_timestamp(1500),
        ));
        let (result2, merged2) = merger.add_and_merge(window2);
        assert_eq!(result2.bounds.start, create_timestamp(0));
        assert_eq!(result2.bounds.end, create_timestamp(1500));
        assert_eq!(merged2.len(), 1);
    }

    #[test]
    fn test_session_window_gap_handling() {
        let mut merger = WindowMerger::new(Duration::milliseconds(1000));

        // Add first window
        let window1 = Window::new(WindowBounds::new(
            create_timestamp(0),
            create_timestamp(1000),
        ));
        merger.add_and_merge(window1);

        // Add window beyond gap - should not merge
        let window2 = Window::new(WindowBounds::new(
            create_timestamp(3000),
            create_timestamp(4000),
        ));
        let (result, merged) = merger.add_and_merge(window2.clone());
        assert_eq!(result, window2);
        assert_eq!(merged.len(), 0);
        assert_eq!(merger.windows().len(), 2);
    }

    #[test]
    fn test_window_trigger_on_watermark() {
        let mut trigger = OnWatermarkTrigger::new();

        let window = Window::new(WindowBounds::new(
            create_timestamp(0),
            create_timestamp(1000),
        ));

        // Watermark before window end - should continue
        let ctx = TriggerContext {
            window: window.clone(),
            event_time: Some(create_timestamp(500)),
            processing_time: create_timestamp(1500),
            watermark: Some(create_timestamp(900)),
            element_count: 1,
        };
        assert_eq!(trigger.on_watermark(&ctx), TriggerResult::Continue);

        // Watermark past window end - should fire
        let ctx = TriggerContext {
            window: window.clone(),
            event_time: Some(create_timestamp(500)),
            processing_time: create_timestamp(1500),
            watermark: Some(create_timestamp(1000)),
            element_count: 1,
        };
        assert_eq!(trigger.on_watermark(&ctx), TriggerResult::Fire);
    }

    #[test]
    fn test_window_trigger_count() {
        let mut trigger = CountTrigger::new(5);

        let window = Window::new(WindowBounds::new(
            create_timestamp(0),
            create_timestamp(1000),
        ));

        // Less than count - should continue
        let ctx = TriggerContext {
            window: window.clone(),
            event_time: Some(create_timestamp(500)),
            processing_time: create_timestamp(500),
            watermark: None,
            element_count: 3,
        };
        assert_eq!(trigger.on_element(&ctx), TriggerResult::Continue);

        // Reached count - should fire
        let ctx = TriggerContext {
            window: window.clone(),
            event_time: Some(create_timestamp(500)),
            processing_time: create_timestamp(500),
            watermark: None,
            element_count: 5,
        };
        assert_eq!(trigger.on_element(&ctx), TriggerResult::Fire);
    }

    #[test]
    fn test_window_lifecycle() {
        let assigner = TumblingWindowAssigner::new(Duration::milliseconds(1000));
        let mut trigger = OnWatermarkTrigger::new();

        // Assign window
        let windows = assigner.assign_windows(create_timestamp(500));
        assert_eq!(windows.len(), 1);

        // Process events in window
        let window = &windows[0];
        let mut event_count = 0;

        for event_time in &[100, 300, 500, 700, 900] {
            event_count += 1;
            let ctx = TriggerContext {
                window: window.clone(),
                event_time: Some(create_timestamp(*event_time)),
                processing_time: create_timestamp(*event_time),
                watermark: Some(create_timestamp(*event_time)),
                element_count: event_count,
            };

            // Should not fire until watermark passes window end
            assert_eq!(trigger.on_element(&ctx), TriggerResult::Continue);
        }

        // Watermark advances past window
        let ctx = TriggerContext {
            window: window.clone(),
            event_time: Some(create_timestamp(1100)),
            processing_time: create_timestamp(1100),
            watermark: Some(create_timestamp(1000)),
            element_count: event_count,
        };
        assert_eq!(trigger.on_watermark(&ctx), TriggerResult::Fire);
    }
}

// ============================================================================
// UNIT TESTS - Aggregations
// ============================================================================

mod aggregation_tests {
    use super::*;

    #[test]
    fn test_count_aggregator() {
        let mut agg = CountAggregator::new();

        assert_eq!(agg.count(), 0);

        agg.update(1.0).unwrap();
        agg.update(2.0).unwrap();
        agg.update(3.0).unwrap();

        assert_eq!(agg.count(), 3);
        assert_eq!(agg.finalize().unwrap(), 3);
    }

    #[test]
    fn test_sum_aggregator() {
        let mut agg = SumAggregator::new();

        agg.update(10.0).unwrap();
        agg.update(20.0).unwrap();
        agg.update(30.0).unwrap();

        assert_eq!(agg.finalize().unwrap(), 60.0);
    }

    #[test]
    fn test_average_aggregator() {
        let mut agg = AverageAggregator::new();

        agg.update(10.0).unwrap();
        agg.update(20.0).unwrap();
        agg.update(30.0).unwrap();

        assert_eq!(agg.finalize().unwrap(), 20.0);
        assert_eq!(agg.count(), 3);
    }

    #[test]
    fn test_min_max_aggregators() {
        let mut min_agg = MinAggregator::new();
        let mut max_agg = MaxAggregator::new();

        let values = vec![50.0, 10.0, 30.0, 90.0, 20.0];
        for value in &values {
            min_agg.update(*value).unwrap();
            max_agg.update(*value).unwrap();
        }

        assert_eq!(min_agg.finalize().unwrap(), 10.0);
        assert_eq!(max_agg.finalize().unwrap(), 90.0);
    }

    #[test]
    fn test_percentile_aggregator() {
        let mut p50_agg = PercentileAggregator::p50();
        let mut p95_agg = PercentileAggregator::p95();
        let mut p99_agg = PercentileAggregator::p99();

        // Feed values 1-100
        for i in 1..=100 {
            p50_agg.update(i as f64).unwrap();
            p95_agg.update(i as f64).unwrap();
            p99_agg.update(i as f64).unwrap();
        }

        let p50 = p50_agg.finalize().unwrap();
        let p95 = p95_agg.finalize().unwrap();
        let p99 = p99_agg.finalize().unwrap();

        // P50 should be around 50
        assert!((p50 - 50.0).abs() < 5.0);
        // P95 should be around 95
        assert!((p95 - 95.0).abs() < 5.0);
        // P99 should be around 99
        assert!((p99 - 99.0).abs() < 5.0);
    }

    #[test]
    fn test_stddev_aggregator() {
        let mut agg = StandardDeviationAggregator::new();

        // Add values with known stddev
        // Values: 2, 4, 4, 4, 5, 5, 7, 9
        // Mean: 5
        // Variance: 4
        // Stddev: 2
        let values = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        for value in &values {
            agg.update(*value).unwrap();
        }

        let stddev = agg.finalize().unwrap();
        assert!((stddev - 2.0).abs() < 0.1);
    }

    #[test]
    fn test_aggregator_batch_update() {
        let mut agg = AverageAggregator::new();

        let batch = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        agg.update_batch(&batch).unwrap();

        assert_eq!(agg.count(), 5);
        assert_eq!(agg.finalize().unwrap(), 30.0);
    }

    #[test]
    fn test_aggregator_merge() {
        let mut agg1 = SumAggregator::new();
        agg1.update_batch(&[1.0, 2.0, 3.0]).unwrap();

        let mut agg2 = SumAggregator::new();
        agg2.update_batch(&[4.0, 5.0, 6.0]).unwrap();

        // Merge agg2 into agg1
        let acc2 = agg2.accumulator();
        agg1.merge(acc2).unwrap();

        assert_eq!(agg1.finalize().unwrap(), 21.0);
    }

    #[test]
    fn test_composite_aggregator() {
        let mut composite = CompositeAggregator::new();
        composite
            .add_count("count")
            .add_sum("sum")
            .add_avg("avg")
            .add_min("min")
            .add_max("max")
            .add_p95("p95");

        let values = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        for value in &values {
            composite.update(*value).unwrap();
        }

        let results = composite.finalize().unwrap();

        assert_eq!(results.get_count("count").unwrap(), 5);
        assert_eq!(results.get_value("sum").unwrap(), 150.0);
        assert_eq!(results.get_value("avg").unwrap(), 30.0);
        assert_eq!(results.get_value("min").unwrap(), 10.0);
        assert_eq!(results.get_value("max").unwrap(), 50.0);
    }

    #[test]
    fn test_aggregator_serialization() {
        let mut agg = AverageAggregator::<f64>::new();
        agg.update_batch(&[10.0, 20.0, 30.0]).unwrap();

        // Serialize
        let acc = agg.accumulator();
        let json = serde_json::to_string(&acc).unwrap();

        // Deserialize and merge
        let deserialized: AverageAccumulator = serde_json::from_str(&json).unwrap();
        let mut new_agg = AverageAggregator::<f64>::new();
        new_agg.merge(deserialized).unwrap();

        assert_eq!(new_agg.finalize().unwrap(), 20.0);
    }

    #[test]
    fn test_aggregator_reset() {
        let mut agg = CountAggregator::new();
        agg.update_batch(&[1.0, 2.0, 3.0]).unwrap();

        assert_eq!(agg.count(), 3);

        agg.reset();
        assert_eq!(agg.count(), 0);
    }

    #[test]
    fn test_aggregation_accuracy() {
        // Test with large dataset for accuracy
        let mut avg_agg = AverageAggregator::new();
        let mut sum_agg = SumAggregator::new();

        let mut expected_sum = 0.0;
        let count = 10000;

        for i in 1..=count {
            let value = i as f64;
            avg_agg.update(value).unwrap();
            sum_agg.update(value).unwrap();
            expected_sum += value;
        }

        let expected_avg = expected_sum / count as f64;

        assert_eq!(sum_agg.finalize().unwrap(), expected_sum);
        assert!((avg_agg.finalize().unwrap() - expected_avg).abs() < 0.001);
    }
}

// ============================================================================
// UNIT TESTS - Watermarking and Late Events
// ============================================================================

mod watermark_tests {
    use super::*;
    use std::time::Duration as StdDuration;

    #[test]
    fn test_watermark_creation() {
        let wm = Watermark::new(1000);
        assert_eq!(wm.timestamp, 1000);
    }

    #[test]
    fn test_watermark_ordering() {
        let wm1 = Watermark::new(1000);
        let wm2 = Watermark::new(2000);

        assert!(wm1 < wm2);
        assert!(wm2 > wm1);
        assert!(wm1.is_before(2000));
        assert!(wm2.is_after(1000));
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
    fn test_bounded_watermark_generator() {
        let mut generator = BoundedOutOfOrdernessWatermark::new(
            StdDuration::from_secs(5),
            None,
        );

        generator.on_event(10000, 0);
        generator.on_event(15000, 0);
        generator.on_event(20000, 0);

        let wm = generator.current_watermark();
        // Watermark = max_timestamp - max_out_of_orderness
        // = 20000 - 5000 = 15000
        assert_eq!(wm.timestamp, 15000);
    }

    #[test]
    fn test_bounded_watermark_out_of_order_events() {
        let mut generator = BoundedOutOfOrdernessWatermark::new(
            StdDuration::from_secs(10),
            None,
        );

        // Events arrive out of order
        generator.on_event(30000, 0);
        generator.on_event(20000, 0);
        generator.on_event(25000, 0);
        generator.on_event(35000, 0);

        let wm = generator.current_watermark();
        // Watermark based on max timestamp: 35000 - 10000 = 25000
        assert_eq!(wm.timestamp, 25000);
    }

    #[test]
    fn test_bounded_watermark_multi_partition() {
        let mut generator = BoundedOutOfOrdernessWatermark::new(
            StdDuration::from_secs(5),
            None,
        );

        // Different partitions with different timestamps
        generator.on_event(10000, 0); // Partition 0: watermark = 5000
        generator.on_event(20000, 1); // Partition 1: watermark = 15000
        generator.on_event(15000, 2); // Partition 2: watermark = 10000

        // Global watermark should be min of all partitions = 5000
        let wm = generator.current_watermark();
        assert_eq!(wm.timestamp, 5000);

        // Verify per-partition watermarks
        assert_eq!(generator.partition_watermark(0).unwrap().timestamp, 5000);
        assert_eq!(generator.partition_watermark(1).unwrap().timestamp, 15000);
        assert_eq!(generator.partition_watermark(2).unwrap().timestamp, 10000);
    }

    #[test]
    fn test_late_event_detection() {
        let mut generator = BoundedOutOfOrdernessWatermark::new(
            StdDuration::from_secs(5),
            None,
        );

        generator.on_event(20000, 0);

        // Current watermark is 15000
        assert!(generator.is_late_event(10000)); // Late by 5000ms
        assert!(!generator.is_late_event(16000)); // Not late

        assert_eq!(generator.get_lateness(10000), 5000);
        assert_eq!(generator.get_lateness(16000), 0);
    }

    #[test]
    fn test_late_event_handler() {
        let config = LateEventConfig {
            allowed_lateness: StdDuration::from_secs(10),
            log_late_events: false,
            emit_metrics: true,
        };
        let handler = LateEventHandler::new(config);

        // Event within allowed lateness - accept
        assert!(handler.handle_late_event(5000, 0));

        // Event beyond allowed lateness - drop
        assert!(!handler.handle_late_event(15000, 0));

        let stats = handler.get_stats();
        assert_eq!(stats.accepted_count, 1);
        assert_eq!(stats.dropped_count, 1);
    }

    #[test]
    fn test_watermark_monotonicity() {
        let mut generator = BoundedOutOfOrdernessWatermark::new(
            StdDuration::from_secs(5),
            None,
        );

        generator.on_event(20000, 0);
        let wm1 = generator.current_watermark();

        // Older event arrives - watermark should not go backwards
        generator.on_event(15000, 0);
        let wm2 = generator.current_watermark();

        assert!(wm2 >= wm1);
    }

    #[test]
    fn test_periodic_watermark() {
        let config = PeriodicWatermarkConfig {
            interval: StdDuration::from_millis(100),
            max_out_of_orderness: StdDuration::from_secs(5),
        };
        let mut generator = PeriodicWatermark::new(config);

        generator.on_event(10000, 0);
        generator.on_event(20000, 0);

        // Should not emit on event
        assert!(generator.on_event(30000, 0).is_none());

        // Sleep and check periodically
        std::thread::sleep(StdDuration::from_millis(150));
        let wm = generator.on_periodic_check();
        assert!(wm.is_some());
    }

    #[test]
    fn test_punctuated_watermark() {
        let mut generator = PunctuatedWatermark::every_n_events(5);

        // First 4 events should not emit
        for i in 1..=4 {
            assert!(generator.on_event(i * 1000, 0).is_none());
        }

        // 5th event should emit
        let wm = generator.on_event(5000, 0);
        assert!(wm.is_some());
        assert_eq!(wm.unwrap().timestamp, 5000);
    }

    #[test]
    fn test_watermark_reset() {
        let mut generator = BoundedOutOfOrdernessWatermark::new(
            StdDuration::from_secs(5),
            None,
        );

        generator.on_event(20000, 0);
        assert_ne!(generator.current_watermark(), Watermark::min());

        generator.reset();
        assert_eq!(generator.current_watermark(), Watermark::min());
    }
}

// ============================================================================
// INTEGRATION TESTS - End-to-End Flows
// ============================================================================

mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_end_to_end_windowed_aggregation() {
        // Simulate a complete windowed aggregation flow
        let assigner = TumblingWindowAssigner::new(Duration::milliseconds(1000));
        let mut trigger = OnWatermarkTrigger::new();
        let mut aggregator = AverageAggregator::new();

        // Generate events
        let events = vec![
            (100, 10.0),
            (200, 20.0),
            (300, 30.0),
            (500, 40.0),
            (800, 50.0),
        ];

        // Assign to window
        let window = assigner.assign_windows(Utc.timestamp_millis_opt(500).unwrap())[0].clone();

        // Accumulate values
        for (_ts, value) in &events {
            aggregator.update(*value).unwrap();
        }

        // Trigger window
        let ctx = TriggerContext {
            window: window.clone(),
            event_time: Some(Utc.timestamp_millis_opt(1000).unwrap()),
            processing_time: Utc.timestamp_millis_opt(1100).unwrap(),
            watermark: Some(Utc.timestamp_millis_opt(1000).unwrap()),
            element_count: events.len(),
        };

        if trigger.on_watermark(&ctx) == TriggerResult::Fire {
            let result = aggregator.finalize().unwrap();
            assert_eq!(result, 30.0); // Average of 10, 20, 30, 40, 50
        }
    }

    #[tokio::test]
    async fn test_multi_window_aggregation() {
        // Test sliding windows with overlapping aggregations
        let assigner = SlidingWindowAssigner::new(
            Duration::milliseconds(2000), // window size
            Duration::milliseconds(1000), // slide
        );

        let mut window_results = HashMap::new();

        // Process events
        let events = vec![
            (500, 10.0),
            (1500, 20.0),
            (2500, 30.0),
        ];

        for (ts, value) in events {
            let timestamp = Utc.timestamp_millis_opt(ts).unwrap();
            let windows = assigner.assign_windows(timestamp);

            for window in windows {
                let agg = window_results
                    .entry(window.id.clone())
                    .or_insert_with(|| AverageAggregator::new());
                agg.update(value).unwrap();
            }
        }

        // Verify multiple windows received data
        assert!(window_results.len() >= 2);
    }

    #[tokio::test]
    async fn test_session_window_with_merging() {
        let assigner = SessionWindowAssigner::new(Duration::milliseconds(1000));
        let mut merger = WindowMerger::new(Duration::milliseconds(1000));

        // Simulate session events
        let events = vec![0, 300, 600, 2500, 2800]; // Gap between 600 and 2500

        let mut sessions = Vec::new();

        for ts in events {
            let timestamp = Utc.timestamp_millis_opt(ts).unwrap();
            let windows = assigner.assign_windows(timestamp);
            let (merged_window, _) = merger.add_and_merge(windows[0].clone());
            sessions.push(merged_window);
        }

        // Should have 2 distinct sessions
        let unique_windows: std::collections::HashSet<_> =
            sessions.iter().map(|w| w.id.clone()).collect();
        assert_eq!(unique_windows.len(), 2);
    }

    #[tokio::test]
    async fn test_late_event_handling_flow() {
        let mut generator = BoundedOutOfOrdernessWatermark::new(
            StdDuration::from_secs(5),
            None,
        );

        let handler = LateEventHandler::new(LateEventConfig {
            allowed_lateness: StdDuration::from_secs(10),
            log_late_events: false,
            emit_metrics: true,
        });

        // Process events
        let events = vec![
            (10000, 0),
            (15000, 0),
            (20000, 0),
            (12000, 0), // Late event
            (5000, 0),  // Very late event
        ];

        let mut processed = 0;
        let mut dropped = 0;

        for (ts, partition) in events {
            generator.on_event(ts, partition);

            if generator.is_late_event(ts) {
                let lateness = generator.get_lateness(ts);
                if handler.handle_late_event(lateness, partition) {
                    processed += 1;
                } else {
                    dropped += 1;
                }
            } else {
                processed += 1;
            }
        }

        assert!(processed > 0);
        assert!(dropped > 0);
    }

    #[tokio::test]
    async fn test_composite_aggregation_flow() {
        let mut composite = CompositeAggregator::new();
        composite
            .add_count("request_count")
            .add_avg("avg_latency")
            .add_min("min_latency")
            .add_max("max_latency")
            .add_p50("p50_latency")
            .add_p95("p95_latency")
            .add_p99("p99_latency")
            .add_stddev("stddev_latency");

        // Simulate latency measurements
        let latencies = vec![
            100.0, 120.0, 95.0, 110.0, 105.0, 200.0, 115.0, 130.0, 98.0, 125.0,
        ];

        for latency in &latencies {
            composite.update(*latency).unwrap();
        }

        let results = composite.finalize().unwrap();

        assert_eq!(results.get_count("request_count").unwrap(), 10);
        assert!(results.get_value("avg_latency").unwrap() > 0.0);
        assert_eq!(results.get_value("min_latency").unwrap(), 95.0);
        assert_eq!(results.get_value("max_latency").unwrap(), 200.0);
        assert!(results.get_value("p95_latency").unwrap() > results.get_value("p50_latency").unwrap());
    }

    #[tokio::test]
    async fn test_distributed_aggregation() {
        // Simulate distributed processing across multiple workers
        let worker_count = 4;
        let values_per_worker = 1000;

        let mut aggregators: Vec<AverageAggregator<f64>> = (0..worker_count)
            .map(|_| AverageAggregator::new())
            .collect();

        // Each worker processes its partition
        for (i, agg) in aggregators.iter_mut().enumerate() {
            let start = i * values_per_worker;
            let values: Vec<f64> = (start..start + values_per_worker)
                .map(|n| n as f64)
                .collect();
            agg.update_batch(&values).unwrap();
        }

        // Merge all workers' results
        let mut final_agg = AverageAggregator::new();
        for agg in aggregators {
            final_agg.merge(agg.accumulator()).unwrap();
        }

        let result = final_agg.finalize().unwrap();
        let expected = ((worker_count * values_per_worker) - 1) as f64 / 2.0;

        assert!((result - expected).abs() < 1.0);
    }
}

// ============================================================================
// STATE PERSISTENCE AND RECOVERY TESTS
// ============================================================================

mod state_tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_backend_basic_operations() {
        let backend = MemoryStateBackend::new(None);

        // Put and get
        backend.put(b"key1", b"value1").await.unwrap();
        let value = backend.get(b"key1").await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));

        // Delete
        backend.delete(b"key1").await.unwrap();
        let value = backend.get(b"key1").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_memory_backend_with_ttl() {
        let backend = MemoryStateBackend::new(Some(StdDuration::from_millis(100)));

        backend.put(b"key1", b"value1").await.unwrap();

        // Value should exist immediately
        let value = backend.get(b"key1").await.unwrap();
        assert!(value.is_some());

        // Wait for TTL expiration
        tokio::time::sleep(StdDuration::from_millis(150)).await;

        // Value should be expired
        let value = backend.get(b"key1").await.unwrap();
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn test_state_backend_list_keys() {
        let backend = MemoryStateBackend::new(None);

        // Insert keys with prefix
        backend.put(b"window:1", b"data1").await.unwrap();
        backend.put(b"window:2", b"data2").await.unwrap();
        backend.put(b"window:3", b"data3").await.unwrap();
        backend.put(b"other:1", b"data4").await.unwrap();

        // List keys with prefix
        let keys = backend.list_keys(b"window:").await.unwrap();
        assert_eq!(keys.len(), 3);
    }

    #[tokio::test]
    async fn test_state_backend_clear() {
        let backend = MemoryStateBackend::new(None);

        backend.put(b"key1", b"value1").await.unwrap();
        backend.put(b"key2", b"value2").await.unwrap();

        backend.clear().await.unwrap();

        let value1 = backend.get(b"key1").await.unwrap();
        let value2 = backend.get(b"key2").await.unwrap();

        assert!(value1.is_none());
        assert!(value2.is_none());
    }

    #[tokio::test]
    async fn test_aggregator_state_persistence() {
        let backend = MemoryStateBackend::new(None);

        // Create and populate aggregator
        let mut agg = AverageAggregator::new();
        agg.update_batch(&[10.0, 20.0, 30.0]).unwrap();

        // Serialize and store
        let acc = agg.accumulator();
        let serialized = serde_json::to_vec(&acc).unwrap();
        backend.put(b"agg:window1", &serialized).await.unwrap();

        // Retrieve and deserialize
        let stored = backend.get(b"agg:window1").await.unwrap().unwrap();
        let deserialized: AverageAccumulator = serde_json::from_slice(&stored).unwrap();

        // Create new aggregator from stored state
        let mut restored_agg = AverageAggregator::new();
        restored_agg.merge(deserialized).unwrap();

        assert_eq!(restored_agg.finalize().unwrap(), 20.0);
        assert_eq!(restored_agg.count(), 3);
    }

    #[tokio::test]
    async fn test_window_state_persistence() {
        let backend = MemoryStateBackend::new(None);

        // Create window with aggregations
        let window = Window::new(WindowBounds::new(
            Utc.timestamp_millis_opt(0).unwrap(),
            Utc.timestamp_millis_opt(1000).unwrap(),
        ));

        let mut agg = CompositeAggregator::new();
        agg.add_count("count").add_avg("avg");
        agg.update_batch(&[10.0, 20.0, 30.0]).unwrap();

        // Store window metadata and aggregation state
        let window_key = format!("window:{}", window.id);
        let agg_key = format!("agg:{}", window.id);

        let window_data = serde_json::to_vec(&window).unwrap();
        let agg_data = serde_json::to_vec(&agg).unwrap();

        backend.put(window_key.as_bytes(), &window_data).await.unwrap();
        backend.put(agg_key.as_bytes(), &agg_data).await.unwrap();

        // Verify storage
        let stored_window = backend.get(window_key.as_bytes()).await.unwrap();
        let stored_agg = backend.get(agg_key.as_bytes()).await.unwrap();

        assert!(stored_window.is_some());
        assert!(stored_agg.is_some());
    }

    #[tokio::test]
    async fn test_checkpoint_creation() {
        let backend = Arc::new(MemoryStateBackend::new(None));

        // Add some state
        backend.put(b"key1", b"value1").await.unwrap();
        backend.put(b"key2", b"value2").await.unwrap();

        let checkpoint_dir = tempfile::tempdir().unwrap();
        let coordinator = CheckpointCoordinator::new(
            backend.clone(),
            StdDuration::from_secs(60),
            checkpoint_dir.path().to_path_buf(),
        );

        // Create checkpoint
        coordinator.create_checkpoint().await.unwrap();

        // Verify checkpoint files exist
        let entries: Vec<_> = std::fs::read_dir(checkpoint_dir.path())
            .unwrap()
            .collect();
        assert!(entries.len() > 0);
    }

    #[tokio::test]
    async fn test_state_recovery_after_failure() {
        let backend = Arc::new(MemoryStateBackend::new(None));

        // Simulate processing state
        let mut agg = AverageAggregator::new();
        agg.update_batch(&[10.0, 20.0, 30.0, 40.0, 50.0]).unwrap();

        // Store checkpoint
        let checkpoint_data = serde_json::to_vec(&agg.accumulator()).unwrap();
        backend.put(b"checkpoint:agg", &checkpoint_data).await.unwrap();

        // Simulate failure and recovery
        let mut recovered_agg = AverageAggregator::new();
        let stored = backend.get(b"checkpoint:agg").await.unwrap().unwrap();
        let acc: AverageAccumulator = serde_json::from_slice(&stored).unwrap();
        recovered_agg.merge(acc).unwrap();

        // Verify state is recovered
        assert_eq!(recovered_agg.finalize().unwrap(), 30.0);
        assert_eq!(recovered_agg.count(), 5);
    }
}

// ============================================================================
// PERFORMANCE TESTS
// ============================================================================

mod performance_tests {
    use super::*;

    #[tokio::test]
    async fn test_aggregation_latency() {
        let mut agg = AverageAggregator::new();
        let event_count = 10000;

        let start = Instant::now();

        for i in 0..event_count {
            agg.update(i as f64).unwrap();
        }

        let _result = agg.finalize().unwrap();
        let duration = start.elapsed();

        // Should process 10k events in under 100ms
        assert!(duration.as_millis() < 100);

        println!("Aggregation latency: {:?} for {} events", duration, event_count);
        println!("Per-event latency: {:?}", duration / event_count);
    }

    #[tokio::test]
    async fn test_throughput_capacity() {
        let mut composite = CompositeAggregator::new();
        composite
            .add_count("count")
            .add_avg("avg")
            .add_min("min")
            .add_max("max")
            .add_p95("p95");

        let event_count = 10000;
        let start = Instant::now();

        for i in 0..event_count {
            composite.update(i as f64).unwrap();
        }

        let _results = composite.finalize().unwrap();
        let duration = start.elapsed();

        let events_per_sec = event_count as f64 / duration.as_secs_f64();

        // Should handle at least 10,000 events/sec
        assert!(events_per_sec > 10000.0);

        println!("Throughput: {:.0} events/sec", events_per_sec);
    }

    #[tokio::test]
    async fn test_window_assignment_performance() {
        let assigner = TumblingWindowAssigner::new(Duration::milliseconds(1000));
        let event_count = 10000;

        let start = Instant::now();

        for i in 0..event_count {
            let timestamp = Utc.timestamp_millis_opt(i * 100).unwrap();
            let _windows = assigner.assign_windows(timestamp);
        }

        let duration = start.elapsed();

        // Window assignment should be fast
        assert!(duration.as_millis() < 100);

        println!("Window assignment: {:?} for {} events", duration, event_count);
    }

    #[tokio::test]
    async fn test_watermark_update_performance() {
        let mut generator = BoundedOutOfOrdernessWatermark::new(
            StdDuration::from_secs(5),
            None,
        );

        let event_count = 10000;
        let start = Instant::now();

        for i in 0..event_count {
            generator.on_event(i * 100, (i % 4) as u32);
        }

        let duration = start.elapsed();

        // Watermark updates should be fast
        assert!(duration.as_millis() < 100);

        println!("Watermark updates: {:?} for {} events", duration, event_count);
    }

    #[tokio::test]
    async fn test_state_backend_performance() {
        let backend = MemoryStateBackend::new(None);
        let operation_count = 1000;

        let start = Instant::now();

        for i in 0..operation_count {
            let key = format!("key:{}", i);
            let value = format!("value:{}", i);
            backend.put(key.as_bytes(), value.as_bytes()).await.unwrap();
        }

        for i in 0..operation_count {
            let key = format!("key:{}", i);
            let _value = backend.get(key.as_bytes()).await.unwrap();
        }

        let duration = start.elapsed();

        println!("State operations: {:?} for {} ops", duration, operation_count * 2);

        // State operations should be fast
        let ops_per_ms = (operation_count * 2) as f64 / duration.as_millis() as f64;
        assert!(ops_per_ms > 10.0); // At least 10 ops/ms
    }

    #[tokio::test]
    async fn test_end_to_end_latency() {
        // Measure complete processing latency from event ingestion to result
        let assigner = TumblingWindowAssigner::new(Duration::milliseconds(1000));
        let mut trigger = OnWatermarkTrigger::new();
        let mut aggregator = AverageAggregator::new();

        let event_count = 100;
        let start = Instant::now();

        // Assign window
        let window = assigner.assign_windows(Utc.timestamp_millis_opt(500).unwrap())[0].clone();

        // Process events
        for i in 0..event_count {
            aggregator.update(i as f64).unwrap();
        }

        // Trigger
        let ctx = TriggerContext {
            window: window.clone(),
            event_time: Some(Utc.timestamp_millis_opt(1000).unwrap()),
            processing_time: Utc.timestamp_millis_opt(1100).unwrap(),
            watermark: Some(Utc.timestamp_millis_opt(1000).unwrap()),
            element_count: event_count,
        };

        if trigger.on_watermark(&ctx) == TriggerResult::Fire {
            let _result = aggregator.finalize().unwrap();
        }

        let duration = start.elapsed();

        // End-to-end latency should be under 100ms
        assert!(duration.as_millis() < 100);

        println!("End-to-end latency: {:?} for {} events", duration, event_count);
    }

    #[tokio::test]
    async fn test_memory_efficiency() {
        // Test memory usage with large number of windows
        let mut windows = Vec::new();
        let window_count = 1000;

        for i in 0..window_count {
            let start = Utc.timestamp_millis_opt(i * 1000).unwrap();
            let end = Utc.timestamp_millis_opt((i + 1) * 1000).unwrap();
            let window = Window::new(WindowBounds::new(start, end));
            windows.push(window);
        }

        // Create aggregators for each window
        let mut aggregators: Vec<AverageAggregator<f64>> = (0..window_count)
            .map(|_| AverageAggregator::new())
            .collect();

        // Add data to each
        for agg in &mut aggregators {
            for i in 0..10 {
                agg.update(i as f64).unwrap();
            }
        }

        // Verify all are functional
        assert_eq!(windows.len(), window_count);
        assert_eq!(aggregators.len(), window_count);
    }
}

// ============================================================================
// EDGE CASES AND ERROR HANDLING TESTS
// ============================================================================

mod edge_case_tests {
    use super::*;

    #[test]
    fn test_empty_aggregation() {
        let agg = AverageAggregator::<f64>::new();
        let result = agg.finalize();

        // Should return error for empty aggregation
        assert!(result.is_err());
    }

    #[test]
    fn test_single_value_aggregation() {
        let mut agg = AverageAggregator::new();
        agg.update(42.0).unwrap();

        assert_eq!(agg.finalize().unwrap(), 42.0);
        assert_eq!(agg.count(), 1);
    }

    #[test]
    fn test_zero_values() {
        let mut agg = AverageAggregator::new();
        agg.update(0.0).unwrap();
        agg.update(0.0).unwrap();
        agg.update(0.0).unwrap();

        assert_eq!(agg.finalize().unwrap(), 0.0);
    }

    #[test]
    fn test_negative_values() {
        let mut agg = AverageAggregator::new();
        agg.update(-10.0).unwrap();
        agg.update(-20.0).unwrap();
        agg.update(-30.0).unwrap();

        assert_eq!(agg.finalize().unwrap(), -20.0);
    }

    #[test]
    fn test_very_large_values() {
        let mut agg = SumAggregator::new();
        agg.update(1e100).unwrap();
        agg.update(2e100).unwrap();

        let result = agg.finalize().unwrap();
        assert!(result > 0.0);
        assert!(result.is_finite());
    }

    #[test]
    fn test_very_small_values() {
        let mut agg = AverageAggregator::new();
        agg.update(1e-100).unwrap();
        agg.update(2e-100).unwrap();
        agg.update(3e-100).unwrap();

        let result = agg.finalize().unwrap();
        assert!(result > 0.0);
        assert!(result < 1e-99);
    }

    #[test]
    fn test_window_boundary_conditions() {
        let assigner = TumblingWindowAssigner::new(Duration::milliseconds(1000));

        // Exactly at window start
        let windows = assigner.assign_windows(Utc.timestamp_millis_opt(0).unwrap());
        assert_eq!(windows[0].bounds.start, Utc.timestamp_millis_opt(0).unwrap());

        // Exactly at window boundary
        let windows = assigner.assign_windows(Utc.timestamp_millis_opt(1000).unwrap());
        assert_eq!(windows[0].bounds.start, Utc.timestamp_millis_opt(1000).unwrap());
    }

    #[test]
    fn test_watermark_edge_cases() {
        let mut generator = BoundedOutOfOrdernessWatermark::new(
            StdDuration::from_secs(0), // No delay
            None,
        );

        // With zero delay, watermark should equal max timestamp
        generator.on_event(10000, 0);
        assert_eq!(generator.current_watermark().timestamp, 10000);

        // Reset and test
        generator.reset();
        assert_eq!(generator.current_watermark(), Watermark::min());
    }

    #[test]
    fn test_composite_aggregator_empty_result() {
        let composite = CompositeAggregator::new();
        let result = composite.finalize();

        // Should succeed even with no aggregators
        assert!(result.is_ok());
    }

    #[test]
    fn test_concurrent_window_access() {
        use std::sync::Arc;
        use std::thread;

        let merger = Arc::new(Mutex::new(WindowMerger::new(Duration::milliseconds(1000))));
        let mut handles = vec![];

        for i in 0..10 {
            let merger_clone = Arc::clone(&merger);
            let handle = thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let window = Window::new(WindowBounds::new(
                        Utc.timestamp_millis_opt(i * 100).unwrap(),
                        Utc.timestamp_millis_opt((i + 1) * 100).unwrap(),
                    ));
                    let mut merger = merger_clone.lock().await;
                    merger.add_and_merge(window);
                });
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_aggregator_overflow_protection() {
        let mut count_agg = CountAggregator::new();

        // Add many values to test overflow protection
        for i in 0..1000000 {
            count_agg.update(i as f64).unwrap();
        }

        assert_eq!(count_agg.count(), 1000000);
    }

    #[tokio::test]
    async fn test_state_backend_error_handling() {
        let backend = MemoryStateBackend::new(None);

        // Test get on non-existent key
        let result = backend.get(b"nonexistent").await.unwrap();
        assert!(result.is_none());

        // Test delete on non-existent key (should not error)
        let result = backend.delete(b"nonexistent").await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_event_deduplication() {
        use std::collections::HashSet;

        let mut seen_ids = HashSet::new();
        let events = vec![
            FeedbackEvent::new(EventSource::User, EventType::Feedback, serde_json::json!({})),
            FeedbackEvent::new(EventSource::User, EventType::Feedback, serde_json::json!({})),
            FeedbackEvent::new(EventSource::User, EventType::Feedback, serde_json::json!({})),
        ];

        let mut unique_count = 0;
        for event in &events {
            if seen_ids.insert(event.id) {
                unique_count += 1;
            }
        }

        // All events should have unique IDs
        assert_eq!(unique_count, events.len());
    }

    #[test]
    fn test_percentile_edge_cases() {
        // Test with single value
        let mut p95 = PercentileAggregator::p95();
        p95.update(100.0).unwrap();
        let result = p95.finalize().unwrap();
        assert_eq!(result, 100.0);

        // Test with identical values
        let mut p50 = PercentileAggregator::p50();
        for _ in 0..100 {
            p50.update(42.0).unwrap();
        }
        let result = p50.finalize().unwrap();
        assert_eq!(result, 42.0);
    }
}

// ============================================================================
// STRESS TESTS
// ============================================================================

mod stress_tests {
    use super::*;

    #[tokio::test]
    async fn test_high_cardinality_keys() {
        let key_extractor = FeedbackEventKeyExtractor;
        let mut unique_keys = std::collections::HashSet::new();

        // Generate 10k unique events with different keys
        for i in 0..10000 {
            let mut event = FeedbackEvent::new(
                EventSource::Observatory,
                EventType::Metric,
                serde_json::json!({}),
            );
            event.metadata.insert("model_id".to_string(), format!("model_{}", i));

            let key = key_extractor.extract_key(&event);
            unique_keys.insert(key.to_key_string());
        }

        assert_eq!(unique_keys.len(), 10000);
    }

    #[tokio::test]
    async fn test_many_concurrent_windows() {
        let assigner = TumblingWindowAssigner::new(Duration::milliseconds(100));
        let mut windows = Vec::new();

        // Create 1000 windows
        for i in 0..1000 {
            let timestamp = Utc.timestamp_millis_opt(i * 100).unwrap();
            let assigned = assigner.assign_windows(timestamp);
            windows.extend(assigned);
        }

        assert_eq!(windows.len(), 1000);

        // Verify no overlaps
        for i in 0..windows.len() - 1 {
            assert!(windows[i].bounds.end <= windows[i + 1].bounds.start);
        }
    }

    #[tokio::test]
    async fn test_sustained_throughput() {
        let mut composite = CompositeAggregator::new();
        composite
            .add_count("count")
            .add_avg("avg")
            .add_p95("p95");

        let total_events = 100000;
        let batch_size = 1000;

        for batch in 0..(total_events / batch_size) {
            let batch_data: Vec<f64> = (0..batch_size)
                .map(|i| (batch * batch_size + i) as f64)
                .collect();
            composite.update_batch(&batch_data).unwrap();
        }

        let results = composite.finalize().unwrap();
        assert_eq!(results.get_count("count").unwrap(), total_events as u64);
    }

    #[tokio::test]
    async fn test_rapid_watermark_updates() {
        let mut generator = BoundedOutOfOrdernessWatermark::new(
            StdDuration::from_millis(100),
            None,
        );

        // Rapid fire events
        for i in 0..10000 {
            generator.on_event(i, (i % 8) as u32);
        }

        let wm = generator.current_watermark();
        assert!(wm.timestamp > 0);
    }
}
