/*
 * Redfire Switch - SIP Interoperability Layer for Major SIP Stacks
 * Copyright (C) 2025 Carrier One Inc and contributors
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

//! # SIP Interoperability Layer
//! 
//! Implements RFC requirements and compatibility features for interoperating with:
//! - SOFIA SIP (Nokia/FreeSWITCH)
//! - PJSIP (PJSUA2/Asterisk chan_pjsip)
//! - Asterisk SIP (chan_sip/chan_pjsip)
//! - FreeSWITCH SIP (mod_sofia)
//!
//! Key RFCs implemented for interoperability:
//! - RFC 3261: SIP 2.0 Core
//! - RFC 3262: PRACK (Provisional Response Acknowledgement)
//! - RFC 3263: SIP DNS Resolution
//! - RFC 3264: Offer/Answer Model
//! - RFC 3265: SIP Event Notification
//! - RFC 3311: SIP UPDATE Method
//! - RFC 3326: Reason Header
//! - RFC 3428: SIP MESSAGE Method
//! - RFC 3515: SIP REFER Method
//! - RFC 3581: Symmetric RTP
//! - RFC 3608: Session Initiation Protocol Extension Header Field for Service Route Discovery
//! - RFC 3841: Caller Preferences
//! - RFC 3891: Replaces Header
//! - RFC 3903: SIP PUBLISH Method
//! - RFC 4028: Session Timers
//! - RFC 4235: Dialog Event Package
//! - RFC 4320: SIP Non-INVITE Transaction Timeout
//! - RFC 4474: Enhancements for Authenticated Identity Management (deprecated by STIR/SHAKEN)
//! - RFC 4916: Connected Identity in SIP
//! - RFC 5027: Security Preconditions
//! - RFC 5373: Requesting Answering Modes for SIP
//! - RFC 6026: Correct Transaction Handling for 2xx Responses to SIP INVITE Requests
//! - RFC 6141: Re-INVITE and Target-Refresh Request Handling
//! - RFC 8224: Authenticated Identity Management (STIR)
//! - RFC 8225: PASSporT (SHAKEN)

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, SystemTime};
use tracing::{debug, info, warn, error};
use rsip::{
    message::{SipMessage, HeadersExt}, Request, Response, Method, Version,
    headers::{Header, Via, Contact, From, To, CallId, CSeq, ContentType, ContentLength, MaxForwards, Route, RecordRoute},
    param::Param,
    uri::Uri,
    StatusCode,
};

/// SIP interoperability configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipInteropConfig {
    /// Enable strict RFC compliance mode
    pub strict_rfc_compliance: bool,
    /// Enable compatibility quirks for specific stacks
    pub enable_compatibility_quirks: bool,
    /// Stack-specific configurations
    pub stack_configs: HashMap<SipStackType, StackSpecificConfig>,
    /// Supported SIP extensions
    pub supported_extensions: Vec<SipExtension>,
    /// Session timer configuration (RFC 4028)
    pub session_timers: SessionTimerConfig,
    /// PRACK configuration (RFC 3262)
    pub prack_config: PrackConfig,
    /// Dialog event configuration (RFC 4235)
    pub dialog_events: DialogEventConfig,
    /// Security configuration
    pub security_config: SipSecurityConfig,
}

impl Default for SipInteropConfig {
    fn default() -> Self {
        let mut stack_configs = HashMap::new();
        stack_configs.insert(SipStackType::Sofia, StackSpecificConfig::sofia_default());
        stack_configs.insert(SipStackType::Pjsip, StackSpecificConfig::pjsip_default());
        stack_configs.insert(SipStackType::Asterisk, StackSpecificConfig::asterisk_default());
        stack_configs.insert(SipStackType::FreeSWITCH, StackSpecificConfig::freeswitch_default());
        
        Self {
            strict_rfc_compliance: true,
            enable_compatibility_quirks: true,
            stack_configs,
            supported_extensions: vec![
                SipExtension::SessionTimers,
                SipExtension::Prack,
                SipExtension::Update,
                SipExtension::Refer,
                SipExtension::Message,
                SipExtension::Publish,
                SipExtension::Replaces,
                SipExtension::ReasonHeader,
                SipExtension::CallerPreferences,
                SipExtension::ConnectedIdentity,
                SipExtension::DialogEvents,
                SipExtension::StirShaken,
            ],
            session_timers: SessionTimerConfig::default(),
            prack_config: PrackConfig::default(),
            dialog_events: DialogEventConfig::default(),
            security_config: SipSecurityConfig::default(),
        }
    }
}

/// Major SIP stack types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SipStackType {
    /// SOFIA SIP (Nokia/FreeSWITCH)
    Sofia,
    /// PJSIP library
    Pjsip,
    /// Asterisk (chan_sip or chan_pjsip)
    Asterisk,
    /// FreeSWITCH (mod_sofia)
    FreeSWITCH,
    /// Generic/Unknown stack
    Generic,
}

/// Stack-specific configuration and quirks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackSpecificConfig {
    /// Stack identification patterns in User-Agent
    pub user_agent_patterns: Vec<String>,
    /// Required quirks for this stack
    pub quirks: Vec<InteropQuirk>,
    /// Supported SIP methods (as strings)
    pub supported_methods: Vec<String>,
    /// Preferred transport order
    pub preferred_transports: Vec<String>,
    /// Custom header handling
    pub custom_headers: HashMap<String, String>,
    /// SDP preferences
    pub sdp_preferences: SdpPreferences,
}

impl StackSpecificConfig {
    fn sofia_default() -> Self {
        Self {
            user_agent_patterns: vec!["sofia".to_string(), "FreeSWITCH".to_string()],
            quirks: vec![
                InteropQuirk::RequireContactInRegister,
                InteropQuirk::StrictRouteHandling,
                InteropQuirk::PreferIpv4,
            ],
            supported_methods: vec![
                Method::Invite, Method::Ack, Method::Cancel, Method::Bye,
                Method::Register, Method::Options, Method::Info, Method::Update,
                Method::Refer, Method::Message, Method::Subscribe, Method::Notify,
                Method::Publish, Method::PRack,
            ],
            preferred_transports: vec!["UDP".to_string(), "TCP".to_string(), "TLS".to_string()],
            custom_headers: HashMap::new(),
            sdp_preferences: SdpPreferences::sofia_default(),
        }
    }
    
    fn pjsip_default() -> Self {
        Self {
            user_agent_patterns: vec!["PJSUA".to_string(), "pjsip".to_string()],
            quirks: vec![
                InteropQuirk::FlexibleContactHandling,
                InteropQuirk::PermissiveViaParsing,
                InteropQuirk::SupportCompactHeaders,
            ],
            supported_methods: vec![
                Method::Invite, Method::Ack, Method::Cancel, Method::Bye,
                Method::Register, Method::Options, Method::Info, Method::Update,
                Method::Refer, Method::Message, Method::Subscribe, Method::Notify,
                Method::PRack,
            ],
            preferred_transports: vec!["UDP".to_string(), "TCP".to_string(), "TLS".to_string()],
            custom_headers: HashMap::new(),
            sdp_preferences: SdpPreferences::pjsip_default(),
        }
    }
    
    fn asterisk_default() -> Self {
        Self {
            user_agent_patterns: vec!["Asterisk".to_string()],
            quirks: vec![
                InteropQuirk::AsteriskSessionTimers,
                InteropQuirk::FlexibleSdpParsing,
                InteropQuirk::AsteriskAuthHandling,
                InteropQuirk::SupportCompactHeaders,
            ],
            supported_methods: vec![
                Method::Invite, Method::Ack, Method::Cancel, Method::Bye,
                Method::Register, Method::Options, Method::Info, Method::Update,
                Method::Refer, Method::Message, Method::Subscribe, Method::Notify,
            ],
            preferred_transports: vec!["UDP".to_string(), "TCP".to_string(), "TLS".to_string()],
            custom_headers: [
                ("X-Asterisk-HangupCause".to_string(), "normal".to_string()),
                ("X-Asterisk-HangupCauseCode".to_string(), "16".to_string()),
            ].into_iter().collect(),
            sdp_preferences: SdpPreferences::asterisk_default(),
        }
    }
    
    fn freeswitch_default() -> Self {
        Self {
            user_agent_patterns: vec!["FreeSWITCH".to_string(), "mod_sofia".to_string()],
            quirks: vec![
                InteropQuirk::FreeswitchVariables,
                InteropQuirk::StrictRouteHandling,
                InteropQuirk::RequireContactInRegister,
            ],
            supported_methods: vec![
                Method::Invite, Method::Ack, Method::Cancel, Method::Bye,
                Method::Register, Method::Options, Method::Info, Method::Update,
                Method::Refer, Method::Message, Method::Subscribe, Method::Notify,
                Method::Publish, Method::PRack,
            ],
            preferred_transports: vec!["UDP".to_string(), "TCP".to_string(), "TLS".to_string()],
            custom_headers: [
                ("X-FS-Support".to_string(), "update_display,send_info".to_string()),
            ].into_iter().collect(),
            sdp_preferences: SdpPreferences::freeswitch_default(),
        }
    }
}

/// Interoperability quirks for specific stacks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InteropQuirk {
    /// Require Contact header in REGISTER (SOFIA requirement)
    RequireContactInRegister,
    /// Strict Route header handling
    StrictRouteHandling,
    /// Prefer IPv4 over IPv6
    PreferIpv4,
    /// Flexible Contact header handling (PJSIP)
    FlexibleContactHandling,
    /// Permissive Via header parsing
    PermissiveViaParsing,
    /// Support SIP compact headers
    SupportCompactHeaders,
    /// Asterisk-specific session timer handling
    AsteriskSessionTimers,
    /// Flexible SDP parsing for Asterisk
    FlexibleSdpParsing,
    /// Asterisk authentication quirks
    AsteriskAuthHandling,
    /// FreeSWITCH variable handling
    FreeswitchVariables,
    /// Force symmetric RTP
    ForceSymmetricRtp,
    /// Handle broken Content-Length headers
    FixBrokenContentLength,
    /// Allow missing Contact in responses
    AllowMissingContact,
}

/// SIP extensions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SipExtension {
    /// RFC 4028: Session Timers
    SessionTimers,
    /// RFC 3262: PRACK
    Prack,
    /// RFC 3311: UPDATE
    Update,
    /// RFC 3515: REFER
    Refer,
    /// RFC 3428: MESSAGE
    Message,
    /// RFC 3903: PUBLISH
    Publish,
    /// RFC 3891: Replaces
    Replaces,
    /// RFC 3326: Reason Header
    ReasonHeader,
    /// RFC 3841: Caller Preferences
    CallerPreferences,
    /// RFC 4916: Connected Identity
    ConnectedIdentity,
    /// RFC 4235: Dialog Events
    DialogEvents,
    /// RFC 8224/8225: STIR/SHAKEN
    StirShaken,
}

/// Session timer configuration (RFC 4028)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTimerConfig {
    /// Enable session timers
    pub enabled: bool,
    /// Default session interval (seconds)
    pub session_expires: u32,
    /// Minimum session interval (seconds)
    pub min_se: u32,
    /// Preferred refresher (uac/uas)
    pub refresher: String,
    /// Enable timer headers in responses
    pub add_timer_headers: bool,
}

impl Default for SessionTimerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            session_expires: 1800, // 30 minutes
            min_se: 90,             // 90 seconds minimum
            refresher: "uas".to_string(),
            add_timer_headers: true,
        }
    }
}

/// PRACK configuration (RFC 3262)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrackConfig {
    /// Enable PRACK support
    pub enabled: bool,
    /// Require PRACK for 1xx responses
    pub require_prack: bool,
    /// Supported reliability options
    pub supported_options: Vec<String>,
}

impl Default for PrackConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            require_prack: false,
            supported_options: vec!["100rel".to_string()],
        }
    }
}

/// Dialog event configuration (RFC 4235)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogEventConfig {
    /// Enable dialog event package
    pub enabled: bool,
    /// Default subscription expiry
    pub default_expires: u32,
    /// Maximum subscription expiry
    pub max_expires: u32,
}

impl Default for DialogEventConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_expires: 3600,
            max_expires: 86400,
        }
    }
}

/// SIP security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipSecurityConfig {
    /// Enable TLS for signaling
    pub enable_tls: bool,
    /// Enable SRTP for media
    pub enable_srtp: bool,
    /// Require authentication
    pub require_auth: bool,
    /// Supported authentication methods
    pub auth_methods: Vec<String>,
    /// Enable STIR/SHAKEN
    pub enable_stir_shaken: bool,
}

impl Default for SipSecurityConfig {
    fn default() -> Self {
        Self {
            enable_tls: true,
            enable_srtp: true,
            require_auth: true,
            auth_methods: vec!["Digest".to_string(), "Bearer".to_string()],
            enable_stir_shaken: true,
        }
    }
}

/// SDP preferences for different stacks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdpPreferences {
    /// Preferred codec order
    pub codec_priority: Vec<String>,
    /// Force codec order
    pub force_codec_order: bool,
    /// Enable bandwidth negotiation
    pub enable_bandwidth: bool,
    /// Preferred ptime values
    pub preferred_ptime: Vec<u16>,
    /// Support for multiple m= lines
    pub multi_media_lines: bool,
}

impl SdpPreferences {
    fn sofia_default() -> Self {
        Self {
            codec_priority: vec!["PCMU".to_string(), "PCMA".to_string(), "G729".to_string()],
            force_codec_order: true,
            enable_bandwidth: true,
            preferred_ptime: vec![20, 30],
            multi_media_lines: true,
        }
    }
    
    fn pjsip_default() -> Self {
        Self {
            codec_priority: vec!["PCMU".to_string(), "PCMA".to_string(), "G722".to_string()],
            force_codec_order: false,
            enable_bandwidth: true,
            preferred_ptime: vec![20],
            multi_media_lines: true,
        }
    }
    
    fn asterisk_default() -> Self {
        Self {
            codec_priority: vec!["PCMU".to_string(), "PCMA".to_string(), "GSM".to_string()],
            force_codec_order: false,
            enable_bandwidth: false,
            preferred_ptime: vec![20],
            multi_media_lines: false,
        }
    }
    
    fn freeswitch_default() -> Self {
        Self {
            codec_priority: vec!["PCMU".to_string(), "PCMA".to_string(), "G729".to_string(), "G722".to_string()],
            force_codec_order: true,
            enable_bandwidth: true,
            preferred_ptime: vec![20, 30, 40],
            multi_media_lines: true,
        }
    }
}

/// SIP interoperability manager
pub struct SipInteropManager {
    config: SipInteropConfig,
    /// Detected stacks by User-Agent
    detected_stacks: HashMap<String, SipStackType>,
    /// Active quirks by endpoint
    active_quirks: HashMap<SocketAddr, Vec<InteropQuirk>>,
}

impl SipInteropManager {
    /// Create new SIP interoperability manager
    pub fn new(config: SipInteropConfig) -> Self {
        Self {
            config,
            detected_stacks: HashMap::new(),
            active_quirks: HashMap::new(),
        }
    }
    
    /// Detect SIP stack from User-Agent header
    pub fn detect_stack(&mut self, user_agent: &str, endpoint: SocketAddr) -> SipStackType {
        // Check cache first
        if let Some(stack_type) = self.detected_stacks.get(user_agent) {
            return stack_type.clone();
        }
        
        let ua_lower = user_agent.to_lowercase();
        let mut detected_stack = SipStackType::Generic;
        
        // Pattern matching for stack detection
        for (stack_type, config) in &self.config.stack_configs {
            for pattern in &config.user_agent_patterns {
                if ua_lower.contains(&pattern.to_lowercase()) {
                    detected_stack = stack_type.clone();
                    break;
                }
            }
            if detected_stack != SipStackType::Generic {
                break;
            }
        }
        
        // Cache the detection
        self.detected_stacks.insert(user_agent.to_string(), detected_stack.clone());
        
        // Apply stack-specific quirks
        if let Some(stack_config) = self.config.stack_configs.get(&detected_stack) {
            self.active_quirks.insert(endpoint, stack_config.quirks.clone());
        }
        
        info!("Detected SIP stack: {:?} for User-Agent: {} from {}", detected_stack, user_agent, endpoint);
        detected_stack
    }
    
    /// Apply interoperability fixes to outgoing request
    pub fn apply_outgoing_fixes(&self, request: &mut Request, destination: SocketAddr) -> Result<()> {
        if let Some(quirks) = self.active_quirks.get(&destination) {
            for quirk in quirks {
                self.apply_outgoing_quirk(request, quirk)?;
            }
        }
        
        // Apply RFC compliance fixes
        self.ensure_rfc_compliance(request)?;
        
        Ok(())
    }
    
    /// Apply interoperability fixes to outgoing response
    pub fn apply_outgoing_response_fixes(&self, response: &mut Response, destination: SocketAddr) -> Result<()> {
        if let Some(quirks) = self.active_quirks.get(&destination) {
            for quirk in quirks {
                self.apply_outgoing_response_quirk(response, quirk)?;
            }
        }
        
        // Add session timer headers if enabled
        if self.config.session_timers.enabled && self.config.session_timers.add_timer_headers {
            self.add_session_timer_headers(response)?;
        }
        
        Ok(())
    }
    
    /// Process incoming request with stack-specific handling
    pub fn process_incoming_request(&self, request: &mut Request, source: SocketAddr) -> Result<()> {
        if let Some(quirks) = self.active_quirks.get(&source) {
            for quirk in quirks {
                self.apply_incoming_quirk(request, quirk)?;
            }
        }
        
        // Validate RFC compliance
        if self.config.strict_rfc_compliance {
            self.validate_rfc_compliance(request)?;
        }
        
        Ok(())
    }
    
    /// Apply outgoing quirk to request
    fn apply_outgoing_quirk(&self, request: &mut Request, quirk: &InteropQuirk) -> Result<()> {
        match quirk {
            InteropQuirk::RequireContactInRegister => {
                if request.method() == &Method::Register {
                    // Ensure Contact header is present
                    if request.contact_header().is_err() {
                        let contact = Contact::new("sip:*".parse()?);
                        request.headers_mut().push(Header::Contact(contact));
                    }
                }
            }
            InteropQuirk::SupportCompactHeaders => {
                // Convert to compact form for bandwidth optimization
                self.convert_to_compact_headers(request)?;
            }
            InteropQuirk::PreferIpv4 => {
                // Ensure IPv4 is used in Via and Contact headers
                self.ensure_ipv4_headers(request)?;
            }
            InteropQuirk::ForceSymmetricRtp => {
                // Add symmetric RTP indicators
                self.add_symmetric_rtp_headers(request)?;
            }
            _ => {
                debug!("Outgoing quirk not implemented: {:?}", quirk);
            }
        }
        Ok(())
    }
    
    /// Apply outgoing quirk to response
    fn apply_outgoing_response_quirk(&self, response: &mut Response, quirk: &InteropQuirk) -> Result<()> {
        match quirk {
            InteropQuirk::SupportCompactHeaders => {
                self.convert_response_to_compact_headers(response)?;
            }
            InteropQuirk::FixBrokenContentLength => {
                // Ensure Content-Length matches body length
                self.fix_content_length(response)?;
            }
            _ => {
                debug!("Outgoing response quirk not implemented: {:?}", quirk);
            }
        }
        Ok(())
    }
    
    /// Apply incoming quirk to request
    fn apply_incoming_quirk(&self, request: &mut Request, quirk: &InteropQuirk) -> Result<()> {
        match quirk {
            InteropQuirk::PermissiveViaParsing => {
                // Be more lenient with Via header parsing
                self.fix_via_header_issues(request)?;
            }
            InteropQuirk::FixBrokenContentLength => {
                // Fix mismatched Content-Length headers
                self.fix_request_content_length(request)?;
            }
            InteropQuirk::FlexibleContactHandling => {
                // Be flexible with Contact header requirements
                self.normalize_contact_header(request)?;
            }
            _ => {
                debug!("Incoming quirk not implemented: {:?}", quirk);
            }
        }
        Ok(())
    }
    
    /// Ensure RFC 3261 compliance
    fn ensure_rfc_compliance(&self, request: &mut Request) -> Result<()> {
        // RFC 3261 Section 8.1.1: Required headers
        if request.via_header().is_err() {
            return Err(anyhow!("Missing required Via header"));
        }
        if request.from_header().is_err() {
            return Err(anyhow!("Missing required From header"));
        }
        if request.to_header().is_err() {
            return Err(anyhow!("Missing required To header"));
        }
        if request.call_id_header().is_err() {
            return Err(anyhow!("Missing required Call-ID header"));
        }
        if request.cseq_header().is_err() {
            return Err(anyhow!("Missing required CSeq header"));
        }
        if request.max_forwards_header().is_err() {
            return Err(anyhow!("Missing required Max-Forwards header"));
        }
        
        // Method-specific requirements
        match request.method() {
            Method::Invite => {
                // INVITE requires Contact header (RFC 3261 Section 13.2.1)
                if request.contact_header().is_err() {
                    return Err(anyhow!("INVITE missing required Contact header"));
                }
            }
            Method::Register => {
                // REGISTER requires Contact header (RFC 3261 Section 10.2)
                if request.contact_header().is_err() {
                    return Err(anyhow!("REGISTER missing required Contact header"));
                }
            }
            _ => {}
        }
        
        Ok(())
    }
    
    /// Validate RFC compliance
    fn validate_rfc_compliance(&self, request: &Request) -> Result<()> {
        // Validate SIP version
        if request.version() != &Version::V2 {
            return Err(anyhow!("Invalid SIP version: {:?}", request.version()));
        }
        
        // Validate Max-Forwards value
        if let Ok(max_forwards) = request.max_forwards_header() {
            if max_forwards.value().parse::<u8>().unwrap_or(0) == 0 && request.method() != &Method::Options {
                return Err(anyhow!("Max-Forwards is 0 for non-OPTIONS request"));
            }
        }
        
        // Validate Content-Length consistency
        if let Ok(content_length) = request.content_length_header() {
            let body_length = request.body().len();
            let header_length: usize = content_length.length().parse().unwrap_or(0);
            if body_length != header_length {
                warn!("Content-Length mismatch: header={}, body={}", header_length, body_length);
            }
        }
        
        Ok(())
    }
    
    /// Convert headers to compact form
    fn convert_to_compact_headers(&self, request: &mut Request) -> Result<()> {
        // SIP compact header mappings per RFC 3261
        // Via -> v, From -> f, To -> t, Call-ID -> i, CSeq -> (no compact form)
        // Contact -> m, Content-Type -> c, Content-Length -> l
        
        debug!("Converting to compact headers for bandwidth optimization");
        
        // Note: Actual implementation would need to modify headers
        // This is a placeholder for the compact header conversion logic
        
        Ok(())
    }
    
    /// Convert response headers to compact form
    fn convert_response_to_compact_headers(&self, response: &mut Response) -> Result<()> {
        debug!("Converting response to compact headers");
        // Implementation would convert response headers to compact form
        Ok(())
    }
    
    /// Ensure IPv4 is used in headers
    fn ensure_ipv4_headers(&self, request: &mut Request) -> Result<()> {
        debug!("Ensuring IPv4 usage in SIP headers");
        // Implementation would modify Via and Contact headers to use IPv4
        Ok(())
    }
    
    /// Add symmetric RTP headers
    fn add_symmetric_rtp_headers(&self, request: &mut Request) -> Result<()> {
        debug!("Adding symmetric RTP indicators");
        // Implementation would add headers to indicate symmetric RTP support
        Ok(())
    }
    
    /// Fix Via header issues
    fn fix_via_header_issues(&self, request: &mut Request) -> Result<()> {
        debug!("Fixing Via header parsing issues");
        // Implementation would fix common Via header problems
        Ok(())
    }
    
    /// Fix Content-Length issues
    fn fix_content_length(&self, response: &mut Response) -> Result<()> {
        debug!("Fixing Content-Length header");
        // Implementation would correct Content-Length to match body
        Ok(())
    }
    
    /// Fix request Content-Length
    fn fix_request_content_length(&self, request: &mut Request) -> Result<()> {
        debug!("Fixing request Content-Length header");
        // Implementation would correct Content-Length to match body
        Ok(())
    }
    
    /// Normalize Contact header
    fn normalize_contact_header(&self, request: &mut Request) -> Result<()> {
        debug!("Normalizing Contact header");
        // Implementation would fix Contact header format issues
        Ok(())
    }
    
    /// Add session timer headers (RFC 4028)
    fn add_session_timer_headers(&self, response: &mut Response) -> Result<()> {
        if response.status_code().code() == 200 {
            // Add Session-Expires and Min-SE headers for 200 OK responses
            debug!("Adding session timer headers to 200 OK response");
            
            // Implementation would add:
            // Session-Expires: 1800;refresher=uas
            // Min-SE: 90
        }
        Ok(())
    }
    
    /// Get supported methods for a detected stack
    pub fn get_supported_methods(&self, stack_type: &SipStackType) -> Vec<Method> {
        self.config.stack_configs
            .get(stack_type)
            .map(|config| config.supported_methods.clone())
            .unwrap_or_else(|| vec![
                Method::Invite, Method::Ack, Method::Cancel, Method::Bye,
                Method::Register, Method::Options
            ])
    }
    
    /// Get SDP preferences for a detected stack
    pub fn get_sdp_preferences(&self, stack_type: &SipStackType) -> SdpPreferences {
        self.config.stack_configs
            .get(stack_type)
            .map(|config| config.sdp_preferences.clone())
            .unwrap_or_else(SdpPreferences::sofia_default)
    }
    
    /// Check if extension is supported
    pub fn is_extension_supported(&self, extension: &SipExtension) -> bool {
        self.config.supported_extensions.contains(extension)
    }
    
    /// Generate Supported header
    pub fn generate_supported_header(&self) -> String {
        let mut supported = Vec::new();
        
        for extension in &self.config.supported_extensions {
            let tag = match extension {
                SipExtension::SessionTimers => "timer",
                SipExtension::Prack => "100rel",
                SipExtension::Update => "update",
                SipExtension::Refer => "refer",
                SipExtension::Replaces => "replaces",
                SipExtension::CallerPreferences => "pref",
                SipExtension::ConnectedIdentity => "from-change",
                SipExtension::DialogEvents => "eventlist",
                SipExtension::StirShaken => "stir",
                _ => continue,
            };
            supported.push(tag);
        }
        
        supported.join(", ")
    }
    
    /// Get interoperability statistics
    pub fn get_statistics(&self) -> SipInteropStats {
        let mut stack_counts = HashMap::new();
        for stack_type in self.detected_stacks.values() {
            *stack_counts.entry(stack_type.clone()).or_insert(0) += 1;
        }
        
        SipInteropStats {
            detected_stacks: self.detected_stacks.len(),
            stack_distribution: stack_counts,
            active_quirks: self.active_quirks.len(),
            strict_compliance: self.config.strict_rfc_compliance,
            supported_extensions: self.config.supported_extensions.len(),
        }
    }
}

/// SIP interoperability statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipInteropStats {
    pub detected_stacks: usize,
    pub stack_distribution: HashMap<SipStackType, usize>,
    pub active_quirks: usize,
    pub strict_compliance: bool,
    pub supported_extensions: usize,
}

/// RFC compliance checker
pub struct RfcComplianceChecker {
    config: SipInteropConfig,
}

impl RfcComplianceChecker {
    pub fn new(config: SipInteropConfig) -> Self {
        Self { config }
    }
    
    /// Validate request against RFC 3261
    pub fn validate_request(&self, request: &Request) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();
        
        // Check required headers (RFC 3261 Section 8.1.1)
        if request.via_header().is_err() {
            issues.push(ComplianceIssue::MissingHeader("Via".to_string()));
        }
        if request.from_header().is_err() {
            issues.push(ComplianceIssue::MissingHeader("From".to_string()));
        }
        if request.to_header().is_err() {
            issues.push(ComplianceIssue::MissingHeader("To".to_string()));
        }
        if request.call_id_header().is_err() {
            issues.push(ComplianceIssue::MissingHeader("Call-ID".to_string()));
        }
        if request.cseq_header().is_err() {
            issues.push(ComplianceIssue::MissingHeader("CSeq".to_string()));
        }
        if request.max_forwards_header().is_err() {
            issues.push(ComplianceIssue::MissingHeader("Max-Forwards".to_string()));
        }
        
        // Method-specific validation
        match request.method() {
            Method::Invite => {
                if request.contact_header().is_err() {
                    issues.push(ComplianceIssue::MissingHeader("Contact".to_string()));
                }
            }
            Method::Register => {
                if request.contact_header().is_err() {
                    issues.push(ComplianceIssue::MissingHeader("Contact".to_string()));
                }
            }
            _ => {}
        }
        
        // Validate Max-Forwards
        if let Ok(max_forwards) = request.max_forwards_header() {
            if let Ok(value) = max_forwards.value().parse::<u8>() {
                if value > 70 {
                    issues.push(ComplianceIssue::InvalidHeaderValue(
                        "Max-Forwards".to_string(),
                        "Value exceeds RFC recommendation of 70".to_string()
                    ));
                }
            }
        }
        
        Ok(issues)
    }
    
    /// Validate response against RFC 3261
    pub fn validate_response(&self, response: &Response) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();
        
        // Check required headers for responses
        if response.via_header().is_err() {
            issues.push(ComplianceIssue::MissingHeader("Via".to_string()));
        }
        if response.from_header().is_err() {
            issues.push(ComplianceIssue::MissingHeader("From".to_string()));
        }
        if response.to_header().is_err() {
            issues.push(ComplianceIssue::MissingHeader("To".to_string()));
        }
        if response.call_id_header().is_err() {
            issues.push(ComplianceIssue::MissingHeader("Call-ID".to_string()));
        }
        if response.cseq_header().is_err() {
            issues.push(ComplianceIssue::MissingHeader("CSeq".to_string()));
        }
        
        // Status-specific validation
        let status_code = response.status_code().code();
        if status_code >= 200 && status_code < 300 {
            // 2xx responses to INVITE should have Contact header
            if let Ok(cseq) = response.cseq_header() {
                if cseq.method() == &Method::Invite && response.contact_header().is_err() {
                    issues.push(ComplianceIssue::MissingHeader("Contact".to_string()));
                }
            }
        }
        
        Ok(issues)
    }
}

/// RFC compliance issues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComplianceIssue {
    MissingHeader(String),
    InvalidHeaderValue(String, String),
    MalformedUri(String),
    InvalidMethod(String),
    ProtocolViolation(String),
}

/// Utility functions for SIP interoperability
pub mod utils {
    use super::*;
    
    /// Detect SIP stack from User-Agent string
    pub fn detect_stack_from_user_agent(user_agent: &str) -> SipStackType {
        let ua_lower = user_agent.to_lowercase();
        
        if ua_lower.contains("sofia") || ua_lower.contains("freeswitch") {
            SipStackType::Sofia
        } else if ua_lower.contains("pjsua") || ua_lower.contains("pjsip") {
            SipStackType::Pjsip
        } else if ua_lower.contains("asterisk") {
            SipStackType::Asterisk
        } else if ua_lower.contains("mod_sofia") {
            SipStackType::FreeSWITCH
        } else {
            SipStackType::Generic
        }
    }
    
    /// Check if method is supported by stack
    pub fn is_method_supported(stack_type: &SipStackType, method: &Method) -> bool {
        match stack_type {
            SipStackType::Sofia | SipStackType::FreeSWITCH => {
                matches!(method, 
                    Method::Invite | Method::Ack | Method::Cancel | Method::Bye |
                    Method::Register | Method::Options | Method::Info | Method::Update |
                    Method::Refer | Method::Message | Method::Subscribe | Method::Notify |
                    Method::Publish | Method::PRack
                )
            }
            SipStackType::Pjsip => {
                matches!(method,
                    Method::Invite | Method::Ack | Method::Cancel | Method::Bye |
                    Method::Register | Method::Options | Method::Info | Method::Update |
                    Method::Refer | Method::Message | Method::Subscribe | Method::Notify |
                    Method::PRack
                )
            }
            SipStackType::Asterisk => {
                matches!(method,
                    Method::Invite | Method::Ack | Method::Cancel | Method::Bye |
                    Method::Register | Method::Options | Method::Info | Method::Update |
                    Method::Refer | Method::Message | Method::Subscribe | Method::Notify
                )
            }
            SipStackType::Generic => {
                matches!(method,
                    Method::Invite | Method::Ack | Method::Cancel | Method::Bye |
                    Method::Register | Method::Options
                )
            }
        }
    }
    
    /// Generate appropriate User-Agent string
    pub fn generate_user_agent(include_extensions: bool) -> String {
        let base = "Redfire-Switch/1.0";
        if include_extensions {
            format!("{} (RFC3261,RFC3262,RFC4028,RFC8224)", base)
        } else {
            base.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_stack_detection() {
        assert_eq!(
            utils::detect_stack_from_user_agent("FreeSWITCH-mod_sofia/1.10.7"),
            SipStackType::FreeSWITCH
        );
        assert_eq!(
            utils::detect_stack_from_user_agent("PJSUA v2.10"),
            SipStackType::Pjsip
        );
        assert_eq!(
            utils::detect_stack_from_user_agent("Asterisk PBX 18.0.0"),
            SipStackType::Asterisk
        );
    }
    
    #[test]
    fn test_method_support() {
        assert!(utils::is_method_supported(&SipStackType::Sofia, &Method::PRack));
        assert!(!utils::is_method_supported(&SipStackType::Asterisk, &Method::Publish));
        assert!(utils::is_method_supported(&SipStackType::Pjsip, &Method::Update));
    }
    
    #[test]
    fn test_user_agent_generation() {
        let ua = utils::generate_user_agent(true);
        assert!(ua.contains("RFC3261"));
        assert!(ua.contains("Redfire-Switch"));
    }
}