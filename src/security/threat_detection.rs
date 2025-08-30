//! Advanced threat detection and anomaly analysis
//!
//! This module provides sophisticated threat detection capabilities including
//! behavioral analysis, anomaly detection, and threat intelligence integration.

use super::SecurityContext;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Threat detection engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreatDetectionConfig {
    /// Enable behavioral analysis
    pub enable_behavioral_analysis: bool,
    /// Enable geolocation checks
    pub enable_geolocation_checks: bool,
    /// Enable reputation scoring
    pub enable_reputation_scoring: bool,
    /// Anomaly detection threshold (0.0 - 1.0)
    pub anomaly_threshold: f64,
    /// Minimum calls before behavioral analysis
    pub min_calls_for_analysis: u32,
    /// Time window for analysis (seconds)
    pub analysis_window_seconds: u64,
    /// Countries to block (ISO 2-letter codes)
    pub blocked_countries: Vec<String>,
    /// High-risk countries requiring additional validation
    pub high_risk_countries: Vec<String>,
}

impl Default for ThreatDetectionConfig {
    fn default() -> Self {
        Self {
            enable_behavioral_analysis: true,
            enable_geolocation_checks: true,
            enable_reputation_scoring: true,
            anomaly_threshold: 0.7,
            min_calls_for_analysis: 10,
            analysis_window_seconds: 3600,               // 1 hour
            blocked_countries: vec!["XX".to_string()],   // Placeholder
            high_risk_countries: vec!["YY".to_string()], // Placeholder
        }
    }
}

/// Threat severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreatSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Threat types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThreatType {
    /// Unusual call patterns
    AnomalousCallPattern {
        pattern_type: String,
        confidence: f64,
        metrics: HashMap<String, f64>,
    },
    /// Suspicious geographic activity
    GeographicAnomaly {
        country_code: String,
        unusual_pattern: String,
    },
    /// High-frequency calling (potential spam/robocalling)
    HighFrequencyCalling {
        calls_per_minute: f64,
        target_count: u32,
    },
    /// Short duration calls (potential scanning)
    ScanningBehavior {
        avg_duration: f64,
        success_rate: f64,
        target_range: String,
    },
    /// Sequential number dialing
    SequentialDialing {
        sequence_length: u32,
        pattern: String,
    },
    /// Known bad actor
    ReputationThreat {
        reputation_score: f64,
        threat_intelligence: String,
    },
}

/// Threat detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreatDetection {
    /// Unique detection ID
    pub id: String,
    /// Source IP address
    pub source_ip: IpAddr,
    /// Threat type and details
    pub threat_type: ThreatType,
    /// Severity level
    pub severity: ThreatSeverity,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Detection timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
    /// Recommended actions
    pub recommended_actions: Vec<String>,
}

/// Call behavior profile for an IP/user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallBehaviorProfile {
    /// IP address
    pub ip_address: IpAddr,
    /// Total calls made
    pub total_calls: u32,
    /// Successful calls
    pub successful_calls: u32,
    /// Average call duration (seconds)
    pub avg_call_duration: f64,
    /// Call frequency (calls per hour)
    pub calls_per_hour: f64,
    /// Unique numbers called
    pub unique_destinations: u32,
    /// Countries called
    pub countries_called: Vec<String>,
    /// Time pattern analysis
    pub time_patterns: HashMap<u8, u32>, // Hour -> call count
    /// Last activity
    pub last_activity: chrono::DateTime<chrono::Utc>,
    /// Reputation score
    pub reputation_score: f64,
}

/// Advanced threat detection engine
pub struct ThreatDetectionEngine {
    /// Configuration
    config: ThreatDetectionConfig,
    /// Call behavior profiles
    behavior_profiles: Arc<RwLock<HashMap<IpAddr, CallBehaviorProfile>>>,
    /// IP reputation cache
    reputation_cache: Arc<RwLock<HashMap<IpAddr, (f64, chrono::DateTime<chrono::Utc>)>>>,
    /// Active threats
    active_threats: Arc<RwLock<HashMap<String, ThreatDetection>>>,
    /// Geolocation database (simplified)
    geolocation_db: Arc<RwLock<HashMap<IpAddr, String>>>,
}

