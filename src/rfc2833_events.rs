/*
 * RFC 2833 - RTP Payload for DTMF Digits, Telephony Tones and Telephony Signals
 *
 * This module implements RFC 2833 RTP event support for:
 * - DTMF digit transport over RTP
 * - Telephony tone events
 * - Telephony signaling events
 * - Bidirectional event negotiation and transport
 */

use anyhow::{anyhow, Result};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

use crate::dtmf_processor::{DtmfEvent, DtmfSource};

/// RFC 2833 RTP event payload types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rfc2833PayloadType {
    /// Dynamic payload type for telephone events (typically 96-127)
    TelephoneEvent(u8),
    /// Dynamic payload type for tones (typically 96-127)
    Tone(u8),
}

impl Default for Rfc2833PayloadType {
    fn default() -> Self {
        Self::TelephoneEvent(101) // Common default value
    }
}

/// RFC 2833 Event IDs for DTMF and telephony events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rfc2833EventId {
    // DTMF Events (0-15)
    Dtmf0 = 0,
    Dtmf1 = 1,
    Dtmf2 = 2,
    Dtmf3 = 3,
    Dtmf4 = 4,
    Dtmf5 = 5,
    Dtmf6 = 6,
    Dtmf7 = 7,
    Dtmf8 = 8,
    Dtmf9 = 9,
    DtmfStar = 10, // *
    DtmfHash = 11, // #
    DtmfA = 12,
    DtmfB = 13,
    DtmfC = 14,
    DtmfD = 15,

    // Telephony Events (16-31)
    Flash = 16, // Hook flash
    Reserved17 = 17,
    Reserved18 = 18,
    Reserved19 = 19,
    Reserved20 = 20,
    Reserved21 = 21,
    Reserved22 = 22,
    Reserved23 = 23,
    Reserved24 = 24,
    Reserved25 = 25,
    Reserved26 = 26,
    Reserved27 = 27,
    Reserved28 = 28,
    Reserved29 = 29,
    Reserved30 = 30,
    Reserved31 = 31,

    // Tone Events (32-63)
    DialTone = 32,        // Dial tone
    RingbackTone = 33,    // Ringback tone
    BusyTone = 34,        // Busy tone
    CongestionTone = 35,  // Congestion tone
    SpecialInfoTone = 36, // Special information tone
    WarningTone = 37,     // Warning tone
    IntrusivenessLevel0 = 38,
    IntrusivenessLevel1 = 39,
    IntrusivenessLevel2 = 40,
    IntrusivenessLevel3 = 41,
    Reserved42 = 42,
    Reserved43 = 43,
    Reserved44 = 44,
    Reserved45 = 45,
    Reserved46 = 46,
    Reserved47 = 47,

    // Country-specific tones (48-63)
    CountryTone48 = 48,
    CountryTone49 = 49,
    CountryTone50 = 50,
    CountryTone51 = 51,
    CountryTone52 = 52,
    CountryTone53 = 53,
    CountryTone54 = 54,
    CountryTone55 = 55,
    CountryTone56 = 56,
    CountryTone57 = 57,
    CountryTone58 = 58,
    CountryTone59 = 59,
    CountryTone60 = 60,
    CountryTone61 = 61,
    CountryTone62 = 62,
    CountryTone63 = 63,
}

impl Rfc2833EventId {
    /// Convert DTMF character to RFC 2833 event ID
    pub fn from_dtmf_char(c: char) -> Option<Self> {
        match c {
            '0' => Some(Self::Dtmf0),
            '1' => Some(Self::Dtmf1),
            '2' => Some(Self::Dtmf2),
            '3' => Some(Self::Dtmf3),
            '4' => Some(Self::Dtmf4),
            '5' => Some(Self::Dtmf5),
            '6' => Some(Self::Dtmf6),
            '7' => Some(Self::Dtmf7),
            '8' => Some(Self::Dtmf8),
            '9' => Some(Self::Dtmf9),
            '*' => Some(Self::DtmfStar),
            '#' => Some(Self::DtmfHash),
            'A' | 'a' => Some(Self::DtmfA),
            'B' | 'b' => Some(Self::DtmfB),
            'C' | 'c' => Some(Self::DtmfC),
            'D' | 'd' => Some(Self::DtmfD),
            _ => None,
        }
    }

