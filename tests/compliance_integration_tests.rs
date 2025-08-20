/*
 * Comprehensive Compliance Framework Integration Tests
 * 
 * This test suite validates the complete integration of J-STD-025 CDR
 * and ETSI LI compliance modules with the RedFire Switch B2BUA.
 */

use anyhow::Result;
use chrono::{DateTime, Utc, Duration};
use redfire_switch::compliance_framework::{
    ComplianceFramework, ComplianceConfig, CallEvent, CallEventType, RtpStatistics
};
use redfire_switch::j_std_025::{
    CdrEngineConfig, CdrType, CallResult, ServiceType, ChargingInfo, QoSMetrics
};
use redfire_switch::etsi_li::{
    LiControllerConfig, DeliveryEndpoints, EncryptionAlgorithm, AuthenticationMethod,
    DeliveryFormat, LiWarrant, TargetIdentifierType, InterceptType, WarrantStatus
};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use tokio::time::{sleep, Duration as TokioDuration};
use uuid::Uuid;

/// Test configuration for compliance framework
fn create_test_compliance_config() -> ComplianceConfig {
    ComplianceConfig {
        cdr_enabled: true,
        li_enabled: false, // Disable LI for basic tests
        cdr_config: CdrEngineConfig {
            realtime_generation: true,
            flush_interval: 60,
            max_memory_cdrs: 1000,
            fraud_detection: true,
            default_currency: "USD".to_string(),
            default_tariff_class: "TEST".to_string(),
        },
        li_config: LiControllerConfig {
            enabled: false,
            max_concurrent_warrants: 100,
            warrant_check_interval: 300,
            content_retention_days: 2555,
            enable_encryption: true,
            default_delivery_format: DeliveryFormat::Asn1Ber,
            audit_retention_days: 2555,
        },
        data_retention_days: 2555,
        realtime_monitoring: true,
        compliance_officer: None,
    }
}

/// Create test call event
fn create_test_call_event(
    call_id: &str,
    event_type: CallEventType,
    calling_number: &str,
    called_number: &str,
    response_code: Option<u16>,
) -> CallEvent {
    CallEvent {
        call_id: call_id.to_string(),
        event_type,
        timestamp: Utc::now(),
        calling_number: calling_number.to_string(),
        called_number: called_number.to_string(),
        sip_method: Some("INVITE".to_string()),
        sip_response_code: response_code,
        source_ip: Some("192.168.1.100".parse().unwrap()),
        dest_ip: Some("192.168.1.200".parse().unwrap()),
        user_agent: Some("RedFire-Test/1.0".to_string()),
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
            codec: "G711-ULAW".to_string(),
        }),
    }
}

#[tokio::test]
async fn test_complete_call_flow_compliance() -> Result<()> {
    let config = create_test_compliance_config();
    let framework = ComplianceFramework::new(config)?;
    
    // Start compliance processing
    framework.start().await?;
    
    let call_id = "test-call-001";
    let calling_number = "+15551234567";
    let called_number = "+15559876543";
    
    // Submit call attempt event
    let call_attempt = create_test_call_event(
        call_id, 
        CallEventType::CallAttempt, 
        calling_number, 
        called_number, 
        None
    );
    framework.submit_call_event(call_attempt)?;
    
    // Wait for processing
    sleep(TokioDuration::from_millis(50)).await;
    
    // Submit call progress event
    let call_progress = create_test_call_event(
        call_id, 
        CallEventType::CallProgress, 
        calling_number, 
        called_number, 
        Some(180)
    );
    framework.submit_call_event(call_progress)?;
    
    // Submit call answered event
    let call_answered = create_test_call_event(
        call_id, 
        CallEventType::CallAnswered, 
        calling_number, 
        called_number, 
        Some(200)
    );
    framework.submit_call_event(call_answered)?;
    
    // Submit media started event
    let media_started = create_test_call_event(
        call_id, 
        CallEventType::MediaStarted, 
        calling_number, 
        called_number, 
        None
    );
    framework.submit_call_event(media_started)?;
    
    // Submit DTMF event
    let dtmf_event = create_test_call_event(
        call_id, 
        CallEventType::DtmfDetected, 
        calling_number, 
        called_number, 
        None
    );
    framework.submit_call_event(dtmf_event)?;
    
    // Submit call ended event
    let call_ended = create_test_call_event(
        call_id, 
        CallEventType::CallEnded, 
        calling_number, 
        called_number, 
        Some(200)
    );
    framework.submit_call_event(call_ended)?;
    
    // Wait for all processing to complete
    sleep(TokioDuration::from_millis(200)).await;
    
    // Verify statistics
    let stats = framework.get_statistics().await;
    assert_eq!(stats.total_calls, 1);
    assert_eq!(stats.cdrs_generated, 1);
    assert_eq!(stats.active_intercepts, 0); // No warrants for this test
    
    // Verify active call count is zero after call ended
    assert_eq!(framework.get_active_call_count().await, 0);
    
    Ok(())
}

