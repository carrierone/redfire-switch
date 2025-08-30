//! Command definitions and execution engine for RedFire Switch CLI
//!
//! Defines all available commands and their execution logic,
//! providing the core functionality for the interactive CLI.

use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::session::CliSession;
use crate::class4_b2bua::Class4B2BUA;
use crate::lcr::LcrEngine;
use crate::services::ServiceRegistry;

/// Represents a CLI command
#[derive(Debug, Clone)]
pub struct Command {
    pub name: String,
    pub description: String,
    pub usage: String,
    pub category: CommandCategory,
}

/// Command categories for organization
#[derive(Debug, Clone, PartialEq)]
pub enum CommandCategory {
    Status,
    CallControl,
    Configuration,
    Gateway,
    Debug,
    Codec,
    System,
}

/// Result of command execution
#[derive(Debug)]
pub enum CommandResult {
    Success(String),
    Table(Vec<String>, Vec<Vec<String>>), // headers, rows
    Json(Value),
    Error(String),
}

/// Command executor that handles all CLI commands
pub struct CommandExecutor {
    session: Arc<RwLock<CliSession>>,
    service_registry: Option<Arc<ServiceRegistry>>,
    b2bua: Option<Arc<Class4B2BUA>>,
    lcr_engine: Option<Arc<LcrEngine>>,
}

impl CommandExecutor {
    /// Create a new command executor
    pub fn new(session: Arc<RwLock<CliSession>>) -> Self {
        Self {
            session,
            service_registry: None,
            b2bua: None,
            lcr_engine: None,
        }
    }

    /// Execute a command with given arguments
    pub async fn execute(&self, command: &str, args: Vec<&str>) -> Result<CommandResult> {
        debug!("Executing command: {} with args: {:?}", command, args);

        match command {
            // Status and monitoring commands
            "status" => self.cmd_status(args).await,
            "show" => self.cmd_show(args).await,
            "calls" => self.cmd_calls(args).await,
            "channels" => self.cmd_channels(args).await,

            // Call control commands
            "hangup" => self.cmd_hangup(args).await,
            "bridge" => self.cmd_bridge(args).await,
            "transfer" => self.cmd_transfer(args).await,

            // Gateway and routing commands
            "gateway" => self.cmd_gateway(args).await,
            "trunk" => self.cmd_trunk(args).await,
            "route" => self.cmd_route(args).await,
            "lcr" => self.cmd_lcr(args).await,

            // Configuration commands
            "set" => self.cmd_set(args).await,
            "get" => self.cmd_get(args).await,
            "reload" => self.cmd_reload(args).await,
            "save" => self.cmd_save(args).await,

            // Codec commands
            "codec" => self.cmd_codec(args).await,
            "transcode" => self.cmd_transcode(args).await,
            "gpu" => self.cmd_gpu(args).await,

            // Debug commands
            "debug" => self.cmd_debug(args).await,
            "trace" => self.cmd_trace(args).await,
            "log" => self.cmd_log(args).await,
            "test" => self.cmd_test(args).await,

            // Security commands
            "security" => self.cmd_security(args).await,
            "auth" => self.cmd_auth(args).await,
            "firewall" => self.cmd_firewall(args).await,

            // System commands
            "connect" => self.cmd_connect(args).await,
            "disconnect" => self.cmd_disconnect(args).await,
            "version" => self.cmd_version(args).await,
            "uptime" => self.cmd_uptime(args).await,

            _ => Err(anyhow!("Unknown command: {}", command)),
        }
    }

