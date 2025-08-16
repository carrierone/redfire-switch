/*
 * Redfire Switch - A Class 4 SIP Telephone Switch
 * Copyright (C) 2025 Carrier One Inc and contributors
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc, Duration};
use dashmap::DashMap;
use parking_lot::RwLock;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, warn, error, debug};

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable security features
    pub enabled: bool,
    /// Loop detection settings
    pub loop_detection: LoopDetectionConfig,
    /// SIP spam detection settings
    pub spam_detection: SpamDetectionConfig,
    /// Rate limiting settings
    pub rate_limiting: RateLimitConfig,
    /// IP blocking settings
    pub ip_blocking: IpBlockingConfig,
    /// Geographic restrictions
    pub geo_restrictions: GeoRestrictionConfig,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            loop_detection: LoopDetectionConfig::default(),
            spam_detection: SpamDetectionConfig::default(),
            rate_limiting: RateLimitConfig::default(),
            ip_blocking: IpBlockingConfig::default(),
            geo_restrictions: GeoRestrictionConfig::default(),
        }
    }
}

/// Loop detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopDetectionConfig {
    /// Enable loop detection
    pub enabled: bool,
    /// Maximum number of hops before declaring a loop
    pub max_hops: u32,
    /// Time window to track call paths (seconds)
    pub tracking_window: u64,
    /// Maximum calls between same endpoints in window
    pub max_calls_between_endpoints: u32,
    /// Track call paths using Via headers
    pub track_via_headers: bool,
    /// Track call paths using custom headers
    pub track_custom_headers: Vec<String>,
}

impl Default for LoopDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_hops: 70, // ITU-T recommendation
            tracking_window: 300, // 5 minutes
            max_calls_between_endpoints: 10,
            track_via_headers: true,
            track_custom_headers: vec![
                "X-Redfire-Call-Path".to_string(),
                "X-Originating-Switch".to_string(),
            ],
        }
    }
}

/// SIP spam detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpamDetectionConfig {
    /// Enable spam detection
    pub enabled: bool,
    /// Enable SIPVicious detection
    pub sipvicious_detection: bool,
    /// Suspicious user agent patterns
    pub suspicious_user_agents: Vec<String>,
    /// Suspicious method patterns
    pub suspicious_methods: Vec<String>,
    /// Maximum requests per second from single IP
    pub max_requests_per_second: u32,
    /// Maximum registration attempts per minute
    pub max_registration_attempts: u32,
    /// Maximum failed authentication attempts
    pub max_auth_failures: u32,
    /// Honeypot extensions (fake extensions to detect scanners)
    pub honeypot_extensions: Vec<String>,
    /// Enable CAPTCHA challenge for suspicious traffic
    pub enable_captcha: bool,
    /// Minimum call duration to be considered legitimate (seconds)
    pub min_call_duration: u32,
}

impl Default for SpamDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sipvicious_detection: true,
            suspicious_user_agents: vec![
                "sipvicious".to_string(),
                "sipcli".to_string(),
                "VaxSIPUserAgent".to_string(),
                "pplsip".to_string(),
                "sundayddr".to_string(),
                "iWar".to_string(),
                "sip-scan".to_string(),
                "sipsak".to_string(),
                "SIPp".to_string(),
                "sipflanker".to_string(),
                "Asterisk PBX".to_string(), // Often used by attackers
                "friendly-scanner".to_string(),
                "sipdump".to_string(),
                "smap".to_string(),
            ],
            suspicious_methods: vec![
                "SCAN".to_string(),
                "NOTIFY".to_string(), // When from unknown sources
                "REFER".to_string(),  // Often abused
            ],
            max_requests_per_second: 10,
            max_registration_attempts: 5,
            max_auth_failures: 3,
            honeypot_extensions: vec![
                "100".to_string(),
                "101".to_string(),
                "102".to_string(),
                "admin".to_string(),
                "root".to_string(),
                "test".to_string(),
                "1000".to_string(),
                "2000".to_string(),
                "3000".to_string(),
                "4000".to_string(),
                "5000".to_string(),
            ],
            enable_captcha: false,
            min_call_duration: 3,
        }
    }
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Enable rate limiting
    pub enabled: bool,
    /// Calls per second per IP
    pub calls_per_second_per_ip: u32,
    /// Messages per second per IP
    pub messages_per_second_per_ip: u32,
    /// Registration attempts per minute per IP
    pub registrations_per_minute_per_ip: u32,
    /// Maximum concurrent calls per IP
    pub max_concurrent_calls_per_ip: u32,
    /// Rate limit window (seconds)
    pub window_size: u64,
    /// Burst allowance multiplier
    pub burst_multiplier: f64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            calls_per_second_per_ip: 5,
            messages_per_second_per_ip: 10,
            registrations_per_minute_per_ip: 3,
            max_concurrent_calls_per_ip: 10,
            window_size: 60,
            burst_multiplier: 2.0,
        }
    }
}

/// IP blocking configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpBlockingConfig {
    /// Enable IP blocking
    pub enabled: bool,
    /// Automatic blocking based on behavior
    pub auto_block: bool,
    /// Manual blocklist
    pub blocked_ips: Vec<String>,
    /// Manual allowlist (whitelist)
    pub allowed_ips: Vec<String>,
    /// Blocked IP ranges (CIDR notation)
    pub blocked_ranges: Vec<String>,
    /// Block duration for automatic blocks (seconds)
    pub auto_block_duration: u64,
    /// Maximum block duration (seconds)
    pub max_block_duration: u64,
    /// Progressive blocking (increase duration for repeat offenders)
    pub progressive_blocking: bool,
}

impl Default for IpBlockingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_block: true,
            blocked_ips: vec![
                // Common scanner IPs - these would be updated regularly
                "0.0.0.0".to_string(), // Placeholder
            ],
            allowed_ips: vec![
                "127.0.0.1".to_string(),
                "::1".to_string(),
            ],
            blocked_ranges: vec![
                // Common suspicious ranges
                "192.168.0.0/16".to_string(), // Private ranges shouldn't reach us
                "10.0.0.0/8".to_string(),
                "172.16.0.0/12".to_string(),
            ],
            auto_block_duration: 3600, // 1 hour
            max_block_duration: 86400, // 24 hours
            progressive_blocking: true,
        }
    }
}

/// Geographic restriction configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoRestrictionConfig {
    /// Enable geographic restrictions
    pub enabled: bool,
    /// Allowed country codes (ISO 3166-1 alpha-2)
    pub allowed_countries: Vec<String>,
    /// Blocked country codes
    pub blocked_countries: Vec<String>,
    /// Enable ASN-based blocking
    pub asn_blocking: bool,
    /// Blocked ASNs
    pub blocked_asns: Vec<u32>,
}

impl Default for GeoRestrictionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            allowed_countries: vec!["US".to_string(), "CA".to_string()],
            blocked_countries: vec![
                // Countries commonly used for toll fraud
                "CU".to_string(), // Cuba
                "IR".to_string(), // Iran
                "KP".to_string(), // North Korea
                "MM".to_string(), // Myanmar
                "SO".to_string(), // Somalia
            ],
            asn_blocking: false,
            blocked_asns: Vec::new(),
        }
    }
}

/// Call path tracking for loop detection
#[derive(Debug, Clone)]
pub struct CallPath {
    pub call_id: String,
    pub hops: Vec<String>,
    pub timestamps: VecDeque<DateTime<Utc>>,
    pub last_seen: DateTime<Utc>,
}

/// IP reputation and behavior tracking
#[derive(Debug, Clone)]
pub struct IpReputation {
    pub ip: IpAddr,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub total_requests: u64,
    pub failed_auths: u32,
    pub registration_attempts: u32,
    pub suspicious_patterns: u32,
    pub blocked_until: Option<DateTime<Utc>>,
    pub block_count: u32,
    pub honeypot_hits: u32,
    pub user_agents: Vec<String>,
    pub methods_used: HashMap<String, u32>,
}

/// Request rate tracking
#[derive(Debug)]
pub struct RateTracker {
    pub requests: VecDeque<Instant>,
    pub window_start: Instant,
}

impl RateTracker {
    pub fn new() -> Self {
        Self {
            requests: VecDeque::new(),
            window_start: Instant::now(),
        }
    }

    pub fn is_rate_limited(&mut self, limit: u32, window_secs: u64) -> bool {
        let now = Instant::now();
        let window_duration = std::time::Duration::from_secs(window_secs);

        // Remove old requests outside the window
        while let Some(&front) = self.requests.front() {
            if now.duration_since(front) > window_duration {
                self.requests.pop_front();
            } else {
                break;
            }
        }

        // Check if we're over the limit
        if self.requests.len() >= limit as usize {
            return true;
        }

        // Add current request
        self.requests.push_back(now);
        false
    }
}

/// Security decision
#[derive(Debug, Clone, PartialEq)]
pub enum SecurityDecision {
    /// Allow the request/call
    Allow,
    /// Block the request/call
    Block(BlockReason),
    /// Challenge with additional verification
    Challenge(ChallengeType),
}

/// Block reason
#[derive(Debug, Clone, PartialEq)]
pub enum BlockReason {
    /// Call loop detected
    CallLoop,
    /// Spam/fraud detected
    SpamDetected,
    /// Rate limit exceeded
    RateLimited,
    /// IP is blocked
    IpBlocked,
    /// Geographic restriction
    GeoBlocked,
    /// Honeypot triggered
    HoneypotTriggered,
    /// SIPVicious scanner detected
    SipViciousDetected,
    /// Too many authentication failures
    AuthFailures,
    /// Suspicious user agent
    SuspiciousUserAgent,
}

/// Challenge type
#[derive(Debug, Clone, PartialEq)]
pub enum ChallengeType {
    /// CAPTCHA verification
    Captcha,
    /// Additional authentication
    ExtraAuth,
    /// Temporary delay
    DelayResponse,
}

/// Main security service
pub struct SecurityService {
    config: SecurityConfig,
    /// Call path tracking for loop detection
    call_paths: Arc<DashMap<String, CallPath>>,
    /// IP reputation tracking
    ip_reputations: Arc<DashMap<IpAddr, IpReputation>>,
    /// Rate limiting trackers
    rate_trackers: Arc<DashMap<IpAddr, RateTracker>>,
    /// Compiled regex patterns for performance
    user_agent_patterns: Vec<Regex>,
    /// Statistics
    stats: Arc<RwLock<SecurityStats>>,
}

/// Security statistics
#[derive(Debug, Clone, Default)]
pub struct SecurityStats {
    pub total_requests: u64,
    pub blocked_requests: u64,
    pub loop_detections: u64,
    pub spam_detections: u64,
    pub rate_limit_blocks: u64,
    pub ip_blocks: u64,
    pub geo_blocks: u64,
    pub honeypot_triggers: u64,
    pub sipvicious_detections: u64,
    pub last_updated: Option<DateTime<Utc>>,
}

impl SecurityService {
    /// Create a new security service
    pub fn new(config: SecurityConfig) -> Result<Self> {
        // Compile regex patterns for user agents
        let mut user_agent_patterns = Vec::new();
        for pattern in &config.spam_detection.suspicious_user_agents {
            match Regex::new(&format!("(?i){}", regex::escape(pattern))) {
                Ok(regex) => user_agent_patterns.push(regex),
                Err(e) => warn!("Invalid user agent pattern '{}': {}", pattern, e),
            }
        }

        let service = Self {
            config,
            call_paths: Arc::new(DashMap::new()),
            ip_reputations: Arc::new(DashMap::new()),
            rate_trackers: Arc::new(DashMap::new()),
            user_agent_patterns,
            stats: Arc::new(RwLock::new(SecurityStats::default())),
        };

        // Start cleanup task
        let cleanup_service = service.clone();
        tokio::spawn(async move {
            cleanup_service.run_cleanup_task().await;
        });

        info!("Security service initialized");
        Ok(service)
    }

    /// Check if a SIP request should be allowed
    pub fn check_sip_request(
        &self,
        src_ip: IpAddr,
        method: &str,
        user_agent: Option<&str>,
        to_user: Option<&str>,
        call_id: Option<&str>,
    ) -> SecurityDecision {
        if !self.config.enabled {
            return SecurityDecision::Allow;
        }

        // Update statistics
        {
            let mut stats = self.stats.write();
            stats.total_requests += 1;
            stats.last_updated = Some(Utc::now());
        }

        // Check IP blocking first
        if let Some(decision) = self.check_ip_blocking(src_ip) {
            if decision != SecurityDecision::Allow {
                self.record_block(&decision);
                return decision;
            }
        }

        // Check rate limiting
        if let Some(decision) = self.check_rate_limiting(src_ip, method) {
            if decision != SecurityDecision::Allow {
                self.record_block(&decision);
                return decision;
            }
        }

        // Check spam detection
        if let Some(decision) = self.check_spam_detection(src_ip, method, user_agent, to_user) {
            if decision != SecurityDecision::Allow {
                self.record_block(&decision);
                return decision;
            }
        }

        // Check call loop detection
        if let Some(call_id) = call_id {
            if let Some(decision) = self.check_call_loop(call_id, src_ip) {
                if decision != SecurityDecision::Allow {
                    self.record_block(&decision);
                    return decision;
                }
            }
        }

        SecurityDecision::Allow
    }

    /// Check IP blocking
    fn check_ip_blocking(&self, src_ip: IpAddr) -> Option<SecurityDecision> {
        if !self.config.ip_blocking.enabled {
            return None;
        }

        // Check allowlist first
        if self.config.ip_blocking.allowed_ips.contains(&src_ip.to_string()) {
            return Some(SecurityDecision::Allow);
        }

        // Check manual blocklist
        if self.config.ip_blocking.blocked_ips.contains(&src_ip.to_string()) {
            return Some(SecurityDecision::Block(BlockReason::IpBlocked));
        }

        // Check if IP is temporarily blocked
        if let Some(reputation) = self.ip_reputations.get(&src_ip) {
            if let Some(blocked_until) = reputation.blocked_until {
                if Utc::now() < blocked_until {
                    return Some(SecurityDecision::Block(BlockReason::IpBlocked));
                }
            }
        }

        None
    }

    /// Check rate limiting
    fn check_rate_limiting(&self, src_ip: IpAddr, method: &str) -> Option<SecurityDecision> {
        if !self.config.rate_limiting.enabled {
            return None;
        }

        let mut tracker = self.rate_trackers.entry(src_ip).or_insert_with(RateTracker::new);

        let limit = match method {
            "INVITE" => self.config.rate_limiting.calls_per_second_per_ip,
            "MESSAGE" => self.config.rate_limiting.messages_per_second_per_ip,
            "REGISTER" => self.config.rate_limiting.registrations_per_minute_per_ip / 60, // Convert to per-second
            _ => self.config.rate_limiting.calls_per_second_per_ip,
        };

        if tracker.is_rate_limited(limit, self.config.rate_limiting.window_size) {
            return Some(SecurityDecision::Block(BlockReason::RateLimited));
        }

        None
    }

    /// Check spam detection
    fn check_spam_detection(
        &self,
        src_ip: IpAddr,
        method: &str,
        user_agent: Option<&str>,
        to_user: Option<&str>,
    ) -> Option<SecurityDecision> {
        if !self.config.spam_detection.enabled {
            return None;
        }

        // Check user agent patterns
        if let Some(ua) = user_agent {
            for pattern in &self.user_agent_patterns {
                if pattern.is_match(ua) {
                    self.mark_suspicious_ip(src_ip, "suspicious_user_agent");
                    return Some(SecurityDecision::Block(BlockReason::SuspiciousUserAgent));
                }
            }
        }

        // Check suspicious methods
        if self.config.spam_detection.suspicious_methods.contains(&method.to_uppercase()) {
            self.mark_suspicious_ip(src_ip, "suspicious_method");
            return Some(SecurityDecision::Block(BlockReason::SpamDetected));
        }

        // Check honeypot extensions
        if let Some(to) = to_user {
            if self.config.spam_detection.honeypot_extensions.iter().any(|ext| to.contains(ext)) {
                self.mark_suspicious_ip(src_ip, "honeypot_hit");
                return Some(SecurityDecision::Block(BlockReason::HoneypotTriggered));
            }
        }

        None
    }

    /// Check for call loops
    fn check_call_loop(&self, call_id: &str, src_ip: IpAddr) -> Option<SecurityDecision> {
        if !self.config.loop_detection.enabled {
            return None;
        }

        let now = Utc::now();
        let src_ip_str = src_ip.to_string();

        // Get or create call path
        let mut call_path = self.call_paths.entry(call_id.to_string())
            .or_insert_with(|| CallPath {
                call_id: call_id.to_string(),
                hops: Vec::new(),
                timestamps: VecDeque::new(),
                last_seen: now,
            });

        // Update last seen
        call_path.last_seen = now;
        call_path.timestamps.push_back(now);

        // Check if we've seen this IP in the call path
        if call_path.hops.contains(&src_ip_str) {
            return Some(SecurityDecision::Block(BlockReason::CallLoop));
        }

        // Add current hop
        call_path.hops.push(src_ip_str);

        // Check hop count
        if call_path.hops.len() > self.config.loop_detection.max_hops as usize {
            return Some(SecurityDecision::Block(BlockReason::CallLoop));
        }

        // Check rapid call attempts between same endpoints
        let tracking_window = Duration::seconds(self.config.loop_detection.tracking_window as i64);
        let cutoff_time = now - tracking_window;

        // Remove old timestamps
        while let Some(&front) = call_path.timestamps.front() {
            if front < cutoff_time {
                call_path.timestamps.pop_front();
            } else {
                break;
            }
        }

        // Check if too many calls in window
        if call_path.timestamps.len() > self.config.loop_detection.max_calls_between_endpoints as usize {
            return Some(SecurityDecision::Block(BlockReason::CallLoop));
        }

        None
    }

    /// Mark an IP as suspicious
    fn mark_suspicious_ip(&self, src_ip: IpAddr, reason: &str) {
        let now = Utc::now();

        let mut reputation = self.ip_reputations.entry(src_ip)
            .or_insert_with(|| IpReputation {
                ip: src_ip,
                first_seen: now,
                last_seen: now,
                total_requests: 0,
                failed_auths: 0,
                registration_attempts: 0,
                suspicious_patterns: 0,
                blocked_until: None,
                block_count: 0,
                honeypot_hits: 0,
                user_agents: Vec::new(),
                methods_used: HashMap::new(),
            });

        reputation.last_seen = now;
        reputation.suspicious_patterns += 1;

        match reason {
            "honeypot_hit" => reputation.honeypot_hits += 1,
            "auth_failure" => reputation.failed_auths += 1,
            _ => {}
        }

        // Auto-block if configured and threshold exceeded
        if self.config.ip_blocking.auto_block && 
           reputation.suspicious_patterns >= 3 {
            
            let block_duration = if self.config.ip_blocking.progressive_blocking {
                std::cmp::min(
                    self.config.ip_blocking.auto_block_duration * (reputation.block_count + 1) as u64,
                    self.config.ip_blocking.max_block_duration
                )
            } else {
                self.config.ip_blocking.auto_block_duration
            };

            reputation.blocked_until = Some(now + Duration::seconds(block_duration as i64));
            reputation.block_count += 1;

            warn!("Auto-blocked IP {} for {} seconds (reason: {}, block count: {})", 
                  src_ip, block_duration, reason, reputation.block_count);
        }
    }

    /// Record a security block in statistics
    fn record_block(&self, decision: &SecurityDecision) {
        let mut stats = self.stats.write();
        stats.blocked_requests += 1;

        if let SecurityDecision::Block(reason) = decision {
            match reason {
                BlockReason::CallLoop => stats.loop_detections += 1,
                BlockReason::SpamDetected => stats.spam_detections += 1,
                BlockReason::RateLimited => stats.rate_limit_blocks += 1,
                BlockReason::IpBlocked => stats.ip_blocks += 1,
                BlockReason::GeoBlocked => stats.geo_blocks += 1,
                BlockReason::HoneypotTriggered => stats.honeypot_triggers += 1,
                BlockReason::SipViciousDetected => stats.sipvicious_detections += 1,
                _ => {}
            }
        }
    }

    /// Get security statistics
    pub fn get_stats(&self) -> SecurityStats {
        self.stats.read().clone()
    }

    /// Manually block an IP
    pub fn block_ip(&self, ip: IpAddr, duration_seconds: u64) -> Result<()> {
        let now = Utc::now();
        let blocked_until = now + Duration::seconds(duration_seconds as i64);

        let mut reputation = self.ip_reputations.entry(ip)
            .or_insert_with(|| IpReputation {
                ip,
                first_seen: now,
                last_seen: now,
                total_requests: 0,
                failed_auths: 0,
                registration_attempts: 0,
                suspicious_patterns: 0,
                blocked_until: None,
                block_count: 0,
                honeypot_hits: 0,
                user_agents: Vec::new(),
                methods_used: HashMap::new(),
            });

        reputation.blocked_until = Some(blocked_until);
        reputation.block_count += 1;

        info!("Manually blocked IP {} for {} seconds", ip, duration_seconds);
        Ok(())
    }

    /// Unblock an IP
    pub fn unblock_ip(&self, ip: IpAddr) -> Result<()> {
        if let Some(mut reputation) = self.ip_reputations.get_mut(&ip) {
            reputation.blocked_until = None;
            info!("Unblocked IP {}", ip);
        }
        Ok(())
    }

    /// Cleanup old tracking data
    async fn run_cleanup_task(&self) {
        let mut cleanup_timer = tokio::time::interval(std::time::Duration::from_secs(300)); // 5 minutes

        loop {
            cleanup_timer.tick().await;
            self.cleanup_old_data().await;
        }
    }

    /// Remove old tracking data
    async fn cleanup_old_data(&self) {
        let now = Utc::now();
        let cleanup_cutoff = now - Duration::hours(24); // Remove data older than 24 hours

        // Cleanup call paths
        self.call_paths.retain(|_, path| path.last_seen > cleanup_cutoff);

        // Cleanup unblocked IP reputations
        self.ip_reputations.retain(|_, reputation| {
            if let Some(blocked_until) = reputation.blocked_until {
                blocked_until > now || reputation.last_seen > cleanup_cutoff
            } else {
                reputation.last_seen > cleanup_cutoff
            }
        });

        debug!("Cleaned up old security tracking data");
    }
}

impl Clone for SecurityService {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            call_paths: self.call_paths.clone(),
            ip_reputations: self.ip_reputations.clone(),
            rate_trackers: self.rate_trackers.clone(),
            user_agent_patterns: self.user_agent_patterns.clone(),
            stats: self.stats.clone(),
        }
    }
}

/// Security utilities
pub mod utils {
    use super::*;

    /// Check if an IP is in a CIDR range
    pub fn ip_in_cidr(ip: IpAddr, cidr: &str) -> Result<bool> {
        let parts: Vec<&str> = cidr.split('/').collect();
        if parts.len() != 2 {
            return Err(anyhow!("Invalid CIDR format"));
        }

        let network: IpAddr = parts[0].parse()?;
        let prefix_len: u8 = parts[1].parse()?;

        match (ip, network) {
            (IpAddr::V4(ip), IpAddr::V4(net)) => {
                if prefix_len > 32 {
                    return Err(anyhow!("Invalid IPv4 prefix length: {}", prefix_len));
                }
                let ip_int = u32::from(ip);
                let net_int = u32::from(net);
                let mask = if prefix_len == 0 {
                    0
                } else {
                    !((1u32 << (32 - prefix_len)) - 1)
                };
                Ok((ip_int & mask) == (net_int & mask))
            }
            (IpAddr::V6(ip), IpAddr::V6(net)) => {
                if prefix_len > 128 {
                    return Err(anyhow!("Invalid IPv6 prefix length: {}", prefix_len));
                }
                let ip_int = u128::from(ip);
                let net_int = u128::from(net);
                let mask = if prefix_len == 0 {
                    0
                } else {
                    !((1u128 << (128 - prefix_len)) - 1)
                };
                Ok((ip_int & mask) == (net_int & mask))
            }
            _ => Ok(false), // Different IP versions
        }
    }

    /// Generate security report
    pub fn generate_security_report(service: &SecurityService) -> String {
        let stats = service.get_stats();
        
        format!(
            "Security Report:\n\
             Total Requests: {}\n\
             Blocked Requests: {} ({:.2}%)\n\
             Loop Detections: {}\n\
             Spam Detections: {}\n\
             Rate Limit Blocks: {}\n\
             IP Blocks: {}\n\
             Geo Blocks: {}\n\
             Honeypot Triggers: {}\n\
             SIPVicious Detections: {}\n\
             Active IP Tracking: {} IPs\n\
             Active Call Paths: {} calls",
            stats.total_requests,
            stats.blocked_requests,
            if stats.total_requests > 0 { 
                (stats.blocked_requests as f64 / stats.total_requests as f64) * 100.0 
            } else { 0.0 },
            stats.loop_detections,
            stats.spam_detections,
            stats.rate_limit_blocks,
            stats.ip_blocks,
            stats.geo_blocks,
            stats.honeypot_triggers,
            stats.sipvicious_detections,
            service.ip_reputations.len(),
            service.call_paths.len()
        )
    }

    /// Extract call path from SIP Via headers
    pub fn extract_call_path_from_via(via_headers: &[String]) -> Vec<String> {
        via_headers.iter()
            .filter_map(|via| {
                // Parse Via header to extract host/IP
                // Format: "SIP/2.0/UDP host:port;branch=..."
                let parts: Vec<&str> = via.split_whitespace().collect();
                if parts.len() >= 2 {
                    let host_port = parts[1].split(';').next()?;
                    let host = host_port.split(':').next()?;
                    Some(host.to_string())
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_ip_in_cidr() {
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));
        assert!(utils::ip_in_cidr(ip, "192.168.1.0/24").unwrap());
        assert!(!utils::ip_in_cidr(ip, "192.168.2.0/24").unwrap());
    }

    #[test]
    fn test_rate_tracker() {
        let mut tracker = RateTracker::new();
        
        // Should not be rate limited initially
        assert!(!tracker.is_rate_limited(10, 60));
        
        // Add requests up to limit
        for _ in 0..9 {
            assert!(!tracker.is_rate_limited(10, 60));
        }
        
        // Should be rate limited after reaching limit
        assert!(tracker.is_rate_limited(10, 60));
    }

    #[tokio::test]
    async fn test_security_service() {
        let config = SecurityConfig::default();
        let service = SecurityService::new(config).unwrap();
        
        let src_ip = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));
        
        // Should allow normal request
        let decision = service.check_sip_request(
            src_ip,
            "INVITE",
            Some("Asterisk PBX 16.0.0"),
            Some("1001"),
            Some("call-123"),
        );
        
        // Should block due to suspicious user agent
        assert_eq!(decision, SecurityDecision::Block(BlockReason::SuspiciousUserAgent));
    }

    #[test]
    fn test_via_header_parsing() {
        let via_headers = vec![
            "SIP/2.0/UDP 192.168.1.1:5060;branch=z9hG4bK123".to_string(),
            "SIP/2.0/TCP 10.0.0.1:5060;branch=z9hG4bK456".to_string(),
        ];
        
        let path = utils::extract_call_path_from_via(&via_headers);
        assert_eq!(path, vec!["192.168.1.1", "10.0.0.1"]);
    }
}