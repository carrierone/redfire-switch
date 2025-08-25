/*
 * LCR-Enabled SIP Server
 * Handles incoming SIP calls and routes them through the LCR engine
 */

use anyhow::{anyhow, Result};
use clap::{Arg, Command};
use redfire_switch::lcr::routing::RouteRequest;
use redfire_switch::lcr::types::RouteType;
use redfire_switch::lcr::LcrEngine;
use regex::Regex;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::signal;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

const MAX_SIP_MESSAGE_SIZE: usize = 65536;
const CALL_TIMEOUT: Duration = Duration::from_secs(1800); // 30 minute timeout

#[derive(Debug, Clone)]
pub struct CallSession {
    pub call_id: String,
    pub ani: String,
    pub dnis: String,
    pub from_tag: String,
    pub to_tag: Option<String>,
    pub ingress_addr: SocketAddr,
    pub egress_route: Option<EgressRoute>,
    pub state: CallState,
    pub created_at: Instant,
    pub last_activity: Instant,
}

#[derive(Debug, Clone)]
pub struct EgressRoute {
    pub host: String,
    pub port: u16,
    pub trunk_name: String,
    pub cost_per_minute: rust_decimal::Decimal,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CallState {
    Initial,
    Trying,
    Proceeding,
    Ringing,
    Connected,
    Disconnected,
}

pub struct LcrSipServer {
    socket: Arc<UdpSocket>,
    bind_addr: SocketAddr,
    calls: Arc<RwLock<HashMap<String, CallSession>>>,
    lcr_engine: Arc<LcrEngine>,
    shutdown: Arc<AtomicBool>,
}

impl LcrSipServer {
    pub async fn new(bind_addr: SocketAddr, database_url: &str) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr).await?;
        info!("LCR SIP Server bound to {}", bind_addr);

        // Initialize LCR engine
        let lcr_engine = Arc::new(LcrEngine::new(database_url).await?);
        info!("LCR Engine initialized");