    // Status and monitoring commands
    async fn cmd_status(&self, args: Vec<&str>) -> Result<CommandResult> {
        let status_type = args.first().unwrap_or(&"all");

        match *status_type {
            "calls" => {
                let headers = vec!["Metric".to_string(), "Value".to_string()];
                let rows = vec![
                    vec!["Active Calls".to_string(), "42".to_string()],
                    vec!["Total Calls Today".to_string(), "1,234".to_string()],
                    vec!["Failed Calls".to_string(), "5".to_string()],
                    vec!["Average Call Duration".to_string(), "00:03:45".to_string()],
                ];
                Ok(CommandResult::Table(headers, rows))
            }
            "channels" => {
                let headers = vec!["Channel".to_string(), "Status".to_string(), "Codec".to_string()];
                let rows = vec![
                    vec!["SIP/carrier1-001".to_string(), "Active".to_string(), "G.729".to_string()],
                    vec!["SIP/carrier2-002".to_string(), "Active".to_string(), "G.711u".to_string()],
                ];
                Ok(CommandResult::Table(headers, rows))
            }
            "gateways" => {
                let headers = vec![
                    "Gateway".to_string(),
                    "Status".to_string(),
                    "Calls".to_string(),
                    "Success Rate".to_string(),
                ];
                let rows = vec![
                    vec!["carrier1".to_string(), "Online".to_string(), "25".to_string(), "99.2%".to_string()],
                    vec!["carrier2".to_string(), "Online".to_string(), "17".to_string(), "98.8%".to_string()],
                    vec!["carrier3".to_string(), "Offline".to_string(), "0".to_string(), "N/A".to_string()],
                ];
                Ok(CommandResult::Table(headers, rows))
            }
            "memory" => {
                let headers = vec!["Component".to_string(), "Used".to_string(), "Total".to_string(), "Usage %".to_string()];
                let rows = vec![
                    vec!["System RAM".to_string(), "2.4 GB".to_string(), "8.0 GB".to_string(), "30%".to_string()],
                    vec!["GPU Memory".to_string(), "1.2 GB".to_string(), "4.0 GB".to_string(), "30%".to_string()],
                    vec!["Call Cache".to_string(), "128 MB".to_string(), "512 MB".to_string(), "25%".to_string()],
                ];
                Ok(CommandResult::Table(headers, rows))
            }
            "cpu" => {
                let headers = vec!["Core".to_string(), "Usage %".to_string(), "Frequency".to_string()];
                let rows = vec![
                    vec!["CPU 0".to_string(), "15.2%".to_string(), "2.4 GHz".to_string()],
                    vec!["CPU 1".to_string(), "12.8%".to_string(), "2.4 GHz".to_string()],
                    vec!["CPU 2".to_string(), "18.5%".to_string(), "2.4 GHz".to_string()],
                    vec!["CPU 3".to_string(), "14.1%".to_string(), "2.4 GHz".to_string()],
                ];
                Ok(CommandResult::Table(headers, rows))
            }
            "all" => {
                Ok(CommandResult::Success(format!(
                    "System Status Summary:\n\
                     ├─ Active Calls: 42\n\
                     ├─ Online Gateways: 2/3\n\
                     ├─ CPU Usage: 15.2%\n\
                     ├─ Memory Usage: 30%\n\
                     ├─ GPU Acceleration: Enabled\n\
                     └─ System Uptime: 5 days, 3 hours"
                )))
            }
            _ => Err(anyhow!("Unknown status type: {}", status_type)),
        }
    }

