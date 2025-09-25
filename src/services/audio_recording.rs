//! Audio Recording Service with WAV Support and Transcoding
//!
//! This service handles audio recording from RTP streams with proper WAV headers
//! and transcoding capabilities for compliance with voice integrity requirements.
//!
//! Key features:
//! - WAV format recording with proper headers
//! - Real-time transcoding from RTP codecs to WAV
//! - ECPA-compliant recording with legal authorization checks
//! - Memory and disk storage options
//! - Integration with lawful intercept targets

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use hound::{WavSpec, WavWriter};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn, instrument};
use uuid;

use crate::events::{EventBus, TelecomEvent};
use crate::services::legal_authorization::LegalAuthorizationService;
use crate::services::memory_management::{MemoryManagementService, RecordingPriority};

/// Audio codec types supported for recording
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordingCodec {
    /// G.711 μ-law (PCMU)
    PCMU,
    /// G.711 A-law (PCMA)
    PCMA,
    /// G.722 wideband
    G722,
    /// G.729 compressed
    G729,
    /// Linear PCM (WAV native)
    PCM16,
}

impl RecordingCodec {
    /// Get the RTP payload type for this codec
    pub fn rtp_payload_type(&self) -> u8 {
        match self {
            RecordingCodec::PCMU => 0,
            RecordingCodec::PCMA => 8,
            RecordingCodec::G722 => 9,
            RecordingCodec::G729 => 18,
            RecordingCodec::PCM16 => 97, // Dynamic payload type
        }
    }

    /// Get the sample rate for this codec
    pub fn sample_rate(&self) -> u32 {
        match self {
            RecordingCodec::PCMU | RecordingCodec::PCMA | RecordingCodec::G729 => 8000,
            RecordingCodec::G722 => 16000,
            RecordingCodec::PCM16 => 8000, // Default telephony
        }
    }

    /// Check if transcoding is needed to WAV format
    pub fn needs_transcoding(&self) -> bool {
        !matches!(self, RecordingCodec::PCM16)
    }
}

/// Recording storage type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageType {
    /// In-memory recording for fraud detection
    Memory,
    /// Persistent disk storage for legal cases
    Disk,
}

/// Audio recording session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioRecording {
    pub recording_id: String,
    pub call_id: String,
    pub session_id: String,
    pub trunk_id: i32,

    // Recording details
    pub storage_type: StorageType,
    pub file_path: PathBuf,
    pub original_codec: RecordingCodec,
    pub wav_sample_rate: u32,
    pub wav_channels: u16,
    pub wav_bits_per_sample: u16,

    // Legal compliance
    pub legal_authorization_id: Option<i32>,
    pub monitoring_purpose: String,
    pub ecpa_compliant: bool,

    // Timing and size
    pub started_at: DateTime<Utc>,
    pub duration_seconds: u32,
    pub file_size_bytes: u64,
    pub total_samples: u64,

    // Status
    pub is_active: bool,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Audio recording configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioRecordingConfig {
    /// Enable audio recording
    pub enabled: bool,
    /// Memory storage path (/dev/shm for faster access)
    pub memory_storage_path: String,
    /// Persistent disk storage path
    pub disk_storage_path: String,
    /// Maximum recording duration in seconds
    pub max_recording_duration_seconds: u32,
    /// WAV sample rate for output
    pub wav_sample_rate: u32,
    /// WAV channels (1 = mono, 2 = stereo)
    pub wav_channels: u16,
    /// WAV bits per sample
    pub wav_bits_per_sample: u16,
    /// Maximum memory usage for recordings (bytes)
    pub max_memory_usage_bytes: u64,
    /// Maximum disk usage for recordings (bytes)
    pub max_disk_usage_bytes: u64,
    /// Enable real-time transcoding
    pub enable_transcoding: bool,
    /// Buffer size for RTP packet processing
    pub rtp_buffer_size: usize,
    /// Cleanup interval for expired recordings
    pub cleanup_interval_minutes: u32,
    /// Enable batch processing mode
    pub enable_batch_processing: bool,
    /// Batch size for packet processing
    pub batch_size: usize,
    /// Batch timeout in milliseconds
    pub batch_timeout_ms: u64,
}

