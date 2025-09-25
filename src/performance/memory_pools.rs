//! High-performance memory pools for carrier-grade call processing
//! Eliminates allocation overhead for hot path objects

use chrono::{DateTime, Utc};
use object_pool::{Pool, Reusable};
use once_cell::sync::Lazy;
use smallstr::SmallString;
use smallvec::SmallVec;
use std::net::IpAddr;
use uuid::Uuid;

/// Small string optimized for phone numbers and identifiers
pub type FastString = SmallString<[u8; 32]>;

/// Small vector optimized for route lists
pub type RouteVec<T> = SmallVec<[T; 8]>;

/// Call session optimized for memory pools
#[derive(Debug, Clone)]
pub struct PooledCallSession {
    pub id: Uuid,
    pub call_id: FastString,
    pub from_addr: IpAddr,
    pub to_addr: IpAddr,
    pub start_time: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub state: CallState,
    pub trunk_id: Option<FastString>,
    pub codec_pair: Option<(FastString, FastString)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CallState {
    Establishing,
    Active,
    Terminating,
    Failed,
}

impl Default for PooledCallSession {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            call_id: FastString::new(),
            from_addr: "0.0.0.0".parse().unwrap(),
            to_addr: "0.0.0.0".parse().unwrap(),
            start_time: Utc::now(),
            last_activity: Utc::now(),
            state: CallState::Establishing,
            trunk_id: None,
            codec_pair: None,
        }
    }
}

/// Route request optimized for memory pools
#[derive(Debug, Clone)]
pub struct PooledRouteRequest {
    pub ani: FastString,
    pub dnis: FastString,
    pub ingress_trunk_id: i32,
    pub route_type: RouteType,
    pub effective_time: Option<DateTime<Utc>>,
    pub client_deck_id: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RouteType {
    NANPA,
    AZ,
    OTHER,
}

impl Default for PooledRouteRequest {
    fn default() -> Self {
        Self {
            ani: FastString::new(),
            dnis: FastString::new(),
            ingress_trunk_id: 0,
            route_type: RouteType::NANPA,
            effective_time: None,
            client_deck_id: None,
        }
    }
}

/// Route response optimized for memory pools
#[derive(Debug, Clone)]
pub struct PooledRouteResponse {
    pub routes: RouteVec<PooledCallRoute>,
    pub jurisdiction: CallJurisdiction,
    pub lrn: Option<FastString>,
    pub client_rate: Option<PooledRate>,
    pub total_routes: usize,
}

#[derive(Debug, Clone)]
pub struct PooledCallRoute {
    pub egress_trunk: PooledEgressTrunk,
    pub vendor_rate: Option<PooledRate>,
    pub client_rate: Option<PooledRate>,
    pub lcr_score: f64,
    pub jurisdiction: CallJurisdiction,
    pub lrn: Option<FastString>,
}

#[derive(Debug, Clone)]
pub struct PooledEgressTrunk {
    pub id: i32,
    pub name: FastString,
    pub ip_address: IpAddr,
    pub port: u16,
    pub active: bool,
    pub capacity: u32,
}

#[derive(Debug, Clone)]
pub struct PooledRate {
    pub rate: rust_decimal::Decimal,
    pub effective_date: DateTime<Utc>,
    pub rating_code: FastString,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CallJurisdiction {
    Intrastate,
    Interstate,
    International,
    Indeterminate,
}

impl Default for PooledRouteResponse {
    fn default() -> Self {
        Self {
            routes: RouteVec::new(),
            jurisdiction: CallJurisdiction::Indeterminate,
            lrn: None,
            client_rate: None,
            total_routes: 0,
        }
    }
}

/// SIP message context optimized for memory pools
#[derive(Debug, Clone)]
pub struct PooledSipContext {
    pub call_id: FastString,
    pub from_uri: FastString,
    pub to_uri: FastString,
    pub calling_number: FastString,
    pub called_number: FastString,
    pub tech_prefix: Option<FastString>,
    pub trunk_id: Option<FastString>,
    pub customer_id: Option<FastString>,
    pub source_ip: std::net::SocketAddr,
    pub transport: SipTransport,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy)]
pub enum SipTransport {
    Udp,
    Tcp,
    Tls,
    Ws,
    Wss,
}

impl Default for PooledSipContext {
    fn default() -> Self {
        Self {
            call_id: FastString::new(),
            from_uri: FastString::new(),
            to_uri: FastString::new(),
            calling_number: FastString::new(),
            called_number: FastString::new(),
            tech_prefix: None,
            trunk_id: None,
            customer_id: None,
            source_ip: "0.0.0.0:0".parse().unwrap(),
            transport: SipTransport::Udp,
            created_at: Utc::now(),
            last_activity: Utc::now(),
        }
    }
}

/// Global memory pools for high-frequency objects
pub struct MemoryPools {
    pub call_sessions: Pool<PooledCallSession>,
    pub route_requests: Pool<PooledRouteRequest>,
    pub route_responses: Pool<PooledRouteResponse>,
    pub sip_contexts: Pool<PooledSipContext>,
    pub string_buffers: Pool<String>,
}

impl MemoryPools {
    pub fn new() -> Self {
        Self {
            call_sessions: Pool::new(1000, || PooledCallSession::default()),
            route_requests: Pool::new(500, || PooledRouteRequest::default()),
            route_responses: Pool::new(500, || PooledRouteResponse::default()),
            sip_contexts: Pool::new(1000, || PooledSipContext::default()),
            string_buffers: Pool::new(2000, || String::with_capacity(256)),
        }
    }

