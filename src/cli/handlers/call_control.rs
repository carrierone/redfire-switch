/*
 * Redfire Switch - Call Control CLI Commands
 * Copyright (C) 2025 Carrier One Inc and contributors
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::{Args, Subcommand};
use std::collections::HashMap;

use crate::call_control::{CallControlService, TrunkDirection, TrunkGroupLimits};
use crate::config::Config;

/// Call control CLI commands
#[derive(Debug, Subcommand)]
pub enum CallControlCommands {
    /// Show call control status
    Status,
    /// Add DNO ANI block
    AddDnoAni {
        /// ANI to block
        ani: String,
        /// Trunk ID
        trunk_id: String,
        /// Reason for blocking
        reason: String,
    },
    /// Remove DNO ANI block
    RemoveDnoAni {
        /// ANI to unblock
        ani: String,
        /// Trunk ID
        trunk_id: String,
    },
    /// Add STI OCN block
    AddStiOcn {
        /// OCN to block
        ocn: String,
        /// Reason for blocking
        reason: String,
    },
    /// Remove STI OCN block
    RemoveStiOcn {
        /// OCN to unblock
        ocn: String,
    },
    /// Show active calls
    ActiveCalls,
    /// Show call control statistics
    Stats,
}

// Simplified CLI structure - complex argument structures removed for compilation

/// Handle call control commands
pub async fn handle_call_control_command(
    command: CallControlCommands,
    config_path: &str,
) -> Result<()> {
    let config = Config::load_from_file(config_path)?;
    let service = CallControlService::new(config.call_control).await?;
    
    match command {
        CallControlCommands::Status => {
            show_call_control_status(&service).await?;
        }
        CallControlCommands::AddDnoAni { ani, trunk_id, reason } => {
            service.add_dno_ani_block(ani.clone(), trunk_id.clone(), reason, None).await?;
            println!("✓ Added DNO ANI block: {} on trunk {}", ani, trunk_id);
        }
        CallControlCommands::RemoveDnoAni { ani, trunk_id } => {
            service.remove_dno_ani_block(&ani, &trunk_id).await?;
            println!("✓ Removed DNO ANI block: {} on trunk {}", ani, trunk_id);
        }
        CallControlCommands::AddStiOcn { ocn, reason } => {
            service.add_sti_ocn_block(ocn.clone(), reason, None).await?;
            println!("✓ Added STI OCN block: {}", ocn);
        }
        CallControlCommands::RemoveStiOcn { ocn } => {
            service.remove_sti_ocn_block(&ocn).await?;
            println!("✓ Removed STI OCN block: {}", ocn);
        }
        CallControlCommands::ActiveCalls => {
            show_active_calls(&service).await?;
        }
        CallControlCommands::Stats => {
            show_call_control_stats(&service).await?;
        }
    }
    
    Ok(())
}

/// Show call control status
async fn show_call_control_status(service: &CallControlService) -> Result<()> {
    let stats = service.get_statistics().await;
    
    println!("Call Control Status");
    println!("==================");
    println!("Active Calls: {}", stats.active_calls);
    println!("Total Ingress Calls: {}", stats.total_ingress_calls);
    println!("Total Egress Calls: {}", stats.total_egress_calls);
    println!("Total Blocks: {}", stats.total_blocks);
    println!("Total Timeouts: {}", stats.total_timeouts);
    println!("DNO ANI Blocks Cached: {}", stats.dno_ani_blocks_cached);
    println!("STI OCN Blocks Cached: {}", stats.sti_ocn_blocks_cached);
    
    Ok(())
}

/// Show active calls
async fn show_active_calls(service: &CallControlService) -> Result<()> {
    let stats = service.get_statistics().await;
    
    println!("Active Calls: {}", stats.active_calls);
    println!("=============");
    
    // This would show detailed active call information
    println!("(Implementation would show active call details)");
    
    Ok(())
}

/// Show call control statistics
async fn show_call_control_stats(service: &CallControlService) -> Result<()> {
    let stats = service.get_statistics().await;
    
    println!("Call Control Statistics");
    println!("======================");
    println!("Active Calls: {}", stats.active_calls);
    println!("Total Ingress Calls: {}", stats.total_ingress_calls);
    println!("Total Egress Calls: {}", stats.total_egress_calls);
    println!("Total Blocks: {}", stats.total_blocks);
    println!("Total Timeouts: {}", stats.total_timeouts);
    println!("DNO ANI Blocks Cached: {}", stats.dno_ani_blocks_cached);
    println!("STI OCN Blocks Cached: {}", stats.sti_ocn_blocks_cached);
    
    Ok(())
}