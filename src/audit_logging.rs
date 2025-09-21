//! Audit Logging System for Compliance and Security
//! Comprehensive audit trail for all system operations and security events

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    pub enabled: bool,
    pub log_level: AuditLogLevel,
    pub storage_config: AuditStorageConfig,
    pub retention_policy: RetentionPolicy,
    pub compliance_standards: Vec<ComplianceStandard>,
    pub sensitive_fields: Vec<String>,
    pub batch_size: usize,
    pub flush_interval_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditLogLevel {
    Essential, // Only critical operations (auth, config changes, security events)
    Standard,  // Standard operations + call control
    Detailed,  // All operations including SIP messages
    Forensic,  // Maximum detail for forensic analysis
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditStorageConfig {
    pub primary_storage: StorageBackend,
    pub backup_storage: Option<StorageBackend>,
    pub encryption_enabled: bool,
    pub compression_enabled: bool,
    pub file_rotation_size_mb: u64,
    pub syslog_facility: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageBackend {
    File {
        base_path: String,
        file_prefix: String,
    },
    Database {
        connection_string: String,
        table_name: String,
    },
    Syslog {
        server: String,
        facility: String,
    },
    ElasticSearch {
        endpoints: Vec<String>,
        index_pattern: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub hot_storage_days: u32,
    pub warm_storage_days: u32,
    pub cold_storage_days: u32,
    pub archive_storage_days: u32,
    pub auto_delete_after_days: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComplianceStandard {
    Sox,       // Sarbanes-Oxley
    Pci,       // PCI DSS
    Hipaa,     // HIPAA
    Gdpr,      // GDPR
    Iso27001,  // ISO 27001
    Nist,      // NIST Cybersecurity Framework
    Calea,     // CALEA (Communications Assistance for Law Enforcement Act)
    FccPart68, // FCC Part 68
    TracedAct, // TRACED Act
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            log_level: AuditLogLevel::Standard,
            storage_config: AuditStorageConfig {
                primary_storage: StorageBackend::File {
                    base_path: "/var/log/redfire/audit".to_string(),
                    file_prefix: "audit".to_string(),
                },
                backup_storage: Some(StorageBackend::Database {
                    connection_string: "postgresql://redfire:password@localhost/redfire_audit"
                        .to_string(),
                    table_name: "audit_log".to_string(),
                }),
                encryption_enabled: true,
                compression_enabled: true,
                file_rotation_size_mb: 100,
                syslog_facility: Some("local0".to_string()),
            },
            retention_policy: RetentionPolicy {
                hot_storage_days: 30,
                warm_storage_days: 90,
                cold_storage_days: 365,
                archive_storage_days: 2555, // 7 years
                auto_delete_after_days: Some(2555),
            },
            compliance_standards: vec![
                ComplianceStandard::Sox,
                ComplianceStandard::Calea,
                ComplianceStandard::TracedAct,
            ],
            sensitive_fields: vec![
                "password".to_string(),
                "private_key".to_string(),
                "secret".to_string(),
                "token".to_string(),
                "credit_card".to_string(),
                "ssn".to_string(),
            ],
            batch_size: 100,
            flush_interval_seconds: 30,
        }
    }
}

pub struct AuditLoggingService {
    config: AuditConfig,
    log_buffer: Arc<Mutex<VecDeque<AuditLogEntry>>>,
    sender: mpsc::UnboundedSender<AuditLogEntry>,
    database_service: Option<Arc<crate::database::DatabaseService>>,
    statistics: Arc<Mutex<AuditStatistics>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub event_type: AuditEventType,
    pub severity: AuditSeverity,
    pub user_id: Option<String>,
    pub session_id: Option<Uuid>,
    pub source_ip: Option<IpAddr>,
    pub user_agent: Option<String>,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub action: String,
    pub description: String,
    pub details: serde_json::Value,
    pub outcome: AuditOutcome,
    pub error_message: Option<String>,
    pub compliance_tags: Vec<ComplianceStandard>,
    pub retention_category: RetentionCategory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEventType {
    Authentication,
    Authorization,
    Configuration,
    CallControl,
    SipMessage,
    SecurityEvent,
    SystemOperation,
    DataAccess,
    UserManagement,
    TrunkManagement,
    RoutingChange,
    BillingOperation,
    MaintenanceOperation,
    EmergencyAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditSeverity {
    Info,
    Warning,
    Error,
    Critical,
    Security,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditOutcome {
    Success,
    Failure,
    Partial,
    Pending,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RetentionCategory {
    Hot,     // Immediate access
    Warm,    // Frequent access
    Cold,    // Infrequent access
    Archive, // Long-term retention
}

#[derive(Debug, Default)]
struct AuditStatistics {
    total_events_logged: u64,
    events_by_type: std::collections::HashMap<String, u64>,
    events_by_severity: std::collections::HashMap<String, u64>,
    failed_writes: u64,
    storage_errors: u64,
    compliance_events: u64,
    retention_actions: u64,
}

impl AuditLoggingService {
    pub async fn new(
        config: AuditConfig,
        database_service: Option<Arc<crate::database::DatabaseService>>,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::unbounded_channel();

        let service = Self {
            config: config.clone(),
            log_buffer: Arc::new(Mutex::new(VecDeque::new())),
            sender,
            database_service,
            statistics: Arc::new(Mutex::new(AuditStatistics::default())),
        };

        // Start background processing task
        if config.enabled {
            service.start_background_processor(receiver).await;
            service.start_retention_manager().await;
        }

        info!(
            "Audit logging service initialized with {:?} compliance standards",
            config.compliance_standards
        );
        Ok(service)
    }

    /// Log an audit event
    pub async fn log_event(
        &self,
        event_type: AuditEventType,
        action: String,
        description: String,
    ) -> Result<()> {
        self.log_detailed_event(AuditLogEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type,
            severity: AuditSeverity::Info,
            user_id: None,
            session_id: None,
            source_ip: None,
            user_agent: None,
            resource_type: None,
            resource_id: None,
            action,
            description,
            details: serde_json::json!({}),
            outcome: AuditOutcome::Success,
            error_message: None,
            compliance_tags: vec![],
            retention_category: RetentionCategory::Hot,
        })
        .await
    }

    /// Log a detailed audit event with full context
    pub async fn log_detailed_event(&self, mut entry: AuditLogEntry) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        // Apply compliance tagging based on event type
        entry.compliance_tags = self.determine_compliance_tags(&entry);

        // Apply retention categorization
        entry.retention_category = self.determine_retention_category(&entry);

        // Sanitize sensitive data
        entry.details = self.sanitize_sensitive_data(entry.details);

        // Check if this event should be logged based on level
        if !self.should_log_event(&entry) {
            return Ok(());
        }

        // Send to processing queue
        if let Err(e) = self.sender.send(entry) {
            error!("Failed to queue audit log entry: {}", e);
            let mut stats = self.statistics.lock().await;
            stats.failed_writes += 1;
            return Err(anyhow!("Audit logging queue full"));
        }

        Ok(())
    }

    /// Log authentication event
    pub async fn log_authentication(
        &self,
        user_id: &str,
        source_ip: IpAddr,
        outcome: AuditOutcome,
        details: serde_json::Value,
    ) -> Result<()> {
        let severity = match outcome {
            AuditOutcome::Success => AuditSeverity::Info,
            AuditOutcome::Failure => AuditSeverity::Security,
            _ => AuditSeverity::Warning,
        };

        self.log_detailed_event(AuditLogEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: AuditEventType::Authentication,
            severity,
            user_id: Some(user_id.to_string()),
            session_id: None,
            source_ip: Some(source_ip),
            user_agent: None,
            resource_type: Some("authentication".to_string()),
            resource_id: None,
            action: "login_attempt".to_string(),
            description: format!(
                "Authentication attempt for user {} from {}",
                user_id, source_ip
            ),
            details,
            outcome,
            error_message: None,
            compliance_tags: vec![],
            retention_category: RetentionCategory::Hot,
        })
        .await
    }

    /// Log configuration change
    pub async fn log_configuration_change(
        &self,
        user_id: Option<&str>,
        resource_type: &str,
        resource_id: &str,
        action: &str,
        old_value: Option<serde_json::Value>,
        new_value: Option<serde_json::Value>,
    ) -> Result<()> {
        let details = serde_json::json!({
            "old_value": old_value,
            "new_value": new_value,
            "change_type": action
        });

        self.log_detailed_event(AuditLogEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: AuditEventType::Configuration,
            severity: AuditSeverity::Warning, // Config changes are always important
            user_id: user_id.map(|u| u.to_string()),
            session_id: None,
            source_ip: None,
            user_agent: None,
            resource_type: Some(resource_type.to_string()),
            resource_id: Some(resource_id.to_string()),
            action: action.to_string(),
            description: format!("Configuration change: {} {}", action, resource_type),
            details,
            outcome: AuditOutcome::Success,
            error_message: None,
            compliance_tags: vec![],
            retention_category: RetentionCategory::Warm,
        })
        .await
    }

    /// Log security event
    pub async fn log_security_event(
        &self,
        threat_type: &str,
        source_ip: IpAddr,
        severity: AuditSeverity,
        description: String,
        details: serde_json::Value,
    ) -> Result<()> {
        self.log_detailed_event(AuditLogEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: AuditEventType::SecurityEvent,
            severity,
            user_id: None,
            session_id: None,
            source_ip: Some(source_ip),
            user_agent: None,
            resource_type: Some("security".to_string()),
            resource_id: Some(threat_type.to_string()),
            action: "threat_detected".to_string(),
            description,
            details,
            outcome: AuditOutcome::Success,
            error_message: None,
            compliance_tags: vec![],
            retention_category: RetentionCategory::Hot,
        })
        .await
    }

    /// Log call control event
    pub async fn log_call_event(
        &self,
        call_id: &str,
        action: &str,
        from_number: &str,
        to_number: &str,
        outcome: AuditOutcome,
        details: serde_json::Value,
    ) -> Result<()> {
        self.log_detailed_event(AuditLogEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: AuditEventType::CallControl,
            severity: AuditSeverity::Info,
            user_id: None,
            session_id: None,
            source_ip: None,
            user_agent: None,
            resource_type: Some("call".to_string()),
            resource_id: Some(call_id.to_string()),
            action: action.to_string(),
            description: format!("Call {} from {} to {}", action, from_number, to_number),
            details,
            outcome,
            error_message: None,
            compliance_tags: vec![],
            retention_category: RetentionCategory::Warm,
        })
        .await
    }

    async fn start_background_processor(
        &self,
        mut receiver: mpsc::UnboundedReceiver<AuditLogEntry>,
    ) {
        let config = self.config.clone();
        let log_buffer = self.log_buffer.clone();
        let database_service = self.database_service.clone();
        let statistics = self.statistics.clone();

        tokio::spawn(async move {
            let mut flush_interval = interval(Duration::from_secs(config.flush_interval_seconds));

            loop {
                tokio::select! {
                    entry = receiver.recv() => {
                        if let Some(entry) = entry {
                            let mut buffer = log_buffer.lock().await;
                            buffer.push_back(entry);

                            // Update statistics
                            {
                                let mut stats = statistics.lock().await;
                                stats.total_events_logged += 1;
                                let event_type = format!("{:?}", buffer.back().unwrap().event_type);
                                *stats.events_by_type.entry(event_type).or_insert(0) += 1;
                                let severity = format!("{:?}", buffer.back().unwrap().severity);
                                *stats.events_by_severity.entry(severity).or_insert(0) += 1;
                            }

                            // Flush if buffer is full
                            if buffer.len() >= config.batch_size {
                                let entries: Vec<_> = buffer.drain(..).collect();
                                drop(buffer);

                                if let Err(e) = Self::flush_entries(&config, &entries, &database_service).await {
                                    error!("Failed to flush audit log entries: {}", e);
                                    let mut stats = statistics.lock().await;
                                    stats.storage_errors += 1;
                                }
                            }
                        }
                    }
                    _ = flush_interval.tick() => {
                        let mut buffer = log_buffer.lock().await;
                        if !buffer.is_empty() {
                            let entries: Vec<_> = buffer.drain(..).collect();
                            drop(buffer);

                            if let Err(e) = Self::flush_entries(&config, &entries, &database_service).await {
                                error!("Failed to flush audit log entries: {}", e);
                                let mut stats = statistics.lock().await;
                                stats.storage_errors += 1;
                            }
                        }
                    }
                }
            }
        });
    }

    async fn flush_entries(
        config: &AuditConfig,
        entries: &[AuditLogEntry],
        database_service: &Option<Arc<crate::database::DatabaseService>>,
    ) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        debug!("Flushing {} audit log entries", entries.len());

        // Write to primary storage
        Self::write_to_storage(&config.storage_config.primary_storage, entries).await?;

        // Write to backup storage if configured
        if let Some(backup_storage) = &config.storage_config.backup_storage {
            if let Err(e) = Self::write_to_storage(backup_storage, entries).await {
                warn!("Failed to write to backup storage: {}", e);
            }
        }

        // Write to database if available
        if let Some(db) = database_service {
            for entry in entries {
                if let Err(e) = Self::write_to_database(db, entry).await {
                    warn!("Failed to write audit entry to database: {}", e);
                }
            }
        }

        info!("Successfully flushed {} audit log entries", entries.len());
        Ok(())
    }

    async fn write_to_storage(storage: &StorageBackend, entries: &[AuditLogEntry]) -> Result<()> {
        match storage {
            StorageBackend::File {
                base_path,
                file_prefix,
            } => {
                let timestamp = Utc::now().format("%Y%m%d_%H");
                let filename = format!("{}/{}_{}.jsonl", base_path, file_prefix, timestamp);

                // Ensure directory exists
                if let Some(parent) = std::path::Path::new(&filename).parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }

                let mut file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&filename)
                    .await?;

                for entry in entries {
                    let json_line = serde_json::to_string(entry)?;
                    file.write_all(json_line.as_bytes()).await?;
                    file.write_all(b"\n").await?;
                }

                file.sync_all().await?;
                debug!("Wrote {} entries to file: {}", entries.len(), filename);
            }
            StorageBackend::Syslog { server, facility } => {
                // TODO: Implement syslog writing
                debug!(
                    "Would write {} entries to syslog: {}:{}",
                    entries.len(),
                    server,
                    facility
                );
            }
            StorageBackend::Database { .. } => {
                // Handled separately by write_to_database
            }
            StorageBackend::ElasticSearch {
                endpoints,
                index_pattern,
            } => {
                // TODO: Implement Elasticsearch writing
                debug!(
                    "Would write {} entries to Elasticsearch: {:?}",
                    entries.len(),
                    endpoints
                );
            }
        }

        Ok(())
    }

    async fn write_to_database(
        database_service: &Arc<crate::database::DatabaseService>,
        entry: &AuditLogEntry,
    ) -> Result<()> {
        // Use the database service to insert audit log entry
        let pool = database_service.get_pool();

        sqlx::query!(
            r#"
            INSERT INTO audit_log (
                id, user_id, action, resource_type, resource_id, details,
                ip_address, user_agent, success, error_message, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
            entry.id,
            entry.user_id,
            entry.action,
            entry.resource_type,
            entry.resource_id,
            entry.details,
            entry.source_ip.map(|ip| ip.to_string()),
            entry.user_agent,
            matches!(entry.outcome, AuditOutcome::Success),
            entry.error_message,
            entry.timestamp
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    async fn start_retention_manager(&self) {
        let config = self.config.clone();
        let statistics = self.statistics.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(24 * 60 * 60)); // Run daily

            loop {
                interval.tick().await;

                debug!("Running audit log retention management");

                // TODO: Implement retention policy enforcement
                // - Move hot -> warm -> cold -> archive based on age
                // - Delete old entries beyond retention period
                // - Compress old files
                // - Generate retention reports

                let mut stats = statistics.lock().await;
                stats.retention_actions += 1;
            }
        });
    }

    fn determine_compliance_tags(&self, entry: &AuditLogEntry) -> Vec<ComplianceStandard> {
        let mut tags = Vec::new();

        for standard in &self.config.compliance_standards {
            let should_tag = match standard {
                ComplianceStandard::Sox => {
                    matches!(
                        entry.event_type,
                        AuditEventType::Configuration
                            | AuditEventType::UserManagement
                            | AuditEventType::SystemOperation
                    )
                }
                ComplianceStandard::Calea => {
                    matches!(
                        entry.event_type,
                        AuditEventType::CallControl
                            | AuditEventType::SipMessage
                            | AuditEventType::EmergencyAction
                    )
                }
                ComplianceStandard::TracedAct => {
                    matches!(
                        entry.event_type,
                        AuditEventType::CallControl | AuditEventType::SecurityEvent
                    ) && entry.description.contains("fraud")
                }
                ComplianceStandard::Pci => {
                    matches!(
                        entry.event_type,
                        AuditEventType::Authentication | AuditEventType::BillingOperation
                    )
                }
                _ => false,
            };

            if should_tag {
                tags.push(standard.clone());
            }
        }

        tags
    }

    fn determine_retention_category(&self, entry: &AuditLogEntry) -> RetentionCategory {
        match entry.event_type {
            AuditEventType::Authentication | AuditEventType::SecurityEvent => {
                RetentionCategory::Hot
            }
            AuditEventType::Configuration | AuditEventType::UserManagement => {
                RetentionCategory::Warm
            }
            AuditEventType::CallControl => RetentionCategory::Cold,
            _ => RetentionCategory::Archive,
        }
    }

    fn sanitize_sensitive_data(&self, mut data: serde_json::Value) -> serde_json::Value {
        if let serde_json::Value::Object(ref mut map) = data {
            for (key, value) in map.iter_mut() {
                if self
                    .config
                    .sensitive_fields
                    .iter()
                    .any(|field| key.to_lowercase().contains(&field.to_lowercase()))
                {
                    *value = serde_json::Value::String("[REDACTED]".to_string());
                }
            }
        }
        data
    }

    fn should_log_event(&self, entry: &AuditLogEntry) -> bool {
        match self.config.log_level {
            AuditLogLevel::Essential => {
                matches!(
                    entry.event_type,
                    AuditEventType::Authentication
                        | AuditEventType::Configuration
                        | AuditEventType::SecurityEvent
                )
            }
            AuditLogLevel::Standard => !matches!(entry.event_type, AuditEventType::SipMessage),
            AuditLogLevel::Detailed | AuditLogLevel::Forensic => true,
        }
    }

    /// Get audit statistics
    pub async fn get_statistics(&self) -> AuditStatistics {
        let stats = self.statistics.lock().await;
        AuditStatistics {
            total_events_logged: stats.total_events_logged,
            events_by_type: stats.events_by_type.clone(),
            events_by_severity: stats.events_by_severity.clone(),
            failed_writes: stats.failed_writes,
            storage_errors: stats.storage_errors,
            compliance_events: stats.compliance_events,
            retention_actions: stats.retention_actions,
        }
    }

    /// Generate compliance report
    pub async fn generate_compliance_report(
        &self,
        standard: ComplianceStandard,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<ComplianceReport> {
        // TODO: Implement compliance report generation
        // Query audit logs for specific compliance standard
        // Generate report with required fields and statistics

        Ok(ComplianceReport {
            standard,
            report_period: (start_date, end_date),
            total_events: 0,
            compliance_events: 0,
            violations: vec![],
            summary: "Report generation not yet implemented".to_string(),
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub standard: ComplianceStandard,
    pub report_period: (DateTime<Utc>, DateTime<Utc>),
    pub total_events: u64,
    pub compliance_events: u64,
    pub violations: Vec<String>,
    pub summary: String,
}

// Convenience functions for common audit events
impl AuditLoggingService {
    pub async fn log_user_login(&self, user_id: &str, ip: IpAddr, success: bool) -> Result<()> {
        let outcome = if success {
            AuditOutcome::Success
        } else {
            AuditOutcome::Failure
        };
        self.log_authentication(
            user_id,
            ip,
            outcome,
            serde_json::json!({
                "action": "login",
                "success": success
            }),
        )
        .await
    }

    pub async fn log_trunk_configuration(
        &self,
        user_id: &str,
        trunk_id: &str,
        action: &str,
    ) -> Result<()> {
        self.log_configuration_change(Some(user_id), "trunk", trunk_id, action, None, None)
            .await
    }

    pub async fn log_emergency_call(&self, call_id: &str, from: &str, to: &str) -> Result<()> {
        self.log_detailed_event(AuditLogEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: AuditEventType::EmergencyAction,
            severity: AuditSeverity::Critical,
            user_id: None,
            session_id: None,
            source_ip: None,
            user_agent: None,
            resource_type: Some("emergency_call".to_string()),
            resource_id: Some(call_id.to_string()),
            action: "emergency_call_initiated".to_string(),
            description: format!("Emergency call from {} to {}", from, to),
            details: serde_json::json!({
                "from_number": from,
                "to_number": to,
                "call_id": call_id,
                "emergency_type": "911"
            }),
            outcome: AuditOutcome::Success,
            error_message: None,
            compliance_tags: vec![ComplianceStandard::Calea],
            retention_category: RetentionCategory::Hot,
        })
        .await
    }
}
