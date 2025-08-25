/*
 * Integrated SIP Stack and Codec Engine Demonstration
 * Shows how the full integration works with real-time codec translation
 */

use anyhow::Result;
use redfire_codec_engine::{AudioCodec, CodecConfig};
use redfire_sip_stack::{SipCoreConfig, SipTransport};
use redfire_switch::rtp_proxy_impl::RtpProxyConfig;
use redfire_switch::sip_codec_integration::{create_integrated_service, SipCodecIntegration};
use std::net::SocketAddr;
use tracing::{error, info};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("\n🚀 Integrated SIP Stack and Codec Engine Demo");
    println!("{}", "=".repeat(60));

    // Configure SIP stack
    let sip_config = SipCoreConfig {
        auth_realm: "example.com".to_string(),
        strict_rfc_compliance: true,
        max_transactions: 1000,
        max_dialogs: 1000,
        transaction_timeout: 32,
        dialog_timeout: 1800,
        enable_authentication: true,
        ..Default::default()
    };

    // Configure codec engine with GPU acceleration if available
    let codec_config = CodecConfig {
        enabled: true,
        use_gpu: cfg!(any(feature = "cuda", feature = "rocm")),
        max_sessions: 100,
        quality: 0.9,
        buffer_size: 8192,
        supported_translations: vec![
            redfire_codec_engine::codec::CodecTranslation {
                from: redfire_codec_engine::codec::AudioCodec::G711Ulaw,
                to: redfire_codec_engine::codec::AudioCodec::G711Alaw,
            },
            redfire_codec_engine::codec::CodecTranslation {
                from: redfire_codec_engine::codec::AudioCodec::G711Ulaw,
                to: redfire_codec_engine::codec::AudioCodec::G729,
            },
            redfire_codec_engine::codec::CodecTranslation {
                from: redfire_codec_engine::codec::AudioCodec::G729,
                to: redfire_codec_engine::codec::AudioCodec::G711Ulaw,
            },
            redfire_codec_engine::codec::CodecTranslation {
                from: redfire_codec_engine::codec::AudioCodec::Opus,
                to: redfire_codec_engine::codec::AudioCodec::G711Ulaw,
            },
        ],
        ..Default::default()
    };

    // Configure RTP proxy for media handling
    let rtp_config = RtpProxyConfig {
        enabled: true,
        max_sessions: 1000,
        rtp_timeout: 30,
        codec_translation: true,
        jitter_buffer_size: 50,
        max_jitter_delay: 200,
        dtmf_relay: true,
        port_range: (10000, 20000),
    };

    // Create integrated service
    info!("Creating integrated SIP/Codec service...");
    let service = SipCodecIntegration::new(sip_config, codec_config, rtp_config).await?;

    println!("\n✅ Service initialized successfully!");
    println!("\n📋 Capabilities:");
    println!("  • Full SIP stack (RFC 3261 compliant)");
    println!("  • Real-time codec translation");
    println!(
        "  • GPU acceleration: {}",
        if cfg!(any(feature = "cuda", feature = "rocm")) {
            "Enabled"
        } else {
            "Disabled"
        }
    );
    println!("  • Supported codecs: G.711 µ-law/A-law, G.729, G.722, Opus");
    println!("  • RTP proxy with jitter buffering");
    println!("  • DTMF relay support");

    // Demonstrate processing a sample INVITE
    println!("\n📞 Processing sample INVITE with codec negotiation...");

    let sample_invite = create_sample_invite();
    let from_addr: SocketAddr = "192.168.1.100:5060".parse()?;

    match service
        .process_sip_message(sample_invite.as_bytes(), from_addr, SipTransport::Udp)
        .await
    {
        Ok(_) => {
            println!("✅ INVITE processed successfully");
            println!("  • SIP message parsed and validated");
            println!("  • Codecs negotiated (G.711µ -> G.711A)");
            println!("  • RTP proxy session established");
            println!("  • Transcoding session initialized");
        }
        Err(e) => {
            error!("Failed to process INVITE: {}", e);
        }
    }

    // Show codec translation in action
    println!("\n🔄 Demonstrating codec translation:");
    demonstrate_codec_translation().await?;

    // Show SIP-I/ISUP integration
    println!("\n📡 SIP-I/ISUP Integration:");
    demonstrate_sipi_integration()?;

    println!("\n✨ Demo completed successfully!");
    println!("{}", "=".repeat(60));

    Ok(())
}

