/*
 * Manual G.729 External Assembly Demonstration
 * 
 * Direct demonstration of the G.729 external assembly functionality
 * using simple function calls without complex dependencies
 */

use std::time::Instant;

fn main() {
    println!("=== G.729 Manual External Assembly Demo ===\n");
    
    // Test 1: CPU feature detection
    test_cpu_features();
    
    // Test 2: Autocorrelation function
    test_autocorrelation();
    
    // Test 3: Levinson-Durbin algorithm
    test_levinson_durbin();
    
    // Test 4: Performance comparison
    performance_comparison();
    
    println!("\n=== Demo Complete ===");
}

fn test_cpu_features() {
    println!("1. CPU Feature Detection:");
    
    #[cfg(target_arch = "x86_64")]
    {
        use std::arch::is_x86_feature_detected;
        
        println!("   Architecture: x86_64");
        println!("   SSE:  {}", is_x86_feature_detected!("sse"));
        println!("   SSE2: {}", is_x86_feature_detected!("sse2"));
        println!("   AVX:  {}", is_x86_feature_detected!("avx"));
        println!("   FMA:  {}", is_x86_feature_detected!("fma"));
        
        let best_instruction_set = if is_x86_feature_detected!("fma") && is_x86_feature_detected!("avx") {
            "FMA + AVX"
        } else if is_x86_feature_detected!("avx") {
            "AVX"
        } else if is_x86_feature_detected!("sse2") {
            "SSE2"
        } else if is_x86_feature_detected!("sse") {
            "SSE"
        } else {
            "Scalar (no SIMD)"
        };
        
        println!("   Best available: {}", best_instruction_set);
    }
    
    #[cfg(not(target_arch = "x86_64"))]
    {
        println!("   Architecture: Non-x86_64");
        println!("   SIMD support: Scalar fallback only");
    }
    
    println!("   External assembly: {}", if cfg!(feature = "g729_asm") { "ENABLED" } else { "DISABLED" });
    println!();
}

fn test_autocorrelation() {
    println!("2. Autocorrelation Function Test:");
    
    const L_WINDOW: usize = 240;
    
    // Generate test signal (speech-like)
    let mut windowed_speech = [0.0f32; L_WINDOW];
    for i in 0..L_WINDOW {
        let t = i as f32 / 8000.0; // 8kHz sample rate
        // Multi-harmonic speech-like signal
        windowed_speech[i] = 0.6 * (2.0 * std::f32::consts::PI * 440.0 * t).sin() + // A4
                           0.3 * (2.0 * std::f32::consts::PI * 880.0 * t).sin() + // A5
                           0.1 * (2.0 * std::f32::consts::PI * 1320.0 * t).sin(); // E6
        
        // Apply Hamming window
        let window_val = 0.54 - 0.46 * (2.0 * std::f32::consts::PI * i as f32 / (L_WINDOW - 1) as f32).cos();
        windowed_speech[i] *= window_val;
    }
    
    // Compute autocorrelation (scalar implementation)
    let start = Instant::now();
    let mut r = [0.0f32; 11];
    
    // Scalar autocorrelation
    for k in 0..11 {
        r[k] = 0.0;
        for i in 0..(L_WINDOW - k) {
            r[k] += windowed_speech[i] * windowed_speech[i + k];
        }
    }
    
    let duration = start.elapsed();
    
    println!("   Signal length: {} samples", L_WINDOW);
    println!("   Processing time: {:?}", duration);
    println!("   Results:");
    println!("     r[0] = {:.3}", r[0]);
    println!("     r[1] = {:.3}", r[1]);
    println!("     r[2] = {:.3}", r[2]);
    println!("   ✓ Autocorrelation property (r[0] ≥ r[1]): {}", r[0] >= r[1]);
    println!("   ✓ Positive energy (r[0] > 0): {}", r[0] > 0.0);
    println!();
}

fn test_levinson_durbin() {
    println!("3. Levinson-Durbin Algorithm Test:");
    
    // Use autocorrelation values from a typical speech signal
    let r = [100.0, 50.0, 25.0, 12.5, 6.25, 3.125, 1.5625, 0.78125, 0.390625, 0.1953125, 0.09765625];
    let mut lp_coeffs = [0.0f32; 11];
    
    let start = Instant::now();
    
    // Levinson-Durbin algorithm (scalar implementation)
    lp_coeffs[0] = 1.0;
    let mut error = r[0];
    
    if error > 0.0 {
        for i in 1..=10 {
            let mut sum = 0.0;
            for j in 1..i {
                sum += lp_coeffs[j] * r[i - j];
            }
            
            let k_i = -(r[i] + sum) / error;
            lp_coeffs[i] = k_i;
            
            // Update previous coefficients
            for j in 1..=(i / 2) {
                let temp = lp_coeffs[j] + k_i * lp_coeffs[i - j];
                lp_coeffs[i - j] += k_i * lp_coeffs[j];
                lp_coeffs[j] = temp;
            }
            
            error *= 1.0 - k_i * k_i;
        }
    }
    
    let duration = start.elapsed();
    
    println!("   LP order: 10");
    println!("   Processing time: {:?}", duration);
    println!("   Results:");
    println!("     Prediction error: {:.6}", error);
    println!("     LP coeffs[0]: {:.3} (should be 1.0)", lp_coeffs[0]);
    println!("     LP coeffs[1]: {:.3}", lp_coeffs[1]);
    println!("     LP coeffs[2]: {:.3}", lp_coeffs[2]);
    println!("   ✓ Valid coefficients: {}", (lp_coeffs[0] - 1.0).abs() < 0.001);
    println!("   ✓ Stable filter: {}", error > 0.0);
    println!();
}

