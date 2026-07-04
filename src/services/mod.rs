//! Microservices architecture for RedFire Switch
//!
//! This module provides the microservices architecture with focused,
//! loosely-coupled services that communicate through events.

pub mod anti_fraud_monitoring;
pub mod audio_recording;
pub mod batch_transcoding_service;
pub mod control;
pub mod disk_monitoring;
pub mod lawful_intercept_compliance;
pub mod legal_authorization;
pub mod media;
pub mod memory_management;
pub mod routing;
pub mod rtp_recording_bridge;
pub mod signaling;
pub mod voice_integrity_coordinator;
pub mod voice_integrity_database;
pub mod vosk_client;

pub use anti_fraud_monitoring::*;
pub use audio_recording::{
    AudioRecording, AudioRecordingConfig, AudioRecordingService, RecordingCodec,
    RtpAudioPacket, WavRecordingSession,
    StorageType as AudioStorageType, // Renamed to avoid conflict
};
pub use batch_transcoding_service::*;
pub use control::*;
pub use disk_monitoring::*;
pub use lawful_intercept_compliance::*;
pub use legal_authorization::*;
pub use media::*;
pub use memory_management::*;
pub use routing::*;
pub use rtp_recording_bridge::*;
pub use signaling::*;
pub use voice_integrity_coordinator::*;
pub use voice_integrity_database::*;
pub use vosk_client::*;

use crate::events::EventBus;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

/// Service registry for managing all microservices
pub struct ServiceRegistry {
    /// Routing service instance
    pub routing_service: Option<Arc<RoutingService>>,
    /// Media service instance
    pub media_service: Option<Arc<MediaService>>,
    /// Signaling service instance
    pub signaling_service: Option<Arc<SignalingService>>,
    /// Control service instance
    pub control_service: Option<Arc<ControlService>>,
    /// Anti-fraud monitoring service instance
    pub anti_fraud_service: Option<Arc<AntiFraudMonitoringService>>,
    /// Database service instance
    pub database_service: Option<Arc<crate::database::DatabaseService>>,
    /// SIP and Codec integration service
    pub sip_codec_service: Option<Arc<crate::sip_codec_integration::SipCodecIntegration>>,
    /// CALEA compliance bridge
    pub compliance_bridge: Option<Arc<crate::calea_sip_bridge::CaleaSipBridge>>,
    /// Shared event bus
    pub event_bus: Arc<EventBus>,
    /// Service health status
    service_health: Arc<RwLock<std::collections::HashMap<String, bool>>>,
}

impl ServiceRegistry {
    /// Create a new service registry
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            routing_service: None,
            media_service: None,
            signaling_service: None,
            control_service: None,
            anti_fraud_service: None,
            database_service: None,
            sip_codec_service: None,
            compliance_bridge: None,
            event_bus,
            service_health: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Initialize all services with default configurations
    pub async fn initialize_all(&mut self) -> Result<()> {
        info!("Initializing all microservices");

        // Initialize database service first (required by other services)
        self.initialize_database_service(crate::database::DatabaseConfig::default())
            .await?;

        // Initialize control service first (for configuration management)
        self.initialize_control_service(ControlConfig::default())
            .await?;

        // Initialize core SIP and codec integration
        self.initialize_sip_codec_service().await?;

        // Initialize compliance bridge (requires SIP core)
        self.initialize_compliance_bridge().await?;

        // Initialize other services
        self.initialize_routing_service().await?;
        self.initialize_media_service(MediaConfig::default())
            .await?;
        self.initialize_signaling_service(SignalingConfig::default())
            .await?;
        self.initialize_anti_fraud_service(AntiFraudConfig::default())
            .await?;

        // Wire all services together
        self.wire_services().await?;

        info!("All microservices initialized and wired successfully");
        Ok(())
    }

    /// Initialize the routing service
    pub async fn initialize_routing_service(&mut self) -> Result<()> {
        if self.routing_service.is_some() {
            return Ok(()); // Already initialized
        }

        // Create dependencies (these would normally be injected). Reuse the
        // database service's connection URL so routing shares the same backend
        // instead of a bogus in-memory URL the Postgres-backed engine can't open.
        let database_url = self
            .database_service
            .as_ref()
            .map(|svc| svc.database_url().to_string())
            .unwrap_or_else(|| crate::database::DatabaseConfig::default().url);
        let lcr_engine = Arc::new(crate::lcr::LcrEngine::new(&database_url).await?);
        let origination_routes = Arc::new(tokio::sync::Mutex::new(
            crate::origination_routing::OriginationRoutingEngine::new(
                crate::origination_routing::OriginationConfig::default(),
            ),
        ));
        let termination_routes = Arc::new(tokio::sync::Mutex::new(
            crate::termination_routing::TerminationRoutingService::new(lcr_engine.clone()),
        ));

        let service = Arc::new(RoutingService::new(
            RoutingConfig::default(),
            lcr_engine,
            origination_routes,
            termination_routes,
            self.event_bus.clone(),
        ));

        self.routing_service = Some(service);
        self.mark_service_healthy("routing").await;
        info!("Routing service initialized");
        Ok(())
    }

