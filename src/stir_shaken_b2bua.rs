/*
 * STIR/SHAKEN-enabled B2BUA Implementation
 * Adds RFC 8224/8225 compliance to the B2BUA for carrier-grade operation
 */

use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use chrono::{DateTime, Utc};

use crate::stir_shaken::{
    StirShakenService, StirShakenConfig, CallInfo, AttestationLevel, IngressPolicy
};
use crate::security_monitor::{SecurityMonitor, SecurityMonitorConfig, SecurityEventType};

/// Enhanced call leg with STIR/SHAKEN support
#[derive(Debug, Clone)]
pub struct StirShakenCallLeg {
    pub call_id: String,
    pub from_tag: String,
    pub to_tag: Option<String>,
    pub local_addr: SocketAddr,
    pub remote_addr: SocketAddr,
    pub state: CallState,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    // STIR/SHAKEN specific fields
    pub identity_header: Option<String>,
    pub attestation_level: Option<AttestationLevel>,
    pub verified: bool,
    pub from_number: Option<String>,
    pub to_number: Option<String>,
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

/// Call session with STIR/SHAKEN tracking
#[derive(Debug, Clone)]
pub struct StirShakenCallSession {
    pub call_id: String,
    pub a_leg: StirShakenCallLeg,
    pub b_leg: StirShakenCallLeg,
    pub state: CallState,
    pub created_at: DateTime<Utc>,
    // STIR/SHAKEN specific
    pub original_identity: Option<String>,
    pub generated_identity: Option<String>,
    pub ingress_trunk_uuid: Option<String>,
    pub egress_trunk_uuid: Option<String>,
}

/// STIR/SHAKEN enabled B2BUA with Security Monitoring
pub struct StirShakenB2BUA {
    socket: Arc<UdpSocket>,
    calls: Arc<RwLock<HashMap<String, StirShakenCallSession>>>,
    termination_host: String,
    termination_port: u16,
    addr_to_call: Arc<RwLock<HashMap<SocketAddr, String>>>,
    // STIR/SHAKEN service
    stir_shaken: Arc<StirShakenService>,
    trunk_group_id: String,
    // Security monitoring
    security_monitor: Arc<SecurityMonitor>,
}

impl StirShakenB2BUA {
    pub async fn new(
        bind_addr: SocketAddr, 
        term_host: String, 
        term_port: u16,
        stir_shaken_config: StirShakenConfig,
        trunk_group_id: String,
        security_config: Option<SecurityMonitorConfig>,
    ) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr).await?;
        info!("STIR/SHAKEN B2BUA listening on {}", bind_addr);
        info!("Termination target: {}:{}", term_host, term_port);
        
        // Initialize STIR/SHAKEN service
        let stir_shaken = Arc::new(StirShakenService::new(stir_shaken_config).await?);
        info!("STIR/SHAKEN service initialized for B2BUA");
        
        // Initialize security monitoring
        let security_monitor = Arc::new(SecurityMonitor::new(
            security_config.unwrap_or_default()
        ));
        security_monitor.start_cleanup_task().await;
        info!("Security monitoring initialized for STIR/SHAKEN B2BUA");
        
        Ok(Self {
            socket: Arc::new(socket),
            calls: Arc::new(RwLock::new(HashMap::new())),
            termination_host: term_host,
            termination_port: term_port,
            addr_to_call: Arc::new(RwLock::new(HashMap::new())),
            stir_shaken,
            trunk_group_id,
            security_monitor,
        })
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting STIR/SHAKEN B2BUA with response forwarding...");
        let mut buffer = vec![0u8; 8192]; // Larger buffer for STIR/SHAKEN headers
        
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
                    
                    debug!("Received from {}: {}", from, 
                           crate::security_utils::sanitize_for_logging(
                               message.lines().next().unwrap_or("")));
                    
                    // SECURITY: Real-time threat analysis
                    if self.security_monitor.is_ip_blocked(from.ip()).await {
                        warn!("🚫 Blocked IP {} attempted connection", from.ip());
                        continue;
                    }
                    
                    // Analyze message for security threats
                    if let Ok(threats) = self.security_monitor.analyze_message(from.ip(), &message).await {
                        if !threats.is_empty() {
                            warn!("🛡️ Security threats detected from {}: {:?}", from.ip(), threats);
                            // Continue processing unless it's a critical threat that should block
                            if threats.contains(&SecurityEventType::JwtAlgorithmConfusion) ||
                               threats.contains(&SecurityEventType::BufferOverflowAttempt) {
                                warn!("🚨 Critical security threat detected, dropping message");
                                continue;
                            }
                        }
                    }
                    
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
        info!("Handling INVITE from {}", from);
        
