//! Redis Distributed State Management Example
//!
//! This example demonstrates advanced distributed state management scenarios:
//! - Multi-node state sharing
//! - Distributed locking
//! - Session management with TTL
//! - Window state for stream processing
//! - Fault tolerance with checkpointing
//!
//! ## Prerequisites
//!
//! Redis must be running on localhost:6379
//!
//! ```bash
//! docker run -d -p 6379:6379 redis:7-alpine
//! ```

use processor::state::{RedisConfig, RedisStateBackend, StateBackend};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// Window state for stream processing
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WindowState {
    window_id: String,
    start_time: i64,
    end_time: i64,
    count: u64,
    sum: f64,
    min: f64,
    max: f64,
}

impl WindowState {
    fn new(window_id: String, start_time: i64, end_time: i64) -> Self {
        Self {
            window_id,
            start_time,
            end_time,
            count: 0,
            sum: 0.0,
            min: f64::MAX,
            max: f64::MIN,
        }
    }

    fn add_value(&mut self, value: f64) {
        self.count += 1;
        self.sum += value;
        self.min = self.min.min(value);
        self.max = self.max.max(value);
    }

    fn avg(&self) -> f64 {
        if self.count > 0 {
            self.sum / self.count as f64
        } else {
            0.0
        }
    }

    fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }

    fn deserialize(data: &[u8]) -> Self {
        bincode::deserialize(data).unwrap()
    }
}

/// User session data
#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserSession {
    user_id: String,
    session_id: String,
    created_at: i64,
    last_activity: i64,
    data: std::collections::HashMap<String, String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info,redis_distributed_state=debug")
        .init();

    println!("=== Redis Distributed State Management Example ===\n");

    // Create Redis backend for distributed state
    let config = RedisConfig::builder()
        .url("redis://localhost:6379")
        .key_prefix("distributed:")
        .default_ttl(Duration::from_secs(3600))
        .pool_size(10, 50)
        .build()?;

    let backend = Arc::new(RedisStateBackend::new(config).await?);
    println!("✓ Connected to Redis\n");

    // Scenario 1: Distributed Locking
    println!("=== Scenario 1: Distributed Locking ===");
    distributed_locking_example(backend.clone()).await?;

    // Scenario 2: Session Management
    println!("\n=== Scenario 2: Session Management with TTL ===");
    session_management_example(backend.clone()).await?;

    // Scenario 3: Window State for Stream Processing
    println!("\n=== Scenario 3: Window State for Stream Processing ===");
    window_state_example(backend.clone()).await?;

    // Scenario 4: Multi-Node Coordination
    println!("\n=== Scenario 4: Multi-Node Coordination ===");
    multi_node_coordination_example(backend.clone()).await?;

    // Scenario 5: Fault Tolerance with Checkpointing
    println!("\n=== Scenario 5: Fault Tolerance with Checkpointing ===");
    fault_tolerance_example(backend.clone()).await?;

    // Cleanup
    println!("\nCleaning up...");
    backend.clear().await?;
    println!("✓ All data cleared");

    println!("\n=== Example Complete ===");

    Ok(())
}

async fn distributed_locking_example(backend: Arc<RedisStateBackend>) -> anyhow::Result<()> {
    println!("Simulating distributed lock acquisition...");

    let lock_key = b"lock:critical_resource";
    let lock_ttl = Duration::from_secs(10);

    // Node 1 acquires lock
    let acquired1 = backend
        .set_if_not_exists(lock_key, b"node-1", lock_ttl)
        .await?;
    println!("✓ Node 1 lock acquisition: {}", if acquired1 { "SUCCESS" } else { "FAILED" });

    // Node 2 tries to acquire same lock
    let acquired2 = backend
        .set_if_not_exists(lock_key, b"node-2", lock_ttl)
        .await?;
    println!("✓ Node 2 lock acquisition: {}", if acquired2 { "SUCCESS" } else { "FAILED" });

    // Check who has the lock
    if let Some(owner) = backend.get(lock_key).await? {
        println!("✓ Lock held by: {:?}", String::from_utf8_lossy(&owner));
    }

    // Check TTL
    if let Some(ttl) = backend.ttl(lock_key).await? {
        println!("✓ Lock TTL: {:?}", ttl);
    }

    // Release lock
    backend.delete(lock_key).await?;
    println!("✓ Lock released");

    // Now node 2 can acquire
    let acquired3 = backend
        .set_if_not_exists(lock_key, b"node-2", lock_ttl)
        .await?;
    println!("✓ Node 2 lock acquisition (retry): {}", if acquired3 { "SUCCESS" } else { "FAILED" });

    backend.delete(lock_key).await?;

    Ok(())
}

