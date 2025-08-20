/*
 * Demonstration of SIP Stack and Codec Engine Integration
 * Shows the complete flow from SIP processing to codec translation
 */

use anyhow::Result;
use redfire_switch::integrated_sip_codec::{IntegratedService, sip, codec};
use tracing::{info, error};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    println!("\n🚀 SIP Stack and Codec Engine Integration Demo");
    println!("=" .repeat(60));
    
    // Create integrated service
    info!("Creating integrated SIP and Codec service...");
    let service = IntegratedService::new().await?;
    
    println!("\n✅ Service initialized with:");
    println!("  • Full SIP parsing and validation");
    println!("  • Codec translation engine");
    println!("  • SIP-I/ISUP support");
    println!("  • GPU acceleration: {}", 
             if cfg!(any(feature = "cuda", feature = "rocm")) { "Available" } else { "Not available" });
    
    // Demo 1: SIP Message Parsing
    println!("\n📞 Demo 1: SIP Message Processing");
    println!("-" .repeat(40));
    demonstrate_sip_parsing(&service)?;
    
    // Demo 2: Codec Translation
    println!("\n🔄 Demo 2: Codec Translation");
    println!("-" .repeat(40));
    demonstrate_codec_translation(&service).await?;
    
    // Demo 3: SIP-I/ISUP Generation
    println!("\n📡 Demo 3: SIP-I/ISUP Integration");
    println!("-" .repeat(40));
    demonstrate_sipi(&service)?;
    
    // Demo 4: Complete Call Flow
    println!("\n🎯 Demo 4: Complete Call Flow");
    println!("-" .repeat(40));
    demonstrate_complete_flow(&service).await?;
    
    println!("\n✨ All demonstrations completed successfully!");
    println!("=" .repeat(60));
    
    Ok(())
}

fn demonstrate_sip_parsing(service: &IntegratedService) -> Result<()> {
    let sip_invite = b"INVITE sip:+15551234567@carrier.com SIP/2.0\r\n\
        Via: SIP/2.0/UDP 192.168.1.100:5060;branch=z9hG4bK776asdhds\r\n\
        Max-Forwards: 70\r\n\
        To: <sip:+15551234567@carrier.com>\r\n\
        From: <sip:+15559876543@example.com>;tag=1928301774\r\n\
        Call-ID: a84b4c76e66710@pc33.example.com\r\n\
        CSeq: 314159 INVITE\r\n\
        Contact: <sip:bob@192.168.1.100>\r\n\
        Content-Type: application/sdp\r\n\
        Content-Length: 0\r\n\r\n";
    
    match service.parse_sip(sip_invite) {
        Ok(message) => {
            println!("✅ Successfully parsed SIP INVITE");
            println!("  • Method: {:?}", message.method);
            println!("  • Call-ID extracted");
            println!("  • Headers validated");
        }
        Err(e) => {
            error!("Failed to parse SIP message: {}", e);
        }
    }
    
    Ok(())
}

async fn demonstrate_codec_translation(service: &IntegratedService) -> Result<()> {
    use codec::AudioCodec;
    
    // Check if transcoding is needed
    let from_codec = AudioCodec::G711Ulaw;
    let to_codec = AudioCodec::G711Alaw;
    
    if IntegratedService::needs_transcoding(from_codec, to_codec) {
        println!("✅ Transcoding required: {:?} -> {:?}", from_codec, to_codec);
        
        // Start transcoding session
        let session_id = "demo-transcode-001".to_string();
        service.start_transcoding(
            session_id.clone(),
            from_codec,
            to_codec,
        ).await?;
        
        println!("✅ Transcoding session started");
        println!("  • Session ID: {}", session_id);
        println!("  • Input: G.711 µ-law @ 8kHz");
        println!("  • Output: G.711 A-law @ 8kHz");
        
        // Simulate transcoding a frame
        let sample_frame = vec![0xFFu8; 160]; // 20ms of G.711µ
        match service.transcode_frame(&session_id, &sample_frame).await {
            Ok(transcoded) => {
                println!("✅ Frame transcoded successfully");
                println!("  • Input size: {} bytes", sample_frame.len());
                println!("  • Output size: {} bytes", transcoded.data.len());
                println!("  • Processing time: {} µs", transcoded.processing_time_us);
            }
            Err(e) => {
                error!("Transcoding failed: {}", e);
            }
        }
        
        // Get statistics
        let stats = service.get_codec_stats().await;
        println!("📊 Codec Statistics:");
        println!("  • Active sessions: {}", stats.active_sessions);
        println!("  • Total frames: {}", stats.total_frames_processed);
    } else {
        println!("ℹ️ No transcoding needed for identical codecs");
    }
    
    Ok(())
}

