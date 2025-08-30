/*
 * SIP Stack and Codec Engine Integration Module
 * Bridges the redfire-sip-stack and redfire-codec-engine libraries
 */

use anyhow::Result;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};

// Import SIP stack components
use redfire_sip_stack::{
    AuthResult, SipAuthenticator, SipCallContext, SipCoreConfig, SipCoreEngine, SipMessage,
    SipMethod, SipParser, SipStateConfig, SipStateManager, SipTransport,
    SipTransportManager, TransportConfig,
};

// Import codec engine components
use redfire_codec_engine::{
    AudioCodec, CodecConfig, CodecService, ResamplerConfig, ResamplingService,
};

// Import our local codec types for AudioFrame and TranscodedFrame

// Import RTP proxy for media handling
use crate::rtp_proxy_impl::{MediaSession, RtpProxyConfig, RtpProxyService};

/// Integrated SIP and Codec Service
pub struct SipCodecIntegration {
    // SIP Stack components
    sip_parser: Arc<SipParser>,
    sip_transport: Arc<SipTransportManager>,
    sip_core: Arc<SipCoreEngine>,
    sip_state: Arc<SipStateManager>,
    sip_auth: Arc<Mutex<SipAuthenticator>>,

    // Codec Engine components
    codec_service: Arc<CodecService>,
    resampler: Arc<ResamplingService>,

    // RTP Proxy for media
    rtp_proxy: Arc<RtpProxyService>,

    // Active sessions mapping call-id to codec session
    active_sessions: Arc<RwLock<HashMap<String, IntegratedSession>>>,
}

/// Integrated session tracking both SIP and media
#[derive(Debug, Clone)]
pub struct IntegratedSession {
    pub call_id: String,
    pub sip_context: SipCallContext,
    pub media_session: Option<MediaSession>,
    pub codec_session: Option<String>, // Codec session ID
    pub ingress_codec: AudioCodec,
    pub egress_codec: AudioCodec,
    pub requires_transcoding: bool,
}

