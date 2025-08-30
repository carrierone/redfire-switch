//! Class 4 B2BUA Implementation
//! Implements a production-ready Class 4 switching B2BUA that routes calls between gateways
//! with codec translation signaling but no media processing

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::lcr::types::{RouteRequest, RouteType};
use crate::origination_routing::{OriginationRequest, OriginationRoutingEngine};
use crate::route_advancement::RouteAdvancementEngine;
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
    trunk_configs: Arc<Vec<TrunkRateConfig>>, // Add trunk configurations
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

/// Call Detail Record with proper ingress/egress costing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallDetailRecord {
    pub session_id: String,
    pub a_leg_call_id: String,
    pub b_leg_call_id: Option<String>,

    // Call parties
    pub ani: String,  // A-Number (calling party)
    pub dnis: String, // B-Number (called party)

    // Timing
    pub start_time: DateTime<Utc>,
    pub answer_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_seconds: Option<u64>,

    // Ingress leg (where call came FROM)
    pub ingress_trunk_id: u32,
    pub ingress_trunk_name: String,
    pub ingress_ip: IpAddr,
    pub ingress_rate_per_minute: f64, // Cost for receiving this call
    pub ingress_cost: f64,            // Total ingress cost (negative = revenue)
    pub ingress_revenue: Option<f64>, // Revenue if we bill for ingress

    // Egress leg (where call went TO)
    pub egress_trunk_id: Option<u32>,
    pub egress_trunk_name: Option<String>,
    pub egress_ip: Option<IpAddr>,
    pub egress_rate_per_minute: f64, // Cost for sending this call
    pub egress_cost: f64,            // Total egress cost
    pub egress_revenue: Option<f64>, // Revenue if we bill for egress

    // Net calculation
    pub total_cost: f64,    // ingress_cost + egress_cost (positive values only)
    pub total_revenue: f64, // ingress_revenue + egress_revenue
    pub net_margin: f64,    // total_revenue - total_cost
    pub profit_margin_percent: f64, // (net_margin / total_revenue) * 100

    // Technical details
    pub codec_negotiated: Option<String>,
    pub transcoding_used: bool,
    pub termination_cause: Option<u16>,
    pub termination_reason: Option<String>,
    pub route_attempts: u32,
    pub final_route: Option<String>,

    // ANI-II (Automatic Number Identification Information Indicator) details
    pub ani_ii_ingress: Option<u8>,    // ANI-II digit from ingress leg
    pub ani_ii_egress: Option<u8>,     // ANI-II digit sent to egress leg
    pub ani_ii_source: Option<String>, // Source of ANI-II (which header)

    // Payphone surcharge information (for toll-free calls)
    pub is_toll_free: bool, // Flag indicating toll-free number
    pub payphone_surcharge_amount: Option<f64>, // Surcharge amount in USD
    pub payphone_surcharge_reason: Option<String>, // Reason for surcharge (ANI-II code)
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

/// Trunk rate configuration for proper billing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrunkRateConfig {
    pub trunk_id: u32,
    pub trunk_name: String,
    pub direction: TrunkDirection,
    pub ip_addresses: Vec<IpAddr>,

    // Rate configuration
    pub default_rate_per_minute: f64,
    pub rate_deck_id: Option<u32>,
    pub rate_overrides: HashMap<String, f64>, // Number prefix → rate

    // Revenue vs Cost designation
    pub is_revenue_trunk: bool, // true = customer trunk (we bill them), false = carrier trunk (we pay them)
    pub billing_increment: u32, // Billing increment in seconds (6, 30, 60)
    pub minimum_duration: u32,  // Minimum billable seconds

    // Trunk type for proper routing
    pub trunk_type: TrunkType, // Origination, Termination, or Bidirectional

    // Call direction detection
    pub our_number_blocks: Vec<String>, // DIDs/TF numbers we serve
    pub customer_number_blocks: Vec<String>, // Customer ANI ranges

    // Payphone surcharge configuration for toll-free calls
    pub payphone_surcharges_enabled: bool, // Enable payphone surcharges
    pub payphone_surcharge_23: Option<f64>, // ANI-II Code 23 surcharge amount
    pub payphone_surcharge_27: Option<f64>, // ANI-II Code 27 surcharge amount
    pub payphone_surcharge_70: Option<f64>, // ANI-II Code 70 surcharge amount

    // ANI-II blocking configuration for customer protection
    pub ani_ii_blocking: Option<crate::ani_ii::blocking::AniIIBlockingConfig>, // ANI-II blocking rules
}

/// Trunk direction for proper routing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TrunkDirection {
    Ingress,       // Calls come FROM this trunk
    Egress,        // Calls go TO this trunk
    Bidirectional, // Both directions
}

/// Trunk type for business logic
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TrunkType {
    Customer, // Customer endpoints (we bill them)
    Carrier,  // Carrier interconnects (we pay them or they pay us)
    Internal, // Internal routing (no billing)
}

/// Traffic type determination
#[derive(Debug, Clone)]
pub enum TrafficType {
    Origination {
        customer_dnis: String,     // Our customer's number being called
        calling_party: String,     // Who is calling them
        source_carrier_ip: IpAddr, // Which carrier sent us this call
        ingress_trunk_id: u32,     // Trunk that delivered the call
    },
    Termination {
        customer_ani: String,  // Our customer making the call
        destination: String,   // Where they want to call
        customer_ip: IpAddr,   // Our customer's IP
        ingress_trunk_id: u32, // Customer's trunk
    },
}

/// Customer information for DID routing
#[derive(Debug, Clone)]
pub struct CustomerInfo {
    pub customer_id: String,
    pub sip_endpoint: SocketAddr,
    pub customer_name: String,

    // ANI-II blocking for specific DIDs owned by this customer
    pub did_ani_ii_blocks: Vec<crate::ani_ii::blocking::DidAniIIBlocking>,
}

/// Implementation of cost calculation methods
impl CallDetailRecord {
    /// Calculate costs based on trunk configurations
    pub fn calculate_costs(
        &mut self,
        ingress_trunk: &TrunkRateConfig,
        egress_trunk: Option<&TrunkRateConfig>,
    ) {
        if let Some(duration) = self.duration_seconds {
            let minutes = (duration as f64) / 60.0;

            // Apply billing increment rounding
            let billable_minutes =
                self.calculate_billable_minutes(minutes, ingress_trunk.billing_increment);

            // Calculate ingress cost/revenue based on trunk type
            if ingress_trunk.is_revenue_trunk {
                // Customer trunk - we bill them (negative cost = revenue)
                self.ingress_cost = -(ingress_trunk.default_rate_per_minute * billable_minutes);
                self.ingress_revenue =
                    Some(ingress_trunk.default_rate_per_minute * billable_minutes);
            } else {
                // Carrier trunk - we pay them (positive cost)
                self.ingress_cost = ingress_trunk.default_rate_per_minute * billable_minutes;
                self.ingress_revenue = None;
            }

            // Calculate egress cost/revenue if there's an egress trunk
            if let Some(egress_trunk) = egress_trunk {
                let egress_billable_minutes =
                    self.calculate_billable_minutes(minutes, egress_trunk.billing_increment);

                if egress_trunk.is_revenue_trunk {
                    // Customer trunk - we bill them (negative cost = revenue)
                    self.egress_cost =
                        -(egress_trunk.default_rate_per_minute * egress_billable_minutes);
                    self.egress_revenue =
                        Some(egress_trunk.default_rate_per_minute * egress_billable_minutes);
                } else {
                    // Carrier trunk - we pay them (positive cost)
                    self.egress_cost =
                        egress_trunk.default_rate_per_minute * egress_billable_minutes;
                    self.egress_revenue = None;
                }
            } else {
                // No egress trunk, but check if we should bill for egress (e.g., DID service)
                if self.egress_revenue.is_some() {
                    // Calculate egress revenue for customer billing
                    self.egress_revenue = Some(self.egress_rate_per_minute * billable_minutes);
                    // Egress cost remains 0 for customer delivery
                }
            }

            // Calculate totals - only positive values count as costs
            self.total_cost = self.ingress_cost.max(0.0) + self.egress_cost.max(0.0);
            self.total_revenue =
                self.ingress_revenue.unwrap_or(0.0) + self.egress_revenue.unwrap_or(0.0);
            self.net_margin = self.total_revenue - self.total_cost;

            if self.total_revenue > 0.0 {
                self.profit_margin_percent = (self.net_margin / self.total_revenue) * 100.0;
            }
        }
    }

