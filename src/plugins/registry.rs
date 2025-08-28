//! Plugin registry for managing loaded plugins
//! 
//! This module provides the plugin registry that tracks all loaded plugins
//! and provides efficient access based on capabilities and priorities.

use super::{B2BUAPlugin, PluginCapability, PluginInfo, PluginMetadata};
use anyhow::{Context, Result};
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Plugin registry for managing loaded plugins
pub struct PluginRegistry {
    /// Map of plugin name to plugin instance
    plugins: HashMap<String, Box<dyn B2BUAPlugin>>,
    /// Map of plugin name to plugin information
    plugin_info: HashMap<String, PluginInfo>,
    /// Index of plugins by capability
    capability_index: HashMap<PluginCapability, Vec<String>>,
    /// Plugin execution order by priority
    execution_order: Vec<String>,
}

impl PluginRegistry {
    /// Create a new plugin registry
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            plugin_info: HashMap::new(),
            capability_index: HashMap::new(),
            execution_order: Vec::new(),
        }
    }

    /// Register a plugin in the registry
    pub async fn register_plugin(&mut self, name: String, plugin: Box<dyn B2BUAPlugin>) -> Result<()> {
        // Check if plugin already exists
        if self.plugins.contains_key(&name) {
            return Err(anyhow::anyhow!("Plugin '{}' is already registered", name));
        }

        let metadata = plugin.metadata().clone();
        
        // Validate plugin dependencies
        self.validate_dependencies(&metadata).await?;

        // Create plugin info
        let plugin_info = PluginInfo {
            metadata: metadata.clone(),
            config: super::PluginConfig {
                name: name.clone(),
                enabled: true,
                priority: 100, // Default priority
                config: serde_json::Value::Null,
                plugin_path: None,
            },
            loaded_at: chrono::Utc::now(),
            last_error: None,
            invocation_count: 0,
            average_processing_time_ms: 0.0,
        };

        // Update capability index
        for capability in &metadata.capabilities {
            self.capability_index
                .entry(capability.clone())
                .or_insert_with(Vec::new)
                .push(name.clone());
        }

        // Insert plugin and info
        self.plugins.insert(name.clone(), plugin);
        self.plugin_info.insert(name.clone(), plugin_info);

        // Update execution order based on priorities
        self.update_execution_order();

        info!("Registered plugin: {} v{}", name, metadata.version);
        Ok(())
    }

    /// Unregister a plugin from the registry
    pub async fn unregister_plugin(&mut self, name: &str) -> Result<()> {
        // Remove from plugins map
        let plugin = self.plugins.remove(name)
            .ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found", name))?;

        // Shutdown the plugin
        let mut plugin_mut = plugin;
        if let Err(e) = plugin_mut.shutdown().await {
            warn!("Error during plugin '{}' shutdown: {}", name, e);
        }

        // Remove plugin info
        let plugin_info = self.plugin_info.remove(name)
            .ok_or_else(|| anyhow::anyhow!("Plugin info for '{}' not found", name))?;

        // Remove from capability index
        for capability in &plugin_info.metadata.capabilities {
            if let Some(plugins) = self.capability_index.get_mut(capability) {
                plugins.retain(|p| p != name);
                if plugins.is_empty() {
                    self.capability_index.remove(capability);
                }
            }
        }

        // Update execution order
        self.execution_order.retain(|p| p != name);

        info!("Unregistered plugin: {}", name);
        Ok(())
    }

    /// Get a plugin by name
    pub async fn get_plugin(&self, name: &str) -> Option<&dyn B2BUAPlugin> {
        self.plugins.get(name).map(|p| p.as_ref())
    }

    /// Get plugin information
    pub async fn get_plugin_info(&self, name: &str) -> Option<&PluginInfo> {
        self.plugin_info.get(name)
    }

    /// List all registered plugin names
    pub async fn list_plugins(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }

    /// Get plugins by capability
    pub async fn get_plugins_by_capability(&self, capability: &PluginCapability) -> Vec<String> {
        self.capability_index
            .get(capability)
            .cloned()
            .unwrap_or_default()
    }

    /// Get plugins in execution order (sorted by priority)
    pub async fn get_plugins_in_execution_order(&self) -> &[String] {
        &self.execution_order
    }

    /// Get plugins that support all specified capabilities
    pub async fn get_plugins_with_all_capabilities(&self, capabilities: &[PluginCapability]) -> Vec<String> {
        let mut result = Vec::new();
        
        for (name, info) in &self.plugin_info {
            if capabilities.iter().all(|cap| info.metadata.capabilities.contains(cap)) {
                result.push(name.clone());
            }
        }
        
        // Sort by priority
        result.sort_by(|a, b| {
            let priority_a = self.plugin_info.get(a).map(|info| info.config.priority).unwrap_or(999);
            let priority_b = self.plugin_info.get(b).map(|info| info.config.priority).unwrap_or(999);
            priority_a.cmp(&priority_b)
        });
        
        result
    }

    /// Check if a capability is supported by any plugin
    pub async fn is_capability_supported(&self, capability: &PluginCapability) -> bool {
        self.capability_index.contains_key(capability)
    }

    /// Get all supported capabilities
    pub async fn get_supported_capabilities(&self) -> Vec<PluginCapability> {
        self.capability_index.keys().cloned().collect()
    }

    /// Update plugin priority
    pub async fn update_plugin_priority(&mut self, name: &str, priority: i32) -> Result<()> {
        let plugin_info = self.plugin_info.get_mut(name)
            .ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found", name))?;

        plugin_info.config.priority = priority;
        self.update_execution_order();

        debug!("Updated priority for plugin '{}' to {}", name, priority);
        Ok(())
    }

    /// Enable or disable a plugin
    pub async fn set_plugin_enabled(&mut self, name: &str, enabled: bool) -> Result<()> {
        let plugin_info = self.plugin_info.get_mut(name)
            .ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found", name))?;

        plugin_info.config.enabled = enabled;

        if enabled {
            info!("Enabled plugin: {}", name);
        } else {
            info!("Disabled plugin: {}", name);
        }

        Ok(())
    }

    /// Check if a plugin is enabled
    pub async fn is_plugin_enabled(&self, name: &str) -> bool {
        self.plugin_info
            .get(name)
            .map(|info| info.config.enabled)
            .unwrap_or(false)
    }

    /// Get plugin count
    pub async fn get_plugin_count(&self) -> usize {
        self.plugins.len()
    }

    /// Get enabled plugin count
    pub async fn get_enabled_plugin_count(&self) -> usize {
        self.plugin_info
            .values()
            .filter(|info| info.config.enabled)
            .count()
    }

    /// Validate plugin dependencies
    async fn validate_dependencies(&self, metadata: &PluginMetadata) -> Result<()> {
        for dependency in &metadata.dependencies {
            if !self.plugins.contains_key(dependency) {
                return Err(anyhow::anyhow!(
                    "Plugin '{}' depends on '{}' which is not loaded",
                    metadata.name,
                    dependency
                ));
            }
        }
        Ok(())
    }

    /// Update execution order based on plugin priorities
    fn update_execution_order(&mut self) {
        let mut plugins_with_priority: Vec<(String, i32)> = self.plugin_info
            .iter()
            .filter(|(_, info)| info.config.enabled)
            .map(|(name, info)| (name.clone(), info.config.priority))
            .collect();

        // Sort by priority (lower number = higher priority)
        plugins_with_priority.sort_by(|a, b| a.1.cmp(&b.1));

        // Extract just the names
        self.execution_order = plugins_with_priority
            .into_iter()
            .map(|(name, _)| name)
            .collect();

        debug!("Updated plugin execution order: {:?}", self.execution_order);
    }

    /// Find plugins with conflicting capabilities
    pub async fn find_capability_conflicts(&self) -> HashMap<PluginCapability, Vec<String>> {
        let mut conflicts = HashMap::new();
        
        for (capability, plugin_names) in &self.capability_index {
            if plugin_names.len() > 1 {
                // Multiple plugins support the same capability - potential conflict
                let enabled_plugins: Vec<String> = plugin_names
                    .iter()
                    .filter(|name| {
                        self.plugin_info
                            .get(*name)
                            .map(|info| info.config.enabled)
                            .unwrap_or(false)
                    })
                    .cloned()
                    .collect();

                if enabled_plugins.len() > 1 {
                    conflicts.insert(capability.clone(), enabled_plugins);
                }
            }
        }
        
        conflicts
    }

    /// Get plugin load order respecting dependencies
    pub async fn get_dependency_sorted_plugins(&self) -> Result<Vec<String>> {
        let mut result = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut visiting = std::collections::HashSet::new();

        for plugin_name in self.plugins.keys() {
            self.dependency_dfs(plugin_name, &mut result, &mut visited, &mut visiting)
                .with_context(|| format!("Failed to resolve dependencies for plugin '{}'", plugin_name))?;
        }

        Ok(result)
    }

    /// Depth-first search for dependency resolution
    fn dependency_dfs(
        &self,
        plugin_name: &str,
        result: &mut Vec<String>,
        visited: &mut std::collections::HashSet<String>,
        visiting: &mut std::collections::HashSet<String>,
    ) -> Result<()> {
        if visited.contains(plugin_name) {
            return Ok(());
        }

        if visiting.contains(plugin_name) {
            return Err(anyhow::anyhow!("Circular dependency detected involving plugin '{}'", plugin_name));
        }

        visiting.insert(plugin_name.to_string());

        // Visit dependencies first
        if let Some(info) = self.plugin_info.get(plugin_name) {
            for dependency in &info.metadata.dependencies {
                self.dependency_dfs(dependency, result, visited, visiting)?;
            }
        }

        visiting.remove(plugin_name);
        visited.insert(plugin_name.to_string());
        result.push(plugin_name.to_string());

        Ok(())
    }

    /// Shutdown all plugins in reverse dependency order
    pub async fn shutdown_all(&mut self) -> Result<()> {
        let shutdown_order = self.get_dependency_sorted_plugins().await?;
        
        // Shutdown in reverse order
        for plugin_name in shutdown_order.iter().rev() {
            if let Some(mut plugin) = self.plugins.remove(plugin_name) {
                if let Err(e) = plugin.shutdown().await {
                    warn!("Error shutting down plugin '{}': {}", plugin_name, e);
                } else {
                    debug!("Successfully shut down plugin: {}", plugin_name);
                }
            }
        }

        self.plugin_info.clear();
        self.capability_index.clear();
        self.execution_order.clear();

        info!("All plugins have been shut down");
        Ok(())
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::{PluginCapability, PluginMetadata};
    use crate::services::signaling::{CallSession, PluginResponse, SipMessage};

    // Mock plugin for testing
    struct MockPlugin {
        metadata: PluginMetadata,
    }

    impl MockPlugin {
        fn new(name: &str, capabilities: Vec<PluginCapability>) -> Self {
            Self {
                metadata: PluginMetadata {
                    name: name.to_string(),
                    version: "1.0.0".to_string(),
                    description: "Mock plugin for testing".to_string(),
                    author: "Test".to_string(),
                    license: "MIT".to_string(),
                    min_system_version: "1.0.0".to_string(),
                    dependencies: vec![],
                    config_schema: None,
                    capabilities,
                },
            }
        }
    }

    #[async_trait::async_trait]
    impl B2BUAPlugin for MockPlugin {
        fn metadata(&self) -> &PluginMetadata {
            &self.metadata
        }

        async fn initialize(&mut self, _config: &super::super::PluginConfig, _context: &super::super::PluginContext) -> Result<()> {
            Ok(())
        }

        async fn handle_message(&self, message: &SipMessage, _context: &super::super::PluginContext) -> Result<PluginResponse> {
            Ok(PluginResponse::Forward(message.clone()))
        }
    }

    #[tokio::test]
    async fn test_plugin_registry_creation() {
        let _registry = PluginRegistry::new();
    }

    #[tokio::test]
    async fn test_plugin_registration() {
        let mut registry = PluginRegistry::new();
        
        let plugin = Box::new(MockPlugin::new("test-plugin", vec![PluginCapability::SipInvite]));
        let result = registry.register_plugin("test-plugin".to_string(), plugin).await;
        
        assert!(result.is_ok());
        assert_eq!(registry.get_plugin_count().await, 1);
        assert!(registry.get_plugin("test-plugin").await.is_some());
    }

    #[tokio::test]
    async fn test_capability_indexing() {
        let mut registry = PluginRegistry::new();
        
        let plugin = Box::new(MockPlugin::new("test-plugin", vec![PluginCapability::SipInvite, PluginCapability::SipResponse]));
        registry.register_plugin("test-plugin".to_string(), plugin).await
            .expect("Plugin registration should succeed");
        
        let sip_invite_plugins = registry.get_plugins_by_capability(&PluginCapability::SipInvite).await;
        assert_eq!(sip_invite_plugins.len(), 1);
        assert_eq!(sip_invite_plugins[0], "test-plugin");
        
        let sip_response_plugins = registry.get_plugins_by_capability(&PluginCapability::SipResponse).await;
        assert_eq!(sip_response_plugins.len(), 1);
        assert_eq!(sip_response_plugins[0], "test-plugin");
        
        assert!(registry.is_capability_supported(&PluginCapability::SipInvite).await);
        assert!(!registry.is_capability_supported(&PluginCapability::SdpModification).await);
    }

    #[tokio::test]
    async fn test_plugin_unregistration() {
        let mut registry = PluginRegistry::new();
        
        let plugin = Box::new(MockPlugin::new("test-plugin", vec![PluginCapability::SipInvite]));
        registry.register_plugin("test-plugin".to_string(), plugin).await
            .expect("Plugin registration should succeed");
        
        assert_eq!(registry.get_plugin_count().await, 1);
        
        let result = registry.unregister_plugin("test-plugin").await;
        assert!(result.is_ok());
        
        assert_eq!(registry.get_plugin_count().await, 0);
        assert!(registry.get_plugin("test-plugin").await.is_none());
        assert!(!registry.is_capability_supported(&PluginCapability::SipInvite).await);
    }
}