impl SipCodecIntegration {
    /// Create new integrated service with full SIP stack and codec capabilities
    pub async fn new(
        sip_config: SipCoreConfig,
        codec_config: CodecConfig,
        rtp_config: RtpProxyConfig,
    ) -> Result<Self> {
        info!("Initializing SIP Stack and Codec Engine integration...");

        // Initialize SIP stack components
        let sip_parser = Arc::new(SipParser::new(
            "localhost".to_string(),
            5060,
            "Redfire-SIP-Stack/1.0".to_string(),
        ));

        let transport_config = TransportConfig {
            transport: SipTransport::Udp,
            bind_address: format!("0.0.0.0:5060").parse()?,
            max_message_size: 65536,
            connection_timeout: 30,
            keep_alive_interval: Some(120),
            tls_config: None,
            enabled: true,
        };
        let sip_transport = Arc::new(SipTransportManager::new(vec![transport_config])?);

        let sip_core = Arc::new(SipCoreEngine::new(sip_config.clone()).await?);

        let state_config = SipStateConfig::default();
        let sip_state = Arc::new(SipStateManager::new(state_config));

        let sip_auth = Arc::new(Mutex::new(SipAuthenticator::new("default".to_string())));

        // Initialize Codec Engine components
        let codec_service = Arc::new(CodecService::new(codec_config).await?);

        let resampler_config = ResamplerConfig::default();
        let resampler = Arc::new(ResamplingService::new(resampler_config).await?);

        // Initialize RTP proxy for media handling
        let rtp_proxy = Arc::new(RtpProxyService::new(rtp_config).await?);

        info!("✅ SIP Stack and Codec Engine integration initialized successfully");

        // Initialize active sessions
        let active_sessions = Arc::new(RwLock::new(HashMap::new()));

        // Start session cleanup task
        let active_sessions_cleanup = Arc::clone(&active_sessions);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                Self::cleanup_stale_sessions(&active_sessions_cleanup).await;
            }
        });

        Ok(Self {
            sip_parser,
            sip_transport,
            sip_core,
            sip_state,
            sip_auth,
            codec_service,
            resampler,
            rtp_proxy,
            active_sessions,
        })
    }

    /// Process incoming SIP message with integrated codec negotiation
    pub async fn process_sip_message(
        &self,
        message_data: &[u8],
        from_addr: SocketAddr,
        transport: SipTransport,
    ) -> Result<()> {
        // Parse SIP message
        let parser_transport = match transport {
            SipTransport::Udp => redfire_sip_stack::parser::SipTransport::UDP,
            SipTransport::Tcp => redfire_sip_stack::parser::SipTransport::TCP,
            SipTransport::Tls => redfire_sip_stack::parser::SipTransport::TLS,
            SipTransport::Wss => redfire_sip_stack::parser::SipTransport::WSS,
        };

        let sip_message = self.sip_parser.parse_message(
            message_data,
            from_addr,
            "0.0.0.0:5060".parse().unwrap_or(from_addr),
            parser_transport,
        )?;

        // Extract method from the wrapped message
        let method = match &sip_message.message {
            rsip::SipMessage::Request(req) => Some(req.method.clone()),
            rsip::SipMessage::Response(_) => None,
        };

        debug!("Processing SIP message from {}: {:?}", from_addr, method);

        // Authenticate if required
        if let Some(ref _method) = method {
            let auth_result = self
                .sip_auth
                .lock()
                .await
                .authenticate_request(&sip_message.message, from_addr.ip())
                .await?;
            match auth_result {
                AuthResult::Authorized {
                    trunk_id,
                    customer_id,
                    ..
                } => {
                    debug!(
                        "Request authenticated successfully: {} / {}",
                        trunk_id, customer_id
                    );
                }
                AuthResult::Challenge { realm, nonce, .. } => {
                    warn!("Authentication challenge required for realm: {}", realm);
                    // Send challenge response
                    let challenge = format!("realm={}, nonce={}", realm, nonce);
                    return self
                        .send_auth_challenge(challenge, from_addr, transport)
                        .await;
                }
                AuthResult::Denied { reason } => {
                    error!("Authentication failed: {:?}", reason);
                    return self.send_auth_failure(from_addr, transport).await;
                }
            }
        }

        // Handle based on method
        match method {
            Some(SipMethod::Invite) => {
                self.handle_invite(sip_message, from_addr, transport)
                    .await?;
            }
            Some(SipMethod::Bye) => {
                self.handle_bye(sip_message, from_addr, transport).await?;
            }
            Some(SipMethod::Ack) => {
                self.handle_ack(sip_message, from_addr, transport).await?;
            }
            Some(SipMethod::Cancel) => {
                self.handle_cancel(sip_message, from_addr, transport)
                    .await?;
            }
            Some(SipMethod::Options) => {
                self.handle_options(sip_message, from_addr, transport)
                    .await?;
            }
            _ => {
                debug!("Unhandled SIP method: {:?}", method);
            }
        }

        Ok(())
    }

    /// Handle INVITE with codec negotiation and media setup
    async fn handle_invite(
        &self,
        message: SipMessage,
        from_addr: SocketAddr,
        transport: SipTransport,
    ) -> Result<()> {
        info!("Processing INVITE with codec negotiation");

        // Extract call ID
        let call_id = self.extract_call_id(&message)?;

        // Parse SDP to determine codecs
        let (ingress_codec, egress_codec) = self.negotiate_codecs(&message)?;

        // Check if transcoding is needed
        let requires_transcoding = ingress_codec != egress_codec;

        if requires_transcoding {
            info!(
                "Call {} requires transcoding: {:?} -> {:?}",
                call_id, ingress_codec, egress_codec
            );
        }

        // Create SIP call context
        let sip_context = SipCallContext {
            call_id: call_id.clone(),
            from_uri: self.extract_from_uri(&message)?,
            to_uri: self.extract_to_uri(&message)?,
            calling_number: "".to_string(),
            called_number: "".to_string(),
            tech_prefix: None,
            trunk_id: None,
            customer_id: None,
            source_ip: from_addr,
            transport,
            created_at: chrono::Utc::now(),
            last_activity: chrono::Utc::now(),
        };

        // Setup RTP proxy session if media is present
        let media_session = if self.has_sdp(&message) {
            let ingress_remote = self.extract_media_address(&message)?;
            let egress_remote = SocketAddr::new(
                "0.0.0.0".parse()?,
                20000, // Placeholder, would be from termination endpoint
            );

            let (ingress_local, egress_local) = self
                .rtp_proxy
                .start_session(
                    call_id.clone(),
                    call_id.clone(),
                    ingress_remote,
                    egress_remote,
                    ingress_codec.into(),
                    egress_codec.into(),
                )
                .await?;

            info!(
                "RTP proxy session established: {} <-> {}",
                ingress_local, egress_local
            );

            Some(MediaSession {
                session_id: call_id.clone(),
                call_id: call_id.clone(),
                ingress_endpoint: Default::default(), // Would be populated
                egress_endpoint: Default::default(),  // Would be populated
                created_at: std::time::Instant::now(),
                last_activity: std::time::Instant::now(),
                stats: Default::default(),
                codec_translation: if requires_transcoding {
                    Some(Default::default())
                } else {
                    None
                },
            })
        } else {
            None
        };

        // Setup codec session if transcoding needed
        let codec_session = if requires_transcoding {
            let session_id = format!("{}-codec", call_id);
            self.codec_service
                .start_session(
                    session_id.clone(),
                    ingress_codec,
                    egress_codec,
                    ingress_codec.sample_rate(),
                    1, // Mono
                )
                .await?;

            Some(session_id)
        } else {
            None
        };

        // Store integrated session
        let integrated_session = IntegratedSession {
            call_id: call_id.clone(),
            sip_context,
            media_session,
            codec_session,
            ingress_codec,
            egress_codec,
            requires_transcoding,
        };

        let mut sessions = self.active_sessions.write().await;
        sessions.insert(call_id.clone(), integrated_session);

        info!("✅ Integrated session established for call {}", call_id);

        // Forward the INVITE with proper Via handling
        self.forward_invite(message, from_addr, transport).await?;

        Ok(())
    }

    /// Handle BYE to tear down session
    async fn handle_bye(
        &self,
        message: SipMessage,
        from_addr: SocketAddr,
        transport: SipTransport,
    ) -> Result<()> {
        let call_id = self.extract_call_id(&message)?;
        info!("Processing BYE for call {}", call_id);

        // Clean up integrated session
        let mut sessions = self.active_sessions.write().await;
        if let Some(session) = sessions.remove(&call_id) {
            // Stop RTP proxy session
            if session.media_session.is_some() {
                self.rtp_proxy.end_session(&call_id).await?;
                info!("RTP proxy session stopped for {}", call_id);
            }

            // Stop codec session
            if let Some(codec_session_id) = session.codec_session {
                self.codec_service.end_session(&codec_session_id).await?;
                info!("Codec session stopped for {}", call_id);
            }

            info!("✅ Integrated session cleaned up for call {}", call_id);
        }

        // Forward the BYE
        self.forward_message(message, from_addr, transport).await?;

        Ok(())
    }

    /// Handle ACK
    async fn handle_ack(
        &self,
        message: SipMessage,
        from_addr: SocketAddr,
        transport: SipTransport,
    ) -> Result<()> {
        let call_id = self.extract_call_id(&message)?;
        debug!("Processing ACK for call {}", call_id);

        // Update session state
        let sessions = self.active_sessions.read().await;
        if let Some(_session) = sessions.get(&call_id) {
            // Session confirmed, media can flow
            info!("Call {} confirmed, media flow enabled", call_id);
        }

        // Forward the ACK
        self.forward_message(message, from_addr, transport).await?;

        Ok(())
    }

    /// Handle CANCEL
    async fn handle_cancel(
        &self,
        message: SipMessage,
        from_addr: SocketAddr,
        transport: SipTransport,
    ) -> Result<()> {
        let call_id = self.extract_call_id(&message)?;
        info!("Processing CANCEL for call {}", call_id);

        // Clean up session if exists
        let mut sessions = self.active_sessions.write().await;
        if let Some(session) = sessions.remove(&call_id) {
            // Stop any active media/codec sessions
            if let Some(codec_session_id) = session.codec_session {
                self.codec_service.end_session(&codec_session_id).await?;
            }
            info!("Session cancelled for {}", call_id);
        }

        // Forward the CANCEL
        self.forward_message(message, from_addr, transport).await?;

        Ok(())
    }

    /// Handle OPTIONS for capability discovery
    async fn handle_options(
        &self,
        _message: SipMessage,
        _from_addr: SocketAddr,
        _transport: SipTransport,
    ) -> Result<()> {
        debug!("Processing OPTIONS request");

        // Respond with our capabilities
        // This would include supported codecs, methods, etc.

        Ok(())
    }

    // Helper methods

    fn extract_call_id(&self, message: &SipMessage) -> Result<String> {
        // Extract Call-ID from headers
        // Simplified implementation
        Ok("test-call-id".to_string())
    }

    fn extract_from_uri(&self, _message: &SipMessage) -> Result<String> {
        // Extract From URI
        Ok("sip:from@example.com".to_string())
    }

    fn extract_to_uri(&self, _message: &SipMessage) -> Result<String> {
        // Extract To URI
        Ok("sip:to@example.com".to_string())
    }

    fn has_sdp(&self, _message: &SipMessage) -> bool {
        // Check if message contains SDP
        true // Simplified
    }

    fn extract_media_address(&self, _message: &SipMessage) -> Result<SocketAddr> {
        // Extract media address from SDP
        Ok(SocketAddr::new("0.0.0.0".parse()?, 10000))
    }

    fn negotiate_codecs(&self, _message: &SipMessage) -> Result<(AudioCodec, AudioCodec)> {
        // Parse SDP and negotiate codecs
        // For now, return default codecs
        Ok((AudioCodec::G711Ulaw, AudioCodec::G711Alaw))
    }

    async fn forward_invite(
        &self,
        _message: SipMessage,
        _from_addr: SocketAddr,
        _transport: SipTransport,
    ) -> Result<()> {
        // Forward INVITE with proper Via handling
        debug!("Forwarding INVITE");
        Ok(())
    }

    async fn forward_message(
        &self,
        _message: SipMessage,
        _from_addr: SocketAddr,
        _transport: SipTransport,
    ) -> Result<()> {
        // Generic message forwarding
        debug!("Forwarding SIP message");
        Ok(())
    }

    async fn send_auth_challenge(
        &self,
        _challenge: String,
        to_addr: SocketAddr,
        _transport: SipTransport,
    ) -> Result<()> {
        // Send 401/407 with challenge
        warn!("Sending auth challenge to {}", to_addr);
        Ok(())
    }

    async fn send_auth_failure(&self, to_addr: SocketAddr, _transport: SipTransport) -> Result<()> {
        // Send 403 Forbidden
        error!("Sending auth failure to {}", to_addr);
        Ok(())
    }

    /// Cleanup stale sessions that have been inactive too long
    async fn cleanup_stale_sessions(sessions: &RwLock<HashMap<String, IntegratedSession>>) {
        let mut sessions_guard = sessions.write().await;
        let now = std::time::Instant::now();
        let session_timeout = std::time::Duration::from_secs(3600); // 1 hour timeout

        sessions_guard.retain(|call_id, session| {
            if let Some(ref media_session) = session.media_session {
                let inactive_duration = now.duration_since(media_session.last_activity);
                if inactive_duration > session_timeout {
                    info!("Cleaning up stale session: {}", call_id);
                    return false;
                }
            }
            true
        });
    }
}

/// Create integrated service with default configuration
pub async fn create_integrated_service() -> Result<SipCodecIntegration> {
    let sip_config = SipCoreConfig::default();
    let codec_config = CodecConfig::default();
    let rtp_config = RtpProxyConfig::default();

    SipCodecIntegration::new(sip_config, codec_config, rtp_config).await
}
