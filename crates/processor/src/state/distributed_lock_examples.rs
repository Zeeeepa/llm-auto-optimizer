//! Example usage patterns for distributed locking
//!
//! This module contains comprehensive examples demonstrating how to use
//! distributed locks for various coordination scenarios in a distributed
//! stream processing system.

#![allow(dead_code)]

use super::distributed_lock::{DistributedLock, LockGuard};
use super::postgres_lock::PostgresDistributedLock;
use super::redis_lock::{RedisDistributedLock, RetryConfig};
use super::{CheckpointCoordinator, StateBackend};
use crate::error::StateResult;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

/// Example: Coordinated checkpoint creation
///
/// This example shows how to use distributed locks to ensure only one
/// processor instance creates a checkpoint at a time.
pub async fn coordinated_checkpoint<B: StateBackend + 'static>(
    backend: Arc<B>,
    lock: Arc<dyn DistributedLock>,
    checkpoint_dir: &str,
) -> StateResult<()> {
    info!("Attempting to acquire checkpoint lock...");

    // Try to acquire lock with 60-second TTL
    // If another instance is checkpointing, this will wait
    let guard = lock
        .acquire("global:checkpoint", Duration::from_secs(60))
        .await?;

    info!("Checkpoint lock acquired, creating checkpoint...");

    // Create checkpoint coordinator
    let coordinator = CheckpointCoordinator::new(
        backend,
        Duration::from_secs(300),
        checkpoint_dir.into(),
    );

    // Create checkpoint
    let checkpoint = coordinator.create_checkpoint().await?;

    info!(
        checkpoint_id = %checkpoint.metadata().id,
        "Checkpoint created successfully"
    );

    // Lock is automatically released when guard is dropped
    drop(guard);

    Ok(())
}

/// Example: Leader election
///
/// This example demonstrates how to use distributed locks for leader election
/// in a cluster of processor instances.
pub struct LeaderElection {
    lock: Arc<dyn DistributedLock>,
    resource: String,
    ttl: Duration,
    guard: Option<LockGuard>,
}

impl LeaderElection {
    pub fn new(lock: Arc<dyn DistributedLock>, cluster_id: &str, ttl: Duration) -> Self {
        Self {
            lock,
            resource: format!("leader:{}", cluster_id),
            ttl,
            guard: None,
        }
    }

    /// Try to become the leader
    pub async fn try_become_leader(&mut self) -> StateResult<bool> {
        if self.guard.is_some() {
            // Already the leader
            return Ok(true);
        }

        match self.lock.try_acquire(&self.resource, self.ttl).await? {
            Some(guard) => {
                info!("Became cluster leader");
                self.guard = Some(guard);
                Ok(true)
            }
            None => {
                info!("Another instance is the leader");
                Ok(false)
            }
        }
    }

    /// Check if this instance is the leader
    pub fn is_leader(&self) -> bool {
        self.guard.is_some()
    }

    /// Extend leadership (call periodically)
    pub async fn maintain_leadership(&mut self) -> StateResult<()> {
        if let Some(ref mut guard) = self.guard {
            // Extend the lock before it expires
            self.lock.extend(guard, self.ttl).await?;
            info!("Leadership extended");
            Ok(())
        } else {
            warn!("Not the leader, cannot maintain leadership");
            Ok(())
        }
    }

    /// Resign from leadership
    pub async fn resign(&mut self) -> StateResult<()> {
        if let Some(guard) = self.guard.take() {
            self.lock.release(guard).await?;
            info!("Resigned from leadership");
        }
        Ok(())
    }
}