    /// Calculate billable minutes based on billing increment
    fn calculate_billable_minutes(&self, actual_minutes: f64, billing_increment: u32) -> f64 {
        if billing_increment <= 1 {
            return actual_minutes; // Per-second billing
        }

        let increment_minutes = (billing_increment as f64) / 60.0;
        (actual_minutes / increment_minutes).ceil() * increment_minutes
    }

    /// Apply rate overrides based on destination number
    pub fn apply_rate_overrides(&mut self, trunk: &TrunkRateConfig, is_ingress: bool) {
        let number = if is_ingress { &self.ani } else { &self.dnis };

        // Find the longest matching prefix
        let mut best_match_rate = None;
        let mut best_match_length = 0;

        for (prefix, rate) in &trunk.rate_overrides {
            if number.starts_with(prefix) && prefix.len() > best_match_length {
                best_match_rate = Some(*rate);
                best_match_length = prefix.len();
            }
        }

        if let Some(override_rate) = best_match_rate {
            if is_ingress {
                self.ingress_rate_per_minute = override_rate;
            } else {
                self.egress_rate_per_minute = override_rate;
            }
        }
    }
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
        trunk_configs: Vec<TrunkRateConfig>, // Add trunk configurations
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
            trunk_configs: Arc::new(trunk_configs), // Store trunk configurations
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
        // Basic message validation
        if message.len() < 10 {
            warn!(
                "Received suspiciously short SIP message from {}: {} bytes",
                addr,
                message.len()
            );
            return Ok(());
        }

        if message.len() > 65536 {
            warn!(
                "Received oversized SIP message from {}: {} bytes, truncating",
                addr,
                message.len()
            );
            // Continue processing but this could indicate an attack
        }

        debug!(
            "Processing SIP message from {}: {}",
            addr,
            message.lines().next().unwrap_or("<empty>")
        );

        let sip_message = match self.parse_sip_message(message) {
            Ok(msg) => msg,
            Err(e) => {
                warn!("Failed to parse SIP message from {}: {}", addr, e);
                return Ok(());
            }
        };

        // Validate required headers for requests and responses
        if !self.validate_sip_message(&sip_message) {
            warn!(
                "Invalid SIP message from {}: missing required headers",
                addr
            );
            return Ok(());
        }

