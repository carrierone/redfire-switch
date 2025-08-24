/*
 * Runtime Security Monitoring System for RedFire Switch B2BUA
 * Real-time threat detection and automated response
 */

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Security threat levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreatLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Types of security events detected
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SecurityEventType {
    // DoS Attacks
    MessageFlood,
    OversizedMessage,
    MalformedMessage,

    // Injection Attacks
    LogInjection,
    HeaderInjection,
    SipInjection,

    // Authentication Attacks
    JwtAlgorithmConfusion,
    InvalidStirShaken,
    CertificateValidationFailure,

    // Buffer Attacks
    BufferOverflowAttempt,
    InvalidUriFormat,
    HeaderTruncation,

    // Protocol Violations
    InvalidSipMethod,
    MissingRequiredHeaders,
    ProtocolViolation,

    // Reconnaissance
    PortScanning,
    MethodEnumeration,
    ServiceFingerprinting,
}

/// Security event details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityEvent {
    pub event_type: SecurityEventType,
    pub threat_level: ThreatLevel,
    pub source_ip: IpAddr,
    pub timestamp: SystemTime,
    pub details: String,
    pub payload_sample: Option<String>, // First 256 chars of payload
    pub count: u64,
}

/// Per-IP security statistics
#[derive(Debug)]
struct IpSecurityStats {
    message_count: u64,
    last_message: Instant,
    threat_events: HashMap<SecurityEventType, u64>,
    first_seen: Instant,
    is_blocked: bool,
    block_until: Option<Instant>,
}

impl Default for IpSecurityStats {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            message_count: 0,
            last_message: now,
            threat_events: HashMap::new(),
            first_seen: now,
            is_blocked: false,
            block_until: None,
        }
    }
}

/// Security monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityMonitorConfig {
    pub enabled: bool,
    pub log_security_events: bool,
    pub auto_block_enabled: bool,
    pub max_messages_per_second: u64,
    pub max_messages_per_minute: u64,
    pub block_duration_minutes: u64,
    pub threat_score_threshold: u64,
    pub oversized_message_threshold: usize,
    pub monitoring_window_minutes: u64,
}

impl Default for SecurityMonitorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            log_security_events: true,
            auto_block_enabled: true,
            max_messages_per_second: 100,
            max_messages_per_minute: 1000,
            block_duration_minutes: 15,
            threat_score_threshold: 10,
            oversized_message_threshold: 65536, // 64KB
            monitoring_window_minutes: 60,
        }
    }
}

/// Real-time security monitoring system
pub struct SecurityMonitor {
    config: SecurityMonitorConfig,
    ip_stats: Arc<RwLock<HashMap<IpAddr, IpSecurityStats>>>,
    security_events: Arc<RwLock<Vec<SecurityEvent>>>,
    blocked_ips: Arc<RwLock<HashMap<IpAddr, Instant>>>,
}

