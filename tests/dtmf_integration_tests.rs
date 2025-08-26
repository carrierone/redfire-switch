/*
 * Comprehensive DTMF Integration Test Suite
 *
 * This test suite validates the complete DTMF functionality across all implemented
 * transport protocols and integration points:
 * - Core DTMF processor (detection and generation)
 * - RFC2833 RTP events
 * - SIP INFO method
 * - Sigtran protocols
 * - STIR/SHAKEN TDM support
 * - Cross-protocol compatibility
 * - Performance and reliability
 */

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::{mpsc, RwLock};
use tokio::time::timeout;

// Import all DTMF-related modules
use redfire_switch::dtmf_processor::{
    DtmfDetector, DtmfDetectorConfig, DtmfEvent, DtmfGenerator, DtmfGeneratorConfig, DtmfProcessor,
    DtmfSource,
};
use redfire_switch::rfc2833_events::{
    Rfc2833Event, Rfc2833EventId, Rfc2833PayloadType, Rfc2833Processor, Rfc2833SdpConfig,
    Rfc2833SdpNegotiator,
};
use redfire_switch::sigtran_dtmf::{
    GenericDigitsEncoding, SigtranDtmfConfig, SigtranDtmfMessage, SigtranDtmfMessageType,
    SigtranDtmfProcessor, SigtranProtocol,
};
use redfire_switch::sip_info_dtmf::{
    SipInfoDtmfContentType, SipInfoDtmfMessage, SipInfoDtmfProcessor, SipInfoPackageNegotiator,
    SipInfoResponse,
};
use redfire_switch::stir_shaken_tdm::{
    AttestationLevel, StirShakenTdmConfig, StirShakenTdmMessage, StirShakenTdmProcessor,
    StirShakenTransport, VerificationStatus,
};

/// Test suite for comprehensive DTMF functionality
struct DtmfIntegrationTestSuite {
    dtmf_processor: DtmfProcessor,
    rfc2833_processor: Rfc2833Processor,
    sip_info_processor: SipInfoDtmfProcessor,
    sigtran_processor: SigtranDtmfProcessor,
    stir_shaken_processor: StirShakenTdmProcessor,
    event_receiver: mpsc::UnboundedReceiver<DtmfEvent>,
}

impl DtmfIntegrationTestSuite {
    /// Create new test suite with all processors
    async fn new() -> Result<Self> {
        let (dtmf_event_sender, dtmf_event_receiver) = mpsc::unbounded_channel();

        // Create DTMF processor
        let dtmf_processor = DtmfProcessor::new();

        // Create RFC2833 processor
        let rfc2833_processor = Rfc2833Processor::new(dtmf_event_sender.clone());

        // Create SIP INFO processor
        let sip_info_processor = SipInfoDtmfProcessor::new(dtmf_event_sender.clone());

        // Create Sigtran processor
        let sigtran_config = SigtranDtmfConfig::default();
        let sigtran_processor =
            SigtranDtmfProcessor::new(dtmf_event_sender.clone(), sigtran_config);

        // Create STIR/SHAKEN processor
        let stir_shaken_config = StirShakenTdmConfig::default();
        let (stir_shaken_processor, _stir_shaken_receiver) =
            StirShakenTdmProcessor::new(stir_shaken_config).await?;

        Ok(Self {
            dtmf_processor,
            rfc2833_processor,
            sip_info_processor,
            sigtran_processor,
            stir_shaken_processor,
            event_receiver: dtmf_event_receiver,
        })
    }
}

#[tokio::test]
async fn test_core_dtmf_processor_functionality() -> Result<()> {
    let suite = DtmfIntegrationTestSuite::new().await?;
    let detector = suite.dtmf_processor.detector();
    let generator = suite.dtmf_processor.generator();

    // Test channel management
    detector.add_channel("test_channel".to_string()).await?;

    // Test DTMF generation
    let test_digits = "123*0#ABC";
    for digit in test_digits.chars() {
        let samples =
            generator.generate_digit(digit, Some(Duration::from_millis(100)), Some(0.5))?;
        assert!(
            !samples.is_empty(),
            "Generated samples should not be empty for digit '{}'",
            digit
        );
        assert!(
            samples.len() >= 800,
            "Should generate at least 800 samples (100ms at 8kHz) for digit '{}'",
            digit
        ); // 100ms at 8kHz
    }

    // Test sequence generation
    let sequence_samples = generator.generate_sequence("12345", None, None, None)?;
    assert!(
        sequence_samples.len() > 4000,
        "Sequence samples should be substantial"
    ); // Multiple digits + silence

    // Test detection statistics
    let stats = detector.get_statistics("test_channel").await?;
    assert_eq!(stats.channel_id, "test_channel");
    assert_eq!(stats.current_digit, None);
    assert_eq!(stats.current_sequence, "");

    Ok(())
}

