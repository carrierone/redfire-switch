/*
 * Compliance Framework Performance and Stress Tests
 *
 * These tests validate the performance characteristics and resilience
 * of the compliance framework under high load conditions.
 */

use anyhow::Result;
use chrono::{DateTime, Utc};
use redfire_switch::compliance_framework::{
    CallEvent, CallEventType, ComplianceConfig, ComplianceFramework, RtpStatistics,
};
use redfire_switch::etsi_li::{
    AuthenticationMethod, DeliveryEndpoints, DeliveryFormat, EncryptionAlgorithm,
    LiControllerConfig,
};
use redfire_switch::j_std_025::{CallResult, CdrEngineConfig, CdrType};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::{sleep, timeout};

/// Performance test configuration
struct PerformanceTestConfig {
    concurrent_calls: usize,
    calls_per_second: u64,
    test_duration_seconds: u64,
    enable_li: bool,
    enable_detailed_stats: bool,
}

/// Performance metrics collector
#[derive(Debug, Clone)]
struct PerformanceMetrics {
    calls_processed: AtomicU64,
    events_processed: AtomicU64,
    processing_errors: AtomicU64,
    average_processing_time_ns: AtomicU64,
    peak_memory_usage: AtomicU64,
    start_time: Instant,
}

impl PerformanceMetrics {
    fn new() -> Self {
        Self {
            calls_processed: AtomicU64::new(0),
            events_processed: AtomicU64::new(0),
            processing_errors: AtomicU64::new(0),
            average_processing_time_ns: AtomicU64::new(0),
            peak_memory_usage: AtomicU64::new(0),
            start_time: Instant::now(),
        }
    }

    fn record_call_processed(&self) {
        self.calls_processed.fetch_add(1, Ordering::Relaxed);
    }

    fn record_event_processed(&self, processing_time_ns: u64) {
        self.events_processed.fetch_add(1, Ordering::Relaxed);

        // Update running average
        let current_avg = self.average_processing_time_ns.load(Ordering::Relaxed);
        let events = self.events_processed.load(Ordering::Relaxed);
        let new_avg = (current_avg * (events - 1) + processing_time_ns) / events;
        self.average_processing_time_ns
            .store(new_avg, Ordering::Relaxed);
    }

    fn record_error(&self) {
        self.processing_errors.fetch_add(1, Ordering::Relaxed);
    }

    fn get_summary(&self) -> PerformanceSummary {
        let elapsed = self.start_time.elapsed();
        let calls = self.calls_processed.load(Ordering::Relaxed);
        let events = self.events_processed.load(Ordering::Relaxed);
        let errors = self.processing_errors.load(Ordering::Relaxed);
        let avg_time_ns = self.average_processing_time_ns.load(Ordering::Relaxed);

        PerformanceSummary {
            total_calls: calls,
            total_events: events,
            total_errors: errors,
            calls_per_second: (calls as f64) / elapsed.as_secs_f64(),
            events_per_second: (events as f64) / elapsed.as_secs_f64(),
            average_processing_time_us: avg_time_ns as f64 / 1000.0,
            test_duration: elapsed,
            error_rate: (errors as f64) / (events as f64) * 100.0,
        }
    }
}

#[derive(Debug)]
struct PerformanceSummary {
    total_calls: u64,
    total_events: u64,
    total_errors: u64,
    calls_per_second: f64,
    events_per_second: f64,
    average_processing_time_us: f64,
    test_duration: Duration,
    error_rate: f64,
}

/// Create performance test configuration
fn create_performance_config(enable_li: bool) -> ComplianceConfig {
    ComplianceConfig {
        cdr_enabled: true,
        li_enabled: enable_li,
        cdr_config: CdrEngineConfig {
            realtime_generation: true,
            flush_interval: 30,
            max_memory_cdrs: 50000,
            fraud_detection: false, // Disable for performance testing
            default_currency: "USD".to_string(),
            default_tariff_class: "PERF".to_string(),
        },
        li_config: if enable_li {
            LiControllerConfig {
                enabled: true,
                delivery_endpoints: DeliveryEndpoints {
                    hi2_endpoint: Some("127.0.0.1:9001".parse().unwrap()),
                    hi3_endpoint: Some("127.0.0.1:9002".parse().unwrap()),
                    encryption_algorithm: EncryptionAlgorithm::Aes256Gcm,
                    tls_certificate_path: "/tmp/perf_cert.pem".to_string(),
                    tls_private_key_path: "/tmp/perf_key.pem".to_string(),
                    auth_method: AuthenticationMethod::MutualTls,
                    delivery_format: DeliveryFormat::Asn1Ber,
                },
                audit_log_path: "/tmp/perf_li_audit.log".to_string(),
                warrant_storage_path: "/tmp/perf_warrants/".to_string(),
                compliance_officer_contact: "perf@example.com".to_string(),
                retention_days: 2555,
                emergency_contact: None,
            }
        } else {
            LiControllerConfig::default()
        },
        data_retention_days: 2555,
        realtime_monitoring: false, // Disable for performance
        compliance_officer: None,
    }
}

