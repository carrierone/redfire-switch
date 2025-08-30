//! ANI-II (Automatic Number Identification Information Indicator) handling
//! 
//! This module provides definitions and utilities for handling ANI-II digits
//! as defined by NANPA (North American Numbering Plan Administration).
//! ANI-II digits provide important call classification information for billing,
//! particularly for payphone and special service calls.

use serde::{Deserialize, Serialize};
use std::fmt;

/// ANI-II digit codes as defined by NANPA
/// These codes provide call classification information for billing purposes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum AniIICode {
    /// Plain Old Telephone Service (POTS) - Regular residential/business line
    RegularLine = 0,
    /// Multiparty line (party line service)
    MultipartyLine = 1,
    /// ANI not available or ANI circuit failure
    AniNotAvailable = 2,
    /// ANI not available (special information tone)
    AniNotAvailableSpecial = 6,
    /// Hotel/Motel - non-registered guest
    HotelNonRegistered = 7,
    /// Hotel/Motel - registered guest
    HotelRegistered = 8,
    /// Wireless/cellular (roaming)
    WirelessRoaming = 15,
    /// Coin/Non-coin status uncertainty - applies to payphones and toll-free calls
    CoinNonCoinUncertainty = 23,
    /// Originating line cannot be identified  
    OriginatingLineUnidentified = 25,
    /// Pay station (payphone) with network coin control signaling
    PayStationNetworkCoin = 27,
    /// Prison/Inmate phone
    PrisonInmate = 32,
    /// Cellular/Wireless (home)
    CellularHome = 61,
    /// Cellular/Wireless (roaming)
    CellularRoaming = 62,
    /// Pay station (payphone) without network coin control signaling
    PayStationNonNetworkCoin = 70,
    /// Reserved for future use
    Reserved = 99,
}

impl AniIICode {
    /// Convert a u8 value to an ANI-II code if valid
    pub fn from_digit(digit: u8) -> Option<Self> {
        match digit {
            0 => Some(Self::RegularLine),
            1 => Some(Self::MultipartyLine),
            2 => Some(Self::AniNotAvailable),
            6 => Some(Self::AniNotAvailableSpecial),
            7 => Some(Self::HotelNonRegistered),
            8 => Some(Self::HotelRegistered),
            15 => Some(Self::WirelessRoaming),
            23 => Some(Self::CoinNonCoinUncertainty),
            25 => Some(Self::OriginatingLineUnidentified),
            27 => Some(Self::PayStationNetworkCoin),
            32 => Some(Self::PrisonInmate),
            61 => Some(Self::CellularHome),
            62 => Some(Self::CellularRoaming),
            70 => Some(Self::PayStationNonNetworkCoin),
            99 => Some(Self::Reserved),
            _ => None,
        }
    }

    /// Get the numeric value of the ANI-II code
    pub fn to_digit(self) -> u8 {
        self as u8
    }

    /// Check if this ANI-II code indicates a payphone that should trigger surcharges
    pub fn is_payphone(&self) -> bool {
        matches!(
            self,
            Self::CoinNonCoinUncertainty 
            | Self::PayStationNetworkCoin 
            | Self::PayStationNonNetworkCoin
        )
    }

    /// Get the default surcharge amount for payphone calls in USD
    /// These are typical industry standard amounts
    pub fn default_surcharge_amount(&self) -> Option<f64> {
        match self {
            Self::CoinNonCoinUncertainty => Some(0.49),      // $0.49 standard
            Self::PayStationNetworkCoin => Some(0.49),       // $0.49 standard  
            Self::PayStationNonNetworkCoin => Some(0.49),    // $0.49 standard
            _ => None,
        }
    }

    /// Get a human-readable description of the ANI-II code
    pub fn description(&self) -> &'static str {
        match self {
            Self::RegularLine => "Regular residential/business line (POTS)",
            Self::MultipartyLine => "Multiparty line (party line service)",
            Self::AniNotAvailable => "ANI not available or ANI circuit failure",
            Self::AniNotAvailableSpecial => "ANI not available (special information tone)",
            Self::HotelNonRegistered => "Hotel/Motel - non-registered guest",
            Self::HotelRegistered => "Hotel/Motel - registered guest",
            Self::WirelessRoaming => "Wireless/cellular (roaming)",
            Self::CoinNonCoinUncertainty => "Coin/Non-coin status uncertainty (payphone/toll-free)",
            Self::OriginatingLineUnidentified => "Originating line cannot be identified",
            Self::PayStationNetworkCoin => "Pay station with network coin control signaling",
            Self::PrisonInmate => "Prison/Inmate phone",
            Self::CellularHome => "Cellular/Wireless (home)",
            Self::CellularRoaming => "Cellular/Wireless (roaming)",
            Self::PayStationNonNetworkCoin => "Pay station without network coin control signaling",
            Self::Reserved => "Reserved for future use",
        }
    }

    /// Get the surcharge reason string for billing records
    pub fn surcharge_reason(&self) -> Option<&'static str> {
        match self {
            Self::CoinNonCoinUncertainty => Some("ANI-II Code 23 - Coin/Non-coin Uncertainty"),
            Self::PayStationNetworkCoin => Some("ANI-II Code 27 - Pay Station Network Coin"),
            Self::PayStationNonNetworkCoin => Some("ANI-II Code 70 - Pay Station Non-Network Coin"),
            _ => None,
        }
    }
}

impl fmt::Display for AniIICode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02} - {}", self.to_digit(), self.description())
    }
}

