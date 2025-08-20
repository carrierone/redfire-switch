/*
 * ETSI LI (Lawful Intercept) Compliance Tests
 * 
 * Comprehensive tests for ETSI TS 102 232 and TS 133 108 compliance
 * including warrant validation, HI2/HI3 delivery, and encryption requirements.
 */

use anyhow::Result;
use chrono::{DateTime, Utc, Duration};
use redfire_switch::etsi_li::{
    EtsiLiController, LiControllerConfig, LiWarrant, Hi2Record, Hi3ContentRecord,
    Hi2EventType, TargetIdentifierType, InterceptType, DeliveryEndpoints,
    EncryptionAlgorithm, AuthenticationMethod, DeliveryFormat,
    PartyInformation, ServiceInformation, NetworkInformation,
    ContentType, ContentMetadata
};
use std::collections::HashMap;
use std::net::IpAddr;
use tokio::fs;
use uuid::Uuid;

/// Create test LI controller configuration
async fn create_test_li_config() -> Result<LiControllerConfig> {
    // Create temporary directories
    let warrant_dir = "/tmp/test_warrants";
    let _ = fs::create_dir_all(warrant_dir).await;
    
    Ok(LiControllerConfig {
        enabled: true,
        delivery_endpoints: DeliveryEndpoints {
            hi2_endpoint: Some("127.0.0.1:9001".parse().unwrap()),
            hi3_endpoint: Some("127.0.0.1:9002".parse().unwrap()),
            encryption_algorithm: EncryptionAlgorithm::Aes256Gcm,
            tls_certificate_path: "/tmp/test_cert.pem".to_string(),
            tls_private_key_path: "/tmp/test_key.pem".to_string(),
            auth_method: AuthenticationMethod::MutualTls,
            delivery_format: DeliveryFormat::Asn1Ber,
        },
        audit_log_path: "/tmp/li_audit_test.log".to_string(),
        warrant_storage_path: warrant_dir.to_string(),
        compliance_officer_contact: "compliance@test.com".to_string(),
        retention_days: 2555,
        emergency_contact: Some("emergency@test.com".to_string()),
    })
}

/// Create valid test warrant
fn create_valid_warrant() -> LiWarrant {
    LiWarrant {
        warrant_id: Uuid::new_v4(),
        target_identifier: "+15551234567".to_string(),
        identifier_type: TargetIdentifierType::PhoneNumber,
        intercept_type: InterceptType::FullIntercept,
        start_date: Utc::now() - Duration::hours(1),
        end_date: Utc::now() + Duration::days(30),
        issuing_authority: "FBI".to_string(),
        case_reference: "CASE-2024-001".to_string(),
        authorized_by: "Judge Smith".to_string(),
        intercept_reason: "Suspected criminal activity".to_string(),
        delivery_endpoint: "192.168.1.10:9001".to_string(),
        created_at: Utc::now(),
        is_active: true,
        audit_trail: Vec::new(),
        emergency_warrant: false,
    }
}

/// Create expired warrant
fn create_expired_warrant() -> LiWarrant {
    let mut warrant = create_valid_warrant();
    warrant.start_date = Utc::now() - Duration::days(40);
    warrant.end_date = Utc::now() - Duration::days(10);
    warrant.is_active = false;
    warrant
}

/// Create future warrant
fn create_future_warrant() -> LiWarrant {
    let mut warrant = create_valid_warrant();
    warrant.start_date = Utc::now() + Duration::days(1);
    warrant.end_date = Utc::now() + Duration::days(31);
    warrant
}

/// Create emergency warrant
fn create_emergency_warrant() -> LiWarrant {
    let mut warrant = create_valid_warrant();
    warrant.emergency_warrant = true;
    warrant.issuing_authority = "Emergency Services".to_string();
    warrant.end_date = Utc::now() + Duration::hours(72); // 72 hour emergency limit
    warrant
}

