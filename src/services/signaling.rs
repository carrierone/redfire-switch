//! Signaling Service - Handles SIP B2BUA implementations
//! 
//! This service manages SIP signaling, B2BUA operations, and call state
//! with pluggable B2BUA implementations and event-driven architecture.

use crate::events::{EventBus, RouteInfo, TelecomEvent};
use crate::security::{SecurityContext, SecurityError, audit_log, AuditEvent, validate_phone_number};
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// Configuration for the Signaling Service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalingConfig {
    /// Local SIP listening address
    pub local_address: IpAddr,
    /// Local SIP listening port
    pub local_port: u16,
    /// Maximum concurrent calls
    pub max_concurrent_calls: usize,
    /// Call setup timeout in seconds
    pub call_timeout_seconds: u64,
    /// Enable SIP authentication
    pub enable_auth: bool,
    /// Enable SIP over TLS
    pub enable_tls: bool,
    /// TLS certificate path
    pub tls_cert_path: Option<String>,
    /// TLS private key path
    pub tls_key_path: Option<String>,
    /// Default SIP user agent
    pub user_agent: String,
    /// Enable call recording
    pub enable_recording: bool,
}

impl Default for SignalingConfig {
    fn default() -> Self {
        Self {
            local_address: "0.0.0.0".parse().expect("Default IP address should be valid"),
            local_port: 5060,
            max_concurrent_calls: 10000,
            call_timeout_seconds: 300,
            enable_auth: false,
            enable_tls: false,
            tls_cert_path: None,
            tls_key_path: None,
            user_agent: "RedFire-Switch/1.0".to_string(),
            enable_recording: false,
        }
    }
}

/// SIP message representation
#[derive(Debug, Clone)]
pub struct SipMessage {
    pub method: String,
    pub request_uri: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub source_addr: SocketAddr,
    pub destination_addr: Option<SocketAddr>,
}

/// Call setup request
#[derive(Debug, Clone)]
pub struct CallSetupRequest {
    pub call_id: String,
    pub from_uri: String,
    pub to_uri: String,
    pub calling_number: String,
    pub called_number: String,
    pub source_ip: IpAddr,
    pub route_info: RouteInfo,
    pub sdp_offer: Option<String>,
    pub custom_headers: HashMap<String, String>,
}

/// Call state tracking
#[derive(Debug, Clone)]
pub enum CallState {
    Initiated,
    Proceeding,
    Ringing,
    Connected,
    Terminated,
    Failed,
}

/// Active call session
#[derive(Debug, Clone)]
pub struct CallSession {
    pub call_id: String,
    pub session_id: String,
    pub state: CallState,
    pub from_uri: String,
    pub to_uri: String,
    pub calling_number: String,
    pub called_number: String,
    pub route_info: RouteInfo,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub connected_at: Option<chrono::DateTime<chrono::Utc>>,
    pub terminated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_response_code: Option<u16>,
    pub failure_reason: Option<String>,
    pub is_recording: bool,
}

/// B2BUA plugin response
#[derive(Debug, Clone)]
pub enum PluginResponse {
    Forward(SipMessage),
    Modify(SipMessage),
    Reject(u16, String),
    Drop,
}

/// Trait for B2BUA plugins
pub trait B2BUAPlugin: Send + Sync {
    /// Plugin name for identification
    fn name(&self) -> &str;

    /// Handle incoming SIP message
    fn handle_message(&self, message: &SipMessage, call_session: Option<&CallSession>) -> Result<PluginResponse>;

    /// Plugin initialization
    fn initialize(&mut self, config: &SignalingConfig) -> Result<()> {
        let _ = config;
        Ok(())
    }

    /// Plugin health check
    fn health_check(&self) -> Result<()> {
        Ok(())
    }

    /// Plugin shutdown
    fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Default B2BUA plugin implementation
pub struct DefaultB2BUAPlugin {
    name: String,
}

impl DefaultB2BUAPlugin {
    pub fn new() -> Self {
        Self {
            name: "default-b2bua".to_string(),
        }
    }
}

impl Default for DefaultB2BUAPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl B2BUAPlugin for DefaultB2BUAPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn handle_message(&self, message: &SipMessage, _call_session: Option<&CallSession>) -> Result<PluginResponse> {
        // Default behavior: forward all messages
        Ok(PluginResponse::Forward(message.clone()))
    }
}

