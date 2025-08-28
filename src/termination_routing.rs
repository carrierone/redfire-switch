//! Termination Routing Engine
//! Handles outbound call routing with SIP response code handling and route advancement

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use crate::lcr::types::{CallRoute, EgressTrunk, RouteRequest, RouteResponse, RouteType};
use crate::lcr::LcrEngine;

/// NANPA jurisdiction types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NanpaJurisdiction {
    Local,
    Intrastate,
    Interstate,
    Indeterminate,
}

/// SIP response codes that trigger route advancement
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SipResponseCode {
    // 4xx Client Error responses
    BadRequest = 400,
    Unauthorized = 401,
    PaymentRequired = 402,
    Forbidden = 403,
    NotFound = 404,
    MethodNotAllowed = 405,
    NotAcceptable = 406,
    RequestTimeout = 408,
    Gone = 410,
    RequestEntityTooLarge = 413,
    RequestUriTooLong = 414,
    UnsupportedMediaType = 415,
    UnsupportedUriScheme = 416,
    BadExtension = 420,
    ExtensionRequired = 421,
    IntervalTooBrief = 423,
    TemporarilyUnavailable = 480,
    CallTransactionDoesNotExist = 481,
    LoopDetected = 482,
    TooManyHops = 483,
    AddressIncomplete = 484,
    Ambiguous = 485,
    BusyHere = 486,
    RequestTerminated = 487,
    NotAcceptableHere = 488,

    // 5xx Server Error responses
    InternalServerError = 500,
    NotImplemented = 501,
    BadGateway = 502,
    ServiceUnavailable = 503,
    ServerTimeout = 504,
    VersionNotSupported = 505,
    MessageTooLarge = 513,

    // 6xx Global Failure responses
    BusyEverywhere = 600,
    Decline = 603,
    DoesNotExistAnywhere = 604,
    NotAcceptableGlobal = 606,
}

impl SipResponseCode {
    /// Check if this response code should trigger route advancement
    pub fn should_advance_route(&self) -> bool {
        match self {
            // Always advance on these codes
            SipResponseCode::NotFound
            | SipResponseCode::MethodNotAllowed
            | SipResponseCode::NotAcceptable
            | SipResponseCode::RequestTimeout
            | SipResponseCode::Gone
            | SipResponseCode::UnsupportedMediaType
            | SipResponseCode::UnsupportedUriScheme
            | SipResponseCode::TemporarilyUnavailable
            | SipResponseCode::CallTransactionDoesNotExist
            | SipResponseCode::AddressIncomplete
            | SipResponseCode::BusyHere
            | SipResponseCode::NotAcceptableHere
            | SipResponseCode::InternalServerError
            | SipResponseCode::NotImplemented
            | SipResponseCode::BadGateway
            | SipResponseCode::ServiceUnavailable
            | SipResponseCode::ServerTimeout
            | SipResponseCode::VersionNotSupported => true,

            // Never advance on these - they indicate call completion or policy decisions
            SipResponseCode::Unauthorized
            | SipResponseCode::PaymentRequired
            | SipResponseCode::Forbidden
            | SipResponseCode::Ambiguous
            | SipResponseCode::RequestTerminated
            | SipResponseCode::BusyEverywhere
            | SipResponseCode::Decline
            | SipResponseCode::DoesNotExistAnywhere
            | SipResponseCode::NotAcceptableGlobal => false,

            // Conditionally advance on these based on carrier behavior
            SipResponseCode::BadRequest
            | SipResponseCode::RequestEntityTooLarge
            | SipResponseCode::RequestUriTooLong
            | SipResponseCode::BadExtension
            | SipResponseCode::ExtensionRequired
            | SipResponseCode::IntervalTooBrief
            | SipResponseCode::LoopDetected
            | SipResponseCode::TooManyHops
            | SipResponseCode::MessageTooLarge => true, // Default to advancing
        }
    }

    /// Get response code category for routing decisions
    pub fn category(&self) -> ResponseCategory {
        let code = *self as u16;
        match code {
            400..=499 => ResponseCategory::ClientError,
            500..=599 => ResponseCategory::ServerError,
            600..=699 => ResponseCategory::GlobalFailure,
            _ => ResponseCategory::Unknown,
        }
    }
}

