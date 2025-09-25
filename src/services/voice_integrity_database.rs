//! Voice Integrity Database Service
//!
//! This service handles all database operations for the voice integrity and legal authorization system.
//! It provides CRUD operations for legal authorizations, call recordings, transcriptions, and audit logs.
//!
//! Key features:
//! - Legal authorization management
//! - Call recording metadata storage
//! - Transcription and banned word analysis storage
//! - Comprehensive audit logging
//! - ECPA compliance tracking

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument};

use crate::events::EventBus;
use crate::services::legal_authorization::{
    LegalAuthorization, AuthorizationStatus,
    LawfulInterceptTarget, CreateAuthorizationRequest, CreateTargetRequest,
};
use crate::services::AudioRecording;
use crate::services::vosk_client::TranscriptionResult;

use sqlx::PgPool;

/// Database configuration for voice integrity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceIntegrityDatabaseConfig {
    /// Database connection string
    pub database_url: String,
    /// Connection pool size
    pub max_connections: u32,
    /// Connection timeout in seconds
    pub connection_timeout_seconds: u64,
    /// Enable database migrations
    pub enable_migrations: bool,
    /// Audit log retention days
    pub audit_log_retention_days: u32,
    /// Recording metadata retention days
    pub recording_retention_days: u32,
}

impl Default for VoiceIntegrityDatabaseConfig {
    fn default() -> Self {
        Self {
            database_url: "postgresql://localhost/redfire_switch".to_string(),
            max_connections: 20,
            connection_timeout_seconds: 30,
            enable_migrations: true,
            audit_log_retention_days: 2555, // 7 years
            recording_retention_days: 365,   // 1 year default
        }
    }
}

/// Call recording metadata for database storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallRecordingMetadata {
    pub id: Option<i32>,
    pub call_id: String,
    pub session_id: String,
    pub ingress_trunk_id: i32,
    pub recording_path: String,
    pub storage_type: String, // 'memory' or 'disk'
    pub file_size_bytes: i64,
    pub duration_seconds: i32,
    pub sample_rate: i32,
    pub channels: i32,
    pub codec: String,
    pub recorded_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
    pub archived_at: Option<DateTime<Utc>>,
    pub retention_expires_at: Option<DateTime<Utc>>,
    pub legal_hold: bool,
    pub legal_authorization_ref: Option<String>,
    pub voice_integrity_officer_id: Option<String>,
    pub legal_review_required: bool,
    pub legal_review_completed: bool,
    pub legal_review_date: Option<DateTime<Utc>>,
    pub data_classification: String,
}

/// Call transcription metadata for database storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallTranscriptionMetadata {
    pub id: Option<i32>,
    pub recording_id: i32,
    pub transcription_text: String,
    pub confidence_score: Option<f64>,
    pub language_detected: Option<String>,
    pub processing_engine: String,
    pub banned_words_detected: i32,
    pub banned_words_list: Vec<String>,
    pub fraud_indicators: HashMap<String, String>,
    pub risk_score: f64,
    pub transcribed_at: DateTime<Utc>,
    pub legal_review_required: bool,
    pub compliance_flags: HashMap<String, bool>,
}

/// Anti-fraud event for database storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntiFraudEventRecord {
    pub id: Option<i32>,
    pub event_type: String,
    pub call_id: Option<String>,
    pub session_id: Option<String>,
    pub recording_id: Option<i32>,
    pub risk_score: f64,
    pub fraud_indicators: HashMap<String, String>,
    pub detection_method: String,
    pub confidence_level: f64,
    pub investigated: bool,
    pub investigation_notes: Option<String>,
    pub resolved: bool,
    pub resolution_date: Option<DateTime<Utc>>,
    pub escalated: bool,
    pub escalation_reason: Option<String>,
    pub lawful_intercept_case: bool,
    pub authorization_id: Option<i32>,
    pub detected_at: DateTime<Utc>,
}

