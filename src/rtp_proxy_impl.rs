/*
 * RTP Proxy Implementation
 * Handles RTP packet forwarding, codec translation, and media session management
 */

use crate::rtp::{RtpPacket, RtpStats};
use anyhow::{anyhow, Result};
use dashmap::DashMap;
use redfire_codec_engine::{AudioCodec as CodecAudioCodec, AudioFrame, CodecConfig, CodecService};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{debug, error, info, instrument, warn};

/// RTP proxy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtpProxyConfig {
    /// Enable RTP proxy
    pub enabled: bool,
    /// Maximum number of concurrent media sessions
    pub max_sessions: u32,
    /// RTP timeout in seconds
    pub rtp_timeout: u64,
    /// Enable codec translation
    pub codec_translation: bool,
    /// Jitter buffer size in packets
    pub jitter_buffer_size: usize,
    /// Maximum jitter buffer delay in milliseconds
    pub max_jitter_delay: u64,
    /// Enable DTMF relay
    pub dtmf_relay: bool,
    /// Port range for RTP allocation
    pub port_range: (u16, u16),
}

impl Default for RtpProxyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_sessions: 1000,
            rtp_timeout: 30,
            codec_translation: true,
            jitter_buffer_size: 50,
            max_jitter_delay: 200,
            dtmf_relay: true,
            port_range: (10000, 20000),
        }
    }
}

/// Media session direction
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MediaDirection {
    Ingress, // Incoming media
    Egress,  // Outgoing media
}

/// Media session state
#[derive(Debug, Clone)]
pub struct MediaSession {
    pub session_id: String,
    pub call_id: String,
    pub ingress_endpoint: MediaEndpoint,
    pub egress_endpoint: MediaEndpoint,
    pub created_at: Instant,
    pub last_activity: Instant,
    pub stats: MediaSessionStats,
    pub codec_translation: Option<CodecTranslation>,
}

/// Media endpoint information
#[derive(Debug, Clone)]
pub struct MediaEndpoint {
    pub remote_addr: SocketAddr,
    pub local_addr: SocketAddr,
    pub codec: AudioCodec,
    pub ssrc: u32,
    pub last_sequence: u16,
    pub last_timestamp: u32,
    pub jitter_buffer: JitterBuffer,
}

impl Default for MediaEndpoint {
    fn default() -> Self {
        Self {
            remote_addr: "0.0.0.0:0".parse().unwrap(),
            local_addr: "0.0.0.0:0".parse().unwrap(),
            codec: AudioCodec::G711Ulaw,
            ssrc: 0,
            last_sequence: 0,
            last_timestamp: 0,
            jitter_buffer: JitterBuffer::default(),
        }
    }
}

/// Audio codec enum for RTP proxy (matches codec module)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AudioCodec {
    G711Ulaw,
    G711Alaw,
    G729,
    G722,
    Pcm16,
}

impl From<redfire_codec_engine::AudioCodec> for AudioCodec {
    fn from(codec: redfire_codec_engine::AudioCodec) -> Self {
        match codec {
            redfire_codec_engine::AudioCodec::G711Ulaw => AudioCodec::G711Ulaw,
            redfire_codec_engine::AudioCodec::G711Alaw => AudioCodec::G711Alaw,
            redfire_codec_engine::AudioCodec::G729 => AudioCodec::G729,
            redfire_codec_engine::AudioCodec::G729AnnexA => AudioCodec::G729,
            redfire_codec_engine::AudioCodec::G729AnnexB => AudioCodec::G729,
            redfire_codec_engine::AudioCodec::Pcm16 => AudioCodec::Pcm16,
            redfire_codec_engine::AudioCodec::G722 => AudioCodec::G722,
            redfire_codec_engine::AudioCodec::G7222 => AudioCodec::G722, // Map G.722.2 to G.722
            redfire_codec_engine::AudioCodec::Opus => AudioCodec::Pcm16, // Map Opus to PCM16
        }
    }
}

