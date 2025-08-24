use anyhow::Result;
use clap::{Parser, Subcommand};
use redfire_switch::lcr::routing::RouteRequest;
use redfire_switch::lcr::types::{ConfigScope, RouteType};
use redfire_switch::lcr::LcrEngine;
use rust_decimal::Decimal;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "lcr-cli")]
#[command(about = "Least Cost Routing CLI for NANPA calls")]
struct Cli {
    #[arg(long, env = "DATABASE_URL")]
    database_url: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Simulate a call routing
    Simulate {
        /// ANI (calling number)
        #[arg(short, long)]
        ani: String,

        /// DNIS (called number)
        #[arg(short, long)]
        dnis: String,

        /// Ingress trunk name (optional)
        #[arg(short = 't', long)]
        trunk: Option<String>,

        /// Output format (json, table, detailed)
        #[arg(short = 'f', long, default_value = "table")]
        format: String,
    },

    /// Find routes for a call
    Route {
        /// ANI (calling number)
        #[arg(short, long)]
        ani: String,

        /// DNIS (called number)
        #[arg(short, long)]
        dnis: String,

        /// Ingress trunk ID
        #[arg(short = 't', long)]
        trunk_id: i32,

        /// Client deck ID (optional)
        #[arg(short = 'c', long)]
        client_deck: Option<i32>,

        /// Route type (NANPA, AZ, OTHER)
        #[arg(short = 'r', long, default_value = "NANPA")]
        route_type: String,

        /// Require profit protection
        #[arg(short = 'p', long)]
        profit_protection: bool,

        /// Minimum profit margin
        #[arg(short = 'm', long)]
        min_profit: Option<String>,
    },

    /// Reload cache from database
    ReloadCache,

    /// Show trunk statistics
    TrunkStats {
        /// Filter by trunk type (ingress, egress, all)
        #[arg(short = 't', long, default_value = "all")]
        trunk_type: String,
    },

    /// List trunks
    ListTrunks {
        /// Trunk type (ingress, egress, all)
        #[arg(short = 't', long, default_value = "all")]
        trunk_type: String,
    },

    /// Check rate for a specific code
    CheckRate {
        /// Deck ID
        #[arg(short, long)]
        deck_id: i32,

        /// Code to check (e.g., 1212555)
        #[arg(short, long)]
        code: String,

        /// Deck type (vendor, client)
        #[arg(short = 't', long, default_value = "vendor")]
        deck_type: String,
    },

    /// Start API server
    ApiServer {
        /// Bind address
        #[arg(short, long, default_value = "0.0.0.0:8080")]
        bind: String,
    },

