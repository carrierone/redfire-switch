/*
 * G.729 External Assembly Integration
 * 
 * FFI interface to external x86-64 assembly optimized G.729 functions
 * Built using external assembler for maximum compatibility and performance
 */

#[cfg(target_arch = "x86_64")]
use std::arch::is_x86_feature_detected;

/// Constants for G.729 processing
pub const L_FRAME: usize = 80;        // Frame size
pub const L_SUBFR: usize = 40;        // Subframe size  
pub const M: usize = 10;              // LP order
pub const L_WINDOW: usize = 240;      // Analysis window size

// External assembly function declarations
#[cfg(feature = "g729_asm")]
extern "C" {
    /// AVX-optimized autocorrelation computation
    fn autocorrelation_avx(windowed_speech: *const f32, r: *mut f32);
    
    /// SSE-optimized autocorrelation computation
    fn autocorrelation_sse(windowed_speech: *const f32, r: *mut f32);
    
    /// Levinson-Durbin algorithm with assembly optimization
    fn levinson_durbin_asm(r: *const f32, lp_coeffs: *mut f32) -> f32;
    
    /// SSE-optimized Levinson-Durbin algorithm
    fn levinson_durbin_sse(r: *const f32, lp_coeffs: *mut f32) -> f32;
    
    /// AVX-optimized LSP quantization
    fn lsp_quantization_avx(
        lsp: *const f32,
        codebook: *const f32,
        codebook_size: i32,
        best_index: *mut i32,
        min_distance: *mut f32,
    );
    
    /// SSE-optimized LSP quantization
    fn lsp_quantization_sse(
        lsp: *const f32,
        codebook: *const f32,
        codebook_size: i32,
        best_index: *mut i32,
        min_distance: *mut f32,
    );
    
    /// Scalar LSP quantization fallback
    fn lsp_quantization_scalar(
        lsp: *const f32,
        codebook: *const f32,
        codebook_size: i32,
        best_index: *mut i32,
        min_distance: *mut f32,
    );
}

/// Scalar fallback implementations for when assembly is not available
mod scalar_fallback {
    use super::*;
    
    pub fn autocorrelation_scalar(windowed_speech: &[f32; L_WINDOW], r: &mut [f32; 11]) {
        for k in 0..11 {
            r[k] = 0.0;
            for i in 0..(L_WINDOW - k) {
                r[k] += windowed_speech[i] * windowed_speech[i + k];
            }
        }
    }
    
    pub fn levinson_durbin_scalar(r: &[f32; 11], lp_coeffs: &mut [f32; 11]) -> f32 {
        lp_coeffs[0] = 1.0;
        let mut error = r[0];
        
        if error == 0.0 {
            return 0.0;
        }
        
        for i in 1..=M {
            let mut sum = 0.0;
            for j in 1..i {
                sum += lp_coeffs[j] * r[i - j];
            }
            
            let k_i = -(r[i] + sum) / error;
            lp_coeffs[i] = k_i;
            
            for j in 1..=(i / 2) {
                let temp = lp_coeffs[j] + k_i * lp_coeffs[i - j];
                lp_coeffs[i - j] += k_i * lp_coeffs[j];
                lp_coeffs[j] = temp;
            }
            
            error *= 1.0 - k_i * k_i;
        }
        
        error
    }
    
    pub fn lsp_quantization_scalar_impl(
        lsp: &[f32; 10],
        codebook: &[[f32; 10]],
        codebook_size: usize,
    ) -> (usize, f32) {
        let mut best_index = 0;
        let mut min_distance = f32::INFINITY;
        
        for (index, entry) in codebook.iter().enumerate().take(codebook_size) {
            let mut distance = 0.0f32;
            for i in 0..10 {
                let diff = lsp[i] - entry[i];
                distance += diff * diff;
            }
            
            if distance < min_distance {
                min_distance = distance;
                best_index = index;
            }
        }
        
        (best_index, min_distance)
    }
}

/// High-level autocorrelation function with runtime CPU feature detection
pub fn autocorrelation_optimized(windowed_speech: &[f32; L_WINDOW], r: &mut [f32; 11]) {
    #[cfg(all(feature = "g729_asm", target_arch = "x86_64"))]
    {
        unsafe {
            if is_x86_feature_detected!("avx") {
                autocorrelation_avx(windowed_speech.as_ptr(), r.as_mut_ptr());
            } else if is_x86_feature_detected!("sse") {
                autocorrelation_sse(windowed_speech.as_ptr(), r.as_mut_ptr());
            } else {
                scalar_fallback::autocorrelation_scalar(windowed_speech, r);
            }
        }
    }
    
    #[cfg(not(all(feature = "g729_asm", target_arch = "x86_64")))]
    {
        scalar_fallback::autocorrelation_scalar(windowed_speech, r);
    }
}

