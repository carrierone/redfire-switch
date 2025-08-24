/*
 * SIP-I Enhanced B2BUA Implementation
 * Adds RFC 3398 ISUP encapsulation for Class 4 carrier interconnection
 */

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use redfire_sip_stack::sipt_sipi::{
    utils, IsupMessage, IsupMessageType, IsupParameter, IsupParameterType, SipTSipIConfig,
    SipTSipIService,
};

// Compliance framework integration
use crate::compliance_framework::{CallEvent, CallEventType, ComplianceFramework};

/// Enhanced call leg with SIP-I support
#[derive(Debug, Clone)]
pub struct SipICallLeg {
    pub call_id: String,
    pub from_tag: String,
    pub to_tag: Option<String>,
    pub local_addr: SocketAddr,
    pub remote_addr: SocketAddr,
    pub state: CallState,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    // SIP-I specific fields
    pub cic: Option<u16>,
    pub isup_message: Option<IsupMessage>,
    pub from_number: Option<String>,
    pub to_number: Option<String>,
    pub carrier_type: CarrierType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CallState {
    Initial,
    Proceeding,
    Ringing,
    Connected,
    Disconnecting,
    Disconnected,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CarrierType {
    SipNative,  // Pure SIP carrier
    LegacyPstn, // Legacy PSTN requiring ISUP
    SipI,       // SIP-I carrier
    Mixed,      // Mixed environment
}

/// Call session with SIP-I tracking
#[derive(Debug, Clone)]
pub struct SipICallSession {
    pub call_id: String,
    pub a_leg: SipICallLeg,
    pub b_leg: SipICallLeg,
    pub state: CallState,
    pub created_at: DateTime<Utc>,
    // SIP-I specific
    pub requires_isup: bool,
    pub isup_iam: Option<IsupMessage>,
    pub isup_acm: Option<IsupMessage>,
    pub isup_anm: Option<IsupMessage>,
    pub isup_rel: Option<IsupMessage>,
}

/// SIP-I enabled B2BUA
pub struct SipIB2BUA {
    socket: Arc<UdpSocket>,
    calls: Arc<RwLock<HashMap<String, SipICallSession>>>,
    termination_host: String,
    termination_port: u16,
    addr_to_call: Arc<RwLock<HashMap<SocketAddr, String>>>,
    // SIP-I service
    sipi_service: Arc<SipTSipIService>,
    // CIC management
    used_cics: Arc<RwLock<Vec<u16>>>,
    trunk_group_id: String,
    // Compliance framework integration
    compliance_framework: Arc<ComplianceFramework>,
}

impl SipIB2BUA {
    pub async fn new(
        bind_addr: SocketAddr,
        term_host: String,
        term_port: u16,
        sipi_config: SipTSipIConfig,
        trunk_group_id: String,
        compliance_framework: Arc<ComplianceFramework>,
    ) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr).await?;
        info!("SIP-I B2BUA listening on {}", bind_addr);
        info!("Termination target: {}:{}", term_host, term_port);

        // Initialize SIP-I service
        let sipi_service = Arc::new(SipTSipIService::new(sipi_config));
        info!(
            "SIP-I service initialized for B2BUA - SIP-T: {}, SIP-I: {}",
            sipi_service.is_sipt_enabled(),
            sipi_service.is_sipi_enabled()
        );

        Ok(Self {
            socket: Arc::new(socket),
            calls: Arc::new(RwLock::new(HashMap::new())),
            termination_host: term_host,
            termination_port: term_port,
            addr_to_call: Arc::new(RwLock::new(HashMap::new())),
            sipi_service,
            used_cics: Arc::new(RwLock::new(Vec::new())),
            trunk_group_id,
            compliance_framework,
        })
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting SIP-I B2BUA with ISUP encapsulation...");
        let mut buffer = vec![0u8; 8192]; // Larger buffer for ISUP content

        loop {
            match self.socket.recv_from(&mut buffer).await {
                Ok((len, from)) => {
                    // SECURITY: Input size validation (Fixes CVE-2024-003)
                    if len > crate::security_utils::MAX_SIP_MESSAGE_SIZE {
                        warn!("Oversized message from {}: {} bytes, dropping", from, len);
                        continue;
                    }

                    let message = String::from_utf8_lossy(&buffer[..len]);

                    // SECURITY: Message content validation
                    if let Err(e) = crate::security_utils::validate_message_size(&message) {
                        warn!("Message validation failed from {}: {}", from, e);
                        continue;
                    }

                    debug!(
                        "Received from {}: {}",
                        from,
                        crate::security_utils::sanitize_for_logging(
                            message.lines().next().unwrap_or("")
                        )
                    );

                    if let Err(e) = self.handle_message(&message, from).await {
                        error!("Error handling message: {}", e);
                    }
                }
                Err(e) => {
                    error!("Error receiving UDP packet: {}", e);
                }
            }
        }
    }

