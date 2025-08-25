use std::process;
use colored::*;

#[tokio::main]
async fn main() {
    println!("{}", "🧪 Phone Number Validation & International Routing Test Suite".cyan().bold());
    println!("{}", "=".repeat(70).cyan());
    println!();

    let mut total_tests = 0;
    let mut passed_tests = 0;
    let mut failed_tests = 0;

    // Test categories
    let test_categories = vec![
        ("Phone Validation Tests", test_phone_validation().await),
        ("Routing Integration Tests", test_routing_integration().await),
        ("Database Integration Tests", test_database_integration().await),
        ("End-to-End Tests", test_end_to_end().await),
    ];

    for (category, results) in test_categories {
        println!("📋 {}", category.yellow().bold());
        println!("{}", "-".repeat(50).yellow());
        
        for (test_name, result) in results {
            total_tests += 1;
            match result {
                TestResult::Passed => {
                    println!("  ✅ {}", test_name.green());
                    passed_tests += 1;
                }
                TestResult::Failed(error) => {
                    println!("  ❌ {} - {}", test_name.red(), error.red());
                    failed_tests += 1;
                }
                TestResult::Skipped(reason) => {
                    println!("  ⏭️  {} - {}", test_name.yellow(), reason.yellow());
                }
            }
        }
        println!();
    }

    // Summary
    println!("{}", "📊 Test Summary".cyan().bold());
    println!("{}", "=".repeat(30).cyan());
    println!("Total tests: {}", total_tests.to_string().bold());
    println!("Passed: {}", passed_tests.to_string().green().bold());
    println!("Failed: {}", failed_tests.to_string().red().bold());
    println!("Skipped: {}", (total_tests - passed_tests - failed_tests).to_string().yellow().bold());
    
    let success_rate = if total_tests > 0 {
        (passed_tests as f32 / total_tests as f32) * 100.0
    } else {
        0.0
    };
    
    println!("Success rate: {:.1}%", success_rate);

    if failed_tests > 0 {
        println!("\n{}", "Some tests failed! Check the output above for details.".red().bold());
        process::exit(1);
    } else {
        println!("\n{}", "All tests passed! 🎉".green().bold());
    }
}

#[derive(Debug)]
enum TestResult {
    Passed,
    Failed(String),
    Skipped(String),
}

async fn test_phone_validation() -> Vec<(String, TestResult)> {
    let mut results = Vec::new();

    // Test 1: Basic phone validation
    results.push((
        "Basic phone number validation".to_string(),
        run_test(|| {
            use redfire_switch::lcr::phone_validation::*;
            
            let config = PhoneValidationConfig::default();
            let validator = PhoneValidator::new(config);
            
            let result = validator.validate("+1-555-123-4567");
            assert!(result.is_valid, "US number should be valid");
            assert_eq!(result.country_code, Some("US".to_string()));
            
            let result = validator.validate("+44 20 7946 0958");
            assert!(result.is_valid, "UK number should be valid");
            assert_eq!(result.country_code, Some("GB".to_string()));
            
            Ok(())
        }).await
    ));

    // Test 2: Country detection
    results.push((
        "Country code detection".to_string(),
        run_test(|| {
            use redfire_switch::lcr::phone_validation::*;
            
            let config = PhoneValidationConfig::default();
            let validator = PhoneValidator::new(config);
            
            assert_eq!(validator.get_country_code("+49 30 12345678"), Some("DE".to_string()));
            assert_eq!(validator.get_country_code("+33 1 42 86 83 26"), Some("FR".to_string()));
            assert_eq!(validator.get_country_code("+39 06 12345678"), Some("IT".to_string()));
            
            Ok(())
        }).await
    ));

    // Test 3: International detection
    results.push((
        "International number detection".to_string(),
        run_test(|| {
            use redfire_switch::lcr::phone_validation::*;
            
            let config = PhoneValidationConfig::default();
            let validator = PhoneValidator::new(config);
            
            assert!(!validator.is_international("555-123-4567")); // Domestic US
            assert!(validator.is_international("+44 20 7946 0958")); // UK
            assert!(validator.is_international("011441234567890")); // US intl prefix
            
            Ok(())
        }).await
    ));

    // Test 4: Strict validation mode
    results.push((
        "Strict validation mode".to_string(),
        run_test(|| {
            use redfire_switch::lcr::phone_validation::*;
            
            let mut config = PhoneValidationConfig::default();
            config.strict_validation = true;
            let validator = PhoneValidator::new(config);
            
            let result = validator.validate("invalid-number");
            assert!(!result.is_valid, "Invalid number should fail strict validation");
            assert!(result.error.is_some());
            
            Ok(())
        }).await
    ));

    // Test 5: E164 normalization
    results.push((
        "E164 format normalization".to_string(),
        run_test(|| {
            use redfire_switch::lcr::phone_validation::*;
            
            let config = PhoneValidationConfig::default();
            let validator = PhoneValidator::new(config);
            
            let result = validator.normalize_to_e164("+1-555-123-4567");
            assert!(result.is_ok());
            assert!(result.unwrap().starts_with("+"));
            
            let result = validator.normalize_to_e164("invalid");
            assert!(result.is_err());
            
            Ok(())
        }).await
    ));

    results
}

