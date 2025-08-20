# Lawful Intercept Compliance and Security Guide

## Overview

This document provides comprehensive security and compliance guidance for operating Lawful Intercept (LI) capabilities in Redfire Switch, ensuring adherence to ETSI LI, CALEA, and other regulatory standards.

## Table of Contents

1. [Legal Compliance Framework](#legal-compliance-framework)
2. [Security Architecture](#security-architecture)
3. [Data Protection Requirements](#data-protection-requirements)
4. [Access Control and Authentication](#access-control-and-authentication)
5. [Audit and Monitoring](#audit-and-monitoring)
6. [Incident Response](#incident-response)
7. [Operational Security](#operational-security)
8. [Regulatory Reporting](#regulatory-reporting)

## Legal Compliance Framework

### Regulatory Standards Compliance

#### ETSI TS 102 232 (HI2/HI3 Interfaces)

**Mandatory Requirements:**
- ✅ ASN.1 BER encoding for HI2 messages
- ✅ Secure delivery of intercept data
- ✅ Complete target identification
- ✅ Timestamp accuracy (synchronized clocks)
- ✅ Location information (where available)

**Implementation Verification:**
```rust
// Verify ETSI compliance in production
async fn verify_etsi_compliance() -> ComplianceReport {
    let mut report = ComplianceReport::new();
    
    // Check ASN.1 BER encoding
    let test_hi2 = generate_test_hi2_message();
    match validate_asn1_ber_encoding(&test_hi2) {
        Ok(_) => report.add_pass("ASN.1 BER encoding validated"),
        Err(e) => report.add_fail(&format!("ASN.1 BER encoding failed: {}", e)),
    }
    
    // Check timestamp synchronization
    let time_sync = check_ntp_synchronization().await;
    if time_sync.drift_ms < 100 {
        report.add_pass("Time synchronization within tolerance");
    } else {
        report.add_fail(&format!("Time drift {}ms exceeds limit", time_sync.drift_ms));
    }
    
    // Check delivery endpoint security
    for endpoint in get_delivery_endpoints() {
        match test_endpoint_security(&endpoint).await {
            Ok(_) => report.add_pass(&format!("Endpoint {} security validated", endpoint.lea_id)),
            Err(e) => report.add_fail(&format!("Endpoint {} security failed: {}", endpoint.lea_id, e)),
        }
    }
    
    report
}
```

#### ETSI TS 133 108 (Security Requirements)

**Mandatory Security Controls:**
- ✅ End-to-end encryption for all LI data
- ✅ Mutual authentication with LEA endpoints
- ✅ Digital signatures for data integrity
- ✅ Secure key management (HSM recommended)
- ✅ Non-repudiation mechanisms

**Security Validation:**
```rust
use redfire_switch::etsi_li::security::*;

async fn validate_security_compliance() -> SecurityReport {
    let mut report = SecurityReport::new();
    
    // Validate encryption
    let encryption_test = test_encryption_pipeline().await;
    if encryption_test.all_data_encrypted {
        report.add_pass("All LI data encrypted in transit and at rest");
    } else {
        report.add_critical("Unencrypted LI data detected");
    }
    
    // Check key management
    let key_manager = KeyManager::instance();
    let key_health = key_manager.health_check().await;
    if key_health.hsm_connected && key_health.keys_rotated_recently {
        report.add_pass("Key management system operational");
    } else {
        report.add_warning("Key management requires attention");
    }
    
    // Verify digital signatures
    let signature_test = test_digital_signatures().await;
    if signature_test.all_data_signed {
        report.add_pass("Digital signatures validated");
    } else {
        report.add_fail("Digital signature validation failed");
    }
    
    report
}
```

#### CALEA Section 103 Compliance

**Technical Requirements:**
- ✅ Real-time call-identifying information (CII)
- ✅ Call detail records (CDR) generation
- ✅ Content delivery when legally authorized
- ✅ Electronic surveillance delivery format

**J-STD-025 Implementation:**
```rust
use redfire_switch::j_std_025::compliance::*;

async fn verify_calea_compliance() -> CaleaReport {
    let mut report = CaleaReport::new();
    
    // Check CDR completeness
    let cdr_validator = CdrValidator::new();
    let sample_cdrs = get_sample_cdrs(100).await;
    
    for cdr in sample_cdrs {
        let validation = cdr_validator.validate(&cdr);
        if validation.is_complete() {
            report.add_cdr_validation_pass();
        } else {
            report.add_cdr_validation_fail(validation.missing_fields);
        }
    }
    
    // Check real-time delivery performance
    let delivery_stats = get_delivery_statistics().await;
    if delivery_stats.average_latency_ms < 1000 {
        report.add_pass("Real-time delivery performance acceptable");
    } else {
        report.add_fail(&format!("Delivery latency {}ms exceeds requirement", 
                                delivery_stats.average_latency_ms));
    }
    
    report
}
```

### Legal Authorization Validation

#### Warrant Validation Process

```rust
use redfire_switch::etsi_li::warrant::*;

#[derive(Debug)]
pub struct WarrantValidation {
    pub is_valid: bool,
    pub validation_errors: Vec<String>,
    pub legal_authority_verified: bool,
    pub jurisdiction_confirmed: bool,
    pub date_range_valid: bool,
    pub target_identification_valid: bool,
}

impl WarrantValidator {
    pub async fn comprehensive_validation(&self, warrant: &InterceptWarrant) -> WarrantValidation {
        let mut validation = WarrantValidation {
            is_valid: true,
            validation_errors: Vec::new(),
            legal_authority_verified: false,
            jurisdiction_confirmed: false,
            date_range_valid: false,
            target_identification_valid: false,
        };
        
        // 1. Validate legal authority
        match self.verify_issuing_authority(&warrant.issuing_authority).await {
            Ok(authority) => {
                validation.legal_authority_verified = authority.is_authorized;
                if !authority.is_authorized {
                    validation.validation_errors.push(
                        format!("Issuing authority not recognized: {}", warrant.issuing_authority)
                    );
                    validation.is_valid = false;
                }
            }
            Err(e) => {
                validation.validation_errors.push(format!("Authority verification failed: {}", e));
                validation.is_valid = false;
            }
        }
        
        // 2. Check jurisdiction
        match self.verify_jurisdiction(&warrant.target_identifier).await {
            Ok(jurisdiction) => {
                validation.jurisdiction_confirmed = jurisdiction.is_valid;
                if !jurisdiction.is_valid {
                    validation.validation_errors.push(
                        "Target outside of legal jurisdiction".to_string()
                    );
                    validation.is_valid = false;
                }
            }
            Err(e) => {
                validation.validation_errors.push(format!("Jurisdiction check failed: {}", e));
                validation.is_valid = false;
            }
        }
        
        // 3. Validate date range
        let now = chrono::Utc::now();
        if warrant.start_date <= now && warrant.end_date >= now {
            validation.date_range_valid = true;
        } else {
            validation.validation_errors.push(
                format!("Warrant dates invalid: {} to {}", warrant.start_date, warrant.end_date)
            );
            validation.is_valid = false;
        }
        
        // 4. Validate target identification
        match self.validate_target_identifier(&warrant.target_identifier).await {
            Ok(target_info) => {
                validation.target_identification_valid = target_info.is_valid;
                if !target_info.is_valid {
                    validation.validation_errors.push(
                        format!("Invalid target identifier: {}", warrant.target_identifier)
                    );
                    validation.is_valid = false;
                }
            }
            Err(e) => {
                validation.validation_errors.push(format!("Target validation failed: {}", e));
                validation.is_valid = false;
            }
        }
        
        validation
    }
}
```

## Security Architecture

### Defense in Depth Model

```
┌─────────────────────────────────────────────────────────────────┐
│                        Physical Security                         │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                    Network Security                         │ │
│  │  ┌─────────────────────────────────────────────────────────┐ │ │
│  │  │                 Host Security                           │ │ │
│  │  │  ┌─────────────────────────────────────────────────────┐ │ │ │
│  │  │  │              Application Security                   │ │ │ │
│  │  │  │  ┌─────────────────────────────────────────────────┐ │ │ │ │
│  │  │  │  │             Data Security                       │ │ │ │ │
│  │  │  │  │  ┌─────────────────────────────────────────────┐ │ │ │ │ │
│  │  │  │  │  │          LI Data Processing             │ │ │ │ │ │
│  │  │  │  │  │  • Warrant Validation                   │ │ │ │ │ │
│  │  │  │  │  │  • Data Encryption                      │ │ │ │ │ │
│  │  │  │  │  │  • Access Control                       │ │ │ │ │ │
│  │  │  │  │  │  • Audit Logging                        │ │ │ │ │ │
│  │  │  │  │  └─────────────────────────────────────────────┘ │ │ │ │ │
│  │  │  │  └─────────────────────────────────────────────────┘ │ │ │ │
│  │  │  └─────────────────────────────────────────────────────┘ │ │ │
│  │  └─────────────────────────────────────────────────────────┘ │ │
│  └─────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

### Encryption Implementation

#### AES-256-GCM for LI Data

```rust
use aes_gcm::{Aes256Gcm, Key, Nonce, NewAead, Aead};
use rand::RngCore;

pub struct LiDataEncryption {
    cipher: Aes256Gcm,
    key_id: String,
}

impl LiDataEncryption {
    pub fn new(key_material: &[u8], key_id: String) -> Result<Self> {
        let key = Key::from_slice(key_material);
        let cipher = Aes256Gcm::new(key);
        
        Ok(Self {
            cipher,
            key_id,
        })
    }
    
    pub fn encrypt_li_data(&self, plaintext: &[u8]) -> Result<EncryptedLiData> {
        // Generate unique nonce for each encryption
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        // Encrypt with authentication
        let ciphertext = self.cipher.encrypt(nonce, plaintext)
            .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;
        
        Ok(EncryptedLiData {
            key_id: self.key_id.clone(),
            nonce: nonce_bytes.to_vec(),
            ciphertext,
            timestamp: chrono::Utc::now(),
        })
    }
    
    pub fn decrypt_li_data(&self, encrypted: &EncryptedLiData) -> Result<Vec<u8>> {
        if encrypted.key_id != self.key_id {
            return Err(anyhow::anyhow!("Key ID mismatch"));
        }
        
        let nonce = Nonce::from_slice(&encrypted.nonce);
        let plaintext = self.cipher.decrypt(nonce, encrypted.ciphertext.as_ref())
            .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))?;
        
        Ok(plaintext)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EncryptedLiData {
    pub key_id: String,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
```

#### Hardware Security Module (HSM) Integration

```rust
use pkcs11::{Ctx, CKF_SERIAL_SESSION, CKU_USER};

pub struct HsmKeyManager {
    ctx: Ctx,
    session: pkcs11::types::CK_SESSION_HANDLE,
}

impl HsmKeyManager {
    pub async fn initialize(pkcs11_lib_path: &str, pin: &str) -> Result<Self> {
        let ctx = Ctx::new_and_initialize(pkcs11_lib_path)
            .map_err(|e| anyhow::anyhow!("HSM initialization failed: {}", e))?;
        
        let slots = ctx.get_slot_list(true)
            .map_err(|e| anyhow::anyhow!("Failed to get HSM slots: {}", e))?;
        
        if slots.is_empty() {
            return Err(anyhow::anyhow!("No HSM slots available"));
        }
        
        let session = ctx.open_session(slots[0], CKF_SERIAL_SESSION, None, None)
            .map_err(|e| anyhow::anyhow!("Failed to open HSM session: {}", e))?;
        
        ctx.login(session, CKU_USER, Some(pin))
            .map_err(|e| anyhow::anyhow!("HSM login failed: {}", e))?;
        
        Ok(Self { ctx, session })
    }
    
    pub async fn generate_lea_key(&self, lea_id: &str) -> Result<String> {
        use pkcs11::types::*;
        
        // Generate AES-256 key in HSM
        let key_template = vec![
            CK_ATTRIBUTE::new(CKA_CLASS).with_ck_ulong(&CKO_SECRET_KEY),
            CK_ATTRIBUTE::new(CKA_KEY_TYPE).with_ck_ulong(&CKK_AES),
            CK_ATTRIBUTE::new(CKA_VALUE_LEN).with_ck_ulong(&32), // 256 bits
            CK_ATTRIBUTE::new(CKA_ENCRYPT).with_bool(&true),
            CK_ATTRIBUTE::new(CKA_DECRYPT).with_bool(&true),
            CK_ATTRIBUTE::new(CKA_LABEL).with_string(lea_id),
        ];
        
        let key_handle = self.ctx.generate_key(
            self.session,
            &CK_MECHANISM::new(CKM_AES_KEY_GEN),
            &key_template
        ).map_err(|e| anyhow::anyhow!("Key generation failed: {}", e))?;
        
        // Return key handle as string ID
        Ok(format!("HSM-{}-{}", lea_id, key_handle))
    }
    
    pub async fn encrypt_with_hsm_key(
        &self, 
        key_id: &str, 
        plaintext: &[u8]
    ) -> Result<Vec<u8>> {
        use pkcs11::types::*;
        
        // Parse HSM key handle from key_id
        let key_handle = self.parse_key_handle(key_id)?;
        
        // Initialize encryption
        let mechanism = CK_MECHANISM::new(CKM_AES_GCM);
        self.ctx.encrypt_init(self.session, &mechanism, key_handle)
            .map_err(|e| anyhow::anyhow!("Encryption init failed: {}", e))?;
        
        // Encrypt data
        let ciphertext = self.ctx.encrypt(self.session, plaintext)
            .map_err(|e| anyhow::anyhow!("HSM encryption failed: {}", e))?;
        
        Ok(ciphertext)
    }
    
    fn parse_key_handle(&self, key_id: &str) -> Result<CK_OBJECT_HANDLE> {
        // Parse "HSM-LEA-001-123456" format
        let parts: Vec<&str> = key_id.split('-').collect();
        if parts.len() < 4 || parts[0] != "HSM" {
            return Err(anyhow::anyhow!("Invalid HSM key ID format"));
        }
        
        let handle_str = parts.last().unwrap();
        let handle = handle_str.parse::<CK_OBJECT_HANDLE>()
            .map_err(|_| anyhow::anyhow!("Invalid key handle"))?;
        
        Ok(handle)
    }
}
```

## Data Protection Requirements

### Data Classification

#### LI Data Categories

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LiDataClassification {
    /// Call metadata (HI2)
    InterceptRelatedInformation {
        sensitivity_level: SensitivityLevel,
        retention_period: chrono::Duration,
        access_restrictions: Vec<String>,
    },
    /// Actual call content (HI3)
    InterceptedContent {
        sensitivity_level: SensitivityLevel,
        retention_period: chrono::Duration,
        access_restrictions: Vec<String>,
        content_type: ContentType,
    },
    /// Call detail records
    CallDetailRecord {
        sensitivity_level: SensitivityLevel,
        retention_period: chrono::Duration,
        billing_correlation: Option<String>,
    },
    /// System audit data
    AuditTrail {
        sensitivity_level: SensitivityLevel,
        retention_period: chrono::Duration,
        immutable: bool,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SensitivityLevel {
    Public,
    Internal,
    Confidential,
    Restricted,
    TopSecret,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ContentType {
    Voice,
    Video,
    Data,
    Multimedia,
    Messaging,
}
```

### Data Retention and Destruction

```rust
use tokio_cron_scheduler::{JobScheduler, Job};

pub struct LiDataRetentionManager {
    scheduler: JobScheduler,
    data_store: Arc<dyn LiDataStore>,
}

impl LiDataRetentionManager {
    pub async fn new(data_store: Arc<dyn LiDataStore>) -> Result<Self> {
        let scheduler = JobScheduler::new().await?;
        
        Ok(Self {
            scheduler,
            data_store,
        })
    }
    
    pub async fn start_retention_management(&self) -> Result<()> {
        // Daily cleanup job
        let data_store = self.data_store.clone();
        let cleanup_job = Job::new_async("0 2 * * *", move |_uuid, _l| {
            let data_store = data_store.clone();
            Box::pin(async move {
                if let Err(e) = Self::cleanup_expired_data(&data_store).await {
                    error!("Data cleanup failed: {}", e);
                } else {
                    info!("Daily data cleanup completed successfully");
                }
            })
        })?;
        
        self.scheduler.add(cleanup_job).await?;
        
        // Weekly audit
        let audit_data_store = self.data_store.clone();
        let audit_job = Job::new_async("0 3 * * 0", move |_uuid, _l| {
            let data_store = audit_data_store.clone();
            Box::pin(async move {
                if let Err(e) = Self::audit_data_retention(&data_store).await {
                    error!("Retention audit failed: {}", e);
                } else {
                    info!("Weekly retention audit completed");
                }
            })
        })?;
        
        self.scheduler.add(audit_job).await?;
        self.scheduler.start().await?;
        
        Ok(())
    }
    
    async fn cleanup_expired_data(data_store: &Arc<dyn LiDataStore>) -> Result<()> {
        let now = chrono::Utc::now();
        
        // Get all data with expiration dates
        let expirable_data = data_store.get_expirable_data().await?;
        
        let mut cleanup_stats = CleanupStats::new();
        
        for data_item in expirable_data {
            match data_item.classification {
                LiDataClassification::InterceptRelatedInformation { retention_period, .. } => {
                    if data_item.created_at + retention_period < now {
                        Self::secure_delete_hi2_data(data_store, &data_item.id).await?;
                        cleanup_stats.hi2_deleted += 1;
                    }
                }
                LiDataClassification::InterceptedContent { retention_period, .. } => {
                    if data_item.created_at + retention_period < now {
                        Self::secure_delete_hi3_data(data_store, &data_item.id).await?;
                        cleanup_stats.hi3_deleted += 1;
                    }
                }
                LiDataClassification::CallDetailRecord { retention_period, .. } => {
                    if data_item.created_at + retention_period < now {
                        Self::secure_delete_cdr_data(data_store, &data_item.id).await?;
                        cleanup_stats.cdrs_deleted += 1;
                    }
                }
                LiDataClassification::AuditTrail { retention_period, immutable, .. } => {
                    if !immutable && data_item.created_at + retention_period < now {
                        // Audit trails may have longer retention requirements
                        Self::archive_audit_data(data_store, &data_item.id).await?;
                        cleanup_stats.audit_archived += 1;
                    }
                }
            }
        }
        
        info!("Data cleanup completed: {:?}", cleanup_stats);
        Ok(())
    }
    
    async fn secure_delete_hi2_data(
        data_store: &Arc<dyn LiDataStore>,
        data_id: &str
    ) -> Result<()> {
        // Multi-pass secure deletion for HI2 data
        for pass in 0..3 {
            match pass {
                0 => data_store.overwrite_with_random(data_id).await?,
                1 => data_store.overwrite_with_zeros(data_id).await?,
                2 => data_store.overwrite_with_ones(data_id).await?,
                _ => unreachable!(),
            }
        }
        
        data_store.final_delete(data_id).await?;
        
        // Log secure deletion
        audit_log!(
            "SECURE_DELETE",
            "data_type" => "HI2",
            "data_id" => data_id,
            "deletion_method" => "3_pass_overwrite"
        );
        
        Ok(())
    }
    
    async fn audit_data_retention(data_store: &Arc<dyn LiDataStore>) -> Result<()> {
        let retention_report = data_store.generate_retention_report().await?;
        
        // Check for data approaching expiration
        let soon_to_expire = retention_report.get_expiring_within_days(30);
        if !soon_to_expire.is_empty() {
            warn!("Data items expiring within 30 days: {}", soon_to_expire.len());
            for item in soon_to_expire {
                info!("Expiring: {} ({})", item.id, item.expiration_date);
            }
        }
        
        // Check for overdue deletions
        let overdue = retention_report.get_overdue_deletions();
        if !overdue.is_empty() {
            error!("Overdue data deletions detected: {}", overdue.len());
            // Alert operations team
            send_alert("Overdue LI data deletions require immediate attention").await?;
        }
        
        // Generate compliance report
        let compliance_report = generate_retention_compliance_report(&retention_report);
        save_compliance_report(&compliance_report).await?;
        
        Ok(())
    }
}

#[derive(Debug)]
struct CleanupStats {
    pub hi2_deleted: u64,
    pub hi3_deleted: u64,
    pub cdrs_deleted: u64,
    pub audit_archived: u64,
}

impl CleanupStats {
    fn new() -> Self {
        Self {
            hi2_deleted: 0,
            hi3_deleted: 0,
            cdrs_deleted: 0,
            audit_archived: 0,
        }
    }
}
```

## Access Control and Authentication

### Role-Based Access Control (RBAC)

```rust
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiAccessControl {
    roles: HashMap<String, Role>,
    users: HashMap<String, User>,
    sessions: HashMap<String, Session>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    name: String,
    permissions: Vec<Permission>,
    description: String,
    max_session_duration: chrono::Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    username: String,
    roles: Vec<String>,
    security_clearance: SecurityClearance,
    mfa_enabled: bool,
    last_login: Option<chrono::DateTime<chrono::Utc>>,
    failed_login_attempts: u32,
    account_locked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permission {
    resource: String,
    actions: Vec<String>,
    conditions: Option<PermissionConditions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionConditions {
    time_restrictions: Option<TimeRestrictions>,
    ip_restrictions: Option<Vec<std::net::IpAddr>>,
    location_restrictions: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityClearance {
    Basic,
    Confidential,
    Secret,
    TopSecret,
}

impl LiAccessControl {
    pub fn new() -> Self {
        let mut roles = HashMap::new();
        
        // Define standard LI roles
        roles.insert("li_viewer".to_string(), Role {
            name: "LI Viewer".to_string(),
            permissions: vec![
                Permission {
                    resource: "warrants".to_string(),
                    actions: vec!["read".to_string()],
                    conditions: None,
                },
                Permission {
                    resource: "li_data".to_string(),
                    actions: vec!["read".to_string()],
                    conditions: Some(PermissionConditions {
                        time_restrictions: Some(TimeRestrictions::business_hours()),
                        ip_restrictions: None,
                        location_restrictions: None,
                    }),
                },
            ],
            description: "View-only access to LI data and warrants".to_string(),
            max_session_duration: chrono::Duration::hours(4),
        });
        
        roles.insert("li_operator".to_string(), Role {
            name: "LI Operator".to_string(),
            permissions: vec![
                Permission {
                    resource: "warrants".to_string(),
                    actions: vec!["read".to_string(), "activate".to_string(), "suspend".to_string()],
                    conditions: None,
                },
                Permission {
                    resource: "li_data".to_string(),
                    actions: vec!["read".to_string(), "export".to_string()],
                    conditions: None,
                },
                Permission {
                    resource: "delivery".to_string(),
                    actions: vec!["read".to_string(), "test".to_string()],
                    conditions: None,
                },
            ],
            description: "Operational control of LI systems".to_string(),
            max_session_duration: chrono::Duration::hours(8),
        });
        
        roles.insert("li_administrator".to_string(), Role {
            name: "LI Administrator".to_string(),
            permissions: vec![
                Permission {
                    resource: "*".to_string(),
                    actions: vec!["*".to_string()],
                    conditions: Some(PermissionConditions {
                        time_restrictions: None,
                        ip_restrictions: Some(vec!["192.168.1.100".parse().unwrap()]),
                        location_restrictions: Some(vec!["secure_operations_center".to_string()]),
                    }),
                },
            ],
            description: "Full administrative access to LI systems".to_string(),
            max_session_duration: chrono::Duration::hours(12),
        });
        
        Self {
            roles,
            users: HashMap::new(),
            sessions: HashMap::new(),
        }
    }
    
    pub async fn authenticate_user(
        &mut self,
        username: &str,
        password: &str,
        mfa_token: Option<&str>,
        client_ip: std::net::IpAddr
    ) -> Result<AuthenticationResult> {
        let user = self.users.get_mut(username)
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;
        
        // Check if account is locked
        if user.account_locked {
            audit_log!(
                "AUTH_FAILED",
                "username" => username,
                "reason" => "account_locked",
                "client_ip" => client_ip.to_string()
            );
            return Ok(AuthenticationResult::AccountLocked);
        }
        
        // Verify password (in production, use proper password hashing)
        if !self.verify_password(username, password).await? {
            user.failed_login_attempts += 1;
            
            if user.failed_login_attempts >= 3 {
                user.account_locked = true;
                audit_log!(
                    "ACCOUNT_LOCKED",
                    "username" => username,
                    "reason" => "too_many_failed_attempts",
                    "client_ip" => client_ip.to_string()
                );
            }
            
            audit_log!(
                "AUTH_FAILED",
                "username" => username,
                "reason" => "invalid_password",
                "client_ip" => client_ip.to_string()
            );
            return Ok(AuthenticationResult::InvalidCredentials);
        }
        
        // Verify MFA if enabled
        if user.mfa_enabled {
            match mfa_token {
                Some(token) => {
                    if !self.verify_mfa_token(username, token).await? {
                        audit_log!(
                            "AUTH_FAILED",
                            "username" => username,
                            "reason" => "invalid_mfa_token",
                            "client_ip" => client_ip.to_string()
                        );
                        return Ok(AuthenticationResult::InvalidMfaToken);
                    }
                }
                None => {
                    audit_log!(
                        "AUTH_FAILED",
                        "username" => username,
                        "reason" => "mfa_token_required",
                        "client_ip" => client_ip.to_string()
                    );
                    return Ok(AuthenticationResult::MfaTokenRequired);
                }
            }
        }
        
        // Create session
        let session_id = uuid::Uuid::new_v4().to_string();
        let max_duration = self.get_max_session_duration(user);
        let session = Session {
            id: session_id.clone(),
            username: username.to_string(),
            created_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + max_duration,
            client_ip,
            permissions: self.get_user_permissions(user),
        };
        
        self.sessions.insert(session_id.clone(), session);
        
        // Reset failed login attempts
        user.failed_login_attempts = 0;
        user.last_login = Some(chrono::Utc::now());
        
        audit_log!(
            "AUTH_SUCCESS",
            "username" => username,
            "session_id" => session_id.clone(),
            "client_ip" => client_ip.to_string()
        );
        
        Ok(AuthenticationResult::Success { session_id })
    }
    
    pub fn check_permission(
        &self,
        session_id: &str,
        resource: &str,
        action: &str
    ) -> Result<bool> {
        let session = self.sessions.get(session_id)
            .ok_or_else(|| anyhow::anyhow!("Invalid session"))?;
        
        // Check session expiration
        if chrono::Utc::now() > session.expires_at {
            return Err(anyhow::anyhow!("Session expired"));
        }
        
        // Check permissions
        for permission in &session.permissions {
            if permission.resource == "*" || permission.resource == resource {
                if permission.actions.contains(&"*".to_string()) || permission.actions.contains(&action.to_string()) {
                    // Check conditions if present
                    if let Some(conditions) = &permission.conditions {
                        if !self.check_permission_conditions(conditions, session)? {
                            continue;
                        }
                    }
                    
                    audit_log!(
                        "PERMISSION_GRANTED",
                        "session_id" => session_id,
                        "username" => &session.username,
                        "resource" => resource,
                        "action" => action
                    );
                    
                    return Ok(true);
                }
            }
        }
        
        audit_log!(
            "PERMISSION_DENIED",
            "session_id" => session_id,
            "username" => &session.username,
            "resource" => resource,
            "action" => action
        );
        
        Ok(false)
    }
    
    fn check_permission_conditions(
        &self,
        conditions: &PermissionConditions,
        session: &Session
    ) -> Result<bool> {
        // Check IP restrictions
        if let Some(allowed_ips) = &conditions.ip_restrictions {
            if !allowed_ips.contains(&session.client_ip) {
                return Ok(false);
            }
        }
        
        // Check time restrictions
        if let Some(time_restrictions) = &conditions.time_restrictions {
            let now = chrono::Utc::now();
            let current_time = now.time();
            let current_day = now.weekday();
            
            if !time_restrictions.is_allowed(current_day, current_time) {
                return Ok(false);
            }
        }
        
        Ok(true)
    }
}

#[derive(Debug)]
pub enum AuthenticationResult {
    Success { session_id: String },
    InvalidCredentials,
    AccountLocked,
    MfaTokenRequired,
    InvalidMfaToken,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub username: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub client_ip: std::net::IpAddr,
    pub permissions: Vec<Permission>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRestrictions {
    allowed_days: Vec<chrono::Weekday>,
    allowed_hours_start: chrono::NaiveTime,
    allowed_hours_end: chrono::NaiveTime,
}

impl TimeRestrictions {
    pub fn business_hours() -> Self {
        Self {
            allowed_days: vec![
                chrono::Weekday::Mon,
                chrono::Weekday::Tue,
                chrono::Weekday::Wed,
                chrono::Weekday::Thu,
                chrono::Weekday::Fri,
            ],
            allowed_hours_start: chrono::NaiveTime::from_hms(9, 0, 0),
            allowed_hours_end: chrono::NaiveTime::from_hms(17, 0, 0),
        }
    }
    
    pub fn is_allowed(&self, day: chrono::Weekday, time: chrono::NaiveTime) -> bool {
        self.allowed_days.contains(&day) &&
        time >= self.allowed_hours_start &&
        time <= self.allowed_hours_end
    }
}
```

## Audit and Monitoring

### Comprehensive Audit Logging

```rust
use serde_json::json;
use tracing::{info, warn, error};

// Macro for structured audit logging
macro_rules! audit_log {
    ($event_type:expr, $($key:expr => $value:expr),*) => {
        {
            let audit_event = json!({
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "event_type": $event_type,
                "source": "redfire_li_system",
                "version": env!("CARGO_PKG_VERSION"),
                $(
                    $key: $value,
                )*
            });
            
            // Log to structured audit trail
            info!(target: "audit", "{}", audit_event);
            
            // Also store in tamper-evident audit database
            if let Err(e) = store_audit_event(&audit_event) {
                error!("Failed to store audit event: {}", e);
            }
        }
    };
}

pub struct AuditLogger {
    audit_db: Arc<dyn AuditDatabase>,
    integrity_checker: IntegrityChecker,
}

impl AuditLogger {
    pub async fn new(audit_db: Arc<dyn AuditDatabase>) -> Result<Self> {
        let integrity_checker = IntegrityChecker::new().await?;
        
        Ok(Self {
            audit_db,
            integrity_checker,
        })
    }
    
    pub async fn log_warrant_action(
        &self,
        action: WarrantAction,
        warrant_id: &str,
        user_id: &str,
        details: Option<&str>
    ) -> Result<()> {
        let audit_entry = AuditEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            event_type: AuditEventType::WarrantAction,
            category: AuditCategory::LegalCompliance,
            user_id: user_id.to_string(),
            resource_id: warrant_id.to_string(),
            action: format!("{:?}", action),
            details: details.map(|s| s.to_string()),
            ip_address: None, // Set by caller if available
            user_agent: None,
            session_id: None, // Set by caller if available
            integrity_hash: String::new(), // Calculated below
        };
        
        let entry_with_hash = self.integrity_checker.add_integrity_hash(audit_entry).await?;
        self.audit_db.store_audit_entry(&entry_with_hash).await?;
        
        // Real-time monitoring alert for critical actions
        match action {
            WarrantAction::Added | WarrantAction::Activated | WarrantAction::Deactivated => {
                self.send_real_time_alert(&entry_with_hash).await?;
            }
            _ => {}
        }
        
        Ok(())
    }
    
    pub async fn log_data_access(
        &self,
        data_type: LiDataType,
        data_id: &str,
        user_id: &str,
        access_type: DataAccessType,
        session_id: Option<&str>
    ) -> Result<()> {
        let audit_entry = AuditEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            event_type: AuditEventType::DataAccess,
            category: AuditCategory::DataProtection,
            user_id: user_id.to_string(),
            resource_id: data_id.to_string(),
            action: format!("{:?}-{:?}", data_type, access_type),
            details: Some(format!("Data type: {:?}, Access: {:?}", data_type, access_type)),
            ip_address: None,
            user_agent: None,
            session_id: session_id.map(|s| s.to_string()),
            integrity_hash: String::new(),
        };
        
        let entry_with_hash = self.integrity_checker.add_integrity_hash(audit_entry).await?;
        self.audit_db.store_audit_entry(&entry_with_hash).await?;
        
        // Alert for sensitive data access
        if matches!(data_type, LiDataType::InterceptedContent) {
            self.send_sensitive_access_alert(&entry_with_hash).await?;
        }
        
        Ok(())
    }
    
    pub async fn verify_audit_integrity(&self, from_date: chrono::DateTime<chrono::Utc>) -> Result<IntegrityReport> {
        let entries = self.audit_db.get_audit_entries_since(from_date).await?;
        let mut report = IntegrityReport::new();
        
        for entry in entries {
            match self.integrity_checker.verify_integrity(&entry).await {
                Ok(valid) => {
                    if valid {
                        report.verified_entries += 1;
                    } else {
                        report.tampered_entries += 1;
                        report.tampered_entry_ids.push(entry.id.clone());
                        
                        // Critical security alert for tampered audit logs
                        error!("Audit log tampering detected: {}", entry.id);
                        self.send_critical_security_alert(&entry).await?;
                    }
                }
                Err(e) => {
                    report.verification_errors += 1;
                    warn!("Could not verify audit entry {}: {}", entry.id, e);
                }
            }
        }
        
        Ok(report)
    }
    
    async fn send_real_time_alert(&self, entry: &AuditEntry) -> Result<()> {
        let alert = SecurityAlert {
            severity: AlertSeverity::High,
            category: AlertCategory::ComplianceEvent,
            message: format!("Critical LI action: {} on warrant {}", entry.action, entry.resource_id),
            timestamp: entry.timestamp,
            source: "audit_logger".to_string(),
            details: json!({
                "audit_entry_id": entry.id,
                "user": entry.user_id,
                "action": entry.action,
                "resource": entry.resource_id
            }),
        };
        
        send_security_alert(alert).await
    }
    
    async fn send_sensitive_access_alert(&self, entry: &AuditEntry) -> Result<()> {
        let alert = SecurityAlert {
            severity: AlertSeverity::Medium,
            category: AlertCategory::DataAccess,
            message: format!("Sensitive LI data accessed by user {}", entry.user_id),
            timestamp: entry.timestamp,
            source: "audit_logger".to_string(),
            details: json!({
                "audit_entry_id": entry.id,
                "user": entry.user_id,
                "data_id": entry.resource_id,
                "session": entry.session_id
            }),
        };
        
        send_security_alert(alert).await
    }
}

#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub event_type: AuditEventType,
    pub category: AuditCategory,
    pub user_id: String,
    pub resource_id: String,
    pub action: String,
    pub details: Option<String>,
    pub ip_address: Option<std::net::IpAddr>,
    pub user_agent: Option<String>,
    pub session_id: Option<String>,
    pub integrity_hash: String,
}

#[derive(Debug, Clone)]
pub enum AuditEventType {
    WarrantAction,
    DataAccess,
    SystemAccess,
    ConfigurationChange,
    SecurityEvent,
}

#[derive(Debug, Clone)]
pub enum AuditCategory {
    LegalCompliance,
    DataProtection,
    AccessControl,
    SystemSecurity,
}

#[derive(Debug, Clone)]
pub enum WarrantAction {
    Added,
    Modified,
    Activated,
    Deactivated,
    Expired,
    Deleted,
    Viewed,
}

#[derive(Debug, Clone)]
pub enum LiDataType {
    InterceptRelatedInformation,
    InterceptedContent,
    CallDetailRecord,
    SystemConfiguration,
}

#[derive(Debug, Clone)]
pub enum DataAccessType {
    Read,
    Export,
    Delete,
    Modify,
}

#[derive(Debug)]
pub struct IntegrityReport {
    pub verified_entries: u64,
    pub tampered_entries: u64,
    pub verification_errors: u64,
    pub tampered_entry_ids: Vec<String>,
}

impl IntegrityReport {
    fn new() -> Self {
        Self {
            verified_entries: 0,
            tampered_entries: 0,
            verification_errors: 0,
            tampered_entry_ids: Vec::new(),
        }
    }
}
```

This comprehensive compliance and security guide provides the framework for secure and legally compliant operation of lawful intercept capabilities. The documentation covers all critical aspects from legal authorization to technical implementation and ongoing security monitoring.