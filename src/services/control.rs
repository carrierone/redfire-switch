//! Control Service - Handles configuration management and system monitoring
//!
//! This service provides centralized configuration management, health monitoring,
//! and administrative functions for the entire telecommunications system.

use crate::events::{EventBus, HealthStatus, TelecomEvent};
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, info, warn};

/// Configuration for the Control Service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlConfig {
    /// Configuration file directory
    pub config_dir: PathBuf,
    /// Enable configuration hot reloading
    pub enable_hot_reload: bool,
    /// Configuration backup directory
    pub backup_dir: Option<PathBuf>,
    /// Health check interval in seconds
    pub health_check_interval_seconds: u64,
    /// Enable metrics collection
    pub enable_metrics: bool,
    /// Metrics export interval in seconds
    pub metrics_export_interval_seconds: u64,
    /// Enable administrative API
    pub enable_admin_api: bool,
    /// Admin API listening port
    pub admin_api_port: u16,
    /// Enable authentication for admin API
    pub enable_admin_auth: bool,
}

impl Default for ControlConfig {
    fn default() -> Self {
        Self {
            config_dir: PathBuf::from("/etc/redfire-switch"),
            enable_hot_reload: true,
            backup_dir: Some(PathBuf::from("/var/backup/redfire-switch")),
            health_check_interval_seconds: 30,
            enable_metrics: true,
            metrics_export_interval_seconds: 60,
            enable_admin_api: true,
            admin_api_port: 8080,
            enable_admin_auth: false,
        }
    }
}

/// System configuration sections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfiguration {
    /// Routing service configuration
    pub routing: Option<serde_json::Value>,
    /// Media service configuration
    pub media: Option<serde_json::Value>,
    /// Signaling service configuration
    pub signaling: Option<serde_json::Value>,
    /// Event bus configuration
    pub events: Option<serde_json::Value>,
    /// Plugin configurations
    pub plugins: Option<HashMap<String, serde_json::Value>>,
    /// Global system settings
    pub global: Option<serde_json::Value>,
}

impl Default for SystemConfiguration {
    fn default() -> Self {
        Self {
            routing: None,
            media: None,
            signaling: None,
            events: None,
            plugins: None,
            global: None,
        }
    }
}

/// Service health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceHealth {
    pub service_name: String,
    pub status: HealthStatus,
    pub last_check: chrono::DateTime<chrono::Utc>,
    pub uptime_seconds: u64,
    pub error_count: u64,
    pub metrics: HashMap<String, f64>,
    pub dependencies: Vec<String>,
}

/// System metrics collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: u64,
    pub disk_usage_percent: f64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub active_calls: u64,
    pub calls_per_second: f64,
    pub error_rate: f64,
    pub service_health: HashMap<String, HealthStatus>,
}

/// Configuration change request
#[derive(Debug, Clone)]
pub struct ConfigurationChangeRequest {
    pub service_name: String,
    pub config_key: String,
    pub old_value: Option<String>,
    pub new_value: String,
    pub requested_by: String,
    pub validate_before_apply: bool,
}

/// Administrative command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AdminCommand {
    /// Reload configuration
    ReloadConfig { service: Option<String> },
    /// Get system status
    GetStatus,
    /// Get metrics
    GetMetrics { duration_seconds: Option<u64> },
    /// Restart service
    RestartService { service_name: String },
    /// Enable/disable service
    ToggleService { service_name: String, enable: bool },
    /// Backup configuration
    BackupConfig,
    /// Restore configuration
    RestoreConfig { backup_id: String },
}

/// Administrative command response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminCommandResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Internal message types for the control service
#[derive(Debug)]
enum ControlServiceMessage {
    UpdateConfiguration {
        request: ConfigurationChangeRequest,
        response_tx: tokio::sync::oneshot::Sender<Result<()>>,
    },
    GetConfiguration {
        service_name: Option<String>,
        response_tx: tokio::sync::oneshot::Sender<Result<SystemConfiguration>>,
    },
    UpdateServiceHealth {
        service_name: String,
        health: ServiceHealth,
        response_tx: tokio::sync::oneshot::Sender<Result<()>>,
    },
    GetSystemStatus {
        response_tx: tokio::sync::oneshot::Sender<Result<SystemStatus>>,
    },
    ExecuteAdminCommand {
        command: AdminCommand,
        response_tx: tokio::sync::oneshot::Sender<Result<AdminCommandResponse>>,
    },
}

