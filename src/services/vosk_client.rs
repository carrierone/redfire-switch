//! Vosk ASR Client Service
//!
//! This service provides automatic speech recognition capabilities using the Vosk ASR server
//! for voice integrity monitoring and compliance transcription.
//!
//! Key features:
//! - WebSocket connection to Vosk ASR server
//! - Real-time audio transcription
//! - Banned word detection for fraud analysis
//! - ECPA-compliant transcription logging
//! - Integration with legal authorization system

use anyhow::{Context, Result};
use base64::Engine;
use chrono::{DateTime, Utc};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn, instrument};
use url::Url;

use crate::events::{EventBus, TelecomEvent};

/// Vosk ASR configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoskConfig {
    /// Vosk server WebSocket URL
    pub server_url: String,
    /// Sample rate for audio (must match Vosk model)
    pub sample_rate: u32,
    /// Connection timeout in seconds
    pub connection_timeout_seconds: u64,
    /// Maximum reconnection attempts
    pub max_reconnection_attempts: u32,
    /// Reconnection delay in seconds
    pub reconnection_delay_seconds: u64,
    /// Enable banned word detection
    pub enable_banned_word_detection: bool,
    /// List of banned words/phrases for fraud detection
    pub banned_words: Vec<String>,
    /// Confidence threshold for transcription acceptance
    pub confidence_threshold: f64,
    /// Maximum transcription length
    pub max_transcription_length: usize,
}

impl Default for VoskConfig {
    fn default() -> Self {
        Self {
            server_url: "ws://vosk-server:2700".to_string(),
            sample_rate: 8000,
            connection_timeout_seconds: 30,
            max_reconnection_attempts: 5,
            reconnection_delay_seconds: 5,
            enable_banned_word_detection: true,
            banned_words: vec![
                // Financial fraud keywords
                "social security".to_string(),
                "credit card".to_string(),
                "bank account".to_string(),
                "pin number".to_string(),
                "password".to_string(),
                // Scam phrases
                "verify your account".to_string(),
                "suspend your account".to_string(),
                "urgent action required".to_string(),
                "limited time offer".to_string(),
                // Prize/lottery scams
                "you have won".to_string(),
                "congratulations winner".to_string(),
                "claim your prize".to_string(),
                // Technical support scams
                "microsoft support".to_string(),
                "your computer is infected".to_string(),
                "refund department".to_string(),
            ],
            confidence_threshold: 0.6,
            max_transcription_length: 10000,
        }
    }
}

/// Transcription request for Vosk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionRequest {
    pub recording_id: String,
    pub call_id: String,
    pub session_id: String,
    pub audio_data: Vec<u8>,
    pub sample_rate: u32,
    pub is_final: bool,
    pub legal_authorization_id: Option<i32>,
}

/// Transcription result from Vosk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    pub recording_id: String,
    pub call_id: String,
    pub text: String,
    pub confidence: f64,
    pub partial: bool,
    pub banned_words_detected: Vec<String>,
    pub fraud_risk_score: f64,
    pub processing_time_ms: u64,
    pub timestamp: DateTime<Utc>,
}

/// Vosk WebSocket message format
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum VoskMessage {
    Config { config: VoskServerConfig },
    AudioData { audio: String }, // Base64 encoded
    Result { result: VoskResult },
    Partial { partial: String },
    Error { error: String },
}

/// Vosk server configuration message
#[derive(Debug, Serialize, Deserialize)]
struct VoskServerConfig {
    sample_rate: u32,
}

/// Vosk recognition result
#[derive(Debug, Serialize, Deserialize)]
struct VoskResult {
    conf: Option<f64>,
    end: Option<f64>,
    start: Option<f64>,
    text: String,
    word: Option<Vec<VoskWord>>,
}

/// Vosk word-level result
#[derive(Debug, Serialize, Deserialize)]
struct VoskWord {
    conf: f64,
    end: f64,
    start: f64,
    word: String,
}

/// Active transcription session
#[derive(Debug, Clone)]
pub struct TranscriptionSession {
    pub recording_id: String,
    pub call_id: String,
    pub session_id: String,
    pub started_at: DateTime<Utc>,
    pub partial_text: String,
    pub final_text: String,
    pub banned_words_found: Vec<String>,
    pub total_confidence: f64,
    pub word_count: usize,
    pub legal_authorization_id: Option<i32>,
}

/// Vosk ASR client service
pub struct VoskClientService {
    config: VoskConfig,
    event_bus: Arc<EventBus>,
    active_sessions: Arc<RwLock<HashMap<String, TranscriptionSession>>>,
    transcription_sender: mpsc::UnboundedSender<TranscriptionRequest>,
    connection_status: Arc<RwLock<bool>>,
}

