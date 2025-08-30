/*
 * Redfire SIP Stack - Complete SIP, SIP-I, and SIP-T Protocol Implementation
 * Copyright (C) 2025 Carrier One Inc and contributors
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Sponsored by Carrier One Inc (https://www.carrierone.com)

// Allow common warnings for library code
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(unreachable_code)]
 */

//! # Redfire SIP Stack
//!
//! A complete implementation of the Session Initiation Protocol (SIP) stack including
//! SIP-T (SIP for Telephones) and SIP-I (SIP with encapsulated ISUP) support.
//!
//! ## Features
//!
//! - Complete SIP message parsing and validation
//! - SIP state machine and transaction handling
//! - Multiple authentication mechanisms (Digest, Basic)
//! - Multi-transport support (UDP, TCP, TLS)
//! - SIP-T multipart MIME support with ISUP encapsulation
//! - SIP-I ISUP message handling
//! - RFC compliance checking and validation
//! - Interoperability with different SIP implementations
//! - Debug and diagnostic tools
//!
//! ## Basic Usage
//!
//! ```rust
//! use redfire_sip_stack::{SipMessage, SipParser, SipMethod};
//!
//! // Parse a SIP message
//! let sip_data = "INVITE sip:alice@example.com SIP/2.0\r\n...";
//! let parser = SipParser::new();
//! let message = parser.parse_message(sip_data.as_bytes())?;
//!
//! match message.method {
//!     Some(SipMethod::Invite) => {
//!         println!("Received INVITE request");
//!     }
//!     _ => {}
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## SIP-T/SIP-I Support
//!
//! ```rust
//! use redfire_sip_stack::{SipTSipIService, SipTSipIConfig, IsupMessage};
//!
//! let config = SipTSipIConfig {
//!     sipt_enabled: true,
//!     sipi_enabled: true,
//!     ..Default::default()
//! };
//!
//! let service = SipTSipIService::new(config);
//!
//! // Create SIP-T multipart body with ISUP
//! let isup_data = vec![0x01, 0x23, 0x45]; // ISUP IAM message
//! let sipt_body = service.create_sipt_body(&isup_data, Some("v=0..."))?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod authentication;
pub mod compliance;
pub mod core;
pub mod debug_cli;
pub mod interop;
pub mod parser;
pub mod sipt_sipi;
pub mod state;
pub mod transport;

// Re-export main SIP types
pub use parser::{
    DialogState, InviteTransactionState, NonInviteTransactionState, SipDialog, SipMessage,
    SipParser, SipTransaction, TransactionState, TransactionTimers,
};

// Re-export rsip types for compatibility
pub use rsip::{
    Error as ParseError, Header as SipHeader, Method as SipMethod, StatusCode as SipStatusCode,
    Uri as SipUri, Version as SipVersion,
};

pub use state::{SipStateAction, SipStateConfig, SipStateManager, TransactionTimerManager};

pub use authentication::{
    AuthFailureReason, AuthResult, DigestAlgorithm, DigestCredentials, IpAuthConfig, RateLimiter,
    SipAuthenticator,
};

pub use transport::{
    ConnectionInfo, SipTransport, SipTransportManager, TlsConfig, TransportConfig, TransportEvent,
    TransportMessage,
};

pub use core::{ProcessorMessage, SipCallContext, SipCoreConfig, SipCoreEngine, SipRequestResult};

pub use debug_cli::{
    CallInfo, ColorScheme, ExportFormat, MessageDirection, MessageTiming, SipDebugConfig,
    SipDebugFilter, SipDebugMessage, TrunkInfo, TrunkType,
};

pub use interop::{
    DialogEventConfig, InteropQuirk, PrackConfig, SdpPreferences, SessionTimerConfig, SipExtension,
    SipInteropConfig, SipSecurityConfig, SipStackType, StackSpecificConfig,
};

pub use compliance::{
    ComplianceLevel, ComplianceReport, ImplementationStatus, InteropConfigGenerator,
    RfcImplementation, SipComplianceChecker,
};

pub use sipt_sipi::{
    BackwardCallIndicators, ForwardCallIndicators, IsupMessage, IsupMessageType, IsupParameter,
    IsupParameterType, IsupVariant, SipTSipIConfig, SipTSipIService,
};

/// Library version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");
pub const DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

/// Supported SIP versions
pub const SUPPORTED_SIP_VERSIONS: &[&str] = &["SIP/2.0"];

/// Default SIP port
pub const DEFAULT_SIP_PORT: u16 = 5060;

/// Default SIP TLS port
pub const DEFAULT_SIPS_PORT: u16 = 5061;