/// Voice integrity audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceIntegrityAuditRecord {
    pub id: Option<i32>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub action_type: String,
    pub resource_type: String,
    pub resource_id: String,
    pub authorization_id: Option<i32>,
    pub legal_basis: Option<String>,
    pub business_justification: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub request_details: Option<HashMap<String, serde_json::Value>>,
    pub response_summary: Option<HashMap<String, serde_json::Value>>,
    pub ecpa_compliant: bool,
    pub calea_notification_required: bool,
    pub data_minimization_applied: bool,
    pub timestamp: DateTime<Utc>,
}

/// Voice integrity statistics for compliance reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceIntegrityStatistics {
    pub id: Option<i32>,
    pub report_date: chrono::NaiveDate,
    pub active_authorizations: i32,
    pub pending_authorizations: i32,
    pub expired_authorizations: i32,
    pub total_targets_monitored: i32,
    pub calls_intercepted_today: i32,
    pub data_collected_bytes_today: i64,
    pub compliance_violations: i32,
    pub overdue_notifications: i32,
    pub expired_authorizations_past_due: i32,
    pub authorized_access_events: i32,
    pub unauthorized_access_attempts: i32,
    pub data_exports_today: i32,
    pub created_at: DateTime<Utc>,
}

/// Voice Integrity Database Service
pub struct VoiceIntegrityDatabaseService {
    config: VoiceIntegrityDatabaseConfig,
    event_bus: Arc<EventBus>,
    pool: PgPool,
    // In-memory storage for demonstration (legacy)
    legal_authorizations: Arc<RwLock<HashMap<i32, LegalAuthorization>>>,
    intercept_targets: Arc<RwLock<HashMap<i32, LawfulInterceptTarget>>>,
    call_recordings: Arc<RwLock<HashMap<i32, CallRecordingMetadata>>>,
    transcriptions: Arc<RwLock<HashMap<i32, CallTranscriptionMetadata>>>,
    audit_logs: Arc<RwLock<Vec<VoiceIntegrityAuditRecord>>>,
    next_id: Arc<RwLock<i32>>,
}

impl VoiceIntegrityDatabaseService {
    /// Create new voice integrity database service
    pub async fn new(config: VoiceIntegrityDatabaseConfig, event_bus: Arc<EventBus>) -> Result<Self> {
        info!("Initializing voice integrity database service");

        // Create database connection pool
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(std::time::Duration::from_secs(config.connection_timeout_seconds))
            .connect(&config.database_url)
            .await
            .context("Failed to create database connection pool")?;

        Ok(Self {
            config,
            event_bus,
            pool,
            legal_authorizations: Arc::new(RwLock::new(HashMap::new())),
            intercept_targets: Arc::new(RwLock::new(HashMap::new())),
            call_recordings: Arc::new(RwLock::new(HashMap::new())),
            transcriptions: Arc::new(RwLock::new(HashMap::new())),
            audit_logs: Arc::new(RwLock::new(Vec::new())),
            next_id: Arc::new(RwLock::new(1)),
        })
    }

    /// Get database connection pool
    pub fn get_pool(&self) -> &PgPool {
        &self.pool
    }

    /// Create a new legal authorization
    #[instrument(skip(self), fields(authorization_number = %request.authorization_number))]
    pub async fn create_legal_authorization(
        &self,
        request: CreateAuthorizationRequest,
        created_by: String,
    ) -> Result<LegalAuthorization> {
        info!("Creating legal authorization: {}", request.authorization_number);

        let mut id_counter = self.next_id.write().await;
        let authorization_id = *id_counter;
        *id_counter += 1;

        let now = Utc::now();
        let authorization = LegalAuthorization {
            id: authorization_id,
            authorization_number: request.authorization_number.clone(),
            authorization_type: request.authorization_type,
            status: AuthorizationStatus::Pending,
            issuing_authority: request.issuing_authority,
            case_number: request.case_number,
            investigating_agency: request.investigating_agency,
            investigating_officer: request.investigating_officer,
            contact_information: request.contact_information,
            target_identifiers: request.target_identifiers,
            target_description: request.target_description,
            scope_description: request.scope_description,
            effective_date: request.effective_date,
            expiration_date: request.expiration_date,
            service_date: None,
            served_by: None,
            legal_review_by: None,
            compliance_notes: None,
            authorization_document_path: None,
            service_acknowledgment_path: None,
            created_at: now,
            updated_at: now,
            created_by,
        };

        // Store in database
        let mut authorizations = self.legal_authorizations.write().await;
        authorizations.insert(authorization_id, authorization.clone());

        // Log audit entry
        self.log_audit_entry(VoiceIntegrityAuditRecord {
            id: None,
            user_id: Some(authorization.created_by.clone()),
            session_id: None,
            action_type: "create_legal_authorization".to_string(),
            resource_type: "legal_authorization".to_string(),
            resource_id: authorization_id.to_string(),
            authorization_id: Some(authorization_id),
            legal_basis: Some("CALEA_compliance".to_string()),
            business_justification: Some(format!("Legal authorization created: {}",
                authorization.scope_description)),
            ip_address: None,
            user_agent: None,
            request_details: None,
            response_summary: None,
            ecpa_compliant: true,
            calea_notification_required: true,
            data_minimization_applied: true,
            timestamp: now,
        }).await?;

        Ok(authorization)
    }

