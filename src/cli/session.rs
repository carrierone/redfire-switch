//! Session management for RedFire Switch CLI
//!
//! Manages connection state, configuration, and session data
//! for the interactive CLI interface.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use tracing::{debug, info, warn};

/// Standard API response wrapper
#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

/// System statistics from the API
#[derive(Debug, Deserialize)]
pub struct SystemStats {
    pub active_calls: u32,
    pub uptime_seconds: u64,
    pub timestamp: String,
}

/// CLI session state and connection management
pub struct CliSession {
    /// Connection status
    connected: bool,
    /// Target API base URL
    api_base_url: String,
    /// Session start time
    session_start: SystemTime,
    /// Last activity timestamp
    last_activity: SystemTime,
    /// Session configuration
    config: SessionConfig,
    /// HTTP client for API calls
    http_client: reqwest::Client,
}

/// Configuration for CLI session
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Connection timeout in seconds
    pub connect_timeout: Duration,
    /// Command timeout in seconds
    pub command_timeout: Duration,
    /// Auto-reconnect on connection loss
    pub auto_reconnect: bool,
    /// Maximum command history to keep
    pub max_history: usize,
    /// Enable verbose output
    pub verbose: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            command_timeout: Duration::from_secs(30),
            auto_reconnect: true,
            max_history: 1000,
            verbose: false,
        }
    }
}

impl CliSession {
    /// Create a new CLI session
    pub fn new() -> Self {
        let now = SystemTime::now();
        Self {
            connected: false,
            api_base_url: "http://localhost:8080".to_string(),
            session_start: now,
            last_activity: now,
            config: SessionConfig::default(),
            http_client: reqwest::Client::new(),
        }
    }

    /// Connect to RedFire Switch instance
    pub async fn connect(&mut self, address: String) -> Result<()> {
        info!("Attempting to connect to {}", address);

        // Parse address and construct API base URL
        let api_base_url = if address.starts_with("http://") || address.starts_with("https://") {
            address.clone()
        } else {
            format!("http://{}", address)
        };

        self.api_base_url = api_base_url.clone();

        // Test connection by making a health check API call
        let health_url = format!("{}/api/v1/system/stats", api_base_url);

        match tokio::time::timeout(
            self.config.connect_timeout,
            self.http_client.get(&health_url).send(),
        )
        .await
        {
            Ok(Ok(response)) => {
                if response.status().is_success() {
                    self.connected = true;
                    self.update_activity();
                    info!("Successfully connected to {}", api_base_url);
                    Ok(())
                } else {
                    warn!("API server returned error: {}", response.status());
                    self.connected = false;
                    Err(anyhow::anyhow!(
                        "API server returned error: {}",
                        response.status()
                    ))
                }
            }
            Ok(Err(e)) => {
                warn!("Failed to connect to {}: {}", api_base_url, e);
                self.connected = false;
                Err(anyhow::anyhow!("Connection failed: {}", e))
            }
            Err(_) => {
                warn!("Connection timeout to {}", api_base_url);
                self.connected = false;
                Err(anyhow::anyhow!("Connection timeout"))
            }
        }
    }

    /// Disconnect from RedFire Switch instance
    pub async fn disconnect(&mut self) {
        if self.connected {
            info!("Disconnecting from {}", self.api_base_url);
            self.connected = false;
        }
    }

    /// Check if currently connected
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Get target host for display
    pub fn get_target_host(&self) -> String {
        self.api_base_url
            .replace("http://", "")
            .replace("https://", "")
    }

    /// Update last activity timestamp
    pub fn update_activity(&mut self) {
        self.last_activity = SystemTime::now();
    }

    /// Get session uptime
    pub fn get_uptime(&self) -> Duration {
        self.session_start
            .elapsed()
            .unwrap_or(Duration::from_secs(0))
    }

    /// Get time since last activity
    pub fn get_idle_time(&self) -> Duration {
        self.last_activity
            .elapsed()
            .unwrap_or(Duration::from_secs(0))
    }

    /// Get session configuration
    pub fn get_config(&self) -> &SessionConfig {
        &self.config
    }

    /// Update session configuration
    pub fn set_config(&mut self, config: SessionConfig) {
        self.config = config;
    }

    /// Check if auto-reconnect is enabled and connection is lost
    pub fn should_reconnect(&self) -> bool {
        self.config.auto_reconnect && !self.connected
    }

    /// Get connection status as string
    pub fn get_status_string(&self) -> String {
        if self.connected {
            format!("Connected to {}", self.get_target_host())
        } else {
            "Disconnected".to_string()
        }
    }

    /// Make an API request to the connected server
    pub async fn api_request<T>(&self, method: reqwest::Method, endpoint: &str) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        if !self.connected {
            return Err(anyhow::anyhow!("Not connected to API server"));
        }

        let url = format!("{}{}", self.api_base_url, endpoint);

        let response = tokio::time::timeout(
            self.config.command_timeout,
            self.http_client.request(method, &url).send(),
        )
        .await??;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "API request failed: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            ));
        }

        let api_response: ApiResponse<T> = response.json().await?;

        if api_response.success {
            api_response
                .data
                .ok_or_else(|| anyhow::anyhow!("API response missing data"))
        } else {
            Err(anyhow::anyhow!(
                "API error: {}",
                api_response
                    .error
                    .unwrap_or_else(|| "Unknown error".to_string())
            ))
        }
    }

    /// Get session statistics
    pub fn get_stats(&self) -> SessionStats {
        SessionStats {
            uptime: self.get_uptime(),
            idle_time: self.get_idle_time(),
            connected: self.connected,
            target: self.get_target_host(),
            auto_reconnect: self.config.auto_reconnect,
        }
    }

    /// Get system statistics from API
    pub async fn get_system_stats(&self) -> Result<SystemStats> {
        self.api_request(reqwest::Method::GET, "/api/v1/system/stats")
            .await
    }
}

/// Session statistics for display
#[derive(Debug)]
pub struct SessionStats {
    pub uptime: Duration,
    pub idle_time: Duration,
    pub connected: bool,
    pub target: String,
    pub auto_reconnect: bool,
}

impl SessionStats {
    /// Format uptime as human-readable string
    pub fn format_uptime(&self) -> String {
        let secs = self.uptime.as_secs();
        let hours = secs / 3600;
        let minutes = (secs % 3600) / 60;
        let seconds = secs % 60;

        if hours > 0 {
            format!("{}h {}m {}s", hours, minutes, seconds)
        } else if minutes > 0 {
            format!("{}m {}s", minutes, seconds)
        } else {
            format!("{}s", seconds)
        }
    }

    /// Format idle time as human-readable string
    pub fn format_idle_time(&self) -> String {
        let secs = self.idle_time.as_secs();
        if secs < 60 {
            format!("{}s", secs)
        } else if secs < 3600 {
            format!("{}m", secs / 60)
        } else {
            format!("{}h", secs / 3600)
        }
    }
}
