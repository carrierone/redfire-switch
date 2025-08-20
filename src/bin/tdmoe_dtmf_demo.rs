/*
 * TDMoE DTMF Integration Demonstration
 *
 * This binary demonstrates the complete integration of DTMF functionality
 * with TDMoE (Time Division Multiplexing over Ethernet) including:
 * - Real-time DTMF detection from TDM voice channels
 * - DTMF generation to TDM voice channels
 * - Cross-protocol DTMF relay between TDM and SIP
 * - NI-2 signaling integration for DTMF transport
 * - Performance monitoring and statistics
 *
 * Usage: cargo run --bin tdmoe-dtmf-demo
 */

use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, interval};
use tracing::{info, warn, Level};
use tracing_subscriber;

use redfire_switch::tdmoe_dtmf::{
    TdmoeDtmfIntegration, TdmoeDtmfChannelConfig, TdmoeDtmfEvent
};
use redfire_switch::tdmoe_ni2_signaling::TdmoeNi2Signaling;

/// Simulated TDM audio data generator for testing
struct TdmAudioSimulator {
    sample_rate: u32,
    current_sample: u64,
}

impl TdmAudioSimulator {
    fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            current_sample: 0,
        }
    }
    
    /// Generate simulated TDM audio samples (mostly silence with optional DTMF)
    fn generate_samples(&mut self, num_samples: usize, dtmf_digit: Option<char>) -> Vec<i16> {
        let mut samples = Vec::with_capacity(num_samples);
        
        if let Some(digit) = dtmf_digit {
            // Generate DTMF-like tones (simplified)
            let (freq1, freq2) = match digit {
                '1' => (697.0, 1209.0),
                '2' => (697.0, 1336.0),
                '3' => (697.0, 1477.0),
                '4' => (770.0, 1209.0),
                '5' => (770.0, 1336.0),
                '6' => (770.0, 1477.0),
                '7' => (852.0, 1209.0),
                '8' => (852.0, 1336.0),
                '9' => (852.0, 1477.0),
                '*' => (941.0, 1209.0),
                '0' => (941.0, 1336.0),
                '#' => (941.0, 1477.0),
                _ => (770.0, 1336.0), // Default to '5'
            };
            
            for i in 0..num_samples {
                let t = (self.current_sample + i as u64) as f32 / self.sample_rate as f32;
                let tone1 = (2.0 * std::f32::consts::PI * freq1 * t).sin();
                let tone2 = (2.0 * std::f32::consts::PI * freq2 * t).sin();
                let sample = 0.3 * (tone1 + tone2) / 2.0; // Moderate amplitude
                samples.push((sample * 16384.0) as i16);
            }
        } else {
            // Generate silence with small amount of noise
            for _i in 0..num_samples {
                let noise = (rand::random::<f32>() - 0.5) * 100.0;
                samples.push(noise as i16);
            }
        }
        
        self.current_sample += num_samples as u64;
        samples
    }
}

/// TDMoE span simulator
struct TdmSpanSimulator {
    span_id: u8,
    channel_count: u8,
    audio_simulators: Vec<TdmAudioSimulator>,
    dtmf_integration: Arc<TdmoeDtmfIntegration>,
}

impl TdmSpanSimulator {
    async fn new(span_id: u8, channel_count: u8, dtmf_integration: Arc<TdmoeDtmfIntegration>) -> Result<Self> {
        let mut audio_simulators = Vec::new();
        
        // Configure TDM channels for DTMF processing
        for channel_num in 1..=channel_count {
            let channel_id = format!("T1-{}-{}", span_id, channel_num);
            
            let config = TdmoeDtmfChannelConfig {
                channel_id: channel_id.clone(),
                span_number: span_id,
                channel_number: channel_num,
                enable_detection: true,
                enable_generation: true,
                detection_sensitivity: 0.8,
                generation_amplitude: 0.7,
                sip_call_id: None,
                b2bua_leg_id: None,
            };
            
            dtmf_integration.add_tdm_channel(config).await?;
            audio_simulators.push(TdmAudioSimulator::new(8000));
        }
        
        Ok(Self {
            span_id,
            channel_count,
            audio_simulators,
            dtmf_integration,
        })
    }
    
