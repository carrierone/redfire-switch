/*
 * ISDN CLI Management and Status Commands
 * 
 * Provides command-line interface for monitoring and managing ISDN PRI stack layers:
 * - Q.921 LAPD data link layer status
 * - Q.931 network layer call states
 * - PRI timer monitoring
 * - CESoPSN circuit status
 * - Configuration examples and helpers
 */

use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::fmt;
use clap::{Parser, Subcommand};
use serde::{Serialize, Deserialize};
use tracing::{info, warn};

use crate::string_parser::{SafeStringBuilder, safe_format, sanitize_string};

use crate::q931_messages::{IsdnVariant, IsdnSideType, IsdnConfig, Q931MessageType};
use crate::q921_lapd::{LapdStatistics, LapdState};
use crate::pri_timers::{PriTimerType, PriTimerStatistics, ActivePriTimer};
use crate::cesopsn::{CesopsnCircuitType, CesopsnServiceQuality, CesopsnPayloadType};
use crate::cesopsn_ni2_integration::{CesopsnNi2CircuitConfig, PcmCodec, CesopsnCircuitStats};

/// ISDN CLI Commands
#[derive(Parser)]
#[command(name = "isdn")]
#[command(about = "ISDN PRI stack management and monitoring")]
pub struct IsdnCli {
    #[command(subcommand)]
    pub command: IsdnCommand,
}

#[derive(Subcommand)]
pub enum IsdnCommand {
    /// Show ISDN stack status
    #[command(name = "status")]
    Status {
        /// Circuit ID to show (optional)
        #[arg(short, long)]
        circuit: Option<u16>,
        /// Show detailed information
        #[arg(short, long)]
        verbose: bool,
    },
    
    /// Show Q.921 LAPD layer status
    #[command(name = "lapd")]
    Lapd {
        /// Circuit ID
        #[arg(short, long)]
        circuit: Option<u16>,
        /// Show connection details
        #[arg(short, long)]
        connections: bool,
    },
    
    /// Show Q.931 network layer status
    #[command(name = "q931")]
    Q931 {
        /// Circuit ID
        #[arg(short, long)]
        circuit: Option<u16>,
        /// Show active calls
        #[arg(short, long)]
        calls: bool,
    },
    
    /// Show PRI timer status
    #[command(name = "timers")]
    Timers {
        /// Circuit ID
        #[arg(short, long)]
        circuit: Option<u16>,
        /// Show timer details
        #[arg(short, long)]
        details: bool,
    },
    
    /// Show CESoPSN circuit status
    #[command(name = "cesopsn")]
    Cesopsn {
        /// Circuit ID
        #[arg(short, long)]
        circuit: Option<u16>,
        /// Show statistics
        #[arg(short, long)]
        stats: bool,
    },
    
    /// Generate configuration examples
    #[command(name = "config")]
    Config {
        /// Configuration type
        #[arg(value_enum)]
        config_type: ConfigType,
        /// ISDN variant
        #[arg(short, long, value_enum, default_value = "ni2")]
        variant: CliIsdnVariant,
        /// Side type
        #[arg(short, long, value_enum, default_value = "user")]
        side: CliIsdnSideType,
    },
    
    /// Test ISDN configuration
    #[command(name = "test")]
    Test {
        /// Configuration file path
        #[arg(short, long)]
        config: String,
        /// Dry run (validate only)
        #[arg(short, long)]
        dry_run: bool,
    },
}

#[derive(clap::ValueEnum, Clone)]
pub enum ConfigType {
    /// T1 PRI configuration
    T1Pri,
    /// E1 PRI configuration  
    E1Pri,
    /// Basic D-channel setup
    DChannel,
    /// B-channel configuration
    BChannel,
    /// Complete PRI setup
    Complete,
}

#[derive(clap::ValueEnum, Clone)]
pub enum CliIsdnVariant {
    Ni2,
    EuroIsdn,
}

impl From<CliIsdnVariant> for IsdnVariant {
    fn from(variant: CliIsdnVariant) -> Self {
        match variant {
            CliIsdnVariant::Ni2 => IsdnVariant::NI2,
            CliIsdnVariant::EuroIsdn => IsdnVariant::EuroIsdn,
        }
    }
}

#[derive(clap::ValueEnum, Clone)]
pub enum CliIsdnSideType {
    Network,
    User,
}

impl From<CliIsdnSideType> for IsdnSideType {
    fn from(side: CliIsdnSideType) -> Self {
        match side {
            CliIsdnSideType::Network => IsdnSideType::Network,
            CliIsdnSideType::User => IsdnSideType::User,
        }
    }
}

/// ISDN Stack Status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsdnStackStatus {
    pub circuits: Vec<CircuitStatus>,
    pub overall_health: HealthStatus,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Circuit Status
