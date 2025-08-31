//! Call Control module stub

use serde::{Deserialize, Serialize};
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallControlConfig {
    pub enabled: bool,
    pub max_concurrent_calls: usize,
}

impl Default for CallControlConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_concurrent_calls: 10000,
        }
    }
}

pub struct CallControlService;

impl CallControlService {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TrunkDirection {
    Inbound,
    Outbound,
    Bidirectional,
}

#[derive(Debug, Clone)]
pub struct TrunkGroupLimits {
    pub max_concurrent: usize,
    pub cps_limit: usize,
}
