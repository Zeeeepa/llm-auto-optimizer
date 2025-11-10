//! Configuration Manager for Actuator
//!
//! This module provides configuration management capabilities including:
//! - Configuration application and reversion
//! - Snapshot creation and restoration
//! - Configuration versioning
//! - Validation before application
//! - Audit logging for compliance
//! - Dry-run mode for safe testing

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

use super::config::ConfigurationManagementConfig;
use super::error::{ActuatorError, ActuatorResult};
use super::traits::ConfigurationManager;
use super::types::ConfigurationSnapshot;
use crate::decision::ConfigChange;

/// Configuration Manager implementation
pub struct ConfigurationManagerImpl {
    /// Configuration
    config: ConfigurationManagementConfig,

    /// Current configuration values
    current_config: HashMap<String, serde_json::Value>,

    /// Configuration snapshots (snapshot_id -> snapshot)
    snapshots: HashMap<String, ConfigurationSnapshot>,

    /// Configuration history (version -> changes)
    history: Vec<ConfigurationVersion>,

    /// Current version number
    current_version: u64,

    /// Audit log entries
    audit_log: Vec<AuditLogEntry>,

    /// Dry-run mode flag
    dry_run_mode: bool,

    /// Configuration backends for different config types
    backends: HashMap<String, Box<dyn ConfigurationBackend>>,
}

/// Configuration version entry
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConfigurationVersion {
    /// Version number
    version: u64,

    /// Timestamp
    timestamp: DateTime<Utc>,

    /// Changes applied in this version
    changes: Vec<ConfigChange>,

    /// Who applied the changes
    applied_by: String,

    /// Description
    description: String,

    /// Snapshot ID before this version (for rollback)
    snapshot_id: String,
}

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    /// Entry ID
    pub id: String,

    /// Timestamp
    pub timestamp: DateTime<Utc>,

    /// Operation type (apply, revert, snapshot, restore)
    pub operation: String,

    /// User/system that performed the operation
    pub actor: String,

    /// Changes involved
    pub changes: Vec<ConfigChange>,

    /// Success or failure
    pub success: bool,

    /// Error message if failed
    pub error: Option<String>,

    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Configuration diff entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigurationDiff {
    /// Path to the configuration
    pub path: String,

    /// Old value (if any)
    pub old_value: Option<serde_json::Value>,

    /// New value
    pub new_value: serde_json::Value,

    /// Type of change (add, update, delete)
    pub change_type: DiffType,
}

/// Type of configuration change
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffType {
    /// Adding a new configuration
    Add,
    /// Updating existing configuration
    Update,
    /// Deleting configuration
    Delete,
}

/// Trait for configuration backends
#[async_trait]
pub trait ConfigurationBackend: Send + Sync {
    /// Get configuration value
    async fn get(&self, path: &str) -> ActuatorResult<Option<serde_json::Value>>;

    /// Set configuration value
    async fn set(&mut self, path: &str, value: serde_json::Value) -> ActuatorResult<()>;

    /// Delete configuration value
    async fn delete(&mut self, path: &str) -> ActuatorResult<()>;

    /// Validate configuration
    async fn validate(&self, path: &str, value: &serde_json::Value) -> ActuatorResult<()>;

    /// Get all configuration values
    async fn get_all(&self) -> ActuatorResult<HashMap<String, serde_json::Value>>;
}

/// In-memory configuration backend (for testing and simple cases)
#[derive(Debug, Clone)]
pub struct InMemoryBackend {
    config: HashMap<String, serde_json::Value>,
}

impl InMemoryBackend {
    pub fn new() -> Self {
        Self {
            config: HashMap::new(),
        }
    }

    pub fn with_initial_config(config: HashMap<String, serde_json::Value>) -> Self {
        Self { config }
    }
}

impl Default for InMemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ConfigurationBackend for InMemoryBackend {
    async fn get(&self, path: &str) -> ActuatorResult<Option<serde_json::Value>> {
        Ok(self.config.get(path).cloned())
    }

