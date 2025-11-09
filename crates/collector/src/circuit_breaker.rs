//! Circuit Breaker Pattern
//!
//! This module implements a circuit breaker pattern for fault tolerance and fast failure.
//! It prevents cascading failures by detecting failures and opening the circuit to fail fast,
//! then automatically testing for recovery.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening circuit
    pub failure_threshold: u64,
    /// Duration to wait in open state before attempting half-open
    pub timeout: Duration,
    /// Number of successful requests needed in half-open state to close
    pub half_open_success_threshold: u32,
    /// Number of failures allowed in half-open state before reopening
    pub half_open_failure_threshold: u32,
    /// Sliding window size for failure rate calculation
    pub sliding_window_size: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            timeout: Duration::from_secs(30),
            half_open_success_threshold: 3,
            half_open_failure_threshold: 1,
            sliding_window_size: 100,
        }
    }
}

/// Circuit breaker states
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitState {
    /// Circuit is closed, requests flow normally
    Closed,
    /// Circuit is open, all requests fail fast
    Open {
        /// When the circuit was opened
        opened_at: Instant,
    },
    /// Circuit is testing recovery with limited requests
    HalfOpen {
        /// Number of successful requests in half-open state
        success_count: u32,
        /// Number of failed requests in half-open state
        failure_count: u32,
    },
}

impl CircuitState {
    /// Check if circuit is open
    pub fn is_open(&self) -> bool {
        matches!(self, CircuitState::Open { .. })
    }

    /// Check if circuit is closed
    pub fn is_closed(&self) -> bool {
        matches!(self, CircuitState::Closed)
    }

    /// Check if circuit is half-open
    pub fn is_half_open(&self) -> bool {
        matches!(self, CircuitState::HalfOpen { .. })
    }

    /// Get state name for metrics
    pub fn state_name(&self) -> &'static str {
        match self {
            CircuitState::Closed => "closed",
            CircuitState::Open { .. } => "open",
            CircuitState::HalfOpen { .. } => "half_open",
        }
    }
}

/// Circuit breaker statistics
#[derive(Debug, Clone, Default)]
pub struct CircuitBreakerStats {
    /// Total successful requests
    pub total_success: u64,
    /// Total failed requests
    pub total_failures: u64,
    /// Total rejected requests (when circuit is open)
    pub total_rejected: u64,
    /// Current consecutive failures
    pub consecutive_failures: u64,
    /// Number of times circuit has opened
    pub open_count: u64,
    /// Number of times circuit has closed
    pub close_count: u64,
    /// Current state
    pub current_state: String,
}

/// Circuit breaker for fault tolerance
pub struct CircuitBreaker {
    /// Current state
    state: Arc<RwLock<CircuitState>>,
    /// Configuration
    config: CircuitBreakerConfig,
    /// Total successful requests
    total_success: AtomicU64,
    /// Total failed requests
    total_failures: AtomicU64,
    /// Total rejected requests (circuit open)
    total_rejected: AtomicU64,
    /// Consecutive failures in closed state
    consecutive_failures: AtomicU64,
    /// Number of times circuit has opened
    open_count: AtomicU64,
    /// Number of times circuit has closed
    close_count: AtomicU64,
}

impl CircuitBreaker {
    /// Create new circuit breaker with default config
    pub fn new() -> Self {
        Self::with_config(CircuitBreakerConfig::default())
    }

    /// Create new circuit breaker with custom config
    pub fn with_config(config: CircuitBreakerConfig) -> Self {
        info!(
            "Creating circuit breaker: failure_threshold={}, timeout={:?}",
            config.failure_threshold, config.timeout
        );

        Self {
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            config,
            total_success: AtomicU64::new(0),
            total_failures: AtomicU64::new(0),
            total_rejected: AtomicU64::new(0),
            consecutive_failures: AtomicU64::new(0),
            open_count: AtomicU64::new(0),
            close_count: AtomicU64::new(0),
        }
    }

