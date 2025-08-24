/*
 * Simple G.729 External Assembly Test
 *
 * Basic test to validate external assembly integration
 */

use redfire_switch::g729_external_asm::{
    autocorrelation_optimized, levinson_durbin_optimized, ExternalAsmG729Codec, L_WINDOW,
};
use std::time::Instant;

fn main() {
    println!("=== G.729 External Assembly Test ===\n");

    // Test individual functions
    test_autocorrelation();
    test_levinson_durbin();
    test_codec();

    println!("\n=== Test Complete ===");
}

fn test_autocorrelation() {
    println!("Testing autocorrelation function...");

    let mut windowed_speech = [0.0f32; L_WINDOW];
    for i in 0..L_WINDOW {
        windowed_speech[i] = (i as f32 * 0.1).sin();
    }

    let start = Instant::now();
    let mut r = [0.0f32; 11];
    autocorrelation_optimized(&windowed_speech, &mut r);
    let duration = start.elapsed();

    println!("  Completed in: {:?}", duration);
    println!("  r[0] = {:.3}, r[1] = {:.3}", r[0], r[1]);
    println!("  ✓ r[0] > r[1]: {}", r[0] > r[1]);
}

fn test_levinson_durbin() {
    println!("\nTesting Levinson-Durbin algorithm...");

    let r = [100.0, 50.0, 25.0, 12.0, 6.0, 3.0, 1.5, 0.7, 0.3, 0.1, 0.05];
    let mut lp_coeffs = [0.0f32; 11];

    let start = Instant::now();
    let error = levinson_durbin_optimized(&r, &mut lp_coeffs);
    let duration = start.elapsed();

    println!("  Completed in: {:?}", duration);
    println!("  Prediction error: {:.6}", error);
    println!("  LP coeffs[0] = {:.3} (should be 1.0)", lp_coeffs[0]);
    println!(
        "  ✓ Valid coefficients: {}",
        (lp_coeffs[0] - 1.0).abs() < 0.001
    );
}

fn test_codec() {
    println!("\nTesting G.729 codec...");

    let mut codec = ExternalAsmG729Codec::new();
    let speech_frame = [0.1f32; 80]; // 80 samples = 10ms at 8kHz

    let start = Instant::now();
    let encoded = codec.encode(&speech_frame);
    let duration = start.elapsed();

    println!("  Encode time: {:?}", duration);
    println!("  Encoded length: {} bytes", encoded.len());

    let stats = codec.get_performance_stats();
    println!("  Assembly available: {}", stats.assembly_available);
    println!(
        "  CPU features - SSE: {}, AVX: {}",
        stats.cpu_features.sse, stats.cpu_features.avx
    );

    // Real-time test
    let real_time_capable = duration.as_millis() <= 10;
    println!(
        "  Real-time capable: {} ({}ms <= 10ms)",
        if real_time_capable { "✓" } else { "✗" },
        duration.as_millis()
    );
}
