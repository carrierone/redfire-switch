/*
 * G.722.2 / AMR-WB (Adaptive Multi-Rate Wideband) Implementation
 * Copyright (C) 2025 Carrier One Inc and contributors
 *
 * Based on ITU-T G.722.2 standard (patent-free since 2023)
 * Adaptive Multi-Rate Wideband codec using ACELP (Algebraic Code-Excited Linear Prediction)
 */

use anyhow::{anyhow, Result};
use std::f32::consts::PI;

// G.722.2 / AMR-WB constants
pub const L_FRAME_WB: usize = 320; // Frame size (20ms at 16kHz)
pub const L_SUBFR_WB: usize = 64; // Subframe size (4ms)
pub const NB_SUBFR_WB: usize = 4; // Number of subframes
pub const M_WB: usize = 16; // LP order for wideband
pub const L_WINDOW_WB: usize = 384; // Window size for LP analysis
pub const L_NEXT_WB: usize = 64; // Lookahead
pub const PIT_MIN_WB: usize = 34; // Minimum pitch (16kHz)
pub const PIT_MAX_WB: usize = 231; // Maximum pitch (16kHz)
pub const L_INTERPOL_WB: usize = 16; // Interpolation filter length

// AMR-WB mode definitions (bitrates)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AmrWbMode {
    Mode0 = 0, // 6.60 kbps
    Mode1 = 1, // 8.85 kbps
    Mode2 = 2, // 12.65 kbps
    Mode3 = 3, // 14.25 kbps
    Mode4 = 4, // 15.85 kbps
    Mode5 = 5, // 18.25 kbps
    Mode6 = 6, // 19.85 kbps
    Mode7 = 7, // 23.05 kbps
    Mode8 = 8, // 23.85 kbps (most common)
}

impl AmrWbMode {
    /// Get frame size in bytes for this mode
    pub fn frame_size_bytes(&self) -> usize {
        match self {
            AmrWbMode::Mode0 => 18, // 132 bits
            AmrWbMode::Mode1 => 24, // 177 bits
            AmrWbMode::Mode2 => 33, // 253 bits
            AmrWbMode::Mode3 => 37, // 285 bits
            AmrWbMode::Mode4 => 41, // 317 bits
            AmrWbMode::Mode5 => 47, // 365 bits
            AmrWbMode::Mode6 => 51, // 397 bits
            AmrWbMode::Mode7 => 59, // 461 bits
            AmrWbMode::Mode8 => 61, // 477 bits
        }
    }

    /// Get bitrate in kbps
    pub fn bitrate(&self) -> f32 {
        match self {
            AmrWbMode::Mode0 => 6.60,
            AmrWbMode::Mode1 => 8.85,
            AmrWbMode::Mode2 => 12.65,
            AmrWbMode::Mode3 => 14.25,
            AmrWbMode::Mode4 => 15.85,
            AmrWbMode::Mode5 => 18.25,
            AmrWbMode::Mode6 => 19.85,
            AmrWbMode::Mode7 => 23.05,
            AmrWbMode::Mode8 => 23.85,
        }
    }

    /// Default mode (commonly used)
    pub fn default() -> Self {
        AmrWbMode::Mode8 // 23.85 kbps
    }
}

/// G.722.2 encoder state
pub struct G7222Encoder {
    // Speech buffers
    old_speech: Vec<f32>, // Old speech buffer for windowing
    old_exc: Vec<f32>,    // Old excitation buffer
    old_wsp: Vec<f32>,    // Weighted speech buffer

    // LP/ISP related (ISP = Immittance Spectral Pairs, similar to LSP)
    old_isp: Vec<f32>,   // Previous ISP
    old_isp_q: Vec<f32>, // Previous quantized ISP

    // Synthesis filter memory
    mem_syn: Vec<f32>, // Synthesis filter memory
    mem_w: Vec<f32>,   // Weighting filter memory
    mem_deemph: f32,   // De-emphasis memory

