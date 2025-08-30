//! Tab completion engine for RedFire Switch CLI
//!
//! Provides intelligent tab completion for commands, arguments, and values
//! similar to FreeSWITCH fs_cli completion system.

use rustyline::completion::{Completer, FilenameCompleter, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::{Highlighter, MatchingBracketHighlighter};
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::validate::{self, MatchingBracketValidator, Validator};
use rustyline::{Context, Helper};
use std::borrow::Cow::{self, Borrowed, Owned};
use std::collections::HashMap;

use super::commands::Command;

/// Tab completion helper for RedFire Switch CLI
pub struct RedFireCompleter {
    commands: HashMap<String, Command>,
    filename_completer: FilenameCompleter,
    history_hinter: HistoryHinter,
    highlighter: MatchingBracketHighlighter,
    validator: MatchingBracketValidator,
}

impl RedFireCompleter {
    /// Create a new completer instance
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            filename_completer: FilenameCompleter::new(),
            history_hinter: HistoryHinter::new(),
            highlighter: MatchingBracketHighlighter::new(),
            validator: MatchingBracketValidator::new(),
        }
    }

    /// Set available commands for completion
    pub fn set_commands(&mut self, commands: Vec<Command>) {
        self.commands = commands
            .into_iter()
            .map(|cmd| (cmd.name.clone(), cmd))
            .collect();
    }

    /// Complete command names
    fn complete_command(&self, line: &str, pos: usize) -> rustyline::Result<(usize, Vec<Pair>)> {
        let words: Vec<&str> = line[..pos].split_whitespace().collect();

        if words.is_empty() || (words.len() == 1 && !line.ends_with(' ')) {
            // Complete command name
            let prefix = words.last().map_or("", |v| *v);
            let matches: Vec<Pair> = self
                .commands
                .keys()
                .filter(|name| name.starts_with(prefix))
                .map(|name| Pair {
                    display: name.clone(),
                    replacement: name.clone(),
                })
                .collect();

            let start = pos - prefix.len();
            Ok((start, matches))
        } else {
            // Complete command arguments
            let command_name = words[0];
            if let Some(command) = self.commands.get(command_name) {
                self.complete_command_args(command, line, pos, &words[1..])
            } else {
                Ok((pos, vec![]))
            }
        }
    }

    /// Complete command arguments based on command definition
    fn complete_command_args(
        &self,
        command: &Command,
        line: &str,
        pos: usize,
        args: &[&str],
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let current_arg = args.last().map_or("", |v| *v);
        let arg_index = if line.ends_with(' ') {
            args.len()
        } else {
            args.len().saturating_sub(1)
        };

        let matches = match command.name.as_str() {
            // Status commands
            "status" => self.complete_status_args(current_arg, arg_index),
            "show" => self.complete_show_args(current_arg, arg_index),

            // Call control commands
            "calls" => self.complete_calls_args(current_arg, arg_index),
            "hangup" => self.complete_hangup_args(current_arg, arg_index),

            // Configuration commands
            "set" => self.complete_set_args(current_arg, arg_index),
            "get" => self.complete_get_args(current_arg, arg_index),
            "reload" => self.complete_reload_args(current_arg, arg_index),

            // Gateway and trunk commands
            "gateway" => self.complete_gateway_args(current_arg, arg_index),
            "trunk" => self.complete_trunk_args(current_arg, arg_index),

            // Route commands
            "route" => self.complete_route_args(current_arg, arg_index),
            "lcr" => self.complete_lcr_args(current_arg, arg_index),

            // Debug commands
            "debug" => self.complete_debug_args(current_arg, arg_index),
            "trace" => self.complete_trace_args(current_arg, arg_index),

            // Codec commands
            "codec" => self.complete_codec_args(current_arg, arg_index),
            "transcode" => self.complete_transcode_args(current_arg, arg_index),

            // Log commands
            "log" => self.complete_log_args(current_arg, arg_index),

            // File operations - skip for now since we need Context
            "load" | "save" | "export" | "import" => {
                vec![]
            }

            _ => vec![],
        };

        let start = pos - current_arg.len();
        Ok((start, matches))
    }

    /// Complete status command arguments
    fn complete_status_args(&self, current_arg: &str, arg_index: usize) -> Vec<Pair> {
        match arg_index {
            0 => vec![
                "calls", "channels", "gateways", "trunks", "codecs", "memory", "cpu", "network",
                "database", "security", "all",
            ]
            .into_iter()
            .filter(|&arg| arg.starts_with(current_arg))
            .map(|arg| Pair {
                display: format!("{} - {}", arg, self.get_arg_description("status", arg)),
                replacement: arg.to_string(),
            })
            .collect(),
            _ => vec![],
        }
    }

    /// Complete show command arguments
    fn complete_show_args(&self, current_arg: &str, arg_index: usize) -> Vec<Pair> {
        match arg_index {
            0 => vec![
                "calls",
                "channels",
                "gateways",
                "trunks",
                "routes",
                "config",
                "stats",
                "alarms",
                "events",
                "logs",
                "performance",
                "security",
            ]
            .into_iter()
            .filter(|&arg| arg.starts_with(current_arg))
            .map(|arg| Pair {
                display: format!("{} - {}", arg, self.get_arg_description("show", arg)),
                replacement: arg.to_string(),
            })
            .collect(),
            1 => match current_arg {
                "calls" => vec!["active", "completed", "failed", "summary"],
                "channels" => vec!["sip", "rtp", "all"],
                "gateways" => vec!["active", "inactive", "all"],
                _ => vec![],
            }
            .into_iter()
            .filter(|&arg| arg.starts_with(current_arg))
            .map(|arg| Pair {
                display: arg.to_string(),
                replacement: arg.to_string(),
            })
            .collect(),
            _ => vec![],
        }
    }

    /// Complete calls command arguments
    fn complete_calls_args(&self, current_arg: &str, arg_index: usize) -> Vec<Pair> {
        match arg_index {
            0 => vec!["list", "count", "active", "history", "search"]
                .into_iter()
                .filter(|&arg| arg.starts_with(current_arg))
                .map(|arg| Pair {
                    display: format!("{} - {}", arg, self.get_arg_description("calls", arg)),
                    replacement: arg.to_string(),
                })
                .collect(),
            _ => vec![],
        }
    }

    /// Complete hangup command arguments (call IDs would be dynamically loaded)
    fn complete_hangup_args(&self, current_arg: &str, arg_index: usize) -> Vec<Pair> {
        match arg_index {
            0 => {
                // In a real implementation, this would query active calls
                vec![
                    Pair {
                        display: "all - Hangup all active calls".to_string(),
                        replacement: "all".to_string(),
                    },
                    Pair {
                        display: "<call-id> - Hangup specific call".to_string(),
                        replacement: "".to_string(),
                    },
                ]
            }
            _ => vec![],
        }
    }

    /// Complete set command arguments
    fn complete_set_args(&self, current_arg: &str, arg_index: usize) -> Vec<Pair> {
        match arg_index {
            0 => vec![
                "log-level",
                "max-calls",
                "rtp-timeout",
                "sip-timeout",
                "codec-priority",
                "gateway-status",
                "debug-level",
            ]
            .into_iter()
            .filter(|&arg| arg.starts_with(current_arg))
            .map(|arg| Pair {
                display: format!("{} - {}", arg, self.get_arg_description("set", arg)),
                replacement: arg.to_string(),
            })
            .collect(),
            1 => {
                // Provide context-sensitive value completion
                vec![]
            }
            _ => vec![],
        }
    }

    /// Complete get command arguments
    fn complete_get_args(&self, current_arg: &str, arg_index: usize) -> Vec<Pair> {
        // Similar to set, but for getting values
        self.complete_set_args(current_arg, arg_index)
    }

    /// Complete reload command arguments
    fn complete_reload_args(&self, current_arg: &str, arg_index: usize) -> Vec<Pair> {
        match arg_index {
            0 => vec!["config", "routes", "gateways", "trunks", "all"]
                .into_iter()
                .filter(|&arg| arg.starts_with(current_arg))
                .map(|arg| Pair {
                    display: format!("{} - {}", arg, self.get_arg_description("reload", arg)),
                    replacement: arg.to_string(),
                })
                .collect(),
            _ => vec![],
        }
    }

    /// Complete gateway command arguments
    fn complete_gateway_args(&self, current_arg: &str, arg_index: usize) -> Vec<Pair> {
        match arg_index {
            0 => vec!["list", "status", "enable", "disable", "test", "stats"]
                .into_iter()
                .filter(|&arg| arg.starts_with(current_arg))
                .map(|arg| Pair {
                    display: format!("{} - {}", arg, self.get_arg_description("gateway", arg)),
                    replacement: arg.to_string(),
                })
                .collect(),
            _ => vec![],
        }
    }

    /// Complete trunk command arguments  
    fn complete_trunk_args(&self, current_arg: &str, arg_index: usize) -> Vec<Pair> {
        match arg_index {
            0 => vec!["list", "status", "stats", "test", "reset"]
                .into_iter()
                .filter(|&arg| arg.starts_with(current_arg))
                .map(|arg| Pair {
                    display: format!("{} - {}", arg, self.get_arg_description("trunk", arg)),
                    replacement: arg.to_string(),
                })
                .collect(),
            _ => vec![],
        }
    }

    /// Complete route command arguments
    fn complete_route_args(&self, current_arg: &str, arg_index: usize) -> Vec<Pair> {
        match arg_index {
            0 => vec!["list", "add", "remove", "test", "stats", "refresh"]
                .into_iter()
                .filter(|&arg| arg.starts_with(current_arg))
                .map(|arg| Pair {
                    display: format!("{} - {}", arg, self.get_arg_description("route", arg)),
                    replacement: arg.to_string(),
                })
                .collect(),
            _ => vec![],
        }
    }

    /// Complete LCR command arguments
    fn complete_lcr_args(&self, current_arg: &str, arg_index: usize) -> Vec<Pair> {
        match arg_index {
            0 => vec!["lookup", "test", "stats", "refresh", "export"]
                .into_iter()
                .filter(|&arg| arg.starts_with(current_arg))
                .map(|arg| Pair {
                    display: format!("{} - {}", arg, self.get_arg_description("lcr", arg)),
                    replacement: arg.to_string(),
                })
                .collect(),
            _ => vec![],
        }
    }

    /// Complete debug command arguments
    fn complete_debug_args(&self, current_arg: &str, arg_index: usize) -> Vec<Pair> {
        match arg_index {
            0 => vec!["sip", "rtp", "codec", "routing", "security", "all", "off"]
                .into_iter()
                .filter(|&arg| arg.starts_with(current_arg))
                .map(|arg| Pair {
                    display: format!("{} - {}", arg, self.get_arg_description("debug", arg)),
                    replacement: arg.to_string(),
                })
                .collect(),
            _ => vec![],
        }
    }

    /// Complete trace command arguments
    fn complete_trace_args(&self, current_arg: &str, arg_index: usize) -> Vec<Pair> {
        match arg_index {
            0 => vec!["start", "stop", "status", "export"]
                .into_iter()
                .filter(|&arg| arg.starts_with(current_arg))
                .map(|arg| Pair {
                    display: format!("{} - {}", arg, self.get_arg_description("trace", arg)),
                    replacement: arg.to_string(),
                })
                .collect(),
            _ => vec![],
        }
    }

    /// Complete codec command arguments
    fn complete_codec_args(&self, current_arg: &str, arg_index: usize) -> Vec<Pair> {
        match arg_index {
            0 => vec!["list", "test", "benchmark", "stats", "priority"]
                .into_iter()
                .filter(|&arg| arg.starts_with(current_arg))
                .map(|arg| Pair {
                    display: format!("{} - {}", arg, self.get_arg_description("codec", arg)),
                    replacement: arg.to_string(),
                })
                .collect(),
            1 => {
                if current_arg == "test" || current_arg == "priority" {
                    vec!["g711u", "g711a", "g729", "g722", "opus", "all"]
                        .into_iter()
                        .map(|codec| Pair {
                            display: codec.to_string(),
                            replacement: codec.to_string(),
                        })
                        .collect()
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    /// Complete transcode command arguments
    fn complete_transcode_args(&self, current_arg: &str, arg_index: usize) -> Vec<Pair> {
        let codecs = vec!["g711u", "g711a", "g729", "g722", "opus"];
        match arg_index {
            0 | 1 => codecs
                .into_iter()
                .filter(|&codec| codec.starts_with(current_arg))
                .map(|codec| Pair {
                    display: codec.to_string(),
                    replacement: codec.to_string(),
                })
                .collect(),
            _ => vec![],
        }
    }

    /// Complete log command arguments
    fn complete_log_args(&self, current_arg: &str, arg_index: usize) -> Vec<Pair> {
        match arg_index {
            0 => vec!["level", "tail", "export", "rotate", "clear"]
                .into_iter()
                .filter(|&arg| arg.starts_with(current_arg))
                .map(|arg| Pair {
                    display: format!("{} - {}", arg, self.get_arg_description("log", arg)),
                    replacement: arg.to_string(),
                })
                .collect(),
            1 => {
                if current_arg == "level" {
                    vec!["error", "warn", "info", "debug", "trace"]
                        .into_iter()
                        .map(|level| Pair {
                            display: level.to_string(),
                            replacement: level.to_string(),
                        })
                        .collect()
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    /// Get description for command argument
    fn get_arg_description(&self, command: &str, arg: &str) -> &'static str {
        match (command, arg) {
            // Status descriptions
            ("status", "calls") => "Show active call statistics",
            ("status", "channels") => "Show channel information",
            ("status", "gateways") => "Show gateway status",
            ("status", "trunks") => "Show trunk group status",
            ("status", "codecs") => "Show codec statistics",
            ("status", "memory") => "Show memory usage",
            ("status", "cpu") => "Show CPU utilization",
            ("status", "network") => "Show network statistics",
            ("status", "database") => "Show database status",
            ("status", "security") => "Show security status",
            ("status", "all") => "Show all status information",

            // Show descriptions
            ("show", "calls") => "Display call information",
            ("show", "channels") => "Display channel details",
            ("show", "gateways") => "Display gateway configuration",
            ("show", "routes") => "Display routing table",

            // Other command descriptions
            ("calls", "list") => "List all calls",
            ("calls", "count") => "Count active calls",
            ("calls", "active") => "Show active calls only",

            _ => "Command argument",
        }
    }
}

impl Helper for RedFireCompleter {}

impl Completer for RedFireCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        self.complete_command(line, pos)
    }
}

impl Hinter for RedFireCompleter {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        self.history_hinter.hint(line, pos, ctx)
    }
}

impl Highlighter for RedFireCompleter {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        default: bool,
    ) -> Cow<'b, str> {
        if default {
            Borrowed(prompt)
        } else {
            Borrowed(prompt)
        }
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Owned(format!("\x1b[1;90m{}\x1b[0m", hint)) // Gray hint text
    }

    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        self.highlighter.highlight(line, pos)
    }

    fn highlight_char(&self, line: &str, pos: usize, forced: bool) -> bool {
        self.highlighter.highlight_char(line, pos, forced)
    }
}

impl Validator for RedFireCompleter {
    fn validate(
        &self,
        ctx: &mut validate::ValidationContext,
    ) -> rustyline::Result<validate::ValidationResult> {
        self.validator.validate(ctx)
    }

    fn validate_while_typing(&self) -> bool {
        self.validator.validate_while_typing()
    }
}
