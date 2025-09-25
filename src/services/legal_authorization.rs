//! Legal Authorization Management Service
//!
//! This service manages legal authorizations for lawful intercept under CALEA, ECPA,
//! and other applicable laws. It provides comprehensive tracking, workflow management,
//! and compliance functionality for voice integrity officers.
//!
//! Key features:
//! - Legal authorization lifecycle management
//! - Lawful intercept target management
//! - Compliance audit trail
//! - Workflow state transitions
//! - ECPA compliance enforcement

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, instrument};

use crate::events::{EventBus, TelecomEvent};

/// Legal authorization types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AuthorizationType {
    CourtOrder,
    SearchWarrant,
    WiretapOrder,
    PenRegister,
    EmergencyRequest,
    AdministrativeSubpoena,
    NationalSecurityLetter,
}

/// Authorization status in workflow
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AuthorizationStatus {
    Pending,
    Approved,
    Active,
    Expired,
    Revoked,
    Appealed,
}

/// Legal authorization entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegalAuthorization {
    pub id: i32,
    pub authorization_number: String,
    pub authorization_type: AuthorizationType,
    pub status: AuthorizationStatus,

    // Legal details
    pub issuing_authority: String,
    pub case_number: Option<String>,
    pub investigating_agency: String,
    pub investigating_officer: String,
    pub contact_information: HashMap<String, String>,

    // Scope and targets
    pub target_identifiers: HashMap<String, Vec<String>>,
    pub target_description: Option<String>,
    pub scope_description: String,

    // Temporal constraints
    pub effective_date: DateTime<Utc>,
    pub expiration_date: DateTime<Utc>,
    pub service_date: Option<DateTime<Utc>>,

    // Compliance tracking
    pub served_by: Option<String>,
    pub legal_review_by: Option<String>,
    pub compliance_notes: Option<String>,

    // Document management
    pub authorization_document_path: Option<String>,
    pub service_acknowledgment_path: Option<String>,

    // Metadata
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: String,
}

/// Lawful intercept target
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LawfulInterceptTarget {
    pub id: i32,
    pub authorization_id: i32,

    // Target identification
    pub target_type: String, // 'phone_number', 'ip_address', 'trunk_id'
    pub target_value: String,
    pub target_description: Option<String>,

    // Monitoring configuration
    pub monitoring_enabled: bool,
    pub content_intercept_enabled: bool,
    pub retention_days: i32,

    // Status tracking
    pub first_activity_date: Option<DateTime<Utc>>,
    pub last_activity_date: Option<DateTime<Utc>>,
    pub total_calls_intercepted: i32,
    pub total_data_collected_bytes: i64,

    // Metadata
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Legal authorization workflow event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationWorkflowEvent {
    pub id: i32,
    pub authorization_id: i32,

    // State change details
    pub previous_status: Option<AuthorizationStatus>,
    pub new_status: AuthorizationStatus,
    pub change_reason: String,
    pub supporting_documentation: Option<String>,

    // Approval chain
    pub changed_by: String,
    pub approved_by: Option<String>,
    pub legal_review_completed: bool,

    // Notification tracking
    pub law_enforcement_notified: bool,
    pub notification_method: Option<String>,
    pub notification_date: Option<DateTime<Utc>>,

    pub created_at: DateTime<Utc>,
}

/// Voice integrity audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceIntegrityAuditEntry {
    pub id: i32,
    pub user_id: Option<String>,
    pub session_id: Option<String>,

    // Action details
    pub action_type: String,
    pub resource_type: String,
    pub resource_id: String,

    // Context and metadata
    pub authorization_id: Option<i32>,
    pub legal_basis: Option<String>,
    pub business_justification: Option<String>,

    // Technical details
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub request_details: Option<HashMap<String, serde_json::Value>>,
    pub response_summary: Option<HashMap<String, serde_json::Value>>,

    // Compliance tracking
    pub ecpa_compliant: bool,
    pub calea_notification_required: bool,
    pub data_minimization_applied: bool,

    pub timestamp: DateTime<Utc>,
}

/// Legal authorization service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegalAuthorizationConfig {
    /// Enable legal authorization management
    pub enabled: bool,
    /// Auto-expire check interval in minutes
    pub expiration_check_interval_minutes: u32,
    /// Days before expiration to send warnings
    pub expiration_warning_days: u32,
    /// Maximum authorization duration in days
    pub max_authorization_duration_days: u32,
    /// Require legal review for all authorizations
    pub require_legal_review: bool,
    /// Auto-notification of law enforcement
    pub auto_notify_law_enforcement: bool,
    /// Compliance officer email
    pub compliance_officer_email: Option<String>,
    /// Legal counsel email
    pub legal_counsel_email: Option<String>,
}

impl Default for LegalAuthorizationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            expiration_check_interval_minutes: 60, // Check hourly
            expiration_warning_days: 7,
            max_authorization_duration_days: 365,
            require_legal_review: true,
            auto_notify_law_enforcement: true,
            compliance_officer_email: None,
            legal_counsel_email: None,
        }
    }
}