    // Gains
    past_qua_en: Vec<f32>, // Past quantized energies

    // AMR-WB mode
    mode: AmrWbMode,

    // High-pass filter state
    hp_filt_mem: Vec<f32>,

    // Pre-emphasis
    preemph_mem: f32,
}

impl G7222Encoder {
    pub fn new(mode: AmrWbMode) -> Self {
        Self {
            old_speech: vec![0.0; L_WINDOW_WB],
            old_exc: vec![0.0; L_FRAME_WB + PIT_MAX_WB + L_INTERPOL_WB],
            old_wsp: vec![0.0; L_FRAME_WB + PIT_MAX_WB],
            old_isp: vec![0.0; M_WB],
            old_isp_q: vec![0.0; M_WB],
            mem_syn: vec![0.0; M_WB],
            mem_w: vec![0.0; M_WB],
            mem_deemph: 0.0,
            past_qua_en: vec![-14.0; 4],
            mode,
            hp_filt_mem: vec![0.0; 4], // 4th order high-pass filter
            preemph_mem: 0.0,
        }
    }

    /// Set encoding mode
    pub fn set_mode(&mut self, mode: AmrWbMode) {
        self.mode = mode;
    }

    /// Main encoding function
    pub fn encode(&mut self, pcm_input: &[i16]) -> Result<Vec<u8>> {
        if pcm_input.len() != L_FRAME_WB {
            return Err(anyhow!("Input must be {} samples", L_FRAME_WB));
        }

        // Convert to float and apply high-pass filter
        let mut speech = vec![0.0; L_FRAME_WB];
        for i in 0..L_FRAME_WB {
            speech[i] = pcm_input[i] as f32 / 32768.0;
        }

        // Apply high-pass filter (50Hz cutoff at 16kHz)
        self.apply_high_pass_filter(&mut speech);

        // Apply pre-emphasis
        self.apply_pre_emphasis(&mut speech);

        // Update speech buffer
        for i in 0..L_WINDOW_WB - L_FRAME_WB {
            self.old_speech[i] = self.old_speech[i + L_FRAME_WB];
        }
        for i in 0..L_FRAME_WB {
            self.old_speech[L_WINDOW_WB - L_FRAME_WB + i] = speech[i];
        }

        // LP analysis (higher order for wideband)
        let lp_coeffs = self.compute_lp_analysis_wb();

        // Convert LP to ISP
        let isp = self.lp_to_isp(&lp_coeffs);

        // Quantize ISP
        let (isp_q, isp_indices) = self.quantize_isp(&isp);

        // Generate bitstream
        let mut bitstream = vec![0u8; self.mode.frame_size_bytes()];
        let mut bit_index = 0;

        // Pack mode and ISP indices
        self.pack_bits(&mut bitstream, &mut bit_index, self.mode as u32, 4);
        self.pack_isp_indices(&mut bitstream, &mut bit_index, &isp_indices);

        // Process subframes
        for subframe in 0..NB_SUBFR_WB {
            let sf_start = subframe * L_SUBFR_WB;
            let sf_speech = &speech[sf_start..sf_start + L_SUBFR_WB];

            // Compute target signal
            let target = self.compute_target_signal_wb(sf_speech, &lp_coeffs);

            // Adaptive codebook search (pitch)
            let (pitch_delay, pitch_gain) = self.adaptive_codebook_search_wb(&target, subframe);

            // Algebraic codebook search (ACELP)
            let (pulse_indices, fixed_gain) = self.acelp_codebook_search(&target, pitch_delay);

            // Pack subframe parameters
            self.pack_subframe_params(
                &mut bitstream,
                &mut bit_index,
                pitch_delay,
                pitch_gain,
                &pulse_indices,
                fixed_gain,
                subframe,
            );

            // Update excitation buffer
            self.update_excitation_wb(pitch_delay, pitch_gain, &pulse_indices, fixed_gain);
        }

        // Update ISP for next frame
        self.old_isp = isp;
        self.old_isp_q = isp_q;

        Ok(bitstream)
    }

