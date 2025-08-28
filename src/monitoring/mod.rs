//! Enhanced monitoring and alerting system
//! 
//! This module provides comprehensive monitoring capabilities including
//! real-time metrics collection, health checks, alerting, and observability.

pub mod metrics;
pub mod health;
pub mod alerts;
pub mod dashboard;

pub use metrics::*;
pub use health::*;
pub use alerts::*;
pub use dashboard::*;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info, warn, error};

/// Monitoring system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable monitoring system
    pub enabled: bool,
    /// Metrics collection interval (seconds)
    pub metrics_interval_seconds: u64,
    /// Health check interval (seconds)
    pub health_check_interval_seconds: u64,
    /// Alert evaluation interval (seconds)
    pub alert_evaluation_interval_seconds: u64,
    /// Metrics retention period (hours)
    pub metrics_retention_hours: u64,
    /// Enable real-time dashboard
    pub enable_dashboard: bool,
    /// Enable external metrics export (Prometheus, etc.)
    pub enable_external_export: bool,
    /// Enable alerting
    pub enable_alerting: bool,
    /// Alert notification endpoints
    pub notification_endpoints: Vec<NotificationEndpoint>,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            metrics_interval_seconds: 30,
            health_check_interval_seconds: 60,
            alert_evaluation_interval_seconds: 30,
            metrics_retention_hours: 24,
            enable_dashboard: true,
            enable_external_export: false,
            enable_alerting: true,
            notification_endpoints: vec![
                NotificationEndpoint {
                    name: "console".to_string(),
                    endpoint_type: NotificationEndpointType::Console,
                    config: HashMap::new(),
                    enabled: true,
                },
            ],
        }
    }
}

/// Notification endpoint configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationEndpoint {
    /// Endpoint name
    pub name: String,
    /// Endpoint type
    pub endpoint_type: NotificationEndpointType,
    /// Endpoint-specific configuration
    pub config: HashMap<String, String>,
    /// Whether endpoint is enabled
    pub enabled: bool,
}

/// Supported notification endpoint types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationEndpointType {
    /// Console/log output
    Console,
    /// Email notifications
    Email,
    /// Slack webhooks
    Slack,
    /// PagerDuty
    PagerDuty,
    /// Generic webhook
    Webhook,
    /// SMS notifications
    Sms,
}

/// Monitoring system event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MonitoringEvent {
    /// Metrics collected
    MetricsCollected {
        timestamp: chrono::DateTime<chrono::Utc>,
        metrics: SystemMetricsSnapshot,
    },
    /// Health check completed
    HealthCheckCompleted {
        timestamp: chrono::DateTime<chrono::Utc>,
        results: HashMap<String, HealthStatus>,
    },
    /// Alert triggered
    AlertTriggered {
        alert: Alert,
    },
    /// Alert resolved
    AlertResolved {
        alert_id: String,
        resolved_at: chrono::DateTime<chrono::Utc>,
    },
    /// System status changed
    SystemStatusChanged {
        previous_status: SystemStatus,
        new_status: SystemStatus,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
}

/// Overall system status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemStatus {
    /// All systems operational
    Healthy,
    /// Some non-critical issues
    Degraded,
    /// Critical issues affecting service
    Unhealthy,
    /// System is starting up
    Starting,
    /// System is shutting down
    Stopping,
    /// System status unknown
    Unknown,
}

/// Comprehensive monitoring system
pub struct MonitoringSystem {
    /// Configuration
    config: MonitoringConfig,
    /// Metrics collector
    metrics_collector: Arc<MetricsCollector>,
    /// Health checker
    health_checker: Arc<HealthChecker>,
    /// Alert manager
    alert_manager: Arc<AlertManager>,
    /// Dashboard manager
    dashboard_manager: Arc<DashboardManager>,
    /// Event broadcaster
    event_sender: broadcast::Sender<MonitoringEvent>,
    /// Current system status
    system_status: Arc<RwLock<SystemStatus>>,
    /// Monitoring start time
    start_time: Instant,
}

impl MonitoringSystem {
    /// Create new monitoring system
    pub fn new(config: MonitoringConfig) -> Result<Self> {
        let (event_sender, _) = broadcast::channel(1000);
        
        let metrics_collector = Arc::new(MetricsCollector::new(
            config.metrics_interval_seconds,
            config.metrics_retention_hours,
        )?);
        
        let health_checker = Arc::new(HealthChecker::new(
            config.health_check_interval_seconds,
        )?);
        
        let alert_manager = Arc::new(AlertManager::new(
            config.alert_evaluation_interval_seconds,
            config.notification_endpoints.clone(),
        )?);
        
        let dashboard_manager = Arc::new(DashboardManager::new(
            config.enable_dashboard,
        )?);
        
        Ok(Self {
            config,
            metrics_collector,
            health_checker,
            alert_manager,
            dashboard_manager,
            event_sender,
            system_status: Arc::new(RwLock::new(SystemStatus::Starting)),
            start_time: Instant::now(),
        })
    }
    
    /// Start monitoring system
    pub async fn start(&self) -> Result<()> {
        if !self.config.enabled {
            info!("Monitoring system disabled, skipping start");
            return Ok(());
        }
        
        info!("Starting monitoring system...");
        
        // Start metrics collection
        self.start_metrics_collection().await?;
        
        // Start health checking
        self.start_health_checking().await?;
        
        // Start alert evaluation
        if self.config.enable_alerting {
            self.start_alert_evaluation().await?;
        }
        
        // Start dashboard if enabled
        if self.config.enable_dashboard {
            self.dashboard_manager.start().await?;
        }
        
        // Update system status
        *self.system_status.write().await = SystemStatus::Healthy;
        
        // Send status change event
        let _ = self.event_sender.send(MonitoringEvent::SystemStatusChanged {
            previous_status: SystemStatus::Starting,
            new_status: SystemStatus::Healthy,
            timestamp: chrono::Utc::now(),
        });
        
        info!("Monitoring system started successfully");
        Ok(())
    }
    
