//! Help system for RedFire Switch CLI
//!
//! Provides comprehensive help documentation for all commands,
//! similar to FreeSWITCH's built-in help system.

use colored::*;
use std::collections::HashMap;

/// Help system for CLI commands and features
pub struct HelpSystem {
    command_help: HashMap<String, CommandHelp>,
}

/// Help information for a command
#[derive(Clone)]
pub struct CommandHelp {
    pub name: String,
    pub description: String,
    pub usage: String,
    pub examples: Vec<String>,
    pub arguments: Vec<ArgumentHelp>,
    pub related: Vec<String>,
}

/// Help information for command arguments
#[derive(Clone)]
pub struct ArgumentHelp {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub values: Vec<String>,
}

impl HelpSystem {
    /// Create a new help system
    pub fn new() -> Self {
        let mut help_system = Self {
            command_help: HashMap::new(),
        };
        help_system.initialize_help_data();
        help_system
    }

    /// Print general help information
    pub fn print_general_help(&self) {
        println!("{}", "RedFire Switch CLI Help".bright_cyan().bold());
        println!("{}", "=".repeat(40).bright_cyan());
        println!();

        println!("{}", "GENERAL COMMANDS:".bright_yellow().bold());
        self.print_command_category(&[
            ("help [command]", "Show help for command or general help"),
            ("quit/exit/bye", "Exit the CLI"),
            ("clear/cls", "Clear the screen"),
        ]);

        println!("{}", "STATUS & MONITORING:".bright_yellow().bold());
        self.print_command_category(&[
            ("status [type]", "Show system status information"),
            ("show [object]", "Display detailed information"),
            ("calls [action]", "Manage and view call information"),
            ("channels", "Show channel information"),
        ]);

        println!("{}", "GATEWAY & TRUNK MANAGEMENT:".bright_yellow().bold());
        self.print_command_category(&[
            ("gateway [action]", "Manage SIP gateways"),
            ("trunk [action]", "Manage trunk groups"),
            ("route [action]", "Manage routing tables"),
            ("lcr [action]", "Least Cost Routing operations"),
        ]);

        println!("{}", "CALL CONTROL:".bright_yellow().bold());
        self.print_command_category(&[
            ("hangup [call-id|all]", "Hangup active calls"),
            ("transfer [call-id]", "Transfer calls"),
            ("bridge [call1] [call2]", "Bridge two calls"),
        ]);

        println!("{}", "CONFIGURATION:".bright_yellow().bold());
        self.print_command_category(&[
            ("set [param] [value]", "Set configuration parameters"),
            ("get [param]", "Get configuration values"),
            ("reload [component]", "Reload configuration"),
            ("save", "Save configuration to disk"),
        ]);

        println!("{}", "CODEC & TRANSCODING:".bright_yellow().bold());
        self.print_command_category(&[
            ("codec [action]", "Manage codec operations"),
            ("transcode [from] [to]", "Test codec transcoding"),
            ("gpu", "GPU acceleration status"),
        ]);

        println!("{}", "DEBUG & DIAGNOSTICS:".bright_yellow().bold());
        self.print_command_category(&[
            ("debug [level/component]", "Control debug output"),
            ("trace [action]", "Protocol tracing"),
            ("log [action]", "Log management"),
            ("test [component]", "Run system tests"),
        ]);

        println!("{}", "SECURITY:".bright_yellow().bold());
        self.print_command_category(&[
            ("security [action]", "Security management"),
            ("auth [action]", "Authentication controls"),
            ("firewall [action]", "Firewall management"),
        ]);

        println!();
        println!(
            "{}",
            "Use 'help <command>' for detailed information about a specific command."
                .bright_green()
        );
        println!("{}", "Use TAB for command completion.".bright_green());
        println!();
    }

    /// Print help for a specific command
    pub fn print_command_help(&self, command: &str) {
        if let Some(help) = self.command_help.get(command) {
            println!("{}: {}", help.name.bright_cyan().bold(), help.description);
            println!();

            println!("{}", "USAGE:".bright_yellow().bold());
            println!("  {}", help.usage.bright_white());
            println!();

            if !help.arguments.is_empty() {
                println!("{}", "ARGUMENTS:".bright_yellow().bold());
                for arg in &help.arguments {
                    let req_indicator = if arg.required {
                        "[required]".red()
                    } else {
                        "[optional]".green()
                    };

                    println!(
                        "  {} {} - {}",
                        arg.name.bright_white().bold(),
                        req_indicator,
                        arg.description
                    );

                    if !arg.values.is_empty() {
                        println!(
                            "    {}: {}",
                            "Values".bright_blue(),
                            arg.values.join(", ").bright_magenta()
                        );
                    }
                }
                println!();
            }

            if !help.examples.is_empty() {
                println!("{}", "EXAMPLES:".bright_yellow().bold());
                for example in &help.examples {
                    println!("  {}", example.bright_green());
                }
                println!();
            }

            if !help.related.is_empty() {
                println!("{}", "SEE ALSO:".bright_yellow().bold());
                println!("  {}", help.related.join(", ").bright_cyan());
                println!();
            }
        } else {
            println!(
                "{}: No help available for '{}'",
                "Error".red().bold(),
                command
            );
            println!("Use '{}' to see all available commands.", "help".yellow());
        }
    }

