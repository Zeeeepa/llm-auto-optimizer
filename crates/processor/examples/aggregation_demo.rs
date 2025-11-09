//! Demonstration of aggregation functions for stream processing
//!
//! This example shows how to use various aggregators to compute statistics
//! over streaming data, including distributed processing scenarios.

use processor::aggregation::{
    Aggregator, AverageAggregator, CompositeAggregator, CountAggregator, MaxAggregator,
    MinAggregator, PercentileAggregator, StandardDeviationAggregator, SumAggregator,
};

fn main() {
    println!("=== Stream Aggregation Demo ===\n");

    // Example 1: Basic aggregation
    println!("1. Basic Aggregation - Request Latencies");
    println!("   Computing various statistics on latency data\n");
    basic_aggregation_example();

    println!("\n{}\n", "=".repeat(60));

    // Example 2: Composite aggregation (multiple stats at once)
    println!("2. Composite Aggregation - All Stats in One Pass");
    println!("   Efficiently computing multiple statistics simultaneously\n");
    composite_aggregation_example();

    println!("\n{}\n", "=".repeat(60));

    // Example 3: Distributed processing
    println!("3. Distributed Processing - Merging Partial Results");
    println!("   Processing data across multiple workers and merging results\n");
    distributed_processing_example();

    println!("\n{}\n", "=".repeat(60));

    // Example 4: Time window aggregation
    println!("4. Time Window Aggregation - Tumbling Windows");
    println!("   Simulating aggregation over 5-second time windows\n");
    time_window_example();

    println!("\n{}\n", "=".repeat(60));

    // Example 5: Real-world scenario - API monitoring
    println!("5. Real-World: API Endpoint Monitoring");
    println!("   Tracking multiple metrics for an API endpoint\n");
    api_monitoring_example();
}

fn basic_aggregation_example() {
    // Simulate request latencies in milliseconds
    let latencies = vec![
        45.2, 48.1, 52.0, 49.5, 51.2, 150.0, 47.8, 50.1, 46.9, 200.0, 48.5, 49.2, 51.8, 500.0,
        47.2,
    ];

    // Create different aggregators
    let mut count = CountAggregator::new();
    let mut avg = AverageAggregator::new();
    let mut min = MinAggregator::new();
    let mut max = MaxAggregator::new();
    let mut p50 = PercentileAggregator::p50();
    let mut p95 = PercentileAggregator::p95();
    let mut p99 = PercentileAggregator::p99();
    let mut stddev = StandardDeviationAggregator::new();

    // Feed data to all aggregators
    for &latency in &latencies {
        count.update(latency).unwrap();
        avg.update(latency).unwrap();
        min.update(latency).unwrap();
        max.update(latency).unwrap();
        p50.update(latency).unwrap();
        p95.update(latency).unwrap();
        p99.update(latency).unwrap();
        stddev.update(latency).unwrap();
    }

    // Print results
    println!("   Total Requests:    {}", count.finalize().unwrap());
    println!("   Average Latency:   {:.2}ms", avg.finalize().unwrap());
    println!("   Min Latency:       {:.2}ms", min.finalize().unwrap());
    println!("   Max Latency:       {:.2}ms", max.finalize().unwrap());
    println!("   P50 (Median):      {:.2}ms", p50.finalize().unwrap());
    println!("   P95:               {:.2}ms", p95.finalize().unwrap());
    println!("   P99:               {:.2}ms", p99.finalize().unwrap());
    println!("   Std Deviation:     {:.2}ms", stddev.finalize().unwrap());
}

