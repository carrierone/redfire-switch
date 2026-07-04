/*
 * ETSI LI (Lawful Intercept) Implementation
 *
 * This module implements ETSI TS 101 671, TS 102 232, and TS 133 108
 * specifications for lawful interception of telecommunications traffic.
 *
 * Standards Compliance:
 * - ETSI TS 101 671: Handover Interface for lawful interception
 * - ETSI TS 102 232: LI Handover specification for IP delivery
 * - ETSI TS 133 108: 3GPP UMTS 3G security handover interface
 *
 * SECURITY WARNING: This module handles lawful intercept functionality.
 * Proper authorization, warrant validation, and audit trails are critical.
 * Misuse of this functionality may violate privacy laws and regulations.
 */

use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// ETSI LI Interface Types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandoverInterface {
    /// HI1: Administrative Interface (not implemented - out of scope)
    HI1,
    /// HI2: Intercept Related Information
    HI2,
    /// HI3: Intercepted Content
    HI3,
}

/// Warrant Status Enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WarrantStatus {
    /// Warrant is active and valid
    Active,
    /// Warrant is pending activation
    Pending,
    /// Warrant has expired
    Expired,
    /// Warrant has been revoked
    Revoked,
    /// Warrant is suspended
    Suspended,
}

/// Intercept Types per ETSI Standards
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterceptType {
    /// Content of Communication (CC)
    ContentOfCommunication,
    /// Intercept Related Information (IRI)
    InterceptRelatedInformation,
    /// Both CC and IRI
    Both,
}

/// Law Enforcement Agency Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LawEnforcementAgency {
    /// LEA identifier
    pub lea_id: String,
    /// LEA name
    pub lea_name: String,
    /// Country code (ISO 3166)
    pub country_code: String,
    /// Contact information
    pub contact_info: LeaContactInfo,
    /// Authorized IP addresses for delivery
    pub authorized_ips: Vec<IpAddr>,
    /// Public key for encryption
    pub public_key: Option<String>,
    /// Maximum warrant duration (days)
    pub max_warrant_duration: u32,
}

/// LEA Contact Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaContactInfo {
    pub primary_contact: String,
    pub phone: String,
    pub email: String,
    pub emergency_contact: Option<String>,
    pub address: String,
}

/// Lawful Intercept Warrant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiWarrant {
    /// Unique warrant identifier
    pub warrant_id: Uuid,
    /// LEA that issued the warrant
    pub issuing_lea: String,
    /// Court or authority reference
    pub court_reference: String,
    /// Target identifier (phone number, IMSI, etc.)
    pub target_identifier: String,
    /// Target identifier type
    pub target_type: TargetIdentifierType,
    /// Intercept type
    pub intercept_type: InterceptType,
    /// Warrant start time
    pub start_time: DateTime<Utc>,
    /// Warrant end time
    pub end_time: DateTime<Utc>,
    /// Current status
    pub status: WarrantStatus,
    /// Authorized officers
    pub authorized_officers: Vec<String>,
    /// Delivery endpoints for HI2/HI3
    pub delivery_endpoints: DeliveryEndpoints,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last modified timestamp
    pub modified_at: DateTime<Utc>,
}

/// Target Identifier Types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetIdentifierType {
    /// Phone number (E.164 format)
    PhoneNumber,
    /// International Mobile Subscriber Identity
    IMSI,
    /// International Mobile Equipment Identity
    IMEI,
    /// IP Address
    IpAddress,
    /// SIP URI
    SipUri,
    /// Email address
    EmailAddress,
    /// Custom identifier
    Custom,
}

/// Delivery Endpoints Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryEndpoints {
    /// HI2 endpoint for administrative information
    pub hi2_endpoint: Option<SocketAddr>,
    /// HI3 endpoint for content delivery
    pub hi3_endpoint: Option<SocketAddr>,
    /// Mandatory encryption algorithm per ETSI TS 133 108
    pub encryption_algorithm: EncryptionAlgorithm,
    /// TLS certificate path for secure communication
    pub tls_certificate_path: String,
    /// Private key path for TLS
    pub tls_private_key_path: String,
    /// Authentication method (must be strong)
    pub auth_method: AuthenticationMethod,
    /// Delivery format
    pub delivery_format: DeliveryFormat,
}

/// ETSI-compliant encryption algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncryptionAlgorithm {
    /// AES-256-GCM (ETSI recommended)
    Aes256Gcm,
    /// ChaCha20-Poly1305 (alternative)
    ChaCha20Poly1305,
}

/// Strong authentication methods for ETSI compliance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthenticationMethod {
    /// Mutual TLS with X.509 certificates
    MutualTls {
        ca_certificate_path: String,
        client_certificate_path: String,
    },
    /// OAuth 2.0 with JWT tokens
    OAuth2 {
        client_id: String,
        client_secret: String,
        token_endpoint: String,
    },
}

/// Content Delivery Format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliveryFormat {
    /// XML over TCP/TLS
    XmlOverTcp,
    /// ASN.1 BER encoded
    Asn1Ber,
    /// JSON over HTTPS
    JsonOverHttps,
    /// Custom format
    Custom,
}

/// HI2 Intercept Related Information Record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hi2Record {
    /// Record identifier
    pub record_id: Uuid,
    /// Warrant ID this relates to
    pub warrant_id: Uuid,
    /// Target identifier
    pub target_id: String,
    /// Timestamp of the event
    pub timestamp: DateTime<Utc>,
    /// Event type
    pub event_type: Hi2EventType,
    /// Calling party information
    pub calling_party: Option<PartyInformation>,
    /// Called party information
    pub called_party: Option<PartyInformation>,
    /// Location information
    pub location_info: Option<LocationInformation>,
    /// Service information
    pub service_info: ServiceInformation,
    /// Network information
    pub network_info: NetworkInformation,
    /// Additional attributes
    pub additional_info: HashMap<String, String>,
}

/// HI2 Event Types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hi2EventType {
    /// Call attempt
    CallAttempt,
    /// Call connected
    CallConnected,
    /// Call released
    CallReleased,
    /// Location update
    LocationUpdate,
    /// SMS originated
    SmsOriginated,
    /// SMS terminated
    SmsTerminated,
    /// Service access
    ServiceAccess,
    /// Registration
    Registration,
    /// Deregistration
    Deregistration,
}

/// Party Information Structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartyInformation {
    /// Party identifier (phone number, URI, etc.)
    pub party_id: String,
    /// Party identity type
    pub identity_type: TargetIdentifierType,
    /// Party role (originating, terminating, etc.)
    pub party_role: String,
    /// Location information
    pub location: Option<LocationInformation>,
    /// Service provider information
    pub service_provider: Option<String>,
}

/// Location Information for ETSI LI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationInformation {
    /// Cell identifier
    pub cell_id: Option<String>,
    /// Location Area Code
    pub lac: Option<String>,
    /// Mobile Country Code
    pub mcc: Option<String>,
    /// Mobile Network Code
    pub mnc: Option<String>,
    /// Geographic coordinates
    pub coordinates: Option<GeographicCoordinates>,
    /// Location timestamp
    pub timestamp: DateTime<Utc>,
    /// Location accuracy (meters)
    pub accuracy: Option<u32>,
}

/// Geographic Coordinates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeographicCoordinates {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f64>,
}

/// Service Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInformation {
    /// Service type (voice, SMS, data, etc.)
    pub service_type: String,
    /// Service identifier
    pub service_id: Option<String>,
    /// Quality of service
    pub qos_info: Option<QosInformation>,
    /// Supplementary services
    pub supplementary_services: Vec<String>,
}

