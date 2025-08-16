/*
 * Redfire Switch - Call Simulation and Testing Framework
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
use csv::Writer;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::interval;
use tracing::{info, debug};
use uuid::Uuid;

use crate::termination_routing::{
    TerminationRoutingService, TerminationRoutingRequest,
    TerminationTrunk, TrunkCodecConfig, TrunkCnamConfig,
    NanpaJurisdiction, CpsTracker, QosRequirements,
    utils as routing_utils
};
use crate::origination_routing::{TollFreePrefix, utils as origination_utils};

/// Call simulation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallSimulationConfig {
    /// Enable mock trunks for testing
    pub use_mock_trunks: bool,
    /// Number of mock trunks per routing group
    pub mock_trunks_per_group: u32,
    /// Simulate trunk failures
    pub simulate_failures: bool,
    /// Simulate capacity limits
    pub simulate_capacity_limits: bool,
    /// Enable NANPA jurisdiction routing
    pub enable_nanpa_jurisdiction: bool,
    /// Simulation timeout in seconds
    pub timeout_seconds: u64,
    /// Default origination for test calls
    pub default_origination: String,
    /// Test call patterns
    pub test_patterns: Vec<TestCallPattern>,
}

impl Default for CallSimulationConfig {
    fn default() -> Self {
        Self {
            use_mock_trunks: true,
            mock_trunks_per_group: 5,
            simulate_failures: false,
            simulate_capacity_limits: false,
            enable_nanpa_jurisdiction: true,
            timeout_seconds: 30,
            default_origination: "15551234567".to_string(),
            test_patterns: create_default_test_patterns(),
        }
    }
}

/// Test call pattern for batch simulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCallPattern {
    /// Pattern name
    pub name: String,
    /// Destination number pattern
    pub destination_pattern: String,
    /// Origination number pattern (optional)
    pub origination_pattern: Option<String>,
    /// Expected jurisdiction (for NANPA)
    pub expected_jurisdiction: Option<NanpaJurisdiction>,
    /// Weight for random selection
    pub weight: u32,
}

/// Call simulation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallSimulationResult {
    /// Call ID
    pub call_id: String,
    /// Destination number
    pub destination: String,
    /// Origination number
    pub origination: String,
    /// Routing success
    pub success: bool,
    /// Selected trunk (if successful)
    pub selected_trunk: Option<String>,
    /// Routing group used
    pub routing_group: Option<String>,
    /// NANPA jurisdiction (if applicable)
    pub jurisdiction: Option<NanpaJurisdiction>,
    /// Processing time in milliseconds
    pub processing_time_ms: u64,
    /// Cost per minute
    pub cost_per_minute: Option<f64>,
    /// Quality score
    pub quality_score: Option<u8>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Batch simulation statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchSimulationStats {
    /// Total calls attempted
    pub total_calls: u64,
    /// Successful calls
    pub successful_calls: u64,
    /// Failed calls
    pub failed_calls: u64,
    /// Success rate percentage
    pub success_rate: f64,
    /// Average processing time in milliseconds
    pub avg_processing_time_ms: f64,
    /// Average cost per minute
    pub avg_cost_per_minute: f64,
    /// Jurisdiction breakdown (for NANPA)
    pub jurisdiction_stats: HashMap<NanpaJurisdiction, u64>,
    /// Routing group utilization
    pub routing_group_stats: HashMap<String, u64>,
    /// Start time
    pub start_time: DateTime<Utc>,
    /// End time
    pub end_time: DateTime<Utc>,
    /// Duration in seconds
    pub duration_seconds: f64,
}

/// Load test statistics
#[derive(Debug, Clone)]
pub struct LoadTestStats {
    /// Calls attempted
    pub calls_attempted: Arc<Mutex<u64>>,
    /// Calls successful
    pub calls_successful: Arc<Mutex<u64>>,
    /// Calls failed
    pub calls_failed: Arc<Mutex<u64>>,
    /// Total processing time (for averaging)
    pub total_processing_time_ms: Arc<Mutex<u64>>,
    /// Current CPS
    pub current_cps: Arc<Mutex<f64>>,
    /// Peak CPS achieved
    pub peak_cps: Arc<Mutex<f64>>,
    /// Start time
    pub start_time: Instant,
}

impl LoadTestStats {
    pub fn new() -> Self {
        Self {
            calls_attempted: Arc::new(Mutex::new(0)),
            calls_successful: Arc::new(Mutex::new(0)),
            calls_failed: Arc::new(Mutex::new(0)),
            total_processing_time_ms: Arc::new(Mutex::new(0)),
            current_cps: Arc::new(Mutex::new(0.0)),
            peak_cps: Arc::new(Mutex::new(0.0)),
            start_time: Instant::now(),
        }
    }

    pub fn record_call(&self, success: bool, processing_time_ms: u64) {
        *self.calls_attempted.lock() += 1;
        if success {
            *self.calls_successful.lock() += 1;
        } else {
            *self.calls_failed.lock() += 1;
        }
        *self.total_processing_time_ms.lock() += processing_time_ms;
    }

    pub fn update_cps(&self, cps: f64) {
        *self.current_cps.lock() = cps;
        let mut peak = self.peak_cps.lock();
        if cps > *peak {
            *peak = cps;
        }
    }

    pub fn get_summary(&self) -> (u64, u64, u64, f64, f64, f64) {
        let attempted = *self.calls_attempted.lock();
        let successful = *self.calls_successful.lock();
        let failed = *self.calls_failed.lock();
        let total_time = *self.total_processing_time_ms.lock();
        let current_cps = *self.current_cps.lock();
        let peak_cps = *self.peak_cps.lock();
        
        let avg_time = if attempted > 0 {
            total_time as f64 / attempted as f64
        } else {
            0.0
        };

        (attempted, successful, failed, avg_time, current_cps, peak_cps)
    }
}

/// Call simulation service
pub struct CallSimulator {
    /// Simulation configuration
    config: CallSimulationConfig,
    /// Termination routing service
    routing_service: Arc<TerminationRoutingService>,
    /// Mock trunks registry
    mock_trunks: HashMap<String, Vec<TerminationTrunk>>,
    /// Active simulations tracking
    active_simulations: Arc<Mutex<HashMap<String, Instant>>>,
}

impl CallSimulator {
    /// Create a new call simulator
    pub fn new(config: CallSimulationConfig) -> Self {
        // Create routing service with mock configuration
        let routing_plans = if config.enable_nanpa_jurisdiction {
            vec![routing_utils::create_nanpa_routing_plan()]
        } else {
            vec![routing_utils::create_default_routing_plan()]
        };

        let mut simulator = Self {
            routing_service: Arc::new(TerminationRoutingService::new(routing_plans)),
            mock_trunks: HashMap::new(),
            active_simulations: Arc::new(Mutex::new(HashMap::new())),
            config,
        };

        // Setup mock trunks if enabled
        if simulator.config.use_mock_trunks {
            simulator.setup_mock_trunks();
        }

        simulator
    }

    /// Setup mock trunks for testing
    pub fn setup_mock_trunks(&mut self) {
        info!("Setting up mock trunks for simulation");

        // Create jurisdiction-specific routing groups if NANPA is enabled
        if self.config.enable_nanpa_jurisdiction {
            self.setup_nanpa_mock_trunks();
        } else {
            self.setup_basic_mock_trunks();
        }

        info!("Mock trunk setup completed");
    }

    /// Setup NANPA jurisdiction-specific mock trunks
    fn setup_nanpa_mock_trunks(&mut self) {
        let jurisdiction_groups = routing_utils::create_jurisdiction_routing_groups();
        
        for (group_name, _group) in jurisdiction_groups {
            let mut trunks = Vec::new();
            
            for i in 0..self.config.mock_trunks_per_group {
                let trunk = self.create_mock_trunk(&group_name, i);
                trunks.push(trunk);
            }
            
            self.mock_trunks.insert(group_name.clone(), trunks);
            debug!("Created {} mock trunks for group {}", self.config.mock_trunks_per_group, group_name);
        }
    }

    /// Setup basic mock trunks for non-NANPA testing
    fn setup_basic_mock_trunks(&mut self) {
        let basic_groups = vec![
            "us-canada".to_string(),
            "international".to_string(),
            "premium".to_string(),
            "economy".to_string(),
        ];

        for group_name in basic_groups {
            let mut trunks = Vec::new();
            
            for i in 0..self.config.mock_trunks_per_group {
                let trunk = self.create_mock_trunk(&group_name, i);
                trunks.push(trunk);
            }
            
            self.mock_trunks.insert(group_name.clone(), trunks);
        }
    }

    /// Create a mock trunk for testing
    fn create_mock_trunk(&self, group_name: &str, index: u32) -> TerminationTrunk {
        let trunk_id = format!("mock-{}-trunk-{:02}", group_name, index);
        let base_quality = match group_name {
            name if name.contains("interstate") => 85,
            name if name.contains("intrastate") => 90,
            name if name.contains("local") => 95,
            name if name.contains("international") => 75,
            _ => 80,
        };

        // Add some randomness to simulate real-world variation
        let quality_variation = (index % 20) as i8 - 10; // -10 to +10
        let quality = ((base_quality as i8 + quality_variation).max(50).min(100)) as u8;

        let max_concurrent = if self.config.simulate_capacity_limits {
            Some(10 + (index % 5) * 5) // 10-25 concurrent calls
        } else {
            Some(100) // High limit for testing
        };

        TerminationTrunk {
            id: trunk_id.clone(),
            name: format!("Mock Trunk {}", trunk_id),
            sip_profile: format!("mock-profile-{}", group_name),
            enabled: !self.config.simulate_failures || (index % 5) != 0, // 20% failure rate if enabled
            priority: index,
            weight: 100 - (index * 5), // Higher weight for lower index
            max_concurrent_calls: max_concurrent.unwrap_or(100),
            cps_limit: 50,
            active_calls: Arc::new(Mutex::new(0)),
            cps_tracker: Arc::new(Mutex::new(CpsTracker::new())),
            quality_score: quality,
            success_rate: 95.0 - (index as f64 * 2.0), // 95% to 85%
            asr: 85.0 + (index as f64), // 85% to 90%
            acd: 120.0 + (index as f64 * 10.0), // 120 to 170 seconds
            last_stats_update: Utc::now(),
            codec_config: TrunkCodecConfig::default(),
            cnam_config: TrunkCnamConfig::default(),
        }
    }

    /// Simulate a single call routing
    pub async fn simulate_call(
        &self,
        destination: &str,
        origination: &str,
        call_id: Option<String>,
        switch_id: &str,
        max_cost: Option<f64>,
        min_quality: Option<u8>,
    ) -> Result<CallSimulationResult> {
        let start_time = Instant::now();
        let call_id = call_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        
        debug!("Simulating call: {} -> {} (ID: {})", origination, destination, call_id);

        // Track active simulation
        self.active_simulations.lock().insert(call_id.clone(), start_time);

        // Create routing request
        let request = TerminationRoutingRequest {
            destination: destination.to_string(),
            origination: origination.to_string(),
            call_id: call_id.clone(),
            switch_id: switch_id.to_string(),
            max_cost,
            qos_requirements: min_quality.map(|q| QosRequirements {
                min_quality_score: q,
                ..QosRequirements::default()
            }),
            customer_id: "simulation-customer".to_string(),
        };

        // Attempt routing
        let routing_result = if self.config.enable_nanpa_jurisdiction {
            // Use NANPA jurisdiction routing if enabled
            let plans = self.routing_service.get_plans();
            if let Some(plan) = plans.first() {
                self.routing_service.route_call_with_nanpa_jurisdiction(request, plan).await
            } else {
                self.routing_service.route_call(request).await
            }
        } else {
            self.routing_service.route_call(request).await
        };

        let processing_time_ms = start_time.elapsed().as_millis() as u64;

        // Remove from active simulations
        self.active_simulations.lock().remove(&call_id);

        // Process result
        let result = match routing_result {
            Ok(response) => {
                let jurisdiction = if self.config.enable_nanpa_jurisdiction {
                    self.determine_jurisdiction(destination, origination).await
                } else {
                    None
                };

                CallSimulationResult {
                    call_id,
                    destination: destination.to_string(),
                    origination: origination.to_string(),
                    success: response.success,
                    selected_trunk: response.selected_trunk.map(|t| t.id),
                    routing_group: None, // Would extract from response in real implementation
                    jurisdiction,
                    processing_time_ms,
                    cost_per_minute: response.rate_info.map(|r| r.rate_per_minute),
                    quality_score: response.selected_trunk.map(|t| t.quality_score),
                    error: response.error,
                    timestamp: Utc::now(),
                }
            }
            Err(e) => CallSimulationResult {
                call_id,
                destination: destination.to_string(),
                origination: origination.to_string(),
                success: false,
                selected_trunk: None,
                routing_group: None,
                jurisdiction: None,
                processing_time_ms,
                cost_per_minute: None,
                quality_score: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }
        };

        info!("Call simulation completed: {} ({}ms)", 
              if result.success { "SUCCESS" } else { "FAILED" },
              result.processing_time_ms);

        Ok(result)
    }

    /// Determine NANPA jurisdiction for a call (simplified for simulation)
    async fn determine_jurisdiction(&self, destination: &str, origination: &str) -> Option<NanpaJurisdiction> {
        // Check if destination is international
        if !origination_utils::is_nanpa_number(destination) {
            return Some(NanpaJurisdiction::International);
        }

        // Check for indeterminate cases
        if origination.is_empty() || 
           origination == "unknown" || 
           !origination_utils::is_nanpa_number(origination) ||
           TollFreePrefix::is_toll_free(origination) {
            return Some(NanpaJurisdiction::Indeterminate);
        }

        // Simplified state comparison based on area codes
        let dest_area = origination_utils::extract_area_code(destination);
        let orig_area = origination_utils::extract_area_code(origination);

        match (dest_area, orig_area) {
            (Some(dest), Some(orig)) => {
                if self.same_state(&dest, &orig) {
                    Some(NanpaJurisdiction::Intrastate)
                } else {
                    Some(NanpaJurisdiction::Interstate)
                }
            }
            _ => Some(NanpaJurisdiction::Indeterminate)
        }
    }

    /// Check if two area codes are in the same state (simplified)
    fn same_state(&self, area1: &str, area2: &str) -> bool {
        let state_mapping = routing_utils::get_area_code_state_mapping();
        match (state_mapping.get(area1), state_mapping.get(area2)) {
            (Some(state1), Some(state2)) => state1 == state2,
            _ => false
        }
    }

    /// Run batch simulation
    pub async fn run_batch_simulation(
        &self,
        count: u32,
        rate: u32,
        output_file: Option<String>,
    ) -> Result<BatchSimulationStats> {
        info!("Starting batch simulation: {} calls at {} CPS", count, rate);
        
        let start_time = Utc::now();
        let mut results = Vec::new();
        let mut stats = BatchSimulationStats {
            total_calls: 0,
            successful_calls: 0,
            failed_calls: 0,
            success_rate: 0.0,
            avg_processing_time_ms: 0.0,
            avg_cost_per_minute: 0.0,
            jurisdiction_stats: HashMap::new(),
            routing_group_stats: HashMap::new(),
            start_time,
            end_time: start_time,
            duration_seconds: 0.0,
        };

        let call_interval = Duration::from_millis(1000 / rate as u64);
        let mut interval_timer = interval(call_interval);

        for i in 0..count {
            interval_timer.tick().await;

            // Generate test call using patterns
            let pattern = self.select_test_pattern();
            let (destination, origination) = self.generate_test_numbers(&pattern);
            
            let result = self.simulate_call(
                &destination,
                &origination,
                None,
                "batch-sim",
                None,
                None,
            ).await?;

            // Update statistics
            stats.total_calls += 1;
            if result.success {
                stats.successful_calls += 1;
            } else {
                stats.failed_calls += 1;
            }

            // Track jurisdiction statistics
            if let Some(jurisdiction) = result.jurisdiction {
                *stats.jurisdiction_stats.entry(jurisdiction).or_insert(0) += 1;
            }

            results.push(result);

            if (i + 1) % 50 == 0 {
                info!("Batch simulation progress: {}/{} calls completed", i + 1, count);
            }
        }

        // Calculate final statistics
        let end_time = Utc::now();
        stats.end_time = end_time;
        stats.duration_seconds = (end_time - start_time).num_milliseconds() as f64 / 1000.0;
        stats.success_rate = if stats.total_calls > 0 {
            (stats.successful_calls as f64 / stats.total_calls as f64) * 100.0
        } else {
            0.0
        };

        let total_processing_time: u64 = results.iter().map(|r| r.processing_time_ms).sum();
        stats.avg_processing_time_ms = if stats.total_calls > 0 {
            total_processing_time as f64 / stats.total_calls as f64
        } else {
            0.0
        };

        let total_cost: f64 = results.iter()
            .filter_map(|r| r.cost_per_minute)
            .sum();
        let cost_count = results.iter().filter(|r| r.cost_per_minute.is_some()).count();
        stats.avg_cost_per_minute = if cost_count > 0 {
            total_cost / cost_count as f64
        } else {
            0.0
        };

        // Export results if requested
        if let Some(output_path) = output_file {
            self.export_results_csv(&results, &output_path).await?;
            info!("Batch simulation results exported to: {}", output_path);
        }

        info!("Batch simulation completed: {:.1}% success rate, {:.2}ms avg processing time", 
              stats.success_rate, stats.avg_processing_time_ms);

        Ok(stats)
    }

    /// Select test pattern based on weights
    fn select_test_pattern(&self) -> &TestCallPattern {
        let total_weight: u32 = self.config.test_patterns.iter().map(|p| p.weight).sum();
        let random_weight = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() % total_weight as u128) as u32;

        let mut current_weight = 0;
        for pattern in &self.config.test_patterns {
            current_weight += pattern.weight;
            if random_weight < current_weight {
                return pattern;
            }
        }

        &self.config.test_patterns[0] // Fallback
    }

    /// Generate test numbers based on pattern
    fn generate_test_numbers(&self, pattern: &TestCallPattern) -> (String, String) {
        let destination = self.generate_number_from_pattern(&pattern.destination_pattern);
        let origination = if let Some(ref orig_pattern) = pattern.origination_pattern {
            self.generate_number_from_pattern(orig_pattern)
        } else {
            self.config.default_origination.clone()
        };

        (destination, origination)
    }

    /// Generate number from pattern (simplified)
    fn generate_number_from_pattern(&self, pattern: &str) -> String {
        // Replace wildcards with random digits
        let mut result = pattern.to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let mut counter = 0;
        while result.contains('X') {
            let digit = (now as u8 + counter) % 10;
            result = result.replacen('X', &digit.to_string(), 1);
            counter += 1;
        }

        result
    }

    /// Export results to CSV
    pub async fn export_results_csv(&self, results: &[CallSimulationResult], path: &str) -> Result<()> {
        let mut writer = Writer::from_path(path)?;

        // Write header
        writer.write_record(&[
            "call_id", "destination", "origination", "success", "selected_trunk",
            "routing_group", "jurisdiction", "processing_time_ms", "cost_per_minute",
            "quality_score", "error", "timestamp"
        ])?;

        // Write data
        for result in results {
            writer.write_record(&[
                &result.call_id,
                &result.destination,
                &result.origination,
                &result.success.to_string(),
                &result.selected_trunk.as_deref().unwrap_or(""),
                &result.routing_group.as_deref().unwrap_or(""),
                &result.jurisdiction.map(|j| j.description()).unwrap_or(""),
                &result.processing_time_ms.to_string(),
                &result.cost_per_minute.map(|c| c.to_string()).unwrap_or_default(),
                &result.quality_score.map(|q| q.to_string()).unwrap_or_default(),
                &result.error.as_deref().unwrap_or(""),
                &result.timestamp.to_rfc3339(),
            ])?;
        }

        writer.flush()?;
        Ok(())
    }

    /// Run load test
    pub async fn run_load_test(
        &self,
        duration_secs: u64,
        target_cps: u32,
        num_threads: u32,
        report_interval_secs: u64,
    ) -> Result<()> {
        info!("Starting load test: {}s duration, {} target CPS, {} threads", 
              duration_secs, target_cps, num_threads);

        let stats = Arc::new(LoadTestStats::new());
        let end_time = Instant::now() + Duration::from_secs(duration_secs);
        let calls_per_thread = target_cps / num_threads;

        // Spawn worker threads
        let mut handles = Vec::new();
        for thread_id in 0..num_threads {
            let stats_clone = Arc::clone(&stats);
            let simulator_clone = Arc::new(self);
            let config_clone = self.config.clone();
            
            let handle = tokio::spawn(async move {
                let mut interval_timer = interval(Duration::from_millis(1000 / calls_per_thread as u64));
                let mut call_counter = 0;

                while Instant::now() < end_time {
                    interval_timer.tick().await;

                    // Generate test call
                    let pattern = &config_clone.test_patterns[call_counter % config_clone.test_patterns.len()];
                    let (destination, origination) = simulator_clone.generate_test_numbers(pattern);
                    
                    let start = Instant::now();
                    let result = simulator_clone.simulate_call(
                        &destination,
                        &origination,
                        None,
                        &format!("load-test-{}", thread_id),
                        None,
                        None,
                    ).await;

                    let processing_time = start.elapsed().as_millis() as u64;
                    
                    match result {
                        Ok(sim_result) => {
                            stats_clone.record_call(sim_result.success, processing_time);
                        }
                        Err(_) => {
                            stats_clone.record_call(false, processing_time);
                        }
                    }

                    call_counter += 1;
                }
            });

            handles.push(handle);
        }

        // Spawn reporting thread
        let stats_reporter = Arc::clone(&stats);
        let report_handle = tokio::spawn(async move {
            let mut interval_timer = interval(Duration::from_secs(report_interval_secs));
            let mut last_calls = 0;

            while Instant::now() < end_time {
                interval_timer.tick().await;

                let (attempted, successful, failed, avg_time, _current_cps, peak_cps) = 
                    stats_reporter.get_summary();
                
                let current_cps = (attempted - last_calls) as f64 / report_interval_secs as f64;
                stats_reporter.update_cps(current_cps);
                
                info!("Load Test Status - Calls: {} ({}S, {}F), CPS: {:.1} (Peak: {:.1}), Avg Time: {:.1}ms",
                      attempted, successful, failed, current_cps, peak_cps, avg_time);

                last_calls = attempted;
            }
        });

        // Wait for all threads to complete
        for handle in handles {
            handle.await?;
        }
        report_handle.abort();

        let (attempted, successful, failed, avg_time, _current_cps, peak_cps) = stats.get_summary();
        let success_rate = if attempted > 0 {
            (successful as f64 / attempted as f64) * 100.0
        } else {
            0.0
        };

        info!("Load test completed - Total: {} calls, Success: {:.1}%, Peak CPS: {:.1}, Avg Time: {:.1}ms",
              attempted, success_rate, peak_cps, avg_time);

        Ok(())
    }

    /// Get mock trunk information
    pub fn get_mock_trunks(&self) -> &HashMap<String, Vec<TerminationTrunk>> {
        &self.mock_trunks
    }

    /// Get active simulations count
    pub fn get_active_simulations_count(&self) -> usize {
        self.active_simulations.lock().len()
    }
}

/// Create default test patterns for simulation
fn create_default_test_patterns() -> Vec<TestCallPattern> {
    vec![
        TestCallPattern {
            name: "US Interstate".to_string(),
            destination_pattern: "1212555XXXX".to_string(), // NYC
            origination_pattern: Some("1310555XXXX".to_string()), // LA
            expected_jurisdiction: Some(NanpaJurisdiction::Interstate),
            weight: 30,
        },
        TestCallPattern {
            name: "US Intrastate".to_string(),
            destination_pattern: "1212555XXXX".to_string(), // NYC
            origination_pattern: Some("1718555XXXX".to_string()), // NYC area
            expected_jurisdiction: Some(NanpaJurisdiction::Intrastate),
            weight: 25,
        },
        TestCallPattern {
            name: "Toll Free".to_string(),
            destination_pattern: "1800555XXXX".to_string(),
            origination_pattern: Some("1555123XXXX".to_string()),
            expected_jurisdiction: Some(NanpaJurisdiction::Indeterminate),
            weight: 15,
        },
        TestCallPattern {
            name: "International".to_string(),
            destination_pattern: "44207123XXXX".to_string(), // UK
            origination_pattern: Some("1555123XXXX".to_string()),
            expected_jurisdiction: Some(NanpaJurisdiction::International),
            weight: 20,
        },
        TestCallPattern {
            name: "Unknown ANI".to_string(),
            destination_pattern: "1555123XXXX".to_string(),
            origination_pattern: None, // Will use default
            expected_jurisdiction: Some(NanpaJurisdiction::Indeterminate),
            weight: 10,
        },
    ]
}