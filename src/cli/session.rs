//! Session management for RedFire Switch CLI
//!
//! Manages connection state, configuration, and session data
//! for the interactive CLI interface.

use anyhow::{Context, Result};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::{debug, info, warn};

/// CLI session state and connection management
pub struct CliSession {
    /// Connection status
    connected: bool,
    /// Target host and port
    target_host: String,
    target_port: u16,
    /// Session start time
    session_start: SystemTime,
    /// Last activity timestamp
    last_activity: SystemTime,
    /// Session configuration
    config: SessionConfig,
    /// Connection handle (if connected)
    connection: Option<TcpStream>,
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
            target_host: "localhost".to_string(),
            target_port: 8080,
            session_start: now,
            last_activity: now,
            config: SessionConfig::default(),
            connection: None,
        }
    }

    /// Connect to RedFire Switch instance
    pub async fn connect(&mut self, address: String) -> Result<()> {
        info!("Attempting to connect to {}", address);
        
        // Parse address
        let parts: Vec<&str> = address.split(':').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid address format. Use host:port"));
        }
        
        let host = parts[0].to_string();
        let port: u16 = parts[1].parse()
            .context("Invalid port number")?;
        
        self.target_host = host.clone();
        self.target_port = port;
        
        // Attempt connection with timeout
        match timeout(
            self.config.connect_timeout,
            TcpStream::connect(format!("{}:{}", host, port))
        ).await {
            Ok(Ok(stream)) => {
                self.connection = Some(stream);
                self.connected = true;
                self.update_activity();
                info!("Successfully connected to {}:{}", host, port);
                Ok(())
            }
            Ok(Err(e)) => {
                warn!("Failed to connect to {}:{}: {}", host, port, e);
                self.connected = false;
                Err(anyhow::anyhow!("Connection failed: {}", e))
            }
            Err(_) => {
                warn!("Connection timeout to {}:{}", host, port);
                self.connected = false;
                Err(anyhow::anyhow!("Connection timeout"))
            }
        }
    }

    /// Disconnect from RedFire Switch instance
    pub async fn disconnect(&mut self) {
        if self.connected {
            info!("Disconnecting from {}:{}", self.target_host, self.target_port);
            self.connection = None;
            self.connected = false;
        }
    }

    /// Check if currently connected
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Get target host for display
    pub fn get_target_host(&self) -> String {
        format!("{}:{}", self.target_host, self.target_port)
    }

    /// Update last activity timestamp
    pub fn update_activity(&mut self) {
        self.last_activity = SystemTime::now();
    }

    /// Get session uptime
    pub fn get_uptime(&self) -> Duration {
        self.session_start.elapsed().unwrap_or(Duration::from_secs(0))
    }

    /// Get time since last activity
    pub fn get_idle_time(&self) -> Duration {
        self.last_activity.elapsed().unwrap_or(Duration::from_secs(0))
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