//! RFC-compliant SIP and SIP-I implementation
//!
//! This module provides RFC 3261 (SIP), RFC 3372 (SIP-T), and ITU-T Q.1912.5 (SIP-I)
//! compliant parsing and validation for telecommunications signaling.

use anyhow::{anyhow, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;

/// RFC 3261 mandatory headers that MUST be present in all SIP requests
pub const MANDATORY_SIP_HEADERS: &[&str] =
    &["To", "From", "CSeq", "Call-ID", "Max-Forwards", "Via"];

/// Valid SIP version per RFC 3261
pub const VALID_SIP_VERSION: &str = "SIP/2.0";

/// Maximum allowed URI length to prevent memory exhaustion attacks
pub const MAX_URI_LENGTH: usize = 8192;

/// Maximum allowed header value length
pub const MAX_HEADER_LENGTH: usize = 4096;

/// Regex for parsing SIP URIs per RFC 3261 - ReDoS safe version
static SIP_URI_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(sip|sips|tel):(?:([^@;?<>]+)@)?([^;?<>]+)(?:[;?](.+))?$")
        .expect("Invalid SIP URI regex")
});

/// Regex for extracting URI parameters - ReDoS safe version
static URI_PARAM_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"([^;=\s]+)=([^;\s]*)")
        .expect("Invalid URI parameter regex")
});

/// Regex for validating E.164 phone numbers - RFC compliant
static E164_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\+[1-9]\d{0,14}$")
        .expect("Invalid E164 regex")
});

/// SIP URI components per RFC 3261
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipUri {
    pub scheme: String,                      // sip, sips, or tel
    pub user: Option<String>,                // User part (phone number usually)
    pub host: String,                        // Domain or IP
    pub port: Option<u16>,                   // Port number if specified
    pub parameters: HashMap<String, String>, // URI parameters (;key=value)
    pub headers: HashMap<String, String>,    // URI headers (?key=value)
}

/// ANI-II/OLI information extracted from SIP headers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OriginatingLineInfo {
    pub oli_value: u8,                  // OLI/ANI-II digit (0-99)
    pub source: OliSource,              // Where it was found
    pub calling_number: Option<String>, // Associated calling number
    pub screening: Option<ScreeningIndicator>,
    pub presentation: Option<PresentationIndicator>,
}

/// Source of OLI/ANI-II information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OliSource {
    FromUriParam,      // From header ;oli= or ;isup-oli=
    PIsupOli,          // P-ISUP-OLI header
    RemotePartyId,     // Remote-Party-ID header
    PAssertedIdentity, // P-Asserted-Identity header
    IsupBody,          // ISUP message body
    DiversionHeader,   // Diversion header
}

/// Screening indicator from ISUP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScreeningIndicator {
    UserProvidedNotScreened = 0,
    UserProvidedVerifiedPassed = 1,
    UserProvidedVerifiedFailed = 2,
    NetworkProvided = 3,
}

/// Presentation indicator from ISUP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PresentationIndicator {
    Allowed = 0,
    Restricted = 1,
    AddressNotAvailable = 2,
    Reserved = 3,
}

/// Helper function to get header value case-insensitively
fn get_header_case_insensitive<'a>(
    headers: &'a HashMap<String, String>,
    header_name: &str,
) -> Option<&'a String> {
    // Try exact match first (fastest)
    if let Some(value) = headers.get(header_name) {
        return Some(value);
    }

    // Try case-insensitive search
    let lower_name = header_name.to_lowercase();
    for (key, value) in headers {
        if key.to_lowercase() == lower_name {
            return Some(value);
        }
    }
    None
}

