//! Security audit logging
//!
//! This module provides comprehensive security audit logging for
//! compliance and forensic analysis.

use super::SecurityContext;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Security audit event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEvent {
    /// Authentication attempt
    AuthenticationAttempt {
        user_id: Option<String>,
        source_ip: std::net::IpAddr,
        success: bool,
        method: String,
        failure_reason: Option<String>,
    },
    /// Authorization check
    AuthorizationCheck {
        user_id: String,
        resource: String,
        action: String,
        allowed: bool,
        reason: Option<String>,
    },
    /// SIP message processing
    SipMessageProcessed {
        source_ip: std::net::IpAddr,
        method: String,
        call_id: Option<String>,
        from_uri: Option<String>,
        to_uri: Option<String>,
        processing_result: String,
    },
    /// Rate limiting triggered
    RateLimitTriggered {
        source_ip: std::net::IpAddr,
        limit_type: String,
        current_rate: u32,
        limit_value: u32,
    },
    /// Security violation detected
    SecurityViolation {
        source_ip: std::net::IpAddr,
        violation_type: String,
        description: String,
        severity: SecurityViolationSeverity,
        data: Option<String>,
    },
    /// Configuration change
    ConfigurationChange {
        user_id: Option<String>,
        source_ip: std::net::IpAddr,
        component: String,
        old_value: Option<String>,
        new_value: String,
        change_reason: Option<String>,
    },
    /// Administrative action
    AdminAction {
        user_id: String,
        source_ip: std::net::IpAddr,
        action: String,
        target: Option<String>,
        result: String,
        details: Option<String>,
    },
}

/// Security violation severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityViolationSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    /// Unique entry ID
    pub id: uuid::Uuid,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Event type
    pub event: AuditEvent,
    /// Security context
    pub context: AuditContext,
    /// Additional metadata
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

/// Audit context (subset of SecurityContext for serialization)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditContext {
    pub source_ip: std::net::IpAddr,
    pub user_agent: Option<String>,
    pub authenticated: bool,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
}

impl From<&SecurityContext> for AuditContext {
    fn from(ctx: &SecurityContext) -> Self {
        Self {
            source_ip: ctx.source_ip,
            user_agent: ctx.user_agent.clone(),
            authenticated: ctx.authenticated,
            user_id: ctx.user_id.clone(),
            session_id: ctx.session_id.clone(),
        }
    }
}

/// Security audit logger
pub struct SecurityAuditLogger {
    /// Log file path
    log_file_path: std::path::PathBuf,
    /// In-memory buffer for recent entries
    recent_entries: Arc<RwLock<Vec<AuditLogEntry>>>,
    /// Maximum entries to keep in memory
    max_memory_entries: usize,
    /// Whether audit logging is enabled
    enabled: bool,
}

impl SecurityAuditLogger {
    /// Create a new security audit logger
    pub fn new(log_file_path: std::path::PathBuf) -> Self {
        Self {
            log_file_path,
            recent_entries: Arc::new(RwLock::new(Vec::new())),
            max_memory_entries: 1000,
            enabled: true,
        }
    }

    /// Log an audit event
    pub async fn log_event(&self, event: AuditEvent, context: &SecurityContext) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let entry = AuditLogEntry {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            event: event.clone(),
            context: context.into(),
            metadata: std::collections::HashMap::new(),
        };

        // Log to structured tracing
        self.log_to_tracing(&entry);

        // Write to audit log file
        self.write_to_file(&entry).await?;

        // Store in memory buffer
        self.store_in_memory(entry).await;

