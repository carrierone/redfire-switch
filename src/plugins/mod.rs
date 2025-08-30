//! Plugin architecture for B2BUA implementations
//! 
//! This module provides a flexible plugin system that allows dynamic loading
//! and configuration of B2BUA functionality without requiring code changes.

use crate::events::{EventBus, TelecomEvent};
use crate::services::signaling::{SipMessage, CallSession, PluginResponse};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

pub mod registry;
pub mod loader;
pub mod examples;

pub use registry::*;
pub use loader::*;
pub use examples::*;

/// Plugin metadata and configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Plugin name (must be unique)
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Plugin description
    pub description: String,
    /// Plugin author
    pub author: String,
    /// Plugin license
    pub license: String,
    /// Minimum system version required
    pub min_system_version: String,
    /// Plugin dependencies
    pub dependencies: Vec<String>,
    /// Plugin configuration schema
    pub config_schema: Option<serde_json::Value>,
    /// Plugin capabilities/features
    pub capabilities: Vec<PluginCapability>,
}

/// Plugin capabilities
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PluginCapability {
    /// Can process SIP INVITE messages
    SipInvite,
    /// Can process SIP responses
    SipResponse,
    /// Can modify SDP content
    SdpModification,
    /// Can perform call routing decisions
    CallRouting,
    /// Can handle media negotiations
    MediaNegotiation,
    /// Can perform security checks
    SecurityValidation,
    /// Can generate CDRs
    CdrGeneration,
    /// Can handle DTMF events
    DtmfHandling,
    /// Custom capability
    Custom(String),
}

/// Plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Plugin name
    pub name: String,
    /// Whether plugin is enabled
    pub enabled: bool,
    /// Plugin priority (lower number = higher priority)
    pub priority: i32,
    /// Plugin-specific configuration
    pub config: serde_json::Value,
    /// Plugin file path
    pub plugin_path: Option<PathBuf>,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            enabled: true,
            priority: 100,
            config: serde_json::Value::Null,
            plugin_path: None,
        }
    }
}

/// Plugin loading information
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub metadata: PluginMetadata,
    pub config: PluginConfig,
    pub loaded_at: chrono::DateTime<chrono::Utc>,
    pub last_error: Option<String>,
    pub invocation_count: u64,
    pub average_processing_time_ms: f64,
}

/// Plugin execution context
#[derive(Clone)]
pub struct PluginContext {
    /// Current call session (if available)
    pub call_session: Option<CallSession>,
    /// Plugin-specific data storage
    pub plugin_data: HashMap<String, serde_json::Value>,
    /// Event bus reference for publishing events
    pub event_bus: Arc<EventBus>,
    /// System configuration
    pub system_config: HashMap<String, serde_json::Value>,
}

/// Enhanced B2BUA plugin trait with advanced features
#[async_trait::async_trait]
pub trait B2BUAPlugin: Send + Sync {
    /// Get plugin metadata
    fn metadata(&self) -> &PluginMetadata;

    /// Initialize the plugin with configuration
    async fn initialize(&mut self, config: &PluginConfig, context: &PluginContext) -> Result<()>;

    /// Handle incoming SIP message with full context
    async fn handle_message(&self, message: &SipMessage, context: &PluginContext) -> Result<PluginResponse>;

    /// Plugin health check
    async fn health_check(&self) -> Result<()> {
        Ok(())
    }

    /// Handle plugin-specific events
    async fn handle_event(&self, event: &TelecomEvent, context: &PluginContext) -> Result<()> {
        let _ = (event, context);
        Ok(())
    }

    /// Get plugin statistics
    async fn get_statistics(&self) -> Result<HashMap<String, serde_json::Value>> {
        Ok(HashMap::new())
    }

    /// Plugin configuration update notification
    async fn on_config_updated(&mut self, config: &PluginConfig, context: &PluginContext) -> Result<()> {
        let _ = (config, context);
        Ok(())
    }

    /// Plugin shutdown cleanup
    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }

    /// Check if plugin can handle a specific capability
    fn supports_capability(&self, capability: &PluginCapability) -> bool {
        self.metadata().capabilities.contains(capability)
    }
}