        Ok(Self {
            socket: Arc::new(socket),
            bind_addr,
            calls: Arc::new(RwLock::new(HashMap::new())),
            lcr_engine,
            shutdown: Arc::new(AtomicBool::new(false)),
        })
    }

    pub async fn run(&self) -> Result<()> {
        info!("🚀 LCR SIP Server starting on {}", self.bind_addr);

        let mut buffer = vec![0u8; MAX_SIP_MESSAGE_SIZE];

        // Start cleanup task
        let cleanup_calls = self.calls.clone();
        let cleanup_shutdown = self.shutdown.clone();
        tokio::spawn(async move {
            Self::cleanup_task(cleanup_calls, cleanup_shutdown).await;
        });

        loop {
            if self.shutdown.load(Ordering::Relaxed) {
                info!("Shutdown signal received, stopping server");
                break;
            }

            tokio::select! {
                // Handle incoming SIP messages
                recv_result = self.socket.recv_from(&mut buffer) => {
                    match recv_result {
                        Ok((size, addr)) => {
                            let message = String::from_utf8_lossy(&buffer[..size]);
                            debug!("Received {} bytes from {}", size, addr);

                            if let Err(e) = self.handle_sip_message(&message, addr).await {
                                error!("Error handling SIP message from {}: {}", addr, e);
                            }
                        }
                        Err(e) => {
                            error!("Error receiving UDP message: {}", e);
                        }
                    }
                }

                // Handle shutdown signal
                _ = signal::ctrl_c() => {
                    info!("Received Ctrl+C, shutting down gracefully");
                    self.shutdown.store(true, Ordering::Relaxed);
                    break;
                }
            }
        }

        info!("LCR SIP Server stopped");
        Ok(())
    }

    async fn handle_sip_message(&self, message: &str, from_addr: SocketAddr) -> Result<()> {
        debug!(
            "Processing SIP message from {}: {}",
            from_addr,
            message.lines().next().unwrap_or("")
        );

        if message.starts_with("INVITE") {
            self.handle_invite(message, from_addr).await
        } else if message.starts_with("ACK") {
            self.handle_ack(message, from_addr).await
        } else if message.starts_with("BYE") {
            self.handle_bye(message, from_addr).await
        } else if message.starts_with("CANCEL") {
            self.handle_cancel(message, from_addr).await
        } else if message.starts_with("OPTIONS") {
            self.handle_options(message, from_addr).await
        } else {
            debug!(
                "Ignoring SIP method: {}",
                message.lines().next().unwrap_or("")
            );
            Ok(())
        }
    }

    async fn handle_invite(&self, message: &str, from_addr: SocketAddr) -> Result<()> {
        info!("📞 Processing INVITE from {}", from_addr);

        // Parse INVITE message
        let call_id = self.extract_call_id(message)?;
        let from_tag = self.extract_from_tag(message)?;
        let (ani, dnis) = self.extract_ani_dnis(message)?;

        info!("Call: {} → {} (Call-ID: {})", ani, dnis, call_id);

        // Send 100 Trying immediately
        self.send_response(100, "Trying", &call_id, &from_tag, None, from_addr)
            .await?;

        // Perform LCR routing
        match self.perform_lcr_routing(&ani, &dnis).await {
            Ok(route) => {
                info!(
                    "✅ LCR Route found: {} → {}:{} ({})",
                    dnis, route.host, route.port, route.trunk_name
                );

                // Create call session
                let session = CallSession {
                    call_id: call_id.clone(),
                    ani: ani.clone(),
                    dnis: dnis.clone(),
                    from_tag: from_tag.clone(),
                    to_tag: None,
                    ingress_addr: from_addr,
                    egress_route: Some(route.clone()),
                    state: CallState::Trying,
                    created_at: Instant::now(),
                    last_activity: Instant::now(),
                };

                self.calls.write().await.insert(call_id.clone(), session);

                // Forward INVITE to egress
                self.forward_invite_to_egress(message, &route, &call_id, &from_tag, from_addr)
                    .await?;
            }
            Err(e) => {
                warn!("❌ LCR routing failed for {} → {}: {}", ani, dnis, e);
                self.send_response(404, "Not Found", &call_id, &from_tag, None, from_addr)
                    .await?;
            }
        }

        Ok(())
    }

    async fn perform_lcr_routing(&self, ani: &str, dnis: &str) -> Result<EgressRoute> {
        // Create route request
        let route_request = RouteRequest {
            ani: ani.to_string(),
            dnis: dnis.to_string(),
            ingress_trunk_id: 999, // Use test trunk ID
            client_deck_id: Some(999),
            route_type: RouteType::NANPA,
            require_profit_protection: false,
            min_profit_margin: None,
            effective_time: None,
            phone_validation: None,
            routing_plan_id: None,
        };

        // Get routes from LCR engine
        let routing_engine = self.lcr_engine.get_routing_engine();
        let routes = routing_engine
            .find_routes(&route_request)
            .await
            .map_err(|e| anyhow!("LCR routing failed: {}", e))?;

        if routes.routes.is_empty() {
            return Err(anyhow!("No routes found for {} → {}", ani, dnis));
        }

        // Use first (best) route
        let best_route = &routes.routes[0];

        Ok(EgressRoute {
            host: best_route.egress_trunk.host.clone(),
            port: best_route.egress_trunk.port as u16,
            trunk_name: best_route.egress_trunk.name.clone(),
            cost_per_minute: best_route.cost_per_minute,
        })
    }

    async fn forward_invite_to_egress(
        &self,
        original_invite: &str,
        route: &EgressRoute,
        call_id: &str,
        from_tag: &str,
        ingress_addr: SocketAddr,
    ) -> Result<()> {
        info!("🔄 Forwarding INVITE to {}:{}", route.host, route.port);

        // For demo purposes, we'll simulate forwarding and send back a response
        // In a real implementation, you'd forward to the actual egress trunk

        tokio::time::sleep(Duration::from_millis(100)).await; // Simulate processing time

        // Simulate successful routing
        self.send_response(180, "Ringing", call_id, from_tag, None, ingress_addr)
            .await?;

        tokio::time::sleep(Duration::from_millis(2000)).await; // Simulate ring time

        // Simulate answer
        let to_tag = format!("tag-{}", chrono::Utc::now().timestamp());
        self.send_response(200, "OK", call_id, from_tag, Some(&to_tag), ingress_addr)
            .await?;

        // Update call session
        if let Some(mut session) = self.calls.write().await.get_mut(call_id) {
            session.to_tag = Some(to_tag);
            session.state = CallState::Connected;
            session.last_activity = Instant::now();
        }

        Ok(())
    }

    async fn handle_ack(&self, message: &str, from_addr: SocketAddr) -> Result<()> {
        let call_id = self.extract_call_id(message)?;
        debug!("📝 ACK received for call {}", call_id);

        if let Some(session) = self.calls.write().await.get_mut(&call_id) {
            session.state = CallState::Connected;
            session.last_activity = Instant::now();
        }

        Ok(())
    }

    async fn handle_bye(&self, message: &str, from_addr: SocketAddr) -> Result<()> {
        let call_id = self.extract_call_id(message)?;
        let from_tag = self.extract_from_tag(message)?;

        info!("📱 BYE received for call {}", call_id);

        // Send 200 OK for BYE
        self.send_response(200, "OK", &call_id, &from_tag, None, from_addr)
            .await?;

        // Remove call session
        if let Some(session) = self.calls.write().await.remove(&call_id) {
            let duration = session.created_at.elapsed();
            info!(
                "📊 Call {} completed: {} → {} ({}s)",
                call_id,
                session.ani,
                session.dnis,
                duration.as_secs()
            );
        }

        Ok(())
    }

    async fn handle_cancel(&self, message: &str, from_addr: SocketAddr) -> Result<()> {
        let call_id = self.extract_call_id(message)?;
        let from_tag = self.extract_from_tag(message)?;

        info!("❌ CANCEL received for call {}", call_id);

        // Send 200 OK for CANCEL
        self.send_response(200, "OK", &call_id, &from_tag, None, from_addr)
            .await?;

        // Send 487 Request Terminated for original INVITE
        self.send_response(
            487,
            "Request Terminated",
            &call_id,
            &from_tag,
            None,
            from_addr,
        )
        .await?;

        // Remove call session
        self.calls.write().await.remove(&call_id);

        Ok(())
    }

    async fn handle_options(&self, message: &str, from_addr: SocketAddr) -> Result<()> {
        let call_id = self
            .extract_call_id(message)
            .unwrap_or_else(|_| "options".to_string());
        debug!("📋 OPTIONS request from {}", from_addr);

        // Send 200 OK with capabilities
        let response = format!(
            "SIP/2.0 200 OK\r\n\
             Via: SIP/2.0/UDP {}\r\n\
             Call-ID: {}\r\n\
             From: <sip:options@{}>;tag=options\r\n\
             To: <sip:options@{}>\r\n\
             CSeq: 1 OPTIONS\r\n\
             Allow: INVITE, ACK, CANCEL, BYE, OPTIONS\r\n\
             Accept: application/sdp\r\n\
             User-Agent: Redfire LCR Switch\r\n\
             Content-Length: 0\r\n\r\n",
            from_addr,
            call_id,
            from_addr.ip(),
            self.bind_addr.ip()
        );

        self.socket.send_to(response.as_bytes(), from_addr).await?;
        Ok(())
    }

    async fn send_response(
        &self,
        code: u16,
        reason: &str,
        call_id: &str,
        from_tag: &str,
        to_tag: Option<&str>,
        to_addr: SocketAddr,
    ) -> Result<()> {
        let to_tag_str = to_tag.map(|t| format!(";tag={}", t)).unwrap_or_default();

        let response = format!(
            "SIP/2.0 {} {}\r\n\
             Via: SIP/2.0/UDP {}\r\n\
             Call-ID: {}\r\n\
             From: <sip:switch@{}>;tag={}\r\n\
             To: <sip:switch@{}>{}\r\n\
             CSeq: 1 INVITE\r\n\
             User-Agent: Redfire LCR Switch\r\n\
             Content-Length: 0\r\n\r\n",
            code,
            reason,
            to_addr,
            call_id,
            self.bind_addr.ip(),
            from_tag,
            self.bind_addr.ip(),
            to_tag_str
        );

        debug!("📤 Sending {} {} to {}", code, reason, to_addr);
        self.socket.send_to(response.as_bytes(), to_addr).await?;
        Ok(())
    }

    // Helper methods for parsing SIP messages
    fn extract_call_id(&self, message: &str) -> Result<String> {
        let re = Regex::new(r"Call-ID:\s*([^\r\n]+)")?;
        re.captures(message)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().trim().to_string())
            .ok_or_else(|| anyhow!("Call-ID not found"))
    }

    fn extract_from_tag(&self, message: &str) -> Result<String> {
        let re = Regex::new(r"From:.*tag=([^;\r\n]+)")?;
        re.captures(message)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().trim().to_string())
            .ok_or_else(|| anyhow!("From tag not found"))
    }

    fn extract_ani_dnis(&self, message: &str) -> Result<(String, String)> {
        // Extract ANI from From header or P-Asserted-Identity
        let ani = if let Ok(pai) = self.extract_p_asserted_identity(message) {
            pai
        } else {
            self.extract_from_user(message)?
        };

        // Extract DNIS from Request-URI
        let dnis = self.extract_request_uri_user(message)?;

        Ok((ani, dnis))
    }

    fn extract_p_asserted_identity(&self, message: &str) -> Result<String> {
        let re = Regex::new(r"P-Asserted-Identity:.*<sip:([^@]+)@")?;
        re.captures(message)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
            .ok_or_else(|| anyhow!("P-Asserted-Identity not found"))
    }

    fn extract_from_user(&self, message: &str) -> Result<String> {
        let re = Regex::new(r"From:.*<sip:([^@]+)@")?;
        re.captures(message)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
            .ok_or_else(|| anyhow!("From user not found"))
    }

    fn extract_request_uri_user(&self, message: &str) -> Result<String> {
        let re = Regex::new(r"INVITE sip:([^@]+)@")?;
        re.captures(message)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
            .ok_or_else(|| anyhow!("Request-URI user not found"))
    }

    async fn cleanup_task(
        calls: Arc<RwLock<HashMap<String, CallSession>>>,
        shutdown: Arc<AtomicBool>,
    ) {
        while !shutdown.load(Ordering::Relaxed) {
            tokio::time::sleep(Duration::from_secs(60)).await; // Check every minute

            let mut calls_guard = calls.write().await;
            let now = Instant::now();
            let mut to_remove = Vec::new();

            for (call_id, session) in calls_guard.iter() {
                if now.duration_since(session.last_activity) > CALL_TIMEOUT {
                    info!("🧹 Cleaning up stale call: {}", call_id);
                    to_remove.push(call_id.clone());
                }
            }

            for call_id in to_remove {
                calls_guard.remove(&call_id);
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let matches = Command::new("lcr-sip-server")
        .version("1.0")
        .about("LCR-enabled SIP Server for call routing")
        .arg(
            Arg::new("bind")
                .short('b')
                .long("bind")
                .value_name("ADDR:PORT")
                .help("Bind address and port")
                .default_value("0.0.0.0:5060"),
        )
        .arg(
            Arg::new("database-url")
                .long("database-url")
                .value_name("URL")
                .help("PostgreSQL database URL (can be set via DATABASE_URL env var)")
                .default_value("postgresql://postgres:postgres@localhost:5432/lcr"),
        )
        .get_matches();

    let bind_addr_str = matches.get_one::<String>("bind").unwrap();
    let database_url = matches
        .get_one::<String>("database-url")
        .map(|s| s.clone())
        .or_else(|| std::env::var("DATABASE_URL").ok())
        .unwrap_or_else(|| "postgresql://postgres:postgres@localhost:5432/lcr".to_string());

    let bind_addr: SocketAddr = bind_addr_str
        .parse()
        .map_err(|_| anyhow!("Invalid bind address: {}", bind_addr_str))?;

    info!("🔥 Starting Redfire LCR SIP Server");
    info!("📍 Bind Address: {}", bind_addr);
    info!("🗄️  Database: {}", database_url);

    let server = LcrSipServer::new(bind_addr, &database_url).await?;
    server.run().await?;

    Ok(())
}
