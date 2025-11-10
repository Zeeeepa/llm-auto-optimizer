//! High Availability State Backend Demo
//!
//! Demonstrates HA patterns for distributed state backends including:
//! - Failover handling
//! - Leader election
//! - Backup and recovery
//! - Split-brain prevention
//! - Health monitoring
//!
//! ## Run
//!
//! ```bash
//! cargo run --example state_ha_demo
//! ```

use processor::state::{
    DistributedLock, PostgresConfig, PostgresStateBackend, RedisConfig, RedisDistributedLock,
    RedisStateBackend, StateBackend,
};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::{interval, sleep};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=".repeat(80));
    println!("High Availability State Backend Demo");
    println!("=".repeat(80));

    // Run HA demos
    failover_demo().await?;
    leader_election_demo().await?;
    backup_recovery_demo().await?;
    health_monitoring_demo().await?;

    println!("\n{}", "=".repeat(80));
    println!("All HA demos completed!");
    println!("=".repeat(80));

    Ok(())
}

// ============================================================================
// FAILOVER HANDLING
// ============================================================================

async fn failover_demo() -> anyhow::Result<()> {
    println!("\n{}", "-".repeat(80));
    println!("1. Failover Handling Demo");
    println!("{}", "-".repeat(80));

    // Primary backend
    let primary_config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("ha:failover:")
        .retries(3, Duration::from_millis(100), Duration::from_secs(2))
        .build()?;

    let primary = Arc::new(RedisStateBackend::new(primary_config).await?);
    println!("✓ Primary backend connected");

    // Simulate failover scenario
    let backend = primary.clone();
    let mut retry_count = 0;
    let max_retries = 5;

    loop {
        match backend.put(b"test_key", b"test_value").await {
            Ok(_) => {
                println!("✓ Operation succeeded on attempt {}", retry_count + 1);
                break;
            }
            Err(e) => {
                retry_count += 1;
                if retry_count >= max_retries {
                    println!("✗ Failed after {} attempts: {}", retry_count, e);
                    break;
                }
                println!("  Attempt {} failed, retrying...", retry_count);
                sleep(Duration::from_secs(1)).await;
            }
        }
    }

    // Verify connection after potential failover
    let healthy = backend.health_check().await?;
    println!("✓ Backend health after failover: {}", if healthy { "OK" } else { "FAILED" });

    Ok(())
}

// ============================================================================
// LEADER ELECTION
// ============================================================================

async fn leader_election_demo() -> anyhow::Result<()> {
    println!("\n{}", "-".repeat(80));
    println!("2. Leader Election Demo");
    println!("{}", "-".repeat(80));

    let lock = Arc::new(RedisDistributedLock::new("redis://localhost:6379").await?);
    let is_running = Arc::new(AtomicBool::new(true));
    let task_counter = Arc::new(AtomicU64::new(0));

    // Spawn 3 instances competing for leadership
    let mut handles = vec![];

    for instance_id in 1..=3 {
        let lock = lock.clone();
        let is_running = is_running.clone();
        let task_counter = task_counter.clone();

        let handle = tokio::spawn(async move {
            leader_worker(instance_id, lock, is_running, task_counter).await
        });

        handles.push(handle);
    }

    // Let them compete for 5 seconds
    sleep(Duration::from_secs(5)).await;

    // Signal shutdown
    is_running.store(false, Ordering::SeqCst);

    // Wait for all instances to stop
    for handle in handles {
        handle.await.ok();
    }

    let total_tasks = task_counter.load(Ordering::SeqCst);
    println!("\n✓ Leader election completed");
    println!("  Total tasks executed: {}", total_tasks);

    Ok(())
}

async fn leader_worker(
    instance_id: u32,
    lock: Arc<RedisDistributedLock>,
    is_running: Arc<AtomicBool>,
    task_counter: Arc<AtomicU64>,
) -> anyhow::Result<()> {
    println!("  Instance {} started", instance_id);

    while is_running.load(Ordering::SeqCst) {
        // Try to become leader
        if let Some(mut guard) = lock
            .try_acquire("leader", Duration::from_secs(2))
            .await?
        {
            println!("  Instance {} became LEADER", instance_id);

            // Perform leader duties
            let mut tenure_time = 0;
            while tenure_time < 2 && is_running.load(Ordering::SeqCst) {
                // Simulate work
                sleep(Duration::from_millis(500)).await;
                task_counter.fetch_add(1, Ordering::SeqCst);
                tenure_time += 1;

                // Extend leadership if needed
                if tenure_time == 1 {
                    lock.extend(&mut guard, Duration::from_secs(2)).await?;
                }
            }

            println!("  Instance {} stepping down", instance_id);
            drop(guard);
        } else {
            // Not leader, wait and retry
            sleep(Duration::from_millis(100)).await;
        }
    }

    println!("  Instance {} stopped", instance_id);
    Ok(())
}

// ============================================================================
// BACKUP AND RECOVERY
// ============================================================================