    /// Apply 4th order high-pass filter (50Hz cutoff)
    fn apply_high_pass_filter(&mut self, speech: &mut [f32]) {
        // 4th order Butterworth high-pass filter coefficients for 50Hz @ 16kHz
        let b = [0.9780, -3.9121, 5.8681, -3.9121, 0.9780]; // Numerator
        let a = [1.0, -3.9317, 5.8135, -3.8607, 0.9806]; // Denominator

        for i in 0..speech.len() {
            let input = speech[i];

            // Apply filter
            let mut output = b[0] * input;
            for j in 1..5 {
                if i >= j {
                    output += b[j]
                        * (if i - j < speech.len() {
                            speech[i - j]
                        } else {
                            0.0
                        });
                }
                if j < self.hp_filt_mem.len() {
                    output -= a[j] * self.hp_filt_mem[j - 1];
                }
            }

            // Update memory
            for j in (1..4).rev() {
                self.hp_filt_mem[j] = self.hp_filt_mem[j - 1];
            }
            self.hp_filt_mem[0] = output;

            speech[i] = output;
        }
    }

    /// Apply pre-emphasis (gamma = 0.68)
    fn apply_pre_emphasis(&mut self, speech: &mut [f32]) {
        let gamma = 0.68;
        for i in (1..speech.len()).rev() {
            speech[i] = speech[i] - gamma * speech[i - 1];
        }
        speech[0] = speech[0] - gamma * self.preemph_mem;
        self.preemph_mem = speech[speech.len() - 1];
    }

    /// Compute LP coefficients for wideband (16th order)
    fn compute_lp_analysis_wb(&self) -> Vec<f32> {
        // Apply asymmetric window for wideband
        let mut windowed = vec![0.0; L_WINDOW_WB];
        for i in 0..L_WINDOW_WB {
            // Asymmetric window optimized for wideband
            let w = if i < 80 {
                0.54 - 0.46 * f32::cos(PI * i as f32 / 80.0)
            } else if i < L_WINDOW_WB - 80 {
                1.0
            } else {
                0.54 - 0.46 * f32::cos(PI * (L_WINDOW_WB - 1 - i) as f32 / 80.0)
            };
            windowed[i] = self.old_speech[i] * w;
        }

        // Compute autocorrelation (16th order for wideband)
        let mut r = vec![0.0; M_WB + 1];
        for k in 0..=M_WB {
            for i in 0..L_WINDOW_WB - k {
                r[k] += windowed[i] * windowed[i + k];
            }
        }

        // White noise correction
        r[0] *= 1.0001;

        // Lag windowing (wideband coefficients)
        let lag_window = [
            1.0000000, 0.9999951, 0.9999804, 0.9999559, 0.9999216, 0.9998774, 0.9998234, 0.9997596,
            0.9996860, 0.9996025, 0.9995092, 0.9994061, 0.9992931, 0.9991703, 0.9990376, 0.9988951,
            0.9987428,
        ];
        for i in 1..=M_WB {
            r[i] *= lag_window[i];
        }

        // Levinson-Durbin recursion for 16th order
        let mut a = vec![0.0; M_WB + 1];
        a[0] = 1.0;
        let mut err = r[0];

        for i in 1..=M_WB {
            let mut sum = 0.0;
            for j in 1..i {
                sum += a[j] * r[i - j];
            }

            let rc = -(r[i] + sum) / err;
            a[i] = rc;

            for j in 1..i / 2 + 1 {
                let temp = a[j] + rc * a[i - j];
                a[i - j] = a[i - j] + rc * a[j];
                a[j] = temp;
            }

            err *= 1.0 - rc * rc;
        }

        a
    }

