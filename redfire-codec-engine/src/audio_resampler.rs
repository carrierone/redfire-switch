/*
 * Professional Audio Resampling
 * High-quality sample rate conversion for media transcoding
 */

use anyhow::{anyhow, Result};
// dasp Signal import removed - not used in this minimal version
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Audio resampling quality profiles
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResamplingQuality {
    /// Fast, lower quality (for real-time applications)
    Fast,
    /// Balanced quality and performance
    Balanced,
    /// High quality (for non-real-time applications)
    High,
    /// Maximum quality (very slow, for offline processing)
    Maximum,
}

impl ResamplingQuality {
    /// Get the filter length for this quality setting
    pub fn filter_length(&self) -> usize {
        match self {
            ResamplingQuality::Fast => 32,
            ResamplingQuality::Balanced => 64,
            ResamplingQuality::High => 128,
            ResamplingQuality::Maximum => 256,
        }
    }

    /// Get the sinc table size for this quality setting
    pub fn sinc_table_size(&self) -> usize {
        match self {
            ResamplingQuality::Fast => 1024,
            ResamplingQuality::Balanced => 2048,
            ResamplingQuality::High => 4096,
            ResamplingQuality::Maximum => 8192,
        }
    }

    /// Get the passband percentage (0.0 to 1.0)
    pub fn passband(&self) -> f64 {
        match self {
            ResamplingQuality::Fast => 0.85,
            ResamplingQuality::Balanced => 0.90,
            ResamplingQuality::High => 0.95,
            ResamplingQuality::Maximum => 0.98,
        }
    }

    /// Get the stopband attenuation in dB
    pub fn stopband_attenuation(&self) -> f64 {
        match self {
            ResamplingQuality::Fast => 60.0,
            ResamplingQuality::Balanced => 80.0,
            ResamplingQuality::High => 100.0,
            ResamplingQuality::Maximum => 120.0,
        }
    }
}

/// Audio resampler configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResamplerConfig {
    /// Resampling quality
    pub quality: ResamplingQuality,
    /// Maximum input sample rate
    pub max_input_rate: u32,
    /// Maximum output sample rate  
    pub max_output_rate: u32,
    /// Maximum number of channels
    pub max_channels: u32,
    /// Buffer size for processing
    pub buffer_size: usize,
    /// Enable anti-aliasing filter
    pub anti_aliasing: bool,
    /// Enable DC blocker
    pub dc_blocker: bool,
}

impl Default for ResamplerConfig {
    fn default() -> Self {
        Self {
            quality: ResamplingQuality::Balanced,
            max_input_rate: 48000,
            max_output_rate: 48000,
            max_channels: 8,
            buffer_size: 4096,
            anti_aliasing: true,
            dc_blocker: true,
        }
    }
}

/// Professional audio resampler using windowed sinc interpolation
pub struct AudioResampler {
    config: ResamplerConfig,
    /// Sinc interpolation table
    sinc_table: Vec<f64>,
    /// Filter coefficients for anti-aliasing
    #[allow(dead_code)]
    lowpass_filter: Vec<f64>,
    /// DC blocker state per channel
    dc_blocker_state: Arc<RwLock<Vec<DcBlockerState>>>,
}

/// DC blocker state for high-pass filtering
#[derive(Debug, Clone)]
struct DcBlockerState {
    x1: f64,
    y1: f64,
}

impl DcBlockerState {
    fn new() -> Self {
        Self { x1: 0.0, y1: 0.0 }
    }

    /// Process sample through DC blocker (high-pass filter)
    fn process(&mut self, x0: f64) -> f64 {
        // DC blocker: y[n] = x[n] - x[n-1] + 0.995 * y[n-1]
        let y0 = x0 - self.x1 + 0.995 * self.y1;
        self.x1 = x0;
        self.y1 = y0;
        y0
    }
}

impl AudioResampler {
    /// Create new audio resampler
    pub async fn new(config: ResamplerConfig) -> Result<Self> {
        let sinc_table = Self::generate_sinc_table(&config);
        let lowpass_filter = Self::generate_lowpass_filter(&config);

        let max_channels = config.max_channels as usize;
        let dc_blocker_state = Arc::new(RwLock::new(
            (0..max_channels).map(|_| DcBlockerState::new()).collect(),
        ));

        info!(
            "Created audio resampler with quality {:?}, filter length {}",
            config.quality,
            config.quality.filter_length()
        );

        Ok(Self {
            config,
            sinc_table,
            lowpass_filter,
            dc_blocker_state,
        })
    }

