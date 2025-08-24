/*
 * Enterprise B2BUA Demo - Comprehensive showcase of all integrated features
 * Demonstrates ML threat detection, clustering, security monitoring, and operational dashboard
 */

use anyhow::Result;
use colored::*;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info};

use redfire_switch::cluster_management::ClusterConfig;
use redfire_switch::enterprise_b2bua::{EnterpriseB2BUA, EnterpriseB2BUAConfig};
use redfire_switch::ml_threat_detection::MLThreatConfig;
use redfire_switch::operational_dashboard::DashboardConfig;
use redfire_switch::security_monitor::SecurityMonitorConfig;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    print_enterprise_banner();

    info!("🚀 Starting Enterprise B2BUA Demo with all integrated features");

    // Create enterprise configuration
    let config = create_enterprise_config();

    // Initialize enterprise B2BUA
    let enterprise_b2bua = EnterpriseB2BUA::new(config).await?;

    // Start all enterprise systems
    enterprise_b2bua.start().await?;

    info!("✅ Enterprise B2BUA fully operational");

    // Run enterprise demonstration
    run_enterprise_demo(&enterprise_b2bua).await?;

    Ok(())
}

fn print_enterprise_banner() {
    println!(
        "\n{}",
        "╔══════════════════════════════════════════════════════════════════════════════╗"
            .bright_red()
    );
    println!(
        "{}",
        "║                                                                              ║"
            .bright_red()
    );
    println!(
        "{}",
        "║    🔥🏢 REDFIRE SWITCH ENTERPRISE B2BUA DEMONSTRATION 🏢🔥                  ║"
            .bright_red()
    );
    println!(
        "{}",
        "║                                                                              ║"
            .bright_red()
    );
    println!(
        "{}",
        "║         🛡️  ML Threat Detection    📊 Real-time Dashboard                  ║"
            .bright_yellow()
    );
    println!(
        "{}",
        "║         🏢 High-Availability       🔍 Security Monitoring                  ║"
            .bright_yellow()
    );
    println!(
        "{}",
        "║         📞 Enterprise B2BUA        🤖 Predictive Analytics                 ║"
            .bright_yellow()
    );
    println!(
        "{}",
        "║                                                                              ║"
            .bright_red()
    );
    println!(
        "{}",
        "║                    COMPLETE CARRIER-GRADE ECOSYSTEM                         ║"
            .bright_green()
    );
    println!(
        "{}",
        "║                                                                              ║"
            .bright_red()
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════════════════════════════════════╝"
            .bright_red()
    );
    println!();
}

fn create_enterprise_config() -> EnterpriseB2BUAConfig {
    EnterpriseB2BUAConfig {
        enabled: true,
        bind_address: "0.0.0.0".to_string(),
        bind_port: 5061,
        max_concurrent_calls: 10000,
        enable_stir_shaken: true,
        enable_sip_i: true,

        // Enhanced security configuration
        security_config: SecurityMonitorConfig {
            enabled: true,
            log_security_events: true,
            auto_block_enabled: true,
            max_messages_per_second: 100,
            max_messages_per_minute: 1000,
            block_duration_minutes: 60,
            threat_score_threshold: 10,
            oversized_message_threshold: 8192,
            monitoring_window_minutes: 60,
        },

        // Advanced dashboard configuration
        dashboard_config: DashboardConfig {
            enabled: true,
            metrics_retention_hours: 168, // 7 days
            alert_retention_hours: 720,   // 30 days
            performance_monitoring: true,
            security_monitoring: true,
            call_quality_monitoring: true,
            auto_alerting: true,
            dashboard_refresh_seconds: 5,
        },

        // High-availability cluster configuration
        cluster_config: ClusterConfig {
            enabled: true,
            node_name: "enterprise-demo-primary".to_string(),
            cluster_bind_port: 7946,
            heartbeat_interval_seconds: 5,
            heartbeat_timeout_seconds: 15,
            call_state_sync_enabled: true,
            failover_timeout_seconds: 30,
            split_brain_detection: true,
            quorum_size: 2,
            auto_failover: true,
            load_balancing_enabled: true,
            geographic_distribution: false,
        },

        // Advanced ML threat detection
        ml_threat_config: MLThreatConfig {
            enabled: true,
            anomaly_detection_enabled: true,
            pattern_recognition_enabled: true,
            behavioral_analysis_enabled: true,
            predictive_blocking_enabled: true,
            learning_rate: 0.01,
            confidence_threshold: 0.85,
            feature_window_size: 100,
            model_update_interval_minutes: 60,
            false_positive_threshold: 0.05,
            adaptive_learning: true,
        },

        call_timeout_seconds: 300,
        health_check_interval_seconds: 30,
    }
}