/// Signaling service statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SignalingStats {
    pub total_calls_attempted: u64,
    pub total_calls_connected: u64,
    pub total_calls_failed: u64,
    pub current_active_calls: usize,
    pub average_call_setup_time_ms: f64,
    pub sip_messages_processed: u64,
    pub plugin_invocations: u64,
}

/// Internal message types for the signaling service
#[derive(Debug)]
enum SignalingServiceMessage {
    HandleSipMessage {
        message: SipMessage,
        response_tx: tokio::sync::oneshot::Sender<Result<()>>,
    },
    SetupCall {
        request: CallSetupRequest,
        response_tx: tokio::sync::oneshot::Sender<Result<String>>,
    },
    TerminateCall {
        call_id: String,
        reason: String,
        response_tx: tokio::sync::oneshot::Sender<Result<()>>,
    },
    GetCallState {
        call_id: String,
        response_tx: tokio::sync::oneshot::Sender<Result<Option<CallSession>>>,
    },
}

/// Microservice for SIP signaling and B2BUA operations
pub struct SignalingService {
    /// Service configuration
    config: SignalingConfig,
    /// Event bus for publishing signaling events
    event_bus: Arc<EventBus>,
    /// Active call sessions
    call_sessions: Arc<RwLock<HashMap<String, CallSession>>>,
    /// Registered B2BUA plugins
    plugins: Arc<RwLock<Vec<Box<dyn B2BUAPlugin>>>>,
    /// Service statistics
    stats: Arc<RwLock<SignalingStats>>,
    /// Message processing channel
    request_sender: mpsc::UnboundedSender<SignalingServiceMessage>,
}

impl SignalingService {
    /// Create a new signaling service
    pub fn new(config: SignalingConfig, event_bus: Arc<EventBus>) -> Self {
        let call_sessions = Arc::new(RwLock::new(HashMap::new()));
        let plugins = Arc::new(RwLock::new(Vec::new()));
        let stats = Arc::new(RwLock::new(SignalingStats::default()));
        let (request_sender, request_receiver) = mpsc::unbounded_channel();

        let service = Self {
            config: config.clone(),
            event_bus: event_bus.clone(),
            call_sessions: call_sessions.clone(),
            plugins: plugins.clone(),
            stats: stats.clone(),
            request_sender,
        };

        // Start background signaling processor
        let processor = SignalingProcessor {
            config,
            event_bus,
            call_sessions,
            plugins,
            stats,
            request_receiver,
        };

        tokio::spawn(async move {
            processor.run().await;
        });

        service
    }

    /// Register a B2BUA plugin
    pub async fn register_plugin(&self, plugin: Box<dyn B2BUAPlugin>) -> Result<()> {
        let plugin_name = plugin.name().to_string();
        
        let mut plugins = self.plugins.write().await;
        plugins.push(plugin);
        
        info!("Registered B2BUA plugin: {}", plugin_name);
        Ok(())
    }

    /// Setup a new call
    pub async fn setup_call(&self, request: CallSetupRequest) -> Result<String> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        self.request_sender
            .send(SignalingServiceMessage::SetupCall { request, response_tx })
            .map_err(|_| anyhow::anyhow!("Failed to send setup call request"))?;

