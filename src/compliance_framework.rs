/*
 * RedFire Switch Compliance Framework
 * Integrates J-STD-025 CDR and ETSI LI functionality with B2BUA call flow
 *
 * This module provides a unified interface for compliance and regulatory
 * requirements including call detail recording and lawful intercept.
 */

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::etsi_li::{
    EtsiLiController, Hi2EventType, Hi2Record, Hi3ContentRecord, InterceptType, LiControllerConfig,
    LiWarrant, TargetIdentifierType,
};
use crate::j_std_025::{
    CallResult, CdrEngineConfig, CdrType, ChargingInfo, JStd025Cdr, JStd025CdrEngine, QoSMetrics,
    ServiceType,
};

/// Compliance Framework Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceConfig {
    /// Enable J-STD-025 CDR generation
    pub cdr_enabled: bool,
    /// Enable ETSI LI lawful intercept
    pub li_enabled: bool,
    /// CDR engine configuration
    pub cdr_config: CdrEngineConfig,
    /// LI controller configuration
    pub li_config: LiControllerConfig,
    /// Compliance data retention (days)
    pub data_retention_days: u32,
    /// Enable real-time compliance monitoring
    pub realtime_monitoring: bool,
    /// Compliance officer contact information
    pub compliance_officer: Option<ComplianceOfficerInfo>,
}

/// Compliance Officer Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceOfficerInfo {
    pub name: String,
    pub email: String,
    pub phone: String,
    pub badge_number: Option<String>,
}

impl Default for ComplianceConfig {
    fn default() -> Self {
        Self {
            cdr_enabled: true,
            li_enabled: false, // Disabled by default for security
            cdr_config: CdrEngineConfig::default(),
            li_config: LiControllerConfig::default(),
            data_retention_days: 2555, // 7 years
            realtime_monitoring: true,
            compliance_officer: None,
        }
    }
}

/// Call Event for Compliance Processing
#[derive(Debug, Clone)]
pub struct CallEvent {
    /// Unique call identifier
    pub call_id: String,
    /// Event type
    pub event_type: CallEventType,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Calling party number
    pub calling_number: String,
    /// Called party number
    pub called_number: String,
    /// SIP method or response code
    pub sip_method: Option<String>,
    /// SIP response code
    pub sip_response_code: Option<u16>,
    /// Source IP address
    pub source_ip: Option<IpAddr>,
    /// Destination IP address
    pub dest_ip: Option<IpAddr>,
    /// User agent string
    pub user_agent: Option<String>,
    /// Additional SIP headers
    pub sip_headers: HashMap<String, String>,
    /// RTP statistics
    pub rtp_stats: Option<RtpStatistics>,
}

/// Call Event Types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallEventType {
    /// Call attempt (INVITE received)
    CallAttempt,
    /// Call progress (180/183 response)
    CallProgress,
    /// Call answered (200 OK)
    CallAnswered,
    /// Call ended (BYE or error response)
    CallEnded,
    /// Call transferred
    CallTransferred,
    /// Call forwarded
    CallForwarded,
    /// Call hold/unhold
    CallHold,
    /// DTMF detected
    DtmfDetected,
    /// Media stream started
    MediaStarted,
    /// Media stream ended
    MediaEnded,
}

/// RTP Statistics for Quality Metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtpStatistics {
    pub packets_sent: u64,
    pub packets_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_lost: u32,
    pub jitter: f32,
    pub rtt: f32,
    pub mos_score: Option<f32>,
    pub codec: String,
}

/// Compliance Framework - Main Integration Point
pub struct ComplianceFramework {
    /// Configuration
    config: ComplianceConfig,
    /// J-STD-025 CDR engine
    cdr_engine: Option<Arc<RwLock<JStd025CdrEngine>>>,
    /// ETSI LI controller
    li_controller: Option<Arc<EtsiLiController>>,
    /// Active call tracking
    active_calls: Arc<RwLock<HashMap<String, CallState>>>,
    /// Event processing channel
    event_sender: mpsc::UnboundedSender<CallEvent>,
    event_receiver: Arc<RwLock<Option<mpsc::UnboundedReceiver<CallEvent>>>>,
    /// Compliance statistics
    stats: Arc<RwLock<ComplianceStatistics>>,
}