async fn run_enterprise_demo(b2bua: &EnterpriseB2BUA) -> Result<()> {
    info!("🎯 Running comprehensive enterprise demonstration...");

    // Phase 1: System Initialization Validation
    println!(
        "\n{}",
        "Phase 1: System Initialization & Health Check"
            .bright_cyan()
            .bold()
    );
    println!(
        "{}",
        "═══════════════════════════════════════════════".bright_cyan()
    );

    sleep(Duration::from_secs(2)).await;
    let initial_stats = b2bua.get_enterprise_stats().await?;
    print_system_status(&initial_stats);

    // Phase 2: Security & ML Demonstration
    println!(
        "\n{}",
        "Phase 2: Security & ML Threat Detection Demo"
            .bright_yellow()
            .bold()
    );
    println!(
        "{}",
        "═════════════════════════════════════════════════".bright_yellow()
    );

    simulate_security_scenarios(b2bua).await?;

    // Phase 3: Call Processing Demonstration
    println!(
        "\n{}",
        "Phase 3: Enterprise Call Processing Demo"
            .bright_green()
            .bold()
    );
    println!(
        "{}",
        "════════════════════════════════════════════════".bright_green()
    );

    simulate_call_scenarios(b2bua).await?;

    // Phase 4: Monitoring & Analytics
    println!(
        "\n{}",
        "Phase 4: Real-time Monitoring & Analytics"
            .bright_blue()
            .bold()
    );
    println!(
        "{}",
        "═════════════════════════════════════════════════".bright_blue()
    );

    demonstrate_monitoring_capabilities(b2bua).await?;

    // Phase 5: Final System Status
    println!(
        "\n{}",
        "Phase 5: Enterprise System Summary".bright_magenta().bold()
    );
    println!(
        "{}",
        "══════════════════════════════════════════".bright_magenta()
    );

    let final_stats = b2bua.get_enterprise_stats().await?;
    print_comprehensive_summary(&final_stats);

    info!("🎉 Enterprise demonstration completed successfully!");
    Ok(())
}

async fn simulate_security_scenarios(b2bua: &EnterpriseB2BUA) -> Result<()> {
    info!("🛡️ Demonstrating enterprise security capabilities...");

    // Simulate various attack patterns
    let attack_scenarios = vec![
        (
            "192.168.1.100",
            "INVITE sip:target@victim.com SIP/2.0\r\nCall-ID: flood-attack-1\r\n",
            "DoS Flood Attack",
        ),
        (
            "10.0.0.50",
            "INVITE sip:scan@target.com SIP/2.0\r\nCall-ID: port-scan-1\r\n",
            "Port Scanning",
        ),
        (
            "172.16.0.25",
            "INVITE sip:inject@target.com SIP/2.0\r\nCall-ID: \r\n\r\nmalicious\r\n",
            "Log Injection",
        ),
        (
            "203.0.113.10",
            "MALFORMED_SIP_MESSAGE_WITHOUT_PROPER_HEADERS",
            "Malformed Message",
        ),
    ];

    for (source_ip, message, attack_type) in attack_scenarios {
        println!(
            "  🚨 Simulating {}: {}",
            attack_type.red(),
            source_ip.yellow()
        );

        let addr = match format!("{}:5060", source_ip).parse() {
            Ok(a) => a,
            Err(e) => {
                println!("    ❌ Invalid address: {}", e);
                continue;
            }
        };
        if let Err(e) = b2bua.process_sip_message(message, addr).await {
            println!("    ✅ Attack blocked: {}", e.to_string().green());
        } else {
            println!("    ⚠️  Attack processed (may have been blocked internally)");
        }

        sleep(Duration::from_millis(500)).await;
    }

    println!("  ✅ Security demonstration completed");
    Ok(())
}

