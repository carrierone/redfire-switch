//! Security utilities and configurations
//! 
//! This module provides security-focused utilities including secure configuration
//! management, input validation, and security audit logging.

pub mod config;
pub mod validation;
pub mod audit;
pub mod rate_limiting;

pub use config::*;
pub use validation::*;
pub use audit::*;
pub use rate_limiting::*;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{error, warn, info};

/// Security configuration for the entire system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable security audit logging
    pub enable_audit_logging: bool,
    /// Maximum request rate per IP (requests per minute)
    pub max_requests_per_minute: u32,
    /// Enable input validation
    pub enable_input_validation: bool,
    /// Maximum SIP message size in bytes
    pub max_sip_message_size: usize,
    /// Timeout for security operations in seconds
    pub security_timeout_seconds: u64,
    /// Enable TLS for all communications
    pub require_tls: bool,
    /// Allowed IP ranges for administrative access
    pub admin_allowed_ips: Vec<String>,
    /// Enable rate limiting
    pub enable_rate_limiting: bool,
    /// Maximum concurrent connections per IP
    pub max_connections_per_ip: u32,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_audit_logging: true,
            max_requests_per_minute: 60,
            enable_input_validation: true,
            max_sip_message_size: 65536, // 64KB max SIP message
            security_timeout_seconds: 30,
            require_tls: false, // Default to false for compatibility, should be true in production
            admin_allowed_ips: vec!["127.0.0.1".to_string(), "::1".to_string()],
            enable_rate_limiting: true,
            max_connections_per_ip: 100,
        }
    }
}

/// Security context for operations
#[derive(Debug, Clone)]
pub struct SecurityContext {
    /// Source IP address
    pub source_ip: std::net::IpAddr,
    /// User agent string
    pub user_agent: Option<String>,
    /// Authentication status
    pub authenticated: bool,
    /// User ID if authenticated
    pub user_id: Option<String>,
    /// Session ID
    pub session_id: Option<String>,
    /// Operation timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl SecurityContext {
    /// Create a new security context
    pub fn new(source_ip: std::net::IpAddr) -> Self {
        Self {
            source_ip,
            user_agent: None,
            authenticated: false,
            user_id: None,
            session_id: None,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Set authentication information
    pub fn with_auth(mut self, user_id: String, session_id: String) -> Self {
        self.authenticated = true;
        self.user_id = Some(user_id);
        self.session_id = Some(session_id);
        self
    }

    /// Set user agent
    pub fn with_user_agent(mut self, user_agent: String) -> Self {
        self.user_agent = Some(user_agent);
        self
    }

    /// Check if the source IP is in the allowed admin IP list
    pub fn is_admin_allowed(&self, config: &SecurityConfig) -> bool {
        let ip_str = self.source_ip.to_string();
        config.admin_allowed_ips.iter().any(|allowed_ip| {
            if allowed_ip.contains('/') {
                // CIDR notation - simplified check (in production use proper CIDR library)
                ip_str.starts_with(&allowed_ip.split('/').next().unwrap_or("").replace(".0", ""))
            } else {
                ip_str == *allowed_ip
            }
        })
    }
}

/// Security error types
#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    #[error("Authentication required")]
    AuthenticationRequired,
    #[error("Access denied")]
    AccessDenied,
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Request too large: {0} bytes")]
    RequestTooLarge(usize),
    #[error("Timeout occurred")]
    Timeout,
    #[error("TLS required")]
    TlsRequired,
}

/// Initialize security subsystem
pub fn initialize_security(config: &SecurityConfig) -> Result<()> {
    info!("Initializing security subsystem");
    
    if config.enable_audit_logging {
        audit::initialize_audit_logging()?;
        info!("Security audit logging enabled");
    }
    
    if config.enable_rate_limiting {
        info!("Rate limiting enabled: {} requests/minute", config.max_requests_per_minute);
    }
    
    if config.enable_input_validation {
        info!("Input validation enabled");
    }
    
    if config.require_tls {
        info!("TLS enforcement enabled");
    } else {
        warn!("TLS enforcement disabled - not recommended for production");
    }
    
    info!("Security subsystem initialized successfully");
    Ok(())
}