    async fn set(&mut self, path: &str, value: serde_json::Value) -> ActuatorResult<()> {
        self.config.insert(path.to_string(), value);
        Ok(())
    }

    async fn delete(&mut self, path: &str) -> ActuatorResult<()> {
        self.config.remove(path);
        Ok(())
    }

    async fn validate(&self, path: &str, value: &serde_json::Value) -> ActuatorResult<()> {
        // Basic validation - check that value is not null
        if value.is_null() {
            return Err(ActuatorError::ValidationFailed(format!(
                "Configuration value at '{}' cannot be null",
                path
            )));
        }

        // Path-specific validation
        if path.contains("..") || path.starts_with('/') {
            return Err(ActuatorError::ValidationFailed(format!(
                "Invalid configuration path: '{}'",
                path
            )));
        }

        Ok(())
    }

    async fn get_all(&self) -> ActuatorResult<HashMap<String, serde_json::Value>> {
        Ok(self.config.clone())
    }
}

impl ConfigurationManagerImpl {
    /// Create a new Configuration Manager
    pub fn new(config: ConfigurationManagementConfig) -> Self {
        Self {
            dry_run_mode: config.enable_dry_run,
            config,
            current_config: HashMap::new(),
            snapshots: HashMap::new(),
            history: Vec::new(),
            current_version: 0,
            audit_log: Vec::new(),
            backends: HashMap::new(),
        }
    }

    /// Register a configuration backend
    pub fn register_backend(&mut self, name: impl Into<String>, backend: Box<dyn ConfigurationBackend>) {
        self.backends.insert(name.into(), backend);
    }

    /// Get backend for a configuration type
    fn get_backend_mut(&mut self, config_type: &str) -> ActuatorResult<&mut Box<dyn ConfigurationBackend>> {
        self.backends
            .get_mut(config_type)
            .ok_or_else(|| ActuatorError::ConfigError(format!(
                "No backend registered for config type: {}",
                config_type
            )))
    }

    /// Set dry-run mode
    pub fn set_dry_run(&mut self, enabled: bool) {
        self.dry_run_mode = enabled;
    }

    /// Get dry-run mode status
    pub fn is_dry_run(&self) -> bool {
        self.dry_run_mode
    }

    /// Calculate diff between current config and proposed changes
    pub fn calculate_diff(&self, changes: &[ConfigChange]) -> Vec<ConfigurationDiff> {
        let mut diffs = Vec::new();

        for change in changes {
            let diff_type = if change.old_value.is_none() {
                DiffType::Add
            } else if change.new_value.is_null() {
                DiffType::Delete
            } else {
                DiffType::Update
            };

            diffs.push(ConfigurationDiff {
                path: change.path.clone(),
                old_value: change.old_value.clone(),
                new_value: change.new_value.clone(),
                change_type: diff_type,
            });
        }

        diffs
    }

    /// Get configuration history
    pub fn get_history(&self) -> &[ConfigurationVersion] {
        &self.history
    }

    /// Get audit log
    pub fn get_audit_log(&self) -> &[AuditLogEntry] {
        &self.audit_log
    }

    /// Get snapshot by ID
    pub fn get_snapshot(&self, snapshot_id: &str) -> Option<&ConfigurationSnapshot> {
        self.snapshots.get(snapshot_id)
    }

    /// List all snapshots
    pub fn list_snapshots(&self) -> Vec<&ConfigurationSnapshot> {
        let mut snapshots: Vec<_> = self.snapshots.values().collect();
        snapshots.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        snapshots
    }