async fn simulate_call_scenarios(b2bua: &EnterpriseB2BUA) -> Result<()> {
    info!("📞 Demonstrating enterprise call processing...");

    // Simulate legitimate call scenarios
    let call_scenarios = vec![
        (
            "10.1.1.100",
            "+15551234567",
            "+15559876543",
            "Normal Enterprise Call",
        ),
        (
            "10.1.1.101",
            "+15551234568",
            "+15559876544",
            "STIR/SHAKEN Verified Call",
        ),
        (
            "10.1.1.102",
            "+15551234569",
            "+15559876545",
            "SIP-I PSTN Call",
        ),
        (
            "10.1.1.103",
            "+15551234570",
            "+15559876546",
            "High-Priority Executive Call",
        ),
    ];

    for (source_ip, from_num, to_num, call_type) in call_scenarios {
        println!(
            "  📞 Processing {}: {} -> {}",
            call_type.green(),
            from_num.blue(),
            to_num.blue()
        );

        let invite_message = format!(
            "INVITE sip:{}@enterprise.com SIP/2.0\r\n\
            Via: SIP/2.0/UDP {}:5060;branch=z9hG4bK{}\r\n\
            From: <sip:{}@enterprise.com>;tag=12345\r\n\
            To: <sip:{}@enterprise.com>\r\n\
            Call-ID: enterprise-call-{}\r\n\
            CSeq: 1 INVITE\r\n\
            Contact: <sip:{}@{}:5060>\r\n\
            Content-Type: application/sdp\r\n\
            Content-Length: 0\r\n\r\n",
            to_num,
            source_ip,
            uuid::Uuid::new_v4().simple(),
            from_num,
            to_num,
            uuid::Uuid::new_v4().simple(),
            from_num,
            source_ip
        );

        let addr = match format!("{}:5060", source_ip).parse() {
            Ok(a) => a,
            Err(e) => {
                println!("    ❌ Invalid address: {}", e);
                continue;
            }
        };
        if let Err(e) = b2bua.process_sip_message(&invite_message, addr).await {
            error!("Call processing error: {}", e);
        } else {
            println!("    ✅ Call processed successfully");
        }

        sleep(Duration::from_millis(300)).await;
    }

    println!("  ✅ Call processing demonstration completed");
    Ok(())
}

async fn demonstrate_monitoring_capabilities(b2bua: &EnterpriseB2BUA) -> Result<()> {
    info!("📊 Demonstrating real-time monitoring capabilities...");

    // Show monitoring data collection
    for i in 1..=5 {
        println!("  📈 Collecting monitoring data... (sample {})", i);

        let stats = b2bua.get_enterprise_stats().await?;

        println!(
            "    • Active Calls: {}",
            stats.b2bua_stats.active_calls.to_string().green()
        );
        println!(
            "    • Total Calls: {}",
            stats.b2bua_stats.total_calls.to_string().blue()
        );
        println!(
            "    • Threats Blocked: {}",
            stats.b2bua_stats.blocked_calls.to_string().red()
        );
        println!(
            "    • System Health: {:.1}%",
            stats.system_health.to_string().green()
        );
        println!(
            "    • ML Predictions: {}",
            stats.ml_stats.total_ips_profiled.to_string().yellow()
        );

        if let Some(ref cluster_status) = stats.cluster_status {
            println!(
                "    • Cluster Nodes: {} active",
                cluster_status.active_nodes.to_string().cyan()
            );
        }

        sleep(Duration::from_secs(2)).await;
    }

    println!("  ✅ Monitoring demonstration completed");
    Ok(())
}

fn print_system_status(stats: &redfire_switch::enterprise_b2bua::EnterpriseSystemStats) {
    println!("  🏢 Enterprise B2BUA Status:");
    println!(
        "    • System Health: {:.1}%",
        stats.system_health.to_string().green()
    );
    println!(
        "    • Uptime: {} seconds",
        stats.b2bua_stats.uptime_seconds.to_string().blue()
    );
    println!(
        "    • Security Events Blocked: {}",
        stats.security_stats.currently_blocked_ips.to_string().red()
    );
    println!(
        "    • ML Models Active: {}",
        stats.ml_stats.models_enabled.len().to_string().yellow()
    );

    if let Some(ref cluster) = stats.cluster_status {
        println!(
            "    • Cluster Health: {}",
            if cluster.cluster_healthy {
                "Healthy".green()
            } else {
                "Degraded".red()
            }
        );
        println!(
            "    • Active Nodes: {}",
            cluster.active_nodes.to_string().cyan()
        );
    } else {
        println!("    • Clustering: {}", "Disabled (Single Node)".yellow());
    }
}

