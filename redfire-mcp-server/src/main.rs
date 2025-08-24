/*
 * Redfire MCP Server - AI Integration for Telecommunications
 * Copyright (C) 2025 Carrier One Inc and contributors
 *
 * MCP (Model Context Protocol) server providing AI access to Redfire Switch
 * telecommunications capabilities including SIP operations and codec transcoding.
 */

use anyhow::Result;
use clap::Parser;
use tracing::info;

mod codec_tools;
mod mcp_handlers;
mod sip_tools;

use mcp_handlers::RedfireMcpServer;

#[derive(Parser)]
#[command(name = "redfire-mcp")]
#[command(about = "Redfire Switch MCP Server for AI Integration")]
#[command(version)]
struct Args {
    /// Port to listen on
    #[arg(short, long, default_value = "3000")]
    port: u16,

    /// Host to bind to
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Enable GPU acceleration
    #[arg(long)]
    gpu: bool,

    /// GPU device ID
    #[arg(long, default_value = "0")]
    gpu_device: u32,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,

    /// Configuration file
    #[arg(short, long)]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.debug { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_max_level(if args.debug {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .init();

    info!("Starting Redfire MCP Server v{}", env!("CARGO_PKG_VERSION"));
    info!("Listening on {}:{}", args.host, args.port);

    if args.gpu {
        info!("GPU acceleration enabled (device: {})", args.gpu_device);
    } else {
        info!("GPU acceleration disabled");
    }

    // Initialize MCP server
    let server = RedfireMcpServer::new(args.gpu, args.gpu_device).await?;

    // Start the server
    let addr = format!("{}:{}", args.host, args.port);
    server.run(&addr).await?;

    Ok(())
}
