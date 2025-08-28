//! IP blacklisting and reputation management
//! 
//! This module provides IP blacklisting, reputation scoring, and dynamic
//! threat response capabilities with per-trunk configuration support.

use super::{SecurityContext, SecurityError};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};

/// IP blacklist configuration with per-trunk overrides
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlacklistConfig {
    /// Enable IP blacklisting globally
    pub enable_global: bool,
    /// Enable per-trunk blacklist overrides
    pub enable_per_trunk: bool,
    /// Auto-blacklist threshold (reputation score)
    pub auto_blacklist_threshold: f64,
    /// Auto-whitelist threshold (reputation score)
    pub auto_whitelist_threshold: f64,
    /// Default blacklist expiry (seconds)
    pub default_blacklist_duration: u64,
    /// Maximum blacklist entries
    pub max_blacklist_entries: usize,
    /// Enable reputation scoring
    pub enable_reputation_scoring: bool,
    /// Enable dynamic reputation updates
    pub enable_dynamic_reputation: bool,
    /// Per-trunk security overrides
    pub trunk_overrides: HashMap<String, TrunkSecurityConfig>,
}

/// Per-trunk security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrunkSecurityConfig {
    /// Trunk ID
    pub trunk_id: String,
    /// Override global blacklist setting
    pub override_blacklist: Option<bool>,
    /// Override global reputation scoring
    pub override_reputation_scoring: Option<bool>,
    /// Override global threat detection
    pub override_threat_detection: Option<bool>,
    /// Override global rate limiting
    pub override_rate_limiting: Option<bool>,
    /// Override global input validation
    pub override_input_validation: Option<bool>,
    /// Custom blacklist for this trunk
    pub custom_blacklist: HashSet<IpAddr>,
    /// Custom whitelist for this trunk
    pub custom_whitelist: HashSet<IpAddr>,
    /// Custom reputation overrides
    pub reputation_overrides: HashMap<IpAddr, f64>,
}

impl Default for BlacklistConfig {
    fn default() -> Self {
        Self {
            enable_global: true,
            enable_per_trunk: true,
            auto_blacklist_threshold: 0.2,
            auto_whitelist_threshold: 0.8,
            default_blacklist_duration: 3600, // 1 hour
            max_blacklist_entries: 10000,
            enable_reputation_scoring: true,
            enable_dynamic_reputation: true,
            trunk_overrides: HashMap::new(),
        }
    }
}

/// Blacklist entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlacklistEntry {
    /// IP address
    pub ip_address: IpAddr,
    /// Reason for blacklisting
    pub reason: String,
    /// When blacklisted
    pub blacklisted_at: chrono::DateTime<chrono::Utc>,
    /// When blacklist expires
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Source of blacklist entry
    pub source: String,
    /// Severity level
    pub severity: String,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Reputation entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationEntry {
    /// IP address
    pub ip_address: IpAddr,
    /// Reputation score (0.0 = bad, 1.0 = good)
    pub score: f64,
    /// Last updated
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// Source of reputation data
    pub source: String,
    /// Number of interactions
    pub interaction_count: u32,
    /// Success rate
    pub success_rate: f64,
}

/// Blacklist and reputation manager
pub struct BlacklistManager {
    /// Configuration
    config: BlacklistConfig,
    /// Global blacklist
    global_blacklist: Arc<RwLock<HashMap<IpAddr, BlacklistEntry>>>,
    /// Global reputation scores
    reputation_scores: Arc<RwLock<HashMap<IpAddr, ReputationEntry>>>,
    /// Per-trunk configurations
    trunk_configs: Arc<RwLock<HashMap<String, TrunkSecurityConfig>>>,
}

impl BlacklistManager {
    /// Create new blacklist manager
    pub fn new(config: BlacklistConfig) -> Self {
        let trunk_configs: HashMap<String, TrunkSecurityConfig> = config.trunk_overrides.clone();
        
        Self {
            config,
            global_blacklist: Arc::new(RwLock::new(HashMap::new())),
            reputation_scores: Arc::new(RwLock::new(HashMap::new())),
            trunk_configs: Arc::new(RwLock::new(trunk_configs)),
        }
    }
    