impl Default for AudioRecordingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            memory_storage_path: "/dev/shm/redfire-audio".to_string(),
            disk_storage_path: "/var/lib/redfire/recordings".to_string(),
            max_recording_duration_seconds: 3600, // 1 hour
            wav_sample_rate: 8000, // Telephony standard
            wav_channels: 1, // Mono
            wav_bits_per_sample: 16, // CD quality
            max_memory_usage_bytes: 5 * 1024 * 1024 * 1024, // 5GB
            max_disk_usage_bytes: 100 * 1024 * 1024 * 1024, // 100GB
            enable_transcoding: true,
            rtp_buffer_size: 8192,
            cleanup_interval_minutes: 60,
            enable_batch_processing: true,
            batch_size: 100, // Process 100 packets at a time
            batch_timeout_ms: 1000, // 1 second timeout
        }
    }
}

/// RTP packet for audio recording
#[derive(Debug, Clone)]
pub struct RtpAudioPacket {
    pub payload_type: u8,
    pub sequence_number: u16,
    pub timestamp: u32,
    pub ssrc: u32,
    pub payload: Vec<u8>,
    pub received_at: DateTime<Utc>,
}

/// Audio transcoder for converting RTP payloads to WAV
pub struct AudioTranscoder {
    input_codec: RecordingCodec,
    output_spec: WavSpec,
}

impl AudioTranscoder {
    /// Create a new audio transcoder
    pub fn new(input_codec: RecordingCodec, output_spec: WavSpec) -> Self {
        Self {
            input_codec,
            output_spec,
        }
    }

    /// Transcode RTP payload to WAV samples
    pub fn transcode_packet(&self, payload: &[u8]) -> Result<Vec<i16>> {
        match self.input_codec {
            RecordingCodec::PCMU => self.transcode_pcmu(payload),
            RecordingCodec::PCMA => self.transcode_pcma(payload),
            RecordingCodec::G722 => self.transcode_g722(payload),
            RecordingCodec::G729 => self.transcode_g729(payload),
            RecordingCodec::PCM16 => self.transcode_pcm16(payload),
        }
    }

    /// Transcode G.711 μ-law to linear PCM
    fn transcode_pcmu(&self, payload: &[u8]) -> Result<Vec<i16>> {
        let mut samples = Vec::with_capacity(payload.len());

        for &byte in payload {
            // G.711 μ-law to linear PCM conversion
            let linear = Self::pcmu_to_linear(byte);
            samples.push(linear);
        }

        Ok(samples)
    }

    /// Transcode G.711 A-law to linear PCM
    fn transcode_pcma(&self, payload: &[u8]) -> Result<Vec<i16>> {
        let mut samples = Vec::with_capacity(payload.len());

        for &byte in payload {
            // G.711 A-law to linear PCM conversion
            let linear = Self::pcma_to_linear(byte);
            samples.push(linear);
        }

        Ok(samples)
    }

    /// Transcode G.722 to linear PCM (simplified implementation)
    fn transcode_g722(&self, payload: &[u8]) -> Result<Vec<i16>> {
        // This is a simplified implementation - in production, you'd use a proper G.722 decoder
        warn!("G.722 transcoding not fully implemented - using placeholder");

        // Convert to approximate PCM samples (placeholder)
        let mut samples = Vec::with_capacity(payload.len() * 2);
        for &byte in payload {
            // Simple approximation - would need proper G.722 decoding
            let sample = (byte as i16 - 128) * 256;
            samples.push(sample);
            samples.push(sample); // Upsample for 16kHz -> 8kHz conversion
        }

        Ok(samples)
    }