impl From<AudioCodec> for CodecAudioCodec {
    fn from(codec: AudioCodec) -> Self {
        match codec {
            AudioCodec::G711Ulaw => CodecAudioCodec::G711Ulaw,
            AudioCodec::G711Alaw => CodecAudioCodec::G711Alaw,
            AudioCodec::G729 => CodecAudioCodec::G729,
            AudioCodec::G722 => CodecAudioCodec::G722,
            AudioCodec::Pcm16 => CodecAudioCodec::Pcm16,
        }
    }
}

impl AudioCodec {
    pub fn payload_type(self) -> u8 {
        match self {
            AudioCodec::G711Ulaw => 0,
            AudioCodec::G711Alaw => 8,
            AudioCodec::G729 => 18,
            AudioCodec::G722 => 9,
            AudioCodec::Pcm16 => 10,
        }
    }

    pub fn from_payload_type(pt: u8) -> Option<Self> {
        match pt {
            0 => Some(AudioCodec::G711Ulaw),
            8 => Some(AudioCodec::G711Alaw),
            18 => Some(AudioCodec::G729),
            9 => Some(AudioCodec::G722),
            10 => Some(AudioCodec::Pcm16),
            _ => None,
        }
    }

    pub fn sample_rate(self) -> u32 {
        match self {
            AudioCodec::G711Ulaw | AudioCodec::G711Alaw => 8000,
            AudioCodec::G729 => 8000,
            AudioCodec::G722 => 16000,
            AudioCodec::Pcm16 => 8000,
        }
    }
}

/// Codec translation configuration
#[derive(Debug, Clone)]
pub struct CodecTranslation {
    pub from_codec: AudioCodec,
    pub to_codec: AudioCodec,
    pub session_id: String,
}

impl Default for CodecTranslation {
    fn default() -> Self {
        Self {
            from_codec: AudioCodec::G711Ulaw,
            to_codec: AudioCodec::G711Ulaw,
            session_id: String::new(),
        }
    }
}

/// Jitter buffer for packet reordering and delay compensation
#[derive(Debug, Clone)]
pub struct JitterBuffer {
    packets: HashMap<u16, (RtpPacket, Instant)>,
    max_size: usize,
    max_delay: Duration,
    next_sequence: u16,
    initialized: bool,
}

impl Default for JitterBuffer {
    fn default() -> Self {
        Self {
            packets: HashMap::new(),
            max_size: 100,
            max_delay: Duration::from_millis(200),
            next_sequence: 0,
            initialized: false,
        }
    }
}

impl JitterBuffer {
    pub fn new(max_size: usize, max_delay_ms: u64) -> Self {
        Self {
            packets: HashMap::new(),
            max_size,
            max_delay: Duration::from_millis(max_delay_ms),
            next_sequence: 0,
            initialized: false,
        }
    }

    /// Add packet to jitter buffer
    pub fn add_packet(&mut self, packet: RtpPacket) -> Vec<RtpPacket> {
        let now = Instant::now();
        let sequence = packet.sequence_number;

        if !self.initialized {
            self.next_sequence = sequence.wrapping_add(1);
            self.initialized = true;
            return vec![packet]; // First packet, no buffering needed
        }

        // Store packet
        self.packets.insert(sequence, (packet, now));

        // Clean up old packets
        self.cleanup_old_packets(now);

        // Extract ready packets in sequence
        self.extract_ready_packets()
    }

    fn cleanup_old_packets(&mut self, now: Instant) {
        self.packets
            .retain(|_, (_, timestamp)| now.duration_since(*timestamp) < self.max_delay);

        // If buffer is too large, remove oldest packets
        if self.packets.len() > self.max_size {
            let mut to_remove: Vec<u16> = self.packets.keys().cloned().collect();
            to_remove.sort();

            let excess = self.packets.len() - self.max_size;
            for seq in to_remove.into_iter().take(excess) {
                self.packets.remove(&seq);
            }
        }
    }