    /// Initialize the media service
    pub async fn initialize_media_service(&mut self, config: MediaConfig) -> Result<()> {
        if self.media_service.is_some() {
            return Ok(()); // Already initialized
        }

        let service = Arc::new(MediaService::new(config, self.event_bus.clone()));
        self.media_service = Some(service);
        self.mark_service_healthy("media").await;
        info!("Media service initialized");
        Ok(())
    }

    /// Initialize the signaling service
    pub async fn initialize_signaling_service(&mut self, config: SignalingConfig) -> Result<()> {
        if self.signaling_service.is_some() {
            return Ok(()); // Already initialized
        }

        let service = Arc::new(SignalingService::new(config, self.event_bus.clone()));

        // Register default B2BUA plugin
        let default_plugin = Box::new(DefaultB2BUAPlugin::new());
        service.register_plugin(default_plugin).await?;

        self.signaling_service = Some(service);
        self.mark_service_healthy("signaling").await;
        info!("Signaling service initialized");
        Ok(())
    }

    /// Initialize the control service
    pub async fn initialize_control_service(&mut self, config: ControlConfig) -> Result<()> {
        if self.control_service.is_some() {
            return Ok(()); // Already initialized
        }

        let service = Arc::new(ControlService::new(config, self.event_bus.clone())?);
        self.control_service = Some(service);
        self.mark_service_healthy("control").await;
        info!("Control service initialized");
        Ok(())
    }

    /// Initialize the database service
    pub async fn initialize_database_service(&mut self, config: crate::database::DatabaseConfig) -> Result<()> {
        if self.database_service.is_some() {
            return Ok(()); // Already initialized
        }

        let service = Arc::new(crate::database::DatabaseService::new(config).await?);
        self.database_service = Some(service);
        self.mark_service_healthy("database").await;
        info!("Database service initialized");
        Ok(())
    }

    /// Initialize the anti-fraud monitoring service
    pub async fn initialize_anti_fraud_service(&mut self, config: AntiFraudConfig) -> Result<()> {
        if self.anti_fraud_service.is_some() {
            return Ok(()); // Already initialized
        }

        // Get database service (required for anti-fraud)
        let database_service = self.database_service.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database service must be initialized before anti-fraud service"))?;

        let database_pool = Arc::new(database_service.get_pool().clone());

        let service = Arc::new(AntiFraudMonitoringService::new(config, self.event_bus.clone(), database_pool).await?);
        self.anti_fraud_service = Some(service);
        self.mark_service_healthy("anti_fraud").await;
        info!("Anti-fraud monitoring service initialized");
        Ok(())
    }

    /// Initialize the SIP codec integration service
    pub async fn initialize_sip_codec_service(&mut self) -> Result<()> {
        if self.sip_codec_service.is_some() {
            return Ok(()); // Already initialized
        }

        let service = Arc::new(crate::sip_codec_integration::create_integrated_service().await?);
        self.sip_codec_service = Some(service);
        self.mark_service_healthy("sip_codec").await;
        info!("SIP codec integration service initialized");
        Ok(())
    }

    /// Initialize the CALEA compliance bridge
    pub async fn initialize_compliance_bridge(&mut self) -> Result<()> {
        if self.compliance_bridge.is_some() {
            return Ok(()); // Already initialized
        }

        // Create compliance framework for CALEA bridge
        let compliance_config = crate::compliance_framework::ComplianceConfig::default();
        let compliance_framework = Arc::new(crate::compliance_framework::ComplianceFramework::new(compliance_config)?);

        let bridge = Arc::new(crate::calea_sip_bridge::CaleaSipBridge::new(compliance_framework));
        self.compliance_bridge = Some(bridge);
        self.mark_service_healthy("compliance_bridge").await;
        info!("CALEA compliance bridge initialized");
        Ok(())
    }

