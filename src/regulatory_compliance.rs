use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};
use std::sync::Arc;

/// 2025 Regulatory Compliance Module for Voice and SMS
/// Implements FCC STIR/SHAKEN, TCPA, CAN-SPAM, and CRTC requirements

/// Voice call regulatory compliance requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceComplianceConfig {
    /// Enable STIR/SHAKEN authentication for all voice calls
    pub stir_shaken_enabled: bool,
    /// Enable robocall mitigation plan (RMP) compliance
    pub robocall_mitigation_enabled: bool,
    /// Enable intermediate provider compliance (post-2023 FCC rules)
    pub intermediate_provider_compliance: bool,
    /// Enable gateway provider STIR/SHAKEN (2022 FCC extension)
    pub gateway_provider_stir_shaken: bool,
    /// Maximum call attempts before auto-blocking (fraud prevention)
    pub max_call_attempts_per_hour: u32,
    /// Enable caller ID authentication verification
    pub caller_id_verification_enabled: bool,
    /// FCC Robocall Mitigation Database compliance
    pub robocall_database_compliance: bool,
}

/// SMS/Text messaging regulatory compliance requirements  
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsComplianceConfig {
    /// Enable TCPA compliance for all SMS traffic
    pub tcpa_compliance_enabled: bool,
    /// Enable A2P 10DLC registration compliance
    pub a2p_10dlc_compliance: bool,
    /// Enable one-to-one consent validation (April 2025 rule)
    pub one_to_one_consent_enabled: bool,
    /// Enable enhanced opt-out mechanisms (2025 update)
    pub enhanced_opt_out_enabled: bool,
    /// CAN-SPAM Act compliance for commercial SMS
    pub can_spam_compliance: bool,
    /// Maximum SMS per day per number (TCPA compliance)
    pub max_sms_per_day_per_number: u32,
    /// Time restriction enforcement (8 AM - 9 PM local time)
    pub time_restriction_enabled: bool,
    /// Do Not Call registry integration
    pub dnc_registry_integration: bool,
}

/// International regulatory compliance (Canada CRTC, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternationalComplianceConfig {
    /// Enable Canadian CRTC STIR/SHAKEN compliance
    pub crtc_stir_shaken_enabled: bool,
    /// CST-GA (Canadian Secure Token Governance Authority) integration
    pub cst_ga_integration: bool,
    /// Enable bi-annual CRTC compliance reporting
    pub crtc_reporting_enabled: bool,
    /// EU GDPR compliance for international calls/SMS
    pub gdpr_compliance_enabled: bool,
    /// Additional country-specific regulations
    pub country_specific_rules: HashMap<String, CountryRegulations>,
}

/// Country-specific regulatory requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountryRegulations {
    /// Country code (ISO 3166-1 alpha-2)
    pub country_code: String,
    /// STIR/SHAKEN or equivalent required
    pub call_authentication_required: bool,
    /// SMS consent requirements
    pub sms_consent_required: bool,
    /// Do Not Call registry required
    pub dnc_registry_required: bool,
    /// Maximum penalty per violation (in USD)
    pub max_penalty_per_violation: u32,
    /// Regulatory authority name
    pub regulatory_authority: String,
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
}

/// Comprehensive regulatory compliance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegulatoryComplianceConfig {
    /// Voice call compliance settings
    pub voice_compliance: VoiceComplianceConfig,
    /// SMS compliance settings
    pub sms_compliance: SmsComplianceConfig,
    /// International compliance settings
    pub international_compliance: InternationalComplianceConfig,
    /// Enable automatic compliance monitoring
    pub auto_monitoring_enabled: bool,
    /// Enable compliance reporting and alerts
    pub compliance_reporting_enabled: bool,
    /// Compliance violation penalty tracking
    pub penalty_tracking_enabled: bool,
}