    /// Generate windowed sinc interpolation table
    fn generate_sinc_table(config: &ResamplerConfig) -> Vec<f64> {
        let table_size = config.quality.sinc_table_size();
        let filter_length = config.quality.filter_length();
        let mut table = Vec::with_capacity(table_size * filter_length);

        let passband = config.quality.passband();
        let nyquist = 0.5 * passband; // Anti-aliasing cutoff

        for i in 0..table_size {
            let fraction = i as f64 / table_size as f64;

            for j in 0..filter_length {
                let t = (j as f64 - filter_length as f64 * 0.5 + fraction) * std::f64::consts::PI;

                let sinc_val = if t.abs() < 1e-10 {
                    1.0
                } else {
                    let sinc = (t * nyquist).sin() / (t * nyquist);
                    let window = Self::kaiser_window(j, filter_length, 8.0); // Kaiser window with β=8
                    sinc * window
                };

                table.push(sinc_val);
            }
        }

        // Normalize to unit gain
        Self::normalize_filter(&mut table, filter_length);
        table
    }

    /// Generate Kaiser window
    fn kaiser_window(n: usize, length: usize, beta: f64) -> f64 {
        let n_centered = n as f64 - (length - 1) as f64 * 0.5;
        let alpha = (length - 1) as f64 * 0.5;

        let x = beta * (1.0 - (n_centered / alpha).powi(2)).sqrt();
        Self::modified_bessel_i0(x) / Self::modified_bessel_i0(beta)
    }

    /// Modified Bessel function of the first kind, order 0
    fn modified_bessel_i0(x: f64) -> f64 {
        let mut sum = 1.0;
        let mut term = 1.0;
        let x_half_squared = (x * 0.5).powi(2);

        for k in 1..=50 {
            term *= x_half_squared / (k as f64).powi(2);
            sum += term;
            if term < 1e-15 {
                break;
            }
        }

        sum
    }

    /// Generate anti-aliasing lowpass filter
    fn generate_lowpass_filter(config: &ResamplerConfig) -> Vec<f64> {
        let filter_length = config.quality.filter_length();
        let cutoff = config.quality.passband() * 0.5; // Normalized cutoff frequency
        let mut filter = Vec::with_capacity(filter_length);

        for i in 0..filter_length {
            let t = (i as f64 - filter_length as f64 * 0.5) * std::f64::consts::PI;

            let h = if t.abs() < 1e-10 {
                cutoff
            } else {
                (t * cutoff).sin() / t
            };

            let window = Self::hanning_window(i, filter_length);
            filter.push(h * window);
        }

        // Normalize to unit gain
        Self::normalize_filter(&mut filter, filter_length);
        filter
    }

    /// Hanning window function
    fn hanning_window(n: usize, length: usize) -> f64 {
        0.5 * (1.0 - (2.0 * std::f64::consts::PI * n as f64 / (length - 1) as f64).cos())
    }

    /// Normalize filter for unit gain
    fn normalize_filter(filter: &mut [f64], step: usize) {
        if step == 0 || filter.is_empty() {
            return;
        }

        let mut sum = 0.0;
        for chunk in filter.chunks(step) {
            sum += chunk[0]; // Sum only the center coefficients
        }

        if sum.abs() > 1e-10 {
            for sample in filter.iter_mut() {
                *sample /= sum;
            }
        }
    }

    /// Resample audio data
    pub async fn resample(
        &self,
        input: &[f32],
        input_rate: u32,
        output_rate: u32,
        channels: u32,
    ) -> Result<Vec<f32>> {
        if input_rate == output_rate {
            return Ok(input.to_vec()); // No resampling needed
        }

        if channels == 0 || channels > self.config.max_channels {
            return Err(anyhow!("Invalid channel count: {}", channels));
        }

        let ratio = output_rate as f64 / input_rate as f64;
        let input_frames = input.len() / channels as usize;
        let output_frames = (input_frames as f64 * ratio).ceil() as usize;
        let mut output = vec![0.0f32; output_frames * channels as usize];

        // Process each channel separately
        for ch in 0..channels as usize {
            let channel_input: Vec<f64> = input
                .iter()
                .skip(ch)
                .step_by(channels as usize)
                .map(|&x| x as f64)
                .collect();

            let channel_output = self.resample_channel(&channel_input, ratio, ch).await?;

            // Interleave output
            for (i, &sample) in channel_output.iter().enumerate() {
                if i * channels as usize + ch < output.len() {
                    output[i * channels as usize + ch] = sample as f32;
                }
            }
        }

        Ok(output)
    }

