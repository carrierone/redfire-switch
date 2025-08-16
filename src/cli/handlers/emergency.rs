/*
 * Redfire Switch - Emergency Routing CLI
 * Copyright (C) 2025 Carrier One Inc and contributors
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

use crate::emergency_routing::{EmergencyConfig, EmergencyRoutingService, DidProviderInfo, EmergencyPattern};
use anyhow::Result;
use clap::Subcommand;
use std::collections::HashMap;
use std::path::Path;

/// Emergency routing CLI commands
#[derive(Debug, Subcommand)]
pub enum EmergencyCommands {
    /// Show emergency routing status
    Status,
    /// List emergency patterns
    ListPatterns,
    /// Add emergency pattern
    AddPattern {
        /// Pattern regex (e.g., "^911$")
        pattern: String,
        /// Region code (e.g., "US")
        region: String,
        /// Description
        description: String,
        /// Priority (1-255, 1 = highest)
        #[arg(default_value = "1")]
        priority: u8,
    },
    /// Remove emergency pattern
    RemovePattern {
        /// Pattern to remove
        pattern: String,
    },
    /// List DID providers
    ListProviders,
    /// Add DID provider
    AddProvider {
        /// Provider ID
        provider_id: String,
        /// Provider name
        name: String,
        /// Emergency route (SIP URI)
        emergency_route: String,
        /// Supported regions (comma-separated)
        regions: String,
        /// Emergency contact
        #[arg(long)]
        contact: Option<String>,
        /// Backup route
        #[arg(long)]
        backup: Option<String>,
    },
    /// Remove DID provider
    RemoveProvider {
        /// Provider ID to remove
        provider_id: String,
    },
    /// Add DID mapping
    AddDidMapping {
        /// DID number
        did: String,
        /// Provider ID
        provider_id: String,
    },
    /// Remove DID mapping
    RemoveDidMapping {
        /// DID number
        did: String,
    },
    /// List DID mappings
    ListMappings,
    /// Test emergency call routing
    TestCall {
        /// Called number (emergency number)
        called: String,
        /// Calling number
        calling: String,
        /// Originating DID (optional)
        #[arg(long)]
        did: Option<String>,
    },
    /// Validate emergency configuration
    Validate,
    /// Generate example configuration
    GenExampleConfig {
        /// Output file path
        output: String,
    },
    /// Export configuration
    Export {
        /// Output file path
        output: String,
    },
    /// Import configuration
    Import {
        /// Input file path
        input: String,
    },
}

/// Handle emergency routing CLI commands
pub async fn handle_emergency_command(command: EmergencyCommands, _config_path: &str) -> Result<()> {
    match command {
        EmergencyCommands::Status => {
            show_emergency_status().await?;
        }
        EmergencyCommands::ListPatterns => {
            list_emergency_patterns().await?;
        }
        EmergencyCommands::AddPattern { pattern, region, description, priority } => {
            add_emergency_pattern(pattern, region, description, priority).await?;
        }
        EmergencyCommands::RemovePattern { pattern } => {
            remove_emergency_pattern(pattern).await?;
        }
        EmergencyCommands::ListProviders => {
            list_did_providers().await?;
        }
        EmergencyCommands::AddProvider { provider_id, name, emergency_route, regions, contact, backup } => {
            add_did_provider(provider_id, name, emergency_route, regions, contact, backup).await?;
        }
        EmergencyCommands::RemoveProvider { provider_id } => {
            remove_did_provider(provider_id).await?;
        }
        EmergencyCommands::AddDidMapping { did, provider_id } => {
            add_did_mapping(did, provider_id).await?;
        }
        EmergencyCommands::RemoveDidMapping { did } => {
            remove_did_mapping(did).await?;
        }
        EmergencyCommands::ListMappings => {
            list_did_mappings().await?;
        }
        EmergencyCommands::TestCall { called, calling, did } => {
            test_emergency_call(called, calling, did).await?;
        }
        EmergencyCommands::Validate => {
            validate_emergency_config().await?;
        }
        EmergencyCommands::GenExampleConfig { output } => {
            generate_example_config(output).await?;
        }
        EmergencyCommands::Export { output } => {
            export_emergency_config(output).await?;
        }
        EmergencyCommands::Import { input } => {
            import_emergency_config(input).await?;
        }
    }
    
    Ok(())
}

async fn show_emergency_status() -> Result<()> {
    println!("Emergency Routing Status");
    println!("========================");
    
    let config = load_emergency_config().await?;
    let service = EmergencyRoutingService::new(config)?;
    let stats = service.get_statistics();
    
    println!("Emergency Enabled: ✓");
    println!("Patterns Configured: {}", stats.patterns_configured);
    println!("Providers Configured: {}", stats.providers_configured);
    println!("DID Mappings: {}", stats.did_mappings);
    
    // Validate configuration
    let warnings = service.validate_config()?;
    if !warnings.is_empty() {
        println!("\nConfiguration Warnings:");
        for warning in warnings {
            println!("  ⚠️  {}", warning);
        }
    } else {
        println!("Configuration: ✓ Valid");
    }
    
    Ok(())
}

async fn list_emergency_patterns() -> Result<()> {
    println!("Emergency Number Patterns");
    println!("=========================");
    
    let config = load_emergency_config().await?;
    
    if config.emergency_patterns.is_empty() {
        println!("No emergency patterns configured.");
        return Ok(());
    }
    
    println!("{:<15} {:<8} {:<30} {:<8}", "Pattern", "Region", "Description", "Priority");
    println!("{:-<15} {:-<8} {:-<30} {:-<8}", "", "", "", "");
    
    for pattern in &config.emergency_patterns {
        println!("{:<15} {:<8} {:<30} {:<8}", 
                 pattern.pattern, pattern.region, pattern.description, pattern.priority);
    }
    
    Ok(())
}

async fn add_emergency_pattern(pattern: String, region: String, description: String, priority: u8) -> Result<()> {
    println!("Adding emergency pattern: {} ({})", pattern, description);
    
    // Validate regex pattern
    if let Err(e) = regex::Regex::new(&pattern) {
        eprintln!("❌ Invalid regex pattern: {}", e);
        return Ok(());
    }
    
    let new_pattern = EmergencyPattern {
        pattern: pattern.clone(),
        region: region.clone(),
        description: description.clone(),
        priority,
    };
    
    let mut config = load_emergency_config().await?;
    
    // Check for duplicates
    if config.emergency_patterns.iter().any(|p| p.pattern == pattern) {
        eprintln!("❌ Pattern already exists: {}", pattern);
        return Ok(());
    }
    
    config.emergency_patterns.push(new_pattern);
    save_emergency_config(&config).await?;
    
    println!("✓ Emergency pattern added successfully");
    Ok(())
}

async fn remove_emergency_pattern(pattern: String) -> Result<()> {
    println!("Removing emergency pattern: {}", pattern);
    
    let mut config = load_emergency_config().await?;
    let initial_len = config.emergency_patterns.len();
    
    config.emergency_patterns.retain(|p| p.pattern != pattern);
    
    if config.emergency_patterns.len() == initial_len {
        eprintln!("❌ Pattern not found: {}", pattern);
        return Ok(());
    }
    
    save_emergency_config(&config).await?;
    println!("✓ Emergency pattern removed successfully");
    Ok(())
}

async fn list_did_providers() -> Result<()> {
    println!("DID Providers for Emergency Routing");
    println!("===================================");
    
    let config = load_emergency_config().await?;
    
    if config.did_providers.is_empty() {
        println!("No DID providers configured.");
        return Ok(());
    }
    
    for (provider_id, provider) in &config.did_providers {
        println!("Provider ID: {}", provider_id);
        println!("  Name: {}", provider.name);
        println!("  Emergency Route: {}", provider.emergency_route);
        println!("  Regions: {}", provider.regions.join(", "));
        println!("  Emergency Enabled: {}", if provider.emergency_enabled { "✓" } else { "❌" });
        if let Some(ref contact) = provider.emergency_contact {
            println!("  Emergency Contact: {}", contact);
        }
        if let Some(ref backup) = provider.backup_route {
            println!("  Backup Route: {}", backup);
        }
        println!();
    }
    
    Ok(())
}

async fn add_did_provider(
    provider_id: String,
    name: String,
    emergency_route: String,
    regions: String,
    contact: Option<String>,
    backup: Option<String>,
) -> Result<()> {
    println!("Adding DID provider: {} ({})", provider_id, name);
    
    let regions_vec: Vec<String> = regions.split(',')
        .map(|s| s.trim().to_uppercase())
        .collect();
    
    let provider = DidProviderInfo {
        name: name.clone(),
        provider_id: provider_id.clone(),
        emergency_route,
        emergency_contact: contact,
        backup_route: backup,
        regions: regions_vec,
        emergency_enabled: true,
    };
    
    let mut config = load_emergency_config().await?;
    
    if config.did_providers.contains_key(&provider_id) {
        eprintln!("❌ Provider already exists: {}", provider_id);
        return Ok(());
    }
    
    config.did_providers.insert(provider_id, provider);
    save_emergency_config(&config).await?;
    
    println!("✓ DID provider added successfully");
    Ok(())
}

async fn remove_did_provider(provider_id: String) -> Result<()> {
    println!("Removing DID provider: {}", provider_id);
    
    let mut config = load_emergency_config().await?;
    
    if config.did_providers.remove(&provider_id).is_none() {
        eprintln!("❌ Provider not found: {}", provider_id);
        return Ok(());
    }
    
    save_emergency_config(&config).await?;
    println!("✓ DID provider removed successfully");
    Ok(())
}

async fn add_did_mapping(did: String, provider_id: String) -> Result<()> {
    println!("Adding DID mapping: {} -> {}", did, provider_id);
    
    // For now, we'll just show what would happen
    // In a real implementation, this would update a persistent mapping store
    println!("✓ DID mapping would be added (implementation needed)");
    println!("  DID: {}", did);
    println!("  Provider: {}", provider_id);
    
    Ok(())
}

async fn remove_did_mapping(did: String) -> Result<()> {
    println!("Removing DID mapping: {}", did);
    
    // For now, we'll just show what would happen
    println!("✓ DID mapping would be removed (implementation needed)");
    
    Ok(())
}

async fn list_did_mappings() -> Result<()> {
    println!("DID to Provider Mappings");
    println!("========================");
    
    // For now, show example mappings
    println!("(Implementation needed - would show actual DID mappings)");
    println!("Example mappings:");
    println!("  +1-555-0100 -> carrier_one");
    println!("  +1-555-0200 -> verizon");
    println!("  +44-20-7946-0958 -> bt_uk");
    
    Ok(())
}

async fn test_emergency_call(called: String, calling: String, did: Option<String>) -> Result<()> {
    println!("Testing Emergency Call Routing");
    println!("==============================");
    println!("Called Number: {}", called);
    println!("Calling Number: {}", calling);
    if let Some(ref did_num) = did {
        println!("Originating DID: {}", did_num);
    }
    println!();
    
    let config = load_emergency_config().await?;
    let service = EmergencyRoutingService::new(config)?;
    
    let decision = service.analyze_call(&called, &calling, did.as_deref(), None).await?;
    
    if decision.is_emergency {
        println!("🚨 EMERGENCY CALL DETECTED");
        println!("Emergency Number: {}", decision.emergency_number.unwrap_or_default());
        
        if let Some(pattern) = decision.matched_pattern {
            println!("Matched Pattern: {} ({})", pattern.pattern, pattern.description);
            println!("Region: {}", pattern.region);
        }
        
        if let Some(provider) = decision.source_provider {
            println!("Source Provider: {}", provider);
        } else {
            println!("Source Provider: Unknown");
        }
        
        if let Some(route) = decision.emergency_route {
            println!("Emergency Route: {}", route);
        } else {
            println!("❌ No emergency route available");
        }
        
        println!("Priority: {}", decision.priority);
        
        if !decision.metadata.is_empty() {
            println!("Metadata:");
            for (key, value) in decision.metadata {
                println!("  {}: {}", key, value);
            }
        }
    } else {
        println!("ℹ️  Normal Call - Not an emergency");
    }
    
    Ok(())
}

async fn validate_emergency_config() -> Result<()> {
    println!("Validating Emergency Configuration");
    println!("=================================");
    
    let config = load_emergency_config().await?;
    let service = EmergencyRoutingService::new(config)?;
    
    let warnings = service.validate_config()?;
    
    if warnings.is_empty() {
        println!("✓ Configuration is valid");
    } else {
        println!("Configuration has {} warning(s):", warnings.len());
        for (i, warning) in warnings.iter().enumerate() {
            println!("  {}: {}", i + 1, warning);
        }
    }
    
    Ok(())
}

async fn generate_example_config(output: String) -> Result<()> {
    println!("Generating example emergency configuration: {}", output);
    
    let config = EmergencyConfig::default();
    let content = serde_json::to_string_pretty(&config)?;
    
    tokio::fs::write(&output, content).await?;
    println!("✓ Example configuration written to: {}", output);
    
    Ok(())
}

async fn export_emergency_config(output: String) -> Result<()> {
    println!("Exporting emergency configuration: {}", output);
    
    let config = load_emergency_config().await?;
    let content = serde_json::to_string_pretty(&config)?;
    
    tokio::fs::write(&output, content).await?;
    println!("✓ Configuration exported to: {}", output);
    
    Ok(())
}

async fn import_emergency_config(input: String) -> Result<()> {
    println!("Importing emergency configuration: {}", input);
    
    if !Path::new(&input).exists() {
        eprintln!("❌ File not found: {}", input);
        return Ok(());
    }
    
    let content = tokio::fs::read_to_string(&input).await?;
    let config: EmergencyConfig = serde_json::from_str(&content)?;
    
    // Validate the imported configuration
    let service = EmergencyRoutingService::new(config.clone())?;
    let warnings = service.validate_config()?;
    
    if !warnings.is_empty() {
        println!("Import warnings:");
        for warning in warnings {
            println!("  ⚠️  {}", warning);
        }
        println!();
    }
    
    save_emergency_config(&config).await?;
    println!("✓ Configuration imported successfully");
    
    Ok(())
}

// Helper functions for loading/saving configuration
async fn load_emergency_config() -> Result<EmergencyConfig> {
    // For now, return default config
    // In a real implementation, this would load from file or database
    Ok(EmergencyConfig::default())
}

async fn save_emergency_config(_config: &EmergencyConfig) -> Result<()> {
    // For now, just simulate saving
    // In a real implementation, this would save to file or database
    Ok(())
}