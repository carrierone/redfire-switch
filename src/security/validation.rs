//! Input validation for SIP messages and API requests
//!
//! This module provides comprehensive input validation to prevent
//! injection attacks, buffer overflows, and other security issues.

use super::{SecurityConfig, SecurityError};
use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;
use tracing::{debug, error, warn};

/// SIP message validator
pub struct SipMessageValidator {
    /// Maximum allowed message size
    max_message_size: usize,
    /// Valid SIP methods
    valid_methods: Vec<String>,
    /// Header validation patterns
    header_patterns: HashMap<String, Regex>,
}

impl SipMessageValidator {
    /// Create a new SIP message validator
    pub fn new(config: &SecurityConfig) -> Result<Self> {
        let mut header_patterns = HashMap::new();

        // Safe patterns for common SIP headers
        header_patterns.insert(
            "Call-ID".to_string(),
            Regex::new(r"^[a-zA-Z0-9@.\-_]{1,255}$")?,
        );
        header_patterns.insert("From".to_string(), Regex::new(r"^[^<>]{1,1024}$")?);
        header_patterns.insert("To".to_string(), Regex::new(r"^[^<>]{1,1024}$")?);
        header_patterns.insert(
            "Via".to_string(),
            Regex::new(r"^SIP/2\.0/[A-Z]{3,4}\s+[a-zA-Z0-9.:@\-_\s;=]+$")?,
        );

        Ok(Self {
            max_message_size: config.max_sip_message_size,
            valid_methods: vec![
                "INVITE".to_string(),
                "ACK".to_string(),
                "BYE".to_string(),
                "CANCEL".to_string(),
                "REGISTER".to_string(),
                "OPTIONS".to_string(),
                "INFO".to_string(),
                "PRACK".to_string(),
                "UPDATE".to_string(),
                "MESSAGE".to_string(),
                "NOTIFY".to_string(),
                "SUBSCRIBE".to_string(),
            ],
            header_patterns,
        })
    }

    /// Validate a SIP message
    pub fn validate_sip_message(&self, message: &[u8]) -> Result<(), SecurityError> {
        // Check message size
        if message.len() > self.max_message_size {
            error!("SIP message too large: {} bytes", message.len());
            return Err(SecurityError::RequestTooLarge(format!(
                "{} bytes",
                message.len()
            )));
        }

        // Convert to string safely
        let message_str = std::str::from_utf8(message)
            .map_err(|_| SecurityError::InvalidInput("Invalid UTF-8 encoding".to_string()))?;

        // Validate basic structure
        let lines: Vec<&str> = message_str.lines().collect();
        if lines.is_empty() {
            return Err(SecurityError::InvalidInput("Empty message".to_string()));
        }

        // Validate request/response line
        self.validate_start_line(lines[0])?;

        // Validate headers
        let mut header_end = 0;
        for (i, line) in lines.iter().enumerate().skip(1) {
            if line.is_empty() {
                header_end = i;
                break;
            }
            self.validate_header_line(line)?;
        }

        // Validate body if present
        if header_end > 0 && header_end + 1 < lines.len() {
            let body = &lines[header_end + 1..].join("\n");
            self.validate_message_body(body)?;
        }

        debug!("SIP message validation passed");
        Ok(())
    }