    fn extract_ready_packets(&mut self) -> Vec<RtpPacket> {
        let mut ready_packets = Vec::new();

        loop {
            if let Some((packet, _)) = self.packets.remove(&self.next_sequence) {
                ready_packets.push(packet);
                self.next_sequence = self.next_sequence.wrapping_add(1);
            } else {
                break;
            }
        }

        ready_packets
    }
}

/// Media session statistics
#[derive(Debug, Clone, Default)]
pub struct MediaSessionStats {
    pub ingress_stats: RtpStats,
    pub egress_stats: RtpStats,
    pub packets_transcoded: u64,
    pub dtmf_events: u64,
    pub jitter_buffer_overruns: u64,
    pub jitter_buffer_underruns: u64,
}

/// Main RTP proxy service
pub struct RtpProxyService {
    config: RtpProxyConfig,
    sessions: Arc<DashMap<String, MediaSession>>,
    codec_service: Arc<CodecService>,
    port_allocator: Arc<RwLock<PortAllocator>>,
    sockets: Arc<DashMap<SocketAddr, Arc<UdpSocket>>>,
}

/// Port allocation for RTP sessions
#[derive(Debug)]
struct PortAllocator {
    range: (u16, u16),
    allocated: HashMap<u16, String>, // port -> session_id
    next_port: u16,
}

impl PortAllocator {
    fn new(range: (u16, u16)) -> Self {
        Self {
            range,
            allocated: HashMap::new(),
            next_port: range.0,
        }
    }

    fn allocate_port(&mut self, session_id: &str) -> Option<u16> {
        let start_port = self.next_port;

        loop {
            if !self.allocated.contains_key(&self.next_port) {
                let port = self.next_port;
                self.allocated.insert(port, session_id.to_string());
                self.next_port = if self.next_port >= self.range.1 {
                    self.range.0
                } else {
                    self.next_port + 2 // RTP uses even ports, RTCP uses odd
                };
                return Some(port);
            }

            self.next_port = if self.next_port >= self.range.1 {
                self.range.0
            } else {
                self.next_port + 2
            };

            if self.next_port == start_port {
                return None; // Full circle, no ports available
            }
        }
    }

    fn deallocate_port(&mut self, port: u16) {
        self.allocated.remove(&port);
    }
}

impl RtpProxyService {
    /// Create new RTP proxy service
    pub async fn new(config: RtpProxyConfig) -> Result<Self> {
        let codec_config = CodecConfig::default();
        let codec_service = Arc::new(CodecService::new(codec_config).await?);

        let port_allocator = Arc::new(RwLock::new(PortAllocator::new(config.port_range)));

        Ok(Self {
            config,
            sessions: Arc::new(DashMap::new()),
            codec_service,
            port_allocator,
            sockets: Arc::new(DashMap::new()),
        })
    }

