/*
 * STIR/SHAKEN TDM Support per ATIS and Trans Nexus Specifications
 *
 * This module implements STIR/SHAKEN (Secure Telephone Identity Revisited/
 * Signature-based Handling of Asserted Information using toKENs) support
 * for TDM networks as specified by:
 *
 * - ATIS-1000074: STIR/SHAKEN Framework
 * - ATIS-1000080: Out-of-Band STIR/SHAKEN Architecture and Procedures
 * - Trans Nexus STIR/SHAKEN TDM specifications
 * - RFC 8224: Authenticated Identity Management in SIP
 * - RFC 8225: PASSporT: Personal Assertion Token
 * - RFC 8226: Common Behavior for STIR Certificate Management
 */

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};
use url::Url;

/// STIR/SHAKEN TDM transport methods
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StirShakenTransport {
    /// Out-of-band via SIP interface
    OutOfBandSip,
    /// In-band via ISUP User-to-User Information
    InBandIsup,
    /// SS7/Sigtran signaling
    SigtranSignaling,
    /// Proprietary TDM extensions
    ProprietaryTdm,
}

/// STIR/SHAKEN verification levels per ATIS specifications
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttestationLevel {
    /// Full attestation ("A") - Service provider verified the call
    FullAttestation,
    /// Partial attestation ("B") - Service provider verified customer but not call authorization
    PartialAttestation,
    /// Gateway attestation ("C") - Call from gateway, limited verification
    GatewayAttestation,
}

impl AttestationLevel {
    /// Get single-character attestation code
    pub fn to_code(&self) -> char {
        match self {
            Self::FullAttestation => 'A',
            Self::PartialAttestation => 'B',
            Self::GatewayAttestation => 'C',
        }
    }

    /// Parse attestation code from character
    pub fn from_code(code: char) -> Option<Self> {
        match code.to_ascii_uppercase() {
            'A' => Some(Self::FullAttestation),
            'B' => Some(Self::PartialAttestation),
            'C' => Some(Self::GatewayAttestation),
            _ => None,
        }
    }
}

/// STIR/SHAKEN verification status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStatus {
    /// TN-Validation-Passed ("TN-Validation-Passed")
    Passed,
    /// TN-Validation-Failed ("TN-Validation-Failed")
    Failed,
    /// No-TN-Validation ("No-TN-Validation")
    NoValidation,
}

impl VerificationStatus {
    /// Get status string per ATIS specification
    pub fn to_string(&self) -> &'static str {
        match self {
            Self::Passed => "TN-Validation-Passed",
            Self::Failed => "TN-Validation-Failed",
            Self::NoValidation => "No-TN-Validation",
        }
    }

    /// Parse status from string
    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "TN-Validation-Passed" => Some(Self::Passed),
            "TN-Validation-Failed" => Some(Self::Failed),
            "No-TN-Validation" => Some(Self::NoValidation),
            _ => None,
        }
    }
}

/// PASSporT (Personal Assertion Token) structure per RFC 8225
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Passport {
    /// Header with algorithm and type
    #[serde(flatten)]
    pub header: PassportHeader,
    /// Payload with identity claims
    pub payload: PassportPayload,
    /// Digital signature
    #[serde(skip)]
    pub signature: Vec<u8>,
}

/// PASSporT header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassportHeader {
    /// Algorithm used for signature (e.g., "ES256")
    pub alg: String,
    /// PASSporT type
    pub ppt: String,
    /// Type header parameter
    pub typ: String,
    /// X.509 certificate URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x5u: Option<String>,
}

/// PASSporT payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassportPayload {
    /// Attestation level
    pub attest: String,
    /// Destination telephone number
    pub dest: PassportDestination,
    /// Issued at (timestamp)
    pub iat: u64,
    /// Origination identifier
    pub orig: PassportOrigination,
    /// Origination identifier
    pub origid: String,
}

/// PASSporT destination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassportDestination {
    /// Telephone number array
    pub tn: Vec<String>,
}

/// PASSporT origination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassportOrigination {
    /// Telephone number
    pub tn: String,
}