/// Generate test call event with performance optimizations
fn create_perf_call_event(
    call_id: u64,
    event_type: CallEventType,
    calling_base: u64,
    called_base: u64,
) -> CallEvent {
    CallEvent {
        call_id: format!("perf-{:010}", call_id),
        event_type,
        timestamp: Utc::now(),
        calling_number: format!("+1555{:07}", calling_base),
        called_number: format!("+1666{:07}", called_base),
        sip_method: Some("INVITE".to_string()),
        sip_response_code: match event_type {
            CallEventType::CallAnswered => Some(200),
            CallEventType::CallEnded => Some(200),
            CallEventType::CallProgress => Some(180),
            _ => None,
        },
        source_ip: Some("192.168.1.100".parse().unwrap()),
        dest_ip: Some("192.168.1.200".parse().unwrap()),
        user_agent: Some("RedFire-Perf/1.0".to_string()),
        sip_headers: HashMap::new(),
        rtp_stats: Some(RtpStatistics {
            packets_sent: 1000,
            packets_received: 995,
            bytes_sent: 160000,
            bytes_received: 159200,
            packets_lost: 5,
            jitter: 12.5,
            rtt: 45.2,
            mos_score: Some(4.2),
            codec: "G711".to_string(),
        }),
    }
}

#[tokio::test]
async fn test_high_throughput_cdr_generation() -> Result<()> {
    let config = create_performance_config(false); // CDR only
    let framework = ComplianceFramework::new(config)?;
    let metrics = Arc::new(PerformanceMetrics::new());

    framework.start().await?;

    let test_config = PerformanceTestConfig {
        concurrent_calls: 1000,
        calls_per_second: 500,
        test_duration_seconds: 10,
        enable_li: false,
        enable_detailed_stats: false,
    };

    println!("Starting high-throughput CDR generation test...");
    println!(
        "Target: {} CPS for {} seconds",
        test_config.calls_per_second, test_config.test_duration_seconds
    );

    let semaphore = Arc::new(Semaphore::new(test_config.concurrent_calls));
    let mut join_set = JoinSet::new();
    let start_time = Instant::now();

    let mut call_id = 0u64;
    while start_time.elapsed().as_secs() < test_config.test_duration_seconds {
        let permit = semaphore.clone().acquire_owned().await?;
        let framework_clone = Arc::new(framework);
        let metrics_clone = metrics.clone();
        let current_call_id = call_id;
        call_id += 1;

        join_set.spawn(async move {
            let _permit = permit; // Keep permit alive

            let call_start = Instant::now();

            // Simulate complete call flow
            let calling_base = (current_call_id % 1000000) + 1000000;
            let called_base = (current_call_id % 1000000) + 2000000;

            // Call attempt
            let event1 = create_perf_call_event(
                current_call_id,
                CallEventType::CallAttempt,
                calling_base,
                called_base,
            );
            let process_start = Instant::now();
            if framework_clone.submit_call_event(event1).is_ok() {
                metrics_clone.record_event_processed(process_start.elapsed().as_nanos() as u64);
            } else {
                metrics_clone.record_error();
            }

            // Call answered
            let event2 = create_perf_call_event(
                current_call_id,
                CallEventType::CallAnswered,
                calling_base,
                called_base,
            );
            let process_start = Instant::now();
            if framework_clone.submit_call_event(event2).is_ok() {
                metrics_clone.record_event_processed(process_start.elapsed().as_nanos() as u64);
            } else {
                metrics_clone.record_error();
            }

            // Call ended
            let event3 = create_perf_call_event(
                current_call_id,
                CallEventType::CallEnded,
                calling_base,
                called_base,
            );
            let process_start = Instant::now();
            if framework_clone.submit_call_event(event3).is_ok() {
                metrics_clone.record_event_processed(process_start.elapsed().as_nanos() as u64);
                metrics_clone.record_call_processed();
            } else {
                metrics_clone.record_error();
            }
        });

        // Rate limiting
        let target_interval = Duration::from_millis(1000 / test_config.calls_per_second);
        let elapsed = call_start.elapsed();
        if elapsed < target_interval {
            sleep(target_interval - elapsed).await;
        }
    }

    // Wait for all tasks to complete
    while let Some(result) = join_set.join_next().await {
        result?;
    }

    // Allow time for processing
    sleep(Duration::from_secs(2)).await;

    let summary = metrics.get_summary();
    let framework_stats = framework.get_statistics().await;

    println!("\n=== Performance Test Results ===");
    println!("Total calls processed: {}", summary.total_calls);
    println!("Total events processed: {}", summary.total_events);
    println!("Calls per second: {:.2}", summary.calls_per_second);
    println!("Events per second: {:.2}", summary.events_per_second);
    println!(
        "Average processing time: {:.2} μs",
        summary.average_processing_time_us
    );
    println!("Error rate: {:.2}%", summary.error_rate);
    println!(
        "Framework CDRs generated: {}",
        framework_stats.cdrs_generated
    );

    // Performance assertions
    assert!(
        summary.calls_per_second > 100.0,
        "Call processing rate too low"
    );
    assert!(
        summary.error_rate < 1.0,
        "Error rate too high: {:.2}%",
        summary.error_rate
    );
    assert!(
        summary.average_processing_time_us < 1000.0,
        "Processing time too high"
    );

    Ok(())
}

