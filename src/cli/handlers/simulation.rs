/*
 * Redfire Switch - Call Simulation CLI Interface
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
use dialoguer::{Select, Input, Confirm};
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;
use tracing::error;

use crate::cli::SimulateCommands;
use crate::call_simulator::{CallSimulator, CallSimulationConfig};
use crate::termination_routing::{NanpaJurisdiction, utils as routing_utils};
use crate::origination_routing::utils as origination_utils;

// Cached state mapping to avoid repeated lookups
static STATE_MAPPING: OnceLock<HashMap<String, String>> = OnceLock::new();

// Cached test scenarios to avoid recreation
static TEST_SCENARIOS: OnceLock<HashMap<&'static str, Vec<(String, String)>>> = OnceLock::new();

/// Handle simulation CLI commands
pub async fn handle_simulate_command(command: SimulateCommands) -> Result<()> {
    match command {
        SimulateCommands::Call {
            destination,
            origination,
            call_id,
            switch_id,
            max_cost,
            min_quality,
            detailed,
            test_jurisdiction,
        } => {
            handle_single_call_simulation(
                &destination,
                &origination,
                call_id,
                &switch_id,
                max_cost,
                min_quality,
                detailed,
                test_jurisdiction,
            ).await
        }
        SimulateCommands::Batch {
            count,
            rate,
            nanpa_jurisdiction,
            output,
            progress,
        } => {
            handle_batch_simulation(count, rate, nanpa_jurisdiction, output, progress).await
        }
        SimulateCommands::Load {
            duration,
            cps,
            threads,
            report_interval,
        } => {
            handle_load_test(duration, cps, threads, report_interval).await
        }
        SimulateCommands::Interactive => {
            handle_interactive_simulation().await
        }
        SimulateCommands::MockTrunks {
            count,
            failures,
            capacity_limits,
            show_config,
        } => {
            handle_mock_trunk_setup(count, failures, capacity_limits, show_config).await
        }
        SimulateCommands::Jurisdiction {
            all,
            interstate,
            intrastate,
            local,
            indeterminate,
            international,
            count,
        } => {
            handle_jurisdiction_testing(
                all, interstate, intrastate, local, indeterminate, international, count
            ).await
        }
    }
}

/// Create a simulator with the given configuration
fn create_simulator(enable_nanpa: bool, mock_trunks: Option<u32>, failures: bool, capacity_limits: bool) -> CallSimulator {
    let config = CallSimulationConfig {
        enable_nanpa_jurisdiction: enable_nanpa,
        mock_trunks_per_group: mock_trunks.unwrap_or(5),
        simulate_failures: failures,
        simulate_capacity_limits: capacity_limits,
        ..CallSimulationConfig::default()
    };
    CallSimulator::new(config)
}

/// Create a basic simulator for simple operations
fn create_basic_simulator() -> CallSimulator {
    create_simulator(false, None, false, false)
}

/// Create a NANPA-enabled simulator for jurisdiction testing
fn create_nanpa_simulator() -> CallSimulator {
    create_simulator(true, None, false, false)
}

/// Get cached state mapping
fn get_state_mapping() -> &'static HashMap<String, String> {
    STATE_MAPPING.get_or_init(|| routing_utils::get_area_code_state_mapping())
}

/// Get cached test scenarios
fn get_test_scenarios() -> &'static HashMap<&'static str, Vec<(String, String)>> {
    TEST_SCENARIOS.get_or_init(|| create_jurisdiction_test_scenarios())
}

/// Show area code information with cached lookup
fn show_area_code_info(term: &Term, label: &str, number: &str) -> Result<()> {
    if let Some(area_code) = origination_utils::extract_area_code(number) {
        let state_mapping = get_state_mapping();
        let state = state_mapping.get(&area_code).map(|s| s.as_str()).unwrap_or("Unknown");
        term.write_line(&format!("   {}: {} ({})", label, area_code, state))?;
    }
    Ok(())
}

/// Create progress bar with consistent styling
fn create_progress_bar(total: u64, message: &str) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(&format!("{{spinner:.green}} [{{elapsed_precise}}] [{{bar:40.cyan/blue}}] {{pos}}/{{len}} {} ({{percent}}%) ETA: {{eta}}", message))
            .expect("Invalid progress bar template")
            .progress_chars("=>-"),
    );
    pb
}

/// Handle errors consistently across simulation functions
fn handle_simulation_error(term: &Term, error: &anyhow::Error, context: &str) -> Result<()> {
    term.write_line(&format!("❌ {}: {}", style(context).bold().red(), error))?;
    error!("Simulation error in {}: {}", context, error);
    Ok(())
}

/// Handle single call simulation
pub async fn handle_single_call_simulation(
    destination: &str,
    origination: &str,
    call_id: Option<String>,
    switch_id: &str,
    max_cost: Option<f64>,
    min_quality: Option<u8>,
    detailed: bool,
    test_jurisdiction: bool,
) -> Result<()> {
    let term = Term::stdout();
    term.write_line(&format!("🔄 {}", style("Simulating Call Routing").bold().cyan()))?;
    term.write_line("")?;

    // Create simulator with configuration
    let simulator = if test_jurisdiction { create_nanpa_simulator() } else { create_basic_simulator() };
    
    // Show call details
    term.write_line(&format!("📞 Call Details:"))?;
    term.write_line(&format!("   From: {}", style(origination).green()))?;
    term.write_line(&format!("   To:   {}", style(destination).green()))?;
    term.write_line(&format!("   ID:   {}", style(call_id.as_deref().unwrap_or("auto-generated")).dim()))?;
    
    if let Some(cost) = max_cost {
        term.write_line(&format!("   Max Cost: ${:.4}/min", cost))?;
    }
    if let Some(quality) = min_quality {
        term.write_line(&format!("   Min Quality: {}%", quality))?;
    }
    term.write_line("")?;

    // Show jurisdiction analysis if requested
    if test_jurisdiction {
        show_jurisdiction_analysis(&term, destination, origination).await?;
    }

    // Run simulation
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(ProgressStyle::default_spinner()
        .template("{spinner:.green} {msg}")
        .unwrap());
    spinner.set_message("Routing call...");
    spinner.enable_steady_tick(Duration::from_millis(100));

    let result = simulator.simulate_call(
        destination,
        origination,
        call_id,
        switch_id,
        max_cost,
        min_quality,
    ).await?;

    spinner.finish_and_clear();

    // Display results
    if result.success {
        term.write_line(&format!("✅ {}", style("Call Routing Successful").bold().green()))?;
        
        if let Some(trunk) = &result.selected_trunk {
            term.write_line(&format!("   Selected Trunk: {}", style(trunk).cyan()))?;
        }
        
        if let Some(cost) = result.cost_per_minute {
            term.write_line(&format!("   Rate: ${:.4}/min", cost))?;
        }
        
        if let Some(quality) = result.quality_score {
            term.write_line(&format!("   Quality Score: {}%", quality))?;
        }
        
        if let Some(jurisdiction) = result.jurisdiction {
            term.write_line(&format!("   Jurisdiction: {}", style(jurisdiction.description()).yellow()))?;
        }
        
        term.write_line(&format!("   Processing Time: {}ms", result.processing_time_ms))?;
        
        if detailed {
            show_detailed_routing_analysis(&term, &result).await?;
        }
    } else {
        term.write_line(&format!("❌ {}", style("Call Routing Failed").bold().red()))?;
        
        if let Some(error) = &result.error {
            term.write_line(&format!("   Error: {}", style(error).red()))?;
        }
        
        term.write_line(&format!("   Processing Time: {}ms", result.processing_time_ms))?;
    }

    Ok(())
}

/// Show jurisdiction analysis
async fn show_jurisdiction_analysis(term: &Term, destination: &str, origination: &str) -> Result<()> {
    term.write_line(&format!("🗺️  {}", style("NANPA Jurisdiction Analysis").bold().yellow()))?;
    
    // Check number types
    let dest_nanpa = origination_utils::is_nanpa_number(destination);
    let orig_nanpa = origination_utils::is_nanpa_number(origination);
    
    term.write_line(&format!("   Destination: {} ({})", 
        destination, 
        if dest_nanpa { style("NANPA").green() } else { style("International").red() }
    ))?;
    
    term.write_line(&format!("   Origination: {} ({})", 
        origination, 
        if orig_nanpa { style("NANPA").green() } else { style("International").red() }
    ))?;

    // Show area code analysis
    if dest_nanpa {
        show_area_code_info(term, "Dest Area Code", destination)?;
    }
    
    if orig_nanpa {
        show_area_code_info(term, "Orig Area Code", origination)?;
    }

    term.write_line("")?;
    Ok(())
}

/// Show detailed routing analysis
async fn show_detailed_routing_analysis(
    term: &Term, 
    result: &crate::call_simulator::CallSimulationResult
) -> Result<()> {
    term.write_line("")?;
    term.write_line(&format!("🔍 {}", style("Detailed Analysis").bold().blue()))?;
    
    term.write_line(&format!("   Call ID: {}", result.call_id))?;
    term.write_line(&format!("   Timestamp: {}", result.timestamp.format("%Y-%m-%d %H:%M:%S UTC")))?;
    
    if let Some(jurisdiction) = result.jurisdiction {
        term.write_line(&format!("   NANPA Jurisdiction: {}", jurisdiction.description()))?;
        
        let routing_notes = match jurisdiction {
            NanpaJurisdiction::Interstate => "Routed via interstate carrier network",
            NanpaJurisdiction::Intrastate => "Routed via intrastate/local carrier",
            NanpaJurisdiction::Local => "Routed via local exchange carrier",
            NanpaJurisdiction::Indeterminate => "Routed via default/fallback path",
            NanpaJurisdiction::International => "Routed via international gateway",
        };
        
        term.write_line(&format!("   Routing Notes: {}", style(routing_notes).dim()))?;
    }
    
    Ok(())
}

/// Handle batch simulation
pub async fn handle_batch_simulation(
    count: u32,
    rate: u32,
    nanpa_jurisdiction: bool,
    output: Option<String>,
    show_progress: bool,
) -> Result<()> {
    let term = Term::stdout();
    term.write_line(&format!("🚀 {}", style("Batch Call Simulation").bold().cyan()))?;
    term.write_line(&format!("   Calls: {}", count))?;
    term.write_line(&format!("   Rate: {} CPS", rate))?;
    term.write_line(&format!("   NANPA Jurisdiction: {}", if nanpa_jurisdiction { "Enabled" } else { "Disabled" }))?;
    
    if let Some(ref output_file) = output {
        term.write_line(&format!("   Output: {}", output_file))?;
    }
    term.write_line("")?;

    let simulator = create_simulator(nanpa_jurisdiction, None, false, false);

    // Setup progress bar
    let progress_bar = if show_progress {
        Some(create_progress_bar(count as u64, "calls"))
    } else {
        None
    };

    // Run simulation
    let stats = simulator.run_batch_simulation(count, rate, output).await?;
    
    if let Some(pb) = &progress_bar {
        pb.set_position(count as u64);
        pb.finish_with_message("Batch simulation completed");
    }

    // Display results
    term.write_line("")?;
    term.write_line(&format!("📊 {}", style("Batch Simulation Results").bold().green()))?;
    term.write_line(&format!("   Total Calls: {}", stats.total_calls))?;
    term.write_line(&format!("   Successful: {} ({:.1}%)", 
        stats.successful_calls, stats.success_rate))?;
    term.write_line(&format!("   Failed: {}", stats.failed_calls))?;
    term.write_line(&format!("   Avg Processing Time: {:.2}ms", stats.avg_processing_time_ms))?;
    term.write_line(&format!("   Avg Cost: ${:.4}/min", stats.avg_cost_per_minute))?;
    term.write_line(&format!("   Duration: {:.1}s", stats.duration_seconds))?;

    // Show jurisdiction breakdown if applicable
    if nanpa_jurisdiction && !stats.jurisdiction_stats.is_empty() {
        term.write_line("")?;
        term.write_line(&format!("🗺️  {}", style("Jurisdiction Breakdown").bold().yellow()))?;
        
        for (jurisdiction, count) in &stats.jurisdiction_stats {
            let percentage = (*count as f64 / stats.total_calls as f64) * 100.0;
            term.write_line(&format!("   {}: {} ({:.1}%)", 
                jurisdiction.description(), count, percentage))?;
        }
    }

    Ok(())
}

/// Handle load test
pub async fn handle_load_test(
    duration: u64,
    cps: u32,
    threads: u32,
    report_interval: u64,
) -> Result<()> {
    let term = Term::stdout();
    term.write_line(&format!("⚡ {}", style("Load Test Starting").bold().cyan()))?;
    term.write_line(&format!("   Duration: {}s", duration))?;
    term.write_line(&format!("   Target CPS: {}", cps))?;
    term.write_line(&format!("   Threads: {}", threads))?;
    term.write_line(&format!("   Report Interval: {}s", report_interval))?;
    term.write_line("")?;

    let simulator = create_simulator(false, None, false, false);

    simulator.run_load_test(duration, cps, threads, report_interval).await?;

    term.write_line("")?;
    term.write_line(&format!("✅ {}", style("Load Test Completed").bold().green()))?;

    Ok(())
}

/// Handle interactive simulation
pub async fn handle_interactive_simulation() -> Result<()> {
    let term = Term::stdout();
    term.clear_screen()?;
    term.write_line(&format!("🎛️  {}", style("Interactive Call Simulation").bold().cyan()))?;
    term.write_line("")?;

    loop {
        let options = vec![
            "Single Call Test",
            "Batch Simulation",
            "NANPA Jurisdiction Test",
            "Mock Trunk Configuration", 
            "Load Test",
            "Exit",
        ];

        let selection = Select::new()
            .with_prompt("Select simulation type")
            .items(&options)
            .interact()?;

        match selection {
            0 => interactive_single_call().await?,
            1 => interactive_batch_simulation().await?,
            2 => interactive_jurisdiction_test().await?,
            3 => interactive_mock_trunk_config().await?,
            4 => interactive_load_test().await?,
            5 => break,
            _ => unreachable!(),
        }

        if !Confirm::new()
            .with_prompt("Continue with another simulation?")
            .interact()? {
            break;
        }

        term.write_line("")?;
    }

    term.write_line(&format!("👋 {}", style("Goodbye!").bold().green()))?;
    Ok(())
}

/// Interactive single call test
async fn interactive_single_call() -> Result<()> {
    let destination: String = Input::new()
        .with_prompt("Destination number (E.164)")
        .with_initial_text("15551234567")
        .interact_text()?;

    let origination: String = Input::new()
        .with_prompt("Origination number (E.164)")
        .with_initial_text("15559876543")
        .interact_text()?;

    let test_jurisdiction = Confirm::new()
        .with_prompt("Test NANPA jurisdiction routing?")
        .interact()?;

    let detailed = Confirm::new()
        .with_prompt("Show detailed analysis?")
        .interact()?;

    handle_single_call_simulation(
        &destination,
        &origination,
        None,
        "interactive",
        None,
        None,
        detailed,
        test_jurisdiction,
    ).await
}

/// Interactive batch simulation
async fn interactive_batch_simulation() -> Result<()> {
    let count: u32 = Input::new()
        .with_prompt("Number of calls")
        .with_initial_text("100")
        .interact()?;

    let rate: u32 = Input::new()
        .with_prompt("Calls per second")
        .with_initial_text("10")
        .interact()?;

    let nanpa_jurisdiction = Confirm::new()
        .with_prompt("Enable NANPA jurisdiction routing?")
        .interact()?;

    let output_file = if Confirm::new()
        .with_prompt("Export results to CSV?")
        .interact()? {
        Some(Input::new()
            .with_prompt("Output filename")
            .with_initial_text("batch_simulation_results.csv")
            .interact_text()?)
    } else {
        None
    };

    handle_batch_simulation(count, rate, nanpa_jurisdiction, output_file, true).await
}

/// Interactive jurisdiction test
async fn interactive_jurisdiction_test() -> Result<()> {
    let term = Term::stdout();
    term.write_line(&format!("🗺️  {}", style("NANPA Jurisdiction Testing").bold().yellow()))?;

    let jurisdictions = vec![
        "All jurisdictions",
        "Interstate only", 
        "Intrastate only",
        "Local only",
        "Indeterminate only",
        "International only",
    ];

    let selection = Select::new()
        .with_prompt("Select jurisdiction to test")
        .items(&jurisdictions)
        .interact()?;

    let count: u32 = Input::new()
        .with_prompt("Number of test calls per jurisdiction")
        .with_initial_text("10")
        .interact()?;

    handle_jurisdiction_testing(
        selection == 0, // all
        selection == 1, // interstate
        selection == 2, // intrastate
        selection == 3, // local
        selection == 4, // indeterminate
        selection == 5, // international
        count,
    ).await
}

/// Interactive mock trunk configuration
async fn interactive_mock_trunk_config() -> Result<()> {
    let count: u32 = Input::new()
        .with_prompt("Number of mock trunks per routing group")
        .with_initial_text("5")
        .interact()?;

    let failures = Confirm::new()
        .with_prompt("Simulate trunk failures?")
        .interact()?;

    let capacity_limits = Confirm::new()
        .with_prompt("Simulate capacity limits?")
        .interact()?;

    handle_mock_trunk_setup(count, failures, capacity_limits, true).await
}

/// Interactive load test
async fn interactive_load_test() -> Result<()> {
    let duration: u64 = Input::new()
        .with_prompt("Duration in seconds")
        .with_initial_text("60")
        .interact()?;

    let cps: u32 = Input::new()
        .with_prompt("Target calls per second")
        .with_initial_text("50")
        .interact()?;

    let threads: u32 = Input::new()
        .with_prompt("Number of threads")
        .with_initial_text("4")
        .interact()?;

    handle_load_test(duration, cps, threads, 10).await
}

/// Handle mock trunk setup
pub async fn handle_mock_trunk_setup(
    count: u32,
    failures: bool,
    capacity_limits: bool,
    show_config: bool,
) -> Result<()> {
    let term = Term::stdout();
    term.write_line(&format!("🔧 {}", style("Mock Trunk Setup").bold().cyan()))?;

    let simulator = create_simulator(false, Some(count), failures, capacity_limits);
    let mock_trunks = simulator.get_mock_trunks();

    term.write_line(&format!("   Created {} routing groups with {} trunks each", 
        mock_trunks.len(), count))?;

    if failures {
        term.write_line(&format!("   {} Trunk failures enabled", style("⚠️").yellow()))?;
    }

    if capacity_limits {
        term.write_line(&format!("   {} Capacity limits enabled", style("📊").blue()))?;
    }

    if show_config {
        term.write_line("")?;
        term.write_line(&format!("📋 {}", style("Mock Trunk Configuration").bold().blue()))?;
        
        for (group_name, trunks) in mock_trunks {
            term.write_line("")?;
            term.write_line(&format!("   Group: {}", style(group_name).green().bold()))?;
            
            for (i, trunk) in trunks.iter().enumerate().take(3) { // Show first 3
                let status = if trunk.enabled {
                    style("✅ ENABLED").green()
                } else {
                    style("❌ DISABLED").red()
                };
                
                term.write_line(&format!("     {}. {} - {} (Quality: {}%, Capacity: {})",
                    i + 1,
                    trunk.id,
                    status,
                    trunk.quality_score,
                    trunk.max_concurrent_calls
                ))?;
            }
            
            if trunks.len() > 3 {
                term.write_line(&format!("     ... and {} more trunks", trunks.len() - 3))?;
            }
        }
    }

    Ok(())
}

/// Handle jurisdiction testing
pub async fn handle_jurisdiction_testing(
    all: bool,
    interstate: bool,
    intrastate: bool,
    local: bool,
    indeterminate: bool,
    international: bool,
    count: u32,
) -> Result<()> {
    let term = Term::stdout();
    term.write_line(&format!("🗺️  {}", style("NANPA Jurisdiction Testing").bold().yellow()))?;

    let simulator = create_simulator(true, None, false, false);

    // Get cached test scenarios for each jurisdiction
    let test_scenarios = get_test_scenarios();
    
    let mut results = HashMap::new();

    if all || interstate {
        term.write_line(&format!("Testing Interstate calls..."))?;
        let interstate_results = run_jurisdiction_test(&simulator, &test_scenarios["interstate"], count).await?;
        results.insert("Interstate", interstate_results);
    }

    if all || intrastate {
        term.write_line(&format!("Testing Intrastate calls..."))?;
        let intrastate_results = run_jurisdiction_test(&simulator, &test_scenarios["intrastate"], count).await?;
        results.insert("Intrastate", intrastate_results);
    }

    if all || local {
        if let Some(local_scenarios) = test_scenarios.get("local") {
            term.write_line(&format!("Testing Local calls..."))?;
            let local_results = run_jurisdiction_test(&simulator, local_scenarios, count).await?;
            results.insert("Local", local_results);
        } else {
            term.write_line(&format!("⚠️  Local testing requires LERG data to be loaded"))?;
        }
    }

    if all || indeterminate {
        term.write_line(&format!("Testing Indeterminate calls..."))?;
        let indeterminate_results = run_jurisdiction_test(&simulator, &test_scenarios["indeterminate"], count).await?;
        results.insert("Indeterminate", indeterminate_results);
    }

    if all || international {
        term.write_line(&format!("Testing International calls..."))?;
        let international_results = run_jurisdiction_test(&simulator, &test_scenarios["international"], count).await?;
        results.insert("International", international_results);
    }

    // Display results summary
    term.write_line("")?;
    term.write_line(&format!("📊 {}", style("Jurisdiction Test Results").bold().green()))?;
    
    for (jurisdiction, (success_count, total_count)) in results {
        let success_rate = (success_count as f64 / total_count as f64) * 100.0;
        term.write_line(&format!("   {}: {}/{} ({:.1}% success)", 
            jurisdiction, success_count, total_count, success_rate))?;
    }

    Ok(())
}

/// Create test scenarios for jurisdiction testing
fn create_jurisdiction_test_scenarios() -> HashMap<&'static str, Vec<(String, String)>> {
    let mut scenarios = HashMap::new();
    
    // Interstate scenarios (NY to CA)
    scenarios.insert("interstate", vec![
        ("12125551234".to_string(), "13105557890".to_string()), // NYC to LA
        ("17185551234".to_string(), "14155557890".to_string()), // NYC to SF
        ("13125551234".to_string(), "12145557890".to_string()), // Chicago to Dallas
    ]);

    // Intrastate scenarios (within same state)
    scenarios.insert("intrastate", vec![
        ("12125551234".to_string(), "17185557890".to_string()), // NYC areas
        ("13105551234".to_string(), "14155557890".to_string()), // CA areas
        ("17135551234".to_string(), "12815557890".to_string()), // TX areas
    ]);

    // Local scenarios (same rate center - requires LERG data for accuracy)
    scenarios.insert("local", vec![
        ("12125551234".to_string(), "12125557890".to_string()), // Same NYC exchange
        ("13105551234".to_string(), "13105557890".to_string()), // Same LA exchange
        ("17135551234".to_string(), "17135557890".to_string()), // Same Houston exchange
    ]);

    // Indeterminate scenarios
    scenarios.insert("indeterminate", vec![
        ("15551234567".to_string(), "unknown".to_string()),     // Unknown ANI
        ("15551234567".to_string(), "18005551234".to_string()), // Toll free origin
        ("15551234567".to_string(), "442071234567".to_string()), // International origin
    ]);

    // International scenarios
    scenarios.insert("international", vec![
        ("442071234567".to_string(), "15551234567".to_string()), // UK destination
        ("33123456789".to_string(), "15551234567".to_string()),  // France destination
        ("491234567890".to_string(), "15551234567".to_string()), // Germany destination
    ]);

    scenarios
}

/// Run jurisdiction test for specific scenario
async fn run_jurisdiction_test(
    simulator: &CallSimulator,
    scenarios: &[(String, String)],
    count_per_scenario: u32,
) -> Result<(u32, u32)> {
    let mut success_count = 0;
    let mut total_count = 0;

    for (destination, origination) in scenarios {
        for _ in 0..count_per_scenario {
            match simulator.simulate_call(
                destination,
                origination,
                None,
                "jurisdiction-test",
                None,
                None,
            ).await {
                Ok(result) => {
                    total_count += 1;
                    if result.success {
                        success_count += 1;
                    }
                }
                Err(e) => {
                    error!("Failed to simulate jurisdiction test call from {} to {}: {}", origination, destination, e);
                    total_count += 1; // Count as attempted even if failed
                }
            }
        }
    }

    Ok((success_count, total_count))
}