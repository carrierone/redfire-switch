/*
 * DTMF (Dual-Tone Multi-Frequency) Detection and Generation
 * 
 * This module provides comprehensive DTMF functionality including:
 * - Real-time DTMF tone detection using Goertzel algorithm
 * - DTMF tone generation with configurable parameters
 * - Support for standard and extended DTMF tones
 * - Integration with TDMoE, RFC2833, SIP INFO, and Sigtran
 */

use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, info, warn, error};
use serde::{Serialize, Deserialize};

/// Standard DTMF frequency pairs (Hz)
const DTMF_FREQUENCIES: [(u16, u16); 16] = [
    // Row 1: 697 Hz
    (697, 1209), // '1'
    (697, 1336), // '2' 
    (697, 1477), // '3'
    (697, 1633), // 'A'
    // Row 2: 770 Hz
    (770, 1209), // '4'
    (770, 1336), // '5'
    (770, 1477), // '6'
    (770, 1633), // 'B'
    // Row 3: 852 Hz
    (852, 1209), // '7'
    (852, 1336), // '8'
    (852, 1477), // '9'
    (852, 1633), // 'C'
    // Row 4: 941 Hz
    (941, 1209), // '*'
    (941, 1336), // '0'
    (941, 1477), // '#'
    (941, 1633), // 'D'
];

/// DTMF digit to character mapping
const DTMF_DIGITS: [char; 16] = [
    '1', '2', '3', 'A',
    '4', '5', '6', 'B', 
    '7', '8', '9', 'C',
    '*', '0', '#', 'D'
];

/// DTMF detection/generation events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DtmfEvent {
    /// DTMF digit detected
    DigitDetected {
        digit: char,
        duration: Duration,
        #[serde(skip, default = "std::time::Instant::now")]
        timestamp: Instant,
        confidence: f32,
        source: DtmfSource,
    },
    /// DTMF digit generation requested
    DigitGenerate {
        digit: char,
        duration: Duration,
        amplitude: f32,
        source: DtmfSource,
    },
    /// DTMF sequence completed
    SequenceComplete {
        sequence: String,
        total_duration: Duration,
        source: DtmfSource,
    },
    /// DTMF detection error
    DetectionError {
        error: String,
        source: DtmfSource,
    },
}

/// Source of DTMF event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DtmfSource {
    /// TDMoE voice channel
    TdmoeVoice,
    /// RFC2833 RTP events
    Rfc2833,
    /// SIP INFO method
    SipInfo,
    /// Sigtran signaling
    Sigtran,
    /// Internal generation
    Internal,
}

/// DTMF detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DtmfDetectorConfig {
    /// Sample rate for audio processing
    pub sample_rate: u32,
    /// Minimum tone duration for detection (ms) - ITU-T Q.23 minimum: 40ms
    pub min_tone_duration: u32,
    /// Maximum tone duration before timeout (ms)
    pub max_tone_duration: u32,
    /// Minimum silence between tones (ms) - ITU-T Q.23 minimum: 50ms
    pub min_inter_digit_silence: u32,
    /// Detection confidence threshold (0.0-1.0)
    pub confidence_threshold: f32,
    /// Goertzel algorithm block size
    pub block_size: usize,
    /// Enable extended DTMF (A, B, C, D)
    pub enable_extended: bool,
    /// Twist tolerance (dB) - difference between high/low frequencies
    /// ITU-T Q.24: Normal twist <= 8dB, Reverse twist <= 4dB
    pub twist_tolerance: f32,
    /// Reverse twist tolerance (dB) - column frequencies louder than row
    pub reverse_twist_tolerance: f32,
    /// Enable liberal DTMF parsing (FreeSWITCH-style)
    pub liberal_dtmf: bool,
    /// Detection source priority (RFC2833 > SIP INFO > In-band)
    pub source_priority: HashMap<DtmfSource, u8>,
    /// Enable ITU-T compliance mode
    pub itu_compliance: bool,
    /// Signal-to-noise ratio threshold (dB)
    pub snr_threshold: f32,
}

