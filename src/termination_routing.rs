//! Termination Routing Engine
//! Handles outbound call routing with SIP response code handling and route advancement

use ahash::AHasher;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::hash::BuildHasherDefault;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use crate::lcr::types::{CallRoute, RouteRequest};
use crate::lcr::LcrEngine;

/// Termination routing plan configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminationRoutingPlan {
    pub name: String,
    pub enabled: bool,
    pub priority: u32,
    pub routes: Vec<CallRoute>,
}

impl Default for TerminationRoutingPlan {
    fn default() -> Self {
        Self {
            name: String::from("default"),
            enabled: true,
            priority: 100,
            routes: Vec::new(),
        }
    }
}

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
    ProfitProtection,   // All routes would result in loss
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

type FastHasher = BuildHasherDefault<AHasher>;

/// High-performance termination routing service with lock-free data structures
pub struct TerminationRoutingService {
    lcr_engine: Arc<LcrEngine>,
    /// Lock-free trunk storage
    trunks: DashMap<i32, TerminationTrunk, FastHasher>,
    /// Lock-free CPS trackers with atomic counters
    cps_trackers: DashMap<i32, CpsTrackerAtomic, FastHasher>,
    /// Atomic call counters per trunk
    active_calls: DashMap<i32, AtomicU32, FastHasher>,
}

/// Atomic CPS tracker for lock-free operation
#[derive(Debug)]
pub struct CpsTrackerAtomic {
    calls_this_second: AtomicU32,
    last_reset: AtomicU64, // Unix timestamp
    limit: u32,
}

impl CpsTrackerAtomic {
    pub fn new(limit: u32) -> Self {
        Self {
            calls_this_second: AtomicU32::new(0),
            last_reset: AtomicU64::new(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            ),
            limit,
        }
    }

    /// Check if a call can be admitted (lock-free)
    pub fn can_admit_call(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let last_reset = self.last_reset.load(Ordering::Relaxed);

        // Reset counter if needed (once per second)
        if now > last_reset {
            if self
                .last_reset
                .compare_exchange_weak(last_reset, now, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                self.calls_this_second.store(0, Ordering::Relaxed);
            }
        }

        let current_calls = self.calls_this_second.load(Ordering::Relaxed);
        current_calls < self.limit
    }

    /// Increment call counter (lock-free)
    pub fn increment(&self) {
        self.calls_this_second.fetch_add(1, Ordering::Relaxed);
    }
}

impl TerminationRoutingService {
    pub fn new(lcr_engine: Arc<LcrEngine>) -> Self {
        Self {
            lcr_engine,
            trunks: DashMap::with_hasher(FastHasher::default()),
            cps_trackers: DashMap::with_hasher(FastHasher::default()),
            active_calls: DashMap::with_hasher(FastHasher::default()),
        }
    }

    /// Add termination trunk (lock-free)
    pub fn add_trunk(&self, trunk: TerminationTrunk) {
        let trunk_id = trunk.id;
        let trunk_name = trunk.name.clone();

        // Initialize CPS tracker if configured (lock-free)
        if let Some(cps_limit) = trunk.cps_limit {
            self.cps_trackers
                .insert(trunk_id, CpsTrackerAtomic::new(cps_limit));
        }

        // Initialize atomic call counter
        self.active_calls.insert(trunk_id, AtomicU32::new(0));

        // Store trunk configuration
        self.trunks.insert(trunk_id, trunk);

        info!(
            "Added termination trunk {} with ID {}",
            trunk_name, trunk_id
        );
    }

    /// Check if trunk can accept call (lock-free)
    pub fn can_trunk_accept_call(&self, trunk_id: i32) -> bool {
        // Check if trunk exists
        let trunk = match self.trunks.get(&trunk_id) {
            Some(trunk) => trunk,
            None => return false,
        };

        // Check if trunk is enabled
        if !trunk.enabled {
            return false;
        }

        // Check concurrent call limit
        if let Some(concurrent_limit) = trunk.concurrent_call_limit {
            let current_calls = self
                .active_calls
                .get(&trunk_id)
                .map(|counter| counter.load(Ordering::Relaxed))
                .unwrap_or(0);

            if current_calls >= concurrent_limit {
                return false;
            }
        }

        // Check CPS limit (lock-free)
        if let Some(cps_tracker) = self.cps_trackers.get(&trunk_id) {
            if !cps_tracker.can_admit_call() {
                return false;
            }
        }

        true
    }