    /// Get legal authorization by ID
    pub async fn get_legal_authorization(&self, authorization_id: i32) -> Result<Option<LegalAuthorization>> {
        let authorizations = self.legal_authorizations.read().await;
        Ok(authorizations.get(&authorization_id).cloned())
    }

    /// Update legal authorization status
    #[instrument(skip(self), fields(authorization_id = authorization_id))]
    pub async fn update_authorization_status(
        &self,
        authorization_id: i32,
        new_status: AuthorizationStatus,
        change_reason: String,
        changed_by: String,
    ) -> Result<()> {
        info!("Updating authorization {} status to {:?}", authorization_id, new_status);

        let mut authorizations = self.legal_authorizations.write().await;
        if let Some(authorization) = authorizations.get_mut(&authorization_id) {
            let previous_status = authorization.status.clone();
            authorization.status = new_status.clone();
            authorization.updated_at = Utc::now();

            // Log audit entry
            self.log_audit_entry(VoiceIntegrityAuditRecord {
                id: None,
                user_id: Some(changed_by),
                session_id: None,
                action_type: "update_authorization_status".to_string(),
                resource_type: "legal_authorization".to_string(),
                resource_id: authorization_id.to_string(),
                authorization_id: Some(authorization_id),
                legal_basis: Some("workflow_management".to_string()),
                business_justification: Some(format!("Status change: {:?} -> {:?}: {}",
                    previous_status, new_status, change_reason)),
                ip_address: None,
                user_agent: None,
                request_details: None,
                response_summary: None,
                ecpa_compliant: true,
                calea_notification_required: false,
                data_minimization_applied: true,
                timestamp: Utc::now(),
            }).await?;

            return Ok(());
        }

        Err(anyhow::anyhow!("Authorization not found: {}", authorization_id))
    }

    /// Create lawful intercept target
    #[instrument(skip(self), fields(authorization_id = authorization_id, target_value = %target.target_value))]
    pub async fn create_intercept_target(
        &self,
        authorization_id: i32,
        target: CreateTargetRequest,
    ) -> Result<LawfulInterceptTarget> {
        info!("Creating intercept target for authorization {}: {}", authorization_id, target.target_value);

        let mut id_counter = self.next_id.write().await;
        let target_id = *id_counter;
        *id_counter += 1;

        let now = Utc::now();
        let intercept_target = LawfulInterceptTarget {
            id: target_id,
            authorization_id,
            target_type: target.target_type,
            target_value: target.target_value.clone(),
            target_description: target.target_description,
            monitoring_enabled: true,
            content_intercept_enabled: target.content_intercept_enabled,
            retention_days: target.retention_days.unwrap_or(365),
            first_activity_date: None,
            last_activity_date: None,
            total_calls_intercepted: 0,
            total_data_collected_bytes: 0,
            created_at: now,
            updated_at: now,
        };

        // Store in database
        let mut targets = self.intercept_targets.write().await;
        targets.insert(target_id, intercept_target.clone());

        // Log audit entry
        self.log_audit_entry(VoiceIntegrityAuditRecord {
            id: None,
            user_id: None,
            session_id: None,
            action_type: "create_intercept_target".to_string(),
            resource_type: "lawful_intercept_target".to_string(),
            resource_id: format!("{}:{}", intercept_target.target_type, intercept_target.target_value),
            authorization_id: Some(authorization_id),
            legal_basis: Some("lawful_intercept".to_string()),
            business_justification: Some(format!("Target added for authorization: {}", authorization_id)),
            ip_address: None,
            user_agent: None,
            request_details: None,
            response_summary: None,
            ecpa_compliant: true,
            calea_notification_required: true,
            data_minimization_applied: true,
            timestamp: now,
        }).await?;

        Ok(intercept_target)
    }

