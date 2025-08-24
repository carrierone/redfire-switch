/*
 * J-STD-025 U.S. Lawful Intercept Implementation
 * ANSI-41 GSM and ANSI-136 GSM Compatibility Standard
 *
 * This module implements J-STD-025 compliant lawful intercept for U.S. jurisdiction,
 * including call detail records for billing, accounting, and regulatory compliance.
 *
 * Standards Compliance:
 * - J-STD-025: U.S. Lawful Intercept Standard (CALEA compliance)
 * - ANSI-41 GSM and ANSI-136 GSM Compatibility
 * - ATIS-0300025: Call Detail Recording Format
 * - Telcordia GR-1100: Billing Requirements
 * - CALEA: Communications Assistance for Law Enforcement Act
 */

use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::net::IpAddr;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// J-STD-025 Call Detail Record Types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CdrType {
    /// Mobile Originated Call
    MOC,
    /// Mobile Terminated Call
    MTC,
    /// Mobile to Mobile Call
    MMC,
    /// Short Message Service
    SMS,
    /// Supplementary Service
    SS,
    /// Location Update
    LU,
    /// Handoff
    HO,
    /// Emergency Call
    Emergency,
    /// Roaming Call
    Roaming,
    /// Conference Call
    Conference,
}

/// Call Result Codes per J-STD-025
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallResult {
    /// Call completed normally
    Normal = 0,
    /// Busy subscriber
    Busy = 1,
    /// No answer
    NoAnswer = 2,
    /// Network congestion
    Congestion = 3,
    /// Invalid number
    InvalidNumber = 4,
    /// Restricted destination
    Restricted = 5,
    /// Service not available
    ServiceUnavailable = 6,
    /// System failure
    SystemFailure = 7,
    /// Call forwarded
    Forwarded = 8,
    /// Call transferred
    Transferred = 9,
    /// Subscriber absent
    SubscriberAbsent = 10,
    /// Authentication failure
    AuthFailure = 11,
    /// Credit limit exceeded
    CreditLimitExceeded = 12,
    /// Fraud detected
    FraudDetected = 13,
}

/// Service Type Classifications
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceType {
    /// Basic voice service
    Voice,
    /// Data service
    Data,
    /// Fax service
    Fax,
    /// Video call
    Video,
    /// SMS service
    SMS,
    /// MMS service
    MMS,
    /// Emergency service
    Emergency,
    /// Premium service
    Premium,
    /// International service
    International,
    /// Roaming service
    Roaming,
}

/// Charging Information Structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChargingInfo {
    /// Tariff class
    pub tariff_class: String,
    /// Rate per minute (in currency units)
    pub rate_per_minute: f64,
    /// Setup charge
    pub setup_charge: f64,
    /// Total charge for the call
    pub total_charge: f64,
    /// Currency code (ISO 4217)
    pub currency_code: String,
    /// Billing increment (seconds)
    pub billing_increment: u32,
    /// Free units used
    pub free_units_used: u32,
    /// Discount applied
    pub discount_percentage: f32,
}

/// Location Information per J-STD-025
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationInfo {
    /// Cell ID
    pub cell_id: Option<String>,
    /// Location Area Code
    pub lac: Option<String>,
    /// Mobile Country Code
    pub mcc: Option<String>,
    /// Mobile Network Code
    pub mnc: Option<String>,
    /// Serving MSC address
    pub serving_msc: Option<IpAddr>,
    /// Location coordinates (if available)
    pub coordinates: Option<(f64, f64)>,
    /// Location timestamp
    pub location_timestamp: DateTime<Utc>,
}

/// Quality of Service Metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QoSMetrics {
    /// Speech quality score (MOS)
    pub mos_score: Option<f32>,
    /// Packet loss percentage
    pub packet_loss: Option<f32>,
    /// Jitter (ms)
    pub jitter: Option<f32>,
    /// Round-trip delay (ms)
    pub rtt: Option<f32>,
    /// Codec used
    pub codec: Option<String>,
    /// Bit rate (kbps)
    pub bit_rate: Option<u32>,
}

