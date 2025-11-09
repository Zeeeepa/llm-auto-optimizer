//! Demonstration of watermark usage for handling late events
//!
//! This example shows how to use the watermarking system to handle
//! out-of-order and late-arriving events in a stream processing pipeline.

use processor::watermark::{
    BoundedOutOfOrdernessWatermark, LateEventConfig, LateEventHandler, PeriodicWatermark,
    PeriodicWatermarkConfig, PunctuatedWatermark, Watermark, WatermarkGenerator,
};
use std::time::Duration;

fn main() {
    println!("=== Watermark System Demo ===\n");

    // Example 1: Bounded Out-of-Orderness Watermark
    println!("1. Bounded Out-of-Orderness Watermark");
    println!("   Handling events that arrive slightly out of order\n");

    let mut bounded_generator = BoundedOutOfOrdernessWatermark::new(
        Duration::from_secs(5), // Allow 5 seconds of out-of-orderness
        Some(Duration::from_secs(60)), // 60 second idle timeout
    );

    // Simulate events arriving out of order
    let events = vec![
        (10000, 0), // Event at t=10s, partition 0
        (15000, 0), // Event at t=15s, partition 0
        (12000, 0), // Late event at t=12s (within bounds)
        (20000, 1), // Event at t=20s, partition 1
        (18000, 1), // Late event at t=18s, partition 1
    ];

    for (timestamp, partition) in events {
        println!(
            "   Processing event: t={}, partition={}",
            timestamp, partition
        );

        if let Some(watermark) = bounded_generator.on_event(timestamp, partition) {
            println!("   -> New watermark emitted: {}", watermark);
        }

        let current = bounded_generator.current_watermark();
        println!("   -> Current watermark: {}", current);

        if bounded_generator.is_late_event(timestamp) {
            let lateness = bounded_generator.get_lateness(timestamp);
            println!("   -> Event is late by {} ms", lateness);
        }
        println!();
    }

    // Example 2: Multi-partition Watermark
    println!("\n2. Multi-Partition Watermark");
    println!("   Tracking watermarks across multiple partitions\n");

    let mut multi_generator = BoundedOutOfOrdernessWatermark::new(
        Duration::from_secs(3),
        None,
    );

    // Events from different partitions
    let partition_events = vec![
        (10000, 0),
        (20000, 1),
        (15000, 2),
        (12000, 0),
        (22000, 1),
    ];

    for (timestamp, partition) in partition_events {
        multi_generator.on_event(timestamp, partition);

        if let Some(pw) = multi_generator.partition_watermark(partition) {
            println!(
                "   Partition {} watermark: {} (event at {})",
                partition, pw, timestamp
            );
        }
    }

    let global = multi_generator.current_watermark();
    println!("\n   Global watermark (min of all): {}", global);
    println!(
        "   Active partitions: {:?}",
        multi_generator.active_partitions()
    );

    // Example 3: Periodic Watermark
    println!("\n3. Periodic Watermark");
    println!("   Emitting watermarks at regular intervals\n");

    let periodic_config = PeriodicWatermarkConfig {
        interval: Duration::from_millis(100),
        max_out_of_orderness: Duration::from_secs(5),
    };

    let mut periodic_generator = PeriodicWatermark::new(periodic_config);

    // Process some events
    periodic_generator.on_event(10000, 0);
    periodic_generator.on_event(20000, 0);
    periodic_generator.on_event(30000, 0);

    println!("   Processed events, waiting for periodic emission...");
    std::thread::sleep(Duration::from_millis(150));

    if let Some(watermark) = periodic_generator.on_periodic_check() {
        println!("   Periodic watermark emitted: {}", watermark);
    }

    // Example 4: Punctuated Watermark
    println!("\n4. Punctuated Watermark");
    println!("   Emitting watermarks based on event patterns\n");

    // Emit watermark every 3 events
    let mut punctuated_generator = PunctuatedWatermark::every_n_events(3);

    for i in 1..=7 {
        let timestamp = i * 1000;
        print!("   Event {}: ", i);
        if let Some(watermark) = punctuated_generator.on_event(timestamp, 0) {
            println!("WATERMARK EMITTED at {}", watermark);
        } else {
            println!("no watermark");
        }
    }

    // Example 5: Watermark on Timestamp Boundaries
    println!("\n5. Watermark on Timestamp Boundaries");
    println!("   Emitting watermarks when crossing time boundaries\n");

    let mut boundary_generator = PunctuatedWatermark::on_timestamp_boundary(10000); // Every 10 seconds

    let boundary_events = vec![5000, 8000, 12000, 15000, 21000, 25000];

    for timestamp in boundary_events {
        print!("   Event at {}: ", timestamp);
        if let Some(watermark) = boundary_generator.on_event(timestamp, 0) {
            println!("WATERMARK at {}", watermark);
        } else {
            println!("no watermark");
        }
    }

    // Example 6: Late Event Handling
    println!("\n6. Late Event Handling");
    println!("   Deciding whether to process or drop late events\n");

    let late_config = LateEventConfig {
        allowed_lateness: Duration::from_secs(10),
        log_late_events: false,
        emit_metrics: true,
    };

    let handler = LateEventHandler::new(late_config);

    // Simulate various late events
    let late_events = vec![
        (5000, "within allowed lateness"),
        (8000, "within allowed lateness"),
        (15000, "beyond allowed lateness - DROPPED"),
        (3000, "within allowed lateness"),
        (20000, "beyond allowed lateness - DROPPED"),
    ];

    for (lateness, description) in late_events {
        let accepted = handler.handle_late_event(lateness, 0);
        println!(
            "   Event late by {}ms: {} -> {}",
            lateness,
            description,
            if accepted { "ACCEPTED" } else { "DROPPED" }
        );
    }

    let stats = handler.get_stats();
    println!("\n   Late Event Statistics:");
    println!("   - Accepted: {}", stats.accepted_count);
    println!("   - Dropped: {}", stats.dropped_count);
    println!("   - Average lateness: {}ms", stats.average_lateness_ms);

    // Example 7: Complete Pipeline
    println!("\n7. Complete Pipeline Example");
    println!("   Combining watermark generation with late event handling\n");

    let mut pipeline_generator = BoundedOutOfOrdernessWatermark::new(
        Duration::from_secs(5),
        None,
    );

    let pipeline_handler = LateEventHandler::new(LateEventConfig {
        allowed_lateness: Duration::from_secs(10),
        log_late_events: false,
        emit_metrics: true,
    });

    // Simulate event stream
    let stream_events = vec![
        30000, 32000, 28000, // Some out-of-order events
        35000, 33000, 40000, 25000, // Event at 25000 is late
        45000, 20000, // Event at 20000 is very late
    ];

    for (i, timestamp) in stream_events.iter().enumerate() {
        println!("   Event {}: timestamp={}", i + 1, timestamp);

        pipeline_generator.on_event(*timestamp, 0);
        let watermark = pipeline_generator.current_watermark();

        println!("      Current watermark: {}", watermark);

        if pipeline_generator.is_late_event(*timestamp) {
            let lateness = pipeline_generator.get_lateness(*timestamp);
            let accepted = pipeline_handler.handle_late_event(lateness, 0);

            println!(
                "      Late by {}ms -> {}",
                lateness,
                if accepted { "ACCEPTED" } else { "DROPPED" }
            );
        } else {
            println!("      On-time event");
        }
        println!();
    }

    let final_stats = pipeline_handler.get_stats();
    println!("   Final Statistics:");
    println!("   - Total late events: {}", final_stats.accepted_count + final_stats.dropped_count);
    println!("   - Accepted late: {}", final_stats.accepted_count);
    println!("   - Dropped late: {}", final_stats.dropped_count);
    println!("   - Average lateness: {}ms", final_stats.average_lateness_ms);

    println!("\n=== Demo Complete ===");
}
