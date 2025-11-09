//! Demonstration of the state management module
//!
//! Run with: cargo run --example state_demo -p processor

use processor::state::{
    Checkpoint, CheckpointCoordinator, CheckpointOptions, MemoryStateBackend, SledStateBackend,
    StateBackend,
};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== State Management Module Demo ===\n");

    // Demo 1: In-Memory State Backend
    println!("1. In-Memory State Backend");
    demo_memory_backend().await?;

    // Demo 2: Memory Backend with TTL
    println!("\n2. Memory Backend with TTL");
    demo_ttl().await?;

    // Demo 3: Sled Persistent Backend
    println!("\n3. Sled Persistent Backend");
    demo_sled_backend().await?;

    // Demo 4: Checkpointing
    println!("\n4. Checkpointing and Recovery");
    demo_checkpointing().await?;

    // Demo 5: Concurrent Access
    println!("\n5. Concurrent State Access");
    demo_concurrent_access().await?;

    println!("\n=== Demo Complete ===");
    Ok(())
}

async fn demo_memory_backend() -> anyhow::Result<()> {
    let backend = MemoryStateBackend::new(None);

    // Store window state
    backend.put(b"window:session123:1", b"aggregated_metrics").await?;
    backend.put(b"window:session123:2", b"more_metrics").await?;
    backend.put(b"meta:checkpoint_id", b"ckpt_001").await?;

    println!("  Stored 3 entries");

    // Retrieve state
    if let Some(data) = backend.get(b"window:session123:1").await? {
        println!("  Retrieved window state: {} bytes", data.len());
    }

    // List all windows for a session
    let windows = backend.list_keys(b"window:session123:").await?;
    println!("  Found {} windows for session123", windows.len());

    // Get statistics
    let stats = backend.stats().await;
    println!("  Stats: {} gets, {} puts, hit rate: {:.1}%",
        stats.get_count,
        stats.put_count,
        100.0 * stats.hit_count as f64 / stats.get_count.max(1) as f64
    );

    Ok(())
}

async fn demo_ttl() -> anyhow::Result<()> {
    let backend = MemoryStateBackend::new(Some(Duration::from_secs(2)));

    backend.put(b"temp_session", b"temporary_data").await?;
    println!("  Stored temporary session data (TTL: 2s)");

    assert!(backend.get(b"temp_session").await?.is_some());
    println!("  Data is accessible immediately");

    tokio::time::sleep(Duration::from_millis(2500)).await;

    assert!(backend.get(b"temp_session").await?.is_none());
    println!("  Data expired after TTL");

    Ok(())
}

async fn demo_sled_backend() -> anyhow::Result<()> {
    let backend = SledStateBackend::temporary().await?;

    // Store persistent state
    backend.put(b"persistent:window:1", b"state_data_1").await?;
    backend.put(b"persistent:window:2", b"state_data_2").await?;

    println!("  Stored 2 entries in Sled backend");

    // Flush to ensure persistence
    let flushed = backend.flush().await?;
    println!("  Flushed {} bytes to disk", flushed);

    // List all persistent windows
    let keys = backend.list_keys(b"persistent:window:").await?;
    println!("  Found {} persistent windows", keys.len());

    // Get statistics
    let stats = backend.stats().await;
    println!("  Stats: {} reads, {} writes, {} bytes written",
        stats.get_count,
        stats.put_count,
        stats.bytes_written
    );

    Ok(())
}

async fn demo_checkpointing() -> anyhow::Result<()> {
    let backend = Arc::new(MemoryStateBackend::new(None));

    // Simulate window state
    for i in 0..10 {
        let key = format!("window:app1:event_count:{}", i);
        let value = format!("count={}", i * 100);
        backend.put(key.as_bytes(), value.as_bytes()).await?;
    }

    println!("  Created 10 window states");

    // Setup checkpoint coordinator
    let temp_dir = std::env::temp_dir().join("state_demo_checkpoints");
    let options = CheckpointOptions {
        checkpoint_dir: temp_dir.clone(),
        max_checkpoints: 3,
        compress: false,
        min_interval: Duration::from_secs(1),
    };

    let coordinator = CheckpointCoordinator::new(
        backend.clone(),
        Duration::from_secs(60),
        options,
    );

    // Create checkpoint
    let checkpoint_id = coordinator.create_checkpoint().await?;
    println!("  Created checkpoint: {}", checkpoint_id);

    // Simulate crash - clear all state
    backend.clear().await?;
    println!("  Simulated crash - all state cleared");

    assert_eq!(backend.count().await?, 0);

    // Restore from checkpoint
    coordinator.restore_latest().await?;
    println!("  Restored from checkpoint");

    assert_eq!(backend.count().await?, 10);
    println!("  Verified: all 10 windows restored");

    // Get checkpoint stats
    let stats = coordinator.stats().await;
    println!("  Checkpoint stats: {} created, {} bytes",
        stats.checkpoints_created,
        stats.total_bytes_checkpointed
    );

    // Cleanup
    tokio::fs::remove_dir_all(&temp_dir).await.ok();

    Ok(())
}

async fn demo_concurrent_access() -> anyhow::Result<()> {
    let backend = Arc::new(MemoryStateBackend::new(None));
    let mut handles = vec![];

    println!("  Spawning 5 concurrent tasks...");

    // Spawn 5 tasks, each managing different windows
    for task_id in 0..5 {
        let backend = Arc::clone(&backend);
        let handle = tokio::spawn(async move {
            for window_id in 0..20 {
                let key = format!("window:task{}:win{}", task_id, window_id);
                let value = format!("data_from_task_{}", task_id);

                // Simulate window operations
                backend.put(key.as_bytes(), value.as_bytes()).await.unwrap();

                // Simulate some processing time
                tokio::time::sleep(Duration::from_micros(100)).await;
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await?;
    }

    println!("  All tasks completed");
    println!("  Total windows created: {}", backend.count().await?);

    // Verify each task's windows
    for task_id in 0..5 {
        let prefix = format!("window:task{}:", task_id);
        let count = backend.list_keys(prefix.as_bytes()).await?.len();
        println!("    Task {}: {} windows", task_id, count);
    }

    Ok(())
}
