//! Additional event types for the telecommunications system
//!
//! This module contains supplementary event types and utilities
//! that extend the core event system functionality.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Extended event metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    /// Event correlation ID
    pub correlation_id: String,
    /// Event source service
    pub source_service: String,
    /// Event priority level
    pub priority: EventPriority,
    /// Additional tags
    pub tags: HashMap<String, String>,
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
}

/// Event priority levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventPriority {
    Critical,
    High,
    Normal,
    Low,
}

impl Default for EventPriority {
    fn default() -> Self {
        EventPriority::Normal
    }
}

/// Event processing status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Retrying,
}

/// Event batch for bulk processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBatch {
    /// Batch identifier
    pub batch_id: String,
    /// Events in this batch
    pub events: Vec<super::TelecomEvent>,
    /// Batch metadata
    pub metadata: EventMetadata,
}

/// Event subscription configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSubscription {
    /// Subscription identifier
    pub subscription_id: String,
    /// Event filter criteria
    pub filter: super::EventFilter,
    /// Subscription callback URL (for webhook subscriptions)
    pub callback_url: Option<String>,
    /// Maximum retry attempts for failed deliveries
    pub max_retries: u32,
    /// Subscription expiry time
    pub expires_at: Option<DateTime<Utc>>,
}

impl Default for EventSubscription {
    fn default() -> Self {
        Self {
            subscription_id: uuid::Uuid::new_v4().to_string(),
            filter: super::EventFilter::default(),
            callback_url: None,
            max_retries: 3,
            expires_at: None,
        }
    }
}
