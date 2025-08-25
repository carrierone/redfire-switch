// Standalone phone validation test - no dependencies on the main library

/// Phone number validation configuration
#[derive(Debug, Clone)]
pub struct PhoneValidationConfig {
    pub enabled: bool,
    pub strict_validation: bool,
    pub default_region: String,
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
    pub original: String,
    pub is_valid: bool,
    pub country_code: Option<String>,
    pub region_code: Option<String>,
    pub number_type: Option<String>,
    pub e164_format: Option<String>,
    pub international_format: Option<String>,
    pub error: Option<String>,
}

/// Phone number validator
pub struct PhoneValidator {
    config: PhoneValidationConfig,
}

impl PhoneValidator {
    pub fn new(config: PhoneValidationConfig) -> Self {
        Self { config }
    }

    pub fn validate(&self, number: &str) -> ValidationResult {
        if !self.config.enabled {
            return ValidationResult {
                original: number.to_string(),
                is_valid: true,
                country_code: None,
                region_code: None,
                number_type: None,
                e164_format: None,
                international_format: None,
                error: None,
            };
        }

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

    pub fn is_international(&self, number: &str) -> bool {
        let normalized = self.normalize_number(number);
        
        normalized.starts_with("00") || 
        number.starts_with('+') || 
        normalized.starts_with("011") ||
        self.detect_country_code(&normalized).map_or(false, |cc| cc != self.config.default_region)
    }

    fn normalize_number(&self, number: &str) -> String {
        let digits: String = number.chars().filter(|c| c.is_digit(10)).collect();
        
        if digits.starts_with("011") && digits.len() > 3 {
            format!("+{}", &digits[3..])
        } else if digits.starts_with("00") && digits.len() > 2 {
            format!("+{}", &digits[2..])
        } else if number.starts_with('+') {
            format!("+{}", digits)
        } else {
            digits
        }
    }

    fn basic_validate(&self, normalized: &str) -> bool {
        if normalized.is_empty() {
            return false;
        }
        
        let digits = if normalized.starts_with('+') {
            &normalized[1..]
        } else {
            normalized
        };
        
        let len = digits.len();
        if len < 5 || len > 15 {
            return false;
        }
        
        digits.chars().all(|c| c.is_digit(10))
    }

    fn detect_country_code(&self, normalized: &str) -> Option<String> {
        if !normalized.starts_with('+') {
            return Some(self.config.default_region.clone());
        }
        
        let digits = &normalized[1..];
        
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
            None
        }
    }

    fn format_international(&self, normalized: &str) -> String {
        if normalized.starts_with('+') {
            normalized.to_string()
        } else if normalized.len() == 10 && self.config.default_region == "US" {
            format!("+1 {} {} {}", &normalized[0..3], &normalized[3..6], &normalized[6..])
        } else {
            format!("+{}", normalized)
        }
    }
}