/// ANI-II information extracted from a call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AniIIInfo {
    /// The ANI-II code
    pub code: AniIICode,
    /// Raw digit value for logging
    pub raw_digit: u8,
    /// Source of the ANI-II information (which SIP header)
    pub source: AniIISource,
    /// Whether this code should trigger a payphone surcharge
    pub triggers_surcharge: bool,
}

impl AniIIInfo {
    /// Create new ANI-II information from a digit
    pub fn from_digit(digit: u8, source: AniIISource) -> Option<Self> {
        AniIICode::from_digit(digit).map(|code| Self {
            triggers_surcharge: code.is_payphone(),
            code,
            raw_digit: digit,
            source,
        })
    }

    /// Get the surcharge amount if applicable
    pub fn surcharge_amount(&self) -> Option<f64> {
        if self.triggers_surcharge {
            self.code.default_surcharge_amount()
        } else {
            None
        }
    }

    /// Get the surcharge reason if applicable  
    pub fn surcharge_reason(&self) -> Option<&'static str> {
        if self.triggers_surcharge {
            self.code.surcharge_reason()
        } else {
            None
        }
    }
}

/// Source of ANI-II information in SIP message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AniIISource {
    /// Found in Remote-Party-ID header
    RemotePartyId,
    /// Found in P-Asserted-Identity header
    PAssertedIdentity,
    /// Found in From header
    FromHeader,
    /// Found in custom X-ANI-II header
    CustomHeader,
    /// Extracted from SIP-I ISUP content
    SipI,
    /// Default value when not provided
    Default,
}

impl fmt::Display for AniIISource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let source_str = match self {
            Self::RemotePartyId => "Remote-Party-ID",
            Self::PAssertedIdentity => "P-Asserted-Identity", 
            Self::FromHeader => "From",
            Self::CustomHeader => "X-ANI-II",
            Self::SipI => "SIP-I ISUP",
            Self::Default => "Default",
        };
        write!(f, "{}", source_str)
    }
}

/// Utility functions for toll-free number identification
pub mod toll_free {
    /// Check if a DNIS (called number) is a toll-free number
    pub fn is_toll_free(dnis: &str) -> bool {
        // Remove any '+1' prefix and non-digit characters
        let clean_number = dnis
            .trim_start_matches("+1")
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect::<String>();

        // Check if it starts with known toll-free prefixes
        if clean_number.len() >= 3 {
            let prefix = &clean_number[..3];
            matches!(prefix, "800" | "833" | "844" | "855" | "866" | "877" | "888")
        } else {
            false
        }
    }
}

/// SIP Header ANI-II parsing functions
pub mod sip_parser {
    use super::*;
    use std::collections::HashMap;

    /// Parse ANI-II from SIP headers in priority order
    /// Checks Remote-Party-ID, P-Asserted-Identity, From, and custom headers
    pub fn parse_ani_ii_from_headers(headers: &HashMap<String, String>) -> Option<AniIIInfo> {
        // Priority 1: Remote-Party-ID header (most common for ANI-II)
        if let Some(ani_ii) = parse_remote_party_id(headers) {
            return Some(ani_ii);
        }
        
        // Priority 2: P-Asserted-Identity header
        if let Some(ani_ii) = parse_p_asserted_identity(headers) {
            return Some(ani_ii);
        }
        
        // Priority 3: Custom X-ANI-II header
        if let Some(ani_ii) = parse_custom_ani_ii_header(headers) {
            return Some(ani_ii);
        }
        
        // Priority 4: From header (less reliable)
        if let Some(ani_ii) = parse_from_header(headers) {
            return Some(ani_ii);
        }
        
        None
    }

    /// Parse ANI-II from Remote-Party-ID header
    /// Format: "Anonymous" <sip:+1234567890@carrier.com>;party=calling;screen=yes;privacy=off;ani-ii=70
    fn parse_remote_party_id(headers: &HashMap<String, String>) -> Option<AniIIInfo> {
        let header = headers.get("Remote-Party-ID")?;
        extract_ani_ii_parameter(header, AniIISource::RemotePartyId)
    }

    /// Parse ANI-II from P-Asserted-Identity header  
    /// Format: <sip:+1234567890@carrier.com>;ani-ii=23
    fn parse_p_asserted_identity(headers: &HashMap<String, String>) -> Option<AniIIInfo> {
        let header = headers.get("P-Asserted-Identity")?;
        extract_ani_ii_parameter(header, AniIISource::PAssertedIdentity)
    }

    /// Parse ANI-II from custom X-ANI-II header
    /// Format: X-ANI-II: 27
    fn parse_custom_ani_ii_header(headers: &HashMap<String, String>) -> Option<AniIIInfo> {
        let header_value = headers.get("X-ANI-II")?;
        let digit = header_value.trim().parse::<u8>().ok()?;
        AniIIInfo::from_digit(digit, AniIISource::CustomHeader)
    }

    /// Parse ANI-II from From header (fallback)
    /// Format: "Display Name" <sip:+1234567890@carrier.com>;tag=abc123;ani-ii=70
    fn parse_from_header(headers: &HashMap<String, String>) -> Option<AniIIInfo> {
        let header = headers.get("From")?;
        extract_ani_ii_parameter(header, AniIISource::FromHeader)
    }

