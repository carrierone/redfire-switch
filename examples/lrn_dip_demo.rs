/// Example demonstrating SIP 302 redirect LRN dip functionality
/// 
/// This example shows how to:
/// 1. Configure LRN dip settings
/// 2. Initialize the LRN dip service
/// 3. Perform LRN lookups with SIP 302 redirects
/// 4. Handle caching and error conditions

use anyhow::Result;
use std::net::IpAddr;
use tokio::time::Duration;

use redfire_switch::lcr::lrn_dip::LrnDipService;
use redfire_switch::lcr::types::{LrnDipConfig, LrnDipServer, LrnAuthConfig};
use redfire_switch::config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("LRN Dip Service Demo");
    println!("===================");

    // Example 1: Basic LRN dip configuration with backup servers
    println!("\n1. LRN Dip Configuration with Backup Servers");
    let lrn_config = LrnDipConfig {
        server_ip: Some("192.168.1.200".parse()?), // Backwards compatibility
        server_port: 5060,
        servers: vec![
            LrnDipServer {
                server_ip: "192.168.1.200".parse()?,
                server_port: 5060,
                priority: 0, // Primary server
                protocol: "sip_302".to_string(),
                auth: None,
            },
            LrnDipServer {
                server_ip: "192.168.1.201".parse()?,
                server_port: 5060,
                priority: 1, // Backup server  
                protocol: "sip_302".to_string(),
                auth: None,
            },
        ],
        local_ip: Some("0.0.0.0".parse()?),
        local_port: Some(0), // Use random port
        timeout_ms: 5000,
        backup_timeout_ms: Some(2000), // Try backup after 2 seconds
        max_redirects: 3,
        enabled: true,
        cache_timeout_sec: 3600, // 1 hour cache
        load_balancing: "priority".to_string(), // Failover mode
    };

    println!("Primary LRN Server: {}:{}", 
             lrn_config.server_ip.unwrap_or("127.0.0.1".parse().unwrap()), 
             lrn_config.server_port);
    println!("Backup Servers: {} configured", lrn_config.servers.len());
    println!("Timeout: {}ms (backup: {}ms)", 
             lrn_config.timeout_ms, 
             lrn_config.get_backup_timeout_ms());
    println!("Load Balancing: {}", lrn_config.load_balancing);
    println!("Max Redirects: {}", lrn_config.max_redirects);
    println!("Cache Duration: {}s", lrn_config.cache_timeout_sec);

    // Example 2: Create and initialize LRN service
    println!("\n2. Initialize LRN Service");
    let service = LrnDipService::new(lrn_config.clone());
    
    if service.is_enabled() {
        println!("✓ LRN dip service is enabled");
        // Note: In a real scenario, you would call service.initialize().await
        // but that requires an actual network interface
        println!("  (Would initialize SIP client socket)");
    } else {
        println!("✗ LRN dip service is disabled");
    }

    // Example 3: Round-robin load balancing
    println!("\n3. Round-Robin Load Balancing");
    let round_robin_config = LrnDipConfig {
        servers: vec![
            LrnDipServer { 
                server_ip: "192.168.1.200".parse()?, 
                server_port: 5060, 
                priority: 0,
                protocol: "sip_302".to_string(),
                auth: None,
            },
            LrnDipServer { 
                server_ip: "api.telique.com".parse()?, 
                server_port: 443, 
                priority: 0,
                protocol: "telique_api".to_string(),
                auth: Some(LrnAuthConfig {
                    auth_type: "api_key".to_string(),
                    username: Some("api_key".to_string()),
                    password: None,
                    token: Some("your-telique-key".to_string()),
                }),
            },
            LrnDipServer { 
                server_ip: "192.168.1.202".parse()?, 
                server_port: 5060, 
                priority: 0,
                protocol: "sip_302".to_string(),
                auth: None,
            },
        ],
        load_balancing: "round_robin".to_string(),
        timeout_ms: 3000,
        backup_timeout_ms: Some(1500),
        enabled: true,
        cache_timeout_sec: 1800,
        ..Default::default()
    };
    
    println!("Mixed protocols: SIP 302 + Telique API");
    println!("Round-robin with {} servers", round_robin_config.servers.len());
    println!("Each request will go to next server in rotation");
    println!("Protocols supported: SIP 302, Telique API, REST API, SOAP");
    println!("Timeout per server: {}ms", round_robin_config.timeout_ms);

    // Example 4: Configuration from file
    println!("\n4. Configuration from File");
    println!("Example configuration file format:");
    println!(r#"
{{
  "lrn_dip": {{
    "servers": [
      {{ "server_ip": "192.168.1.200", "server_port": 5060, "priority": 0 }},
      {{ "server_ip": "192.168.1.201", "server_port": 5060, "priority": 1 }}
    ],
    "timeout_ms": 5000,
    "backup_timeout_ms": 2000,
    "max_redirects": 3,
    "enabled": true,
    "cache_timeout_sec": 3600,
    "load_balancing": "priority"
  }}
}}"#);

    // Example 9: Demonstrate different number formats
    println!("\n5. Number Normalization");
    let test_numbers = vec![
        "2125551234",
        "12125551234", 
        "+1-212-555-1234",
        "1 (212) 555-1234",
        "212.555.1234",
    ];

    for number in test_numbers {
        let normalized = normalize_tn_demo(number);
        println!("  {} -> {}", number, normalized);
    }

    // Example 9: Simulate LRN dip responses
    println!("\n5. Simulated LRN Dip Responses");
    
    // Simulate successful 302 redirect
    println!("\nScenario A: 302 Redirect with LRN");
    let response_302 = r#"SIP/2.0 302 Moved Temporarily
Contact: <sip:12135551234@lrn.example.com>
X-SPID: 1234
Content-Length: 0

"#;
    
    if let Some(lrn) = extract_lrn_from_302(response_302) {
        println!("  Original: 12025551234");
        println!("  LRN Found: {}", lrn);
        println!("  Status: Ported number");
    }

    // Simulate 200 OK with headers
    println!("\nScenario B: 200 OK with X-LRN Header");
    let response_200 = r#"SIP/2.0 200 OK
X-LRN: 17035552222
X-SPID: 5678
Content-Length: 0

"#;
    
    if let Some(lrn) = extract_lrn_from_headers(response_200) {
        println!("  Original: 17035551111");
        println!("  LRN Found: {}", lrn);
        println!("  SPID: 5678");
        println!("  Status: Ported number");
    }

    // Simulate 404 Not Found
    println!("\nScenario C: 404 Not Found (Non-ported)");
    println!("  Original: 19995551234");
    println!("  LRN Found: None");
    println!("  Status: Not ported (use original number)");

    // Example 6: Cache management
    println!("\n6. Cache Management");
    println!("Cache operations:");
    println!("  - Successful LRN dips are cached for {} seconds", lrn_config.cache_timeout_sec);
    println!("  - Subsequent dips for same number return cached result");
    println!("  - Expired entries are cleaned up automatically");
    println!("  - Cache statistics available via get_cache_stats()");

    // Example 7: Backup server and failover
    println!("\n7. Backup Server & Load Balancing");
    println!("Load balancing strategies:");
    println!("  - Priority/Failover: Try primary first, then backup servers in order");
    println!("  - Round-robin: Distribute requests evenly across all servers");
    println!("Timeout behavior:");
    println!("  - Primary server: Full timeout ({}ms)", lrn_config.timeout_ms);
    println!("  - Backup servers: Reduced timeout ({}ms)", lrn_config.get_backup_timeout_ms());
    println!("  - Automatic failover on timeout or error");

    // Example 8: Error handling
    println!("\n8. Error Handling");
    println!("Common error scenarios:");
    println!("  - Primary server timeout: Try backup servers automatically");
    println!("  - All servers unreachable: Falls back to original number");
    println!("  - Invalid SIP response: Try next server in priority order");
    println!("  - Service disabled: Returns disabled status");
    println!("  - Network issues: Automatic retry with backup servers");

    // Example 9: Integration with routing
    println!("\n9. Integration with Routing Engine");
    println!("The LRN dip service integrates with the routing engine:");
    println!("  - Called during jurisdiction determination");
    println!("  - LRN replaces DNIS for rate lookups");
    println!("  - Cached results improve performance");
    println!("  - Fallback to original number on errors");

    println!("\nDemo completed successfully!");
    println!("\nTo use LRN dipping in production:");
    println!("1. Configure your LRN server IP and port in config file");
    println!("2. Set enabled: true in lrn_dip section");
    println!("3. Initialize LcrEngine with LRN config");
    println!("4. The routing engine will automatically perform LRN dips");

    Ok(())
}