    /// Get intercept targets for authorization
    pub async fn get_intercept_targets(&self, authorization_id: i32) -> Result<Vec<LawfulInterceptTarget>> {
        let targets = self.intercept_targets.read().await;
        let filtered_targets: Vec<LawfulInterceptTarget> = targets
            .values()
            .filter(|target| target.authorization_id == authorization_id)
            .cloned()
            .collect();

        Ok(filtered_targets)
    }

    /// Store call recording metadata
    #[instrument(skip(self), fields(call_id = %recording.call_id))]
    pub async fn store_recording_metadata(&self, recording: &AudioRecording) -> Result<i32> {
        info!("Storing recording metadata for call: {}", recording.call_id);

        let mut id_counter = self.next_id.write().await;
        let recording_id = *id_counter;
        *id_counter += 1;

        let metadata = CallRecordingMetadata {
            id: Some(recording_id),
            call_id: recording.call_id.clone(),
            session_id: recording.session_id.clone(),
            ingress_trunk_id: recording.trunk_id,
            recording_path: recording.file_path.to_string_lossy().to_string(),
            storage_type: match recording.storage_type {
                crate::services::audio_recording::StorageType::Memory => "memory".to_string(),
                crate::services::audio_recording::StorageType::Disk => "disk".to_string(),
            },
            file_size_bytes: recording.file_size_bytes as i64,
            duration_seconds: recording.duration_seconds as i32,
            sample_rate: recording.wav_sample_rate as i32,
            channels: recording.wav_channels as i32,
            codec: format!("{:?}", recording.original_codec),
            recorded_at: recording.started_at,
            processed_at: recording.completed_at,
            archived_at: None,
            retention_expires_at: None, // Would be calculated based on policy
            legal_hold: recording.legal_authorization_id.is_some(),
            legal_authorization_ref: recording.legal_authorization_id.map(|id| id.to_string()),
            voice_integrity_officer_id: None,
            legal_review_required: recording.legal_authorization_id.is_some(),
            legal_review_completed: false,
            legal_review_date: None,
            data_classification: if recording.legal_authorization_id.is_some() {
                "confidential".to_string()
            } else {
                "unclassified".to_string()
            },
        };

        // Store in database
        let mut recordings = self.call_recordings.write().await;
        recordings.insert(recording_id, metadata);

        // Log audit entry
        self.log_audit_entry(VoiceIntegrityAuditRecord {
            id: None,
            user_id: None,
            session_id: Some(recording.session_id.clone()),
            action_type: "store_recording_metadata".to_string(),
            resource_type: "call_recording".to_string(),
            resource_id: recording_id.to_string(),
            authorization_id: recording.legal_authorization_id,
            legal_basis: Some(recording.monitoring_purpose.clone()),
            business_justification: Some("Recording metadata stored for compliance".to_string()),
            ip_address: None,
            user_agent: None,
            request_details: None,
            response_summary: None,
            ecpa_compliant: recording.ecpa_compliant,
            calea_notification_required: recording.legal_authorization_id.is_some(),
            data_minimization_applied: true,
            timestamp: Utc::now(),
        }).await?;

        Ok(recording_id)
    }

