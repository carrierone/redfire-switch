/*
 * G.729 Codec Implementation
 * ITU-T G.729 standard codec implementation (patents expired)
 * 8kHz sampling, 8kbps bitrate, 10ms frame size
 */

use anyhow::{anyhow, Result};
// VecDeque import removed - not used

/// G.729 frame size constants
pub const G729_FRAME_SIZE: usize = 80; // 80 samples per 10ms frame at 8kHz
pub const G729_ENCODED_SIZE: usize = 10; // 10 bytes per encoded frame
pub const G729_SAMPLE_RATE: u32 = 8000; // 8kHz sampling rate

/// G.729 encoder/decoder state
#[derive(Debug, Clone)]
pub struct G729Codec {
    /// Previous speech samples for prediction
    old_speech: [f32; 240],
    /// Previous excitation for pitch analysis  
    old_exc: [f32; 154],
    /// Previous LSP quantization
    lsp_old: [f32; 10],
    /// Speech analysis window
    window: [f32; 240],
    /// Autocorrelation analysis
    autocorr: [f32; 11],
    /// Reflection coefficients
    refl_coeff: [f32; 10],
    /// Linear prediction coefficients
    lp_coeff: [f32; 11],
    /// Quantization table for LSP
    lsf_q_table: Vec<Vec<f32>>,
    /// Frame counter for periodic resets
    frame_count: u32,
}

/// G.729 frame structure
#[derive(Debug, Clone)]
pub struct G729Frame {
    /// Line Spectral Pairs index
    pub lsp_index: u16,
    /// Adaptive codebook parameters (2 subframes)
    pub pitch_lag: [u8; 2],
    pub pitch_gain: [u8; 2],
    /// Fixed codebook parameters (2 subframes)  
    pub fixed_index: [u16; 2],
    pub fixed_sign: [u8; 2],
    pub fixed_gain: [u8; 2],
}

impl G729Codec {
    /// Create new G.729 codec instance
    pub fn new() -> Self {
        let mut codec = Self {
            old_speech: [0.0; 240],
            old_exc: [0.0; 154],
            lsp_old: [0.0; 10],
            window: [0.0; 240],
            autocorr: [0.0; 11],
            refl_coeff: [0.0; 10],
            lp_coeff: [0.0; 11],
            lsf_q_table: Self::initialize_lsf_quantization_table(),
            frame_count: 0,
        };

        // Initialize analysis window (Hamming window)
        codec.initialize_analysis_window();

        // Initialize LSP to stable values
        for i in 0..10 {
            codec.lsp_old[i] = (i + 1) as f32 * std::f32::consts::PI / 11.0;
        }

        codec
    }

    /// Initialize Hamming window for speech analysis
    fn initialize_analysis_window(&mut self) {
        for i in 0..240 {
            let n = i as f32;
            self.window[i] = 0.54 - 0.46 * (2.0 * std::f32::consts::PI * n / 239.0).cos();
        }
    }

    /// Initialize LSF quantization tables (simplified)
    fn initialize_lsf_quantization_table() -> Vec<Vec<f32>> {
        // Simplified quantization table - real G.729 has complex multi-stage tables
        let mut table = Vec::new();

        // Stage 1: Coarse quantization
        for i in 0..256 {
            let mut entry = Vec::new();
            for j in 0..10 {
                entry.push((i as f32 / 256.0 + j as f32 / 10.0) * 0.1);
            }
            table.push(entry);
        }

        table
    }

