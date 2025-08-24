use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::sync::Arc;

// Mock implementations for testing without the full codebase
use redfire_switch::lcr::types::*;

#[tokio::main]
async fn main() {
    println!("🧪 Testing LCR Rate Engine");

    // Test rate matching logic
    test_rate_matching().await;

    // Test jurisdiction logic
    test_jurisdiction_logic().await;

    // Test route cost calculations
    test_cost_calculations().await;

    // Test billing increments
    test_billing_increments();

    println!("\n✅ All LCR tests completed!");
}

async fn test_rate_matching() {
    println!("\n📞 Testing Rate Matching Logic");

    // Create mock rate entries
    let rates = vec![
        create_test_rate("1", dec!(0.0050), "Default US"),
        create_test_rate("1212", dec!(0.0035), "NYC"),
        create_test_rate("1213", dec!(0.0040), "LA"),
        create_test_rate("1702", dec!(0.0045), "Las Vegas"),
        create_test_rate("1702777", dec!(0.0030), "Specific Vegas"),
    ];

    // Test cases for longest prefix matching
    let test_cases = vec![
        (
            "1702777555",
            "1702777",
            dec!(0.0030),
            "Should match specific Vegas rate",
        ),
        (
            "1702555123",
            "1702",
            dec!(0.0045),
            "Should match general Vegas rate",
        ),
        ("1212555123", "1212", dec!(0.0035), "Should match NYC rate"),
        (
            "1415555123",
            "1",
            dec!(0.0050),
            "Should fall back to default US rate",
        ),
        (
            "1702777",
            "1702777",
            dec!(0.0030),
            "Should match exact rate",
        ),
    ];

    for (input, expected_code, expected_rate, description) in test_cases {
        let matched_rate = find_best_rate(&rates, input);

        match matched_rate {
            Some(rate) if rate.code == expected_code && rate.inter_rate == expected_rate => {
                println!(
                    "  ✅ {}: {} -> {} (${}/min)",
                    description, input, expected_code, expected_rate
                );
            }
            Some(rate) => {
                println!(
                    "  ❌ {}: {} -> {} (${}/min), expected {} (${}/min)",
                    description, input, rate.code, rate.inter_rate, expected_code, expected_rate
                );
            }
            None => {
                println!("  ❌ {}: {} -> No match found", description, input);
            }
        }
    }
}

async fn test_jurisdiction_logic() {
    println!("\n🗺️  Testing Jurisdiction Logic");

    let test_cases = vec![
        (
            "12125551234",
            "14155555678",
            CallJurisdiction::Interstate,
            "NYC to SF (Interstate)",
        ),
        (
            "12125551234",
            "12125556789",
            CallJurisdiction::Intrastate,
            "NYC to NYC (Intrastate)",
        ),
        (
            "14155551234",
            "12135555678",
            CallJurisdiction::Interstate,
            "SF to LA (Interstate)",
        ),
        (
            "13055551234",
            "13055556789",
            CallJurisdiction::Local,
            "Miami local",
        ),
    ];

    for (ani, dnis, expected, description) in test_cases {
        let jurisdiction = mock_determine_jurisdiction(ani, dnis);

        if jurisdiction == expected {
            println!(
                "  ✅ {}: {} -> {} = {:?}",
                description, ani, dnis, jurisdiction
            );
        } else {
            println!(
                "  ❌ {}: {} -> {} = {:?}, expected {:?}",
                description, ani, dnis, jurisdiction, expected
            );
        }
    }
}

async fn test_cost_calculations() {
    println!("\n💰 Testing Cost Calculations");

    let test_cases = vec![
        (
            dec!(0.0050),
            dec!(0.0100),
            60,
            6,
            6,
            dec!(0.0150),
            "Basic 60-second call",
        ),
        (
            dec!(0.0050),
            dec!(0.0100),
            30,
            6,
            6,
            dec!(0.0105),
            "30-second call with 6-second minimum",
        ),
        (
            dec!(0.0050),
            dec!(0.0100),
            70,
            30,
            6,
            dec!(0.0133),
            "70-second call with 30/6 billing",
        ),
        (
            dec!(0.0050),
            dec!(0.0100),
            5,
            6,
            6,
            dec!(0.0105),
            "5-second call rounds to minimum",
        ),
    ];

    for (cost_rate, setup_fee, duration, min_inc, interval, expected_cost, description) in
        test_cases
    {
        let total_cost = calculate_call_cost(cost_rate, setup_fee, min_inc, interval, duration);

        if (total_cost - expected_cost).abs() < dec!(0.0001) {
            println!(
                "  ✅ {}: {}s @ ${}/min + ${} setup = ${}",
                description, duration, cost_rate, setup_fee, total_cost
            );
        } else {
            println!(
                "  ❌ {}: {}s @ ${}/min + ${} setup = ${}, expected ${}",
                description, duration, cost_rate, setup_fee, total_cost, expected_cost
            );
        }
    }
}