/// Legal authorization service
pub struct LegalAuthorizationService {
    config: LegalAuthorizationConfig,
    event_bus: Arc<EventBus>,
    active_authorizations: Arc<RwLock<HashMap<i32, LegalAuthorization>>>,
    active_targets: Arc<RwLock<HashMap<String, Vec<LawfulInterceptTarget>>>>,
}

impl LegalAuthorizationService {
    /// Create new legal authorization service
    pub fn new(config: LegalAuthorizationConfig, event_bus: Arc<EventBus>) -> Self {
        Self {
            config,
            event_bus,
            active_authorizations: Arc::new(RwLock::new(HashMap::new())),
            active_targets: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new legal authorization
    #[instrument(skip(self), fields(authorization_number = %request.authorization_number))]
    pub async fn create_authorization(
        &self,
        request: CreateAuthorizationRequest,
        created_by: String,
    ) -> Result<LegalAuthorization> {
        info!("Creating legal authorization: {}", request.authorization_number);

        // Validate authorization request
        self.validate_authorization_request(&request)?;

        let now = Utc::now();
        let authorization = LegalAuthorization {
            id: 0, // Will be set by database
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

        // Log the creation
        self.log_audit_entry(VoiceIntegrityAuditEntry {
            id: 0,
            user_id: Some(authorization.created_by.clone()),
            session_id: None,
            action_type: "create_authorization".to_string(),
            resource_type: "legal_authorization".to_string(),
            resource_id: authorization.authorization_number.clone(),
            authorization_id: None,
            legal_basis: Some("CALEA/ECPA_compliance".to_string()),
            business_justification: Some(format!("Legal authorization created: {}",
                authorization.scope_description)),
            ip_address: None,
            user_agent: None,
            request_details: None,
            response_summary: None,
            ecpa_compliant: true,
            calea_notification_required: matches!(
                authorization.authorization_type,
                AuthorizationType::WiretapOrder | AuthorizationType::CourtOrder
            ),
            data_minimization_applied: true,
            timestamp: now,
        }).await;

        // Emit event
        let event = TelecomEvent::LegalAuthorizationCreated {
            authorization_id: authorization.authorization_number.clone(),
            authorization_type: format!("{:?}", authorization.authorization_type),
            effective_date: authorization.effective_date,
            expiration_date: authorization.expiration_date,
        };
        self.event_bus.publish(event).await?;

        Ok(authorization)
    }

    /// Update authorization status
    #[instrument(skip(self), fields(authorization_id = authorization_id))]
    pub async fn update_authorization_status(
        &self,
        authorization_id: i32,
        new_status: AuthorizationStatus,
        change_reason: String,
        changed_by: String,
    ) -> Result<()> {
        info!("Updating authorization {} status to {:?}", authorization_id, new_status);

        let mut authorizations = self.active_authorizations.write().await;
        let authorization = authorizations.get_mut(&authorization_id)
            .ok_or_else(|| anyhow::anyhow!("Authorization not found: {}", authorization_id))?;

        let previous_status = authorization.status.clone();
        authorization.status = new_status.clone();
        authorization.updated_at = Utc::now();

        // Log workflow event
        let workflow_event = AuthorizationWorkflowEvent {
            id: 0,
            authorization_id,
            previous_status: Some(previous_status.clone()),
            new_status: new_status.clone(),
            change_reason: change_reason.clone(),
            supporting_documentation: None,
            changed_by: changed_by.clone(),
            approved_by: None,
            legal_review_completed: false,
            law_enforcement_notified: false,
            notification_method: None,
            notification_date: None,
            created_at: Utc::now(),
        };

        // Log audit entry
        self.log_audit_entry(VoiceIntegrityAuditEntry {
            id: 0,
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
        }).await;

        // Handle status-specific actions
        match new_status {
            AuthorizationStatus::Active => {
                self.activate_authorization_targets(authorization_id).await?;
            },
            AuthorizationStatus::Expired | AuthorizationStatus::Revoked => {
                self.deactivate_authorization_targets(authorization_id).await?;
            },
            _ => {}
        }

        // Emit event
        let event = TelecomEvent::LegalAuthorizationUpdated {
            authorization_id: authorization.authorization_number.clone(),
            previous_status: format!("{:?}", previous_status),
            new_status: format!("{:?}", new_status),
            change_reason,
        };
        self.event_bus.publish(event).await?;

        Ok(())
    }

    /// Add lawful intercept target
    #[instrument(skip(self), fields(authorization_id = authorization_id, target_value = %target.target_value))]
    pub async fn add_intercept_target(
        &self,
        authorization_id: i32,
        target: CreateTargetRequest,
    ) -> Result<LawfulInterceptTarget> {
        info!("Adding intercept target for authorization {}: {}", authorization_id, target.target_value);

        // Validate authorization exists and is active
        let authorizations = self.active_authorizations.read().await;
        let authorization = authorizations.get(&authorization_id)
            .ok_or_else(|| anyhow::anyhow!("Authorization not found: {}", authorization_id))?;

        if !matches!(authorization.status, AuthorizationStatus::Active) {
            return Err(anyhow::anyhow!("Authorization {} is not active", authorization_id));
        }

        let now = Utc::now();
        let intercept_target = LawfulInterceptTarget {
            id: 0, // Will be set by database
            authorization_id,
            target_type: target.target_type.clone(),
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

        // Update target cache
        let mut targets = self.active_targets.write().await;
        targets.entry(target.target_type.clone())
            .or_insert_with(Vec::new)
            .push(intercept_target.clone());

        // Log audit entry
        self.log_audit_entry(VoiceIntegrityAuditEntry {
            id: 0,
            user_id: None,
            session_id: None,
            action_type: "add_intercept_target".to_string(),
            resource_type: "lawful_intercept_target".to_string(),
            resource_id: format!("{}:{}", target.target_type, target.target_value),
            authorization_id: Some(authorization_id),
            legal_basis: Some(authorization.authorization_number.clone()),
            business_justification: Some(format!("Target added for authorization: {}",
                authorization.authorization_number)),
            ip_address: None,
            user_agent: None,
            request_details: None,
            response_summary: None,
            ecpa_compliant: true,
            calea_notification_required: true,
            data_minimization_applied: true,
            timestamp: now,
        }).await;

        Ok(intercept_target)
    }

    /// Check if target should be intercepted
    #[instrument(skip(self), fields(target_type = %target_type, target_value = %target_value))]
    pub async fn should_intercept_target(
        &self,
        target_type: &str,
        target_value: &str,
    ) -> Result<Option<LawfulInterceptTarget>> {
        let targets = self.active_targets.read().await;

        if let Some(type_targets) = targets.get(target_type) {
            for target in type_targets {
                if target.target_value == target_value && target.monitoring_enabled {
                    debug!("Target {} requires lawful intercept under authorization {}",
                           target_value, target.authorization_id);
                    return Ok(Some(target.clone()));
                }
            }
        }

        Ok(None)
    }

    /// Log audit entry for compliance tracking
    async fn log_audit_entry(&self, entry: VoiceIntegrityAuditEntry) {
        // In production, this would write to the database
        debug!("Audit log: {:?}", entry);
    }

    /// Validate authorization request
    fn validate_authorization_request(&self, request: &CreateAuthorizationRequest) -> Result<()> {
        // Check duration limits
        let duration = request.expiration_date.signed_duration_since(request.effective_date);
        let max_duration = chrono::Duration::days(self.config.max_authorization_duration_days as i64);

        if duration > max_duration {
            return Err(anyhow::anyhow!(
                "Authorization duration exceeds maximum allowed: {} days",
                self.config.max_authorization_duration_days
            ));
        }

        // Validate required fields
        if request.authorization_number.is_empty() {
            return Err(anyhow::anyhow!("Authorization number is required"));
        }

        if request.scope_description.is_empty() {
            return Err(anyhow::anyhow!("Scope description is required"));
        }

        if request.investigating_agency.is_empty() {
            return Err(anyhow::anyhow!("Investigating agency is required"));
        }

        Ok(())
    }

    /// Activate targets for an authorization
    async fn activate_authorization_targets(&self, authorization_id: i32) -> Result<()> {
        debug!("Activating targets for authorization {}", authorization_id);
        // Implementation would update database to enable monitoring
        Ok(())
    }

    /// Deactivate targets for an authorization
    async fn deactivate_authorization_targets(&self, authorization_id: i32) -> Result<()> {
        debug!("Deactivating targets for authorization {}", authorization_id);
        // Implementation would update database to disable monitoring
        Ok(())
    }

    /// Check for expiring authorizations
    pub async fn check_expiring_authorizations(&self) -> Result<()> {
        let now = Utc::now();
        let warning_threshold = now + chrono::Duration::days(self.config.expiration_warning_days as i64);

        let authorizations = self.active_authorizations.read().await;
        for authorization in authorizations.values() {
            if authorization.expiration_date <= warning_threshold &&
               matches!(authorization.status, AuthorizationStatus::Active) {
                warn!("Authorization {} expires soon: {}",
                      authorization.authorization_number, authorization.expiration_date);

                // Send notification if configured
                if let Some(email) = &self.config.compliance_officer_email {
                    // Would send email notification
                    debug!("Would send expiration warning to: {}", email);
                }
            }
        }

        Ok(())
    }
}

/// Request to create new legal authorization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAuthorizationRequest {
    pub authorization_number: String,
    pub authorization_type: AuthorizationType,
    pub issuing_authority: String,
    pub case_number: Option<String>,
    pub investigating_agency: String,
    pub investigating_officer: String,
    pub contact_information: HashMap<String, String>,
    pub target_identifiers: HashMap<String, Vec<String>>,
    pub target_description: Option<String>,
    pub scope_description: String,
    pub effective_date: DateTime<Utc>,
    pub expiration_date: DateTime<Utc>,
}

/// Request to create new intercept target
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTargetRequest {
    pub target_type: String,
    pub target_value: String,
    pub target_description: Option<String>,
    pub content_intercept_enabled: bool,
    pub retention_days: Option<i32>,
}