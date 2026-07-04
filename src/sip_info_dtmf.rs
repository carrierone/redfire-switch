/*
 * SIP INFO Method for DTMF Transport
 *
 * This module implements DTMF transport using SIP INFO method as defined in:
 * - RFC 6086: Session Initiation Protocol (SIP) INFO Method and Package Framework
 * - RFC 2976: The SIP INFO Method (obsoleted by RFC 6086)
 * - Various vendor-specific implementations (Cisco, Asterisk, etc.)
 *
 * Supports multiple content types:
 * - application/dtmf-relay (Cisco)
 * - application/dtmf (generic)
 * - application/vnd.nortel.text (Nortel)
 */

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

use crate::dtmf_processor::{DtmfEvent, DtmfSource};

/// SIP INFO DTMF content types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SipInfoDtmfContentType {
    /// Cisco DTMF relay format
    CiscoDtmfRelay,
    /// Generic DTMF format
    GenericDtmf,
    /// Nortel text format
    NortelText,
    /// Custom application-specific format
    Custom(String),
}

impl SipInfoDtmfContentType {
    /// Get MIME content type string
    pub fn to_mime_type(&self) -> &'static str {
        match self {
            Self::CiscoDtmfRelay => "application/dtmf-relay",
            Self::GenericDtmf => "application/dtmf",
            Self::NortelText => "application/vnd.nortel.text",
            Self::Custom(_) => "application/dtmf", // Fallback
        }
    }

    /// Parse MIME content type to enum
    pub fn from_mime_type(mime_type: &str) -> Self {
        match mime_type.to_lowercase().as_str() {
            "application/dtmf-relay" => Self::CiscoDtmfRelay,
            "application/dtmf" => Self::GenericDtmf,
            "application/vnd.nortel.text" => Self::NortelText,
            other => Self::Custom(other.to_string()),
        }
    }
}

/// SIP INFO DTMF message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipInfoDtmfMessage {
    /// DTMF digit or command
    pub digit: char,
    /// Duration in milliseconds
    pub duration: Option<u32>,
    /// Volume/amplitude (0-100)
    pub volume: Option<u32>,
    /// Content type format
    pub content_type: SipInfoDtmfContentType,
    /// Additional parameters
    pub parameters: HashMap<String, String>,
}

impl SipInfoDtmfMessage {
    /// Create new SIP INFO DTMF message
    pub fn new(digit: char, content_type: SipInfoDtmfContentType) -> Self {
        Self {
            digit,
            duration: None,
            volume: None,
            content_type,
            parameters: HashMap::new(),
        }
    }

    /// Create message with duration
    pub fn with_duration(mut self, duration: u32) -> Self {
        self.duration = Some(duration);
        self
    }

    /// Create message with volume
    pub fn with_volume(mut self, volume: u32) -> Self {
        self.volume = Some(volume.min(100));
        self
    }

    /// Add custom parameter
    pub fn with_parameter(mut self, key: String, value: String) -> Self {
        self.parameters.insert(key, value);
        self
    }

    /// Serialize to SIP INFO body content
    pub fn to_body_content(&self) -> String {
        match self.content_type {
            SipInfoDtmfContentType::CiscoDtmfRelay => self.to_cisco_format(),
            SipInfoDtmfContentType::GenericDtmf => self.to_generic_format(),
            SipInfoDtmfContentType::NortelText => self.to_nortel_format(),
            SipInfoDtmfContentType::Custom(_) => self.to_generic_format(),
        }
    }

    /// Cisco DTMF relay format
    fn to_cisco_format(&self) -> String {
        let mut body = format!("Signal={}\r\n", self.digit);

        if let Some(duration) = self.duration {
            body.push_str(&format!("Duration={}\r\n", duration));
        }

        if let Some(volume) = self.volume {
            // Cisco uses 0-63 scale
            let cisco_volume = (volume * 63) / 100;
            body.push_str(&format!("Volume={}\r\n", cisco_volume));
        }

        // Add custom parameters
        for (key, value) in &self.parameters {
            body.push_str(&format!("{}={}\r\n", key, value));
        }

        body
    }

