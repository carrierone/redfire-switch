//! Example B2BUA plugin implementations
//!
//! This module contains example plugins that demonstrate various capabilities
//! and patterns for B2BUA plugin development.

use super::{B2BUAPlugin, PluginCapability, PluginConfig, PluginContext, PluginMetadata};
use crate::events::TelecomEvent;
use crate::services::signaling::{PluginResponse, SipMessage};
use anyhow::Result;
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Default B2BUA plugin that simply forwards messages
pub struct DefaultB2BUAExample {
    metadata: PluginMetadata,
    message_count: AtomicU64,
}

impl DefaultB2BUAExample {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                name: "default-b2bua".to_string(),
                version: "1.0.0".to_string(),
                description: "Default B2BUA plugin that forwards all messages".to_string(),
                author: "RedFire Switch Team".to_string(),
                license: "MIT".to_string(),
                min_system_version: "1.0.0".to_string(),
                dependencies: vec![],
                config_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "log_messages": {
                            "type": "boolean",
                            "description": "Whether to log processed messages",
                            "default": false
                        }
                    }
                })),
                capabilities: vec![PluginCapability::SipInvite, PluginCapability::SipResponse],
            },
            message_count: AtomicU64::new(0),
        }
    }
}

#[async_trait::async_trait]
impl B2BUAPlugin for DefaultB2BUAExample {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn initialize(&mut self, _config: &PluginConfig, _context: &PluginContext) -> Result<()> {
        info!("Default B2BUA plugin initialized");
        Ok(())
    }

    async fn handle_message(
        &self,
        message: &SipMessage,
        context: &PluginContext,
    ) -> Result<PluginResponse> {
        let count = self.message_count.fetch_add(1, Ordering::Relaxed) + 1;

        debug!(
            "Default B2BUA plugin processing message #{}: {}",
            count, message.method
        );

        // Simple logging if call session is available
        if let Some(call_session) = &context.call_session {
            debug!("Processing message for call: {}", call_session.call_id);
        }

        // Always forward the message unchanged
        Ok(PluginResponse::Forward(message.clone()))
    }

    async fn get_statistics(&self) -> Result<HashMap<String, serde_json::Value>> {
        let mut stats = HashMap::new();
        stats.insert(
            "messages_processed".to_string(),
            json!(self.message_count.load(Ordering::Relaxed)),
        );
        Ok(stats)
    }
}

/// SIP Authentication plugin
pub struct SipAuthenticatorPlugin {
    metadata: PluginMetadata,
    credentials: Arc<RwLock<HashMap<String, String>>>, // username -> password
    failed_attempts: Arc<RwLock<HashMap<String, u32>>>, // IP -> attempt count
}

impl SipAuthenticatorPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                name: "sip-authenticator".to_string(),
                version: "1.0.0".to_string(),
                description: "SIP authentication plugin with digest auth support".to_string(),
                author: "RedFire Switch Team".to_string(),
                license: "MIT".to_string(),
                min_system_version: "1.0.0".to_string(),
                dependencies: vec![],
                config_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "realm": {
                            "type": "string",
                            "description": "SIP authentication realm",
                            "default": "redfire-switch"
                        },
                        "max_failed_attempts": {
                            "type": "integer",
                            "description": "Maximum failed attempts before blocking IP",
                            "default": 3
                        }
                    }
                })),
                capabilities: vec![
                    PluginCapability::SipInvite,
                    PluginCapability::SecurityValidation,
                ],
            },
            credentials: Arc::new(RwLock::new(HashMap::new())),
            failed_attempts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn is_authenticated(&self, message: &SipMessage) -> bool {
        // Check for Authorization header
        if let Some(auth_header) = message.headers.get("Authorization") {
            if auth_header.starts_with("Digest ") {
                // TODO: Implement proper digest authentication validation
                return true; // Simplified for example
            }
        }
        false
    }

    async fn should_challenge(&self, message: &SipMessage) -> bool {
        // Challenge INVITE requests without authentication
        message.method == "INVITE" && !self.is_authenticated(message).await
    }
}

