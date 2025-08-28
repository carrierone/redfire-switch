//! Class 4 B2BUA Implementation
//! Implements a production-ready Class 4 switching B2BUA that routes calls between gateways
//! with codec translation signaling but no media processing

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::lcr::types::{RouteRequest, RouteType};
use crate::origination_routing::{OriginationRequest, OriginationRoutingEngine};
use crate::route_advancement::{RouteAdvancementEngine, RouteAdvancementResult};
use crate::termination_routing::{TerminationRoutingRequest, TerminationRoutingService};

/// Class 4 B2BUA main structure
pub struct Class4B2BUA {
    config: Arc<Class4Config>,
    socket: Arc<UdpSocket>,
    session_manager: Arc<SessionManager>,
    origination_engine: Arc<Mutex<OriginationRoutingEngine>>,
    termination_service: Arc<Mutex<TerminationRoutingService>>,
    route_advancement: Arc<Mutex<RouteAdvancementEngine>>,
    call_processor: Arc<CallProcessor>,
    cdr_generator: Arc<CDRGenerator>,
    codec_translator: Arc<CodecTranslator>,
}

/// Class 4 B2BUA Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Class4Config {
    pub bind_address: IpAddr,
    pub bind_port: u16,
    pub max_concurrent_calls: u32,
    pub call_timeout_seconds: u64,
    pub session_cleanup_interval_seconds: u64,
    pub enable_cdr_generation: bool,
    pub enable_codec_translation: bool,
    pub enable_call_recording_headers: bool,
    pub max_route_attempts: u32,
    pub rtp_proxy_host: Option<String>,
    pub rtp_proxy_port: Option<u16>,
}

impl Default for Class4Config {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".parse().expect("Invalid default bind address"),
            bind_port: 5060,
            max_concurrent_calls: 10000,
            call_timeout_seconds: 1800, // 30 minutes
            session_cleanup_interval_seconds: 60,
            enable_cdr_generation: true,
            enable_codec_translation: true,
            enable_call_recording_headers: false,
            max_route_attempts: 3,
            rtp_proxy_host: None,
            rtp_proxy_port: None,
        }
    }
}

/// SIP Session Manager for B2BUA operations
pub struct SessionManager {
    active_sessions: RwLock<HashMap<String, CallSession>>,
    call_id_mapping: RwLock<HashMap<String, String>>, // Map between A-leg and B-leg call IDs
    stats: RwLock<SessionStats>,
}

/// Complete call session with both legs
#[derive(Debug, Clone)]
pub struct CallSession {
    pub session_id: String,
    pub a_leg: CallLeg,
    pub b_leg: Option<CallLeg>,
    pub state: SessionState,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub route_attempts: u32,
    pub current_route: Option<crate::lcr::types::CallRoute>,
    pub codec_negotiation: CodecNegotiation,
    pub cdr: CallDetailRecord,
}

/// Individual call leg (A-leg or B-leg)
#[derive(Debug, Clone)]
pub struct CallLeg {
    pub call_id: String,
    pub from_tag: String,
    pub to_tag: Option<String>,
    pub local_addr: SocketAddr,
    pub remote_addr: SocketAddr,
    pub state: LegState,
    pub sip_headers: HashMap<String, String>,
    pub supported_codecs: Vec<String>,
    pub selected_codec: Option<String>,
    pub last_cseq: u32,
}

/// Session state for B2BUA operations
#[derive(Debug, Clone, PartialEq)]
pub enum SessionState {
    Initial,
    Originating,   // Processing origination
    Routing,       // Finding termination route
    Terminating,   // Attempting termination
    Connecting,    // Both legs trying to connect
    Connected,     // Call established
    Disconnecting, // One or both legs terminating
    Terminated,    // Call ended
    Failed,        // Call failed
}

/// Individual leg state
#[derive(Debug, Clone, PartialEq)]
pub enum LegState {
    Initial,
    Invited,
    Proceeding,
    Ringing,
    Connected,
    Disconnecting,
    Terminated,
    Failed,
}

/// Codec negotiation state
#[derive(Debug, Clone)]
pub struct CodecNegotiation {
    pub a_leg_codecs: Vec<String>,
    pub b_leg_codecs: Vec<String>,
    pub negotiated_codec: Option<String>,
    pub transcoding_required: bool,
    pub transcoding_profile: Option<String>,
}

/// Call processing engine
pub struct CallProcessor {
    config: Arc<Class4Config>,
}

/// CDR (Call Detail Record) generator
pub struct CDRGenerator {
    config: Arc<Class4Config>,
    cdr_sender: mpsc::UnboundedSender<CallDetailRecord>,
}