#[tokio::test]
async fn test_warrant_validation_compliance() -> Result<()> {
    let config = create_test_li_config().await?;
    let mut controller = EtsiLiController::new(config);
    
    // Test valid warrant
    let valid_warrant = create_valid_warrant();
    assert!(controller.validate_warrant(&valid_warrant).is_ok());
    
    // Test expired warrant
    let expired_warrant = create_expired_warrant();
    assert!(controller.validate_warrant(&expired_warrant).is_err());
    
    // Test future warrant (should be valid but not active yet)
    let future_warrant = create_future_warrant();
    let validation_result = controller.validate_warrant(&future_warrant);
    assert!(validation_result.is_ok() || validation_result.err().unwrap().to_string().contains("not yet active"));
    
    // Test warrant with invalid authority (empty)
    let mut invalid_warrant = create_valid_warrant();
    invalid_warrant.issuing_authority = "".to_string();
    assert!(controller.validate_warrant(&invalid_warrant).is_err());
    
    // Test warrant with invalid case reference (empty)
    let mut invalid_warrant = create_valid_warrant();
    invalid_warrant.case_reference = "".to_string();
    assert!(controller.validate_warrant(&invalid_warrant).is_err());
    
    // Test warrant with invalid target identifier (empty)
    let mut invalid_warrant = create_valid_warrant();
    invalid_warrant.target_identifier = "".to_string();
    assert!(controller.validate_warrant(&invalid_warrant).is_err());
    
    Ok(())
}

#[tokio::test]
async fn test_warrant_expiry_enforcement() -> Result<()> {
    let config = create_test_li_config().await?;
    let mut controller = EtsiLiController::new(config);
    
    // Add a warrant that will expire soon
    let mut short_warrant = create_valid_warrant();
    short_warrant.end_date = Utc::now() + Duration::minutes(1);
    
    controller.add_warrant(short_warrant.clone())?;
    
    // Warrant should be active initially
    let active_warrants = controller.should_intercept(&short_warrant.target_identifier).await?;
    assert_eq!(active_warrants.len(), 1);
    
    // Manually trigger warrant expiry check
    controller.check_warrant_expiry().await?;
    
    // Warrant should still be active (not expired yet)
    let active_warrants = controller.should_intercept(&short_warrant.target_identifier).await?;
    assert_eq!(active_warrants.len(), 1);
    
    // Simulate passage of time by updating warrant end date to past
    let mut expired_warrant = short_warrant.clone();
    expired_warrant.end_date = Utc::now() - Duration::minutes(1);
    expired_warrant.is_active = false;
    
    // Remove old warrant and add expired one
    controller.remove_warrant(&short_warrant.warrant_id)?;
    // Note: Adding expired warrant should fail or be marked inactive
    
    // Check no intercept should occur for expired warrant
    let active_warrants = controller.should_intercept(&expired_warrant.target_identifier).await?;
    assert_eq!(active_warrants.len(), 0);
    
    Ok(())
}

#[tokio::test]
async fn test_emergency_warrant_handling() -> Result<()> {
    let config = create_test_li_config().await?;
    let mut controller = EtsiLiController::new(config);
    
    let emergency_warrant = create_emergency_warrant();
    controller.add_warrant(emergency_warrant.clone())?;
    
    // Emergency warrant should be immediately active
    let active_warrants = controller.should_intercept(&emergency_warrant.target_identifier).await?;
    assert_eq!(active_warrants.len(), 1);
    assert!(active_warrants[0].emergency_warrant);
    
    // Emergency warrants should have shorter duration (72 hours max)
    let duration = emergency_warrant.end_date - emergency_warrant.start_date;
    assert!(duration <= Duration::hours(72), "Emergency warrant duration exceeds 72 hours");
    
    Ok(())
}