/// Call State Tracking
#[derive(Debug, Clone)]
struct CallState {
    call_id: String,
    calling_number: String,
    called_number: String,
    start_time: DateTime<Utc>,
    answer_time: Option<DateTime<Utc>>,
    end_time: Option<DateTime<Utc>>,
    current_state: CallStateEnum,
    intercept_warrants: Vec<Uuid>,
    cdr_created: bool,
    quality_metrics: Option<RtpStatistics>,
}

/// Call State Enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CallStateEnum {
    Attempting,
    Progressing,
    Answered,
    Ended,
    Transferred,
    Forwarded,
}

/// Compliance Statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceStatistics {
    /// Total calls processed
    pub total_calls: u64,
    /// CDRs generated
    pub cdrs_generated: u64,
    /// LI events captured
    pub li_events_captured: u64,
    /// Active intercepts
    pub active_intercepts: u64,
    /// Compliance errors
    pub compliance_errors: u64,
    /// Last update time
    pub last_updated: DateTime<Utc>,
}

impl ComplianceFramework {
    /// Create new compliance framework
    pub fn new(config: ComplianceConfig) -> Result<Self> {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        // Initialize CDR engine if enabled
        let cdr_engine = if config.cdr_enabled {
            // In a real implementation, this would use a proper storage backend
            let storage = Box::new(MemoryCdrStorage::new());
            let engine = JStd025CdrEngine::new(storage, config.cdr_config.clone());
            Some(Arc::new(RwLock::new(engine)))
        } else {
            None
        };

        // Initialize LI controller if enabled
        let li_controller = if config.li_enabled {
            let controller = EtsiLiController::new(config.li_config.clone());
            Some(Arc::new(controller))
        } else {
            None
        };

        let framework = Self {
            config,
            cdr_engine,
            li_controller,
            active_calls: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
            event_receiver: Arc::new(RwLock::new(Some(event_receiver))),
            stats: Arc::new(RwLock::new(ComplianceStatistics {
                total_calls: 0,
                cdrs_generated: 0,
                li_events_captured: 0,
                active_intercepts: 0,
                compliance_errors: 0,
                last_updated: Utc::now(),
            })),
        };

        Ok(framework)
    }

    /// Start compliance processing
    pub async fn start(&self) -> Result<()> {
        info!("Starting Compliance Framework");

        // Take event receiver
        let receiver = {
            let mut receiver_opt = self.event_receiver.write().await;
            receiver_opt.take()
        };

        if let Some(mut receiver) = receiver {
            let active_calls = Arc::clone(&self.active_calls);
            let cdr_engine = self.cdr_engine.clone();
            let li_controller = self.li_controller.clone();
            let stats = Arc::clone(&self.stats);
            let config = self.config.clone();

            // Spawn event processing task
            tokio::spawn(async move {
                while let Some(event) = receiver.recv().await {
                    if let Err(e) = Self::process_event(
                        &event,
                        &active_calls,
                        &cdr_engine,
                        &li_controller,
                        &stats,
                        &config,
                    )
                    .await
                    {
                        error!("Error processing compliance event: {}", e);
                        let mut stats_guard = stats.write().await;
                        stats_guard.compliance_errors += 1;
                    }
                }
            });
        }

        info!("Compliance Framework started successfully");
        Ok(())
    }

