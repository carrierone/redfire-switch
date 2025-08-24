/*
 * G.729 Performance Benchmark Suite
 *
 * Comprehensive benchmarks comparing pure Rust G.729 implementation
 * with x86-64 SIMD-optimized version
 */

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use redfire_switch::g729_codec::G729Codec;
use redfire_switch::g729_external_asm::{ExternalAsmG729Codec, L_FRAME, L_WINDOW};
use redfire_switch::g729_optimized::OptimizedG729Codec;
use std::time::Duration;

/// Generate realistic speech-like test signals
fn generate_test_signals(num_frames: usize) -> Vec<Vec<i16>> {
    let mut signals = Vec::new();

    for frame_idx in 0..num_frames {
        let mut frame = Vec::with_capacity(L_FRAME);

        for i in 0..L_FRAME {
            let t = (frame_idx * L_FRAME + i) as f32 / 8000.0; // Time in seconds

            // Composite signal: fundamental + harmonics + noise
            let fundamental = 800.0 + 200.0 * (t * 0.5).sin(); // Varying pitch
            let signal = 0.6 * (2.0 * std::f32::consts::PI * fundamental * t).sin()
                + 0.3 * (2.0 * std::f32::consts::PI * fundamental * 2.0 * t).sin()
                + 0.1 * (2.0 * std::f32::consts::PI * fundamental * 3.0 * t).sin()
                + 0.05 * (t * 1000.0).sin(); // Add some noise

            // Apply envelope for more realistic speech-like signal
            let envelope = 1.0 - 0.3 * ((t * 10.0).sin().abs());
            let sample = (signal * envelope * 16000.0) as i16;

            frame.push(sample);
        }

        signals.push(frame);
    }

    signals
}

/// Benchmark pure Rust G.729 codec encoding
fn bench_rust_g729_encode(c: &mut Criterion) {
    let test_signals = generate_test_signals(100);
    let mut group = c.benchmark_group("g729_rust_encode");

    // Set throughput for meaningful comparison
    group.throughput(Throughput::Elements(L_FRAME as u64));
    group.measurement_time(Duration::from_secs(30));

    for signal_count in [1, 10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("frames", signal_count),
            signal_count,
            |b, &signal_count| {
                let signals = &test_signals[..signal_count];
                b.iter(|| {
                    let mut codec = G729Codec::new();
                    let mut encoded_frames = Vec::new();

                    for signal in signals {
                        let encoded = codec.encode(black_box(signal)).unwrap();
                        encoded_frames.push(black_box(encoded));
                    }

                    encoded_frames
                });
            },
        );
    }

    group.finish();
}

/// Benchmark optimized G.729 codec encoding
fn bench_optimized_g729_encode(c: &mut Criterion) {
    let test_signals = generate_test_signals(100);
    let mut group = c.benchmark_group("g729_optimized_encode");

    group.throughput(Throughput::Elements(L_FRAME as u64));
    group.measurement_time(Duration::from_secs(30));

    for signal_count in [1, 10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("frames", signal_count),
            signal_count,
            |b, &signal_count| {
                let signals = &test_signals[..signal_count];
                b.iter(|| {
                    let mut codec = OptimizedG729Codec::new();
                    let mut encoded_frames = Vec::new();

                    for signal in signals {
                        let encoded = codec.encode(black_box(signal)).unwrap();
                        encoded_frames.push(black_box(encoded));
                    }

                    encoded_frames
                });
            },
        );
    }

    group.finish();
}

/// Benchmark pure Rust G.729 codec decoding
fn bench_rust_g729_decode(c: &mut Criterion) {
    // Generate encoded frames for decoding
    let test_signals = generate_test_signals(100);
    let mut encoded_frames = Vec::new();
    {
        let mut encoder = G729Codec::new();
        for signal in &test_signals {
            encoded_frames.push(encoder.encode(signal).unwrap());
        }
    }

    let mut group = c.benchmark_group("g729_rust_decode");
    group.throughput(Throughput::Elements(L_FRAME as u64));
    group.measurement_time(Duration::from_secs(20));

    for frame_count in [1, 10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("frames", frame_count),
            frame_count,
            |b, &frame_count| {
                let frames = &encoded_frames[..frame_count];
                b.iter(|| {
                    let mut codec = G729Codec::new();
                    let mut decoded_frames = Vec::new();

                    for frame in frames {
                        let decoded = codec.decode(black_box(frame)).unwrap();
                        decoded_frames.push(black_box(decoded));
                    }

                    decoded_frames
                });
            },
        );
    }

    group.finish();
}

