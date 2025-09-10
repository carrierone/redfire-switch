//! CDR (Call Detail Records) module stub

use anyhow::Result;
use chrono::{DateTime, Utc};
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
    pub call_id: String,
    pub from_number: String,
    pub to_number: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_seconds: u64,
    pub disposition: CallDisposition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CallDisposition {
    Answered,
    NoAnswer,
    Busy,
    Failed,
}