/// Demonstrate telephone number normalization
fn normalize_tn_demo(tn: &str) -> String {
    let digits: String = tn.chars().filter(|c| c.is_ascii_digit()).collect();
    
    // Convert to 11-digit format (1NPANXXNNNN)
    if digits.starts_with('1') && digits.len() == 11 {
        digits
    } else if digits.len() == 10 {
        format!("1{}", digits)
    } else {
        digits
    }
}

/// Extract LRN from SIP 302 Contact header
fn extract_lrn_from_302(response: &str) -> Option<String> {
    for line in response.lines() {
        if line.to_lowercase().starts_with("contact:") {
            if let Some(start) = line.find("sip:") {
                let uri_part = &line[start + 4..];
                if let Some(end) = uri_part.find('@') {
                    let user_part = &uri_part[..end];
                    if user_part.len() >= 10 && user_part.chars().all(|c| c.is_ascii_digit()) {
                        return Some(user_part.to_string());
                    }
                }
            }
        }
    }
    None
}

/// Extract LRN from SIP headers (X-LRN, P-LRN)
fn extract_lrn_from_headers(response: &str) -> Option<String> {
    for line in response.lines() {
        let line_lower = line.to_lowercase();
        if line_lower.starts_with("x-lrn:") || line_lower.starts_with("p-lrn:") {
            if let Some(value) = line.split(':').nth(1) {
                let lrn = value.trim();
                if lrn.len() >= 10 && lrn.chars().all(|c| c.is_ascii_digit()) {
                    return Some(lrn.to_string());
                }
            }
        }
    }
    None
}