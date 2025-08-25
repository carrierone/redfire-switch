/*
 * SDP (Session Description Protocol) Implementation
 * RFC 4566 compliant SDP parsing and generation for media negotiation
 */

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;
use tracing::{debug, warn};

/// SDP session description
#[derive(Debug, Clone, PartialEq)]
pub struct SdpSession {
    /// Session-level attributes
    pub session_name: String,
    pub session_info: Option<String>,
    pub uri: Option<String>,
    pub email: Vec<String>,
    pub phone: Vec<String>,
    pub connection: Option<ConnectionData>,
    pub bandwidth: Vec<BandwidthInfo>,
    pub times: Vec<TimeDescription>,
    pub encryption_key: Option<String>,
    pub attributes: HashMap<String, Option<String>>,

    /// Media descriptions
    pub media: Vec<MediaDescription>,

    /// Version (always 0)
    pub version: u8,
    /// Origin information
    pub origin: OriginField,
}

/// Origin field (o=)
#[derive(Debug, Clone, PartialEq)]
pub struct OriginField {
    pub username: String,
    pub session_id: String,
    pub session_version: String,
    pub network_type: String,
    pub address_type: String,
    pub address: String,
}

/// Time description (t=)
#[derive(Debug, Clone, PartialEq)]
pub struct TimeDescription {
    pub start_time: u64,
    pub stop_time: u64,
    pub repeat_times: Vec<RepeatTime>,
}

/// Repeat time (r=)
#[derive(Debug, Clone, PartialEq)]
pub struct RepeatTime {
    pub interval: u64,
    pub duration: u64,
    pub offsets: Vec<u64>,
}

/// Connection data (c=)
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectionData {
    pub network_type: String,
    pub address_type: String,
    pub address: IpAddr,
    pub ttl: Option<u8>,
    pub num_addresses: Option<u32>,
}

/// Bandwidth information (b=)
#[derive(Debug, Clone, PartialEq)]
pub struct BandwidthInfo {
    pub bandwidth_type: String,
    pub bandwidth: u32,
}

/// Media description (m=)
#[derive(Debug, Clone, PartialEq)]
pub struct MediaDescription {
    /// Media type (audio, video, application, etc.)
    pub media_type: MediaType,
    /// Port number
    pub port: u16,
    /// Number of ports (for multicast)
    pub num_ports: Option<u16>,
    /// Transport protocol
    pub protocol: String,
    /// Format list (payload types)
    pub formats: Vec<String>,

    /// Media-level attributes
    pub connection: Option<ConnectionData>,
    pub bandwidth: Vec<BandwidthInfo>,
    pub encryption_key: Option<String>,
    pub attributes: HashMap<String, Option<String>>,

    /// Parsed codec information
    pub codecs: Vec<CodecInfo>,
}

/// Media type enumeration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MediaType {
    Audio,
    Video,
    Application,
    Data,
    Control,
    Unknown(String),
}

impl fmt::Display for MediaType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MediaType::Audio => write!(f, "audio"),
            MediaType::Video => write!(f, "video"),
            MediaType::Application => write!(f, "application"),
            MediaType::Data => write!(f, "data"),
            MediaType::Control => write!(f, "control"),
            MediaType::Unknown(s) => write!(f, "{}", s),
        }
    }
}

impl FromStr for MediaType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        Ok(match s.to_lowercase().as_str() {
            "audio" => MediaType::Audio,
            "video" => MediaType::Video,
            "application" => MediaType::Application,
            "data" => MediaType::Data,
            "control" => MediaType::Control,
            _ => MediaType::Unknown(s.to_string()),
        })
    }
}

/// Codec information parsed from SDP
#[derive(Debug, Clone, PartialEq)]
pub struct CodecInfo {
    pub payload_type: u8,
    pub name: String,
    pub clock_rate: u32,
    pub channels: Option<u8>,
    pub format_parameters: HashMap<String, String>,
}