    /// Encode speech frame to G.729 bitstream
    pub fn encode(&mut self, speech: &[i16]) -> Result<Vec<u8>> {
        if speech.len() != G729_FRAME_SIZE {
            return Err(anyhow!(
                "Invalid frame size: expected {}, got {}",
                G729_FRAME_SIZE,
                speech.len()
            ));
        }

        // Convert to floating point and normalize
        let mut speech_f: [f32; G729_FRAME_SIZE] = [0.0; G729_FRAME_SIZE];
        for (i, &sample) in speech.iter().enumerate() {
            speech_f[i] = sample as f32 / 32768.0;
        }

        // Pre-processing: high-pass filter
        self.high_pass_filter(&mut speech_f);

        // Update speech buffer
        for i in 0..(240 - G729_FRAME_SIZE) {
            self.old_speech[i] = self.old_speech[i + G729_FRAME_SIZE];
        }
        for i in 0..G729_FRAME_SIZE {
            self.old_speech[240 - G729_FRAME_SIZE + i] = speech_f[i];
        }

        // Linear Prediction Analysis
        let lp_coeffs = self.lp_analysis()?;

        // Convert LP coefficients to Line Spectral Pairs
        let lsp = self.lp_to_lsp(&lp_coeffs)?;

        // Quantize LSP parameters
        let lsp_index = self.quantize_lsp(&lsp)?;

        // Perceptual weighting
        let weighted_speech = self.perceptual_weighting(&speech_f, &lp_coeffs)?;

        // Pitch analysis and adaptive codebook search
        let (pitch_lag, pitch_gain) = self.pitch_analysis(&weighted_speech)?;

        // Fixed codebook search
        let (fixed_index, fixed_sign, fixed_gain) =
            self.fixed_codebook_search(&weighted_speech, &pitch_lag)?;

        // Create G.729 frame
        let frame = G729Frame {
            lsp_index,
            pitch_lag,
            pitch_gain,
            fixed_index,
            fixed_sign,
            fixed_gain,
        };

        // Pack frame into bitstream
        let encoded = self.pack_frame(&frame)?;

        self.frame_count += 1;
        Ok(encoded)
    }

    /// Decode G.729 bitstream to speech samples
    pub fn decode(&mut self, encoded: &[u8]) -> Result<Vec<i16>> {
        if encoded.len() != G729_ENCODED_SIZE {
            return Err(anyhow!(
                "Invalid encoded frame size: expected {}, got {}",
                G729_ENCODED_SIZE,
                encoded.len()
            ));
        }

        // Unpack bitstream to frame parameters
        let frame = self.unpack_frame(encoded)?;

        // Dequantize LSP parameters
        let lsp = self.dequantize_lsp(frame.lsp_index)?;

        // Convert LSP to LP coefficients
        let lp_coeffs = self.lsp_to_lp(&lsp)?;

        // Decode adaptive codebook
        let adaptive_excitation =
            self.decode_adaptive_codebook(&frame.pitch_lag, &frame.pitch_gain)?;

        // Decode fixed codebook
        let fixed_excitation =
            self.decode_fixed_codebook(&frame.fixed_index, &frame.fixed_sign, &frame.fixed_gain)?;

        // Combine excitations
        let mut total_excitation = [0.0f32; G729_FRAME_SIZE];
        for i in 0..G729_FRAME_SIZE {
            total_excitation[i] = adaptive_excitation[i] + fixed_excitation[i];
        }

        // LP synthesis filter
        let mut speech_f = self.lp_synthesis(&total_excitation, &lp_coeffs)?;

        // Post-processing: high-pass filter and de-emphasis
        self.post_processing(&mut speech_f);

        // Convert to 16-bit PCM
        let mut speech_pcm = Vec::with_capacity(G729_FRAME_SIZE);
        for &sample in speech_f.iter() {
            let pcm_sample = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
            speech_pcm.push(pcm_sample);
        }

        self.frame_count += 1;
        Ok(speech_pcm)
    }

    /// High-pass filter preprocessing
    fn high_pass_filter(&self, speech: &mut [f32]) {
        // Simple 1st order high-pass filter
        let mut x1 = 0.0f32;
        let mut y1 = 0.0f32;

        for sample in speech.iter_mut() {
            let x0 = *sample;
            let y0 = 0.93 * (x0 - x1) + 0.93 * y1;
            *sample = y0;
            x1 = x0;
            y1 = y0;
        }
    }