    /// Check if IP is allowed for specific trunk
    pub async fn is_ip_allowed(&self, ip: IpAddr, trunk_id: Option<&str>) -> Result<bool> {
        // Check trunk-specific configuration first
        if let Some(trunk_id) = trunk_id {
            if let Some(allowed) = self.check_trunk_specific_rules(ip, trunk_id).await? {
                return Ok(allowed);
            }
        }
        
        // Fall back to global rules if enabled
        if !self.config.enable_global {
            return Ok(true); // Security disabled globally
        }
        
        // Check global blacklist
        let blacklist = self.global_blacklist.read().await;
        if let Some(entry) = blacklist.get(&ip) {
            // Check if blacklist entry has expired
            if let Some(expires_at) = entry.expires_at {
                if chrono::Utc::now() > expires_at {
                    drop(blacklist); // Release read lock
                    self.remove_from_blacklist(ip).await?;
                    return Ok(true);
                }
            }
            
            warn!("IP {} blocked by blacklist: {}", ip, entry.reason);
            return Ok(false);
        }
        
        // Check reputation-based blocking
        if self.config.enable_reputation_scoring {
            let reputation = self.reputation_scores.read().await;
            if let Some(entry) = reputation.get(&ip) {
                if entry.score < self.config.auto_blacklist_threshold {
                    warn!("IP {} blocked due to low reputation score: {}", ip, entry.score);
                    return Ok(false);
                }
            }
        }
        
        Ok(true)
    }
    
    /// Check trunk-specific security rules
    async fn check_trunk_specific_rules(&self, ip: IpAddr, trunk_id: &str) -> Result<Option<bool>> {
        let trunk_configs = self.trunk_configs.read().await;
        
        if let Some(trunk_config) = trunk_configs.get(trunk_id) {
            // Check trunk-specific whitelist first
            if trunk_config.custom_whitelist.contains(&ip) {
                debug!("IP {} allowed by trunk {} whitelist", ip, trunk_id);
                return Ok(Some(true));
            }
            
            // Check trunk-specific blacklist
            if trunk_config.custom_blacklist.contains(&ip) {
                warn!("IP {} blocked by trunk {} blacklist", ip, trunk_id);
                return Ok(Some(false));
            }
            
            // Check if blacklisting is disabled for this trunk
            if let Some(false) = trunk_config.override_blacklist {
                debug!("Blacklisting disabled for trunk {}", trunk_id);
                return Ok(Some(true));
            }
            
            // Check trunk-specific reputation overrides
            if let Some(score) = trunk_config.reputation_overrides.get(&ip) {
                if *score < self.config.auto_blacklist_threshold {
                    warn!("IP {} blocked by trunk {} reputation override: {}", ip, trunk_id, score);
                    return Ok(Some(false));
                }
            }
        }
        
        Ok(None) // No trunk-specific rule found
    }
    
    /// Add IP to blacklist
    pub async fn add_to_blacklist(
        &self,
        ip: IpAddr,
        reason: String,
        source: String,
        duration_seconds: Option<u64>,
        severity: String,
    ) -> Result<()> {
        let mut blacklist = self.global_blacklist.write().await;
        
        // Check if we're at capacity
        if blacklist.len() >= self.config.max_blacklist_entries {
            // Remove oldest entry
            let oldest_ip = blacklist.iter()
                .min_by_key(|(_, entry)| entry.blacklisted_at)
                .map(|(ip, _)| *ip);
            
            if let Some(oldest_ip) = oldest_ip {
                blacklist.remove(&oldest_ip);
                warn!("Blacklist at capacity, removed oldest entry: {}", oldest_ip);
            }
        }
        
        let expires_at = duration_seconds.map(|duration| {
            chrono::Utc::now() + chrono::Duration::seconds(duration as i64)
        });
        
        let entry = BlacklistEntry {
            ip_address: ip,
            reason: reason.clone(),
            blacklisted_at: chrono::Utc::now(),
            expires_at,
            source: source.clone(),
            severity: severity.clone(),
            metadata: HashMap::new(),
        };
        
        blacklist.insert(ip, entry);
        
        info!("Added IP {} to blacklist: {} (source: {}, severity: {})", 
              ip, reason, source, severity);
        
        Ok(())
    }
    
    /// Remove IP from blacklist
    pub async fn remove_from_blacklist(&self, ip: IpAddr) -> Result<bool> {
        let mut blacklist = self.global_blacklist.write().await;
        let removed = blacklist.remove(&ip).is_some();
        
        if removed {
            info!("Removed IP {} from blacklist", ip);
        }
        
        Ok(removed)
    }
    