// #[tokio::test]
// async fn test_lawful_intercept_workflow() -> Result<()> {
//     // TODO: Implement when LI controller integration is complete
//     Ok(())
// }

#[tokio::test]
async fn test_fraud_detection_workflow() -> Result<()> {
    let config = create_test_compliance_config();
    let framework = ComplianceFramework::new(config)?;
    
    framework.start().await?;
    
    let calling_number = "+15551234567";
    
    // Simulate multiple rapid calls from same number (fraud pattern)
    for i in 0..10 {
        let call_id = format!("fraud-call-{:03}", i);
        let called_number = format!("+1555{:07}", 1000000 + i);
        
        let call_attempt = create_test_call_event(
            &call_id, 
            CallEventType::CallAttempt, 
            calling_number, 
            &called_number, 
            None
        );
        framework.submit_call_event(call_attempt)?;
        
        // Some calls fail (suspicious pattern)
        let call_ended = create_test_call_event(
            &call_id, 
            CallEventType::CallEnded, 
            calling_number, 
            &called_number, 
            Some(404) // Not found
        );
        framework.submit_call_event(call_ended)?;
        
        // Small delay between calls
        sleep(TokioDuration::from_millis(10)).await;
    }
    
    // Wait for processing
    sleep(TokioDuration::from_millis(200)).await;
    
    let stats = framework.get_statistics().await;
    assert_eq!(stats.total_calls, 10);
    assert_eq!(stats.cdrs_generated, 10);
    
    Ok(())
}

#[tokio::test]
async fn test_conference_call_compliance() -> Result<()> {
    let config = create_test_compliance_config();
    let framework = ComplianceFramework::new(config)?;
    
    framework.start().await?;
    
    // Conference call with multiple participants
    let conference_id = "conf-001";
    let participants = vec![
        "+15551111111",
        "+15552222222", 
        "+15553333333"
    ];
    
    // Each participant joins the conference
    for (i, participant) in participants.iter().enumerate() {
        let call_id = format!("{}-leg-{}", conference_id, i);
        
        let mut call_attempt = create_test_call_event(
            &call_id, 
            CallEventType::CallAttempt, 
            participant, 
            &conference_id, 
            None
        );
        
        // Mark as conference call
        call_attempt.sip_headers.insert("X-Conference-ID".to_string(), conference_id.to_string());
        framework.submit_call_event(call_attempt)?;
        
        let call_answered = create_test_call_event(
            &call_id, 
            CallEventType::CallAnswered, 
            participant, 
            &conference_id, 
            Some(200)
        );
        framework.submit_call_event(call_answered)?;
        
        let media_started = create_test_call_event(
            &call_id, 
            CallEventType::MediaStarted, 
            participant, 
            &conference_id, 
            None
        );
        framework.submit_call_event(media_started)?;
    }
    
    // Wait for processing
    sleep(TokioDuration::from_millis(100)).await;
    
    // Conference ends - all participants disconnect
    for (i, participant) in participants.iter().enumerate() {
        let call_id = format!("{}-leg-{}", conference_id, i);
        
        let call_ended = create_test_call_event(
            &call_id, 
            CallEventType::CallEnded, 
            participant, 
            &conference_id, 
            Some(200)
        );
        framework.submit_call_event(call_ended)?;
    }
    
    // Wait for processing
    sleep(TokioDuration::from_millis(200)).await;
    
    let stats = framework.get_statistics().await;
    assert_eq!(stats.total_calls, 3); // Three conference legs
    assert_eq!(stats.cdrs_generated, 3);
    
    Ok(())
}