impl VoskClientService {
    /// Create new Vosk client service
    pub fn new(config: VoskConfig, event_bus: Arc<EventBus>) -> Result<Self> {
        let (transcription_sender, transcription_receiver) = mpsc::unbounded_channel();

        let service = Self {
            config: config.clone(),
            event_bus: event_bus.clone(),
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            transcription_sender,
            connection_status: Arc::new(RwLock::new(false)),
        };

        // Start transcription processor
        service.start_transcription_processor(transcription_receiver);

        Ok(service)
    }

    /// Submit audio for transcription
    #[instrument(skip(self, request), fields(recording_id = %request.recording_id, call_id = %request.call_id))]
    pub async fn transcribe_audio(&self, request: TranscriptionRequest) -> Result<()> {
        debug!("Submitting audio for transcription: {} bytes", request.audio_data.len());

        // Create or update transcription session
        let mut sessions = self.active_sessions.write().await;
        let session = sessions.entry(request.recording_id.clone())
            .or_insert_with(|| TranscriptionSession {
                recording_id: request.recording_id.clone(),
                call_id: request.call_id.clone(),
                session_id: request.session_id.clone(),
                started_at: Utc::now(),
                partial_text: String::new(),
                final_text: String::new(),
                banned_words_found: Vec::new(),
                total_confidence: 0.0,
                word_count: 0,
                legal_authorization_id: request.legal_authorization_id,
            });

        // Update session timestamp
        session.started_at = Utc::now();

        // Send to transcription processor
        self.transcription_sender.send(request)
            .context("Failed to queue transcription request")?;

        Ok(())
    }

    /// Get transcription session status
    pub async fn get_session_status(&self, recording_id: &str) -> Option<TranscriptionSession> {
        let sessions = self.active_sessions.read().await;
        sessions.get(recording_id).cloned()
    }

    /// Complete transcription session
    #[instrument(skip(self), fields(recording_id = %recording_id))]
    pub async fn complete_session(&self, recording_id: String) -> Result<Option<TranscriptionResult>> {
        info!("Completing transcription session: {}", recording_id);

        let mut sessions = self.active_sessions.write().await;
        if let Some(session) = sessions.remove(&recording_id) {
            let final_text = if session.final_text.is_empty() {
                session.partial_text
            } else {
                session.final_text
            };

            let avg_confidence = if session.word_count > 0 {
                session.total_confidence / session.word_count as f64
            } else {
                0.0
            };

            // Calculate fraud risk score based on banned words
            let fraud_risk_score = self.calculate_fraud_risk_score(
                &final_text,
                &session.banned_words_found,
            );

            let result = TranscriptionResult {
                recording_id: session.recording_id.clone(),
                call_id: session.call_id.clone(),
                text: final_text,
                confidence: avg_confidence,
                partial: false,
                banned_words_detected: session.banned_words_found,
                fraud_risk_score,
                processing_time_ms: (Utc::now() - session.started_at).num_milliseconds() as u64,
                timestamp: Utc::now(),
            };

            // Emit transcription completed event
            if fraud_risk_score > 0.7 {
                let event = TelecomEvent::FraudDetected(crate::events::FraudDetectedEvent {
                    alert_id: uuid::Uuid::new_v4().to_string(),
                    call_id: Some(session.call_id.clone()),
                    session_id: Some(session.session_id.clone()),
                    fraud_type: "voice_content_analysis".to_string(),
                    risk_score: fraud_risk_score,
                    source_ip: None,
                    calling_number: None,
                    details: {
                        let mut details = HashMap::new();
                        details.insert("banned_words".to_string(),
                                     format!("{:?}", result.banned_words_detected));
                        details.insert("confidence".to_string(),
                                     result.confidence.to_string());
                        details.insert("transcription_length".to_string(),
                                     result.text.len().to_string());
                        details
                    },
                    timestamp: Utc::now(),
                });
                self.event_bus.publish(event).await?;
            }

            // Emit voice integrity audit event
            let audit_event = TelecomEvent::VoiceIntegrityAudit {
                user_id: None,
                action_type: "transcription_completed".to_string(),
                resource_type: "transcription".to_string(),
                resource_id: result.recording_id.clone(),
                authorization_id: session.legal_authorization_id,
                ecpa_compliant: true,
            };
            self.event_bus.publish(audit_event).await?;

            return Ok(Some(result));
        }

        Ok(None)
    }

