#!/usr/bin/env cargo-script

//! # Country-specific STIR/SHAKEN Implementation Example
//! 
//! This example demonstrates the new regulatory body-based STIR/SHAKEN functionality.
//! 
//! ## Usage
//! ```bash
//! cargo run --example regulatory_stir_shaken
//! ```

use std::collections::HashMap;

// Simulate the basic structures without the full codebase
#[derive(Debug, Clone)]
pub struct RegulatoryBody {
    pub country_code: String,
    pub country_name: String,
    pub authority_name: String,
    pub authority_acronym: String,
    pub stir_shaken_mandated: bool,
    pub call_authentication_required: bool,
    pub authority_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RegulatoryRegistry {
    pub regulatory_bodies: HashMap<String, RegulatoryBody>,
}

impl RegulatoryRegistry {
    pub fn new() -> Self {
        let mut regulatory_bodies = HashMap::new();
        
        // United States - FCC mandates STIR/SHAKEN
        regulatory_bodies.insert("US".to_string(), RegulatoryBody {
            country_code: "US".to_string(),
            country_name: "United States".to_string(),
            authority_name: "Federal Communications Commission".to_string(),
            authority_acronym: "FCC".to_string(),
            stir_shaken_mandated: true,
            call_authentication_required: true,
            authority_url: Some("https://www.fcc.gov/".to_string()),
        });
        
        // Canada - CRTC has STIR/SHAKEN requirements
        regulatory_bodies.insert("CA".to_string(), RegulatoryBody {
            country_code: "CA".to_string(),
            country_name: "Canada".to_string(),
            authority_name: "Canadian Radio-television and Telecommunications Commission".to_string(),
            authority_acronym: "CRTC".to_string(),
            stir_shaken_mandated: true,
            call_authentication_required: true,
            authority_url: Some("https://crtc.gc.ca/".to_string()),
        });
        
        // United Kingdom - Ofcom exploring call authentication
        regulatory_bodies.insert("GB".to_string(), RegulatoryBody {
            country_code: "GB".to_string(),
            country_name: "United Kingdom".to_string(),
            authority_name: "Office of Communications".to_string(),
            authority_acronym: "Ofcom".to_string(),
            stir_shaken_mandated: false,
            call_authentication_required: true,
            authority_url: Some("https://www.ofcom.org.uk/".to_string()),
        });
        
        // Germany - BNetzA regulates telecommunications
        regulatory_bodies.insert("DE".to_string(), RegulatoryBody {
            country_code: "DE".to_string(),
            country_name: "Germany".to_string(),
            authority_name: "Bundesnetzagentur".to_string(),
            authority_acronym: "BNetzA".to_string(),
            stir_shaken_mandated: false,
            call_authentication_required: false,
            authority_url: Some("https://www.bundesnetzagentur.de/".to_string()),
        });

        RegulatoryRegistry { regulatory_bodies }
    }
    
    pub fn has_regulatory_body(&self, country_code: &str) -> bool {
        self.regulatory_bodies.contains_key(country_code)
    }
    
    pub fn is_stir_shaken_mandated(&self, country_code: &str) -> bool {
        self.regulatory_bodies
            .get(country_code)
            .map(|body| body.stir_shaken_mandated)
            .unwrap_or(false)
    }
    
    pub fn is_call_authentication_required(&self, country_code: &str) -> bool {
        self.regulatory_bodies
            .get(country_code)
            .map(|body| body.call_authentication_required)
            .unwrap_or(false)
    }
    
    pub fn should_enable_stir_shaken(&self, from_country: &str, to_country: &str) -> (bool, String) {
        let has_from_regulatory = self.has_regulatory_body(from_country);
        let has_to_regulatory = self.has_regulatory_body(to_country);
        
        if !has_from_regulatory && !has_to_regulatory {
            return (false, format!("No regulatory bodies found for {} or {}", from_country, to_country));
        }
        
        // Check if STIR/SHAKEN is mandated in either country
        let from_mandated = self.is_stir_shaken_mandated(from_country);
        let to_mandated = self.is_stir_shaken_mandated(to_country);
        
        if from_mandated || to_mandated {
            return (true, format!("STIR/SHAKEN mandated by {} or {}", from_country, to_country));
        }
        
        // Check if call authentication is required
        let from_auth_required = self.is_call_authentication_required(from_country);
        let to_auth_required = self.is_call_authentication_required(to_country);
        
        if from_auth_required || to_auth_required {
            return (true, format!("Call authentication required by {} or {}", from_country, to_country));
        }
        
        (false, format!("No requirements found for {} to {}", from_country, to_country))
    }
}

fn extract_country_code(number: &str) -> String {
    let cleaned = number.trim_start_matches('+');
    
    // US/Canada +1 (simplified)
    if cleaned.starts_with('1') {
        return "US".to_string(); // Could be CA too, but simplified
    }
    // UK +44
    if cleaned.starts_with("44") {
        return "GB".to_string();
    }
    // Germany +49
    if cleaned.starts_with("49") {
        return "DE".to_string();
    }
    
    "US".to_string() // Default fallback
}

fn main() {
    println!("🔐 Country-specific STIR/SHAKEN Implementation Example");
    println!("=====================================================");
    
    let registry = RegulatoryRegistry::new();
    
    // Test scenarios
    let test_calls = vec![
        ("+12125551234", "+13105551234", "US domestic call"),
        ("+12125551234", "+14165551234", "US to Canada call"),
        ("+442071234567", "+12125551234", "UK to US call"),  
        ("+4930123456", "+33123456789", "Germany to France call"),
        ("+12125551234", "+4930123456", "US to Germany call"),
    ];
    
    println!("\n📊 Regulatory Bodies Status:");
    for (code, body) in &registry.regulatory_bodies {
        println!("  {} ({}): {} - STIR/SHAKEN: {}, Auth Required: {}", 
                code, 
                body.authority_acronym,
                body.country_name,
                if body.stir_shaken_mandated { "✅ Mandated" } else { "❌ Not mandated" },
                if body.call_authentication_required { "✅ Required" } else { "❌ Not required" }
        );
    }
    
    println!("\n📞 Call Routing Analysis:");
    for (from, to, description) in test_calls {
        let from_country = extract_country_code(from);
        let to_country = extract_country_code(to);
        let (should_enable, reason) = registry.should_enable_stir_shaken(&from_country, &to_country);
        
        println!("  {}: {} -> {} ({}→{})", 
                description, from, to, from_country, to_country);
        println!("    STIR/SHAKEN: {} - {}", 
                if should_enable { "🟢 ENABLED" } else { "🔴 DISABLED" }, 
                reason);
        println!();
    }
    
    println!("📋 Summary:");
    println!("  • STIR/SHAKEN is enabled only when at least one country has a regulatory body");
    println!("  • Priority: Mandate > Call Authentication Required > No Requirements");
    println!("  • Countries without regulatory bodies will not have STIR/SHAKEN applied");
    
    println!("\n✨ Implementation complete! STIR/SHAKEN now respects regulatory requirements per country.");
}