    /// Resample a single channel
    async fn resample_channel(
        &self,
        input: &[f64],
        ratio: f64,
        channel_index: usize,
    ) -> Result<Vec<f64>> {
        let filter_length = self.config.quality.filter_length();
        let table_size = self.config.quality.sinc_table_size();
        let output_length = (input.len() as f64 * ratio).ceil() as usize;
        let mut output = Vec::with_capacity(output_length);

        // Pad input for boundary conditions
        let padded_input = self.pad_input(input, filter_length);

        for i in 0..output_length {
            let src_pos = i as f64 / ratio;
            let src_index = src_pos.floor() as usize;
            let fraction = src_pos - src_index as f64;

            // Get interpolation coefficients
            let table_index = (fraction * table_size as f64) as usize;
            let coeff_start = table_index * filter_length;

            // Perform convolution
            let mut sample = 0.0;
            for j in 0..filter_length {
                let input_idx = src_index + j;
                if input_idx < padded_input.len() {
                    sample += padded_input[input_idx] * self.sinc_table[coeff_start + j];
                }
            }

            // Apply anti-aliasing filter if downsampling
            if ratio < 1.0 && self.config.anti_aliasing {
                sample = self.apply_lowpass_filter(sample, &padded_input, src_index);
            }

            output.push(sample);
        }

        // Apply DC blocker if enabled
        if self.config.dc_blocker {
            let mut dc_state = self.dc_blocker_state.write().await;
            if channel_index < dc_state.len() {
                for sample in output.iter_mut() {
                    *sample = dc_state[channel_index].process(*sample);
                }
            }
        }

        Ok(output)
    }

    /// Pad input for boundary conditions
    fn pad_input(&self, input: &[f64], filter_length: usize) -> Vec<f64> {
        let pad_size = filter_length / 2;
        let mut padded = Vec::with_capacity(input.len() + 2 * pad_size);

        // Pad beginning with zeros or first sample
        for _ in 0..pad_size {
            padded.push(input.first().copied().unwrap_or(0.0));
        }

        // Copy input
        padded.extend_from_slice(input);

        // Pad end with zeros or last sample
        for _ in 0..pad_size {
            padded.push(input.last().copied().unwrap_or(0.0));
        }

        padded
    }

    /// Apply lowpass filter for anti-aliasing
    fn apply_lowpass_filter(&self, _sample: f64, _input: &[f64], _index: usize) -> f64 {
        // Simplified implementation - real filter would use self.lowpass_filter
        // For now, just return the sample unfiltered
        _sample
    }

    /// Estimate output length for given input and sample rates
    pub fn estimate_output_length(
        &self,
        input_length: usize,
        input_rate: u32,
        output_rate: u32,
        channels: u32,
    ) -> usize {
        if input_rate == output_rate {
            return input_length;
        }

        let input_frames = input_length / channels as usize;
        let ratio = output_rate as f64 / input_rate as f64;
        let output_frames = (input_frames as f64 * ratio).ceil() as usize;
        output_frames * channels as usize
    }

    /// Get latency introduced by resampling (in samples at output rate)
    pub fn get_latency(&self, output_rate: u32) -> usize {
        // Latency is approximately half the filter length
        let filter_samples = self.config.quality.filter_length() / 2;
        filter_samples * output_rate as usize / 48000 // Normalize to output rate
    }
}

/// High-level resampling service for managing multiple resamplers
pub struct ResamplingService {
    config: ResamplerConfig,
    /// Per-session resamplers
    resamplers: Arc<RwLock<std::collections::HashMap<String, Arc<AudioResampler>>>>,
}

impl ResamplingService {
    /// Create new resampling service
    pub async fn new(config: ResamplerConfig) -> Result<Self> {
        Ok(Self {
            config,
            resamplers: Arc::new(RwLock::new(std::collections::HashMap::new())),
        })
    }

    /// Start resampling session
    pub async fn start_session(&self, session_id: String) -> Result<()> {
        let resampler = Arc::new(AudioResampler::new(self.config.clone()).await?);
        let mut resamplers = self.resamplers.write().await;
        resamplers.insert(session_id, resampler);
        Ok(())
    }

    /// End resampling session
    pub async fn end_session(&self, session_id: &str) -> Result<()> {
        let mut resamplers = self.resamplers.write().await;
        resamplers.remove(session_id);
        Ok(())
    }