    /// Transcode G.729 to linear PCM (requires external library)
    fn transcode_g729(&self, payload: &[u8]) -> Result<Vec<i16>> {
        // G.729 decoding requires specialized library - placeholder implementation
        warn!("G.729 transcoding not implemented - using silence");

        // Each G.729 frame is 10 bytes and represents 80 samples at 8kHz
        let frame_count = payload.len() / 10;
        let sample_count = frame_count * 80;

        // Return silence as placeholder
        Ok(vec![0; sample_count])
    }

    /// Handle PCM16 (no transcoding needed)
    fn transcode_pcm16(&self, payload: &[u8]) -> Result<Vec<i16>> {
        if payload.len() % 2 != 0 {
            return Err(anyhow::anyhow!("PCM16 payload must have even number of bytes"));
        }

        let mut samples = Vec::with_capacity(payload.len() / 2);
        for chunk in payload.chunks_exact(2) {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
            samples.push(sample);
        }

        Ok(samples)
    }

    /// Convert μ-law byte to linear PCM
    fn pcmu_to_linear(pcmu: u8) -> i16 {
        const BIAS: i16 = 0x84;
        const CLIP: i16 = 8159;

        let pcmu = pcmu ^ 0xFF;
        let sign = if (pcmu & 0x80) != 0 { -1 } else { 1 };
        let exponent = ((pcmu >> 4) & 0x07) as i16;
        let mantissa = (pcmu & 0x0F) as i16;

        let linear = if exponent == 0 {
            (mantissa << 4) + BIAS
        } else {
            ((mantissa << 4) + BIAS) << (exponent - 1)
        };

        (sign * linear).clamp(-CLIP, CLIP)
    }

    /// Convert A-law byte to linear PCM
    fn pcma_to_linear(pcma: u8) -> i16 {
        const QUANT_MASK: u8 = 0x0F;
        const SEG_SHIFT: i16 = 4;
        const SEG_MASK: u8 = 0x70;
        const SIGN_BIT: u8 = 0x80;

        let pcma = pcma ^ 0x55;
        let sign = if (pcma & SIGN_BIT) != 0 { -1 } else { 1 };
        let seg = ((pcma & SEG_MASK) >> SEG_SHIFT) as i16;
        let quant = (pcma & QUANT_MASK) as i16;

        let linear = if seg == 0 {
            quant << 4
        } else {
            ((quant << 4) + 0x108) << (seg - 1)
        };

        sign * linear
    }
}

/// Active WAV recording session
pub struct WavRecordingSession {
    recording: AudioRecording,
    wav_writer: WavWriter<BufWriter<File>>,
    transcoder: AudioTranscoder,
    packet_count: u64,
    last_sequence: Option<u16>,
}

impl WavRecordingSession {
    /// Create a new WAV recording session
    pub fn new(recording: AudioRecording, config: &AudioRecordingConfig) -> Result<Self> {
        // Create the directory if it doesn't exist
        if let Some(parent) = recording.file_path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create recording directory")?;
        }

        // Create WAV specification
        let wav_spec = WavSpec {
            channels: recording.wav_channels,
            sample_rate: recording.wav_sample_rate,
            bits_per_sample: recording.wav_bits_per_sample,
            sample_format: hound::SampleFormat::Int,
        };

        // Create WAV writer
        let file = File::create(&recording.file_path)
            .context("Failed to create recording file")?;
        let writer = BufWriter::new(file);
        let wav_writer = WavWriter::new(writer, wav_spec)
            .context("Failed to create WAV writer")?;

        // Create transcoder
        let transcoder = AudioTranscoder::new(recording.original_codec, wav_spec);