/// Compliance violation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceViolation {
    /// Unique violation ID
    pub violation_id: String,
    /// Type of violation (TCPA, STIR_SHAKEN, etc.)
    pub violation_type: ViolationType,
    /// Severity level
    pub severity: ViolationSeverity,
    /// Description of the violation
    pub description: String,
    /// Phone number involved (if applicable)
    pub phone_number: Option<String>,
    /// Call ID or SMS ID involved
    pub message_id: Option<String>,
    /// Timestamp when violation occurred
    pub timestamp: DateTime<Utc>,
    /// Estimated penalty amount (USD)
    pub estimated_penalty: Option<u32>,
    /// Resolution status
    pub resolution_status: ResolutionStatus,
    /// Notes and remediation actions taken
    pub notes: Option<String>,
}

/// Types of regulatory violations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ViolationType {
    /// TCPA violation (SMS/Voice)
    TcpaViolation,
    /// STIR/SHAKEN authentication failure
    StirShakenViolation,
    /// CAN-SPAM Act violation
    CanSpamViolation,
    /// Do Not Call registry violation
    DncViolation,
    /// Robocall mitigation failure
    RobocallMitigationViolation,
    /// International regulation violation
    InternationalViolation,
    /// Time restriction violation
    TimeRestrictionViolation,
    /// Consent validation failure
    ConsentViolation,
    /// A2P 10DLC compliance violation
    A2p10dlcViolation,
}

/// Severity levels for violations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ViolationSeverity {
    /// Low severity - warning only
    Low,
    /// Medium severity - requires attention
    Medium,
    /// High severity - immediate action required
    High,
    /// Critical severity - service may be suspended
    Critical,
}

/// Resolution status for violations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ResolutionStatus {
    /// Violation detected but not yet addressed
    Open,
    /// Under investigation
    InProgress,
    /// Resolved and mitigated
    Resolved,
    /// Disputed with regulatory authority
    Disputed,
    /// Penalty paid and closed
    Closed,
}

/// Do Not Call registry entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DncEntry {
    /// Phone number in E.164 format
    pub phone_number: String,
    /// Date added to DNC registry
    pub date_added: DateTime<Utc>,
    /// Source of DNC entry (FTC, carrier, etc.)
    pub source: String,
    /// Expiration date (if applicable)
    pub expiration_date: Option<DateTime<Utc>>,
    /// Additional notes
    pub notes: Option<String>,
}

/// SMS consent record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsConsentRecord {
    /// Phone number in E.164 format
    pub phone_number: String,
    /// Brand or campaign that obtained consent
    pub brand_id: String,
    /// Campaign ID (A2P 10DLC)
    pub campaign_id: Option<String>,
    /// Consent obtained timestamp
    pub consent_timestamp: DateTime<Utc>,
    /// Consent method (web form, keyword, etc.)
    pub consent_method: String,
    /// Consent is active
    pub is_active: bool,
    /// Opt-out timestamp (if opted out)
    pub opt_out_timestamp: Option<DateTime<Utc>>,
    /// One-to-one consent compliance (April 2025)
    pub one_to_one_consent: bool,
    /// IP address where consent was obtained
    pub ip_address: Option<String>,
    /// User agent string
    pub user_agent: Option<String>,
}

/// Regulatory compliance service
pub struct RegulatoryComplianceService {
    config: RegulatoryComplianceConfig,
    violations: Arc<RwLock<Vec<ComplianceViolation>>>,
    dnc_registry: Arc<RwLock<HashMap<String, DncEntry>>>,
    sms_consents: Arc<RwLock<HashMap<String, Vec<SmsConsentRecord>>>>,
    call_counts: Arc<RwLock<HashMap<String, CallCounts>>>,
    sms_counts: Arc<RwLock<HashMap<String, SmsCounts>>>,
}

/// Call count tracking for compliance
#[derive(Debug, Clone)]
struct CallCounts {
    pub hourly_counts: Vec<(DateTime<Utc>, u32)>,
    pub daily_counts: Vec<(DateTime<Utc>, u32)>,
    pub total_calls: u64,
    pub violations: u32,
}

