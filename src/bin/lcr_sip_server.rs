/*
 * LCR-Enabled SIP Server (binary entry point).
 *
 * Thin wrapper around `redfire_switch::sip_call_server::LcrSipServer`, which
 * contains the actual routing/call-flow logic (shared with the automated SIP
 * call-flow tests). This binary just parses CLI args, installs a Ctrl+C handler,
 * and runs the server.
 */

use anyhow::{anyhow, Result};
use clap::{Arg, Command};
use redfire_switch::sip_call_server::LcrSipServer;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let matches = Command::new("lcr-sip-server")
        .version("1.0")
        .about("LCR-enabled SIP Server for call routing")
        .arg(
            Arg::new("bind")
                .short('b')
                .long("bind")
                .value_name("ADDR:PORT")
                .help("Bind address and port")
                .default_value("0.0.0.0:5060"),
        )
        .arg(
            Arg::new("database-url")
                .long("database-url")
                .value_name("URL")
                .help("PostgreSQL database URL (can be set via DATABASE_URL env var)")
                .default_value("postgresql://postgres:postgres@localhost:5432/lcr"),
        )
        .get_matches();

    let bind_addr_str = matches
        .get_one::<String>("bind")
        .expect("bind argument should have a default value");
    let database_url = matches
        .get_one::<String>("database-url")
        .map(|s| s.clone())
        .or_else(|| std::env::var("DATABASE_URL").ok())
        .unwrap_or_else(|| "postgresql://postgres:postgres@localhost:5432/lcr".to_string());

    let bind_addr: SocketAddr = bind_addr_str
        .parse()
        .map_err(|_| anyhow!("Invalid bind address: {}", bind_addr_str))?;

    info!("🔥 Starting Redfire LCR SIP Server");
    info!("📍 Bind Address: {}", bind_addr);
    info!("🗄️  Database: {}", database_url);

    let server = Arc::new(LcrSipServer::new(bind_addr, &database_url).await?);

    // Install a Ctrl+C handler that flips the server's shutdown flag.
    let shutdown = server.shutdown_handle();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            info!("Received Ctrl+C, shutting down gracefully");
            shutdown.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    });

    server.run().await?;

    Ok(())
}
