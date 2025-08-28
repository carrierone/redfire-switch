//! Plugin loader for dynamic loading of B2BUA plugins
//! 
//! This module handles the loading of plugins from various sources including
//! built-in plugins and external plugin files.

use super::{B2BUAPlugin, PluginCapability, PluginConfig, PluginMetadata, examples::*};
use anyhow::{Context, Result};
use std::path::Path;
use tracing::{debug, info, warn};

/// Plugin loader for managing plugin loading operations
pub struct PluginLoader {
    /// Registry of built-in plugins
    builtin_plugins: std::collections::HashMap<String, fn() -> Box<dyn B2BUAPlugin>>,
}

impl PluginLoader {
    /// Create a new plugin loader
    pub fn new() -> Self {
        let mut loader = Self {
            builtin_plugins: std::collections::HashMap::new(),
        };
        
        // Register built-in plugins
        loader.register_builtin_plugins();
        loader
    }

    /// Load a plugin from a file path
    pub async fn load_from_file(&self, _plugin_path: &Path, config: &PluginConfig) -> Result<Box<dyn B2BUAPlugin>> {
        // TODO: In a full implementation, this would load shared libraries (.so/.dll/.dylib)
        // using libloading or similar crate. For now, we'll fall back to built-in plugins
        // or return an error if the requested plugin isn't built-in.
        
        warn!("Dynamic plugin loading from files is not yet implemented");
        warn!("Attempting to load as built-in plugin: {}", config.name);
        
        self.load_builtin(&config.name, config).await
    }

    /// Load a built-in plugin by name
    pub async fn load_builtin(&self, plugin_name: &str, _config: &PluginConfig) -> Result<Box<dyn B2BUAPlugin>> {
        let plugin_constructor = self.builtin_plugins.get(plugin_name)
            .ok_or_else(|| anyhow::anyhow!("Built-in plugin '{}' not found", plugin_name))?;

        let plugin = plugin_constructor();
        
        info!("Loaded built-in plugin: {}", plugin_name);
        debug!("Plugin metadata: {:?}", plugin.metadata());
        
        Ok(plugin)
    }

    /// Get list of available built-in plugins
    pub fn list_builtin_plugins(&self) -> Vec<String> {
        self.builtin_plugins.keys().cloned().collect()
    }

    /// Check if a built-in plugin exists
    pub fn has_builtin_plugin(&self, plugin_name: &str) -> bool {
        self.builtin_plugins.contains_key(plugin_name)
    }

    /// Register built-in plugins
    fn register_builtin_plugins(&mut self) {
        // Register example plugins
        self.builtin_plugins.insert("default-b2bua".to_string(), || {
            Box::new(DefaultB2BUAExample::new())
        });
        
        self.builtin_plugins.insert("sip-authenticator".to_string(), || {
            Box::new(SipAuthenticatorPlugin::new())
        });
        
        self.builtin_plugins.insert("call-limiter".to_string(), || {
            Box::new(CallLimiterPlugin::new(100)) // Default limit of 100 calls
        });
        
        self.builtin_plugins.insert("header-manipulator".to_string(), || {
            Box::new(HeaderManipulatorPlugin::new())
        });
        
        self.builtin_plugins.insert("fraud-detector".to_string(), || {
            Box::new(FraudDetectorPlugin::new())
        });
        
        self.builtin_plugins.insert("cdr-generator".to_string(), || {
            Box::new(CdrGeneratorPlugin::new())
        });

        info!("Registered {} built-in plugins", self.builtin_plugins.len());
    }

    /// Load plugins from a directory
    pub async fn load_from_directory(&self, _plugin_dir: &Path) -> Result<Vec<(String, Box<dyn B2BUAPlugin>)>> {
        // TODO: Implement directory scanning for plugin files
        warn!("Directory-based plugin loading is not yet implemented");
        Ok(Vec::new())
    }

    /// Validate plugin file before loading
    pub async fn validate_plugin_file(&self, _plugin_path: &Path) -> Result<PluginMetadata> {
        // TODO: Implement plugin file validation
        // This would check:
        // - File format and signature
        // - Required symbols/exports
        // - Version compatibility
        // - Security checks (signatures, checksums)
        
        Err(anyhow::anyhow!("Plugin file validation not yet implemented"))
    }

    /// Create a plugin configuration template
    pub fn create_plugin_config_template(&self, plugin_name: &str) -> Result<PluginConfig> {
        let plugin = self.load_builtin(plugin_name, &PluginConfig::default())
            .await?;
        
        let metadata = plugin.metadata();
        
        Ok(PluginConfig {
            name: metadata.name.clone(),
            enabled: true,
            priority: 100,
            config: metadata.config_schema.clone().unwrap_or(serde_json::Value::Null),
            plugin_path: None,
        })
    }

    /// Get plugin information without loading
    pub async fn get_plugin_info(&self, plugin_name: &str) -> Result<PluginMetadata> {
        if self.has_builtin_plugin(plugin_name) {
            let plugin = self.load_builtin(plugin_name, &PluginConfig::default()).await?;
            Ok(plugin.metadata().clone())
        } else {
            Err(anyhow::anyhow!("Plugin '{}' not found", plugin_name))
        }
    }

