/*
 * Standalone G.729 External Assembly Test
 * 
 * Self-contained test for external assembly G.729 codec functionality
 * This file has minimal dependencies to avoid compilation issues
 */

#[cfg(target_arch = "x86_64")]
use std::arch::is_x86_feature_detected;
use std::time::Instant;

/// G.729 Constants
pub const L_FRAME: usize = 80;        // Frame size
pub const L_WINDOW: usize = 240;      // Analysis window size

// External assembly function declarations
#[cfg(all(feature = "g729_asm", target_arch = "x86_64"))]
extern "C" {
    fn autocorrelation_avx(windowed_speech: *const f32, r: *mut f32);
    fn autocorrelation_sse(windowed_speech: *const f32, r: *mut f32);
    fn levinson_durbin_asm(r: *const f32, lp_coeffs: *mut f32) -> f32;
}

/// Scalar autocorrelation fallback
pub fn autocorrelation_scalar(windowed_speech: &[f32; L_WINDOW], r: &mut [f32; 11]) {
    for k in 0..11 {
        r[k] = 0.0;
        for i in 0..(L_WINDOW - k) {
            r[k] += windowed_speech[i] * windowed_speech[i + k];
        }
    }
}

/// Scalar Levinson-Durbin fallback
pub fn levinson_durbin_scalar(r: &[f32; 11], lp_coeffs: &mut [f32; 11]) -> f32 {
    lp_coeffs[0] = 1.0;
    let mut error = r[0];
    
    if error == 0.0 {
        return 0.0;
    }
    
    for i in 1..=10 {
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

/// High-level autocorrelation with runtime feature detection
pub fn autocorrelation_optimized(windowed_speech: &[f32; L_WINDOW], r: &mut [f32; 11]) {
    #[cfg(all(feature = "g729_asm", target_arch = "x86_64"))]
    {
        unsafe {
            if is_x86_feature_detected!("avx") {
                autocorrelation_avx(windowed_speech.as_ptr(), r.as_mut_ptr());
                return;
            } else if is_x86_feature_detected!("sse") {
                autocorrelation_sse(windowed_speech.as_ptr(), r.as_mut_ptr());
                return;
            }
        }
    }
    
    // Fallback to scalar implementation
    autocorrelation_scalar(windowed_speech, r);
}

/// High-level Levinson-Durbin with runtime feature detection
pub fn levinson_durbin_optimized(r: &[f32; 11], lp_coeffs: &mut [f32; 11]) -> f32 {
    #[cfg(all(feature = "g729_asm", target_arch = "x86_64"))]
    {
        unsafe {
            if is_x86_feature_detected!("sse2") {
                return levinson_durbin_asm(r.as_ptr(), lp_coeffs.as_mut_ptr());
            }
        }
    }
    
    // Fallback to scalar implementation
    levinson_durbin_scalar(r, lp_coeffs)
}

/// Simple G.729 codec state
#[derive(Debug)]
pub struct SimpleG729Codec {
    old_speech: [f32; L_WINDOW],
    window: [f32; L_WINDOW],
    frames_processed: u64,
    total_encode_time_ns: u64,
}

impl SimpleG729Codec {
    pub fn new() -> Self {
        let mut window = [0.0f32; L_WINDOW];
        
        // Initialize Hamming window
        for i in 0..L_WINDOW {
            window[i] = 0.54 - 0.46 * (2.0 * std::f32::consts::PI * i as f32 / (L_WINDOW - 1) as f32).cos();
        }
        
        Self {
            old_speech: [0.0; L_WINDOW],
            window,
            frames_processed: 0,
            total_encode_time_ns: 0,
        }
    }
    
    pub fn encode(&mut self, speech_frame: &[f32; L_FRAME]) -> Vec<u8> {
        let start = Instant::now();
        
        // Update speech buffer
        let mut temp_speech = [0.0f32; L_WINDOW];
        temp_speech[..L_WINDOW - L_FRAME].copy_from_slice(&self.old_speech[L_FRAME..]);
        temp_speech[L_WINDOW - L_FRAME..].copy_from_slice(speech_frame);
        self.old_speech = temp_speech;
        
        // Apply window
        let mut windowed_speech = [0.0f32; L_WINDOW];
        for i in 0..L_WINDOW {
            windowed_speech[i] = self.old_speech[i] * self.window[i];
        }
        
        // Compute autocorrelation using optimized function
        let mut r = [0.0f32; 11];
        autocorrelation_optimized(&windowed_speech, &mut r);
        
        // Compute LP coefficients
        let mut lp_coeffs = [0.0f32; 11];
        let _prediction_error = levinson_durbin_optimized(&r, &mut lp_coeffs);
        
        // Update statistics
        self.frames_processed += 1;
        self.total_encode_time_ns += start.elapsed().as_nanos() as u64;
        
        // Return simplified encoded data (10 bytes for G.729)
        vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A]
    }
    
    pub fn get_statistics(&self) -> G729Statistics {
        let avg_encode_time_ns = if self.frames_processed > 0 {
            self.total_encode_time_ns / self.frames_processed
        } else {
            0
        };
        
        G729Statistics {
            frames_processed: self.frames_processed,
            avg_encode_time_ms: avg_encode_time_ns as f64 / 1_000_000.0,
            total_time_ms: self.total_encode_time_ns as f64 / 1_000_000.0,
            real_time_capable: avg_encode_time_ns <= 10_000_000, // 10ms in nanoseconds
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

#[derive(Debug, Clone)]
pub struct G729Statistics {
    pub frames_processed: u64,
    pub avg_encode_time_ms: f64,
    pub total_time_ms: f64,
    pub real_time_capable: bool,
    pub assembly_available: bool,
    pub cpu_features: CpuFeatures,
}

#[derive(Debug, Clone)]
pub struct CpuFeatures {
    pub sse: bool,
    pub sse2: bool,
    pub avx: bool,
    pub fma: bool,
}

/// Run G.729 performance test
pub fn run_g729_standalone_test() {
    println!("=== G.729 External Assembly Standalone Test ===\n");
    
    // Test CPU features
    test_cpu_features();
    
    // Test individual DSP functions
    test_dsp_functions();
    
    // Test codec performance
    test_codec_performance();
    
    println!("\n=== Test Complete ===");
}

fn test_cpu_features() {
    println!("CPU Feature Detection:");
    
    #[cfg(target_arch = "x86_64")]
    {
        println!("  Architecture: x86_64");
        println!("  SSE:  {}", is_x86_feature_detected!("sse"));
        println!("  SSE2: {}", is_x86_feature_detected!("sse2"));
        println!("  AVX:  {}", is_x86_feature_detected!("avx"));
        println!("  FMA:  {}", is_x86_feature_detected!("fma"));
    }
    
    #[cfg(not(target_arch = "x86_64"))]
    {
        println!("  Architecture: Non-x86_64 (using scalar fallbacks)");
    }
    
    println!("  External assembly available: {}", cfg!(all(feature = "g729_asm", target_arch = "x86_64")));
    println!();
}

fn test_dsp_functions() {
    println!("DSP Function Testing:");
    
    // Test autocorrelation
    println!("  Testing autocorrelation...");
    let mut windowed_speech = [0.0f32; L_WINDOW];
    for i in 0..L_WINDOW {
        let t = i as f32 / 8000.0;
        windowed_speech[i] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 
                           (0.54 - 0.46 * (2.0 * std::f32::consts::PI * i as f32 / (L_WINDOW - 1) as f32).cos());
    }
    
    let start = Instant::now();
    let mut r = [0.0f32; 11];
    autocorrelation_optimized(&windowed_speech, &mut r);
    let autocorr_time = start.elapsed();
    
    println!("    Time: {:?}", autocorr_time);
    println!("    r[0] = {:.3}, r[1] = {:.3}", r[0], r[1]);
    println!("    ✓ r[0] > r[1]: {}", r[0] > r[1]);
    
    // Test Levinson-Durbin
    println!("  Testing Levinson-Durbin...");
    let start = Instant::now();
    let mut lp_coeffs = [0.0f32; 11];
    let prediction_error = levinson_durbin_optimized(&r, &mut lp_coeffs);
    let levinson_time = start.elapsed();
    
    println!("    Time: {:?}", levinson_time);
    println!("    Prediction error: {:.6}", prediction_error);
    println!("    ✓ lp_coeffs[0] = 1.0: {}", (lp_coeffs[0] - 1.0).abs() < 0.001);
    println!();
}

fn test_codec_performance() {
    println!("Codec Performance Test:");
    
    let mut codec = SimpleG729Codec::new();
    let test_frames = 1000;
    
    println!("  Processing {} frames...", test_frames);
    
    for frame_num in 0..test_frames {
        // Generate test speech frame
        let mut speech_frame = [0.0f32; L_FRAME];
        for i in 0..L_FRAME {
            let t = (frame_num * L_FRAME + i) as f32 / 8000.0;
            speech_frame[i] = 0.6 * (2.0 * std::f32::consts::PI * 440.0 * t).sin() +
                             0.3 * (2.0 * std::f32::consts::PI * 880.0 * t).sin() +
                             0.1 * (2.0 * std::f32::consts::PI * 1320.0 * t).sin();
        }
        
        let _encoded = codec.encode(&speech_frame);
        
        if frame_num % 200 == 0 {
            print!(".");
            std::io::Write::flush(&mut std::io::stdout()).unwrap();
        }
    }
    println!();
    
    let stats = codec.get_statistics();
    
    println!("  Results:");
    println!("    Frames processed: {}", stats.frames_processed);
    println!("    Average encode time: {:.3} ms", stats.avg_encode_time_ms);
    println!("    Total processing time: {:.1} ms", stats.total_time_ms);
    println!("    Throughput: {:.0} frames/sec", 1000.0 / stats.avg_encode_time_ms);
    println!("    Real-time capable: {} ({})", 
             if stats.real_time_capable { "✓" } else { "✗" },
             if stats.real_time_capable { "Yes" } else { "No" });
    println!("    Assembly optimization: {}", 
             if stats.assembly_available { "ENABLED" } else { "DISABLED (fallback mode)" });
    
    // Real-time assessment
    let real_time_margin = 10.0 / stats.avg_encode_time_ms;
    println!("    Real-time margin: {:.1}x", real_time_margin);
    
    if real_time_margin >= 10.0 {
        println!("    Performance: Excellent (>10x real-time)");
    } else if real_time_margin >= 5.0 {
        println!("    Performance: Very Good (>5x real-time)");
    } else if real_time_margin >= 2.0 {
        println!("    Performance: Good (>2x real-time)");
    } else if real_time_margin >= 1.0 {
        println!("    Performance: Adequate (real-time capable)");
    } else {
        println!("    Performance: Insufficient (not real-time)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_autocorrelation_scalar() {
        let mut windowed_speech = [0.0f32; L_WINDOW];
        for i in 0..L_WINDOW {
            windowed_speech[i] = (i as f32 * 0.1).sin();
        }
        
        let mut r = [0.0f32; 11];
        autocorrelation_scalar(&windowed_speech, &mut r);
        
        assert!(r[0] > r[1]);
        assert!(r[0] > 0.0);
    }
    
    #[test]
    fn test_levinson_durbin_scalar() {
        let r = [100.0, 50.0, 25.0, 12.0, 6.0, 3.0, 1.5, 0.7, 0.3, 0.1, 0.05];
        let mut lp_coeffs = [0.0f32; 11];
        
        let error = levinson_durbin_scalar(&r, &mut lp_coeffs);
        assert!(error > 0.0);
        assert!((lp_coeffs[0] - 1.0).abs() < 0.001);
    }
    
    #[test]
    fn test_codec_basic() {
        let mut codec = SimpleG729Codec::new();
        let speech_frame = [0.1f32; L_FRAME];
        
        let encoded = codec.encode(&speech_frame);
        assert_eq!(encoded.len(), 10);
        
        let stats = codec.get_statistics();
        assert_eq!(stats.frames_processed, 1);
    }
}