    /// Check if request is allowed
    pub async fn is_request_allowed(&self) -> bool {
        let state = self.state.read().await;

        match &*state {
            CircuitState::Closed => true,
            CircuitState::Open { opened_at } => {
                // Check if timeout has elapsed to transition to half-open
                if opened_at.elapsed() >= self.config.timeout {
                    drop(state); // Release read lock
                    self.transition_to_half_open().await;
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen { .. } => true,
        }
    }

    /// Record successful request
    pub async fn record_success(&self) {
        self.total_success.fetch_add(1, Ordering::Relaxed);
        self.consecutive_failures.store(0, Ordering::Relaxed);

        let mut state = self.state.write().await;

        match &*state {
            CircuitState::Closed => {
                // Already closed, nothing to do
                debug!("Request succeeded in closed state");
            }
            CircuitState::HalfOpen {
                success_count,
                failure_count,
            } => {
                let new_success_count = success_count + 1;
                debug!(
                    "Request succeeded in half-open state: {}/{}",
                    new_success_count, self.config.half_open_success_threshold
                );

                if new_success_count >= self.config.half_open_success_threshold {
                    // Enough successes, close the circuit
                    info!("Circuit breaker closing: enough successful requests in half-open state");
                    *state = CircuitState::Closed;
                    self.close_count.fetch_add(1, Ordering::Relaxed);
                } else {
                    // Update success count
                    *state = CircuitState::HalfOpen {
                        success_count: new_success_count,
                        failure_count: *failure_count,
                    };
                }
            }
            CircuitState::Open { .. } => {
                // This shouldn't happen, but handle it gracefully
                warn!("Received success in open state - should not happen");
            }
        }
    }

    /// Record failed request
    pub async fn record_failure(&self) {
        self.total_failures.fetch_add(1, Ordering::Relaxed);
        let consecutive = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;

        let mut state = self.state.write().await;

        match &*state {
            CircuitState::Closed => {
                debug!(
                    "Request failed in closed state: {}/{}",
                    consecutive, self.config.failure_threshold
                );

                if consecutive >= self.config.failure_threshold {
                    // Too many failures, open the circuit
                    warn!(
                        "Circuit breaker opening: {} consecutive failures exceeded threshold {}",
                        consecutive, self.config.failure_threshold
                    );
                    *state = CircuitState::Open {
                        opened_at: Instant::now(),
                    };
                    self.open_count.fetch_add(1, Ordering::Relaxed);
                }
            }
            CircuitState::HalfOpen {
                success_count,
                failure_count,
            } => {
                let new_failure_count = failure_count + 1;
                debug!(
                    "Request failed in half-open state: {}/{}",
                    new_failure_count, self.config.half_open_failure_threshold
                );

                if new_failure_count >= self.config.half_open_failure_threshold {
                    // Too many failures in half-open, reopen the circuit
                    warn!("Circuit breaker reopening: failures in half-open state");
                    *state = CircuitState::Open {
                        opened_at: Instant::now(),
                    };
                    self.open_count.fetch_add(1, Ordering::Relaxed);
                } else {
                    // Update failure count
                    *state = CircuitState::HalfOpen {
                        success_count: *success_count,
                        failure_count: new_failure_count,
                    };
                }
            }
            CircuitState::Open { .. } => {
                // Already open, nothing to do
                debug!("Additional failure recorded in open state");
            }
        }
    }

    /// Record rejected request (circuit open)
    pub fn record_rejection(&self) {
        self.total_rejected.fetch_add(1, Ordering::Relaxed);
    }

    /// Transition to half-open state
    async fn transition_to_half_open(&self) {
        let mut state = self.state.write().await;

        if let CircuitState::Open { opened_at } = &*state {
            if opened_at.elapsed() >= self.config.timeout {
                info!(
                    "Circuit breaker transitioning to half-open after {:?}",
                    opened_at.elapsed()
                );
                *state = CircuitState::HalfOpen {
                    success_count: 0,
                    failure_count: 0,
                };
            }
        }
    }

    /// Get current state
    pub async fn state(&self) -> CircuitState {
        self.state.read().await.clone()
    }

    /// Get statistics
    pub async fn stats(&self) -> CircuitBreakerStats {
        let state = self.state.read().await;

        CircuitBreakerStats {
            total_success: self.total_success.load(Ordering::Relaxed),
            total_failures: self.total_failures.load(Ordering::Relaxed),
            total_rejected: self.total_rejected.load(Ordering::Relaxed),
            consecutive_failures: self.consecutive_failures.load(Ordering::Relaxed),
            open_count: self.open_count.load(Ordering::Relaxed),
            close_count: self.close_count.load(Ordering::Relaxed),
            current_state: state.state_name().to_string(),
        }
    }

    /// Reset circuit breaker to closed state
    pub async fn reset(&self) {
        info!("Circuit breaker manually reset");
        let mut state = self.state.write().await;
        *state = CircuitState::Closed;
        self.consecutive_failures.store(0, Ordering::Relaxed);
    }

    /// Force circuit to open (for testing or manual intervention)
    pub async fn force_open(&self) {
        warn!("Circuit breaker manually forced open");
        let mut state = self.state.write().await;
        *state = CircuitState::Open {
            opened_at: Instant::now(),
        };
        self.open_count.fetch_add(1, Ordering::Relaxed);
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_circuit_breaker_default_config() {
        let config = CircuitBreakerConfig::default();
        assert_eq!(config.failure_threshold, 5);
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.half_open_success_threshold, 3);
    }

    #[tokio::test]
    async fn test_circuit_starts_closed() {
        let cb = CircuitBreaker::new();
        let state = cb.state().await;
        assert!(state.is_closed());
        assert!(cb.is_request_allowed().await);
    }

    #[tokio::test]
    async fn test_circuit_opens_after_threshold_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let cb = CircuitBreaker::with_config(config);

        // Record 3 failures
        for i in 1..=3 {
            cb.record_failure().await;
            let stats = cb.stats().await;
            assert_eq!(stats.total_failures, i);
        }

        // Circuit should now be open
        let state = cb.state().await;
        assert!(state.is_open());

        // Requests should be rejected
        assert!(!cb.is_request_allowed().await);

        let stats = cb.stats().await;
        assert_eq!(stats.open_count, 1);
    }

    #[tokio::test]
    async fn test_circuit_resets_on_success() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let cb = CircuitBreaker::with_config(config);

        // Record 2 failures
        cb.record_failure().await;
        cb.record_failure().await;

        let stats = cb.stats().await;
        assert_eq!(stats.consecutive_failures, 2);

        // Record success - should reset consecutive failures
        cb.record_success().await;

        let stats = cb.stats().await;
        assert_eq!(stats.consecutive_failures, 0);
        assert_eq!(stats.total_success, 1);

        // Circuit should still be closed
        let state = cb.state().await;
        assert!(state.is_closed());
    }

