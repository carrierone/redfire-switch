use std::collections::HashMap;

// Simple test without external dependencies
fn main() {
    println!("🧪 LCR Rate Engine Core Logic Tests");
    println!("===================================");
    
    // Test 1: Rate matching with longest prefix
    test_rate_matching();
    
    // Test 2: Billing increment calculations
    test_billing_increments();
    
    // Test 3: Cost calculations with setup fees
    test_cost_calculations();
    
    // Test 4: Jurisdiction determination
    test_jurisdiction_logic();
    
    println!("\n✅ All LCR core logic tests completed!");
}

fn test_rate_matching() {
    println!("\n📞 Testing LCR Rate Matching (Longest Prefix)");
    
    // Create test rate deck
    let mut rates = HashMap::new();
    rates.insert("1".to_string(), 0.0050);        // Default US
    rates.insert("1212".to_string(), 0.0035);     // NYC
    rates.insert("1213".to_string(), 0.0040);     // LA
    rates.insert("1702".to_string(), 0.0045);     // Las Vegas
    rates.insert("1702777".to_string(), 0.0025);  // Specific Vegas
    rates.insert("1702888".to_string(), 0.0030);  // Another Vegas
    
    let test_cases = vec![
        ("1702777123", "1702777", 0.0025, "Exact match for specific Vegas prefix"),
        ("1702888456", "1702888", 0.0030, "Exact match for another Vegas prefix"),
        ("1702555123", "1702", 0.0045, "Fall back to general Vegas"),
        ("1212555123", "1212", 0.0035, "NYC area code match"),
        ("1415555123", "1", 0.0050, "Fall back to default US"),
        ("1702777", "1702777", 0.0025, "Exact length match"),
        ("170", "1", 0.0050, "Partial number falls back to default"),
    ];
    
    for (input, expected_code, expected_rate, description) in test_cases {
        let matched = find_longest_prefix_rate(&rates, input);
        
        match matched {
            Some((code, rate)) if code == expected_code && (rate - expected_rate).abs() < 0.00001 => {
                println!("  ✅ {}: {} → {} @ ${}/min", description, input, expected_code, expected_rate);
            }
            Some((code, rate)) => {
                println!("  ❌ {}: {} → {} @ ${}/min (expected {} @ ${}/min)", 
                    description, input, code, rate, expected_code, expected_rate);
            }
            None => {
                println!("  ❌ {}: {} → No match (expected {} @ ${}/min)", description, input, expected_code, expected_rate);
            }
        }
    }
}

fn test_billing_increments() {
    println!("\n⏱️  Testing Billing Increment Logic");
    
    let test_cases = vec![
        // Format: (actual_seconds, min_increment, interval, expected_billed, description)
        (5, 6, 6, 6, "5s call with 6/6 billing → minimum 6s"),
        (6, 6, 6, 6, "6s call with 6/6 billing → exactly 6s"),
        (7, 6, 6, 12, "7s call with 6/6 billing → next increment"),
        (30, 6, 6, 30, "30s call with 6/6 billing → 30s"),
        (65, 6, 6, 66, "65s call with 6/6 billing → 66s"),
        (35, 30, 6, 36, "35s call with 30/6 billing → 30+6s"),
        (90, 30, 6, 90, "90s call with 30/6 billing → 90s"),
        (95, 30, 6, 96, "95s call with 30/6 billing → 96s"),
        (1, 30, 6, 30, "1s call with 30/6 billing → minimum 30s"),
        (185, 60, 1, 185, "185s call with 60/1 billing → 185s"),
    ];
    
    for (actual, min_inc, interval, expected, description) in test_cases {
        let billed = calculate_billed_duration(actual, min_inc, interval);
        
        if billed == expected {
            println!("  ✅ {}", description);
        } else {
            println!("  ❌ {}: got {}s, expected {}s", description, billed, expected);
        }
    }
}

fn test_cost_calculations() {
    println!("\n💰 Testing Cost Calculations with Setup Fees");
    
    let test_cases = vec![
        // Format: (rate/min, setup_fee, duration_s, min_inc, interval, expected_total, description)
        (0.0060, 0.0100, 60, 6, 6, 0.0160, "60s call: $0.006/min + $0.01 setup"),
        (0.0060, 0.0100, 30, 6, 6, 0.0130, "30s call: billed for 30s"),
        (0.0060, 0.0100, 5, 6, 6, 0.0106, "5s call: minimum 6s billing"),
        (0.0060, 0.0100, 65, 6, 6, 0.0166, "65s call: billed for 66s"),
        (0.0050, 0.0000, 60, 6, 6, 0.0050, "60s call: no setup fee"),
        (0.0100, 0.0200, 30, 30, 6, 0.0250, "30s call: 30/6 billing with high setup"),
    ];
    
    for (rate, setup, duration, min_inc, interval, expected, description) in test_cases {
        let total_cost = calculate_total_cost(rate, setup, duration, min_inc, interval);
        
        if (total_cost - expected).abs() < 0.00001 {
            println!("  ✅ {}: ${:.4}", description, total_cost);
        } else {
            println!("  ❌ {}: got ${:.4}, expected ${:.4}", description, total_cost, expected);
        }
    }
}