        response_rx
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive setup call response"))?
    }

    /// Handle incoming SIP message with security validation
    pub async fn handle_sip_message(&self, message: SipMessage) -> Result<()> {
        // Create security context
        let security_context = SecurityContext::new(message.source_addr.ip());
        
        // Validate SIP message format and content
        self.validate_sip_message(&message, &security_context).await?;
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        self.request_sender
            .send(SignalingServiceMessage::HandleSipMessage { message, response_tx })
            .map_err(|_| anyhow::anyhow!("Failed to send SIP message"))?;

        response_rx
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive SIP message response"))?
    }

    /// Terminate a call
    pub async fn terminate_call(&self, call_id: String, reason: String) -> Result<()> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        self.request_sender
            .send(SignalingServiceMessage::TerminateCall { call_id, reason, response_tx })
            .map_err(|_| anyhow::anyhow!("Failed to send terminate call request"))?;

        response_rx
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive terminate call response"))?
    }

    /// Get call state
    pub async fn get_call_state(&self, call_id: String) -> Result<Option<CallSession>> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        self.request_sender
            .send(SignalingServiceMessage::GetCallState { call_id, response_tx })
            .map_err(|_| anyhow::anyhow!("Failed to send get call state request"))?;

        response_rx
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive get call state response"))?
    }

    /// List all active calls
    pub async fn list_active_calls(&self) -> Result<Vec<CallSession>> {
        let sessions = self.call_sessions.read().await;
        Ok(sessions.values().cloned().collect())
    }

    /// Get signaling statistics
    pub async fn get_stats(&self) -> Result<SignalingStats> {
        let stats = self.stats.read().await;
        Ok(stats.clone())
    }

    /// Validate SIP message for security
    async fn validate_sip_message(&self, message: &SipMessage, context: &SecurityContext) -> Result<()> {
        // Validate message size (prevent memory exhaustion)
        let message_size = message.method.len() + message.request_uri.len() + 
            message.headers.values().map(|v| v.len()).sum::<usize>();
        
        if message_size > 65536 { // 64KB limit
            return Err(SecurityError::RequestTooLarge(format!("{} bytes", message_size)).into());
        }
        
        // Validate SIP method
        let valid_methods = ["INVITE", "ACK", "BYE", "CANCEL", "REGISTER", "OPTIONS", "INFO"];
        if !valid_methods.contains(&message.method.as_str()) {
            return Err(SecurityError::InvalidInput(format!("Invalid SIP method: {}", message.method)).into());
        }
        
        // Validate phone numbers in From/To headers if present
        if let Some(from_header) = message.headers.get("From") {
            if let Some(number) = self.extract_phone_number(from_header) {
                validate_phone_number(&number)?;
            }
        }
        
        if let Some(to_header) = message.headers.get("To") {
            if let Some(number) = self.extract_phone_number(to_header) {
                validate_phone_number(&number)?;
            }
        }
        
        // Log security audit event
        if let Some(call_id) = message.headers.get("Call-ID") {
            let event = AuditEvent::SipMessageProcessed {
                source_ip: context.source_ip,
                method: message.method.clone(),
                call_id: Some(call_id.clone()),
                from_uri: message.headers.get("From").cloned(),
                to_uri: message.headers.get("To").cloned(),
                processing_result: "validated".to_string(),
            };
            
            if let Err(e) = audit_log(event, context).await {
                warn!("Failed to log audit event: {}", e);
            }
        }
        
        Ok(())
    }
    
    /// Extract phone number from SIP URI
    fn extract_phone_number(&self, uri: &str) -> Option<String> {
        // Simple extraction - in practice would use proper SIP URI parsing
        if let Some(start) = uri.find("sip:") {
            let after_scheme = &uri[start + 4..];
            if let Some(end) = after_scheme.find('@') {
                return Some(after_scheme[..end].to_string());
            }
        }
        None
    }

    /// Shutdown the signaling service
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down signaling service");
        
        // Terminate all active calls
        let sessions = self.call_sessions.read().await;
        let call_ids: Vec<String> = sessions.keys().cloned().collect();
        drop(sessions);

        for call_id in call_ids {
            if let Err(e) = self.terminate_call(call_id, "service_shutdown".to_string()).await {
                warn!("Failed to terminate call during shutdown: {}", e);
            }
        }

        // Shutdown plugins
        let mut plugins = self.plugins.write().await;
        for plugin in plugins.iter_mut() {
            if let Err(e) = plugin.shutdown() {
                warn!("Failed to shutdown plugin {}: {}", plugin.name(), e);
            }
        }

        Ok(())
    }
}

/// Background processor for signaling operations
struct SignalingProcessor {
    config: SignalingConfig,
    event_bus: Arc<EventBus>,
    call_sessions: Arc<RwLock<HashMap<String, CallSession>>>,
    plugins: Arc<RwLock<Vec<Box<dyn B2BUAPlugin>>>>,
    stats: Arc<RwLock<SignalingStats>>,
    request_receiver: mpsc::UnboundedReceiver<SignalingServiceMessage>,
}

