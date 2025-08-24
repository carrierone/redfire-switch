use std::collections::HashMap;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

// Copy the core LCR types for testing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallJurisdiction {
    Interstate,
    Intrastate,
    IndeterminateJurisdiction,
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NanpaRate {
    pub id: i32,
    pub deck_id: i32,
    pub code: String,
    pub inter_rate: Decimal,
    pub intra_rate: Decimal,
    pub ij_rate: Decimal,
    pub local_rate: Option<Decimal>,
    pub min_increment: i32,
    pub interval: i32,
    pub setup_fee: Option<Decimal>,
}

fn main() {
    println!("🧪 LCR Rate Engine Standalone Tests");
    println!("====================================");
    
    // Test 1: Rate matching with longest prefix
    test_rate_matching();
    
    // Test 2: Billing increment calculations
    test_billing_increments();
    
    // Test 3: Cost calculations with setup fees
    test_cost_calculations();
    
    // Test 4: Jurisdiction determination
    test_jurisdiction_logic();
    
    // Test 5: Route sorting logic
    test_route_sorting();
    
    println!("\n✅ All LCR core logic tests completed!");
}

fn test_rate_matching() {
    println!("\n📞 Testing LCR Rate Matching (Longest Prefix)");
    
    // Create test rate deck
    let mut rates = HashMap::new();
    rates.insert("1".to_string(), create_rate("1", dec!(0.0050), "Default US"));
    rates.insert("1212".to_string(), create_rate("1212", dec!(0.0035), "NYC"));
    rates.insert("1213".to_string(), create_rate("1213", dec!(0.0040), "LA"));
    rates.insert("1702".to_string(), create_rate("1702", dec!(0.0045), "Las Vegas"));
    rates.insert("1702777".to_string(), create_rate("1702777", dec!(0.0025), "Specific Vegas"));
    rates.insert("1702888".to_string(), create_rate("1702888", dec!(0.0030), "Another Vegas"));
    
    let test_cases = vec![
        ("1702777123", "1702777", dec!(0.0025), "Exact match for specific Vegas prefix"),
        ("1702888456", "1702888", dec!(0.0030), "Exact match for another Vegas prefix"),
        ("1702555123", "1702", dec!(0.0045), "Fall back to general Vegas"),
        ("1212555123", "1212", dec!(0.0035), "NYC area code match"),
        ("1415555123", "1", dec!(0.0050), "Fall back to default US"),
        ("1702777", "1702777", dec!(0.0025), "Exact length match"),
        ("170", "1", dec!(0.0050), "Partial number falls back to default"),
    ];
    
    for (input, expected_code, expected_rate, description) in test_cases {
        let matched_rate = find_longest_prefix_rate(&rates, input);
        
        match matched_rate {
            Some(rate) if rate.code == expected_code && rate.inter_rate == expected_rate => {
                println!("  ✅ {}: {} → {} @ ${}/min", description, input, expected_code, expected_rate);
            }
            Some(rate) => {
                println!("  ❌ {}: {} → {} @ ${}/min (expected {} @ ${}/min)", 
                    description, input, rate.code, rate.inter_rate, expected_code, expected_rate);
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
        (dec!(0.0060), dec!(0.0100), 60, 6, 6, dec!(0.0160), "60s call: $0.006/min + $0.01 setup"),
        (dec!(0.0060), dec!(0.0100), 30, 6, 6, dec!(0.0130), "30s call: billed for 30s"),
        (dec!(0.0060), dec!(0.0100), 5, 6, 6, dec!(0.0106), "5s call: minimum 6s billing"),
        (dec!(0.0060), dec!(0.0100), 65, 6, 6, dec!(0.0166), "65s call: billed for 66s"),
        (dec!(0.0050), dec!(0.0000), 60, 6, 6, dec!(0.0050), "60s call: no setup fee"),
        (dec!(0.0100), dec!(0.0200), 30, 30, 6, dec!(0.0250), "30s call: 30/6 billing with high setup"),
    ];
    
    for (rate, setup, duration, min_inc, interval, expected, description) in test_cases {
        let total_cost = calculate_total_cost(rate, setup, duration, min_inc, interval);
        
        if (total_cost - expected).abs() < dec!(0.00001) {
            println!("  ✅ {}: ${}", description, total_cost);
        } else {
            println!("  ❌ {}: got ${}, expected ${}", description, total_cost, expected);
        }
    }
}

fn test_jurisdiction_logic() {
    println!("\n🗺️  Testing Jurisdiction Determination");
    
    let test_cases = vec![
        ("12125551234", "14155555678", CallJurisdiction::Interstate, "NYC → SF (different states)"),
        ("12125551234", "12125556789", CallJurisdiction::Local, "NYC → NYC (same exchange)"),
        ("12125551234", "12135556789", CallJurisdiction::Interstate, "NYC → LA (different states)"),
        ("14155551234", "14165556789", CallJurisdiction::Intrastate, "SF → different CA area code"),
        ("13055551234", "13055556789", CallJurisdiction::Local, "Miami → Miami (same area)"),
        ("17025551234", "17025556789", CallJurisdiction::Local, "Vegas → Vegas (same area)"),
        ("12345", "67890", CallJurisdiction::IndeterminateJurisdiction, "Invalid numbers"),
    ];
    
    for (ani, dnis, expected, description) in test_cases {
        let jurisdiction = determine_jurisdiction(ani, dnis);
        
        if jurisdiction == expected {
            println!("  ✅ {}: {:?}", description, jurisdiction);
        } else {
            println!("  ❌ {}: got {:?}, expected {:?}", description, jurisdiction, expected);
        }
    }
}

fn test_route_sorting() {
    println!("\n📊 Testing LCR Route Sorting Logic");
    
    // Create test routes with different costs, setup fees, and priorities
    let mut routes = vec![
        TestRoute::new("Vendor-A", dec!(0.0050), dec!(0.0100), 100),
        TestRoute::new("Vendor-B", dec!(0.0040), dec!(0.0150), 110),
        TestRoute::new("Vendor-C", dec!(0.0045), dec!(0.0080), 90),
        TestRoute::new("Vendor-D", dec!(0.0040), dec!(0.0150), 100),
    ];
    
    // Sort by LCR logic: total cost for 60s call, then priority
    routes.sort_by(|a, b| {
        let a_total = a.setup_fee + a.cost_per_minute;
        let b_total = b.setup_fee + b.cost_per_minute;
        
        a_total.cmp(&b_total).then(a.priority.cmp(&b.priority))
    });
    
    println!("  Routes sorted by total cost (setup + 1 min):");
    for (i, route) in routes.iter().enumerate() {
        let total_cost = route.setup_fee + route.cost_per_minute;
        println!("    {}. {} - ${}/min + ${} setup = ${} total (priority {})", 
            i + 1, route.name, route.cost_per_minute, route.setup_fee, total_cost, route.priority);
    }
    
    // Verify sorting is correct
    let expected_order = vec!["Vendor-C", "Vendor-A", "Vendor-D", "Vendor-B"];
    let actual_order: Vec<&str> = routes.iter().map(|r| r.name.as_str()).collect();
    
    if actual_order == expected_order {
        println!("  ✅ Routes correctly sorted by least cost + priority");
    } else {
        println!("  ❌ Route sorting incorrect. Expected: {:?}, Got: {:?}", expected_order, actual_order);
    }
}

// Helper functions

fn create_rate(code: &str, rate: Decimal, _description: &str) -> NanpaRate {
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

fn find_longest_prefix_rate(rates: &HashMap<String, NanpaRate>, code: &str) -> Option<NanpaRate> {
    // LCR longest prefix matching: try from longest to shortest
    for prefix_len in (1..=code.len()).rev() {
        let prefix = &code[0..prefix_len];
        
        if let Some(rate) = rates.get(prefix) {
            return Some(rate.clone());
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
    rate_per_minute: Decimal,
    setup_fee: Decimal,
    duration_seconds: i32,
    min_increment: i32,
    interval: i32,
) -> Decimal {
    let billed_duration = calculate_billed_duration(duration_seconds, min_increment, interval);
    let billed_minutes = Decimal::from(billed_duration) / Decimal::from(60);
    setup_fee + (rate_per_minute * billed_minutes)
}

fn determine_jurisdiction(ani: &str, dnis: &str) -> CallJurisdiction {
    // Simple jurisdiction logic for testing
    if ani.len() < 10 || dnis.len() < 10 {
        return CallJurisdiction::IndeterminateJurisdiction;
    }
    
    // Extract area codes (skip leading 1 if present)
    let ani_start = if ani.starts_with('1') && ani.len() == 11 { 1 } else { 0 };
    let dnis_start = if dnis.starts_with('1') && dnis.len() == 11 { 1 } else { 0 };
    
    if ani.len() < ani_start + 3 || dnis.len() < dnis_start + 3 {
        return CallJurisdiction::IndeterminateJurisdiction;
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
                return CallJurisdiction::Local;
            }
        }
        return CallJurisdiction::Intrastate;
    }
    
    // Different area codes - check states
    let state_map = [
        ("212", "NY"), ("213", "CA"), ("415", "CA"), ("416", "CA"),
        ("305", "FL"), ("702", "NV"), ("404", "GA"), ("202", "DC"),
    ];
    
    let ani_state = state_map.iter().find(|(npa, _)| *npa == ani_npa).map(|(_, state)| *state);
    let dnis_state = state_map.iter().find(|(npa, _)| *npa == dnis_npa).map(|(_, state)| *state);
    
    match (ani_state, dnis_state) {
        (Some(ani_st), Some(dnis_st)) if ani_st == dnis_st => CallJurisdiction::Intrastate,
        (Some(_), Some(_)) => CallJurisdiction::Interstate,
        _ => CallJurisdiction::IndeterminateJurisdiction,
    }
}

#[derive(Debug, Clone)]
struct TestRoute {
    name: String,
    cost_per_minute: Decimal,
    setup_fee: Decimal,
    priority: i32,
}

impl TestRoute {
    fn new(name: &str, cost: Decimal, setup: Decimal, priority: i32) -> Self {
        Self {
            name: name.to_string(),
            cost_per_minute: cost,
            setup_fee: setup,
            priority,
        }
    }
}