/*
 * Security Utilities for B2BUA Implementation
 * Provides secure input validation, sanitization, and logging functions
 */

use anyhow::{anyhow, Result};
use regex::Regex;
use std::sync::OnceLock;
use tracing::{error, warn};

// Security constants
pub const MAX_SIP_MESSAGE_SIZE: usize = 65536; // 64KB max SIP message
pub const MAX_HEADER_LENGTH: usize = 2048; // 2KB max header
pub const MAX_PHONE_NUMBER_LENGTH: usize = 20; // E.164 max length
pub const MAX_ISUP_SIZE: usize = 4096; // 4KB max ISUP data
pub const MAX_HEX_INPUT_SIZE: usize = MAX_ISUP_SIZE * 2; // Hex is 2x binary size
pub const MAX_JWT_SIZE: usize = 4096; // 4KB max JWT token

// Static regex patterns for validation
static SAFE_LOGGING_REGEX: OnceLock<Regex> = OnceLock::new();
static PHONE_NUMBER_REGEX: OnceLock<Regex> = OnceLock::new();
static SIP_URI_REGEX: OnceLock<Regex> = OnceLock::new();
static HEX_REGEX: OnceLock<Regex> = OnceLock::new();
static HEADER_NAME_REGEX: OnceLock<Regex> = OnceLock::new();

/// Initialize security utilities
pub fn init_security() {
    // Pre-compile all regex patterns for performance
    SAFE_LOGGING_REGEX.get_or_init(|| {
        // Allow alphanumeric, space, and common safe punctuation
        // Explicitly excludes control characters like \r \n \t
        Regex::new(r"[^a-zA-Z0-9+\-().\x20@:/_]").expect("Invalid safe logging regex")
    });

    PHONE_NUMBER_REGEX
        .get_or_init(|| Regex::new(r"^\+?[1-9]\d{7,18}$").expect("Invalid phone number regex"));

    SIP_URI_REGEX.get_or_init(|| {
        Regex::new(r"^sip:[a-zA-Z0-9+\-._]+@[a-zA-Z0-9.\-]+$").expect("Invalid SIP URI regex")
    });

    HEX_REGEX.get_or_init(|| Regex::new(r"^[0-9a-fA-F\s]*$").expect("Invalid hex regex"));

    HEADER_NAME_REGEX.get_or_init(|| {
        Regex::new(r"^[a-zA-Z][a-zA-Z0-9\-]*$").expect("Invalid header name regex")
    });
}