/// STIR/SHAKEN TDM message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StirShakenTdmMessage {
    /// Transport method used
    pub transport: StirShakenTransport,
    /// Calling party number
    pub calling_number: String,
    /// Called party number
    pub called_number: String,
    /// PASSporT token (JWT format)
    pub passport_token: String,
    /// Attestation level
    pub attestation_level: AttestationLevel,
    /// Verification status
    pub verification_status: VerificationStatus,
    /// Circuit identification code (for ISUP)
    pub cic: Option<u32>,
    /// Call identifier
    pub call_id: String,
    /// Additional parameters
    pub parameters: HashMap<String, String>,
    /// Timestamp
    pub timestamp: SystemTime,
}

/// STIR/SHAKEN certificate information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StirShakenCertificate {
    /// Certificate URL
    pub cert_url: String,
    /// Certificate data (PEM format)
    pub cert_data: String,
    /// Public key
    pub public_key: Vec<u8>,
    /// Expiration time
    pub expires_at: SystemTime,
    /// Authorized telephone number blocks
    pub authorized_tn_blocks: Vec<String>,
}

/// STIR/SHAKEN TDM processor
pub struct StirShakenTdmProcessor {
    /// Configuration
    config: StirShakenTdmConfig,
    /// Certificate cache
    cert_cache: Arc<RwLock<HashMap<String, StirShakenCertificate>>>,
    /// HTTP client for certificate fetching
    http_client: HttpClient,
    /// Private key for signing (if acting as authentication service)
    signing_key: Option<EncodingKey>,
    /// Event sender for notifications
    event_sender: mpsc::UnboundedSender<StirShakenEvent>,
}

/// STIR/SHAKEN configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StirShakenTdmConfig {
    /// Enable STIR/SHAKEN processing
    pub enabled: bool,
    /// Certificate cache timeout (seconds)
    pub cert_cache_timeout: u64,
    /// Maximum certificate size (bytes)
    pub max_cert_size: usize,
    /// Supported transport methods
    pub supported_transports: Vec<StirShakenTransport>,
    /// Default attestation level for outgoing calls
    pub default_attestation_level: AttestationLevel,
    /// Certificate store URL
    pub cert_store_url: Option<String>,
    /// Local certificate for signing
    pub local_cert_path: Option<String>,
    /// Local private key for signing
    pub local_key_path: Option<String>,
    /// Require verification for incoming calls
    pub require_verification: bool,
}

impl Default for StirShakenTdmConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cert_cache_timeout: 3600, // 1 hour
            max_cert_size: 10 * 1024, // 10KB
            supported_transports: vec![
                StirShakenTransport::OutOfBandSip,
                StirShakenTransport::InBandIsup,
            ],
            default_attestation_level: AttestationLevel::PartialAttestation,
            cert_store_url: None,
            local_cert_path: None,
            local_key_path: None,
            require_verification: false,
        }
    }
}

/// STIR/SHAKEN events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StirShakenEvent {
    /// Call verified successfully
    CallVerified {
        call_id: String,
        calling_number: String,
        called_number: String,
        attestation_level: AttestationLevel,
        verification_status: VerificationStatus,
        transport: StirShakenTransport,
    },
    /// Call verification failed
    VerificationFailed {
        call_id: String,
        calling_number: String,
        called_number: String,
        reason: String,
        transport: StirShakenTransport,
    },
    /// Certificate retrieved
    CertificateRetrieved {
        cert_url: String,
        expires_at: SystemTime,
    },
    /// Certificate validation failed
    CertificateValidationFailed { cert_url: String, reason: String },
}

impl StirShakenTdmProcessor {
    /// Create new STIR/SHAKEN TDM processor
    pub async fn new(
        config: StirShakenTdmConfig,
    ) -> Result<(Self, mpsc::UnboundedReceiver<StirShakenEvent>)> {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        let http_client = HttpClient::builder()
            .timeout(Duration::from_secs(10))
            .build()?;

        // Load signing key if configured
        let signing_key = if let Some(key_path) = &config.local_key_path {
            match tokio::fs::read_to_string(key_path).await {
                Ok(key_data) => match EncodingKey::from_ec_pem(key_data.as_bytes()) {
                    Ok(key) => Some(key),
                    Err(e) => {
                        warn!("Failed to load STIR/SHAKEN signing key: {}", e);
                        None
                    }
                },
                Err(e) => {
                    warn!("Failed to read STIR/SHAKEN key file {}: {}", key_path, e);
                    None
                }
            }
        } else {
            None
        };

        let processor = Self {
            config,
            cert_cache: Arc::new(RwLock::new(HashMap::new())),
            http_client,
            signing_key,
            event_sender,
        };

        Ok((processor, event_receiver))
    }

