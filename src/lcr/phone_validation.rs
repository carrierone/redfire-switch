use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

/// Phone number validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoneValidationConfig {
    /// Enable phone number validation for international routing
    pub enabled: bool,
    /// Require strict validation (reject invalid numbers)
    pub strict_validation: bool,
    /// Default region code to assume for numbers without country code
    pub default_region: String,
    /// Whether to use libphonenumber for country detection
    pub use_country_detection: bool,
}

impl Default for PhoneValidationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            strict_validation: false,
            default_region: "US".to_string(),
            use_country_detection: true,
        }
    }
}

/// Phone number validation result
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Original phone number
    pub original: String,
    /// Whether the number is valid
    pub is_valid: bool,
    /// Detected country code (ISO 2-letter)
    pub country_code: Option<String>,
    /// Detected region code used by libphonenumber
    pub region_code: Option<String>,
    /// Phone number type (mobile, fixed-line, etc.)
    pub number_type: Option<String>,
    /// Formatted number in E164 format
    pub e164_format: Option<String>,
    /// Formatted number in international format
    pub international_format: Option<String>,
    /// Validation error message if any
    pub error: Option<String>,
}

/// Phone number validator - simplified version for now
pub struct PhoneValidator {
    config: PhoneValidationConfig,
}

impl PhoneValidator {
    /// Create new phone validator with configuration
    pub fn new(config: PhoneValidationConfig) -> Self {
        Self { config }
    }

    /// Validate and parse a phone number
    /// This is a simplified implementation that focuses on basic validation
    /// A production version would integrate with rlibphonenumber properly
    pub fn validate(&self, number: &str) -> ValidationResult {
        if !self.config.enabled {
            return ValidationResult {
                original: number.to_string(),
                is_valid: true, // Pass through if validation disabled
                country_code: None,
                region_code: None,
                number_type: None,
                e164_format: None,
                international_format: None,
                error: None,
            };
        }

        debug!("Validating phone number: {}", number);

        // Basic validation logic
        let normalized = self.normalize_number(number);
        let is_valid = self.basic_validate(&normalized);
        
        let country_code = if self.config.use_country_detection {
            self.detect_country_code(&normalized)
        } else {
            None
        };

        if self.config.strict_validation && !is_valid {
            return ValidationResult {
                original: number.to_string(),
                is_valid: false,
                country_code: country_code.clone(),
                region_code: country_code.clone(),
                number_type: None,
                e164_format: None,
                international_format: None,
                error: Some("Number failed strict validation".to_string()),
            };
        }

        ValidationResult {
            original: number.to_string(),
            is_valid,
            country_code: country_code.clone(),
            region_code: country_code,
            number_type: Some("unknown".to_string()),
            e164_format: Some(normalized.clone()),
            international_format: Some(self.format_international(&normalized)),
            error: None,
        }
    }

    /// Get country code from phone number
    pub fn get_country_code(&self, number: &str) -> Option<String> {
        if !self.config.use_country_detection {
            return None;
        }
        
        let normalized = self.normalize_number(number);
        self.detect_country_code(&normalized)
    }

    /// Check if number is likely international (not in default region)
    pub fn is_international(&self, number: &str) -> bool {
        let normalized = self.normalize_number(number);
        
        // Check for international prefixes
        normalized.starts_with("00") || 
        number.starts_with('+') || 
        normalized.starts_with("011") ||
        self.detect_country_code(&normalized).map_or(false, |cc| cc != self.config.default_region)
    }

    /// Normalize phone number to digits only, handling common prefixes
    fn normalize_number(&self, number: &str) -> String {
        let digits: String = number.chars().filter(|c| c.is_digit(10)).collect();
        
        // Remove international access codes
        if digits.starts_with("011") && digits.len() > 3 {
            // US international prefix
            format!("+{}", &digits[3..])
        } else if digits.starts_with("00") && digits.len() > 2 {
            // International prefix (most countries)
            format!("+{}", &digits[2..])
        } else if number.starts_with('+') {
            // Plus format - keep as is but ensure digits only after +
            format!("+{}", digits)
        } else {
            // Assume domestic number or already normalized
            digits
        }
    }

    /// Basic phone number validation
    fn basic_validate(&self, normalized: &str) -> bool {
        if normalized.is_empty() {
            return false;
        }
        
        let digits = if normalized.starts_with('+') {
            &normalized[1..]
        } else {
            normalized
        };
        
        // Basic length check (5-15 digits for international numbers)
        let len = digits.len();
        if len < 5 || len > 15 {
            return false;
        }
        
        // Must be all digits
        digits.chars().all(|c| c.is_digit(10))
    }