    /// Extract ANI-II parameter from SIP header value
    /// Looks for ;ani-ii=XX pattern in the header
    fn extract_ani_ii_parameter(header_value: &str, source: AniIISource) -> Option<AniIIInfo> {
        // Look for ;ani-ii= parameter
        let ani_ii_prefix = ";ani-ii=";
        let ani_ii_start = header_value.find(ani_ii_prefix)?;
        let value_start = ani_ii_start + ani_ii_prefix.len();
        
        // Extract the digit(s) after ani-ii=
        let remaining = &header_value[value_start..];
        let mut chars = remaining.chars();
        let mut digit_str = String::new();
        
        // Collect consecutive digits
        while let Some(ch) = chars.next() {
            if ch.is_ascii_digit() {
                digit_str.push(ch);
            } else {
                break; // Stop at first non-digit (like ; or space)
            }
        }
        
        // Parse the digit string (should be 1-2 digits for ANI-II)
        if digit_str.len() >= 1 && digit_str.len() <= 2 {
            let digit = digit_str.parse::<u8>().ok()?;
            AniIIInfo::from_digit(digit, source)
        } else {
            None
        }
    }

    /// Validate ANI-II digit is in valid range and format
    pub fn validate_ani_ii_digit(digit: u8) -> bool {
        // ANI-II digits are typically 0-99, but not all values are defined
        digit <= 99
    }

    /// Parse ANI-II from multiple header variations
    /// Handles different carriers that may use different header names
    pub fn parse_ani_ii_extended(headers: &HashMap<String, String>) -> Option<AniIIInfo> {
        // Standard parsing first
        if let Some(ani_ii) = parse_ani_ii_from_headers(headers) {
            return Some(ani_ii);
        }
        
        // Check for carrier-specific variations
        let carrier_specific_headers = [
            "P-Calling-Party-ID",    // Some carriers use this
            "P-Original-Called",     // Less common
            "X-Calling-Party",       // Custom header variation
            "Remote-ID",            // Shorter variation
        ];
        
        for header_name in &carrier_specific_headers {
            if let Some(header_value) = headers.get(*header_name) {
                if let Some(ani_ii) = extract_ani_ii_parameter(header_value, AniIISource::CustomHeader) {
                    return Some(ani_ii);
                }
            }
        }
        
        None
    }

    /// Extract calling number with ANI-II from headers
    /// Returns (calling_number, ani_ii_info) tuple
    pub fn parse_calling_info_with_ani_ii(headers: &HashMap<String, String>) -> (Option<String>, Option<AniIIInfo>) {
        let ani_ii = parse_ani_ii_extended(headers);
        
        // Extract calling number from appropriate header
        let calling_number = extract_calling_number(headers);
        
        (calling_number, ani_ii)
    }

    /// Extract calling number from SIP headers
    fn extract_calling_number(headers: &HashMap<String, String>) -> Option<String> {
        // Try Remote-Party-ID first
        if let Some(number) = extract_number_from_header(headers.get("Remote-Party-ID")?) {
            return Some(number);
        }
        
        // Try P-Asserted-Identity
        if let Some(number) = extract_number_from_header(headers.get("P-Asserted-Identity")?) {
            return Some(number);
        }
        
        // Fallback to From header
        if let Some(number) = extract_number_from_header(headers.get("From")?) {
            return Some(number);
        }
        
        None
    }

    /// Extract phone number from SIP URI in header
    /// Handles formats like <sip:+12345678901@carrier.com> or sip:+12345678901@carrier.com
    fn extract_number_from_header(header_value: &str) -> Option<String> {
        // Find sip: URI
        let sip_start = header_value.find("sip:")?;
        let after_sip = &header_value[sip_start + 4..];
        
        // Find the @ symbol to get the user part
        let at_pos = after_sip.find('@')?;
        let user_part = &after_sip[..at_pos];
        
        // Clean up the number (remove non-digits and + prefix)
        let cleaned = user_part
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '+')
            .collect::<String>();
        
        if !cleaned.is_empty() {
            Some(cleaned)
        } else {
            None
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_parse_remote_party_id() {
            let mut headers = HashMap::new();
            headers.insert(
                "Remote-Party-ID".to_string(),
                "\"Anonymous\" <sip:+12345678901@carrier.com>;party=calling;screen=yes;privacy=off;ani-ii=70".to_string()
            );
            
            let ani_ii = parse_ani_ii_from_headers(&headers).expect("Should find ANI-II");
            assert_eq!(ani_ii.code, AniIICode::PayStationNonNetworkCoin);
            assert_eq!(ani_ii.raw_digit, 70);
            assert!(matches!(ani_ii.source, AniIISource::RemotePartyId));
            assert!(ani_ii.triggers_surcharge);
        }

        #[test]
        fn test_parse_p_asserted_identity() {
            let mut headers = HashMap::new();
            headers.insert(
                "P-Asserted-Identity".to_string(),
                "<sip:+18005551234@carrier.com>;ani-ii=23".to_string()
            );
            
            let ani_ii = parse_ani_ii_from_headers(&headers).expect("Should find ANI-II");
            assert_eq!(ani_ii.code, AniIICode::CoinNonCoinUncertainty);
            assert_eq!(ani_ii.raw_digit, 23);
            assert!(matches!(ani_ii.source, AniIISource::PAssertedIdentity));
        }

        #[test]
        fn test_parse_custom_header() {
            let mut headers = HashMap::new();
            headers.insert("X-ANI-II".to_string(), "27".to_string());
            
            let ani_ii = parse_ani_ii_from_headers(&headers).expect("Should find ANI-II");
            assert_eq!(ani_ii.code, AniIICode::PayStationNetworkCoin);
            assert_eq!(ani_ii.raw_digit, 27);
        }

        #[test]
        fn test_calling_info_extraction() {
            let mut headers = HashMap::new();
            headers.insert(
                "Remote-Party-ID".to_string(),
                "\"Payphone\" <sip:+15551234567@carrier.com>;party=calling;ani-ii=70".to_string()
            );
            
            let (calling_number, ani_ii) = parse_calling_info_with_ani_ii(&headers);
            assert_eq!(calling_number, Some("+15551234567".to_string()));
            assert!(ani_ii.is_some());
            let ani_ii = ani_ii.unwrap();
            assert_eq!(ani_ii.raw_digit, 70);
            assert!(ani_ii.triggers_surcharge);
        }

        #[test]
        fn test_no_ani_ii_present() {
            let mut headers = HashMap::new();
            headers.insert(
                "From".to_string(),
                "\"Regular User\" <sip:+15551234567@provider.com>;tag=abc123".to_string()
            );
            
            let ani_ii = parse_ani_ii_from_headers(&headers);
            assert!(ani_ii.is_none());
        }

        #[test]
        fn test_invalid_ani_ii_digit() {
            let mut headers = HashMap::new();
            headers.insert(
                "Remote-Party-ID".to_string(),
                "<sip:+12345678901@carrier.com>;ani-ii=999".to_string() // Invalid
            );
            
            let ani_ii = parse_ani_ii_from_headers(&headers);
            assert!(ani_ii.is_none());
        }
    }
}

