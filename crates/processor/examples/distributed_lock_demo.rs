//! Distributed Lock Demo
//!
//! This example demonstrates distributed locking patterns using Redis including:
//! - Lock acquisition and release
//! - Leader election
//! - Coordination between multiple instances
//! - Lock expiration and renewal
//!
//! ## Prerequisites
//!
//! Start Redis:
//! ```bash
//! docker-compose up -d redis
//! ```
//!
//! ## Run Example
//!
//! ```bash
//! cargo run --example distributed_lock_demo
//! ```
//!
//! To simulate multi-instance behavior, run multiple instances in separate terminals.

use processor::state::{DistributedLock, RedisLock};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Distributed Lock Demo ===\n");

    // Generate instance ID
    let instance_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
    println!("Instance ID: {}\n", instance_id);

    // ========================================================================
    // Demo 1: Basic Lock Acquisition and Release
    // ========================================================================
    println!("--- Demo 1: Basic Lock Acquisition ---");

    let lock = RedisLock::new("redis://localhost:6379", "demo:basic_lock").await?;

    println!("Attempting to acquire lock...");
    let acquired = lock.try_lock(Duration::from_secs(30)).await?;

    if acquired {
        println!("✓ Lock acquired!");
        println!("  Holding lock for 2 seconds...");
        sleep(Duration::from_secs(2)).await;

        println!("  Releasing lock...");
        lock.unlock().await?;
        println!("✓ Lock released\n");
    } else {
        println!("✗ Failed to acquire lock (already held by another instance)\n");
    }

    // ========================================================================
    // Demo 2: Lock Expiration
    // ========================================================================
    println!("--- Demo 2: Lock Expiration ---");

    let lock_expire = RedisLock::new("redis://localhost:6379", "demo:expire_lock").await?;

    println!("Acquiring lock with 3-second TTL...");
    let acquired = lock_expire.try_lock(Duration::from_secs(3)).await?;

    if acquired {
        println!("✓ Lock acquired");
        println!("  Waiting 4 seconds for expiration...");
        sleep(Duration::from_secs(4)).await;

        // Try to acquire with new instance (simulating different process)
        let lock_expire2 = RedisLock::new("redis://localhost:6379", "demo:expire_lock").await?;

        let acquired2 = lock_expire2.try_lock(Duration::from_secs(10)).await?;
        if acquired2 {
            println!("✓ Acquired expired lock (automatic cleanup worked)");
            lock_expire2.unlock().await?;
        }
    }
    println!();

    // ========================================================================
    // Demo 3: Using Lock with Closure
    // ========================================================================
    println!("--- Demo 3: Lock with Closure (RAII Pattern) ---");

    let lock_raii = RedisLock::new("redis://localhost:6379", "demo:raii_lock").await?;

    println!("Executing critical section with automatic lock management...");
    let result = lock_raii
        .with_lock(Duration::from_secs(30), || async {
            println!("  → Inside critical section");
            println!("  → Performing work...");
            sleep(Duration::from_millis(500)).await;
            println!("  → Work complete");
            Ok::<i32, anyhow::Error>(42)
        })
        .await?;

    println!("✓ Critical section completed with result: {}", result);
    println!("✓ Lock automatically released\n");

    // ========================================================================
    // Demo 4: Leader Election
    // ========================================================================
    println!("--- Demo 4: Leader Election ---");

    let leader_lock = RedisLock::new("redis://localhost:6379", "demo:leader").await?;

    println!("Attempting leader election...");
    let is_leader = leader_lock.try_lock(Duration::from_secs(30)).await?;

    if is_leader {
        println!("✓ This instance is the LEADER");
        println!("  Leader tasks:");
        println!("  - Coordinating worker instances");
        println!("  - Managing distributed tasks");
        println!("  - Handling failover");

        // Simulate leader work
        for i in 1..=5 {
            println!("  [Leader] Heartbeat {}/5", i);
            sleep(Duration::from_millis(500)).await;
        }

        println!("  Stepping down as leader...");
        leader_lock.unlock().await?;
        println!("✓ Leadership released\n");
    } else {
        println!("✗ This instance is a FOLLOWER");
        println!("  Follower tasks:");
        println!("  - Waiting for leader commands");
        println!("  - Processing delegated work");
        println!("  - Ready to become leader if needed\n");
    }

    // ========================================================================
    // Demo 5: Lock Renewal (Long-Running Tasks)
    // ========================================================================
    println!("--- Demo 5: Lock Renewal ---");

    let renew_lock = RedisLock::new("redis://localhost:6379", "demo:renew_lock").await?;

    println!("Acquiring lock for long-running task...");
    let acquired = renew_lock.try_lock(Duration::from_secs(5)).await?;

    if acquired {
        println!("✓ Lock acquired with 5-second TTL");

        // Spawn renewal task
        let renew_lock_clone = RedisLock::new("redis://localhost:6379", "demo:renew_lock").await?;
        let renewal_handle = tokio::spawn(async move {
            for i in 1..=3 {
                sleep(Duration::from_secs(3)).await;
                println!("  [Renewal] Extending lock TTL (attempt {})", i);
                if let Err(e) = renew_lock_clone.renew(Duration::from_secs(5)).await {
                    eprintln!("  [Renewal] Failed to extend: {}", e);
                    break;
                }
            }
        });

        // Simulate long-running work
        println!("  Performing long-running task (10 seconds)...");
        for i in 1..=10 {
            println!("  [Work] Progress: {}0%", i);
            sleep(Duration::from_secs(1)).await;
        }

        renewal_handle.await?;
        renew_lock.unlock().await?;
        println!("✓ Task complete, lock released\n");
    }

    // ========================================================================
    // Demo 6: Coordinated Work (Distributed Queue)
    // ========================================================================
    println!("--- Demo 6: Coordinated Work Distribution ---");

    // Simulate processing items from a distributed queue
    let tasks = vec!["task_1", "task_2", "task_3", "task_4", "task_5"];

    for task_id in tasks {
        let lock_key = format!("demo:task:{}", task_id);
        let task_lock = RedisLock::new("redis://localhost:6379", &lock_key).await?;

        println!("Attempting to claim {}...", task_id);

        if task_lock.try_lock(Duration::from_secs(10)).await? {
            println!("  ✓ Claimed {} (instance: {})", task_id, instance_id);
            println!("    Processing...");
            sleep(Duration::from_millis(300)).await;
            println!("    Complete!");
            task_lock.unlock().await?;
        } else {
            println!("  ✗ {} already claimed by another instance", task_id);
        }
    }
    println!();

    // ========================================================================
    // Demo 7: Multiple Locks (Dining Philosophers Pattern)
    // ========================================================================
    println!("--- Demo 7: Multiple Resource Locks ---");

    let resource_a = RedisLock::new("redis://localhost:6379", "demo:resource:a").await?;
    let resource_b = RedisLock::new("redis://localhost:6379", "demo:resource:b").await?;

    println!("Acquiring multiple resources atomically...");

    // Try to acquire both locks
    let lock_a = resource_a.try_lock(Duration::from_secs(10)).await?;
    if lock_a {
        println!("  ✓ Acquired resource A");

        let lock_b = resource_b.try_lock(Duration::from_secs(10)).await?;
        if lock_b {
            println!("  ✓ Acquired resource B");
            println!("  → Both resources locked, performing atomic operation");
            sleep(Duration::from_millis(500)).await;
            println!("  ✓ Operation complete");

            resource_b.unlock().await?;
            println!("  ✓ Released resource B");
        } else {
            println!("  ✗ Could not acquire resource B");
        }

        resource_a.unlock().await?;
        println!("  ✓ Released resource A");
    } else {
        println!("  ✗ Could not acquire resource A");
    }
    println!();

    // ========================================================================
    // Demo 8: Lock Monitoring
    // ========================================================================
    println!("--- Demo 8: Lock Monitoring ---");

    let monitor_lock = RedisLock::new("redis://localhost:6379", "demo:monitor").await?;

    if monitor_lock.try_lock(Duration::from_secs(30)).await? {
        println!("✓ Lock acquired");

        // Check lock status
        println!("  Checking lock status...");
        let is_locked = monitor_lock.is_locked().await?;
        println!("    Locked: {}", is_locked);

        // Get lock holder
        if let Some(holder) = monitor_lock.get_holder().await? {
            println!("    Holder: {}", holder);
        }

        // Get remaining TTL
        if let Some(ttl) = monitor_lock.get_ttl().await? {
            println!("    TTL: {:?}", ttl);
        }

        monitor_lock.unlock().await?;
        println!("  ✓ Lock released");

        // Verify unlocked
        let is_locked_after = monitor_lock.is_locked().await?;
        println!("    Locked after release: {}", is_locked_after);
    }
    println!();

    // ========================================================================
    // Demo 9: Concurrent Lock Attempts
    // ========================================================================
    println!("--- Demo 9: Concurrent Lock Attempts ---");

    let lock_name = "demo:concurrent";
    let mut handles = vec![];

    println!("Spawning 5 concurrent tasks competing for the same lock...");

    for i in 0..5 {
        let instance = format!("worker-{}", i);
        let handle = tokio::spawn(async move {
            let lock = RedisLock::new("redis://localhost:6379", lock_name).await?;

            if lock.try_lock(Duration::from_secs(10)).await? {
                println!("  ✓ {} acquired lock", instance);
                sleep(Duration::from_millis(500)).await;
                lock.unlock().await?;
                println!("  ✓ {} released lock", instance);
                Ok::<bool, anyhow::Error>(true)
            } else {
                println!("  ✗ {} failed to acquire lock", instance);
                Ok(false)
            }
        });
        handles.push(handle);
    }

    let mut successful = 0;
    for handle in handles {
        if handle.await?? {
            successful += 1;
        }
    }

    println!("\n  Results: {} successful, {} failed\n", successful, 5 - successful);

    // ========================================================================
    // Demo 10: Graceful Shutdown with Lock
    // ========================================================================
    println!("--- Demo 10: Graceful Shutdown ---");

    let shutdown_lock = RedisLock::new("redis://localhost:6379", "demo:shutdown").await?;

    if shutdown_lock.try_lock(Duration::from_secs(30)).await? {
        println!("✓ Acquired shutdown lock");
        println!("  Performing graceful shutdown sequence:");
        println!("    1. Stopping new work");
        sleep(Duration::from_millis(200)).await;
        println!("    2. Completing in-flight tasks");
        sleep(Duration::from_millis(200)).await;
        println!("    3. Flushing buffers");
        sleep(Duration::from_millis(200)).await;
        println!("    4. Releasing resources");
        sleep(Duration::from_millis(200)).await;

        shutdown_lock.unlock().await?;
        println!("  ✓ Shutdown complete, lock released");
    }
    println!();

    println!("=== Demo Complete ===");
    println!("\nTo see multi-instance behavior:");
    println!("  1. Run this example in multiple terminals");
    println!("  2. Observe leader election and lock contention");
    println!("  3. See coordinated work distribution");

    Ok(())
}