    /// Cleanup old snapshots
    fn cleanup_old_snapshots(&mut self) {
        if self.snapshots.len() > self.config.max_snapshots {
            // Keep only the most recent snapshots
            let mut snapshot_list: Vec<_> = self.snapshots.iter()
                .map(|(id, snap)| (id.clone(), snap.timestamp))
                .collect();

            snapshot_list.sort_by(|a, b| b.1.cmp(&a.1));

            // Remove oldest snapshots
            for (id, _) in snapshot_list.iter().skip(self.config.max_snapshots) {
                self.snapshots.remove(id);
                debug!("Removed old snapshot: {}", id);
            }
        }
    }

    /// Add audit log entry
    fn add_audit_log(
        &mut self,
        operation: impl Into<String>,
        actor: impl Into<String>,
        changes: Vec<ConfigChange>,
        success: bool,
        error: Option<String>,
    ) {
        if !self.config.enable_audit_log {
            return;
        }

        let entry = AuditLogEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            operation: operation.into(),
            actor: actor.into(),
            changes,
            success,
            error,
            metadata: HashMap::new(),
        };

        self.audit_log.push(entry);
        debug!("Added audit log entry: {} - success: {}", self.audit_log.last().unwrap().operation, success);
    }

    /// Validate all changes
    async fn validate_changes(&mut self, changes: &[ConfigChange]) -> ActuatorResult<()> {
        if !self.config.enable_validation {
            return Ok(());
        }

        for change in changes {
            // Get backend for this config type
            let config_type = format!("{:?}", change.config_type);

            // If backend exists, use it for validation
            if let Ok(backend) = self.get_backend_mut(&config_type) {
                backend.validate(&change.path, &change.new_value).await?;
            } else {
                // Fallback to basic validation
                self.basic_validation(change)?;
            }
        }

        Ok(())
    }

    /// Basic validation when no backend is available
    fn basic_validation(&self, change: &ConfigChange) -> ActuatorResult<()> {
        // Check path is not empty
        if change.path.is_empty() {
            return Err(ActuatorError::ValidationFailed(
                "Configuration path cannot be empty".to_string()
            ));
        }

        // Check for invalid characters in path
        if change.path.contains("..") {
            return Err(ActuatorError::ValidationFailed(
                format!("Invalid path contains '..': {}", change.path)
            ));
        }

        Ok(())
    }

    /// Apply changes to backends
    async fn apply_to_backends(&mut self, changes: &[ConfigChange]) -> ActuatorResult<()> {
        for change in changes {
            let config_type = format!("{:?}", change.config_type);

            if let Ok(backend) = self.get_backend_mut(&config_type) {
                if change.new_value.is_null() {
                    backend.delete(&change.path).await?;
                } else {
                    backend.set(&change.path, change.new_value.clone()).await?;
                }
            }

            // Update current config
            if change.new_value.is_null() {
                self.current_config.remove(&change.path);
            } else {
                self.current_config.insert(change.path.clone(), change.new_value.clone());
            }
        }

        Ok(())
    }

    /// Revert changes from backends
    async fn revert_from_backends(&mut self, changes: &[ConfigChange]) -> ActuatorResult<()> {
        for change in changes {
            let config_type = format!("{:?}", change.config_type);

            if let Ok(backend) = self.get_backend_mut(&config_type) {
                if let Some(old_value) = &change.old_value {
                    backend.set(&change.path, old_value.clone()).await?;
                } else {
                    backend.delete(&change.path).await?;
                }
            }

            // Update current config
            if let Some(old_value) = &change.old_value {
                self.current_config.insert(change.path.clone(), old_value.clone());
            } else {
                self.current_config.remove(&change.path);
            }
        }

        Ok(())
    }
}

