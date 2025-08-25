/*
 * G.729 CELP (Code-Excited Linear Prediction) Implementation
 * Copyright (C) 2025 Carrier One Inc and contributors
 *
 * Based on ITU-T G.729 standard (patent-free since 2017)
 * CS-ACELP: Conjugate-Structure Algebraic-Code-Excited Linear Prediction
 */

use anyhow::{anyhow, Result};
use std::f32::consts::PI;

// G.729 constants
pub const L_FRAME: usize = 80; // Frame size (10ms at 8kHz)
pub const L_SUBFR: usize = 40; // Subframe size (5ms)
pub const M: usize = 10; // LP order
pub const L_WINDOW: usize = 240; // Window size for LP analysis
pub const L_NEXT: usize = 40; // Lookahead
pub const PIT_MIN: usize = 18; // Minimum pitch
pub const PIT_MAX: usize = 143; // Maximum pitch
pub const L_INTERPOL: usize = 11; // Interpolation filter length

// LSP quantizer dimensions
pub const NC0_B: usize = 7; // First stage LSP codebook bits
pub const NC1_B: usize = 5; // Second stage LSP codebook bits

/// G.729 encoder state
pub struct G729Encoder {
    // Speech buffers
    old_speech: Vec<f32>, // Old speech buffer for windowing
    old_exc: Vec<f32>,    // Old excitation buffer
    old_wsp: Vec<f32>,    // Weighted speech buffer

    // LP/LSP related
    old_lsp: Vec<f32>,   // Previous LSP
    old_lsp_q: Vec<f32>, // Previous quantized LSP

    // Synthesis filter memory
    mem_w: Vec<f32>,    // Error weighting filter memory
    mem_w0: Vec<f32>,   // Error weighting filter memory
    mem_zero: Vec<f32>, // Zero filter memory
    sharp: f32,         // Sharpening factor

    // Gains
    past_qua_en: Vec<f32>, // Past quantized energies

    // VAD/DTX (Annex A)
    pub vad_enable: bool,
    energy_hist: Vec<f32>,
    hangover_count: i32,
    sid_frame_count: i32,

    // CNG (Annex B)
    sid_gain: f32,
    cur_gain: f32,
    sid_update_counter: i32,
}

impl G729Encoder {
    pub fn new() -> Self {
        Self {
            old_speech: vec![0.0; L_WINDOW],
            old_exc: vec![0.0; L_FRAME + PIT_MAX + L_INTERPOL],
            old_wsp: vec![0.0; L_FRAME + PIT_MAX],
            old_lsp: vec![0.0; M],
            old_lsp_q: vec![0.0; M],
            mem_w: vec![0.0; M],
            mem_w0: vec![0.0; M],
            mem_zero: vec![0.0; M],
            sharp: 0.0,
            past_qua_en: vec![-14.0; 4], // Initialize with low energy
            vad_enable: false,
            energy_hist: vec![0.0; 10],
            hangover_count: 0,
            sid_frame_count: 0,
            sid_gain: 0.0,
            cur_gain: 0.0,
            sid_update_counter: 0,
        }
    }