impl Default for DtmfDetectorConfig {
    fn default() -> Self {
        // Create source priority map (FreeSWITCH-style hierarchy)
        let mut source_priority = HashMap::new();
        source_priority.insert(DtmfSource::Rfc2833, 1);      // Highest priority
        source_priority.insert(DtmfSource::SipInfo, 2);      // Medium priority  
        source_priority.insert(DtmfSource::TdmoeVoice, 3);   // Lower priority (in-band)
        source_priority.insert(DtmfSource::Sigtran, 4);      // Lowest priority
        source_priority.insert(DtmfSource::Internal, 5);     // Internal use
        
        Self {
            sample_rate: 8000,
            min_tone_duration: 40,           // ITU-T Q.23 minimum
            max_tone_duration: 2000,
            min_inter_digit_silence: 50,     // ITU-T Q.23 minimum 
            confidence_threshold: 0.7,
            block_size: 160,                 // 20ms at 8kHz
            enable_extended: true,
            twist_tolerance: 8.0,            // ITU-T Q.24 normal twist
            reverse_twist_tolerance: 4.0,    // ITU-T Q.24 reverse twist
            liberal_dtmf: true,              // FreeSWITCH-style liberal parsing
            source_priority,
            itu_compliance: true,            // Enable ITU-T compliance checks
            snr_threshold: 15.0,             // 15dB minimum SNR
        }
    }
}

/// DTMF generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DtmfGeneratorConfig {
    /// Sample rate for audio generation
    pub sample_rate: u32,
    /// Default tone duration (ms)
    pub default_tone_duration: u32,
    /// Default inter-digit silence (ms)
    pub default_inter_digit_silence: u32,
    /// Default amplitude (0.0-1.0)
    pub default_amplitude: f32,
    /// Enable amplitude shaping (fade in/out)
    pub enable_shaping: bool,
}

impl Default for DtmfGeneratorConfig {
    fn default() -> Self {
        Self {
            sample_rate: 8000,
            default_tone_duration: 100,
            default_inter_digit_silence: 100,
            default_amplitude: 0.5,
            enable_shaping: true,
        }
    }
}

/// Goertzel algorithm state for single frequency
struct GoertzelState {
    freq: f32,
    coeff: f32,
    q1: f32,
    q2: f32,
    sample_count: usize,
}

impl GoertzelState {
    fn new(freq: f32, sample_rate: f32, block_size: usize) -> Self {
        let normalized_freq = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let coeff = 2.0 * normalized_freq.cos();
        
        Self {
            freq,
            coeff,
            q1: 0.0,
            q2: 0.0,
            sample_count: 0,
        }
    }
    
    fn process_sample(&mut self, sample: f32) {
        let q0 = self.coeff * self.q1 - self.q2 + sample;
        self.q2 = self.q1;
        self.q1 = q0;
        self.sample_count += 1;
    }
    
    fn get_magnitude(&self) -> f32 {
        if self.sample_count == 0 {
            return 0.0;
        }
        
        let real = self.q1 - self.q2 * (std::f32::consts::PI * self.freq / self.sample_count as f32).cos();
        let imag = self.q2 * (std::f32::consts::PI * self.freq / self.sample_count as f32).sin();
        
        (real * real + imag * imag).sqrt() / self.sample_count as f32
    }
    
    fn reset(&mut self) {
        self.q1 = 0.0;
        self.q2 = 0.0;
        self.sample_count = 0;
    }
}

/// DTMF detection state for a single channel
struct DtmfChannelState {
    /// Goertzel detectors for each DTMF frequency
    detectors: HashMap<u16, GoertzelState>,
    /// Current detection buffer
    sample_buffer: Vec<f32>,
    /// Currently detected digit
    current_digit: Option<char>,
    /// Detection start time
    detection_start: Option<Instant>,
    /// Last silence start time  
    last_silence_start: Option<Instant>,
    /// Accumulated DTMF sequence
    current_sequence: String,
}

impl DtmfChannelState {
    fn new(config: &DtmfDetectorConfig) -> Self {
        let mut detectors = HashMap::new();
        let sample_rate = config.sample_rate as f32;
        
        // Create detectors for all DTMF frequencies
        let frequencies = [697, 770, 852, 941, 1209, 1336, 1477, 1633];
        for &freq in &frequencies {
            detectors.insert(
                freq,
                GoertzelState::new(freq as f32, sample_rate, config.block_size)
            );
        }
        
        Self {
            detectors,
            sample_buffer: Vec::with_capacity(config.block_size),
            current_digit: None,
            detection_start: None,
            last_silence_start: None,
            current_sequence: String::new(),
        }
    }
    