#[tokio::test]
async fn test_rfc2833_event_processing() -> Result<()> {
    let mut suite = DtmfIntegrationTestSuite::new().await?;

    // Add RFC2833 payload type
    suite
        .rfc2833_processor
        .add_payload_type(101, Rfc2833PayloadType::TelephoneEvent(101));

    // Test event packet creation and processing
    let test_digits = ['1', '5', '9', '*', '#', 'A'];

    for &digit in &test_digits {
        // Create RFC2833 event
        let event_id = Rfc2833EventId::from_dtmf_char(digit).unwrap();
        let start_event = Rfc2833Event::new(event_id, 10, 800);
        let end_event = Rfc2833Event::end_event(event_id, 10, 1600);

        // Test serialization/deserialization
        let start_bytes = start_event.to_bytes()?;
        let parsed_start = Rfc2833Event::from_bytes(&start_bytes)?;
        assert_eq!(parsed_start.event_id as u8, start_event.event_id as u8);
        assert_eq!(parsed_start.volume, start_event.volume);
        assert!(!parsed_start.end_of_event);

        // Process events
        let session_id = format!("rfc2833_session_{}", digit);
        suite
            .rfc2833_processor
            .process_incoming_packet(&session_id, 101, 1000, &start_bytes)
            .await?;

        let end_bytes = end_event.to_bytes()?;
        suite
            .rfc2833_processor
            .process_incoming_packet(&session_id, 101, 2000, &end_bytes)
            .await?;

        // Verify DTMF event was generated
        let received_event = timeout(Duration::from_millis(100), suite.event_receiver.recv())
            .await
            .map_err(|_| anyhow::anyhow!("Timeout waiting for event"))?
            .ok_or_else(|| anyhow::anyhow!("Channel closed unexpectedly"))?;
        match received_event {
            DtmfEvent::DigitDetected {
                digit: detected_digit,
                source,
                ..
            } => {
                assert_eq!(detected_digit, digit);
                assert_eq!(source, DtmfSource::Rfc2833);
            }
            _ => assert!(false, "Expected DigitDetected event for digit '{}', got: {:?}", digit, event),
        }
    }

    // Test packet generation
    let packets = suite
        .rfc2833_processor
        .generate_outgoing_packets("test_session", '7', 200, 20, 5000)
        .await?;
    assert!(!packets.is_empty(), "Should generate RFC2833 packets");
    assert!(
        packets.len() >= 3,
        "Should include end-of-event repetitions"
    );

    Ok(())
}