    /// Process incoming STIR/SHAKEN TDM message
    pub async fn process_incoming_message(
        &self,
        message: StirShakenTdmMessage,
    ) -> Result<VerificationStatus> {
        if !self.config.enabled {
            return Ok(VerificationStatus::NoValidation);
        }

        debug!(
            "Processing STIR/SHAKEN TDM message for call {}",
            message.call_id
        );

        // Verify PASSporT token
        match self
            .verify_passport_token(
                &message.passport_token,
                &message.calling_number,
                &message.called_number,
            )
            .await
        {
            Ok(verification_result) => {
                // Send verification event
                let event = StirShakenEvent::CallVerified {
                    call_id: message.call_id.clone(),
                    calling_number: message.calling_number.clone(),
                    called_number: message.called_number.clone(),
                    attestation_level: message.attestation_level,
                    verification_status: verification_result,
                    transport: message.transport,
                };

                if let Err(e) = self.event_sender.send(event) {
                    warn!("Failed to send STIR/SHAKEN verification event: {}", e);
                }

                info!(
                    "STIR/SHAKEN verification passed for call {} ({} -> {})",
                    message.call_id, message.calling_number, message.called_number
                );

                Ok(verification_result)
            }
            Err(e) => {
                // Send failure event
                let event = StirShakenEvent::VerificationFailed {
                    call_id: message.call_id.clone(),
                    calling_number: message.calling_number.clone(),
                    called_number: message.called_number.clone(),
                    reason: e.to_string(),
                    transport: message.transport,
                };

                if let Err(send_err) = self.event_sender.send(event) {
                    warn!("Failed to send STIR/SHAKEN failure event: {}", send_err);
                }

                warn!(
                    "STIR/SHAKEN verification failed for call {}: {}",
                    message.call_id, e
                );

                if self.config.require_verification {
                    Ok(VerificationStatus::Failed)
                } else {
                    Ok(VerificationStatus::NoValidation)
                }
            }
        }
    }

    /// Generate outgoing STIR/SHAKEN TDM message
    pub async fn generate_outgoing_message(
        &self,
        call_id: String,
        calling_number: String,
        called_number: String,
        transport: StirShakenTransport,
        cic: Option<u32>,
    ) -> Result<StirShakenTdmMessage> {
        if !self.config.enabled {
            return Err(anyhow!("STIR/SHAKEN is disabled"));
        }

        let signing_key = self
            .signing_key
            .as_ref()
            .ok_or_else(|| anyhow!("No signing key configured"))?;

        // Create PASSporT
        let passport = self.create_passport(
            &calling_number,
            &called_number,
            self.config.default_attestation_level,
        )?;

        // Sign PASSporT to create JWT
        let passport_token = self.sign_passport(passport, signing_key)?;

        let message = StirShakenTdmMessage {
            transport,
            calling_number: calling_number.clone(),
            called_number: called_number.clone(),
            passport_token,
            attestation_level: self.config.default_attestation_level,
            verification_status: VerificationStatus::NoValidation, // Will be set by receiving end
            cic,
            call_id: call_id.clone(),
            parameters: HashMap::new(),
            timestamp: SystemTime::now(),
        };

        info!(
            "Generated STIR/SHAKEN TDM message for call {} ({} -> {})",
            call_id, calling_number, called_number
        );

        Ok(message)
    }

    /// Create PASSporT token
    fn create_passport(
        &self,
        calling_number: &str,
        called_number: &str,
        attestation: AttestationLevel,
    ) -> Result<Passport> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        let header = PassportHeader {
            alg: "ES256".to_string(),
            ppt: "shaken".to_string(),
            typ: "passport".to_string(),
            x5u: self.config.cert_store_url.clone(),
        };

        let payload = PassportPayload {
            attest: attestation.to_code().to_string(),
            dest: PassportDestination {
                tn: vec![called_number.to_string()],
            },
            iat: now,
            orig: PassportOrigination {
                tn: calling_number.to_string(),
            },
            origid: format!("{}_{}", calling_number, now), // Unique identifier
        };

