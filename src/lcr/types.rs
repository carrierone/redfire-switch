use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RateType {
    LRN,
    DNIS,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RouteType {
    NANPA,
    AZ,
    OTHER,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallJurisdiction {
    Interstate,
    Intrastate,
    IndeterminateJurisdiction,
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NanpaRate {
    pub id: i32,
    pub deck_id: i32,
    pub code: String, // 1NPANXX or more specific
    pub inter_rate: Decimal,
    pub intra_rate: Decimal,
    pub ij_rate: Decimal,
    pub local_rate: Option<Decimal>,
    pub min_increment: i32,
    pub interval: i32,
    pub setup_fee: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateDeck {
    pub id: i32,
    pub name: String,
    pub owner_id: i32, // vendor_id or client_id
    pub rate_type: RateType,
    pub effective_date: DateTime<Utc>,
    pub expires_date: Option<DateTime<Utc>>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EgressTrunk {
    pub id: i32,
    pub name: String,
    pub vendor_id: i32,
    pub host: String,
    pub port: u16,
    pub transport: TransportProtocol,
    pub capacity_limit: i32,
    pub cps_limit: Decimal,
    pub active: bool,
    pub priority: i32,
    pub weight: i32,
    pub tech_prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressTrunk {
    pub id: i32,
    pub name: String,
    pub client_id: i32,
    pub ip_address: IpAddr,
    pub capacity_limit: i32,
    pub cps_limit: Decimal,
    pub profit_protection: bool,
    pub min_profit_margin: Decimal,
    pub active: bool,
    pub auth_username: Option<String>,
    pub auth_password: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportProtocol {
    UDP,
    TCP,
    TLS,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LcrRoute {
    pub id: i32,
    pub name: String,
    pub route_type: RouteType,
    pub description: Option<String>,
    pub active: bool,
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticRoute {
    pub id: i32,
    pub ingress_trunk_id: Option<i32>,
    pub egress_trunk_id: i32,
    pub pattern: String, // Regex pattern
    pub priority: i32,
    pub position: RoutePosition,
    pub description: Option<String>,
    pub active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoutePosition {
    Before,
    After,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteAdvanceConfig {
    pub id: i32,
    pub scope: ConfigScope,
    pub scope_id: Option<i32>,
    pub advance_on_codes: Vec<String>,
    pub stop_on_codes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConfigScope {
    Global,
    IngressTrunk,
    EgressTrunk,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerConfig {
    pub id: i32,
    pub scope: ConfigScope,
    pub scope_id: Option<i32>,
    pub timer_100_to_183_ms: i32,
    pub timer_max_call_duration_sec: i32,
    pub timer_post_dial_delay_ms: i32,
    pub timer_ringing_timeout_sec: i32,
    pub timer_transaction_timeout_ms: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrunkUsageStats {
    pub trunk_id: i32,
    pub trunk_type: TrunkType,
    pub current_calls: i32,
    pub current_cps: Decimal,
    pub total_calls: i64,
    pub total_minutes: Decimal,
    pub last_call_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrunkType {
    Ingress,
    Egress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LrnCacheEntry {
    pub tn: String,
    pub lrn: String,
    pub spid: Option<String>,
    pub ocn: Option<String>,
    pub lata: Option<String>,
    pub state: Option<String>,
    pub jurisdiction: Option<CallJurisdiction>,
    pub cached_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NanpaStatic {
    pub npa: String,
    pub nxx: Option<String>,
    pub state: String,
    pub country: String,
    pub lata: Option<String>,
    pub ocn: Option<String>,
    pub rate_center: Option<String>,
    pub switch_clli: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallRoute {
    pub egress_trunk: EgressTrunk,
    pub vendor_rate: Option<NanpaRate>,
    pub cost_per_minute: Decimal,
    pub selling_per_minute: Decimal,
    pub profit_margin: Decimal,
    pub priority: i32,
    pub setup_fee: Decimal,
    pub min_increment: i32,
    pub interval: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRequest {
    pub ani: String,
    pub dnis: String,
    pub ingress_trunk_id: i32,
    pub client_deck_id: Option<i32>,
    pub route_type: RouteType,
    pub require_profit_protection: bool,
    pub min_profit_margin: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteResponse {
    pub routes: Vec<CallRoute>,
    pub jurisdiction: CallJurisdiction,
    pub lrn: Option<String>,
    pub total_routes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrunkRateAssociation {
    pub id: i32,
    pub egress_trunk_id: Option<i32>,
    pub ingress_trunk_id: Option<i32>,
    pub vendor_deck_id: Option<i32>,
    pub client_deck_id: Option<i32>,
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LcrRouteTrunk {
    pub id: i32,
    pub lcr_route_id: i32,
    pub egress_trunk_id: i32,
    pub vendor_deck_id: i32,
    pub priority: i32,
    pub weight: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SipResponseCode {
    // 4xx Client Errors
    NotFound = 404,
    BusyHere = 486,
    Unauthorized = 401,
    Forbidden = 403,
    PaymentRequired = 402,
    TemporarilyUnavailable = 480,
    CallDoesNotExist = 481,
    RequestTerminated = 487,

    // 5xx Server Errors
    InternalServerError = 500,
    BadGateway = 502,
    ServiceUnavailable = 503,
    ServerTimeout = 504,

    // 6xx Global Failures
    BusyEverywhere = 600,
    Decline = 603,
    DoesNotExistAnywhere = 604,
    NotAcceptable = 606,
}

impl SipResponseCode {
    pub fn should_advance(&self, config: &RouteAdvanceConfig) -> bool {
        let code_str = (*self as u16).to_string();
        config.advance_on_codes.contains(&code_str)
    }

    pub fn should_stop(&self, config: &RouteAdvanceConfig) -> bool {
        let code_str = (*self as u16).to_string();
        config.stop_on_codes.contains(&code_str)
    }
}