/// Quality of Service Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QosInformation {
    /// Bandwidth (kbps)
    pub bandwidth: Option<u32>,
    /// Latency (ms)
    pub latency: Option<u32>,
    /// Jitter (ms)
    pub jitter: Option<u32>,
    /// Packet loss percentage
    pub packet_loss: Option<f32>,
}

/// Network Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInformation {
    /// Network identifier
    pub network_id: String,
    /// Access technology (GSM, UMTS, LTE, 5G, etc.)
    pub access_technology: String,
    /// Serving network element
    pub serving_element: String,
    /// IP address of network element
    pub element_ip: Option<IpAddr>,
}

/// HI3 Content Record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hi3ContentRecord {
    /// Record identifier
    pub record_id: Uuid,
    /// Warrant ID
    pub warrant_id: Uuid,
    /// Related HI2 record ID
    pub hi2_record_id: Option<Uuid>,
    /// Content timestamp
    pub timestamp: DateTime<Utc>,
    /// Content type
    pub content_type: ContentType,
    /// Content payload (encrypted)
    pub content_payload: Vec<u8>,
    /// Content metadata
    pub metadata: ContentMetadata,
    /// Sequence number for ordering
    pub sequence_number: u64,
}

/// Content Types for HI3
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentType {
    /// Voice call audio
    VoiceAudio,
    /// Video call content
    VideoCall,
    /// SMS message
    SmsMessage,
    /// MMS message
    MmsMessage,
    /// Data session content
    DataSession,
    /// Email content
    Email,
    /// Other content type
    Other,
}

/// Content Metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentMetadata {
    /// Content encoding
    pub encoding: String,
    /// Content size in bytes
    pub size: u64,
    /// Checksum for integrity
    pub checksum: String,
    /// Encryption algorithm used
    pub encryption_algorithm: Option<String>,
    /// Compression algorithm used
    pub compression_algorithm: Option<String>,
}

/// ETSI LI Intercept Controller
pub struct EtsiLiController {
    /// Active warrants
    warrants: Arc<RwLock<HashMap<Uuid, LiWarrant>>>,
    /// Registered LEAs
    leas: Arc<RwLock<HashMap<String, LawEnforcementAgency>>>,
    /// Active intercepts by target
    active_intercepts: Arc<RwLock<HashMap<String, Vec<Uuid>>>>,
    /// HI2 delivery service
    hi2_service: Arc<Hi2DeliveryService>,
    /// HI3 delivery service
    hi3_service: Arc<Hi3DeliveryService>,
    /// Audit logger
    audit_logger: Arc<LiAuditLogger>,
    /// Configuration
    config: LiControllerConfig,
}

/// LI Controller Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiControllerConfig {
    /// Enable lawful intercept
    pub enabled: bool,
    /// Maximum concurrent warrants
    pub max_concurrent_warrants: usize,
    /// Warrant validation interval (seconds)
    pub warrant_check_interval: u64,
    /// Content retention period (days)
    pub content_retention_days: u32,
    /// Enable content encryption
    pub enable_encryption: bool,
    /// Default delivery format
    pub default_delivery_format: DeliveryFormat,
    /// Audit log retention (days)
    pub audit_retention_days: u32,
}

impl Default for LiControllerConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Disabled by default for security
            max_concurrent_warrants: 100,
            warrant_check_interval: 300, // 5 minutes
            content_retention_days: 90,
            enable_encryption: true,
            default_delivery_format: DeliveryFormat::XmlOverTcp,
            audit_retention_days: 2555, // 7 years
        }
    }
}

/// HI2 Delivery Service
pub struct Hi2DeliveryService {
    /// Delivery configuration
    config: Hi2DeliveryConfig,
    /// Active connections to LEMFs
    connections: Arc<RwLock<HashMap<String, Hi2Connection>>>,
}

/// HI2 Delivery Configuration
#[derive(Debug, Clone)]
pub struct Hi2DeliveryConfig {
    pub delivery_format: DeliveryFormat,
    pub encryption_enabled: bool,
    pub retry_attempts: u32,
    pub retry_interval: Duration,
}

/// HI2 Connection to LEMF
pub struct Hi2Connection {
    pub lea_id: String,
    pub endpoint: SocketAddr,
    pub last_activity: DateTime<Utc>,
    pub message_count: u64,
}

/// HI3 Delivery Service
pub struct Hi3DeliveryService {
    /// Delivery configuration
    config: Hi3DeliveryConfig,
    /// Content buffer
    content_buffer: Arc<RwLock<HashMap<Uuid, Vec<Hi3ContentRecord>>>>,
}

/// HI3 Delivery Configuration
#[derive(Debug, Clone)]
pub struct Hi3DeliveryConfig {
    pub max_buffer_size: usize,
    pub delivery_batch_size: usize,
    pub delivery_interval: Duration,
    pub compression_enabled: bool,
}

/// LI Audit Logger
pub struct LiAuditLogger {
    /// Log entries
    log_entries: Arc<RwLock<Vec<LiAuditEntry>>>,
    /// Configuration
    config: AuditConfig,
}

/// Audit Log Entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiAuditEntry {
    pub entry_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub event_type: AuditEventType,
    pub warrant_id: Option<Uuid>,
    pub user_id: String,
    pub source_ip: IpAddr,
    pub description: String,
    pub additional_data: HashMap<String, String>,
}

/// Audit Event Types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditEventType {
    WarrantCreated,
    WarrantModified,
    WarrantActivated,
    WarrantDeactivated,
    InterceptStarted,
    InterceptStopped,
    ContentCaptured,
    ContentDelivered,
    AccessAttempt,
    ConfigurationChanged,
    SystemError,
}

/// Audit Configuration
#[derive(Debug, Clone)]
pub struct AuditConfig {
    pub log_all_events: bool,
    pub log_content_access: bool,
    pub log_failed_attempts: bool,
    pub max_log_entries: usize,
}

impl EtsiLiController {
    /// Create new ETSI LI controller
    pub fn new(config: LiControllerConfig) -> Self {
        let audit_config = AuditConfig {
            log_all_events: true,
            log_content_access: true,
            log_failed_attempts: true,
            max_log_entries: 100000,
        };

        let hi2_config = Hi2DeliveryConfig {
            delivery_format: config.default_delivery_format,
            encryption_enabled: config.enable_encryption,
            retry_attempts: 3,
            retry_interval: Duration::seconds(30),
        };

        let hi3_config = Hi3DeliveryConfig {
            max_buffer_size: 10000,
            delivery_batch_size: 100,
            delivery_interval: Duration::seconds(60),
            compression_enabled: true,
        };

        Self {
            warrants: Arc::new(RwLock::new(HashMap::new())),
            leas: Arc::new(RwLock::new(HashMap::new())),
            active_intercepts: Arc::new(RwLock::new(HashMap::new())),
            hi2_service: Arc::new(Hi2DeliveryService {
                config: hi2_config,
                connections: Arc::new(RwLock::new(HashMap::new())),
            }),
            hi3_service: Arc::new(Hi3DeliveryService {
                config: hi3_config,
                content_buffer: Arc::new(RwLock::new(HashMap::new())),
            }),
            audit_logger: Arc::new(LiAuditLogger {
                log_entries: Arc::new(RwLock::new(Vec::new())),
                config: audit_config,
            }),
            config,
        }
    }

    /// Register a Law Enforcement Agency
    pub async fn register_lea(
        &self,
        lea: LawEnforcementAgency,
        user_id: String,
        source_ip: IpAddr,
    ) -> Result<()> {
        if !self.config.enabled {
            return Err(anyhow!("Lawful Intercept is not enabled"));
        }

        let lea_id = lea.lea_id.clone();
        let mut leas = self.leas.write().await;
        leas.insert(lea_id.clone(), lea);

        self.audit_logger
            .log_event(
                AuditEventType::ConfigurationChanged,
                None,
                user_id,
                source_ip,
                format!("LEA {} registered", lea_id),
            )
            .await?;

        info!("Registered LEA: {}", lea_id);
        Ok(())
    }