    async fn handle_message(&self, message: &str, from: SocketAddr) -> Result<()> {
        if message.starts_with("INVITE ") {
            self.handle_invite(message, from).await
        } else if message.starts_with("ACK ") {
            self.handle_ack(message, from).await
        } else if message.starts_with("BYE ") {
            self.handle_bye(message, from).await
        } else if message.starts_with("CANCEL ") {
            self.handle_cancel(message, from).await
        } else if message.starts_with("OPTIONS ") {
            self.handle_options(message, from).await
        } else if message.starts_with("SIP/2.0 ") {
            self.handle_response(message, from).await
        } else {
            warn!("Unhandled message type from {}", from);
            Ok(())
        }
    }

    async fn handle_invite(&self, message: &str, from: SocketAddr) -> Result<()> {
        info!("Handling SIP-I INVITE from {}", from);

        let call_id = self.extract_header(message, "Call-ID")?;
        let from_number = self.extract_phone_number_from_header(message, "From")?;
        let to_number = self.extract_phone_number_from_header(message, "To")?;

        // Determine carrier type based on headers or configuration
        let originating_carrier = self.determine_carrier_type(message, from).await;
        let terminating_carrier = self.determine_terminating_carrier_type(&to_number).await;

        info!(
            "Call routing: {} -> {} (Orig: {:?}, Term: {:?})",
            from_number, to_number, originating_carrier, terminating_carrier
        );

        // Extract any existing ISUP data from SIP-I body
        let incoming_isup = self.extract_isup_from_sip(message).await?;

        // Send 100 Trying immediately
        let trying_response = self.create_100_trying(message)?;
        self.send_to(trying_response.as_bytes(), from).await?;

        // Create or modify INVITE for termination
        let (modified_invite, cic, isup_iam) = if terminating_carrier == CarrierType::LegacyPstn
            || terminating_carrier == CarrierType::SipI
        {
            // Generate ISUP IAM for PSTN/SIP-I termination
            let cic = self.allocate_cic().await?;
            let iam = if let Some(ref existing_isup) = incoming_isup {
                // Pass through existing ISUP with modifications
                self.modify_isup_for_termination(
                    existing_isup.clone(),
                    &from_number,
                    &to_number,
                    cic,
                )
                .await?
            } else {
                // Create new ISUP IAM from SIP
                self.sipi_service
                    .sip_to_iam(&from_number, &to_number, cic)?
            };

            let modified_invite = self.add_isup_to_sip(message, &iam).await?;
            (modified_invite, Some(cic), Some(iam))
        } else {
            // Pure SIP termination - remove any ISUP content
            let modified_invite = self.modify_invite_for_sip_termination(message)?;
            (modified_invite, None, None)
        };

        // Forward to termination
        let termination_addr = format!("{}:{}", self.termination_host, self.termination_port);
        let termination_socket: SocketAddr = termination_addr.parse()?;

        self.send_to(modified_invite.as_bytes(), termination_socket)
            .await?;
        info!(
            "INVITE forwarded to termination for call {} (CIC: {:?})",
            call_id, cic
        );

        // Create call session with SIP-I data
        let a_leg = SipICallLeg {
            call_id: call_id.clone(),
            from_tag: self.extract_from_tag(message)?,
            to_tag: None,
            local_addr: self.socket.local_addr()?,
            remote_addr: from,
            state: CallState::Initial,
            created_at: Utc::now(),
            last_activity: Utc::now(),
            cic: None,
            isup_message: incoming_isup.clone(),
            from_number: Some(from_number.clone()),
            to_number: Some(to_number.clone()),
            carrier_type: originating_carrier,
        };

        let b_leg = SipICallLeg {
            call_id: call_id.clone(),
            from_tag: self.extract_from_tag(&modified_invite)?,
            to_tag: None,
            local_addr: self.socket.local_addr()?,
            remote_addr: termination_socket,
            state: CallState::Initial,
            created_at: Utc::now(),
            last_activity: Utc::now(),
            cic,
            isup_message: isup_iam.clone(),
            from_number: Some(from_number),
            to_number: Some(to_number),
            carrier_type: terminating_carrier,
        };

        let session = SipICallSession {
            call_id: call_id.clone(),
            a_leg,
            b_leg,
            state: CallState::Proceeding,
            created_at: Utc::now(),
            requires_isup: cic.is_some(),
            isup_iam,
            isup_acm: None,
            isup_anm: None,
            isup_rel: None,
        };

        // Store session
        {
            let mut calls = self.calls.write().await;
            calls.insert(call_id.clone(), session.clone());
        }

        // Map termination address to call ID for response routing
        {
            let mut addr_map = self.addr_to_call.write().await;
            addr_map.insert(termination_socket, call_id.clone());
        }

        // Submit compliance event for call initiation
        let mut call_event = CallEvent {
            call_id: call_id.clone(),
            event_type: CallEventType::CallAttempt,
            timestamp: Utc::now(),
            calling_number: session.a_leg.from_number.clone().unwrap_or_default(),
            called_number: session.a_leg.to_number.clone().unwrap_or_default(),
            sip_method: Some("INVITE".to_string()),
            sip_response_code: None,
            source_ip: Some(session.a_leg.remote_addr.ip()),
            dest_ip: Some(session.b_leg.remote_addr.ip()),
            user_agent: None,
            sip_headers: HashMap::new(),
            rtp_stats: None,
        };

        // Check for J-STD-025 (U.S.) or ETSI LI (international) lawful intercept requirements
        if let Some(from_number) = &session.a_leg.from_number {
            if let Some(to_number) = &session.a_leg.to_number {
                // Add jurisdiction-specific intercept flags
                call_event
                    .sip_headers
                    .insert("X-Jurisdiction".to_string(), "US-JSTD025".to_string());

                info!("J-STD-025 B2BUA Integration: Checking intercept requirements for call {} ({} -> {})", 
                      call_id, from_number, to_number);
            }
        }

        if let Err(e) = self.compliance_framework.submit_call_event(call_event) {
            warn!("Failed to submit call attempt event for {}: {}", call_id, e);
        }

        Ok(())
    }

