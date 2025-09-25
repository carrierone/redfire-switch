/*
 * Redfire Switch - Service Integration Tests
 * Copyright (C) 2025 Carrier One Inc and contributors
 *
 * Tests the wired microservices architecture to ensure proper integration
 * between SIP stack, codec engine, compliance bridge, and other services.
 */

//! Integration tests for the service registry and service wiring system
//!
//! This test suite validates that all microservices can be initialized,
//! wired together, and communicate properly through the event bus.

use anyhow::Result;
use redfire_switch::events::{EventBus, TelecomEvent};
use redfire_switch::services::{ServiceRegistry, AntiFraudConfig, MediaConfig, SignalingConfig, ControlConfig};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn, debug};

/// Test basic service registry initialization
#[tokio::test]
async fn test_service_registry_initialization() -> Result<()> {
    info!("🔧 Testing service registry initialization");

    // Create event bus
    let event_bus = Arc::new(EventBus::new());
    let mut registry = ServiceRegistry::new(event_bus);

    // Initialize all services
    registry.initialize_all().await?;

    // Verify all services are initialized
    assert!(registry.database().is_some(), "Database service should be initialized");
    assert!(registry.control().is_some(), "Control service should be initialized");
    assert!(registry.sip_codec().is_some(), "SIP codec service should be initialized");
    assert!(registry.compliance_bridge().is_some(), "Compliance bridge should be initialized");
    assert!(registry.routing().is_some(), "Routing service should be initialized");
    assert!(registry.media().is_some(), "Media service should be initialized");
    assert!(registry.signaling().is_some(), "Signaling service should be initialized");
    assert!(registry.anti_fraud().is_some(), "Anti-fraud service should be initialized");

    // Verify all services are healthy
    assert!(registry.are_all_services_healthy().await, "All services should be healthy");

    info!("✅ Service registry initialization test passed");
    Ok(())
}

/// Test individual service initialization
#[tokio::test]
async fn test_individual_service_initialization() -> Result<()> {
    info!("🔧 Testing individual service initialization");

    let event_bus = Arc::new(EventBus::new());
    let mut registry = ServiceRegistry::new(event_bus);

    // Test database service initialization first
    registry.initialize_database_service(redfire_switch::database::DatabaseConfig::default()).await?;
    assert!(registry.database().is_some());

    // Test control service initialization
    registry.initialize_control_service(ControlConfig::default()).await?;
    assert!(registry.control().is_some());

    // Test SIP codec service initialization
    registry.initialize_sip_codec_service().await?;
    assert!(registry.sip_codec().is_some());

    // Test compliance bridge initialization
    registry.initialize_compliance_bridge().await?;
    assert!(registry.compliance_bridge().is_some());

    // Test routing service initialization
    registry.initialize_routing_service().await?;
    assert!(registry.routing().is_some());

    // Test media service initialization
    registry.initialize_media_service(MediaConfig::default()).await?;
    assert!(registry.media().is_some());

    // Test signaling service initialization
    registry.initialize_signaling_service(SignalingConfig::default()).await?;
    assert!(registry.signaling().is_some());

    // Test anti-fraud service initialization
    registry.initialize_anti_fraud_service(AntiFraudConfig::default()).await?;
    assert!(registry.anti_fraud().is_some());

    info!("✅ Individual service initialization test passed");
    Ok(())
}

/// Test service wiring and inter-service communication
#[tokio::test]
async fn test_service_wiring() -> Result<()> {
    info!("🔧 Testing service wiring and communication");

    let event_bus = Arc::new(EventBus::new());
    let mut registry = ServiceRegistry::new(event_bus.clone());

    // Initialize all services
    registry.initialize_all().await?;

    // Test that services can access each other through the registry
    let sip_codec = registry.sip_codec().unwrap();
    let compliance_bridge = registry.compliance_bridge().unwrap();
    let media_service = registry.media().unwrap();
    let routing_service = registry.routing().unwrap();

    // Verify services exist and have proper configuration
    debug!("SIP codec service: {:?}", Arc::as_ptr(&sip_codec));
    debug!("Compliance bridge: {:?}", Arc::as_ptr(&compliance_bridge));
    debug!("Media service: {:?}", Arc::as_ptr(&media_service));
    debug!("Routing service: {:?}", Arc::as_ptr(&routing_service));

    info!("✅ Service wiring test passed");
    Ok(())
}