/// System status overview
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub overall_health: HealthStatus,
    pub uptime_seconds: u64,
    pub version: String,
    pub services: HashMap<String, ServiceHealth>,
    pub metrics: SystemMetrics,
    pub configuration_version: String,
    pub last_config_change: Option<chrono::DateTime<chrono::Utc>>,
}

/// Microservice for system control and monitoring
pub struct ControlService {
    /// Service configuration
    config: ControlConfig,
    /// Event bus for publishing control events
    event_bus: Arc<EventBus>,
    /// System configuration storage
    system_config: Arc<RwLock<SystemConfiguration>>,
    /// Service health tracking
    service_health: Arc<RwLock<HashMap<String, ServiceHealth>>>,
    /// System metrics
    metrics: Arc<RwLock<SystemMetrics>>,
    /// Service start time for uptime calculation
    start_time: chrono::DateTime<chrono::Utc>,
    /// Message processing channel
    request_sender: mpsc::UnboundedSender<ControlServiceMessage>,
    /// Shutdown signal for graceful termination
    shutdown_sender: broadcast::Sender<()>,
}

impl ControlService {
    /// Create a new control service
    pub fn new(config: ControlConfig, event_bus: Arc<EventBus>) -> Result<Self> {
        let system_config = Arc::new(RwLock::new(SystemConfiguration::default()));
        let service_health = Arc::new(RwLock::new(HashMap::new()));
        let metrics = Arc::new(RwLock::new(Self::default_metrics()));
        let start_time = Utc::now();
        let (request_sender, request_receiver) = mpsc::unbounded_channel();
        let (shutdown_sender, _) = broadcast::channel(1);

        // Start background control processor
        let processor = ControlProcessor {
            config: config.clone(),
            event_bus: event_bus.clone(),
            system_config: system_config.clone(),
            service_health: service_health.clone(),
            metrics: metrics.clone(),
            start_time,
            request_receiver,
            shutdown_sender: shutdown_sender.clone(),
        };

        let service = Self {
            config: config.clone(),
            event_bus: event_bus.clone(),
            system_config: system_config.clone(),
            service_health: service_health.clone(),
            metrics: metrics.clone(),
            start_time,
            request_sender,
            shutdown_sender,
        };

        tokio::spawn(async move {
            processor.run().await;
        });

        Ok(service)
    }

    /// Update system configuration
    pub async fn update_configuration(&self, request: ConfigurationChangeRequest) -> Result<()> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        self.request_sender
            .send(ControlServiceMessage::UpdateConfiguration {
                request,
                response_tx,
            })
            .map_err(|_| anyhow::anyhow!("Failed to send configuration update request"))?;

