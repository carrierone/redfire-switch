/*
 * Call Bridging Integration Tests
 * Tests complete SIP call flow with and without media relay
 */

use anyhow::Result;
use redfire_switch::codec::{AudioCodec, AudioFrame, CodecConfig, CodecService};
use redfire_switch::g729_codec::{G729Codec, G729_FRAME_SIZE, G729_SAMPLE_RATE};
use redfire_switch::rtp::{RtpPacket, RtpStats};
use redfire_switch::rtp_proxy_impl::{
    AudioCodec as RtpAudioCodec, RtpProxyConfig, RtpProxyService,
};
use redfire_switch::sdp::{
    CodecInfo, ConnectionData, MediaDescription, MediaType, OriginField, SdpSession,
    TimeDescription,
};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::time::{sleep, timeout};
use tracing::{debug, info, warn};

/// Test configuration for call bridging scenarios
#[derive(Debug, Clone)]
struct CallBridgingTestConfig {
    /// Test scenario name
    pub name: String,
    /// Caller codec
    pub caller_codec: RtpAudioCodec,
    /// Callee codec  
    pub callee_codec: RtpAudioCodec,
    /// Enable media relay (vs direct passthrough)
    pub enable_media_relay: bool,
    /// Test duration in seconds
    pub test_duration: u64,
    /// Number of test packets to send
    pub packet_count: u32,
    /// Expected transcoding (when codecs differ)
    pub expect_transcoding: bool,
}

impl CallBridgingTestConfig {
    /// Create test config for direct passthrough (same codecs)
    fn direct_passthrough(name: &str, codec: RtpAudioCodec) -> Self {
        Self {
            name: name.to_string(),
            caller_codec: codec,
            callee_codec: codec,
            enable_media_relay: false,
            test_duration: 5,
            packet_count: 100,
            expect_transcoding: false,
        }
    }

    /// Create test config for codec transcoding
    fn codec_transcoding(name: &str, from_codec: RtpAudioCodec, to_codec: RtpAudioCodec) -> Self {
        Self {
            name: name.to_string(),
            caller_codec: from_codec,
            callee_codec: to_codec,
            enable_media_relay: true,
            test_duration: 5,
            packet_count: 100,
            expect_transcoding: true,
        }
    }
}

/// Simulated SIP endpoint for testing
struct SipEndpoint {
    name: String,
    socket: Arc<UdpSocket>,
    local_addr: SocketAddr,
    remote_addr: Option<SocketAddr>,
    codec: RtpAudioCodec,
    sequence_number: u16,
    timestamp: u32,
    ssrc: u32,
    stats: RtpStats,
}

impl SipEndpoint {
    /// Create new SIP endpoint
    async fn new(name: &str, codec: RtpAudioCodec) -> Result<Self> {
        let socket = UdpSocket::bind("127.0.0.1:0").await?;
        let local_addr = socket.local_addr()?;

        Ok(Self {
            name: name.to_string(),
            socket: Arc::new(socket),
            local_addr,
            remote_addr: None,
            codec,
            sequence_number: 1000,
            timestamp: 0,
            ssrc: rand::random(),
            stats: RtpStats::new(),
        })
    }

    /// Connect to remote endpoint
    fn connect_to(&mut self, remote_addr: SocketAddr) {
        self.remote_addr = Some(remote_addr);
    }