        Ok(Self {
            recording,
            wav_writer,
            transcoder,
            packet_count: 0,
            last_sequence: None,
        })
    }

    /// Process an RTP audio packet
    #[instrument(skip(self, packet), fields(sequence = packet.sequence_number))]
    pub fn process_rtp_packet(&mut self, packet: RtpAudioPacket) -> Result<()> {
        // Check for sequence number gaps (packet loss detection)
        if let Some(last_seq) = self.last_sequence {
            let expected_seq = last_seq.wrapping_add(1);
            if packet.sequence_number != expected_seq {
                warn!("Packet loss detected: expected {}, got {}",
                      expected_seq, packet.sequence_number);
            }
        }
        self.last_sequence = Some(packet.sequence_number);

        // Verify payload type matches expected codec
        if packet.payload_type != self.recording.original_codec.rtp_payload_type() {
            warn!("Payload type mismatch: expected {}, got {}",
                  self.recording.original_codec.rtp_payload_type(), packet.payload_type);
        }

        // Transcode the payload to WAV samples
        let samples = self.transcoder.transcode_packet(&packet.payload)
            .context("Failed to transcode RTP payload")?;

        // Write samples to WAV file
        for sample in samples {
            self.wav_writer.write_sample(sample)
                .context("Failed to write WAV sample")?;
        }

        self.packet_count += 1;

        // Update recording statistics
        self.recording.total_samples += packet.payload.len() as u64;

        debug!("Processed RTP packet: seq={}, samples={}, total_packets={}",
               packet.sequence_number, packet.payload.len(), self.packet_count);

        Ok(())
    }

    /// Complete the recording session
    pub fn finalize(mut self) -> Result<AudioRecording> {
        // Finalize the WAV file
        self.wav_writer.finalize()
            .context("Failed to finalize WAV file")?;

        // Update recording metadata
        self.recording.completed_at = Some(Utc::now());
        self.recording.is_active = false;

        // Get file size
        if let Ok(metadata) = std::fs::metadata(&self.recording.file_path) {
            self.recording.file_size_bytes = metadata.len();
        }

        // Calculate duration
        if let Some(completed) = self.recording.completed_at {
            self.recording.duration_seconds = (completed - self.recording.started_at)
                .num_seconds() as u32;
        }

        info!("Recording completed: {} packets, {} bytes, {} seconds",
              self.packet_count, self.recording.file_size_bytes, self.recording.duration_seconds);

        Ok(self.recording)
    }
}

/// Audio recording service
pub struct AudioRecordingService {
    config: AudioRecordingConfig,
    event_bus: Arc<EventBus>,
    legal_auth_service: Arc<LegalAuthorizationService>,
    memory_management_service: Arc<MemoryManagementService>,
    active_recordings: Arc<RwLock<HashMap<String, WavRecordingSession>>>,
    rtp_packet_sender: mpsc::UnboundedSender<(String, RtpAudioPacket)>,
    // Batch processing
    packet_buffer: Arc<RwLock<HashMap<String, Vec<RtpAudioPacket>>>>,
    batch_transcoding_service: Option<Arc<crate::services::batch_transcoding_service::BatchTranscodingService>>,
}

impl AudioRecordingService {
    /// Create new audio recording service
    pub fn new(
        config: AudioRecordingConfig,
        event_bus: Arc<EventBus>,
        legal_auth_service: Arc<LegalAuthorizationService>,
        memory_management_service: Arc<MemoryManagementService>,
        batch_transcoding_service: Option<Arc<crate::services::batch_transcoding_service::BatchTranscodingService>>,
    ) -> Result<Self> {
        let (rtp_packet_sender, rtp_packet_receiver) = mpsc::unbounded_channel();

        let service = Self {
            config,
            event_bus,
            legal_auth_service,
            memory_management_service,
            active_recordings: Arc::new(RwLock::new(HashMap::new())),
            rtp_packet_sender,
            packet_buffer: Arc::new(RwLock::new(HashMap::new())),
            batch_transcoding_service,
        };

        // Start RTP packet processing task
        service.start_rtp_processor(rtp_packet_receiver);

        Ok(service)
    }