    /// Convert RFC 2833 event ID to DTMF character
    pub fn to_dtmf_char(self) -> Option<char> {
        match self {
            Self::Dtmf0 => Some('0'),
            Self::Dtmf1 => Some('1'),
            Self::Dtmf2 => Some('2'),
            Self::Dtmf3 => Some('3'),
            Self::Dtmf4 => Some('4'),
            Self::Dtmf5 => Some('5'),
            Self::Dtmf6 => Some('6'),
            Self::Dtmf7 => Some('7'),
            Self::Dtmf8 => Some('8'),
            Self::Dtmf9 => Some('9'),
            Self::DtmfStar => Some('*'),
            Self::DtmfHash => Some('#'),
            Self::DtmfA => Some('A'),
            Self::DtmfB => Some('B'),
            Self::DtmfC => Some('C'),
            Self::DtmfD => Some('D'),
            _ => None,
        }
    }

    /// Check if event ID is a DTMF event
    pub fn is_dtmf(self) -> bool {
        matches!(self as u8, 0..=15)
    }

    /// Check if event ID is a telephony event
    pub fn is_telephony(self) -> bool {
        matches!(self as u8, 16..=31)
    }

    /// Check if event ID is a tone event
    pub fn is_tone(self) -> bool {
        matches!(self as u8, 32..=63)
    }
}

/// RFC 2833 RTP Event Packet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rfc2833Event {
    /// Event identifier
    pub event_id: Rfc2833EventId,
    /// End of event flag
    pub end_of_event: bool,
    /// Reserved flag (must be 0)
    pub reserved: bool,
    /// Volume level (0 = loudest, 63 = softest, values above 55 should be avoided)
    pub volume: u8,
    /// Event duration in RTP timestamp units
    pub duration: u16,
}

impl Rfc2833Event {
    /// Create new RFC 2833 event
    pub fn new(event_id: Rfc2833EventId, volume: u8, duration: u16) -> Self {
        Self {
            event_id,
            end_of_event: false,
            reserved: false,
            volume: volume.min(63),
            duration,
        }
    }

    /// Create end-of-event marker
    pub fn end_event(event_id: Rfc2833EventId, volume: u8, duration: u16) -> Self {
        Self {
            event_id,
            end_of_event: true,
            reserved: false,
            volume: volume.min(63),
            duration,
        }
    }

    /// Serialize to RFC 2833 packet format
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::with_capacity(4);

        // Byte 0: Event ID
        bytes.push(self.event_id as u8);

        // Byte 1: E|R|Volume
        let mut flags_volume = self.volume & 0x3F; // Volume is 6 bits
        if self.end_of_event {
            flags_volume |= 0x80; // Set E bit
        }
        if self.reserved {
            flags_volume |= 0x40; // Set R bit (should be 0)
        }
        bytes.push(flags_volume);

        // Bytes 2-3: Duration (big-endian)
        bytes.write_u16::<BigEndian>(self.duration)?;

        Ok(bytes)
    }

    /// Deserialize from RFC 2833 packet format
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 4 {
            return Err(anyhow!("RFC 2833 packet too short: {} bytes", data.len()));
        }

        let event_id = match data[0] {
            0 => Rfc2833EventId::Dtmf0,
            1 => Rfc2833EventId::Dtmf1,
            2 => Rfc2833EventId::Dtmf2,
            3 => Rfc2833EventId::Dtmf3,
            4 => Rfc2833EventId::Dtmf4,
            5 => Rfc2833EventId::Dtmf5,
            6 => Rfc2833EventId::Dtmf6,
            7 => Rfc2833EventId::Dtmf7,
            8 => Rfc2833EventId::Dtmf8,
            9 => Rfc2833EventId::Dtmf9,
            10 => Rfc2833EventId::DtmfStar,
            11 => Rfc2833EventId::DtmfHash,
            12 => Rfc2833EventId::DtmfA,
            13 => Rfc2833EventId::DtmfB,
            14 => Rfc2833EventId::DtmfC,
            15 => Rfc2833EventId::DtmfD,
            16 => Rfc2833EventId::Flash,
            32 => Rfc2833EventId::DialTone,
            33 => Rfc2833EventId::RingbackTone,
            34 => Rfc2833EventId::BusyTone,
            35 => Rfc2833EventId::CongestionTone,
            36 => Rfc2833EventId::SpecialInfoTone,
            37 => Rfc2833EventId::WarningTone,
            id => return Err(anyhow!("Unknown RFC 2833 event ID: {}", id)),
        };

        let flags_volume = data[1];
        let end_of_event = (flags_volume & 0x80) != 0;
        let reserved = (flags_volume & 0x40) != 0;
        let volume = flags_volume & 0x3F;

        let mut cursor = Cursor::new(&data[2..4]);
        let duration = cursor.read_u16::<BigEndian>()?;

        Ok(Self {
            event_id,
            end_of_event,
            reserved,
            volume,
            duration,
        })
    }
}

