//! Circuit breaker implementation for fault tolerance

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Circuit breaker for handling Redis failures
///
/// Implements the circuit breaker pattern to prevent cascading failures
/// when Redis is unavailable. Opens after consecutive failures and closes
/// after successful recovery.
pub struct CircuitBreaker {
    /// Current state of the circuit breaker
    state: Arc<RwLock<CircuitBreakerState>>,

    /// Number of consecutive failures before opening
    failure_threshold: usize,

    /// Time to wait before attempting to close after opening
    timeout: Duration,

    /// Counter for consecutive failures
    consecutive_failures: Arc<AtomicUsize>,

    /// Timestamp of the last failure
    last_failure_time: Arc<RwLock<Option<Instant>>>,

    /// Timestamp when circuit was opened
    opened_at: Arc<RwLock<Option<Instant>>>,
}

/// State of the circuit breaker
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CircuitBreakerState {
    /// Normal operation - all requests allowed
    Closed,

    /// Too many failures - requests fail fast
    Open,

    /// Testing if service recovered - limited requests allowed
    HalfOpen,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    ///
    /// # Arguments
    ///
    /// * `failure_threshold` - Number of consecutive failures before opening
    /// * `timeout` - Duration to wait before attempting recovery
    pub fn new(failure_threshold: usize, timeout: Duration) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitBreakerState::Closed)),
            failure_threshold,
            timeout,
            consecutive_failures: Arc::new(AtomicUsize::new(0)),
            last_failure_time: Arc::new(RwLock::new(None)),
            opened_at: Arc::new(RwLock::new(None)),
        }
    }

    /// Check if the circuit breaker is open
    ///
    /// Returns `true` if the circuit is open and requests should fail fast.
    pub async fn is_open(&self) -> bool {
        let state = *self.state.read().await;

        match state {
            CircuitBreakerState::Open => {
                // Check if timeout has elapsed
                if let Some(opened_time) = *self.opened_at.read().await {
                    if opened_time.elapsed() > self.timeout {
                        // Transition to half-open to test recovery
                        info!("Circuit breaker transitioning to half-open");
                        *self.state.write().await = CircuitBreakerState::HalfOpen;
                        return false;
                    }
                }
                true
            }
            _ => false,
        }
    }

    /// Record a successful operation
    ///
    /// Resets failure count and closes the circuit if it was open.
    pub async fn record_success(&self) {
        let prev_failures = self.consecutive_failures.swap(0, Ordering::SeqCst);
        let prev_state = *self.state.read().await;

        match prev_state {
            CircuitBreakerState::HalfOpen => {
                info!("Circuit breaker closing after successful recovery");
                *self.state.write().await = CircuitBreakerState::Closed;
                *self.opened_at.write().await = None;
            }
            CircuitBreakerState::Open => {
                // Should not happen, but handle gracefully
                warn!("Success recorded while circuit was open");
                *self.state.write().await = CircuitBreakerState::Closed;
                *self.opened_at.write().await = None;
            }
            CircuitBreakerState::Closed => {
                if prev_failures > 0 {
                    debug!("Failure count reset after success");
                }
            }
        }
    }

    /// Record a failed operation
    ///
    /// Increments failure count and opens circuit if threshold is exceeded.
    pub async fn record_failure(&self) {
        let failures = self.consecutive_failures.fetch_add(1, Ordering::SeqCst) + 1;
        *self.last_failure_time.write().await = Some(Instant::now());

        let current_state = *self.state.read().await;

        match current_state {
            CircuitBreakerState::HalfOpen => {
                // Failed during recovery attempt - reopen circuit
                warn!(
                    "Circuit breaker reopening after failed recovery attempt (failure #{})",
                    failures
                );
                *self.state.write().await = CircuitBreakerState::Open;
                *self.opened_at.write().await = Some(Instant::now());
            }
            CircuitBreakerState::Closed => {
                if failures >= self.failure_threshold {
                    warn!(
                        "Circuit breaker opening after {} consecutive failures",
                        failures
                    );
                    *self.state.write().await = CircuitBreakerState::Open;
                    *self.opened_at.write().await = Some(Instant::now());
                } else {
                    debug!("Failure recorded: {} of {}", failures, self.failure_threshold);
                }
            }
            CircuitBreakerState::Open => {
                // Already open, just log
                debug!("Failure recorded while circuit is open");
            }
        }
    }

    /// Get current state
    pub async fn state(&self) -> CircuitBreakerState {
        *self.state.read().await
    }

    /// Get consecutive failure count
    pub fn failure_count(&self) -> usize {
        self.consecutive_failures.load(Ordering::SeqCst)
    }

    /// Get time since last failure
    pub async fn time_since_last_failure(&self) -> Option<Duration> {
        self.last_failure_time
            .read()
            .await
            .as_ref()
            .map(|t| t.elapsed())
    }

    /// Get time since circuit was opened
    pub async fn time_since_opened(&self) -> Option<Duration> {
        self.opened_at.read().await.as_ref().map(|t| t.elapsed())
    }

    /// Reset the circuit breaker to closed state
    ///
    /// **Warning**: Use with caution in production!
    pub async fn reset(&self) {
        info!("Circuit breaker manually reset");
        *self.state.write().await = CircuitBreakerState::Closed;
        self.consecutive_failures.store(0, Ordering::SeqCst);
        *self.last_failure_time.write().await = None;
        *self.opened_at.write().await = None;
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(5, Duration::from_secs(30))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_circuit_breaker_opens_after_threshold() {
        let cb = CircuitBreaker::new(3, Duration::from_secs(10));

        assert!(!cb.is_open().await);
        assert_eq!(cb.state().await, CircuitBreakerState::Closed);

        // Record failures
        cb.record_failure().await;
        assert!(!cb.is_open().await);
        assert_eq!(cb.failure_count(), 1);

        cb.record_failure().await;
        assert!(!cb.is_open().await);
        assert_eq!(cb.failure_count(), 2);

        cb.record_failure().await;
        assert!(cb.is_open().await);
        assert_eq!(cb.state().await, CircuitBreakerState::Open);
        assert_eq!(cb.failure_count(), 3);
    }

    #[tokio::test]
    async fn test_circuit_breaker_closes_after_success() {
        let cb = CircuitBreaker::new(2, Duration::from_secs(10));

        // Open the circuit
        cb.record_failure().await;
        cb.record_failure().await;
        assert!(cb.is_open().await);

        // Record success - should close
        cb.record_success().await;
        assert!(!cb.is_open().await);
        assert_eq!(cb.state().await, CircuitBreakerState::Closed);
        assert_eq!(cb.failure_count(), 0);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_transition() {
        let cb = CircuitBreaker::new(2, Duration::from_millis(100));

        // Open the circuit
        cb.record_failure().await;
        cb.record_failure().await;
        assert!(cb.is_open().await);

        // Wait for timeout
        sleep(Duration::from_millis(150)).await;

        // Should transition to half-open
        assert!(!cb.is_open().await);
        assert_eq!(cb.state().await, CircuitBreakerState::HalfOpen);

        // Success in half-open should close
        cb.record_success().await;
        assert_eq!(cb.state().await, CircuitBreakerState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_failure() {
        let cb = CircuitBreaker::new(2, Duration::from_millis(100));

        // Open the circuit
        cb.record_failure().await;
        cb.record_failure().await;
        assert!(cb.is_open().await);

        // Wait for timeout and transition to half-open
        sleep(Duration::from_millis(150)).await;
        let _ = cb.is_open().await; // Trigger transition

        // Failure in half-open should reopen
        cb.record_failure().await;
        assert_eq!(cb.state().await, CircuitBreakerState::Open);
    }

    #[tokio::test]
    async fn test_circuit_breaker_reset() {
        let cb = CircuitBreaker::new(2, Duration::from_secs(10));

        // Open the circuit
        cb.record_failure().await;
        cb.record_failure().await;
        assert!(cb.is_open().await);

        // Reset
        cb.reset().await;
        assert!(!cb.is_open().await);
        assert_eq!(cb.state().await, CircuitBreakerState::Closed);
        assert_eq!(cb.failure_count(), 0);
    }

    #[tokio::test]
    async fn test_circuit_breaker_time_tracking() {
        let cb = CircuitBreaker::new(2, Duration::from_secs(10));

        assert!(cb.time_since_last_failure().await.is_none());

        cb.record_failure().await;
        sleep(Duration::from_millis(50)).await;

        let time = cb.time_since_last_failure().await.unwrap();
        assert!(time >= Duration::from_millis(50));
    }
}