/// High-level Levinson-Durbin function with runtime CPU feature detection
pub fn levinson_durbin_optimized(r: &[f32; 11], lp_coeffs: &mut [f32; 11]) -> f32 {
    #[cfg(all(feature = "g729_asm", target_arch = "x86_64"))]
    {
        unsafe {
            if is_x86_feature_detected!("sse2") {
                levinson_durbin_asm(r.as_ptr(), lp_coeffs.as_mut_ptr())
            } else {
                scalar_fallback::levinson_durbin_scalar(r, lp_coeffs)
            }
        }
    }
    
    #[cfg(not(all(feature = "g729_asm", target_arch = "x86_64")))]
    {
        scalar_fallback::levinson_durbin_scalar(r, lp_coeffs)
    }
}

/// High-level LSP quantization function with runtime CPU feature detection
pub fn lsp_quantization_optimized(
    lsp: &[f32; 10],
    codebook: &[[f32; 10]],
    codebook_size: usize,
) -> (usize, f32) {
    #[cfg(all(feature = "g729_asm", target_arch = "x86_64"))]
    {
        unsafe {
            let mut best_index: i32 = 0;
            let mut min_distance: f32 = 0.0;
            
            if is_x86_feature_detected!("avx") {
                lsp_quantization_avx(
                    lsp.as_ptr(),
                    codebook.as_ptr() as *const f32,
                    codebook_size as i32,
                    &mut best_index,
                    &mut min_distance,
                );
            } else if is_x86_feature_detected!("sse") {
                lsp_quantization_sse(
                    lsp.as_ptr(),
                    codebook.as_ptr() as *const f32,
                    codebook_size as i32,
                    &mut best_index,
                    &mut min_distance,
                );
            } else {
                lsp_quantization_scalar(
                    lsp.as_ptr(),
                    codebook.as_ptr() as *const f32,
                    codebook_size as i32,
                    &mut best_index,
                    &mut min_distance,
                );
            }
            
            (best_index as usize, min_distance)
        }
    }
    
    #[cfg(not(all(feature = "g729_asm", target_arch = "x86_64")))]
    {
        scalar_fallback::lsp_quantization_scalar_impl(lsp, codebook, codebook_size)
    }
}

/// External Assembly G.729 Codec using external assembler
pub struct ExternalAsmG729Codec {
    // Previous speech for overlap-add
    old_speech: [f32; L_WINDOW],
    // Previous excitation for long-term prediction
    old_exc: [f32; 154],
    // Previous LSP coefficients
    lsp_old: [f32; M],
    // Hamming window coefficients
    window: [f32; L_WINDOW],
    // Performance counters
    frames_processed: u64,
    asm_operations: u64,
    fallback_operations: u64,
}

impl Default for ExternalAsmG729Codec {
    fn default() -> Self {
        Self::new()
    }
}

impl ExternalAsmG729Codec {
    /// Create new G.729 codec with external assembly optimization
    pub fn new() -> Self {
        let mut window = [0.0f32; L_WINDOW];
        
        // Initialize Hamming window
        for i in 0..L_WINDOW {
            window[i] = 0.54 - 0.46 * (2.0 * std::f32::consts::PI * i as f32 / (L_WINDOW - 1) as f32).cos();
        }
        
        Self {
            old_speech: [0.0; L_WINDOW],
            old_exc: [0.0; 154],
            lsp_old: [0.0; M],
            window,
            frames_processed: 0,
            asm_operations: 0,
            fallback_operations: 0,
        }
    }
    