impl SignalingProcessor {
    async fn run(mut self) {
        // Start call timeout cleanup task
        let sessions_cleanup = self.call_sessions.clone();
        let config_cleanup = self.config.clone();
        let event_bus_cleanup = self.event_bus.clone();
        tokio::spawn(async move {
            Self::call_timeout_task(sessions_cleanup, config_cleanup, event_bus_cleanup).await;
        });

        // Process incoming requests
        while let Some(message) = self.request_receiver.recv().await {
            match message {
                SignalingServiceMessage::HandleSipMessage { message, response_tx } => {
                    let response = self.handle_sip_message_internal(message).await;
                    let _ = response_tx.send(response);
                }
                SignalingServiceMessage::SetupCall { request, response_tx } => {
                    let response = self.handle_setup_call(request).await;
                    let _ = response_tx.send(response);
                }
                SignalingServiceMessage::TerminateCall { call_id, reason, response_tx } => {
                    let response = self.handle_terminate_call(&call_id, &reason).await;
                    let _ = response_tx.send(response);
                }
                SignalingServiceMessage::GetCallState { call_id, response_tx } => {
                    let sessions = self.call_sessions.read().await;
                    let session = sessions.get(&call_id).cloned();
                    let _ = response_tx.send(Ok(session));
                }
            }
        }
    }

    async fn handle_sip_message_internal(&self, message: SipMessage) -> Result<()> {
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.sip_messages_processed += 1;
        }

        // Extract call ID from message
        let call_id = message.headers.get("Call-ID")
            .ok_or_else(|| anyhow::anyhow!("Missing Call-ID header"))?
            .clone();

        // Get current call session if exists
        let call_session = {
            let sessions = self.call_sessions.read().await;
            sessions.get(&call_id).cloned()
        };

        // Process message through plugins
        let plugin_response = self.process_through_plugins(&message, call_session.as_ref()).await?;

        match plugin_response {
            PluginResponse::Forward(modified_message) => {
                self.forward_message(modified_message, call_session).await?;
            }
            PluginResponse::Modify(modified_message) => {
                self.forward_message(modified_message, call_session).await?;
            }
            PluginResponse::Reject(code, reason) => {
                self.reject_message(&message, code, &reason).await?;
            }
            PluginResponse::Drop => {
                debug!("Message dropped by plugin for call {}", call_id);
            }
        }