    /// Process call event
    async fn process_event(
        event: &CallEvent,
        active_calls: &Arc<RwLock<HashMap<String, CallState>>>,
        cdr_engine: &Option<Arc<RwLock<JStd025CdrEngine>>>,
        li_controller: &Option<Arc<EtsiLiController>>,
        stats: &Arc<RwLock<ComplianceStatistics>>,
        config: &ComplianceConfig,
    ) -> Result<()> {
        debug!("Processing compliance event: {:?}", event.event_type);

        // Update call state with proper race condition handling
        let (call_state, is_new_call) = {
            let mut calls = active_calls.write().await;
            let is_new = !calls.contains_key(&event.call_id);

            // Get or create call state entry
            let call_state = calls
                .entry(event.call_id.clone())
                .or_insert_with(|| CallState {
                    call_id: event.call_id.clone(),
                    calling_number: event.calling_number.clone(),
                    called_number: event.called_number.clone(),
                    start_time: event.timestamp,
                    answer_time: None,
                    end_time: None,
                    current_state: CallStateEnum::Attempting,
                    intercept_warrants: Vec::new(),
                    cdr_created: false,
                    quality_metrics: event.rtp_stats.clone(),
                });

            // Validate state transitions to prevent race conditions
            let valid_transition = match (call_state.current_state, event.event_type) {
                // Valid state transitions
                (CallStateEnum::Attempting, CallEventType::CallProgress) => true,
                (CallStateEnum::Attempting, CallEventType::CallAnswered) => true,
                (CallStateEnum::Attempting, CallEventType::CallEnded) => true,
                (CallStateEnum::Progressing, CallEventType::CallAnswered) => true,
                (CallStateEnum::Progressing, CallEventType::CallEnded) => true,
                (CallStateEnum::Answered, CallEventType::CallEnded) => true,
                (CallStateEnum::Answered, CallEventType::CallTransferred) => true,
                (CallStateEnum::Answered, CallEventType::CallForwarded) => true,
                (CallStateEnum::Answered, CallEventType::CallHold) => true,
                (CallStateEnum::Answered, CallEventType::DtmfDetected) => true,
                (CallStateEnum::Answered, CallEventType::MediaStarted) => true,
                (CallStateEnum::Answered, CallEventType::MediaEnded) => true,
                // Same state events are always valid
                (current, event_type) if Self::event_to_state(event_type) == Some(current) => true,
                // New call attempt is valid
                (_, CallEventType::CallAttempt) if is_new => true,
                // Invalid transitions
                _ => {
                    warn!(
                        "Invalid state transition from {:?} to {:?} for call {}",
                        call_state.current_state, event.event_type, event.call_id
                    );
                    false
                }
            };

            // Only update state if transition is valid
            if valid_transition {
                // Update state based on event
                match event.event_type {
                    CallEventType::CallAttempt => {
                        call_state.current_state = CallStateEnum::Attempting;
                    }
                    CallEventType::CallProgress => {
                        call_state.current_state = CallStateEnum::Progressing;
                    }
                    CallEventType::CallAnswered => {
                        call_state.current_state = CallStateEnum::Answered;
                        call_state.answer_time = Some(event.timestamp);
                    }
                    CallEventType::CallEnded => {
                        call_state.current_state = CallStateEnum::Ended;
                        call_state.end_time = Some(event.timestamp);
                    }
                    CallEventType::CallTransferred => {
                        call_state.current_state = CallStateEnum::Transferred;
                    }
                    CallEventType::CallForwarded => {
                        call_state.current_state = CallStateEnum::Forwarded;
                    }
                    _ => {}
                }

                // Update quality metrics
                if let Some(ref rtp_stats) = event.rtp_stats {
                    call_state.quality_metrics = Some(rtp_stats.clone());
                }
            }

            (call_state.clone(), is_new)
        };

        // Check for lawful intercept if enabled
        if let Some(ref li_ctrl) = li_controller {
            let calling_warrants = li_ctrl.should_intercept(&event.calling_number).await?;
            let called_warrants = li_ctrl.should_intercept(&event.called_number).await?;

            let mut all_warrants = calling_warrants;
            all_warrants.extend(called_warrants);

            if !all_warrants.is_empty() {
                // Update call state with intercept warrants
                {
                    let mut calls = active_calls.write().await;
                    if let Some(state) = calls.get_mut(&event.call_id) {
                        state.intercept_warrants = all_warrants.clone();
                    }
                }

                // Generate HI2 record
                let hi2_record = Self::create_hi2_record(event, &call_state)?;
                li_ctrl
                    .capture_hi2(all_warrants.clone(), hi2_record)
                    .await?;

                // Capture HI3 content if applicable
                if matches!(event.event_type, CallEventType::MediaStarted) {
                    if let Some(ref rtp_stats) = event.rtp_stats {
                        let hi3_record = Self::create_hi3_record(event, &call_state, rtp_stats)?;
                        li_ctrl
                            .capture_hi3(all_warrants.clone(), hi3_record)
                            .await?;
                    }
                }

                // Update statistics
                let mut stats_guard = stats.write().await;
                stats_guard.li_events_captured += 1;
                stats_guard.active_intercepts = all_warrants.len() as u64;
            }
        }

        // Process CDR if enabled
        if let Some(ref cdr_eng) = cdr_engine {
            let mut engine = cdr_eng.write().await;

            match event.event_type {
                CallEventType::CallAttempt => {
                    if is_new_call {
                        let cdr_type = if event.calling_number.starts_with('+') {
                            CdrType::MOC // Mobile Originated
                        } else {
                            CdrType::MTC // Mobile Terminated
                        };

                        engine.start_call(
                            event.call_id.clone(),
                            cdr_type,
                            event.calling_number.clone(),
                            event.called_number.clone(),
                        )?;

                        // Mark CDR as created
                        let mut calls = active_calls.write().await;
                        if let Some(state) = calls.get_mut(&event.call_id) {
                            state.cdr_created = true;
                        }
                    }
                }
                CallEventType::CallAnswered => {
                    engine.answer_call(&event.call_id)?;
                }
                CallEventType::CallEnded => {
                    let result = Self::determine_call_result(event, &call_state);
                    engine.end_call(&event.call_id, result)?;

                    // Update quality metrics if available
                    if let Some(ref rtp_stats) = call_state.quality_metrics {
                        let qos_metrics = Self::convert_rtp_to_qos(rtp_stats);
                        engine.update_qos_metrics(&event.call_id, qos_metrics)?;
                    }

                    // Remove from active calls
                    let mut calls = active_calls.write().await;
                    calls.remove(&event.call_id);

                    // Update statistics
                    let mut stats_guard = stats.write().await;
                    stats_guard.cdrs_generated += 1;
                }
                _ => {}
            }
        }

        // Update general statistics
        {
            let mut stats_guard = stats.write().await;
            if is_new_call {
                stats_guard.total_calls += 1;
            }
            stats_guard.last_updated = Utc::now();
        }

        Ok(())
    }