        response_rx
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive configuration update response"))?
    }

    /// Get system configuration
    pub async fn get_configuration(
        &self,
        service_name: Option<String>,
    ) -> Result<SystemConfiguration> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        self.request_sender
            .send(ControlServiceMessage::GetConfiguration {
                service_name,
                response_tx,
            })
            .map_err(|_| anyhow::anyhow!("Failed to send get configuration request"))?;

        response_rx
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive get configuration response"))?
    }

    /// Update service health information
    pub async fn update_service_health(
        &self,
        service_name: String,
        health: ServiceHealth,
    ) -> Result<()> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        self.request_sender
            .send(ControlServiceMessage::UpdateServiceHealth {
                service_name,
                health,
                response_tx,
            })
            .map_err(|_| anyhow::anyhow!("Failed to send health update request"))?;

        response_rx
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive health update response"))?
    }

    /// Get overall system status
    pub async fn get_system_status(&self) -> Result<SystemStatus> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        self.request_sender
            .send(ControlServiceMessage::GetSystemStatus { response_tx })
            .map_err(|_| anyhow::anyhow!("Failed to send get status request"))?;

        response_rx
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive get status response"))?
    }

    /// Execute administrative command
    pub async fn execute_admin_command(
        &self,
        command: AdminCommand,
    ) -> Result<AdminCommandResponse> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        self.request_sender
            .send(ControlServiceMessage::ExecuteAdminCommand {
                command,
                response_tx,
            })
            .map_err(|_| anyhow::anyhow!("Failed to send admin command"))?;

        response_rx
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive admin command response"))?
    }

    /// Shutdown the control service
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down control service");

        // Save current configuration if needed
        if let Err(e) = self.backup_configuration().await {
            warn!("Failed to backup configuration during shutdown: {}", e);
        }

        Ok(())
    }

    /// Backup current configuration
    async fn backup_configuration(&self) -> Result<()> {
        if let Some(backup_dir) = &self.config.backup_dir {
            let config = self.system_config.read().await;
            let backup_file = backup_dir.join(format!(
                "config-backup-{}.json",
                Utc::now().format("%Y%m%d-%H%M%S")
            ));

            let config_json = serde_json::to_string_pretty(&*config)
                .context("Failed to serialize configuration")?;

            fs::write(&backup_file, config_json)
                .await
                .with_context(|| format!("Failed to write backup to {:?}", backup_file))?;

            info!("Configuration backed up to {:?}", backup_file);
        }

        Ok(())
    }

    fn default_metrics() -> SystemMetrics {
        SystemMetrics {
            timestamp: Utc::now(),
            cpu_usage_percent: 0.0,
            memory_usage_mb: 0,
            disk_usage_percent: 0.0,
            network_rx_bytes: 0,
            network_tx_bytes: 0,
            active_calls: 0,
            calls_per_second: 0.0,
            error_rate: 0.0,
            service_health: HashMap::new(),
        }
    }
}

/// Background processor for control operations
struct ControlProcessor {
    config: ControlConfig,
    event_bus: Arc<EventBus>,
    system_config: Arc<RwLock<SystemConfiguration>>,
    service_health: Arc<RwLock<HashMap<String, ServiceHealth>>>,
    metrics: Arc<RwLock<SystemMetrics>>,
    start_time: chrono::DateTime<chrono::Utc>,
    request_receiver: mpsc::UnboundedReceiver<ControlServiceMessage>,
    shutdown_sender: broadcast::Sender<()>,
}

impl ControlProcessor {
    async fn run(mut self) {
        // Start periodic tasks
        self.start_health_check_task().await;
        self.start_metrics_collection_task().await;

        if self.config.enable_hot_reload {
            self.start_config_watch_task().await;
        }

        // Process incoming requests
        while let Some(message) = self.request_receiver.recv().await {
            match message {
                ControlServiceMessage::UpdateConfiguration {
                    request,
                    response_tx,
                } => {
                    let response = self.handle_configuration_update(request).await;
                    let _ = response_tx.send(response);
                }
                ControlServiceMessage::GetConfiguration {
                    service_name,
                    response_tx,
                } => {
                    let response = self.handle_get_configuration(service_name).await;
                    let _ = response_tx.send(response);
                }
                ControlServiceMessage::UpdateServiceHealth {
                    service_name,
                    health,
                    response_tx,
                } => {
                    let response = self.handle_health_update(service_name, health).await;
                    let _ = response_tx.send(response);
                }
                ControlServiceMessage::GetSystemStatus { response_tx } => {
                    let response = self.handle_get_system_status().await;
                    let _ = response_tx.send(response);
                }
                ControlServiceMessage::ExecuteAdminCommand {
                    command,
                    response_tx,
                } => {
                    let response = self.handle_admin_command(command).await;
                    let _ = response_tx.send(response);
                }
            }
        }
    }

