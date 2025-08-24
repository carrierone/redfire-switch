/*
 * DTMF Performance Benchmarks
 *
 * This benchmark suite measures the performance characteristics of the DTMF
 * implementation under various conditions:
 * - Detection latency and throughput
 * - Generation performance
 * - Cross-protocol processing overhead
 * - Memory usage patterns
 * - Concurrent processing capabilities
 */

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

use redfire_switch::dtmf_processor::{
    DtmfDetector, DtmfDetectorConfig, DtmfEvent, DtmfGenerator, DtmfGeneratorConfig, DtmfProcessor,
    DtmfSource,
};
use redfire_switch::rfc2833_events::{
    Rfc2833Event, Rfc2833EventId, Rfc2833PayloadType, Rfc2833Processor,
};
use redfire_switch::sigtran_dtmf::{
    GenericDigitsEncoding, SigtranDtmfConfig, SigtranDtmfMessage, SigtranDtmfMessageType,
    SigtranDtmfProcessor, SigtranProtocol,
};
use redfire_switch::sip_info_dtmf::{
    SipInfoDtmfContentType, SipInfoDtmfMessage, SipInfoDtmfProcessor,
};

/// Benchmark DTMF tone generation
fn bench_dtmf_generation(c: &mut Criterion) {
    let generator = DtmfGenerator::new(DtmfGeneratorConfig::default());

    let mut group = c.benchmark_group("dtmf_generation");

    // Single digit generation
    group.bench_function("single_digit", |b| {
        b.iter(|| {
            black_box(generator.generate_digit('5', None, None).unwrap());
        })
    });

    // Different durations
    for &duration_ms in &[50, 100, 200, 500] {
        group.bench_with_input(
            BenchmarkId::new("duration", duration_ms),
            &duration_ms,
            |b, &duration_ms| {
                b.iter(|| {
                    black_box(
                        generator
                            .generate_digit('5', Some(Duration::from_millis(duration_ms)), None)
                            .unwrap(),
                    );
                })
            },
        );
    }

    // Sequence generation
    let sequences = ["123", "1234567890", "*123#456*", "1234567890*0#ABCD"];
    for sequence in &sequences {
        group.bench_with_input(
            BenchmarkId::new("sequence", sequence.len()),
            sequence,
            |b, &sequence| {
                b.iter(|| {
                    black_box(
                        generator
                            .generate_sequence(sequence, None, None, None)
                            .unwrap(),
                    );
                })
            },
        );
    }

    group.finish();
}

/// Benchmark DTMF detection processing
fn bench_dtmf_detection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("dtmf_detection");
    group.sample_size(10); // Reduce sample size for async benchmarks

    // Setup
    let (detector, _receiver) = DtmfDetector::new(DtmfDetectorConfig::default());
    let generator = DtmfGenerator::new(DtmfGeneratorConfig::default());
    let detector = Arc::new(detector);

    // Pre-generate test audio samples
    let test_samples = generator
        .generate_digit('5', Some(Duration::from_millis(200)), None)
        .unwrap();

    // Benchmark audio processing
    group.bench_function("process_audio", |b| {
        b.to_async(&rt).iter(|| async {
            let detector_clone = Arc::clone(&detector);
            detector_clone
                .add_channel("bench_channel".to_string())
                .await
                .unwrap();
            black_box(
                detector_clone
                    .process_audio("bench_channel", &test_samples, DtmfSource::Internal)
                    .await
                    .unwrap(),
            );
            detector_clone
                .remove_channel("bench_channel")
                .await
                .unwrap();
        })
    });

    // Benchmark with different block sizes
    for &block_size in &[80, 160, 320, 640] {
        let mut config = DtmfDetectorConfig::default();
        config.block_size = block_size;
        let (detector, _receiver) = DtmfDetector::new(config);
        let detector = Arc::new(detector);

        group.bench_with_input(
            BenchmarkId::new("block_size", block_size),
            &block_size,
            |b, _| {
                b.to_async(&rt).iter(|| async {
                    let detector_clone = Arc::clone(&detector);
                    detector_clone
                        .add_channel("bench_channel".to_string())
                        .await
                        .unwrap();
                    black_box(
                        detector_clone
                            .process_audio("bench_channel", &test_samples, DtmfSource::Internal)
                            .await
                            .unwrap(),
                    );
                    detector_clone
                        .remove_channel("bench_channel")
                        .await
                        .unwrap();
                })
            },
        );
    }

    group.finish();
}