    /// Linear Prediction analysis using autocorrelation method
    fn lp_analysis(&mut self) -> Result<[f32; 11]> {
        // Apply window to speech
        let mut windowed = [0.0f32; 240];
        for i in 0..240 {
            windowed[i] = self.old_speech[i] * self.window[i];
        }

        // Compute autocorrelation
        for k in 0..11 {
            self.autocorr[k] = 0.0;
            for i in 0..(240 - k) {
                self.autocorr[k] += windowed[i] * windowed[i + k];
            }
        }

        // Add white noise floor
        self.autocorr[0] *= 1.0001;

        // Levinson-Durbin algorithm
        let mut lp_coeffs = [0.0f32; 11];
        lp_coeffs[0] = 1.0;

        if self.autocorr[0] == 0.0 {
            return Ok(lp_coeffs);
        }

        let mut k = [0.0f32; 11];
        let mut e = self.autocorr[0];

        for i in 1..11 {
            let mut sum = 0.0;
            for j in 1..i {
                sum += lp_coeffs[j] * self.autocorr[i - j];
            }

            k[i] = -(self.autocorr[i] + sum) / e;
            lp_coeffs[i] = k[i];

            for j in 1..(i / 2 + 1) {
                let temp = lp_coeffs[j] + k[i] * lp_coeffs[i - j];
                lp_coeffs[i - j] += k[i] * lp_coeffs[j];
                lp_coeffs[j] = temp;
            }

            e *= 1.0 - k[i] * k[i];
        }

        Ok(lp_coeffs)
    }

    /// Convert LP coefficients to Line Spectral Pairs
    fn lp_to_lsp(&self, lp_coeffs: &[f32; 11]) -> Result<[f32; 10]> {
        // Simplified LSP computation - real G.729 uses Chebyshev polynomials
        let mut lsp = [0.0f32; 10];

        for i in 0..10 {
            lsp[i] = (i + 1) as f32 * std::f32::consts::PI / 11.0;

            // Add small perturbation based on LP coefficients
            if i < 10 {
                lsp[i] += lp_coeffs[i + 1] * 0.1;
            }
        }

        // Ensure LSP ordering
        for i in 1..10 {
            if lsp[i] <= lsp[i - 1] {
                lsp[i] = lsp[i - 1] + 0.01;
            }
        }

        Ok(lsp)
    }

    /// Convert Line Spectral Pairs to LP coefficients
    fn lsp_to_lp(&self, lsp: &[f32; 10]) -> Result<[f32; 11]> {
        // Simplified conversion - real G.729 uses polynomial evaluation
        let mut lp_coeffs = [0.0f32; 11];
        lp_coeffs[0] = 1.0;

        for i in 0..10 {
            lp_coeffs[i + 1] = (lsp[i] - (i + 1) as f32 * std::f32::consts::PI / 11.0) * 10.0;
        }

        Ok(lp_coeffs)
    }

    /// Quantize LSP parameters
    fn quantize_lsp(&self, lsp: &[f32; 10]) -> Result<u16> {
        // Simplified quantization - real G.729 uses split vector quantization
        let mut best_index = 0u16;
        let mut best_distance = f32::INFINITY;

        for (index, entry) in self.lsf_q_table.iter().enumerate().take(256) {
            let mut distance = 0.0f32;
            for i in 0..10 {
                let diff = lsp[i] - entry[i];
                distance += diff * diff;
            }

            if distance < best_distance {
                best_distance = distance;
                best_index = index as u16;
            }
        }

        Ok(best_index)
    }

    /// Dequantize LSP parameters
    fn dequantize_lsp(&self, index: u16) -> Result<[f32; 10]> {
        if (index as usize) >= self.lsf_q_table.len() {
            return Err(anyhow!("Invalid LSP quantization index: {}", index));
        }

        let mut lsp = [0.0f32; 10];
        let entry = &self.lsf_q_table[index as usize];

        for i in 0..10 {
            lsp[i] = entry[i];
        }

        Ok(lsp)
    }

    /// Perceptual weighting filter
    fn perceptual_weighting(
        &self,
        speech: &[f32],
        lp_coeffs: &[f32; 11],
    ) -> Result<[f32; G729_FRAME_SIZE]> {
        let mut weighted = [0.0f32; G729_FRAME_SIZE];

        // Apply perceptual weighting W(z) = A(z/γ₁) / A(z/γ₂)
        // Simplified implementation
        for i in 0..G729_FRAME_SIZE {
            weighted[i] = speech[i];

            // Apply weighting based on spectral properties
            for j in 1..11.min(i + 1) {
                weighted[i] -= lp_coeffs[j] * 0.6_f32.powi(j as i32) * speech[i - j];
            }
        }

        Ok(weighted)
    }