        Ok(Passport {
            header,
            payload,
            signature: Vec::new(), // Will be filled by signing
        })
    }

    /// Sign PASSporT to create JWT
    fn sign_passport(&self, passport: Passport, signing_key: &EncodingKey) -> Result<String> {
        let header = Header::new(Algorithm::ES256);
        let token = encode(&header, &passport.payload, signing_key)
            .map_err(|e| anyhow!("Failed to sign PASSporT: {}", e))?;

        Ok(token)
    }

    /// Verify PASSporT token
    async fn verify_passport_token(
        &self,
        token: &str,
        calling_number: &str,
        called_number: &str,
    ) -> Result<VerificationStatus> {
        // Decode JWT header to get certificate URL
        let header_b64 = token
            .split('.')
            .next()
            .ok_or_else(|| anyhow!("Invalid JWT format"))?;

        let header_json = general_purpose::URL_SAFE_NO_PAD
            .decode(header_b64)
            .map_err(|e| anyhow!("Failed to decode JWT header: {}", e))?;

        let header: PassportHeader = serde_json::from_slice(&header_json)
            .map_err(|e| anyhow!("Failed to parse JWT header: {}", e))?;

        // Get certificate
        let cert_url = header
            .x5u
            .ok_or_else(|| anyhow!("No certificate URL in PASSporT header"))?;
        let certificate = self.get_certificate(&cert_url).await?;

        // Create decoding key from certificate
        let decoding_key = DecodingKey::from_ec_pem(certificate.cert_data.as_bytes())
            .map_err(|e| anyhow!("Failed to create decoding key: {}", e))?;

        // Verify JWT signature and decode payload
        let mut validation = Validation::new(Algorithm::ES256);
        validation.validate_exp = true;
        validation.validate_aud = false; // PASSporT doesn't use audience

        let token_data = decode::<PassportPayload>(token, &decoding_key, &validation)
            .map_err(|e| anyhow!("JWT verification failed: {}", e))?;

        // Verify payload contents
        self.verify_passport_payload(
            &token_data.claims,
            calling_number,
            called_number,
            &certificate,
        )?;

        Ok(VerificationStatus::Passed)
    }

    /// Verify PASSporT payload contents
    fn verify_passport_payload(
        &self,
        payload: &PassportPayload,
        calling_number: &str,
        called_number: &str,
        certificate: &StirShakenCertificate,
    ) -> Result<()> {
        // Verify calling number matches
        if payload.orig.tn != calling_number {
            return Err(anyhow!(
                "PASSporT calling number mismatch: {} != {}",
                payload.orig.tn,
                calling_number
            ));
        }

        // Verify called number is in destination list
        if !payload.dest.tn.contains(&called_number.to_string()) {
            return Err(anyhow!("PASSporT called number not in destination list"));
        }

        // Verify calling number is authorized by certificate
        if !self.is_number_authorized(&payload.orig.tn, certificate) {
            return Err(anyhow!(
                "Calling number {} not authorized by certificate",
                payload.orig.tn
            ));
        }

        // Verify timestamp is recent (within 60 seconds)
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let age = now.saturating_sub(payload.iat);
        if age > 60 {
            return Err(anyhow!("PASSporT is too old: {} seconds", age));
        }

        Ok(())
    }

    /// Check if telephone number is authorized by certificate
    fn is_number_authorized(&self, tn: &str, certificate: &StirShakenCertificate) -> bool {
        for block in &certificate.authorized_tn_blocks {
            if tn.starts_with(block) {
                return true;
            }
        }
        false
    }

    /// Get certificate from cache or fetch from URL
    async fn get_certificate(&self, cert_url: &str) -> Result<StirShakenCertificate> {
        // Check cache first
        {
            let cache = self.cert_cache.read().await;
            if let Some(cert) = cache.get(cert_url) {
                // Check if certificate is still valid
                if cert.expires_at > SystemTime::now() {
                    return Ok(cert.clone());
                }
            }
        }

        // Fetch certificate from URL
        let cert = self.fetch_certificate(cert_url).await?;

        // Update cache
        {
            let mut cache = self.cert_cache.write().await;
            cache.insert(cert_url.to_string(), cert.clone());
        }

        // Send certificate event
        let event = StirShakenEvent::CertificateRetrieved {
            cert_url: cert_url.to_string(),
            expires_at: cert.expires_at,
        };

        if let Err(e) = self.event_sender.send(event) {
            warn!("Failed to send certificate retrieved event: {}", e);
        }

        Ok(cert)
    }

    /// Fetch certificate from URL
    async fn fetch_certificate(&self, cert_url: &str) -> Result<StirShakenCertificate> {
        let url = Url::parse(cert_url).map_err(|e| anyhow!("Invalid certificate URL: {}", e))?;

        let response = self
            .http_client
            .get(url)
            .header("Accept", "application/pkcs7-mime")
            .send()
            .await
            .map_err(|e| anyhow!("Failed to fetch certificate: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Certificate fetch failed: HTTP {}",
                response.status()
            ));
        }

        let cert_data = response
            .text()
            .await
            .map_err(|e| anyhow!("Failed to read certificate data: {}", e))?;

        if cert_data.len() > self.config.max_cert_size {
            return Err(anyhow!("Certificate too large: {} bytes", cert_data.len()));
        }

        // Parse certificate to extract information
        // This is a simplified implementation - in production you'd use proper X.509 parsing
        let expires_at = SystemTime::now() + Duration::from_secs(self.config.cert_cache_timeout);
        let authorized_tn_blocks = vec!["1".to_string()]; // Simplified - would parse from cert

        Ok(StirShakenCertificate {
            cert_url: cert_url.to_string(),
            cert_data,
            public_key: Vec::new(), // Would extract from certificate
            expires_at,
            authorized_tn_blocks,
        })
    }

    /// Encode STIR/SHAKEN for ISUP User-to-User Information
    pub fn encode_for_isup_uui(&self, message: &StirShakenTdmMessage) -> Result<Vec<u8>> {
        let mut data = Vec::new();

        // UUI Protocol Discriminator (0x04 for User-specific)
        data.push(0x04);

        // STIR/SHAKEN indicator
        data.extend_from_slice(b"STIR");

        // Attestation level
        data.push(message.attestation_level.to_code() as u8);

        // PASSporT token (truncated to fit UUI size limits)
        let token_bytes = message.passport_token.as_bytes();
        let max_token_size = 128 - data.len(); // UUI size limit minus header
        let token_data = if token_bytes.len() > max_token_size {
            &token_bytes[..max_token_size]
        } else {
            token_bytes
        };

        data.extend_from_slice(token_data);

        Ok(data)
    }

    /// Decode STIR/SHAKEN from ISUP User-to-User Information
    pub fn decode_from_isup_uui(&self, uui_data: &[u8]) -> Result<StirShakenTdmMessage> {
        if uui_data.len() < 6 {
            return Err(anyhow!("UUI data too short for STIR/SHAKEN"));
        }

        // Check protocol discriminator
        if uui_data[0] != 0x04 {
            return Err(anyhow!("Invalid UUI protocol discriminator"));
        }

        // Check STIR/SHAKEN indicator
        if &uui_data[1..5] != b"STIR" {
            return Err(anyhow!("STIR/SHAKEN indicator not found in UUI"));
        }

        // Extract attestation level
        let attestation_level = AttestationLevel::from_code(uui_data[5] as char)
            .ok_or_else(|| anyhow!("Invalid attestation level in UUI"))?;

        // Extract PASSporT token (rest of the data)
        let passport_token = String::from_utf8_lossy(&uui_data[6..]).to_string();

        Ok(StirShakenTdmMessage {
            transport: StirShakenTransport::InBandIsup,
            calling_number: String::new(), // Would be extracted from ISUP IAM
            called_number: String::new(),  // Would be extracted from ISUP IAM
            passport_token,
            attestation_level,
            verification_status: VerificationStatus::NoValidation,
            cic: None,
            call_id: String::new(),
            parameters: HashMap::new(),
            timestamp: SystemTime::now(),
        })
    }

    /// Clean up expired certificates from cache
    pub async fn cleanup_certificate_cache(&self) {
        let mut cache = self.cert_cache.write().await;
        let now = SystemTime::now();
        let mut to_remove = Vec::new();

        for (url, cert) in cache.iter() {
            if cert.expires_at <= now {
                to_remove.push(url.clone());
            }
        }

        for url in to_remove {
            cache.remove(&url);
            debug!("Removed expired STIR/SHAKEN certificate: {}", url);
        }
    }

    /// Get processor statistics
    pub async fn get_statistics(&self) -> StirShakenTdmStats {
        let cache = self.cert_cache.read().await;

        StirShakenTdmStats {
            enabled: self.config.enabled,
            cached_certificates: cache.len(),
            supported_transports: self.config.supported_transports.clone(),
            require_verification: self.config.require_verification,
        }
    }
}

