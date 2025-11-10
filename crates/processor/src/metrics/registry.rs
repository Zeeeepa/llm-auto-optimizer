//! Global metrics registry for Prometheus metrics
//!
//! This module provides a singleton registry for all processor metrics,
//! ensuring thread-safe access and metric registration.

use prometheus_client::registry::Registry;
use std::sync::{Arc, OnceLock};
use parking_lot::RwLock;

/// Global metrics registry instance
pub static METRICS_REGISTRY: OnceLock<Arc<MetricsRegistry>> = OnceLock::new();

/// Thread-safe registry for Prometheus metrics
pub struct MetricsRegistry {
    registry: Arc<RwLock<Registry>>,
}

impl MetricsRegistry {
    /// Create a new metrics registry
    pub fn new() -> Self {
        Self {
            registry: Arc::new(RwLock::new(Registry::default())),
        }
    }

    /// Get the global metrics registry, initializing it if necessary
    pub fn global() -> Arc<MetricsRegistry> {
        METRICS_REGISTRY
            .get_or_init(|| Arc::new(MetricsRegistry::new()))
            .clone()
    }

    /// Get a reference to the internal registry
    pub fn registry(&self) -> Arc<RwLock<Registry>> {
        self.registry.clone()
    }

    /// Encode metrics in Prometheus text format
    pub fn encode(&self) -> Result<String, crate::metrics::MetricsError> {
        let registry = self.registry.read();
        let mut buffer = String::new();

        prometheus_client::encoding::text::encode(&mut buffer, &registry)
            .map_err(|e| crate::metrics::MetricsError::EncodingError(e.to_string()))?;

        Ok(buffer)
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for MetricsRegistry {
    fn clone(&self) -> Self {
        Self {
            registry: self.registry.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = MetricsRegistry::new();
        assert!(registry.encode().is_ok());
    }

    #[test]
    fn test_global_registry() {
        let registry1 = MetricsRegistry::global();
        let registry2 = MetricsRegistry::global();

        // Should return the same instance
        assert!(Arc::ptr_eq(&registry1, &registry2));
    }

    #[test]
    fn test_registry_clone() {
        let registry1 = MetricsRegistry::new();
        let registry2 = registry1.clone();

        // Both should share the same underlying registry
        assert!(Arc::ptr_eq(&registry1.registry(), &registry2.registry()));
    }
}