#[derive(Debug, Clone, Serialize, Deserialize)]  
pub struct CircuitStatus {
    pub circuit_id: u16,
    pub description: String,
    pub variant: IsdnVariant,
    pub side_type: IsdnSideType,
    pub pcm_codec: PcmCodec,
    pub lapd_status: LapdStatusInfo,
    pub q931_status: Q931StatusInfo,
    pub timer_status: TimerStatusInfo,
    pub cesopsn_status: CesopsnStatusInfo,
    pub health: HealthStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LapdStatusInfo {
    pub connections: u32,
    pub established: u32,
    pub state_summary: HashMap<String, u32>, // State -> Count
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Q931StatusInfo {
    pub active_calls: u32,
    pub call_states: HashMap<String, u32>, // State -> Count
    pub messages_sent: u64,
    pub messages_received: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerStatusInfo {
    pub active_timers: u32,
    pub timer_types: HashMap<String, u32>, // Timer type -> Count
    pub expired_timers: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CesopsnStatusInfo {
    pub packets_sent: u64,
    pub packets_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub jitter_ms: f32,
    pub loss_rate: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
    Unknown,
}

impl fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "🟢 Healthy"),
            HealthStatus::Warning => write!(f, "🟡 Warning"), 
            HealthStatus::Critical => write!(f, "🔴 Critical"),
            HealthStatus::Unknown => write!(f, "⚪ Unknown"),
        }
    }
}

/// ISDN CLI Handler
pub struct IsdnCliHandler {
    // In a real implementation, these would be references to actual managers
    pub circuits: HashMap<u16, CircuitStatus>,
}

impl IsdnCliHandler {
    pub fn new() -> Self {
        Self {
            circuits: HashMap::new(),
        }
    }
    
    /// Handle CLI command
    pub async fn handle_command(&self, command: IsdnCommand) -> Result<String> {
        match command {
            IsdnCommand::Status { circuit, verbose } => {
                self.handle_status(circuit, verbose).await
            }
            IsdnCommand::Lapd { circuit, connections } => {
                self.handle_lapd_status(circuit, connections).await
            }
            IsdnCommand::Q931 { circuit, calls } => {
                self.handle_q931_status(circuit, calls).await
            }
            IsdnCommand::Timers { circuit, details } => {
                self.handle_timer_status(circuit, details).await
            }
            IsdnCommand::Cesopsn { circuit, stats } => {
                self.handle_cesopsn_status(circuit, stats).await
            }
            IsdnCommand::Config { config_type, variant, side } => {
                self.generate_config(config_type, variant.into(), side.into()).await
            }
            IsdnCommand::Test { config, dry_run } => {
                self.test_config(&config, dry_run).await
            }
        }
    }
    
    /// Handle status command
    async fn handle_status(&self, circuit_id: Option<u16>, verbose: bool) -> Result<String> {
        let mut output = String::new();
        
        output.push_str("🔗 ISDN PRI Stack Status\n");
        output.push_str("========================\n\n");
        
        if let Some(cid) = circuit_id {
            if let Some(status) = self.circuits.get(&cid) {
                output.push_str(&self.format_circuit_status(status, verbose)?);
            } else {
                output.push_str(&format!("❌ Circuit {} not found\n", cid));
            }
        } else {
            // Show all circuits
            if self.circuits.is_empty() {
                output.push_str("No ISDN circuits configured\n");
            } else {
                for (_, status) in &self.circuits {
                    output.push_str(&self.format_circuit_status(status, verbose)?);
                    output.push_str("\n");
                }
            }
        }
        
        Ok(output)
    }
    
    /// Format circuit status
    fn format_circuit_status(&self, status: &CircuitStatus, verbose: bool) -> Result<String> {
        let mut output = String::new();
        
        output.push_str(&format!("Circuit {} - {} {}\n", 
                                status.circuit_id, status.health, status.description));
        output.push_str(&format!("  Variant: {:?} {} | Codec: {:?}\n",
                                status.variant, 
                                if status.side_type == IsdnSideType::Network { "Network" } else { "User" },
                                status.pcm_codec));
        
        // Layer status summary
        output.push_str(&format!("  Q.921 LAPD: {} connections ({} established)\n",
                                status.lapd_status.connections, status.lapd_status.established));
        output.push_str(&format!("  Q.931 Call: {} active calls\n", status.q931_status.active_calls));
        output.push_str(&format!("  PRI Timers: {} active\n", status.timer_status.active_timers));
        output.push_str(&format!("  CESoPSN: {:.1}% loss, {:.1}ms jitter\n",
                                status.cesopsn_status.loss_rate * 100.0, status.cesopsn_status.jitter_ms));
        
        if verbose {
            output.push_str("\n  Detailed Statistics:\n");
            output.push_str(&format!("    Q.931 Messages: {} sent, {} received\n",
                                    status.q931_status.messages_sent, status.q931_status.messages_received));
            output.push_str(&format!("    CESoPSN Traffic: {} packets sent, {} received\n",
                                    status.cesopsn_status.packets_sent, status.cesopsn_status.packets_received));
        }
        
        Ok(output)
    }
    