fn demonstrate_sipi(service: &IntegratedService) -> Result<()> {
    // Generate ISUP IAM from SIP data
    let calling = "+15559876543";
    let called = "+15551234567";
    let cic = 42;
    
    match service.generate_isup_iam(calling, called, cic) {
        Ok(iam) => {
            println!("✅ Generated ISUP IAM message");
            println!("  • Message Type: {:?}", iam.message_type);
            println!("  • CIC: {}", iam.cic);
            println!("  • Calling: {}", calling);
            println!("  • Called: {}", called);
            println!("  • Parameters: {} items", iam.optional.len());
        }
        Err(e) => {
            error!("ISUP generation failed: {}", e);
        }
    }
    
    // Create SIP-T multipart body
    let isup_data = vec![0x01, 0x00, 0x2A]; // Sample ISUP data
    let sdp = "v=0\r\no=- 0 0 IN IP4 192.168.1.1\r\ns=-\r\nc=IN IP4 192.168.1.1\r\nt=0 0\r\nm=audio 10000 RTP/AVP 0\r\n";
    
    match service.create_sipt_body(&isup_data, Some(sdp)) {
        Ok(body) => {
            println!("✅ Created SIP-T multipart body");
            println!("  • Content-Type: {}", body.content_type);
            println!("  • Parts: {}", body.parts.len());
            println!("  • Boundary: {}", body.boundary);
        }
        Err(e) => {
            error!("SIP-T body creation failed: {}", e);
        }
    }
    
    Ok(())
}

async fn demonstrate_complete_flow(service: &IntegratedService) -> Result<()> {
    println!("📞 Simulating complete SIP call with codec translation:");
    
    // Step 1: Parse incoming INVITE
    println!("\n1️⃣ Parsing incoming INVITE...");
    let invite = b"INVITE sip:alice@example.com SIP/2.0\r\n\
        Via: SIP/2.0/UDP 192.168.1.100:5060\r\n\
        From: <sip:bob@example.com>\r\n\
        To: <sip:alice@example.com>\r\n\
        Call-ID: call-123\r\n\
        CSeq: 1 INVITE\r\n\
        Content-Length: 0\r\n\r\n";
    
    let _message = service.parse_sip(invite)?;
    println!("   ✅ INVITE parsed successfully");
    
    // Step 2: Determine codec requirements
    println!("\n2️⃣ Analyzing codec requirements...");
    let ingress_codec = codec::AudioCodec::G711Ulaw;
    let egress_codec = codec::AudioCodec::G729;
    
    if IntegratedService::needs_transcoding(ingress_codec, egress_codec) {
        println!("   ✅ Transcoding needed: G.711µ -> G.729");
        
        // Step 3: Set up transcoding
        println!("\n3️⃣ Setting up transcoding session...");
        let session_id = "call-123-codec".to_string();
        service.start_transcoding(
            session_id.clone(),
            ingress_codec,
            egress_codec,
        ).await?;
        println!("   ✅ Transcoding session established");
        
        // Step 4: Generate ISUP if needed
        println!("\n4️⃣ Generating ISUP for PSTN interconnection...");
        let iam = service.generate_isup_iam("+15559876543", "+15551234567", 100)?;
        println!("   ✅ ISUP IAM generated with CIC {}", iam.cic);
        
        // Step 5: Process media
        println!("\n5️⃣ Processing media stream...");
        let audio_frame = vec![0xFFu8; 160]; // Sample audio
        let transcoded = service.transcode_frame(&session_id, &audio_frame).await?;
        println!("   ✅ Audio transcoded: {} bytes -> {} bytes", 
                audio_frame.len(), transcoded.data.len());
    }
    
    println!("\n✅ Complete call flow demonstrated successfully!");
    
    Ok(())
}