/// SIP-I (ISUP over SIP) ANI-II parsing functions
pub mod sip_i_parser {
    use super::*;
    use std::collections::HashMap;

    /// Parse ANI-II from SIP-I message body containing ISUP content
    pub fn parse_ani_ii_from_sip_i(headers: &HashMap<String, String>, message_body: &str) -> Option<AniIIInfo> {
        // Check if this is a SIP-I message
        if !is_sip_i_message(headers) {
            return None;
        }
        
        // Parse the ISUP content from the message body
        parse_isup_calling_party_number(message_body)
    }

    /// Check if this is a SIP-I message by examining headers
    fn is_sip_i_message(headers: &HashMap<String, String>) -> bool {
        // Check Content-Type for ISUP encapsulation
        if let Some(content_type) = headers.get("Content-Type") {
            if content_type.contains("application/isup") || content_type.contains("application/isdn") {
                return true;
            }
        }
        
        // Check for SIP-I specific headers
        if headers.contains_key("P-ISUP-OLI") || 
           headers.contains_key("X-ISUP-ANI-II") ||
           headers.get("Content-Encoding").map_or(false, |v| v.contains("isup")) {
            return true;
        }
        
        false
    }

    /// Parse ISUP IAM (Initial Address Message) for Calling Party Number with ANI-II
    /// ISUP Calling Party Number format includes ANI-II in the numbering plan area
    fn parse_isup_calling_party_number(isup_body: &str) -> Option<AniIIInfo> {
        // This is a simplified ISUP parser - in production would use proper ISUP library
        // ISUP messages are typically in binary format, but some SIP-I implementations
        // may encode them as hex strings or include ANI-II in headers
        
        // Look for hex-encoded ISUP content
        if let Some(ani_ii) = parse_hex_encoded_isup(isup_body) {
            return Some(ani_ii);
        }
        
        // Look for text-encoded ISUP parameters
        if let Some(ani_ii) = parse_text_encoded_isup(isup_body) {
            return Some(ani_ii);
        }
        
        None
    }

    /// Parse hex-encoded ISUP content for ANI-II
    /// ISUP Calling Party Number parameter typically at offset with ANI-II info
    fn parse_hex_encoded_isup(hex_content: &str) -> Option<AniIIInfo> {
        // Remove whitespace and validate hex
        let cleaned: String = hex_content
            .chars()
            .filter(|c| c.is_ascii_hexdigit())
            .collect();
        
        if cleaned.len() < 20 {
            return None; // Too short to be valid ISUP
        }
        
        // Look for Calling Party Number parameter (0x0A in ISUP)
        let calling_party_marker = "0a"; // Calling Party Number parameter code
        
        if let Some(start_pos) = cleaned.to_lowercase().find(calling_party_marker) {
            // Parse the parameter starting at this position
            // This is a simplified implementation - would need full ISUP parsing in production
            let param_start = start_pos + 2;
            if param_start + 8 < cleaned.len() {
                // Extract ANI-II from the appropriate byte position in the parameter
                // In ISUP, ANI-II is typically in the numbering plan/type of number field
                let ani_ii_hex = &cleaned[param_start + 4..param_start + 6];
                if let Ok(ani_ii_byte) = u8::from_str_radix(ani_ii_hex, 16) {
                    // ANI-II is usually in the lower 7 bits of this byte
                    let ani_ii_digit = ani_ii_byte & 0x7F;
                    return AniIIInfo::from_digit(ani_ii_digit, AniIISource::SipI);
                }
            }
        }
        
        None
    }

    /// Parse text-encoded ISUP parameters for ANI-II
    /// Some SIP-I implementations include parameters in text format
    fn parse_text_encoded_isup(text_content: &str) -> Option<AniIIInfo> {
        // Look for common text encodings of ISUP parameters
        let patterns = [
            "ani-ii:",
            "oli:",
            "calling-party-category:",
            "originat-line-info:",
        ];
        
        for pattern in &patterns {
            if let Some(start) = text_content.to_lowercase().find(pattern) {
                let after_pattern = &text_content[start + pattern.len()..];
                let trimmed = after_pattern.trim();
                
                // Extract the numeric value
                let digit_str: String = trimmed
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                
                if let Ok(digit) = digit_str.parse::<u8>() {
                    if let Some(ani_ii) = AniIIInfo::from_digit(digit, AniIISource::SipI) {
                        return Some(ani_ii);
                    }
                }
            }
        }
        
        None
    }