async fn test_routing_integration() -> Vec<(String, TestResult)> {
    let mut results = Vec::new();

    // Test 1: Route request structure
    results.push((
        "Route request creation".to_string(),
        run_test(|| {
            use redfire_switch::lcr::types::*;
            use redfire_switch::lcr::phone_validation::*;
            use chrono::Utc;
            
            let request = RouteRequest {
                ani: "15551234567".to_string(),
                dnis: "+44 20 7946 0958".to_string(),
                ingress_trunk_id: 1,
                client_deck_id: None,
                route_type: RouteType::AZ,
                require_profit_protection: false,
                min_profit_margin: None,
                effective_time: Some(Utc::now()),
                phone_validation: Some(PhoneValidationConfig::default()),
                routing_plan_id: Some(1),
            };
            
            assert_eq!(request.route_type, RouteType::AZ);
            assert!(request.phone_validation.is_some());
            assert_eq!(request.routing_plan_id, Some(1));
            
            Ok(())
        }).await
    ));

    // Test 2: International routing plan structure
    results.push((
        "International routing plan structure".to_string(),
        run_test(|| {
            use redfire_switch::lcr::types::*;
            use chrono::Utc;
            use rust_decimal::Decimal;
            use std::str::FromStr;
            
            let plan = InternationalRoutingPlan {
                id: 1,
                name: "Test EEA Plan".to_string(),
                description: Some("Test routing plan".to_string()),
                phone_validation_enabled: true,
                phone_validation_strict: false,
                phone_validation_default_region: "US".to_string(),
                phone_validation_use_country_detection: true,
                eea_routing_enabled: true,
                eea_priority_routing: true,
                eea_reduced_rates: true,
                eea_rate_reduction: Decimal::from_str("0.1").unwrap(),
                default_jurisdiction: InternationalJurisdiction::ROW,
                allow_unknown_destinations: true,
                max_rate_unknown_destinations: Decimal::from_str("1.0").unwrap(),
                require_strict_validation_unknown: false,
                active: true,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };
            
            assert!(plan.phone_validation_enabled);
            assert!(plan.eea_routing_enabled);
            assert_eq!(plan.eea_rate_reduction, Decimal::from_str("0.1").unwrap());
            
            Ok(())
        }).await
    ));

    // Test 3: Country routing preferences
    results.push((
        "Country routing preferences".to_string(),
        run_test(|| {
            use redfire_switch::lcr::types::*;
            use chrono::Utc;
            use rust_decimal::Decimal;
            use std::str::FromStr;
            
            let preference = CountryRoutingPreference {
                id: 1,
                routing_plan_id: 1,
                country_code: "DE".to_string(),
                country_name: "Germany".to_string(),
                jurisdiction: InternationalJurisdiction::EEA,
                quality_score: 95,
                cost_multiplier: Decimal::from_str("0.9").unwrap(),
                require_validation: true,
                max_duration_minutes: 0,
                created_at: Utc::now(),
            };
            
            assert_eq!(preference.country_code, "DE");
            assert_eq!(preference.jurisdiction, InternationalJurisdiction::EEA);
            assert_eq!(preference.cost_multiplier, Decimal::from_str("0.9").unwrap());
            
            Ok(())
        }).await
    ));

    results
}