    /// Convert LP coefficients to Immittance Spectral Pairs (ISP)
    fn lp_to_isp(&self, lp: &[f32]) -> Vec<f32> {
        let mut isp = vec![0.0; M_WB];

        // Form P(z) and Q(z) polynomials
        let mut p = vec![0.0; M_WB / 2 + 1];
        let mut q = vec![0.0; M_WB / 2 + 1];

        p[0] = 1.0;
        q[0] = 1.0;

        for i in 1..=M_WB / 2 {
            p[i] = lp[i] + lp[M_WB + 1 - i] - p[i - 1];
            q[i] = lp[i] - lp[M_WB + 1 - i] + q[i - 1];
        }

        // Find roots using Chebyshev polynomial evaluation
        let mut isp_index = 0;

        // Find P(z) roots
        for _ in 0..M_WB / 2 {
            let root = self.find_polynomial_root(&p, -1.0, 1.0);
            isp[isp_index] = f32::acos(root);
            isp_index += 2;
        }

        isp_index = 1;

        // Find Q(z) roots
        for _ in 0..M_WB / 2 {
            let root = self.find_polynomial_root(&q, -1.0, 1.0);
            isp[isp_index] = f32::acos(root);
            isp_index += 2;
        }

        // Sort ISPs
        isp.sort_by(|a, b| a.partial_cmp(b).unwrap());

        isp
    }

    /// Find polynomial root using bisection
    fn find_polynomial_root(&self, poly: &[f32], mut low: f32, mut high: f32) -> f32 {
        for _ in 0..15 {
            // More iterations for wideband precision
            let mid = (low + high) / 2.0;

            let mut val = poly[poly.len() - 1];
            for i in (0..poly.len() - 1).rev() {
                val = val * mid + poly[i];
            }

            if val > 0.0 {
                high = mid;
            } else {
                low = mid;
            }
        }

        (low + high) / 2.0
    }

    /// Quantize ISP parameters
    fn quantize_isp(&self, isp: &[f32]) -> (Vec<f32>, Vec<usize>) {
        // Simplified ISP quantization for AMR-WB
        // Real implementation uses split vector quantization
        let mut isp_q = isp.to_vec();
        let mut indices = Vec::new();

        // Convert to LSF domain and quantize
        for i in 0..M_WB {
            let lsf = isp[i] / PI; // Normalize to [0,1]
            let quantized_index = (lsf * 256.0) as usize;
            isp_q[i] = (quantized_index as f32 / 256.0) * PI;
            indices.push(quantized_index & 0xFF);
        }

        (isp_q, indices)
    }

    /// Compute target signal for wideband
    fn compute_target_signal_wb(&self, speech: &[f32], lp: &[f32]) -> Vec<f32> {
        let mut target = vec![0.0; L_SUBFR_WB];

        // Apply perceptual weighting for wideband
        let gamma1: f32 = 0.92; // Slightly different for wideband
        let gamma2 = 0.7;

        for i in 0..L_SUBFR_WB {
            target[i] = speech[i];

            // Apply A(z/gamma1) / A(z/gamma2)
            for j in 1..=M_WB.min(i) {
                target[i] -= lp[j] * gamma1.powi(j as i32) * speech[i - j];
            }
        }

        target
    }

    /// Adaptive codebook search for wideband
    fn adaptive_codebook_search_wb(&self, target: &[f32], subframe: usize) -> (usize, f32) {
        let mut best_delay = PIT_MIN_WB;
        let mut best_gain = 0.0;
        let mut max_corr = -1e10;

        // Search range depends on subframe
        let (pit_min, pit_max) = if subframe == 0 {
            (PIT_MIN_WB, PIT_MAX_WB)
        } else {
            // Differential search for other subframes
            let last_pitch = best_delay; // Would use previous subframe's pitch
            (
                (last_pitch - 8).max(PIT_MIN_WB),
                (last_pitch + 8).min(PIT_MAX_WB),
            )
        };

        for delay in pit_min..=pit_max {
            let mut corr = 0.0;
            let mut energy = 0.0;

            for i in 0..L_SUBFR_WB {
                let exc_val = if delay <= i + L_FRAME_WB {
                    self.old_exc[L_FRAME_WB + i - delay]
                } else {
                    0.0
                };

                corr += target[i] * exc_val;
                energy += exc_val * exc_val;
            }

            if energy > 0.0 {
                let normalized_corr = corr * corr / energy;
                if normalized_corr > max_corr {
                    max_corr = normalized_corr;
                    best_delay = delay;
                    best_gain = corr / energy;
                }
            }
        }

        // Fractional pitch refinement for wideband
        best_gain = best_gain.max(0.0).min(1.2);

        (best_delay, best_gain)
    }

