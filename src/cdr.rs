//! CDR (Call Detail Records) module stub

use anyhow::Result;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdrConfig {
    pub enabled: bool,
    pub storage_path: String,
    pub rotation_days: u32,
}

impl Default for CdrConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            storage_path: String::from("/var/log/redfire/cdr"),
            rotation_days: 30,
        }
    }
}

pub struct CdrService {
    #[allow(dead_code)]
    config: CdrConfig,
}

impl CdrService {
    pub fn new(config: CdrConfig) -> Self {
        Self { config }
    }

    pub async fn record_call(&self, _cdr: CallDetailRecord) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallDetailRecord {
    pub id: Option<String>,
    pub call_id: String,
    pub session_id: Option<String>,
    pub from_number: String,
    pub to_number: String,
    pub from_ip: Option<std::net::IpAddr>,
    pub to_ip: Option<std::net::IpAddr>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_seconds: u64,
    pub disposition: CallDisposition,
    pub hangup_cause: Option<u32>,
    pub trunk_id: Option<String>,
    pub route_id: Option<String>,
    pub codec_in: Option<String>,
    pub codec_out: Option<String>,
    pub recording_enabled: bool,
    pub cost: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CallDisposition {
    /// Call was answered and established successfully
    Answered,
    /// Call was not answered (timeout or busy)
    NoAnswer,
    /// Call was busy
    Busy,
    /// Call failed due to network error
    Failed,
    /// Call was cancelled by caller
    Cancelled,
    /// Call was rejected by callee
    Rejected,
    /// Call failed due to routing error
    RoutingFailed,
    /// Call failed due to codec negotiation
    CodecFailed,
    /// Call failed due to authentication
    AuthFailed,
}