    /// Update reputation score for IP
    pub async fn update_reputation(&self, ip: IpAddr, success: bool, source: String) -> Result<()> {
        if !self.config.enable_reputation_scoring {
            return Ok(());
        }
        
        let mut reputation = self.reputation_scores.write().await;
        
        let entry = reputation.entry(ip).or_insert_with(|| ReputationEntry {
            ip_address: ip,
            score: 0.5, // Start neutral
            updated_at: chrono::Utc::now(),
            source: source.clone(),
            interaction_count: 0,
            success_rate: 0.0,
        });
        
        entry.interaction_count += 1;
        entry.updated_at = chrono::Utc::now();
        
        // Update success rate
        let old_successes = (entry.success_rate * (entry.interaction_count - 1) as f64) as u32;
        let new_successes = if success { old_successes + 1 } else { old_successes };
        entry.success_rate = new_successes as f64 / entry.interaction_count as f64;
        
        // Update reputation score (weighted average with more weight on recent interactions)
        let weight = 0.1; // Adjust weight for new interactions
        if success {
            entry.score = entry.score * (1.0 - weight) + weight;
        } else {
            entry.score = entry.score * (1.0 - weight);
        }
        
        debug!("Updated reputation for {}: score={:.3}, success_rate={:.3}, interactions={}", 
               ip, entry.score, entry.success_rate, entry.interaction_count);
        
        // Check for auto-blacklisting
        if self.config.enable_dynamic_reputation {
            if entry.score < self.config.auto_blacklist_threshold && entry.interaction_count >= 10 {
                drop(reputation); // Release lock before calling add_to_blacklist
                self.add_to_blacklist(
                    ip,
                    format!("Auto-blacklisted due to low reputation: {:.3}", entry.score),
                    "reputation_system".to_string(),
                    Some(self.config.default_blacklist_duration),
                    "medium".to_string(),
                ).await?;
            }
        }
        
        Ok(())
    }
    
    /// Get reputation score for IP
    pub async fn get_reputation(&self, ip: IpAddr) -> Option<f64> {
        let reputation = self.reputation_scores.read().await;
        reputation.get(&ip).map(|entry| entry.score)
    }
    
    /// Add or update trunk configuration
    pub async fn configure_trunk(&self, trunk_config: TrunkSecurityConfig) -> Result<()> {
        let mut configs = self.trunk_configs.write().await;
        configs.insert(trunk_config.trunk_id.clone(), trunk_config.clone());
        
        info!("Updated security configuration for trunk: {}", trunk_config.trunk_id);
        Ok(())
    }
    
    /// Check if security feature is enabled for trunk
    pub async fn is_feature_enabled_for_trunk(&self, feature: &str, trunk_id: &str) -> bool {
        let configs = self.trunk_configs.read().await;
        
        if let Some(trunk_config) = configs.get(trunk_id) {
            match feature {
                "blacklist" => trunk_config.override_blacklist.unwrap_or(self.config.enable_global),
                "reputation_scoring" => trunk_config.override_reputation_scoring.unwrap_or(self.config.enable_reputation_scoring),
                "threat_detection" => trunk_config.override_threat_detection.unwrap_or(true), // Default enabled
                "rate_limiting" => trunk_config.override_rate_limiting.unwrap_or(true), // Default enabled
                "input_validation" => trunk_config.override_input_validation.unwrap_or(true), // Default enabled
                _ => self.config.enable_global,
            }
        } else {
            // No trunk-specific config, use global settings
            match feature {
                "blacklist" => self.config.enable_global,
                "reputation_scoring" => self.config.enable_reputation_scoring,
                _ => true, // Default enabled for other features
            }
        }
    }
    
    /// Get blacklist statistics
    pub async fn get_blacklist_stats(&self) -> Result<HashMap<String, u64>> {
        let blacklist = self.global_blacklist.read().await;
        let reputation = self.reputation_scores.read().await;
        
        let mut stats = HashMap::new();
        stats.insert("total_blacklisted".to_string(), blacklist.len() as u64);
        stats.insert("total_reputation_entries".to_string(), reputation.len() as u64);
        
        // Count by severity
        let mut severity_counts = HashMap::new();
        for entry in blacklist.values() {
            *severity_counts.entry(entry.severity.clone()).or_insert(0u64) += 1;
        }
        
        for (severity, count) in severity_counts {
            stats.insert(format!("blacklisted_{}", severity), count);
        }
        
        // Reputation distribution
        let mut high_rep = 0u64;
        let mut med_rep = 0u64;
        let mut low_rep = 0u64;
        
        for entry in reputation.values() {
            if entry.score > 0.7 {
                high_rep += 1;
            } else if entry.score > 0.3 {
                med_rep += 1;
            } else {
                low_rep += 1;
            }
        }
        
        stats.insert("high_reputation".to_string(), high_rep);
        stats.insert("medium_reputation".to_string(), med_rep);
        stats.insert("low_reputation".to_string(), low_rep);
        
        Ok(stats)
    }
    