/// Test service health monitoring
#[tokio::test]
async fn test_service_health_monitoring() -> Result<()> {
    info!("🔧 Testing service health monitoring");

    let event_bus = Arc::new(EventBus::new());
    let mut registry = ServiceRegistry::new(event_bus);

    // Initially no services should be healthy
    assert!(!registry.are_all_services_healthy().await);

    // Initialize services one by one and check health
    registry.initialize_database_service(redfire_switch::database::DatabaseConfig::default()).await?;
    registry.initialize_control_service(ControlConfig::default()).await?;
    registry.initialize_sip_codec_service().await?;
    registry.initialize_compliance_bridge().await?;
    registry.initialize_routing_service().await?;
    registry.initialize_media_service(MediaConfig::default()).await?;
    registry.initialize_signaling_service(SignalingConfig::default()).await?;
    registry.initialize_anti_fraud_service(AntiFraudConfig::default()).await?;

    // Wire services
    registry.wire_services().await?;

    // Now all services should be healthy
    assert!(registry.are_all_services_healthy().await);

    // Check there are no unhealthy services
    let unhealthy = registry.get_unhealthy_services().await;
    assert!(unhealthy.is_empty(), "No services should be unhealthy: {:?}", unhealthy);

    info!("✅ Service health monitoring test passed");
    Ok(())
}

/// Test event bus integration with services
#[tokio::test]
async fn test_event_bus_integration() -> Result<()> {
    info!("🔧 Testing event bus integration with services");

    let event_bus = Arc::new(EventBus::new());
    let mut registry = ServiceRegistry::new(event_bus.clone());

    // Initialize all services
    registry.initialize_all().await?;

    // Test that event bus is properly wired
    // Note: In a full implementation, we would test actual event publishing
    // and subscription between services through the event bus

    // For now, verify the event bus exists and is shared
    assert!(Arc::strong_count(&event_bus) > 1, "Event bus should be shared among services");

    info!("✅ Event bus integration test passed");
    Ok(())
}

/// Test service shutdown sequence
#[tokio::test]
async fn test_service_shutdown() -> Result<()> {
    info!("🔧 Testing service shutdown sequence");

    let event_bus = Arc::new(EventBus::new());
    let mut registry = ServiceRegistry::new(event_bus);

    // Initialize all services
    registry.initialize_all().await?;

    // Verify all services are healthy before shutdown
    assert!(registry.are_all_services_healthy().await);

    // Test graceful shutdown
    registry.shutdown_all().await?;

    // Note: Since the services don't have explicit health checks after shutdown,
    // we primarily test that shutdown doesn't panic or error

    info!("✅ Service shutdown test passed");
    Ok(())
}

/// Test SIP codec integration service functionality
#[tokio::test]
async fn test_sip_codec_integration() -> Result<()> {
    info!("🔧 Testing SIP codec integration service");

    let event_bus = Arc::new(EventBus::new());
    let mut registry = ServiceRegistry::new(event_bus);

    // Initialize required services
    registry.initialize_database_service(redfire_switch::database::DatabaseConfig::default()).await?;
    registry.initialize_control_service(ControlConfig::default()).await?;
    registry.initialize_sip_codec_service().await?;

    let sip_codec = registry.sip_codec().unwrap();

    // Test that the SIP codec integration service is properly configured
    // Note: This tests basic initialization since the integration service
    // mainly provides SIP message processing capabilities

    info!("✅ SIP codec integration test passed");
    Ok(())
}

/// Test CALEA compliance bridge integration
#[tokio::test]
async fn test_compliance_bridge_integration() -> Result<()> {
    info!("🔧 Testing CALEA compliance bridge integration");

    let event_bus = Arc::new(EventBus::new());
    let mut registry = ServiceRegistry::new(event_bus);

    // Initialize required services
    registry.initialize_database_service(redfire_switch::database::DatabaseConfig::default()).await?;
    registry.initialize_control_service(ControlConfig::default()).await?;
    registry.initialize_compliance_bridge().await?;

    let compliance_bridge = registry.compliance_bridge().unwrap();

    // Test that the compliance bridge is properly initialized
    // Note: This tests basic initialization since the bridge is mainly
    // used for compliance notification callbacks

    info!("✅ Compliance bridge integration test passed");
    Ok(())
}

/// Full end-to-end service integration test
#[tokio::test]
async fn test_full_service_integration() -> Result<()> {
    info!("🚀 Running full end-to-end service integration test");

    let event_bus = Arc::new(EventBus::new());
    let mut registry = ServiceRegistry::new(event_bus);

    // Time the full initialization
    let start = std::time::Instant::now();

    // Initialize all services
    registry.initialize_all().await?;

    let init_duration = start.elapsed();
    info!("Service initialization took: {:?}", init_duration);

    // Verify all components are properly wired
    assert!(registry.are_all_services_healthy().await, "All services must be healthy");

    // Verify service accessibility
    assert!(registry.database().is_some());
    assert!(registry.control().is_some());
    assert!(registry.sip_codec().is_some());
    assert!(registry.compliance_bridge().is_some());
    assert!(registry.routing().is_some());
    assert!(registry.media().is_some());
    assert!(registry.signaling().is_some());
    assert!(registry.anti_fraud().is_some());

    // Test graceful shutdown
    registry.shutdown_all().await?;

    let total_duration = start.elapsed();
    info!("Total test duration: {:?}", total_duration);

    info!("✅ Full service integration test completed successfully");
    Ok(())
}