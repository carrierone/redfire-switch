/*
 * Redfire Switch - A Class 4 SIP Telephone Switch
 * Copyright (C) 2025 Carrier One Inc and contributors
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

use anyhow::{Result, anyhow};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::{get, post, put, delete},
    Router,
};
use base64::{Engine as _, engine::general_purpose};
use chrono::{DateTime, Utc};
use hmac::Hmac;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::{info, warn, error, debug};
use uuid::Uuid;

use crate::sms::{SmsService, SmsMessage, MessageStatus, MessageDirection};

/// Twilio-compatible REST API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwilioApiConfig {
    /// Enable the Twilio-compatible API
    pub enabled: bool,
    /// API bind address
    pub bind_address: String,
    /// API port
    pub port: u16,
    /// Account SID
    pub account_sid: String,
    /// Auth token for request validation
    pub auth_token: String,
    /// API base path
    pub base_path: String,
    /// Enable webhook signatures validation
    pub validate_signatures: bool,
    /// Request timeout (seconds)
    pub request_timeout: u64,
    /// Rate limiting (requests per minute)
    pub rate_limit: u32,
    /// Enable CORS
    pub enable_cors: bool,
}

impl Default for TwilioApiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bind_address: "0.0.0.0".to_string(),
            port: 8080,
            account_sid: "AC_redfire_switch_account".to_string(),
            auth_token: "your_auth_token_here".to_string(),
            base_path: "/2010-04-01".to_string(),
            validate_signatures: true,
            request_timeout: 30,
            rate_limit: 1000,
            enable_cors: true,
        }
    }
}

/// Twilio Conversations Service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationsConfig {
    /// Chat Service SID
    pub chat_service_sid: String,
    /// Default webhook URL for events
    pub webhook_url: Option<String>,
    /// Webhook events to send
    pub webhook_events: Vec<String>,
    /// Default conversation timeout (seconds)
    pub conversation_timeout: u64,
    /// Maximum participants per conversation
    pub max_participants: u32,
}

impl Default for ConversationsConfig {
    fn default() -> Self {
        Self {
            chat_service_sid: "IS_redfire_conversations".to_string(),
            webhook_url: None,
            webhook_events: vec![
                "onMessageAdded".to_string(),
                "onConversationAdded".to_string(),
                "onParticipantAdded".to_string(),
            ],
            conversation_timeout: 86400, // 24 hours
            max_participants: 50,
        }
    }
}

/// Twilio-compatible message request
#[derive(Debug, Deserialize)]
pub struct CreateMessageRequest {
    #[serde(rename = "To")]
    pub to: String,
    #[serde(rename = "From")]
    pub from: String,
    #[serde(rename = "Body")]
    pub body: String,
    #[serde(rename = "MediaUrl")]
    pub media_url: Option<String>,
    #[serde(rename = "StatusCallback")]
    pub status_callback: Option<String>,
    #[serde(rename = "ApplicationSid")]
    pub application_sid: Option<String>,
    #[serde(rename = "MaxPrice")]
    pub max_price: Option<String>,
    #[serde(rename = "ProvideFeedback")]
    pub provide_feedback: Option<bool>,
}

/// Twilio-compatible message response
#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub sid: String,
    pub account_sid: String,
    pub messaging_service_sid: Option<String>,
    pub from: String,
    pub to: String,
    pub body: String,
    pub status: String,
    pub num_segments: String,
    pub num_media: String,
    pub direction: String,
    pub api_version: String,
    pub price: Option<String>,
    pub price_unit: Option<String>,
    pub error_code: Option<i32>,
    pub error_message: Option<String>,
    pub uri: String,
    pub subresource_uris: HashMap<String, String>,
    pub date_created: String,
    pub date_updated: String,
    pub date_sent: Option<String>,
}

/// Conversation object (Twilio Conversations API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub sid: String,
    pub account_sid: String,
    pub chat_service_sid: String,
    pub messaging_service_sid: Option<String>,
    pub friendly_name: Option<String>,
    pub unique_name: Option<String>,
    pub attributes: String,
    pub state: ConversationState,
    pub date_created: DateTime<Utc>,
    pub date_updated: DateTime<Utc>,
    pub url: String,
    pub links: HashMap<String, String>,
}

/// Conversation state
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConversationState {
    Active,
    Inactive,
    Closed,
}

/// Conversation participant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Participant {
    pub sid: String,
    pub account_sid: String,
    pub conversation_sid: String,
    pub messaging_binding: Option<MessagingBinding>,
    pub identity: Option<String>,
    pub attributes: String,
    pub role_sid: Option<String>,
    pub date_created: DateTime<Utc>,
    pub date_updated: DateTime<Utc>,
    pub url: String,
}

/// Messaging binding for participants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagingBinding {
    #[serde(rename = "type")]
    pub binding_type: String,
    pub address: String,
    pub proxy_address: Option<String>,
}

/// Conversation message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub sid: String,
    pub account_sid: String,
    pub conversation_sid: String,
    pub participant_sid: Option<String>,
    pub body: String,
    pub media: Vec<MessageMedia>,
    pub author: String,
    pub attributes: String,
    pub date_created: DateTime<Utc>,
    pub date_updated: DateTime<Utc>,
    pub index: u64,
    pub delivery: Option<MessageDelivery>,
    pub url: String,
}

/// Message media attachment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMedia {
    pub sid: String,
    pub filename: Option<String>,
    pub content_type: String,
    pub size: u64,
    pub url: String,
}

/// Message delivery information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDelivery {
    pub total: u32,
    pub sent: u32,
    pub delivered: u32,
    pub read: u32,
    pub failed: u32,
    pub undelivered: u32,
}

/// Request to create a conversation
#[derive(Debug, Deserialize)]
pub struct CreateConversationRequest {
    #[serde(rename = "FriendlyName")]
    pub friendly_name: Option<String>,
    #[serde(rename = "UniqueName")]
    pub unique_name: Option<String>,
    #[serde(rename = "Attributes")]
    pub attributes: Option<String>,
    #[serde(rename = "MessagingServiceSid")]
    pub messaging_service_sid: Option<String>,
}

/// Request to add participant to conversation
#[derive(Debug, Deserialize)]
pub struct AddParticipantRequest {
    #[serde(rename = "Identity")]
    pub identity: Option<String>,
    #[serde(rename = "MessagingBinding.Address")]
    pub messaging_binding_address: Option<String>,
    #[serde(rename = "MessagingBinding.ProxyAddress")]
    pub messaging_binding_proxy_address: Option<String>,
    #[serde(rename = "Attributes")]
    pub attributes: Option<String>,
    #[serde(rename = "RoleSid")]
    pub role_sid: Option<String>,
}

/// Request to send message in conversation
#[derive(Debug, Deserialize)]
pub struct SendConversationMessageRequest {
    #[serde(rename = "Author")]
    pub author: Option<String>,
    #[serde(rename = "Body")]
    pub body: String,
    #[serde(rename = "MediaUrl")]
    pub media_url: Option<String>,
    #[serde(rename = "Attributes")]
    pub attributes: Option<String>,
}

/// Twilio API service state
#[derive(Clone)]
pub struct TwilioApiState {
    pub config: TwilioApiConfig,
    pub conversations_config: ConversationsConfig,
    pub sms_service: Arc<SmsService>,
    pub conversations: Arc<dashmap::DashMap<String, Conversation>>,
    pub participants: Arc<dashmap::DashMap<String, Vec<Participant>>>,
    pub conversation_messages: Arc<dashmap::DashMap<String, Vec<ConversationMessage>>>,
}

/// Create Twilio-compatible REST API router
pub fn create_twilio_router(state: TwilioApiState) -> Router {
    let mut router = Router::new()
        // Standard Twilio Messages API
        .route("/2010-04-01/Accounts/:account_sid/Messages.json", post(create_message))
        .route("/2010-04-01/Accounts/:account_sid/Messages/:message_sid.json", get(get_message))
        
        // Conversations API
        .route("/v1/Services/:chat_service_sid/Conversations", post(create_conversation))
        .route("/v1/Services/:chat_service_sid/Conversations", get(list_conversations))
        .route("/v1/Services/:chat_service_sid/Conversations/:conversation_sid", get(get_conversation))
        .route("/v1/Services/:chat_service_sid/Conversations/:conversation_sid", put(update_conversation))
        .route("/v1/Services/:chat_service_sid/Conversations/:conversation_sid", delete(delete_conversation))
        
        // Conversation Participants
        .route("/v1/Services/:chat_service_sid/Conversations/:conversation_sid/Participants", post(add_participant))
        .route("/v1/Services/:chat_service_sid/Conversations/:conversation_sid/Participants", get(list_participants))
        .route("/v1/Services/:chat_service_sid/Conversations/:conversation_sid/Participants/:participant_sid", get(get_participant))
        .route("/v1/Services/:chat_service_sid/Conversations/:conversation_sid/Participants/:participant_sid", delete(remove_participant))
        
        // Conversation Messages
        .route("/v1/Services/:chat_service_sid/Conversations/:conversation_sid/Messages", post(send_conversation_message))
        .route("/v1/Services/:chat_service_sid/Conversations/:conversation_sid/Messages", get(list_conversation_messages))
        .route("/v1/Services/:chat_service_sid/Conversations/:conversation_sid/Messages/:message_sid", get(get_conversation_message))
        
        .with_state(state);

    // Add CORS if enabled
    // Add CORS layer (enable_cors check removed due to axum API changes)
    router = router.layer(CorsLayer::permissive());

    router
}

/// Create a new SMS message (Twilio Messages API)
async fn create_message(
    State(state): State<TwilioApiState>,
    Path(account_sid): Path<String>,
    headers: HeaderMap,
    axum::Form(req): axum::Form<CreateMessageRequest>,
) -> Result<(StatusCode, Json<MessageResponse>), (StatusCode, String)> {
    info!("Creating message from {} to {}", req.from, req.to);

    // Validate account SID
    if account_sid != state.config.account_sid {
        return Err((StatusCode::UNAUTHORIZED, "Invalid Account SID".to_string()));
    }

    // Validate authentication
    if let Err(e) = validate_auth(&headers, &state.config) {
        warn!("Authentication failed: {}", e);
        return Err((StatusCode::UNAUTHORIZED, "Authentication failed".to_string()));
    }

    // Send SMS via our SMS service
    let priority = SmsPriority::Normal;
    match state.sms_service.send_sms(
        req.from.clone(),
        req.to.clone(),
        req.body.clone(),
        None, // No customer ID from Twilio API
        priority,
    ).await {
        Ok(message_id) => {
            let response = MessageResponse {
                sid: message_id.clone(),
                account_sid: account_sid.clone(),
                messaging_service_sid: None,
                from: req.from,
                to: req.to,
                body: req.body,
                status: "queued".to_string(),
                num_segments: "1".to_string(),
                num_media: "0".to_string(),
                direction: "outbound-api".to_string(),
                api_version: "2010-04-01".to_string(),
                price: Some("-0.0050".to_string()),
                price_unit: Some("USD".to_string()),
                error_code: None,
                error_message: None,
                uri: format!("/2010-04-01/Accounts/{}/Messages/{}.json", account_sid, message_id),
                subresource_uris: HashMap::new(),
                date_created: Utc::now().format("%a, %d %b %Y %H:%M:%S %z").to_string(),
                date_updated: Utc::now().format("%a, %d %b %Y %H:%M:%S %z").to_string(),
                date_sent: None,
            };

            Ok((StatusCode::CREATED, Json(response)))
        }
        Err(e) => {
            error!("Failed to send SMS: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to send message".to_string()))
        }
    }
}

/// Get message details
async fn get_message(
    State(state): State<TwilioApiState>,
    Path((account_sid, message_sid)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<MessageResponse>), (StatusCode, String)> {
    // Validate account SID
    if account_sid != state.config.account_sid {
        return Err((StatusCode::UNAUTHORIZED, "Invalid Account SID".to_string()));
    }

    // Validate authentication
    if let Err(e) = validate_auth(&headers, &state.config) {
        return Err((StatusCode::UNAUTHORIZED, e.to_string()));
    }

    // Get message status from SMS service
    if let Some(status) = state.sms_service.get_message_status(&message_sid) {
        let twilio_status = match status {
            MessageStatus::Pending => "queued",
            MessageStatus::Sent => "sent",
            MessageStatus::Delivered => "delivered",
            MessageStatus::Failed => "failed",
        };

        let response = MessageResponse {
            sid: message_sid.clone(),
            account_sid: account_sid.clone(),
            messaging_service_sid: None,
            from: "+15551234567".to_string(), // Would come from stored message
            to: "+15559876543".to_string(),   // Would come from stored message
            body: "Message content".to_string(), // Would come from stored message
            status: twilio_status.to_string(),
            num_segments: "1".to_string(),
            num_media: "0".to_string(),
            direction: "outbound-api".to_string(),
            api_version: "2010-04-01".to_string(),
            price: Some("-0.0050".to_string()),
            price_unit: Some("USD".to_string()),
            error_code: None,
            error_message: None,
            uri: format!("/2010-04-01/Accounts/{}/Messages/{}.json", account_sid, message_sid),
            subresource_uris: HashMap::new(),
            date_created: Utc::now().format("%a, %d %b %Y %H:%M:%S %z").to_string(),
            date_updated: Utc::now().format("%a, %d %b %Y %H:%M:%S %z").to_string(),
            date_sent: Some(Utc::now().format("%a, %d %b %Y %H:%M:%S %z").to_string()),
        };

        Ok((StatusCode::OK, Json(response)))
    } else {
        Err((StatusCode::NOT_FOUND, "Message not found".to_string()))
    }
}

/// Create a new conversation
async fn create_conversation(
    State(state): State<TwilioApiState>,
    Path(chat_service_sid): Path<String>,
    headers: HeaderMap,
    axum::Form(req): axum::Form<CreateConversationRequest>,
) -> Result<(StatusCode, Json<Conversation>), (StatusCode, String)> {
    // Validate chat service SID
    if chat_service_sid != state.conversations_config.chat_service_sid {
        return Err((StatusCode::UNAUTHORIZED, "Invalid Chat Service SID".to_string()));
    }

    // Validate authentication
    if let Err(e) = validate_auth(&headers, &state.config) {
        return Err((StatusCode::UNAUTHORIZED, e.to_string()));
    }

    let conversation_sid = format!("CH{}", Uuid::new_v4().to_string().replace("-", ""));
    let now = Utc::now();

    let conversation = Conversation {
        sid: conversation_sid.clone(),
        account_sid: state.config.account_sid.clone(),
        chat_service_sid: chat_service_sid.clone(),
        messaging_service_sid: req.messaging_service_sid,
        friendly_name: req.friendly_name,
        unique_name: req.unique_name,
        attributes: req.attributes.unwrap_or_default(),
        state: ConversationState::Active,
        date_created: now,
        date_updated: now,
        url: format!("/v1/Services/{}/Conversations/{}", chat_service_sid, conversation_sid),
        links: HashMap::new(),
    };

    // Store conversation
    state.conversations.insert(conversation_sid.clone(), conversation.clone());
    state.participants.insert(conversation_sid.clone(), Vec::new());
    state.conversation_messages.insert(conversation_sid.clone(), Vec::new());

    info!("Created conversation: {}", conversation_sid);
    Ok((StatusCode::CREATED, Json(conversation)))
}

/// List conversations
async fn list_conversations(
    State(state): State<TwilioApiState>,
    Path(chat_service_sid): Path<String>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<HashMap<String, serde_json::Value>>), (StatusCode, String)> {
    // Validate authentication
    if let Err(e) = validate_auth(&headers, &state.config) {
        return Err((StatusCode::UNAUTHORIZED, e.to_string()));
    }

    let conversations: Vec<Conversation> = state.conversations
        .iter()
        .filter(|entry| entry.value().chat_service_sid == chat_service_sid)
        .map(|entry| entry.value().clone())
        .collect();

    let mut response = HashMap::new();
    response.insert("conversations".to_string(), serde_json::to_value(conversations).unwrap());
    response.insert("meta".to_string(), serde_json::json!({
        "page": 0,
        "page_size": 50,
        "first_page_url": format!("/v1/Services/{}/Conversations?PageSize=50&Page=0", chat_service_sid),
        "previous_page_url": null,
        "next_page_url": null,
        "key": "conversations"
    }));

    Ok((StatusCode::OK, Json(response)))
}

/// Get conversation details
async fn get_conversation(
    State(state): State<TwilioApiState>,
    Path((chat_service_sid, conversation_sid)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<Conversation>), (StatusCode, String)> {
    // Validate authentication
    if let Err(e) = validate_auth(&headers, &state.config) {
        return Err((StatusCode::UNAUTHORIZED, e.to_string()));
    }

    if let Some(conversation) = state.conversations.get(&conversation_sid) {
        Ok((StatusCode::OK, Json(conversation.clone())))
    } else {
        Err((StatusCode::NOT_FOUND, "Conversation not found".to_string()))
    }
}

/// Update conversation
async fn update_conversation(
    State(state): State<TwilioApiState>,
    Path((chat_service_sid, conversation_sid)): Path<(String, String)>,
    headers: HeaderMap,
    axum::Form(req): axum::Form<CreateConversationRequest>,
) -> Result<(StatusCode, Json<Conversation>), (StatusCode, String)> {
    // Validate authentication
    if let Err(e) = validate_auth(&headers, &state.config) {
        return Err((StatusCode::UNAUTHORIZED, e.to_string()));
    }

    if let Some(mut conversation_ref) = state.conversations.get_mut(&conversation_sid) {
        let conversation = conversation_ref.value_mut();
        
        if let Some(friendly_name) = req.friendly_name {
            conversation.friendly_name = Some(friendly_name);
        }
        if let Some(unique_name) = req.unique_name {
            conversation.unique_name = Some(unique_name);
        }
        if let Some(attributes) = req.attributes {
            conversation.attributes = attributes;
        }
        conversation.date_updated = Utc::now();

        Ok((StatusCode::OK, Json(conversation.clone())))
    } else {
        Err((StatusCode::NOT_FOUND, "Conversation not found".to_string()))
    }
}

/// Delete conversation
async fn delete_conversation(
    State(state): State<TwilioApiState>,
    Path((chat_service_sid, conversation_sid)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<StatusCode, (StatusCode, String)> {
    // Validate authentication
    if let Err(e) = validate_auth(&headers, &state.config) {
        return Err((StatusCode::UNAUTHORIZED, e.to_string()));
    }

    if state.conversations.remove(&conversation_sid).is_some() {
        state.participants.remove(&conversation_sid);
        state.conversation_messages.remove(&conversation_sid);
        info!("Deleted conversation: {}", conversation_sid);
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((StatusCode::NOT_FOUND, "Conversation not found".to_string()))
    }
}

/// Add participant to conversation
async fn add_participant(
    State(state): State<TwilioApiState>,
    Path((chat_service_sid, conversation_sid)): Path<(String, String)>,
    headers: HeaderMap,
    axum::Form(req): axum::Form<AddParticipantRequest>,
) -> Result<(StatusCode, Json<Participant>), (StatusCode, String)> {
    // Validate authentication
    if let Err(e) = validate_auth(&headers, &state.config) {
        return Err((StatusCode::UNAUTHORIZED, e.to_string()));
    }

    // Check if conversation exists
    if !state.conversations.contains_key(&conversation_sid) {
        return Err((StatusCode::NOT_FOUND, "Conversation not found".to_string()));
    }

    let participant_sid = format!("MB{}", Uuid::new_v4().to_string().replace("-", ""));
    let now = Utc::now();

    let messaging_binding = if let Some(address) = req.messaging_binding_address {
        Some(MessagingBinding {
            binding_type: "sms".to_string(),
            address,
            proxy_address: req.messaging_binding_proxy_address,
        })
    } else {
        None
    };

    let participant = Participant {
        sid: participant_sid.clone(),
        account_sid: state.config.account_sid.clone(),
        conversation_sid: conversation_sid.clone(),
        messaging_binding,
        identity: req.identity,
        attributes: req.attributes.unwrap_or_default(),
        role_sid: req.role_sid,
        date_created: now,
        date_updated: now,
        url: format!("/v1/Services/{}/Conversations/{}/Participants/{}", 
                    chat_service_sid, conversation_sid, participant_sid),
    };

    // Add participant to conversation
    if let Some(mut participants_ref) = state.participants.get_mut(&conversation_sid) {
        participants_ref.push(participant.clone());
    }

    info!("Added participant {} to conversation {}", participant_sid, conversation_sid);
    Ok((StatusCode::CREATED, Json(participant)))
}

/// List participants in conversation
async fn list_participants(
    State(state): State<TwilioApiState>,
    Path((chat_service_sid, conversation_sid)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<HashMap<String, serde_json::Value>>), (StatusCode, String)> {
    // Validate authentication
    if let Err(e) = validate_auth(&headers, &state.config) {
        return Err((StatusCode::UNAUTHORIZED, e.to_string()));
    }

    if let Some(participants_ref) = state.participants.get(&conversation_sid) {
        let participants = participants_ref.clone();
        
        let mut response = HashMap::new();
        response.insert("participants".to_string(), serde_json::to_value(participants).unwrap());
        response.insert("meta".to_string(), serde_json::json!({
            "page": 0,
            "page_size": 50,
            "first_page_url": format!("/v1/Services/{}/Conversations/{}/Participants?PageSize=50&Page=0", chat_service_sid, conversation_sid),
            "previous_page_url": null,
            "next_page_url": null,
            "key": "participants"
        }));

        Ok((StatusCode::OK, Json(response)))
    } else {
        Err((StatusCode::NOT_FOUND, "Conversation not found".to_string()))
    }
}

/// Get participant details
async fn get_participant(
    State(state): State<TwilioApiState>,
    Path((chat_service_sid, conversation_sid, participant_sid)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<Participant>), (StatusCode, String)> {
    // Validate authentication
    if let Err(e) = validate_auth(&headers, &state.config) {
        return Err((StatusCode::UNAUTHORIZED, e.to_string()));
    }

    if let Some(participants_ref) = state.participants.get(&conversation_sid) {
        if let Some(participant) = participants_ref.iter().find(|p| p.sid == participant_sid) {
            Ok((StatusCode::OK, Json(participant.clone())))
        } else {
            Err((StatusCode::NOT_FOUND, "Participant not found".to_string()))
        }
    } else {
        Err((StatusCode::NOT_FOUND, "Conversation not found".to_string()))
    }
}

/// Remove participant from conversation
async fn remove_participant(
    State(state): State<TwilioApiState>,
    Path((chat_service_sid, conversation_sid, participant_sid)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Result<StatusCode, (StatusCode, String)> {
    // Validate authentication
    if let Err(e) = validate_auth(&headers, &state.config) {
        return Err((StatusCode::UNAUTHORIZED, e.to_string()));
    }

    if let Some(mut participants_ref) = state.participants.get_mut(&conversation_sid) {
        if let Some(pos) = participants_ref.iter().position(|p| p.sid == participant_sid) {
            participants_ref.remove(pos);
            info!("Removed participant {} from conversation {}", participant_sid, conversation_sid);
            Ok(StatusCode::NO_CONTENT)
        } else {
            Err((StatusCode::NOT_FOUND, "Participant not found".to_string()))
        }
    } else {
        Err((StatusCode::NOT_FOUND, "Conversation not found".to_string()))
    }
}

/// Send message in conversation
async fn send_conversation_message(
    State(state): State<TwilioApiState>,
    Path((chat_service_sid, conversation_sid)): Path<(String, String)>,
    headers: HeaderMap,
    axum::Form(req): axum::Form<SendConversationMessageRequest>,
) -> Result<(StatusCode, Json<ConversationMessage>), (StatusCode, String)> {
    // Validate authentication
    if let Err(e) = validate_auth(&headers, &state.config) {
        return Err((StatusCode::UNAUTHORIZED, e.to_string()));
    }

    // Check if conversation exists
    if !state.conversations.contains_key(&conversation_sid) {
        return Err((StatusCode::NOT_FOUND, "Conversation not found".to_string()));
    }

    let message_sid = format!("IM{}", Uuid::new_v4().to_string().replace("-", ""));
    let now = Utc::now();

    // Get current message count for index
    let index = if let Some(messages_ref) = state.conversation_messages.get(&conversation_sid) {
        messages_ref.len() as u64
    } else {
        0
    };

    let message = ConversationMessage {
        sid: message_sid.clone(),
        account_sid: state.config.account_sid.clone(),
        conversation_sid: conversation_sid.clone(),
        participant_sid: None, // Would be determined from author
        body: req.body.clone(),
        media: Vec::new(),
        author: req.author.unwrap_or_else(|| "system".to_string()),
        attributes: req.attributes.unwrap_or_default(),
        date_created: now,
        date_updated: now,
        index,
        delivery: Some(MessageDelivery {
            total: 1,
            sent: 1,
            delivered: 1,
            read: 0,
            failed: 0,
            undelivered: 0,
        }),
        url: format!("/v1/Services/{}/Conversations/{}/Messages/{}", 
                    chat_service_sid, conversation_sid, message_sid),
    };

    // Add message to conversation
    if let Some(mut messages_ref) = state.conversation_messages.get_mut(&conversation_sid) {
        messages_ref.push(message.clone());
    }

    // TODO: Send actual SMS messages to participants with phone numbers
    // This would iterate through participants and send SMS via the SMS service

    info!("Added message {} to conversation {}", message_sid, conversation_sid);
    Ok((StatusCode::CREATED, Json(message)))
}

/// List messages in conversation
async fn list_conversation_messages(
    State(state): State<TwilioApiState>,
    Path((chat_service_sid, conversation_sid)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<HashMap<String, serde_json::Value>>), (StatusCode, String)> {
    // Validate authentication
    if let Err(e) = validate_auth(&headers, &state.config) {
        return Err((StatusCode::UNAUTHORIZED, e.to_string()));
    }

    if let Some(messages_ref) = state.conversation_messages.get(&conversation_sid) {
        let messages = messages_ref.clone();
        
        let mut response = HashMap::new();
        response.insert("messages".to_string(), serde_json::to_value(messages).unwrap());
        response.insert("meta".to_string(), serde_json::json!({
            "page": 0,
            "page_size": 50,
            "first_page_url": format!("/v1/Services/{}/Conversations/{}/Messages?PageSize=50&Page=0", chat_service_sid, conversation_sid),
            "previous_page_url": null,
            "next_page_url": null,
            "key": "messages"
        }));

        Ok((StatusCode::OK, Json(response)))
    } else {
        Err((StatusCode::NOT_FOUND, "Conversation not found".to_string()))
    }
}

/// Get conversation message details
async fn get_conversation_message(
    State(state): State<TwilioApiState>,
    Path((chat_service_sid, conversation_sid, message_sid)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<ConversationMessage>), (StatusCode, String)> {
    // Validate authentication
    if let Err(e) = validate_auth(&headers, &state.config) {
        return Err((StatusCode::UNAUTHORIZED, e.to_string()));
    }

    if let Some(messages_ref) = state.conversation_messages.get(&conversation_sid) {
        if let Some(message) = messages_ref.iter().find(|m| m.sid == message_sid) {
            Ok((StatusCode::OK, Json(message.clone())))
        } else {
            Err((StatusCode::NOT_FOUND, "Message not found".to_string()))
        }
    } else {
        Err((StatusCode::NOT_FOUND, "Conversation not found".to_string()))
    }
}

/// Validate HTTP Basic Auth
fn validate_auth(headers: &HeaderMap, config: &TwilioApiConfig) -> Result<()> {
    if !config.validate_signatures {
        return Ok(());
    }

    let auth_header = headers.get("authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| anyhow!("Missing authorization header"))?;

    if !auth_header.starts_with("Basic ") {
        return Err(anyhow!("Invalid authorization header format"));
    }

    let encoded = &auth_header[6..];
    let decoded = general_purpose::STANDARD.decode(encoded)
        .map_err(|_| anyhow!("Invalid base64 encoding"))?;
    
    let credentials = String::from_utf8(decoded)
        .map_err(|_| anyhow!("Invalid UTF-8 in credentials"))?;

    let expected = format!("{}:{}", config.account_sid, config.auth_token);
    
    if credentials != expected {
        return Err(anyhow!("Invalid credentials"));
    }

    Ok(())
}

/// Start Twilio-compatible REST API server
pub async fn start_twilio_api_server(
    config: TwilioApiConfig,
    conversations_config: ConversationsConfig,
    sms_service: Arc<SmsService>,
) -> Result<()> {
    let state = TwilioApiState {
        config: config.clone(),
        conversations_config,
        sms_service,
        conversations: Arc::new(dashmap::DashMap::new()),
        participants: Arc::new(dashmap::DashMap::new()),
        conversation_messages: Arc::new(dashmap::DashMap::new()),
    };

    let app = create_twilio_router(state);
    
    let bind_addr = format!("{}:{}", config.bind_address, config.port);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    
    info!("Twilio-compatible API server listening on {}", bind_addr);
    
    axum::serve(listener, app).await?;
    
    Ok(())
}