    /// Register call on trunk (lock-free)
    pub fn register_call(&self, trunk_id: i32) -> bool {
        if !self.can_trunk_accept_call(trunk_id) {
            return false;
        }

        // Increment counters atomically
        if let Some(counter) = self.active_calls.get(&trunk_id) {
            counter.fetch_add(1, Ordering::Relaxed);
        }

        if let Some(cps_tracker) = self.cps_trackers.get(&trunk_id) {
            cps_tracker.increment();
        }

        true
    }

    /// End call on trunk (lock-free)
    pub fn end_call(&self, trunk_id: i32) {
        if let Some(counter) = self.active_calls.get(&trunk_id) {
            counter.fetch_sub(1, Ordering::Relaxed);
        }
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
                // Check if all routes were rejected due to profit protection
                let all_unprofitable = available_routes
                    .iter()
                    .all(|route| route.cost_per_minute > route.selling_per_minute);

                if all_unprofitable {
                    return Ok(TerminationRoutingResponse {
                        success: false,
                        selected_route: None,
                        routing_decision: RoutingDecision::ProfitProtection,
                        remaining_routes: available_routes,
                        total_attempts: request.attempt_number,
                        routing_time_ms: start_time.elapsed().as_millis() as u64,
                        reason: "All routes would result in loss - profit protection activated"
                            .to_string(),
                    });
                } else {
                    return Ok(TerminationRoutingResponse {
                        success: false,
                        selected_route: None,
                        routing_decision: RoutingDecision::PolicyBlocked,
                        remaining_routes: available_routes,
                        total_attempts: request.attempt_number,
                        routing_time_ms: start_time.elapsed().as_millis() as u64,
                        reason: "No routes pass policy checks (CPS limits, capacity, etc.)"
                            .to_string(),
                    });
                }
            }
        };

        // Update active call count with proper error handling
        {
            let trunk_id = selected_route.egress_trunk.id;
            let current = self
                .active_calls
                .entry(trunk_id)
                .or_insert_with(|| AtomicU32::new(0))
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            debug!(
                "Incremented active calls for trunk {}: {}",
                trunk_id,
                current + 1
            );
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
        // Decrement count for final responses (>=200) except for 2xx success codes that start a call
        let should_decrement = match response_code {
            100..=199 => false, // Provisional responses - don't decrement
            200..=299 => false, // Success responses - call is established, don't decrement yet
            300..=699 => true,  // Failure responses - decrement count
            _ => false,         // Invalid codes - don't decrement
        };

        if should_decrement {
            if let Some(count_ref) = self.active_calls.get_mut(&trunk_id) {
                let current = count_ref.load(std::sync::atomic::Ordering::Relaxed);
                if current > 0 {
                    count_ref.store(current - 1, std::sync::atomic::Ordering::Relaxed);
                    debug!(
                        "Decremented active calls for trunk {}: {}",
                        trunk_id,
                        current - 1
                    );
                }
            }
        }

        // Determine if we should advance to next route
        let should_advance =
            if let Ok(sip_code) = TryInto::<SipResponseCode>::try_into(response_code) {
                sip_code.should_advance_route()
            } else {
                // For non-standard codes, check trunk-specific configuration
                if let Some(trunk) = self.trunks.get(&trunk_id) {
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
            let trunk = match self.trunks.get(&trunk_id) {
                Some(trunk_ref) if trunk_ref.enabled => trunk_ref,
                _ => continue,
            };

            // PROFIT PROTECTION: Check if cost exceeds selling price
            if route.cost_per_minute > route.selling_per_minute {
                warn!(
                    "Profit protection: Rejecting route {} - cost {} exceeds selling price {}",
                    route.egress_trunk.name, route.cost_per_minute, route.selling_per_minute
                );
                continue; // Skip this route - would result in loss
            }

            // Check CPS limits (simplified for now)
            // TODO: Implement proper CPS tracking

            // Check concurrent call limits
            if let Some(limit) = trunk.concurrent_call_limit {
                let active_calls = self
                    .active_calls
                    .get(&trunk_id)
                    .map(|count| count.load(std::sync::atomic::Ordering::Relaxed))
                    .unwrap_or(0);
                if active_calls >= limit {
                    debug!("Trunk {} concurrent call limit exceeded", trunk_id);
                    continue;
                }
            }

            // Route passes all checks including profit protection
            return Some(route.clone());
        }

        None // No profitable routes available
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
        self.trunks.get(&trunk_id).map(|trunk| {
            let active_calls = self
                .active_calls
                .get(&trunk_id)
                .map(|count| count.load(std::sync::atomic::Ordering::Relaxed))
                .unwrap_or(0);
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

    #[test]
    fn test_profit_protection_routing_decision() {
        use crate::lcr::types::{CallRoute, EgressTrunk, TransportProtocol};
        use rust_decimal::Decimal;
        use std::str::FromStr;

        // Create a route where cost exceeds selling price (unprofitable)
        let unprofitable_route = CallRoute {
            egress_trunk: EgressTrunk {
                id: 1,
                name: "Unprofitable-Trunk".to_string(),
                vendor_id: 1,
                host: "sip1.carrier.com".to_string(),
                port: 5060,
                transport: TransportProtocol::Udp,
                capacity_limit: 100,
                cps_limit: Decimal::from(10),
                active: true,
                priority: 1,
                weight: 100,
                tech_prefix: None,
                supports_international: true,
            },
            vendor: "unprofitable_vendor".to_string(),
            vendor_rate: None,
            cost_per_minute: Decimal::from_str("0.020").unwrap(), // 2 cents cost
            selling_per_minute: Decimal::from_str("0.015").unwrap(), // 1.5 cents selling (LOSS!)
            profit_margin: Decimal::from_str("-0.005").unwrap(),  // Negative margin
            priority: 1,
            setup_fee: Decimal::ZERO,
            min_increment: 6,
            interval: 6,
        };

        // Verify that cost > selling price (would result in loss)
        assert!(
            unprofitable_route.cost_per_minute > unprofitable_route.selling_per_minute,
            "Route should be unprofitable for testing profit protection"
        );

        // Test that ProfitProtection routing decision exists and is distinct
        assert_ne!(
            RoutingDecision::ProfitProtection,
            RoutingDecision::PolicyBlocked
        );
        assert_ne!(
            RoutingDecision::ProfitProtection,
            RoutingDecision::NoRoutesAvailable
        );
        assert_ne!(
            RoutingDecision::ProfitProtection,
            RoutingDecision::RouteFound
        );

        // Create a profitable route for comparison
        let profitable_route = CallRoute {
            egress_trunk: EgressTrunk {
                id: 2,
                name: "Profitable-Trunk".to_string(),
                vendor_id: 1,
                host: "sip2.carrier.com".to_string(),
                port: 5060,
                transport: TransportProtocol::Udp,
                capacity_limit: 100,
                cps_limit: Decimal::from(10),
                active: true,
                priority: 2,
                weight: 100,
                tech_prefix: None,
                supports_international: true,
            },
            vendor: "profitable_vendor".to_string(),
            vendor_rate: None,
            cost_per_minute: Decimal::from_str("0.010").unwrap(), // 1 cent cost
            selling_per_minute: Decimal::from_str("0.015").unwrap(), // 1.5 cents selling (PROFIT!)
            profit_margin: Decimal::from_str("0.005").unwrap(),   // Positive margin
            priority: 2,
            setup_fee: Decimal::ZERO,
            min_increment: 6,
            interval: 6,
        };

        // Verify that cost < selling price (profitable)
        assert!(
            profitable_route.cost_per_minute < profitable_route.selling_per_minute,
            "Route should be profitable"
        );
        assert!(
            profitable_route.profit_margin > Decimal::ZERO,
            "Profit margin should be positive"
        );
    }
}