        let call_id = self.extract_header(message, "Call-ID")?;
        let from_number = self.extract_phone_number_from_header(message, "From")?;
        let to_number = self.extract_phone_number_from_header(message, "To")?;
        
        // Extract existing Identity header if present
        let identity_header = self.extract_header(message, "Identity").ok();
        
        // Create call info for STIR/SHAKEN processing
        let mut call_info = CallInfo {
            from_number: from_number.clone(),
            to_number: to_number.clone(),
            call_id: call_id.clone(),
            attestation: AttestationLevel::Gateway, // Will be updated
            ingress_trunk_uuid: Some(uuid::Uuid::new_v4().to_string()),
            egress_trunk_uuid: Some(uuid::Uuid::new_v4().to_string()),
        };

        // Process ingress call with STIR/SHAKEN
        let new_identity: Option<String> = if self.stir_shaken.is_enabled() && 
            self.stir_shaken.should_enable_for_call(&from_number, &to_number) {
            
            match self.stir_shaken.process_ingress_call(
                &self.trunk_group_id,
                identity_header.as_deref(),
                &from_number,
                &mut call_info
            ).await {
                Ok(new_identity) => {
                    info!("STIR/SHAKEN ingress processing successful for call {}", call_id);
                    new_identity
                }
                Err(e) => {
                    warn!("STIR/SHAKEN ingress processing failed: {}", e);
                    // Continue with call processing even if STIR/SHAKEN fails
                    None
                }
            }
        } else {
            None
        };

        // Send 100 Trying immediately
        let trying_response = self.create_100_trying(message)?;
        self.send_to(trying_response.as_bytes(), from).await?;

        // Create modified INVITE for termination
        let mut modified_invite = self.modify_invite_for_termination(message, &call_info)?;
        
        // Add new Identity header if generated
        if let Some(identity) = &new_identity {
            modified_invite = self.add_identity_header(&modified_invite, identity)?;
            info!("Added STIR/SHAKEN Identity header to call {}", call_id);
        }

        // Forward to termination
        let termination_addr = format!("{}:{}", self.termination_host, self.termination_port);
        let termination_socket: SocketAddr = termination_addr.parse()?;
        
        self.send_to(modified_invite.as_bytes(), termination_socket).await?;
        info!("INVITE forwarded to termination for call {}", call_id);

        // Create call session with STIR/SHAKEN data
        let a_leg = StirShakenCallLeg {
            call_id: call_id.clone(),
            from_tag: self.extract_from_tag(message)?,
            to_tag: None,
            local_addr: self.socket.local_addr()?,
            remote_addr: from,
            state: CallState::Initial,
            created_at: Utc::now(),
            last_activity: Utc::now(),
            identity_header: identity_header.clone(),
            attestation_level: Some(call_info.attestation.clone()),
            verified: identity_header.is_some(),
            from_number: Some(from_number.clone()),
            to_number: Some(to_number.clone()),
        };

        let b_leg = StirShakenCallLeg {
            call_id: call_id.clone(),
            from_tag: self.extract_from_tag(&modified_invite)?,
            to_tag: None,
            local_addr: self.socket.local_addr()?,
            remote_addr: termination_socket,
            state: CallState::Initial,
            created_at: Utc::now(),
            last_activity: Utc::now(),
            identity_header: new_identity.clone(),
            attestation_level: Some(call_info.attestation.clone()),
            verified: false,
            from_number: Some(from_number),
            to_number: Some(to_number),
        };

        let session = StirShakenCallSession {
            call_id: call_id.clone(),
            a_leg,
            b_leg,
            state: CallState::Proceeding,
            created_at: Utc::now(),
            original_identity: identity_header,
            generated_identity: new_identity,
            ingress_trunk_uuid: call_info.ingress_trunk_uuid,
            egress_trunk_uuid: call_info.egress_trunk_uuid,
        };

        // Store session
        {
            let mut calls = self.calls.write().await;
            calls.insert(call_id.clone(), session);
        }

        // Map termination address to call ID for response routing
        {
            let mut addr_map = self.addr_to_call.write().await;
            addr_map.insert(termination_socket, call_id);
        }

