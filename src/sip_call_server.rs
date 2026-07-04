/*
 * LCR-enabled SIP call server (library form).
 *
 * This is the in-process, reusable form of the `lcr_sip_server` binary. It
 * accepts SIP INVITEs, runs them through the real LCR routing engine, and drives
 * a basic INVITE -> 100 -> 180 -> 200 -> ACK -> BYE call flow back to the caller.
 *
 * Extracting it here (instead of leaving it bin-only) lets the automated SIP
 * call-flow tests start a real server on an ephemeral port and place real calls
 * against it, and keeps the binary a thin wrapper.
 */

use anyhow::{anyhow, Result};
use regex::Regex;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::lcr::routing::RouteRequest;
use crate::lcr::types::RouteType;
use crate::lcr::LcrEngine;

const MAX_SIP_MESSAGE_SIZE: usize = 65536;
const CALL_TIMEOUT: Duration = Duration::from_secs(1800); // 30 minute timeout

/// Timing profile for the simulated egress leg. Production uses realistic ring
/// times; tests use near-zero delays so call-flow tests run fast.
#[derive(Debug, Clone, Copy)]
pub struct CallTiming {
    /// Delay between receiving the INVITE and sending 180 Ringing.
    pub proceeding_delay: Duration,
    /// Delay between 180 Ringing and 200 OK (simulated ring/answer time).
    pub ring_delay: Duration,
}

impl Default for CallTiming {
    fn default() -> Self {
        Self {
            proceeding_delay: Duration::from_millis(100),
            ring_delay: Duration::from_millis(2000),
        }
    }
}

impl CallTiming {
    /// Fast timing for tests: answer almost immediately.
    pub fn fast() -> Self {
        Self {
            proceeding_delay: Duration::from_millis(5),
            ring_delay: Duration::from_millis(20),
        }
    }
}

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
    timing: CallTiming,
    /// Ingress trunk id to attribute inbound calls to when routing. In a full
    /// deployment this is resolved from the source IP / SIP profile; here it is a
    /// single configurable default (the previous hardcoded 999 matched no seeded
    /// trunk, so every call fell through to 404).
    default_ingress_trunk_id: i32,
}

impl LcrSipServer {
    /// Bind and construct a server with production timing.
    pub async fn new(bind_addr: SocketAddr, database_url: &str) -> Result<Self> {
        Self::with_timing(bind_addr, database_url, CallTiming::default()).await
    }