    async fn handle_response(&self, message: &str, from: SocketAddr) -> Result<()> {
        debug!("Handling SIP-I response from {}", from);

        // Find call session based on the sender address
        let call_id = {
            let addr_map = self.addr_to_call.read().await;
            addr_map.get(&from).cloned()
        };

        if let Some(call_id) = call_id {
            let session = {
                let calls = self.calls.read().await;
                calls.get(&call_id).cloned()
            };

            if let Some(mut session) = session {
                // Extract any ISUP message from response
                let response_isup = self.extract_isup_from_sip(message).await?;

                // Update call state and ISUP messages based on response
                if message.contains("SIP/2.0 18") {
                    session.state = CallState::Ringing;
                    // Check for ISUP ACM (Address Complete Message)
                    if let Some(isup) = &response_isup {
                        if isup.message_type == IsupMessageType::ACM {
                            session.isup_acm = response_isup.clone();
                            info!("Received ISUP ACM for call {}", call_id);
                        }
                    }
                } else if message.contains("SIP/2.0 200") {
                    session.state = CallState::Connected;
                    // Check for ISUP ANM (Answer Message)
                    if let Some(isup) = &response_isup {
                        if isup.message_type == IsupMessageType::ANM {
                            session.isup_anm = response_isup.clone();
                            info!("Received ISUP ANM for call {}", call_id);
                        }
                    }

                    // Submit compliance event for call establishment
                    let call_event = CallEvent {
                        call_id: call_id.clone(),
                        event_type: CallEventType::CallAnswered,
                        timestamp: Utc::now(),
                        calling_number: session.a_leg.from_number.clone().unwrap_or_default(),
                        called_number: session.a_leg.to_number.clone().unwrap_or_default(),
                        sip_method: None,
                        sip_response_code: Some(200),
                        source_ip: Some(session.b_leg.remote_addr.ip()),
                        dest_ip: Some(session.a_leg.remote_addr.ip()),
                        user_agent: None,
                        sip_headers: HashMap::new(),
                        rtp_stats: None,
                    };

                    if let Err(e) = self.compliance_framework.submit_call_event(call_event) {
                        warn!(
                            "Failed to submit call established event for {}: {}",
                            call_id, e
                        );
                    }
                } else if message.contains("SIP/2.0 4")
                    || message.contains("SIP/2.0 5")
                    || message.contains("SIP/2.0 6")
                {
                    session.state = CallState::Disconnected;
                    // Check for ISUP REL (Release)
                    if let Some(isup) = &response_isup {
                        if isup.message_type == IsupMessageType::REL {
                            session.isup_rel = response_isup.clone();
                            info!("Received ISUP REL for call {}", call_id);
                        }
                    }

                    // Calculate call duration
                    let call_duration = Utc::now().signed_duration_since(session.created_at);
                    let duration_seconds = if call_duration.num_seconds() >= 0 {
                        call_duration.num_seconds() as u64
                    } else {
                        0
                    };

                    // Extract response code for termination cause
                    let response_code = if message.contains("SIP/2.0 4") {
                        400
                    } else if message.contains("SIP/2.0 5") {
                        500
                    } else if message.contains("SIP/2.0 6") {
                        600
                    } else {
                        487
                    };

                    // Submit compliance event for call termination
                    let call_event = CallEvent {
                        call_id: call_id.clone(),
                        event_type: CallEventType::CallEnded,
                        timestamp: Utc::now(),
                        calling_number: session.a_leg.from_number.clone().unwrap_or_default(),
                        called_number: session.a_leg.to_number.clone().unwrap_or_default(),
                        sip_method: None,
                        sip_response_code: Some(response_code),
                        source_ip: Some(session.b_leg.remote_addr.ip()),
                        dest_ip: Some(session.a_leg.remote_addr.ip()),
                        user_agent: None,
                        sip_headers: HashMap::new(),
                        rtp_stats: None,
                    };

                    if let Err(e) = self.compliance_framework.submit_call_event(call_event) {
                        warn!(
                            "Failed to submit call termination event for {}: {}",
                            call_id, e
                        );
                    }
                }

                // Modify response for origination leg based on carrier type
                let modified_response = self
                    .modify_response_for_origination(message, &session)
                    .await?;

                // Send response to A-leg
                self.send_to(modified_response.as_bytes(), session.a_leg.remote_addr)
                    .await?;
                debug!("Response forwarded to A-leg for call {}", call_id);

                // Update session
                {
                    let mut calls = self.calls.write().await;
                    calls.insert(call_id, session);
                }
            }
        } else {
            warn!("No call session found for response from {}", from);
        }

        Ok(())
    }