/// Benchmark individual DSP functions for detailed analysis
fn bench_dsp_functions(c: &mut Criterion) {
    let mut group = c.benchmark_group("dsp_functions");
    group.measurement_time(Duration::from_secs(20));

    // Test data
    let windowed_speech = {
        let mut data = [0.0f32; L_WINDOW];
        for (i, sample) in data.iter_mut().enumerate() {
            *sample = (i as f32 * 0.01).sin() * 0.5;
        }
        data
    };

    let autocorr_data = [
        100.0, 80.0, 60.0, 40.0, 30.0, 20.0, 15.0, 10.0, 5.0, 2.0, 1.0,
    ];

    // Benchmark autocorrelation with external assembly
    let codec = ExternalAsmG729Codec::new();
    if codec.get_performance_stats().cpu_features.avx {
        // AVX available
        group.bench_function("autocorr_external_asm", |b| {
            b.iter(|| {
                let mut result = [0.0f32; 11];
                redfire_switch::g729_external_asm::autocorrelation_optimized(
                    black_box(&windowed_speech),
                    black_box(&mut result),
                );
                result
            });
        });
    }

    group.bench_function("autocorr_scalar", |b| {
        b.iter(|| {
            let mut result = [0.0f32; 11];
            // Scalar autocorrelation
            for k in 0..11 {
                result[k] = 0.0;
                for i in 0..(L_WINDOW - k) {
                    result[k] += windowed_speech[i] * windowed_speech[i + k];
                }
            }
            black_box(result)
        });
    });

    // Benchmark Levinson-Durbin algorithm with external assembly
    if codec.get_performance_stats().cpu_features.sse2 {
        group.bench_function("levinson_durbin_external_asm", |b| {
            b.iter(|| {
                let mut lp_coeffs = [0.0f32; 11];
                let error = redfire_switch::g729_external_asm::levinson_durbin_optimized(
                    black_box(&autocorr_data),
                    black_box(&mut lp_coeffs),
                );
                (lp_coeffs, error)
            });
        });
    }

    group.bench_function("levinson_durbin_scalar", |b| {
        b.iter(|| {
            let mut lp_coeffs = [0.0f32; 11];
            lp_coeffs[0] = 1.0;
            let mut error = autocorr_data[0];

            for i in 1..=10 {
                let mut sum = 0.0;
                for j in 1..i {
                    sum += lp_coeffs[j] * autocorr_data[i - j];
                }

                let k_i = -(autocorr_data[i] + sum) / error;
                lp_coeffs[i] = k_i;

                for j in 1..=(i / 2) {
                    let temp = lp_coeffs[j] + k_i * lp_coeffs[i - j];
                    lp_coeffs[i - j] += k_i * lp_coeffs[j];
                    lp_coeffs[j] = temp;
                }

                error *= 1.0 - k_i * k_i;
            }

            black_box((lp_coeffs, error))
        });
    });

    group.finish();
}

/// Comprehensive comparison benchmark
fn bench_codec_comparison(c: &mut Criterion) {
    let test_signals = generate_test_signals(50);
    let mut group = c.benchmark_group("codec_comparison");

    group.throughput(Throughput::Elements(50 * L_FRAME as u64));
    group.measurement_time(Duration::from_secs(45));

    group.bench_function("rust_g729", |b| {
        b.iter(|| {
            let mut codec = G729Codec::new();
            let mut results = Vec::new();

            for signal in &test_signals {
                let encoded = codec.encode(black_box(signal)).unwrap();
                results.push(black_box(encoded));
            }

            results
        });
    });

    group.bench_function("optimized_g729", |b| {
        b.iter(|| {
            let mut codec = OptimizedG729Codec::new();
            let mut results = Vec::new();

            for signal in &test_signals {
                let encoded = codec.encode(black_box(signal)).unwrap();
                results.push(black_box(encoded));
            }

            // Print performance stats for analysis
            let (simd_ops, fallback_ops, ratio) = codec.get_performance_stats();
            if simd_ops + fallback_ops > 0 {
                eprintln!(
                    "SIMD usage: {:.1}% ({} SIMD, {} fallback)",
                    ratio, simd_ops, fallback_ops
                );
            }

            results
        });
    });

    group.finish();
}

/// Memory usage and allocation benchmark
fn bench_memory_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_performance");
    group.measurement_time(Duration::from_secs(15));

    let test_signal: Vec<i16> = (0..L_FRAME)
        .map(|i| ((i as f32 * 0.1).sin() * 16000.0) as i16)
        .collect();

    group.bench_function("rust_codec_creation", |b| {
        b.iter(|| {
            let codec = G729Codec::new();
            black_box(codec)
        });
    });

    group.bench_function("optimized_codec_creation", |b| {
        b.iter(|| {
            let codec = OptimizedG729Codec::new();
            black_box(codec)
        });
    });

    group.bench_function("encode_single_frame_rust", |b| {
        let mut codec = G729Codec::new();
        b.iter(|| {
            let encoded = codec.encode(black_box(&test_signal)).unwrap();
            black_box(encoded)
        });
    });

    group.bench_function("encode_single_frame_optimized", |b| {
        let mut codec = OptimizedG729Codec::new();
        b.iter(|| {
            let encoded = codec.encode(black_box(&test_signal)).unwrap();
            black_box(encoded)
        });
    });

    group.finish();
}