    /// Pitch analysis and adaptive codebook search
    fn pitch_analysis(&mut self, weighted_speech: &[f32]) -> Result<([u8; 2], [u8; 2])> {
        let mut pitch_lag = [0u8; 2];
        let mut pitch_gain = [0u8; 2];

        // Two subframes of 40 samples each
        for subframe in 0..2 {
            let start = subframe * 40;
            let subframe_speech = &weighted_speech[start..start + 40];

            // Simplified pitch search in range 20-143 samples
            let mut best_lag = 20u8;
            let mut best_correlation = 0.0f32;

            for lag in 20..144 {
                let mut correlation = 0.0f32;
                for i in 0..40 {
                    if start + i >= lag {
                        correlation += subframe_speech[i] * weighted_speech[start + i - lag];
                    }
                }

                if correlation > best_correlation {
                    best_correlation = correlation;
                    best_lag = lag as u8;
                }
            }

            pitch_lag[subframe] = best_lag;

            // Compute gain (simplified)
            let mut energy = 0.0f32;
            for &sample in subframe_speech {
                energy += sample * sample;
            }

            let gain = if energy > 0.0 {
                (best_correlation / energy.sqrt()).clamp(0.0, 1.2)
            } else {
                0.0
            };

            pitch_gain[subframe] = (gain * 127.0) as u8;
        }

        Ok((pitch_lag, pitch_gain))
    }

    /// Fixed codebook search (simplified ACELP)
    fn fixed_codebook_search(
        &self,
        weighted_speech: &[f32],
        _pitch_lag: &[u8; 2],
    ) -> Result<([u16; 2], [u8; 2], [u8; 2])> {
        let mut fixed_index = [0u16; 2];
        let mut fixed_sign = [0u8; 2];
        let mut fixed_gain = [0u8; 2];

        // Simplified fixed codebook search for each subframe
        for subframe in 0..2 {
            let start = subframe * 40;
            let subframe_speech = &weighted_speech[start..start + 40];

            // Find 4 pulses with best correlation (simplified)
            let mut best_index = 0u16;
            let mut best_correlation = 0.0f32;

            // Search through possible pulse positions
            for index in 0..512 {
                let mut correlation = 0.0f32;

                // Decode pulse positions from index (simplified)
                for pulse in 0..4 {
                    let pos = (index >> (pulse * 3)) & 0x7;
                    let actual_pos = pos * 5 + pulse; // Simplified position mapping

                    if actual_pos < 40 {
                        correlation += subframe_speech[actual_pos].abs();
                    }
                }

                if correlation > best_correlation {
                    best_correlation = correlation;
                    best_index = index as u16;
                }
            }

            fixed_index[subframe] = best_index;
            fixed_sign[subframe] = 0x0F; // All positive signs (simplified)
            fixed_gain[subframe] = (best_correlation * 32.0).clamp(0.0, 255.0) as u8;
        }

        Ok((fixed_index, fixed_sign, fixed_gain))
    }

    /// Decode adaptive codebook contribution
    fn decode_adaptive_codebook(
        &mut self,
        pitch_lag: &[u8; 2],
        pitch_gain: &[u8; 2],
    ) -> Result<[f32; G729_FRAME_SIZE]> {
        let mut adaptive_exc = [0.0f32; G729_FRAME_SIZE];

        for subframe in 0..2 {
            let start = subframe * 40;
            let lag = pitch_lag[subframe] as usize;
            let gain = pitch_gain[subframe] as f32 / 127.0;

            for i in 0..40 {
                let exc_index = 154 - lag + i;
                if exc_index < 154 {
                    adaptive_exc[start + i] = gain * self.old_exc[exc_index];
                }
            }
        }

        // Update excitation memory
        for i in 0..(154 - G729_FRAME_SIZE) {
            self.old_exc[i] = self.old_exc[i + G729_FRAME_SIZE];
        }
        for i in 0..G729_FRAME_SIZE {
            self.old_exc[154 - G729_FRAME_SIZE + i] = adaptive_exc[i];
        }

        Ok(adaptive_exc)
    }

