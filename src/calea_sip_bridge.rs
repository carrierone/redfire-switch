/*
 * CALEA SIP Stack Bridge
 * Bridges between SIP stack ComplianceNotifier and main ComplianceFramework
 * Implements U.S. J-STD-025 lawful intercept requirements
 */

use crate::compliance_framework::{ComplianceFramework, CallEvent, CallEventType};
use redfire_sip_stack::core::{ComplianceNotifier, SipCallContext};
use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Bridge implementation for CALEA compliance
pub struct CaleaSipBridge {
    compliance_framework: Arc<ComplianceFramework>,
}

impl CaleaSipBridge {
    /// Create new CALEA SIP bridge
    pub fn new(compliance_framework: Arc<ComplianceFramework>) -> Self {
        info!("Initializing CALEA SIP bridge for J-STD-025 compliance");
        Self {
            compliance_framework,
        }
    }
}

impl ComplianceNotifier for CaleaSipBridge {
    /// Notify of call attempt (INVITE received) - J-STD-025 compliance
    fn notify_call_attempt(&self, context: &SipCallContext, source_ip: IpAddr) {
        let mut sip_headers = HashMap::new();
        sip_headers.insert("From-URI".to_string(), context.from_uri.clone());
        sip_headers.insert("To-URI".to_string(), context.to_uri.clone());
        sip_headers.insert("Trunk-ID".to_string(), 
                          context.trunk_id.clone().unwrap_or("unknown".to_string()));
        sip_headers.insert("Transport".to_string(), format!("{:?}", context.transport));
        
        // Add U.S. jurisdiction flag for CALEA
        sip_headers.insert("CALEA-Jurisdiction".to_string(), "US".to_string());
        
        let call_event = CallEvent {
            call_id: context.call_id.clone(),
            event_type: CallEventType::CallAttempt,
            timestamp: Utc::now(),
            calling_number: context.calling_number.clone(),
            called_number: context.called_number.clone(),
            sip_method: Some("INVITE".to_string()),
            sip_response_code: None,
            source_ip: Some(source_ip),
            dest_ip: Some(context.source_ip.ip()),
            user_agent: None,
            sip_headers,
            rtp_stats: None,
        };
        
        if let Err(e) = self.compliance_framework.submit_call_event(call_event) {
            warn!("CALEA: Failed to submit call attempt event: {}", e);
        } else {
            debug!("CALEA: Call attempt reported for J-STD-025 compliance: {}", context.call_id);
        }
    }
    
    /// Notify of call establishment (200 OK sent/received) - J-STD-025 compliance  
    fn notify_call_established(&self, context: &SipCallContext) {
        let call_event = CallEvent {
            call_id: context.call_id.clone(),
            event_type: CallEventType::CallAnswered,
            timestamp: Utc::now(),
            calling_number: context.calling_number.clone(),
            called_number: context.called_number.clone(),
            sip_method: None,
            sip_response_code: Some(200),
            source_ip: Some(context.source_ip.ip()),
            dest_ip: None,
            user_agent: None,
            sip_headers: HashMap::new(),
            rtp_stats: None,
        };
        
        if let Err(e) = self.compliance_framework.submit_call_event(call_event) {
            warn!("CALEA: Failed to submit call established event: {}", e);
        } else {
            debug!("CALEA: Call established reported for J-STD-025 compliance: {}", context.call_id);
        }
    }
    
    /// Notify of call termination (BYE/error response) - J-STD-025 compliance
    fn notify_call_terminated(&self, context: &SipCallContext, termination_reason: &str) {
        let mut sip_headers = HashMap::new();
        sip_headers.insert("Termination-Reason".to_string(), termination_reason.to_string());
        sip_headers.insert("CALEA-Jurisdiction".to_string(), "US".to_string());
        
        let call_event = CallEvent {
            call_id: context.call_id.clone(),
            event_type: CallEventType::CallEnded,
            timestamp: Utc::now(),
            calling_number: context.calling_number.clone(),
            called_number: context.called_number.clone(),
            sip_method: Some("BYE".to_string()),
            sip_response_code: None,
            source_ip: Some(context.source_ip.ip()),
            dest_ip: None,
            user_agent: None,
            sip_headers,
            rtp_stats: None,
        };
        
        if let Err(e) = self.compliance_framework.submit_call_event(call_event) {
            warn!("CALEA: Failed to submit call termination event: {}", e);
        } else {
            info!("CALEA: Call termination reported for J-STD-025 compliance: {} ({})", 
                  context.call_id, termination_reason);
        }
    }
    
    /// Notify of SIP method processing (for CDR generation) - J-STD-025 compliance
    fn notify_sip_method(&self, call_id: &str, method: &str, response_code: Option<u16>, source_ip: IpAddr) {
        let mut sip_headers = HashMap::new();
        sip_headers.insert("SIP-Method".to_string(), method.to_string());
        if let Some(code) = response_code {
            sip_headers.insert("Response-Code".to_string(), code.to_string());
        }
        sip_headers.insert("CALEA-Jurisdiction".to_string(), "US".to_string());
        
        // Use appropriate event type based on SIP method
        let event_type = match method {
            "INVITE" => CallEventType::CallAttempt,
            "BYE" => CallEventType::CallEnded,
            "CANCEL" => CallEventType::CallEnded,
            _ => CallEventType::CallProgress, // Generic for other methods
        };
        
        let call_event = CallEvent {
            call_id: call_id.to_string(),
            event_type,
            timestamp: Utc::now(),
            calling_number: "unknown".to_string(), // Will be filled by compliance framework if available
            called_number: "unknown".to_string(),  // Will be filled by compliance framework if available
            sip_method: Some(method.to_string()),
            sip_response_code: response_code,
            source_ip: Some(source_ip),
            dest_ip: None,
            user_agent: None,
            sip_headers,
            rtp_stats: None,
        };
        
        if let Err(e) = self.compliance_framework.submit_call_event(call_event) {
            debug!("CALEA: Failed to submit SIP method event: {}", e);
        } else {
            debug!("CALEA: SIP method {} reported for J-STD-025 compliance: {}", method, call_id);
        }
    }
}