    /// Simulate TDM audio processing for all channels
    async fn process_audio_frame(&mut self, frame_size: usize) -> Result<()> {
        for (channel_idx, simulator) in self.audio_simulators.iter_mut().enumerate() {
            let channel_num = channel_idx as u8 + 1;
            let channel_id = format!("T1-{}-{}", self.span_id, channel_num);
            
            // Occasionally inject DTMF digits for testing
            let dtmf_digit = if rand::random::<f32>() < 0.001 { // Very low probability
                Some(match rand::random::<u8>() % 10 {
                    0 => '0', 1 => '1', 2 => '2', 3 => '3', 4 => '4',
                    5 => '5', 6 => '6', 7 => '7', 8 => '8', 9 => '9',
                    _ => '5',
                })
            } else {
                None
            };
            
            let samples = simulator.generate_samples(frame_size, dtmf_digit);
            
            // Process with DTMF integration
            self.dtmf_integration.process_tdm_audio(&channel_id, &samples).await?;
        }
        
        Ok(())
    }
}

/// Event monitor for TDMoE DTMF integration
struct DtmfEventMonitor {
    event_receiver: tokio::sync::broadcast::Receiver<TdmoeDtmfEvent>,
    event_count: u64,
}

impl DtmfEventMonitor {
    fn new(event_receiver: tokio::sync::broadcast::Receiver<TdmoeDtmfEvent>) -> Self {
        Self {
            event_receiver,
            event_count: 0,
        }
    }
    
    async fn monitor_events(&mut self) {
        while let Ok(event) = self.event_receiver.recv().await {
            self.event_count += 1;
            
            match event {
                TdmoeDtmfEvent::TdmDigitDetected { channel_id, digit, duration, confidence, .. } => {
                    info!("🔢 TDM DTMF detected: '{}' on {} (duration: {:?}, confidence: {:.2})",
                          digit, channel_id, duration, confidence);
                }
                TdmoeDtmfEvent::TdmSequenceComplete { channel_id, sequence, total_duration } => {
                    info!("✅ TDM DTMF sequence complete: '{}' on {} (total: {:?})",
                          sequence, channel_id, total_duration);
                }
                TdmoeDtmfEvent::DtmfRelaySipOut { tdm_channel, sip_call_id, digit, transport_method } => {
                    info!("📤 DTMF relay TDM->SIP: '{}' from {} to {} via {}",
                          digit, tdm_channel, sip_call_id, transport_method);
                }
                TdmoeDtmfEvent::DtmfRelaySipIn { sip_call_id, tdm_channel, digit, source_method } => {
                    info!("📥 DTMF relay SIP->TDM: '{}' from {} to {} via {}",
                          digit, sip_call_id, tdm_channel, source_method);
                }
                TdmoeDtmfEvent::Ni2DtmfSignaling { channel_id, message_type, digits } => {
                    info!("🔗 NI-2 DTMF signaling: {} on {} with digits '{}'",
                          message_type, channel_id, digits);
                }
            }
        }
    }
}

/// Statistics reporter
struct StatsReporter {
    dtmf_integration: Arc<TdmoeDtmfIntegration>,
    report_interval: Duration,
}

impl StatsReporter {
    fn new(dtmf_integration: Arc<TdmoeDtmfIntegration>, report_interval: Duration) -> Self {
        Self {
            dtmf_integration,
            report_interval,
        }
    }
    