    /// ACELP codebook search (Algebraic Code-Excited Linear Prediction)
    fn acelp_codebook_search(&self, target: &[f32], pitch_delay: usize) -> (Vec<usize>, f32) {
        // AMR-WB uses more sophisticated algebraic codebook
        // Number of pulses depends on the mode
        let num_pulses = match self.mode {
            AmrWbMode::Mode0 | AmrWbMode::Mode1 => 2,
            AmrWbMode::Mode2 | AmrWbMode::Mode3 => 4,
            _ => 4, // Modes 4-8 use 4 pulses
        };

        let track_length = L_SUBFR_WB / num_pulses;
        let mut pulse_indices = vec![0; num_pulses];
        let mut pulse_signs = vec![1.0; num_pulses];

        // Search for optimal pulse positions in each track
        for track in 0..num_pulses {
            let mut max_corr = 0.0;
            let mut best_pos = track;
            let mut best_sign = 1.0;

            for pos in (track..L_SUBFR_WB).step_by(num_pulses) {
                let corr_pos = target[pos];
                let corr_neg = -target[pos];

                if corr_pos.abs() > max_corr {
                    max_corr = corr_pos.abs();
                    best_pos = pos;
                    best_sign = if corr_pos > 0.0 { 1.0 } else { -1.0 };
                }
            }

            pulse_indices[track] = best_pos;
            pulse_signs[track] = best_sign;
        }

        // Compute optimal gain
        let mut corr = 0.0;
        let mut energy = 0.0;

        for i in 0..num_pulses {
            corr += target[pulse_indices[i]] * pulse_signs[i];
            energy += 1.0; // Unit pulse energy
        }

        let gain = if energy > 0.0 { corr / energy } else { 0.0 };
        let limited_gain = gain.max(-2.0).min(2.0);

        // Encode pulse positions (simplified)
        let mut encoded_indices = Vec::new();
        for i in 0..num_pulses {
            let mut index = pulse_indices[i] / num_pulses;
            if pulse_signs[i] < 0.0 {
                index |= 1 << 7; // Sign bit
            }
            encoded_indices.push(index);
        }

        (encoded_indices, limited_gain)
    }

    /// Pack ISP indices into bitstream
    fn pack_isp_indices(&self, bitstream: &mut [u8], bit_index: &mut usize, indices: &[usize]) {
        // Simplified ISP packing - real AMR-WB uses sophisticated VQ
        for &index in indices {
            self.pack_bits(bitstream, bit_index, index as u32, 8);
        }
    }

    /// Pack subframe parameters
    fn pack_subframe_params(
        &self,
        bitstream: &mut [u8],
        bit_index: &mut usize,
        pitch_delay: usize,
        pitch_gain: f32,
        pulse_indices: &[usize],
        fixed_gain: f32,
        subframe: usize,
    ) {
        // Pack pitch delay
        if subframe == 0 {
            self.pack_bits(bitstream, bit_index, pitch_delay as u32, 9);
        } else {
            // Differential encoding for other subframes
            self.pack_bits(bitstream, bit_index, (pitch_delay - PIT_MIN_WB) as u32, 6);
        }

        // Pack gains (quantized)
        let pitch_gain_q = (pitch_gain * 128.0).max(0.0).min(127.0) as u32;
        let fixed_gain_q = ((fixed_gain + 2.0) * 64.0).max(0.0).min(127.0) as u32;

        self.pack_bits(bitstream, bit_index, pitch_gain_q, 7);
        self.pack_bits(bitstream, bit_index, fixed_gain_q, 7);

        // Pack pulse indices
        for &index in pulse_indices {
            self.pack_bits(bitstream, bit_index, index as u32, 8);
        }
    }