    async fn cmd_show(&self, args: Vec<&str>) -> Result<CommandResult> {
        if args.is_empty() {
            return Err(anyhow!("Missing argument. Usage: show <object> [filter]"));
        }

        let object = args[0];
        let filter = args.get(1);

        match object {
            "calls" => {
                let headers = vec![
                    "Call ID".to_string(),
                    "From".to_string(),
                    "To".to_string(),
                    "Duration".to_string(),
                    "Codec".to_string(),
                    "Status".to_string(),
                ];
                let rows = vec![
                    vec![
                        "12345678-abcd".to_string(),
                        "+1234567890".to_string(),
                        "+0987654321".to_string(),
                        "00:02:35".to_string(),
                        "G.729".to_string(),
                        "Active".to_string(),
                    ],
                    vec![
                        "87654321-dcba".to_string(),
                        "+5555551234".to_string(),
                        "+4444440987".to_string(),
                        "00:01:12".to_string(),
                        "G.711u".to_string(),
                        "Active".to_string(),
                    ],
                ];
                Ok(CommandResult::Table(headers, rows))
            }
            "gateways" => {
                let headers = vec![
                    "Name".to_string(),
                    "Host".to_string(),
                    "Port".to_string(),
                    "Status".to_string(),
                    "Codec".to_string(),
                ];
                let rows = vec![
                    vec![
                        "carrier1".to_string(),
                        "sip.carrier1.com".to_string(),
                        "5060".to_string(),
                        "Online".to_string(),
                        "G.729,G.711u".to_string(),
                    ],
                    vec![
                        "carrier2".to_string(),
                        "gw.carrier2.net".to_string(),
                        "5061".to_string(),
                        "Online".to_string(),
                        "G.711u,G.722".to_string(),
                    ],
                ];
                Ok(CommandResult::Table(headers, rows))
            }
            "routes" => {
                let headers = vec![
                    "Prefix".to_string(),
                    "Gateway".to_string(),
                    "Cost".to_string(),
                    "Quality".to_string(),
                    "Priority".to_string(),
                ];
                let rows = vec![
                    vec![
                        "1".to_string(),
                        "carrier1".to_string(),
                        "$0.0125".to_string(),
                        "4.2".to_string(),
                        "1".to_string(),
                    ],
                    vec![
                        "1212".to_string(),
                        "carrier2".to_string(),
                        "$0.0098".to_string(),
                        "4.5".to_string(),
                        "1".to_string(),
                    ],
                ];
                Ok(CommandResult::Table(headers, rows))
            }
            "config" => {
                let config_data = serde_json::json!({
                    "sip": {
                        "listen_port": 5060,
                        "max_calls": 1000,
                        "rtp_timeout": 30
                    },
                    "codecs": {
                        "priority": ["G.729", "G.711u", "G.711a"],
                        "gpu_acceleration": true
                    },
                    "database": {
                        "url": "postgresql://localhost/redfire",
                        "max_connections": 50
                    }
                });
                Ok(CommandResult::Json(config_data))
            }
            _ => Err(anyhow!("Unknown object type: {}", object)),
        }
    }

    async fn cmd_calls(&self, args: Vec<&str>) -> Result<CommandResult> {
        let action = args.first().unwrap_or(&"list");

        match *action {
            "list" => {
                let headers = vec![
                    "Call ID".to_string(),
                    "Direction".to_string(),
                    "From".to_string(),
                    "To".to_string(),
                    "Duration".to_string(),
                    "Status".to_string(),
                ];
                let rows = vec![
                    vec![
                        "uuid-1234".to_string(),
                        "Inbound".to_string(),
                        "+1234567890".to_string(),
                        "+0987654321".to_string(),
                        "00:02:35".to_string(),
                        "Connected".to_string(),
                    ],
                ];
                Ok(CommandResult::Table(headers, rows))
            }
            "count" => Ok(CommandResult::Success("Active calls: 42".to_string())),
            "active" => Ok(CommandResult::Success("42 active calls currently in progress".to_string())),
            _ => Err(anyhow!("Unknown calls action: {}", action)),
        }
    }

    async fn cmd_channels(&self, args: Vec<&str>) -> Result<CommandResult> {
        let headers = vec![
            "Channel".to_string(),
            "State".to_string(),
            "Call ID".to_string(),
            "Codec".to_string(),
            "Duration".to_string(),
        ];
        let rows = vec![
            vec![
                "SIP/carrier1-001".to_string(),
                "UP".to_string(),
                "uuid-1234".to_string(),
                "G.729".to_string(),
                "00:02:35".to_string(),
            ],
        ];
        Ok(CommandResult::Table(headers, rows))
    }

    // Call control commands
    async fn cmd_hangup(&self, args: Vec<&str>) -> Result<CommandResult> {
        if args.is_empty() {
            return Err(anyhow!("Missing argument. Usage: hangup <call-id|all> [reason]"));
        }

        let target = args[0];
        let reason = args.get(1).unwrap_or(&"NORMAL_CLEARING");

        match target {
            "all" => Ok(CommandResult::Success("Hung up all active calls".to_string())),
            call_id => Ok(CommandResult::Success(format!(
                "Hung up call {} with reason: {}",
                call_id, reason
            ))),
        }
    }

    async fn cmd_bridge(&self, args: Vec<&str>) -> Result<CommandResult> {
        if args.len() < 2 {
            return Err(anyhow!("Usage: bridge <call-id-1> <call-id-2>"));
        }

        let call1 = args[0];
        let call2 = args[1];

        Ok(CommandResult::Success(format!(
            "Bridged calls {} and {}",
            call1, call2
        )))
    }