impl SdpSession {
    /// Parse SDP from string
    pub fn parse(sdp_text: &str) -> Result<Self> {
        let lines: Vec<&str> = sdp_text.lines().collect();
        if lines.is_empty() {
            return Err(anyhow!("Empty SDP"));
        }

        let mut session = SdpSession {
            version: 0,
            origin: OriginField {
                username: "-".to_string(),
                session_id: "0".to_string(),
                session_version: "0".to_string(),
                network_type: "IN".to_string(),
                address_type: "IP4".to_string(),
                address: "0.0.0.0".to_string(),
            },
            session_name: "-".to_string(),
            session_info: None,
            uri: None,
            email: Vec::new(),
            phone: Vec::new(),
            connection: None,
            bandwidth: Vec::new(),
            times: Vec::new(),
            encryption_key: None,
            attributes: HashMap::new(),
            media: Vec::new(),
        };

        let mut current_media: Option<MediaDescription> = None;
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();
            if line.is_empty() {
                i += 1;
                continue;
            }

            if line.len() < 2 || line.chars().nth(1) != Some('=') {
                warn!("Invalid SDP line format: {}", line);
                i += 1;
                continue;
            }

            let line_type = line
                .chars()
                .nth(0)
                .ok_or_else(|| anyhow!("Empty SDP line"))?;
            if line.len() < 2 {
                return Err(anyhow!("Invalid SDP line format: {}", line));
            }
            let line_value = &line[2..];

            match line_type {
                'v' => session.version = line_value.parse().unwrap_or(0),
                'o' => session.origin = Self::parse_origin(line_value)?,
                's' => session.session_name = line_value.to_string(),
                'i' => {
                    if let Some(ref mut media) = current_media {
                        // Media-level session information (not commonly used)
                    } else {
                        session.session_info = Some(line_value.to_string());
                    }
                }
                'u' => session.uri = Some(line_value.to_string()),
                'e' => session.email.push(line_value.to_string()),
                'p' => session.phone.push(line_value.to_string()),
                'c' => {
                    let connection = Self::parse_connection(line_value)?;
                    if let Some(ref mut media) = current_media {
                        media.connection = Some(connection);
                    } else {
                        session.connection = Some(connection);
                    }
                }
                'b' => {
                    let bandwidth = Self::parse_bandwidth(line_value)?;
                    if let Some(ref mut media) = current_media {
                        media.bandwidth.push(bandwidth);
                    } else {
                        session.bandwidth.push(bandwidth);
                    }
                }
                't' => {
                    let time_desc = Self::parse_time(line_value)?;
                    session.times.push(time_desc);
                }
                'k' => {
                    if let Some(ref mut media) = current_media {
                        media.encryption_key = Some(line_value.to_string());
                    } else {
                        session.encryption_key = Some(line_value.to_string());
                    }
                }
                'a' => {
                    let (key, value) = Self::parse_attribute(line_value);
                    if let Some(ref mut media) = current_media {
                        media.attributes.insert(key, value);
                    } else {
                        session.attributes.insert(key, value);
                    }
                }
                'm' => {
                    // Save previous media description
                    if let Some(media) = current_media.take() {
                        session.media.push(media);
                    }
                    // Parse new media description
                    current_media = Some(Self::parse_media(line_value)?);
                }
                _ => {
                    debug!("Unknown SDP line type: {}", line_type);
                }
            }

            i += 1;
        }

        // Save last media description
        if let Some(media) = current_media {
            session.media.push(media);
        }

        // Post-process: parse codec information from attributes
        for media in &mut session.media {
            Self::parse_codecs(media)?;
        }