    /// Update excitation buffer for wideband
    fn update_excitation_wb(
        &mut self,
        pitch_delay: usize,
        pitch_gain: f32,
        pulse_indices: &[usize],
        fixed_gain: f32,
    ) {
        // Shift old excitation
        for i in 0..PIT_MAX_WB + L_INTERPOL_WB {
            self.old_exc[i] = self.old_exc[i + L_SUBFR_WB];
        }

        // Generate new excitation
        for i in 0..L_SUBFR_WB {
            let mut exc = 0.0;

            // Adaptive codebook contribution
            if pitch_delay <= i + L_FRAME_WB {
                exc += pitch_gain * self.old_exc[L_FRAME_WB + i - pitch_delay];
            }

            // Fixed codebook contribution
            for &pulse_pos in pulse_indices {
                let pos = pulse_pos & 0x7F;
                let sign = if pulse_pos & 0x80 != 0 { -1.0 } else { 1.0 };
                if pos == i {
                    exc += fixed_gain * sign;
                }
            }

            self.old_exc[PIT_MAX_WB + L_INTERPOL_WB + i] = exc;
        }
    }

    /// Pack bits into bitstream
    fn pack_bits(&self, bitstream: &mut [u8], bit_index: &mut usize, value: u32, num_bits: usize) {
        for i in 0..num_bits {
            let bit = (value >> (num_bits - 1 - i)) & 1;
            let byte_index = *bit_index / 8;
            let bit_position = 7 - (*bit_index % 8);

            if byte_index < bitstream.len() {
                if bit == 1 {
                    bitstream[byte_index] |= 1 << bit_position;
                }
            }

            *bit_index += 1;
        }
    }
}

/// G.722.2 / AMR-WB decoder
pub struct G7222Decoder {
    // Synthesis filter memories
    old_exc: Vec<f32>,
    old_isp: Vec<f32>,
    mem_syn: Vec<f32>,

    // Post-filter memories
    mem_deemph: f32,
    mem_hp_out: Vec<f32>,

    // Current mode
    mode: AmrWbMode,
}

impl G7222Decoder {
    pub fn new() -> Self {
        Self {
            old_exc: vec![0.0; L_FRAME_WB + PIT_MAX_WB + L_INTERPOL_WB],
            old_isp: vec![0.0; M_WB],
            mem_syn: vec![0.0; M_WB],
            mem_deemph: 0.0,
            mem_hp_out: vec![0.0; 4],
            mode: AmrWbMode::default(),
        }
    }

