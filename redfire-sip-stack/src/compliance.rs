/*
 * Redfire Switch - RFC Compliance and SIP Stack Compatibility Guide
 * Copyright (C) 2025 Carrier One Inc and contributors
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

//! # SIP RFC Compliance and Compatibility Documentation
//!
//! This module documents all RFC implementations and compatibility requirements
//! for interoperating with major SIP stacks in Class 4 switching environments.
//!
//! ## Core RFCs Implemented
//!
//! ### **RFC 3261 - SIP: Session Initiation Protocol**
//! - **Status**: REQUIRED - Fully Implemented
//! - **Purpose**: Core SIP 2.0 specification
//! - **Key Features**:
//!   - Request/Response message structure
//!   - Transaction state machines
//!   - Dialog state management
//!   - Routing and record routing
//!   - Authentication framework
//!   - Registration procedures
//! - **Interop Notes**:
//!   - SOFIA SIP: Strict header validation required
//!   - PJSIP: Flexible Contact header handling
//!   - Asterisk: Custom header support needed
//!   - FreeSWITCH: Symmetric RTP support required
//!
//! ### **RFC 3262 - Reliability of Provisional Responses in SIP (PRACK)**
//! - **Status**: RECOMMENDED - Implemented
//! - **Purpose**: Reliable delivery of 1xx responses
//! - **Interop Notes**:
//!   - SOFIA SIP: Full PRACK support
//!   - PJSIP: Configurable PRACK support
//!   - Asterisk: Limited PRACK support
//!   - FreeSWITCH: Full PRACK support via mod_sofia
//!
//! ### **RFC 3263 - SIP: Locating SIP Servers**
//! - **Status**: REQUIRED - Implemented
//! - **Purpose**: DNS SRV and NAPTR resolution for SIP
//! - **Key Features**:
//!   - SRV record lookups (_sip._udp, _sip._tcp, _sips._tcp)
//!   - NAPTR record processing
//!   - Transport protocol selection
//!   - Failover mechanisms
//!
//! ### **RFC 3264 - An Offer/Answer Model with SDP**
//! - **Status**: REQUIRED - Implemented
//! - **Purpose**: SDP negotiation framework
//! - **Interop Notes**:
//!   - SOFIA SIP: Strict SDP format requirements
//!   - PJSIP: Flexible SDP parsing
//!   - Asterisk: Custom SDP attributes support
//!   - FreeSWITCH: Advanced SDP manipulation
//!
//! ### **RFC 3265 - Session Initiation Protocol (SIP)-Specific Event Notification**
//! - **Status**: RECOMMENDED - Implemented
//! - **Purpose**: SUBSCRIBE/NOTIFY framework
//! - **Key Features**:
//!   - Event package framework
//!   - Subscription state management
//!   - Event notification delivery
//!   - Subscription expiration handling
//!
//! ### **RFC 3311 - The Session Initiation Protocol (SIP) UPDATE Method**
//! - **Status**: RECOMMENDED - Implemented
//! - **Purpose**: Mid-dialog parameter updates
//! - **Interop Notes**:
//!   - SOFIA SIP: Full UPDATE support
//!   - PJSIP: Configurable UPDATE support
//!   - Asterisk: Basic UPDATE support
//!   - FreeSWITCH: Full UPDATE support
//!
//! ### **RFC 3326 - The Reason Header Field for SIP**
//! - **Status**: RECOMMENDED - Implemented
//! - **Purpose**: Call termination reason indication
//! - **Key Features**:
//!   - Q.850 cause codes
//!   - SIP response codes
//!   - Protocol-specific reasons
//!   - Multiple reason values
//!
//! ### **RFC 3428 - Session Initiation Protocol (SIP) Extension for Instant Messaging**
//! - **Status**: OPTIONAL - Implemented
//! - **Purpose**: SIP MESSAGE method for instant messaging
//! - **Interop Notes**:
//!   - All major stacks support MESSAGE method
//!   - Content-Type handling varies
//!
//! ### **RFC 3515 - The Session Initiation Protocol (SIP) REFER Method**
//! - **Status**: RECOMMENDED - Implemented
//! - **Purpose**: Call transfer and redirection
//! - **Key Features**:
//!   - Refer-To header
//!   - Referred-By header
//!   - Transfer progress notifications
//!   - Replaces header integration
//!
//! ### **RFC 3581 - An Extension to SIP for Symmetric Response Routing**
//! - **Status**: RECOMMENDED - Implemented
//! - **Purpose**: NAT traversal support
//! - **Key Features**:
//!   - "rport" parameter in Via header
//!   - Symmetric response routing
//!   - NAT detection and handling
//!
//! ### **RFC 3841 - Caller Preferences for SIP**
//! - **Status**: OPTIONAL - Implemented
//! - **Purpose**: Feature and capability negotiation
//! - **Key Features**:
//!   - Accept-Contact header
//!   - Reject-Contact header
//!   - Request-Disposition header
//!   - Feature parameter matching
//!
//! ### **RFC 3891 - The Session Initiation Protocol (SIP) "Replaces" Header**
//! - **Status**: RECOMMENDED - Implemented
//! - **Purpose**: Call replacement for transfers
//! - **Interop Notes**:
//!   - Essential for attended transfers
//!   - All major stacks support Replaces
//!
//! ### **RFC 3903 - Session Initiation Protocol (SIP) Extension for Event State Publication**
//! - **Status**: OPTIONAL - Implemented
//! - **Purpose**: PUBLISH method for event state
//! - **Interop Notes**:
//!   - SOFIA SIP: Full PUBLISH support
//!   - PJSIP: Configurable PUBLISH support
//!   - Asterisk: Limited PUBLISH support
//!   - FreeSWITCH: Full PUBLISH support
//!
//! ### **RFC 4028 - Session Timers in SIP**
//! - **Status**: RECOMMENDED - Implemented
//! - **Purpose**: Session refresh and timeout handling
//! - **Key Features**:
//!   - Session-Expires header
//!   - Min-SE header
//!   - Refresher parameter
//!   - Timer-based session management
//! - **Interop Notes**:
//!   - Asterisk: Custom session timer implementation
//!   - Others: Standard RFC 4028 implementation
//!
//! ### **RFC 4235 - An INVITE-Initiated Dialog Event Package for SIP**
//! - **Status**: OPTIONAL - Implemented
//! - **Purpose**: Dialog state monitoring
//! - **Key Features**:
//!   - Dialog information XML format
//!   - Dialog state notifications
//!   - Multiple dialog tracking
//!
//! ### **RFC 4916 - Connected Identity in SIP**
//! - **Status**: RECOMMENDED - Implemented
//! - **Purpose**: Connected party identification
//! - **Key Features**:
//!   - P-Asserted-Identity header
//!   - P-Preferred-Identity header
//!   - Remote-Party-ID header (legacy)
//!   - Privacy header integration
//!
//! ### **RFC 6026 - Correct Transaction Handling for 2xx Responses to SIP INVITE**
//! - **Status**: REQUIRED - Implemented
//! - **Purpose**: Proper INVITE transaction handling
//! - **Key Features**:
//!   - Multiple 2xx response handling
//!   - Transaction termination rules
//!   - Forking proxy behavior
//!
//! ### **RFC 6141 - Re-INVITE and Target-Refresh Request Handling in SIP**
//! - **Status**: RECOMMENDED - Implemented
//! - **Purpose**: Mid-dialog request handling
//! - **Key Features**:
//!   - Target-refresh request rules
//!   - Route set updates
//!   - Contact header updates
//!
//! ### **RFC 8224 - Authenticated Identity Management in SIP (STIR)**
//! - **Status**: RECOMMENDED - Implemented
//! - **Purpose**: Call authentication framework
//! - **Key Features**:
//!   - Identity header
//!   - PASSporT token format
//!   - Certificate-based authentication
//!   - Verification procedures
//!
//! ### **RFC 8225 - PASSporT: Personal Assertion Token (SHAKEN)**
//! - **Status**: RECOMMENDED - Implemented
//! - **Purpose**: Secure caller ID framework
//! - **Key Features**:
//!   - JWT-based assertions
//!   - Attestation levels (A, B, C)
//!   - Origination and verification
//!   - Anti-spoofing measures
//!
//! ## SIP Stack Specific Compatibility Requirements
//!
//! ### **SOFIA SIP (Nokia/FreeSWITCH) Compatibility**
//! ```text
//! User-Agent Detection: "sofia", "FreeSWITCH"
//! Key Requirements:
//! - Strict RFC 3261 compliance
//! - Contact header required in REGISTER
//! - Proper Route header handling
//! - IPv4 preference for media
//! - Session timer support (RFC 4028)
//! - PRACK support (RFC 3262)
//!
//! Configuration:
//! [sofia_compatibility]
//! strict_rfc_compliance = true
//! require_contact_in_register = true
//! prefer_ipv4 = true
//! session_timers = true
//! prack_support = true
//! ```
//!
//! ### **PJSIP Compatibility**
//! ```text
//! User-Agent Detection: "PJSUA", "pjsip"
//! Key Requirements:
//! - Flexible header parsing
//! - Compact header support
//! - Multiple transport support
//! - UPDATE method support
//! - Configurable PRACK
//!
//! Configuration:
//! [pjsip_compatibility]
//! flexible_parsing = true
//! compact_headers = true
//! update_support = true
//! configurable_prack = true
//! multiple_transports = true
//! ```
//!
//! ### **Asterisk Compatibility**
//! ```text
//! User-Agent Detection: "Asterisk"
//! Key Requirements:
//! - Custom session timer handling
//! - Flexible SDP parsing
//! - Custom authentication quirks
//! - Limited PRACK support
//! - X-Asterisk headers support
//!
//! Configuration:
//! [asterisk_compatibility]
//! custom_session_timers = true
//! flexible_sdp = true
//! custom_auth = true
//! limited_prack = true
//! asterisk_headers = true
//! ```
//!
//! ### **FreeSWITCH Compatibility**
//! ```text
//! User-Agent Detection: "FreeSWITCH", "mod_sofia"
//! Key Requirements:
//! - Variable header support
//! - Advanced routing features
//! - Full RFC compliance
//! - Media optimization
//! - Event framework support
//!
//! Configuration:
//! [freeswitch_compatibility]
//! variable_headers = true
//! advanced_routing = true
//! full_rfc_compliance = true
//! media_optimization = true
//! event_framework = true
//! ```
//!
//! ## Class 4 Switch Specific Requirements
//!
//! ### **Carrier-Grade Features**
//! - High-volume call processing (10,000+ CPS)
//! - Sub-millisecond routing decisions
//! - Advanced billing and rating
//! - Multiple codec support
//! - Transcoding capabilities
//! - Media anchoring/bypass
//!
//! ### **Interconnect Requirements**
//! - SIP-I (ISUP over SIP) support
//! - SIP-T (ISUP tunneling) support
//! - Q.850 cause code mapping
//! - Billing record generation
//! - LCR (Least Cost Routing)
//! - Quality monitoring
//!
//! ### **Regulatory Compliance**
//! - STIR/SHAKEN implementation
//! - CALEA compliance preparation
//! - Emergency services routing
//! - Number portability support
//! - Fraud detection and prevention

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

/// RFC compliance levels
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComplianceLevel {
    /// Must implement for basic SIP functionality
    Required,
    /// Should implement for full functionality
    Recommended,
    /// May implement for enhanced features
    Optional,
    /// Implemented for Class 4 specific needs
    Class4Specific,
}

/// RFC implementation status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RfcImplementation {
    /// RFC number
    pub rfc_number: u16,
    /// RFC title
    pub title: String,
    /// Implementation status
    pub status: ImplementationStatus,
    /// Compliance level
    pub compliance_level: ComplianceLevel,
    /// Key features implemented
    pub features: Vec<String>,
    /// Interop notes for different stacks
    pub interop_notes: HashMap<String, String>,
    /// Configuration requirements
    pub config_requirements: Vec<String>,
}

/// Implementation status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImplementationStatus {
    /// Fully implemented and tested
    Complete,
    /// Partially implemented
    Partial,
    /// Planned for implementation
    Planned,
    /// Not implemented
    NotImplemented,
}

/// SIP compliance checker
pub struct SipComplianceChecker {
    /// RFC implementations
    rfc_implementations: HashMap<u16, RfcImplementation>,
}

impl Default for SipComplianceChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl SipComplianceChecker {
    /// Create new compliance checker
    pub fn new() -> Self {
        let mut checker = Self {
            rfc_implementations: HashMap::new(),
        };

        checker.initialize_rfc_database();
        checker
    }

    /// Initialize RFC implementation database
    fn initialize_rfc_database(&mut self) {
        // Core SIP RFCs
        self.add_rfc(RfcImplementation {
            rfc_number: 3261,
            title: "SIP: Session Initiation Protocol".to_string(),
            status: ImplementationStatus::Complete,
            compliance_level: ComplianceLevel::Required,
            features: vec![
                "Request/Response structure".to_string(),
                "Transaction state machines".to_string(),
                "Dialog management".to_string(),
                "Registration procedures".to_string(),
                "Authentication framework".to_string(),
            ],
            interop_notes: [
                (
                    "SOFIA".to_string(),
                    "Requires strict header validation".to_string(),
                ),
                (
                    "PJSIP".to_string(),
                    "Flexible Contact header handling".to_string(),
                ),
                (
                    "Asterisk".to_string(),
                    "Custom header support needed".to_string(),
                ),
                (
                    "FreeSWITCH".to_string(),
                    "Symmetric RTP support required".to_string(),
                ),
            ]
            .into_iter()
            .collect(),
            config_requirements: vec![
                "strict_rfc_compliance = true".to_string(),
                "validate_headers = true".to_string(),
            ],
        });

        self.add_rfc(RfcImplementation {
            rfc_number: 3262,
            title: "Reliability of Provisional Responses in SIP (PRACK)".to_string(),
            status: ImplementationStatus::Complete,
            compliance_level: ComplianceLevel::Recommended,
            features: vec![
                "Reliable 1xx responses".to_string(),
                "PRACK method".to_string(),
                "RSeq header".to_string(),
                "RAck header".to_string(),
            ],
            interop_notes: [
                ("SOFIA".to_string(), "Full PRACK support".to_string()),
                (
                    "PJSIP".to_string(),
                    "Configurable PRACK support".to_string(),
                ),
                ("Asterisk".to_string(), "Limited PRACK support".to_string()),
                (
                    "FreeSWITCH".to_string(),
                    "Full PRACK via mod_sofia".to_string(),
                ),
            ]
            .into_iter()
            .collect(),
            config_requirements: vec![
                "prack_enabled = true".to_string(),
                "require_100rel = false".to_string(),
            ],
        });

        self.add_rfc(RfcImplementation {
            rfc_number: 3263,
            title: "SIP: Locating SIP Servers".to_string(),
            status: ImplementationStatus::Complete,
            compliance_level: ComplianceLevel::Required,
            features: vec![
                "DNS SRV lookups".to_string(),
                "NAPTR processing".to_string(),
                "Transport selection".to_string(),
                "Failover mechanisms".to_string(),
            ],
            interop_notes: [(
                "All".to_string(),
                "Essential for proper routing".to_string(),
            )]
            .into_iter()
            .collect(),
            config_requirements: vec![
                "dns_srv_enabled = true".to_string(),
                "naptr_enabled = true".to_string(),
            ],
        });

        self.add_rfc(RfcImplementation {
            rfc_number: 4028,
            title: "Session Timers in SIP".to_string(),
            status: ImplementationStatus::Complete,
            compliance_level: ComplianceLevel::Recommended,
            features: vec![
                "Session-Expires header".to_string(),
                "Min-SE header".to_string(),
                "Refresher parameter".to_string(),
                "Timer-based refresh".to_string(),
            ],
            interop_notes: [
                (
                    "Asterisk".to_string(),
                    "Custom timer implementation".to_string(),
                ),
                (
                    "Others".to_string(),
                    "Standard RFC 4028 implementation".to_string(),
                ),
            ]
            .into_iter()
            .collect(),
            config_requirements: vec![
                "session_timers = true".to_string(),
                "session_expires = 1800".to_string(),
                "min_se = 90".to_string(),
            ],
        });

        self.add_rfc(RfcImplementation {
            rfc_number: 8224,
            title: "Authenticated Identity Management in SIP (STIR)".to_string(),
            status: ImplementationStatus::Complete,
            compliance_level: ComplianceLevel::Recommended,
            features: vec![
                "Identity header".to_string(),
                "PASSporT tokens".to_string(),
                "Certificate verification".to_string(),
                "Authentication procedures".to_string(),
            ],
            interop_notes: [(
                "All".to_string(),
                "Critical for carrier interconnect".to_string(),
            )]
            .into_iter()
            .collect(),
            config_requirements: vec![
                "stir_shaken_enabled = true".to_string(),
                "certificate_path = /etc/certs/".to_string(),
            ],
        });

        self.add_rfc(RfcImplementation {
            rfc_number: 8225,
            title: "PASSporT: Personal Assertion Token (SHAKEN)".to_string(),
            status: ImplementationStatus::Complete,
            compliance_level: ComplianceLevel::Recommended,
            features: vec![
                "JWT-based assertions".to_string(),
                "Attestation levels".to_string(),
                "Signature verification".to_string(),
                "Anti-spoofing measures".to_string(),
            ],
            interop_notes: [("All".to_string(), "Required for US carriers".to_string())]
                .into_iter()
                .collect(),
            config_requirements: vec![
                "shaken_enabled = true".to_string(),
                "attestation_level = A".to_string(),
            ],
        });

        // Add more RFCs...
        info!(
            "Initialized RFC compliance database with {} RFCs",
            self.rfc_implementations.len()
        );
    }

    /// Add RFC implementation
    fn add_rfc(&mut self, rfc: RfcImplementation) {
        self.rfc_implementations.insert(rfc.rfc_number, rfc);
    }

    /// Get RFC implementation details
    pub fn get_rfc(&self, rfc_number: u16) -> Option<&RfcImplementation> {
        self.rfc_implementations.get(&rfc_number)
    }

    /// Get all implemented RFCs
    pub fn get_all_rfcs(&self) -> Vec<&RfcImplementation> {
        self.rfc_implementations.values().collect()
    }

    /// Get RFCs by compliance level
    pub fn get_rfcs_by_level(&self, level: ComplianceLevel) -> Vec<&RfcImplementation> {
        self.rfc_implementations
            .values()
            .filter(|rfc| rfc.compliance_level == level)
            .collect()
    }

    /// Check overall compliance status
    pub fn check_compliance(&self) -> ComplianceReport {
        let mut report = ComplianceReport {
            total_rfcs: self.rfc_implementations.len(),
            required_complete: 0,
            required_total: 0,
            recommended_complete: 0,
            recommended_total: 0,
            optional_complete: 0,
            optional_total: 0,
            incomplete_rfcs: Vec::new(),
        };

        for rfc in self.rfc_implementations.values() {
            match rfc.compliance_level {
                ComplianceLevel::Required => {
                    report.required_total += 1;
                    if rfc.status == ImplementationStatus::Complete {
                        report.required_complete += 1;
                    } else {
                        report.incomplete_rfcs.push(rfc.rfc_number);
                    }
                }
                ComplianceLevel::Recommended => {
                    report.recommended_total += 1;
                    if rfc.status == ImplementationStatus::Complete {
                        report.recommended_complete += 1;
                    }
                }
                ComplianceLevel::Optional | ComplianceLevel::Class4Specific => {
                    report.optional_total += 1;
                    if rfc.status == ImplementationStatus::Complete {
                        report.optional_complete += 1;
                    }
                }
            }
        }

        report
    }

    /// Generate interoperability configuration
    pub fn generate_interop_config(&self, stack_type: &str) -> String {
        let mut config = String::new();
        config.push_str(&format!(
            "# SIP Interoperability Configuration for {stack_type}\n"
        ));
        config.push_str("# Generated by Redfire Switch RFC Compliance Checker\n\n");

        for rfc in self.rfc_implementations.values() {
            if rfc.status == ImplementationStatus::Complete {
                config.push_str(&format!("# RFC {} - {}\n", rfc.rfc_number, rfc.title));

                if let Some(note) = rfc.interop_notes.get(stack_type) {
                    config.push_str(&format!("# Interop Note: {note}\n"));
                }

                for requirement in &rfc.config_requirements {
                    config.push_str(&format!("{requirement}\n"));
                }

                config.push('\n');
            }
        }

        config
    }
}

/// Compliance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub total_rfcs: usize,
    pub required_complete: usize,
    pub required_total: usize,
    pub recommended_complete: usize,
    pub recommended_total: usize,
    pub optional_complete: usize,
    pub optional_total: usize,
    pub incomplete_rfcs: Vec<u16>,
}

impl ComplianceReport {
    /// Calculate compliance percentage
    pub fn compliance_percentage(&self) -> f64 {
        if self.required_total == 0 {
            100.0
        } else {
            (self.required_complete as f64 / self.required_total as f64) * 100.0
        }
    }

    /// Check if fully compliant
    pub fn is_fully_compliant(&self) -> bool {
        self.required_complete == self.required_total
    }
}

/// Configuration generator for different SIP stacks
pub struct InteropConfigGenerator;

impl InteropConfigGenerator {
    /// Generate SOFIA SIP configuration
    pub fn generate_sofia_config() -> String {
        r#"
# SOFIA SIP (FreeSWITCH) Interoperability Configuration
# RFC 3261 Core SIP
strict_rfc_compliance = true
validate_all_headers = true
require_contact_in_register = true

# RFC 3262 PRACK Support
prack_enabled = true
require_100rel = false
supported_100rel = true

# RFC 4028 Session Timers
session_timers_enabled = true
session_expires = 1800
min_se = 90
refresher = "uas"

# Transport Preferences
preferred_transport = ["UDP", "TCP", "TLS"]
symmetric_rtp = true
rport_enabled = true

# Media Handling
codec_negotiation_strict = true
sdp_validation_strict = true
prefer_ipv4 = true

# SOFIA-Specific Features
route_header_strict = true
record_route_enabled = true
contact_header_required = true
"#
        .to_string()
    }

    /// Generate PJSIP configuration
    pub fn generate_pjsip_config() -> String {
        r#"
# PJSIP Interoperability Configuration
# RFC 3261 Core SIP
strict_rfc_compliance = true
flexible_header_parsing = true
compact_headers_enabled = true

# RFC 3262 PRACK Support
prack_enabled = true
prack_configurable = true
require_100rel = false

# RFC 3311 UPDATE Support
update_method_enabled = true
update_configurable = true

# Transport Support
multiple_transports = true
transport_auto_selection = true
tcp_keepalive = true

# Media Handling
codec_negotiation_flexible = true
sdp_parsing_permissive = true
multiple_media_lines = true

# PJSIP-Specific Features
contact_header_flexible = true
via_parsing_permissive = true
header_validation_relaxed = true
"#
        .to_string()
    }

    /// Generate Asterisk configuration
    pub fn generate_asterisk_config() -> String {
        r#"
# Asterisk Interoperability Configuration
# RFC 3261 Core SIP
strict_rfc_compliance = true
custom_header_support = true
asterisk_extensions = true

# Session Timers (Asterisk-specific)
session_timers_enabled = true
session_timers_mode = "asterisk"
session_expires = 1800
min_se = 90

# Authentication Quirks
auth_handling_asterisk = true
realm_case_insensitive = true
auth_username_flexible = true

# Custom Headers
asterisk_hangup_cause = true
asterisk_variables = true
x_asterisk_headers = true

# Media Handling
sdp_parsing_flexible = true
codec_order_flexible = true
bandwidth_modifier_support = false

# Asterisk-Specific Features
chan_sip_compatibility = true
chan_pjsip_compatibility = true
compact_headers_support = true
"#
        .to_string()
    }

    /// Generate FreeSWITCH configuration
    pub fn generate_freeswitch_config() -> String {
        r#"
# FreeSWITCH Interoperability Configuration
# RFC 3261 Core SIP
strict_rfc_compliance = true
full_rfc_support = true
advanced_routing = true

# RFC 3262 PRACK Support
prack_enabled = true
prack_full_support = true
reliable_provisionals = true

# RFC 3903 PUBLISH Support
publish_enabled = true
event_framework = true
subscription_support = true

# Variable Support
freeswitch_variables = true
custom_headers = true
x_fs_headers = true

# Media Optimization
media_optimization = true
codec_negotiation_advanced = true
bandwidth_management = true

# Advanced Features
event_socket_support = true
lua_scripting = true
database_integration = true
recording_support = true

# FreeSWITCH-Specific
mod_sofia_compatibility = true
profile_based_config = true
gateway_support = true
"#
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compliance_checker() {
        let checker = SipComplianceChecker::new();
        let report = checker.check_compliance();

        assert!(report.total_rfcs > 0);
        assert!(report.compliance_percentage() >= 0.0);
        assert!(report.compliance_percentage() <= 100.0);
    }

    #[test]
    fn test_rfc_retrieval() {
        let checker = SipComplianceChecker::new();
        let rfc3261 = checker.get_rfc(3261);

        assert!(rfc3261.is_some());
        assert_eq!(rfc3261.unwrap().compliance_level, ComplianceLevel::Required);
    }

    #[test]
    fn test_config_generation() {
        let config = InteropConfigGenerator::generate_sofia_config();
        assert!(config.contains("strict_rfc_compliance"));
        assert!(config.contains("SOFIA"));
    }
}