        Ok(session)
    }

    /// Parse origin field (o=)
    fn parse_origin(value: &str) -> Result<OriginField> {
        let parts: Vec<&str> = value.split_whitespace().collect();
        if parts.len() != 6 {
            return Err(anyhow!("Invalid origin field: {}", value));
        }

        Ok(OriginField {
            username: parts[0].to_string(),
            session_id: parts[1].to_string(),
            session_version: parts[2].to_string(),
            network_type: parts[3].to_string(),
            address_type: parts[4].to_string(),
            address: parts[5].to_string(),
        })
    }

    /// Parse connection data (c=)
    fn parse_connection(value: &str) -> Result<ConnectionData> {
        let parts: Vec<&str> = value.split_whitespace().collect();
        if parts.len() < 3 {
            return Err(anyhow!("Invalid connection field: {}", value));
        }

        let network_type = parts[0].to_string();
        let address_type = parts[1].to_string();
        let address_part = parts[2];

        // Parse address with optional TTL and number of addresses
        let (address_str, ttl, num_addresses) = if address_part.contains('/') {
            let addr_parts: Vec<&str> = address_part.split('/').collect();
            let addr = addr_parts[0];
            let ttl = if addr_parts.len() > 1 {
                addr_parts[1].parse().ok()
            } else {
                None
            };
            let num_addr = if addr_parts.len() > 2 {
                addr_parts[2].parse().ok()
            } else {
                None
            };
            (addr, ttl, num_addr)
        } else {
            (address_part, None, None)
        };

        let address = address_str
            .parse()
            .map_err(|_| anyhow!("Invalid IP address: {}", address_str))?;

        Ok(ConnectionData {
            network_type,
            address_type,
            address,
            ttl,
            num_addresses,
        })
    }

    /// Parse bandwidth information (b=)
    fn parse_bandwidth(value: &str) -> Result<BandwidthInfo> {
        if let Some(colon_pos) = value.find(':') {
            let bandwidth_type = value[..colon_pos].to_string();
            let bandwidth = value[colon_pos + 1..]
                .parse()
                .map_err(|_| anyhow!("Invalid bandwidth value: {}", value))?;

            Ok(BandwidthInfo {
                bandwidth_type,
                bandwidth,
            })
        } else {
            Err(anyhow!("Invalid bandwidth format: {}", value))
        }
    }

    /// Parse time description (t=)
    fn parse_time(value: &str) -> Result<TimeDescription> {
        let parts: Vec<&str> = value.split_whitespace().collect();
        if parts.len() != 2 {
            return Err(anyhow!("Invalid time field: {}", value));
        }

        let start_time = parts[0]
            .parse()
            .map_err(|_| anyhow!("Invalid start time: {}", parts[0]))?;
        let stop_time = parts[1]
            .parse()
            .map_err(|_| anyhow!("Invalid stop time: {}", parts[1]))?;

        Ok(TimeDescription {
            start_time,
            stop_time,
            repeat_times: Vec::new(),
        })
    }

    /// Parse attribute (a=)
    fn parse_attribute(value: &str) -> (String, Option<String>) {
        if let Some(colon_pos) = value.find(':') {
            let key = value[..colon_pos].to_string();
            let val = value[colon_pos + 1..].to_string();
            (key, Some(val))
        } else {
            (value.to_string(), None)
        }
    }

    /// Parse media description (m=)
    fn parse_media(value: &str) -> Result<MediaDescription> {
        let parts: Vec<&str> = value.split_whitespace().collect();
        if parts.len() < 4 {
            return Err(anyhow!("Invalid media field: {}", value));
        }

        let media_type = parts[0].parse()?;

        let (port, num_ports) = if parts[1].contains('/') {
            let port_parts: Vec<&str> = parts[1].split('/').collect();
            let port = port_parts[0]
                .parse()
                .map_err(|_| anyhow!("Invalid port: {}", port_parts[0]))?;
            let num_ports = if port_parts.len() > 1 {
                Some(
                    port_parts[1]
                        .parse()
                        .map_err(|_| anyhow!("Invalid number of ports: {}", port_parts[1]))?,
                )
            } else {
                None
            };
            (port, num_ports)
        } else {
            let port = parts[1]
                .parse()
                .map_err(|_| anyhow!("Invalid port: {}", parts[1]))?;
            (port, None)
        };

        let protocol = parts[2].to_string();
        let formats = parts[3..].iter().map(|s| s.to_string()).collect();

        Ok(MediaDescription {
            media_type,
            port,
            num_ports,
            protocol,
            formats,
            connection: None,
            bandwidth: Vec::new(),
            encryption_key: None,
            attributes: HashMap::new(),
            codecs: Vec::new(),
        })
    }

    /// Parse codec information from media attributes
    fn parse_codecs(media: &mut MediaDescription) -> Result<()> {
        let mut codecs = Vec::new();

        for format in &media.formats {
            if let Ok(payload_type) = format.parse::<u8>() {
                // Look for rtpmap attribute
                let rtpmap_key = format!("rtpmap:{}", payload_type);
                if let Some(Some(rtpmap_value)) = media.attributes.get(&rtpmap_key) {
                    let codec = Self::parse_rtpmap(payload_type, rtpmap_value)?;
                    codecs.push(codec);
                } else {
                    // Static payload type
                    if let Some(codec) = Self::static_payload_type(payload_type) {
                        codecs.push(codec);
                    }
                }
            }
        }

        // Parse format parameters (fmtp)
        for codec in &mut codecs {
            let fmtp_key = format!("fmtp:{}", codec.payload_type);
            if let Some(Some(fmtp_value)) = media.attributes.get(&fmtp_key) {
                codec.format_parameters = Self::parse_fmtp(fmtp_value);
            }
        }

        media.codecs = codecs;
        Ok(())
    }

    /// Parse rtpmap attribute
    fn parse_rtpmap(payload_type: u8, rtpmap: &str) -> Result<CodecInfo> {
        let parts: Vec<&str> = rtpmap.split_whitespace().collect();
        if parts.len() != 2 {
            return Err(anyhow!("Invalid rtpmap format: {}", rtpmap));
        }

        let name = parts[0].to_string();
        let clock_info = parts[1];

        let (clock_rate, channels) = if clock_info.contains('/') {
            let clock_parts: Vec<&str> = clock_info.split('/').collect();
            let rate = clock_parts[0]
                .parse()
                .map_err(|_| anyhow!("Invalid clock rate: {}", clock_parts[0]))?;
            let ch = if clock_parts.len() > 1 {
                Some(
                    clock_parts[1]
                        .parse()
                        .map_err(|_| anyhow!("Invalid channels: {}", clock_parts[1]))?,
                )
            } else {
                None
            };
            (rate, ch)
        } else {
            let rate = clock_info
                .parse()
                .map_err(|_| anyhow!("Invalid clock rate: {}", clock_info))?;
            (rate, None)
        };

        Ok(CodecInfo {
            payload_type,
            name,
            clock_rate,
            channels,
            format_parameters: HashMap::new(),
        })
    }

    /// Parse fmtp (format parameters) attribute
    fn parse_fmtp(fmtp: &str) -> HashMap<String, String> {
        let mut params = HashMap::new();

        for param in fmtp.split(';') {
            let param = param.trim();
            if let Some(eq_pos) = param.find('=') {
                let key = param[..eq_pos].trim().to_string();
                let value = param[eq_pos + 1..].trim().to_string();
                params.insert(key, value);
            } else {
                params.insert(param.to_string(), "".to_string());
            }
        }

        params
    }

    /// Get codec info for static payload types
    fn static_payload_type(pt: u8) -> Option<CodecInfo> {
        match pt {
            0 => Some(CodecInfo {
                payload_type: 0,
                name: "PCMU".to_string(),
                clock_rate: 8000,
                channels: Some(1),
                format_parameters: HashMap::new(),
            }),
            8 => Some(CodecInfo {
                payload_type: 8,
                name: "PCMA".to_string(),
                clock_rate: 8000,
                channels: Some(1),
                format_parameters: HashMap::new(),
            }),
            9 => Some(CodecInfo {
                payload_type: 9,
                name: "G722".to_string(),
                clock_rate: 8000, // Note: G.722 is 16kHz but RTP clock is 8kHz
                channels: Some(1),
                format_parameters: HashMap::new(),
            }),
            18 => Some(CodecInfo {
                payload_type: 18,
                name: "G729".to_string(),
                clock_rate: 8000,
                channels: Some(1),
                format_parameters: HashMap::new(),
            }),
            _ => None,
        }
    }

    /// Generate SDP string from session
    pub fn to_string(&self) -> String {
        let mut sdp = String::new();

        // Version
        sdp.push_str(&format!("v={}\r\n", self.version));

        // Origin
        sdp.push_str(&format!(
            "o={} {} {} {} {} {}\r\n",
            self.origin.username,
            self.origin.session_id,
            self.origin.session_version,
            self.origin.network_type,
            self.origin.address_type,
            self.origin.address
        ));

        // Session name
        sdp.push_str(&format!("s={}\r\n", self.session_name));

        // Session information
        if let Some(ref info) = self.session_info {
            sdp.push_str(&format!("i={}\r\n", info));
        }

        // URI
        if let Some(ref uri) = self.uri {
            sdp.push_str(&format!("u={}\r\n", uri));
        }

        // Email
        for email in &self.email {
            sdp.push_str(&format!("e={}\r\n", email));
        }

        // Phone
        for phone in &self.phone {
            sdp.push_str(&format!("p={}\r\n", phone));
        }

        // Connection (session-level)
        if let Some(ref conn) = self.connection {
            sdp.push_str(&Self::connection_to_string(conn));
        }

        // Bandwidth (session-level)
        for bw in &self.bandwidth {
            sdp.push_str(&format!("b={}:{}\r\n", bw.bandwidth_type, bw.bandwidth));
        }

        // Time descriptions
        for time in &self.times {
            sdp.push_str(&format!("t={} {}\r\n", time.start_time, time.stop_time));
        }

        // Session attributes
        for (key, value) in &self.attributes {
            if let Some(val) = value {
                sdp.push_str(&format!("a={}:{}\r\n", key, val));
            } else {
                sdp.push_str(&format!("a={}\r\n", key));
            }
        }

        // Media descriptions
        for media in &self.media {
            sdp.push_str(&Self::media_to_string(media));
        }

        sdp
    }

    fn connection_to_string(conn: &ConnectionData) -> String {
        let mut result = format!(
            "c={} {} {}",
            conn.network_type, conn.address_type, conn.address
        );

        if let Some(ttl) = conn.ttl {
            result.push_str(&format!("/{}", ttl));
            if let Some(num) = conn.num_addresses {
                result.push_str(&format!("/{}", num));
            }
        }

        result.push_str("\r\n");
        result
    }

    fn media_to_string(media: &MediaDescription) -> String {
        let mut result = String::new();

        // Media line
        result.push_str(&format!("m={} {}", media.media_type, media.port));
        if let Some(num_ports) = media.num_ports {
            result.push_str(&format!("/{}", num_ports));
        }
        result.push_str(&format!(" {}", media.protocol));
        for format in &media.formats {
            result.push_str(&format!(" {}", format));
        }
        result.push_str("\r\n");

        // Media-level connection
        if let Some(ref conn) = media.connection {
            result.push_str(&Self::connection_to_string(conn));
        }

        // Media-level bandwidth
        for bw in &media.bandwidth {
            result.push_str(&format!("b={}:{}\r\n", bw.bandwidth_type, bw.bandwidth));
        }

        // Media attributes
        for (key, value) in &media.attributes {
            if let Some(val) = value {
                result.push_str(&format!("a={}:{}\r\n", key, val));
            } else {
                result.push_str(&format!("a={}\r\n", key));
            }
        }

        result
    }

    /// Find audio media description
    pub fn find_audio_media(&self) -> Option<&MediaDescription> {
        self.media.iter().find(|m| m.media_type == MediaType::Audio)
    }

    /// Find video media description
    pub fn find_video_media(&self) -> Option<&MediaDescription> {
        self.media.iter().find(|m| m.media_type == MediaType::Video)
    }

    /// Get common codecs between two SDP sessions
    pub fn find_common_codecs(&self, other: &SdpSession) -> Vec<CodecInfo> {
        let mut common = Vec::new();

        if let (Some(audio1), Some(audio2)) = (self.find_audio_media(), other.find_audio_media()) {
            for codec1 in &audio1.codecs {
                for codec2 in &audio2.codecs {
                    if codec1.name == codec2.name && codec1.clock_rate == codec2.clock_rate {
                        common.push(codec1.clone());
                        break;
                    }
                }
            }
        }

        common
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sdp_parsing() {
        let sdp_text = r#"v=0
o=alice 2890844526 2890844527 IN IP4 192.168.1.100
s=
c=IN IP4 192.168.1.100
t=0 0
m=audio 49170 RTP/AVP 0 8
a=rtpmap:0 PCMU/8000
a=rtpmap:8 PCMA/8000
"#;

        let session = SdpSession::parse(sdp_text).unwrap();

        assert_eq!(session.version, 0);
        assert_eq!(session.origin.username, "alice");
        assert_eq!(session.media.len(), 1);

        let audio = &session.media[0];
        assert_eq!(audio.media_type, MediaType::Audio);
        assert_eq!(audio.port, 49170);
        assert_eq!(audio.codecs.len(), 2);

        assert_eq!(audio.codecs[0].name, "PCMU");
        assert_eq!(audio.codecs[0].clock_rate, 8000);
        assert_eq!(audio.codecs[1].name, "PCMA");
    }

    #[test]
    fn test_sdp_generation() {
        let mut session = SdpSession {
            version: 0,
            origin: OriginField {
                username: "test".to_string(),
                session_id: "123456".to_string(),
                session_version: "1".to_string(),
                network_type: "IN".to_string(),
                address_type: "IP4".to_string(),
                address: "192.168.1.100".to_string(),
            },
            session_name: "Test Session".to_string(),
            session_info: None,
            uri: None,
            email: Vec::new(),
            phone: Vec::new(),
            connection: Some(ConnectionData {
                network_type: "IN".to_string(),
                address_type: "IP4".to_string(),
                address: "192.168.1.100".parse().unwrap(),
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
            media: Vec::new(),
        };

        let mut audio_media = MediaDescription {
            media_type: MediaType::Audio,
            port: 5004,
            num_ports: None,
            protocol: "RTP/AVP".to_string(),
            formats: vec!["0".to_string(), "8".to_string()],
            connection: None,
            bandwidth: Vec::new(),
            encryption_key: None,
            attributes: HashMap::new(),
            codecs: Vec::new(),
        };

        audio_media
            .attributes
            .insert("rtpmap:0".to_string(), Some("PCMU/8000".to_string()));
        audio_media
            .attributes
            .insert("rtpmap:8".to_string(), Some("PCMA/8000".to_string()));

        session.media.push(audio_media);

        let sdp_string = session.to_string();
        assert!(sdp_string.contains("v=0"));
        assert!(sdp_string.contains("o=test 123456 1 IN IP4 192.168.1.100"));
        assert!(sdp_string.contains("s=Test Session"));
        assert!(sdp_string.contains("m=audio 5004 RTP/AVP 0 8"));
    }

    #[test]
    fn test_codec_negotiation() {
        let sdp1_text = r#"v=0
o=alice 1 1 IN IP4 192.168.1.100
s=-
c=IN IP4 192.168.1.100
t=0 0
m=audio 5004 RTP/AVP 0 8 18
a=rtpmap:0 PCMU/8000
a=rtpmap:8 PCMA/8000
a=rtpmap:18 G729/8000
"#;

        let sdp2_text = r#"v=0
o=bob 1 1 IN IP4 192.168.1.200
s=-
c=IN IP4 192.168.1.200
t=0 0
m=audio 5006 RTP/AVP 8 18 96
a=rtpmap:8 PCMA/8000
a=rtpmap:18 G729/8000
a=rtpmap:96 opus/48000/2
"#;

        let session1 = SdpSession::parse(sdp1_text).unwrap();
        let session2 = SdpSession::parse(sdp2_text).unwrap();

        let common_codecs = session1.find_common_codecs(&session2);
        assert_eq!(common_codecs.len(), 2); // PCMA and G729

        assert!(common_codecs.iter().any(|c| c.name == "PCMA"));
        assert!(common_codecs.iter().any(|c| c.name == "G729"));
    }
}