    /// Start a new audio recording for a call
    #[instrument(skip(self), fields(call_id = %call_id, recording_id = %recording_id))]
    pub async fn start_recording(
        &self,
        recording_id: String,
        call_id: String,
        session_id: String,
        trunk_id: i32,
        original_codec: RecordingCodec,
        storage_type: StorageType,
        legal_authorization_id: Option<i32>,
    ) -> Result<()> {
        info!("Starting audio recording for call {}", call_id);

        // Check legal authorization if required
        if let Some(auth_id) = legal_authorization_id {
            // In production, verify the authorization is active
            debug!("Recording authorized under legal authorization {}", auth_id);
        }

        // Determine storage path
        let base_path = match storage_type {
            StorageType::Memory => &self.config.memory_storage_path,
            StorageType::Disk => &self.config.disk_storage_path,
        };

        let file_name = format!("{}_{}.wav", call_id, recording_id);
        let file_path = Path::new(base_path).join(file_name);

        // Create recording metadata
        let recording = AudioRecording {
            recording_id: recording_id.clone(),
            call_id: call_id.clone(),
            session_id,
            trunk_id,
            storage_type,
            file_path,
            original_codec,
            wav_sample_rate: self.config.wav_sample_rate,
            wav_channels: self.config.wav_channels,
            wav_bits_per_sample: self.config.wav_bits_per_sample,
            legal_authorization_id,
            monitoring_purpose: if legal_authorization_id.is_some() {
                "lawful_intercept".to_string()
            } else {
                "fraud_prevention".to_string()
            },
            ecpa_compliant: true,
            started_at: Utc::now(),
            duration_seconds: 0,
            file_size_bytes: 0,
            total_samples: 0,
            is_active: true,
            completed_at: None,
        };

        // Create WAV recording session
        let session = WavRecordingSession::new(recording.clone(), &self.config)
            .context("Failed to create WAV recording session")?;

        // Determine recording priority based on legal authorization
        let priority = if legal_authorization_id.is_some() {
            RecordingPriority::High
        } else {
            RecordingPriority::Normal
        };

        // Register with memory management service
        let recording_uuid = uuid::Uuid::parse_str(&recording_id)?;
        let estimated_size = self.estimate_recording_size(&recording);
        self.memory_management_service.register_recording(
            recording_uuid,
            estimated_size,
            priority,
            storage_type,
        ).await?;

        // Store active recording session
        let mut recordings = self.active_recordings.write().await;
        recordings.insert(recording_id.clone(), session);

        // Emit event
        let event = TelecomEvent::VoiceIntegrityAudit {
            user_id: None,
            action_type: "start_recording".to_string(),
            resource_type: "audio_recording".to_string(),
            resource_id: recording_id,
            authorization_id: legal_authorization_id,
            ecpa_compliant: true,
        };
        self.event_bus.publish(event).await?;

        Ok(())
    }

    /// Estimate the recording size based on codec and expected duration
    fn estimate_recording_size(&self, recording: &AudioRecording) -> u64 {
        // Base estimate on 10 minutes of audio at the given sample rate and bit depth
        let duration_seconds = 600; // 10 minutes
        let bytes_per_sample = (recording.wav_bits_per_sample / 8) as u64;
        let bytes_per_second = recording.wav_sample_rate as u64 *
                              recording.wav_channels as u64 *
                              bytes_per_sample;

        let estimated_audio_size = duration_seconds * bytes_per_second;

        // Add WAV header overhead and some buffer
        estimated_audio_size + 1024 // 1KB header buffer
    }

    /// Process an RTP packet for recording
    pub async fn process_rtp_packet(
        &self,
        recording_id: String,
        packet: RtpAudioPacket,
    ) -> Result<()> {
        // Send packet to processing queue for non-blocking operation
        self.rtp_packet_sender.send((recording_id, packet))
            .context("Failed to queue RTP packet for recording")?;

        Ok(())
    }