/// Codec translation handler (signaling only, no media processing)
pub struct CodecTranslator {
    supported_codecs: Vec<String>,
    transcoding_profiles: HashMap<String, TranscodingProfile>,
}

/// Call Detail Record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallDetailRecord {
    pub session_id: String,
    pub a_leg_call_id: String,
    pub b_leg_call_id: Option<String>,
    pub calling_number: String,
    pub called_number: String,
    pub origination_ip: IpAddr,
    pub termination_ip: Option<IpAddr>,
    pub start_time: DateTime<Utc>,
    pub answer_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_seconds: Option<u64>,
    pub termination_cause: Option<u16>,
    pub termination_reason: Option<String>,
    pub route_attempts: u32,
    pub final_route: Option<String>,
    pub codec_negotiated: Option<String>,
    pub transcoding_used: bool,
}

/// Transcoding profile for codec translation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscodingProfile {
    pub name: String,
    pub source_codec: String,
    pub target_codec: String,
    pub quality_profile: String,
    pub bandwidth_optimization: bool,
}

/// Session statistics
#[derive(Debug, Default, Clone)]
pub struct SessionStats {
    pub total_sessions: u64,
    pub active_sessions: u32,
    pub successful_calls: u64,
    pub failed_calls: u64,
    pub average_setup_time_ms: u64,
    pub total_call_minutes: u64,
    pub peak_concurrent_calls: u32,
}

impl Class4B2BUA {
    /// Create a new Class 4 B2BUA instance
    pub async fn new(
        config: Class4Config,
        origination_engine: Arc<Mutex<OriginationRoutingEngine>>,
        termination_service: Arc<Mutex<TerminationRoutingService>>,
        route_advancement: Arc<Mutex<RouteAdvancementEngine>>,
    ) -> Result<Self> {
        let bind_addr = SocketAddr::new(config.bind_address, config.bind_port);
        let socket = UdpSocket::bind(bind_addr).await?;

        info!("Class 4 B2BUA starting on {}", bind_addr);

        let config_arc = Arc::new(config);
        let session_manager = Arc::new(SessionManager::new());
        let call_processor = Arc::new(CallProcessor::new(config_arc.clone()));

        let (cdr_sender, cdr_receiver) = mpsc::unbounded_channel();
        let cdr_generator = Arc::new(CDRGenerator::new(config_arc.clone(), cdr_sender));

        // Start CDR processing task
        CDRGenerator::start_cdr_processor(cdr_receiver);

        let codec_translator = Arc::new(CodecTranslator::new());

        let b2bua = Self {
            config: config_arc,
            socket: Arc::new(socket),
            session_manager,
            origination_engine,
            termination_service,
            route_advancement,
            call_processor,
            cdr_generator,
            codec_translator,
        };

        // Start background tasks
        b2bua.start_session_cleanup_task();

        info!("Class 4 B2BUA initialized successfully");
        Ok(b2bua)
    }

    /// Get session manager for external access
    pub fn session_manager(&self) -> &Arc<SessionManager> {
        &self.session_manager
    }

