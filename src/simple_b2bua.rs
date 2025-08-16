/*
 * Simple B2BUA Implementation for Testing
 * This is a minimal working B2BUA to test basic SIP forwarding
 */

use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::{RwLock, mpsc};
use tracing::{info, warn, error, debug};

/// Simple call leg representation
#[derive(Debug, Clone)]
pub struct CallLeg {
    pub call_id: String,
    pub from_tag: String,
    pub to_tag: Option<String>,
    pub local_addr: SocketAddr,
    pub remote_addr: SocketAddr,
    pub state: CallState,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CallState {
    Initial,
    Proceeding,
    Connected,
    Disconnected,
}

/// Simple B2BUA that forwards SIP messages between two legs
pub struct SimpleB2BUA {
    socket: Arc<UdpSocket>,
    calls: Arc<RwLock<HashMap<String, (CallLeg, CallLeg)>>>,
    termination_host: String,
    termination_port: u16,
}

impl SimpleB2BUA {
    pub async fn new(bind_addr: SocketAddr, term_host: String, term_port: u16) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr).await?;
        info!("Simple B2BUA listening on {}", bind_addr);
        info!("Termination target: {}:{}", term_host, term_port);
        
        Ok(Self {
            socket: Arc::new(socket),
            calls: Arc::new(RwLock::new(HashMap::new())),
            termination_host: term_host,
            termination_port: term_port,
        })
    }

    pub async fn start(&self) -> Result<()> {
        let mut buffer = vec![0u8; 4096];
        
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
                           crate::security_utils::sanitize_for_logging(&message));
                    
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
        
        // Extract Call-ID
        let call_id = self.extract_header(message, "Call-ID")?;
        let from_tag = self.extract_from_tag(message)?;
        
        // Send 100 Trying
        let trying_response = self.create_100_trying(message)?;
        self.send_to(trying_response.as_bytes(), from).await?;
        
        // Create A-leg (origination side)
        let a_leg = CallLeg {
            call_id: call_id.clone(),
            from_tag: from_tag.clone(),
            to_tag: None,
            local_addr: self.socket.local_addr()?,
            remote_addr: from,
            state: CallState::Proceeding,
        };
        
        // Forward INVITE to termination
        let forwarded_invite = self.modify_invite_for_termination(message)?;
        let term_addr = format!("{}:{}", self.termination_host, self.termination_port)
            .parse::<SocketAddr>()?;
        
        // Create B-leg (termination side)  
        let b_leg = CallLeg {
            call_id: call_id.clone(),
            from_tag: from_tag,
            to_tag: None,
            local_addr: self.socket.local_addr()?,
            remote_addr: term_addr,
            state: CallState::Initial,
        };
        
        // Store call legs
        {
            let mut calls = self.calls.write().await;
            calls.insert(call_id, (a_leg, b_leg));
        }
        
        // Forward INVITE to termination
        self.send_to(forwarded_invite.as_bytes(), term_addr).await?;
        
        info!("INVITE forwarded to termination");
        Ok(())
    }

    async fn handle_options(&self, message: &str, from: SocketAddr) -> Result<()> {
        info!("Handling OPTIONS from {}", from);
        
        let response = self.create_options_response(message)?;
        self.send_to(response.as_bytes(), from).await?;
        
        info!("OPTIONS response sent");
        Ok(())
    }

    async fn handle_response(&self, message: &str, from: SocketAddr) -> Result<()> {
        info!("Handling SIP response from {}", from);
        
        // For now, just forward responses back to origination
        // In a full implementation, we'd track call legs and forward appropriately
        
        Ok(())
    }

    async fn handle_ack(&self, _message: &str, from: SocketAddr) -> Result<()> {
        info!("Handling ACK from {}", from);
        // Forward ACK to termination
        Ok(())
    }

    async fn handle_bye(&self, _message: &str, from: SocketAddr) -> Result<()> {
        info!("Handling BYE from {}", from);
        // Forward BYE and clean up call
        Ok(())
    }

    async fn handle_cancel(&self, _message: &str, from: SocketAddr) -> Result<()> {
        info!("Handling CANCEL from {}", from);
        // Forward CANCEL and clean up call
        Ok(())
    }

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
             Server: Simple-B2BUA/1.0\r\n\
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
             Server: Simple-B2BUA/1.0\r\n\
             Allow: INVITE, ACK, CANCEL, BYE, OPTIONS\r\n\
             Content-Length: 0\r\n\
             \r\n",
            via, from, to, chrono::Utc::now().timestamp(), call_id, cseq
        ))
    }

    fn modify_invite_for_termination(&self, invite: &str) -> Result<String> {
        // For now, just forward the INVITE as-is
        // In a full implementation, we'd modify headers appropriately
        Ok(invite.to_string())
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
}

/// Simple test function to run the B2BUA
pub async fn run_simple_b2bua() -> Result<()> {
    let bind_addr = "0.0.0.0:5060".parse()?;
    let b2bua = SimpleB2BUA::new(bind_addr, "127.0.0.1".to_string(), 5070).await?;
    
    info!("Starting Simple B2BUA for testing...");
    b2bua.start().await
}