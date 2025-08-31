//! RTP Proxy module stub

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtpProxyConfig {
    pub enabled: bool,
    pub listen_address: String,
    pub port_range_start: u16,
    pub port_range_end: u16,
}

impl Default for RtpProxyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            listen_address: String::from("0.0.0.0"),
            port_range_start: 10000,
            port_range_end: 20000,
        }
    }
}