    /// Generate test audio data for codec
    fn generate_test_audio(&self, frame_number: u32) -> Vec<u8> {
        match self.codec {
            RtpAudioCodec::G711Ulaw => {
                // Generate μ-law encoded sine wave
                let mut samples = Vec::with_capacity(160); // 20ms @ 8kHz
                for i in 0..160 {
                    let sample =
                        (2.0 * std::f32::consts::PI * 1000.0 * (frame_number * 160 + i) as f32
                            / 8000.0)
                            .sin();
                    let pcm = (sample * 32767.0) as i16;
                    let ulaw = Self::linear_to_ulaw(pcm);
                    samples.push(ulaw);
                }
                samples
            }
            RtpAudioCodec::G711Alaw => {
                // Generate A-law encoded sine wave
                let mut samples = Vec::with_capacity(160);
                for i in 0..160 {
                    let sample =
                        (2.0 * std::f32::consts::PI * 1000.0 * (frame_number * 160 + i) as f32
                            / 8000.0)
                            .sin();
                    let pcm = (sample * 32767.0) as i16;
                    let alaw = Self::linear_to_alaw(pcm);
                    samples.push(alaw);
                }
                samples
            }
            RtpAudioCodec::G729 => {
                // Generate G.729 encoded frames (10 bytes per 10ms)
                vec![0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11, 0x22]
            }
            RtpAudioCodec::G722 => {
                // Generate G.722 encoded data (16kHz, but RTP clock is 8kHz)
                let mut samples = Vec::with_capacity(160);
                for i in 0..160 {
                    let sample = (2.0 * std::f32::consts::PI * 2000.0 * i as f32 / 16000.0).sin();
                    samples.push(((sample + 1.0) * 127.5) as u8);
                }
                samples
            }
            RtpAudioCodec::Pcm16 => {
                // Generate 16-bit PCM sine wave
                let mut samples = Vec::with_capacity(320); // 160 samples * 2 bytes
                for i in 0..160 {
                    let sample =
                        (2.0 * std::f32::consts::PI * 1000.0 * (frame_number * 160 + i) as f32
                            / 8000.0)
                            .sin();
                    let pcm = (sample * 32767.0) as i16;
                    samples.extend_from_slice(&pcm.to_be_bytes());
                }
                samples
            }
        }
    }

    /// Send RTP packet
    async fn send_rtp_packet(&mut self, frame_number: u32) -> Result<()> {
        if let Some(remote_addr) = self.remote_addr {
            let payload = self.generate_test_audio(frame_number);

            let packet = RtpPacket::new(
                self.codec.payload_type(),
                self.sequence_number,
                self.timestamp,
                self.ssrc,
                payload,
            );

            let packet_data = packet.serialize()?;
            self.socket.send_to(&packet_data, remote_addr).await?;

            self.stats.update_sent(&packet);
            self.sequence_number = self.sequence_number.wrapping_add(1);

            // Update timestamp based on codec
            let samples_per_frame = match self.codec {
                RtpAudioCodec::G711Ulaw | RtpAudioCodec::G711Alaw => 160, // 20ms @ 8kHz
                RtpAudioCodec::G729 => 80,                                // 10ms @ 8kHz
                RtpAudioCodec::G722 => 160,                               // 20ms @ 8kHz RTP clock
                RtpAudioCodec::Pcm16 => 160,                              // 20ms @ 8kHz
            };
            self.timestamp = self.timestamp.wrapping_add(samples_per_frame);
        }

        Ok(())
    }

    /// Receive RTP packets for specified duration
    async fn receive_rtp_packets(&mut self, duration: Duration) -> Result<Vec<RtpPacket>> {
        let mut packets = Vec::new();
        let start_time = Instant::now();
        let mut buffer = vec![0u8; 2048];

        while start_time.elapsed() < duration {
            match timeout(
                Duration::from_millis(100),
                self.socket.recv_from(&mut buffer),
            )
            .await
            {
                Ok(Ok((len, _from))) => {
                    if let Ok(packet) = RtpPacket::parse(&buffer[..len]) {
                        self.stats.update_received(&packet);
                        packets.push(packet);
                    }
                }
                Ok(Err(_)) => break,
                Err(_) => continue, // Timeout, keep trying
            }
        }

        Ok(packets)
    }

    /// Linear to μ-law conversion
    fn linear_to_ulaw(pcm: i16) -> u8 {
        const BIAS: i16 = 0x84;
        const CLIP: i16 = 32635;

        let mut sample = pcm;
        if sample < 0 {
            sample = -sample;
        }
        if sample > CLIP {
            sample = CLIP;
        }

        sample += BIAS;
        let exponent = (sample >> 7) & 0xF;
        let mantissa = (sample >> (exponent + 3)) & 0xF;
        let ulaw = ((exponent << 4) | mantissa) as u8;

        if pcm < 0 {
            ulaw
        } else {
            !ulaw
        }
    }

    /// Linear to A-law conversion  
    fn linear_to_alaw(pcm: i16) -> u8 {
        const CLIP: i16 = 32635;

        let mut sample = pcm;
        let sign = if sample < 0 {
            sample = -sample;
            0x80
        } else {
            0x00
        };

        if sample > CLIP {
            sample = CLIP;
        }

        let exponent = if sample >= 256 {
            let mut exp = 7;
            let mut temp = sample >> 8;
            while temp > 1 {
                temp >>= 1;
                exp -= 1;
            }
            exp
        } else {
            0
        };

        let mantissa = (sample >> (exponent + 4)) & 0xF;
        (sign | (exponent << 4) | mantissa) ^ 0x55
    }
}