    #[tokio::test]
    async fn test_half_open_transitions() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_millis(100),
            half_open_success_threshold: 2,
            half_open_failure_threshold: 1,
            ..Default::default()
        };
        let cb = CircuitBreaker::with_config(config);

        // Open the circuit
        cb.record_failure().await;
        cb.record_failure().await;

        let state = cb.state().await;
        assert!(state.is_open());

        // Wait for timeout
        sleep(Duration::from_millis(150)).await;

        // Should transition to half-open
        assert!(cb.is_request_allowed().await);
        let state = cb.state().await;
        assert!(state.is_half_open());

        // Record successes to close
        cb.record_success().await;
        cb.record_success().await;

        let state = cb.state().await;
        assert!(state.is_closed());

        let stats = cb.stats().await;
        assert_eq!(stats.close_count, 1);
    }

    #[tokio::test]
    async fn test_half_open_reopens_on_failure() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_millis(100),
            half_open_success_threshold: 3,
            half_open_failure_threshold: 1,
            ..Default::default()
        };
        let cb = CircuitBreaker::with_config(config);

        // Open the circuit
        cb.record_failure().await;
        cb.record_failure().await;

        // Wait for timeout and transition to half-open
        sleep(Duration::from_millis(150)).await;
        assert!(cb.is_request_allowed().await);

        // Failure in half-open should reopen
        cb.record_failure().await;

        let state = cb.state().await;
        assert!(state.is_open());

        let stats = cb.stats().await;
        assert_eq!(stats.open_count, 2); // Opened twice
    }

    #[tokio::test]
    async fn test_manual_reset() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            ..Default::default()
        };
        let cb = CircuitBreaker::with_config(config);

        // Open the circuit
        cb.record_failure().await;
        cb.record_failure().await;

        let state = cb.state().await;
        assert!(state.is_open());

        // Manual reset
        cb.reset().await;

        let state = cb.state().await;
        assert!(state.is_closed());

        let stats = cb.stats().await;
        assert_eq!(stats.consecutive_failures, 0);
    }

    #[tokio::test]
    async fn test_force_open() {
        let cb = CircuitBreaker::new();

        // Initially closed
        let state = cb.state().await;
        assert!(state.is_closed());

        // Force open
        cb.force_open().await;

        let state = cb.state().await;
        assert!(state.is_open());

        assert!(!cb.is_request_allowed().await);
    }

    #[tokio::test]
    async fn test_rejection_tracking() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            ..Default::default()
        };
        let cb = CircuitBreaker::with_config(config);

        // Open the circuit
        cb.record_failure().await;
        cb.record_failure().await;

        // Record rejections
        for _ in 0..5 {
            if !cb.is_request_allowed().await {
                cb.record_rejection();
            }
        }

        let stats = cb.stats().await;
        assert_eq!(stats.total_rejected, 5);
    }

    #[tokio::test]
    async fn test_state_names() {
        assert_eq!(CircuitState::Closed.state_name(), "closed");
        assert_eq!(
            CircuitState::Open {
                opened_at: Instant::now()
            }
            .state_name(),
            "open"
        );
        assert_eq!(
            CircuitState::HalfOpen {
                success_count: 0,
                failure_count: 0
            }
            .state_name(),
            "half_open"
        );
    }
}
