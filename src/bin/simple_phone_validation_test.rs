use colored::*;
use redfire_switch::lcr::phone_validation::{PhoneValidationConfig, PhoneValidator};

fn main() {
    println!("{}", "📞 Phone Number Validation Test".cyan().bold());
    println!("{}", "=".repeat(50).cyan());
    println!();

    // Create validator with default configuration
    let config = PhoneValidationConfig::default();
    let validator = PhoneValidator::new(config);

    // Test various phone numbers
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
                    println!("  ✅ Valid - Country: {}", country.green());
                    if let Some(ref e164) = result.e164_format {
                        println!("  📱 E164: {}", e164.blue());
                    }
                    passed += 1;
                } else {
                    println!(
                        "  ❌ Valid but wrong country - Expected: {}, Got: {}",
                        expected_country.red(),
                        country.yellow()
                    );
                }
            } else {
                if expected_country.is_empty() {
                    println!("  ✅ Valid - No country detection");
                    passed += 1;
                } else {
                    println!(
                        "  ❌ Valid but no country detected - Expected: {}",
                        expected_country.red()
                    );
                }
            }
        } else {
            if expected_country.is_empty() {
                println!("  ✅ Correctly identified as invalid");
                if let Some(ref error) = result.error {
                    println!("  ⚠️  Error: {}", error.yellow());
                }
                passed += 1;
            } else {
                println!("  ❌ Should be valid but marked invalid");
                if let Some(ref error) = result.error {
                    println!("  ⚠️  Error: {}", error.red());
                }
            }
        }

        // Test international detection
        let is_intl = validator.is_international(number);
        let expected_intl = !expected_country.is_empty() && expected_country != "US"
            || number.contains('+')
            || number.starts_with("011");
        if is_intl == expected_intl {
            println!(
                "  🌍 International detection: {} ✓",
                if is_intl { "Yes".green() } else { "No".blue() }
            );
        } else {
            println!(
                "  🌍 International detection: {} ❌ (expected {})",
                if is_intl { "Yes".red() } else { "No".red() },
                if expected_intl { "Yes" } else { "No" }
            );
        }

        println!();
    }

    // Test strict validation mode
    println!("{}", "🔒 Testing Strict Validation Mode".cyan().bold());
    println!("{}", "-".repeat(40).cyan());

    let mut strict_config = PhoneValidationConfig::default();
    strict_config.strict_validation = true;
    let strict_validator = PhoneValidator::new(strict_config);

    let invalid_numbers = vec![
        "invalid-number",
        "123",
        "+1234567890123456789",
        "",
        "+++---",
    ];

    let mut strict_passed = 0;
    for number in invalid_numbers {
        let result = strict_validator.validate(number);
        if !result.is_valid {
            println!("  ✅ {} correctly rejected", number);
            strict_passed += 1;
        } else {
            println!("  ❌ {} should have been rejected", number.red());
        }
    }

    // Test disabled validation
    println!("\n{}", "🔓 Testing Disabled Validation Mode".cyan().bold());
    println!("{}", "-".repeat(40).cyan());

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
    println!("\n{}", "📊 Test Summary".cyan().bold());
    println!("{}", "=".repeat(30).cyan());
    println!(
        "Basic validation tests: {}/{}",
        passed.to_string().green(),
        total.to_string().bold()
    );
    println!(
        "Strict validation tests: {}/6",
        strict_passed.to_string().green()
    );

    let total_tests = total + 6;
    let total_passed = passed + strict_passed;
    let success_rate = (total_passed as f32 / total_tests as f32) * 100.0;

    println!("Overall success rate: {:.1}%", success_rate);

    if total_passed == total_tests {
        println!("\n{}", "🎉 All tests passed!".green().bold());
    } else {
        println!(
            "\n{}",
            "⚠️  Some tests failed. Check output above.".yellow().bold()
        );
    }

    // Test E164 normalization
    println!("\n{}", "🔄 Testing E164 Normalization".cyan().bold());
    println!("{}", "-".repeat(40).cyan());

    let normalization_tests = vec!["+1-555-123-4567", "+44 20 7946 0958", "011 49 30 12345678"];

    for number in normalization_tests {
        match validator.normalize_to_e164(number) {
            Ok(normalized) => {
                println!("  ✅ {} → {}", number, normalized.green());
            }
            Err(e) => {
                println!("  ❌ {} → Error: {}", number, e.to_string().red());
            }
        }
    }

    println!(
        "\n{}",
        "✨ Phone validation functionality test complete!"
            .cyan()
            .bold()
    );
}