    /// Get a reusable call session object
    pub fn get_call_session(&self) -> Reusable<'_, PooledCallSession> {
        let mut session = self.call_sessions.pull(PooledCallSession::default);

        // Reset to default state
        session.id = Uuid::new_v4();
        session.call_id.clear();
        session.from_addr = "0.0.0.0".parse().unwrap();
        session.to_addr = "0.0.0.0".parse().unwrap();
        session.start_time = Utc::now();
        session.last_activity = Utc::now();
        session.state = CallState::Establishing;
        session.trunk_id = None;
        session.codec_pair = None;

        session
    }

    /// Get a reusable route request object
    pub fn get_route_request(&self) -> Reusable<'_, PooledRouteRequest> {
        let mut request = self.route_requests.pull(PooledRouteRequest::default);

        // Reset to default state
        request.ani.clear();
        request.dnis.clear();
        request.ingress_trunk_id = 0;
        request.route_type = RouteType::NANPA;
        request.effective_time = None;
        request.client_deck_id = None;

        request
    }

    /// Get a reusable route response object
    pub fn get_route_response(&self) -> Reusable<'_, PooledRouteResponse> {
        let mut response = self.route_responses.pull(PooledRouteResponse::default);

        // Reset to default state
        response.routes.clear();
        response.jurisdiction = CallJurisdiction::Indeterminate;
        response.lrn = None;
        response.client_rate = None;
        response.total_routes = 0;

        response
    }

    /// Get a reusable SIP context object
    pub fn get_sip_context(&self) -> Reusable<'_, PooledSipContext> {
        let mut context = self.sip_contexts.pull(PooledSipContext::default);

        // Reset to default state
        context.call_id.clear();
        context.from_uri.clear();
        context.to_uri.clear();
        context.calling_number.clear();
        context.called_number.clear();
        context.tech_prefix = None;
        context.trunk_id = None;
        context.customer_id = None;
        context.source_ip = "0.0.0.0:0".parse().unwrap();
        context.transport = SipTransport::Udp;
        context.created_at = Utc::now();
        context.last_activity = Utc::now();

        context
    }

    /// Get a reusable string buffer
    pub fn get_string_buffer(&self) -> Reusable<'_, String> {
        let mut buffer = self.string_buffers.pull(|| String::with_capacity(256));
        buffer.clear();
        buffer
    }
}

/// Global memory pools instance
pub static MEMORY_POOLS: Lazy<MemoryPools> = Lazy::new(|| MemoryPools::new());

/// Convenient access to global memory pools
pub fn pools() -> &'static MemoryPools {
    &MEMORY_POOLS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_call_session_pool() {
        let pools = MemoryPools::new();
        let session = pools.get_call_session();
        assert_eq!(session.state, CallState::Establishing);
        assert!(session.call_id.is_empty());
    }

    #[test]
    fn test_route_request_pool() {
        let pools = MemoryPools::new();
        let request = pools.get_route_request();
        assert_eq!(request.route_type, RouteType::NANPA);
        assert!(request.ani.is_empty());
    }

    #[test]
    fn test_string_buffer_pool() {
        let pools = MemoryPools::new();
        let buffer = pools.get_string_buffer();
        assert!(buffer.is_empty());
        assert!(buffer.capacity() >= 256);
    }
}
