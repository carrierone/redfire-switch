/*
 * Redfire Switch - CNAM CLI Interface  
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
use console::{style, Term};
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressStyle, HumanDuration};
use std::time::Duration;

use crate::cli::CnamCommands;
use crate::cnam::CnamService;
use crate::config::Config;
use crate::lerg_nanpa::LergNanpaService;

/// Handle CNAM CLI commands
pub async fn handle_cnam_command(command: CnamCommands, config_path: &str) -> Result<()> {
    let config = Config::load_from_file(config_path)?;
    
    if !config.cnam.enabled {
        let term = Term::stdout();
        term.write_line(&format!("⚠️  {}", style("CNAM service is disabled").yellow()))?;
        term.write_line("   Enable CNAM in configuration to use these commands")?;
        return Ok(());
    }

    // Initialize LERG/NANPA service if needed for better country detection
    let lerg_nanpa_service = match LergNanpaService::new() {
        service => {
            // Note: LERG/NANPA data would need to be loaded separately via CLI commands
            // This provides the framework for using the data when available
            Some(std::sync::Arc::new(service))
        }
    };

    let service = CnamService::with_lerg_nanpa(config.cnam.clone(), lerg_nanpa_service)?;

    match command {
        CnamCommands::Status => {
            handle_status(&service).await
        }
        CnamCommands::Lookup { number, force } => {
            handle_lookup(&service, &number, force).await
        }
        CnamCommands::TestCountry { number } => {
            handle_test_country(&service, &number).await
        }
        CnamCommands::Countries => {
            handle_countries(&service).await
        }
        CnamCommands::EnableCountry { country_code } => {
            handle_enable_country(&country_code, config_path).await
        }
        CnamCommands::DisableCountry { country_code } => {
            handle_disable_country(&country_code, config_path).await
        }
        CnamCommands::CacheStats => {
            handle_cache_stats(&service).await
        }
        CnamCommands::ClearCache { yes } => {
            handle_clear_cache(&service, yes).await
        }
        CnamCommands::Stats => {
            handle_stats(&service).await
        }
    }
}

/// Handle CNAM status command
async fn handle_status(service: &CnamService) -> Result<()> {
    let term = Term::stdout();
    let stats = service.get_stats();

    term.write_line(&format!("📞 {}", style("CNAM Service Status").bold().cyan()))?;
    term.write_line("")?;

    // Service Configuration
    term.write_line(&format!("🔧 {}", style("Configuration").bold().yellow()))?;
    term.write_line(&format!("   Enabled Countries: {}", service.get_enabled_countries().join(", ")))?;
    term.write_line(&format!("   Cache Enabled: {}", if service.get_stats().cache_hits + service.get_stats().cache_misses > 0 { "Yes" } else { "No" }))?;
    
    // LERG/NANPA Integration Status
    let (has_integration, lerg_count, nanpa_count) = service.get_lerg_nanpa_status();
    term.write_line(&format!("   LERG/NANPA Integration: {}", if has_integration { style("Available").green() } else { style("Not Available").yellow() }))?;
    if has_integration {
        term.write_line(&format!("   LERG Entries: {}", lerg_count))?;
        term.write_line(&format!("   NANPA Entries: {}", nanpa_count))?;
        if lerg_count == 0 || nanpa_count == 0 {
            term.write_line(&format!("   ⚠️  {}", style("Load LERG/NANPA data for improved accuracy").yellow()))?;
        }
    } else {
        term.write_line(&format!("   💡 Use 'lerg-nanpa' commands to load data for improved country detection"))?;
    }
    term.write_line("")?;

    // Statistics Summary
    term.write_line(&format!("📊 {}", style("Statistics Summary").bold().yellow()))?;
    term.write_line(&format!("   Total Lookups: {}", stats.total_lookups))?;
    term.write_line(&format!("   Successful: {} ({:.1}%)", stats.successful_lookups, stats.success_rate))?;
    term.write_line(&format!("   Failed: {}", stats.failed_lookups))?;
    term.write_line(&format!("   Cache Hit Rate: {:.1}%", stats.cache_hit_rate))?;
    term.write_line(&format!("   Average Response: {:.1}ms", stats.avg_response_time_ms))?;
    term.write_line(&format!("   Total Cost: ${:.3}", stats.total_cost_cents / 100.0))?;

    Ok(())
}

/// Handle CNAM lookup test
async fn handle_lookup(service: &CnamService, number: &str, force: bool) -> Result<()> {
    let term = Term::stdout();
    term.write_line(&format!("🔍 {}", style("CNAM Lookup Test").bold().cyan()))?;
    term.write_line(&format!("   Number: {}", style(number).green()))?;
    term.write_line(&format!("   Force Fresh: {}", if force { "Yes" } else { "No" }))?;
    term.write_line("")?;

    // Test country detection first
    if let Some(country) = service.test_country_detection(number) {
        term.write_line(&format!("🌍 Country Detected: {}", style(&country).cyan()))?;
        if !service.is_country_enabled(&country) {
            term.write_line(&format!("⚠️  {}", style("Country not enabled for CNAM dipping").yellow()))?;
            return Ok(());
        }
    } else {
        term.write_line(&format!("❌ {}", style("Could not detect country for number").red()))?;
        return Ok(());
    }

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(ProgressStyle::default_spinner()
        .template("{spinner:.green} {msg}")
        .unwrap());
    spinner.set_message("Performing CNAM lookup...");
    spinner.enable_steady_tick(Duration::from_millis(100));

    let start_time = std::time::Instant::now();
    match service.lookup_cnam(number, "cli-test").await {
        Ok(result) => {
            spinner.finish_and_clear();
            
            let duration = start_time.elapsed();
            term.write_line(&format!("✅ {}", style("CNAM Lookup Completed").bold().green()))?;
            term.write_line("")?;
            
            // Results
            term.write_line(&format!("📋 {}", style("Results").bold().blue()))?;
            term.write_line(&format!("   Number: {}", result.number))?;
            if let Some(name) = &result.name {
                term.write_line(&format!("   Name: {}", style(name).green().bold()))?;
            } else {
                term.write_line(&format!("   Name: {}", style("Not found").dim()))?;
            }
            term.write_line(&format!("   Success: {}", if result.success { style("Yes").green() } else { style("No").red() }))?;
            term.write_line(&format!("   Provider: {}", result.provider))?;
            term.write_line(&format!("   Cost: ${:.3}", result.cost_cents / 100.0))?;
            term.write_line(&format!("   Response Time: {}ms", result.response_time_ms))?;
            term.write_line(&format!("   Cache Hit: {}", if result.cache_hit { "Yes" } else { "No" }))?;
            term.write_line(&format!("   Duration: {}", HumanDuration(duration)))?;

            if let Some(error) = &result.error {
                term.write_line(&format!("   Error: {}", style(error).red()))?;
            }
        }
        Err(e) => {
            spinner.finish_and_clear();
            term.write_line(&format!("❌ {}", style("CNAM Lookup Failed").bold().red()))?;
            term.write_line(&format!("   Error: {}", e))?;
        }
    }

    Ok(())
}

/// Handle country detection test
async fn handle_test_country(service: &CnamService, number: &str) -> Result<()> {
    let term = Term::stdout();
    term.write_line(&format!("🌍 {}", style("Country Detection Test").bold().cyan()))?;
    term.write_line(&format!("   Number: {}", style(number).green()))?;
    term.write_line("")?;

    if let Some(country) = service.test_country_detection(number) {
        term.write_line(&format!("✅ {}", style("Country Detected").bold().green()))?;
        term.write_line(&format!("   Country Code: {}", style(&country).cyan().bold()))?;
        term.write_line(&format!("   CNAM Enabled: {}", 
            if service.is_country_enabled(&country) { 
                style("Yes").green() 
            } else { 
                style("No").red() 
            }
        ))?;

        // Show country name
        let country_name = match country.as_str() {
            "US" => "United States",
            "CA" => "Canada", 
            "GB" => "United Kingdom",
            "DE" => "Germany",
            "FR" => "France",
            "IT" => "Italy",
            "ES" => "Spain",
            "JP" => "Japan",
            "CN" => "China",
            "AU" => "Australia",
            "BR" => "Brazil",
            "RU" => "Russia",
            _ => "Unknown",
        };
        term.write_line(&format!("   Country Name: {}", country_name))?;
        
        // Show data source
        let (has_integration, lerg_count, nanpa_count) = service.get_lerg_nanpa_status();
        if has_integration && nanpa_count > 0 {
            term.write_line(&format!("   Data Source: {}", style("NANPA Official Data").green()))?;
        } else {
            term.write_line(&format!("   Data Source: {}", style("Fallback Detection").yellow()))?;
            term.write_line(&format!("   💡 Load NANPA data for improved accuracy"))?;
        }
    } else {
        term.write_line(&format!("❌ {}", style("Could not detect country").bold().red()))?;
        term.write_line("   Possible reasons:")?;
        term.write_line("   - Invalid phone number format")?;
        term.write_line("   - Unsupported country code")?;
        term.write_line("   - Number too short or too long")?;
    }

    Ok(())
}

/// Handle show enabled countries
async fn handle_countries(service: &CnamService) -> Result<()> {
    let term = Term::stdout();
    term.write_line(&format!("🌍 {}", style("CNAM Enabled Countries").bold().cyan()))?;
    term.write_line("")?;

    let countries = service.get_enabled_countries();
    if countries.is_empty() {
        term.write_line(&format!("⚠️  {}", style("No countries enabled for CNAM dipping").yellow()))?;
        term.write_line("   Use 'cnam enable-country <CODE>' to add countries")?;
    } else {
        for country in countries {
            let country_name = match country.as_str() {
                "US" => "United States",
                "CA" => "Canada",
                "GB" => "United Kingdom", 
                "DE" => "Germany",
                "FR" => "France",
                "IT" => "Italy",
                "ES" => "Spain",
                "JP" => "Japan",
                "CN" => "China",
                "AU" => "Australia",
                "BR" => "Brazil",
                "RU" => "Russia",
                _ => "Unknown Country",
            };
            term.write_line(&format!("   {} - {}", style(country).green(), country_name))?;
        }
        
        term.write_line("")?;
        term.write_line(&format!("📝 {}", style("Available Commands").dim()))?;
        term.write_line("   cnam enable-country <CODE>  - Add a country")?;
        term.write_line("   cnam disable-country <CODE> - Remove a country")?;
        term.write_line("   Example: cnam enable-country CA")?;
    }

    Ok(())
}

/// Handle enable country (Note: This would need to modify config file)
async fn handle_enable_country(country_code: &str, _config_path: &str) -> Result<()> {
    let term = Term::stdout();
    term.write_line(&format!("⚠️  {}", style("Configuration Modification Required").yellow()))?;
    term.write_line(&format!("   To enable country '{}' for CNAM dipping:", country_code.to_uppercase()))?;
    term.write_line("")?;
    term.write_line("   1. Edit your configuration file")?;
    term.write_line("   2. Add the country code to cnam.enabled_countries array:")?;
    term.write_line("")?;
    term.write_line(&format!("      \"enabled_countries\": [\"US\", \"{}\"]", country_code.to_uppercase()))?;
    term.write_line("")?;
    term.write_line("   3. Restart the switch to apply changes")?;
    
    Ok(())
}

/// Handle disable country (Note: This would need to modify config file)  
async fn handle_disable_country(country_code: &str, _config_path: &str) -> Result<()> {
    let term = Term::stdout();
    term.write_line(&format!("⚠️  {}", style("Configuration Modification Required").yellow()))?;
    term.write_line(&format!("   To disable country '{}' for CNAM dipping:", country_code.to_uppercase()))?;
    term.write_line("")?;
    term.write_line("   1. Edit your configuration file")?;
    term.write_line("   2. Remove the country code from cnam.enabled_countries array")?;
    term.write_line("   3. Restart the switch to apply changes")?;
    
    Ok(())
}

/// Handle cache statistics
async fn handle_cache_stats(service: &CnamService) -> Result<()> {
    let term = Term::stdout();
    let stats = service.get_stats();

    term.write_line(&format!("🗄️  {}", style("CNAM Cache Statistics").bold().cyan()))?;
    term.write_line("")?;

    term.write_line(&format!("📊 {}", style("Cache Performance").bold().yellow()))?;
    term.write_line(&format!("   Total Cache Entries: {}", service.get_cache_size()))?;
    term.write_line(&format!("   Cache Hits: {}", stats.cache_hits))?;
    term.write_line(&format!("   Cache Misses: {}", stats.cache_misses))?;
    term.write_line(&format!("   Hit Rate: {:.1}%", stats.cache_hit_rate))?;
    term.write_line("")?;

    if stats.cache_hits + stats.cache_misses > 0 {
        term.write_line(&format!("💰 {}", style("Cost Savings").bold().yellow()))?;
        let estimated_savings = stats.cache_hits as f64 * 0.50; // Assuming 0.5 cents per lookup
        term.write_line(&format!("   Estimated Savings: ${:.3}", estimated_savings / 100.0))?;
        term.write_line(&format!("   From Cache Hits: {}", stats.cache_hits))?;
    }

    Ok(())
}

/// Handle clear cache
async fn handle_clear_cache(service: &CnamService, confirmed: bool) -> Result<()> {
    let term = Term::stdout();

    let should_proceed = if confirmed {
        true
    } else {
        Confirm::new()
            .with_prompt("Are you sure you want to clear the CNAM cache?")
            .interact()?
    };

    if should_proceed {
        let cache_size = service.get_cache_size();
        service.clear_cache();
        
        term.write_line(&format!("✅ {}", style("CNAM Cache Cleared").bold().green()))?;
        term.write_line(&format!("   Cleared {} entries", cache_size))?;
    } else {
        term.write_line(&format!("❌ {}", style("Operation cancelled").yellow()))?;
    }

    Ok(())
}

/// Handle service statistics
async fn handle_stats(service: &CnamService) -> Result<()> {
    let term = Term::stdout();
    let stats = service.get_stats();

    term.write_line(&format!("📊 {}", style("CNAM Service Statistics").bold().cyan()))?;
    term.write_line("")?;

    // Lookup Statistics
    term.write_line(&format!("🔍 {}", style("Lookup Statistics").bold().yellow()))?;
    term.write_line(&format!("   Total Lookups: {}", stats.total_lookups))?;
    term.write_line(&format!("   Successful: {} ({:.1}%)", stats.successful_lookups, stats.success_rate))?;
    term.write_line(&format!("   Failed: {} ({:.1}%)", stats.failed_lookups, 100.0 - stats.success_rate))?;
    term.write_line("")?;

    // Performance Statistics
    term.write_line(&format!("⚡ {}", style("Performance").bold().yellow()))?;
    term.write_line(&format!("   Average Response Time: {:.1}ms", stats.avg_response_time_ms))?;
    term.write_line(&format!("   Cache Hit Rate: {:.1}%", stats.cache_hit_rate))?;
    term.write_line(&format!("   Active Lookups: {}", service.get_active_lookups_count()))?;
    term.write_line("")?;

    // Cost Statistics
    term.write_line(&format!("💰 {}", style("Cost Analysis").bold().yellow()))?;
    term.write_line(&format!("   Total Cost: ${:.3}", stats.total_cost_cents / 100.0))?;
    term.write_line(&format!("   Average Cost per Lookup: ${:.4}", if stats.total_lookups > 0 { stats.total_cost_cents / stats.total_lookups as f64 / 100.0 } else { 0.0 }))?;
    term.write_line(&format!("   Cache Savings: ${:.3}", stats.cache_hits as f64 * 0.50 / 100.0))?;

    Ok(())
}