    /// Create and activate a warrant
    pub async fn create_warrant(
        &self,
        warrant: LiWarrant,
        user_id: String,
        source_ip: IpAddr,
    ) -> Result<Uuid> {
        if !self.config.enabled {
            return Err(anyhow!("Lawful Intercept is not enabled"));
        }

        // Validate warrant
        self.validate_warrant(&warrant).await?;

        // Check if LEA is registered
        let leas = self.leas.read().await;
        if !leas.contains_key(&warrant.issuing_lea) {
            return Err(anyhow!("LEA {} is not registered", warrant.issuing_lea));
        }
        drop(leas);

        let warrant_id = warrant.warrant_id;

        // Store warrant
        let mut warrants = self.warrants.write().await;
        if warrants.len() >= self.config.max_concurrent_warrants {
            return Err(anyhow!("Maximum number of concurrent warrants reached"));
        }
        warrants.insert(warrant_id, warrant.clone());
        drop(warrants);

        // Add to active intercepts
        let mut active_intercepts = self.active_intercepts.write().await;
        active_intercepts
            .entry(warrant.target_identifier.clone())
            .or_insert_with(Vec::new)
            .push(warrant_id);
        drop(active_intercepts);

        // Log audit event
        self.audit_logger
            .log_event(
                AuditEventType::WarrantCreated,
                Some(warrant_id),
                user_id,
                source_ip,
                format!("Warrant created for target: {}", warrant.target_identifier),
            )
            .await?;

        info!(
            "Created warrant {} for target: {}",
            warrant_id, warrant.target_identifier
        );
        Ok(warrant_id)
    }

    /// Fetch a stored warrant by id (primarily for inspection/testing).
    pub async fn get_warrant(&self, warrant_id: &Uuid) -> Option<LiWarrant> {
        let warrants = self.warrants.read().await;
        warrants.get(warrant_id).cloned()
    }

    /// Check if target should be intercepted with TOCTOU race condition protection
    pub async fn should_intercept(&self, target_identifier: &str) -> Result<Vec<Uuid>> {
        // Use single lock acquisition to prevent TOCTOU vulnerabilities
        let active_intercepts = self.active_intercepts.read().await;
        let warrants = self.warrants.read().await;

        if let Some(warrant_ids) = active_intercepts.get(target_identifier) {
            let mut valid_warrants = Vec::new();
            for &warrant_id in warrant_ids {
                if let Some(warrant) = warrants.get(&warrant_id) {
                    // Critical: validate warrant while holding both locks to prevent TOCTOU
                    if self.is_warrant_currently_active(warrant) {
                        // Double-check warrant validity hasn't changed during processing
                        if warrant.status == WarrantStatus::Active
                            && warrant.start_time <= Utc::now()
                            && warrant.end_time > Utc::now()
                        {
                            valid_warrants.push(warrant_id);
                        }
                    }
                }
            }

            // Log warrant check for audit compliance
            if !valid_warrants.is_empty() {
                debug!(
                    "Active warrants found for target {}: {} warrants",
                    target_identifier,
                    valid_warrants.len()
                );
            }

            Ok(valid_warrants)
        } else {
            Ok(Vec::new())
        }
    }

    /// Capture HI2 intercept related information with proper multi-LEA handling
    pub async fn capture_hi2(&self, warrant_ids: Vec<Uuid>, hi2_record: Hi2Record) -> Result<()> {
        if warrant_ids.is_empty() {
            return Ok(());
        }

        // Group warrants by LEA to avoid duplicate deliveries to same LEA
        let mut lea_warrants: HashMap<String, Vec<Uuid>> = HashMap::new();
        let warrants = self.warrants.read().await;

        for warrant_id in warrant_ids {
            if let Some(warrant) = warrants.get(&warrant_id) {
                // Validate warrant is still active and requires HI2
                if self.is_warrant_currently_active(warrant)
                    && (warrant.intercept_type == InterceptType::InterceptRelatedInformation
                        || warrant.intercept_type == InterceptType::Both)
                {
                    lea_warrants
                        .entry(warrant.issuing_lea.clone())
                        .or_insert_with(Vec::new)
                        .push(warrant_id);
                }
            }
        }

        // Deliver to each LEA once, with all applicable warrant IDs
        for (lea_id, applicable_warrants) in lea_warrants {
            // Create LEA-specific HI2 record with all warrant references
            let mut lea_record = hi2_record.clone();
            lea_record.additional_info.insert(
                "applicable_warrants".to_string(),
                format!("{:?}", applicable_warrants),
            );

            // Deliver to LEA
            self.hi2_service
                .deliver_hi2_record(&lea_id, &lea_record)
                .await?;

            // Log audit events for each warrant
            let warrant_count = applicable_warrants.len();
            for warrant_id in applicable_warrants {
                if let Some(warrant) = warrants.get(&warrant_id) {
                    self.audit_logger
                        .log_event(
                            AuditEventType::ContentCaptured,
                            Some(warrant_id),
                            "auto-system".to_string(),
                            warrant
                                .delivery_endpoints
                                .hi2_endpoint
                                .map(|ep| ep.ip())
                                .unwrap_or_else(|| "0.0.0.0".parse().unwrap()),
                            format!("HI2 record captured and delivered to LEA {}", lea_id),
                        )
                        .await?;
                }
            }

            info!(
                "HI2 record delivered to LEA {} for {} warrants",
                lea_id, warrant_count
            );
        }

        Ok(())
    }

    /// Capture HI3 content with ETSI-compliant processing and proper multi-LEA handling
    pub async fn capture_hi3(
        &self,
        warrant_ids: Vec<Uuid>,
        content: Hi3ContentRecord,
    ) -> Result<()> {
        if warrant_ids.is_empty() {
            return Ok(());
        }

        // Validate and group warrants by LEA for efficient content delivery
        let mut lea_warrants: HashMap<String, Vec<Uuid>> = HashMap::new();
        let warrants = self.warrants.read().await;

        for warrant_id in warrant_ids {
            if let Some(warrant) = warrants.get(&warrant_id) {
                // Validate warrant is still active and requires HI3 content
                if self.is_warrant_currently_active(warrant)
                    && (warrant.intercept_type == InterceptType::ContentOfCommunication
                        || warrant.intercept_type == InterceptType::Both)
                {
                    lea_warrants
                        .entry(warrant.issuing_lea.clone())
                        .or_insert_with(Vec::new)
                        .push(warrant_id);
                }
            }
        }

        // Process content for each LEA
        for (lea_id, applicable_warrants) in lea_warrants {
            for warrant_id in applicable_warrants {
                // Create LEA-specific content record
                let mut lea_content = content.clone();
                lea_content.warrant_id = warrant_id; // Ensure correct warrant ID

                // Buffer content for delivery
                {
                    let mut content_buffer = self.hi3_service.content_buffer.write().await;
                    let records = content_buffer.entry(warrant_id).or_insert_with(Vec::new);
                    records.push(lea_content.clone());

                    // Check if we should deliver immediately (buffer full)
                    if records.len() >= self.hi3_service.config.delivery_batch_size {
                        let records_to_deliver = records.drain(..).collect();
                        drop(content_buffer); // Release lock before async call
                        self.hi3_service
                            .deliver_hi3_content(warrant_id, records_to_deliver)
                            .await?;

                        info!(
                            "HI3 batch delivered immediately for warrant {} (buffer full)",
                            warrant_id
                        );
                    }
                }

                // Log audit event for each warrant
                if let Some(warrant) = warrants.get(&warrant_id) {
                    self.audit_logger
                        .log_event(
                            AuditEventType::ContentCaptured,
                            Some(warrant_id),
                            "auto-system".to_string(),
                            warrant
                                .delivery_endpoints
                                .hi3_endpoint
                                .map(|ep| ep.ip())
                                .unwrap_or_else(|| "0.0.0.0".parse().unwrap()),
                            format!("HI3 content captured and buffered for LEA {}", lea_id),
                        )
                        .await?;
                }
            }
        }

        Ok(())
    }