/// Sanitize input for safe logging (prevent log injection)
pub fn sanitize_for_logging(input: &str) -> String {
    // Initialize if needed
    init_security();

    // FIXED: Handle missing regex gracefully instead of panicking
    let regex = match SAFE_LOGGING_REGEX.get() {
        Some(regex) => regex,
        None => {
            // Fallback: basic character filtering if regex not initialized
            let truncated = if input.len() > 256 {
                &input[..256]
            } else {
                input
            };
            let result: String = truncated
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || " .-+()@:/_".contains(c) {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            return result;
        }
    };

    // Truncate overly long inputs
    let truncated = if input.len() > 256 {
        warn!("Truncating oversized input for logging");
        &input[..256]
    } else {
        input
    };

    // Replace unsafe characters with underscores
    regex.replace_all(truncated, "_").to_string()
}

/// Mask phone number for secure logging
pub fn mask_phone_number(number: &str) -> String {
    if number.is_empty() {
        return "****".to_string();
    }

    // Remove non-numeric characters for processing
    let digits: String = number.chars().filter(|c| c.is_ascii_digit()).collect();

    if digits.len() > 4 {
        format!("{}****{}", &digits[..2], &digits[digits.len() - 2..])
    } else if digits.len() > 0 {
        "****".to_string()
    } else {
        "INVALID".to_string()
    }
}

/// Validate SIP message size
pub fn validate_message_size(message: &str) -> Result<()> {
    if message.len() > MAX_SIP_MESSAGE_SIZE {
        error!("SIP message exceeds maximum size: {} bytes", message.len());
        return Err(anyhow!("SIP message exceeds maximum allowed size"));
    }
    Ok(())
}

/// Validate SIP header size and content
pub fn validate_header(header_name: &str, header_value: &str) -> Result<()> {
    // Ensure the security regexes are compiled (idempotent).
    init_security();
    // Validate header name format
    let name_regex = HEADER_NAME_REGEX.get().expect("Security not initialized");
    if !name_regex.is_match(header_name) {
        return Err(anyhow!("Invalid SIP header name format"));
    }

    // Validate header size
    if header_value.len() > MAX_HEADER_LENGTH {
        error!(
            "Header '{}' exceeds maximum length: {} bytes",
            sanitize_for_logging(header_name),
            header_value.len()
        );
        return Err(anyhow!("SIP header exceeds maximum allowed size"));
    }

    // Check for header injection attacks (CRLF injection)
    if header_value.contains('\r') || header_value.contains('\n') {
        error!(
            "Potential header injection attack detected in header '{}'",
            sanitize_for_logging(header_name)
        );
        return Err(anyhow!("Invalid characters in SIP header"));
    }

    Ok(())
}

/// Validate and sanitize phone number
pub fn validate_phone_number(number: &str) -> Result<String> {
    // Ensure the security regexes are compiled (idempotent).
    init_security();
    if number.len() > MAX_PHONE_NUMBER_LENGTH {
        return Err(anyhow!("Phone number exceeds maximum length"));
    }

    let regex = PHONE_NUMBER_REGEX.get().expect("Security not initialized");

    // Clean the number (remove common formatting)
    let cleaned = number
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '+')
        .collect::<String>();

    if !regex.is_match(&cleaned) {
        return Err(anyhow!("Invalid phone number format"));
    }

    Ok(cleaned)
}

/// Validate SIP URI format
pub fn validate_sip_uri(uri: &str) -> Result<()> {
    // Ensure the security regexes are compiled (idempotent).
    init_security();
    let regex = SIP_URI_REGEX.get().expect("Security not initialized");

    if uri.len() > 256 {
        return Err(anyhow!("SIP URI exceeds maximum length"));
    }

    if !regex.is_match(uri) {
        return Err(anyhow!("Invalid SIP URI format"));
    }

    Ok(())
}

/// Secure hex string validation and decoding
pub fn validate_and_decode_hex(hex_input: &str) -> Result<Vec<u8>> {
    // Ensure the security regexes are compiled (idempotent).
    init_security();
    // Input size validation
    if hex_input.len() > MAX_HEX_INPUT_SIZE {
        error!("Hex input exceeds maximum size: {} chars", hex_input.len());
        return Err(anyhow!("Hex input exceeds maximum allowed size"));
    }

    // Format validation
    let hex_regex = HEX_REGEX.get().expect("Security not initialized");
    if !hex_regex.is_match(hex_input) {
        return Err(anyhow!("Invalid hex format - contains illegal characters"));
    }

    // Clean whitespace
    let cleaned = hex_input
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();

    // Validate even length
    if cleaned.len() % 2 != 0 {
        return Err(anyhow!("Invalid hex data - odd number of characters"));
    }

    // Decode with size check
    match hex::decode(&cleaned) {
        Ok(data) => {
            if data.len() > MAX_ISUP_SIZE {
                error!(
                    "Decoded hex data exceeds maximum size: {} bytes",
                    data.len()
                );
                return Err(anyhow!("Decoded data exceeds maximum allowed size"));
            }
            Ok(data)
        }
        Err(_) => {
            // Don't expose the actual hex content in error message
            error!("Failed to decode hex data");
            Err(anyhow!("Invalid hex encoding"))
        }
    }
}

