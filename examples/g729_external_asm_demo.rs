/*
 * G.729 External Assembly Codec Demonstration
 *
 * Comprehensive demo showing the external assembly-optimized G.729 codec
 * with performance benchmarking and CPU feature detection.
 */

use redfire_switch::g729_external_asm::{
    autocorrelation_optimized, levinson_durbin_optimized, lsp_quantization_optimized,
    ExternalAsmG729Codec, L_FRAME, L_WINDOW,
};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== G.729 External Assembly Codec Demo ===\n");

    // Test CPU feature detection
    test_cpu_features();

    // Test individual DSP functions
    test_dsp_functions()?;

    // Test complete codec with performance measurement
    test_codec_performance()?;

    // Run comprehensive benchmark
    run_comprehensive_benchmark()?;

    println!("\n=== Demo Complete ===");
    Ok(())
}

fn test_cpu_features() {
    println!("=== CPU Feature Detection ===");
    let codec = ExternalAsmG729Codec::new();
    let stats = codec.get_performance_stats();

    println!(
        "Assembly optimization available: {}",
        stats.assembly_available
    );
    println!("CPU Features:");
    println!("  SSE:  {}", stats.cpu_features.sse);
    println!("  SSE2: {}", stats.cpu_features.sse2);
    println!("  AVX:  {}", stats.cpu_features.avx);
    println!("  FMA:  {}", stats.cpu_features.fma);
    println!();
}

fn test_dsp_functions() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== DSP Function Testing ===");

    // Test autocorrelation
    println!("Testing autocorrelation function...");
    let mut windowed_speech = [0.0f32; L_WINDOW];
    for i in 0..L_WINDOW {
        // Generate speech-like signal
        let t = i as f32 / 8000.0;
        windowed_speech[i] = (2.0 * std::f32::consts::PI * 200.0 * t).sin()
            * (0.54 - 0.46 * (2.0 * std::f32::consts::PI * i as f32 / (L_WINDOW - 1) as f32).cos());
    }

    let start = Instant::now();
    let mut r = [0.0f32; 11];
    autocorrelation_optimized(&windowed_speech, &mut r);
    let autocorr_time = start.elapsed();

    println!("  Autocorrelation completed in {:?}", autocorr_time);
    println!(
        "  r[0] = {:.3}, r[1] = {:.3}, r[2] = {:.3}",
        r[0], r[1], r[2]
    );
    println!("  ✓ r[0] > r[1]: {}", r[0] > r[1]);

    // Test Levinson-Durbin
    println!("\nTesting Levinson-Durbin algorithm...");
    let start = Instant::now();
    let mut lp_coeffs = [0.0f32; 11];
    let prediction_error = levinson_durbin_optimized(&r, &mut lp_coeffs);
    let levinson_time = start.elapsed();

    println!("  Levinson-Durbin completed in {:?}", levinson_time);
    println!("  Prediction error: {:.6}", prediction_error);
    println!(
        "  LP coeffs[0-2]: [{:.3}, {:.3}, {:.3}]",
        lp_coeffs[0], lp_coeffs[1], lp_coeffs[2]
    );
    println!(
        "  ✓ lp_coeffs[0] == 1.0: {}",
        (lp_coeffs[0] - 1.0).abs() < 0.001
    );

    // Test LSP quantization
    println!("\nTesting LSP quantization...");
    let lsp = [
        0.314, 0.628, 0.942, 1.256, 1.570, 1.884, 2.198, 2.512, 2.826, 3.140,
    ];
    let codebook = generate_test_codebook(512);

    let start = Instant::now();
    let (best_index, min_distance) = lsp_quantization_optimized(&lsp, &codebook, 512);
    let lsp_time = start.elapsed();

    println!("  LSP quantization completed in {:?}", lsp_time);
    println!(
        "  Best index: {}, Min distance: {:.6}",
        best_index, min_distance
    );
    println!("  ✓ Valid index: {}", best_index < 512);

    println!("DSP functions test completed successfully!\n");
    Ok(())
}