    async fn handle_options(&self, message: &str, from: SocketAddr) -> Result<()> {
        info!("Handling OPTIONS from {}", from);

        let response = self.create_options_response(message)?;
        self.send_to(response.as_bytes(), from).await?;
        info!("OPTIONS response sent to {}", from);

        Ok(())
    }

    async fn handle_ack(&self, message: &str, from: SocketAddr) -> Result<()> {
        debug!("Handling ACK from {}", from);

        let call_id = self.extract_header(message, "Call-ID")?;

        // Find session and forward ACK to B-leg
        let session = {
            let calls = self.calls.read().await;
            calls.get(&call_id).cloned()
        };

        if let Some(session) = session {
            let modified_ack = self.modify_ack_for_termination(message, &session)?;
            self.send_to(modified_ack.as_bytes(), session.b_leg.remote_addr)
                .await?;
            debug!("ACK forwarded to B-leg for call {}", call_id);
        }

        Ok(())
    }

    async fn handle_bye(&self, message: &str, from: SocketAddr) -> Result<()> {
        info!("Handling BYE from {}", from);

        let call_id = self.extract_header(message, "Call-ID")?;

        // Find session
        let session = {
            let calls = self.calls.read().await;
            calls.get(&call_id).cloned()
        };

        if let Some(mut session) = session {
            session.state = CallState::Disconnecting;

            // Create ISUP REL if terminating to PSTN/SIP-I
            if session.requires_isup && from == session.a_leg.remote_addr {
                let isup_rel = self.create_isup_rel_from_bye(message, &session).await?;
                session.isup_rel = Some(isup_rel.clone());

                // Add ISUP REL to forwarded BYE
                let modified_bye = self.add_isup_to_sip(message, &isup_rel).await?;
                self.send_to(modified_bye.as_bytes(), session.b_leg.remote_addr)
                    .await?;
            } else {
                // Forward BYE without ISUP
                let modified_bye = if from == session.a_leg.remote_addr {
                    self.modify_bye_for_termination(message, &session)?
                } else {
                    self.modify_bye_for_origination(message, &session)?
                };

                let target_addr = if from == session.a_leg.remote_addr {
                    session.b_leg.remote_addr
                } else {
                    session.a_leg.remote_addr
                };

                self.send_to(modified_bye.as_bytes(), target_addr).await?;
            }

            // Send 200 OK to sender
            let bye_response = self.create_bye_response(message)?;
            self.send_to(bye_response.as_bytes(), from).await?;

            // Calculate call duration for compliance
            let call_duration = Utc::now().signed_duration_since(session.created_at);
            let duration_seconds = if call_duration.num_seconds() >= 0 {
                call_duration.num_seconds() as u64
            } else {
                0
            };

            // Submit compliance event for call termination (BYE)
            let call_event = CallEvent {
                call_id: call_id.clone(),
                event_type: CallEventType::CallEnded,
                timestamp: Utc::now(),
                calling_number: session.a_leg.from_number.clone().unwrap_or_default(),
                called_number: session.a_leg.to_number.clone().unwrap_or_default(),
                sip_method: Some("BYE".to_string()),
                sip_response_code: None,
                source_ip: Some(from.ip()),
                dest_ip: Some(session.a_leg.local_addr.ip()),
                user_agent: None,
                sip_headers: HashMap::new(),
                rtp_stats: None,
            };

            if let Err(e) = self.compliance_framework.submit_call_event(call_event) {
                warn!(
                    "Failed to submit call termination event for {}: {}",
                    call_id, e
                );
            }

            // Release CIC if allocated
            if let Some(cic) = session.b_leg.cic {
                self.release_cic(cic).await;
            }

            // Clean up session
            {
                let mut calls = self.calls.write().await;
                calls.remove(&call_id);
            }

            info!("Call {} terminated with SIP-I processing", call_id);
        }

        Ok(())
    }

    async fn handle_cancel(&self, message: &str, from: SocketAddr) -> Result<()> {
        info!("Handling CANCEL from {}", from);

        let call_id = self.extract_header(message, "Call-ID")?;

        // Find session and forward CANCEL
        let session = {
            let calls = self.calls.read().await;
            calls.get(&call_id).cloned()
        };

        if let Some(session) = session {
            // Create ISUP REL for CANCEL if needed
            if session.requires_isup {
                let isup_rel = self.create_isup_rel_from_cancel(message, &session).await?;
                let modified_cancel = self.add_isup_to_sip(message, &isup_rel).await?;
                self.send_to(modified_cancel.as_bytes(), session.b_leg.remote_addr)
                    .await?;
            } else {
                let modified_cancel = self.modify_cancel_for_termination(message, &session)?;
                self.send_to(modified_cancel.as_bytes(), session.b_leg.remote_addr)
                    .await?;
            }

            // Send 200 OK to CANCEL
            let cancel_response = self.create_cancel_response(message)?;
            self.send_to(cancel_response.as_bytes(), from).await?;

            info!("CANCEL forwarded for call {}", call_id);
        }

        Ok(())
    }

