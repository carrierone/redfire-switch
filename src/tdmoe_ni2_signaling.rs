/*
 * TDMoE (Time Division Multiplexing over Ethernet) with NI-2 Signaling
 *
 * This module implements TDM over Ethernet with National ISDN-2 (NI-2) signaling
 * for integration with SIP endpoints in a complete call flow test.
 */

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, error, info, warn};

/// TDMoE frame structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TdmoeFrame {
    /// Frame sequence number
    pub sequence: u32,
    /// Timestamp
    pub timestamp: u64,
    /// Channel identifier (DS0 channel)
    pub channel: u16,
    /// TDM data payload (8 bytes for DS0)
    pub data: Vec<u8>,
    /// D-channel signaling data (if present)
    pub d_channel_data: Option<Vec<u8>>,
    /// Frame type
    pub frame_type: TdmoeFrameType,
}

/// TDMoE frame types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TdmoeFrameType {
    /// Voice/data frame
    Voice,
    /// D-channel signaling
    Signaling,
    /// Keepalive frame
    Keepalive,
    /// Synchronization frame
    Sync,
}

/// NI-2 (National ISDN-2) signaling message types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ni2MessageType {
    /// Initial Address Message (IAM)
    IAM,
    /// Address Complete Message (ACM)
    ACM,
    /// Answer Message (ANM)
    ANM,
    /// Release Message (REL)
    REL,
    /// Release Complete (RLC)
    RLC,
    /// Call Progress (CPG)
    CPG,
    /// Continuity Check Request (CCR)
    CCR,
    /// Continuity (COT)
    COT,
}

/// NI-2 signaling message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ni2Message {
    /// Message type
    pub message_type: Ni2MessageType,
    /// Circuit Identification Code (CIC)
    pub cic: u16,
    /// Calling party number
    pub calling_number: Option<String>,
    /// Called party number
    pub called_number: Option<String>,
    /// Originating line information (OLI)
    pub oli: Option<u8>,
    /// Charge number
    pub charge_number: Option<String>,
    /// Location routing number
    pub lrn: Option<String>,
    /// Jurisdiction information parameter
    pub jip: Option<String>,
    /// Custom parameters
    pub parameters: HashMap<String, String>,
    /// Message timestamp
    pub timestamp: u64,
}

/// TDMoE trunk configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TdmoeConfig {
    /// Local endpoint address
    pub local_address: SocketAddr,
    /// Remote endpoint address
    pub remote_address: SocketAddr,
    /// Number of DS0 channels (typically 24 for T1, 30 for E1)
    pub channel_count: u16,
    /// Trunk type
    pub trunk_type: TdmoeTrunkType,
    /// NI-2 signaling configuration
    pub ni2_config: Ni2Config,
    /// Codec for voice channels
    pub voice_codec: TdmoeCodec,
    /// Enable packet loss detection
    pub enable_packet_loss_detection: bool,
    /// Jitter buffer size (ms)
    pub jitter_buffer_ms: u32,
}

/// TDMoE trunk types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TdmoeTrunkType {
    /// T1 (24 channels)
    T1,
    /// E1 (30 channels)
    E1,
    /// Fractional T1
    FractionalT1(u16),
}

/// NI-2 configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ni2Config {
    /// Point code
    pub point_code: u32,
    /// Network indicator
    pub network_indicator: u8,
    /// Service indicator
    pub service_indicator: u8,
    /// Subsystem number
    pub subsystem_number: u8,
    /// Enable continuity testing
    pub enable_continuity_test: bool,
    /// Answer supervision timeout (seconds)
    pub answer_supervision_timeout: u32,
}

/// TDMoE voice codecs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TdmoeCodec {
    /// μ-law (North America)
    ULaw,
    /// A-law (International)
    ALaw,
    /// Clear channel (no encoding)
    ClearChannel,
}

/// TDMoE call state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TdmoeCallState {
    /// Idle - no call
    Idle,
    /// Outgoing call setup
    OutgoingSetup,
    /// Incoming call setup
    IncomingSetup,
    /// Call proceeding
    Proceeding,
    /// Alerting (ringing)
    Alerting,
    /// Connected (answered)
    Connected,
    /// Disconnecting
    Disconnecting,
    /// Continuity test in progress
    ContinuityTest,
}

/// Active TDMoE call
#[derive(Debug, Clone)]
pub struct TdmoeCall {
    /// Call identifier
    pub call_id: String,
    /// Circuit identification code
    pub cic: u16,
    /// Channel number
    pub channel: u16,
    /// Call state
    pub state: TdmoeCallState,
    /// Calling number
    pub calling_number: String,
    /// Called number
    pub called_number: String,
    /// Call start time
    pub start_time: Instant,
    /// Last activity time
    pub last_activity: Instant,
    /// Codec in use
    pub codec: TdmoeCodec,
    /// RTP statistics
    pub stats: TdmoeCallStats,
}