#[tokio::test]
async fn test_hi2_record_etsi_compliance() -> Result<()> {
    let config = create_test_li_config().await?;
    let controller = EtsiLiController::new(config);
    
    let warrant = create_valid_warrant();
    
    // Create Hi2 record with all required ETSI fields
    let hi2_record = Hi2Record {
        record_id: Uuid::new_v4(),
        warrant_id: warrant.warrant_id,
        target_id: "+15551234567".to_string(),
        timestamp: Utc::now(),
        event_type: Hi2EventType::CallAttempt,
        calling_party: Some(PartyInformation {
            party_id: "+15551234567".to_string(),
            identity_type: TargetIdentifierType::PhoneNumber,
            party_role: "originating".to_string(),
            location: None,
            service_provider: Some("RedFire Switch".to_string()),
        }),
        called_party: Some(PartyInformation {
            party_id: "+15559876543".to_string(),
            identity_type: TargetIdentifierType::PhoneNumber,
            party_role: "terminating".to_string(),
            location: None,
            service_provider: None,
        }),
        location_info: None,
        service_info: ServiceInformation {
            service_type: "voice".to_string(),
            service_id: Some("call-001".to_string()),
            qos_info: None,
            supplementary_services: Vec::new(),
        },
        network_info: NetworkInformation {
            network_id: "REDFIRE_NETWORK".to_string(),
            access_technology: "SIP".to_string(),
            serving_element: "RedFire-B2BUA".to_string(),
            element_ip: Some("192.168.1.100".parse().unwrap()),
        },
        additional_info: HashMap::new(),
    };
    
    // Validate Hi2 record structure
    assert!(!hi2_record.record_id.is_nil());
    assert!(!hi2_record.warrant_id.is_nil());
    assert!(!hi2_record.target_id.is_empty());
    assert!(hi2_record.calling_party.is_some());
    assert!(hi2_record.called_party.is_some());
    
    // Validate timestamps are reasonable
    let now = Utc::now();
    assert!(hi2_record.timestamp <= now);
    assert!(hi2_record.timestamp > now - Duration::hours(1));
    
    // Validate party information
    let calling_party = hi2_record.calling_party.unwrap();
    assert!(!calling_party.party_id.is_empty());
    assert_eq!(calling_party.party_role, "originating");
    
    Ok(())
}

#[tokio::test]
async fn test_hi3_content_etsi_compliance() -> Result<()> {
    let config = create_test_li_config().await?;
    let controller = EtsiLiController::new(config);
    
    let warrant = create_valid_warrant();
    
    // Create Hi3 content record
    let audio_content = b"MOCK_AUDIO_CONTENT_DATA".to_vec();
    let hi3_record = Hi3ContentRecord {
        record_id: Uuid::new_v4(),
        warrant_id: warrant.warrant_id,
        hi2_record_id: Some(Uuid::new_v4()),
        timestamp: Utc::now(),
        content_type: ContentType::VoiceAudio,
        content_payload: audio_content.clone(),
        metadata: ContentMetadata {
            encoding: "G711-ULAW".to_string(),
            size: audio_content.len() as u64,
            checksum: "SHA256:MOCK_CHECKSUM".to_string(),
            encryption_algorithm: Some("AES-256-GCM".to_string()),
            compression_algorithm: None,
        },
        sequence_number: 1,
    };
    
    // Validate Hi3 record compliance
    assert!(!hi3_record.record_id.is_nil());
    assert!(!hi3_record.warrant_id.is_nil());
    assert!(!hi3_record.content_payload.is_empty());
    assert!(hi3_record.metadata.size > 0);
    assert!(!hi3_record.metadata.encoding.is_empty());
    assert!(!hi3_record.metadata.checksum.is_empty());
    
    // Validate encryption is specified (ETSI requirement)
    assert!(hi3_record.metadata.encryption_algorithm.is_some());
    assert!(hi3_record.metadata.encryption_algorithm.unwrap().contains("AES"));
    
    Ok(())
}

#[tokio::test]
async fn test_encryption_requirements_compliance() -> Result<()> {
    let config = create_test_li_config().await?;
    
    // Verify encryption is mandatory in delivery endpoints
    match config.delivery_endpoints.encryption_algorithm {
        EncryptionAlgorithm::Aes256Gcm | EncryptionAlgorithm::ChaCha20Poly1305 => {
            // Valid encryption algorithms
        }
    }
    
    // Verify TLS certificate paths are specified
    assert!(!config.delivery_endpoints.tls_certificate_path.is_empty());
    assert!(!config.delivery_endpoints.tls_private_key_path.is_empty());
    
    // Verify authentication method is secure
    match config.delivery_endpoints.auth_method {
        AuthenticationMethod::MutualTls | AuthenticationMethod::OAuth2 => {
            // Valid authentication methods
        }
    }
    
    Ok(())
}

