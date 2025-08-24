/*
 * DTMF Functionality Showcase
 *
 * This example demonstrates the complete DTMF implementation across all
 * supported transport protocols:
 * - Core DTMF detection and generation using Goertzel algorithm
 * - RFC2833 RTP event processing with SDP negotiation
 * - SIP INFO method with multiple content type support
 * - Sigtran protocols (ISUP, TCAP, INAP) for SS7/telephony networks
 * - STIR/SHAKEN TDM support for secure caller ID verification
 *
 * Usage: cargo run --example dtmf_showcase
 */

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{mpsc, RwLock};
use tokio::time::{sleep, timeout};
use tracing::{debug, info, warn, Level};
use tracing_subscriber;

use redfire_switch::dtmf_processor::{
    DtmfDetectorConfig, DtmfEvent, DtmfGeneratorConfig, DtmfProcessor, DtmfSource,
};
use redfire_switch::rfc2833_events::{
    Rfc2833Event, Rfc2833EventId, Rfc2833PayloadType, Rfc2833Processor, Rfc2833SdpNegotiator,
};
use redfire_switch::sigtran_dtmf::{
    GenericDigitsEncoding, SigtranDtmfConfig, SigtranDtmfMessage, SigtranDtmfMessageType,
    SigtranDtmfProcessor, SigtranProtocol,
};
use redfire_switch::sip_info_dtmf::{
    SipInfoDtmfContentType, SipInfoDtmfMessage, SipInfoDtmfProcessor, SipInfoPackageNegotiator,
};
use redfire_switch::stir_shaken_tdm::{
    AttestationLevel, StirShakenTdmConfig, StirShakenTdmMessage, StirShakenTdmProcessor,
    StirShakenTransport, VerificationStatus,
};

/// Main DTMF showcase orchestrator
struct DtmfShowcase {
    dtmf_processor: DtmfProcessor,
    rfc2833_processor: Rfc2833Processor,
    sip_info_processor: SipInfoDtmfProcessor,
    sigtran_processor: SigtranDtmfProcessor,
    stir_shaken_processor: StirShakenTdmProcessor,
    event_receiver: mpsc::UnboundedReceiver<DtmfEvent>,
    event_log: Arc<RwLock<Vec<DtmfEvent>>>,
}