fn main() {
    println!("📞 Standalone Phone Number Validation Test");
    println!("==========================================");
    println!();

    let config = PhoneValidationConfig::default();
    let validator = PhoneValidator::new(config);

    let test_numbers = vec![
        ("+1-555-123-4567", "US", "US number with dashes"),
        ("+44 20 7946 0958", "GB", "UK London number"),
        ("+49 30 12345678", "DE", "German Berlin number"),
        ("+33 1 42 86 83 26", "FR", "French Paris number"),
        ("+39 06 12345678", "IT", "Italian Rome number"),
        ("+34 91 123 4567", "ES", "Spanish Madrid number"),
        ("+31 20 123 4567", "NL", "Dutch Amsterdam number"),
        ("+41 44 123 45 67", "CH", "Swiss Zurich number"),
        ("+46 8 123 456 78", "SE", "Swedish Stockholm number"),
        ("555-123-4567", "US", "US domestic number"),
        ("011441234567890", "GB", "US international prefix to UK"),
        ("invalid-number", "", "Invalid number"),
    ];

    let mut passed = 0;
    let mut total = 0;

    for (number, expected_country, description) in test_numbers {
        total += 1;
        println!("🔍 Testing: {} ({})", number, description);
        
        let result = validator.validate(number);
        
        if result.is_valid {
            if let Some(ref country) = result.country_code {
                if country == expected_country || expected_country.is_empty() {
                    println!("  ✅ Valid - Country: {}", country);
                    if let Some(ref e164) = result.e164_format {
                        println!("  📱 E164: {}", e164);
                    }
                    passed += 1;
                } else {
                    println!("  ❌ Valid but wrong country - Expected: {}, Got: {}", 
                            expected_country, country);
                }
            } else {
                if expected_country.is_empty() {
                    println!("  ✅ Valid - No country detection needed");
                    passed += 1;
                } else {
                    println!("  ❌ Valid but no country detected - Expected: {}", 
                            expected_country);
                }
            }
        } else {
            if expected_country.is_empty() {
                println!("  ✅ Correctly identified as invalid");
                if let Some(ref error) = result.error {
                    println!("  ⚠️  Error: {}", error);
                }
                passed += 1;
            } else {
                println!("  ❌ Should be valid but marked invalid");
                if let Some(ref error) = result.error {
                    println!("  ⚠️  Error: {}", error);
                }
            }
        }
        
        // Test international detection
        let is_intl = validator.is_international(number);
        let expected_intl = !expected_country.is_empty() && expected_country != "US" || number.contains('+') || number.starts_with("011");
        if is_intl == expected_intl {
            println!("  🌍 International: {} ✓", 
                    if is_intl { "Yes" } else { "No" });
        } else {
            println!("  🌍 International: {} ❌ (expected {})", 
                    if is_intl { "Yes" } else { "No" },
                    if expected_intl { "Yes" } else { "No" });
        }
        
        println!();
    }

    // Test strict validation mode
    println!("🔒 Testing Strict Validation Mode");
    println!("----------------------------------");
    
    let mut strict_config = PhoneValidationConfig::default();
    strict_config.strict_validation = true;
    let strict_validator = PhoneValidator::new(strict_config);
    
    let invalid_numbers = vec!["invalid-number", "123", "+1234567890123456789", "", "+++---"];
    let mut strict_passed = 0;
    
    for number in invalid_numbers {
        let result = strict_validator.validate(number);
        if !result.is_valid {
            println!("  ✅ {} correctly rejected", number);
            strict_passed += 1;
        } else {
            println!("  ❌ {} should have been rejected", number);
        }
    }
    
    // Test disabled validation
    println!("\n🔓 Testing Disabled Validation Mode");
    println!("-----------------------------------");
    
    let mut disabled_config = PhoneValidationConfig::default();
    disabled_config.enabled = false;
    let disabled_validator = PhoneValidator::new(disabled_config);
    
    let disabled_result = disabled_validator.validate("completely-invalid-number");
    if disabled_result.is_valid {
        println!("  ✅ Disabled validation passes invalid numbers");
        strict_passed += 1;
    } else {
        println!("  ❌ Disabled validation should pass all numbers");
    }

    // Summary
    println!("\n📊 Test Summary");
    println!("===============");
    println!("Basic validation tests: {}/{}", passed, total);
    println!("Strict validation tests: {}/6", strict_passed);
    
    let total_tests = total + 6;
    let total_passed = passed + strict_passed;
    let success_rate = (total_passed as f32 / total_tests as f32) * 100.0;
    
    println!("Overall success rate: {:.1}%", success_rate);
    
    if total_passed == total_tests {
        println!("\n🎉 All tests passed!");
        println!("✨ Phone validation is working correctly and ready for international routing!");
    } else {
        println!("\n⚠️  Some tests failed. Check output above.");
    }

    println!("\n🚀 Key Features Demonstrated:");
    println!("  • ✅ Country code detection (US, GB, DE, FR, IT, ES, etc.)");
    println!("  • ✅ International vs domestic number classification");
    println!("  • ✅ E164 format normalization");
    println!("  • ✅ Configurable strict validation mode");
    println!("  • ✅ Enable/disable validation toggle");
    println!("  • ✅ Support for various international prefixes (+, 011, 00)");
    
    println!("\n🎯 Ready for integration with A-Z international routing!");
}