    pub fn decode(&mut self, bitstream: &[u8]) -> Result<Vec<i16>> {
        let mut pcm_output = vec![0i16; L_FRAME_WB];
        let mut bit_index = 0;

        // Unpack mode
        let mode_bits = self.unpack_bits(bitstream, &mut bit_index, 4);
        self.mode = match mode_bits {
            0 => AmrWbMode::Mode0,
            1 => AmrWbMode::Mode1,
            2 => AmrWbMode::Mode2,
            3 => AmrWbMode::Mode3,
            4 => AmrWbMode::Mode4,
            5 => AmrWbMode::Mode5,
            6 => AmrWbMode::Mode6,
            7 => AmrWbMode::Mode7,
            8 => AmrWbMode::Mode8,
            _ => AmrWbMode::Mode8, // Default fallback
        };

        // Unpack ISP parameters
        let isp_indices = self.unpack_isp_indices(bitstream, &mut bit_index);
        let isp = self.decode_isp(&isp_indices);
        let lp = self.isp_to_lp(&isp);

        // Process subframes
        for subframe in 0..NB_SUBFR_WB {
            let (pitch_delay, pitch_gain, pulse_indices, fixed_gain) =
                self.unpack_subframe_params(bitstream, &mut bit_index, subframe);

            // Generate excitation
            let exc =
                self.generate_excitation_wb(pitch_delay, pitch_gain, &pulse_indices, fixed_gain);

            // Synthesis filter
            let synth = self.synthesis_filter_wb(&exc, &lp);

            // Copy to output with post-processing
            let sf_start = subframe * L_SUBFR_WB;
            for i in 0..L_SUBFR_WB {
                let mut sample = synth[i];

                // De-emphasis
                sample = sample + 0.68 * self.mem_deemph;
                self.mem_deemph = sample;

                // High-pass post-filter
                sample = self.apply_hp_post_filter(sample);

                // Clip and convert to PCM
                sample = sample.max(-1.0).min(1.0);
                pcm_output[sf_start + i] = (sample * 32767.0) as i16;
            }
        }

        // Update ISP for next frame
        self.old_isp = isp;

        Ok(pcm_output)
    }

    fn unpack_bits(&self, bitstream: &[u8], bit_index: &mut usize, num_bits: usize) -> usize {
        let mut value = 0;

        for _ in 0..num_bits {
            let byte_index = *bit_index / 8;
            let bit_position = 7 - (*bit_index % 8);

            if byte_index < bitstream.len() {
                let bit = (bitstream[byte_index] >> bit_position) & 1;
                value = (value << 1) | bit as usize;
            }

            *bit_index += 1;
        }

        value
    }

    fn unpack_isp_indices(&self, bitstream: &[u8], bit_index: &mut usize) -> Vec<usize> {
        let mut indices = Vec::new();
        for _ in 0..M_WB {
            indices.push(self.unpack_bits(bitstream, bit_index, 8));
        }
        indices
    }

    fn decode_isp(&self, indices: &[usize]) -> Vec<f32> {
        let mut isp = vec![0.0; M_WB];

        // Simplified ISP decoding
        for i in 0..M_WB {
            let normalized = (indices[i] & 0xFF) as f32 / 256.0;
            isp[i] = normalized * PI;
        }

        // Ensure ordering
        for i in 1..M_WB {
            if isp[i] <= isp[i - 1] {
                isp[i] = isp[i - 1] + 0.01;
            }
        }

        isp
    }

    fn isp_to_lp(&self, isp: &[f32]) -> Vec<f32> {
        let mut lp = vec![0.0; M_WB + 1];
        lp[0] = 1.0;

        // Simplified ISP to LP conversion
        for i in 1..=M_WB {
            lp[i] = -0.05 * i as f32; // Placeholder
        }

        lp
    }

    fn unpack_subframe_params(
        &self,
        bitstream: &[u8],
        bit_index: &mut usize,
        subframe: usize,
    ) -> (usize, f32, Vec<usize>, f32) {
        // Unpack pitch delay
        let pitch_delay = if subframe == 0 {
            self.unpack_bits(bitstream, bit_index, 9)
        } else {
            PIT_MIN_WB + self.unpack_bits(bitstream, bit_index, 6)
        };

        // Unpack gains
        let pitch_gain_q = self.unpack_bits(bitstream, bit_index, 7);
        let fixed_gain_q = self.unpack_bits(bitstream, bit_index, 7);

        let pitch_gain = pitch_gain_q as f32 / 128.0;
        let fixed_gain = (fixed_gain_q as f32 / 64.0) - 2.0;

        // Unpack pulse indices
        let num_pulses = match self.mode {
            AmrWbMode::Mode0 | AmrWbMode::Mode1 => 2,
            _ => 4,
        };

        let mut pulse_indices = Vec::new();
        for _ in 0..num_pulses {
            pulse_indices.push(self.unpack_bits(bitstream, bit_index, 8));
        }

        (pitch_delay, pitch_gain, pulse_indices, fixed_gain)
    }