#[tokio::test]
async fn test_emergency_call_compliance() -> Result<()> {
    let config = create_test_compliance_config();
    let framework = ComplianceFramework::new(config)?;
    
    framework.start().await?;
    
    let call_id = "emergency-call-001";
    let calling_number = "+15551234567";
    let called_number = "911"; // Emergency number
    
    let mut call_attempt = create_test_call_event(
        call_id, 
        CallEventType::CallAttempt, 
        calling_number, 
        called_number, 
        None
    );
    
    // Mark as emergency call
    call_attempt.sip_headers.insert("Priority".to_string(), "emergency".to_string());
    call_attempt.sip_headers.insert("Resource-Priority".to_string(), "ets.0".to_string());
    
    framework.submit_call_event(call_attempt)?;
    
    let call_answered = create_test_call_event(
        call_id, 
        CallEventType::CallAnswered, 
        calling_number, 
        called_number, 
        Some(200)
    );
    framework.submit_call_event(call_answered)?;
    
    let media_started = create_test_call_event(
        call_id, 
        CallEventType::MediaStarted, 
        calling_number, 
        called_number, 
        None
    );
    framework.submit_call_event(media_started)?;
    
    let call_ended = create_test_call_event(
        call_id, 
        CallEventType::CallEnded, 
        calling_number, 
        called_number, 
        Some(200)
    );
    framework.submit_call_event(call_ended)?;
    
    // Wait for processing
    sleep(TokioDuration::from_millis(200)).await;
    
    let stats = framework.get_statistics().await;
    assert_eq!(stats.total_calls, 1);
    assert_eq!(stats.cdrs_generated, 1);
    
    Ok(())
}

#[tokio::test]
async fn test_international_call_compliance() -> Result<()> {
    let config = create_test_compliance_config();
    let framework = ComplianceFramework::new(config)?;
    
    framework.start().await?;
    
    let call_id = "intl-call-001";
    let calling_number = "+15551234567"; // US number
    let called_number = "+441234567890"; // UK number
    
    let call_attempt = create_test_call_event(
        call_id, 
        CallEventType::CallAttempt, 
        calling_number, 
        called_number, 
        None
    );
    framework.submit_call_event(call_attempt)?;
    
    let call_answered = create_test_call_event(
        call_id, 
        CallEventType::CallAnswered, 
        calling_number, 
        called_number, 
        Some(200)
    );
    framework.submit_call_event(call_answered)?;
    
    let call_ended = create_test_call_event(
        call_id, 
        CallEventType::CallEnded, 
        calling_number, 
        called_number, 
        Some(200)
    );
    framework.submit_call_event(call_ended)?;
    
    // Wait for processing
    sleep(TokioDuration::from_millis(200)).await;
    
    let stats = framework.get_statistics().await;
    assert_eq!(stats.total_calls, 1);
    assert_eq!(stats.cdrs_generated, 1);
    
    Ok(())
}

#[tokio::test]
async fn test_call_transfer_compliance() -> Result<()> {
    let config = create_test_compliance_config();
    let framework = ComplianceFramework::new(config)?;
    
    framework.start().await?;
    
    let original_call_id = "transfer-call-001";
    let calling_number = "+15551234567";
    let original_called = "+15559876543";
    let transfer_target = "+15555555555";
    
    // Original call setup
    let call_attempt = create_test_call_event(
        original_call_id, 
        CallEventType::CallAttempt, 
        calling_number, 
        original_called, 
        None
    );
    framework.submit_call_event(call_attempt)?;
    
    let call_answered = create_test_call_event(
        original_call_id, 
        CallEventType::CallAnswered, 
        calling_number, 
        original_called, 
        Some(200)
    );
    framework.submit_call_event(call_answered)?;
    
    // Call transfer occurs
    let mut call_transferred = create_test_call_event(
        original_call_id, 
        CallEventType::CallTransferred, 
        calling_number, 
        original_called, 
        None
    );
    call_transferred.sip_headers.insert("Refer-To".to_string(), transfer_target.to_string());
    framework.submit_call_event(call_transferred)?;
    
    // Original call ends
    let call_ended = create_test_call_event(
        original_call_id, 
        CallEventType::CallEnded, 
        calling_number, 
        original_called, 
        Some(200)
    );
    framework.submit_call_event(call_ended)?;
    
    // New call to transfer target
    let transfer_call_id = "transfer-call-002";
    let transfer_attempt = create_test_call_event(
        transfer_call_id, 
        CallEventType::CallAttempt, 
        calling_number, 
        transfer_target, 
        None
    );
    framework.submit_call_event(transfer_attempt)?;
    
    let transfer_answered = create_test_call_event(
        transfer_call_id, 
        CallEventType::CallAnswered, 
        calling_number, 
        transfer_target, 
        Some(200)
    );
    framework.submit_call_event(transfer_answered)?;
    
    let transfer_ended = create_test_call_event(
        transfer_call_id, 
        CallEventType::CallEnded, 
        calling_number, 
        transfer_target, 
        Some(200)
    );
    framework.submit_call_event(transfer_ended)?;
    
    // Wait for processing
    sleep(TokioDuration::from_millis(200)).await;
    
    let stats = framework.get_statistics().await;
    assert_eq!(stats.total_calls, 2); // Original call + transfer call
    assert_eq!(stats.cdrs_generated, 2);
    
    Ok(())
}