    /// Store transcription result
    #[instrument(skip(self), fields(recording_id = %transcription.recording_id))]
    pub async fn store_transcription(&self, transcription: &TranscriptionResult, recording_db_id: i32) -> Result<i32> {
        info!("Storing transcription for recording: {}", transcription.recording_id);

        let mut id_counter = self.next_id.write().await;
        let transcription_id = *id_counter;
        *id_counter += 1;

        let metadata = CallTranscriptionMetadata {
            id: Some(transcription_id),
            recording_id: recording_db_id,
            transcription_text: transcription.text.clone(),
            confidence_score: Some(transcription.confidence),
            language_detected: Some("en-US".to_string()), // Would be detected by Vosk
            processing_engine: "vosk".to_string(),
            banned_words_detected: transcription.banned_words_detected.len() as i32,
            banned_words_list: transcription.banned_words_detected.clone(),
            fraud_indicators: HashMap::new(), // Would be populated by analysis
            risk_score: transcription.fraud_risk_score,
            transcribed_at: transcription.timestamp,
            legal_review_required: transcription.fraud_risk_score > 0.7,
            compliance_flags: {
                let mut flags = HashMap::new();
                flags.insert("ecpa_compliant".to_string(), true);
                flags.insert("contains_banned_words".to_string(), !transcription.banned_words_detected.is_empty());
                flags.insert("high_risk".to_string(), transcription.fraud_risk_score > 0.8);
                flags
            },
        };

        // Store in database
        let mut transcriptions = self.transcriptions.write().await;
        transcriptions.insert(transcription_id, metadata);

        // Log audit entry
        self.log_audit_entry(VoiceIntegrityAuditRecord {
            id: None,
            user_id: None,
            session_id: None,
            action_type: "store_transcription".to_string(),
            resource_type: "call_transcription".to_string(),
            resource_id: transcription_id.to_string(),
            authorization_id: None,
            legal_basis: Some("fraud_detection".to_string()),
            business_justification: Some(format!("Transcription stored with risk score: {}",
                transcription.fraud_risk_score)),
            ip_address: None,
            user_agent: None,
            request_details: None,
            response_summary: None,
            ecpa_compliant: true,
            calea_notification_required: false,
            data_minimization_applied: true,
            timestamp: Utc::now(),
        }).await?;

        Ok(transcription_id)
    }

    /// Log voice integrity audit entry
    pub async fn log_audit_entry(&self, entry: VoiceIntegrityAuditRecord) -> Result<()> {
        debug!("Logging voice integrity audit entry: {}", entry.action_type);

        let mut audit_logs = self.audit_logs.write().await;
        audit_logs.push(entry);

        // In production, this would also emit an event for real-time monitoring
        Ok(())
    }