/// Generate SDP offer for endpoint
fn create_sdp_offer(endpoint: &SipEndpoint, session_id: &str) -> SdpSession {
    let mut codecs = vec![CodecInfo {
        payload_type: endpoint.codec.payload_type(),
        name: match endpoint.codec {
            RtpAudioCodec::G711Ulaw => "PCMU".to_string(),
            RtpAudioCodec::G711Alaw => "PCMA".to_string(),
            RtpAudioCodec::G729 => "G729".to_string(),
            RtpAudioCodec::G722 => "G722".to_string(),
            RtpAudioCodec::Pcm16 => "L16".to_string(),
        },
        clock_rate: endpoint.codec.sample_rate(),
        channels: Some(1),
        format_parameters: HashMap::new(),
    }];

    let mut media = MediaDescription {
        media_type: MediaType::Audio,
        port: endpoint.local_addr.port(),
        num_ports: None,
        protocol: "RTP/AVP".to_string(),
        formats: vec![endpoint.codec.payload_type().to_string()],
        connection: Some(ConnectionData {
            network_type: "IN".to_string(),
            address_type: "IP4".to_string(),
            address: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            ttl: None,
            num_addresses: None,
        }),
        bandwidth: Vec::new(),
        encryption_key: None,
        attributes: HashMap::new(),
        codecs,
    };

    // Add rtpmap attribute
    let rtpmap_key = format!("rtpmap:{}", endpoint.codec.payload_type());
    let rtpmap_value = format!("{}/{}/1", media.codecs[0].name, media.codecs[0].clock_rate);
    media.attributes.insert(rtpmap_key, Some(rtpmap_value));

    SdpSession {
        version: 0,
        origin: OriginField {
            username: endpoint.name.clone(),
            session_id: session_id.to_string(),
            session_version: "1".to_string(),
            network_type: "IN".to_string(),
            address_type: "IP4".to_string(),
            address: "127.0.0.1".to_string(),
        },
        session_name: format!("{} Call", endpoint.name),
        session_info: None,
        uri: None,
        email: Vec::new(),
        phone: Vec::new(),
        connection: Some(ConnectionData {
            network_type: "IN".to_string(),
            address_type: "IP4".to_string(),
            address: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            ttl: None,
            num_addresses: None,
        }),
        bandwidth: Vec::new(),
        times: vec![TimeDescription {
            start_time: 0,
            stop_time: 0,
            repeat_times: Vec::new(),
        }],
        encryption_key: None,
        attributes: HashMap::new(),
        media: vec![media],
    }
}

/// Test direct media passthrough (same codecs, no relay)
#[tokio::test]
async fn test_direct_media_passthrough() -> Result<()> {
    let config =
        CallBridgingTestConfig::direct_passthrough("G.711 μ-law Direct", RtpAudioCodec::G711Ulaw);

    info!("Starting test: {}", config.name);

    // Create endpoints
    let mut caller = SipEndpoint::new("Caller", config.caller_codec).await?;
    let mut callee = SipEndpoint::new("Callee", config.callee_codec).await?;

    // Direct connection (no B2BUA)
    caller.connect_to(callee.local_addr);
    callee.connect_to(caller.local_addr);

    // Create SDP offers
    let caller_sdp = create_sdp_offer(&caller, "caller-session");
    let callee_sdp = create_sdp_offer(&callee, "callee-session");

    // Verify SDP negotiation
    let common_codecs = caller_sdp.find_common_codecs(&callee_sdp);
    assert!(!common_codecs.is_empty(), "No common codecs found");
    assert_eq!(common_codecs[0].name, "PCMU");

    // Start media exchange
    let test_duration = Duration::from_secs(config.test_duration);

    // Spawn packet sending task
    let mut caller_clone = SipEndpoint::new("CallerSender", config.caller_codec).await?;
    caller_clone.connect_to(callee.local_addr);

    let send_task = tokio::spawn(async move {
        for i in 0..config.packet_count {
            caller_clone.send_rtp_packet(i).await.unwrap();
            sleep(Duration::from_millis(20)).await; // 20ms packets
        }
    });

    // Receive packets
    let received_packets = callee.receive_rtp_packets(test_duration).await?;

    // Wait for sending to complete
    send_task.await?;

    // Verify results
    assert!(received_packets.len() > 0, "No packets received");
    assert!(
        received_packets.len() >= (config.packet_count as f32 * 0.8) as usize,
        "Significant packet loss: got {}, expected ~{}",
        received_packets.len(),
        config.packet_count
    );

    // Verify packet contents
    for packet in &received_packets {
        assert_eq!(packet.payload_type, config.caller_codec.payload_type());
        assert_eq!(packet.version, 2);
        assert!(packet.is_valid());
    }

    info!(
        "✅ Direct passthrough test completed: {}/{} packets received",
        received_packets.len(),
        config.packet_count
    );

    Ok(())
}