#[async_trait]
impl ConfigurationManager for ConfigurationManagerImpl {
    async fn apply_config(&mut self, changes: &[ConfigChange]) -> ActuatorResult<()> {
        info!("Applying {} configuration changes (dry-run: {})", changes.len(), self.dry_run_mode);

        // Validate configuration if enabled
        if let Err(e) = self.validate_changes(changes).await {
            error!("Configuration validation failed: {}", e);
            self.add_audit_log("apply_config", "system", changes.to_vec(), false, Some(e.to_string()));
            return Err(e);
        }

        // In dry-run mode, just validate and return
        if self.dry_run_mode {
            info!("Dry-run mode: Configuration changes validated successfully");
            self.add_audit_log("apply_config_dry_run", "system", changes.to_vec(), true, None);
            return Ok(());
        }

        // Create snapshot before applying if versioning is enabled
        let snapshot_id = if self.config.enable_versioning {
            let snapshot = self.create_snapshot().await?;
            snapshot.id
        } else {
            String::new()
        };

        // Apply changes
        match self.apply_to_backends(changes).await {
            Ok(_) => {
                info!("Configuration changes applied successfully");

                // Update version history
                if self.config.enable_versioning {
                    self.current_version += 1;
                    self.history.push(ConfigurationVersion {
                        version: self.current_version,
                        timestamp: Utc::now(),
                        changes: changes.to_vec(),
                        applied_by: "system".to_string(),
                        description: format!("Applied {} changes", changes.len()),
                        snapshot_id,
                    });
                }

                self.add_audit_log("apply_config", "system", changes.to_vec(), true, None);
                Ok(())
            }
            Err(e) => {
                error!("Failed to apply configuration changes: {}", e);
                self.add_audit_log("apply_config", "system", changes.to_vec(), false, Some(e.to_string()));

                // Try to restore from snapshot if available
                if self.config.enable_versioning && !snapshot_id.is_empty() {
                    warn!("Attempting to restore from snapshot due to error");
                    if let Err(restore_err) = self.restore_snapshot(&snapshot_id).await {
                        error!("Failed to restore snapshot after error: {}", restore_err);
                    }
                }

                Err(e)
            }
        }
    }

    async fn revert_config(&mut self, changes: &[ConfigChange]) -> ActuatorResult<()> {
        info!("Reverting {} configuration changes (dry-run: {})", changes.len(), self.dry_run_mode);

        // In dry-run mode, just validate and return
        if self.dry_run_mode {
            info!("Dry-run mode: Configuration revert validated successfully");
            self.add_audit_log("revert_config_dry_run", "system", changes.to_vec(), true, None);
            return Ok(());
        }

        // Revert changes
        match self.revert_from_backends(changes).await {
            Ok(_) => {
                info!("Configuration changes reverted successfully");
                self.add_audit_log("revert_config", "system", changes.to_vec(), true, None);
                Ok(())
            }
            Err(e) => {
                error!("Failed to revert configuration changes: {}", e);
                self.add_audit_log("revert_config", "system", changes.to_vec(), false, Some(e.to_string()));
                Err(e)
            }
        }
    }

    async fn create_snapshot(&mut self) -> ActuatorResult<ConfigurationSnapshot> {
        debug!("Creating configuration snapshot");

        let snapshot = ConfigurationSnapshot {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            config: self.current_config.clone(),
            deployment_id: None,
            version: self.current_version,
            tags: vec!["auto".to_string()],
        };

        if !self.dry_run_mode {
            self.snapshots.insert(snapshot.id.clone(), snapshot.clone());
            self.cleanup_old_snapshots();
            info!("Created configuration snapshot: {}", snapshot.id);
        }

        Ok(snapshot)
    }