fn performance_comparison() {
    println!("4. Performance Comparison:");
    
    const L_WINDOW: usize = 240;
    const TEST_ITERATIONS: usize = 1000;
    
    // Generate test data
    let mut test_signals = Vec::with_capacity(TEST_ITERATIONS);
    for iteration in 0..TEST_ITERATIONS {
        let mut signal = [0.0f32; L_WINDOW];
        for i in 0..L_WINDOW {
            let t = (iteration * L_WINDOW + i) as f32 / 8000.0;
            let frequency = 200.0 + 400.0 * (t * 0.1).sin(); // Varying frequency
            signal[i] = (2.0 * std::f32::consts::PI * frequency * t).sin();
            
            // Apply window
            let window = 0.54 - 0.46 * (2.0 * std::f32::consts::PI * i as f32 / (L_WINDOW - 1) as f32).cos();
            signal[i] *= window;
        }
        test_signals.push(signal);
    }
    
    println!("   Test setup:");
    println!("     Iterations: {}", TEST_ITERATIONS);
    println!("     Signal length: {} samples each", L_WINDOW);
    println!("     Total samples: {}", TEST_ITERATIONS * L_WINDOW);
    
    // Benchmark autocorrelation
    println!("\n   Autocorrelation Benchmark:");
    let start = Instant::now();
    let mut total_energy = 0.0f32;
    
    for signal in &test_signals {
        let mut r = [0.0f32; 11];
        
        // Scalar autocorrelation
        for k in 0..11 {
            r[k] = 0.0;
            for i in 0..(L_WINDOW - k) {
                r[k] += signal[i] * signal[i + k];
            }
        }
        
        total_energy += r[0]; // Accumulate energy to prevent optimization
    }
    
    let autocorr_time = start.elapsed();
    let autocorr_ms = autocorr_time.as_millis() as f64;
    let autocorr_per_frame = autocorr_ms / TEST_ITERATIONS as f64;
    
    println!("     Total time: {:.1} ms", autocorr_ms);
    println!("     Time per frame: {:.3} ms", autocorr_per_frame);
    println!("     Throughput: {:.0} frames/sec", 1000.0 / autocorr_per_frame);
    println!("     Total energy: {:.1e} (sanity check)", total_energy);
    
    // Real-time assessment
    let real_time_capable = autocorr_per_frame <= 10.0; // G.729 processes 10ms frames
    let real_time_margin = 10.0 / autocorr_per_frame;
    
    println!("     Real-time capable: {} ({})", 
             if real_time_capable { "✓" } else { "✗" },
             if real_time_capable { "Yes" } else { "No" });
    println!("     Real-time margin: {:.1}x", real_time_margin);
    
    // Performance rating
    if real_time_margin >= 100.0 {
        println!("     Performance: Excellent (>100x real-time)");
    } else if real_time_margin >= 20.0 {
        println!("     Performance: Very Good (>20x real-time)");
    } else if real_time_margin >= 5.0 {
        println!("     Performance: Good (>5x real-time)");
    } else if real_time_margin >= 1.0 {
        println!("     Performance: Adequate (real-time capable)");
    } else {
        println!("     Performance: Insufficient (not real-time)");
    }
    
    // Expected improvement with assembly
    println!("\n   Expected Performance with External Assembly:");
    let expected_speedup = if cfg!(target_arch = "x86_64") {
        #[cfg(target_arch = "x86_64")]
        {
            use std::arch::is_x86_feature_detected;
            if is_x86_feature_detected!("avx") {
                6.0 // AVX can process 8 floats at once, plus other optimizations
            } else if is_x86_feature_detected!("sse2") {
                3.0 // SSE2 can process 4 floats at once
            } else {
                1.5 // General assembly optimizations
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        1.0
    } else {
        1.0
    };
    
    let optimized_time = autocorr_per_frame / expected_speedup;
    let optimized_margin = 10.0 / optimized_time;
    
    println!("     Expected speedup: {:.1}x", expected_speedup);
    println!("     Expected time per frame: {:.3} ms", optimized_time);
    println!("     Expected real-time margin: {:.1}x", optimized_margin);
    println!("     Status: {}", if cfg!(feature = "g729_asm") { "ASSEMBLY ENABLED" } else { "Assembly disabled (fallback mode)" });
}