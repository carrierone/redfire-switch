/*
 * Improved B2BUA Implementation - Iteration 2
 * Adds response forwarding and proper call state management
 */

use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use chrono::{DateTime, Utc};

/// Enhanced call leg with response tracking
#[derive(Debug, Clone)]
pub struct CallLeg {
    pub call_id: String,
    pub from_tag: String,
    pub to_tag: Option<String>,
    pub local_addr: SocketAddr,
    pub remote_addr: SocketAddr,
    pub state: CallState,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
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

/// Call session tracking both legs
#[derive(Debug, Clone)]
pub struct CallSession {
    pub call_id: String,
    pub a_leg: CallLeg,
    pub b_leg: CallLeg,
    pub state: CallState,
    pub created_at: DateTime<Utc>,
}

/// Improved B2BUA with response forwarding
pub struct ImprovedB2BUA {
    socket: Arc<UdpSocket>,
    calls: Arc<RwLock<HashMap<String, CallSession>>>,
    termination_host: String,
    termination_port: u16,
    // Map remote addresses to call IDs for response routing
    addr_to_call: Arc<RwLock<HashMap<SocketAddr, String>>>,
}

impl ImprovedB2BUA {
    pub async fn new(bind_addr: SocketAddr, term_host: String, term_port: u16) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr).await?;
        info!("Improved B2BUA listening on {}", bind_addr);
        info!("Termination target: {}:{}", term_host, term_port);
        