/// Plugin execution result with performance metrics
#[derive(Debug, Clone)]
pub struct PluginExecutionResult {
    pub plugin_name: String,
    pub response: PluginResponse,
    pub execution_time_ms: u64,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Plugin manager for coordinating all plugins
pub struct PluginManager {
    /// Plugin registry
    registry: Arc<RwLock<PluginRegistry>>,
    /// Plugin loader
    loader: PluginLoader,
    /// Event bus for plugin communication
    event_bus: Arc<EventBus>,
    /// Plugin execution statistics
    stats: Arc<RwLock<HashMap<String, PluginStats>>>,
}

/// Plugin execution statistics
#[derive(Debug, Clone, Default)]
pub struct PluginStats {
    pub total_invocations: u64,
    pub successful_invocations: u64,
    pub failed_invocations: u64,
    pub average_execution_time_ms: f64,
    pub last_execution: Option<chrono::DateTime<chrono::Utc>>,
    pub last_error: Option<String>,
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            registry: Arc::new(RwLock::new(PluginRegistry::new())),
            loader: PluginLoader::new(),
            event_bus,
            stats: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Load plugins from a configuration file
    pub async fn load_plugins_from_config(&self, config_path: &Path) -> Result<()> {
        let config_content = tokio::fs::read_to_string(config_path).await
            .with_context(|| format!("Failed to read plugin config from {:?}", config_path))?;

        let plugin_configs: Vec<PluginConfig> = serde_json::from_str(&config_content)
            .context("Failed to parse plugin configuration")?;

        for plugin_config in plugin_configs {
            if plugin_config.enabled {
                self.load_plugin(plugin_config).await?;
            }
        }

        info!("Loaded {} plugins from configuration", self.get_plugin_count().await);
        Ok(())
    }

    /// Load a single plugin
    pub async fn load_plugin(&self, config: PluginConfig) -> Result<()> {
        let plugin = if let Some(plugin_path) = &config.plugin_path {
            // Load external plugin from file
            self.loader.load_from_file(plugin_path, &config).await?
        } else {
            // Load built-in plugin
            self.loader.load_builtin(&config.name, &config).await?
        };

        // Initialize plugin
        let context = self.create_plugin_context(None).await;
        let mut plugin_mut = plugin;
        plugin_mut.initialize(&config, &context).await?;

        // Register plugin
        let mut registry = self.registry.write().await;
        registry.register_plugin(config.name.clone(), plugin_mut).await?;

        // Initialize statistics
        let mut stats = self.stats.write().await;
        stats.insert(config.name.clone(), PluginStats::default());

        info!("Successfully loaded and registered plugin: {}", config.name);
        Ok(())
    }

    /// Unload a plugin
    pub async fn unload_plugin(&self, plugin_name: &str) -> Result<()> {
        let mut registry = self.registry.write().await;
        registry.unregister_plugin(plugin_name).await?;

        let mut stats = self.stats.write().await;
        stats.remove(plugin_name);

        info!("Successfully unloaded plugin: {}", plugin_name);
        Ok(())
    }

    /// Process SIP message through all applicable plugins
    pub async fn process_message(&self, message: &SipMessage, call_session: Option<&CallSession>) -> Result<Vec<PluginExecutionResult>> {
        let registry = self.registry.read().await;
        let plugins = registry.get_plugins_by_capability(&PluginCapability::SipInvite).await;
        let mut results = Vec::new();

        let context = self.create_plugin_context(call_session.cloned()).await;

        for plugin_name in plugins {
            if let Some(plugin) = registry.get_plugin(&plugin_name).await {
                let start_time = std::time::Instant::now();
                
                let result = match plugin.handle_message(message, &context).await {
                    Ok(response) => {
                        let execution_time = start_time.elapsed().as_millis() as u64;
                        
                        // Update statistics
                        self.update_plugin_stats(&plugin_name, true, execution_time, None).await;
                        
                        PluginExecutionResult {
                            plugin_name: plugin_name.clone(),
                            response,
                            execution_time_ms: execution_time,
                            success: true,
                            error_message: None,
                        }
                    }
                    Err(e) => {
                        let execution_time = start_time.elapsed().as_millis() as u64;
                        let error_msg = e.to_string();
                        
                        // Update statistics
                        self.update_plugin_stats(&plugin_name, false, execution_time, Some(error_msg.clone())).await;
                        
                        warn!("Plugin {} failed to process message: {}", plugin_name, error_msg);
                        
                        PluginExecutionResult {
                            plugin_name: plugin_name.clone(),
                            response: PluginResponse::Forward(message.clone()),
                            execution_time_ms: execution_time,
                            success: false,
                            error_message: Some(error_msg),
                        }
                    }
                };
                
                results.push(result);
            }
        }

        Ok(results)
    }

