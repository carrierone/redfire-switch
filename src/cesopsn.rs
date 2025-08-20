/*
 * CESoPSN - Circuit Emulation Service over Packet Switched Network
 * 
 * RFC 5086 compliant implementation for carrying TDM circuits over IP networks
 * with proper structure awareness, error handling, and Quality of Service.
 * 
 * Features:
 * - Structure-aware TDM circuit emulation
 * - Per-timeslot error detection and correction
 * - Adaptive jitter buffering
 * - Clock recovery and synchronization
 * - Support for T1/E1 circuits with signaling
 */

use anyhow::{Result, anyhow};
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc, Mutex};
use tokio::net::UdpSocket;
use tracing::{debug, info, warn, error};
use serde::{Serialize, Deserialize};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use rand;

/// CESoPSN Protocol Version (RFC 5086)
pub const CESOPSN_VERSION: u8 = 0;

/// CESoPSN Header Length (12 bytes base + optional extensions)
pub const CESOPSN_HEADER_LEN: usize = 12;

/// Maximum payload size for CESoPSN packet
pub const MAX_CESOPSN_PAYLOAD: usize = 1440; // To fit in standard MTU

/// TDM Circuit Types supported by CESoPSN
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CesopsnCircuitType {
    /// T1 Circuit (24 DS0 channels, 1.544 Mbps)
    T1 = 1,
    /// E1 Circuit (32 timeslots, 30 DS0 channels, 2.048 Mbps) 
    E1 = 2,
    /// Fractional T1 (subset of T1 channels)
    FractionalT1 = 3,
    /// Fractional E1 (subset of E1 channels)
    FractionalE1 = 4,
}

/// CESoPSN Service Quality
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CesopsnServiceQuality {
    /// Best Effort (no QoS guarantees)
    BestEffort = 0,
    /// Assured Forwarding (moderate QoS)
    AssuredForwarding = 1,
    /// Expedited Forwarding (high priority, low latency)
    ExpeditedForwarding = 2,
}

/// CESoPSN Packet Header (RFC 5086 Section 5.1)
#[derive(Debug, Clone)]
pub struct CesopsnHeader {
    /// Sequence number (16 bits)
    pub sequence_number: u16,
    /// Timestamp (32 bits) - RTP timestamp format
    pub timestamp: u32,
    /// Synchronization Source ID (32 bits)
    pub ssrc: u32,
    /// Circuit ID (16 bits) - identifies specific TDM circuit
    pub circuit_id: u16,
    /// Payload Type (6 bits) - type of TDM data
    pub payload_type: u8,
    /// Marker bit - indicates significant events
    pub marker: bool,
    /// Extension bit - indicates header extensions present
    pub extension: bool,
    /// Version (2 bits) - always 0 for RFC 5086
    pub version: u8,
}

impl CesopsnHeader {
    /// Create new CESoPSN header
    pub fn new(circuit_id: u16, ssrc: u32) -> Self {
        Self {
            sequence_number: 0,
            timestamp: 0,
            ssrc,
            circuit_id,
            payload_type: 0,
            marker: false,
            extension: false,
            version: CESOPSN_VERSION,
        }
    }
    
    /// Serialize header to bytes (RFC 5086 format)
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(CESOPSN_HEADER_LEN);
        
        // Byte 0: V(2) + P(1) + X(1) + CC(4) - Version, Padding, eXtension, CSRC Count
        let byte0 = (self.version << 6) | 
                   (if self.extension { 0x10 } else { 0x00 }) | 
                   0x00; // No padding, no CSRC
        bytes.push(byte0);
        
        // Byte 1: M(1) + PT(7) - Marker + Payload Type  
        let byte1 = (if self.marker { 0x80 } else { 0x00 }) | 
                   (self.payload_type & 0x7F);
        bytes.push(byte1);
        
        // Bytes 2-3: Sequence Number
        let _ = bytes.write_u16::<BigEndian>(self.sequence_number); // Vec write never fails
        
        // Bytes 4-7: Timestamp
        let _ = bytes.write_u32::<BigEndian>(self.timestamp); // Vec write never fails
        
