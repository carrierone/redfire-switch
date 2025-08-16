/*
 * Redfire SIP Stack Minimal - Lightweight SIP Implementation
 * Copyright (C) 2025 Carrier One Inc and contributors
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

//! # Redfire SIP Stack Minimal
//! 
//! A lightweight, production-ready SIP stack implementation focusing on:
//! - Basic SIP message parsing and generation
//! - Simple transaction handling
//! - Core RFC 3261 compliance
//! - Minimal dependencies for embedded use
//! 
//! ## Basic Usage
//! 
//! ```rust
//! use redfire_sip_stack_minimal::{SipMessage, SipParser, SipMethod};
//! 
//! let parser = SipParser::new();
//! let sip_data = "INVITE sip:alice@example.com SIP/2.0\r\n...";
//! let message = parser.parse(sip_data.as_bytes())?;
//! 
//! if let Some(SipMethod::Invite) = message.method {
//!     println!("Received INVITE request");
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

/// SIP Methods as defined in RFC 3261
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SipMethod {
    Invite,
    Ack,
    Bye,
    Cancel,
    Register,
    Options,
    Info,
    Prack,
    Subscribe,
    Notify,
    Update,
    Message,
    Refer,
}

impl FromStr for SipMethod {
    type Err = anyhow::Error;
    
    fn from_str(s: &str) -> Result<Self> {
        match s.to_uppercase().as_str() {
            "INVITE" => Ok(SipMethod::Invite),
            "ACK" => Ok(SipMethod::Ack),
            "BYE" => Ok(SipMethod::Bye),
            "CANCEL" => Ok(SipMethod::Cancel),
            "REGISTER" => Ok(SipMethod::Register),
            "OPTIONS" => Ok(SipMethod::Options),
            "INFO" => Ok(SipMethod::Info),
            "PRACK" => Ok(SipMethod::Prack),
            "SUBSCRIBE" => Ok(SipMethod::Subscribe),
            "NOTIFY" => Ok(SipMethod::Notify),
            "UPDATE" => Ok(SipMethod::Update),
            "MESSAGE" => Ok(SipMethod::Message),
            "REFER" => Ok(SipMethod::Refer),
            _ => Err(anyhow!("Unknown SIP method: {}", s)),
        }
    }
}

impl std::fmt::Display for SipMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let method_str = match self {
            SipMethod::Invite => "INVITE",
            SipMethod::Ack => "ACK",
            SipMethod::Bye => "BYE",
            SipMethod::Cancel => "CANCEL",
            SipMethod::Register => "REGISTER",
            SipMethod::Options => "OPTIONS",
            SipMethod::Info => "INFO",
            SipMethod::Prack => "PRACK",
            SipMethod::Subscribe => "SUBSCRIBE",
            SipMethod::Notify => "NOTIFY",
            SipMethod::Update => "UPDATE",
            SipMethod::Message => "MESSAGE",
            SipMethod::Refer => "REFER",
        };
        write!(f, "{}", method_str)
    }
}

/// SIP Status Codes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SipStatusCode {
    // 1xx Provisional
    Trying = 100,
    Ringing = 180,
    CallIsBeingForwarded = 181,
    Queued = 182,
    SessionProgress = 183,
    EarlyDialogTerminated = 199,
    
    // 2xx Success
    Ok = 200,
    Accepted = 202,
    NoNotification = 204,
    
    // 3xx Redirection
    MultipleChoices = 300,
    MovedPermanently = 301,
    MovedTemporarily = 302,
    UseProxy = 305,
    AlternativeService = 380,
    
    // 4xx Client Error
    BadRequest = 400,
    Unauthorized = 401,
    PaymentRequired = 402,
    Forbidden = 403,
    NotFound = 404,
    MethodNotAllowed = 405,
    NotAcceptable = 406,
    ProxyAuthenticationRequired = 407,
    RequestTimeout = 408,
    Gone = 410,
    ConditionalRequestFailed = 412,
    RequestEntityTooLarge = 413,
    RequestUriTooLong = 414,
    UnsupportedMediaType = 415,
    UnsupportedUriScheme = 416,
    UnknownResourcePriority = 417,
    BadExtension = 420,
    ExtensionRequired = 421,
    SessionIntervalTooSmall = 422,
    IntervalTooBrief = 423,
    BadLocationInformation = 424,
    UseIdentityHeader = 428,
    ProvideReferrerIdentity = 429,
    FlowFailed = 430,
    AnonymityDisallowed = 433,
    BadIdentityInfo = 436,
    UnsupportedCertificate = 437,
    InvalidIdentityHeader = 438,
    FirstHopLacksOutboundSupport = 439,
    MaxBreadthExceeded = 440,
    BadInfoPackage = 469,
    ConsentNeeded = 470,
    TemporarilyUnavailable = 480,
    CallTransactionDoesNotExist = 481,
    LoopDetected = 482,
    TooManyHops = 483,
    AddressIncomplete = 484,
    Ambiguous = 485,
    BusyHere = 486,
    RequestTerminated = 487,
    NotAcceptableHere = 488,
    BadEvent = 489,
    RequestPending = 491,
    Undecipherable = 493,
    SecurityAgreementRequired = 494,
    
    // 5xx Server Error
    ServerInternalError = 500,
    NotImplemented = 501,
    BadGateway = 502,
    ServiceUnavailable = 503,
    ServerTimeout = 504,
    VersionNotSupported = 505,
    MessageTooLarge = 513,
    PreconditionFailure = 580,
    
    // 6xx Global Failure
    BusyEverywhere = 600,
    Decline = 603,
    DoesNotExistAnywhere = 604,
    NotAcceptableAnywhere = 606,
}

impl SipStatusCode {
    pub fn as_u16(&self) -> u16 {
        *self as u16
    }
    
    pub fn reason_phrase(&self) -> &'static str {
        match self {
            SipStatusCode::Trying => "Trying",
            SipStatusCode::Ringing => "Ringing",
            SipStatusCode::SessionProgress => "Session Progress",
            SipStatusCode::Ok => "OK",
            SipStatusCode::BadRequest => "Bad Request",
            SipStatusCode::Unauthorized => "Unauthorized",
            SipStatusCode::Forbidden => "Forbidden",
            SipStatusCode::NotFound => "Not Found",
            SipStatusCode::MethodNotAllowed => "Method Not Allowed",
            SipStatusCode::ProxyAuthenticationRequired => "Proxy Authentication Required",
            SipStatusCode::RequestTimeout => "Request Timeout",
            SipStatusCode::TemporarilyUnavailable => "Temporarily Unavailable",
            SipStatusCode::BusyHere => "Busy Here",
            SipStatusCode::RequestTerminated => "Request Terminated",
            SipStatusCode::ServerInternalError => "Server Internal Error",
            SipStatusCode::NotImplemented => "Not Implemented",
            SipStatusCode::BadGateway => "Bad Gateway",
            SipStatusCode::ServiceUnavailable => "Service Unavailable",
            SipStatusCode::BusyEverywhere => "Busy Everywhere",
            SipStatusCode::Decline => "Decline",
            _ => "Unknown",
        }
    }
}

/// SIP Header representation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SipHeader {
    pub name: String,
    pub value: String,
}

/// Basic SIP Message structure
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SipMessage {
    /// Request method (None for responses)
    pub method: Option<SipMethod>,
    /// Request URI (for requests)
    pub request_uri: Option<String>,
    /// Status code (for responses)
    pub status_code: Option<SipStatusCode>,
    /// Reason phrase (for responses)
    pub reason_phrase: Option<String>,
    /// SIP version (usually "SIP/2.0")
    pub version: String,
    /// All headers
    pub headers: Vec<SipHeader>,
    /// Message body
    pub body: Option<String>,
    /// Source address (if known)
    pub source: Option<SocketAddr>,
}

impl SipMessage {
    /// Create a new request message
    pub fn new_request(method: SipMethod, request_uri: String) -> Self {
        Self {
            method: Some(method),
            request_uri: Some(request_uri),
            status_code: None,
            reason_phrase: None,
            version: "SIP/2.0".to_string(),
            headers: Vec::new(),
            body: None,
            source: None,
        }
    }
    
    /// Create a new response message
    pub fn new_response(status_code: SipStatusCode) -> Self {
        Self {
            method: None,
            request_uri: None,
            status_code: Some(status_code),
            reason_phrase: Some(status_code.reason_phrase().to_string()),
            version: "SIP/2.0".to_string(),
            headers: Vec::new(),
            body: None,
            source: None,
        }
    }
    
    /// Add a header to the message
    pub fn add_header(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.headers.push(SipHeader {
            name: name.into(),
            value: value.into(),
        });
    }
    
    /// Get the first header with the given name
    pub fn get_header(&self, name: &str) -> Option<&SipHeader> {
        self.headers.iter().find(|h| h.name.eq_ignore_ascii_case(name))
    }
    
    /// Get all headers with the given name
    pub fn get_headers(&self, name: &str) -> Vec<&SipHeader> {
        self.headers.iter().filter(|h| h.name.eq_ignore_ascii_case(name)).collect()
    }
    
    /// Set the message body
    pub fn set_body(&mut self, body: String) {
        // Update Content-Length header
        let content_length = body.len().to_string();
        
        // Remove existing Content-Length headers
        self.headers.retain(|h| !h.name.eq_ignore_ascii_case("Content-Length"));
        
        // Add new Content-Length header
        self.add_header("Content-Length", content_length);
        
        self.body = Some(body);
    }
    
    /// Convert the message to a string representation
    pub fn to_string(&self) -> String {
        let mut result = String::new();
        
        // Start line
        if let Some(method) = &self.method {
            // Request line
            result.push_str(&format!("{} {} {}\r\n", 
                method, 
                self.request_uri.as_ref().unwrap_or(&"".to_string()),
                self.version
            ));
        } else {
            // Status line
            result.push_str(&format!("{} {} {}\r\n",
                self.version,
                self.status_code.unwrap_or(SipStatusCode::ServerInternalError).as_u16(),
                self.reason_phrase.as_ref().unwrap_or(&"Unknown".to_string())
            ));
        }
        
        // Headers
        for header in &self.headers {
            result.push_str(&format!("{}: {}\r\n", header.name, header.value));
        }
        
        // Empty line before body
        result.push_str("\r\n");
        
        // Body
        if let Some(body) = &self.body {
            result.push_str(body);
        }
        
        result
    }
    
    /// Check if this is a request message
    pub fn is_request(&self) -> bool {
        self.method.is_some()
    }
    
    /// Check if this is a response message
    pub fn is_response(&self) -> bool {
        self.status_code.is_some()
    }
}

/// Simple SIP message parser
pub struct SipParser {
    // Future: Add parser configuration options
}

impl SipParser {
    /// Create a new SIP parser
    pub fn new() -> Self {
        Self {}
    }
    
    /// Parse a SIP message from raw bytes
    pub fn parse(&self, data: &[u8]) -> Result<SipMessage> {
        let text = std::str::from_utf8(data)
            .map_err(|_| anyhow!("Invalid UTF-8 in SIP message"))?;
        
        self.parse_str(text)
    }
    
    /// Parse a SIP message from a string
    pub fn parse_str(&self, text: &str) -> Result<SipMessage> {
        let lines: Vec<&str> = text.split("\r\n").collect();
        
        if lines.is_empty() {
            return Err(anyhow!("Empty SIP message"));
        }
        
        // Parse start line (request or status line)
        let start_line = lines[0];
        let mut message = self.parse_start_line(start_line)?;
        
        // Find the empty line separating headers from body
        let mut header_end = lines.len();
        for (i, line) in lines.iter().enumerate().skip(1) {
            if line.is_empty() {
                header_end = i;
                break;
            }
        }
        
        // Parse headers
        for line in &lines[1..header_end] {
            if let Some(header) = self.parse_header(line)? {
                message.headers.push(header);
            }
        }
        
        // Parse body if present
        if header_end + 1 < lines.len() {
            let body_lines: Vec<&str> = lines[(header_end + 1)..].iter().collect();
            if !body_lines.is_empty() {
                message.body = Some(body_lines.join("\r\n"));
            }
        }
        
        Ok(message)
    }
    
    fn parse_start_line(&self, line: &str) -> Result<SipMessage> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        
        if parts.len() < 3 {
            return Err(anyhow!("Invalid start line: {}", line));
        }
        
        if parts[0].starts_with("SIP/") {
            // Response line: SIP/2.0 200 OK
            let version = parts[0].to_string();
            let status_code: u16 = parts[1].parse()
                .map_err(|_| anyhow!("Invalid status code: {}", parts[1]))?;
            let reason_phrase = parts[2..].join(" ");
            
            // Convert status code to enum (simplified)
            let status_code = match status_code {
                100 => SipStatusCode::Trying,
                180 => SipStatusCode::Ringing,
                183 => SipStatusCode::SessionProgress,
                200 => SipStatusCode::Ok,
                400 => SipStatusCode::BadRequest,
                401 => SipStatusCode::Unauthorized,
                403 => SipStatusCode::Forbidden,
                404 => SipStatusCode::NotFound,
                480 => SipStatusCode::TemporarilyUnavailable,
                486 => SipStatusCode::BusyHere,
                487 => SipStatusCode::RequestTerminated,
                500 => SipStatusCode::ServerInternalError,
                _ => SipStatusCode::ServerInternalError, // Default fallback
            };
            
            Ok(SipMessage {
                method: None,
                request_uri: None,
                status_code: Some(status_code),
                reason_phrase: Some(reason_phrase),
                version,
                headers: Vec::new(),
                body: None,
                source: None,
            })
        } else {
            // Request line: INVITE sip:alice@example.com SIP/2.0
            let method = SipMethod::from_str(parts[0])?;
            let request_uri = parts[1].to_string();
            let version = parts[2].to_string();
            
            Ok(SipMessage {
                method: Some(method),
                request_uri: Some(request_uri),
                status_code: None,
                reason_phrase: None,
                version,
                headers: Vec::new(),
                body: None,
                source: None,
            })
        }
    }
    
    fn parse_header(&self, line: &str) -> Result<Option<SipHeader>> {
        if line.trim().is_empty() {
            return Ok(None);
        }
        
        if let Some(colon_pos) = line.find(':') {
            let name = line[..colon_pos].trim().to_string();
            let value = line[(colon_pos + 1)..].trim().to_string();
            
            Ok(Some(SipHeader { name, value }))
        } else {
            // Handle header continuation (starts with space or tab)
            if line.starts_with(' ') || line.starts_with('\t') {
                // This should be handled by combining with previous header
                // For simplicity, we'll ignore it for now
                Ok(None)
            } else {
                Err(anyhow!("Invalid header line: {}", line))
            }
        }
    }
}

impl Default for SipParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility functions for SIP operations
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
        if let Some(scheme_end) = uri.find(':') {
            let after_scheme = &uri[scheme_end + 1..];
            if let Some(at_pos) = after_scheme.find('@') {
                let domain_part = &after_scheme[at_pos + 1..];
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
}

/// Library version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");
pub const DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

/// Default SIP port
pub const DEFAULT_SIP_PORT: u16 = 5060;

/// Default SIP TLS port
pub const DEFAULT_SIPS_PORT: u16 = 5061;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sip_method_parsing() {
        assert_eq!(SipMethod::from_str("INVITE").unwrap(), SipMethod::Invite);
        assert_eq!(SipMethod::from_str("invite").unwrap(), SipMethod::Invite);
        assert_eq!(SipMethod::from_str("BYE").unwrap(), SipMethod::Bye);
        assert!(SipMethod::from_str("INVALID").is_err());
    }
    
    #[test]
    fn test_sip_message_creation() {
        let mut msg = SipMessage::new_request(SipMethod::Invite, "sip:alice@example.com".to_string());
        msg.add_header("From", "Bob <sip:bob@example.org>");
        msg.add_header("To", "Alice <sip:alice@example.com>");
        
        assert!(msg.is_request());
        assert!(!msg.is_response());
        assert_eq!(msg.method, Some(SipMethod::Invite));
        assert_eq!(msg.get_header("From").unwrap().value, "Bob <sip:bob@example.org>");
    }
    
    #[test]
    fn test_sip_response_creation() {
        let mut msg = SipMessage::new_response(SipStatusCode::Ok);
        msg.add_header("Content-Length", "0");
        
        assert!(!msg.is_request());
        assert!(msg.is_response());
        assert_eq!(msg.status_code, Some(SipStatusCode::Ok));
        assert_eq!(msg.reason_phrase, Some("OK".to_string()));
    }
    
    #[test]
    fn test_simple_parsing() {
        let parser = SipParser::new();
        let sip_text = "INVITE sip:alice@example.com SIP/2.0\r\nFrom: Bob\r\nTo: Alice\r\n\r\n";
        
        let msg = parser.parse_str(sip_text).unwrap();
        assert_eq!(msg.method, Some(SipMethod::Invite));
        assert_eq!(msg.request_uri, Some("sip:alice@example.com".to_string()));
        assert_eq!(msg.headers.len(), 2);
    }
    
    #[test]
    fn test_utils() {
        let call_id = utils::generate_call_id();
        assert!(call_id.starts_with("redfire-"));
        
        let branch = utils::generate_branch();
        assert!(branch.starts_with("z9hG4bK-redfire-"));
        
        assert!(utils::validate_sip_uri("sip:alice@example.com"));
        assert!(utils::validate_sip_uri("sips:bob@secure.example.com"));
        assert!(!utils::validate_sip_uri("http://example.com"));
        
        assert_eq!(utils::extract_domain("sip:alice@example.com"), Some("example.com".to_string()));
        assert_eq!(utils::extract_user("sip:alice@example.com"), Some("alice".to_string()));
        
        assert!(utils::method_requires_response(&SipMethod::Invite));
        assert!(!utils::method_requires_response(&SipMethod::Ack));
    }
}