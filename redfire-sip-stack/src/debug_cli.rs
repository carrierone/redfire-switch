/*
 * Redfire Switch - SIP Debugging CLI with Color-Coded Filtering
 * Copyright (C) 2025 Carrier One Inc and contributors
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

//! # SIP Debugging CLI
//!
//! Provides real-time SIP message debugging with:
//! - Color-coded message display
//! - Filtering by trunk, ANI, DNIS, IP, response codes
//! - Message flow visualization
//! - Performance statistics
//! - Export capabilities

use anyhow::{anyhow, Result};
use colored::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::{HashMap, VecDeque};
use std::io::{stdin, stdout, Write};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::{clear, cursor, style};
use tokio::sync::{mpsc, RwLock};
use tracing::info;

use crate::parser::SipMessage;

/// SIP debug configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipDebugConfig {
    /// Enable real-time debugging
    pub enabled: bool,
    /// Maximum messages to buffer
    pub max_buffer_size: usize,
    /// Auto-scroll in real-time mode
    pub auto_scroll: bool,
    /// Show timestamps
    pub show_timestamps: bool,
    /// Show message details
    pub show_details: bool,
    /// Color scheme
    pub color_scheme: ColorScheme,
    /// Export format
    pub export_format: ExportFormat,
}

impl Default for SipDebugConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_buffer_size: 10000,
            auto_scroll: true,
            show_timestamps: true,
            show_details: true,
            color_scheme: ColorScheme::Default,
            export_format: ExportFormat::Json,
        }
    }
}

/// Color schemes for different message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ColorScheme {
    Default,
    HighContrast,
    Monochrome,
    Custom(HashMap<String, String>),
}

/// Export formats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportFormat {
    Json,
    Pcap,
    Text,
    Csv,
}

/// SIP debug filter criteria
#[derive(Debug, Clone, Default)]
pub struct SipDebugFilter {
    /// Filter by origination trunk
    pub orig_trunk: Option<String>,
    /// Filter by termination trunk
    pub term_trunk: Option<String>,
    /// Filter by ANI (calling number)
    pub ani: Option<String>,
    /// Filter by DNIS (called number)
    pub dnis: Option<String>,
    /// Filter by specific IP address
    pub ip_address: Option<IpAddr>,
    /// Filter by SIP response codes
    pub response_codes: Vec<u16>,
    /// Filter by SIP methods
    pub methods: Vec<String>,
    /// Filter by Call-ID
    pub call_id: Option<String>,
    /// Filter by User-Agent
    pub user_agent: Option<String>,
    /// Regular expression filter
    pub regex_filter: Option<Regex>,
    /// Time range filter
    pub time_range: Option<(SystemTime, SystemTime)>,
}

/// SIP message for debugging
#[derive(Debug, Clone, Serialize)]
pub struct SipDebugMessage {
    /// Original SIP message
    pub message: SipMessage,
    /// Message direction
    pub direction: MessageDirection,
    /// Trunk information
    pub trunk_info: Option<TrunkInfo>,
    /// Call information
    pub call_info: Option<CallInfo>,
    /// Timing information
    pub timing: MessageTiming,
    /// Processing result
    pub processing_result: Option<ProcessingResult>,
}

/// Message direction
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum MessageDirection {
    Inbound,
    Outbound,
    Internal,
}

/// Trunk information
#[derive(Debug, Clone, Serialize)]
pub struct TrunkInfo {
    pub trunk_id: String,
    pub trunk_name: String,
    pub trunk_type: TrunkType,
    pub provider: String,
}

/// Trunk types
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum TrunkType {
    Origination,
    Termination,
    Internal,
    Emergency,
}

/// Call information extracted from SIP message
#[derive(Debug, Clone, Serialize)]
pub struct CallInfo {
    pub call_id: String,
    pub ani: Option<String>,
    pub dnis: Option<String>,
    pub user_agent: Option<String>,
    pub method: Option<String>,
    pub response_code: Option<u16>,
    pub cseq: Option<u32>,
}

/// Message timing information
#[derive(Debug, Clone, Serialize)]
pub struct MessageTiming {
    pub received_at: SystemTime,
    pub processed_at: Option<SystemTime>,
    pub response_time: Option<Duration>,
}

/// Processing result
#[derive(Debug, Clone, Serialize)]
pub struct ProcessingResult {
    pub status: ProcessingStatus,
    pub route_decision: Option<String>,
    pub error_message: Option<String>,
}

/// Processing status
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ProcessingStatus {
    Success,
    Failed,
    Routed,
    Blocked,
    Retrying,
}

/// SIP debug CLI
pub struct SipDebugCli {
    config: SipDebugConfig,
    /// Message buffer
    message_buffer: Arc<RwLock<VecDeque<SipDebugMessage>>>,
    /// Current filter
    current_filter: Arc<RwLock<SipDebugFilter>>,
    /// Message receiver
    message_receiver: Option<mpsc::Receiver<SipDebugMessage>>,
    /// Statistics
    statistics: Arc<RwLock<DebugStatistics>>,
    /// Export buffer
    export_buffer: Arc<RwLock<Vec<SipDebugMessage>>>,
}

/// Debug statistics
#[derive(Debug, Clone, Default)]
pub struct DebugStatistics {
    pub total_messages: usize,
    pub filtered_messages: usize,
    pub invites: usize,
    pub responses_2xx: usize,
    pub responses_4xx: usize,
    pub responses_5xx: usize,
    pub responses_6xx: usize,
    pub average_response_time: Duration,
    pub messages_per_second: f64,
    pub start_time: Option<SystemTime>,
}

impl SipDebugCli {
    /// Create new SIP debug CLI
    pub fn new(config: SipDebugConfig) -> Self {
        Self {
            config,
            message_buffer: Arc::new(RwLock::new(VecDeque::new())),
            current_filter: Arc::new(RwLock::new(SipDebugFilter::default())),
            message_receiver: None,
            statistics: Arc::new(RwLock::new(DebugStatistics::default())),
            export_buffer: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Start the SIP debug CLI
    pub async fn start(&mut self, message_receiver: mpsc::Receiver<SipDebugMessage>) -> Result<()> {
        self.message_receiver = Some(message_receiver);

        // Initialize statistics
        {
            let mut stats = self.statistics.write().await;
            stats.start_time = Some(SystemTime::now());
        }

        info!("Starting SIP Debug CLI");
        println!("{}", self.format_header());

        // Start message processing task
        let message_buffer = self.message_buffer.clone();
        let current_filter = self.current_filter.clone();
        let statistics = self.statistics.clone();
        let config = self.config.clone();

        if let Some(mut receiver) = self.message_receiver.take() {
            tokio::spawn(async move {
                while let Some(debug_message) = receiver.recv().await {
                    Self::process_debug_message(
                        debug_message,
                        &message_buffer,
                        &current_filter,
                        &statistics,
                        &config,
                    )
                    .await;
                }
            });
        }

        // Start interactive CLI
        self.start_interactive_mode().await?;

        Ok(())
    }

    /// Process incoming debug message
    async fn process_debug_message(
        debug_message: SipDebugMessage,
        message_buffer: &Arc<RwLock<VecDeque<SipDebugMessage>>>,
        current_filter: &Arc<RwLock<SipDebugFilter>>,
        statistics: &Arc<RwLock<DebugStatistics>>,
        config: &SipDebugConfig,
    ) {
        // Update statistics
        {
            let mut stats = statistics.write().await;
            stats.total_messages += 1;

            if let Some(call_info) = &debug_message.call_info {
                if let Some(method) = &call_info.method {
                    if method == "INVITE" {
                        stats.invites += 1;
                    }
                }

                if let Some(code) = call_info.response_code {
                    match code {
                        200..=299 => stats.responses_2xx += 1,
                        400..=499 => stats.responses_4xx += 1,
                        500..=599 => stats.responses_5xx += 1,
                        600..=699 => stats.responses_6xx += 1,
                        _ => {}
                    }
                }
            }

            // Calculate messages per second
            if let Some(start_time) = stats.start_time {
                let elapsed = SystemTime::now()
                    .duration_since(start_time)
                    .unwrap_or(Duration::from_secs(1));
                stats.messages_per_second = stats.total_messages as f64 / elapsed.as_secs_f64();
            }
        }

        // Apply filter
        let filter = current_filter.read().await;
        if Self::message_matches_filter(&debug_message, &filter) {
            let mut buffer = message_buffer.write().await;

            // Update filtered count
            {
                let mut stats = statistics.write().await;
                stats.filtered_messages += 1;
            }

            buffer.push_back(debug_message.clone());

            // Maintain buffer size
            if buffer.len() > config.max_buffer_size {
                buffer.pop_front();
            }

            // Print message if auto-scroll is enabled
            if config.auto_scroll {
                println!("{}", Self::format_debug_message(&debug_message, config));
            }
        }
    }

    /// Check if message matches current filter
    fn message_matches_filter(message: &SipDebugMessage, filter: &SipDebugFilter) -> bool {
        // Trunk filter
        if let Some(ref orig_trunk) = filter.orig_trunk {
            if let Some(ref trunk_info) = message.trunk_info {
                if trunk_info.trunk_type == TrunkType::Origination
                    && trunk_info.trunk_id != *orig_trunk
                {
                    return false;
                }
            }
        }

        if let Some(ref term_trunk) = filter.term_trunk {
            if let Some(ref trunk_info) = message.trunk_info {
                if trunk_info.trunk_type == TrunkType::Termination
                    && trunk_info.trunk_id != *term_trunk
                {
                    return false;
                }
            }
        }

        // ANI/DNIS filter
        if let Some(ref call_info) = message.call_info {
            if let Some(ref ani_filter) = filter.ani {
                if let Some(ref ani) = call_info.ani {
                    if !ani.contains(ani_filter) {
                        return false;
                    }
                } else {
                    return false;
                }
            }

            if let Some(ref dnis_filter) = filter.dnis {
                if let Some(ref dnis) = call_info.dnis {
                    if !dnis.contains(dnis_filter) {
                        return false;
                    }
                } else {
                    return false;
                }
            }

            // Response code filter
            if !filter.response_codes.is_empty() {
                if let Some(code) = call_info.response_code {
                    if !filter.response_codes.contains(&code) {
                        return false;
                    }
                } else {
                    return false;
                }
            }

            // Method filter
            if !filter.methods.is_empty() {
                if let Some(ref method) = call_info.method {
                    if !filter
                        .methods
                        .iter()
                        .any(|m| m.eq_ignore_ascii_case(method))
                    {
                        return false;
                    }
                } else {
                    return false;
                }
            }

            // Call-ID filter
            if let Some(ref call_id_filter) = filter.call_id {
                if !call_info.call_id.contains(call_id_filter) {
                    return false;
                }
            }

            // User-Agent filter
            if let Some(ref ua_filter) = filter.user_agent {
                if let Some(ref user_agent) = call_info.user_agent {
                    if !user_agent
                        .to_lowercase()
                        .contains(&ua_filter.to_lowercase())
                    {
                        return false;
                    }
                } else {
                    return false;
                }
            }
        }

        // IP address filter
        if let Some(ip_filter) = filter.ip_address {
            if message.message.source.ip() != ip_filter
                && message.message.destination.ip() != ip_filter
            {
                return false;
            }
        }

        // Time range filter
        if let Some((start, end)) = filter.time_range {
            if message.timing.received_at < start || message.timing.received_at > end {
                return false;
            }
        }

        // Regex filter
        if let Some(ref regex) = filter.regex_filter {
            let message_text = format!("{:?}", message.message);
            if !regex.is_match(&message_text) {
                return false;
            }
        }

        true
    }

    /// Start interactive CLI mode
    async fn start_interactive_mode(&self) -> Result<()> {
        println!("{}", self.format_help());

        let stdin = stdin();
        let mut stdout = stdout().into_raw_mode()?;

        write!(stdout, "{}{}", clear::All, cursor::Goto(1, 1))?;
        stdout.flush()?;

        for key in stdin.keys() {
            match key? {
                Key::Char('q') => {
                    write!(stdout, "{}Exiting SIP Debug CLI...\r\n", style::Reset)?;
                    break;
                }
                Key::Char('h') => {
                    write!(stdout, "{}{}", clear::All, self.format_help())?;
                }
                Key::Char('s') => {
                    write!(stdout, "{}{}", clear::All, self.format_statistics().await)?;
                }
                Key::Char('f') => {
                    write!(stdout, "{}Enter filter command: ", clear::All)?;
                    stdout.flush()?;
                    // TODO: Implement interactive filter input
                }
                Key::Char('c') => {
                    write!(stdout, "{}{}", clear::All, cursor::Goto(1, 1))?;
                    self.clear_buffer().await;
                }
                Key::Char('p') => {
                    write!(
                        stdout,
                        "{}{}",
                        clear::All,
                        self.format_recent_messages().await
                    )?;
                }
                Key::Char('e') => {
                    self.export_messages().await?;
                    write!(stdout, "Messages exported successfully!\r\n")?;
                }
                _ => {}
            }
            stdout.flush()?;
        }

        Ok(())
    }

    /// Format debug message for display
    fn format_debug_message(message: &SipDebugMessage, config: &SipDebugConfig) -> String {
        let mut output = String::new();

        // Timestamp
        if config.show_timestamps {
            let timestamp = message
                .timing
                .received_at
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_secs();
            output.push_str(&format!("[{timestamp}] ").dimmed().to_string());
        }

        // Direction indicator
        let direction_str = match message.direction {
            MessageDirection::Inbound => "→".green(),
            MessageDirection::Outbound => "←".blue(),
            MessageDirection::Internal => "↔".yellow(),
        };
        output.push_str(&format!("{direction_str} "));

        // Source and destination
        output.push_str(&format!(
            "{} → {} ",
            message.message.source.to_string().cyan(),
            message.message.destination.to_string().magenta()
        ));

        // Method/Response
        if let Some(ref call_info) = message.call_info {
            if let Some(ref method) = call_info.method {
                output.push_str(&method.bright_green().bold().to_string());
            } else if let Some(code) = call_info.response_code {
                let color_code = match code {
                    100..=199 => code.to_string().bright_blue(),
                    200..=299 => code.to_string().bright_green(),
                    300..=399 => code.to_string().bright_yellow(),
                    400..=499 => code.to_string().bright_red(),
                    500..=599 => code.to_string().red(),
                    600..=699 => code.to_string().bright_red().bold(),
                    _ => code.to_string().white(),
                };
                output.push_str(&color_code.to_string());
            }
        }

        // Call information
        if let Some(ref call_info) = message.call_info {
            if let Some(ref ani) = call_info.ani {
                output.push_str(&format!(" ANI:{}", ani.bright_cyan()));
            }
            if let Some(ref dnis) = call_info.dnis {
                output.push_str(&format!(" DNIS:{}", dnis.bright_magenta()));
            }
            if config.show_details {
                output.push_str(&format!(" Call-ID:{}", call_info.call_id.dimmed()));
            }
        }

        // Trunk information
        if let Some(ref trunk_info) = message.trunk_info {
            let trunk_color = match trunk_info.trunk_type {
                TrunkType::Origination => trunk_info.trunk_name.green(),
                TrunkType::Termination => trunk_info.trunk_name.blue(),
                TrunkType::Internal => trunk_info.trunk_name.yellow(),
                TrunkType::Emergency => trunk_info.trunk_name.red().bold(),
            };
            output.push_str(&format!(" Trunk:{trunk_color}"));
        }

        // Processing result
        if let Some(ref result) = message.processing_result {
            let status_str = match result.status {
                ProcessingStatus::Success => "✓".bright_green(),
                ProcessingStatus::Failed => "✗".bright_red(),
                ProcessingStatus::Routed => "→".bright_blue(),
                ProcessingStatus::Blocked => "⊘".bright_red(),
                ProcessingStatus::Retrying => "↻".bright_yellow(),
            };
            output.push_str(&format!(" {status_str}"));

            if let Some(ref route) = result.route_decision {
                output.push_str(&format!(" Route:{}", route.bright_blue()));
            }

            if let Some(ref error) = result.error_message {
                output.push_str(&format!(" Error:{}", error.bright_red()));
            }
        }

        // Response time
        if let Some(response_time) = message.timing.response_time {
            let time_color = if response_time.as_millis() < 100 {
                response_time.as_millis().to_string().green()
            } else if response_time.as_millis() < 1000 {
                response_time.as_millis().to_string().yellow()
            } else {
                response_time.as_millis().to_string().red()
            };
            output.push_str(&format!(" {time_color}ms"));
        }

        output.push('\n');

        // Show message details if enabled
        if config.show_details {
            // TODO: Add formatted SIP message content
        }

        output
    }

    /// Format CLI header
    fn format_header(&self) -> String {
        format!(
            "{}{}{}",
            "🔍 Redfire Switch - SIP Debug CLI".bright_green().bold(),
            "\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n"
                .dimmed(),
            "Press 'h' for help, 'q' to quit, 's' for statistics\n".bright_yellow()
        )
    }

    /// Format help text
    fn format_help(&self) -> String {
        let inbound_outbound_internal = format!(
            "  {} Inbound   {} Outbound   {} Internal\n",
            "→".green(),
            "←".blue(),
            "↔".yellow()
        );
        let success_failed_routed = format!(
            "  {} Success   {} Failed     {} Routed\n",
            "✓".bright_green(),
            "✗".bright_red(),
            "→".bright_blue()
        );

        format!("{}{}{}{}{}{}{}{}{}{}",
            "📖 SIP Debug CLI Help\n".bright_green().bold(),
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n".dimmed(),
            "Navigation:\n".bright_blue().bold(),
            "  h - Show this help\n  q - Quit the debugger\n  s - Show statistics\n  c - Clear message buffer\n  p - Print recent messages\n  e - Export messages\n",
            "\nFilters:\n".bright_blue().bold(),
            "  Use filter commands to narrow down messages:\n  - orig_trunk:<trunk_id>    Filter by origination trunk\n  - term_trunk:<trunk_id>    Filter by termination trunk\n  - ani:<number>             Filter by calling number\n  - dnis:<number>            Filter by called number\n  - ip:<address>             Filter by IP address\n  - code:<response_code>     Filter by SIP response code\n  - method:<sip_method>      Filter by SIP method\n  - regex:<pattern>          Filter by regex pattern\n",
            "\nColor Legend:\n".bright_blue().bold(),
            inbound_outbound_internal,
            success_failed_routed,
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n".dimmed(),
        )
    }

    /// Format statistics
    async fn format_statistics(&self) -> String {
        let stats = self.statistics.read().await;
        let uptime = if let Some(start_time) = stats.start_time {
            SystemTime::now()
                .duration_since(start_time)
                .unwrap_or(Duration::ZERO)
        } else {
            Duration::ZERO
        };

        format!(
            "{}{}{}{}{}{}{}{}{}{}{}{}",
            "📊 SIP Debug Statistics\n".bright_green().bold(),
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n"
                .dimmed(),
            format!("Uptime: {}s\n", uptime.as_secs()),
            format!("Total Messages: {}\n", stats.total_messages),
            format!("Filtered Messages: {}\n", stats.filtered_messages),
            format!("Messages/Second: {:.2}\n", stats.messages_per_second),
            format!("INVITE Requests: {}\n", stats.invites),
            format!(
                "2xx Responses: {}\n",
                stats.responses_2xx.to_string().green()
            ),
            format!(
                "4xx Responses: {}\n",
                stats.responses_4xx.to_string().yellow()
            ),
            format!("5xx Responses: {}\n", stats.responses_5xx.to_string().red()),
            format!(
                "6xx Responses: {}\n",
                stats.responses_6xx.to_string().bright_red()
            ),
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n"
                .dimmed(),
        )
    }

    /// Format recent messages
    async fn format_recent_messages(&self) -> String {
        let buffer = self.message_buffer.read().await;
        let mut output = String::new();

        output.push_str(&"📝 Recent SIP Messages\n".bright_green().bold());
        output.push_str(
            &"━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n"
                .dimmed(),
        );

        for message in buffer.iter().rev().take(20) {
            output.push_str(&Self::format_debug_message(message, &self.config));
        }

        output
    }

    /// Clear message buffer
    async fn clear_buffer(&self) {
        let mut buffer = self.message_buffer.write().await;
        buffer.clear();

        let mut stats = self.statistics.write().await;
        stats.filtered_messages = 0;
    }

    /// Export messages
    async fn export_messages(&self) -> Result<()> {
        let buffer = self.message_buffer.read().await;
        let export_data = buffer.iter().cloned().collect::<Vec<_>>();

        match self.config.export_format {
            ExportFormat::Json => {
                let json = serde_json::to_string_pretty(&export_data)?;
                std::fs::write("sip_debug_export.json", json)?;
            }
            ExportFormat::Text => {
                let mut text = String::new();
                for message in &export_data {
                    text.push_str(&Self::format_debug_message(message, &self.config));
                }
                std::fs::write("sip_debug_export.txt", text)?;
            }
            ExportFormat::Csv => {
                // TODO: Implement CSV export
            }
            ExportFormat::Pcap => {
                // TODO: Implement PCAP export
            }
        }

        Ok(())
    }

    /// Apply filter from command string
    pub async fn apply_filter(&self, filter_command: &str) -> Result<()> {
        let mut filter = self.current_filter.write().await;

        // Parse filter command
        let parts: Vec<&str> = filter_command.split(':').collect();
        if parts.len() != 2 {
            return Err(anyhow!("Invalid filter format. Use type:value"));
        }

        let filter_type = parts[0].trim();
        let filter_value = parts[1].trim();

        match filter_type {
            "orig_trunk" => filter.orig_trunk = Some(filter_value.to_string()),
            "term_trunk" => filter.term_trunk = Some(filter_value.to_string()),
            "ani" => filter.ani = Some(filter_value.to_string()),
            "dnis" => filter.dnis = Some(filter_value.to_string()),
            "ip" => filter.ip_address = Some(filter_value.parse()?),
            "code" => {
                let code: u16 = filter_value.parse()?;
                filter.response_codes.push(code);
            }
            "method" => filter.methods.push(filter_value.to_uppercase()),
            "call_id" => filter.call_id = Some(filter_value.to_string()),
            "user_agent" => filter.user_agent = Some(filter_value.to_string()),
            "regex" => filter.regex_filter = Some(Regex::new(filter_value)?),
            _ => return Err(anyhow!("Unknown filter type: {}", filter_type)),
        }

        info!("Applied filter: {} = {}", filter_type, filter_value);
        Ok(())
    }

    /// Clear all filters
    pub async fn clear_filters(&self) {
        let mut filter = self.current_filter.write().await;
        *filter = SipDebugFilter::default();
        info!("Cleared all filters");
    }
}

/// Utility functions for SIP debugging
pub mod utils {
    use super::*;

    /// Extract call information from SIP message
    pub fn extract_call_info(_message: &SipMessage) -> CallInfo {
        // TODO: Implement actual SIP header parsing
        // This is a placeholder implementation

        CallInfo {
            call_id: "unknown".to_string(),
            ani: None,
            dnis: None,
            user_agent: None,
            method: None,
            response_code: None,
            cseq: None,
        }
    }

    /// Create debug message from SIP message
    pub fn create_debug_message(
        sip_message: SipMessage,
        direction: MessageDirection,
        trunk_info: Option<TrunkInfo>,
    ) -> SipDebugMessage {
        let call_info = extract_call_info(&sip_message);

        SipDebugMessage {
            message: sip_message,
            direction,
            trunk_info,
            call_info: Some(call_info),
            timing: MessageTiming {
                received_at: SystemTime::now(),
                processed_at: None,
                response_time: None,
            },
            processing_result: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_filter() {
        // TODO: Add tests for message filtering
    }

    #[test]
    fn test_color_formatting() {
        // TODO: Add tests for color formatting
    }
}