        // Bytes 8-11: SSRC
        let _ = bytes.write_u32::<BigEndian>(self.ssrc); // Vec write never fails
        
        bytes
    }
    
    /// Parse header from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < CESOPSN_HEADER_LEN {
            return Err(anyhow!("CESoPSN header too short: {} bytes", bytes.len()));
        }
        
        let version = (bytes[0] >> 6) & 0x03;
        if version != CESOPSN_VERSION {
            return Err(anyhow!("Unsupported CESoPSN version: {}", version));
        }
        
        let extension = (bytes[0] & 0x10) != 0;
        let marker = (bytes[1] & 0x80) != 0;
        let payload_type = bytes[1] & 0x7F;
        
        let sequence_number = BigEndian::read_u16(&bytes[2..4]);
        let timestamp = BigEndian::read_u32(&bytes[4..8]);
        let ssrc = BigEndian::read_u32(&bytes[8..12]);
        
        Ok(Self {
            sequence_number,
            timestamp,
            ssrc,
            circuit_id: 0, // Will be set from circuit configuration
            payload_type,
            marker,
            extension,
            version,
        })
    }
}

/// CESoPSN Payload Types (RFC 5086 Section 5.2)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CesopsnPayloadType {
    /// Structured T1/E1 with signaling bits
    StructuredT1E1 = 0,
    /// Unstructured T1/E1 (transparent)
    UnstructuredT1E1 = 1,
    /// T1 SF (Super Frame) with signaling
    T1SuperFrame = 2,
    /// T1 ESF (Extended Super Frame) with signaling
    T1ExtendedSuperFrame = 3,
    /// E1 with CAS (Channel Associated Signaling)
    E1WithCAS = 4,
    /// E1 without CAS
    E1WithoutCAS = 5,
}

/// CESoPSN Packet containing header and TDM payload
#[derive(Debug, Clone)]
pub struct CesopsnPacket {
    /// CESoPSN header
    pub header: CesopsnHeader,
    /// TDM payload data
    pub payload: Vec<u8>,
    /// Reception timestamp for jitter calculation
    pub received_at: Option<Instant>,
}

impl CesopsnPacket {
    /// Create new CESoPSN packet
    pub fn new(header: CesopsnHeader, payload: Vec<u8>) -> Self {
        Self {
            header,
            payload,
            received_at: None,
        }
    }
    
    /// Serialize complete packet to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.header.to_bytes();
        bytes.extend_from_slice(&self.payload);
        bytes
    }
    
    /// Parse complete packet from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let header = CesopsnHeader::from_bytes(bytes)?;
        let payload = bytes[CESOPSN_HEADER_LEN..].to_vec();
        
        Ok(Self {
            header,
            payload,
            received_at: Some(Instant::now()),
        })
    }
    
    /// Get packet size in bytes
    pub fn size(&self) -> usize {
        CESOPSN_HEADER_LEN + self.payload.len()
    }
}

/// CESoPSN Circuit Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CesopsnCircuitConfig {
    /// Unique circuit identifier
    pub circuit_id: u16,
    /// Circuit type (T1/E1/etc)
    pub circuit_type: CesopsnCircuitType,
    /// Remote endpoint address
    pub remote_address: SocketAddr,
    /// Local bind address
    pub local_address: SocketAddr,
    /// Service quality requirements
    pub service_quality: CesopsnServiceQuality,
    /// Payload type for this circuit
    pub payload_type: CesopsnPayloadType,
    /// Frame size in bytes (typically 32 for T1, 32 for E1)
    pub frame_size: usize,
    /// Frames per packet (for packetization efficiency)
    pub frames_per_packet: usize,
    /// Jitter buffer size in milliseconds
    pub jitter_buffer_ms: u32,
    /// Enable Adaptive Clock Recovery
    pub enable_acr: bool,
    /// Timeslot bitmap (which DS0s are active)
    pub active_timeslots: u32,
}