    /// Generic DTMF format
    fn to_generic_format(&self) -> String {
        let mut parts = vec![format!("Signal={}", self.digit)];

        if let Some(duration) = self.duration {
            parts.push(format!("Duration={}", duration));
        }

        if let Some(volume) = self.volume {
            parts.push(format!("Volume={}", volume));
        }

        // Add custom parameters
        for (key, value) in &self.parameters {
            parts.push(format!("{}={}", key, value));
        }

        parts.join("\r\n") + "\r\n"
    }

    /// Nortel text format
    fn to_nortel_format(&self) -> String {
        // Nortel uses simple text format
        let mut body = self.digit.to_string();

        if let Some(duration) = self.duration {
            body = format!("{},{}", body, duration);
        }

        body
    }

    /// Parse SIP INFO body content to DTMF message
    pub fn from_body_content(content_type: &str, body: &str) -> Result<Self> {
        let content_type_enum = SipInfoDtmfContentType::from_mime_type(content_type);

        match content_type_enum {
            SipInfoDtmfContentType::CiscoDtmfRelay | SipInfoDtmfContentType::GenericDtmf => {
                Self::parse_key_value_format(body, content_type_enum)
            }
            SipInfoDtmfContentType::NortelText => Self::parse_nortel_format(body),
            SipInfoDtmfContentType::Custom(_) => {
                Self::parse_key_value_format(body, content_type_enum)
            }
        }
    }

    /// Parse key-value format (Cisco/Generic)
    fn parse_key_value_format(body: &str, content_type: SipInfoDtmfContentType) -> Result<Self> {
        let mut message = SipInfoDtmfMessage {
            digit: '?', // Will be set from Signal parameter
            duration: None,
            volume: None,
            content_type: content_type.clone(),
            parameters: HashMap::new(),
        };

        let mut signal_found = false;

        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim().to_lowercase();
                let value = line[eq_pos + 1..].trim();

                match key.as_str() {
                    "signal" => {
                        if let Some(digit_char) = value.chars().next() {
                            message.digit = digit_char;
                            signal_found = true;
                        }
                    }
                    "duration" => {
                        if let Ok(duration) = value.parse::<u32>() {
                            message.duration = Some(duration);
                        }
                    }
                    "volume" => {
                        if let Ok(volume) = value.parse::<u32>() {
                            // Convert Cisco volume (0-63) to percentage if needed
                            let normalized_volume =
                                if matches!(content_type, SipInfoDtmfContentType::CiscoDtmfRelay) {
                                    (volume * 100) / 63
                                } else {
                                    volume
                                };
                            message.volume = Some(normalized_volume.min(100));
                        }
                    }
                    _ => {
                        // Store as custom parameter
                        message.parameters.insert(key, value.to_string());
                    }
                }
            }
        }

        if !signal_found {
            return Err(anyhow!("Missing Signal parameter in SIP INFO DTMF body"));
        }

        Ok(message)
    }

    /// Parse Nortel text format
    fn parse_nortel_format(body: &str) -> Result<Self> {
        let body = body.trim();
        let parts: Vec<&str> = body.split(',').collect();

        if parts.is_empty() {
            return Err(anyhow!("Empty Nortel DTMF body"));
        }

        let digit = parts[0]
            .chars()
            .next()
            .ok_or_else(|| anyhow!("Invalid digit in Nortel DTMF body"))?;

        let duration = if parts.len() > 1 {
            parts[1].parse::<u32>().ok()
        } else {
            None
        };

        Ok(SipInfoDtmfMessage {
            digit,
            duration,
            volume: None,
            content_type: SipInfoDtmfContentType::NortelText,
            parameters: HashMap::new(),
        })
    }
}

/// SIP INFO DTMF processor
pub struct SipInfoDtmfProcessor {
    /// Event sender for integration with DTMF processor
    event_sender: mpsc::UnboundedSender<DtmfEvent>,
    /// Active sessions
    active_sessions: Arc<RwLock<HashMap<String, SipInfoSession>>>,
    /// Supported content types
    supported_content_types: Vec<SipInfoDtmfContentType>,
    /// Preferred content type for outgoing
    preferred_content_type: SipInfoDtmfContentType,
}

/// Active SIP INFO session state
#[derive(Debug, Clone)]
struct SipInfoSession {
    session_id: String,
    call_id: String,
    from_tag: String,
    to_tag: String,
    last_activity: Instant,
    dtmf_sequence: String,
}