/// TDMoE call statistics
#[derive(Debug, Clone, Default)]
pub struct TdmoeCallStats {
    /// Packets sent
    pub packets_sent: u64,
    /// Packets received
    pub packets_received: u64,
    /// Bytes sent
    pub bytes_sent: u64,
    /// Bytes received
    pub bytes_received: u64,
    /// Packets lost
    pub packets_lost: u64,
    /// Jitter (ms)
    pub jitter_ms: f64,
    /// Round-trip time (ms)
    pub rtt_ms: f64,
}

/// TDMoE service implementation
pub struct TdmoeService {
    /// Configuration
    config: TdmoeConfig,
    /// UDP socket for TDMoE frames
    socket: Arc<UdpSocket>,
    /// Active calls by CIC
    active_calls: Arc<RwLock<HashMap<u16, TdmoeCall>>>,
    /// Channel states (indexed by channel number)
    channel_states: Arc<RwLock<Vec<TdmoeCallState>>>,
    /// Signaling message queue
    signaling_queue: Arc<Mutex<mpsc::UnboundedSender<Ni2Message>>>,
    /// Frame sequence counter
    sequence_counter: Arc<Mutex<u32>>,
    /// Statistics
    stats: Arc<RwLock<TdmoeServiceStats>>,
}

/// TDMoE service statistics
#[derive(Debug, Default, Clone)]
pub struct TdmoeServiceStats {
    /// Total frames processed
    pub total_frames: u64,
    /// Voice frames processed
    pub voice_frames: u64,
    /// Signaling messages processed
    pub signaling_messages: u64,
    /// Active calls
    pub active_calls: u32,
    /// Service uptime
    pub uptime: Duration,
    /// Last activity time
    pub last_activity: Option<Instant>,
}

impl Default for TdmoeConfig {
    fn default() -> Self {
        Self {
            local_address: "0.0.0.0:9000".parse().unwrap(),
            remote_address: "127.0.0.1:9001".parse().unwrap(),
            channel_count: 24, // T1
            trunk_type: TdmoeTrunkType::T1,
            ni2_config: Ni2Config::default(),
            voice_codec: TdmoeCodec::ULaw,
            enable_packet_loss_detection: true,
            jitter_buffer_ms: 20,
        }
    }
}

impl Default for Ni2Config {
    fn default() -> Self {
        Self {
            point_code: 1,
            network_indicator: 2,  // National network
            service_indicator: 3,  // SCCP
            subsystem_number: 254, // ISUP
            enable_continuity_test: true,
            answer_supervision_timeout: 60,
        }
    }
}

impl TdmoeService {
    /// Create new TDMoE service
    pub async fn new(config: TdmoeConfig) -> Result<Self> {
        info!("Creating TDMoE service with config: {:?}", config);

        // Bind UDP socket
        let socket = UdpSocket::bind(config.local_address).await.map_err(|e| {
            anyhow!(
                "Failed to bind TDMoE socket to {}: {}",
                config.local_address,
                e
            )
        })?;

        info!("TDMoE service bound to {}", config.local_address);

        // Initialize channel states
        let mut channel_states = Vec::new();
        for _ in 0..config.channel_count {
            channel_states.push(TdmoeCallState::Idle);
        }

        // Create signaling message queue
        let (signaling_tx, _signaling_rx) = mpsc::unbounded_channel();

        let service = Self {
            config,
            socket: Arc::new(socket),
            active_calls: Arc::new(RwLock::new(HashMap::new())),
            channel_states: Arc::new(RwLock::new(channel_states)),
            signaling_queue: Arc::new(Mutex::new(signaling_tx)),
            sequence_counter: Arc::new(Mutex::new(0)),
            stats: Arc::new(RwLock::new(TdmoeServiceStats::default())),
        };

        Ok(service)
    }