    /// Simple country detection based on common prefixes
    fn detect_country_code(&self, normalized: &str) -> Option<String> {
        if !normalized.starts_with('+') {
            // Domestic number - use default region
            return Some(self.config.default_region.clone());
        }
        
        let digits = &normalized[1..];
        
        // Common country code mappings (simplified)
        if digits.starts_with("1") {
            Some("US".to_string())
        } else if digits.starts_with("44") {
            Some("GB".to_string())
        } else if digits.starts_with("49") {
            Some("DE".to_string())
        } else if digits.starts_with("33") {
            Some("FR".to_string())
        } else if digits.starts_with("39") {
            Some("IT".to_string())
        } else if digits.starts_with("34") {
            Some("ES".to_string())
        } else if digits.starts_with("31") {
            Some("NL".to_string())
        } else if digits.starts_with("32") {
            Some("BE".to_string())
        } else if digits.starts_with("43") {
            Some("AT".to_string())
        } else if digits.starts_with("41") {
            Some("CH".to_string())
        } else if digits.starts_with("46") {
            Some("SE".to_string())
        } else if digits.starts_with("47") {
            Some("NO".to_string())
        } else if digits.starts_with("45") {
            Some("DK".to_string())
        } else if digits.starts_with("358") {
            Some("FI".to_string())
        } else {
            // Unknown country - return None or "XX"
            None
        }
    }

    /// Format number for international display
    fn format_international(&self, normalized: &str) -> String {
        if normalized.starts_with('+') {
            normalized.to_string()
        } else if normalized.len() == 10 && self.config.default_region == "US" {
            // Format US number
            format!("+1 {} {} {}", &normalized[0..3], &normalized[3..6], &normalized[6..])
        } else {
            format!("+{}", normalized)
        }
    }

    /// Normalize phone number to E164 format if possible
    pub fn normalize_to_e164(&self, number: &str) -> Result<String> {
        let normalized = self.normalize_number(number);
        if self.basic_validate(&normalized) {
            Ok(if normalized.starts_with('+') {
                normalized
            } else {
                format!("+{}", normalized)
            })
        } else {
            Err(anyhow!("Invalid phone number: {}", number))
        }
    }

    /// Get international format of phone number
    pub fn format_international_public(&self, number: &str) -> Result<String> {
        let normalized = self.normalize_number(number);
        Ok(self.format_international(&normalized))
    }

    /// Extract country prefix from phone number
    pub fn extract_country_prefix(&self, number: &str) -> Option<String> {
        let normalized = self.normalize_number(number);
        if normalized.starts_with('+') {
            // Extract country code (this is simplified - real implementation would be more complex)
            let digits = &normalized[1..];
            if digits.starts_with("1") {
                Some("1".to_string())
            } else if digits.len() >= 2 {
                Some(digits[0..2].to_string())
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phone_validation() {
        let config = PhoneValidationConfig::default();
        let validator = PhoneValidator::new(config);

        // Test US number
        let result = validator.validate("+1-555-123-4567");
        assert!(result.is_valid);
        assert_eq!(result.country_code, Some("US".to_string()));

        // Test UK number
        let result = validator.validate("+44 20 7946 0958");
        assert!(result.is_valid);
        assert_eq!(result.country_code, Some("GB".to_string()));

        // Test invalid number with strict validation
        let mut config = PhoneValidationConfig::default();
        config.strict_validation = true;
        let validator = PhoneValidator::new(config);
        
        let result = validator.validate("invalid-number");
        assert!(!result.is_valid);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_country_detection() {
        let config = PhoneValidationConfig::default();
        let validator = PhoneValidator::new(config);

        assert_eq!(validator.get_country_code("+1-555-123-4567"), Some("US".to_string()));
        assert_eq!(validator.get_country_code("+44 20 7946 0958"), Some("GB".to_string()));
        assert_eq!(validator.get_country_code("+49 30 12345678"), Some("DE".to_string()));
    }

    #[test]
    fn test_international_detection() {
        let config = PhoneValidationConfig::default();
        let validator = PhoneValidator::new(config);

        assert!(!validator.is_international("555-123-4567")); // US number without country code
        assert!(validator.is_international("+44 20 7946 0958")); // UK number
        assert!(validator.is_international("011441234567890")); // International prefix
    }

    #[test]
    fn test_disabled_validation() {
        let mut config = PhoneValidationConfig::default();
        config.enabled = false;
        let validator = PhoneValidator::new(config);

        let result = validator.validate("invalid-number");
        assert!(result.is_valid); // Should pass through when disabled
        assert!(result.error.is_none());
    }
}