impl SipInfoDtmfProcessor {
    /// Create new SIP INFO DTMF processor
    pub fn new(event_sender: mpsc::UnboundedSender<DtmfEvent>) -> Self {
        Self {
            event_sender,
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            supported_content_types: vec![
                SipInfoDtmfContentType::CiscoDtmfRelay,
                SipInfoDtmfContentType::GenericDtmf,
                SipInfoDtmfContentType::NortelText,
            ],
            preferred_content_type: SipInfoDtmfContentType::CiscoDtmfRelay,
        }
    }

    /// Set preferred content type for outgoing SIP INFO
    pub fn set_preferred_content_type(&mut self, content_type: SipInfoDtmfContentType) {
        self.preferred_content_type = content_type;
    }

    /// Add supported content type
    pub fn add_supported_content_type(&mut self, content_type: SipInfoDtmfContentType) {
        if !self.supported_content_types.contains(&content_type) {
            self.supported_content_types.push(content_type);
        }
    }

    /// Process incoming SIP INFO request
    pub async fn process_incoming_info(
        &self,
        session_id: &str,
        call_id: &str,
        from_tag: &str,
        to_tag: &str,
        content_type: &str,
        body: &str,
    ) -> Result<SipInfoResponse> {
        debug!(
            "Processing SIP INFO DTMF for session {}: {}",
            session_id, content_type
        );

        // Check if content type is supported
        let content_type_enum = SipInfoDtmfContentType::from_mime_type(content_type);
        if !self.is_content_type_supported(&content_type_enum) {
            return Ok(SipInfoResponse::UnsupportedMediaType);
        }

        // Parse DTMF message
        let dtmf_message = SipInfoDtmfMessage::from_body_content(content_type, body)?;

        // Update or create session
        self.update_session(session_id, call_id, from_tag, to_tag, dtmf_message.digit)
            .await;

        // Generate DTMF event
        let duration = dtmf_message
            .duration
            .map(|d| Duration::from_millis(d.into()))
            .unwrap_or(Duration::from_millis(100)); // Default 100ms

        let confidence = dtmf_message
            .volume
            .map(|v| (v as f32) / 100.0)
            .unwrap_or(0.8); // Default confidence

        let dtmf_event = DtmfEvent::DigitDetected {
            digit: dtmf_message.digit,
            duration,
            timestamp: Instant::now(),
            confidence,
            source: DtmfSource::SipInfo,
        };

        // Send event
        if let Err(e) = self.event_sender.send(dtmf_event) {
            warn!("Failed to send DTMF event from SIP INFO: {}", e);
        } else {
            info!(
                "SIP INFO DTMF '{}' received for session {} (duration: {:?})",
                dtmf_message.digit, session_id, duration
            );
        }

        Ok(SipInfoResponse::Ok)
    }

    /// Generate SIP INFO request for outgoing DTMF
    pub async fn generate_outgoing_info(
        &self,
        session_id: &str,
        digit: char,
        duration: Option<u32>,
        volume: Option<u32>,
    ) -> Result<SipInfoRequest> {
        let message = SipInfoDtmfMessage::new(digit, self.preferred_content_type.clone())
            .with_duration(duration.unwrap_or(100))
            .with_volume(volume.unwrap_or(50));

        let request = SipInfoRequest {
            method: "INFO".to_string(),
            content_type: self.preferred_content_type.to_mime_type().to_string(),
            content_length: 0, // Will be calculated
            body: message.to_body_content(),
            headers: self.generate_info_headers(session_id).await,
        };

        info!(
            "Generated SIP INFO DTMF '{}' for session {}",
            digit, session_id
        );
        Ok(request)
    }

    /// Check if content type is supported
    fn is_content_type_supported(&self, content_type: &SipInfoDtmfContentType) -> bool {
        // A Custom content type is only accepted if it was explicitly registered
        // via add_supported_content_type. Unknown MIME types are rejected so the
        // caller can respond with 415 Unsupported Media Type.
        self.supported_content_types.contains(content_type)
    }