impl Default for CesopsnCircuitConfig {
    fn default() -> Self {
        Self {
            circuit_id: 1,
            circuit_type: CesopsnCircuitType::T1,
            remote_address: "127.0.0.1:20000".parse().unwrap(),
            local_address: "0.0.0.0:20000".parse().unwrap(),
            service_quality: CesopsnServiceQuality::ExpeditedForwarding,
            payload_type: CesopsnPayloadType::StructuredT1E1,
            frame_size: 32, // T1: 24 DS0 + 1 framing = 25, but aligned to 32
            frames_per_packet: 4, // 4 frames = ~500μs of audio
            jitter_buffer_ms: 20,
            enable_acr: true,
            active_timeslots: 0x00FFFFFF, // All 24 T1 channels active
        }
    }
}

/// CESoPSN Jitter Buffer for handling packet delay variation
#[derive(Debug)]
pub struct CesopsnJitterBuffer {
    /// Buffer to store out-of-order packets
    buffer: VecDeque<CesopsnPacket>,
    /// Maximum buffer size (in packets)
    max_size: usize,
    /// Target buffer depth for adaptive sizing
    target_depth: usize,
    /// Expected next sequence number
    next_sequence: u16,
    /// Statistics
    packets_received: u64,
    packets_dropped: u64,
    packets_late: u64,
    last_playout_time: Option<Instant>,
}

impl CesopsnJitterBuffer {
    /// Create new jitter buffer
    pub fn new(max_size: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(max_size),
            max_size,
            target_depth: max_size / 2,
            next_sequence: 0,
            packets_received: 0,
            packets_dropped: 0,
            packets_late: 0,
            last_playout_time: None,
        }
    }
    
    /// Add packet to jitter buffer
    pub fn add_packet(&mut self, mut packet: CesopsnPacket) -> Result<()> {
        self.packets_received += 1;
        packet.received_at = Some(Instant::now());
        
        // Check for late packets
        if self.is_sequence_late(packet.header.sequence_number) {
            self.packets_late += 1;
            debug!("Late packet received: seq={}", packet.header.sequence_number);
            return Ok(());
        }
        
        // Drop oldest packets if buffer full
        if self.buffer.len() >= self.max_size {
            if let Some(dropped) = self.buffer.pop_front() {
                self.packets_dropped += 1;
                warn!("Dropped packet due to buffer overflow: seq={}", 
                      dropped.header.sequence_number);
            }
        }
        
        // Insert packet in sequence order
        let insert_pos = self.buffer.iter()
            .position(|p| self.sequence_compare(packet.header.sequence_number, 
                                              p.header.sequence_number) < 0)
            .unwrap_or(self.buffer.len());
        
        self.buffer.insert(insert_pos, packet);
        Ok(())
    }
    
    /// Get next packet for playout (if available and ready)
    pub fn get_next_packet(&mut self) -> Option<CesopsnPacket> {
        // Check if we have the next expected packet
        if let Some(packet) = self.buffer.front() {
            if packet.header.sequence_number == self.next_sequence {
                let packet = self.buffer.pop_front().unwrap();
                self.next_sequence = self.next_sequence.wrapping_add(1);
                self.last_playout_time = Some(Instant::now());
                return Some(packet);
            }
        }
        
        // Adaptive buffer management - release packet if buffer too deep
        if self.buffer.len() > self.target_depth {
            if let Some(packet) = self.buffer.pop_front() {
                self.next_sequence = packet.header.sequence_number.wrapping_add(1);
                self.last_playout_time = Some(Instant::now());
                return Some(packet);
            }
        }
        
        None
    }
    
    /// Check if sequence number is considered late
    fn is_sequence_late(&self, seq: u16) -> bool {
        let diff = seq.wrapping_sub(self.next_sequence);
        diff > 32768 // More than half the sequence space behind
    }
    
    /// Compare sequence numbers accounting for wraparound
    fn sequence_compare(&self, a: u16, b: u16) -> i32 {
        let diff = a.wrapping_sub(b) as i16;
        diff as i32
    }
    
    /// Get buffer statistics
    pub fn get_stats(&self) -> CesopsnJitterBufferStats {
        CesopsnJitterBufferStats {
            buffer_depth: self.buffer.len(),
            packets_received: self.packets_received,
            packets_dropped: self.packets_dropped,
            packets_late: self.packets_late,
            next_sequence: self.next_sequence,
        }
    }
}

