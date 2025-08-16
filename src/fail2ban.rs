/*
 * Redfire Switch - Fail2Ban Integration for Authentication Failures
 * Copyright (C) 2025 Carrier One Inc and contributors
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

//! # Fail2Ban Integration
//! 
//! This module provides fail2ban integration for Redfire Switch to automatically
//! ban IP addresses that exhibit patterns of authentication failures for SIP and SMS.

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc, Duration as ChronoDuration};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::Write;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, Instant};
use tracing::{debug, info, warn, error, instrument};

/// Fail2Ban integration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fail2BanConfig {
    /// Enable fail2ban integration
    pub enabled: bool,
    /// Log file path for fail2ban to monitor
    pub log_file: PathBuf,
    /// SIP authentication failure configuration
    pub sip: SipFailureConfig,
    /// SMS authentication failure configuration
    pub sms: SmsFailureConfig,
    /// IP whitelist (never ban these IPs)
    pub whitelist: Vec<String>,
    /// Custom fail2ban action to execute
    pub custom_action: Option<String>,
    /// Enable automatic cleanup of old failure records
    pub auto_cleanup: bool,
    /// Cleanup interval in seconds
    pub cleanup_interval_seconds: u64,
}

impl Default for Fail2BanConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            log_file: PathBuf::from("/var/log/redfire-switch/auth-failures.log"),
            sip: SipFailureConfig::default(),
            sms: SmsFailureConfig::default(),
            whitelist: vec![
                "127.0.0.1".to_string(),
                "::1".to_string(),
                "10.0.0.0/8".to_string(),
                "172.16.0.0/12".to_string(),
                "192.168.0.0/16".to_string(),
            ],
            custom_action: None,
            auto_cleanup: true,
            cleanup_interval_seconds: 3600, // 1 hour
        }
    }
}

/// SIP authentication failure configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipFailureConfig {
    /// Maximum authentication failures before logging
    pub max_failures: u32,
    /// Time window for failure counting (seconds)
    pub time_window_seconds: u64,
    /// Ban duration in seconds
    pub ban_duration_seconds: u64,
    /// Monitor INVITE failures
    pub monitor_invite_failures: bool,
    /// Monitor REGISTER failures
    pub monitor_register_failures: bool,
    /// Monitor SUBSCRIBE failures
    pub monitor_subscribe_failures: bool,
    /// Pattern for fail2ban log entries
    pub log_pattern: String,
}

impl Default for SipFailureConfig {
    fn default() -> Self {
        Self {
            max_failures: 5,
            time_window_seconds: 300, // 5 minutes
            ban_duration_seconds: 3600, // 1 hour
            monitor_invite_failures: true,
            monitor_register_failures: true,
            monitor_subscribe_failures: true,
            log_pattern: "SIP authentication failure from <HOST> - user: {user}, method: {method}, reason: {reason}".to_string(),
        }
    }
}

/// SMS authentication failure configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsFailureConfig {
    /// Maximum authentication failures before logging
    pub max_failures: u32,
    /// Time window for failure counting (seconds)
    pub time_window_seconds: u64,
    /// Ban duration in seconds
    pub ban_duration_seconds: u64,
    /// Monitor SMPP bind failures
    pub monitor_smpp_bind_failures: bool,
    /// Monitor HTTP API authentication failures
    pub monitor_http_failures: bool,
    /// Pattern for fail2ban log entries
    pub log_pattern: String,
}

impl Default for SmsFailureConfig {
    fn default() -> Self {
        Self {
            max_failures: 10,
            time_window_seconds: 300, // 5 minutes
            ban_duration_seconds: 1800, // 30 minutes
            monitor_smpp_bind_failures: true,
            monitor_http_failures: true,
            log_pattern: "SMS authentication failure from <HOST> - system_id: {system_id}, reason: {reason}".to_string(),
        }
    }
}

/// Authentication failure types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureType {
    SipInvite,
    SipRegister,
    SipSubscribe,
    SipOptions,
    SmppBind,
    SmsHttp,
}

/// Authentication failure record
#[derive(Debug, Clone)]
pub struct AuthFailure {
    /// IP address of the failure
    pub ip: IpAddr,
    /// Type of authentication failure
    pub failure_type: FailureType,
    /// Username/system_id attempted
    pub user: Option<String>,
    /// SIP method or SMS protocol
    pub method: String,
    /// Reason for failure
    pub reason: String,
    /// User agent or client identifier
    pub user_agent: Option<String>,
    /// Timestamp of failure
    pub timestamp: DateTime<Utc>,
    /// Additional metadata
    pub metadata: std::collections::HashMap<String, String>,
}

/// IP failure tracking information
#[derive(Debug, Clone)]
pub struct IpFailureTracker {
    /// Recent failures within the time window
    pub failures: VecDeque<AuthFailure>,
    /// First failure timestamp in current window
    pub first_failure: DateTime<Utc>,
    /// Last failure timestamp
    pub last_failure: DateTime<Utc>,
    /// Total failure count
    pub total_failures: u32,
    /// Whether IP is currently banned
    pub is_banned: bool,
    /// Ban expiry time
    pub ban_expires: Option<DateTime<Utc>>,
}

impl Default for IpFailureTracker {
    fn default() -> Self {
        Self {
            failures: VecDeque::new(),
            first_failure: Utc::now(),
            last_failure: Utc::now(),
            total_failures: 0,
            is_banned: false,
            ban_expires: None,
        }
    }
}

/// Fail2Ban integration service
pub struct Fail2BanService {
    /// Configuration
    config: Fail2BanConfig,
    /// IP failure tracking
    failure_tracking: Arc<DashMap<IpAddr, IpFailureTracker>>,
    /// Log file handle
    log_file: Arc<tokio::sync::Mutex<std::fs::File>>,
}

impl Fail2BanService {
    /// Create new fail2ban service
    pub async fn new(config: Fail2BanConfig) -> Result<Self> {
        // Create log directory if it doesn't exist
        if let Some(parent) = config.log_file.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Open log file for writing
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&config.log_file)?;

        let service = Self {
            config,
            failure_tracking: Arc::new(DashMap::new()),
            log_file: Arc::new(tokio::sync::Mutex::new(log_file)),
        };

        info!("Fail2Ban service initialized, logging to: {:?}", service.config.log_file);
        Ok(service)
    }

    /// Start the fail2ban service background tasks
    pub async fn start(&self) -> Result<()> {
        if !self.config.enabled {
            info!("Fail2Ban integration is disabled");
            return Ok(());
        }

        info!("Starting Fail2Ban service");

        // Start cleanup task if enabled
        if self.config.auto_cleanup {
            self.start_cleanup_task().await;
        }

        // Write startup message to log
        self.write_log_entry("Redfire Switch Fail2Ban integration started").await?;

        Ok(())
    }

    /// Record a SIP authentication failure
    #[instrument(skip(self))]
    pub async fn record_sip_failure(
        &self,
        ip: IpAddr,
        failure_type: FailureType,
        user: Option<String>,
        method: String,
        reason: String,
        user_agent: Option<String>,
    ) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        // Check if IP is whitelisted
        if self.is_whitelisted(ip).await {
            debug!("Ignoring failure from whitelisted IP: {}", ip);
            return Ok(());
        }

        let failure = AuthFailure {
            ip,
            failure_type: failure_type.clone(),
            user: user.clone(),
            method: method.clone(),
            reason: reason.clone(),
            user_agent,
            timestamp: Utc::now(),
            metadata: std::collections::HashMap::new(),
        };

        // Update failure tracking
        self.update_failure_tracking(failure.clone()).await?;

        // Check if we need to log for fail2ban
        if self.should_log_sip_failure(&failure_type).await {
            let tracker = self.failure_tracking.get(&ip);
            if let Some(tracker) = tracker {
                if tracker.failures.len() >= self.config.sip.max_failures as usize {
                    // Format log entry for fail2ban
                    let log_entry = self.format_sip_log_entry(&failure).await;
                    self.write_log_entry(&log_entry).await?;

                    info!("Logged SIP authentication failures from {} for fail2ban processing", ip);
                }
            }
        }

        Ok(())
    }

    /// Record an SMS authentication failure
    #[instrument(skip(self))]
    pub async fn record_sms_failure(
        &self,
        ip: IpAddr,
        failure_type: FailureType,
        system_id: Option<String>,
        reason: String,
        protocol: String,
    ) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        // Check if IP is whitelisted
        if self.is_whitelisted(ip).await {
            debug!("Ignoring SMS failure from whitelisted IP: {}", ip);
            return Ok(());
        }

        let failure = AuthFailure {
            ip,
            failure_type: failure_type.clone(),
            user: system_id.clone(),
            method: protocol.clone(),
            reason: reason.clone(),
            user_agent: None,
            timestamp: Utc::now(),
            metadata: std::collections::HashMap::new(),
        };

        // Update failure tracking
        self.update_failure_tracking(failure.clone()).await?;

        // Check if we need to log for fail2ban
        if self.should_log_sms_failure(&failure_type).await {
            let tracker = self.failure_tracking.get(&ip);
            if let Some(tracker) = tracker {
                if tracker.failures.len() >= self.config.sms.max_failures as usize {
                    // Format log entry for fail2ban
                    let log_entry = self.format_sms_log_entry(&failure).await;
                    self.write_log_entry(&log_entry).await?;

                    info!("Logged SMS authentication failures from {} for fail2ban processing", ip);
                }
            }
        }

        Ok(())
    }

    /// Check if an IP address is currently banned
    pub async fn is_banned(&self, ip: IpAddr) -> bool {
        if let Some(tracker) = self.failure_tracking.get(&ip) {
            if tracker.is_banned {
                if let Some(ban_expires) = tracker.ban_expires {
                    if Utc::now() < ban_expires {
                        return true;
                    } else {
                        // Ban has expired, remove it
                        drop(tracker);
                        if let Some(mut tracker) = self.failure_tracking.get_mut(&ip) {
                            tracker.is_banned = false;
                            tracker.ban_expires = None;
                        }
                    }
                }
            }
        }
        false
    }

    /// Get failure statistics for an IP
    pub async fn get_failure_stats(&self, ip: IpAddr) -> Option<IpFailureTracker> {
        self.failure_tracking.get(&ip).map(|tracker| tracker.clone())
    }

    /// Get all tracked IPs with failure counts
    pub async fn get_all_failure_stats(&self) -> Vec<(IpAddr, IpFailureTracker)> {
        self.failure_tracking
            .iter()
            .map(|entry| (*entry.key(), entry.value().clone()))
            .collect()
    }

    /// Manually ban an IP address
    pub async fn manual_ban(&self, ip: IpAddr, duration_seconds: u64, reason: String) -> Result<()> {
        let ban_expires = Utc::now() + ChronoDuration::seconds(duration_seconds as i64);
        
        let mut tracker = self.failure_tracking.entry(ip).or_insert_with(IpFailureTracker::default);
        tracker.is_banned = true;
        tracker.ban_expires = Some(ban_expires);

        // Log manual ban
        let log_entry = format!("Manual ban applied to {} for {} seconds - reason: {}", 
            ip, duration_seconds, reason);
        self.write_log_entry(&log_entry).await?;

        info!("Manually banned IP {} for {} seconds: {}", ip, duration_seconds, reason);
        Ok(())
    }

    /// Manually unban an IP address
    pub async fn manual_unban(&self, ip: IpAddr, reason: String) -> Result<()> {
        if let Some(mut tracker) = self.failure_tracking.get_mut(&ip) {
            tracker.is_banned = false;
            tracker.ban_expires = None;

            // Log manual unban
            let log_entry = format!("Manual unban applied to {} - reason: {}", ip, reason);
            self.write_log_entry(&log_entry).await?;

            info!("Manually unbanned IP {}: {}", ip, reason);
        }
        Ok(())
    }

    /// Clear all failure records for an IP
    pub async fn clear_failures(&self, ip: IpAddr) -> Result<()> {
        if let Some((_, _)) = self.failure_tracking.remove(&ip) {
            info!("Cleared all failure records for IP: {}", ip);
        }
        Ok(())
    }

    /// Update failure tracking for an IP
    async fn update_failure_tracking(&self, failure: AuthFailure) -> Result<()> {
        let mut tracker = self.failure_tracking.entry(failure.ip).or_insert_with(IpFailureTracker::default);
        let now = failure.timestamp;

        // Determine time window based on failure type
        let time_window = match failure.failure_type {
            FailureType::SipInvite | FailureType::SipRegister | FailureType::SipSubscribe | FailureType::SipOptions => {
                ChronoDuration::seconds(self.config.sip.time_window_seconds as i64)
            }
            FailureType::SmppBind | FailureType::SmsHttp => {
                ChronoDuration::seconds(self.config.sms.time_window_seconds as i64)
            }
        };

        // Remove failures outside the time window
        let cutoff_time = now - time_window;
        while let Some(front) = tracker.failures.front() {
            if front.timestamp < cutoff_time {
                tracker.failures.pop_front();
            } else {
                break;
            }
        }

        // Add new failure
        tracker.failures.push_back(failure);
        tracker.last_failure = now;
        tracker.total_failures += 1;

        // Update first failure if this is the only failure in window
        if tracker.failures.len() == 1 {
            tracker.first_failure = now;
        }

        debug!("Updated failure tracking for {}: {} failures in window", 
            tracker.failures.back().unwrap().ip, tracker.failures.len());

        Ok(())
    }

    /// Check if IP is whitelisted
    async fn is_whitelisted(&self, ip: IpAddr) -> bool {
        for whitelist_entry in &self.config.whitelist {
            if let Ok(whitelist_ip) = whitelist_entry.parse::<IpAddr>() {
                if ip == whitelist_ip {
                    return true;
                }
            } else if whitelist_entry.contains('/') {
                // Handle CIDR notation
                if let Ok(network) = ipnetwork::IpNetwork::from_str(whitelist_entry) {
                    if network.contains(ip) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Check if we should log SIP failure type
    async fn should_log_sip_failure(&self, failure_type: &FailureType) -> bool {
        match failure_type {
            FailureType::SipInvite => self.config.sip.monitor_invite_failures,
            FailureType::SipRegister => self.config.sip.monitor_register_failures,
            FailureType::SipSubscribe => self.config.sip.monitor_subscribe_failures,
            _ => true,
        }
    }

    /// Check if we should log SMS failure type
    async fn should_log_sms_failure(&self, failure_type: &FailureType) -> bool {
        match failure_type {
            FailureType::SmppBind => self.config.sms.monitor_smpp_bind_failures,
            FailureType::SmsHttp => self.config.sms.monitor_http_failures,
            _ => true,
        }
    }

    /// Format SIP failure for fail2ban log
    async fn format_sip_log_entry(&self, failure: &AuthFailure) -> String {
        let pattern = &self.config.sip.log_pattern;
        let timestamp = failure.timestamp.format("%Y-%m-%d %H:%M:%S UTC");
        
        pattern
            .replace("<HOST>", &failure.ip.to_string())
            .replace("{user}", &failure.user.as_deref().unwrap_or("unknown"))
            .replace("{method}", &failure.method)
            .replace("{reason}", &failure.reason)
            .replace("{timestamp}", &timestamp.to_string())
    }

    /// Format SMS failure for fail2ban log
    async fn format_sms_log_entry(&self, failure: &AuthFailure) -> String {
        let pattern = &self.config.sms.log_pattern;
        let timestamp = failure.timestamp.format("%Y-%m-%d %H:%M:%S UTC");
        
        pattern
            .replace("<HOST>", &failure.ip.to_string())
            .replace("{system_id}", &failure.user.as_deref().unwrap_or("unknown"))
            .replace("{reason}", &failure.reason)
            .replace("{protocol}", &failure.method)
            .replace("{timestamp}", &timestamp.to_string())
    }

    /// Write entry to fail2ban log file
    async fn write_log_entry(&self, entry: &str) -> Result<()> {
        let mut log_file = self.log_file.lock().await;
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
        let full_entry = format!("[{}] {}\n", timestamp, entry);
        
        log_file.write_all(full_entry.as_bytes())?;
        log_file.flush()?;
        
        debug!("Wrote fail2ban log entry: {}", entry);
        Ok(())
    }

    /// Start cleanup background task
    async fn start_cleanup_task(&self) {
        let failure_tracking = self.failure_tracking.clone();
        let cleanup_interval = self.config.cleanup_interval_seconds;
        let sip_window = self.config.sip.time_window_seconds;
        let sms_window = self.config.sms.time_window_seconds;

        tokio::spawn(async move {
            let mut timer = interval(Duration::from_secs(cleanup_interval));
            
            loop {
                timer.tick().await;
                
                let now = Utc::now();
                let max_window = std::cmp::max(sip_window, sms_window);
                let cutoff_time = now - ChronoDuration::seconds((max_window * 2) as i64);
                
                // Clean up old failure records
                let expired_ips: Vec<IpAddr> = failure_tracking
                    .iter()
                    .filter_map(|entry| {
                        let tracker = entry.value();
                        if tracker.last_failure < cutoff_time && !tracker.is_banned {
                            Some(*entry.key())
                        } else {
                            None
                        }
                    })
                    .collect();
                
                for ip in expired_ips {
                    failure_tracking.remove(&ip);
                    debug!("Cleaned up expired failure records for IP: {}", ip);
                }
                
                // Check for expired bans
                let unbanned_ips: Vec<IpAddr> = failure_tracking
                    .iter_mut()
                    .filter_map(|mut entry| {
                        let tracker = entry.value_mut();
                        if tracker.is_banned {
                            if let Some(ban_expires) = tracker.ban_expires {
                                if now >= ban_expires {
                                    tracker.is_banned = false;
                                    tracker.ban_expires = None;
                                    Some(*entry.key())
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                    .collect();
                
                for ip in unbanned_ips {
                    info!("Ban expired for IP: {}", ip);
                }
            }
        });
    }
}

/// Convenience macros for recording failures
#[macro_export]
macro_rules! record_sip_failure {
    ($service:expr, $ip:expr, $failure_type:expr, $user:expr, $method:expr, $reason:expr) => {
        if let Some(service) = $service.as_ref() {
            if let Err(e) = service.record_sip_failure(
                $ip, 
                $failure_type, 
                $user.map(String::from), 
                $method.to_string(), 
                $reason.to_string(), 
                None
            ).await {
                tracing::warn!("Failed to record SIP failure: {}", e);
            }
        }
    };
}

#[macro_export]
macro_rules! record_sms_failure {
    ($service:expr, $ip:expr, $failure_type:expr, $system_id:expr, $reason:expr, $protocol:expr) => {
        if let Some(service) = $service.as_ref() {
            if let Err(e) = service.record_sms_failure(
                $ip, 
                $failure_type, 
                $system_id.map(String::from), 
                $reason.to_string(), 
                $protocol.to_string()
            ).await {
                tracing::warn!("Failed to record SMS failure: {}", e);
            }
        }
    };
}

// Utility functions
use std::str::FromStr;
use ipnetwork;

/// Parse IP address with fallback for IPv4-mapped IPv6
pub fn parse_ip_addr(addr_str: &str) -> Result<IpAddr> {
    // Try direct parsing first
    if let Ok(addr) = IpAddr::from_str(addr_str) {
        return Ok(addr);
    }
    
    // Handle IPv4-mapped IPv6 addresses
    if addr_str.starts_with("::ffff:") {
        let ipv4_part = &addr_str[7..];
        if let Ok(ipv4_addr) = std::net::Ipv4Addr::from_str(ipv4_part) {
            return Ok(IpAddr::V4(ipv4_addr));
        }
    }
    
    Err(anyhow!("Invalid IP address format: {}", addr_str))
}