/// Real-time performance simulation
fn bench_realtime_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("realtime_simulation");
    group.measurement_time(Duration::from_secs(60));

    // Simulate 1 second of audio (100 frames @ 10ms each)
    let audio_frames = generate_test_signals(100);

    group.bench_function("1_second_rust", |b| {
        b.iter(|| {
            let mut codec = G729Codec::new();
            let mut encoded_data = Vec::new();

            for frame in &audio_frames {
                let encoded = codec.encode(black_box(frame)).unwrap();
                encoded_data.push(encoded);
            }

            black_box(encoded_data)
        });
    });

    group.bench_function("1_second_optimized", |b| {
        b.iter(|| {
            let mut codec = OptimizedG729Codec::new();
            let mut encoded_data = Vec::new();

            for frame in &audio_frames {
                let encoded = codec.encode(black_box(frame)).unwrap();
                encoded_data.push(encoded);
            }

            black_box(encoded_data)
        });
    });

    group.finish();
}

/// Print system information for benchmark context
fn print_system_info() {
    println!("=== G.729 Benchmark System Information ===");

    let (sse, avx, fma) = check_simd_support();
    println!("SIMD Support - SSE: {}, AVX: {}, FMA: {}", sse, avx, fma);

    if let Ok(cpu_info) = std::fs::read_to_string("/proc/cpuinfo") {
        if let Some(model_line) = cpu_info.lines().find(|line| line.starts_with("model name")) {
            println!(
                "CPU: {}",
                model_line.split(':').nth(1).unwrap_or("Unknown").trim()
            );
        }

        let cpu_count = cpu_info
            .lines()
            .filter(|line| line.starts_with("processor"))
            .count();
        println!("CPU Cores: {}", cpu_count);
    }

    println!(
        "G.729 Frame Size: {} samples ({} ms)",
        L_FRAME,
        L_FRAME * 1000 / 8000
    );
    println!("Analysis Window Size: {} samples", L_WINDOW);
    println!("===========================================");
}

// Custom benchmark configuration
fn custom_criterion() -> Criterion {
    Criterion::default()
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(30))
        .sample_size(50)
        .noise_threshold(0.05)
        .confidence_level(0.95)
}

criterion_group!(
    name = benches;
    config = custom_criterion();
    targets =
        bench_rust_g729_encode,
        bench_optimized_g729_encode,
        bench_rust_g729_decode,
        bench_dsp_functions,
        bench_codec_comparison,
        bench_memory_performance,
        bench_realtime_simulation
);

criterion_main!(benches);

#[cfg(test)]
mod benchmark_tests {
    use super::*;

    #[test]
    fn test_signal_generation() {
        let signals = generate_test_signals(10);
        assert_eq!(signals.len(), 10);
        assert_eq!(signals[0].len(), L_FRAME);

        // Check signal has reasonable amplitude
        let max_amplitude = signals[0].iter().map(|&x| x.abs()).max().unwrap();
        assert!(max_amplitude > 1000);
        assert!(max_amplitude < 32767);
    }

    #[test]
    fn test_codec_functionality() {
        let test_signal = generate_test_signals(1);

        // Test Rust codec
        let mut rust_codec = G729Codec::new();
        let rust_encoded = rust_codec.encode(&test_signal[0]).unwrap();
        assert_eq!(rust_encoded.len(), 10);

        // Test optimized codec
        let mut opt_codec = OptimizedG729Codec::new();
        let opt_encoded = opt_codec.encode(&test_signal[0]).unwrap();
        assert_eq!(opt_encoded.len(), 10);

        // Results should be similar (but may not be identical due to different implementations)
        println!("Rust encoded: {:?}", &rust_encoded[..5]);
        println!("Optimized encoded: {:?}", &opt_encoded[..5]);
    }

    #[test]
    fn test_performance_stats() {
        let mut codec = OptimizedG729Codec::new();
        let test_signal = generate_test_signals(1);

        let _ = codec.encode(&test_signal[0]).unwrap();

        let (simd_ops, fallback_ops, ratio) = codec.get_performance_stats();
        println!(
            "Performance stats: SIMD={}, Fallback={}, Ratio={:.1}%",
            simd_ops, fallback_ops, ratio
        );

        assert!(simd_ops > 0 || fallback_ops > 0);
    }

    #[test]
    fn verify_benchmark_setup() {
        print_system_info();

        // Verify both codec versions are functional
        let signals = generate_test_signals(5);

        let mut rust_codec = G729Codec::new();
        let mut opt_codec = OptimizedG729Codec::new();

        for signal in &signals {
            let rust_result = rust_codec.encode(signal);
            let opt_result = opt_codec.encode(signal);

            assert!(rust_result.is_ok());
            assert!(opt_result.is_ok());
        }

        println!("Benchmark setup verification completed successfully!");
    }
}