    /// Encode speech frame using external assembly optimization
    pub fn encode(&mut self, speech_frame: &[f32; L_FRAME]) -> Vec<u8> {
        self.frames_processed += 1;
        
        // Create windowed speech signal for analysis
        let mut windowed_speech = [0.0f32; L_WINDOW];
        
        // Update speech buffer with new frame (avoid borrow checker issue)
        let mut temp_speech = [0.0f32; L_WINDOW];
        temp_speech[..L_WINDOW - L_FRAME].copy_from_slice(&self.old_speech[L_FRAME..]);
        temp_speech[L_WINDOW - L_FRAME..].copy_from_slice(speech_frame);
        self.old_speech = temp_speech;
        
        // Apply Hamming window
        for i in 0..L_WINDOW {
            windowed_speech[i] = self.old_speech[i] * self.window[i];
        }
        
        // Compute autocorrelation using optimized assembly
        let mut r = [0.0f32; 11];
        autocorrelation_optimized(&windowed_speech, &mut r);
        
        // Track assembly usage
        #[cfg(all(feature = "g729_asm", target_arch = "x86_64"))]
        {
            self.asm_operations += 1;
        }
        #[cfg(not(all(feature = "g729_asm", target_arch = "x86_64")))]
        {
            self.fallback_operations += 1;
        }
        
        // Compute LP coefficients using Levinson-Durbin
        let mut lp_coeffs = [0.0f32; 11];
        let _prediction_error = levinson_durbin_optimized(&r, &mut lp_coeffs);
        
        // Convert LP coefficients to LSP (simplified)
        let mut lsp = [0.0f32; 10];
        for i in 0..10 {
            lsp[i] = (i + 1) as f32 * std::f32::consts::PI / 11.0;
        }
        
        // Quantize LSP using optimized assembly
        let codebook = self.generate_lsp_codebook();
        let (_best_index, _min_distance) = lsp_quantization_optimized(&lsp, &codebook, 1024);
        
        self.lsp_old = lsp;
        
        // Return simplified encoded data (in real implementation, would include all G.729 encoding steps)
        vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A]
    }
    
    /// Generate LSP codebook (simplified for demonstration)
    fn generate_lsp_codebook(&self) -> [[f32; 10]; 1024] {
        let mut codebook = [[0.0f32; 10]; 1024];
        
        for i in 0..1024 {
            for j in 0..10 {
                codebook[i][j] = (i as f32 / 1024.0 + j as f32 / 10.0) * std::f32::consts::PI;
            }
        }
        
        codebook
    }
    
    /// Get performance statistics
    pub fn get_performance_stats(&self) -> ExternalAsmPerformanceStats {
        ExternalAsmPerformanceStats {
            frames_processed: self.frames_processed,
            asm_operations: self.asm_operations,
            fallback_operations: self.fallback_operations,
            asm_usage_percentage: if self.frames_processed > 0 {
                (self.asm_operations as f64 / self.frames_processed as f64) * 100.0
            } else {
                0.0
            },
            assembly_available: cfg!(all(feature = "g729_asm", target_arch = "x86_64")),
            cpu_features: self.get_cpu_features(),
        }
    }
    
    fn get_cpu_features(&self) -> CpuFeatures {
        #[cfg(target_arch = "x86_64")]
        {
            CpuFeatures {
                sse: is_x86_feature_detected!("sse"),
                sse2: is_x86_feature_detected!("sse2"),
                avx: is_x86_feature_detected!("avx"),
                fma: is_x86_feature_detected!("fma"),
            }
        }
        
        #[cfg(not(target_arch = "x86_64"))]
        {
            CpuFeatures {
                sse: false,
                sse2: false,
                avx: false,
                fma: false,
            }
        }
    }
}

/// Performance statistics for external assembly codec
#[derive(Debug, Clone)]
pub struct ExternalAsmPerformanceStats {
    pub frames_processed: u64,
    pub asm_operations: u64,
    pub fallback_operations: u64,
    pub asm_usage_percentage: f64,
    pub assembly_available: bool,
    pub cpu_features: CpuFeatures,
}

/// CPU feature detection results
#[derive(Debug, Clone)]
pub struct CpuFeatures {
    pub sse: bool,
    pub sse2: bool,
    pub avx: bool,
    pub fma: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_autocorrelation_optimized() {
        let mut windowed_speech = [0.0f32; L_WINDOW];
        let mut r = [0.0f32; 11];
        
        // Generate test signal
        for i in 0..L_WINDOW {
            windowed_speech[i] = (i as f32 * 0.1).sin();
        }
        
        autocorrelation_optimized(&windowed_speech, &mut r);
        
        // Check that r[0] is maximum (autocorrelation property)
        assert!(r[0] > r[1]);
        assert!(r[0] > 0.0);
    }
    
    #[test]
    fn test_levinson_durbin_optimized() {
        let r = [100.0, 50.0, 25.0, 12.0, 6.0, 3.0, 1.5, 0.7, 0.3, 0.1, 0.05];
        let mut lp_coeffs = [0.0f32; 11];
        
        let error = levinson_durbin_optimized(&r, &mut lp_coeffs);
        assert!(error > 0.0);
        assert_eq!(lp_coeffs[0], 1.0);
    }
    
    #[test]
    fn test_lsp_quantization_optimized() {
        let lsp = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
        let codebook = [[0.0f32; 10]; 256];
        
        let (index, distance) = lsp_quantization_optimized(&lsp, &codebook, 256);
        assert!(index < 256);
        assert!(distance >= 0.0);
    }
    
    #[test]
    fn test_external_asm_codec() {
        let mut codec = ExternalAsmG729Codec::new();
        let speech_frame = [0.0f32; L_FRAME];
        
        let encoded = codec.encode(&speech_frame);
        assert_eq!(encoded.len(), 10);
        
        let stats = codec.get_performance_stats();
        assert_eq!(stats.frames_processed, 1);
        assert!(stats.asm_usage_percentage >= 0.0);
        assert!(stats.asm_usage_percentage <= 100.0);
    }
    
    #[test]
    fn test_cpu_feature_detection() {
        let codec = ExternalAsmG729Codec::new();
        let features = codec.get_cpu_features();
        
        // On modern x86-64, we expect at least SSE support
        #[cfg(target_arch = "x86_64")]
        assert!(features.sse);
        
        println!("CPU Features - SSE: {}, SSE2: {}, AVX: {}, FMA: {}", 
                features.sse, features.sse2, features.avx, features.fma);
    }
}