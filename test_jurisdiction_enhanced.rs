// Test enhanced jurisdiction logic including indeterminate cases
fn main() {
    println!("🗺️  Testing Enhanced Jurisdiction Logic");
    println!("======================================");
    
    test_indeterminate_numbers();
    test_toll_free_scenarios();
    test_international_scenarios();
    test_canadian_scenarios();
    test_premium_services();
    test_mixed_scenarios();
    
    println!("\n✅ Enhanced jurisdiction tests completed!");
}

fn test_indeterminate_numbers() {
    println!("\n🚫 Testing Indeterminate Number Detection");
    
    let test_cases = vec![
        // Toll-free numbers
        ("18005551234", true, "800 toll-free"),
        ("18335551234", true, "833 toll-free"),
        ("18445551234", true, "844 toll-free"),
        ("18555551234", true, "855 toll-free"),
        ("18665551234", true, "866 toll-free"),
        ("18775551234", true, "877 toll-free"),
        ("18885551234", true, "888 toll-free"),
        
        // Premium services
        ("19005551234", true, "900 premium rate"),
        ("19765551234", true, "976 premium rate"),
        
        // Special services
        ("15005551234", true, "500 personal communication"),
        ("17005551234", true, "700 IC services"),
        ("17105551234", true, "710 government"),
        ("17205551234", true, "720 special/VoIP"),
        
        // International
        ("01144207946", true, "International UK number"),
        ("011861234567", true, "International China number"),
        ("01152554321", true, "International Mexico"),
        
        // Canadian (NANPA but different jurisdiction)
        ("14165551234", true, "Toronto, Canada"),
        ("16045551234", true, "Vancouver, Canada"),
        ("15145551234", true, "Montreal, Canada"),
        ("14035551234", true, "Calgary, Canada"),
        
        // Invalid formats
        ("123456789", true, "Too short"),
        ("123456789012345", true, "Too long"),
        ("011", true, "Incomplete international"),
        ("0115551234", true, "Invalid 0XX area code"),
        ("1115551234", true, "Invalid 1XX area code"),
        ("1911", true, "N11 service code"),
        
        // Valid US numbers (should NOT be indeterminate)
        ("12125551234", false, "NYC landline"),
        ("13105551234", false, "LA landline"),
        ("14155551234", false, "SF landline"),
        ("17025551234", false, "Las Vegas landline"),
        ("13055551234", false, "Miami landline"),
        ("14045551234", false, "Atlanta landline"),
    ];
    
    for (number, expected_indeterminate, description) in test_cases {
        let is_indeterminate = is_indeterminate_number(number);
        
        if is_indeterminate == expected_indeterminate {
            let status = if expected_indeterminate { "Indeterminate" } else { "Determinate" };
            println!("  ✅ {}: {} → {}", description, number, status);
        } else {
            let expected = if expected_indeterminate { "Indeterminate" } else { "Determinate" };
            let actual = if is_indeterminate { "Indeterminate" } else { "Determinate" };
            println!("  ❌ {}: {} → {} (expected {})", description, number, actual, expected);
        }
    }
}

fn test_toll_free_scenarios() {
    println!("\n📞 Testing Toll-Free Call Scenarios");
    
    let scenarios = vec![
        ("12125551234", "18005551234", "IndeterminateJurisdiction", "Regular → Toll-free"),
        ("18005551234", "12125551234", "IndeterminateJurisdiction", "Toll-free → Regular"),
        ("18005551234", "18775551234", "IndeterminateJurisdiction", "Toll-free → Toll-free"),
        ("13105551234", "18885551234", "IndeterminateJurisdiction", "LA → 888 toll-free"),
    ];
    
    for (ani, dnis, expected, description) in scenarios {
        let jurisdiction = determine_jurisdiction_enhanced(ani, dnis);
        
        if format!("{:?}", jurisdiction) == expected {
            println!("  ✅ {}: {} → {} = {}", description, ani, dnis, expected);
        } else {
            println!("  ❌ {}: {} → {} = {:?} (expected {})", description, ani, dnis, jurisdiction, expected);
        }
    }
}