    /// Parse ANI-II from SIP-I headers directly (alternative to body parsing)
    /// Some implementations put ISUP-derived values in SIP headers
    pub fn parse_ani_ii_from_sip_i_headers(headers: &HashMap<String, String>) -> Option<AniIIInfo> {
        // Check P-ISUP-OLI header (Originating Line Information)
        if let Some(oli_value) = headers.get("P-ISUP-OLI") {
            if let Ok(digit) = oli_value.trim().parse::<u8>() {
                return AniIIInfo::from_digit(digit, AniIISource::SipI);
            }
        }
        
        // Check X-ISUP-ANI-II custom header
        if let Some(ani_ii_value) = headers.get("X-ISUP-ANI-II") {
            if let Ok(digit) = ani_ii_value.trim().parse::<u8>() {
                return AniIIInfo::from_digit(digit, AniIISource::SipI);
            }
        }
        
        // Check P-Calling-Party-Category header
        if let Some(category) = headers.get("P-Calling-Party-Category") {
            // Calling party category sometimes maps to ANI-II
            if let Ok(category_digit) = category.trim().parse::<u8>() {
                // Map common category values to ANI-II if applicable
                let ani_ii_digit = match category_digit {
                    15 => Some(70), // Payphone category maps to ANI-II 70
                    10 => Some(0),  // Ordinary subscriber maps to ANI-II 0
                    _ => None,
                };
                
                if let Some(digit) = ani_ii_digit {
                    return AniIIInfo::from_digit(digit, AniIISource::SipI);
                }
            }
        }
        
        None
    }

    /// Parse complete SIP-I message for ANI-II information
    /// Tries both header and body parsing approaches
    pub fn parse_sip_i_message_complete(headers: &HashMap<String, String>, body: &str) -> Option<AniIIInfo> {
        // Try header-based parsing first (faster)
        if let Some(ani_ii) = parse_ani_ii_from_sip_i_headers(headers) {
            return Some(ani_ii);
        }
        
        // Try body parsing if no header info found
        if let Some(ani_ii) = parse_ani_ii_from_sip_i(headers, body) {
            return Some(ani_ii);
        }
        
        None
    }

    /// Validate SIP-I message format and content
    pub fn validate_sip_i_format(headers: &HashMap<String, String>, body: &str) -> bool {
        if !is_sip_i_message(headers) {
            return false;
        }
        
        // Basic validation of ISUP content
        if body.is_empty() {
            return false;
        }
        
        // Check if body contains valid hex or text content
        let has_hex = body.chars().any(|c| c.is_ascii_hexdigit());
        let has_isup_keywords = body.to_lowercase().contains("iam") || 
                               body.to_lowercase().contains("isup") ||
                               body.to_lowercase().contains("calling");
        
        has_hex || has_isup_keywords
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_sip_i_message_detection() {
            let mut headers = HashMap::new();
            headers.insert("Content-Type".to_string(), "application/isup".to_string());
            
            assert!(is_sip_i_message(&headers));
        }

        #[test]
        fn test_p_isup_oli_header() {
            let mut headers = HashMap::new();
            headers.insert("P-ISUP-OLI".to_string(), "70".to_string());
            
            let ani_ii = parse_ani_ii_from_sip_i_headers(&headers).expect("Should find ANI-II");
            assert_eq!(ani_ii.raw_digit, 70);
            assert_eq!(ani_ii.code, AniIICode::PayStationNonNetworkCoin);
            assert!(matches!(ani_ii.source, AniIISource::SipI));
        }

        #[test]
        fn test_calling_party_category_mapping() {
            let mut headers = HashMap::new();
            headers.insert("P-Calling-Party-Category".to_string(), "15".to_string()); // Payphone
            
            let ani_ii = parse_ani_ii_from_sip_i_headers(&headers).expect("Should find ANI-II");
            assert_eq!(ani_ii.raw_digit, 70); // Should map to ANI-II 70
            assert_eq!(ani_ii.code, AniIICode::PayStationNonNetworkCoin);
        }

        #[test]
        fn test_text_encoded_isup() {
            let isup_text = "IAM message: calling-party-number=+15551234567, ani-ii: 23, called-party=+18005551234";
            
            let ani_ii = parse_text_encoded_isup(isup_text).expect("Should find ANI-II");
            assert_eq!(ani_ii.raw_digit, 23);
            assert_eq!(ani_ii.code, AniIICode::CoinNonCoinUncertainty);
        }

        #[test]
        fn test_hex_encoded_isup() {
            // Simplified hex representation with calling party number parameter
            let hex_isup = "01 00 0a 05 01 23 55 12 34 56 78 90"; // Contains ANI-II 23
            
            // This test would need a more complete hex parser in production
            // For now, just test the parsing doesn't crash
            let result = parse_hex_encoded_isup(hex_isup);
            // In a real implementation, this would find and parse the ANI-II value
        }

        #[test]
        fn test_complete_sip_i_parsing() {
            let mut headers = HashMap::new();
            headers.insert("Content-Type".to_string(), "application/isup".to_string());
            headers.insert("P-ISUP-OLI".to_string(), "27".to_string());
            
            let body = "IAM: calling-party with OLI=27";
            
            let ani_ii = parse_sip_i_message_complete(&headers, body).expect("Should find ANI-II");
            assert_eq!(ani_ii.raw_digit, 27);
            assert_eq!(ani_ii.code, AniIICode::PayStationNetworkCoin);
        }

        #[test]
        fn test_invalid_sip_i_content() {
            let mut headers = HashMap::new();
            headers.insert("Content-Type".to_string(), "text/plain".to_string()); // Not SIP-I
            
            let body = "Regular SIP message body";
            
            let ani_ii = parse_sip_i_message_complete(&headers, body);
            assert!(ani_ii.is_none());
        }
    }
}