    async fn restore_snapshot(&mut self, snapshot_id: &str) -> ActuatorResult<()> {
        info!("Restoring configuration from snapshot: {} (dry-run: {})", snapshot_id, self.dry_run_mode);

        let snapshot = self.snapshots.get(snapshot_id)
            .ok_or_else(|| ActuatorError::ConfigError(format!(
                "Snapshot not found: {}",
                snapshot_id
            )))?
            .clone();

        // In dry-run mode, just verify snapshot exists
        if self.dry_run_mode {
            info!("Dry-run mode: Snapshot restore validated successfully");
            return Ok(());
        }

        // Calculate changes needed to restore
        let mut changes = Vec::new();

        // Add all snapshot values
        for (path, value) in &snapshot.config {
            let old_value = self.current_config.get(path).cloned();
            changes.push(ConfigChange {
                config_type: crate::decision::ConfigType::Other,
                path: path.clone(),
                old_value,
                new_value: value.clone(),
                description: format!("Restore from snapshot {}", snapshot_id),
            });
        }

        // Remove values not in snapshot
        for path in self.current_config.keys() {
            if !snapshot.config.contains_key(path) {
                changes.push(ConfigChange {
                    config_type: crate::decision::ConfigType::Other,
                    path: path.clone(),
                    old_value: self.current_config.get(path).cloned(),
                    new_value: serde_json::Value::Null,
                    description: format!("Remove during snapshot {} restore", snapshot_id),
                });
            }
        }

        // Apply the restore changes
        match self.apply_to_backends(&changes).await {
            Ok(_) => {
                info!("Successfully restored configuration from snapshot: {}", snapshot_id);
                self.add_audit_log("restore_snapshot", "system", changes, true, None);
                Ok(())
            }
            Err(e) => {
                error!("Failed to restore snapshot: {}", e);
                self.add_audit_log("restore_snapshot", "system", changes, false, Some(e.to_string()));
                Err(e)
            }
        }
    }

    async fn get_config(&self) -> ActuatorResult<HashMap<String, serde_json::Value>> {
        Ok(self.current_config.clone())
    }

