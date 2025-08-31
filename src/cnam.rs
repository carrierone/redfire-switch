//! CNAM module stub

use serde::{Deserialize, Serialize};
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CnamConfig {
    pub enabled: bool,
    pub provider_url: String,
    pub api_key: String,
}

impl Default for CnamConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider_url: String::new(),
            api_key: String::new(),
        }
    }
}

pub struct CnamService {
    config: CnamConfig,
}

impl CnamService {
    pub fn new(config: CnamConfig) -> Self {
        Self { config }
    }

    pub async fn lookup(&self, _number: &str) -> Result<String> {
        Ok(String::from("Unknown"))
    }
}
