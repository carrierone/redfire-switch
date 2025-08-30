/*
 * RTP (Real-time Transport Protocol) Implementation
 * RFC 3550 compliant RTP packet handling for media transport
 */

use anyhow::{anyhow, Result};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::warn;

/// RTP packet structure (RFC 3550)
#[derive(Debug, Clone, PartialEq)]
pub struct RtpPacket {
    /// Version (always 2)
    pub version: u8,
    /// Padding flag
    pub padding: bool,
    /// Extension flag
    pub extension: bool,
    /// CSRC count
    pub csrc_count: u8,
    /// Marker bit
    pub marker: bool,
    /// Payload type
    pub payload_type: u8,
    /// Sequence number
    pub sequence_number: u16,
    /// Timestamp
    pub timestamp: u32,
    /// Synchronization source identifier
    pub ssrc: u32,
    /// Contributing source identifiers
    pub csrc: Vec<u32>,
    /// Extension header (if present)
    pub extension_header: Option<RtpExtension>,
    /// Payload data
    pub payload: Vec<u8>,
    /// Padding bytes (if padding flag is set)
    pub padding_bytes: u8,
}

/// RTP extension header
#[derive(Debug, Clone, PartialEq)]
pub struct RtpExtension {
    pub profile: u16,
    pub length: u16,
    pub data: Vec<u8>,
}