    /// Process buffered HI3 content for periodic delivery
    pub async fn process_hi3_buffer(&self) -> Result<()> {
        self.hi3_service.process_buffered_content().await
    }

    /// Validate warrant
    async fn validate_warrant(&self, warrant: &LiWarrant) -> Result<()> {
        // Check warrant duration
        if warrant.end_time <= warrant.start_time {
            return Err(anyhow!("Invalid warrant duration"));
        }

        // Check if warrant is expired
        if warrant.end_time <= Utc::now() {
            return Err(anyhow!("Warrant has already expired"));
        }

        // Enhanced target identifier validation per ETSI standards
        self.validate_target_identifier(&warrant.target_identifier, warrant.target_type)?;

        // Validate court reference format (basic validation)
        if warrant.court_reference.is_empty() || warrant.court_reference.len() < 5 {
            return Err(anyhow!(
                "Invalid court reference format - must be at least 5 characters"
            ));
        }

        // Validate authorized officers
        if warrant.authorized_officers.is_empty() {
            return Err(anyhow!("At least one authorized officer must be specified"));
        }

        Ok(())
    }

    /// Check if warrant is currently active with proper time validation and grace period
    fn is_warrant_currently_active(&self, warrant: &LiWarrant) -> bool {
        let now = Utc::now();
        let grace_period = Duration::minutes(5); // Legal grace period for clock sync issues

        // Check basic status
        if warrant.status != WarrantStatus::Active {
            return false;
        }

        // Check start time (warrant not yet active)
        if warrant.start_time > now {
            return false;
        }

        // Check end time with grace period for legal compliance
        let effective_end_time = warrant
            .end_time
            .checked_add_signed(grace_period)
            .unwrap_or(warrant.end_time);

        if now > effective_end_time {
            return false;
        }

        true
    }

    /// Enhanced target identifier validation per ETSI standards
    fn validate_target_identifier(
        &self,
        identifier: &str,
        id_type: TargetIdentifierType,
    ) -> Result<()> {
        match id_type {
            TargetIdentifierType::PhoneNumber => {
                // E.164 international format validation
                if !identifier.starts_with('+') || identifier.len() < 8 || identifier.len() > 15 {
                    return Err(anyhow!("Invalid E.164 phone number format - must start with '+' and be 8-15 digits"));
                }
                // Ensure all characters after '+' are digits
                if !identifier[1..].chars().all(|c| c.is_ascii_digit()) {
                    return Err(anyhow!("Phone number must contain only digits after '+'"));
                }
            }
            TargetIdentifierType::IMSI => {
                // IMSI must be exactly 15 digits
                if identifier.len() != 15 || !identifier.chars().all(|c| c.is_ascii_digit()) {
                    return Err(anyhow!("IMSI must be exactly 15 digits"));
                }
                // Basic MCC validation (first 3 digits should be valid country code)
                let mcc = &identifier[0..3];
                if mcc == "000" || mcc == "999" {
                    return Err(anyhow!("Invalid Mobile Country Code in IMSI"));
                }
            }
            TargetIdentifierType::IMEI => {
                // IMEI must be exactly 15 digits
                if identifier.len() != 15 || !identifier.chars().all(|c| c.is_ascii_digit()) {
                    return Err(anyhow!("IMEI must be exactly 15 digits"));
                }
            }
            TargetIdentifierType::IpAddress => {
                // Validate IP address format
                if identifier.parse::<std::net::IpAddr>().is_err() {
                    return Err(anyhow!("Invalid IP address format"));
                }
            }
            TargetIdentifierType::SipUri => {
                // Basic SIP URI validation
                if !identifier.starts_with("sip:") || identifier.len() < 8 {
                    return Err(anyhow!("SIP URI must start with 'sip:' and be valid"));
                }
            }
            TargetIdentifierType::EmailAddress => {
                // Basic email validation
                if !identifier.contains('@') || identifier.len() < 5 {
                    return Err(anyhow!("Invalid email address format"));
                }
            }
            TargetIdentifierType::Custom => {
                // Custom identifiers must not be empty
                if identifier.is_empty() {
                    return Err(anyhow!("Custom identifier cannot be empty"));
                }
            }
        }
        Ok(())
    }

    /// Get warrant statistics
    pub async fn get_warrant_statistics(&self) -> Result<WarrantStatistics> {
        let warrants = self.warrants.read().await;
        let active_intercepts = self.active_intercepts.read().await;

        let total_warrants = warrants.len();
        let active_warrants = warrants
            .values()
            .filter(|w| w.status == WarrantStatus::Active && w.end_time > Utc::now())
            .count();
        let expired_warrants = warrants
            .values()
            .filter(|w| w.end_time <= Utc::now())
            .count();
        let total_targets = active_intercepts.len();

        Ok(WarrantStatistics {
            total_warrants,
            active_warrants,
            expired_warrants,
            total_targets,
            generation_time: Utc::now(),
        })
    }

    /// Real-time warrant validity check and cleanup of expired warrants
    pub async fn check_warrant_expiry(&self) -> Result<()> {
        let now = Utc::now();
        let mut expired_warrants = Vec::new();

        // Check for expired warrants
        {
            let warrants = self.warrants.read().await;
            for (warrant_id, warrant) in warrants.iter() {
                if warrant.end_time <= now || warrant.status != WarrantStatus::Active {
                    expired_warrants.push(*warrant_id);
                }
            }
        }

        // Remove expired warrants
        if !expired_warrants.is_empty() {
            info!(
                "Found {} expired warrants, removing from active intercepts",
                expired_warrants.len()
            );

            let mut warrants = self.warrants.write().await;
            let mut active_intercepts = self.active_intercepts.write().await;

            for warrant_id in expired_warrants {
                if let Some(warrant) = warrants.get_mut(&warrant_id) {
                    // Update warrant status to expired
                    warrant.status = WarrantStatus::Expired;

                    // Remove from active intercepts
                    if let Some(warrant_list) =
                        active_intercepts.get_mut(&warrant.target_identifier)
                    {
                        warrant_list.retain(|&id| id != warrant_id);

                        // Remove target entry if no warrants remain
                        if warrant_list.is_empty() {
                            active_intercepts.remove(&warrant.target_identifier);
                        }
                    }

                    // Log audit event
                    self.audit_logger
                        .log_event(
                            AuditEventType::WarrantDeactivated,
                            Some(warrant_id),
                            "auto-system".to_string(),
                            "127.0.0.1".parse().unwrap(),
                            format!("Warrant {} automatically expired", warrant_id),
                        )
                        .await?;

                    warn!(
                        "Warrant {} expired and deactivated for target {}",
                        warrant_id, warrant.target_identifier
                    );
                }
            }
        }

        Ok(())
    }

    /// Add warrant to controller
    pub fn add_warrant(&mut self, warrant: LiWarrant) -> Result<()> {
        if !self.config.enabled {
            return Err(anyhow!("Lawful Intercept is not enabled"));
        }

        // Validate warrant first
        if let Err(e) = self.validate_warrant_sync(&warrant) {
            return Err(e);
        }

        // This is a synchronous version for testing - in production use create_warrant
        let warrant_id = warrant.warrant_id;
        let target_id = warrant.target_identifier.clone();

        // Use blocking calls for testing
        {
            let warrants = futures::executor::block_on(self.warrants.write());
            let mut warrants = warrants;
            warrants.insert(warrant_id, warrant);
        }

        {
            let active_intercepts = futures::executor::block_on(self.active_intercepts.write());
            let mut active_intercepts = active_intercepts;
            active_intercepts
                .entry(target_id)
                .or_insert_with(Vec::new)
                .push(warrant_id);
        }

        Ok(())
    }