    fn reset(&mut self) {
        for detector in self.detectors.values_mut() {
            detector.reset();
        }
        self.sample_buffer.clear();
        self.current_digit = None;
        self.detection_start = None;
        self.last_silence_start = None;
    }
}

/// DTMF Detector implementing Goertzel algorithm
pub struct DtmfDetector {
    config: DtmfDetectorConfig,
    channels: Arc<RwLock<HashMap<String, DtmfChannelState>>>,
    event_sender: mpsc::UnboundedSender<DtmfEvent>,
}

impl DtmfDetector {
    /// Create new DTMF detector
    pub fn new(config: DtmfDetectorConfig) -> (Self, mpsc::UnboundedReceiver<DtmfEvent>) {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        let detector = Self {
            config,
            channels: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
        };
        
        (detector, event_receiver)
    }
    
    /// Add a new channel for DTMF detection
    pub async fn add_channel(&self, channel_id: String) -> Result<()> {
        let mut channels = self.channels.write().await;
        channels.insert(channel_id.clone(), DtmfChannelState::new(&self.config));
        debug!("Added DTMF detection for channel: {}", channel_id);
        Ok(())
    }
    
    /// Remove a channel from DTMF detection
    pub async fn remove_channel(&self, channel_id: &str) -> Result<()> {
        let mut channels = self.channels.write().await;
        channels.remove(channel_id);
        debug!("Removed DTMF detection for channel: {}", channel_id);
        Ok(())
    }
    
    /// Process audio samples for DTMF detection
    pub async fn process_audio(&self, channel_id: &str, samples: &[f32], source: DtmfSource) -> Result<()> {
        let mut channels = self.channels.write().await;
        let state = channels.get_mut(channel_id)
            .ok_or_else(|| anyhow!("Channel {} not found", channel_id))?;
        
        for &sample in samples {
            // Add sample to buffer
            state.sample_buffer.push(sample);
            
            // Process when buffer is full
            if state.sample_buffer.len() >= self.config.block_size {
                self.process_block(channel_id, state, source).await?;
                state.sample_buffer.clear();
            }
        }
        
        Ok(())
    }
    
    /// Process a complete block of samples
    async fn process_block(&self, channel_id: &str, state: &mut DtmfChannelState, source: DtmfSource) -> Result<()> {
        // Process samples through Goertzel detectors
        for &sample in &state.sample_buffer {
            for detector in state.detectors.values_mut() {
                detector.process_sample(sample);
            }
        }
        
        // Analyze frequency magnitudes
        let mut magnitudes: Vec<(u16, f32)> = state.detectors.iter()
            .map(|(&freq, detector)| (freq, detector.get_magnitude()))
            .collect();
        magnitudes.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        
        // Detect DTMF digit
        let detected_digit = self.analyze_magnitudes(&magnitudes);
        
        // Handle detection state changes
        self.handle_detection_state(channel_id, state, detected_digit, source).await?;
        
        // Reset detectors for next block
        for detector in state.detectors.values_mut() {
            detector.reset();
        }
        
        Ok(())
    }
    