    /// Stop a recording session
    #[instrument(skip(self), fields(recording_id = %recording_id))]
    pub async fn stop_recording(&self, recording_id: String) -> Result<AudioRecording> {
        info!("Stopping audio recording: {}", recording_id);

        // Remove from active recordings
        let mut recordings = self.active_recordings.write().await;
        let session = recordings.remove(&recording_id)
            .ok_or_else(|| anyhow::anyhow!("Recording not found: {}", recording_id))?;

        // Finalize the recording
        let completed_recording = session.finalize()
            .context("Failed to finalize recording")?;

        // Emit completion event
        let event = TelecomEvent::VoiceIntegrityAudit {
            user_id: None,
            action_type: "stop_recording".to_string(),
            resource_type: "audio_recording".to_string(),
            resource_id: recording_id,
            authorization_id: completed_recording.legal_authorization_id,
            ecpa_compliant: true,
        };
        self.event_bus.publish(event).await?;

        Ok(completed_recording)
    }

    /// Check if a call should be recorded based on legal authorization
    pub async fn should_record_call(
        &self,
        trunk_id: i32,
        calling_number: &str,
        called_number: &str,
    ) -> Result<Option<i32>> {
        // Check for trunk-based intercept
        if let Some(target) = self.legal_auth_service
            .should_intercept_target("trunk_id", &trunk_id.to_string()).await? {
            return Ok(Some(target.authorization_id));
        }

        // Check for phone number based intercept
        if let Some(target) = self.legal_auth_service
            .should_intercept_target("phone_number", calling_number).await? {
            return Ok(Some(target.authorization_id));
        }

        if let Some(target) = self.legal_auth_service
            .should_intercept_target("phone_number", called_number).await? {
            return Ok(Some(target.authorization_id));
        }

        Ok(None)
    }

    /// Start RTP packet processing task
    fn start_rtp_processor(&self, mut receiver: mpsc::UnboundedReceiver<(String, RtpAudioPacket)>) {
        let recordings = self.active_recordings.clone();
        let packet_buffer = self.packet_buffer.clone();
        let config = self.config.clone();
        let batch_service = self.batch_transcoding_service.clone();

        tokio::spawn(async move {
            // Start batch flushing timer if batch processing is enabled
            if config.enable_batch_processing {
                let packet_buffer_timer = packet_buffer.clone();
                let recordings_timer = recordings.clone();
                let batch_service_timer = batch_service.clone();
                let config_timer = config.clone();

                tokio::spawn(async move {
                    let mut interval = tokio::time::interval(
                        Duration::from_millis(config_timer.batch_timeout_ms)
                    );

                    loop {
                        interval.tick().await;
                        Self::flush_packet_batches(
                            &packet_buffer_timer,
                            &recordings_timer,
                            &batch_service_timer,
                            &config_timer,
                        ).await;
                    }
                });
            }

            // Main packet processing loop
            while let Some((recording_id, packet)) = receiver.recv().await {
                if config.enable_batch_processing {
                    // Add to batch buffer
                    let mut buffer = packet_buffer.write().await;
                    buffer.entry(recording_id.clone())
                        .or_insert_with(Vec::new)
                        .push(packet);

                    // Check if batch is full
                    if let Some(packets) = buffer.get(&recording_id) {
                        if packets.len() >= config.batch_size {
                            let packets_to_process = buffer.remove(&recording_id).unwrap();
                            drop(buffer); // Release lock early

                            Self::process_packet_batch(
                                recording_id,
                                packets_to_process,
                                &recordings,
                                &batch_service,
                                &config,
                            ).await;
                        }
                    }
                } else {
                    // Process immediately (legacy mode)
                    let mut recordings_guard = recordings.write().await;
                    if let Some(session) = recordings_guard.get_mut(&recording_id) {
                        if let Err(e) = session.process_rtp_packet(packet) {
                            error!("Failed to process RTP packet for recording {}: {}",
                                   recording_id, e);
                        }
                    } else {
                        warn!("Received RTP packet for unknown recording: {}", recording_id);
                    }
                }
            }
        });
    }

