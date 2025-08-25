#[cfg(test)]
mod phone_validation_tests {
    use super::super::phone_validation::*;

    #[test]
    fn test_phone_validation_config_default() {
        let config = PhoneValidationConfig::default();
        assert!(config.enabled);
        assert!(!config.strict_validation);
        assert_eq!(config.default_region, "US");
        assert!(config.use_country_detection);
    }

    #[test]
    fn test_basic_phone_validation() {
        let config = PhoneValidationConfig::default();
        let validator = PhoneValidator::new(config);

        // Valid US numbers
        let result = validator.validate("+1-555-123-4567");
        assert!(result.is_valid);
        assert_eq!(result.country_code, Some("US".to_string()));
        assert!(result.e164_format.is_some());
        assert!(result.error.is_none());

        // Valid UK number
        let result = validator.validate("+44 20 7946 0958");
        assert!(result.is_valid);
        assert_eq!(result.country_code, Some("GB".to_string()));

        // Valid German number
        let result = validator.validate("+49 30 12345678");
        assert!(result.is_valid);
        assert_eq!(result.country_code, Some("DE".to_string()));

        // Valid French number
        let result = validator.validate("+33 1 42 86 83 26");
        assert!(result.is_valid);
        assert_eq!(result.country_code, Some("FR".to_string()));
    }

    #[test]
    fn test_domestic_number_handling() {
        let config = PhoneValidationConfig::default();
        let validator = PhoneValidator::new(config);

        // US domestic number
        let result = validator.validate("555-123-4567");
        assert!(result.is_valid);
        assert_eq!(result.country_code, Some("US".to_string()));
        
        // US number with area code
        let result = validator.validate("(212) 555-1234");
        assert!(result.is_valid);
        assert_eq!(result.country_code, Some("US".to_string()));
    }

    #[test]
    fn test_international_prefixes() {
        let config = PhoneValidationConfig::default();
        let validator = PhoneValidator::new(config);

        // US international prefix (011)
        let result = validator.validate("011441234567890");
        assert!(result.is_valid);
        assert_eq!(result.country_code, Some("GB".to_string()));

        // European international prefix (00)
        let result = validator.validate("0014155551234");
        assert!(result.is_valid);
        assert_eq!(result.country_code, Some("US".to_string()));

        // Plus format
        let result = validator.validate("+49 30 12345678");
        assert!(result.is_valid);
        assert_eq!(result.country_code, Some("DE".to_string()));
    }

    #[test]
    fn test_strict_validation_mode() {
        let mut config = PhoneValidationConfig::default();
        config.strict_validation = true;
        let validator = PhoneValidator::new(config);

        // Valid number should pass
        let result = validator.validate("+1-555-123-4567");
        assert!(result.is_valid);
        assert!(result.error.is_none());

        // Invalid number should fail
        let result = validator.validate("invalid-number");
        assert!(!result.is_valid);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("strict validation"));

        // Too short number should fail
        let result = validator.validate("123");
        assert!(!result.is_valid);
        assert!(result.error.is_some());

        // Too long number should fail
        let result = validator.validate("+1234567890123456789");
        assert!(!result.is_valid);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_non_strict_validation_mode() {
        let mut config = PhoneValidationConfig::default();
        config.strict_validation = false;
        let validator = PhoneValidator::new(config);

        // Invalid number should still validate as true in non-strict mode
        let result = validator.validate("invalid-number");
        assert!(result.is_valid); // Non-strict allows invalid numbers
        assert!(result.error.is_none());
    }

    #[test]
    fn test_disabled_validation() {
        let mut config = PhoneValidationConfig::default();
        config.enabled = false;
        let validator = PhoneValidator::new(config);

        // All numbers should pass when validation is disabled
        let result = validator.validate("completely-invalid-number");
        assert!(result.is_valid);
        assert!(result.error.is_none());
        assert!(result.country_code.is_none());
    }

    #[test]
    fn test_country_detection_disabled() {
        let mut config = PhoneValidationConfig::default();
        config.use_country_detection = false;
        let validator = PhoneValidator::new(config);

        let result = validator.validate("+44 20 7946 0958");
        assert!(result.is_valid);
        assert!(result.country_code.is_none()); // Should be None when detection disabled
    }

    #[test]
    fn test_country_code_detection() {
        let config = PhoneValidationConfig::default();
        let validator = PhoneValidator::new(config);

        // Test various country codes
        assert_eq!(validator.get_country_code("+1-555-123-4567"), Some("US".to_string()));
        assert_eq!(validator.get_country_code("+44 20 7946 0958"), Some("GB".to_string()));
        assert_eq!(validator.get_country_code("+49 30 12345678"), Some("DE".to_string()));
        assert_eq!(validator.get_country_code("+33 1 42 86 83 26"), Some("FR".to_string()));
        assert_eq!(validator.get_country_code("+39 06 12345678"), Some("IT".to_string()));
        assert_eq!(validator.get_country_code("+34 91 123 4567"), Some("ES".to_string()));
        assert_eq!(validator.get_country_code("+31 20 123 4567"), Some("NL".to_string()));
        assert_eq!(validator.get_country_code("+32 2 123 45 67"), Some("BE".to_string()));
        assert_eq!(validator.get_country_code("+43 1 12345678"), Some("AT".to_string()));
        assert_eq!(validator.get_country_code("+41 44 123 45 67"), Some("CH".to_string()));
        assert_eq!(validator.get_country_code("+46 8 123 456 78"), Some("SE".to_string()));
        assert_eq!(validator.get_country_code("+47 22 12 34 56"), Some("NO".to_string()));
        assert_eq!(validator.get_country_code("+45 32 12 34 56"), Some("DK".to_string()));
        assert_eq!(validator.get_country_code("+358 9 1234 5678"), Some("FI".to_string()));

        // Unknown country code should return None
        assert_eq!(validator.get_country_code("+999 123 456 789"), None);
    }