#[async_trait::async_trait]
impl B2BUAPlugin for SipAuthenticatorPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn initialize(&mut self, config: &PluginConfig, _context: &PluginContext) -> Result<()> {
        // Load credentials from config
        if let Some(creds) = config.config.get("credentials").and_then(|v| v.as_object()) {
            let mut credentials = self.credentials.write().await;
            for (username, password) in creds {
                if let Some(pass_str) = password.as_str() {
                    credentials.insert(username.clone(), pass_str.to_string());
                }
            }
        }

        info!("SIP Authenticator plugin initialized");
        Ok(())
    }

    async fn handle_message(
        &self,
        message: &SipMessage,
        _context: &PluginContext,
    ) -> Result<PluginResponse> {
        if self.should_challenge(message).await {
            debug!(
                "Challenging unauthenticated INVITE from {}",
                message.source_addr
            );

            return Ok(PluginResponse::Reject(401, "Unauthorized".to_string()));
        }

        Ok(PluginResponse::Forward(message.clone()))
    }
}

/// Call limiter plugin to prevent resource exhaustion
pub struct CallLimiterPlugin {
    metadata: PluginMetadata,
    max_calls: u32,
    current_calls: AtomicU64,
}

impl CallLimiterPlugin {
    pub fn new(max_calls: u32) -> Self {
        Self {
            metadata: PluginMetadata {
                name: "call-limiter".to_string(),
                version: "1.0.0".to_string(),
                description: "Limits concurrent calls to prevent system overload".to_string(),
                author: "RedFire Switch Team".to_string(),
                license: "MIT".to_string(),
                min_system_version: "1.0.0".to_string(),
                dependencies: vec![],
                config_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "max_calls": {
                            "type": "integer",
                            "description": "Maximum concurrent calls allowed",
                            "minimum": 1,
                            "default": 1000
                        }
                    }
                })),
                capabilities: vec![PluginCapability::SipInvite, PluginCapability::CallRouting],
            },
            max_calls,
            current_calls: AtomicU64::new(0),
        }
    }
}

#[async_trait::async_trait]
impl B2BUAPlugin for CallLimiterPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn initialize(&mut self, config: &PluginConfig, _context: &PluginContext) -> Result<()> {
        if let Some(max_calls) = config.config.get("max_calls").and_then(|v| v.as_u64()) {
            self.max_calls = max_calls as u32;
        }

        info!(
            "Call limiter plugin initialized with max {} calls",
            self.max_calls
        );
        Ok(())
    }

    async fn handle_message(
        &self,
        message: &SipMessage,
        _context: &PluginContext,
    ) -> Result<PluginResponse> {
        if message.method == "INVITE" {
            let current = self.current_calls.load(Ordering::Relaxed);

            if current >= self.max_calls as u64 {
                warn!(
                    "Rejecting call due to limit reached: {} >= {}",
                    current, self.max_calls
                );
                return Ok(PluginResponse::Reject(
                    503,
                    "Service Unavailable - Call limit reached".to_string(),
                ));
            }

            self.current_calls.fetch_add(1, Ordering::Relaxed);
            debug!("Call accepted, current calls: {}", current + 1);
        } else if message.method == "BYE"
            || (message.method.chars().all(char::is_numeric)
                && message.method.parse::<u16>().unwrap_or(0) >= 400)
        {
            // Call ended or failed
            self.current_calls.fetch_sub(1, Ordering::Relaxed);
        }

        Ok(PluginResponse::Forward(message.clone()))
    }

    async fn get_statistics(&self) -> Result<HashMap<String, serde_json::Value>> {
        let mut stats = HashMap::new();
        stats.insert(
            "current_calls".to_string(),
            json!(self.current_calls.load(Ordering::Relaxed)),
        );
        stats.insert("max_calls".to_string(), json!(self.max_calls));
        Ok(stats)
    }
}

/// Header manipulation plugin
pub struct HeaderManipulatorPlugin {
    metadata: PluginMetadata,
    header_rules: Arc<RwLock<Vec<HeaderRule>>>,
}

#[derive(Debug, Clone)]
struct HeaderRule {
    action: HeaderAction,
    header_name: String,
    header_value: Option<String>,
    condition: Option<String>,
}

#[derive(Debug, Clone)]
enum HeaderAction {
    Add,
    Remove,
    Replace,
    Modify,
}

