// DTMF Processor Module
//
// Real DTMF (Dual-Tone Multi-Frequency) tone generation and detection.
//
// - Generation synthesizes the two sine components for each digit (ITU-T Q.23
//   low/high group frequencies) at 8 kHz, returned as f32 PCM samples.
// - Detection uses the Goertzel algorithm to measure energy at the eight DTMF
//   frequencies plus a simple state machine (minimum tone duration and
//   inter-digit silence) to emit stable digit events.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::RwLock;

/// DTMF low-group (row) frequencies in Hz.
const LOW_FREQS: [f32; 4] = [697.0, 770.0, 852.0, 941.0];
/// DTMF high-group (column) frequencies in Hz.
const HIGH_FREQS: [f32; 4] = [1209.0, 1336.0, 1477.0, 1633.0];
/// Standard DTMF keypad layout indexed by [row][col].
const DTMF_KEYPAD: [[char; 4]; 4] = [
    ['1', '2', '3', 'A'],
    ['4', '5', '6', 'B'],
    ['7', '8', '9', 'C'],
    ['*', '0', '#', 'D'],
];

/// Return the (low, high) frequency pair for a DTMF digit, if valid.
fn digit_frequencies(digit: char) -> Option<(f32, f32)> {
    let d = digit.to_ascii_uppercase();
    for (row, &low) in LOW_FREQS.iter().enumerate() {
        for (col, &high) in HIGH_FREQS.iter().enumerate() {
            if DTMF_KEYPAD[row][col] == d {
                return Some((low, high));
            }
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DtmfEvent {
    DigitDetected {
        digit: char,
        duration: std::time::Duration,
        #[serde(skip, default = "std::time::Instant::now")]
        timestamp: std::time::Instant,
        confidence: f32,
        source: DtmfSource,
    },
    SequenceComplete {
        sequence: String,
        total_duration: std::time::Duration,
        source: DtmfSource,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DtmfSource {
    Rfc2833,
    SipInfo,
    Sigtran,
    AudioDetection,
    TdmoeVoice,
}

pub struct DtmfProcessor {
    detector: DtmfDetector,
    generator: DtmfGenerator,
}

#[derive(Debug, Clone)]
pub struct DtmfDetectorConfig {
    pub sample_rate: u32,
    pub min_tone_duration: u32,
    pub max_tone_duration: u32,
    pub min_inter_digit_silence: u32,
    pub confidence_threshold: f32,
    pub block_size: usize,
    pub enable_extended: bool,
    pub twist_tolerance: f32,
    pub reverse_twist_tolerance: f32,
    pub liberal_dtmf: bool,
    pub source_priority: HashMap<DtmfSource, u8>,
    pub itu_compliance: bool,
    pub snr_threshold: f32,
}

#[derive(Debug, Clone)]
pub struct DtmfGeneratorConfig {
    pub sample_rate: u32,
    pub default_tone_duration: u32,
    pub default_inter_digit_silence: u32,
    pub default_amplitude: f32,
    pub enable_shaping: bool,
}

/// Per-channel detection statistics and current state.
#[derive(Debug, Clone, Default)]
pub struct DtmfChannelStatistics {
    pub channel_id: String,
    pub current_digit: Option<char>,
    pub current_sequence: String,
    pub digits_detected: u64,
}

/// Internal per-channel detection state machine.
#[derive(Debug, Default)]
struct ChannelState {
    /// Digit currently being held (seen in consecutive blocks) but not yet emitted.
    candidate: Option<char>,
    /// Milliseconds the candidate tone has been present.
    candidate_ms: u32,
    /// Milliseconds of silence observed since the last stable tone.
    silence_ms: u32,
    /// Last stably-detected and emitted digit.
    last_emitted: Option<char>,
    stats: DtmfChannelStatistics,
}

pub struct DtmfDetector {
    config: DtmfDetectorConfig,
    channels: RwLock<HashMap<String, Mutex<ChannelState>>>,
}

pub struct DtmfGenerator {
    config: DtmfGeneratorConfig,
}

impl Default for DtmfDetectorConfig {
    fn default() -> Self {
        Self {
            sample_rate: 8000,
            min_tone_duration: 40,
            max_tone_duration: 2000,
            min_inter_digit_silence: 40,
            confidence_threshold: 0.7,
            block_size: 80,
            enable_extended: false,
            twist_tolerance: 8.0,
            reverse_twist_tolerance: 4.0,
            liberal_dtmf: false,
            source_priority: HashMap::new(),
            itu_compliance: true,
            snr_threshold: 15.0,
        }
    }
}

impl Default for DtmfGeneratorConfig {
    fn default() -> Self {
        Self {
            sample_rate: 8000,
            default_tone_duration: 100,
            default_inter_digit_silence: 100,
            default_amplitude: 0.6,
            enable_shaping: true,
        }
    }
}

impl Default for DtmfProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl DtmfProcessor {
    pub fn new() -> Self {
        Self::with_config(DtmfDetectorConfig::default(), DtmfGeneratorConfig::default())
    }

    pub fn with_config(
        detector_config: DtmfDetectorConfig,
        generator_config: DtmfGeneratorConfig,
    ) -> Self {
        Self {
            detector: DtmfDetector::new(detector_config),
            generator: DtmfGenerator::new(generator_config),
        }
    }

    pub fn detector(&self) -> &DtmfDetector {
        &self.detector
    }

    pub fn generator(&self) -> &DtmfGenerator {
        &self.generator
    }
}

impl DtmfDetector {
    pub fn new(config: DtmfDetectorConfig) -> Self {
        Self {
            config,
            channels: RwLock::new(HashMap::new()),
        }
    }

    /// Register a detection channel by identifier.
    pub async fn add_channel(&self, channel_id: impl Into<String>) -> Result<()> {
        let channel_id = channel_id.into();
        let mut channels = self.channels.write().await;
        let mut state = ChannelState::default();
        state.stats.channel_id = channel_id.clone();
        channels.insert(channel_id, Mutex::new(state));
        Ok(())
    }

    /// Remove a detection channel.
    pub async fn remove_channel(&self, channel_id: &str) -> Result<()> {
        let mut channels = self.channels.write().await;
        channels.remove(channel_id);
        Ok(())
    }

    /// Return a snapshot of a channel's detection statistics.
    pub async fn get_statistics(&self, channel_id: &str) -> Result<DtmfChannelStatistics> {
        let channels = self.channels.read().await;
        let state = channels
            .get(channel_id)
            .ok_or_else(|| anyhow!("unknown DTMF channel: {channel_id}"))?;
        let state = state.lock().unwrap();
        Ok(state.stats.clone())
    }

    /// Feed a block of f32 PCM samples for a channel and return a newly
    /// detected, stable digit if one just completed.
    pub async fn process_samples(&self, channel_id: &str, samples: &[f32]) -> Result<Option<char>> {
        let detected = self.detect_block(samples);
        let block_ms = ((samples.len() as f32 / self.config.sample_rate as f32) * 1000.0) as u32;

        let channels = self.channels.read().await;
        let state = channels
            .get(channel_id)
            .ok_or_else(|| anyhow!("unknown DTMF channel: {channel_id}"))?;
        let mut state = state.lock().unwrap();
        Ok(self.advance_state(&mut state, detected, block_ms.max(1)))
    }

    /// Feed a block of 8-bit (offset-binary) PCM audio, as used by the TDM path.
    pub async fn process_audio(&self, channel_id: &str, data: &[u8]) -> Result<Option<char>> {
        let samples: Vec<f32> = data
            .iter()
            .map(|&b| (b as f32 - 128.0) / 128.0)
            .collect();
        self.process_samples(channel_id, &samples).await
    }

    /// Run one detection cycle: returns the strongest DTMF digit present in this
    /// block, if the tone energies satisfy the configured thresholds.
    fn detect_block(&self, samples: &[f32]) -> Option<char> {
        if samples.is_empty() {
            return None;
        }

        let low_energies: Vec<f32> = LOW_FREQS
            .iter()
            .map(|&f| goertzel(samples, f, self.config.sample_rate as f32))
            .collect();
        let high_energies: Vec<f32> = HIGH_FREQS
            .iter()
            .map(|&f| goertzel(samples, f, self.config.sample_rate as f32))
            .collect();

        let (low_idx, low_energy) = max_index(&low_energies);
        let (high_idx, high_energy) = max_index(&high_energies);

        // Overall signal energy for a relative threshold.
        let total: f32 = samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32;
        if total < 1e-5 {
            return None; // effectively silence
        }

        // Require both groups to carry a meaningful share of the energy.
        let min_tone = total * samples.len() as f32 * 0.05;
        if low_energy < min_tone || high_energy < min_tone {
            return None;
        }

        Some(DTMF_KEYPAD[low_idx][high_idx])
    }

    /// Advance the per-channel state machine and return a digit if one just
    /// became stable (present for at least `min_tone_duration`).
    fn advance_state(
        &self,
        state: &mut ChannelState,
        detected: Option<char>,
        block_ms: u32,
    ) -> Option<char> {
        match detected {
            Some(digit) => {
                state.silence_ms = 0;
                if state.candidate == Some(digit) {
                    state.candidate_ms += block_ms;
                } else {
                    state.candidate = Some(digit);
                    state.candidate_ms = block_ms;
                }

                let stable = state.candidate_ms >= self.config.min_tone_duration;
                let is_new = state.last_emitted != Some(digit);
                if stable && is_new {
                    state.last_emitted = Some(digit);
                    state.stats.current_digit = Some(digit);
                    state.stats.current_sequence.push(digit);
                    state.stats.digits_detected += 1;
                    return Some(digit);
                }
                None
            }
            None => {
                state.candidate = None;
                state.candidate_ms = 0;
                state.silence_ms += block_ms;
                if state.silence_ms >= self.config.min_inter_digit_silence {
                    // Allow the same digit to be detected again after silence.
                    state.last_emitted = None;
                    state.stats.current_digit = None;
                }
                None
            }
        }
    }
}

impl DtmfGenerator {
    pub fn new(config: DtmfGeneratorConfig) -> Self {
        Self { config }
    }

    /// Generate f32 PCM samples for a single DTMF digit.
    ///
    /// `duration` defaults to the configured tone duration; `amplitude`
    /// defaults to the configured amplitude (0.0-1.0 per component).
    pub fn generate_digit(
        &self,
        digit: char,
        duration: Option<Duration>,
        amplitude: Option<f32>,
    ) -> Result<Vec<f32>> {
        let (low, high) = digit_frequencies(digit)
            .ok_or_else(|| anyhow!("invalid DTMF digit: {digit:?}"))?;

        let duration_ms = duration
            .map(|d| d.as_millis() as u32)
            .unwrap_or(self.config.default_tone_duration);
        let amplitude = amplitude.unwrap_or(self.config.default_amplitude).clamp(0.0, 1.0);

        let sample_rate = self.config.sample_rate as f32;
        let num_samples = ((duration_ms as f32 / 1000.0) * sample_rate).round() as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for n in 0..num_samples {
            let t = n as f32 / sample_rate;
            let mut s = amplitude
                * (0.5 * (2.0 * std::f32::consts::PI * low * t).sin()
                    + 0.5 * (2.0 * std::f32::consts::PI * high * t).sin());

            // Optional raised-cosine ramp to reduce spectral splatter at edges.
            if self.config.enable_shaping {
                let ramp = (sample_rate * 0.005) as usize; // 5ms ramp
                if ramp > 0 {
                    if n < ramp {
                        s *= n as f32 / ramp as f32;
                    } else if n >= num_samples.saturating_sub(ramp) {
                        s *= (num_samples - n) as f32 / ramp as f32;
                    }
                }
            }
            samples.push(s);
        }

        Ok(samples)
    }

    /// Generate PCM samples for a sequence of digits, separated by silence.
    pub fn generate_sequence(
        &self,
        digits: &str,
        tone_duration: Option<Duration>,
        inter_digit_silence: Option<Duration>,
        amplitude: Option<f32>,
    ) -> Result<Vec<f32>> {
        let silence_ms = inter_digit_silence
            .map(|d| d.as_millis() as u32)
            .unwrap_or(self.config.default_inter_digit_silence);
        let silence_samples =
            ((silence_ms as f32 / 1000.0) * self.config.sample_rate as f32).round() as usize;

        let mut out = Vec::new();
        for (i, digit) in digits.chars().enumerate() {
            if i > 0 {
                out.extend(std::iter::repeat(0.0f32).take(silence_samples));
            }
            out.extend(self.generate_digit(digit, tone_duration, amplitude)?);
        }
        Ok(out)
    }

    /// Generate a digit as 8-bit offset-binary PCM (TDM convenience helper).
    pub fn generate_digit_u8(&self, digit: char, duration: Option<Duration>) -> Result<Vec<u8>> {
        let samples = self.generate_digit(digit, duration, None)?;
        Ok(samples
            .iter()
            .map(|&s| ((s.clamp(-1.0, 1.0) * 127.0) + 128.0) as u8)
            .collect())
    }
}

/// Goertzel algorithm: energy of `samples` at frequency `target_freq`.
fn goertzel(samples: &[f32], target_freq: f32, sample_rate: f32) -> f32 {
    let k = (0.5 + (samples.len() as f32 * target_freq) / sample_rate).floor();
    let omega = (2.0 * std::f32::consts::PI * k) / samples.len() as f32;
    let coeff = 2.0 * omega.cos();

    let mut s_prev = 0.0f32;
    let mut s_prev2 = 0.0f32;
    for &x in samples {
        let s = x + coeff * s_prev - s_prev2;
        s_prev2 = s_prev;
        s_prev = s;
    }
    s_prev2 * s_prev2 + s_prev * s_prev - coeff * s_prev * s_prev2
}

/// Return (index, value) of the maximum entry.
fn max_index(values: &[f32]) -> (usize, f32) {
    let mut best_idx = 0;
    let mut best_val = f32::MIN;
    for (i, &v) in values.iter().enumerate() {
        if v > best_val {
            best_val = v;
            best_idx = i;
        }
    }
    (best_idx, best_val)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_and_detect_roundtrip() {
        let generator = DtmfGenerator::new(DtmfGeneratorConfig::default());
        let detector = DtmfDetector::new(DtmfDetectorConfig::default());
        detector.add_channel("c1").await.unwrap();

        for digit in "0123456789*#ABCD".chars() {
            // 100ms tone is well over the 40ms minimum.
            let samples = generator
                .generate_digit(digit, Some(Duration::from_millis(100)), Some(0.8))
                .unwrap();
            assert!(samples.len() >= 800);

            // Feed in 20ms blocks and expect the digit to be detected.
            detector.add_channel("c1").await.unwrap(); // reset state
            let block = (0.02 * 8000.0) as usize;
            let mut detected = None;
            for chunk in samples.chunks(block) {
                if let Some(d) = detector.process_samples("c1", chunk).await.unwrap() {
                    detected = Some(d);
                }
            }
            assert_eq!(detected, Some(digit), "failed to detect digit {digit}");
        }
    }

    #[tokio::test]
    async fn test_statistics_tracking() {
        let detector = DtmfDetector::new(DtmfDetectorConfig::default());
        detector.add_channel("ch").await.unwrap();
        let stats = detector.get_statistics("ch").await.unwrap();
        assert_eq!(stats.channel_id, "ch");
        assert_eq!(stats.current_digit, None);
        assert_eq!(stats.current_sequence, "");
    }

    #[test]
    fn test_silence_not_detected() {
        let detector = DtmfDetector::new(DtmfDetectorConfig::default());
        let silence = vec![0.0f32; 160];
        assert_eq!(detector.detect_block(&silence), None);
    }
}