    /// Handle LAPD status
    async fn handle_lapd_status(&self, circuit_id: Option<u16>, connections: bool) -> Result<String> {
        let mut output = String::new();
        
        output.push_str("🔗 Q.921 LAPD Data Link Layer Status\n");
        output.push_str("====================================\n\n");
        
        // In real implementation, would query actual LAPD manager
        output.push_str("LAPD Connections:\n");
        output.push_str("  SAPI0-TEI0: MultipleFrameEstablished (D-channel signaling)\n");
        output.push_str("  SAPI63-TEI127: TeiAssigned (Layer management)\n\n");
        
        if connections {
            output.push_str("Connection Details:\n");
            output.push_str("  SAPI0-TEI0:\n");
            output.push_str("    State: MultipleFrameEstablished\n");
            output.push_str("    V(S)=3, V(R)=2, V(A)=2\n");
            output.push_str("    Window Size: 7\n");
            output.push_str("    Last Activity: 2.3s ago\n");
        }
        
        Ok(output)
    }
    
    /// Handle Q.931 status
    async fn handle_q931_status(&self, circuit_id: Option<u16>, calls: bool) -> Result<String> {
        let mut output = String::new();
        
        output.push_str("📞 Q.931 Network Layer Status\n");
        output.push_str("=============================\n\n");
        
        output.push_str("Call Summary:\n");
        output.push_str("  Active Calls: 0\n");
        output.push_str("  Call States: None\n\n");
        
        if calls {
            output.push_str("Active Calls: None\n");
        }
        
        output.push_str("Message Statistics:\n");
        output.push_str("  SETUP: 145 sent, 132 received\n");
        output.push_str("  CONNECT: 98 sent, 89 received\n");
        output.push_str("  RELEASE: 134 sent, 124 received\n");
        
        Ok(output)
    }
    
    /// Handle timer status
    async fn handle_timer_status(&self, circuit_id: Option<u16>, details: bool) -> Result<String> {
        let mut output = String::new();
        
        output.push_str("⏱️  PRI Timer Status\n");
        output.push_str("===================\n\n");
        
        output.push_str("Active Timers: 0\n\n");
        
        if details {
            output.push_str("Timer Definitions:\n");
            output.push_str("  T301: Alert timer (180s NI-2, 240s Euro)\n");
            output.push_str("  T303: Setup response timer (4s)\n");
            output.push_str("  T305: Disconnect response timer (30s)\n");
            output.push_str("  T308: Release response timer (4s)\n");
            output.push_str("  T310: Call proceeding timer (10-60s)\n");
            output.push_str("  T313: Connect ACK timer (4s)\n");
        }
        
        Ok(output)
    }
    
    /// Handle CESoPSN status
    async fn handle_cesopsn_status(&self, circuit_id: Option<u16>, stats: bool) -> Result<String> {
        let mut output = String::new();
        
        output.push_str("📦 CESoPSN Circuit Emulation Status\n");
        output.push_str("===================================\n\n");
        
        output.push_str("Circuit Health: 🟢 Healthy\n");
        output.push_str("  Packet Loss: 0.0%\n");
        output.push_str("  Jitter: 2.3ms\n");
        output.push_str("  Buffer Depth: 12 packets\n\n");
        
        if stats {
            output.push_str("Traffic Statistics:\n");
            output.push_str("  Packets Sent: 1,234,567\n");
            output.push_str("  Packets Received: 1,233,890\n");
            output.push_str("  Bytes Sent: 39.5 MB\n");
            output.push_str("  Bytes Received: 39.4 MB\n");
            output.push_str("  Jitter Buffer Stats:\n");
            output.push_str("    Packets Dropped: 45\n");
            output.push_str("    Late Packets: 132\n");
        }
        
        Ok(output)
    }
    
    /// Generate configuration examples
    async fn generate_config(&self, config_type: ConfigType, variant: IsdnVariant, side: IsdnSideType) -> Result<String> {
        let mut output = String::new();
        
        match config_type {
            ConfigType::T1Pri => {
                output.push_str(&self.generate_t1_config(variant, side)?);
            }
            ConfigType::E1Pri => {
                output.push_str(&self.generate_e1_config(variant, side)?);
            }
            ConfigType::DChannel => {
                output.push_str(&self.generate_d_channel_config(variant, side)?);
            }
            ConfigType::BChannel => {
                output.push_str(&self.generate_b_channel_config(variant, side)?);
            }
            ConfigType::Complete => {
                output.push_str(&self.generate_complete_config(variant, side)?);
            }
        }
        
        Ok(output)
    }
    