/// Test media relay with codec transcoding
#[tokio::test]
async fn test_media_relay_with_transcoding() -> Result<()> {
    let config = CallBridgingTestConfig::codec_transcoding(
        "G.711 μ-law to A-law Transcoding",
        RtpAudioCodec::G711Ulaw,
        RtpAudioCodec::G711Alaw,
    );

    info!("Starting test: {}", config.name);

    // Create RTP proxy service
    let proxy_config = RtpProxyConfig {
        enabled: true,
        codec_translation: true,
        port_range: (20000, 25000),
        ..Default::default()
    };
    let rtp_proxy = RtpProxyService::new(proxy_config).await?;

    // Create endpoints
    let mut caller = SipEndpoint::new("Caller", config.caller_codec).await?;
    let mut callee = SipEndpoint::new("Callee", config.callee_codec).await?;

    // Start media session through proxy
    let session_id = format!("test-session-{}", rand::random::<u32>());
    let call_id = format!("test-call-{}", rand::random::<u32>());

    let (caller_proxy_addr, callee_proxy_addr) = rtp_proxy
        .start_session(
            session_id.clone(),
            call_id.clone(),
            caller.local_addr,
            callee.local_addr,
            config.caller_codec,
            config.callee_codec,
        )
        .await?;

    // Connect endpoints to proxy
    caller.connect_to(caller_proxy_addr);
    callee.connect_to(callee_proxy_addr);

    // Create SDP with transcoding capabilities
    let caller_sdp = create_sdp_offer(&caller, &session_id);
    let callee_sdp = create_sdp_offer(&callee, &session_id);

    // Start media exchange
    let test_duration = Duration::from_secs(config.test_duration);

    // Spawn packet sending task
    let mut caller_clone = SipEndpoint::new("CallerSender", config.caller_codec).await?;
    caller_clone.connect_to(caller_proxy_addr);

    let send_task = tokio::spawn(async move {
        for i in 0..config.packet_count {
            caller_clone.send_rtp_packet(i).await.unwrap();
            sleep(Duration::from_millis(20)).await;
        }
    });

    // Receive transcoded packets
    let received_packets = callee.receive_rtp_packets(test_duration).await?;

    // Wait for sending to complete
    send_task.await?;

    // Verify transcoding occurred
    assert!(received_packets.len() > 0, "No transcoded packets received");

    for packet in &received_packets {
        // Should receive A-law packets (payload type 8) from μ-law input (payload type 0)
        assert_eq!(packet.payload_type, config.callee_codec.payload_type());
        assert_eq!(packet.version, 2);
        assert!(packet.is_valid());
    }

    // Get session stats
    let stats = rtp_proxy.get_session_stats(&session_id).unwrap();
    assert!(stats.packets_transcoded > 0, "No transcoding occurred");

    // Clean up
    rtp_proxy.end_session(&session_id).await?;

    info!(
        "✅ Transcoding test completed: {}/{} packets transcoded",
        received_packets.len(),
        config.packet_count
    );

    Ok(())
}

