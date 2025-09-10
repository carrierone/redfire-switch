//! SMS module stub

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsConfig {
    pub enabled: bool,
    pub gateway_url: String,
}

impl Default for SmsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            gateway_url: String::new(),
        }
    }
}

pub struct SmsService {
    config: SmsConfig,
}

impl SmsService {
    pub fn new(config: SmsConfig) -> Self {
        Self { config }
    }

    pub async fn send_message(&self, _message: SmsMessage) -> Result<String> {
        Ok(String::from("message-id"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsMessage {
    pub from: String,
    pub to: String,
    pub body: String,
    pub status: MessageStatus,
    pub direction: MessageDirection,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageStatus {
    Pending,
    Sent,
    Delivered,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageDirection {
    Inbound,
    Outbound,
}