    /// Start a new media session
    #[instrument(skip(self))]
    pub async fn start_session(
        &self,
        session_id: String,
        call_id: String,
        ingress_remote: SocketAddr,
        egress_remote: SocketAddr,
        ingress_codec: AudioCodec,
        egress_codec: AudioCodec,
    ) -> Result<(SocketAddr, SocketAddr)> {
        // Allocate ports for RTP
        let mut port_allocator = self.port_allocator.write().await;
        let ingress_port = port_allocator
            .allocate_port(&session_id)
            .ok_or_else(|| anyhow!("No available ports for RTP session"))?;
        let egress_port = port_allocator
            .allocate_port(&session_id)
            .ok_or_else(|| anyhow!("No available ports for RTP session"))?;
        drop(port_allocator);

        // Bind UDP sockets
        let ingress_addr = SocketAddr::new("0.0.0.0".parse()?, ingress_port);
        let egress_addr = SocketAddr::new("0.0.0.0".parse()?, egress_port);

        let ingress_socket = Arc::new(UdpSocket::bind(ingress_addr).await?);
        let egress_socket = Arc::new(UdpSocket::bind(egress_addr).await?);

        let ingress_local = ingress_socket.local_addr()?;
        let egress_local = egress_socket.local_addr()?;

        // Store sockets
        self.sockets.insert(ingress_local, ingress_socket.clone());
        self.sockets.insert(egress_local, egress_socket.clone());

        // Set up codec translation if needed
        let codec_translation = if self.config.codec_translation && ingress_codec != egress_codec {
            // Start codec translation session
            self.codec_service
                .start_session(
                    format!("{}-codec", session_id),
                    ingress_codec.into(),
                    egress_codec.into(),
                    ingress_codec.sample_rate(),
                    1, // Mono
                )
                .await?;

            Some(CodecTranslation {
                from_codec: ingress_codec,
                to_codec: egress_codec,
                session_id: format!("{}-codec", session_id),
            })
        } else {
            None
        };

        // Create media session
        let session = MediaSession {
            session_id: session_id.clone(),
            call_id: call_id.clone(),
            ingress_endpoint: MediaEndpoint {
                remote_addr: ingress_remote,
                local_addr: ingress_local,
                codec: ingress_codec,
                ssrc: 0, // Will be set when first packet arrives
                last_sequence: 0,
                last_timestamp: 0,
                jitter_buffer: JitterBuffer::new(
                    self.config.jitter_buffer_size,
                    self.config.max_jitter_delay,
                ),
            },
            egress_endpoint: MediaEndpoint {
                remote_addr: egress_remote,
                local_addr: egress_local,
                codec: egress_codec,
                ssrc: 0,
                last_sequence: 0,
                last_timestamp: 0,
                jitter_buffer: JitterBuffer::new(
                    self.config.jitter_buffer_size,
                    self.config.max_jitter_delay,
                ),
            },
            created_at: Instant::now(),
            last_activity: Instant::now(),
            stats: MediaSessionStats::default(),
            codec_translation,
        };

        // Store session
        self.sessions.insert(session_id.clone(), session);

        // Start packet forwarding tasks
        self.start_forwarding_task(session_id.clone(), MediaDirection::Ingress, ingress_socket)
            .await;
        self.start_forwarding_task(session_id.clone(), MediaDirection::Egress, egress_socket)
            .await;

        info!(
            "Started RTP proxy session {} for call {}: {} <-> {}",
            session_id, call_id, ingress_local, egress_local
        );

        Ok((ingress_local, egress_local))
    }

