//! Event-driven architecture for telecommunications services
//! 
//! This module provides a comprehensive event system that enables loose coupling
//! between microservices and supports real-time monitoring and analytics.

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use uuid::Uuid;

pub mod bus;
pub mod handlers;
pub mod types;

pub use bus::*;
pub use handlers::*;
pub use types::*;

/// Core telecom event types that flow through the system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TelecomEvent {
    /// Call setup initiated
    CallInitiated(CallInitiatedEvent),
    /// Call routing decision made
    CallRouted(CallRoutedEvent),
    /// Media session established
    CallConnected(CallConnectedEvent),
    /// Call terminated with CDR
    CallTerminated(CallTerminatedEvent),
    /// Route advancement occurred
    RouteAdvanced(RouteAdvancedEvent),
    /// Fraud detection alert
    FraudDetected(FraudDetectedEvent),
    /// System health status change
    HealthStatus(HealthStatusEvent),
    /// Configuration changed
    ConfigChanged(ConfigChangedEvent),
    /// Custom plugin event
    PluginEvent(PluginEvent),
}

/// Call initiation details
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CallInitiatedEvent {
    pub call_id: String,
    pub session_id: String,
    pub calling_number: String,
    pub called_number: String,
    pub source_ip: IpAddr,
    pub timestamp: DateTime<Utc>,
    pub trunk_id: Option<i32>,
    pub customer_id: Option<i32>,
    pub user_agent: Option<String>,
    pub sdp_offered: bool,
}

/// Routing decision details
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CallRoutedEvent {
    pub call_id: String,
    pub session_id: String,
    pub selected_route: Option<RouteInfo>,
    pub attempted_routes: Vec<RouteInfo>,
    pub routing_time_ms: u64,
    pub routing_decision: String,
    pub timestamp: DateTime<Utc>,
}

/// Media connection details
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CallConnectedEvent {
    pub call_id: String,
    pub session_id: String,
    pub media_details: MediaSessionInfo,
    pub codec_negotiated: String,
    pub rtp_proxy_used: bool,
    pub connection_time_ms: u64,
    pub timestamp: DateTime<Utc>,
}

/// Call termination and CDR data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CallTerminatedEvent {
    pub call_id: String,
    pub session_id: String,
    pub cdr: CallDetailRecord,
    pub termination_reason: String,
    pub final_response_code: u16,
    pub call_duration_seconds: u32,
    pub timestamp: DateTime<Utc>,
}

/// Route advancement event
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RouteAdvancedEvent {
    pub call_id: String,
    pub session_id: String,
    pub from_route: RouteInfo,
    pub to_route: Option<RouteInfo>,
    pub failure_reason: String,
    pub response_code: u16,
    pub attempt_number: u32,
    pub timestamp: DateTime<Utc>,
}

/// Fraud detection alert
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FraudDetectedEvent {
    pub alert_id: String,
    pub call_id: Option<String>,
    pub session_id: Option<String>,
    pub fraud_type: String,
    pub risk_score: f64,
    pub source_ip: Option<IpAddr>,
    pub calling_number: Option<String>,
    pub details: HashMap<String, String>,
    pub timestamp: DateTime<Utc>,
}

/// System health status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthStatusEvent {
    pub service_name: String,
    pub instance_id: String,
    pub status: HealthStatus,
    pub metrics: HashMap<String, f64>,
    pub timestamp: DateTime<Utc>,
}

/// Configuration change event
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigChangedEvent {
    pub service_name: String,
    pub config_key: String,
    pub old_value: Option<String>,
    pub new_value: String,
    pub changed_by: String,
    pub timestamp: DateTime<Utc>,
}

/// Plugin-generated event
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginEvent {
    pub plugin_name: String,
    pub event_type: String,
    pub data: HashMap<String, serde_json::Value>,
    pub timestamp: DateTime<Utc>,
}

/// Route information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RouteInfo {
    pub route_id: String,
    pub trunk_id: i32,
    pub trunk_name: String,
    pub gateway_ip: IpAddr,
    pub gateway_port: u16,
    pub priority: i32,
    pub cost: f64,
}

/// Media session information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MediaSessionInfo {
    pub rtp_local_port: u16,
    pub rtp_remote_port: u16,
    pub rtp_remote_ip: IpAddr,
    pub rtcp_enabled: bool,
    pub encryption_enabled: bool,
    pub bandwidth_kbps: u32,
}