fn composite_aggregation_example() {
    // Same latency data, but process with composite aggregator
    let latencies = vec![
        45.2, 48.1, 52.0, 49.5, 51.2, 150.0, 47.8, 50.1, 46.9, 200.0, 48.5, 49.2, 51.8, 500.0,
        47.2,
    ];

    // Create composite aggregator with all stats
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

    // Single pass through data
    for latency in latencies {
        composite.update(latency).unwrap();
    }

    // Get all results at once
    let results = composite.finalize().unwrap();

    println!("   Total Requests:    {}", results.get_count("request_count").unwrap());
    println!("   Average Latency:   {:.2}ms", results.get_value("avg_latency").unwrap());
    println!("   Min Latency:       {:.2}ms", results.get_value("min_latency").unwrap());
    println!("   Max Latency:       {:.2}ms", results.get_value("max_latency").unwrap());
    println!("   P50 (Median):      {:.2}ms", results.get_value("p50_latency").unwrap());
    println!("   P95:               {:.2}ms", results.get_value("p95_latency").unwrap());
    println!("   P99:               {:.2}ms", results.get_value("p99_latency").unwrap());
    println!("   Std Deviation:     {:.2}ms", results.get_value("stddev_latency").unwrap());

    println!("\n   Performance benefit: Single pass vs 8 separate passes!");
}

fn distributed_processing_example() {
    // Simulate processing across 3 worker nodes
    let worker1_data = vec![45.0, 50.0, 55.0, 60.0];
    let worker2_data = vec![48.0, 52.0, 58.0, 62.0];
    let worker3_data = vec![46.0, 51.0, 56.0, 61.0];

    println!("   Worker 1: Processing {} requests", worker1_data.len());
    let mut agg1 = AverageAggregator::new();
    agg1.update_batch(&worker1_data).unwrap();
    let worker1_avg = agg1.finalize().unwrap();
    println!("     Local average: {:.2}ms", worker1_avg);

    println!("   Worker 2: Processing {} requests", worker2_data.len());
    let mut agg2 = AverageAggregator::new();
    agg2.update_batch(&worker2_data).unwrap();
    let worker2_avg = agg2.finalize().unwrap();
    println!("     Local average: {:.2}ms", worker2_avg);

    println!("   Worker 3: Processing {} requests", worker3_data.len());
    let mut agg3 = AverageAggregator::new();
    agg3.update_batch(&worker3_data).unwrap();
    let worker3_avg = agg3.finalize().unwrap();
    println!("     Local average: {:.2}ms", worker3_avg);

    // Merge results from all workers
    println!("\n   Merging results from all workers...");
    let mut combined = AverageAggregator::<f64>::new();
    combined.merge(agg1.accumulator()).unwrap();
    combined.merge(agg2.accumulator()).unwrap();
    combined.merge(agg3.accumulator()).unwrap();

    println!("   Combined total:    {} requests", combined.count());
    println!("   Global average:    {:.2}ms", combined.finalize().unwrap());
}

fn time_window_example() {
    // Simulate 15 seconds of data in 5-second tumbling windows
    struct Event {
        timestamp: u64,
        latency: f64,
    }

    let events = vec![
        Event { timestamp: 1000, latency: 45.0 },
        Event { timestamp: 1500, latency: 50.0 },
        Event { timestamp: 2000, latency: 48.0 },
        Event { timestamp: 3000, latency: 52.0 },
        Event { timestamp: 4500, latency: 47.0 },
        Event { timestamp: 6000, latency: 150.0 }, // Window 2
        Event { timestamp: 7000, latency: 55.0 },
        Event { timestamp: 8500, latency: 49.0 },
        Event { timestamp: 9000, latency: 51.0 },
        Event { timestamp: 11000, latency: 200.0 }, // Window 3
        Event { timestamp: 12000, latency: 46.0 },
        Event { timestamp: 13500, latency: 48.0 },
    ];

    let window_size = 5000; // 5 seconds
    let mut windows: std::collections::HashMap<u64, CompositeAggregator> =
        std::collections::HashMap::new();

    // Assign events to windows
    for event in events {
        let window_id = event.timestamp / window_size;
        let agg = windows.entry(window_id).or_insert_with(|| {
            let mut comp = CompositeAggregator::new();
            comp.add_count("count").add_avg("avg").add_max("max");
            comp
        });
        agg.update(event.latency).unwrap();
    }

    // Print results for each window
    let mut sorted_windows: Vec<_> = windows.iter().collect();
    sorted_windows.sort_by_key(|(id, _)| **id);

    for (window_id, agg) in sorted_windows {
        let results = agg.finalize().unwrap();
        let start_ms = window_id * window_size;
        let end_ms = (window_id + 1) * window_size;

        println!(
            "   Window [{:>5}ms - {:>5}ms]: count={}, avg={:.1}ms, max={:.1}ms",
            start_ms,
            end_ms,
            results.get_count("count").unwrap(),
            results.get_value("avg").unwrap(),
            results.get_value("max").unwrap()
        );
    }
}