/// RFC 2833 RTP Event Processor
pub struct Rfc2833Processor {
    /// Payload type mapping
    payload_types: HashMap<u8, Rfc2833PayloadType>,
    /// Active events by session
    active_events: Arc<RwLock<HashMap<String, Rfc2833ActiveEvent>>>,
    /// Event sender for integration with DTMF processor
    event_sender: mpsc::UnboundedSender<DtmfEvent>,
    /// RTP timestamp clock rate (usually 8000 Hz)
    clock_rate: u32,
}

/// Active RFC 2833 event state
#[derive(Debug, Clone)]
struct Rfc2833ActiveEvent {
    event: Rfc2833Event,
    start_timestamp: u32,
    last_timestamp: u32,
    start_time: Instant,
    packet_count: u32,
}

impl Rfc2833Processor {
    /// Create new RFC 2833 processor
    pub fn new(event_sender: mpsc::UnboundedSender<DtmfEvent>) -> Self {
        let mut payload_types = HashMap::new();
        payload_types.insert(101, Rfc2833PayloadType::TelephoneEvent(101)); // Default

        Self {
            payload_types,
            active_events: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
            clock_rate: 8000,
        }
    }

    /// Add payload type mapping
    pub fn add_payload_type(&mut self, pt: u8, event_type: Rfc2833PayloadType) {
        self.payload_types.insert(pt, event_type);
        debug!(
            "Added RFC 2833 payload type mapping: {} -> {:?}",
            pt, event_type
        );
    }

    /// Remove payload type mapping
    pub fn remove_payload_type(&mut self, pt: u8) {
        self.payload_types.remove(&pt);
        debug!("Removed RFC 2833 payload type mapping: {}", pt);
    }

    /// Process incoming RFC 2833 RTP packet
    pub async fn process_incoming_packet(
        &self,
        session_id: &str,
        payload_type: u8,
        timestamp: u32,
        payload: &[u8],
    ) -> Result<()> {
        // Check if this is a known RFC 2833 payload type
        if !self.payload_types.contains_key(&payload_type) {
            return Ok(()); // Not an RFC 2833 packet
        }

        // Parse RFC 2833 event
        let event = Rfc2833Event::from_bytes(payload)?;
        debug!(
            "Received RFC 2833 event: {:?} for session {}",
            event, session_id
        );

        // Handle event
        self.handle_incoming_event(session_id, timestamp, event)
            .await?;

        Ok(())
    }