/// Response code categories for routing logic
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseCategory {
    ClientError,   // 4xx - usually advance to next route
    ServerError,   // 5xx - usually advance to next route
    GlobalFailure, // 6xx - usually don't advance
    Unknown,       // Other codes
}

/// Termination routing request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminationRoutingRequest {
    pub call_id: String,
    pub ani: String,                            // A-number (caller)
    pub dnis: String,                           // B-number (called)
    pub route_request: RouteRequest,            // LCR request
    pub attempt_number: u32,                    // Current routing attempt
    pub previous_responses: Vec<FailedAttempt>, // Previous failed attempts
    pub max_attempts: u32,                      // Maximum routing attempts
    pub timestamp: DateTime<Utc>,
}

/// Failed routing attempt record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedAttempt {
    pub trunk_id: i32,
    pub trunk_name: String,
    pub response_code: u16,
    pub response_reason: String,
    pub attempt_time: DateTime<Utc>,
    pub duration_ms: u64,
}

/// Termination routing response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminationRoutingResponse {
    pub success: bool,
    pub selected_route: Option<CallRoute>,
    pub routing_decision: RoutingDecision,
    pub remaining_routes: Vec<CallRoute>,
    pub total_attempts: u32,
    pub routing_time_ms: u64,
    pub reason: String,
}

/// Routing decision outcome
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RoutingDecision {
    RouteFound,         // Route found and selected
    NoRoutesAvailable,  // No routes available for destination
    AllRoutesFailed,    // All routes attempted and failed
    MaxAttemptsReached, // Hit maximum attempt limit
    PolicyBlocked,      // Blocked by routing policy
}

/// Trunk configuration for termination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminationTrunk {
    pub id: i32,
    pub name: String,
    pub enabled: bool,
    pub destination_ip: String,
    pub destination_port: u16,
    pub codec_config: TrunkCodecConfig,
    pub cnam_config: Option<TrunkCnamConfig>,
    pub qos_requirements: QosRequirements,
    pub cps_limit: Option<u32>,
    pub concurrent_call_limit: Option<u32>,
    pub route_advance_codes: Vec<u16>, // Custom codes for route advancement
}

/// Codec configuration per trunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrunkCodecConfig {
    pub preferred_codecs: Vec<String>,
    pub allow_transcoding: bool,
    pub dtmf_relay_method: DtmfRelayMethod,
    pub silence_suppression: bool,
    pub echo_cancellation: bool,
}

/// DTMF relay methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DtmfRelayMethod {
    Rfc2833, // RFC 4733 RTP events
    SipInfo, // SIP INFO messages
    Inband,  // Inband audio
}

/// Caller Name (CNAM) configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrunkCnamConfig {
    pub enabled: bool,
    pub lookup_method: CnamLookupMethod,
    pub cache_ttl_seconds: u32,
    pub default_name: Option<String>,
}

/// CNAM lookup methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CnamLookupMethod {
    Database, // Internal database lookup
    Sip,      // SIP header (P-Asserted-Identity)
    External, // External CNAM service
    None,     // No CNAM lookup
}

/// QoS requirements for trunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QosRequirements {
    pub dscp_marking: u8,
    pub max_latency_ms: u32,
    pub max_jitter_ms: u32,
    pub max_packet_loss_percent: f32,
    pub bandwidth_kbps: u32,
}

/// Calls per second tracker
#[derive(Debug, Clone)]
pub struct CpsTracker {
    calls_this_second: u32,
    last_reset: Instant,
    limit: u32,
}

impl CpsTracker {
    pub fn new(limit: u32) -> Self {
        Self {
            calls_this_second: 0,
            last_reset: Instant::now(),
            limit,
        }
    }

    pub fn can_place_call(&mut self) -> bool {
        let now = Instant::now();
        if now.duration_since(self.last_reset) >= Duration::from_secs(1) {
            self.calls_this_second = 0;
            self.last_reset = now;
        }

        if self.calls_this_second < self.limit {
            self.calls_this_second += 1;
            true
        } else {
            false
        }
    }
}