/// J-STD-025 Compliant Call Detail Record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JStd025Cdr {
    /// Unique record identifier
    pub record_id: Uuid,
    /// CDR type
    pub cdr_type: CdrType,
    /// Call start time (UTC)
    pub call_start_time: DateTime<Utc>,
    /// Call answer time (UTC)
    pub call_answer_time: Option<DateTime<Utc>>,
    /// Call end time (UTC)
    pub call_end_time: Option<DateTime<Utc>>,
    /// Call duration in seconds
    pub call_duration: Option<u64>,
    /// Billable duration in seconds
    pub billable_duration: Option<u64>,

    // Subscriber Information
    /// Calling party number (A-number)
    pub calling_number: String,
    /// Called party number (B-number)
    pub called_number: String,
    /// Original called number (before translation)
    pub original_called_number: Option<String>,
    /// International Mobile Subscriber Identity
    pub imsi: Option<String>,
    /// International Mobile Equipment Identity
    pub imei: Option<String>,
    /// Mobile Station ISDN Number
    pub msisdn: Option<String>,

    // Network Information
    /// Originating switch ID
    pub originating_switch: String,
    /// Terminating switch ID
    pub terminating_switch: Option<String>,
    /// Trunk group ID
    pub trunk_group_id: Option<String>,
    /// Circuit identification code
    pub circuit_id: Option<String>,

    // Call Classification
    /// Service type
    pub service_type: ServiceType,
    /// Call result
    pub call_result: CallResult,
    /// Call direction (0=outgoing, 1=incoming, 2=transit)
    pub call_direction: u8,

    // Billing Information
    /// Charging information
    pub charging_info: Option<ChargingInfo>,
    /// Account code
    pub account_code: Option<String>,
    /// Customer ID
    pub customer_id: Option<String>,

    // Location Information
    /// Originating location
    pub originating_location: Option<LocationInfo>,
    /// Terminating location
    pub terminating_location: Option<LocationInfo>,

    // Quality Metrics
    /// Quality of service metrics
    pub qos_metrics: Option<QoSMetrics>,

    // Supplementary Services
    /// Call forwarding indicator
    pub call_forwarding: bool,
    /// Call waiting indicator
    pub call_waiting: bool,
    /// Conference call indicator
    pub conference_call: bool,
    /// Three-way calling indicator
    pub three_way_calling: bool,

    // Fraud and Security
    /// Fraud flags
    pub fraud_flags: Vec<String>,
    /// Authentication result
    pub auth_result: Option<String>,
    /// STIR/SHAKEN verification status
    pub stir_shaken_status: Option<String>,

    // J-STD-025 Lawful Intercept (CALEA Compliance)
    /// Warrant IDs for U.S. lawful intercept under CALEA
    pub warrant_ids: Vec<Uuid>,
    /// LEA (Law Enforcement Agency) identifiers  
    pub lea_identifiers: Vec<String>,
    /// CALEA intercept type (content/non-content)
    pub calea_intercept_type: Option<String>,
    /// Intercept priority level
    pub intercept_priority: Option<u8>,

    // Additional Fields
    /// Custom attributes for operator-specific needs
    pub custom_attributes: HashMap<String, String>,
    /// Record generation timestamp
    pub record_timestamp: DateTime<Utc>,
    /// Record version
    pub record_version: String,
}

impl JStd025Cdr {
    /// Create a new CDR record
    pub fn new(cdr_type: CdrType, calling_number: String, called_number: String) -> Self {
        Self {
            record_id: Uuid::new_v4(),
            cdr_type,
            call_start_time: Utc::now(),
            call_answer_time: None,
            call_end_time: None,
            call_duration: None,
            billable_duration: None,
            calling_number,
            called_number,
            original_called_number: None,
            imsi: None,
            imei: None,
            msisdn: None,
            originating_switch: "redfire-switch".to_string(),
            terminating_switch: None,
            trunk_group_id: None,
            circuit_id: None,
            service_type: ServiceType::Voice,
            call_result: CallResult::Normal,
            call_direction: 0,
            charging_info: None,
            account_code: None,
            customer_id: None,
            originating_location: None,
            terminating_location: None,
            qos_metrics: None,
            call_forwarding: false,
            call_waiting: false,
            conference_call: false,
            three_way_calling: false,
            fraud_flags: Vec::new(),
            auth_result: None,
            stir_shaken_status: None,
            // J-STD-025 Lawful Intercept fields (CALEA compliance)
            warrant_ids: Vec::new(),
            lea_identifiers: Vec::new(),
            calea_intercept_type: None,
            intercept_priority: None,
            custom_attributes: HashMap::new(),
            record_timestamp: Utc::now(),
            record_version: "J-STD-025-1.0".to_string(),
        }
    }