async fn session_management_example(backend: Arc<RedisStateBackend>) -> anyhow::Result<()> {
    println!("Managing user sessions with TTL...");

    let session_ttl = Duration::from_secs(30 * 60); // 30 minutes

    // Create session
    let mut session = UserSession {
        user_id: "user123".to_string(),
        session_id: "sess_abc123".to_string(),
        created_at: chrono::Utc::now().timestamp(),
        last_activity: chrono::Utc::now().timestamp(),
        data: std::collections::HashMap::new(),
    };

    session.data.insert("ip".to_string(), "192.168.1.1".to_string());
    session.data.insert("user_agent".to_string(), "Mozilla/5.0".to_string());

    let session_key = format!("session:{}", session.session_id).into_bytes();
    let session_data = bincode::serialize(&session)?;

    // Store session with TTL
    backend.put_with_ttl(&session_key, &session_data, session_ttl).await?;
    println!("✓ Created session: {}", session.session_id);

    // Retrieve session
    if let Some(data) = backend.get(&session_key).await? {
        let retrieved: UserSession = bincode::deserialize(&data)?;
        println!("✓ Retrieved session for user: {}", retrieved.user_id);
        println!("  - Created at: {}", retrieved.created_at);
        println!("  - Session data: {:?}", retrieved.data);
    }

    // Update session activity
    session.last_activity = chrono::Utc::now().timestamp();
    session.data.insert("page".to_string(), "/dashboard".to_string());
    let updated_data = bincode::serialize(&session)?;
    backend.put_with_ttl(&session_key, &updated_data, session_ttl).await?;
    println!("✓ Updated session activity");

    // List all active sessions
    let all_sessions = backend.list_keys(b"session:").await?;
    println!("✓ Active sessions: {}", all_sessions.len());

    Ok(())
}

async fn window_state_example(backend: Arc<RedisStateBackend>) -> anyhow::Result<()> {
    println!("Managing window state for stream processing...");

    // Simulate tumbling windows (5-second windows)
    let window_duration = 5000; // milliseconds
    let now = chrono::Utc::now().timestamp_millis();

    // Create windows
    let mut windows = vec![];
    for i in 0..3 {
        let start = now + (i * window_duration);
        let end = start + window_duration;
        let window_id = format!("window:{}", start);

        let mut window = WindowState::new(window_id.clone(), start, end);

        // Add some events to the window
        for _ in 0..10 {
            window.add_value(rand::random::<f64>() * 100.0);
        }

        windows.push((window_id.clone(), window));
    }

    // Store all windows using batch operation
    let window_pairs: Vec<(&[u8], &[u8])> = windows
        .iter()
        .map(|(id, state)| {
            let key = format!("window:metric:{}", id).into_bytes();
            let data = state.serialize();
            (key.leak() as &[u8], data.leak() as &[u8])
        })
        .collect();

    println!("✓ Storing {} windows", windows.len());

    // Store windows individually with proper memory management
    for (id, state) in &windows {
        let key = format!("window:metric:{}", id).into_bytes();
        let data = state.serialize();
        backend.put(&key, &data).await?;
    }
    println!("✓ Stored all windows");

    // Retrieve and process windows
    for (id, _) in &windows {
        let key = format!("window:metric:{}", id).into_bytes();
        if let Some(data) = backend.get(&key).await? {
            let state = WindowState::deserialize(&data);
            println!("✓ Window {}: count={}, avg={:.2}, min={:.2}, max={:.2}",
                     state.window_id, state.count, state.avg(), state.min, state.max);
        }
    }

    // Clean up old windows
    let old_windows = backend.list_keys(b"window:metric:").await?;
    for window_key in &old_windows {
        backend.delete(window_key).await?;
    }
    println!("✓ Cleaned up {} old windows", old_windows.len());

    Ok(())
}