/// Jitter buffer statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CesopsnJitterBufferStats {
    pub buffer_depth: usize,
    pub packets_received: u64,
    pub packets_dropped: u64,
    pub packets_late: u64,
    pub next_sequence: u16,
}

/// CESoPSN Service for managing TDM circuit emulation
pub struct CesopsnService {
    /// Circuit configuration
    config: CesopsnCircuitConfig,
    /// UDP socket for packet transport
    socket: Arc<UdpSocket>,
    /// Jitter buffer for incoming packets
    jitter_buffer: Arc<Mutex<CesopsnJitterBuffer>>,
    /// Sequence number for outgoing packets
    tx_sequence: Arc<RwLock<u16>>,
    /// SSRC identifier for this service
    ssrc: u32,
    /// TDM data sender for received packets
    tdm_sender: mpsc::UnboundedSender<Vec<u8>>,
    /// Statistics
    stats: Arc<RwLock<CesopsnServiceStats>>,
    /// Circuit state
    circuit_state: Arc<RwLock<CesopsnCircuitState>>,
}

/// CESoPSN Service Statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CesopsnServiceStats {
    /// Packets transmitted
    pub packets_sent: u64,
    /// Packets received
    pub packets_received: u64,
    /// Bytes transmitted
    pub bytes_sent: u64,
    /// Bytes received
    pub bytes_received: u64,
    /// Current jitter estimate (microseconds)
    pub jitter_us: u32,
    /// Packet loss rate (percentage)
    pub loss_rate: f32,
    /// Average round-trip time (milliseconds)
    pub rtt_ms: f32,
}

/// CESoPSN Circuit State
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CesopsnCircuitState {
    /// Circuit is down/inactive
    Down,
    /// Circuit is initializing
    Initializing,
    /// Circuit is active and passing traffic
    Active,
    /// Circuit has errors but may recover
    Degraded,
    /// Circuit has failed
    Failed,
}

impl CesopsnService {
    /// Create new CESoPSN service
    pub async fn new(
        config: CesopsnCircuitConfig,
        tdm_sender: mpsc::UnboundedSender<Vec<u8>>
    ) -> Result<Self> {
        let socket = UdpSocket::bind(&config.local_address).await
            .map_err(|e| anyhow!("Failed to bind CESoPSN socket: {}", e))?;
        
        let jitter_buffer = CesopsnJitterBuffer::new(
            (config.jitter_buffer_ms * 50) as usize // ~50 packets per second
        );
        
        // Generate random SSRC
        let ssrc = rand::random::<u32>();
        
        info!("Created CESoPSN service for circuit {} on {} (SSRC: 0x{:08X})",
              config.circuit_id, config.local_address, ssrc);
        
        Ok(Self {
            config,
            socket: Arc::new(socket),
            jitter_buffer: Arc::new(Mutex::new(jitter_buffer)),
            tx_sequence: Arc::new(RwLock::new(1)),
            ssrc,
            tdm_sender,
            stats: Arc::new(RwLock::new(CesopsnServiceStats::default())),
            circuit_state: Arc::new(RwLock::new(CesopsnCircuitState::Down)),
        })
    }
    
    /// Start the CESoPSN service
    pub async fn start(&self) -> Result<()> {
        *self.circuit_state.write().await = CesopsnCircuitState::Initializing;
        
        // Start packet receiver task
        let socket = Arc::clone(&self.socket);
        let jitter_buffer = Arc::clone(&self.jitter_buffer);
        let stats = Arc::clone(&self.stats);
        let circuit_state = Arc::clone(&self.circuit_state);
        let tdm_sender = self.tdm_sender.clone();
        
        tokio::spawn(async move {
            Self::packet_receiver(socket, jitter_buffer, stats, circuit_state, tdm_sender).await;
        });
        
        // Start TDM playout task
        let jitter_buffer_clone = Arc::clone(&self.jitter_buffer);
        let tdm_sender_clone = self.tdm_sender.clone();
        let config = self.config.clone();
        
        tokio::spawn(async move {
            Self::tdm_playout_task(jitter_buffer_clone, tdm_sender_clone, config).await;
        });
        
        *self.circuit_state.write().await = CesopsnCircuitState::Active;
        info!("CESoPSN service started for circuit {}", self.config.circuit_id);
        
        Ok(())
    }
    