/// Termination routing service
pub struct TerminationRoutingService {
    lcr_engine: Arc<LcrEngine>,
    trunks: Mutex<HashMap<i32, TerminationTrunk>>,
    cps_trackers: Mutex<HashMap<i32, CpsTracker>>,
    active_calls: Mutex<HashMap<i32, u32>>, // trunk_id -> call count
}

impl TerminationRoutingService {
    pub fn new(lcr_engine: Arc<LcrEngine>) -> Self {
        Self {
            lcr_engine,
            trunks: Mutex::new(HashMap::new()),
            cps_trackers: Mutex::new(HashMap::new()),
            active_calls: Mutex::new(HashMap::new()),
        }
    }

    /// Add termination trunk
    pub fn add_trunk(&self, trunk: TerminationTrunk) {
        let trunk_id = trunk.id;
        let trunk_name = trunk.name.clone();

        // Initialize CPS tracker if configured
        if let Some(cps_limit) = trunk.cps_limit {
            self.cps_trackers
                .lock()
                .unwrap()
                .insert(trunk_id, CpsTracker::new(cps_limit));
        }

        self.active_calls.lock().unwrap().insert(trunk_id, 0);
        self.trunks.lock().unwrap().insert(trunk_id, trunk);
        info!(
            "Added termination trunk {} with ID {}",
            trunk_name, trunk_id
        );
    }

    /// Route termination call with route advancement on SIP failures
    pub async fn route_termination(
        &mut self,
        request: TerminationRoutingRequest,
    ) -> Result<TerminationRoutingResponse> {
        let start_time = Instant::now();

        info!(
            "Processing termination request: {} -> {} (attempt {})",
            request.ani, request.dnis, request.attempt_number
        );

        // Check if we've hit maximum attempts
        if request.attempt_number >= request.max_attempts {
            return Ok(TerminationRoutingResponse {
                success: false,
                selected_route: None,
                routing_decision: RoutingDecision::MaxAttemptsReached,
                remaining_routes: vec![],
                total_attempts: request.attempt_number,
                routing_time_ms: start_time.elapsed().as_millis() as u64,
                reason: "Maximum routing attempts reached".to_string(),
            });
        }

        // Get routes from LCR engine
        let lcr_response = match self
            .lcr_engine
            .get_routing_engine()
            .find_routes(&request.route_request)
            .await
        {
            Ok(response) => response,
            Err(e) => {
                warn!("LCR routing failed: {}", e);
                return Ok(TerminationRoutingResponse {
                    success: false,
                    selected_route: None,
                    routing_decision: RoutingDecision::NoRoutesAvailable,
                    remaining_routes: vec![],
                    total_attempts: request.attempt_number,
                    routing_time_ms: start_time.elapsed().as_millis() as u64,
                    reason: format!("LCR routing failed: {}", e),
                });
            }
        };

        if lcr_response.routes.is_empty() {
            return Ok(TerminationRoutingResponse {
                success: false,
                selected_route: None,
                routing_decision: RoutingDecision::NoRoutesAvailable,
                remaining_routes: vec![],
                total_attempts: request.attempt_number,
                routing_time_ms: start_time.elapsed().as_millis() as u64,
                reason: "No routes available for destination".to_string(),
            });
        }

        // Filter out previously failed routes that shouldn't be retried
        let available_routes =
            self.filter_available_routes(&lcr_response.routes, &request.previous_responses);

        if available_routes.is_empty() {
            return Ok(TerminationRoutingResponse {
                success: false,
                selected_route: None,
                routing_decision: RoutingDecision::AllRoutesFailed,
                remaining_routes: vec![],
                total_attempts: request.attempt_number,
                routing_time_ms: start_time.elapsed().as_millis() as u64,
                reason: "All available routes have been attempted and failed".to_string(),
            });
        }

        // Select best available route
        let selected_route = match self.select_best_route(&available_routes).await {
            Some(route) => route,
            None => {
                return Ok(TerminationRoutingResponse {
                    success: false,
                    selected_route: None,
                    routing_decision: RoutingDecision::PolicyBlocked,
                    remaining_routes: available_routes,
                    total_attempts: request.attempt_number,
                    routing_time_ms: start_time.elapsed().as_millis() as u64,
                    reason: "No routes pass policy checks (CPS limits, capacity, etc.)".to_string(),
                });
            }
        };

        // Update active call count
        {
            let mut active_calls_map = self.active_calls.lock().unwrap();
            if let Some(count) = active_calls_map.get_mut(&selected_route.egress_trunk.id) {
                *count += 1;
            }
        }

        let mut remaining_routes = available_routes;
        remaining_routes.retain(|r| r.egress_trunk.id != selected_route.egress_trunk.id);

        Ok(TerminationRoutingResponse {
            success: true,
            selected_route: Some(selected_route),
            routing_decision: RoutingDecision::RouteFound,
            remaining_routes,
            total_attempts: request.attempt_number,
            routing_time_ms: start_time.elapsed().as_millis() as u64,
            reason: "Route successfully selected".to_string(),
        })
    }