    /// Start the TDMoE service
    pub async fn start(&self) -> Result<()> {
        info!("Starting TDMoE service");

        // Start frame receiver
        let socket = Arc::clone(&self.socket);
        let active_calls = Arc::clone(&self.active_calls);
        let stats = Arc::clone(&self.stats);

        tokio::spawn(async move {
            let mut buffer = vec![0u8; 1500]; // MTU size

            loop {
                match socket.recv_from(&mut buffer).await {
                    Ok((size, addr)) => {
                        debug!("Received {} bytes from {}", size, addr);

                        // Parse TDMoE frame
                        match Self::parse_tdmoe_frame(&buffer[..size]) {
                            Ok(frame) => {
                                Self::process_frame(frame, &active_calls, &stats).await;
                            }
                            Err(e) => {
                                warn!("Failed to parse TDMoE frame: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("TDMoE socket receive error: {}", e);
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        });

        // Start keepalive sender
        self.start_keepalive_sender().await;

        info!("TDMoE service started successfully");
        Ok(())
    }

    /// Originate a call (send IAM)
    pub async fn originate_call(
        &self,
        calling_number: &str,
        called_number: &str,
        cic: u16,
    ) -> Result<String> {
        info!(
            "Originating call from {} to {} on CIC {}",
            calling_number, called_number, cic
        );

        // Check if CIC is available
        let active_calls = self.active_calls.read().await;
        if active_calls.contains_key(&cic) {
            return Err(anyhow!("CIC {} is already in use", cic));
        }
        drop(active_calls);

        // Generate call ID
        let call_id = format!(
            "TDMOE-{}-{}",
            cic,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis()
        );

        // Create IAM message
        let iam = Ni2Message {
            message_type: Ni2MessageType::IAM,
            cic,
            calling_number: Some(calling_number.to_string()),
            called_number: Some(called_number.to_string()),
            oli: Some(0), // Normal calling party
            charge_number: None,
            lrn: None,
            jip: None,
            parameters: HashMap::new(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        // Send IAM
        self.send_signaling_message(iam).await?;

        // Create call record
        let call = TdmoeCall {
            call_id: call_id.clone(),
            cic,
            channel: cic, // Assuming 1:1 mapping for simplicity
            state: TdmoeCallState::OutgoingSetup,
            calling_number: calling_number.to_string(),
            called_number: called_number.to_string(),
            start_time: Instant::now(),
            last_activity: Instant::now(),
            codec: self.config.voice_codec,
            stats: TdmoeCallStats::default(),
        };

        // Store call
        let mut active_calls = self.active_calls.write().await;
        active_calls.insert(cic, call);

        // Update channel state
        let mut channel_states = self.channel_states.write().await;
        if (cic as usize) < channel_states.len() {
            channel_states[cic as usize] = TdmoeCallState::OutgoingSetup;
        }

        info!("Call {} originated successfully", call_id);
        Ok(call_id)
    }

    /// Send answer (ANM) for incoming call
    pub async fn answer_call(&self, cic: u16) -> Result<()> {
        info!("Answering call on CIC {}", cic);

        // Find the call
        let mut active_calls = self.active_calls.write().await;
        let call = active_calls
            .get_mut(&cic)
            .ok_or_else(|| anyhow!("No call found on CIC {}", cic))?;

        if call.state != TdmoeCallState::IncomingSetup && call.state != TdmoeCallState::Alerting {
            return Err(anyhow!(
                "Call on CIC {} is not in a state to be answered",
                cic
            ));
        }

        // Update call state
        call.state = TdmoeCallState::Connected;
        call.last_activity = Instant::now();

        // Create ANM message
        let anm = Ni2Message {
            message_type: Ni2MessageType::ANM,
            cic,
            calling_number: None,
            called_number: None,
            oli: None,
            charge_number: None,
            lrn: None,
            jip: None,
            parameters: HashMap::new(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        // Send ANM
        self.send_signaling_message(anm).await?;

        // Update channel state
        let mut channel_states = self.channel_states.write().await;
        if (cic as usize) < channel_states.len() {
            channel_states[cic as usize] = TdmoeCallState::Connected;
        }

        info!("Call on CIC {} answered successfully", cic);
        Ok(())
    }

    /// Release call (send REL)
    pub async fn release_call(&self, cic: u16, cause: u8) -> Result<()> {
        info!("Releasing call on CIC {} with cause {}", cic, cause);

        // Find the call
        let mut active_calls = self.active_calls.write().await;
        let call = active_calls
            .get_mut(&cic)
            .ok_or_else(|| anyhow!("No call found on CIC {}", cic))?;

        // Update call state
        call.state = TdmoeCallState::Disconnecting;
        call.last_activity = Instant::now();

        // Create REL message
        let mut parameters = HashMap::new();
        parameters.insert("cause".to_string(), cause.to_string());

        let rel = Ni2Message {
            message_type: Ni2MessageType::REL,
            cic,
            calling_number: None,
            called_number: None,
            oli: None,
            charge_number: None,
            lrn: None,
            jip: None,
            parameters,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        // Send REL
        self.send_signaling_message(rel).await?;

        info!("Release message sent for CIC {}", cic);
        Ok(())
    }

    /// Send voice data on a channel
    pub async fn send_voice_data(&self, cic: u16, data: &[u8]) -> Result<()> {
        // Check if call is active
        let active_calls = self.active_calls.read().await;
        let call = active_calls
            .get(&cic)
            .ok_or_else(|| anyhow!("No active call on CIC {}", cic))?;

        if call.state != TdmoeCallState::Connected {
            return Err(anyhow!("Call on CIC {} is not in connected state", cic));
        }
        drop(active_calls);

        // Create voice frame
        let sequence = {
            let mut seq = self.sequence_counter.lock().await;
            *seq += 1;
            *seq
        };

        let frame = TdmoeFrame {
            sequence,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            channel: cic,
            data: data.to_vec(),
            d_channel_data: None,
            frame_type: TdmoeFrameType::Voice,
        };

        // Send frame
        self.send_frame(frame).await?;

        // Update statistics
        let mut active_calls = self.active_calls.write().await;
        if let Some(call) = active_calls.get_mut(&cic) {
            call.stats.packets_sent += 1;
            call.stats.bytes_sent += data.len() as u64;
            call.last_activity = Instant::now();
        }

        Ok(())
    }

    /// Get call status
    pub async fn get_call_status(&self, cic: u16) -> Option<TdmoeCall> {
        let active_calls = self.active_calls.read().await;
        active_calls.get(&cic).cloned()
    }

    /// List all active calls
    pub async fn list_active_calls(&self) -> Vec<TdmoeCall> {
        let active_calls = self.active_calls.read().await;
        active_calls.values().cloned().collect()
    }

    /// Get service statistics
    pub async fn get_statistics(&self) -> TdmoeServiceStats {
        let stats = self.stats.read().await;
        let mut stats_copy = (*stats).clone();

        // Update active calls count
        let active_calls = self.active_calls.read().await;
        stats_copy.active_calls = active_calls.len() as u32;

        stats_copy
    }

    // Private methods

    async fn send_signaling_message(&self, message: Ni2Message) -> Result<()> {
        debug!("Sending NI-2 message: {:?}", message);

        // Create signaling frame
        let signaling_data = self.serialize_ni2_message(&message)?;

        let sequence = {
            let mut seq = self.sequence_counter.lock().await;
            *seq += 1;
            *seq
        };

        let frame = TdmoeFrame {
            sequence,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            channel: 0, // D-channel
            data: vec![],
            d_channel_data: Some(signaling_data),
            frame_type: TdmoeFrameType::Signaling,
        };

        self.send_frame(frame).await?;

        // Update statistics
        let mut stats = self.stats.write().await;
        stats.signaling_messages += 1;

        Ok(())
    }

    async fn send_frame(&self, frame: TdmoeFrame) -> Result<()> {
        let frame_data = self.serialize_frame(&frame)?;

        self.socket
            .send_to(&frame_data, self.config.remote_address)
            .await
            .map_err(|e| anyhow!("Failed to send TDMoE frame: {}", e))?;

        // Update statistics
        let mut stats = self.stats.write().await;
        stats.total_frames += 1;
        if frame.frame_type == TdmoeFrameType::Voice {
            stats.voice_frames += 1;
        }
        stats.last_activity = Some(Instant::now());

        debug!(
            "Sent TDMoE frame: seq={}, type={:?}, channel={}",
            frame.sequence, frame.frame_type, frame.channel
        );

        Ok(())
    }

    fn serialize_frame(&self, frame: &TdmoeFrame) -> Result<Vec<u8>> {
        // Simple binary serialization
        // In production, this would use a proper protocol like RTP or custom framing
        bincode::serialize(frame).map_err(|e| anyhow!("Failed to serialize TDMoE frame: {}", e))
    }

    fn parse_tdmoe_frame(data: &[u8]) -> Result<TdmoeFrame> {
        bincode::deserialize(data).map_err(|e| anyhow!("Failed to deserialize TDMoE frame: {}", e))
    }

    async fn process_frame(
        frame: TdmoeFrame,
        active_calls: &Arc<RwLock<HashMap<u16, TdmoeCall>>>,
        stats: &Arc<RwLock<TdmoeServiceStats>>,
    ) {
        debug!(
            "Processing TDMoE frame: seq={}, type={:?}, channel={}",
            frame.sequence, frame.frame_type, frame.channel
        );

        match frame.frame_type {
            TdmoeFrameType::Voice => {
                // Update call statistics
                let mut calls = active_calls.write().await;
                if let Some(call) = calls.get_mut(&frame.channel) {
                    call.stats.packets_received += 1;
                    call.stats.bytes_received += frame.data.len() as u64;
                    call.last_activity = Instant::now();
                }
            }
            TdmoeFrameType::Signaling => {
                if let Some(d_channel_data) = &frame.d_channel_data {
                    if let Ok(message) = Self::deserialize_ni2_message(d_channel_data) {
                        Self::process_signaling_message(message, active_calls).await;
                    }
                }
            }
            TdmoeFrameType::Keepalive => {
                debug!("Received keepalive frame");
            }
            TdmoeFrameType::Sync => {
                debug!("Received sync frame");
            }
        }

        // Update global statistics
        let mut stats_guard = stats.write().await;
        stats_guard.total_frames += 1;
        if frame.frame_type == TdmoeFrameType::Voice {
            stats_guard.voice_frames += 1;
        }
        stats_guard.last_activity = Some(Instant::now());
    }

    async fn process_signaling_message(
        message: Ni2Message,
        active_calls: &Arc<RwLock<HashMap<u16, TdmoeCall>>>,
    ) {
        info!("Processing NI-2 message: {:?}", message);

        let mut calls = active_calls.write().await;

        match message.message_type {
            Ni2MessageType::IAM => {
                // Incoming call
                let call = TdmoeCall {
                    call_id: format!("TDMOE-{}-{}", message.cic, message.timestamp),
                    cic: message.cic,
                    channel: message.cic,
                    state: TdmoeCallState::IncomingSetup,
                    calling_number: message.calling_number.unwrap_or_default(),
                    called_number: message.called_number.unwrap_or_default(),
                    start_time: Instant::now(),
                    last_activity: Instant::now(),
                    codec: TdmoeCodec::ULaw, // Default
                    stats: TdmoeCallStats::default(),
                };

                calls.insert(message.cic, call);
                info!("Incoming call setup on CIC {}", message.cic);
            }
            Ni2MessageType::ACM => {
                if let Some(call) = calls.get_mut(&message.cic) {
                    call.state = TdmoeCallState::Proceeding;
                    call.last_activity = Instant::now();
                    info!("Call proceeding on CIC {}", message.cic);
                }
            }
            Ni2MessageType::ANM => {
                if let Some(call) = calls.get_mut(&message.cic) {
                    call.state = TdmoeCallState::Connected;
                    call.last_activity = Instant::now();
                    info!("Call answered on CIC {}", message.cic);
                }
            }
            Ni2MessageType::REL => {
                if let Some(call) = calls.get_mut(&message.cic) {
                    call.state = TdmoeCallState::Disconnecting;
                    call.last_activity = Instant::now();
                    info!("Call released on CIC {}", message.cic);
                }
            }
            Ni2MessageType::RLC => {
                calls.remove(&message.cic);
                info!("Call cleared on CIC {}", message.cic);
            }
            _ => {
                debug!("Unhandled NI-2 message type: {:?}", message.message_type);
            }
        }
    }

    fn serialize_ni2_message(&self, message: &Ni2Message) -> Result<Vec<u8>> {
        bincode::serialize(message).map_err(|e| anyhow!("Failed to serialize NI-2 message: {}", e))
    }

    fn deserialize_ni2_message(data: &[u8]) -> Result<Ni2Message> {
        bincode::deserialize(data).map_err(|e| anyhow!("Failed to deserialize NI-2 message: {}", e))
    }

    async fn start_keepalive_sender(&self) {
        let socket = Arc::clone(&self.socket);
        let remote_addr = self.config.remote_address;
        let sequence_counter = Arc::clone(&self.sequence_counter);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));

            loop {
                interval.tick().await;

                let sequence = {
                    let mut seq = sequence_counter.lock().await;
                    *seq += 1;
                    *seq
                };

                let keepalive_frame = TdmoeFrame {
                    sequence,
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64,
                    channel: 0,
                    data: vec![],
                    d_channel_data: None,
                    frame_type: TdmoeFrameType::Keepalive,
                };

                if let Ok(frame_data) = bincode::serialize(&keepalive_frame) {
                    if let Err(e) = socket.send_to(&frame_data, remote_addr).await {
                        warn!("Failed to send keepalive: {}", e);
                    } else {
                        debug!("Sent keepalive frame");
                    }
                }
            }
        });
    }
}

/// TDMoE trunk pair for loopback testing
pub struct TdmoeTrunkPair {
    /// Ingress trunk (SIP -> TDMoE)
    pub ingress: Arc<TdmoeService>,
    /// Egress trunk (TDMoE -> SIP)
    pub egress: Arc<TdmoeService>,
}

impl TdmoeTrunkPair {
    /// Create a loopback TDMoE trunk pair
    pub async fn create_loopback_pair() -> Result<Self> {
        let ingress_config = TdmoeConfig {
            local_address: "127.0.0.1:9000".parse().unwrap(),
            remote_address: "127.0.0.1:9001".parse().unwrap(),
            ..TdmoeConfig::default()
        };

        let egress_config = TdmoeConfig {
            local_address: "127.0.0.1:9001".parse().unwrap(),
            remote_address: "127.0.0.1:9000".parse().unwrap(),
            ..TdmoeConfig::default()
        };

        let ingress = Arc::new(TdmoeService::new(ingress_config).await?);
        let egress = Arc::new(TdmoeService::new(egress_config).await?);

        Ok(Self { ingress, egress })
    }

    /// Start both trunks
    pub async fn start(&self) -> Result<()> {
        self.ingress.start().await?;
        self.egress.start().await?;

        // Allow time for services to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok(())
    }
}

/// NI-2 Side Type (Network vs User)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ni2SideType {
    /// Network side (switch/carrier)
    Network,
    /// User side (customer/endpoint)
    User,
}

/// NI-2 Call State per ITU-T Q.931
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ni2CallState {
    /// Null state (no call)
    Null = 0,
    /// Call initiated (U1/N1)
    CallInitiated = 1,
    /// Overlap sending (U2/N2)
    OverlapSending = 2,
    /// Outgoing call proceeding (U3/N3)
    OutgoingCallProceeding = 3,
    /// Call delivered (U4/N4)
    CallDelivered = 4,
    /// Call present (U6/N6)
    CallPresent = 6,
    /// Call received (U7/N7)
    CallReceived = 7,
    /// Connect request (U8/N8)
    ConnectRequest = 8,
    /// Incoming call proceeding (U9/N9)
    IncomingCallProceeding = 9,
    /// Active (U10/N10)
    Active = 10,
    /// Disconnect request (U11/N11)
    DisconnectRequest = 11,
    /// Disconnect indication (U12/N12)
    DisconnectIndication = 12,
    /// Release request (U19/N19)
    ReleaseRequest = 19,
    /// Call abort (U25/N25)
    CallAbort = 25,
}

/// NI-2 Layer 3 Protocol Discriminator
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ni2ProtocolDiscriminator {
    /// Q.931/I.451 Call Control = 0x08
    CallControl = 0x08,
    /// Maintenance = 0x3F
    Maintenance = 0x3F,
}

/// TDMoE NI-2 signaling processor
pub struct TdmoeNi2Signaling {
    /// NI-2 side configuration
    side_type: Ni2SideType,
    /// Active calls with call state tracking
    active_calls: Arc<RwLock<HashMap<String, Ni2CallContext>>>,
    /// Event sender
    event_sender: tokio::sync::broadcast::Sender<Ni2Event>,
    /// Call reference counter (for Network side)
    call_ref_counter: Arc<RwLock<u16>>,
}

/// NI-2 Call Context with state management
#[derive(Debug, Clone)]
pub struct Ni2CallContext {
    /// Call reference value (CRV)
    pub call_reference: u16,
    /// Current call state
    pub state: Ni2CallState,
    /// Channel/CIC identifier
    pub channel_id: String,
    /// Calling party number
    pub calling_number: Option<String>,
    /// Called party number  
    pub called_number: Option<String>,
    /// Bearer capability
    pub bearer_capability: Option<String>,
    /// Side type (Network/User)
    pub side_type: Ni2SideType,
    /// Last state change timestamp
    pub last_state_change: std::time::Instant,
    /// Call start time
    pub call_start_time: Option<std::time::Instant>,
}

impl TdmoeNi2Signaling {
    /// Create new NI-2 signaling processor
    pub fn new() -> Result<Self> {
        Self::new_with_side(Ni2SideType::User) // Default to User side
    }

    /// Create new NI-2 signaling processor with specific side
    pub fn new_with_side(side_type: Ni2SideType) -> Result<Self> {
        let (event_sender, _) = tokio::sync::broadcast::channel(1000);

        Ok(Self {
            side_type,
            active_calls: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
            call_ref_counter: Arc::new(RwLock::new(1)),
        })
    }

    /// Get the configured side type
    pub fn get_side_type(&self) -> Ni2SideType {
        self.side_type
    }

    /// Subscribe to NI-2 events
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<Ni2Event> {
        self.event_sender.subscribe()
    }

    /// Initiate outgoing call (User side sends SETUP)
    pub async fn initiate_call(
        &self,
        channel_id: &str,
        calling_number: &str,
        called_number: &str,
    ) -> Result<u16> {
        if self.side_type != Ni2SideType::User {
            return Err(anyhow!("Only User side can initiate calls"));
        }

        let mut call_ref_guard = self.call_ref_counter.write().await;
        let call_reference = *call_ref_guard;
        *call_ref_guard += 1;
        drop(call_ref_guard);

        let call_context = Ni2CallContext {
            call_reference,
            state: Ni2CallState::CallInitiated,
            channel_id: channel_id.to_string(),
            calling_number: Some(calling_number.to_string()),
            called_number: Some(called_number.to_string()),
            bearer_capability: Some("speech".to_string()),
            side_type: self.side_type,
            last_state_change: std::time::Instant::now(),
            call_start_time: Some(std::time::Instant::now()),
        };

        self.active_calls
            .write()
            .await
            .insert(channel_id.to_string(), call_context.clone());

        info!(
            "User side initiated call {} -> {} on {} (CRV: {})",
            calling_number, called_number, channel_id, call_reference
        );

        let event = Ni2Event::CallInitiated {
            channel_id: channel_id.to_string(),
            call_reference,
            calling_number: calling_number.to_string(),
            called_number: called_number.to_string(),
        };
        let _ = self.event_sender.send(event);

        Ok(call_reference)
    }

    /// Process incoming SETUP message (Network side)
    pub async fn process_incoming_setup(
        &self,
        channel_id: &str,
        call_reference: u16,
        calling_number: &str,
        called_number: &str,
    ) -> Result<()> {
        if self.side_type != Ni2SideType::Network {
            return Err(anyhow!("Only Network side can process incoming SETUP"));
        }

        let call_context = Ni2CallContext {
            call_reference,
            state: Ni2CallState::CallPresent,
            channel_id: channel_id.to_string(),
            calling_number: Some(calling_number.to_string()),
            called_number: Some(called_number.to_string()),
            bearer_capability: Some("speech".to_string()),
            side_type: self.side_type,
            last_state_change: std::time::Instant::now(),
            call_start_time: Some(std::time::Instant::now()),
        };

        self.active_calls
            .write()
            .await
            .insert(channel_id.to_string(), call_context);

        info!(
            "Network side received SETUP {} -> {} on {} (CRV: {})",
            calling_number, called_number, channel_id, call_reference
        );

        let event = Ni2Event::CallPresent {
            channel_id: channel_id.to_string(),
            call_reference,
            calling_number: calling_number.to_string(),
            called_number: called_number.to_string(),
        };
        let _ = self.event_sender.send(event);

        Ok(())
    }

    /// Send CALL PROCEEDING (Network side response to SETUP)
    pub async fn send_call_proceeding(&self, channel_id: &str) -> Result<()> {
        if self.side_type != Ni2SideType::Network {
            return Err(anyhow!("Only Network side can send CALL PROCEEDING"));
        }

        if let Some(mut call_context) = self.active_calls.write().await.get_mut(channel_id) {
            if call_context.state != Ni2CallState::CallPresent {
                return Err(anyhow!(
                    "Invalid state for CALL PROCEEDING: {:?}",
                    call_context.state
                ));
            }

            call_context.state = Ni2CallState::IncomingCallProceeding;
            call_context.last_state_change = std::time::Instant::now();

            info!(
                "Network side sent CALL PROCEEDING on {} (CRV: {})",
                channel_id, call_context.call_reference
            );

            let event = Ni2Event::CallProceeding {
                channel_id: channel_id.to_string(),
                call_reference: call_context.call_reference,
            };
            let _ = self.event_sender.send(event);

            Ok(())
        } else {
            Err(anyhow!("No active call found for channel {}", channel_id))
        }
    }

    /// Send ALERTING (Network side indicates ringing)
    pub async fn send_alerting(&self, channel_id: &str) -> Result<()> {
        if self.side_type != Ni2SideType::Network {
            return Err(anyhow!("Only Network side can send ALERTING"));
        }

        if let Some(mut call_context) = self.active_calls.write().await.get_mut(channel_id) {
            if call_context.state != Ni2CallState::IncomingCallProceeding {
                return Err(anyhow!(
                    "Invalid state for ALERTING: {:?}",
                    call_context.state
                ));
            }

            call_context.state = Ni2CallState::CallDelivered;
            call_context.last_state_change = std::time::Instant::now();

            info!(
                "Network side sent ALERTING on {} (CRV: {})",
                channel_id, call_context.call_reference
            );

            let event = Ni2Event::CallAlerting {
                channel_id: channel_id.to_string(),
                call_reference: call_context.call_reference,
            };
            let _ = self.event_sender.send(event);

            Ok(())
        } else {
            Err(anyhow!("No active call found for channel {}", channel_id))
        }
    }

    /// Send CONNECT (Network side indicates answer)
    pub async fn send_connect(&self, channel_id: &str) -> Result<()> {
        if self.side_type != Ni2SideType::Network {
            return Err(anyhow!("Only Network side can send CONNECT"));
        }

        if let Some(mut call_context) = self.active_calls.write().await.get_mut(channel_id) {
            if call_context.state != Ni2CallState::CallDelivered {
                return Err(anyhow!(
                    "Invalid state for CONNECT: {:?}",
                    call_context.state
                ));
            }

            call_context.state = Ni2CallState::Active;
            call_context.last_state_change = std::time::Instant::now();

            info!(
                "Network side sent CONNECT on {} (CRV: {}) - Call Active",
                channel_id, call_context.call_reference
            );

            let event = Ni2Event::CallConnected {
                channel_id: channel_id.to_string(),
                call_reference: call_context.call_reference,
            };
            let _ = self.event_sender.send(event);

            Ok(())
        } else {
            Err(anyhow!("No active call found for channel {}", channel_id))
        }
    }

    /// Send DISCONNECT (either side can disconnect)
    pub async fn send_disconnect(&self, channel_id: &str, cause: u8) -> Result<()> {
        if let Some(mut call_context) = self.active_calls.write().await.get_mut(channel_id) {
            if call_context.state == Ni2CallState::Null {
                return Err(anyhow!("Call already in null state"));
            }

            call_context.state = Ni2CallState::DisconnectRequest;
            call_context.last_state_change = std::time::Instant::now();

            info!(
                "{:?} side sent DISCONNECT on {} (CRV: {}, cause: {})",
                self.side_type, channel_id, call_context.call_reference, cause
            );

            let event = Ni2Event::CallDisconnected {
                channel_id: channel_id.to_string(),
                call_reference: call_context.call_reference,
                cause,
            };
            let _ = self.event_sender.send(event);

            Ok(())
        } else {
            Err(anyhow!("No active call found for channel {}", channel_id))
        }
    }

    /// Get call state
    pub async fn get_call_state(&self, channel_id: &str) -> Option<Ni2CallState> {
        self.active_calls
            .read()
            .await
            .get(channel_id)
            .map(|ctx| ctx.state)
    }

    /// Get all active calls
    pub async fn get_active_calls(&self) -> HashMap<String, Ni2CallContext> {
        self.active_calls.read().await.clone()
    }

    /// Send information element
    pub async fn send_information_element(
        &self,
        channel_id: &str,
        ie: InformationElement,
    ) -> Result<()> {
        info!("Sending IE to {}: {:?}", channel_id, ie);

        let event = Ni2Event::InformationElementSent {
            channel_id: channel_id.to_string(),
            element: ie,
        };

        let _ = self.event_sender.send(event);
        Ok(())
    }

    /// Process D-channel message data
    pub async fn process_d_channel_message(&self, channel_id: &str, data: &[u8]) -> Result<()> {
        // Simplified D-channel processing - in real implementation this would
        // need HDLC framing and Q.921 LAPD protocol handling
        if data.len() >= 4 {
            debug!(
                "Processing D-channel data on {}: {} bytes",
                channel_id,
                data.len()
            );
            // For now, just log the data - full NI-2 processing would happen here
            debug!(
                "D-channel data: {:02X?}",
                &data[..std::cmp::min(data.len(), 8)]
            );
        }
        Ok(())
    }
}

/// NI-2 events
#[derive(Debug, Clone)]
pub enum Ni2Event {
    /// D-channel message received
    MessageReceived {
        channel_id: String,
        message: Vec<u8>,
    },
    /// Information element sent
    InformationElementSent {
        channel_id: String,
        element: InformationElement,
    },
    /// Call initiated by User side
    CallInitiated {
        channel_id: String,
        call_reference: u16,
        calling_number: String,
        called_number: String,
    },
    /// Call present at Network side
    CallPresent {
        channel_id: String,
        call_reference: u16,
        calling_number: String,
        called_number: String,
    },
    /// Call proceeding sent by Network side
    CallProceeding {
        channel_id: String,
        call_reference: u16,
    },
    /// Call alerting (ringing) sent by Network side
    CallAlerting {
        channel_id: String,
        call_reference: u16,
    },
    /// Call connected (answered)
    CallConnected {
        channel_id: String,
        call_reference: u16,
    },
    /// Call disconnected
    CallDisconnected {
        channel_id: String,
        call_reference: u16,
        cause: u8,
    },
    /// Call state changed
    CallStateChanged {
        channel_id: String,
        old_state: Ni2CallState,
        new_state: Ni2CallState,
    },
}

/// Q.931 Information Elements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InformationElement {
    /// Called Party Number (0x70)
    CalledPartyNumber {
        number: String,
        plan: NumberingPlan,
        nature: NumberNature,
    },
    /// User-to-User Information (0x7E)
    UserToUserInformation {
        protocol_discriminator: u8,
        data: Vec<u8>,
    },
}

/// Numbering plan identification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NumberingPlan {
    /// Unknown numbering plan
    Unknown = 0x00,
    /// ISDN/telephony numbering plan (ITU-T E.164)
    Isdn = 0x01,
    /// Data numbering plan (ITU-T X.121)
    Data = 0x03,
    /// Telex numbering plan (ITU-T F.69)
    Telex = 0x04,
    /// Private numbering plan
    Private = 0x09,
}

/// Nature of number
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NumberNature {
    /// Unknown nature
    Unknown = 0x00,
    /// International number
    International = 0x01,
    /// National significant number
    National = 0x02,
    /// Network specific number
    NetworkSpecific = 0x03,
    /// Subscriber number
    Subscriber = 0x04,
    /// Abbreviated number
    Abbreviated = 0x06,
}