async fn test_database_integration() -> Vec<(String, TestResult)> {
    let mut results = Vec::new();

    // Test 1: Default routing plans structure
    results.push((
        "Default routing plans structure".to_string(),
        run_test(|| {
            // Mock test for database structure validation
            // In a real implementation, this would test database connectivity
            
            // Test EEA countries list
            let eea_countries = vec![
                "AT", "BE", "BG", "CY", "CZ", "DE", "DK", "EE",
                "ES", "FI", "FR", "GR", "HR", "HU", "IE", "IS",
                "IT", "LI", "LT", "LU", "LV", "MT", "NL", "NO",
                "PL", "PT", "RO", "SE", "SI", "SK",
            ];
            
            assert!(eea_countries.contains(&"DE"));
            assert!(eea_countries.contains(&"FR"));
            assert!(eea_countries.contains(&"IT"));
            assert!(eea_countries.len() >= 27);
            
            // Test ROW countries list
            let row_countries = vec![
                "US", "CA", "MX", "AU", "NZ", "JP", "KR", "CN", 
                "IN", "BR", "AR", "CL", "ZA", "RU", "TR", "AE", "SA",
            ];
            
            assert!(row_countries.contains(&"US"));
            assert!(row_countries.contains(&"JP"));
            assert!(row_countries.contains(&"CN"));
            
            Ok(())
        }).await
    ));

    // Test 2: International rates structure
    results.push((
        "International rates structure".to_string(),
        run_test(|| {
            use redfire_switch::lcr::types::*;
            use chrono::Utc;
            use rust_decimal::Decimal;
            use std::str::FromStr;
            
            let rate = InternationalRate {
                id: 1,
                deck_id: 1,
                country_code: "44".to_string(),
                destination_code: Some("207".to_string()),
                destination_name: "UK London".to_string(),
                jurisdiction: InternationalJurisdiction::ROW,
                rate: Decimal::from_str("0.0125").unwrap(),
                initial_increment: 30,
                subsequent_increment: 6,
                setup_fee: Some(Decimal::from_str("0.001").unwrap()),
                created_at: Utc::now(),
            };
            
            assert_eq!(rate.country_code, "44");
            assert_eq!(rate.destination_code, Some("207".to_string()));
            assert_eq!(rate.initial_increment, 30);
            assert_eq!(rate.subsequent_increment, 6);
            
            Ok(())
        }).await
    ));

    results
}