    // SIP-I specific helper methods

    async fn determine_carrier_type(&self, _message: &str, _from: SocketAddr) -> CarrierType {
        // In a real implementation, this would check:
        // - Source IP ranges
        // - SIP headers (User-Agent, Via, etc.)
        // - Configuration mappings
        // - Content-Type for existing ISUP content
        CarrierType::SipNative // Default for testing
    }

    async fn determine_terminating_carrier_type(&self, _to_number: &str) -> CarrierType {
        // In a real implementation, this would check:
        // - Number prefix routing tables
        // - Carrier configuration
        // - LRN/LNP lookups
        // - Route policies
        CarrierType::LegacyPstn // Default to PSTN for testing
    }

    async fn extract_isup_from_sip(&self, message: &str) -> Result<Option<IsupMessage>> {
        // Check Content-Type for ISUP content
        if let Ok(content_type) = self.extract_header(message, "Content-Type") {
            if content_type.contains("application/ISUP") || content_type.contains("multipart/mixed")
            {
                // Extract body
                if let Some(body_start) = message.find("\r\n\r\n") {
                    let body = &message[body_start + 4..];

                    if self.sipi_service.is_sipt_enabled()
                        && content_type.contains("multipart/mixed")
                    {
                        // Parse SIP-T multipart body
                        match self.sipi_service.parse_sipt_body(body) {
                            Ok((isup_data, _sdp)) => {
                                match self.sipi_service.parse_isup_message(&isup_data) {
                                    Ok(isup_msg) => return Ok(Some(isup_msg)),
                                    Err(e) => debug!("Failed to parse ISUP from SIP-T: {}", e),
                                }
                            }
                            Err(e) => debug!("Failed to parse SIP-T body: {}", e),
                        }
                    } else if self.sipi_service.is_sipi_enabled()
                        && content_type.contains("application/ISUP")
                    {
                        // Parse SIP-I direct ISUP body
                        match self.sipi_service.parse_sipi_body(body) {
                            Ok(isup_data) => {
                                match self.sipi_service.parse_isup_message(&isup_data) {
                                    Ok(isup_msg) => return Ok(Some(isup_msg)),
                                    Err(e) => debug!("Failed to parse ISUP from SIP-I: {}", e),
                                }
                            }
                            Err(e) => debug!("Failed to parse SIP-I body: {}", e),
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    async fn add_isup_to_sip(
        &self,
        sip_message: &str,
        isup_message: &IsupMessage,
    ) -> Result<String> {
        let isup_data = self.sipi_service.create_isup_message(isup_message)?;

        // Replace or add ISUP content based on service configuration
        let mut modified_message = sip_message.to_string();

        if self.sipi_service.is_sipi_enabled() {
            // SIP-I: Direct ISUP encapsulation
            let isup_body = self.sipi_service.create_sipi_body(&isup_data)?;

            // Update Content-Type and Content-Length
            modified_message = self.replace_header(
                &modified_message,
                "Content-Type",
                "application/ISUP; version=itu-t92+",
            )?;
            modified_message = self.replace_header(
                &modified_message,
                "Content-Length",
                &isup_body.len().to_string(),
            )?;

            // Replace body
            if let Some(body_start) = modified_message.find("\r\n\r\n") {
                modified_message.truncate(body_start + 4);
                modified_message.push_str(&isup_body);
            }
        } else if self.sipi_service.is_sipt_enabled() {
            // SIP-T: Multipart MIME with ISUP
            let existing_sdp = self.extract_sdp_from_sip(sip_message);
            let sipt_body = self
                .sipi_service
                .create_sipt_body(&isup_data, existing_sdp.as_deref())?;

            // Update Content-Type and Content-Length for multipart
            let content_type = sipt_body
                .lines()
                .next()
                .unwrap_or("")
                .trim_start_matches("Content-Type: ");
            modified_message =
                self.replace_header(&modified_message, "Content-Type", content_type)?;
            modified_message = self.replace_header(
                &modified_message,
                "Content-Length",
                &sipt_body.len().to_string(),
            )?;

            // Replace body
            if let Some(body_start) = modified_message.find("\r\n\r\n") {
                modified_message.truncate(body_start + 4);
                modified_message.push_str(&sipt_body);
            }
        }

        Ok(modified_message)
    }

    async fn modify_isup_for_termination(
        &self,
        mut isup: IsupMessage,
        _from: &str,
        _to: &str,
        new_cic: u16,
    ) -> Result<IsupMessage> {
        // Update CIC for new circuit
        isup.cic = new_cic;

        // Modify parameters as needed for termination
        // In a real implementation, this would:
        // - Update point codes
        // - Modify routing labels
        // - Update calling/called party numbers
        // - Add carrier-specific parameters

        Ok(isup)
    }

    async fn create_isup_rel_from_bye(
        &self,
        _bye_message: &str,
        session: &SipICallSession,
    ) -> Result<IsupMessage> {
        let cic = session.b_leg.cic.unwrap_or(0);

        let mut rel_message = IsupMessage {
            cic,
            message_type: IsupMessageType::REL,
            mandatory_fixed: vec![0x10], // Cause: Normal call clearing
            mandatory_variable: Vec::new(),
            optional: Vec::new(),
            raw_data: Vec::new(),
        };

        // Add cause indicators as optional parameter
        rel_message.optional.push(IsupParameter {
            param_type: IsupParameterType::CauseIndicators,
            length: 2,
            data: vec![0x80, 0x90], // Normal call clearing
        });

        Ok(rel_message)
    }

    async fn create_isup_rel_from_cancel(
        &self,
        _cancel_message: &str,
        session: &SipICallSession,
    ) -> Result<IsupMessage> {
        let cic = session.b_leg.cic.unwrap_or(0);

        let mut rel_message = IsupMessage {
            cic,
            message_type: IsupMessageType::REL,
            mandatory_fixed: vec![0x15], // Cause: Call rejected
            mandatory_variable: Vec::new(),
            optional: Vec::new(),
            raw_data: Vec::new(),
        };

        // Add cause indicators for cancellation
        rel_message.optional.push(IsupParameter {
            param_type: IsupParameterType::CauseIndicators,
            length: 2,
            data: vec![0x80, 0x95], // Call rejected
        });

        Ok(rel_message)
    }

    async fn modify_response_for_origination(
        &self,
        message: &str,
        session: &SipICallSession,
    ) -> Result<String> {
        let mut modified = message.to_string();

        // Remove ISUP content if originating carrier doesn't support it
        if session.a_leg.carrier_type == CarrierType::SipNative {
            modified = self.remove_isup_from_sip(&modified).await?;
        }

        // Update Via header for response routing
        if let Ok(via) = self.extract_header(message, "Via") {
            let local_addr = self
                .socket
                .local_addr()
                .map_err(|e| anyhow!("Failed to get local address: {}", e))?;
            let modified_via = format!("Via: SIP/2.0/UDP {}", local_addr);
            modified = modified.replace(&format!("Via: {}", via), &modified_via);
        }

        // Add SIP-I related headers
        if session.requires_isup {
            modified = modified.replace(
                "\r\n\r\n",
                &format!(
                    "\r\nX-SIP-I-CIC: {}\r\nX-SIP-I-Processing: enabled\r\n\r\n",
                    session.b_leg.cic.unwrap_or(0)
                ),
            );
        }

        Ok(modified)
    }

    async fn remove_isup_from_sip(&self, message: &str) -> Result<String> {
        let mut modified = message.to_string();

        // Check if message has ISUP content
        if let Ok(content_type) = self.extract_header(message, "Content-Type") {
            if content_type.contains("application/ISUP") || content_type.contains("multipart/mixed")
            {
                // Convert to standard SIP with SDP only
                if let Some(sdp) = self.extract_sdp_from_sip(message) {
                    modified = self.replace_header(&modified, "Content-Type", "application/sdp")?;
                    modified =
                        self.replace_header(&modified, "Content-Length", &sdp.len().to_string())?;

                    // Replace body with SDP only
                    if let Some(body_start) = modified.find("\r\n\r\n") {
                        modified.truncate(body_start + 4);
                        modified.push_str(&sdp);
                    }
                } else {
                    // No SDP, remove body entirely
                    modified = self.replace_header(&modified, "Content-Length", "0")?;
                    if let Some(body_start) = modified.find("\r\n\r\n") {
                        modified.truncate(body_start + 4);
                    }
                }
            }
        }

        Ok(modified)
    }

    fn extract_sdp_from_sip(&self, message: &str) -> Option<String> {
        if let Some(body_start) = message.find("\r\n\r\n") {
            let body = &message[body_start + 4..];

            // If it's already SDP, return it
            if body.starts_with("v=") {
                return Some(body.to_string());
            }

            // If it's SIP-T multipart, extract SDP part
            if body.contains("application/sdp") {
                // Simple SDP extraction from multipart
                if let Some(sdp_start) = body.find("v=") {
                    if let Some(sdp_end) = body[sdp_start..].find("\r\n--") {
                        return Some(body[sdp_start..sdp_start + sdp_end].to_string());
                    } else {
                        return Some(body[sdp_start..].to_string());
                    }
                }
            }
        }

        None
    }

    async fn allocate_cic(&self) -> Result<u16> {
        let used_cics = self.used_cics.read().await;
        let config = self.sipi_service.get_config();

        if let Some(cic) = utils::get_next_cic(config, &used_cics) {
            drop(used_cics);
            let mut used_cics = self.used_cics.write().await;
            used_cics.push(cic);
            Ok(cic)
        } else {
            Err(anyhow!("No available CICs in range"))
        }
    }

    async fn release_cic(&self, cic: u16) {
        let mut used_cics = self.used_cics.write().await;
        used_cics.retain(|&x| x != cic);
        debug!("Released CIC: {}", cic);
    }

    // Standard SIP helper methods
    fn extract_header(&self, message: &str, header_name: &str) -> Result<String> {
        // SECURITY: Validate header name format (Fixes header injection)
        crate::security_utils::validate_header(header_name, "")?;

        for line in message.lines() {
            // SECURITY: Limit line length to prevent DoS
            if line.len() > crate::security_utils::MAX_HEADER_LENGTH {
                warn!("Oversized header line detected, skipping");
                continue;
            }

            let lower_line = line.to_lowercase();
            let header_prefix = format!("{}:", header_name.to_lowercase());
            if lower_line.starts_with(&header_prefix) {
                // Find the first colon and take everything after it
                if let Some(colon_pos) = line.find(':') {
                    let header_value = line[(colon_pos + 1)..].trim().to_string();

                    // SECURITY: Validate header content
                    crate::security_utils::validate_header(header_name, &header_value)?;

                    return Ok(header_value);
                }
            }
        }
        Err(anyhow!(
            "Header {} not found",
            crate::security_utils::sanitize_for_logging(header_name)
        ))
    }

    fn extract_phone_number_from_header(&self, message: &str, header_name: &str) -> Result<String> {
        let header_value = self.extract_header(message, header_name)?;
        debug!(
            "Extracting phone number from {} header: '{}'",
            header_name, header_value
        );

        // Extract number from SIP URI with secure bounds checking (Fixes CVE-2024-004)
        if let Some(start) = header_value.find("sip:") {
            let sip_uri = &header_value[start..];
            debug!(
                "Found SIP URI: '{}'",
                crate::security_utils::sanitize_for_logging(sip_uri)
            );

            if let Some(end) = sip_uri.find('@') {
                // SECURITY: Safe bounds checking to prevent buffer overflow
                if sip_uri.len() >= 4 && end > 4 {
                    let number_part = crate::security_utils::safe_slice(sip_uri, 4, end)?; // Remove "sip:" prefix securely
                    debug!(
                        "Extracted number part: '{}'",
                        crate::security_utils::sanitize_for_logging(number_part)
                    );

                    // Validate extracted phone number
                    let validated_number =
                        crate::security_utils::validate_phone_number(number_part)?;
                    return Ok(validated_number.trim_start_matches('+').to_string());
                } else {
                    return Err(anyhow!(
                        "Invalid SIP URI format - insufficient length or invalid @ position"
                    ));
                }
            }
        }

        Err(anyhow!(
            "Could not extract phone number from {} header: '{}'",
            header_name,
            header_value
        ))
    }

    fn extract_from_tag(&self, message: &str) -> Result<String> {
        let from_header = self.extract_header(message, "From")?;
        if let Some(tag_start) = from_header.find("tag=") {
            let tag_part = &from_header[tag_start + 4..];
            let tag = tag_part.split([';', ' ', '>']).next().unwrap_or("").trim();
            Ok(tag.to_string())
        } else {
            Err(anyhow!("No tag found in From header"))
        }
    }

    fn replace_header(&self, message: &str, header_name: &str, new_value: &str) -> Result<String> {
        let lines: Vec<&str> = message.lines().collect();
        let mut modified_lines = Vec::new();
        let mut header_found = false;

        for line in lines {
            if line
                .to_lowercase()
                .starts_with(&format!("{}:", header_name.to_lowercase()))
            {
                modified_lines.push(format!("{}: {}", header_name, new_value));
                header_found = true;
            } else {
                modified_lines.push(line.to_string());
            }
        }

        // If header wasn't found, add it before the first empty line (before body)
        if !header_found {
            for (i, line) in modified_lines.iter().enumerate() {
                if line.is_empty() {
                    modified_lines.insert(i, format!("{}: {}", header_name, new_value));
                    break;
                }
            }
        }

        Ok(modified_lines.join("\r\n"))
    }

    fn modify_invite_for_sip_termination(&self, message: &str) -> Result<String> {
        let mut modified = message.to_string();
        modified = modified.replace("\r\n\r\n", "\r\nX-SIP-I-Leg: termination\r\n\r\n");
        Ok(modified)
    }

    fn modify_ack_for_termination(
        &self,
        message: &str,
        _session: &SipICallSession,
    ) -> Result<String> {
        let mut modified = message.to_string();
        modified = modified.replace("\r\n\r\n", "\r\nX-SIP-I-Leg: termination\r\n\r\n");
        Ok(modified)
    }

    fn modify_bye_for_termination(
        &self,
        message: &str,
        _session: &SipICallSession,
    ) -> Result<String> {
        let mut modified = message.to_string();
        modified = modified.replace("\r\n\r\n", "\r\nX-SIP-I-Leg: termination\r\n\r\n");
        Ok(modified)
    }

    fn modify_bye_for_origination(
        &self,
        message: &str,
        _session: &SipICallSession,
    ) -> Result<String> {
        let mut modified = message.to_string();
        modified = modified.replace("\r\n\r\n", "\r\nX-SIP-I-Leg: origination\r\n\r\n");
        Ok(modified)
    }

    fn modify_cancel_for_termination(
        &self,
        message: &str,
        _session: &SipICallSession,
    ) -> Result<String> {
        let mut modified = message.to_string();
        modified = modified.replace("\r\n\r\n", "\r\nX-SIP-I-Leg: termination\r\n\r\n");
        Ok(modified)
    }

    fn create_100_trying(&self, invite_message: &str) -> Result<String> {
        let via = self.extract_header(invite_message, "Via")?;
        let from = self.extract_header(invite_message, "From")?;
        let to = self.extract_header(invite_message, "To")?;
        let call_id = self.extract_header(invite_message, "Call-ID")?;
        let cseq = self.extract_header(invite_message, "CSeq")?;

        Ok(format!(
            "SIP/2.0 100 Trying\r\n\
            Via: {}\r\n\
            From: {}\r\n\
            To: {}\r\n\
            Call-ID: {}\r\n\
            CSeq: {}\r\n\
            Server: SIP-I-B2BUA/1.0\r\n\
            Content-Length: 0\r\n\
            \r\n",
            via, from, to, call_id, cseq
        ))
    }

    fn create_options_response(&self, options_message: &str) -> Result<String> {
        let via = self.extract_header(options_message, "Via")?;
        let from = self.extract_header(options_message, "From")?;
        let to = self.extract_header(options_message, "To")?;
        let call_id = self.extract_header(options_message, "Call-ID")?;
        let cseq = self.extract_header(options_message, "CSeq")?;

        let to_with_tag = if to.contains("tag=") {
            to
        } else {
            format!("{};tag=sipi-{}", to, chrono::Utc::now().timestamp())
        };

        Ok(format!(
            "SIP/2.0 200 OK\r\n\
            Via: {}\r\n\
            From: {}\r\n\
            To: {}\r\n\
            Call-ID: {}\r\n\
            CSeq: {}\r\n\
            Contact: <sip:sipi@{}>\r\n\
            Server: SIP-I-B2BUA/1.0\r\n\
            Allow: INVITE, ACK, CANCEL, BYE, OPTIONS, PRACK, UPDATE\r\n\
            Supported: 100rel, timer, replaces\r\n\
            Accept: application/sdp, application/ISUP\r\n\
            X-SIP-I-Enabled: {}\r\n\
            X-SIP-T-Enabled: {}\r\n\
            Content-Length: 0\r\n\
            \r\n",
            via,
            from,
            to_with_tag,
            call_id,
            cseq,
            self.socket
                .local_addr()
                .map_err(|e| anyhow!("Failed to get local address: {}", e))?,
            self.sipi_service.is_sipi_enabled(),
            self.sipi_service.is_sipt_enabled()
        ))
    }

    fn create_bye_response(&self, bye_message: &str) -> Result<String> {
        let via = self.extract_header(bye_message, "Via")?;
        let from = self.extract_header(bye_message, "From")?;
        let to = self.extract_header(bye_message, "To")?;
        let call_id = self.extract_header(bye_message, "Call-ID")?;
        let cseq = self.extract_header(bye_message, "CSeq")?;

        Ok(format!(
            "SIP/2.0 200 OK\r\n\
            Via: {}\r\n\
            From: {}\r\n\
            To: {}\r\n\
            Call-ID: {}\r\n\
            CSeq: {}\r\n\
            Server: SIP-I-B2BUA/1.0\r\n\
            Content-Length: 0\r\n\
            \r\n",
            via, from, to, call_id, cseq
        ))
    }

    fn create_cancel_response(&self, cancel_message: &str) -> Result<String> {
        let via = self.extract_header(cancel_message, "Via")?;
        let from = self.extract_header(cancel_message, "From")?;
        let to = self.extract_header(cancel_message, "To")?;
        let call_id = self.extract_header(cancel_message, "Call-ID")?;
        let cseq = self.extract_header(cancel_message, "CSeq")?;

        Ok(format!(
            "SIP/2.0 200 OK\r\n\
            Via: {}\r\n\
            From: {}\r\n\
            To: {}\r\n\
            Call-ID: {}\r\n\
            CSeq: {}\r\n\
            Server: SIP-I-B2BUA/1.0\r\n\
            Content-Length: 0\r\n\
            \r\n",
            via, from, to, call_id, cseq
        ))
    }

    async fn send_to(&self, data: &[u8], addr: SocketAddr) -> Result<()> {
        self.socket.send_to(data, addr).await?;
        Ok(())
    }

    // Statistics and management methods
    pub async fn get_active_calls(&self) -> usize {
        let calls = self.calls.read().await;
        calls.len()
    }

    pub async fn get_sipi_stats(&self) -> HashMap<String, usize> {
        let calls = self.calls.read().await;
        let mut stats = HashMap::new();

        for session in calls.values() {
            let key = format!("{:?}", session.a_leg.carrier_type);
            *stats.entry(key).or_insert(0) += 1;

            if session.requires_isup {
                *stats.entry("isup_calls".to_string()).or_insert(0) += 1;
            }
        }

        stats
    }

    pub async fn get_cic_usage(&self) -> (usize, usize) {
        let used_cics = self.used_cics.read().await;
        let config = self.sipi_service.get_config();
        let total_cics = (config.cic_range_end - config.cic_range_start + 1) as usize;
        (used_cics.len(), total_cics)
    }

    pub fn is_sipi_enabled(&self) -> bool {
        self.sipi_service.is_sipi_enabled()
    }

    pub fn is_sipt_enabled(&self) -> bool {
        self.sipi_service.is_sipt_enabled()
    }
}
