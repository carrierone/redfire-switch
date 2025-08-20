/*
 * Simple G.729 Performance Test
 * 
 * A minimal standalone test for G.729 performance comparison
 * between scalar and optimized implementations
 */

use std::time::Instant;

// Simple G.729 frame constants
const G729_FRAME_SIZE: usize = 80;
const G729_WINDOW_SIZE: usize = 240;

/// Simple scalar autocorrelation for baseline
fn scalar_autocorrelation(windowed: &[f32], r: &mut [f32; 11]) {
    for k in 0..11 {
        r[k] = 0.0;
        let limit = windowed.len() - k;
        for i in 0..limit {
            r[k] += windowed[i] * windowed[i + k];
        }
    }
}

/// Optimized autocorrelation using manual loop unrolling
fn optimized_autocorrelation(windowed: &[f32], r: &mut [f32; 11]) {
    for k in 0..11 {
        let mut sum = 0.0f32;
        let limit = windowed.len() - k;
        
        // Process 4 samples at a time (manual unrolling)
        let mut i = 0;
        while i + 4 <= limit {
            sum += windowed[i] * windowed[i + k] + 
                   windowed[i + 1] * windowed[i + 1 + k] +
                   windowed[i + 2] * windowed[i + 2 + k] +
                   windowed[i + 3] * windowed[i + 3 + k];
            i += 4;
        }
        
        // Handle remaining samples
        while i < limit {
            sum += windowed[i] * windowed[i + k];
            i += 1;
        }
        
        r[k] = sum;
    }
}

/// Simple LSP quantization simulation
fn scalar_lsp_quantization(lsp: &[f32; 10], codebook_size: usize) -> (usize, f32) {
    let mut best_index = 0;
    let mut min_distance = f32::INFINITY;
    
    // Simulate codebook search
    for index in 0..codebook_size {
        let mut distance = 0.0f32;
        for i in 0..10 {
            // Generate synthetic codebook entry
            let entry_val = (index as f32 / codebook_size as f32 + i as f32 / 10.0) * 0.1;
            let diff = lsp[i] - entry_val;
            distance += diff * diff;
        }
        
        if distance < min_distance {
            min_distance = distance;
            best_index = index;
        }
    }
    
    (best_index, min_distance)
}

/// Optimized LSP quantization with loop unrolling
fn optimized_lsp_quantization(lsp: &[f32; 10], codebook_size: usize) -> (usize, f32) {
    let mut best_index = 0;
    let mut min_distance = f32::INFINITY;
    
    // Process codebook entries in groups of 4
    for chunk_start in (0..codebook_size).step_by(4) {
        let chunk_end = (chunk_start + 4).min(codebook_size);
        
        for index in chunk_start..chunk_end {
            let mut distance = 0.0f32;
            
            // Unroll inner loop for better performance
            let mut i = 0;
            while i + 4 <= 10 {
                let entry_val0 = (index as f32 / codebook_size as f32 + (i + 0) as f32 / 10.0) * 0.1;
                let entry_val1 = (index as f32 / codebook_size as f32 + (i + 1) as f32 / 10.0) * 0.1;
                let entry_val2 = (index as f32 / codebook_size as f32 + (i + 2) as f32 / 10.0) * 0.1;
                let entry_val3 = (index as f32 / codebook_size as f32 + (i + 3) as f32 / 10.0) * 0.1;
                
                let diff0 = lsp[i] - entry_val0;
                let diff1 = lsp[i + 1] - entry_val1;
                let diff2 = lsp[i + 2] - entry_val2;
                let diff3 = lsp[i + 3] - entry_val3;
                
                distance += diff0 * diff0 + diff1 * diff1 + diff2 * diff2 + diff3 * diff3;
                i += 4;
            }
            
            // Handle remaining elements
            while i < 10 {
                let entry_val = (index as f32 / codebook_size as f32 + i as f32 / 10.0) * 0.1;
                let diff = lsp[i] - entry_val;
                distance += diff * diff;
                i += 1;
            }
            
            if distance < min_distance {
                min_distance = distance;
                best_index = index;
            }
        }
    }
    
    (best_index, min_distance)
}

/// Generate test signal
fn generate_test_signal(frames: usize) -> Vec<Vec<f32>> {
    let mut signals = Vec::new();
    
    for frame_idx in 0..frames {
        let mut frame = Vec::with_capacity(G729_WINDOW_SIZE);
        
        for i in 0..G729_WINDOW_SIZE {
            let t = (frame_idx * G729_FRAME_SIZE + i) as f32 / 8000.0;
            
            // Speech-like signal with multiple harmonics
            let fundamental = 200.0 + 600.0 * (t * 0.3).sin();
            let signal = 0.6 * (2.0 * std::f32::consts::PI * fundamental * t).sin()
                       + 0.3 * (2.0 * std::f32::consts::PI * fundamental * 2.0 * t).sin()
                       + 0.1 * (2.0 * std::f32::consts::PI * fundamental * 3.0 * t).sin();
            
            // Apply window function
            let window = 0.54 - 0.46 * (2.0 * std::f32::consts::PI * i as f32 / (G729_WINDOW_SIZE - 1) as f32).cos();
            frame.push(signal * window);
        }
        
        signals.push(frame);
    }
    
    signals
}

