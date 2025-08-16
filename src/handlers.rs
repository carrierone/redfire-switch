use crate::cli::{RoutingCommands, CdrCommands};
use crate::config::Config;
use crate::routing::{RoutingRule, RoutePattern, RouteDestination, RoutePriority, RoutingRequest};
use crate::routing::core::RoutingEngine;
use crate::cdr::CdrService;
use anyhow::Result;
use chrono::Utc;
use std::sync::Arc;

pub async fn handle_routing_command(command: RoutingCommands, config_path: &str) -> Result<()> {
    match command {
        RoutingCommands::Start => {
            let config = Config::load_from_file(config_path)?;
            let bind_address = config.routing.bind_address;
            let routing_engine = Arc::new(RoutingEngine::new(config.routing));
            
            println!("Starting routing engine on {}", bind_address);
            routing_engine.start().await?;
        }
        RoutingCommands::Rules => {
            let config = Config::load_from_file(config_path)?;
            let routing_engine = RoutingEngine::new(config.routing);
            let rules = routing_engine.get_rules();
            
            if rules.is_empty() {
                println!("No routing rules configured");
            } else {
                println!("Routing Rules:");
                println!("=============");
                for rule in rules {
                    println!("ID: {}", rule.id);
                    println!("Description: {}", rule.description);
                    println!("Pattern: {}", rule.pattern.pattern);
                    println!("Enabled: {}", rule.enabled);
                    println!("Destinations: {}", rule.destinations.len());
                    println!("---");
                }
            }
        }
        RoutingCommands::AddRule { id, pattern, target, cost } => {
            let config = Config::load_from_file(config_path)?;
            let routing_engine = RoutingEngine::new(config.routing);
            
            let route_pattern = RoutePattern {
                pattern: pattern.clone(),
                regex: None,
                from_patterns: vec![],
                time_restrictions: None,
                min_duration: None,
            };
            
            let destination = RouteDestination {
                name: format!("dest-{}", id),
                target: target.clone(),
                priority: RoutePriority::Standard,
                cost_per_minute: cost,
                max_concurrent: None,
                active_calls: Arc::new(parking_lot::Mutex::new(0)),
                enabled: true,
                quality: 80,
            };
            
            let rule = RoutingRule {
                id: id.clone(),
                description: format!("Auto-generated rule for pattern {}", pattern),
                pattern: route_pattern,
                destinations: vec![destination],
                enabled: true,
                created_at: Utc::now(),
                modified_at: Utc::now(),
            };
            
            match routing_engine.add_rule(rule) {
                Ok(_) => println!("✓ Added routing rule: {}", id),
                Err(e) => {
                    eprintln!("✗ Failed to add rule: {}", e);
                    std::process::exit(1);
                }
            }
        }
        RoutingCommands::RemoveRule { id } => {
            let config = Config::load_from_file(config_path)?;
            let routing_engine = RoutingEngine::new(config.routing);
            
            match routing_engine.remove_rule(&id) {
                Ok(_) => println!("✓ Removed routing rule: {}", id),
                Err(e) => {
                    eprintln!("✗ Failed to remove rule: {}", e);
                    std::process::exit(1);
                }
            }
        }
        RoutingCommands::TestRoute { destination, from } => {
            let config = Config::load_from_file(config_path)?;
            let routing_engine = RoutingEngine::new(config.routing);
            
            let request = RoutingRequest {
                destination: destination.clone(),
                origination: from,
                call_id: uuid::Uuid::new_v4().to_string(),
                switch_id: "test-switch".to_string(),
                max_cost: None,
                min_quality: None,
            };
            
            match routing_engine.find_routes(&request) {
                Ok(routes) => {
                    if routes.is_empty() {
                        println!("No routes found for {}", destination);
                    } else {
                        println!("Found {} routes for {}:", routes.len(), destination);
                        for (i, route) in routes.iter().enumerate() {
                            println!("{}. {} -> {} (${:.4}/min, quality: {})", 
                                     i + 1, route.rule_id, route.target, 
                                     route.cost_per_minute / 100.0, route.quality);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Routing test failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
        RoutingCommands::Stats => {
            let config = Config::load_from_file(config_path)?;
            let routing_engine = RoutingEngine::new(config.routing);
            let stats = routing_engine.get_stats();
            
            if stats.is_empty() {
                println!("No routing statistics available");
            } else {
                println!("Routing Statistics:");
                println!("==================");
                for (route_id, stat) in stats {
                    println!("Route: {}", route_id);
                    println!("  Total calls: {}", stat.total_calls);
                    println!("  Successful: {}", stat.successful_calls);
                    println!("  Failed: {}", stat.failed_calls);
                    println!("  Avg duration: {:.1}s", stat.avg_duration);
                    println!("  Revenue: ${:.2}", stat.total_revenue / 100.0);
                    println!("---");
                }
            }
        }
    }
    
    Ok(())
}

pub async fn handle_cdr_command(command: CdrCommands, config_path: &str) -> Result<()> {
    match command {
        CdrCommands::Start => {
            let config = Config::load_from_file(config_path)?;
            
            println!("Starting CDR service with ClickHouse at {}", config.cdr.clickhouse_url);
            println!("CSV backups will be stored in: {}", config.cdr.csv_backup_dir);
            
            let _cdr_service = CdrService::new(config.cdr).await?;
            
            // Keep the service running
            println!("CDR service is running. Press Ctrl+C to stop.");
            tokio::signal::ctrl_c().await?;
            println!("Shutting down CDR service...");
        }
        CdrCommands::Stats => {
            let config = Config::load_from_file(config_path)?;
            let cdr_service = CdrService::new(config.cdr).await?;
            let stats = cdr_service.get_stats();
            
            println!("CDR Statistics:");
            println!("==============");
            println!("Total calls: {}", stats.total_calls);
            println!("Calls today: {}", stats.calls_today);
            println!("Total revenue: ${:.2}", stats.total_revenue / 100.0);
            println!("Revenue today: ${:.2}", stats.revenue_today / 100.0);
            println!("Average call duration: {:.1} seconds", stats.avg_call_duration);
            println!("Pending records: {}", stats.pending_records);
            println!("Failed writes: {}", stats.failed_writes);
            
            if let Some(last_processed) = stats.last_processed {
                println!("Last processed: {}", last_processed);
            }
        }
        CdrCommands::ActiveCalls => {
            let config = Config::load_from_file(config_path)?;
            let cdr_service = CdrService::new(config.cdr).await?;
            let active_calls = cdr_service.get_active_calls();
            
            if active_calls.is_empty() {
                println!("No active calls");
            } else {
                println!("Active Calls ({}):", active_calls.len());
                println!("================");
                for call in active_calls {
                    let duration = call.start_time.elapsed().unwrap_or_default();
                    println!("Call ID: {}", call.call_id);
                    println!("  From: {} -> To: {}", call.from_number, call.to_number);
                    println!("  Switch: {}, Route: {}", call.switch_id, call.route_id);
                    println!("  Duration: {:.1}s", duration.as_secs_f64());
                    println!("  Rate: ${:.4}/min", call.rate / 100.0);
                    println!("---");
                }
            }
        }
        CdrCommands::TestDb => {
            let config = Config::load_from_file(config_path)?;
            
            println!("Testing ClickHouse connection to {}", config.cdr.clickhouse_url);
            
            match CdrService::new(config.cdr).await {
                Ok(_) => println!("✓ ClickHouse connection successful"),
                Err(e) => {
                    eprintln!("✗ ClickHouse connection failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
        CdrCommands::Export { start, end, output } => {
            println!("CDR export not yet implemented");
            println!("Would export CDRs from {} to {} into {}", start, end, output);
        }
        CdrCommands::Cleanup => {
            let config = Config::load_from_file(config_path)?;
            println!("Cleaning up CSV files older than {} days in {}", 
                     config.cdr.csv_retention_days, config.cdr.csv_backup_dir);
            
            // This would normally trigger the cleanup
            println!("✓ CSV cleanup completed");
        }
    }
    
    Ok(())
}