/// SMS count tracking for compliance
#[derive(Debug, Clone)]
struct SmsCounts {
    pub daily_counts: Vec<(DateTime<Utc>, u32)>,
    pub hourly_counts: Vec<(DateTime<Utc>, u32)>,
    pub total_sms: u64,
    pub violations: u32,
}

impl Default for RegulatoryComplianceConfig {
    fn default() -> Self {
        let mut country_rules = HashMap::new();
        
        // United States regulations (2025)
        country_rules.insert("US".to_string(), CountryRegulations {
            country_code: "US".to_string(),
            call_authentication_required: true,
            sms_consent_required: true,
            dnc_registry_required: true,
            max_penalty_per_violation: 53088, // Updated CAN-SPAM penalty 2025
            regulatory_authority: "FCC/FTC".to_string(),
            last_updated: Utc::now(),
        });
        
        // Canada regulations (2025)
        country_rules.insert("CA".to_string(), CountryRegulations {
            country_code: "CA".to_string(),
            call_authentication_required: true,
            sms_consent_required: true,
            dnc_registry_required: true,
            max_penalty_per_violation: 25000, // CAD converted to USD approximate
            regulatory_authority: "CRTC".to_string(),
            last_updated: Utc::now(),
        });

        RegulatoryComplianceConfig {
            voice_compliance: VoiceComplianceConfig {
                stir_shaken_enabled: true,
                robocall_mitigation_enabled: true,
                intermediate_provider_compliance: true, // 2023 FCC rule
                gateway_provider_stir_shaken: true, // 2022 FCC extension
                max_call_attempts_per_hour: 100,
                caller_id_verification_enabled: true,
                robocall_database_compliance: true,
            },
            sms_compliance: SmsComplianceConfig {
                tcpa_compliance_enabled: true,
                a2p_10dlc_compliance: true,
                one_to_one_consent_enabled: true, // April 2025 rule
                enhanced_opt_out_enabled: true, // 2025 update
                can_spam_compliance: true,
                max_sms_per_day_per_number: 10,
                time_restriction_enabled: true,
                dnc_registry_integration: true,
            },
            international_compliance: InternationalComplianceConfig {
                crtc_stir_shaken_enabled: true,
                cst_ga_integration: true,
                crtc_reporting_enabled: true,
                gdpr_compliance_enabled: true,
                country_specific_rules: country_rules,
            },
            auto_monitoring_enabled: true,
            compliance_reporting_enabled: true,
            penalty_tracking_enabled: true,
        }
    }
}