    /// Handle SIP response and determine if route advancement is needed
    pub fn handle_sip_response(
        &mut self,
        call_id: &str,
        trunk_id: i32,
        response_code: u16,
        response_reason: &str,
    ) -> RouteAdvanceDecision {
        debug!(
            "Handling SIP response for call {}: {} {} on trunk {}",
            call_id, response_code, response_reason, trunk_id
        );

        // Update active call count on call completion/failure
        if response_code >= 300 || response_code < 200 {
            let mut active_calls_map = self.active_calls.lock().unwrap();
            if let Some(count) = active_calls_map.get_mut(&trunk_id) {
                if *count > 0 {
                    *count -= 1;
                }
            }
        }

        // Determine if we should advance to next route
        let should_advance =
            if let Ok(sip_code) = TryInto::<SipResponseCode>::try_into(response_code) {
                sip_code.should_advance_route()
            } else {
                // For non-standard codes, check trunk-specific configuration
                let trunks_map = self.trunks.lock().unwrap();
                if let Some(trunk) = trunks_map.get(&trunk_id) {
                    trunk.route_advance_codes.contains(&response_code)
                } else {
                    // Default behavior: advance on 4xx/5xx, don't advance on 6xx
                    match response_code {
                        400..=599 => true,
                        _ => false,
                    }
                }
            };

        if should_advance {
            RouteAdvanceDecision::AdvanceToNextRoute
        } else {
            RouteAdvanceDecision::CompleteCall
        }
    }

    /// Filter routes based on previous failures
    fn filter_available_routes(
        &self,
        all_routes: &[CallRoute],
        previous_responses: &[FailedAttempt],
    ) -> Vec<CallRoute> {
        all_routes
            .iter()
            .filter(|route| {
                // Don't retry routes that failed with non-retriable codes
                !previous_responses.iter().any(|attempt| {
                    attempt.trunk_id == route.egress_trunk.id
                        && !self.should_retry_response_code(attempt.response_code)
                })
            })
            .cloned()
            .collect()
    }

    /// Select best available route based on capacity and policy
    async fn select_best_route(&mut self, routes: &[CallRoute]) -> Option<CallRoute> {
        for route in routes {
            let trunk_id = route.egress_trunk.id;

            // Check if trunk exists and is enabled
            let trunks_map = self.trunks.lock().unwrap();
            let trunk = match trunks_map.get(&trunk_id) {
                Some(trunk) if trunk.enabled => trunk,
                _ => continue,
            };

            // Check CPS limits
            {
                let mut cps_trackers = self.cps_trackers.lock().unwrap();
                if let Some(cps_tracker) = cps_trackers.get_mut(&trunk_id) {
                    if !cps_tracker.can_place_call() {
                        debug!("Trunk {} CPS limit exceeded", trunk_id);
                        continue;
                    }
                }
            }

            // Check concurrent call limits
            if let Some(limit) = trunk.concurrent_call_limit {
                let active_calls_map = self.active_calls.lock().unwrap();
                let active_calls = active_calls_map.get(&trunk_id).unwrap_or(&0);
                if *active_calls >= limit {
                    debug!("Trunk {} concurrent call limit exceeded", trunk_id);
                    continue;
                }
            }

            // Route passes all checks
            return Some(route.clone());
        }

        None // No routes available
    }