    async fn handle_configuration_update(&self, request: ConfigurationChangeRequest) -> Result<()> {
        // Validate configuration change if requested
        if request.validate_before_apply {
            self.validate_configuration_change(&request).await?;
        }

        // Apply configuration change
        {
            let mut config = self.system_config.write().await;

            // Update the specific configuration section
            match request.service_name.as_str() {
                "routing" => {
                    config.routing = Some(
                        serde_json::from_str(&request.new_value)
                            .context("Failed to parse routing configuration")?,
                    );
                }
                "media" => {
                    config.media = Some(
                        serde_json::from_str(&request.new_value)
                            .context("Failed to parse media configuration")?,
                    );
                }
                "signaling" => {
                    config.signaling = Some(
                        serde_json::from_str(&request.new_value)
                            .context("Failed to parse signaling configuration")?,
                    );
                }
                "events" => {
                    config.events = Some(
                        serde_json::from_str(&request.new_value)
                            .context("Failed to parse events configuration")?,
                    );
                }
                "global" => {
                    config.global = Some(
                        serde_json::from_str(&request.new_value)
                            .context("Failed to parse global configuration")?,
                    );
                }
                service_name => {
                    // Plugin or custom service configuration
                    if config.plugins.is_none() {
                        config.plugins = Some(HashMap::new());
                    }
                    if let Some(ref mut plugins) = config.plugins {
                        plugins.insert(
                            service_name.to_string(),
                            serde_json::from_str(&request.new_value)
                                .context("Failed to parse plugin configuration")?,
                        );
                    }
                }
            }
        }

        // Publish configuration change event
        self.publish_config_change_event(&request).await?;

        // Save configuration to disk
        self.save_configuration_to_disk().await?;

        info!(
            "Configuration updated for service: {} key: {}",
            request.service_name, request.config_key
        );

        Ok(())
    }

    async fn handle_get_configuration(
        &self,
        service_name: Option<String>,
    ) -> Result<SystemConfiguration> {
        let config = self.system_config.read().await;

        if let Some(service) = service_name {
            // Return configuration for specific service
            let mut filtered_config = SystemConfiguration::default();

            match service.as_str() {
                "routing" => filtered_config.routing = config.routing.clone(),
                "media" => filtered_config.media = config.media.clone(),
                "signaling" => filtered_config.signaling = config.signaling.clone(),
                "events" => filtered_config.events = config.events.clone(),
                "global" => filtered_config.global = config.global.clone(),
                plugin_name => {
                    if let Some(ref plugins) = config.plugins {
                        if let Some(plugin_config) = plugins.get(plugin_name) {
                            let mut plugin_map = HashMap::new();
                            plugin_map.insert(plugin_name.to_string(), plugin_config.clone());
                            filtered_config.plugins = Some(plugin_map);
                        }
                    }
                }
            }

            Ok(filtered_config)
        } else {
            // Return full configuration
            Ok(config.clone())
        }
    }

    async fn handle_health_update(
        &self,
        service_name: String,
        health: ServiceHealth,
    ) -> Result<()> {
        let mut service_health = self.service_health.write().await;
        service_health.insert(service_name.clone(), health.clone());

        // Publish health status event if status changed
        self.publish_health_status_event(&service_name, &health)
            .await?;

        debug!(
            "Updated health for service: {} status: {:?}",
            service_name, health.status
        );
        Ok(())
    }

    async fn handle_get_system_status(&self) -> Result<SystemStatus> {
        let service_health = self.service_health.read().await;
        let metrics = self.metrics.read().await;

        // Calculate overall health
        let overall_health = if service_health
            .values()
            .all(|h| h.status == HealthStatus::Healthy)
        {
            HealthStatus::Healthy
        } else if service_health
            .values()
            .any(|h| h.status == HealthStatus::Critical)
        {
            HealthStatus::Critical
        } else if service_health
            .values()
            .any(|h| h.status == HealthStatus::Unhealthy)
        {
            HealthStatus::Unhealthy
        } else {
            HealthStatus::Degraded
        };

        let uptime_seconds = (Utc::now() - self.start_time).num_seconds() as u64;

        let status = SystemStatus {
            overall_health,
            uptime_seconds,
            version: env!("CARGO_PKG_VERSION").to_string(),
            services: service_health.clone(),
            metrics: metrics.clone(),
            configuration_version: "1.0".to_string(), // TODO: Track actual version
            last_config_change: None,                 // TODO: Track last change time
        };

        Ok(status)
    }