/// Example: State compaction with distributed locking
///
/// This example shows how to use locks to prevent concurrent compaction
/// of the same state partition.
pub async fn compact_state_partition(
    lock: Arc<dyn DistributedLock>,
    partition_id: u32,
    backend: Arc<dyn StateBackend>,
) -> StateResult<()> {
    let resource = format!("compaction:partition:{}", partition_id);

    info!(partition = partition_id, "Attempting to acquire compaction lock");

    // Try to acquire lock - if another instance is compacting, skip
    match lock
        .try_acquire(&resource, Duration::from_secs(300))
        .await?
    {
        Some(guard) => {
            info!(partition = partition_id, "Starting compaction");

            // Perform compaction
            compact_partition(partition_id, backend).await?;

            info!(partition = partition_id, "Compaction completed");

            // Release lock
            drop(guard);

            Ok(())
        }
        None => {
            info!(
                partition = partition_id,
                "Compaction already in progress by another instance"
            );
            Ok(())
        }
    }
}

async fn compact_partition(_partition_id: u32, _backend: Arc<dyn StateBackend>) -> StateResult<()> {
    // Simulate compaction work
    tokio::time::sleep(Duration::from_secs(1)).await;
    Ok(())
}

/// Example: Critical section with automatic retry
///
/// This example demonstrates retrying lock acquisition with exponential backoff.
pub async fn critical_section_with_retry<F, T>(
    lock: Arc<RedisDistributedLock>,
    resource: &str,
    f: F,
) -> StateResult<T>
where
    F: FnOnce() -> T,
{
    info!(resource = %resource, "Acquiring lock for critical section");

    // Acquire lock with retry
    let guard = lock.acquire(resource, Duration::from_secs(30)).await?;

    info!(resource = %resource, "Lock acquired, executing critical section");

    // Execute critical section
    let result = f();

    info!(resource = %resource, "Critical section completed");

    // Release lock
    drop(guard);

    Ok(result)
}

/// Example: Long-running operation with lock extension
///
/// This example shows how to extend a lock during a long-running operation.
pub async fn long_running_operation_with_lock_extension(
    lock: Arc<dyn DistributedLock>,
    resource: &str,
) -> StateResult<()> {
    info!(resource = %resource, "Starting long-running operation");

    // Acquire lock with initial 30-second TTL
    let mut guard = lock.acquire(resource, Duration::from_secs(30)).await?;

    // Simulate long-running operation with multiple phases
    for phase in 1..=5 {
        info!(
            resource = %resource,
            phase = phase,
            "Processing phase"
        );

        // Simulate work (20 seconds per phase)
        tokio::time::sleep(Duration::from_secs(20)).await;

        // Extend lock before it expires (every 20 seconds, TTL is 30)
        lock.extend(&mut guard, Duration::from_secs(30)).await?;

        info!(
            resource = %resource,
            phase = phase,
            "Lock extended, phase completed"
        );
    }

    info!(resource = %resource, "Long-running operation completed");

    Ok(())
}

/// Example: Distributed task queue with work stealing prevention
///
/// This example demonstrates using locks to claim tasks from a queue
/// and prevent work stealing.
pub struct DistributedTaskQueue {
    lock: Arc<dyn DistributedLock>,
    task_prefix: String,
}

impl DistributedTaskQueue {
    pub fn new(lock: Arc<dyn DistributedLock>, queue_name: &str) -> Self {
        Self {
            lock,
            task_prefix: format!("task:{}", queue_name),
        }
    }

    /// Try to claim and execute a task
    pub async fn claim_and_execute_task(&self, task_id: &str) -> StateResult<bool> {
        let resource = format!("{}:{}", self.task_prefix, task_id);

        // Try to claim the task
        match self
            .lock
            .try_acquire(&resource, Duration::from_secs(60))
            .await?
        {
            Some(guard) => {
                info!(task_id = %task_id, "Task claimed, executing");

                // Execute task
                self.execute_task(task_id).await?;

                info!(task_id = %task_id, "Task completed");

                // Release lock
                drop(guard);

                Ok(true)
            }
            None => {
                info!(
                    task_id = %task_id,
                    "Task already claimed by another worker"
                );
                Ok(false)
            }
        }
    }

    async fn execute_task(&self, _task_id: &str) -> StateResult<()> {
        // Simulate task execution
        tokio::time::sleep(Duration::from_secs(1)).await;
        Ok(())
    }
}