    /// Load NANPA data from CSV files
    LoadNanpa {
        /// Path to NANPA CSV files directory
        #[arg(short, long, default_value = "files")]
        path: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    // Create LCR engine
    let lcr_engine = Arc::new(LcrEngine::new(&cli.database_url).await?);

    match cli.command {
        Commands::Simulate {
            ani,
            dnis,
            trunk,
            format,
        } => {
            let routing_engine = lcr_engine.get_routing_engine();
            let simulation = routing_engine
                .simulate_call(&ani, &dnis, trunk.as_deref())
                .await?;

            match format.as_str() {
                "json" => {
                    println!("{}", serde_json::to_string_pretty(&simulation)?);
                }
                "detailed" => {
                    println!("Call Simulation Results");
                    println!("========================");
                    println!("ANI: {}", simulation.ani);
                    println!("DNIS: {}", simulation.dnis);
                    if let Some(lrn) = &simulation.lrn {
                        println!("LRN: {}", lrn);
                    }
                    println!("Jurisdiction: {:?}", simulation.jurisdiction);
                    println!("Ingress Trunk: {}", simulation.ingress_trunk);
                    println!("Routing Decision: {}", simulation.routing_decision);
                    println!("Total Routes Found: {}", simulation.total_routes);
                    println!("\nRoutes (in priority order):");
                    println!("----------------------------");

                    for (i, route) in simulation.routes.iter().enumerate() {
                        println!("\n{}. Egress Trunk: {}", i + 1, route.egress_trunk);
                        println!("   Vendor: {}", route.vendor);
                        println!("   Cost/min: ${}", route.cost_per_minute);
                        println!("   Sell/min: ${}", route.selling_per_minute);
                        println!("   Profit/min: ${}", route.profit_margin);
                        println!("   Priority: {}", route.priority);
                    }
                }
                _ => {
                    // Table format
                    println!(
                        "ANI: {} -> DNIS: {} (LRN: {})",
                        simulation.ani,
                        simulation.dnis,
                        simulation.lrn.as_deref().unwrap_or("N/A")
                    );
                    println!(
                        "Jurisdiction: {:?} | Decision: {}",
                        simulation.jurisdiction, simulation.routing_decision
                    );
                    println!(
                        "\n{:<30} {:<15} {:<12} {:<12} {:<12} {:<8} {:<8}",
                        "Egress Trunk",
                        "Vendor",
                        "Cost/min",
                        "Sell/min",
                        "Profit/min",
                        "Setup",
                        "Billing"
                    );
                    println!("{}", "-".repeat(105));

                    for route in simulation.routes.iter().take(10) {
                        println!(
                            "{:<30} {:<15} ${:<11.4} ${:<11.4} ${:<11.4} ${:<7.4} {}/{}",
                            route.egress_trunk,
                            route.vendor,
                            route.cost_per_minute,
                            route.selling_per_minute,
                            route.profit_margin,
                            route.setup_fee,
                            route.min_increment,
                            route.interval
                        );
                    }

                    if simulation.routes.len() > 10 {
                        println!("... and {} more routes", simulation.routes.len() - 10);
                    }
                }
            }
        }

        Commands::Route {
            ani,
            dnis,
            trunk_id,
            client_deck,
            route_type,
            profit_protection,
            min_profit,
        } => {
            let routing_engine = lcr_engine.get_routing_engine();

            let route_type_enum = match route_type.to_uppercase().as_str() {
                "NANPA" => RouteType::NANPA,
                "AZ" | "A-Z" => RouteType::AZ,
                _ => RouteType::OTHER,
            };

            let min_profit_margin = min_profit
                .map(|p| Decimal::from_str_exact(&p).expect("Invalid decimal for min_profit"));

            let request = RouteRequest {
                ani,
                dnis,
                ingress_trunk_id: trunk_id,
                client_deck_id: client_deck,
                route_type: route_type_enum,
                require_profit_protection: profit_protection,
                min_profit_margin,
            };

            let response = routing_engine.find_routes(&request).await?;

            println!("Found {} routes", response.total_routes);
            println!("Jurisdiction: {:?}", response.jurisdiction);
            if let Some(lrn) = response.lrn {
                println!("LRN: {}", lrn);
            }

            println!("\nTop 10 routes:");
            for (i, route) in response.routes.iter().take(10).enumerate() {
                println!(
                    "{}. {} - Cost: ${}/min, Profit: ${}/min",
                    i + 1,
                    route.egress_trunk.name,
                    route.cost_per_minute,
                    route.profit_margin
                );
            }
        }

        Commands::ReloadCache => {
            lcr_engine.reload_cache().await?;
            println!("Cache reloaded successfully");
        }

        Commands::TrunkStats { trunk_type } => {
            let trunk_manager = lcr_engine.get_trunk_manager();
            let stats = trunk_manager.get_all_stats().await;

            println!(
                "{:<20} {:<10} {:<15} {:<10} {:<10} {:<15}",
                "Trunk ID", "Type", "Current Calls", "CPS", "Total", "Total Minutes"
            );
            println!("{}", "-".repeat(85));

            for stat in stats {
                if trunk_type != "all" {
                    let stat_type = format!("{:?}", stat.trunk_type).to_lowercase();
                    if stat_type != trunk_type {
                        continue;
                    }
                }

                println!(
                    "{:<20} {:<10} {:<15} {:<10} {:<10} {:<15.2}",
                    stat.trunk_id,
                    format!("{:?}", stat.trunk_type),
                    stat.current_calls,
                    stat.current_cps,
                    stat.total_calls,
                    stat.total_minutes
                );
            }
        }

        Commands::ListTrunks { trunk_type } => {
            if trunk_type == "ingress" || trunk_type == "all" {
                println!("\nIngress Trunks:");
                println!(
                    "{:<30} {:<20} {:<15} {:<10}",
                    "Name", "IP Address", "Capacity", "CPS Limit"
                );
                println!("{}", "-".repeat(75));

                for trunk in lcr_engine.cache.get_all_ingress_trunks() {
                    println!(
                        "{:<30} {:<20} {:<15} {:<10}",
                        trunk.name, trunk.ip_address, trunk.capacity_limit, trunk.cps_limit
                    );
                }
            }

            if trunk_type == "egress" || trunk_type == "all" {
                println!("\nEgress Trunks:");
                println!(
                    "{:<30} {:<30} {:<15} {:<10}",
                    "Name", "Host", "Capacity", "CPS Limit"
                );
                println!("{}", "-".repeat(85));

                for trunk in lcr_engine.cache.get_all_egress_trunks() {
                    println!(
                        "{:<30} {:<30} {:<15} {:<10}",
                        trunk.name,
                        format!("{}:{}", trunk.host, trunk.port),
                        trunk.capacity_limit,
                        trunk.cps_limit
                    );
                }
            }
        }

        Commands::CheckRate {
            deck_id,
            code,
            deck_type,
        } => {
            let rate = if deck_type == "client" {
                lcr_engine.cache.get_client_rate(deck_id, &code)
            } else {
                lcr_engine.cache.get_vendor_rate(deck_id, &code)
            };

            if let Some(rate) = rate {
                println!("Rate found for code: {}", rate.code);
                println!("Interstate: ${}/min", rate.inter_rate);
                println!("Intrastate: ${}/min", rate.intra_rate);
                println!("Indeterminate: ${}/min", rate.ij_rate);
                if let Some(local) = rate.local_rate {
                    println!("Local: ${}/min", local);
                }
                println!("Billing: {}/{} seconds", rate.min_increment, rate.interval);
            } else {
                println!("No rate found for code {} in deck {}", code, deck_id);
            }
        }

        Commands::ApiServer { bind } => {
            println!("Starting LCR API server on {}", bind);
            redfire_switch::lcr::api::start_api_server(lcr_engine, &bind).await?;
        }

        Commands::LoadNanpa { path } => {
            println!("Loading NANPA data from {}/", path);
            redfire_switch::lcr::nanpa_loader::load_nanpa_command(&cli.database_url).await?;
            println!("NANPA data loaded successfully");
        }
    }

    Ok(())
}

use std::str::FromStr;