        Ok(())
    }

    /// Log authentication attempt
    pub async fn log_auth_attempt(
        &self,
        context: &SecurityContext,
        user_id: Option<String>,
        method: String,
        success: bool,
        failure_reason: Option<String>,
    ) -> Result<()> {
        let event = AuditEvent::AuthenticationAttempt {
            user_id,
            source_ip: context.source_ip,
            success,
            method,
            failure_reason,
        };

        self.log_event(event, context).await
    }

    /// Log security violation
    pub async fn log_security_violation(
        &self,
        context: &SecurityContext,
        violation_type: String,
        description: String,
        severity: SecurityViolationSeverity,
        data: Option<String>,
    ) -> Result<()> {
        let event = AuditEvent::SecurityViolation {
            source_ip: context.source_ip,
            violation_type,
            description,
            severity,
            data,
        };

        self.log_event(event, context).await
    }

    /// Log SIP message processing
    pub async fn log_sip_message(
        &self,
        context: &SecurityContext,
        method: String,
        call_id: Option<String>,
        from_uri: Option<String>,
        to_uri: Option<String>,
        result: String,
    ) -> Result<()> {
        let event = AuditEvent::SipMessageProcessed {
            source_ip: context.source_ip,
            method,
            call_id,
            from_uri,
            to_uri,
            processing_result: result,
        };

        self.log_event(event, context).await
    }

    /// Get recent audit entries
    pub async fn get_recent_entries(&self, limit: usize) -> Vec<AuditLogEntry> {
        let entries = self.recent_entries.read().await;
        entries.iter().rev().take(limit).cloned().collect()
    }

    /// Search audit entries by criteria
    pub async fn search_entries(
        &self,
        source_ip: Option<std::net::IpAddr>,
        user_id: Option<String>,
        event_type: Option<String>,
        since: Option<chrono::DateTime<chrono::Utc>>,
        limit: usize,
    ) -> Vec<AuditLogEntry> {
        let entries = self.recent_entries.read().await;

        entries
            .iter()
            .filter(|entry| {
                if let Some(ip) = source_ip {
                    if entry.context.source_ip != ip {
                        return false;
                    }
                }

                if let Some(ref uid) = user_id {
                    if entry.context.user_id.as_ref() != Some(uid) {
                        return false;
                    }
                }

                if let Some(since_time) = since {
                    if entry.timestamp < since_time {
                        return false;
                    }
                }

                // TODO: Add event_type filtering based on discriminant
                true
            })
            .take(limit)
            .cloned()
            .collect()
    }

    /// Log to structured tracing
    fn log_to_tracing(&self, entry: &AuditLogEntry) {
        match &entry.event {
            AuditEvent::AuthenticationAttempt {
                success,
                user_id,
                source_ip,
                ..
            } => {
                if *success {
                    info!(
                        event_id = %entry.id,
                        user_id = ?user_id,
                        source_ip = %source_ip,
                        "Authentication successful"
                    );
                } else {
                    warn!(
                        event_id = %entry.id,
                        user_id = ?user_id,
                        source_ip = %source_ip,
                        "Authentication failed"
                    );
                }
            }
            AuditEvent::SecurityViolation {
                source_ip,
                violation_type,
                severity,
                ..
            } => match severity {
                SecurityViolationSeverity::Critical | SecurityViolationSeverity::High => {
                    error!(
                        event_id = %entry.id,
                        source_ip = %source_ip,
                        violation_type = %violation_type,
                        severity = ?severity,
                        "Security violation detected"
                    );
                }
                _ => {
                    warn!(
                        event_id = %entry.id,
                        source_ip = %source_ip,
                        violation_type = %violation_type,
                        severity = ?severity,
                        "Security violation detected"
                    );
                }
            },
            _ => {
                debug!(
                    event_id = %entry.id,
                    event_type = std::any::type_name::<AuditEvent>(),
                    "Security audit event"
                );
            }
        }
    }

    /// Write entry to audit log file
    async fn write_to_file(&self, entry: &AuditLogEntry) -> Result<()> {
        let log_line = serde_json::to_string(entry)? + "\n";

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file_path)
            .await?;

        file.write_all(log_line.as_bytes()).await?;
        file.flush().await?;

        Ok(())
    }

    /// Store entry in memory buffer
    async fn store_in_memory(&self, entry: AuditLogEntry) {
        let mut entries = self.recent_entries.write().await;
        entries.push(entry);

        // Trim to max size
        if entries.len() > self.max_memory_entries {
            entries.remove(0);
        }
    }

    /// Enable/disable audit logging
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled {
            info!("Security audit logging enabled");
        } else {
            warn!("Security audit logging disabled");
        }
    }
}

/// Global audit logger instance
static mut AUDIT_LOGGER: Option<Arc<SecurityAuditLogger>> = None;
static AUDIT_LOGGER_INIT: std::sync::Once = std::sync::Once::new();

/// Initialize audit logging
pub fn initialize_audit_logging() -> Result<()> {
    AUDIT_LOGGER_INIT.call_once(|| {
        let log_path = std::env::var("SECURITY_AUDIT_LOG_PATH")
            .unwrap_or_else(|_| "/var/log/redfire-switch/security-audit.log".to_string());

        let logger = Arc::new(SecurityAuditLogger::new(log_path.into()));

        unsafe {
            AUDIT_LOGGER = Some(logger);
        }

        info!("Security audit logging initialized");
    });

    Ok(())
}

/// Get global audit logger
#[allow(clippy::missing_safety_doc)]
pub fn get_audit_logger() -> Option<Arc<SecurityAuditLogger>> {
    #[allow(static_mut_refs)]
    unsafe {
        AUDIT_LOGGER.clone()
    }
}

/// Convenience function to log audit event
pub async fn audit_log(event: AuditEvent, context: &SecurityContext) -> Result<()> {
    if let Some(logger) = get_audit_logger() {
        logger.log_event(event, context).await?;
    }
    Ok(())
}

/// Macro for easy audit logging
#[macro_export]
macro_rules! audit_log {
    ($event:expr, $context:expr) => {
        if let Err(e) = crate::security::audit::audit_log($event, $context).await {
            tracing::error!("Failed to write audit log: {}", e);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_audit_logging() {
        let temp_dir = tempdir().unwrap();
        let log_file = temp_dir.path().join("test-audit.log");

        let logger = SecurityAuditLogger::new(log_file.clone());
        let context = SecurityContext::new("192.168.1.1".parse().unwrap());

        let event = AuditEvent::AuthenticationAttempt {
            user_id: Some("test_user".to_string()),
            source_ip: context.source_ip,
            success: true,
            method: "password".to_string(),
            failure_reason: None,
        };

        logger.log_event(event, &context).await.unwrap();

        // Check that entry was stored in memory
        let recent = logger.get_recent_entries(10).await;
        assert_eq!(recent.len(), 1);

        // Check that file was written
        assert!(log_file.exists());
    }
}
