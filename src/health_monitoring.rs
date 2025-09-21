//! Health Monitoring and System Observability Service
//! Comprehensive health checks, metrics collection, and alerting for production deployment

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMonitoringConfig {
    pub enabled: bool,
    pub check_interval_seconds: u64,
    pub unhealthy_threshold: u32,
    pub recovery_threshold: u32,
    pub timeout_seconds: u64,
    pub prometheus_enabled: bool,
    pub prometheus_port: u16,
    pub alert_channels: Vec<AlertChannel>,
    pub components: Vec<ComponentConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentConfig {
    pub name: String,
    pub component_type: ComponentType,
    pub enabled: bool,
    pub critical: bool, // If true, system is unhealthy when this component fails
    pub timeout_seconds: u64,
    pub custom_config: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComponentType {
    Database,
    SipStack,
    CodecEngine,
    RtpProxy,
    LcrEngine,
    SecurityService,
    FileSystem,
    NetworkConnectivity,
    MemoryUsage,
    CpuUsage,
    DiskUsage,
    CustomEndpoint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertChannel {
    pub name: String,
    pub channel_type: AlertChannelType,
    pub config: HashMap<String, String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertChannelType {
    Email,
    Webhook,
    Slack,
    Sms,
    Pagerduty,
    Log,
}

impl Default for HealthMonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            check_interval_seconds: 30,
            unhealthy_threshold: 3,
            recovery_threshold: 2,
            timeout_seconds: 10,
            prometheus_enabled: true,
            prometheus_port: 9090,
            alert_channels: vec![AlertChannel {
                name: "log".to_string(),
                channel_type: AlertChannelType::Log,
                config: HashMap::new(),
                enabled: true,
            }],
            components: vec![
                ComponentConfig {
                    name: "database".to_string(),
                    component_type: ComponentType::Database,
                    enabled: true,
                    critical: true,
                    timeout_seconds: 5,
                    custom_config: HashMap::new(),
                },
                ComponentConfig {
                    name: "sip_stack".to_string(),
                    component_type: ComponentType::SipStack,
                    enabled: true,
                    critical: true,
                    timeout_seconds: 5,
                    custom_config: HashMap::new(),
                },
                ComponentConfig {
                    name: "memory_usage".to_string(),
                    component_type: ComponentType::MemoryUsage,
                    enabled: true,
                    critical: false,
                    timeout_seconds: 1,
                    custom_config: HashMap::new(),
                },
                ComponentConfig {
                    name: "cpu_usage".to_string(),
                    component_type: ComponentType::CpuUsage,
                    enabled: true,
                    critical: false,
                    timeout_seconds: 1,
                    custom_config: HashMap::new(),
                },
                ComponentConfig {
                    name: "disk_usage".to_string(),
                    component_type: ComponentType::DiskUsage,
                    enabled: true,
                    critical: false,
                    timeout_seconds: 1,
                    custom_config: HashMap::new(),
                },
            ],
        }
    }
}

pub struct HealthMonitoringService {
    config: HealthMonitoringConfig,
    component_states: Arc<RwLock<HashMap<String, ComponentState>>>,
    system_metrics: Arc<RwLock<SystemMetrics>>,
    alert_sender: mpsc::UnboundedSender<Alert>,
    database_service: Option<Arc<crate::database::DatabaseService>>,
}

#[derive(Debug, Clone)]
struct ComponentState {
    name: String,
    status: HealthStatus,
    last_check: DateTime<Utc>,
    consecutive_failures: u32,
    consecutive_successes: u32,
    response_time_ms: Option<f64>,
    error_message: Option<String>,
    metrics: HashMap<String, f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct SystemMetrics {
    uptime_seconds: u64,
    memory_usage_percentage: f64,
    cpu_usage_percentage: f64,
    disk_usage_percentage: f64,
    active_connections: u32,
    total_requests: u64,
    error_rate_percentage: f64,
    response_time_p95_ms: f64,
}

#[derive(Debug, Clone)]
struct Alert {
    id: Uuid,
    component: String,
    severity: AlertSeverity,
    title: String,
    description: String,
    timestamp: DateTime<Utc>,
    resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

impl HealthMonitoringService {
    pub async fn new(
        config: HealthMonitoringConfig,
        database_service: Option<Arc<crate::database::DatabaseService>>,
    ) -> Result<Self> {
        let (alert_sender, alert_receiver) = mpsc::unbounded_channel();

        let service = Self {
            config: config.clone(),
            component_states: Arc::new(RwLock::new(HashMap::new())),
            system_metrics: Arc::new(RwLock::new(SystemMetrics::default())),
            alert_sender,
            database_service,
        };

        // Initialize component states
        {
            let mut states = service.component_states.write().await;
            for component in &config.components {
                states.insert(
                    component.name.clone(),
                    ComponentState {
                        name: component.name.clone(),
                        status: HealthStatus::Unknown,
                        last_check: Utc::now(),
                        consecutive_failures: 0,
                        consecutive_successes: 0,
                        response_time_ms: None,
                        error_message: None,
                        metrics: HashMap::new(),
                    },
                );
            }
        }

        // Start alert processing task
        service.start_alert_processor(alert_receiver).await;

        // Start health check task
        if config.enabled {
            service.start_health_check_task().await;
        }

        // Start metrics collection task
        service.start_metrics_collection_task().await;

        // Start Prometheus exporter if enabled
        if config.prometheus_enabled {
            service.start_prometheus_exporter().await?;
        }

        info!("Health monitoring service initialized");
        Ok(service)
    }

    async fn start_health_check_task(&self) {
        let config = self.config.clone();
        let component_states = self.component_states.clone();
        let alert_sender = self.alert_sender.clone();
        let database_service = self.database_service.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(config.check_interval_seconds));

            loop {
                interval.tick().await;

                for component_config in &config.components {
                    if !component_config.enabled {
                        continue;
                    }

                    let check_result =
                        Self::perform_health_check(component_config, &database_service).await;

                    let mut states = component_states.write().await;
                    if let Some(state) = states.get_mut(&component_config.name) {
                        Self::update_component_state(state, check_result, &config, &alert_sender)
                            .await;
                    }
                }
            }
        });
    }

    async fn perform_health_check(
        component_config: &ComponentConfig,
        database_service: &Option<Arc<crate::database::DatabaseService>>,
    ) -> HealthCheckResult {
        let start_time = Instant::now();
        let timeout = Duration::from_secs(component_config.timeout_seconds);

        let result = tokio::time::timeout(timeout, async {
            match component_config.component_type {
                ComponentType::Database => {
                    if let Some(db) = database_service {
                        match db.health_check().await {
                            Ok(status) => {
                                if status.connected {
                                    HealthCheckResult::Healthy {
                                        response_time_ms: status.response_time_ms,
                                        metrics: HashMap::new(),
                                    }
                                } else {
                                    HealthCheckResult::Unhealthy {
                                        error: status
                                            .error
                                            .unwrap_or("Database not connected".to_string()),
                                        response_time_ms: Some(status.response_time_ms),
                                    }
                                }
                            }
                            Err(e) => HealthCheckResult::Unhealthy {
                                error: e.to_string(),
                                response_time_ms: None,
                            },
                        }
                    } else {
                        HealthCheckResult::Unhealthy {
                            error: "Database service not configured".to_string(),
                            response_time_ms: None,
                        }
                    }
                }
                ComponentType::MemoryUsage => match Self::check_memory_usage().await {
                    Ok(usage_percentage) => {
                        let mut metrics = HashMap::new();
                        metrics.insert("usage_percentage".to_string(), usage_percentage);

                        if usage_percentage > 90.0 {
                            HealthCheckResult::Unhealthy {
                                error: format!("Memory usage too high: {:.1}%", usage_percentage),
                                response_time_ms: Some(start_time.elapsed().as_millis() as f64),
                            }
                        } else if usage_percentage > 80.0 {
                            HealthCheckResult::Degraded {
                                warning: format!("High memory usage: {:.1}%", usage_percentage),
                                response_time_ms: start_time.elapsed().as_millis() as f64,
                                metrics,
                            }
                        } else {
                            HealthCheckResult::Healthy {
                                response_time_ms: start_time.elapsed().as_millis() as f64,
                                metrics,
                            }
                        }
                    }
                    Err(e) => HealthCheckResult::Unhealthy {
                        error: e.to_string(),
                        response_time_ms: None,
                    },
                },
                ComponentType::CpuUsage => match Self::check_cpu_usage().await {
                    Ok(usage_percentage) => {
                        let mut metrics = HashMap::new();
                        metrics.insert("usage_percentage".to_string(), usage_percentage);

                        if usage_percentage > 95.0 {
                            HealthCheckResult::Unhealthy {
                                error: format!("CPU usage too high: {:.1}%", usage_percentage),
                                response_time_ms: Some(start_time.elapsed().as_millis() as f64),
                            }
                        } else if usage_percentage > 85.0 {
                            HealthCheckResult::Degraded {
                                warning: format!("High CPU usage: {:.1}%", usage_percentage),
                                response_time_ms: start_time.elapsed().as_millis() as f64,
                                metrics,
                            }
                        } else {
                            HealthCheckResult::Healthy {
                                response_time_ms: start_time.elapsed().as_millis() as f64,
                                metrics,
                            }
                        }
                    }
                    Err(e) => HealthCheckResult::Unhealthy {
                        error: e.to_string(),
                        response_time_ms: None,
                    },
                },
                ComponentType::DiskUsage => match Self::check_disk_usage("/").await {
                    Ok(usage_percentage) => {
                        let mut metrics = HashMap::new();
                        metrics.insert("usage_percentage".to_string(), usage_percentage);

                        if usage_percentage > 95.0 {
                            HealthCheckResult::Unhealthy {
                                error: format!("Disk usage too high: {:.1}%", usage_percentage),
                                response_time_ms: Some(start_time.elapsed().as_millis() as f64),
                            }
                        } else if usage_percentage > 85.0 {
                            HealthCheckResult::Degraded {
                                warning: format!("High disk usage: {:.1}%", usage_percentage),
                                response_time_ms: start_time.elapsed().as_millis() as f64,
                                metrics,
                            }
                        } else {
                            HealthCheckResult::Healthy {
                                response_time_ms: start_time.elapsed().as_millis() as f64,
                                metrics,
                            }
                        }
                    }
                    Err(e) => HealthCheckResult::Unhealthy {
                        error: e.to_string(),
                        response_time_ms: None,
                    },
                },
                ComponentType::NetworkConnectivity => {
                    // Simple network connectivity check (ping localhost)
                    match tokio::process::Command::new("ping")
                        .args(["-c", "1", "127.0.0.1"])
                        .output()
                        .await
                    {
                        Ok(output) => {
                            if output.status.success() {
                                HealthCheckResult::Healthy {
                                    response_time_ms: start_time.elapsed().as_millis() as f64,
                                    metrics: HashMap::new(),
                                }
                            } else {
                                HealthCheckResult::Unhealthy {
                                    error: "Network connectivity check failed".to_string(),
                                    response_time_ms: Some(start_time.elapsed().as_millis() as f64),
                                }
                            }
                        }
                        Err(e) => HealthCheckResult::Unhealthy {
                            error: format!("Network check error: {}", e),
                            response_time_ms: None,
                        },
                    }
                }
                _ => {
                    // TODO: Implement other component types
                    HealthCheckResult::Healthy {
                        response_time_ms: start_time.elapsed().as_millis() as f64,
                        metrics: HashMap::new(),
                    }
                }
            }
        })
        .await;

        match result {
            Ok(check_result) => check_result,
            Err(_) => HealthCheckResult::Unhealthy {
                error: "Health check timed out".to_string(),
                response_time_ms: Some(timeout.as_millis() as f64),
            },
        }
    }

    async fn update_component_state(
        state: &mut ComponentState,
        result: HealthCheckResult,
        config: &HealthMonitoringConfig,
        alert_sender: &mpsc::UnboundedSender<Alert>,
    ) {
        let previous_status = state.status.clone();
        state.last_check = Utc::now();

        match result {
            HealthCheckResult::Healthy {
                response_time_ms,
                metrics,
            } => {
                state.consecutive_failures = 0;
                state.consecutive_successes += 1;
                state.response_time_ms = Some(response_time_ms);
                state.error_message = None;
                state.metrics = metrics;

                if state.consecutive_successes >= config.recovery_threshold {
                    state.status = HealthStatus::Healthy;
                }
            }
            HealthCheckResult::Degraded {
                warning: _,
                response_time_ms,
                metrics,
            } => {
                state.consecutive_failures = 0;
                state.consecutive_successes += 1;
                state.response_time_ms = Some(response_time_ms);
                state.error_message = None;
                state.metrics = metrics;
                state.status = HealthStatus::Degraded;
            }
            HealthCheckResult::Unhealthy {
                error,
                response_time_ms,
            } => {
                state.consecutive_successes = 0;
                state.consecutive_failures += 1;
                state.response_time_ms = response_time_ms;
                state.error_message = Some(error.clone());

                if state.consecutive_failures >= config.unhealthy_threshold {
                    state.status = HealthStatus::Unhealthy;
                }
            }
        }

        // Send alert if status changed
        if previous_status != state.status {
            let severity = match state.status {
                HealthStatus::Healthy => AlertSeverity::Info,
                HealthStatus::Degraded => AlertSeverity::Warning,
                HealthStatus::Unhealthy => AlertSeverity::Critical,
                HealthStatus::Unknown => AlertSeverity::Warning,
            };

            let alert = Alert {
                id: Uuid::new_v4(),
                component: state.name.clone(),
                severity,
                title: format!(
                    "Component {} status changed to {:?}",
                    state.name, state.status
                ),
                description: state.error_message.clone().unwrap_or_default(),
                timestamp: Utc::now(),
                resolved: state.status == HealthStatus::Healthy,
            };

            if let Err(e) = alert_sender.send(alert) {
                error!("Failed to send alert: {}", e);
            }
        }

        debug!(
            "Component {} health check: {:?} ({}ms)",
            state.name,
            state.status,
            state.response_time_ms.unwrap_or(0.0)
        );
    }

    async fn start_alert_processor(&self, mut alert_receiver: mpsc::UnboundedReceiver<Alert>) {
        let config = self.config.clone();

        tokio::spawn(async move {
            while let Some(alert) = alert_receiver.recv().await {
                Self::process_alert(&alert, &config.alert_channels).await;
            }
        });
    }

    async fn process_alert(alert: &Alert, channels: &[AlertChannel]) {
        for channel in channels {
            if !channel.enabled {
                continue;
            }

            match channel.channel_type {
                AlertChannelType::Log => match alert.severity {
                    AlertSeverity::Info => info!(
                        "[ALERT] {}: {} - {}",
                        alert.component, alert.title, alert.description
                    ),
                    AlertSeverity::Warning => warn!(
                        "[ALERT] {}: {} - {}",
                        alert.component, alert.title, alert.description
                    ),
                    AlertSeverity::Critical => error!(
                        "[ALERT] {}: {} - {}",
                        alert.component, alert.title, alert.description
                    ),
                },
                AlertChannelType::Webhook => {
                    // TODO: Implement webhook alert sending
                    debug!("Would send webhook alert: {}", alert.title);
                }
                _ => {
                    // TODO: Implement other alert channel types
                    debug!(
                        "Would send {:?} alert: {}",
                        channel.channel_type, alert.title
                    );
                }
            }
        }
    }

    async fn start_metrics_collection_task(&self) {
        let system_metrics = self.system_metrics.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60)); // Collect metrics every minute

            loop {
                interval.tick().await;

                let mut metrics = system_metrics.write().await;

                // Update system metrics
                if let Ok(memory_usage) = Self::check_memory_usage().await {
                    metrics.memory_usage_percentage = memory_usage;
                }

                if let Ok(cpu_usage) = Self::check_cpu_usage().await {
                    metrics.cpu_usage_percentage = cpu_usage;
                }

                if let Ok(disk_usage) = Self::check_disk_usage("/").await {
                    metrics.disk_usage_percentage = disk_usage;
                }

                // TODO: Update other metrics (connections, requests, etc.)
                debug!(
                    "System metrics updated: mem={:.1}%, cpu={:.1}%, disk={:.1}%",
                    metrics.memory_usage_percentage,
                    metrics.cpu_usage_percentage,
                    metrics.disk_usage_percentage
                );
            }
        });
    }

    async fn start_prometheus_exporter(&self) -> Result<()> {
        let port = self.config.prometheus_port;
        let _component_states = self.component_states.clone();
        let _system_metrics = self.system_metrics.clone();

        tokio::spawn(async move {
            // TODO: Implement proper Prometheus metrics server
            // For now, just log that it would be started
            info!("Prometheus metrics exporter would start on port {}", port);

            // In a real implementation, this would start an HTTP server
            // that serves Prometheus-formatted metrics
        });

        Ok(())
    }

    async fn check_memory_usage() -> Result<f64> {
        // Read /proc/meminfo on Linux
        let meminfo = tokio::fs::read_to_string("/proc/meminfo")
            .await
            .map_err(|e| anyhow!("Failed to read memory info: {}", e))?;

        let mut total_kb = 0u64;
        let mut available_kb = 0u64;

        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                total_kb = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            } else if line.starts_with("MemAvailable:") {
                available_kb = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            }
        }

        if total_kb > 0 {
            let used_kb = total_kb.saturating_sub(available_kb);
            Ok((used_kb as f64 / total_kb as f64) * 100.0)
        } else {
            Err(anyhow!("Could not determine memory usage"))
        }
    }

    async fn check_cpu_usage() -> Result<f64> {
        // Read /proc/loadavg on Linux
        let loadavg = tokio::fs::read_to_string("/proc/loadavg")
            .await
            .map_err(|e| anyhow!("Failed to read load average: {}", e))?;

        let load_1min = loadavg
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);

        // Rough approximation: load average as percentage
        // In a real implementation, you'd want to calculate actual CPU usage
        let cpu_count = num_cpus::get() as f64;
        Ok((load_1min / cpu_count) * 100.0)
    }

    async fn check_disk_usage(path: &str) -> Result<f64> {
        // Use statvfs system call equivalent
        match tokio::process::Command::new("df")
            .args(["-h", path])
            .output()
            .await
        {
            Ok(output) => {
                let output_str = String::from_utf8_lossy(&output.stdout);
                for line in output_str.lines().skip(1) {
                    // Skip header
                    if let Some(usage_str) = line.split_whitespace().nth(4) {
                        if let Some(percentage_str) = usage_str.strip_suffix('%') {
                            if let Ok(percentage) = percentage_str.parse::<f64>() {
                                return Ok(percentage);
                            }
                        }
                    }
                }
                Err(anyhow!("Could not parse disk usage"))
            }
            Err(e) => Err(anyhow!("Failed to check disk usage: {}", e)),
        }
    }

    /// Get overall system health status
    pub async fn get_system_health(&self) -> SystemHealthStatus {
        let states = self.component_states.read().await;
        let metrics = self.system_metrics.read().await;

        let mut healthy_count = 0;
        let mut degraded_count = 0;
        let mut unhealthy_count = 0;
        let mut unknown_count = 0;
        let mut has_critical_failure = false;

        for (name, state) in states.iter() {
            match state.status {
                HealthStatus::Healthy => healthy_count += 1,
                HealthStatus::Degraded => degraded_count += 1,
                HealthStatus::Unhealthy => {
                    unhealthy_count += 1;
                    // Check if this is a critical component
                    if let Some(component_config) =
                        self.config.components.iter().find(|c| c.name == *name)
                    {
                        if component_config.critical {
                            has_critical_failure = true;
                        }
                    }
                }
                HealthStatus::Unknown => unknown_count += 1,
            }
        }

        let overall_status = if has_critical_failure || unhealthy_count > 0 {
            HealthStatus::Unhealthy
        } else if degraded_count > 0 {
            HealthStatus::Degraded
        } else if healthy_count > 0 {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unknown
        };

        SystemHealthStatus {
            overall_status,
            healthy_components: healthy_count,
            degraded_components: degraded_count,
            unhealthy_components: unhealthy_count,
            unknown_components: unknown_count,
            system_metrics: metrics.clone(),
            last_updated: Utc::now(),
        }
    }

    /// Get detailed component health information
    pub async fn get_component_health(
        &self,
        component_name: &str,
    ) -> Option<ComponentHealthStatus> {
        let states = self.component_states.read().await;
        states
            .get(component_name)
            .map(|state| ComponentHealthStatus {
                name: state.name.clone(),
                status: state.status.clone(),
                last_check: state.last_check,
                response_time_ms: state.response_time_ms,
                error_message: state.error_message.clone(),
                metrics: state.metrics.clone(),
                consecutive_failures: state.consecutive_failures,
                consecutive_successes: state.consecutive_successes,
            })
    }

    /// Trigger manual health check for all components
    pub async fn trigger_health_check(&self) -> Result<()> {
        // TODO: Implement manual health check trigger
        info!("Manual health check triggered");
        Ok(())
    }
}

#[derive(Debug, Clone)]
enum HealthCheckResult {
    Healthy {
        response_time_ms: f64,
        metrics: HashMap<String, f64>,
    },
    Degraded {
        warning: String,
        response_time_ms: f64,
        metrics: HashMap<String, f64>,
    },
    Unhealthy {
        error: String,
        response_time_ms: Option<f64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthStatus {
    pub overall_status: HealthStatus,
    pub healthy_components: u32,
    pub degraded_components: u32,
    pub unhealthy_components: u32,
    pub unknown_components: u32,
    pub system_metrics: SystemMetrics,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealthStatus {
    pub name: String,
    pub status: HealthStatus,
    pub last_check: DateTime<Utc>,
    pub response_time_ms: Option<f64>,
    pub error_message: Option<String>,
    pub metrics: HashMap<String, f64>,
    pub consecutive_failures: u32,
    pub consecutive_successes: u32,
}