fn test_codec_performance() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Codec Performance Test ===");

    let mut codec = ExternalAsmG729Codec::new();
    let test_frames = 100;

    println!("Processing {} frames...", test_frames);

    let overall_start = Instant::now();
    let mut total_encode_time = 0;

    for frame_num in 0..test_frames {
        // Generate test speech frame
        let mut speech_frame = [0.0f32; L_FRAME];
        for i in 0..L_FRAME {
            let t = (frame_num * L_FRAME + i) as f32 / 8000.0;
            // Multi-harmonic speech-like signal
            speech_frame[i] = 0.6 * (2.0 * std::f32::consts::PI * 440.0 * t).sin()
                + 0.3 * (2.0 * std::f32::consts::PI * 880.0 * t).sin()
                + 0.1 * (2.0 * std::f32::consts::PI * 1320.0 * t).sin();
        }

        let start = Instant::now();
        let _encoded = codec.encode(&speech_frame);
        total_encode_time += start.elapsed().as_nanos();

        if frame_num % 25 == 0 {
            print!(".");
            std::io::Write::flush(&mut std::io::stdout()).unwrap();
        }
    }
    println!();

    let total_time = overall_start.elapsed();
    let avg_encode_time = total_encode_time as f64 / test_frames as f64 / 1_000_000.0; // ms

    println!("Performance Results:");
    println!("  Total time: {:?}", total_time);
    println!("  Average encode time: {:.3} ms per frame", avg_encode_time);
    println!("  Throughput: {:.0} frames/sec", 1000.0 / avg_encode_time);

    // Real-time assessment (G.729 processes 10ms frames)
    let real_time_capable = avg_encode_time <= 10.0;
    println!(
        "  Real-time capable (≤10ms): {} {}",
        if real_time_capable { "✓" } else { "✗" },
        if real_time_capable { "Yes" } else { "No" }
    );

    // Get detailed stats
    let stats = codec.get_performance_stats();
    println!("\nDetailed Statistics:");
    println!("  Frames processed: {}", stats.frames_processed);
    println!("  Assembly operations: {}", stats.asm_operations);
    println!("  Fallback operations: {}", stats.fallback_operations);
    println!("  Assembly usage: {:.1}%", stats.asm_usage_percentage);
    println!();

    Ok(())
}

fn run_comprehensive_benchmark() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Comprehensive Performance Benchmark ===");

    let test_sizes = [100, 500, 1000, 2000];

    for &frames in &test_sizes {
        println!("\nBenchmarking {} frames:", frames);

        let mut codec = ExternalAsmG729Codec::new();
        let start_time = Instant::now();

        for frame_num in 0..frames {
            let mut speech_frame = [0.0f32; L_FRAME];

            // Generate more complex test signal
            for i in 0..L_FRAME {
                let t = (frame_num * L_FRAME + i) as f32 / 8000.0;
                let freq_mod = 200.0 + 100.0 * (t * 0.5).sin();
                speech_frame[i] = (2.0 * std::f32::consts::PI * freq_mod * t).sin()
                    * (0.8 + 0.2 * (t * 3.0).cos())
                    * (-t * 0.1).exp(); // Decay envelope
            }

            let _encoded = codec.encode(&speech_frame);
        }

        let total_time = start_time.elapsed();
        let stats = codec.get_performance_stats();

        // Calculate metrics
        let frames_per_sec = frames as f64 / total_time.as_secs_f64();
        let avg_time_per_frame = total_time.as_millis() as f64 / frames as f64;
        let real_time_factor = frames_per_sec / 100.0; // G.729 is 100 frames/sec at 10ms each

        println!(
            "  Time: {:?} ({:.3} ms/frame)",
            total_time, avg_time_per_frame
        );
        println!("  Throughput: {:.0} frames/sec", frames_per_sec);
        println!("  Real-time factor: {:.1}x", real_time_factor);
        println!("  Assembly usage: {:.1}%", stats.asm_usage_percentage);

        if real_time_factor >= 1.0 {
            println!("  ✓ Real-time capable");
        } else {
            println!("  ✗ Not real-time capable");
        }
    }

    // Memory usage and efficiency analysis
    println!("\nMemory and Efficiency Analysis:");
    println!(
        "  Codec state size: ~{} bytes",
        std::mem::size_of::<ExternalAsmG729Codec>()
    );
    println!("  Frame size: {} samples ({} bytes)", L_FRAME, L_FRAME * 4);
    println!(
        "  Analysis window: {} samples ({} bytes)",
        L_WINDOW,
        L_WINDOW * 4
    );

    #[cfg(feature = "g729_asm")]
    {
        println!("  External assembly: ENABLED");
        println!("  Expected performance gain: 2-4x over scalar");
    }
    #[cfg(not(feature = "g729_asm"))]
    {
        println!("  External assembly: DISABLED (fallback mode)");
        println!("  Performance: Scalar baseline");
    }

    Ok(())
}

fn generate_test_codebook(size: usize) -> [[f32; 10]; 512] {
    let mut codebook = [[0.0f32; 10]; 512];

    for i in 0..size.min(512) {
        for j in 0..10 {
            // Generate LSP-like values in proper range
            codebook[i][j] = (j + 1) as f32 * std::f32::consts::PI / 11.0
                + (i as f32 / size as f32)
                    * 0.1
                    * (2.0 * std::f32::consts::PI * j as f32 / 10.0).cos();
        }
    }

    codebook
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demo_functions() {
        // Basic smoke test to ensure demo functions work
        assert!(test_dsp_functions().is_ok());
        assert!(test_codec_performance().is_ok());
    }
}
