/*
 * Redfire Switch - A Class 4 SIP Telephone Switch
 * Copyright (C) 2025 Carrier One Inc and contributors
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

use anyhow::Result;
use clap::{Arg, Command};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let matches = Command::new("redfire-switch")
        .version("0.1.0")
        .author("Carrier One Inc <info@carrierone.com>")
        .about("A Class 4 SIP Telephone Switch")
        .arg(
            Arg::new("mode")
                .short('m')
                .long("mode")
                .value_name("MODE")
                .help("Operation mode")
                .value_parser(["b2bua", "demo", "test"])
                .default_value("demo"),
        )
        .arg(
            Arg::new("bind")
                .short('b')
                .long("bind")
                .value_name("ADDR")
                .help("Bind address")
                .default_value("0.0.0.0:5060"),
        )
        .get_matches();

    // FIXED: Replace unwrap() with proper error handling
    let mode = matches
        .get_one::<String>("mode")
        .ok_or_else(|| anyhow::anyhow!("Mode parameter is required"))?;
    let _bind_addr = matches
        .get_one::<String>("bind")
        .ok_or_else(|| anyhow::anyhow!("Bind address parameter is required"))?;

    match mode.as_str() {
        "demo" => {
            println!("🔥 RedFire Switch - Class 4 SIP Switch");
            println!("======================================");
            println!();
            println!("Available binaries:");
            println!("  • lcr_sip_server         - LCR-routing SIP server (place real calls)");
            println!("  • simple-b2bua-test      - Basic SIP B2BUA forwarding");
            println!("  • sipi-automated-tests   - SIP-I/SIP-T automated testing");
            println!("  • standalone_api_server  - Configuration/management REST API");
            println!("  • redfire-web-ui         - Web administration UI");
            println!("  • lcr_cli                - Least-cost-routing CLI");
            println!();
            println!("Run: cargo run --bin <binary-name>");
            println!("Test calling: cargo test --test sip_call_flow_tests");
        }
        "b2bua" => {
            println!("🔥 RedFire Switch - B2BUA Mode");
            println!("Use: cargo run --bin simple-b2bua-test");
        }
        "test" => {
            println!("🔥 RedFire Switch - Test Mode");
            println!("Automated SIP call flow: cargo test --test sip_call_flow_tests");
            println!("SIP-I/SIP-T tests:       cargo run --bin sipi-automated-tests");
        }
        _ => {
            println!("Unknown mode: {}", mode);
        }
    }

    Ok(())
}
