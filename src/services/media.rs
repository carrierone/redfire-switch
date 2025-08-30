//! Media Service - Handles RTP proxy and codec operations
//! 
//! This service manages media streams, codec negotiation, and RTP proxying
//! for telecommunications calls with event-driven monitoring.

use crate::events::{EventBus, MediaSessionInfo, TelecomEvent};
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

/// Configuration for the Media Service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaConfig {
    /// RTP port range start
    pub rtp_port_start: u16,
    /// RTP port range end
    pub rtp_port_end: u16,
    /// Enable RTCP
    pub enable_rtcp: bool,
    /// RTP timeout in seconds
    pub rtp_timeout_seconds: u64,
    /// Enable media encryption (SRTP)
    pub enable_encryption: bool,
    /// Maximum concurrent media sessions
    pub max_sessions: usize,
    /// Buffer size for RTP packets
    pub rtp_buffer_size: usize,
    /// Enable codec transcoding
    pub enable_transcoding: bool,
    /// Supported codecs
    pub supported_codecs: Vec<String>,
}

impl Default for MediaConfig {
    fn default() -> Self {
        Self {
            rtp_port_start: 10000,
            rtp_port_end: 20000,
            enable_rtcp: true,
            rtp_timeout_seconds: 30,
            enable_encryption: false,
            max_sessions: 10000,
            rtp_buffer_size: 8192,
            enable_transcoding: false,
            supported_codecs: vec![
                "PCMU".to_string(),
                "PCMA".to_string(), 
                "G729".to_string(),
                "G722".to_string(),
                "GSM".to_string(),
            ],
        }
    }
}

/// Media session request
#[derive(Debug, Clone)]
pub struct MediaSessionRequest {
    pub call_id: String,
    pub session_id: String,
    pub caller_ip: IpAddr,
    pub caller_port: u16,
    pub called_ip: Option<IpAddr>,
    pub called_port: Option<u16>,
    pub preferred_codec: Option<String>,
    pub enable_recording: bool,
}

/// Media session response
#[derive(Debug, Clone)]
pub struct MediaSessionResponse {
    pub session_id: String,
    pub local_rtp_port: u16,
    pub local_rtcp_port: Option<u16>,
    pub allocated_bandwidth_kbps: u32,
    pub negotiated_codec: String,
    pub encryption_enabled: bool,
}

/// Active media session
#[derive(Debug, Clone)]
pub struct MediaSession {
    pub session_id: String,
    pub call_id: String,
    pub local_rtp_port: u16,
    pub local_rtcp_port: Option<u16>,
    pub remote_rtp_addr: Option<SocketAddr>,
    pub remote_rtcp_addr: Option<SocketAddr>,
    pub codec: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_packet_time: chrono::DateTime<chrono::Utc>,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub jitter_ms: f64,
    pub packet_loss_rate: f64,
    pub is_recording: bool,
}

/// RTP packet statistics
#[derive(Debug, Clone, Default)]
pub struct RtpStats {
    pub packets_received: u64,
    pub packets_sent: u64,
    pub bytes_received: u64,
    pub bytes_sent: u64,
    pub packets_lost: u64,
    pub average_jitter_ms: f64,
    pub current_sessions: usize,
}

/// Microservice for media handling
pub struct MediaService {
    /// Service configuration
    config: MediaConfig,
    /// Event bus for publishing media events
    event_bus: Arc<EventBus>,
    /// Active media sessions
    sessions: Arc<RwLock<HashMap<String, MediaSession>>>,
    /// Port allocation tracker
    allocated_ports: Arc<RwLock<HashMap<u16, String>>>,
    /// RTP statistics
    stats: Arc<RwLock<RtpStats>>,
    /// Session processing channel
    request_sender: mpsc::UnboundedSender<MediaServiceMessage>,
}

/// Internal message types for the media service
#[derive(Debug)]
enum MediaServiceMessage {
    CreateSession {
        request: MediaSessionRequest,
        response_tx: tokio::sync::oneshot::Sender<Result<MediaSessionResponse>>,
    },
    DestroySession {
        session_id: String,
        response_tx: tokio::sync::oneshot::Sender<Result<()>>,
    },
    UpdateSessionStats {
        session_id: String,
        rx_packets: u64,
        tx_packets: u64,
        jitter_ms: f64,
    },
}

