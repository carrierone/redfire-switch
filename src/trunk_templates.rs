/*
 * Redfire Switch - Trunk Template System
 * Copyright (C) 2025 Carrier One Inc and contributors
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

//! # Trunk Template System
//! 
//! Provides a template-based configuration system for trunks where:
//! - Named templates define common trunk settings
//! - Individual trunks inherit from templates and override specific settings
//! - Templates support inheritance hierarchies
//! - Settings cascade from template to trunk-specific overrides

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::termination_routing::{TrunkCnamConfig, TrunkCodecConfig};
use crate::codec::AudioCodec;
use crate::routing_engine::DefaultRouting;

/// Trunk template with named configuration that can be inherited by actual trunks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrunkTemplate {
    /// Template name (unique identifier)
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Parent template to inherit from (optional)
    pub inherit_from: Option<String>,
    /// Template version for change tracking
    pub version: String,
    /// Creation/modification timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Template configuration settings
    pub settings: TrunkTemplateSettings,
}

/// Template settings that can be inherited and overridden
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrunkTemplateSettings {
    // === Basic Settings ===
    /// Enable/disable trunks using this template
    pub enabled: Option<bool>,
    /// Maximum concurrent calls
    pub max_concurrent_calls: Option<u32>,
    /// Call rate limiting (calls per second)
    pub rate_limit_cps: Option<u32>,
    
    // === Authentication ===
    /// Authentication required
    pub auth_required: Option<bool>,
    /// Default username pattern for authentication
    pub auth_username_template: Option<String>,
    /// Source IP restrictions (CIDR blocks)
    pub allowed_source_ips: Option<Vec<String>>,
    /// Require digest authentication
    pub require_digest_auth: Option<bool>,
    
    // === SIP Settings ===
    /// SIP profile to use
    pub sip_profile: Option<String>,
    /// SIP transport protocol preference
    pub preferred_transport: Option<String>,
    /// Session timers configuration
    pub session_timers_enabled: Option<bool>,
    /// Session timer value in seconds
    pub session_timer_seconds: Option<u32>,
    /// Contact header rewriting
    pub rewrite_contact_header: Option<bool>,
    
    // === Routing ===
    /// Default routing behavior
    pub default_routing: Option<DefaultRouting>,
    /// On-net routing enabled
    pub on_net_routing_enabled: Option<bool>,
    /// Off-net routing enabled
    pub off_net_routing_enabled: Option<bool>,
    /// Allowed destination patterns
    pub allowed_destinations: Option<Vec<String>>,
    /// Blocked destination patterns
    pub blocked_destinations: Option<Vec<String>>,
    
    // === Media/Codec ===
    /// Codec configuration
    pub codec_config: Option<TrunkCodecConfig>,
    /// Enable media anchoring
    pub media_anchoring_enabled: Option<bool>,
    /// RTP timeout in seconds
    pub rtp_timeout_seconds: Option<u32>,
    /// Enable SRTP
    pub srtp_enabled: Option<bool>,
    
    // === CNAM Settings ===
    /// CNAM configuration
    pub cnam_config: Option<TrunkCnamConfig>,
    
    // === Billing ===
    /// Billing profile to use
    pub billing_profile: Option<String>,
    /// Enable billing for this trunk template
    pub billing_enabled: Option<bool>,
    /// Default rate per minute (in cents)
    pub default_rate_cents: Option<f64>,
    
    // === Quality Settings ===
    /// Minimum MOS score for call completion
    pub min_mos_score: Option<f32>,
    /// Enable call recording
    pub call_recording_enabled: Option<bool>,
    /// Recording retention days
    pub recording_retention_days: Option<u32>,
    
    // === Emergency Settings ===
    /// Emergency routing behavior
    pub emergency_routing_enabled: Option<bool>,
    /// Route emergency calls back to originating provider
    pub emergency_route_to_origin: Option<bool>,
    
    // === STIR/SHAKEN ===
    /// STIR/SHAKEN verification required
    pub stir_shaken_verification: Option<bool>,
    /// Default attestation level for outbound calls
    pub default_attestation_level: Option<String>,
    
    // === Monitoring ===
    /// Enable health monitoring
    pub monitoring_enabled: Option<bool>,
    /// Health check interval in seconds
    pub health_check_interval: Option<u32>,
    /// Health check timeout in seconds
    pub health_check_timeout: Option<u32>,
    
    // === Advanced Settings ===
    /// Custom SIP headers to add
    pub custom_sip_headers: Option<HashMap<String, String>>,
    /// Number manipulation rules
    pub number_manipulation_rules: Option<Vec<String>>,
    /// Enable debug logging for this template
    pub debug_logging: Option<bool>,
    /// Priority for load balancing (1 = highest)
    pub priority: Option<u32>,
    /// Weight for load balancing (higher = more traffic)
    pub weight: Option<u32>,
}

impl Default for TrunkTemplateSettings {
    fn default() -> Self {
        Self {
            enabled: Some(true),
            max_concurrent_calls: Some(1000),
            rate_limit_cps: Some(100),
            auth_required: Some(false),
            auth_username_template: None,
            allowed_source_ips: None,
            require_digest_auth: Some(false),
            sip_profile: Some("default".to_string()),
            preferred_transport: Some("UDP".to_string()),
            session_timers_enabled: Some(true),
            session_timer_seconds: Some(1800),
            rewrite_contact_header: Some(false),
            default_routing: Some(DefaultRouting::OnNetFirst),
            on_net_routing_enabled: Some(true),
            off_net_routing_enabled: Some(true),
            allowed_destinations: None,
            blocked_destinations: None,
            codec_config: None,
            media_anchoring_enabled: Some(true),
            rtp_timeout_seconds: Some(60),
            srtp_enabled: Some(false),
            cnam_config: None,
            billing_profile: Some("default".to_string()),
            billing_enabled: Some(true),
            default_rate_cents: None,
            min_mos_score: Some(3.0),
            call_recording_enabled: Some(false),
            recording_retention_days: Some(30),
            emergency_routing_enabled: Some(true),
            emergency_route_to_origin: Some(true),
            stir_shaken_verification: Some(false),
            default_attestation_level: Some("C".to_string()),
            monitoring_enabled: Some(true),
            health_check_interval: Some(30),
            health_check_timeout: Some(5),
            custom_sip_headers: None,
            number_manipulation_rules: None,
            debug_logging: Some(false),
            priority: Some(1),
            weight: Some(1),
        }
    }
}

/// Trunk-specific overrides that apply to individual trunk configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrunkOverrides {
    /// Override enabled status
    pub enabled: Option<bool>,
    /// Override max concurrent calls
    pub max_concurrent_calls: Option<u32>,
    /// Override rate limit
    pub rate_limit_cps: Option<u32>,
    /// Override authentication requirement
    pub auth_required: Option<bool>,
    /// Override allowed source IPs
    pub allowed_source_ips: Option<Vec<String>>,
    /// Override SIP profile
    pub sip_profile: Option<String>,
    /// Override routing behavior
    pub default_routing: Option<DefaultRouting>,
    /// Override codec configuration
    pub codec_config: Option<TrunkCodecConfig>,
    /// Override CNAM configuration
    pub cnam_config: Option<TrunkCnamConfig>,
    /// Override billing profile
    pub billing_profile: Option<String>,
    /// Override custom SIP headers
    pub custom_sip_headers: Option<HashMap<String, String>>,
    /// Override priority
    pub priority: Option<u32>,
    /// Override weight
    pub weight: Option<u32>,
    /// Additional allowed destinations
    pub additional_allowed_destinations: Option<Vec<String>>,
    /// Additional blocked destinations
    pub additional_blocked_destinations: Option<Vec<String>>,
}

/// Configuration linking trunks to templates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrunkTemplateAssignment {
    /// Trunk ID
    pub trunk_id: String,
    /// Template name to inherit from
    pub template_name: String,
    /// Trunk-specific overrides
    pub overrides: TrunkOverrides,
    /// Last applied timestamp
    pub applied_at: chrono::DateTime<chrono::Utc>,
}

/// Template manager for handling trunk templates and inheritance
#[derive(Debug)]
pub struct TrunkTemplateManager {
    /// Available templates by name
    templates: HashMap<String, TrunkTemplate>,
    /// Template assignments for trunks
    assignments: HashMap<String, TrunkTemplateAssignment>,
}

impl TrunkTemplateManager {
    /// Create new template manager
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
            assignments: HashMap::new(),
        }
    }
    
    /// Add or update a template
    pub fn add_template(&mut self, template: TrunkTemplate) -> Result<()> {
        // Validate template doesn't create circular inheritance
        self.validate_inheritance(&template)?;
        
        info!("Adding trunk template: {}", template.name);
        self.templates.insert(template.name.clone(), template);
        Ok(())
    }
    
    /// Remove a template
    pub fn remove_template(&mut self, template_name: &str) -> Result<()> {
        // Check if any trunks are using this template
        let using_trunks: Vec<_> = self.assignments
            .values()
            .filter(|a| a.template_name == template_name)
            .map(|a| a.trunk_id.clone())
            .collect();
            
        if !using_trunks.is_empty() {
            return Err(anyhow!(
                "Cannot remove template '{}': in use by trunks: {}",
                template_name,
                using_trunks.join(", ")
            ));
        }
        
        // Check if any other templates inherit from this one
        let dependent_templates: Vec<_> = self.templates
            .values()
            .filter_map(|t| {
                if let Some(ref parent) = t.inherit_from {
                    if parent == template_name {
                        Some(t.name.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
            
        if !dependent_templates.is_empty() {
            return Err(anyhow!(
                "Cannot remove template '{}': inherited by templates: {}",
                template_name,
                dependent_templates.join(", ")
            ));
        }
        
        self.templates.remove(template_name);
        info!("Removed trunk template: {}", template_name);
        Ok(())
    }
    
    /// Assign a template to a trunk with overrides
    pub fn assign_template(&mut self, trunk_id: String, template_name: String, overrides: TrunkOverrides) -> Result<()> {
        // Validate template exists
        if !self.templates.contains_key(&template_name) {
            return Err(anyhow!("Template '{}' not found", template_name));
        }
        
        let assignment = TrunkTemplateAssignment {
            trunk_id: trunk_id.clone(),
            template_name,
            overrides,
            applied_at: chrono::Utc::now(),
        };
        
        info!("Assigning template '{}' to trunk '{}'", assignment.template_name, trunk_id);
        self.assignments.insert(trunk_id, assignment);
        Ok(())
    }
    
    /// Remove template assignment from a trunk
    pub fn unassign_template(&mut self, trunk_id: &str) -> Result<()> {
        if self.assignments.remove(trunk_id).is_some() {
            info!("Removed template assignment for trunk: {}", trunk_id);
            Ok(())
        } else {
            Err(anyhow!("Trunk '{}' has no template assignment", trunk_id))
        }
    }
    
    /// Get resolved configuration for a trunk (template + overrides)
    pub fn get_trunk_config(&self, trunk_id: &str) -> Result<TrunkTemplateSettings> {
        let assignment = self.assignments.get(trunk_id)
            .ok_or_else(|| anyhow!("Trunk '{}' has no template assignment", trunk_id))?;
            
        // Resolve template with inheritance
        let template_config = self.resolve_template(&assignment.template_name)?;
        
        // Apply overrides
        let final_config = self.apply_overrides(template_config, &assignment.overrides);
        
        debug!("Resolved configuration for trunk '{}' using template '{}'", 
               trunk_id, assignment.template_name);
        Ok(final_config)
    }
    
    /// List all available templates
    pub fn list_templates(&self) -> Vec<&TrunkTemplate> {
        self.templates.values().collect()
    }
    
    /// List template assignments
    pub fn list_assignments(&self) -> Vec<&TrunkTemplateAssignment> {
        self.assignments.values().collect()
    }
    
    /// Get template by name
    pub fn get_template(&self, name: &str) -> Option<&TrunkTemplate> {
        self.templates.get(name)
    }
    
    /// Validate inheritance hierarchy doesn't create cycles
    fn validate_inheritance(&self, template: &TrunkTemplate) -> Result<()> {
        let mut visited = std::collections::HashSet::new();
        let mut current = template.inherit_from.as_deref();
        
        while let Some(parent_name) = current {
            if visited.contains(parent_name) {
                return Err(anyhow!("Circular inheritance detected for template '{}'", template.name));
            }
            visited.insert(parent_name);
            
            if let Some(parent_template) = self.templates.get(parent_name) {
                current = parent_template.inherit_from.as_deref();
            } else if parent_name != template.name {
                return Err(anyhow!("Parent template '{}' not found", parent_name));
            } else {
                break;
            }
        }
        
        Ok(())
    }
    
    /// Resolve template configuration with inheritance
    fn resolve_template(&self, template_name: &str) -> Result<TrunkTemplateSettings> {
        let mut config = TrunkTemplateSettings::default();
        let mut inheritance_chain = Vec::new();
        let mut current = Some(template_name);
        
        // Build inheritance chain from root to child
        while let Some(name) = current {
            if let Some(template) = self.templates.get(name) {
                inheritance_chain.push(template);
                current = template.inherit_from.as_deref();
            } else {
                return Err(anyhow!("Template '{}' not found in inheritance chain", name));
            }
        }
        
        // Apply settings from root to child (child overrides parent)
        for template in inheritance_chain.into_iter().rev() {
            config = self.merge_template_settings(config, &template.settings);
        }
        
        Ok(config)
    }
    
    /// Merge template settings (child overrides parent)
    fn merge_template_settings(&self, mut base: TrunkTemplateSettings, overlay: &TrunkTemplateSettings) -> TrunkTemplateSettings {
        // Merge all optional fields - if overlay has a value, it overrides base
        if overlay.enabled.is_some() { base.enabled = overlay.enabled; }
        if overlay.max_concurrent_calls.is_some() { base.max_concurrent_calls = overlay.max_concurrent_calls; }
        if overlay.rate_limit_cps.is_some() { base.rate_limit_cps = overlay.rate_limit_cps; }
        if overlay.auth_required.is_some() { base.auth_required = overlay.auth_required; }
        if overlay.auth_username_template.is_some() { base.auth_username_template = overlay.auth_username_template.clone(); }
        if overlay.allowed_source_ips.is_some() { base.allowed_source_ips = overlay.allowed_source_ips.clone(); }
        if overlay.require_digest_auth.is_some() { base.require_digest_auth = overlay.require_digest_auth; }
        if overlay.sip_profile.is_some() { base.sip_profile = overlay.sip_profile.clone(); }
        if overlay.preferred_transport.is_some() { base.preferred_transport = overlay.preferred_transport.clone(); }
        if overlay.session_timers_enabled.is_some() { base.session_timers_enabled = overlay.session_timers_enabled; }
        if overlay.session_timer_seconds.is_some() { base.session_timer_seconds = overlay.session_timer_seconds; }
        if overlay.rewrite_contact_header.is_some() { base.rewrite_contact_header = overlay.rewrite_contact_header; }
        if overlay.default_routing.is_some() { base.default_routing = overlay.default_routing.clone(); }
        if overlay.on_net_routing_enabled.is_some() { base.on_net_routing_enabled = overlay.on_net_routing_enabled; }
        if overlay.off_net_routing_enabled.is_some() { base.off_net_routing_enabled = overlay.off_net_routing_enabled; }
        if overlay.allowed_destinations.is_some() { base.allowed_destinations = overlay.allowed_destinations.clone(); }
        if overlay.blocked_destinations.is_some() { base.blocked_destinations = overlay.blocked_destinations.clone(); }
        if overlay.codec_config.is_some() { base.codec_config = overlay.codec_config.clone(); }
        if overlay.media_anchoring_enabled.is_some() { base.media_anchoring_enabled = overlay.media_anchoring_enabled; }
        if overlay.rtp_timeout_seconds.is_some() { base.rtp_timeout_seconds = overlay.rtp_timeout_seconds; }
        if overlay.srtp_enabled.is_some() { base.srtp_enabled = overlay.srtp_enabled; }
        if overlay.cnam_config.is_some() { base.cnam_config = overlay.cnam_config.clone(); }
        if overlay.billing_profile.is_some() { base.billing_profile = overlay.billing_profile.clone(); }
        if overlay.billing_enabled.is_some() { base.billing_enabled = overlay.billing_enabled; }
        if overlay.default_rate_cents.is_some() { base.default_rate_cents = overlay.default_rate_cents; }
        if overlay.min_mos_score.is_some() { base.min_mos_score = overlay.min_mos_score; }
        if overlay.call_recording_enabled.is_some() { base.call_recording_enabled = overlay.call_recording_enabled; }
        if overlay.recording_retention_days.is_some() { base.recording_retention_days = overlay.recording_retention_days; }
        if overlay.emergency_routing_enabled.is_some() { base.emergency_routing_enabled = overlay.emergency_routing_enabled; }
        if overlay.emergency_route_to_origin.is_some() { base.emergency_route_to_origin = overlay.emergency_route_to_origin; }
        if overlay.stir_shaken_verification.is_some() { base.stir_shaken_verification = overlay.stir_shaken_verification; }
        if overlay.default_attestation_level.is_some() { base.default_attestation_level = overlay.default_attestation_level.clone(); }
        if overlay.monitoring_enabled.is_some() { base.monitoring_enabled = overlay.monitoring_enabled; }
        if overlay.health_check_interval.is_some() { base.health_check_interval = overlay.health_check_interval; }
        if overlay.health_check_timeout.is_some() { base.health_check_timeout = overlay.health_check_timeout; }
        if overlay.custom_sip_headers.is_some() { base.custom_sip_headers = overlay.custom_sip_headers.clone(); }
        if overlay.number_manipulation_rules.is_some() { base.number_manipulation_rules = overlay.number_manipulation_rules.clone(); }
        if overlay.debug_logging.is_some() { base.debug_logging = overlay.debug_logging; }
        if overlay.priority.is_some() { base.priority = overlay.priority; }
        if overlay.weight.is_some() { base.weight = overlay.weight; }
        
        base
    }
    
    /// Apply trunk-specific overrides to template configuration
    fn apply_overrides(&self, mut config: TrunkTemplateSettings, overrides: &TrunkOverrides) -> TrunkTemplateSettings {
        if overrides.enabled.is_some() { config.enabled = overrides.enabled; }
        if overrides.max_concurrent_calls.is_some() { config.max_concurrent_calls = overrides.max_concurrent_calls; }
        if overrides.rate_limit_cps.is_some() { config.rate_limit_cps = overrides.rate_limit_cps; }
        if overrides.auth_required.is_some() { config.auth_required = overrides.auth_required; }
        if overrides.allowed_source_ips.is_some() { config.allowed_source_ips = overrides.allowed_source_ips.clone(); }
        if overrides.sip_profile.is_some() { config.sip_profile = overrides.sip_profile.clone(); }
        if overrides.default_routing.is_some() { config.default_routing = overrides.default_routing.clone(); }
        if overrides.codec_config.is_some() { config.codec_config = overrides.codec_config.clone(); }
        if overrides.cnam_config.is_some() { config.cnam_config = overrides.cnam_config.clone(); }
        if overrides.billing_profile.is_some() { config.billing_profile = overrides.billing_profile.clone(); }
        if overrides.priority.is_some() { config.priority = overrides.priority; }
        if overrides.weight.is_some() { config.weight = overrides.weight; }
        
        // Handle additional destinations/blocks (append to existing)
        if let Some(ref additional_allowed) = overrides.additional_allowed_destinations {
            if let Some(ref mut allowed) = config.allowed_destinations {
                allowed.extend(additional_allowed.clone());
            } else {
                config.allowed_destinations = Some(additional_allowed.clone());
            }
        }
        
        if let Some(ref additional_blocked) = overrides.additional_blocked_destinations {
            if let Some(ref mut blocked) = config.blocked_destinations {
                blocked.extend(additional_blocked.clone());
            } else {
                config.blocked_destinations = Some(additional_blocked.clone());
            }
        }
        
        // Handle custom headers (merge with template headers)
        if let Some(ref override_headers) = overrides.custom_sip_headers {
            if let Some(ref mut template_headers) = config.custom_sip_headers {
                template_headers.extend(override_headers.clone());
            } else {
                config.custom_sip_headers = Some(override_headers.clone());
            }
        }
        
        config
    }
    
    /// Create predefined templates for common trunk types
    pub fn create_default_templates(&mut self) -> Result<()> {
        // Create base template with common settings
        let base_template = TrunkTemplate {
            name: "base".to_string(),
            description: "Base template with common trunk settings".to_string(),
            inherit_from: None,
            version: "1.0.0".to_string(),
            created_at: chrono::Utc::now(),
            settings: TrunkTemplateSettings::default(),
        };
        self.add_template(base_template)?;
        
        // Create carrier template inheriting from base
        let mut carrier_settings = TrunkTemplateSettings {
            enabled: Some(true),
            max_concurrent_calls: Some(10000),
            rate_limit_cps: Some(1000),
            auth_required: Some(false),
            sip_profile: Some("carrier".to_string()),
            preferred_transport: Some("UDP".to_string()),
            session_timers_enabled: Some(true),
            media_anchoring_enabled: Some(true),
            stir_shaken_verification: Some(true),
            default_attestation_level: Some("A".to_string()),
            monitoring_enabled: Some(true),
            health_check_interval: Some(30),
            ..Default::default()
        };
        
        let carrier_template = TrunkTemplate {
            name: "carrier".to_string(),
            description: "Template for carrier interconnect trunks".to_string(),
            inherit_from: Some("base".to_string()),
            version: "1.0.0".to_string(),
            created_at: chrono::Utc::now(),
            settings: carrier_settings,
        };
        self.add_template(carrier_template)?;
        
        // Create customer template
        let customer_settings = TrunkTemplateSettings {
            enabled: Some(true),
            max_concurrent_calls: Some(100),
            rate_limit_cps: Some(10),
            auth_required: Some(true),
            require_digest_auth: Some(true),
            sip_profile: Some("customer".to_string()),
            session_timers_enabled: Some(true),
            call_recording_enabled: Some(false),
            stir_shaken_verification: Some(false),
            monitoring_enabled: Some(true),
            ..Default::default()
        };
        
        let customer_template = TrunkTemplate {
            name: "customer".to_string(),
            description: "Template for customer SIP trunks".to_string(),
            inherit_from: Some("base".to_string()),
            version: "1.0.0".to_string(),
            created_at: chrono::Utc::now(),
            settings: customer_settings,
        };
        self.add_template(customer_template)?;
        
        // Create internal template for system trunks
        let internal_settings = TrunkTemplateSettings {
            enabled: Some(true),
            max_concurrent_calls: Some(1000),
            rate_limit_cps: Some(100),
            auth_required: Some(false),
            sip_profile: Some("internal".to_string()),
            preferred_transport: Some("TCP".to_string()),
            on_net_routing_enabled: Some(true),
            off_net_routing_enabled: Some(false),
            media_anchoring_enabled: Some(false),
            call_recording_enabled: Some(true),
            recording_retention_days: Some(90),
            monitoring_enabled: Some(false),
            ..Default::default()
        };
        
        let internal_template = TrunkTemplate {
            name: "internal".to_string(),
            description: "Template for internal system trunks".to_string(),
            inherit_from: Some("base".to_string()),
            version: "1.0.0".to_string(),
            created_at: chrono::Utc::now(),
            settings: internal_settings,
        };
        self.add_template(internal_template)?;
        
        info!("Created default trunk templates: base, carrier, customer, internal");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_template_inheritance() {
        let mut manager = TrunkTemplateManager::new();
        manager.create_default_templates().unwrap();
        
        // Test that carrier template inherits from base
        let carrier_config = manager.resolve_template("carrier").unwrap();
        assert!(carrier_config.enabled.unwrap());
        assert_eq!(carrier_config.max_concurrent_calls.unwrap(), 10000);
        assert_eq!(carrier_config.sip_profile.unwrap(), "carrier");
    }
    
    #[test]
    fn test_trunk_assignment() {
        let mut manager = TrunkTemplateManager::new();
        manager.create_default_templates().unwrap();
        
        let overrides = TrunkOverrides {
            max_concurrent_calls: Some(500),
            ..Default::default()
        };
        
        manager.assign_template("trunk1".to_string(), "carrier".to_string(), overrides).unwrap();
        
        let config = manager.get_trunk_config("trunk1").unwrap();
        assert_eq!(config.max_concurrent_calls.unwrap(), 500); // Override applied
        assert_eq!(config.sip_profile.unwrap(), "carrier"); // Template setting
    }
    
    #[test]
    fn test_circular_inheritance_detection() {
        let mut manager = TrunkTemplateManager::new();
        
        let template1 = TrunkTemplate {
            name: "template1".to_string(),
            description: "Test template 1".to_string(),
            inherit_from: Some("template2".to_string()),
            version: "1.0.0".to_string(),
            created_at: chrono::Utc::now(),
            settings: TrunkTemplateSettings::default(),
        };
        
        let template2 = TrunkTemplate {
            name: "template2".to_string(),
            description: "Test template 2".to_string(),
            inherit_from: Some("template1".to_string()),
            version: "1.0.0".to_string(),
            created_at: chrono::Utc::now(),
            settings: TrunkTemplateSettings::default(),
        };
        
        manager.add_template(template1).unwrap();
        assert!(manager.add_template(template2).is_err()); // Should detect circular inheritance
    }
}

impl Default for TrunkOverrides {
    fn default() -> Self {
        Self {
            enabled: None,
            max_concurrent_calls: None,
            rate_limit_cps: None,
            auth_required: None,
            allowed_source_ips: None,
            sip_profile: None,
            default_routing: None,
            codec_config: None,
            cnam_config: None,
            billing_profile: None,
            custom_sip_headers: None,
            priority: None,
            weight: None,
            additional_allowed_destinations: None,
            additional_blocked_destinations: None,
        }
    }
}