    /// Submit call event for compliance processing
    pub fn submit_call_event(&self, event: CallEvent) -> Result<()> {
        self.event_sender
            .send(event)
            .map_err(|e| anyhow!("Failed to submit call event: {}", e))
    }

    /// Create HI2 record from call event
    fn create_hi2_record(event: &CallEvent, call_state: &CallState) -> Result<Hi2Record> {
        use crate::etsi_li::{NetworkInformation, PartyInformation, ServiceInformation};

        let event_type = match event.event_type {
            CallEventType::CallAttempt => Hi2EventType::CallAttempt,
            CallEventType::CallAnswered => Hi2EventType::CallConnected,
            CallEventType::CallEnded => Hi2EventType::CallReleased,
            _ => Hi2EventType::CallAttempt,
        };

        // Ensure we have a valid warrant - creating fake warrant IDs violates legal compliance
        let warrant_id = call_state.intercept_warrants.first().copied()
            .ok_or_else(|| anyhow!("No valid warrant ID for HI2 record - cannot create lawful intercept record without warrant"))?;

        Ok(Hi2Record {
            record_id: Uuid::new_v4(),
            warrant_id,
            target_id: event.calling_number.clone(),
            timestamp: event.timestamp,
            event_type,
            calling_party: Some(PartyInformation {
                party_id: event.calling_number.clone(),
                identity_type: TargetIdentifierType::PhoneNumber,
                party_role: "originating".to_string(),
                location: None,
                service_provider: Some("RedFire Switch".to_string()),
            }),
            called_party: Some(PartyInformation {
                party_id: event.called_number.clone(),
                identity_type: TargetIdentifierType::PhoneNumber,
                party_role: "terminating".to_string(),
                location: None,
                service_provider: None,
            }),
            location_info: None,
            service_info: ServiceInformation {
                service_type: "voice".to_string(),
                service_id: Some(event.call_id.clone()),
                qos_info: None,
                supplementary_services: Vec::new(),
            },
            network_info: NetworkInformation {
                network_id: "REDFIRE_NETWORK".to_string(),
                access_technology: "SIP".to_string(),
                serving_element: "RedFire-B2BUA".to_string(),
                element_ip: event.source_ip,
            },
            additional_info: HashMap::new(),
        })
    }