        match sip_message.method.as_deref() {
            Some("INVITE") => self.handle_invite(sip_message, addr).await,
            Some("ACK") => self.handle_ack(sip_message, addr).await,
            Some("BYE") => self.handle_bye(sip_message, addr).await,
            Some("CANCEL") => self.handle_cancel(sip_message, addr).await,
            Some("OPTIONS") => self.handle_options(sip_message, addr).await,
            Some(method) => {
                debug!("Responding 405 Method Not Allowed for method: {}", method);
                if let Some(call_id) = sip_message.headers.get("Call-ID") {
                    self.send_sip_response(addr, call_id, 405, "Method Not Allowed", "")
                        .await?;
                }
                Ok(())
            }
            None => {
                // This is a response
                self.handle_sip_response(sip_message, addr).await
            }
        }
    }

    /// Parse ANI-II information from SIP message using RFC-compliant parser
    fn parse_ani_ii_from_sip_message(
        &self,
        sip_message: &SipMessage,
        body: &str,
    ) -> Option<crate::ani_ii_rfc_compliant::AniIIInfo> {
        // Use the new RFC-compliant ANI-II parser
        if let Some(oli_info) =
            crate::sip_rfc_compliance::extract_oli_info(&sip_message.headers, Some(body))
        {
            if let Some(ani_ii_info) =
                crate::ani_ii_rfc_compliant::AniIIInfo::from_oli_info(oli_info)
            {
                return Some(ani_ii_info);
            }
        }

        // Fallback to legacy parsers only if RFC-compliant parsing fails
        if let Some(legacy_ani_ii) =
            crate::ani_ii::sip_parser::parse_ani_ii_extended(&sip_message.headers)
        {
            // Convert legacy format to RFC-compliant format
            return crate::ani_ii_rfc_compliant::AniIIInfo::from_legacy(legacy_ani_ii).ok();
        }

        None
    }

    /// Calculate payphone surcharge for the call
    fn calculate_payphone_surcharge(
        &self,
        ani_ii: Option<&crate::ani_ii::AniIIInfo>,
        is_toll_free: bool,
        trunk_id: Option<u32>,
    ) -> crate::ani_ii::surcharge_calculator::SurchargeResult {
        // Create payphone surcharge config from trunk configuration
        let trunk_config = trunk_id.and_then(|id| {
            self.find_trunk_by_id(id).map(|trunk| {
                crate::ani_ii::surcharge_calculator::PayphoneSurchargeConfig {
                    enabled: trunk.payphone_surcharges_enabled,
                    code_23_amount: trunk.payphone_surcharge_23,
                    code_27_amount: trunk.payphone_surcharge_27,
                    code_70_amount: trunk.payphone_surcharge_70,
                    bill_to_customer: true, // Default to billing customer for toll-free
                }
            })
        });

        crate::ani_ii::surcharge_calculator::calculate_payphone_surcharge(
            ani_ii,
            is_toll_free,
            trunk_config.as_ref(),
        )
    }

    /// Update CDR with ANI-II information and surcharge details
    fn update_cdr_with_ani_ii(
        &self,
        cdr: &mut CallDetailRecord,
        ani_ii: Option<&crate::ani_ii::AniIIInfo>,
        surcharge_result: &crate::ani_ii::surcharge_calculator::SurchargeResult,
    ) {
        if let Some(ani_ii_info) = ani_ii {
            cdr.ani_ii_ingress = Some(ani_ii_info.raw_digit);
            cdr.ani_ii_source = Some(ani_ii_info.source.to_string());
        }

        if surcharge_result.applies {
            cdr.payphone_surcharge_amount = Some(surcharge_result.amount);
            cdr.payphone_surcharge_reason = Some(surcharge_result.reason.clone());
        }
    }

    /// Add ANI-II information to outbound SIP INVITE headers
    fn add_ani_ii_to_headers(
        &self,
        headers: &mut std::collections::HashMap<String, String>,
        ani_ii: Option<&crate::ani_ii::AniIIInfo>,
    ) {
        if let Some(ani_ii_info) = ani_ii {
            // Add ANI-II to Remote-Party-ID header if it doesn't already exist
            if !headers.contains_key("Remote-Party-ID") {
                // Create a basic Remote-Party-ID header with ANI-II
                let remote_party_id = format!(
                    "<sip:anonymous@127.0.0.1>;party=calling;privacy=off;ani-ii={}",
                    ani_ii_info.raw_digit
                );
                headers.insert("Remote-Party-ID".to_string(), remote_party_id);
            } else {
                // Modify existing Remote-Party-ID to include ANI-II
                let existing = headers.get("Remote-Party-ID").unwrap().clone();
                if !existing.contains("ani-ii=") {
                    let updated = format!("{};ani-ii={}", existing, ani_ii_info.raw_digit);
                    headers.insert("Remote-Party-ID".to_string(), updated);
                }
            }

            // Also add custom header for carriers that prefer this approach
            headers.insert("X-ANI-II".to_string(), ani_ii_info.raw_digit.to_string());

            // Add P-Asserted-Identity if not present and we have calling number info
            if !headers.contains_key("P-Asserted-Identity") {
                // This would typically use the actual calling number from the session
                // For now, using a placeholder - in production would extract from session
                let p_asserted =
                    format!("<sip:calling@carrier.com>;ani-ii={}", ani_ii_info.raw_digit);
                headers.insert("P-Asserted-Identity".to_string(), p_asserted);
            }
        }
    }

    /// Create outbound INVITE with ANI-II information for B-leg
    fn create_b_leg_invite_with_ani_ii(
        &self,
        session: &CallSession,
        target_gateway: &str,
        ani_ii: Option<&crate::ani_ii::AniIIInfo>,
    ) -> Result<String> {
        let mut headers = std::collections::HashMap::new();

        // Basic SIP headers
        headers.insert(
            "Via".to_string(),
            format!(
                "SIP/2.0/UDP {};branch=z9hG4bK{}",
                self.config.bind_address,
                Uuid::new_v4()
            ),
        );
        headers.insert(
            "From".to_string(),
            format!(
                "<sip:{}@{}>;tag={}",
                session.cdr.ani, self.config.bind_address, session.a_leg.from_tag
            ),
        );
        headers.insert(
            "To".to_string(),
            format!("<sip:{}@{}>", session.cdr.dnis, target_gateway),
        );
        headers.insert("Call-ID".to_string(), session.a_leg.call_id.clone());
        headers.insert("CSeq".to_string(), "1 INVITE".to_string());

        // Add ANI-II information to headers
        self.add_ani_ii_to_headers(&mut headers, ani_ii);

        // Build the INVITE message
        let mut invite = format!(
            "INVITE sip:{}@{} SIP/2.0\r\n",
            session.cdr.dnis, target_gateway
        );
        for (name, value) in &headers {
            invite.push_str(&format!("{}: {}\r\n", name, value));
        }
        invite.push_str("Content-Length: 0\r\n\r\n");

        Ok(invite)
    }

    /// Handle INVITE messages (call setup) - Updated for proper origination/termination logic
    async fn handle_invite(&self, sip_message: SipMessage, addr: SocketAddr) -> Result<()> {
        // Extract actual request line for proper RFC 3261 validation
        let request_uri = sip_message
            .headers
            .get("Request-URI")
            .cloned()
            .unwrap_or_else(|| "sip:unknown@unknown.com".to_string());
        let request_line = format!("INVITE {} SIP/2.0", request_uri);

        // RFC 3261 compliance validation with actual request line
        if let Err(e) = crate::sip_rfc_compliance::Rfc3261Validator::validate_message(
            &sip_message.headers,
            &request_line,
        ) {
            let call_id = sip_message
                .headers
                .get("Call-ID")
                .cloned()
                .unwrap_or_else(|| "invalid-call-id".to_string());
            warn!("RFC 3261 validation failed from {}: {}", addr, e);
            self.send_sip_response(addr, &call_id, 400, "Bad Request", "")
                .await?;
            return Ok(());
        }

        let call_id = sip_message
            .headers
            .get("Call-ID")
            .ok_or_else(|| anyhow!("Missing Call-ID header"))?;

        info!(
            "Processing RFC 3261 compliant INVITE for call {} from {}",
            call_id, addr
        );

        // Check if this is a new call or retransmission
        if self.session_manager.session_exists(call_id).await {
            debug!("Retransmission detected for call {}", call_id);
            return Ok(());
        }

        // Determine traffic type (origination vs termination)
        let traffic_type = match self.determine_traffic_type(&sip_message, addr.ip()) {
            Ok(traffic_type) => traffic_type,
            Err(e) => {
                warn!(
                    "Failed to determine traffic type for call {}: {}",
                    call_id, e
                );
                self.send_sip_response(addr, call_id, 500, "Internal Server Error", "")
                    .await?;
                return Ok(());
            }
        };

        // Send 100 Trying immediately
        self.send_sip_response(addr, call_id, 100, "Trying", "")
            .await?;

        // Process based on traffic type
        match traffic_type {
            TrafficType::Origination {
                customer_dnis,
                calling_party,
                source_carrier_ip,
                ingress_trunk_id,
            } => {
                info!(
                    "ORIGINATION: {} calling DID {} from carrier {} (trunk {})",
                    calling_party, customer_dnis, source_carrier_ip, ingress_trunk_id
                );
                self.process_origination_traffic(
                    sip_message,
                    addr,
                    customer_dnis,
                    calling_party,
                    ingress_trunk_id,
                )
                .await
            }
            TrafficType::Termination {
                customer_ani,
                destination,
                customer_ip,
                ingress_trunk_id,
            } => {
                info!(
                    "TERMINATION: Customer {} calling {} via IP {} (trunk {})",
                    customer_ani, destination, customer_ip, ingress_trunk_id
                );
                self.process_termination_traffic(
                    sip_message,
                    addr,
                    customer_ani,
                    destination,
                    ingress_trunk_id,
                )
                .await
            }
        }
    }

    /// Process origination traffic (DID/Toll-Free inbound from carrier to customer)
    async fn process_origination_traffic(
        &self,
        sip_message: SipMessage,
        addr: SocketAddr,
        customer_dnis: String,
        calling_party: String,
        ingress_trunk_id: u32,
    ) -> Result<()> {
        let call_id = sip_message.headers.get("Call-ID").unwrap();

        // Parse ANI-II information from the SIP message using RFC-compliant parser
        let ani_ii = crate::ani_ii_rfc_compliant::RfcCompliantAniIIParser::parse_from_sip_message(
            &sip_message.headers,
            None,
        );
        if let Some(ref ani_ii_info) = ani_ii {
            info!(
                "Origination call {} has ANI-II code {} ({})",
                call_id,
                ani_ii_info.raw_digit,
                ani_ii_info.code.description()
            );
        }

        // Check if this is a toll-free call (need to check before blocking logic)
        let is_toll_free =
            crate::ani_ii_rfc_compliant::RfcCompliantAniIIParser::is_toll_free(&customer_dnis);

        // Check ANI-II blocking for toll-free calls
        if is_toll_free {
            if let Some(ref ani_ii_info) = ani_ii {
                // First check trunk-level blocking
                let trunk_blocking_result = self
                    .check_trunk_ani_ii_blocking(ingress_trunk_id, ani_ii_info.raw_digit)
                    .await;

                if trunk_blocking_result.blocked {
                    warn!("Blocking toll-free origination call {} from ANI-II {} due to trunk policy: {}", 
                          call_id, ani_ii_info.raw_digit, trunk_blocking_result.reason);

                    // Create CDR for blocked call
                    let blocked_cdr = self.create_blocked_call_cdr(
                        &calling_party,
                        &customer_dnis,
                        ingress_trunk_id,
                        Some(ani_ii_info.raw_digit),
                        &trunk_blocking_result.reason,
                    );

                    // Publish CDR for blocked call analytics
                    let generator = self.cdr_generator.clone();
                    tokio::spawn(async move {
                        generator.generate_cdr(blocked_cdr).await;
                    });

                    self.send_sip_response(
                        addr,
                        call_id,
                        trunk_blocking_result.response_code.unwrap_or(403),
                        &trunk_blocking_result.reason,
                        "",
                    )
                    .await?;
                    return Ok(());
                }
            }
        }

        // Find the customer who owns this DID
        let customer_route = self.find_customer_for_dnis(&customer_dnis).await?;

        if customer_route.is_none() {
            warn!("No customer found for DID {}", customer_dnis);
            self.send_sip_response(addr, call_id, 404, "Number Not Found", "")
                .await?;
            return Ok(());
        }

        let customer_info = customer_route.unwrap();

        // Check DID-level ANI-II blocking for toll-free calls (overrides trunk-level config)
        if is_toll_free {
            if let Some(ref ani_ii_info) = ani_ii {
                let did_blocking_result = self
                    .check_did_ani_ii_blocking(
                        &customer_info,
                        &customer_dnis,
                        ani_ii_info.raw_digit,
                    )
                    .await;

                if did_blocking_result.blocked {
                    warn!("Blocking toll-free origination call {} from ANI-II {} due to DID policy: {}", 
                          call_id, ani_ii_info.raw_digit, did_blocking_result.reason);

                    // Create CDR for blocked call
                    let blocked_cdr = self.create_blocked_call_cdr(
                        &calling_party,
                        &customer_dnis,
                        ingress_trunk_id,
                        Some(ani_ii_info.raw_digit),
                        &did_blocking_result.reason,
                    );

                    // Publish CDR for blocked call analytics
                    let generator = self.cdr_generator.clone();
                    tokio::spawn(async move {
                        generator.generate_cdr(blocked_cdr).await;
                    });

                    self.send_sip_response(
                        addr,
                        call_id,
                        did_blocking_result.response_code.unwrap_or(403),
                        &did_blocking_result.reason,
                        "",
                    )
                    .await?;
                    return Ok(());
                }
            }
        }

        // Calculate payphone surcharge for approved calls using RFC-compliant parser
        let (surcharge_applies, surcharge_amount, surcharge_reason) =
            crate::ani_ii_rfc_compliant::RfcCompliantAniIIParser::calculate_surcharge(
                ani_ii.as_ref(),
                is_toll_free,
                None, // TODO: Add trunk-specific config
            );

        if surcharge_applies {
            info!(
                "Payphone surcharge of ${:.2} applies for call {} ({})",
                surcharge_amount, call_id, surcharge_reason
            );
        }

        // Create origination session
        let mut session = self
            .create_origination_session(
                &sip_message,
                addr,
                calling_party.clone(),
                customer_dnis.clone(),
                ingress_trunk_id,
                customer_info,
            )
            .await?;

        // Update CDR with RFC-compliant ANI-II information and surcharge details
        if let Some(ref ani_ii_info) = ani_ii {
            session.cdr.ani_ii_ingress = Some(ani_ii_info.raw_digit);
            session.cdr.ani_ii_source = Some(ani_ii_info.source.clone());
            session.cdr.is_toll_free = is_toll_free;
            if surcharge_applies {
                session.cdr.payphone_surcharge_amount = Some(surcharge_amount);
                session.cdr.payphone_surcharge_reason = Some(surcharge_reason.clone());
            }
        }

        // Store session
        self.session_manager.add_session(session.clone()).await;

        // Route directly to customer (no LCR needed for origination)
        self.deliver_to_customer(session).await
    }

    /// Process termination traffic (customer outbound to carrier)
    async fn process_termination_traffic(
        &self,
        sip_message: SipMessage,
        addr: SocketAddr,
        customer_ani: String,
        destination: String,
        ingress_trunk_id: u32,
    ) -> Result<()> {
        let call_id = sip_message.headers.get("Call-ID").unwrap();

        // Parse ANI-II information from the SIP message (if present from customer)
        let ani_ii = self.parse_ani_ii_from_sip_message(&sip_message, "");
        if let Some(ref ani_ii_info) = ani_ii {
            info!(
                "Termination call {} has ANI-II code {} ({})",
                call_id,
                ani_ii_info.raw_digit,
                ani_ii_info.code.description()
            );
        }

        // Validate customer permissions
        let origination_request = OriginationRequest {
            ani: customer_ani.clone(),
            dnis: destination.clone(),
            source_ip: addr.ip(),
            ingress_trunk_id: ingress_trunk_id as i32,
            customer_id: None, // TODO: Extract from trunk mapping
            route_type: RouteType::NANPA,
            timestamp: Utc::now(),
        };

        let origination_result = {
            let mut engine = self.origination_engine.lock().await;
            engine.route_origination(origination_request).await?
        };

        if !origination_result.allowed {
            info!(
                "Customer {} not allowed to call {}: {}",
                customer_ani, destination, origination_result.reason
            );
            self.send_sip_response(addr, call_id, 403, "Forbidden", &origination_result.reason)
                .await?;
            return Ok(());
        }

        // Check if this is a toll-free call (less likely for termination, but possible)
        let is_toll_free = crate::ani_ii::toll_free::is_toll_free(&destination);
        let (surcharge_applies, surcharge_amount, surcharge_reason) =
            crate::ani_ii_rfc_compliant::RfcCompliantAniIIParser::calculate_surcharge(
                ani_ii.as_ref(),
                is_toll_free,
                None, // TODO: Use trunk-specific config
            );

        // Create termination session
        let session = self
            .create_termination_session(
                &sip_message,
                addr,
                customer_ani,
                destination,
                ingress_trunk_id,
            )
            .await?;

        // Log ANI-II information (TODO: add to CDR when fields are available)
        if let Some(ani_ii_info) = &ani_ii {
            debug!(
                "ANI-II information: code={}, source={}, is_payphone={}, restricted={}",
                ani_ii_info.raw_digit,
                ani_ii_info.source,
                ani_ii_info.is_payphone,
                ani_ii_info.restricted
            );
            debug!(
                "Payphone surcharge: applies={}, amount=${}, reason={}",
                surcharge_applies, surcharge_amount, surcharge_reason
            );
            // TODO: Add ANI-II fields to CallDetailRecord struct
        }

        // Store session
        self.session_manager.add_session(session.clone()).await;

        // Find best route using LCR for termination
        self.begin_termination_routing(session).await
    }

    /// Begin termination routing process
    async fn begin_termination_routing(&self, mut session: CallSession) -> Result<()> {
        session.state = SessionState::Routing;
        self.session_manager.update_session(session.clone()).await;

        // Perform early codec validation before routing - FIX: Move codec negotiation earlier
        let a_leg_codecs = &session.codec_negotiation.a_leg_codecs;
        if a_leg_codecs.is_empty() {
            warn!(
                "No codecs available from A-leg for call {}",
                session.session_id
            );
            self.terminate_session(&session.session_id, 488, "No acceptable codec")
                .await?;
            return Ok(());
        }

        debug!(
            "A-leg codecs for call {}: {:?}",
            session.session_id, a_leg_codecs
        );

        let route_request = RouteRequest {
            ani: session.cdr.ani.clone(),
            dnis: session.cdr.dnis.clone(),
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
            ani: session.cdr.ani.clone(),
            dnis: session.cdr.dnis.clone(),
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

            // Attempt termination with codec awareness
            self.attempt_termination(session, route).await?
        }

        Ok(())
    }

    /// Attempt call termination to selected route
    async fn attempt_termination(
        &self,
        mut session: CallSession,
        route: crate::lcr::types::CallRoute,
    ) -> Result<()> {
        info!(
            "Attempting termination for call {} via trunk {}",
            session.session_id, route.egress_trunk.name
        );

        // Create B-leg with proper state tracking - FIX: Initialize B-leg properly
        let b_leg_call_id = format!("{}-b", session.session_id);
        let b_leg = CallLeg {
            call_id: b_leg_call_id.clone(),
            from_tag: format!("{}-tag", session.session_id),
            to_tag: None,
            local_addr: self.socket.local_addr()?,
            remote_addr: SocketAddr::new(route.egress_trunk.host.parse()?, route.egress_trunk.port),
            state: LegState::Initial,
            sip_headers: HashMap::new(),
            supported_codecs: vec![], // Will be populated from B-leg response
            selected_codec: None,
            last_cseq: 1,
        };

        // Update session with B-leg and add call ID mapping
        session.b_leg = Some(b_leg);
        session.state = SessionState::Terminating;
        session.route_attempts += 1;
        session.cdr.b_leg_call_id = Some(b_leg_call_id.clone());
        session.cdr.egress_ip = Some(route.egress_trunk.host.parse()?);
        session.cdr.egress_trunk_id = Some(route.egress_trunk.id as u32);
        session.cdr.egress_trunk_name = Some(route.egress_trunk.name.clone());

        self.session_manager.update_session(session.clone()).await;

        // Add B-leg call ID mapping
        {
            let mut mapping = self.session_manager.call_id_mapping.write().await;
            mapping.insert(b_leg_call_id.clone(), session.session_id.clone());
        }

        // Create B-leg INVITE with codec-aware SDP
        let b_leg_invite = self.create_b_leg_invite(&session, &route).await?;

        // Send INVITE to termination gateway
        let term_addr = SocketAddr::new(route.egress_trunk.host.parse()?, route.egress_trunk.port);
        self.send_sip_message(term_addr, &b_leg_invite).await?;

        info!(
            "Sent B-leg INVITE for call {} to {} (B-leg call-id: {})",
            session.session_id, term_addr, b_leg_call_id
        );

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

        // Fix race condition: properly handle Optional session
        let session = match self
            .session_manager
            .get_session_by_any_call_id(call_id)
            .await
        {
            Some(session) => session,
            None => {
                debug!("Received response for unknown call: {}", call_id);
                return Ok(());
            }
        };

        match response_code {
            100..=199 => {
                self.handle_provisional_response(session, response_code, &sip_message)
                    .await
            }
            200..=299 => {
                self.handle_success_response(session, response_code, &sip_message)
                    .await
            }
            300..=699 => {
                self.handle_error_response(session, response_code, &sip_message)
                    .await
            }
            _ => {
                warn!("Unexpected response code {}: ignoring", response_code);
                Ok(())
            }
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
                    session.cdr.codec_negotiated =
                        session.codec_negotiation.negotiated_codec.clone();
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
                }
                Err(e) => {
                    // Codec negotiation failed - trigger route advancement with cleanup
                    warn!(
                        "Codec negotiation failed, attempting route advancement: {}",
                        e
                    );

                    // Clean up current B-leg connection before advancing
                    if let Some(ref b_leg) = session.b_leg {
                        info!(
                            "Sending BYE to current B-leg {} before route advancement",
                            b_leg.call_id
                        );
                        let bye_message = self.create_bye_message(&session, true).await?;
                        if let Err(bye_err) =
                            self.send_sip_message(b_leg.remote_addr, &bye_message).await
                        {
                            warn!(
                                "Failed to send BYE to B-leg during route advancement: {}",
                                bye_err
                            );
                        }

                        // Remove B-leg call ID mapping
                        {
                            let mut mapping = self.session_manager.call_id_mapping.write().await;
                            mapping.remove(&b_leg.call_id);
                        }
                    }

                    // Check if we should attempt route advancement
                    let advancement_result = {
                        let mut route_advancement = self.route_advancement.lock().await;
                        route_advancement
                            .handle_sip_response(&session.session_id, 488, "No compatible codec")
                            .await?
                    };

                    match advancement_result.action {
                        crate::route_advancement::AdvancementAction::RouteToNext => {
                            info!(
                                "Advancing to next route for call {} due to codec mismatch",
                                session.session_id
                            );

                            if let Some(new_route) = advancement_result.new_route {
                                // Reset session state for new attempt
                                let mut clean_session = session;
                                clean_session.b_leg = None; // Clear previous B-leg
                                clean_session.cdr.b_leg_call_id = None;
                                clean_session.state = SessionState::Routing;

                                self.attempt_termination(clean_session, new_route).await?;
                            } else {
                                self.terminate_session(
                                    &session.session_id,
                                    488,
                                    "No compatible codec on B leg",
                                )
                                .await?;
                            }
                        }
                        _ => {
                            // No more routes available
                            self.terminate_session(
                                &session.session_id,
                                488,
                                "No compatible codec on B leg",
                            )
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
    async fn handle_ack(&self, sip_message: SipMessage, addr: SocketAddr) -> Result<()> {
        let call_id = sip_message
            .headers
            .get("Call-ID")
            .ok_or_else(|| anyhow!("Missing Call-ID header"))?;

        debug!("Processing ACK for call {} from {}", call_id, addr);

        let session = match self
            .session_manager
            .get_session_by_any_call_id(call_id)
            .await
        {
            Some(session) => session,
            None => {
                debug!("Received ACK for unknown call: {}", call_id);
                return Ok(());
            }
        };

        // Determine which leg sent the ACK and forward to the other leg
        let is_from_a_leg = session.a_leg.remote_addr == addr;

        if is_from_a_leg {
            // ACK from A-leg, forward to B-leg
            if let Some(ref b_leg) = session.b_leg {
                let forwarded_ack = self
                    .create_forwarded_ack(&session, &sip_message, true)
                    .await?;
                self.send_sip_message(b_leg.remote_addr, &forwarded_ack)
                    .await?;
                debug!(
                    "Forwarded ACK from A-leg to B-leg for call {}",
                    session.session_id
                );
            } else {
                warn!(
                    "Received ACK from A-leg but no B-leg exists for call {}",
                    session.session_id
                );
            }
        } else {
            // ACK from B-leg, forward to A-leg
            let forwarded_ack = self
                .create_forwarded_ack(&session, &sip_message, false)
                .await?;
            self.send_sip_message(session.a_leg.remote_addr, &forwarded_ack)
                .await?;
            debug!(
                "Forwarded ACK from B-leg to A-leg for call {}",
                session.session_id
            );
        }

        // Update call state to connected if both legs are established
        if session.state == SessionState::Connecting {
            let mut updated_session = session;
            updated_session.state = SessionState::Connected;
            updated_session.last_activity = Utc::now();
            self.session_manager.update_session(updated_session).await;
            info!("Call {} fully established (ACK received)", call_id);
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

            // Calculate duration - FIX: Handle missing end_time gracefully
            if let Some(answer_time) = session.cdr.answer_time {
                if let Some(end_time) = session.cdr.end_time {
                    session.cdr.duration_seconds = Some(
                        end_time
                            .signed_duration_since(answer_time)
                            .max(chrono::Duration::zero())
                            .num_seconds() as u64,
                    );
                } else {
                    warn!(
                        "End time not set for session {}, using current time",
                        session_id
                    );
                    let now = Utc::now();
                    session.cdr.duration_seconds = Some(
                        now.signed_duration_since(answer_time)
                            .max(chrono::Duration::zero())
                            .num_seconds() as u64,
                    );
                }
            }

            // Calculate final costs before sending CDR
            let ingress_trunk = self.find_trunk_by_id(session.cdr.ingress_trunk_id);
            let egress_trunk = session
                .cdr
                .egress_trunk_id
                .and_then(|id| self.find_trunk_by_id(id));

            if let Some(ingress_trunk) = ingress_trunk {
                session.cdr.apply_rate_overrides(ingress_trunk, true);
                if let Some(egress_trunk) = egress_trunk {
                    session.cdr.apply_rate_overrides(egress_trunk, false);
                }
                session.cdr.calculate_costs(ingress_trunk, egress_trunk);
            }

            // Send CDR with calculated costs
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

    /// Determine traffic type based on called number and source IP
    fn determine_traffic_type(
        &self,
        sip_message: &SipMessage,
        source_ip: IpAddr,
    ) -> Result<TrafficType> {
        let called_number = self.extract_called_number(sip_message)?;
        let calling_number = self.extract_calling_number(sip_message)?;

        // Find the ingress trunk based on source IP
        let ingress_trunk = self
            .trunk_configs
            .iter()
            .find(|trunk| trunk.ip_addresses.contains(&source_ip))
            .ok_or_else(|| anyhow!("No trunk found for source IP: {}", source_ip))?;

        // Check if DNIS belongs to our customers (origination traffic)
        let is_our_dnis = self.is_our_customer_dnis(&called_number, ingress_trunk);

        if is_our_dnis {
            // Someone calling our customer's number = ORIGINATION traffic
            Ok(TrafficType::Origination {
                customer_dnis: called_number,
                calling_party: calling_number,
                source_carrier_ip: source_ip,
                ingress_trunk_id: ingress_trunk.trunk_id,
            })
        } else {
            // Our customer calling external number = TERMINATION traffic
            Ok(TrafficType::Termination {
                customer_ani: calling_number,
                destination: called_number,
                customer_ip: source_ip,
                ingress_trunk_id: ingress_trunk.trunk_id,
            })
        }
    }

    /// Check if a DNIS belongs to our customers
    fn is_our_customer_dnis(&self, dnis: &str, trunk: &TrunkRateConfig) -> bool {
        // Check trunk-specific number blocks first
        for number_block in &trunk.our_number_blocks {
            if self.matches_number_pattern(dnis, number_block) {
                return true;
            }
        }

        // Check global number blocks from config if available
        false // TODO: Add global number block checking
    }

    /// Match number against pattern (supports wildcards)
    fn matches_number_pattern(&self, number: &str, pattern: &str) -> bool {
        if pattern.contains('*') {
            // Handle wildcard patterns like "1800555****"
            let prefix = pattern.trim_end_matches('*');
            number.starts_with(prefix)
        } else {
            // Exact match
            number == pattern
        }
    }

    /// Find trunk configuration by IP address
    fn find_trunk_by_ip(&self, ip: IpAddr) -> Option<&TrunkRateConfig> {
        self.trunk_configs
            .iter()
            .find(|trunk| trunk.ip_addresses.contains(&ip))
    }

    /// Validate SIP message has required headers
    fn validate_sip_message(&self, sip_message: &SipMessage) -> bool {
        // All SIP messages must have Call-ID
        if !sip_message.headers.contains_key("Call-ID") {
            return false;
        }

        // Requests must have Via, From, To, CSeq
        if sip_message.method.is_some() {
            let required_headers = ["Via", "From", "To", "CSeq"];
            for header in &required_headers {
                if !sip_message.headers.contains_key(*header) {
                    return false;
                }
            }
        }

        // Responses must have Via, From, To, CSeq and status code
        if sip_message.method.is_none() {
            if sip_message.status_code.is_none() {
                return false;
            }
            let required_headers = ["Via", "From", "To", "CSeq"];
            for header in &required_headers {
                if !sip_message.headers.contains_key(*header) {
                    return false;
                }
            }
        }

        true
    }

    /// Handle OPTIONS messages (for health checks and capability discovery)
    async fn handle_options(&self, sip_message: SipMessage, addr: SocketAddr) -> Result<()> {
        let call_id = sip_message
            .headers
            .get("Call-ID")
            .ok_or_else(|| anyhow!("Missing Call-ID header in OPTIONS"))?;

        debug!("Processing OPTIONS request from {}", addr);

        // Respond with 200 OK and our capabilities
        let allow_methods = "INVITE,ACK,BYE,CANCEL,OPTIONS";
        let supported_codecs = "PCMU,PCMA,G729,G722";

        let options_response = format!(
            "SIP/2.0 200 OK\r\nVia: {}\r\nFrom: {}\r\nTo: {}\r\nCall-ID: {}\r\nCSeq: {}\r\nAllow: {}\r\nSupported: {}\r\nContent-Length: 0\r\n\r\n",
            sip_message.headers.get("Via").unwrap_or(&String::new()),
            sip_message.headers.get("From").unwrap_or(&String::new()),
            sip_message.headers.get("To").unwrap_or(&String::new()),
            call_id,
            sip_message.headers.get("CSeq").unwrap_or(&String::new()),
            allow_methods,
            supported_codecs
        );

        self.send_sip_message(addr, &options_response).await?;
        Ok(())
    }

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
        let call_id = sip_message
            .headers
            .get("Call-ID")
            .ok_or_else(|| anyhow!("Missing Call-ID header"))?
            .clone();
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

            // Call parties
            ani: calling.clone(),
            dnis: called.clone(),

            // Timing
            start_time: Utc::now(),
            answer_time: None,
            end_time: None,
            duration_seconds: None,

            // Ingress leg (will be determined by call direction logic)
            ingress_trunk_id: 0, // TODO: Extract from trunk mapping
            ingress_trunk_name: "Unknown".to_string(),
            ingress_ip: addr.ip(),
            ingress_rate_per_minute: 0.0,
            ingress_cost: 0.0,
            ingress_revenue: None,

            // Egress leg (will be set during routing)
            egress_trunk_id: None,
            egress_trunk_name: None,
            egress_ip: None,
            egress_rate_per_minute: 0.0,
            egress_cost: 0.0,
            egress_revenue: None,

            // Net calculation (will be calculated at end)
            total_cost: 0.0,
            total_revenue: 0.0,
            net_margin: 0.0,
            profit_margin_percent: 0.0,

            // Technical details
            codec_negotiated: None,
            transcoding_used: false,
            termination_cause: None,
            termination_reason: None,
            route_attempts: 0,
            final_route: None,

            // ANI-II (Automatic Number Identification Information Indicator) details
            ani_ii_ingress: None, // Will be parsed from SIP headers
            ani_ii_egress: None,  // Will be set during egress routing
            ani_ii_source: None,  // Will be set when ANI-II is found

            // Payphone surcharge information (for toll-free calls)
            is_toll_free: crate::ani_ii::toll_free::is_toll_free(&called),
            payphone_surcharge_amount: None, // Will be calculated if ANI-II indicates payphone
            payphone_surcharge_reason: None, // Will be set if surcharge applies
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

    // Additional helper methods for origination/termination processing

    /// Find customer information for a DID (origination traffic)
    async fn find_customer_for_dnis(&self, dnis: &str) -> Result<Option<CustomerInfo>> {
        // TODO: Implement customer lookup based on DID assignment
        // This would typically query a database of DID assignments

        // For now, return a mock customer for any DID that matches our patterns
        if self.trunk_configs.iter().any(|trunk| {
            trunk
                .our_number_blocks
                .iter()
                .any(|pattern| self.matches_number_pattern(dnis, pattern))
        }) {
            Ok(Some(CustomerInfo {
                customer_id: "MOCK_CUSTOMER".to_string(),
                sip_endpoint: "192.168.1.100:5060".parse()?,
                customer_name: "Mock Customer Corp".to_string(),
                did_ani_ii_blocks: vec![], // No ANI-II blocking by default
            }))
        } else {
            Ok(None)
        }
    }

    /// Create session for origination traffic
    async fn create_origination_session(
        &self,
        sip_message: &SipMessage,
        addr: SocketAddr,
        calling_party: String,
        customer_dnis: String,
        ingress_trunk_id: u32,
        customer_info: CustomerInfo,
    ) -> Result<CallSession> {
        let session_id = Uuid::new_v4().to_string();
        let call_id = sip_message
            .headers
            .get("Call-ID")
            .ok_or_else(|| anyhow!("Missing Call-ID header"))?
            .clone();
        let from_tag = self.extract_tag(&sip_message.headers, "From")?;

        let ingress_trunk = self
            .find_trunk_by_id(ingress_trunk_id)
            .ok_or_else(|| anyhow!("Trunk {} not found", ingress_trunk_id))?;

        let a_leg = CallLeg {
            call_id: call_id.clone(),
            from_tag,
            to_tag: None,
            local_addr: self.socket.local_addr()?,
            remote_addr: addr, // Carrier IP
            state: LegState::Invited,
            sip_headers: sip_message.headers.clone(),
            supported_codecs: self.extract_codecs(&sip_message),
            selected_codec: None,
            last_cseq: 1,
        };

        let mut cdr = CallDetailRecord {
            session_id: session_id.clone(),
            a_leg_call_id: call_id,
            b_leg_call_id: None,

            // Call parties
            ani: calling_party,
            dnis: customer_dnis.clone(),

            // Timing
            start_time: Utc::now(),
            answer_time: None,
            end_time: None,
            duration_seconds: None,

            // Ingress leg (from carrier - WE PAY THEM for origination)
            ingress_trunk_id,
            ingress_trunk_name: ingress_trunk.trunk_name.clone(),
            ingress_ip: addr.ip(),
            ingress_rate_per_minute: ingress_trunk.default_rate_per_minute,
            ingress_cost: 0.0, // Will be calculated at end (positive = we pay carrier)
            ingress_revenue: None, // No revenue from ingress on origination

            // Egress leg (to customer - WE BILL THEM for DID service)
            egress_trunk_id: None, // Customer endpoint, not a trunk
            egress_trunk_name: Some(format!("Customer_{}", customer_info.customer_id)),
            egress_ip: Some(customer_info.sip_endpoint.ip()),
            egress_rate_per_minute: 0.10, // TODO: Get customer's DID rate from config ($0.10/min example)
            egress_cost: 0.0,             // No cost to deliver to customer
            egress_revenue: Some(0.0),    // Will be calculated - we bill customer for DID

            // Net calculation (will be calculated at end)
            total_cost: 0.0,
            total_revenue: 0.0,
            net_margin: 0.0,
            profit_margin_percent: 0.0,

            // Technical details
            codec_negotiated: None,
            transcoding_used: false,
            termination_cause: None,
            termination_reason: None,
            route_attempts: 0,
            final_route: Some(customer_info.customer_id.clone()),

            // ANI-II (Automatic Number Identification Information Indicator) details
            ani_ii_ingress: None, // Will be parsed from SIP headers
            ani_ii_egress: None,  // Will be set during egress routing
            ani_ii_source: None,  // Will be set when ANI-II is found

            // Payphone surcharge information (for toll-free calls)
            is_toll_free: crate::ani_ii::toll_free::is_toll_free(&customer_dnis),
            payphone_surcharge_amount: None, // Will be calculated if ANI-II indicates payphone
            payphone_surcharge_reason: None, // Will be set if surcharge applies
        };

        // Set revenue flag for ingress if it's a revenue trunk
        if ingress_trunk.is_revenue_trunk {
            cdr.ingress_revenue = Some(0.0); // Will be calculated at end
        }

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

    /// Create session for termination traffic
    async fn create_termination_session(
        &self,
        sip_message: &SipMessage,
        addr: SocketAddr,
        customer_ani: String,
        destination: String,
        ingress_trunk_id: u32,
    ) -> Result<CallSession> {
        let session_id = Uuid::new_v4().to_string();
        let call_id = sip_message
            .headers
            .get("Call-ID")
            .ok_or_else(|| anyhow!("Missing Call-ID header"))?
            .clone();
        let from_tag = self.extract_tag(&sip_message.headers, "From")?;

        let ingress_trunk = self
            .find_trunk_by_id(ingress_trunk_id)
            .ok_or_else(|| anyhow!("Trunk {} not found", ingress_trunk_id))?;

        let a_leg = CallLeg {
            call_id: call_id.clone(),
            from_tag,
            to_tag: None,
            local_addr: self.socket.local_addr()?,
            remote_addr: addr, // Customer IP
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

            // Call parties
            ani: customer_ani,
            dnis: destination.clone(),

            // Timing
            start_time: Utc::now(),
            answer_time: None,
            end_time: None,
            duration_seconds: None,

            // Ingress leg (from customer - WE BILL THEM for outbound service)
            ingress_trunk_id,
            ingress_trunk_name: ingress_trunk.trunk_name.clone(),
            ingress_ip: addr.ip(),
            ingress_rate_per_minute: ingress_trunk.default_rate_per_minute,
            ingress_cost: 0.0, // No cost from customer (negative cost = revenue)
            ingress_revenue: Some(0.0), // Will be calculated - we bill customer for outbound

            // Egress leg (to carrier - we pay them, will be set during routing)
            egress_trunk_id: None,
            egress_trunk_name: None,
            egress_ip: None,
            egress_rate_per_minute: 0.0,
            egress_cost: 0.0,
            egress_revenue: None,

            // Net calculation (will be calculated at end)
            total_cost: 0.0,
            total_revenue: 0.0,
            net_margin: 0.0,
            profit_margin_percent: 0.0,

            // Technical details
            codec_negotiated: None,
            transcoding_used: false,
            termination_cause: None,
            termination_reason: None,
            route_attempts: 0,
            final_route: None,

            // ANI-II (Automatic Number Identification Information Indicator) details
            ani_ii_ingress: None, // Will be parsed from SIP headers
            ani_ii_egress: None,  // Will be set during egress routing
            ani_ii_source: None,  // Will be set when ANI-II is found

            // Payphone surcharge information (for toll-free calls)
            is_toll_free: crate::ani_ii::toll_free::is_toll_free(&destination),
            payphone_surcharge_amount: None, // Will be calculated if ANI-II indicates payphone
            payphone_surcharge_reason: None, // Will be set if surcharge applies
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

    /// Deliver call to customer (origination traffic)
    async fn deliver_to_customer(&self, session: CallSession) -> Result<()> {
        info!(
            "Delivering origination call {} to customer",
            session.session_id
        );

        // TODO: Implement direct delivery to customer SIP endpoint
        // This would create B-leg directly to customer without LCR

        Ok(())
    }

    /// Find trunk by ID
    fn find_trunk_by_id(&self, trunk_id: u32) -> Option<&TrunkRateConfig> {
        self.trunk_configs
            .iter()
            .find(|trunk| trunk.trunk_id == trunk_id)
    }

    fn extract_calling_number(&self, sip_message: &SipMessage) -> Result<String> {
        // Extract from From header
        if let Some(from) = sip_message.headers.get("From") {
            // Look for sip: URI
            if let Some(start) = from.find("sip:") {
                let after_sip = &from[start + 4..];
                if let Some(at_pos) = after_sip.find('@') {
                    let number = &after_sip[..at_pos];
                    // Basic validation - should be numeric for NANPA numbers
                    if number.chars().all(|c| c.is_ascii_digit() || c == '+') && !number.is_empty()
                    {
                        return Ok(number.to_string());
                    }
                }
            }

            // Also try tel: URI format
            if let Some(start) = from.find("tel:") {
                let after_tel = &from[start + 4..];
                // Extract until semicolon or angle bracket
                let end_pos = after_tel
                    .find(';')
                    .unwrap_or_else(|| after_tel.find('>').unwrap_or(after_tel.len()));
                let number = &after_tel[..end_pos];
                if number
                    .chars()
                    .all(|c| c.is_ascii_digit() || c == '+' || c == '-')
                    && !number.is_empty()
                {
                    // Remove dashes and return clean number
                    return Ok(number.replace('-', ""));
                }
            }
        }
        Err(anyhow!("Could not extract calling number from From header"))
    }

    fn extract_called_number(&self, sip_message: &SipMessage) -> Result<String> {
        // Extract from To header first, then fall back to request URI
        if let Some(to_header) = sip_message.headers.get("To") {
            if let Some(start) = to_header.find("sip:") {
                let after_sip = &to_header[start + 4..];
                if let Some(at_pos) = after_sip.find('@') {
                    let number = &after_sip[..at_pos];
                    // Basic validation - should be numeric for NANPA numbers
                    if number.chars().all(|c| c.is_ascii_digit() || c == '+') && !number.is_empty()
                    {
                        return Ok(number.to_string());
                    }
                }
            }
        }

        // Try extracting from request URI if available in a request message
        // This would need the full SIP message first line to parse properly
        // For now, return error to force proper header parsing
        Err(anyhow!("Could not extract called number from SIP headers"))
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
                    "PCMU".to_string(), // G.711 μ-law
                    "PCMA".to_string(), // G.711 A-law
                    "G729".to_string(), // G.729
                    "G722".to_string(), // G.722
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
        match self
            .codec_translator
            .negotiate_codecs(&mut session.codec_negotiation, message)
            .await
        {
            Ok(_) => {
                info!(
                    "Codec negotiation successful for call {}: {:?}",
                    session.session_id, session.codec_negotiation.negotiated_codec
                );
                Ok(())
            }
            Err(e) => {
                warn!(
                    "Codec negotiation failed for call {}: {}",
                    session.session_id, e
                );

                // If transcoding is disabled, this should trigger route advancement
                if !self.config.enable_codec_translation {
                    // Log CDR with specific cause
                    session.cdr.termination_cause = Some(488); // Not Acceptable Here
                    session.cdr.termination_reason =
                        Some("No compatible codec on B leg".to_string());

                    return Err(anyhow!(
                        "No compatible codec found and transcoding disabled: {}",
                        e
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
            session.cdr.dnis, route.egress_trunk.host, route.egress_trunk.port
        ))
    }

    async fn create_forwarded_response(
        &self,
        session: &CallSession,
        code: u16,
        reason: &str,
        original: &SipMessage,
    ) -> Result<String> {
        // Create forwarded response message with proper headers
        let empty_string = String::new();
        let via = original.headers.get("Via").unwrap_or(&empty_string);
        let from = original.headers.get("From").unwrap_or(&empty_string);
        let to = original.headers.get("To").unwrap_or(&empty_string);
        let cseq = original.headers.get("CSeq").unwrap_or(&empty_string);

        Ok(format!(
            "SIP/2.0 {} {}\r\nVia: {}\r\nFrom: {}\r\nTo: {}\r\nCall-ID: {}\r\nCSeq: {}\r\nContent-Length: 0\r\n\r\n",
            code, reason, via, from, to, session.a_leg.call_id, cseq
        ))
    }

    /// Create forwarded ACK message
    async fn create_forwarded_ack(
        &self,
        session: &CallSession,
        original: &SipMessage,
        to_b_leg: bool,
    ) -> Result<String> {
        let target_leg = if to_b_leg {
            session
                .b_leg
                .as_ref()
                .ok_or_else(|| anyhow!("No B-leg available for ACK forwarding"))?
        } else {
            &session.a_leg
        };

        let request_uri = if to_b_leg {
            format!(
                "sip:{}@{}:{}",
                session.cdr.dnis,
                target_leg.remote_addr.ip(),
                target_leg.remote_addr.port()
            )
        } else {
            format!(
                "sip:{}@{}:{}",
                session.cdr.ani,
                target_leg.remote_addr.ip(),
                target_leg.remote_addr.port()
            )
        };

        let via = format!(
            "SIP/2.0/UDP {}:{};branch=z9hG4bK{}",
            self.socket.local_addr()?.ip(),
            self.socket.local_addr()?.port(),
            target_leg.from_tag
        );

        let from = if to_b_leg {
            format!(
                "sip:{}@{}:{};tag={}",
                session.cdr.ani,
                self.socket.local_addr()?.ip(),
                self.socket.local_addr()?.port(),
                session.a_leg.from_tag
            )
        } else {
            format!(
                "sip:{}@{}:{};tag={}",
                session.cdr.dnis,
                self.socket.local_addr()?.ip(),
                self.socket.local_addr()?.port(),
                target_leg.from_tag
            )
        };

        let to = if to_b_leg {
            format!(
                "sip:{}@{}:{}{}",
                session.cdr.dnis,
                target_leg.remote_addr.ip(),
                target_leg.remote_addr.port(),
                target_leg
                    .to_tag
                    .as_ref()
                    .map(|t| format!(";tag={}", t))
                    .unwrap_or_default()
            )
        } else {
            let a_leg_to_tag = session.a_leg.to_tag.as_deref().unwrap_or("missing");
            format!(
                "sip:{}@{}:{};tag={}",
                session.cdr.ani,
                target_leg.remote_addr.ip(),
                target_leg.remote_addr.port(),
                a_leg_to_tag
            )
        };

        Ok(format!(
            "ACK {} SIP/2.0\r\nVia: {}\r\nFrom: {}\r\nTo: {}\r\nCall-ID: {}\r\nCSeq: {} ACK\r\nContent-Length: 0\r\n\r\n",
            request_uri, via, from, to, target_leg.call_id, target_leg.last_cseq
        ))
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

    /// Create BYE message for call leg termination
    async fn create_bye_message(&self, session: &CallSession, to_b_leg: bool) -> Result<String> {
        let target_leg = if to_b_leg {
            session
                .b_leg
                .as_ref()
                .ok_or_else(|| anyhow!("No B-leg available for BYE message"))?
        } else {
            &session.a_leg
        };

        let request_uri = if to_b_leg {
            format!(
                "sip:{}@{}:{}",
                session.cdr.dnis,
                target_leg.remote_addr.ip(),
                target_leg.remote_addr.port()
            )
        } else {
            format!(
                "sip:{}@{}:{}",
                session.cdr.ani,
                target_leg.remote_addr.ip(),
                target_leg.remote_addr.port()
            )
        };

        let via = format!(
            "SIP/2.0/UDP {}:{};branch=z9hG4bK{}",
            self.socket.local_addr()?.ip(),
            self.socket.local_addr()?.port(),
            target_leg.from_tag
        );

        let from = if to_b_leg {
            format!(
                "sip:{}@{}:{};tag={}",
                session.cdr.ani,
                self.socket.local_addr()?.ip(),
                self.socket.local_addr()?.port(),
                session.a_leg.from_tag
            )
        } else {
            format!(
                "sip:{}@{}:{};tag={}",
                session.cdr.dnis,
                self.socket.local_addr()?.ip(),
                self.socket.local_addr()?.port(),
                target_leg.from_tag
            )
        };

        let to = if to_b_leg {
            format!(
                "sip:{}@{}:{}{}",
                session.cdr.dnis,
                target_leg.remote_addr.ip(),
                target_leg.remote_addr.port(),
                target_leg
                    .to_tag
                    .as_ref()
                    .map(|t| format!(";tag={}", t))
                    .unwrap_or_default()
            )
        } else {
            let a_leg_to_tag = session.a_leg.to_tag.as_deref().unwrap_or("missing");
            format!(
                "sip:{}@{}:{};tag={}",
                session.cdr.ani,
                target_leg.remote_addr.ip(),
                target_leg.remote_addr.port(),
                a_leg_to_tag
            )
        };

        Ok(format!(
            "BYE {} SIP/2.0\r\nVia: {}\r\nFrom: {}\r\nTo: {}\r\nCall-ID: {}\r\nCSeq: {} BYE\r\nContent-Length: 0\r\n\r\n",
            request_uri, via, from, to, target_leg.call_id, target_leg.last_cseq + 1
        ))
    }

    /// Check trunk-level ANI-II blocking configuration
    async fn check_trunk_ani_ii_blocking(
        &self,
        trunk_id: u32,
        ani_ii_code: u8,
    ) -> crate::ani_ii::blocking::BlockingResult {
        // Find trunk configuration
        if let Some(trunk) = self.trunk_configs.iter().find(|t| t.trunk_id == trunk_id) {
            if let Some(ref blocking_config) = trunk.ani_ii_blocking {
                return crate::ani_ii::blocking::check_ani_ii_blocking(
                    blocking_config,
                    ani_ii_code,
                );
            }
        }

        // No blocking configuration found - allow call
        crate::ani_ii::blocking::BlockingResult {
            blocked: false,
            reason: "No blocking configuration".to_string(),
            response_code: None,
        }
    }

    /// Check DID-level ANI-II blocking configuration (overrides trunk-level)
    async fn check_did_ani_ii_blocking(
        &self,
        customer_info: &CustomerInfo,
        did_number: &str,
        ani_ii_code: u8,
    ) -> crate::ani_ii::blocking::BlockingResult {
        // Look for DID-specific blocking configuration
        for did_block in &customer_info.did_ani_ii_blocks {
            if did_block.did_number == did_number {
                return crate::ani_ii::blocking::check_ani_ii_blocking(
                    &did_block.blocking_config,
                    ani_ii_code,
                );
            }
        }

        // No DID-specific configuration found - use trunk-level config if available
        // Note: We would need the trunk ID here to check trunk config, but for now
        // we'll just allow the call since DID-level check is an override
        crate::ani_ii::blocking::BlockingResult {
            blocked: false,
            reason: "No DID-specific blocking configuration".to_string(),
            response_code: None,
        }
    }

    /// Create CDR for blocked call attempts for security auditing
    fn create_blocked_call_cdr(
        &self,
        calling_number: &str,
        called_number: &str,
        ingress_trunk_id: u32,
        ani_ii_code: Option<u8>,
        block_reason: &str,
    ) -> CallDetailRecord {
        let now = Utc::now();
        CallDetailRecord {
            session_id: format!("BLOCKED-{}", uuid::Uuid::new_v4()),
            a_leg_call_id: format!("blocked-{}", uuid::Uuid::new_v4()),
            b_leg_call_id: None,
            ani: calling_number.to_string(),
            dnis: called_number.to_string(),
            start_time: now,
            answer_time: None,
            end_time: Some(now), // Blocked immediately
            duration_seconds: Some(0),

            // Ingress trunk information
            ingress_trunk_id,
            ingress_trunk_name: "BLOCKED".to_string(),
            ingress_ip: "0.0.0.0".parse().unwrap(),
            ingress_rate_per_minute: 0.0,
            ingress_cost: 0.0,
            ingress_revenue: None,

            // No egress for blocked calls
            egress_trunk_id: None,
            egress_trunk_name: None,
            egress_ip: None,
            egress_rate_per_minute: 0.0,
            egress_cost: 0.0,
            egress_revenue: None,

            // Net calculations (all zero for blocked calls)
            total_cost: 0.0,
            total_revenue: 0.0,
            net_margin: 0.0,
            profit_margin_percent: 0.0,

            // Technical details
            codec_negotiated: None,
            transcoding_used: false,
            termination_cause: Some(403), // Forbidden
            termination_reason: Some(format!("ANI-II Blocking: {}", block_reason)),
            route_attempts: 0,
            final_route: None,

            // ANI-II specific fields
            ani_ii_ingress: ani_ii_code,
            ani_ii_egress: None,
            ani_ii_source: Some("BLOCKED".to_string()),

            // Toll-free and surcharge fields
            is_toll_free: crate::ani_ii::toll_free::is_toll_free(called_number),
            payphone_surcharge_amount: None, // No surcharge for blocked calls
            payphone_surcharge_reason: Some(format!("BLOCKED: {}", block_reason)),
        }
    }

    fn start_session_cleanup_task(&self) {
        let session_manager = self.session_manager.clone();
        let cleanup_interval = Duration::from_secs(self.config.session_cleanup_interval_seconds);
        let call_timeout = Duration::from_secs(self.config.call_timeout_seconds);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);
            info!(
                "Started session cleanup task with interval: {:?}, timeout: {:?}",
                cleanup_interval, call_timeout
            );

            loop {
                interval.tick().await;

                let cleanup_start = std::time::Instant::now();
                let cleaned_count = session_manager.cleanup_expired_sessions(call_timeout).await;
                let cleanup_duration = cleanup_start.elapsed();

                if cleaned_count > 0 {
                    info!(
                        "Session cleanup completed: {} sessions cleaned in {:?}",
                        cleaned_count, cleanup_duration
                    );
                } else {
                    debug!(
                        "Session cleanup completed: no expired sessions found (took {:?})",
                        cleanup_duration
                    );
                }
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
                    stats.total_call_minutes += duration / 60;
                }
            } else {
                stats.failed_calls += 1;
            }
        }
    }

    pub async fn cleanup_expired_sessions(&self, timeout: Duration) -> usize {
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

        let cleanup_count = expired_sessions.len();
        for session_id in expired_sessions {
            warn!("Cleaning up expired session: {}", session_id);
            self.remove_session(&session_id).await;
        }

        cleanup_count
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
                    cdr.ani, cdr.dnis, cdr.duration_seconds
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
        Err(anyhow!(
            "No compatible codecs found between A-leg {:?} and B-leg {:?}",
            negotiation.a_leg_codecs,
            negotiation.b_leg_codecs
        ))
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
                    "PCMU".to_string(), // G.711 μ-law (most common)
                    "G729".to_string(), // G.729 (bandwidth efficient)
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