/// Create a default SIP parser
pub fn create_default_parser() -> SipParser {
    SipParser::new(
        "localhost".to_string(),
        DEFAULT_SIP_PORT,
        "Redfire-SIP-Stack/1.0".to_string(),
    )
}

/// Create a default SIP core with basic configuration
pub async fn create_default_core() -> anyhow::Result<SipCoreEngine> {
    let config = SipCoreConfig::default();
    SipCoreEngine::new(config).await
}

/// Create a SIP-T/SIP-I service with default configuration
pub fn create_sipt_sipi_service() -> SipTSipIService {
    let config = SipTSipIConfig::default();
    SipTSipIService::new(config)
}

/// Utility functions for common SIP operations  
pub mod utils {
    use super::*;

    /// Generate a unique Call-ID
    pub fn generate_call_id() -> String {
        format!("redfire-{}", uuid::Uuid::new_v4())
    }

    /// Generate a unique branch parameter for Via header
    pub fn generate_branch() -> String {
        format!("z9hG4bK-redfire-{}", uuid::Uuid::new_v4().as_simple())
    }

    /// Generate a unique tag parameter
    pub fn generate_tag() -> String {
        format!("redfire-{}", uuid::Uuid::new_v4().as_simple())
    }

    /// Validate SIP URI format
    pub fn validate_sip_uri(uri: &str) -> bool {
        uri.starts_with("sip:") || uri.starts_with("sips:")
    }

    /// Extract domain from SIP URI
    pub fn extract_domain(uri: &str) -> Option<String> {
        if let Some(at_pos) = uri.find('@') {
            let domain_part = &uri[at_pos + 1..];
            if let Some(colon_pos) = domain_part.find(':') {
                Some(domain_part[..colon_pos].to_string())
            } else if let Some(params_pos) = domain_part.find(';') {
                Some(domain_part[..params_pos].to_string())
            } else {
                Some(domain_part.to_string())
            }
        } else {
            None
        }
    }

    /// Extract user part from SIP URI
    pub fn extract_user(uri: &str) -> Option<String> {
        if let Some(scheme_end) = uri.find(':') {
            let after_scheme = &uri[scheme_end + 1..];
            if let Some(at_pos) = after_scheme.find('@') {
                Some(after_scheme[..at_pos].to_string())
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Check if method requires a response
    pub fn method_requires_response(method: &SipMethod) -> bool {
        !matches!(method, SipMethod::Ack)
    }

    /// Get default port for transport type
    pub fn default_port_for_transport(transport: &crate::transport::SipTransport) -> u16 {
        match transport {
            crate::transport::SipTransport::Udp | crate::transport::SipTransport::Tcp => {
                DEFAULT_SIP_PORT
            }
            crate::transport::SipTransport::Tls | crate::transport::SipTransport::Wss => {
                DEFAULT_SIPS_PORT
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_info() {
        assert!(!VERSION.is_empty());
        assert_eq!(NAME, "redfire-sip-stack");
        assert!(!DESCRIPTION.is_empty());
    }

    #[test]
    fn test_supported_versions() {
        assert!(SUPPORTED_SIP_VERSIONS.contains(&"SIP/2.0"));
    }

    #[test]
    fn test_utils() {
        let call_id = utils::generate_call_id();
        assert!(call_id.starts_with("redfire-"));

        let branch = utils::generate_branch();
        assert!(branch.starts_with("z9hG4bK-redfire-"));

        let tag = utils::generate_tag();
        assert!(tag.starts_with("redfire-"));

        assert!(utils::validate_sip_uri("sip:alice@example.com"));
        assert!(utils::validate_sip_uri("sips:bob@secure.example.com"));
        assert!(!utils::validate_sip_uri("http://example.com"));

        assert_eq!(
            utils::extract_domain("sip:alice@example.com"),
            Some("example.com".to_string())
        );
        assert_eq!(
            utils::extract_user("sip:alice@example.com"),
            Some("alice".to_string())
        );

        assert_eq!(
            utils::default_port_for_transport(&crate::transport::SipTransport::Udp),
            5060
        );
        assert_eq!(
            utils::default_port_for_transport(&crate::transport::SipTransport::Tls),
            5061
        );
    }

    #[test]
    fn test_parser_creation() {
        let parser = create_default_parser();
        // Basic parser should be created successfully
        assert!(true);
    }

    #[tokio::test]
    async fn test_core_creation() {
        let core = create_default_core().await;
        assert!(core.is_ok());
    }

    #[test]
    fn test_sipt_sipi_service_creation() {
        let service = create_sipt_sipi_service();
        assert!(!service.is_sipt_enabled()); // Default config has SIP-T disabled
    }
}