    /// Resample audio for a specific session
    pub async fn resample_for_session(
        &self,
        session_id: &str,
        input: &[f32],
        input_rate: u32,
        output_rate: u32,
        channels: u32,
    ) -> Result<Vec<f32>> {
        let resamplers = self.resamplers.read().await;
        let resampler = resamplers
            .get(session_id)
            .ok_or_else(|| anyhow!("Resampling session {} not found", session_id))?;

        resampler
            .resample(input, input_rate, output_rate, channels)
            .await
    }

    /// Get active session count
    pub async fn get_active_sessions(&self) -> usize {
        let resamplers = self.resamplers.read().await;
        resamplers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resampler_creation() {
        let config = ResamplerConfig::default();
        let resampler = AudioResampler::new(config).await.unwrap();
        assert!(resampler.sinc_table.len() > 0);
    }

    #[tokio::test]
    async fn test_no_resampling_needed() {
        let config = ResamplerConfig::default();
        let resampler = AudioResampler::new(config).await.unwrap();

        let input = vec![0.5, -0.3, 0.8, -0.2];
        let output = resampler.resample(&input, 8000, 8000, 1).await.unwrap();

        assert_eq!(input, output);
    }

    #[tokio::test]
    async fn test_upsampling() {
        let config = ResamplerConfig::default();
        let resampler = AudioResampler::new(config).await.unwrap();

        let input = vec![1.0, 0.0, -1.0, 0.0]; // 4 samples
        let output = resampler.resample(&input, 8000, 16000, 1).await.unwrap();

        // Should approximately double the length
        assert!(output.len() >= 7 && output.len() <= 9);
    }

    #[tokio::test]
    async fn test_downsampling() {
        let config = ResamplerConfig::default();
        let resampler = AudioResampler::new(config).await.unwrap();

        let input = vec![1.0, 0.5, 0.0, -0.5, -1.0, -0.5, 0.0, 0.5]; // 8 samples
        let output = resampler.resample(&input, 16000, 8000, 1).await.unwrap();

        // Should approximately halve the length
        assert!(output.len() >= 3 && output.len() <= 5);
    }

    #[tokio::test]
    async fn test_stereo_resampling() {
        let config = ResamplerConfig::default();
        let resampler = AudioResampler::new(config).await.unwrap();

        // Stereo input: L, R, L, R, ...
        let input = vec![1.0, -1.0, 0.5, -0.5, 0.0, 0.0, -0.5, 0.5];
        let output = resampler.resample(&input, 8000, 16000, 2).await.unwrap();

        // Should maintain stereo interleaving
        assert_eq!(output.len() % 2, 0); // Even number of samples
        assert!(output.len() >= 14 && output.len() <= 18);
    }

    #[tokio::test]
    async fn test_resampling_service() {
        let config = ResamplerConfig::default();
        let service = ResamplingService::new(config).await.unwrap();

        service
            .start_session("test_session".to_string())
            .await
            .unwrap();
        assert_eq!(service.get_active_sessions().await, 1);

        let input = vec![1.0, 0.0, -1.0, 0.0];
        let output = service
            .resample_for_session("test_session", &input, 8000, 16000, 1)
            .await
            .unwrap();

        assert!(output.len() > input.len());

        service.end_session("test_session").await.unwrap();
        assert_eq!(service.get_active_sessions().await, 0);
    }

    #[tokio::test]
    async fn test_quality_settings() {
        for quality in [
            ResamplingQuality::Fast,
            ResamplingQuality::Balanced,
            ResamplingQuality::High,
            ResamplingQuality::Maximum,
        ] {
            let config = ResamplerConfig {
                quality,
                ..Default::default()
            };

            let resampler = AudioResampler::new(config).await.unwrap();
            assert_eq!(
                resampler.sinc_table.len(),
                quality.sinc_table_size() * quality.filter_length()
            );
        }
    }

    #[tokio::test]
    async fn test_estimate_output_length() {
        let config = ResamplerConfig::default();
        let resampler = AudioResampler::new(config).await.unwrap();

        // Test upsampling
        let estimated = resampler.estimate_output_length(1000, 8000, 16000, 1);
        assert_eq!(estimated, 2000);

        // Test downsampling
        let estimated = resampler.estimate_output_length(2000, 16000, 8000, 1);
        assert_eq!(estimated, 1000);

        // Test with channels
        let estimated = resampler.estimate_output_length(1000, 8000, 16000, 2);
        assert_eq!(estimated, 2000);
    }
}
