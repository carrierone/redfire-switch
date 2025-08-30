/*
 * TDMoE DTMF Integration
 *
 * This module integrates the comprehensive DTMF functionality with the existing
 * TDMoE (Time Division Multiplexing over Ethernet) implementation, providing:
 * - DTMF detection from TDM voice channels
 * - DTMF generation to TDM voice channels
 * - Cross-protocol DTMF transport (TDMoE <-> SIP/RTP)
 * - Integration with NI-2 signaling for DTMF events
 * - Performance optimization for real-time TDM processing
 */

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, info};

use crate::dtmf_processor::{
    DtmfDetectorConfig, DtmfEvent, DtmfGeneratorConfig, DtmfProcessor, DtmfSource,
};
use crate::rfc2833_events::Rfc2833Processor;
use crate::sigtran_dtmf::SigtranDtmfProcessor;
use crate::sip_info_dtmf::SipInfoDtmfProcessor;
use crate::tdmoe_ni2_signaling::{Ni2Message, TdmoeNi2Signaling};

/// TDMoE channel configuration for DTMF processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TdmoeDtmfChannelConfig {
    /// Channel identifier (e.g., "T1-1-1" for T1 span 1, channel 1)
    pub channel_id: String,
    /// Physical span number
    pub span_number: u8,
    /// Channel number within span
    pub channel_number: u8,
    /// Enable DTMF detection on this channel
    pub enable_detection: bool,
    /// Enable DTMF generation on this channel
    pub enable_generation: bool,
    /// Detection sensitivity (0.0-1.0)
    pub detection_sensitivity: f32,
    /// Generation amplitude (0.0-1.0)
    pub generation_amplitude: f32,
    /// Associated SIP call ID (if any)
    pub sip_call_id: Option<String>,
    /// Associated B2BUA leg identifier
    pub b2bua_leg_id: Option<String>,
}

impl Default for TdmoeDtmfChannelConfig {
    fn default() -> Self {
        Self {
            channel_id: String::new(),
            span_number: 0,
            channel_number: 0,
            enable_detection: true,
            enable_generation: true,
            detection_sensitivity: 0.8,
            generation_amplitude: 0.6,
            sip_call_id: None,
            b2bua_leg_id: None,
        }
    }
}

/// TDMoE DTMF events for integration with signaling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TdmoeDtmfEvent {
    /// DTMF digit detected on TDM channel
    TdmDigitDetected {
        channel_id: String,
        digit: char,
        duration: Duration,
        #[serde(skip, default = "std::time::Instant::now")]
        timestamp: Instant,
        confidence: f32,
    },
    /// DTMF sequence completed on TDM channel
    TdmSequenceComplete {
        channel_id: String,
        sequence: String,
        total_duration: Duration,
    },
    /// Cross-protocol DTMF relay (TDM -> SIP)
    DtmfRelaySipOut {
        tdm_channel: String,
        sip_call_id: String,
        digit: char,
        transport_method: String, // "RFC2833", "SIP_INFO", etc.
    },
    /// Cross-protocol DTMF relay (SIP -> TDM)
    DtmfRelaySipIn {
        sip_call_id: String,
        tdm_channel: String,
        digit: char,
        source_method: String,
    },
    /// NI-2 signaling integration
    Ni2DtmfSignaling {
        channel_id: String,
        message_type: String,
        digits: String,
    },
}

/// TDMoE DTMF Integration processor
pub struct TdmoeDtmfIntegration {
    /// Core DTMF processor
    dtmf_processor: DtmfProcessor,
    /// RFC2833 processor for SIP integration
    rfc2833_processor: Rfc2833Processor,
    /// SIP INFO processor
    sip_info_processor: SipInfoDtmfProcessor,
    /// Sigtran processor
    sigtran_processor: SigtranDtmfProcessor,
    /// NI-2 signaling integration
    ni2_signaling: Arc<TdmoeNi2Signaling>,
    /// TDM channel configurations
    channel_configs: Arc<RwLock<HashMap<String, TdmoeDtmfChannelConfig>>>,
    /// Active TDM to SIP call mappings
    call_mappings: Arc<RwLock<HashMap<String, String>>>, // TDM channel -> SIP call ID
    /// Event publisher for integration events
    event_publisher: broadcast::Sender<TdmoeDtmfEvent>,
    /// Internal event receiver from DTMF processors
    dtmf_event_receiver: Arc<RwLock<Option<mpsc::UnboundedReceiver<DtmfEvent>>>>,
    /// Audio sample buffer for TDM channels
    audio_buffers: Arc<RwLock<HashMap<String, Vec<f32>>>>,
    /// Performance statistics
    stats: Arc<RwLock<TdmoeDtmfStats>>,
}