    /// Main encoding function
    pub fn encode(&mut self, pcm_input: &[i16]) -> Result<Vec<u8>> {
        if pcm_input.len() != L_FRAME {
            return Err(anyhow!("Input must be {} samples", L_FRAME));
        }

        // Convert to float and apply pre-emphasis
        let mut speech = vec![0.0; L_FRAME];
        let preemph = 0.68;
        for i in 0..L_FRAME {
            let s = pcm_input[i] as f32 / 32768.0;
            speech[i] = s - preemph
                * if i > 0 {
                    pcm_input[i - 1] as f32 / 32768.0
                } else {
                    self.old_speech[L_WINDOW - 1]
                };
        }

        // Update speech buffer
        for i in 0..L_WINDOW - L_FRAME {
            self.old_speech[i] = self.old_speech[i + L_FRAME];
        }
        for i in 0..L_FRAME {
            self.old_speech[L_WINDOW - L_FRAME + i] = speech[i];
        }

        // LP analysis
        let lp_coeffs = self.compute_lp_analysis();

        // Convert LP to LSP
        let lsp = self.lp_to_lsp(&lp_coeffs);

        // Quantize LSP
        let (lsp_q, lsp_indices) = self.quantize_lsp(&lsp);

        // VAD decision
        let is_speech = if self.vad_enable {
            self.compute_vad_decision(&speech)
        } else {
            true
        };

        // Process subframes
        let mut bitstream = vec![0u8; 10]; // G.729 frame is 10 bytes
        let mut bit_index = 0;

        // Pack LSP indices (18 bits total)
        self.pack_bits(&mut bitstream, &mut bit_index, lsp_indices.0 as u32, 7);
        self.pack_bits(&mut bitstream, &mut bit_index, lsp_indices.1 as u32, 5);
        self.pack_bits(&mut bitstream, &mut bit_index, lsp_indices.2 as u32, 5);
        self.pack_bits(&mut bitstream, &mut bit_index, lsp_indices.3 as u32, 1);

        if !is_speech && self.vad_enable {
            // Generate SID frame (Annex B)
            self.generate_sid_frame(&mut bitstream);
        } else {
            // Process two subframes
            for subframe in 0..2 {
                let sf_start = subframe * L_SUBFR;
                let sf_speech = &speech[sf_start..sf_start + L_SUBFR];

                // Compute target signal
                let target = self.compute_target_signal(sf_speech, &lp_coeffs);

                // Adaptive codebook search (pitch)
                let (pitch_delay, pitch_gain) = self.adaptive_codebook_search(&target);

                // Fixed codebook search
                let (fixed_index, fixed_gain) = self.fixed_codebook_search(&target, pitch_delay);

                // Pack subframe parameters
                if subframe == 0 {
                    // First subframe: 8+1+3+13+4+3 = 32 bits
                    self.pack_bits(&mut bitstream, &mut bit_index, pitch_delay as u32, 8);
                    self.pack_bits(&mut bitstream, &mut bit_index, 0, 1); // Parity bit
                    self.pack_bits(&mut bitstream, &mut bit_index, fixed_index as u32, 13);
                    self.pack_bits(
                        &mut bitstream,
                        &mut bit_index,
                        self.quantize_gains(pitch_gain, fixed_gain) as u32,
                        7,
                    );
                } else {
                    // Second subframe: 5+13+4+3 = 25 bits
                    self.pack_bits(&mut bitstream, &mut bit_index, (pitch_delay - 18) as u32, 5); // Differential
                    self.pack_bits(&mut bitstream, &mut bit_index, fixed_index as u32, 13);
                    self.pack_bits(
                        &mut bitstream,
                        &mut bit_index,
                        self.quantize_gains(pitch_gain, fixed_gain) as u32,
                        7,
                    );
                }

                // Update excitation buffer
                self.update_excitation(pitch_delay, pitch_gain, fixed_index, fixed_gain);
            }
        }

        // Update LSP for next frame
        self.old_lsp = lsp;
        self.old_lsp_q = lsp_q;

        Ok(bitstream)
    }

    /// Compute LP coefficients using autocorrelation and Levinson-Durbin
    fn compute_lp_analysis(&self) -> Vec<f32> {
        // Apply Hamming window
        let mut windowed = vec![0.0; L_WINDOW];
        for i in 0..L_WINDOW {
            let window = 0.54 - 0.46 * f32::cos(2.0 * PI * i as f32 / (L_WINDOW - 1) as f32);
            windowed[i] = self.old_speech[i] * window;
        }

        // Compute autocorrelation
        let mut r = vec![0.0; M + 1];
        for k in 0..=M {
            for i in 0..L_WINDOW - k {
                r[k] += windowed[i] * windowed[i + k];
            }
        }

        // Lag windowing
        let lag_window = [
            1.00000000, 0.99879038, 0.99518473, 0.98921439, 0.98092961, 0.97039264, 0.95767454,
            0.94285714, 0.92603099, 0.90729493, 0.88675135,
        ];
        for i in 1..=M {
            r[i] *= lag_window[i];
        }

        // Levinson-Durbin recursion
        let mut a = vec![0.0; M + 1];
        a[0] = 1.0;
        let mut k = vec![0.0; M];
        let mut err = r[0];

        for i in 1..=M {
            let mut sum = 0.0;
            for j in 1..i {
                sum += a[j] * r[i - j];
            }

            k[i - 1] = -(r[i] + sum) / err;
            a[i] = k[i - 1];

            for j in 1..i / 2 + 1 {
                let temp = a[j] + k[i - 1] * a[i - j];
                a[i - j] = a[i - j] + k[i - 1] * a[j];
                a[j] = temp;
            }

            err *= 1.0 - k[i - 1] * k[i - 1];
        }

        a
    }