    /// Check plugin compatibility with system
    pub async fn check_compatibility(&self, plugin_name: &str, system_version: &str) -> Result<bool> {
        let plugin_info = self.get_plugin_info(plugin_name).await?;
        
        // Simple version comparison (in practice, you'd use a proper semver crate)
        let is_compatible = plugin_info.min_system_version <= system_version.to_string();
        
        if !is_compatible {
            warn!(
                "Plugin '{}' requires system version {} but current is {}",
                plugin_name, plugin_info.min_system_version, system_version
            );
        }
        
        Ok(is_compatible)
    }
}

impl Default for PluginLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Plugin loading statistics
#[derive(Debug, Clone, Default)]
pub struct PluginLoadStats {
    pub total_load_attempts: u64,
    pub successful_loads: u64,
    pub failed_loads: u64,
    pub builtin_plugins_loaded: u64,
    pub external_plugins_loaded: u64,
    pub load_errors: Vec<String>,
}

/// Advanced plugin loader with statistics and caching
pub struct AdvancedPluginLoader {
    base_loader: PluginLoader,
    stats: PluginLoadStats,
    plugin_cache: std::collections::HashMap<String, Box<dyn B2BUAPlugin>>,
    cache_enabled: bool,
}

impl AdvancedPluginLoader {
    /// Create a new advanced plugin loader
    pub fn new(cache_enabled: bool) -> Self {
        Self {
            base_loader: PluginLoader::new(),
            stats: PluginLoadStats::default(),
            plugin_cache: std::collections::HashMap::new(),
            cache_enabled,
        }
    }

    /// Load plugin with caching and statistics
    pub async fn load_plugin(&mut self, plugin_name: &str, config: &PluginConfig) -> Result<Box<dyn B2BUAPlugin>> {
        self.stats.total_load_attempts += 1;
        
        // Check cache first if enabled
        if self.cache_enabled && self.plugin_cache.contains_key(plugin_name) {
            debug!("Loading plugin '{}' from cache", plugin_name);
            // Note: In practice, you'd clone the plugin or return a reference
            // For simplicity, we'll reload from the base loader
        }

        let result = if let Some(plugin_path) = &config.plugin_path {
            self.base_loader.load_from_file(plugin_path, config).await
        } else {
            self.base_loader.load_builtin(plugin_name, config).await
        };

        match result {
            Ok(plugin) => {
                self.stats.successful_loads += 1;
                if config.plugin_path.is_some() {
                    self.stats.external_plugins_loaded += 1;
                } else {
                    self.stats.builtin_plugins_loaded += 1;
                }

                // Cache the plugin if caching is enabled
                // Note: This is conceptual - actual caching would be more complex
                // as plugins contain state and can't be easily cloned
                
                Ok(plugin)
            }
            Err(e) => {
                self.stats.failed_loads += 1;
                self.stats.load_errors.push(format!("Failed to load '{}': {}", plugin_name, e));
                Err(e)
            }
        }
    }

    /// Get loading statistics
    pub fn get_stats(&self) -> &PluginLoadStats {
        &self.stats
    }

    /// Clear plugin cache
    pub fn clear_cache(&mut self) {
        self.plugin_cache.clear();
        debug!("Plugin cache cleared");
    }

    /// Get cache size
    pub fn cache_size(&self) -> usize {
        self.plugin_cache.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_plugin_loader_creation() {
        let _loader = PluginLoader::new();
    }

    #[tokio::test]
    async fn test_builtin_plugin_loading() {
        let loader = PluginLoader::new();
        
        let builtin_plugins = loader.list_builtin_plugins();
        assert!(!builtin_plugins.is_empty());
        
        // Try to load the default plugin
        if builtin_plugins.contains(&"default-b2bua".to_string()) {
            let config = PluginConfig {
                name: "default-b2bua".to_string(),
                ..Default::default()
            };
            
            let result = loader.load_builtin("default-b2bua", &config).await;
            assert!(result.is_ok());
            
            let plugin = result.expect("Plugin loading should succeed");
            assert_eq!(plugin.metadata().name, "default-b2bua");
        }
    }

    #[tokio::test]
    async fn test_plugin_info_retrieval() {
        let loader = PluginLoader::new();
        
        let result = loader.get_plugin_info("default-b2bua").await;
        assert!(result.is_ok());
        
        let metadata = result.expect("Plugin info retrieval should succeed");
        assert_eq!(metadata.name, "default-b2bua");
        assert!(!metadata.version.is_empty());
    }

    #[tokio::test]
    async fn test_plugin_compatibility_check() {
        let loader = PluginLoader::new();
        
        let result = loader.check_compatibility("default-b2bua", "1.0.0").await;
        assert!(result.is_ok());
        assert!(result.expect("Compatibility check should succeed")); // Should be compatible
    }

    #[tokio::test]
    async fn test_advanced_plugin_loader() {
        let mut loader = AdvancedPluginLoader::new(true);
        
        let config = PluginConfig {
            name: "default-b2bua".to_string(),
            ..Default::default()
        };
        
        let result = loader.load_plugin("default-b2bua", &config).await;
        assert!(result.is_ok());
        
        let stats = loader.get_stats();
        assert_eq!(stats.total_load_attempts, 1);
        assert_eq!(stats.successful_loads, 1);
        assert_eq!(stats.builtin_plugins_loaded, 1);
    }
}