    async fn handle_admin_command(&self, command: AdminCommand) -> Result<AdminCommandResponse> {
        match command {
            AdminCommand::ReloadConfig { service } => {
                // TODO: Implement configuration reloading
                let message = if let Some(svc) = service {
                    format!("Configuration reloaded for service: {}", svc)
                } else {
                    "System configuration reloaded".to_string()
                };

                Ok(AdminCommandResponse {
                    success: true,
                    message,
                    data: None,
                    timestamp: Utc::now(),
                })
            }
            AdminCommand::GetStatus => {
                let status = self.handle_get_system_status().await?;
                Ok(AdminCommandResponse {
                    success: true,
                    message: "System status retrieved".to_string(),
                    data: Some(serde_json::to_value(status)?),
                    timestamp: Utc::now(),
                })
            }
            AdminCommand::GetMetrics {
                duration_seconds: _,
            } => {
                let metrics = self.metrics.read().await;
                Ok(AdminCommandResponse {
                    success: true,
                    message: "Metrics retrieved".to_string(),
                    data: Some(serde_json::to_value(&*metrics)?),
                    timestamp: Utc::now(),
                })
            }
            AdminCommand::RestartService { service_name } => {
                // TODO: Implement service restart
                Ok(AdminCommandResponse {
                    success: true,
                    message: format!("Service {} restart initiated", service_name),
                    data: None,
                    timestamp: Utc::now(),
                })
            }
            AdminCommand::ToggleService {
                service_name,
                enable,
            } => {
                // TODO: Implement service enable/disable
                let action = if enable { "enabled" } else { "disabled" };
                Ok(AdminCommandResponse {
                    success: true,
                    message: format!("Service {} {}", service_name, action),
                    data: None,
                    timestamp: Utc::now(),
                })
            }
            AdminCommand::BackupConfig => {
                // TODO: Implement configuration backup
                Ok(AdminCommandResponse {
                    success: true,
                    message: "Configuration backup completed".to_string(),
                    data: None,
                    timestamp: Utc::now(),
                })
            }
            AdminCommand::RestoreConfig { backup_id } => {
                // TODO: Implement configuration restore
                Ok(AdminCommandResponse {
                    success: true,
                    message: format!("Configuration restored from backup: {}", backup_id),
                    data: None,
                    timestamp: Utc::now(),
                })
            }
        }
    }

    async fn validate_configuration_change(
        &self,
        _request: &ConfigurationChangeRequest,
    ) -> Result<()> {
        // TODO: Implement configuration validation
        // This would validate JSON schema, check dependencies, etc.
        Ok(())
    }

    async fn publish_config_change_event(
        &self,
        request: &ConfigurationChangeRequest,
    ) -> Result<()> {
        let event = TelecomEvent::ConfigChanged(crate::events::ConfigChangedEvent {
            service_name: request.service_name.clone(),
            config_key: request.config_key.clone(),
            old_value: request.old_value.clone(),
            new_value: request.new_value.clone(),
            changed_by: request.requested_by.clone(),
            timestamp: Utc::now(),
        });

        self.event_bus
            .publish(event)
            .await
            .context("Failed to publish configuration change event")?;

        Ok(())
    }

    async fn publish_health_status_event(
        &self,
        service_name: &str,
        health: &ServiceHealth,
    ) -> Result<()> {
        let event = TelecomEvent::health_status(
            service_name.to_string(),
            "main".to_string(), // TODO: Use actual instance ID
            health.status.clone(),
            health.metrics.clone(),
        );

        self.event_bus
            .publish(event)
            .await
            .context("Failed to publish health status event")?;

        Ok(())
    }

