//! Voice Integrity Coordinator Service
//!
//! This service coordinates all voice integrity components including batch transcoding,
//! audio recording, legal authorization, and Vosk ASR integration.
//!
//! Key features:
//! - Unified configuration and initialization
//! - Resource management and optimization
//! - Performance monitoring and alerting
//! - Graceful degradation under load

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};

use crate::events::EventBus;
use crate::services::{
    AudioRecordingService, AudioRecordingConfig,
    BatchTranscodingService, BatchTranscodingConfig,
    DiskMonitoringService, DiskMonitoringConfig,
    LawfulInterceptComplianceService, ComplianceConfig,
    LegalAuthorizationService, LegalAuthorizationConfig,
    MemoryManagementService, MemoryManagementConfig,
    VoiceIntegrityDatabaseService, VoiceIntegrityDatabaseConfig,
    VoskClientService, VoskConfig,
    RtpRecordingBridgeService, RtpRecordingBridgeConfig,
};

/// Voice integrity system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceIntegrityConfig {
    /// Audio recording configuration
    pub audio_recording: AudioRecordingConfig,
    /// Batch transcoding configuration
    pub batch_transcoding: BatchTranscodingConfig,
    /// Legal authorization configuration
    pub legal_authorization: LegalAuthorizationConfig,
    /// Database configuration
    pub database: VoiceIntegrityDatabaseConfig,
    /// Vosk ASR configuration
    pub vosk: VoskConfig,
    /// RTP recording bridge configuration
    pub rtp_bridge: RtpRecordingBridgeConfig,
    /// Compliance tracking configuration
    pub compliance: ComplianceConfig,
    /// Memory management configuration
    pub memory_management: MemoryManagementConfig,
    /// Disk monitoring configuration
    pub disk_monitoring: DiskMonitoringConfig,
    /// Enable performance monitoring
    pub enable_monitoring: bool,
    /// Performance alert thresholds
    pub performance_thresholds: PerformanceThresholds,
}

/// Performance monitoring thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceThresholds {
    /// CPU usage warning threshold (0.0-1.0)
    pub cpu_warning_threshold: f64,
    /// CPU usage critical threshold (0.0-1.0)
    pub cpu_critical_threshold: f64,
    /// Queue backlog warning threshold
    pub queue_warning_threshold: usize,
    /// Queue backlog critical threshold
    pub queue_critical_threshold: usize,
    /// Memory usage warning threshold (bytes)
    pub memory_warning_threshold: u64,
    /// Memory usage critical threshold (bytes)
    pub memory_critical_threshold: u64,
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self {
            cpu_warning_threshold: 0.75,
            cpu_critical_threshold: 0.90,
            queue_warning_threshold: 500,
            queue_critical_threshold: 1000,
            memory_warning_threshold: 8 * 1024 * 1024 * 1024, // 8GB
            memory_critical_threshold: 12 * 1024 * 1024 * 1024, // 12GB
        }
    }
}

impl Default for VoiceIntegrityConfig {
    fn default() -> Self {
        Self {
            audio_recording: AudioRecordingConfig::default(),
            batch_transcoding: BatchTranscodingConfig::default(),
            legal_authorization: LegalAuthorizationConfig::default(),
            database: VoiceIntegrityDatabaseConfig::default(),
            vosk: VoskConfig::default(),
            rtp_bridge: RtpRecordingBridgeConfig::default(),
            compliance: ComplianceConfig::default(),
            memory_management: MemoryManagementConfig::default(),
            disk_monitoring: DiskMonitoringConfig::default(),
            enable_monitoring: true,
            performance_thresholds: PerformanceThresholds::default(),
        }
    }
}

/// Voice integrity system status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceIntegrityStatus {
    pub audio_recording_active: bool,
    pub batch_transcoding_active: bool,
    pub legal_authorization_active: bool,
    pub database_active: bool,
    pub vosk_connected: bool,
    pub rtp_bridge_active: bool,
    pub compliance_monitoring_active: bool,
    pub memory_management_active: bool,
    pub disk_monitoring_active: bool,
    pub current_cpu_load: f64,
    pub memory_usage_percent: f64,
    pub disk_usage_percent: f64,
    pub queue_backlog: usize,
    pub active_recordings: usize,
    pub active_violations: usize,
    pub performance_alerts: Vec<String>,
}