    /// Start packet forwarding task for a direction
    async fn start_forwarding_task(
        &self,
        session_id: String,
        direction: MediaDirection,
        socket: Arc<UdpSocket>,
    ) {
        let sessions = Arc::clone(&self.sessions);
        let codec_service = Arc::clone(&self.codec_service);
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut buffer = vec![0u8; 2048];

            loop {
                match timeout(
                    Duration::from_secs(config.rtp_timeout),
                    socket.recv_from(&mut buffer),
                )
                .await
                {
                    Ok(Ok((len, from_addr))) => {
                        if let Err(e) = Self::handle_rtp_packet(
                            &sessions,
                            &codec_service,
                            &session_id,
                            direction,
                            &buffer[..len],
                            from_addr,
                            &socket,
                            &config,
                        )
                        .await
                        {
                            warn!("Error handling RTP packet: {}", e);
                        }
                    }
                    Ok(Err(e)) => {
                        error!("Socket error in RTP forwarding: {}", e);
                        break;
                    }
                    Err(_) => {
                        debug!("RTP timeout for session {}", session_id);
                        break;
                    }
                }
            }

            info!("RTP forwarding task ended for session {}", session_id);
        });
    }

    /// Handle incoming RTP packet
    async fn handle_rtp_packet(
        sessions: &DashMap<String, MediaSession>,
        codec_service: &CodecService,
        session_id: &str,
        direction: MediaDirection,
        packet_data: &[u8],
        from_addr: SocketAddr,
        socket: &UdpSocket,
        config: &RtpProxyConfig,
    ) -> Result<()> {
        // Parse RTP packet
        let rtp_packet = RtpPacket::parse(packet_data)?;

        // Get session
        let mut session = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow!("Session {} not found", session_id))?;

        // Update session activity
        session.last_activity = Instant::now();

        // Handle DTMF events (RFC 4733)
        if config.dtmf_relay && rtp_packet.payload_type >= 96 && rtp_packet.payload_type <= 127 {
            // DTMF event payload type range
            Self::handle_dtmf_event(&mut session, &rtp_packet, direction)?;
        }

        // Process packet through jitter buffer
        let (endpoint, target_socket) = match direction {
            MediaDirection::Ingress => {
                session.stats.ingress_stats.update_received(&rtp_packet);
                let packets = session
                    .ingress_endpoint
                    .jitter_buffer
                    .add_packet(rtp_packet.clone());
                let target_addr = session.egress_endpoint.remote_addr;
                (target_addr, &session.egress_endpoint.local_addr)
            }
            MediaDirection::Egress => {
                session.stats.egress_stats.update_received(&rtp_packet);
                let packets = session
                    .egress_endpoint
                    .jitter_buffer
                    .add_packet(rtp_packet.clone());
                let target_addr = session.ingress_endpoint.remote_addr;
                (target_addr, &session.ingress_endpoint.local_addr)
            }
        };

        // Forward packets (may be multiple due to jitter buffer)
        for packet in [rtp_packet].iter() {
            // Simplified - should use jitter buffer output
            let forwarded_packet = if let Some(ref translation) = session.codec_translation {
                // Transcode packet
                Self::transcode_packet(codec_service, translation, packet.clone()).await?
            } else {
                // Forward without transcoding
                packet.clone()
            };

            // Serialize and send
            let packet_bytes = forwarded_packet.serialize()?;
            if let Err(e) = socket.send_to(&packet_bytes, endpoint).await {
                warn!("Failed to forward RTP packet: {}", e);
            } else {
                match direction {
                    MediaDirection::Ingress => {
                        session.stats.egress_stats.update_sent(&forwarded_packet)
                    }
                    MediaDirection::Egress => {
                        session.stats.ingress_stats.update_sent(&forwarded_packet)
                    }
                }
            }
        }

        Ok(())
    }

    /// Transcode RTP packet
    async fn transcode_packet(
        codec_service: &CodecService,
        translation: &CodecTranslation,
        mut packet: RtpPacket,
    ) -> Result<RtpPacket> {
        // Create audio frame
        let audio_frame = AudioFrame {
            data: packet.payload.clone(),
            codec: translation.from_codec.into(),
            sample_rate: translation.from_codec.sample_rate(),
            channels: 1,
            timestamp: packet.timestamp,
            sequence: packet.sequence_number,
        };

        // Transcode
        let transcoded = codec_service
            .transcode_frame(&translation.session_id, audio_frame)
            .await?;

        // Update packet
        packet.payload = transcoded.data;
        packet.payload_type = translation.to_codec.payload_type();

        Ok(packet)
    }

    /// Handle DTMF event
    fn handle_dtmf_event(
        session: &mut MediaSession,
        packet: &RtpPacket,
        direction: MediaDirection,
    ) -> Result<()> {
        if packet.payload.len() >= 4 {
            let event = packet.payload[0];
            let end_flag = (packet.payload[1] & 0x80) != 0;
            let volume = packet.payload[1] & 0x3F;
            let duration = u16::from_be_bytes([packet.payload[2], packet.payload[3]]);

            debug!(
                "DTMF event: digit={}, end={}, volume={}, duration={}",
                event, end_flag, volume, duration
            );

            session.stats.dtmf_events += 1;
        }

        Ok(())
    }

    /// End media session
    pub async fn end_session(&self, session_id: &str) -> Result<MediaSessionStats> {
        let session = self
            .sessions
            .remove(session_id)
            .ok_or_else(|| anyhow!("Session {} not found", session_id))?;

        let (_, session) = session;

        // Clean up codec translation session
        if let Some(ref translation) = session.codec_translation {
            let _ = self
                .codec_service
                .end_session(&translation.session_id)
                .await;
        }

        // Clean up sockets
        self.sockets.remove(&session.ingress_endpoint.local_addr);
        self.sockets.remove(&session.egress_endpoint.local_addr);

        // Deallocate ports
        let mut port_allocator = self.port_allocator.write().await;
        port_allocator.deallocate_port(session.ingress_endpoint.local_addr.port());
        port_allocator.deallocate_port(session.egress_endpoint.local_addr.port());

        info!(
            "Ended RTP proxy session {} for call {}",
            session_id, session.call_id
        );

        Ok(session.stats)
    }

    /// Get session statistics
    pub fn get_session_stats(&self, session_id: &str) -> Option<MediaSessionStats> {
        self.sessions
            .get(session_id)
            .map(|session| session.stats.clone())
    }

    /// Get all active sessions
    pub fn get_active_sessions(&self) -> Vec<String> {
        self.sessions
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Clean up expired sessions
    pub async fn cleanup_expired_sessions(&self) {
        let timeout_duration = Duration::from_secs(self.config.rtp_timeout);
        let now = Instant::now();

        let expired_sessions: Vec<String> = self
            .sessions
            .iter()
            .filter_map(|entry| {
                let session = entry.value();
                if now.duration_since(session.last_activity) > timeout_duration {
                    Some(entry.key().clone())
                } else {
                    None
                }
            })
            .collect();

        for session_id in expired_sessions {
            if let Err(e) = self.end_session(&session_id).await {
                warn!("Error cleaning up expired session {}: {}", session_id, e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jitter_buffer() {
        let mut buffer = JitterBuffer::new(10, 100);

        let packet1 = RtpPacket::new(0, 100, 1000, 1, vec![1, 2, 3]);
        let packet2 = RtpPacket::new(0, 101, 1020, 1, vec![4, 5, 6]);
        let packet3 = RtpPacket::new(0, 102, 1040, 1, vec![7, 8, 9]);

        // First packet should be delivered immediately
        let result1 = buffer.add_packet(packet1);
        assert_eq!(result1.len(), 1);

        // Second packet in sequence
        let result2 = buffer.add_packet(packet2);
        assert_eq!(result2.len(), 1);

        // Third packet in sequence
        let result3 = buffer.add_packet(packet3);
        assert_eq!(result3.len(), 1);
    }

    #[test]
    fn test_jitter_buffer_reordering() {
        let mut buffer = JitterBuffer::new(10, 100);

        let packet1 = RtpPacket::new(0, 100, 1000, 1, vec![1, 2, 3]);
        let packet3 = RtpPacket::new(0, 102, 1040, 1, vec![7, 8, 9]);
        let packet2 = RtpPacket::new(0, 101, 1020, 1, vec![4, 5, 6]);

        // First packet
        let result1 = buffer.add_packet(packet1);
        assert_eq!(result1.len(), 1);

        // Out of order packet (102 before 101)
        let result2 = buffer.add_packet(packet3);
        assert_eq!(result2.len(), 0); // Should be buffered

        // Missing packet arrives
        let result3 = buffer.add_packet(packet2);
        assert_eq!(result3.len(), 2); // Should deliver both 101 and 102
    }

    #[tokio::test]
    async fn test_port_allocator() {
        let mut allocator = PortAllocator::new((10000, 10010));

        let port1 = allocator.allocate_port("session1").unwrap();
        let port2 = allocator.allocate_port("session2").unwrap();

        assert_ne!(port1, port2);
        assert!(port1 >= 10000 && port1 <= 10010);
        assert!(port2 >= 10000 && port2 <= 10010);

        allocator.deallocate_port(port1);
        let port3 = allocator.allocate_port("session3").unwrap();
        // Should reuse the deallocated port eventually
    }
}