    /// Mark call as answered
    pub fn mark_answered(&mut self) {
        self.call_answer_time = Some(Utc::now());
    }

    /// Mark call as ended and calculate duration
    pub fn mark_ended(&mut self, result: CallResult) {
        let end_time = Utc::now();
        self.call_end_time = Some(end_time);
        self.call_result = result;

        // Calculate total duration with overflow protection
        let total_duration_seconds = (end_time - self.call_start_time).num_seconds();
        self.call_duration = Some(if total_duration_seconds < 0 {
            warn!("Negative call duration detected, setting to 0");
            0
        } else {
            total_duration_seconds as u64
        });

        // Calculate billable duration (from answer to end, or 0 if never answered)
        if let Some(answer_time) = self.call_answer_time {
            let billable_duration_seconds = (end_time - answer_time).num_seconds();
            self.billable_duration = Some(if billable_duration_seconds < 0 {
                warn!("Negative billable duration detected, setting to 0");
                0
            } else {
                billable_duration_seconds as u64
            });
        } else {
            // Call was never answered, billable duration is 0
            self.billable_duration = Some(0);
        }
    }

    /// Add fraud flag
    pub fn add_fraud_flag(&mut self, flag: String) {
        if !self.fraud_flags.contains(&flag) {
            self.fraud_flags.push(flag);
        }
    }

    /// Set charging information
    pub fn set_charging_info(&mut self, charging_info: ChargingInfo) {
        self.charging_info = Some(charging_info);
    }

    /// Set quality metrics
    pub fn set_qos_metrics(&mut self, qos_metrics: QoSMetrics) {
        self.qos_metrics = Some(qos_metrics);
    }

    /// Export as TAP3 format (simplified)
    pub fn to_tap3_format(&self) -> Result<String> {
        // Simplified TAP3 record generation
        // In production, this would generate proper ASN.1 encoded TAP3
        let mut tap3_record = HashMap::new();

        tap3_record.insert("recordType".to_string(), "CallEventDetail".to_string());
        tap3_record.insert("recordId".to_string(), self.record_id.to_string());
        tap3_record.insert(
            "callStartTime".to_string(),
            self.call_start_time.to_rfc3339(),
        );
        tap3_record.insert("callingNumber".to_string(), self.calling_number.clone());
        tap3_record.insert("calledNumber".to_string(), self.called_number.clone());

        if let Some(duration) = self.billable_duration {
            tap3_record.insert("billableDuration".to_string(), duration.to_string());
        }

        if let Some(ref charging) = self.charging_info {
            tap3_record.insert("totalCharge".to_string(), charging.total_charge.to_string());
            tap3_record.insert("currency".to_string(), charging.currency_code.clone());
        }

        serde_json::to_string(&tap3_record)
            .map_err(|e| anyhow!("Failed to serialize TAP3 record: {}", e))
    }

    /// Export as CIBER format
    pub fn to_ciber_format(&self) -> Result<String> {
        // CIBER (Common IXC Billing Exchange Roamer) format
        let mut ciber_fields = Vec::new();

        // Record type
        ciber_fields.push(format!("{:02}", self.cdr_type as u8));

        // Call start time (YYYYMMDDHHMISS)
        ciber_fields.push(self.call_start_time.format("%Y%m%d%H%M%S").to_string());

        // Calling number
        ciber_fields.push(format!("{:15}", self.calling_number));

        // Called number
        ciber_fields.push(format!("{:15}", self.called_number));

        // Duration (seconds)
        ciber_fields.push(format!("{:08}", self.billable_duration.unwrap_or(0)));

        // Call result
        ciber_fields.push(format!("{:02}", self.call_result as u8));

        // Originating switch
        ciber_fields.push(format!("{:20}", self.originating_switch));

        // Charge amount (in cents)
        let charge_cents = self
            .charging_info
            .as_ref()
            .map(|c| (c.total_charge * 100.0) as u64)
            .unwrap_or(0);
        ciber_fields.push(format!("{:010}", charge_cents));

        Ok(ciber_fields.join("|"))
    }