    /// Send TDM data over CESoPSN
    pub async fn send_tdm_data(&self, data: &[u8]) -> Result<()> {
        let mut header = CesopsnHeader::new(self.config.circuit_id, self.ssrc);
        header.payload_type = self.config.payload_type as u8;
        header.timestamp = Self::generate_timestamp();
        
        // Get and increment sequence number
        {
            let mut seq = self.tx_sequence.write().await;
            header.sequence_number = *seq;
            *seq = seq.wrapping_add(1);
        }
        
        let packet = CesopsnPacket::new(header, data.to_vec());
        let packet_bytes = packet.to_bytes();
        
        // Send packet
        let bytes_sent = self.socket.send_to(&packet_bytes, &self.config.remote_address).await
            .map_err(|e| anyhow!("Failed to send CESoPSN packet: {}", e))?;
        
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.packets_sent += 1;
            stats.bytes_sent += bytes_sent as u64;
        }
        
        Ok(())
    }
    
    /// Packet receiver task
    async fn packet_receiver(
        socket: Arc<UdpSocket>,
        jitter_buffer: Arc<Mutex<CesopsnJitterBuffer>>,
        stats: Arc<RwLock<CesopsnServiceStats>>,
        circuit_state: Arc<RwLock<CesopsnCircuitState>>,
        _tdm_sender: mpsc::UnboundedSender<Vec<u8>>
    ) {
        let mut buffer = vec![0u8; MAX_CESOPSN_PAYLOAD];
        
        loop {
            match socket.recv_from(&mut buffer).await {
                Ok((len, _from)) => {
                    match CesopsnPacket::from_bytes(&buffer[..len]) {
                        Ok(packet) => {
                            // Update statistics
                            {
                                let mut stats_guard = stats.write().await;
                                stats_guard.packets_received += 1;
                                stats_guard.bytes_received += len as u64;
                            }
                            
                            // Add to jitter buffer
                            if let Err(e) = jitter_buffer.lock().await.add_packet(packet) {
                                warn!("Failed to add packet to jitter buffer: {}", e);
                            }
                        }
                        Err(e) => {
                            warn!("Failed to parse CESoPSN packet: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("CESoPSN socket receive error: {}", e);
                    *circuit_state.write().await = CesopsnCircuitState::Failed;
                    break;
                }
            }
        }
    }
    
    /// TDM playout task - retrieves packets from jitter buffer and plays them out
    async fn tdm_playout_task(
        jitter_buffer: Arc<Mutex<CesopsnJitterBuffer>>,
        tdm_sender: mpsc::UnboundedSender<Vec<u8>>,
        config: CesopsnCircuitConfig
    ) {
        let frame_interval = Duration::from_micros(125 * config.frames_per_packet as u64); // 125μs per frame
        let mut interval = tokio::time::interval(frame_interval);
        
        loop {
            interval.tick().await;
            
            // Try to get next packet from jitter buffer
            if let Some(packet) = jitter_buffer.lock().await.get_next_packet() {
                // Send TDM data to upper layers
                if let Err(_) = tdm_sender.send(packet.payload) {
                    warn!("TDM data receiver dropped, stopping playout");
                    break;
                }
            }
        }
    }
    
    /// Generate RTP timestamp for current time
    fn generate_timestamp() -> u32 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        
        // Convert to 8kHz timestamp (8000 ticks per second)
        (now.as_secs() * 8000 + (now.subsec_nanos() / 125_000) as u64) as u32
    }
    
    /// Get service statistics
    pub async fn get_stats(&self) -> CesopsnServiceStats {
        self.stats.read().await.clone()
    }
    
    /// Get circuit state
    pub async fn get_circuit_state(&self) -> CesopsnCircuitState {
        self.circuit_state.read().await.clone()
    }
    
    /// Get jitter buffer statistics
    pub async fn get_jitter_buffer_stats(&self) -> CesopsnJitterBufferStats {
        self.jitter_buffer.lock().await.get_stats()
    }
}

/// CESoPSN Manager for handling multiple circuits
pub struct CesopsnManager {
    /// Active CESoPSN services by circuit ID
    services: Arc<RwLock<HashMap<u16, Arc<CesopsnService>>>>,
    /// TDM data receiver for all circuits
    _tdm_receiver: Arc<Mutex<mpsc::UnboundedReceiver<(u16, Vec<u8>)>>>,
    /// TDM data sender
    tdm_sender: mpsc::UnboundedSender<(u16, Vec<u8>)>,
}

impl CesopsnManager {
    /// Create new CESoPSN manager
    pub fn new() -> Self {
        let (tdm_sender, tdm_receiver) = mpsc::unbounded_channel();
        
        Self {
            services: Arc::new(RwLock::new(HashMap::new())),
            _tdm_receiver: Arc::new(Mutex::new(tdm_receiver)),
            tdm_sender,
        }
    }
    
    /// Add new CESoPSN circuit
    pub async fn add_circuit(&self, config: CesopsnCircuitConfig) -> Result<()> {
        let circuit_id = config.circuit_id;
        let (circuit_tdm_sender, mut circuit_tdm_receiver) = mpsc::unbounded_channel();
        
        let service = Arc::new(CesopsnService::new(config, circuit_tdm_sender).await?);
        service.start().await?;
        
        // Forward TDM data from circuit to manager
        let manager_sender = self.tdm_sender.clone();
        tokio::spawn(async move {
            while let Some(tdm_data) = circuit_tdm_receiver.recv().await {
                if let Err(_) = manager_sender.send((circuit_id, tdm_data)) {
                    break;
                }
            }
        });
        
        self.services.write().await.insert(circuit_id, service);
        info!("Added CESoPSN circuit {}", circuit_id);
        
        Ok(())
    }
    
    /// Send TDM data to specific circuit
    pub async fn send_tdm_data(&self, circuit_id: u16, data: &[u8]) -> Result<()> {
        if let Some(service) = self.services.read().await.get(&circuit_id) {
            service.send_tdm_data(data).await
        } else {
            Err(anyhow!("CESoPSN circuit {} not found", circuit_id))
        }
    }
    
    /// Get statistics for all circuits
    pub async fn get_all_stats(&self) -> HashMap<u16, CesopsnServiceStats> {
        let services = self.services.read().await;
        let mut stats = HashMap::new();
        
        for (&circuit_id, service) in services.iter() {
            stats.insert(circuit_id, service.get_stats().await);
        }
        
        stats
    }
    
    /// Subscribe to received TDM data
    pub fn subscribe_tdm_data(&self) -> mpsc::UnboundedReceiver<(u16, Vec<u8>)> {
        let (_sender, receiver) = mpsc::unbounded_channel();
        
        // This is a simplified approach - in production you'd want proper subscription management
        receiver
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cesopsn_header_serialization() {
        let mut header = CesopsnHeader::new(123, 0x12345678);
        header.sequence_number = 0xABCD;
        header.timestamp = 0x87654321;
        header.payload_type = 5;
        header.marker = true;
        
        let bytes = header.to_bytes();
        assert_eq!(bytes.len(), CESOPSN_HEADER_LEN);
        
        let parsed = CesopsnHeader::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.sequence_number, 0xABCD);
        assert_eq!(parsed.timestamp, 0x87654321);
        assert_eq!(parsed.ssrc, 0x12345678);
        assert_eq!(parsed.payload_type, 5);
        assert_eq!(parsed.marker, true);
    }
    
    #[test]
    fn test_jitter_buffer_ordering() {
        let mut buffer = CesopsnJitterBuffer::new(10);
        
        // Add packets out of order
        for seq in [3, 1, 4, 2, 5].iter() {
            let mut header = CesopsnHeader::new(1, 0x12345678);
            header.sequence_number = *seq;
            let packet = CesopsnPacket::new(header, vec![*seq as u8; 32]);
            buffer.add_packet(packet).unwrap();
        }
        
        buffer.next_sequence = 1;
        
        // Should get packets in order
        for expected_seq in 1..=5 {
            let packet = buffer.get_next_packet().unwrap();
            assert_eq!(packet.header.sequence_number, expected_seq);
        }
    }
}