#[tokio::test]
async fn test_sip_info_dtmf_processing() -> Result<()> {
    let mut suite = DtmfIntegrationTestSuite::new().await?;

    // Test different content types
    let content_types = [
        (
            "application/dtmf-relay",
            SipInfoDtmfContentType::CiscoDtmfRelay,
        ),
        ("application/dtmf", SipInfoDtmfContentType::GenericDtmf),
        (
            "application/vnd.nortel.text",
            SipInfoDtmfContentType::NortelText,
        ),
    ];

    for (mime_type, content_type) in &content_types {
        let test_digit = '8';
        let message = SipInfoDtmfMessage::new(test_digit, content_type.clone())
            .with_duration(150)
            .with_volume(75);

        let body_content = message.to_body_content();
        assert!(!body_content.is_empty(), "Body content should not be empty");

        // Test round-trip parsing
        let parsed_message = SipInfoDtmfMessage::from_body_content(mime_type, &body_content)?;
        assert_eq!(parsed_message.digit, test_digit);

        // Test processing through processor
        let response = suite
            .sip_info_processor
            .process_incoming_info(
                "sip_session_123",
                "call_id_456",
                "from_tag",
                "to_tag",
                mime_type,
                &body_content,
            )
            .await?;

        assert_eq!(response, SipInfoResponse::Ok);

        // Verify event was generated
        let received_event = timeout(Duration::from_millis(100), suite.event_receiver.recv())
            .await
            .map_err(|_| anyhow::anyhow!("Timeout waiting for event"))?
            .ok_or_else(|| anyhow::anyhow!("Channel closed unexpectedly"))?;
        match received_event {
            DtmfEvent::DigitDetected {
                digit: detected_digit,
                source,
                ..
            } => {
                assert_eq!(detected_digit, test_digit);
                assert_eq!(source, DtmfSource::SipInfo);
            }
            _ => assert!(false, "Expected DigitDetected event for SIP INFO, got: {:?}", event),
        }
    }

    // Test outgoing message generation
    let outgoing_request = suite
        .sip_info_processor
        .generate_outgoing_info("test_session", '3', Some(120), Some(60))
        .await?;

    assert_eq!(outgoing_request.method, "INFO");
    assert!(!outgoing_request.body.is_empty());
    assert!(!outgoing_request.content_type.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_sigtran_dtmf_processing() -> Result<()> {
    let mut suite = DtmfIntegrationTestSuite::new().await?;

    // Test ISUP Generic Digits processing
    let isup_message = SigtranDtmfMessage {
        protocol: SigtranProtocol::M3ua,
        message_type: SigtranDtmfMessageType::IsupGenericDigits,
        digits: "456789".to_string(),
        encoding: GenericDigitsEncoding::Ia5Character,
        cic: Some(100),
        transaction_id: None,
        parameters: HashMap::new(),
    };

    suite
        .sigtran_processor
        .process_incoming_message(isup_message)
        .await?;

    // Should receive events for each digit in sequence
    let expected_digits = ['4', '5', '6', '7', '8', '9'];
    for &expected_digit in &expected_digits {
        let received_event = timeout(Duration::from_millis(200), suite.event_receiver.recv())
            .await
            .map_err(|_| anyhow::anyhow!("Timeout waiting for event"))?
            .ok_or_else(|| anyhow::anyhow!("Channel closed unexpectedly"))?;
        match received_event {
            DtmfEvent::DigitDetected { digit, source, .. } => {
                assert_eq!(digit, expected_digit);
                assert_eq!(source, DtmfSource::Sigtran);
            }
            _ => assert!(false, "Expected DigitDetected event for digit '{}', got: {:?}", expected_digit, event),
        }
    }

    // Should also receive sequence complete event
    let sequence_event = timeout(Duration::from_millis(100), suite.event_receiver.recv())
        .await
        .map_err(|_| anyhow::anyhow!("Timeout waiting for event"))?
        .ok_or_else(|| anyhow::anyhow!("Channel closed unexpectedly"))?;
    match sequence_event {
        DtmfEvent::SequenceComplete {
            sequence, source, ..
        } => {
            assert_eq!(sequence, "456789");
            assert_eq!(source, DtmfSource::Sigtran);
        }
        _ => assert!(false, "Expected SequenceComplete event, got: {:?}", seq_event),
    }

    // Test TCAP transaction management
    let transaction_id = suite
        .sigtran_processor
        .start_digit_collection(SigtranProtocol::M3ua, Some(200))
        .await;
    assert!(transaction_id > 0, "Transaction ID should be positive");

    let stats = suite.sigtran_processor.get_transaction_stats().await;
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].transaction_id, transaction_id);
    assert_eq!(stats[0].cic, Some(200));

    Ok(())
}

