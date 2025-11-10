//! Statistics and health tracking for deduplication operations

use super::circuit_breaker::CircuitBreakerState;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Statistics for deduplication operations
///
/// Tracks all deduplication metrics including duplicate rate, errors,
/// latency, and circuit breaker state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeduplicationStats {
    /// Total number of events checked for deduplication
    pub total_checked: u64,

    /// Number of duplicate events detected and filtered
    pub duplicates_found: u64,

    /// Number of unique events allowed through
    pub unique_events: u64,

    /// Number of Redis errors encountered
    pub redis_errors: u64,

    /// Number of ID extraction failures
    pub extraction_failures: u64,

    /// Number of fallback decisions (events allowed due to errors)
    pub fallback_count: u64,

    /// Total processing time across all checks (microseconds)
    pub total_latency_us: u64,

    /// Average latency per check (microseconds)
    pub avg_latency_us: u64,

    /// P50 latency (microseconds)
    pub p50_latency_us: u64,

    /// P95 latency (microseconds)
    pub p95_latency_us: u64,

    /// P99 latency (microseconds)
    pub p99_latency_us: u64,

    /// Last successful check timestamp
    pub last_check_time: Option<DateTime<Utc>>,

    /// Last error timestamp
    pub last_error_time: Option<DateTime<Utc>>,

    /// Current circuit breaker state
    pub circuit_breaker_state: CircuitBreakerState,
}

impl DeduplicationStats {
    /// Calculate the duplication rate (0.0 to 1.0)
    ///
    /// Returns the ratio of duplicate events to total checked events.
    pub fn duplication_rate(&self) -> f64 {
        if self.total_checked == 0 {
            0.0
        } else {
            self.duplicates_found as f64 / self.total_checked as f64
        }
    }

    /// Calculate the error rate (0.0 to 1.0)
    ///
    /// Returns the ratio of errors to total checked events.
    pub fn error_rate(&self) -> f64 {
        if self.total_checked == 0 {
            0.0
        } else {
            (self.redis_errors + self.extraction_failures) as f64 / self.total_checked as f64
        }
    }

    /// Calculate the fallback rate (0.0 to 1.0)
    ///
    /// Returns the ratio of fallback decisions to total checked events.
    pub fn fallback_rate(&self) -> f64 {
        if self.total_checked == 0 {
            0.0
        } else {
            self.fallback_count as f64 / self.total_checked as f64
        }
    }

    /// Calculate the success rate (0.0 to 1.0)
    ///
    /// Returns the ratio of successful checks to total checked events.
    pub fn success_rate(&self) -> f64 {
        1.0 - self.error_rate()
    }

    /// Check if the system is healthy
    ///
    /// Considers error rate and circuit breaker state.
    pub fn is_healthy(&self) -> bool {
        self.error_rate() < 0.05 && // Less than 5% error rate
        matches!(self.circuit_breaker_state, CircuitBreakerState::Closed)
    }

    /// Get a human-readable summary
    pub fn summary(&self) -> String {
        format!(
            "Dedup Stats: checked={}, unique={}, duplicates={} ({:.1}%), errors={} ({:.1}%), fallbacks={} ({:.1}%), avg_latency={}us",
            self.total_checked,
            self.unique_events,
            self.duplicates_found,
            self.duplication_rate() * 100.0,
            self.redis_errors + self.extraction_failures,
            self.error_rate() * 100.0,
            self.fallback_count,
            self.fallback_rate() * 100.0,
            self.avg_latency_us,
        )
    }
}

/// Health status for the deduplication system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Overall health state
    pub status: HealthState,

    /// Whether Redis is available
    pub redis_available: bool,

    /// Current circuit breaker state
    pub circuit_breaker_state: CircuitBreakerState,

    /// Last successful check timestamp
    pub last_check_time: Option<DateTime<Utc>>,

    /// Last error timestamp
    pub last_error_time: Option<DateTime<Utc>>,

    /// Current statistics
    pub stats: DeduplicationStats,

    /// Additional diagnostic messages
    pub messages: Vec<String>,
}

impl HealthStatus {
    /// Check if the system is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self.status, HealthState::Healthy)
    }

    /// Check if the system is degraded
    pub fn is_degraded(&self) -> bool {
        matches!(self.status, HealthState::Degraded)
    }

    /// Check if the system is unhealthy
    pub fn is_unhealthy(&self) -> bool {
        matches!(self.status, HealthState::Unhealthy)
    }
}

/// Overall health state of the deduplication system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthState {
    /// System is fully operational
    Healthy,

    /// System is operational but with degraded performance
    Degraded,

    /// System is not operational
    Unhealthy,
}

impl std::fmt::Display for HealthState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthState::Healthy => write!(f, "healthy"),
            HealthState::Degraded => write!(f, "degraded"),
            HealthState::Unhealthy => write!(f, "unhealthy"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stats_default() {
        let stats = DeduplicationStats::default();
        assert_eq!(stats.total_checked, 0);
        assert_eq!(stats.duplicates_found, 0);
        assert_eq!(stats.duplication_rate(), 0.0);
    }

    #[test]
    fn test_duplication_rate() {
        let mut stats = DeduplicationStats::default();
        stats.total_checked = 100;
        stats.duplicates_found = 10;
        stats.unique_events = 90;

        assert_eq!(stats.duplication_rate(), 0.1);
    }

    #[test]
    fn test_error_rate() {
        let mut stats = DeduplicationStats::default();
        stats.total_checked = 100;
        stats.redis_errors = 5;
        stats.extraction_failures = 3;

        assert_eq!(stats.error_rate(), 0.08);
    }

    #[test]
    fn test_fallback_rate() {
        let mut stats = DeduplicationStats::default();
        stats.total_checked = 100;
        stats.fallback_count = 15;

        assert_eq!(stats.fallback_rate(), 0.15);
    }

    #[test]
    fn test_success_rate() {
        let mut stats = DeduplicationStats::default();
        stats.total_checked = 100;
        stats.redis_errors = 2;
        stats.extraction_failures = 3;

        assert_eq!(stats.success_rate(), 0.95);
    }

    #[test]
    fn test_is_healthy() {
        let mut stats = DeduplicationStats::default();
        stats.total_checked = 1000;
        stats.redis_errors = 10; // 1% error rate

        assert!(stats.is_healthy());

        stats.redis_errors = 60; // 6% error rate
        assert!(!stats.is_healthy());
    }

    #[test]
    fn test_summary() {
        let mut stats = DeduplicationStats::default();
        stats.total_checked = 100;
        stats.duplicates_found = 10;
        stats.unique_events = 90;
        stats.avg_latency_us = 1500;

        let summary = stats.summary();
        assert!(summary.contains("checked=100"));
        assert!(summary.contains("duplicates=10"));
    }

    #[test]
    fn test_health_state_display() {
        assert_eq!(HealthState::Healthy.to_string(), "healthy");
        assert_eq!(HealthState::Degraded.to_string(), "degraded");
        assert_eq!(HealthState::Unhealthy.to_string(), "unhealthy");
    }

    #[test]
    fn test_health_status_checks() {
        let status = HealthStatus {
            status: HealthState::Healthy,
            redis_available: true,
            circuit_breaker_state: CircuitBreakerState::Closed,
            last_check_time: None,
            last_error_time: None,
            stats: DeduplicationStats::default(),
            messages: vec![],
        };

        assert!(status.is_healthy());
        assert!(!status.is_degraded());
        assert!(!status.is_unhealthy());
    }
}