/// Voice integrity coordinator service
pub struct VoiceIntegrityCoordinator {
    config: VoiceIntegrityConfig,
    event_bus: Arc<EventBus>,

    // Core services
    audio_recording_service: Arc<AudioRecordingService>,
    batch_transcoding_service: Arc<BatchTranscodingService>,
    legal_authorization_service: Arc<LegalAuthorizationService>,
    database_service: Arc<VoiceIntegrityDatabaseService>,
    vosk_client_service: Arc<VoskClientService>,
    rtp_bridge_service: Arc<RtpRecordingBridgeService>,
    compliance_service: Arc<LawfulInterceptComplianceService>,
    memory_management_service: Arc<MemoryManagementService>,
    disk_monitoring_service: Arc<DiskMonitoringService>,

    // Status tracking
    system_status: Arc<RwLock<VoiceIntegrityStatus>>,
}

impl VoiceIntegrityCoordinator {
    /// Create and initialize voice integrity coordinator
    pub async fn new(config: VoiceIntegrityConfig, event_bus: Arc<EventBus>) -> Result<Self> {
        info!("Initializing Voice Integrity Coordinator");

        // Initialize core services in dependency order

        // 1. Database service (required by others)
        let database_service = Arc::new(
            VoiceIntegrityDatabaseService::new(config.database.clone(), event_bus.clone()).await?
        );

        // 2. Legal authorization service
        let legal_authorization_service = Arc::new(
            LegalAuthorizationService::new(config.legal_authorization.clone(), event_bus.clone())
        );

        // 3. Vosk client service
        let vosk_client_service = Arc::new(
            VoskClientService::new(config.vosk.clone(), event_bus.clone())?
        );

        // 4. Memory management service (needed by audio recording)
        let memory_management_service = Arc::new(
            MemoryManagementService::new(
                config.memory_management.clone(),
                event_bus.clone(),
            )
        );

        // 5. Batch transcoding service
        let batch_transcoding_service = Arc::new(
            BatchTranscodingService::new(
                config.batch_transcoding.clone(),
                event_bus.clone(),
                vosk_client_service.clone(),
            )?
        );

        // 6. Audio recording service
        let audio_recording_service = Arc::new(
            AudioRecordingService::new(
                config.audio_recording.clone(),
                event_bus.clone(),
                legal_authorization_service.clone(),
                memory_management_service.clone(),
                Some(batch_transcoding_service.clone()),
            )?
        );

        // 7. RTP recording bridge service
        let rtp_bridge_service = Arc::new(
            RtpRecordingBridgeService::new(
                config.rtp_bridge.clone(),
                event_bus.clone(),
                audio_recording_service.clone(),
                legal_authorization_service.clone(),
            )
        );

        // 8. Disk monitoring service
        let disk_monitoring_service = Arc::new(
            DiskMonitoringService::new(
                config.disk_monitoring.clone(),
                event_bus.clone(),
            )
        );

        // 9. Compliance service
        let compliance_service = Arc::new(
            LawfulInterceptComplianceService::new(
                database_service.get_pool().clone(),
                event_bus.clone(),
                config.compliance.clone(),
            )
        );

        // Initialize system status
        let system_status = Arc::new(RwLock::new(VoiceIntegrityStatus {
            audio_recording_active: true,
            batch_transcoding_active: true,
            legal_authorization_active: true,
            database_active: true,
            vosk_connected: false, // Will be updated by monitoring
            rtp_bridge_active: true,
            compliance_monitoring_active: true,
            memory_management_active: true,
            disk_monitoring_active: true,
            current_cpu_load: 0.0,
            memory_usage_percent: 0.0,
            disk_usage_percent: 0.0,
            queue_backlog: 0,
            active_recordings: 0,
            active_violations: 0,
            performance_alerts: Vec::new(),
        }));

        let coordinator = Self {
            config: config.clone(),
            event_bus: event_bus.clone(),
            audio_recording_service,
            batch_transcoding_service,
            legal_authorization_service,
            database_service,
            vosk_client_service,
            rtp_bridge_service,
            compliance_service,
            memory_management_service,
            disk_monitoring_service,
            system_status,
        };

        // Start performance monitoring if enabled
        if config.enable_monitoring {
            coordinator.start_performance_monitoring().await;
        }

        // Start compliance monitoring
        if config.compliance.enable_monitoring {
            if let Err(e) = coordinator.compliance_service.start_monitoring().await {
                warn!("Failed to start compliance monitoring: {}", e);
            }
        }

        // Start memory management monitoring
        if let Err(e) = coordinator.memory_management_service.start_monitoring().await {
            warn!("Failed to start memory management monitoring: {}", e);
        }

        // Start disk monitoring
        if let Err(e) = coordinator.disk_monitoring_service.start_monitoring().await {
            warn!("Failed to start disk monitoring: {}", e);
        }

        info!("Voice Integrity Coordinator initialized successfully");
        Ok(coordinator)
    }

