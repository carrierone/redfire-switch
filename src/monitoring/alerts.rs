//! Alert management system
//!
//! This module provides alerting capabilities for monitoring thresholds,
//! anomalies, and system health issues.

use super::{SystemMetricsSnapshot, HealthStatus};
use super::NotificationEndpoint;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Alert severity levels
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    /// Informational alert
    Info,
    /// Warning - needs attention
    Warning,
    /// Critical - requires immediate action
    Critical,
    /// Emergency - service disruption
    Emergency,
}

/// Alert status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertStatus {
    /// Alert is active/firing
    Firing,
    /// Alert condition resolved
    Resolved,
    /// Alert is acknowledged but not resolved
    Acknowledged,
    /// Alert is silenced
    Silenced,
}

/// Alert definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    /// Unique alert ID
    pub id: String,
    /// Alert name
    pub name: String,
    /// Alert description
    pub description: String,
    /// Severity level
    pub severity: AlertSeverity,
    /// Current status
    pub status: AlertStatus,
    /// When the alert was triggered
    pub triggered_at: chrono::DateTime<chrono::Utc>,
    /// When the alert was resolved (if applicable)
    pub resolved_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Alert labels/tags
    pub labels: HashMap<String, String>,
    /// Alert annotations
    pub annotations: HashMap<String, String>,
    /// Metric value that triggered the alert
    pub trigger_value: f64,
    /// Threshold that was exceeded
    pub threshold: f64,
}

/// Alert rule definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    /// Rule ID
    pub id: String,
    /// Rule name
    pub name: String,
    /// Rule description
    pub description: String,
    /// Metric to monitor
    pub metric_path: String,
    /// Comparison operator
    pub operator: ComparisonOperator,
    /// Threshold value
    pub threshold: f64,
    /// Duration the condition must be true before alerting
    pub duration_seconds: u64,
    /// Alert severity
    pub severity: AlertSeverity,
    /// Whether rule is enabled
    pub enabled: bool,
    /// Labels to attach to alerts
    pub labels: HashMap<String, String>,
    /// Annotations to attach to alerts
    pub annotations: HashMap<String, String>,
}

/// Comparison operators for alert rules
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComparisonOperator {
    /// Greater than
    GreaterThan,
    /// Greater than or equal
    GreaterThanOrEqual,
    /// Less than
    LessThan,
    /// Less than or equal
    LessThanOrEqual,
    /// Equal
    Equal,
    /// Not equal
    NotEqual,
}

impl ComparisonOperator {
    /// Evaluate the comparison
    pub fn evaluate(&self, value: f64, threshold: f64) -> bool {
        match self {
            Self::GreaterThan => value > threshold,
            Self::GreaterThanOrEqual => value >= threshold,
            Self::LessThan => value < threshold,
            Self::LessThanOrEqual => value <= threshold,
            Self::Equal => (value - threshold).abs() < f64::EPSILON,
            Self::NotEqual => (value - threshold).abs() >= f64::EPSILON,
        }
    }
}

/// Alert manager
pub struct AlertManager {
    /// Evaluation interval
    evaluation_interval: std::time::Duration,
    /// Active alerts
    active_alerts: Arc<RwLock<HashMap<String, Alert>>>,
    /// Alert rules
    rules: Arc<RwLock<Vec<AlertRule>>>,
    /// Notification endpoints
    notification_endpoints: Vec<NotificationEndpoint>,
    /// Alert history
    alert_history: Arc<RwLock<Vec<Alert>>>,
}

