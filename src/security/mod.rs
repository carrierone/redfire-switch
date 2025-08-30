//! Security utilities and configurations
//!
//! This module provides comprehensive security features including secure configuration
//! management, input validation, security audit logging, threat detection, and IP blacklisting.
//! All features support global and per-trunk configuration overrides.

pub mod audit;
pub mod blacklist;
pub mod config;
pub mod rate_limiting;
pub mod threat_detection;
pub mod validation;

pub use audit::*;
pub use blacklist::*;
pub use config::*;
pub use rate_limiting::*;
pub use threat_detection::*;
pub use validation::*;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{error::Error as StdError, fmt};
use tracing::{info, warn};

/// Security-related error types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityError {
    ValidationFailed(String),
    RateLimitExceeded(String),
    ThreatDetected(String),
    BlacklistViolation(String),
    ConfigurationError(String),
    AuthenticationFailed(String),
    AuthenticationRequired(String),
    AccessDenied(String),
    InvalidInput(String),
    RequestTooLarge(String),
    Timeout(String),
    TlsRequired(String),
}

impl fmt::Display for SecurityError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SecurityError::ValidationFailed(msg) => write!(f, "Validation failed: {}", msg),
            SecurityError::RateLimitExceeded(msg) => write!(f, "Rate limit exceeded: {}", msg),
            SecurityError::ThreatDetected(msg) => write!(f, "Threat detected: {}", msg),
            SecurityError::BlacklistViolation(msg) => write!(f, "Blacklist violation: {}", msg),
            SecurityError::ConfigurationError(msg) => write!(f, "Configuration error: {}", msg),
            SecurityError::AuthenticationFailed(msg) => write!(f, "Authentication failed: {}", msg),
            SecurityError::AuthenticationRequired(msg) => {
                write!(f, "Authentication required: {}", msg)
            }
            SecurityError::AccessDenied(msg) => write!(f, "Access denied: {}", msg),
            SecurityError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            SecurityError::RequestTooLarge(msg) => write!(f, "Request too large: {}", msg),
            SecurityError::Timeout(msg) => write!(f, "Security timeout: {}", msg),
            SecurityError::TlsRequired(msg) => write!(f, "TLS required: {}", msg),
        }
    }
}

impl StdError for SecurityError {}

/// Comprehensive security configuration with global and per-trunk settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable security audit logging globally
    pub enable_audit_logging: bool,
    /// Enable rate limiting globally
    pub enable_rate_limiting: bool,
    /// Enable input validation globally
    pub enable_input_validation: bool,
    /// Enable threat detection globally
    pub enable_threat_detection: bool,
    /// Enable IP blacklisting globally
    pub enable_blacklisting: bool,
    /// Enable reputation scoring globally
    pub enable_reputation_scoring: bool,
    /// Maximum request rate per IP (requests per minute)
    pub max_requests_per_minute: u32,
    /// Maximum concurrent connections per IP
    pub max_connections_per_ip: u32,
    /// Maximum SIP message size in bytes
    pub max_sip_message_size: usize,
    /// Timeout for security operations in seconds
    pub security_timeout_seconds: u64,
    /// Enable TLS for all communications
    pub require_tls: bool,
    /// Allowed IP ranges for administrative access
    pub admin_allowed_ips: Vec<String>,
    /// Enable per-trunk security overrides
    pub enable_per_trunk_overrides: bool,
    /// Threat detection configuration
    pub threat_detection: threat_detection::ThreatDetectionConfig,
    /// Blacklist and reputation configuration
    pub blacklist: blacklist::BlacklistConfig,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_audit_logging: true,
            enable_rate_limiting: true,
            enable_input_validation: true,
            enable_threat_detection: true,
            enable_blacklisting: true,
            enable_reputation_scoring: true,
            max_requests_per_minute: 60,
            max_connections_per_ip: 100,
            max_sip_message_size: 65536, // 64KB max SIP message
            security_timeout_seconds: 30,
            require_tls: false, // Default to false for compatibility, should be true in production
            admin_allowed_ips: vec!["127.0.0.1".to_string(), "::1".to_string()],
            enable_per_trunk_overrides: true,
            threat_detection: threat_detection::ThreatDetectionConfig::default(),
            blacklist: blacklist::BlacklistConfig::default(),
        }
    }
}

/// Security context for operations with trunk-specific information
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
    /// Trunk ID for per-trunk security settings
    pub trunk_id: Option<String>,
    /// Additional security metadata
    pub metadata: std::collections::HashMap<String, String>,
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
            trunk_id: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Create security context with trunk information
    pub fn with_trunk(source_ip: std::net::IpAddr, trunk_id: String) -> Self {
        Self {
            source_ip,
            user_agent: None,
            authenticated: false,
            user_id: None,
            session_id: None,
            timestamp: chrono::Utc::now(),
            trunk_id: Some(trunk_id),
            metadata: std::collections::HashMap::new(),
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

    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Get effective trunk ID (for trunk-specific security rules)
    pub fn get_trunk_id(&self) -> Option<&str> {
        self.trunk_id.as_deref()
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

// SecurityError is already defined above - removing duplicate

/// Initialize security subsystem
pub fn initialize_security(config: &SecurityConfig) -> Result<()> {
    info!("Initializing security subsystem");

    if config.enable_audit_logging {
        audit::initialize_audit_logging()?;
        info!("Security audit logging enabled");
    }

    if config.enable_rate_limiting {
        info!(
            "Rate limiting enabled: {} requests/minute",
            config.max_requests_per_minute
        );
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