    /// Broadcast event to all plugins that can handle events
    pub async fn broadcast_event(&self, event: &TelecomEvent) -> Result<()> {
        let registry = self.registry.read().await;
        let all_plugins = registry.list_plugins().await;
        let context = self.create_plugin_context(None).await;

        for plugin_name in all_plugins {
            if let Some(plugin) = registry.get_plugin(&plugin_name).await {
                if let Err(e) = plugin.handle_event(event, &context).await {
                    warn!("Plugin {} failed to handle event: {}", plugin_name, e);
                }
            }
        }

        Ok(())
    }

    /// Get plugin statistics
    pub async fn get_plugin_stats(&self, plugin_name: &str) -> Option<PluginStats> {
        let stats = self.stats.read().await;
        stats.get(plugin_name).cloned()
    }

    /// Get all plugin statistics
    pub async fn get_all_stats(&self) -> HashMap<String, PluginStats> {
        let stats = self.stats.read().await;
        stats.clone()
    }

    /// Get number of loaded plugins
    pub async fn get_plugin_count(&self) -> usize {
        let registry = self.registry.read().await;
        registry.list_plugins().await.len()
    }

    /// Perform health checks on all plugins
    pub async fn health_check_all(&self) -> Result<HashMap<String, bool>> {
        let registry = self.registry.read().await;
        let all_plugins = registry.list_plugins().await;
        let mut results = HashMap::new();

        for plugin_name in all_plugins {
            if let Some(plugin) = registry.get_plugin(&plugin_name).await {
                let is_healthy = plugin.health_check().await.is_ok();
                results.insert(plugin_name, is_healthy);
            }
        }

        Ok(results)
    }

    /// Shutdown all plugins
    pub async fn shutdown_all(&self) -> Result<()> {
        let mut registry = self.registry.write().await;
        registry.shutdown_all().await?;
        info!("All plugins shutdown successfully");
        Ok(())
    }

    /// Create plugin execution context
    async fn create_plugin_context(&self, call_session: Option<CallSession>) -> PluginContext {
        PluginContext {
            call_session,
            plugin_data: HashMap::new(),
            event_bus: self.event_bus.clone(),
            system_config: HashMap::new(), // TODO: Get from control service
        }
    }

    /// Update plugin execution statistics
    async fn update_plugin_stats(&self, plugin_name: &str, success: bool, execution_time_ms: u64, error: Option<String>) {
        let mut stats = self.stats.write().await;
        
        let plugin_stats = stats.entry(plugin_name.to_string()).or_default();
        plugin_stats.total_invocations += 1;
        
        if success {
            plugin_stats.successful_invocations += 1;
        } else {
            plugin_stats.failed_invocations += 1;
            plugin_stats.last_error = error;
        }

        // Update rolling average execution time
        let total = plugin_stats.total_invocations as f64;
        plugin_stats.average_execution_time_ms = 
            (plugin_stats.average_execution_time_ms * (total - 1.0) + execution_time_ms as f64) / total;
        
        plugin_stats.last_execution = Some(chrono::Utc::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventBus;

    #[tokio::test]
    async fn test_plugin_manager_creation() {
        let event_bus = Arc::new(EventBus::new());
        let _manager = PluginManager::new(event_bus);
    }

    #[tokio::test]
    async fn test_plugin_stats_update() {
        let event_bus = Arc::new(EventBus::new());
        let manager = PluginManager::new(event_bus);

        manager.update_plugin_stats("test-plugin", true, 100, None).await;
        
        let stats = manager.get_plugin_stats("test-plugin").await;
        assert!(stats.is_some());
        
        let stats = stats.expect("Plugin stats should be available");
        assert_eq!(stats.total_invocations, 1);
        assert_eq!(stats.successful_invocations, 1);
        assert_eq!(stats.failed_invocations, 0);
        assert_eq!(stats.average_execution_time_ms, 100.0);
    }
}