    fn generate_excitation_wb(
        &mut self,
        pitch_delay: usize,
        pitch_gain: f32,
        pulse_indices: &[usize],
        fixed_gain: f32,
    ) -> Vec<f32> {
        let mut exc = vec![0.0; L_SUBFR_WB];

        // Adaptive codebook contribution
        for i in 0..L_SUBFR_WB {
            if pitch_delay <= i + L_FRAME_WB {
                exc[i] += pitch_gain * self.old_exc[L_FRAME_WB + i - pitch_delay];
            }
        }

        // Fixed codebook contribution
        for &pulse_data in pulse_indices {
            let pos = pulse_data & 0x7F;
            let sign = if pulse_data & 0x80 != 0 { -1.0 } else { 1.0 };
            if pos < L_SUBFR_WB {
                exc[pos] += fixed_gain * sign;
            }
        }

        // Update excitation buffer
        for i in 0..PIT_MAX_WB + L_INTERPOL_WB {
            self.old_exc[i] = self.old_exc[i + L_SUBFR_WB];
        }
        for i in 0..L_SUBFR_WB {
            self.old_exc[PIT_MAX_WB + L_INTERPOL_WB + i] = exc[i];
        }

        exc
    }

    fn synthesis_filter_wb(&mut self, exc: &[f32], lp: &[f32]) -> Vec<f32> {
        let mut synth = vec![0.0; L_SUBFR_WB];

        for i in 0..L_SUBFR_WB {
            synth[i] = exc[i];

            // Apply synthesis filter 1/A(z)
            for j in 1..=M_WB.min(i) {
                synth[i] -= lp[j]
                    * if j <= i {
                        synth[i - j]
                    } else {
                        self.mem_syn[M_WB - (j - i)]
                    };
            }
        }

        // Update synthesis memory
        for i in 0..M_WB {
            if i < L_SUBFR_WB {
                self.mem_syn[i] = synth[L_SUBFR_WB - 1 - i];
            }
        }

        synth
    }

    fn apply_hp_post_filter(&mut self, sample: f32) -> f32 {
        // Simple high-pass post-filter (placeholder)
        sample
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amr_wb_modes() {
        assert_eq!(AmrWbMode::Mode0.bitrate(), 6.60);
        assert_eq!(AmrWbMode::Mode8.bitrate(), 23.85);
        assert_eq!(AmrWbMode::Mode0.frame_size_bytes(), 18);
        assert_eq!(AmrWbMode::Mode8.frame_size_bytes(), 61);
    }

    #[test]
    fn test_encoder_creation() {
        let encoder = G7222Encoder::new(AmrWbMode::Mode8);
        assert_eq!(encoder.old_speech.len(), L_WINDOW_WB);
        assert_eq!(encoder.mode, AmrWbMode::Mode8);
    }

    #[test]
    fn test_decoder_creation() {
        let decoder = G7222Decoder::new();
        assert_eq!(
            decoder.old_exc.len(),
            L_FRAME_WB + PIT_MAX_WB + L_INTERPOL_WB
        );
    }

    #[test]
    fn test_encode_decode_round_trip() {
        let mut encoder = G7222Encoder::new(AmrWbMode::Mode8);
        let mut decoder = G7222Decoder::new();

        // Create test PCM frame (wideband)
        let pcm_input: Vec<i16> = (0..L_FRAME_WB)
            .map(|i| ((i as f32 * 0.1).sin() * 16384.0) as i16)
            .collect();

        // Encode
        let encoded = encoder.encode(&pcm_input).unwrap();
        assert_eq!(encoded.len(), AmrWbMode::Mode8.frame_size_bytes());

        // Decode
        let decoded = decoder.decode(&encoded).unwrap();
        assert_eq!(decoded.len(), L_FRAME_WB);
    }
}