    #[test]
    fn test_international_detection() {
        let config = PhoneValidationConfig::default();
        let validator = PhoneValidator::new(config);

        // Domestic numbers (assuming US default region)
        assert!(!validator.is_international("555-123-4567"));
        assert!(!validator.is_international("(212) 555-1234"));
        assert!(!validator.is_international("2125551234"));

        // International numbers
        assert!(validator.is_international("+44 20 7946 0958")); // UK
        assert!(validator.is_international("+49 30 12345678")); // Germany
        assert!(validator.is_international("011441234567890")); // US international prefix
        assert!(validator.is_international("0049301234567")); // European international prefix

        // Edge cases
        assert!(validator.is_international("+999 123 456 789")); // Unknown country
    }

    #[test]
    fn test_e164_normalization() {
        let config = PhoneValidationConfig::default();
        let validator = PhoneValidator::new(config);

        // Test various input formats normalize to E164
        assert_eq!(validator.normalize_to_e164("+1-555-123-4567").unwrap(), "+15551234567");
        assert_eq!(validator.normalize_to_e164("+44 20 7946 0958").unwrap(), "+442079460958");
        assert_eq!(validator.normalize_to_e164("011 44 20 7946 0958").unwrap(), "+442079460958");
        assert_eq!(validator.normalize_to_e164("00 49 30 12345678").unwrap(), "+4930123456");

        // Invalid numbers should fail
        assert!(validator.normalize_to_e164("invalid").is_err());
        assert!(validator.normalize_to_e164("123").is_err());
    }

    #[test]
    fn test_international_formatting() {
        let config = PhoneValidationConfig::default();
        let validator = PhoneValidator::new(config);

        // Test international formatting
        let result = validator.format_international("+1-555-123-4567");
        assert!(result.is_ok());
        assert!(result.unwrap().starts_with("+"));

        let result = validator.format_international("555-123-4567");
        assert!(result.is_ok());
        assert!(result.unwrap().contains("555"));
    }

    #[test]
    fn test_country_prefix_extraction() {
        let config = PhoneValidationConfig::default();
        let validator = PhoneValidator::new(config);

        assert_eq!(validator.extract_country_prefix("+1-555-123-4567"), Some("1".to_string()));
        assert_eq!(validator.extract_country_prefix("+44 20 7946 0958"), Some("44".to_string()));
        assert_eq!(validator.extract_country_prefix("+49 30 12345678"), Some("49".to_string()));
        
        // Domestic numbers should return None
        assert_eq!(validator.extract_country_prefix("555-123-4567"), None);
    }

    #[test]
    fn test_validation_result_structure() {
        let config = PhoneValidationConfig::default();
        let validator = PhoneValidator::new(config);

        let result = validator.validate("+1-555-123-4567");
        
        // Check all fields are properly populated
        assert_eq!(result.original, "+1-555-123-4567");
        assert!(result.is_valid);
        assert!(result.country_code.is_some());
        assert!(result.region_code.is_some());
        assert!(result.number_type.is_some());
        assert!(result.e164_format.is_some());
        assert!(result.international_format.is_some());
        assert!(result.error.is_none());

        // Check E164 format is properly formatted
        let e164 = result.e164_format.unwrap();
        assert!(e164.starts_with("+"));
        assert!(e164.len() > 5);
    }

    #[test]
    fn test_different_default_regions() {
        // Test with GB default region
        let mut config = PhoneValidationConfig::default();
        config.default_region = "GB".to_string();
        let validator = PhoneValidator::new(config);

        let result = validator.validate("20 7946 0958"); // UK domestic
        assert!(result.is_valid);
        assert_eq!(result.country_code, Some("GB".to_string()));

        // US number should be detected as international
        assert!(validator.is_international("+1-555-123-4567"));
    }

    #[test]
    fn test_edge_cases() {
        let config = PhoneValidationConfig::default();
        let validator = PhoneValidator::new(config);

        // Empty string
        let result = validator.validate("");
        assert!(!result.is_valid);

        // Only symbols
        let result = validator.validate("+++---()()");
        assert!(!result.is_valid);

        // Very long number
        let result = validator.validate("+1234567890123456789012345");
        assert!(!result.is_valid);

        // Number with letters
        let result = validator.validate("+1-555-CALL-NOW");
        assert!(!result.is_valid);
    }

    #[test]
    fn test_various_formatting_inputs() {
        let config = PhoneValidationConfig::default();
        let validator = PhoneValidator::new(config);

        // Test various common formatting styles
        let test_numbers = vec![
            "+1 555 123 4567",
            "+1-555-123-4567",
            "+1 (555) 123-4567",
            "+15551234567",
            "011 1 555 123 4567",
            "1-555-123-4567",
            "(555) 123-4567",
            "555.123.4567",
        ];

        for number in test_numbers {
            let result = validator.validate(number);
            assert!(result.is_valid, "Number {} should be valid", number);
            assert_eq!(result.country_code, Some("US".to_string()), "Number {} should be US", number);
        }
    }
}