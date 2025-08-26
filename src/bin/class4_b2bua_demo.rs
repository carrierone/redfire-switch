//! Class 4 B2BUA Demo Application
//! Demonstrates the complete Class 4 switching functionality

use anyhow::Result;
use redfire_switch::class4_integration::{Class4SwitchService, Class4SwitchAPI};
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info, warn};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .with_thread_ids(true)
        .with_line_number(true)
        .init();

    info!("Starting Class 4 B2BUA Demo");

    // Build the Class 4 Switch Service
    let service = build_demo_service().await?;
    let service_arc = Arc::new(service);
    
    // Create API for monitoring
    let api = Class4SwitchAPI::new(service_arc.clone());
    
    // Start monitoring task
    start_monitoring_task(api).await;
    
    // Set up graceful shutdown
    let shutdown_service = service_arc.clone();
    tokio::spawn(async move {
        if let Err(e) = signal::ctrl_c().await {
            error!("Failed to listen for shutdown signal: {}", e);
        }
        
        info!("Shutdown signal received, stopping Class 4 Switch");
        if let Err(e) = shutdown_service.shutdown().await {
            error!("Error during shutdown: {}", e);
        }
        
        std::process::exit(0);
    });

    // Start the service (this blocks until shutdown)
    match service_arc.start().await {
        Ok(()) => info!("Class 4 B2BUA Demo completed successfully"),
        Err(e) => error!("Class 4 B2BUA Demo failed: {}", e),
    }

    Ok(())
}

async fn build_demo_service() -> Result<Class4SwitchService> {
    info!("Building Class 4 Switch Service for demo");
    
    // Use environment variables or defaults for configuration
    let bind_address = std::env::var("CLASS4_BIND_ADDRESS")
        .unwrap_or_else(|_| "127.0.0.1".to_string())
        .parse()?;
    
    let bind_port: u16 = std::env::var("CLASS4_BIND_PORT")
        .unwrap_or_else(|_| "5060".to_string())
        .parse()?;
    
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost/redfire_switch".to_string());
    
    let max_calls: u32 = std::env::var("CLASS4_MAX_CALLS")
        .unwrap_or_else(|_| "1000".to_string())
        .parse()?;

    info!("Configuration:");
    info!("  Bind Address: {}", bind_address);
    info!("  Bind Port: {}", bind_port);
    info!("  Max Concurrent Calls: {}", max_calls);
    info!("  Database URL: {}", mask_password(&database_url));

    let service = Class4SwitchService::builder()
        .bind_address(bind_address)
        .bind_port(bind_port)
        .max_concurrent_calls(max_calls)
        .call_timeout_seconds(1800) // 30 minutes
        .max_route_attempts(3)
        .enable_cdr_generation(true)
        .enable_codec_translation(true)
        .database_url(database_url)
        .build()
        .await?;

    Ok(service)
}

async fn start_monitoring_task(api: Class4SwitchAPI) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        
        loop {
            interval.tick().await;
            
            match api.health_check().await {
                Ok(health) => {
                    if health.overall_healthy {
                        info!("Health Check: OK - {} active calls, DB response time: {}ms", 
                              health.active_calls, health.database_response_time_ms);
                    } else {
                        warn!("Health Check: UNHEALTHY - Database issues detected");
                    }
                }
                Err(e) => {
                    error!("Health check failed: {}", e);
                }
            }
            
            match api.get_call_stats().await {
                Ok(stats) => {
                    info!("Call Stats: Active={}, Total={}, Success={}, Failed={}, Peak={}", 
                          stats.active_calls,
                          stats.total_calls,
                          stats.successful_calls, 
                          stats.failed_calls,
                          stats.peak_concurrent_calls);
                }
                Err(e) => {
                    error!("Failed to get call stats: {}", e);
                }
            }
        }
    });
}

fn mask_password(url: &str) -> String {
    // Simple password masking for logging
    if let Some(at_pos) = url.find('@') {
        if let Some(colon_pos) = url[..at_pos].rfind(':') {
            let mut masked = url.to_string();
            masked.replace_range(colon_pos + 1..at_pos, "****");
            return masked;
        }
    }
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_masking() {
        let url = "postgres://user:secret@localhost/db";
        let masked = mask_password(url);
        assert_eq!(masked, "postgres://user:****@localhost/db");
    }
    
    #[test]
    fn test_password_masking_no_password() {
        let url = "postgres://localhost/db";
        let masked = mask_password(url);
        assert_eq!(masked, "postgres://localhost/db");
    }
}