    /// Print a category of commands
    fn print_command_category(&self, commands: &[(&str, &str)]) {
        for (command, description) in commands {
            println!("  {:20} - {}", command.bright_cyan(), description);
        }
        println!();
    }

    /// Initialize all help data
    fn initialize_help_data(&mut self) {
        // Status command help
        self.add_command_help(CommandHelp {
            name: "status".to_string(),
            description: "Display system status information".to_string(),
            usage: "status [calls|channels|gateways|trunks|codecs|memory|cpu|network|database|security|all]".to_string(),
            examples: vec![
                "status".to_string(),
                "status calls".to_string(),
                "status all".to_string(),
            ],
            arguments: vec![
                ArgumentHelp {
                    name: "type".to_string(),
                    description: "Type of status to display".to_string(),
                    required: false,
                    values: vec![
                        "calls".to_string(), "channels".to_string(), "gateways".to_string(),
                        "trunks".to_string(), "codecs".to_string(), "memory".to_string(),
                        "cpu".to_string(), "network".to_string(), "database".to_string(),
                        "security".to_string(), "all".to_string()
                    ],
                }
            ],
            related: vec!["show".to_string(), "calls".to_string()],
        });

        // Show command help
        self.add_command_help(CommandHelp {
            name: "show".to_string(),
            description: "Display detailed information about system components".to_string(),
            usage: "show <object> [filter]".to_string(),
            examples: vec![
                "show calls".to_string(),
                "show calls active".to_string(),
                "show gateways".to_string(),
                "show config sip".to_string(),
            ],
            arguments: vec![
                ArgumentHelp {
                    name: "object".to_string(),
                    description: "Object type to display".to_string(),
                    required: true,
                    values: vec![
                        "calls".to_string(),
                        "channels".to_string(),
                        "gateways".to_string(),
                        "trunks".to_string(),
                        "routes".to_string(),
                        "config".to_string(),
                        "stats".to_string(),
                        "alarms".to_string(),
                        "events".to_string(),
                    ],
                },
                ArgumentHelp {
                    name: "filter".to_string(),
                    description: "Optional filter criteria".to_string(),
                    required: false,
                    values: vec![
                        "active".to_string(),
                        "inactive".to_string(),
                        "all".to_string(),
                    ],
                },
            ],
            related: vec!["status".to_string(), "calls".to_string()],
        });

        // Calls command help
        self.add_command_help(CommandHelp {
            name: "calls".to_string(),
            description: "Manage and view call information".to_string(),
            usage: "calls [list|count|active|history|search] [criteria]".to_string(),
            examples: vec![
                "calls".to_string(),
                "calls list".to_string(),
                "calls count".to_string(),
                "calls active".to_string(),
                "calls search +1234567890".to_string(),
            ],
            arguments: vec![ArgumentHelp {
                name: "action".to_string(),
                description: "Action to perform on calls".to_string(),
                required: false,
                values: vec![
                    "list".to_string(),
                    "count".to_string(),
                    "active".to_string(),
                    "history".to_string(),
                    "search".to_string(),
                ],
            }],
            related: vec![
                "status".to_string(),
                "hangup".to_string(),
                "show".to_string(),
            ],
        });

        // Hangup command help
        self.add_command_help(CommandHelp {
            name: "hangup".to_string(),
            description: "Terminate active calls".to_string(),
            usage: "hangup <call-id|all> [reason]".to_string(),
            examples: vec![
                "hangup all".to_string(),
                "hangup 12345678-1234-1234-1234-123456789012".to_string(),
                "hangup all SYSTEM_SHUTDOWN".to_string(),
            ],
            arguments: vec![
                ArgumentHelp {
                    name: "target".to_string(),
                    description: "Call ID or 'all' for all calls".to_string(),
                    required: true,
                    values: vec!["all".to_string(), "<uuid>".to_string()],
                },
                ArgumentHelp {
                    name: "reason".to_string(),
                    description: "Hangup reason code".to_string(),
                    required: false,
                    values: vec![
                        "NORMAL_CLEARING".to_string(),
                        "USER_BUSY".to_string(),
                        "NO_ANSWER".to_string(),
                        "SYSTEM_SHUTDOWN".to_string(),
                    ],
                },
            ],
            related: vec!["calls".to_string(), "bridge".to_string()],
        });

        // Gateway command help
        self.add_command_help(CommandHelp {
            name: "gateway".to_string(),
            description: "Manage SIP gateways and their status".to_string(),
            usage: "gateway [list|status|enable|disable|test|stats] [gateway-name]".to_string(),
            examples: vec![
                "gateway list".to_string(),
                "gateway status".to_string(),
                "gateway test carrier1".to_string(),
                "gateway enable carrier1".to_string(),
            ],
            arguments: vec![
                ArgumentHelp {
                    name: "action".to_string(),
                    description: "Action to perform".to_string(),
                    required: false,
                    values: vec![
                        "list".to_string(),
                        "status".to_string(),
                        "enable".to_string(),
                        "disable".to_string(),
                        "test".to_string(),
                        "stats".to_string(),
                    ],
                },
                ArgumentHelp {
                    name: "gateway-name".to_string(),
                    description: "Specific gateway name".to_string(),
                    required: false,
                    values: vec!["<gateway-name>".to_string()],
                },
            ],
            related: vec!["trunk".to_string(), "route".to_string()],
        });

        // Set command help
        self.add_command_help(CommandHelp {
            name: "set".to_string(),
            description: "Set configuration parameters".to_string(),
            usage: "set <parameter> <value>".to_string(),
            examples: vec![
                "set log-level debug".to_string(),
                "set max-calls 1000".to_string(),
                "set rtp-timeout 30".to_string(),
            ],
            arguments: vec![
                ArgumentHelp {
                    name: "parameter".to_string(),
                    description: "Configuration parameter name".to_string(),
                    required: true,
                    values: vec![
                        "log-level".to_string(),
                        "max-calls".to_string(),
                        "rtp-timeout".to_string(),
                        "sip-timeout".to_string(),
                        "codec-priority".to_string(),
                    ],
                },
                ArgumentHelp {
                    name: "value".to_string(),
                    description: "Parameter value".to_string(),
                    required: true,
                    values: vec!["<value>".to_string()],
                },
            ],
            related: vec!["get".to_string(), "reload".to_string()],
        });

        // Debug command help
        self.add_command_help(CommandHelp {
            name: "debug".to_string(),
            description: "Control debug output and logging".to_string(),
            usage: "debug [sip|rtp|codec|routing|security|all|off] [level]".to_string(),
            examples: vec![
                "debug sip".to_string(),
                "debug all".to_string(),
                "debug off".to_string(),
                "debug rtp 2".to_string(),
            ],
            arguments: vec![
                ArgumentHelp {
                    name: "component".to_string(),
                    description: "Component to debug".to_string(),
                    required: false,
                    values: vec![
                        "sip".to_string(),
                        "rtp".to_string(),
                        "codec".to_string(),
                        "routing".to_string(),
                        "security".to_string(),
                        "all".to_string(),
                        "off".to_string(),
                    ],
                },
                ArgumentHelp {
                    name: "level".to_string(),
                    description: "Debug level (0-9)".to_string(),
                    required: false,
                    values: vec![
                        "0".to_string(),
                        "1".to_string(),
                        "2".to_string(),
                        "3".to_string(),
                    ],
                },
            ],
            related: vec!["trace".to_string(), "log".to_string()],
        });

        // Codec command help
        self.add_command_help(CommandHelp {
            name: "codec".to_string(),
            description: "Manage codec operations and transcoding".to_string(),
            usage: "codec [list|test|benchmark|stats|priority] [codec-name]".to_string(),
            examples: vec![
                "codec list".to_string(),
                "codec test g729".to_string(),
                "codec benchmark".to_string(),
                "codec stats".to_string(),
            ],
            arguments: vec![
                ArgumentHelp {
                    name: "action".to_string(),
                    description: "Codec action to perform".to_string(),
                    required: false,
                    values: vec![
                        "list".to_string(),
                        "test".to_string(),
                        "benchmark".to_string(),
                        "stats".to_string(),
                        "priority".to_string(),
                    ],
                },
                ArgumentHelp {
                    name: "codec-name".to_string(),
                    description: "Specific codec name".to_string(),
                    required: false,
                    values: vec![
                        "g711u".to_string(),
                        "g711a".to_string(),
                        "g729".to_string(),
                        "g722".to_string(),
                        "opus".to_string(),
                    ],
                },
            ],
            related: vec!["transcode".to_string(), "gpu".to_string()],
        });

        // Log command help
        self.add_command_help(CommandHelp {
            name: "log".to_string(),
            description: "Manage system logging".to_string(),
            usage: "log [level|tail|export|rotate|clear] [parameters]".to_string(),
            examples: vec![
                "log level info".to_string(),
                "log tail 100".to_string(),
                "log export /tmp/redfire.log".to_string(),
                "log clear".to_string(),
            ],
            arguments: vec![ArgumentHelp {
                name: "action".to_string(),
                description: "Log action to perform".to_string(),
                required: false,
                values: vec![
                    "level".to_string(),
                    "tail".to_string(),
                    "export".to_string(),
                    "rotate".to_string(),
                    "clear".to_string(),
                ],
            }],
            related: vec!["debug".to_string(), "trace".to_string()],
        });
    }

    /// Add command help to the system
    fn add_command_help(&mut self, help: CommandHelp) {
        self.command_help.insert(help.name.clone(), help);
    }
}