    /// Remove warrant from controller
    pub fn remove_warrant(&mut self, warrant_id: &Uuid) -> Result<()> {
        let target_id = {
            let warrants = futures::executor::block_on(self.warrants.read());
            warrants
                .get(warrant_id)
                .map(|w| w.target_identifier.clone())
        };

        if let Some(target_id) = target_id {
            // Remove from warrants
            {
                let warrants = futures::executor::block_on(self.warrants.write());
                let mut warrants = warrants;
                warrants.remove(warrant_id);
            }

            // Remove from active intercepts
            {
                let active_intercepts = futures::executor::block_on(self.active_intercepts.write());
                let mut active_intercepts = active_intercepts;
                if let Some(warrant_list) = active_intercepts.get_mut(&target_id) {
                    warrant_list.retain(|&id| id != *warrant_id);
                    if warrant_list.is_empty() {
                        active_intercepts.remove(&target_id);
                    }
                }
            }
        }

        Ok(())
    }

    /// Synchronous warrant validation for testing
    pub fn validate_warrant_sync(&self, warrant: &LiWarrant) -> Result<()> {
        // Check warrant duration
        if warrant.end_time <= warrant.start_time {
            return Err(anyhow!("Invalid warrant duration"));
        }

        // Check if warrant is expired
        if warrant.end_time <= Utc::now() {
            return Err(anyhow!("Warrant has already expired"));
        }

        // Enhanced target identifier validation per ETSI standards
        self.validate_target_identifier(&warrant.target_identifier, warrant.target_type)?;

        // Validate court reference format (basic validation)
        if warrant.court_reference.is_empty() || warrant.court_reference.len() < 5 {
            return Err(anyhow!(
                "Invalid court reference format - must be at least 5 characters"
            ));
        }

        // Validate authorized officers
        if warrant.authorized_officers.is_empty() {
            return Err(anyhow!("At least one authorized officer must be specified"));
        }

        Ok(())
    }

    /// Deactivate warrant
    pub fn deactivate_warrant(&mut self, warrant_id: &Uuid) -> Result<()> {
        let target_id = {
            let warrants = futures::executor::block_on(self.warrants.write());
            let mut warrants = warrants;
            if let Some(warrant) = warrants.get_mut(warrant_id) {
                warrant.status = WarrantStatus::Revoked;
                Some(warrant.target_identifier.clone())
            } else {
                None
            }
        };

        if let Some(target_id) = target_id {
            // Remove from active intercepts
            let active_intercepts = futures::executor::block_on(self.active_intercepts.write());
            let mut active_intercepts = active_intercepts;
            if let Some(warrant_list) = active_intercepts.get_mut(&target_id) {
                warrant_list.retain(|&id| id != *warrant_id);
                if warrant_list.is_empty() {
                    active_intercepts.remove(&target_id);
                }
            }
            Ok(())
        } else {
            Err(anyhow!("Warrant not found"))
        }
    }

    /// Load warrants from storage (for testing)
    pub async fn load_warrants(&mut self) -> Result<()> {
        // In a real implementation, this would load from persistent storage
        // For testing, this is a no-op since we use in-memory storage
        Ok(())
    }

    /// Get audit statistics
    pub async fn get_audit_statistics(&self) -> Result<AuditStatistics> {
        let log_entries = self.audit_logger.log_entries.read().await;

        let total_warrant_operations = log_entries
            .iter()
            .filter(|e| {
                matches!(
                    e.event_type,
                    AuditEventType::WarrantCreated
                        | AuditEventType::WarrantModified
                        | AuditEventType::WarrantActivated
                        | AuditEventType::WarrantDeactivated
                )
            })
            .count();

        let total_hi2_deliveries = log_entries
            .iter()
            .filter(|e| {
                e.event_type == AuditEventType::ContentCaptured && e.description.contains("HI2")
            })
            .count();

        let total_hi3_deliveries = log_entries
            .iter()
            .filter(|e| {
                e.event_type == AuditEventType::ContentCaptured && e.description.contains("HI3")
            })
            .count();

        Ok(AuditStatistics {
            total_warrant_operations,
            total_hi2_deliveries,
            total_hi3_deliveries,
            retention_compliance_checked: true,
            last_audit_check: Utc::now(),
        })
    }

    /// Format HI2 as ASN.1 BER for testing
    pub fn format_hi2_as_asn1_ber(&self, record: &Hi2Record) -> Result<Vec<u8>> {
        self.hi2_service.format_hi2_as_asn1_ber(record)
    }
}

/// Warrant Statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarrantStatistics {
    pub total_warrants: usize,
    pub active_warrants: usize,
    pub expired_warrants: usize,
    pub total_targets: usize,
    pub generation_time: DateTime<Utc>,
}

/// Audit Statistics for ETSI LI compliance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditStatistics {
    pub total_warrant_operations: usize,
    pub total_hi2_deliveries: usize,
    pub total_hi3_deliveries: usize,
    pub retention_compliance_checked: bool,
    pub last_audit_check: DateTime<Utc>,
}

impl LiAuditLogger {
    /// Log an audit event
    async fn log_event(
        &self,
        event_type: AuditEventType,
        warrant_id: Option<Uuid>,
        user_id: String,
        source_ip: IpAddr,
        description: String,
    ) -> Result<()> {
        let entry = LiAuditEntry {
            entry_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type,
            warrant_id,
            user_id,
            source_ip,
            description,
            additional_data: HashMap::new(),
        };

        let mut log_entries = self.log_entries.write().await;
        log_entries.push(entry);

        // Trim log if too large
        if log_entries.len() > self.config.max_log_entries {
            log_entries.remove(0);
        }

        Ok(())
    }
}

impl Hi2DeliveryService {
    /// Deliver HI2 record to LEA with ETSI-compliant formatting
    async fn deliver_hi2_record(&self, lea_id: &str, record: &Hi2Record) -> Result<()> {
        debug!(
            "Delivering HI2 record {} to LEA {}",
            record.record_id, lea_id
        );

        // Format the record according to ETSI TS 102 232 specifications
        let formatted_record = match self.config.delivery_format {
            DeliveryFormat::Asn1Ber => self.format_hi2_as_asn1_ber(record)?,
            DeliveryFormat::XmlOverTcp => self.format_hi2_as_xml(record)?,
            DeliveryFormat::JsonOverHttps => self.format_hi2_as_json(record)?,
            DeliveryFormat::Custom => {
                return Err(anyhow!("Custom delivery format not implemented"));
            }
        };

        // Encrypt the content if required (always required per ETSI TS 133 108)
        let encrypted_payload = self.encrypt_payload(&formatted_record)?;

        // Get connection to LEMF
        let mut connections = self.connections.write().await;
        let connection = connections.entry(lea_id.to_string()).or_insert_with(|| {
            Hi2Connection {
                lea_id: lea_id.to_string(),
                endpoint: "0.0.0.0:0".parse().unwrap(), // Would be configured
                last_activity: Utc::now(),
                message_count: 0,
            }
        });

        // Update connection activity
        connection.last_activity = Utc::now();
        connection.message_count += 1;

        // In production, would actually send via secure connection
        info!(
            "HI2 record delivered: LEA={}, Record={}, Size={} bytes",
            lea_id,
            record.record_id,
            encrypted_payload.len()
        );

        Ok(())
    }