/// Validate JWT token format and size
pub fn validate_jwt_token(token: &str) -> Result<()> {
    if token.len() > MAX_JWT_SIZE {
        error!("JWT token exceeds maximum size: {} chars", token.len());
        return Err(anyhow!("JWT token exceeds maximum allowed size"));
    }

    // Validate JWT structure (header.payload.signature)
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(anyhow!("Invalid JWT format - must have 3 parts"));
    }

    // Validate each part is valid base64
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            return Err(anyhow!("Invalid JWT format - empty part {}", i));
        }

        // Basic base64 character validation
        if !part
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(anyhow!(
                "Invalid JWT format - illegal characters in part {}",
                i
            ));
        }
    }

    Ok(())
}

/// Safe string slicing with bounds checking
pub fn safe_slice(input: &str, start: usize, end: usize) -> Result<&str> {
    if start > input.len() {
        return Err(anyhow!(
            "Start index {} exceeds string length {}",
            start,
            input.len()
        ));
    }

    if end > input.len() {
        return Err(anyhow!(
            "End index {} exceeds string length {}",
            end,
            input.len()
        ));
    }

    if start > end {
        return Err(anyhow!(
            "Start index {} is greater than end index {}",
            start,
            end
        ));
    }

    Ok(&input[start..end])
}

/// Rate limiting structure for DoS protection
#[derive(Debug)]
pub struct RateLimiter {
    max_requests: u32,
    window_seconds: u64,
    requests: std::collections::HashMap<std::net::IpAddr, (u32, std::time::Instant)>,
}

impl RateLimiter {
    pub fn new(max_requests: u32, window_seconds: u64) -> Self {
        Self {
            max_requests,
            window_seconds,
            requests: std::collections::HashMap::new(),
        }
    }

    pub fn check_rate_limit(&mut self, ip: std::net::IpAddr) -> bool {
        let now = std::time::Instant::now();
        let window_duration = std::time::Duration::from_secs(self.window_seconds);

        // Clean old entries
        self.requests
            .retain(|_, (_, timestamp)| now.duration_since(*timestamp) < window_duration);

        // Check current IP
        match self.requests.get_mut(&ip) {
            Some((count, timestamp)) => {
                if now.duration_since(*timestamp) < window_duration {
                    if *count >= self.max_requests {
                        warn!("Rate limit exceeded for IP: {}", ip);
                        return false;
                    }
                    *count += 1;
                } else {
                    *count = 1;
                    *timestamp = now;
                }
            }
            None => {
                self.requests.insert(ip, (1, now));
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_for_logging() {
        // Don't call init_security() here - let sanitize_for_logging do it
        // This ensures we get a fresh regex in each test run

        assert_eq!(sanitize_for_logging("normal@test.com"), "normal@test.com");
        let result = sanitize_for_logging("evil\r\ninjection");
        assert_eq!(result, "evil__injection");
        assert_eq!(sanitize_for_logging("script<>alert"), "script__alert");
    }

    #[test]
    fn test_mask_phone_number() {
        assert_eq!(mask_phone_number("+15551234567"), "15****67");
        assert_eq!(mask_phone_number("123"), "****");
        assert_eq!(mask_phone_number(""), "****");
    }

    #[test]
    fn test_validate_phone_number() {
        init_security();

        assert!(validate_phone_number("+15551234567").is_ok());
        assert!(validate_phone_number("15551234567").is_ok());
        assert!(validate_phone_number("invalid").is_err());
        assert!(validate_phone_number("").is_err());
    }

    #[test]
    fn test_validate_hex() {
        init_security();

        assert!(validate_and_decode_hex("48656c6c6f").is_ok());
        assert!(validate_and_decode_hex("48 65 6c 6c 6f").is_ok());
        assert!(validate_and_decode_hex("invalid").is_err());
        assert!(validate_and_decode_hex("4865g").is_err());
    }

    #[test]
    fn test_safe_slice() {
        assert!(safe_slice("hello", 0, 3).is_ok());
        assert_eq!(safe_slice("hello", 0, 3).unwrap(), "hel");
        assert!(safe_slice("hello", 0, 10).is_err());
        assert!(safe_slice("hello", 3, 1).is_err());
    }
}