fn test_international_scenarios() {
    println!("\n🌍 Testing International Call Scenarios");
    
    let scenarios = vec![
        ("12125551234", "01144207946", "IndeterminateJurisdiction", "US → UK"),
        ("01144207946", "12125551234", "IndeterminateJurisdiction", "UK → US"),
        ("13105551234", "011861234567", "IndeterminateJurisdiction", "US → China"),
        ("01152554321", "14155551234", "IndeterminateJurisdiction", "Mexico → US"),
        ("01144", "12125551234", "IndeterminateJurisdiction", "Incomplete international → US"),
    ];
    
    for (ani, dnis, expected, description) in scenarios {
        let jurisdiction = determine_jurisdiction_enhanced(ani, dnis);
        
        if format!("{:?}", jurisdiction) == expected {
            println!("  ✅ {}: {} → {} = {}", description, ani, dnis, expected);
        } else {
            println!("  ❌ {}: {} → {} = {:?} (expected {})", description, ani, dnis, jurisdiction, expected);
        }
    }
}

fn test_canadian_scenarios() {
    println!("\n🍁 Testing Canadian NANPA Scenarios");
    
    let scenarios = vec![
        ("12125551234", "14165551234", "IndeterminateJurisdiction", "US → Toronto"),
        ("14165551234", "12125551234", "IndeterminateJurisdiction", "Toronto → US"),
        ("14165551234", "16045551234", "IndeterminateJurisdiction", "Toronto → Vancouver"),
        ("13105551234", "15145551234", "IndeterminateJurisdiction", "LA → Montreal"),
        ("14035551234", "14035556789", "IndeterminateJurisdiction", "Calgary → Calgary (still indeterminate)"),
    ];
    
    for (ani, dnis, expected, description) in scenarios {
        let jurisdiction = determine_jurisdiction_enhanced(ani, dnis);
        
        if format!("{:?}", jurisdiction) == expected {
            println!("  ✅ {}: {} → {} = {}", description, ani, dnis, expected);
        } else {
            println!("  ❌ {}: {} → {} = {:?} (expected {})", description, ani, dnis, jurisdiction, expected);
        }
    }
}

fn test_premium_services() {
    println!("\n💰 Testing Premium Service Scenarios");
    
    let scenarios = vec![
        ("12125551234", "19005551234", "IndeterminateJurisdiction", "Regular → 900 premium"),
        ("19005551234", "12125551234", "IndeterminateJurisdiction", "900 premium → Regular"),
        ("13105551234", "19765551234", "IndeterminateJurisdiction", "LA → 976 premium"),
        ("17005551234", "12125551234", "IndeterminateJurisdiction", "700 service → Regular"),
        ("15005551234", "14155551234", "IndeterminateJurisdiction", "500 PCS → SF"),
    ];
    
    for (ani, dnis, expected, description) in scenarios {
        let jurisdiction = determine_jurisdiction_enhanced(ani, dnis);
        
        if format!("{:?}", jurisdiction) == expected {
            println!("  ✅ {}: {} → {} = {}", description, ani, dnis, expected);
        } else {
            println!("  ❌ {}: {} → {} = {:?} (expected {})", description, ani, dnis, jurisdiction, expected);
        }
    }
}

fn test_mixed_scenarios() {
    println!("\n🎯 Testing Mixed Valid US Call Scenarios");
    
    let scenarios = vec![
        ("12125551234", "14155555678", "Interstate", "NYC → SF (different states)"),
        ("12125551234", "12125556789", "Interstate", "NYC → NYC (would need NANPA data for Local)"),
        ("13105551234", "14155555678", "Interstate", "LA → SF (same state, would be Intrastate with NANPA data)"),
        ("14155551234", "17025556789", "Interstate", "SF → Vegas (different states)"),
        ("17025551234", "17025556789", "Interstate", "Vegas → Vegas (would need NANPA data for Local)"),
    ];
    
    for (ani, dnis, expected, description) in scenarios {
        let jurisdiction = determine_jurisdiction_enhanced(ani, dnis);
        
        if format!("{:?}", jurisdiction) == expected {
            println!("  ✅ {}: {} → {} = {}", description, ani, dnis, expected);
        } else {
            println!("  ❌ {}: {} → {} = {:?} (expected {})", description, ani, dnis, jurisdiction, expected);
        }
    }
}