#[tokio::test]
async fn test_stir_shaken_tdm_processing() -> Result<()> {
    let suite = DtmfIntegrationTestSuite::new().await?;

    // Test message creation and processing
    let calling_number = "+15551234567";
    let called_number = "+15559876543";
    let call_id = "stir_shaken_test_call";

    // Create test message (without actual signing for test)
    let test_message = StirShakenTdmMessage {
        transport: StirShakenTransport::InBandIsup,
        calling_number: calling_number.to_string(),
        called_number: called_number.to_string(),
        passport_token: "test.jwt.token".to_string(), // Simplified for testing
        attestation_level: AttestationLevel::FullAttestation,
        verification_status: VerificationStatus::NoValidation,
        cic: Some(300),
        call_id: call_id.to_string(),
        parameters: HashMap::new(),
        timestamp: SystemTime::now(),
    };

    // Test ISUP UUI encoding/decoding
    let uui_data = suite
        .stir_shaken_processor
        .encode_for_isup_uui(&test_message)?;
    assert!(!uui_data.is_empty(), "UUI data should not be empty");
    assert_eq!(uui_data[0], 0x04); // Protocol discriminator
    assert_eq!(&uui_data[1..5], b"STIR"); // STIR indicator
    assert_eq!(uui_data[5], b'A'); // Attestation level

    // Test round-trip encoding/decoding
    let decoded_message = suite
        .stir_shaken_processor
        .decode_from_isup_uui(&uui_data)?;
    assert_eq!(
        decoded_message.attestation_level,
        AttestationLevel::FullAttestation
    );
    assert_eq!(decoded_message.transport, StirShakenTransport::InBandIsup);

    // Test statistics
    let stats = suite.stir_shaken_processor.get_statistics().await;
    assert_eq!(stats.enabled, true);
    assert_eq!(stats.cached_certificates, 0);
    assert!(!stats.supported_transports.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_cross_protocol_compatibility() -> Result<()> {
    let mut suite = DtmfIntegrationTestSuite::new().await?;

    // Test the same DTMF digit '7' across all protocols
    let test_digit = '7';
    let test_session = "cross_protocol_test";

    // 1. RFC2833 Event
    suite
        .rfc2833_processor
        .add_payload_type(101, Rfc2833PayloadType::TelephoneEvent(101));
    let rfc2833_event = Rfc2833Event::new(Rfc2833EventId::Dtmf7, 15, 1200);
    let rfc2833_bytes = rfc2833_event.to_bytes()?;
    suite
        .rfc2833_processor
        .process_incoming_packet(test_session, 101, 1000, &rfc2833_bytes)
        .await?;

    let end_event = Rfc2833Event::end_event(Rfc2833EventId::Dtmf7, 15, 1600);
    let end_bytes = end_event.to_bytes()?;
    suite
        .rfc2833_processor
        .process_incoming_packet(test_session, 101, 2000, &end_bytes)
        .await?;

    // 2. SIP INFO
    let sip_message = SipInfoDtmfMessage::new(test_digit, SipInfoDtmfContentType::CiscoDtmfRelay)
        .with_duration(160)
        .with_volume(70);
    let sip_body = sip_message.to_body_content();

    suite
        .sip_info_processor
        .process_incoming_info(
            test_session,
            "cross_test_call",
            "from_tag",
            "to_tag",
            "application/dtmf-relay",
            &sip_body,
        )
        .await?;

    // 3. Sigtran
    let sigtran_message = SigtranDtmfMessage {
        protocol: SigtranProtocol::M3ua,
        message_type: SigtranDtmfMessageType::IsupGenericDigits,
        digits: test_digit.to_string(),
        encoding: GenericDigitsEncoding::Ia5Character,
        cic: Some(400),
        transaction_id: None,
        parameters: HashMap::new(),
    };

    suite
        .sigtran_processor
        .process_incoming_message(sigtran_message)
        .await?;

    // Verify we received 3 DTMF events for digit '7' from different sources
    let mut event_count = 0;
    let mut sources_seen = Vec::new();

    while event_count < 3 {
        match timeout(Duration::from_millis(500), suite.event_receiver.recv()).await {
            Ok(Some(DtmfEvent::DigitDetected { digit, source, .. })) => {
                assert_eq!(digit, test_digit);
                sources_seen.push(source);
                event_count += 1;
            }
            Ok(Some(DtmfEvent::SequenceComplete { .. })) => {
                // Skip sequence complete events
                continue;
            }
            Ok(Some(other)) => {
                assert!(false, "Unexpected event type: {:?}", other);
            }
            Ok(None) => {
                assert!(false, "Event channel closed unexpectedly");
            }
            Err(_) => {
                assert!(false, "Timeout waiting for DTMF event {}", event_count + 1);;
            }
        }
    }

    // Verify all three sources were seen
    assert!(
        sources_seen.contains(&DtmfSource::Rfc2833),
        "RFC2833 source not seen"
    );
    assert!(
        sources_seen.contains(&DtmfSource::SipInfo),
        "SIP INFO source not seen"
    );
    assert!(
        sources_seen.contains(&DtmfSource::Sigtran),
        "Sigtran source not seen"
    );

    Ok(())
}

#[tokio::test]
async fn test_performance_and_reliability() -> Result<()> {
    let mut suite = DtmfIntegrationTestSuite::new().await?;

    // Performance test: Process large number of DTMF events quickly
    let start_time = Instant::now();
    let test_sequence = "123456789*0#ABCD";
    let iterations = 100;

    for i in 0..iterations {
        for digit in test_sequence.chars() {
            // RFC2833 processing
            if let Some(event_id) = Rfc2833EventId::from_dtmf_char(digit) {
                let event = Rfc2833Event::new(event_id, 10, 800);
                let bytes = event.to_bytes()?;
                let session_id = format!("perf_test_{}", i);
                suite
                    .rfc2833_processor
                    .process_incoming_packet(&session_id, 101, 1000, &bytes)
                    .await?;

                let end_event = Rfc2833Event::end_event(event_id, 10, 1200);
                let end_bytes = end_event.to_bytes()?;
                suite
                    .rfc2833_processor
                    .process_incoming_packet(&session_id, 101, 1500, &end_bytes)
                    .await?;
            }
        }
    }

    let processing_time = start_time.elapsed();
    let events_processed = test_sequence.len() * iterations;
    let events_per_second = events_processed as f64 / processing_time.as_secs_f64();

    println!(
        "Performance: Processed {} DTMF events in {:?} ({:.2} events/sec)",
        events_processed, processing_time, events_per_second
    );

    // Should be able to process at least 1000 events per second
    assert!(
        events_per_second > 1000.0,
        "Performance below threshold: {:.2} events/sec",
        events_per_second
    );

    // Reliability test: Verify no events are lost
    let mut received_count = 0;
    while let Ok(Some(_)) = timeout(Duration::from_millis(10), suite.event_receiver.recv()).await {
        received_count += 1;
    }

    // Should receive events for all processed digits
    assert!(
        received_count >= events_processed,
        "Event loss detected: {} received vs {} expected",
        received_count,
        events_processed
    );

    Ok(())
}

#[tokio::test]
async fn test_sdp_negotiation() -> Result<()> {
    // Test RFC2833 SDP negotiation
    let negotiator = Rfc2833SdpNegotiator::new();

    // Generate SDP attributes
    let attributes = negotiator.generate_sdp_attributes();
    assert!(!attributes.is_empty(), "SDP attributes should not be empty");

    let rtpmap_found = attributes
        .iter()
        .any(|attr| attr.contains("rtpmap") && attr.contains("telephone-event"));
    assert!(rtpmap_found, "Should contain rtpmap for telephone-event");

    let fmtp_found = attributes
        .iter()
        .any(|attr| attr.contains("fmtp") && attr.contains("0-15"));
    assert!(fmtp_found, "Should contain fmtp with DTMF event range");

    // Test parsing
    let test_sdp = ["a=rtpmap:101 telephone-event/8000", "a=fmtp:101 0-15,32-35"];

    let config = negotiator.parse_sdp_attributes(&test_sdp)?;
    assert_eq!(config.payload_type, 101);
    assert_eq!(config.clock_rate, 8000);
    assert!(config.supported_events.contains(&0)); // DTMF 0
    assert!(config.supported_events.contains(&15)); // DTMF D
    assert!(config.supported_events.contains(&32)); // Dial tone

    Ok(())
}

#[tokio::test]
async fn test_error_handling_and_edge_cases() -> Result<()> {
    let mut suite = DtmfIntegrationTestSuite::new().await?;

    // Test invalid RFC2833 packets
    let invalid_packet = vec![0xFF, 0xFF, 0xFF]; // Too short
    let result = suite
        .rfc2833_processor
        .process_incoming_packet("test", 101, 1000, &invalid_packet)
        .await;
    assert!(result.is_err(), "Should reject invalid RFC2833 packet");

    // Test invalid SIP INFO content
    let invalid_sip_body = "InvalidFormat=NoSignal";
    let result = suite
        .sip_info_processor
        .process_incoming_info(
            "test",
            "call",
            "from",
            "to",
            "application/dtmf-relay",
            invalid_sip_body,
        )
        .await;
    assert!(result.is_err(), "Should reject invalid SIP INFO body");

    // Test unsupported content type
    let result = suite
        .sip_info_processor
        .process_incoming_info(
            "test",
            "call",
            "from",
            "to",
            "application/unknown",
            "Signal=5",
        )
        .await?;
    assert_eq!(result, SipInfoResponse::UnsupportedMediaType);

    // Test empty/invalid digit sequences
    let empty_sigtran = SigtranDtmfMessage {
        protocol: SigtranProtocol::M3ua,
        message_type: SigtranDtmfMessageType::IsupGenericDigits,
        digits: "".to_string(), // Empty
        encoding: GenericDigitsEncoding::Ia5Character,
        cic: None,
        transaction_id: None,
        parameters: HashMap::new(),
    };

    // Should handle gracefully without crashing
    let result = suite
        .sigtran_processor
        .process_incoming_message(empty_sigtran)
        .await;
    assert!(
        result.is_ok(),
        "Should handle empty digit sequence gracefully"
    );

    Ok(())
}

/// Integration test for real-world telephony scenarios
#[tokio::test]
async fn test_realistic_telephony_scenarios() -> Result<()> {
    let mut suite = DtmfIntegrationTestSuite::new().await?;

    // Scenario 1: IVR system with mixed DTMF inputs
    // Simulate caller entering account number via different protocols
    let account_number = "1234567890";

    // First digits via RFC2833 (SIP call)
    for (i, digit) in account_number[0..5].chars().enumerate() {
        if let Some(event_id) = Rfc2833EventId::from_dtmf_char(digit) {
            let event = Rfc2833Event::new(event_id, 15, 800);
            let bytes = event.to_bytes()?;
            suite
                .rfc2833_processor
                .process_incoming_packet("ivr_session", 101, 1000 + i as u32 * 1000, &bytes)
                .await?;

            let end_event = Rfc2833Event::end_event(event_id, 15, 1200);
            let end_bytes = end_event.to_bytes()?;
            suite
                .rfc2833_processor
                .process_incoming_packet("ivr_session", 101, 1500 + i as u32 * 1000, &end_bytes)
                .await?;
        }
    }

    // Remaining digits via Sigtran (PSTN gateway)
    let remaining_digits = &account_number[5..];
    let sigtran_message = SigtranDtmfMessage {
        protocol: SigtranProtocol::M3ua,
        message_type: SigtranDtmfMessageType::IsupGenericDigits,
        digits: remaining_digits.to_string(),
        encoding: GenericDigitsEncoding::Ia5Character,
        cic: Some(500),
        transaction_id: None,
        parameters: HashMap::new(),
    };

    suite
        .sigtran_processor
        .process_incoming_message(sigtran_message)
        .await?;

    // Collect all received digits
    let mut received_digits = String::new();
    let mut event_count = 0;
    let expected_events = account_number.len();

    while event_count < expected_events {
        match timeout(Duration::from_millis(1000), suite.event_receiver.recv()).await {
            Ok(Some(DtmfEvent::DigitDetected { digit, .. })) => {
                received_digits.push(digit);
                event_count += 1;
            }
            Ok(Some(DtmfEvent::SequenceComplete { .. })) => {
                // Skip sequence complete events
                continue;
            }
            Ok(Some(_)) => continue,
            Ok(None) => break,
            Err(_) => {
                println!(
                    "Timeout after receiving {} of {} digits: '{}'",
                    event_count, expected_events, received_digits
                );
                break;
            }
        }
    }

    assert_eq!(
        received_digits, account_number,
        "Should reconstruct complete account number"
    );

    // Scenario 2: Call authentication with STIR/SHAKEN
    let _auth_message = StirShakenTdmMessage {
        transport: StirShakenTransport::OutOfBandSip,
        calling_number: "+15551234567".to_string(),
        called_number: "+15559876543".to_string(),
        passport_token: "eyJhbGciOiJFUzI1NiIsInR5cCI6IkpXVCJ9.test.signature".to_string(), // Mock JWT
        attestation_level: AttestationLevel::PartialAttestation,
        verification_status: VerificationStatus::NoValidation,
        cic: None,
        call_id: "auth_test_call".to_string(),
        parameters: HashMap::new(),
        timestamp: SystemTime::now(),
    };

    // Process without actual verification (would need proper certificates in production)
    let stats = suite.stir_shaken_processor.get_statistics().await;
    assert!(stats.enabled, "STIR/SHAKEN should be enabled");

    println!("Telephony scenario tests completed successfully");
    Ok(())
}