    /// Bind and construct a server with an explicit timing profile.
    ///
    /// Pass `bind_addr` with port 0 to bind an ephemeral port, then read the
    /// actual address back via [`local_addr`](Self::local_addr).
    pub async fn with_timing(
        bind_addr: SocketAddr,
        database_url: &str,
        timing: CallTiming,
    ) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr).await?;
        let bind_addr = socket.local_addr()?;
        info!("LCR SIP Server bound to {}", bind_addr);

        let lcr_engine = Arc::new(LcrEngine::new(database_url).await?);
        info!("LCR Engine initialized");

        Ok(Self {
            socket: Arc::new(socket),
            bind_addr,
            calls: Arc::new(RwLock::new(HashMap::new())),
            lcr_engine,
            shutdown: Arc::new(AtomicBool::new(false)),
            timing,
            default_ingress_trunk_id: 1,
        })
    }

    /// Set the default ingress trunk id used to attribute inbound calls.
    pub fn set_default_ingress_trunk_id(&mut self, id: i32) {
        self.default_ingress_trunk_id = id;
    }

    /// The address the server is actually bound to (useful with port 0).
    pub fn local_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    /// A shutdown handle that can be flipped to stop [`run`](Self::run).
    pub fn shutdown_handle(&self) -> Arc<AtomicBool> {
        self.shutdown.clone()
    }

    /// Request a graceful shutdown.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }

    /// Run the receive loop until shutdown is requested. Unlike the binary, this
    /// does not install a Ctrl+C handler, so it composes cleanly inside tests and
    /// larger services.
    pub async fn run(&self) -> Result<()> {
        info!("🚀 LCR SIP Server starting on {}", self.bind_addr);

        let mut buffer = vec![0u8; MAX_SIP_MESSAGE_SIZE];

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

            // Use a short recv timeout so we notice shutdown promptly.
            let recv = tokio::time::timeout(
                Duration::from_millis(200),
                self.socket.recv_from(&mut buffer),
            )
            .await;

            match recv {
                Ok(Ok((size, addr))) => {
                    let message = String::from_utf8_lossy(&buffer[..size]);
                    debug!("Received {} bytes from {}", size, addr);
                    if let Err(e) = self.handle_sip_message(&message, addr).await {
                        error!("Error handling SIP message from {}: {}", addr, e);
                    }
                }
                Ok(Err(e)) => {
                    error!("Error receiving UDP message: {}", e);
                }
                Err(_) => {
                    // recv timed out; loop to re-check shutdown.
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

        let call_id = self.extract_call_id(message)?;
        let from_tag = self.extract_from_tag(message)?;
        let (ani, dnis) = self.extract_ani_dnis(message)?;

        info!("Call: {} → {} (Call-ID: {})", ani, dnis, call_id);

        // Send 100 Trying immediately.
        self.send_response(100, "Trying", &call_id, &from_tag, None, from_addr)
            .await?;

        match self.perform_lcr_routing(&ani, &dnis).await {
            Ok(route) => {
                info!(
                    "✅ LCR Route found: {} → {}:{} ({})",
                    dnis, route.host, route.port, route.trunk_name
                );

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

    /// Look up the best egress route for a call via the real LCR engine.
    pub async fn perform_lcr_routing(&self, ani: &str, dnis: &str) -> Result<EgressRoute> {
        let route_request = RouteRequest {
            ani: ani.to_string(),
            dnis: dnis.to_string(),
            ingress_trunk_id: self.default_ingress_trunk_id,
            client_deck_id: None,
            route_type: RouteType::NANPA,
            require_profit_protection: false,
            min_profit_margin: None,
            effective_time: None,
            phone_validation: None,
            routing_plan_id: None,
        };

        let routing_engine = self.lcr_engine.get_routing_engine();
        let routes = routing_engine
            .find_routes(&route_request)
            .await
            .map_err(|e| anyhow!("LCR routing failed: {}", e))?;

        if routes.routes.is_empty() {
            return Err(anyhow!("No routes found for {} → {}", ani, dnis));
        }

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
        _original_invite: &str,
        route: &EgressRoute,
        call_id: &str,
        from_tag: &str,
        ingress_addr: SocketAddr,
    ) -> Result<()> {
        info!("🔄 Forwarding INVITE to {}:{}", route.host, route.port);

        // Simulate egress processing/ring time, then answer. A real deployment
        // forwards to the egress trunk and relays its provisional/final responses.
        tokio::time::sleep(self.timing.proceeding_delay).await;

        self.send_response(180, "Ringing", call_id, from_tag, None, ingress_addr)
            .await?;
        if let Some(session) = self.calls.write().await.get_mut(call_id) {
            session.state = CallState::Ringing;
            session.last_activity = Instant::now();
        }

        tokio::time::sleep(self.timing.ring_delay).await;

        let to_tag = format!("tag-{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
        self.send_response(200, "OK", call_id, from_tag, Some(&to_tag), ingress_addr)
            .await?;

        if let Some(session) = self.calls.write().await.get_mut(call_id) {
            session.to_tag = Some(to_tag);
            session.state = CallState::Connected;
            session.last_activity = Instant::now();
        }

        Ok(())
    }

    async fn handle_ack(&self, message: &str, _from_addr: SocketAddr) -> Result<()> {
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

        self.send_response(200, "OK", &call_id, &from_tag, None, from_addr)
            .await?;

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

        self.send_response(200, "OK", &call_id, &from_tag, None, from_addr)
            .await?;
        self.send_response(
            487,
            "Request Terminated",
            &call_id,
            &from_tag,
            None,
            from_addr,
        )
        .await?;

        self.calls.write().await.remove(&call_id);

        Ok(())
    }

    async fn handle_options(&self, message: &str, from_addr: SocketAddr) -> Result<()> {
        let call_id = self
            .extract_call_id(message)
            .unwrap_or_else(|_| "options".to_string());
        debug!("📋 OPTIONS request from {}", from_addr);

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
        let ani = if let Ok(pai) = self.extract_p_asserted_identity(message) {
            pai
        } else {
            self.extract_from_user(message)?
        };

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
            tokio::time::sleep(Duration::from_secs(60)).await;

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