/// Benchmark scalar implementation
fn benchmark_scalar(test_signals: &[Vec<f32>]) -> (f64, f64) {
    let mut total_autocorr_time = 0.0;
    let mut total_lsp_time = 0.0;
    
    for signal in test_signals {
        // Autocorrelation benchmark
        let mut r = [0.0f32; 11];
        let start = Instant::now();
        scalar_autocorrelation(signal, &mut r);
        total_autocorr_time += start.elapsed().as_nanos() as f64;
        
        // Generate LSP for quantization test
        let mut lsp = [0.0f32; 10];
        for i in 0..10 {
            lsp[i] = (i + 1) as f32 * std::f32::consts::PI / 11.0 + r[i + 1] * 0.01;
        }
        
        // LSP quantization benchmark
        let start = Instant::now();
        let _ = scalar_lsp_quantization(&lsp, 1024);
        total_lsp_time += start.elapsed().as_nanos() as f64;
    }
    
    (total_autocorr_time / 1_000_000.0, total_lsp_time / 1_000_000.0)
}

/// Benchmark optimized implementation
fn benchmark_optimized(test_signals: &[Vec<f32>]) -> (f64, f64) {
    let mut total_autocorr_time = 0.0;
    let mut total_lsp_time = 0.0;
    
    for signal in test_signals {
        // Autocorrelation benchmark
        let mut r = [0.0f32; 11];
        let start = Instant::now();
        optimized_autocorrelation(signal, &mut r);
        total_autocorr_time += start.elapsed().as_nanos() as f64;
        
        // Generate LSP for quantization test
        let mut lsp = [0.0f32; 10];
        for i in 0..10 {
            lsp[i] = (i + 1) as f32 * std::f32::consts::PI / 11.0 + r[i + 1] * 0.01;
        }
        
        // LSP quantization benchmark
        let start = Instant::now();
        let _ = optimized_lsp_quantization(&lsp, 1024);
        total_lsp_time += start.elapsed().as_nanos() as f64;
    }
    
    (total_autocorr_time / 1_000_000.0, total_lsp_time / 1_000_000.0)
}

pub fn run_g729_performance_test() {
    println!("=== G.729 Simple Performance Test ===");
    
    // Generate test data
    let test_frames = 1000;
    println!("Generating {} test frames...", test_frames);
    let test_signals = generate_test_signal(test_frames);
    
    // Benchmark scalar implementation
    println!("\nBenchmarking scalar implementation...");
    let (scalar_autocorr_ms, scalar_lsp_ms) = benchmark_scalar(&test_signals);
    
    // Benchmark optimized implementation  
    println!("Benchmarking optimized implementation...");
    let (opt_autocorr_ms, opt_lsp_ms) = benchmark_optimized(&test_signals);
    
    // Display results
    println!("\n=== Performance Results ===");
    println!("Autocorrelation:");
    println!("  Scalar:    {:.2} ms", scalar_autocorr_ms);
    println!("  Optimized: {:.2} ms", opt_autocorr_ms);
    println!("  Speedup:   {:.2}x", scalar_autocorr_ms / opt_autocorr_ms);
    
    println!("\nLSP Quantization:");
    println!("  Scalar:    {:.2} ms", scalar_lsp_ms);
    println!("  Optimized: {:.2} ms", opt_lsp_ms);
    println!("  Speedup:   {:.2}x", scalar_lsp_ms / opt_lsp_ms);
    
    let total_scalar = scalar_autocorr_ms + scalar_lsp_ms;
    let total_opt = opt_autocorr_ms + opt_lsp_ms;
    
    println!("\nTotal DSP Operations:");
    println!("  Scalar:    {:.2} ms", total_scalar);
    println!("  Optimized: {:.2} ms", total_opt);
    println!("  Overall speedup: {:.2}x", total_scalar / total_opt);
    
    // Real-time assessment
    let required_time_ms = test_frames as f64 * 10.0; // 10ms per frame
    println!("\n=== Real-time Capability ===");
    println!("Required time for real-time: {:.2} ms", required_time_ms);
    println!("Scalar real-time capable: {}", if total_scalar <= required_time_ms { "✓ Yes" } else { "✗ No" });
    println!("Optimized real-time capable: {}", if total_opt <= required_time_ms { "✓ Yes" } else { "✗ No" });
    
    let improvement_percent = ((total_scalar - total_opt) / total_scalar) * 100.0;
    println!("Performance improvement: {:.1}%", improvement_percent);
    
    // Additional metrics
    println!("\n=== Additional Metrics ===");
    println!("Frames processed: {}", test_frames);
    println!("Data processed: {:.2} MB", (test_frames * G729_WINDOW_SIZE * 4) as f64 / 1_000_000.0);
    println!("Scalar throughput: {:.0} frames/sec", test_frames as f64 / (total_scalar / 1000.0));
    println!("Optimized throughput: {:.0} frames/sec", test_frames as f64 / (total_opt / 1000.0));
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_autocorrelation_consistency() {
        let signal = generate_test_signal(1);
        
        let mut r_scalar = [0.0f32; 11];
        let mut r_optimized = [0.0f32; 11];
        
        scalar_autocorrelation(&signal[0], &mut r_scalar);
        optimized_autocorrelation(&signal[0], &mut r_optimized);
        
        // Check results are close (allowing for floating point differences)
        for i in 0..11 {
            let diff = (r_scalar[i] - r_optimized[i]).abs();
            assert!(diff < 0.001, "Autocorr mismatch at index {}: {} vs {}", i, r_scalar[i], r_optimized[i]);
        }
    }
    
    #[test]
    fn test_lsp_quantization_consistency() {
        let lsp = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
        
        let (idx_scalar, _) = scalar_lsp_quantization(&lsp, 256);
        let (idx_opt, _) = optimized_lsp_quantization(&lsp, 256);
        
        // Should find the same minimum (allowing for small numerical differences)
        assert_eq!(idx_scalar, idx_opt, "LSP quantization results should match");
    }
}