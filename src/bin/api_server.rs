/*
 * Redfire Switch - API Server Binary
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
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;
use tracing::{error, info};
use tracing_subscriber;

use redfire_switch::api::{
    auth::AuthConfig,
    config::{ApiServerConfig, ApiSettings, HttpListener, HttpProtocol, UnixListener},
    server::{
        start_custom_server, start_development_server, start_production_server,
        start_unix_only_server,
    },
};
use redfire_switch::monitor::SipMonitor;
use redfire_switch::rest_api::AppState;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let matches = Command::new("redfire-api-server")
        .version("1.0.0")
        .author("Carrier One Inc <info@carrierone.com>")
        .about("Redfire Switch REST API Server")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path")
                .value_parser(clap::value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("mode")
                .short('m')
                .long("mode")
                .value_name("MODE")
                .help("Server mode")
                .value_parser(["development", "production", "unix-only", "custom"])
                .default_value("development"),
        )
        .arg(
            Arg::new("bind-http")
                .long("bind-http")
                .value_name("ADDR:PORT")
                .help("HTTP bind address (e.g., 127.0.0.1:8080)")
                .conflicts_with("mode"),
        )
        .arg(
            Arg::new("bind-unix")
                .long("bind-unix")
                .value_name("PATH")
                .help("Unix socket path")
                .value_parser(clap::value_parser!(PathBuf))
                .conflicts_with("mode"),
        )
        .arg(
            Arg::new("enable-ipv6")
                .long("enable-ipv6")
                .help("Enable IPv6 listener")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("jwt-secret")
                .long("jwt-secret")
                .value_name("SECRET")
                .help("JWT secret key for authentication"),
        )
        .arg(
            Arg::new("admin-password")
                .long("admin-password")
                .value_name("PASSWORD")
                .help("Set custom admin password (default: admin123)"),
        )
        .get_matches();

    let mode = matches.get_one::<String>("mode").unwrap();

    info!("🔥 RedFire Switch API Server v1.0.0");
    info!("Mode: {}", mode);

    // Create auth configuration
    let mut auth_config = AuthConfig::default();
    if let Some(jwt_secret) = matches.get_one::<String>("jwt-secret") {
        auth_config.jwt_secret = jwt_secret.clone();
    }

    // Create application state
    let mut app_state = AppState::with_auth_config(auth_config);

    // Add config reload callback
    app_state = app_state.with_config_reload_callback(|| {
        info!("Configuration reload requested");
        // In a real implementation, this would reload actual configuration files
        Ok(())
    });

    match mode.as_str() {
        "development" => {
            info!("Starting development server...");
            info!("API will be available at:");
            info!("  HTTP: http://127.0.0.1:8080");
            info!("  Unix: /tmp/redfire-switch-dev.sock");
            info!("  Swagger UI: http://127.0.0.1:8080/swagger-ui");
            info!("");
            info!("Default credentials:");
            info!("  Username: admin");
            info!("  Password: admin123");
            info!("");
            info!("⚠️  DEVELOPMENT MODE - NOT FOR PRODUCTION USE");

            start_development_server(app_state).await?;
        }

        "production" => {
            info!("Starting production server...");
            info!("API will be available at:");
            info!("  HTTPS: https://127.0.0.1:8443");
            info!("  Unix: /var/run/redfire-switch/api.sock");
            info!("");
            info!("⚠️  Ensure TLS certificates are configured");

            start_production_server(app_state).await?;
        }

        "unix-only" => {
            info!("Starting Unix-only server...");
            info!("API will be available at:");
            info!("  Unix: /var/run/redfire-switch/api.sock");
            info!("");
            info!("Use curl with --unix-socket for testing");

            start_unix_only_server(app_state).await?;
        }

        "custom" => {
            let mut config = ApiServerConfig::development();

            // Customize configuration based on CLI arguments
            if let Some(http_bind) = matches.get_one::<String>("bind-http") {
                if let Ok(addr) = http_bind.parse::<std::net::SocketAddr>() {
                    config.http_listeners = vec![HttpListener {
                        enabled: true,
                        bind_address: addr.ip(),
                        port: addr.port(),
                        protocol: HttpProtocol::Http,
                        name: "custom-http".to_string(),
                        description: "Custom HTTP listener".to_string(),
                    }];
                } else {
                    error!("Invalid HTTP bind address: {}", http_bind);
                    return Ok(());
                }
            }

            if let Some(unix_path) = matches.get_one::<PathBuf>("bind-unix") {
                config.unix_listeners = vec![UnixListener {
                    enabled: true,
                    socket_path: unix_path.clone(),
                    name: "custom-unix".to_string(),
                    description: "Custom Unix socket".to_string(),
                    file_permissions: 0o600,
                }];
            }

            if matches.get_flag("enable-ipv6") {
                config.http_listeners.push(HttpListener {
                    enabled: true,
                    bind_address: IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)),
                    port: 8080,
                    protocol: HttpProtocol::Http,
                    name: "ipv6-listener".to_string(),
                    description: "IPv6 HTTP listener".to_string(),
                });
            }

            info!("Starting custom server...");
            for listener in &config.http_listeners {
                if listener.enabled {
                    info!(
                        "  HTTP: {}://{}:{}",
                        match listener.protocol {
                            HttpProtocol::Http => "http",
                            HttpProtocol::Https => "https",
                        },
                        listener.bind_address,
                        listener.port
                    );
                }
            }
            for listener in &config.unix_listeners {
                if listener.enabled {
                    info!("  Unix: {}", listener.socket_path.display());
                }
            }

            start_custom_server(config, app_state).await?;
        }

        _ => {
            error!("Unknown mode: {}", mode);
            return Ok(());
        }
    }

    Ok(())
}