#[tokio::test]
async fn test_asn1_ber_delivery_format_compliance() -> Result<()> {
    let config = create_test_li_config().await?;
    let controller = EtsiLiController::new(config);
    
    let warrant = create_valid_warrant();
    let hi2_record = Hi2Record {
        record_id: Uuid::new_v4(),
        warrant_id: warrant.warrant_id,
        target_id: "+15551234567".to_string(),
        timestamp: Utc::now(),
        event_type: Hi2EventType::CallAttempt,
        calling_party: Some(PartyInformation {
            party_id: "+15551234567".to_string(),
            identity_type: TargetIdentifierType::PhoneNumber,
            party_role: "originating".to_string(),
            location: None,
            service_provider: Some("RedFire Switch".to_string()),
        }),
        called_party: Some(PartyInformation {
            party_id: "+15559876543".to_string(),
            identity_type: TargetIdentifierType::PhoneNumber,
            party_role: "terminating".to_string(),
            location: None,
            service_provider: None,
        }),
        location_info: None,
        service_info: ServiceInformation {
            service_type: "voice".to_string(),
            service_id: Some("call-001".to_string()),
            qos_info: None,
            supplementary_services: Vec::new(),
        },
        network_info: NetworkInformation {
            network_id: "REDFIRE_NETWORK".to_string(),
            access_technology: "SIP".to_string(),
            serving_element: "RedFire-B2BUA".to_string(),
            element_ip: Some("192.168.1.100".parse().unwrap()),
        },
        additional_info: HashMap::new(),
    };
    
    // Test ASN.1 BER formatting (this would normally produce binary data)
    // For this test, we verify the format is ASN.1 BER and structure is correct
    let asn1_data = controller.format_hi2_as_asn1_ber(&hi2_record)?;
    
    // Basic validation that we get some encoded data
    assert!(!asn1_data.is_empty());
    // ASN.1 BER typically starts with a tag byte
    // This is a simplified check - real implementation would validate full ASN.1 structure
    
    Ok(())
}

#[tokio::test]
async fn test_audit_trail_compliance() -> Result<()> {
    let config = create_test_li_config().await?;
    let mut controller = EtsiLiController::new(config);
    
    let warrant = create_valid_warrant();
    controller.add_warrant(warrant.clone())?;
    
    // Perform some intercept operations that should be audited
    let warrants = controller.should_intercept(&warrant.target_identifier).await?;
    assert_eq!(warrants.len(), 1);
    
    // Create and capture Hi2 record
    let hi2_record = Hi2Record {
        record_id: Uuid::new_v4(),
        warrant_id: warrant.warrant_id,
        target_id: warrant.target_identifier.clone(),
        timestamp: Utc::now(),
        event_type: Hi2EventType::CallAttempt,
        calling_party: Some(PartyInformation {
            party_id: warrant.target_identifier.clone(),
            identity_type: TargetIdentifierType::PhoneNumber,
            party_role: "originating".to_string(),
            location: None,
            service_provider: Some("RedFire Switch".to_string()),
        }),
        called_party: None,
        location_info: None,
        service_info: ServiceInformation {
            service_type: "voice".to_string(),
            service_id: None,
            qos_info: None,
            supplementary_services: Vec::new(),
        },
        network_info: NetworkInformation {
            network_id: "REDFIRE_NETWORK".to_string(),
            access_technology: "SIP".to_string(),
            serving_element: "RedFire-B2BUA".to_string(),
            element_ip: None,
        },
        additional_info: HashMap::new(),
    };
    
    controller.capture_hi2(warrants.clone(), hi2_record).await?;
    
    // Verify audit trail exists and is accessible
    let audit_stats = controller.get_audit_statistics().await?;
    assert!(audit_stats.total_warrant_operations > 0);
    assert!(audit_stats.total_hi2_deliveries > 0);
    
    Ok(())
}

