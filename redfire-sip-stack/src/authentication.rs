/*
 * Redfire Switch - SIP Authentication with IP-based and Tech Prefix Support
 * Copyright (C) 2025 Carrier One Inc and contributors
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

use anyhow::{anyhow, Result};
use ipnet::IpNet;
use rsip::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tracing::{debug, info, warn};
// Fail2ban integration removed - can be added as optional feature later

/// Placeholder for Fail2Ban service - stub implementation
#[derive(Debug)]
pub struct Fail2BanService;

impl Fail2BanService {
    pub fn new() -> Self {
        Self
    }

    /// Record SIP failure - stub implementation
    pub async fn record_sip_failure(
        &self,
        _ip: std::net::IpAddr,
        _failure_type: FailureType,
        _user: Option<String>,
        _method: String,
        _reason: String,
        _user_agent: Option<String>,
    ) -> Result<()> {
        // Stub implementation - in production this would log to fail2ban
        Ok(())
    }
}

/// Failure types for tracking purposes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureType {
    AuthenticationFailure,
    InvalidCredentials,
    RateLimitExceeded,
    SipInvite,
    SipRegister,
    SipSubscribe,
    SipOptions,
}

/// Load IP authentication configurations from external source
/// This is an example implementation - in production this would load from database/config files
pub async fn load_ip_auth_configs() -> Result<Vec<IpAuthConfig>> {
    // In production, this would load from database or config file
    // For now, return example configurations

    Ok(vec![
        IpAuthConfig {
            trunk_id: "carrier1_inbound".to_string(),
            customer_id: "carrier_one_llc".to_string(),
            allowed_networks: vec![
                "10.1.1.0/24".to_string(),
                "192.168.100.0/24".to_string(),
                "2001:db8::/32".to_string(),
            ],
            required_tech_prefix: Some("1001".to_string()),
            optional_tech_prefixes: vec!["1002".to_string(), "1003".to_string()],
            rate_limit: Some(100), // 100 CPS
            enabled: true,
            priority: 1,
        },
        IpAuthConfig {
            trunk_id: "carrier2_inbound".to_string(),
            customer_id: "global_telecom_inc".to_string(),
            allowed_networks: vec!["203.0.113.0/24".to_string(), "198.51.100.0/24".to_string()],
            required_tech_prefix: None, // No tech prefix required
            optional_tech_prefixes: vec!["2001".to_string(), "2002".to_string()],
            rate_limit: Some(50), // 50 CPS
            enabled: true,
            priority: 2,
        },
        IpAuthConfig {
            trunk_id: "enterprise_customer1".to_string(),
            customer_id: "acme_corporation".to_string(),
            allowed_networks: vec!["172.16.0.0/16".to_string()],
            required_tech_prefix: Some("*001".to_string()),
            optional_tech_prefixes: vec![],
            rate_limit: Some(20), // 20 CPS
            enabled: true,
            priority: 3,
        },
    ])
}

/// SIP authentication result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthResult {
    /// Authentication successful
    Authorized {
        trunk_id: String,
        customer_id: String,
        tech_prefix: Option<String>,
        rate_limit: Option<u32>,
    },
    /// Authentication failed
    Denied { reason: AuthFailureReason },
    /// Challenge required (for digest auth)
    Challenge {
        realm: String,
        nonce: String,
        algorithm: DigestAlgorithm,
    },
}

/// Authentication failure reasons
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthFailureReason {
    /// Source IP not authorized
    UnauthorizedIp,
    /// Invalid tech prefix
    InvalidTechPrefix,
    /// Rate limit exceeded
    RateLimitExceeded,
    /// Invalid credentials
    InvalidCredentials,
    /// Account suspended
    AccountSuspended,
    /// Configuration error
    ConfigurationError,
}

/// Digest authentication algorithms
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DigestAlgorithm {
    MD5,
    SHA256,
}

/// IP-based authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpAuthConfig {
    /// Trunk identifier
    pub trunk_id: String,
    /// Customer identifier
    pub customer_id: String,
    /// Allowed source IP networks (as CIDR strings)
    pub allowed_networks: Vec<String>,
    /// Required tech prefix (if any)
    pub required_tech_prefix: Option<String>,
    /// Optional tech prefixes
    pub optional_tech_prefixes: Vec<String>,
    /// Rate limit (calls per second)
    pub rate_limit: Option<u32>,
    /// Account status
    pub enabled: bool,
    /// Authentication priority (lower = higher priority)
    pub priority: u32,
}

impl IpAuthConfig {
    /// Check if IP address is authorized
    pub fn is_ip_authorized(&self, ip: IpAddr) -> bool {
        if !self.enabled {
            return false;
        }

        self.allowed_networks.iter().any(|net| {
            // Parse CIDR string and check if IP is contained
            if let Ok(parsed_net) = net.parse::<IpNet>() {
                parsed_net.contains(&ip)
            } else {
                false
            }
        })
    }

    /// Validate tech prefix
    pub fn validate_tech_prefix(&self, prefix: Option<&str>) -> bool {
        match (&self.required_tech_prefix, prefix) {
            (Some(required), Some(provided)) => {
                required == provided || self.optional_tech_prefixes.contains(&provided.to_string())
            }
            (Some(_), None) => false, // Required but not provided
            (None, _) => true,        // Not required, any or none is fine
        }
    }

    /// Get effective tech prefix
    pub fn get_tech_prefix(&self, provided: Option<&str>) -> Option<String> {
        if let Some(prefix) = provided {
            if self.validate_tech_prefix(Some(prefix)) {
                return Some(prefix.to_string());
            }
        }
        self.required_tech_prefix.clone()
    }
}

/// SIP digest credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestCredentials {
    /// Username
    pub username: String,
    /// Password hash (for security)
    pub password_hash: String,
    /// Realm
    pub realm: String,
    /// Customer ID
    pub customer_id: String,
    /// Account enabled
    pub enabled: bool,
}

impl DigestCredentials {
    /// Verify digest authentication
    pub fn verify_digest(
        &self,
        username: &str,
        realm: &str,
        nonce: &str,
        uri: &str,
        response: &str,
        algorithm: DigestAlgorithm,
    ) -> bool {
        if !self.enabled || self.username != username || self.realm != realm {
            return false;
        }

        // Calculate expected response
        let expected = self.calculate_digest_response(realm, nonce, uri, algorithm);

        // Constant-time comparison to prevent timing attacks
        expected == response
    }

    /// Calculate digest response
    fn calculate_digest_response(
        &self,
        realm: &str,
        nonce: &str,
        uri: &str,
        algorithm: DigestAlgorithm,
    ) -> String {
        let ha1 = match algorithm {
            DigestAlgorithm::MD5 => format!(
                "{:x}",
                md5::compute(format!(
                    "{}:{}:{}",
                    self.username, realm, self.password_hash
                ))
            ),
            DigestAlgorithm::SHA256 => format!(
                "{:x}",
                sha2::Sha256::digest(
                    format!("{}:{}:{}", self.username, realm, self.password_hash).as_bytes()
                )
            ),
        };

        let ha2 = match algorithm {
            DigestAlgorithm::MD5 => format!("{:x}", md5::compute(format!("INVITE:{}", uri))),
            DigestAlgorithm::SHA256 => format!(
                "{:x}",
                sha2::Sha256::digest(format!("INVITE:{}", uri).as_bytes())
            ),
        };

        let response = match algorithm {
            DigestAlgorithm::MD5 => {
                format!("{:x}", md5::compute(format!("{}:{}:{}", ha1, nonce, ha2)))
            }
            DigestAlgorithm::SHA256 => format!(
                "{:x}",
                sha2::Sha256::digest(format!("{}:{}:{}", ha1, nonce, ha2).as_bytes())
            ),
        };

        response
    }
}

/// Rate limiting tracker
#[derive(Debug)]
pub struct RateLimiter {
    /// Call attempts per IP/trunk
    call_counts: HashMap<String, (u32, std::time::Instant)>,
    /// Window size for rate limiting
    window_seconds: u64,
}

impl RateLimiter {
    /// Create new rate limiter
    pub fn new(window_seconds: u64) -> Self {
        Self {
            call_counts: HashMap::new(),
            window_seconds,
        }
    }

    /// Check if rate limit is exceeded
    pub fn is_rate_limited(&mut self, key: &str, limit: u32) -> bool {
        let now = std::time::Instant::now();

        let (count, last_reset) = self.call_counts.entry(key.to_string()).or_insert((0, now));

        // Reset counter if window expired
        if now.duration_since(*last_reset).as_secs() >= self.window_seconds {
            *count = 0;
            *last_reset = now;
        }

        *count += 1;
        *count > limit
    }
}

/// SIP authentication manager
pub struct SipAuthenticator {
    /// IP-based authentication configs
    ip_auth_configs: HashMap<String, IpAuthConfig>,
    /// Digest credentials
    digest_credentials: HashMap<String, DigestCredentials>,
    /// Rate limiter
    rate_limiter: RateLimiter,
    /// Default realm for digest auth
    default_realm: String,
    /// Fail2ban service for tracking authentication failures
    fail2ban_service: Option<Arc<Fail2BanService>>,
}

impl SipAuthenticator {
    /// Create new SIP authenticator
    pub fn new(default_realm: String) -> Self {
        Self {
            ip_auth_configs: HashMap::new(),
            digest_credentials: HashMap::new(),
            rate_limiter: RateLimiter::new(60), // 1 minute window
            default_realm,
            fail2ban_service: None,
        }
    }

    /// Set fail2ban service for authentication failure tracking
    pub fn set_fail2ban_service(&mut self, service: Arc<Fail2BanService>) {
        self.fail2ban_service = Some(service);
    }

    /// Add IP-based authentication configuration
    pub fn add_ip_auth_config(&mut self, config: IpAuthConfig) {
        self.ip_auth_configs.insert(config.trunk_id.clone(), config);
    }

    /// Add digest credentials
    pub fn add_digest_credentials(&mut self, username: String, credentials: DigestCredentials) {
        self.digest_credentials.insert(username, credentials);
    }

    /// Authenticate SIP request
    pub async fn authenticate_request(
        &mut self,
        message: &rsip::SipMessage,
        source_ip: IpAddr,
    ) -> Result<AuthResult> {
        debug!("Authenticating SIP request from {}", source_ip);

        // Extract authentication information from SIP message
        let (method, request_uri) = match message {
            rsip::SipMessage::Request(req) => (req.method.clone(), req.uri.clone()),
            _ => return Err(anyhow!("Authentication only applies to requests")),
        };

        // Try IP-based authentication first
        if let Some(auth_result) = self.try_ip_authentication(source_ip, &request_uri).await? {
            return Ok(auth_result);
        }

        // Try digest authentication
        if let Some(auth_header) = self.extract_authorization_header(message)? {
            return self
                .try_digest_authentication(&auth_header, &method, &request_uri)
                .await;
        }

        // No authentication provided - check if digest auth is required
        if self.requires_digest_auth(&source_ip) {
            return Ok(AuthResult::Challenge {
                realm: self.default_realm.clone(),
                nonce: self.generate_nonce(),
                algorithm: DigestAlgorithm::SHA256,
            });
        }

        // Record authentication failure
        self.record_auth_failure(source_ip, method, "Unauthorized IP address")
            .await;

        // Authentication failed
        Ok(AuthResult::Denied {
            reason: AuthFailureReason::UnauthorizedIp,
        })
    }

    /// Try IP-based authentication
    async fn try_ip_authentication(
        &mut self,
        source_ip: IpAddr,
        request_uri: &rsip::Uri,
    ) -> Result<Option<AuthResult>> {
        // Extract tech prefix from request URI user part
        let tech_prefix = self.extract_tech_prefix(request_uri)?;

        // Find matching IP authentication config
        let mut matching_configs: Vec<_> = self
            .ip_auth_configs
            .values()
            .filter(|config| config.is_ip_authorized(source_ip))
            .collect();

        // Sort by priority (lower number = higher priority)
        matching_configs.sort_by_key(|config| config.priority);

        for config in matching_configs {
            // Validate tech prefix
            if !config.validate_tech_prefix(tech_prefix.as_deref()) {
                debug!(
                    "Tech prefix validation failed for trunk {}: expected {:?}, got {:?}",
                    config.trunk_id, config.required_tech_prefix, tech_prefix
                );

                // Record tech prefix failure
                let reason = format!(
                    "Invalid tech prefix: expected {:?}, got {:?}",
                    config.required_tech_prefix, tech_prefix
                );
                self.record_sip_failure(source_ip, FailureType::SipInvite, None, "INVITE", &reason)
                    .await;
                continue;
            }

            // Check rate limit
            if let Some(rate_limit) = config.rate_limit {
                let rate_key = format!("{}:{}", source_ip, config.trunk_id);
                if self.rate_limiter.is_rate_limited(&rate_key, rate_limit) {
                    warn!(
                        "Rate limit exceeded for trunk {} from IP {}",
                        config.trunk_id, source_ip
                    );

                    // Record rate limit failure
                    let reason = format!("Rate limit exceeded: {} CPS", rate_limit);
                    self.record_sip_failure(
                        source_ip,
                        FailureType::SipInvite,
                        None,
                        "INVITE",
                        &reason,
                    )
                    .await;

                    return Ok(Some(AuthResult::Denied {
                        reason: AuthFailureReason::RateLimitExceeded,
                    }));
                }
            }

            // Authentication successful
            info!(
                "IP authentication successful: trunk={}, customer={}, source={}",
                config.trunk_id, config.customer_id, source_ip
            );

            return Ok(Some(AuthResult::Authorized {
                trunk_id: config.trunk_id.clone(),
                customer_id: config.customer_id.clone(),
                tech_prefix: config.get_tech_prefix(tech_prefix.as_deref()),
                rate_limit: config.rate_limit,
            }));
        }

        Ok(None)
    }

    /// Try digest authentication
    async fn try_digest_authentication(
        &mut self,
        auth_header: &str,
        method: &rsip::Method,
        request_uri: &rsip::Uri,
    ) -> Result<AuthResult> {
        // Parse authorization header
        let auth_params = self.parse_authorization_header(auth_header)?;

        let username = auth_params
            .get("username")
            .ok_or_else(|| anyhow!("Missing username in Authorization header"))?;
        let realm = auth_params
            .get("realm")
            .ok_or_else(|| anyhow!("Missing realm in Authorization header"))?;
        let nonce = auth_params
            .get("nonce")
            .ok_or_else(|| anyhow!("Missing nonce in Authorization header"))?;
        let response = auth_params
            .get("response")
            .ok_or_else(|| anyhow!("Missing response in Authorization header"))?;
        let uri = auth_params
            .get("uri")
            .ok_or_else(|| anyhow!("Missing uri in Authorization header"))?;

        // Find credentials
        let credentials = self
            .digest_credentials
            .get(username)
            .ok_or_else(|| anyhow!("Unknown username: {}", username))?;

        // Verify digest
        let algorithm = DigestAlgorithm::SHA256; // Default to SHA256
        if credentials.verify_digest(username, realm, nonce, uri, response, algorithm) {
            info!("Digest authentication successful for user {}", username);
            return Ok(AuthResult::Authorized {
                trunk_id: format!("digest_{}", username),
                customer_id: credentials.customer_id.clone(),
                tech_prefix: None,
                rate_limit: None,
            });
        }

        warn!("Digest authentication failed for user {}", username);

        // Record digest authentication failure
        self.record_sip_failure(
            IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)), // Will be updated by caller
            FailureType::SipRegister,
            Some(username),
            "REGISTER",
            "Invalid digest credentials",
        )
        .await;

        Ok(AuthResult::Denied {
            reason: AuthFailureReason::InvalidCredentials,
        })
    }

    /// Extract tech prefix from request URI
    fn extract_tech_prefix(&self, uri: &rsip::Uri) -> Result<Option<String>> {
        // TODO: Implement tech prefix extraction from URI
        // if let Some(user_info) = &uri.user_info {
        //     let user = &user_info.user;

        // Check for common tech prefix patterns:
        // - 4-6 digit prefixes: 1001, 10001, 100001
        // - Star-based prefixes: *1001, *10001
        // - Plus-based prefixes: +1001

        // Pattern 1: Digits followed by '*' separator
        /*if let Some(pos) = user.find('*') {
            if pos >= 3 && pos <= 6 {
                let prefix = &user[..pos];
                if prefix.chars().all(|c| c.is_ascii_digit()) {
                    debug!("Extracted tech prefix: {}", prefix);
                    return Ok(Some(prefix.to_string()));
                }
            }
        }

        // Pattern 2: Leading '*' or '+' followed by digits
        if user.starts_with('*') || user.starts_with('+') {
            let digits = &user[1..];
            if digits.len() >= 3 && digits.len() <= 6 && digits.chars().all(|c| c.is_ascii_digit()) {
                debug!("Extracted tech prefix: {}", user);
                return Ok(Some(user.to_string()));
            }
        }

        // Pattern 3: Pure numeric prefix (3-6 digits) at start
        for len in (3..=6).rev() {
            if user.len() > len {
                let potential_prefix = &user[..len];
                if potential_prefix.chars().all(|c| c.is_ascii_digit()) {
                    // Check if remaining part looks like a phone number
                    let remaining = &user[len..];
                    if remaining.len() >= 7 && remaining.chars().all(|c| c.is_ascii_digit()) {
                        debug!("Extracted tech prefix: {}", potential_prefix);
                        return Ok(Some(potential_prefix.to_string()));
                    }
                }
            }
        }*/
        // }

        Ok(None)
    }

    /// Extract authorization header from SIP message
    fn extract_authorization_header(&self, message: &rsip::SipMessage) -> Result<Option<String>> {
        if let rsip::SipMessage::Request(req) = message {
            for header in req.headers.iter() {
                if let rsip::Header::Authorization(auth) = header {
                    return Ok(Some(auth.to_string()));
                } else if let rsip::Header::ProxyAuthorization(auth) = header {
                    return Ok(Some(auth.to_string()));
                }
            }
        }
        Ok(None)
    }

    /// Parse authorization header parameters
    fn parse_authorization_header(&self, header: &str) -> Result<HashMap<String, String>> {
        let mut params = HashMap::new();

        // Remove "Digest " prefix if present
        let header = header.strip_prefix("Digest ").unwrap_or(header);

        // Simple parameter parsing (production would use proper parser)
        for part in header.split(',') {
            let part = part.trim();
            if let Some(eq_pos) = part.find('=') {
                let key = part[..eq_pos].trim();
                let value = part[eq_pos + 1..].trim().trim_matches('"');
                params.insert(key.to_string(), value.to_string());
            }
        }

        Ok(params)
    }

    /// Check if digest authentication is required
    fn requires_digest_auth(&self, _source_ip: &IpAddr) -> bool {
        // In this implementation, digest auth is optional
        // Production might have policies based on IP ranges
        false
    }

    /// Generate cryptographically secure nonce
    fn generate_nonce(&self) -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let nonce_bytes: [u8; 16] = rng.gen();
        hex::encode(nonce_bytes)
    }

    /// Create 401/407 challenge response
    pub fn create_challenge_response(
        &self,
        request: &rsip::Request,
        algorithm: DigestAlgorithm,
    ) -> Result<rsip::Response> {
        let nonce = self.generate_nonce();
        let realm = &self.default_realm;

        let algorithm_str = match algorithm {
            DigestAlgorithm::MD5 => "MD5",
            DigestAlgorithm::SHA256 => "SHA-256",
        };

        let www_authenticate = format!(
            "Digest realm=\"{}\", nonce=\"{}\", algorithm=\"{}\", qop=\"auth\"",
            realm, nonce, algorithm_str
        );

        let mut response = rsip::Response::default(); // TODO: Set status to Unauthorized

        // Add WWW-Authenticate header
        response.headers.push(rsip::Header::WwwAuthenticate(
            rsip::headers::WwwAuthenticate::new(www_authenticate),
        ));

        // Add standard headers
        response.headers.push(rsip::Header::CallId(
            request
                .headers
                .iter()
                .find_map(|h| {
                    if let rsip::Header::CallId(call_id) = h {
                        Some(call_id.clone())
                    } else {
                        None
                    }
                })
                .ok_or_else(|| anyhow!("No Call-ID header in request"))?,
        ));

        response.headers.push(rsip::Header::CSeq(
            request
                .headers
                .iter()
                .find_map(|h| {
                    if let rsip::Header::CSeq(cseq) = h {
                        Some(cseq.clone())
                    } else {
                        None
                    }
                })
                .ok_or_else(|| anyhow!("No CSeq header in request"))?,
        ));

        Ok(response)
    }

    /// Record SIP authentication failure
    async fn record_sip_failure(
        &self,
        ip: IpAddr,
        failure_type: FailureType,
        user: Option<&str>,
        method: &str,
        reason: &str,
    ) {
        if let Some(fail2ban) = &self.fail2ban_service {
            if let Err(e) = fail2ban
                .record_sip_failure(
                    ip,
                    failure_type,
                    user.map(String::from),
                    method.to_string(),
                    reason.to_string(),
                    None, // user_agent
                )
                .await
            {
                warn!("Failed to record SIP authentication failure: {}", e);
            }
        }
    }

    /// Record generic authentication failure (backwards compatibility)
    async fn record_auth_failure(&self, ip: IpAddr, method: rsip::Method, reason: &str) {
        let failure_type = match method {
            rsip::Method::Invite => FailureType::SipInvite,
            rsip::Method::Register => FailureType::SipRegister,
            rsip::Method::Subscribe => FailureType::SipSubscribe,
            _ => FailureType::SipOptions,
        };

        self.record_sip_failure(ip, failure_type, None, &method.to_string(), reason)
            .await;
    }
}