    /// Clean up expired blacklist entries
    pub async fn cleanup_expired_entries(&self) -> usize {
        let mut blacklist = self.global_blacklist.write().await;
        let now = chrono::Utc::now();
        
        let expired_ips: Vec<IpAddr> = blacklist.iter()
            .filter_map(|(ip, entry)| {
                if let Some(expires_at) = entry.expires_at {
                    if now > expires_at {
                        Some(*ip)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        
        let removed_count = expired_ips.len();
        for ip in expired_ips {
            blacklist.remove(&ip);
        }
        
        if removed_count > 0 {
            info!("Cleaned up {} expired blacklist entries", removed_count);
        }
        
        removed_count
    }
    
    /// Import external blacklist
    pub async fn import_external_blacklist(&self, ips: Vec<IpAddr>, source: String) -> Result<usize> {
        let mut imported = 0;
        
        for ip in ips {
            self.add_to_blacklist(
                ip,
                "External threat intelligence".to_string(),
                source.clone(),
                None, // No expiration
                "high".to_string(),
            ).await?;
            imported += 1;
        }
        
        info!("Imported {} IPs from external blacklist: {}", imported, source);
        Ok(imported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[tokio::test]
    async fn test_blacklist_functionality() {
        let config = BlacklistConfig::default();
        let manager = BlacklistManager::new(config);
        let test_ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));
        
        // Initially should be allowed
        assert!(manager.is_ip_allowed(test_ip, None).await.unwrap());
        
        // Add to blacklist
        manager.add_to_blacklist(
            test_ip,
            "Test blacklist".to_string(),
            "test".to_string(),
            Some(3600),
            "medium".to_string(),
        ).await.unwrap();
        
        // Should now be blocked
        assert!(!manager.is_ip_allowed(test_ip, None).await.unwrap());
        
        // Remove from blacklist
        manager.remove_from_blacklist(test_ip).await.unwrap();
        
        // Should be allowed again
        assert!(manager.is_ip_allowed(test_ip, None).await.unwrap());
    }
    
    #[tokio::test]
    async fn test_trunk_specific_rules() {
        let config = BlacklistConfig::default();
        let manager = BlacklistManager::new(config);
        let test_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        
        // Configure trunk with custom whitelist
        let mut trunk_config = TrunkSecurityConfig {
            trunk_id: "trunk1".to_string(),
            override_blacklist: Some(true),
            override_reputation_scoring: None,
            override_threat_detection: None,
            override_rate_limiting: None,
            override_input_validation: None,
            custom_blacklist: HashSet::new(),
            custom_whitelist: HashSet::new(),
            reputation_overrides: HashMap::new(),
        };
        trunk_config.custom_whitelist.insert(test_ip);
        
        manager.configure_trunk(trunk_config).await.unwrap();
        
        // Add to global blacklist
        manager.add_to_blacklist(
            test_ip,
            "Global blacklist".to_string(),
            "test".to_string(),
            None,
            "high".to_string(),
        ).await.unwrap();
        
        // Should be blocked globally
        assert!(!manager.is_ip_allowed(test_ip, None).await.unwrap());
        
        // But allowed for trunk1 due to whitelist
        assert!(manager.is_ip_allowed(test_ip, Some("trunk1")).await.unwrap());
    }
    
    #[tokio::test]
    async fn test_reputation_system() {
        let config = BlacklistConfig::default();
        let manager = BlacklistManager::new(config);
        let test_ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1));
        
        // Update with successful interactions
        for _ in 0..10 {
            manager.update_reputation(test_ip, true, "test".to_string()).await.unwrap();
        }
        
        let reputation = manager.get_reputation(test_ip).await.unwrap();
        assert!(reputation > 0.7); // Should have good reputation
        
        // Update with many failures
        for _ in 0..50 {
            manager.update_reputation(test_ip, false, "test".to_string()).await.unwrap();
        }
        
        let reputation = manager.get_reputation(test_ip).await.unwrap();
        assert!(reputation < 0.3); // Should have bad reputation
    }
}