/// Benchmark RFC2833 event processing
fn bench_rfc2833_processing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("rfc2833_processing");

    let (event_sender, _receiver) = mpsc::unbounded_channel();
    let mut processor = Rfc2833Processor::new(event_sender);
    processor.add_payload_type(101, Rfc2833PayloadType::TelephoneEvent(101));
    let processor = Arc::new(processor);

    // Pre-generate test events
    let test_events: Vec<_> = "1234567890*#ABCD"
        .chars()
        .filter_map(|c| Rfc2833EventId::from_dtmf_char(c))
        .map(|id| Rfc2833Event::new(id, 10, 800).to_bytes().unwrap())
        .collect();

    // Benchmark single event processing
    group.bench_function("single_event", |b| {
        b.to_async(&rt).iter(|| async {
            let processor_clone = Arc::clone(&processor);
            black_box(
                processor_clone
                    .process_incoming_packet("bench_session", 101, 1000, &test_events[0])
                    .await
                    .unwrap(),
            );
        })
    });

    // Benchmark event sequence processing
    group.bench_function("event_sequence", |b| {
        b.to_async(&rt).iter(|| async {
            let processor_clone = Arc::clone(&processor);
            for (i, event_bytes) in test_events.iter().enumerate() {
                black_box(
                    processor_clone
                        .process_incoming_packet(
                            "bench_session",
                            101,
                            1000 + i as u32 * 100,
                            event_bytes,
                        )
                        .await
                        .unwrap(),
                );
            }
        })
    });

    // Benchmark packet generation
    group.bench_function("packet_generation", |b| {
        b.to_async(&rt).iter(|| async {
            let processor_clone = Arc::clone(&processor);
            black_box(
                processor_clone
                    .generate_outgoing_packets("bench_session", '5', 200, 20, 5000)
                    .await
                    .unwrap(),
            );
        })
    });

    group.finish();
}

/// Benchmark SIP INFO processing
fn bench_sip_info_processing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("sip_info_processing");

    let (event_sender, _receiver) = mpsc::unbounded_channel();
    let processor = Arc::new(SipInfoDtmfProcessor::new(event_sender));

    // Pre-generate test messages
    let cisco_message = SipInfoDtmfMessage::new('7', SipInfoDtmfContentType::CiscoDtmfRelay)
        .with_duration(150)
        .with_volume(80);
    let cisco_body = cisco_message.to_body_content();

    let generic_message =
        SipInfoDtmfMessage::new('7', SipInfoDtmfContentType::GenericDtmf).with_duration(150);
    let generic_body = generic_message.to_body_content();

    // Benchmark message parsing
    group.bench_function("parse_cisco_format", |b| {
        b.iter(|| {
            black_box(
                SipInfoDtmfMessage::from_body_content("application/dtmf-relay", &cisco_body)
                    .unwrap(),
            );
        })
    });

    group.bench_function("parse_generic_format", |b| {
        b.iter(|| {
            black_box(
                SipInfoDtmfMessage::from_body_content("application/dtmf", &generic_body).unwrap(),
            );
        })
    });

    // Benchmark full processing
    group.bench_function("process_incoming", |b| {
        b.to_async(&rt).iter(|| async {
            let processor_clone = Arc::clone(&processor);
            black_box(
                processor_clone
                    .process_incoming_info(
                        "bench_session",
                        "bench_call",
                        "from_tag",
                        "to_tag",
                        "application/dtmf-relay",
                        &cisco_body,
                    )
                    .await
                    .unwrap(),
            );
        })
    });

    // Benchmark outgoing generation
    group.bench_function("generate_outgoing", |b| {
        b.to_async(&rt).iter(|| async {
            let processor_clone = Arc::clone(&processor);
            black_box(
                processor_clone
                    .generate_outgoing_info("bench_session", '5', Some(120), Some(60))
                    .await
                    .unwrap(),
            );
        })
    });

    group.finish();
}