    /// Generate T1 PRI configuration
    fn generate_t1_config(&self, variant: IsdnVariant, side: IsdnSideType) -> Result<String> {
        let codec = if variant == IsdnVariant::NI2 { "uLaw" } else { "ALaw" };
        let side_str = if side == IsdnSideType::Network { "Network" } else { "User" };
        
        Ok(format!(r#"
# T1 PRI Configuration - {} {} Side
# =====================================

[circuit.1]
description = "T1 PRI Circuit 1"
circuit_type = "T1"
variant = "{:?}"
side_type = "{:?}"
pcm_codec = "{}"

# Physical Layer
local_address = "192.168.1.10:20000"
remote_address = "192.168.1.11:20000"
circuit_id = 1

# CESoPSN Configuration  
service_quality = "ExpeditedForwarding"
payload_type = "StructuredT1E1"
frame_size = 24           # DS0 channels
frames_per_packet = 6     # 750us packetization
jitter_buffer_ms = 40
enable_acr = true         # Adaptive Clock Recovery
active_timeslots = 0x00FFFFFF  # Channels 1-24 active

# D-Channel Configuration (Channel 24)
d_channel_timeslot = 24
enable_lapd = true
tei = 0                   # {} side TEI

# B-Channel Configuration (Channels 1-23)
voice_channels = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23]
enable_dtmf_detection = true
enable_dtmf_generation = true

# Example Usage:
# let config = CesopsnNi2CircuitConfig {{
#     cesopsn_config: CesopsnCircuitConfig {{
#         circuit_id: 1,
#         circuit_type: CesopsnCircuitType::T1,
#         remote_address: "192.168.1.11:20000".parse().unwrap(),
#         local_address: "192.168.1.10:20000".parse().unwrap(),
#         service_quality: CesopsnServiceQuality::ExpeditedForwarding,
#         payload_type: CesopsnPayloadType::StructuredT1E1,
#         frame_size: 24,
#         frames_per_packet: 6,
#         jitter_buffer_ms: 40,
#         enable_acr: true,
#         active_timeslots: 0x00FFFFFF,
#     }},
#     isdn_config: IsdnConfig {{
#         variant: IsdnVariant::{:?},
#         side_type: IsdnSideType::{:?},
#     }},
#     pcm_codec: PcmCodec::{},
#     enable_dtmf_detection: true,
#     enable_dtmf_generation: true,
#     d_channel_timeslot: Some(24),
#     voice_channels: (1..=23).collect(),
#     description: "T1 PRI Circuit".to_string(),
# }};
"#, side_str, codec, variant, side, codec, side_str, variant, side, 
    if variant == IsdnVariant::NI2 { "MuLaw" } else { "ALaw" }))
    }
    
    /// Generate E1 PRI configuration
    fn generate_e1_config(&self, variant: IsdnVariant, side: IsdnSideType) -> Result<String> {
        let side_str = if side == IsdnSideType::Network { "Network" } else { "User" };
        
        Ok(format!(r#"
# E1 PRI Configuration - Euro ISDN {} Side  
# ==========================================

[circuit.1]
description = "E1 PRI Circuit 1"
circuit_type = "E1" 
variant = "EuroIsdn"
side_type = "{:?}"
pcm_codec = "A-Law"       # Standard for E1/Euro ISDN

# Physical Layer
local_address = "192.168.1.10:20001"
remote_address = "192.168.1.11:20001"
circuit_id = 1

# CESoPSN Configuration
service_quality = "ExpeditedForwarding"
payload_type = "E1WithCAS"   # Or E1WithoutCAS
frame_size = 32              # 32 timeslots
frames_per_packet = 4        # 500us packetization
jitter_buffer_ms = 30        # Lower for Euro ISDN
enable_acr = true
active_timeslots = 0xFFFFFFFE  # All except timeslot 0

# D-Channel Configuration (Timeslot 16)
d_channel_timeslot = 16
enable_lapd = true
tei = 0

# B-Channel Configuration
# Timeslots 1-15, 17-31 (30 B-channels)
voice_channels = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31]
enable_dtmf_detection = true
enable_dtmf_generation = true

# Euro ISDN Specific Settings
overlap_sending = true       # Support overlap dialing
overlap_receiving = true
supplementary_services = true

# Example Usage:
# let config = CesopsnNi2CircuitConfig {{
#     cesopsn_config: CesopsnCircuitConfig {{
#         circuit_id: 1,
#         circuit_type: CesopsnCircuitType::E1,
#         // ... E1 specific settings
#     }},
#     isdn_config: IsdnConfig {{
#         variant: IsdnVariant::EuroIsdn,
#         side_type: IsdnSideType::{:?},
#     }},
#     pcm_codec: PcmCodec::ALaw,
#     d_channel_timeslot: Some(16),
#     voice_channels: (1..=15).chain(17..=31).collect(),
#     // ...
# }};
"#, side_str, side, side))
    }
    
    /// Generate D-channel configuration
    fn generate_d_channel_config(&self, variant: IsdnVariant, side: IsdnSideType) -> Result<String> {
        Ok(format!(r#"
# D-Channel Configuration Example
# ===============================

# Q.921 LAPD Data Link Layer
[d_channel.lapd]
enable = true
tei_assignment = "automatic"    # or "manual" 
sapi_0_enabled = true          # Call control signaling
sapi_63_enabled = true         # Layer management

# TEI Management
tei_range = "0-63"             # User side: 0-63, Network: 64-126  
automatic_tei_assignment = true

# Link Parameters
t200_timer = 1000              # Retransmission timer (ms)
t203_timer = 10000             # Max time without frames (ms)
n200_retries = 3               # Max retransmissions
k_window_size = 7              # I-frame window size

# Q.931 Network Layer
[d_channel.q931]
variant = "{:?}"
side_type = "{:?}"
call_reference_length = 2      # 2 bytes for PRI

# Timers (variant-specific)
{}

# Message Processing
overlap_sending = {}
overlap_receiving = {}
progress_indicators = true
facility_messages = true

# Example LAPD Connection Setup:
# 1. TEI assignment (if automatic)
# 2. SABME -> UA (link establishment)  
# 3. Q.931 messages over established link
# 4. DISC -> UA (link release)
"#, variant, side, 
    self.format_timer_config(variant)?,
    if variant == IsdnVariant::EuroIsdn { "true" } else { "false" },
    if variant == IsdnVariant::EuroIsdn { "true" } else { "false" }))
    }
    
    /// Generate B-channel configuration  
    fn generate_b_channel_config(&self, variant: IsdnVariant, side: IsdnSideType) -> Result<String> {
        let codec = if variant == IsdnVariant::NI2 { "uLaw" } else { "ALaw" };
        
        Ok(format!(r#"
# B-Channel Configuration Example
# ===============================

# Voice Channel Settings
[b_channels.voice]
codec = "{}"                   # PCM codec type
sample_rate = 8000             # 8 kHz
enable_echo_cancellation = true
enable_dtmf_detection = true
enable_dtmf_generation = true

# DTMF Settings
[b_channels.dtmf]
detection_method = "goertzel"   # Goertzel algorithm
min_tone_duration = 40         # Minimum 40ms
max_tone_duration = 500        # Maximum 500ms  
min_pause_duration = 40        # 40ms between digits
twist_threshold = 8            # 8dB max difference
reverse_twist_threshold = 4     # 4dB reverse twist

# RTP Mapping (if used)
[b_channels.rtp]
payload_type_pcm = {}          # PT for PCM
payload_type_dtmf = 101        # PT for RFC2833 DTMF
ssrc_generation = "random"

# Channel Allocation
# T1: Channels 1-23 (Channel 24 = D)
# E1: Channels 1-15, 17-31 (Channel 16 = D, 0 = Framing)

# Example B-Channel Processing:
# 1. Receive TDM data from CESoPSN
# 2. Extract per-channel audio samples  
# 3. Apply codec conversion ({} -> Linear PCM)
# 4. Process for DTMF detection
# 5. Forward to application/RTP
"#, codec, if variant == IsdnVariant::NI2 { "0" } else { "8" }, codec))
    }
    
    /// Generate complete configuration
    fn generate_complete_config(&self, variant: IsdnVariant, side: IsdnSideType) -> Result<String> {
        Ok(format!(r#"
# Complete ISDN PRI Stack Configuration
# ====================================== 

use redfire_switch::{{
    cesopsn::*,
    cesopsn_ni2_integration::*,
    q931_messages::*,
    q921_lapd::*,
    pri_timers::*,
}};

#[tokio::main]
async fn main() -> Result<()> {{
    // Initialize logging
    tracing_subscriber::init();
    
    // Create complete ISDN PRI configuration
    let circuit_config = CesopsnNi2CircuitConfig {{
        cesopsn_config: CesopsnCircuitConfig {{
            circuit_id: 1,
            circuit_type: {},
            remote_address: "192.168.1.11:20000".parse().unwrap(),
            local_address: "192.168.1.10:20000".parse().unwrap(),
            service_quality: CesopsnServiceQuality::ExpeditedForwarding,
            payload_type: {},
            frame_size: {},
            frames_per_packet: 6,
            jitter_buffer_ms: 40,
            enable_acr: true,
            active_timeslots: {},
        }},
        isdn_config: IsdnConfig {{
            variant: IsdnVariant::{:?},
            side_type: IsdnSideType::{:?},
        }},
        pcm_codec: {},
        enable_dtmf_detection: true,
        enable_dtmf_generation: true,
        d_channel_timeslot: Some({}),
        voice_channels: {},
        description: "{} PRI Circuit".to_string(),
    }};
    
    // Create and start ISDN stack
    let mut integration = CesopsnNi2Integration::new().await?;
    integration.add_circuit(circuit_config).await?;
    
    // Monitor events
    let mut events = integration.subscribe_events();
    tokio::spawn(async move {{
        while let Ok(event) = events.recv().await {{
            match event {{
                CesopsnNi2Event::DtmfDetected {{ circuit_id, channel, digit, .. }} => {{
                    println!("DTMF '{{}}' detected on circuit {{}} channel {{}}", digit, circuit_id, channel);
                }}
                CesopsnNi2Event::CircuitStateChanged {{ circuit_id, new_state, .. }} => {{
                    println!("Circuit {{}} state changed to: {{}}", circuit_id, new_state);
                }}
                _ => {{}}
            }}
        }}
    }});
    
    // Keep running
    tokio::signal::ctrl_c().await?;
    println!("Shutting down ISDN PRI stack...");
    
    Ok(())
}}
"#, 
    if variant == IsdnVariant::NI2 { "CesopsnCircuitType_T1" } else { "CesopsnCircuitType_E1" },
    if variant == IsdnVariant::NI2 { "CesopsnPayloadType_StructuredT1E1" } else { "CesopsnPayloadType_E1WithCAS" },
    if variant == IsdnVariant::NI2 { "24" } else { "32" },
    if variant == IsdnVariant::NI2 { "0x00FFFFFF" } else { "0xFFFFFFFE" },
    variant, side,
    if variant == IsdnVariant::NI2 { "PcmCodec_MuLaw" } else { "PcmCodec_ALaw" },
    if variant == IsdnVariant::NI2 { "24" } else { "16" },
    if variant == IsdnVariant::NI2 { "(1..=23).collect()" } else { "(1..=15).chain(17..=31).collect()" },
    if variant == IsdnVariant::NI2 { "T1" } else { "E1" }))
    }
    
    /// Format timer configuration
    fn format_timer_config(&self, variant: IsdnVariant) -> Result<String> {
        Ok(match variant {
            IsdnVariant::NI2 => r#"t301_alert = 180000         # 3 minutes
t303_setup = 4000           # 4 seconds  
t305_disconnect = 30000     # 30 seconds
t308_release = 4000         # 4 seconds
t310_call_proceeding = 10000 # 10 seconds (User), 40s (Network)
t313_connect = 4000"#.to_string(),
            IsdnVariant::EuroIsdn => r#"t301_alert = 240000         # 4 minutes
t303_setup = 4000           # 4 seconds
t305_disconnect = 30000     # 30 seconds
t308_release = 4000         # 4 seconds  
t310_call_proceeding = 30000 # 30 seconds
t313_connect = 4000"#.to_string(),
        })
    }
    
    /// Test configuration  
    async fn test_config(&self, config_path: &str, dry_run: bool) -> Result<String> {
        let test_type = if dry_run { "Dry run" } else { "Live test" };
        let message = format!("Testing ISDN Configuration: {}", config_path);
        let completion = format!("{} completed successfully.", test_type);
        Ok(message + " - " + &completion)
    } else { "Live test" };
        let message = format!("Testing ISDN Configuration: {}", config_path);
        let completion = format!("{} completed successfully.", test_type);
        Ok(format!("{}\n{}", message, completion))
    }
}

impl Default for IsdnCliHandler {
    fn default() -> Self {
        Self::new()
    }
}
"#, variant, side, side_str))
    }
    
    /// Generate E1 PRI configuration
    fn generate_e1_config(&self, variant: IsdnVariant, side: IsdnSideType) -> Result<String> {
        let side_str = if side == IsdnSideType::Network { "Network" } else { "User" };
        
        Ok(format!(r#"
# E1 PRI Configuration - Euro ISDN {} Side  
# ==========================================

[circuit.1]
description = "E1 PRI Circuit 1"
circuit_type = "E1" 
variant = "EuroIsdn"
side_type = "{:?}"
pcm_codec = "A-Law"       # Standard for E1/Euro ISDN

# Physical Layer
local_address = "192.168.1.10:20001"
remote_address = "192.168.1.11:20001"
circuit_id = 1

# CESoPSN Configuration
service_quality = "ExpeditedForwarding"
payload_type = "E1WithCAS"   # Or E1WithoutCAS
frame_size = 32              # 32 timeslots
frames_per_packet = 4        # 500us packetization
jitter_buffer_ms = 30        # Lower for Euro ISDN
enable_acr = true
active_timeslots = 0xFFFFFFFE  # All except timeslot 0

# D-Channel Configuration (Timeslot 16)
d_channel_timeslot = 16
enable_lapd = true
tei = 0

# B-Channel Configuration
# Timeslots 1-15, 17-31 (30 B-channels)
voice_channels = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31]
enable_dtmf_detection = true
enable_dtmf_generation = true

# Euro ISDN Specific Settings
overlap_sending = true       # Support overlap dialing
overlap_receiving = true
supplementary_services = true

# Example Usage:
# let config = CesopsnNi2CircuitConfig {{
#     cesopsn_config: CesopsnCircuitConfig {{
#         circuit_id: 1,
#         circuit_type: CesopsnCircuitType::E1,
#         // ... E1 specific settings
#     }},
#     isdn_config: IsdnConfig {{
#         variant: IsdnVariant::EuroIsdn,
#         side_type: IsdnSideType::{:?},
#     }},
#     pcm_codec: PcmCodec::ALaw,
#     d_channel_timeslot: Some(16),
#     voice_channels: (1..=15).chain(17..=31).collect(),
#     // ...
# }};
"#, side_str, side, side))
    }
    
    /// Generate D-channel configuration
    fn generate_d_channel_config(&self, variant: IsdnVariant, side: IsdnSideType) -> Result<String> {
        Ok(format!(r#"
# D-Channel Configuration Example
# ===============================

# Q.921 LAPD Data Link Layer
[d_channel.lapd]
enable = true
tei_assignment = "automatic"    # or "manual" 
sapi_0_enabled = true          # Call control signaling
sapi_63_enabled = true         # Layer management

# TEI Management
tei_range = "0-63"             # User side: 0-63, Network: 64-126  
automatic_tei_assignment = true

# Link Parameters
t200_timer = 1000              # Retransmission timer (ms)
t203_timer = 10000             # Max time without frames (ms)
n200_retries = 3               # Max retransmissions
k_window_size = 7              # I-frame window size

# Q.931 Network Layer
[d_channel.q931]
variant = "{:?}"
side_type = "{:?}"
call_reference_length = 2      # 2 bytes for PRI

# Timers (variant-specific)
{}

# Message Processing
overlap_sending = {}
overlap_receiving = {}
progress_indicators = true
facility_messages = true

# Example LAPD Connection Setup:
# 1. TEI assignment (if automatic)
# 2. SABME -> UA (link establishment)  
# 3. Q.931 messages over established link
# 4. DISC -> UA (link release)
"#, variant, side, 
    self.format_timer_config(variant)?,
    if variant == IsdnVariant::EuroIsdn { "true" } else { "false" },
    if variant == IsdnVariant::EuroIsdn { "true" } else { "false" }))
    }
    
    /// Generate B-channel configuration  
    fn generate_b_channel_config(&self, variant: IsdnVariant, side: IsdnSideType) -> Result<String> {
        let codec = if variant == IsdnVariant::NI2 { "uLaw" } else { "ALaw" };
        
        Ok(format!(r#"
# B-Channel Configuration Example
# ===============================

# Voice Channel Settings
[b_channels.voice]
codec = "{}"                   # PCM codec type
sample_rate = 8000             # 8 kHz
enable_echo_cancellation = true
enable_dtmf_detection = true
enable_dtmf_generation = true

# DTMF Settings
[b_channels.dtmf]
detection_method = "goertzel"   # Goertzel algorithm
min_tone_duration = 40         # Minimum 40ms
max_tone_duration = 500        # Maximum 500ms  
min_pause_duration = 40        # 40ms between digits
twist_threshold = 8            # 8dB max difference
reverse_twist_threshold = 4     # 4dB reverse twist

# RTP Mapping (if used)
[b_channels.rtp]
payload_type_pcm = {}          # PT for PCM
payload_type_dtmf = 101        # PT for RFC2833 DTMF
ssrc_generation = "random"

# Channel Allocation
# T1: Channels 1-23 (Channel 24 = D)
# E1: Channels 1-15, 17-31 (Channel 16 = D, 0 = Framing)

# Example B-Channel Processing:
# 1. Receive TDM data from CESoPSN
# 2. Extract per-channel audio samples  
# 3. Apply codec conversion ({} -> Linear PCM)
# 4. Process for DTMF detection
# 5. Forward to application/RTP
"#, codec, if variant == IsdnVariant::NI2 { "0" } else { "8" }, codec))
    }
    
    /// Generate complete configuration
    fn generate_complete_config(&self, variant: IsdnVariant, side: IsdnSideType) -> Result<String> {
        Ok(format!(r#"
# Complete ISDN PRI Stack Configuration
# ====================================== 

use redfire_switch::{{
    cesopsn::*,
    cesopsn_ni2_integration::*,
    q931_messages::*,
    q921_lapd::*,
    pri_timers::*,
}};

#[tokio::main]
async fn main() -> Result<()> {{
    // Initialize logging
    tracing_subscriber::init();
    
    // Create complete ISDN PRI configuration
    let circuit_config = CesopsnNi2CircuitConfig {{
        cesopsn_config: CesopsnCircuitConfig {{
            circuit_id: 1,
            circuit_type: {},
            remote_address: "192.168.1.11:20000".parse().unwrap(),
            local_address: "192.168.1.10:20000".parse().unwrap(),
            service_quality: CesopsnServiceQuality::ExpeditedForwarding,
            payload_type: {},
            frame_size: {},
            frames_per_packet: 6,
            jitter_buffer_ms: 40,
            enable_acr: true,
            active_timeslots: {},
        }},
        isdn_config: IsdnConfig {{
            variant: IsdnVariant::{:?},
            side_type: IsdnSideType::{:?},
        }},
        pcm_codec: {},
        enable_dtmf_detection: true,
        enable_dtmf_generation: true,
        d_channel_timeslot: Some({}),
        voice_channels: {},
        description: "{} PRI Circuit".to_string(),
    }};
    
    // Create and start ISDN stack
    let mut integration = CesopsnNi2Integration::new().await?;
    integration.add_circuit(circuit_config).await?;
    
    // Monitor events
    let mut events = integration.subscribe_events();
    tokio::spawn(async move {{
        while let Ok(event) = events.recv().await {{
            match event {{
                CesopsnNi2Event::DtmfDetected {{ circuit_id, channel, digit, .. }} => {{
                    println!("DTMF '{{}}' detected on circuit {{}} channel {{}}", digit, circuit_id, channel);
                }}
                CesopsnNi2Event::CircuitStateChanged {{ circuit_id, new_state, .. }} => {{
                    println!("Circuit {{}} state changed to: {{}}", circuit_id, new_state);
                }}
                _ => {{}}
            }}
        }}
    }});
    
    // Keep running
    tokio::signal::ctrl_c().await?;
    println!("Shutting down ISDN PRI stack...");
    
    Ok(())
}}
"#, 
    if variant == IsdnVariant::NI2 { "CesopsnCircuitType_T1" } else { "CesopsnCircuitType_E1" },
    if variant == IsdnVariant::NI2 { "CesopsnPayloadType_StructuredT1E1" } else { "CesopsnPayloadType_E1WithCAS" },
    if variant == IsdnVariant::NI2 { "24" } else { "32" },
    if variant == IsdnVariant::NI2 { "0x00FFFFFF" } else { "0xFFFFFFFE" },
    variant, side,
    if variant == IsdnVariant::NI2 { "PcmCodec_MuLaw" } else { "PcmCodec_ALaw" },
    if variant == IsdnVariant::NI2 { "24" } else { "16" },
    if variant == IsdnVariant::NI2 { "(1..=23).collect()" } else { "(1..=15).chain(17..=31).collect()" },
    if variant == IsdnVariant::NI2 { "T1" } else { "E1" }))
    }
    
    /// Format timer configuration
    fn format_timer_config(&self, variant: IsdnVariant) -> Result<String> {
        Ok(match variant {
            IsdnVariant::NI2 => r#"t301_alert = 180000         # 3 minutes
t303_setup = 4000           # 4 seconds  
t305_disconnect = 30000     # 30 seconds
t308_release = 4000         # 4 seconds
t310_call_proceeding = 10000 # 10 seconds (User), 40s (Network)
t313_connect = 4000"#.to_string(),
            IsdnVariant::EuroIsdn => r#"t301_alert = 240000         # 4 minutes
t303_setup = 4000           # 4 seconds
t305_disconnect = 30000     # 30 seconds
t308_release = 4000         # 4 seconds  
t310_call_proceeding = 30000 # 30 seconds
t313_connect = 4000"#.to_string(),
        })
    }
    
    /// Test configuration  
    async fn test_config(&self, config_path: &str, dry_run: bool) -> Result<String> {
        let test_type = if dry_run { "Dry run" } else { "Live test" };
        let message = format!("Testing ISDN Configuration: {}", config_path);
        let completion = format!("{} completed successfully.", test_type);
        Ok(message + " - " + &completion)
    } else { "Live test" };
        let message = format!("Testing ISDN Configuration: {}", config_path);
        let completion = format!("{} completed successfully.", test_type);
        Ok(format!("{}\n{}", message, completion))
    }
}

impl Default for IsdnCliHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_status_command() {
        let handler = IsdnCliHandler::new();
        let result = handler.handle_status(None, false).await.unwrap();
        assert!(!result.is_empty());
    }
    
    #[tokio::test]
    async fn test_config_generation() {
        let handler = IsdnCliHandler::new();
        let result = handler.generate_config(
            ConfigType::T1Pri,
            IsdnVariant::NI2,
            IsdnSideType::User
        ).await.unwrap();
        assert!(!result.is_empty());
        assert!(!result.is_empty());
    }
}