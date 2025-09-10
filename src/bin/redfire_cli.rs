//! RedFire Switch Interactive CLI
//!
//! An interactive command-line interface for managing and monitoring
//! the RedFire Switch telecommunications platform, similar to FreeSWITCH fs_cli.

use anyhow::{Context, Result};
use clap::Parser;
use colored::*;
use redfire_switch::cli::InteractiveCli;
use std::process;
use tokio;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "redfire-cli")]
#[command(about = "RedFire Switch Interactive CLI")]
#[command(version = "0.1.0")]
struct Args {
    /// Host to connect to
    #[arg(short = 'H', long = "host", default_value = "localhost")]
    host: String,

    /// Port to connect to
    #[arg(short = 'P', long = "port", default_value = "8080")]
    port: u16,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Execute command and exit (non-interactive mode)
    #[arg(short = 'x', long = "execute")]
    execute: Option<String>,

    /// Disable colored output
    #[arg(long = "no-color")]
    no_color: bool,

    /// Connection timeout in seconds
    #[arg(long = "timeout", default_value = "10")]
    timeout: u64,

    /// Log level
    #[arg(long = "log-level", default_value = "info")]
    log_level: String,

    /// Log to file instead of stdout
    #[arg(long = "log-file")]
    log_file: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    setup_logging(&args)?;

    // Display banner if in interactive mode
    if args.execute.is_none() {
        print_startup_banner();
    }

    // Create and configure CLI
    let mut cli = InteractiveCli::new().context("Failed to create CLI instance")?;

    // Auto-connect to the specified host and port
    let target_address = format!("{}:{}", args.host, args.port);
    info!("Attempting to connect to {}", target_address);

    // Get session and attempt connection
    {
        let session_clone = cli.get_session();
        let mut session = session_clone.write().await;
        if let Err(e) = session.connect(target_address.clone()).await {
            warn!("Failed to connect to {}: {}", target_address, e);
            if args.execute.is_some() {
                // For non-interactive mode, exit on connection failure
                eprintln!(
                    "{}: Failed to connect to {}: {}",
                    "Error".red().bold(),
                    target_address,
                    e
                );
                process::exit(1);
            } else {
                // For interactive mode, warn but continue
                eprintln!(
                    "{}: Failed to connect to {}: {}",
                    "Warning".yellow().bold(),
                    target_address,
                    e
                );
                eprintln!("Use 'connect {}' command to try again", target_address);
            }
        } else {
            info!("Successfully connected to {}", target_address);
        }
    }

    // Handle non-interactive execution
    if let Some(ref command) = args.execute {
        return execute_single_command(&mut cli, command, &args).await;
    }

    // Start interactive session
    info!("Starting RedFire Switch CLI");

    match cli.run().await {
        Ok(()) => {
            info!("CLI session ended normally");
            Ok(())
        }
        Err(e) => {
            error!("CLI session ended with error: {}", e);
            eprintln!("{}: {}", "Error".red().bold(), e);
            process::exit(1);
        }
    }
}

/// Setup logging based on command line arguments
fn setup_logging(args: &Args) -> Result<()> {
    let _log_level = match args.log_level.as_str() {
        "trace" => tracing::Level::TRACE,
        "debug" => tracing::Level::DEBUG,
        "info" => tracing::Level::INFO,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => tracing::Level::INFO,
    };

    let subscriber = tracing_subscriber::registry();

    if let Some(log_file) = &args.log_file {
        // Log to file
        let file = std::fs::File::create(log_file).context("Failed to create log file")?;

        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(file)
            .with_ansi(false);

        subscriber.with(file_layer).init();
    } else {
        // Log to stdout/stderr
        let fmt_layer = tracing_subscriber::fmt::layer().with_ansi(!args.no_color);

        subscriber.with(fmt_layer).init();
    }

    Ok(())
}

/// Print startup banner
fn print_startup_banner() {
    if !std::env::var("NO_BANNER").is_ok() {
        let banner = r#"
 ____          _   _____ _            ____        _ _       _     
|  _ \ ___  __| | |  ___(_)_ __ ___  / ___|_      _(_) |_ ___| |__  
| |_) / _ \/ _` | | |_  | | '__/ _ \ \___ \ \ /\ / / | __/ __| '_ \ 
|  _ <  __/ (_| | |  _| | | | |  __/  ___) \ V  V /| | || (__| | | |
|_| \_\___|\__,_| |_|   |_|_|  \___| |____/ \_/\_/ |_|\__\___|_| |_|
                                                                   
"#;

        println!("{}", banner.bright_red().bold());
        println!("{}", "Interactive Command Line Interface".bright_cyan());
        println!("{}", "Version 0.1.0".bright_white());
        println!();
        println!("{}", "Starting interactive session...".bright_green());
        println!(
            "Type '{}' for help, '{}' to exit",
            "help".yellow(),
            "quit".yellow()
        );
        println!();
    }
}

/// Execute a single command in non-interactive mode
async fn execute_single_command(
    cli: &mut InteractiveCli,
    command: &str,
    _args: &Args,
) -> Result<()> {
    info!("Executing command: {}", command);

    // Parse the command and execute it
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(());
    }

    let command_name = parts[0];
    let args = parts[1..].to_vec();

    // Handle built-in commands
    match command_name {
        "help" | "?" => {
            cli.handle_help_command(&args).await?;
            return Ok(());
        }
        "version" => {
            println!("RedFire Switch v0.1.0");
            println!("Built with Rust");
            println!("GPU acceleration enabled");
            return Ok(());
        }
        _ => {}
    }

    // Execute command through the command executor
    match cli.command_executor.execute(command_name, args).await {
        Ok(result) => {
            cli.display_command_result(result).await;
        }
        Err(e) => {
            eprintln!("{}: {}", "Command Error".red().bold(), e);
            std::process::exit(1);
        }
    }

    Ok(())
}

/// Handle graceful shutdown
#[allow(dead_code)]
async fn handle_shutdown() {
    info!("Received shutdown signal");
    println!("\n{}", "Shutting down...".yellow());

    // Clean up resources here
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    println!("{}", "Goodbye!".green());
}
