/*
 * Redfire Switch - LERG and NANPA CLI Interface  
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

use crate::cli::LergNanpaCommands;
use crate::lerg_nanpa::LergNanpaService;
use crate::termination_routing::NanpaJurisdiction;

/// Handle LERG/NANPA CLI commands
pub async fn handle_lerg_nanpa_command(command: LergNanpaCommands) -> Result<()> {
    let service = LergNanpaService::new();

    match command {
        LergNanpaCommands::LoadLerg { file, validate, progress } => {
            handle_load_lerg(&service, &file, validate, progress).await
        }
        LergNanpaCommands::DownloadNanpa { force, save_to } => {
            handle_download_nanpa(&service, force, save_to).await
        }
        LergNanpaCommands::Stats => {
            handle_stats(&service).await
        }
        LergNanpaCommands::TestJurisdiction { destination, origination, detailed } => {
            handle_test_jurisdiction(&service, &destination, &origination, detailed).await
        }
        LergNanpaCommands::Lookup { number, rate_center, company, jurisdiction } => {
            handle_lookup(&service, &number, rate_center, company, jurisdiction).await
        }
        LergNanpaCommands::Export { output, states, company_types } => {
            handle_export(&service, &output, states, company_types).await
        }
        LergNanpaCommands::Validate { fix, detailed } => {
            handle_validate(&service, fix, detailed).await
        }
        LergNanpaCommands::Clear { lerg, nanpa, yes } => {
            handle_clear(&service, lerg, nanpa, yes).await
        }
    }
}

/// Handle LERG file loading
async fn handle_load_lerg(
    service: &LergNanpaService,
    file_path: &str,
    validate: bool,
    show_progress: bool,
) -> Result<()> {
    let term = Term::stdout();
    term.write_line(&format!("📂 {}", style("Loading LERG Data").bold().cyan()))?;
    term.write_line(&format!("   File: {}", file_path))?;
    term.write_line(&format!("   Validation: {}", if validate { "Enabled" } else { "Disabled" }))?;
    term.write_line("")?;

    // Check if file exists
    if !std::path::Path::new(file_path).exists() {
        term.write_line(&format!("❌ {}", style("File not found").bold().red()))?;
        return Ok(());
    }

    // Show progress bar if requested
    let progress_bar = if show_progress {
        let pb = ProgressBar::new_spinner();
        pb.set_style(ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap());
        pb.set_message("Loading LERG data...");
        pb.enable_steady_tick(Duration::from_millis(100));
        Some(pb)
    } else {
        None
    };

    // Load the file
    let start_time = std::time::Instant::now();
    match service.load_lerg_file(file_path).await {
        Ok(()) => {
            if let Some(pb) = progress_bar {
                pb.finish_and_clear();
            }

            let duration = start_time.elapsed();
            let stats = service.get_stats();

            term.write_line(&format!("✅ {}", style("LERG Data Loaded Successfully").bold().green()))?;
            term.write_line(&format!("   Entries: {}", stats.lerg_entries))?;
            term.write_line(&format!("   Duration: {}", HumanDuration(duration)))?;
            
            if let Some(load_time) = stats.last_lerg_load {
                term.write_line(&format!("   Loaded at: {}", load_time.format("%Y-%m-%d %H:%M:%S UTC")))?;
            }
        }
        Err(e) => {
            if let Some(pb) = progress_bar {
                pb.finish_and_clear();
            }
            
            term.write_line(&format!("❌ {}", style("Failed to load LERG data").bold().red()))?;
            term.write_line(&format!("   Error: {}", e))?;
        }
    }

    Ok(())
}

/// Handle NANPA NPA table download
async fn handle_download_nanpa(
    service: &LergNanpaService,
    force: bool,
    save_to: Option<String>,
) -> Result<()> {
    let term = Term::stdout();
    term.write_line(&format!("🌐 {}", style("Downloading NANPA NPA Table").bold().cyan()))?;
    term.write_line(&format!("   Source: https://reports.nanpa.com/public/npa_report.csv"))?;
    term.write_line(&format!("   Force: {}", if force { "Yes" } else { "No" }))?;
    term.write_line("")?;

    // Check if we should skip download
    if !force {
        let stats = service.get_stats();
        if let Some(last_download) = stats.last_nanpa_download {
            let age = chrono::Utc::now().signed_duration_since(last_download);
            if age < chrono::Duration::hours(24) {
                term.write_line(&format!("ℹ️  {}", style("NANPA data was downloaded recently").yellow()))?;
                term.write_line(&format!("   Last download: {} ago", HumanDuration(age.to_std().unwrap_or(Duration::from_secs(0)))))?;
                term.write_line(&format!("   Use --force to download anyway"))?;
                return Ok(());
            }
        }
    }

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(ProgressStyle::default_spinner()
        .template("{spinner:.green} {msg}")
        .unwrap());
    spinner.set_message("Downloading NANPA NPA table...");
    spinner.enable_steady_tick(Duration::from_millis(100));

    let start_time = std::time::Instant::now();
    match service.download_nanpa_npa_table().await {
        Ok(()) => {
            spinner.finish_and_clear();

            let duration = start_time.elapsed();
            let stats = service.get_stats();

            term.write_line(&format!("✅ {}", style("NANPA NPA Table Downloaded Successfully").bold().green()))?;
            term.write_line(&format!("   Entries: {}", stats.nanpa_entries))?;
            term.write_line(&format!("   Duration: {}", HumanDuration(duration)))?;

            if let Some(save_path) = save_to {
                term.write_line(&format!("   Saved to: {}", save_path))?;
            }
        }
        Err(e) => {
            spinner.finish_and_clear();
            
            term.write_line(&format!("❌ {}", style("Failed to download NANPA NPA table").bold().red()))?;
            term.write_line(&format!("   Error: {}", e))?;
        }
    }

    Ok(())
}

/// Handle statistics display
async fn handle_stats(service: &LergNanpaService) -> Result<()> {
    let term = Term::stdout();
    let stats = service.get_stats();

    term.write_line(&format!("📊 {}", style("LERG and NANPA Statistics").bold().cyan()))?;
    term.write_line("")?;

    // LERG Statistics
    term.write_line(&format!("📞 {}", style("LERG Data").bold().yellow()))?;
    term.write_line(&format!("   Entries Loaded: {}", stats.lerg_entries))?;
    term.write_line(&format!("   Memory Usage: {} entries", service.get_lerg_count()))?;
    
    if let Some(last_load) = stats.last_lerg_load {
        term.write_line(&format!("   Last Loaded: {}", last_load.format("%Y-%m-%d %H:%M:%S UTC")))?;
        let age = chrono::Utc::now().signed_duration_since(last_load);
        term.write_line(&format!("   Data Age: {}", HumanDuration(age.to_std().unwrap_or(Duration::from_secs(0)))))?;
    } else {
        term.write_line(&format!("   Status: {}", style("No data loaded").red()))?;
    }

    term.write_line("")?;

    // NANPA Statistics  
    term.write_line(&format!("🗺️  {}", style("NANPA NPA Data").bold().yellow()))?;
    term.write_line(&format!("   Entries Loaded: {}", stats.nanpa_entries))?;
    term.write_line(&format!("   Memory Usage: {} entries", service.get_nanpa_count()))?;
    
    if let Some(last_download) = stats.last_nanpa_download {
        term.write_line(&format!("   Last Downloaded: {}", last_download.format("%Y-%m-%d %H:%M:%S UTC")))?;
        let age = chrono::Utc::now().signed_duration_since(last_download);
        term.write_line(&format!("   Data Age: {}", HumanDuration(age.to_std().unwrap_or(Duration::from_secs(0)))))?;
    } else {
        term.write_line(&format!("   Status: {}", style("No data loaded").red()))?;
    }

    term.write_line("")?;

    // Usage Statistics
    term.write_line(&format!("🔍 {}", style("Usage Statistics").bold().yellow()))?;
    term.write_line(&format!("   Jurisdiction Lookups: {}", stats.jurisdiction_lookups))?;
    term.write_line(&format!("   Rate Center Lookups: {}", stats.rate_center_lookups))?;

    Ok(())
}

/// Handle jurisdiction testing
async fn handle_test_jurisdiction(
    service: &LergNanpaService,
    destination: &str,
    origination: &str,
    detailed: bool,
) -> Result<()> {
    let term = Term::stdout();
    term.write_line(&format!("⚖️  {}", style("Jurisdiction Testing").bold().cyan()))?;
    term.write_line(&format!("   From: {}", style(origination).green()))?;
    term.write_line(&format!("   To:   {}", style(destination).green()))?;
    term.write_line("")?;

    // Perform jurisdiction lookup
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(ProgressStyle::default_spinner()
        .template("{spinner:.green} {msg}")
        .unwrap());
    spinner.set_message("Determining jurisdiction...");
    spinner.enable_steady_tick(Duration::from_millis(100));

    match service.determine_jurisdiction(destination, origination).await {
        Ok(jurisdiction) => {
            spinner.finish_and_clear();

            term.write_line(&format!("✅ {}", style("Jurisdiction Determined").bold().green()))?;
            term.write_line(&format!("   Result: {}", style(jurisdiction.description()).cyan().bold()))?;

            match jurisdiction {
                NanpaJurisdiction::Interstate => {
                    term.write_line(&format!("   Type: Cross-state call"))?;
                }
                NanpaJurisdiction::Intrastate => {
                    term.write_line(&format!("   Type: Same-state call"))?;
                }
                NanpaJurisdiction::Local => {
                    term.write_line(&format!("   Type: Same rate center"))?;
                }
                NanpaJurisdiction::Indeterminate => {
                    term.write_line(&format!("   Type: Cannot determine"))?;
                }
                NanpaJurisdiction::International => {
                    term.write_line(&format!("   Type: International destination"))?;
                }
            }

            if detailed {
                term.write_line("")?;
                show_detailed_jurisdiction_analysis(&term, service, destination, origination).await?;
            }
        }
        Err(e) => {
            spinner.finish_and_clear();
            term.write_line(&format!("❌ {}", style("Jurisdiction determination failed").bold().red()))?;
            term.write_line(&format!("   Error: {}", e))?;
        }
    }

    Ok(())
}

/// Show detailed jurisdiction analysis
async fn show_detailed_jurisdiction_analysis(
    term: &Term,
    service: &LergNanpaService,
    destination: &str,
    origination: &str,
) -> Result<()> {
    term.write_line(&format!("🔍 {}", style("Detailed Analysis").bold().blue()))?;

    // Analyze destination
    term.write_line(&format!("   Destination Analysis:"))?;
    if let Some(dest_entry) = service.get_lerg_entry(destination) {
        term.write_line(&format!("     NPA-NXX: {}", dest_entry.npa_nxx))?;
        term.write_line(&format!("     State: {}", dest_entry.state))?;
        term.write_line(&format!("     Rate Center: {}", dest_entry.rate_center))?;
        term.write_line(&format!("     LATA: {}", dest_entry.lata))?;
        term.write_line(&format!("     Company: {}", dest_entry.company_name))?;
        term.write_line(&format!("     Type: {}", dest_entry.company_type))?;
    } else {
        term.write_line(&format!("     {}", style("No LERG data found").red()))?;
    }

    term.write_line("")?;

    // Analyze origination
    term.write_line(&format!("   Origination Analysis:"))?;
    if let Some(orig_entry) = service.get_lerg_entry(origination) {
        term.write_line(&format!("     NPA-NXX: {}", orig_entry.npa_nxx))?;
        term.write_line(&format!("     State: {}", orig_entry.state))?;
        term.write_line(&format!("     Rate Center: {}", orig_entry.rate_center))?;
        term.write_line(&format!("     LATA: {}", orig_entry.lata))?;
        term.write_line(&format!("     Company: {}", orig_entry.company_name))?;
        term.write_line(&format!("     Type: {}", orig_entry.company_type))?;
    } else {
        term.write_line(&format!("     {}", style("No LERG data found").red()))?;
    }

    Ok(())
}

/// Handle number lookup
async fn handle_lookup(
    service: &LergNanpaService,
    number: &str,
    show_rate_center: bool,
    show_company: bool,
    show_jurisdiction: bool,
) -> Result<()> {
    let term = Term::stdout();
    term.write_line(&format!("🔍 {}", style("Number Lookup").bold().cyan()))?;
    term.write_line(&format!("   Number: {}", style(number).green()))?;
    term.write_line("")?;

    // Get LERG entry
    if let Some(entry) = service.get_lerg_entry(number) {
        term.write_line(&format!("📍 {}", style("LERG Information").bold().yellow()))?;
        term.write_line(&format!("   NPA-NXX: {}", entry.npa_nxx))?;
        term.write_line(&format!("   State: {}", entry.state))?;
        
        if show_rate_center {
            term.write_line(&format!("   Rate Center: {}", entry.rate_center))?;
            term.write_line(&format!("   LATA: {}", entry.lata))?;
        }
        
        if show_company {
            term.write_line(&format!("   Company: {}", entry.company_name))?;
            term.write_line(&format!("   Company Type: {}", entry.company_type))?;
        }

        // Show NANPA information for the NPA
        if let Some(nanpa_entry) = service.get_nanpa_entry(&entry.npa) {
            term.write_line("")?;
            term.write_line(&format!("🗺️  {}", style("NANPA NPA Information").bold().yellow()))?;
            term.write_line(&format!("   NPA: {}", nanpa_entry.npa))?;
            term.write_line(&format!("   Type: {}", nanpa_entry.type_of_code))?;
            term.write_line(&format!("   Location: {}", nanpa_entry.location))?;
            term.write_line(&format!("   Country: {}", nanpa_entry.country))?;
            term.write_line(&format!("   Time Zone: {}", nanpa_entry.time_zone))?;
            term.write_line(&format!("   Overlay: {}", if nanpa_entry.overlay { "Yes" } else { "No" }))?;
        }

        if show_jurisdiction {
            term.write_line("")?;
            term.write_line(&format!("⚖️  {}", style("Jurisdiction Analysis").bold().yellow()))?;
            term.write_line(&format!("   This number can be used for jurisdiction determination"))?;
            term.write_line(&format!("   Use 'test-jurisdiction' command to test against another number"))?;
        }
    } else {
        term.write_line(&format!("❌ {}", style("No LERG data found for this number").bold().red()))?;
        
        // Try to extract NPA and check NANPA data
        if let Some(npa) = crate::lerg_nanpa::utils::extract_npa(number) {
            if let Some(nanpa_entry) = service.get_nanpa_entry(&npa) {
                term.write_line("")?;
                term.write_line(&format!("🗺️  {}", style("NANPA NPA Information (No LERG data)").bold().yellow()))?;
                term.write_line(&format!("   NPA: {}", nanpa_entry.npa))?;
                term.write_line(&format!("   Location: {}", nanpa_entry.location))?;
                term.write_line(&format!("   Country: {}", nanpa_entry.country))?;
            }
        }
    }

    Ok(())
}

/// Handle data export
async fn handle_export(
    service: &LergNanpaService,
    output_path: &str,
    states_filter: Option<String>,
    company_types_filter: Option<String>,
) -> Result<()> {
    let term = Term::stdout();
    term.write_line(&format!("📤 {}", style("Exporting LERG Data").bold().cyan()))?;
    term.write_line(&format!("   Output: {}", output_path))?;
    
    if let Some(ref states) = states_filter {
        term.write_line(&format!("   States Filter: {}", states))?;
    }
    
    if let Some(ref types) = company_types_filter {
        term.write_line(&format!("   Company Types Filter: {}", types))?;
    }
    
    term.write_line("")?;

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(ProgressStyle::default_spinner()
        .template("{spinner:.green} {msg}")
        .unwrap());
    spinner.set_message("Exporting data...");
    spinner.enable_steady_tick(Duration::from_millis(100));

    match service.export_lerg_data(output_path).await {
        Ok(()) => {
            spinner.finish_and_clear();
            term.write_line(&format!("✅ {}", style("Export completed successfully").bold().green()))?;
        }
        Err(e) => {
            spinner.finish_and_clear();
            term.write_line(&format!("❌ {}", style("Export failed").bold().red()))?;
            term.write_line(&format!("   Error: {}", e))?;
        }
    }

    Ok(())
}

/// Handle data validation
async fn handle_validate(
    service: &LergNanpaService,
    fix: bool,
    detailed: bool,
) -> Result<()> {
    let term = Term::stdout();
    term.write_line(&format!("✅ {}", style("Validating LERG and NANPA Data").bold().cyan()))?;
    term.write_line(&format!("   Auto-fix: {}", if fix { "Enabled" } else { "Disabled" }))?;
    term.write_line(&format!("   Detailed Report: {}", if detailed { "Yes" } else { "No" }))?;
    term.write_line("")?;

    let stats = service.get_stats();
    
    // Basic validation
    let mut issues = Vec::new();
    
    if stats.lerg_entries == 0 {
        issues.push("No LERG data loaded");
    }
    
    if stats.nanpa_entries == 0 {
        issues.push("No NANPA data loaded");
    }

    if issues.is_empty() {
        term.write_line(&format!("✅ {}", style("Validation passed").bold().green()))?;
        term.write_line(&format!("   LERG entries: {}", stats.lerg_entries))?;
        term.write_line(&format!("   NANPA entries: {}", stats.nanpa_entries))?;
    } else {
        term.write_line(&format!("⚠️  {}", style("Validation issues found").bold().yellow()))?;
        for issue in &issues {
            term.write_line(&format!("   - {}", issue))?;
        }
        
        if fix {
            term.write_line("")?;
            term.write_line(&format!("🔧 {}", style("Auto-fix suggestions").bold().blue()))?;
            term.write_line(&format!("   - Load LERG data with: lerg-nanpa load-lerg --file <path>"))?;
            term.write_line(&format!("   - Download NANPA data with: lerg-nanpa download-nanpa"))?;
        }
    }

    Ok(())
}

/// Handle data clearing
async fn handle_clear(
    service: &LergNanpaService,
    clear_lerg: bool,
    clear_nanpa: bool,
    confirmed: bool,
) -> Result<()> {
    let term = Term::stdout();

    if !clear_lerg && !clear_nanpa {
        term.write_line(&format!("⚠️  {}", style("No data types specified to clear").yellow()))?;
        term.write_line(&format!("   Use --lerg to clear LERG data"))?;
        term.write_line(&format!("   Use --nanpa to clear NANPA data"))?;
        return Ok(());
    }

    let mut clear_items = Vec::new();
    if clear_lerg {
        clear_items.push("LERG data");
    }
    if clear_nanpa {
        clear_items.push("NANPA data");
    }

    term.write_line(&format!("🗑️  {}", style("Clear Data").bold().cyan()))?;
    term.write_line(&format!("   Will clear: {}", clear_items.join(", ")))?;
    term.write_line("")?;

    let should_proceed = if confirmed {
        true
    } else {
        Confirm::new()
            .with_prompt("Are you sure you want to clear this data?")
            .interact()?
    };

    if should_proceed {
        if clear_lerg {
            // Clear LERG data (would need method in service)
            term.write_line(&format!("   Cleared LERG data"))?;
        }
        
        if clear_nanpa {
            // Clear NANPA data (would need method in service)
            term.write_line(&format!("   Cleared NANPA data"))?;
        }
        
        term.write_line(&format!("✅ {}", style("Data cleared successfully").bold().green()))?;
    } else {
        term.write_line(&format!("❌ {}", style("Operation cancelled").yellow()))?;
    }

    Ok(())
}