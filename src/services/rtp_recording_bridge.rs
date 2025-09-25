//! RTP Recording Bridge Service
//!
//! This service bridges RTP packet processing with audio recording,
//! integrating lawful intercept and voice integrity monitoring.
//!
//! Key features:
//! - RTP packet capture from media sessions
//! - Real-time audio recording with WAV headers
//! - Legal authorization compliance checking
//! - ECPA-compliant recording decisions

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, instrument};
use uuid::Uuid;

use crate::events::{EventBus, TelecomEvent};
use crate::services::audio_recording::{
    AudioRecordingService, RecordingCodec, StorageType, RtpAudioPacket,
};
use crate::services::legal_authorization::LegalAuthorizationService;

/// Call recording decision
#[derive(Debug, Clone, PartialEq)]
pub enum RecordingDecision {
    /// No recording required
    None,
    /// Record to memory for fraud detection
    FraudDetection,
    /// Record to disk for legal authorization
    LegalAuthorization(i32),
}

/// Active call session for recording
#[derive(Debug, Clone)]
pub struct CallRecordingSession {
    pub call_id: String,
    pub session_id: String,
    pub recording_id: String,
    pub trunk_id: i32,
    pub calling_number: String,
    pub called_number: String,
    pub recording_decision: RecordingDecision,
    pub original_codec: RecordingCodec,
    pub started_at: chrono::DateTime<Utc>,
    pub packet_count: u64,
}

/// RTP Recording Bridge configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtpRecordingBridgeConfig {
    /// Enable RTP recording bridge
    pub enabled: bool,
    /// Default fraud detection recording percentage (0.0-1.0)
    pub fraud_detection_sample_rate: f32,
    /// Enable automatic recording for specific trunk patterns
    pub auto_record_trunk_patterns: Vec<String>,
    /// Enable automatic recording for specific number patterns
    pub auto_record_number_patterns: Vec<String>,
    /// Maximum concurrent recordings
    pub max_concurrent_recordings: usize,
    /// Default codec for new recordings
    pub default_recording_codec: RecordingCodec,
}

impl Default for RtpRecordingBridgeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            fraud_detection_sample_rate: 0.05, // 5% sampling
            auto_record_trunk_patterns: vec![
                "international_.*".to_string(),
                "premium_.*".to_string(),
            ],
            auto_record_number_patterns: vec![
                r"^1900\d+".to_string(), // Premium rate numbers
                r"^011\d+".to_string(),   // International calls
            ],
            max_concurrent_recordings: 1000,
            default_recording_codec: RecordingCodec::PCMU,
        }
    }
}

/// RTP Recording Bridge Service
pub struct RtpRecordingBridgeService {
    config: RtpRecordingBridgeConfig,
    event_bus: Arc<EventBus>,
    audio_recording_service: Arc<AudioRecordingService>,
    legal_auth_service: Arc<LegalAuthorizationService>,
    active_sessions: Arc<RwLock<HashMap<String, CallRecordingSession>>>,
}

