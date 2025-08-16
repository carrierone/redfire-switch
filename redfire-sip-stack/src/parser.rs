/*
 * Redfire Switch - RFC 3261 Compliant SIP Message Parser
 * Copyright (C) 2025 Carrier One Inc and contributors
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use rsip::{
    message::SipMessage as RsipMessage, Request, Response, 
    headers::{Header, Via, Contact, From, To, CallId, CSeq, ContentType, ContentLength},
    method::Method,
    version::Version,
    uri::Uri,
    param::Param,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::str::FromStr;
use tracing::{debug, info, warn, error};
use uuid::Uuid;

/// SIP message wrapper with additional metadata
#[derive(Debug, Clone)]
pub struct SipMessage {
    /// Parsed SIP message
    pub message: RsipMessage,
    /// Source address
    pub source: SocketAddr,
    /// Destination address  
    pub destination: SocketAddr,
    /// Transport protocol used
    pub transport: SipTransport,
    /// Message reception timestamp
    pub received_at: chrono::DateTime<chrono::Utc>,
    /// Unique message ID for tracking
    pub message_id: String,
}

/// SIP transport protocols
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SipTransport {
    UDP,
    TCP,
    TLS,
    DTLS,
    WS,  // WebSocket
    WSS, // WebSocket Secure
}

impl std::fmt::Display for SipTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SipTransport::UDP => write!(f, "UDP"),
            SipTransport::TCP => write!(f, "TCP"),
            SipTransport::TLS => write!(f, "TLS"),
            SipTransport::DTLS => write!(f, "DTLS"),
            SipTransport::WS => write!(f, "WS"),
            SipTransport::WSS => write!(f, "WSS"),
        }
    }
}

/// SIP dialog state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogState {
    /// No dialog established
    None,
    /// Early dialog (provisional response received)
    Early,
    /// Confirmed dialog (2xx response received)
    Confirmed,
    /// Dialog terminated
    Terminated,
}

/// SIP transaction state for INVITE
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InviteTransactionState {
    /// Initial state
    Calling,
    /// Provisional response received
    Proceeding,
    /// Final response received
    Completed,
    /// Transaction completed
    Terminated,
}

/// SIP transaction state for non-INVITE
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NonInviteTransactionState {
    /// Request sent
    Trying,
    /// Final response received
    Completed,
    /// Transaction completed
    Terminated,
}

/// SIP dialog information
#[derive(Debug, Clone)]
pub struct SipDialog {
    /// Dialog ID (Call-ID + From tag + To tag)
    pub dialog_id: String,
    /// Call-ID
    pub call_id: String,
    /// Local tag
    pub local_tag: String,
    /// Remote tag
    pub remote_tag: String,
    /// Local URI
    pub local_uri: Uri,
    /// Remote URI
    pub remote_uri: Uri,
    /// Local sequence number
    pub local_seq: u32,
    /// Remote sequence number
    pub remote_seq: u32,
    /// Route set
    pub route_set: Vec<Uri>,
    /// Dialog state
    pub state: DialogState,
    /// Creation time
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last activity time
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

/// SIP transaction
#[derive(Debug, Clone)]
pub struct SipTransaction {
    /// Transaction ID (branch parameter)
    pub transaction_id: String,
    /// Transaction method
    pub method: Method,
    /// Transaction state
    pub state: TransactionState,
    /// Request that started the transaction
    pub request: Request,
    /// Responses received/sent
    pub responses: Vec<Response>,
    /// Timer values
    pub timers: TransactionTimers,
    /// Creation time
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Transaction state enum
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionState {
    Invite(InviteTransactionState),
    NonInvite(NonInviteTransactionState),
}

/// SIP transaction timers per RFC 3261
#[derive(Debug, Clone)]
pub struct TransactionTimers {
    /// Timer A (INVITE retransmission)
    pub timer_a: Option<chrono::DateTime<chrono::Utc>>,
    /// Timer B (INVITE timeout)
    pub timer_b: Option<chrono::DateTime<chrono::Utc>>,
    /// Timer D (wait for ACK)
    pub timer_d: Option<chrono::DateTime<chrono::Utc>>,
    /// Timer E (non-INVITE retransmission)
    pub timer_e: Option<chrono::DateTime<chrono::Utc>>,
    /// Timer F (non-INVITE timeout)
    pub timer_f: Option<chrono::DateTime<chrono::Utc>>,
    /// Timer G (INVITE response retransmission)
    pub timer_g: Option<chrono::DateTime<chrono::Utc>>,
    /// Timer H (wait for ACK timeout)
    pub timer_h: Option<chrono::DateTime<chrono::Utc>>,
    /// Timer I (ACK wait)
    pub timer_i: Option<chrono::DateTime<chrono::Utc>>,
    /// Timer J (non-INVITE response wait)
    pub timer_j: Option<chrono::DateTime<chrono::Utc>>,
    /// Timer K (response wait)
    pub timer_k: Option<chrono::DateTime<chrono::Utc>>,
}

impl Default for TransactionTimers {
    fn default() -> Self {
        Self {
            timer_a: None,
            timer_b: None,
            timer_d: None,
            timer_e: None,
            timer_f: None,
            timer_g: None,
            timer_h: None,
            timer_i: None,
            timer_j: None,
            timer_k: None,
        }
    }
}

/// SIP message parser implementing RFC 3261
pub struct SipParser {
    /// Local hostname/IP for Via headers
    pub local_host: String,
    /// Local port
    pub local_port: u16,
    /// User agent string
    pub user_agent: String,
}

impl SipParser {
    /// Create new SIP parser
    pub fn new(local_host: String, local_port: u16, user_agent: String) -> Self {
        Self {
            local_host,
            local_port,
            user_agent,
        }
    }

    /// Parse SIP message from bytes
    pub fn parse_message(&self, data: &[u8], source: SocketAddr, destination: SocketAddr, transport: SipTransport) -> Result<SipMessage> {
        let message_str = String::from_utf8_lossy(data);
        debug!("Parsing SIP message from {}: {}", source, message_str);

        // Parse using rsip library
        let message = RsipMessage::from_str(&message_str)
            .map_err(|e| anyhow!("Failed to parse SIP message: {}", e))?;

        // Validate message structure
        self.validate_message(&message)?;

        Ok(SipMessage {
            message,
            source,
            destination,
            transport,
            received_at: chrono::Utc::now(),
            message_id: Uuid::new_v4().to_string(),
        })
    }

    /// Validate SIP message per RFC 3261
    fn validate_message(&self, message: &RsipMessage) -> Result<()> {
        match message {
            RsipMessage::Request(req) => self.validate_request(req),
            RsipMessage::Response(resp) => self.validate_response(resp),
        }
    }

    /// Validate SIP request
    fn validate_request(&self, request: &Request) -> Result<()> {
        // Check required headers per RFC 3261 Section 8.1.1
        
        // Via header (required)
        if request.via_header().is_err() {
            return Err(anyhow!("Missing required Via header"));
        }

        // From header (required)
        if request.from_header().is_err() {
            return Err(anyhow!("Missing required From header"));
        }

        // To header (required)
        if request.to_header().is_err() {
            return Err(anyhow!("Missing required To header"));
        }

        // Call-ID header (required)
        if request.call_id_header().is_err() {
            return Err(anyhow!("Missing required Call-ID header"));
        }

        // CSeq header (required)
        if request.cseq_header().is_err() {
            return Err(anyhow!("Missing required CSeq header"));
        }

        // Max-Forwards header (required)
        if request.max_forwards_header().is_err() {
            return Err(anyhow!("Missing required Max-Forwards header"));
        }

        // Method-specific validation
        match request.method() {
            Method::Invite => self.validate_invite_request(request),
            Method::Register => self.validate_register_request(request),
            Method::Options => self.validate_options_request(request),
            _ => Ok(()),
        }
    }

    /// Validate INVITE request
    fn validate_invite_request(&self, request: &Request) -> Result<()> {
        // Contact header required for INVITE
        if request.contact_header().is_err() {
            return Err(anyhow!("INVITE request missing required Contact header"));
        }

        // Content-Type and Content-Length for SDP
        if let Ok(content_length) = request.content_length_header() {
            if content_length.length() > 0 {
                if request.content_type_header().is_err() {
                    return Err(anyhow!("INVITE with body missing Content-Type header"));
                }
            }
        }

        Ok(())
    }

    /// Validate REGISTER request
    fn validate_register_request(&self, request: &Request) -> Result<()> {
        // REGISTER must have Contact header
        if request.contact_header().is_err() {
            return Err(anyhow!("REGISTER request missing required Contact header"));
        }

        Ok(())
    }

    /// Validate OPTIONS request
    fn validate_options_request(&self, _request: &Request) -> Result<()> {
        // OPTIONS has no special requirements beyond basic headers
        Ok(())
    }

    /// Validate SIP response
    fn validate_response(&self, response: &Response) -> Result<()> {
        // Check required headers per RFC 3261 Section 8.1.1
        
        // Via header (required)
        if response.via_header().is_err() {
            return Err(anyhow!("Response missing required Via header"));
        }

        // From header (required)
        if response.from_header().is_err() {
            return Err(anyhow!("Response missing required From header"));
        }

        // To header (required)
        if response.to_header().is_err() {
            return Err(anyhow!("Response missing required To header"));
        }

        // Call-ID header (required)
        if response.call_id_header().is_err() {
            return Err(anyhow!("Response missing required Call-ID header"));
        }

        // CSeq header (required)
        if response.cseq_header().is_err() {
            return Err(anyhow!("Response missing required CSeq header"));
        }

        Ok(())
    }

    /// Create transaction ID from request
    pub fn create_transaction_id(&self, request: &Request) -> Result<String> {
        // Transaction ID is the branch parameter from the top Via header
        let via = request.via_header()
            .map_err(|e| anyhow!("No Via header found: {}", e))?;
        
        // Look for branch parameter
        for param in via.params() {
            if let Param::Branch(branch) = param {
                return Ok(branch.to_string());
            }
        }

        // If no branch parameter, create one (shouldn't happen with RFC 3261 compliant clients)
        warn!("No branch parameter found in Via header, creating transaction ID");
        Ok(format!("z9hG4bK-{}", Uuid::new_v4()))
    }

    /// Create dialog ID from request/response
    pub fn create_dialog_id(&self, call_id: &str, from_tag: &str, to_tag: &str) -> String {
        format!("{}:{}:{}", call_id, from_tag, to_tag)
    }

    /// Extract tag from From/To header
    pub fn extract_tag(&self, header: &str) -> Option<String> {
        // Simple tag extraction - in production this should be more robust
        if let Some(tag_start) = header.find("tag=") {
            let tag_value = &header[tag_start + 4..];
            if let Some(end) = tag_value.find(';') {
                Some(tag_value[..end].to_string())
            } else {
                Some(tag_value.trim().to_string())
            }
        } else {
            None
        }
    }

    /// Create SIP response from request
    pub fn create_response(&self, request: &Request, status_code: u16, reason_phrase: &str) -> Result<Response> {
        let mut response = Response::new(
            Version::V2,
            status_code.into(),
            reason_phrase
        );

        // Copy required headers from request
        if let Ok(via) = request.via_header() {
            response.headers_mut().push(Header::Via(via.clone()));
        }

        if let Ok(from) = request.from_header() {
            response.headers_mut().push(Header::From(from.clone()));
        }

        if let Ok(to) = request.to_header() {
            let mut to_header = to.clone();
            // Add tag to To header if not present (for dialog creation)
            if !to_header.params().iter().any(|p| matches!(p, Param::Tag(_))) {
                let tag = format!("tag-{}", Uuid::new_v4().to_string()[..8]);
                to_header.params_mut().push(Param::Tag(tag));
            }
            response.headers_mut().push(Header::To(to_header));
        }

        if let Ok(call_id) = request.call_id_header() {
            response.headers_mut().push(Header::CallId(call_id.clone()));
        }

        if let Ok(cseq) = request.cseq_header() {
            response.headers_mut().push(Header::CSeq(cseq.clone()));
        }

        // Add Content-Length: 0 for responses without body
        let content_length = ContentLength::new("0");
        response.headers_mut().push(Header::ContentLength(content_length));

        Ok(response)
    }

    /// Create ACK request for 2xx response
    pub fn create_ack_for_2xx(&self, invite: &Request, response: &Response) -> Result<Request> {
        let mut ack = Request::new(
            Method::Ack,
            invite.uri().clone(),
            Version::V2
        );

        // Copy headers from original INVITE, but update as needed
        if let Ok(via) = invite.via_header() {
            let mut ack_via = via.clone();
            // Generate new branch for ACK
            let new_branch = format!("z9hG4bK-ack-{}", Uuid::new_v4());
            for param in ack_via.params_mut() {
                if let Param::Branch(_) = param {
                    *param = Param::Branch(new_branch.clone());
                    break;
                }
            }
            ack.headers_mut().push(Header::Via(ack_via));
        }

        if let Ok(from) = invite.from_header() {
            ack.headers_mut().push(Header::From(from.clone()));
        }

        if let Ok(to) = response.to_header() {
            ack.headers_mut().push(Header::To(to.clone()));
        }

        if let Ok(call_id) = invite.call_id_header() {
            ack.headers_mut().push(Header::CallId(call_id.clone()));
        }

        // CSeq number same as INVITE, but method is ACK
        if let Ok(cseq) = invite.cseq_header() {
            let ack_cseq = CSeq::new(cseq.seq(), Method::Ack);
            ack.headers_mut().push(Header::CSeq(ack_cseq));
        }

        // Max-Forwards
        let max_forwards = rsip::headers::MaxForwards::new("70");
        ack.headers_mut().push(Header::MaxForwards(max_forwards));

        // Content-Length: 0
        let content_length = ContentLength::new("0");
        ack.headers_mut().push(Header::ContentLength(content_length));

        Ok(ack)
    }

    /// Check if message is retransmission
    pub fn is_retransmission(&self, message: &SipMessage, previous_messages: &[SipMessage]) -> bool {
        match &message.message {
            RsipMessage::Request(req) => {
                // For requests, check Call-ID, CSeq, From tag, and method
                for prev_msg in previous_messages {
                    if let RsipMessage::Request(prev_req) = &prev_msg.message {
                        if self.requests_match(req, prev_req) {
                            return true;
                        }
                    }
                }
            }
            RsipMessage::Response(resp) => {
                // For responses, check Call-ID, CSeq, From tag, To tag, and status code
                for prev_msg in previous_messages {
                    if let RsipMessage::Response(prev_resp) = &prev_msg.message {
                        if self.responses_match(resp, prev_resp) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Check if two requests match (for retransmission detection)
    fn requests_match(&self, req1: &Request, req2: &Request) -> bool {
        // Method must match
        if req1.method() != req2.method() {
            return false;
        }

        // Call-ID must match
        let call_id1 = req1.call_id_header().ok();
        let call_id2 = req2.call_id_header().ok();
        if call_id1 != call_id2 {
            return false;
        }

        // CSeq must match
        let cseq1 = req1.cseq_header().ok();
        let cseq2 = req2.cseq_header().ok();
        if cseq1 != cseq2 {
            return false;
        }

        // From tag must match
        let from1 = req1.from_header().ok();
        let from2 = req2.from_header().ok();
        if from1 != from2 {
            return false;
        }

        true
    }

    /// Check if two responses match (for retransmission detection)
    fn responses_match(&self, resp1: &Response, resp2: &Response) -> bool {
        // Status code must match
        if resp1.status_code() != resp2.status_code() {
            return false;
        }

        // Call-ID must match
        let call_id1 = resp1.call_id_header().ok();
        let call_id2 = resp2.call_id_header().ok();
        if call_id1 != call_id2 {
            return false;
        }

        // CSeq must match
        let cseq1 = resp1.cseq_header().ok();
        let cseq2 = resp2.cseq_header().ok();
        if cseq1 != cseq2 {
            return false;
        }

        // From tag must match
        let from1 = resp1.from_header().ok();
        let from2 = resp2.from_header().ok();
        if from1 != from2 {
            return false;
        }

        // To tag must match
        let to1 = resp1.to_header().ok();
        let to2 = resp2.to_header().ok();
        if to1 != to2 {
            return false;
        }

        true
    }
}

/// SIP message utilities
pub mod utils {
    use super::*;

    /// Extract Call-ID from message
    pub fn extract_call_id(message: &RsipMessage) -> Result<String> {
        match message {
            RsipMessage::Request(req) => {
                let call_id = req.call_id_header()
                    .map_err(|e| anyhow!("No Call-ID header: {}", e))?;
                Ok(call_id.value().to_string())
            }
            RsipMessage::Response(resp) => {
                let call_id = resp.call_id_header()
                    .map_err(|e| anyhow!("No Call-ID header: {}", e))?;
                Ok(call_id.value().to_string())
            }
        }
    }

    /// Extract From tag from message
    pub fn extract_from_tag(message: &RsipMessage) -> Result<Option<String>> {
        match message {
            RsipMessage::Request(req) => {
                let from = req.from_header()
                    .map_err(|e| anyhow!("No From header: {}", e))?;
                Ok(extract_tag_from_header(&from))
            }
            RsipMessage::Response(resp) => {
                let from = resp.from_header()
                    .map_err(|e| anyhow!("No From header: {}", e))?;
                Ok(extract_tag_from_header(&from))
            }
        }
    }

    /// Extract To tag from message
    pub fn extract_to_tag(message: &RsipMessage) -> Result<Option<String>> {
        match message {
            RsipMessage::Request(req) => {
                let to = req.to_header()
                    .map_err(|e| anyhow!("No To header: {}", e))?;
                Ok(extract_tag_from_header(&to))
            }
            RsipMessage::Response(resp) => {
                let to = resp.to_header()
                    .map_err(|e| anyhow!("No To header: {}", e))?;
                Ok(extract_tag_from_header(&to))
            }
        }
    }

    /// Extract tag parameter from From/To header
    fn extract_tag_from_header(header: &dyn std::fmt::Display) -> Option<String> {
        let header_str = header.to_string();
        if let Some(tag_start) = header_str.find("tag=") {
            let tag_value = &header_str[tag_start + 4..];
            if let Some(end) = tag_value.find(';') {
                Some(tag_value[..end].to_string())
            } else {
                Some(tag_value.trim().to_string())
            }
        } else {
            None
        }
    }

    /// Extract CSeq number from message
    pub fn extract_cseq_number(message: &RsipMessage) -> Result<u32> {
        match message {
            RsipMessage::Request(req) => {
                let cseq = req.cseq_header()
                    .map_err(|e| anyhow!("No CSeq header: {}", e))?;
                Ok(cseq.seq())
            }
            RsipMessage::Response(resp) => {
                let cseq = resp.cseq_header()
                    .map_err(|e| anyhow!("No CSeq header: {}", e))?;
                Ok(cseq.seq())
            }
        }
    }

    /// Check if response is provisional (1xx)
    pub fn is_provisional_response(message: &RsipMessage) -> bool {
        if let RsipMessage::Response(resp) = message {
            let status = resp.status_code().as_u16();
            status >= 100 && status < 200
        } else {
            false
        }
    }

    /// Check if response is success (2xx)
    pub fn is_success_response(message: &RsipMessage) -> bool {
        if let RsipMessage::Response(resp) = message {
            let status = resp.status_code().as_u16();
            status >= 200 && status < 300
        } else {
            false
        }
    }

    /// Check if response is failure (3xx, 4xx, 5xx, 6xx)
    pub fn is_failure_response(message: &RsipMessage) -> bool {
        if let RsipMessage::Response(resp) = message {
            let status = resp.status_code().as_u16();
            status >= 300
        } else {
            false
        }
    }
}