/// Example: Multi-resource locking with deadlock avoidance
///
/// This example shows how to acquire multiple locks in a consistent order
/// to avoid deadlocks.
pub async fn multi_resource_operation(
    lock: Arc<dyn DistributedLock>,
    resource1: &str,
    resource2: &str,
) -> StateResult<()> {
    // Always acquire locks in alphabetical order to prevent deadlocks
    let (first, second) = if resource1 < resource2 {
        (resource1, resource2)
    } else {
        (resource2, resource1)
    };

    info!(first = %first, "Acquiring first lock");
    let guard1 = lock.acquire(first, Duration::from_secs(30)).await?;

    info!(second = %second, "Acquiring second lock");
    let guard2 = lock.acquire(second, Duration::from_secs(30)).await?;

    info!("Both locks acquired, performing operation");

    // Perform operation requiring both resources
    tokio::time::sleep(Duration::from_millis(100)).await;

    info!("Operation completed, releasing locks");

    // Locks released in reverse order (LIFO) via drop
    drop(guard2);
    drop(guard1);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::MemoryStateBackend;

    // These tests require Redis/PostgreSQL running
    // Run with: cargo test --features redis,postgres -- --ignored

    #[tokio::test]
    #[ignore]
    async fn test_leader_election() {
        let lock = Arc::new(
            RedisDistributedLock::with_config(
                "redis://localhost:6379",
                "test:",
                RetryConfig::no_retry(),
            )
            .await
            .unwrap(),
        );

        let mut election = LeaderElection::new(lock, "test-cluster", Duration::from_secs(5));

        // Should become leader
        assert!(election.try_become_leader().await.unwrap());
        assert!(election.is_leader());

        // Should maintain leadership
        election.maintain_leadership().await.unwrap();
        assert!(election.is_leader());

        // Should resign
        election.resign().await.unwrap();
        assert!(!election.is_leader());
    }

    #[tokio::test]
    #[ignore]
    async fn test_coordinated_checkpoint() {
        let backend = Arc::new(MemoryStateBackend::new(None));
        let lock = Arc::new(
            RedisDistributedLock::new("redis://localhost:6379")
                .await
                .unwrap(),
        );

        // Create a temporary directory for checkpoints
        let temp_dir = tempfile::tempdir().unwrap();

        coordinated_checkpoint(backend, lock, temp_dir.path().to_str().unwrap())
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn test_task_queue() {
        let lock = Arc::new(
            RedisDistributedLock::new("redis://localhost:6379")
                .await
                .unwrap(),
        );

        let queue = DistributedTaskQueue::new(lock, "test-queue");

        // Should claim and execute task
        assert!(queue.claim_and_execute_task("task1").await.unwrap());

        // Should not claim already-completed task (in real scenario)
        // This will succeed because the lock was released
        assert!(queue.claim_and_execute_task("task2").await.unwrap());
    }

    #[tokio::test]
    #[ignore]
    async fn test_concurrent_leader_election() {
        let lock = Arc::new(
            RedisDistributedLock::new("redis://localhost:6379")
                .await
                .unwrap(),
        );

        let cluster_id = format!("test-cluster-{}", uuid::Uuid::new_v4());

        // Spawn 5 instances trying to become leader
        let mut handles = vec![];

        for i in 0..5 {
            let lock = lock.clone();
            let cluster_id = cluster_id.clone();

            let handle = tokio::spawn(async move {
                let mut election = LeaderElection::new(
                    lock as Arc<dyn DistributedLock>,
                    &cluster_id,
                    Duration::from_secs(5),
                );

                election.try_become_leader().await.unwrap()
            });

            handles.push(handle);
        }

        // Collect results
        let results: Vec<bool> = futures::future::join_all(handles)
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        // Exactly one should have become leader
        assert_eq!(results.iter().filter(|&&r| r).count(), 1);
    }
}