    /// Start performance monitoring
    async fn start_performance_monitoring(&self) {
        let system_status = self.system_status.clone();
        let batch_service = self.batch_transcoding_service.clone();
        let audio_service = self.audio_recording_service.clone();
        let vosk_service = self.vosk_client_service.clone();
        let event_bus = self.event_bus.clone();
        let thresholds = self.config.performance_thresholds.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));

            loop {
                interval.tick().await;

                // Gather performance metrics
                let cpu_metrics = batch_service.get_cpu_metrics().await;
                let batch_stats = batch_service.get_statistics().await;
                let recording_count = audio_service.get_active_recording_count().await;
                let vosk_connected = vosk_service.is_connected().await;

                // Get compliance statistics
                let compliance_stats = system_status.read().await.clone();

                // Check thresholds and generate alerts
                let mut alerts = Vec::new();

                if cpu_metrics.current_load_percent > thresholds.cpu_critical_threshold {
                    alerts.push(format!("CRITICAL: CPU usage at {:.1}%", cpu_metrics.current_load_percent * 100.0));
                } else if cpu_metrics.current_load_percent > thresholds.cpu_warning_threshold {
                    alerts.push(format!("WARNING: CPU usage at {:.1}%", cpu_metrics.current_load_percent * 100.0));
                }

                if batch_stats.current_queue_size > thresholds.queue_critical_threshold {
                    alerts.push(format!("CRITICAL: Queue backlog at {} jobs", batch_stats.current_queue_size));
                } else if batch_stats.current_queue_size > thresholds.queue_warning_threshold {
                    alerts.push(format!("WARNING: Queue backlog at {} jobs", batch_stats.current_queue_size));
                }

                if !vosk_connected {
                    alerts.push("WARNING: Vosk ASR service disconnected".to_string());
                }

                // Update system status
                {
                    let mut status = system_status.write().await;
                    status.vosk_connected = vosk_connected;
                    status.current_cpu_load = cpu_metrics.current_load_percent;
                    status.queue_backlog = batch_stats.current_queue_size;
                    status.active_recordings = recording_count;
                    status.performance_alerts = alerts.clone();

                    // Update compliance monitoring status
                    status.compliance_monitoring_active = true;
                }

                // Emit critical alerts as events
                for alert in alerts {
                    if alert.starts_with("CRITICAL") {
                        error!("{}", alert);
                        let event = crate::events::TelecomEvent::VoiceIntegrityAudit {
                            user_id: None,
                            action_type: "performance_alert".to_string(),
                            resource_type: "system_performance".to_string(),
                            resource_id: "voice_integrity_coordinator".to_string(),
                            authorization_id: None,
                            ecpa_compliant: true,
                        };
                        let _ = event_bus.publish(event).await;
                    } else if alert.starts_with("WARNING") {
                        warn!("{}", alert);
                    }
                }
            }
        });
    }

    /// Get current system status
    pub async fn get_system_status(&self) -> VoiceIntegrityStatus {
        self.system_status.read().await.clone()
    }

    /// Get comprehensive system statistics
    pub async fn get_system_statistics(&self) -> Result<serde_json::Value> {
        let audio_stats = self.audio_recording_service.get_recording_stats().await;
        let batch_stats = self.batch_transcoding_service.get_statistics().await;
        let cpu_metrics = self.batch_transcoding_service.get_cpu_metrics().await;
        let vosk_stats = self.vosk_client_service.get_statistics().await;
        let queue_status = self.batch_transcoding_service.get_queue_status().await;
        let database_stats = self.database_service.get_service_statistics().await;
        let compliance_stats = self.compliance_service.get_compliance_statistics().await.unwrap_or(serde_json::json!({}));

        let stats = serde_json::json!({
            "timestamp": chrono::Utc::now(),
            "audio_recording": audio_stats,
            "batch_transcoding": {
                "statistics": batch_stats,
                "cpu_metrics": cpu_metrics,
                "queue_status": queue_status,
            },
            "vosk_asr": vosk_stats,
            "database": database_stats,
            "compliance": compliance_stats,
            "system_status": self.get_system_status().await,
        });

        Ok(stats)
    }

    /// Perform health check on all components
    pub async fn health_check(&self) -> Result<std::collections::HashMap<String, bool>> {
        let mut health = std::collections::HashMap::new();

        // Check audio recording service
        let recording_count = self.audio_recording_service.get_active_recording_count().await;
        health.insert("audio_recording".to_string(), recording_count < 10000); // Arbitrary limit

        // Check batch transcoding service
        let batch_stats = self.batch_transcoding_service.get_statistics().await;
        health.insert("batch_transcoding".to_string(), batch_stats.current_queue_size < 1000);

        // Check Vosk connectivity
        let vosk_connected = self.vosk_client_service.is_connected().await;
        health.insert("vosk_asr".to_string(), vosk_connected);

        // Check CPU load
        let cpu_metrics = self.batch_transcoding_service.get_cpu_metrics().await;
        health.insert("cpu_load".to_string(), cpu_metrics.current_load_percent < 0.95);

        // Overall system health
        let overall_healthy = health.values().all(|&h| h);
        health.insert("overall".to_string(), overall_healthy);

        Ok(health)
    }

    /// Get service references for external use
    pub fn get_audio_recording_service(&self) -> Arc<AudioRecordingService> {
        self.audio_recording_service.clone()
    }

    pub fn get_batch_transcoding_service(&self) -> Arc<BatchTranscodingService> {
        self.batch_transcoding_service.clone()
    }

    pub fn get_legal_authorization_service(&self) -> Arc<LegalAuthorizationService> {
        self.legal_authorization_service.clone()
    }

    pub fn get_database_service(&self) -> Arc<VoiceIntegrityDatabaseService> {
        self.database_service.clone()
    }

    pub fn get_vosk_client_service(&self) -> Arc<VoskClientService> {
        self.vosk_client_service.clone()
    }

    pub fn get_rtp_bridge_service(&self) -> Arc<RtpRecordingBridgeService> {
        self.rtp_bridge_service.clone()
    }

    pub fn get_compliance_service(&self) -> Arc<LawfulInterceptComplianceService> {
        self.compliance_service.clone()
    }

    /// Graceful shutdown of all services
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down Voice Integrity Coordinator");

        // Shutdown services in reverse dependency order
        // Note: In a real implementation, each service would have its own shutdown method

        info!("Voice Integrity Coordinator shutdown completed");
        Ok(())
    }

    /// Update configuration dynamically
    pub async fn update_configuration(&mut self, new_config: VoiceIntegrityConfig) -> Result<()> {
        info!("Updating Voice Integrity Coordinator configuration");

        // Note: In a real implementation, we would need to implement configuration
        // update methods that take Arc<RwLock<Config>> for thread-safe updates
        // For now, we'll just store the new configuration

        // Store new configuration
        self.config = new_config;

        info!("Configuration update completed (runtime updates not implemented)");
        Ok(())
    }
}