    /// Update session state
    async fn update_session(
        &self,
        session_id: &str,
        call_id: &str,
        from_tag: &str,
        to_tag: &str,
        digit: char,
    ) {
        let mut sessions = self.active_sessions.write().await;

        match sessions.get_mut(session_id) {
            Some(session) => {
                session.last_activity = Instant::now();
                session.dtmf_sequence.push(digit);
            }
            None => {
                let session = SipInfoSession {
                    session_id: session_id.to_string(),
                    call_id: call_id.to_string(),
                    from_tag: from_tag.to_string(),
                    to_tag: to_tag.to_string(),
                    last_activity: Instant::now(),
                    dtmf_sequence: digit.to_string(),
                };
                sessions.insert(session_id.to_string(), session);
            }
        }
    }

    /// Generate SIP INFO headers
    async fn generate_info_headers(&self, session_id: &str) -> HashMap<String, String> {
        let mut headers = HashMap::new();

        // Standard INFO headers
        headers.insert("Info-Package".to_string(), "dtmf-relay".to_string());

        // Session-specific headers would be added here based on dialog state
        // This would typically be handled by the SIP stack

        headers
    }

    /// Get session statistics
    pub async fn get_session_stats(&self, session_id: &str) -> Option<SipInfoSessionStats> {
        let sessions = self.active_sessions.read().await;
        sessions.get(session_id).map(|session| SipInfoSessionStats {
            session_id: session.session_id.clone(),
            call_id: session.call_id.clone(),
            dtmf_sequence: session.dtmf_sequence.clone(),
            last_activity: session.last_activity,
        })
    }

    /// Clean up inactive sessions
    pub async fn cleanup_inactive_sessions(&self, max_age: Duration) {
        let mut sessions = self.active_sessions.write().await;
        let now = Instant::now();
        let mut to_remove = Vec::new();

        for (session_id, session) in sessions.iter() {
            if now.duration_since(session.last_activity) > max_age {
                to_remove.push(session_id.clone());
            }
        }

        for session_id in to_remove {
            sessions.remove(&session_id);
            debug!("Cleaned up inactive SIP INFO session: {}", session_id);
        }
    }

    /// Get all active sessions
    pub async fn get_active_sessions(&self) -> Vec<SipInfoSessionStats> {
        let sessions = self.active_sessions.read().await;
        sessions
            .values()
            .map(|session| SipInfoSessionStats {
                session_id: session.session_id.clone(),
                call_id: session.call_id.clone(),
                dtmf_sequence: session.dtmf_sequence.clone(),
                last_activity: session.last_activity,
            })
            .collect()
    }
}

/// SIP INFO response types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SipInfoResponse {
    /// 200 OK
    Ok,
    /// 415 Unsupported Media Type
    UnsupportedMediaType,
    /// 481 Call/Transaction Does Not Exist
    CallDoesNotExist,
    /// 500 Internal Server Error
    InternalError,
}

impl SipInfoResponse {
    /// Get SIP status code
    pub fn status_code(&self) -> u16 {
        match self {
            Self::Ok => 200,
            Self::UnsupportedMediaType => 415,
            Self::CallDoesNotExist => 481,
            Self::InternalError => 500,
        }
    }

    /// Get reason phrase
    pub fn reason_phrase(&self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::UnsupportedMediaType => "Unsupported Media Type",
            Self::CallDoesNotExist => "Call/Transaction Does Not Exist",
            Self::InternalError => "Internal Server Error",
        }
    }
}

/// SIP INFO request structure
#[derive(Debug, Clone)]
pub struct SipInfoRequest {
    pub method: String,
    pub content_type: String,
    pub content_length: usize,
    pub body: String,
    pub headers: HashMap<String, String>,
}

impl SipInfoRequest {
    /// Calculate and set content length
    pub fn calculate_content_length(&mut self) {
        self.content_length = self.body.len();
    }
}

/// SIP INFO session statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipInfoSessionStats {
    pub session_id: String,
    pub call_id: String,
    pub dtmf_sequence: String,
    #[serde(skip, default = "std::time::Instant::now")]
    pub last_activity: Instant,
}

/// SIP INFO package negotiation
pub struct SipInfoPackageNegotiator {
    /// Supported INFO packages
    supported_packages: Vec<String>,
}