    /// Handle incoming RFC 2833 event
    async fn handle_incoming_event(
        &self,
        session_id: &str,
        timestamp: u32,
        event: Rfc2833Event,
    ) -> Result<()> {
        let mut active_events = self.active_events.write().await;
        let now = Instant::now();

        if event.end_of_event {
            // End of event
            if let Some(active_event) = active_events.remove(session_id) {
                let total_duration =
                    Duration::from_millis((event.duration as u64 * 1000) / self.clock_rate as u64);

                // Convert to DTMF event if applicable
                if let Some(digit) = event.event_id.to_dtmf_char() {
                    let dtmf_event = DtmfEvent::DigitDetected {
                        digit,
                        duration: total_duration,
                        timestamp: active_event.start_time,
                        confidence: self.volume_to_confidence(event.volume),
                        source: DtmfSource::Rfc2833,
                    };

                    if let Err(e) = self.event_sender.send(dtmf_event) {
                        warn!("Failed to send DTMF event from RFC 2833: {}", e);
                    }

                    info!(
                        "RFC 2833 DTMF '{}' completed for session {} (duration: {:?})",
                        digit, session_id, total_duration
                    );
                }
            }
        } else {
            // Start or continuation of event
            match active_events.get_mut(session_id) {
                Some(active_event) => {
                    // Update existing event
                    active_event.last_timestamp = timestamp;
                    active_event.packet_count += 1;
                    active_event.event.duration = event.duration;
                }
                None => {
                    // New event
                    let active_event = Rfc2833ActiveEvent {
                        event: event.clone(),
                        start_timestamp: timestamp,
                        last_timestamp: timestamp,
                        start_time: now,
                        packet_count: 1,
                    };

                    active_events.insert(session_id.to_string(), active_event);

                    // Log start of event
                    if let Some(digit) = event.event_id.to_dtmf_char() {
                        debug!(
                            "RFC 2833 DTMF '{}' started for session {}",
                            digit, session_id
                        );
                    } else {
                        debug!(
                            "RFC 2833 event {:?} started for session {}",
                            event.event_id, session_id
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Generate RFC 2833 event packets for outgoing DTMF
    pub async fn generate_outgoing_packets(
        &self,
        session_id: &str,
        digit: char,
        duration_ms: u32,
        volume: u8,
        start_timestamp: u32,
    ) -> Result<Vec<(u32, Vec<u8>)>> {
        let event_id = Rfc2833EventId::from_dtmf_char(digit)
            .ok_or_else(|| anyhow!("Invalid DTMF digit for RFC 2833: {}", digit))?;

        let mut packets = Vec::new();

        // Calculate timing parameters
        let duration_samples = (duration_ms as u64 * self.clock_rate as u64) / 1000;
        let packet_interval = self.clock_rate / 50; // 20ms packets at 8kHz = 160 samples
        let total_packets = (duration_samples / packet_interval as u64).max(1) as u32;

        // Generate event packets
        for i in 0..total_packets {
            let timestamp = start_timestamp + (i * packet_interval);
            let current_duration = ((i + 1) * packet_interval).min(duration_samples as u32);

            let event = if i == total_packets - 1 {
                // Last packet - mark as end of event
                Rfc2833Event::end_event(event_id, volume, current_duration as u16)
            } else {
                // Regular event packet
                Rfc2833Event::new(event_id, volume, current_duration as u16)
            };

            let packet_data = event.to_bytes()?;
            packets.push((timestamp, packet_data));
        }

        // Send end-of-event packets (RFC recommends sending 3 copies)
        if total_packets > 0 {
            let final_timestamp = start_timestamp + duration_samples as u32;
            let end_event = Rfc2833Event::end_event(event_id, volume, duration_samples as u16);
            let end_packet_data = end_event.to_bytes()?;

            for _ in 0..3 {
                packets.push((final_timestamp, end_packet_data.clone()));
            }
        }

        info!(
            "Generated {} RFC 2833 packets for DTMF '{}' (session: {}, duration: {}ms)",
            packets.len(),
            digit,
            session_id,
            duration_ms
        );

        Ok(packets)
    }

    /// Convert volume level to confidence score
    fn volume_to_confidence(&self, volume: u8) -> f32 {
        // RFC 2833 volume: 0 = loudest, 63 = softest
        // Convert to confidence: higher volume = higher confidence
        let normalized_volume = (63 - volume.min(63)) as f32 / 63.0;
        0.5 + (normalized_volume * 0.5) // Range: 0.5 to 1.0
    }

    /// Get active events statistics
    pub async fn get_active_events(&self) -> HashMap<String, Rfc2833EventStats> {
        let active_events = self.active_events.read().await;
        let mut stats = HashMap::new();

        for (session_id, active_event) in active_events.iter() {
            let event_stats = Rfc2833EventStats {
                session_id: session_id.clone(),
                event_id: active_event.event.event_id,
                start_time: active_event.start_time,
                duration: Duration::from_millis(
                    (active_event.event.duration as u64 * 1000) / self.clock_rate as u64,
                ),
                packet_count: active_event.packet_count,
                volume: active_event.event.volume,
            };
            stats.insert(session_id.clone(), event_stats);
        }

        stats
    }

    /// Clean up stale events (events that haven't been updated recently)
    pub async fn cleanup_stale_events(&self, max_age: Duration) {
        let mut active_events = self.active_events.write().await;
        let now = Instant::now();
        let mut to_remove = Vec::new();

        for (session_id, active_event) in active_events.iter() {
            if now.duration_since(active_event.start_time) > max_age {
                to_remove.push(session_id.clone());
            }
        }

        for session_id in to_remove {
            active_events.remove(&session_id);
            warn!("Removed stale RFC 2833 event for session: {}", session_id);
        }
    }
}

/// RFC 2833 event statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rfc2833EventStats {
    pub session_id: String,
    pub event_id: Rfc2833EventId,
    #[serde(skip, default = "std::time::Instant::now")]
    pub start_time: Instant,
    pub duration: Duration,
    pub packet_count: u32,
    pub volume: u8,
}

/// SDP negotiation helper for RFC 2833
pub struct Rfc2833SdpNegotiator {
    /// Supported payload types
    supported_payload_types: Vec<u8>,
    /// Preferred payload type
    preferred_payload_type: u8,
}

impl Rfc2833SdpNegotiator {
    /// Create new SDP negotiator
    pub fn new() -> Self {
        Self {
            supported_payload_types: vec![96, 97, 98, 99, 100, 101, 102, 103], // Common dynamic range
            preferred_payload_type: 101, // Most common default
        }
    }

    /// Generate SDP attribute for RFC 2833 support
    pub fn generate_sdp_attributes(&self) -> Vec<String> {
        let mut attributes = Vec::new();

        // Add rtpmap attribute
        attributes.push(format!(
            "a=rtpmap:{} telephone-event/8000",
            self.preferred_payload_type
        ));

        // Add fmtp attribute for supported events
        attributes.push(format!("a=fmtp:{} 0-15", self.preferred_payload_type)); // DTMF events

        attributes
    }

    /// Parse SDP attributes to extract RFC 2833 configuration
    pub fn parse_sdp_attributes(&self, sdp_lines: &[&str]) -> Result<Rfc2833SdpConfig> {
        let mut config = Rfc2833SdpConfig::default();

        for line in sdp_lines {
            if let Some(rtpmap) = line.strip_prefix("a=rtpmap:") {
                if rtpmap.contains("telephone-event") {
                    let parts: Vec<&str> = rtpmap.split_whitespace().collect();
                    if let Ok(pt) = parts[0].parse::<u8>() {
                        config.payload_type = pt;

                        // Parse clock rate if specified
                        if let Some(format_part) = parts.get(1) {
                            if let Some(rate_part) = format_part.split('/').nth(1) {
                                if let Ok(rate) = rate_part.parse::<u32>() {
                                    config.clock_rate = rate;
                                }
                            }
                        }
                    }
                }
            } else if let Some(fmtp) = line.strip_prefix("a=fmtp:") {
                let parts: Vec<&str> = fmtp.splitn(2, ' ').collect();
                if parts.len() == 2 {
                    if let Ok(pt) = parts[0].parse::<u8>() {
                        if pt == config.payload_type {
                            config.supported_events = Self::parse_event_list(parts[1])?;
                        }
                    }
                }
            }
        }

        Ok(config)
    }

    /// Parse event list from fmtp attribute
    fn parse_event_list(event_str: &str) -> Result<Vec<u8>> {
        let mut events = Vec::new();

        for part in event_str.split(',') {
            let part = part.trim();
            if let Some(dash_pos) = part.find('-') {
                // Range of events (e.g., "0-15")
                let start = part[..dash_pos].parse::<u8>()?;
                let end = part[dash_pos + 1..].parse::<u8>()?;
                for event_id in start..=end {
                    events.push(event_id);
                }
            } else {
                // Single event
                events.push(part.parse::<u8>()?);
            }
        }

        Ok(events)
    }
}

impl Default for Rfc2833SdpNegotiator {
    fn default() -> Self {
        Self::new()
    }
}

/// RFC 2833 SDP configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rfc2833SdpConfig {
    pub payload_type: u8,
    pub clock_rate: u32,
    pub supported_events: Vec<u8>,
}

impl Default for Rfc2833SdpConfig {
    fn default() -> Self {
        Self {
            payload_type: 101,
            clock_rate: 8000,
            supported_events: (0..=15).collect(), // DTMF events
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[test]
    fn test_rfc2833_event_serialization() {
        let event = Rfc2833Event::new(Rfc2833EventId::Dtmf5, 10, 1600);
        let bytes = event.to_bytes().unwrap();

        assert_eq!(bytes.len(), 4);
        assert_eq!(bytes[0], 5); // Event ID for '5'
        assert_eq!(bytes[1], 10); // Volume
        assert_eq!((bytes[2] as u16) << 8 | bytes[3] as u16, 1600); // Duration

        // Test round-trip
        let decoded = Rfc2833Event::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.event_id as u8, event.event_id as u8);
        assert_eq!(decoded.volume, event.volume);
        assert_eq!(decoded.duration, event.duration);
        assert_eq!(decoded.end_of_event, event.end_of_event);
    }

    #[test]
    fn test_dtmf_char_conversion() {
        assert_eq!(
            Rfc2833EventId::from_dtmf_char('5'),
            Some(Rfc2833EventId::Dtmf5)
        );
        assert_eq!(
            Rfc2833EventId::from_dtmf_char('*'),
            Some(Rfc2833EventId::DtmfStar)
        );
        assert_eq!(
            Rfc2833EventId::from_dtmf_char('A'),
            Some(Rfc2833EventId::DtmfA)
        );
        assert_eq!(Rfc2833EventId::from_dtmf_char('X'), None);

        assert_eq!(Rfc2833EventId::Dtmf5.to_dtmf_char(), Some('5'));
        assert_eq!(Rfc2833EventId::DtmfStar.to_dtmf_char(), Some('*'));
        assert_eq!(Rfc2833EventId::DtmfA.to_dtmf_char(), Some('A'));
        assert_eq!(Rfc2833EventId::Flash.to_dtmf_char(), None);
    }

    #[tokio::test]
    async fn test_rfc2833_processor() {
        let (event_sender, mut event_receiver) = mpsc::unbounded_channel();
        let processor = Rfc2833Processor::new(event_sender);

        // Create test event packet
        let event = Rfc2833Event::new(Rfc2833EventId::Dtmf5, 10, 800);
        let packet_data = event.to_bytes().unwrap();

        // Process start event
        processor
            .process_incoming_packet("test_session", 101, 1000, &packet_data)
            .await
            .unwrap();

        // Process end event
        let end_event = Rfc2833Event::end_event(Rfc2833EventId::Dtmf5, 10, 1600);
        let end_packet_data = end_event.to_bytes().unwrap();
        processor
            .process_incoming_packet("test_session", 101, 1000, &end_packet_data)
            .await
            .unwrap();

        // Should receive DTMF event
        let received_event = event_receiver.try_recv().unwrap();
        match received_event {
            DtmfEvent::DigitDetected { digit, source, .. } => {
                assert_eq!(digit, '5');
                assert_eq!(source, DtmfSource::Rfc2833);
            }
            _ => assert!(false, "Expected DigitDetected event, got: {:?}", event),
        }
    }

    #[test]
    fn test_sdp_negotiation() {
        let negotiator = Rfc2833SdpNegotiator::new();

        // Test SDP generation
        let attributes = negotiator.generate_sdp_attributes();
        assert!(attributes
            .iter()
            .any(|attr| attr.contains("telephone-event/8000")));
        assert!(attributes.iter().any(|attr| attr.contains("0-15")));

        // Test SDP parsing
        let sdp_lines = vec!["a=rtpmap:101 telephone-event/8000", "a=fmtp:101 0-15"];
        let config = negotiator.parse_sdp_attributes(&sdp_lines).unwrap();
        assert_eq!(config.payload_type, 101);
        assert_eq!(config.clock_rate, 8000);
        assert_eq!(config.supported_events, (0..=15).collect::<Vec<u8>>());
    }
}