fn test_billing_increments() {
    println!("\n⏱️  Testing Billing Increments");

    let test_cases = vec![
        (5, 6, 6, 6, "5 seconds with 6/6 billing"),
        (30, 6, 6, 30, "30 seconds with 6/6 billing"),
        (65, 6, 6, 66, "65 seconds with 6/6 billing"),
        (35, 30, 6, 36, "35 seconds with 30/6 billing"),
        (90, 30, 6, 90, "90 seconds with 30/6 billing"),
        (95, 30, 6, 96, "95 seconds with 30/6 billing"),
    ];

    for (actual, min_increment, interval, expected, description) in test_cases {
        let billed = calculate_billed_duration(actual, min_increment, interval);

        if billed == expected {
            println!("  ✅ {}: {} -> {} seconds", description, actual, billed);
        } else {
            println!(
                "  ❌ {}: {} -> {} seconds, expected {}",
                description, actual, billed, expected
            );
        }
    }
}

// Helper functions for testing

fn create_test_rate(code: &str, rate: Decimal, description: &str) -> NanpaRate {
    NanpaRate {
        id: 1,
        deck_id: 1,
        code: code.to_string(),
        inter_rate: rate,
        intra_rate: rate * dec!(0.9),
        ij_rate: rate * dec!(0.95),
        local_rate: Some(rate * dec!(0.8)),
        min_increment: 6,
        interval: 6,
        setup_fee: Some(dec!(0.0100)),
    }
}

fn find_best_rate(rates: &[NanpaRate], code: &str) -> Option<NanpaRate> {
    // Implement the same longest prefix matching logic as in cache.rs
    for prefix_len in (1..=code.len()).rev() {
        let prefix = &code[0..prefix_len];

        if let Some(rate) = rates.iter().find(|r| r.code == prefix) {
            return Some(rate.clone());
        }
    }

    None
}

fn mock_determine_jurisdiction(ani: &str, dnis: &str) -> CallJurisdiction {
    // Simple mock jurisdiction logic
    if ani.len() >= 4 && dnis.len() >= 4 {
        let ani_npa = &ani[1..4]; // Skip leading 1
        let dnis_npa = &dnis[1..4];

        if ani_npa == dnis_npa {
            // Same area code - could be local or intrastate
            if ani.len() >= 7 && dnis.len() >= 7 {
                let ani_nxx = &ani[4..7];
                let dnis_nxx = &dnis[4..7];

                if ani_nxx == dnis_nxx {
                    return CallJurisdiction::Local;
                }
            }
            CallJurisdiction::Intrastate
        } else {
            // Different area codes
            let state_map = [
                ("212", "NY"),
                ("213", "CA"),
                ("415", "CA"),
                ("305", "FL"),
                ("702", "NV"),
                ("404", "GA"),
                ("202", "DC"),
                ("617", "MA"),
            ];

            let ani_state = state_map
                .iter()
                .find(|(npa, _)| *npa == ani_npa)
                .map(|(_, state)| *state);
            let dnis_state = state_map
                .iter()
                .find(|(npa, _)| *npa == dnis_npa)
                .map(|(_, state)| *state);

            match (ani_state, dnis_state) {
                (Some(ani_st), Some(dnis_st)) if ani_st == dnis_st => CallJurisdiction::Intrastate,
                (Some(_), Some(_)) => CallJurisdiction::Interstate,
                _ => CallJurisdiction::IndeterminateJurisdiction,
            }
        }
    } else {
        CallJurisdiction::IndeterminateJurisdiction
    }
}

fn calculate_call_cost(
    rate_per_minute: Decimal,
    setup_fee: Decimal,
    min_increment: i32,
    interval: i32,
    call_duration_seconds: i32,
) -> Decimal {
    let billed_duration = calculate_billed_duration(call_duration_seconds, min_increment, interval);
    let billed_minutes = Decimal::from(billed_duration) / Decimal::from(60);
    setup_fee + (rate_per_minute * billed_minutes)
}

fn calculate_billed_duration(actual_seconds: i32, min_increment: i32, interval: i32) -> i32 {
    if actual_seconds <= min_increment {
        min_increment
    } else {
        let excess = actual_seconds - min_increment;
        let additional_intervals = (excess + interval - 1) / interval; // Ceiling division
        min_increment + (additional_intervals * interval)
    }
}
