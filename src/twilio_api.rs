//! Twilio API module stub

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwilioApiConfig {
    pub enabled: bool,
    pub account_sid: String,
    pub auth_token: String,
}

impl Default for TwilioApiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            account_sid: String::new(),
            auth_token: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationsConfig {
    pub enabled: bool,
}

impl Default for ConversationsConfig {
    fn default() -> Self {
        Self { enabled: false }
    }
}
