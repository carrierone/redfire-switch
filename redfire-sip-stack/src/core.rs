//! SIP Core Engine - Central SIP processing and call management
//!
//! This module provides the core SIP protocol processing engine that handles
//! call state management, transaction processing, and dialog management.

use crate::parser::SipMessage;
use crate::state::{SipStateManager, SipStateConfig};
use crate::transport::{SipTransportManager, TransportConfig, SipTransport};
use crate::authentication::SipAuthenticator;
use rsip::method::Method;
use rsip::message::HeadersExt;
use rsip::headers::UntypedHeader;
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};
use tracing::{debug, info, warn};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// SIP Core Engine Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipCoreConfig {
    /// Local SIP domain
    pub domain: String,
    /// Local IP address
    pub local_ip: IpAddr,
    /// SIP port
    pub port: u16,
    /// User agent string
    pub user_agent: String,
    /// Maximum concurrent calls
    pub max_calls: u32,
    /// Enable strict RFC compliance
    pub strict_compliance: bool,
    /// Session expires timeout (seconds)
    pub session_expires: u32,
    /// Minimum session expires (seconds)
    pub min_se: u32,
    /// Supported SIP methods
    pub supported_methods: Vec<String>,
}

impl Default for SipCoreConfig {
    fn default() -> Self {
        Self {
            domain: "localhost".to_string(),
            local_ip: "127.0.0.1".parse().unwrap(),
            port: 5060,
            user_agent: "Redfire-SIP-Core/1.0".to_string(),
            max_calls: 10000,
            strict_compliance: false,
            session_expires: 3600,
            min_se: 90,
            supported_methods: vec![
                "INVITE".to_string(),
                "ACK".to_string(),
                "BYE".to_string(),
                "CANCEL".to_string(),
                "OPTIONS".to_string(),
                "REGISTER".to_string(),
                "INFO".to_string(),
                "UPDATE".to_string(),
                "PRACK".to_string(),
            ],
        }
    }
}

/// SIP Call Context - Tracks call session information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipCallContext {
    /// Unique call identifier
    pub call_id: String,
    /// From URI
    pub from_uri: String,
    /// To URI
    pub to_uri: String,
    /// Calling number (ANI)
    pub calling_number: String,
    /// Called number (DNIS)
    pub called_number: String,
    /// Technology prefix
    pub tech_prefix: Option<String>,
    /// Trunk identifier
    pub trunk_id: Option<String>,
    /// Customer identifier
    pub customer_id: Option<String>,
    /// Source IP address
    pub source_ip: SocketAddr,
    /// Transport protocol
    pub transport: SipTransport,
    /// Call creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last activity timestamp
    pub last_activity: DateTime<Utc>,
}

impl SipCallContext {
    /// Create new call context
    pub fn new(call_id: String, from_uri: String, to_uri: String, source_ip: SocketAddr, transport: SipTransport) -> Self {
        let now = Utc::now();
        Self {
            call_id,
            from_uri,
            to_uri,
            calling_number: String::new(),
            called_number: String::new(),
            tech_prefix: None,
            trunk_id: None,
            customer_id: None,
            source_ip,
            transport,
            created_at: now,
            last_activity: now,
        }
    }

    /// Update last activity timestamp
    pub fn update_activity(&mut self) {
        self.last_activity = Utc::now();
    }
}

/// SIP processing result
#[derive(Debug, Clone)]
pub enum SipRequestResult {
    /// Process the request normally
    Forward(SipMessage),
    /// Send a response
    Respond(SipMessage),
    /// Drop the request
    Drop,
    /// Error occurred
    Error(String),
}

/// Processor message for async communication
#[derive(Debug, Clone)]
pub enum ProcessorMessage {
    /// New incoming SIP message
    IncomingMessage {
        message: SipMessage,
        from: SocketAddr,
        transport: SipTransport,
    },
    /// Send outgoing SIP message
    OutgoingMessage {
        message: SipMessage,
        to: SocketAddr,
        transport: SipTransport,
    },
    /// Shutdown processor
    Shutdown,
}