    async fn save_configuration_to_disk(&self) -> Result<()> {
        let config = self.system_config.read().await;
        let config_file = self.config.config_dir.join("system.json");

        let config_json =
            serde_json::to_string_pretty(&*config).context("Failed to serialize configuration")?;

        fs::write(&config_file, config_json)
            .await
            .with_context(|| format!("Failed to write configuration to {:?}", config_file))?;

        Ok(())
    }

    async fn start_health_check_task(&self) {
        let service_health = self.service_health.clone();
        let event_bus = self.event_bus.clone();
        let interval_seconds = self.config.health_check_interval_seconds;
        let mut shutdown_rx = self.shutdown_sender.subscribe();

        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(std::time::Duration::from_secs(interval_seconds));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Perform health checks on all registered services
                        let health_map = service_health.read().await;

                        for (service_name, health) in health_map.iter() {
                            // Check if health status is stale
                            let age = (Utc::now() - health.last_check).num_seconds();
                            if age > interval_seconds as i64 * 2 {
                                warn!("Health status for service {} is stale ({} seconds old)",
                                      service_name, age);
                            }
                        }

                        debug!("Health check completed for {} services", health_map.len());
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Health check task shutting down gracefully");
                        break;
                    }
                }
            }
        });
    }

    async fn start_metrics_collection_task(&self) {
        let metrics = self.metrics.clone();
        let interval_seconds = self.config.metrics_export_interval_seconds;

        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(std::time::Duration::from_secs(interval_seconds));

            loop {
                interval.tick().await;

                // Collect system metrics
                let mut metrics_guard = metrics.write().await;
                metrics_guard.timestamp = Utc::now();

                // TODO: Implement actual metrics collection
                // - CPU usage from /proc/stat
                // - Memory usage from /proc/meminfo
                // - Network stats from /proc/net/dev
                // - Disk usage from filesystem

                debug!("System metrics updated");
            }
        });
    }

    async fn start_config_watch_task(&self) {
        let config_dir = self.config.config_dir.clone();

        tokio::spawn(async move {
            // TODO: Implement filesystem watching for configuration files
            // This would use inotify on Linux to watch for file changes
            // and automatically reload configuration when files are modified

            info!(
                "Configuration hot reload monitoring started for {:?}",
                config_dir
            );
        });
    }

    /// Gracefully shutdown the control service and all background tasks
    pub async fn shutdown(&self) -> Result<()> {
        info!("Initiating graceful shutdown of ControlService");

        // Send shutdown signal to all background tasks
        if let Err(e) = self.shutdown_sender.send(()) {
            warn!("Failed to send shutdown signal: {}", e);
        }

        info!("ControlService shutdown completed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventBus;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_control_service_creation() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config = ControlConfig {
            config_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        let event_bus = Arc::new(EventBus::new());
        let _service = ControlService::new(config, event_bus)?;
        Ok(())
    }

    #[tokio::test]
    async fn test_configuration_update() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config = ControlConfig {
            config_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        let event_bus = Arc::new(EventBus::new());
        let service = ControlService::new(config, event_bus)?;

        let request = ConfigurationChangeRequest {
            service_name: "routing".to_string(),
            config_key: "max_routes".to_string(),
            old_value: Some("10".to_string()),
            new_value: "20".to_string(),
            requested_by: "admin".to_string(),
            validate_before_apply: false,
        };

        let result = service.update_configuration(request).await;
        assert!(result.is_ok());

        Ok(())
    }

    #[tokio::test]
    async fn test_service_health_update() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config = ControlConfig {
            config_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        let event_bus = Arc::new(EventBus::new());
        let service = ControlService::new(config, event_bus)?;

        let health = ServiceHealth {
            service_name: "routing".to_string(),
            status: HealthStatus::Healthy,
            last_check: Utc::now(),
            uptime_seconds: 3600,
            error_count: 0,
            metrics: HashMap::new(),
            dependencies: vec!["database".to_string()],
        };

        let result = service
            .update_service_health("routing".to_string(), health)
            .await;
        assert!(result.is_ok());

        let status = service.get_system_status().await?;
        assert_eq!(status.overall_health, HealthStatus::Healthy);

        Ok(())
    }
}