#[tokio::test]
async fn test_concurrent_call_processing() -> Result<()> {
    let config = create_performance_config(false);
    let framework = Arc::new(ComplianceFramework::new(config)?);
    let metrics = Arc::new(PerformanceMetrics::new());

    framework.start().await?;

    let concurrent_calls = 500;
    let mut join_set = JoinSet::new();

    println!(
        "Starting concurrent call processing test with {} calls...",
        concurrent_calls
    );

    // Launch concurrent calls
    for call_id in 0..concurrent_calls {
        let framework_clone = framework.clone();
        let metrics_clone = metrics.clone();

        join_set.spawn(async move {
            let calling_base = (call_id % 1000000) + 1000000;
            let called_base = (call_id % 1000000) + 2000000;

            // Full call lifecycle
            let events = vec![
                (CallEventType::CallAttempt, None),
                (CallEventType::CallProgress, Some(180)),
                (CallEventType::CallAnswered, Some(200)),
                (CallEventType::MediaStarted, None),
                (CallEventType::CallEnded, Some(200)),
            ];

            for (event_type, response_code) in events {
                let mut event =
                    create_perf_call_event(call_id as u64, event_type, calling_base, called_base);
                event.sip_response_code = response_code;

                let process_start = Instant::now();
                if framework_clone.submit_call_event(event).is_ok() {
                    metrics_clone.record_event_processed(process_start.elapsed().as_nanos() as u64);
                } else {
                    metrics_clone.record_error();
                }

                // Small delay between events
                sleep(Duration::from_millis(1)).await;
            }

            metrics_clone.record_call_processed();
        });
    }

    // Wait for all calls to complete
    while let Some(result) = join_set.join_next().await {
        result?;
    }

    // Allow processing time
    sleep(Duration::from_secs(3)).await;

    let summary = metrics.get_summary();
    let framework_stats = framework.get_statistics().await;

    println!("\n=== Concurrent Processing Results ===");
    println!("Concurrent calls: {}", concurrent_calls);
    println!("Total events: {}", summary.total_events);
    println!(
        "Processing time: {:.2} seconds",
        summary.test_duration.as_secs_f64()
    );
    println!("Events per second: {:.2}", summary.events_per_second);
    println!(
        "Average event processing time: {:.2} μs",
        summary.average_processing_time_us
    );
    println!(
        "Framework CDRs generated: {}",
        framework_stats.cdrs_generated
    );

    assert_eq!(summary.total_calls, concurrent_calls as u64);
    assert_eq!(framework_stats.cdrs_generated, concurrent_calls as u64);
    assert!(summary.error_rate < 0.1, "Error rate too high");

    Ok(())
}