impl RtpRecordingBridgeService {
    /// Create new RTP recording bridge service
    pub fn new(
        config: RtpRecordingBridgeConfig,
        event_bus: Arc<EventBus>,
        audio_recording_service: Arc<AudioRecordingService>,
        legal_auth_service: Arc<LegalAuthorizationService>,
    ) -> Self {
        Self {
            config,
            event_bus,
            audio_recording_service,
            legal_auth_service,
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Handle call initiation - decide whether to record
    #[instrument(skip(self), fields(call_id = %call_id, calling = %calling_number, called = %called_number))]
    pub async fn handle_call_initiated(
        &self,
        call_id: String,
        session_id: String,
        trunk_id: i32,
        calling_number: String,
        called_number: String,
        offered_codec: RecordingCodec,
    ) -> Result<()> {
        info!("Evaluating recording requirements for call {}", call_id);

        // Check legal authorization requirements first
        let legal_auth_id = self.audio_recording_service
            .should_record_call(trunk_id, &calling_number, &called_number)
            .await?;

        let recording_decision = if let Some(auth_id) = legal_auth_id {
            // Legal authorization requires recording
            RecordingDecision::LegalAuthorization(auth_id)
        } else if self.should_record_for_fraud_detection(&calling_number, &called_number, trunk_id) {
            // Fraud detection sampling
            RecordingDecision::FraudDetection
        } else {
            RecordingDecision::None
        };

        // Start recording if required
        if !matches!(recording_decision, RecordingDecision::None) {
            let recording_id = Uuid::new_v4().to_string();

            let session = CallRecordingSession {
                call_id: call_id.clone(),
                session_id: session_id.clone(),
                recording_id: recording_id.clone(),
                trunk_id,
                calling_number: calling_number.clone(),
                called_number: called_number.clone(),
                recording_decision: recording_decision.clone(),
                original_codec: offered_codec,
                started_at: Utc::now(),
                packet_count: 0,
            };

            // Determine storage type and authorization
            let (storage_type, auth_id) = match recording_decision {
                RecordingDecision::LegalAuthorization(id) => (StorageType::Disk, Some(id)),
                RecordingDecision::FraudDetection => (StorageType::Memory, None),
                RecordingDecision::None => unreachable!(),
            };

            // Start audio recording
            self.audio_recording_service
                .start_recording(
                    recording_id.clone(),
                    call_id.clone(),
                    session_id.clone(),
                    trunk_id,
                    offered_codec,
                    storage_type,
                    auth_id,
                )
                .await?;

            // Track the session
            let mut sessions = self.active_sessions.write().await;
            sessions.insert(call_id.clone(), session);

            info!("Started recording for call {} with decision: {:?}", call_id, recording_decision);

            // Emit audit event
            let event = TelecomEvent::VoiceIntegrityAudit {
                user_id: None,
                action_type: "call_recording_started".to_string(),
                resource_type: "call_session".to_string(),
                resource_id: call_id,
                authorization_id: auth_id,
                ecpa_compliant: true,
            };
            self.event_bus.publish(event).await?;
        } else {
            debug!("No recording required for call {}", call_id);
        }

        Ok(())
    }

    /// Handle RTP packet from media session
    #[instrument(skip(self, payload), fields(call_id = %call_id, sequence = sequence_number))]
    pub async fn handle_rtp_packet(
        &self,
        call_id: String,
        payload_type: u8,
        sequence_number: u16,
        timestamp: u32,
        ssrc: u32,
        payload: Vec<u8>,
    ) -> Result<()> {
        // Check if this call is being recorded
        let sessions = self.active_sessions.read().await;
        if let Some(session) = sessions.get(&call_id) {
            // Create RTP audio packet
            let rtp_packet = RtpAudioPacket {
                payload_type,
                sequence_number,
                timestamp,
                ssrc,
                payload,
                received_at: Utc::now(),
            };

            // Send to audio recording service
            self.audio_recording_service
                .process_rtp_packet(session.recording_id.clone(), rtp_packet)
                .await?;

            debug!("Processed RTP packet for recording: call={}, seq={}",
                   call_id, sequence_number);
        }

        Ok(())
    }

    /// Handle call termination
    #[instrument(skip(self), fields(call_id = %call_id))]
    pub async fn handle_call_terminated(&self, call_id: String) -> Result<()> {
        info!("Handling call termination for {}", call_id);

        // Remove from active sessions
        let mut sessions = self.active_sessions.write().await;
        if let Some(session) = sessions.remove(&call_id) {
            // Stop the recording
            let completed_recording = self.audio_recording_service
                .stop_recording(session.recording_id.clone())
                .await?;

            info!("Completed recording for call {}: {} bytes, {} seconds",
                  call_id, completed_recording.file_size_bytes, completed_recording.duration_seconds);

            // Emit completion event
            let event = TelecomEvent::VoiceIntegrityAudit {
                user_id: None,
                action_type: "call_recording_completed".to_string(),
                resource_type: "call_session".to_string(),
                resource_id: call_id,
                authorization_id: completed_recording.legal_authorization_id,
                ecpa_compliant: true,
            };
            self.event_bus.publish(event).await?;
        }

        Ok(())
    }

    /// Check if call should be recorded for fraud detection
    fn should_record_for_fraud_detection(
        &self,
        calling_number: &str,
        called_number: &str,
        trunk_id: i32,
    ) -> bool {
        // Check trunk patterns
        let trunk_name = format!("trunk_{}", trunk_id);
        for pattern in &self.config.auto_record_trunk_patterns {
            if regex::Regex::new(pattern)
                .map(|re| re.is_match(&trunk_name))
                .unwrap_or(false)
            {
                debug!("Trunk pattern match for recording: {}", pattern);
                return true;
            }
        }

        // Check number patterns
        for pattern in &self.config.auto_record_number_patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                if re.is_match(calling_number) || re.is_match(called_number) {
                    debug!("Number pattern match for recording: {}", pattern);
                    return true;
                }
            }
        }

        // Statistical sampling for fraud detection
        use rand::prelude::*;
        let mut rng = thread_rng();
        let sample: f32 = rng.gen();

        if sample < self.config.fraud_detection_sample_rate {
            debug!("Fraud detection sampling triggered: {} < {}",
                   sample, self.config.fraud_detection_sample_rate);
            return true;
        }

        false
    }