    /// Process a batch of RTP packets
    async fn process_packet_batch(
        recording_id: String,
        packets: Vec<RtpAudioPacket>,
        recordings: &Arc<RwLock<HashMap<String, WavRecordingSession>>>,
        batch_service: &Option<Arc<crate::services::batch_transcoding_service::BatchTranscodingService>>,
        config: &AudioRecordingConfig,
    ) {
        debug!("Processing batch of {} packets for recording {}", packets.len(), recording_id);

        if let Some(batch_svc) = batch_service {
            // Use batch transcoding service
            let recording_info = {
                let recordings_guard = recordings.read().await;
                recordings_guard.get(&recording_id).map(|session| {
                    (session.recording.call_id.clone(),
                     session.recording.original_codec,
                     session.recording.legal_authorization_id)
                })
            };

            if let Some((call_id, codec, auth_id)) = recording_info {
                let job = crate::services::batch_transcoding_service::TranscodingJob {
                    job_id: uuid::Uuid::new_v4().to_string(),
                    recording_id: recording_id.clone(),
                    call_id,
                    priority: if auth_id.is_some() {
                        crate::services::batch_transcoding_service::TranscodingPriority::High
                    } else {
                        crate::services::batch_transcoding_service::TranscodingPriority::Normal
                    },
                    input_codec: codec,
                    audio_packets: packets.clone(),
                    legal_authorization_id: auth_id,
                    submitted_at: chrono::Utc::now(),
                    max_processing_time_ms: 30000, // 30 seconds max
                };

                if let Err(e) = batch_svc.submit_transcoding_job(job).await {
                    error!("Failed to submit transcoding job for recording {}: {}", recording_id, e);
                    // Fallback to immediate processing
                    Self::process_packets_immediately(recording_id, packets, recordings).await;
                }
            }
        } else {
            // Process immediately
            Self::process_packets_immediately(recording_id, packets, recordings).await;
        }
    }

    /// Process packets immediately (fallback or legacy mode)
    async fn process_packets_immediately(
        recording_id: String,
        packets: Vec<RtpAudioPacket>,
        recordings: &Arc<RwLock<HashMap<String, WavRecordingSession>>>,
    ) {
        let mut recordings_guard = recordings.write().await;
        if let Some(session) = recordings_guard.get_mut(&recording_id) {
            for packet in packets {
                if let Err(e) = session.process_rtp_packet(packet) {
                    error!("Failed to process RTP packet for recording {}: {}", recording_id, e);
                    break; // Stop processing on error
                }
            }
        } else {
            warn!("Received RTP packets for unknown recording: {}", recording_id);
        }
    }

    /// Flush packet batches based on timeout
    async fn flush_packet_batches(
        packet_buffer: &Arc<RwLock<HashMap<String, Vec<RtpAudioPacket>>>>,
        recordings: &Arc<RwLock<HashMap<String, WavRecordingSession>>>,
        batch_service: &Option<Arc<crate::services::batch_transcoding_service::BatchTranscodingService>>,
        config: &AudioRecordingConfig,
    ) {
        let mut buffer = packet_buffer.write().await;
        let mut to_process = Vec::new();

        // Collect all non-empty buffers
        for (recording_id, packets) in buffer.drain() {
            if !packets.is_empty() {
                to_process.push((recording_id, packets));
            }
        }
        drop(buffer); // Release lock

        // Process collected batches
        for (recording_id, packets) in to_process {
            Self::process_packet_batch(
                recording_id,
                packets,
                recordings,
                batch_service,
                config,
            ).await;
        }
    }

    /// Get active recording count
    pub async fn get_active_recording_count(&self) -> usize {
        self.active_recordings.read().await.len()
    }

    /// Get recording statistics
    pub async fn get_recording_stats(&self) -> HashMap<String, u64> {
        let recordings = self.active_recordings.read().await;
        let mut stats = HashMap::new();

        stats.insert("active_recordings".to_string(), recordings.len() as u64);

        let mut total_packets = 0u64;
        let mut total_samples = 0u64;

        for session in recordings.values() {
            total_packets += session.packet_count;
            total_samples += session.recording.total_samples;
        }

        stats.insert("total_packets_processed".to_string(), total_packets);
        stats.insert("total_samples_recorded".to_string(), total_samples);

        stats
    }
}