    /// Validate SIP start line (request or response)
    fn validate_start_line(&self, line: &str) -> Result<(), SecurityError> {
        if line.starts_with("SIP/2.0") {
            // Response line: SIP/2.0 <status-code> <reason-phrase>
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                return Err(SecurityError::InvalidInput(
                    "Invalid response line format".to_string(),
                ));
            }

            // Validate status code
            if let Err(_) = parts[1].parse::<u16>() {
                return Err(SecurityError::InvalidInput(
                    "Invalid status code".to_string(),
                ));
            }
        } else {
            // Request line: <method> <request-uri> SIP/2.0
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() != 3 {
                return Err(SecurityError::InvalidInput(
                    "Invalid request line format".to_string(),
                ));
            }

            // Validate method
            if !self.valid_methods.contains(&parts[0].to_string()) {
                warn!("Unknown SIP method: {}", parts[0]);
                return Err(SecurityError::InvalidInput(format!(
                    "Invalid SIP method: {}",
                    parts[0]
                )));
            }

            // Validate SIP version
            if parts[2] != "SIP/2.0" {
                return Err(SecurityError::InvalidInput(
                    "Invalid SIP version".to_string(),
                ));
            }

            // Basic URI validation
            if !parts[1].starts_with("sip:") && !parts[1].starts_with("sips:") {
                return Err(SecurityError::InvalidInput(
                    "Invalid SIP URI scheme".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Validate SIP header line
    fn validate_header_line(&self, line: &str) -> Result<(), SecurityError> {
        // Check for header injection
        if line.contains('\r') || line.contains('\n') {
            return Err(SecurityError::InvalidInput(
                "Header injection detected".to_string(),
            ));
        }

        // Parse header
        let header_parts: Vec<&str> = line.splitn(2, ':').collect();
        if header_parts.len() != 2 {
            return Err(SecurityError::InvalidInput(
                "Invalid header format".to_string(),
            ));
        }

        let header_name = header_parts[0].trim();
        let header_value = header_parts[1].trim();

        // Validate header name
        if header_name.is_empty() || header_name.len() > 100 {
            return Err(SecurityError::InvalidInput(
                "Invalid header name".to_string(),
            ));
        }

        // Validate header value length
        if header_value.len() > 4096 {
            return Err(SecurityError::InvalidInput(
                "Header value too long".to_string(),
            ));
        }

        // Apply specific header validation if available
        if let Some(pattern) = self.header_patterns.get(header_name) {
            if !pattern.is_match(header_value) {
                return Err(SecurityError::InvalidInput(format!(
                    "Invalid {} header format",
                    header_name
                )));
            }
        }

        Ok(())
    }

    /// Validate SIP message body
    fn validate_message_body(&self, body: &str) -> Result<(), SecurityError> {
        // Check body size
        if body.len() > 65536 {
            return Err(SecurityError::RequestTooLarge(format!(
                "{} bytes",
                body.len()
            )));
        }

        // Basic SDP validation if body appears to be SDP
        if body.starts_with("v=") {
            self.validate_sdp_body(body)?;
        }

        Ok(())
    }

    /// Validate SDP body
    fn validate_sdp_body(&self, sdp: &str) -> Result<(), SecurityError> {
        // Basic SDP line validation
        for line in sdp.lines() {
            if !line.is_empty() {
                // SDP lines should be in format "type=value"
                if line.len() < 3 || !line.chars().nth(1).map_or(false, |c| c == '=') {
                    return Err(SecurityError::InvalidInput(
                        "Invalid SDP format".to_string(),
                    ));
                }

                // Check for dangerous characters
                if line.contains("$(") || line.contains("`") || line.contains("${") {
                    return Err(SecurityError::InvalidInput(
                        "Dangerous characters in SDP".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }
}

/// Validate phone number format
pub fn validate_phone_number(number: &str) -> Result<String, SecurityError> {
    // Remove common formatting characters
    let cleaned = number
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '+')
        .collect::<String>();

    // Basic validation
    if cleaned.is_empty() || cleaned.len() > 15 {
        return Err(SecurityError::InvalidInput(
            "Invalid phone number format".to_string(),
        ));
    }

    // E.164 format validation (simplified)
    if cleaned.starts_with('+') {
        if cleaned.len() < 8 || cleaned.len() > 15 {
            return Err(SecurityError::InvalidInput(
                "Invalid E.164 number format".to_string(),
            ));
        }
    } else if cleaned.len() < 7 || cleaned.len() > 15 {
        return Err(SecurityError::InvalidInput(
            "Invalid phone number length".to_string(),
        ));
    }

    Ok(cleaned)
}

/// Validate IP address and check for private/reserved ranges
pub fn validate_ip_address(ip: &str) -> Result<std::net::IpAddr, SecurityError> {
    let addr: std::net::IpAddr = ip
        .parse()
        .map_err(|_| SecurityError::InvalidInput("Invalid IP address format".to_string()))?;

    // Check for dangerous IP ranges (simplified)
    match addr {
        std::net::IpAddr::V4(ipv4) => {
            let octets = ipv4.octets();
            // Block localhost (127.0.0.0/8)
            if octets[0] == 127 {
                warn!("Localhost IP address rejected: {}", ip);
                return Err(SecurityError::InvalidInput(
                    "Localhost IP not allowed".to_string(),
                ));
            }
            // Block multicast (224.0.0.0/4)
            if octets[0] >= 224 && octets[0] <= 239 {
                return Err(SecurityError::InvalidInput(
                    "Multicast IP not allowed".to_string(),
                ));
            }
        }
        std::net::IpAddr::V6(ipv6) => {
            // Block localhost (::1)
            if ipv6.is_loopback() {
                warn!("IPv6 localhost rejected: {}", ip);
                return Err(SecurityError::InvalidInput(
                    "Localhost IP not allowed".to_string(),
                ));
            }
        }
    }

    Ok(addr)
}

/// Sanitize string input for logging/storage
pub fn sanitize_string(input: &str, max_length: usize) -> String {
    input
        .chars()
        .take(max_length)
        .filter(|c| c.is_ascii() && !c.is_control())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phone_number_validation() {
        assert!(validate_phone_number("+1234567890").is_ok());
        assert!(validate_phone_number("1234567890").is_ok());
        assert!(validate_phone_number("123-456-7890").is_ok());

        assert!(validate_phone_number("").is_err());
        assert!(validate_phone_number("123").is_err());
        assert!(validate_phone_number("+1234567890123456789").is_err());
    }

    #[test]
    fn test_ip_validation() {
        assert!(validate_ip_address("192.168.1.1").is_ok());
        assert!(validate_ip_address("10.0.0.1").is_ok());

        assert!(validate_ip_address("127.0.0.1").is_err());
        assert!(validate_ip_address("224.0.0.1").is_err());
        assert!(validate_ip_address("invalid").is_err());
    }

    #[test]
    fn test_string_sanitization() {
        assert_eq!(sanitize_string("Hello\x00World\n", 20), "HelloWorld");
        assert_eq!(sanitize_string("Test", 2), "Te");
    }
}