    /// Decode fixed codebook contribution
    fn decode_fixed_codebook(
        &self,
        fixed_index: &[u16; 2],
        fixed_sign: &[u8; 2],
        fixed_gain: &[u8; 2],
    ) -> Result<[f32; G729_FRAME_SIZE]> {
        let mut fixed_exc = [0.0f32; G729_FRAME_SIZE];

        for subframe in 0..2 {
            let start = subframe * 40;
            let index = fixed_index[subframe];
            let signs = fixed_sign[subframe];
            let gain = fixed_gain[subframe] as f32 / 32.0;

            // Decode 4 pulses from index
            for pulse in 0..4 {
                let pos = ((index >> (pulse * 3)) & 0x7) as usize;
                let actual_pos = pos * 5 + pulse;

                if actual_pos < 40 {
                    let sign = if (signs >> pulse) & 1 == 1 { 1.0 } else { -1.0 };
                    fixed_exc[start + actual_pos] = gain * sign;
                }
            }
        }

        Ok(fixed_exc)
    }

    /// LP synthesis filter
    fn lp_synthesis(
        &self,
        excitation: &[f32; G729_FRAME_SIZE],
        lp_coeffs: &[f32; 11],
    ) -> Result<[f32; G729_FRAME_SIZE]> {
        let mut speech = [0.0f32; G729_FRAME_SIZE];
        let memory = [0.0f32; 10];

        for i in 0..G729_FRAME_SIZE {
            speech[i] = excitation[i];

            for j in 1..11 {
                if i >= j {
                    speech[i] -= lp_coeffs[j] * speech[i - j];
                } else if j - i - 1 < 10 {
                    speech[i] -= lp_coeffs[j] * memory[10 - (j - i)];
                }
            }
        }

        Ok(speech)
    }

    /// Post-processing filters
    fn post_processing(&self, speech: &mut [f32]) {
        // De-emphasis filter: H(z) = 1 / (1 - μz^-1), μ = 0.68
        let mut prev = 0.0f32;
        for sample in speech.iter_mut() {
            *sample += 0.68 * prev;
            prev = *sample;
        }
    }

    /// Pack G.729 frame into 10-byte bitstream
    fn pack_frame(&self, frame: &G729Frame) -> Result<Vec<u8>> {
        let mut bits = Vec::with_capacity(80); // 80 bits total

        // LSP indices (18 bits)
        for i in 0..18 {
            bits.push(((frame.lsp_index >> (17 - i)) & 1) as u8);
        }

        // Subframe parameters (31 bits per subframe)
        for subframe in 0..2 {
            // Pitch lag (8 bits)
            for i in 0..8 {
                bits.push(((frame.pitch_lag[subframe] >> (7 - i)) & 1) as u8);
            }

            // Pitch gain (5 bits)
            for i in 0..5 {
                bits.push(((frame.pitch_gain[subframe] >> (4 - i)) & 1) as u8);
            }

            // Fixed codebook index (13 bits)
            for i in 0..13 {
                bits.push(((frame.fixed_index[subframe] >> (12 - i)) & 1) as u8);
            }

            // Fixed codebook signs (4 bits)
            for i in 0..4 {
                bits.push(((frame.fixed_sign[subframe] >> (3 - i)) & 1) as u8);
            }

            // Fixed codebook gain (1 bit, simplified)
            bits.push((frame.fixed_gain[subframe] & 1) as u8);
        }

        // Pack bits into bytes
        let mut bytes = Vec::with_capacity(10);
        for chunk in bits.chunks(8) {
            let mut byte = 0u8;
            for (i, &bit) in chunk.iter().enumerate() {
                byte |= bit << (7 - i);
            }
            bytes.push(byte);
        }

        Ok(bytes)
    }