    /// Check if a response code should trigger a retry
    fn should_retry_response_code(&self, response_code: u16) -> bool {
        match TryInto::<SipResponseCode>::try_into(response_code) {
            Ok(sip_code) => sip_code.should_advance_route(),
            Err(_) => {
                // For non-standard codes, default to retrying 4xx/5xx
                matches!(response_code, 400..=599)
            }
        }
    }

    /// Get trunk statistics
    pub fn get_trunk_stats(&self, trunk_id: i32) -> Option<TrunkStats> {
        let trunks_map = self.trunks.lock().unwrap();
        trunks_map.get(&trunk_id).map(|trunk| {
            let active_calls_map = self.active_calls.lock().unwrap();
            let active_calls = *active_calls_map.get(&trunk_id).unwrap_or(&0);
            TrunkStats {
                trunk_id,
                trunk_name: trunk.name.clone(),
                enabled: trunk.enabled,
                active_calls,
                concurrent_limit: trunk.concurrent_call_limit,
                cps_limit: trunk.cps_limit,
            }
        })
    }
}

/// Route advancement decision
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteAdvanceDecision {
    AdvanceToNextRoute, // Try next route
    CompleteCall,       // Don't advance, complete call
}

/// Trunk statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrunkStats {
    pub trunk_id: i32,
    pub trunk_name: String,
    pub enabled: bool,
    pub active_calls: u32,
    pub concurrent_limit: Option<u32>,
    pub cps_limit: Option<u32>,
}

/// Convert u16 response codes to SipResponseCode enum
impl TryFrom<u16> for SipResponseCode {
    type Error = anyhow::Error;

    fn try_from(code: u16) -> Result<Self> {
        match code {
            400 => Ok(SipResponseCode::BadRequest),
            401 => Ok(SipResponseCode::Unauthorized),
            402 => Ok(SipResponseCode::PaymentRequired),
            403 => Ok(SipResponseCode::Forbidden),
            404 => Ok(SipResponseCode::NotFound),
            405 => Ok(SipResponseCode::MethodNotAllowed),
            406 => Ok(SipResponseCode::NotAcceptable),
            408 => Ok(SipResponseCode::RequestTimeout),
            410 => Ok(SipResponseCode::Gone),
            413 => Ok(SipResponseCode::RequestEntityTooLarge),
            414 => Ok(SipResponseCode::RequestUriTooLong),
            415 => Ok(SipResponseCode::UnsupportedMediaType),
            416 => Ok(SipResponseCode::UnsupportedUriScheme),
            420 => Ok(SipResponseCode::BadExtension),
            421 => Ok(SipResponseCode::ExtensionRequired),
            423 => Ok(SipResponseCode::IntervalTooBrief),
            480 => Ok(SipResponseCode::TemporarilyUnavailable),
            481 => Ok(SipResponseCode::CallTransactionDoesNotExist),
            482 => Ok(SipResponseCode::LoopDetected),
            483 => Ok(SipResponseCode::TooManyHops),
            484 => Ok(SipResponseCode::AddressIncomplete),
            485 => Ok(SipResponseCode::Ambiguous),
            486 => Ok(SipResponseCode::BusyHere),
            487 => Ok(SipResponseCode::RequestTerminated),
            488 => Ok(SipResponseCode::NotAcceptableHere),
            500 => Ok(SipResponseCode::InternalServerError),
            501 => Ok(SipResponseCode::NotImplemented),
            502 => Ok(SipResponseCode::BadGateway),
            503 => Ok(SipResponseCode::ServiceUnavailable),
            504 => Ok(SipResponseCode::ServerTimeout),
            505 => Ok(SipResponseCode::VersionNotSupported),
            513 => Ok(SipResponseCode::MessageTooLarge),
            600 => Ok(SipResponseCode::BusyEverywhere),
            603 => Ok(SipResponseCode::Decline),
            604 => Ok(SipResponseCode::DoesNotExistAnywhere),
            606 => Ok(SipResponseCode::NotAcceptableGlobal),
            _ => Err(anyhow!("Unknown SIP response code: {}", code)),
        }
    }
}

/// Utility functions
pub mod utils {
    use super::*;
    use std::net::IpAddr;