impl RegulatoryComplianceService {
    /// Create new regulatory compliance service
    pub fn new(config: RegulatoryComplianceConfig) -> Self {
        Self {
            config,
            violations: Arc::new(RwLock::new(Vec::new())),
            dnc_registry: Arc::new(RwLock::new(HashMap::new())),
            sms_consents: Arc::new(RwLock::new(HashMap::new())),
            call_counts: Arc::new(RwLock::new(HashMap::new())),
            sms_counts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Validate voice call compliance before allowing call
    /// Phone numbers should be without spaces, + prefix is optional
    pub async fn validate_voice_call(&self, 
        from_number: &str, 
        to_number: &str, 
        call_id: &str,
        has_stir_shaken: bool
    ) -> Result<bool> {
        // Clean phone numbers
        let clean_from = self.clean_phone_number(from_number);
        let clean_to = self.clean_phone_number(to_number);
        let mut violations = Vec::new();

        // Check STIR/SHAKEN compliance
        if self.config.voice_compliance.stir_shaken_enabled && !has_stir_shaken {
            violations.push(self.create_violation(
                ViolationType::StirShakenViolation,
                ViolationSeverity::High,
                format!("Call {} missing STIR/SHAKEN authentication", call_id),
                Some(clean_from.clone()),
                Some(call_id.to_string()),
            ));
        }

        // Check Do Not Call registry
        if self.config.voice_compliance.caller_id_verification_enabled {
            if self.is_on_dnc_registry(&clean_to).await? {
                violations.push(self.create_violation(
                    ViolationType::DncViolation,
                    ViolationSeverity::Critical,
                    format!("Call to DNC registered number {}", clean_to),
                    Some(clean_from.clone()),
                    Some(call_id.to_string()),
                ));
            }
        }

        // Check call rate limits
        if let Err(e) = self.check_call_rate_limits(&clean_from).await {
            violations.push(self.create_violation(
                ViolationType::RobocallMitigationViolation,
                ViolationSeverity::Medium,
                format!("Call rate limit exceeded: {}", e),
                Some(clean_from.clone()),
                Some(call_id.to_string()),
            ));
        }

        // Record violations
        if !violations.is_empty() {
            let mut violation_store = self.violations.write().await;
            violation_store.extend(violations.clone());
            
            // Log critical violations
            for violation in &violations {
                if violation.severity == ViolationSeverity::Critical {
                    error!("Critical regulatory violation: {}", violation.description);
                    return Ok(false); // Block call
                }
            }
        }

        // Increment call count
        self.increment_call_count(&clean_from).await;

        Ok(true) // Allow call
    }

    /// Validate SMS compliance before sending
    /// Phone numbers should be without spaces, + prefix is optional
    pub async fn validate_sms(&self, 
        from_number: &str, 
        to_number: &str, 
        message_id: &str,
        brand_id: &str,
        campaign_id: Option<&str>
    ) -> Result<bool> {
        // Clean phone numbers
        let clean_from = self.clean_phone_number(from_number);
        let clean_to = self.clean_phone_number(to_number);
        let mut violations = Vec::new();

        // Check TCPA consent requirements
        if self.config.sms_compliance.tcpa_compliance_enabled {
            if !self.has_valid_sms_consent(&clean_to, brand_id, campaign_id).await? {
                violations.push(self.create_violation(
                    ViolationType::TcpaViolation,
                    ViolationSeverity::Critical,
                    format!("SMS to {} without valid TCPA consent", clean_to),
                    Some(clean_from.clone()),
                    Some(message_id.to_string()),
                ));
            }
        }

        // Check one-to-one consent (April 2025 rule)
        if self.config.sms_compliance.one_to_one_consent_enabled {
            if !self.has_one_to_one_consent(&clean_to, brand_id).await? {
                violations.push(self.create_violation(
                    ViolationType::ConsentViolation,
                    ViolationSeverity::High,
                    format!("SMS lacks one-to-one consent required by April 2025 TCPA rule"),
                    Some(clean_from.clone()),
                    Some(message_id.to_string()),
                ));
            }
        }

        // Check time restrictions (8 AM - 9 PM local time)
        if self.config.sms_compliance.time_restriction_enabled {
            if self.is_outside_allowed_hours(&clean_to).await? {
                violations.push(self.create_violation(
                    ViolationType::TimeRestrictionViolation,
                    ViolationSeverity::Medium,
                    format!("SMS sent outside allowed hours (8 AM - 9 PM local time)"),
                    Some(clean_from.clone()),
                    Some(message_id.to_string()),
                ));
            }
        }

        // Check SMS rate limits
        if let Err(e) = self.check_sms_rate_limits(&clean_from, &clean_to).await {
            violations.push(self.create_violation(
                ViolationType::TcpaViolation,
                ViolationSeverity::Medium,
                format!("SMS rate limit exceeded: {}", e),
                Some(clean_from.clone()),
                Some(message_id.to_string()),
            ));
        }

        // Check Do Not Call registry for SMS
        if self.config.sms_compliance.dnc_registry_integration {
            if self.is_on_dnc_registry(&clean_to).await? {
                violations.push(self.create_violation(
                    ViolationType::DncViolation,
                    ViolationSeverity::Critical,
                    format!("SMS to DNC registered number {}", clean_to),
                    Some(clean_from.clone()),
                    Some(message_id.to_string()),
                ));
            }
        }

        // Record violations
        if !violations.is_empty() {
            let mut violation_store = self.violations.write().await;
            violation_store.extend(violations.clone());
            
            // Block SMS for critical violations
            for violation in &violations {
                if violation.severity == ViolationSeverity::Critical {
                    error!("Critical SMS regulatory violation: {}", violation.description);
                    return Ok(false); // Block SMS
                }
            }
        }

        // Increment SMS count
        self.increment_sms_count(&clean_from, &clean_to).await;

        Ok(true) // Allow SMS
    }

    /// Record SMS consent (for opt-ins)
    pub async fn record_sms_consent(&self, 
        phone_number: &str,
        brand_id: &str,
        campaign_id: Option<&str>,
        consent_method: &str,
        one_to_one_consent: bool,
        ip_address: Option<&str>,
        user_agent: Option<&str>
    ) -> Result<()> {
        let consent_record = SmsConsentRecord {
            phone_number: phone_number.to_string(),
            brand_id: brand_id.to_string(),
            campaign_id: campaign_id.map(|s| s.to_string()),
            consent_timestamp: Utc::now(),
            consent_method: consent_method.to_string(),
            is_active: true,
            opt_out_timestamp: None,
            one_to_one_consent,
            ip_address: ip_address.map(|s| s.to_string()),
            user_agent: user_agent.map(|s| s.to_string()),
        };

        let mut consents = self.sms_consents.write().await;
        consents.entry(phone_number.to_string())
            .or_insert_with(Vec::new)
            .push(consent_record);

        info!("Recorded SMS consent for {} with brand {}", phone_number, brand_id);
        Ok(())
    }

    /// Process SMS opt-out
    pub async fn process_sms_opt_out(&self, phone_number: &str, brand_id: Option<&str>) -> Result<()> {
        let mut consents = self.sms_consents.write().await;
        
        if let Some(phone_consents) = consents.get_mut(phone_number) {
            let now = Utc::now();
            
            // If brand_id specified, opt out from that brand only
            if let Some(brand) = brand_id {
                for consent in phone_consents.iter_mut() {
                    if consent.brand_id == brand && consent.is_active {
                        consent.is_active = false;
                        consent.opt_out_timestamp = Some(now);
                    }
                }
            } else {
                // Opt out from all brands (global opt-out)
                for consent in phone_consents.iter_mut() {
                    if consent.is_active {
                        consent.is_active = false;
                        consent.opt_out_timestamp = Some(now);
                    }
                }
            }
        }

        info!("Processed SMS opt-out for {} (brand: {:?})", phone_number, brand_id);
        Ok(())
    }

    /// Add number to Do Not Call registry
    pub async fn add_to_dnc_registry(&self, 
        phone_number: &str, 
        source: &str,
        expiration_date: Option<DateTime<Utc>>,
        notes: Option<&str>
    ) -> Result<()> {
        let dnc_entry = DncEntry {
            phone_number: phone_number.to_string(),
            date_added: Utc::now(),
            source: source.to_string(),
            expiration_date,
            notes: notes.map(|s| s.to_string()),
        };

        let mut dnc_registry = self.dnc_registry.write().await;
        dnc_registry.insert(phone_number.to_string(), dnc_entry);

        info!("Added {} to DNC registry (source: {})", phone_number, source);
        Ok(())
    }

    /// Check if number is on Do Not Call registry
    async fn is_on_dnc_registry(&self, phone_number: &str) -> Result<bool> {
        let dnc_registry = self.dnc_registry.read().await;
        
        if let Some(entry) = dnc_registry.get(phone_number) {
            // Check if entry has expired
            if let Some(expiration) = entry.expiration_date {
                if Utc::now() > expiration {
                    return Ok(false); // Expired entry
                }
            }
            return Ok(true);
        }
        
        Ok(false)
    }

    /// Check if phone number has valid SMS consent
    async fn has_valid_sms_consent(&self, 
        phone_number: &str, 
        brand_id: &str,
        _campaign_id: Option<&str>
    ) -> Result<bool> {
        let consents = self.sms_consents.read().await;
        
        if let Some(phone_consents) = consents.get(phone_number) {
            for consent in phone_consents {
                if consent.brand_id == brand_id && consent.is_active {
                    return Ok(true);
                }
            }
        }
        
        Ok(false)
    }

    /// Check one-to-one consent compliance (April 2025 TCPA rule)
    async fn has_one_to_one_consent(&self, phone_number: &str, brand_id: &str) -> Result<bool> {
        let consents = self.sms_consents.read().await;
        
        if let Some(phone_consents) = consents.get(phone_number) {
            for consent in phone_consents {
                if consent.brand_id == brand_id && consent.is_active && consent.one_to_one_consent {
                    return Ok(true);
                }
            }
        }
        
        Ok(false)
    }

    /// Check if SMS is being sent outside allowed hours
    async fn is_outside_allowed_hours(&self, _phone_number: &str) -> Result<bool> {
        // TODO: Implement timezone detection and local time checking
        // For now, return false (assume within allowed hours)
        // Real implementation would:
        // 1. Detect timezone from phone number
        // 2. Convert current UTC time to local time
        // 3. Check if between 8 AM and 9 PM local time
        Ok(false)
    }

    /// Check call rate limits for robocall mitigation
    async fn check_call_rate_limits(&self, from_number: &str) -> Result<()> {
        let call_counts = self.call_counts.read().await;
        
        if let Some(counts) = call_counts.get(from_number) {
            let now = Utc::now();
            let one_hour_ago = now - chrono::Duration::hours(1);
            
            // Count calls in the last hour
            let hourly_count = counts.hourly_counts.iter()
                .filter(|(timestamp, _)| *timestamp > one_hour_ago)
                .map(|(_, count)| count)
                .sum::<u32>();
                
            if hourly_count >= self.config.voice_compliance.max_call_attempts_per_hour {
                return Err(anyhow!("Hourly call limit exceeded: {} calls", hourly_count));
            }
        }
        
        Ok(())
    }

    /// Check SMS rate limits for TCPA compliance
    async fn check_sms_rate_limits(&self, from_number: &str, to_number: &str) -> Result<()> {
        let sms_counts = self.sms_counts.read().await;
        let key = format!("{}:{}", from_number, to_number);
        
        if let Some(counts) = sms_counts.get(&key) {
            let now = Utc::now();
            let one_day_ago = now - chrono::Duration::days(1);
            
            // Count SMS in the last day to this specific number
            let daily_count = counts.daily_counts.iter()
                .filter(|(timestamp, _)| *timestamp > one_day_ago)
                .map(|(_, count)| count)
                .sum::<u32>();
                
            if daily_count >= self.config.sms_compliance.max_sms_per_day_per_number {
                return Err(anyhow!("Daily SMS limit exceeded to {}: {} messages", to_number, daily_count));
            }
        }
        
        Ok(())
    }

    /// Increment call count for tracking
    async fn increment_call_count(&self, from_number: &str) {
        let mut call_counts = self.call_counts.write().await;
        let now = Utc::now();
        
        let counts = call_counts.entry(from_number.to_string()).or_insert(CallCounts {
            hourly_counts: Vec::new(),
            daily_counts: Vec::new(),
            total_calls: 0,
            violations: 0,
        });
        
        counts.total_calls += 1;
        counts.hourly_counts.push((now, 1));
        counts.daily_counts.push((now, 1));
        
        // Clean up old entries (keep last 24 hours)
        let one_day_ago = now - chrono::Duration::days(1);
        counts.hourly_counts.retain(|(timestamp, _)| *timestamp > one_day_ago);
        counts.daily_counts.retain(|(timestamp, _)| *timestamp > one_day_ago);
    }

    /// Increment SMS count for tracking
    async fn increment_sms_count(&self, from_number: &str, to_number: &str) {
        let mut sms_counts = self.sms_counts.write().await;
        let now = Utc::now();
        let key = format!("{}:{}", from_number, to_number);
        
        let counts = sms_counts.entry(key).or_insert(SmsCounts {
            daily_counts: Vec::new(),
            hourly_counts: Vec::new(),
            total_sms: 0,
            violations: 0,
        });
        
        counts.total_sms += 1;
        counts.hourly_counts.push((now, 1));
        counts.daily_counts.push((now, 1));
        
        // Clean up old entries (keep last 7 days)
        let one_week_ago = now - chrono::Duration::days(7);
        counts.hourly_counts.retain(|(timestamp, _)| *timestamp > one_week_ago);
        counts.daily_counts.retain(|(timestamp, _)| *timestamp > one_week_ago);
    }

    /// Create a compliance violation record
    fn create_violation(&self,
        violation_type: ViolationType,
        severity: ViolationSeverity,
        description: String,
        phone_number: Option<String>,
        message_id: Option<String>,
    ) -> ComplianceViolation {
        use uuid::Uuid;
        
        let estimated_penalty = match (&violation_type, &severity) {
            (ViolationType::TcpaViolation, ViolationSeverity::Critical) => Some(1500),
            (ViolationType::TcpaViolation, _) => Some(500),
            (ViolationType::CanSpamViolation, _) => Some(53088), // 2025 penalty
            (ViolationType::DncViolation, _) => Some(500),
            _ => Some(100),
        };

        ComplianceViolation {
            violation_id: Uuid::new_v4().to_string(),
            violation_type,
            severity,
            description,
            phone_number,
            message_id,
            timestamp: Utc::now(),
            estimated_penalty,
            resolution_status: ResolutionStatus::Open,
            notes: None,
        }
    }

    /// Get compliance statistics
    pub async fn get_compliance_stats(&self) -> ComplianceStats {
        let violations = self.violations.read().await;
        let dnc_count = self.dnc_registry.read().await.len();
        let consents_count = self.sms_consents.read().await.len();
        
        let total_violations = violations.len();
        let critical_violations = violations.iter()
            .filter(|v| v.severity == ViolationSeverity::Critical)
            .count();
        let open_violations = violations.iter()
            .filter(|v| v.resolution_status == ResolutionStatus::Open)
            .count();
        
        let total_estimated_penalties = violations.iter()
            .filter_map(|v| v.estimated_penalty)
            .sum::<u32>();

        ComplianceStats {
            total_violations,
            critical_violations,
            open_violations,
            total_estimated_penalties,
            dnc_registry_count: dnc_count,
            active_sms_consents: consents_count,
            compliance_score: calculate_compliance_score(total_violations, critical_violations),
        }
    }

    /// Export compliance report
    pub async fn export_compliance_report(&self, format: ReportFormat) -> Result<String> {
        let stats = self.get_compliance_stats().await;
        let violations = self.violations.read().await;
        
        match format {
            ReportFormat::Json => {
                let report = ComplianceReport {
                    generated_at: Utc::now(),
                    stats,
                    violations: violations.clone(),
                };
                Ok(serde_json::to_string_pretty(&report)?)
            }
            ReportFormat::Csv => {
                let mut csv = String::from("violation_id,type,severity,description,phone_number,timestamp,penalty\n");
                for violation in violations.iter() {
                    csv.push_str(&format!(
                        "{},{:?},{:?},{},{},{},{}\n",
                        violation.violation_id,
                        violation.violation_type,
                        violation.severity,
                        violation.description.replace(',', ";"),
                        violation.phone_number.as_deref().unwrap_or(""),
                        violation.timestamp.format("%Y-%m-%d %H:%M:%S"),
                        violation.estimated_penalty.unwrap_or(0)
                    ));
                }
                Ok(csv)
            }
        }
    }

    /// Clean phone number by removing + prefix, spaces, and formatting
    /// Returns normalized phone number with digits only
    fn clean_phone_number(&self, phone_number: &str) -> String {
        phone_number
            .trim_start_matches('+')
            .replace(['-', ' ', '(', ')', '.'], "")
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect()
    }
}

/// Compliance statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceStats {
    pub total_violations: usize,
    pub critical_violations: usize,
    pub open_violations: usize,
    pub total_estimated_penalties: u32,
    pub dnc_registry_count: usize,
    pub active_sms_consents: usize,
    pub compliance_score: f32, // 0.0 to 100.0
}

/// Compliance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub generated_at: DateTime<Utc>,
    pub stats: ComplianceStats,
    pub violations: Vec<ComplianceViolation>,
}