    /// Unpack 10-byte bitstream into G.729 frame
    fn unpack_frame(&self, encoded: &[u8]) -> Result<G729Frame> {
        if encoded.len() != 10 {
            return Err(anyhow!("Invalid G.729 frame size"));
        }

        // Extract bits
        let mut bits = Vec::with_capacity(80);
        for &byte in encoded {
            for i in 0..8 {
                bits.push((byte >> (7 - i)) & 1);
            }
        }

        let mut bit_index = 0;

        // LSP index (18 bits)
        let mut lsp_index = 0u16;
        for i in 0..18 {
            lsp_index |= (bits[bit_index] as u16) << (17 - i);
            bit_index += 1;
        }

        let mut pitch_lag = [0u8; 2];
        let mut pitch_gain = [0u8; 2];
        let mut fixed_index = [0u16; 2];
        let mut fixed_sign = [0u8; 2];
        let mut fixed_gain = [0u8; 2];

        // Subframe parameters
        for subframe in 0..2 {
            // Pitch lag (8 bits)
            for i in 0..8 {
                pitch_lag[subframe] |= bits[bit_index] << (7 - i);
                bit_index += 1;
            }

            // Pitch gain (5 bits)
            for i in 0..5 {
                pitch_gain[subframe] |= bits[bit_index] << (4 - i);
                bit_index += 1;
            }

            // Fixed codebook index (13 bits)
            for i in 0..13 {
                fixed_index[subframe] |= (bits[bit_index] as u16) << (12 - i);
                bit_index += 1;
            }

            // Fixed codebook signs (4 bits)
            for i in 0..4 {
                fixed_sign[subframe] |= bits[bit_index] << (3 - i);
                bit_index += 1;
            }

            // Fixed codebook gain (1 bit)
            fixed_gain[subframe] = bits[bit_index];
            bit_index += 1;
        }

        Ok(G729Frame {
            lsp_index,
            pitch_lag,
            pitch_gain,
            fixed_index,
            fixed_sign,
            fixed_gain,
        })
    }

    /// Reset codec state
    pub fn reset(&mut self) {
        self.old_speech.fill(0.0);
        self.old_exc.fill(0.0);
        for i in 0..10 {
            self.lsp_old[i] = (i + 1) as f32 * std::f32::consts::PI / 11.0;
        }
        self.frame_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_g729_codec_creation() {
        let codec = G729Codec::new();
        assert_eq!(codec.frame_count, 0);
    }

    #[test]
    fn test_g729_encode_decode() {
        let mut codec = G729Codec::new();

        // Generate test signal (sine wave)
        let mut test_signal = Vec::with_capacity(G729_FRAME_SIZE);
        for i in 0..G729_FRAME_SIZE {
            let sample =
                (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / G729_SAMPLE_RATE as f32).sin();
            test_signal.push((sample * 16384.0) as i16);
        }

        // Encode
        let encoded = codec.encode(&test_signal).unwrap();
        assert_eq!(encoded.len(), G729_ENCODED_SIZE);

        // Decode
        let decoded = codec.decode(&encoded).unwrap();
        assert_eq!(decoded.len(), G729_FRAME_SIZE);

        // Check that decoded signal has reasonable amplitude
        let max_amplitude = decoded.iter().map(|&x| x.abs()).max().unwrap_or(0);
        assert!(max_amplitude > 100); // Should have some signal content
    }

    #[test]
    fn test_g729_frame_packing() {
        let codec = G729Codec::new();

        let frame = G729Frame {
            lsp_index: 12345,
            pitch_lag: [60, 65],
            pitch_gain: [15, 18],
            fixed_index: [123, 456],
            fixed_sign: [0x0F, 0x05],
            fixed_gain: [1, 0],
        };

        let packed = codec.pack_frame(&frame).unwrap();
        assert_eq!(packed.len(), 10);

        let unpacked = codec.unpack_frame(&packed).unwrap();
        assert_eq!(unpacked.lsp_index, frame.lsp_index);
        assert_eq!(unpacked.pitch_lag, frame.pitch_lag);
        assert_eq!(unpacked.pitch_gain, frame.pitch_gain);
    }

    #[test]
    fn test_g729_multiple_frames() {
        let mut encoder = G729Codec::new();
        let mut decoder = G729Codec::new();

        // Encode and decode multiple frames
        for frame_num in 0..10 {
            let mut test_signal = Vec::with_capacity(G729_FRAME_SIZE);
            for i in 0..G729_FRAME_SIZE {
                let freq = 800.0 + frame_num as f32 * 100.0; // Varying frequency
                let sample =
                    (2.0 * std::f32::consts::PI * freq * i as f32 / G729_SAMPLE_RATE as f32).sin();
                test_signal.push((sample * 12000.0) as i16);
            }

            let encoded = encoder.encode(&test_signal).unwrap();
            let decoded = decoder.decode(&encoded).unwrap();

            assert_eq!(encoded.len(), G729_ENCODED_SIZE);
            assert_eq!(decoded.len(), G729_FRAME_SIZE);
        }
    }
}