    /// Create test termination trunk
    pub fn create_test_trunk(id: i32, name: &str, destination_ip: &str) -> TerminationTrunk {
        TerminationTrunk {
            id,
            name: name.to_string(),
            enabled: true,
            destination_ip: destination_ip.to_string(),
            destination_port: 5060,
            codec_config: TrunkCodecConfig {
                preferred_codecs: vec!["PCMU".to_string(), "PCMA".to_string()],
                allow_transcoding: true,
                dtmf_relay_method: DtmfRelayMethod::Rfc2833,
                silence_suppression: false,
                echo_cancellation: true,
            },
            cnam_config: Some(TrunkCnamConfig {
                enabled: true,
                lookup_method: CnamLookupMethod::Sip,
                cache_ttl_seconds: 3600,
                default_name: Some("Unknown".to_string()),
            }),
            qos_requirements: QosRequirements {
                dscp_marking: 46, // EF (Expedited Forwarding)
                max_latency_ms: 150,
                max_jitter_ms: 50,
                max_packet_loss_percent: 1.0,
                bandwidth_kbps: 64,
            },
            cps_limit: Some(100),
            concurrent_call_limit: Some(1000),
            route_advance_codes: vec![404, 503, 502], // Custom advance codes
        }
    }

    /// Create test termination request
    pub fn create_test_termination_request(
        ani: &str,
        dnis: &str,
        route_request: RouteRequest,
    ) -> TerminationRoutingRequest {
        TerminationRoutingRequest {
            call_id: format!("test-call-{}", Utc::now().timestamp()),
            ani: ani.to_string(),
            dnis: dnis.to_string(),
            route_request,
            attempt_number: 1,
            previous_responses: vec![],
            max_attempts: 3,
            timestamp: Utc::now(),
        }
    }

    /// Create failed attempt record
    pub fn create_failed_attempt(
        trunk_id: i32,
        trunk_name: &str,
        response_code: u16,
        response_reason: &str,
    ) -> FailedAttempt {
        FailedAttempt {
            trunk_id,
            trunk_name: trunk_name.to_string(),
            response_code,
            response_reason: response_reason.to_string(),
            attempt_time: Utc::now(),
            duration_ms: 1000, // 1 second attempt
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sip_response_code_advance_logic() {
        // Should advance
        assert!(SipResponseCode::NotFound.should_advance_route());
        assert!(SipResponseCode::ServiceUnavailable.should_advance_route());
        assert!(SipResponseCode::TemporarilyUnavailable.should_advance_route());

        // Should not advance
        assert!(!SipResponseCode::Decline.should_advance_route());
        assert!(!SipResponseCode::BusyEverywhere.should_advance_route());
        assert!(!SipResponseCode::Unauthorized.should_advance_route());
    }

    #[test]
    fn test_response_code_conversion() {
        assert_eq!(
            SipResponseCode::try_from(404).unwrap(),
            SipResponseCode::NotFound
        );
        assert_eq!(
            SipResponseCode::try_from(503).unwrap(),
            SipResponseCode::ServiceUnavailable
        );
        assert!(SipResponseCode::try_from(999).is_err());
    }

    #[test]
    fn test_cps_tracker() {
        let mut tracker = CpsTracker::new(2);

        // Should allow first two calls
        assert!(tracker.can_place_call());
        assert!(tracker.can_place_call());

        // Should block third call
        assert!(!tracker.can_place_call());
    }

    #[tokio::test]
    async fn test_termination_routing_no_routes() {
        let lcr_engine = Arc::new(crate::lcr::LcrEngine::new("test://").await.unwrap());
        let mut service = TerminationRoutingService::new(lcr_engine);

        let route_request = RouteRequest {
            ani: "15551234567".to_string(),
            dnis: "18005551234".to_string(),
            ingress_trunk_id: 1,
            client_deck_id: None,
            route_type: RouteType::NANPA,
            require_profit_protection: false,
            min_profit_margin: None,
            effective_time: None,
            phone_validation: None,
            routing_plan_id: None,
        };

        let request =
            utils::create_test_termination_request("15551234567", "18005551234", route_request);

        let response = service.route_termination(request).await.unwrap();
        assert!(!response.success);
        assert_eq!(
            response.routing_decision,
            RoutingDecision::NoRoutesAvailable
        );
    }
}
