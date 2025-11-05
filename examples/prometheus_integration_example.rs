//! Example: Prometheus Metrics Integration
//!
//! This example demonstrates how to integrate Prometheus metrics
//! into your Redfire Switch application.

use anyhow::Result;
use redfire_switch::monitoring::{
    MonitoringConfig, MonitoringSystem, PrometheusExporter,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tracing::{info, Level};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("Prometheus Integration Example");
    info!("================================");

    // Step 1: Create monitoring configuration
    let config = MonitoringConfig {
        enabled: true,
        metrics_interval_seconds: 10,
        health_check_interval_seconds: 30,
        alert_evaluation_interval_seconds: 30,
        metrics_retention_hours: 1,
        enable_dashboard: false,
        enable_external_export: true,
        enable_alerting: true,
        notification_endpoints: vec![],
    };

    info!("✓ Created monitoring configuration");

    // Step 2: Create and start monitoring system
    let monitoring = Arc::new(MonitoringSystem::new(config)?);
    monitoring.start().await?;

    info!("✓ Started monitoring system");

    // Step 3: Create Prometheus exporter
    let prometheus = Arc::new(PrometheusExporter::new(monitoring.metrics())?);

    info!("✓ Created Prometheus exporter");

    // Step 4: Get access to metrics counters for updates
    let counters = monitoring.metrics().counters();

    info!("✓ Got metrics counters");
    info!("");
    info!("Simulating SIP switch activity...");
    info!("");

    // Step 5: Simulate some activity
    for i in 1..=10 {
        info!("Cycle {}/10", i);

        // Simulate SIP messages
        counters.total_sip_messages.fetch_add(100, std::sync::atomic::Ordering::Relaxed);
        counters.invite_messages.fetch_add(20, std::sync::atomic::Ordering::Relaxed);
        counters.response_2xx.fetch_add(80, std::sync::atomic::Ordering::Relaxed);
        counters.response_4xx.fetch_add(10, std::sync::atomic::Ordering::Relaxed);
        counters.response_5xx.fetch_add(10, std::sync::atomic::Ordering::Relaxed);

        // Simulate calls
        counters.total_calls.fetch_add(20, std::sync::atomic::Ordering::Relaxed);
        counters.active_calls.store(i * 5, std::sync::atomic::Ordering::Relaxed);
        counters.successful_calls.fetch_add(18, std::sync::atomic::Ordering::Relaxed);
        counters.failed_calls.fetch_add(2, std::sync::atomic::Ordering::Relaxed);

        // Simulate media
        counters.active_media_sessions.store(i * 5, std::sync::atomic::Ordering::Relaxed);
        counters.rtp_packets_sent.fetch_add(10000, std::sync::atomic::Ordering::Relaxed);
        counters.rtp_packets_received.fetch_add(9800, std::sync::atomic::Ordering::Relaxed);

        time::sleep(Duration::from_secs(2)).await;
    }

    info!("");
    info!("Exporting Prometheus metrics...");
    info!("");

    // Step 6: Export metrics
    let metrics_text = prometheus.export_metrics().await?;

    // Print a sample of the metrics
    let lines: Vec<&str> = metrics_text.lines().take(50).collect();
    for line in lines {
        println!("{}", line);
    }

    info!("");
    info!("✓ Successfully exported {} bytes of metrics", metrics_text.len());
    info!("");
    info!("System Status: {:?}", monitoring.get_system_status().await);
    info!("System Uptime: {:?}", monitoring.get_uptime());

    // Step 7: Get latest metrics snapshot
    let latest = monitoring.metrics().get_latest_metrics().await?;
    info!("");
    info!("Latest Metrics Snapshot:");
    info!("  CPU Usage: {:.1}%", latest.system.cpu_usage_percent);
    info!("  Memory Usage: {} MB", latest.system.memory_usage_mb);
    info!("  Active Calls: {}", latest.business.active_calls);
    info!("  Total Calls: {}", latest.business.total_calls);
    info!("  Call Success Rate: {:.1}%", latest.business.call_success_rate);
    info!("  SIP Messages/sec: {:.1}", latest.sip.messages_per_second);

    // Step 8: Check health status
    let health_checker = monitoring.health();
    let health_results = health_checker.check_all_health().await?;

    info!("");
    info!("Health Check Results:");
    for (component, status) in health_results {
        info!("  {}: {:?}", component, status);
    }

    // Step 9: Get active alerts
    let alert_manager = monitoring.alerts();
    let active_alerts = alert_manager.get_active_alerts().await;

    info!("");
    if active_alerts.is_empty() {
        info!("✓ No active alerts");
    } else {
        info!("Active Alerts:");
        for alert in active_alerts {
            info!("  - {}: {} (severity: {:?})", alert.name, alert.description, alert.severity);
        }
    }

    // Step 10: Cleanup
    info!("");
    info!("Shutting down monitoring system...");
    monitoring.shutdown().await?;
    info!("✓ Monitoring system shut down successfully");

    info!("");
    info!("Example completed successfully!");
    info!("");
    info!("Next steps:");
    info!("  1. Run 'cargo run --bin prometheus-metrics-server' to start HTTP server");
    info!("  2. Access metrics at http://localhost:9090/metrics");
    info!("  3. Configure Prometheus to scrape this endpoint");
    info!("  4. Import Grafana dashboard from grafana/dashboards/redfire-overview.json");

    Ok(())
}