        Ok(Self {
            socket: Arc::new(socket),
            calls: Arc::new(RwLock::new(HashMap::new())),
            termination_host: term_host,
            termination_port: term_port,
            addr_to_call: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub async fn start(&self) -> Result<()> {
        let mut buffer = vec![0u8; 4096];
        
        loop {
            match self.socket.recv_from(&mut buffer).await {
                Ok((len, from)) => {
                    let message = String::from_utf8_lossy(&buffer[..len]);
                    debug!("Received from {}: {}", from, message);
                    
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
        let from_tag = self.extract_from_tag(message)?;
        
        // Send 100 Trying immediately
        let trying_response = self.create_100_trying(message)?;
        self.send_to(trying_response.as_bytes(), from).await?;
        
        // Create termination address
        let term_addr = format!("{}:{}", self.termination_host, self.termination_port)
            .parse::<SocketAddr>()?;
        
        // Create call session
        let now = Utc::now();
        let a_leg = CallLeg {
            call_id: call_id.clone(),
            from_tag: from_tag.clone(),
            to_tag: None,
            local_addr: self.socket.local_addr()?,
            remote_addr: from,
            state: CallState::Proceeding,
            created_at: now,
            last_activity: now,
        };
        
        let b_leg = CallLeg {
            call_id: call_id.clone(),
            from_tag: format!("b2bua-{}", chrono::Utc::now().timestamp()),
            to_tag: None,
            local_addr: self.socket.local_addr()?,
            remote_addr: term_addr,
            state: CallState::Initial,
            created_at: now,
            last_activity: now,
        };
        
        let session = CallSession {
            call_id: call_id.clone(),
            a_leg,
            b_leg,
            state: CallState::Proceeding,
            created_at: now,
        };
        
        // Store call session
        {
            let mut calls = self.calls.write().await;
            calls.insert(call_id.clone(), session);
        }
        
        // Map termination address to call ID for response routing
        {
            let mut addr_map = self.addr_to_call.write().await;
            addr_map.insert(term_addr, call_id.clone());
        }
        
        // Forward modified INVITE to termination
        let forwarded_invite = self.modify_invite_for_termination(message, &call_id)?;
        self.send_to(forwarded_invite.as_bytes(), term_addr).await?;
        
        info!("INVITE forwarded to termination for call {}", call_id);
        Ok(())
    }

    async fn handle_response(&self, message: &str, from: SocketAddr) -> Result<()> {
        info!("Handling SIP response from {}", from);
        
        // Find the call this response belongs to
        let call_id = {
            let addr_map = self.addr_to_call.read().await;
            addr_map.get(&from).cloned()
        };
        
        if let Some(call_id) = call_id {
            // Get call session
            let session = {
                let calls = self.calls.read().await;
                calls.get(&call_id).cloned()
            };
            
            if let Some(session) = session {
                // Extract response code
                let response_code = self.extract_response_code(message)?;
                info!("Forwarding {} response for call {}", response_code, call_id);
                
                // Modify response for origination and forward
                let modified_response = self.modify_response_for_origination(message, &session)?;
                self.send_to(modified_response.as_bytes(), session.a_leg.remote_addr).await?;
                
                // Update call state based on response
                self.update_call_state(&call_id, response_code).await?;
                
                info!("Response {} forwarded to origination for call {}", response_code, call_id);
            } else {
                warn!("Received response for unknown call from {}", from);
            }
        } else {
            warn!("Received response from unmapped address {}", from);
        }
        
        Ok(())
    }

    async fn handle_ack(&self, message: &str, from: SocketAddr) -> Result<()> {
        info!("Handling ACK from {}", from);
        
        let call_id = self.extract_header(message, "Call-ID")?;
        
        // Get call session and forward ACK to termination
        let session = {
            let calls = self.calls.read().await;
            calls.get(&call_id).cloned()
        };
        
        if let Some(session) = session {
            let modified_ack = self.modify_ack_for_termination(message, &session)?;
            self.send_to(modified_ack.as_bytes(), session.b_leg.remote_addr).await?;
            
            // Update call state to connected
            {
                let mut calls = self.calls.write().await;
                if let Some(call) = calls.get_mut(&call_id) {
                    call.state = CallState::Connected;
                    call.a_leg.state = CallState::Connected;
                    call.b_leg.state = CallState::Connected;
                }
            }
            
            info!("ACK forwarded to termination for call {}", call_id);
        }
        
        Ok(())
    }

    async fn handle_bye(&self, message: &str, from: SocketAddr) -> Result<()> {
        info!("Handling BYE from {}", from);
        
        let call_id = self.extract_header(message, "Call-ID")?;
        
        // Get call session and forward BYE
        let session = {
            let calls = self.calls.read().await;
            calls.get(&call_id).cloned()
        };
        
        if let Some(session) = session {
            // Determine if BYE is from A-leg or B-leg
            if from == session.a_leg.remote_addr {
                // BYE from origination, forward to termination
                let modified_bye = self.modify_bye_for_termination(message, &session)?;
                self.send_to(modified_bye.as_bytes(), session.b_leg.remote_addr).await?;
                info!("BYE forwarded from A-leg to B-leg for call {}", call_id);
            } else if from == session.b_leg.remote_addr {
                // BYE from termination, forward to origination
                let modified_bye = self.modify_bye_for_origination(message, &session)?;
                self.send_to(modified_bye.as_bytes(), session.a_leg.remote_addr).await?;
                info!("BYE forwarded from B-leg to A-leg for call {}", call_id);
            }
            
            // Update call state
            {
                let mut calls = self.calls.write().await;
                if let Some(call) = calls.get_mut(&call_id) {
                    call.state = CallState::Disconnecting;
                }
            }
        }
        
        Ok(())
    }

    async fn handle_cancel(&self, message: &str, from: SocketAddr) -> Result<()> {
        info!("Handling CANCEL from {}", from);
        
        let call_id = self.extract_header(message, "Call-ID")?;
        
        // Forward CANCEL to termination and clean up call
        let session = {
            let calls = self.calls.read().await;
            calls.get(&call_id).cloned()
        };
        
        if let Some(session) = session {
            let modified_cancel = self.modify_cancel_for_termination(message, &session)?;
            self.send_to(modified_cancel.as_bytes(), session.b_leg.remote_addr).await?;
            
            // Clean up call
            self.cleanup_call(&call_id).await;
            info!("CANCEL forwarded and call {} cleaned up", call_id);
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

    async fn update_call_state(&self, call_id: &str, response_code: u16) -> Result<()> {
        let mut calls = self.calls.write().await;
        if let Some(call) = calls.get_mut(call_id) {
            match response_code {
                180..=183 => {
                    call.state = CallState::Ringing;
                    call.b_leg.state = CallState::Ringing;
                }
                200 => {
                    call.state = CallState::Connected;
                    call.b_leg.state = CallState::Connected;
                }
                400..=699 => {
                    call.state = CallState::Disconnected;
                    // Will be cleaned up later
                }
                _ => {}
            }
            call.a_leg.last_activity = Utc::now();
            call.b_leg.last_activity = Utc::now();
        }
        Ok(())
    }

    async fn cleanup_call(&self, call_id: &str) {
        let mut calls = self.calls.write().await;
        if let Some(session) = calls.remove(call_id) {
            // Remove address mapping
            let mut addr_map = self.addr_to_call.write().await;
            addr_map.remove(&session.b_leg.remote_addr);
            info!("Cleaned up call session {}", call_id);
        }
    }

    // Helper methods for message modification
    fn modify_invite_for_termination(&self, invite: &str, call_id: &str) -> Result<String> {
        // For now, modify minimal headers - in full implementation would modify Via, Contact, etc.
        let mut modified = invite.to_string();
        
        // Add custom header to track B2BUA processing
        modified = modified.replace(
            &format!("Call-ID: {}", call_id),
            &format!("Call-ID: {}\r\nX-B2BUA-Leg: termination", call_id)
        );
        
        Ok(modified)
    }

    fn modify_response_for_origination(&self, response: &str, session: &CallSession) -> Result<String> {
        // Modify response headers for origination
        let mut modified = response.to_string();
        
        // Update Via header (remove our entry)
        // Update Contact header if present
        // Add proper To-tag if 200 OK
        
        if response.contains("SIP/2.0 200") {
            // Add To-tag for 200 OK
            let to_line = self.extract_header(response, "To")?;
            if !to_line.contains("tag=") {
                let new_to = format!("{};tag=b2bua-{}", to_line, session.created_at.timestamp());
                modified = modified.replace(
                    &format!("To: {}", to_line),
                    &format!("To: {}", new_to)
                );
            }
        }
        
        Ok(modified)
    }

    fn modify_ack_for_termination(&self, ack: &str, _session: &CallSession) -> Result<String> {
        Ok(ack.to_string()) // Minimal modification for now
    }

    fn modify_bye_for_termination(&self, bye: &str, _session: &CallSession) -> Result<String> {
        Ok(bye.to_string()) // Minimal modification for now
    }

    fn modify_bye_for_origination(&self, bye: &str, _session: &CallSession) -> Result<String> {
        Ok(bye.to_string()) // Minimal modification for now
    }

    fn modify_cancel_for_termination(&self, cancel: &str, _session: &CallSession) -> Result<String> {
        Ok(cancel.to_string()) // Minimal modification for now
    }

    fn extract_response_code(&self, response: &str) -> Result<u16> {
        let first_line = response.lines().next().ok_or_else(|| anyhow!("Empty response"))?;
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() >= 2 {
            Ok(parts[1].parse()?)
        } else {
            Err(anyhow!("Invalid response format"))
        }
    }

    // Reuse existing helper methods from simple B2BUA
    fn extract_header(&self, message: &str, header_name: &str) -> Result<String> {
        for line in message.lines() {
            if line.to_lowercase().starts_with(&format!("{}:", header_name.to_lowercase())) {
                if let Some(value) = line.split(':').nth(1) {
                    return Ok(value.trim().to_string());
                }
            }
        }
        Err(anyhow!("Header {} not found", header_name))
    }

    fn extract_from_tag(&self, message: &str) -> Result<String> {
        for line in message.lines() {
            if line.to_lowercase().starts_with("from:") {
                if let Some(tag_part) = line.split("tag=").nth(1) {
                    if let Some(tag) = tag_part.split(';').next().or_else(|| tag_part.split('>').next()) {
                        return Ok(tag.trim().to_string());
                    }
                }
            }
        }
        Err(anyhow!("From tag not found"))
    }

    fn create_100_trying(&self, request: &str) -> Result<String> {
        let call_id = self.extract_header(request, "Call-ID")?;
        let via = self.extract_header(request, "Via")?;
        let from = self.extract_header(request, "From")?;
        let to = self.extract_header(request, "To")?;
        let cseq = self.extract_header(request, "CSeq")?;
        
        Ok(format!(
            "SIP/2.0 100 Trying\r\n\
             Via: {}\r\n\
             From: {}\r\n\
             To: {}\r\n\
             Call-ID: {}\r\n\
             CSeq: {}\r\n\
             Server: Improved-B2BUA/2.0\r\n\
             Content-Length: 0\r\n\
             \r\n",
            via, from, to, call_id, cseq
        ))
    }

    fn create_options_response(&self, request: &str) -> Result<String> {
        let call_id = self.extract_header(request, "Call-ID")?;
        let via = self.extract_header(request, "Via")?;
        let from = self.extract_header(request, "From")?;
        let to = self.extract_header(request, "To")?;
        let cseq = self.extract_header(request, "CSeq")?;
        
        Ok(format!(
            "SIP/2.0 200 OK\r\n\
             Via: {}\r\n\
             From: {}\r\n\
             To: {};tag=b2bua-{}\r\n\
             Call-ID: {}\r\n\
             CSeq: {}\r\n\
             Server: Improved-B2BUA/2.0\r\n\
             Allow: INVITE, ACK, CANCEL, BYE, OPTIONS\r\n\
             Accept: application/sdp\r\n\
             Content-Length: 0\r\n\
             \r\n",
            via, from, to, chrono::Utc::now().timestamp(), call_id, cseq
        ))
    }

    async fn send_to(&self, data: &[u8], addr: SocketAddr) -> Result<()> {
        match self.socket.send_to(data, addr).await {
            Ok(_) => {
                debug!("Sent {} bytes to {}", data.len(), addr);
                Ok(())
            }
            Err(e) => {
                error!("Failed to send to {}: {}", addr, e);
                Err(e.into())
            }
        }
    }

    // Statistics and monitoring
    pub async fn get_active_calls(&self) -> usize {
        let calls = self.calls.read().await;
        calls.len()
    }

    pub async fn get_call_stats(&self) -> HashMap<String, usize> {
        let calls = self.calls.read().await;
        let mut stats = HashMap::new();
        
        for (_, session) in calls.iter() {
            let state_name = format!("{:?}", session.state);
            *stats.entry(state_name).or_insert(0) += 1;
        }
        
        stats
    }
}

/// Run the improved B2BUA
pub async fn run_improved_b2bua() -> Result<()> {
    let bind_addr = "0.0.0.0:5060".parse()?;
    let b2bua = ImprovedB2BUA::new(bind_addr, "127.0.0.1".to_string(), 5070).await?;
    
    info!("Starting Improved B2BUA with response forwarding...");
    b2bua.start().await
}