    /// Create HI3 content record
    fn create_hi3_record(
        event: &CallEvent,
        call_state: &CallState,
        rtp_stats: &RtpStatistics,
    ) -> Result<Hi3ContentRecord> {
        use crate::etsi_li::{ContentMetadata, ContentType};

        // Ensure we have a valid warrant for content intercept
        let warrant_id = call_state.intercept_warrants.first().copied()
            .ok_or_else(|| anyhow!("No valid warrant ID for HI3 content record - cannot intercept content without warrant"))?;

        // In a real implementation, this would contain actual audio content
        let mock_audio_content = b"MOCK_AUDIO_CONTENT".to_vec();

        Ok(Hi3ContentRecord {
            record_id: Uuid::new_v4(),
            warrant_id,
            hi2_record_id: None,
            timestamp: event.timestamp,
            content_type: ContentType::VoiceAudio,
            content_payload: mock_audio_content.clone(),
            metadata: ContentMetadata {
                encoding: rtp_stats.codec.clone(),
                size: mock_audio_content.len() as u64,
                checksum: "SHA256:MOCK_CHECKSUM".to_string(),
                encryption_algorithm: Some("AES-256-GCM".to_string()),
                compression_algorithm: None,
            },
            sequence_number: 1,
        })
    }

    /// Determine call result from event and state
    fn determine_call_result(event: &CallEvent, call_state: &CallState) -> CallResult {
        if let Some(response_code) = event.sip_response_code {
            match response_code {
                200 => CallResult::Normal,
                486 => CallResult::Busy,
                408 | 480 => CallResult::NoAnswer,
                503 => CallResult::ServiceUnavailable,
                404 => CallResult::InvalidNumber,
                _ => CallResult::SystemFailure,
            }
        } else if call_state.answer_time.is_some() {
            CallResult::Normal
        } else {
            CallResult::SystemFailure
        }
    }

    /// Convert RTP statistics to QoS metrics
    fn convert_rtp_to_qos(rtp_stats: &RtpStatistics) -> QoSMetrics {
        let packet_loss_rate = if rtp_stats.packets_sent > 0 {
            (rtp_stats.packets_lost as f32 / rtp_stats.packets_sent as f32) * 100.0
        } else {
            0.0
        };

        QoSMetrics {
            mos_score: rtp_stats.mos_score,
            packet_loss: Some(packet_loss_rate),
            jitter: Some(rtp_stats.jitter),
            rtt: Some(rtp_stats.rtt),
            codec: Some(rtp_stats.codec.clone()),
            bit_rate: None, // Could be calculated from bytes and duration
        }
    }

    /// Get compliance statistics
    pub async fn get_statistics(&self) -> ComplianceStatistics {
        let stats = self.stats.read().await;
        stats.clone()
    }

    /// Get active call count
    pub async fn get_active_call_count(&self) -> usize {
        let calls = self.active_calls.read().await;
        calls.len()
    }

    /// Convert event type to call state for validation
    fn event_to_state(event_type: CallEventType) -> Option<CallStateEnum> {
        match event_type {
            CallEventType::CallAttempt => Some(CallStateEnum::Attempting),
            CallEventType::CallProgress => Some(CallStateEnum::Progressing),
            CallEventType::CallAnswered => Some(CallStateEnum::Answered),
            CallEventType::CallEnded => Some(CallStateEnum::Ended),
            CallEventType::CallTransferred => Some(CallStateEnum::Transferred),
            CallEventType::CallForwarded => Some(CallStateEnum::Forwarded),
            _ => None,
        }
    }
}

/// Memory-based CDR storage for testing/development
/// In production, this should be replaced with database storage
struct MemoryCdrStorage {
    cdrs: Vec<JStd025Cdr>,
}

impl MemoryCdrStorage {
    fn new() -> Self {
        Self { cdrs: Vec::new() }
    }
}

impl crate::j_std_025::CdrStorage for MemoryCdrStorage {
    fn store_cdr(&mut self, cdr: &JStd025Cdr) -> Result<()> {
        self.cdrs.push(cdr.clone());
        debug!("Stored CDR: {}", cdr.record_id);
        Ok(())
    }