impl ThreatDetectionEngine {
    /// Create new threat detection engine
    pub fn new(config: ThreatDetectionConfig) -> Self {
        Self {
            config,
            behavior_profiles: Arc::new(RwLock::new(HashMap::new())),
            reputation_cache: Arc::new(RwLock::new(HashMap::new())),
            active_threats: Arc::new(RwLock::new(HashMap::new())),
            geolocation_db: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Analyze security context for threats
    pub async fn analyze_security_context(
        &self,
        context: &SecurityContext,
    ) -> Result<Vec<ThreatDetection>> {
        let mut threats = Vec::new();

        // Get or create behavior profile
        let mut profile = self.get_or_create_profile(context.source_ip).await;

        // Update profile with current activity
        self.update_profile(&mut profile, context).await;

        // Perform various threat analyses
        if self.config.enable_behavioral_analysis {
            if let Some(threat) = self.analyze_behavioral_anomalies(&profile).await? {
                threats.push(threat);
            }
        }

        if self.config.enable_geolocation_checks {
            if let Some(threat) = self.analyze_geographic_anomalies(&profile).await? {
                threats.push(threat);
            }
        }

        if self.config.enable_reputation_scoring {
            if let Some(threat) = self.analyze_reputation_threats(&profile).await? {
                threats.push(threat);
            }
        }

        // Check for scanning behavior
        if let Some(threat) = self.analyze_scanning_behavior(&profile).await? {
            threats.push(threat);
        }

        // Check for high-frequency calling
        if let Some(threat) = self.analyze_high_frequency_calling(&profile).await? {
            threats.push(threat);
        }

        // Store updated profile
        self.store_profile(profile).await;

        // Register any new threats
        for threat in &threats {
            self.register_threat(threat.clone()).await;
        }

        Ok(threats)
    }

    /// Get or create behavior profile for IP
    async fn get_or_create_profile(&self, ip: IpAddr) -> CallBehaviorProfile {
        let profiles = self.behavior_profiles.read().await;
        if let Some(profile) = profiles.get(&ip) {
            profile.clone()
        } else {
            drop(profiles); // Release read lock

            CallBehaviorProfile {
                ip_address: ip,
                total_calls: 0,
                successful_calls: 0,
                avg_call_duration: 0.0,
                calls_per_hour: 0.0,
                unique_destinations: 0,
                countries_called: Vec::new(),
                time_patterns: HashMap::new(),
                last_activity: chrono::Utc::now(),
                reputation_score: 0.5, // Neutral
            }
        }
    }

    /// Update behavior profile with current activity
    async fn update_profile(&self, profile: &mut CallBehaviorProfile, context: &SecurityContext) {
        profile.total_calls += 1;
        profile.last_activity = chrono::Utc::now();

        // Update time patterns - using format to extract hour
        let hour = profile
            .last_activity
            .format("%H")
            .to_string()
            .parse::<u8>()
            .unwrap_or(0);
        *profile.time_patterns.entry(hour).or_insert(0) += 1;

        // Update calls per hour (simplified rolling average)
        profile.calls_per_hour = profile.calls_per_hour * 0.9 + 0.1;

        debug!(
            "Updated behavior profile for {}: {} total calls",
            profile.ip_address, profile.total_calls
        );
    }

    /// Analyze behavioral anomalies
    async fn analyze_behavioral_anomalies(
        &self,
        profile: &CallBehaviorProfile,
    ) -> Result<Option<ThreatDetection>> {
        if profile.total_calls < self.config.min_calls_for_analysis {
            return Ok(None);
        }

        let mut anomaly_score = 0.0;
        let mut metrics = HashMap::new();

        // Check call success rate
        let success_rate = profile.successful_calls as f64 / profile.total_calls as f64;
        if success_rate < 0.1 {
            anomaly_score += 0.4;
            metrics.insert("low_success_rate".to_string(), success_rate);
        }

        // Check call frequency
        if profile.calls_per_hour > 60.0 {
            anomaly_score += 0.3;
            metrics.insert("high_frequency".to_string(), profile.calls_per_hour);
        }

        // Check average call duration (very short calls are suspicious)
        if profile.avg_call_duration < 5.0 && profile.total_calls > 20 {
            anomaly_score += 0.3;
            metrics.insert("short_duration".to_string(), profile.avg_call_duration);
        }

        if anomaly_score >= self.config.anomaly_threshold {
            let severity = match anomaly_score {
                s if s >= 0.9 => ThreatSeverity::Critical,
                s if s >= 0.8 => ThreatSeverity::High,
                s if s >= 0.7 => ThreatSeverity::Medium,
                _ => ThreatSeverity::Low,
            };

            return Ok(Some(ThreatDetection {
                id: uuid::Uuid::new_v4().to_string(),
                source_ip: profile.ip_address,
                threat_type: ThreatType::AnomalousCallPattern {
                    pattern_type: "behavioral_anomaly".to_string(),
                    confidence: anomaly_score,
                    metrics,
                },
                severity,
                confidence: anomaly_score,
                timestamp: chrono::Utc::now(),
                metadata: HashMap::new(),
                recommended_actions: vec![
                    "Monitor closely".to_string(),
                    "Apply additional rate limiting".to_string(),
                    "Require authentication".to_string(),
                ],
            }));
        }

        Ok(None)
    }

    /// Analyze geographic anomalies
    async fn analyze_geographic_anomalies(
        &self,
        profile: &CallBehaviorProfile,
    ) -> Result<Option<ThreatDetection>> {
        // Get country for IP (simplified - in production would use GeoIP database)
        let country_code = self.get_country_for_ip(profile.ip_address).await;

        if self.config.blocked_countries.contains(&country_code) {
            return Ok(Some(ThreatDetection {
                id: uuid::Uuid::new_v4().to_string(),
                source_ip: profile.ip_address,
                threat_type: ThreatType::GeographicAnomaly {
                    country_code: country_code.clone(),
                    unusual_pattern: "blocked_country".to_string(),
                },
                severity: ThreatSeverity::High,
                confidence: 1.0,
                timestamp: chrono::Utc::now(),
                metadata: HashMap::new(),
                recommended_actions: vec![
                    "Block immediately".to_string(),
                    "Add to blacklist".to_string(),
                ],
            }));
        }

        if self.config.high_risk_countries.contains(&country_code) && profile.total_calls > 5 {
            return Ok(Some(ThreatDetection {
                id: uuid::Uuid::new_v4().to_string(),
                source_ip: profile.ip_address,
                threat_type: ThreatType::GeographicAnomaly {
                    country_code,
                    unusual_pattern: "high_risk_country".to_string(),
                },
                severity: ThreatSeverity::Medium,
                confidence: 0.8,
                timestamp: chrono::Utc::now(),
                metadata: HashMap::new(),
                recommended_actions: vec![
                    "Enhanced monitoring".to_string(),
                    "Require additional verification".to_string(),
                ],
            }));
        }

        Ok(None)
    }

    /// Analyze reputation-based threats
    async fn analyze_reputation_threats(
        &self,
        profile: &CallBehaviorProfile,
    ) -> Result<Option<ThreatDetection>> {
        if profile.reputation_score < 0.2 {
            return Ok(Some(ThreatDetection {
                id: uuid::Uuid::new_v4().to_string(),
                source_ip: profile.ip_address,
                threat_type: ThreatType::ReputationThreat {
                    reputation_score: profile.reputation_score,
                    threat_intelligence: "Low reputation score".to_string(),
                },
                severity: ThreatSeverity::High,
                confidence: 1.0 - profile.reputation_score,
                timestamp: chrono::Utc::now(),
                metadata: HashMap::new(),
                recommended_actions: vec![
                    "Block or restrict".to_string(),
                    "Manual review required".to_string(),
                ],
            }));
        }

        Ok(None)
    }

    /// Analyze scanning behavior
    async fn analyze_scanning_behavior(
        &self,
        profile: &CallBehaviorProfile,
    ) -> Result<Option<ThreatDetection>> {
        let success_rate = profile.successful_calls as f64 / profile.total_calls as f64;

        if profile.avg_call_duration < 3.0 && success_rate < 0.2 && profile.unique_destinations > 50
        {
            return Ok(Some(ThreatDetection {
                id: uuid::Uuid::new_v4().to_string(),
                source_ip: profile.ip_address,
                threat_type: ThreatType::ScanningBehavior {
                    avg_duration: profile.avg_call_duration,
                    success_rate,
                    target_range: format!("{} unique targets", profile.unique_destinations),
                },
                severity: ThreatSeverity::High,
                confidence: 0.9,
                timestamp: chrono::Utc::now(),
                metadata: HashMap::new(),
                recommended_actions: vec![
                    "Block immediately".to_string(),
                    "Report to threat intelligence".to_string(),
                ],
            }));
        }

        Ok(None)
    }

    /// Analyze high-frequency calling patterns
    async fn analyze_high_frequency_calling(
        &self,
        profile: &CallBehaviorProfile,
    ) -> Result<Option<ThreatDetection>> {
        if profile.calls_per_hour > 120.0 {
            // More than 2 calls per minute
            return Ok(Some(ThreatDetection {
                id: uuid::Uuid::new_v4().to_string(),
                source_ip: profile.ip_address,
                threat_type: ThreatType::HighFrequencyCalling {
                    calls_per_minute: profile.calls_per_hour / 60.0,
                    target_count: profile.unique_destinations,
                },
                severity: ThreatSeverity::Medium,
                confidence: 0.8,
                timestamp: chrono::Utc::now(),
                metadata: HashMap::new(),
                recommended_actions: vec![
                    "Apply aggressive rate limiting".to_string(),
                    "Monitor for spam/robocalling".to_string(),
                ],
            }));
        }

        Ok(None)
    }

    /// Get country code for IP address (simplified)
    async fn get_country_for_ip(&self, ip: IpAddr) -> String {
        let geo_db = self.geolocation_db.read().await;
        geo_db.get(&ip).cloned().unwrap_or_else(|| "US".to_string()) // Default to US
    }

    /// Store updated behavior profile
    async fn store_profile(&self, profile: CallBehaviorProfile) {
        let mut profiles = self.behavior_profiles.write().await;
        profiles.insert(profile.ip_address, profile);
    }

    /// Register active threat
    async fn register_threat(&self, threat: ThreatDetection) {
        let mut threats = self.active_threats.write().await;
        threats.insert(threat.id.clone(), threat);
    }

    /// Get active threats for IP
    pub async fn get_threats_for_ip(&self, ip: IpAddr) -> Vec<ThreatDetection> {
        let threats = self.active_threats.read().await;
        threats
            .values()
            .filter(|t| t.source_ip == ip)
            .cloned()
            .collect()
    }

    /// Clear expired threats
    pub async fn cleanup_expired_threats(&self) -> usize {
        let mut threats = self.active_threats.write().await;
        let now = chrono::Utc::now();
        let expiry_duration = chrono::Duration::hours(24);

        let expired_threats: Vec<String> = threats
            .iter()
            .filter_map(|(id, threat)| {
                if now - threat.timestamp > expiry_duration {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect();

        let removed_count = expired_threats.len();
        for id in expired_threats {
            threats.remove(&id);
        }

        if removed_count > 0 {
            info!("Cleaned up {} expired threat detections", removed_count);
        }

        removed_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[tokio::test]
    async fn test_threat_detection_engine() {
        let config = ThreatDetectionConfig::default();
        let engine = ThreatDetectionEngine::new(config);

        let context = SecurityContext::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)));

        let threats = engine.analyze_security_context(&context).await.unwrap();
        // Should have no threats for new IP with default behavior
        assert!(threats.is_empty());
    }

    #[tokio::test]
    async fn test_behavioral_anomaly_detection() {
        let mut config = ThreatDetectionConfig::default();
        config.min_calls_for_analysis = 1; // Lower threshold for testing

        let engine = ThreatDetectionEngine::new(config);

        // Create a suspicious profile
        let mut profile = CallBehaviorProfile {
            ip_address: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            total_calls: 100,
            successful_calls: 5,    // Very low success rate
            avg_call_duration: 1.0, // Very short calls
            calls_per_hour: 200.0,  // Very high frequency
            unique_destinations: 0,
            countries_called: Vec::new(),
            time_patterns: HashMap::new(),
            last_activity: chrono::Utc::now(),
            reputation_score: 0.5,
        };

        let threat = engine.analyze_behavioral_anomalies(&profile).await.unwrap();
        assert!(threat.is_some());

        let threat = threat.unwrap();
        assert!(matches!(
            threat.severity,
            ThreatSeverity::High | ThreatSeverity::Critical
        ));
    }
}
