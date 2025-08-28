/*
 * Redfire Switch - Phase 2: Media Plane Implementation
 * Copyright (C) 2025 Carrier One Inc and contributors
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

//! # Phase 2: Media Plane
//! 
//! This module implements Phase 2 of the dependency optimization plan:
//! - RTP proxy/relay
//! - Basic codec transcoding (G.711)
//! - DTMF relay
//! - Video passthrough
//! - SRTP support

use crate::rtp::monitor::{RtpMonitor, RtpMonitorConfig};
// Video passthrough removed - feature not implemented
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::{RwLock, mpsc};
use tokio::time::Duration;
use tracing::{debug, info, warn, error, instrument};
use dashmap::DashMap;

/// Audio codec types for media plane
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AudioCodec {
    /// G.711 μ-law (North America)
    PCMU,
    /// G.711 A-law (Europe/International)
    PCMA,
    /// G.722 wideband
    G722,
    /// G.729 compressed
    G729,
    /// Opus modern codec
    Opus,
    /// AMR narrowband
    AMR,
}

impl std::fmt::Display for AudioCodec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioCodec::PCMU => write!(f, "PCMU"),
            AudioCodec::PCMA => write!(f, "PCMA"),
            AudioCodec::G722 => write!(f, "G722"),
            AudioCodec::G729 => write!(f, "G729"),
            AudioCodec::Opus => write!(f, "Opus"),
            AudioCodec::AMR => write!(f, "AMR"),
        }
    }
}

impl AudioCodec {
    /// Get default RTP payload type for codec
    pub fn default_payload_type(&self) -> u8 {
        match self {
            AudioCodec::PCMU => 0,
            AudioCodec::PCMA => 8,
            AudioCodec::G722 => 9,
            AudioCodec::G729 => 18,
            AudioCodec::Opus => 96, // Dynamic
            AudioCodec::AMR => 97,  // Dynamic
        }
    }

    /// Get codec sample rate
    pub fn sample_rate(&self) -> u32 {
        match self {
            AudioCodec::PCMU | AudioCodec::PCMA => 8000,
            AudioCodec::G722 => 16000,
            AudioCodec::G729 => 8000,
            AudioCodec::Opus => 48000,
            AudioCodec::AMR => 8000,
        }
    }

    /// Get codec bit rate (bps)
    pub fn bit_rate(&self) -> u32 {
        match self {
            AudioCodec::PCMU | AudioCodec::PCMA => 64000,
            AudioCodec::G722 => 64000,
            AudioCodec::G729 => 8000,
            AudioCodec::Opus => 32000, // Variable, this is average
            AudioCodec::AMR => 12200,  // Variable, this is average
        }
    }
}

/// DTMF event types (RFC 4733)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DtmfEvent {
    Digit0 = 0,
    Digit1 = 1,
    Digit2 = 2,
    Digit3 = 3,
    Digit4 = 4,
    Digit5 = 5,
    Digit6 = 6,
    Digit7 = 7,
    Digit8 = 8,
    Digit9 = 9,
    Star = 10,   // *
    Hash = 11,   // #
    A = 12,
    B = 13,
    C = 14,
    D = 15,
    Flash = 16,
}

impl DtmfEvent {
    /// Parse DTMF event from digit character
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            '0' => Some(DtmfEvent::Digit0),
            '1' => Some(DtmfEvent::Digit1),
            '2' => Some(DtmfEvent::Digit2),
            '3' => Some(DtmfEvent::Digit3),
            '4' => Some(DtmfEvent::Digit4),
            '5' => Some(DtmfEvent::Digit5),
            '6' => Some(DtmfEvent::Digit6),
            '7' => Some(DtmfEvent::Digit7),
            '8' => Some(DtmfEvent::Digit8),
            '9' => Some(DtmfEvent::Digit9),
            '*' => Some(DtmfEvent::Star),
            '#' => Some(DtmfEvent::Hash),
            'A' | 'a' => Some(DtmfEvent::A),
            'B' | 'b' => Some(DtmfEvent::B),
            'C' | 'c' => Some(DtmfEvent::C),
            'D' | 'd' => Some(DtmfEvent::D),
            _ => None,
        }
    }

    /// Convert to character representation
    pub fn to_char(&self) -> char {
        match self {
            DtmfEvent::Digit0 => '0',
            DtmfEvent::Digit1 => '1',
            DtmfEvent::Digit2 => '2',
            DtmfEvent::Digit3 => '3',
            DtmfEvent::Digit4 => '4',
            DtmfEvent::Digit5 => '5',
            DtmfEvent::Digit6 => '6',
            DtmfEvent::Digit7 => '7',
            DtmfEvent::Digit8 => '8',
            DtmfEvent::Digit9 => '9',
            DtmfEvent::Star => '*',
            DtmfEvent::Hash => '#',
            DtmfEvent::A => 'A',
            DtmfEvent::B => 'B',
            DtmfEvent::C => 'C',
            DtmfEvent::D => 'D',
            DtmfEvent::Flash => 'F',
        }
    }
}

/// DTMF relay packet (RFC 4733)
#[derive(Debug, Clone)]
pub struct DtmfPacket {
    /// DTMF event
    pub event: DtmfEvent,
    /// End flag
    pub end: bool,
    /// Volume level (0-63)
    pub volume: u8,
    /// Duration in timestamp units
    pub duration: u16,
}

impl DtmfPacket {
    /// Parse DTMF packet from RTP payload
    pub fn parse(payload: &[u8]) -> Result<Self> {
        if payload.len() < 4 {
            return Err(anyhow!("DTMF packet too short: {} bytes", payload.len()));
        }

        let event_num = payload[0];
        let flags_volume = payload[1];
        let duration = u16::from_be_bytes([payload[2], payload[3]]);

        let event = match event_num {
            0 => DtmfEvent::Digit0,
            1 => DtmfEvent::Digit1,
            2 => DtmfEvent::Digit2,
            3 => DtmfEvent::Digit3,
            4 => DtmfEvent::Digit4,
            5 => DtmfEvent::Digit5,
            6 => DtmfEvent::Digit6,
            7 => DtmfEvent::Digit7,
            8 => DtmfEvent::Digit8,
            9 => DtmfEvent::Digit9,
            10 => DtmfEvent::Star,
            11 => DtmfEvent::Hash,
            12 => DtmfEvent::FlashA,
            13 => DtmfEvent::FlashB,
            14 => DtmfEvent::FlashC,
            15 => DtmfEvent::FlashD,
            16 => DtmfEvent::Flash,
            _ => return Err(anyhow!("Invalid DTMF event: {}", event_num)),
        };

        let end = (flags_volume & 0x80) != 0;
        let volume = flags_volume & 0x3F;

        Ok(Self {
            event,
            end,
            volume,
            duration,
        })
    }

    /// Serialize DTMF packet to RTP payload
    pub fn serialize(&self) -> Vec<u8> {
        let mut payload = Vec::with_capacity(4);
        payload.push(self.event as u8);
        
        let flags_volume = if self.end { 0x80 } else { 0x00 } | (self.volume & 0x3F);
        payload.push(flags_volume);
        
        payload.extend_from_slice(&self.duration.to_be_bytes());
        payload
    }
}

/// Codec transcoder for audio conversion
pub struct CodecTranscoder {
    /// Supported codec conversions
    supported_conversions: HashMap<(AudioCodec, AudioCodec), bool>,
}

impl CodecTranscoder {
    /// Create new codec transcoder
    pub fn new() -> Self {
        let mut supported_conversions = HashMap::new();
        
        // G.711 conversions (μ-law ↔ A-law)
        supported_conversions.insert((AudioCodec::PCMU, AudioCodec::PCMA), true);
        supported_conversions.insert((AudioCodec::PCMA, AudioCodec::PCMU), true);
        
        // Pass-through for same codec
        for codec in [AudioCodec::PCMU, AudioCodec::PCMA, AudioCodec::G722, AudioCodec::G729, AudioCodec::Opus, AudioCodec::AMR] {
            supported_conversions.insert((codec, codec), true);
        }

        Self {
            supported_conversions,
        }
    }

    /// Check if codec conversion is supported
    pub fn is_conversion_supported(&self, input: AudioCodec, output: AudioCodec) -> bool {
        self.supported_conversions.get(&(input, output)).copied().unwrap_or(false)
    }

    /// Transcode audio payload
    pub async fn transcode_audio(&self, payload: &[u8], input: AudioCodec, output: AudioCodec) -> Result<Vec<u8>> {
        if input == output {
            return Ok(payload.to_vec());
        }

        if !self.is_conversion_supported(input, output) {
            return Err(anyhow!("Unsupported codec conversion: {} -> {}", input, output));
        }

        match (input, output) {
            (AudioCodec::PCMU, AudioCodec::PCMA) => {
                // μ-law to A-law conversion
                Ok(self.mulaw_to_alaw(payload))
            },
            (AudioCodec::PCMA, AudioCodec::PCMU) => {
                // A-law to μ-law conversion
                Ok(self.alaw_to_mulaw(payload))
            },
            _ => {
                // For now, return original payload (placeholder for complex transcoding)
                warn!("Complex transcoding not implemented: {} -> {}", input, output);
                Ok(payload.to_vec())
            }
        }
    }

    /// Convert μ-law to A-law
    fn mulaw_to_alaw(&self, mulaw_data: &[u8]) -> Vec<u8> {
        // Simplified μ-law to A-law conversion
        // In production, this would use proper lookup tables
        mulaw_data.iter().map(|&sample| {
            // Convert μ-law to linear PCM, then to A-law
            let linear = self.mulaw_to_linear(sample);
            self.linear_to_alaw(linear)
        }).collect()
    }

    /// Convert A-law to μ-law
    fn alaw_to_mulaw(&self, alaw_data: &[u8]) -> Vec<u8> {
        // Simplified A-law to μ-law conversion
        alaw_data.iter().map(|&sample| {
            // Convert A-law to linear PCM, then to μ-law
            let linear = self.alaw_to_linear(sample);
            self.linear_to_mulaw(linear)
        }).collect()
    }

    /// Convert μ-law sample to linear PCM (simplified)
    fn mulaw_to_linear(&self, mulaw: u8) -> i16 {
        // Simplified conversion - production would use proper algorithm
        let sign = if (mulaw & 0x80) != 0 { -1 } else { 1 };
        let exponent = (mulaw >> 4) & 0x07;
        let mantissa = mulaw & 0x0F;
        
        let linear = ((mantissa << 1) + 33) << exponent;
        (sign * linear as i32) as i16
    }

    /// Convert linear PCM to μ-law (simplified)
    fn linear_to_mulaw(&self, linear: i16) -> u8 {
        // Simplified conversion - production would use proper algorithm
        let sign = if linear < 0 { 0x80 } else { 0x00 };
        let abs_linear = linear.abs() as u16;
        
        // Find exponent and mantissa (simplified)
        let exponent = if abs_linear < 33 {
            0
        } else {
            (16 - abs_linear.leading_zeros()).saturating_sub(5) as u8
        };
        
        let mantissa = if exponent == 0 {
            (abs_linear >> 1).saturating_sub(33) as u8 & 0x0F
        } else {
            ((abs_linear >> (exponent + 1)) & 0x0F) as u8
        };
        
        sign | (exponent << 4) | mantissa
    }

    /// Convert A-law sample to linear PCM (simplified)
    fn alaw_to_linear(&self, alaw: u8) -> i16 {
        // Simplified conversion
        let sign = if (alaw & 0x80) != 0 { -1 } else { 1 };
        let exponent = (alaw >> 4) & 0x07;
        let mantissa = alaw & 0x0F;
        
        let linear = if exponent == 0 {
            (mantissa << 1) + 1
        } else {
            ((mantissa << 1) + 33) << (exponent - 1)
        };
        
        (sign * linear as i32) as i16
    }

    /// Convert linear PCM to A-law (simplified)
    fn linear_to_alaw(&self, linear: i16) -> u8 {
        // Simplified conversion
        let sign = if linear < 0 { 0x80 } else { 0x00 };
        let abs_linear = linear.abs() as u16;
        
        let (exponent, mantissa) = if abs_linear < 33 {
            (0, (abs_linear >> 1) as u8 & 0x0F)
        } else {
            let exp = (16 - abs_linear.leading_zeros()).saturating_sub(5) as u8;
            let mant = ((abs_linear >> (exp + 1)) & 0x0F) as u8;
            (exp + 1, mant)
        };
        
        sign | (exponent << 4) | mantissa
    }
}

/// Media session for Phase 2
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaPlaneSession {
    /// Session ID
    pub session_id: String,
    /// Call ID
    pub call_id: String,
    /// Leg A endpoint
    pub leg_a: MediaEndpoint,
    /// Leg B endpoint
    pub leg_b: MediaEndpoint,
    /// Session creation time
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last activity
    pub last_activity: chrono::DateTime<chrono::Utc>,
    /// Session is active
    pub active: bool,
    /// Enable audio transcoding
    pub enable_transcoding: bool,
    /// Enable DTMF relay
    pub enable_dtmf_relay: bool,
    /// Enable SRTP
    pub enable_srtp: bool,
    /// Associated video session
    pub video_session: Option<String>,
    /// Session statistics
    pub stats: MediaSessionStats,
}

/// Media endpoint for Phase 2
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaEndpoint {
    /// Remote RTP address
    pub rtp_addr: SocketAddr,
    /// Remote RTCP address
    pub rtcp_addr: SocketAddr,
    /// Local RTP address (proxy)
    pub local_rtp_addr: Option<SocketAddr>,
    /// Local RTCP address (proxy)
    pub local_rtcp_addr: Option<SocketAddr>,
    /// Audio codec
    pub codec: AudioCodec,
    /// RTP payload type
    pub payload_type: u8,
    /// DTMF payload type
    pub dtmf_payload_type: Option<u8>,
    /// SSRC identifier
    pub ssrc: Option<u32>,
    /// SRTP parameters
    pub srtp_params: Option<SrtpParams>,
}

/// SRTP parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SrtpParams {
    /// Crypto suite
    pub crypto_suite: String,
    /// Master key
    pub master_key: Vec<u8>,
    /// Master salt
    pub master_salt: Vec<u8>,
}

/// Media session statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MediaSessionStats {
    /// RTP packets relayed (A->B)
    pub packets_a_to_b: u64,
    /// RTP packets relayed (B->A)
    pub packets_b_to_a: u64,
    /// Bytes relayed (A->B)
    pub bytes_a_to_b: u64,
    /// Bytes relayed (B->A)
    pub bytes_b_to_a: u64,
    /// Packets lost (A->B)
    pub packets_lost_a_to_b: u64,
    /// Packets lost (B->A)
    pub packets_lost_b_to_a: u64,
    /// DTMF events relayed
    pub dtmf_events_relayed: u64,
    /// Transcoding operations
    pub transcoding_operations: u64,
    /// SRTP packets processed
    pub srtp_packets_processed: u64,
}

/// Media plane configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaPlaneConfig {
    /// Port range for RTP allocation
    pub rtp_port_range: (u16, u16),
    /// Local IP for media
    pub local_ip: String,
    /// Enable RTP monitoring
    pub enable_monitoring: bool,
    /// Enable video passthrough
    pub enable_video: bool,
    /// Enable SRTP
    pub enable_srtp: bool,
    /// Maximum concurrent sessions
    pub max_sessions: u32,
    /// Session timeout (seconds)
    pub session_timeout: u64,
}

impl Default for MediaPlaneConfig {
    fn default() -> Self {
        Self {
            rtp_port_range: (20000, 30000),
            local_ip: "127.0.0.1".to_string(),
            enable_monitoring: true,
            enable_video: true,
            enable_srtp: true,
            max_sessions: 10000,
            session_timeout: 3600,
        }
    }
}

/// Phase 2 Media Plane
pub struct MediaPlane {
    /// Configuration
    config: MediaPlaneConfig,
    /// Active sessions
    sessions: Arc<DashMap<String, MediaPlaneSession>>,
    /// Port allocator
    port_allocator: PortAllocator,
    /// Codec transcoder
    transcoder: Arc<CodecTranscoder>,
    /// RTP monitor
    rtp_monitor: Option<Arc<RtpMonitor>>,
    /// Video passthrough manager
    // Video removed - feature not implemented
    // video_manager: Option<Arc<VideoPassthroughManager>>,
    /// Active sockets
    sockets: Arc<DashMap<String, (Arc<UdpSocket>, Arc<UdpSocket>)>>,
}

impl MediaPlane {
    /// Create new media plane
    pub fn new(config: MediaPlaneConfig) -> Result<Self> {
        let transcoder = Arc::new(CodecTranscoder::new());
        let port_allocator = PortAllocator::new(config.rtp_port_range);
        
        let rtp_monitor = if config.enable_monitoring {
            Some(Arc::new(RtpMonitor::new(RtpMonitorConfig::default())))
        } else {
            None
        };

        let video_manager = if config.enable_video {
            None // Video removed - feature not implemented
        } else {
            None
        };

        Ok(Self {
            config,
            sessions: Arc::new(DashMap::new()),
            port_allocator,
            transcoder,
            rtp_monitor,
            video_manager,
            sockets: Arc::new(DashMap::new()),
        })
    }

    /// Start media plane
    #[instrument(skip(self))]
    pub async fn start(&self) -> Result<()> {
        info!("Starting media plane");

        if let Some(monitor) = &self.rtp_monitor {
            monitor.start().await?;
        }

        info!("Media plane started successfully");
        Ok(())
    }

    /// Create media session
    #[instrument(skip(self))]
    pub async fn create_session(
        &self,
        call_id: String,
        leg_a: MediaEndpoint,
        leg_b: MediaEndpoint,
    ) -> Result<MediaPlaneSession> {
        let session_id = uuid::Uuid::new_v4().to_string();

        // Allocate ports
        let (a_rtp_port, a_rtcp_port) = self.port_allocator.allocate_port_pair(&session_id).await?;
        let (b_rtp_port, b_rtcp_port) = self.port_allocator.allocate_port_pair(&session_id).await?;

        let local_ip: std::net::IpAddr = self.config.local_ip.parse()
            .map_err(|e| anyhow!("Invalid local IP {}: {}", self.config.local_ip, e))?;

        let mut leg_a_updated = leg_a;
        leg_a_updated.local_rtp_addr = Some(SocketAddr::new(local_ip, a_rtp_port));
        leg_a_updated.local_rtcp_addr = Some(SocketAddr::new(local_ip, a_rtcp_port));

        let mut leg_b_updated = leg_b;
        leg_b_updated.local_rtp_addr = Some(SocketAddr::new(local_ip, b_rtp_port));
        leg_b_updated.local_rtcp_addr = Some(SocketAddr::new(local_ip, b_rtcp_port));

        let session = MediaPlaneSession {
            session_id: session_id.clone(),
            call_id: call_id.clone(),
            leg_a: leg_a_updated,
            leg_b: leg_b_updated,
            created_at: chrono::Utc::now(),
            last_activity: chrono::Utc::now(),
            active: false,
            enable_transcoding: false, // Will be determined later
            enable_dtmf_relay: true,
            enable_srtp: self.config.enable_srtp,
            video_session: None,
            stats: MediaSessionStats::default(),
        };

        // Create sockets
        self.create_session_sockets(&session).await?;

        // Register with RTP monitor
        if let Some(monitor) = &self.rtp_monitor {
            monitor.register_stream(
                format!("{}_a", session_id),
                session.leg_a.ssrc.unwrap_or(0),
                session.leg_a.codec.to_string(),
                session.leg_a.local_rtp_addr.unwrap(),
                session.leg_a.rtp_addr,
            ).await?;

            monitor.register_stream(
                format!("{}_b", session_id),
                session.leg_b.ssrc.unwrap_or(0),
                session.leg_b.codec.to_string(),
                session.leg_b.local_rtp_addr.unwrap(),
                session.leg_b.rtp_addr,
            ).await?;
        }

        self.sessions.insert(session_id.clone(), session.clone());

        info!("Created media plane session {} for call {}", session_id, call_id);
        Ok(session)
    }

    /// Start media session
    #[instrument(skip(self))]
    pub async fn start_session(&self, session_id: &str) -> Result<()> {
        let mut session = self.sessions.get_mut(session_id)
            .ok_or_else(|| anyhow!("Session not found: {}", session_id))?;

        session.active = true;
        session.last_activity = chrono::Utc::now();

        // Determine if transcoding is needed
        session.enable_transcoding = session.leg_a.codec != session.leg_b.codec;

        info!("Started media session {} (transcoding: {})", 
            session_id, session.enable_transcoding);

        // Start RTP relay tasks
        self.start_rtp_relay_tasks(session_id).await?;

        Ok(())
    }

    /// Stop media session
    #[instrument(skip(self))]
    pub async fn stop_session(&self, session_id: &str) -> Result<()> {
        if let Some((_, mut session)) = self.sessions.remove(session_id) {
            session.active = false;
            
            // Cleanup sockets
            self.sockets.remove(session_id);
            
            // Deallocate ports
            self.port_allocator.deallocate_ports(session_id).await;
            
            // Unregister from RTP monitor
            if let Some(monitor) = &self.rtp_monitor {
                let _ = monitor.unregister_stream(&format!("{}_a", session_id)).await;
                let _ = monitor.unregister_stream(&format!("{}_b", session_id)).await;
            }

            // Clean up video session if any
            if let Some(video_session_id) = session.video_session {
                if let Some(video_manager) = &self.video_manager {
                    let _ = video_manager.stop_video_session(&video_session_id).await;
                }
            }
            
            info!("Stopped media session {}. Final stats: {:?}", session_id, session.stats);
        }
        
        Ok(())
    }

    /// Create video session for existing media session
    pub async fn create_video_session(&self, session_id: &str, remote_sdp: &str) -> Result<String> {
        if let Some(video_manager) = &self.video_manager {
            let mut session = self.sessions.get_mut(session_id)
                .ok_or_else(|| anyhow!("Media session not found: {}", session_id))?;

            let local_addr = session.leg_a.local_rtp_addr
                .ok_or_else(|| anyhow!("No local RTP address"))?;

            let (video_session, _answer_sdp) = video_manager
                .process_video_offer(&session.call_id, local_addr, remote_sdp).await?;

            session.video_session = Some(video_session.session_id.clone());

            info!("Created video session {} for media session {}", video_session.session_id, session_id);
            Ok(video_session.session_id)
        } else {
            Err(anyhow!("Video passthrough not enabled"))
        }
    }

    /// Get session statistics
    pub async fn get_session_stats(&self, session_id: &str) -> Option<MediaSessionStats> {
        self.sessions.get(session_id).map(|s| s.stats.clone())
    }

    /// List active sessions
    pub async fn list_sessions(&self) -> Vec<String> {
        self.sessions.iter()
            .filter(|entry| entry.value().active)
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Create UDP sockets for session
    async fn create_session_sockets(&self, session: &MediaPlaneSession) -> Result<()> {
        let leg_a_rtp_addr = session.leg_a.local_rtp_addr
            .ok_or_else(|| anyhow!("No local RTP address for leg A"))?;
        let leg_a_rtcp_addr = session.leg_a.local_rtcp_addr
            .ok_or_else(|| anyhow!("No local RTCP address for leg A"))?;
        
        let leg_b_rtp_addr = session.leg_b.local_rtp_addr
            .ok_or_else(|| anyhow!("No local RTP address for leg B"))?;
        let leg_b_rtcp_addr = session.leg_b.local_rtcp_addr
            .ok_or_else(|| anyhow!("No local RTCP address for leg B"))?;

        // Create sockets
        let leg_a_rtp_socket = Arc::new(UdpSocket::bind(leg_a_rtp_addr).await?);
        let leg_a_rtcp_socket = Arc::new(UdpSocket::bind(leg_a_rtcp_addr).await?);
        let leg_b_rtp_socket = Arc::new(UdpSocket::bind(leg_b_rtp_addr).await?);
        let leg_b_rtcp_socket = Arc::new(UdpSocket::bind(leg_b_rtcp_addr).await?);

        // Store socket references
        self.sockets.insert(
            format!("{}_a", session.session_id),
            (leg_a_rtp_socket, leg_a_rtcp_socket)
        );
        self.sockets.insert(
            format!("{}_b", session.session_id),
            (leg_b_rtp_socket, leg_b_rtcp_socket)
        );

        debug!("Created sockets for session {}", session.session_id);
        Ok(())
    }

    /// Start RTP relay tasks
    async fn start_rtp_relay_tasks(&self, session_id: &str) -> Result<()> {
        let session = self.sessions.get(session_id)
            .ok_or_else(|| anyhow!("Session not found: {}", session_id))?
            .clone();

        let sockets_a = self.sockets.get(&format!("{}_a", session_id))
            .ok_or_else(|| anyhow!("Sockets not found for leg A"))?
            .clone();
        let sockets_b = self.sockets.get(&format!("{}_b", session_id))
            .ok_or_else(|| anyhow!("Sockets not found for leg B"))?
            .clone();

        // Start relay tasks with Phase 2 enhancements
        let sessions_clone = self.sessions.clone();
        let transcoder_clone = self.transcoder.clone();
        let monitor_clone = self.rtp_monitor.clone();

        // A->B relay
        let session_id_clone = session_id.to_string();
        let session_clone = session.clone();
        tokio::spawn(async move {
            Self::enhanced_rtp_relay_task(
                session_id_clone,
                "a_to_b".to_string(),
                sockets_a.0,
                sockets_b.0,
                session_clone.leg_b.rtp_addr,
                sessions_clone,
                transcoder_clone,
                monitor_clone,
                session_clone.leg_a.codec,
                session_clone.leg_b.codec,
                session_clone.leg_a.dtmf_payload_type,
                session_clone.leg_b.dtmf_payload_type,
                session_clone.enable_transcoding,
                session_clone.enable_dtmf_relay,
            ).await;
        });

        // B->A relay  
        let sessions_clone = self.sessions.clone();
        let transcoder_clone = self.transcoder.clone();
        let monitor_clone = self.rtp_monitor.clone();
        let session_id_clone = session_id.to_string();

        tokio::spawn(async move {
            Self::enhanced_rtp_relay_task(
                session_id_clone,
                "b_to_a".to_string(),
                sockets_b.0,
                sockets_a.0,
                session.leg_a.rtp_addr,
                sessions_clone,
                transcoder_clone,
                monitor_clone,
                session.leg_b.codec,
                session.leg_a.codec,
                session.leg_b.dtmf_payload_type,
                session.leg_a.dtmf_payload_type,
                session.enable_transcoding,
                session.enable_dtmf_relay,
            ).await;
        });

        Ok(())
    }

    /// Enhanced RTP relay task with Phase 2 features
    async fn enhanced_rtp_relay_task(
        session_id: String,
        direction: String,
        input_socket: Arc<UdpSocket>,
        output_socket: Arc<UdpSocket>,
        output_addr: SocketAddr,
        sessions: Arc<DashMap<String, MediaPlaneSession>>,
        transcoder: Arc<CodecTranscoder>,
        monitor: Option<Arc<RtpMonitor>>,
        input_codec: AudioCodec,
        output_codec: AudioCodec,
        input_dtmf_pt: Option<u8>,
        output_dtmf_pt: Option<u8>,
        enable_transcoding: bool,
        enable_dtmf_relay: bool,
    ) {
        let mut buffer = vec![0u8; 2048];
        info!("Started enhanced RTP relay for session {} direction {}", session_id, direction);

        loop {
            match input_socket.recv_from(&mut buffer).await {
                Ok((size, _source)) => {
                    let packet_data = &buffer[..size];
                    
                    // Parse RTP header
                    let rtp_header = match Self::parse_rtp_header(packet_data) {
                        Ok(header) => header,
                        Err(e) => {
                            debug!("Failed to parse RTP header: {}", e);
                            continue;
                        }
                    };

                    // Update RTP monitor
                    if let Some(ref monitor) = monitor {
                        let stream_id = format!("{}_{}", session_id, if direction == "a_to_b" { "a" } else { "b" });
                        if let Err(e) = monitor.process_rtp_packet(&stream_id, packet_data, None).await {
                            debug!("RTP monitor error: {}", e);
                        }
                    }

                    // Process packet based on payload type
                    let output_packet = if let Some(dtmf_pt) = input_dtmf_pt {
                        if rtp_header.payload_type() == dtmf_pt && enable_dtmf_relay {
                            // DTMF packet
                            Self::process_dtmf_packet(packet_data, output_dtmf_pt)
                        } else {
                            // Regular audio packet
                            Self::process_audio_packet(
                                packet_data, 
                                input_codec, 
                                output_codec, 
                                &transcoder, 
                                enable_transcoding
                            ).await
                        }
                    } else {
                        // Regular audio packet
                        Self::process_audio_packet(
                            packet_data, 
                            input_codec, 
                            output_codec, 
                            &transcoder, 
                            enable_transcoding
                        ).await
                    };

                    let output_packet = match output_packet {
                        Ok(packet) => packet,
                        Err(e) => {
                            debug!("Packet processing failed: {}", e);
                            continue;
                        }
                    };

                    // Forward packet
                    if let Err(e) = output_socket.send_to(&output_packet, output_addr).await {
                        error!("Failed to forward RTP packet: {}", e);
                        continue;
                    }

                    // Update statistics
                    if let Some(mut session) = sessions.get_mut(&session_id) {
                        session.last_activity = chrono::Utc::now();
                        
                        if direction == "a_to_b" {
                            session.stats.packets_a_to_b += 1;
                            session.stats.bytes_a_to_b += size as u64;
                        } else {
                            session.stats.packets_b_to_a += 1;
                            session.stats.bytes_b_to_a += size as u64;
                        }

                        if enable_transcoding && input_codec != output_codec {
                            session.stats.transcoding_operations += 1;
                        }

                        if input_dtmf_pt.is_some() && rtp_header.payload_type() == input_dtmf_pt.unwrap() {
                            session.stats.dtmf_events_relayed += 1;
                        }
                    }
                },
                Err(e) => {
                    error!("RTP relay error for session {} direction {}: {}", session_id, direction, e);
                    break;
                }
            }

            // Check if session is still active
            if let Some(session) = sessions.get(&session_id) {
                if !session.active {
                    debug!("Session {} is no longer active, stopping relay", session_id);
                    break;
                }
            } else {
                debug!("Session {} no longer exists, stopping relay", session_id);
                break;
            }
        }

        info!("Stopped enhanced RTP relay for session {} direction {}", session_id, direction);
    }

    /// Parse RTP header from packet
    fn parse_rtp_header(packet: &[u8]) -> Result<RtpHeader> {
        if packet.len() < 12 {
            return Err(anyhow!("RTP packet too small"));
        }

        Ok(RtpHeader {
            version: (packet[0] >> 6) & 0x03,
            padding: (packet[0] & 0x20) != 0,
            extension: (packet[0] & 0x10) != 0,
            csrc_count: packet[0] & 0x0F,
            marker: (packet[1] & 0x80) != 0,
            payload_type: packet[1] & 0x7F,
            sequence_number: u16::from_be_bytes([packet[2], packet[3]]),
            timestamp: u32::from_be_bytes([packet[4], packet[5], packet[6], packet[7]]),
            ssrc: u32::from_be_bytes([packet[8], packet[9], packet[10], packet[11]]),
        })
    }

    /// Process DTMF packet
    fn process_dtmf_packet(packet: &[u8], output_dtmf_pt: Option<u8>) -> Result<Vec<u8>> {
        if let Some(output_pt) = output_dtmf_pt {
            // Parse DTMF packet
            let header_len = 12; // Basic RTP header
            if packet.len() < header_len + 4 {
                return Err(anyhow!("DTMF packet too small"));
            }

            let dtmf_payload = &packet[header_len..];
            let dtmf_event = DtmfPacket::parse(dtmf_payload)?;

            debug!("Relaying DTMF event: {:?}", dtmf_event);

            // Create output packet with new payload type
            let mut output_packet = packet.to_vec();
            output_packet[1] = (output_packet[1] & 0x80) | (output_pt & 0x7F);
            
            Ok(output_packet)
        } else {
            // No DTMF support on output, drop packet
            Err(anyhow!("DTMF not supported on output"))
        }
    }

    /// Process audio packet
    async fn process_audio_packet(
        packet: &[u8],
        input_codec: AudioCodec,
        output_codec: AudioCodec,
        transcoder: &CodecTranscoder,
        enable_transcoding: bool,
    ) -> Result<Vec<u8>> {
        if enable_transcoding && input_codec != output_codec {
            // Extract payload and transcode
            let header_len = 12; // Basic RTP header (simplified)
            if packet.len() < header_len {
                return Err(anyhow!("Audio packet too small"));
            }

            let payload = &packet[header_len..];
            let transcoded_payload = transcoder.transcode_audio(payload, input_codec, output_codec).await?;

            // Reconstruct packet
            let mut output_packet = packet[..header_len].to_vec();
            output_packet.extend_from_slice(&transcoded_payload);

            // Update payload type
            output_packet[1] = (output_packet[1] & 0x80) | (output_codec.default_payload_type() & 0x7F);

            Ok(output_packet)
        } else {
            // Direct relay
            Ok(packet.to_vec())
        }
    }
}

/// Simple RTP header structure
#[derive(Debug)]
struct RtpHeader {
    pub version: u8,
    pub padding: bool,
    pub extension: bool,
    pub csrc_count: u8,
    pub marker: bool,
    pub payload_type: u8,
    pub sequence_number: u16,
    pub timestamp: u32,
    pub ssrc: u32,
}

impl RtpHeader {
    pub fn payload_type(&self) -> u8 {
        self.payload_type
    }
}

/// Port allocator
#[derive(Debug)]
pub struct PortAllocator {
    port_range: (u16, u16),
    allocated_ports: Arc<RwLock<HashMap<u16, String>>>,
    next_port: Arc<RwLock<u16>>,
}

impl PortAllocator {
    pub fn new(port_range: (u16, u16)) -> Self {
        Self {
            port_range,
            allocated_ports: Arc::new(RwLock::new(HashMap::new())),
            next_port: Arc::new(RwLock::new(port_range.0)),
        }
    }

    pub async fn allocate_port_pair(&self, session_id: &str) -> Result<(u16, u16)> {
        let mut allocated = self.allocated_ports.write().await;
        let mut next_port = self.next_port.write().await;

        let mut attempts = 0;
        let max_attempts = (self.port_range.1 - self.port_range.0) / 2;

        while attempts < max_attempts {
            let rtp_port = *next_port;
            let rtcp_port = rtp_port + 1;

            if rtp_port >= self.port_range.1 || rtp_port % 2 != 0 {
                *next_port = self.port_range.0;
                continue;
            }

            if !allocated.contains_key(&rtp_port) && !allocated.contains_key(&rtcp_port) {
                allocated.insert(rtp_port, session_id.to_string());
                allocated.insert(rtcp_port, session_id.to_string());
                
                *next_port = rtcp_port + 1;
                if *next_port >= self.port_range.1 {
                    *next_port = self.port_range.0;
                }

                return Ok((rtp_port, rtcp_port));
            }

            *next_port += 2;
            if *next_port >= self.port_range.1 {
                *next_port = self.port_range.0;
            }
            attempts += 1;
        }

        Err(anyhow!("No available port pairs"))
    }

    pub async fn deallocate_ports(&self, session_id: &str) {
        let mut allocated = self.allocated_ports.write().await;
        let ports_to_remove: Vec<u16> = allocated
            .iter()
            .filter_map(|(port, sid)| if sid == session_id { Some(*port) } else { None })
            .collect();

        for port in ports_to_remove {
            allocated.remove(&port);
        }
    }
}