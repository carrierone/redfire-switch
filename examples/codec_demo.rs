/*
 * Example: Using the Redfire Codec Engine Library
 *
 * This example demonstrates how to use the codec translation
 * capabilities from the extracted library.
 */

use anyhow::Result;
use redfire_codec_engine::{
    create_default_service, AudioCodec, AudioFrame, CodecConfig, CodecService,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("Redfire Codec Engine Demo");
    println!("=========================");

    // Create codec service using the library
    let service = create_default_service().await?;

    // Start a transcoding session (G.711 μ-law to A-law)
    let session_id = "demo_session".to_string();
    service
        .start_session(
            session_id.clone(),
            AudioCodec::G711Ulaw,
            AudioCodec::G711Alaw,
            8000, // Sample rate
            1,    // Mono
        )
        .await?;

    // Create a test audio frame (160 samples of μ-law audio)
    let test_frame = AudioFrame {
        data: vec![0x7F; 160], // Test μ-law data
        codec: AudioCodec::G711Ulaw,
        sample_rate: 8000,
        channels: 1,
        timestamp: 0,
        sequence: 1,
    };

    // Transcode the frame
    let transcoded = service.transcode_frame(&session_id, test_frame).await?;

    println!("Original codec: {:?}", transcoded.original.codec);
    println!("Target codec: {:?}", transcoded.target_codec);
    println!("Processing time: {}μs", transcoded.processing_time_us);
    println!("Original size: {} bytes", transcoded.original.data.len());
    println!("Transcoded size: {} bytes", transcoded.data.len());

    // Get service statistics
    let stats = service.get_statistics().await;
    println!("\nService Statistics:");
    println!("  Active sessions: {}", stats.active_sessions);
    println!("  GPU acceleration: {}", stats.gpu_acceleration_active);

    // Clean up
    service.end_session(&session_id).await?;

    println!("\nDemo completed successfully!");

    Ok(())
}