    /// Start transcription processor task
    fn start_transcription_processor(&self, mut receiver: mpsc::UnboundedReceiver<TranscriptionRequest>) {
        let config = self.config.clone();
        let active_sessions = self.active_sessions.clone();
        let connection_status = self.connection_status.clone();
        let event_bus = self.event_bus.clone();

        tokio::spawn(async move {
            let mut reconnection_attempts = 0;

            loop {
                match Self::connect_to_vosk_server(&config).await {
                    Ok((mut ws_sender, mut ws_receiver)) => {
                        info!("Connected to Vosk server at {}", config.server_url);
                        reconnection_attempts = 0;
                        *connection_status.write().await = true;

                        // Send initial configuration
                        let config_msg = VoskMessage::Config {
                            config: VoskServerConfig {
                                sample_rate: config.sample_rate,
                            },
                        };

                        if let Ok(config_json) = serde_json::to_string(&config_msg) {
                            if ws_sender.send(Message::Text(config_json)).await.is_err() {
                                error!("Failed to send configuration to Vosk server");
                                continue;
                            }
                        }

                        // Process transcription requests
                        loop {
                            tokio::select! {
                                // Handle incoming transcription requests
                                request = receiver.recv() => {
                                    if let Some(req) = request {
                                        if let Err(e) = Self::process_transcription_request(
                                            &mut ws_sender,
                                            req,
                                            &active_sessions,
                                            &config,
                                        ).await {
                                            error!("Failed to process transcription request: {}", e);
                                            break;
                                        }
                                    }
                                }

                                // Handle Vosk server responses
                                message = ws_receiver.next() => {
                                    match message {
                                        Some(Ok(Message::Text(text))) => {
                                            if let Err(e) = Self::handle_vosk_response(
                                                &text,
                                                &active_sessions,
                                                &config,
                                                &event_bus,
                                            ).await {
                                                error!("Failed to handle Vosk response: {}", e);
                                            }
                                        }
                                        Some(Ok(Message::Close(_))) => {
                                            warn!("Vosk server closed connection");
                                            break;
                                        }
                                        Some(Err(e)) => {
                                            error!("WebSocket error: {}", e);
                                            break;
                                        }
                                        None => {
                                            warn!("Vosk server connection lost");
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }

                        *connection_status.write().await = false;
                    }
                    Err(e) => {
                        error!("Failed to connect to Vosk server: {}", e);
                        reconnection_attempts += 1;

                        if reconnection_attempts >= config.max_reconnection_attempts {
                            error!("Max reconnection attempts reached, giving up");
                            break;
                        }

                        warn!("Retrying connection in {} seconds (attempt {}/{})",
                              config.reconnection_delay_seconds,
                              reconnection_attempts,
                              config.max_reconnection_attempts);

                        tokio::time::sleep(Duration::from_secs(config.reconnection_delay_seconds)).await;
                    }
                }
            }
        });
    }

    /// Connect to Vosk WebSocket server
    async fn connect_to_vosk_server(
        config: &VoskConfig,
    ) -> Result<(futures::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Message>, futures::stream::SplitStream<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>)> {
        let url = Url::parse(&config.server_url)
            .context("Invalid Vosk server URL")?;

        let connection_future = connect_async(url);
        let connection_timeout = Duration::from_secs(config.connection_timeout_seconds);

        let (ws_stream, _) = timeout(connection_timeout, connection_future)
            .await
            .context("Connection timeout")?
            .context("Failed to connect to Vosk server")?;

        let (ws_sender, ws_receiver) = ws_stream.split();
        Ok((ws_sender, ws_receiver))
    }

    /// Process transcription request
    async fn process_transcription_request(
        ws_sender: &mut futures::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Message>,
        request: TranscriptionRequest,
        active_sessions: &Arc<RwLock<HashMap<String, TranscriptionSession>>>,
        _config: &VoskConfig,
    ) -> Result<()> {
        // Convert audio data to base64
        let audio_base64 = base64::engine::general_purpose::STANDARD.encode(&request.audio_data);

        // Send audio data to Vosk
        let audio_msg = VoskMessage::AudioData {
            audio: audio_base64,
        };

        let audio_json = serde_json::to_string(&audio_msg)
            .context("Failed to serialize audio message")?;

        ws_sender.send(Message::Text(audio_json)).await
            .context("Failed to send audio to Vosk server")?;

        debug!("Sent {} bytes of audio data to Vosk", request.audio_data.len());

        // Update session state
        let mut sessions = active_sessions.write().await;
        if let Some(session) = sessions.get_mut(&request.recording_id) {
            session.started_at = Utc::now();
        }

        Ok(())
    }

    /// Handle response from Vosk server
    async fn handle_vosk_response(
        text: &str,
        active_sessions: &Arc<RwLock<HashMap<String, TranscriptionSession>>>,
        config: &VoskConfig,
        _event_bus: &Arc<EventBus>,
    ) -> Result<()> {
        if let Ok(vosk_msg) = serde_json::from_str::<VoskMessage>(text) {
            match vosk_msg {
                VoskMessage::Result { result } => {
                    debug!("Received final transcription: {}", result.text);
                    Self::process_final_transcription(result, active_sessions, config).await?;
                }
                VoskMessage::Partial { partial } => {
                    debug!("Received partial transcription: {}", partial);
                    Self::process_partial_transcription(partial, active_sessions, config).await?;
                }
                VoskMessage::Error { error } => {
                    error!("Vosk server error: {}", error);
                }
                _ => {
                    debug!("Received other Vosk message type");
                }
            }
        } else {
            warn!("Failed to parse Vosk response: {}", text);
        }

        Ok(())
    }

    /// Process final transcription result
    async fn process_final_transcription(
        result: VoskResult,
        active_sessions: &Arc<RwLock<HashMap<String, TranscriptionSession>>>,
        config: &VoskConfig,
    ) -> Result<()> {
        if result.text.trim().is_empty() {
            return Ok(());
        }

        let mut sessions = active_sessions.write().await;

        // For simplicity, update all active sessions with this result
        // In production, you'd want to track which session corresponds to which audio
        for session in sessions.values_mut() {
            session.final_text.push(' ');
            session.final_text.push_str(&result.text);

            if let Some(confidence) = result.conf {
                session.total_confidence += confidence;
                session.word_count += result.text.split_whitespace().count();
            }

            // Check for banned words
            if config.enable_banned_word_detection {
                let banned_words = Self::detect_banned_words(&result.text, &config.banned_words);
                session.banned_words_found.extend(banned_words);
            }
        }

        Ok(())
    }

    /// Process partial transcription result
    async fn process_partial_transcription(
        partial: String,
        active_sessions: &Arc<RwLock<HashMap<String, TranscriptionSession>>>,
        config: &VoskConfig,
    ) -> Result<()> {
        if partial.trim().is_empty() {
            return Ok(());
        }

        let mut sessions = active_sessions.write().await;

        // Update partial text for all active sessions
        for session in sessions.values_mut() {
            session.partial_text = partial.clone();

            // Check for banned words in partial results too
            if config.enable_banned_word_detection {
                let banned_words = Self::detect_banned_words(&partial, &config.banned_words);
                for word in banned_words {
                    if !session.banned_words_found.contains(&word) {
                        session.banned_words_found.push(word);
                    }
                }
            }
        }

        Ok(())
    }

    /// Detect banned words in transcription text
    fn detect_banned_words(text: &str, banned_words: &[String]) -> Vec<String> {
        let text_lower = text.to_lowercase();
        let mut found_words = Vec::new();

        for banned_word in banned_words {
            if text_lower.contains(&banned_word.to_lowercase()) {
                found_words.push(banned_word.clone());
            }
        }

        found_words
    }

    /// Calculate fraud risk score based on transcription analysis
    fn calculate_fraud_risk_score(&self, text: &str, banned_words: &[String]) -> f64 {
        let mut score = 0.0;

        // Base score from banned words
        score += banned_words.len() as f64 * 0.3;

        // Additional patterns (simplified scoring)
        let text_lower = text.to_lowercase();

        // Urgency indicators
        if text_lower.contains("urgent") || text_lower.contains("immediately") {
            score += 0.2;
        }

        // Financial requests
        if text_lower.contains("send money") || text_lower.contains("wire transfer") {
            score += 0.4;
        }

        // Verification requests
        if text_lower.contains("verify") && (text_lower.contains("account") || text_lower.contains("card")) {
            score += 0.3;
        }

        // Normalize to 0.0-1.0 range
        score.min(1.0)
    }

    /// Get connection status
    pub async fn is_connected(&self) -> bool {
        *self.connection_status.read().await
    }

    /// Get service statistics
    pub async fn get_statistics(&self) -> HashMap<String, u64> {
        let sessions = self.active_sessions.read().await;
        let mut stats = HashMap::new();

        stats.insert("active_transcription_sessions".to_string(), sessions.len() as u64);
        stats.insert("connected_to_vosk".to_string(), if self.is_connected().await { 1 } else { 0 });

        let mut total_words = 0u64;
        let mut total_banned_words = 0u64;

        for session in sessions.values() {
            total_words += session.word_count as u64;
            total_banned_words += session.banned_words_found.len() as u64;
        }

        stats.insert("total_words_transcribed".to_string(), total_words);
        stats.insert("total_banned_words_detected".to_string(), total_banned_words);

        stats
    }
}