    /// Validate CDR completeness for billing
    pub fn validate_for_billing(&self) -> Result<()> {
        if self.calling_number.is_empty() {
            return Err(anyhow!("Calling number is required"));
        }

        if self.called_number.is_empty() {
            return Err(anyhow!("Called number is required"));
        }

        if self.call_end_time.is_none() {
            return Err(anyhow!("Call end time is required for billing"));
        }

        if self.call_result == CallResult::Normal && self.billable_duration.is_none() {
            return Err(anyhow!("Billable duration is required for completed calls"));
        }

        if self.charging_info.is_none() && self.call_result == CallResult::Normal {
            warn!("No charging information available for billable call");
        }

        Ok(())
    }
}

impl fmt::Display for JStd025Cdr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CDR[{}]: {} -> {} ({:?}) Duration: {}s Result: {:?}",
            self.record_id,
            self.calling_number,
            self.called_number,
            self.service_type,
            self.billable_duration.unwrap_or(0),
            self.call_result
        )
    }
}

/// J-STD-025 CDR Processing Engine
pub struct JStd025CdrEngine {
    /// Active CDR records
    active_cdrs: HashMap<String, JStd025Cdr>,
    /// CDR storage backend
    storage: Box<dyn CdrStorage + Send + Sync>,
    /// Configuration
    config: CdrEngineConfig,
}

/// CDR Engine Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdrEngineConfig {
    /// Enable real-time CDR generation
    pub realtime_generation: bool,
    /// CDR flush interval (seconds)
    pub flush_interval: u64,
    /// Maximum CDRs to keep in memory
    pub max_memory_cdrs: usize,
    /// Enable fraud detection
    pub fraud_detection: bool,
    /// Default currency code
    pub default_currency: String,
    /// Default tariff class
    pub default_tariff_class: String,
    /// Intercept targets for J-STD-025 lawful intercept (CALEA)
    pub intercept_targets: Vec<String>,
}

impl Default for CdrEngineConfig {
    fn default() -> Self {
        Self {
            realtime_generation: true,
            flush_interval: 300, // 5 minutes
            max_memory_cdrs: 10000,
            fraud_detection: true,
            default_currency: "USD".to_string(),
            default_tariff_class: "STANDARD".to_string(),
            intercept_targets: Vec::new(),
        }
    }
}

/// CDR Storage Backend Trait
pub trait CdrStorage {
    /// Store a CDR record
    fn store_cdr(&mut self, cdr: &JStd025Cdr) -> Result<()>;

    /// Retrieve CDR records by criteria
    fn retrieve_cdrs(&self, criteria: &CdrSearchCriteria) -> Result<Vec<JStd025Cdr>>;

    /// Generate billing report
    fn generate_billing_report(&self, criteria: &BillingReportCriteria) -> Result<BillingReport>;

    /// Archive old CDRs
    fn archive_cdrs(&mut self, older_than: DateTime<Utc>) -> Result<u64>;

    /// Query CDRs for J-STD-025 lawful intercept (CALEA compliance)
    fn query_cdrs_for_intercept(
        &self,
        target_number: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<JStd025Cdr>>;
}

/// CDR Search Criteria
#[derive(Debug, Clone)]
pub struct CdrSearchCriteria {
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub calling_number: Option<String>,
    pub called_number: Option<String>,
    pub customer_id: Option<String>,
    pub call_result: Option<CallResult>,
    pub service_type: Option<ServiceType>,
    pub limit: Option<usize>,
}

/// Billing Report Criteria
#[derive(Debug, Clone)]
pub struct BillingReportCriteria {
    pub customer_id: Option<String>,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub currency: Option<String>,
    pub include_taxes: bool,
}

/// Generated Billing Report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingReport {
    pub report_id: Uuid,
    pub generation_time: DateTime<Utc>,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub customer_id: Option<String>,
    pub total_calls: u64,
    pub total_duration: u64,
    pub total_charges: f64,
    pub currency: String,
    pub call_summary: Vec<CallSummary>,
    pub fraud_alerts: Vec<FraudAlert>,
}