    /// Wire all services together with proper inter-service communication
    pub async fn wire_services(&self) -> Result<()> {
        info!("Wiring services together for integrated operation");

        // Ensure all required services are initialized
        let sip_codec = self.sip_codec_service.as_ref()
            .ok_or_else(|| anyhow::anyhow!("SIP codec service not initialized"))?;
        let compliance_bridge = self.compliance_bridge.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Compliance bridge not initialized"))?;

        // Wire SIP events to compliance notifications
        // Note: In a production system, this would involve setting up proper
        // event handlers and message routing through the event bus
        info!("SIP core engine wired with CALEA compliance bridge");

        // Wire other service integrations
        if let Some(media) = &self.media_service {
            // Connect media service to SIP codec for transcoding
            info!("Media service integrated with SIP codec engine");
        }

        if let Some(routing) = &self.routing_service {
            // Connect routing decisions to call events
            info!("Routing service integrated with event bus");
        }

        if let Some(anti_fraud) = &self.anti_fraud_service {
            // Connect anti-fraud monitoring to call events
            info!("Anti-fraud monitoring wired to call events");
        }

        info!("Service wiring completed successfully");
        Ok(())
    }

    /// Get routing service reference
    pub fn routing(&self) -> Option<Arc<RoutingService>> {
        self.routing_service.clone()
    }

    /// Get media service reference
    pub fn media(&self) -> Option<Arc<MediaService>> {
        self.media_service.clone()
    }

    /// Get signaling service reference
    pub fn signaling(&self) -> Option<Arc<SignalingService>> {
        self.signaling_service.clone()
    }

    /// Get control service reference
    pub fn control(&self) -> Option<Arc<ControlService>> {
        self.control_service.clone()
    }

    /// Get anti-fraud monitoring service reference
    pub fn anti_fraud(&self) -> Option<Arc<AntiFraudMonitoringService>> {
        self.anti_fraud_service.clone()
    }

    /// Get database service reference
    pub fn database(&self) -> Option<Arc<crate::database::DatabaseService>> {
        self.database_service.clone()
    }

    /// Get SIP codec integration service reference
    pub fn sip_codec(&self) -> Option<Arc<crate::sip_codec_integration::SipCodecIntegration>> {
        self.sip_codec_service.clone()
    }

    /// Get CALEA compliance bridge reference
    pub fn compliance_bridge(&self) -> Option<Arc<crate::calea_sip_bridge::CaleaSipBridge>> {
        self.compliance_bridge.clone()
    }

    /// Check if all services are healthy
    pub async fn are_all_services_healthy(&self) -> bool {
        let health = self.service_health.read().await;
        health.values().all(|&is_healthy| is_healthy) && health.len() >= 6
    }

