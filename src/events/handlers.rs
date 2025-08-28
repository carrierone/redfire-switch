//! Default event handlers for common telecommunications operations

use super::{EventHandler, EventType, TelecomEvent};
use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Logs all events to structured log output
pub struct LoggingHandler {
    name: String,
    log_level: tracing::Level,
    event_count: AtomicU64,
}

impl LoggingHandler {
    pub fn new(name: String, log_level: tracing::Level) -> Self {
        Self {
            name,
            log_level,
            event_count: AtomicU64::new(0),
        }
    }

    pub fn get_event_count(&self) -> u64 {
        self.event_count.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl EventHandler for LoggingHandler {
    async fn handle_event(&self, event: &TelecomEvent) -> Result<()> {
        self.event_count.fetch_add(1, Ordering::Relaxed);

        match self.log_level {
            tracing::Level::ERROR => error!("Event: {}", serde_json::to_string(event)?),
            tracing::Level::WARN => warn!("Event: {}", serde_json::to_string(event)?),
            tracing::Level::INFO => info!("Event: {}", serde_json::to_string(event)?),
            tracing::Level::DEBUG => debug!("Event: {}", serde_json::to_string(event)?),
            tracing::Level::TRACE => tracing::trace!("Event: {}", serde_json::to_string(event)?),
        }

        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn interested_events(&self) -> Vec<EventType> {
        vec![EventType::All]
    }
}

/// Writes events to a JSON file for persistence and analysis
pub struct FileHandler {
    name: String,
    file_path: PathBuf,
    event_types: Vec<EventType>,
    event_count: AtomicU64,
    file_handle: Arc<RwLock<Option<tokio::fs::File>>>,
}

impl FileHandler {
    pub async fn new(name: String, file_path: PathBuf, event_types: Vec<EventType>) -> Result<Self> {
        // Create parent directories if they don't exist
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await
                .with_context(|| format!("Failed to create directory: {:?}", parent))?;
        }

        let handler = Self {
            name,
            file_path,
            event_types,
            event_count: AtomicU64::new(0),
            file_handle: Arc::new(RwLock::new(None)),
        };

        // Initialize file handle
        handler.ensure_file_handle().await?;

        Ok(handler)
    }

    async fn ensure_file_handle(&self) -> Result<()> {
        let mut handle_guard = self.file_handle.write().await;
        
        if handle_guard.is_none() {
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.file_path)
                .await
                .with_context(|| format!("Failed to open file: {:?}", self.file_path))?;
            
            *handle_guard = Some(file);
        }

        Ok(())
    }

    pub fn get_event_count(&self) -> u64 {
        self.event_count.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl EventHandler for FileHandler {
    async fn handle_event(&self, event: &TelecomEvent) -> Result<()> {
        self.event_count.fetch_add(1, Ordering::Relaxed);

        self.ensure_file_handle().await?;

        let event_json = serde_json::to_string(event)
            .with_context(|| "Failed to serialize event to JSON")?;

        let mut handle_guard = self.file_handle.write().await;
        if let Some(file) = handle_guard.as_mut() {
            file.write_all(event_json.as_bytes()).await
                .with_context(|| "Failed to write event to file")?;
            file.write_all(b"\n").await
                .with_context(|| "Failed to write newline to file")?;
            file.flush().await
                .with_context(|| "Failed to flush file")?;
        }

        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn interested_events(&self) -> Vec<EventType> {
        self.event_types.clone()
    }

    async fn health_check(&self) -> Result<()> {
        // Check if file is writable
        self.ensure_file_handle().await?;
        Ok(())
    }
}

/// Collects metrics and statistics from events
pub struct MetricsHandler {
    name: String,
    event_types: Vec<EventType>,
    metrics: Arc<RwLock<TelecomMetrics>>,
}

#[derive(Debug, Default, Clone)]
pub struct TelecomMetrics {
    pub total_calls_initiated: u64,
    pub total_calls_connected: u64,
    pub total_calls_terminated: u64,
    pub total_routes_advanced: u64,
    pub fraud_alerts_generated: u64,
    pub average_call_setup_time_ms: f64,
    pub average_call_duration_seconds: f64,
    pub calls_by_trunk: HashMap<i32, u64>,
    pub calls_by_response_code: HashMap<u16, u64>,
    pub health_status_by_service: HashMap<String, String>,
    pub last_updated: Option<DateTime<Utc>>,
}

impl MetricsHandler {
    pub fn new(name: String, event_types: Vec<EventType>) -> Self {
        Self {
            name,
            event_types,
            metrics: Arc::new(RwLock::new(TelecomMetrics::default())),
        }
    }

    pub async fn get_metrics(&self) -> TelecomMetrics {
        self.metrics.read().await.clone()
    }

    pub async fn reset_metrics(&self) {
        let mut metrics = self.metrics.write().await;
        *metrics = TelecomMetrics::default();
    }
}

#[async_trait]
impl EventHandler for MetricsHandler {
    async fn handle_event(&self, event: &TelecomEvent) -> Result<()> {
        let mut metrics = self.metrics.write().await;
        metrics.last_updated = Some(Utc::now());

        match event {
            TelecomEvent::CallInitiated(e) => {
                metrics.total_calls_initiated += 1;
                if let Some(trunk_id) = e.trunk_id {
                    *metrics.calls_by_trunk.entry(trunk_id).or_insert(0) += 1;
                }
            }
            TelecomEvent::CallConnected(e) => {
                metrics.total_calls_connected += 1;
                
                // Update average call setup time
                let setup_time = e.connection_time_ms as f64;
                if metrics.total_calls_connected == 1 {
                    metrics.average_call_setup_time_ms = setup_time;
                } else {
                    let count = metrics.total_calls_connected as f64;
                    metrics.average_call_setup_time_ms = 
                        (metrics.average_call_setup_time_ms * (count - 1.0) + setup_time) / count;
                }
            }
            TelecomEvent::CallTerminated(e) => {
                metrics.total_calls_terminated += 1;
                
                // Update average call duration
                let duration = e.call_duration_seconds as f64;
                if metrics.total_calls_terminated == 1 {
                    metrics.average_call_duration_seconds = duration;
                } else {
                    let count = metrics.total_calls_terminated as f64;
                    metrics.average_call_duration_seconds = 
                        (metrics.average_call_duration_seconds * (count - 1.0) + duration) / count;
                }
                
                // Track response codes
                *metrics.calls_by_response_code.entry(e.final_response_code).or_insert(0) += 1;
            }
            TelecomEvent::RouteAdvanced(_) => {
                metrics.total_routes_advanced += 1;
            }
            TelecomEvent::FraudDetected(_) => {
                metrics.fraud_alerts_generated += 1;
            }
            TelecomEvent::HealthStatus(e) => {
                metrics.health_status_by_service.insert(
                    e.service_name.clone(),
                    format!("{:?}", e.status)
                );
            }
            _ => {
                // Other events don't update metrics
            }
        }

        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn interested_events(&self) -> Vec<EventType> {
        self.event_types.clone()
    }
}

/// Monitors for fraud patterns and generates alerts
pub struct FraudMonitorHandler {
    name: String,
    call_patterns: Arc<RwLock<HashMap<String, CallPattern>>>,
    alert_threshold: u32,
}

#[derive(Debug, Clone)]
struct CallPattern {
    calling_number: String,
    call_count: u32,
    unique_destinations: std::collections::HashSet<String>,
    first_call_time: DateTime<Utc>,
    last_call_time: DateTime<Utc>,
}

impl FraudMonitorHandler {
    pub fn new(name: String, alert_threshold: u32) -> Self {
        Self {
            name,
            call_patterns: Arc::new(RwLock::new(HashMap::new())),
            alert_threshold,
        }
    }

    async fn analyze_call_pattern(&self, calling_number: &str, called_number: &str) -> Option<String> {
        let mut patterns = self.call_patterns.write().await;
        let now = Utc::now();

        let pattern = patterns.entry(calling_number.to_string()).or_insert_with(|| {
            let mut destinations = std::collections::HashSet::new();
            destinations.insert(called_number.to_string());
            
            CallPattern {
                calling_number: calling_number.to_string(),
                call_count: 0,
                unique_destinations: destinations,
                first_call_time: now,
                last_call_time: now,
            }
        });

        pattern.call_count += 1;
        pattern.unique_destinations.insert(called_number.to_string());
        pattern.last_call_time = now;

        // Check for suspicious patterns
        if pattern.call_count > self.alert_threshold {
            let time_span = (now - pattern.first_call_time).num_minutes();
            if time_span < 60 && pattern.unique_destinations.len() > 10 {
                return Some(format!(
                    "High volume fraud: {} calls to {} destinations in {} minutes",
                    pattern.call_count,
                    pattern.unique_destinations.len(),
                    time_span
                ));
            }
        }

        None
    }

    pub async fn get_call_patterns(&self) -> HashMap<String, CallPattern> {
        self.call_patterns.read().await.clone()
    }

    pub async fn clear_patterns(&self) {
        let mut patterns = self.call_patterns.write().await;
        patterns.clear();
    }
}

#[async_trait]
impl EventHandler for FraudMonitorHandler {
    async fn handle_event(&self, event: &TelecomEvent) -> Result<()> {
        if let TelecomEvent::CallInitiated(call_event) = event {
            if let Some(alert_reason) = self.analyze_call_pattern(
                &call_event.calling_number,
                &call_event.called_number
            ).await {
                warn!(
                    "Fraud alert for call {}: {} from {}",
                    call_event.call_id,
                    alert_reason,
                    call_event.calling_number
                );

                // In a real system, you would publish a FraudDetected event here
                // or send alerts to external monitoring systems
            }
        }

        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn interested_events(&self) -> Vec<EventType> {
        vec![EventType::CallInitiated]
    }
}

/// Handles health status events and maintains service health state
pub struct HealthStatusHandler {
    name: String,
    service_health: Arc<RwLock<HashMap<String, ServiceHealth>>>,
}

#[derive(Debug, Clone)]
pub struct ServiceHealth {
    pub service_name: String,
    pub instance_id: String,
    pub status: String,
    pub last_update: DateTime<Utc>,
    pub metrics: HashMap<String, f64>,
    pub consecutive_failures: u32,
}

impl HealthStatusHandler {
    pub fn new(name: String) -> Self {
        Self {
            name,
            service_health: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_service_health(&self) -> HashMap<String, ServiceHealth> {
        self.service_health.read().await.clone()
    }

    pub async fn get_unhealthy_services(&self) -> Vec<ServiceHealth> {
        let health_map = self.service_health.read().await;
        health_map
            .values()
            .filter(|health| health.status != "Healthy")
            .cloned()
            .collect()
    }
}

#[async_trait]
impl EventHandler for HealthStatusHandler {
    async fn handle_event(&self, event: &TelecomEvent) -> Result<()> {
        if let TelecomEvent::HealthStatus(health_event) = event {
            let mut health_map = self.service_health.write().await;
            
            let key = format!("{}:{}", health_event.service_name, health_event.instance_id);
            let status_str = format!("{:?}", health_event.status);

            let service_health = health_map.entry(key).or_insert_with(|| ServiceHealth {
                service_name: health_event.service_name.clone(),
                instance_id: health_event.instance_id.clone(),
                status: status_str.clone(),
                last_update: health_event.timestamp,
                metrics: HashMap::new(),
                consecutive_failures: 0,
            });

            // Update health information
            let was_healthy = service_health.status == "Healthy";
            service_health.status = status_str.clone();
            service_health.last_update = health_event.timestamp;
            service_health.metrics = health_event.metrics.clone();

            // Track consecutive failures
            if status_str == "Healthy" {
                service_health.consecutive_failures = 0;
            } else if was_healthy {
                service_health.consecutive_failures = 1;
            } else {
                service_health.consecutive_failures += 1;
            }

            // Log significant health changes
            if !was_healthy && status_str == "Healthy" {
                info!(
                    "Service {}:{} recovered to healthy status",
                    health_event.service_name,
                    health_event.instance_id
                );
            } else if was_healthy && status_str != "Healthy" {
                warn!(
                    "Service {}:{} became unhealthy: {}",
                    health_event.service_name,
                    health_event.instance_id,
                    status_str
                );
            }
        }

        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn interested_events(&self) -> Vec<EventType> {
        vec![EventType::HealthStatus]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::TelecomEvent;
    use std::net::{IpAddr, Ipv4Addr};
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_logging_handler() {
        let handler = LoggingHandler::new("test-logger".to_string(), tracing::Level::DEBUG);
        
        let event = TelecomEvent::call_initiated(
            "test-call".to_string(),
            "test-session".to_string(),
            "1234567890".to_string(),
            "0987654321".to_string(),
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        );

        assert!(handler.handle_event(&event).await.is_ok());
        assert_eq!(handler.get_event_count(), 1);
    }

    #[tokio::test]
    async fn test_file_handler() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("test_events.jsonl");

        let handler = FileHandler::new(
            "test-file-handler".to_string(),
            file_path.clone(),
            vec![EventType::CallInitiated],
        ).await.expect("Failed to create file handler");

        let event = TelecomEvent::call_initiated(
            "test-call".to_string(),
            "test-session".to_string(),
            "1234567890".to_string(),
            "0987654321".to_string(),
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        );

        assert!(handler.handle_event(&event).await.is_ok());
        assert_eq!(handler.get_event_count(), 1);

        // Verify file was created and contains event
        let file_contents = fs::read_to_string(&file_path).await
            .expect("Failed to read file");
        assert!(file_contents.contains("test-call"));
    }

    #[tokio::test]
    async fn test_metrics_handler() {
        let handler = MetricsHandler::new(
            "test-metrics".to_string(),
            vec![EventType::CallInitiated, EventType::CallTerminated],
        );

        let init_event = TelecomEvent::call_initiated(
            "test-call".to_string(),
            "test-session".to_string(),
            "1234567890".to_string(),
            "0987654321".to_string(),
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        );

        assert!(handler.handle_event(&init_event).await.is_ok());

        let metrics = handler.get_metrics().await;
        assert_eq!(metrics.total_calls_initiated, 1);
        assert!(metrics.last_updated.is_some());
    }

    #[tokio::test]
    async fn test_fraud_monitor_handler() {
        let handler = FraudMonitorHandler::new("test-fraud".to_string(), 5);

        // Simulate multiple calls from same number
        for i in 0..10 {
            let event = TelecomEvent::call_initiated(
                format!("call-{}", i),
                format!("session-{}", i),
                "1234567890".to_string(), // Same calling number
                format!("098765432{}", i), // Different called numbers
                IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            );

            assert!(handler.handle_event(&event).await.is_ok());
        }

        let patterns = handler.get_call_patterns().await;
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns.get("1234567890").expect("Pattern should exist").call_count, 10);
    }
}