impl MediaService {
    /// Create a new media service
    pub fn new(config: MediaConfig, event_bus: Arc<EventBus>) -> Self {
        let sessions = Arc::new(RwLock::new(HashMap::new()));
        let allocated_ports = Arc::new(RwLock::new(HashMap::new()));
        let stats = Arc::new(RwLock::new(RtpStats::default()));
        let (request_sender, request_receiver) = mpsc::unbounded_channel();

        let service = Self {
            config: config.clone(),
            event_bus: event_bus.clone(),
            sessions: sessions.clone(),
            allocated_ports: allocated_ports.clone(),
            stats: stats.clone(),
            request_sender,
        };

        // Start background media processor
        let processor = MediaProcessor {
            config,
            event_bus,
            sessions,
            allocated_ports,
            stats,
            request_receiver,
        };

        tokio::spawn(async move {
            processor.run().await;
        });

        service
    }

    /// Create a new media session
    pub async fn create_session(&self, request: MediaSessionRequest) -> Result<MediaSessionResponse> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        self.request_sender
            .send(MediaServiceMessage::CreateSession { request, response_tx })
            .map_err(|_| anyhow::anyhow!("Failed to send create session request"))?;

        response_rx
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive create session response"))?
    }

    /// Destroy a media session
    pub async fn destroy_session(&self, session_id: String) -> Result<()> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        self.request_sender
            .send(MediaServiceMessage::DestroySession { session_id, response_tx })
            .map_err(|_| anyhow::anyhow!("Failed to send destroy session request"))?;

        response_rx
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive destroy session response"))?
    }

    /// Get media session information
    pub async fn get_session(&self, session_id: &str) -> Result<Option<MediaSession>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.get(session_id).cloned())
    }

    /// List all active sessions
    pub async fn list_sessions(&self) -> Result<Vec<MediaSession>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.values().cloned().collect())
    }

    /// Get RTP statistics
    pub async fn get_stats(&self) -> Result<RtpStats> {
        let stats = self.stats.read().await;
        Ok(stats.clone())
    }

    /// Update session statistics
    pub async fn update_session_stats(
        &self,
        session_id: String,
        rx_packets: u64,
        tx_packets: u64,
        jitter_ms: f64,
    ) -> Result<()> {
        self.request_sender
            .send(MediaServiceMessage::UpdateSessionStats {
                session_id,
                rx_packets,
                tx_packets,
                jitter_ms,
            })
            .map_err(|_| anyhow::anyhow!("Failed to send stats update"))?;

        Ok(())
    }

    /// Shutdown the media service
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down media service");
        
        // Close all active sessions
        let sessions = self.sessions.read().await;
        let session_ids: Vec<String> = sessions.keys().cloned().collect();
        drop(sessions);

        for session_id in session_ids {
            if let Err(e) = self.destroy_session(session_id).await {
                warn!("Failed to destroy session during shutdown: {}", e);
            }
        }

        Ok(())
    }
}

/// Background processor for media operations
struct MediaProcessor {
    config: MediaConfig,
    event_bus: Arc<EventBus>,
    sessions: Arc<RwLock<HashMap<String, MediaSession>>>,
    allocated_ports: Arc<RwLock<HashMap<u16, String>>>,
    stats: Arc<RwLock<RtpStats>>,
    request_receiver: mpsc::UnboundedReceiver<MediaServiceMessage>,
}

impl MediaProcessor {
    async fn run(mut self) {
        // Start session cleanup task
        let sessions_cleanup = self.sessions.clone();
        let config_cleanup = self.config.clone();
        tokio::spawn(async move {
            Self::session_cleanup_task(sessions_cleanup, config_cleanup).await;
        });

        // Process incoming requests
        while let Some(message) = self.request_receiver.recv().await {
            match message {
                MediaServiceMessage::CreateSession { request, response_tx } => {
                    let response = self.handle_create_session(request).await;
                    let _ = response_tx.send(response);
                }
                MediaServiceMessage::DestroySession { session_id, response_tx } => {
                    let response = self.handle_destroy_session(&session_id).await;
                    let _ = response_tx.send(response);
                }
                MediaServiceMessage::UpdateSessionStats { session_id, rx_packets, tx_packets, jitter_ms } => {
                    self.handle_update_stats(&session_id, rx_packets, tx_packets, jitter_ms).await;
                }
            }
        }
    }

