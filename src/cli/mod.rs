//! RedFire Switch Interactive CLI
//!
//! Provides an interactive command-line interface similar to FreeSWITCH fs_cli
//! with tab completion, help system, and real-time switch operation capabilities.

use anyhow::{Context, Result};
use colored::*;
use rustyline::error::ReadlineError;
use rustyline::{DefaultEditor, Editor};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

pub mod commands;
pub mod completion;
pub mod help;
pub mod session;

use commands::{Command, CommandExecutor, CommandResult};
use completion::RedFireCompleter;
use help::HelpSystem;
use session::CliSession;

/// Interactive CLI for RedFire Switch
pub struct InteractiveCli {
    editor: Editor<RedFireCompleter, rustyline::history::DefaultHistory>,
    session: Arc<RwLock<CliSession>>,
    pub command_executor: CommandExecutor,
    help_system: HelpSystem,
    running: Arc<RwLock<bool>>,
}

impl InteractiveCli {
    /// Create a new interactive CLI instance
    pub fn new() -> Result<Self> {
        let mut editor = Editor::<RedFireCompleter, rustyline::history::DefaultHistory>::new()
            .context("Failed to create readline editor")?;

        let session = Arc::new(RwLock::new(CliSession::new()));
        let mut completer = RedFireCompleter::new();

        // Set up tab completion
        completer.set_commands(commands::get_all_commands());
        editor.set_helper(Some(completer));

        let command_executor = CommandExecutor::new(session.clone());
        let help_system = HelpSystem::new();
        let running = Arc::new(RwLock::new(true));

        Ok(Self {
            editor,
            session,
            command_executor,
            help_system,
            running,
        })
    }

    /// Start the interactive CLI session
    pub async fn run(&mut self) -> Result<()> {
        self.print_banner();
        self.print_welcome();

        loop {
            // Check if we should continue running
            if !*self.running.read().await {
                break;
            }

            let prompt = self.get_prompt().await;

            match self.editor.readline(&prompt) {
                Ok(line) => {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    // Add to history
                    let _ = self.editor.add_history_entry(line);

                    // Process command
                    if let Err(e) = self.process_command(line).await {
                        eprintln!("{}: {}", "Error".red().bold(), e);
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    println!("^C");
                    continue;
                }
                Err(ReadlineError::Eof) => {
                    println!("exit");
                    break;
                }
                Err(err) => {
                    error!("CLI readline error: {}", err);
                    break;
                }
            }
        }

        println!("{}", "Goodbye!".green());
        Ok(())
    }

    /// Print the RedFire Switch banner
    fn print_banner(&self) {
        let banner = r#"
 ____          _   _____ _            ____        _ _       _     
|  _ \ ___  __| | |  ___(_)_ __ ___  / ___|_      _(_) |_ ___| |__  
| |_) / _ \/ _` | | |_  | | '__/ _ \ \___ \ \ /\ / / | __/ __| '_ \ 
|  _ <  __/ (_| | |  _| | | | |  __/  ___) \ V  V /| | || (__| | | |
|_| \_\___|\__,_| |_|   |_|_|  \___| |____/ \_/\_/ |_|\__\___|_| |_|
                                                                   
        "#;

        println!("{}", banner.bright_red().bold());
        println!("{}", "High-Performance Class 4 SIP Switch".bright_white());
        println!("{}", "Interactive Command Line Interface".bright_cyan());
        println!();
    }

    /// Print welcome message and basic help
    fn print_welcome(&self) {
        println!("{}", "Welcome to RedFire Switch CLI".green().bold());
        println!();
        println!(
            "Type '{}' for help, '{}' to exit",
            "help".yellow(),
            "quit".yellow()
        );
        println!("Use {} for command completion", "TAB".bright_blue().bold());
        println!();
    }

    /// Get the current prompt string
    async fn get_prompt(&self) -> String {
        let session = self.session.read().await;
        let status_color = if session.is_connected() {
            "green"
        } else {
            "red"
        };

        format!(
            "{}@{} > ",
            "redfire".color(status_color).bold(),
            session.get_target_host().color(status_color)
        )
    }

    /// Process a command line input
    async fn process_command(&mut self, line: &str) -> Result<()> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }

        let command_name = parts[0];
        let args = parts[1..].to_vec();

        // Handle built-in commands
        match command_name {
            "help" | "?" => {
                self.handle_help_command(&args).await?;
                return Ok(());
            }
            "quit" | "exit" | "bye" => {
                *self.running.write().await = false;
                return Ok(());
            }
            "clear" | "cls" => {
                print!("\x1B[2J\x1B[1;1H"); // Clear screen and move to top
                return Ok(());
            }
            _ => {}
        }

        // Execute command through the command executor
        match self.command_executor.execute(command_name, args).await {
            Ok(result) => {
                self.display_command_result(result).await;
            }
            Err(e) => {
                eprintln!("{}: {}", "Command Error".red().bold(), e);
            }
        }

        Ok(())
    }

    /// Handle help command  
    pub async fn handle_help_command(&self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            self.help_system.print_general_help();
        } else {
            let topic = args[0];
            self.help_system.print_command_help(topic);
        }
        Ok(())
    }

    /// Display command execution result
    pub async fn display_command_result(&self, result: CommandResult) {
        match result {
            CommandResult::Success(message) => {
                if !message.is_empty() {
                    println!("{}", message);
                }
            }
            CommandResult::Table(headers, rows) => {
                self.print_table(headers, rows);
            }
            CommandResult::Json(value) => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&value).unwrap_or_default()
                );
            }
            CommandResult::Error(message) => {
                eprintln!("{}: {}", "Error".red().bold(), message);
            }
        }
    }

    /// Print a formatted table
    fn print_table(&self, headers: Vec<String>, rows: Vec<Vec<String>>) {
        if headers.is_empty() || rows.is_empty() {
            return;
        }

        // Calculate column widths
        let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
        for row in &rows {
            for (i, cell) in row.iter().enumerate() {
                if i < widths.len() {
                    widths[i] = widths[i].max(cell.len());
                }
            }
        }

        // Print header
        print!("┌");
        for (i, width) in widths.iter().enumerate() {
            print!("{}", "─".repeat(width + 2));
            if i < widths.len() - 1 {
                print!("┬");
            }
        }
        println!("┐");

        print!("│");
        for (i, (header, width)) in headers.iter().zip(&widths).enumerate() {
            print!(" {:width$} ", header.bright_cyan().bold(), width = width);
            if i < widths.len() - 1 {
                print!("│");
            }
        }
        println!("│");

        print!("├");
        for (i, width) in widths.iter().enumerate() {
            print!("{}", "─".repeat(width + 2));
            if i < widths.len() - 1 {
                print!("┼");
            }
        }
        println!("┤");

        // Print rows
        for row in rows {
            print!("│");
            for (i, (cell, width)) in row.iter().zip(&widths).enumerate() {
                print!(" {:width$} ", cell, width = width);
                if i < widths.len() - 1 {
                    print!("│");
                }
            }
            println!("│");
        }

        print!("└");
        for (i, width) in widths.iter().enumerate() {
            print!("{}", "─".repeat(width + 2));
            if i < widths.len() - 1 {
                print!("┴");
            }
        }
        println!("┘");
    }
}