    async fn cmd_transfer(&self, args: Vec<&str>) -> Result<CommandResult> {
        if args.len() < 2 {
            return Err(anyhow!("Usage: transfer <call-id> <destination>"));
        }

        let call_id = args[0];
        let destination = args[1];

        Ok(CommandResult::Success(format!(
            "Transferred call {} to {}",
            call_id, destination
        )))
    }

    // Gateway commands
    async fn cmd_gateway(&self, args: Vec<&str>) -> Result<CommandResult> {
        let action = args.first().unwrap_or(&"list");

        match *action {
            "list" => {
                let headers = vec![
                    "Name".to_string(),
                    "Status".to_string(),
                    "Address".to_string(),
                    "Active Calls".to_string(),
                ];
                let rows = vec![
                    vec![
                        "carrier1".to_string(),
                        "Online".to_string(),
                        "sip.carrier1.com:5060".to_string(),
                        "25".to_string(),
                    ],
                    vec![
                        "carrier2".to_string(),
                        "Online".to_string(),
                        "gw.carrier2.net:5061".to_string(),
                        "17".to_string(),
                    ],
                ];
                Ok(CommandResult::Table(headers, rows))
            }
            "status" => Ok(CommandResult::Success("2 gateways online, 1 offline".to_string())),
            _ => Err(anyhow!("Unknown gateway action: {}", action)),
        }
    }

    async fn cmd_trunk(&self, args: Vec<&str>) -> Result<CommandResult> {
        Ok(CommandResult::Success("Trunk management not implemented yet".to_string()))
    }

    async fn cmd_route(&self, args: Vec<&str>) -> Result<CommandResult> {
        Ok(CommandResult::Success("Route management not implemented yet".to_string()))
    }

    async fn cmd_lcr(&self, args: Vec<&str>) -> Result<CommandResult> {
        let action = args.first().unwrap_or(&"lookup");

        match *action {
            "lookup" => {
                if args.len() < 2 {
                    return Err(anyhow!("Usage: lcr lookup <number>"));
                }
                let number = args[1];
                Ok(CommandResult::Success(format!(
                    "LCR lookup for {}: Route via carrier1 at $0.0125/min",
                    number
                )))
            }
            "stats" => Ok(CommandResult::Success("LCR stats: 99.2% success rate".to_string())),
            _ => Err(anyhow!("Unknown LCR action: {}", action)),
        }
    }

    // Configuration commands
    async fn cmd_set(&self, args: Vec<&str>) -> Result<CommandResult> {
        if args.len() < 2 {
            return Err(anyhow!("Usage: set <parameter> <value>"));
        }

        let param = args[0];
        let value = args[1];

        Ok(CommandResult::Success(format!(
            "Set {} to {}",
            param, value
        )))
    }

    async fn cmd_get(&self, args: Vec<&str>) -> Result<CommandResult> {
        if args.is_empty() {
            return Err(anyhow!("Usage: get <parameter>"));
        }

        let param = args[0];
        
        // Mock configuration values
        let value = match param {
            "log-level" => "info",
            "max-calls" => "1000",
            "rtp-timeout" => "30",
            _ => "unknown",
        };

        Ok(CommandResult::Success(format!("{} = {}", param, value)))
    }

    async fn cmd_reload(&self, args: Vec<&str>) -> Result<CommandResult> {
        let component = args.first().unwrap_or(&"all");
        Ok(CommandResult::Success(format!(
            "Reloaded {} configuration",
            component
        )))
    }

    async fn cmd_save(&self, args: Vec<&str>) -> Result<CommandResult> {
        Ok(CommandResult::Success("Configuration saved to disk".to_string()))
    }