/// Report formats
#[derive(Debug, Clone)]
pub enum ReportFormat {
    Json,
    Csv,
}

/// Calculate compliance score (0-100)
fn calculate_compliance_score(total_violations: usize, critical_violations: usize) -> f32 {
    if total_violations == 0 {
        return 100.0;
    }
    
    let base_score = 100.0 - (total_violations as f32 * 5.0).min(50.0);
    let critical_penalty = critical_violations as f32 * 20.0;
    
    (base_score - critical_penalty).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_voice_call_validation() {
        let config = RegulatoryComplianceConfig::default();
        let service = RegulatoryComplianceService::new(config);
        
        // Test valid call with STIR/SHAKEN
        let result = service.validate_voice_call(
            "+12125551234",
            "+13105554321",
            "test-call-id",
            true
        ).await.unwrap();
        
        assert!(result); // Should allow call
        
        // Test call without STIR/SHAKEN
        let result = service.validate_voice_call(
            "+12125551234",
            "+13105554321",
            "test-call-id-2",
            false
        ).await.unwrap();
        
        // Should still allow but record violation
        assert!(result);
        let violations = service.violations.read().await;
        assert!(!violations.is_empty());
    }

    #[tokio::test]
    async fn test_sms_consent_validation() {
        let config = RegulatoryComplianceConfig::default();
        let service = RegulatoryComplianceService::new(config);
        
        // Record consent first
        service.record_sms_consent(
            "+12125551234",
            "test-brand",
            Some("test-campaign"),
            "web-form",
            true, // one-to-one consent
            Some("192.168.1.1"),
            Some("Mozilla/5.0...")
        ).await.unwrap();
        
        // Test SMS with valid consent
        let result = service.validate_sms(
            "+15551234567",
            "+12125551234",
            "test-sms-id",
            "test-brand",
            Some("test-campaign")
        ).await.unwrap();
        
        assert!(result); // Should allow SMS
        
        // Test SMS without consent
        let result = service.validate_sms(
            "+15551234567",
            "+19999999999",
            "test-sms-id-2",
            "test-brand",
            Some("test-campaign")
        ).await.unwrap();
        
        assert!(!result); // Should block SMS due to missing consent
    }

    #[tokio::test]
    async fn test_dnc_registry() {
        let config = RegulatoryComplianceConfig::default();
        let service = RegulatoryComplianceService::new(config);
        
        // Add number to DNC registry
        service.add_to_dnc_registry(
            "+12125551234",
            "FTC",
            None,
            Some("Consumer request")
        ).await.unwrap();
        
        // Check if number is on DNC registry
        let is_dnc = service.is_on_dnc_registry("+12125551234").await.unwrap();
        assert!(is_dnc);
        
        // Check number not on DNC registry
        let is_dnc = service.is_on_dnc_registry("+19999999999").await.unwrap();
        assert!(!is_dnc);
    }

    #[test]
    fn test_compliance_score_calculation() {
        assert_eq!(calculate_compliance_score(0, 0), 100.0);
        assert_eq!(calculate_compliance_score(5, 0), 75.0);
        assert_eq!(calculate_compliance_score(5, 1), 55.0);
        assert_eq!(calculate_compliance_score(20, 5), 0.0);
    }
}