    /// Convert LP coefficients to Line Spectral Pairs (LSP)
    fn lp_to_lsp(&self, lp: &[f32]) -> Vec<f32> {
        let mut lsp = vec![0.0; M];

        // Form P(z) and Q(z) polynomials
        let mut p = vec![0.0; M / 2 + 1];
        let mut q = vec![0.0; M / 2 + 1];

        p[0] = 1.0;
        q[0] = 1.0;

        for i in 1..=M / 2 {
            p[i] = lp[i] + lp[M + 1 - i] - p[i - 1];
            q[i] = lp[i] - lp[M + 1 - i] + q[i - 1];
        }

        // Find roots using Chebyshev polynomial evaluation
        let mut lsp_index = 0;

        // Find P(z) roots (even indices)
        for _ in 0..M / 2 {
            let root = self.find_polynomial_root(&p, -1.0, 1.0);
            lsp[lsp_index] = f32::acos(root);
            lsp_index += 2;
        }

        lsp_index = 1;

        // Find Q(z) roots (odd indices)
        for _ in 0..M / 2 {
            let root = self.find_polynomial_root(&q, -1.0, 1.0);
            lsp[lsp_index] = f32::acos(root);
            lsp_index += 2;
        }

        // Sort LSPs
        lsp.sort_by(|a, b| a.partial_cmp(b).unwrap());

        lsp
    }

