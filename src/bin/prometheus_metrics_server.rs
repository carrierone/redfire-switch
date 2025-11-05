//! Prometheus Metrics Server Example
//!
//! This binary demonstrates how to run a standalone Prometheus metrics server
//! with the full monitoring system.
//!
//! Usage:
//!   cargo run --bin prometheus_metrics_server
//!
//! Then access metrics at: http://localhost:9090/metrics

use anyhow::Result;
use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tracing::{info, Level};
use tracing_subscriber;

use redfire_switch::monitoring::{
    MonitoringConfig, MonitoringSystem, PrometheusExporter,
};
use redfire_switch::api::metrics_endpoints::{create_metrics_router, MetricsState};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .init();

    info!("Starting Prometheus Metrics Server...");

    // Create monitoring configuration
    let monitoring_config = MonitoringConfig {
        enabled: true,
        metrics_interval_seconds: 15,  // Collect metrics every 15 seconds
        health_check_interval_seconds: 30,
        alert_evaluation_interval_seconds: 30,
        metrics_retention_hours: 24,
        enable_dashboard: false,  // Disabled for this example
        enable_external_export: true,
        enable_alerting: true,
        notification_endpoints: vec![],
    };

    // Create monitoring system
    let monitoring_system = Arc::new(MonitoringSystem::new(monitoring_config)?);

    // Start monitoring system
    monitoring_system.start().await?;
    info!("Monitoring system started");

    // Create Prometheus exporter
    let prometheus_exporter = Arc::new(PrometheusExporter::new(
        monitoring_system.metrics()
    )?);
    info!("Prometheus exporter initialized");

    // Create metrics state
    let metrics_state = MetricsState {
        prometheus_exporter: prometheus_exporter.clone(),
        monitoring_system: monitoring_system.clone(),
    };

    // Create metrics router
    let app = create_metrics_router(metrics_state);

    // Bind to address
    let addr = SocketAddr::from(([0, 0, 0, 0], 9090));
    info!("Starting HTTP server on {}", addr);
    info!("Metrics available at: http://{}/metrics", addr);
    info!("Health check available at: http://{}/health", addr);

    // Spawn a task to simulate some metrics activity
    let metrics_collector = monitoring_system.metrics();
    tokio::spawn(async move {
        info!("Starting metrics simulation...");

        loop {
            // Simulate some SIP activity
            metrics_collector.counters().total_sip_messages.fetch_add(10, std::sync::atomic::Ordering::Relaxed);
            metrics_collector.counters().invite_messages.fetch_add(2, std::sync::atomic::Ordering::Relaxed);
            metrics_collector.counters().response_2xx.fetch_add(8, std::sync::atomic::Ordering::Relaxed);
            metrics_collector.counters().active_transactions.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            // Simulate some call activity
            metrics_collector.counters().total_calls.fetch_add(2, std::sync::atomic::Ordering::Relaxed);
            metrics_collector.counters().active_calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            metrics_collector.counters().successful_calls.fetch_add(2, std::sync::atomic::Ordering::Relaxed);

            // Simulate some media activity
            metrics_collector.counters().active_media_sessions.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            metrics_collector.counters().rtp_packets_sent.fetch_add(1000, std::sync::atomic::Ordering::Relaxed);
            metrics_collector.counters().rtp_packets_received.fetch_add(950, std::sync::atomic::Ordering::Relaxed);

            time::sleep(Duration::from_secs(5)).await;
        }
    });

    // Start the HTTP server
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!("HTTP server listening on {}", addr);
    info!("Press Ctrl+C to stop");

    axum::serve(listener, app)
        .await?;

    // Cleanup
    monitoring_system.shutdown().await?;
    info!("Monitoring system shut down successfully");

    Ok(())
}