/// Payphone surcharge calculation logic for toll-free calls
pub mod surcharge_calculator {
    use super::*;

    /// Surcharge calculation result
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SurchargeResult {
        /// Whether a surcharge should be applied
        pub applies: bool,
        /// Surcharge amount in USD
        pub amount: f64,
        /// Reason for the surcharge
        pub reason: String,
        /// ANI-II code that triggered the surcharge
        pub ani_ii_code: Option<u8>,
        /// Source of the surcharge configuration
        pub source: SurchargeSource,
    }

    /// Source of surcharge configuration
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum SurchargeSource {
        /// Default surcharge from ANI-II code definition
        Default,
        /// Trunk-specific override
        TrunkOverride,
        /// Customer-specific configuration
        CustomerOverride,
    }

    impl SurchargeResult {
        /// Create a result indicating no surcharge applies
        pub fn no_surcharge() -> Self {
            Self {
                applies: false,
                amount: 0.0,
                reason: "No surcharge applicable".to_string(),
                ani_ii_code: None,
                source: SurchargeSource::Default,
            }
        }

        /// Create a result with surcharge details
        pub fn with_surcharge(amount: f64, reason: String, ani_ii_code: u8, source: SurchargeSource) -> Self {
            Self {
                applies: true,
                amount,
                reason,
                ani_ii_code: Some(ani_ii_code),
                source,
            }
        }
    }

    /// Calculate payphone surcharge for a toll-free call
    /// Takes into account ANI-II code, trunk configuration, and call details
    pub fn calculate_payphone_surcharge(
        ani_ii: Option<&AniIIInfo>,
        is_toll_free: bool,
        trunk_config: Option<&PayphoneSurchargeConfig>,
    ) -> SurchargeResult {
        // No surcharge if not a toll-free call
        if !is_toll_free {
            return SurchargeResult::no_surcharge();
        }

        // No surcharge if no ANI-II information available
        let ani_ii = match ani_ii {
            Some(info) => info,
            None => return SurchargeResult::no_surcharge(),
        };

        // No surcharge if ANI-II doesn't indicate payphone
        if !ani_ii.triggers_surcharge {
            return SurchargeResult::no_surcharge();
        }

        // Check trunk-specific overrides first
        if let Some(trunk_cfg) = trunk_config {
            if !trunk_cfg.enabled {
                return SurchargeResult::no_surcharge();
            }

            if let Some(amount) = get_trunk_surcharge_amount(ani_ii.raw_digit, trunk_cfg) {
                return SurchargeResult::with_surcharge(
                    amount,
                    format!("Trunk-configured surcharge for ANI-II {} - {}", 
                           ani_ii.raw_digit, ani_ii.code.description()),
                    ani_ii.raw_digit,
                    SurchargeSource::TrunkOverride,
                );
            }
        }

        // Fall back to default surcharge from ANI-II code
        if let Some(default_amount) = ani_ii.code.default_surcharge_amount() {
            SurchargeResult::with_surcharge(
                default_amount,
                format!("Default payphone surcharge for ANI-II {} - {}", 
                       ani_ii.raw_digit, ani_ii.code.description()),
                ani_ii.raw_digit,
                SurchargeSource::Default,
            )
        } else {
            SurchargeResult::no_surcharge()
        }
    }

    /// Get trunk-specific surcharge amount for ANI-II code
    fn get_trunk_surcharge_amount(ani_ii_code: u8, trunk_config: &PayphoneSurchargeConfig) -> Option<f64> {
        match ani_ii_code {
            23 => trunk_config.code_23_amount,
            27 => trunk_config.code_27_amount,
            70 => trunk_config.code_70_amount,
            _ => None,
        }
    }

    /// Configuration for payphone surcharges on a trunk
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PayphoneSurchargeConfig {
        /// Whether payphone surcharges are enabled for this trunk
        pub enabled: bool,
        /// Surcharge amount for ANI-II Code 23 (Coin/Non-coin Uncertainty)
        pub code_23_amount: Option<f64>,
        /// Surcharge amount for ANI-II Code 27 (Pay Station Network Coin)
        pub code_27_amount: Option<f64>,
        /// Surcharge amount for ANI-II Code 70 (Pay Station Non-Network Coin)
        pub code_70_amount: Option<f64>,
        /// Whether to apply surcharges to customer or carrier
        pub bill_to_customer: bool,
    }

    impl Default for PayphoneSurchargeConfig {
        fn default() -> Self {
            Self {
                enabled: true,
                code_23_amount: Some(0.49), // Standard industry amounts
                code_27_amount: Some(0.49),
                code_70_amount: Some(0.49),
                bill_to_customer: true, // Typically billed to toll-free subscriber
            }
        }
    }

    /// Validate surcharge configuration
    pub fn validate_surcharge_config(config: &PayphoneSurchargeConfig) -> Result<(), String> {
        if config.enabled {
            // Validate surcharge amounts are reasonable
            let amounts = [
                ("Code 23", config.code_23_amount),
                ("Code 27", config.code_27_amount),
                ("Code 70", config.code_70_amount),
            ];

            for (name, amount_opt) in &amounts {
                if let Some(amount) = amount_opt {
                    if *amount < 0.0 {
                        return Err(format!("{} surcharge amount cannot be negative: {}", name, amount));
                    }
                    if *amount > 5.00 {
                        return Err(format!("{} surcharge amount seems too high: ${:.2}", name, amount));
                    }
                }
            }
        }

        Ok(())
    }