/// Call Summary for Billing Reports
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallSummary {
    pub service_type: ServiceType,
    pub call_count: u64,
    pub total_duration: u64,
    pub total_charges: f64,
    pub average_duration: f64,
}

/// Fraud Alert Structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FraudAlert {
    pub alert_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub alert_type: String,
    pub severity: String,
    pub calling_number: String,
    pub description: String,
    pub recommended_action: String,
}

impl JStd025CdrEngine {
    /// Create new CDR engine
    pub fn new(storage: Box<dyn CdrStorage + Send + Sync>, config: CdrEngineConfig) -> Self {
        Self {
            active_cdrs: HashMap::new(),
            storage,
            config,
        }
    }

    /// Start a new call and create CDR
    pub fn start_call(
        &mut self,
        call_id: String,
        cdr_type: CdrType,
        calling_number: String,
        called_number: String,
    ) -> Result<()> {
        let mut cdr = JStd025Cdr::new(cdr_type, calling_number, called_number);

        // Set additional fields based on configuration
        if let Some(ref charging) = self.create_default_charging_info() {
            cdr.set_charging_info(charging.clone());
        }

        self.active_cdrs.insert(call_id.clone(), cdr);
        debug!("Started CDR tracking for call: {}", call_id);

        Ok(())
    }

    /// Mark call as answered
    pub fn answer_call(&mut self, call_id: &str) -> Result<()> {
        if let Some(cdr) = self.active_cdrs.get_mut(call_id) {
            cdr.mark_answered();
            debug!("Marked call {} as answered", call_id);
        }
        Ok(())
    }

    /// End call and finalize CDR
    pub fn end_call(&mut self, call_id: &str, result: CallResult) -> Result<()> {
        if let Some(mut cdr) = self.active_cdrs.remove(call_id) {
            cdr.mark_ended(result);

            // Validate CDR before storing
            if let Err(e) = cdr.validate_for_billing() {
                warn!("CDR validation failed for call {}: {}", call_id, e);
            }

            // Store CDR
            self.storage.store_cdr(&cdr)?;
            info!("Finalized and stored CDR for call: {}", call_id);
        }

        Ok(())
    }

    /// Update CDR with quality metrics
    pub fn update_qos_metrics(&mut self, call_id: &str, metrics: QoSMetrics) -> Result<()> {
        if let Some(cdr) = self.active_cdrs.get_mut(call_id) {
            cdr.set_qos_metrics(metrics);
        }
        Ok(())
    }

    /// Add fraud flag to active call
    pub fn add_fraud_flag(&mut self, call_id: &str, flag: String) -> Result<()> {
        if let Some(cdr) = self.active_cdrs.get_mut(call_id) {
            cdr.add_fraud_flag(flag.clone());
            warn!("Added fraud flag '{}' to call {}", flag, call_id);
        }
        Ok(())
    }