    /// Get audit logs for a specific resource
    pub async fn get_audit_logs(
        &self,
        resource_type: Option<String>,
        resource_id: Option<String>,
        limit: Option<usize>,
    ) -> Result<Vec<VoiceIntegrityAuditRecord>> {
        let audit_logs = self.audit_logs.read().await;

        let mut filtered_logs: Vec<VoiceIntegrityAuditRecord> = audit_logs
            .iter()
            .filter(|log| {
                if let Some(ref r_type) = resource_type {
                    if log.resource_type != *r_type {
                        return false;
                    }
                }
                if let Some(ref r_id) = resource_id {
                    if log.resource_id != *r_id {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        // Sort by timestamp (newest first)
        filtered_logs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        // Apply limit
        if let Some(limit_count) = limit {
            filtered_logs.truncate(limit_count);
        }

        Ok(filtered_logs)
    }

    /// Get voice integrity statistics
    pub async fn get_voice_integrity_statistics(&self) -> Result<VoiceIntegrityStatistics> {
        let authorizations = self.legal_authorizations.read().await;
        let targets = self.intercept_targets.read().await;
        let recordings = self.call_recordings.read().await;

        let active_authorizations = authorizations
            .values()
            .filter(|auth| matches!(auth.status, AuthorizationStatus::Active))
            .count() as i32;

        let pending_authorizations = authorizations
            .values()
            .filter(|auth| matches!(auth.status, AuthorizationStatus::Pending))
            .count() as i32;

        let expired_authorizations = authorizations
            .values()
            .filter(|auth| matches!(auth.status, AuthorizationStatus::Expired))
            .count() as i32;

        let total_targets_monitored = targets
            .values()
            .filter(|target| target.monitoring_enabled)
            .count() as i32;

        let today = Utc::now().date_naive();
        let calls_intercepted_today = recordings
            .values()
            .filter(|rec| rec.recorded_at.date_naive() == today && rec.legal_hold)
            .count() as i32;

        let data_collected_bytes_today = recordings
            .values()
            .filter(|rec| rec.recorded_at.date_naive() == today)
            .map(|rec| rec.file_size_bytes)
            .sum();

        Ok(VoiceIntegrityStatistics {
            id: None,
            report_date: today,
            active_authorizations,
            pending_authorizations,
            expired_authorizations,
            total_targets_monitored,
            calls_intercepted_today,
            data_collected_bytes_today,
            compliance_violations: 0, // Would be calculated from audit logs
            overdue_notifications: 0, // Would be calculated from authorization dates
            expired_authorizations_past_due: 0, // Would be calculated
            authorized_access_events: 0, // Would be calculated from audit logs
            unauthorized_access_attempts: 0, // Would be calculated from audit logs
            data_exports_today: 0, // Would be calculated from audit logs
            created_at: Utc::now(),
        })
    }

    /// Search call recordings by criteria
    pub async fn search_recordings(
        &self,
        call_id: Option<String>,
        trunk_id: Option<i32>,
        legal_authorization_id: Option<i32>,
        start_date: Option<DateTime<Utc>>,
        end_date: Option<DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<Vec<CallRecordingMetadata>> {
        let recordings = self.call_recordings.read().await;

        let mut filtered_recordings: Vec<CallRecordingMetadata> = recordings
            .values()
            .filter(|recording| {
                if let Some(ref c_id) = call_id {
                    if recording.call_id != *c_id {
                        return false;
                    }
                }
                if let Some(t_id) = trunk_id {
                    if recording.ingress_trunk_id != t_id {
                        return false;
                    }
                }
                if let Some(auth_id) = legal_authorization_id {
                    if recording.legal_authorization_ref != Some(auth_id.to_string()) {
                        return false;
                    }
                }
                if let Some(start) = start_date {
                    if recording.recorded_at < start {
                        return false;
                    }
                }
                if let Some(end) = end_date {
                    if recording.recorded_at > end {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        // Sort by recorded date (newest first)
        filtered_recordings.sort_by(|a, b| b.recorded_at.cmp(&a.recorded_at));

        // Apply limit
        if let Some(limit_count) = limit {
            filtered_recordings.truncate(limit_count);
        }

        Ok(filtered_recordings)
    }

    /// Get database service statistics
    pub async fn get_service_statistics(&self) -> HashMap<String, u64> {
        let authorizations = self.legal_authorizations.read().await;
        let targets = self.intercept_targets.read().await;
        let recordings = self.call_recordings.read().await;
        let transcriptions = self.transcriptions.read().await;
        let audit_logs = self.audit_logs.read().await;

        let mut stats = HashMap::new();

        stats.insert("total_legal_authorizations".to_string(), authorizations.len() as u64);
        stats.insert("total_intercept_targets".to_string(), targets.len() as u64);
        stats.insert("total_call_recordings".to_string(), recordings.len() as u64);
        stats.insert("total_transcriptions".to_string(), transcriptions.len() as u64);
        stats.insert("total_audit_entries".to_string(), audit_logs.len() as u64);

        let active_authorizations = authorizations
            .values()
            .filter(|auth| matches!(auth.status, AuthorizationStatus::Active))
            .count() as u64;
        stats.insert("active_authorizations".to_string(), active_authorizations);

        let legal_recordings = recordings
            .values()
            .filter(|rec| rec.legal_hold)
            .count() as u64;
        stats.insert("legal_recordings".to_string(), legal_recordings);

        let high_risk_transcriptions = transcriptions
            .values()
            .filter(|trans| trans.risk_score > 0.8)
            .count() as u64;
        stats.insert("high_risk_transcriptions".to_string(), high_risk_transcriptions);

        stats
    }
}