impl SecurityMonitor {
    pub fn new(config: SecurityMonitorConfig) -> Self {
        info!(
            "🛡️ Security Monitor initialized - Auto-block: {}",
            config.auto_block_enabled
        );

        Self {
            config,
            ip_stats: Arc::new(RwLock::new(HashMap::new())),
            security_events: Arc::new(RwLock::new(Vec::new())),
            blocked_ips: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Check if an IP address is currently blocked
    pub async fn is_ip_blocked(&self, ip: IpAddr) -> bool {
        if !self.config.enabled || !self.config.auto_block_enabled {
            return false;
        }

        let blocked_ips = self.blocked_ips.read().await;
        if let Some(block_until) = blocked_ips.get(&ip) {
            if Instant::now() < *block_until {
                debug!("IP {} is blocked until {:?}", ip, block_until);
                return true;
            }
        }
        false
    }

    /// Record a security event and analyze for threats
    pub async fn record_security_event(
        &self,
        event_type: SecurityEventType,
        source_ip: IpAddr,
        details: String,
        payload_sample: Option<String>,
    ) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let threat_level = self.assess_threat_level(&event_type, &details);

        let event = SecurityEvent {
            event_type: event_type.clone(),
            threat_level,
            source_ip,
            timestamp: SystemTime::now(),
            details: details.clone(),
            payload_sample: payload_sample.clone(),
            count: 1,
        };

        // Log security event
        if self.config.log_security_events {
            match threat_level {
                ThreatLevel::Critical => error!(
                    "🚨 CRITICAL SECURITY EVENT: {:?} from {} - {}",
                    event_type, source_ip, details
                ),
                ThreatLevel::High => warn!(
                    "⚠️ HIGH THREAT: {:?} from {} - {}",
                    event_type, source_ip, details
                ),
                ThreatLevel::Medium => warn!(
                    "⚠️ MEDIUM THREAT: {:?} from {} - {}",
                    event_type, source_ip, details
                ),
                ThreatLevel::Low => debug!(
                    "ℹ️ Security Event: {:?} from {} - {}",
                    event_type, source_ip, details
                ),
            }
        }

        // Update IP statistics
        self.update_ip_stats(source_ip, &event_type).await;

        // Store security event
        {
            let mut events = self.security_events.write().await;
            events.push(event);

            // Keep only recent events (sliding window)
            let cutoff =
                SystemTime::now() - Duration::from_secs(self.config.monitoring_window_minutes * 60);
            events.retain(|e| e.timestamp > cutoff);
        }

        // Check if IP should be blocked
        if self.config.auto_block_enabled {
            self.evaluate_ip_for_blocking(source_ip).await?;
        }

        Ok(())
    }

    /// Analyze message for potential security threats
    pub async fn analyze_message(
        &self,
        source_ip: IpAddr,
        message: &str,
    ) -> Result<Vec<SecurityEventType>> {
        if !self.config.enabled {
            return Ok(Vec::new());
        }

        let mut detected_threats = Vec::new();

        // Check message size
        if message.len() > self.config.oversized_message_threshold {
            detected_threats.push(SecurityEventType::OversizedMessage);
            self.record_security_event(
                SecurityEventType::OversizedMessage,
                source_ip,
                format!("Message size: {} bytes", message.len()),
                Some(message.chars().take(256).collect()),
            )
            .await?;
        }

        // Check for log injection attempts
        if self.contains_log_injection(message) {
            detected_threats.push(SecurityEventType::LogInjection);
            self.record_security_event(
                SecurityEventType::LogInjection,
                source_ip,
                "Log injection pattern detected".to_string(),
                Some(crate::security_utils::sanitize_for_logging(message)),
            )
            .await?;
        }

        // Check for header injection
        if self.contains_header_injection(message) {
            detected_threats.push(SecurityEventType::HeaderInjection);
            self.record_security_event(
                SecurityEventType::HeaderInjection,
                source_ip,
                "Header injection pattern detected".to_string(),
                Some(message.lines().next().unwrap_or("").to_string()),
            )
            .await?;
        }

        // Check for malformed SIP structure
        if self.is_malformed_sip(message) {
            detected_threats.push(SecurityEventType::MalformedMessage);
            self.record_security_event(
                SecurityEventType::MalformedMessage,
                source_ip,
                "Malformed SIP message structure".to_string(),
                Some(message.lines().take(3).collect::<Vec<_>>().join("\\n")),
            )
            .await?;
        }

        // Check for JWT algorithm confusion in Identity headers
        if self.contains_jwt_algorithm_confusion(message) {
            detected_threats.push(SecurityEventType::JwtAlgorithmConfusion);
            self.record_security_event(
                SecurityEventType::JwtAlgorithmConfusion,
                source_ip,
                "JWT algorithm confusion attempt detected".to_string(),
                None, // Don't log JWT tokens
            )
            .await?;
        }

        // Check rate limiting
        if self.check_rate_limit(source_ip).await? {
            detected_threats.push(SecurityEventType::MessageFlood);
            self.record_security_event(
                SecurityEventType::MessageFlood,
                source_ip,
                "Message rate limit exceeded".to_string(),
                None,
            )
            .await?;
        }

        Ok(detected_threats)
    }

    /// Check if message contains log injection patterns
    fn contains_log_injection(&self, message: &str) -> bool {
        let injection_patterns = [
            "\x1b[", // ANSI escape sequences
            "\n", "\r", // Newline injection
            "\x00", "\x01", "\x02", // Control characters
            "\\n", "\\r", // Escaped newlines
        ];

        injection_patterns
            .iter()
            .any(|pattern| message.contains(pattern))
    }

    /// Check if message contains header injection patterns
    fn contains_header_injection(&self, message: &str) -> bool {
        // Look for CRLF injection attempts
        message.contains("\r\n\r\n") && message.lines().count() > 10
    }

    /// Check if SIP message is malformed
    fn is_malformed_sip(&self, message: &str) -> bool {
        let lines: Vec<&str> = message.lines().collect();

        // Check if first line looks like a SIP request or response
        if lines.is_empty() {
            return true;
        }

        let first_line = lines[0];

        // Valid SIP request methods
        let valid_methods = [
            "INVITE", "ACK", "BYE", "CANCEL", "OPTIONS", "REGISTER", "PRACK", "UPDATE",
        ];

        // Check if it's a SIP request
        if valid_methods
            .iter()
            .any(|method| first_line.starts_with(method))
        {
            return !first_line.contains("SIP/2.0");
        }

        // Check if it's a SIP response
        if first_line.starts_with("SIP/2.0") {
            // Should have a status code
            let parts: Vec<&str> = first_line.split_whitespace().collect();
            if parts.len() < 3 {
                return true;
            }
            // Status code should be numeric
            return parts[1].parse::<u16>().is_err();
        }

        // Not a valid SIP message
        true
    }

    /// Check for JWT algorithm confusion attempts
    fn contains_jwt_algorithm_confusion(&self, message: &str) -> bool {
        if !message.contains("Identity:") {
            return false;
        }

        // Look for JWT tokens with "none" algorithm
        for line in message.lines() {
            if line.to_lowercase().starts_with("identity:") {
                let jwt_part = line.split(':').nth(1).unwrap_or("").trim();
                if jwt_part.len() > 10 {
                    // Decode JWT header to check algorithm
                    if let Some(header_part) = jwt_part.split('.').next() {
                        use base64::{engine::general_purpose, Engine as _};
                        if let Ok(decoded) = general_purpose::STANDARD.decode(header_part) {
                            if let Ok(header_str) = String::from_utf8(decoded) {
                                if header_str.contains("\"alg\":\"none\"")
                                    || header_str.contains("\"alg\":\"HS256\"")
                                {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
        }
        false
    }

    /// Check rate limiting for an IP
    async fn check_rate_limit(&self, source_ip: IpAddr) -> Result<bool> {
        let mut ip_stats = self.ip_stats.write().await;
        let now = Instant::now();

        let stats = ip_stats
            .entry(source_ip)
            .or_insert_with(|| IpSecurityStats {
                first_seen: now,
                last_message: now,
                ..Default::default()
            });

        stats.message_count += 1;

        // Check messages per second
        if now.duration_since(stats.last_message) < Duration::from_secs(1) {
            if stats.message_count > self.config.max_messages_per_second {
                return Ok(true);
            }
        }

        // Check messages per minute
        if now.duration_since(stats.first_seen) < Duration::from_secs(60) {
            if stats.message_count > self.config.max_messages_per_minute {
                return Ok(true);
            }
        }

        stats.last_message = now;
        Ok(false)
    }

    /// Update IP statistics for security analysis
    async fn update_ip_stats(&self, source_ip: IpAddr, event_type: &SecurityEventType) {
        let mut ip_stats = self.ip_stats.write().await;
        let now = Instant::now();

        let stats = ip_stats
            .entry(source_ip)
            .or_insert_with(|| IpSecurityStats {
                first_seen: now,
                last_message: now,
                ..Default::default()
            });

        *stats.threat_events.entry(event_type.clone()).or_insert(0) += 1;
        stats.last_message = now;
    }

    /// Evaluate if an IP should be blocked based on threat score
    async fn evaluate_ip_for_blocking(&self, source_ip: IpAddr) -> Result<()> {
        let ip_stats = self.ip_stats.read().await;

        if let Some(stats) = ip_stats.get(&source_ip) {
            let threat_score = self.calculate_threat_score(stats);

            if threat_score >= self.config.threat_score_threshold {
                drop(ip_stats); // Release read lock

                let block_until =
                    Instant::now() + Duration::from_secs(self.config.block_duration_minutes * 60);

                {
                    let mut blocked_ips = self.blocked_ips.write().await;
                    blocked_ips.insert(source_ip, block_until);
                }

                warn!(
                    "🚫 BLOCKING IP {} for {} minutes (threat score: {})",
                    source_ip, self.config.block_duration_minutes, threat_score
                );

                // Avoid recursion by directly logging without calling record_security_event
                warn!(
                    "🚫 IP {} blocked due to high threat score: {}",
                    source_ip, threat_score
                );
            }
        }

        Ok(())
    }

    /// Calculate threat score for an IP based on events
    fn calculate_threat_score(&self, stats: &IpSecurityStats) -> u64 {
        let mut score = 0;

        for (event_type, count) in &stats.threat_events {
            let event_weight = match event_type {
                SecurityEventType::JwtAlgorithmConfusion => 5,
                SecurityEventType::BufferOverflowAttempt => 5,
                SecurityEventType::LogInjection => 3,
                SecurityEventType::HeaderInjection => 3,
                SecurityEventType::MessageFlood => 2,
                SecurityEventType::OversizedMessage => 2,
                SecurityEventType::MalformedMessage => 1,
                _ => 1,
            };
            score += count * event_weight;
        }

        score
    }

    /// Assess threat level for a security event
    fn assess_threat_level(&self, event_type: &SecurityEventType, _details: &str) -> ThreatLevel {
        match event_type {
            SecurityEventType::JwtAlgorithmConfusion => ThreatLevel::Critical,
            SecurityEventType::BufferOverflowAttempt => ThreatLevel::Critical,
            SecurityEventType::LogInjection => ThreatLevel::High,
            SecurityEventType::HeaderInjection => ThreatLevel::High,
            SecurityEventType::MessageFlood => ThreatLevel::Medium,
            SecurityEventType::OversizedMessage => ThreatLevel::Medium,
            SecurityEventType::MalformedMessage => ThreatLevel::Low,
            SecurityEventType::InvalidSipMethod => ThreatLevel::Low,
            _ => ThreatLevel::Low,
        }
    }

    /// Get security statistics for monitoring dashboard
    pub async fn get_security_stats(&self) -> Result<SecurityStats> {
        let ip_stats = self.ip_stats.read().await;
        let events = self.security_events.read().await;
        let blocked_ips = self.blocked_ips.read().await;

        let mut event_counts = HashMap::new();
        for event in events.iter() {
            *event_counts.entry(event.event_type.clone()).or_insert(0) += 1;
        }

        Ok(SecurityStats {
            total_monitored_ips: ip_stats.len(),
            currently_blocked_ips: blocked_ips.len(),
            total_security_events: events.len(),
            event_type_counts: event_counts,
            monitoring_enabled: self.config.enabled,
            auto_block_enabled: self.config.auto_block_enabled,
        })
    }

    /// Start cleanup task for expired blocks and old events
    pub async fn start_cleanup_task(&self) {
        let blocked_ips = Arc::clone(&self.blocked_ips);
        let security_events = Arc::clone(&self.security_events);
        let monitoring_window = self.config.monitoring_window_minutes;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes

            loop {
                interval.tick().await;

                let now = Instant::now();

                // Clean up expired IP blocks
                {
                    let mut blocked = blocked_ips.write().await;
                    blocked.retain(|_ip, block_until| now < *block_until);
                }

                // Clean up old security events
                {
                    let mut events = security_events.write().await;
                    let cutoff = SystemTime::now() - Duration::from_secs(monitoring_window * 60);
                    events.retain(|e| e.timestamp > cutoff);
                }

                debug!("Security monitor cleanup completed");
            }
        });
    }
}

/// Security statistics for monitoring dashboard
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SecurityStats {
    pub total_monitored_ips: usize,
    pub currently_blocked_ips: usize,
    pub total_security_events: usize,
    pub event_type_counts: HashMap<SecurityEventType, u64>,
    pub monitoring_enabled: bool,
    pub auto_block_enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[tokio::test]
    async fn test_security_monitor_basic() {
        let config = SecurityMonitorConfig::default();
        let monitor = SecurityMonitor::new(config);

        let test_ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));

        // Should not be blocked initially
        assert!(!monitor.is_ip_blocked(test_ip).await);

        // Record a security event
        monitor
            .record_security_event(
                SecurityEventType::LogInjection,
                test_ip,
                "Test log injection".to_string(),
                None,
            )
            .await
            .unwrap();

        let stats = monitor.get_security_stats().await.unwrap();
        assert_eq!(stats.total_security_events, 1);
    }

    #[tokio::test]
    async fn test_malformed_sip_detection() {
        let config = SecurityMonitorConfig::default();
        let monitor = SecurityMonitor::new(config);

        let test_ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));

        // Test malformed SIP message
        let malformed_msg = "INVALID MESSAGE\r\n\r\n";
        let threats = monitor
            .analyze_message(test_ip, malformed_msg)
            .await
            .unwrap();

        assert!(threats.contains(&SecurityEventType::MalformedMessage));
    }

    #[tokio::test]
    async fn test_oversized_message_detection() {
        let config = SecurityMonitorConfig {
            oversized_message_threshold: 100,
            ..Default::default()
        };
        let monitor = SecurityMonitor::new(config);

        let test_ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));

        // Create oversized message
        let oversized_msg = "A".repeat(200);
        let threats = monitor
            .analyze_message(test_ip, &oversized_msg)
            .await
            .unwrap();

        assert!(threats.contains(&SecurityEventType::OversizedMessage));
    }
}