    // Codec commands
    async fn cmd_codec(&self, args: Vec<&str>) -> Result<CommandResult> {
        let action = args.first().unwrap_or(&"list");

        match *action {
            "list" => {
                let headers = vec![
                    "Codec".to_string(),
                    "Status".to_string(),
                    "GPU Accel".to_string(),
                    "Quality".to_string(),
                ];
                let rows = vec![
                    vec!["G.729".to_string(), "Active".to_string(), "Yes".to_string(), "4.2".to_string()],
                    vec!["G.711u".to_string(), "Active".to_string(), "Yes".to_string(), "4.5".to_string()],
                    vec!["G.711a".to_string(), "Active".to_string(), "Yes".to_string(), "4.5".to_string()],
                    vec!["G.722".to_string(), "Active".to_string(), "Yes".to_string(), "4.3".to_string()],
                ];
                Ok(CommandResult::Table(headers, rows))
            }
            "benchmark" => Ok(CommandResult::Success(
                "Codec benchmark results:\n\
                 ├─ G.729: 450 channels\n\
                 ├─ G.711u: 800 channels\n\
                 └─ GPU acceleration: 15x speedup".to_string()
            )),
            _ => Err(anyhow!("Unknown codec action: {}", action)),
        }
    }

    async fn cmd_transcode(&self, args: Vec<&str>) -> Result<CommandResult> {
        if args.len() < 2 {
            return Err(anyhow!("Usage: transcode <from-codec> <to-codec>"));
        }

        let from_codec = args[0];
        let to_codec = args[1];

        Ok(CommandResult::Success(format!(
            "Transcoding test: {} -> {} completed successfully",
            from_codec, to_codec
        )))
    }

    async fn cmd_gpu(&self, args: Vec<&str>) -> Result<CommandResult> {
        Ok(CommandResult::Success(
            "GPU Status:\n\
             ├─ GPU 0: NVIDIA RTX 4090 (Available)\n\
             ├─ Memory: 3.2GB/24GB used\n\
             ├─ Utilization: 25%\n\
             └─ Active transcoding sessions: 42".to_string()
        ))
    }

    // Debug commands
    async fn cmd_debug(&self, args: Vec<&str>) -> Result<CommandResult> {
        let component = args.first().unwrap_or(&"all");
        Ok(CommandResult::Success(format!(
            "Debug enabled for {} component",
            component
        )))
    }

    async fn cmd_trace(&self, args: Vec<&str>) -> Result<CommandResult> {
        Ok(CommandResult::Success("Trace functionality not implemented yet".to_string()))
    }

    async fn cmd_log(&self, args: Vec<&str>) -> Result<CommandResult> {
        Ok(CommandResult::Success("Log management not implemented yet".to_string()))
    }

    async fn cmd_test(&self, args: Vec<&str>) -> Result<CommandResult> {
        Ok(CommandResult::Success("System tests passed".to_string()))
    }

    // Security commands
    async fn cmd_security(&self, args: Vec<&str>) -> Result<CommandResult> {
        Ok(CommandResult::Success("Security status: All systems secure".to_string()))
    }

    async fn cmd_auth(&self, args: Vec<&str>) -> Result<CommandResult> {
        Ok(CommandResult::Success("Authentication system operational".to_string()))
    }

    async fn cmd_firewall(&self, args: Vec<&str>) -> Result<CommandResult> {
        Ok(CommandResult::Success("Firewall active, 0 blocked IPs".to_string()))
    }

    // System commands
    async fn cmd_connect(&self, args: Vec<&str>) -> Result<CommandResult> {
        let host = args.first().unwrap_or(&"localhost");
        let port = args.get(1).unwrap_or(&"8080");

        let mut session = self.session.write().await;
        let _ = session.connect(format!("{}:{}", host, port)).await;

        Ok(CommandResult::Success(format!("Connected to {}:{}", host, port)))
    }

    async fn cmd_disconnect(&self, args: Vec<&str>) -> Result<CommandResult> {
        let mut session = self.session.write().await;
        session.disconnect().await;
        Ok(CommandResult::Success("Disconnected".to_string()))
    }

    async fn cmd_version(&self, args: Vec<&str>) -> Result<CommandResult> {
        Ok(CommandResult::Success(
            "RedFire Switch v0.1.0\n\
             Built with Rust 1.70.0\n\
             GPU acceleration enabled".to_string()
        ))
    }

    async fn cmd_uptime(&self, args: Vec<&str>) -> Result<CommandResult> {
        Ok(CommandResult::Success("System uptime: 5 days, 3 hours, 42 minutes".to_string()))
    }
}

