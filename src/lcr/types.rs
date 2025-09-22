use chrono::{DateTime, NaiveTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use std::net::IpAddr;

use crate::lcr::phone_validation::PhoneValidationConfig;

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

impl std::fmt::Display for RouteType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RouteType::NANPA => write!(f, "NANPA"),
            RouteType::AZ => write!(f, "A-Z"),
            RouteType::OTHER => write!(f, "OTHER"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CallJurisdiction {
    Inter,
    Intra,
    Indeterminate,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InternationalJurisdiction {
    EEA, // European Economic Area
    ROW, // Rest of World
}

impl std::fmt::Display for InternationalJurisdiction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InternationalJurisdiction::EEA => write!(f, "EEA"),
            InternationalJurisdiction::ROW => write!(f, "ROW"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternationalRate {
    pub id: i32,
    pub deck_id: i32,
    pub country_code: String, // Country prefix (e.g., "44", "49", "33")
    pub destination_code: Option<String>, // Optional more specific code (e.g., "44207")
    pub destination_name: String, // "United Kingdom", "Germany Mobile", etc.
    pub jurisdiction: InternationalJurisdiction, // EEA or ROW
    pub rate: Decimal,        // Single rate for international
    pub initial_increment: i32, // Initial billing increment in seconds (e.g., 30, 60, 6)
    pub subsequent_increment: i32, // Subsequent billing increment (e.g., 6, 60, 1)
    pub setup_fee: Option<Decimal>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallSimulation {
    pub ani: String,
    pub dnis: String,
    pub lrn: Option<String>,
    pub jurisdiction: CallJurisdiction,
    pub ingress_trunk: String,
    pub total_routes: usize,
    pub routes: Vec<SimulatedRoute>,
    pub routing_decision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulatedRoute {
    pub egress_trunk: String,
    pub vendor: String,
    pub cost_per_minute: Decimal,
    pub selling_per_minute: Decimal,
    pub profit_margin: Decimal,
    pub priority: i32,
    pub setup_fee: Decimal,
    pub min_increment: i32,
    pub interval: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RateDeck {
    pub id: i32,
    pub name: String,
    pub owner_id: i32, // vendor_id or client_id
    pub rate_type: RateType,
    pub effective_date: DateTime<Utc>,
    pub end_date: Option<DateTime<Utc>>,
    pub deck_version: i32,
    pub parent_deck_id: Option<i32>,
    pub effective_time: NaiveTime,
    pub preload_minutes: i32,
    pub loaded_at: Option<DateTime<Utc>>,
    pub is_staged: bool,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeckCutoverSchedule {
    pub id: i32,
    pub deck_type: String,
    pub current_deck_id: i32,
    pub new_deck_id: i32,
    pub cutover_date: DateTime<Utc>,
    pub preload_at: DateTime<Utc>,
    pub status: String, // Changed from CutoverStatus enum to String for simpler DB mapping
    pub preloaded_at: Option<DateTime<Utc>>,
    pub activated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CutoverStatus {
    Scheduled,
    Preloading,
    Preloaded,
    Active,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeckLoadRequest {
    pub deck_name: String,
    pub owner_id: i32,
    pub rate_type: RateType,
    pub effective_date: DateTime<Utc>,
    pub effective_time: Option<NaiveTime>,
    pub preload_minutes: Option<i32>,
    pub rates_csv: Option<String>,
    pub rates_data: Option<Vec<NanpaRate>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportProtocol {
    Udp,
    Tcp,
    Tls,
    Ws,
    Wss,
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
    pub supports_international: bool,
}

impl fmt::Display for EgressTrunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({}:{})", self.name, self.host, self.port)
    }
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
    pub supports_international: bool,
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
    /// Whether the number is ported (has different LRN)
    pub ported: bool,
    /// Response time from LRN dip server
    pub dip_response_time_ms: Option<u64>,
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
    pub vendor: String,
    pub vendor_rate: Option<NanpaRate>,
    pub cost_per_minute: Decimal,
    pub selling_per_minute: Decimal,
    pub profit_margin: Decimal,
    pub priority: i32,
    pub setup_fee: Decimal,
    pub min_increment: i32,
    pub interval: i32,
}

/// International routing plan configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternationalRoutingPlan {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    // Phone validation settings
    pub phone_validation_enabled: bool,
    pub phone_validation_strict: bool,
    pub phone_validation_default_region: String,
    pub phone_validation_use_country_detection: bool,
    // EEA routing settings
    pub eea_routing_enabled: bool,
    pub eea_priority_routing: bool,
    pub eea_reduced_rates: bool,
    pub eea_rate_reduction: Decimal,
    // Default routing settings
    pub default_jurisdiction: InternationalJurisdiction,
    pub allow_unknown_destinations: bool,
    pub max_rate_unknown_destinations: Decimal,
    pub require_strict_validation_unknown: bool,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Country-specific routing preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountryRoutingPreference {
    pub id: i32,
    pub routing_plan_id: i32,
    pub country_code: String, // ISO 2-letter country code
    pub country_name: String,
    /// Preferred jurisdiction classification
    pub jurisdiction: InternationalJurisdiction,
    /// Quality scoring for this destination
    pub quality_score: i32,
    /// Cost multiplier (1.0 = normal, >1.0 = more expensive)
    pub cost_multiplier: Decimal,
    /// Whether to require phone validation for this country
    pub require_validation: bool,
    /// Maximum call duration in minutes (0 = unlimited)
    pub max_duration_minutes: i32,
    pub created_at: DateTime<Utc>,
}

/// EEA routing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EeaRoutingConfig {
    /// Enable EEA-specific routing
    pub enabled: bool,
    /// EEA countries get priority routing
    pub priority_routing: bool,
    /// Apply reduced rates for EEA destinations
    pub reduced_rates: bool,
    /// Rate reduction percentage (0.1 = 10% reduction)
    pub rate_reduction: Decimal,
}

/// Default routing configuration for unknown destinations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultRoutingConfig {
    /// Default jurisdiction for unknown countries
    pub default_jurisdiction: InternationalJurisdiction,
    /// Whether to allow routing to unknown destinations
    pub allow_unknown: bool,
    /// Maximum rate per minute for unknown destinations
    pub max_rate_per_minute: Decimal,
    /// Require strict phone validation for unknown destinations
    pub require_strict_validation: bool,
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
    pub effective_time: Option<DateTime<Utc>>,
    /// Phone validation configuration for this request
    pub phone_validation: Option<PhoneValidationConfig>,
    /// International routing plan ID to use
    pub routing_plan_id: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteResponse {
    pub routes: Vec<CallRoute>,
    pub jurisdiction: CallJurisdiction,
    pub lrn: Option<String>,
    pub total_routes: usize,
    pub ani: String,
    pub dnis: String,
    pub ingress_trunk: String,
    pub routing_decision: String,
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

/// LRN Dip Server Configuration with multiple protocol support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LrnDipServer {
    /// Server IP address for LRN dips
    pub server_ip: IpAddr,
    /// Server port (default 5060 for SIP, 443 for HTTPS)
    pub server_port: u16,
    /// Server priority (lower = higher priority, 0 = highest)
    #[serde(default)]
    pub priority: u8,
    /// LRN dip protocol: "sip_302", "telique_api", "restapi", "soap"
    #[serde(default = "default_protocol")]
    pub protocol: String,
    /// Authentication credentials (for API-based methods)
    pub auth: Option<LrnAuthConfig>,
}

/// Authentication configuration for API-based LRN dips
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LrnAuthConfig {
    /// Authentication type: "basic", "bearer", "api_key"
    pub auth_type: String,
    /// Username for basic auth or API key name
    pub username: Option<String>,
    /// Password for basic auth or API key value
    pub password: Option<String>,
    /// Bearer token or API key
    pub token: Option<String>,
}

fn default_protocol() -> String {
    "sip_302".to_string()
}

/// LRN Dip Configuration with backup server support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LrnDipConfig {
    /// Primary LRN server (for backwards compatibility)
    #[serde(default)]
    pub server_ip: Option<IpAddr>,
    /// Primary LRN server port (for backwards compatibility)
    #[serde(default = "default_lrn_port")]
    pub server_port: u16,
    /// List of LRN dip servers (primary and backups)
    #[serde(default)]
    pub servers: Vec<LrnDipServer>,
    /// Local IP to bind for SIP client
    pub local_ip: Option<IpAddr>,
    /// Local port for SIP client (0 for random)
    pub local_port: Option<u16>,
    /// Timeout in milliseconds for LRN dips per server
    pub timeout_ms: u32,
    /// Timeout in milliseconds before trying backup server
    pub backup_timeout_ms: Option<u32>,
    /// Maximum number of redirects to follow
    pub max_redirects: u8,
    /// Enable/disable LRN dipping
    pub enabled: bool,
    /// Cache timeout for LRN results in seconds
    pub cache_timeout_sec: u32,
    /// Load balancing strategy: "priority" (failover) or "round_robin"
    #[serde(default = "default_lb_strategy")]
    pub load_balancing: String,
}

fn default_lrn_port() -> u16 {
    5060
}

fn default_lb_strategy() -> String {
    "priority".to_string()
}

impl LrnDipConfig {
    /// Get effective list of servers (backwards compatibility + new format)
    pub fn get_servers(&self) -> Vec<LrnDipServer> {
        if !self.servers.is_empty() {
            // Sort servers by priority if using priority strategy
            if self.load_balancing == "priority" {
                let mut servers = self.servers.clone();
                servers.sort_by_key(|s| s.priority);
                servers
            } else {
                self.servers.clone()
            }
        } else if let Some(ip) = self.server_ip {
            // Backwards compatibility: use server_ip/server_port
            vec![LrnDipServer {
                server_ip: ip,
                server_port: self.server_port,
                priority: 0,
                protocol: "sip_302".to_string(),
                auth: None,
            }]
        } else {
            vec![]
        }
    }

    /// Get the effective backup timeout (fallback to main timeout if not set)
    pub fn get_backup_timeout_ms(&self) -> u32 {
        self.backup_timeout_ms.unwrap_or(self.timeout_ms / 2)
    }
}

impl Default for LrnDipConfig {
    fn default() -> Self {
        Self {
            server_ip: Some("127.0.0.1".parse().expect("Valid IP address")),
            server_port: 5060,
            servers: vec![],
            local_ip: None,
            local_port: None,
            timeout_ms: 5000,
            backup_timeout_ms: Some(2000), // Try backup after 2 seconds
            max_redirects: 3,
            enabled: false,
            cache_timeout_sec: 3600,
            load_balancing: "priority".to_string(),
        }
    }
}

/// LRN Dip Request
#[derive(Debug, Clone)]
pub struct LrnDipRequest {
    pub tn: String,
    pub ani: Option<String>,
    pub request_id: String,
}

/// LRN Dip Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LrnDipResponse {
    pub original_tn: String,
    pub lrn: Option<String>,
    pub ported: bool,
    pub spid: Option<String>,
    pub response_time_ms: u64,
    pub redirect_count: u8,
    pub error: Option<String>,
}

/// SIP 302 Redirect Response Handler
#[derive(Debug, Clone)]
pub struct SipRedirectResponse {
    pub contact_uri: String,
    pub lrn: Option<String>,
    pub spid: Option<String>,
}