/// Test G.729 codec encoding/decoding through relay
#[tokio::test]
async fn test_g729_codec_relay() -> Result<()> {
    let config = CallBridgingTestConfig::codec_transcoding(
        "G.711 to G.729 Transcoding",
        RtpAudioCodec::G711Ulaw,
        RtpAudioCodec::G729,
    );

    info!("Starting test: {}", config.name);

    // Test G.729 codec directly first
    let mut g729_codec = G729Codec::new();

    // Generate test speech samples (80 samples for 10ms frame)
    let mut test_speech = Vec::with_capacity(G729_FRAME_SIZE);
    for i in 0..G729_FRAME_SIZE {
        let sample =
            (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / G729_SAMPLE_RATE as f32).sin();
        test_speech.push((sample * 16384.0) as i16);
    }

    // Test encode/decode
    let encoded = g729_codec.encode(&test_speech)?;
    assert_eq!(encoded.len(), 10, "G.729 frame should be 10 bytes");

    let decoded = g729_codec.decode(&encoded)?;
    assert_eq!(
        decoded.len(),
        G729_FRAME_SIZE,
        "Decoded frame should be 80 samples"
    );

    // Verify decoded signal has reasonable amplitude
    let max_amplitude = decoded.iter().map(|&x| x.abs()).max().unwrap_or(0);
    assert!(
        max_amplitude > 100,
        "Decoded signal should have reasonable amplitude"
    );

    info!("✅ G.729 codec test passed");

    // Now test through RTP proxy
    let proxy_config = RtpProxyConfig {
        enabled: true,
        codec_translation: true,
        port_range: (25000, 30000),
        ..Default::default()
    };
    let rtp_proxy = RtpProxyService::new(proxy_config).await?;

    let mut caller = SipEndpoint::new("G711Caller", config.caller_codec).await?;
    let mut callee = SipEndpoint::new("G729Callee", config.callee_codec).await?;

    let session_id = format!("g729-session-{}", rand::random::<u32>());
    let call_id = format!("g729-call-{}", rand::random::<u32>());

    let (caller_proxy_addr, callee_proxy_addr) = rtp_proxy
        .start_session(
            session_id.clone(),
            call_id,
            caller.local_addr,
            callee.local_addr,
            config.caller_codec,
            config.callee_codec,
        )
        .await?;

    caller.connect_to(caller_proxy_addr);
    callee.connect_to(callee_proxy_addr);

    // Send fewer packets for G.729 test (more intensive)
    let packet_count = 20;
    let mut caller_clone = SipEndpoint::new("G711Sender", config.caller_codec).await?;
    caller_clone.connect_to(caller_proxy_addr);

    let send_task = tokio::spawn(async move {
        for i in 0..packet_count {
            caller_clone.send_rtp_packet(i).await.unwrap();
            sleep(Duration::from_millis(20)).await;
        }
    });

    let received_packets = callee.receive_rtp_packets(Duration::from_secs(3)).await?;
    send_task.await?;

    // Verify G.729 packets received
    assert!(received_packets.len() > 0, "No G.729 packets received");

    for packet in &received_packets {
        assert_eq!(packet.payload_type, 18); // G.729 payload type
                                             // G.729 frames should be 10 bytes
        assert_eq!(packet.payload.len(), 10, "G.729 payload should be 10 bytes");
    }

    let stats = rtp_proxy.get_session_stats(&session_id).unwrap();
    assert!(
        stats.packets_transcoded > 0,
        "No G.729 transcoding occurred"
    );

    rtp_proxy.end_session(&session_id).await?;

    info!(
        "✅ G.729 relay test completed: {} packets transcoded",
        received_packets.len()
    );

    Ok(())
}