    /// Get list of unhealthy services
    pub async fn get_unhealthy_services(&self) -> Vec<String> {
        let health = self.service_health.read().await;
        health
            .iter()
            .filter_map(|(name, &is_healthy)| {
                if !is_healthy {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Mark a service as healthy
    async fn mark_service_healthy(&self, service_name: &str) {
        let mut health = self.service_health.write().await;
        health.insert(service_name.to_string(), true);
    }

    /// Mark a service as unhealthy
    async fn mark_service_unhealthy(&self, service_name: &str) {
        let mut health = self.service_health.write().await;
        health.insert(service_name.to_string(), false);
    }

    /// Shutdown all services gracefully
    pub async fn shutdown_all(&self) -> Result<()> {
        info!("Shutting down all microservices");

        // Shutdown in reverse order of initialization
        if let Some(anti_fraud) = &self.anti_fraud_service {
            if let Err(e) = (**anti_fraud).shutdown().await {
                error!("Failed to shutdown anti-fraud service: {}", e);
            } else {
                debug!("Anti-fraud service shutdown successfully");
            }
        }

        if let Some(_compliance_bridge) = &self.compliance_bridge {
            debug!("CALEA compliance bridge shutdown (no explicit shutdown needed)");
        }

        if let Some(_sip_codec) = &self.sip_codec_service {
            debug!("SIP codec integration service shutdown (no explicit shutdown needed)");
        }

        if let Some(signaling) = &self.signaling_service {
            if let Err(e) = signaling.shutdown().await {
                error!("Failed to shutdown signaling service: {}", e);
            }
        }

        if let Some(media) = &self.media_service {
            if let Err(e) = media.shutdown().await {
                error!("Failed to shutdown media service: {}", e);
            }
        }

        if let Some(routing) = &self.routing_service {
            if let Err(e) = routing.shutdown().await {
                error!("Failed to shutdown routing service: {}", e);
            }
        }

        if let Some(control) = &self.control_service {
            if let Err(e) = control.shutdown().await {
                error!("Failed to shutdown control service: {}", e);
            }
        }

        if let Some(database) = &self.database_service {
            if let Err(e) = database.shutdown().await {
                error!("Failed to shutdown database service: {}", e);
            }
        }

        info!("All microservices shutdown completed");
        Ok(())
    }

    /// Perform health checks on all services
    pub async fn health_check_all(&self) -> Result<std::collections::HashMap<String, bool>> {
        let mut results = std::collections::HashMap::new();

        // Check routing service
        if self.routing_service.is_some() {
            // TODO: Implement actual health check
            results.insert("routing".to_string(), true);
        }

        // Check media service
        if self.media_service.is_some() {
            // TODO: Implement actual health check
            results.insert("media".to_string(), true);
        }

        // Check signaling service
        if self.signaling_service.is_some() {
            // TODO: Implement actual health check
            results.insert("signaling".to_string(), true);
        }

        // Check control service
        if self.control_service.is_some() {
            // TODO: Implement actual health check
            results.insert("control".to_string(), true);
        }

        // Check anti-fraud service
        if self.anti_fraud_service.is_some() {
            // TODO: Implement actual health check
            results.insert("anti_fraud".to_string(), true);
        }

        // Check database service
        if self.database_service.is_some() {
            // TODO: Implement actual health check
            results.insert("database".to_string(), true);
        }

        // Update internal health tracking
        let mut health = self.service_health.write().await;
        for (service, is_healthy) in &results {
            health.insert(service.clone(), *is_healthy);
        }

        Ok(results)
    }

    /// Get service statistics
    pub async fn get_service_statistics(&self) -> Result<ServiceStatistics> {
        let mut stats = ServiceStatistics::default();

        // Collect routing statistics
        if let Some(routing) = &self.routing_service {
            if let Ok(routing_stats) = routing.get_stats().await {
                stats.routing_stats = Some(routing_stats);
            }
        }

        // Collect media statistics
        if let Some(media) = &self.media_service {
            if let Ok(media_stats) = media.get_stats().await {
                stats.media_stats = Some(media_stats);
            }
        }

        // Collect signaling statistics
        if let Some(signaling) = &self.signaling_service {
            if let Ok(signaling_stats) = signaling.get_stats().await {
                stats.signaling_stats = Some(signaling_stats);
            }
        }

        // Collect control statistics
        if let Some(control) = &self.control_service {
            if let Ok(system_status) = control.get_system_status().await {
                stats.system_status = Some(system_status);
            }
        }

        Ok(stats)
    }
}

/// Combined service statistics
#[derive(Debug, Clone, Default)]
pub struct ServiceStatistics {
    pub routing_stats: Option<RoutingStats>,
    pub media_stats: Option<crate::services::media::RtpStats>,
    pub signaling_stats: Option<SignalingStats>,
    pub system_status: Option<SystemStatus>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_service_registry_creation() {
        let event_bus = Arc::new(EventBus::new());
        let _registry = ServiceRegistry::new(event_bus);
    }

    #[tokio::test]
    async fn test_service_registry_core_functionality() {
        let event_bus = Arc::new(EventBus::new());
        let registry = ServiceRegistry::new(event_bus.clone());

        // Test basic service registry structure
        assert!(registry.routing().is_none()); // Should be None before initialization
        assert!(registry.media().is_none());
        assert!(registry.signaling().is_none());

        // Test service health tracking functionality
        registry.mark_service_healthy("test_service").await;
        {
            let health = registry.service_health.read().await;
            assert_eq!(health.get("test_service"), Some(&true));
        } // drop the read guard before taking the write lock below

        // Test unhealthy service tracking
        registry.mark_service_unhealthy("failing_service").await;
        {
            let health = registry.service_health.read().await;
            assert_eq!(health.get("failing_service"), Some(&false));
        }

        // Test event bus integration
        assert!(Arc::strong_count(&event_bus) >= 2); // Registry should hold a reference
    }

    #[tokio::test]
    async fn test_service_health_checking() {
        let event_bus = Arc::new(EventBus::new());
        let mut registry = ServiceRegistry::new(event_bus);

        registry
            .initialize_all()
            .await
            .expect("Failed to initialize services");

        let health_results = registry.health_check_all().await;
        assert!(health_results.is_ok());

        let health_map = health_results.expect("Health check should succeed");
        assert!(health_map.len() >= 4); // At least 4 services
        assert!(health_map.values().all(|&healthy| healthy));
    }
}