/// STIR/SHAKEN TDM statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StirShakenTdmStats {
    pub enabled: bool,
    pub cached_certificates: usize,
    pub supported_transports: Vec<StirShakenTransport>,
    pub require_verification: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;

    #[test]
    fn test_attestation_level_conversion() {
        assert_eq!(AttestationLevel::FullAttestation.to_code(), 'A');
        assert_eq!(AttestationLevel::PartialAttestation.to_code(), 'B');
        assert_eq!(AttestationLevel::GatewayAttestation.to_code(), 'C');

        assert_eq!(
            AttestationLevel::from_code('A'),
            Some(AttestationLevel::FullAttestation)
        );
        assert_eq!(
            AttestationLevel::from_code('b'),
            Some(AttestationLevel::PartialAttestation)
        );
        assert_eq!(AttestationLevel::from_code('X'), None);
    }

    #[test]
    fn test_verification_status_conversion() {
        assert_eq!(
            VerificationStatus::Passed.to_string(),
            "TN-Validation-Passed"
        );
        assert_eq!(
            VerificationStatus::Failed.to_string(),
            "TN-Validation-Failed"
        );
        assert_eq!(
            VerificationStatus::NoValidation.to_string(),
            "No-TN-Validation"
        );

        assert_eq!(
            VerificationStatus::from_string("TN-Validation-Passed"),
            Some(VerificationStatus::Passed)
        );
        assert_eq!(VerificationStatus::from_string("Unknown"), None);
    }

    #[tokio::test]
    async fn test_stir_shaken_processor_creation() {
        let config = StirShakenTdmConfig::default();
        let result = StirShakenTdmProcessor::new(config).await;
        assert!(result.is_ok());

        let (processor, _receiver) = result.unwrap();
        let stats = processor.get_statistics().await;
        assert_eq!(stats.enabled, true);
        assert_eq!(stats.cached_certificates, 0);
    }

    #[test]
    fn test_passport_creation() {
        let config = StirShakenTdmConfig::default();
        let (event_sender, _) = mpsc::unbounded_channel();
        let http_client = HttpClient::new();
        let processor = StirShakenTdmProcessor {
            config,
            cert_cache: Arc::new(RwLock::new(HashMap::new())),
            http_client,
            signing_key: None,
            event_sender,
        };

        let passport = processor
            .create_passport(
                "+15551234567",
                "+15559876543",
                AttestationLevel::FullAttestation,
            )
            .unwrap();

        assert_eq!(passport.payload.orig.tn, "+15551234567");
        assert_eq!(passport.payload.dest.tn, vec!["+15559876543"]);
        assert_eq!(passport.payload.attest, "A");
    }

    #[test]
    fn test_isup_uui_encoding() {
        let config = StirShakenTdmConfig::default();
        let (event_sender, _) = mpsc::unbounded_channel();
        let http_client = HttpClient::new();
        let processor = StirShakenTdmProcessor {
            config,
            cert_cache: Arc::new(RwLock::new(HashMap::new())),
            http_client,
            signing_key: None,
            event_sender,
        };

        let message = StirShakenTdmMessage {
            transport: StirShakenTransport::InBandIsup,
            calling_number: "+15551234567".to_string(),
            called_number: "+15559876543".to_string(),
            passport_token: "test.jwt.token".to_string(),
            attestation_level: AttestationLevel::FullAttestation,
            verification_status: VerificationStatus::NoValidation,
            cic: Some(100),
            call_id: "test-call".to_string(),
            parameters: HashMap::new(),
            timestamp: SystemTime::now(),
        };

        let uui_data = processor.encode_for_isup_uui(&message).unwrap();
        assert_eq!(uui_data[0], 0x04); // Protocol discriminator
        assert_eq!(&uui_data[1..5], b"STIR"); // STIR indicator
        assert_eq!(uui_data[5], b'A'); // Attestation level

        // Test round-trip
        let decoded = processor.decode_from_isup_uui(&uui_data).unwrap();
        assert_eq!(decoded.attestation_level, AttestationLevel::FullAttestation);
        assert_eq!(decoded.passport_token, "test.jwt.token");
    }
}
