/*
 * Redfire Switch - Simple API Server (Working Demo)
 * Copyright (C) 2025 Carrier One Inc and contributors
 */

use anyhow::Result;
use axum::serve;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber;

use redfire_switch::api::simplified_server::create_simple_api_router;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("🔥 RedFire Switch Simple API Server v1.0.0");

    let app = create_simple_api_router();
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));

    info!("Server starting on http://{}", addr);
    info!("API Documentation: http://{}/swagger-ui", addr);
    info!("");
    info!("Available endpoints:");
    info!("  GET  /api/v1/system/stats    - System statistics");
    info!("  POST /api/v1/auth/login      - User authentication");
    info!("");
    info!("Demo credentials:");
    info!("  Username: admin");
    info!("  Password: admin123");
    info!("");

    let listener = TcpListener::bind(addr).await?;
    serve(listener, app).await?;

    Ok(())
}