async fn multi_node_coordination_example(backend: Arc<RedisStateBackend>) -> anyhow::Result<()> {
    println!("Simulating multi-node coordination...");

    // Simulate 3 nodes processing partitions
    let nodes = vec!["node-1", "node-2", "node-3"];
    let partitions = vec![0, 1, 2, 3, 4, 5];

    // Each node claims partitions
    for (i, node) in nodes.iter().enumerate() {
        let partition = partitions[i % partitions.len()];
        let key = format!("partition:owner:{}", partition).into_bytes();
        backend.put(&key, node.as_bytes()).await?;
        println!("✓ {} claimed partition {}", node, partition);
    }

    // Check partition assignments
    println!("\nPartition assignments:");
    for partition in &partitions {
        let key = format!("partition:owner:{}", partition).into_bytes();
        if let Some(owner) = backend.get(&key).await? {
            println!("  - Partition {}: {:?}", partition, String::from_utf8_lossy(&owner));
        }
    }

    // Simulate node failure and rebalancing
    println!("\nSimulating node-2 failure...");
    backend.delete(b"partition:owner:1").await?;

    // Node-3 takes over
    backend.put(b"partition:owner:1", b"node-3").await?;
    println!("✓ node-3 took over partition 1");

    // Store node heartbeats with TTL
    for node in &nodes {
        if *node == "node-2" {
            continue; // Skip failed node
        }
        let key = format!("heartbeat:{}", node).into_bytes();
        let timestamp = chrono::Utc::now().timestamp().to_string();
        backend.put_with_ttl(&key, timestamp.as_bytes(), Duration::from_secs(10)).await?;
    }
    println!("✓ Updated heartbeats for active nodes");

    // Check which nodes are alive
    let heartbeats = backend.list_keys(b"heartbeat:").await?;
    println!("✓ Active nodes: {}", heartbeats.len());

    Ok(())
}

async fn fault_tolerance_example(backend: Arc<RedisStateBackend>) -> anyhow::Result<()> {
    println!("Demonstrating fault tolerance with checkpointing...");

    // Create some important state
    backend.put(b"config:max_connections", b"1000").await?;
    backend.put(b"config:timeout", b"30").await?;
    backend.put(b"config:retry_count", b"3").await?;

    // Store some processing state
    for i in 0..5 {
        let key = format!("state:processor:{}", i).into_bytes();
        let value = format!("offset:{}", i * 100).into_bytes();
        backend.put(&key, &value).await?;
    }

    let count = backend.count().await?;
    println!("✓ Created {} state entries", count);

    // Create checkpoint
    println!("Creating checkpoint...");
    let checkpoint = backend.checkpoint().await?;
    println!("✓ Checkpoint created with {} entries", checkpoint.len());

    // Simulate crash by modifying state
    println!("\nSimulating crash and data corruption...");
    backend.delete(b"config:max_connections").await?;
    backend.put(b"config:timeout", b"CORRUPTED").await?;
    backend.put(b"state:processor:2", b"INVALID").await?;
    println!("✓ State corrupted");

    // Restore from checkpoint
    println!("\nRestoring from checkpoint...");
    backend.clear().await?;
    backend.restore(checkpoint).await?;
    println!("✓ State restored");

    // Verify restoration
    if let Some(max_conn) = backend.get(b"config:max_connections").await? {
        println!("✓ Verified config:max_connections = {:?}", String::from_utf8_lossy(&max_conn));
    }

    if let Some(timeout) = backend.get(b"config:timeout").await? {
        println!("✓ Verified config:timeout = {:?}", String::from_utf8_lossy(&timeout));
    }

    let final_count = backend.count().await?;
    println!("✓ Final state entry count: {}", final_count);

    // Get final statistics
    let stats = backend.stats().await;
    println!("\nFinal statistics:");
    println!("  - Total operations: {}", stats.total_operations());
    println!("  - Hit rate: {:.2}%", stats.hit_rate() * 100.0);
    println!("  - Error rate: {:.2}%", stats.error_rate() * 100.0);

    Ok(())
}
