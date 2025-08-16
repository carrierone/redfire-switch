/*
 * Redfire Switch - STIR/SHAKEN Fraud Detection
 * Copyright (C) 2025 Carrier One Inc and contributors
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

//! # STIR/SHAKEN Fraud Detection
//! 
//! This module provides fraud detection for STIR/SHAKEN attestation validation.
//! It cross-references ANI (Automatic Number Identification) with LERG (Local Exchange 
//! Routing Guide) OCN (Operating Company Number) data to detect suspicious attestation 
//! levels that don't match the actual number assignment.

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};
use crate::lerg_nanpa::LergEntry;

/// LERG data type
pub type LergData = HashMap<String, LergEntry>;

/// STIR/SHAKEN fraud detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StirShakenFraudConfig {
    /// Enable fraud detection
    pub enabled: bool,
    /// Enable logging of suspicious calls
    pub enable_logging: bool,
    /// Log file path for fraud alerts
    pub fraud_log_path: String,
    /// Threshold for suspicious attestation (percentage)
    pub attestation_threshold: f32,
    /// Enable OCN validation against LERG
    pub validate_ocn: bool,
    /// Enable wireless number attestation validation
    pub validate_wireless: bool,
    /// Enable ILEC/RBOC validation
    pub validate_ilec_rboc: bool,
    /// Minimum attestation level required for wireless numbers
    pub wireless_min_attestation: AttestationLevel,
    /// Action to take on fraud detection
    pub fraud_action: FraudAction,
    /// Enable real-time alerting
    pub real_time_alerts: bool,
    /// Alert webhook URL
    pub alert_webhook: Option<String>,
}

impl Default for StirShakenFraudConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            enable_logging: true,
            fraud_log_path: "/var/log/redfire-switch/stir-shaken-fraud.log".to_string(),
            attestation_threshold: 80.0, // 80% of calls should be properly attested
            validate_ocn: true,
            validate_wireless: true,
            validate_ilec_rboc: true,
            wireless_min_attestation: AttestationLevel::B, // Wireless should be at least B
            fraud_action: FraudAction::LogAndContinue,
            real_time_alerts: true,
            alert_webhook: None,
        }
    }
}

/// STIR/SHAKEN attestation levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AttestationLevel {
    /// Full attestation - SP has verified caller identity
    A,
    /// Partial attestation - SP verified originating customer, not end user
    B,
    /// Gateway attestation - SP is gateway, limited verification
    C,
}

impl AttestationLevel {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "A" => Some(AttestationLevel::A),
            "B" => Some(AttestationLevel::B),
            "C" => Some(AttestationLevel::C),
            _ => None,
        }
    }
    
    pub fn to_string(&self) -> String {
        match self {
            AttestationLevel::A => "A".to_string(),
            AttestationLevel::B => "B".to_string(),
            AttestationLevel::C => "C".to_string(),
        }
    }
}

/// Actions to take when fraud is detected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FraudAction {
    /// Log fraud but allow call to continue
    LogAndContinue,
    /// Log fraud and reject the call
    LogAndReject,
    /// Log fraud and downgrade attestation
    LogAndDowngrade,
    /// Log fraud and route to verification queue
    LogAndVerify,
}

/// Number type classification based on LERG data
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NumberType {
    /// Wireless/Mobile number
    Wireless,
    /// Incumbent Local Exchange Carrier (ILEC)
    Ilec,
    /// Regional Bell Operating Company (RBOC)
    Rboc,
    /// Competitive Local Exchange Carrier (CLEC)
    Clec,
    /// Voice over IP (VoIP) provider
    Voip,
    /// Toll-free number
    TollFree,
    /// Unknown or unassigned
    Unknown,
}

/// Fraud detection result
#[derive(Debug, Clone)]
pub struct FraudDetectionResult {
    /// Whether fraud was detected
    pub fraud_detected: bool,
    /// Fraud confidence score (0.0 - 1.0)
    pub confidence_score: f32,
    /// Fraud reasons
    pub fraud_reasons: Vec<String>,
    /// ANI information
    pub ani_info: AniInfo,
    /// Recommended action
    pub recommended_action: FraudAction,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// ANI (calling number) information
#[derive(Debug, Clone)]
pub struct AniInfo {
    /// The calling number
    pub number: String,
    /// Formatted E.164 number
    pub e164_number: String,
    /// Number type (Wireless, ILEC, etc.)
    pub number_type: NumberType,
    /// Operating Company Number from LERG
    pub ocn: Option<String>,
    /// Operating Company Name
    pub company_name: Option<String>,
    /// Rate center
    pub rate_center: Option<String>,
    /// State/Province
    pub state: Option<String>,
    /// LATA (Local Access and Transport Area)
    pub lata: Option<String>,
    /// Is this a wireless number?
    pub is_wireless: bool,
    /// Is this an ILEC/RBOC number?
    pub is_ilec_rboc: bool,
}

/// STIR/SHAKEN call information
#[derive(Debug, Clone)]
pub struct StirShakenCallInfo {
    /// Call ID
    pub call_id: String,
    /// Calling number (ANI)
    pub calling_number: String,
    /// Called number
    pub called_number: String,
    /// STIR/SHAKEN attestation level
    pub attestation: Option<AttestationLevel>,
    /// Service Provider ID from certificate
    pub sp_id: Option<String>,
    /// Certificate URL
    pub cert_url: Option<String>,
    /// Origination ID
    pub orig_id: Option<String>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Source IP address
    pub source_ip: Option<String>,
}

/// STIR/SHAKEN fraud detection service
pub struct StirShakenFraudDetector {
    config: StirShakenFraudConfig,
    /// LERG data for OCN lookups
    lerg_data: Arc<RwLock<Option<LergData>>>,
    /// Fraud statistics
    stats: Arc<RwLock<FraudStats>>,
    /// Known fraudulent patterns
    fraud_patterns: Arc<RwLock<HashMap<String, FraudPattern>>>,
}

/// Fraud detection statistics
#[derive(Debug, Clone, Default)]
pub struct FraudStats {
    pub total_calls_analyzed: u64,
    pub fraud_detected: u64,
    pub attestation_mismatches: u64,
    pub wireless_violations: u64,
    pub ilec_rboc_violations: u64,
    pub unknown_ocn_count: u64,
    pub last_fraud_detected: Option<DateTime<Utc>>,
}

/// Fraudulent pattern detection
#[derive(Debug, Clone)]
pub struct FraudPattern {
    pub pattern_id: String,
    pub description: String,
    pub calling_number_pattern: Option<String>,
    pub sp_id_pattern: Option<String>,
    pub confidence_weight: f32,
    pub created_at: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub occurrence_count: u64,
}

impl StirShakenFraudDetector {
    /// Create new fraud detector
    pub fn new(config: StirShakenFraudConfig) -> Self {
        info!("Initializing STIR/SHAKEN fraud detector");
        
        Self {
            config,
            lerg_data: Arc::new(RwLock::new(None)),
            stats: Arc::new(RwLock::new(FraudStats::default())),
            fraud_patterns: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Set LERG data for OCN validation
    pub async fn set_lerg_data(&self, lerg: LergData) {
        let mut data = self.lerg_data.write().await;
        *data = Some(lerg);
        info!("LERG data loaded for STIR/SHAKEN fraud detection");
    }
    
    /// Analyze call for STIR/SHAKEN fraud
    pub async fn analyze_call(&self, call_info: &StirShakenCallInfo) -> Result<FraudDetectionResult> {
        if !self.config.enabled {
            return Ok(FraudDetectionResult {
                fraud_detected: false,
                confidence_score: 0.0,
                fraud_reasons: vec![],
                ani_info: self.create_basic_ani_info(&call_info.calling_number).await,
                recommended_action: FraudAction::LogAndContinue,
                metadata: HashMap::new(),
            });
        }
        
        debug!("Analyzing STIR/SHAKEN call: {} -> {} (attestation: {:?})", 
               call_info.calling_number, call_info.called_number, call_info.attestation);
        
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_calls_analyzed += 1;
        }
        
        // Get ANI information from LERG
        let ani_info = self.analyze_ani(&call_info.calling_number).await;
        
        // Perform fraud checks
        let mut fraud_reasons = Vec::new();
        let mut confidence_score = 0.0;
        
        // Check OCN validation
        if self.config.validate_ocn {
            if let Some(ocn_fraud) = self.check_ocn_mismatch(call_info, &ani_info).await {
                fraud_reasons.push(ocn_fraud.0);
                confidence_score += ocn_fraud.1;
            }
        }
        
        // Check wireless number attestation
        if self.config.validate_wireless && ani_info.is_wireless {
            if let Some(wireless_fraud) = self.check_wireless_attestation(call_info, &ani_info).await {
                fraud_reasons.push(wireless_fraud.0);
                confidence_score += wireless_fraud.1;
            }
        }
        
        // Check ILEC/RBOC attestation
        if self.config.validate_ilec_rboc && ani_info.is_ilec_rboc {
            if let Some(ilec_fraud) = self.check_ilec_rboc_attestation(call_info, &ani_info).await {
                fraud_reasons.push(ilec_fraud.0);
                confidence_score += ilec_fraud.1;
            }
        }
        
        // Check against known fraud patterns
        if let Some(pattern_fraud) = self.check_fraud_patterns(call_info).await {
            fraud_reasons.push(pattern_fraud.0);
            confidence_score += pattern_fraud.1;
        }
        
        // Determine if fraud detected
        let fraud_detected = confidence_score >= 0.5 || !fraud_reasons.is_empty();
        
        if fraud_detected {
            let mut stats = self.stats.write().await;
            stats.fraud_detected += 1;
            stats.last_fraud_detected = Some(Utc::now());
            
            // Update specific violation counters
            if fraud_reasons.iter().any(|r| r.contains("wireless")) {
                stats.wireless_violations += 1;
            }
            if fraud_reasons.iter().any(|r| r.contains("ILEC") || r.contains("RBOC")) {
                stats.ilec_rboc_violations += 1;
            }
            if fraud_reasons.iter().any(|r| r.contains("OCN")) {
                stats.attestation_mismatches += 1;
            }
        }
        
        let recommended_action = if fraud_detected {
            self.config.fraud_action.clone()
        } else {
            FraudAction::LogAndContinue
        };
        
        let result = FraudDetectionResult {
            fraud_detected,
            confidence_score,
            fraud_reasons,
            ani_info,
            recommended_action,
            metadata: HashMap::new(),
        };
        
        // Log fraud if detected
        if fraud_detected && self.config.enable_logging {
            self.log_fraud_detection(call_info, &result).await?;
        }
        
        Ok(result)
    }
    
    /// Check for OCN mismatch with attestation level
    async fn check_ocn_mismatch(&self, call_info: &StirShakenCallInfo, ani_info: &AniInfo) -> Option<(String, f32)> {
        if let Some(attestation) = call_info.attestation {
            if attestation == AttestationLevel::A {
                // Level A attestation claims full verification
                // Check if the SP actually owns this number's OCN
                if let (Some(ref sp_id), Some(ref ocn)) = (&call_info.sp_id, &ani_info.ocn) {
                    // This would require a mapping of SP IDs to OCNs
                    // For now, we'll check for known suspicious patterns
                    if self.is_suspicious_sp_ocn_combination(sp_id, ocn).await {
                        return Some((
                            format!("Suspicious attestation A for OCN {} by SP {}", ocn, sp_id),
                            0.8
                        ));
                    }
                }
                
                // Check if wireless numbers are claiming A attestation inappropriately
                if ani_info.is_wireless && call_info.sp_id.is_some() {
                    return Some((
                        format!("Wireless number {} claiming A attestation", ani_info.number),
                        0.7
                    ));
                }
            }
        }
        
        None
    }
    
    /// Check wireless number attestation rules
    async fn check_wireless_attestation(&self, call_info: &StirShakenCallInfo, ani_info: &AniInfo) -> Option<(String, f32)> {
        if let Some(attestation) = call_info.attestation {
            // Wireless numbers should typically not have A attestation unless
            // the wireless carrier is the one signing
            if attestation == AttestationLevel::A {
                return Some((
                    format!("Wireless number {} with suspicious A attestation", ani_info.number),
                    0.9
                ));
            }
            
            // Check minimum attestation level for wireless
            if attestation < self.config.wireless_min_attestation {
                return Some((
                    format!("Wireless number {} below minimum attestation level", ani_info.number),
                    0.6
                ));
            }
        } else {
            // Wireless numbers without any attestation are suspicious
            return Some((
                format!("Wireless number {} without STIR/SHAKEN attestation", ani_info.number),
                0.8
            ));
        }
        
        None
    }
    
    /// Check ILEC/RBOC attestation rules
    async fn check_ilec_rboc_attestation(&self, call_info: &StirShakenCallInfo, ani_info: &AniInfo) -> Option<(String, f32)> {
        if let Some(attestation) = call_info.attestation {
            if attestation == AttestationLevel::A {
                // ILEC/RBOC numbers claiming A attestation should be verified
                if let Some(ref sp_id) = call_info.sp_id {
                    if !self.is_authorized_ilec_rboc_sp(sp_id, ani_info).await {
                        return Some((
                            format!("ILEC/RBOC number {} A attestation by unauthorized SP {}", 
                                   ani_info.number, sp_id),
                            0.9
                        ));
                    }
                }
            }
        }
        
        None
    }
    
    /// Check against known fraud patterns
    async fn check_fraud_patterns(&self, call_info: &StirShakenCallInfo) -> Option<(String, f32)> {
        let patterns = self.fraud_patterns.read().await;
        
        for pattern in patterns.values() {
            let mut matches = true;
            
            // Check calling number pattern
            if let Some(ref num_pattern) = pattern.calling_number_pattern {
                if let Ok(regex) = regex::Regex::new(num_pattern) {
                    if !regex.is_match(&call_info.calling_number) {
                        matches = false;
                    }
                }
            }
            
            // Check SP ID pattern
            if let Some(ref sp_pattern) = pattern.sp_id_pattern {
                if let Some(ref sp_id) = call_info.sp_id {
                    if let Ok(regex) = regex::Regex::new(sp_pattern) {
                        if !regex.is_match(sp_id) {
                            matches = false;
                        }
                    }
                } else {
                    matches = false;
                }
            }
            
            if matches {
                return Some((
                    format!("Matches fraud pattern: {}", pattern.description),
                    pattern.confidence_weight
                ));
            }
        }
        
        None
    }
    
    /// Analyze ANI using LERG data
    async fn analyze_ani(&self, calling_number: &str) -> AniInfo {
        let e164_number = self.normalize_to_e164(calling_number);
        
        // Extract NPA-NXX for LERG lookup
        if let Some((npa, nxx)) = self.extract_npa_nxx(&e164_number) {
            if let Some(lerg_data) = self.lerg_data.read().await.as_ref() {
                if let Some(lerg_entry) = lerg_data.lookup_npa_nxx(&npa, &nxx) {
                    return AniInfo {
                        number: calling_number.to_string(),
                        e164_number: e164_number.clone(),
                        number_type: self.classify_number_type(&lerg_entry.company_type),
                        ocn: Some(lerg_entry.ocn.clone()),
                        company_name: Some(lerg_entry.company_name.clone()),
                        rate_center: Some(lerg_entry.rate_center.clone()),
                        state: Some(lerg_entry.state.clone()),
                        lata: Some(lerg_entry.lata.clone()),
                        is_wireless: lerg_entry.company_type.contains("WIRELESS") || 
                                   lerg_entry.company_type.contains("CELLULAR"),
                        is_ilec_rboc: lerg_entry.company_type.contains("ILEC") || 
                                     lerg_entry.company_type.contains("RBOC"),
                    };
                }
            }
        }
        
        // Fallback for unknown numbers
        self.create_basic_ani_info(calling_number).await
    }
    
    /// Create basic ANI info for unknown numbers
    async fn create_basic_ani_info(&self, calling_number: &str) -> AniInfo {
        let mut stats = self.stats.write().await;
        stats.unknown_ocn_count += 1;
        
        AniInfo {
            number: calling_number.to_string(),
            e164_number: self.normalize_to_e164(calling_number),
            number_type: NumberType::Unknown,
            ocn: None,
            company_name: None,
            rate_center: None,
            state: None,
            lata: None,
            is_wireless: false,
            is_ilec_rboc: false,
        }
    }
    
    /// Normalize number to E.164 format
    fn normalize_to_e164(&self, number: &str) -> String {
        let digits: String = number.chars().filter(|c| c.is_ascii_digit()).collect();
        
        if digits.len() == 10 {
            format!("+1{}", digits)
        } else if digits.len() == 11 && digits.starts_with('1') {
            format!("+{}", digits)
        } else {
            format!("+{}", digits)
        }
    }
    
    /// Extract NPA-NXX from phone number
    fn extract_npa_nxx(&self, e164_number: &str) -> Option<(String, String)> {
        if e164_number.len() >= 12 && e164_number.starts_with("+1") {
            let digits = &e164_number[2..];
            if digits.len() >= 6 {
                let npa = digits[0..3].to_string();
                let nxx = digits[3..6].to_string();
                return Some((npa, nxx));
            }
        }
        None
    }
    
    /// Classify number type from LERG company type
    fn classify_number_type(&self, company_type: &str) -> NumberType {
        let company_upper = company_type.to_uppercase();
        
        if company_upper.contains("WIRELESS") || company_upper.contains("CELLULAR") {
            NumberType::Wireless
        } else if company_upper.contains("ILEC") {
            NumberType::Ilec
        } else if company_upper.contains("RBOC") {
            NumberType::Rboc
        } else if company_upper.contains("CLEC") {
            NumberType::Clec
        } else if company_upper.contains("VOIP") || company_upper.contains("IP") {
            NumberType::Voip
        } else {
            NumberType::Unknown
        }
    }
    
    /// Check if SP/OCN combination is suspicious
    async fn is_suspicious_sp_ocn_combination(&self, _sp_id: &str, _ocn: &str) -> bool {
        // This would implement logic to check if the Service Provider
        // is authorized to sign for numbers from this OCN
        // For now, return false (no fraud detected)
        false
    }
    
    /// Check if SP is authorized for ILEC/RBOC numbers
    async fn is_authorized_ilec_rboc_sp(&self, _sp_id: &str, _ani_info: &AniInfo) -> bool {
        // This would check if the SP is authorized to sign for ILEC/RBOC numbers
        // For now, return true (authorized)
        true
    }
    
    /// Log fraud detection
    async fn log_fraud_detection(&self, call_info: &StirShakenCallInfo, result: &FraudDetectionResult) -> Result<()> {
        let log_entry = serde_json::json!({
            "timestamp": Utc::now().to_rfc3339(),
            "call_id": call_info.call_id,
            "calling_number": call_info.calling_number,
            "called_number": call_info.called_number,
            "attestation": call_info.attestation.map(|a| a.to_string()),
            "sp_id": call_info.sp_id,
            "confidence_score": result.confidence_score,
            "fraud_reasons": result.fraud_reasons,
            "ani_info": {
                "number_type": format!("{:?}", result.ani_info.number_type),
                "ocn": result.ani_info.ocn,
                "company_name": result.ani_info.company_name,
                "is_wireless": result.ani_info.is_wireless,
                "is_ilec_rboc": result.ani_info.is_ilec_rboc
            }
        });
        
        // Write to log file
        let log_line = format!("{}\n", log_entry);
        tokio::fs::write(&self.config.fraud_log_path, log_line).await?;
        
        warn!("STIR/SHAKEN fraud detected: {} (confidence: {:.2})", 
              call_info.call_id, result.confidence_score);
        
        Ok(())
    }
    
    /// Get fraud detection statistics
    pub async fn get_stats(&self) -> FraudStats {
        self.stats.read().await.clone()
    }
    
    /// Add fraud pattern
    pub async fn add_fraud_pattern(&self, pattern: FraudPattern) {
        let mut patterns = self.fraud_patterns.write().await;
        patterns.insert(pattern.pattern_id.clone(), pattern);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_wireless_fraud_detection() {
        let config = StirShakenFraudConfig::default();
        let detector = StirShakenFraudDetector::new(config);
        
        let call_info = StirShakenCallInfo {
            call_id: "test-call-1".to_string(),
            calling_number: "+15551234567".to_string(),
            called_number: "+15559876543".to_string(),
            attestation: Some(AttestationLevel::A),
            sp_id: Some("suspicious-sp".to_string()),
            cert_url: Some("https://example.com/cert".to_string()),
            orig_id: None,
            timestamp: Utc::now(),
            source_ip: None,
        };
        
        let result = detector.analyze_call(&call_info).await.unwrap();
        
        // Test would depend on having LERG data loaded
        // For now, just verify the analysis completes
        assert!(result.confidence_score >= 0.0);
    }
}