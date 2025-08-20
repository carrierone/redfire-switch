/*
 * Redfire Switch - Phase 1: Core SIP Stack Implementation
 * Copyright (C) 2025 Carrier One Inc and contributors
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

//! # Phase 1: Core SIP Stack
//! 
//! This module implements Phase 1 of the dependency optimization plan:
//! - Core SIP Stack with IP-based authentication
//! - Transaction management
//! - Dialog management  
//! - Transport layer (UDP/TCP/TLS)
//! - Basic authentication with tech prefix support

use crate::parser::{SipMessage, SipParser};
use crate::state::{SipStateAction, SipStateManager};
use crate::authentication::{SipAuthenticator, AuthResult};
use crate::transport::{SipTransport, SipTransportManager, TransportConfig, TransportEvent, TransportMessage};
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, info, warn, error, instrument};
use dashmap::DashMap;
use uuid::Uuid;

/// Trait for CALEA compliance notifications from SIP stack
pub trait ComplianceNotifier {
    /// Notify of call attempt (INVITE received)
    fn notify_call_attempt(&self, context: &SipCallContext, source_ip: std::net::IpAddr);
    
    /// Notify of call establishment (200 OK sent/received)
    fn notify_call_established(&self, context: &SipCallContext);
    
    /// Notify of call termination (BYE/error response)
    fn notify_call_terminated(&self, context: &SipCallContext, termination_reason: &str);
    
    /// Notify of SIP method processing (for CDR generation)
    fn notify_sip_method(&self, call_id: &str, method: &str, response_code: Option<u16>, source_ip: std::net::IpAddr);
}

/// CALEA compliance event types
#[derive(Debug, Clone)]
enum ComplianceEventType {
    CallAttempt { source_ip: std::net::IpAddr },
    CallEstablished,
    CallTerminated { reason: String },
    SipMethod { method: String, response_code: Option<u16>, source_ip: std::net::IpAddr },
}

/// Extra compliance data
#[derive(Debug, Clone)]
struct ComplianceExtraData {
    pub method: Option<String>,
    pub response_code: Option<u16>,
    pub source_ip: Option<std::net::IpAddr>,
}

/// Core SIP engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipCoreConfig {
    /// Transport configurations
    pub transports: Vec<TransportConfig>,
    /// Authentication realm
    pub auth_realm: String,
    /// Enable strict RFC compliance
    pub strict_rfc_compliance: bool,
    /// Maximum concurrent transactions
    pub max_transactions: u32,
    /// Maximum concurrent dialogs
    pub max_dialogs: u32,
    /// Transaction timeout (seconds)
    pub transaction_timeout: u64,
    /// Dialog timeout (seconds)
    pub dialog_timeout: u64,
    /// Enable authentication
    pub enable_authentication: bool,
    /// User agent string
    pub user_agent: String,
}

impl Default for SipCoreConfig {
    fn default() -> Self {
        Self {
            transports: vec![
                TransportConfig {
                    transport: SipTransport::Udp,
                    bind_address: "0.0.0.0:5060".parse().expect("Default UDP bind address must be valid"),
                    max_message_size: 8192,
                    connection_timeout: 30,
                    keep_alive_interval: Some(60),
                    tls_config: None,
                    enabled: true,
                },
                TransportConfig {
                    transport: SipTransport::Tcp,
                    bind_address: "0.0.0.0:5060".parse().expect("Default TCP bind address must be valid"),
                    max_message_size: 65536,
                    connection_timeout: 30,
                    keep_alive_interval: Some(300),
                    tls_config: None,
                    enabled: true,
                },
            ],
            auth_realm: "redfire.switch".to_string(),
            strict_rfc_compliance: true,
            max_transactions: 100000,
            max_dialogs: 50000,
            transaction_timeout: 32, // RFC 3261 Timer B
            dialog_timeout: 3600,    // 1 hour
            enable_authentication: true,
            user_agent: "Redfire-Switch/1.0".to_string(),
        }
    }
}

/// SIP call context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipCallContext {
    /// Call ID
    pub call_id: String,
    /// From URI
    pub from_uri: String,
    /// To URI
    pub to_uri: String,
    /// Calling number (extracted)
    pub calling_number: String,
    /// Called number (extracted)
    pub called_number: String,
    /// Tech prefix (if any)
    pub tech_prefix: Option<String>,
    /// Trunk ID (from authentication)
    pub trunk_id: Option<String>,
    /// Customer ID (from authentication)
    pub customer_id: Option<String>,
    /// Source IP
    pub source_ip: SocketAddr,
    /// Transport used
    pub transport: SipTransport,
    /// Call creation time
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last activity
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

/// SIP request processing result
#[derive(Debug, Clone)]
pub enum SipRequestResult {
    /// Request processed successfully, forward to routing engine
    RouteCall {
        context: SipCallContext,
        original_request: rsip::Request,
    },
    /// Send SIP response immediately
    SendResponse {
        response: rsip::Response,
        destination: SocketAddr,
        transport: SipTransport,
    },
    /// Authentication challenge required
    AuthChallenge {
        challenge_response: rsip::Response,
        destination: SocketAddr,
        transport: SipTransport,
    },
    /// Drop request silently
    Drop {
        reason: String,
    },
}

/// Core SIP engine
pub struct SipCoreEngine {
    /// Configuration
    config: SipCoreConfig,
    /// Transport manager
    transport_manager: Arc<SipTransportManager>,
    /// SIP parser
    parser: Arc<SipParser>,
    /// State manager
    state_manager: Arc<SipStateManager>,
    /// Authenticator
    authenticator: Arc<RwLock<SipAuthenticator>>,
    /// Active calls
    active_calls: Arc<DashMap<String, SipCallContext>>,
    /// Message processor
    message_processor: mpsc::UnboundedSender<ProcessorMessage>,
    /// CALEA compliance framework for U.S. lawful intercept
    compliance_framework: Option<Arc<dyn ComplianceNotifier + Send + Sync>>,
}

/// Internal processor messages
#[derive(Debug)]
pub enum ProcessorMessage {
    TransportEvent(TransportEvent),
    ProcessRequest {
        message: TransportMessage,
        callback: tokio::sync::oneshot::Sender<SipRequestResult>,
    },
}

impl SipCoreEngine {
    /// Create new SIP core engine
    #[instrument(skip(config))]
    pub async fn new(config: SipCoreConfig) -> Result<Self> {
        info!("Initializing SIP core engine");
        
        // Create transport manager
        let transport_manager = Arc::new(SipTransportManager::new(config.transports.clone())?);
        
        // Create SIP parser
        let parser = Arc::new(SipParser::new(
            config.transports.first()
                .map(|t| t.bind_address.ip().to_string())
                .unwrap_or_else(|| "127.0.0.1".to_string()),
            config.transports.first()
                .map(|t| t.bind_address.port())
                .unwrap_or(5060),
            config.user_agent.clone(),
        ));
        
        // Create state manager with default config
        let state_config = crate::state::SipStateConfig::default();
        let state_manager = Arc::new(SipStateManager::new(state_config));
        
        // Create authenticator
        let mut authenticator = SipAuthenticator::new(config.auth_realm.clone());
        
        // Load authentication configurations
        if config.enable_authentication {
            let ip_configs = crate::authentication::load_ip_auth_configs().await?;
            for ip_config in ip_configs {
                authenticator.add_ip_auth_config(ip_config);
            }
        }
        
        let authenticator = Arc::new(RwLock::new(authenticator));
        
        // Create message processor channel
        let (message_processor, processor_receiver) = mpsc::unbounded_channel();
        
        let engine = Self {
            config,
            transport_manager,
            parser,
            state_manager,
            authenticator,
            active_calls: Arc::new(DashMap::new()),
            message_processor,
            compliance_framework: None,
        };
        
        // Start message processor task
        let processor = SipMessageProcessor::new(
            engine.parser.clone(),
            engine.state_manager.clone(),
            engine.authenticator.clone(),
            engine.active_calls.clone(),
            engine.config.clone(),
            engine.compliance_framework.clone(),
        );
        
        tokio::spawn(async move {
            processor.run(processor_receiver).await;
        });
        
        info!("SIP core engine initialized successfully");
        Ok(engine)
    }
    
    /// Start the SIP core engine
    #[instrument(skip(self))]
    pub async fn start(&self) -> Result<()> {
        info!("Starting SIP core engine");
        
        // Start transport manager
        self.transport_manager.start().await?;
        
        // Start transport event processor
        let event_receiver = self.transport_manager.get_event_receiver().await;
        let message_processor = self.message_processor.clone();
        
        tokio::spawn(async move {
            let mut receiver = event_receiver.write().await;
            while let Some(event) = receiver.recv().await {
                if let Err(e) = message_processor.send(ProcessorMessage::TransportEvent(event)) {
                    error!("Failed to send transport event to processor: {}", e);
                    break;
                }
            }
        });
        
        info!("SIP core engine started successfully");
        Ok(())
    }
    
    /// Process incoming SIP request
    pub async fn process_request(&self, message: TransportMessage) -> Result<SipRequestResult> {
        let (callback_sender, callback_receiver) = tokio::sync::oneshot::channel();
        
        self.message_processor.send(ProcessorMessage::ProcessRequest {
            message,
            callback: callback_sender,
        }).map_err(|e| anyhow!("Failed to send request to processor: {}", e))?;
        
        callback_receiver.await
            .map_err(|e| anyhow!("Failed to receive response from processor: {}", e))
    }
    
    /// Get call context
    pub async fn get_call_context(&self, call_id: &str) -> Option<SipCallContext> {
        self.active_calls.get(call_id).map(|ctx| ctx.clone())
    }
    
    /// Set CALEA compliance framework for lawful intercept
    pub fn set_compliance_framework(&mut self, framework: Arc<dyn ComplianceNotifier + Send + Sync>) {
        self.compliance_framework = Some(framework);
        info!("CALEA compliance framework integrated with SIP stack");
    }
    
    /// Notify compliance framework of call events
    fn notify_compliance(&self, event_type: ComplianceEventType, context: &SipCallContext, extra_data: Option<ComplianceExtraData>) {
        if let Some(ref framework) = self.compliance_framework {
            match event_type {
                ComplianceEventType::CallAttempt { source_ip } => {
                    framework.notify_call_attempt(context, source_ip);
                }
                ComplianceEventType::CallEstablished => {
                    framework.notify_call_established(context);
                }
                ComplianceEventType::CallTerminated { reason } => {
                    framework.notify_call_terminated(context, &reason);
                }
                ComplianceEventType::SipMethod { method, response_code, source_ip } => {
                    framework.notify_sip_method(&context.call_id, &method, response_code, source_ip);
                }
            }
        }
    }
    
    /// List active calls
    pub async fn list_active_calls(&self) -> Vec<SipCallContext> {
        self.active_calls.iter().map(|entry| entry.value().clone()).collect()
    }
    
    /// Send SIP response
    pub async fn send_response(&self, response: rsip::Response, destination: SocketAddr, transport: SipTransport) -> Result<()> {
        self.transport_manager.send_message(&rsip::SipMessage::Response(response), destination, transport).await
    }
    
    /// Send SIP request
    pub async fn send_request(&self, request: rsip::Request, destination: SocketAddr, transport: SipTransport) -> Result<()> {
        self.transport_manager.send_message(&rsip::SipMessage::Request(request), destination, transport).await
    }
}

/// SIP message processor
struct SipMessageProcessor {
    parser: Arc<SipParser>,
    state_manager: Arc<SipStateManager>,
    authenticator: Arc<RwLock<SipAuthenticator>>,
    active_calls: Arc<DashMap<String, SipCallContext>>,
    config: SipCoreConfig,
    compliance_framework: Option<Arc<dyn ComplianceNotifier + Send + Sync>>,
}

impl SipMessageProcessor {
    fn new(
        parser: Arc<SipParser>,
        state_manager: Arc<SipStateManager>,
        authenticator: Arc<RwLock<SipAuthenticator>>,
        active_calls: Arc<DashMap<String, SipCallContext>>,
        config: SipCoreConfig,
        compliance_framework: Option<Arc<dyn ComplianceNotifier + Send + Sync>>,
    ) -> Self {
        Self {
            parser,
            state_manager,
            authenticator,
            active_calls,
            config,
            compliance_framework,
        }
    }
    
    async fn run(self, mut receiver: mpsc::UnboundedReceiver<ProcessorMessage>) {
        info!("Starting SIP message processor");
        
        while let Some(message) = receiver.recv().await {
            match message {
                ProcessorMessage::TransportEvent(event) => {
                    self.handle_transport_event(event).await;
                },
                ProcessorMessage::ProcessRequest { message, callback } => {
                    let result = self.process_sip_request(message).await;
                    if let Err(e) = callback.send(result) {
                        error!("Failed to send processing result: {:?}", e);
                    }
                },
            }
        }
        
        warn!("SIP message processor stopped");
    }
    
    async fn handle_transport_event(&self, event: TransportEvent) {
        match event {
            TransportEvent::MessageReceived { message } => {
                debug!("Received SIP message from {}:{} via {:?}", 
                    message.source.ip(), message.source.port(), message.transport);
                
                // Process the message asynchronously
                let processor = SipMessageProcessor {
                    parser: self.parser.clone(),
                    state_manager: self.state_manager.clone(),
                    authenticator: self.authenticator.clone(),
                    active_calls: self.active_calls.clone(),
                    config: self.config.clone(),
                    compliance_framework: self.compliance_framework.clone(),
                };
                
                tokio::spawn(async move {
                    let _result = processor.process_sip_request(message).await;
                    // In a real implementation, we would handle the result appropriately
                });
            },
            TransportEvent::ConnectionEstablished { connection_id, remote_addr, transport } => {
                debug!("New connection established: {} from {} via {:?}", connection_id, remote_addr, transport);
            },
            TransportEvent::ConnectionClosed { connection_id, reason } => {
                debug!("Connection closed: {} ({})", connection_id, reason);
            },
            TransportEvent::TransportError { transport, error } => {
                error!("Transport error on {:?}: {}", transport, error);
            },
            _ => {
                debug!("Unhandled transport event: {:?}", event);
            }
        }
    }
    
    async fn process_sip_request(&self, message: TransportMessage) -> SipRequestResult {
        // Create SipMessage from TransportMessage (already parsed)
        let parser_transport = match message.transport {
            crate::transport::SipTransport::Udp => crate::parser::SipTransport::UDP,
            crate::transport::SipTransport::Tcp => crate::parser::SipTransport::TCP,
            crate::transport::SipTransport::Tls => crate::parser::SipTransport::TLS,
            crate::transport::SipTransport::Wss => crate::parser::SipTransport::WSS,
        };
        
        let sip_msg = crate::parser::SipMessage {
            message: message.message.clone(),
            source: message.source,
            destination: message.destination,
            transport: parser_transport,
            received_at: message.received_at,
            message_id: Uuid::new_v4().to_string(),
        };
        
        // Only process requests for now
        let request = match &sip_msg.message {
            rsip::SipMessage::Request(req) => req,
            rsip::SipMessage::Response(_) => {
                // Responses are handled by state manager
                return SipRequestResult::Drop {
                    reason: "Response handling not implemented".to_string(),
                };
            }
        };
        
        // Authenticate the request if enabled
        if self.config.enable_authentication {
            let auth_result = match self.authenticator.write().await.authenticate_request(&sip_msg.message, message.source.ip()).await {
                Ok(result) => result,
                Err(e) => {
                    error!("Authentication error: {}", e);
                    return self.create_error_response(request, 500, "Internal Server Error", message.source, message.transport);
                }
            };
            
            match auth_result {
                AuthResult::Authorized { trunk_id, customer_id, tech_prefix, rate_limit: _ } => {
                    debug!("Request authorized: trunk={}, customer={}", trunk_id, customer_id);
                    return self.process_authorized_request(request, &sip_msg, message, trunk_id, customer_id, tech_prefix).await;
                },
                AuthResult::Challenge { realm, nonce, algorithm } => {
                    debug!("Authentication challenge required");
                    match self.authenticator.read().await.create_challenge_response(request, algorithm) {
                        Ok(challenge) => {
                            return SipRequestResult::AuthChallenge {
                                challenge_response: challenge,
                                destination: message.source,
                                transport: message.transport,
                            };
                        },
                        Err(e) => {
                            error!("Failed to create auth challenge: {}", e);
                            return self.create_error_response(request, 500, "Internal Server Error", message.source, message.transport);
                        }
                    }
                },
                AuthResult::Denied { reason } => {
                    warn!("Request denied from {}: {:?}", message.source, reason);
                    return self.create_error_response(request, 403, "Forbidden", message.source, message.transport);
                },
            }
        } else {
            // No authentication required, process request
            return self.process_authorized_request(request, &sip_msg, message, "default".to_string(), "default".to_string(), None).await;
        }
    }
    
    async fn process_authorized_request(
        &self,
        request: &rsip::Request,
        sip_msg: &SipMessage,
        message: TransportMessage,
        trunk_id: String,
        customer_id: String,
        tech_prefix: Option<String>,
    ) -> SipRequestResult {
        // Extract call information
        let call_id = self.extract_call_id(request);
        let from_uri = self.extract_from_uri(request);
        let to_uri = self.extract_to_uri(request);
        let calling_number = self.extract_calling_number(request, &tech_prefix);
        let called_number = self.extract_called_number(request);
        
        // Create call context
        let context = SipCallContext {
            call_id: call_id.clone(),
            from_uri,
            to_uri,
            calling_number,
            called_number,
            tech_prefix,
            trunk_id: Some(trunk_id),
            customer_id: Some(customer_id),
            source_ip: message.source,
            transport: message.transport,
            created_at: chrono::Utc::now(),
            last_activity: chrono::Utc::now(),
        };
        
        // Store call context
        self.active_calls.insert(call_id.clone(), context.clone());
        
        // Process with state manager
        let state_action = match self.state_manager.process_message(sip_msg).await {
            Ok(action) => action,
            Err(e) => {
                error!("State manager error: {}", e);
                return self.create_error_response(request, 500, "Internal Server Error", message.source, message.transport);
            }
        };
        
        match state_action {
            SipStateAction::ProcessNewInvite { transaction_id: _ } => {
                info!("Processing new INVITE for call {}", call_id);
                
                // CALEA compliance: Report call attempt for U.S. lawful intercept
                if let Some(ref framework) = self.compliance_framework {
                    framework.notify_call_attempt(&context, message.source.ip());
                    debug!("CALEA: Reported call attempt for {}", call_id);
                }
                
                // Forward to routing engine
                SipRequestResult::RouteCall {
                    context,
                    original_request: request.clone(),
                }
            },
            SipStateAction::ProcessReInvite { transaction_id: _, dialog_id: _ } => {
                info!("Processing re-INVITE for call {}", call_id);
                
                // Handle re-INVITE (media change, hold, etc.)
                SipRequestResult::RouteCall {
                    context,
                    original_request: request.clone(),
                }
            },
            SipStateAction::ProcessAck { dialog_id: _ } => {
                debug!("Processing ACK for call {}", call_id);
                
                // ACK processed, no response needed
                SipRequestResult::Drop {
                    reason: "ACK processed".to_string(),
                }
            },
            SipStateAction::ProcessBye { transaction_id: _, dialog_id: _ } => {
                info!("Processing BYE for call {}", call_id);
                
                // CALEA compliance: Report call termination for U.S. lawful intercept
                if let Some(ref framework) = self.compliance_framework {
                    framework.notify_call_terminated(&context, "normal_hangup");
                    debug!("CALEA: Reported call termination for {}", call_id);
                }
                
                // Send 200 OK for BYE
                let response = self.create_ok_response(request);
                self.active_calls.remove(&call_id);
                
                SipRequestResult::SendResponse {
                    response,
                    destination: message.source,
                    transport: message.transport,
                }
            },
            SipStateAction::RetransmitLastResponse { transaction_id: _ } => {
                debug!("Retransmitting response for call {}", call_id);
                
                // This should be handled by state manager internally
                SipRequestResult::Drop {
                    reason: "Retransmission handled internally".to_string(),
                }
            },
            SipStateAction::ProcessCancel { transaction_id: _, invite_transaction_id: _ } => {
                info!("Processing CANCEL for call {}", call_id);
                
                // CALEA compliance: Report call cancellation for U.S. lawful intercept
                if let Some(ref framework) = self.compliance_framework {
                    framework.notify_call_terminated(&context, "call_cancelled");
                    debug!("CALEA: Reported call cancellation for {}", call_id);
                }
                
                // Send 200 OK for CANCEL and 487 Request Terminated for original request
                let response = self.create_ok_response(request);
                self.active_calls.remove(&call_id);
                
                SipRequestResult::SendResponse {
                    response,
                    destination: message.source,
                    transport: message.transport,
                }
            },
            SipStateAction::ProcessOtherRequest { transaction_id: _, method: _ } => {
                info!("Processing other SIP request for call {}", call_id);
                
                // Forward to routing engine for handling
                SipRequestResult::RouteCall {
                    context,
                    original_request: request.clone(),
                }
            },
            SipStateAction::ProcessResponse { transaction_id: _, dialog_id: _, requires_ack: _ } => {
                debug!("Processing SIP response for call {}", call_id);
                
                // Responses are handled by state manager internally
                SipRequestResult::Drop {
                    reason: "Response processed by state manager".to_string(),
                }
            },
            SipStateAction::DropMessage => {
                debug!("Dropping message for call {}", call_id);
                
                SipRequestResult::Drop {
                    reason: "State manager requested drop".to_string(),
                }
            },
        }
    }
    
    // Helper methods for extracting SIP information
    fn extract_call_id(&self, request: &rsip::Request) -> String {
        request.headers.iter()
            .find_map(|h| match h {
                rsip::Header::CallId(call_id) => Some(call_id.to_string()),
                _ => None,
            })
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
    }
    
    fn extract_from_uri(&self, request: &rsip::Request) -> String {
        request.headers.iter()
            .find_map(|h| match h {
                rsip::Header::From(from) => {
                    if let Ok(uri) = from.uri() {
                        Some(uri.to_string())
                    } else {
                        None
                    }
                },
                _ => None,
            })
            .unwrap_or_default()
    }
    
    fn extract_to_uri(&self, request: &rsip::Request) -> String {
        request.headers.iter()
            .find_map(|h| match h {
                rsip::Header::To(to) => {
                    if let Ok(uri) = to.uri() {
                        Some(uri.to_string())
                    } else {
                        None
                    }
                },
                _ => None,
            })
            .unwrap_or_default()
    }
    
    fn extract_calling_number(&self, request: &rsip::Request, tech_prefix: &Option<String>) -> String {
        let from_uri = self.extract_from_uri(request);
        
        // Extract user part from URI
        if let Ok(uri) = rsip::Uri::try_from(from_uri.as_str()) {
            // TODO: Extract user info from URI properly
            /*if let Some(user_info) = uri.user_info {
                let user = user_info.user;*/
                /*
                // Remove tech prefix if present
                if let Some(prefix) = tech_prefix {
                    if user.starts_with(prefix) {
                        return user[prefix.len()..].to_string();
                    }
                }
                
                return user;
            }*/
        }
        
        "unknown".to_string()
    }
    
    fn extract_called_number(&self, request: &rsip::Request) -> String {
        // TODO: Extract user part from Request-URI properly
        /*if let Some(user_info) = request.uri.user_info.as_ref() {
            user_info.user.clone()
        } else {*/
            "unknown".to_string()
        //}
    }
    
    fn create_error_response(&self, request: &rsip::Request, status_code: u16, reason: &str, destination: SocketAddr, transport: SipTransport) -> SipRequestResult {
        let status = rsip::StatusCode::try_from(status_code).unwrap_or(rsip::StatusCode::default());
        let mut response = rsip::Response::default(); // TODO: Properly set status code
        
        // Copy required headers
        for header in request.headers.iter() {
            match header {
                rsip::Header::CallId(_) | rsip::Header::CSeq(_) | rsip::Header::Via(_) => {
                    response.headers.push(header.clone());
                },
                _ => {}
            }
        }
        
        SipRequestResult::SendResponse {
            response,
            destination,
            transport,
        }
    }
    
    fn create_ok_response(&self, request: &rsip::Request) -> rsip::Response {
        let mut response = rsip::Response::default(); // TODO: Properly set status code
        
        // Copy required headers
        for header in request.headers.iter() {
            match header {
                rsip::Header::CallId(_) | rsip::Header::CSeq(_) | rsip::Header::Via(_) => {
                    response.headers.push(header.clone());
                },
                _ => {}
            }
        }
        
        response
    }
}