/// RTCP packet types
#[derive(Debug, Clone, PartialEq)]
pub enum RtcpPacket {
    SenderReport(SenderReport),
    ReceiverReport(ReceiverReport),
    SourceDescription(SourceDescription),
    Bye(ByePacket),
    AppDefined(AppDefinedPacket),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SenderReport {
    pub ssrc: u32,
    pub ntp_timestamp: u64,
    pub rtp_timestamp: u32,
    pub packet_count: u32,
    pub octet_count: u32,
    pub reception_reports: Vec<ReceptionReport>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReceiverReport {
    pub ssrc: u32,
    pub reception_reports: Vec<ReceptionReport>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReceptionReport {
    pub ssrc: u32,
    pub fraction_lost: u8,
    pub cumulative_lost: u32,
    pub highest_sequence: u32,
    pub jitter: u32,
    pub last_sr: u32,
    pub delay_since_last_sr: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceDescription {
    pub chunks: Vec<SdesChunk>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SdesChunk {
    pub ssrc: u32,
    pub items: Vec<SdesItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SdesItem {
    pub item_type: u8,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ByePacket {
    pub sources: Vec<u32>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppDefinedPacket {
    pub subtype: u8,
    pub ssrc: u32,
    pub name: [u8; 4],
    pub data: Vec<u8>,
}

impl RtpPacket {
    /// Create a new RTP packet
    pub fn new(
        payload_type: u8,
        sequence_number: u16,
        timestamp: u32,
        ssrc: u32,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            version: 2,
            padding: false,
            extension: false,
            csrc_count: 0,
            marker: false,
            payload_type,
            sequence_number,
            timestamp,
            ssrc,
            csrc: Vec::new(),
            extension_header: None,
            payload,
            padding_bytes: 0,
        }
    }

    /// Parse RTP packet from bytes
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 12 {
            return Err(anyhow!("RTP packet too short: {} bytes", data.len()));
        }

        let mut cursor = Cursor::new(data);

        // First byte: V(2), P(1), X(1), CC(4)
        let first_byte = cursor.read_u8()?;
        let version = (first_byte >> 6) & 0x03;
        let padding = (first_byte >> 5) & 0x01 != 0;
        let extension = (first_byte >> 4) & 0x01 != 0;
        let csrc_count = first_byte & 0x0F;

        if version != 2 {
            return Err(anyhow!("Invalid RTP version: {}", version));
        }

        // Second byte: M(1), PT(7)
        let second_byte = cursor.read_u8()?;
        let marker = (second_byte >> 7) & 0x01 != 0;
        let payload_type = second_byte & 0x7F;

        // Sequence number, timestamp, SSRC
        let sequence_number = cursor.read_u16::<BigEndian>()?;
        let timestamp = cursor.read_u32::<BigEndian>()?;
        let ssrc = cursor.read_u32::<BigEndian>()?;

        // CSRC list
        let mut csrc = Vec::new();
        for _ in 0..csrc_count {
            csrc.push(cursor.read_u32::<BigEndian>()?);
        }

        // Extension header
        let extension_header = if extension {
            let profile = cursor.read_u16::<BigEndian>()?;
            let length = cursor.read_u16::<BigEndian>()?;
            let mut ext_data = vec![0u8; (length * 4) as usize];
            std::io::Read::read_exact(&mut cursor, &mut ext_data)?;
            Some(RtpExtension {
                profile,
                length,
                data: ext_data,
            })
        } else {
            None
        };

        // Payload
        let header_len = cursor.position() as usize;
        let mut payload = data[header_len..].to_vec();

        // Handle padding
        let padding_bytes = if padding && !payload.is_empty() {
            let pad_count = payload[payload.len() - 1];
            if pad_count as usize <= payload.len() {
                let new_len = payload.len() - pad_count as usize;
                payload.truncate(new_len);
                pad_count
            } else {
                warn!("Invalid padding count: {}", pad_count);
                0
            }
        } else {
            0
        };

        Ok(Self {
            version,
            padding,
            extension,
            csrc_count,
            marker,
            payload_type,
            sequence_number,
            timestamp,
            ssrc,
            csrc,
            extension_header,
            payload,
            padding_bytes,
        })
    }

    /// Serialize RTP packet to bytes
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();

        // Calculate total size
        let mut size = 12 + (self.csrc.len() * 4);
        if let Some(ref ext) = self.extension_header {
            size += 4 + ext.data.len();
        }
        size += self.payload.len();
        if self.padding {
            size += self.padding_bytes as usize;
        }

        buffer.reserve(size);

        // First byte: V(2), P(1), X(1), CC(4)
        let first_byte = (self.version << 6)
            | ((self.padding as u8) << 5)
            | ((self.extension as u8) << 4)
            | (self.csrc_count & 0x0F);
        buffer.write_u8(first_byte)?;

        // Second byte: M(1), PT(7)
        let second_byte = ((self.marker as u8) << 7) | (self.payload_type & 0x7F);
        buffer.write_u8(second_byte)?;

        // Sequence number, timestamp, SSRC
        buffer.write_u16::<BigEndian>(self.sequence_number)?;
        buffer.write_u32::<BigEndian>(self.timestamp)?;
        buffer.write_u32::<BigEndian>(self.ssrc)?;

        // CSRC list
        for &csrc in &self.csrc {
            buffer.write_u32::<BigEndian>(csrc)?;
        }

        // Extension header
        if let Some(ref ext) = self.extension_header {
            buffer.write_u16::<BigEndian>(ext.profile)?;
            buffer.write_u16::<BigEndian>(ext.length)?;
            buffer.extend_from_slice(&ext.data);
        }

        // Payload
        buffer.extend_from_slice(&self.payload);

        // Padding
        if self.padding {
            for _ in 0..(self.padding_bytes - 1) {
                buffer.write_u8(0)?;
            }
            buffer.write_u8(self.padding_bytes)?;
        }

        Ok(buffer)
    }

    /// Get the size of the RTP header in bytes
    pub fn header_size(&self) -> usize {
        let mut size = 12; // Fixed header
        size += self.csrc.len() * 4; // CSRC list
        if let Some(ref ext) = self.extension_header {
            size += 4 + ext.data.len(); // Extension
        }
        size
    }

    /// Check if this is a valid RTP packet
    pub fn is_valid(&self) -> bool {
        self.version == 2 && self.csrc_count == self.csrc.len() as u8
    }

    /// Generate current NTP timestamp
    pub fn current_ntp_timestamp() -> u64 {
        let since_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();

        // NTP epoch starts Jan 1, 1900, Unix epoch starts Jan 1, 1970
        // Difference is 70 years = 2208988800 seconds
        let ntp_seconds = since_epoch.as_secs() + 2208988800;
        let ntp_fraction = ((since_epoch.subsec_nanos() as u64) << 32) / 1_000_000_000;

        (ntp_seconds << 32) | ntp_fraction
    }

    /// Convert NTP timestamp to RTP timestamp for given sample rate
    pub fn ntp_to_rtp_timestamp(ntp_timestamp: u64, sample_rate: u32) -> u32 {
        let ntp_seconds = (ntp_timestamp >> 32) as f64;
        let ntp_fraction = (ntp_timestamp & 0xFFFFFFFF) as f64 / (1u64 << 32) as f64;
        let time_seconds = ntp_seconds + ntp_fraction;

        // Convert to RTP timestamp units
        (time_seconds * sample_rate as f64) as u32
    }
}

/// RTP session statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RtpStats {
    pub packets_sent: u64,
    pub packets_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_lost: u64,
    pub jitter: f64,
    pub round_trip_time: Option<f64>,
    pub last_sequence_number: u16,
    pub last_timestamp: u32,
}

impl RtpStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_sent(&mut self, packet: &RtpPacket) {
        self.packets_sent += 1;
        self.bytes_sent += packet.payload.len() as u64;
    }

    pub fn update_received(&mut self, packet: &RtpPacket) {
        self.packets_received += 1;
        self.bytes_received += packet.payload.len() as u64;

        // Calculate packet loss
        let expected_seq = self.last_sequence_number.wrapping_add(1);
        if packet.sequence_number != expected_seq && self.packets_received > 1 {
            let gap = packet.sequence_number.wrapping_sub(expected_seq);
            self.packets_lost += gap as u64;
        }

        self.last_sequence_number = packet.sequence_number;
        self.last_timestamp = packet.timestamp;
    }

    pub fn packet_loss_rate(&self) -> f64 {
        if self.packets_received + self.packets_lost == 0 {
            0.0
        } else {
            self.packets_lost as f64 / (self.packets_received + self.packets_lost) as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rtp_packet_creation() {
        let payload = vec![1, 2, 3, 4, 5];
        let packet = RtpPacket::new(0, 1234, 567890, 0x12345678, payload.clone());

        assert_eq!(packet.version, 2);
        assert_eq!(packet.payload_type, 0);
        assert_eq!(packet.sequence_number, 1234);
        assert_eq!(packet.timestamp, 567890);
        assert_eq!(packet.ssrc, 0x12345678);
        assert_eq!(packet.payload, payload);
        assert!(packet.is_valid());
    }

    #[test]
    fn test_rtp_packet_serialization() {
        let payload = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let packet = RtpPacket::new(96, 12345, 987654321, 0xABCDEF00, payload);

        let serialized = packet.serialize().unwrap();
        let parsed = RtpPacket::parse(&serialized).unwrap();

        assert_eq!(packet, parsed);
    }

    #[test]
    fn test_rtp_packet_with_csrc() {
        let mut packet = RtpPacket::new(0, 1, 2, 3, vec![1, 2, 3]);
        packet.csrc = vec![0x11111111, 0x22222222];
        packet.csrc_count = 2;

        let serialized = packet.serialize().unwrap();
        let parsed = RtpPacket::parse(&serialized).unwrap();

        assert_eq!(packet.csrc, parsed.csrc);
        assert_eq!(packet.csrc_count, parsed.csrc_count);
    }

    #[test]
    fn test_rtp_stats() {
        let mut stats = RtpStats::new();
        let packet1 = RtpPacket::new(0, 100, 1000, 1, vec![1, 2, 3]);
        let packet2 = RtpPacket::new(0, 101, 1020, 1, vec![4, 5, 6]);
        let packet3 = RtpPacket::new(0, 103, 1060, 1, vec![7, 8, 9]); // Gap at 102

        stats.update_received(&packet1);
        stats.update_received(&packet2);
        stats.update_received(&packet3);

        assert_eq!(stats.packets_received, 3);
        assert_eq!(stats.packets_lost, 1); // Sequence 102 was lost
        assert_eq!(stats.bytes_received, 9);
        assert!(stats.packet_loss_rate() > 0.0);
    }

    #[test]
    fn test_invalid_rtp_packet() {
        let short_data = vec![1, 2, 3]; // Too short
        assert!(RtpPacket::parse(&short_data).is_err());

        let wrong_version = vec![0x40, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3]; // Version 1
        assert!(RtpPacket::parse(&wrong_version).is_err());
    }
}