        Ok(())
    }

    async fn handle_response(&self, message: &str, from: SocketAddr) -> Result<()> {
        debug!("Handling response from {}", from);

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
                // Update call state based on response
                if message.contains("SIP/2.0 18") {
                    session.state = CallState::Ringing;
                } else if message.contains("SIP/2.0 200") {
                    session.state = CallState::Connected;
                } else if message.contains("SIP/2.0 4") || message.contains("SIP/2.0 5") || message.contains("SIP/2.0 6") {
                    session.state = CallState::Disconnected;
                }

                // Modify response for origination leg
                let modified_response = self.modify_response_for_origination(message, &session)?;
                
                // Send response to A-leg
                self.send_to(modified_response.as_bytes(), session.a_leg.remote_addr).await?;
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
            self.send_to(modified_ack.as_bytes(), session.b_leg.remote_addr).await?;
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
            
            // Forward BYE to other leg
            if from == session.a_leg.remote_addr {
                // BYE from A-leg, forward to B-leg
                let modified_bye = self.modify_bye_for_termination(message, &session)?;
                self.send_to(modified_bye.as_bytes(), session.b_leg.remote_addr).await?;
            } else if from == session.b_leg.remote_addr {
                // BYE from B-leg, forward to A-leg
                let modified_bye = self.modify_bye_for_origination(message, &session)?;
                self.send_to(modified_bye.as_bytes(), session.a_leg.remote_addr).await?;
            }

            // Send 200 OK to sender
            let bye_response = self.create_bye_response(message)?;
            self.send_to(bye_response.as_bytes(), from).await?;
            
            // Clean up session
            {
                let mut calls = self.calls.write().await;
                calls.remove(&call_id);
            }
            
            info!("Call {} terminated", call_id);
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
            let modified_cancel = self.modify_cancel_for_termination(message, &session)?;
            self.send_to(modified_cancel.as_bytes(), session.b_leg.remote_addr).await?;
            
            // Send 200 OK to CANCEL
            let cancel_response = self.create_cancel_response(message)?;
            self.send_to(cancel_response.as_bytes(), from).await?;
            
            info!("CANCEL forwarded for call {}", call_id);
        }

        Ok(())
    }

    // Helper methods for message modification and processing
    fn extract_header(&self, message: &str, header_name: &str) -> Result<String> {
        for line in message.lines() {
            if line.to_lowercase().starts_with(&format!("{}:", header_name.to_lowercase())) {
                return Ok(line.split(':').nth(1).unwrap_or("").trim().to_string());
            }
        }
        Err(anyhow!("Header {} not found", header_name))
    }

    fn extract_phone_number_from_header(&self, message: &str, header_name: &str) -> Result<String> {
        let header_value = self.extract_header(message, header_name)?;
        
        // Extract number from SIP URI with secure bounds checking (Fixes CVE-2024-004)
        if let Some(start) = header_value.find("sip:") {
            let sip_uri = &header_value[start..];
            if let Some(end) = sip_uri.find('@') {
                // SECURITY: Safe bounds checking to prevent buffer overflow
                if sip_uri.len() >= 4 && end > 4 {
                    let number_part = &sip_uri[4..end]; // Remove "sip:" prefix - bounds validated
                    if let Some(cleaned) = self.stir_shaken.extract_phone_number(&format!("sip:{}@example.com", number_part)) {
                        return Ok(format!("+{}", cleaned)); // Add + prefix for E.164
                    }
                } else {
                    return Err(anyhow!("Invalid SIP URI format - insufficient length or invalid @ position"));
                }
            }
        }
        
        Err(anyhow!("Could not extract phone number from {} header", header_name))
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

    fn add_identity_header(&self, message: &str, identity: &str) -> Result<String> {
        let mut lines: Vec<&str> = message.lines().collect();
        
        // Find insertion point (after CSeq header, before body)
        let mut insert_pos = lines.len();
        for (i, line) in lines.iter().enumerate() {
            if line.to_lowercase().starts_with("cseq:") {
                insert_pos = i + 1;
                break;
            }
        }
        
        // Create the Identity header string
        let identity_header = format!("Identity: {}", identity);
        
        // Insert Identity header
        lines.insert(insert_pos, &identity_header);
        
        Ok(lines.join("\r\n"))
    }

    fn modify_invite_for_termination(&self, message: &str, call_info: &CallInfo) -> Result<String> {
        let mut modified = message.to_string();
        
        // Add X-B2BUA-Leg header to identify this as termination leg
        modified = modified.replace(
            "\r\n\r\n",
            &format!("\r\nX-B2BUA-Leg: termination\r\nX-Ingress-Trunk-UUID: {}\r\nX-Egress-Trunk-UUID: {}\r\n\r\n", 
                call_info.ingress_trunk_uuid.as_ref().unwrap_or(&"unknown".to_string()),
                call_info.egress_trunk_uuid.as_ref().unwrap_or(&"unknown".to_string())
            )
        );
        
        Ok(modified)
    }

    fn modify_response_for_origination(&self, message: &str, session: &StirShakenCallSession) -> Result<String> {
        let mut modified = message.to_string();
        
        // Update Via header for response routing
        if let Ok(via) = self.extract_header(message, "Via") {
            let local_addr = self.socket.local_addr()
                .map_err(|e| anyhow!("Failed to get local address: {}", e))?;
            let modified_via = format!("Via: SIP/2.0/UDP {}", local_addr);
            modified = modified.replace(&format!("Via: {}", via), &modified_via);
        }
        
        // Add STIR/SHAKEN related headers to responses
        if message.contains("SIP/2.0 200") {
            modified = modified.replace(
                "\r\n\r\n",
                &format!("\r\nX-STIR-SHAKEN-Verified: {}\r\nX-Attestation-Level: {:?}\r\n\r\n",
                    session.a_leg.verified,
                    session.a_leg.attestation_level.as_ref().unwrap_or(&AttestationLevel::Gateway)
                )
            );
        }
        
        Ok(modified)
    }

    fn modify_ack_for_termination(&self, message: &str, _session: &StirShakenCallSession) -> Result<String> {
        let mut modified = message.to_string();
        modified = modified.replace("\r\n\r\n", "\r\nX-B2BUA-Leg: termination\r\n\r\n");
        Ok(modified)
    }

    fn modify_bye_for_termination(&self, message: &str, _session: &StirShakenCallSession) -> Result<String> {
        let mut modified = message.to_string();
        modified = modified.replace("\r\n\r\n", "\r\nX-B2BUA-Leg: termination\r\n\r\n");
        Ok(modified)
    }

    fn modify_bye_for_origination(&self, message: &str, _session: &StirShakenCallSession) -> Result<String> {
        let mut modified = message.to_string();
        modified = modified.replace("\r\n\r\n", "\r\nX-B2BUA-Leg: origination\r\n\r\n");
        Ok(modified)
    }

    fn modify_cancel_for_termination(&self, message: &str, _session: &StirShakenCallSession) -> Result<String> {
        let mut modified = message.to_string();
        modified = modified.replace("\r\n\r\n", "\r\nX-B2BUA-Leg: termination\r\n\r\n");
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
            Server: STIR-SHAKEN-B2BUA/1.0\r\n\
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
            format!("{};tag=b2bua-{}", to, chrono::Utc::now().timestamp())
        };

        Ok(format!(
            "SIP/2.0 200 OK\r\n\
            Via: {}\r\n\
            From: {}\r\n\
            To: {}\r\n\
            Call-ID: {}\r\n\
            CSeq: {}\r\n\
            Contact: <sip:b2bua@{}>\r\n\
            Server: STIR-SHAKEN-B2BUA/1.0\r\n\
            Allow: INVITE, ACK, CANCEL, BYE, OPTIONS, PRACK, UPDATE\r\n\
            Supported: 100rel, timer, replaces, stir\r\n\
            Accept: application/sdp\r\n\
            X-STIR-SHAKEN-Enabled: {}\r\n\
            Content-Length: 0\r\n\
            \r\n",
            via, from, to_with_tag, call_id, cseq,
            self.socket.local_addr()
                .map_err(|e| anyhow!("Failed to get local address: {}", e))?,
            self.stir_shaken.is_enabled()
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
            Server: STIR-SHAKEN-B2BUA/1.0\r\n\
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
            Server: STIR-SHAKEN-B2BUA/1.0\r\n\
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

    pub async fn get_stir_shaken_stats(&self) -> HashMap<String, usize> {
        let calls = self.calls.read().await;
        let mut stats = HashMap::new();
        
        for session in calls.values() {
            if let Some(attestation) = &session.a_leg.attestation_level {
                let key = format!("{:?}", attestation);
                *stats.entry(key).or_insert(0) += 1;
            }
        }
        
        stats
    }

    pub async fn get_verified_calls_count(&self) -> usize {
        let calls = self.calls.read().await;
        calls.values().filter(|s| s.a_leg.verified).count()
    }

    pub fn is_stir_shaken_enabled(&self) -> bool {
        self.stir_shaken.is_enabled()
    }

    /// Get comprehensive security statistics
    pub async fn get_security_stats(&self) -> Result<crate::security_monitor::SecurityStats> {
        self.security_monitor.get_security_stats().await
    }

    /// Check if an IP address is currently blocked
    pub async fn is_ip_blocked(&self, ip: std::net::IpAddr) -> bool {
        self.security_monitor.is_ip_blocked(ip).await
    }

    /// Manually record a security event (for integration with external systems)
    pub async fn record_security_event(
        &self,
        event_type: SecurityEventType,
        source_ip: std::net::IpAddr,
        details: String,
    ) -> Result<()> {
        self.security_monitor.record_security_event(
            event_type,
            source_ip,
            details,
            None,
        ).await
    }
}