    async fn handle_create_session(&self, request: MediaSessionRequest) -> Result<MediaSessionResponse> {
        // Check if we've reached the maximum number of sessions
        let sessions = self.sessions.read().await;
        if sessions.len() >= self.config.max_sessions {
            return Err(anyhow::anyhow!("Maximum number of media sessions reached"));
        }
        drop(sessions);

        // Allocate RTP port
        let rtp_port = self.allocate_port().await
            .context("Failed to allocate RTP port")?;

        // Allocate RTCP port if enabled
        let rtcp_port = if self.config.enable_rtcp {
            Some(self.allocate_port().await
                .context("Failed to allocate RTCP port")?)
        } else {
            None
        };

        // Determine codec
        let negotiated_codec = self.negotiate_codec(request.preferred_codec.as_deref());

        // Create media session
        let session = MediaSession {
            session_id: request.session_id.clone(),
            call_id: request.call_id.clone(),
            local_rtp_port: rtp_port,
            local_rtcp_port: rtcp_port,
            remote_rtp_addr: None,
            remote_rtcp_addr: None,
            codec: negotiated_codec.clone(),
            created_at: Utc::now(),
            last_packet_time: Utc::now(),
            rx_packets: 0,
            tx_packets: 0,
            rx_bytes: 0,
            tx_bytes: 0,
            jitter_ms: 0.0,
            packet_loss_rate: 0.0,
            is_recording: request.enable_recording,
        };

        // Store session
        let mut sessions = self.sessions.write().await;
        sessions.insert(request.session_id.clone(), session);
        drop(sessions);

        // Update statistics
        let mut stats = self.stats.write().await;
        stats.current_sessions += 1;
        drop(stats);

        // Publish event
        self.publish_session_created_event(&request, rtp_port, &negotiated_codec).await?;

        let response = MediaSessionResponse {
            session_id: request.session_id,
            local_rtp_port: rtp_port,
            local_rtcp_port: rtcp_port,
            allocated_bandwidth_kbps: self.calculate_bandwidth(&negotiated_codec),
            negotiated_codec,
            encryption_enabled: self.config.enable_encryption,
        };

        debug!("Created media session: {:?}", response);
        Ok(response)
    }

    async fn handle_destroy_session(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(session) = sessions.remove(session_id) {
            // Release allocated ports
            let mut ports = self.allocated_ports.write().await;
            ports.remove(&session.local_rtp_port);
            if let Some(rtcp_port) = session.local_rtcp_port {
                ports.remove(&rtcp_port);
            }
            drop(ports);

            // Update statistics
            let mut stats = self.stats.write().await;
            stats.current_sessions = stats.current_sessions.saturating_sub(1);
            drop(stats);

            // Publish event
            self.publish_session_destroyed_event(&session).await?;

            info!("Destroyed media session: {}", session_id);
        } else {
            warn!("Attempted to destroy non-existent session: {}", session_id);
        }

        Ok(())
    }

    async fn handle_update_stats(&self, session_id: &str, rx_packets: u64, tx_packets: u64, jitter_ms: f64) {
        let mut sessions = self.sessions.write().await;
        
        if let Some(session) = sessions.get_mut(session_id) {
            session.rx_packets = rx_packets;
            session.tx_packets = tx_packets;
            session.jitter_ms = jitter_ms;
            session.last_packet_time = Utc::now();
        }
    }

    async fn allocate_port(&self) -> Result<u16> {
        let mut ports = self.allocated_ports.write().await;
        
        for port in self.config.rtp_port_start..=self.config.rtp_port_end {
            if !ports.contains_key(&port) {
                // Try to bind to the port to ensure it's available
                if let Ok(_socket) = UdpSocket::bind(("0.0.0.0", port)).await {
                    ports.insert(port, "allocated".to_string());
                    return Ok(port);
                }
            }
        }

        Err(anyhow::anyhow!("No available ports in range"))
    }

    fn negotiate_codec(&self, preferred: Option<&str>) -> String {
        // If a preferred codec is specified and supported, use it
        if let Some(preferred_codec) = preferred {
            if self.config.supported_codecs.contains(&preferred_codec.to_string()) {
                return preferred_codec.to_string();
            }
        }

        // Default to first supported codec
        self.config.supported_codecs
            .first()
            .unwrap_or(&"PCMU".to_string())
            .clone()
    }

    fn calculate_bandwidth(&self, codec: &str) -> u32 {
        match codec {
            "G729" => 8,      // kbps
            "GSM" => 13,      // kbps
            "PCMU" => 64,     // kbps
            "PCMA" => 64,     // kbps
            "G722" => 64,     // kbps
            _ => 64,          // Default to PCMU bandwidth
        }
    }