/// Extract OLI information from SIP headers and body
pub fn extract_oli_info(
    headers: &HashMap<String, String>,
    body: Option<&str>,
) -> Option<OriginatingLineInfo> {
    // Try P-ISUP-OLI header first (highest priority)
    if let Some(oli_header) = get_header_case_insensitive(headers, "P-ISUP-OLI") {
        if let Ok(oli_value) = oli_header.parse::<u8>() {
            return Some(OriginatingLineInfo {
                oli_value,
                source: OliSource::PIsupOli,
                calling_number: get_header_case_insensitive(headers, "From").cloned(),
                screening: None,
                presentation: None,
            });
        }
    }

    // Try From header with OLI parameter
    if let Some(from_header) = get_header_case_insensitive(headers, "From") {
        if let Ok(uri) = SipUriParser::parse_header_field(from_header) {
            if let Some(oli_str) = uri
                .1
                .parameters
                .get("oli")
                .or_else(|| uri.1.parameters.get("isup-oli"))
            {
                if let Ok(oli_value) = oli_str.parse::<u8>() {
                    return Some(OriginatingLineInfo {
                        oli_value,
                        source: OliSource::FromUriParam,
                        calling_number: uri.1.user.clone(),
                        screening: None,
                        presentation: None,
                    });
                }
            }
        }
    }

    // Try P-Asserted-Identity header
    if let Some(pai_header) = get_header_case_insensitive(headers, "P-Asserted-Identity") {
        if let Ok(uri) = SipUriParser::parse_header_field(pai_header) {
            if let Some(oli_str) = uri.1.parameters.get("oli") {
                if let Ok(oli_value) = oli_str.parse::<u8>() {
                    return Some(OriginatingLineInfo {
                        oli_value,
                        source: OliSource::PAssertedIdentity,
                        calling_number: uri.1.user.clone(),
                        screening: None,
                        presentation: None,
                    });
                }
            }
        }
    }

    // Try parsing ISUP from message body if present
    if let Some(body_content) = body {
        if let Some(oli_info) = IsupParser::extract_oli_from_body(body_content) {
            return Some(oli_info);
        }
    }

    None
}

/// Parser for ISUP content
pub struct IsupParser;

impl IsupParser {
    /// Extract OLI information from ISUP message body
    pub fn extract_oli_from_body(body: &str) -> Option<OriginatingLineInfo> {
        // Parse multipart/mixed content to find ISUP part
        if body.contains("Content-Type: application/isup") {
            // Extract hex-encoded ISUP data
            let lines: Vec<&str> = body.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                if line.contains("application/isup") && i + 2 < lines.len() {
                    let isup_data = lines[i + 2].trim();
                    if let Some(oli_value) = Self::parse_isup_calling_party_category(isup_data) {
                        return Some(OriginatingLineInfo {
                            oli_value,
                            source: OliSource::IsupBody,
                            calling_number: None,
                            screening: None,
                            presentation: None,
                        });
                    }
                }
            }
        }
        None
    }

    /// Parse ISUP IAM message to extract calling party category
    fn parse_isup_calling_party_category(isup_hex: &str) -> Option<u8> {
        // Remove whitespace and validate hex format
        let hex_data = isup_hex.replace(&[' ', '\t', '\r', '\n'][..], "");
        if hex_data.len() < 16 {
            // Minimum ISUP IAM size
            return None;
        }

        // Look for Calling Party Category parameter (typically at offset 12-13 in IAM)
        // This is a simplified parser - production would need full ISUP parsing
        if let Ok(bytes) = hex::decode(&hex_data) {
            if bytes.len() > 13 {
                // Calling Party Category is usually at byte 13 in ISUP IAM
                return Some(bytes[13]);
            }
        }

        None
    }
}

/// Parser for OLI parameters
pub struct OliParser;

impl OliParser {
    /// Parse OLI value from various string formats
    pub fn parse_oli_string(oli_str: &str) -> Option<u8> {
        oli_str.parse::<u8>().ok().filter(|&val| val <= 99)
    }

    /// Parse OLI information from SIP headers
    pub fn parse_from_headers(headers: &HashMap<String, String>) -> Option<OriginatingLineInfo> {
        extract_oli_info(headers, None)
    }
}

/// RFC 3261 compliant SIP message validation
pub struct Rfc3261Validator;

impl Rfc3261Validator {
    /// Validate a SIP message for RFC 3261 compliance
    pub fn validate_message(headers: &HashMap<String, String>, request_line: &str) -> Result<()> {
        // Check SIP version
        if !request_line.ends_with(VALID_SIP_VERSION) {
            return Err(anyhow!("Invalid SIP version - must be SIP/2.0"));
        }

        // Validate mandatory headers (case-insensitive)
        for header in MANDATORY_SIP_HEADERS {
            if get_header_case_insensitive(headers, header).is_none() {
                return Err(anyhow!("Missing mandatory header: {}", header));
            }
        }

        // Validate Request-URI (no unescaped spaces or control chars)
        if let Some(uri_part) = request_line.split_whitespace().nth(1) {
            if uri_part.contains(char::is_control) {
                return Err(anyhow!("Request-URI contains control characters"));
            }
            if uri_part.starts_with('<') || uri_part.ends_with('>') {
                return Err(anyhow!("Request-URI must not be enclosed in <>"));
            }
        }

        // Validate From and To headers have valid SIP URIs
        if let Some(from_header) = get_header_case_insensitive(headers, "From") {
            Self::validate_uri_header(from_header, "From")?;
        }
        if let Some(to_header) = get_header_case_insensitive(headers, "To") {
            Self::validate_uri_header(to_header, "To")?;
        }

        // Validate Call-ID format (should not contain spaces)
        if let Some(call_id) = get_header_case_insensitive(headers, "Call-ID") {
            if call_id.contains(' ') {
                return Err(anyhow!("Call-ID must not contain spaces"));
            }
        }

        // Validate Max-Forwards is numeric
        if let Some(max_fwd) = get_header_case_insensitive(headers, "Max-Forwards") {
            max_fwd
                .trim()
                .parse::<u32>()
                .map_err(|_| anyhow!("Max-Forwards must be numeric"))?;
        }

        Ok(())
    }

