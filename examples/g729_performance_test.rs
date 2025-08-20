/*
 * G.729 Performance Test and Benchmark
 * 
 * Simple test to verify G.729 assembly optimization performance
 */

use std::time::Instant;

// Import G.729 modules directly
use redfire_switch::g729_codec::G729Codec;
use redfire_switch::g729_optimized::OptimizedG729Codec;

const TEST_FRAMES: usize = 1000;
const FRAME_SIZE: usize = 80;

fn generate_test_signal(frames: usize) -> Vec<Vec<i16>> {
    let mut signals = Vec::new();
    
    for frame_idx in 0..frames {
        let mut frame = Vec::with_capacity(FRAME_SIZE);
        
        for i in 0..FRAME_SIZE {
            let t = (frame_idx * FRAME_SIZE + i) as f32 / 8000.0;
            
            // Composite signal: fundamental + harmonics
            let fundamental = 800.0 + 200.0 * (t * 0.5).sin();
            let signal = 0.6 * (2.0 * std::f32::consts::PI * fundamental * t).sin()
                       + 0.3 * (2.0 * std::f32::consts::PI * fundamental * 2.0 * t).sin()
                       + 0.1 * (2.0 * std::f32::consts::PI * fundamental * 3.0 * t).sin();
            
            let envelope = 1.0 - 0.2 * ((t * 10.0).sin().abs());
            let sample = (signal * envelope * 16000.0) as i16;
            
            frame.push(sample);
        }
        
        signals.push(frame);
    }
    
    signals
}

fn benchmark_rust_g729(test_signals: &[Vec<i16>]) -> (f64, Vec<Vec<u8>>) {
    println!("Benchmarking pure Rust G.729 implementation...");
    
    let mut codec = G729Codec::new();
    let mut encoded_frames = Vec::new();
    
    let start = Instant::now();
    
    for signal in test_signals {
        match codec.encode(signal) {
            Ok(encoded) => encoded_frames.push(encoded),
            Err(e) => eprintln!("Encoding error: {}", e),
        }
    }
    
    let elapsed = start.elapsed();
    let duration_ms = elapsed.as_millis() as f64;
    
    println!("Rust G.729: {} frames in {:.2} ms ({:.2} frames/sec)",
             encoded_frames.len(), duration_ms, encoded_frames.len() as f64 / duration_ms * 1000.0);
    
    (duration_ms, encoded_frames)
}

fn benchmark_optimized_g729(test_signals: &[Vec<i16>]) -> (f64, Vec<Vec<u8>>) {
    println!("Benchmarking optimized G.729 implementation...");
    
    let mut codec = OptimizedG729Codec::new();
    let mut encoded_frames = Vec::new();
    
    let start = Instant::now();
    
    for signal in test_signals {
        match codec.encode(signal) {
            Ok(encoded) => encoded_frames.push(encoded),
            Err(e) => eprintln!("Encoding error: {}", e),
        }
    }
    
    let elapsed = start.elapsed();
    let duration_ms = elapsed.as_millis() as f64;
    
    println!("Optimized G.729: {} frames in {:.2} ms ({:.2} frames/sec)",
             encoded_frames.len(), duration_ms, encoded_frames.len() as f64 / duration_ms * 1000.0);
    
    // Print SIMD usage stats
    let (simd_ops, fallback_ops, ratio) = codec.get_performance_stats();
    println!("SIMD usage: {:.1}% ({} SIMD ops, {} fallback ops)", 
             ratio, simd_ops, fallback_ops);
    
    (duration_ms, encoded_frames)
}

fn main() {
    println!("=== G.729 Performance Test ===");
    
    // Check system capabilities using external assembly codec
    use redfire_switch::g729_external_asm::ExternalAsmG729Codec;
    let codec = ExternalAsmG729Codec::new();
    let cpu_features = codec.get_performance_stats().cpu_features;
    let (sse, avx, fma) = (cpu_features.sse, cpu_features.avx, cpu_features.fma);
    println!("System SIMD support - SSE: {}, AVX: {}, FMA: {}", sse, avx, fma);
    
    // Generate test signals
    println!("\nGenerating {} test frames...", TEST_FRAMES);
    let test_signals = generate_test_signal(TEST_FRAMES);
    
    // Benchmark pure Rust implementation
    let (rust_time, rust_encoded) = benchmark_rust_g729(&test_signals);
    
    // Benchmark optimized implementation
    let (opt_time, opt_encoded) = benchmark_optimized_g729(&test_signals);
    
    // Calculate performance improvement
    if rust_time > 0.0 {
        let speedup = rust_time / opt_time;
        println!("\n=== Performance Results ===");
        println!("Rust G.729: {:.2} ms", rust_time);
        println!("Optimized G.729: {:.2} ms", opt_time);
        println!("Speedup: {:.2}x", speedup);
        
        let improvement_percent = ((rust_time - opt_time) / rust_time) * 100.0;
        println!("Performance improvement: {:.1}%", improvement_percent);
        
        // Validate output consistency
        if rust_encoded.len() == opt_encoded.len() {
            println!("Output frame count: {} (consistent)", rust_encoded.len());
            
            // Check a few frames for basic consistency
            let mut differences = 0;
            for (i, (rust_frame, opt_frame)) in rust_encoded.iter().zip(opt_encoded.iter()).enumerate().take(10) {
                if rust_frame != opt_frame {
                    differences += 1;
                    if i < 3 {
                        println!("Frame {}: Rust {:?} vs Opt {:?}", i, &rust_frame[..5], &opt_frame[..5]);
                    }
                }
            }
            
            if differences == 0 {
                println!("Output consistency: Perfect match");
            } else {
                println!("Output consistency: {} differences in first 10 frames", differences);
                println!("Note: Some differences expected due to different implementations");
            }
        }
    } else {
        println!("Error: Invalid benchmark results");
    }
    
    // Real-time capability assessment
    let frames_per_second = 100.0; // G.729 uses 10ms frames = 100 fps
    let required_time = TEST_FRAMES as f64 * 10.0; // 10ms per frame
    
    println!("\n=== Real-time Capability ===");
    println!("Required time for real-time: {:.2} ms", required_time);
    println!("Rust G.729 real-time capable: {}", if rust_time <= required_time { "Yes" } else { "No" });
    println!("Optimized G.729 real-time capable: {}", if opt_time <= required_time { "Yes" } else { "No" });
    
    if avx {
        println!("SIMD acceleration available and should provide significant speedup");
    } else {
        println!("Limited SIMD support - consider upgrading hardware for better performance");
    }
}