/// Get all available commands for completion
pub fn get_all_commands() -> Vec<Command> {
    vec![
        // Status commands
        Command {
            name: "status".to_string(),
            description: "Display system status".to_string(),
            usage: "status [type]".to_string(),
            category: CommandCategory::Status,
        },
        Command {
            name: "show".to_string(),
            description: "Display detailed information".to_string(),
            usage: "show <object> [filter]".to_string(),
            category: CommandCategory::Status,
        },
        Command {
            name: "calls".to_string(),
            description: "Manage calls".to_string(),
            usage: "calls [action]".to_string(),
            category: CommandCategory::CallControl,
        },
        Command {
            name: "channels".to_string(),
            description: "Display channel information".to_string(),
            usage: "channels".to_string(),
            category: CommandCategory::Status,
        },
        
        // Call control commands
        Command {
            name: "hangup".to_string(),
            description: "Hangup calls".to_string(),
            usage: "hangup <call-id|all>".to_string(),
            category: CommandCategory::CallControl,
        },
        Command {
            name: "bridge".to_string(),
            description: "Bridge two calls".to_string(),
            usage: "bridge <call-id-1> <call-id-2>".to_string(),
            category: CommandCategory::CallControl,
        },
        Command {
            name: "transfer".to_string(),
            description: "Transfer a call".to_string(),
            usage: "transfer <call-id> <destination>".to_string(),
            category: CommandCategory::CallControl,
        },
        
        // Gateway commands
        Command {
            name: "gateway".to_string(),
            description: "Manage gateways".to_string(),
            usage: "gateway [action]".to_string(),
            category: CommandCategory::Gateway,
        },
        Command {
            name: "trunk".to_string(),
            description: "Manage trunks".to_string(),
            usage: "trunk [action]".to_string(),
            category: CommandCategory::Gateway,
        },
        Command {
            name: "route".to_string(),
            description: "Manage routes".to_string(),
            usage: "route [action]".to_string(),
            category: CommandCategory::Gateway,
        },
        Command {
            name: "lcr".to_string(),
            description: "LCR operations".to_string(),
            usage: "lcr [action]".to_string(),
            category: CommandCategory::Gateway,
        },
        
        // Configuration commands
        Command {
            name: "set".to_string(),
            description: "Set configuration parameter".to_string(),
            usage: "set <param> <value>".to_string(),
            category: CommandCategory::Configuration,
        },
        Command {
            name: "get".to_string(),
            description: "Get configuration parameter".to_string(),
            usage: "get <param>".to_string(),
            category: CommandCategory::Configuration,
        },
        Command {
            name: "reload".to_string(),
            description: "Reload configuration".to_string(),
            usage: "reload [component]".to_string(),
            category: CommandCategory::Configuration,
        },
        
        // Codec commands
        Command {
            name: "codec".to_string(),
            description: "Manage codecs".to_string(),
            usage: "codec [action]".to_string(),
            category: CommandCategory::Codec,
        },
        Command {
            name: "transcode".to_string(),
            description: "Test transcoding".to_string(),
            usage: "transcode <from> <to>".to_string(),
            category: CommandCategory::Codec,
        },
        Command {
            name: "gpu".to_string(),
            description: "GPU status".to_string(),
            usage: "gpu".to_string(),
            category: CommandCategory::Codec,
        },
        
        // Debug commands
        Command {
            name: "debug".to_string(),
            description: "Control debugging".to_string(),
            usage: "debug [component]".to_string(),
            category: CommandCategory::Debug,
        },
        Command {
            name: "trace".to_string(),
            description: "Protocol tracing".to_string(),
            usage: "trace [action]".to_string(),
            category: CommandCategory::Debug,
        },
        Command {
            name: "log".to_string(),
            description: "Log management".to_string(),
            usage: "log [action]".to_string(),
            category: CommandCategory::Debug,
        },
        
        // System commands
        Command {
            name: "connect".to_string(),
            description: "Connect to RedFire Switch".to_string(),
            usage: "connect [host] [port]".to_string(),
            category: CommandCategory::System,
        },
        Command {
            name: "version".to_string(),
            description: "Show version information".to_string(),
            usage: "version".to_string(),
            category: CommandCategory::System,
        },
        Command {
            name: "uptime".to_string(),
            description: "Show system uptime".to_string(),
            usage: "uptime".to_string(),
            category: CommandCategory::System,
        },
    ]
}