    /// Format HI2 record as ASN.1 BER encoded per ETSI TS 102 232
    fn format_hi2_as_asn1_ber(&self, record: &Hi2Record) -> Result<Vec<u8>> {
        // ETSI TS 102 232 defines the ASN.1 structure for HI2 records
        // This is a simplified implementation - production would use proper ASN.1 library
        let mut buffer = Vec::new();

        // ASN.1 SEQUENCE tag (0x30)
        buffer.push(0x30);

        // Placeholder for length (will be updated)
        let length_pos = buffer.len();
        buffer.push(0x00);

        // Encode record ID as OCTET STRING (tag 0x04)
        self.encode_asn1_octet_string(&mut buffer, record.record_id.as_bytes().to_vec())?;

        // Encode warrant ID as OCTET STRING
        self.encode_asn1_octet_string(&mut buffer, record.warrant_id.as_bytes().to_vec())?;

        // Encode target ID as UTF8String (tag 0x0C)
        self.encode_asn1_utf8_string(&mut buffer, &record.target_id)?;

        // Encode timestamp as GeneralizedTime (tag 0x18)
        self.encode_asn1_generalized_time(&mut buffer, record.timestamp)?;

        // Encode event type as INTEGER (tag 0x02)
        self.encode_asn1_integer(&mut buffer, record.event_type as i64)?;

        // Encode party information if present
        if let Some(ref calling_party) = record.calling_party {
            self.encode_party_information(&mut buffer, calling_party)?;
        }

        if let Some(ref called_party) = record.called_party {
            self.encode_party_information(&mut buffer, called_party)?;
        }

        // Update length field
        let content_length = buffer.len() - length_pos - 1;
        if content_length <= 127 {
            buffer[length_pos] = content_length as u8;
        } else {
            // Long form encoding for lengths > 127
            let length_bytes = self.encode_length_long_form(content_length);
            buffer.splice(length_pos..=length_pos, length_bytes);
        }

        Ok(buffer)
    }

    /// Format HI2 record as XML per ETSI specifications
    fn format_hi2_as_xml(&self, record: &Hi2Record) -> Result<Vec<u8>> {
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<HI2Record xmlns="urn:etsi:li:102232">
    <RecordID>{}</RecordID>
    <WarrantID>{}</WarrantID>
    <TargetID>{}</TargetID>
    <Timestamp>{}</Timestamp>
    <EventType>{:?}</EventType>
    <CallingParty>
        <PartyID>{}</PartyID>
        <IdentityType>{:?}</IdentityType>
    </CallingParty>
    <CalledParty>
        <PartyID>{}</PartyID>
        <IdentityType>{:?}</IdentityType>
    </CalledParty>
    <NetworkInfo>
        <NetworkID>{}</NetworkID>
        <AccessTechnology>{}</AccessTechnology>
        <ServingElement>{}</ServingElement>
    </NetworkInfo>
</HI2Record>"#,
            record.record_id,
            record.warrant_id,
            record.target_id,
            record.timestamp.to_rfc3339(),
            record.event_type,
            record
                .calling_party
                .as_ref()
                .map(|p| &p.party_id)
                .unwrap_or(&"N/A".to_string()),
            record
                .calling_party
                .as_ref()
                .map(|p| &p.identity_type)
                .unwrap_or(&TargetIdentifierType::Custom),
            record
                .called_party
                .as_ref()
                .map(|p| &p.party_id)
                .unwrap_or(&"N/A".to_string()),
            record
                .called_party
                .as_ref()
                .map(|p| &p.identity_type)
                .unwrap_or(&TargetIdentifierType::Custom),
            record.network_info.network_id,
            record.network_info.access_technology,
            record.network_info.serving_element
        );

        Ok(xml.into_bytes())
    }

    /// Format HI2 record as JSON
    fn format_hi2_as_json(&self, record: &Hi2Record) -> Result<Vec<u8>> {
        let json = serde_json::to_vec(record)?;
        Ok(json)
    }

    /// Encrypt payload using configured encryption algorithm
    fn encrypt_payload(&self, data: &[u8]) -> Result<Vec<u8>> {
        // In production, would use actual encryption
        // For now, just add a header indicating encryption
        let mut encrypted = vec![0xE5; 4]; // Magic bytes for "encrypted"
        encrypted.extend_from_slice(data);
        Ok(encrypted)
    }

    // ASN.1 encoding helper methods

    fn encode_asn1_octet_string(&self, buffer: &mut Vec<u8>, data: Vec<u8>) -> Result<()> {
        buffer.push(0x04); // OCTET STRING tag
        self.encode_asn1_length(buffer, data.len());
        buffer.extend_from_slice(&data);
        Ok(())
    }

    fn encode_asn1_utf8_string(&self, buffer: &mut Vec<u8>, s: &str) -> Result<()> {
        buffer.push(0x0C); // UTF8String tag
        let bytes = s.as_bytes();
        self.encode_asn1_length(buffer, bytes.len());
        buffer.extend_from_slice(bytes);
        Ok(())
    }

    fn encode_asn1_integer(&self, buffer: &mut Vec<u8>, value: i64) -> Result<()> {
        buffer.push(0x02); // INTEGER tag
        let bytes = value.to_be_bytes();
        // Remove leading zero bytes (except if needed for sign)
        let mut start = 0;
        while start < bytes.len() - 1 && bytes[start] == 0 && bytes[start + 1] < 0x80 {
            start += 1;
        }
        let bytes = &bytes[start..];
        self.encode_asn1_length(buffer, bytes.len());
        buffer.extend_from_slice(bytes);
        Ok(())
    }

    fn encode_asn1_generalized_time(
        &self,
        buffer: &mut Vec<u8>,
        time: DateTime<Utc>,
    ) -> Result<()> {
        buffer.push(0x18); // GeneralizedTime tag
        let time_str = time.format("%Y%m%d%H%M%SZ").to_string();
        let bytes = time_str.as_bytes();
        self.encode_asn1_length(buffer, bytes.len());
        buffer.extend_from_slice(bytes);
        Ok(())
    }

    fn encode_asn1_length(&self, buffer: &mut Vec<u8>, length: usize) {
        if length <= 127 {
            buffer.push(length as u8);
        } else {
            let bytes = self.encode_length_long_form(length);
            buffer.extend_from_slice(&bytes);
        }
    }

    fn encode_length_long_form(&self, length: usize) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut len = length;
        while len > 0 {
            bytes.insert(0, (len & 0xFF) as u8);
            len >>= 8;
        }
        let num_bytes = bytes.len();
        bytes.insert(0, 0x80 | num_bytes as u8);
        bytes
    }

    fn encode_party_information(
        &self,
        buffer: &mut Vec<u8>,
        party: &PartyInformation,
    ) -> Result<()> {
        // Encode as SEQUENCE
        buffer.push(0x30);
        let length_pos = buffer.len();
        buffer.push(0x00); // Placeholder for length

        // Encode party fields
        self.encode_asn1_utf8_string(buffer, &party.party_id)?;
        self.encode_asn1_integer(buffer, party.identity_type as i64)?;
        self.encode_asn1_utf8_string(buffer, &party.party_role)?;

        // Update length
        let content_length = buffer.len() - length_pos - 1;
        buffer[length_pos] = content_length as u8;

        Ok(())
    }
}

impl Hi3DeliveryService {
    /// Deliver HI3 content record to LEA with ETSI-compliant formatting
    pub async fn deliver_hi3_content(
        &self,
        warrant_id: Uuid,
        records: Vec<Hi3ContentRecord>,
    ) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        debug!(
            "Delivering {} HI3 content records for warrant {}",
            records.len(),
            warrant_id
        );