    fn retrieve_cdrs(
        &self,
        _criteria: &crate::j_std_025::CdrSearchCriteria,
    ) -> Result<Vec<JStd025Cdr>> {
        Ok(self.cdrs.clone())
    }

    fn generate_billing_report(
        &self,
        _criteria: &crate::j_std_025::BillingReportCriteria,
    ) -> Result<crate::j_std_025::BillingReport> {
        use crate::j_std_025::BillingReport;

        Ok(BillingReport {
            report_id: Uuid::new_v4(),
            generation_time: Utc::now(),
            period_start: Utc::now() - chrono::Duration::days(30),
            period_end: Utc::now(),
            customer_id: None,
            total_calls: self.cdrs.len() as u64,
            total_duration: self
                .cdrs
                .iter()
                .filter_map(|cdr| cdr.billable_duration)
                .sum::<u64>(),
            total_charges: self
                .cdrs
                .iter()
                .filter_map(|cdr| cdr.charging_info.as_ref())
                .map(|c| c.total_charge)
                .sum::<f64>(),
            currency: "USD".to_string(),
            call_summary: Vec::new(),
            fraud_alerts: Vec::new(),
        })
    }

    fn archive_cdrs(&mut self, older_than: DateTime<Utc>) -> Result<u64> {
        let initial_count = self.cdrs.len();
        self.cdrs.retain(|cdr| cdr.record_timestamp > older_than);
        let archived_count = initial_count - self.cdrs.len();
        Ok(archived_count as u64)
    }

    fn query_cdrs_for_intercept(
        &self,
        target_number: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<JStd025Cdr>> {
        let matching_cdrs = self
            .cdrs
            .iter()
            .filter(|cdr| {
                (cdr.calling_number == target_number || cdr.called_number == target_number)
                    && cdr.call_start_time >= start_time
                    && cdr.call_start_time <= end_time
            })
            .cloned()
            .collect();

        Ok(matching_cdrs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_compliance_framework_creation() {
        let config = ComplianceConfig::default();
        let framework = ComplianceFramework::new(config).unwrap();

        let stats = framework.get_statistics().await;
        assert_eq!(stats.total_calls, 0);
        assert_eq!(stats.cdrs_generated, 0);
    }

    #[tokio::test]
    async fn test_call_event_processing() {
        let config = ComplianceConfig {
            cdr_enabled: true,
            li_enabled: false,
            ..Default::default()
        };

        let framework = ComplianceFramework::new(config).unwrap();
        framework.start().await.unwrap();

        // Submit call attempt event
        let call_event = CallEvent {
            call_id: "test_call_001".to_string(),
            event_type: CallEventType::CallAttempt,
            timestamp: Utc::now(),
            calling_number: "+15551234567".to_string(),
            called_number: "+15559876543".to_string(),
            sip_method: Some("INVITE".to_string()),
            sip_response_code: None,
            source_ip: Some("192.168.1.100".parse().unwrap()),
            dest_ip: Some("192.168.1.200".parse().unwrap()),
            user_agent: Some("RedFire-Switch/1.0".to_string()),
            sip_headers: HashMap::new(),
            rtp_stats: None,
        };

        framework.submit_call_event(call_event).unwrap();

        // Give some time for processing
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        assert_eq!(framework.get_active_call_count().await, 1);
    }

    #[test]
    fn test_call_result_determination() {
        let call_state = CallState {
            call_id: "test".to_string(),
            calling_number: "+15551234567".to_string(),
            called_number: "+15559876543".to_string(),
            start_time: Utc::now(),
            answer_time: Some(Utc::now()),
            end_time: None,
            current_state: CallStateEnum::Answered,
            intercept_warrants: Vec::new(),
            cdr_created: false,
            quality_metrics: None,
        };

        let event = CallEvent {
            call_id: "test".to_string(),
            event_type: CallEventType::CallEnded,
            timestamp: Utc::now(),
            calling_number: "+15551234567".to_string(),
            called_number: "+15559876543".to_string(),
            sip_method: None,
            sip_response_code: Some(200),
            source_ip: None,
            dest_ip: None,
            user_agent: None,
            sip_headers: HashMap::new(),
            rtp_stats: None,
        };

        let result = ComplianceFramework::determine_call_result(&event, &call_state);
        assert_eq!(result, CallResult::Normal);
    }
}