    async fn start_reporting(&self) {
        let mut interval = interval(self.report_interval);
        
        loop {
            interval.tick().await;
            
            let stats = self.dtmf_integration.get_statistics().await;
            
            info!("📊 TDMoE DTMF Statistics Report:");
            info!("   Total channels: {}", stats.total_channels);
            info!("   Active detection: {}", stats.active_detection_channels);
            info!("   Active generation: {}", stats.active_generation_channels);
            info!("   TDM digits detected: {}", stats.total_tdm_digits);
            info!("   Generated digits: {}", stats.total_generated_digits);
            info!("   TDM->SIP relays: {}", stats.tdm_to_sip_relays);
            info!("   SIP->TDM relays: {}", stats.sip_to_tdm_relays);
            info!("   NI-2 messages: {}", stats.ni2_messages);
            info!("   Avg detection latency: {:.2}ms", stats.avg_detection_latency_ms);
            info!("   Audio samples/sec: {:.0}", stats.audio_samples_per_second);
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .init();
    
    info!("🚀 Starting TDMoE DTMF Integration Demonstration");
    
    // Create NI-2 signaling (User side for demo)
    let ni2_signaling = Arc::new(TdmoeNi2Signaling::new_with_side(
        redfire_switch::tdmoe_ni2_signaling::Ni2SideType::User
    )?);
    
    // Create TDMoE DTMF integration
    let dtmf_integration = Arc::new(TdmoeDtmfIntegration::new(ni2_signaling).await?);
    
    // Start event processing
    dtmf_integration.start_event_processing().await?;
    
    // Create TDM span simulators
    let mut spans = Vec::new();
    for span_id in 1..=2 {
        let span = TdmSpanSimulator::new(span_id, 4, Arc::clone(&dtmf_integration)).await?;
        spans.push(span);
        info!("📡 Created TDM span {} with 4 channels", span_id);
    }
    
    // Setup SIP call associations for cross-protocol testing
    dtmf_integration.associate_sip_call("T1-1-1", "sip_call_123").await?;
    dtmf_integration.associate_sip_call("T1-2-1", "sip_call_456").await?;
    info!("🔗 Associated TDM channels with SIP calls for relay testing");
    
    // Start event monitoring
    let event_receiver = dtmf_integration.subscribe_events();
    let mut event_monitor = DtmfEventMonitor::new(event_receiver);
    
    // Start statistics reporting
    let stats_reporter = StatsReporter::new(Arc::clone(&dtmf_integration), Duration::from_secs(10));
    
    // Spawn monitoring tasks
    let event_monitor_task = tokio::spawn(async move {
        event_monitor.monitor_events().await;
    });
    
    let stats_reporter_task = tokio::spawn(async move {
        stats_reporter.start_reporting().await;
    });
    
    // Test DTMF generation
    info!("🎵 Testing DTMF generation to TDM channels...");
    for digit in "123456789*0#".chars() {
        let samples = dtmf_integration.generate_tdm_dtmf("T1-1-1", digit, None).await?;
        info!("Generated {} samples for DTMF '{}'", samples.len(), digit);
        sleep(Duration::from_millis(200)).await;
    }
    
    // Test cross-protocol DTMF relay
    info!("📡 Testing cross-protocol DTMF relay...");
    for digit in "987*654#321".chars() {
        dtmf_integration.relay_dtmf_to_rfc2833("T1-1-1", digit, 150).await?;
        sleep(Duration::from_millis(100)).await;
    }
    
    // Test NI-2 signaling integration
    info!("🔗 Testing NI-2 signaling integration...");
    dtmf_integration.process_ni2_dtmf_signaling("T1-1-1", "5551234567").await?;
    
    // Main processing loop
    info!("🔄 Starting main TDM audio processing loop...");
    let frame_size = 80; // 10ms frames at 8kHz
    let mut frame_count = 0;
    
    loop {
        // Process audio frames for all spans
        for span in &mut spans {
            span.process_audio_frame(frame_size).await?;
        }
        
        frame_count += 1;
        
        // Test SIP->TDM relay periodically
        if frame_count % 8000 == 0 { // Every ~10 seconds
            if let Err(e) = dtmf_integration.relay_dtmf_from_sip("sip_call_123", '#', 120).await {
                warn!("Failed to relay DTMF from SIP: {}", e);
            }
        }
        
        // Sleep to simulate real-time processing (10ms frames)
        sleep(Duration::from_millis(10)).await;
        
        // Stop after reasonable demo time
        if frame_count > 6000 { // ~1 minute of demo
            break;
        }
    }
    
    info!("🏁 TDMoE DTMF demonstration completed after {} frames", frame_count);
    
    // Cancel monitoring tasks
    event_monitor_task.abort();
    stats_reporter_task.abort();
    
    // Final statistics
    let final_stats = dtmf_integration.get_statistics().await;
    info!("📈 Final Statistics:");
    info!("   Total TDM digits detected: {}", final_stats.total_tdm_digits);
    info!("   Total digits generated: {}", final_stats.total_generated_digits);
    info!("   Cross-protocol relays: {} TDM->SIP, {} SIP->TDM", 
          final_stats.tdm_to_sip_relays, final_stats.sip_to_tdm_relays);
    
    Ok(())
}