fn print_comprehensive_summary(stats: &redfire_switch::enterprise_b2bua::EnterpriseSystemStats) {
    println!(
        "\n{}",
        "🏆 ENTERPRISE B2BUA DEMONSTRATION SUMMARY"
            .bright_green()
            .bold()
    );
    println!(
        "{}",
        "══════════════════════════════════════════════".bright_green()
    );

    println!(
        "\n📊 {} Call Processing Performance:",
        "Enterprise".bright_blue()
    );
    println!(
        "  • Total Calls Processed: {}",
        stats.b2bua_stats.total_calls.to_string().green()
    );
    println!(
        "  • Successfully Completed: {}",
        stats.b2bua_stats.completed_calls.to_string().green()
    );
    println!(
        "  • Security Threats Blocked: {}",
        stats.b2bua_stats.blocked_calls.to_string().red()
    );
    println!(
        "  • STIR/SHAKEN Verified: {}",
        stats.b2bua_stats.stir_shaken_verified.to_string().blue()
    );

    println!(
        "\n🛡️ {} Security & ML Performance:",
        "Advanced".bright_red()
    );
    println!(
        "  • Currently Blocked IPs: {}",
        stats.security_stats.currently_blocked_ips.to_string().red()
    );
    println!(
        "  • Security Events: {}",
        stats
            .security_stats
            .total_security_events
            .to_string()
            .yellow()
    );
    println!(
        "  • ML IPs Profiled: {}",
        stats.ml_stats.total_ips_profiled.to_string().cyan()
    );
    println!(
        "  • ML Detection Rate: {:.1}%",
        (stats.ml_stats.detection_rate * 100.0).to_string().green()
    );

    println!("\n📈 {} System Health:", "Overall".bright_green());
    println!(
        "  • System Health Score: {:.1}%",
        stats.system_health.to_string().green()
    );
    println!(
        "  • Dashboard Health: {:.1}%",
        stats.dashboard_summary.overall_health.to_string().blue()
    );
    println!(
        "  • Active Alerts: {}",
        stats.dashboard_summary.active_alerts.to_string().yellow()
    );
    println!(
        "  • Critical Alerts: {}",
        stats.dashboard_summary.critical_alerts.to_string().red()
    );

    if let Some(ref cluster) = stats.cluster_status {
        println!("\n🏢 {} Cluster Status:", "High-Availability".bright_cyan());
        println!(
            "  • Cluster Health: {}",
            if cluster.cluster_healthy {
                "Healthy".green()
            } else {
                "Degraded".red()
            }
        );
        println!(
            "  • Total Nodes: {}",
            cluster.total_nodes.to_string().blue()
        );
        println!(
            "  • Active Nodes: {}",
            cluster.active_nodes.to_string().green()
        );
        println!(
            "  • Failed Nodes: {}",
            cluster.failed_nodes.to_string().red()
        );
        println!(
            "  • Total Cluster Calls: {}",
            cluster.total_cluster_calls.to_string().cyan()
        );
    }

    println!(
        "\n{}",
        "🎯 ENTERPRISE FEATURES DEMONSTRATED:"
            .bright_magenta()
            .bold()
    );
    println!("  ✅ Real-time ML Threat Detection & Behavioral Analysis");
    println!("  ✅ Advanced Security Monitoring & Auto-blocking");
    println!("  ✅ Comprehensive Operational Dashboard & Analytics");
    println!("  ✅ High-Availability Clustering & Call State Sync");
    println!("  ✅ Enterprise-Grade B2BUA Call Processing");
    println!("  ✅ STIR/SHAKEN Identity Verification");
    println!("  ✅ SIP-I PSTN Interconnection Support");
    println!("  ✅ Production-Ready Monitoring & Alerting");

    println!(
        "\n{}",
        "🔥 REDFIRE SWITCH: ENTERPRISE B2BUA ECOSYSTEM COMPLETE! 🔥"
            .bright_red()
            .bold()
    );
    println!(
        "{}",
        "════════════════════════════════════════════════════════════".bright_red()
    );
}