    async fn publish_session_created_event(&self, request: &MediaSessionRequest, rtp_port: u16, codec: &str) -> Result<()> {
        let media_info = MediaSessionInfo {
            rtp_local_port: rtp_port,
            rtp_remote_port: request.caller_port,
            rtp_remote_ip: request.caller_ip,
            rtcp_enabled: self.config.enable_rtcp,
            encryption_enabled: self.config.enable_encryption,
            bandwidth_kbps: self.calculate_bandwidth(codec),
        };

        let event = TelecomEvent::CallConnected(crate::events::CallConnectedEvent {
            call_id: request.call_id.clone(),
            session_id: request.session_id.clone(),
            media_details: media_info,
            codec_negotiated: codec.to_string(),
            rtp_proxy_used: true,
            connection_time_ms: 0, // TODO: Track actual connection time
            timestamp: Utc::now(),
        });

        self.event_bus.publish(event).await
            .context("Failed to publish session created event")?;

        Ok(())
    }

    async fn publish_session_destroyed_event(&self, session: &MediaSession) -> Result<()> {
        // Create a call terminated event with media statistics
        let cdr = crate::events::CallDetailRecord {
            call_id: session.call_id.clone(),
            calling_number: "unknown".to_string(), // TODO: Store caller info
            called_number: "unknown".to_string(),  // TODO: Store callee info
            start_time: session.created_at,
            end_time: Utc::now(),
            duration_seconds: (Utc::now() - session.created_at).num_seconds() as u32,
            ingress_trunk_id: 0, // TODO: Store trunk info
            egress_trunk_id: None,
            termination_cause: "normal".to_string(),
            cost: None,
            customer_id: None,
            ani_ii_digit: None, // ANI-II not available in media layer
            payphone_surcharge: None, // Surcharge not applicable in media layer
        };

        let event = TelecomEvent::CallTerminated(crate::events::CallTerminatedEvent {
            call_id: session.call_id.clone(),
            session_id: session.session_id.clone(),
            cdr,
            termination_reason: "session_destroyed".to_string(),
            final_response_code: 200,
            call_duration_seconds: (Utc::now() - session.created_at).num_seconds() as u32,
            timestamp: Utc::now(),
        });

        self.event_bus.publish(event).await
            .context("Failed to publish session destroyed event")?;

        Ok(())
    }

    /// Background task to cleanup stale sessions
    async fn session_cleanup_task(
        sessions: Arc<RwLock<HashMap<String, MediaSession>>>,
        config: MediaConfig,
    ) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        
        loop {
            interval.tick().await;
            
            let mut sessions_guard = sessions.write().await;
            let now = Utc::now();
            let timeout_duration = chrono::Duration::seconds(config.rtp_timeout_seconds as i64);
            
            let mut to_remove = Vec::new();
            
            for (session_id, session) in sessions_guard.iter() {
                if now - session.last_packet_time > timeout_duration {
                    to_remove.push(session_id.clone());
                }
            }
            
            for session_id in to_remove {
                sessions_guard.remove(&session_id);
                debug!("Cleaned up stale media session: {}", session_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventBus;
    use std::net::Ipv4Addr;

    #[tokio::test]
    async fn test_media_service_creation() {
        let config = MediaConfig::default();
        let event_bus = Arc::new(EventBus::new());
        let _service = MediaService::new(config, event_bus);
    }

    #[tokio::test]
    async fn test_media_session_lifecycle() {
        let config = MediaConfig::default();
        let event_bus = Arc::new(EventBus::new());
        let service = MediaService::new(config, event_bus);

        let request = MediaSessionRequest {
            call_id: "test-call-123".to_string(),
            session_id: "test-session-456".to_string(),
            caller_ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)),
            caller_port: 5004,
            called_ip: None,
            called_port: None,
            preferred_codec: Some("G729".to_string()),
            enable_recording: false,
        };

        // Create session
        let response = service.create_session(request).await;
        assert!(response.is_ok());

        let response = response.expect("Session creation should succeed");
        assert_eq!(response.negotiated_codec, "G729");

        // Get session
        let session = service.get_session(&response.session_id).await;
        assert!(session.is_ok());
        assert!(session.expect("Get session should succeed").is_some());

        // Destroy session
        let result = service.destroy_session(response.session_id.clone()).await;
        assert!(result.is_ok());

        // Verify session is gone
        let session = service.get_session(&response.session_id).await;
        assert!(session.is_ok());
        assert!(session.expect("Get session should succeed").is_none());
    }
}