fn api_monitoring_example() {
    println!("   Monitoring: POST /api/v1/completions");

    // Simulate real API metrics
    struct ApiRequest {
        latency_ms: f64,
        tokens_used: f64,
        cost_usd: f64,
    }

    let requests = vec![
        ApiRequest { latency_ms: 1200.0, tokens_used: 150.0, cost_usd: 0.003 },
        ApiRequest { latency_ms: 950.0, tokens_used: 120.0, cost_usd: 0.0024 },
        ApiRequest { latency_ms: 2100.0, tokens_used: 250.0, cost_usd: 0.005 },
        ApiRequest { latency_ms: 1100.0, tokens_used: 140.0, cost_usd: 0.0028 },
        ApiRequest { latency_ms: 5200.0, tokens_used: 500.0, cost_usd: 0.010 }, // Outlier
        ApiRequest { latency_ms: 1050.0, tokens_used: 135.0, cost_usd: 0.0027 },
        ApiRequest { latency_ms: 1300.0, tokens_used: 160.0, cost_usd: 0.0032 },
        ApiRequest { latency_ms: 980.0, tokens_used: 125.0, cost_usd: 0.0025 },
    ];

    // Track latency metrics
    let mut latency_stats = CompositeAggregator::new();
    latency_stats
        .add_count("count")
        .add_avg("avg")
        .add_min("min")
        .add_max("max")
        .add_p50("p50")
        .add_p95("p95");

    // Track token usage
    let mut token_stats = CompositeAggregator::new();
    token_stats.add_sum("total").add_avg("avg");

    // Track costs
    let mut cost_stats = CompositeAggregator::new();
    cost_stats.add_sum("total").add_avg("avg");

    // Process requests
    for req in requests {
        latency_stats.update(req.latency_ms).unwrap();
        token_stats.update(req.tokens_used).unwrap();
        cost_stats.update(req.cost_usd).unwrap();
    }

    // Print comprehensive report
    let lat_results = latency_stats.finalize().unwrap();
    let tok_results = token_stats.finalize().unwrap();
    let cost_results = cost_stats.finalize().unwrap();

    println!("\n   Latency Metrics:");
    println!("     Total Requests: {}", lat_results.get_count("count").unwrap());
    println!("     Avg:            {:.0}ms", lat_results.get_value("avg").unwrap());
    println!("     Min:            {:.0}ms", lat_results.get_value("min").unwrap());
    println!("     Max:            {:.0}ms", lat_results.get_value("max").unwrap());
    println!("     P50:            {:.0}ms", lat_results.get_value("p50").unwrap());
    println!("     P95:            {:.0}ms", lat_results.get_value("p95").unwrap());

    println!("\n   Token Usage:");
    println!("     Total:          {:.0} tokens", tok_results.get_value("total").unwrap());
    println!("     Avg/Request:    {:.0} tokens", tok_results.get_value("avg").unwrap());

    println!("\n   Cost Metrics:");
    println!("     Total:          ${:.4}", cost_results.get_value("total").unwrap());
    println!("     Avg/Request:    ${:.4}", cost_results.get_value("avg").unwrap());

    println!("\n   SLA Status:");
    let p95_latency = lat_results.get_value("p95").unwrap();
    let sla_threshold = 3000.0; // 3 second SLA
    if p95_latency < sla_threshold {
        println!("     P95 Latency: WITHIN SLA ({:.0}ms < {:.0}ms)", p95_latency, sla_threshold);
    } else {
        println!(
            "     P95 Latency: EXCEEDS SLA ({:.0}ms > {:.0}ms)",
            p95_latency, sla_threshold
        );
    }
}