impl SipInfoPackageNegotiator {
    /// Create new package negotiator
    pub fn new() -> Self {
        Self {
            supported_packages: vec!["dtmf-relay".to_string(), "dtmf".to_string()],
        }
    }

    /// Add supported package
    pub fn add_package(&mut self, package: String) {
        if !self.supported_packages.contains(&package) {
            self.supported_packages.push(package);
        }
    }

    /// Generate Recv-Info header value
    pub fn generate_recv_info_header(&self) -> String {
        self.supported_packages.join(", ")
    }

    /// Parse Recv-Info header to get supported packages
    pub fn parse_recv_info_header(&self, header_value: &str) -> Vec<String> {
        header_value
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Check if package is supported
    pub fn is_package_supported(&self, package: &str) -> bool {
        self.supported_packages.contains(&package.to_string())
    }
}

impl Default for SipInfoPackageNegotiator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[test]
    fn test_sip_info_message_cisco_format() {
        let message = SipInfoDtmfMessage::new('5', SipInfoDtmfContentType::CiscoDtmfRelay)
            .with_duration(200)
            .with_volume(80);

        let body = message.to_body_content();
        assert!(body.contains("Signal=5"));
        assert!(body.contains("Duration=200"));
        assert!(body.contains("Volume=50")); // 80% of 63 ≈ 50

        // Test round-trip
        let parsed =
            SipInfoDtmfMessage::from_body_content("application/dtmf-relay", &body).unwrap();
        assert_eq!(parsed.digit, '5');
        assert_eq!(parsed.duration, Some(200));
        assert_eq!(parsed.volume, Some(79)); // Should convert back (precision loss from integer division)
    }

    #[test]
    fn test_sip_info_message_generic_format() {
        let message =
            SipInfoDtmfMessage::new('*', SipInfoDtmfContentType::GenericDtmf).with_duration(100);

        let body = message.to_body_content();
        assert!(body.contains("Signal=*"));
        assert!(body.contains("Duration=100"));

        // Test parsing
        let parsed = SipInfoDtmfMessage::from_body_content("application/dtmf", &body).unwrap();
        assert_eq!(parsed.digit, '*');
        assert_eq!(parsed.duration, Some(100));
    }

    #[test]
    fn test_sip_info_message_nortel_format() {
        let message =
            SipInfoDtmfMessage::new('#', SipInfoDtmfContentType::NortelText).with_duration(150);

        let body = message.to_body_content();
        assert_eq!(body, "#,150");

        // Test parsing
        let parsed =
            SipInfoDtmfMessage::from_body_content("application/vnd.nortel.text", &body).unwrap();
        assert_eq!(parsed.digit, '#');
        assert_eq!(parsed.duration, Some(150));
    }

    #[tokio::test]
    async fn test_sip_info_processor() {
        let (event_sender, mut event_receiver) = mpsc::unbounded_channel();
        let processor = SipInfoDtmfProcessor::new(event_sender);

        // Process incoming SIP INFO
        let response = processor
            .process_incoming_info(
                "test_session",
                "test_call_id",
                "from_tag",
                "to_tag",
                "application/dtmf-relay",
                "Signal=7\r\nDuration=150\r\n",
            )
            .await
            .unwrap();

        assert_eq!(response, SipInfoResponse::Ok);

        // Should receive DTMF event
        let received_event = event_receiver.try_recv().unwrap();
        match received_event {
            DtmfEvent::DigitDetected { digit, source, .. } => {
                assert_eq!(digit, '7');
                assert_eq!(source, DtmfSource::SipInfo);
            }
            _ => assert!(
                false,
                "Expected DigitDetected event, got: {:?}",
                received_event
            ),
        }
    }

    #[test]
    fn test_package_negotiation() {
        let negotiator = SipInfoPackageNegotiator::new();

        let recv_info = negotiator.generate_recv_info_header();
        assert!(recv_info.contains("dtmf-relay"));
        assert!(recv_info.contains("dtmf"));

        let packages = negotiator.parse_recv_info_header("dtmf-relay, dtmf, custom-package");
        assert_eq!(packages.len(), 3);
        assert!(packages.contains(&"dtmf-relay".to_string()));

        assert!(negotiator.is_package_supported("dtmf-relay"));
        assert!(!negotiator.is_package_supported("unknown-package"));
    }
}