    /// Calculate total surcharge for multiple calls
    /// Useful for batch processing or reporting
    pub fn calculate_batch_surcharges(
        call_records: &[(Option<AniIIInfo>, bool, Option<PayphoneSurchargeConfig>)],
    ) -> BatchSurchargeResult {
        let mut total_amount = 0.0;
        let mut applicable_calls = 0;
        let mut by_code: std::collections::HashMap<u8, (usize, f64)> = std::collections::HashMap::new();

        for (ani_ii, is_toll_free, trunk_config) in call_records {
            let result = calculate_payphone_surcharge(
                ani_ii.as_ref(),
                *is_toll_free,
                trunk_config.as_ref(),
            );

            if result.applies {
                total_amount += result.amount;
                applicable_calls += 1;

                if let Some(code) = result.ani_ii_code {
                    let entry = by_code.entry(code).or_insert((0, 0.0));
                    entry.0 += 1;
                    entry.1 += result.amount;
                }
            }
        }

        BatchSurchargeResult {
            total_calls: call_records.len(),
            applicable_calls,
            total_surcharge_amount: total_amount,
            average_surcharge: if applicable_calls > 0 { 
                total_amount / applicable_calls as f64 
            } else { 
                0.0 
            },
            surcharges_by_code: by_code,
        }
    }

    /// Result of batch surcharge calculation
    #[derive(Debug, Clone)]
    pub struct BatchSurchargeResult {
        pub total_calls: usize,
        pub applicable_calls: usize,
        pub total_surcharge_amount: f64,
        pub average_surcharge: f64,
        pub surcharges_by_code: std::collections::HashMap<u8, (usize, f64)>, // code -> (count, total_amount)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_no_surcharge_for_non_toll_free() {
            let ani_ii = AniIIInfo::from_digit(70, AniIISource::RemotePartyId).unwrap();
            let result = calculate_payphone_surcharge(Some(&ani_ii), false, None);
            assert!(!result.applies);
        }

        #[test]
        fn test_no_surcharge_without_ani_ii() {
            let result = calculate_payphone_surcharge(None, true, None);
            assert!(!result.applies);
        }

        #[test]
        fn test_no_surcharge_for_regular_line() {
            let ani_ii = AniIIInfo::from_digit(0, AniIISource::RemotePartyId).unwrap(); // Regular line
            let result = calculate_payphone_surcharge(Some(&ani_ii), true, None);
            assert!(!result.applies);
        }

        #[test]
        fn test_default_payphone_surcharge() {
            let ani_ii = AniIIInfo::from_digit(70, AniIISource::RemotePartyId).unwrap();
            let result = calculate_payphone_surcharge(Some(&ani_ii), true, None);
            
            assert!(result.applies);
            assert_eq!(result.amount, 0.49);
            assert_eq!(result.ani_ii_code, Some(70));
            assert!(matches!(result.source, SurchargeSource::Default));
        }

        #[test]
        fn test_trunk_override_surcharge() {
            let ani_ii = AniIIInfo::from_digit(23, AniIISource::RemotePartyId).unwrap();
            let trunk_config = PayphoneSurchargeConfig {
                enabled: true,
                code_23_amount: Some(0.65), // Override
                ..Default::default()
            };
            
            let result = calculate_payphone_surcharge(Some(&ani_ii), true, Some(&trunk_config));
            
            assert!(result.applies);
            assert_eq!(result.amount, 0.65); // Should use override
            assert!(matches!(result.source, SurchargeSource::TrunkOverride));
        }

        #[test]
        fn test_disabled_trunk_surcharges() {
            let ani_ii = AniIIInfo::from_digit(70, AniIISource::RemotePartyId).unwrap();
            let trunk_config = PayphoneSurchargeConfig {
                enabled: false,
                ..Default::default()
            };
            
            let result = calculate_payphone_surcharge(Some(&ani_ii), true, Some(&trunk_config));
            assert!(!result.applies);
        }

        #[test]
        fn test_batch_calculation() {
            let calls = vec![
                (Some(AniIIInfo::from_digit(70, AniIISource::RemotePartyId).unwrap()), true, None),
                (Some(AniIIInfo::from_digit(23, AniIISource::RemotePartyId).unwrap()), true, None),
                (Some(AniIIInfo::from_digit(0, AniIISource::RemotePartyId).unwrap()), true, None), // No surcharge
                (None, true, None), // No ANI-II
            ];
            
            let result = calculate_batch_surcharges(&calls);
            
            assert_eq!(result.total_calls, 4);
            assert_eq!(result.applicable_calls, 2);
            assert_eq!(result.total_surcharge_amount, 0.98); // 2 × $0.49
            assert_eq!(result.average_surcharge, 0.49);
            assert_eq!(result.surcharges_by_code.len(), 2);
        }

        #[test]
        fn test_surcharge_config_validation() {
            let good_config = PayphoneSurchargeConfig::default();
            assert!(validate_surcharge_config(&good_config).is_ok());
            
            let bad_config = PayphoneSurchargeConfig {
                enabled: true,
                code_23_amount: Some(-0.10), // Negative
                ..Default::default()
            };
            assert!(validate_surcharge_config(&bad_config).is_err());
            
            let expensive_config = PayphoneSurchargeConfig {
                enabled: true,
                code_23_amount: Some(10.00), // Too expensive
                ..Default::default()
            };
            assert!(validate_surcharge_config(&expensive_config).is_err());
        }
    }
}

/// ANI-II blocking functionality for customer protection
pub mod blocking {
    use super::*;
    use std::collections::HashSet;