    /// Start metrics collection task
    async fn start_metrics_collection(&self) -> Result<()> {
        let collector = self.metrics_collector.clone();
        let event_sender = self.event_sender.clone();
        let interval = Duration::from_secs(self.config.metrics_interval_seconds);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);
            
            loop {
                interval.tick().await;
                
                match collector.collect_metrics().await {
                    Ok(metrics) => {
                        let event = MonitoringEvent::MetricsCollected {
                            timestamp: chrono::Utc::now(),
                            metrics,
                        };
                        
                        if let Err(e) = event_sender.send(event) {
                            error!("Failed to send metrics event: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Failed to collect metrics: {}", e);
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Start health checking task
    async fn start_health_checking(&self) -> Result<()> {
        let checker = self.health_checker.clone();
        let event_sender = self.event_sender.clone();
        let interval = Duration::from_secs(self.config.health_check_interval_seconds);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);
            
            loop {
                interval.tick().await;
                
                match checker.check_all_health().await {
                    Ok(results) => {
                        let event = MonitoringEvent::HealthCheckCompleted {
                            timestamp: chrono::Utc::now(),
                            results,
                        };
                        
                        if let Err(e) = event_sender.send(event) {
                            error!("Failed to send health check event: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Failed to perform health checks: {}", e);
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Start alert evaluation task
    async fn start_alert_evaluation(&self) -> Result<()> {
        let alert_manager = self.alert_manager.clone();
        let metrics_collector = self.metrics_collector.clone();
        let health_checker = self.health_checker.clone();
        let event_sender = self.event_sender.clone();
        let interval = Duration::from_secs(self.config.alert_evaluation_interval_seconds);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);
            
            loop {
                interval.tick().await;
                
                // Get current metrics and health status
                let metrics = match metrics_collector.get_latest_metrics().await {
                    Ok(m) => m,
                    Err(e) => {
                        error!("Failed to get metrics for alert evaluation: {}", e);
                        continue;
                    }
                };
                
                let health_status = match health_checker.get_current_health().await {
                    Ok(h) => h,
                    Err(e) => {
                        error!("Failed to get health status for alert evaluation: {}", e);
                        continue;
                    }
                };
                
                // Evaluate alerts
                match alert_manager.evaluate_alerts(&metrics, &health_status).await {
                    Ok(triggered_alerts) => {
                        for alert in triggered_alerts {
                            let event = MonitoringEvent::AlertTriggered {
                                alert: alert.clone(),
                            };
                            
                            if let Err(e) = event_sender.send(event) {
                                error!("Failed to send alert event: {}", e);
                            }
                            
                            // Send notifications
                            if let Err(e) = alert_manager.send_alert_notification(&alert).await {
                                error!("Failed to send alert notification: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to evaluate alerts: {}", e);
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Get event subscriber
    pub fn subscribe_events(&self) -> broadcast::Receiver<MonitoringEvent> {
        self.event_sender.subscribe()
    }
    
    /// Get current system status
    pub async fn get_system_status(&self) -> SystemStatus {
        *self.system_status.read().await
    }
    
    /// Get system uptime
    pub fn get_uptime(&self) -> Duration {
        self.start_time.elapsed()
    }
    
    /// Get metrics collector
    pub fn metrics(&self) -> Arc<MetricsCollector> {
        self.metrics_collector.clone()
    }
    
    /// Get health checker
    pub fn health(&self) -> Arc<HealthChecker> {
        self.health_checker.clone()
    }
    
    /// Get alert manager
    pub fn alerts(&self) -> Arc<AlertManager> {
        self.alert_manager.clone()
    }
    
    /// Get dashboard manager
    pub fn dashboard(&self) -> Arc<DashboardManager> {
        self.dashboard_manager.clone()
    }
    
    /// Shutdown monitoring system
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down monitoring system...");
        
        // Update system status
        *self.system_status.write().await = SystemStatus::Stopping;
        
        // Send status change event
        let _ = self.event_sender.send(MonitoringEvent::SystemStatusChanged {
            previous_status: SystemStatus::Healthy,
            new_status: SystemStatus::Stopping,
            timestamp: chrono::Utc::now(),
        });
        
        // Shutdown components
        if self.config.enable_dashboard {
            self.dashboard_manager.stop().await?;
        }
        
        // Final metrics collection
        if let Ok(final_metrics) = self.metrics_collector.collect_metrics().await {
            let _ = self.event_sender.send(MonitoringEvent::MetricsCollected {
                timestamp: chrono::Utc::now(),
                metrics: final_metrics,
            });
        }
        
        info!("Monitoring system shutdown complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_monitoring_system_creation() {
        let config = MonitoringConfig::default();
        let monitoring = MonitoringSystem::new(config).unwrap();
        
        assert_eq!(monitoring.get_system_status().await, SystemStatus::Starting);
        assert!(monitoring.get_uptime().as_secs() >= 0);
    }
    
    #[tokio::test]
    async fn test_event_subscription() {
        let config = MonitoringConfig::default();
        let monitoring = MonitoringSystem::new(config).unwrap();
        
        let mut receiver = monitoring.subscribe_events();
        
        // Should be able to receive events
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                // Timeout is expected since no events are being sent
            }
            _ = receiver.recv() => {
                panic!("Should not receive events in this test");
            }
        }
    }
}