/// Main SIP Core Engine
pub struct SipCoreEngine {
    /// Configuration
    config: SipCoreConfig,
    /// Transport manager
    transport_manager: Arc<SipTransportManager>,
    /// State manager
    state_manager: Arc<SipStateManager>,
    /// Authentication manager
    auth_manager: Arc<Mutex<SipAuthenticator>>,
    /// Active call contexts
    call_contexts: Arc<RwLock<HashMap<String, SipCallContext>>>,
    /// Message processor handle
    processor_handle: Option<tokio::task::JoinHandle<()>>,
}

impl SipCoreEngine {
    /// Create new SIP core engine
    pub async fn new(config: SipCoreConfig) -> Result<Self> {
        info!("Initializing SIP Core Engine with domain: {}", config.domain);

        // Initialize transport manager
        let transport_config = TransportConfig {
            transport: SipTransport::UDP,
            bind_address: SocketAddr::new(config.local_ip, config.port),
            connection_timeout: 30,
            keep_alive_interval: Some(120),
            max_message_size: 65536,
            tls_config: None,
            enabled: true,
        };
        let transport_manager = Arc::new(SipTransportManager::new(vec![transport_config])?);

        // Initialize state manager
        let state_config = SipStateConfig::default();
        let state_manager = Arc::new(SipStateManager::new(state_config));

        // Initialize authentication manager
        let auth_manager = Arc::new(Mutex::new(SipAuthenticator::new(config.domain.clone())));

        // Initialize call contexts
        let call_contexts = Arc::new(RwLock::new(HashMap::new()));

        Ok(Self {
            config,
            transport_manager,
            state_manager,
            auth_manager,
            call_contexts,
            processor_handle: None,
        })
    }

    /// Start the SIP core engine
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting SIP Core Engine on {}:{}", self.config.local_ip, self.config.port);

        // Start transport manager
        // Note: In a full implementation, this would start listening on the configured transports

        // Start message processor
        let call_contexts = Arc::clone(&self.call_contexts);
        let state_manager = Arc::clone(&self.state_manager);
        let config = self.config.clone();

        let handle = tokio::spawn(async move {
            Self::message_processor_loop(call_contexts, state_manager, config).await;
        });

        self.processor_handle = Some(handle);