impl HeaderManipulatorPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                name: "header-manipulator".to_string(),
                version: "1.0.0".to_string(),
                description: "Manipulates SIP headers based on configurable rules".to_string(),
                author: "RedFire Switch Team".to_string(),
                license: "MIT".to_string(),
                min_system_version: "1.0.0".to_string(),
                dependencies: vec![],
                config_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "rules": {
                            "type": "array",
                            "description": "Header manipulation rules",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "action": {
                                        "type": "string",
                                        "enum": ["add", "remove", "replace", "modify"]
                                    },
                                    "header_name": {
                                        "type": "string"
                                    },
                                    "header_value": {
                                        "type": "string"
                                    },
                                    "condition": {
                                        "type": "string",
                                        "description": "Optional condition for when to apply the rule"
                                    }
                                },
                                "required": ["action", "header_name"]
                            }
                        }
                    }
                })),
                capabilities: vec![PluginCapability::SipInvite, PluginCapability::SipResponse],
            },
            header_rules: Arc::new(RwLock::new(Vec::new())),
        }
    }

    async fn apply_header_rules(&self, message: &SipMessage) -> Result<SipMessage> {
        let mut modified_message = message.clone();
        let rules = self.header_rules.read().await;

        for rule in rules.iter() {
            // TODO: Implement condition checking

            match rule.action {
                HeaderAction::Add => {
                    if let Some(value) = &rule.header_value {
                        modified_message
                            .headers
                            .insert(rule.header_name.clone(), value.clone());
                    }
                }
                HeaderAction::Remove => {
                    modified_message.headers.remove(&rule.header_name);
                }
                HeaderAction::Replace => {
                    if let Some(value) = &rule.header_value {
                        modified_message
                            .headers
                            .insert(rule.header_name.clone(), value.clone());
                    }
                }
                HeaderAction::Modify => {
                    // TODO: Implement header value modification
                }
            }
        }

        Ok(modified_message)
    }
}

#[async_trait::async_trait]
impl B2BUAPlugin for HeaderManipulatorPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn initialize(&mut self, config: &PluginConfig, _context: &PluginContext) -> Result<()> {
        // Load header rules from config
        if let Some(rules_array) = config.config.get("rules").and_then(|v| v.as_array()) {
            let mut rules = self.header_rules.write().await;

            for rule_value in rules_array {
                if let Some(rule_obj) = rule_value.as_object() {
                    if let (Some(action_str), Some(header_name)) = (
                        rule_obj.get("action").and_then(|v| v.as_str()),
                        rule_obj.get("header_name").and_then(|v| v.as_str()),
                    ) {
                        let action = match action_str {
                            "add" => HeaderAction::Add,
                            "remove" => HeaderAction::Remove,
                            "replace" => HeaderAction::Replace,
                            "modify" => HeaderAction::Modify,
                            _ => continue,
                        };

                        let header_value = rule_obj
                            .get("header_value")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        let condition = rule_obj
                            .get("condition")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());

                        rules.push(HeaderRule {
                            action,
                            header_name: header_name.to_string(),
                            header_value,
                            condition,
                        });
                    }
                }
            }
        }

        let rule_count = self.header_rules.read().await.len();
        info!(
            "Header manipulator plugin initialized with {} rules",
            rule_count
        );
        Ok(())
    }

    async fn handle_message(
        &self,
        message: &SipMessage,
        _context: &PluginContext,
    ) -> Result<PluginResponse> {
        let modified_message = self.apply_header_rules(message).await?;
        Ok(PluginResponse::Modify(modified_message))
    }
}

/// Fraud detection plugin
pub struct FraudDetectorPlugin {
    metadata: PluginMetadata,
    suspicious_patterns: Arc<RwLock<Vec<String>>>,
    call_rates: Arc<RwLock<HashMap<String, Vec<chrono::DateTime<chrono::Utc>>>>>, // IP -> timestamps
}

