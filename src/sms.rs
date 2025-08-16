/*
 * Redfire Switch - Simplified SMS Service (SIP MESSAGE only)
 * Copyright (C) 2025 Carrier One Inc and contributors
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

//! Simplified SMS service supporting only SIP MESSAGE

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};
use chrono::{DateTime, Utc};

/// SMS message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsMessage {
    pub id: String,
    pub from: String,
    pub to: String,
    pub text: String,
    pub timestamp: DateTime<Utc>,
    pub status: MessageStatus,
    pub direction: MessageDirection,
}

/// Message status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageStatus {
    Pending,
    Sent,
    Delivered,
    Failed,
}

/// Message direction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageDirection {
    Inbound,
    Outbound,
}

/// SMS service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsConfig {
    pub enabled: bool,
    pub max_message_length: usize,
    pub store_messages: bool,
    pub message_retention_days: u32,
}

impl Default for SmsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_message_length: 160,
            store_messages: true,
            message_retention_days: 30,
        }
    }
}

/// Simple SMS service using SIP MESSAGE
pub struct SmsService {
    config: SmsConfig,
    messages: Arc<RwLock<HashMap<String, SmsMessage>>>,
}

impl SmsService {
    /// Create new SMS service
    pub fn new(config: SmsConfig) -> Self {
        Self {
            config,
            messages: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Send SMS via SIP MESSAGE
    pub async fn send_sms(&self, from: &str, to: &str, text: &str) -> Result<String> {
        if !self.config.enabled {
            return Err(anyhow!("SMS service is not enabled"));
        }
        
        if text.len() > self.config.max_message_length {
            return Err(anyhow!("Message exceeds maximum length of {} characters", 
                self.config.max_message_length));
        }
        
        let message_id = uuid::Uuid::new_v4().to_string();
        
        let message = SmsMessage {
            id: message_id.clone(),
            from: from.to_string(),
            to: to.to_string(),
            text: text.to_string(),
            timestamp: Utc::now(),
            status: MessageStatus::Pending,
            direction: MessageDirection::Outbound,
        };
        
        // Store message if configured
        if self.config.store_messages {
            let mut messages = self.messages.write().await;
            messages.insert(message_id.clone(), message.clone());
        }
        
        // In a real implementation, this would send via SIP MESSAGE
        info!("Sending SMS from {} to {}: {}", from, to, text);
        
        // Simulate sending
        self.update_message_status(&message_id, MessageStatus::Sent).await?;
        
        Ok(message_id)
    }
    
    /// Receive SMS via SIP MESSAGE
    pub async fn receive_sms(&self, from: &str, to: &str, text: &str) -> Result<String> {
        if !self.config.enabled {
            return Err(anyhow!("SMS service is not enabled"));
        }
        
        let message_id = uuid::Uuid::new_v4().to_string();
        
        let message = SmsMessage {
            id: message_id.clone(),
            from: from.to_string(),
            to: to.to_string(),
            text: text.to_string(),
            timestamp: Utc::now(),
            status: MessageStatus::Delivered,
            direction: MessageDirection::Inbound,
        };
        
        // Store message if configured
        if self.config.store_messages {
            let mut messages = self.messages.write().await;
            messages.insert(message_id.clone(), message);
        }
        
        info!("Received SMS from {} to {}: {}", from, to, text);
        
        Ok(message_id)
    }
    
    /// Update message status
    async fn update_message_status(&self, message_id: &str, status: MessageStatus) -> Result<()> {
        if self.config.store_messages {
            let mut messages = self.messages.write().await;
            if let Some(message) = messages.get_mut(message_id) {
                message.status = status;
                debug!("Updated message {} status to {:?}", message_id, status);
            }
        }
        Ok(())
    }
    
    /// Get message by ID
    pub async fn get_message(&self, message_id: &str) -> Option<SmsMessage> {
        let messages = self.messages.read().await;
        messages.get(message_id).cloned()
    }
    
    /// Get message status
    pub async fn get_message_status(&self, message_id: &str) -> Option<MessageStatus> {
        self.get_message(message_id).await.map(|m| m.status)
    }
    
    /// List all messages
    pub async fn list_messages(&self) -> Vec<SmsMessage> {
        let messages = self.messages.read().await;
        messages.values().cloned().collect()
    }
    
    /// Get statistics
    pub async fn get_stats(&self) -> SmsStats {
        let messages = self.messages.read().await;
        
        let total = messages.len();
        let sent = messages.values().filter(|m| m.status == MessageStatus::Sent).count();
        let delivered = messages.values().filter(|m| m.status == MessageStatus::Delivered).count();
        let failed = messages.values().filter(|m| m.status == MessageStatus::Failed).count();
        let pending = messages.values().filter(|m| m.status == MessageStatus::Pending).count();
        
        SmsStats {
            total_messages: total,
            messages_sent: sent,
            messages_delivered: delivered,
            messages_failed: failed,
            messages_pending: pending,
        }
    }
    
    /// Clear old messages
    pub async fn cleanup_old_messages(&self) -> Result<usize> {
        if !self.config.store_messages {
            return Ok(0);
        }
        
        let cutoff = Utc::now() - chrono::Duration::days(self.config.message_retention_days as i64);
        let mut messages = self.messages.write().await;
        
        let old_count = messages.len();
        messages.retain(|_, msg| msg.timestamp > cutoff);
        let removed = old_count - messages.len();
        
        if removed > 0 {
            info!("Cleaned up {} old SMS messages", removed);
        }
        
        Ok(removed)
    }
}

/// SMS statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsStats {
    pub total_messages: usize,
    pub messages_sent: usize,
    pub messages_delivered: usize,
    pub messages_failed: usize,
    pub messages_pending: usize,
}

/// SMS session (simplified - no SMPP)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsSession {
    pub session_id: String,
    pub endpoint: String,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub messages_sent: u64,
    pub messages_received: u64,
}

// Stub types for compatibility
pub type SmppConfig = SmsConfig;
pub type SmppService = SmsService;
pub type SmppSession = SmsSession;

// Re-export main service
pub use SmsService as Service;