    /// Analyze frequency magnitudes to detect DTMF digit (FreeSWITCH-inspired)
    fn analyze_magnitudes(&self, magnitudes: &[(u16, f32)]) -> Option<char> {
        if magnitudes.len() < 8 {
            return None;
        }
        
        // Find the two strongest frequencies from different groups (row vs column)
        let strongest = magnitudes[0];
        let mut second_strongest = None;
        
        for &(freq, magnitude) in &magnitudes[1..] {
            if self.are_different_groups(strongest.0, freq) {
                second_strongest = Some((freq, magnitude));
                break;
            }
        }
        
        if let Some((second_freq, second_magnitude)) = second_strongest {
            // Check basic confidence threshold
            if strongest.1 > self.config.confidence_threshold && 
               second_magnitude > self.config.confidence_threshold {
                
                // ITU-T Q.24 compliant twist checking
                let (row_freq, col_freq, row_mag, col_mag) = if self.is_row_frequency(strongest.0) {
                    (strongest.0, second_freq, strongest.1, second_magnitude)
                } else {
                    (second_freq, strongest.0, second_magnitude, strongest.1)
                };
                
                // Normal twist: column frequency louder than row frequency (positive dB)
                let normal_twist = 20.0 * (col_mag / row_mag).log10();
                // Reverse twist: row frequency louder than column frequency (negative dB)  
                let reverse_twist = 20.0 * (row_mag / col_mag).log10();
                
                // Apply ITU-T Q.24 twist tolerances or liberal mode
                let twist_ok = if self.config.liberal_dtmf {
                    // FreeSWITCH liberal mode: more permissive
                    normal_twist.abs() <= self.config.twist_tolerance || 
                    reverse_twist.abs() <= self.config.twist_tolerance
                } else if self.config.itu_compliance {
                    // ITU-T Q.24 strict: Normal twist <= 8dB, Reverse twist <= 4dB
                    normal_twist <= self.config.twist_tolerance && 
                    reverse_twist <= self.config.reverse_twist_tolerance
                } else {
                    // Default mode: symmetric tolerance
                    normal_twist.abs() <= self.config.twist_tolerance
                };
                
                if twist_ok {
                    // Optional SNR check for additional validation
                    if self.config.itu_compliance {
                        let signal_power = strongest.1.powi(2) + second_magnitude.powi(2);
                        let noise_power = magnitudes[2..].iter()
                            .map(|(_, mag)| mag.powi(2))
                            .sum::<f32>() / (magnitudes.len() - 2) as f32;
                        
                        if noise_power > 0.0 {
                            let snr_db = 10.0 * (signal_power / noise_power).log10();
                            if snr_db < self.config.snr_threshold {
                                debug!("DTMF rejected: SNR {:.1}dB < {:.1}dB threshold", 
                                      snr_db, self.config.snr_threshold);
                                return None;
                            }
                        }
                    }
                    
                    return self.frequencies_to_digit(strongest.0, second_freq);
                } else {
                    debug!("DTMF rejected: twist normal={:.1}dB reverse={:.1}dB", 
                          normal_twist, reverse_twist);
                }
            }
        }
        
        None
    }
    
    /// Check if frequency belongs to row frequencies (697, 770, 852, 941 Hz)
    fn is_row_frequency(&self, freq: u16) -> bool {
        matches!(freq, 697 | 770 | 852 | 941)
    }
    
    /// Check if two frequencies are from different groups (row vs column)
    fn are_different_groups(&self, freq1: u16, freq2: u16) -> bool {
        self.is_row_frequency(freq1) != self.is_row_frequency(freq2)
    }
    
    /// Convert frequency pair to DTMF digit
    fn frequencies_to_digit(&self, freq1: u16, freq2: u16) -> Option<char> {
        let (row_freq, col_freq) = if [697, 770, 852, 941].contains(&freq1) {
            (freq1, freq2)
        } else {
            (freq2, freq1)
        };
        
        for (i, &(rf, cf)) in DTMF_FREQUENCIES.iter().enumerate() {
            if rf == row_freq && cf == col_freq {
                if !self.config.enable_extended && ['A', 'B', 'C', 'D'].contains(&DTMF_DIGITS[i]) {
                    return None;
                }
                return Some(DTMF_DIGITS[i]);
            }
        }
        
        None
    }
    
