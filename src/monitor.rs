/*
 * Redfire Switch - Basic Monitor Module (Stub for API Compatibility)
 * Copyright (C) 2025 Carrier One Inc and contributors
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::info;

#[derive(Debug, Clone)]
pub enum EndpointStatus {
    Unknown,
    Online,
    Offline,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct EndpointHealth {
    pub status: EndpointStatus,
    pub last_check: Instant,
    pub last_response_time: Option<Duration>,
    pub consecutive_failures: u32,
    pub total_pings: u64,
    pub successful_pings: u64,
}

impl Default for EndpointHealth {
    fn default() -> Self {
        EndpointHealth {
            status: EndpointStatus::Unknown,
            last_check: Instant::now(),
            last_response_time: None,
            consecutive_failures: 0,
            total_pings: 0,
            successful_pings: 0,
        }
    }
}

pub struct SipMonitor {
    endpoint_health: Arc<RwLock<HashMap<String, EndpointHealth>>>,
}

impl SipMonitor {
    pub fn new() -> Self {
        Self {
            endpoint_health: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_endpoint_status(&self, endpoint_name: &str) -> Option<EndpointHealth> {
        let health_guard = self.endpoint_health.read().await;
        health_guard.get(endpoint_name).cloned()
    }

    pub async fn get_all_endpoint_status(&self) -> HashMap<String, EndpointHealth> {
        let health_guard = self.endpoint_health.read().await;
        health_guard.clone()
    }

    pub async fn enable_endpoint(&self, endpoint_name: &str) -> Result<()> {
        info!("Enable endpoint {} (stub implementation)", endpoint_name);
        Ok(())
    }

    pub async fn disable_endpoint(&self, endpoint_name: &str) -> Result<()> {
        info!("Disable endpoint {} (stub implementation)", endpoint_name);
        Ok(())
    }

    pub async fn start(&self) -> Result<()> {
        info!("SIP monitoring started (stub implementation)");
        Ok(())
    }
}
