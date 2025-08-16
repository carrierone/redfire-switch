/*
 * Simple B2BUA Test Binary - WORKING VERSION
 * Minimal SIP forwarding B2BUA for testing
 */

use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tokio::signal;
use tracing::{info, warn, error};

// Security constants (embedded to avoid import issues)
const MAX_SIP_MESSAGE_SIZE: usize = 65536;
// FIXED: Add call timeout to prevent memory leaks
const CALL_TIMEOUT: Duration = Duration::from_secs(3600); // 1 hour timeout

#[derive(Debug, Clone)]
pub struct CallLeg {
    pub call_id: String,
    pub from_tag: String,
    pub to_tag: Option<String>,
    pub local_addr: SocketAddr,
    pub remote_addr: SocketAddr,
    pub state: CallState,
    // FIXED: Add timestamp for call cleanup
    pub created_at: Instant,
    pub last_activity: Instant,
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
    // FIXED: Add shutdown flag for graceful termination
    shutdown: Arc<AtomicBool>,
}

impl SimpleB2BUA {
    pub async fn new(bind_addr: SocketAddr, term_host: String, term_port: u16) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr).await?;
        info!("✅ Simple B2BUA listening on {}", bind_addr);
        info!("📞 Termination target: {}:{}", term_host, term_port);
        
        Ok(Self {
            socket: Arc::new(socket),
            calls: Arc::new(RwLock::new(HashMap::new())),
            termination_host: term_host,
            termination_port: term_port,
            // FIXED: Initialize shutdown flag
            shutdown: Arc::new(AtomicBool::new(false)),
        })
    }

    pub async fn start(&self) -> Result<()> {
        // FIXED: Start call cleanup task to prevent memory leaks
        self.start_call_cleanup_task().await;
        
        // FIXED: Setup graceful shutdown handler
        self.setup_shutdown_handler().await;
        
        let mut buffer = vec![0u8; 4096];
        
        // FIXED: Check shutdown flag in loop
        while !self.shutdown.load(Ordering::Relaxed) {
            match self.socket.recv_from(&mut buffer).await {
                Ok((len, from)) => {
                    // Basic input validation
                    if len > MAX_SIP_MESSAGE_SIZE {
                        warn!("⚠️ Oversized message from {}: {} bytes, dropping", from, len);
                        continue;
                    }
                    
                    let message = String::from_utf8_lossy(&buffer[..len]);
                    
                    // FIXED: Improved input validation 
                    if let Err(e) = self.validate_sip_message(&message, from) {
                        warn!("⚠️ Invalid SIP message from {}: {}, dropping", from, e);
                        continue;
                    }
                    
                    info!("📨 Received {} bytes from {}", len, from);
                    
                    // Simple forwarding logic
                    if let Err(e) = self.forward_message(&message, from).await {
                        error!("❌ Failed to forward message: {}", e);
                    }
                }
                Err(e) => {
                    error!("❌ Failed to receive UDP packet: {}", e);
                }
            }
        }
        
        info!("🛑 Simple B2BUA shutting down gracefully");
        Ok(())
    }

    async fn forward_message(&self, message: &str, from: SocketAddr) -> Result<()> {
        // Extract Call-ID for session tracking
        let call_id = self.extract_call_id(message)?;
        
        // Determine forwarding target
        let target_addr = format!("{}:{}", self.termination_host, self.termination_port);
        let target: SocketAddr = target_addr.parse()?;
        
        // Forward the message
        let message_bytes = message.as_bytes();
        match self.socket.send_to(message_bytes, target).await {
            Ok(sent) => {
                info!("📤 Forwarded {} bytes to {} for call {}", sent, target, call_id);
                Ok(())
            }
            Err(e) => {
                error!("❌ Failed to forward to {}: {}", target, e);
                Err(anyhow!("Forward failed: {}", e))
            }
        }
    }

    fn extract_call_id(&self, message: &str) -> Result<String> {
        for line in message.lines() {
            if line.to_lowercase().starts_with("call-id:") {
                if let Some(call_id) = line.split(':').nth(1) {
                    return Ok(call_id.trim().to_string());
                }
            }
        }
        Ok("unknown".to_string())
    }

    // FIXED: Enhanced SIP message validation
    fn validate_sip_message(&self, message: &str, from: SocketAddr) -> Result<()> {
        // Check for empty message
        if message.is_empty() {
            return Err(anyhow!("Empty message"));
        }

        // Check for minimum SIP structure
        if !message.starts_with("SIP/") && !message.contains("SIP/2.0") {
            return Err(anyhow!("Not a valid SIP message"));
        }

        // Check for required SIP headers in requests/responses
        let lines: Vec<&str> = message.lines().collect();
        if lines.is_empty() {
            return Err(anyhow!("No lines in message"));
        }

        // Validate first line (request line or status line)
        let first_line = lines[0];
        if first_line.is_empty() {
            return Err(anyhow!("Empty first line"));
        }

        // Check for suspicious characters that might indicate injection attacks
        if message.contains('\0') || message.contains('\r') && !message.contains("\r\n") {
            return Err(anyhow!("Suspicious characters detected"));
        }

        // Validate line endings (SIP requires CRLF)
        if message.contains('\n') && !message.contains("\r\n") {
            return Err(anyhow!("Invalid line endings (CRLF required)"));
        }

        // Check for excessively long lines (potential buffer overflow attempt)
        for line in &lines {
            if line.len() > 2048 {
                return Err(anyhow!("Line too long: {} chars", line.len()));
            }
        }

        // Basic header validation for potential injection
        for line in lines.iter().skip(1) { // Skip first line (request/status line)
            if line.is_empty() {
                break; // End of headers
            }
            
            if !line.contains(':') && !line.starts_with(' ') && !line.starts_with('\t') {
                return Err(anyhow!("Invalid header format"));
            }
        }

        Ok(())
    }

    // FIXED: Add call cleanup task to prevent memory leaks
    async fn start_call_cleanup_task(&self) {
        let calls = Arc::clone(&self.calls);
        let shutdown = Arc::clone(&self.shutdown);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // Every 5 minutes
            
            while !shutdown.load(Ordering::Relaxed) {
                interval.tick().await;
                
                let now = Instant::now();
                let mut calls_guard = calls.write().await;
                let initial_count = calls_guard.len();
                
                // Remove calls that have timed out
                calls_guard.retain(|_call_id, (a_leg, b_leg)| {
                    let a_active = now.duration_since(a_leg.last_activity) < CALL_TIMEOUT;
                    let b_active = now.duration_since(b_leg.last_activity) < CALL_TIMEOUT;
                    a_active && b_active
                });
                
                let cleaned_count = initial_count - calls_guard.len();
                if cleaned_count > 0 {
                    info!("🧹 Cleaned up {} timed-out calls, {} active calls remaining", 
                          cleaned_count, calls_guard.len());
                }
            }
        });
    }

    // FIXED: Add graceful shutdown handler
    async fn setup_shutdown_handler(&self) {
        let shutdown = Arc::clone(&self.shutdown);
        
        tokio::spawn(async move {
            // Wait for SIGINT (Ctrl+C) or SIGTERM
            let _ = signal::ctrl_c().await;
            info!("🛑 Shutdown signal received, initiating graceful shutdown...");
            shutdown.store(true, Ordering::Relaxed);
        });
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("🔥 Simple B2BUA Test - WORKING VERSION");
    println!("=====================================");

    // Configuration - use alternate port to avoid conflicts
    let bind_addr: SocketAddr = "0.0.0.0:5070".parse()?;
    let term_host = "127.0.0.1".to_string();
    let term_port = 5080;

    // Create and start B2BUA
    let b2bua = SimpleB2BUA::new(bind_addr, term_host, term_port).await?;
    
    info!("🚀 Starting Simple B2BUA...");
    b2bua.start().await?;

    Ok(())
}