/*
 * Example: Using the Redfire SIP Stack Library
 *
 * This example demonstrates how to use the SIP stack
 * capabilities from the extracted library.
 */

use anyhow::Result;
use redfire_sip_stack::{
    create_default_parser, create_sipt_sipi_service, utils, SipMethod, SipParser,
};

#[tokio::main]
async fn main() -> Result<()> {
    println!("Redfire SIP Stack Demo");
    println!("======================");

    // Create SIP parser using the library
    let parser = create_default_parser();

    // Example SIP INVITE message
    let sip_invite = r#"INVITE sip:alice@example.com SIP/2.0
Via: SIP/2.0/UDP 192.168.1.100:5060;branch=z9hG4bK-redfire-12345
From: Bob <sip:bob@example.org>;tag=redfire-67890
To: Alice <sip:alice@example.com>
Call-ID: redfire-call-12345@example.org
CSeq: 1 INVITE
Contact: <sip:bob@192.168.1.100:5060>
Content-Type: application/sdp
Content-Length: 0

"#;

    // Parse the SIP message
    match parser.parse_message(sip_invite.as_bytes()) {
        Ok(message) => {
            println!("Successfully parsed SIP message:");
            if let Some(method) = &message.method {
                println!("  Method: {:?}", method);

                // Check if this method requires a response
                if utils::method_requires_response(method) {
                    println!("  This method requires a response");
                }
            }

            if let Some(ref uri) = message.request_uri {
                println!("  Request URI: {}", uri);

                // Extract domain and user from URI
                if let Some(domain) = utils::extract_domain(uri) {
                    println!("  Domain: {}", domain);
                }
                if let Some(user) = utils::extract_user(uri) {
                    println!("  User: {}", user);
                }
            }

            println!("  Headers: {}", message.headers.len());
        }
        Err(e) => {
            println!("Failed to parse SIP message: {}", e);
        }
    }

    // Demonstrate SIP-T/SIP-I service
    println!("\nSIP-T/SIP-I Demo:");
    let sipt_service = create_sipt_sipi_service();
    println!("  SIP-T enabled: {}", sipt_service.is_sipt_enabled());
    println!("  SIP-I enabled: {}", sipt_service.is_sipi_enabled());

    // Generate some SIP utilities
    println!("\nSIP Utilities Demo:");
    println!("  Generated Call-ID: {}", utils::generate_call_id());
    println!("  Generated Branch: {}", utils::generate_branch());
    println!("  Generated Tag: {}", utils::generate_tag());

    // URI validation
    let test_uris = vec![
        "sip:alice@example.com",
        "sips:bob@secure.example.com",
        "http://example.com",
        "tel:+15551234567",
    ];

    println!("\nURI Validation:");
    for uri in test_uris {
        let valid = utils::validate_sip_uri(uri);
        println!(
            "  {} -> {}",
            uri,
            if valid {
                "Valid SIP URI"
            } else {
                "Invalid SIP URI"
            }
        );
    }

    println!("\nDemo completed successfully!");

    Ok(())
}