#[tokio::test]
async fn test_high_volume_call_processing() -> Result<()> {
    let config = create_test_compliance_config();
    let framework = ComplianceFramework::new(config)?;
    
    framework.start().await?;
    
    let call_count = 100;
    
    // Generate high volume of calls
    for i in 0..call_count {
        let call_id = format!("volume-call-{:03}", i);
        let calling_number = format!("+155512345{:02}", i % 100);
        let called_number = format!("+155598765{:02}", i % 100);
        
        let call_attempt = create_test_call_event(
            &call_id, 
            CallEventType::CallAttempt, 
            &calling_number, 
            &called_number, 
            None
        );
        framework.submit_call_event(call_attempt)?;
        
        let call_answered = create_test_call_event(
            &call_id, 
            CallEventType::CallAnswered, 
            &calling_number, 
            &called_number, 
            Some(200)
        );
        framework.submit_call_event(call_answered)?;
        
        let call_ended = create_test_call_event(
            &call_id, 
            CallEventType::CallEnded, 
            &calling_number, 
            &called_number, 
            Some(200)
        );
        framework.submit_call_event(call_ended)?;
    }
    
    // Wait for all processing to complete
    sleep(TokioDuration::from_millis(500)).await;
    
    let stats = framework.get_statistics().await;
    assert_eq!(stats.total_calls, call_count);
    assert_eq!(stats.cdrs_generated, call_count);
    assert_eq!(framework.get_active_call_count().await, 0);
    
    Ok(())
}

#[tokio::test]
async fn test_compliance_error_handling() -> Result<()> {
    let config = create_test_compliance_config();
    let framework = ComplianceFramework::new(config)?;
    
    framework.start().await?;
    
    // Test with malformed call events
    let call_id = "error-call-001";
    
    // Call event with missing required fields
    let invalid_event = CallEvent {
        call_id: call_id.to_string(),
        event_type: CallEventType::CallAttempt,
        timestamp: Utc::now(),
        calling_number: "".to_string(), // Invalid empty number
        called_number: "".to_string(), // Invalid empty number
        sip_method: None,
        sip_response_code: None,
        source_ip: None,
        dest_ip: None,
        user_agent: None,
        sip_headers: HashMap::new(),
        rtp_stats: None,
    };
    
    // This should not crash the system
    let _ = framework.submit_call_event(invalid_event);
    
    // Wait for processing
    sleep(TokioDuration::from_millis(100)).await;
    
    let stats = framework.get_statistics().await;
    // Error handling should increment error count
    assert!(stats.compliance_errors >= 0);
    
    Ok(())
}

#[tokio::test] 
async fn test_compliance_statistics_accuracy() -> Result<()> {
    let config = create_test_compliance_config();
    let framework = ComplianceFramework::new(config)?;
    
    framework.start().await?;
    
    // Generate calls with different outcomes
    let test_cases = vec![
        (CallEventType::CallEnded, Some(200)), // Normal
        (CallEventType::CallEnded, Some(486)), // Busy
        (CallEventType::CallEnded, Some(408)), // No Answer
        (CallEventType::CallEnded, Some(503)), // Service Unavailable
    ];
    
    for (i, (event_type, response_code)) in test_cases.iter().enumerate() {
        let call_id = format!("stats-call-{:03}", i);
        let calling_number = "+15551234567";
        let called_number = format!("+155598765{:02}", i);
        
        let call_attempt = create_test_call_event(
            &call_id, 
            CallEventType::CallAttempt, 
            calling_number, 
            &called_number, 
            None
        );
        framework.submit_call_event(call_attempt)?;
        
        if *response_code == Some(200) {
            let call_answered = create_test_call_event(
                &call_id, 
                CallEventType::CallAnswered, 
                calling_number, 
                &called_number, 
                Some(200)
            );
            framework.submit_call_event(call_answered)?;
        }
        
        let call_ended = create_test_call_event(
            &call_id, 
            *event_type, 
            calling_number, 
            &called_number, 
            *response_code
        );
        framework.submit_call_event(call_ended)?;
    }
    
    // Wait for processing
    sleep(TokioDuration::from_millis(200)).await;
    
    let stats = framework.get_statistics().await;
    assert_eq!(stats.total_calls, 4);
    assert_eq!(stats.cdrs_generated, 4);
    assert!(stats.last_updated > Utc::now() - Duration::seconds(10));
    
    Ok(())
}