/// Benchmark Sigtran processing
fn bench_sigtran_processing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("sigtran_processing");

    let (event_sender, _receiver) = mpsc::unbounded_channel();
    let config = SigtranDtmfConfig::default();
    let processor = Arc::new(SigtranDtmfProcessor::new(event_sender, config));

    // Pre-generate test messages
    let test_messages = vec![
        SigtranDtmfMessage {
            protocol: SigtranProtocol::M3ua,
            message_type: SigtranDtmfMessageType::IsupGenericDigits,
            digits: "12345".to_string(),
            encoding: GenericDigitsEncoding::Ia5Character,
            cic: Some(100),
            transaction_id: None,
            parameters: std::collections::HashMap::new(),
        },
        SigtranDtmfMessage {
            protocol: SigtranProtocol::M3ua,
            message_type: SigtranDtmfMessageType::IsupUserToUser,
            digits: "*67890#".to_string(),
            encoding: GenericDigitsEncoding::BcdEven,
            cic: Some(200),
            transaction_id: None,
            parameters: std::collections::HashMap::new(),
        },
    ];

    // Benchmark message processing
    for (i, message) in test_messages.iter().enumerate() {
        group.bench_with_input(
            BenchmarkId::new("process_message", i),
            message,
            |b, message| {
                b.to_async(&rt).iter(|| async {
                    let processor_clone = Arc::clone(&processor);
                    black_box(
                        processor_clone
                            .process_incoming_message(message.clone())
                            .await
                            .unwrap(),
                    );
                })
            },
        );
    }

    // Benchmark ISUP parameter creation
    group.bench_function("create_isup_parameter", |b| {
        b.iter(|| {
            black_box(
                processor
                    .create_isup_generic_digits(
                        "12345",
                        redfire_switch::sigtran_dtmf::GenericDigitsType::DtmfDigits,
                    )
                    .unwrap(),
            );
        })
    });

    // Benchmark BCD encoding/decoding
    group.bench_function("bcd_encode", |b| {
        b.iter(|| {
            black_box(
                processor
                    .encode_digits("1234567890*#ABC", GenericDigitsEncoding::BcdEven)
                    .unwrap(),
            );
        })
    });

    group.bench_function("bcd_decode", |b| {
        let encoded = processor
            .encode_digits("1234567890*#ABC", GenericDigitsEncoding::BcdEven)
            .unwrap();
        let encoded_str = String::from_utf8_lossy(&encoded);
        b.iter(|| {
            black_box(
                processor
                    .decode_digits(&encoded_str, GenericDigitsEncoding::BcdEven)
                    .unwrap(),
            );
        })
    });

    group.finish();
}

/// Benchmark concurrent processing
fn bench_concurrent_processing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("concurrent_processing");
    group.sample_size(10);

    // Setup processors
    let (event_sender, _receiver) = mpsc::unbounded_channel();
    let rfc2833_processor = Arc::new({
        let mut p = Rfc2833Processor::new(event_sender.clone());
        p.add_payload_type(101, Rfc2833PayloadType::TelephoneEvent(101));
        p
    });
    let sip_info_processor = Arc::new(SipInfoDtmfProcessor::new(event_sender.clone()));
    let sigtran_processor = Arc::new(SigtranDtmfProcessor::new(
        event_sender,
        SigtranDtmfConfig::default(),
    ));

    // Benchmark concurrent sessions
    for &num_sessions in &[1, 5, 10, 25, 50] {
        group.bench_with_input(
            BenchmarkId::new("concurrent_rfc2833", num_sessions),
            &num_sessions,
            |b, &num_sessions| {
                b.to_async(&rt).iter(|| async {
                    let processor = Arc::clone(&rfc2833_processor);
                    let mut handles = Vec::new();

                    for i in 0..num_sessions {
                        let processor_clone = Arc::clone(&processor);
                        let handle = tokio::spawn(async move {
                            let event = Rfc2833Event::new(Rfc2833EventId::Dtmf5, 10, 800);
                            let bytes = event.to_bytes().unwrap();
                            let session_id = format!("session_{}", i);
                            processor_clone
                                .process_incoming_packet(&session_id, 101, 1000, &bytes)
                                .await
                                .unwrap();

                            let end_event =
                                Rfc2833Event::end_event(Rfc2833EventId::Dtmf5, 10, 1200);
                            let end_bytes = end_event.to_bytes().unwrap();
                            processor_clone
                                .process_incoming_packet(&session_id, 101, 1500, &end_bytes)
                                .await
                                .unwrap();
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        black_box(handle.await.unwrap());
                    }
                })
            },
        );
    }

    group.finish();
}

/// Benchmark memory usage patterns
fn bench_memory_usage(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("memory_usage");
    group.sample_size(10);

    // Test session scaling
    let (event_sender, _receiver) = mpsc::unbounded_channel();
    let processor = Arc::new(SipInfoDtmfProcessor::new(event_sender));

    for &num_sessions in &[100, 500, 1000, 2000] {
        group.bench_with_input(
            BenchmarkId::new("session_scaling", num_sessions),
            &num_sessions,
            |b, &num_sessions| {
                b.to_async(&rt).iter(|| async {
                    let processor_clone = Arc::clone(&processor);

                    for i in 0..num_sessions {
                        let session_id = format!("session_{}", i);
                        let call_id = format!("call_{}", i);

                        black_box(
                            processor_clone
                                .process_incoming_info(
                                    &session_id,
                                    &call_id,
                                    "from_tag",
                                    "to_tag",
                                    "application/dtmf-relay",
                                    "Signal=5\r\nDuration=100\r\n",
                                )
                                .await
                                .unwrap(),
                        );
                    }
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_dtmf_generation,
    bench_dtmf_detection,
    bench_rfc2833_processing,
    bench_sip_info_processing,
    bench_sigtran_processing,
    bench_concurrent_processing,
    bench_memory_usage
);

criterion_main!(benches);