    async fn validate_config(&self, changes: &[ConfigChange]) -> ActuatorResult<()> {
        debug!("Validating {} configuration changes", changes.len());

        // Clone self to avoid mutable borrow issues
        let mut temp_manager = ConfigurationManagerImpl {
            config: self.config.clone(),
            current_config: self.current_config.clone(),
            snapshots: HashMap::new(),
            history: Vec::new(),
            current_version: self.current_version,
            audit_log: Vec::new(),
            dry_run_mode: false,
            backends: HashMap::new(),
        };

        // Register backends (note: we can't clone trait objects, so validation may be limited)
        temp_manager.validate_changes(changes).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decision::ConfigType;

    fn create_test_manager() -> ConfigurationManagerImpl {
        let config = ConfigurationManagementConfig {
            enable_versioning: true,
            enable_validation: true,
            enable_audit_log: true,
            snapshot_interval: std::time::Duration::from_secs(3600),
            max_snapshots: 10,
            enable_dry_run: false,
        };

        let mut manager = ConfigurationManagerImpl::new(config);

        // Register in-memory backend for testing
        manager.register_backend("Model", Box::new(InMemoryBackend::new()));
        manager.register_backend("Cache", Box::new(InMemoryBackend::new()));
        manager.register_backend("RateLimit", Box::new(InMemoryBackend::new()));

        manager
    }

    fn create_test_change(path: &str, value: serde_json::Value) -> ConfigChange {
        ConfigChange {
            config_type: ConfigType::Model,
            path: path.to_string(),
            old_value: None,
            new_value: value,
            description: format!("Test change for {}", path),
        }
    }

    #[tokio::test]
    async fn test_apply_config_basic() {
        let mut manager = create_test_manager();

        let changes = vec![
            create_test_change("model.name", serde_json::json!("gpt-4")),
            create_test_change("model.temperature", serde_json::json!(0.7)),
        ];

        let result = manager.apply_config(&changes).await;
        assert!(result.is_ok(), "Failed to apply config: {:?}", result);

        let config = manager.get_config().await.unwrap();
        assert_eq!(config.get("model.name"), Some(&serde_json::json!("gpt-4")));
        assert_eq!(config.get("model.temperature"), Some(&serde_json::json!(0.7)));
    }

    #[tokio::test]
    async fn test_revert_config() {
        let mut manager = create_test_manager();

        // Apply initial config
        let changes = vec![
            create_test_change("model.name", serde_json::json!("gpt-4")),
        ];
        manager.apply_config(&changes).await.unwrap();

        // Create revert changes
        let revert_changes = vec![
            ConfigChange {
                config_type: ConfigType::Model,
                path: "model.name".to_string(),
                old_value: Some(serde_json::json!("gpt-4")),
                new_value: serde_json::json!("gpt-3.5-turbo"),
                description: "Revert to previous model".to_string(),
            },
        ];

        let result = manager.revert_config(&revert_changes).await;
        assert!(result.is_ok(), "Failed to revert config: {:?}", result);

        let config = manager.get_config().await.unwrap();
        assert_eq!(config.get("model.name"), Some(&serde_json::json!("gpt-3.5-turbo")));
    }

    #[tokio::test]
    async fn test_create_snapshot() {
        let mut manager = create_test_manager();

        // Apply some config
        let changes = vec![
            create_test_change("model.name", serde_json::json!("gpt-4")),
            create_test_change("cache.enabled", serde_json::json!(true)),
        ];
        manager.apply_config(&changes).await.unwrap();

        // Create snapshot
        let snapshot = manager.create_snapshot().await.unwrap();
        assert!(!snapshot.id.is_empty());
        assert_eq!(snapshot.config.len(), 2);
        assert_eq!(snapshot.version, 1);
    }

    #[tokio::test]
    async fn test_restore_snapshot() {
        let mut manager = create_test_manager();

        // Apply initial config
        let changes1 = vec![
            create_test_change("model.name", serde_json::json!("gpt-4")),
        ];
        manager.apply_config(&changes1).await.unwrap();

        // Create snapshot
        let snapshot = manager.create_snapshot().await.unwrap();
        let snapshot_id = snapshot.id.clone();

        // Apply more changes
        let changes2 = vec![
            create_test_change("model.name", serde_json::json!("gpt-3.5-turbo")),
            create_test_change("model.temperature", serde_json::json!(0.9)),
        ];
        manager.apply_config(&changes2).await.unwrap();

        // Restore snapshot
        manager.restore_snapshot(&snapshot_id).await.unwrap();

        let config = manager.get_config().await.unwrap();
        assert_eq!(config.get("model.name"), Some(&serde_json::json!("gpt-4")));
        assert!(!config.contains_key("model.temperature"));
    }

    #[tokio::test]
    async fn test_configuration_versioning() {
        let mut manager = create_test_manager();

        // Apply multiple changes
        let changes1 = vec![create_test_change("model.name", serde_json::json!("gpt-4"))];
        manager.apply_config(&changes1).await.unwrap();

        let changes2 = vec![create_test_change("cache.enabled", serde_json::json!(true))];
        manager.apply_config(&changes2).await.unwrap();

        let changes3 = vec![create_test_change("rate_limit.max_rps", serde_json::json!(100))];
        manager.apply_config(&changes3).await.unwrap();

        // Check version
        assert_eq!(manager.current_version, 3);

        // Check history
        let history = manager.get_history();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].version, 1);
        assert_eq!(history[1].version, 2);
        assert_eq!(history[2].version, 3);
    }

    #[tokio::test]
    async fn test_validation_failure() {
        let mut manager = create_test_manager();

        let changes = vec![
            ConfigChange {
                config_type: ConfigType::Model,
                path: "".to_string(), // Invalid empty path
                old_value: None,
                new_value: serde_json::json!("value"),
                description: "Invalid change".to_string(),
            },
        ];

        let result = manager.apply_config(&changes).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_dry_run_mode() {
        let mut manager = create_test_manager();
        manager.set_dry_run(true);

        let changes = vec![
            create_test_change("model.name", serde_json::json!("gpt-4")),
        ];

        let result = manager.apply_config(&changes).await;
        assert!(result.is_ok());

        // Config should not be applied in dry-run mode
        let config = manager.get_config().await.unwrap();
        assert!(!config.contains_key("model.name"));

        // But validation should have occurred
        let audit_log = manager.get_audit_log();
        assert!(!audit_log.is_empty());
        assert_eq!(audit_log[0].operation, "apply_config_dry_run");
    }

    #[tokio::test]
    async fn test_audit_logging() {
        let mut manager = create_test_manager();

        let changes = vec![
            create_test_change("model.name", serde_json::json!("gpt-4")),
        ];

        manager.apply_config(&changes).await.unwrap();

        let audit_log = manager.get_audit_log();
        assert_eq!(audit_log.len(), 1);
        assert_eq!(audit_log[0].operation, "apply_config");
        assert!(audit_log[0].success);
        assert_eq!(audit_log[0].changes.len(), 1);
    }

    #[tokio::test]
    async fn test_configuration_diff() {
        let manager = create_test_manager();

        let changes = vec![
            create_test_change("model.name", serde_json::json!("gpt-4")),
            ConfigChange {
                config_type: ConfigType::Model,
                path: "model.temperature".to_string(),
                old_value: Some(serde_json::json!(0.5)),
                new_value: serde_json::json!(0.7),
                description: "Update temperature".to_string(),
            },
        ];

        let diffs = manager.calculate_diff(&changes);
        assert_eq!(diffs.len(), 2);
        assert_eq!(diffs[0].change_type, DiffType::Add);
        assert_eq!(diffs[1].change_type, DiffType::Update);
    }

    #[tokio::test]
    async fn test_snapshot_cleanup() {
        let mut manager = create_test_manager();

        // Create more snapshots than max_snapshots
        for i in 0..15 {
            let changes = vec![
                create_test_change(&format!("key_{}", i), serde_json::json!(i)),
            ];
            manager.apply_config(&changes).await.unwrap();
        }

        // Should only keep max_snapshots (10)
        assert!(manager.snapshots.len() <= manager.config.max_snapshots);
    }

    #[tokio::test]
    async fn test_list_snapshots() {
        let mut manager = create_test_manager();

        // Create multiple snapshots
        for i in 0..3 {
            let changes = vec![
                create_test_change(&format!("key_{}", i), serde_json::json!(i)),
            ];
            manager.apply_config(&changes).await.unwrap();
        }

        let snapshots = manager.list_snapshots();
        assert_eq!(snapshots.len(), 3);

        // Should be sorted by timestamp (newest first)
        for i in 0..snapshots.len() - 1 {
            assert!(snapshots[i].timestamp >= snapshots[i + 1].timestamp);
        }
    }

    #[tokio::test]
    async fn test_validate_config_only() {
        let manager = create_test_manager();

        let valid_changes = vec![
            create_test_change("model.name", serde_json::json!("gpt-4")),
        ];

        let result = manager.validate_config(&valid_changes).await;
        assert!(result.is_ok());

        let invalid_changes = vec![
            ConfigChange {
                config_type: ConfigType::Model,
                path: "".to_string(),
                old_value: None,
                new_value: serde_json::json!("value"),
                description: "Invalid".to_string(),
            },
        ];

        let result = manager.validate_config(&invalid_changes).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_snapshot() {
        let mut manager = create_test_manager();

        let changes = vec![
            create_test_change("model.name", serde_json::json!("gpt-4")),
        ];
        manager.apply_config(&changes).await.unwrap();

        let snapshot = manager.create_snapshot().await.unwrap();
        let snapshot_id = snapshot.id.clone();

        let retrieved = manager.get_snapshot(&snapshot_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, snapshot_id);

        let not_found = manager.get_snapshot("nonexistent");
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_backend_not_found() {
        let mut manager = create_test_manager();

        // Remove all backends
        manager.backends.clear();

        let changes = vec![
            create_test_change("model.name", serde_json::json!("gpt-4")),
        ];

        // Should still work with basic validation
        let result = manager.apply_config(&changes).await;
        // This will fail because no backend is registered
        assert!(result.is_err());
    }
}