/// Performance and operational statistics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TdmoeDtmfStats {
    /// Total TDM channels configured
    pub total_channels: usize,
    /// Active detection channels
    pub active_detection_channels: usize,
    /// Active generation channels  
    pub active_generation_channels: usize,
    /// Total digits detected from TDM
    pub total_tdm_digits: u64,
    /// Total digits generated to TDM
    pub total_generated_digits: u64,
    /// Cross-protocol relays (TDM -> SIP)
    pub tdm_to_sip_relays: u64,
    /// Cross-protocol relays (SIP -> TDM)
    pub sip_to_tdm_relays: u64,
    /// NI-2 signaling messages processed
    pub ni2_messages: u64,
    /// Average detection latency (milliseconds)
    pub avg_detection_latency_ms: f64,
    /// Audio processing performance (samples/sec)
    pub audio_samples_per_second: f64,
}

impl TdmoeDtmfIntegration {
    /// Create new TDMoE DTMF integration
    pub async fn new(ni2_signaling: Arc<TdmoeNi2Signaling>) -> Result<Self> {
        // Setup shared DTMF event channel
        let (dtmf_event_sender, dtmf_event_receiver) = mpsc::unbounded_channel();

        // Create DTMF processor optimized for TDMoE (8kHz, real-time processing)
        let detector_config = DtmfDetectorConfig {
            sample_rate: 8000,
            min_tone_duration: 40,
            max_tone_duration: 2000,
            min_inter_digit_silence: 40,
            confidence_threshold: 0.7,
            block_size: 80, // 10ms blocks for low latency
            enable_extended: true,
            twist_tolerance: 8.0,
            reverse_twist_tolerance: 4.0,
            liberal_dtmf: true,
            source_priority: std::collections::HashMap::new(),
            itu_compliance: true,
            snr_threshold: 15.0,
        };
        let generator_config = DtmfGeneratorConfig {
            sample_rate: 8000,
            default_tone_duration: 100,
            default_inter_digit_silence: 100,
            default_amplitude: 0.6,
            enable_shaping: true,
        };
        let dtmf_processor = DtmfProcessor::with_config(detector_config, generator_config);

        // Create protocol processors with shared event sender
        let mut rfc2833_processor = Rfc2833Processor::new(dtmf_event_sender.clone());
        rfc2833_processor.add_payload_type(
            101,
            crate::rfc2833_events::Rfc2833PayloadType::TelephoneEvent(101),
        );

        let sip_info_processor = SipInfoDtmfProcessor::new(dtmf_event_sender.clone());

        let sigtran_config = crate::sigtran_dtmf::SigtranDtmfConfig::default();
        let sigtran_processor = SigtranDtmfProcessor::new(dtmf_event_sender, sigtran_config);

        // Create integration event channel
        let (event_publisher, _) = broadcast::channel(1000);

        Ok(Self {
            dtmf_processor,
            rfc2833_processor,
            sip_info_processor,
            sigtran_processor,
            ni2_signaling,
            channel_configs: Arc::new(RwLock::new(HashMap::new())),
            call_mappings: Arc::new(RwLock::new(HashMap::new())),
            event_publisher,
            dtmf_event_receiver: Arc::new(RwLock::new(Some(dtmf_event_receiver))),
            audio_buffers: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(TdmoeDtmfStats::default())),
        })
    }

    /// Add TDM channel for DTMF processing
    pub async fn add_tdm_channel(&self, config: TdmoeDtmfChannelConfig) -> Result<()> {
        let channel_id = config.channel_id.clone();

        // Add to DTMF detector if detection enabled
        if config.enable_detection {
            let detector = self.dtmf_processor.detector();
            let channel_num = self.channel_to_number(&channel_id)?;
            detector.add_channel(channel_num)?;

            // Initialize audio buffer
            let mut buffers = self.audio_buffers.write().await;
            buffers.insert(channel_id.clone(), Vec::with_capacity(8000)); // 1 second buffer
        }

        // Store configuration
        let mut configs = self.channel_configs.write().await;
        configs.insert(channel_id.clone(), config);

        // Update statistics
        let mut stats = self.stats.write().await;
        stats.total_channels = configs.len();
        stats.active_detection_channels = configs.values().filter(|c| c.enable_detection).count();
        stats.active_generation_channels = configs.values().filter(|c| c.enable_generation).count();

        info!("Added TDM channel '{}' for DTMF processing", channel_id);
        Ok(())
    }

    /// Remove TDM channel from DTMF processing
    pub async fn remove_tdm_channel(&self, channel_id: &str) -> Result<()> {
        // Remove from DTMF detector
        let detector = self.dtmf_processor.detector();
        let channel_num = self.channel_to_number(channel_id)?;
        detector.remove_channel(channel_num)?;

        // Remove audio buffer
        let mut buffers = self.audio_buffers.write().await;
        buffers.remove(channel_id);

        // Remove configuration
        let mut configs = self.channel_configs.write().await;
        configs.remove(channel_id);

        // Remove call mapping if exists
        let mut mappings = self.call_mappings.write().await;
        mappings.retain(|_, v| v != channel_id);

        info!("Removed TDM channel '{}' from DTMF processing", channel_id);
        Ok(())
    }

    /// Process TDM audio samples for DTMF detection
    pub async fn process_tdm_audio(&self, channel_id: &str, samples: &[i16]) -> Result<()> {
        let configs = self.channel_configs.read().await;
        let config = configs
            .get(channel_id)
            .ok_or_else(|| anyhow!("TDM channel '{}' not configured for DTMF", channel_id))?;

        if !config.enable_detection {
            return Ok(()); // Detection disabled for this channel
        }

        // Convert i16 samples to f32 and apply sensitivity scaling
        let mut audio_buffers = self.audio_buffers.write().await;
        let buffer = audio_buffers
            .get_mut(channel_id)
            .ok_or_else(|| anyhow!("Audio buffer not found for channel '{}'", channel_id))?;

        // Convert and scale samples
        let f32_samples: Vec<f32> = samples
            .iter()
            .map(|&sample| {
                let normalized = sample as f32 / 32768.0; // Convert i16 to f32 [-1.0, 1.0]
                normalized * config.detection_sensitivity
            })
            .collect();

        // Add to buffer
        buffer.extend_from_slice(&f32_samples);

        // Process in chunks if buffer is large enough
        const CHUNK_SIZE: usize = 80; // 10ms at 8kHz
        while buffer.len() >= CHUNK_SIZE {
            let chunk: Vec<f32> = buffer.drain(0..CHUNK_SIZE).collect();

            // Process with DTMF detector
            let detector = self.dtmf_processor.detector();
            // Convert f32 samples to u8 for detector
            let u8_chunk: Vec<u8> = chunk
                .iter()
                .map(|&s| ((s * 128.0 + 128.0).clamp(0.0, 255.0) as u8))
                .collect();
            let channel_num = self.channel_to_number(channel_id)?;
            detector.process_audio(channel_num, &u8_chunk)?;
        }

        // Update performance statistics
        let mut stats = self.stats.write().await;
        stats.audio_samples_per_second = samples.len() as f64; // Simplified calculation

        Ok(())
    }

    /// Generate DTMF tones to TDM channel
    pub async fn generate_tdm_dtmf(
        &self,
        channel_id: &str,
        digit: char,
        duration: Option<Duration>,
    ) -> Result<Vec<i16>> {
        let configs = self.channel_configs.read().await;
        let config = configs
            .get(channel_id)
            .ok_or_else(|| anyhow!("TDM channel '{}' not configured for DTMF", channel_id))?;

        if !config.enable_generation {
            return Err(anyhow!(
                "DTMF generation disabled for channel '{}'",
                channel_id
            ));
        }

        // Generate DTMF samples
        let generator = self.dtmf_processor.generator();
        let duration_ms = duration.map(|d| d.as_millis() as u32).unwrap_or(100); // Default 100ms
        let f32_samples = generator.generate_digit(digit, duration_ms)?;

        // Convert f32 samples to i16 for TDM output
        let i16_samples: Vec<i16> = f32_samples
            .iter()
            .map(|&sample| {
                let scaled = sample * 32767.0;
                scaled.clamp(-32768.0, 32767.0) as i16
            })
            .collect();

        // Update statistics
        let mut stats = self.stats.write().await;
        stats.total_generated_digits += 1;

        // Publish event
        let event = TdmoeDtmfEvent::TdmDigitDetected {
            channel_id: channel_id.to_string(),
            digit,
            duration: duration.unwrap_or(Duration::from_millis(100)),
            timestamp: Instant::now(),
            confidence: 1.0, // Generated digits have perfect confidence
        };

        if let Err(e) = self.event_publisher.send(event) {
            debug!("No subscribers for TDMoE DTMF event: {}", e);
        }

        info!(
            "Generated DTMF digit '{}' for TDM channel '{}' ({} samples)",
            digit,
            channel_id,
            i16_samples.len()
        );

        Ok(i16_samples)
    }

    /// Associate TDM channel with SIP call for cross-protocol DTMF relay
    pub async fn associate_sip_call(&self, tdm_channel: &str, sip_call_id: &str) -> Result<()> {
        let mut mappings = self.call_mappings.write().await;
        mappings.insert(tdm_channel.to_string(), sip_call_id.to_string());

        // Update channel configuration
        let mut configs = self.channel_configs.write().await;
        if let Some(config) = configs.get_mut(tdm_channel) {
            config.sip_call_id = Some(sip_call_id.to_string());
        }

        info!(
            "Associated TDM channel '{}' with SIP call '{}'",
            tdm_channel, sip_call_id
        );
        Ok(())
    }

    /// Relay DTMF from TDM to SIP via RFC2833
    pub async fn relay_dtmf_to_rfc2833(
        &self,
        tdm_channel: &str,
        digit: char,
        duration_ms: u32,
    ) -> Result<()> {
        let mappings = self.call_mappings.read().await;
        let sip_call_id = mappings
            .get(tdm_channel)
            .ok_or_else(|| anyhow!("No SIP call associated with TDM channel '{}'", tdm_channel))?;

        // Generate RFC2833 packets
        let packets = self
            .rfc2833_processor
            .generate_outgoing_packets(
                sip_call_id,
                digit,
                duration_ms,
                20, // Volume level
                0,  // Start timestamp (would be real RTP timestamp in production)
            )
            .await?;

        // In a real implementation, these packets would be sent via RTP
        // For now, we'll just log the generation
        info!(
            "Generated {} RFC2833 packets for DTMF '{}' (TDM {} -> SIP {})",
            packets.len(),
            digit,
            tdm_channel,
            sip_call_id
        );

        // Update statistics and publish event
        let mut stats = self.stats.write().await;
        stats.tdm_to_sip_relays += 1;

        let event = TdmoeDtmfEvent::DtmfRelaySipOut {
            tdm_channel: tdm_channel.to_string(),
            sip_call_id: sip_call_id.clone(),
            digit,
            transport_method: "RFC2833".to_string(),
        };

        if let Err(e) = self.event_publisher.send(event) {
            debug!("No subscribers for DTMF relay event: {}", e);
        }

        Ok(())
    }

    /// Relay DTMF from SIP to TDM
    pub async fn relay_dtmf_from_sip(
        &self,
        sip_call_id: &str,
        digit: char,
        duration_ms: u32,
    ) -> Result<()> {
        // Find TDM channel associated with this SIP call
        let mappings = self.call_mappings.read().await;
        let tdm_channel = mappings
            .iter()
            .find(|(_, call_id)| *call_id == sip_call_id)
            .map(|(channel, _)| channel.clone())
            .ok_or_else(|| anyhow!("No TDM channel associated with SIP call '{}'", sip_call_id))?;

        // Generate DTMF to TDM channel
        let _samples = self
            .generate_tdm_dtmf(
                &tdm_channel,
                digit,
                Some(Duration::from_millis(duration_ms as u64)),
            )
            .await?;

        // Update statistics and publish event
        let mut stats = self.stats.write().await;
        stats.sip_to_tdm_relays += 1;

        let event = TdmoeDtmfEvent::DtmfRelaySipIn {
            sip_call_id: sip_call_id.to_string(),
            tdm_channel: tdm_channel.clone(),
            digit,
            source_method: "SIP_RELAY".to_string(),
        };

        if let Err(e) = self.event_publisher.send(event) {
            debug!("No subscribers for DTMF relay event: {}", e);
        }

        info!(
            "Relayed DTMF '{}' from SIP '{}' to TDM '{}'",
            digit, sip_call_id, tdm_channel
        );
        Ok(())
    }

    /// Integrate with NI-2 signaling for DTMF events
    pub async fn process_ni2_dtmf_signaling(&self, channel_id: &str, digits: &str) -> Result<()> {
        // Create NI-2 message for DTMF transport
        let ni2_message = Ni2Message {
            message_type: crate::tdmoe_ni2_signaling::Ni2MessageType::CPG, // Call Progress for DTMF
            cic: 1, // Circuit Identification Code
            calling_number: None,
            called_number: Some(digits.to_string()),
            oli: None,
            charge_number: None,
            lrn: None,
            jip: None,
            parameters: std::collections::HashMap::new(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        // Send via NI-2 signaling (this would actually transmit over TDM D-channel)
        // self.ni2_signaling.send_message(channel_id, ni2_message).await?;

        // Update statistics and publish event
        let mut stats = self.stats.write().await;
        stats.ni2_messages += 1;

        let event = TdmoeDtmfEvent::Ni2DtmfSignaling {
            channel_id: channel_id.to_string(),
            message_type: "InformationRequest".to_string(),
            digits: digits.to_string(),
        };

        if let Err(e) = self.event_publisher.send(event) {
            debug!("No subscribers for NI-2 DTMF event: {}", e);
        }

        info!(
            "Processed NI-2 DTMF signaling for channel '{}': '{}'",
            channel_id, digits
        );
        Ok(())
    }

    /// Start the event processing loop
    pub async fn start_event_processing(&self) -> Result<()> {
        let mut dtmf_receiver_guard = self.dtmf_event_receiver.write().await;
        let mut dtmf_receiver = dtmf_receiver_guard
            .take()
            .ok_or_else(|| anyhow!("Event processing already started"))?;
        drop(dtmf_receiver_guard);

        let event_publisher = self.event_publisher.clone();
        let call_mappings = Arc::clone(&self.call_mappings);
        let stats = Arc::clone(&self.stats);

        // Spawn event processing task
        tokio::spawn(async move {
            while let Some(event) = dtmf_receiver.recv().await {
                match event {
                    DtmfEvent::DigitDetected {
                        digit,
                        duration,
                        timestamp,
                        confidence,
                        source,
                    } => {
                        if source == DtmfSource::TdmoeVoice {
                            // Update statistics
                            let mut stats_guard = stats.write().await;
                            stats_guard.total_tdm_digits += 1;

                            // TODO: Calculate actual latency
                            stats_guard.avg_detection_latency_ms = 50.0; // Placeholder
                            drop(stats_guard);

                            // Find associated channel and relay if needed
                            let mappings = call_mappings.read().await;
                            for (channel_id, _sip_call_id) in mappings.iter() {
                                // In a real implementation, we'd match by actual channel context
                                let tdm_event = TdmoeDtmfEvent::TdmDigitDetected {
                                    channel_id: channel_id.clone(),
                                    digit,
                                    duration,
                                    timestamp,
                                    confidence,
                                };

                                if let Err(e) = event_publisher.send(tdm_event) {
                                    debug!("No subscribers for TDM DTMF event: {}", e);
                                }
                                break; // Just process first mapping for demo
                            }
                        }
                    }
                    DtmfEvent::SequenceComplete {
                        sequence,
                        total_duration,
                        source,
                    } => {
                        if source == DtmfSource::TdmoeVoice {
                            let mappings = call_mappings.read().await;
                            for (channel_id, _) in mappings.iter() {
                                let tdm_event = TdmoeDtmfEvent::TdmSequenceComplete {
                                    channel_id: channel_id.clone(),
                                    sequence: sequence.clone(),
                                    total_duration,
                                };

                                if let Err(e) = event_publisher.send(tdm_event) {
                                    debug!("No subscribers for TDM sequence event: {}", e);
                                }
                                break;
                            }
                        }
                    }
                }
            }
        });

        info!("Started TDMoE DTMF event processing");
        Ok(())
    }

    /// Subscribe to TDMoE DTMF integration events
    pub fn subscribe_events(&self) -> broadcast::Receiver<TdmoeDtmfEvent> {
        self.event_publisher.subscribe()
    }

    /// Get current performance statistics
    pub async fn get_statistics(&self) -> TdmoeDtmfStats {
        self.stats.read().await.clone()
    }

    /// Get channel configuration
    pub async fn get_channel_config(&self, channel_id: &str) -> Option<TdmoeDtmfChannelConfig> {
        let configs = self.channel_configs.read().await;
        configs.get(channel_id).cloned()
    }

    /// List all configured TDM channels
    pub async fn list_channels(&self) -> Vec<String> {
        let configs = self.channel_configs.read().await;
        configs.keys().cloned().collect()
    }

    /// Convert channel ID to numeric ID for DTMF processor
    fn channel_to_number(&self, channel_id: &str) -> Result<u32> {
        // Parse channel ID format (e.g., "T1-1-1" -> span 1, channel 1)
        let parts: Vec<&str> = channel_id.split('-').collect();
        if parts.len() >= 3 {
            let span = parts[1].parse::<u32>().unwrap_or(1);
            let channel = parts[2].parse::<u32>().unwrap_or(1);
            Ok((span - 1) * 24 + channel) // T1 has 24 channels
        } else {
            // Fallback: use hash of channel ID
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            channel_id.hash(&mut hasher);
            Ok((hasher.finish() % 1000) as u32)
        }
    }
}