impl AlertManager {
    /// Create new alert manager
    pub fn new(
        evaluation_interval_seconds: u64,
        notification_endpoints: Vec<NotificationEndpoint>,
    ) -> Result<Self> {
        let default_rules = Self::create_default_rules();

        Ok(Self {
            evaluation_interval: std::time::Duration::from_secs(evaluation_interval_seconds),
            active_alerts: Arc::new(RwLock::new(HashMap::new())),
            rules: Arc::new(RwLock::new(default_rules)),
            notification_endpoints,
            alert_history: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Create default alert rules
    fn create_default_rules() -> Vec<AlertRule> {
        vec![
            // High CPU usage
            AlertRule {
                id: "high_cpu".to_string(),
                name: "High CPU Usage".to_string(),
                description: "CPU usage above 80%".to_string(),
                metric_path: "system.cpu_usage_percent".to_string(),
                operator: ComparisonOperator::GreaterThan,
                threshold: 80.0,
                duration_seconds: 60,
                severity: AlertSeverity::Warning,
                enabled: true,
                labels: HashMap::from([("type".to_string(), "system".to_string())]),
                annotations: HashMap::new(),
            },
            // High memory usage
            AlertRule {
                id: "high_memory".to_string(),
                name: "High Memory Usage".to_string(),
                description: "Memory usage above 85%".to_string(),
                metric_path: "system.memory_usage_mb".to_string(),
                operator: ComparisonOperator::GreaterThan,
                threshold: 1700.0, // ~85% of 2GB
                duration_seconds: 60,
                severity: AlertSeverity::Warning,
                enabled: true,
                labels: HashMap::from([("type".to_string(), "system".to_string())]),
                annotations: HashMap::new(),
            },
            // Low call success rate
            AlertRule {
                id: "low_call_success_rate".to_string(),
                name: "Low Call Success Rate".to_string(),
                description: "Call success rate below 95%".to_string(),
                metric_path: "business.call_success_rate".to_string(),
                operator: ComparisonOperator::LessThan,
                threshold: 95.0,
                duration_seconds: 300,
                severity: AlertSeverity::Critical,
                enabled: true,
                labels: HashMap::from([("type".to_string(), "business".to_string())]),
                annotations: HashMap::new(),
            },
            // High error rate
            AlertRule {
                id: "high_sip_error_rate".to_string(),
                name: "High SIP Error Rate".to_string(),
                description: "SIP 5xx responses above 5%".to_string(),
                metric_path: "sip.response_codes.500".to_string(),
                operator: ComparisonOperator::GreaterThan,
                threshold: 50.0, // If total messages is 1000, 50 = 5%
                duration_seconds: 60,
                severity: AlertSeverity::Warning,
                enabled: true,
                labels: HashMap::from([("type".to_string(), "sip".to_string())]),
                annotations: HashMap::new(),
            },
            // Security violations
            AlertRule {
                id: "security_violations".to_string(),
                name: "Security Violations Detected".to_string(),
                description: "Security violations detected".to_string(),
                metric_path: "security.security_violations".to_string(),
                operator: ComparisonOperator::GreaterThan,
                threshold: 10.0,
                duration_seconds: 60,
                severity: AlertSeverity::Critical,
                enabled: true,
                labels: HashMap::from([("type".to_string(), "security".to_string())]),
                annotations: HashMap::new(),
            },
        ]
    }

    /// Evaluate all alert rules against current metrics
    pub async fn evaluate_alerts(
        &self,
        metrics: &SystemMetricsSnapshot,
        _health_status: &HashMap<String, HealthStatus>,
    ) -> Result<Vec<Alert>> {
        let rules = self.rules.read().await;
        let mut triggered_alerts = Vec::new();
        let mut active_alerts = self.active_alerts.write().await;

        for rule in rules.iter().filter(|r| r.enabled) {
            // Extract metric value
            let metric_value = self.extract_metric_value(metrics, &rule.metric_path);

            // Evaluate rule condition
            if rule.operator.evaluate(metric_value, rule.threshold) {
                // Check if alert already exists
                if !active_alerts.contains_key(&rule.id) {
                    let alert = Alert {
                        id: rule.id.clone(),
                        name: rule.name.clone(),
                        description: rule.description.clone(),
                        severity: rule.severity.clone(),
                        status: AlertStatus::Firing,
                        triggered_at: chrono::Utc::now(),
                        resolved_at: None,
                        labels: rule.labels.clone(),
                        annotations: rule.annotations.clone(),
                        trigger_value: metric_value,
                        threshold: rule.threshold,
                    };

                    info!(
                        "Alert triggered: {} (value: {}, threshold: {})",
                        alert.name, metric_value, rule.threshold
                    );

                    active_alerts.insert(rule.id.clone(), alert.clone());
                    self.add_to_history(alert.clone()).await;
                    triggered_alerts.push(alert);
                }
            } else {
                // Resolve alert if it was active
                if let Some(mut alert) = active_alerts.remove(&rule.id) {
                    alert.status = AlertStatus::Resolved;
                    alert.resolved_at = Some(chrono::Utc::now());

                    info!("Alert resolved: {}", alert.name);
                    self.add_to_history(alert).await;
                }
            }
        }

        Ok(triggered_alerts)
    }

    /// Extract metric value from metrics snapshot
    fn extract_metric_value(&self, metrics: &SystemMetricsSnapshot, path: &str) -> f64 {
        // Parse metric path and extract value
        // Format: "category.field" or "category.field.subfield"
        let parts: Vec<&str> = path.split('.').collect();

        match parts.as_slice() {
            ["system", "cpu_usage_percent"] => metrics.system.cpu_usage_percent,
            ["system", "memory_usage_mb"] => metrics.system.memory_usage_mb as f64,
            ["business", "call_success_rate"] => metrics.business.call_success_rate,
            ["sip", "response_codes", code] => {
                let code_num: u16 = code.parse().unwrap_or(0);
                *metrics.sip.response_codes.get(&code_num).unwrap_or(&0) as f64
            }
            ["security", "security_violations"] => metrics.security.security_violations as f64,
            _ => {
                warn!("Unknown metric path: {}", path);
                0.0
            }
        }
    }

    /// Send alert notification
    pub async fn send_alert_notification(&self, alert: &Alert) -> Result<()> {
        for endpoint in &self.notification_endpoints {
            if !endpoint.enabled {
                continue;
            }

            match &endpoint.endpoint_type {
                super::NotificationEndpointType::Console => {
                    self.send_console_notification(alert).await?;
                }
                super::NotificationEndpointType::Email => {
                    debug!("Email notification not yet implemented");
                }
                super::NotificationEndpointType::Slack => {
                    debug!("Slack notification not yet implemented");
                }
                super::NotificationEndpointType::Webhook => {
                    debug!("Webhook notification not yet implemented");
                }
                super::NotificationEndpointType::PagerDuty => {
                    debug!("PagerDuty notification not yet implemented");
                }
                super::NotificationEndpointType::Sms => {
                    debug!("SMS notification not yet implemented");
                }
            }
        }

        Ok(())
    }

    /// Send console notification
    async fn send_console_notification(&self, alert: &Alert) -> Result<()> {
        let severity_str = match alert.severity {
            AlertSeverity::Info => "INFO",
            AlertSeverity::Warning => "WARNING",
            AlertSeverity::Critical => "CRITICAL",
            AlertSeverity::Emergency => "EMERGENCY",
        };

        warn!(
            "[ALERT-{}] {} - {} (value: {}, threshold: {})",
            severity_str, alert.name, alert.description, alert.trigger_value, alert.threshold
        );

        Ok(())
    }

    /// Add alert to history
    async fn add_to_history(&self, alert: Alert) {
        let mut history = self.alert_history.write().await;
        history.push(alert);

        // Keep only last 1000 alerts
        if history.len() > 1000 {
            history.remove(0);
        }
    }

    /// Get active alerts
    pub async fn get_active_alerts(&self) -> Vec<Alert> {
        let active = self.active_alerts.read().await;
        active.values().cloned().collect()
    }

    /// Get alert history
    pub async fn get_alert_history(&self, limit: usize) -> Vec<Alert> {
        let history = self.alert_history.read().await;
        history.iter().rev().take(limit).cloned().collect()
    }

    /// Add custom alert rule
    pub async fn add_rule(&self, rule: AlertRule) -> Result<()> {
        let mut rules = self.rules.write().await;
        rules.push(rule);
        Ok(())
    }

    /// Get all alert rules
    pub async fn get_rules(&self) -> Vec<AlertRule> {
        let rules = self.rules.read().await;
        rules.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comparison_operators() {
        assert!(ComparisonOperator::GreaterThan.evaluate(10.0, 5.0));
        assert!(!ComparisonOperator::GreaterThan.evaluate(5.0, 10.0));

        assert!(ComparisonOperator::LessThan.evaluate(5.0, 10.0));
        assert!(!ComparisonOperator::LessThan.evaluate(10.0, 5.0));
    }

    #[tokio::test]
    async fn test_alert_manager_creation() {
        let manager = AlertManager::new(30, vec![]).unwrap();

        let rules = manager.get_rules().await;
        assert!(rules.len() > 0);

        let active = manager.get_active_alerts().await;
        assert_eq!(active.len(), 0);
    }
}