    /// Validate a SIP URI header field
    fn validate_uri_header(header: &str, header_name: &str) -> Result<()> {
        // Extract URI from header (may have display name and parameters)
        let uri_part = if let Some(start) = header.find('<') {
            if let Some(end) = header.find('>') {
                &header[start + 1..end]
            } else {
                return Err(anyhow!("{} header has unmatched < >", header_name));
            }
        } else {
            // No angle brackets, parse until semicolon or end
            header.split(';').next().unwrap_or(header)
        };

        // Basic URI validation
        if !uri_part.starts_with("sip:")
            && !uri_part.starts_with("sips:")
            && !uri_part.starts_with("tel:")
        {
            return Err(anyhow!(
                "{} header must contain a valid SIP or TEL URI",
                header_name
            ));
        }

        Ok(())
    }
}

/// Parser for SIP URIs compliant with RFC 3261
pub struct SipUriParser;

impl SipUriParser {
    /// Parse a SIP URI according to RFC 3261
    pub fn parse(uri_str: &str) -> Result<SipUri> {
        // Input length validation
        if uri_str.len() > MAX_URI_LENGTH {
            return Err(anyhow!(
                "URI exceeds maximum length of {} characters",
                MAX_URI_LENGTH
            ));
        }

        let captures = SIP_URI_REGEX
            .captures(uri_str)
            .ok_or_else(|| anyhow!("Invalid SIP URI format: {}", uri_str))?;

        let scheme = captures
            .get(1)
            .ok_or_else(|| anyhow!("Missing URI scheme"))?
            .as_str()
            .to_lowercase();
        let user = captures.get(2).map(|m| m.as_str().to_string());
        let host_part = captures
            .get(3)
            .ok_or_else(|| anyhow!("Missing URI host part"))?
            .as_str();

        // Parse host and port with proper IPv6 support
        let (host, port) = if host_part.starts_with('[') && host_part.contains("]:") {
            // IPv6 address with port: [::1]:5060
            if let Some(bracket_end) = host_part.find("]:") {
                let ipv6_part = &host_part[1..bracket_end]; // Remove brackets
                let port_str = &host_part[bracket_end + 2..];
                match port_str.parse::<u16>() {
                    Ok(port_num) => (ipv6_part.to_string(), Some(port_num)),
                    Err(_) => return Err(anyhow!("Invalid port number: {}", port_str)),
                }
            } else {
                (host_part.to_string(), None)
            }
        } else if let Some(colon_pos) = host_part.rfind(':') {
            // Regular hostname or IPv4 with port
            let host_part_left = &host_part[..colon_pos];
            let port_str = &host_part[colon_pos + 1..];

            // Don't treat IPv6 addresses without brackets as having ports
            if host_part_left.contains(':') && !host_part.starts_with('[') {
                // This looks like an IPv6 address without brackets
                (host_part.to_string(), None)
            } else {
                match port_str.parse::<u16>() {
                    Ok(port_num) => (host_part_left.to_string(), Some(port_num)),
                    Err(_) => return Err(anyhow!("Invalid port number: {}", port_str)),
                }
            }
        } else {
            (host_part.to_string(), None)
        };

        // Parse parameters and headers
        let mut parameters = HashMap::new();
        let mut headers = HashMap::new();

        if let Some(remainder) = captures.get(5).map(|m| m.as_str()) {
            // Split by ? to separate parameters from headers
            let parts: Vec<&str> = remainder.splitn(2, '?').collect();

            // Parse parameters (;key=value)
            if !parts[0].is_empty() {
                for param in parts[0].split(';') {
                    if param.is_empty() {
                        continue;
                    }
                    let kv: Vec<&str> = param.splitn(2, '=').collect();
                    if !kv.is_empty() && !kv[0].is_empty() {
                        parameters.insert(
                            kv[0].to_string(),
                            kv.get(1).map(|s| s.to_string()).unwrap_or_default(),
                        );
                    }
                }
            }

            // Parse headers (?key=value&key2=value2)
            if parts.len() > 1 && !parts[1].is_empty() {
                for header in parts[1].split('&') {
                    if header.is_empty() {
                        continue;
                    }
                    let kv: Vec<&str> = header.splitn(2, '=').collect();
                    if !kv.is_empty() && !kv[0].is_empty() {
                        headers.insert(
                            kv[0].to_string(),
                            kv.get(1).map(|s| s.to_string()).unwrap_or_default(),
                        );
                    }
                }
            }
        }

        Ok(SipUri {
            scheme,
            user,
            host,
            port,
            parameters,
            headers,
        })
    }