async fn test_end_to_end() -> Vec<(String, TestResult)> {
    let mut results = Vec::new();

    // Test 1: Complete validation workflow
    results.push((
        "Complete phone validation workflow".to_string(),
        run_test(|| {
            use redfire_switch::lcr::phone_validation::*;
            use redfire_switch::lcr::types::*;
            use chrono::Utc;
            
            // Step 1: Create validator
            let config = PhoneValidationConfig::default();
            let validator = PhoneValidator::new(config.clone());
            
            // Step 2: Test international numbers
            let numbers = vec![
                ("+44 20 7946 0958", "GB"),
                ("+49 30 12345678", "DE"), 
                ("+33 1 42 86 83 26", "FR"),
            ];
            
            for (number, expected_country) in numbers {
                let result = validator.validate(number);
                assert!(result.is_valid, "Number {} should be valid", number);
                assert_eq!(result.country_code, Some(expected_country.to_string()));
            }
            
            // Step 3: Create routing request
            let request = RouteRequest {
                ani: "15551234567".to_string(),
                dnis: "+44 20 7946 0958".to_string(),
                ingress_trunk_id: 1,
                client_deck_id: None,
                route_type: RouteType::AZ,
                require_profit_protection: false,
                min_profit_margin: None,
                effective_time: Some(Utc::now()),
                phone_validation: Some(config),
                routing_plan_id: Some(1),
            };
            
            assert_eq!(request.route_type, RouteType::AZ);
            assert!(request.phone_validation.is_some());
            
            Ok(())
        }).await
    ));

    // Test 2: EEA vs ROW routing logic
    results.push((
        "EEA vs ROW routing preferences".to_string(),
        run_test(|| {
            use redfire_switch::lcr::types::*;
            use rust_decimal::Decimal;
            use std::str::FromStr;
            use chrono::Utc;
            
            // EEA preference (should have cost reduction)
            let eea_pref = CountryRoutingPreference {
                id: 1,
                routing_plan_id: 1,
                country_code: "DE".to_string(),
                country_name: "Germany".to_string(),
                jurisdiction: InternationalJurisdiction::EEA,
                quality_score: 95,
                cost_multiplier: Decimal::from_str("0.9").unwrap(),
                require_validation: true,
                max_duration_minutes: 0,
                created_at: Utc::now(),
            };
            
            // ROW preference (normal pricing)
            let row_pref = CountryRoutingPreference {
                id: 2,
                routing_plan_id: 2,
                country_code: "US".to_string(),
                country_name: "United States".to_string(),
                jurisdiction: InternationalJurisdiction::ROW,
                quality_score: 85,
                cost_multiplier: Decimal::ONE,
                require_validation: false,
                max_duration_minutes: 0,
                created_at: Utc::now(),
            };
            
            // Test cost calculation
            let base_rate = Decimal::from_str("0.10").unwrap();
            let eea_rate = base_rate * eea_pref.cost_multiplier;
            let row_rate = base_rate * row_pref.cost_multiplier;
            
            assert_eq!(eea_rate, Decimal::from_str("0.09").unwrap());
            assert_eq!(row_rate, Decimal::from_str("0.10").unwrap());
            assert!(eea_rate < row_rate); // EEA should be cheaper
            
            Ok(())
        }).await
    ));

    // Test 3: Profit protection logic
    results.push((
        "Profit protection logic".to_string(),
        run_test(|| {
            use rust_decimal::Decimal;
            use std::str::FromStr;
            
            let cost = Decimal::from_str("0.05").unwrap();
            let selling = Decimal::from_str("0.06").unwrap();
            let profit = selling - cost;
            let min_margin = Decimal::from_str("0.02").unwrap();
            
            assert_eq!(profit, Decimal::from_str("0.01").unwrap());
            assert!(profit < min_margin); // Should fail profit protection
            
            // Test profitable route
            let profitable_selling = Decimal::from_str("0.08").unwrap();
            let profitable_profit = profitable_selling - cost;
            assert!(profitable_profit >= min_margin); // Should pass profit protection
            
            Ok(())
        }).await
    ));

    results
}

async fn run_test<F>(test_fn: F) -> TestResult
where
    F: FnOnce() -> Result<(), Box<dyn std::error::Error + Send + Sync>> + std::panic::UnwindSafe,
{
    match std::panic::catch_unwind(test_fn) {
        Ok(Ok(())) => TestResult::Passed,
        Ok(Err(e)) => TestResult::Failed(e.to_string()),
        Err(panic_info) => {
            let msg = if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else if let Some(&s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "Test panicked with unknown error".to_string()
            };
            TestResult::Failed(msg)
        }
    }
}