    /// Get recording statistics
    pub async fn get_recording_statistics(&self) -> HashMap<String, u64> {
        let sessions = self.active_sessions.read().await;
        let mut stats = HashMap::new();

        stats.insert("active_recording_sessions".to_string(), sessions.len() as u64);

        let mut legal_recordings = 0u64;
        let mut fraud_recordings = 0u64;
        let mut total_packets = 0u64;

        for session in sessions.values() {
            match session.recording_decision {
                RecordingDecision::LegalAuthorization(_) => legal_recordings += 1,
                RecordingDecision::FraudDetection => fraud_recordings += 1,
                RecordingDecision::None => {}
            }
            total_packets += session.packet_count;
        }

        stats.insert("legal_authorization_recordings".to_string(), legal_recordings);
        stats.insert("fraud_detection_recordings".to_string(), fraud_recordings);
        stats.insert("total_packets_recorded".to_string(), total_packets);

        // Get audio recording service stats
        let audio_stats = self.audio_recording_service.get_recording_stats().await;
        for (key, value) in audio_stats {
            stats.insert(format!("audio_{}", key), value);
        }

        stats
    }

    /// Update configuration
    pub fn update_config(&mut self, new_config: RtpRecordingBridgeConfig) {
        self.config = new_config;
        info!("RTP Recording Bridge configuration updated");
    }

    /// Check if recording is active for a call
    pub async fn is_recording_active(&self, call_id: &str) -> bool {
        let sessions = self.active_sessions.read().await;
        sessions.contains_key(call_id)
    }

    /// Get active recording session info
    pub async fn get_recording_session(&self, call_id: &str) -> Option<CallRecordingSession> {
        let sessions = self.active_sessions.read().await;
        sessions.get(call_id).cloned()
    }

    /// Force start recording for a call (for emergency situations)
    #[instrument(skip(self), fields(call_id = %call_id))]
    pub async fn force_start_recording(
        &self,
        call_id: String,
        session_id: String,
        trunk_id: i32,
        calling_number: String,
        called_number: String,
        codec: RecordingCodec,
        legal_authorization_id: Option<i32>,
    ) -> Result<()> {
        warn!("Force starting recording for call {} (emergency/manual override)", call_id);

        let recording_id = Uuid::new_v4().to_string();

        let session = CallRecordingSession {
            call_id: call_id.clone(),
            session_id: session_id.clone(),
            recording_id: recording_id.clone(),
            trunk_id,
            calling_number,
            called_number,
            recording_decision: if legal_authorization_id.is_some() {
                RecordingDecision::LegalAuthorization(legal_authorization_id.unwrap())
            } else {
                RecordingDecision::FraudDetection
            },
            original_codec: codec,
            started_at: Utc::now(),
            packet_count: 0,
        };

        let storage_type = if legal_authorization_id.is_some() {
            StorageType::Disk
        } else {
            StorageType::Memory
        };

        // Start recording
        self.audio_recording_service
            .start_recording(
                recording_id,
                call_id.clone(),
                session_id,
                trunk_id,
                codec,
                storage_type,
                legal_authorization_id,
            )
            .await?;

        // Track the session
        let mut sessions = self.active_sessions.write().await;
        sessions.insert(call_id.clone(), session);

        // Emit audit event
        let event = TelecomEvent::VoiceIntegrityAudit {
            user_id: None,
            action_type: "force_recording_started".to_string(),
            resource_type: "call_session".to_string(),
            resource_id: call_id,
            authorization_id: legal_authorization_id,
            ecpa_compliant: true,
        };
        self.event_bus.publish(event).await?;

        Ok(())
    }
}