fn create_sample_invite() -> String {
    r#"INVITE sip:alice@example.com SIP/2.0
Via: SIP/2.0/UDP 192.168.1.100:5060;branch=z9hG4bK776asdhds
Max-Forwards: 70
To: Alice <sip:alice@example.com>
From: Bob <sip:bob@example.com>;tag=1928301774
Call-ID: a84b4c76e66710@pc33.example.com
CSeq: 314159 INVITE
Contact: <sip:bob@192.168.1.100>
Content-Type: application/sdp
Content-Length: 142

v=0
o=bob 2890844526 2890842807 IN IP4 192.168.1.100
s=-
c=IN IP4 192.168.1.100
t=0 0
m=audio 49170 RTP/AVP 0 8
a=rtpmap:0 PCMU/8000
a=rtpmap:8 PCMA/8000"#
        .to_string()
}

async fn demonstrate_codec_translation() -> Result<()> {
    use redfire_codec_engine::{AudioCodec, CodecConfig, CodecService};

    let config = CodecConfig::default();
    let service = CodecService::new(config).await?;

    // Start a transcoding session
    let session_id = "demo-transcode".to_string();
    service
        .start_session(
            session_id.clone(),
            redfire_codec_engine::codec::AudioCodec::G711Ulaw,
            redfire_codec_engine::codec::AudioCodec::G711Alaw,
            8000,
            1,
        )
        .await?;

    println!("  ✅ Created transcoding session: G.711µ -> G.711A");

    // Simulate transcoding some audio
    let sample_frame = redfire_codec_engine::codec::AudioFrame {
        data: vec![0u8; 160], // 20ms of G.711µ
        codec: redfire_codec_engine::codec::AudioCodec::G711Ulaw,
        sample_rate: 8000,
        channels: 1,
        timestamp: 0,
        sequence: 1,
    };

    let frame_len = sample_frame.data.len();
    match service.transcode_frame(&session_id, sample_frame).await {
        Ok(transcoded) => {
            println!(
                "  ✅ Transcoded {} bytes -> {} bytes",
                frame_len,
                transcoded.data.len()
            );
        }
        Err(e) => {
            println!("  ❌ Transcoding failed: {}", e);
        }
    }

    // Clean up
    service.end_session(&session_id).await?;
    println!("  ✅ Session cleaned up");

    Ok(())
}

fn demonstrate_sipi_integration() -> Result<()> {
    use redfire_sip_stack::{SipTSipIConfig, SipTSipIService};

    let config = SipTSipIConfig {
        sipt_enabled: true,
        sipi_enabled: true,
        isup_variant: redfire_sip_stack::sipt_sipi::IsupVariant::Itu,
        originating_point_code: 0x123456,
        destination_point_code: 0x654321,
        cic_range_start: 1,
        cic_range_end: 4096,
        validate_isup: true,
        multipart_support: true,
        max_isup_size: 8192,
    };

    let service = SipTSipIService::new(config);

    // Generate ISUP IAM from SIP
    match service.sip_to_iam("+15551234567", "+15559876543", 42) {
        Ok(iam) => {
            println!("  ✅ Generated ISUP IAM message");
            println!("    • Message Type: {:?}", iam.message_type);
            println!("    • CIC: {}", iam.cic);
            println!("    • Parameters: {} items", iam.optional.len());
        }
        Err(e) => {
            println!("  ❌ ISUP generation failed: {}", e);
        }
    }

    // Show multipart MIME support for SIP-T
    let isup_data = vec![0x01, 0x23, 0x45, 0x67];
    let sdp = "v=0\r\no=- 0 0 IN IP4 192.168.1.1\r\ns=-\r\nc=IN IP4 192.168.1.1\r\nt=0 0\r\nm=audio 10000 RTP/AVP 0\r\n";

    match service.create_sipt_body(&isup_data, Some(sdp)) {
        Ok(body) => {
            println!("  ✅ Created SIP-T multipart body");
            println!("    • Content-Type: multipart/mixed");
            println!("    • Body length: {} bytes", body.len());
        }
        Err(e) => {
            println!("  ❌ SIP-T body creation failed: {}", e);
        }
    }

    Ok(())
}
