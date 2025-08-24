use std::collections::HashMap;

// Test the cache rate matching logic specifically
fn main() {
    println!("🔍 Testing Cache Rate Matching Logic");
    println!("===================================");
    
    test_cache_rate_matching();
    test_edge_cases();
    test_performance_scenarios();
    
    println!("\n✅ Cache rate matching tests completed!");
}

fn test_cache_rate_matching() {
    println!("\n📞 Testing Cache-Style Rate Matching");
    
    // Simulate the cache structure with multiple rate entries
    let mut rates = Vec::new();
    rates.push(Rate::new("1", 0.0050, "Default US"));
    rates.push(Rate::new("1212", 0.0035, "NYC"));
    rates.push(Rate::new("1213", 0.0040, "LA"));
    rates.push(Rate::new("1702", 0.0045, "Las Vegas"));
    rates.push(Rate::new("1702777", 0.0025, "Specific Vegas block"));
    rates.push(Rate::new("1702888", 0.0030, "Another Vegas block"));
    rates.push(Rate::new("170277712", 0.0020, "Very specific Vegas"));
    
    let test_cases = vec![
        ("170277712345", "170277712", 0.0020, "Should match most specific"),
        ("170277755555", "1702777", 0.0025, "Should match less specific"),
        ("170288855555", "1702888", 0.0030, "Should match different block"),
        ("170255555555", "1702", 0.0045, "Should fall back to area code"),
        ("121255555555", "1212", 0.0035, "Should match NYC"),
        ("141555555555", "1", 0.0050, "Should fall back to default"),
        ("1", "1", 0.0050, "Should match exact single digit"),
        ("17", "1", 0.0050, "Partial should fall back"),
    ];
    
    for (input, expected_code, expected_rate, description) in test_cases {
        let matched = cache_style_rate_match(&rates, input);
        
        match matched {
            Some(rate) if rate.code == expected_code && (rate.rate - expected_rate).abs() < 0.00001 => {
                println!("  ✅ {}: {} → {} @ ${:.4}/min", description, input, expected_code, expected_rate);
            }
            Some(rate) => {
                println!("  ❌ {}: {} → {} @ ${:.4}/min (expected {} @ ${:.4}/min)", 
                    description, input, rate.code, rate.rate, expected_code, expected_rate);
            }
            None => {
                println!("  ❌ {}: {} → No match (expected {} @ ${:.4}/min)", 
                    description, input, expected_code, expected_rate);
            }
        }
    }
}

fn test_edge_cases() {
    println!("\n🔬 Testing Edge Cases");
    
    let mut rates = Vec::new();
    rates.push(Rate::new("1", 0.0050, "Default"));
    rates.push(Rate::new("1234567890", 0.0010, "Full 10-digit"));
    
    let edge_cases = vec![
        ("", None, "Empty string"),
        ("0", None, "Single non-matching digit"),
        ("1234567890", Some(("1234567890", 0.0010)), "Exact full match"),
        ("12345678901", Some(("1234567890", 0.0010)), "Longer than exact match"),
        ("123456789", Some(("1", 0.0050)), "One digit short of full match"),
        ("999", Some(("1", 0.0050)), "Should fall back to default"),
    ];
    
    for (input, expected, description) in edge_cases {
        let matched = cache_style_rate_match(&rates, input);
        
        match (matched, expected) {
            (Some(rate), Some((exp_code, exp_rate))) => {
                if rate.code == exp_code && (rate.rate - exp_rate).abs() < 0.00001 {
                    println!("  ✅ {}: {} → {} @ ${:.4}/min", description, input, rate.code, rate.rate);
                } else {
                    println!("  ❌ {}: {} → {} @ ${:.4}/min (expected {} @ ${:.4}/min)", 
                        description, input, rate.code, rate.rate, exp_code, exp_rate);
                }
            }
            (None, None) => {
                println!("  ✅ {}: {} → No match (as expected)", description, input);
            }
            (Some(rate), None) => {
                println!("  ❌ {}: {} → {} @ ${:.4}/min (expected no match)", 
                    description, input, rate.code, rate.rate);
            }
            (None, Some((exp_code, exp_rate))) => {
                println!("  ❌ {}: {} → No match (expected {} @ ${:.4}/min)", 
                    description, input, exp_code, exp_rate);
            }
        }
    }
}

fn test_performance_scenarios() {
    println!("\n⚡ Testing Performance Scenarios");
    
    // Create a larger rate deck to test performance
    let mut rates = Vec::new();
    
    // Add comprehensive NANPA coverage
    for npa in 200..999 {
        rates.push(Rate::new(&format!("1{}", npa), 0.0050, "NPA rate"));
        
        // Add some NXX rates for major areas
        if npa == 212 || npa == 213 || npa == 415 || npa == 702 {
            for nxx in 200..299 {
                rates.push(Rate::new(&format!("1{}{}", npa, nxx), 0.0030, "NPANXX rate"));
            }
        }
    }
    
    println!("  📊 Created rate deck with {} entries", rates.len());
    
    let test_numbers = vec![
        "12122551234", // NYC with NXX rate
        "12134561234", // LA with NXX rate  
        "14155551234", // SF with NPA rate only
        "17022221234", // Vegas with NXX rate
        "18005551234", // Toll-free with NPA rate
        "19005551234", // Premium with NPA rate
    ];
    
    for number in test_numbers {
        let start = std::time::Instant::now();
        let matched = cache_style_rate_match(&rates, number);
        let duration = start.elapsed();
        
        match matched {
            Some(rate) => {
                println!("  ✅ {} → {} @ ${:.4}/min ({}μs)", 
                    number, rate.code, rate.rate, duration.as_micros());
            }
            None => {
                println!("  ❌ {} → No match ({}μs)", number, duration.as_micros());
            }
        }
    }
}

// Helper structures and functions

#[derive(Debug, Clone)]
struct Rate {
    code: String,
    rate: f64,
    description: String,
}

impl Rate {
    fn new(code: &str, rate: f64, description: &str) -> Self {
        Self {
            code: code.to_string(),
            rate,
            description: description.to_string(),
        }
    }
}

fn cache_style_rate_match(rates: &[Rate], code: &str) -> Option<Rate> {
    if code.is_empty() {
        return None;
    }
    
    // LCR longest prefix matching: try from longest to shortest
    for prefix_len in (1..=code.len()).rev() {
        let prefix = &code[0..prefix_len];
        
        // Look for exact match of this prefix length
        if let Some(rate) = rates.iter().find(|r| r.code == prefix) {
            return Some(rate.clone());
        }
    }
    
    None
}