#[tokio::test]
async fn test_memory_usage_under_load() -> Result<()> {
    let config = create_performance_config(false);
    let framework = ComplianceFramework::new(config)?;

    framework.start().await?;

    println!("Starting memory usage test...");

    let initial_active_calls = framework.get_active_call_count().await;
    assert_eq!(initial_active_calls, 0);

    // Create many long-running calls
    let long_running_calls = 1000;
    for call_id in 0..long_running_calls {
        let calling_base = (call_id % 1000000) + 1000000;
        let called_base = (call_id % 1000000) + 2000000;

        // Start call but don't end it
        let event = create_perf_call_event(
            call_id as u64,
            CallEventType::CallAttempt,
            calling_base,
            called_base,
        );
        framework.submit_call_event(event)?;

        let answered_event = create_perf_call_event(
            call_id as u64,
            CallEventType::CallAnswered,
            calling_base,
            called_base,
        );
        framework.submit_call_event(answered_event)?;
    }

    // Allow processing
    sleep(Duration::from_secs(2)).await;

    let active_calls_after_setup = framework.get_active_call_count().await;
    println!("Active calls after setup: {}", active_calls_after_setup);
    assert_eq!(active_calls_after_setup, long_running_calls);

    // Now end all calls
    for call_id in 0..long_running_calls {
        let calling_base = (call_id % 1000000) + 1000000;
        let called_base = (call_id % 1000000) + 2000000;

        let event = create_perf_call_event(
            call_id as u64,
            CallEventType::CallEnded,
            calling_base,
            called_base,
        );
        framework.submit_call_event(event)?;
    }

    // Allow processing
    sleep(Duration::from_secs(3)).await;

    let final_active_calls = framework.get_active_call_count().await;
    let final_stats = framework.get_statistics().await;

    println!("Final active calls: {}", final_active_calls);
    println!("Total CDRs generated: {}", final_stats.cdrs_generated);

    assert_eq!(
        final_active_calls, 0,
        "Memory leak: calls not properly cleaned up"
    );
    assert_eq!(final_stats.cdrs_generated, long_running_calls as u64);

    Ok(())
}

#[tokio::test]
async fn test_stress_with_li_enabled() -> Result<()> {
    let config = create_performance_config(true); // Enable LI
    let framework = ComplianceFramework::new(config)?;
    let metrics = Arc::new(PerformanceMetrics::new());

    framework.start().await?;

    println!("Starting stress test with LI enabled...");

    let stress_calls = 200; // Lower due to LI overhead
    let mut join_set = JoinSet::new();

    for call_id in 0..stress_calls {
        let framework_clone = Arc::new(framework);
        let metrics_clone = metrics.clone();

        join_set.spawn(async move {
            let calling_base = (call_id % 1000000) + 1000000;
            let called_base = (call_id % 1000000) + 2000000;

            // Complete call flow with all event types
            let events = vec![
                CallEventType::CallAttempt,
                CallEventType::CallProgress,
                CallEventType::CallAnswered,
                CallEventType::MediaStarted,
                CallEventType::DtmfDetected,
                CallEventType::CallEnded,
            ];

            for event_type in events {
                let event =
                    create_perf_call_event(call_id as u64, event_type, calling_base, called_base);

                let process_start = Instant::now();
                if framework_clone.submit_call_event(event).is_ok() {
                    metrics_clone.record_event_processed(process_start.elapsed().as_nanos() as u64);
                } else {
                    metrics_clone.record_error();
                }

                // Realistic timing between events
                sleep(Duration::from_millis(10)).await;
            }

            metrics_clone.record_call_processed();
        });
    }

    // Wait for completion
    while let Some(result) = join_set.join_next().await {
        result?;
    }

    sleep(Duration::from_secs(3)).await;

    let summary = metrics.get_summary();
    let framework_stats = framework.get_statistics().await;

    println!("\n=== Stress Test with LI Results ===");
    println!("Calls processed: {}", summary.total_calls);
    println!("Events processed: {}", summary.total_events);
    println!("Events per second: {:.2}", summary.events_per_second);
    println!(
        "Average processing time: {:.2} μs",
        summary.average_processing_time_us
    );
    println!("LI events captured: {}", framework_stats.li_events_captured);
    println!("CDRs generated: {}", framework_stats.cdrs_generated);

    assert_eq!(summary.total_calls, stress_calls as u64);
    assert!(summary.error_rate < 1.0, "Error rate too high with LI");
    assert!(
        summary.average_processing_time_us < 2000.0,
        "Processing too slow with LI"
    );

    Ok(())
}