    /// Get CDR records for lawful intercept (J-STD-025 compliance)
    /// This method supports court-ordered disclosure of call records
    pub fn get_intercept_records(
        &self,
        target_number: &str,
        warrant_id: Uuid,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<JStd025Cdr>> {
        info!(
            "J-STD-025 Lawful Intercept Request - Warrant: {} for target: {} (period: {} to {})",
            warrant_id, target_number, start_time, end_time
        );

        // Query storage for CDRs matching the target number and time range
        let records = self
            .storage
            .query_cdrs_for_intercept(target_number, start_time, end_time)?;

        // Log lawful intercept activity for audit trail
        info!(
            "J-STD-025 Lawful Intercept: Provided {} CDR records for warrant {}",
            records.len(),
            warrant_id
        );

        Ok(records)
    }

    /// Check if a target number requires J-STD-025 intercept monitoring
    pub fn is_intercept_target(&self, calling_number: &str, called_number: &str) -> bool {
        // Check if either party is subject to lawful intercept
        // This would integrate with warrant management system
        self.config
            .intercept_targets
            .iter()
            .any(|target| target == calling_number)
            || self
                .config
                .intercept_targets
                .iter()
                .any(|target| target == called_number)
    }

    /// Mark CDR for lawful intercept monitoring (J-STD-025)
    pub fn mark_for_intercept(&mut self, call_id: &str, warrant_id: Uuid) -> Result<()> {
        if let Some(cdr) = self.active_cdrs.get_mut(call_id) {
            cdr.warrant_ids.push(warrant_id);
            info!(
                "J-STD-025: Marked call {} for lawful intercept under warrant {}",
                call_id, warrant_id
            );
        }
        Ok(())
    }

    /// Create default charging information
    fn create_default_charging_info(&self) -> Option<ChargingInfo> {
        Some(ChargingInfo {
            tariff_class: self.config.default_tariff_class.clone(),
            rate_per_minute: 0.05, // Default rate
            setup_charge: 0.00,
            total_charge: 0.00,
            currency_code: self.config.default_currency.clone(),
            billing_increment: 60, // 1 minute
            free_units_used: 0,
            discount_percentage: 0.0,
        })
    }

    /// Generate billing report
    pub fn generate_billing_report(
        &self,
        criteria: BillingReportCriteria,
    ) -> Result<BillingReport> {
        self.storage.generate_billing_report(&criteria)
    }

    /// Get active CDR count
    pub fn active_cdr_count(&self) -> usize {
        self.active_cdrs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cdr_creation() {
        let cdr = JStd025Cdr::new(
            CdrType::MOC,
            "+15551234567".to_string(),
            "+15559876543".to_string(),
        );

        assert_eq!(cdr.cdr_type, CdrType::MOC);
        assert_eq!(cdr.calling_number, "+15551234567");
        assert_eq!(cdr.called_number, "+15559876543");
        assert_eq!(cdr.call_result, CallResult::Normal);
    }

    #[test]
    fn test_cdr_lifecycle() {
        let mut cdr = JStd025Cdr::new(
            CdrType::MOC,
            "+15551234567".to_string(),
            "+15559876543".to_string(),
        );

        // Mark as answered
        cdr.mark_answered();
        assert!(cdr.call_answer_time.is_some());

        // Mark as ended
        cdr.mark_ended(CallResult::Normal);
        assert!(cdr.call_end_time.is_some());
        assert!(cdr.call_duration.is_some());
        assert!(cdr.billable_duration.is_some());
    }

    #[test]
    fn test_ciber_format() {
        let mut cdr = JStd025Cdr::new(
            CdrType::MOC,
            "+15551234567".to_string(),
            "+15559876543".to_string(),
        );

        cdr.mark_answered();
        cdr.mark_ended(CallResult::Normal);

        let ciber_output = cdr.to_ciber_format().unwrap();
        assert!(ciber_output.contains("+15551234567"));
        assert!(ciber_output.contains("+15559876543"));
    }

    #[test]
    fn test_cdr_validation() {
        let mut cdr = JStd025Cdr::new(
            CdrType::MOC,
            "+15551234567".to_string(),
            "+15559876543".to_string(),
        );

        // Should fail without end time
        assert!(cdr.validate_for_billing().is_err());

        // Should pass after marking as ended
        cdr.mark_ended(CallResult::Normal);
        assert!(cdr.validate_for_billing().is_ok());
    }

    #[test]
    fn test_fraud_flags() {
        let mut cdr = JStd025Cdr::new(
            CdrType::MOC,
            "+15551234567".to_string(),
            "+15559876543".to_string(),
        );

        cdr.add_fraud_flag("SUSPICIOUS_PATTERN".to_string());
        cdr.add_fraud_flag("HIGH_VOLUME".to_string());

        assert_eq!(cdr.fraud_flags.len(), 2);
        assert!(cdr.fraud_flags.contains(&"SUSPICIOUS_PATTERN".to_string()));

        // Adding same flag again should not duplicate
        cdr.add_fraud_flag("SUSPICIOUS_PATTERN".to_string());
        assert_eq!(cdr.fraud_flags.len(), 2);
    }
}