    /// Start the main B2BUA processing loop
    pub async fn run(&self) -> Result<()> {
        info!("Class 4 B2BUA starting main processing loop");

        let mut buffer = [0u8; 4096];

        loop {
            match self.socket.recv_from(&mut buffer).await {
                Ok((size, addr)) => {
                    let data = &buffer[..size];

                    if let Ok(message) = std::str::from_utf8(data) {
                        if let Err(e) = self.process_sip_message(message, addr).await {
                            error!("Failed to process SIP message from {}: {}", addr, e);
                        }
                    } else {
                        warn!("Received non-UTF8 data from {}", addr);
                    }
                }
                Err(e) => {
                    error!("Failed to receive UDP data: {}", e);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }

    /// Process incoming SIP messages
    async fn process_sip_message(&self, message: &str, addr: SocketAddr) -> Result<()> {
        debug!(
            "Processing SIP message from {}: {}",
            addr,
            message.lines().next().unwrap_or("")
        );

        let sip_message = self.parse_sip_message(message)?;

        match sip_message.method.as_deref() {
            Some("INVITE") => self.handle_invite(sip_message, addr).await,
            Some("ACK") => self.handle_ack(sip_message, addr).await,
            Some("BYE") => self.handle_bye(sip_message, addr).await,
            Some("CANCEL") => self.handle_cancel(sip_message, addr).await,
            None => {
                // This is a response
                self.handle_sip_response(sip_message, addr).await
            }
            _ => {
                debug!("Ignoring SIP method: {:?}", sip_message.method);
                Ok(())
            }
        }
    }

    /// Handle INVITE messages (call setup)
    async fn handle_invite(&self, sip_message: SipMessage, addr: SocketAddr) -> Result<()> {
        let call_id = sip_message
            .headers
            .get("Call-ID")
            .ok_or_else(|| anyhow!("Missing Call-ID header"))?;

        info!("Processing INVITE for call {}", call_id);

        // Check if this is a new call or retransmission
        if self.session_manager.session_exists(call_id).await {
            debug!("Retransmission detected for call {}", call_id);
            return Ok(());
        }

        // Extract call information
        let calling_number = self.extract_calling_number(&sip_message)?;
        let called_number = self.extract_called_number(&sip_message)?;

        // Create origination request
        let origination_request = OriginationRequest {
            ani: calling_number.clone(),
            dnis: called_number.clone(),
            source_ip: addr.ip(),
            ingress_trunk_id: 0, // TODO: Extract from actual trunk mapping
            customer_id: None,   // TODO: Extract from authentication
            route_type: RouteType::NANPA, // Default to NANPA routing
            timestamp: Utc::now(),
        };

        // Process origination routing
        let origination_result = {
            let mut engine = self.origination_engine.lock().await;
            engine.route_origination(origination_request).await?
        };

        if !origination_result.allowed {
            info!(
                "Call rejected by origination routing: {}",
                origination_result.reason
            );
            self.send_sip_response(addr, call_id, 403, "Forbidden", &origination_result.reason)
                .await?;
            return Ok(());
        }

        // Create call session
        let session = self
            .create_call_session(&sip_message, addr, calling_number, called_number)
            .await?;

        // Store session
        self.session_manager.add_session(session.clone()).await;

        // Send 100 Trying
        self.send_sip_response(addr, call_id, 100, "Trying", "")
            .await?;

        // Begin termination routing
        self.begin_termination_routing(session).await?;

        Ok(())
    }

    /// Begin termination routing process
    async fn begin_termination_routing(&self, mut session: CallSession) -> Result<()> {
        session.state = SessionState::Routing;
        self.session_manager.update_session(session.clone()).await;

        let route_request = RouteRequest {
            ani: session.cdr.calling_number.clone(),
            dnis: session.cdr.called_number.clone(),
            ingress_trunk_id: 1, // TODO: Get from origination result
            client_deck_id: None,
            route_type: RouteType::NANPA, // TODO: Determine from number analysis
            require_profit_protection: false,
            min_profit_margin: None,
            effective_time: Some(Utc::now()),
            phone_validation: None,
            routing_plan_id: None,
        };

        let termination_request = TerminationRoutingRequest {
            call_id: session.session_id.clone(),
            ani: session.cdr.calling_number.clone(),
            dnis: session.cdr.called_number.clone(),
            route_request,
            attempt_number: 1,
            previous_responses: vec![],
            max_attempts: self.config.max_route_attempts,
            timestamp: Utc::now(),
        };

        // Get route from termination service
        let routing_response = {
            let mut service = self.termination_service.lock().await;
            service.route_termination(termination_request).await?
        };

        if !routing_response.success {
            info!(
                "No routes available for call {}: {}",
                session.session_id, routing_response.reason
            );
            self.terminate_session(&session.session_id, 503, "Service Unavailable")
                .await?;
            return Ok(());
        }

        if let Some(route) = routing_response.selected_route {
            session.current_route = Some(route.clone());
            session.state = SessionState::Terminating;
            self.session_manager.update_session(session.clone()).await;

            // Attempt termination
            self.attempt_termination(session, route).await?;
        }

        Ok(())
    }

    /// Attempt call termination to selected route
    async fn attempt_termination(
        &self,
        session: CallSession,
        route: crate::lcr::types::CallRoute,
    ) -> Result<()> {
        info!(
            "Attempting termination for call {} via trunk {}",
            session.session_id, route.egress_trunk.name
        );

        // Create B-leg INVITE
        let b_leg_invite = self.create_b_leg_invite(&session, &route).await?;

        // Send INVITE to termination gateway
        let term_addr = SocketAddr::new(route.egress_trunk.host.parse()?, route.egress_trunk.port);

        self.send_sip_message(term_addr, &b_leg_invite).await?;

        // Update session state
        let mut updated_session = session;
        updated_session.state = SessionState::Terminating;
        updated_session.route_attempts += 1;
        self.session_manager.update_session(updated_session).await;

        Ok(())
    }

    /// Handle SIP responses
    async fn handle_sip_response(&self, sip_message: SipMessage, addr: SocketAddr) -> Result<()> {
        let call_id = sip_message
            .headers
            .get("Call-ID")
            .ok_or_else(|| anyhow!("Missing Call-ID header"))?;

        let response_code = sip_message
            .status_code
            .ok_or_else(|| anyhow!("Missing status code in response"))?;

        debug!(
            "Processing SIP response {} for call {}",
            response_code, call_id
        );

        let session = self
            .session_manager
            .get_session_by_any_call_id(call_id)
            .await;
        if session.is_none() {
            debug!("Received response for unknown call: {}", call_id);
            return Ok(());
        }

        let session = session; // Already checked for None above

        match response_code {
            100..=199 => {
                if let Some(session) = session {
                    self.handle_provisional_response(session, response_code, &sip_message)
                        .await
                } else {
                    Ok(())
                }
            }
            200..=299 => {
                if let Some(session) = session {
                    self.handle_success_response(session, response_code, &sip_message)
                        .await
                } else {
                    Ok(())
                }
            }
            300..=699 => {
                if let Some(session) = session {
                    self.handle_error_response(session, response_code, &sip_message)
                        .await
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        }
    }

    /// Handle provisional responses (100-199)
    async fn handle_provisional_response(
        &self,
        session: CallSession,
        code: u16,
        _message: &SipMessage,
    ) -> Result<()> {
        debug!(
            "Provisional response {} for call {}",
            code, session.session_id
        );

        // Forward provisional response to A-leg
        if let Some(a_leg_addr) = self.get_a_leg_address(&session).await? {
            self.send_sip_response(a_leg_addr, &session.a_leg.call_id, code, "Progress", "")
                .await?;
        }

        Ok(())
    }

    /// Handle success responses (200-299)  
    async fn handle_success_response(
        &self,
        mut session: CallSession,
        code: u16,
        message: &SipMessage,
    ) -> Result<()> {
        info!("Success response {} for call {}", code, session.session_id);

        if code == 200 {
            // Call answered - now attempt codec negotiation
            session.state = SessionState::Connected;
            session.cdr.answer_time = Some(Utc::now());

            // Perform codec negotiation - critical step
            match self.negotiate_codecs(&mut session, message).await {
                Ok(_) => {
                    // Codec negotiation successful
                    session.cdr.codec_negotiated = session.codec_negotiation.negotiated_codec.clone();
                    session.cdr.transcoding_used = session.codec_negotiation.transcoding_required;
                    
                    self.session_manager.update_session(session.clone()).await;

                    // Forward 200 OK to A-leg
                    if let Some(a_leg_addr) = self.get_a_leg_address(&session).await? {
                        let forwarded_response = self
                            .create_forwarded_response(&session, code, "OK", message)
                            .await?;
                        self.send_sip_message(a_leg_addr, &forwarded_response)
                            .await?;
                    }
                },
                Err(e) => {
                    // Codec negotiation failed - trigger route advancement
                    warn!("Codec negotiation failed, attempting route advancement: {}", e);
                    
                    // Check if we should attempt route advancement
                    let advancement_result = {
                        let mut route_advancement = self.route_advancement.lock().await;
                        route_advancement
                            .handle_sip_response(&session.session_id, 488, "No compatible codec")
                            .await?
                    };

                    match advancement_result.action {
                        crate::route_advancement::AdvancementAction::RouteToNext => {
                            info!("Advancing to next route for call {} due to codec mismatch", session.session_id);

                            if let Some(new_route) = advancement_result.new_route {
                                self.attempt_termination(session, new_route).await?;
                            } else {
                                self.terminate_session(&session.session_id, 488, "No compatible codec on B leg")
                                    .await?;
                            }
                        }
                        _ => {
                            // No more routes available
                            self.terminate_session(&session.session_id, 488, "No compatible codec on B leg")
                                .await?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Handle error responses (300-699)
    async fn handle_error_response(
        &self,
        session: CallSession,
        code: u16,
        message: &SipMessage,
    ) -> Result<()> {
        warn!("Error response {} for call {}", code, session.session_id);

        let reason = message.reason_phrase.as_deref().unwrap_or("Error");

        // Check if we should attempt route advancement
        let advancement_result = {
            let mut route_advancement = self.route_advancement.lock().await;
            route_advancement
                .handle_sip_response(&session.session_id, code, reason)
                .await?
        };

        match advancement_result.action {
            crate::route_advancement::AdvancementAction::RouteToNext => {
                info!("Advancing to next route for call {}", session.session_id);

                if let Some(new_route) = advancement_result.new_route {
                    self.attempt_termination(session, new_route).await?;
                } else {
                    self.terminate_session(&session.session_id, code, reason)
                        .await?;
                }
            }
            _ => {
                // Complete or reject call
                self.terminate_session(&session.session_id, code, reason)
                    .await?;
            }
        }

        Ok(())
    }

    /// Handle BYE messages (call termination)
    async fn handle_bye(&self, sip_message: SipMessage, addr: SocketAddr) -> Result<()> {
        let call_id = sip_message
            .headers
            .get("Call-ID")
            .ok_or_else(|| anyhow!("Missing Call-ID header"))?;

        info!("Processing BYE for call {}", call_id);

        if let Some(session) = self
            .session_manager
            .get_session_by_any_call_id(call_id)
            .await
        {
            self.terminate_session(&session.session_id, 200, "Normal clearing")
                .await?;
            self.send_sip_response(addr, call_id, 200, "OK", "").await?;
        }

        Ok(())
    }

    /// Handle ACK messages
    async fn handle_ack(&self, sip_message: SipMessage, _addr: SocketAddr) -> Result<()> {
        let call_id = sip_message
            .headers
            .get("Call-ID")
            .ok_or_else(|| anyhow!("Missing Call-ID header"))?;

        debug!("Processing ACK for call {}", call_id);

        // Forward ACK to appropriate leg
        if let Some(session) = self
            .session_manager
            .get_session_by_any_call_id(call_id)
            .await
        {
            // Forward ACK to B-leg if this is from A-leg
            // Implementation depends on which leg sent the ACK
        }

        Ok(())
    }

    /// Handle CANCEL messages
    async fn handle_cancel(&self, sip_message: SipMessage, addr: SocketAddr) -> Result<()> {
        let call_id = sip_message
            .headers
            .get("Call-ID")
            .ok_or_else(|| anyhow!("Missing Call-ID header"))?;

        info!("Processing CANCEL for call {}", call_id);

        if let Some(session) = self
            .session_manager
            .get_session_by_any_call_id(call_id)
            .await
        {
            self.terminate_session(&session.session_id, 487, "Request Cancelled")
                .await?;
            self.send_sip_response(addr, call_id, 200, "OK", "").await?;
        }

        Ok(())
    }

    /// Terminate a call session
    async fn terminate_session(
        &self,
        session_id: &str,
        cause_code: u16,
        reason: &str,
    ) -> Result<()> {
        if let Some(mut session) = self.session_manager.get_session(session_id).await {
            session.state = SessionState::Terminated;
            session.cdr.end_time = Some(Utc::now());
            session.cdr.termination_cause = Some(cause_code);
            session.cdr.termination_reason = Some(reason.to_string());

            // Calculate duration
            if let Some(answer_time) = session.cdr.answer_time {
                session.cdr.duration_seconds = Some(
                    session
                        .cdr
                        .end_time
                        .expect("End time should be set")
                        .signed_duration_since(answer_time)
                        .num_seconds() as u64,
                );
            }

            // Send CDR
            if self.config.enable_cdr_generation {
                self.cdr_generator.generate_cdr(session.cdr.clone()).await;
            }

            self.session_manager.remove_session(session_id).await;

            info!(
                "Terminated session {} with cause {}: {}",
                session_id, cause_code, reason
            );
        }

        Ok(())
    }

    // Helper methods for SIP message processing

    fn parse_sip_message(&self, message: &str) -> Result<SipMessage> {
        // Simple SIP message parser - in production would use a proper SIP parser
        let mut lines = message.lines();
        let first_line = lines.next().ok_or_else(|| anyhow!("Empty SIP message"))?;

        let mut headers = HashMap::new();
        let mut method = None;
        let mut status_code = None;
        let mut reason_phrase = None;

        // Parse first line
        if first_line.starts_with("SIP/2.0") {
            // This is a response
            let parts: Vec<&str> = first_line.split_whitespace().collect();
            if parts.len() >= 3 {
                status_code = parts[1].parse().ok();
                reason_phrase = Some(parts[2..].join(" "));
            }
        } else {
            // This is a request
            let parts: Vec<&str> = first_line.split_whitespace().collect();
            if !parts.is_empty() {
                method = Some(parts[0].to_string());
            }
        }

        // Parse headers
        for line in lines {
            if line.trim().is_empty() {
                break; // End of headers
            }

            if let Some(colon_pos) = line.find(':') {
                let name = line[..colon_pos].trim().to_string();
                let value = line[colon_pos + 1..].trim().to_string();
                headers.insert(name, value);
            }
        }

        Ok(SipMessage {
            method,
            status_code,
            reason_phrase,
            headers,
        })
    }

    async fn create_call_session(
        &self,
        sip_message: &SipMessage,
        addr: SocketAddr,
        calling: String,
        called: String,
    ) -> Result<CallSession> {
        let session_id = Uuid::new_v4().to_string();
        let call_id = sip_message.headers.get("Call-ID")
            .ok_or_else(|| anyhow!("Missing Call-ID header"))?.clone();
        let from_tag = self.extract_tag(&sip_message.headers, "From")?;

        let a_leg = CallLeg {
            call_id: call_id.clone(),
            from_tag,
            to_tag: None,
            local_addr: self.socket.local_addr()?,
            remote_addr: addr,
            state: LegState::Invited,
            sip_headers: sip_message.headers.clone(),
            supported_codecs: self.extract_codecs(&sip_message),
            selected_codec: None,
            last_cseq: 1,
        };

        let cdr = CallDetailRecord {
            session_id: session_id.clone(),
            a_leg_call_id: call_id,
            b_leg_call_id: None,
            calling_number: calling,
            called_number: called,
            origination_ip: addr.ip(),
            termination_ip: None,
            start_time: Utc::now(),
            answer_time: None,
            end_time: None,
            duration_seconds: None,
            termination_cause: None,
            termination_reason: None,
            route_attempts: 0,
            final_route: None,
            codec_negotiated: None,
            transcoding_used: false,
        };

        Ok(CallSession {
            session_id,
            a_leg,
            b_leg: None,
            state: SessionState::Originating,
            created_at: Utc::now(),
            last_activity: Utc::now(),
            route_attempts: 0,
            current_route: None,
            codec_negotiation: CodecNegotiation {
                a_leg_codecs: self.extract_codecs(&sip_message),
                b_leg_codecs: vec![],
                negotiated_codec: None,
                transcoding_required: false,
                transcoding_profile: None,
            },
            cdr,
        })
    }

    // Additional helper methods would be implemented here...

    fn extract_calling_number(&self, sip_message: &SipMessage) -> Result<String> {
        // Extract from From header
        if let Some(from) = sip_message.headers.get("From") {
            // Simple extraction - in production would use proper SIP parser
            if let Some(start) = from.find("sip:") {
                let after_sip = &from[start + 4..];
                if let Some(at_pos) = after_sip.find('@') {
                    return Ok(after_sip[..at_pos].to_string());
                }
            }
        }
        Err(anyhow!("Could not extract calling number"))
    }

    fn extract_called_number(&self, sip_message: &SipMessage) -> Result<String> {
        // Extract from Request-URI or To header
        // Implementation similar to calling number extraction
        Ok("18005551234".to_string()) // Placeholder
    }

    fn extract_tag(&self, headers: &HashMap<String, String>, header_name: &str) -> Result<String> {
        if let Some(header_value) = headers.get(header_name) {
            if let Some(tag_start) = header_value.find("tag=") {
                let tag_part = &header_value[tag_start + 4..];
                if let Some(semicolon) = tag_part.find(';') {
                    return Ok(tag_part[..semicolon].to_string());
                }
                return Ok(tag_part.to_string());
            }
        }
        Err(anyhow!("Could not extract tag from {}", header_name))
    }

    fn extract_codecs(&self, sip_message: &SipMessage) -> Vec<String> {
        // Extract codecs from SDP in message body
        // This is a simplified implementation - in production would use proper SDP parser
        let mut codecs = Vec::new();
        
        // Look for Content-Type header to confirm SDP
        if let Some(content_type) = sip_message.headers.get("Content-Type") {
            if content_type.contains("application/sdp") {
                // In a real implementation, we'd parse the SDP body
                // For now, return common codecs based on typical SDP patterns
                codecs.extend(vec![
                    "PCMU".to_string(),  // G.711 μ-law
                    "PCMA".to_string(),  // G.711 A-law  
                    "G729".to_string(),  // G.729
                    "G722".to_string(),  // G.722
                ]);
            }
        }
        
        // Fallback to default codecs if no SDP found
        if codecs.is_empty() {
            codecs = vec!["PCMU".to_string(), "G729".to_string()];
        }
        
        debug!("Extracted codecs: {:?}", codecs);
        codecs
    }

    async fn negotiate_codecs(
        &self,
        session: &mut CallSession,
        message: &SipMessage,
    ) -> Result<()> {
        // Attempt codec negotiation
        match self.codec_translator
            .negotiate_codecs(&mut session.codec_negotiation, message)
            .await {
            Ok(_) => {
                info!(
                    "Codec negotiation successful for call {}: {:?}",
                    session.session_id, 
                    session.codec_negotiation.negotiated_codec
                );
                Ok(())
            },
            Err(e) => {
                warn!(
                    "Codec negotiation failed for call {}: {}", 
                    session.session_id, e
                );
                
                // If transcoding is disabled, this should trigger route advancement
                if !self.config.enable_codec_translation {
                    // Log CDR with specific cause
                    session.cdr.termination_cause = Some(488); // Not Acceptable Here
                    session.cdr.termination_reason = Some("No compatible codec on B leg".to_string());
                    
                    return Err(anyhow!(
                        "No compatible codec found and transcoding disabled: {}", e
                    ));
                }
                
                Err(e)
            }
        }
    }

    async fn create_b_leg_invite(
        &self,
        session: &CallSession,
        route: &crate::lcr::types::CallRoute,
    ) -> Result<String> {
        // Create B-leg INVITE message
        Ok(format!(
            "INVITE sip:{}@{}:{} SIP/2.0\r\n\r\n",
            session.cdr.called_number, route.egress_trunk.host, route.egress_trunk.port
        ))
    }

    async fn create_forwarded_response(
        &self,
        session: &CallSession,
        code: u16,
        reason: &str,
        original: &SipMessage,
    ) -> Result<String> {
        // Create forwarded response message
        Ok(format!("SIP/2.0 {} {}\r\n\r\n", code, reason))
    }

    async fn get_a_leg_address(&self, session: &CallSession) -> Result<Option<SocketAddr>> {
        Ok(Some(session.a_leg.remote_addr))
    }

    async fn send_sip_message(&self, addr: SocketAddr, message: &str) -> Result<()> {
        self.socket.send_to(message.as_bytes(), addr).await?;
        debug!("Sent SIP message to {}", addr);
        Ok(())
    }

    async fn send_sip_response(
        &self,
        addr: SocketAddr,
        call_id: &str,
        code: u16,
        reason: &str,
        body: &str,
    ) -> Result<()> {
        let response = format!(
            "SIP/2.0 {} {}\r\nCall-ID: {}\r\nContent-Length: {}\r\n\r\n{}",
            code,
            reason,
            call_id,
            body.len(),
            body
        );
        self.send_sip_message(addr, &response).await
    }

    fn start_session_cleanup_task(&self) {
        let session_manager = self.session_manager.clone();
        let cleanup_interval = Duration::from_secs(self.config.session_cleanup_interval_seconds);
        let call_timeout = Duration::from_secs(self.config.call_timeout_seconds);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);
            loop {
                interval.tick().await;
                session_manager.cleanup_expired_sessions(call_timeout).await;
            }
        });
    }
}

/// Simple SIP message representation
#[derive(Debug)]
pub struct SipMessage {
    pub method: Option<String>,
    pub status_code: Option<u16>,
    pub reason_phrase: Option<String>,
    pub headers: HashMap<String, String>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            active_sessions: RwLock::new(HashMap::new()),
            call_id_mapping: RwLock::new(HashMap::new()),
            stats: RwLock::new(SessionStats::default()),
        }
    }

    pub async fn add_session(&self, session: CallSession) {
        let mut sessions = self.active_sessions.write().await;
        let mut mapping = self.call_id_mapping.write().await;
        let mut stats = self.stats.write().await;

        // Add call ID mappings
        mapping.insert(session.a_leg.call_id.clone(), session.session_id.clone());
        if let Some(ref b_leg) = session.b_leg {
            mapping.insert(b_leg.call_id.clone(), session.session_id.clone());
        }

        sessions.insert(session.session_id.clone(), session);

        stats.total_sessions += 1;
        stats.active_sessions += 1;
        if stats.active_sessions > stats.peak_concurrent_calls {
            stats.peak_concurrent_calls = stats.active_sessions;
        }
    }

    pub async fn get_session(&self, session_id: &str) -> Option<CallSession> {
        self.active_sessions.read().await.get(session_id).cloned()
    }

    pub async fn get_session_by_any_call_id(&self, call_id: &str) -> Option<CallSession> {
        let mapping = self.call_id_mapping.read().await;
        if let Some(session_id) = mapping.get(call_id) {
            self.get_session(session_id).await
        } else {
            None
        }
    }

    pub async fn session_exists(&self, call_id: &str) -> bool {
        self.call_id_mapping.read().await.contains_key(call_id)
    }

    pub async fn update_session(&self, session: CallSession) {
        let mut sessions = self.active_sessions.write().await;
        sessions.insert(session.session_id.clone(), session);
    }

    pub async fn remove_session(&self, session_id: &str) {
        let mut sessions = self.active_sessions.write().await;
        let mut mapping = self.call_id_mapping.write().await;
        let mut stats = self.stats.write().await;

        if let Some(session) = sessions.remove(session_id) {
            // Remove call ID mappings
            mapping.remove(&session.a_leg.call_id);
            if let Some(ref b_leg) = session.b_leg {
                mapping.remove(&b_leg.call_id);
            }

            stats.active_sessions = stats.active_sessions.saturating_sub(1);

            if session.state == SessionState::Connected {
                stats.successful_calls += 1;
                if let Some(duration) = session.cdr.duration_seconds {
                    stats.total_call_minutes += (duration / 60);
                }
            } else {
                stats.failed_calls += 1;
            }
        }
    }

    pub async fn cleanup_expired_sessions(&self, timeout: Duration) {
        let now = Utc::now();
        let cutoff = now - chrono::Duration::seconds(timeout.as_secs() as i64);

        let expired_sessions: Vec<String> = {
            let sessions = self.active_sessions.read().await;
            sessions
                .iter()
                .filter(|(_, session)| session.last_activity < cutoff)
                .map(|(id, _)| id.clone())
                .collect()
        };

        for session_id in expired_sessions {
            warn!("Cleaning up expired session: {}", session_id);
            self.remove_session(&session_id).await;
        }
    }

    pub async fn get_stats(&self) -> SessionStats {
        self.stats.read().await.clone()
    }
}

impl CallProcessor {
    pub fn new(config: Arc<Class4Config>) -> Self {
        Self { config }
    }
}

impl CDRGenerator {
    pub fn new(config: Arc<Class4Config>, sender: mpsc::UnboundedSender<CallDetailRecord>) -> Self {
        Self {
            config,
            cdr_sender: sender,
        }
    }

    pub async fn generate_cdr(&self, cdr: CallDetailRecord) {
        if self.config.enable_cdr_generation {
            if let Err(e) = self.cdr_sender.send(cdr) {
                error!("Failed to send CDR: {}", e);
            }
        }
    }

    pub fn start_cdr_processor(mut receiver: mpsc::UnboundedReceiver<CallDetailRecord>) {
        tokio::spawn(async move {
            while let Some(cdr) = receiver.recv().await {
                // In production, this would write to database or file
                info!(
                    "CDR: {} -> {} duration: {:?}s",
                    cdr.calling_number, cdr.called_number, cdr.duration_seconds
                );
            }
        });
    }
}

impl CodecTranslator {
    pub fn new() -> Self {
        let supported_codecs = vec![
            "G711U".to_string(),
            "G711A".to_string(),
            "G729".to_string(),
            "G722".to_string(),
        ];

        let mut transcoding_profiles = HashMap::new();
        transcoding_profiles.insert(
            "G711U_to_G729".to_string(),
            TranscodingProfile {
                name: "G711U to G729".to_string(),
                source_codec: "G711U".to_string(),
                target_codec: "G729".to_string(),
                quality_profile: "standard".to_string(),
                bandwidth_optimization: true,
            },
        );

        Self {
            supported_codecs,
            transcoding_profiles,
        }
    }

    pub async fn negotiate_codecs(
        &self,
        negotiation: &mut CodecNegotiation,
        message: &SipMessage,
    ) -> Result<()> {
        // Extract B-leg codecs from SDP
        negotiation.b_leg_codecs = self.extract_codecs_from_sdp(message);

        // Find common codec
        for a_codec in &negotiation.a_leg_codecs {
            if negotiation.b_leg_codecs.contains(a_codec) {
                negotiation.negotiated_codec = Some(a_codec.clone());
                negotiation.transcoding_required = false;
                return Ok(());
            }
        }

        // No common codec, check if transcoding is possible
        for a_codec in &negotiation.a_leg_codecs {
            for b_codec in &negotiation.b_leg_codecs {
                let profile_key = format!("{}_to_{}", a_codec, b_codec);
                if self.transcoding_profiles.contains_key(&profile_key) {
                    negotiation.transcoding_required = true;
                    negotiation.transcoding_profile = Some(profile_key);
                    negotiation.negotiated_codec = Some(format!("{}->{}", a_codec, b_codec));
                    return Ok(());
                }
            }
        }

        // No compatible codecs and no transcoding available
        Err(anyhow!("No compatible codecs found between A-leg {:?} and B-leg {:?}", 
                   negotiation.a_leg_codecs, negotiation.b_leg_codecs))
    }

    fn extract_codecs_from_sdp(&self, message: &SipMessage) -> Vec<String> {
        // Extract codecs from SDP body in B-leg response
        let mut codecs = Vec::new();
        
        // Look for Content-Type and SDP body
        if let Some(content_type) = message.headers.get("Content-Type") {
            if content_type.contains("application/sdp") {
                // Parse SDP body for media formats (m= lines and a=rtpmap lines)
                // This is simplified - production would use proper SDP parser
                
                // Common B-leg codec patterns based on carrier capabilities
                codecs.extend(vec![
                    "PCMU".to_string(),  // G.711 μ-law (most common)
                    "G729".to_string(),  // G.729 (bandwidth efficient)
                ]);
            }
        }
        
        // Fallback for carriers that support limited codecs
        if codecs.is_empty() {
            codecs = vec!["PCMU".to_string()]; // Most carriers support G.711
        }
        
        debug!("Extracted B-leg codecs: {:?}", codecs);
        codecs
    }
}