impl FraudDetectorPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                name: "fraud-detector".to_string(),
                version: "1.0.0".to_string(),
                description: "Detects potentially fraudulent call patterns".to_string(),
                author: "RedFire Switch Team".to_string(),
                license: "MIT".to_string(),
                min_system_version: "1.0.0".to_string(),
                dependencies: vec![],
                config_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "max_calls_per_minute": {
                            "type": "integer",
                            "description": "Maximum calls per minute from single IP",
                            "default": 30
                        },
                        "suspicious_prefixes": {
                            "type": "array",
                            "description": "Phone number prefixes considered suspicious",
                            "items": {"type": "string"},
                            "default": ["900", "976"]
                        }
                    }
                })),
                capabilities: vec![
                    PluginCapability::SipInvite,
                    PluginCapability::SecurityValidation,
                ],
            },
            suspicious_patterns: Arc::new(RwLock::new(vec!["900".to_string(), "976".to_string()])),
            call_rates: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn is_suspicious_number(&self, number: &str) -> bool {
        let patterns = self.suspicious_patterns.read().await;
        patterns.iter().any(|pattern| number.starts_with(pattern))
    }

    async fn check_call_rate(&self, ip: &str) -> bool {
        let mut rates = self.call_rates.write().await;
        let now = chrono::Utc::now();
        let one_minute_ago = now - chrono::Duration::minutes(1);

        let timestamps = rates.entry(ip.to_string()).or_insert_with(Vec::new);

        // Remove old timestamps
        timestamps.retain(|&ts| ts > one_minute_ago);

        // Add current timestamp
        timestamps.push(now);

        // Check if rate is exceeded (simplified: 30 calls per minute)
        timestamps.len() > 30
    }
}

#[async_trait::async_trait]
impl B2BUAPlugin for FraudDetectorPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn initialize(&mut self, config: &PluginConfig, _context: &PluginContext) -> Result<()> {
        if let Some(prefixes) = config
            .config
            .get("suspicious_prefixes")
            .and_then(|v| v.as_array())
        {
            let mut patterns = self.suspicious_patterns.write().await;
            patterns.clear();

            for prefix in prefixes {
                if let Some(prefix_str) = prefix.as_str() {
                    patterns.push(prefix_str.to_string());
                }
            }
        }

        info!("Fraud detector plugin initialized");
        Ok(())
    }

    async fn handle_message(
        &self,
        message: &SipMessage,
        _context: &PluginContext,
    ) -> Result<PluginResponse> {
        if message.method == "INVITE" {
            let source_ip = message.source_addr.ip().to_string();

            // Check call rate
            if self.check_call_rate(&source_ip).await {
                warn!("Blocking high call rate from IP: {}", source_ip);
                return Ok(PluginResponse::Reject(429, "Too Many Requests".to_string()));
            }

            // Check for suspicious destination numbers
            if let Some(to_header) = message.headers.get("To") {
                // Extract number from To header (simplified parsing)
                if let Some(number_start) = to_header.find("sip:") {
                    let number_part = &to_header[number_start + 4..];
                    if let Some(number_end) = number_part.find('@') {
                        let number = &number_part[..number_end];

                        if self.is_suspicious_number(number).await {
                            warn!("Blocking suspicious destination number: {}", number);
                            return Ok(PluginResponse::Reject(
                                403,
                                "Forbidden - Suspicious destination".to_string(),
                            ));
                        }
                    }
                }
            }
        }

        Ok(PluginResponse::Forward(message.clone()))
    }

    async fn handle_event(&self, event: &TelecomEvent, context: &PluginContext) -> Result<()> {
        // Listen for fraud detection events to update patterns
        if let TelecomEvent::FraudDetected(fraud_event) = event {
            info!("Fraud detected event received: {}", fraud_event.fraud_type);

            // Publish alert through event bus
            let alert_event = TelecomEvent::fraud_detected(
                format!("fraud-plugin-{}", uuid::Uuid::new_v4()),
                "plugin_detected".to_string(),
                fraud_event.risk_score,
                std::collections::HashMap::new(),
            );

            if let Err(e) = context.event_bus.publish(alert_event).await {
                warn!("Failed to publish fraud alert: {}", e);
            }
        }

        Ok(())
    }
}

/// CDR generation plugin
pub struct CdrGeneratorPlugin {
    metadata: PluginMetadata,
    call_records: Arc<RwLock<HashMap<String, CallRecord>>>,
}

#[derive(Debug, Clone)]
struct CallRecord {
    call_id: String,
    start_time: chrono::DateTime<chrono::Utc>,
    calling_number: String,
    called_number: String,
    source_ip: std::net::SocketAddr,
    end_time: Option<chrono::DateTime<chrono::Utc>>,
    final_response: Option<u16>,
}