#[tokio::test]
async fn test_error_recovery_resilience() -> Result<()> {
    let config = create_performance_config(false);
    let framework = ComplianceFramework::new(config)?;
    let metrics = Arc::new(PerformanceMetrics::new());

    framework.start().await?;

    println!("Starting error recovery resilience test...");

    // Mix of valid and invalid events
    let total_events = 500;
    let error_injection_rate = 0.1; // 10% error rate

    for i in 0..total_events {
        let call_id = i;
        let calling_base = 1000000 + (i % 100000);
        let called_base = 2000000 + (i % 100000);

        // Inject errors randomly
        let inject_error = (i as f64 / total_events as f64) < error_injection_rate;

        let mut event = create_perf_call_event(
            call_id as u64,
            CallEventType::CallAttempt,
            calling_base,
            called_base,
        );

        if inject_error {
            // Create invalid event
            event.calling_number = "".to_string(); // Invalid
            event.called_number = "invalid_number".to_string(); // Invalid
        }

        let process_start = Instant::now();
        if framework.submit_call_event(event).is_ok() {
            metrics.record_event_processed(process_start.elapsed().as_nanos() as u64);
        } else {
            metrics.record_error();
        }

        // Also submit corresponding end event for valid calls
        if !inject_error {
            let end_event = create_perf_call_event(
                call_id as u64,
                CallEventType::CallEnded,
                calling_base,
                called_base,
            );
            let _ = framework.submit_call_event(end_event);
        }
    }

    sleep(Duration::from_secs(2)).await;

    let summary = metrics.get_summary();
    let framework_stats = framework.get_statistics().await;

    println!("\n=== Error Recovery Test Results ===");
    println!("Total events submitted: {}", total_events);
    println!("Events processed: {}", summary.total_events);
    println!("Processing errors: {}", summary.total_errors);
    println!("Error rate: {:.2}%", summary.error_rate);
    println!(
        "Framework compliance errors: {}",
        framework_stats.compliance_errors
    );

    // System should remain stable despite errors
    assert!(
        framework_stats.compliance_errors < (total_events as u64) / 2,
        "Too many compliance errors"
    );
    assert!(
        summary.events_per_second > 0.0,
        "System became unresponsive"
    );

    Ok(())
}

#[tokio::test]
async fn test_long_running_stability() -> Result<()> {
    let config = create_performance_config(false);
    let framework = ComplianceFramework::new(config)?;

    framework.start().await?;

    println!("Starting long-running stability test (30 seconds)...");

    let test_duration = Duration::from_secs(30);
    let call_interval = Duration::from_millis(100); // 10 CPS
    let start_time = Instant::now();

    let mut call_id = 0u64;
    let mut total_calls = 0u64;

    while start_time.elapsed() < test_duration {
        let calling_base = 1000000 + (call_id % 100000);
        let called_base = 2000000 + (call_id % 100000);

        // Quick call lifecycle
        let events = vec![
            CallEventType::CallAttempt,
            CallEventType::CallAnswered,
            CallEventType::CallEnded,
        ];

        for event_type in events {
            let event = create_perf_call_event(call_id, event_type, calling_base, called_base);
            let _ = framework.submit_call_event(event);
        }

        call_id += 1;
        total_calls += 1;

        sleep(call_interval).await;
    }

    // Allow final processing
    sleep(Duration::from_secs(3)).await;

    let final_stats = framework.get_statistics().await;
    let active_calls = framework.get_active_call_count().await;

    println!("\n=== Long-Running Stability Results ===");
    println!(
        "Test duration: {:.1} seconds",
        start_time.elapsed().as_secs_f64()
    );
    println!("Calls submitted: {}", total_calls);
    println!("CDRs generated: {}", final_stats.cdrs_generated);
    println!("Active calls remaining: {}", active_calls);
    println!("Compliance errors: {}", final_stats.compliance_errors);

    // Stability checks
    assert_eq!(
        active_calls, 0,
        "Calls not properly cleaned up after long run"
    );
    assert_eq!(
        final_stats.cdrs_generated, total_calls,
        "CDR count mismatch"
    );
    assert!(
        final_stats.compliance_errors < total_calls / 10,
        "Too many errors during long run"
    );

    Ok(())
}