async fn backup_recovery_demo() -> anyhow::Result<()> {
    println!("\n{}", "-".repeat(80));
    println!("3. Backup and Recovery Demo");
    println!("{}", "-".repeat(80));

    // Redis backup
    println!("\n--- Redis Backup ---");
    let redis_config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("ha:backup:")
        .build()?;

    let redis_backend = RedisStateBackend::new(redis_config).await?;
    redis_backend.clear().await?;

    // Create sample data
    for i in 0..100 {
        let key = format!("data:{}", i);
        let value = format!("value_{}", i);
        redis_backend.put(key.as_bytes(), value.as_bytes()).await?;
    }
    println!("✓ Created 100 sample entries");

    // Create checkpoint (backup)
    let redis_checkpoint = redis_backend.checkpoint().await?;
    println!("✓ Checkpoint created: {} entries", redis_checkpoint.len());

    // Simulate data loss
    redis_backend.clear().await?;
    println!("⚠ Simulated data loss (cleared all entries)");
    println!("  Current count: {}", redis_backend.count().await?);

    // Restore from checkpoint
    redis_backend.restore(redis_checkpoint).await?;
    println!("✓ Restored from checkpoint");
    println!("  Recovered count: {}", redis_backend.count().await?);

    // PostgreSQL backup
    println!("\n--- PostgreSQL Backup ---");
    let pg_config = PostgresConfig::new("postgresql://postgres:postgres@localhost:5432/optimizer");
    let pg_backend = PostgresStateBackend::new(pg_config).await?;
    pg_backend.clear().await?;

    // Create sample data
    for i in 0..50 {
        let key = format!("pg_data:{}", i);
        pg_backend.put(key.as_bytes(), b"pg_value").await?;
    }
    println!("✓ Created 50 sample entries in PostgreSQL");

    // Create checkpoint
    let checkpoint_id = pg_backend.create_checkpoint().await?;
    println!("✓ Checkpoint created: {}", checkpoint_id);

    // List all checkpoints
    let checkpoints = pg_backend.list_checkpoints().await?;
    println!("✓ Available checkpoints:");
    for cp in &checkpoints {
        println!("  - {}: {} rows, {} bytes ({})",
            cp.checkpoint_id,
            cp.row_count,
            cp.data_size,
            cp.created_at);
    }

    // Modify data
    pg_backend.put(b"pg_data:0", b"modified").await?;
    println!("⚠ Modified data");

    // Restore to previous checkpoint
    pg_backend.restore_checkpoint(checkpoint_id).await?;
    println!("✓ Restored to checkpoint");

    // Verify restoration
    let value = pg_backend.get(b"pg_data:0").await?;
    println!("  Restored value: {:?}", String::from_utf8_lossy(&value.unwrap()));

    Ok(())
}

// ============================================================================
// HEALTH MONITORING
// ============================================================================

async fn health_monitoring_demo() -> anyhow::Result<()> {
    println!("\n{}", "-".repeat(80));
    println!("4. Health Monitoring Demo");
    println!("{}", "-".repeat(80));

    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("ha:health:")
        .build()?;

    let backend = Arc::new(RedisStateBackend::new(config).await?);

    // Health checker
    let health_status = Arc::new(RwLock::new(true));
    let health_checker = HealthChecker::new(backend.clone(), health_status.clone());

    // Start monitoring
    let monitor_handle = tokio::spawn(async move {
        health_checker.start_monitoring().await
    });

    // Simulate operations
    println!("✓ Starting health monitoring...");
    for i in 0..10 {
        sleep(Duration::from_secs(1)).await;

        let is_healthy = *health_status.read().await;
        println!("  Cycle {}: Backend is {}", i + 1,
            if is_healthy { "HEALTHY" } else { "UNHEALTHY" });

        // Perform some operations
        backend.put(format!("key_{}", i).as_bytes(), b"value").await?;
    }

    // Stop monitoring
    drop(monitor_handle);
    println!("✓ Health monitoring completed");

    // Final stats
    let stats = backend.stats().await;
    println!("\n✓ Final Statistics:");
    println!("  - Total operations: {}", stats.total_operations());
    println!("  - Hit rate: {:.2}%", stats.hit_rate() * 100.0);
    println!("  - Error rate: {:.2}%", stats.error_rate() * 100.0);
    println!("  - Retries: {}", stats.retry_count);

    Ok(())
}

// ============================================================================
// HEALTH CHECKER
// ============================================================================

struct HealthChecker {
    backend: Arc<RedisStateBackend>,
    health_status: Arc<RwLock<bool>>,
}

impl HealthChecker {
    fn new(backend: Arc<RedisStateBackend>, health_status: Arc<RwLock<bool>>) -> Self {
        Self {
            backend,
            health_status,
        }
    }

    async fn start_monitoring(&self) -> anyhow::Result<()> {
        let mut interval = interval(Duration::from_secs(1));

        loop {
            interval.tick().await;

            // Perform health check
            let is_healthy = self.check_health().await;

            // Update status
            let mut status = self.health_status.write().await;
            *status = is_healthy;
            drop(status);

            // Alert if unhealthy
            if !is_healthy {
                eprintln!("⚠ ALERT: Backend unhealthy!");
            }
        }
    }

    async fn check_health(&self) -> bool {
        // Check 1: Connection health
        if !self.backend.health_check().await.unwrap_or(false) {
            return false;
        }

        // Check 2: Error rate
        let stats = self.backend.stats().await;
        if stats.error_rate() > 0.1 {  // > 10% error rate
            return false;
        }

        // Check 3: Latency
        if stats.avg_latency_us > 100_000 {  // > 100ms
            return false;
        }

        true
    }
}