/// Call Detail Record
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CallDetailRecord {
    pub call_id: String,
    pub calling_number: String,
    pub called_number: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub duration_seconds: u32,
    pub ingress_trunk_id: i32,
    pub egress_trunk_id: Option<i32>,
    pub termination_cause: String,
    pub cost: Option<f64>,
    pub customer_id: Option<i32>,
    
    // ANI-II information for billing and classification
    pub ani_ii_digit: Option<u8>,
    pub payphone_surcharge: Option<f64>,
}

/// Health status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Critical,
}

/// Event type enumeration for filtering
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventType {
    CallInitiated,
    CallRouted, 
    CallConnected,
    CallTerminated,
    RouteAdvanced,
    FraudDetected,
    HealthStatus,
    ConfigChanged,
    PluginEvent,
    All, // Special type for handlers that want all events
}

impl From<&TelecomEvent> for EventType {
    fn from(event: &TelecomEvent) -> Self {
        match event {
            TelecomEvent::CallInitiated(_) => EventType::CallInitiated,
            TelecomEvent::CallRouted(_) => EventType::CallRouted,
            TelecomEvent::CallConnected(_) => EventType::CallConnected,
            TelecomEvent::CallTerminated(_) => EventType::CallTerminated,
            TelecomEvent::RouteAdvanced(_) => EventType::RouteAdvanced,
            TelecomEvent::FraudDetected(_) => EventType::FraudDetected,
            TelecomEvent::HealthStatus(_) => EventType::HealthStatus,
            TelecomEvent::ConfigChanged(_) => EventType::ConfigChanged,
            TelecomEvent::PluginEvent(_) => EventType::PluginEvent,
        }
    }
}

/// Async event handler trait
#[async_trait]
pub trait EventHandler: Send + Sync {
    /// Handle an incoming event
    async fn handle_event(&self, event: &TelecomEvent) -> Result<()>;

    /// Get the handler name for logging and debugging
    fn name(&self) -> &str;

    /// Get event types this handler is interested in
    fn interested_events(&self) -> Vec<EventType>;

    /// Check if handler is healthy and can process events
    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

/// Event handler registration info
pub struct HandlerRegistration {
    pub handler: Arc<dyn EventHandler>,
    pub event_types: Vec<EventType>,
    pub created_at: DateTime<Utc>,
    pub last_health_check: Option<DateTime<Utc>>,
    pub error_count: u64,
}

/// Event processing statistics
#[derive(Debug, Clone, Default)]
pub struct EventStats {
    pub total_events_published: u64,
    pub total_events_handled: u64,
    pub events_by_type: HashMap<String, u64>,
    pub handler_error_count: u64,
    pub average_processing_time_ms: f64,
}

/// Event filter for selective subscription
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventFilter {
    pub event_types: Vec<EventType>,
    pub call_id_pattern: Option<String>,
    pub source_service: Option<String>,
    pub min_timestamp: Option<DateTime<Utc>>,
}

impl Default for EventFilter {
    fn default() -> Self {
        Self {
            event_types: vec![EventType::All],
            call_id_pattern: None,
            source_service: None,
            min_timestamp: None,
        }
    }
}

impl EventFilter {
    /// Create a new filter for specific event types
    pub fn for_types(types: Vec<EventType>) -> Self {
        Self {
            event_types: types,
            ..Default::default()
        }
    }

    /// Check if event matches this filter
    pub fn matches(&self, event: &TelecomEvent) -> bool {
        // Check event type
        let event_type = EventType::from(event);
        if !self.event_types.contains(&EventType::All) && !self.event_types.contains(&event_type) {
            return false;
        }

        // Check call ID pattern if specified
        if let Some(pattern) = &self.call_id_pattern {
            let call_id = match event {
                TelecomEvent::CallInitiated(e) => &e.call_id,
                TelecomEvent::CallRouted(e) => &e.call_id,
                TelecomEvent::CallConnected(e) => &e.call_id,
                TelecomEvent::CallTerminated(e) => &e.call_id,
                TelecomEvent::RouteAdvanced(e) => &e.call_id,
                _ => return true, // Non-call events pass call ID filter
            };
            
            if !call_id.contains(pattern) {
                return false;
            }
        }

        // Check timestamp if specified
        if let Some(min_time) = self.min_timestamp {
            let event_time = match event {
                TelecomEvent::CallInitiated(e) => e.timestamp,
                TelecomEvent::CallRouted(e) => e.timestamp,
                TelecomEvent::CallConnected(e) => e.timestamp,
                TelecomEvent::CallTerminated(e) => e.timestamp,
                TelecomEvent::RouteAdvanced(e) => e.timestamp,
                TelecomEvent::FraudDetected(e) => e.timestamp,
                TelecomEvent::HealthStatus(e) => e.timestamp,
                TelecomEvent::ConfigChanged(e) => e.timestamp,
                TelecomEvent::PluginEvent(e) => e.timestamp,
            };
            
            if event_time < min_time {
                return false;
            }
        }

        true
    }
}

/// Helper functions for creating events
impl TelecomEvent {
    /// Create a call initiated event
    pub fn call_initiated(
        call_id: String,
        session_id: String,
        calling_number: String,
        called_number: String,
        source_ip: IpAddr,
    ) -> Self {
        TelecomEvent::CallInitiated(CallInitiatedEvent {
            call_id,
            session_id,
            calling_number,
            called_number,
            source_ip,
            timestamp: Utc::now(),
            trunk_id: None,
            customer_id: None,
            user_agent: None,
            sdp_offered: false,
        })
    }