// Helper functions for testing

fn is_indeterminate_number(number: &str) -> bool {
    let normalized = normalize_nanpa_number(number);
    
    // Check length first
    if normalized.len() < 10 || normalized.len() > 14 {
        return true; // Invalid length
    }

    // Extract NPA (area code)
    let npa = if normalized.starts_with("1") && normalized.len() >= 4 {
        &normalized[1..4]
    } else if normalized.len() >= 3 {
        &normalized[0..3]
    } else {
        return true; // Too short
    };

    // Check for special NPAs that are Indeterminate
    match npa {
        // Toll-free numbers
        "800" | "833" | "844" | "855" | "866" | "877" | "888" => true,
        
        // Premium rate services
        "900" | "976" => true,
        
        // Personal communication services (may be mobile/wireless)
        "500" => true,
        
        // Special services
        "700" => true, // IC services
        "710" => true, // Government
        "720" => true, // May be VoIP/special
        
        // International access codes
        _ if normalized.starts_with("011") => true, // International
        _ if normalized.starts_with("01") => true,  // International variants
        
        // Canadian numbers (technically NANPA but different jurisdiction rules)
        _ if is_canadian_npa(npa) => true,
        
        // Invalid NPAs (N11, N9X where N=0/1)
        _ if npa.starts_with('0') => true, // 0XX is invalid
        _ if npa.starts_with('1') => true, // 1XX is invalid (except special services)
        _ if npa.ends_with("11") => true,  // N11 is invalid
        
        // Valid US NPAs
        _ => false,
    }
}

fn is_canadian_npa(npa: &str) -> bool {
    // Major Canadian area codes
    let canadian_npas = [
        "204", "226", "236", "249", "250", "289", "306", "343", "365", "367",
        "403", "416", "418", "431", "437", "438", "450", "506", "514", "519",
        "548", "579", "581", "587", "604", "613", "639", "647", "672", "705",
        "709", "778", "780", "782", "807", "819", "825", "867", "873", "902",
        "905", "915"
    ];
    
    canadian_npas.contains(&npa)
}

fn normalize_nanpa_number(number: &str) -> String {
    // Remove any non-digit characters
    let digits: String = number.chars().filter(|c| c.is_digit(10)).collect();
    
    // Handle different formats
    if digits.starts_with("1") && digits.len() == 11 {
        // Already in 1NPANXXNNNN format
        digits
    } else if digits.len() == 10 {
        // Add leading 1 for NPANXXNNNN
        format!("1{}", digits)
    } else if digits.starts_with("011") {
        // International number, not NANPA
        digits
    } else {
        // Return as-is if not recognized
        digits
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallJurisdiction {
    Interstate,
    Intrastate,
    IndeterminateJurisdiction,
    Local,
}

fn determine_jurisdiction_enhanced(ani: &str, dnis: &str) -> CallJurisdiction {
    // First check for special numbers that are always Indeterminate
    if is_indeterminate_number(ani) || is_indeterminate_number(dnis) {
        return CallJurisdiction::IndeterminateJurisdiction;
    }

    // Since we don't have NANPA database info in this test, 
    // assume all valid US numbers are Interstate unless same area code
    let ani_npa = extract_npa(ani);
    let dnis_npa = extract_npa(dnis);
    
    match (ani_npa, dnis_npa) {
        (Some(ani_area), Some(dnis_area)) if ani_area == dnis_area => {
            // Same area code - would need full NANPA data to determine Local vs Intrastate
            CallJurisdiction::Interstate // Conservative assumption for test
        }
        (Some(_), Some(_)) => CallJurisdiction::Interstate,
        _ => CallJurisdiction::IndeterminateJurisdiction,
    }
}

fn extract_npa(number: &str) -> Option<String> {
    let normalized = normalize_nanpa_number(number);
    
    // Check if it's a valid NANPA number
    if normalized.len() == 11 && normalized.starts_with("1") {
        // Extract NPA (positions 1-4, skipping the leading 1)
        Some(normalized[1..4].to_string())
    } else if normalized.len() == 10 {
        // NPANXXNNNN format
        Some(normalized[0..3].to_string())
    } else {
        None
    }
}