impl CdrGeneratorPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                name: "cdr-generator".to_string(),
                version: "1.0.0".to_string(),
                description: "Generates Call Detail Records for billing and analytics".to_string(),
                author: "RedFire Switch Team".to_string(),
                license: "MIT".to_string(),
                min_system_version: "1.0.0".to_string(),
                dependencies: vec![],
                config_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "output_format": {
                            "type": "string",
                            "enum": ["json", "csv", "xml"],
                            "description": "CDR output format",
                            "default": "json"
                        },
                        "output_file": {
                            "type": "string",
                            "description": "File path for CDR output",
                            "default": "/var/log/redfire-switch/cdrs.log"
                        }
                    }
                })),
                capabilities: vec![
                    PluginCapability::SipInvite,
                    PluginCapability::SipResponse,
                    PluginCapability::CdrGeneration,
                ],
            },
            call_records: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn extract_call_info(&self, message: &SipMessage) -> Option<(String, String, String)> {
        let call_id = message.headers.get("Call-ID")?.clone();

        // Extract calling number from From header (simplified)
        let calling_number = message
            .headers
            .get("From")
            .and_then(|from| from.split("sip:").nth(1))
            .and_then(|part| part.split('@').next())
            .unwrap_or("unknown")
            .to_string();

        // Extract called number from To header (simplified)
        let called_number = message
            .headers
            .get("To")
            .and_then(|to| to.split("sip:").nth(1))
            .and_then(|part| part.split('@').next())
            .unwrap_or("unknown")
            .to_string();

        Some((call_id, calling_number, called_number))
    }
}

#[async_trait::async_trait]
impl B2BUAPlugin for CdrGeneratorPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn initialize(&mut self, _config: &PluginConfig, _context: &PluginContext) -> Result<()> {
        info!("CDR generator plugin initialized");
        Ok(())
    }

    async fn handle_message(
        &self,
        message: &SipMessage,
        _context: &PluginContext,
    ) -> Result<PluginResponse> {
        if message.method == "INVITE" {
            // Start CDR for new call
            if let Some((call_id, calling_number, called_number)) =
                self.extract_call_info(message).await
            {
                let record = CallRecord {
                    call_id: call_id.clone(),
                    start_time: chrono::Utc::now(),
                    calling_number,
                    called_number,
                    source_ip: message.source_addr,
                    end_time: None,
                    final_response: None,
                };

                let mut records = self.call_records.write().await;
                records.insert(call_id, record);

                debug!("Started CDR for call");
            }
        } else if message.method == "BYE" {
            // End CDR for terminated call
            if let Some(call_id) = message.headers.get("Call-ID") {
                let mut records = self.call_records.write().await;
                if let Some(record) = records.get_mut(call_id) {
                    record.end_time = Some(chrono::Utc::now());
                    record.final_response = Some(200);

                    debug!("Completed CDR for call: {}", call_id);
                    // TODO: Output CDR to file/database
                }
            }
        } else if message.method.chars().all(char::is_numeric) {
            // SIP response
            if let Ok(response_code) = message.method.parse::<u16>() {
                if response_code >= 400 {
                    // Call failed
                    if let Some(call_id) = message.headers.get("Call-ID") {
                        let mut records = self.call_records.write().await;
                        if let Some(record) = records.get_mut(call_id) {
                            record.end_time = Some(chrono::Utc::now());
                            record.final_response = Some(response_code);

                            debug!(
                                "Marked CDR as failed for call: {} ({})",
                                call_id, response_code
                            );
                        }
                    }
                }
            }
        }

        Ok(PluginResponse::Forward(message.clone()))
    }

    async fn get_statistics(&self) -> Result<HashMap<String, serde_json::Value>> {
        let records = self.call_records.read().await;
        let mut stats = HashMap::new();

        let total_calls = records.len();
        let completed_calls = records.values().filter(|r| r.end_time.is_some()).count();
        let failed_calls = records
            .values()
            .filter(|r| r.final_response.unwrap_or(0) >= 400)
            .count();

        stats.insert("total_calls".to_string(), json!(total_calls));
        stats.insert("completed_calls".to_string(), json!(completed_calls));
        stats.insert("failed_calls".to_string(), json!(failed_calls));

        Ok(stats)
    }
}