    /// Create a fraud detected event
    pub fn fraud_detected(
        alert_id: String,
        fraud_type: String,
        risk_score: f64,
        details: HashMap<String, String>,
    ) -> Self {
        TelecomEvent::FraudDetected(FraudDetectedEvent {
            alert_id,
            call_id: None,
            session_id: None,
            fraud_type,
            risk_score,
            source_ip: None,
            calling_number: None,
            details,
            timestamp: Utc::now(),
        })
    }

    /// Create a health status event
    pub fn health_status(
        service_name: String,
        instance_id: String,
        status: HealthStatus,
        metrics: HashMap<String, f64>,
    ) -> Self {
        TelecomEvent::HealthStatus(HealthStatusEvent {
            service_name,
            instance_id,
            status,
            metrics,
            timestamp: Utc::now(),
        })
    }

    /// Get event ID for correlation
    pub fn event_id(&self) -> String {
        Uuid::new_v4().to_string()
    }

    /// Get call ID if this is a call-related event
    pub fn call_id(&self) -> Option<&str> {
        match self {
            TelecomEvent::CallInitiated(e) => Some(&e.call_id),
            TelecomEvent::CallRouted(e) => Some(&e.call_id),
            TelecomEvent::CallConnected(e) => Some(&e.call_id),
            TelecomEvent::CallTerminated(e) => Some(&e.call_id),
            TelecomEvent::RouteAdvanced(e) => Some(&e.call_id),
            TelecomEvent::FraudDetected(e) => e.call_id.as_deref(),
            _ => None,
        }
    }

    /// Get timestamp of the event
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            TelecomEvent::CallInitiated(e) => e.timestamp,
            TelecomEvent::CallRouted(e) => e.timestamp,
            TelecomEvent::CallConnected(e) => e.timestamp,
            TelecomEvent::CallTerminated(e) => e.timestamp,
            TelecomEvent::RouteAdvanced(e) => e.timestamp,
            TelecomEvent::FraudDetected(e) => e.timestamp,
            TelecomEvent::HealthStatus(e) => e.timestamp,
            TelecomEvent::ConfigChanged(e) => e.timestamp,
            TelecomEvent::PluginEvent(e) => e.timestamp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_event_type_conversion() {
        let event = TelecomEvent::call_initiated(
            "test-call".to_string(),
            "test-session".to_string(),
            "1234567890".to_string(),
            "0987654321".to_string(),
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        );

        let event_type = EventType::from(&event);
        assert_eq!(event_type, EventType::CallInitiated);
    }

    #[test]
    fn test_event_filter_matches() {
        let filter = EventFilter::for_types(vec![EventType::CallInitiated]);
        
        let matching_event = TelecomEvent::call_initiated(
            "test-call".to_string(),
            "test-session".to_string(),
            "1234567890".to_string(),
            "0987654321".to_string(),
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        );

        let non_matching_event = TelecomEvent::health_status(
            "test-service".to_string(),
            "instance-1".to_string(),
            HealthStatus::Healthy,
            HashMap::new(),
        );

        assert!(filter.matches(&matching_event));
        assert!(!filter.matches(&non_matching_event));
    }

    #[test]
    fn test_call_id_extraction() {
        let event = TelecomEvent::call_initiated(
            "test-call-123".to_string(),
            "test-session".to_string(),
            "1234567890".to_string(),
            "0987654321".to_string(),
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        );

        assert_eq!(event.call_id(), Some("test-call-123"));
    }
}