        Ok(())
    }

    async fn process_through_plugins(&self, message: &SipMessage, call_session: Option<&CallSession>) -> Result<PluginResponse> {
        let plugins = self.plugins.read().await;
        
        // Update plugin invocation stats
        {
            let mut stats = self.stats.write().await;
            stats.plugin_invocations += 1;
        }

        // If no plugins, use default behavior (forward)
        if plugins.is_empty() {
            return Ok(PluginResponse::Forward(message.clone()));
        }

        // Process through each plugin in sequence
        let mut current_message = message.clone();
        
        for plugin in plugins.iter() {
            match plugin.handle_message(&current_message, call_session)? {
                PluginResponse::Forward(msg) => {
                    current_message = msg;
                }
                PluginResponse::Modify(msg) => {
                    current_message = msg;
                }
                response @ (PluginResponse::Reject(_, _) | PluginResponse::Drop) => {
                    return Ok(response);
                }
            }
        }

        Ok(PluginResponse::Forward(current_message))
    }

    async fn forward_message(&self, message: SipMessage, call_session: Option<CallSession>) -> Result<()> {
        // TODO: Implement actual SIP message forwarding
        debug!("Forwarding SIP message: {} for call {:?}", 
               message.method, 
               message.headers.get("Call-ID"));

        // Update call state based on message
        if let Some(call_id) = message.headers.get("Call-ID") {
            self.update_call_state_from_message(call_id, &message).await?;
        }

        Ok(())
    }

    async fn reject_message(&self, message: &SipMessage, code: u16, reason: &str) -> Result<()> {
        // TODO: Implement actual SIP rejection response
        debug!("Rejecting SIP message {} with code {}: {}", 
               message.method, code, reason);

        if let Some(call_id) = message.headers.get("Call-ID") {
            self.mark_call_failed(call_id, format!("Rejected: {} {}", code, reason)).await?;
        }

        Ok(())
    }

    async fn handle_setup_call(&self, request: CallSetupRequest) -> Result<String> {
        let sessions = self.call_sessions.read().await;
        if sessions.len() >= self.config.max_concurrent_calls {
            return Err(anyhow::anyhow!("Maximum concurrent calls reached"));
        }
        drop(sessions);

        // Create call session
        let session_id = uuid::Uuid::new_v4().to_string();
        let call_session = CallSession {
            call_id: request.call_id.clone(),
            session_id: session_id.clone(),
            state: CallState::Initiated,
            from_uri: request.from_uri.clone(),
            to_uri: request.to_uri.clone(),
            calling_number: request.calling_number.clone(),
            called_number: request.called_number.clone(),
            route_info: request.route_info.clone(),
            created_at: Utc::now(),
            connected_at: None,
            terminated_at: None,
            last_response_code: None,
            failure_reason: None,
            is_recording: self.config.enable_recording,
        };

        // Store session
        let mut sessions = self.call_sessions.write().await;
        sessions.insert(request.call_id.clone(), call_session);
        drop(sessions);

        // Update statistics
        let mut stats = self.stats.write().await;
        stats.total_calls_attempted += 1;
        stats.current_active_calls += 1;
        drop(stats);

        // Publish call initiated event
        self.publish_call_initiated_event(&request).await?;

        debug!("Setup call completed: {}", request.call_id);
        Ok(session_id)
    }

    async fn handle_terminate_call(&self, call_id: &str, reason: &str) -> Result<()> {
        let mut sessions = self.call_sessions.write().await;
        
        if let Some(mut session) = sessions.remove(call_id) {
            session.state = CallState::Terminated;
            session.terminated_at = Some(Utc::now());
            session.failure_reason = Some(reason.to_string());

            // Update statistics
            let mut stats = self.stats.write().await;
            stats.current_active_calls = stats.current_active_calls.saturating_sub(1);
            drop(stats);

            // Publish call terminated event
            self.publish_call_terminated_event(&session, reason).await?;

            info!("Terminated call: {} (reason: {})", call_id, reason);
        } else {
            warn!("Attempted to terminate non-existent call: {}", call_id);
        }

        Ok(())
    }

    async fn update_call_state_from_message(&self, call_id: &str, message: &SipMessage) -> Result<()> {
        let mut sessions = self.call_sessions.write().await;
        
        if let Some(session) = sessions.get_mut(call_id) {
            match message.method.as_str() {
                "INVITE" => session.state = CallState::Initiated,
                "100" => session.state = CallState::Proceeding,
                "180" | "183" => session.state = CallState::Ringing,
                "200" => {
                    session.state = CallState::Connected;
                    if session.connected_at.is_none() {
                        session.connected_at = Some(Utc::now());
                        
                        // Update connected calls statistics
                        let mut stats = self.stats.write().await;
                        stats.total_calls_connected += 1;
                    }
                }
                "BYE" => session.state = CallState::Terminated,
                _ => {
                    // Check for error response codes
                    if let Ok(code) = message.method.parse::<u16>() {
                        if code >= 400 {
                            session.state = CallState::Failed;
                            session.last_response_code = Some(code);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn mark_call_failed(&self, call_id: &str, reason: String) -> Result<()> {
        let mut sessions = self.call_sessions.write().await;
        
        if let Some(session) = sessions.get_mut(call_id) {
            session.state = CallState::Failed;
            session.failure_reason = Some(reason);

            // Update statistics
            let mut stats = self.stats.write().await;
            stats.total_calls_failed += 1;
        }

        Ok(())
    }

    async fn publish_call_initiated_event(&self, request: &CallSetupRequest) -> Result<()> {
        let event = TelecomEvent::call_initiated(
            request.call_id.clone(),
            uuid::Uuid::new_v4().to_string(),
            request.calling_number.clone(),
            request.called_number.clone(),
            request.source_ip,
        );

        self.event_bus.publish(event).await
            .context("Failed to publish call initiated event")?;

        Ok(())
    }

    async fn publish_call_terminated_event(&self, session: &CallSession, reason: &str) -> Result<()> {
        let cdr = crate::events::CallDetailRecord {
            call_id: session.call_id.clone(),
            calling_number: session.calling_number.clone(),
            called_number: session.called_number.clone(),
            start_time: session.created_at,
            end_time: session.terminated_at.unwrap_or_else(Utc::now),
            duration_seconds: session.connected_at
                .map(|connected| (session.terminated_at.unwrap_or_else(Utc::now) - connected).num_seconds() as u32)
                .unwrap_or(0),
            ingress_trunk_id: 0, // TODO: Get from route info
            egress_trunk_id: Some(session.route_info.trunk_id),
            termination_cause: reason.to_string(),
            cost: Some(session.route_info.cost),
            customer_id: None, // TODO: Get from request
            ani_ii_digit: None, // TODO: Extract from SIP headers
            payphone_surcharge: None, // TODO: Calculate from ANI-II
        };

        let event = TelecomEvent::CallTerminated(crate::events::CallTerminatedEvent {
            call_id: session.call_id.clone(),
            session_id: session.session_id.clone(),
            cdr,
            termination_reason: reason.to_string(),
            final_response_code: session.last_response_code.unwrap_or(200),
            call_duration_seconds: (session.terminated_at.unwrap_or_else(Utc::now) - session.created_at).num_seconds() as u32,
            timestamp: Utc::now(),
        });

        self.event_bus.publish(event).await
            .context("Failed to publish call terminated event")?;

        Ok(())
    }

    /// Background task to cleanup timed-out calls
    async fn call_timeout_task(
        sessions: Arc<RwLock<HashMap<String, CallSession>>>,
        config: SignalingConfig,
        event_bus: Arc<EventBus>,
    ) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        
        loop {
            interval.tick().await;
            
            let mut sessions_guard = sessions.write().await;
            let now = Utc::now();
            let timeout_duration = chrono::Duration::seconds(config.call_timeout_seconds as i64);
            
            let mut to_remove = Vec::new();
            
            for (call_id, session) in sessions_guard.iter() {
                let session_age = now - session.created_at;
                
                // Check if call has timed out
                if session_age > timeout_duration && 
                   !matches!(session.state, CallState::Connected | CallState::Terminated) {
                    to_remove.push((call_id.clone(), session.clone()));
                }
            }
            
            for (call_id, session) in to_remove {
                sessions_guard.remove(&call_id);
                
                // Publish timeout event
                let cdr = crate::events::CallDetailRecord {
                    call_id: session.call_id.clone(),
                    calling_number: session.calling_number.clone(),
                    called_number: session.called_number.clone(),
                    start_time: session.created_at,
                    end_time: now,
                    duration_seconds: 0,
                    ingress_trunk_id: 0,
                    egress_trunk_id: Some(session.route_info.trunk_id),
                    termination_cause: "timeout".to_string(),
                    cost: Some(session.route_info.cost),
                    customer_id: None,
                    ani_ii_digit: None, // TODO: Extract from SIP headers
                    payphone_surcharge: None, // TODO: Calculate from ANI-II
                };

                let event = TelecomEvent::CallTerminated(crate::events::CallTerminatedEvent {
                    call_id: session.call_id.clone(),
                    session_id: session.session_id.clone(),
                    cdr,
                    termination_reason: "call_timeout".to_string(),
                    final_response_code: 408, // Request Timeout
                    call_duration_seconds: (now - session.created_at).num_seconds() as u32,
                    timestamp: now,
                });

                if let Err(e) = event_bus.publish(event).await {
                    error!("Failed to publish timeout event for call {}: {}", call_id, e);
                }

                debug!("Cleaned up timed-out call: {}", call_id);
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
    async fn test_signaling_service_creation() {
        let config = SignalingConfig::default();
        let event_bus = Arc::new(EventBus::new());
        let _service = SignalingService::new(config, event_bus);
    }

    #[tokio::test]
    async fn test_plugin_registration() {
        let config = SignalingConfig::default();
        let event_bus = Arc::new(EventBus::new());
        let service = SignalingService::new(config, event_bus);

        let plugin = Box::new(DefaultB2BUAPlugin::new());
        let result = service.register_plugin(plugin).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_call_setup() {
        let config = SignalingConfig::default();
        let event_bus = Arc::new(EventBus::new());
        let service = SignalingService::new(config, event_bus);

        let route_info = RouteInfo {
            route_id: "test-route-1".to_string(),
            trunk_id: 42,
            trunk_name: "test-trunk".to_string(),
            gateway_ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
            gateway_port: 5060,
            priority: 1,
            cost: 0.05,
        };

        let request = CallSetupRequest {
            call_id: "test-call-123".to_string(),
            from_uri: "sip:1234567890@example.com".to_string(),
            to_uri: "sip:0987654321@example.com".to_string(),
            calling_number: "1234567890".to_string(),
            called_number: "0987654321".to_string(),
            source_ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)),
            route_info,
            sdp_offer: None,
            custom_headers: HashMap::new(),
        };

        let session_id = service.setup_call(request).await;
        assert!(session_id.is_ok());

        let session_id = session_id.expect("Call setup should succeed");
        assert!(!session_id.is_empty());

        // Verify call state
        let call_state = service.get_call_state("test-call-123".to_string()).await;
        assert!(call_state.is_ok());
        assert!(call_state.expect("Get call state should succeed").is_some());
    }
}