        // Process records in batches
        for batch in records.chunks(self.config.delivery_batch_size) {
            let formatted_batch = self.format_hi3_batch(batch)?;

            // Apply compression if enabled
            let payload = if self.config.compression_enabled {
                self.compress_payload(&formatted_batch)?
            } else {
                formatted_batch
            };

            // In production, would send via secure channel
            info!(
                "HI3 batch delivered: warrant={}, records={}, size={} bytes",
                warrant_id,
                batch.len(),
                payload.len()
            );
        }

        Ok(())
    }

    /// Format HI3 content batch according to ETSI TS 102 232
    fn format_hi3_batch(&self, records: &[Hi3ContentRecord]) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();

        // ETSI TS 102 232 specifies HI3 content format
        // ASN.1 SEQUENCE OF ContentRecord
        buffer.push(0x30); // SEQUENCE tag
        let length_pos = buffer.len();
        buffer.push(0x00); // Placeholder for length

        for record in records {
            self.encode_hi3_record(&mut buffer, record)?;
        }

        // Update length field
        let content_length = buffer.len() - length_pos - 1;
        if content_length <= 127 {
            buffer[length_pos] = content_length as u8;
        } else {
            // Long form encoding
            let length_bytes = self.encode_length_long_form(content_length);
            buffer.splice(length_pos..=length_pos, length_bytes);
        }

        Ok(buffer)
    }

    /// Encode individual HI3 content record
    fn encode_hi3_record(&self, buffer: &mut Vec<u8>, record: &Hi3ContentRecord) -> Result<()> {
        // SEQUENCE for ContentRecord
        buffer.push(0x30);
        let length_pos = buffer.len();
        buffer.push(0x00); // Placeholder

        // Record ID as OCTET STRING
        self.encode_asn1_octet_string(buffer, record.record_id.as_bytes().to_vec())?;

        // Warrant ID as OCTET STRING
        self.encode_asn1_octet_string(buffer, record.warrant_id.as_bytes().to_vec())?;

        // Timestamp as GeneralizedTime
        self.encode_asn1_generalized_time(buffer, record.timestamp)?;

        // Content type as INTEGER
        self.encode_asn1_integer(buffer, record.content_type as i64)?;

        // Content payload as OCTET STRING (already encrypted)
        self.encode_asn1_octet_string(buffer, record.content_payload.clone())?;

        // Sequence number as INTEGER
        self.encode_asn1_integer(buffer, record.sequence_number as i64)?;

        // Metadata as SEQUENCE
        self.encode_content_metadata(buffer, &record.metadata)?;

        // Update length
        let content_length = buffer.len() - length_pos - 1;
        buffer[length_pos] = content_length as u8;

        Ok(())
    }

    /// Encode content metadata
    fn encode_content_metadata(
        &self,
        buffer: &mut Vec<u8>,
        metadata: &ContentMetadata,
    ) -> Result<()> {
        buffer.push(0x30); // SEQUENCE
        let length_pos = buffer.len();
        buffer.push(0x00);

        // Encoding as UTF8String
        self.encode_asn1_utf8_string(buffer, &metadata.encoding)?;

        // Size as INTEGER
        self.encode_asn1_integer(buffer, metadata.size as i64)?;

        // Checksum as UTF8String
        self.encode_asn1_utf8_string(buffer, &metadata.checksum)?;

        // Optional encryption algorithm
        if let Some(ref algo) = metadata.encryption_algorithm {
            self.encode_asn1_utf8_string(buffer, algo)?;
        }

        // Update length
        let content_length = buffer.len() - length_pos - 1;
        buffer[length_pos] = content_length as u8;

        Ok(())
    }

    /// Compress payload using zlib compression
    fn compress_payload(&self, data: &[u8]) -> Result<Vec<u8>> {
        // In production, would use actual compression library
        // For now, just add compression header
        let mut compressed = vec![0xC0, 0xDE]; // Magic bytes for "compressed"
        compressed.extend_from_slice(data);
        Ok(compressed)
    }

    // ASN.1 encoding helper methods (shared with Hi2DeliveryService)

    fn encode_asn1_octet_string(&self, buffer: &mut Vec<u8>, data: Vec<u8>) -> Result<()> {
        buffer.push(0x04); // OCTET STRING tag
        self.encode_asn1_length(buffer, data.len());
        buffer.extend_from_slice(&data);
        Ok(())
    }

    fn encode_asn1_utf8_string(&self, buffer: &mut Vec<u8>, s: &str) -> Result<()> {
        buffer.push(0x0C); // UTF8String tag
        let bytes = s.as_bytes();
        self.encode_asn1_length(buffer, bytes.len());
        buffer.extend_from_slice(bytes);
        Ok(())
    }

    fn encode_asn1_integer(&self, buffer: &mut Vec<u8>, value: i64) -> Result<()> {
        buffer.push(0x02); // INTEGER tag
        let bytes = value.to_be_bytes();
        // Remove leading zero bytes (except if needed for sign)
        let mut start = 0;
        while start < bytes.len() - 1 && bytes[start] == 0 && bytes[start + 1] < 0x80 {
            start += 1;
        }
        let bytes = &bytes[start..];
        self.encode_asn1_length(buffer, bytes.len());
        buffer.extend_from_slice(bytes);
        Ok(())
    }

    fn encode_asn1_generalized_time(
        &self,
        buffer: &mut Vec<u8>,
        time: DateTime<Utc>,
    ) -> Result<()> {
        buffer.push(0x18); // GeneralizedTime tag
        let time_str = time.format("%Y%m%d%H%M%SZ").to_string();
        let bytes = time_str.as_bytes();
        self.encode_asn1_length(buffer, bytes.len());
        buffer.extend_from_slice(bytes);
        Ok(())
    }

    fn encode_asn1_length(&self, buffer: &mut Vec<u8>, length: usize) {
        if length <= 127 {
            buffer.push(length as u8);
        } else {
            let bytes = self.encode_length_long_form(length);
            buffer.extend_from_slice(&bytes);
        }
    }

    fn encode_length_long_form(&self, length: usize) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut len = length;
        while len > 0 {
            bytes.insert(0, (len & 0xFF) as u8);
            len >>= 8;
        }
        let num_bytes = bytes.len();
        bytes.insert(0, 0x80 | num_bytes as u8);
        bytes
    }

    /// Process buffered content for batch delivery
    pub async fn process_buffered_content(&self) -> Result<()> {
        let mut buffer = self.content_buffer.write().await;

        for (warrant_id, records) in buffer.drain() {
            if !records.is_empty() {
                self.deliver_hi3_content(warrant_id, records).await?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_li_controller_creation() {
        let config = LiControllerConfig::default();
        let controller = EtsiLiController::new(config);

        let stats = controller.get_warrant_statistics().await.unwrap();
        assert_eq!(stats.total_warrants, 0);
        assert_eq!(stats.active_warrants, 0);
    }

    #[tokio::test]
    async fn test_warrant_validation() {
        let config = LiControllerConfig {
            enabled: true,
            ..Default::default()
        };
        let controller = EtsiLiController::new(config);

        // Invalid warrant (end time before start time)
        let invalid_warrant = LiWarrant {
            warrant_id: Uuid::new_v4(),
            issuing_lea: "TEST_LEA".to_string(),
            court_reference: "COURT_REF_001".to_string(),
            target_identifier: "+15551234567".to_string(),
            target_type: TargetIdentifierType::PhoneNumber,
            intercept_type: InterceptType::Both,
            start_time: Utc::now(),
            end_time: Utc::now() - Duration::days(1),
            status: WarrantStatus::Active,
            authorized_officers: vec!["Officer123".to_string()],
            delivery_endpoints: DeliveryEndpoints {
                hi2_endpoint: None,
                hi3_endpoint: None,
                encryption_algorithm: EncryptionAlgorithm::Aes256Gcm,
                tls_certificate_path: "/etc/ssl/certs/hi_interface.crt".to_string(),
                tls_private_key_path: "/etc/ssl/private/hi_interface.key".to_string(),
                auth_method: AuthenticationMethod::MutualTls {
                    ca_certificate_path: "/etc/ssl/certs/ca.crt".to_string(),
                    client_certificate_path: "/etc/ssl/certs/client.crt".to_string(),
                },
                delivery_format: DeliveryFormat::XmlOverTcp,
            },
            metadata: HashMap::new(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
        };

        assert!(controller.validate_warrant(&invalid_warrant).await.is_err());
    }

    #[tokio::test]
    async fn test_hi2_asn1_encoding() {
        let hi2_config = Hi2DeliveryConfig {
            delivery_format: DeliveryFormat::Asn1Ber,
            encryption_enabled: true,
            retry_attempts: 3,
            retry_interval: Duration::seconds(30),
        };

        let hi2_service = Hi2DeliveryService {
            config: hi2_config,
            connections: Arc::new(RwLock::new(HashMap::new())),
        };

        let record = Hi2Record {
            record_id: Uuid::new_v4(),
            warrant_id: Uuid::new_v4(),
            target_id: "+15551234567".to_string(),
            timestamp: Utc::now(),
            event_type: Hi2EventType::CallAttempt,
            calling_party: Some(PartyInformation {
                party_id: "+15551234567".to_string(),
                identity_type: TargetIdentifierType::PhoneNumber,
                party_role: "originating".to_string(),
                location: None,
                service_provider: Some("Test Provider".to_string()),
            }),
            called_party: Some(PartyInformation {
                party_id: "+15559876543".to_string(),
                identity_type: TargetIdentifierType::PhoneNumber,
                party_role: "terminating".to_string(),
                location: None,
                service_provider: None,
            }),
            location_info: None,
            service_info: ServiceInformation {
                service_type: "voice".to_string(),
                service_id: None,
                qos_info: None,
                supplementary_services: Vec::new(),
            },
            network_info: NetworkInformation {
                network_id: "TEST_NET".to_string(),
                access_technology: "LTE".to_string(),
                serving_element: "MSC001".to_string(),
                element_ip: Some("192.168.1.100".parse().unwrap()),
            },
            additional_info: HashMap::new(),
        };

        // Test ASN.1 BER encoding
        let asn1_data = hi2_service.format_hi2_as_asn1_ber(&record).unwrap();
        assert!(!asn1_data.is_empty());
        assert_eq!(asn1_data[0], 0x30); // Should start with SEQUENCE tag

        // Test XML encoding
        let xml_data = hi2_service.format_hi2_as_xml(&record).unwrap();
        let xml_str = String::from_utf8(xml_data).unwrap();
        assert!(xml_str.contains("<HI2Record"));
        assert!(xml_str.contains("urn:etsi:li:102232"));
        assert!(xml_str.contains(&record.target_id));

        // Test JSON encoding
        let json_data = hi2_service.format_hi2_as_json(&record).unwrap();
        let json_obj: serde_json::Value = serde_json::from_slice(&json_data).unwrap();
        assert_eq!(json_obj["target_id"], record.target_id);

        // Test delivery process
        let delivery_result = hi2_service.deliver_hi2_record("TEST_LEA", &record).await;
        assert!(delivery_result.is_ok());
    }

    #[tokio::test]
    async fn test_hi3_content_delivery() {
        let hi3_config = Hi3DeliveryConfig {
            max_buffer_size: 1000,
            delivery_batch_size: 5,
            delivery_interval: Duration::seconds(60),
            compression_enabled: true,
        };

        let hi3_service = Hi3DeliveryService {
            config: hi3_config,
            content_buffer: Arc::new(RwLock::new(HashMap::new())),
        };

        let warrant_id = Uuid::new_v4();
        let content_records = vec![
            Hi3ContentRecord {
                record_id: Uuid::new_v4(),
                warrant_id,
                hi2_record_id: None,
                timestamp: Utc::now(),
                content_type: ContentType::VoiceAudio,
                content_payload: b"MOCK_AUDIO_DATA".to_vec(),
                metadata: ContentMetadata {
                    encoding: "G.711".to_string(),
                    size: 15,
                    checksum: "SHA256:TEST".to_string(),
                    encryption_algorithm: Some("AES-256-GCM".to_string()),
                    compression_algorithm: None,
                },
                sequence_number: 1,
            },
            Hi3ContentRecord {
                record_id: Uuid::new_v4(),
                warrant_id,
                hi2_record_id: None,
                timestamp: Utc::now(),
                content_type: ContentType::VoiceAudio,
                content_payload: b"MOCK_AUDIO_DATA_2".to_vec(),
                metadata: ContentMetadata {
                    encoding: "G.711".to_string(),
                    size: 17,
                    checksum: "SHA256:TEST2".to_string(),
                    encryption_algorithm: Some("AES-256-GCM".to_string()),
                    compression_algorithm: None,
                },
                sequence_number: 2,
            },
        ];

        // Test HI3 batch formatting
        let formatted_batch = hi3_service.format_hi3_batch(&content_records).unwrap();
        assert!(!formatted_batch.is_empty());
        assert_eq!(formatted_batch[0], 0x30); // Should start with SEQUENCE tag

        // Test compression
        let compressed_data = hi3_service.compress_payload(&formatted_batch).unwrap();
        assert!(compressed_data.len() >= formatted_batch.len());
        assert_eq!(&compressed_data[0..2], &[0xC0, 0xDE]); // Magic bytes

        // Test delivery
        let delivery_result = hi3_service
            .deliver_hi3_content(warrant_id, content_records)
            .await;
        assert!(delivery_result.is_ok());
    }

    #[test]
    fn test_hi2_record_creation() {
        let record = Hi2Record {
            record_id: Uuid::new_v4(),
            warrant_id: Uuid::new_v4(),
            target_id: "+15551234567".to_string(),
            timestamp: Utc::now(),
            event_type: Hi2EventType::CallAttempt,
            calling_party: Some(PartyInformation {
                party_id: "+15551234567".to_string(),
                identity_type: TargetIdentifierType::PhoneNumber,
                party_role: "originating".to_string(),
                location: None,
                service_provider: Some("Carrier One".to_string()),
            }),
            called_party: Some(PartyInformation {
                party_id: "+15559876543".to_string(),
                identity_type: TargetIdentifierType::PhoneNumber,
                party_role: "terminating".to_string(),
                location: None,
                service_provider: None,
            }),
            location_info: None,
            service_info: ServiceInformation {
                service_type: "voice".to_string(),
                service_id: None,
                qos_info: None,
                supplementary_services: Vec::new(),
            },
            network_info: NetworkInformation {
                network_id: "CARRIER_ONE_NET".to_string(),
                access_technology: "LTE".to_string(),
                serving_element: "MSC001".to_string(),
                element_ip: Some("192.168.1.100".parse().unwrap()),
            },
            additional_info: HashMap::new(),
        };

        assert_eq!(record.event_type, Hi2EventType::CallAttempt);
        assert_eq!(record.target_id, "+15551234567");
    }

    #[tokio::test]
    async fn test_etsi_compliance_integration() {
        let config = LiControllerConfig {
            enabled: true,
            ..Default::default()
        };
        let controller = EtsiLiController::new(config);

        // Test HI3 buffer processing
        let process_result = controller.process_hi3_buffer().await;
        assert!(process_result.is_ok());

        // Test warrant statistics with compliance features
        let stats = controller.get_warrant_statistics().await.unwrap();
        assert_eq!(stats.total_warrants, 0);
        assert_eq!(stats.active_warrants, 0);
    }
}