/// Test multiple concurrent calls
#[tokio::test]
async fn test_concurrent_call_bridging() -> Result<()> {
    info!("Starting concurrent call bridging test");

    let proxy_config = RtpProxyConfig {
        enabled: true,
        codec_translation: true,
        max_sessions: 100,
        port_range: (30000, 35000),
        ..Default::default()
    };
    let rtp_proxy = Arc::new(RtpProxyService::new(proxy_config).await?);

    let num_calls = 3;
    let mut join_handles = Vec::new();

    for call_num in 0..num_calls {
        let proxy = rtp_proxy.clone();
        let handle = tokio::spawn(async move {
            let session_id = format!("concurrent-session-{}", call_num);
            let call_id = format!("concurrent-call-{}", call_num);

            let mut caller =
                SipEndpoint::new(&format!("Caller{}", call_num), RtpAudioCodec::G711Ulaw).await?;
            let mut callee =
                SipEndpoint::new(&format!("Callee{}", call_num), RtpAudioCodec::G711Alaw).await?;

            let (caller_proxy_addr, callee_proxy_addr) = proxy
                .start_session(
                    session_id.clone(),
                    call_id,
                    caller.local_addr,
                    callee.local_addr,
                    RtpAudioCodec::G711Ulaw,
                    RtpAudioCodec::G711Alaw,
                )
                .await?;

            caller.connect_to(caller_proxy_addr);
            callee.connect_to(callee_proxy_addr);

            // Send packets concurrently
            let mut caller_clone = SipEndpoint::new(
                &format!("CallerSender{}", call_num),
                RtpAudioCodec::G711Ulaw,
            )
            .await?;
            caller_clone.connect_to(caller_proxy_addr);

            let send_task = tokio::spawn(async move {
                for i in 0..20 {
                    caller_clone.send_rtp_packet(i).await.unwrap();
                    sleep(Duration::from_millis(50)).await;
                }
            });

            let received_packets = callee.receive_rtp_packets(Duration::from_secs(2)).await?;
            send_task.await?;

            // Verify transcoding
            assert!(
                received_packets.len() > 0,
                "Call {} received no packets",
                call_num
            );
            for packet in &received_packets {
                assert_eq!(packet.payload_type, 8); // A-law
            }

            proxy.end_session(&session_id).await?;

            info!(
                "✅ Concurrent call {} completed: {} packets",
                call_num,
                received_packets.len()
            );
            Ok::<(), anyhow::Error>(())
        });

        join_handles.push(handle);
    }

    // Wait for all calls to complete
    for handle in join_handles {
        handle.await??;
    }

    // Verify no active sessions remain
    let active_sessions = rtp_proxy.get_active_sessions();
    assert_eq!(active_sessions.len(), 0, "Sessions not properly cleaned up");

    info!(
        "✅ All {} concurrent calls completed successfully",
        num_calls
    );

    Ok(())
}

/// Test DTMF relay functionality
#[tokio::test]
async fn test_dtmf_relay() -> Result<()> {
    info!("Starting DTMF relay test");

    let proxy_config = RtpProxyConfig {
        enabled: true,
        dtmf_relay: true,
        port_range: (35000, 40000),
        ..Default::default()
    };
    let rtp_proxy = RtpProxyService::new(proxy_config).await?;

    let mut caller = SipEndpoint::new("DTMFCaller", RtpAudioCodec::G711Ulaw).await?;
    let mut callee = SipEndpoint::new("DTMFCallee", RtpAudioCodec::G711Ulaw).await?;

    let session_id = "dtmf-test-session".to_string();
    let call_id = "dtmf-test-call".to_string();

    let (caller_proxy_addr, callee_proxy_addr) = rtp_proxy
        .start_session(
            session_id.clone(),
            call_id,
            caller.local_addr,
            callee.local_addr,
            RtpAudioCodec::G711Ulaw,
            RtpAudioCodec::G711Ulaw,
        )
        .await?;

    caller.connect_to(caller_proxy_addr);
    callee.connect_to(callee_proxy_addr);

    // Send DTMF event (RFC 4733)
    let dtmf_payload = vec![
        0x05, // Event: digit '5'
        0x0A, // End=0, Volume=10
        0x00, 0x50, // Duration=80 (10ms @ 8kHz)
    ];

    let dtmf_packet = RtpPacket::new(
        101, // DTMF payload type
        1234,
        8000,
        caller.ssrc,
        dtmf_payload,
    );

    let packet_data = dtmf_packet.serialize()?;
    caller
        .socket
        .send_to(&packet_data, caller_proxy_addr)
        .await?;

    // Receive DTMF event
    let received_packets = callee.receive_rtp_packets(Duration::from_secs(1)).await?;

    // Verify DTMF was relayed
    let dtmf_packets: Vec<_> = received_packets
        .iter()
        .filter(|p| p.payload_type == 101)
        .collect();

    assert!(dtmf_packets.len() > 0, "DTMF event not relayed");

    let dtmf_event = &dtmf_packets[0];
    assert_eq!(dtmf_event.payload[0], 0x05, "DTMF digit not preserved");

    // Check session stats
    let stats = rtp_proxy.get_session_stats(&session_id).unwrap();
    assert!(stats.dtmf_events > 0, "DTMF events not counted");

    rtp_proxy.end_session(&session_id).await?;

    info!(
        "✅ DTMF relay test completed: {} DTMF events processed",
        stats.dtmf_events
    );

    Ok(())
}