        info!("SIP Core Engine started successfully");
        Ok(())
    }

    /// Stop the SIP core engine
    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping SIP Core Engine");

        if let Some(handle) = self.processor_handle.take() {
            handle.abort();
        }

        info!("SIP Core Engine stopped");
        Ok(())
    }

    /// Process incoming SIP message
    pub async fn process_message(
        &self,
        message: SipMessage,
        from_addr: SocketAddr,
        transport: SipTransport,
    ) -> Result<SipRequestResult> {
        debug!("Processing SIP message from {}", from_addr);

        // Extract call-id
        let call_id = self.extract_call_id(&message)?;

        // Extract from and to URIs
        let (from_uri, to_uri) = self.extract_uris(&message)?;

        // Get or create call context
        let mut contexts = self.call_contexts.write().await;
        let context = contexts.entry(call_id.clone()).or_insert_with(|| {
            SipCallContext::new(
                call_id.clone(),
                from_uri.clone(),
                to_uri.clone(),
                from_addr,
                transport,
            )
        });
        context.update_activity();

        // Process based on method if this is a request
        if let rsip::message::SipMessage::Request(ref request) = message.message {
            match request.method {
                Method::Invite => self.process_invite(message, context).await,
                Method::Ack => self.process_ack(message, context).await,
                Method::Bye => self.process_bye(message, context).await,
                Method::Cancel => self.process_cancel(message, context).await,
                Method::Options => self.process_options(message, context).await,
                Method::Register => self.process_register(message, context).await,
                Method::Info => self.process_info(message, context).await,
                _ => {
                    warn!("Unsupported SIP method: {:?}", request.method);
                    Ok(SipRequestResult::Error("Method not supported".to_string()))
                }
            }
        } else {
            // This is a response - forward it
            Ok(SipRequestResult::Forward(message))
        }
    }

    /// Process INVITE request
    async fn process_invite(&self, message: SipMessage, _context: &mut SipCallContext) -> Result<SipRequestResult> {
        debug!("Processing INVITE request");

        // Basic INVITE processing - in a full implementation this would:
        // 1. Validate the request
        // 2. Perform authentication if required
        // 3. Route the call
        // 4. Establish media session
        // 5. Send appropriate response

        // For now, return forward for further processing
        Ok(SipRequestResult::Forward(message))
    }

    /// Process ACK request
    async fn process_ack(&self, message: SipMessage, _context: &mut SipCallContext) -> Result<SipRequestResult> {
        debug!("Processing ACK request");
        Ok(SipRequestResult::Forward(message))
    }

    /// Process BYE request
    async fn process_bye(&self, message: SipMessage, context: &mut SipCallContext) -> Result<SipRequestResult> {
        debug!("Processing BYE request for call: {}", context.call_id);

        // Clean up call context
        let mut contexts = self.call_contexts.write().await;
        contexts.remove(&context.call_id);

        Ok(SipRequestResult::Forward(message))
    }

    /// Process CANCEL request
    async fn process_cancel(&self, message: SipMessage, _context: &mut SipCallContext) -> Result<SipRequestResult> {
        debug!("Processing CANCEL request");
        Ok(SipRequestResult::Forward(message))
    }

    /// Process OPTIONS request
    async fn process_options(&self, message: SipMessage, _context: &mut SipCallContext) -> Result<SipRequestResult> {
        debug!("Processing OPTIONS request");

        // For now, just forward the OPTIONS request
        // In a full implementation, this would create a proper 200 OK response with capabilities
        Ok(SipRequestResult::Forward(message))
    }

    /// Process REGISTER request
    async fn process_register(&self, message: SipMessage, _context: &mut SipCallContext) -> Result<SipRequestResult> {
        debug!("Processing REGISTER request");

        // For now, just forward the REGISTER request
        // In a full implementation, this would:
        // 1. Perform authentication using auth_manager.authenticate_request()
        // 2. Update registration database
        // 3. Send appropriate response
        Ok(SipRequestResult::Forward(message))
    }

    /// Process INFO request
    async fn process_info(&self, message: SipMessage, _context: &mut SipCallContext) -> Result<SipRequestResult> {
        debug!("Processing INFO request");
        Ok(SipRequestResult::Forward(message))
    }

    /// Extract call-id from SIP message
    fn extract_call_id(&self, message: &SipMessage) -> Result<String> {
        match &message.message {
            rsip::message::SipMessage::Request(request) => {
                let call_id = request.call_id_header()
                    .map_err(|_| anyhow!("Missing Call-ID header in request"))?
                    .value();
                Ok(call_id.to_string())
            },
            rsip::message::SipMessage::Response(response) => {
                let call_id = response.call_id_header()
                    .map_err(|_| anyhow!("Missing Call-ID header in response"))?
                    .value();
                Ok(call_id.to_string())
            }
        }
    }

    /// Extract From and To URIs from SIP message
    fn extract_uris(&self, message: &SipMessage) -> Result<(String, String)> {
        match &message.message {
            rsip::message::SipMessage::Request(request) => {
                let from_uri = request.from_header()
                    .map(|h| h.uri().map(|u| u.to_string()).unwrap_or_else(|_| "unknown".to_string()))
                    .unwrap_or_else(|_| "unknown".to_string());
                let to_uri = request.to_header()
                    .map(|h| h.uri().map(|u| u.to_string()).unwrap_or_else(|_| "unknown".to_string()))
                    .unwrap_or_else(|_| "unknown".to_string());
                Ok((from_uri, to_uri))
            },
            rsip::message::SipMessage::Response(response) => {
                let from_uri = response.from_header()
                    .map(|h| h.uri().map(|u| u.to_string()).unwrap_or_else(|_| "unknown".to_string()))
                    .unwrap_or_else(|_| "unknown".to_string());
                let to_uri = response.to_header()
                    .map(|h| h.uri().map(|u| u.to_string()).unwrap_or_else(|_| "unknown".to_string()))
                    .unwrap_or_else(|_| "unknown".to_string());
                Ok((from_uri, to_uri))
            }
        }
    }

    /// Get call context by call-id
    pub async fn get_call_context(&self, call_id: &str) -> Option<SipCallContext> {
        let contexts = self.call_contexts.read().await;
        contexts.get(call_id).cloned()
    }

    /// Get all active call contexts
    pub async fn get_active_calls(&self) -> Vec<SipCallContext> {
        let contexts = self.call_contexts.read().await;
        contexts.values().cloned().collect()
    }

    /// Get engine statistics
    pub async fn get_statistics(&self) -> HashMap<String, u64> {
        let contexts = self.call_contexts.read().await;
        let mut stats = HashMap::new();
        stats.insert("active_calls".to_string(), contexts.len() as u64);
        stats
    }

    /// Message processor loop
    async fn message_processor_loop(
        _call_contexts: Arc<RwLock<HashMap<String, SipCallContext>>>,
        _state_manager: Arc<SipStateManager>,
        _config: SipCoreConfig,
    ) {
        info!("SIP message processor loop started");

        // In a full implementation, this would:
        // 1. Listen for incoming messages from transports
        // 2. Process messages through the state machine
        // 3. Handle timeouts and retransmissions
        // 4. Manage call state transitions

        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            // Placeholder for actual message processing
        }
    }
}