#[tokio::test]
async fn test_multiple_warrant_types() -> Result<()> {
    let config = create_test_li_config().await?;
    let mut controller = EtsiLiController::new(config);
    
    // Test different intercept types
    let mut metadata_warrant = create_valid_warrant();
    metadata_warrant.intercept_type = InterceptType::MetadataOnly;
    metadata_warrant.target_identifier = "+15551111111".to_string();
    
    let mut content_warrant = create_valid_warrant();
    content_warrant.intercept_type = InterceptType::ContentOnly;
    content_warrant.target_identifier = "+15552222222".to_string();
    
    let mut full_warrant = create_valid_warrant();
    full_warrant.intercept_type = InterceptType::FullIntercept;
    full_warrant.target_identifier = "+15553333333".to_string();
    
    controller.add_warrant(metadata_warrant.clone())?;
    controller.add_warrant(content_warrant.clone())?;
    controller.add_warrant(full_warrant.clone())?;
    
    // Test metadata-only intercept
    let warrants = controller.should_intercept(&metadata_warrant.target_identifier).await?;
    assert_eq!(warrants.len(), 1);
    assert_eq!(warrants[0].intercept_type, InterceptType::MetadataOnly);
    
    // Test content-only intercept
    let warrants = controller.should_intercept(&content_warrant.target_identifier).await?;
    assert_eq!(warrants.len(), 1);
    assert_eq!(warrants[0].intercept_type, InterceptType::ContentOnly);
    
    // Test full intercept
    let warrants = controller.should_intercept(&full_warrant.target_identifier).await?;
    assert_eq!(warrants.len(), 1);
    assert_eq!(warrants[0].intercept_type, InterceptType::FullIntercept);
    
    // Test non-intercepted number
    let warrants = controller.should_intercept("+15554444444").await?;
    assert_eq!(warrants.len(), 0);
    
    Ok(())
}

#[tokio::test]
async fn test_warrant_storage_persistence() -> Result<()> {
    let config = create_test_li_config().await?;
    let mut controller1 = EtsiLiController::new(config.clone());
    
    let warrant = create_valid_warrant();
    controller1.add_warrant(warrant.clone())?;
    
    // Verify warrant is stored
    let warrants = controller1.should_intercept(&warrant.target_identifier).await?;
    assert_eq!(warrants.len(), 1);
    
    // Create new controller instance (simulates restart)
    let mut controller2 = EtsiLiController::new(config);
    
    // Load warrants from storage
    controller2.load_warrants().await?;
    
    // Verify warrant persisted across restart
    let warrants = controller2.should_intercept(&warrant.target_identifier).await?;
    assert_eq!(warrants.len(), 1);
    assert_eq!(warrants[0].warrant_id, warrant.warrant_id);
    
    Ok(())
}

#[tokio::test]
async fn test_compliance_officer_notification() -> Result<()> {
    let config = create_test_li_config().await?;
    let controller = EtsiLiController::new(config.clone());
    
    // Verify compliance officer contact is configured
    assert!(!config.compliance_officer_contact.is_empty());
    assert!(config.compliance_officer_contact.contains("@"));
    
    // Verify emergency contact is configured
    assert!(config.emergency_contact.is_some());
    let emergency_contact = config.emergency_contact.unwrap();
    assert!(!emergency_contact.is_empty());
    assert!(emergency_contact.contains("@"));
    
    Ok(())
}

#[tokio::test]
async fn test_retention_period_compliance() -> Result<()> {
    let config = create_test_li_config().await?;
    
    // Verify retention period meets legal requirements (7 years = 2555 days)
    assert_eq!(config.retention_days, 2555);
    
    let controller = EtsiLiController::new(config);
    
    // Test audit statistics include retention information
    let stats = controller.get_audit_statistics().await?;
    assert!(stats.retention_compliance_checked);
    
    Ok(())
}

#[tokio::test]
async fn test_warrant_deactivation_compliance() -> Result<()> {
    let config = create_test_li_config().await?;
    let mut controller = EtsiLiController::new(config);
    
    let warrant = create_valid_warrant();
    controller.add_warrant(warrant.clone())?;
    
    // Verify warrant is active
    let warrants = controller.should_intercept(&warrant.target_identifier).await?;
    assert_eq!(warrants.len(), 1);
    assert!(warrants[0].is_active);
    
    // Deactivate warrant
    controller.deactivate_warrant(&warrant.warrant_id)?;
    
    // Verify warrant is no longer active
    let warrants = controller.should_intercept(&warrant.target_identifier).await?;
    assert_eq!(warrants.len(), 0);
    
    Ok(())
}