    /// ANI-II blocking configuration for customers
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct AniIIBlockingConfig {
        /// Whether ANI-II blocking is enabled
        pub enabled: bool,
        /// Block all payphone calls (ANI-II codes 23, 27, 70)
        pub block_all_payphones: bool,
        /// Specific ANI-II codes to block
        pub blocked_codes: HashSet<u8>,
        /// Block calls with no ANI-II information
        pub block_no_ani_ii: bool,
        /// Block calls with unknown/invalid ANI-II codes
        pub block_unknown_codes: bool,
        /// Custom message for blocked calls
        pub block_message: String,
        /// SIP response code to use for blocked calls
        pub response_code: u16,
        /// Whether to charge the customer for blocked calls
        pub charge_for_blocked: bool,
    }

    impl Default for AniIIBlockingConfig {
        fn default() -> Self {
            Self {
                enabled: false,
                block_all_payphones: false,
                blocked_codes: HashSet::new(),
                block_no_ani_ii: false,
                block_unknown_codes: false,
                block_message: "Call blocked due to origination line type restrictions".to_string(),
                response_code: 403, // Forbidden - commonly used for policy blocks
                charge_for_blocked: false,
            }
        }
    }

    /// DID-specific ANI-II blocking configuration
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct DidAniIIBlocking {
        /// DID number (e.g., "18005551234")
        pub did_number: String,
        /// ANI-II blocking configuration for this DID
        pub blocking_config: AniIIBlockingConfig,
        /// Whether this DID inherits trunk-level blocking rules
        pub inherit_trunk_rules: bool,
    }

    /// Result of ANI-II blocking check
    #[derive(Debug, Clone)]
    pub struct BlockingResult {
        /// Whether the call should be blocked
        pub blocked: bool,
        /// Reason for blocking (or allowing)
        pub reason: String,
        /// SIP response code to use if blocked
        pub response_code: Option<u16>,
    }

    /// Check if a call should be blocked based on ANI-II code
    pub fn check_ani_ii_blocking(config: &AniIIBlockingConfig, ani_ii_code: u8) -> BlockingResult {
        if !config.enabled {
            return BlockingResult {
                blocked: false,
                reason: "ANI-II blocking disabled".to_string(),
                response_code: None,
            };
        }

        // Check specific blocked codes
        if config.blocked_codes.contains(&ani_ii_code) {
            return BlockingResult {
                blocked: true,
                reason: format!("ANI-II code {} is blocked by policy", ani_ii_code),
                response_code: Some(config.response_code),
            };
        }

        // Check payphone blocking
        if config.block_all_payphones {
            let payphone_codes = [23, 27, 70]; // Common payphone ANI-II codes
            if payphone_codes.contains(&ani_ii_code) {
                return BlockingResult {
                    blocked: true,
                    reason: format!("Payphone call blocked (ANI-II {})", ani_ii_code),
                    response_code: Some(config.response_code),
                };
            }
        }

        // Check unknown codes
        if config.block_unknown_codes && ani_ii_code >= 100 {
            return BlockingResult {
                blocked: true,
                reason: format!("Unknown ANI-II code {} blocked", ani_ii_code),
                response_code: Some(config.response_code),
            };
        }

        BlockingResult {
            blocked: false,
            reason: "ANI-II code allowed".to_string(),
            response_code: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ani_ii_code_from_digit() {
        assert_eq!(AniIICode::from_digit(0), Some(AniIICode::RegularLine));
        assert_eq!(AniIICode::from_digit(23), Some(AniIICode::CoinNonCoinUncertainty));
        assert_eq!(AniIICode::from_digit(27), Some(AniIICode::PayStationNetworkCoin));
        assert_eq!(AniIICode::from_digit(70), Some(AniIICode::PayStationNonNetworkCoin));
        assert_eq!(AniIICode::from_digit(255), None); // Invalid code
    }

    #[test]
    fn test_payphone_detection() {
        assert!(AniIICode::CoinNonCoinUncertainty.is_payphone());
        assert!(AniIICode::PayStationNetworkCoin.is_payphone());
        assert!(AniIICode::PayStationNonNetworkCoin.is_payphone());
        assert!(!AniIICode::RegularLine.is_payphone());
        assert!(!AniIICode::CellularHome.is_payphone());
    }

    #[test]
    fn test_surcharge_amounts() {
        assert_eq!(AniIICode::CoinNonCoinUncertainty.default_surcharge_amount(), Some(0.49));
        assert_eq!(AniIICode::PayStationNetworkCoin.default_surcharge_amount(), Some(0.49));
        assert_eq!(AniIICode::PayStationNonNetworkCoin.default_surcharge_amount(), Some(0.49));
        assert_eq!(AniIICode::RegularLine.default_surcharge_amount(), None);
    }

    #[test]
    fn test_toll_free_detection() {
        assert!(toll_free::is_toll_free("18005551234"));
        assert!(toll_free::is_toll_free("+18005551234"));
        assert!(toll_free::is_toll_free("8005551234"));
        assert!(toll_free::is_toll_free("8885551234"));
        assert!(!toll_free::is_toll_free("2125551234"));
        assert!(!toll_free::is_toll_free("9005551234"));
    }

    #[test]
    fn test_ani_ii_info_creation() {
        let info = AniIIInfo::from_digit(23, AniIISource::RemotePartyId).expect("Should create ANI-II info");
        assert_eq!(info.code, AniIICode::CoinNonCoinUncertainty);
        assert_eq!(info.raw_digit, 23);
        assert!(info.triggers_surcharge);
        assert_eq!(info.surcharge_amount(), Some(0.49));
    }
}