impl Drop for SipCoreEngine {
    fn drop(&mut self) {
        if let Some(handle) = self.processor_handle.take() {
            handle.abort();
        }
    }
}

/// Compliance notifier trait for lawful intercept integration
pub trait ComplianceNotifier: Send + Sync {
    /// Notify of call attempt
    fn notify_call_attempt(&self, context: &SipCallContext, source_ip: IpAddr);

    /// Notify of call establishment
    fn notify_call_established(&self, context: &SipCallContext);

    /// Notify of call termination
    fn notify_call_terminated(&self, context: &SipCallContext, reason: &str);

    /// Notify of SIP method processing
    fn notify_sip_method(&self, call_id: &str, method: &str, response_code: Option<u16>, source_ip: IpAddr);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sip_core_engine_creation() {
        let config = SipCoreConfig::default();
        let engine = SipCoreEngine::new(config).await;
        assert!(engine.is_ok());
    }

    #[tokio::test]
    async fn test_call_context_creation() {
        let call_id = "test-call-123".to_string();
        let from_uri = "sip:alice@example.com".to_string();
        let to_uri = "sip:bob@example.com".to_string();
        let source_ip = "127.0.0.1:5060".parse().unwrap();
        let transport = SipTransport::UDP;

        let context = SipCallContext::new(call_id.clone(), from_uri.clone(), to_uri.clone(), source_ip, transport);

        assert_eq!(context.call_id, call_id);
        assert_eq!(context.from_uri, from_uri);
        assert_eq!(context.to_uri, to_uri);
        assert_eq!(context.source_ip, source_ip);
    }

    #[tokio::test]
    async fn test_call_context_activity_update() {
        let mut context = SipCallContext::new(
            "test".to_string(),
            "from".to_string(),
            "to".to_string(),
            "127.0.0.1:5060".parse().unwrap(),
            SipTransport::UDP
        );

        let original_activity = context.last_activity;
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        context.update_activity();
        assert!(context.last_activity > original_activity);
    }
}