    /// Parse a full SIP header field (including display name and header parameters)
    pub fn parse_header_field(
        header: &str,
    ) -> Result<(Option<String>, SipUri, HashMap<String, String>)> {
        let mut display_name = None;
        let mut header_params = HashMap::new();

        // Find the URI part
        let (uri_str, params_str) = if let Some(start) = header.find('<') {
            if let Some(end) = header.find('>') {
                // Has angle brackets - extract display name
                if start > 0 {
                    display_name = Some(header[..start].trim().trim_matches('"').to_string());
                }

                let uri = &header[start + 1..end];
                let params = if end + 1 < header.len() {
                    &header[end + 1..]
                } else {
                    ""
                };
                (uri, params)
            } else {
                return Err(anyhow!("Unmatched angle brackets in header"));
            }
        } else {
            // No angle brackets - split at first semicolon outside URI
            if let Some(semi_pos) = header.find(';') {
                (&header[..semi_pos], &header[semi_pos..])
            } else {
                (header, "")
            }
        };

        // Parse the URI
        let uri = Self::parse(uri_str)?;

        // Parse header parameters (not URI parameters)
        for param in params_str.split(';') {
            if param.is_empty() {
                continue;
            }
            let kv: Vec<&str> = param.trim().splitn(2, '=').collect();
            header_params.insert(kv[0].to_string(), kv.get(1).unwrap_or(&"").to_string());
        }

        Ok((display_name, uri, header_params))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sip_uri_parsing() {
        let uri = SipUriParser::parse("sip:+12125551234@example.com;oli=70?Subject=Test").unwrap();
        assert_eq!(uri.scheme, "sip");
        assert_eq!(uri.user, Some("+12125551234".to_string()));
        assert_eq!(uri.host, "example.com");
        assert_eq!(uri.parameters.get("oli"), Some(&"70".to_string()));
    }

    #[test]
    fn test_from_header_with_oli() {
        let mut headers = HashMap::new();
        headers.insert(
            "From".to_string(),
            "<sip:+15551234567@carrier.com;oli=23>;tag=abc123".to_string(),
        );

        let oli = OliParser::parse_from_headers(&headers).unwrap();
        assert_eq!(oli.oli_value, 23);
        assert!(matches!(oli.source, OliSource::FromUriParam));
    }

    #[test]
    fn test_p_isup_oli_header() {
        let mut headers = HashMap::new();
        headers.insert("P-ISUP-OLI".to_string(), "70".to_string());

        let oli = OliParser::parse_from_headers(&headers).unwrap();
        assert_eq!(oli.oli_value, 70);
        assert!(matches!(oli.source, OliSource::PIsupOli));
    }

    #[test]
    fn test_rfc3261_validation() {
        let mut headers = HashMap::new();
        headers.insert("To".to_string(), "<sip:bob@example.com>".to_string());
        headers.insert(
            "From".to_string(),
            "<sip:alice@example.com>;tag=123".to_string(),
        );
        headers.insert("Call-ID".to_string(), "abc123@example.com".to_string());
        headers.insert("CSeq".to_string(), "1 INVITE".to_string());
        headers.insert("Via".to_string(), "SIP/2.0/UDP example.com".to_string());
        headers.insert("Max-Forwards".to_string(), "70".to_string());

        assert!(
            Rfc3261Validator::validate_message(&headers, "INVITE sip:bob@example.com SIP/2.0")
                .is_ok()
        );
    }

    #[test]
    fn test_multipart_parsing() {
        let content_type = "multipart/mixed;boundary=unique-boundary-1";
        let body = r#"--unique-boundary-1
Content-Type: application/sdp

v=0
o=- 0 0 IN IP4 0.0.0.0
s=-
c=IN IP4 0.0.0.0
t=0 0
m=audio 0 RTP/AVP 0

--unique-boundary-1
Content-Type: application/isup;base=itu-t92+
Content-Disposition: signal;handling=required

0A02830A
--unique-boundary-1--"#;

        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), content_type.to_string());

        let oli = extract_oli_info(&headers, Some(body));
        assert!(oli.is_some());
    }
}