impl DtmfShowcase {
    async fn new() -> Result<Self> {
        // Setup shared event channel
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        // Create DTMF processor with custom configuration
        let detector_config = DtmfDetectorConfig {
            sample_rate: 8000,
            min_tone_duration: 40,
            confidence_threshold: 0.75,
            enable_extended: true,
            ..Default::default()
        };
        let generator_config = DtmfGeneratorConfig {
            sample_rate: 8000,
            default_tone_duration: 100,
            enable_shaping: true,
            ..Default::default()
        };
        let dtmf_processor = DtmfProcessor::with_config(detector_config, generator_config);

        // Create RFC2833 processor
        let mut rfc2833_processor = Rfc2833Processor::new(event_sender.clone());
        rfc2833_processor.add_payload_type(101, Rfc2833PayloadType::TelephoneEvent(101));
        rfc2833_processor.add_payload_type(102, Rfc2833PayloadType::TelephoneEvent(102));

        // Create SIP INFO processor
        let mut sip_info_processor = SipInfoDtmfProcessor::new(event_sender.clone());
        sip_info_processor.set_preferred_content_type(SipInfoDtmfContentType::CiscoDtmfRelay);
        sip_info_processor.add_supported_content_type(SipInfoDtmfContentType::GenericDtmf);
        sip_info_processor.add_supported_content_type(SipInfoDtmfContentType::NortelText);

        // Create Sigtran processor
        let sigtran_config = SigtranDtmfConfig {
            max_digits: 20,
            collection_timeout: 30,
            supported_protocols: vec![SigtranProtocol::M3ua, SigtranProtocol::Sua],
            default_encoding: GenericDigitsEncoding::Ia5Character,
        };
        let sigtran_processor = SigtranDtmfProcessor::new(event_sender.clone(), sigtran_config);

        // Create STIR/SHAKEN processor
        let stir_shaken_config = StirShakenTdmConfig {
            enabled: true,
            supported_transports: vec![
                StirShakenTransport::OutOfBandSip,
                StirShakenTransport::InBandIsup,
                StirShakenTransport::SigtranSignaling,
            ],
            default_attestation_level: AttestationLevel::PartialAttestation,
            require_verification: false,
            ..Default::default()
        };
        let (stir_shaken_processor, _stir_shaken_receiver) =
            StirShakenTdmProcessor::new(stir_shaken_config).await?;

        Ok(Self {
            dtmf_processor,
            rfc2833_processor,
            sip_info_processor,
            sigtran_processor,
            stir_shaken_processor,
            event_receiver,
            event_log: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Demonstrate core DTMF functionality
    async fn demonstrate_core_dtmf(&self) -> Result<()> {
        println!("\n🎵 === Core DTMF Processing Demonstration ===");

        let detector = self.dtmf_processor.detector();
        let generator = self.dtmf_processor.generator();

        // Add test channel
        detector.add_channel("showcase_channel".to_string()).await?;

        // Generate and detect DTMF sequence
        let test_sequence = "1234567890*#ABCD";
        println!("📞 Testing DTMF sequence: {}", test_sequence);

        for digit in test_sequence.chars() {
            // Generate DTMF tone
            let samples =
                generator.generate_digit(digit, Some(Duration::from_millis(150)), Some(0.7))?;
            println!(
                "  🔊 Generated {} samples for digit '{}'",
                samples.len(),
                digit
            );

            // Process for detection (simulated)
            detector
                .process_audio("showcase_channel", &samples, DtmfSource::Internal)
                .await?;

            // Brief pause between digits
            sleep(Duration::from_millis(10)).await;
        }

        // Generate complete sequence
        let sequence_samples = generator.generate_sequence(
            "123*456#",
            Some(Duration::from_millis(100)),
            Some(Duration::from_millis(50)),
            Some(0.6),
        )?;
        println!(
            "🎼 Generated sequence with {} total samples",
            sequence_samples.len()
        );

        // Get detection statistics
        let stats = detector.get_statistics("showcase_channel").await?;
        println!(
            "📊 Detection stats - Channel: {}, Active: {}, Sequence: '{}'",
            stats.channel_id, stats.detection_active, stats.current_sequence
        );

        Ok(())
    }

    /// Demonstrate RFC2833 RTP event processing
    async fn demonstrate_rfc2833(&mut self) -> Result<()> {
        println!("\n📡 === RFC2833 RTP Event Processing ===");

        // SDP negotiation demonstration
        let sdp_negotiator = Rfc2833SdpNegotiator::new();
        let sdp_attributes = sdp_negotiator.generate_sdp_attributes();
        println!("📋 Generated SDP attributes:");
        for attr in &sdp_attributes {
            println!("  {}", attr);
        }

        // Process incoming RFC2833 events
        let test_digits = ['1', '5', '9', '*', '#', 'A'];
        println!("🔢 Processing RFC2833 events for digits: {:?}", test_digits);

        for &digit in &test_digits {
            let event_id = Rfc2833EventId::from_dtmf_char(digit).unwrap();
            let session_id = format!("rfc2833_session_{}", digit);

            // Start event
            let start_event = Rfc2833Event::new(event_id, 12, 800);
            let start_bytes = start_event.to_bytes()?;
            self.rfc2833_processor
                .process_incoming_packet(&session_id, 101, 1000, &start_bytes)
                .await?;

            // End event
            let end_event = Rfc2833Event::end_event(event_id, 12, 1600);
            let end_bytes = end_event.to_bytes()?;
            self.rfc2833_processor
                .process_incoming_packet(&session_id, 101, 2000, &end_bytes)
                .await?;

            println!("  ✅ Processed RFC2833 event for digit '{}'", digit);
        }

        // Generate outgoing RFC2833 packets
        let outgoing_packets = self
            .rfc2833_processor
            .generate_outgoing_packets("outgoing_session", '7', 200, 15, 5000)
            .await?;
        println!(
            "📤 Generated {} RFC2833 packets for outgoing digit '7'",
            outgoing_packets.len()
        );

        // Show active events
        let active_events = self.rfc2833_processor.get_active_events().await;
        println!("📈 Active RFC2833 events: {}", active_events.len());

        Ok(())
    }

    /// Demonstrate SIP INFO DTMF processing
    async fn demonstrate_sip_info(&mut self) -> Result<()> {
        println!("\n📞 === SIP INFO DTMF Method Processing ===");

        // Package negotiation
        let package_negotiator = SipInfoPackageNegotiator::new();
        let recv_info_header = package_negotiator.generate_recv_info_header();
        println!("📋 Recv-Info header: {}", recv_info_header);

        // Test different content types
        let content_types = [
            (
                "Cisco DTMF-Relay",
                "application/dtmf-relay",
                SipInfoDtmfContentType::CiscoDtmfRelay,
            ),
            (
                "Generic DTMF",
                "application/dtmf",
                SipInfoDtmfContentType::GenericDtmf,
            ),
            (
                "Nortel Text",
                "application/vnd.nortel.text",
                SipInfoDtmfContentType::NortelText,
            ),
        ];

        for (name, mime_type, content_type) in &content_types {
            println!("🔄 Testing {} format", name);

            let test_digit = '8';
            let message = SipInfoDtmfMessage::new(test_digit, content_type.clone())
                .with_duration(120)
                .with_volume(80)
                .with_parameter("vendor".to_string(), "redfire".to_string());

            let body_content = message.to_body_content();
            println!(
                "  📝 Generated body: {}",
                body_content.replace('\r', "\\r").replace('\n', "\\n")
            );

            // Process incoming SIP INFO
            let response = self
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

            println!(
                "  ✅ Response: {} {}",
                response.status_code(),
                response.reason_phrase()
            );
        }

        // Generate outgoing SIP INFO
        let outgoing_request = self
            .sip_info_processor
            .generate_outgoing_info("test_session", '3', Some(150), Some(70))
            .await?;
        println!("📤 Generated outgoing SIP INFO:");
        println!("  Method: {}", outgoing_request.method);
        println!("  Content-Type: {}", outgoing_request.content_type);
        println!(
            "  Body: {}",
            outgoing_request
                .body
                .replace('\r', "\\r")
                .replace('\n', "\\n")
        );

        // Session statistics
        let session_stats = self
            .sip_info_processor
            .get_session_stats("sip_session_123")
            .await;
        if let Some(stats) = session_stats {
            println!(
                "📊 Session stats - ID: {}, Call: {}, Sequence: '{}'",
                stats.session_id, stats.call_id, stats.dtmf_sequence
            );
        }

        Ok(())
    }

    /// Demonstrate Sigtran DTMF processing
    async fn demonstrate_sigtran(&mut self) -> Result<()> {
        println!("\n🌐 === Sigtran DTMF Processing ===");

        // ISUP Generic Digits processing
        println!("📡 Processing ISUP Generic Digits");
        let isup_message = SigtranDtmfMessage {
            protocol: SigtranProtocol::M3ua,
            message_type: SigtranDtmfMessageType::IsupGenericDigits,
            digits: "987654321".to_string(),
            encoding: GenericDigitsEncoding::Ia5Character,
            cic: Some(100),
            transaction_id: None,
            parameters: HashMap::new(),
        };

        self.sigtran_processor
            .process_incoming_message(isup_message)
            .await?;
        println!("  ✅ Processed ISUP Generic Digits: '987654321'");

        // TCAP transaction management
        println!("🔄 Managing TCAP transactions");
        let transaction_id = self
            .sigtran_processor
            .start_digit_collection(SigtranProtocol::M3ua, Some(200))
            .await;
        println!("  📞 Started transaction ID: {} (CIC: 200)", transaction_id);

        // TCAP Return Result processing
        let tcap_message = SigtranDtmfMessage {
            protocol: SigtranProtocol::M3ua,
            message_type: SigtranDtmfMessageType::TcapReturnResult,
            digits: "*67890#".to_string(),
            encoding: GenericDigitsEncoding::Ia5Character,
            cic: Some(200),
            transaction_id: Some(transaction_id),
            parameters: HashMap::new(),
        };

        self.sigtran_processor
            .process_incoming_message(tcap_message)
            .await?;
        println!("  ✅ Processed TCAP Return Result: '*67890#'");

        // Create ISUP parameter
        let isup_param = self.sigtran_processor.create_isup_generic_digits(
            "12345",
            redfire_switch::sigtran_dtmf::GenericDigitsType::DtmfDigits,
        )?;
        println!("📦 Created ISUP parameter: {} bytes", isup_param.len());

        // Transaction statistics
        let tx_stats = self.sigtran_processor.get_transaction_stats().await;
        println!("📊 Active transactions: {}", tx_stats.len());
        for stat in tx_stats {
            println!(
                "  Transaction {}: Protocol {:?}, CIC {:?}, Digits: '{}'",
                stat.transaction_id, stat.protocol, stat.cic, stat.collected_digits
            );
        }

        Ok(())
    }

    /// Demonstrate STIR/SHAKEN TDM processing
    async fn demonstrate_stir_shaken(&self) -> Result<()> {
        println!("\n🔐 === STIR/SHAKEN TDM Processing ===");

        let calling_number = "+15551234567";
        let called_number = "+15559876543";
        let call_id = "showcase_call_2024";

        // Create test message
        let test_message = StirShakenTdmMessage {
            transport: StirShakenTransport::InBandIsup,
            calling_number: calling_number.to_string(),
            called_number: called_number.to_string(),
            passport_token: "eyJhbGciOiJFUzI1NiIsInR5cCI6IkpXVCJ9.test_payload.mock_signature"
                .to_string(),
            attestation_level: AttestationLevel::FullAttestation,
            verification_status: VerificationStatus::NoValidation,
            cic: Some(300),
            call_id: call_id.to_string(),
            parameters: HashMap::new(),
            timestamp: SystemTime::now(),
        };

        println!(
            "📞 STIR/SHAKEN call: {} -> {}",
            calling_number, called_number
        );
        println!("🏆 Attestation level: {:?}", test_message.attestation_level);
        println!("🚛 Transport: {:?}", test_message.transport);

        // ISUP UUI encoding/decoding demonstration
        let uui_data = self
            .stir_shaken_processor
            .encode_for_isup_uui(&test_message)?;
        println!("📦 ISUP UUI encoding: {} bytes", uui_data.len());
        println!("  Protocol discriminator: 0x{:02X}", uui_data[0]);
        println!(
            "  STIR indicator: {}",
            String::from_utf8_lossy(&uui_data[1..5])
        );
        println!("  Attestation level: {}", uui_data[5] as char);

        // Round-trip test
        let decoded_message = self.stir_shaken_processor.decode_from_isup_uui(&uui_data)?;
        println!(
            "🔄 Round-trip test - Decoded attestation: {:?}",
            decoded_message.attestation_level
        );

        // Statistics
        let stats = self.stir_shaken_processor.get_statistics().await;
        println!("📊 STIR/SHAKEN stats:");
        println!("  Enabled: {}", stats.enabled);
        println!("  Cached certificates: {}", stats.cached_certificates);
        println!(
            "  Supported transports: {} types",
            stats.supported_transports.len()
        );
        println!("  Requires verification: {}", stats.require_verification);

        Ok(())
    }

    /// Monitor and display DTMF events
    async fn monitor_events(&mut self) -> Result<()> {
        println!("\n📺 === Event Monitoring Started ===");

        let mut event_count = 0;
        let max_events = 50; // Limit to prevent infinite loop

        while event_count < max_events {
            match timeout(Duration::from_millis(100), self.event_receiver.recv()).await {
                Ok(Some(event)) => {
                    self.event_log.write().await.push(event.clone());
                    event_count += 1;

                    match event {
                        DtmfEvent::DigitDetected {
                            digit,
                            duration,
                            source,
                            confidence,
                            ..
                        } => {
                            println!("🔢 Digit '{}' detected from {:?} (duration: {:?}, confidence: {:.2})", 
                                     digit, source, duration, confidence);
                        }
                        DtmfEvent::SequenceComplete {
                            sequence,
                            source,
                            total_duration,
                        } => {
                            println!(
                                "✅ Sequence '{}' completed from {:?} (total: {:?})",
                                sequence, source, total_duration
                            );
                        }
                        DtmfEvent::DigitGenerate {
                            digit,
                            duration,
                            source,
                            ..
                        } => {
                            println!(
                                "🎵 Generate digit '{}' request from {:?} (duration: {:?})",
                                digit, source, duration
                            );
                        }
                        DtmfEvent::DetectionError { error, source } => {
                            println!("❌ Detection error from {:?}: {}", source, error);
                        }
                    }
                }
                Ok(None) => {
                    println!("📺 Event channel closed");
                    break;
                }
                Err(_) => {
                    // Timeout - no more events
                    break;
                }
            }
        }

        println!("📈 Total events processed: {}", event_count);
        Ok(())
    }

    /// Generate summary report
    async fn generate_summary(&self) -> Result<()> {
        println!("\n📋 === DTMF Showcase Summary Report ===");

        let event_log = self.event_log.read().await;
        let mut source_counts: HashMap<DtmfSource, u32> = HashMap::new();
        let mut digit_counts: HashMap<char, u32> = HashMap::new();

        for event in event_log.iter() {
            match event {
                DtmfEvent::DigitDetected { digit, source, .. } => {
                    *source_counts.entry(*source).or_insert(0) += 1;
                    *digit_counts.entry(*digit).or_insert(0) += 1;
                }
                _ => {}
            }
        }

        println!("📊 Events by Source:");
        for (source, count) in source_counts {
            println!("  {:?}: {} events", source, count);
        }

        println!("📊 Digits Detected:");
        for (digit, count) in digit_counts {
            println!("  '{}': {} times", digit, count);
        }

        println!("🎯 Total unique digits processed: {}", digit_counts.len());
        println!("🌐 Transport protocols tested: 4 (Core, RFC2833, SIP INFO, Sigtran)");
        println!("🔐 Security features: STIR/SHAKEN TDM support included");
        println!("✅ Showcase completed successfully!");

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .init();

    println!("🚀 === RedFire Switch DTMF Functionality Showcase ===");
    println!("🎼 Demonstrating comprehensive DTMF support across all transport protocols");
    println!();

    let mut showcase = DtmfShowcase::new().await?;

    // Run demonstrations
    showcase.demonstrate_core_dtmf().await?;
    showcase.demonstrate_rfc2833().await?;
    showcase.demonstrate_sip_info().await?;
    showcase.demonstrate_sigtran().await?;
    showcase.demonstrate_stir_shaken().await?;

    // Monitor and collect events
    showcase.monitor_events().await?;

    // Generate final summary
    showcase.generate_summary().await?;

    println!("\n🎉 DTMF Showcase completed! All transport protocols tested successfully.");

    Ok(())
}