    /// Find polynomial root using bisection
    fn find_polynomial_root(&self, poly: &[f32], mut low: f32, mut high: f32) -> f32 {
        for _ in 0..10 {
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

    /// Quantize LSP parameters
    fn quantize_lsp(&self, lsp: &[f32]) -> (Vec<f32>, (usize, usize, usize, usize)) {
        // Simplified LSP quantization
        // In reality, this would use trained codebooks
        let mut lsp_q = lsp.to_vec();

        // Simple uniform quantization for demonstration
        for i in 0..M {
            let normalized = lsp[i] / PI;
            let quantized_index = (normalized * 128.0) as i32;
            lsp_q[i] = (quantized_index as f32 / 128.0) * PI;
        }

        // Return quantized LSPs and indices (simplified)
        (lsp_q, (0, 0, 0, 0))
    }

    /// Compute target signal for codebook search
    fn compute_target_signal(&self, speech: &[f32], lp: &[f32]) -> Vec<f32> {
        let mut target = vec![0.0; L_SUBFR];

        // Apply perceptual weighting filter
        let gamma1: f32 = 0.94;
        let gamma2: f32 = 0.6;

        for i in 0..L_SUBFR {
            target[i] = speech[i];

            // Apply A(z/gamma1)
            for j in 1..=M.min(i) {
                target[i] -= lp[j] * gamma1.powi(j as i32) * speech[i - j];
            }
        }

        target
    }

    /// Adaptive codebook search (pitch prediction)
    fn adaptive_codebook_search(&self, target: &[f32]) -> (usize, f32) {
        let mut best_delay = PIT_MIN;
        let mut best_gain = 0.0;
        let mut max_corr = -1e10;

        // Search for best pitch delay
        for delay in PIT_MIN..=PIT_MAX {
            let mut corr = 0.0;
            let mut energy = 0.0;

            // Compute correlation with past excitation
            for i in 0..L_SUBFR {
                let exc_val = if delay <= i + L_FRAME {
                    self.old_exc[L_FRAME + i - delay]
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

        // Limit gain
        best_gain = best_gain.max(0.0).min(1.2);

        (best_delay, best_gain)
    }

    /// Fixed codebook search (algebraic structure)
    fn fixed_codebook_search(&self, target: &[f32], pitch_delay: usize) -> (usize, f32) {
        // G.729 uses 17-bit algebraic codebook
        // 4 pulses in specific tracks
        let tracks = [
            vec![0, 5, 10, 15, 20, 25, 30, 35], // Track 0
            vec![1, 6, 11, 16, 21, 26, 31, 36], // Track 1
            vec![2, 7, 12, 17, 22, 27, 32, 37], // Track 2
            vec![3, 8, 13, 18, 23, 28, 33, 38], // Track 3
        ];

        let mut best_index = 0;
        let mut best_gain = 0.0;
        let max_criterion = -1e10;

        // Simplified search: find best pulse position in each track
        let mut pulse_positions = vec![0; 4];
        let mut pulse_signs = vec![1.0; 4];

        for (track_idx, track) in tracks.iter().enumerate() {
            let mut track_max = 0.0;

            for &pos in track {
                if pos < L_SUBFR {
                    let val = target[pos].abs();
                    if val > track_max {
                        track_max = val;
                        pulse_positions[track_idx] = pos;
                        pulse_signs[track_idx] = if target[pos] > 0.0 { 1.0 } else { -1.0 };
                    }
                }
            }
        }

        // Encode pulse positions and signs into index
        for i in 0..4 {
            best_index = (best_index << 3) | (pulse_positions[i] / 5);
            if pulse_signs[i] < 0.0 {
                best_index |= 1 << (12 - i);
            }
        }

        // Compute optimal gain
        let mut corr = 0.0;
        let mut energy = 0.0;

        for i in 0..4 {
            corr += target[pulse_positions[i]] * pulse_signs[i];
            energy += 1.0; // Each pulse has unit energy
        }

        if energy > 0.0 {
            best_gain = corr / energy;
        }

        // Limit gain
        best_gain = best_gain.max(-2.0).min(2.0);

        (best_index, best_gain)
    }

    /// Quantize adaptive and fixed codebook gains
    fn quantize_gains(&self, pitch_gain: f32, fixed_gain: f32) -> usize {
        // Simplified gain quantization
        // Real implementation uses trained codebooks
        let gp_index = ((pitch_gain * 16.0).max(0.0).min(31.0)) as usize;
        let gc_index = ((fixed_gain.abs() * 8.0).max(0.0).min(15.0)) as usize;

        (gp_index << 4) | gc_index
    }

    /// Update excitation buffer
    fn update_excitation(
        &mut self,
        pitch_delay: usize,
        pitch_gain: f32,
        fixed_index: usize,
        fixed_gain: f32,
    ) {
        // Shift old excitation
        for i in 0..PIT_MAX + L_INTERPOL {
            self.old_exc[i] = self.old_exc[i + L_SUBFR];
        }

        // Generate new excitation
        for i in 0..L_SUBFR {
            let mut exc = 0.0;

            // Adaptive codebook contribution
            if pitch_delay <= i + L_FRAME {
                exc += pitch_gain * self.old_exc[L_FRAME + i - pitch_delay];
            }

            // Fixed codebook contribution (simplified)
            // Decode pulse positions from index
            for pulse in 0..4 {
                let pos = ((fixed_index >> (pulse * 3)) & 0x7) * 5 + pulse;
                if pos == i {
                    let sign = if (fixed_index >> (12 - pulse)) & 1 == 1 {
                        -1.0
                    } else {
                        1.0
                    };
                    exc += fixed_gain * sign;
                }
            }

            self.old_exc[PIT_MAX + L_INTERPOL + i] = exc;
        }
    }

    /// VAD decision
    fn compute_vad_decision(&mut self, speech: &[f32]) -> bool {
        // Compute frame energy
        let mut energy = 0.0;
        for &sample in speech {
            energy += sample * sample;
        }
        energy = 10.0 * f32::log10(energy / L_FRAME as f32 + 1e-10);

        // Simple energy-based VAD
        let vad_threshold = -35.0;
        let is_speech = energy > vad_threshold;

        // Hangover logic
        if !is_speech && self.hangover_count > 0 {
            self.hangover_count -= 1;
            return true;
        } else if is_speech {
            self.hangover_count = 5; // 50ms hangover
        }

        // Update energy history
        self.energy_hist.rotate_right(1);
        self.energy_hist[0] = energy;

        is_speech
    }

    /// Generate SID frame for DTX
    fn generate_sid_frame(&mut self, bitstream: &mut [u8]) {
        // Simplified SID frame generation
        // Contains comfort noise parameters
        bitstream[0] = 0x00; // SID marker

        // Quantize and pack energy
        let avg_energy = self.energy_hist.iter().sum::<f32>() / self.energy_hist.len() as f32;
        let energy_index = ((avg_energy + 60.0) * 2.0).max(0.0).min(63.0) as u8;
        bitstream[1] = energy_index;

        // Rest is reserved/padding
        for i in 2..10 {
            bitstream[i] = 0;
        }
    }

    /// Pack bits into bitstream
    fn pack_bits(&self, bitstream: &mut [u8], bit_index: &mut usize, value: u32, num_bits: usize) {
        for i in 0..num_bits {
            let bit = (value >> (num_bits - 1 - i)) & 1;
            let byte_index = *bit_index / 8;
            let bit_position = 7 - (*bit_index % 8);

            if bit == 1 {
                bitstream[byte_index] |= 1 << bit_position;
            }

            *bit_index += 1;
        }
    }
}

/// G.729 decoder
pub struct G729Decoder {
    // Synthesis filter memories
    old_exc: Vec<f32>,
    old_lsp: Vec<f32>,
    mem_syn: Vec<f32>,

    // Post-filter memories
    mem_deemph: f32,

    // CNG state
    cng_seed: u32,
    sid_gain: f32,
}

impl G729Decoder {
    pub fn new() -> Self {
        Self {
            old_exc: vec![0.0; L_FRAME + PIT_MAX + L_INTERPOL],
            old_lsp: vec![0.0; M],
            mem_syn: vec![0.0; M],
            mem_deemph: 0.0,
            cng_seed: 12345,
            sid_gain: 0.0,
        }
    }

    pub fn decode(&mut self, bitstream: &[u8]) -> Result<Vec<i16>> {
        if bitstream.len() != 10 {
            return Err(anyhow!("G.729 frame must be 10 bytes"));
        }

        let mut pcm_output = vec![0i16; L_FRAME];
        let mut bit_index = 0;

        // Check for SID frame
        let is_sid = bitstream[0] & 0x80 == 0;

        if is_sid {
            // Generate comfort noise
            self.generate_comfort_noise(&mut pcm_output);
        } else {
            // Decode speech frame
            // Unpack LSP indices
            let lsp_index1 = self.unpack_bits(bitstream, &mut bit_index, 7);
            let lsp_index2 = self.unpack_bits(bitstream, &mut bit_index, 5);
            let lsp_index3 = self.unpack_bits(bitstream, &mut bit_index, 5);
            let lsp_index4 = self.unpack_bits(bitstream, &mut bit_index, 1);

            // Decode LSP (simplified)
            let lsp = self.decode_lsp(lsp_index1, lsp_index2, lsp_index3, lsp_index4);

            // Convert LSP to LP
            let lp = self.lsp_to_lp(&lsp);

            // Process subframes
            for subframe in 0..2 {
                let (pitch_delay, fixed_index, gain_index) = if subframe == 0 {
                    let pitch = self.unpack_bits(bitstream, &mut bit_index, 8);
                    let _parity = self.unpack_bits(bitstream, &mut bit_index, 1);
                    let fixed = self.unpack_bits(bitstream, &mut bit_index, 13);
                    let gain = self.unpack_bits(bitstream, &mut bit_index, 7);
                    (pitch, fixed, gain)
                } else {
                    let pitch_diff = self.unpack_bits(bitstream, &mut bit_index, 5);
                    let fixed = self.unpack_bits(bitstream, &mut bit_index, 13);
                    let gain = self.unpack_bits(bitstream, &mut bit_index, 7);
                    (pitch_diff + 18, fixed, gain)
                };

                // Decode gains
                let (pitch_gain, fixed_gain) = self.decode_gains(gain_index);

                // Generate excitation
                let exc =
                    self.generate_excitation(pitch_delay, pitch_gain, fixed_index, fixed_gain);

                // Synthesis filter
                let synth = self.synthesis_filter(&exc, &lp);

                // Copy to output with de-emphasis
                let sf_start = subframe * L_SUBFR;
                for i in 0..L_SUBFR {
                    let mut sample = synth[i] + 0.68 * self.mem_deemph;
                    self.mem_deemph = sample;

                    // Clip and convert to PCM
                    sample = sample.max(-1.0).min(1.0);
                    pcm_output[sf_start + i] = (sample * 32767.0) as i16;
                }
            }

            // Update LSP for next frame
            self.old_lsp = lsp;
        }

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

    fn decode_lsp(&self, idx1: usize, idx2: usize, idx3: usize, idx4: usize) -> Vec<f32> {
        // Simplified LSP decoding
        let mut lsp = vec![0.0; M];

        // Generate evenly spaced LSPs (simplified)
        for i in 0..M {
            lsp[i] = (i + 1) as f32 * PI / (M + 1) as f32;
        }

        lsp
    }

    fn lsp_to_lp(&self, lsp: &[f32]) -> Vec<f32> {
        let mut lp = vec![0.0; M + 1];
        lp[0] = 1.0;

        // Simplified LSP to LP conversion
        // Real implementation uses Chebyshev polynomials
        for i in 1..=M {
            lp[i] = -0.1 * i as f32; // Placeholder
        }

        lp
    }

    fn decode_gains(&self, gain_index: usize) -> (f32, f32) {
        let pitch_gain = ((gain_index >> 4) & 0x1F) as f32 / 16.0;
        let fixed_gain = (gain_index & 0x0F) as f32 / 8.0;

        (pitch_gain, fixed_gain)
    }

    fn generate_excitation(
        &mut self,
        pitch_delay: usize,
        pitch_gain: f32,
        fixed_index: usize,
        fixed_gain: f32,
    ) -> Vec<f32> {
        let mut exc = vec![0.0; L_SUBFR];

        // Adaptive codebook contribution
        for i in 0..L_SUBFR {
            if pitch_delay <= i + L_FRAME {
                exc[i] += pitch_gain * self.old_exc[L_FRAME + i - pitch_delay];
            }
        }

        // Fixed codebook contribution
        for pulse in 0..4 {
            let pos = ((fixed_index >> (pulse * 3)) & 0x7) * 5 + pulse;
            if pos < L_SUBFR {
                let sign = if (fixed_index >> (12 - pulse)) & 1 == 1 {
                    -1.0
                } else {
                    1.0
                };
                exc[pos] += fixed_gain * sign;
            }
        }

        // Update excitation buffer
        for i in 0..PIT_MAX + L_INTERPOL {
            self.old_exc[i] = self.old_exc[i + L_SUBFR];
        }
        for i in 0..L_SUBFR {
            self.old_exc[PIT_MAX + L_INTERPOL + i] = exc[i];
        }

        exc
    }

    fn synthesis_filter(&mut self, exc: &[f32], lp: &[f32]) -> Vec<f32> {
        let mut synth = vec![0.0; L_SUBFR];

        for i in 0..L_SUBFR {
            synth[i] = exc[i];

            // Apply synthesis filter 1/A(z)
            for j in 1..=M.min(i) {
                synth[i] -= lp[j]
                    * if j <= i {
                        synth[i - j]
                    } else {
                        self.mem_syn[M - (j - i)]
                    };
            }
        }

        // Update synthesis memory
        for i in 0..M {
            if i < L_SUBFR {
                self.mem_syn[i] = synth[L_SUBFR - 1 - i];
            }
        }

        synth
    }

    fn generate_comfort_noise(&mut self, pcm_output: &mut [i16]) {
        // Simple white noise generation
        for i in 0..L_FRAME {
            // Linear congruential generator
            self.cng_seed = self.cng_seed.wrapping_mul(1103515245).wrapping_add(12345);
            let noise = ((self.cng_seed / 65536) % 32768) as i16 - 16384;

            // Apply gain
            pcm_output[i] = (noise as f32 * self.sid_gain) as i16;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoder_creation() {
        let encoder = G729Encoder::new();
        assert_eq!(encoder.old_speech.len(), L_WINDOW);
        assert_eq!(encoder.old_exc.len(), L_FRAME + PIT_MAX + L_INTERPOL);
    }

    #[test]
    fn test_decoder_creation() {
        let decoder = G729Decoder::new();
        assert_eq!(decoder.old_exc.len(), L_FRAME + PIT_MAX + L_INTERPOL);
    }

    #[test]
    fn test_encode_decode_round_trip() {
        let mut encoder = G729Encoder::new();
        let mut decoder = G729Decoder::new();

        // Create test PCM frame
        let pcm_input: Vec<i16> = (0..L_FRAME)
            .map(|i| ((i as f32 * 0.1).sin() * 16384.0) as i16)
            .collect();

        // Encode
        let encoded = encoder.encode(&pcm_input).unwrap();
        assert_eq!(encoded.len(), 10);

        // Decode
        let decoded = decoder.decode(&encoded).unwrap();
        assert_eq!(decoded.len(), L_FRAME);
    }
}