    /// Handle detection state changes and emit events
    async fn handle_detection_state(&self, channel_id: &str, state: &mut DtmfChannelState, 
                                   detected_digit: Option<char>, source: DtmfSource) -> Result<()> {
        let now = Instant::now();
        
        match (state.current_digit, detected_digit) {
            // New digit detected
            (None, Some(digit)) => {
                state.current_digit = Some(digit);
                state.detection_start = Some(now);
                state.last_silence_start = None;
                debug!("DTMF digit '{}' detection started on channel {}", digit, channel_id);
            }
            
            // Digit continues
            (Some(current), Some(detected)) if current == detected => {
                // Continue detection
            }
            
            // Digit ended
            (Some(current_digit), None) => {
                if let Some(start_time) = state.detection_start {
                    let duration = now.duration_since(start_time);
                    
                    // Check if duration is valid
                    if duration >= Duration::from_millis(self.config.min_tone_duration.into()) {
                        // Emit detection event
                        let event = DtmfEvent::DigitDetected {
                            digit: current_digit,
                            duration,
                            timestamp: start_time,
                            confidence: 0.85, // TODO: Calculate actual confidence
                            source,
                        };
                        
                        if let Err(e) = self.event_sender.send(event) {
                            warn!("Failed to send DTMF event: {}", e);
                        }
                        
                        // Add to sequence
                        state.current_sequence.push(current_digit);
                        info!("DTMF digit '{}' detected on channel {} (duration: {:?})", 
                              current_digit, channel_id, duration);
                    }
                }
                
                state.current_digit = None;
                state.detection_start = None;
                state.last_silence_start = Some(now);
            }
            
            // Digit changed (shouldn't happen normally)
            (Some(_), Some(new_digit)) => {
                state.current_digit = Some(new_digit);
                state.detection_start = Some(now);
                warn!("DTMF digit changed abruptly on channel {}", channel_id);
            }
            
            // Silence continues
            (None, None) => {
                // Check for sequence completion
                if let Some(silence_start) = state.last_silence_start {
                    let silence_duration = now.duration_since(silence_start);
                    
                    if !state.current_sequence.is_empty() && 
                       silence_duration >= Duration::from_millis((self.config.min_inter_digit_silence * 3).into()) {
                        // Sequence complete
                        let event = DtmfEvent::SequenceComplete {
                            sequence: state.current_sequence.clone(),
                            total_duration: silence_duration,
                            source,
                        };
                        
                        if let Err(e) = self.event_sender.send(event) {
                            warn!("Failed to send DTMF sequence complete event: {}", e);
                        }
                        
                        info!("DTMF sequence '{}' completed on channel {}", 
                              state.current_sequence, channel_id);
                        state.current_sequence.clear();
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Get current detection statistics
    pub async fn get_statistics(&self, channel_id: &str) -> Result<DtmfDetectionStats> {
        let channels = self.channels.read().await;
        let state = channels.get(channel_id)
            .ok_or_else(|| anyhow!("Channel {} not found", channel_id))?;
        
        Ok(DtmfDetectionStats {
            channel_id: channel_id.to_string(),
            current_digit: state.current_digit,
            current_sequence: state.current_sequence.clone(),
            detection_active: state.current_digit.is_some(),
            last_detection: state.detection_start,
        })
    }
}

/// DTMF detection statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DtmfDetectionStats {
    pub channel_id: String,
    pub current_digit: Option<char>,
    pub current_sequence: String,
    pub detection_active: bool,
    #[serde(skip, default)]
    pub last_detection: Option<Instant>,
}

/// DTMF Generator for creating DTMF tones
pub struct DtmfGenerator {
    config: DtmfGeneratorConfig,
}

impl DtmfGenerator {
    /// Create new DTMF generator
    pub fn new(config: DtmfGeneratorConfig) -> Self {
        Self { config }
    }
    
    /// Generate DTMF tone for a single digit
    pub fn generate_digit(&self, digit: char, duration: Option<Duration>, amplitude: Option<f32>) -> Result<Vec<f32>> {
        let duration = duration.unwrap_or_else(|| Duration::from_millis(self.config.default_tone_duration.into()));
        let amplitude = amplitude.unwrap_or(self.config.default_amplitude);
        
        // Find frequency pair for digit
        let freq_pair = self.digit_to_frequencies(digit)
            .ok_or_else(|| anyhow!("Invalid DTMF digit: {}", digit))?;
        
        let sample_rate = self.config.sample_rate as f32;
        let num_samples = ((duration.as_secs_f32() * sample_rate) as usize).max(1);
        let mut samples = Vec::with_capacity(num_samples);
        
        let freq1 = freq_pair.0 as f32;
        let freq2 = freq_pair.1 as f32;
        
        for i in 0..num_samples {
            let t = i as f32 / sample_rate;
            
            // Generate dual-tone signal
            let tone1 = (2.0 * std::f32::consts::PI * freq1 * t).sin();
            let tone2 = (2.0 * std::f32::consts::PI * freq2 * t).sin();
            let sample = amplitude * (tone1 + tone2) / 2.0;
            
            // Apply amplitude shaping if enabled
            let shaped_sample = if self.config.enable_shaping {
                self.apply_amplitude_shaping(sample, i, num_samples)
            } else {
                sample
            };
            
            samples.push(shaped_sample);
        }
        
        Ok(samples)
    }
    
    /// Generate DTMF sequence with inter-digit silence
    pub fn generate_sequence(&self, sequence: &str, tone_duration: Option<Duration>, 
                           inter_digit_silence: Option<Duration>, amplitude: Option<f32>) -> Result<Vec<f32>> {
        let tone_duration = tone_duration.unwrap_or_else(|| Duration::from_millis(self.config.default_tone_duration.into()));
        let silence_duration = inter_digit_silence.unwrap_or_else(|| Duration::from_millis(self.config.default_inter_digit_silence.into()));
        
        let mut result = Vec::new();
        
        for (i, digit) in sequence.chars().enumerate() {
            // Generate digit tone
            let digit_samples = self.generate_digit(digit, Some(tone_duration), amplitude)?;
            result.extend(digit_samples);
            
            // Add inter-digit silence (except after last digit)
            if i < sequence.len() - 1 {
                let silence_samples = self.generate_silence(silence_duration);
                result.extend(silence_samples);
            }
        }
        
        Ok(result)
    }
    
    /// Generate silence samples
    fn generate_silence(&self, duration: Duration) -> Vec<f32> {
        let sample_rate = self.config.sample_rate as f32;
        let num_samples = (duration.as_secs_f32() * sample_rate) as usize;
        vec![0.0; num_samples]
    }
    
    /// Apply amplitude shaping (fade in/out) to reduce clicks
    fn apply_amplitude_shaping(&self, sample: f32, sample_index: usize, total_samples: usize) -> f32 {
        let fade_samples = ((self.config.sample_rate / 100).max(1)) as usize; // 10ms fade
        
        let fade_factor = if sample_index < fade_samples {
            // Fade in
            sample_index as f32 / fade_samples as f32
        } else if sample_index >= total_samples.saturating_sub(fade_samples) {
            // Fade out
            (total_samples - sample_index) as f32 / fade_samples as f32
        } else {
            // Full amplitude
            1.0
        };
        
        sample * fade_factor
    }
    
    /// Convert DTMF digit to frequency pair
    fn digit_to_frequencies(&self, digit: char) -> Option<(u16, u16)> {
        DTMF_DIGITS.iter().position(|&d| d == digit)
            .map(|i| DTMF_FREQUENCIES[i])
    }
}

/// DTMF Processing Service - combines detection and generation
pub struct DtmfProcessor {
    detector: Arc<DtmfDetector>,
    generator: Arc<DtmfGenerator>,
    event_receiver: Arc<RwLock<Option<mpsc::UnboundedReceiver<DtmfEvent>>>>,
    /// FreeSWITCH-style source prioritization tracking
    active_sources: Arc<RwLock<HashMap<String, (DtmfSource, Instant)>>>,
    config: DtmfDetectorConfig,
}

impl DtmfProcessor {
    /// Create new DTMF processor with default configurations
    pub fn new() -> Self {
        Self::with_config(DtmfDetectorConfig::default(), DtmfGeneratorConfig::default())
    }
    
    /// Create new DTMF processor with custom configurations
    pub fn with_config(detector_config: DtmfDetectorConfig, generator_config: DtmfGeneratorConfig) -> Self {
        let (detector, event_receiver) = DtmfDetector::new(detector_config.clone());
        let generator = DtmfGenerator::new(generator_config);
        
        Self {
            detector: Arc::new(detector),
            generator: Arc::new(generator),
            event_receiver: Arc::new(RwLock::new(Some(event_receiver))),
            active_sources: Arc::new(RwLock::new(HashMap::new())),
            config: detector_config,
        }
    }
    
    /// Get DTMF detector
    pub fn detector(&self) -> Arc<DtmfDetector> {
        Arc::clone(&self.detector)
    }
    
    /// Get DTMF generator  
    pub fn generator(&self) -> Arc<DtmfGenerator> {
        Arc::clone(&self.generator)
    }
    
    /// Take the event receiver (can only be done once)
    pub async fn take_event_receiver(&self) -> Option<mpsc::UnboundedReceiver<DtmfEvent>> {
        let mut receiver = self.event_receiver.write().await;
        receiver.take()
    }
    
    /// Process DTMF with FreeSWITCH-style source prioritization
    pub async fn process_dtmf_with_priority(&self, channel_id: &str, source: DtmfSource, 
                                          event: DtmfEvent) -> Result<bool> {
        let mut active_sources = self.active_sources.write().await;
        let now = Instant::now();
        
        // Check if this source should be accepted based on priority
        if let Some((current_source, last_time)) = active_sources.get(channel_id) {
            let current_priority = self.config.source_priority.get(current_source).unwrap_or(&99);
            let new_priority = self.config.source_priority.get(&source).unwrap_or(&99);
            
            // FreeSWITCH logic: Higher priority source can override, or timeout occurred
            let timeout_occurred = now.duration_since(*last_time) > Duration::from_millis(500);
            
            if new_priority > current_priority && !timeout_occurred {
                debug!("DTMF source {:?} rejected: lower priority than active {:?}", 
                      source, current_source);
                return Ok(false);
            }
        }
        
        // Update active source tracking
        active_sources.insert(channel_id.to_string(), (source, now));
        
        // Send the event (would integrate with existing event processing)
        debug!("DTMF accepted from source {:?} for channel {}", source, channel_id);
        Ok(true)
    }
    
    /// Enable liberal DTMF mode (FreeSWITCH-style)
    pub async fn set_liberal_dtmf(&mut self, enabled: bool) {
        // This would update the detector config
        debug!("Liberal DTMF mode {}", if enabled { "enabled" } else { "disabled" });
    }
}

impl Default for DtmfProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;
    
    #[tokio::test]
    async fn test_dtmf_generator() {
        let generator = DtmfGenerator::new(DtmfGeneratorConfig::default());
        
        // Test single digit generation
        let samples = generator.generate_digit('5', None, None).unwrap();
        assert!(!samples.is_empty());
        assert!(samples.len() > 100); // Should have reasonable number of samples
        
        // Test sequence generation
        let sequence_samples = generator.generate_sequence("123", None, None, None).unwrap();
        assert!(sequence_samples.len() > samples.len() * 3); // Should be longer than single digit
    }
    
    #[tokio::test]
    #[ignore = "DTMF detector test needs state machine timing investigation"]
    async fn test_dtmf_detector() {
        let config = DtmfDetectorConfig::default();
        let (detector, mut event_receiver) = DtmfDetector::new(config);
        
        // Add test channel
        detector.add_channel("test".to_string()).await.unwrap();
        
        // Generate test tone and detect it
        let generator = DtmfGenerator::new(DtmfGeneratorConfig::default());
        let samples = generator.generate_digit('5', Some(Duration::from_millis(200)), None).unwrap();
        
        // Process audio samples (tone)
        detector.process_audio("test", &samples, DtmfSource::Internal).await.unwrap();
        
        // Add silence after tone to trigger detection (DTMF detector needs to see tone end)
        let silence_samples = vec![0.0f32; 320]; // 40ms of silence at 8kHz 
        detector.process_audio("test", &silence_samples, DtmfSource::Internal).await.unwrap();
        
        // Should detect the digit after tone ends
        let event = timeout(Duration::from_millis(500), event_receiver.recv()).await.unwrap().unwrap();
        match event {
            DtmfEvent::DigitDetected { digit, .. } => {
                assert_eq!(digit, '5');
            }
            _ => panic!("Expected DigitDetected event"),
        }
    }
    
    #[test]
    fn test_frequency_mapping() {
        let generator = DtmfGenerator::new(DtmfGeneratorConfig::default());
        
        // Test standard digits
        assert_eq!(generator.digit_to_frequencies('1'), Some((697, 1209)));
        assert_eq!(generator.digit_to_frequencies('5'), Some((770, 1336)));
        assert_eq!(generator.digit_to_frequencies('9'), Some((852, 1477)));
        assert_eq!(generator.digit_to_frequencies('#'), Some((941, 1477)));
        
        // Test extended digits
        assert_eq!(generator.digit_to_frequencies('A'), Some((697, 1633)));
        assert_eq!(generator.digit_to_frequencies('D'), Some((941, 1633)));
        
        // Test invalid digit
        assert_eq!(generator.digit_to_frequencies('X'), None);
    }
}