fn test_jurisdiction_logic() {
    println!("\n🗺️  Testing Jurisdiction Determination");
    
    let test_cases = vec![
        ("12125551234", "14155555678", "Interstate", "NYC → SF (different states)"),
        ("12125551234", "12125556789", "Local", "NYC → NYC (same exchange)"),
        ("12125551234", "12135556789", "Interstate", "NYC → LA (different states)"),
        ("14155551234", "14165556789", "Intrastate", "SF → different CA area code"),
        ("13055551234", "13055556789", "Local", "Miami → Miami (same area)"),
        ("17025551234", "17025556789", "Local", "Vegas → Vegas (same area)"),
        ("12345", "67890", "Indeterminate", "Invalid numbers"),
    ];
    
    for (ani, dnis, expected, description) in test_cases {
        let jurisdiction = determine_jurisdiction(ani, dnis);
        
        if jurisdiction == expected {
            println!("  ✅ {}: {}", description, jurisdiction);
        } else {
            println!("  ❌ {}: got {}, expected {}", description, jurisdiction, expected);
        }
    }
}

// Helper functions

fn find_longest_prefix_rate(rates: &HashMap<String, f64>, code: &str) -> Option<(String, f64)> {
    // LCR longest prefix matching: try from longest to shortest
    for prefix_len in (1..=code.len()).rev() {
        let prefix = &code[0..prefix_len];
        
        if let Some(&rate) = rates.get(prefix) {
            return Some((prefix.to_string(), rate));
        }
    }
    
    None
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

fn calculate_total_cost(
    rate_per_minute: f64,
    setup_fee: f64,
    duration_seconds: i32,
    min_increment: i32,
    interval: i32,
) -> f64 {
    let billed_duration = calculate_billed_duration(duration_seconds, min_increment, interval);
    let billed_minutes = billed_duration as f64 / 60.0;
    setup_fee + (rate_per_minute * billed_minutes)
}

fn determine_jurisdiction(ani: &str, dnis: &str) -> &'static str {
    // Simple jurisdiction logic for testing
    if ani.len() < 10 || dnis.len() < 10 {
        return "Indeterminate";
    }
    
    // Extract area codes (skip leading 1 if present)
    let ani_start = if ani.starts_with('1') && ani.len() == 11 { 1 } else { 0 };
    let dnis_start = if dnis.starts_with('1') && dnis.len() == 11 { 1 } else { 0 };
    
    if ani.len() < ani_start + 3 || dnis.len() < dnis_start + 3 {
        return "Indeterminate";
    }
    
    let ani_npa = &ani[ani_start..ani_start + 3];
    let dnis_npa = &dnis[dnis_start..dnis_start + 3];
    
    // Same area code - check for local
    if ani_npa == dnis_npa {
        // Check exchange codes for local determination
        if ani.len() >= ani_start + 6 && dnis.len() >= dnis_start + 6 {
            let ani_nxx = &ani[ani_start + 3..ani_start + 6];
            let dnis_nxx = &dnis[dnis_start + 3..dnis_start + 6];
            
            if ani_nxx == dnis_nxx {
                return "Local";
            }
        }
        return "Intrastate";
    }
    
    // Different area codes - check states
    let state_map = [
        ("212", "NY"), ("213", "CA"), ("415", "CA"), ("416", "CA"),
        ("305", "FL"), ("702", "NV"), ("404", "GA"), ("202", "DC"),
    ];
    
    let ani_state = state_map.iter().find(|(npa, _)| *npa == ani_npa).map(|(_, state)| *state);
    let dnis_state = state_map.iter().find(|(npa, _)| *npa == dnis_npa).map(|(_, state)| *state);
    
    match (ani_state, dnis_state) {
        (Some(ani_st), Some(dnis_st)) if ani_st == dnis_st => "Intrastate",
        (Some(_), Some(_)) => "Interstate",
        _ => "Indeterminate",
    }
}